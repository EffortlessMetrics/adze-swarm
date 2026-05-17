//! LSP feature implementations for adze grammars.

mod completion;
mod diagnostics;
mod hover;

pub use completion::CompletionProvider;
pub use diagnostics::DiagnosticsProvider;
pub use hover::HoverProvider;

/// Trait for LSP features.
pub trait LspFeature: Send + Sync {
    /// Get the name of this feature.
    fn name(&self) -> &str;

    /// Generate handler code for this feature.
    fn generate_handler(&self) -> String;

    /// Get required imports for this feature.
    fn required_imports(&self) -> Vec<String>;

    /// Get capabilities for this feature.
    fn capabilities(&self) -> serde_json::Value;
}
