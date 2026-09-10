//! Composing the changelog out of what the repository already states.
//!
//! Nothing here is authored. A section per release record, newest first, with an unreleased
//! section leading when there is work after the last one:
//!
//! ```text
//!   release record  ──►  version, tag, date, commit, artifacts
//!   adr objects     ──►  the decisions dated inside this release's window
//!   git log A..B    ──►  the changes, as conventional commits
//! ```
//!
//! The window of a release is `(previous release's date, this release's date]` for the
//! decisions and `previous..this` for the commits. Both are the same interval said in the
//! two vocabularies the two sources have — an ADR carries a date and no commit, and a commit
//! carries no date the layer indexes.

use std::path::Path;

use serde_json::Value;

use crate::model::Object;

use super::commits;
use super::model::{
    Artifact, Change, ChangeGroup, Changelog, Decision, ReleaseSection, CHANGELOG_SCHEMA,
};
use super::version;

/// The repository the links point into, as the crate manifest declares it.
///
/// Never a literal: `about::REPOSITORY` is `CARGO_PKG_REPOSITORY`, so a fork or a move
/// carries every link with it and nothing here has to be told. A URL that is not a forge
/// this understands yields no links at all rather than a guess — a wrong link is worse than
/// no link, because a reader cannot tell it is wrong until they follow it.
fn forge() -> Option<&'static str> {
    let url = crate::about::REPOSITORY.trim_end_matches('/');
    if url.starts_with("https://github.com/") && url.split('/').count() == 5 {
        Some(url)
    } else {
        None
    }
}

/// The kind the layer gives a published release.
pub const RELEASE_KIND: &str = "release-record";
/// The kind the layer gives a decision.
pub const ADR_KIND: &str = "adr";

/// One release, as read out of its record, before the changes are joined to it.
struct Record {
    version: String,
    tag: String,
    date: String,
    commit: String,
    notes: Option<String>,
    artifacts: Vec<Artifact>,
}

/// Compose the changelog, unreleased section included.
///
/// `objects` is the index's own view of the layer; `root` is the repository git reads. A
/// repository with no records yields one unreleased section, which is the right answer for a
/// project that has never published rather than an error.
///
/// This is what a live reader wants — the API, MCP and the Cockpit answer at request time
/// and HEAD is whatever it is then. It is not what a committed artifact may hold: see
/// [`compose_published`].
pub fn compose(root: &Path, objects: &[Object]) -> Changelog {
    compose_with(root, objects, true)
}

/// Compose the changelog of what has been published, and nothing after it.
///
/// The one form a committed artifact may take. The unreleased section is
/// `<last release>..HEAD`, and a file inside a commit cannot describe the commit it is in:
/// every commit made the committed changelog stale by construction, the pre-commit gate
/// refused until a full derive had run, and every session paid several minutes per commit
/// or worked around the gate. A document that depends only on the release records and the
/// history up to the newest of them is the same bytes at every commit that changes neither,
/// which is what "current" has to mean for a file under `docs/generated/`.
///
/// A repository with no records yields no section at all here — there is nothing published
/// to describe — where [`compose`] yields the unreleased one.
pub fn compose_published(root: &Path, objects: &[Object]) -> Changelog {
    compose_with(root, objects, false)
}

fn compose_with(root: &Path, objects: &[Object], unreleased: bool) -> Changelog {
    let mut diagnostics = Vec::new();

    let mut records: Vec<Record> = objects
        .iter()
        .filter(|o| o.kind == RELEASE_KIND)
        .filter_map(|o| record_of(&o.metadata, &mut diagnostics))
        .collect();
    // Newest first, by the date the record carries: the order a changelog is read in.
    records.sort_by(|a, b| b.date.cmp(&a.date));

    let decisions = decisions_of(root, objects);

    let mut sections = Vec::new();

    // The unreleased section: everything after the newest release, when there is any, and
    // only for a reader that asked for it — a committed document never does (see
    // `compose_published`).
    let newest = records.first();
    let unreleased_range = match newest {
        Some(r) => format!("{}..HEAD", r.commit),
        None => "HEAD".to_string(),
    };
    let unreleased_changes = if unreleased {
        commits::in_range(root, &unreleased_range, objects)
    } else {
        Vec::new()
    };
    if !unreleased || (newest.is_some() && unreleased_changes.is_empty()) {
        // nothing since the last release, or nobody asked; no section rather than an empty one
    } else {
        sections.push(ReleaseSection {
            version: "unreleased".into(),
            tag: None,
            date: None,
            commit: None,
            unreleased: true,
            // Nothing is published yet, so there are no notes; the range is what has landed
            // since the last release, which is exactly what this section lists.
            notes_url: None,
            compare_url: forge()
                .zip(newest)
                .map(|(base, r)| format!("{base}/compare/{}...master", r.tag)),
            tree_url: forge().map(|base| format!("{base}/tree/master")),
            decisions: decisions_after(root, &decisions, newest.map(|r| r.commit.as_str())),
            groups: grouped(unreleased_changes),
            artifacts: Vec::new(),
        });
    }

    for (i, r) in records.iter().enumerate() {
        let previous = records.get(i + 1);
        let range = match previous {
            Some(p) => format!("{}..{}", p.commit, r.commit),
            // The first release: everything up to it. A clone without that history answers
            // nothing, which the diagnostics below make visible rather than silent.
            None => r.commit.clone(),
        };
        let changes = commits::in_range(root, &range, objects);
        if changes.is_empty() {
            diagnostics.push(format!(
                "no commit was readable for {} ({}); the clone may not carry that history",
                r.version, range
            ));
        }
        sections.push(ReleaseSection {
            version: r.version.clone(),
            tag: Some(r.tag.clone()),
            date: Some(r.date.clone()),
            commit: Some(r.commit.clone()),
            unreleased: false,
            // The record names its own notes; the range and the tree are the forge's own
            // addresses for facts the record already carries, so neither is authored.
            notes_url: r
                .notes
                .clone()
                .or_else(|| forge().map(|base| format!("{base}/releases/tag/{}", r.tag))),
            compare_url: forge().map(|base| match previous {
                Some(p) => format!("{base}/compare/{}...{}", p.tag, r.tag),
                None => format!("{base}/commits/{}", r.tag),
            }),
            tree_url: forge().map(|base| format!("{base}/tree/{}", r.tag)),
            decisions: decisions_between(
                root,
                &decisions,
                previous.map(|p| p.commit.as_str()),
                &r.commit,
            ),
            groups: grouped(changes),
            artifacts: r.artifacts.clone(),
        });
    }

    Changelog {
        schema: CHANGELOG_SCHEMA.into(),
        current: version::declared(root).unwrap_or_else(|| "unknown".into()),
        sections,
        diagnostics,
        // Filled by whoever answers with it: the composer does not know which surface asked.
        produced_by: None,
    }
}

/// The changes of a section, grouped by kind and ordered by the rank each kind carries.
///
/// Done here rather than in each renderer: the Markdown, the site and any other reader get
/// the same order because they are given it, not because they each reimplemented it.
fn grouped(mut changes: Vec<Change>) -> Vec<ChangeGroup> {
    if let Some(base) = forge() {
        for c in &mut changes {
            c.url = Some(format!("{base}/commit/{}", c.commit));
        }
    }
    let mut kinds: Vec<_> = changes.iter().map(|c| c.kind).collect();
    kinds.sort_by_key(|k| k.rank());
    kinds.dedup();
    kinds
        .into_iter()
        .map(|kind| ChangeGroup {
            kind,
            heading: kind.heading().to_string(),
            rank: kind.rank(),
            changes: changes.iter().filter(|c| c.kind == kind).cloned().collect(),
        })
        .collect()
}

/// One release record, from its metadata.
fn record_of(metadata: &Value, diagnostics: &mut Vec<String>) -> Option<Record> {
    let version = metadata.get("version")?.as_str()?.to_string();
    let tag = metadata
        .get("tag")
        .and_then(|v| v.as_str())
        .unwrap_or(&version)
        .to_string();
    let date = metadata
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let Some(commit) = metadata.get("commit").and_then(|v| v.as_str()) else {
        diagnostics.push(format!(
            "the record for {version} names no commit, so its changes cannot be read"
        ));
        return None;
    };
    let artifacts = metadata
        .get("artifacts")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| {
                    Some(Artifact {
                        target: v.get("target")?.as_str()?.to_string(),
                        name: v.get("name")?.as_str()?.to_string(),
                        sha256: v
                            .get("sha256")
                            .and_then(|s| s.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Record {
        version,
        tag,
        date,
        commit: commit.to_string(),
        notes: metadata
            .get("notes_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        artifacts,
    })
}

/// When the file that states a decision was added, as git knows it.
///
/// The date in an ADR's front matter is when the decision was *made*, which is not the same
/// question as which release carried it — and using it put two decisions written on 2026-09-09
/// into a release published on 2026-09-08, because both ends were compared as calendar days.
/// The commits half of this module already asks git; the decisions half asks git now too.
fn added_at(root: &Path, path: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "log",
            "--diff-filter=A",
            "--format=%H",
            "--max-count=1",
            "--",
            path,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}

/// Whether `commit` is an ancestor of `of` — that is, whether it was already in that tree.
fn is_in(root: &Path, commit: &str, of: &str) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", "--is-ancestor", commit, of])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Every decision the layer holds, newest first.
fn decisions_of(root: &Path, objects: &[Object]) -> Vec<Decision> {
    let mut out: Vec<Decision> = objects
        .iter()
        .filter(|o| o.kind == ADR_KIND)
        .filter_map(|o| {
            let id = o.metadata.get("id")?.as_str()?.to_string();
            Some(Decision {
                title: o
                    .title
                    .clone()
                    .or_else(|| {
                        o.metadata
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| id.clone()),
                status: o
                    .metadata
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                date: o
                    .metadata
                    .get("date")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                url: forge().map(|base| format!("{base}/blob/master/{}", o.provenance.path)),
                // Which release carried it is a question for git, not for the front matter:
                // the date says when the decision was made, and two ADRs written the day
                // after a release were claimed by it while this read that field instead.
                added: added_at(root, &o.provenance.path),
                id,
            })
        })
        .collect();
    out.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.id.cmp(&a.id)));
    out
}

/// The decisions not yet in any release: their file was added after the newest release's tree.
fn decisions_after(root: &Path, decisions: &[Decision], newest: Option<&str>) -> Vec<Decision> {
    decisions
        .iter()
        .filter(|d| match (newest, d.added.as_deref()) {
            // added, and not already in the last release's tree
            (Some(rel), Some(added)) => !is_in(root, added, rel),
            // nothing released yet, or git could not say when it was added: an unreleased
            // section that omits a decision is worse than one that shows an early arrival
            _ => true,
        })
        .cloned()
        .collect()
}

/// The decisions this release carried: in its tree, and not in the one before it.
fn decisions_between(
    root: &Path,
    decisions: &[Decision],
    previous: Option<&str>,
    release: &str,
) -> Vec<Decision> {
    decisions
        .iter()
        .filter(|d| {
            let Some(added) = d.added.as_deref() else {
                return false;
            };
            is_in(root, added, release) && previous.is_none_or(|p| !is_in(root, added, p))
        })
        .cloned()
        .collect()
}

/// The `YYYY-MM-DD` of an ISO-8601 date or timestamp.
fn day(text: &str) -> &str {
    text.split('T').next().unwrap_or(text)
}

/// The changelog as Markdown, which is what the generated document and the site render.
pub fn render(changelog: &Changelog) -> String {
    let mut out = String::new();
    out.push_str("# Changelog\n\n");
    out.push_str(
        "Every entry below is derived: a section per release the layer records, its decisions \
         the ADRs dated inside that release's window, its changes the conventional commits in \
         its range, its artifacts the record's own evidence. Nothing here is written by hand, \
         and `majordomus generate changelog --check` fails when it stops matching the tree.\n\n",
    );
    out.push_str(&format!("Current version: **{}**\n", changelog.current));

    for section in &changelog.sections {
        out.push('\n');
        if section.unreleased {
            out.push_str("## Unreleased\n\n");
        } else {
            let date = section.date.as_deref().unwrap_or("");
            out.push_str(&format!(
                "## {} — {}\n\n",
                section.tag.as_deref().unwrap_or(&section.version),
                day(date)
            ));
        }

        if !section.decisions.is_empty() {
            out.push_str("### Decisions\n\n");
            for d in &section.decisions {
                out.push_str(&format!(
                    "- **{}** {} _({})_\n",
                    d.id.to_uppercase(),
                    d.title,
                    d.status
                ));
            }
            out.push('\n');
        }

        // The groups the document carries, in the order it carries them: this renderer no
        // longer decides the order, it reads it, which is what lets the site agree with it.
        for group in &section.groups {
            out.push_str(&format!("### {}\n\n", group.heading));
            for c in &group.changes {
                let scope = c
                    .scope
                    .as_ref()
                    .map(|s| format!("**{s}**: "))
                    .unwrap_or_default();
                let breaking = if c.breaking { "**BREAKING** " } else { "" };
                out.push_str(&format!(
                    "- {breaking}{scope}{} (`{}`)\n",
                    c.subject, c.commit
                ));
            }
            out.push('\n');
        }

        if !section.artifacts.is_empty() {
            out.push_str("### Published\n\n");
            for a in &section.artifacts {
                out.push_str(&format!(
                    "- `{}` — {} (`{}`)\n",
                    a.target,
                    a.name,
                    &a.sha256[..a.sha256.len().min(12)]
                ));
            }
            out.push('\n');
        }
    }

    if !changelog.diagnostics.is_empty() {
        out.push_str("\n## What could not be read\n\n");
        for d in &changelog.diagnostics {
            out.push_str(&format!("- {d}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::model::ChangeKind;

    fn decision(id: &str, date: &str) -> Decision {
        Decision {
            id: id.into(),
            title: "t".into(),
            status: "accepted".into(),
            date: date.into(),
            url: None,
            added: None,
        }
    }

    fn git(root: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A repository whose decisions arrive one file per commit, so that git can say when each
    /// was added: the fact the two window functions read. Returns the commit of each arrival,
    /// in order.
    fn repo_with_decisions(ids: &[&str]) -> (tempfile::TempDir, Vec<String>) {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path();
        git(root, &["init", "-q"]);
        std::fs::create_dir_all(root.join(".ai/repo/decisions")).expect("the decisions dir");
        let mut arrivals = Vec::new();
        for id in ids {
            std::fs::write(
                root.join(format!(".ai/repo/decisions/{id}.md")),
                "decided\n",
            )
            .expect("a decision file");
            git(root, &["add", "-A"]);
            git(root, &["commit", "-q", "-m", &format!("docs(adr): {id}")]);
            arrivals.push(git(root, &["rev-parse", "HEAD"]));
        }
        (dir, arrivals)
    }

    /// An ADR object as the index would hold it, dated on the same day as every other one
    /// here: the date is deliberately useless, so that only git's answer can pass the test.
    fn adr_object(id: &str) -> Object {
        let mut metadata = serde_json::Map::new();
        metadata.insert("id".into(), id.into());
        metadata.insert("status".into(), "accepted".into());
        metadata.insert("date".into(), "2026-09-01".into());
        Object {
            kind: ADR_KIND.into(),
            identity: id.into(),
            uri: format!("majordomus://adr/{id}"),
            title: Some("t".into()),
            description: None,
            metadata: serde_json::Value::Object(metadata),
            body: String::new(),
            content: String::new(),
            media_type: "text/markdown",
            provenance: crate::model::Provenance {
                path: format!(".ai/repo/decisions/{id}.md"),
                directory: ".ai/repo/decisions".into(),
                source_class: "decision".into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    #[test]
    fn a_decision_belongs_to_the_release_whose_tree_first_holds_it() {
        let (dir, at) = repo_with_decisions(&["adr-0001", "adr-0002", "adr-0003"]);
        let all = decisions_of(
            dir.path(),
            &[
                adr_object("adr-0001"),
                adr_object("adr-0002"),
                adr_object("adr-0003"),
            ],
        );
        assert!(
            all.iter().all(|d| d.added.is_some()),
            "git says when each file was added"
        );
        // (first tree, second tree] — the second only, whatever its front matter is dated.
        let between = decisions_between(dir.path(), &all, Some(&at[0]), &at[1]);
        assert_eq!(between.len(), 1);
        assert_eq!(between[0].id, "adr-0002");
        // The first release has no predecessor, so it carries everything its tree holds.
        let first = decisions_between(dir.path(), &all, None, &at[0]);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, "adr-0001");
    }

    #[test]
    fn the_unreleased_section_holds_what_no_released_tree_holds() {
        let (dir, at) = repo_with_decisions(&["adr-0001", "adr-0003"]);
        let all = decisions_of(
            dir.path(),
            &[adr_object("adr-0001"), adr_object("adr-0003")],
        );
        let after = decisions_after(dir.path(), &all, Some(&at[0]));
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, "adr-0003");
        // No release at all: everything is unreleased.
        assert_eq!(decisions_after(dir.path(), &all, None).len(), 2);
    }

    /// A throwaway repository with `n` commits, the first of them tagged as a release the
    /// records name. What every test of the two compositions needs: a real HEAD that moves.
    fn repo_with_commits(n: usize) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .expect("git runs");
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "feat(a): the release",
        ]);
        let release = git(&["rev-parse", "HEAD"]);
        for i in 1..n {
            git(&[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                &format!("fix(b): after the release {i}"),
            ]);
        }
        (dir, release)
    }

    fn release_record(commit: &str) -> Object {
        let mut metadata = serde_json::Map::new();
        metadata.insert("schema".into(), "release/v1".into());
        metadata.insert("version".into(), "0.1.0".into());
        metadata.insert("tag".into(), "v0.1.0".into());
        metadata.insert("channel".into(), "stable".into());
        metadata.insert("commit".into(), commit.into());
        metadata.insert("published_at".into(), "2026-09-01T00:00:00Z".into());
        metadata.insert("artifacts".into(), serde_json::Value::Array(Vec::new()));
        Object {
            kind: RELEASE_KIND.into(),
            identity: "v0.1.0".into(),
            uri: "majordomus://release-record/v0.1.0".into(),
            title: None,
            description: None,
            metadata: serde_json::Value::Object(metadata),
            body: String::new(),
            content: String::new(),
            media_type: "application/yaml",
            provenance: crate::model::Provenance {
                path: ".ai/repo/releases/v0.1.0.yaml".into(),
                directory: ".ai/repo/releases".into(),
                source_class: "release".into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    /// The property the committed artifact exists to have: a commit that publishes nothing
    /// does not change it. The live form is the one that follows HEAD.
    #[test]
    fn the_published_changelog_is_the_same_bytes_at_every_commit_after_the_release() {
        let (dir, release) = repo_with_commits(2);
        let objects = vec![release_record(&release)];
        let before = compose_published(dir.path(), &objects);
        let live_before = compose(dir.path(), &objects);

        std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args([
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "fix(c): one more commit",
            ])
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .status()
            .expect("git runs");

        let after = compose_published(dir.path(), &objects);
        let live_after = compose(dir.path(), &objects);

        assert_eq!(
            serde_json::to_string(&before).unwrap(),
            serde_json::to_string(&after).unwrap(),
            "a commit that publishes nothing must not move the committed changelog"
        );
        assert!(
            !before.sections.iter().any(|s| s.unreleased),
            "the committed form carries no unreleased section"
        );
        assert!(
            live_before.sections.iter().any(|s| s.unreleased),
            "the live form does"
        );
        assert_ne!(
            serde_json::to_string(&live_before).unwrap(),
            serde_json::to_string(&live_after).unwrap(),
            "and the live form follows HEAD"
        );
    }

    /// A repository that has never published: the live form says so with an unreleased
    /// section, and the committed form says so with nothing — there is nothing published to
    /// describe, and an empty section would be a claim.
    #[test]
    fn with_no_release_the_published_form_is_empty_and_the_live_form_is_unreleased() {
        let (dir, _) = repo_with_commits(1);
        let published = compose_published(dir.path(), &[]);
        assert!(published.sections.is_empty());
        let live = compose(dir.path(), &[]);
        assert_eq!(live.sections.len(), 1);
        assert!(live.sections[0].unreleased);
    }

    #[test]
    fn a_timestamp_and_a_date_compare_on_the_day_they_share() {
        assert_eq!(day("2026-09-09T15:04:05Z"), "2026-09-09");
        assert_eq!(day("2026-09-09"), "2026-09-09");
    }

    #[test]
    fn the_rendering_groups_by_kind_and_marks_what_breaks() {
        let changelog = Changelog {
            schema: CHANGELOG_SCHEMA.into(),
            current: "0.4.0".into(),
            sections: vec![ReleaseSection {
                version: "unreleased".into(),
                tag: None,
                date: None,
                commit: None,
                unreleased: true,
                notes_url: None,
                compare_url: None,
                tree_url: None,
                decisions: vec![decision("adr-0027", "2026-09-09")],
                // Given in the order the commits arrived — a fix first — so that the
                // grouping, not the input, is what decides the order the renderer shows.
                groups: grouped(vec![
                    Change {
                        kind: ChangeKind::Fix,
                        scope: Some("ci".into()),
                        subject: "the gate runs".into(),
                        breaking: false,
                        commit: "aaa1111".into(),
                        url: None,
                        references: Vec::new(),
                    },
                    Change {
                        kind: ChangeKind::Feat,
                        scope: Some("commands".into()),
                        subject: "one graph".into(),
                        breaking: true,
                        commit: "bbb2222".into(),
                        url: None,
                        references: Vec::new(),
                    },
                ]),
                artifacts: Vec::new(),
            }],
            diagnostics: Vec::new(),
            produced_by: None,
        };
        let md = render(&changelog);
        assert!(md.contains("## Unreleased"));
        assert!(md.contains("**ADR-0027**"));
        // Added before Fixed, whatever order the commits arrived in.
        let added = md.find("### Added").expect("no Added heading");
        let fixed = md.find("### Fixed").expect("no Fixed heading");
        assert!(added < fixed, "the headings are out of order:\n{md}");
        assert!(md.contains("**BREAKING** **commands**: one graph"));
    }
}
