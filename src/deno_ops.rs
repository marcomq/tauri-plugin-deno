use deno_core::op2;

#[op2(fast)]
pub fn op_hello(#[string] text: &str) {
    println!("Hello {} from an op!", text);
}
