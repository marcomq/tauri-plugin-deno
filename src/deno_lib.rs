//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use crate::LOG_PREFIX;
use crate::deno_ops::*;
use crate::models::*;
use deno_core::FsModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::PollEventLoopOptions;
use deno_core::error::{CoreError, CoreErrorKind};
use deno_core::serde_json::Value;
use deno_core::serde_v8;
use deno_core::v8;
use deno_fs::RealFs;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::worker::WorkerServiceOptions;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::mpsc;

type JsResult<T> = std::result::Result<T, String>;

/// Makes `console.*` safe when stdout/stderr are invalid, e.g. on a Windows
/// release build without a console (`windows_subsystem = "windows"`).
const STDIO_GUARD: &str = r#"
for (const name of ["log", "info", "warn", "error", "debug", "trace", "dir", "table"]) {
  const original = globalThis.console?.[name];
  if (typeof original === "function") {
    globalThis.console[name] = function (...args) {
      try { return original.apply(this, args); } catch { /* no usable stdio */ }
    };
  }
}
"#;

/// Global holding the names of all functions exported by the bundle entry point,
/// written by the bundler (`deno_bundler.js`).
const EXPORTS_VAR: &str = "_tauri_plugin_exports";
const FUNCTIONS_VAR: &str = "_tauri_plugin_functions";
const DISPATCH_FN: &str = "__tauriPluginDispatch";

pub(crate) fn error_message(err: &CoreError, tracebacks: bool) -> String {
    kind_message(&err.0, tracebacks)
}

fn kind_message(kind: &CoreErrorKind, tracebacks: bool) -> String {
    match kind {
        CoreErrorKind::Js(js) if tracebacks => js.to_string(),
        CoreErrorKind::Js(js) => js.exception_message.clone(),
        CoreErrorKind::CouldNotExecute { error, .. } => kind_message(error, tracebacks),
        other => other.to_string(),
    }
}

/// Accepts plain identifiers and dotted paths like `api.users.list`.
fn is_valid_fn_path(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
                && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        })
}

struct DenoState {
    worker: MainWorker,
    functions: HashMap<String, v8::Global<v8::Function>>,
    dispatch: Option<v8::Global<v8::Function>>,
    tracebacks: bool,
}

impl DenoState {
    fn js_err(&self, err: impl Into<CoreError>) -> String {
        error_message(&err.into(), self.tracebacks)
    }

    /// Converts like `JSON.stringify`, the same way Tauri sends values to the frontend.
    /// `serde_v8::from_v8` would panic on e.g. symbols and kill the Deno thread.
    fn json_from_v8(&mut self, value: v8::Global<v8::Value>) -> JsResult<Value> {
        deno_core::scope!(scope, self.worker.js_runtime);
        v8::tc_scope!(let scope, scope);
        let local = v8::Local::new(scope, value);
        let Some(json) = v8::json::stringify(scope, local) else {
            let reason = scope
                .exception()
                .map(|err| err.to_rust_string_lossy(scope))
                .unwrap_or_else(|| "unknown error".into());
            return Err(format!("cannot convert the result to JSON: {reason}"));
        };
        let json = json.to_rust_string_lossy(scope);
        if json == "undefined" {
            return Ok(Value::Null);
        }
        deno_core::serde_json::from_str(&json)
            .map_err(|err| format!("cannot convert the result to JSON: {err}"))
    }

    fn v8_args_from_json(&mut self, args: Vec<Value>) -> JsResult<Vec<v8::Global<v8::Value>>> {
        deno_core::scope!(scope, self.worker.js_runtime);
        args.into_iter()
            .map(|arg| {
                let local = serde_v8::to_v8(scope, arg)
                    .map_err(|err| format!("cannot convert argument: {err}"))?;
                Ok(v8::Global::new(scope, local))
            })
            .collect()
    }

    fn exec(&mut self, name: &'static str, code: String) -> JsResult<Value> {
        let value = self
            .worker
            .js_runtime
            .execute_script(name, code)
            .map_err(|err| self.js_err(err))?;
        self.json_from_v8(value)
    }

    fn lookup_fn(&mut self, name: &str) -> JsResult<v8::Global<v8::Function>> {
        if !is_valid_fn_path(name) {
            return Err(format!("'{name}' is not a valid function name"));
        }
        let not_found = || {
            format!(
                "'{name}' not found in deno_dist.js (is it defined at top level, exported or passed to addTauri?)"
            )
        };
        // Methods like `api.list` are bound to their object, so `this` works.
        let code = match name.rsplit_once('.') {
            Some((parent, _)) => format!(
                "typeof {name} === 'function' ? {name}.bind({parent}) : typeof {name} === 'undefined' ? undefined : {name}"
            ),
            None => format!("typeof {name} === 'undefined' ? undefined : {name}"),
        };
        let value = self
            .worker
            .js_runtime
            .execute_script("<tauri-plugin-deno:lookup>", code)
            .map_err(|_| not_found())?;
        deno_core::scope!(scope, self.worker.js_runtime);
        let local = v8::Local::new(scope, value);
        if local.is_undefined() {
            return Err(not_found());
        }
        let function = v8::Local::<v8::Function>::try_from(local)
            .map_err(|_| format!("'{name}' is not a function"))?;
        Ok(v8::Global::new(scope, function))
    }

    fn function_length(&mut self, function: &v8::Global<v8::Function>) -> u32 {
        deno_core::scope!(scope, self.worker.js_runtime);
        let function = v8::Local::new(scope, function);
        let key = v8::String::new(scope, "length").expect("static string");
        function
            .get(scope, key.into())
            .and_then(|len| len.uint32_value(scope))
            .unwrap_or(0)
    }

    fn register(&mut self, name: &str, number_of_args: Option<u8>) -> JsResult<Value> {
        let function = self
            .lookup_fn(name)
            .map_err(|err| format!("Cannot register {err}"))?;
        if let Some(given) = number_of_args {
            let required = self.function_length(&function);
            if u32::from(given) < required {
                return Err(format!(
                    "Cannot register '{name}': it requires {required} arguments, but numberOfArgs is {given}"
                ));
            }
        }
        self.functions.insert(name.to_string(), function);
        Ok(Value::Bool(true))
    }

    async fn call(&mut self, name: &str, args: Vec<Value>) -> JsResult<Value> {
        let function = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Function '{name}' has not been registered yet"))?;
        let args = self.v8_args_from_json(args)?;
        let call = self.worker.js_runtime.call_with_args(&function, &args);
        let value = self
            .worker
            .js_runtime
            .with_event_loop_promise(call, PollEventLoopOptions::default())
            .await
            .map_err(|err| format!("Error calling Deno function '{name}': {}", self.js_err(err)))?;
        self.json_from_v8(value)
    }

    fn read_var(&mut self, variable: &str) -> JsResult<Value> {
        let variable = variable.replace(&['(', ')', '"', ';', '\'', '=', ':', '`'][..], "");
        self.exec("<tauri-plugin-deno:read>", variable)
    }

    async fn handle(&mut self, req: JsRequest) -> JsResult<Value> {
        match req {
            JsRequest::RunCodeRequest(req) => self.exec("<tauri-plugin-deno:run>", req.value),
            JsRequest::ReadVarRequest(req) => self.read_var(&req.value),
            JsRequest::RegisterRequest(req) => {
                self.register(&req.function_name, req.number_of_args)
            }
            JsRequest::CallFnRequest(req) => self.call(&req.function_name, req.args).await,
        }
    }

    fn dispatch_event(&mut self, event: IncomingEvent) {
        let dispatch = match self.dispatch.clone() {
            Some(dispatch) => Ok(dispatch),
            None => self.lookup_fn(DISPATCH_FN),
        };
        let result = dispatch.and_then(|dispatch| {
            self.dispatch = Some(dispatch.clone());
            let args = self.v8_args_from_json(vec![Value::String(event.name), event.payload])?;
            drop(self.worker.js_runtime.call_with_args(&dispatch, &args));
            Ok(())
        });
        if let Err(err) = result {
            eprintln!("{LOG_PREFIX} cannot dispatch event: {err}");
        }
    }

    fn register_startup_functions(&mut self) -> JsResult<()> {
        for var in [FUNCTIONS_VAR, EXPORTS_VAR] {
            let names = match self.read_var(&format!("globalThis.{var} ?? []")) {
                Ok(Value::Array(names)) => names,
                Ok(other) => return Err(format!("{var} must be an array of names, got {other}")),
                Err(err) => return Err(err),
            };
            for name in names {
                let name = name
                    .as_str()
                    .ok_or_else(|| format!("{var} must only contain strings, got {name}"))?;
                // Exports may be constants or classes; only functions become callable.
                if var == EXPORTS_VAR && self.read_var(&format!("typeof {name}"))? != "function" {
                    continue;
                }
                self.register(name, None)?;
            }
        }
        Ok(())
    }
}

enum Wake {
    Request(Option<JsMsg>),
    Event(Option<IncomingEvent>),
    EventLoop(Result<(), CoreError>),
}

async fn serve(
    mut state: DenoState,
    mut requests: mpsc::Receiver<JsMsg>,
    mut events: mpsc::UnboundedReceiver<IncomingEvent>,
) {
    let mut idle = false;
    let mut events_open = true;
    loop {
        let wake = tokio::select! {
            msg = requests.recv() => Wake::Request(msg),
            event = events.recv(), if events_open => Wake::Event(event),
            result = state.worker.js_runtime.run_event_loop(PollEventLoopOptions::default()), if !idle => {
                Wake::EventLoop(result)
            }
        };
        match wake {
            Wake::Request(None) => break,
            Wake::Request(Some(msg)) => {
                let result = state.handle(msg.req).await;
                if let Err(err) = &result
                    && state.tracebacks
                {
                    eprintln!("{LOG_PREFIX} {err}");
                }
                let _ = msg.responder.send(result);
                idle = false;
            }
            Wake::Event(Some(event)) => {
                state.dispatch_event(event);
                idle = false;
            }
            Wake::Event(None) => events_open = false,
            Wake::EventLoop(result) => {
                idle = true;
                if let Err(err) = result {
                    eprintln!("{LOG_PREFIX} uncaught error: {}", state.js_err(err));
                }
            }
        }
    }
}

/// Answers every request with the startup error, so the frontend sees why nothing works.
async fn serve_startup_error(error: String, mut requests: mpsc::Receiver<JsMsg>) {
    while let Some(msg) = requests.recv().await {
        let _ = msg
            .responder
            .send(Err(format!("Deno runtime failed to start: {error}")));
    }
}

deno_runtime::deno_core::extension!(
    tauri_plugin_deno,
    ops = [op_emit, op_listen],
    esm_entry_point = "ext:tauri_plugin_deno/plugin_runtime.js",
    esm = [dir "src", "plugin_runtime.js"],
    options = { outgoing: OutgoingSender },
    state = |state, options| {
        state.put::<OutgoingSender>(options.outgoing);
    },
);

fn create_worker(outgoing: OutgoingSender) -> MainWorker {
    let main_module = ModuleSpecifier::parse("data:text/plain").expect("static url");
    let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
        sys_traits::impls::RealSys,
    ));
    MainWorker::bootstrap_from_options(
        &main_module,
        WorkerServiceOptions::<
            DenoInNpmPackageChecker,
            NpmResolver<sys_traits::impls::RealSys>,
            sys_traits::impls::RealSys,
        > {
            module_loader: Rc::new(FsModuleLoader),
            permissions: PermissionsContainer::allow_all(permission_desc_parser),
            blob_store: Arc::new(BlobStore::default()),
            bundle_provider: None,
            deno_rt_native_addon_loader: None,
            broadcast_channel: Default::default(),
            feature_checker: Default::default(),
            node_services: Default::default(),
            npm_process_state_provider: Default::default(),
            root_cert_store_provider: Default::default(),
            fetch_dns_resolver: Default::default(),
            shared_array_buffer_store: Default::default(),
            compiled_wasm_module_store: Default::default(),
            v8_code_cache: Default::default(),
            fs: Arc::new(RealFs),
        },
        WorkerOptions {
            extensions: vec![tauri_plugin_deno::init(outgoing)],
            ..Default::default()
        },
    )
}

async fn start(code: String, outgoing: OutgoingSender, tracebacks: bool) -> JsResult<DenoState> {
    let mut state = DenoState {
        worker: create_worker(outgoing),
        functions: HashMap::new(),
        dispatch: None,
        tracebacks,
    };
    state
        .worker
        .js_runtime
        .execute_script("<tauri-plugin-deno:stdio-guard>", STDIO_GUARD)
        .map_err(|err| format!("stdio guard: {}", state.js_err(err)))?;
    state
        .worker
        .js_runtime
        .execute_script("deno_dist.js", code)
        .map_err(|err| format!("error in deno_dist.js: {}", state.js_err(err)))?;
    state.register_startup_functions()?;
    Ok(state)
}

/// Runs the Deno runtime on the current thread until `requests` is closed.
pub(crate) fn run(
    code: String,
    requests: mpsc::Receiver<JsMsg>,
    events: mpsc::UnboundedReceiver<IncomingEvent>,
    outgoing: OutgoingSender,
    tracebacks: bool,
) {
    let tokio_runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("{LOG_PREFIX} cannot create tokio runtime for Deno: {err}");
            return;
        }
    };
    tokio_runtime.block_on(async move {
        match start(code, outgoing, tracebacks).await {
            Ok(state) => serve(state, requests, events).await,
            Err(err) => {
                eprintln!(
                    "{LOG_PREFIX} Deno runtime failed to start: {err}\n\
                     Hint: is deno_dist.js up to date? Run the bundler (`tauri-plugin-deno-api bundle`) before building."
                );
                serve_startup_error(err, requests).await;
            }
        }
    });
}
