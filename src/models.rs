//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use deno_core::serde_json::Value;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFnRequest {
    pub function_name: String,
    #[serde(default)]
    pub args: Vec<Value>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonResponse {
    pub value: Value,
}

pub(crate) struct JsMsg {
    pub req: JsRequest,
    pub responder: oneshot::Sender<Result<Value, String>>,
}

/// Messages from the Deno runtime to the Tauri side.
#[derive(Debug)]
pub(crate) enum Outgoing {
    Emit(String, Value),
    Listen(String),
}

/// A Tauri event forwarded into the Deno runtime.
#[derive(Debug)]
pub(crate) struct IncomingEvent {
    pub name: String,
    pub payload: Value,
}

pub(crate) type OutgoingSender = mpsc::UnboundedSender<Outgoing>;
pub(crate) type IncomingSender = mpsc::UnboundedSender<IncomingEvent>;
