//! The release manifest: one machine-readable document per release, generated.
//!
//! The release record under `.ai/repo/releases/` is evidence of what was *uploaded* —
//! artifacts, digests, sizes — and says nothing about why the release had the version it
//! had. The manifest is the other half: the compatibility the release carried, the
//! contract it was measured against, the changes it published, the documents a reader
//! needs. Together they are enough to explain a release to somebody who was not there, and
//! to prove that the tag, the version, the artifacts and the runtime all describe the same
//! thing.
//!
//! Generated from the release state and committed, like every other projection. Nothing
//! here is authored, and nothing here restates a fact the release record already carries
//! except the version and the tag, which are the join.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::changelog::Changelog;
use super::diff::CompatibilityImpact;
use super::version::{Bump, Version};
use super::{Baseline, Engine};

/// The schema identifier this document is written under.
pub const SCHEMA: &str = "release-manifest/v1";

/// What a release was measured against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseManifestContract")]
pub struct ManifestContract {
    /// The baseline release's tag, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_tag: Option<String>,
    /// The fingerprint of the contract at the baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_fingerprint: Option<String>,
    /// The fingerprint of the contract this release publishes.
    pub fingerprint: String,
    /// How many contract changes the diff found.
    pub changes: usize,
}

/// One release, explained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseManifest")]
pub struct ReleaseManifest {
    /// The document's schema.
    pub schema: String,
    /// The version.
    pub version: Version,
    /// The tag: `v` and the version.
    pub tag: String,
    /// The version this one succeeds, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<Version>,
    /// The commit it was built from, when a release record says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// When it was published, UTC, from the release record. Absent when it has not been.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// What the release costs a caller.
    pub compatibility: CompatibilityImpact,
    /// The bump that compatibility required.
    pub required_bump: Bump,
    /// The lowest version it could have declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_version: Option<Version>,
    /// What it was measured against.
    pub contract: ManifestContract,
    /// The identities of the change records it published, sorted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<String>,
    /// The migration documents its breaking changes name, sorted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub documentation: Vec<String>,
    /// The artifacts it published, by name, from the release record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ManifestArtifact>,
}

/// One artifact, as the manifest names it.
///
/// A narrowing of the release record's own artifact: the name and the digest, which is
/// what proves an artifact is the one this release published. The URL and the size stay in
/// the release record, which is what the installer reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseManifestArtifact")]
pub struct ManifestArtifact {
    /// The target's id in the distribution model.
    pub target: String,
    /// The archive's file name.
    pub name: String,
    /// Its SHA-256.
    pub sha256: String,
}

impl ReleaseManifest {
    /// Build the manifest for one version from the release state.
    ///
    /// Deterministic given the same repository state: every value comes from a record, a
    /// snapshot or the policy, and nothing is read from the clock. The published instant is
    /// the release record's, written when the release actually went out.
    pub fn build(engine: &Engine, version: &Version, changelog: &Changelog) -> Self {
        let tag = version.tag();
        let record = engine.releases.by_tag(&tag);
        let section = changelog.release(version);
        let compatibility = section
            .map(|s| s.impact)
            .or_else(|| engine.impact())
            .unwrap_or(CompatibilityImpact::None);
        let previous_version = engine.baseline.version().cloned().filter(|v| v < version);
        let required_bump = previous_version
            .as_ref()
            .map(|p| super::diff::required_bump(compatibility, p))
            .unwrap_or(Bump::None);
        let minimum_version = previous_version
            .as_ref()
            .map(|p| super::diff::minimum_version(compatibility, p));
        let mut changes: Vec<String> = section
            .map(|s| {
                s.groups
                    .iter()
                    .flat_map(|g| g.changes.iter().map(|c| c.id.clone()))
                    .collect()
            })
            .unwrap_or_default();
        changes.sort();
        let documentation = section.map(|s| s.migrations.clone()).unwrap_or_default();
        let artifacts = record
            .map(|r| {
                r.artifacts
                    .iter()
                    .map(|a| ManifestArtifact {
                        target: a.target.clone(),
                        name: a.name.clone(),
                        sha256: a.sha256.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            schema: SCHEMA.into(),
            version: version.clone(),
            tag,
            previous_version,
            commit: record.map(|r| r.commit.clone()),
            published_at: record.map(|r| r.published_at.clone()),
            compatibility,
            required_bump,
            minimum_version,
            contract: ManifestContract {
                baseline_tag: engine.baseline.tag().map(str::to_string),
                baseline_fingerprint: match &engine.baseline {
                    Baseline::Resolved { fingerprint, .. } => Some(fingerprint.clone()),
                    _ => None,
                },
                fingerprint: engine.current.fingerprint.clone(),
                changes: engine.diff.as_ref().map(|d| d.changes.len()).unwrap_or(0),
            },
            changes,
            documentation,
            artifacts,
        }
    }

    /// Every way the manifest disagrees with the release it describes.
    ///
    /// The identity check the whole subsystem exists to make: the tag, the version, the
    /// release record and the artifacts must describe one release. A mismatch here is a
    /// release whose identity cannot be proved, and no such release may be published.
    pub fn findings(&self, engine: &Engine) -> Vec<String> {
        let mut out = Vec::new();
        if self.tag != self.version.tag() {
            out.push(format!(
                "the manifest's tag is `{}` and its version is `{}`: a tag is `v` followed by its version",
                self.tag, self.version
            ));
        }
        if let Some(record) = engine.releases.by_tag(&self.tag) {
            if record.version != self.version.to_string() {
                out.push(format!(
                    "the release record for {} states version `{}` and the manifest states `{}`",
                    self.tag, record.version, self.version
                ));
            }
            if self.artifacts.len() != record.artifacts.len() {
                out.push(format!(
                    "the release record for {} publishes {} artifact(s) and the manifest names {}",
                    self.tag,
                    record.artifacts.len(),
                    self.artifacts.len()
                ));
            }
        }
        if let Some(minimum) = &self.minimum_version {
            if self.version < *minimum {
                out.push(format!(
                    "{} carries a {} change, which requires at least {minimum}",
                    self.version, self.compatibility
                ));
            }
        }
        if self.compatibility == CompatibilityImpact::Breaking && self.documentation.is_empty() {
            out.push(format!(
                "{} is a breaking release and names no migration document",
                self.version
            ));
        }
        out
    }

    /// The document as it is committed: pretty JSON with one trailing newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribution::release::Releases;
    use crate::release::change::{Change, ChangeType, Changes};
    use crate::release::contract::ContractSnapshot;

    fn engine(changes: Vec<Change>, baseline: Baseline) -> Engine {
        Engine {
            source: "0.4.0".parse().unwrap(),
            current: ContractSnapshot::from_entries(Vec::new()),
            baseline_snapshot: None,
            baseline,
            releases: Releases {
                releases: Vec::new(),
            },
            changes: Changes {
                changes,
                unreadable: Vec::new(),
            },
            diff: None,
            distribution: None,
            snapshot_error: None,
            stale: None,
            layer_degraded: false,
        }
    }

    fn change(id: &str, impact: CompatibilityImpact, migration: Option<&str>) -> Change {
        Change {
            id: id.into(),
            title: id.into(),
            change_type: ChangeType::Changed,
            impact,
            released_in: Some("0.4.0".parse().unwrap()),
            scopes: Vec::new(),
            contract: Vec::new(),
            issues: Vec::new(),
            pull_requests: Vec::new(),
            commits: Vec::new(),
            adrs: Vec::new(),
            migration: migration.map(str::to_string),
            summary: String::new(),
            path: format!(".ai/repo/changes/{id}.md"),
        }
    }

    fn baseline() -> Baseline {
        Baseline::Resolved {
            version: "0.3.1".parse().unwrap(),
            tag: "v0.3.1".into(),
            commit: "a".repeat(40),
            fingerprint: "b".repeat(64),
        }
    }

    #[test]
    fn the_manifest_carries_the_release_it_succeeds_and_the_contract_it_was_measured_against() {
        let e = engine(
            vec![change("x", CompatibilityImpact::Additive, None)],
            baseline(),
        );
        let log = e.changelog();
        let m = ReleaseManifest::build(&e, &"0.4.0".parse().unwrap(), &log);
        assert_eq!(m.tag, "v0.4.0");
        assert_eq!(m.previous_version.as_ref().unwrap().to_string(), "0.3.1");
        assert_eq!(m.contract.baseline_tag.as_deref(), Some("v0.3.1"));
        assert_eq!(m.changes, ["x"]);
        assert!(m.findings(&e).is_empty(), "{:?}", m.findings(&e));
    }

    #[test]
    fn a_breaking_release_with_no_migration_document_is_a_finding() {
        let e = engine(
            vec![change("x", CompatibilityImpact::Breaking, None)],
            baseline(),
        );
        let log = e.changelog();
        let m = ReleaseManifest::build(&e, &"0.4.0".parse().unwrap(), &log);
        assert_eq!(m.compatibility, CompatibilityImpact::Breaking);
        let findings = m.findings(&e);
        assert!(
            findings
                .iter()
                .any(|f| f.contains("names no migration document")),
            "{findings:?}"
        );
    }

    #[test]
    fn a_version_below_its_own_minimum_is_a_finding() {
        let e = engine(
            vec![{
                let mut c = change("x", CompatibilityImpact::Breaking, Some("docs/m.md"));
                c.released_in = Some("0.3.2".parse().unwrap());
                c
            }],
            baseline(),
        );
        let log = e.changelog();
        let m = ReleaseManifest::build(&e, &"0.3.2".parse().unwrap(), &log);
        // below 1.0.0 a break requires the minor: 0.4.0, and 0.3.2 does not clear it.
        assert_eq!(m.minimum_version.as_ref().unwrap().to_string(), "0.4.0");
        let findings = m.findings(&e);
        assert!(
            findings
                .iter()
                .any(|f| f.contains("requires at least 0.4.0")),
            "{findings:?}"
        );
    }

    #[test]
    fn the_document_is_byte_identical_for_the_same_state() {
        let build = || {
            let e = engine(
                vec![change("x", CompatibilityImpact::Additive, None)],
                baseline(),
            );
            let log = e.changelog();
            ReleaseManifest::build(&e, &"0.4.0".parse().unwrap(), &log).to_json()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn a_first_release_has_no_previous_version_and_requires_no_bump() {
        let e = engine(
            vec![change("x", CompatibilityImpact::Additive, None)],
            Baseline::Initial,
        );
        let log = e.changelog();
        let m = ReleaseManifest::build(&e, &"0.4.0".parse().unwrap(), &log);
        assert_eq!(m.previous_version, None);
        assert_eq!(m.required_bump, Bump::None);
        assert_eq!(m.minimum_version, None);
        assert!(m.findings(&e).is_empty());
    }
}
