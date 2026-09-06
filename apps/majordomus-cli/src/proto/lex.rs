//! The tokenizer for the proto dialect the layer's document schemas are written in.
//!
//! Line comments are not discarded: the block of `//` lines immediately above a
//! declaration is that declaration's documentation, and it becomes the `description` of
//! the projected JSON Schema. That is the whole reason this is a hand-written lexer rather
//! than a split on whitespace — the prose beside a field is part of the contract, and a
//! reader who has to open the proto to learn what a key means has been failed by the
//! projection.

/// One token, with the comment block that preceded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    /// The token text: an identifier, a number, one punctuation character, or a quoted
    /// string including its quotes.
    pub text: &'a str,
    /// The comment block immediately above, joined with single spaces; empty when none.
    pub doc: String,
    /// 1-based line, for diagnostics.
    pub line: usize,
}

impl Token<'_> {
    /// The unquoted content when the token is a string literal, `None` otherwise.
    pub fn string(&self) -> Option<String> {
        let inner = self
            .text
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))?;
        // The dialect uses only the escapes a pattern needs.
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        }
        Some(out)
    }
}

/// The punctuation this dialect uses. Every one is a token of its own.
const PUNCTUATION: &[char] = &['{', '}', '[', ']', '(', ')', '=', ';', ',', '.', '<', '>'];

pub struct Tokenizer<'a> {
    text: &'a str,
    /// Byte offset of the next character to read.
    at: usize,
    line: usize,
    /// The comment lines gathered since the last token.
    pending: Vec<String>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(text: &'a str) -> Self {
        Tokenizer {
            text,
            at: 0,
            line: 1,
            pending: Vec::new(),
        }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn bump(&mut self, bytes: usize) -> &'a str {
        let taken = &self.text[self.at..self.at + bytes];
        self.line += taken.matches('\n').count();
        self.at += bytes;
        taken
    }

    /// Whitespace and comments, up to the next real token. A blank line ends a comment
    /// block, so prose that belongs to the file header does not attach to the first
    /// declaration below it.
    fn trivia(&mut self) {
        loop {
            let rest = self.rest();
            if rest.is_empty() {
                return;
            }
            if let Some(c) = rest.chars().next() {
                if c.is_whitespace() {
                    let newlines_before = self.line;
                    let len = c.len_utf8();
                    self.bump(len);
                    // two newlines in a row: the comment block above is not this
                    // declaration's
                    if c == '\n' && self.rest().starts_with('\n') && self.line > newlines_before {
                        self.pending.clear();
                    }
                    continue;
                }
            }
            if let Some(after) = rest.strip_prefix("//") {
                let len = after.find('\n').unwrap_or(after.len());
                let comment = &after[..len];
                self.bump(2 + len);
                self.pending.push(comment.trim().to_string());
                continue;
            }
            if let Some(after) = rest.strip_prefix("/*") {
                let len = after.find("*/").map(|i| i + 4).unwrap_or(rest.len());
                self.bump(len);
                continue;
            }
            return;
        }
    }

    fn take_doc(&mut self) -> String {
        let doc = self
            .pending
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        self.pending.clear();
        doc
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        self.trivia();
        let rest = self.rest();
        let first = rest.chars().next()?;
        let line = self.line;

        if first == '"' {
            // to the closing quote, honouring a backslash escape
            let bytes = rest.as_bytes();
            let mut i = 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            let text = self.bump(i.min(rest.len()));
            let doc = self.take_doc();
            return Some(Token { text, doc, line });
        }

        if PUNCTUATION.contains(&first) {
            let text = self.bump(first.len_utf8());
            let doc = self.take_doc();
            return Some(Token { text, doc, line });
        }

        // an identifier or a number: to the next whitespace or punctuation
        let len = rest
            .find(|c: char| c.is_whitespace() || PUNCTUATION.contains(&c))
            .unwrap_or(rest.len());
        let text = self.bump(len.max(1));
        let doc = self.take_doc();
        Some(Token { text, doc, line })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_punctuation_from_words() {
        let tokens: Vec<_> = Tokenizer::new("message Header { string id = 1; }")
            .map(|t| t.text)
            .collect();
        assert_eq!(
            tokens,
            ["message", "Header", "{", "string", "id", "=", "1", ";", "}"]
        );
    }

    #[test]
    fn a_comment_block_attaches_to_the_declaration_below_it() {
        let mut it = Tokenizer::new("// one\n// two\nstring id = 1;");
        let first = it.next().unwrap();
        assert_eq!(first.text, "string");
        assert_eq!(first.doc, "one two");
    }

    #[test]
    fn a_blank_line_ends_a_comment_block() {
        let mut it = Tokenizer::new("// a file header\n\nstring id = 1;");
        assert_eq!(it.next().unwrap().doc, "");
    }

    #[test]
    fn reads_a_string_with_escapes_and_keeps_a_pattern_intact() {
        let mut it = Tokenizer::new(r#""^adr-[0-9]{4}$""#);
        assert_eq!(it.next().unwrap().string().unwrap(), "^adr-[0-9]{4}$");
        let mut it = Tokenizer::new(r#""a\"b""#);
        assert_eq!(it.next().unwrap().string().unwrap(), "a\"b");
    }

    #[test]
    fn counts_lines_for_diagnostics() {
        let mut it = Tokenizer::new("a\n\n\nb");
        assert_eq!(it.next().unwrap().line, 1);
        assert_eq!(it.next().unwrap().line, 4);
    }
}
