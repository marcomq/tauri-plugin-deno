//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use crate::models;
use serde::{ser::Serializer, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error: {0}")]
    String(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl From<&str> for Error {
    fn from(error: &str) -> Self {
        Error::String(error.into())
    }
}

impl From<std::sync::mpsc::SendError<models::JsRequest>> for Error {
    fn from(error: std::sync::mpsc::SendError<models::JsRequest>) -> Self {
        Error::String(error.to_string())
    }
}

impl From<std::sync::mpsc::RecvError> for Error {
    fn from(error: std::sync::mpsc::RecvError) -> Self {
        Error::String(error.to_string())
    }
}
