//! Error types for table generation and compression.

/// Errors produced by table generation and compression.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum TableGenError {
    /// Invalid input was provided to a table generation function.
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),

    /// Automaton construction failed during table generation.
    #[error("automaton build failed: {0}")]
    Automaton(String),

    /// Table compression algorithm encountered an error.
    #[error("compression failed: {0}")]
    Compression(String),

    /// General table generation failure, often from upstream errors.
    #[error("table generation failed: {0}")]
    TableGeneration(String),

    /// The table structure is invalid or corrupted.
    #[error("invalid table structure: {0}")]
    InvalidTable(String),

    /// Symbol index is out of bounds for the grammar.
    #[error("symbol index out of bounds: {0}")]
    InvalidSymbolIndex(usize),

    /// State index is out of bounds for the parse table.
    #[error("state index out of bounds: {0}")]
    InvalidStateIndex(usize),

    /// The grammar is empty and cannot be processed.
    #[error("empty grammar")]
    EmptyGrammar,

    /// Grammar validation failed before table generation.
    #[error("grammar validation failed: {0}")]
    ValidationError(String),

    /// I/O error occurred during file operations.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Error bubbled from the GLR automaton builder.
    #[error(transparent)]
    Glr(#[from] adze_glr_core::GLRError),

    /// Error bubbled from the IR layer.
    #[error(transparent)]
    Ir(#[from] adze_ir::IrError),
}

/// Convenience type alias for TableGen results.
pub type Result<T> = std::result::Result<T, TableGenError>;

impl From<String> for TableGenError {
    fn from(s: String) -> Self {
        TableGenError::TableGeneration(s)
    }
}

impl From<&str> for TableGenError {
    fn from(s: &str) -> Self {
        TableGenError::TableGeneration(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_invalid_input_includes_message() {
        let err = TableGenError::InvalidInput("missing field");
        assert_eq!(err.to_string(), "invalid input: missing field");
    }

    #[test]
    fn display_automaton_includes_message() {
        let err = TableGenError::Automaton("state explosion".into());
        assert_eq!(err.to_string(), "automaton build failed: state explosion");
    }

    #[test]
    fn display_compression_includes_message() {
        let err = TableGenError::Compression("overflow".into());
        assert_eq!(err.to_string(), "compression failed: overflow");
    }

    #[test]
    fn display_table_generation_includes_message() {
        let err = TableGenError::TableGeneration("downstream".into());
        assert_eq!(err.to_string(), "table generation failed: downstream");
    }

    #[test]
    fn display_invalid_table_includes_message() {
        let err = TableGenError::InvalidTable("bad row offset".into());
        assert_eq!(err.to_string(), "invalid table structure: bad row offset");
    }

    #[test]
    fn display_invalid_symbol_index_includes_value() {
        let err = TableGenError::InvalidSymbolIndex(42);
        assert_eq!(err.to_string(), "symbol index out of bounds: 42");
    }

    #[test]
    fn display_invalid_state_index_includes_value() {
        let err = TableGenError::InvalidStateIndex(7);
        assert_eq!(err.to_string(), "state index out of bounds: 7");
    }

    #[test]
    fn display_empty_grammar_is_static() {
        let err = TableGenError::EmptyGrammar;
        assert_eq!(err.to_string(), "empty grammar");
    }

    #[test]
    fn display_validation_error_includes_message() {
        let err = TableGenError::ValidationError("undefined symbol".into());
        assert_eq!(
            err.to_string(),
            "grammar validation failed: undefined symbol"
        );
    }

    #[test]
    fn from_string_routes_to_table_generation() {
        let err: TableGenError = String::from("boom").into();
        match err {
            TableGenError::TableGeneration(msg) => assert_eq!(msg, "boom"),
            other => panic!("expected TableGeneration, got {:?}", other),
        }
    }

    #[test]
    fn from_str_routes_to_table_generation() {
        let err: TableGenError = "fizz".into();
        match err {
            TableGenError::TableGeneration(msg) => assert_eq!(msg, "fizz"),
            other => panic!("expected TableGeneration, got {:?}", other),
        }
    }

    #[test]
    fn from_io_error_preserves_kind() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: TableGenError = io.into();
        match err {
            TableGenError::Io(inner) => {
                assert_eq!(inner.kind(), std::io::ErrorKind::NotFound);
            }
            other => panic!("expected Io, got {:?}", other),
        }
    }

    #[test]
    fn from_json_error_carries_inner_message() {
        let json_err = serde_json::from_str::<serde_json::Value>("{not json}").unwrap_err();
        let err: TableGenError = json_err.into();
        assert!(matches!(err, TableGenError::Json(_)));
    }
}
