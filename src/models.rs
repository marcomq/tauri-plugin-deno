//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use deno_core::serde_json::Value;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, Mutex};

#[derive(Deserialize, Serialize)]
pub enum JsRequest {
    RegisterRequest(RegisterRequest),
    CallFnRequest(CallFnRequest),
    RunCodeRequest(RunCodeRequest),
    ReadVarRequest(ReadVarRequest),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RunCodeRequest {
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReadVarRequest {
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub function_name: String,
    pub number_of_args: Option<u8>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFnRequest {
    pub function_name: String,
    pub args: Vec<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonResponse {
    pub value: Value,
}

pub struct JsMsg {
    pub req: JsRequest,
    pub responder: oneshot::Sender<Value>,
}

pub type UiSender = Mutex<mpsc::Sender<JsMsg>>;
pub type EmitPayload = (String, Value);
pub type DenoEmitSender = mpsc::Sender<EmitPayload>;
