// runtime/examples/external_scanner_demo.rs
use std::panic::{self, AssertUnwindSafe};

use adze_python::grammar_python::LANGUAGE as PY_LANGUAGE;

// Minimal inline surface for the deleted legacy `pure_external_scanner`
// module (production scanners live in `adze::scanners` + streaming paths).
// Kept only so this demo still compiles.
pub trait ExternalScanner: Send + Sync {
    fn scan(&mut self, lexer: &mut Lexer, valid_symbols: &[bool]) -> bool;

    fn serialize(&self, _buffer: &mut [u8]) -> usize {
        0
    }

    fn deserialize(&mut self, _buffer: &[u8]) {}
}

pub struct Lexer<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a [u8], position: usize) -> Self {
        Lexer { input, position }
    }
}

pub struct ExternalScannerRegistry {
    scanners: Vec<(String, Box<dyn ExternalScanner>)>,
}

impl Default for ExternalScannerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalScannerRegistry {
    pub fn new() -> Self {
        ExternalScannerRegistry {
            scanners: Vec::new(),
        }
    }

    pub fn register(&mut self, name: String, scanner: Box<dyn ExternalScanner>) {
        self.scanners.push((name, scanner));
    }
}

#[derive(Default)]
struct PythonStringsScanner;

impl ExternalScanner for PythonStringsScanner {
    fn scan(&mut self, _lexer: &mut Lexer, _valid_symbols: &[bool]) -> bool {
        false
    }

    // If your trait requires these (some builds do), keep them; otherwise, remove.
    fn serialize(&self, _buf: &mut [u8]) -> usize {
        0
    }
    fn deserialize(&mut self, _buf: &[u8]) {}
}

fn main() {
    // Enable backtrace for debugging - safe in examples
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    println!("A: about to take &PY_LANGUAGE");
    let res_a = panic::catch_unwind(|| {
        let _ = &PY_LANGUAGE;
    });
    if let Err(e) = res_a {
        eprintln!("PANIC at A (&PY_LANGUAGE): {e:?}");
        return;
    }
    println!("A✓");

    println!("B: creating registry");
    let mut registry = match panic::catch_unwind(|| ExternalScannerRegistry::default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("PANIC at B (registry::default): {e:?}");
            return;
        }
    };
    println!("B✓");

    println!("C: registering scanner as \"python_strings\"");
    let res_c = panic::catch_unwind(AssertUnwindSafe(|| {
        registry.register(
            "python_strings".to_string(),
            Box::new(PythonStringsScanner::default()),
        );
    }));
    if let Err(e) = res_c {
        eprintln!("PANIC at C (registry::register): {e:?}");
        return;
    }
    println!("C✓");

    println!("D: done. exiting normally");
}
