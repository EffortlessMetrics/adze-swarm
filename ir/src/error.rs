//! Error types for grammar IR operations.

/// Errors that can occur while building and validating the IR.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum IrError {
    /// The referenced symbol was malformed or not present in the grammar.
    #[error("invalid symbol: {0}")]
    InvalidSymbol(String),

    /// Attempted to insert a rule that already exists.
    #[error("duplicate rule: {0}")]
    DuplicateRule(String),

    /// An unexpected internal IR failure.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience type alias for IR results.
pub type Result<T> = std::result::Result<T, IrError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_invalid_symbol_uses_format_string() {
        let err = IrError::InvalidSymbol("foo".into());
        assert_eq!(format!("{err}"), "invalid symbol: foo");
    }

    #[test]
    fn display_duplicate_rule_uses_format_string() {
        let err = IrError::DuplicateRule("bar".into());
        assert_eq!(format!("{err}"), "duplicate rule: bar");
    }

    #[test]
    fn display_internal_uses_format_string() {
        let err = IrError::Internal("boom".into());
        assert_eq!(format!("{err}"), "internal error: boom");
    }

    #[test]
    fn debug_output_is_non_empty_and_names_variant() {
        let err = IrError::InvalidSymbol("foo".into());
        let dbg = format!("{err:?}");
        assert!(!dbg.is_empty());
        assert!(dbg.contains("InvalidSymbol"));

        let err = IrError::DuplicateRule("bar".into());
        assert!(format!("{err:?}").contains("DuplicateRule"));

        let err = IrError::Internal("boom".into());
        assert!(format!("{err:?}").contains("Internal"));
    }

    #[test]
    fn result_alias_resolves_to_std_result_with_ir_error() {
        let value: Result<()> = Err(IrError::Internal("x".into()));
        // The alias must also satisfy the equivalent fully-qualified type.
        let _check: std::result::Result<(), IrError> = value;
    }

    #[test]
    fn implements_std_error_trait() {
        let err = IrError::Internal("inner".into());
        let as_err: &dyn std::error::Error = &err;
        // `source()` should not panic; for these leaf variants it is None.
        assert!(as_err.source().is_none());
        // `to_string()` goes through Display via the Error trait object.
        assert_eq!(as_err.to_string(), "internal error: inner");
    }

    #[test]
    fn ir_error_is_send_and_sync() {
        fn _assert<T: Send + Sync>() {}
        _assert::<IrError>();
    }
}
