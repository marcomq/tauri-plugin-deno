//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_core::error::CoreError;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::JsRuntime;
pub use models::*;
use std::collections::HashMap;
use std::rc::Rc;
use std::thread;
use std::vec;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};
use tokio::sync::mpsc;
use tokio::sync::Mutex;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod deno_ops;
mod error;
mod models;
use deno_ops::*;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Deno;
#[cfg(mobile)]
use mobile::Deno;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the deno APIs.
pub trait DenoExt<R: Runtime> {
    fn deno(&self) -> &Deno<R>;
}

impl<R: Runtime, T: Manager<R>> crate::DenoExt<R> for T {
    fn deno(&self) -> &Deno<R> {
        self.state::<Deno<R>>().inner()
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
            #[cfg(mobile)]
            let deno = mobile::init(app, api)?;
            #[cfg(desktop)]
            let deno = desktop::init(app, api)?;
            app.manage(deno);
            let (tx, rx) = mpsc::channel(1000);
            app.manage(Mutex::new(tx));
            start_deno_thread(rx);
            Ok(())
        })
        .build()
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
fn deno_exec_js(
    code: String,
    js_runtime: &mut JsRuntime,
) -> std::result::Result<String, CoreError> {
    let my_var = js_runtime.execute_script("ext:<anon>", code).unwrap();
    Ok(my_var
        .open(js_runtime.v8_isolate())
        .to_rust_string_lossy(&mut js_runtime.handle_scope()))
}

/// Executes given javascripts and returns value - used here just to read variable.
/// Replaces some characters to reduce risk of unwanted code execution
fn deno_read_var(
    variable: &str,
    js_runtime: &mut JsRuntime,
) -> std::result::Result<String, CoreError> {
    let variable = variable.replace(&['(', ')', '\"', ';', '\'', '=', ':'][..], ""); // replace chars to avoid function call
    let my_var = js_runtime.execute_script("()", variable).unwrap();
    Ok(my_var
        .open(js_runtime.v8_isolate())
        .to_rust_string_lossy(&mut js_runtime.handle_scope()))
}

async fn deno_tokio_loop(
    mut js_runtime: JsRuntime,
    mut global_fns: HashMap<String, v8::Global<v8::Function>>,
    mut rx: mpsc::Receiver<JsMsg>,
) {
    loop {
        let msg = rx.recv().await;
        if let Some(received) = msg {
            let result = match received.req {
                JsRequest::RunCodeRequest(req) => deno_exec_js(req.value, &mut js_runtime),
                JsRequest::ReadVarRequest(req) => deno_read_var(&req.value, &mut js_runtime),
                JsRequest::RegisterRequest(req) => {
                    register_fn(req.function_name, &mut js_runtime, &mut global_fns)
                }
                JsRequest::CallFnRequest(req) => {
                    deno_call_js(req.args, &mut js_runtime, &global_fns[&req.function_name]).await
                }
            };
            let response = match result {
                Ok(val) => val,
                Err(err) => format!("error: {}", err.to_string()),
            };

            let _ignore = received.responder.send(response);
        }
    }
}

fn register_fn(
    fn_name: String,
    js_runtime: &mut JsRuntime,
    fn_map: &mut HashMap<String, v8::Global<v8::Function>>,
) -> std::result::Result<String, CoreError> {
    let my_fn = get_fn(js_runtime, &fn_name);
    fn_map.insert(fn_name, my_fn);
    Ok("Ok".to_string())
}

deno_core::extension!(
    runjs,
    ops = [
        op_set_timeout,
        op_read_file,
        op_write_file,
        op_read_file_sync,
        op_write_file_sync,
        op_fetch,
        op_remove_file,
    ]
);


async fn init_main_js(js_runtime: &mut JsRuntime) {
    let file_path = "src-js/main.js";
    let code = std::fs::read_to_string(file_path).unwrap_or_default();
    let parsed = deno_ast::parse_module(ParseParams {
        specifier: deno_core::ModuleSpecifier::parse(&format!("file:///{file_path}")).unwrap(),
        text: code.into(),
        media_type: MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    })
    .unwrap();
    let transpiled_code = parsed
        .transpile(
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
        .unwrap()
        .into_source()
        .text;

    let _res = js_runtime
        .execute_script(file_path, transpiled_code)
        .unwrap();

    js_runtime
        .run_event_loop(Default::default())
        .await
        .expect("Error initializing main.js");
}

fn js_runtime_worker(rx: mpsc::Receiver<JsMsg>) {
    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
        module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
        extensions: vec![runjs::init_ops()],
        startup_snapshot: Some(RUNTIME_SNAPSHOT),
        ..Default::default()
    });
    let tokio_runtime: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    tokio_runtime.block_on(init_main_js(&mut js_runtime));

    let mut global_fns: HashMap<String, v8::Global<v8::Function>> = HashMap::new();

    let js_allowed_fns = if let Ok(fn_s) = deno_read_var("_tauri_plugin_functions", &mut js_runtime)
    {
        fn_s.split(",").map(str::to_string).collect() // array to string conversion doesn't work yet in read_var
    } else {
        vec![]
    };
    for function_name in js_allowed_fns {
        register_fn(function_name.clone(), &mut js_runtime, &mut global_fns)
            .expect(&format!("cannot register function {function_name}"));
    }
    tokio_runtime.block_on(deno_tokio_loop(js_runtime, global_fns, rx));

    println!("exit thread");
}

fn start_deno_thread(rx: mpsc::Receiver<JsMsg>) {
    let _detached = thread::spawn(move || js_runtime_worker(rx));
}
