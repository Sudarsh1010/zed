mod extension_lsp_adapter;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use extension::{ExtensionGrammarProxy, ExtensionHostProxy, ExtensionLanguageProxy};
use gpui::App;
use language::{LanguageMatcher, LanguageName, LanguageRegistry, LoadedLanguage};
use node_runtime::NodeRuntime;

use extension_lsp_adapter::RuntimeBinaryPathFn;

#[derive(Clone)]
pub enum LspAccess {
    ViaLspStore(gpui::Entity<project::LspStore>),
    ViaWorkspaces(
        Arc<
            dyn Fn(&mut App) -> Result<Vec<gpui::Entity<project::LspStore>>>
                + Send
                + Sync
                + 'static,
        >,
    ),
    Noop,
}

pub fn init(
    lsp_access: LspAccess,
    extension_host_proxy: Arc<ExtensionHostProxy>,
    language_registry: Arc<LanguageRegistry>,
    node_runtime: Option<NodeRuntime>,
) {
    let runtime_binary_path_fn = node_runtime.map(|nr| {
        Arc::new(move || -> futures::future::BoxFuture<'static, Option<PathBuf>> {
            let nr = nr.clone();
            Box::pin(async move { nr.binary_path().await.ok() })
        }) as RuntimeBinaryPathFn
    });
    let language_server_registry_proxy = LanguageServerRegistryProxy {
        language_registry,
        lsp_access,
        runtime_binary_path_fn,
    };
    extension_host_proxy.register_grammar_proxy(language_server_registry_proxy.clone());
    extension_host_proxy.register_language_proxy(language_server_registry_proxy.clone());
    extension_host_proxy.register_language_server_proxy(language_server_registry_proxy);
}

#[derive(Clone)]
struct LanguageServerRegistryProxy {
    language_registry: Arc<LanguageRegistry>,
    lsp_access: LspAccess,
    runtime_binary_path_fn: Option<RuntimeBinaryPathFn>,
}

impl ExtensionGrammarProxy for LanguageServerRegistryProxy {
    #[ztracing::instrument(skip_all)]
    fn register_grammars(&self, grammars: Vec<(Arc<str>, PathBuf)>) {
        self.language_registry.register_wasm_grammars(grammars)
    }
}

impl ExtensionLanguageProxy for LanguageServerRegistryProxy {
    fn register_language(
        &self,
        language: LanguageName,
        grammar: Option<Arc<str>>,
        matcher: LanguageMatcher,
        hidden: bool,
        load: Arc<dyn Fn() -> Result<LoadedLanguage> + Send + Sync + 'static>,
    ) {
        self.language_registry
            .register_language(language, grammar, matcher, hidden, None, load);
    }

    fn remove_languages(
        &self,
        languages_to_remove: &[LanguageName],
        grammars_to_remove: &[Arc<str>>,
    ) {
        self.language_registry
            .remove_languages(languages_to_remove, grammars_to_remove);
    }
}
