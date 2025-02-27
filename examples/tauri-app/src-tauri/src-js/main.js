_tauri_plugin_functions = ["greet_js"];
console.log("Js initialized");

let myTest = "test ...";
let counter = 0;

function greet_js(input) {
    counter += 1;
    console.log("Call Successfull!");
    return input + " from js (" + counter + ")";
}