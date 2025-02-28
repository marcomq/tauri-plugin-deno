//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use crate::models::*;
use crate::Result;
use crate::UiChannel;
use tauri::Manager;
use tauri::{command, AppHandle, Runtime};

macro_rules! send_receive_result {
    ($app:expr, $payload:expr) => {{
        let channel_state = $app.state::<UiChannel>();
        let channel = channel_state.lock().await;
        channel.tx.send($payload).await.unwrap();
        drop(channel);
        let mut channel = channel_state.lock().await;
        Ok(channel.rx.recv().await.unwrap())
    }};
}

#[command]
pub(crate) async fn run_code<R: Runtime>(
    app: AppHandle<R>,
    payload: RunCodeRequest,
) -> Result<StringResponse> {
    send_receive_result!(app, JsRequest::RunCodeRequest(payload))
}
#[command]
pub(crate) async fn register_function<R: Runtime>(
    app: AppHandle<R>,
    payload: RegisterRequest,
) -> Result<StringResponse> {
    send_receive_result!(app, JsRequest::RegisterRequest(payload))
}
#[command]
pub(crate) async fn call_function<R: Runtime>(
    app: AppHandle<R>,
    payload: CallFnRequest,
) -> Result<StringResponse> {
    send_receive_result!(app, JsRequest::CallFnRequest(payload))
}
#[command]
pub(crate) async fn read_variable<R: Runtime>(
    app: AppHandle<R>,
    payload: ReadVarRequest,
) -> Result<StringResponse> {
    send_receive_result!(app, JsRequest::ReadVarRequest(payload))
}
