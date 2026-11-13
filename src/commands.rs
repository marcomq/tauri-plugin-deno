//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use crate::models::*;
use crate::{DenoHandle, Result};
use tauri::{State, command};

async fn respond(deno: &DenoHandle, req: JsRequest) -> Result<JsonResponse> {
    let value = deno.request(req).await?;
    Ok(JsonResponse { value })
}

#[command]
pub(crate) async fn run_code(
    deno: State<'_, DenoHandle>,
    payload: RunCodeRequest,
) -> Result<JsonResponse> {
    respond(&deno, JsRequest::RunCodeRequest(payload)).await
}

#[command]
pub(crate) async fn register_function(
    deno: State<'_, DenoHandle>,
    payload: RegisterRequest,
) -> Result<JsonResponse> {
    respond(&deno, JsRequest::RegisterRequest(payload)).await
}

#[command]
pub(crate) async fn call_function(
    deno: State<'_, DenoHandle>,
    payload: CallFnRequest,
) -> Result<JsonResponse> {
    respond(&deno, JsRequest::CallFnRequest(payload)).await
}

#[command]
pub(crate) async fn read_variable(
    deno: State<'_, DenoHandle>,
    payload: ReadVarRequest,
) -> Result<JsonResponse> {
    respond(&deno, JsRequest::ReadVarRequest(payload)).await
}
