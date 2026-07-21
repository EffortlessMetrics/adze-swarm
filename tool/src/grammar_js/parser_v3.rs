#![allow(clippy::manual_strip)]

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use regex::Regex;

use super::helpers::HelperFunctions;
use super::{ExternalToken, GrammarJs, Rule};

#[cfg(not(debug_assertions))]
macro_rules! eprintln {
    ($($arg:tt)*) => {};
}

#[cfg(debug_assertions)]
macro_rules! eprintln {
    ($($arg:tt)*) => {
        if std::env::var("RUST_LOG")
            .ok()
            .unwrap_or_default()
            .contains("debug")
        {
            std::eprintln!($($arg)*);
        }
    };
}

mod function_block;
mod lexing;

/// Check if a string is a symbol reference like $.identifier
fn is_symbol_ref(s: &str) -> bool {
    let trimmed = s.trim();
    trimmed.starts_with("$.")
        && trimmed.len() > 2
        && trimmed[2..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A more robust parser for grammar.js files
pub struct GrammarJsParserV3 {
    content: String,
    precedence_map: IndexMap<String, i32>,
}

impl GrammarJsParserV3 {
    pub fn new(content: String) -> Self {
        Self {
            content,
            precedence_map: IndexMap::new(),
        }
    }

    pub fn parse(&mut self) -> Result<GrammarJs> {
        // First, extract any JavaScript constants like PREC
        self.extract_js_constants()?;

        // Then find the module.exports pattern
        let exports_regex = Regex::new(r"module\.exports\s*=\s*grammar\s*\(")?;

        let grammar_content = if let Some(mat) = exports_regex.find(&self.content) {
            // Found the start, now find the matching closing parenthesis
            let start = mat.end();
            let end = self.find_matching_paren(&self.content[start..])?;
            self.content[start..start + end].to_string()
        } else {
            bail!("Could not find module.exports = grammar(...) pattern")
        };

        // Parse the grammar content
        self.parse_grammar_content(&grammar_content)
    }

    fn parse_grammar_content(&mut self, content: &str) -> Result<GrammarJs> {
        let mut grammar = GrammarJs {
            name: String::new(),
            word: None,
            rules: IndexMap::new(),
            extras: vec![],
            conflicts: vec![],
            externals: vec![],
            inline: vec![],
            supertypes: vec![],
            precedences: vec![],
            start_symbol: None,
            wrapper_token_relations: IndexMap::new(),
        };

        // Extract name
        grammar.name = self.extract_grammar_name(content)?;

        // Extract word token
        grammar.word = self.extract_word_token(content);

        // Extract extras
        grammar.extras = self.extract_extras(content)?;

        // Extract externals
        grammar.externals = self.extract_externals(content)?;

        // Extract conflicts
        grammar.conflicts = self.extract_conflicts(content)?;

        // Extract inline rules
        grammar.inline = self.extract_inline(content)?;

        // Extract supertypes
        grammar.supertypes = self.extract_supertypes(content)?;

        // Extract precedences and build precedence map
        grammar.precedences = self.extract_precedences(content)?;
        self.build_precedence_map(&grammar.precedences);

        // Extract rules
        grammar.rules = self.extract_rules(content)?;

        Ok(grammar)
    }

    fn extract_js_constants(&mut self) -> Result<()> {
        // Look for const PREC = { ... } or similar patterns
        let const_regex = Regex::new(r"const\s+(\w+)\s*=\s*\{")?;

        if let Some(mat) = const_regex.find(&self.content) {
            let const_name = self.content[mat.start()..mat.end()]
                .split_whitespace()
                .nth(1)
                .unwrap_or("")
                .trim_end_matches('=');

            if const_name == "PREC" {
                // Extract the object content
                let start = mat.end();
                let end = self.find_matching_brace(&self.content[start..])?;
                let prec_content = self.content[start..start + end].to_string();

                // Parse the precedence object
                self.parse_prec_object(&prec_content)?;
            }
        }

        Ok(())
    }

    fn parse_prec_object(&mut self, content: &str) -> Result<()> {
        // Parse JavaScript object like { lambda: -2, typed_parameter: -1, ... }
        // Use a simple regex to extract key: value pairs
        let pair_regex = Regex::new(r"(\w+)\s*:\s*(-?\d+)")?;

        for caps in pair_regex.captures_iter(content) {
            let name = caps[1].to_string();
            let value: i32 = caps[2].parse()?;
            self.precedence_map.insert(name, value);
        }

        Ok(())
    }

    fn find_matching_brace(&self, content: &str) -> Result<usize> {
        self.find_matching_bracket(content, '{', '}')
    }

    fn extract_grammar_name(&self, content: &str) -> Result<String> {
        let name_regex = Regex::new(r#"name:\s*['"]([^'"]+)['"]"#)?;

        if let Some(caps) = name_regex.captures(content) {
            Ok(caps[1].to_string())
        } else {
            bail!("Could not find grammar name")
        }
    }

    fn extract_word_token(&self, content: &str) -> Option<String> {
        let word_regex = Regex::new(r#"word:\s*\$\s*=>\s*\$\.(\w+)"#).ok()?;

        word_regex.captures(content).map(|caps| caps[1].to_string())
    }

    fn extract_extras(&self, content: &str) -> Result<Vec<Rule>> {
        // Find extras: $ => [
        if let Some(extras_start) = content.find("extras:") {
            let after_extras = &content[extras_start + 7..]; // Skip "extras:"
            let trimmed = after_extras.trim_start();

            // Skip $ =>
            if let Some(arrow_pos) = trimmed.find("=>") {
                let after_arrow = trimmed[arrow_pos + 2..].trim_start();

                if after_arrow.starts_with('[') {
                    // Extract the array content by matching brackets
                    let array_content = self.extract_balanced_delim(&after_arrow[1..], '[', ']')?;
                    return self.parse_rule_array(&array_content);
                }
            }
        }

        Ok(vec![])
    }

    fn extract_rules(&self, content: &str) -> Result<IndexMap<String, Rule>> {
        let mut rules = IndexMap::new();

        // Find the rules: section
        if let Some(rules_start) = content.find("rules:") {
            let after_rules = &content[rules_start + 6..]; // Skip "rules:"

            // Skip whitespace and find the opening brace
            let trimmed = after_rules.trim_start();
            if !trimmed.starts_with('{') {
                bail!("Expected '{{' after 'rules:'");
            }

            // Extract the rules object content by matching braces
            let rules_content = self.extract_balanced_delim(&trimmed[1..], '{', '}')?;

            eprintln!(
                "Debug: Found rules content of length {}",
                rules_content.len()
            );

            // Parse individual rules using a more robust approach
            self.parse_rules_object(&rules_content, &mut rules)?;
        }

        Ok(rules)
    }

    fn parse_rules_object(&self, content: &str, rules: &mut IndexMap<String, Rule>) -> Result<()> {
        // Use regex to find all rule definitions
        let rule_regex = Regex::new(r"(\w+):\s*\$\s*=>\s*")?;

        let mut _last_end = 0;
        for mat in rule_regex.find_iter(content) {
            // Extract rule name
            let rule_name = content[mat.start()..mat.end()]
                .split(':')
                .next()
                .unwrap()
                .trim()
                .to_string();

            // Find the end of this rule by looking for the next rule or end of object
            let rule_start = mat.end();
            let mut rule_end = content.len();

            // Look for the next rule
            if let Some(next_match) = rule_regex.find_at(content, rule_start) {
                // Back up to find the comma before the next rule
                let mut pos = next_match.start();
                while pos > rule_start {
                    pos -= 1;
                    if content.chars().nth(pos) == Some(',') {
                        rule_end = pos;
                        break;
                    }
                }
            }

            let rule_def = content[rule_start..rule_end].trim();
            let rule_def = rule_def.trim_end_matches(',');

            let def_preview = if rule_def.len() > 50 {
                format!("{}...", &rule_def[..50])
            } else {
                rule_def.to_string()
            };
            eprintln!(
                "Debug: Parsing rule '{}' with definition: {}",
                rule_name, def_preview
            );

            let rule = self
                .parse_rule(rule_def)
                .with_context(|| format!("Failed to parse rule '{}'", rule_name))?;

            rules.insert(rule_name, rule);
            _last_end = rule_end;
        }

        Ok(())
    }

    fn extract_balanced_delim(&self, content: &str, open: char, close: char) -> Result<String> {
        eprintln!(
            "Debug: extract_balanced_delim called with open='{}' close='{}', content length={}",
            open,
            close,
            content.chars().count()
        );

        lexing::extract_balanced_delim(content, open, close)
    }

    fn find_matching_paren(&self, content: &str) -> Result<usize> {
        self.extract_balanced_delim(content, '(', ')')
            .map(|s| s.len() + 1)
    }

    fn parse_rule(&self, rule_def: &str) -> Result<Rule> {
        // Strip arrow function callbacks (we don't need JS callbacks for code generation)
        let rule_def = rule_def.split("=>").next().unwrap_or(rule_def).trim();
        let trimmed = rule_def;

        // Handle different rule patterns
        if trimmed.starts_with("prec.left(") {
            self.parse_prec_left(trimmed)
        } else if trimmed.starts_with("prec.right(") {
            self.parse_prec_right(trimmed)
        } else if trimmed.starts_with("prec.dynamic(") {
            self.parse_prec_dynamic(trimmed)
        } else if trimmed.starts_with("prec(") {
            self.parse_prec(trimmed)
        } else if trimmed.starts_with("seq(") {
            self.parse_seq(trimmed)
        } else if trimmed.starts_with("choice(") {
            self.parse_choice(trimmed)
        } else if trimmed.starts_with("repeat(") {
            self.parse_repeat(trimmed)
        } else if trimmed.starts_with("repeat1(") {
            self.parse_repeat1(trimmed)
        } else if trimmed.starts_with("optional(") {
            self.parse_optional(trimmed)
        } else if trimmed.starts_with("field(") {
            self.parse_field(trimmed)
        } else if trimmed.starts_with("alias(") {
            self.parse_alias(trimmed)
        } else if trimmed.starts_with("token(") {
            self.parse_token(trimmed)
        } else if trimmed.starts_with("$") {
            // Symbol reference
            Ok(Rule::Symbol {
                name: trimmed[1..].trim_start_matches('.').to_string(),
            })
        } else if trimmed.starts_with("'") || trimmed.starts_with("\"") {
            // String literal
            let quote = &trimmed[0..1];
            if let Some(end) = trimmed[1..].find(quote) {
                Ok(Rule::String {
                    value: trimmed[1..end + 1].to_string(),
                })
            } else {
                bail!("Unterminated string literal")
            }
        } else if trimmed.starts_with("/") {
            // Regex pattern
            if let Some(end) = trimmed[1..].find('/') {
                Ok(Rule::Pattern {
                    value: trimmed[1..end + 1].to_string(),
                })
            } else {
                bail!("Unterminated regex pattern")
            }
        } else if trimmed.starts_with("{") {
            // Function block - extract the return statement or parse const/table-based definitions
            self.parse_function_block(trimmed)
        } else if trimmed.contains('(') {
            // Could be a helper function call
            if let Some(paren_pos) = trimmed.find('(') {
                let func_name = trimmed[..paren_pos].trim();
                if HelperFunctions::is_helper_function(func_name) {
                    self.parse_helper_call(trimmed)
                } else {
                    eprintln!("Warning: Unknown function call: {}", trimmed);
                    Ok(Rule::Seq { members: vec![] })
                }
            } else {
                eprintln!("Warning: Unknown rule pattern: {}", trimmed);
                Ok(Rule::Seq { members: vec![] })
            }
        } else {
            // Unknown pattern - for now return a placeholder
            eprintln!("Warning: Unknown rule pattern: {}", trimmed);
            Ok(Rule::Seq { members: vec![] })
        }
    }

    fn parse_function_block(&self, block: &str) -> Result<Rule> {
        match function_block::parse(block)? {
            function_block::ParsedFunctionBlock::PlaceholderChoice => {
                Ok(Rule::Choice { members: vec![] })
            }
            function_block::ParsedFunctionBlock::ReturnExpression(return_expr) => {
                self.parse_rule(&return_expr)
            }
        }
    }

    fn parse_helper_call(&self, call: &str) -> Result<Rule> {
        // Extract function name and arguments
        if let Some(paren_pos) = call.find('(') {
            let func_name = call[..paren_pos].trim();
            let args_with_paren = &call[paren_pos..];
            let args_content = self.extract_function_args(args_with_paren, "")?;

            // Parse arguments
            let args = self.parse_rule_list(&args_content)?;

            // Evaluate the helper function
            HelperFunctions::evaluate_helper(func_name, args)
        } else {
            bail!("Invalid helper function call: {}", call)
        }
    }

    fn parse_rule_array(&self, content: &str) -> Result<Vec<Rule>> {
        let mut rules = vec![];

        // Use split_args for proper comma handling
        let args = self.split_args(content, -1)?;
        for arg in args {
            let trimmed = arg.trim();
            if !trimmed.is_empty() {
                rules.push(self.parse_arg(trimmed)?);
            }
        }

        Ok(rules)
    }

    fn build_precedence_map(&mut self, precedences: &[Vec<(String, i32)>]) {
        // Build a map from precedence names to values
        // If precedences are given as [['name1', 'name2'], ['name3', 'name4']]
        // They get values like name1=2, name2=2, name3=1, name4=1
        let mut value = precedences.len() as i32;
        for group in precedences {
            for (name, explicit_value) in group {
                if *explicit_value != 0 {
                    self.precedence_map.insert(name.clone(), *explicit_value);
                } else {
                    self.precedence_map.insert(name.clone(), value);
                }
            }
            value -= 1;
        }
    }

    /// Parse any argument that could be a symbol reference, string literal, or nested rule
    fn parse_arg(&self, text: &str) -> Result<Rule> {
        let trimmed = text.trim();
        if is_symbol_ref(trimmed) {
            Ok(Rule::Symbol {
                name: trimmed[2..].to_string(),
            })
        } else if trimmed.starts_with("'") || trimmed.starts_with('"') {
            Ok(Rule::String {
                value: self.extract_string_literal(trimmed)?,
            })
        } else {
            // nested rule expression - recurse
            self.parse_rule(trimmed)
        }
    }

    fn parse_precedence_value(&self, level_str: &str) -> Result<i32> {
        if let Ok(val) = level_str.parse::<i32>() {
            Ok(val)
        } else {
            // Try to look up named precedence
            let name = if level_str.starts_with("PREC.") {
                // Handle PREC.name format
                &level_str[5..]
            } else {
                // Handle 'name' format
                level_str.trim_matches(|c| c == '\'' || c == '"')
            };
            self.precedence_map
                .get(name)
                .copied()
                .with_context(|| format!("Unknown precedence name: {}", name))
        }
    }

    // Parse precedence functions
    fn parse_prec(&self, rule_def: &str) -> Result<Rule> {
        // prec(level, rule) OR prec(rule) - level can be numeric or a string name
        let content = self.extract_function_args(rule_def, "prec")?;
        let parts = self.split_args(&content, -1)?;

        let (value, rule_idx) = if parts.len() == 1 {
            // No explicit precedence, use default 0
            (0, 0)
        } else if parts.len() == 2 {
            let level_str = parts[0].trim();
            let value = self.parse_precedence_value(level_str)?;
            (value, 1)
        } else {
            bail!("prec() expects 1 or 2 arguments, got {}", parts.len());
        };

        let content = Box::new(self.parse_rule(&parts[rule_idx])?);

        Ok(Rule::Prec { value, content })
    }

    fn parse_prec_left(&self, rule_def: &str) -> Result<Rule> {
        // prec.left(level, rule) OR prec.left(rule) - level can be numeric or a string name
        let content = self.extract_function_args(rule_def, "prec.left")?;
        let parts = self.split_args(&content, -1)?;

        let (value, rule_idx) = if parts.len() == 1 {
            // No explicit precedence, use default 0
            (0, 0)
        } else if parts.len() == 2 {
            let level_str = parts[0].trim();
            let value = self.parse_precedence_value(level_str)?;
            (value, 1)
        } else {
            bail!("prec.left() expects 1 or 2 arguments, got {}", parts.len());
        };

        let content = Box::new(self.parse_rule(&parts[rule_idx])?);

        Ok(Rule::PrecLeft { value, content })
    }

    fn parse_prec_right(&self, rule_def: &str) -> Result<Rule> {
        // prec.right(level, rule) OR prec.right(rule) - level can be numeric or a string name
        let content = self.extract_function_args(rule_def, "prec.right")?;
        let parts = self.split_args(&content, -1)?;

        let (value, rule_idx) = if parts.len() == 1 {
            // No explicit precedence, use default 0
            (0, 0)
        } else if parts.len() == 2 {
            let level_str = parts[0].trim();
            let value = self.parse_precedence_value(level_str)?;
            (value, 1)
        } else {
            bail!("prec.right() expects 1 or 2 arguments, got {}", parts.len());
        };

        let content = Box::new(self.parse_rule(&parts[rule_idx])?);

        Ok(Rule::PrecRight { value, content })
    }

    fn parse_prec_dynamic(&self, rule_def: &str) -> Result<Rule> {
        // prec.dynamic(level, rule) - level can be numeric or a string name
        let content = self.extract_function_args(rule_def, "prec.dynamic")?;
        let parts = self.split_args(&content, -1)?;

        if parts.len() != 2 {
            bail!(
                "prec.dynamic() expects exactly 2 arguments, got {}",
                parts.len()
            );
        }

        let level_str = parts[0].trim();
        let value = self.parse_precedence_value(level_str)?;

        let content = Box::new(self.parse_rule(&parts[1])?);

        Ok(Rule::PrecDynamic { value, content })
    }

    // Parse other functions
    fn parse_seq(&self, rule_def: &str) -> Result<Rule> {
        let content = self.extract_function_args(rule_def, "seq")?;

        // Handle spread operators
        if content.contains("...") {
            eprintln!(
                "Warning: Spread operator in seq not fully supported yet: {}",
                content
            );
            // Try to parse what we can
            let members = self.parse_arg_list(&content)?;
            Ok(Rule::Seq { members })
        } else {
            let members = self.parse_arg_list(&content)?;
            Ok(Rule::Seq { members })
        }
    }

    fn parse_choice(&self, rule_def: &str) -> Result<Rule> {
        let content = self.extract_function_args(rule_def, "choice")?;

        // Handle spread operators (...array)
        if content.trim().starts_with("...") {
            // For now, we'll just parse it as an empty choice
            // In a full implementation, we'd need to evaluate the JavaScript expression
            eprintln!(
                "Warning: Spread operator in choice not fully supported yet: {}",
                content
            );
            Ok(Rule::Choice { members: vec![] })
        } else {
            let members = self.parse_arg_list(&content)?;
            Ok(Rule::Choice { members })
        }
    }

    fn parse_repeat(&self, rule_def: &str) -> Result<Rule> {
        let content = self.extract_function_args(rule_def, "repeat")?;
        let content = Box::new(self.parse_arg(&content)?);
        Ok(Rule::Repeat { content })
    }

    fn parse_repeat1(&self, rule_def: &str) -> Result<Rule> {
        let content = self.extract_function_args(rule_def, "repeat1")?;
        let content = Box::new(self.parse_arg(&content)?);
        Ok(Rule::Repeat1 { content })
    }

    fn parse_optional(&self, rule_def: &str) -> Result<Rule> {
        let content = self.extract_function_args(rule_def, "optional")?;
        let value = Box::new(self.parse_arg(&content)?);
        Ok(Rule::Optional { value })
    }

    fn parse_field(&self, rule_def: &str) -> Result<Rule> {
        // field('name', rule)
        let content = self.extract_function_args(rule_def, "field")?;
        let parts = self.split_args(&content, 2)?;

        let name = self.extract_string_literal(&parts[0])?;
        let content = Box::new(self.parse_arg(&parts[1])?);

        Ok(Rule::Field { name, content })
    }

    fn parse_alias(&self, rule_def: &str) -> Result<Rule> {
        // alias(rule, 'name') or alias(rule, $.symbol) or alias(rule, 'name', named)
        let content = self.extract_function_args(rule_def, "alias")?;
        let parts = self.split_args(&content, -1)?; // Variable number of args

        if parts.len() < 2 {
            bail!("alias() requires at least 2 arguments");
        }

        let content = Box::new(self.parse_rule(&parts[0])?);

        // The second argument can be either a string literal or a symbol reference
        let value = if parts[1].trim().starts_with("$.") {
            // It's a symbol reference like $.block - extract the symbol name
            parts[1].trim()[2..].to_string()
        } else {
            // It's a string literal
            self.extract_string_literal(&parts[1])?
        };

        let named = if parts.len() > 2 {
            parts[2].trim() == "true"
        } else {
            // Default to true if the alias starts with a letter
            value.chars().next().is_some_and(|c| c.is_alphabetic())
        };

        Ok(Rule::Alias {
            content,
            value,
            named,
        })
    }

    fn parse_token(&self, rule_def: &str) -> Result<Rule> {
        let content = self.extract_function_args(rule_def, "token")?;
        let content = Box::new(self.parse_rule(&content)?);
        Ok(Rule::Token { content })
    }

    // Helper methods
    fn extract_function_args(&self, rule_def: &str, func_name: &str) -> Result<String> {
        let start = func_name.len() + 1; // Skip function name and opening paren
        if !rule_def[..start - 1].starts_with(func_name) || !rule_def[start - 1..].starts_with('(')
        {
            bail!("Expected {}(...) but got: {}", func_name, rule_def);
        }

        let content = &rule_def[start..];
        self.extract_balanced_delim(content, '(', ')')
    }

    fn split_args(&self, content: &str, expected: i32) -> Result<Vec<String>> {
        lexing::split_args(content, expected)
    }

    fn parse_rule_list(&self, content: &str) -> Result<Vec<Rule>> {
        let args = self.split_args(content, -1)?;
        let mut rules = Vec::new();

        for arg in args {
            rules.push(self.parse_rule(&arg)?);
        }

        Ok(rules)
    }

    fn parse_arg_list(&self, content: &str) -> Result<Vec<Rule>> {
        let args = self.split_args(content, -1)?;
        let mut rules = Vec::new();

        for arg in args {
            rules.push(self.parse_arg(&arg)?);
        }

        Ok(rules)
    }

    fn extract_string_literal(&self, s: &str) -> Result<String> {
        let trimmed = s.trim();
        if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        {
            Ok(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            bail!("Expected string literal, got: {}", s)
        }
    }

    fn extract_externals(&self, content: &str) -> Result<Vec<ExternalToken>> {
        // Look for externals: $ => [...]
        let externals_regex = Regex::new(r#"externals:\s*\$\s*=>\s*\["#)?;

        if let Some(mat) = externals_regex.find(content) {
            let start = mat.end();
            let end = self.find_matching_bracket(&content[start..], '[', ']')?;
            let externals_content = &content[start..start + end];

            // Parse the array of external tokens
            let args = self.split_args(externals_content, -1)?;
            let mut externals = Vec::new();

            for arg in args {
                let trimmed = arg.trim();
                if trimmed.starts_with("$.") {
                    let name = trimmed[2..].to_string();
                    externals.push(ExternalToken {
                        name: name.clone(),
                        symbol: name,
                    });
                }
            }

            Ok(externals)
        } else {
            Ok(Vec::new())
        }
    }

    fn extract_conflicts(&self, content: &str) -> Result<Vec<Vec<String>>> {
        // Look for conflicts: $ => [[...], [...]]
        let conflicts_regex = Regex::new(r#"conflicts:\s*\$\s*=>\s*\["#)?;

        if let Some(mat) = conflicts_regex.find(content) {
            let start = mat.end();
            let end = self.find_matching_bracket(&content[start..], '[', ']')?;
            let conflicts_content = &content[start..start + end];

            // Parse nested arrays
            let mut conflicts = Vec::new();
            let mut i = 0;
            let chars: Vec<char> = conflicts_content.chars().collect();

            while i < chars.len() {
                if chars[i] == '[' {
                    // Find the matching bracket
                    let sub_end =
                        self.find_matching_bracket(&conflicts_content[i + 1..], '[', ']')?;
                    let conflict_set = &conflicts_content[i + 1..i + 1 + sub_end];

                    // Parse the conflict set
                    let args = self.split_args(conflict_set, -1)?;
                    let mut set = Vec::new();

                    for arg in args {
                        let trimmed = arg.trim();
                        if trimmed.starts_with("$.") {
                            set.push(trimmed[2..].to_string());
                        }
                    }

                    if !set.is_empty() {
                        conflicts.push(set);
                    }

                    i += sub_end + 2; // Skip past the closing bracket
                } else {
                    i += 1;
                }
            }

            Ok(conflicts)
        } else {
            Ok(Vec::new())
        }
    }

    fn extract_inline(&self, content: &str) -> Result<Vec<String>> {
        // Look for inline: $ => [...]
        let inline_regex = Regex::new(r#"inline:\s*\$\s*=>\s*\["#)?;

        if let Some(mat) = inline_regex.find(content) {
            let start = mat.end();
            let end = self.find_matching_bracket(&content[start..], '[', ']')?;
            let inline_content = &content[start..start + end];

            // Parse the array
            let args = self.split_args(inline_content, -1)?;
            let mut inline = Vec::new();

            for arg in args {
                let trimmed = arg.trim();
                if trimmed.starts_with("$.") {
                    inline.push(trimmed[2..].to_string());
                }
            }

            Ok(inline)
        } else {
            Ok(Vec::new())
        }
    }

    fn extract_supertypes(&self, content: &str) -> Result<Vec<String>> {
        // Look for supertypes: $ => [...]
        let supertypes_regex = Regex::new(r#"supertypes:\s*\$\s*=>\s*\["#)?;

        if let Some(mat) = supertypes_regex.find(content) {
            let start = mat.end();
            let end = self.find_matching_bracket(&content[start..], '[', ']')?;
            let supertypes_content = &content[start..start + end];

            // Parse the array
            let args = self.split_args(supertypes_content, -1)?;
            let mut supertypes = Vec::new();

            for arg in args {
                let trimmed = arg.trim();
                if trimmed.starts_with("$.") {
                    supertypes.push(trimmed[2..].to_string());
                }
            }

            Ok(supertypes)
        } else {
            Ok(Vec::new())
        }
    }

    fn extract_precedences(&self, content: &str) -> Result<Vec<Vec<(String, i32)>>> {
        // Look for precedences: $ => [[...], [...]]
        let precedences_regex = Regex::new(r#"precedences:\s*\$\s*=>\s*\["#)?;

        if let Some(mat) = precedences_regex.find(content) {
            let start = mat.end();
            let end = self.find_matching_bracket(&content[start..], '[', ']')?;
            let precedences_content = &content[start..start + end];

            // Parse the nested array structure
            let mut result = Vec::new();

            // Split by top-level commas and parse each group
            let mut depth = 0;
            let mut current_group_start = 0;
            let mut i = 0;

            while i < precedences_content.len() {
                let ch = precedences_content.chars().nth(i).unwrap();
                match ch {
                    '[' => {
                        if depth == 0 {
                            current_group_start = i;
                        }
                        depth += 1;
                    }
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            // Found end of a group
                            let group_content = &precedences_content[current_group_start + 1..i];
                            let group = self.parse_precedence_group(group_content)?;
                            if !group.is_empty() {
                                result.push(group);
                            }
                        }
                    }
                    _ => {}
                }
                i += 1;
            }

            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    fn parse_precedence_group(&self, content: &str) -> Result<Vec<(String, i32)>> {
        // Parse a precedence group like ['call', 'member'] or ['high', 10]
        let mut items = Vec::new();
        let parts: Vec<&str> = content.split(',').collect();

        for part in parts {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check if it's a string (precedence name) or number
            if trimmed.starts_with('\'') || trimmed.starts_with('"') {
                let name = trimmed.trim_matches(|c| c == '\'' || c == '"' || c == ' ');
                items.push((name.to_string(), 0)); // 0 means "use automatic value"
            } else if let Ok(value) = trimmed.parse::<i32>() {
                // If we have a numeric value, pair it with the previous name
                if let Some((_, v)) = items.last_mut() {
                    *v = value;
                }
            }
        }

        Ok(items)
    }

    fn find_matching_bracket(&self, content: &str, open: char, close: char) -> Result<usize> {
        let mut depth = 1;
        let mut in_string = false;
        let mut in_regex = false;
        let mut escape_next = false;

        for (i, ch) in content.chars().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }

            if ch == '\\' {
                escape_next = true;
                continue;
            }

            if !in_regex && (ch == '"' || ch == '\'') {
                in_string = !in_string;
            }

            if !in_string && ch == '/' {
                in_regex = !in_regex;
            }

            if !in_string && !in_regex {
                if ch == open {
                    depth += 1;
                } else if ch == close {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(i);
                    }
                }
            }
        }

        bail!("Unbalanced {} and {}", open, close)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let content = r#"
module.exports = grammar({
  name: 'test',
  
  rules: {
    program: $ => $.expression,
    expression: $ => 'hello'
  }
})
"#;

        let mut parser = GrammarJsParserV3::new(content.to_string());
        let result = parser.parse();
        assert!(result.is_ok());

        let grammar = result.unwrap();
        assert_eq!(grammar.name, "test");
        assert_eq!(grammar.rules.len(), 2);
    }
}
