//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

pub use models::*;
use std::thread;
use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, Runtime,
};
use tokio::sync::{mpsc, Mutex};

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod deno_lib;
mod deno_ops;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Deno;
#[cfg(mobile)]
use mobile::Deno;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the deno APIs.
pub trait DenoExt<R: Runtime> {
    fn deno(&self) -> &Deno<R>;
}

impl<R: Runtime, T: Manager<R>> crate::DenoExt<R> for T {
    fn deno(&self) -> &Deno<R> {
        self.state::<Deno<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("deno")
        .invoke_handler(tauri::generate_handler![
            commands::run_code,
            commands::register_function,
            commands::call_function,
            commands::read_variable
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let deno = mobile::init(app, api)?;
            #[cfg(desktop)]
            let deno = desktop::init(app, api)?;
            app.manage(deno);
            let (tx, rx) = mpsc::channel(1000);
            app.manage(Mutex::new(tx));
            start_deno_thread(rx);
            start_emit_handler_thread(app.clone());
            Ok(())
        })
        .build()
}

fn start_deno_thread(rx: mpsc::Receiver<JsMsg>) {
    let _detached = thread::spawn(move || deno_lib::js_runtime_thread(rx));
}

fn start_emit_handler_thread<R: Runtime>(app: AppHandle<R>) {
    let _detached = thread::spawn(move || emit_handler_thread(app));
}

fn emit_handler_thread<R: Runtime>(app: AppHandle<R>) {
    let _lazy_init: DenoEmitSender = deno_lib::DENO_EMIT_SENDER.clone();
    loop {
        deno_lib::handle_emit(&app);
    }
}
