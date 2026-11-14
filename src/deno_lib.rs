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
use deno_core::error::{CoreError, CoreErrorKind, JsError};
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
use futures::StreamExt;
use futures::future::{self, Either, LocalBoxFuture};
use futures::stream::FuturesUnordered;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

type JsResult<T> = std::result::Result<T, String>;
type CallResult = Result<v8::Global<v8::Value>, Box<JsError>>;
type Responder = oneshot::Sender<JsResult<Value>>;
/// `None` once the caller stopped waiting (e.g. after a timeout).
type PendingCalls =
    FuturesUnordered<LocalBoxFuture<'static, Option<(String, CallResult, Responder)>>>;

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

/// The bundler moves static imports (e.g. `node:fs`) out of the bundle, which runs as a
/// classic script, and lists their specifiers in this var at the very start of the bundle.
/// They are loaded as ES module beforehand and exposed in `IMPORTS_GLOBAL` / `IMPORT_META_GLOBAL`.
const IMPORTS_VAR: &str = "_tauri_plugin_imports";
const IMPORTS_GLOBAL: &str = "__tauri_plugin_deno_imports";
const IMPORT_META_GLOBAL: &str = "__tauri_plugin_deno_import_meta";

/// Native addons crash the process if the executable doesn't export the N-API
/// functions, so loading one fails with a hint instead.
const NAPI_GUARD: &str = r#"
import Module from "node:module";
const message =
  "[tauri-plugin-deno] Native Node addons (.node) need the N-API symbols exported by the app executable. " +
  "Add to src-tauri/build.rs: if let Ok(arg) = std::env::var(\"DEP_TAURI_PLUGIN_DENO_NAPI_LINK_ARG\") " +
  "{ println!(\"cargo:rustc-link-arg-bins={arg}\"); }";
// Logged as well, because some addon loaders replace the error with their own.
const fail = () => {
  console.error(message);
  throw new Error(message);
};
Module._extensions[".node"] = fail;
process.dlopen = fail;
"#;

pub(crate) fn napi_exported() -> bool {
    let symbol = c"napi_create_function";
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn dlsym(
                handle: *mut std::ffi::c_void,
                symbol: *const std::ffi::c_char,
            ) -> *mut std::ffi::c_void;
        }
        // RTLD_DEFAULT
        #[cfg(target_vendor = "apple")]
        let handle = -2isize as *mut std::ffi::c_void;
        #[cfg(not(target_vendor = "apple"))]
        let handle = std::ptr::null_mut();
        // SAFETY: dlsym with RTLD_DEFAULT and a valid C string only looks up a symbol.
        !unsafe { dlsym(handle, symbol.as_ptr()) }.is_null()
    }
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetModuleHandleW(name: *const u16) -> *mut std::ffi::c_void;
            fn GetProcAddress(
                module: *mut std::ffi::c_void,
                name: *const std::ffi::c_char,
            ) -> *mut std::ffi::c_void;
        }
        // SAFETY: a null name returns the executable's handle; the lookup has no side effects.
        !unsafe { GetProcAddress(GetModuleHandleW(std::ptr::null()), symbol.as_ptr()) }.is_null()
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

fn bundle_imports(code: &str) -> JsResult<Option<Vec<String>>> {
    let Some(rest) = code.strip_prefix(&format!("var {IMPORTS_VAR} = ")) else {
        return Ok(None);
    };
    deno_core::serde_json::Deserializer::from_str(rest)
        .into_iter::<Vec<String>>()
        .next()
        .transpose()
        .map_err(|err| format!("invalid {IMPORTS_VAR}: {err}"))
}

/// Returns an ES module that imports URLs like `node:fs`, and a classic script that
/// loads bare and relative specifiers (externals such as native npm packages) with
/// `require` from `node_modules`. The script runs separately, because deno_core doesn't
/// report exceptions thrown while evaluating a side module.
fn imports_code(specifiers: &[String]) -> (String, String) {
    let mut module = String::from("import { createRequire } from \"node:module\";\n");
    let mut script = String::from(
        "(() => {\n\
         const require = globalThis.__tauri_plugin_deno_require;\n\
         delete globalThis.__tauri_plugin_deno_require;\n\
         const ns = (m) => m?.[Symbol.toStringTag] === \"Module\" ? m : { ...m, default: m };\n",
    );
    let mut entries = Vec::new();
    for (i, specifier) in specifiers.iter().enumerate() {
        let quoted = Value::from(specifier.as_str()).to_string();
        let is_url = ModuleSpecifier::parse(specifier).is_ok_and(|url| url.scheme().len() > 1);
        if is_url {
            module.push_str(&format!("import * as m{i} from {quoted};\n"));
            entries.push(format!("{quoted}: m{i}"));
        } else {
            script.push_str(&format!(
                "{IMPORTS_GLOBAL}[{quoted}] = ns(require({quoted}));\n"
            ));
        }
    }
    module.push_str(&format!(
        "globalThis.{IMPORTS_GLOBAL} = {{ {} }};\n\
         globalThis.{IMPORT_META_GLOBAL} = import.meta;\n\
         globalThis.__tauri_plugin_deno_require = createRequire(import.meta.url);\n",
        entries.join(", ")
    ));
    script.push_str("})();\n");
    (module, script)
}

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

    async fn run_side_module(&mut self, url: &ModuleSpecifier, code: String) -> JsResult<()> {
        let id = self
            .worker
            .js_runtime
            .load_side_es_module_from_code(url, code)
            .await
            .map_err(|err| self.js_err(err))?;
        self.worker
            .evaluate_module(id)
            .await
            .map_err(|err| self.js_err(err))
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

    /// Calls the function; the returned future settles with its (awaited) result
    /// while the event loop is polled.
    fn start_call(
        &mut self,
        name: &str,
        args: Vec<Value>,
    ) -> JsResult<impl Future<Output = CallResult> + use<>> {
        let function = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Function '{name}' has not been registered yet"))?;
        let args = self.v8_args_from_json(args)?;
        Ok(self.worker.js_runtime.call_with_args(&function, &args))
    }

    fn finish_call(&mut self, name: &str, result: CallResult) -> JsResult<Value> {
        let value = result
            .map_err(|err| format!("Error calling Deno function '{name}': {}", self.js_err(err)))?;
        self.json_from_v8(value)
    }

    fn read_var(&mut self, variable: &str) -> JsResult<Value> {
        let variable = variable.replace(&['(', ')', '"', ';', '\'', '=', ':', '`'][..], "");
        self.exec("<tauri-plugin-deno:read>", variable)
    }

    fn respond(&self, responder: Responder, result: JsResult<Value>) {
        if let Err(err) = &result
            && self.tracebacks
        {
            eprintln!("{LOG_PREFIX} {err}");
        }
        let _ = responder.send(result);
    }

    /// Answers the request, except function calls, which are added to `pending`
    /// and answered once their promise settles.
    fn handle(&mut self, msg: JsMsg, pending: &mut PendingCalls) {
        let result = match msg.req {
            JsRequest::RunCodeRequest(req) => self.exec("<tauri-plugin-deno:run>", req.value),
            JsRequest::ReadVarRequest(req) => self.read_var(&req.value),
            JsRequest::RegisterRequest(req) => {
                self.register(&req.function_name, req.number_of_args)
            }
            JsRequest::CallFnRequest(req) => match self.start_call(&req.function_name, req.args) {
                Ok(call) => {
                    let (name, mut responder) = (req.function_name, msg.responder);
                    // Dropping the call future only releases it; the JS keeps running.
                    pending.push(Box::pin(async move {
                        let closed = responder.closed();
                        let result = match future::select(pin!(call), pin!(closed)).await {
                            Either::Left((result, _)) => result,
                            Either::Right(_) => return None,
                        };
                        Some((name, result, responder))
                    }));
                    return;
                }
                Err(err) => Err(err),
            },
        };
        self.respond(msg.responder, result);
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
    Call(String, CallResult, Responder),
    Abandoned,
    EventLoop(Result<(), CoreError>),
}

/// Function calls stay pending while requests and events are handled, so a call can
/// await an event. An unsettled call is only ended by the caller's timeout.
async fn serve(
    mut state: DenoState,
    mut requests: mpsc::Receiver<JsMsg>,
    mut events: mpsc::UnboundedReceiver<IncomingEvent>,
) {
    let mut pending = PendingCalls::new();
    let mut idle = false;
    let mut events_open = true;
    loop {
        let wake = tokio::select! {
            msg = requests.recv() => Wake::Request(msg),
            event = events.recv(), if events_open => Wake::Event(event),
            Some(call) = pending.next(), if !pending.is_empty() => match call {
                Some((name, result, responder)) => Wake::Call(name, result, responder),
                None => Wake::Abandoned,
            },
            result = state.worker.js_runtime.run_event_loop(PollEventLoopOptions::default()), if !idle => {
                Wake::EventLoop(result)
            }
        };
        idle = false;
        match wake {
            Wake::Request(None) => break,
            Wake::Request(Some(msg)) => state.handle(msg, &mut pending),
            Wake::Event(Some(event)) => state.dispatch_event(event),
            Wake::Event(None) => events_open = false,
            Wake::Call(name, result, responder) => {
                let result = state.finish_call(&name, result);
                state.respond(responder, result);
            }
            Wake::Abandoned => {}
            // After an error, other timers or I/O may still be pending, so keep polling.
            Wake::EventLoop(Ok(())) => idle = true,
            Wake::EventLoop(Err(err)) => {
                eprintln!("{LOG_PREFIX} uncaught error: {}", state.js_err(err));
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
            node_services: Some(crate::node_services::create()),
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

async fn start(
    code: String,
    base_dir: &Path,
    outgoing: OutgoingSender,
    tracebacks: bool,
) -> JsResult<DenoState> {
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
    if !napi_exported() {
        let url =
            ModuleSpecifier::parse("file:///tauri-plugin-deno/napi-guard.js").expect("static url");
        state
            .run_side_module(&url, NAPI_GUARD.to_string())
            .await
            .map_err(|err| format!("napi guard: {err}"))?;
    }
    if let Some(specifiers) = bundle_imports(&code)? {
        // import.meta.url is the base for `require` and relative imports.
        let url = ModuleSpecifier::from_file_path(base_dir.join("deno_dist.js"))
            .map_err(|()| format!("invalid base directory {}", base_dir.display()))?;
        let (module, script) = imports_code(&specifiers);
        state
            .run_side_module(&url, module)
            .await
            .map_err(|err| format!("cannot load imports of deno_dist.js: {err}"))?;
        state
            .worker
            .js_runtime
            .execute_script("<tauri-plugin-deno:require>", script)
            .map_err(|err| format!("cannot load imports of deno_dist.js: {}", state.js_err(err)))?;
    }
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
    base_dir: PathBuf,
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
        match start(code, &base_dir, outgoing, tracebacks).await {
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
