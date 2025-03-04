use deno_core::op2;
use deno_error::JsErrorBox;

#[op2(async)]
#[string]
pub async fn op_read_file(#[string] path: String) -> Result<String, std::io::Error> {
    tokio::fs::read_to_string(path).await
}

#[op2]
#[string]
pub fn op_read_file_sync(#[string] path: String) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}

#[op2(async)]
pub async fn op_write_file(
    #[string] path: String,
    #[string] contents: String,
) -> Result<(), std::io::Error> {
    tokio::fs::write(path, contents).await
}

#[op2(fast)]
pub fn op_write_file_sync(
    #[string] path: String,
    #[string] contents: String,
) -> Result<(), std::io::Error> {
    std::fs::write(path, contents)
}

#[op2(fast)]
pub fn op_remove_file(#[string] path: String) -> Result<(), std::io::Error> {
    std::fs::remove_file(path)
}

#[op2(async)]
#[string]
pub async fn op_fetch(#[string] url: String) -> Result<String, JsErrorBox> {
    reqwest::get(url)
        .await
        .map_err(|e| JsErrorBox::type_error(e.to_string()))?
        .text()
        .await
        .map_err(|e| JsErrorBox::type_error(e.to_string()))
}

#[op2(async)]
pub async fn op_set_timeout(delay: f64) {
    tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
}

pub static RUNTIME_SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/RUNJS_SNAPSHOT.bin"));