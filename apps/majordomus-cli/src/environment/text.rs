//! Terminal text: making repository metadata safe to print, and measuring it in columns
//! rather than in bytes.
//!
//! Everything the banner shows comes out of the repository — a branch name, a recipe's doc
//! comment, a directory name — and none of it is trusted. A branch really can be called
//! `$(rm -rf /)`, and a doc comment really can carry an escape sequence that repaints the
//! terminal, moves the cursor, or sets the window title. [`sanitise`] is what stands
//! between that and a person's shell.
//!
//! Width is the other half. A box drawn by counting bytes falls apart on the first
//! non-ASCII character, and a box drawn by counting `char`s falls apart on the first
//! combining mark or CJK name. [`width`] counts columns.

/// The replacement for a character that may not be printed. A space, so that a name with
/// something nasty in it still lines up rather than shifting the layout.
const REPLACEMENT: char = ' ';

/// Make one value safe to print in a terminal.
///
/// Removes every C0 and C1 control character, the escape that introduces an ANSI sequence
/// along with the sequence itself, and the Unicode characters that reorder or hide text:
/// the bidirectional overrides, the zero-width joiners at the start of a value, and the
/// deprecated tag characters that terminals may swallow entirely. Tabs and newlines become
/// spaces. Everything else — every language, every emoji — is left exactly as it was.
///
/// ```
/// use majordomus_cli::environment::text::sanitise;
/// assert_eq!(sanitise("feature/přehled"), "feature/přehled");
/// assert_eq!(sanitise("a\u{1b}[31mred\u{1b}[0m"), "ared");
/// assert_eq!(sanitise("two\nlines\tin one"), "two lines in one");
/// assert_eq!(sanitise("\u{202e}gnorts"), "gnorts", "a right-to-left override is removed");
/// assert_eq!(sanitise("\u{7}bell"), "bell");
/// ```
pub fn sanitise(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // ESC: drop it and the sequence it introduces. CSI (`\x1b[`) runs to a byte in
            // 0x40..=0x7e; OSC (`\x1b]`) runs to BEL or ST. Anything else is a two-byte
            // escape, so dropping the next character is right.
            '\u{1b}' => match chars.peek() {
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            },
            '\t' | '\n' | '\r' => out.push(REPLACEMENT),
            // C0 and C1 controls, and DEL.
            c if c.is_control() => {}
            // Bidirectional overrides and embeddings, the invisible operators, and the
            // deprecated tag block: text that renders as something other than what it is.
            '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => {}
            '\u{2060}'..='\u{2064}' | '\u{feff}' | '\u{e0000}'..='\u{e007f}' => {}
            c => out.push(c),
        }
    }
    out
}

/// The width of one character in terminal columns: zero for a combining mark, two for the
/// wide and fullwidth blocks, one otherwise.
///
/// An approximation of Unicode's East Asian Width, held to the ranges that actually reach
/// a repository's metadata: the CJK and Hangul blocks, the fullwidth forms, and the emoji
/// that terminals draw double. It is deliberately a table here rather than a dependency —
/// the alternative is a crate with its own Unicode database for the sake of drawing a box.
pub fn char_width(c: char) -> usize {
    match c {
        // combining marks, variation selectors, zero-width joiners: no column of their own
        '\u{0300}'..='\u{036f}'
        | '\u{200b}'..='\u{200f}'
        | '\u{fe00}'..='\u{fe0f}'
        | '\u{feff}' => 0,
        c if c.is_control() => 0,
        // Hangul Jamo, CJK, Kangxi, Hiragana/Katakana, Hangul syllables, CJK compatibility
        '\u{1100}'..='\u{115f}'
        | '\u{2e80}'..='\u{303e}'
        | '\u{3041}'..='\u{33ff}'
        | '\u{3400}'..='\u{4dbf}'
        | '\u{4e00}'..='\u{9fff}'
        | '\u{a000}'..='\u{a4cf}'
        | '\u{ac00}'..='\u{d7a3}'
        | '\u{f900}'..='\u{faff}'
        | '\u{fe30}'..='\u{fe6f}'
        | '\u{ff00}'..='\u{ff60}'
        | '\u{ffe0}'..='\u{ffe6}' => 2,
        // the emoji blocks terminals draw double
        '\u{1f300}'..='\u{1f64f}'
        | '\u{1f900}'..='\u{1f9ff}'
        | '\u{1f680}'..='\u{1f6ff}'
        | '\u{20000}'..='\u{3fffd}' => 2,
        _ => 1,
    }
}

/// The width of a string in terminal columns.
///
/// ```
/// use majordomus_cli::environment::text::width;
/// assert_eq!(width("majordomus"), 10);
/// assert_eq!(width("přehled"), 7, "a two-byte character is one column");
/// assert_eq!(width("日本語"), 6, "a wide character is two");
/// assert_eq!(width("e\u{0301}"), 1, "a combining accent adds none");
/// assert_eq!(width("◆ ✓ ✱"), 5);
/// ```
pub fn width(value: &str) -> usize {
    value.chars().map(char_width).sum()
}

/// Cut a string to at most `columns` display columns, ending with `…` when anything was
/// removed. Never splits a character, and never returns something wider than it was asked
/// for — including the ellipsis.
///
/// ```
/// use majordomus_cli::environment::text::{truncate, width};
/// assert_eq!(truncate("majordomus", 10), "majordomus");
/// assert_eq!(truncate("majordomus", 6), "major…");
/// assert_eq!(truncate("日本語です", 5), "日本…");
/// assert_eq!(width(&truncate("日本語です", 5)), 5, "and exactly as wide as asked");
/// assert_eq!(truncate("abc", 0), "");
/// ```
pub fn truncate(value: &str, columns: usize) -> String {
    truncate_with(value, columns, "…")
}

/// [`truncate`], with the mark that says something was removed given explicitly. A
/// terminal that cannot render `…` gets `...`, and the width arithmetic follows the mark
/// it was actually given rather than assuming one column.
///
/// ```
/// use majordomus_cli::environment::text::{truncate_with, width};
/// assert_eq!(truncate_with("majordomus", 6, "..."), "maj...");
/// assert_eq!(width(&truncate_with("majordomus", 6, "...")), 6);
/// assert_eq!(truncate_with("majordomus", 2, "..."), "ma", "no room for the mark");
/// ```
pub fn truncate_with(value: &str, columns: usize, ellipsis: &str) -> String {
    if width(value) <= columns {
        return value.to_string();
    }
    if columns == 0 {
        return String::new();
    }
    // With no room for the mark as well as a character, the mark is dropped: a cell
    // holding nothing but `...` says less than one holding the first two letters.
    let mark = if width(ellipsis) < columns {
        ellipsis
    } else {
        ""
    };
    let budget = columns.saturating_sub(width(mark));
    let mut out = String::new();
    let mut used = 0;
    for c in value.chars() {
        let w = char_width(c);
        if used + w > budget {
            break;
        }
        used += w;
        out.push(c);
    }
    out.push_str(mark);
    out
}

/// Pad a string on the right to `columns` display columns, cutting it if it is longer.
///
/// ```
/// use majordomus_cli::environment::text::{pad, width};
/// assert_eq!(pad("ok", 5), "ok   ");
/// assert_eq!(width(&pad("日本語", 8)), 8);
/// ```
pub fn pad(value: &str, columns: usize) -> String {
    pad_with(value, columns, "…")
}

/// [`pad`], with the truncation mark given explicitly.
pub fn pad_with(value: &str, columns: usize, ellipsis: &str) -> String {
    let cut = truncate_with(value, columns, ellipsis);
    let w = width(&cut);
    format!("{cut}{}", " ".repeat(columns.saturating_sub(w)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The attack this module exists for: a branch name is arbitrary bytes chosen by
    /// whoever made the branch, and the banner prints it into a terminal on every `cd`.
    #[test]
    fn a_hostile_branch_name_cannot_reach_the_terminal() {
        let hostile = "\u{1b}]0;pwned\u{7}\u{1b}[2J\u{1b}[Hfeature/x";
        let safe = sanitise(hostile);
        assert_eq!(safe, "feature/x");
        assert!(!safe.contains('\u{1b}'));
        assert!(!safe.chars().any(char::is_control));
    }

    #[test]
    fn a_sanitised_value_never_carries_a_control_character_whatever_it_was() {
        for hostile in [
            "\u{1b}[38;5;196mred",
            "a\u{0}b",
            "\u{1b}]8;;http://evil.test\u{7}link\u{1b}]8;;\u{7}",
            "line\r\nbreak",
            "\u{1b}",
            "\u{1b}c",
        ] {
            let safe = sanitise(hostile);
            assert!(
                !safe.chars().any(|c| c.is_control()),
                "{hostile:?} left a control character in {safe:?}"
            );
        }
    }

    #[test]
    fn ordinary_text_of_any_language_survives_untouched() {
        for ok in [
            "master",
            "feature/repository-environment",
            "Přehled prostředí",
            "日本語のブランチ",
            "emoji 🚀 branch",
        ] {
            assert_eq!(sanitise(ok), ok);
        }
    }

    #[test]
    fn width_is_columns_and_never_bytes() {
        assert_ne!(width("日本語"), "日本語".len());
        assert_eq!(width("日本語"), 6);
        assert_eq!(width(""), 0);
    }

    #[test]
    fn padding_a_wide_string_produces_the_width_it_promised() {
        for value in ["", "a", "日本語", "přehled", "◆ majordomus"] {
            for columns in [0, 1, 4, 12, 40] {
                assert_eq!(
                    width(&pad(value, columns)),
                    columns,
                    "pad({value:?}, {columns}) is not {columns} columns wide"
                );
            }
        }
    }

    #[test]
    fn truncation_never_exceeds_the_width_it_was_given() {
        for value in [
            "short",
            "a rather longer value than the budget",
            "日本語です",
            "🚀🚀🚀🚀",
        ] {
            for columns in [0, 1, 2, 3, 7, 20] {
                assert!(
                    width(&truncate(value, columns)) <= columns,
                    "truncate({value:?}, {columns}) is wider than {columns}"
                );
            }
        }
    }

    #[test]
    fn truncation_leaves_a_string_that_fits_alone() {
        assert_eq!(truncate("exact", 5), "exact");
        assert_eq!(truncate("exact", 6), "exact");
    }
}
