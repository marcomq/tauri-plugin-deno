const tauri = window.__TAURI__;

let inputField;
let outputEl;

function show(text) {
  outputEl.textContent = text;
}

async function greetRust() {
  show(await tauri.core.invoke("greet_rust", { name: inputField.value }));
}

async function greetJs() {
  show(await tauri.deno.call.greetJs(inputField.value));
  // Alternatively:
  // show(await tauri.deno.callFunction("greetJs", [inputField.value]));
}

async function failJs() {
  try {
    await tauri.deno.call.failJs();
  } catch (error) {
    show(`Caught: ${error}`);
  }
}

async function square() {
  const api = tauri.deno.createApi();
  const text = inputField.value.trim();
  const value = text !== "" && Number.isFinite(Number(text)) ? Number(text) : 7;
  show(`${value}² = ${await api.slowSquare(value)} (computed in Deno)`);
}

window.addEventListener("DOMContentLoaded", async () => {
  tauri.deno.registerJs("greetJs"); // makes "tauri.deno.call.greetJs" available
  tauri.deno.registerJs("failJs");
  inputField = document.querySelector("#input-field");
  outputEl = document.querySelector("#output-element");

  await tauri.event.listen("pong", (event) => show(`Deno answered: ${JSON.stringify(event.payload)}`));

  const actions = {
    submit_rust: greetRust,
    submit_deno: greetJs,
    submit_fail: failJs,
    submit_square: square,
    submit_ping: () => tauri.event.emit("ping", inputField.value),
  };
  for (const form of document.querySelectorAll("form")) {
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      Promise.resolve(actions[e.submitter.value]?.()).catch((error) => show(`Error: ${error}`));
    });
  }
  window.appReady = true; // read by the e2e test (e2e/clicks.js)
  window.dispatchEvent(new Event("app-ready"));
});
