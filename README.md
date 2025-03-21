# Tauri Plugin deno

This [tauri](https://v2.tauri.app/) v2 plugin is supposed to make it easy to use Javascript as backend code.

It uses Deno as runtime. 

A single thread is started in the background, using the rust Deno runtime and uses tokio to call javascript functions asynchronously. Channels are used to exchange data between tauri UI and Deno runtime. 

It unfortunately is complicated in Deno to import external modules when not running from command line interface. Therefore, all the modules need to be transpiled into one javascript file `src-tauri/target/deno_dist.js` during compilation. This file is then used by the plugin. In the example project, this transpile step happens automatically during compilation. It is defined in the `package.json` file as `pretauri` step, using the binary of `tauri-plugin-deno-api`. This binary always takes the entrypoint as parameter and always transpiles the result to `src-tauri/target/deno_dist.js`. Nearly all Deno runtime libraries are available. The entrypoint js file will always have full permissions when used in tauri - there is no restriction in reading or writing files as in Deno yet. 

## Status

This plugin has not been tested in production yet, but you may already use it. It might still have some major issues. I currently only tested it on MacOS and did not optimize it yet for release builds.
There might be small changes later that may also break code.

Current TODO list:
- make sure that windows & linux production binaries are working fine
- try to also call fronted functions from backend
- check if Android and iOS support can be added easily
- try to use tauri permissions for deno ops / functions
- implement tests & github workflows

## Usage

- run `npm run tauri add deno`
- add `src-tauri/src-deno/main.js` and modify it according to your needs, for example add 

```javascript
// src-tauri/src-deno/main.js
function greetJs(input) {
    return str(input) + " from main.js"
}
_tauri_plugin_functions = [ greetJs.name ] // This will make the function "greetJs" callable from UI
```

- add the plugin in your client-side javascript: 
- add `import { callFunction } from 'tauri-plugin-deno-api'`
- add `window.document.body.innerText = await callFunction("greetJs", "hello world")` to get the output of the backend javascript function `greetJs` with parameter `hello world`
- alternatively use `window.document.body.innerText = window.__TAURI__.deno.callFunction("greetJs", ["hello world"])` directly, without import, if you want to use old style javascript

Input parameters are not limited to strings, you can also use numbers or arrays. The return value currently always needs to be a string.


## Security considerations
This plugin has been created with security in mind.
No network server or client is started by this plugin.
By default, this plugin cannot call any javascript backend function, it just runs `deno_dist.js` automatically on startup. Backend functions can only be called if registered, for example by using the `_tauri_plugin_functions=["fn"]` variable in the backend javascript code. This will make the backend function `fn` available in to UI.

There are following additional permissions available that can be added to by using the permissions file `tauri-app/src-tauri/capabilities/default.json`:

- allow-call-function (allows `callFunction`; calls function in backend; enabled by default)
- allow-read-variable (allows `readVariable`; read js variable from backend)
- allow-register-function (allows `registerFunction`; client side way of allowing additional functions)
- allow-run-code  (allows `runCode`; call code directly in backend)