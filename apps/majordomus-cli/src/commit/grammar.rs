//! The grammar a repository declares for its commit headers.
//!
//! The conventional grammar is [`CommitHeader::parse`]'s, and it stays the default: a policy
//! that says nothing about a grammar is judged exactly as before. A repository whose history
//! follows another grammar — the one measured first was a fork that keeps its upstream's
//! `<Keyword>( #<issue>|<commit>, ...)?: [<Component>] <Details>` — declares it under
//! `commit.grammar`, and the judge, the history gate and the scope vocabulary read the same
//! declaration. Without it such a repository cannot wire the `commit-msg` hook at all, because
//! every one of its valid messages reads as `commit.not_conventional`, and a defect in a
//! message is then found by the forge after the push, when fixing it means superseding the
//! branch.
//!
//! The keyword style is the upstream checker's grammar, encoded rule for rule rather than as a
//! regular expression, so that a finding can say which part is wrong:
//!
//! - the header is `<prefixes>: <details>`, split at the first `": "`;
//! - the prefixes are separated by `", "`, and each one is `Word #N`, `Word <hash>`, a bare
//!   `Word` only when it is the only prefix, or a bare `<hash>` only after the first;
//! - a hash is four or more lowercase hexadecimal digits;
//! - the first prefix's word is the commit's kind, through the declared `words` table;
//! - `[X] ` at the start of the details is the component, which this layer reads as the scope.
//!
//! ```
//! use majordomus_cli::commit::grammar::CommitGrammar;
//! use majordomus_cli::release::ChangeKind;
//!
//! let g: CommitGrammar = serde_json::from_str(
//!     r#"{"style": "keyword", "words": [{"word": "Fix", "kind": "fix"}, {"word": "Feature", "kind": "feat"}, {"word": "Codechange", "kind": "refactor"}]}"#,
//! ).expect("a keyword grammar");
//! let h = g.parse("Fix #12: [YAPF] Infinite loop in pathfinder");
//! assert_eq!(h.kind, ChangeKind::Fix);
//! assert_eq!(h.scope.as_deref(), Some("YAPF"));
//! assert_eq!(h.subject, "Infinite loop in pathfinder");
//! assert!(g.defect("Fix #12: [YAPF] Infinite loop in pathfinder").is_none());
//! assert!(g.defect("Test: [YAPF] Infinite loop").is_some(), "Test is not a declared word");
//! // and saying nothing is the conventional grammar, unchanged
//! assert_eq!(CommitGrammar::default(), CommitGrammar::Conventional);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::header::CommitHeader;
use super::policy::git_authored;
use crate::release::ChangeKind;

/// `commit.grammar` in the repository policy: which grammar a commit header is written in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(tag = "style", rename_all = "lowercase")]
/// ```
/// use majordomus_cli::commit::grammar::CommitGrammar;
/// let c: CommitGrammar = serde_json::from_str(r#"{"style": "conventional"}"#).unwrap();
/// assert_eq!(c, CommitGrammar::Conventional);
/// assert!(serde_json::from_str::<CommitGrammar>(r#"{"style": "regex"}"#).is_err());
/// ```
pub enum CommitGrammar {
    /// `type(scope)!: subject`, the type words being [`ChangeKind`]'s. The default.
    #[default]
    Conventional,
    /// `Keyword #N, <hash>: [Component] Details`, the words declared by the repository.
    Keyword(KeywordGrammar),
}

/// The keyword style's declaration: the closed list of words, each with the kind of change it
/// is, and whether the details open with a bracketed component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// ```
/// use majordomus_cli::commit::grammar::{ComponentStyle, KeywordGrammar};
/// let k: KeywordGrammar = serde_json::from_str(r#"{"words": [{"word": "Fix", "kind": "fix"}]}"#).unwrap();
/// assert_eq!(k.component, ComponentStyle::Brackets, "a component is read by default");
/// ```
pub struct KeywordGrammar {
    /// Every word a header may open with, in the order a finding lists them, each with the
    /// kind of change it records. One table is both the closed list the judge holds a header
    /// to and the classification `fix_requires_test` (and any reader of kinds) uses, so there
    /// is no second copy of either.
    pub words: Vec<KeywordWord>,
    /// Whether `[X] ` at the start of the details is a component. Read, never judged: the
    /// upstream grammar does not constrain it, and a judged component would refuse what the
    /// forge accepts.
    #[serde(default)]
    pub component: ComponentStyle,
}

/// One word of a keyword grammar and the kind of change it records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeywordWord {
    /// The word as a header writes it: `Fix`, `Codechange`.
    pub word: String,
    /// A [`ChangeKind`] word (`feat`, `fix`, `refactor`, `docs`, ...) or `other`.
    pub kind: String,
}

/// How a keyword header marks the part of the project it is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum ComponentStyle {
    /// `[X] ` opens the details, and `X` is the scope.
    #[default]
    Brackets,
    /// The details are the subject whole.
    None,
}

/// What is wrong with a header under the declared grammar: the finding's code and its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderDefect {
    /// The header is not `type(scope): subject` (the conventional grammar).
    NotConventional(String),
    /// A prefix names a word the grammar does not declare.
    UnknownWord(String),
    /// The header is not `<prefixes>: <details>`, or a prefix is malformed.
    Malformed(String),
}

/// Whether `s` is a commit hash as the keyword style writes one: four or more lowercase
/// hexadecimal digits.
fn is_hash(s: &str) -> bool {
    s.len() >= 4
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether `s` is `#` followed by one or more digits.
fn is_issue(s: &str) -> bool {
    s.strip_prefix('#')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The kind a declared word names; `other`, and anything [`ChangeKind`] does not know, is
/// [`ChangeKind::Other`].
fn kind_of(word: &str) -> ChangeKind {
    ChangeKind::parse(word)
}

impl KeywordGrammar {
    /// The declared words, in their declared order, as one list for a finding to name.
    pub fn word_list(&self) -> String {
        self.words
            .iter()
            .map(|w| w.word.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The kind a declared word names, or `None` for a word the grammar does not hold.
    fn kind(&self, word: &str) -> Option<&str> {
        self.words
            .iter()
            .find(|w| w.word == word)
            .map(|w| w.kind.as_str())
    }

    /// Split `Word`, `Word #N` or `Word <hash>` into its word and reference.
    fn split_prefix<'a>(&self, prefix: &'a str) -> (&'a str, Option<&'a str>) {
        match prefix.split_once(' ') {
            Some((w, r)) => (w, Some(r)),
            None => (prefix, None),
        }
    }

    /// What is wrong with one header line, or nothing.
    fn defect(&self, line: &str) -> Option<HeaderDefect> {
        let shape = "`<Keyword>( #<issue>| <commit>(, (<Keyword> #<issue>|<commit>))*)?: [<Component>] <Details>`";
        let Some((head, _)) = line.split_once(": ") else {
            return Some(HeaderDefect::Malformed(format!(
                "the header has no `: `; it must be {shape}"
            )));
        };
        let prefixes: Vec<&str> = head.split(", ").collect();
        let sole = prefixes.len() == 1;
        for (i, prefix) in prefixes.iter().enumerate() {
            if i > 0 && is_hash(prefix) {
                continue;
            }
            let (word, reference) = self.split_prefix(prefix);
            if self.kind(word).is_none() {
                if word.chars().all(|c| c.is_ascii_alphabetic()) && !word.is_empty() {
                    return Some(HeaderDefect::UnknownWord(format!(
                        "'{word}' is not a word of this repository's commit grammar; the words are {}",
                        self.word_list()
                    )));
                }
                return Some(HeaderDefect::Malformed(format!(
                    "'{prefix}' is not a prefix; it must be {shape}"
                )));
            }
            match reference {
                None if sole => {}
                None => {
                    return Some(HeaderDefect::Malformed(format!(
                        "'{word}' stands alone only when it is the only prefix; give it an issue (#N) or a commit hash"
                    )))
                }
                Some(r) if is_issue(r) || is_hash(r) => {}
                Some(r) => {
                    return Some(HeaderDefect::Malformed(format!(
                        "'{r}' after '{word}' is neither an issue (#N) nor a commit hash of four or more lowercase hex digits"
                    )))
                }
            }
        }
        None
    }

    /// The header taken apart: the first prefix's word and kind, the component as the scope,
    /// the details as the subject. Total, like [`CommitHeader::parse`].
    fn parse(&self, line: &str) -> CommitHeader {
        let line = line.trim_end_matches(['\r', '\n']);
        let whole = || CommitHeader {
            word: String::new(),
            kind: ChangeKind::Other,
            scope: None,
            breaking: false,
            subject: line.trim().to_string(),
        };
        let Some((head, details)) = line.split_once(": ") else {
            return whole();
        };
        let first = head.split(", ").next().unwrap_or_default();
        let (word, _) = self.split_prefix(first);
        let Some(kind) = self.kind(word) else {
            return whole();
        };
        let (scope, subject) = match self.component {
            ComponentStyle::Brackets => match details
                .strip_prefix('[')
                .and_then(|rest| rest.split_once("] "))
            {
                Some((c, rest)) if !c.is_empty() => (Some(c.to_string()), rest.trim()),
                _ => (None, details.trim()),
            },
            ComponentStyle::None => (None, details.trim()),
        };
        CommitHeader {
            word: word.to_string(),
            kind: kind_of(kind),
            scope,
            breaking: false,
            subject: subject.to_string(),
        }
    }
}

impl CommitGrammar {
    /// The header of one line, read under this grammar. Never fails.
    /// ```
    /// use majordomus_cli::commit::grammar::CommitGrammar;
    /// let h = CommitGrammar::Conventional.parse("fix(plan): a thing");
    /// assert_eq!(h.scope.as_deref(), Some("plan"));
    /// ```
    pub fn parse(&self, line: &str) -> CommitHeader {
        match self {
            CommitGrammar::Conventional => CommitHeader::parse(line),
            CommitGrammar::Keyword(k) => k.parse(line),
        }
    }

    /// What is wrong with a header line under this grammar, or `None` when it is well formed.
    /// ```
    /// use majordomus_cli::commit::grammar::{CommitGrammar, HeaderDefect};
    /// assert!(CommitGrammar::Conventional.defect("feat(a): b").is_none());
    /// assert!(matches!(CommitGrammar::Conventional.defect("update stuff"), Some(HeaderDefect::NotConventional(_))));
    /// ```
    pub fn defect(&self, line: &str) -> Option<HeaderDefect> {
        match self {
            CommitGrammar::Conventional => {
                let head = CommitHeader::parse(line);
                if head.is_conventional() {
                    return None;
                }
                let words = ChangeKind::WORDS.join(", ");
                Some(HeaderDefect::NotConventional(if head.word.is_empty() {
                    format!(
                        "'{}' is not `type(scope): subject`; the types are {words}",
                        head.subject.chars().take(60).collect::<String>()
                    )
                } else {
                    format!(
                        "'{}' is not a commit type; the types are {words}",
                        head.word
                    )
                }))
            }
            CommitGrammar::Keyword(k) => k.defect(line),
        }
    }

    /// Whether git composed this subject, so that no grammar applies to it. The conventional
    /// grammar exempts every subject git writes; the keyword style exempts only a merge,
    /// because the forge check it mirrors judges every other commit, reverts and fixups
    /// included.
    /// ```
    /// use majordomus_cli::commit::grammar::CommitGrammar;
    /// let k: CommitGrammar = serde_json::from_str(r#"{"style":"keyword","words": [{"word": "Fix", "kind": "fix"}]}"#).unwrap();
    /// assert!(CommitGrammar::Conventional.exempt("Revert \"feat: x\""));
    /// assert!(!k.exempt("Revert \"Fix: x\""));
    /// assert!(k.exempt("Merge branch 'master' into feature/x"));
    /// ```
    pub fn exempt(&self, subject: &str) -> bool {
        match self {
            CommitGrammar::Conventional => git_authored(subject),
            CommitGrammar::Keyword(_) => subject.starts_with("Merge "),
        }
    }

    /// Whether this is the keyword style.
    pub fn is_keyword(&self) -> bool {
        matches!(self, CommitGrammar::Keyword(_))
    }

    /// Everything wrong with the declaration itself, one sentence each. A grammar with a
    /// defect refuses every message rather than judging some of them by accident.
    /// ```
    /// use majordomus_cli::commit::grammar::CommitGrammar;
    /// let bad: CommitGrammar = serde_json::from_str(
    ///     r#"{"style":"keyword","words": [{"word": "beef", "kind": "fix"}, {"word": "Fix", "kind": "merge"}, {"word": "Odd word", "kind": "fix"}, {"word": "Doc", "kind": "nonsense"}]}"#,
    /// ).unwrap();
    /// assert_eq!(bad.problems().len(), 4);
    /// assert!(CommitGrammar::Conventional.problems().is_empty());
    /// ```
    pub fn problems(&self) -> Vec<String> {
        let CommitGrammar::Keyword(k) = self else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if k.words.is_empty() {
            out.push("commit.grammar declares the keyword style with no words".into());
        }
        let mut seen: Vec<&str> = Vec::new();
        for KeywordWord { word, kind } in &k.words {
            if seen.contains(&word.as_str()) {
                out.push(format!("commit.grammar declares the word '{word}' twice"));
            }
            seen.push(word);
            if word.is_empty() || !word.chars().all(|c| c.is_ascii_alphanumeric()) {
                out.push(format!(
                    "commit.grammar word '{word}' must be letters and digits only (no space, comma or colon)"
                ));
            } else if is_hash(word) {
                out.push(format!(
                    "commit.grammar word '{word}' reads as a commit hash; a word needs a letter outside a-f or an upper-case letter"
                ));
            }
            if kind == "merge" {
                out.push(format!(
                    "commit.grammar word '{word}' maps to merge, which only git's own merge subjects are"
                ));
            } else if kind != "other" && kind_of(kind) == ChangeKind::Other {
                out.push(format!(
                    "commit.grammar word '{word}' maps to '{kind}', which is not a change kind; the kinds are {}, other",
                    ChangeKind::WORDS.join(", ")
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The upstream checker's keyword list, with the classification a repository would give it.
    fn upstream() -> CommitGrammar {
        serde_json::from_value(serde_json::json!({
            "style": "keyword",
            "words": [
                {"word": "Add", "kind": "feat"},
                {"word": "Feature", "kind": "feat"},
                {"word": "Change", "kind": "refactor"},
                {"word": "Remove", "kind": "refactor"},
                {"word": "Codechange", "kind": "refactor"},
                {"word": "Codefix", "kind": "chore"},
                {"word": "Cleanup", "kind": "refactor"},
                {"word": "Fix", "kind": "fix"},
                {"word": "Revert", "kind": "revert"},
                {"word": "Doc", "kind": "docs"},
                {"word": "Update", "kind": "build"},
                {"word": "Upgrade", "kind": "build"},
                {"word": "Prepare", "kind": "other"}
            ]
        }))
        .expect("the upstream grammar")
    }

    #[test]
    fn every_example_the_upstream_checker_prints_is_well_formed() {
        let g = upstream();
        for line in [
            "Fix: [YAPF] Infinite loop in pathfinder.",
            "Fix #5926: [YAPF] Infinite loop in pathfinder.",
            "Fix 80dffae130: Warning about unsigned unary minus.",
            "Fix #6673, 99bb3a95b4: Store the map variety setting in the samegame.",
            "Revert d9065fbfbe, Fix #5922: ClientSizeChanged is only called via WndProcGdi which already has the mutex.",
            "Fix #1264, Fix #2037, Fix #2038, Fix #2110: Rewrite the autoreplace kernel.",
        ] {
            assert_eq!(g.defect(line), None, "{line}");
        }
    }

    #[test]
    fn a_word_the_grammar_does_not_declare_is_named_with_the_words_it_does() {
        let Some(HeaderDefect::UnknownWord(m)) = upstream().defect("Test: [Script] A test") else {
            panic!("Test is not an upstream word");
        };
        assert!(m.contains("'Test'") && m.contains("Codechange"), "{m}");
    }

    #[test]
    fn the_prefixes_follow_the_upstream_rules_exactly() {
        let g = upstream();
        for refused in [
            "Fix #12, #13: x",
            "Fix, Add: x",
            "Fix CAFE: x",
            "Fix abc: x",
            "cafe: x",
            "Fix #: x",
            "Fix: ",
            "no colon at all",
            "Fix:x",
            "Fix,Add: x",
            "Fix #12,cafe1: x",
        ] {
            let d = g.defect(refused);
            if refused == "Fix: " {
                // the details are empty, which the judge reports as an empty subject: the
                // header itself is well formed, as upstream says
                assert_eq!(d, None, "{refused}");
                continue;
            }
            assert!(d.is_some(), "{refused} should be refused");
        }
        // the header splits at the first `: `, so a colon in the details is the details'
        for accepted in [
            "Fix cafe: x",
            "Fix: x",
            "Fix #12, cafe1234: x",
            "Doc #3: x",
            "Fix: #12: x",
        ] {
            assert_eq!(g.defect(accepted), None, "{accepted}");
        }
    }

    #[test]
    fn the_first_prefix_decides_the_kind_and_the_component_is_the_scope() {
        let g = upstream();
        let h = g.parse("Revert d9065fbfbe, Fix #5922: [Win32] The mutex is held");
        assert_eq!(h.kind, ChangeKind::Revert);
        assert_eq!(h.word, "Revert");
        assert_eq!(h.scope.as_deref(), Some("Win32"));
        assert_eq!(h.subject, "The mutex is held");
        let plain = g.parse("Codefix: A comment said the wrong thing");
        assert_eq!(plain.kind, ChangeKind::Chore);
        assert_eq!(plain.scope, None);
        assert_eq!(g.parse("Prepare: for the release").kind, ChangeKind::Other);
        // a header the grammar cannot read is carried whole
        assert_eq!(g.parse("whatever this is").subject, "whatever this is");
        assert_eq!(g.parse("Test: [X] y").word, "");
    }

    #[test]
    fn a_component_is_read_only_when_the_grammar_declares_one() {
        let g: CommitGrammar = serde_json::from_value(serde_json::json!({
            "style": "keyword", "words": [{"word": "Fix", "kind": "fix"}], "component": "none"
        }))
        .unwrap();
        let h = g.parse("Fix: [X] y");
        assert_eq!(h.scope, None);
        assert_eq!(h.subject, "[X] y");
        // and `[]` or a bracket with no space after it is not a component
        let u = upstream();
        assert_eq!(u.parse("Fix: [] y").scope, None);
        assert_eq!(u.parse("Fix: [X]y").scope, None);
    }

    #[test]
    fn exemptions_belong_to_the_grammar() {
        let g = upstream();
        assert!(g.exempt("Merge pull request #1 from a/b"));
        for judged in ["Revert \"Fix: x\"", "fixup! Fix: x", "squash! Fix: x"] {
            assert!(!g.exempt(judged), "{judged}");
            assert!(CommitGrammar::Conventional.exempt(judged), "{judged}");
        }
        assert!(g.is_keyword());
        assert!(!CommitGrammar::Conventional.is_keyword());
    }

    #[test]
    fn a_declaration_with_a_defect_names_each_one() {
        let empty: CommitGrammar =
            serde_json::from_value(serde_json::json!({"style": "keyword", "words": []})).unwrap();
        assert_eq!(empty.problems().len(), 1);
        assert!(upstream().problems().is_empty());
        let bad: CommitGrammar = serde_json::from_value(serde_json::json!({
            "style": "keyword",
            "words": [{"word": "beef", "kind": "fix"}, {"word": "Fix", "kind": "merge"}, {"word": "Odd word", "kind": "fix"}, {"word": "Doc", "kind": "nonsense"}]
        }))
        .unwrap();
        let p = bad.problems();
        assert!(
            p.iter()
                .any(|m| m.contains("'beef' reads as a commit hash")),
            "{p:?}"
        );
        assert!(p.iter().any(|m| m.contains("maps to merge")), "{p:?}");
        assert!(
            p.iter().any(|m| m.contains("letters and digits only")),
            "{p:?}"
        );
        assert!(
            p.iter()
                .any(|m| m.contains("'nonsense', which is not a change kind")),
            "{p:?}"
        );
        // an unknown key is refused when the policy is read, not silently ignored
        assert!(serde_json::from_value::<CommitGrammar>(serde_json::json!({
            "style": "keyword", "words": [{"word": "Fix", "kind": "fix"}], "subject_case": "capital"
        }))
        .is_err());
    }

    #[test]
    fn a_word_with_odd_characters_is_malformed_rather_than_unknown() {
        assert!(matches!(
            upstream().defect("fix(x): y"),
            Some(HeaderDefect::Malformed(_))
        ));
        assert!(matches!(
            upstream().defect("Fix 12: y"),
            Some(HeaderDefect::Malformed(_))
        ));
    }
}
