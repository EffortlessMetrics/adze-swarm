//! Error types for the adze build tool.

/// Errors that can occur during grammar parsing and expansion
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// Multiple word rules were specified when only one is allowed
    #[error("multiple word rules specified - only one word rule is allowed per grammar")]
    MultipleWordRules,

    /// Multiple precedence attributes were specified when only one is allowed
    #[error("only one of prec, prec_left, and prec_right can be specified")]
    MultiplePrecedenceAttributes,

    /// Expected a string literal but found something else
    #[error("expected string literal for {context}: {actual}")]
    ExpectedStringLiteral { context: String, actual: String },

    /// Expected an integer literal but found something else
    #[error("expected integer literal for precedence: {actual}")]
    ExpectedIntegerLiteral { actual: String },

    /// Expected a path type but found something else
    #[error("expected a path or unit type: {actual}")]
    ExpectedPathType { actual: String },

    /// Expected a single segment path but found multiple segments
    #[error("expected a single segment path: {actual}")]
    ExpectedSingleSegmentPath { actual: String },

    /// Nested Option types are not supported
    #[error("Option<Option<_>> is not supported")]
    NestedOptionType,

    /// Struct has no non-skipped fields
    #[error("struct {name} has no non-skipped fields")]
    StructHasNoFields { name: String },

    /// Complex symbols should be normalized before processing
    #[error("complex symbols should be normalized before {operation}")]
    ComplexSymbolsNotNormalized { operation: String },

    /// Expected a specific symbol type but found something else
    #[error("expected {expected} symbol")]
    ExpectedSymbolType { expected: String },

    /// Expected a specific action type but found something else
    #[error("expected {expected} action")]
    ExpectedActionType { expected: String },

    /// Expected a specific error type but found something else
    #[error("expected {expected} error")]
    ExpectedErrorType { expected: String },

    /// String too long for extraction
    #[error("string too long for {operation}: length {length} exceeds maximum")]
    StringTooLong { operation: String, length: usize },

    /// Invalid production rule
    #[error("invalid production rule: {details}")]
    InvalidProduction { details: String },

    /// Grammar validation failed
    #[error("grammar validation failed: {reason}")]
    GrammarValidation { reason: String },

    /// Other tool error with custom message
    #[error("{0}")]
    Other(String),

    /// IO error occurred during file operations
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Error from the IR layer
    #[error(transparent)]
    Ir(#[from] adze_ir::IrError),

    /// Error from the GLR core
    #[error(transparent)]
    Glr(#[from] adze_glr_core::GLRError),

    /// Error from table generation
    #[error(transparent)]
    TableGen(#[from] adze_tablegen::TableGenError),

    /// Syn parsing error
    #[error(transparent)]
    SynError {
        #[from]
        syn_error: syn::Error,
    },
}

/// Convenience type alias for tool results
pub type Result<T> = std::result::Result<T, ToolError>;

impl From<String> for ToolError {
    fn from(s: String) -> Self {
        ToolError::Other(s)
    }
}

impl From<&str> for ToolError {
    fn from(s: &str) -> Self {
        ToolError::Other(s.to_string())
    }
}

impl ToolError {
    /// Create a string too long error
    pub fn string_too_long(operation: &str, length: usize) -> Self {
        ToolError::StringTooLong {
            operation: operation.to_string(),
            length,
        }
    }

    /// Create a complex symbols error
    pub fn complex_symbols_not_normalized(operation: &str) -> Self {
        ToolError::ComplexSymbolsNotNormalized {
            operation: operation.to_string(),
        }
    }

    /// Create an expected symbol type error
    pub fn expected_symbol_type(expected: &str) -> Self {
        ToolError::ExpectedSymbolType {
            expected: expected.to_string(),
        }
    }

    /// Create an expected action type error
    pub fn expected_action_type(expected: &str) -> Self {
        ToolError::ExpectedActionType {
            expected: expected.to_string(),
        }
    }

    /// Create an expected error type error
    pub fn expected_error_type(expected: &str) -> Self {
        ToolError::ExpectedErrorType {
            expected: expected.to_string(),
        }
    }

    /// Create a grammar validation error
    pub fn grammar_validation(reason: &str) -> Self {
        ToolError::GrammarValidation {
            reason: reason.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_multiple_word_rules_is_static() {
        let err = ToolError::MultipleWordRules;
        assert_eq!(
            err.to_string(),
            "multiple word rules specified - only one word rule is allowed per grammar"
        );
    }

    #[test]
    fn display_multiple_precedence_attributes_is_static() {
        let err = ToolError::MultiplePrecedenceAttributes;
        assert_eq!(
            err.to_string(),
            "only one of prec, prec_left, and prec_right can be specified"
        );
    }

    #[test]
    fn display_expected_string_literal_includes_context_and_actual() {
        let err = ToolError::ExpectedStringLiteral {
            context: "token name".into(),
            actual: "42".into(),
        };
        assert_eq!(
            err.to_string(),
            "expected string literal for token name: 42"
        );
    }

    #[test]
    fn display_expected_integer_literal_includes_actual() {
        let err = ToolError::ExpectedIntegerLiteral {
            actual: "\"abc\"".into(),
        };
        assert_eq!(
            err.to_string(),
            "expected integer literal for precedence: \"abc\""
        );
    }

    #[test]
    fn display_expected_path_type_includes_actual() {
        let err = ToolError::ExpectedPathType {
            actual: "tuple".into(),
        };
        assert_eq!(err.to_string(), "expected a path or unit type: tuple");
    }

    #[test]
    fn display_expected_single_segment_path_includes_actual() {
        let err = ToolError::ExpectedSingleSegmentPath {
            actual: "foo::bar".into(),
        };
        assert_eq!(err.to_string(), "expected a single segment path: foo::bar");
    }

    #[test]
    fn display_nested_option_type_is_static() {
        let err = ToolError::NestedOptionType;
        assert_eq!(err.to_string(), "Option<Option<_>> is not supported");
    }

    #[test]
    fn display_struct_has_no_fields_includes_name() {
        let err = ToolError::StructHasNoFields {
            name: "Empty".into(),
        };
        assert_eq!(err.to_string(), "struct Empty has no non-skipped fields");
    }

    #[test]
    fn display_invalid_production_includes_details() {
        let err = ToolError::InvalidProduction {
            details: "missing lhs".into(),
        };
        assert_eq!(err.to_string(), "invalid production rule: missing lhs");
    }

    #[test]
    fn display_other_includes_message() {
        let err = ToolError::Other("boom".into());
        assert_eq!(err.to_string(), "boom");
    }

    #[test]
    fn string_too_long_builds_variant_with_fields() {
        let err = ToolError::string_too_long("tokenize", 9001);
        match err {
            ToolError::StringTooLong { operation, length } => {
                assert_eq!(operation, "tokenize");
                assert_eq!(length, 9001);
            }
            other => panic!("expected StringTooLong, got {:?}", other),
        }
    }

    #[test]
    fn string_too_long_display_includes_operation_and_length() {
        let err = ToolError::string_too_long("compress", 128);
        assert_eq!(
            err.to_string(),
            "string too long for compress: length 128 exceeds maximum"
        );
    }

    #[test]
    fn complex_symbols_not_normalized_builds_variant_with_operation() {
        let err = ToolError::complex_symbols_not_normalized("expansion");
        match err {
            ToolError::ComplexSymbolsNotNormalized { operation } => {
                assert_eq!(operation, "expansion");
            }
            other => panic!("expected ComplexSymbolsNotNormalized, got {:?}", other),
        }
    }

    #[test]
    fn complex_symbols_not_normalized_display_includes_operation() {
        let err = ToolError::complex_symbols_not_normalized("flattening");
        assert_eq!(
            err.to_string(),
            "complex symbols should be normalized before flattening"
        );
    }

    #[test]
    fn expected_symbol_type_builds_variant_with_expected() {
        let err = ToolError::expected_symbol_type("terminal");
        match err {
            ToolError::ExpectedSymbolType { expected } => {
                assert_eq!(expected, "terminal");
            }
            other => panic!("expected ExpectedSymbolType, got {:?}", other),
        }
    }

    #[test]
    fn expected_symbol_type_display_includes_expected() {
        let err = ToolError::expected_symbol_type("non-terminal");
        assert_eq!(err.to_string(), "expected non-terminal symbol");
    }

    #[test]
    fn expected_action_type_builds_variant_with_expected() {
        let err = ToolError::expected_action_type("shift");
        match err {
            ToolError::ExpectedActionType { expected } => {
                assert_eq!(expected, "shift");
            }
            other => panic!("expected ExpectedActionType, got {:?}", other),
        }
    }

    #[test]
    fn expected_action_type_display_includes_expected() {
        let err = ToolError::expected_action_type("reduce");
        assert_eq!(err.to_string(), "expected reduce action");
    }

    #[test]
    fn expected_error_type_builds_variant_with_expected() {
        let err = ToolError::expected_error_type("parse");
        match err {
            ToolError::ExpectedErrorType { expected } => {
                assert_eq!(expected, "parse");
            }
            other => panic!("expected ExpectedErrorType, got {:?}", other),
        }
    }

    #[test]
    fn expected_error_type_display_includes_expected() {
        let err = ToolError::expected_error_type("io");
        assert_eq!(err.to_string(), "expected io error");
    }

    #[test]
    fn grammar_validation_builds_variant_with_reason() {
        let err = ToolError::grammar_validation("duplicate rule");
        match err {
            ToolError::GrammarValidation { reason } => {
                assert_eq!(reason, "duplicate rule");
            }
            other => panic!("expected GrammarValidation, got {:?}", other),
        }
    }

    #[test]
    fn grammar_validation_display_includes_reason() {
        let err = ToolError::grammar_validation("undefined symbol");
        assert_eq!(
            err.to_string(),
            "grammar validation failed: undefined symbol"
        );
    }

    #[test]
    fn from_string_routes_to_other() {
        let err: ToolError = String::from("boom").into();
        match err {
            ToolError::Other(msg) => assert_eq!(msg, "boom"),
            other => panic!("expected Other, got {:?}", other),
        }
    }

    #[test]
    fn from_str_routes_to_other() {
        let err: ToolError = "fizz".into();
        match err {
            ToolError::Other(msg) => assert_eq!(msg, "fizz"),
            other => panic!("expected Other, got {:?}", other),
        }
    }

    #[test]
    fn from_io_error_preserves_kind() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: ToolError = io.into();
        match err {
            ToolError::Io(inner) => {
                assert_eq!(inner.kind(), std::io::ErrorKind::NotFound);
            }
            other => panic!("expected Io, got {:?}", other),
        }
    }

    #[test]
    fn from_json_error_routes_to_json_variant() {
        let json_err = serde_json::from_str::<serde_json::Value>("{not json}").unwrap_err();
        let err: ToolError = json_err.into();
        assert!(matches!(err, ToolError::Json(_)));
    }
}
