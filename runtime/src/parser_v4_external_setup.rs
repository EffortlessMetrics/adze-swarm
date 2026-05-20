use crate::external_scanner::ExternalScannerRuntime;
use crate::scanner_registry::{DynExternalScanner, get_global_registry};
use adze_ir::Grammar;

pub(crate) fn build_external_scanner(
    grammar: &Grammar,
    language_name: &str,
    has_external_tokens: bool,
) -> (
    Option<Box<dyn DynExternalScanner>>,
    Option<ExternalScannerRuntime>,
) {
    if !has_external_tokens {
        return (None, None);
    }

    let registry = get_global_registry();
    let registry = registry.lock().unwrap_or_else(|err| err.into_inner());

    if let Some(scanner) = registry.create_scanner(language_name) {
        let external_tokens: Vec<crate::SymbolId> = grammar
            .externals
            .iter()
            .map(|ext| ext.symbol_id.0)
            .collect();
        let runtime = ExternalScannerRuntime::new(external_tokens);
        (Some(scanner), Some(runtime))
    } else {
        (None, None)
    }
}
