// Grammar visualization tools for Adze
//! Grammar and parse tree visualization tools.

// This module provides tools to visualize grammars and parse trees

use adze_ir::{Grammar, Symbol, SymbolId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

/// Grammar visualizer that generates various output formats
pub struct GrammarVisualizer {
    grammar: Grammar,
}

impl GrammarVisualizer {
    pub fn new(grammar: Grammar) -> Self {
        Self { grammar }
    }

    /// Generate a Graphviz DOT representation of the grammar
    pub fn to_dot(&self) -> String {
        let mut output = String::new();
        writeln!(&mut output, "digraph Grammar {{").unwrap();
        writeln!(&mut output, "  rankdir=LR;").unwrap();
        writeln!(&mut output, "  node [shape=box];").unwrap();

        // Style for different node types
        writeln!(&mut output, "  // Terminals").unwrap();
        for (id, token) in &self.grammar.tokens {
            let label = self.escape_dot(&token.name);
            writeln!(
                &mut output,
                "  t{} [label=\"{}\" shape=ellipse style=filled fillcolor=lightblue];",
                id.0, label
            )
            .unwrap();
        }

        writeln!(&mut output, "\n  // Non-terminals").unwrap();
        for id in self.grammar.rules.keys() {
            let name = self.get_symbol_name(*id);
            writeln!(
                &mut output,
                "  n{} [label=\"{}\" style=filled fillcolor=lightgreen];",
                id.0,
                self.escape_dot(&name)
            )
            .unwrap();
        }

        writeln!(&mut output, "\n  // External tokens").unwrap();
        for external in &self.grammar.externals {
            writeln!(
                &mut output,
                "  e{} [label=\"{}\" shape=diamond style=filled fillcolor=lightcoral];",
                external.symbol_id.0,
                self.escape_dot(&external.name)
            )
            .unwrap();
        }

        writeln!(&mut output, "\n  // Rules").unwrap();
        for (lhs, rules) in &self.grammar.rules {
            for rule in rules {
                for (i, symbol) in rule.rhs.iter().enumerate() {
                    let from = format!("n{}", lhs.0);
                    let to = match symbol {
                        Symbol::Terminal(id) => format!("t{}", id.0),
                        Symbol::NonTerminal(id) => format!("n{}", id.0),
                        Symbol::External(id) => format!("e{}", id.0),
                        Symbol::Optional(_) => format!("opt{}", i),
                        Symbol::Repeat(_) => format!("rep{}", i),
                        Symbol::RepeatOne(_) => format!("rep1{}", i),
                        Symbol::Choice(_) => format!("choice{}", i),
                        Symbol::Sequence(_) => format!("seq{}", i),
                        Symbol::Epsilon => continue, // Skip epsilon transitions in visualization
                    };

                    let label = if rule.rhs.len() > 1 {
                        format!("{}", i + 1)
                    } else {
                        String::new()
                    };

                    writeln!(&mut output, "  {} -> {} [label=\"{}\"];", from, to, label).unwrap();
                }
            }
        }

        writeln!(&mut output, "}}").unwrap();
        output
    }

    /// Generate a railroad diagram in SVG format
    pub fn to_railroad_svg(&self) -> String {
        let mut output = String::new();
        let width = 800;
        let mut y_offset = 50;

        writeln!(
            &mut output,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="600">"#,
            width
        )
        .unwrap();
        writeln!(&mut output, r#"  <style>"#).unwrap();
        writeln!(
            &mut output,
            r#"    .rule-name {{ font-family: monospace; font-weight: bold; }}"#
        )
        .unwrap();
        writeln!(&mut output, r#"    .terminal {{ fill: #4a90e2; }}"#).unwrap();
        writeln!(&mut output, r#"    .non-terminal {{ fill: #7ed321; }}"#).unwrap();
        writeln!(
            &mut output,
            r#"    .line {{ stroke: #333; stroke-width: 2; fill: none; }}"#
        )
        .unwrap();
        writeln!(&mut output, r#"  </style>"#).unwrap();

        // Draw each rule
        for (lhs, rules) in &self.grammar.rules {
            let rule_name = self.get_symbol_name(*lhs);

            for rule in rules {
                // Rule name
                writeln!(
                    &mut output,
                    r#"  <text x="10" y="{}" class="rule-name">{} ::=</text>"#,
                    y_offset,
                    self.escape_xml(&rule_name)
                )
                .unwrap();

                // Rule diagram
                let mut x_offset = 150;
                for symbol in &rule.rhs {
                    let (text, class) = match symbol {
                        Symbol::Terminal(id) => {
                            let token = self
                                .grammar
                                .tokens
                                .get(id)
                                .map(|t| t.name.clone())
                                .unwrap_or_else(|| format!("T{}", id.0));
                            (token, "terminal")
                        }
                        Symbol::NonTerminal(id) => (self.get_symbol_name(*id), "non-terminal"),
                        Symbol::External(id) => (format!("External{}", id.0), "terminal"),
                        Symbol::Optional(inner) => {
                            (format!("{}?", self.format_symbol_simple(inner)), "optional")
                        }
                        Symbol::Repeat(inner) => {
                            (format!("{}*", self.format_symbol_simple(inner)), "repeat")
                        }
                        Symbol::RepeatOne(inner) => {
                            (format!("{}+", self.format_symbol_simple(inner)), "repeat")
                        }
                        Symbol::Choice(choices) => {
                            let choice_text = choices
                                .iter()
                                .map(|s| self.format_symbol_simple(s))
                                .collect::<Vec<_>>()
                                .join(" | ");
                            (format!("({})", choice_text), "choice")
                        }
                        Symbol::Sequence(seq) => {
                            let seq_text = seq
                                .iter()
                                .map(|s| self.format_symbol_simple(s))
                                .collect::<Vec<_>>()
                                .join(" ");
                            (seq_text, "sequence")
                        }
                        Symbol::Epsilon => ("ε".to_string(), "epsilon"),
                    };

                    let text_width = text.len() * 8 + 20;

                    // Draw box
                    writeln!(&mut output, r#"  <rect x="{}" y="{}" width="{}" height="30" rx="5" class="{}" opacity="0.3"/>"#, 
                    x_offset, y_offset - 15, text_width, class).unwrap();

                    // Draw text
                    writeln!(
                        &mut output,
                        r#"  <text x="{}" y="{}" text-anchor="middle">{}</text>"#,
                        x_offset + text_width / 2,
                        y_offset + 5,
                        self.escape_xml(&text)
                    )
                    .unwrap();

                    // Draw connecting line
                    if x_offset > 150 {
                        writeln!(
                            &mut output,
                            r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" class="line"/>"#,
                            x_offset - 10,
                            y_offset,
                            x_offset,
                            y_offset
                        )
                        .unwrap();
                    }

                    x_offset += text_width + 20;
                }

                y_offset += 60;
            }
        }

        writeln!(&mut output, "</svg>").unwrap();
        output
    }

    /// Generate a textual representation of the grammar
    pub fn to_text(&self) -> String {
        let mut output = String::new();

        writeln!(&mut output, "Grammar: {}", self.grammar.name).unwrap();
        writeln!(&mut output, "{}", "=".repeat(50)).unwrap();

        // Tokens
        writeln!(&mut output, "\nTokens:").unwrap();
        for (id, token) in &self.grammar.tokens {
            let pattern = match &token.pattern {
                adze_ir::TokenPattern::String(s) => format!("\"{}\"", s),
                adze_ir::TokenPattern::Regex(r) => format!("/{}/", r),
            };
            writeln!(&mut output, "  {} ({:?}) = {}", token.name, id, pattern).unwrap();
        }

        // External tokens
        if !self.grammar.externals.is_empty() {
            writeln!(&mut output, "\nExternal Tokens:").unwrap();
            for external in &self.grammar.externals {
                writeln!(
                    &mut output,
                    "  {} ({:?})",
                    external.name, external.symbol_id
                )
                .unwrap();
            }
        }

        // Rules
        writeln!(&mut output, "\nRules:").unwrap();
        for (lhs, rules) in &self.grammar.rules {
            let lhs_name = self.get_symbol_name(*lhs);
            for rule in rules {
                write!(&mut output, "  {} ::=", lhs_name).unwrap();

                for symbol in &rule.rhs {
                    match symbol {
                        Symbol::Terminal(id) => {
                            let name = self
                                .grammar
                                .tokens
                                .get(id)
                                .map(|t| t.name.clone())
                                .unwrap_or_else(|| format!("T{}", id.0));
                            write!(&mut output, " '{}'", name).unwrap();
                        }
                        Symbol::NonTerminal(id) => {
                            write!(&mut output, " {}", self.get_symbol_name(*id)).unwrap();
                        }
                        Symbol::External(id) => {
                            write!(&mut output, " ${}", id.0).unwrap();
                        }
                        Symbol::Optional(inner) => {
                            write!(&mut output, " {}?", self.format_symbol_simple(inner)).unwrap();
                        }
                        Symbol::Repeat(inner) => {
                            write!(&mut output, " {}*", self.format_symbol_simple(inner)).unwrap();
                        }
                        Symbol::RepeatOne(inner) => {
                            write!(&mut output, " {}+", self.format_symbol_simple(inner)).unwrap();
                        }
                        Symbol::Choice(choices) => {
                            write!(&mut output, " (").unwrap();
                            for (i, choice) in choices.iter().enumerate() {
                                if i > 0 {
                                    write!(&mut output, " | ").unwrap();
                                }
                                write!(&mut output, "{}", self.format_symbol_simple(choice))
                                    .unwrap();
                            }
                            write!(&mut output, ")").unwrap();
                        }
                        Symbol::Sequence(seq) => {
                            for s in seq {
                                write!(&mut output, " {}", self.format_symbol_simple(s)).unwrap();
                            }
                        }
                        Symbol::Epsilon => {
                            write!(&mut output, " ε").unwrap();
                        }
                    }
                }

                // Add metadata
                if let Some(prec) = &rule.precedence {
                    write!(&mut output, " [precedence: {:?}]", prec).unwrap();
                }
                if let Some(assoc) = &rule.associativity {
                    write!(&mut output, " [associativity: {:?}]", assoc).unwrap();
                }

                writeln!(&mut output).unwrap();
            }
        }

        // Precedences
        if !self.grammar.precedences.is_empty() {
            writeln!(&mut output, "\nPrecedence Declarations:").unwrap();
            for prec in &self.grammar.precedences {
                write!(&mut output, "  Level {}: ", prec.level).unwrap();
                for symbol in &prec.symbols {
                    write!(&mut output, "{:?} ", symbol).unwrap();
                }
                writeln!(&mut output, "({:?})", prec.associativity).unwrap();
            }
        }

        // Conflicts
        if !self.grammar.conflicts.is_empty() {
            writeln!(&mut output, "\nConflict Declarations:").unwrap();
            for conflict in &self.grammar.conflicts {
                write!(&mut output, "  Symbols: ").unwrap();
                for symbol in &conflict.symbols {
                    write!(&mut output, "{:?} ", symbol).unwrap();
                }
                writeln!(&mut output, "Resolution: {:?}", conflict.resolution).unwrap();
            }
        }

        output
    }

    /// Generate dependency graph showing which symbols depend on which
    pub fn dependency_graph(&self) -> String {
        let mut output = String::new();
        let mut dependencies: BTreeMap<SymbolId, BTreeSet<SymbolId>> = BTreeMap::new();

        // Build dependency map
        for (lhs, rules) in &self.grammar.rules {
            let deps = dependencies.entry(*lhs).or_default();
            for rule in rules {
                for symbol in &rule.rhs {
                    if let Symbol::NonTerminal(id) = symbol {
                        deps.insert(*id);
                    }
                }
            }
        }

        writeln!(&mut output, "Symbol Dependencies:").unwrap();
        writeln!(&mut output, "===================").unwrap();

        for (symbol, deps) in dependencies {
            let symbol_name = self.get_symbol_name(symbol);
            write!(&mut output, "{} depends on:", symbol_name).unwrap();

            if deps.is_empty() {
                write!(&mut output, " (none)").unwrap();
            } else {
                for dep in deps {
                    write!(&mut output, " {}", self.get_symbol_name(dep)).unwrap();
                }
            }
            writeln!(&mut output).unwrap();
        }

        output
    }

    fn get_symbol_name(&self, id: SymbolId) -> String {
        // Check tokens
        if let Some(token) = self.grammar.tokens.get(&id) {
            return token.name.clone();
        }

        // Check if it's a rule
        if self.grammar.rules.contains_key(&id) {
            return format!("rule_{}", id.0);
        }

        // Check externals
        for external in &self.grammar.externals {
            if external.symbol_id == id {
                return external.name.clone();
            }
        }

        format!("symbol_{}", id.0)
    }

    fn format_symbol_simple(&self, symbol: &Symbol) -> String {
        match symbol {
            Symbol::Terminal(id) => self
                .grammar
                .tokens
                .get(id)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| format!("T{}", id.0)),
            Symbol::NonTerminal(id) => self.get_symbol_name(*id),
            Symbol::External(id) => format!("External{}", id.0),
            Symbol::Optional(inner) => format!("{}?", self.format_symbol_simple(inner)),
            Symbol::Repeat(inner) => format!("{}*", self.format_symbol_simple(inner)),
            Symbol::RepeatOne(inner) => format!("{}+", self.format_symbol_simple(inner)),
            Symbol::Choice(choices) => {
                let parts: Vec<_> = choices
                    .iter()
                    .map(|s| self.format_symbol_simple(s))
                    .collect();
                format!("({})", parts.join("|"))
            }
            Symbol::Sequence(seq) => {
                let parts: Vec<_> = seq.iter().map(|s| self.format_symbol_simple(s)).collect();
                parts.join(" ")
            }
            Symbol::Epsilon => "ε".to_string(),
        }
    }

    fn escape_dot(&self, s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    }

    fn escape_xml(&self, s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
}

// Note: TreeVisualizer for parse trees should be implemented in the runtime crate
// where tree_sitter types are available, not in the tool crate

#[cfg(test)]
mod tests {
    use super::*;
    use adze_ir::Rule;
    use adze_ir::{ProductionId, Token, TokenPattern};

    #[test]
    fn test_grammar_to_text() {
        let mut grammar = Grammar::new("test".to_string());

        let id_sym = SymbolId(1);
        grammar.tokens.insert(
            id_sym,
            Token {
                name: "identifier".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );

        let expr_sym = SymbolId(2);
        grammar.rules.insert(
            expr_sym,
            vec![Rule {
                lhs: expr_sym,
                rhs: vec![Symbol::Terminal(id_sym)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );

        let visualizer = GrammarVisualizer::new(grammar);
        let text = visualizer.to_text();

        assert!(text.contains("Grammar: test"));
        assert!(text.contains("identifier"));
        assert!(text.contains("rule_2 ::= 'identifier'"));
    }

    #[test]
    fn test_dot_generation() {
        let grammar = Grammar::new("test".to_string());
        let visualizer = GrammarVisualizer::new(grammar);
        let dot = visualizer.to_dot();

        assert!(dot.contains("digraph Grammar"));
        assert!(dot.contains("rankdir=LR"));
    }

    // --- Helpers ----------------------------------------------------------

    fn empty_visualizer() -> GrammarVisualizer {
        GrammarVisualizer::new(Grammar::new("g".to_string()))
    }

    fn make_token(name: &str) -> Token {
        Token {
            name: name.to_string(),
            pattern: TokenPattern::String(name.to_string()),
            fragile: false,
        }
    }

    // --- escape_dot / escape_xml ----------------------------------------

    #[test]
    fn escape_dot_escapes_backslash_quote_and_newline() {
        let v = empty_visualizer();
        assert_eq!(v.escape_dot("plain"), "plain");
        assert_eq!(v.escape_dot("a\\b"), "a\\\\b");
        assert_eq!(v.escape_dot("he said \"hi\""), "he said \\\"hi\\\"");
        assert_eq!(v.escape_dot("one\ntwo"), "one\\ntwo");
        // Combinations apply in the documented order: backslash first.
        assert_eq!(v.escape_dot("\\\"\n"), "\\\\\\\"\\n");
    }

    #[test]
    fn escape_xml_escapes_all_five_xml_entities() {
        let v = empty_visualizer();
        assert_eq!(v.escape_xml(""), "");
        assert_eq!(v.escape_xml("ok"), "ok");
        assert_eq!(v.escape_xml("a&b"), "a&amp;b");
        assert_eq!(v.escape_xml("a<b>c"), "a&lt;b&gt;c");
        assert_eq!(v.escape_xml("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(v.escape_xml("it's"), "it&apos;s");
        // The replacement of `&` happens first, so previously-introduced `&`
        // markers from later passes are not double-escaped.
        assert_eq!(v.escape_xml("<&>"), "&lt;&amp;&gt;");
    }

    // --- format_symbol_simple -------------------------------------------

    #[test]
    fn format_symbol_simple_known_terminal_uses_token_name() {
        let mut grammar = Grammar::new("g".to_string());
        let id = SymbolId(7);
        grammar.tokens.insert(id, make_token("plus"));
        let v = GrammarVisualizer::new(grammar);
        assert_eq!(v.format_symbol_simple(&Symbol::Terminal(id)), "plus");
    }

    #[test]
    fn format_symbol_simple_unknown_terminal_falls_back_to_t_prefix() {
        let v = empty_visualizer();
        assert_eq!(v.format_symbol_simple(&Symbol::Terminal(SymbolId(9))), "T9");
    }

    #[test]
    fn format_symbol_simple_handles_all_compound_variants() {
        let v = empty_visualizer();
        let term = Box::new(Symbol::Terminal(SymbolId(1)));

        assert_eq!(
            v.format_symbol_simple(&Symbol::Optional(term.clone())),
            "T1?"
        );
        assert_eq!(v.format_symbol_simple(&Symbol::Repeat(term.clone())), "T1*");
        assert_eq!(
            v.format_symbol_simple(&Symbol::RepeatOne(term.clone())),
            "T1+"
        );
        assert_eq!(
            v.format_symbol_simple(&Symbol::External(SymbolId(3))),
            "External3"
        );
        assert_eq!(v.format_symbol_simple(&Symbol::Epsilon), "ε");

        let choice = Symbol::Choice(vec![Symbol::Terminal(SymbolId(1)), Symbol::Epsilon]);
        assert_eq!(v.format_symbol_simple(&choice), "(T1|ε)");

        let seq = Symbol::Sequence(vec![
            Symbol::Terminal(SymbolId(1)),
            Symbol::Terminal(SymbolId(2)),
        ]);
        assert_eq!(v.format_symbol_simple(&seq), "T1 T2");
    }

    #[test]
    fn format_symbol_simple_nests_through_modifiers() {
        let v = empty_visualizer();
        // Optional(Repeat(Terminal)) — exercises recursion through two layers.
        let inner = Box::new(Symbol::Repeat(Box::new(Symbol::Terminal(SymbolId(5)))));
        assert_eq!(v.format_symbol_simple(&Symbol::Optional(inner)), "T5*?");
    }

    #[test]
    fn format_symbol_simple_nonterminal_uses_rule_id_when_unnamed() {
        let mut grammar = Grammar::new("g".to_string());
        let id = SymbolId(4);
        grammar.rules.insert(id, vec![]);
        let v = GrammarVisualizer::new(grammar);
        assert_eq!(v.format_symbol_simple(&Symbol::NonTerminal(id)), "rule_4");
    }

    // --- get_symbol_name ------------------------------------------------

    #[test]
    fn get_symbol_name_returns_token_name_first() {
        let mut grammar = Grammar::new("g".to_string());
        let id = SymbolId(1);
        grammar.tokens.insert(id, make_token("ident"));
        let v = GrammarVisualizer::new(grammar);
        assert_eq!(v.get_symbol_name(id), "ident");
    }

    #[test]
    fn get_symbol_name_returns_rule_id_label_for_rules() {
        let mut grammar = Grammar::new("g".to_string());
        let id = SymbolId(2);
        grammar.rules.insert(id, vec![]);
        let v = GrammarVisualizer::new(grammar);
        assert_eq!(v.get_symbol_name(id), "rule_2");
    }

    #[test]
    fn get_symbol_name_returns_external_name() {
        let mut grammar = Grammar::new("g".to_string());
        grammar.externals.push(adze_ir::ExternalToken {
            name: "newline".to_string(),
            symbol_id: SymbolId(8),
        });
        let v = GrammarVisualizer::new(grammar);
        assert_eq!(v.get_symbol_name(SymbolId(8)), "newline");
    }

    #[test]
    fn get_symbol_name_falls_back_to_symbol_prefix() {
        let v = empty_visualizer();
        assert_eq!(v.get_symbol_name(SymbolId(42)), "symbol_42");
    }

    // --- to_dot ---------------------------------------------------------

    #[test]
    fn to_dot_emits_terminal_nonterminal_external_and_edges() {
        let mut grammar = Grammar::new("g".to_string());

        let tok = SymbolId(1);
        grammar.tokens.insert(tok, make_token("\"id\""));

        let rule_sym = SymbolId(2);
        grammar.rules.insert(
            rule_sym,
            vec![Rule {
                lhs: rule_sym,
                rhs: vec![
                    Symbol::Terminal(tok),
                    Symbol::NonTerminal(rule_sym),
                    Symbol::External(SymbolId(3)),
                    Symbol::Epsilon,
                ],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );

        grammar.externals.push(adze_ir::ExternalToken {
            name: "ext".to_string(),
            symbol_id: SymbolId(3),
        });

        let dot = GrammarVisualizer::new(grammar).to_dot();

        // Terminal node + escaped quote in the label.
        assert!(dot.contains("t1 [label=\"\\\"id\\\"\""));
        // Non-terminal node.
        assert!(dot.contains("n2 [label=\"rule_2\""));
        // External node.
        assert!(dot.contains("e3 [label=\"ext\""));
        // Edges for terminal, nonterminal and external — labels reflect rhs index.
        assert!(dot.contains("n2 -> t1 [label=\"1\"]"));
        assert!(dot.contains("n2 -> n2 [label=\"2\"]"));
        assert!(dot.contains("n2 -> e3 [label=\"3\"]"));
        // Epsilon is skipped — no edge for position 4.
        assert!(!dot.contains("[label=\"4\"]"));
    }

    #[test]
    fn to_dot_uses_empty_edge_label_for_single_rhs() {
        let mut grammar = Grammar::new("g".to_string());
        let tok = SymbolId(1);
        grammar.tokens.insert(tok, make_token("t"));
        let lhs = SymbolId(2);
        grammar.rules.insert(
            lhs,
            vec![Rule {
                lhs,
                rhs: vec![Symbol::Terminal(tok)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );

        let dot = GrammarVisualizer::new(grammar).to_dot();
        assert!(dot.contains("n2 -> t1 [label=\"\"]"));
    }

    #[test]
    fn to_dot_emits_compound_placeholder_targets() {
        let mut grammar = Grammar::new("g".to_string());
        let tok = SymbolId(1);
        grammar.tokens.insert(tok, make_token("t"));
        let lhs = SymbolId(2);
        grammar.rules.insert(
            lhs,
            vec![Rule {
                lhs,
                rhs: vec![
                    Symbol::Optional(Box::new(Symbol::Terminal(tok))),
                    Symbol::Repeat(Box::new(Symbol::Terminal(tok))),
                    Symbol::RepeatOne(Box::new(Symbol::Terminal(tok))),
                    Symbol::Choice(vec![Symbol::Terminal(tok)]),
                    Symbol::Sequence(vec![Symbol::Terminal(tok)]),
                ],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );

        let dot = GrammarVisualizer::new(grammar).to_dot();
        // Each compound symbol creates a synthetic target name keyed on the rhs index.
        assert!(dot.contains("n2 -> opt0"));
        assert!(dot.contains("n2 -> rep1"));
        assert!(dot.contains("n2 -> rep12"));
        assert!(dot.contains("n2 -> choice3"));
        assert!(dot.contains("n2 -> seq4"));
    }

    // --- to_railroad_svg ------------------------------------------------

    #[test]
    fn to_railroad_svg_renders_each_symbol_variant() {
        let mut grammar = Grammar::new("g".to_string());
        let tok = SymbolId(1);
        grammar.tokens.insert(tok, make_token("kw"));

        let lhs = SymbolId(2);
        grammar.rules.insert(
            lhs,
            vec![Rule {
                lhs,
                rhs: vec![
                    Symbol::Terminal(tok),
                    Symbol::NonTerminal(lhs),
                    Symbol::External(SymbolId(3)),
                    Symbol::Optional(Box::new(Symbol::Terminal(tok))),
                    Symbol::Repeat(Box::new(Symbol::Terminal(tok))),
                    Symbol::RepeatOne(Box::new(Symbol::Terminal(tok))),
                    Symbol::Choice(vec![
                        Symbol::Terminal(tok),
                        Symbol::Terminal(SymbolId(99)), // unknown -> T99 fallback
                    ]),
                    Symbol::Sequence(vec![Symbol::Terminal(tok), Symbol::Epsilon]),
                    Symbol::Epsilon,
                ],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );

        let svg = GrammarVisualizer::new(grammar).to_railroad_svg();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(r#"<text x="10""#));
        assert!(svg.contains("rule_2 ::="));
        assert!(svg.contains("kw"));
        assert!(svg.contains("External3"));
        assert!(svg.contains("kw?"));
        assert!(svg.contains("kw*"));
        assert!(svg.contains("kw+"));
        // Choice formatting and unknown-terminal fallback both render.
        assert!(svg.contains("(kw | T99)"));
        // Epsilon symbol is present.
        assert!(svg.contains("ε"));
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    // --- to_text --------------------------------------------------------

    #[test]
    fn to_text_renders_empty_grammar_header_only() {
        let text = empty_visualizer().to_text();
        assert!(text.starts_with("Grammar: g\n"));
        assert!(text.contains("Tokens:"));
        assert!(text.contains("Rules:"));
        // Empty grammar has no externals / precedences / conflicts sections.
        assert!(!text.contains("External Tokens:"));
        assert!(!text.contains("Precedence Declarations:"));
        assert!(!text.contains("Conflict Declarations:"));
    }

    #[test]
    fn to_text_renders_all_symbol_kinds_and_metadata() {
        let mut grammar = Grammar::new("g".to_string());
        let tok_str = SymbolId(1);
        let tok_re = SymbolId(2);
        grammar.tokens.insert(
            tok_str,
            Token {
                name: "plus".to_string(),
                pattern: TokenPattern::String("+".to_string()),
                fragile: false,
            },
        );
        grammar.tokens.insert(
            tok_re,
            Token {
                name: "ident".to_string(),
                pattern: TokenPattern::Regex(r"[a-z]+".to_string()),
                fragile: false,
            },
        );

        grammar.externals.push(adze_ir::ExternalToken {
            name: "newline".to_string(),
            symbol_id: SymbolId(7),
        });

        let lhs = SymbolId(3);
        grammar.rules.insert(
            lhs,
            vec![Rule {
                lhs,
                rhs: vec![
                    Symbol::Terminal(tok_str),
                    Symbol::NonTerminal(lhs),
                    Symbol::External(SymbolId(7)),
                    Symbol::Optional(Box::new(Symbol::Terminal(tok_str))),
                    Symbol::Repeat(Box::new(Symbol::Terminal(tok_str))),
                    Symbol::RepeatOne(Box::new(Symbol::Terminal(tok_str))),
                    Symbol::Choice(vec![Symbol::Terminal(tok_str), Symbol::Terminal(tok_re)]),
                    Symbol::Sequence(vec![Symbol::Terminal(tok_str), Symbol::Epsilon]),
                    Symbol::Epsilon,
                ],
                precedence: Some(adze_ir::PrecedenceKind::Static(2)),
                associativity: Some(adze_ir::Associativity::Left),
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );

        grammar.precedences.push(adze_ir::Precedence {
            level: 5,
            associativity: adze_ir::Associativity::Right,
            symbols: vec![tok_str],
        });

        grammar.conflicts.push(adze_ir::ConflictDeclaration {
            symbols: vec![tok_str, tok_re],
            resolution: adze_ir::ConflictResolution::GLR,
        });

        let text = GrammarVisualizer::new(grammar).to_text();

        // String + regex token formatting both appear.
        assert!(text.contains("plus (SymbolId(1)) = \"+\""));
        assert!(text.contains("ident (SymbolId(2)) = /[a-z]+/"));
        // Externals section renders.
        assert!(text.contains("External Tokens:"));
        assert!(text.contains("newline (SymbolId(7))"));
        // Compound formatting on rhs.
        assert!(text.contains("'plus'"));
        assert!(text.contains("$7"));
        assert!(text.contains("plus?"));
        assert!(text.contains("plus*"));
        assert!(text.contains("plus+"));
        assert!(text.contains("(plus | ident)"));
        assert!(text.contains(" ε"));
        // Metadata for precedence + associativity.
        assert!(text.contains("[precedence: Static(2)]"));
        assert!(text.contains("[associativity: Left]"));
        // Precedence + conflict sections.
        assert!(text.contains("Precedence Declarations:"));
        assert!(text.contains("Level 5:"));
        assert!(text.contains("Conflict Declarations:"));
        assert!(text.contains("Resolution: GLR"));
    }

    #[test]
    fn to_text_uses_t_prefix_for_unknown_terminal_in_rule() {
        let mut grammar = Grammar::new("g".to_string());
        let lhs = SymbolId(1);
        // Terminal id (99) not present in tokens map — exercises the fallback branch.
        grammar.rules.insert(
            lhs,
            vec![Rule {
                lhs,
                rhs: vec![Symbol::Terminal(SymbolId(99))],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );
        let text = GrammarVisualizer::new(grammar).to_text();
        assert!(text.contains("'T99'"));
    }

    // --- dependency_graph ----------------------------------------------

    #[test]
    fn dependency_graph_lists_nonterminal_dependencies() {
        let mut grammar = Grammar::new("g".to_string());
        let a = SymbolId(1);
        let b = SymbolId(2);
        let tok = SymbolId(3);
        grammar.tokens.insert(tok, make_token("t"));
        // A depends on B and itself, B has no nonterminal deps.
        grammar.rules.insert(
            a,
            vec![Rule {
                lhs: a,
                rhs: vec![
                    Symbol::NonTerminal(b),
                    Symbol::NonTerminal(a),
                    Symbol::Terminal(tok), // terminals do not contribute deps
                ],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );
        grammar.rules.insert(
            b,
            vec![Rule {
                lhs: b,
                rhs: vec![Symbol::Terminal(tok)],
                precedence: None,
                associativity: None,
                fields: vec![],
                production_id: ProductionId(0),
            }],
        );

        let graph = GrammarVisualizer::new(grammar).dependency_graph();
        assert!(graph.starts_with("Symbol Dependencies:\n"));
        assert!(graph.contains("rule_1 depends on: rule_1 rule_2"));
        assert!(graph.contains("rule_2 depends on: (none)"));
    }

    #[test]
    fn dependency_graph_empty_grammar_emits_header_only() {
        let graph = empty_visualizer().dependency_graph();
        assert!(graph.contains("Symbol Dependencies:"));
        // No `depends on` lines for an empty grammar.
        assert!(!graph.contains(" depends on:"));
    }
}
