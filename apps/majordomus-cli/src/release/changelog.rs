//! The changelog: one model, several renderings.
//!
//! `CHANGELOG.md`, the GitHub release body, the Cockpit's release page, the site's
//! changelog and the API's answer are the same document rendered five ways. Nothing here
//! is authored: a section is the change records that carry a version, in the order the
//! reader wants them, beside the release record that says when it was published.
//!
//! # Determinism
//!
//! Same records, same output, byte for byte. The only value that could vary between two
//! runs is the release date, and that is read from the release record — written once, when
//! the release was actually published — rather than from the clock. A section with no date
//! is a section for a version that has been prepared and not published, and it says so
//! instead of stamping today.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::distribution::release::Releases;

use super::change::{Change, ChangeType, Changes};
use super::diff::CompatibilityImpact;
use super::version::Version;

/// One release's worth of changelog, or the unreleased one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ChangelogSection")]
pub struct Section {
    /// The version, absent for the unreleased section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<Version>,
    /// The tag, absent for the unreleased section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The day it was published, UTC, from the release record. Absent when the version has
    /// been stamped on records but no release was published for it — which is a state
    /// worth seeing rather than papering over with today's date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// The commit it was built from, from the release record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// True when the record says the release was withdrawn.
    pub yanked: bool,
    /// The strongest impact among this section's changes.
    pub impact: CompatibilityImpact,
    /// The changes, grouped by type, in the reader's order. A type with no change is not
    /// a key here: an empty ceremonial section is noise.
    pub groups: Vec<Group>,
    /// The migration documents this section's breaking changes name, deduplicated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub migrations: Vec<String>,
}

impl Section {
    /// How many changes the section carries.
    pub fn len(&self) -> usize {
        self.groups.iter().map(|g| g.changes.len()).sum()
    }

    /// True when the section carries no change at all.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// One type's worth of a section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ChangelogGroup")]
pub struct Group {
    /// The type.
    #[serde(rename = "type")]
    pub change_type: ChangeType,
    /// The heading a renderer prints.
    pub heading: String,
    /// The changes, by id.
    pub changes: Vec<Change>,
}

/// The whole changelog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Changelog {
    /// The changes not yet published, when there are any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreleased: Option<Section>,
    /// Every published version, newest first.
    pub releases: Vec<Section>,
}

impl Changelog {
    /// Build the changelog from the change records and the release records.
    ///
    /// The two are joined on the version: a change says which version published it, a
    /// release record says when that version was published and from which commit. Neither
    /// restates the other, and a version that has changes but no release record still
    /// appears — as a section with no date, which is exactly what "prepared, not published"
    /// looks like.
    pub fn build(changes: &Changes, releases: &Releases) -> Self {
        let published: BTreeMap<String, &crate::distribution::Release> = releases
            .releases
            .iter()
            .map(|r| (r.version.clone(), r))
            .collect();

        let unreleased = section(None, changes.unreleased().collect(), None);
        let mut sections = Vec::new();
        for version in changes.versions() {
            let record = published.get(&version.to_string()).copied();
            let members: Vec<&Change> = changes.released_in(&version).collect();
            if let Some(s) = section(Some(version.clone()), members, record) {
                sections.push(s);
            }
        }
        // A published release with no change record still belongs in the changelog: it
        // happened, and a changelog that omits it is a changelog with a hole in it.
        for record in &releases.releases {
            let Ok(version) = record.version.parse::<Version>() else {
                continue;
            };
            if sections
                .iter()
                .any(|s| s.version.as_ref() == Some(&version))
            {
                continue;
            }
            sections.push(Section {
                version: Some(version),
                tag: Some(record.tag.clone()),
                date: Some(day(&record.published_at)),
                commit: Some(record.commit.clone()),
                yanked: record.yanked,
                impact: CompatibilityImpact::None,
                groups: Vec::new(),
                migrations: Vec::new(),
            });
        }
        sections.sort_by(|a, b| b.version.cmp(&a.version));
        Self {
            unreleased,
            releases: sections,
        }
    }

    /// The section for one version.
    pub fn release(&self, version: &Version) -> Option<&Section> {
        self.releases
            .iter()
            .find(|s| s.version.as_ref() == Some(version))
    }

    /// The whole document as `CHANGELOG.md` carries it.
    ///
    /// The header is fixed prose; everything below it is the model. A reader who edits
    /// this file loses the edit at the next generation, which the header says.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Changelog\n\n");
        out.push_str(
            "Every entry below is a record under `.ai/repo/changes/`, rendered here and in the \
             release notes, the Cockpit, the API and the site from that one record. \
             `majordomus release changelog` prints the same document.\n",
        );
        if let Some(unreleased) = &self.unreleased {
            out.push('\n');
            out.push_str("## Unreleased\n\n");
            render_body(&mut out, unreleased);
        }
        for section in &self.releases {
            out.push('\n');
            let version = section
                .version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            out.push_str(&format!("## {version}"));
            if let Some(date) = &section.date {
                out.push_str(&format!(" — {date}"));
            } else {
                out.push_str(" — unpublished");
            }
            if section.yanked {
                out.push_str(" (withdrawn)");
            }
            out.push_str("\n\n");
            if section.impact != CompatibilityImpact::None {
                out.push_str(&format!("Compatibility: **{}**.", section.impact));
                if let Some(commit) = &section.commit {
                    out.push_str(&format!(
                        " Built from `{}`.",
                        &commit[..commit.len().min(12)]
                    ));
                }
                out.push_str("\n\n");
            } else if let Some(commit) = &section.commit {
                out.push_str(&format!(
                    "Built from `{}`.\n\n",
                    &commit[..commit.len().min(12)]
                ));
            }
            render_body(&mut out, section);
        }
        out
    }

    /// The body of one release's notes, for a release page or a GitHub release.
    ///
    /// The same rendering as a section of `CHANGELOG.md` without the file's own header, so
    /// that a release note and a changelog entry cannot say different things.
    pub fn notes(&self, version: &Version) -> Option<String> {
        let section = self.release(version)?;
        let mut out = String::new();
        if section.impact != CompatibilityImpact::None {
            out.push_str(&format!("Compatibility: **{}**.\n\n", section.impact));
        }
        render_body(&mut out, section);
        Some(out)
    }
}

/// One section's groups and migration links.
fn render_body(out: &mut String, section: &Section) {
    if section.is_empty() {
        out.push_str("_No recorded change._\n");
        return;
    }
    for group in &section.groups {
        out.push_str(&format!("### {}\n\n", group.heading));
        for change in &group.changes {
            out.push_str(&format!("- {}", change.title));
            let mut marks = Vec::new();
            if change.impact == CompatibilityImpact::Breaking {
                marks.push("**breaking**".to_string());
            }
            for issue in &change.issues {
                marks.push(format!("#{issue}"));
            }
            for pr in &change.pull_requests {
                marks.push(format!("PR #{pr}"));
            }
            for adr in &change.adrs {
                marks.push(adr.clone());
            }
            if !marks.is_empty() {
                out.push_str(&format!(" ({})", marks.join(", ")));
            }
            out.push('\n');
        }
        out.push('\n');
    }
    if !section.migrations.is_empty() {
        out.push_str("### Migration\n\n");
        for path in &section.migrations {
            out.push_str(&format!("- [{path}]({path})\n"));
        }
        out.push('\n');
    }
}

/// A section over a set of changes, or `None` when there are none.
fn section(
    version: Option<Version>,
    members: Vec<&Change>,
    record: Option<&crate::distribution::Release>,
) -> Option<Section> {
    if members.is_empty() {
        return None;
    }
    let mut groups = Vec::new();
    for change_type in ChangeType::ORDER {
        let mut changes: Vec<Change> = members
            .iter()
            .filter(|c| c.change_type == *change_type)
            .map(|c| (*c).clone())
            .collect();
        if changes.is_empty() {
            continue;
        }
        changes.sort_by(|a, b| a.id.cmp(&b.id));
        groups.push(Group {
            change_type: *change_type,
            heading: change_type.heading().to_string(),
            changes,
        });
    }
    let impact = members
        .iter()
        .map(|c| c.impact)
        .fold(CompatibilityImpact::None, CompatibilityImpact::max);
    let mut migrations: Vec<String> = members.iter().filter_map(|c| c.migration.clone()).collect();
    migrations.sort();
    migrations.dedup();
    Some(Section {
        version,
        tag: record.map(|r| r.tag.clone()),
        date: record.map(|r| day(&r.published_at)),
        commit: record.map(|r| r.commit.clone()),
        yanked: record.is_some_and(|r| r.yanked),
        impact,
        groups,
        migrations,
    })
}

/// The date part of a UTC timestamp.
///
/// A changelog is read by a person, and a person wants the day. The full instant stays in
/// the release record, which is where a machine reads it.
fn day(timestamp: &str) -> String {
    timestamp.split('T').next().unwrap_or(timestamp).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::change::Changes;

    fn change(id: &str, t: ChangeType, released: Option<&str>) -> Change {
        Change {
            id: id.into(),
            title: format!("the thing called {id}"),
            change_type: t,
            impact: CompatibilityImpact::Additive,
            released_in: released.map(|v| v.parse().unwrap()),
            scopes: Vec::new(),
            contract: Vec::new(),
            issues: Vec::new(),
            pull_requests: Vec::new(),
            commits: Vec::new(),
            adrs: Vec::new(),
            migration: None,
            summary: String::new(),
            path: format!(".ai/repo/changes/{id}.md"),
        }
    }

    fn changes(list: Vec<Change>) -> Changes {
        let mut c = Changes {
            changes: list,
            unreadable: Vec::new(),
        };
        c.changes.sort_by(|a, b| {
            match (&a.released_in, &b.released_in) {
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(x), Some(y)) => y.cmp(x),
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| a.change_type.cmp(&b.change_type))
            .then_with(|| a.id.cmp(&b.id))
        });
        c
    }

    fn no_releases() -> Releases {
        Releases {
            releases: Vec::new(),
        }
    }

    #[test]
    fn an_unreleased_change_lands_in_the_unreleased_section() {
        let log = Changelog::build(
            &changes(vec![change("a", ChangeType::Added, None)]),
            &no_releases(),
        );
        let unreleased = log.unreleased.expect("there is one");
        assert_eq!(unreleased.len(), 1);
        assert!(log.releases.is_empty());
    }

    #[test]
    fn a_type_with_no_change_gets_no_heading() {
        let log = Changelog::build(
            &changes(vec![change("a", ChangeType::Fixed, None)]),
            &no_releases(),
        );
        let text = log.to_markdown();
        assert!(text.contains("### Fixed"));
        assert!(!text.contains("### Added"), "no empty ceremonial section");
        assert!(!text.contains("### Security"));
    }

    #[test]
    fn the_groups_are_in_the_readers_order_not_the_alphabets() {
        let log = Changelog::build(
            &changes(vec![
                change("z", ChangeType::Security, None),
                change("a", ChangeType::Added, None),
                change("m", ChangeType::Fixed, None),
            ]),
            &no_releases(),
        );
        let headings: Vec<ChangeType> = log
            .unreleased
            .unwrap()
            .groups
            .iter()
            .map(|g| g.change_type)
            .collect();
        assert_eq!(
            headings,
            vec![ChangeType::Added, ChangeType::Fixed, ChangeType::Security]
        );
    }

    #[test]
    fn a_version_with_changes_and_no_release_record_says_it_is_unpublished() {
        let log = Changelog::build(
            &changes(vec![change("a", ChangeType::Added, Some("0.4.0"))]),
            &no_releases(),
        );
        let text = log.to_markdown();
        assert!(text.contains("## 0.4.0 — unpublished"), "{text}");
    }

    #[test]
    fn the_document_is_byte_identical_for_the_same_records() {
        let records = || {
            changes(vec![
                change("a", ChangeType::Added, None),
                change("b", ChangeType::Fixed, None),
            ])
        };
        assert_eq!(
            Changelog::build(&records(), &no_releases()).to_markdown(),
            Changelog::build(&records(), &no_releases()).to_markdown(),
        );
    }

    #[test]
    fn the_order_of_the_records_does_not_change_the_document() {
        let forward = changes(vec![
            change("a", ChangeType::Added, None),
            change("b", ChangeType::Added, None),
        ]);
        let backward = changes(vec![
            change("b", ChangeType::Added, None),
            change("a", ChangeType::Added, None),
        ]);
        assert_eq!(
            Changelog::build(&forward, &no_releases()).to_markdown(),
            Changelog::build(&backward, &no_releases()).to_markdown(),
        );
    }

    #[test]
    fn a_breaking_change_is_marked_and_its_migration_is_linked() {
        let mut c = change("b", ChangeType::Removed, None);
        c.impact = CompatibilityImpact::Breaking;
        c.migration = Some("docs/migrations/0-4-0.md".into());
        let log = Changelog::build(&changes(vec![c]), &no_releases());
        let text = log.to_markdown();
        assert!(text.contains("**breaking**"), "{text}");
        assert!(text.contains("[docs/migrations/0-4-0.md]"), "{text}");
        assert_eq!(
            log.unreleased.unwrap().impact,
            CompatibilityImpact::Breaking
        );
    }

    #[test]
    fn issues_and_pull_requests_are_rendered_from_the_record_and_nowhere_else() {
        let mut c = change("a", ChangeType::Added, None);
        c.issues = vec!["42".into()];
        c.pull_requests = vec!["117".into()];
        c.adrs = vec!["adr-0028".into()];
        let text = Changelog::build(&changes(vec![c]), &no_releases()).to_markdown();
        assert!(text.contains("(#42, PR #117, adr-0028)"), "{text}");
    }

    #[test]
    fn a_release_note_and_a_changelog_entry_carry_the_same_body() {
        let log = Changelog::build(
            &changes(vec![change("a", ChangeType::Added, Some("0.4.0"))]),
            &no_releases(),
        );
        let notes = log.notes(&"0.4.0".parse().unwrap()).expect("a section");
        assert!(notes.contains("the thing called a"));
        assert!(log.to_markdown().contains("the thing called a"));
        assert!(
            !notes.contains("# Changelog"),
            "the notes carry no file header"
        );
    }

    /// The document opens with its own title and says where its entries come from. The
    /// provenance banner is the generator's, added when the artifact is written, so that
    /// this rendering is the one a person reads at a command line and the one committed to
    /// the file, differing only by the header every generated file carries.
    #[test]
    fn the_document_names_the_records_it_renders() {
        let text = Changelog::build(&changes(Vec::new()), &no_releases()).to_markdown();
        assert!(text.starts_with("# Changelog"), "{text}");
        assert!(text.contains(".ai/repo/changes/"));
    }
}
