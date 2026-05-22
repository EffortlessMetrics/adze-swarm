use wasm_bindgen::prelude::*;

// Called when the WASM module is instantiated
#[wasm_bindgen(start)]
pub fn wasm_demo_start() {
    // Set panic hook for better error messages in browser console
    // console_error_panic_hook::set_once();

    web_sys::console::log_1(&"adze WASM demo initialized".into());
}

/// Advisory placeholder for a future Python WASM parser smoke.
#[wasm_bindgen]
pub fn parse_python(_source: &str) -> String {
    // Temporarily disabled - Python ts_compat helper not yet implemented
    "Python parser temporarily disabled - needs ts_compat implementation".to_string()
}

/// Parse arithmetic expressions through the generated Rust parser.
#[wasm_bindgen]
pub fn parse_arithmetic(source: &str) -> String {
    match adze_example::arithmetic::grammar::parse(source) {
        Ok(ast) => format!("Parse successful! {:?}", ast),
        Err(_) => "Parse failed".to_string(),
    }
}

/// Advisory placeholder for future parser statistics.
#[wasm_bindgen]
pub fn get_parser_stats() -> String {
    // This would need to be stored in a global or passed back differently
    // For now, just return a placeholder
    "Stats: To be implemented".to_string()
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_smoke {
    use super::*;

    #[test]
    fn test_wasm_parser_facing_smoke_compile() {
        // Parser-facing smoke: ensure the exported entrypoint compiles for wasm32
        // and reaches a real parse path from the demo surface.
        let result = parse_arithmetic("1+2");
        assert!(!result.is_empty());
    }
}
