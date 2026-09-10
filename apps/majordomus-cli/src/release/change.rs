//! The release changes a repository records, and what they say about a release.
//!
//! A release has two halves. The contract diff is the mechanical half: it knows that a
//! capability appeared, that a route moved, that a target was withdrawn, and it knows it
//! without anybody writing it down. The other half is everything the contract cannot see —
//! a bug fixed, a behaviour moved inside an unchanged signature, the reason a break was
//! worth making, and the way through it — and that half is authored, once, as a `change`
//! document under `.ai/repo/changes/`.
//!
//! ```text
//!   .ai/repo/changes/*.md ──▶ Change ──┐
//!                                      ├──▶ Changelog ──▶ CHANGELOG.md · release notes
//!   contract diff ─────────────────────┘                  · Cockpit · API · MCP · site
//! ```
//!
//! Nothing renders a release fact from anywhere else. A record with no `released_in` is
//! unreleased and belongs to whatever the next release turns out to be; `release prepare`
//! is what stamps it, and it is the only thing that does.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Object;

use super::diff::CompatibilityImpact;
use super::version::Version;

/// The schema a change document declares.
pub const SCHEMA: &str = "change/v1";

/// The section of a changelog a change appears under.
///
/// The six of *Keep a Changelog*, which every reader of a changelog already knows, and no
/// seventh: a vocabulary a project invents for itself is a vocabulary its readers have to
/// learn.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "ReleaseChangeType")]
pub enum ChangeType {
    /// Something that did not exist before.
    Added,
    /// Something that existed and behaves differently.
    Changed,
    /// Something still here and on its way out.
    Deprecated,
    /// Something that is gone.
    Removed,
    /// Something that was wrong and is not.
    Fixed,
    /// Something that was exploitable and is not.
    Security,
}

impl ChangeType {
    /// The word the front matter carries.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Deprecated => "deprecated",
            Self::Removed => "removed",
            Self::Fixed => "fixed",
            Self::Security => "security",
        }
    }

    /// The heading a changelog prints for this section.
    pub fn heading(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Changed => "Changed",
            Self::Deprecated => "Deprecated",
            Self::Removed => "Removed",
            Self::Fixed => "Fixed",
            Self::Security => "Security",
        }
    }

    /// The type a word names.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "added" => Self::Added,
            "changed" => Self::Changed,
            "deprecated" => Self::Deprecated,
            "removed" => Self::Removed,
            "fixed" => Self::Fixed,
            "security" => Self::Security,
            _ => return None,
        })
    }

    /// Every type, in the order a changelog prints them.
    ///
    /// The order is the reader's, not the alphabet's: what is new, then what moved, then
    /// what is going, then what is gone, then what was wrong, then what was dangerous.
    pub const ORDER: &'static [Self] = &[
        Self::Added,
        Self::Changed,
        Self::Deprecated,
        Self::Removed,
        Self::Fixed,
        Self::Security,
    ];
}

/// One authored release change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseChange")]
pub struct Change {
    /// The identity, allocated once and never reused.
    pub id: String,
    /// One line, present tense, as the changelog prints it.
    pub title: String,
    /// The section it appears under.
    #[serde(rename = "type")]
    pub change_type: ChangeType,
    /// What it costs a caller, as the record claims. Checked against the contract diff:
    /// a record may not understate what the contract says.
    pub impact: CompatibilityImpact,
    /// The version it was published in; absent means unreleased.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_in: Option<Version>,
    /// The surfaces a reader cares about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// The contract entries this record accounts for, `<surface>:<id>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contract: Vec<String>,
    /// The issues it closes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<String>,
    /// The pull requests that carried it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pull_requests: Vec<String>,
    /// The commits that carried it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<String>,
    /// The decisions behind it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adrs: Vec<String>,
    /// The migration guidance, repository-relative. Required for a breaking change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<String>,
    /// The summary section of the body.
    pub summary: String,
    /// The repository-relative path of the record itself.
    pub path: String,
}

impl Change {
    /// True when this change has not been published yet.
    pub fn is_unreleased(&self) -> bool {
        self.released_in.is_none()
    }

    /// Read a change out of an indexed object.
    ///
    /// The object's metadata is the front matter as the layer's reader produced it; this
    /// turns it into the typed record the release engine works with, refusing rather than
    /// guessing on anything malformed. Validation against the document schema has already
    /// happened by the time an object exists — this is the typed view of a valid document,
    /// not a second validator.
    pub fn from_object(object: &Object) -> Result<Self, String> {
        let field = |name: &str| -> Option<&str> {
            object
                .metadata
                .get(name)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
        };
        let list = |name: &str| -> Vec<String> {
            object
                .metadata
                .get(name)
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };
        let at = |what: &str| format!("{}: {what}", object.provenance.path);

        let schema = field("schema").ok_or_else(|| at("no `schema`"))?;
        if schema != SCHEMA {
            return Err(at(&format!(
                "states schema `{schema}` and this build reads `{SCHEMA}`"
            )));
        }
        let id = field("id").ok_or_else(|| at("no `id`"))?.to_string();
        let title = field("title").ok_or_else(|| at("no `title`"))?.to_string();
        let change_type = field("type").and_then(ChangeType::parse).ok_or_else(|| {
            at("`type` is not one of added, changed, deprecated, removed, fixed, security")
        })?;
        let impact = field("impact")
            .and_then(parse_impact)
            .ok_or_else(|| at("`impact` is not one of none, patch, additive, breaking"))?;
        let released_in = match field("released_in") {
            Some(text) => Some(
                text.parse::<Version>()
                    .map_err(|e| at(&format!("`released_in` is not a version: {e}")))?,
            ),
            None => None,
        };
        Ok(Self {
            id,
            title,
            change_type,
            impact,
            released_in,
            scopes: list("scopes"),
            contract: list("contract"),
            issues: list("issues"),
            pull_requests: list("pull_requests"),
            commits: list("commits"),
            adrs: list("adrs"),
            migration: field("migration").map(str::to_string),
            summary: section(&object.body, "Summary"),
            path: object.provenance.path.clone(),
        })
    }
}

/// The impact a word names.
fn parse_impact(word: &str) -> Option<CompatibilityImpact> {
    Some(match word {
        "none" => CompatibilityImpact::None,
        "patch" => CompatibilityImpact::Patch,
        "additive" => CompatibilityImpact::Additive,
        "breaking" => CompatibilityImpact::Breaking,
        _ => return None,
    })
}

/// The text under one level-two heading of a Markdown body.
///
/// The body's sections are already validated by the document schema, so this is a reader
/// and not a parser: it takes the text a valid document is known to have.
fn section(body: &str, heading: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in body.lines() {
        let trimmed = line.trim_end();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if inside {
                break;
            }
            inside = rest.trim() == heading;
            continue;
        }
        if trimmed.starts_with("# ") && inside {
            break;
        }
        if inside {
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// Every change record a repository holds.
#[derive(Debug, Clone, Default)]
pub struct Changes {
    /// The records, in a deterministic order: newest release first, unreleased first of
    /// all, then by type and then by id.
    pub changes: Vec<Change>,
    /// The records that could not be read, with the reason. A malformed record is
    /// reported and never silently dropped: a change nobody can read is a release fact
    /// that would go missing from every projection at once.
    pub unreadable: Vec<String>,
}

impl Changes {
    /// Read every change record out of an index.
    pub fn from_index(index: &crate::Index) -> Self {
        let mut changes = Vec::new();
        let mut unreadable = Vec::new();
        for object in index.objects.iter().filter(|o| o.kind == "change") {
            match Change::from_object(object) {
                Ok(change) => changes.push(change),
                Err(reason) => unreadable.push(reason),
            }
        }
        changes.sort_by(|a, b| {
            // unreleased first, then newest release first
            match (&a.released_in, &b.released_in) {
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(x), Some(y)) => y.cmp(x),
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| a.change_type.cmp(&b.change_type))
            .then_with(|| a.id.cmp(&b.id))
        });
        unreadable.sort();
        Self {
            changes,
            unreadable,
        }
    }

    /// The changes that have not been published.
    pub fn unreleased(&self) -> impl Iterator<Item = &Change> {
        self.changes.iter().filter(|c| c.is_unreleased())
    }

    /// The changes published in one version.
    pub fn released_in<'a>(&'a self, version: &'a Version) -> impl Iterator<Item = &'a Change> {
        self.changes
            .iter()
            .filter(move |c| c.released_in.as_ref() == Some(version))
    }

    /// Every version any record was published in, newest first.
    pub fn versions(&self) -> Vec<Version> {
        let mut versions: Vec<Version> = self
            .changes
            .iter()
            .filter_map(|c| c.released_in.clone())
            .collect();
        versions.sort_by(|a, b| b.cmp(a));
        versions.dedup();
        versions
    }

    /// The contract entries every record between them accounts for.
    pub fn covered(&self) -> std::collections::BTreeSet<&str> {
        self.changes
            .iter()
            .flat_map(|c| c.contract.iter().map(String::as_str))
            .collect()
    }

    /// The records that name a contract entry, by entry.
    pub fn by_contract(&self) -> BTreeMap<&str, Vec<&Change>> {
        let mut out: BTreeMap<&str, Vec<&Change>> = BTreeMap::new();
        for change in &self.changes {
            for entry in &change.contract {
                out.entry(entry.as_str()).or_default().push(change);
            }
        }
        out
    }

    /// Every way the records are unusable.
    ///
    /// Two records claiming one identity is *not* here, deliberately. The layer already
    /// refuses that: the index detects a duplicate identity, drops both objects and reports
    /// it, so by the time a change reaches this type the duplicates are gone and a check
    /// here could never fire. What the release engine must do about it instead is notice
    /// that the layer came back degraded, which is the `LAYER_DEGRADED` diagnostic — a
    /// release decided over a layer that failed to read is not a decision.
    pub fn findings(&self) -> Vec<String> {
        let mut out = self.unreadable.clone();
        for change in &self.changes {
            if change.impact == CompatibilityImpact::Breaking && change.migration.is_none() {
                out.push(format!(
                    "{}: a breaking change states no `migration`, so the release has no way through it",
                    change.path
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn object(metadata: serde_json::Value, body: &str) -> Object {
        Object {
            kind: "change".into(),
            identity: "x".into(),
            uri: "majordomus://change/x".into(),
            title: None,
            description: None,
            metadata,
            body: body.into(),
            content: String::new(),
            media_type: "text/markdown",
            provenance: crate::model::Provenance {
                path: ".ai/repo/changes/x.md".into(),
                directory: ".ai/repo/changes".into(),
                source_class: "change".into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    fn valid() -> serde_json::Value {
        json!({
            "schema": "change/v1",
            "id": "release-identity",
            "kind": "change",
            "title": "the release state is one model",
            "type": "added",
            "impact": "additive",
            "scopes": ["cli", "api"],
            "contract": ["capability:release.status"],
        })
    }

    #[test]
    fn a_valid_record_reads_into_the_typed_change() {
        let change =
            Change::from_object(&object(valid(), "## Summary\n\nWhat it does.\n")).unwrap();
        assert_eq!(change.id, "release-identity");
        assert_eq!(change.change_type, ChangeType::Added);
        assert_eq!(change.impact, CompatibilityImpact::Additive);
        assert!(change.is_unreleased());
        assert_eq!(change.summary, "What it does.");
        assert_eq!(change.contract, ["capability:release.status"]);
    }

    #[test]
    fn a_record_from_a_schema_this_build_does_not_read_is_refused_by_name() {
        let mut m = valid();
        m["schema"] = json!("change/v2");
        let err = Change::from_object(&object(m, "")).unwrap_err();
        assert!(err.contains("change/v2"), "{err}");
    }

    #[test]
    fn a_released_record_carries_the_version_it_was_published_in() {
        let mut m = valid();
        m["released_in"] = json!("0.4.0");
        let change = Change::from_object(&object(m, "## Summary\n\nx\n")).unwrap();
        assert_eq!(change.released_in, Some(Version::new(0, 4, 0)));
        assert!(!change.is_unreleased());
    }

    #[test]
    fn a_released_in_that_is_not_a_version_is_refused_with_the_reason() {
        let mut m = valid();
        m["released_in"] = json!("v0.4");
        let err = Change::from_object(&object(m, "")).unwrap_err();
        assert!(err.contains("not a version"), "{err}");
    }

    #[test]
    fn the_summary_section_is_read_and_the_next_heading_ends_it() {
        let body = "## Summary\n\nThe first part.\n\n## Migration\n\nNot the summary.\n";
        let change = Change::from_object(&object(valid(), body)).unwrap();
        assert_eq!(change.summary, "The first part.");
    }

    #[test]
    fn a_breaking_record_without_a_migration_is_a_finding() {
        let mut m = valid();
        m["impact"] = json!("breaking");
        let change = Change::from_object(&object(m, "## Summary\n\nx\n")).unwrap();
        let changes = Changes {
            changes: vec![change],
            unreadable: Vec::new(),
        };
        let findings = changes.findings();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("no way through it"), "{findings:?}");
    }

    /// A record the reader could not turn into a change is reported rather than dropped:
    /// a release fact that went missing from every projection at once is the failure this
    /// exists to prevent.
    #[test]
    fn an_unreadable_record_is_a_finding() {
        let changes = Changes {
            changes: Vec::new(),
            unreadable: vec![".ai/repo/changes/x.md: no `id`".into()],
        };
        assert_eq!(changes.findings(), [".ai/repo/changes/x.md: no `id`"]);
    }

    #[test]
    fn unreleased_records_sort_before_released_ones_and_releases_sort_newest_first() {
        let mut records = Vec::new();
        for (id, released) in [("c", Some("0.3.0")), ("a", None), ("b", Some("0.4.0"))] {
            let mut m = valid();
            m["id"] = json!(id);
            if let Some(v) = released {
                m["released_in"] = json!(v);
            }
            records.push(Change::from_object(&object(m, "## Summary\n\nx\n")).unwrap());
        }
        let mut changes = Changes {
            changes: records,
            unreadable: Vec::new(),
        };
        changes.changes.sort_by(|a, b| {
            match (&a.released_in, &b.released_in) {
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(x), Some(y)) => y.cmp(x),
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| a.change_type.cmp(&b.change_type))
            .then_with(|| a.id.cmp(&b.id))
        });
        let ids: Vec<&str> = changes.changes.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!(
            changes.versions(),
            vec![Version::new(0, 4, 0), Version::new(0, 3, 0)]
        );
    }

    #[test]
    fn every_type_word_round_trips_and_the_reading_order_is_complete() {
        for t in ChangeType::ORDER {
            assert_eq!(ChangeType::parse(t.as_str()), Some(*t));
            assert!(!t.heading().is_empty());
        }
        assert_eq!(ChangeType::ORDER.len(), 6);
    }
}
