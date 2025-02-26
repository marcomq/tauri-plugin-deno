//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum JsRequest {
    RegisterRequest(RegisterRequest),
    CallFnRequest(CallFnRequest),
    RunCodeRequest(RunCodeRequest),
    ReadVarRequest(ReadVarRequest),
    StringResponse(StringResponse),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCodeRequest {
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadVarRequest {
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsMany {
    Bool(bool),
    Number(u64),
    Float(f64),
    String(String),
    StringVec(Vec<String>),
    FloatVec(Vec<f64>),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub function_name: String,
    pub number_of_args: Option<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFnRequest {
    pub function_name: String,
    pub args: Vec<JsMany>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StringResponse {
    pub value: String,
}
