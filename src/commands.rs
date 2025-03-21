//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use crate::models::*;
use crate::Result;
use crate::UiSender;
use tauri::Manager;
use tauri::{command, AppHandle, Runtime};
use tokio::sync::oneshot;

macro_rules! send_receive_result {
    ($app:expr, $payload:expr) => {{
        let (responder, receiver) = oneshot::channel::<JsMany>();
        let sender_state = $app.state::<UiSender>();
        let locked_tx = sender_state.lock().await;
        locked_tx
            .send(JsMsg {
                req: $payload,
                responder,
            })
            .await
            .unwrap();
        drop(locked_tx);
        Ok(JsManyResponse {
            value: receiver.await.unwrap_or_default(),
        })
    }};
}

#[command]
pub(crate) async fn run_code<R: Runtime>(
    app: AppHandle<R>,
    payload: RunCodeRequest,
) -> Result<JsManyResponse> {
    send_receive_result!(app, JsRequest::RunCodeRequest(payload))
}
#[command]
pub(crate) async fn register_function<R: Runtime>(
    app: AppHandle<R>,
    payload: RegisterRequest,
) -> Result<JsManyResponse> {
    send_receive_result!(app, JsRequest::RegisterRequest(payload))
}
#[command]
pub(crate) async fn call_function<R: Runtime>(
    app: AppHandle<R>,
    payload: CallFnRequest,
) -> Result<JsManyResponse> {
    send_receive_result!(app, JsRequest::CallFnRequest(payload))
}
#[command]
pub(crate) async fn read_variable<R: Runtime>(
    app: AppHandle<R>,
    payload: ReadVarRequest,
) -> Result<JsManyResponse> {
    send_receive_result!(app, JsRequest::ReadVarRequest(payload))
}
