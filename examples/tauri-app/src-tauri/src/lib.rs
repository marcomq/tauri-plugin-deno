// Learn more about Tauri commands at https://v2.tauri.app/develop/calling-rust/#commands
#[tauri::command]
fn greet_rust(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet_rust])
        .plugin(tauri_plugin_deno::init());
    e2e::attach(builder)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// UI test hook, inactive unless `TAURI_E2E_SCRIPT` is set (see `e2e/run.mjs`): runs the
/// script in the page, writes its "e2e-progress"/"e2e-done" JSON to `TAURI_E2E_REPORT`
/// and exits after "e2e-done" with 0 if it contains `"ok":true`.
mod e2e {
    use tauri::{Builder, Listener, Wry, webview::PageLoadEvent};

    pub fn attach(builder: Builder<Wry>) -> Builder<Wry> {
        let Ok(path) = std::env::var("TAURI_E2E_SCRIPT") else {
            return builder;
        };
        let script = std::fs::read_to_string(&path).expect("cannot read TAURI_E2E_SCRIPT");
        builder
            .on_page_load(move |webview, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    let _ = webview.eval(script.as_str());
                }
            })
            .setup(|app| {
                app.listen_any("e2e-progress", |event| write_report(event.payload()));
                let handle = app.handle().clone();
                app.listen_any("e2e-done", move |event| {
                    let report = event.payload().to_string();
                    write_report(&report);
                    println!("E2E {report}");
                    handle.exit(if report.contains("\"ok\":true") { 0 } else { 1 });
                });
                Ok(())
            })
    }

    fn write_report(report: &str) {
        if let Ok(path) = std::env::var("TAURI_E2E_REPORT") {
            let _ = std::fs::write(path, report);
        }
    }
}
