use anyhow::{Result, bail};

/// Extracts content until the matching delimiter while respecting strings and regex literals.
pub(super) fn extract_balanced_delim(content: &str, open: char, close: char) -> Result<String> {
    let chars: Vec<char> = content.chars().collect();
    let mut depth = 1;
    let mut pos = 0;

    while pos < chars.len() && depth > 0 {
        let ch = chars[pos];

        if ch == '\'' || ch == '"' || ch == '`' {
            pos = skip_quoted_literal(&chars, pos, ch);
        } else if ch == '/' && pos + 1 < chars.len() && is_regex_literal_start(&chars, pos) {
            pos = skip_regex_literal(&chars, pos);
        } else {
            if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
            }
            pos += 1;
        }
    }

    if depth == 0 {
        // Quote/regex skipping uses character cursors. Resolve the closing
        // character back to the original source's byte boundary before slicing.
        pos.checked_sub(1)
            .and_then(|closing| content.char_indices().nth(closing))
            .and_then(|(byte, _)| content.get(..byte))
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("Invalid closing delimiter boundary"))
    } else {
        bail!("Unbalanced {} and {} in content", open, close)
    }
}

/// Splits top-level comma-separated arguments while preserving nested expressions.
pub(super) fn split_args(content: &str, expected: i32) -> Result<Vec<String>> {
    let mut splitter = ArgSplitter::default();

    for ch in content.chars() {
        splitter.accept(ch);
    }

    let args = splitter.finish();
    if expected > 0 && args.len() != expected as usize {
        bail!("Expected {} arguments, got {}", expected, args.len());
    }

    Ok(args)
}

fn skip_quoted_literal(chars: &[char], start: usize, quote: char) -> usize {
    let mut pos = start + 1;

    while pos < chars.len() {
        if chars[pos] == '\\' {
            pos += 2;
        } else if chars[pos] == quote {
            return pos + 1;
        } else {
            pos += 1;
        }
    }

    pos
}

fn is_regex_literal_start(chars: &[char], pos: usize) -> bool {
    pos > 0
        && "[,({:;=\n ".contains(chars[pos - 1])
        && chars[pos + 1] != '/'
        && chars[pos + 1] != '*'
}

fn skip_regex_literal(chars: &[char], start: usize) -> usize {
    let mut pos = start + 1;

    while pos < chars.len() {
        if chars[pos] == '\\' {
            pos += 2;
        } else if chars[pos] == '/' {
            return pos + 1;
        } else {
            pos += 1;
        }
    }

    pos
}

#[derive(Default)]
struct ArgSplitter {
    args: Vec<String>,
    current: String,
    depth: i32,
    in_string: bool,
    string_char: char,
    escape_next: bool,
}

impl ArgSplitter {
    fn accept(&mut self, ch: char) {
        if self.escape_next {
            self.escape_next = false;
            self.current.push(ch);
        } else if ch == '\\' {
            self.escape_next = true;
            self.current.push(ch);
        } else if !self.in_string && (ch == '\'' || ch == '"' || ch == '`') {
            self.in_string = true;
            self.string_char = ch;
            self.current.push(ch);
        } else if self.in_string && ch == self.string_char {
            self.in_string = false;
            self.current.push(ch);
        } else if !self.in_string {
            self.accept_unquoted(ch);
        } else {
            self.current.push(ch);
        }
    }

    fn accept_unquoted(&mut self, ch: char) {
        match ch {
            '(' | '[' | '{' => {
                self.depth += 1;
                self.current.push(ch);
            }
            ')' | ']' | '}' => {
                self.depth -= 1;
                self.current.push(ch);
            }
            ',' if self.depth == 0 => self.flush_current(),
            _ => self.current.push(ch),
        }
    }

    fn flush_current(&mut self) {
        self.args.push(self.current.trim().to_string());
        self.current.clear();
    }

    fn finish(mut self) -> Vec<String> {
        if !self.current.trim().is_empty() {
            self.flush_current();
        }
        self.args
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_balanced_delim, split_args};

    #[test]
    fn splits_only_top_level_arguments() {
        let args = split_args("$.left, choice(',', seq($.middle, $.right)), $.tail", 3).unwrap();

        assert_eq!(
            args,
            vec!["$.left", "choice(',', seq($.middle, $.right))", "$.tail"]
        );
    }

    #[test]
    fn extracts_balanced_delimiter_content_around_nested_syntax() {
        let content = "seq($.left, choice(')', /a\\)/), $.right))";

        let extracted = extract_balanced_delim(content, '(', ')').unwrap();

        assert_eq!(extracted, "seq($.left, choice(')', /a\\)/), $.right)");
    }
    #[test]
    fn test_extract_unicode_delimiter_content_preserves_byte_boundaries() {
        assert_eq!(extract_balanced_delim("€)", '(', ')').unwrap(), "€");
        let content = "seq('€)', /é\\)/))";
        assert_eq!(
            extract_balanced_delim(content, '(', ')').unwrap(),
            "seq('€)', /é\\)/)"
        );
    }
}
