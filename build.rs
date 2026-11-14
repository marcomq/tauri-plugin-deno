//  Tauri Plugin Deno
//  © Copyright 2025, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

use std::path::{Path, PathBuf};

const COMMANDS: &[&str] = &[
    "run_code",
    "register_function",
    "call_function",
    "read_variable",
];

const DIST_ENV: &str = "TAURI_PLUGIN_DENO_DIST";
const DIST_FILE: &str = "deno_dist.js";

/// Finds the bundle: `TAURI_PLUGIN_DENO_DIST` if set, otherwise the closest
/// `deno_dist.js` above `OUT_DIR` (normally `<app>/src-tauri/target/deno_dist.js`,
/// also with `--target <triple>` or a custom target dir).
fn find_dist() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(DIST_ENV) {
        return Some(PathBuf::from(path));
    }
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR")?);
    out_dir
        .ancestors()
        .map(|dir| dir.join(DIST_FILE))
        .find(|candidate| candidate.is_file())
}

fn stub_dist() -> PathBuf {
    let stub = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"))
        .join("deno_dist_stub.js");
    std::fs::write(&stub, "// tauri-plugin-deno: no deno_dist.js available\n")
        .expect("cannot write stub bundle");
    stub
}

fn resolve_dist() -> PathBuf {
    println!("cargo:rerun-if-env-changed={DIST_ENV}");
    let found = find_dist().filter(|path| path.is_file());
    if let Some(path) = found {
        let path = path.canonicalize().unwrap_or(path);
        println!("cargo:rerun-if-changed={}", path.display());
        return path;
    }
    if std::env::var_os("DOCS_RS").is_some() {
        return stub_dist();
    }
    if let Some(path) = std::env::var_os(DIST_ENV) {
        panic!(
            "\n[tauri-plugin-deno] {DIST_ENV} points to '{}', but that file does not exist.\n",
            Path::new(&path).display()
        );
    }
    panic!(
        "\n[tauri-plugin-deno] {DIST_FILE} not found.\n\
         Bundle your Deno code before compiling, e.g. with\n  \
         deno run -A npm:tauri-plugin-deno-api bundle\n\
         (or use `tauri-plugin-deno-api dev` / `build`, which do this for you).\n\
         The bundle is expected at src-tauri/target/{DIST_FILE}; set {DIST_ENV} to an absolute path to use another location.\n"
    );
}

/// Native Node addons (`.node`) look up the `napi_*` functions in the executable, so it
/// has to export them. The lists in `napi/` are copied from `deno_napi` 0.191 (used by
/// `deno_runtime` 0.268) and must be updated with it.
fn napi_link_arg() -> Option<String> {
    let os = std::env::var("CARGO_CFG_TARGET_OS").ok()?;
    let list = |name: &str| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("napi")
            .join(name)
    };
    match os.as_str() {
        "macos" => Some(format!(
            "-Wl,-exported_symbols_list,{}",
            list("macos.def").display()
        )),
        "linux" | "android" | "freebsd" | "openbsd" => Some(format!(
            "-Wl,--export-dynamic-symbol-list={}",
            list("linux.def").display()
        )),
        "windows" => Some(format!("/DEF:{}", list("windows.def").display())),
        _ => None,
    }
}

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .global_api_script_path("./dist-js/index.iife.js")
        .android_path("android")
        .ios_path("ios")
        .build();

    println!(
        "cargo:rustc-env=TAURI_PLUGIN_DENO_DIST_PATH={}",
        resolve_dist().display()
    );

    // Apps read this as DEP_TAURI_PLUGIN_DENO_NAPI_LINK_ARG in their build.rs.
    if let Some(arg) = napi_link_arg() {
        println!("cargo:napi_link_arg={arg}");
        println!("cargo:rustc-link-arg={arg}");
        println!("cargo:rerun-if-changed=napi");
    }

    // Windows: test binaries need a Common-Controls v6 manifest, otherwise they abort
    // with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139). See tauri-apps/tauri #13419.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    }
}
