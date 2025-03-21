import * as other from "./other.ts" 
_tauri_plugin_functions = [ greetJs.name ]

console.log("Js initialized");
other.printHello("world");

let counter = 0;

function greetJs(input) {
    counter += 1;
    console.log("Call Successful!");
    let s =  (counter > 1) ? "s" : "";
    return "Hello, " + input + "! You've been greeted " + counter + " time" + s + " from Deno/JS!"
}

// console.log(Deno.readTextFileSync("src-deno/main.js")); // print this file to console, only works in debug