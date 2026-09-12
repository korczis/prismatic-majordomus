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

use super::model::{Change, Reference};
use crate::model::Object;

/// The separator between a commit's fields, and between commits. Chosen because git will
/// not produce it and a commit body may contain any newline arrangement it likes.
const FIELD: &str = "\u{1}";
const RECORD: &str = "\u{2}";

/// How many hex digits of a commit's name the changelog carries.
///
/// A constant this repository chooses, rather than git's `%h`. Git abbreviates to whatever
/// is unambiguous in the object database in front of it, so `%h` is a function of how many
/// objects the clone happens to hold: the same commit reads `933dba91d` in a working
/// checkout that has fetched every branch and `933dba91` in the fresh one CI makes. The
/// changelog is a committed artifact, so that turned the width of somebody's object store
/// into part of a generated file — `generate --check` then reported the artifact stale in
/// every clone but the one the generator last ran in, with no canonical source changed
/// anywhere (`project.derived-files-regenerated`). Nine digits is what this history already
/// carries; the abbreviation is now the repository's decision and reads the same everywhere.
const NAME_DIGITS: usize = 9;

/// A commit's name, as the changelog carries it: the full name git gave, cut to
/// [`NAME_DIGITS`]. The cut is here and not in git's format string, because git's own
/// abbreviation depends on the clone rather than on the commit.
fn short(name: &str) -> &str {
    &name[..NAME_DIGITS.min(name.len())]
}

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
            &format!("--format=%H{FIELD}%s{FIELD}%b{RECORD}"),
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
            let mut change = parse(short(commit), subject, body);
            change.references = references(&format!("{subject} {body}"), objects);
            Some(change)
        })
        .collect()
}

/// Every word in `text` shaped like a record id of this repository: `I` and four digits,
/// `M` and three.
///
/// The shape alone, with nothing resolved. Both halves of the question read it: the
/// changelog asks which candidates the layer *holds*, and the commit validator asks which
/// it does *not* — and a repository that scanned for one direction and not the other is how
/// a commit came to name a record nobody had, with nothing to say so. One scan, two callers,
/// no way for the two to disagree about what counts as an id.
///
/// ```
/// use majordomus_cli::release::commits::record_candidates;
/// assert_eq!(record_candidates("done under I1305 for M000"), vec!["I1305", "M000"]);
/// // a word that merely starts with the letter is never a candidate
/// assert!(record_candidates("Interesting M1 and I12345 are not ids").is_empty());
/// // and each is offered once, in the order it was written
/// assert_eq!(record_candidates("I1305 again I1305"), vec!["I1305"]);
/// ```
pub fn record_candidates(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            word.push(ch);
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    let mut out: Vec<String> = Vec::new();
    for c in words {
        let looks_like = (c.starts_with('I')
            && c.len() == 5
            && c[1..].chars().all(|d| d.is_ascii_digit()))
            || (c.starts_with('M') && c.len() == 4 && c[1..].chars().all(|d| d.is_ascii_digit()));
        if looks_like && !out.contains(&c) {
            out.push(c);
        }
    }
    out
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
    for c in record_candidates(text) {
        if seen.contains(&c) {
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
///
/// The grammar is [`crate::commit::Header`]'s, not a second one: the changelog and the
/// commit validator read the same parse and disagree only about what to *do* with a header
/// that is not conventional. Here that is not a failure — a commit nobody spelled
/// conventionally still happened, and a changelog that dropped it would lie about what the
/// release contains.
pub fn parse(commit: &str, subject: &str, body: &str) -> Change {
    let breaking_trailer = body
        .lines()
        .any(|l| l.starts_with("BREAKING CHANGE:") || l.starts_with("BREAKING-CHANGE:"));
    let header = crate::commit::CommitHeader::parse(subject);
    Change {
        kind: header.kind,
        scope: header.scope,
        subject: header.subject,
        breaking: header.breaking || breaking_trailer,
        commit: commit.to_string(),
        url: None,
        references: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::ChangeKind;

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
    fn a_commit_name_is_cut_to_a_width_this_repository_chooses() {
        // The changelog is committed, so the width cannot come from git: `%h` is short in a
        // fresh clone and longer in one that has fetched everything, and the artifact would
        // then be stale in every checkout but the last one the generator ran in.
        let full = "933dba91d1f7e0a54a1a2b3c4d5e6f7a8b9c0d1e";
        assert_eq!(short(full), "933dba91d");
        assert_eq!(short(full).len(), NAME_DIGITS);
        // a name already shorter than the width is carried whole rather than panicking
        assert_eq!(short("abc"), "abc");
        assert_eq!(short(""), "");
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
