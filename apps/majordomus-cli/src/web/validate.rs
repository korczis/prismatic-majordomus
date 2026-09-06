//! Validation: the invariants a topology must satisfy before anything serves or publishes it.
//!
//! A collision here is an error, never an ordering accident: two surfaces claiming one path
//! is refused at the point it is discovered rather than resolved by whichever router
//! happened to insert its route first. Every finding names the surface, the value, where the
//! value came from and what to do about it, because a convention-heavy system that reports
//! "invalid topology" is a system nobody can maintain.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::discover::GENERATED_ROOT;
use super::model::{Availability, SurfaceKind, Topology};

/// How much a finding matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// The topology may not be served or published in this state.
    Error,
    /// Worth reading; serving is still coherent.
    Warning,
}

/// One thing wrong with a topology, said so a person can fix it without reading this file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// How much it matters.
    pub severity: Severity,
    /// The rule that produced it, stable enough to grep for.
    pub rule: String,
    /// The surface it is about.
    pub surface: String,
    /// What is wrong.
    pub message: String,
    /// What to do about it.
    pub remedy: String,
}

/// Every invariant, over one topology.
///
/// `artifacts` says whether a static surface's directory must already exist: a validation
/// before a build has nothing to find, and one before serving or publishing has everything
/// to lose by finding nothing.
///
/// ```
/// use majordomus_cli::web::{model::*, validate};
/// # use std::collections::BTreeMap;
/// # fn s(id: &str, mount: &str) -> Surface {
/// #     Surface { id: id.into(), title: id.into(), category: Category::Report,
/// #         visibility: Visibility::Public, kind: SurfaceKind::StaticDirectory,
/// #         mount: Mount::parse(mount).unwrap(), producer: "t".into(), feature: None,
/// #         artifact: Some("target/web/x".into()), index: Some("index.html".into()),
/// #         availability: Availability::Both, built_from: None, provenance: BTreeMap::new() }
/// # }
/// let clash = Topology::new(vec![s("a", "/tests"), s("b", "/tests")]);
/// let findings = validate::validate(&clash, std::path::Path::new("."), validate::Artifacts::Ignore);
/// assert!(findings.iter().any(|f| f.rule == "surface.mount-collision"));
/// ```
pub fn validate(topology: &Topology, root: &Path, artifacts: Artifacts) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(identities(topology));
    findings.extend(mounts(topology));
    findings.extend(roots(topology, root, artifacts));
    findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.rule.cmp(&b.rule))
            .then_with(|| a.surface.cmp(&b.surface))
    });
    findings
}

/// Whether a static surface's generated directory must exist for the topology to be valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Artifacts {
    /// The directory must be there, with its index: what serving and publishing need.
    Required,
    /// The directory may be missing: what a validation before the build can say.
    Ignore,
}

/// Two surfaces may not share an identity: the id is the selector, and an ambiguous
/// selector silently selects the wrong thing.
fn identities(topology: &Topology) -> Vec<Finding> {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for surface in &topology.surfaces {
        *seen.entry(surface.id.as_str()).or_default() += 1;
    }
    seen.into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(id, n)| Finding {
            severity: Severity::Error,
            rule: "surface.duplicate-id".into(),
            surface: id.to_string(),
            message: format!("{n} surfaces claim the id '{id}'"),
            remedy: "give each producer its own id; the id is what --only and --exclude select"
                .into(),
        })
        .collect()
}

/// Mount ownership: one owner per path, and nesting only under the root application.
///
/// A surface owns its mount and everything under it. Two surfaces claiming one mount is a
/// collision. A surface mounted inside another's subtree is a shadowing conflict — the
/// outer surface would answer for paths the inner one was built for, or the other way
/// round, and which one wins would depend on the router. The single exception is the root
/// application, which exists precisely to answer what nothing else claims.
fn mounts(topology: &Topology) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (i, outer) in topology.surfaces.iter().enumerate() {
        for inner in topology.surfaces.iter().skip(i + 1) {
            if outer.mount == inner.mount {
                findings.push(Finding {
                    severity: Severity::Error,
                    rule: "surface.mount-collision".into(),
                    surface: format!("{} and {}", outer.id, inner.id),
                    message: format!(
                        "both claim {} ({} from {}, {} from {})",
                        outer.mount,
                        outer.id,
                        provenance_of(outer, "mount"),
                        inner.id,
                        provenance_of(inner, "mount")
                    ),
                    remedy: "one path has one owner: change one producer's declared mount".into(),
                });
                continue;
            }
            let (over, under) = if outer.mount.contains(&inner.mount) {
                (outer, inner)
            } else if inner.mount.contains(&outer.mount) {
                (inner, outer)
            } else {
                continue;
            };
            if over.mount.is_root() {
                continue; // the application answers what nothing else claims: that is its job
            }
            findings.push(Finding {
                severity: Severity::Error,
                rule: "surface.nested-mount".into(),
                surface: format!("{} inside {}", under.id, over.id),
                message: format!(
                    "{} is mounted at {} inside {}, which owns {} and everything under it",
                    under.id, under.mount, over.id, over.mount
                ),
                remedy: format!(
                    "move {} outside {}, or let {} produce it as part of its own output",
                    under.id,
                    over.mount.prefix(),
                    over.id
                ),
            });
        }
    }
    findings
}

/// What a static surface's directory must be: inside the generated root or the site's own
/// output, present when it is needed, and carrying the index its mount answers with.
fn roots(topology: &Topology, root: &Path, artifacts: Artifacts) -> Vec<Finding> {
    let mut findings = Vec::new();
    for surface in &topology.surfaces {
        match surface.kind {
            SurfaceKind::StaticDirectory => {}
            SurfaceKind::NativeRoute | SurfaceKind::Redirect => {
                if surface.artifact.is_some() {
                    findings.push(Finding {
                        severity: Severity::Warning,
                        rule: "surface.artifact-on-dynamic".into(),
                        surface: surface.id.clone(),
                        message: "a route the executable answers itself names a directory".into(),
                        remedy: "drop the artifact, or declare the surface static".into(),
                    });
                }
                continue;
            }
        }
        let Some(artifact) = surface.artifact.as_ref() else {
            findings.push(Finding {
                severity: Severity::Error,
                rule: "surface.missing-artifact-path".into(),
                surface: surface.id.clone(),
                message: "a static surface names no directory".into(),
                remedy: "declare the directory the producer writes".into(),
            });
            continue;
        };
        let text = artifact.to_string_lossy().replace('\\', "/");
        if artifact.is_absolute() || text.split('/').any(|s| s == "..") {
            findings.push(Finding {
                severity: Severity::Error,
                rule: "surface.artifact-escapes".into(),
                surface: surface.id.clone(),
                message: format!("the directory '{text}' leaves the repository"),
                remedy: "a surface is served from a repository-relative generated directory".into(),
            });
            continue;
        }
        if !(text.starts_with(GENERATED_ROOT) || text.starts_with("site/")) {
            findings.push(Finding {
                severity: Severity::Error,
                rule: "surface.artifact-outside-generated-root".into(),
                surface: surface.id.clone(),
                message: format!(
                    "the directory '{text}' is neither under {GENERATED_ROOT}/ nor the site's output"
                ),
                remedy: format!(
                    "write the producer's output into {GENERATED_ROOT}/{}/",
                    surface.id
                ),
            });
            continue;
        }
        if artifacts == Artifacts::Ignore {
            continue;
        }
        let dir = root.join(artifact);
        if !dir.is_dir() {
            findings.push(Finding {
                severity: Severity::Error,
                rule: "surface.artifact-absent".into(),
                surface: surface.id.clone(),
                message: format!("'{text}' does not exist"),
                remedy: format!("run the producer: {}", surface.producer),
            });
            continue;
        }
        if let Some(index) = surface.index.as_ref() {
            if !dir.join(index).is_file() && surface.availability != Availability::ServedOnly {
                findings.push(Finding {
                    severity: Severity::Error,
                    rule: "surface.index-absent".into(),
                    surface: surface.id.clone(),
                    message: format!(
                        "'{text}/{index}' does not exist, so {} answers with nothing",
                        surface.mount
                    ),
                    remedy: format!("the producer writes {index}, or declares another index"),
                });
            }
        }
    }
    findings
}

fn provenance_of(surface: &super::model::Surface, field: &str) -> String {
    surface
        .provenance
        .get(field)
        .map(|p| p.to_string())
        .unwrap_or_else(|| "an unrecorded source".into())
}

/// Does this set of findings stop a topology from being served or published?
pub fn blocking(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::model::{Availability, Category, Mount, Surface, Visibility};
    use std::collections::BTreeMap;

    fn surface(id: &str, mount: &str, artifact: Option<&str>) -> Surface {
        Surface {
            id: id.into(),
            title: id.into(),
            category: Category::Report,
            visibility: Visibility::Public,
            kind: if artifact.is_some() {
                SurfaceKind::StaticDirectory
            } else {
                SurfaceKind::NativeRoute
            },
            mount: Mount::parse(mount).unwrap(),
            producer: format!("producer of {id}"),
            feature: None,
            artifact: artifact.map(Into::into),
            index: artifact.map(|_| "index.html".to_string()),
            availability: if artifact.is_some() {
                Availability::Both
            } else {
                Availability::ServedOnly
            },
            built_from: None,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn a_clean_topology_has_nothing_to_say() {
        let t = Topology::new(vec![
            surface("app", "/", Some("site/public")),
            surface("tests", "/tests", Some("target/web/tests")),
            surface("api", "/api/v1", None),
        ]);
        let findings = validate(&t, Path::new("/nonexistent"), Artifacts::Ignore);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn the_root_application_may_hold_every_other_surface() {
        let t = Topology::new(vec![
            surface("app", "/", Some("site/public")),
            surface("tests", "/tests", Some("target/web/tests")),
        ]);
        assert!(validate(&t, Path::new("/nonexistent"), Artifacts::Ignore).is_empty());
    }

    #[test]
    fn nesting_under_anything_else_is_refused_with_both_names() {
        let t = Topology::new(vec![
            surface("tests", "/tests", Some("target/web/tests")),
            surface("ui", "/tests/ui", Some("target/web/ui")),
        ]);
        let findings = validate(&t, Path::new("/nonexistent"), Artifacts::Ignore);
        let nested = findings
            .iter()
            .find(|f| f.rule == "surface.nested-mount")
            .expect("nesting is a finding");
        assert!(nested.message.contains("/tests/ui"), "{}", nested.message);
        assert!(nested.remedy.contains("tests"), "{}", nested.remedy);
    }

    #[test]
    fn an_artifact_outside_the_generated_root_is_refused() {
        let t = Topology::new(vec![surface("rogue", "/rogue", Some("docs"))]);
        let findings = validate(&t, Path::new("/nonexistent"), Artifacts::Ignore);
        assert!(findings
            .iter()
            .any(|f| f.rule == "surface.artifact-outside-generated-root"));
    }

    #[test]
    fn an_artifact_that_walks_out_of_the_repository_is_refused() {
        let t = Topology::new(vec![surface(
            "rogue",
            "/rogue",
            Some("target/web/../../etc"),
        )]);
        let findings = validate(&t, Path::new("/nonexistent"), Artifacts::Ignore);
        assert!(findings
            .iter()
            .any(|f| f.rule == "surface.artifact-escapes"));
    }

    #[test]
    fn a_missing_directory_is_a_finding_only_when_the_artifact_is_required() {
        let t = Topology::new(vec![surface("tests", "/tests", Some("target/web/tests"))]);
        assert!(validate(&t, Path::new("/nonexistent"), Artifacts::Ignore).is_empty());
        let required = validate(&t, Path::new("/nonexistent"), Artifacts::Required);
        assert!(required.iter().any(|f| f.rule == "surface.artifact-absent"));
        assert!(blocking(&required));
    }

    #[test]
    fn adding_an_unrelated_surface_does_not_disturb_the_others() {
        let before = Topology::new(vec![
            surface("app", "/", Some("site/public")),
            surface("tests", "/tests", Some("target/web/tests")),
        ]);
        let after = Topology::new(vec![
            surface("app", "/", Some("site/public")),
            surface("tests", "/tests", Some("target/web/tests")),
            surface(
                "example-report",
                "/example-report",
                Some("target/web/example-report"),
            ),
        ]);
        assert!(validate(&after, Path::new("/nonexistent"), Artifacts::Ignore).is_empty());
        assert_eq!(before.owner("/tests/x").unwrap().id, "tests");
        assert_eq!(after.owner("/tests/x").unwrap().id, "tests");
        assert_eq!(
            after.owner("/example-report/i").unwrap().id,
            "example-report"
        );
    }
}
