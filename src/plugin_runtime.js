const { core } = Deno;

globalThis.op_hello = (...args) => { 
  op_hello(arts);
};

globalThis.addTauri = (fn) => {
  if (typeof globalThis._tauri_plugin_functions == "undefined") {
    globalThis._tauri_plugin_functions = [];
  }
  globalThis._tauri_plugin_functions.push(fn.name);
  globalThis[fn.name] = fn;
};
