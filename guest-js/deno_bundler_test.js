import { assertEquals, assertThrows } from "jsr:@std/assert@^1";
import { dirname, fromFileUrl, join } from "jsr:@std/path@^1";
import { configToBundleArgs, defaultOutfile, exportsToGlobals } from "./deno_bundler.js";

Deno.test("code without exports is unchanged", () => {
  assertEquals(exportsToGlobals("function a() {}\n"), "function a() {}\n");
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
  assertEquals(defaultOutfile(undefined), join("src-tauri", "target", "deno_dist.js"));
  assertEquals(defaultOutfile("out"), join("src-tauri", "out", "deno_dist.js"));
  const abs = join(Deno.cwd(), "abs");
  assertEquals(defaultOutfile(abs), join(abs, "deno_dist.js"));
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
    }).output();
    assertEquals(code, 3);
    const out = new TextDecoder().decode(stdout);
    assertEquals(out.includes("build --debug"), true, out);
    assertEquals(out.includes(join("src-tauri", "target", "deno_dist.js")), true, out);
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});
