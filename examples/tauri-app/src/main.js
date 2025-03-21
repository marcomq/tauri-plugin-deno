const tauri = window.__TAURI__

let inputField;
let outputEl;

async function greetRust() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  outputEl.textContent = await tauri.core.invoke("greet_rust", { name: inputField.value });
}
async function greetJs() {
  outputEl.textContent = await tauri.deno.call.greetJs(inputField.value + 2);
  // Alternatively:
  // outputEl.textContent = await tauri.deno.callFunction("greet_deno", [inputField.value])
}

window.addEventListener("DOMContentLoaded", () => {
  tauri.deno.registerJs("greetJs"); // Optional, makes it possible to use "tauri.deno.call.greetJs"
  inputField = document.querySelector("#input-field");
  outputEl = document.querySelector("#output-element");
  document.querySelector("#callback-form").addEventListener("submit", (e) => {
    e.preventDefault();
    switch (e.submitter.value) {
      case "submit_rust":
        greetRust();
        break;
      case "submit_deno":
        greetJs();
        break;
    }
  });
});
