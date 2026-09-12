//! The conventional-commit grammar: one parser, one renderer, one place.
//!
//! ```text
//!   feat(commit)!: the header is parsed once
//!   ^^^^ ^^^^^^ ^  ^^^^^^^^^^^^^^^^^^^^^^^^
//!   word scope  breaking            subject
//! ```
//!
//! # Why the parse is total
//!
//! `git log` returns what the history holds, and the history holds merges, reverts, and
//! subjects nobody spelled conventionally. A parser that returned an error for those would
//! push every caller into deciding what to do with a commit that exists, and the changelog
//! already made that decision once: an entry it cannot classify is carried whole rather
//! than dropped, because a changelog that omits what it cannot parse lies about what
//! happened.
//!
//! So [`CommitHeader::parse`] never fails. It answers with what it found, and
//! [`CommitHeader::is_conventional`] is the separate question a *judge* asks. The changelog reads
//! the first; [`crate::commit::verdict`] reads the second. Two verdicts, one grammar — which
//! is the whole reason this module exists rather than a second regular expression in the
//! gate that checks commits.
//!
//! ```
//! use majordomus_cli::commit::CommitHeader;
//! use majordomus_cli::release::ChangeKind;
//!
//! let h = CommitHeader::parse("feat(commit)!: the header is parsed once");
//! assert_eq!(h.kind, ChangeKind::Feat);
//! assert_eq!(h.scope.as_deref(), Some("commit"));
//! assert!(h.breaking);
//! assert_eq!(h.subject, "the header is parsed once");
//! assert!(h.is_conventional());
//!
//! // a merge commit is not conventional, and is not damaged by being read
//! let m = CommitHeader::parse("Merge pull request #249 from korczis/fix/pipelines");
//! assert!(!m.is_conventional());
//! assert_eq!(m.subject, "Merge pull request #249 from korczis/fix/pipelines");
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::release::ChangeKind;

/// The header of a commit message: its first line, taken apart.
///
/// `word` is the type as the author wrote it and `kind` is what that word names; they
/// differ exactly when the word is not one of the eleven the convention defines, and
/// keeping both is what lets a reader see `wip(site): halfway through` as its author typed
/// it while a judge still reports that `wip` is not a type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommitHeader {
    /// The type as written: `feat`, `fix`, `wip`, or the empty string when the subject
    /// carries no `type: ` head at all.
    pub word: String,
    /// What [`Self::word`] names; [`ChangeKind::Other`] for a word the convention does not
    /// define, and for a subject with no head.
    pub kind: ChangeKind,
    /// The parenthesised scope, when there is one.
    pub scope: Option<String>,
    /// Whether the head carried the `!` that marks a breaking change. The `BREAKING CHANGE:`
    /// trailer is a property of the body and is read by [`crate::commit::CommitMessage`].
    pub breaking: bool,
    /// What the commit says it did. For a header that does not parse this is the whole
    /// first line, unchanged, and for an unknown type word it keeps the word: nothing that
    /// the author wrote is dropped on the way through.
    pub subject: String,
}

impl CommitHeader {
    /// Read one subject line. Never fails; see the module documentation for why.
    pub fn parse(subject: &str) -> CommitHeader {
        let line = subject.trim_end_matches(['\r', '\n']);
        let Some((head, rest)) = line.split_once(": ") else {
            return CommitHeader {
                word: String::new(),
                kind: ChangeKind::Other,
                scope: None,
                breaking: false,
                subject: line.trim().to_string(),
            };
        };
        let head = head.trim();
        // A head with whitespace in it was never a type word; `Merge branch 'x': into y`
        // splits on `: ` and must not be read as the type `Merge branch 'x'`.
        if head.is_empty() || head.contains(char::is_whitespace) {
            return CommitHeader {
                word: String::new(),
                kind: ChangeKind::Other,
                scope: None,
                breaking: false,
                subject: line.trim().to_string(),
            };
        }
        let breaking = head.ends_with('!');
        let head = head.trim_end_matches('!');
        let (word, scope) = match head.split_once('(') {
            Some((w, s)) => (w, s.strip_suffix(')').map(str::to_string)),
            None => (head, None),
        };
        let kind = ChangeKind::parse(word);
        let subject = if kind == ChangeKind::Other {
            // The word is what the author said about the change; hiding it says less.
            format!("{word}: {}", rest.trim())
        } else {
            rest.trim().to_string()
        };
        CommitHeader {
            word: word.to_string(),
            kind,
            scope,
            breaking,
            subject,
        }
    }

    /// Whether this header is a conventional commit: a type word the convention defines,
    /// followed by an optional scope, an optional `!`, `: `, and a subject.
    ///
    /// ```
    /// use majordomus_cli::commit::CommitHeader;
    /// assert!(CommitHeader::parse("docs: the page the recipes cite").is_conventional());
    /// assert!(!CommitHeader::parse("update stuff").is_conventional());
    /// assert!(!CommitHeader::parse("wip(site): halfway").is_conventional());
    /// ```
    pub fn is_conventional(&self) -> bool {
        self.kind != ChangeKind::Other && !self.subject.is_empty()
    }

    /// Render a header back to the line it came from.
    ///
    /// The round trip is exact for every conventional header, which is the property that
    /// lets a planner build a header, a validator judge the rendered line, and both be
    /// talking about the same commit.
    ///
    /// ```
    /// use majordomus_cli::commit::CommitHeader;
    /// for line in ["feat(commit)!: derived", "fix: one thing", "perf(http): fewer reads"] {
    ///     assert_eq!(CommitHeader::parse(line).render(), line);
    /// }
    /// ```
    pub fn render(&self) -> String {
        if self.word.is_empty() {
            return self.subject.clone();
        }
        let scope = match &self.scope {
            Some(s) => format!("({s})"),
            None => String::new(),
        };
        let bang = if self.breaking { "!" } else { "" };
        format!("{}{scope}{bang}: {}", self.word, self.subject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_of_a_conventional_header_are_separated() {
        let h = CommitHeader::parse("feat(commands): one canonical command graph");
        assert_eq!(h.kind, ChangeKind::Feat);
        assert_eq!(h.word, "feat");
        assert_eq!(h.scope.as_deref(), Some("commands"));
        assert_eq!(h.subject, "one canonical command graph");
        assert!(!h.breaking);
        assert!(h.is_conventional());
    }

    #[test]
    fn a_scope_is_optional_and_a_bang_is_read() {
        let h = CommitHeader::parse("docs: the document the env recipes cite");
        assert_eq!(h.kind, ChangeKind::Docs);
        assert_eq!(h.scope, None);
        assert!(CommitHeader::parse("feat(api)!: the route moved").breaking);
        assert!(CommitHeader::parse("feat!: the route moved").breaking);
        assert_eq!(CommitHeader::parse("feat!: the route moved").word, "feat");
    }

    #[test]
    fn a_subject_that_is_not_conventional_is_kept_whole() {
        // The property the changelog depends on: an entry it cannot classify is carried,
        // not dropped, and is not split at a colon that was part of the sentence.
        for line in [
            "Merge pull request #117 from korczis/int",
            "Revert: the thing that broke",
            "update stuff",
        ] {
            let h = CommitHeader::parse(line);
            assert_eq!(h.kind, ChangeKind::Other, "{line}");
            assert_eq!(h.subject, line, "{line}");
            assert!(!h.is_conventional(), "{line}");
        }
    }

    #[test]
    fn an_unknown_type_word_keeps_the_word_and_its_scope() {
        let h = CommitHeader::parse("wip(site): halfway through");
        assert_eq!(h.kind, ChangeKind::Other);
        assert_eq!(h.word, "wip");
        assert_eq!(h.scope.as_deref(), Some("site"));
        assert_eq!(h.subject, "wip: halfway through");
        assert!(!h.is_conventional());
    }

    #[test]
    fn a_head_with_a_space_in_it_was_never_a_type_word() {
        // `git merge` writes subjects that contain `: `; reading the left half as a type
        // would make every merge in the history a commit with a scope.
        let h = CommitHeader::parse("Merge branch 'master' of gitlab.com: into feature/x");
        assert_eq!(h.word, "");
        assert_eq!(h.scope, None);
        assert_eq!(
            h.subject,
            "Merge branch 'master' of gitlab.com: into feature/x"
        );
    }

    #[test]
    fn every_conventional_header_survives_the_round_trip() {
        for line in [
            "feat(commit)!: the header is parsed once",
            "fix: one thing",
            "chore(deps): bump the toolchain",
            "refactor!: the module moved",
        ] {
            assert_eq!(CommitHeader::parse(line).render(), line, "{line}");
        }
        // and a line that is not a header renders as itself
        let odd = "Merge pull request #1 from a/b";
        assert_eq!(CommitHeader::parse(odd).render(), odd);
    }
}
