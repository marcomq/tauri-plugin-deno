# Tauri Plugin deno

This [tauri](https://v2.tauri.app/) v2 plugin lets you write your app backend in JavaScript or TypeScript, similar to Electrobun or Neutralinojs, but on top of Tauri.

The code runs in an embedded Deno runtime (V8), inside the app process. No sidecar, no Node installation and no network server are needed. Most Deno APIs are available, for example reading and writing files or `fetch`.

> **Note on Deno's future:** in October 2026 the Deno team announced that it is joining Cloudflare ([deno.com/blog/cloudflare](https://deno.com/blog/cloudflare)). The runtime receives bugfix and security releases for about one more year. This plugin stays on Deno for now; a successor plugin with another engine may follow later.

## How it works

- Your backend code lives in `src-tauri/src-deno/main.ts` (or `main.js`) and may import local files, `npm:` and `jsr:` packages.
- The CLI bundles it with `deno bundle` into a single file `src-tauri/target/deno_dist.js`. The plugin embeds this file at compile time (in debug builds it is re-read from disk on every start).
- On startup, a single background thread runs the bundle. Frontend calls, events and results are passed over channels.

## Setup

Preconditions in addition to the usual [tauri prerequisites](https://v2.tauri.app/start/prerequisites/):
- [Deno](https://deno.com) ≥ 2.4 (for `deno bundle`). You can also add it as npm dev dependency: `npm install -D deno`. npm ≥ 11.19 skips its optional postinstall (which only swaps a Node wrapper for the native binary); `npm install-scripts approve deno --no-allow-scripts-pin` allows it.
- Windows: clang/LLVM (`choco install llvm`), needed to compile V8.

Steps:
1. `npm run tauri add deno` (adds the crate, `tauri-plugin-deno-api` and the default permission)
2. Create `src-tauri/src-deno/main.ts`.
3. Use the CLI in your `package.json` scripts:

```json
"scripts": {
  "dev": "deno run -A npm:tauri-plugin-deno-api dev",
  "build": "deno run -A npm:tauri-plugin-deno-api build",
  "bundle": "deno run -A npm:tauri-plugin-deno-api bundle",
  "tauri": "tauri"
}
```

Also set `"beforeBuildCommand": "npm run bundle"` in `tauri.conf.json`, so a plain `tauri build` bundles as well.

### CLI

```
deno run -A npm:tauri-plugin-deno-api <command> [options] [-- tauri args]

  bundle          Bundle the Deno code once and exit
  bundle --watch  Bundle and rebundle on every change
  dev             Bundle, watch and run `tauri dev`
  build           Bundle and run `tauri build`

  --entry <file>    Entry point (default: src-tauri/src-deno/main.ts or main.js)
  --outfile <file>  Bundle file (default: src-tauri/target/deno_dist.js)
  --config <json>   Bundle options: minify, keepNames, sourcemap, external
```

In `dev`, changes in `src-tauri/src-deno` are rebundled and restart the app, like Rust changes do.
`dev` and `build` pass `TAURI_PLUGIN_DENO_DIST` to tauri, so the Rust build always finds the bundle. Without the CLI, the plugin's `build.rs` looks for `deno_dist.js` in the target directory, or uses `TAURI_PLUGIN_DENO_DIST` (absolute path) if set.

## Usage

### Make Deno functions callable

Only functions that are exported or listed in `_tauri_plugin_functions` can be called from the frontend.

```typescript
// src-tauri/src-deno/main.ts
function greet(name: string): string {
  return `Hello ${name} from Deno`;
}
_tauri_plugin_functions = [greet.name]; // or: addTauri(greet)

export async function readConfig(path: string) {
  return JSON.parse(await Deno.readTextFile(path));
}
```

Exported constants and classes are ignored; only functions become callable. Methods like `api.list` (via `registerFunction`) keep `this`. Don't name your own globals `emit`, `listen` or `addTauri`, as that replaces the plugin's functions.

Async functions are awaited. Arguments and results use JSON semantics (numbers, strings, arrays, objects, `null`); values that JSON can't represent, like functions or symbols, become `null`/`undefined`.

### Call them from the frontend

```typescript
import { callFunction, createApi } from "tauri-plugin-deno-api";

const text = await callFunction("greet", ["world"]);

// Typed RPC: the module's types flow into the frontend.
import type * as backend from "../src-tauri/src-deno/main.ts";
const api = createApi<typeof backend>();
const config = await api.readConfig("config.json"); // Promise<any>
const hello = await api.greet("world");             // Promise<string>
```

Without a bundler, use the global API (`"withGlobalTauri": true`): `await window.__TAURI__.deno.callFunction("greet", ["world"])`, or `registerJs("greet")` followed by `window.__TAURI__.deno.call.greet("world")`.

### Errors

If a Deno function throws, the frontend promise is rejected with the error message:

```javascript
try {
  await callFunction("greet", []);
} catch (error) {
  console.error(error); // e.g. "Error calling Deno function 'greet': ..."
}
```

A thrown error never stops the Deno runtime; later calls keep working.

### Events in both directions

```typescript
// src-tauri/src-deno/main.ts
emit("progress", { percent: 50 });            // to the frontend
const unlisten = listen("ping", (payload) => { // from the frontend
  emit("pong", payload);
});
```

```javascript
// frontend
import { emit, listen } from "@tauri-apps/api/event";
await listen("pong", (event) => console.log(event.payload));
await emit("ping", "hello");
```

`emit`/`listen` in Deno use regular Tauri events, so Rust code (`app.emit`, `app.listen_any`) can take part as well.

### Rust API

```rust
use tauri_plugin_deno::DenoExt;
use serde_json::json;

let result = app.deno().call_function("greet", vec![json!("world")]).await?;
let value = app.deno().run_code("1 + 1").await?;
```

Also available: `register_function` and `read_variable`.

## Configuration

| Environment variable | Effect |
| -------------------- | ------ |
| `TAURI_PLUGIN_DENO_TRACEBACK=1` | Include JS stack traces in errors and log them to stderr in release builds (always on in debug builds). |
| `TAURI_PLUGIN_DENO_TIMEOUT_SECS` | Timeout per call in seconds, default `300`, `0` disables it. After a timeout the frontend gets an error, but the Deno thread stays busy until the hanging call finishes. |
| `TAURI_PLUGIN_DENO_DIST` | Build time: absolute path of the bundle to embed. |

All plugin messages on stderr are prefixed with `[tauri-plugin-deno]`.

## Debugging

- Errors arrive as promise rejections; in debug builds they include the JS stack trace.
- If `main.ts` itself fails on startup, every call is rejected with `Deno runtime failed to start: …`, which contains the original error.
- `Function 'x' has not been registered`: export the function or add it to `_tauri_plugin_functions`.

### Works in `tauri dev` but not in the production build?

- Was the bundle created before `tauri build`? Use the `build` command of the CLI or `beforeBuildCommand`.
- Release builds embed the bundle at compile time. If you changed Deno code, the Rust build has to run again (the CLI handles this).
- Relative paths (e.g. `Deno.readTextFile("data.json")`) resolve against the working directory, which differs between `tauri dev` and an installed app. Use paths from Tauri, for example the resource directory, passed in as arguments.
- In a Cargo workspace, the target directory is at the workspace root, so `build.rs` can't find `src-tauri/target/deno_dist.js`. Use the CLI (`dev`/`build`), which passes `TAURI_PLUGIN_DENO_DIST`, or set it yourself.
- Start the binary from a terminal to see stderr output.
- Windows: release apps usually have no console (`windows_subsystem = "windows"`). The plugin guards `console.*` against the missing console, so logging there does not throw, but the output is lost.

## Security considerations

- The Deno code runs with **all permissions** (`allow_all`): full file, network, environment and process access. Treat it like Rust backend code.
- No network server or client is started by the plugin itself.
- By default, the frontend can only call functions that the backend exported or listed in `_tauri_plugin_functions`.

Additional permissions, configured in `src-tauri/capabilities/default.json`:

- `deno:allow-call-function` (`callFunction`; enabled by default)
- `deno:allow-read-variable` (`readVariable`)
- `deno:allow-register-function` (`registerFunction`; lets the frontend make any global function callable)
- `deno:allow-run-code` (`runCode`; runs arbitrary code in the backend)

Only enable `run-code` or `register-function` if the frontend never loads remote content.

## Status

Tested on macOS; Linux and Windows are covered by CI. Android and iOS are not supported.

## Development

```bash
pnpm install
pnpm build                     # guest-js -> dist-js
pnpm test                      # CLI tests (deno)
TAURI_PLUGIN_DENO_DIST=$PWD/tests/fixtures/deno_dist.js cargo test
```
