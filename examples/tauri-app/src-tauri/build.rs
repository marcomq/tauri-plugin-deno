fn main() {
    // Exports the N-API symbols, so the Deno code can load native Node addons (.node).
    if let Ok(arg) = std::env::var("DEP_TAURI_PLUGIN_DENO_NAPI_LINK_ARG") {
        println!("cargo:rustc-link-arg-bins={arg}");
    }
    tauri_build::build()
}
