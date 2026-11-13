//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use super::*;
use serde_json::json;

const CODE: &str = r#"
var counter = 0;
function add(a, b) { return a + b; }
function greet(name) { counter += 1; return `Hello ${name} #${counter}`; }
function fails() { throw new Error("intended failure"); }
async function later(ms) { await new Promise((r) => setTimeout(r, ms)); return ms; }
function hangs() { return new Promise(() => {}); }
function returnsSymbol() { return { keep: 1, drop: Symbol("x"), date: new Date(0) }; }
function returnsBigInt() { return 1n; }
function needsTwo(a, b) { return a; }
const arrow = (x) => x * 2;
var api = { double(x) { return x * 2; } };
function notListed() { return "not listed"; }
var notAFunction = 42;
var received = [];
listen("to-deno", (payload) => { received.push(payload); emit("from-deno", { echo: payload }); });
_tauri_plugin_functions = [add.name, greet.name, fails.name, later.name, hangs.name, returnsSymbol.name, returnsBigInt.name];
var _tauri_plugin_exports = ["arrow"];
"#;

fn spawn(
    code: &str,
    timeout: Option<Duration>,
) -> (
    DenoHandle,
    mpsc::UnboundedReceiver<Outgoing>,
    IncomingSender,
) {
    DenoHandle::spawn(code.to_string(), timeout, true)
}

fn call(name: &str, args: Vec<Value>) -> JsRequest {
    JsRequest::CallFnRequest(CallFnRequest {
        function_name: name.into(),
        args,
    })
}

fn register(name: &str, number_of_args: Option<u8>) -> JsRequest {
    JsRequest::RegisterRequest(RegisterRequest {
        function_name: name.into(),
        number_of_args,
    })
}

fn run(code: &str) -> JsRequest {
    JsRequest::RunCodeRequest(RunCodeRequest { value: code.into() })
}

fn read(name: &str) -> JsRequest {
    JsRequest::ReadVarRequest(ReadVarRequest { value: name.into() })
}

fn err_text(result: Result<Value>) -> String {
    result.expect_err("expected an error").to_string()
}

mod runtime {
    use super::*;

    #[tokio::test]
    async fn calls_registered_function() {
        let (deno, _, _) = spawn(CODE, None);
        let value = deno
            .request(call("add", vec![json!(2), json!(3)]))
            .await
            .unwrap();
        assert_eq!(value, json!(5));
    }

    #[tokio::test]
    async fn keeps_state_between_calls() {
        let (deno, _, _) = spawn(CODE, None);
        deno.request(call("greet", vec![json!("a")])).await.unwrap();
        let value = deno.request(call("greet", vec![json!("b")])).await.unwrap();
        assert_eq!(value, json!("Hello b #2"));
    }

    #[tokio::test]
    async fn passes_json_values() {
        let (deno, _, _) = spawn(CODE, None);
        let value = deno
            .request(run("(function (o) { return { sum: o.a + o.b.length, list: [1, null, 'x'] }; })({ a: 1, b: [1, 2] })"))
            .await
            .unwrap();
        assert_eq!(value, json!({ "sum": 3, "list": [1, null, "x"] }));
    }

    #[tokio::test]
    async fn exported_functions_are_callable() {
        let (deno, _, _) = spawn(CODE, None);
        let value = deno.request(call("arrow", vec![json!(21)])).await.unwrap();
        assert_eq!(value, json!(42));
    }

    #[tokio::test]
    async fn awaits_async_functions_with_timers() {
        let (deno, _, _) = spawn(CODE, None);
        let value = deno.request(call("later", vec![json!(20)])).await.unwrap();
        assert_eq!(value, json!(20));
    }

    #[tokio::test]
    async fn unregistered_function_fails_and_runtime_survives() {
        let (deno, _, _) = spawn(CODE, None);
        let err = err_text(deno.request(call("doesNotExist", vec![])).await);
        assert_eq!(err, "Function 'doesNotExist' has not been registered yet");
        let err = err_text(deno.request(call("notListed", vec![])).await);
        assert!(err.contains("not been registered"), "{err}");
        let value = deno
            .request(call("add", vec![json!(1), json!(1)]))
            .await
            .unwrap();
        assert_eq!(value, json!(2));
    }

    #[tokio::test]
    async fn js_exception_is_reported_with_context() {
        let (deno, _, _) = spawn(CODE, None);
        let err = err_text(deno.request(call("fails", vec![])).await);
        assert!(
            err.starts_with("Error calling Deno function 'fails':"),
            "{err}"
        );
        assert!(err.contains("intended failure"), "{err}");
        let value = deno
            .request(call("add", vec![json!(1), json!(2)]))
            .await
            .unwrap();
        assert_eq!(value, json!(3));
    }

    #[tokio::test]
    async fn syntax_error_in_run_code_does_not_kill_runtime() {
        let (deno, _, _) = spawn(CODE, None);
        let err = err_text(deno.request(run("this is not js")).await);
        assert!(err.contains("SyntaxError"), "{err}");
        assert_eq!(deno.request(run("1 + 1")).await.unwrap(), json!(2));
    }

    #[tokio::test]
    async fn results_follow_json_semantics() {
        let (deno, _, _) = spawn(CODE, None);
        let value = deno.request(call("returnsSymbol", vec![])).await.unwrap();
        assert_eq!(
            value,
            json!({ "keep": 1, "date": "1970-01-01T00:00:00.000Z" })
        );
        assert_eq!(deno.request(run("undefined")).await.unwrap(), Value::Null);
        let err = err_text(deno.request(call("returnsBigInt", vec![])).await);
        assert!(
            err.contains("cannot convert") && err.contains("BigInt"),
            "{err}"
        );
        assert_eq!(
            deno.request(call("add", vec![json!(1), json!(3)]))
                .await
                .unwrap(),
            json!(4)
        );
    }

    #[tokio::test]
    async fn register_from_frontend() {
        let (deno, _, _) = spawn(CODE, None);
        assert_eq!(
            deno.request(register("notListed", None)).await.unwrap(),
            json!(true)
        );
        assert_eq!(
            deno.request(call("notListed", vec![])).await.unwrap(),
            json!("not listed")
        );
        deno.request(register("api.double", Some(1))).await.unwrap();
        assert_eq!(
            deno.request(call("api.double", vec![json!(4)]))
                .await
                .unwrap(),
            json!(8)
        );
    }

    #[tokio::test]
    async fn register_rejects_bad_targets() {
        let (deno, _, _) = spawn(CODE, None);
        let err = err_text(deno.request(register("missing", None)).await);
        assert!(err.contains("'missing' not found"), "{err}");
        let err = err_text(deno.request(register("notAFunction", None)).await);
        assert!(err.contains("is not a function"), "{err}");
        let err = err_text(deno.request(register("alert(1)", None)).await);
        assert!(err.contains("not a valid function name"), "{err}");
        let err = err_text(deno.request(register("needsTwo", Some(1))).await);
        assert!(err.contains("requires 2 arguments"), "{err}");
        deno.request(register("needsTwo", Some(2))).await.unwrap();
    }

    #[tokio::test]
    async fn reads_variables() {
        let (deno, _, _) = spawn(CODE, None);
        assert_eq!(deno.request(read("notAFunction")).await.unwrap(), json!(42));
        let err = err_text(deno.request(read("unknownVariable")).await);
        assert!(err.contains("unknownVariable is not defined"), "{err}");
    }

    #[tokio::test]
    async fn timeout_returns_error_quickly() {
        let (deno, _, _) = spawn(CODE, Some(Duration::from_millis(200)));
        let err = deno.request(call("hangs", vec![])).await.unwrap_err();
        assert!(matches!(err, Error::Timeout(_)), "{err}");
    }

    #[tokio::test]
    async fn startup_error_is_reported_on_every_call() {
        let (deno, _, _) = spawn("this is not valid javascript", None);
        let err = err_text(deno.request(call("add", vec![])).await);
        assert!(
            err.starts_with("Deno runtime failed to start: error in deno_dist.js"),
            "{err}"
        );
        let err = err_text(deno.request(run("1")).await);
        assert!(err.starts_with("Deno runtime failed to start"), "{err}");
    }

    #[tokio::test]
    async fn invalid_function_list_is_a_startup_error() {
        let (deno, _, _) = spawn("_tauri_plugin_functions = ['missingFn'];", None);
        let err = err_text(deno.request(run("1")).await);
        assert!(err.contains("'missingFn' not found"), "{err}");
    }

    #[tokio::test]
    async fn emits_and_receives_events() {
        let (deno, mut outgoing, incoming) = spawn(CODE, None);
        match outgoing.recv().await {
            Some(Outgoing::Listen(name)) => assert_eq!(name, "to-deno"),
            other => panic!("expected listen, got {other:?}"),
        }
        incoming
            .send(IncomingEvent {
                name: "to-deno".into(),
                payload: json!({ "n": 1 }),
            })
            .unwrap();
        match outgoing.recv().await {
            Some(Outgoing::Emit(name, value)) => {
                assert_eq!(name, "from-deno");
                assert_eq!(value, json!({ "echo": { "n": 1 } }));
            }
            other => panic!("expected emit, got {other:?}"),
        }
        assert_eq!(
            deno.request(read("received")).await.unwrap(),
            json!([{ "n": 1 }])
        );
    }

    #[tokio::test]
    async fn non_function_exports_are_skipped() {
        let (deno, _, _) = spawn(
            "var VERSION = '1'; function f() { return 1; } var _tauri_plugin_exports = ['VERSION', 'f'];",
            None,
        );
        assert_eq!(deno.request(call("f", vec![])).await.unwrap(), json!(1));
        let err = err_text(deno.request(call("VERSION", vec![])).await);
        assert!(err.contains("has not been registered"), "{err}");
    }

    #[tokio::test]
    async fn dotted_functions_keep_this() {
        let (deno, _, _) = spawn(
            "var store = { items: [1, 2], list() { return this.items; } };",
            None,
        );
        deno.request(register("store.list", None)).await.unwrap();
        assert_eq!(
            deno.request(call("store.list", vec![])).await.unwrap(),
            json!([1, 2])
        );
    }

    #[tokio::test]
    async fn background_timers_run_between_calls() {
        let (deno, mut outgoing, _) = spawn(
            "setTimeout(() => emit('tick', 1), 10); _tauri_plugin_functions = [];",
            None,
        );
        deno.request(run("1")).await.unwrap();
        match tokio::time::timeout(Duration::from_secs(5), outgoing.recv()).await {
            Ok(Some(Outgoing::Emit(name, _))) => assert_eq!(name, "tick"),
            other => panic!("expected tick, got {other:?}"),
        }
    }
}

mod tauri_app {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};

    fn app() -> tauri::App<MockRuntime> {
        mock_builder()
            .plugin(plugin(|| CODE.to_string()))
            .build(mock_context(noop_assets()))
            .expect("mock app")
    }

    #[test]
    fn rust_api_calls_deno() {
        let app = app();
        let value =
            tauri::async_runtime::block_on(app.deno().call_function("greet", vec![json!("Rust")]))
                .unwrap();
        assert_eq!(value, json!("Hello Rust #1"));
    }

    #[test]
    fn frontend_events_reach_deno_and_back() {
        let app = app();
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = Arc::new(Mutex::new(tx));
        app.listen_any("from-deno", move |event| {
            let _ = tx.lock().unwrap().send(event.payload().to_string());
        });
        // Waits until Deno's `listen` call reached the Tauri side.
        let deno = app.deno();
        tauri::async_runtime::block_on(deno.run_code("1")).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let payload = loop {
            app.emit("to-deno", json!("ping")).unwrap();
            if let Ok(payload) = rx.recv_timeout(Duration::from_millis(100)) {
                break payload;
            }
            assert!(std::time::Instant::now() < deadline, "no event from Deno");
        };
        assert_eq!(payload, r#"{"echo":"ping"}"#);
    }

    const ECHO: &str = "var count = 0; listen('echo', (p) => { count += 1; emit('echo', p); });";

    #[test]
    fn deno_does_not_receive_its_own_events() {
        let app = mock_builder()
            .plugin(plugin(|| ECHO.to_string()))
            .build(mock_context(noop_assets()))
            .expect("mock app");
        let deno = app.deno();
        let count = || {
            tauri::async_runtime::block_on(deno.read_variable("count"))
                .unwrap()
                .as_u64()
                .unwrap()
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut sent = 0;
        while count() == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "no event reached Deno"
            );
            app.emit("echo", json!(1)).unwrap();
            sent += 1;
            std::thread::sleep(Duration::from_millis(100));
        }
        std::thread::sleep(Duration::from_millis(500));
        assert!(
            count() <= sent,
            "Deno received its own emits: {} > {sent}",
            count()
        );
    }
}
