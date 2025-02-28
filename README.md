# Tauri Plugin deno

This [tauri](https://v2.tauri.app/) v2 plugin is supposed to make it easy to use Javascript as backend code.
It uses Deno as runtime. 

A single deno thread is started in the background and uses tokio to call javascript functions asynchronously. Channels are used to exchange data between tauri and deno runtime. By default, the file `src-tauri/src-js/main.js` is executed on startup.

## Status

This plugin is in an early state. It might still have some issues. I currently only tested it on MacOS and did not optimize it yet for release builds.


## Usage

- run `npm run tauri add deno`
- add `src-tauri/src-js/main.js` and modify it according to your needs, for example add 

```javascript
# src-tauri/src-js/main.js
_tauri_plugin_functions = ["greet_js"] # make "greet_js" callable from UI
def greet_js(rust_var)
    return str(rust_var) + " from javascript"
```

- add `"bundle": {"resources": [  "src-js/**/*"],` to `tauri.conf.json` so that javascript files are bundled with your application
- add the plugin in your client-side javascript: 
   - add `import { callFunction } from 'tauri-plugin-deno-api'` 
   - add `outputEl.textContent = await callFunction("greet_js", [value])` to get the output of the backend javascript function `greet_js` with parameter of js variable `value`


## Security considerations
This plugin has been created with security in mind.
No network server or client is started by this plugin.
By default, this plugin cannot call arbitrary javascript backend code. Backend functions can only be called if registered, for example by using the `_tauri_plugin_functions` variable in the backend javascript code.

There are following additional permissions available that can be added to by using the permissions file `tauri-app/src-tauri/capabilities/default.json`:

- allow-call-function (allows `callFunction`; calls function in backend; enabled by default)
- allow-read-variable (allows `readVariable`; read variable from backend)
- allow-register-function (allows `registerFunction`; client side way of allowing additional functions)
- allow-run-code  (allows `runCode`; call code directly in backend)