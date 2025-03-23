import { op_emit } from "ext:core/ops";

globalThis.emit = (event, value) => { 
  op_emit(event, value);
};

globalThis.addTauri = (fn) => {
  if (typeof globalThis._tauri_plugin_functions == "undefined") {
    globalThis._tauri_plugin_functions = [];
  }
  globalThis._tauri_plugin_functions.push(fn.name);
  globalThis[fn.name] = fn;
};
