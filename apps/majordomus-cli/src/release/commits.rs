//! Conventional commits, read from git and parsed.
//!
//! The parse is deliberately small and total: a subject that does not match the convention
//! is [`ChangeKind::Other`] rather than an error, because the changelog's job is to say what
//! happened and a commit nobody spelled conventionally still happened.
//!
//! ```text
//!   feat(commands): one canonical command graph
//!   ^^^^ ^^^^^^^^   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
//!   kind scope      subject
//!
//!   feat(api)!: the route moved      `!` marks it breaking
//!   BREAKING CHANGE: <why>           so does this trailer, in the body
//! ```

use std::path::Path;
use std::process::Command;

use super::model::{Change, ChangeKind};

/// The separator between a commit's fields, and between commits. Chosen because git will
/// not produce it and a commit body may contain any newline arrangement it likes.
const FIELD: &str = "\u{1}";
const RECORD: &str = "\u{2}";

/// Every commit in `range`, newest first, parsed.
///
/// `range` is anything `git log` accepts — `v0.3.1..HEAD`, a bare `HEAD`, two shas. An
/// unknown ref is not an error here: it yields no commits and the caller reports it, which
/// is what lets a changelog render in a shallow clone that has no tags.
pub fn in_range(root: &Path, range: &str) -> Vec<Change> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "log",
            "--no-merges",
            &format!("--format=%h{FIELD}%s{FIELD}%b{RECORD}"),
            range,
        ])
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .split(RECORD)
        .filter_map(|record| {
            let mut parts = record.trim_start_matches('\n').split(FIELD);
            let commit = parts.next()?.trim();
            let subject = parts.next()?;
            let body = parts.next().unwrap_or_default();
            if commit.is_empty() {
                return None;
            }
            Some(parse(commit, subject, body))
        })
        .collect()
}

/// One commit, as a change.
pub fn parse(commit: &str, subject: &str, body: &str) -> Change {
    let breaking_trailer = body
        .lines()
        .any(|l| l.starts_with("BREAKING CHANGE:") || l.starts_with("BREAKING-CHANGE:"));

    // `type(scope)!: subject`, with the scope and the `!` both optional.
    let Some((head, rest)) = subject.split_once(": ") else {
        return Change {
            kind: ChangeKind::Other,
            scope: None,
            subject: subject.trim().to_string(),
            breaking: breaking_trailer,
            commit: commit.to_string(),
        };
    };
    let head = head.trim();
    let bang = head.ends_with('!');
    let head = head.trim_end_matches('!');
    let (word, scope) = match head.split_once('(') {
        Some((w, s)) => (w, s.strip_suffix(')').map(|s| s.to_string())),
        None => (head, None),
    };
    let kind = ChangeKind::parse(word);
    // A head that parsed to `Other` was not a conventional commit at all, so its "subject"
    // is the whole line rather than the half after a colon that happened to be there.
    if kind == ChangeKind::Other && scope.is_none() && !word.chars().all(|c| c.is_ascii_lowercase())
    {
        return Change {
            kind: ChangeKind::Other,
            scope: None,
            subject: subject.trim().to_string(),
            breaking: bang || breaking_trailer,
            commit: commit.to_string(),
        };
    }
    Change {
        kind,
        scope,
        subject: rest.trim().to_string(),
        breaking: bang || breaking_trailer,
        commit: commit.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conventional_subject_splits_into_its_parts() {
        let c = parse("abc1234", "feat(commands): one canonical command graph", "");
        assert_eq!(c.kind, ChangeKind::Feat);
        assert_eq!(c.scope.as_deref(), Some("commands"));
        assert_eq!(c.subject, "one canonical command graph");
        assert!(!c.breaking);
    }

    #[test]
    fn a_scope_is_optional() {
        let c = parse("abc1234", "docs: the document the env recipes cite", "");
        assert_eq!(c.kind, ChangeKind::Docs);
        assert_eq!(c.scope, None);
        assert_eq!(c.subject, "the document the env recipes cite");
    }

    #[test]
    fn both_spellings_of_breaking_are_read() {
        assert!(parse("a", "feat(api)!: the route moved", "").breaking);
        assert!(
            parse(
                "a",
                "feat(api): the route moved",
                "BREAKING CHANGE: it moved"
            )
            .breaking
        );
        assert!(
            parse(
                "a",
                "feat(api): the route moved",
                "BREAKING-CHANGE: it moved"
            )
            .breaking
        );
    }

    #[test]
    fn a_subject_that_is_not_conventional_is_kept_whole() {
        // A changelog that drops what it cannot classify lies by omission, so the entry
        // survives as `Other` with its subject intact rather than being split at a colon
        // that was part of the sentence.
        let c = parse("abc1234", "Merge pull request #117 from korczis/int", "");
        assert_eq!(c.kind, ChangeKind::Other);
        assert_eq!(c.subject, "Merge pull request #117 from korczis/int");
        let c = parse("abc1234", "Revert: the thing that broke", "");
        assert_eq!(c.kind, ChangeKind::Other);
        assert_eq!(c.subject, "Revert: the thing that broke");
    }

    #[test]
    fn an_unknown_lowercase_type_is_other_but_keeps_its_subject() {
        let c = parse("abc1234", "wip(site): halfway through", "");
        assert_eq!(c.kind, ChangeKind::Other);
        assert_eq!(c.scope.as_deref(), Some("site"));
        assert_eq!(c.subject, "halfway through");
    }
}
