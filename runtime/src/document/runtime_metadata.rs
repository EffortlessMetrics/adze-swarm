use crate::pure_parser::TSLanguage;
use adze_glr_core::ParseTable;
use adze_ir::Grammar;

/// Runtime parse context used to construct an [`super::AdzeDocument`].
pub(crate) struct DocumentRuntime<'a> {
    pub(crate) language_name: &'a str,
    pub(crate) grammar: &'a Grammar,
    pub(crate) parse_table: &'a ParseTable,
    pub(crate) pure_language: Option<&'static TSLanguage>,
}

/// Reason an incremental parse request fell back to a full reparse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncrementalFallbackReason {
    /// The document API can currently expose the requested lifecycle, but no
    /// document-level reuse path is implemented for this parser path yet.
    FullReparseOnly,
    /// There was no trustworthy previous document or forest to reuse.
    MissingOldDocument,
    /// The supplied edit shape is not supported by the incremental path.
    UnsupportedEdit,
    /// The parser/runtime path does not support incremental reuse.
    UnsupportedParser,
}

impl IncrementalFallbackReason {
    /// Return the stable metadata string for this fallback reason.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FullReparseOnly => "full_reparse_only",
            Self::MissingOldDocument => "missing_old_document",
            Self::UnsupportedEdit => "unsupported_edit",
            Self::UnsupportedParser => "unsupported_parser",
        }
    }
}

/// Basic parse metadata for a native document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseMetadata {
    /// Number of parser recovery/error events recorded for this parse.
    pub error_count: usize,
    /// Whether a caller requested incremental parsing for this document.
    pub incremental_requested: bool,
    /// Whether an incremental reuse path actually produced this document.
    pub incremental_used: bool,
    /// Reason an incremental request fell back to a full reparse.
    pub fallback_reason: Option<IncrementalFallbackReason>,
}

impl ParseMetadata {
    /// Build metadata for an ordinary non-incremental parse.
    #[must_use]
    pub fn new(error_count: usize) -> Self {
        Self {
            error_count,
            incremental_requested: false,
            incremental_used: false,
            fallback_reason: None,
        }
    }

    /// Return whether this document records a full-reparse fallback.
    #[must_use]
    pub fn full_reparse_fallback(&self) -> bool {
        self.incremental_requested && !self.incremental_used && self.fallback_reason.is_some()
    }
}
