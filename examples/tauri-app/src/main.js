const tauri = window.__TAURI__

let inputField;
let outputEl;

async function greet_rust() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  outputEl.textContent = await tauri.core.invoke("greet_rust", { name: inputField.value });
}
async function greet_js() {
  outputEl.textContent = await tauri.deno.call.greet_js(inputField.value);
  // Alternatively:
  // outputEl.textContent = await tauri.deno.callFunction("greet_deno", [inputField.value])
}

window.addEventListener("DOMContentLoaded", () => {
  tauri.deno.registerJs("greet_js"); // Optional, makes it possible to use "tauri.deno.call.greet_js"
  inputField = document.querySelector("#input-field");
  outputEl = document.querySelector("#output-element");
  document.querySelector("#callback-form").addEventListener("submit", (e) => {
    e.preventDefault();
    switch (e.submitter.value) {
      case "submit_rust":
        greet_rust();
        break;
      case "submit_deno":
        greet_js();
        break;
    }
  });
});
