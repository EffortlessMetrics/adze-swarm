#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);

    if input.len() > 32 * 1024 {
        return;
    }

    // Rust tokenization should never panic on arbitrary UTF-8.
    let tokens = input.parse::<proc_macro2::TokenStream>();

    // If tokenization succeeded, token rendering should remain tokenizable.
    if let Ok(stream) = tokens {
        let rendered = stream.to_string();
        let reparsed = rendered
            .parse::<proc_macro2::TokenStream>()
            .expect("rendered token stream should parse as tokens");

        // Basic stability invariant: token count should be preserved.
        let original_count = stream.clone().into_iter().count();
        let reparsed_count = reparsed.into_iter().count();
        assert_eq!(original_count, reparsed_count);
    }
});
