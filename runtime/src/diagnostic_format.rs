//! Formatting helpers for parse diagnostics.

#[cfg(feature = "pure-rust")]
pub(crate) fn diagnostic_symbol_name(raw_name: String) -> String {
    if raw_name.starts_with("_/") && raw_name.ends_with('/') {
        raw_name[1..].to_string()
    } else {
        raw_name
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
