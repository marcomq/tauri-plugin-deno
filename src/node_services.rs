//  Tauri Plugin Deno
//  © Copyright 2026, by Marco Mengelkoch
//  Licensed under MIT License, see License file for more details
//  git clone https://github.com/marcomq/tauri-plugin-deno

//! Node compatibility services: `require()`, CommonJS and native addons (`.node`).
//! npm packages are resolved from `node_modules` folders on disk ("bring your own
//! node_modules"), searched upwards from the importing file.

use deno_core::FastString;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_media_type::MediaType;
use deno_resolver::cjs::{CjsTracker, IsCjsResolutionMode};
use deno_resolver::npm::{
    ByonmNpmResolverCreateOptions, CreateInNpmPkgCheckerOptions, DenoInNpmPackageChecker,
    NpmResolver, NpmResolverCreateOptions,
};
use deno_runtime::deno_node::{NodeExtInitServices, NodeRequireLoader};
use deno_runtime::deno_permissions::PermissionsContainer;
use node_resolver::cache::NodeResolutionSys;
use node_resolver::errors::PackageJsonLoadError;
use node_resolver::{
    DenoIsBuiltInNodeModuleChecker, NodeResolver, NodeResolverOptions, NodeResolverRc,
    PackageJsonResolver, PackageJsonResolverRc,
};
use std::borrow::Cow;
use std::path::Path;
use std::rc::Rc;
use sys_traits::impls::RealSys;

pub(crate) type Services =
    NodeExtInitServices<DenoInNpmPackageChecker, NpmResolver<RealSys>, RealSys>;

struct RequireLoader {
    cjs_tracker: CjsTracker<DenoInNpmPackageChecker, RealSys>,
}

impl NodeRequireLoader for RequireLoader {
    /// The runtime has all permissions, see `PermissionsContainer::allow_all`.
    fn ensure_read_permission<'a>(
        &self,
        _permissions: &mut PermissionsContainer,
        path: Cow<'a, Path>,
    ) -> Result<Cow<'a, Path>, JsErrorBox> {
        Ok(path)
    }

    fn load_text_file_lossy(&self, path: &Path) -> Result<FastString, JsErrorBox> {
        let bytes = std::fs::read(path)
            .map_err(|err| JsErrorBox::generic(format!("cannot read {}: {err}", path.display())))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned().into())
    }

    fn is_maybe_cjs(&self, specifier: &Url) -> Result<bool, PackageJsonLoadError> {
        self.cjs_tracker
            .is_maybe_cjs(specifier, MediaType::from_specifier(specifier))
    }

    fn is_maybe_cjs_from_require(&self, specifier: &Url) -> Result<bool, PackageJsonLoadError> {
        self.cjs_tracker
            .is_maybe_cjs_from_require(specifier, MediaType::from_specifier(specifier))
    }
}

pub(crate) fn create() -> Services {
    let pkg_json_resolver: PackageJsonResolverRc<RealSys> =
        PackageJsonResolverRc::new(PackageJsonResolver::new(RealSys, None));
    let in_npm_pkg_checker = DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);
    let npm_resolver = NpmResolver::<RealSys>::new::<RealSys>(NpmResolverCreateOptions::Byonm(
        ByonmNpmResolverCreateOptions {
            root_node_modules_dir: None,
            search_stop_dir: None,
            sys: NodeResolutionSys::new(RealSys, None),
            pkg_json_resolver: pkg_json_resolver.clone(),
        },
    ));
    let node_resolver = NodeResolverRc::new(NodeResolver::new(
        in_npm_pkg_checker.clone(),
        DenoIsBuiltInNodeModuleChecker,
        npm_resolver,
        pkg_json_resolver.clone(),
        NodeResolutionSys::new(RealSys, None),
        NodeResolverOptions::default(),
    ));
    let cjs_tracker = CjsTracker::new(
        in_npm_pkg_checker,
        pkg_json_resolver.clone(),
        IsCjsResolutionMode::ImplicitTypeCommonJs,
        Vec::new(),
    );
    NodeExtInitServices {
        node_require_loader: Rc::new(RequireLoader { cjs_tracker }),
        node_resolver,
        pkg_json_resolver,
        sys: RealSys,
    }
}
