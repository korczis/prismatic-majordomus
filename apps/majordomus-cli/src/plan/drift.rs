//! Where an issue's declared status contradicts its delivery: the drift between the stamps
//! and the work, and on the trunk, past a window, a gate (ADR 0166, decision 2).
//!
//! [`super::delivery`] says how far each issue's work has travelled; the plan's stamps say
//! what was accepted. Inference never writes a stamp — a merge is not acceptance — so the two
//! can disagree, and this module names every disagreement with the object that proves it:
//!
//! | class                 | declared                 | delivered                                                     |
//! |-----------------------|--------------------------|---------------------------------------------------------------|
//! | `merged_not_recorded` | READY, BLOCKED or ACTIVE | a merge names it, and no open pull request names it           |
//! | `active_without_work` | ACTIVE, started before the window | no branch, no pull request, no merge and no mesh claim names it |
//! | `done_without_trace`  | DONE                     | no merge names it (advisory: record-only issues exist)        |
//!
//! # The window and the gate
//!
//! `.ai/repo/ci/plan.yaml` declares `drift.window`, the seconds the trunk may carry merged
//! work the plan has not recorded, and `drift.enforcement`, `advisory` or `blocking`. A gate
//! is asked `on` a head or `on` the trunk:
//!
//! - on a head every `merged_not_recorded` is PENDING: the head is not the shared plan;
//! - on the trunk one merged longer than the window ago is REFUSED, naming
//!   `majordomus plan verify|done <id>`, and a younger one is PENDING;
//! - `active_without_work` is reported and `done_without_trace` is advisory: neither refuses;
//! - a window that is missing or unreadable refuses the gate (exit 12): no merge can be
//!   called young without one;
//! - under `advisory` enforcement the refusals are reported and the gate exits 0, which is
//!   how the gate lands before the existing drift is settled (I2748 makes it blocking).
//!
//! A finding whose open pull request could not be read (the forge unread) is still a
//! finding: it says so in `unread`, and it is not `settled`.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub(crate) use crate::capability::builtin::plan::DriftGate;

use super::delivery::{
    DeliveryObservation, DeliveryReading, DeliverySource, DeliveryStage, IssueDelivery,
    IssueDeliveryReport,
};

/// The version of the report's shape.
pub(crate) const DRIFT_SCHEMA: u32 = 1;

/// Where the window is declared.
pub(crate) const MODEL: &str = ".ai/repo/ci/plan.yaml";

/// Exit code of a gate that refuses an enforced finding.
pub(crate) const EXIT_REFUSED: u8 = 10;

/// Exit code of a gate that cannot judge: no window it can read.
pub(crate) const EXIT_UNJUDGED: u8 = 12;

/// One kind of contradiction between the stamps and the work.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DriftClass {
    /// READY, BLOCKED or ACTIVE, with merged work and no open pull request naming it.
    MergedNotRecorded,
    /// ACTIVE since before the window, with no branch, pull request, merge or mesh claim.
    ActiveWithoutWork,
    /// DONE with no merge naming it. Advisory: record-only issues exist.
    DoneWithoutTrace,
}

impl DriftClass {
    /// Every class, in order.
    pub(crate) const ALL: [DriftClass; 3] = [
        DriftClass::MergedNotRecorded,
        DriftClass::ActiveWithoutWork,
        DriftClass::DoneWithoutTrace,
    ];

    /// The word the report spells it with.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            DriftClass::MergedNotRecorded => "merged_not_recorded",
            DriftClass::ActiveWithoutWork => "active_without_work",
            DriftClass::DoneWithoutTrace => "done_without_trace",
        }
    }
}

/// What a gate made of one finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DriftVerdict {
    /// Refused: merged work the trunk has carried unrecorded past the window.
    Refused,
    /// Reported and not refused: on a head, or inside the window.
    Pending,
    /// Reported; this class never refuses.
    Reported,
    /// Advisory; this class never refuses (`done_without_trace`).
    Advisory,
}

/// The declared window, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DriftWindow {
    /// The file it is declared in.
    pub model: String,
    /// Read, or unread with `detail` saying why.
    pub state: DeliveryReading,
    /// The seconds the trunk may carry merged work the plan has not recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u64>,
    /// `advisory` (refusals are reported, the gate passes) or `blocking`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforcement: Option<String>,
    /// What was read, or why nothing was.
    pub detail: String,
}

/// The objects that prove a finding.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DriftProof {
    /// The pull request the newest merge naming the issue merged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<u64>,
    /// The newest commit in the trunk's history that names the issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_commit: Option<String>,
    /// When that commit was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
    /// How it names the issue (`title_or_branch`, `slug`, `subject`, `forge_title`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    /// The first release that contains it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// When execution was recorded to have begun, for `active_without_work`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
}

/// One contradiction between an issue's stamps and its delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DriftFinding {
    /// The issue id.
    pub issue: String,
    /// Its title.
    pub title: String,
    /// Its milestone.
    pub milestone: String,
    /// Its declared status.
    pub status: String,
    /// Its inferred delivery stage.
    pub stage: DeliveryStage,
    /// The contradiction.
    pub class: DriftClass,
    /// True for a class that never refuses.
    pub advisory: bool,
    /// What proves it.
    pub proof: DriftProof,
    /// Since when the contradiction holds: the merge for `merged_not_recorded`, the start
    /// stamp for `active_without_work`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Seconds from `since` to the observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<u64>,
    /// Whether that is longer than the window; absent when either is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub past_window: Option<bool>,
    /// True when no unread source could have dissolved it.
    pub settled: bool,
    /// The sources that were not read and could have dissolved it.
    pub unread: Vec<String>,
    /// The command that settles it.
    pub remedy: String,
    /// What the gate made of it, when a gate was asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<DriftVerdict>,
}

/// Every contradiction between the plan's stamps and its delivery, and, when a gate was
/// asked, its verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DriftReport {
    /// The version of this shape.
    pub schema: u32,
    /// Where and when the delivery was observed.
    pub observed: DeliveryObservation,
    /// The declared window.
    pub window: DriftWindow,
    /// The tree a gate judged, when one was asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<DriftGate>,
    /// Every source, read or not: the delivery's, and the mesh claims.
    pub sources: Vec<DeliverySource>,
    /// The class vocabulary, in order.
    pub classes: Vec<DriftClass>,
    /// How many findings of each class, keyed by every class.
    pub tallies: BTreeMap<String, usize>,
    /// The findings, in issue order, then class order.
    pub findings: Vec<DriftFinding>,
    /// How many findings the gate refused.
    pub refused: usize,
    /// How many it reported as pending.
    pub pending: usize,
    /// Whether a refusal fails the gate (`drift.enforcement: blocking`).
    pub enforced: bool,
    /// `report` (no gate), `clean`, `pending`, `advisory` (refusals not enforced),
    /// `refused`, or `unjudged` (no window).
    pub verdict: String,
    /// The exit code the gate ends with: 0, [`EXIT_REFUSED`] or [`EXIT_UNJUDGED`].
    pub exit_code: u8,
}

/// The window declared in `text`, the content of [`MODEL`].
pub(crate) fn window_of(text: &str) -> Result<(u64, String), String> {
    let map = crate::metadata::yaml::parse_mapping(text)?;
    let drift = map
        .get("drift")
        .ok_or_else(|| "it declares no `drift` block".to_string())?;
    let seconds = drift
        .get("window")
        .and_then(serde_json::Value::as_u64)
        .filter(|s| *s > 0)
        .ok_or_else(|| "`drift.window` is not a whole number of seconds above zero".to_string())?;
    let enforcement = drift
        .get("enforcement")
        .and_then(serde_json::Value::as_str)
        .filter(|e| matches!(*e, "advisory" | "blocking"))
        .ok_or_else(|| "`drift.enforcement` is neither `advisory` nor `blocking`".to_string())?;
    Ok((seconds, enforcement.to_string()))
}

/// The window declared in the repository at `root`.
pub(crate) fn read_window(root: &Path) -> DriftWindow {
    let read = std::fs::read_to_string(root.join(MODEL))
        .map_err(|e| format!("{MODEL} cannot be read: {e}"))
        .and_then(|t| window_of(&t).map_err(|e| format!("{MODEL}: {e}")));
    match read {
        Ok((seconds, enforcement)) => DriftWindow {
            model: MODEL.into(),
            state: DeliveryReading::Read,
            detail: format!("drift.window {seconds}s, enforcement {enforcement}"),
            seconds: Some(seconds),
            enforcement: Some(enforcement),
        },
        Err(detail) => DriftWindow {
            model: MODEL.into(),
            state: DeliveryReading::Unread,
            seconds: None,
            enforcement: None,
            detail,
        },
    }
}

/// Seconds since the epoch of a stamp: RFC 3339 to the second, or a bare date.
fn seconds_of(stamp: &str) -> Option<u64> {
    crate::peers::parse_rfc3339(stamp)
        .or_else(|| crate::peers::parse_rfc3339(&format!("{stamp}T00:00:00Z")))
}

/// How a finding is settled.
fn remedy(class: DriftClass, d: &IssueDelivery) -> String {
    let id = &d.issue;
    match class {
        DriftClass::MergedNotRecorded if d.status == "ACTIVE" => {
            format!("majordomus plan verify|done {id}")
        }
        DriftClass::MergedNotRecorded => {
            format!("majordomus plan start {id}, then majordomus plan verify|done {id}")
        }
        DriftClass::ActiveWithoutWork => format!(
            "open its branch (majordomus worktree create) or claim it on the mesh, or settle it: \
             majordomus plan verify|done {id}"
        ),
        DriftClass::DoneWithoutTrace => {
            format!("nothing if {id} is record-only; else name {id} in its pull request's title")
        }
    }
}

fn is_read(sources: &[DeliverySource], name: &str) -> bool {
    sources
        .iter()
        .any(|s| s.name == name && s.state == DeliveryReading::Read)
}

/// The findings over a delivery report, judged by `gate` when one is asked. Pure.
///
/// `claims` maps an issue to the live mesh claim that names it; `None` when the mesh was not
/// read. `started` maps an issue to its start stamp.
pub(crate) fn judge(
    delivery: IssueDeliveryReport,
    started: &BTreeMap<String, String>,
    claims: Option<&BTreeMap<String, String>>,
    claims_source: DeliverySource,
    window: DriftWindow,
    gate: Option<DriftGate>,
) -> DriftReport {
    let now = seconds_of(&delivery.observed.observed_at);
    let forge_read = is_read(&delivery.sources, "forge");
    let titles_read = is_read(&delivery.sources, "merged_titles");
    let age =
        |since: Option<&str>| -> Option<u64> { Some(now?.saturating_sub(seconds_of(since?)?)) };
    let past = |age: Option<u64>| -> Option<bool> { Some(age? > window.seconds?) };
    let mut findings: Vec<DriftFinding> = Vec::new();
    for d in &delivery.issues {
        let mut found =
            |class: DriftClass, proof: DriftProof, since: Option<String>, unread: Vec<String>| {
                let age_seconds = age(since.as_deref());
                findings.push(DriftFinding {
                    issue: d.issue.clone(),
                    title: d.title.clone(),
                    milestone: d.milestone.clone(),
                    status: d.status.clone(),
                    stage: d.stage,
                    class,
                    advisory: class == DriftClass::DoneWithoutTrace,
                    remedy: remedy(class, d),
                    settled: unread.is_empty(),
                    unread,
                    proof,
                    past_window: past(age_seconds),
                    age_seconds,
                    since,
                    verdict: None,
                });
            };
        let merge_proof = || DriftProof {
            pull_request: d.merged.as_ref().and_then(|m| m.pull_request),
            merge_commit: d.merged.as_ref().map(|m| m.commit.clone()),
            merged_at: d.merged.as_ref().map(|m| m.merged_at.clone()),
            via: d.merged.as_ref().map(|m| m.via.clone()),
            release: d.released.as_ref().map(|r| r.tag.clone()),
            started_at: None,
        };
        match d.status.as_str() {
            "READY" | "BLOCKED" | "ACTIVE" if d.merged.is_some() && d.pr_open.is_none() => {
                let unread = if forge_read {
                    Vec::new()
                } else {
                    vec!["forge".to_string()]
                };
                let since = d.merged.as_ref().map(|m| m.merged_at.clone());
                found(DriftClass::MergedNotRecorded, merge_proof(), since, unread);
            }
            "ACTIVE"
                if d.branch.is_none()
                    && d.pr_open.is_none()
                    && d.merged.is_none()
                    && claims.is_none_or(|c| !c.contains_key(&d.issue)) =>
            {
                let since = started.get(&d.issue).filter(|s| !s.is_empty()).cloned();
                let inside = past(age(since.as_deref())) == Some(false);
                if !inside {
                    let mut unread = d.unread.clone();
                    if claims.is_none() {
                        unread.push("claims".into());
                    }
                    let proof = DriftProof {
                        started_at: since.clone(),
                        ..DriftProof::default()
                    };
                    found(DriftClass::ActiveWithoutWork, proof, since, unread);
                }
            }
            "DONE" if d.merged.is_none() => {
                let unread = if titles_read {
                    Vec::new()
                } else {
                    vec!["merged_titles".to_string()]
                };
                found(
                    DriftClass::DoneWithoutTrace,
                    DriftProof::default(),
                    None,
                    unread,
                );
            }
            _ => {}
        }
    }

    let enforced = window.enforcement.as_deref() == Some("blocking");
    let window_read = window.state == DeliveryReading::Read;
    if let Some(gate) = gate {
        for f in &mut findings {
            f.verdict = Some(match f.class {
                DriftClass::DoneWithoutTrace => DriftVerdict::Advisory,
                DriftClass::ActiveWithoutWork => DriftVerdict::Reported,
                DriftClass::MergedNotRecorded => match (gate, f.past_window) {
                    (DriftGate::Trunk, Some(true)) => DriftVerdict::Refused,
                    _ => DriftVerdict::Pending,
                },
            });
        }
    }
    let count = |v: DriftVerdict| findings.iter().filter(|f| f.verdict == Some(v)).count();
    let (refused, pending) = (count(DriftVerdict::Refused), count(DriftVerdict::Pending));
    let (verdict, exit_code) = match gate {
        None => ("report", 0),
        Some(_) if !window_read => ("unjudged", EXIT_UNJUDGED),
        Some(_) if refused > 0 && enforced => ("refused", EXIT_REFUSED),
        Some(_) if refused > 0 => ("advisory", 0),
        Some(_) if pending > 0 => ("pending", 0),
        Some(_) => ("clean", 0),
    };
    let mut tallies: BTreeMap<String, usize> = DriftClass::ALL
        .iter()
        .map(|c| (c.as_str().to_string(), 0))
        .collect();
    for f in &findings {
        *tallies.entry(f.class.as_str().to_string()).or_default() += 1;
    }
    let mut sources = delivery.sources;
    sources.push(claims_source);
    DriftReport {
        schema: DRIFT_SCHEMA,
        observed: delivery.observed,
        window,
        gate,
        sources,
        classes: DriftClass::ALL.to_vec(),
        tallies,
        findings,
        refused,
        pending,
        enforced,
        verdict: verdict.to_string(),
        exit_code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::delivery::{DeliveryMerge, DeliveryPullRequest};

    fn source(name: &str, read: bool) -> DeliverySource {
        DeliverySource {
            name: name.into(),
            state: if read {
                DeliveryReading::Read
            } else {
                DeliveryReading::Unread
            },
            detail: String::new(),
            observed_at: None,
        }
    }

    fn issue(id: &str, status: &str) -> IssueDelivery {
        IssueDelivery {
            issue: id.into(),
            title: format!("the {id}"),
            milestone: "m".into(),
            status: status.into(),
            stage: DeliveryStage::None,
            settled: true,
            unread: Vec::new(),
            branch: None,
            pr_open: None,
            merged: None,
            released: None,
            deployed: None,
        }
    }

    fn merged(mut d: IssueDelivery, at: &str) -> IssueDelivery {
        d.stage = DeliveryStage::Merged;
        d.merged = Some(DeliveryMerge {
            commit: "c0ffee".into(),
            pull_request: Some(7),
            branch: Some("feature/x".into()),
            title: Some("x".into()),
            via: "title_or_branch".into(),
            merged_at: at.into(),
            merges: 1,
            observed_at: at.into(),
        });
        d
    }

    fn report(issues: Vec<IssueDelivery>, forge: bool) -> IssueDeliveryReport {
        IssueDeliveryReport {
            schema: 1,
            observed: DeliveryObservation {
                mode: "live".into(),
                trunk: "master".into(),
                commit: "abc".into(),
                observed_at: "2026-10-16T12:00:00Z".into(),
            },
            stages: DeliveryStage::ALL.to_vec(),
            sources: vec![source("forge", forge), source("merged_titles", true)],
            tallies: BTreeMap::new(),
            unsettled: 0,
            issues,
        }
    }

    fn window(seconds: u64, enforcement: &str) -> DriftWindow {
        DriftWindow {
            model: MODEL.into(),
            state: DeliveryReading::Read,
            seconds: Some(seconds),
            enforcement: Some(enforcement.into()),
            detail: String::new(),
        }
    }

    fn run(issues: Vec<IssueDelivery>, w: DriftWindow, gate: Option<DriftGate>) -> DriftReport {
        judge(
            report(issues, true),
            &BTreeMap::new(),
            Some(&BTreeMap::new()),
            source("claims", true),
            w,
            gate,
        )
    }

    #[test]
    fn a_window_is_declared_or_the_gate_cannot_judge() {
        assert_eq!(
            window_of("version: 1\ndrift:\n  window: 3600\n  enforcement: advisory\n"),
            Ok((3600, "advisory".to_string()))
        );
        assert!(window_of("version: 1\n")
            .unwrap_err()
            .contains("no `drift`"));
        assert!(window_of("drift:\n  window: 0\n  enforcement: blocking\n").is_err());
        assert!(window_of("drift:\n  window: soon\n  enforcement: blocking\n").is_err());
        assert!(window_of("drift:\n  window: 60\n  enforcement: maybe\n").is_err());
        let dir = tempfile::tempdir().unwrap();
        let w = read_window(dir.path());
        assert_eq!(w.state, DeliveryReading::Unread);
        assert!(w.detail.contains("cannot be read"), "{}", w.detail);
        let r = run(
            vec![merged(issue("I1", "READY"), "2026-10-01T00:00:00Z")],
            w,
            Some(DriftGate::Trunk),
        );
        assert_eq!(
            (r.verdict.as_str(), r.exit_code),
            ("unjudged", EXIT_UNJUDGED)
        );
    }

    #[test]
    fn merged_work_unrecorded_is_refused_on_the_trunk_past_the_window_only() {
        let old = merged(issue("I1", "READY"), "2026-10-15T00:00:00Z");
        let young = merged(issue("I2", "ACTIVE"), "2026-10-16T11:30:00Z");
        let mut followed = merged(issue("I3", "ACTIVE"), "2026-10-01T00:00:00Z");
        followed.pr_open = Some(DeliveryPullRequest {
            number: 9,
            title: "more (I3)".into(),
            head_ref: "fix/i3".into(),
            draft: false,
            via: "title_or_branch".into(),
            observed_at: "t".into(),
        });
        let recorded = merged(issue("I4", "VERIFY"), "2026-10-01T00:00:00Z");
        let issues = vec![old, young, followed, recorded];

        let r = run(
            issues.clone(),
            window(3600, "blocking"),
            Some(DriftGate::Trunk),
        );
        let ids: Vec<&str> = r.findings.iter().map(|f| f.issue.as_str()).collect();
        assert_eq!(
            ids,
            ["I1", "I2"],
            "an open follow-up and a stamp are not drift"
        );
        let f = &r.findings[0];
        assert_eq!(f.class, DriftClass::MergedNotRecorded);
        assert_eq!(
            (f.proof.pull_request, f.proof.merge_commit.as_deref()),
            (Some(7), Some("c0ffee"))
        );
        assert_eq!(f.past_window, Some(true));
        assert_eq!(f.verdict, Some(DriftVerdict::Refused));
        assert!(
            f.remedy.contains("majordomus plan verify|done I1"),
            "{}",
            f.remedy
        );
        assert_eq!(r.findings[1].verdict, Some(DriftVerdict::Pending));
        assert_eq!(r.findings[1].age_seconds, Some(1800));
        assert_eq!((r.verdict.as_str(), r.exit_code), ("refused", EXIT_REFUSED));

        let head = run(
            issues.clone(),
            window(3600, "blocking"),
            Some(DriftGate::Head),
        );
        assert_eq!(
            (head.verdict.as_str(), head.exit_code, head.pending),
            ("pending", 0, 2)
        );

        let advisory = run(
            issues.clone(),
            window(3600, "advisory"),
            Some(DriftGate::Trunk),
        );
        assert_eq!(
            (
                advisory.verdict.as_str(),
                advisory.exit_code,
                advisory.refused
            ),
            ("advisory", 0, 1)
        );

        let wide = run(
            issues,
            window(10 * 86_400, "blocking"),
            Some(DriftGate::Trunk),
        );
        assert_eq!((wide.verdict.as_str(), wide.exit_code), ("pending", 0));
    }

    #[test]
    fn an_unread_forge_leaves_a_finding_unsettled_and_says_so() {
        let r = judge(
            report(
                vec![merged(issue("I1", "BLOCKED"), "2026-10-01T00:00:00Z")],
                false,
            ),
            &BTreeMap::new(),
            None,
            source("claims", false),
            window(60, "blocking"),
            Some(DriftGate::Trunk),
        );
        let f = &r.findings[0];
        assert_eq!(
            (f.settled, f.unread.clone()),
            (false, vec!["forge".to_string()])
        );
        assert_eq!(
            f.verdict,
            Some(DriftVerdict::Refused),
            "unread is not a pass"
        );
        assert!(f.remedy.starts_with("majordomus plan start I1"));
    }

    #[test]
    fn active_without_work_and_done_without_trace_never_refuse() {
        let mut started = BTreeMap::new();
        started.insert("I1".to_string(), "2026-10-01".to_string());
        started.insert("I2".to_string(), "2026-10-16T11:59:00Z".to_string());
        started.insert("I3".to_string(), "2026-10-01T00:00:00Z".to_string());
        let mut claims = BTreeMap::new();
        claims.insert("I3".to_string(), "c-1".to_string());
        let r = judge(
            report(
                vec![
                    issue("I1", "ACTIVE"),
                    issue("I2", "ACTIVE"),
                    issue("I3", "ACTIVE"),
                    issue("I4", "DONE"),
                    merged(issue("I5", "DONE"), "2026-10-01T00:00:00Z"),
                ],
                true,
            ),
            &started,
            Some(&claims),
            source("claims", true),
            window(3600, "blocking"),
            Some(DriftGate::Trunk),
        );
        let got: Vec<(&str, DriftClass, Option<DriftVerdict>)> = r
            .findings
            .iter()
            .map(|f| (f.issue.as_str(), f.class, f.verdict))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "I1",
                    DriftClass::ActiveWithoutWork,
                    Some(DriftVerdict::Reported)
                ),
                (
                    "I4",
                    DriftClass::DoneWithoutTrace,
                    Some(DriftVerdict::Advisory)
                ),
            ],
            "I2 started inside the window, I3 is claimed, I5 is traced"
        );
        assert_eq!(
            r.findings[0].proof.started_at.as_deref(),
            Some("2026-10-01")
        );
        assert!(r.findings[1].advisory);
        assert_eq!((r.verdict.as_str(), r.exit_code), ("clean", 0));
        assert_eq!(r.tallies["active_without_work"], 1);
        assert_eq!(r.tallies["merged_not_recorded"], 0);

        let report_only = run(vec![issue("I4", "DONE")], window(60, "blocking"), None);
        assert_eq!(
            (report_only.verdict.as_str(), report_only.exit_code),
            ("report", 0)
        );
        assert_eq!(report_only.findings[0].verdict, None);
    }
}
