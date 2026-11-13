//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use crate::LOG_PREFIX;
use crate::models::{Outgoing, OutgoingSender};
use deno_core::{OpState, op2, serde_json};

fn send(state: &OpState, msg: Outgoing) {
    if let Err(err) = state.borrow::<OutgoingSender>().send(msg) {
        eprintln!(
            "{LOG_PREFIX} cannot forward {:?}: Tauri side is gone",
            err.0
        );
    }
}

#[op2]
pub fn op_emit(state: &mut OpState, #[string] event: String, #[serde] value: serde_json::Value) {
    send(state, Outgoing::Emit(event, value));
}

#[op2(fast)]
pub fn op_listen(state: &mut OpState, #[string] event: String) {
    send(state, Outgoing::Listen(event));
}
