#![cfg_attr(feature = "strict_docs", allow(missing_docs))]
//! Pure-Rust parser implementation using compressed parse tables.

// Pure-Rust parser implementation using compressed tables
// This implements Tree-sitter's parsing algorithm with GLR support

use crate::abi::*;

/// A parser state consisting of the current state ID and lookahead symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseState {
    /// Current parser state index.
    pub state: u16,
    /// Current lookahead token symbol.
    pub lookahead: u16,
}

/// A node in the parse tree produced by the compressed table parser.
#[derive(Debug, Clone)]
pub struct ParseNode {
    /// Symbol ID of this node.
    pub symbol: u16,
    /// Child nodes.
    pub children: Vec<ParseNode>,
    /// Byte offset where this node starts.
    pub start_byte: usize,
    /// Byte offset where this node ends.
    pub end_byte: usize,
}

/// A parser that drives parsing using compressed parse tables.
pub struct Parser {
    language: &'static TSLanguage,
    stack: Vec<ParseState>,
    nodes: Vec<ParseNode>,
}

impl Parser {
    pub fn new(language: &'static TSLanguage) -> Self {
        Self {
            language,
            stack: vec![ParseState {
                state: 0,
                lookahead: 0,
            }],
            nodes: Vec::new(),
        }
    }

    /// Parse input text using the compressed tables
    pub fn parse(&mut self, input: &str) -> Result<ParseNode, String> {
        let tokens = self.tokenize(input)?;
        let mut position = 0;

        while position < tokens.len() {
            let token = tokens[position];
            let current_state = self
                .stack
                .last()
                .ok_or_else(|| "parser stack is empty".to_string())?
                .state;

            // Look up action in compressed table
            let action = self.get_action(current_state, token.symbol)?;

            match action {
                ParseAction::Shift(state) => {
                    self.stack.push(ParseState {
                        state,
                        lookahead: token.symbol,
                    });
                    self.nodes.push(ParseNode {
                        symbol: token.symbol,
                        children: Vec::new(),
                        start_byte: token.start,
                        end_byte: token.end,
                    });
                    position += 1;
                }
                ParseAction::Reduce(rule_id) => {
                    self.perform_reduction(rule_id)?;
                }
                ParseAction::Accept => {
                    if self.nodes.len() > 1 {
                        return Err("Accept but multiple nodes remain".to_string());
                    }
                    return self
                        .nodes
                        .pop()
                        .ok_or_else(|| "Accept with empty node stack".to_string());
                }
                ParseAction::Error => {
                    return Err(format!("Parse error at position {}", position));
                }
            }
        }

        Err("Unexpected end of input".to_string())
    }

    fn get_action(&self, state: u16, symbol: u16) -> Result<ParseAction, String> {
        // Access compressed parse table
        let parse_table = self.parse_table_slice()?;

        // Decode compressed action
        let table_offset = (state as usize) * 2;
        if table_offset + 1 >= parse_table.len() {
            return Err("State out of bounds".to_string());
        }

        let entry_count = parse_table[table_offset];
        let data_offset = parse_table[table_offset + 1] as usize;

        // Search for symbol in action entries
        for i in 0..entry_count {
            let entry_offset = data_offset + (i as usize) * 2;
            if entry_offset + 1 >= parse_table.len() {
                continue;
            }

            let entry_symbol = parse_table[entry_offset];
            if entry_symbol == symbol {
                let action_data = parse_table[entry_offset + 1];
                return Ok(self.decode_action(action_data));
            }
        }

        // Check default action
        if entry_count > 0 {
            let default_offset = data_offset + (entry_count as usize - 1) * 2 + 1;
            if default_offset < parse_table.len() {
                let default_action = parse_table[default_offset];
                return Ok(self.decode_action(default_action));
            }
        }

        Ok(ParseAction::Error)
    }

    fn decode_action(&self, encoded: u16) -> ParseAction {
        match encoded {
            0xFFFF => ParseAction::Accept,
            0xFFFE => ParseAction::Error,
            _ if encoded & 0x8000 != 0 => {
                let rule_id = (encoded & 0x7FFF) >> 1;
                ParseAction::Reduce(rule_id)
            }
            state => ParseAction::Shift(state),
        }
    }

    fn perform_reduction(&mut self, rule_id: u16) -> Result<(), String> {
        // Get rule info from grammar
        let production_id_map = self.production_id_map_slice()?;

        if rule_id as usize >= production_id_map.len() {
            return Err("Invalid rule ID".to_string());
        }

        // For now, simplified reduction - real implementation needs rule lengths
        // This would come from the grammar IR
        let rule_length = 2; // Placeholder

        // Pop rule_length items from stack
        for _ in 0..rule_length {
            self.stack.pop();
        }

        // Create new node for the reduction
        let mut children = Vec::new();
        for _ in 0..rule_length {
            if let Some(node) = self.nodes.pop() {
                children.push(node);
            }
        }
        children.reverse();

        let start_byte = children.first().map(|n| n.start_byte).unwrap_or(0);
        let end_byte = children.last().map(|n| n.end_byte).unwrap_or(0);

        // Get LHS symbol for the rule (would come from grammar)
        let lhs_symbol = rule_id + self.language.token_count as u16; // Simplified

        self.nodes.push(ParseNode {
            symbol: lhs_symbol,
            children,
            start_byte,
            end_byte,
        });

        // Get goto state
        let current_state = self
            .stack
            .last()
            .ok_or_else(|| "parser stack is empty after reduction".to_string())?
            .state;
        let goto_state = self.get_goto(current_state, lhs_symbol)?;

        self.stack.push(ParseState {
            state: goto_state,
            lookahead: lhs_symbol,
        });

        Ok(())
    }

    fn get_goto(&self, state: u16, _symbol: u16) -> Result<u16, String> {
        // Access small parse table for gotos
        let small_parse_table_map = self.small_parse_table_map_slice()?;

        // Simplified goto lookup - real implementation would decode the compressed goto table
        let map_offset = (state as usize) * 4;
        if map_offset + 3 >= small_parse_table_map.len() {
            return Ok(0); // Default to state 0
        }

        // This is a simplified version - actual implementation needs proper goto decoding
        Ok(state + 1)
    }

    fn tokenize(&self, input: &str) -> Result<Vec<Token>, String> {
        // Simplified tokenizer - real implementation would use tree-sitter lexer
        let mut tokens = Vec::new();
        let _position = 0;

        for (i, ch) in input.chars().enumerate() {
            if ch.is_whitespace() {
                continue;
            }

            // Map characters to token IDs (simplified)
            let symbol = match ch {
                '(' => 1,
                ')' => 2,
                '+' => 3,
                '-' => 4,
                '*' => 5,
                '/' => 6,
                _ if ch.is_ascii_digit() => 7,
                _ => return Err(format!("Unknown character: {}", ch)),
            };

            tokens.push(Token {
                symbol,
                start: i,
                end: i + 1,
            });
        }

        // Add EOF token
        tokens.push(Token {
            symbol: 0,
            start: input.len(),
            end: input.len(),
        });

        Ok(tokens)
    }

    fn parse_table_slice(&self) -> Result<&[u16], String> {
        let len = (self.language.state_count as usize)
            .checked_mul(2)
            .ok_or_else(|| "parse table length overflow".to_string())?;

        if len == 0 {
            return Ok(&[]);
        }
        if self.language.parse_table.is_null() {
            return Err("parse table pointer is null".to_string());
        }

        // SAFETY: Pointer was checked for null, and length was derived from language metadata.
        Ok(unsafe { std::slice::from_raw_parts(self.language.parse_table, len) })
    }

    fn production_id_map_slice(&self) -> Result<&[u16], String> {
        let len = self.language.production_id_count as usize;
        if len == 0 {
            return Ok(&[]);
        }
        if self.language.production_id_map.is_null() {
            return Err("production_id_map pointer is null".to_string());
        }

        // SAFETY: Pointer was checked for null, and length was derived from language metadata.
        Ok(unsafe { std::slice::from_raw_parts(self.language.production_id_map, len) })
    }

    fn small_parse_table_map_slice(&self) -> Result<&[u32], String> {
        let len = (self.language.state_count as usize)
            .checked_mul(4)
            .ok_or_else(|| "small parse table map length overflow".to_string())?;

        if len == 0 {
            return Ok(&[]);
        }
        if self.language.small_parse_table_map.is_null() {
            return Err("small parse table map pointer is null".to_string());
        }

        // SAFETY: Pointer was checked for null, and length was derived from language metadata.
        Ok(unsafe { std::slice::from_raw_parts(self.language.small_parse_table_map, len) })
    }
}

#[derive(Debug, Clone, Copy)]
struct Token {
    symbol: u16,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy)]
enum ParseAction {
    Shift(u16),
    Reduce(u16),
    Accept,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn null_language() -> TSLanguage {
        TSLanguage {
            version: 0,
            symbol_count: 0,
            alias_count: 0,
            token_count: 0,
            external_token_count: 0,
            state_count: 0,
            large_state_count: 0,
            production_id_count: 0,
            field_count: 0,
            max_alias_sequence_length: 0,
            production_id_map: std::ptr::null(),
            parse_table: std::ptr::null(),
            small_parse_table: std::ptr::null(),
            small_parse_table_map: std::ptr::null(),
            parse_actions: std::ptr::null(),
            symbol_names: std::ptr::null(),
            field_names: std::ptr::null(),
            field_map_slices: std::ptr::null(),
            field_map_entries: std::ptr::null(),
            symbol_metadata: std::ptr::null(),
            public_symbol_map: std::ptr::null(),
            alias_map: std::ptr::null(),
            alias_sequences: std::ptr::null(),
            lex_modes: std::ptr::null(),
            lex_fn: None,
            keyword_lex_fn: None,
            keyword_capture_token: TSSymbol(0),
            external_scanner: ExternalScanner::default(),
            primary_state_ids: std::ptr::null(),
            production_lhs_index: std::ptr::null(),
            production_count: 0,
            eof_symbol: 0,
        }
    }

    #[test]
    fn test_decode_action() {
        // Create a dummy language for testing
        let lang = null_language();

        // For testing, we'll use unsafe to extend the lifetime
        // SAFETY: `lang` is stack-local and lives for the rest of this scope.
        // We create a pointer and immediately re-borrow it as `&'static` to
        // satisfy `Parser::new`. This is sound only because `parser` does not
        // escape this function.
        let parser = unsafe {
            let lang_ptr = &lang as *const TSLanguage;
            Parser::new(&*lang_ptr)
        };

        // Test shift action
        assert!(matches!(parser.decode_action(42), ParseAction::Shift(42)));

        // Test reduce action
        assert!(matches!(
            parser.decode_action(0x8002),
            ParseAction::Reduce(1)
        ));

        // Test accept
        assert!(matches!(parser.decode_action(0xFFFF), ParseAction::Accept));

        // Test error
        assert!(matches!(parser.decode_action(0xFFFE), ParseAction::Error));
    }

    #[test]
    fn test_parse_reports_null_parse_table_pointer() {
        let mut lang = null_language();
        lang.state_count = 1;

        // SAFETY: `lang` outlives `parser` in this test scope.
        let mut parser = unsafe {
            let lang_ptr = &lang as *const TSLanguage;
            Parser::new(&*lang_ptr)
        };

        let err = parser.parse("1").expect_err("expected parse to fail");
        assert!(err.contains("parse table pointer is null"));
    }

    #[test]
    fn test_accept_with_empty_nodes_returns_error() {
        // Slice length is state_count*2; entries must live inside that window.
        // State 0 maps symbol 7 straight to Accept; entries sit at [4, 5].
        static PARSE_TABLE: [u16; 6] = [1, 4, 0, 0, 7, 0xFFFF];
        let mut lang = null_language();
        lang.state_count = 3;
        lang.parse_table = PARSE_TABLE.as_ptr();

        // SAFETY: `lang` outlives `parser` in this test scope; table is static.
        let mut parser = unsafe {
            let lang_ptr = &lang as *const TSLanguage;
            Parser::new(&*lang_ptr)
        };

        let err = parser.parse("1").expect_err("expected parse to fail");
        assert_eq!(err, "Accept with empty node stack");
    }

    #[test]
    fn test_accept_with_single_node_returns_ok() {
        // State 0 shifts symbol 7 to state 1 (entries at [6, 7]);
        // state 1 accepts on EOF (entries at [4, 5]).
        static PARSE_TABLE: [u16; 8] = [1, 6, 1, 4, 0, 0xFFFF, 7, 1];
        let mut lang = null_language();
        lang.state_count = 4;
        lang.parse_table = PARSE_TABLE.as_ptr();

        // SAFETY: `lang` outlives `parser` in this test scope; table is static.
        let mut parser = unsafe {
            let lang_ptr = &lang as *const TSLanguage;
            Parser::new(&*lang_ptr)
        };

        let node = parser.parse("1").expect("expected parse to succeed");
        assert_eq!(node.symbol, 7);
        assert_eq!((node.start_byte, node.end_byte), (0, 1));
    }

    #[test]
    fn test_accept_with_multiple_nodes_preserves_error_and_nodes() {
        // Two numeric shifts followed by Accept without a reduction.
        static PARSE_TABLE: [u16; 12] = [1, 6, 1, 8, 1, 10, 7, 1, 7, 2, 0, 0xFFFF];
        let mut lang = null_language();
        lang.state_count = 6;
        lang.parse_table = PARSE_TABLE.as_ptr();

        // SAFETY: `lang` outlives `parser`; the advertised table is static.
        let mut parser = unsafe {
            let lang_ptr = &lang as *const TSLanguage;
            Parser::new(&*lang_ptr)
        };

        assert_eq!(
            parser.parse("12").unwrap_err(),
            "Accept but multiple nodes remain"
        );
        assert_eq!(parser.nodes.len(), 2);
        assert_eq!(parser.nodes[0].symbol, 7);
        assert_eq!(parser.nodes[1].symbol, 7);
        assert_eq!(
            (parser.nodes[0].start_byte, parser.nodes[0].end_byte),
            (0, 1)
        );
        assert_eq!(
            (parser.nodes[1].start_byte, parser.nodes[1].end_byte),
            (1, 2)
        );
    }
}
