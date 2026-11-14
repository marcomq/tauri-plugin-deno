import { op_emit, op_listen } from "ext:core/ops";

const listeners = new Map();

globalThis.emit = (event, value) => {
  op_emit(String(event), value ?? null);
};

/**
 * Listens to a Tauri event, e.g. one sent by the frontend with
 * `window.__TAURI__.event.emit(event, payload)`. Returns an unlisten function.
 */
globalThis.listen = (event, handler) => {
  event = String(event);
  let handlers = listeners.get(event);
  if (!handlers) {
    handlers = new Set();
    listeners.set(event, handlers);
    op_listen(event);
  }
  handlers.add(handler);
  return () => handlers.delete(handler);
};

globalThis.__tauriPluginDispatch = (event, payload) => {
  for (const handler of listeners.get(event) ?? []) {
    Promise.resolve()
      .then(() => handler(payload))
      .catch((err) => console.error(`[tauri-plugin-deno] listener for '${event}' failed:`, err));
  }
};

globalThis.addTauri = (fn) => {
  if (typeof globalThis._tauri_plugin_functions == "undefined") {
    globalThis._tauri_plugin_functions = [];
  }
  globalThis._tauri_plugin_functions.push(fn.name);
  globalThis[fn.name] = fn;
};
