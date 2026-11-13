//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use crate::DenoHandle;
use crate::models::*;
use deno_core::serde_json::Value;
use serde::de::DeserializeOwned;
use tauri::{AppHandle, Manager, Runtime, plugin::PluginApi};

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Deno<R>> {
    Ok(Deno(app.clone()))
}

/// Access to the deno APIs from Rust, e.g. `app.deno().call_function("greetJs", args)`.
pub struct Deno<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Deno<R> {
    async fn request(&self, req: JsRequest) -> crate::Result<Value> {
        self.0.state::<DenoHandle>().request(req).await
    }

    /// Calls a registered (or exported) Deno function.
    pub async fn call_function(
        &self,
        function_name: impl Into<String>,
        args: Vec<Value>,
    ) -> crate::Result<Value> {
        self.request(JsRequest::CallFnRequest(CallFnRequest {
            function_name: function_name.into(),
            args,
        }))
        .await
    }

    /// Makes a global Deno function callable via [`Self::call_function`] and the frontend.
    pub async fn register_function(
        &self,
        function_name: impl Into<String>,
        number_of_args: Option<u8>,
    ) -> crate::Result<Value> {
        self.request(JsRequest::RegisterRequest(RegisterRequest {
            function_name: function_name.into(),
            number_of_args,
        }))
        .await
    }

    /// Runs JavaScript code as a classic script and returns its completion value.
    pub async fn run_code(&self, code: impl Into<String>) -> crate::Result<Value> {
        self.request(JsRequest::RunCodeRequest(RunCodeRequest {
            value: code.into(),
        }))
        .await
    }

    /// Reads a global variable.
    pub async fn read_variable(&self, name: impl Into<String>) -> crate::Result<Value> {
        self.request(JsRequest::ReadVarRequest(ReadVarRequest {
            value: name.into(),
        }))
        .await
    }
}
