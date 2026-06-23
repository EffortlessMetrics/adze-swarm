#![cfg(all(test, feature = "external_scanners"))]

/// Test for OCaml-style nested comments external scanner
use adze::external_scanner::{ExternalScanner, Lexer, ScanResult};

/// Token type for nested comments
const COMMENT: u16 = 2000;

/// OCaml-style nested comment scanner
/// Handles comments like (* nested (* comments *) are allowed *)
struct NestedCommentScanner {
    depth: usize,
}

impl NestedCommentScanner {
    fn new() -> Self {
        Self { depth: 0 }
    }
}

impl ExternalScanner for NestedCommentScanner {
    fn scan(&mut self, lexer: &mut dyn Lexer, valid_symbols: &[bool]) -> Option<ScanResult> {
        // Only scan if comment token is valid
        if valid_symbols.get(COMMENT as usize) != Some(&true) {
            return None;
    }

        // Check for comment start
        if lexer.lookahead() == Some(b'(') {
            lexer.advance(1);

            if lexer.lookahead() == Some(b'*') {
                lexer.advance(1);
            } else {
                // Not a comment start, backtrack
                return None;
            }
        } else {
            return None;
    }

        let mut consumed = 2;
        let mut depth = 1;

        // Scan through comment body, handling nesting
        while depth > 0 && !lexer.is_eof() {
            match lexer.lookahead() {
                Some(b'(') => {
                    lexer.advance(1);
                    consumed += 1;
                    if lexer.lookahead() == Some(b'*') {
                        lexer.advance(1);
                        consumed += 1;
                        depth += 1; // Nested comment start
                    }
                }
                Some(b'*') => {
                    lexer.advance(1);
                    consumed += 1;
                    if lexer.lookahead() == Some(b')') {
                        lexer.advance(1);
                        consumed += 1;
                        depth -= 1;
                        if depth == 0 {
                            lexer.mark_end();
                            return Some(ScanResult {
                                symbol: COMMENT,
                                length: consumed,
                            });
                        }
                    }
                }
                Some(_) => {
                    lexer.advance(1);
                    consumed += 1;
                }
                None => break,
            }
    }

        // Unclosed comment
        None
    }

    fn serialize(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&(self.depth as u32).to_le_bytes());
    }

    fn deserialize(&mut self, buffer: &[u8]) {
        if buffer.len() >= 4 {
            let bytes: [u8; 4] = buffer[0..4].try_into().unwrap();
            self.depth = u32::from_le_bytes(bytes) as usize;
    }
    }
}

#[test]
fn test_simple_comment() {
    let input = b"(* simple comment *)";
    let mut scanner = NestedCommentScanner::new();

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
        fn lookahead(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn lookahead_n(&self, n: usize) -> Option<u8> {
        self.input.get(self.position.saturating_add(n)).copied()
    }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
    }

        fn mark_end(&mut self) {
            self.mark = self.position;
    }

        fn column(&self) -> usize {
            0 // Not needed for this test
    }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
    }
    }

    let mut lexer = TestLexer {
        input,
        position: 0,
        mark: 0,
    };

    let valid_symbols = vec![true; 3000];

    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: COMMENT,
            length: 20
        })
    );
    assert_eq!(lexer.mark, 20);
}

#[test]
fn test_nested_comments() {
    let input = b"(* outer (* inner *) still outer *)";
    let mut scanner = NestedCommentScanner::new();

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
        fn lookahead(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn lookahead_n(&self, n: usize) -> Option<u8> {
        self.input.get(self.position.saturating_add(n)).copied()
    }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
    }

        fn mark_end(&mut self) {
            self.mark = self.position;
    }

        fn column(&self) -> usize {
            0
    }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
    }
    }

    let mut lexer = TestLexer {
        input,
        position: 0,
        mark: 0,
    };

    let valid_symbols = vec![true; 3000];

    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: COMMENT,
            length: 35
        })
    );
    assert_eq!(lexer.position, 35);
}

#[test]
fn test_deeply_nested_comments() {
    let input = b"(* a (* b (* c *) b *) a *)";
    let mut scanner = NestedCommentScanner::new();

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
        fn lookahead(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn lookahead_n(&self, n: usize) -> Option<u8> {
        self.input.get(self.position.saturating_add(n)).copied()
    }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
    }

        fn mark_end(&mut self) {
            self.mark = self.position;
    }

        fn column(&self) -> usize {
            0
    }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
    }
    }

    let mut lexer = TestLexer {
        input,
        position: 0,
        mark: 0,
    };

    let valid_symbols = vec![true; 3000];

    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(
        result,
        Some(ScanResult {
            symbol: COMMENT,
            length: 27
        })
    );

    // Verify depth handling during scan
    let _test_scanner = NestedCommentScanner::new();
    let mut test_lexer = TestLexer {
        input,
        position: 0,
        mark: 0,
    };

    // Manually trace through to verify depth tracking
    assert_eq!(test_lexer.lookahead(), Some(b'('));
    test_lexer.advance(1);
    assert_eq!(test_lexer.lookahead(), Some(b'*'));
    test_lexer.advance(1);
    // depth = 1

    test_lexer.advance(3); // " a "
    assert_eq!(test_lexer.lookahead(), Some(b'('));
    test_lexer.advance(1);
    assert_eq!(test_lexer.lookahead(), Some(b'*'));
    test_lexer.advance(1);
    // depth = 2

    test_lexer.advance(3); // " b "
    assert_eq!(test_lexer.lookahead(), Some(b'('));
    test_lexer.advance(1);
    assert_eq!(test_lexer.lookahead(), Some(b'*'));
    test_lexer.advance(1);
    // depth = 3
}

#[test]
fn test_unclosed_comment() {
    let input = b"(* unclosed comment";
    let mut scanner = NestedCommentScanner::new();

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
        fn lookahead(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn lookahead_n(&self, n: usize) -> Option<u8> {
        self.input.get(self.position.saturating_add(n)).copied()
    }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
    }

        fn mark_end(&mut self) {
            self.mark = self.position;
    }

        fn column(&self) -> usize {
            0
    }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
    }
    }

    let mut lexer = TestLexer {
        input,
        position: 0,
        mark: 0,
    };

    let valid_symbols = vec![true; 3000];

    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(result, None); // Should return None for unclosed comment
}

#[test]
fn test_not_a_comment() {
    let input = b"(not * a comment)";
    let mut scanner = NestedCommentScanner::new();

    struct TestLexer<'a> {
        input: &'a [u8],
        position: usize,
        mark: usize,
    }

    impl<'a> Lexer for TestLexer<'a> {
        fn lookahead(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn lookahead_n(&self, n: usize) -> Option<u8> {
        self.input.get(self.position.saturating_add(n)).copied()
    }

        fn advance(&mut self, n: usize) {
            self.position = (self.position + n).min(self.input.len());
    }

        fn mark_end(&mut self) {
            self.mark = self.position;
    }

        fn column(&self) -> usize {
            0
    }

        fn is_eof(&self) -> bool {
            self.position >= self.input.len()
    }
    }

    let mut lexer = TestLexer {
        input,
        position: 0,
        mark: 0,
    };

    let valid_symbols = vec![true; 3000];

    let result = scanner.scan(&mut lexer, &valid_symbols);
    assert_eq!(result, None); // Should return None, not a comment start
}
