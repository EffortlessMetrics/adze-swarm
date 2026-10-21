//! Formatting helpers for parse diagnostics.

#[cfg(feature = "pure-rust")]
pub(crate) fn diagnostic_symbol_name(raw_name: String) -> String {
    // Generated regex tokens are named by their pattern, wrapped in slashes
    // (with a leading `_` for hidden tokens). Map the common exact patterns
    // to stable human-readable names; every other pattern keeps the raw
    // `/pattern/` rendering so unfamiliar tokens stay precise.
    let pattern = raw_name
        .strip_prefix("_/")
        .or_else(|| raw_name.strip_prefix('/'))
        .and_then(|rest| rest.strip_suffix('/'));
    if let Some(pattern) = pattern {
        return friendly_pattern_name(pattern)
            .map(str::to_string)
            .unwrap_or_else(|| format!("/{pattern}/"));
    }
    raw_name
}

/// Stable display name for an exact regex pattern, when one is unambiguous.
#[cfg(feature = "pure-rust")]
fn friendly_pattern_name(pattern: &str) -> Option<&'static str> {
    match pattern {
        r"\d+" | r"\d" | "[0-9]+" | "[0-9]" => Some("number"),
        "[a-zA-Z_][a-zA-Z0-9_]*"
        | "[a-zA-Z_][a-zA-Z0-9_]+"
        | "[a-z]+"
        | "[a-z_]+"
        | "[A-Za-z_][A-Za-z0-9_]*" => Some("identifier"),
        r#""([^"\\]|\\.)*""# | "'([^'\\\\]|\\\\.)*'" => Some("string"),
        r"\s+" | r"\s" => Some("whitespace"),
        _ => None,
    }
}

#[cfg(all(test, feature = "pure-rust"))]
mod tests {
    use super::diagnostic_symbol_name;

    #[test]
    fn test_diagnostic_symbol_name_when_digit_pattern_number() {
        assert_eq!(diagnostic_symbol_name(r"/\d+/".into()), "number");
        assert_eq!(diagnostic_symbol_name(r"/[0-9]+/".into()), "number");
        assert_eq!(diagnostic_symbol_name(r"_/\d+/".into()), "number");
    }

    #[test]
    fn test_diagnostic_symbol_name_when_identifier_pattern_identifier() {
        assert_eq!(
            diagnostic_symbol_name(r"/[a-zA-Z_][a-zA-Z0-9_]*/".into()),
            "identifier"
        );
        assert_eq!(diagnostic_symbol_name(r"/[a-z_]+/".into()), "identifier");
    }

    #[test]
    fn test_diagnostic_symbol_name_when_string_pattern_string() {
        assert_eq!(
            diagnostic_symbol_name(r#"/"([^"\\]|\\.)*"/"#.into()),
            "string"
        );
    }

    #[test]
    fn test_diagnostic_symbol_name_when_unknown_pattern_keeps_slashes() {
        assert_eq!(
            diagnostic_symbol_name(r"/[0-9a-f]+/".into()),
            r"/[0-9a-f]+/"
        );
        assert_eq!(
            diagnostic_symbol_name(r"_/[0-9a-f]+/".into()),
            r"/[0-9a-f]+/"
        );
    }

    #[test]
    fn test_diagnostic_symbol_name_when_plain_name_unchanged() {
        assert_eq!(diagnostic_symbol_name("+".into()), "+");
        assert_eq!(diagnostic_symbol_name("end".into()), "end");
        assert_eq!(diagnostic_symbol_name("identifier".into()), "identifier");
    }
}

#[cfg(feature = "pure-rust")]
pub(crate) fn unexpected_token_message(found: String, expected: Vec<String>) -> String {
    if expected.is_empty() {
        found
    } else {
        format!("{found}; expected one of: {}", expected.join(", "))
    }
}
