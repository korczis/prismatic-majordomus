//! The version a landing allocates (ADR 0134): what the integration executor raises the
//! trunk's version to when it lands a pull request, decided from three things and nothing
//! else — the version the trunk declares, how far the pull request moves the public contract
//! (ADR 0051's measurement), and the repository's typed `release:` policy.
//!
//! ```text
//! trunk version ─┐
//! observed impact ─ compat::Policy::required_of ─ required ─┐
//! policy floor ── (an exemption lowers only this) ── floor ─┴─ max ─ selected ─ next
//! ```
//!
//! The decision is a pure function, so the executor, `majordomus release next`, the
//! Cockpit and a test all reach the same answer from the same inputs. Nothing here reads a
//! branch's own version: the number is always the trunk's raised, which is what makes a
//! retried or repeated landing allocate once and two pull requests unable to hold the same
//! number.
//!
//! ```
//! use majordomus_cli::release::compat::Impact;
//! use majordomus_cli::release::landing::{decide, Allocation, ReleasePolicy};
//! use majordomus_cli::release::version::Version;
//!
//! let policy = ReleasePolicy { allocation: Allocation::Landing, floor: Impact::Minor, exempt: vec![] };
//! // a fix behind the boundary still advances a minor under a minor floor
//! let d = decide(Version::parse("0.20.0").unwrap(), Impact::Patch, &policy, &["src/a.rs".into()]);
//! assert_eq!(d.next_version, "0.21.0");
//! assert_eq!(d.selected, Impact::Minor);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::discovery::glob::Glob;
use crate::release::compat::{Impact, Mode, Policy as CompatPolicy};
use crate::release::version::Version;

/// The schema a [`LandingDecision`] carries, so a reader knows which rules reached it.
pub const DECISION_SCHEMA: &str = "majordomus/landing-decision/v1";

/// Whether the integration executor allocates the version when it lands a pull request.
///
/// ```
/// use majordomus_cli::release::landing::Allocation;
/// // a policy that says nothing changes nothing: a pull request raises its own version
/// assert_eq!(Allocation::default(), Allocation::Off);
/// let a: Allocation = serde_json::from_str("\"landing\"").unwrap();
/// assert_eq!(a.as_str(), "landing");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseAllocation")]
pub enum Allocation {
    /// The version is raised by whoever changes the contract, as before ADR 0134.
    #[default]
    Off,
    /// The integration executor raises it at landing; a pull request does not.
    Landing,
}

impl Allocation {
    /// The word the policy file and every surface use.
    ///
    /// ```
    /// use majordomus_cli::release::landing::Allocation;
    /// assert_eq!(Allocation::Off.as_str(), "off");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Allocation::Off => "off",
            Allocation::Landing => "landing",
        }
    }
}

/// One exemption class of `release.exempt`: a pull request whose every changed path one
/// class covers is not held to the floor. The compatibility requirement still applies.
///
/// ```
/// use majordomus_cli::release::landing::ReleaseExemption;
/// let e = ReleaseExemption {
///     id: "plan-records".into(),
///     description: "plan records".into(),
///     paths: vec![".ai/repo/project/**".into()],
/// };
/// assert!(e.covers(".ai/repo/project/issues/I1.yaml"));
/// assert!(!e.covers("apps/majordomus-cli/src/lib.rs"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExemption {
    /// The class, as a decision names it.
    pub id: String,
    /// What the class is, in a sentence a reviewer reads.
    pub description: String,
    /// Repository-relative globs, in the `:(glob)` subset `discovery::glob` reads.
    pub paths: Vec<String>,
}

impl ReleaseExemption {
    /// Whether this class covers one changed path.
    ///
    /// ```
    /// use majordomus_cli::release::landing::ReleaseExemption;
    /// let e = ReleaseExemption { id: "x".into(), description: "x".into(), paths: vec!["docs/*.md".into()] };
    /// assert!(e.covers("docs/A.md") && !e.covers("docs/sub/A.md"));
    /// ```
    pub fn covers(&self, path: &str) -> bool {
        self.paths.iter().any(|p| Glob::new(p).matches(path))
    }
}

/// The `release:` block of `.ai/repo/policy.yaml` (ADR 0134).
///
/// ```
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::landing::{Allocation, ReleasePolicy};
/// // absent is off with no floor: a repository whose policy predates the block is unchanged
/// let p = ReleasePolicy::default();
/// assert_eq!((p.allocation, p.floor), (Allocation::Off, Impact::None));
/// let p: ReleasePolicy = serde_json::from_str(r#"{"allocation":"landing","floor":"minor"}"#).unwrap();
/// assert_eq!((p.allocation, p.floor), (Allocation::Landing, Impact::Minor));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleasePolicy {
    /// `release.allocation:` — `off` or `landing`.
    #[serde(default)]
    pub allocation: Allocation,
    /// `release.floor:` — the least an eligible integration advances: `none`, `patch`,
    /// `minor` or `major`.
    #[serde(default = "no_floor")]
    pub floor: Impact,
    /// `release.exempt:` — the classes a pull request may fall in instead of the floor.
    #[serde(default)]
    pub exempt: Vec<ReleaseExemption>,
}

fn no_floor() -> Impact {
    Impact::None
}

impl Default for ReleasePolicy {
    /// Off, with no floor and no exemption: what a policy that says nothing means.
    fn default() -> Self {
        ReleasePolicy {
            allocation: Allocation::Off,
            floor: no_floor(),
            exempt: Vec::new(),
        }
    }
}

/// The exemption a decision applied, by class.
///
/// ```
/// use majordomus_cli::release::landing::AppliedExemption;
/// let e = AppliedExemption { id: "plan-records".into(), reason: "plan records".into() };
/// assert_eq!(e.id, "plan-records");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppliedExemption {
    /// The class's id.
    pub id: String,
    /// The class's description, which is the reason the floor did not apply.
    pub reason: String,
}

/// What a landing allocates, and why, step by step.
///
/// ```
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::landing::{decide, LandingDecision, ReleasePolicy, DECISION_SCHEMA};
/// use majordomus_cli::release::version::Version;
/// let d: LandingDecision = decide(Version::parse("1.2.3").unwrap(), Impact::None, &ReleasePolicy::default(), &[]);
/// assert_eq!(d.schema, DECISION_SCHEMA);
/// assert_eq!(d.next_version, "1.2.3", "nothing moved and nothing is owed");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LandingDecision {
    /// [`DECISION_SCHEMA`].
    pub schema: String,
    /// What the trunk declares: the number the next one is raised from.
    pub trunk_version: String,
    /// How far the change moves the public contract, measured (ADR 0051).
    pub observed: Impact,
    /// What that movement requires under the version's mode.
    pub required_by_compatibility: Impact,
    /// The policy's floor, or `none` when an exemption applied.
    pub floor: Impact,
    /// The exemption that lowered the floor, if one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exemption: Option<AppliedExemption>,
    /// The larger of the requirement and the floor.
    pub selected: Impact,
    /// The trunk's version raised by `selected`.
    pub next_version: String,
    /// Whether the policy allocates at landing; the decision is computed either way.
    pub allocation: Allocation,
    /// The compatibility mode the trunk's version puts in force.
    pub mode: Mode,
    /// One sentence per step, in the order the decision was made.
    pub explanation: Vec<String>,
}

/// The exemption class covering every changed path, when one does. An empty change is
/// covered by nothing: a landing that changed nothing is not an exempt landing, it is no
/// landing.
fn exemption_for<'a>(
    policy: &'a ReleasePolicy,
    changed: &[String],
) -> Option<&'a ReleaseExemption> {
    if changed.is_empty() {
        return None;
    }
    policy
        .exempt
        .iter()
        .find(|class| changed.iter().all(|path| class.covers(path)))
}

/// Decide what a landing allocates (ADR 0134 §2-4).
///
/// `selected = max(required_by_compatibility, floor)`, where an exemption covering every
/// changed path sets the floor to `none` and never touches the requirement.
///
/// ```
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::landing::{decide, Allocation, ReleaseExemption, ReleasePolicy};
/// use majordomus_cli::release::version::Version;
///
/// let policy = ReleasePolicy {
///     allocation: Allocation::Landing,
///     floor: Impact::Minor,
///     exempt: vec![ReleaseExemption {
///         id: "plan-records".into(),
///         description: "plan records change no product behaviour".into(),
///         paths: vec![".ai/repo/project/**".into()],
///     }],
/// };
/// let trunk = Version::parse("0.20.0").unwrap();
/// // a plan record alone is exempt from the floor, and so allocates nothing
/// let d = decide(trunk, Impact::None, &policy, &[".ai/repo/project/issues/I9.yaml".into()]);
/// assert_eq!(d.next_version, "0.20.0");
/// assert_eq!(d.exemption.unwrap().id, "plan-records");
/// // a breaking change below 1.0.0 costs a minor, which the floor already asks
/// let d = decide(trunk, Impact::Major, &policy, &["src/a.rs".into()]);
/// assert_eq!(d.next_version, "0.21.0");
/// ```
pub fn decide(
    trunk: Version,
    observed: Impact,
    policy: &ReleasePolicy,
    changed: &[String],
) -> LandingDecision {
    let compat = CompatPolicy::for_version(trunk);
    let required = compat.required_of(observed);
    let exemption = exemption_for(policy, changed);
    let floor = if exemption.is_some() {
        Impact::None
    } else {
        policy.floor
    };
    let selected = required.max(floor);
    let next = trunk.raised_to(selected);

    let mut explanation = vec![
        format!("the trunk declares {trunk}"),
        format!(
            "the change moves the public contract by {}: {}",
            observed.as_str(),
            compat.statement()
        ),
        format!("compatibility therefore requires {}", required.as_str()),
    ];
    match exemption {
        Some(class) => explanation.push(format!(
            "every changed path is in the exemption class {} ({}), so the policy floor of {} does not apply; the compatibility requirement still does",
            class.id,
            class.description,
            policy.floor.as_str()
        )),
        None => explanation.push(format!("the policy floor is {}", policy.floor.as_str())),
    }
    explanation.push(format!(
        "the larger of the two is {}, so the next version is {next}",
        selected.as_str()
    ));
    if policy.allocation == Allocation::Off {
        explanation.push(
            "the policy does not allocate at landing (release.allocation: off): this is what a landing would raise to, and a pull request still raises its own version"
                .to_string(),
        );
    }

    LandingDecision {
        schema: DECISION_SCHEMA.to_string(),
        trunk_version: trunk.to_string(),
        observed,
        required_by_compatibility: required,
        floor,
        exemption: exemption.map(|class| AppliedExemption {
            id: class.id.clone(),
            reason: class.description.clone(),
        }),
        selected,
        next_version: next.to_string(),
        allocation: policy.allocation,
        mode: compat.mode,
        explanation,
    }
}

/// Why a landing decision could not be made: an input that could not be read. Never a
/// decision of its own — an unreadable trunk or surface is refused, not guessed.
///
/// ```
/// use majordomus_cli::release::landing::Unreadable;
/// let e = Unreadable("no ref nonesuch".into());
/// assert_eq!(e.to_string(), "no ref nonesuch");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable(pub String);

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unreadable {}

/// What a landing of `head` onto `base` allocates in the repository at `root`: the trunk's
/// version is the manifest at `base`, the movement is `base`'s committed registry against
/// `head`'s, and the changed paths are git's between the two. Everything is read from git,
/// so the answer is the same in any checkout and does not depend on which executable runs.
///
/// ```
/// use majordomus_cli::release::landing::{next_for, ReleasePolicy};
/// // a directory that is no repository has no trunk to raise from
/// let err = next_for(std::path::Path::new("/"), "origin/master", "HEAD", &ReleasePolicy::default());
/// assert!(err.is_err());
/// ```
pub fn next_for(
    root: &std::path::Path,
    base: &str,
    head: &str,
    policy: &ReleasePolicy,
) -> Result<LandingDecision, Unreadable> {
    use crate::release::surface::Surface;
    let git = |args: &[&str]| -> Result<String, Unreadable> {
        let out = std::process::Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .map_err(|e| Unreadable(format!("git could not run: {e}")))?;
        if !out.status.success() {
            return Err(Unreadable(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let manifest = git(&[
        "show",
        &format!("{base}:{}", crate::release::version::MANIFEST),
    ])?;
    let trunk = crate::release::version::declared_in(&manifest)
        .and_then(|v| Version::parse(&v))
        .ok_or_else(|| {
            Unreadable(format!(
                "{base}:{} declares no version of three numbers",
                crate::release::version::MANIFEST
            ))
        })?;
    let before = Surface::read_ref(root, base).map_err(|e| Unreadable(e.to_string()))?;
    let after = Surface::read_ref(root, head).map_err(|e| Unreadable(e.to_string()))?;
    let observed = crate::release::compat::diff(&before, &after)
        .iter()
        .map(|c| c.impact)
        .max()
        .unwrap_or(Impact::None);
    let changed: Vec<String> = git(&["diff", "--name-only", &format!("{base}...{head}")])?
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    Ok(decide(trunk, observed, policy, &changed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).expect("a version")
    }

    fn landing(floor: Impact) -> ReleasePolicy {
        ReleasePolicy {
            allocation: Allocation::Landing,
            floor,
            exempt: vec![
                ReleaseExemption {
                    id: "plan-records".into(),
                    description: "plan records change no product behaviour".into(),
                    paths: vec![".ai/repo/project/**".into(), ".ai/repo/sessions/**".into()],
                },
                ReleaseExemption {
                    id: "release-record".into(),
                    description: "the record a release writes of itself".into(),
                    paths: vec![
                        ".ai/repo/releases/**".into(),
                        "site/static/releases/**".into(),
                    ],
                },
            ],
        }
    }

    fn src() -> Vec<String> {
        vec!["apps/majordomus-cli/src/lib.rs".into()]
    }

    #[test]
    fn the_selected_increment_is_the_larger_of_compatibility_and_the_floor() {
        let p = landing(Impact::Minor);
        for (observed, trunk, next) in [
            (Impact::None, "0.20.0", "0.21.0"),
            (Impact::Patch, "0.20.0", "0.21.0"),
            (Impact::Minor, "0.20.0", "0.21.0"),
            (Impact::Major, "0.20.0", "0.21.0"),
            (Impact::Patch, "1.4.2", "1.5.0"),
            (Impact::Major, "1.4.2", "2.0.0"),
        ] {
            let d = decide(v(trunk), observed, &p, &src());
            assert_eq!(
                d.next_version, next,
                "{observed:?} on {trunk}: {:#?}",
                d.explanation
            );
        }
    }

    #[test]
    fn below_one_a_breaking_change_costs_a_minor_and_from_one_a_major() {
        let p = landing(Impact::None);
        assert_eq!(
            decide(v("0.9.0"), Impact::Major, &p, &src()).selected,
            Impact::Minor
        );
        assert_eq!(
            decide(v("0.9.0"), Impact::Major, &p, &src()).mode,
            Mode::Pre1_0Strict
        );
        assert_eq!(
            decide(v("1.0.0"), Impact::Major, &p, &src()).selected,
            Impact::Major
        );
        assert_eq!(
            decide(v("1.0.0"), Impact::Major, &p, &src()).mode,
            Mode::Semver
        );
    }

    #[test]
    fn with_no_floor_a_change_behind_the_boundary_advances_nothing() {
        let d = decide(v("0.20.0"), Impact::Patch, &landing(Impact::None), &src());
        assert_eq!(
            (d.selected, d.next_version.as_str()),
            (Impact::None, "0.20.0")
        );
    }

    #[test]
    fn an_exemption_lowers_the_floor_and_never_the_compatibility_requirement() {
        let p = landing(Impact::Minor);
        let plan = vec![".ai/repo/project/issues/I9.yaml".to_string()];
        let d = decide(v("1.0.0"), Impact::None, &p, &plan);
        assert_eq!(
            d.exemption.as_ref().map(|e| e.id.as_str()),
            Some("plan-records")
        );
        assert_eq!((d.floor, d.next_version.as_str()), (Impact::None, "1.0.0"));
        // the same paths, had they moved the contract, still owe what the movement owes
        let d = decide(v("1.0.0"), Impact::Major, &p, &plan);
        assert_eq!(
            (d.selected, d.next_version.as_str()),
            (Impact::Major, "2.0.0")
        );
    }

    #[test]
    fn a_change_one_class_covers_only_in_part_is_not_exempt() {
        let p = landing(Impact::Minor);
        let mixed = vec![
            ".ai/repo/project/issues/I9.yaml".to_string(),
            "apps/majordomus-cli/src/lib.rs".to_string(),
        ];
        let d = decide(v("0.20.0"), Impact::None, &p, &mixed);
        assert!(d.exemption.is_none());
        assert_eq!(d.next_version, "0.21.0");
        // nor is a change two classes cover between them: one class must cover it all
        let split = vec![
            ".ai/repo/project/issues/I9.yaml".to_string(),
            ".ai/repo/releases/v0.20.0.yaml".to_string(),
        ];
        assert!(decide(v("0.20.0"), Impact::None, &p, &split)
            .exemption
            .is_none());
    }

    #[test]
    fn an_empty_change_is_covered_by_no_class() {
        let d = decide(v("0.20.0"), Impact::None, &landing(Impact::Minor), &[]);
        assert!(d.exemption.is_none());
        assert_eq!(d.next_version, "0.21.0");
    }

    #[test]
    fn the_same_inputs_give_the_same_decision_and_explanation() {
        let p = landing(Impact::Minor);
        let a = decide(v("0.20.0"), Impact::Minor, &p, &src());
        let b = decide(v("0.20.0"), Impact::Minor, &p, &src());
        assert_eq!(a, b);
        assert!(a.explanation[0].contains("0.20.0"));
        assert!(a
            .explanation
            .iter()
            .any(|line| line.contains("floor is minor")));
    }

    #[test]
    fn a_policy_that_does_not_allocate_says_so_and_still_decides() {
        let p = ReleasePolicy {
            allocation: Allocation::Off,
            ..landing(Impact::Minor)
        };
        let d = decide(v("0.20.0"), Impact::Minor, &p, &src());
        assert_eq!(d.next_version, "0.21.0");
        assert!(d
            .explanation
            .last()
            .unwrap()
            .contains("release.allocation: off"));
    }

    #[test]
    fn the_default_policy_changes_nothing() {
        let d = decide(
            v("0.20.0"),
            Impact::Patch,
            &ReleasePolicy::default(),
            &src(),
        );
        assert_eq!(
            (d.allocation, d.floor, d.next_version.as_str()),
            (Allocation::Off, Impact::None, "0.20.0")
        );
    }

    #[test]
    fn the_policy_block_reads_from_the_shape_the_policy_file_has() {
        let p: ReleasePolicy = serde_json::from_value(serde_json::json!({
            "allocation": "landing",
            "floor": "minor",
            "exempt": [{"id": "plan-records", "description": "plans", "paths": [".ai/repo/project/**"]}],
        }))
        .expect("the block");
        assert_eq!(
            p,
            ReleasePolicy {
                exempt: p.exempt.clone(),
                ..landing(Impact::Minor)
            }
        );
        assert!(
            serde_json::from_value::<ReleasePolicy>(serde_json::json!({"skip": true})).is_err(),
            "there is no switch outside the declared keys"
        );
    }
}
