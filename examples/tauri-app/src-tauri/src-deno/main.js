import * as other from "./other.ts";

console.log("Js initialized");
other.printHello("world");

let counter = 0;

// Listed in `_tauri_plugin_functions`: callable from the frontend.
function greetJs(input) {
  counter += 1;
  const s = counter > 1 ? "s" : "";
  return `Hello, ${input}! You've been greeted ${counter} time${s} from Deno/JS!`;
}

function failJs() {
  throw new Error("This error was thrown in Deno and rejected the frontend promise");
}

_tauri_plugin_functions = [greetJs.name, failJs.name];

// Exported functions are callable as well, e.g. via `createApi()` in the frontend.
export async function slowSquare(value) {
  await new Promise((resolve) => setTimeout(resolve, 300));
  return value * value;
}

export function readOwnSource() {
  // Full Deno APIs are available; this only works in `tauri dev` (relative path).
  return Deno.readTextFileSync("src-deno/main.js").split("\n").length;
}

// Events in both directions: the frontend emits "ping", Deno answers with "pong".
listen("ping", (payload) => {
  emit("pong", { received: payload, at: new Date().toISOString() });
});
