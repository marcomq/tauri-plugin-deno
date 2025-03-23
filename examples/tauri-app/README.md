# Deno example

This example uses vanilla javascript.
But you may use just any javascript framework you want. It may be recommended to use some of those that are already included in tauri.

The main modification was in `package.json` to add a `pretauri` scripts step to transpile Deno JS into a single file in `target/deno_dist.js`.

```bash
"deno run --allow-read --allow-write --allow-env --allow-net --allow-run npm:tauri-plugin-deno-api src-tauri/src-deno/main.js&",
```

This will also listen to changes and re-transpile the file.

Also, `src-tauri/src-deno` was added that contain the Deno backend files.

Steps that are expected to be added after `tauri add deno`:
- In tauri rust code, `.plugin(tauri_plugin_deno::init()` was added to `src-tauri/src/lib.rs`, 
- In package.json, `tauri-plugin-deno-api` will be added to dependencies.