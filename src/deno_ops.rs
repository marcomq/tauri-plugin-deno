use crate::models::DenoEmitSender;
use deno_core::{op2, serde_json, OpState};
use std::{cell::RefCell, rc::Rc};

#[op2(async)]
pub async fn op_emit(
    state: Rc<RefCell<OpState>>,
    #[string] event: String,
    #[serde] value: serde_json::Value,
) -> Result<(), std::io::Error> {
    let state = state.borrow();
    let emit_sender = state.borrow::<DenoEmitSender>();
    // println!("Hello {} from an op!", &event);
    emit_sender
        .send((event, value))
        .await
        .expect("cannot send message as emit");
    Ok(())
}
