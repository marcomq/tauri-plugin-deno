#!/usr/bin/env -S deno run -A

/** Tauri Plugin Deno
 * © Copyright 2025, by Marco Mengelkoch
 * Licensed under MIT License, see License file for more details
 * git clone https://github.com/marcomq/tauri-plugin-deno
 *
 * CLI: bundles `src-tauri/src-deno/main.(ts|js)` into `src-tauri/target/deno_dist.js`
 * using `deno bundle` and optionally runs `tauri dev` / `tauri build`. Run it with
 *   deno run -A npm:tauri-plugin-deno-api <bundle|dev|build> [options] [-- tauri args]
 **/

// Only `node:` imports: Deno refuses `npm:`/`jsr:` specifiers in files inside node_modules.
import { basename, dirname, isAbsolute, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const PREFIX = "[tauri-plugin-deno]";
const IS_WINDOWS = Deno.build.os === "windows";

const USAGE = `Usage: deno run -A npm:tauri-plugin-deno-api <command> [options] [-- tauri args]

Commands:
  bundle          Bundle the Deno code once and exit
  bundle --watch  Bundle and rebundle on every change
  dev             Bundle, watch and run \`tauri dev\`
  build           Bundle and run \`tauri build\`

Options:
  --entry <file>    Entry point (default: src-tauri/src-deno/main.ts or main.js)
  --outfile <file>  Bundle file (default: src-tauri/target/deno_dist.js,
                    or $CARGO_TARGET_DIR/deno_dist.js if set)
  --config <json>   Bundle options: minify, keepNames, sourcemap, external,
                    e.g. '{"minify":true}'
  --watch           Keep watching (bundle only)

Arguments after \`--\` (and unknown arguments for dev/build) are passed to tauri.`;

function fail(message) {
  console.error(`${PREFIX} ${message}`);
  Deno.exit(1);
}

function exists(path) {
  try {
    Deno.statSync(path);
    return true;
  } catch {
    return false;
  }
}

function parseArgs(args) {
  const opts = { command: args[0], watch: false, config: {}, tauriArgs: [] };
  const known = { "--entry": "entry", "--outfile": "outfile", "--config": "config" };
  for (let i = 1; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--") {
      opts.tauriArgs.push(...args.slice(i + 1));
      break;
    } else if (arg === "--watch") {
      opts.watch = true;
    } else if (arg in known) {
      const value = args[++i];
      if (value === undefined) fail(`${arg} needs a value`);
      opts[known[arg]] = value;
    } else {
      opts.tauriArgs.push(arg);
    }
  }
  if (typeof opts.config === "string") {
    try {
      opts.config = JSON.parse(opts.config);
    } catch (err) {
      fail(`--config is not valid JSON: ${err.message}`);
    }
  }
  return opts;
}

/** Old usage: `deno_bundler.js <entry> [json]`, which always watched. */
function parseLegacyArgs(args) {
  console.warn(
    `${PREFIX} Deprecated usage. Use \`bundle --watch --entry ${args[0] ?? "<file>"}\` or \`dev\` / \`build\` instead.`,
  );
  let config = {};
  if (args[1]) {
    try {
      config = JSON.parse(args[1]);
    } catch (err) {
      fail(`cannot parse JSON options: ${err.message}`);
    }
  }
  return { command: "bundle", watch: true, entry: args[0], config, tauriArgs: [] };
}

function defaultEntry() {
  const candidates = ["src-tauri/src-deno/main.ts", "src-tauri/src-deno/main.js"];
  return candidates.find(exists) ?? fail(
    `No entry point found (looked for ${candidates.join(", ")}). Pass one with --entry.`,
  );
}

/** Cargo runs in `src-tauri`, so a relative CARGO_TARGET_DIR is relative to it. */
export function defaultOutfile(targetDir = Deno.env.get("CARGO_TARGET_DIR")) {
  return join(isAbsolute(targetDir ?? "") ? targetDir : join("src-tauri", targetDir || "target"), "deno_dist.js");
}

/**
 * The plugin runs the bundle as a classic script, which can't contain `export`.
 * Replaces the trailing ESM export list with globals and records the exported names
 * in `_tauri_plugin_exports`, so every export is callable from the frontend.
 */
export function exportsToGlobals(code) {
  // Trailing comments (e.g. license comments or the source map) follow the export list.
  const match = code.match(/(?<=^|[\n;}])\s*export\s*\{([^}]*)\};?((?:\s*(?:\/\/[^\n]*|\/\*[\s\S]*?\*\/))*)\s*$/);
  if (!match) {
    if (/(^|[\n;}])\s*export[\s{*]/.test(code)) {
      throw new Error("unsupported export syntax in bundle output");
    }
    return code;
  }
  const lines = [];
  const names = [];
  for (const spec of match[1].split(",").map((s) => s.trim()).filter(Boolean)) {
    const [local, exported = local] = spec.split(/\s+as\s+/).map((s) => s.trim());
    if (exported === "default" || exported.startsWith('"')) continue;
    if (local !== exported) lines.push(`globalThis[${JSON.stringify(exported)}] = ${local};`);
    names.push(exported);
  }
  lines.push(`var _tauri_plugin_exports = ${JSON.stringify(names)};`);
  const comments = match[2].trimStart();
  return code.slice(0, match.index) + "\n" + lines.join("\n") + "\n" + (comments && `${comments}\n`);
}

const IMPORT_RE =
  /(?<=^|[\n;}])[ \t]*import\b\s*(?:([\w$]+\s*,\s*\*\s*as\s+[\w$]+|[\w$]+\s*,\s*\{[^}]*\}|\*\s*as\s+[\w$]+|\{[^}]*\}|[\w$]+)\s*from\s*)?("[^"\n]*"|'[^'\n]*')[ \t]*;?/g;

function importBindings(clause, source) {
  const [, def, ns, named] = clause.match(/^([\w$]+)?\s*,?\s*(?:\*\s*as\s+([\w$]+)|\{([^}]*)\})?$/);
  const parts = [];
  if (def) parts.push(`${def} = ${source}.default`);
  if (ns) parts.push(`${ns} = ${source}`);
  if (named !== undefined) {
    const props = named.split(",").map((s) => s.trim()).filter(Boolean).map((spec) => {
      const [imported, local = imported] = spec.split(/\s+as\s+/).map((s) => s.trim());
      return imported === local ? local : `${imported}: ${local}`;
    });
    parts.push(`{ ${props.join(", ")} } = ${source}`);
  }
  return parts;
}

/**
 * The plugin runs the bundle as a classic script, which can't contain static imports
 * (e.g. `node:fs`, kept by `deno bundle`) or `import.meta`. Removes the imports and lists
 * them in `_tauri_plugin_imports` on line 1, so the runtime can preload them; their
 * bindings are read from `__tauri_plugin_deno_imports`. Line numbers stay unchanged.
 */
export function importsToGlobals(code) {
  const specifiers = [];
  const declarations = [];
  let out = code.replace(IMPORT_RE, (statement, clause, quoted) => {
    const specifier = quoted.slice(1, -1);
    if (!specifiers.includes(specifier)) specifiers.push(specifier);
    const parts = clause ? importBindings(clause, `__tauri_plugin_deno_imports[${JSON.stringify(specifier)}]`) : [];
    if (parts.length) declarations.push(`const ${parts.join(", ")};`);
    return "\n".repeat(statement.split("\n").length - 1);
  });
  out = out.replace(/\bimport\.meta\b/g, "__tauri_plugin_deno_import_meta");
  if (out === code) return code;
  return `var _tauri_plugin_imports = ${JSON.stringify(specifiers)}; ${declarations.join(" ")} ${out}`;
}

const CONFIG_FLAGS = {
  minify: (v) => (v ? ["--minify"] : []),
  keepNames: (v) => (v ? ["--keep-names"] : []),
  sourcemap: (v) => (v ? [v === true ? "--sourcemap=inline" : `--sourcemap=${v}`] : []),
  external: (v) => [].concat(v).flatMap((name) => ["--external", name]),
};

/** Maps the supported esbuild-style options to `deno bundle` flags. */
export function configToBundleArgs(config) {
  return Object.entries(config).flatMap(([key, value]) => {
    if (!(key in CONFIG_FLAGS)) {
      throw new Error(`unsupported --config option '${key}' (supported: ${Object.keys(CONFIG_FLAGS).join(", ")})`);
    }
    return CONFIG_FLAGS[key](value);
  });
}

function writeAtomic(path, content) {
  Deno.mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.${Deno.pid}.tmp`;
  Deno.writeTextFileSync(tmp, content);
  Deno.renameSync(tmp, path);
}

function createBundler(entry, outfile, config) {
  const extraArgs = configToBundleArgs(config);
  const tmp = `${outfile}.${Deno.pid}.bundle.tmp.js`;
  return async function run() {
    const started = performance.now();
    Deno.mkdirSync(dirname(outfile), { recursive: true });
    const { code, stderr } = await new Deno.Command(Deno.execPath(), {
      args: ["bundle", "--platform", "deno", "--format", "esm", ...extraArgs, "-o", tmp, entry],
      env: { NO_COLOR: "1" },
      stdout: "null",
      stderr: "piped",
    }).output();
    try {
      if (code !== 0) {
        const messages = new TextDecoder().decode(stderr).split("\n")
          .filter((line) => line.trim() && !line.includes("deno bundle is experimental"));
        console.error(`${PREFIX} bundling failed:\n  ${messages.join("\n  ")}`);
        return false;
      }
      writeAtomic(outfile, exportsToGlobals(importsToGlobals(Deno.readTextFileSync(tmp))));
      console.log(`${PREFIX} bundled ${entry} -> ${outfile} (${Math.round(performance.now() - started)} ms)`);
      return true;
    } catch (err) {
      console.error(`${PREFIX} bundling failed: ${err.message}`);
      return false;
    } finally {
      try {
        Deno.removeSync(tmp);
      } catch {
        // not written
      }
    }
  };
}

/** Runs `task` again after it finishes if it was requested while running. */
function serialized(task) {
  let running = null;
  let again = false;
  const loop = async () => {
    do {
      again = false;
      await task();
    } while (again);
    running = null;
  };
  return () => {
    if (running) again = true;
    else running = loop();
    return running;
  };
}

/** Directories of the entry and of all local files it imports, according to `deno info`. */
async function sourceDirs(entry) {
  const dirs = [dirname(resolve(entry))];
  try {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["info", "--json", entry],
      env: { NO_COLOR: "1" },
      stdout: "piped",
      stderr: "null",
    }).output();
    for (const { specifier } of JSON.parse(new TextDecoder().decode(stdout)).modules ?? []) {
      if (specifier.startsWith("file:") && !specifier.includes("/node_modules/")) {
        dirs.push(dirname(fileURLToPath(specifier)));
      }
    }
  } catch {
    // e.g. a syntax error; the directories watched so far are kept
  }
  return dirs;
}

/** True for the bundle and its temporary files, so writing them doesn't trigger a rebundle. */
function outputMatcher(outfile) {
  const paths = [resolve(outfile)];
  try {
    // Watch events may report the real path, e.g. /private/var instead of /var on macOS.
    paths.push(join(Deno.realPathSync(dirname(paths[0])), basename(paths[0])));
  } catch {
    // not created yet
  }
  return (path) => paths.some((out) => path === out || path.startsWith(`${out}.`));
}

/**
 * Rebundles when a source file changes. The watched directories are updated after
 * every bundle, since imports may have changed; directories are only added.
 */
function watchSources(entry, outfile, bundle) {
  const isOutput = outputMatcher(outfile);
  let dirs = [];
  let watcher = null;
  let timer;
  let stopped = false;
  const update = async () => {
    const found = [...new Set([...dirs, ...(await sourceDirs(entry))])].filter(exists).sort()
      .filter((dir, i, all) => !all.slice(0, i).some((parent) => dir.startsWith(parent + sep)));
    if (stopped || found.join("\n") === dirs.join("\n")) return;
    dirs = found;
    watcher?.close();
    const current = (watcher = Deno.watchFs(dirs, { recursive: true }));
    console.log(`${PREFIX} watching ${dirs.join(", ")}`);
    (async () => {
      try {
        for await (const event of current) {
          if (event.paths.every(isOutput)) continue;
          clearTimeout(timer);
          timer = setTimeout(rebuild, 30);
        }
      } catch {
        // closed
      }
    })();
  };
  const rebuild = serialized(async () => {
    await bundle();
    await update();
  });
  return {
    ready: update(),
    stop: () => {
      stopped = true;
      clearTimeout(timer);
      watcher?.close();
    },
  };
}

function findTauri() {
  const bin = join("node_modules", ".bin", IS_WINDOWS ? "tauri.cmd" : "tauri");
  if (exists(bin)) return { cmd: resolve(bin), args: [] };
  return { cmd: IS_WINDOWS ? "npx.cmd" : "npx", args: ["tauri"] };
}

async function runTauri(subcommand, args, outfile) {
  const tauri = findTauri();
  const child = new Deno.Command(tauri.cmd, {
    args: [...tauri.args, subcommand, ...args],
    env: { TAURI_PLUGIN_DENO_DIST: resolve(outfile) },
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  }).spawn();
  // Ctrl+C reaches tauri directly; keep this process alive to clean up afterwards.
  const ignore = () => {};
  const signals = IS_WINDOWS ? ["SIGINT", "SIGBREAK"] : ["SIGINT", "SIGTERM"];
  for (const signal of signals) {
    try {
      Deno.addSignalListener(signal, ignore);
    } catch {
      // signal not supported on this platform
    }
  }
  const status = await child.status;
  return status.code;
}

async function main(args) {
  if (args.length === 0 || ["-h", "--help", "help"].includes(args[0])) {
    console.log(USAGE);
    return 0;
  }
  const isCommand = ["bundle", "dev", "build"].includes(args[0]);
  const opts = isCommand ? parseArgs(args) : parseLegacyArgs(args);
  if (opts.command === "bundle" && opts.tauriArgs.length > 0) {
    fail(`unknown arguments for bundle: ${opts.tauriArgs.join(" ")}\n\n${USAGE}`);
  }
  const entry = opts.entry ?? defaultEntry();
  if (!exists(entry)) fail(`entry point '${entry}' not found`);
  const outfile = opts.outfile ?? defaultOutfile();

  let bundle;
  try {
    bundle = createBundler(entry, outfile, opts.config);
  } catch (err) {
    fail(err.message);
  }
  const ok = await bundle();
  const watching = opts.command === "dev" || opts.watch;
  // Only `bundle --watch` keeps going after a failed first bundle.
  if (!ok && !(opts.command === "bundle" && opts.watch)) return 1;

  let stopWatching = () => {};
  if (watching) {
    const watcher = watchSources(entry, outfile, bundle);
    await watcher.ready;
    stopWatching = watcher.stop;
  }
  if (opts.command === "bundle") {
    if (watching) await new Promise(() => {});
    return 0;
  }
  const code = await runTauri(opts.command, opts.tauriArgs, outfile);
  stopWatching();
  return code;
}

if (import.meta.main) {
  // Run relative to the project, even when started from a subfolder like `src-tauri`.
  if (!exists("src-tauri") && exists(join("..", "src-tauri"))) Deno.chdir("..");
  Deno.exit(await main(Deno.args));
}
