_tauri_plugin_functions = ["greet_js"];
console.log("Js initialized");

let myTest = "test ...";
let counter = 0;

function greet_js(input) {
    counter += 1;
    console.log("Call Successfull!");
    let s =  (counter > 1) ? "s" : "";
    return "Hello, " + input + "! You've been greeted " + counter + " time" + s + " from Deno/JS!"
}