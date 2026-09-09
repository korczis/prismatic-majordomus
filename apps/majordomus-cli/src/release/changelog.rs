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
    artifacts: Vec<Artifact>,
}

/// Compose the changelog.
///
/// `objects` is the index's own view of the layer; `root` is the repository git reads. A
/// repository with no records yields one unreleased section, which is the right answer for a
/// project that has never published rather than an error.
pub fn compose(root: &Path, objects: &[Object]) -> Changelog {
    let mut diagnostics = Vec::new();

    let mut records: Vec<Record> = objects
        .iter()
        .filter(|o| o.kind == RELEASE_KIND)
        .filter_map(|o| record_of(&o.metadata, &mut diagnostics))
        .collect();
    // Newest first, by the date the record carries: the order a changelog is read in.
    records.sort_by(|a, b| b.date.cmp(&a.date));

    let decisions = decisions_of(objects);

    let mut sections = Vec::new();

    // The unreleased section: everything after the newest release, when there is any.
    let newest = records.first();
    let unreleased_range = match newest {
        Some(r) => format!("{}..HEAD", r.commit),
        None => "HEAD".to_string(),
    };
    let unreleased_changes = commits::in_range(root, &unreleased_range);
    if newest.is_some() && unreleased_changes.is_empty() {
        // nothing since the last release; no section rather than an empty one
    } else {
        sections.push(ReleaseSection {
            version: "unreleased".into(),
            tag: None,
            date: None,
            commit: None,
            unreleased: true,
            decisions: decisions_after(&decisions, newest.map(|r| r.date.as_str())),
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
        let changes = commits::in_range(root, &range);
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
            decisions: decisions_between(&decisions, previous.map(|p| p.date.as_str()), &r.date),
            groups: grouped(changes),
            artifacts: r.artifacts.clone(),
        });
    }

    Changelog {
        schema: CHANGELOG_SCHEMA.into(),
        current: version::declared(root).unwrap_or_else(|| "unknown".into()),
        sections,
        diagnostics,
    }
}

/// The changes of a section, grouped by kind and ordered by the rank each kind carries.
///
/// Done here rather than in each renderer: the Markdown, the site and any other reader get
/// the same order because they are given it, not because they each reimplemented it.
fn grouped(changes: Vec<Change>) -> Vec<ChangeGroup> {
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
        artifacts,
    })
}

/// Every decision the layer holds, newest first.
fn decisions_of(objects: &[Object]) -> Vec<Decision> {
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
                id,
            })
        })
        .collect();
    out.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.id.cmp(&a.id)));
    out
}

/// The decisions dated after `after`, for the unreleased section.
///
/// Dates are compared as the strings they are: both sources write ISO-8601, where
/// lexicographic order is chronological order. A record's `published_at` carries a time and
/// an ADR's `date` does not, so the comparison is made on the date half of each.
fn decisions_after(decisions: &[Decision], after: Option<&str>) -> Vec<Decision> {
    let after = after.map(day);
    decisions
        .iter()
        .filter(|d| match after {
            Some(a) => day(&d.date) > a,
            None => true,
        })
        .cloned()
        .collect()
}

/// The decisions dated inside `(after, until]`.
fn decisions_between(decisions: &[Decision], after: Option<&str>, until: &str) -> Vec<Decision> {
    let until = day(until);
    let after = after.map(day);
    decisions
        .iter()
        .filter(|d| {
            let d = day(&d.date);
            d <= until && after.is_none_or(|a| d > a)
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
        }
    }

    #[test]
    fn a_decision_belongs_to_the_release_whose_window_contains_its_date() {
        let all = vec![
            decision("adr-0001", "2026-09-01"),
            decision("adr-0002", "2026-09-05"),
            decision("adr-0003", "2026-09-09"),
        ];
        // (2026-09-03, 2026-09-06] — the second only.
        let between = decisions_between(&all, Some("2026-09-03T10:00:00Z"), "2026-09-06T00:00:00Z");
        assert_eq!(between.len(), 1);
        assert_eq!(between[0].id, "adr-0002");
        // The first release has no predecessor, so its window opens at the beginning.
        let first = decisions_between(&all, None, "2026-09-03");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, "adr-0001");
    }

    #[test]
    fn the_unreleased_window_opens_after_the_last_release() {
        let all = vec![
            decision("adr-0001", "2026-09-01"),
            decision("adr-0003", "2026-09-09"),
        ];
        let after = decisions_after(&all, Some("2026-09-05T00:00:00Z"));
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, "adr-0003");
        // No release at all: everything is unreleased.
        assert_eq!(decisions_after(&all, None).len(), 2);
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
                    },
                    Change {
                        kind: ChangeKind::Feat,
                        scope: Some("commands".into()),
                        subject: "one graph".into(),
                        breaking: true,
                        commit: "bbb2222".into(),
                    },
                ]),
                artifacts: Vec::new(),
            }],
            diagnostics: Vec::new(),
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
