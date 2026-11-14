//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use deno_core::serde_json::{self, Value};
pub use models::*;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, Listener, Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};
use tokio::sync::{mpsc, oneshot};

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod deno_lib;
mod deno_ops;
mod error;
mod models;
mod node_services;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod unit_tests;

pub use error::{Error, Result};

#[cfg(desktop)]
pub use desktop::Deno;
#[cfg(mobile)]
pub use mobile::Deno;

pub(crate) const LOG_PREFIX: &str = "[tauri-plugin-deno]";
const TRACEBACK_ENV: &str = "TAURI_PLUGIN_DENO_TRACEBACK";
const TIMEOUT_ENV: &str = "TAURI_PLUGIN_DENO_TIMEOUT_SECS";

/// Default per-call timeout, so a hanging JS call can't block the frontend forever.
/// Override with `TAURI_PLUGIN_DENO_TIMEOUT_SECS` (`0` disables it).
const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Deno's emits that its own listener hasn't seen yet; each entry is skipped once. Shared
/// across threads, since Tauri queues an emit while its listeners are locked and another
/// thread may dispatch it later. Queued emits only keep name and payload to match on.
type OwnEmits = Arc<Mutex<Vec<(String, String)>>>;

/// Bundle created by the bundler (`tauri-plugin-deno-api bundle`), located by `build.rs`.
static TAURI_PLUGIN_DENO_DIST: &str = include_str!(env!("TAURI_PLUGIN_DENO_DIST_PATH"));

/// Stack traces and stderr logging are on in debug builds, and in release builds
/// if `TAURI_PLUGIN_DENO_TRACEBACK` is `1` or `true`.
fn tracebacks_enabled(env_value: Option<&str>) -> bool {
    cfg!(debug_assertions)
        || env_value.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true"))
}

/// Resolves the per-call timeout from the raw env var value; `None` disables it.
fn call_timeout(env_value: Option<&str>) -> Option<Duration> {
    match env_value.and_then(|v| v.trim().parse::<u64>().ok()) {
        Some(0) => None,
        Some(secs) => Some(Duration::from_secs(secs)),
        None => Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
    }
}

/// Tauri restricts event names to alphanumeric characters, `-`, `/`, `:` and `_`.
fn is_valid_event_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '/' | ':' | '_'))
}

/// Debug builds prefer the bundle on disk, so `tauri dev` picks up a fresh bundle
/// on restart even if cargo didn't rebuild the plugin yet.
fn dist_code() -> String {
    #[cfg(debug_assertions)]
    if let Ok(code) = std::fs::read_to_string(env!("TAURI_PLUGIN_DENO_DIST_PATH")) {
        return code;
    }
    TAURI_PLUGIN_DENO_DIST.to_string()
}

/// Sends requests to the Deno thread. Managed as Tauri state.
pub(crate) struct DenoHandle {
    requests: mpsc::Sender<JsMsg>,
    timeout: Option<Duration>,
}

impl DenoHandle {
    /// Starts the Deno thread with `code`. Messages from Deno arrive on the returned receiver,
    /// Tauri events for Deno are sent with the returned sender. External packages are
    /// resolved from `node_modules` in `base_dir` or above.
    pub(crate) fn spawn(
        code: String,
        base_dir: std::path::PathBuf,
        timeout: Option<Duration>,
        tracebacks: bool,
    ) -> (Self, mpsc::UnboundedReceiver<Outgoing>, IncomingSender) {
        let (requests, requests_rx) = mpsc::channel(1000);
        let (outgoing, outgoing_rx) = mpsc::unbounded_channel();
        let (incoming, incoming_rx) = mpsc::unbounded_channel();
        let spawned = std::thread::Builder::new()
            .name("tauri-plugin-deno".into())
            .spawn(move || {
                deno_lib::run(
                    code,
                    base_dir,
                    requests_rx,
                    incoming_rx,
                    outgoing,
                    tracebacks,
                )
            });
        if let Err(err) = spawned {
            eprintln!("{LOG_PREFIX} cannot start the Deno thread: {err}");
        }
        (Self { requests, timeout }, outgoing_rx, incoming)
    }

    pub(crate) async fn request(&self, req: JsRequest) -> Result<Value> {
        let (responder, receiver) = oneshot::channel();
        // The timeout covers waiting for queue capacity as well as the response.
        let response = async {
            self.requests
                .send(JsMsg { req, responder })
                .await
                .map_err(|_| Error::NotRunning)?;
            receiver.await.map_err(|_| Error::NotRunning)
        };
        let response = match self.timeout {
            Some(timeout) => tokio::time::timeout(timeout, response)
                .await
                .map_err(|_| Error::Timeout(timeout.as_secs()))?,
            None => response.await,
        };
        response?.map_err(Error::Js)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn start_bridge<R: Runtime>(
    app: AppHandle<R>,
    mut outgoing: mpsc::UnboundedReceiver<Outgoing>,
    incoming: IncomingSender,
) {
    let own_emits = OwnEmits::default();
    let mut listened = HashSet::new();
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = outgoing.recv().await {
            match msg {
                Outgoing::Emit(name, value) => {
                    if listened.contains(&name) {
                        let payload = serde_json::to_string(&value).unwrap_or_default();
                        lock(&own_emits).push((name.clone(), payload));
                    }
                    if let Err(err) = app.emit(&name, value) {
                        eprintln!("{LOG_PREFIX} cannot emit '{name}': {err}");
                    }
                }
                Outgoing::Listen(name) if !is_valid_event_name(&name) => {
                    eprintln!(
                        "{LOG_PREFIX} cannot listen to '{name}': only alphanumeric characters, '-', '/', ':' and '_' are allowed"
                    );
                }
                Outgoing::Listen(name) => {
                    listened.insert(name.clone());
                    let incoming = incoming.clone();
                    let own_emits = own_emits.clone();
                    app.listen_any(name.clone(), move |event| {
                        let mut own = lock(&own_emits);
                        if let Some(i) = own
                            .iter()
                            .position(|(n, payload)| *n == name && payload == event.payload())
                        {
                            own.remove(i);
                            return;
                        }
                        drop(own);
                        let payload = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
                        let _ = incoming.send(IncomingEvent {
                            name: name.clone(),
                            payload,
                        });
                    });
                }
            }
        }
    });
}

fn plugin<R: Runtime>(code: fn() -> String) -> TauriPlugin<R> {
    Builder::new("deno")
        .invoke_handler(tauri::generate_handler![
            commands::run_code,
            commands::register_function,
            commands::call_function,
            commands::read_variable
        ])
        .setup(move |app, api| {
            #[cfg(mobile)]
            let deno = mobile::init(app, api)?;
            #[cfg(desktop)]
            let deno = desktop::init(app, api)?;
            app.manage(deno);
            let base_dir = app
                .path()
                .resource_dir()
                .or_else(|_| std::env::current_dir())
                .unwrap_or_default();
            let (handle, outgoing, incoming) = DenoHandle::spawn(
                code(),
                base_dir,
                call_timeout(std::env::var(TIMEOUT_ENV).ok().as_deref()),
                tracebacks_enabled(std::env::var(TRACEBACK_ENV).ok().as_deref()),
            );
            app.manage(handle);
            start_bridge(app.clone(), outgoing, incoming);
            Ok(())
        })
        .build()
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    plugin(dist_code)
}

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the deno APIs.
pub trait DenoExt<R: Runtime> {
    fn deno(&self) -> &Deno<R>;
}

impl<R: Runtime, T: Manager<R>> crate::DenoExt<R> for T {
    fn deno(&self) -> &Deno<R> {
        self.state::<Deno<R>>().inner()
    }
}
