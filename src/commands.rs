//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::DenoExt;
use crate::Result;

#[command]
pub(crate) async fn run_code<R: Runtime>(
    app: AppHandle<R>,
    payload: RunCodeRequest,
) -> Result<StringResponse> {
    app.run_code(payload)
}
#[command]
pub(crate) async fn register_function<R: Runtime>(
    app: AppHandle<R>,
    payload: RegisterRequest,
) -> Result<StringResponse> {
    app.register_function(payload)
}
#[command]
pub(crate) async fn call_function<R: Runtime>(
    app: AppHandle<R>,
    payload: CallFnRequest,
) -> Result<StringResponse> {
    app.call_function(payload)
}
#[command]
pub(crate) async fn read_variable<R: Runtime>(
    app: AppHandle<R>,
    payload: ReadVarRequest,
) -> Result<StringResponse> {
    app.read_variable(payload)
}