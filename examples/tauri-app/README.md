# Deno example

A vanilla JavaScript frontend with a Deno backend in `src-tauri/src-deno`. Any frontend framework works the same way.

```bash
npm install
npm run dev     # bundle, watch src-tauri/src-deno and run `tauri dev`
npm run build   # bundle and run `tauri build`
npm run bundle  # only create src-tauri/target/deno_dist.js
npm run e2e -- src-tauri/target/release/tauri-app  # click test, see below
```

What the example shows (`src-tauri/src-deno/main.js` and `src/main.js`):
- `greetJs` / `failJs`: functions listed in `_tauri_plugin_functions`, called via `registerJs` and `call.*`; the error of `failJs` is caught in the frontend.
- `slowSquare`: an exported async function, called through `createApi()`.
- `ping` / `pong`: the frontend emits an event, Deno listens and answers with `emit`.
- `other.ts`: TypeScript imports are bundled as well.

Changes compared to a plain `create-tauri-app` project:
- `.plugin(tauri_plugin_deno::init())` in `src-tauri/src/lib.rs` and `"deno:default"` in the capabilities (done by `tauri add deno`)
- the `dev`, `build` and `bundle` scripts in `package.json`, and `deno` as dev dependency
- `"beforeBuildCommand": "npm run bundle"` in `src-tauri/tauri.conf.json`

## Click test

`e2e/run.mjs` starts a built app with `TAURI_E2E_SCRIPT=e2e/clicks.js`. The hook in `src-tauri/src/lib.rs` loads that script into the page; it clicks every button, checks the output and the app exits with the result. Without the environment variable the hook does nothing. CI runs it on Linux, macOS and Windows.
