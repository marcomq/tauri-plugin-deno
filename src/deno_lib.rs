//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use crate::deno_ops::*;
use crate::models::*;
use deno_core::error::CoreError;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::FsModuleLoader;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_fs::RealFs;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::worker::WorkerServiceOptions;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::vec;
use tokio::sync::mpsc;

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

/// Executes given javascript and returns value - used here just to read variable.
/// Replaces some characters to reduce risk of unwanted code execution
fn deno_read_var(
    variable: &str,
    js_runtime: &mut JsRuntime,
) -> std::result::Result<String, CoreError> {
    let variable = variable.replace(&['(', ')', '\"', ';', '\'', '=', ':'][..], ""); // replace chars to avoid function call
    let my_var = js_runtime.execute_script("()", variable)?;
    Ok(my_var
        .open(js_runtime.v8_isolate())
        .to_rust_string_lossy(&mut js_runtime.handle_scope()))
}

async fn deno_tokio_loop(
    mut js_runtime: JsRuntime,
    mut global_fns: HashMap<String, v8::Global<v8::Function>>,
    mut rx: mpsc::Receiver<JsMsg>,
) {
    js_runtime
        .run_event_loop(Default::default())
        .await
        .expect("Error initializing deno-dist.js");
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
    // println!("register {}", &fn_name);
    let my_fn = get_fn(js_runtime, &fn_name);
    fn_map.insert(fn_name, my_fn);
    Ok("Ok".to_string())
}

deno_runtime::deno_core::extension!(
    runjs,
    ops = [
        op_hello,
    ],
    esm_entry_point = "ext:runjs/plugin_runtime.js",
    esm = [dir "src", "plugin_runtime.js"]
);

pub fn js_runtime_thread(rx: mpsc::Receiver<JsMsg>) {
    let file_path = std::fs::canonicalize("target/deno_dist.js").unwrap();
    let code = std::fs::read_to_string(file_path.as_path()).unwrap_or_default();

    let main_module = ModuleSpecifier::parse("data:text/plain").unwrap();
    let fs = Arc::new(RealFs);
    let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
        sys_traits::impls::RealSys,
    ));
    let worker = MainWorker::bootstrap_from_options(
        &main_module,
        WorkerServiceOptions::<
            DenoInNpmPackageChecker,
            NpmResolver<sys_traits::impls::RealSys>,
            sys_traits::impls::RealSys,
        > {
            module_loader: Rc::new(FsModuleLoader),
            permissions: PermissionsContainer::allow_all(permission_desc_parser),
            blob_store: Default::default(),
            broadcast_channel: Default::default(),
            feature_checker: Default::default(),
            node_services: Default::default(),
            npm_process_state_provider: Default::default(),
            root_cert_store_provider: Default::default(),
            fetch_dns_resolver: Default::default(),
            shared_array_buffer_store: Default::default(),
            compiled_wasm_module_store: Default::default(),
            v8_code_cache: Default::default(),
            fs,
        },
        WorkerOptions {
            extensions: vec![runjs::init_ops_and_esm()],
            ..Default::default()
        },
    );

    let tokio_runtime: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let mut js_runtime = worker.js_runtime;
    js_runtime.execute_script("deno_dist.js", code).unwrap();

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
