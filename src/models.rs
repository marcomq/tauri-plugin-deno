//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};

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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum JsMany {
    Bool(bool),
    Number(u64),
    Float(f64),
    String(String),
    StringVec(Vec<String>),
    FloatVec(Vec<f64>),
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
    pub args: Vec<JsMany>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StringResponse {
    pub value: String,
}

pub struct SendReceive<T, R> {
    pub tx: mpsc::Sender<T>,
    pub rx: mpsc::Receiver<R>,
}

pub type UiChannel = Mutex<SendReceive<JsRequest, StringResponse>>;
pub type DenoChannel = Mutex<SendReceive<StringResponse, JsRequest>>;

pub struct Channels {
    pub ui: UiChannel,     // to deno, from deno
    pub deno: DenoChannel, // to ui, from ui,
}

impl Channels {
    pub fn new() -> Channels {
        let (tx_to_deno, rx_from_ui) = mpsc::channel(1000);
        let (tx_to_ui, rx_from_deno) = mpsc::channel(1000);
        Channels {
            ui: Mutex::new(SendReceive {
                tx: tx_to_deno,
                rx: rx_from_deno,
            }),
            deno: Mutex::new(SendReceive {
                tx: tx_to_ui,
                rx: rx_from_ui,
            }),
        }
    }
}