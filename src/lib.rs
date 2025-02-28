//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_core::error::CoreError;
use deno_core::error::ModuleLoaderError;
use deno_core::extension;
use deno_core::op2;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::ModuleLoadResponse;
use deno_core::ModuleSourceCode;
use deno_error::JsErrorBox;
pub use models::*;
use std::collections::HashMap;
use std::env;
use std::rc::Rc;
use std::sync::LazyLock;
use std::thread;
use std::vec;
use tokio::sync::broadcast;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};
use futures;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Deno;
#[cfg(mobile)]
use mobile::Deno;

#[op2(async)]
async fn op_set_timeout(delay: f64) {
    tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
}

struct TsModuleLoader;

  static SEND_RECEIVE: LazyLock<(broadcast::Sender<JsRequest>, broadcast::Receiver<JsRequest>)> =
      LazyLock::new(|| broadcast::channel(1000));



static RUNTIME_SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/RUNJS_SNAPSHOT.bin"));
extension!(runjs, ops = [op_set_timeout,]);

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the deno APIs.
pub trait DenoExt<R: Runtime> {
    fn deno(&self) -> &Deno<R>;
    fn run_code(&self, payload: RunCodeRequest) -> crate::Result<StringResponse>;
    fn register_function(&self, payload: RegisterRequest) -> crate::Result<StringResponse>;
    fn call_function(&self, payload: CallFnRequest) -> crate::Result<StringResponse>;
    fn read_variable(&self, payload: ReadVarRequest) -> crate::Result<StringResponse>;
}

macro_rules! send_receive_result {
    ($payload:expr) => {{
        SEND_RECEIVE.0.send($payload)?;
        let mut rx = SEND_RECEIVE.1.resubscribe();
        let msg = futures::executor::block_on(async {rx.recv().await });
        if let Ok(JsRequest::StringResponse(result)) = msg {
            Ok(result)
        } else {
            Err(Error::String("wrong response type".into()))
        }
    }};
}

impl<R: Runtime, T: Manager<R>> crate::DenoExt<R> for T {
    fn deno(&self) -> &Deno<R> {
        self.state::<Deno<R>>().inner()
    }
    fn run_code(&self, payload: RunCodeRequest) -> crate::Result<StringResponse> {
        send_receive_result!(models::JsRequest::RunCodeRequest(payload))
    }
    fn register_function(&self, payload: RegisterRequest) -> crate::Result<StringResponse> {
        send_receive_result!(models::JsRequest::RegisterRequest(payload))
    }
    fn call_function(&self, payload: CallFnRequest) -> crate::Result<StringResponse> {
        send_receive_result!(models::JsRequest::CallFnRequest(payload))
    }
    fn read_variable(&self, payload: ReadVarRequest) -> crate::Result<StringResponse> {
        send_receive_result!(models::JsRequest::ReadVarRequest(payload))
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("deno")
        .invoke_handler(tauri::generate_handler![
            commands::run_code,
            commands::register_function,
            commands::call_function,
            commands::read_variable
        ])
        .setup(|app, api| {
            start_deno_thread();
            #[cfg(mobile)]
            let deno = mobile::init(app, api)?;
            #[cfg(desktop)]
            let deno = desktop::init(app, api)?;
            app.manage(deno);
            Ok(())
        })
        .build()
}

impl deno_core::ModuleLoader for TsModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: deno_core::ResolutionKind,
    ) -> std::result::Result<deno_core::ModuleSpecifier, ModuleLoaderError> {
        deno_core::resolve_import(specifier, referrer).map_err(Into::into)
    }

    fn load(
        &self,
        module_specifier: &deno_core::ModuleSpecifier,
        _maybe_referrer: Option<&reqwest::Url>,
        _is_dyn_import: bool,
        _requested_module_type: deno_core::RequestedModuleType,
    ) -> ModuleLoadResponse {
        let module_specifier = module_specifier.clone();

        let module_load = move || {
            let path = module_specifier.to_file_path().unwrap();

            let media_type = MediaType::from_path(&path);
            let (module_type, should_transpile) = match MediaType::from_path(&path) {
                MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
                    (deno_core::ModuleType::JavaScript, false)
                }
                MediaType::Jsx => (deno_core::ModuleType::JavaScript, true),
                MediaType::TypeScript
                | MediaType::Mts
                | MediaType::Cts
                | MediaType::Dts
                | MediaType::Dmts
                | MediaType::Dcts
                | MediaType::Tsx => (deno_core::ModuleType::JavaScript, true),
                MediaType::Json => (deno_core::ModuleType::Json, false),
                _ => panic!("Unknown extension {:?}", path.extension()),
            };

            let code = std::fs::read_to_string(&path)?;
            let code = if should_transpile {
                let parsed = deno_ast::parse_module(ParseParams {
                    specifier: module_specifier.clone(),
                    text: code.into(),
                    media_type,
                    capture_tokens: false,
                    scope_analysis: false,
                    maybe_syntax: None,
                })
                .map_err(JsErrorBox::from_err)?;
                parsed
                    .transpile(
                        &Default::default(),
                        &Default::default(),
                        &Default::default(),
                    )
                    .map_err(JsErrorBox::from_err)?
                    .into_source()
                    .text
            } else {
                code
            };
            let module = deno_core::ModuleSource::new(
                module_type,
                ModuleSourceCode::String(code.into()),
                &module_specifier,
                None,
            );
            Ok(module)
        };

        ModuleLoadResponse::Sync(module_load())
    }
}

fn get_fn(js_runtime: &mut JsRuntime, fn_name: &str) -> v8::Global<v8::Function> {
    let deno_ctx = js_runtime.main_context();
    let ctx = deno_ctx.open(js_runtime.v8_isolate());
    let mut scope = js_runtime.handle_scope();
    let var_str = v8::String::new(&mut scope, fn_name).unwrap();
    let val = ctx
        .global(&mut scope)
        .get(&mut scope, var_str.into())
        .expect("missing function");
    let v8_val = v8::Local::<v8::Function>::try_from(val).expect("function expected");
    v8::Global::new(&mut scope, v8_val)
}

async fn init_main_js(js_runtime: &mut JsRuntime,) {
    let file_path = "src-js/main.js";
    let code = std::fs::read_to_string(file_path).unwrap_or_default();

    let _res = js_runtime.execute_script(file_path, code.clone()).unwrap();

    js_runtime.run_event_loop(Default::default()).await
        .expect("Error initializing main.js");
}

fn vec_to_v8_vec(my_vec: Vec<JsMany>, js_runtime: &mut JsRuntime) -> Vec<v8::Global<v8::Value>> {
    let mut v8_vec: vec::Vec<v8::Global<v8::Value>> = vec![];
    let mut scope = js_runtime.handle_scope();
    v8_vec.reserve(my_vec.len());
    for val in my_vec {
        let handle = serde_v8::to_v8(&mut scope, val).unwrap();
        let v8_global_val: v8::Global<v8::Value> = v8::Global::<v8::Value>::new(&mut scope, handle);
        v8_vec.push(v8_global_val);
    }
    v8_vec
}

/// Call given javascript function and return result
async fn deno_call_js(
    args: Vec<JsMany>,
    js_runtime: &mut JsRuntime,
    js_fn: &v8::Global<v8::Function>,
) -> std::result::Result<String, CoreError> {
    let v8_args = vec_to_v8_vec(args, js_runtime);
    let v8_result = js_runtime.call_with_args(js_fn, &v8_args).await?;
    Ok(v8_result
        .open(js_runtime.v8_isolate())
        .to_rust_string_lossy(&mut js_runtime.handle_scope()))
}

/// Execute given javascript without any preprocessing
fn deno_exec_js(code: String, js_runtime: &mut JsRuntime) -> std::result::Result<String, CoreError> {
    let my_var = js_runtime.execute_script("()", code).unwrap();
    Ok(my_var
        .open(js_runtime.v8_isolate())
        .to_rust_string_lossy(&mut js_runtime.handle_scope()))
}

/// Executes given javascripts and returns value - used here just to read variable.
/// Replaces some characters to reduce risk of unwanted code execution
fn deno_read_var(variable: &str, js_runtime: &mut JsRuntime) -> std::result::Result<String, CoreError> {
    let variable = variable.replace(&['(', ')', '\"', ';', '\''][..], ""); // replace chars to avoid function call
    let my_var = js_runtime.execute_script("()", variable).unwrap();
    Ok(my_var
        .open(js_runtime.v8_isolate())
        .to_rust_string_lossy(&mut js_runtime.handle_scope()))
}

async fn deno_tokio_loop(mut js_runtime: JsRuntime, mut global_fns: HashMap<String, v8::Global<v8::Function>>) {
  loop {
      let mut rx = SEND_RECEIVE.1.resubscribe();
        if let Ok(received) = rx.recv().await {
            let result = match received {
                JsRequest::RunCodeRequest(req) => deno_exec_js(req.value, &mut js_runtime),
                JsRequest::ReadVarRequest(req) => deno_read_var(&req.value, &mut js_runtime),
                JsRequest::RegisterRequest(req) => {
                    register_fn(req.function_name, &mut js_runtime, &mut global_fns)
                }
                JsRequest::CallFnRequest(req) => deno_call_js(
                    req.args,
                    &mut js_runtime,
                    &global_fns[&req.function_name],
                ).await,
                _ => break,
            };
            let response = match result {
                Ok(val) => val,
                Err(err) => format!("error: {}", err.to_string()),
            };
            let _ignore = SEND_RECEIVE
                .0
                .send(JsRequest::StringResponse(StringResponse {
                    value: response,
                }));
        }
    }
}

fn register_fn(fn_name: String,
    js_runtime: &mut JsRuntime,
    fn_map: &mut HashMap<String, v8::Global<v8::Function>> ) -> std::result::Result<String, CoreError> {
        let my_fn = get_fn(js_runtime, &fn_name);
        fn_map.insert(fn_name, my_fn);
        Ok("Ok".to_string())
    }

fn js_runtime_worker() {
    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
        module_loader: Some(Rc::new(TsModuleLoader)),
        startup_snapshot: Some(RUNTIME_SNAPSHOT),
        extensions: vec![runjs::init_ops()],
        ..Default::default()
    });
    let tokio_runtime: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    tokio_runtime.block_on(init_main_js(&mut js_runtime));

    let mut global_fns: HashMap<String, v8::Global<v8::Function>> = HashMap::new();

    let js_allowed_fns = if let Ok(fn_s) = deno_read_var("_tauri_plugin_functions", &mut js_runtime) {
        fn_s.split(",").map(str::to_string).collect() // array to string conversion doesn't work yet in read_var
    } else {
        vec![]
    };
    for function_name in js_allowed_fns {
        register_fn(function_name.clone(), &mut js_runtime, &mut global_fns)
            .expect(&format!("cannot register function {function_name}"));
    }
    tokio_runtime.block_on(deno_tokio_loop(js_runtime, global_fns));
    
    println!("exit thread");
}

fn start_deno_thread() {
    let _detached = thread::spawn(js_runtime_worker);
}
