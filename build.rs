const COMMANDS: &[&str] = &[
    "run_code",
    "register_function",
    "call_function",
    "read_variable",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./dist-js/index.iife.js")
        .android_path("android")
        .ios_path("ios")
        .build();
}

