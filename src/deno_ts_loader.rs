use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_core::error::ModuleLoaderError;
use deno_core::ModuleLoadResponse;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_error::JsErrorBox;

pub struct TsModuleLoader;

impl deno_core::ModuleLoader for TsModuleLoader {
    fn load(
        &self,
        module_specifier: &deno_core::ModuleSpecifier,
        _referrer: Option<&reqwest::Url>,
        _is_dyn_import: bool,
        _requested_module_type: deno_core::RequestedModuleType,
    ) -> ModuleLoadResponse {
        let module_specifier = module_specifier.clone();

        let module_load = move || {
            let path = module_specifier.to_file_path().unwrap();

            let media_type = MediaType::from_path(&path);
            let (module_type, transpile) = match MediaType::from_path(&path) {
                MediaType::Json => (deno_core::ModuleType::Json, false),
                MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
                    (deno_core::ModuleType::JavaScript, false)
                }
                MediaType::TypeScript
                | MediaType::Cts
                | MediaType::Dcts
                | MediaType::Dmts
                | MediaType::Dts
                | MediaType::Jsx
                | MediaType::Mts
                | MediaType::Tsx => (deno_core::ModuleType::JavaScript, true),
                _ => panic!("Unknown extension {:?}", path.extension()),
            };

            let source_code = std::fs::read_to_string(&path)?;
            let code = if transpile {
                let parsed = deno_ast::parse_module(ParseParams {
                    specifier: module_specifier.clone(),
                    text: source_code.into(),
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
                source_code
            };
            let module_source = deno_core::ModuleSource::new(
                module_type,
                ModuleSourceCode::String(code.into()),
                &module_specifier,
                None,
            );
            Ok(module_source)
        };

        ModuleLoadResponse::Sync(module_load())
    }

    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: deno_core::ResolutionKind,
    ) -> std::result::Result<ModuleSpecifier, ModuleLoaderError> {
        deno_core::resolve_import(specifier, referrer).map_err(Into::into)
    }
}
