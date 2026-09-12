//! A whole commit message: the header, the body, and the trailers under it.
//!
//! The message is the *rendering*. What a worker states is a [`CommitMessage`]'s fields — a kind,
//! a scope, what it did, what it refers to — and the text git stores is derived from them.
//! That direction matters: the same value renders the commit, answers what a plan intends
//! to do before anything is written, and is what a validator judges after it is. A free-form
//! string as the canonical representation could do only the last of those.
//!
//! ```
//! use majordomus_cli::commit::{CommitHeader, CommitMessage};
//! use majordomus_cli::release::ChangeKind;
//!
//! let m = CommitMessage::parse("fix(commit): the trailer is read\n\nWhy it was wrong.\n\nRefs: I1305\n");
//! assert_eq!(m.header.kind, ChangeKind::Fix);
//! assert_eq!(m.body, "Why it was wrong.");
//! assert_eq!(m.trailer("Refs"), Some("I1305"));
//! // and the value renders back to the message it was read from
//! assert!(m.render().starts_with("fix(commit): the trailer is read\n\nWhy it was wrong."));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::CommitHeader;

/// One `Key: value` line in the trailer block at the end of a commit message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommitTrailer {
    /// The key, as written: `Refs`, `Co-Authored-By`, `BREAKING CHANGE`.
    pub key: String,
    /// Everything after the colon and one space.
    pub value: String,
}

/// A commit message taken apart: header, body, trailers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommitMessage {
    /// The first line, parsed.
    pub header: CommitHeader,
    /// Everything between the header and the trailer block, with surrounding blank lines
    /// removed. Empty when the message is a subject alone.
    pub body: String,
    /// The `Key: value` lines of the final paragraph, in the order they were written.
    pub trailers: Vec<CommitTrailer>,
}

/// Whether a line is a trailer: `Key: value`, where the key is a word of letters, digits
/// and hyphens, or one of the two spellings of the breaking-change trailer.
///
/// Deliberately narrower than git's own interpretation. Git will treat almost any
/// `word: rest` as a trailer if it is in the last paragraph, which turns the last sentence
/// of a body — `The reason: it was wrong.` — into a trailer with the key `The reason`. The
/// keys this repository writes are single words, and a paragraph that does not consist
/// entirely of them stays a body.
fn trailer_of(line: &str) -> Option<CommitTrailer> {
    if let Some(rest) = line
        .strip_prefix("BREAKING CHANGE:")
        .or_else(|| line.strip_prefix("BREAKING-CHANGE:"))
    {
        return Some(CommitTrailer {
            key: "BREAKING CHANGE".into(),
            value: rest.trim().to_string(),
        });
    }
    let (key, value) = line.split_once(':')?;
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some(CommitTrailer {
        key: key.to_string(),
        value: value.trim().to_string(),
    })
}

impl CommitMessage {
    /// Read a whole commit message. Total, like [`CommitHeader::parse`]: what cannot be read as a
    /// header or a trailer is body, and nothing is an error.
    pub fn parse(text: &str) -> CommitMessage {
        // A message given to `git commit` may carry comment lines; git strips them and so
        // does this, so that what is validated is what will be stored.
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.trim_end())
            .collect();
        let header = CommitHeader::parse(lines.first().copied().unwrap_or_default());
        let rest = lines.get(1..).unwrap_or_default();
        // The trailer block is the last paragraph, and only when every line of it is a
        // trailer. Anything else is body.
        let paragraphs: Vec<&[&str]> = rest
            .split(|l| l.trim().is_empty())
            .filter(|p| !p.is_empty())
            .collect();
        let mut trailers = Vec::new();
        let mut body_paragraphs: Vec<String> = Vec::new();
        for (i, para) in paragraphs.iter().enumerate() {
            let last = i + 1 == paragraphs.len();
            let all: Option<Vec<CommitTrailer>> = para.iter().map(|l| trailer_of(l)).collect();
            match (last, all) {
                (true, Some(ts)) if !ts.is_empty() => trailers = ts,
                _ => body_paragraphs.push(para.join("\n")),
            }
        }
        CommitMessage {
            header,
            body: body_paragraphs.join("\n\n"),
            trailers,
        }
    }

    /// The value of the first trailer with this key, compared case-insensitively.
    pub fn trailer(&self, key: &str) -> Option<&str> {
        self.trailers
            .iter()
            .find(|t| t.key.eq_ignore_ascii_case(key))
            .map(|t| t.value.as_str())
    }

    /// Whether this commit declares itself breaking, by either spelling: the `!` in the
    /// header or the trailer in the message.
    ///
    /// ```
    /// use majordomus_cli::commit::CommitMessage;
    /// assert!(CommitMessage::parse("feat(api)!: the route moved").breaking());
    /// assert!(CommitMessage::parse("feat(api): moved\n\nBREAKING CHANGE: it moved").breaking());
    /// assert!(!CommitMessage::parse("feat(api): moved").breaking());
    /// ```
    pub fn breaking(&self) -> bool {
        self.header.breaking || self.trailer("BREAKING CHANGE").is_some()
    }

    /// Render the message as git will store it: header, blank line, body, blank line,
    /// trailers, one final newline.
    pub fn render(&self) -> String {
        let mut out = self.header.render();
        if !self.body.is_empty() {
            out.push_str("\n\n");
            out.push_str(self.body.trim_end());
        }
        if !self.trailers.is_empty() {
            out.push_str("\n\n");
            for t in &self.trailers {
                out.push_str(&t.key);
                out.push_str(": ");
                out.push_str(&t.value);
                out.push('\n');
            }
            return out;
        }
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::ChangeKind;

    #[test]
    fn a_message_is_header_body_and_trailers() {
        let m = CommitMessage::parse(
            "feat(commit): the message is a value\n\nFirst paragraph.\n\nSecond paragraph.\n\nRefs: I1305\nCo-Authored-By: Someone <s@example.org>\n",
        );
        assert_eq!(m.header.kind, ChangeKind::Feat);
        assert_eq!(m.body, "First paragraph.\n\nSecond paragraph.");
        assert_eq!(m.trailers.len(), 2);
        assert_eq!(m.trailer("refs"), Some("I1305"));
        assert_eq!(m.trailer("Co-Authored-By"), Some("Someone <s@example.org>"));
    }

    #[test]
    fn a_last_paragraph_of_prose_is_body_and_not_trailers() {
        // Git would read `The reason` as a trailer key here. A commit body that ends in a
        // sentence containing a colon is normal writing, and turning it into metadata
        // would make the validator report a key nobody wrote.
        let m = CommitMessage::parse("fix(x): a thing\n\nThe reason: it was wrong.");
        assert!(m.trailers.is_empty());
        assert_eq!(m.body, "The reason: it was wrong.");
    }

    #[test]
    fn a_subject_alone_is_a_message() {
        let m = CommitMessage::parse("docs: one line");
        assert_eq!(m.body, "");
        assert!(m.trailers.is_empty());
        assert_eq!(m.render(), "docs: one line\n");
    }

    #[test]
    fn comment_lines_are_not_part_of_the_message() {
        // What is judged must be what git will store, or the hook passes a message that
        // the repository then rejects — or worse, the other way around.
        let m = CommitMessage::parse("fix(x): a thing\n# Please enter the commit message\n\nBody.\n");
        assert_eq!(m.header.subject, "a thing");
        assert_eq!(m.body, "Body.");
    }

    #[test]
    fn a_message_survives_the_round_trip() {
        let text = "feat(commit)!: the value renders\n\nWhy.\n\nRefs: I1305\n";
        assert_eq!(CommitMessage::parse(text).render(), text);
    }

    #[test]
    fn both_spellings_of_breaking_are_read() {
        assert!(CommitMessage::parse("feat(api)!: moved").breaking());
        assert!(CommitMessage::parse("feat(api): moved\n\nBREAKING CHANGE: why").breaking());
        assert!(CommitMessage::parse("feat(api): moved\n\nBREAKING-CHANGE: why").breaking());
        assert!(!CommitMessage::parse("feat(api): moved\n\nRefs: I1305").breaking());
    }
}
