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

use super::model::{Change, ChangeKind, Reference};
use crate::model::Object;

/// The separator between a commit's fields, and between commits. Chosen because git will
/// not produce it and a commit body may contain any newline arrangement it likes.
const FIELD: &str = "\u{1}";
const RECORD: &str = "\u{2}";

/// Every commit in `range`, newest first, parsed.
///
/// `range` is anything `git log` accepts — `v0.3.1..HEAD`, a bare `HEAD`, two shas. An
/// unknown ref is not an error here: it yields no commits and the caller reports it, which
/// is what lets a changelog render in a shallow clone that has no tags.
pub fn in_range(root: &Path, range: &str, objects: &[Object]) -> Vec<Change> {
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
            let mut change = parse(commit, subject, body);
            change.references = references(&format!("{subject} {body}"), objects);
            Some(change)
        })
        .collect()
}

/// The records a commit's own text names, resolved against what the layer holds.
///
/// The reference is inferred from the text — `I1305`, `M000` — and then *looked up*. An id
/// that matches the shape but names nothing is dropped, so this can never produce a link to
/// a record that does not exist. The scan is over the subject and the body together, because
/// a commit that explains itself in its body is the one most worth linking.
///
/// ```
/// use majordomus_cli::release::commits::references;
/// // the shape alone is not a reference: against a layer that holds nothing, nothing resolves
/// assert!(references("fix(link): resolves I4242 and mentions M000", &[]).is_empty());
/// // and a word that merely starts with the letter is never a candidate
/// assert!(references("Interesting: M1 and I12345 are not ids", &[]).is_empty());
/// ```
pub fn references(text: &str, objects: &[Object]) -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut candidates: Vec<String> = Vec::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            word.push(ch);
        } else if !word.is_empty() {
            candidates.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        candidates.push(word);
    }
    for c in candidates {
        let looks_like = (c.starts_with('I')
            && c.len() == 5
            && c[1..].chars().all(|d| d.is_ascii_digit()))
            || (c.starts_with('M') && c.len() == 4 && c[1..].chars().all(|d| d.is_ascii_digit()));
        if !looks_like || seen.contains(&c) {
            continue;
        }
        let kind = if c.starts_with('I') {
            "issue"
        } else {
            "milestone"
        };
        if let Some(o) = objects.iter().find(|o| o.kind == kind && o.identity == c) {
            seen.push(c.clone());
            out.push(Reference {
                kind: kind.to_string(),
                title: o
                    .title
                    .clone()
                    .or_else(|| {
                        o.metadata
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    })
                    .unwrap_or_else(|| c.clone()),
                route: Some(format!("/plan/{}/", c.to_lowercase())),
                id: c,
            });
        }
    }
    out
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
            url: None,
            references: Vec::new(),
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
    // A type word this does not know is not a reason to hide it. `wip(site): halfway
    // through` rendered as `halfway through` under "Other" tells a reader less than the
    // author wrote; the word stays, and so does a head that was never a type at all
    // (`Revert: the thing that broke` is carried whole, as its author typed it) — the same
    // refusal to lie by omission the module makes for a subject with no colon in it.
    if kind == ChangeKind::Other {
        return Change {
            kind,
            scope,
            subject: format!("{word}: {}", rest.trim()),
            breaking: bang || breaking_trailer,
            commit: commit.to_string(),
            url: None,
            references: Vec::new(),
        };
    }
    Change {
        kind,
        scope,
        subject: rest.trim().to_string(),
        breaking: bang || breaking_trailer,
        commit: commit.to_string(),
        url: None,
        references: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(kind: &str, id: &str, title: &str) -> Object {
        // Only the three fields the resolution reads carry meaning; the rest is what an
        // object must have to exist, so the fixture states the question and nothing else.
        Object {
            kind: kind.into(),
            identity: id.into(),
            uri: format!("majordomus://{kind}/{id}"),
            title: Some(title.into()),
            description: None,
            metadata: serde_json::Value::Null,
            body: String::new(),
            content: String::new(),
            media_type: "text/yaml",
            provenance: crate::model::Provenance {
                path: format!(".ai/repo/project/{kind}s/{id}.yaml"),
                directory: format!(".ai/repo/project/{kind}s"),
                source_class: kind.into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    #[test]
    fn a_reference_is_carried_only_when_the_layer_holds_what_it_names() {
        let layer = vec![
            object("issue", "I1305", "Traceability"),
            object("milestone", "M000", "Foundations"),
        ];
        // both named and both held
        let refs = references("feat(x): done under I1305 for M000", &layer);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].id, "I1305");
        assert_eq!(refs[0].kind, "issue");
        assert_eq!(refs[1].kind, "milestone");

        // the shape matches and the layer holds nothing: no link, because a link to a record
        // that does not exist cannot be told from a good one until it is followed
        assert!(references("feat(x): done under I9999", &layer).is_empty());

        // named twice, carried once
        assert_eq!(references("I1305 and again I1305", &layer).len(), 1);

        // a word that merely starts with the letter is not an id
        assert!(references("Interesting M1 Improvements", &layer).is_empty());
    }

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
    fn an_unknown_lowercase_type_is_other_and_keeps_its_word() {
        let c = parse("abc1234", "wip(site): halfway through", "");
        assert_eq!(c.kind, ChangeKind::Other);
        assert_eq!(c.scope.as_deref(), Some("site"));
        // the word is what the author said about the change; hiding it says less
        assert_eq!(c.subject, "wip: halfway through");
    }
}
