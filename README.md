# Tauri Plugin deno

This [tauri](https://v2.tauri.app/) v2 plugin is supposed to make it easy to use Javascript as backend code.
It uses Deno as runtime. 

A single deno thread is started in the background and uses tokio to call javascript functions asynchronously. Channels are used to exchange data between tauri and deno runtime. By default, the file `src-tauri/src-deno/main.ts` is executed on startup. If you want to use typescript, please use some packager to transpile
your typescript into main.js first - for example by using `rollup`.

## Status

This plugin is not yet in production state. It might still have some issues. I currently only tested it on MacOS and did not optimize it yet for release builds.
There might be changes later, for example to auto include backend javascript.

Planned changes:
- try to read dependencies from deno.json / package.json
- make sure that windows & linux production binaries are working fine
- try to compile backend js into production binary without additional resource
- try to use tauri permissions for deno ops / functions
- try to also call fronted functions from backend
- check if Android and iOS support can be added easily
- implement tests & github workflows

## Usage

- run `npm run tauri add deno`
- add `src-tauri/src-deno/main.js` and modify it according to your needs, for example add 

```javascript
# src-tauri/src-deno/main.ts
function greetJs(input) {
    return str(input) + " from main.js"
}
addTauri(greetJs); // This will make the function "greetJs" callable from UI
```

- add `"bundle": {"resources": [  "src-deno/**/*"],` to `tauri.conf.json` so that javascript files are bundled with your application
- add the plugin in your client-side javascript: 
   - add `import { callFunction } from 'tauri-plugin-deno-api'`
   - add `window.document.body.innerText = await callFunction("greetJs", [value])` to get the output of the backend javascript function `greetJs` with parameter of js variable `value`
   - alternatively use `window.document.body.innerText = window.__TAURI__.deno.callFunction("greetJs", [value.value])` directly, if you want to use old style javascript without using imports

This is just an example of how to modify client side content. You may just set some value or a local variable.


## Security considerations
This plugin has been created with security in mind.
No network server or client is started by this plugin.
By default, this plugin cannot call javascript backend code. Backend functions can only be called if registered, for example by using the `addTauri(fn)` function in the backend javascript code. This will make the backend function `fn` available in to UI.

There are following additional permissions available that can be added to by using the permissions file `tauri-app/src-tauri/capabilities/default.json`:

- allow-call-function (allows `callFunction`; calls function in backend; enabled by default)
- allow-read-variable (allows `readVariable`; read variable from backend)
- allow-register-function (allows `registerFunction`; client side way of allowing additional functions)
- allow-run-code  (allows `runCode`; call code directly in backend)