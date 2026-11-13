import { assertEquals, assertThrows } from "jsr:@std/assert@^1";
import { dirname, fromFileUrl, join } from "jsr:@std/path@^1";
import { configToBundleArgs, defaultOutfile, exportsToGlobals, importsToGlobals } from "./deno_bundler.js";

Deno.test("code without exports is unchanged", () => {
  assertEquals(exportsToGlobals("function a() {}\n"), "function a() {}\n");
});

Deno.test("code without imports is unchanged", () => {
  const code = 'function a() { return import("node:fs"); }\nvar important = 1;\n';
  assertEquals(importsToGlobals(code), code);
});

Deno.test("static imports are preloaded and keep line numbers", () => {
  const out = importsToGlobals(
    '// main.ts\nimport d, * as ns from "node:path";\nimport fs, { a, b as c } from \'node:fs\';\nimport "node:process";\nvar x = import.meta.url;\n',
  );
  const I = "__tauri_plugin_deno_imports";
  assertEquals(
    out,
    `var _tauri_plugin_imports = ["node:path","node:fs","node:process"]; ` +
      `const d = ${I}["node:path"].default, ns = ${I}["node:path"]; ` +
      `const fs = ${I}["node:fs"].default, { a, b: c } = ${I}["node:fs"]; ` +
      "// main.ts\n\n\n\nvar x = __tauri_plugin_deno_import_meta.url;\n",
  );
});

Deno.test("minified imports are preloaded", () => {
  assertEquals(
    importsToGlobals('import{readFileSync as r}from"node:fs";var x=r;'),
    'var _tauri_plugin_imports = ["node:fs"]; const { readFileSync: r } = __tauri_plugin_deno_imports["node:fs"]; var x=r;',
  );
});

Deno.test("exports become globals and are listed", () => {
  const out = exportsToGlobals("function a() {}\nvar b2 = 1;\nexport {\n  a,\n  b2 as b,\n  a as default\n};\n");
  assertEquals(
    out,
    'function a() {}\nvar b2 = 1;\nglobalThis["b"] = b2;\nvar _tauri_plugin_exports = ["a","b"];\n',
  );
});

Deno.test("minified exports become globals", () => {
  assertEquals(
    exportsToGlobals("function u(){}var s=1;export{u as readOwnSource,s};"),
    'function u(){}var s=1;\nglobalThis["readOwnSource"] = u;\nvar _tauri_plugin_exports = ["readOwnSource","s"];\n',
  );
});

Deno.test("a trailing source map comment is kept", () => {
  assertEquals(
    exportsToGlobals("function a() {}\nexport {\n  a\n};\n//# sourceMappingURL=data:abc\n"),
    'function a() {}\nvar _tauri_plugin_exports = ["a"];\n//# sourceMappingURL=data:abc\n',
  );
});

Deno.test("a relative CARGO_TARGET_DIR is relative to src-tauri", () => {
  const saved = Deno.env.get("CARGO_TARGET_DIR");
  Deno.env.delete("CARGO_TARGET_DIR");
  try {
    assertEquals(defaultOutfile(undefined), join("src-tauri", "target", "deno_dist.js"));
    assertEquals(defaultOutfile("out"), join("src-tauri", "out", "deno_dist.js"));
    const abs = join(Deno.cwd(), "abs");
    assertEquals(defaultOutfile(abs), join(abs, "deno_dist.js"));
  } finally {
    if (saved !== undefined) Deno.env.set("CARGO_TARGET_DIR", saved);
  }
});

Deno.test("trailing comments after the exports are kept", () => {
  assertEquals(
    exportsToGlobals("function a() {}\nexport {\n  a\n};\n/*! license */\n// end\n//# sourceMappingURL=data:abc\n"),
    'function a() {}\nvar _tauri_plugin_exports = ["a"];\n/*! license */\n// end\n//# sourceMappingURL=data:abc\n',
  );
});

Deno.test("unsupported export syntax is rejected", () => {
  assertThrows(() => exportsToGlobals("export default 1;\nfoo();"));
});

Deno.test("esbuild-style config maps to deno bundle flags", () => {
  assertEquals(
    configToBundleArgs({ minify: true, keepNames: true, sourcemap: "inline", external: ["a", "b"] }),
    ["--minify", "--keep-names", "--sourcemap=inline", "--external", "a", "--external", "b"],
  );
  assertThrows(() => configToBundleArgs({ target: "es2020" }));
});

// Installed packages live in node_modules, where Deno forbids `npm:`/`jsr:` imports.
Deno.test("CLI bundles a project when installed in node_modules", async () => {
  const dir = await Deno.makeTempDir();
  try {
    const cli = join(dir, "node_modules", "tauri-plugin-deno-api", "deno_bundler.js");
    await Deno.mkdir(dirname(cli), { recursive: true });
    await Deno.copyFile(new URL("./deno_bundler.js", import.meta.url), cli);
    await Deno.mkdir(join(dir, "src-tauri", "src-deno"), { recursive: true });
    await Deno.writeTextFile(join(dir, "src-tauri", "src-deno", "util.ts"), "export const twice = (x: number): number => x * 2;\n");
    await Deno.writeTextFile(
      join(dir, "src-tauri", "src-deno", "main.ts"),
      'import { twice } from "./util.ts";\nexport function four(): number { return twice(2); }\n',
    );
    const { code, stderr } = await new Deno.Command(Deno.execPath(), {
      args: ["run", "-A", cli, "bundle"],
      cwd: join(dir, "src-tauri"),
      env: { CARGO_TARGET_DIR: "target" },
      stdout: "null",
    }).output();
    assertEquals(code, 0, new TextDecoder().decode(stderr));
    const bundle = await Deno.readTextFile(join(dir, "src-tauri", "target", "deno_dist.js"));
    assertEquals(/^\s*export\s/m.test(bundle), false);
    assertEquals(new Function(`${bundle}; return [four(), _tauri_plugin_exports];`)(), [4, ["four"]]);
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});

Deno.test("CLI turns node imports into a classic script", async () => {
  const dir = await Deno.makeTempDir();
  try {
    await Deno.mkdir(join(dir, "src-tauri", "src-deno"), { recursive: true });
    await Deno.writeTextFile(
      join(dir, "src-tauri", "src-deno", "main.ts"),
      'import { join } from "node:path";\nimport { createRequire } from "node:module";\n' +
        'const os = createRequire(import.meta.url)("node:os");\n' +
        'export function info(): string[] { return [join("a", "b"), typeof os.platform]; }\n',
    );
    const { code, stderr } = await new Deno.Command(Deno.execPath(), {
      args: ["run", "-A", fromFileUrl(new URL("./deno_bundler.js", import.meta.url)), "bundle"],
      cwd: dir,
      env: { CARGO_TARGET_DIR: "target" },
      stdout: "null",
    }).output();
    assertEquals(code, 0, new TextDecoder().decode(stderr));
    const bundle = await Deno.readTextFile(join(dir, "src-tauri", "target", "deno_dist.js"));
    assertEquals(/^\s*import\s/m.test(bundle) || /\bimport\.meta\b/.test(bundle), false, bundle);
    const { createRequire } = await import("node:module");
    globalThis.__tauri_plugin_deno_imports = { "node:path": await import("node:path"), "node:module": { createRequire } };
    globalThis.__tauri_plugin_deno_import_meta = import.meta;
    assertEquals(new Function(`${bundle}; return info();`)(), [join("a", "b"), "function"]);
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});

// The outfile's folder contains the sources, and the import is outside the entry's folder.
Deno.test("bundle --watch rebundles on changes in imported files", async () => {
  const dir = await Deno.realPath(await Deno.makeTempDir());
  const util = join(dir, "shared", "util.ts");
  const outfile = join(dir, "deno_dist.js");
  await Deno.mkdir(dirname(util), { recursive: true });
  await Deno.mkdir(join(dir, "src-tauri", "src-deno"), { recursive: true });
  await Deno.writeTextFile(util, 'export const word = "first";\n');
  await Deno.writeTextFile(
    join(dir, "src-tauri", "src-deno", "main.ts"),
    'import { word } from "../../shared/util.ts";\nexport function get(): string { return word; }\n',
  );
  const child = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", fromFileUrl(new URL("./deno_bundler.js", import.meta.url)), "bundle", "--watch", "--outfile", outfile],
    cwd: dir,
    stdout: "null",
    stderr: "null",
  }).spawn();
  const waitFor = async (text) => {
    for (let i = 0; i < 100; i++) {
      try {
        if ((await Deno.readTextFile(outfile)).includes(text)) return true;
      } catch {
        // not written yet
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    return false;
  };
  try {
    assertEquals(await waitFor('"first"'), true);
    await new Promise((resolve) => setTimeout(resolve, 1000)); // until the watcher is set up
    await Deno.writeTextFile(util, 'export const word = "second";\n');
    assertEquals(await waitFor('"second"'), true);
  } finally {
    child.kill();
    await child.status;
    await Deno.remove(dir, { recursive: true });
  }
});

// `dev`/`build` used to hang: the CLI must exit with tauri's exit code.
Deno.test("build runs tauri with the bundle path and exits with its code", async () => {
  const dir = await Deno.makeTempDir();
  try {
    await Deno.mkdir(join(dir, "src-tauri", "src-deno"), { recursive: true });
    await Deno.writeTextFile(join(dir, "src-tauri", "src-deno", "main.js"), "export const a = 1;\n");
    const bin = join(dir, "node_modules", ".bin");
    await Deno.mkdir(bin, { recursive: true });
    await Deno.writeTextFile(join(bin, "tauri"), '#!/bin/sh\necho "$@ $TAURI_PLUGIN_DENO_DIST"\nexit 3\n', { mode: 0o755 });
    await Deno.writeTextFile(join(bin, "tauri.cmd"), "@echo %* %TAURI_PLUGIN_DENO_DIST%\r\n@exit /b 3\r\n");
    const { code, stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["run", "-A", fromFileUrl(new URL("./deno_bundler.js", import.meta.url)), "build", "--", "--debug"],
      cwd: dir,
      env: { CARGO_TARGET_DIR: "target" },
    }).output();
    assertEquals(code, 3);
    // On Windows, the arguments reach tauri.cmd quoted, so `%*` echoes `"build" "--debug"`.
    const out = new TextDecoder().decode(stdout).replaceAll('"', "");
    assertEquals(out.includes("build --debug"), true, out);
    assertEquals(out.includes(join("src-tauri", "target", "deno_dist.js")), true, out);
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});
