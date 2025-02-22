use tauri::{
  plugin::{Builder, TauriPlugin},
  Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
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
    .invoke_handler(tauri::generate_handler![commands::ping])
    .setup(|app, api| {
      #[cfg(mobile)]
      let deno = mobile::init(app, api)?;
      #[cfg(desktop)]
      let deno = desktop::init(app, api)?;
      app.manage(deno);
      Ok(())
    })
    .build()
}
