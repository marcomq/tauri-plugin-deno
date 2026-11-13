//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use super::*;

mod traceback {
    use super::*;

    #[test]
    fn env_var_enables_tracebacks() {
        assert!(tracebacks_enabled(Some("1")));
        assert!(tracebacks_enabled(Some("true")));
        assert!(tracebacks_enabled(Some(" TRUE ")));
    }

    #[test]
    fn without_env_var_only_debug_builds_have_tracebacks() {
        assert_eq!(tracebacks_enabled(None), cfg!(debug_assertions));
        assert_eq!(tracebacks_enabled(Some("0")), cfg!(debug_assertions));
        assert_eq!(
            tracebacks_enabled(Some("yes please")),
            cfg!(debug_assertions)
        );
    }
}

mod timeout {
    use super::*;

    #[test]
    fn default_timeout_applies_without_valid_value() {
        let default = Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        assert_eq!(call_timeout(None), default);
        assert_eq!(call_timeout(Some("abc")), default);
        assert_eq!(call_timeout(Some("-1")), default);
    }

    #[test]
    fn env_var_overrides_or_disables_timeout() {
        assert_eq!(call_timeout(Some(" 12 ")), Some(Duration::from_secs(12)));
        assert_eq!(call_timeout(Some("0")), None);
    }
}

mod payload {
    use super::*;

    #[test]
    fn register_request_matches_frontend_payload() {
        let req: RegisterRequest =
            serde_json::from_str(r#"{"functionName":"greetJs","numberOfArgs":1}"#).unwrap();
        assert_eq!(req.function_name, "greetJs");
        assert_eq!(req.number_of_args, Some(1));

        let req: RegisterRequest = serde_json::from_str(r#"{"functionName":"greetJs"}"#).unwrap();
        assert_eq!(req.number_of_args, None);
    }

    #[test]
    fn call_request_args_default_to_empty() {
        let req: CallFnRequest = serde_json::from_str(r#"{"functionName":"f"}"#).unwrap();
        assert!(req.args.is_empty());
    }

    #[test]
    fn error_serializes_as_plain_message() {
        let json = serde_json::to_string(&Error::Js("boom".into())).unwrap();
        assert_eq!(json, r#""boom""#);
    }
}

mod names {
    use super::*;

    #[test]
    fn event_names_follow_tauri_rules() {
        assert!(is_valid_event_name("progress"));
        assert!(is_valid_event_name("app:job-1/done_now"));
        assert!(!is_valid_event_name(""));
        assert!(!is_valid_event_name("has space"));
        assert!(!is_valid_event_name("dot.name"));
    }
}
