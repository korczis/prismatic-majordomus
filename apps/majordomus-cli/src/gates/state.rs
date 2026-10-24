//! The state of every declared gate, as this checkout's record holds it: which gates the CI
//! model declares, what the last recorded run of each one said, over which tree and at which
//! head — and, said as plainly as a verdict, which gates no run has ever reported.
//!
//! # Why this is not the completion report
//!
//! `gates.completion` asks a narrower question — may *this task* be called finished — and so
//! reads only the active task's own runs over the gates its change set selects. With no task
//! open it attributes nothing, and every gate is `unknown`. A person asking "how did the
//! gates last go?" is asking about the gates, not about whichever task is open, so this
//! reads the newest run of each gate whatever task recorded it.
//!
//! # What it refuses to do
//!
//! It never renders a missing verdict as a passing one. A gate with no recorded run is
//! [`GateStatus::Queued`] and is named in [`GateStateReport::never_reported`]; a run whose
//! inputs have changed since is [`GateStatus::Stale`]; a run whose inputs cannot be hashed
//! here is [`GateStatus::Unknown`]. Only a run over the files as they stand now, exiting 0,
//! is `pass`. And it never reaches the network: what it did not consult — the forge's own
//! CI runs — is listed in [`GateStateReport::unreached`] rather than silently absent.
//!
//! ```
//! use majordomus_cli::gates::GateStatus;
//!
//! // the words this report answers in are the completion report's own
//! assert_eq!(GateStatus::Queued.as_str(), "queued");
//! assert_ne!(GateStatus::Queued, GateStatus::Pass, "never reported is not passed");
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::judge::{GateRun, GateStatus};
use super::model::GateModel;

/// What this capability says it did not consult, every time it answers: the CI runs the
/// forge holds. Reading them needs a network and a token, and `gates.state` is the offline
/// half; the sentence is part of the answer so that an absent forge verdict is never read as
/// a passing one.
pub(crate) const FORGE_UNREACHED: &str =
    "the forge's CI runs (GitHub Actions) were not consulted: \
     this answer is this checkout's own record, read offline, and a verdict that exists only \
     on the forge is not in it";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One declared gate and the standing of its last recorded run.
///
/// ```
/// use majordomus_cli::gates::{GateState, GateStatus};
///
/// let never: GateState = serde_json::from_value(serde_json::json!({
///     "id": "rust-check",
///     "job": "rust",
///     "always": true,
///     "on_demand": false,
///     "status": "queued",
///     "reason": "no run of this gate has ever been recorded in this checkout",
/// }))
/// .unwrap();
/// // no run, no head, and a word that is not `pass`
/// assert!(never.last_run.is_none() && never.on_head.is_none());
/// assert_eq!(never.status, GateStatus::Queued);
/// ```
pub struct GateState {
    /// The gate's identity in the model.
    pub id: String,
    /// The CI job that runs it.
    pub job: String,
    /// Whether every plan selects it.
    pub always: bool,
    /// Whether its runner can be had only on demand, so no routine plan runs it.
    pub on_demand: bool,
    /// What is known about its last run: `pass`, `fail`, `stale`, `queued` (never
    /// reported) or `unknown` (a run exists and cannot be checked against this tree).
    pub status: GateStatus,
    /// Why it is in that status, in words a person can act on.
    pub reason: String,
    /// The run that last reported, whichever task recorded it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<GateRun>,
    /// Whether that run was taken at the commit this checkout is at now. Absent when there
    /// is no run or when the current commit could not be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_head: Option<bool>,
    /// The hash of the gate's inputs as they stand now, when it could be taken.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inputs_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The declared gate set and the standing of each gate's last recorded run.
///
/// `count`, `tallies` and `never_reported` are measured from `gates`, never written down,
/// and `unreached` always names what was not consulted.
///
/// ```
/// use majordomus_cli::gates::GateStateReport;
///
/// let r: GateStateReport = serde_json::from_value(serde_json::json!({
///     "source": ".ai/repo/ci/gates.yaml",
///     "ledger": ".ai/local/state/ledger.jsonl",
///     "count": 0,
///     "tallies": {},
///     "never_reported": [],
///     "gates": [],
///     "unreached": ["the forge's CI runs were not consulted"],
///     "findings": [],
///     "evaluated_at": "2026-10-15T00:00:00Z",
/// }))
/// .unwrap();
/// assert_eq!(r.count, r.gates.len());
/// assert!(r.head.is_none(), "a checkout without git still answers, and says so");
/// assert!(!r.unreached.is_empty());
/// ```
pub struct GateStateReport {
    /// Where the gate set was read from, repository-relative.
    pub source: String,
    /// Where the recorded runs were read from, repository-relative.
    pub ledger: String,
    /// The commit this checkout is at, when git could say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// How many gates the model declares; measured.
    pub count: usize,
    /// How many gates stand in each status, by the status word.
    pub tallies: BTreeMap<String, usize>,
    /// The gates no run has ever reported, in declaration order.
    pub never_reported: Vec<String>,
    /// Every declared gate, in declaration order.
    pub gates: Vec<GateState>,
    /// What this answer did not consult, so its silence about it is not read as a verdict.
    pub unreached: Vec<String>,
    /// What could not be read, and what that means for the answer.
    pub findings: Vec<String>,
    /// When this answer was taken, RFC 3339 UTC.
    pub evaluated_at: String,
}

/// The standing of one gate given its last run and the hash of its inputs now.
fn standing_of(run: Option<&GateRun>, hash_now: Option<&str>) -> (GateStatus, String) {
    let Some(r) = run else {
        return (
            GateStatus::Queued,
            "no run of this gate has ever been recorded in this checkout: never reported, \
             which is not a pass"
                .to_string(),
        );
    };
    let Some(now) = hash_now else {
        return (
            GateStatus::Unknown,
            "the files that select this gate could not be hashed here, so its last run cannot \
             be checked against this tree"
                .to_string(),
        );
    };
    if r.inputs_hash != now {
        return (
            GateStatus::Stale,
            format!(
                "the last run (exit {} at {}) was taken over inputs {} and this tree hashes to \
                 {}; it no longer describes what it proved",
                r.exit,
                short(&r.head),
                short(&r.inputs_hash),
                short(now)
            ),
        );
    }
    if r.exit == 0 {
        (
            GateStatus::Pass,
            format!("exit 0 at {} over inputs {}", short(&r.head), short(now)),
        )
    } else {
        (
            GateStatus::Fail,
            format!(
                "exit {} at {} over inputs {}",
                r.exit,
                short(&r.head),
                short(now)
            ),
        )
    }
}

/// Twelve characters of a hash or a commit, as every gate report here prints them.
fn short(v: &str) -> String {
    if v.is_empty() {
        "(none)".to_string()
    } else {
        v.chars().take(12).collect()
    }
}

/// Every gate of `model` with the standing of its last run in `runs`.
///
/// `hashes` holds, per gate, the hash of its inputs now (`None` when it could not be taken;
/// the empty string for a gate with no inputs, as the ledger records it), and `head` the
/// commit this checkout is at. Everything is passed in so that a test drives the same
/// function the capability does.
pub(crate) fn state(
    model: &GateModel,
    runs: &BTreeMap<String, GateRun>,
    hashes: &BTreeMap<String, Option<String>>,
    head: Option<&str>,
) -> Vec<GateState> {
    model
        .gates
        .iter()
        .map(|g| {
            let run = runs.get(&g.id);
            let hash_now = hashes.get(&g.id).cloned().flatten();
            let (status, reason) = standing_of(run, hash_now.as_deref());
            let on_head = match (run, head) {
                (Some(r), Some(h)) if !r.head.is_empty() && !h.is_empty() => {
                    Some(h.starts_with(&r.head) || r.head.starts_with(h))
                }
                (Some(_), Some(_)) => Some(false),
                _ => None,
            };
            GateState {
                id: g.id.clone(),
                job: g.job.clone(),
                always: g.always,
                on_demand: g.on_demand,
                status,
                reason,
                last_run: run.cloned(),
                on_head,
                inputs_hash: hash_now.unwrap_or_default(),
            }
        })
        .collect()
}

/// Fold the per-gate states into the whole report.
pub(crate) fn report(
    source: &str,
    ledger: &str,
    head: Option<String>,
    gates: Vec<GateState>,
    findings: Vec<String>,
    now: &str,
) -> GateStateReport {
    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    for g in &gates {
        *tallies.entry(g.status.as_str().to_string()).or_default() += 1;
    }
    GateStateReport {
        source: source.to_string(),
        ledger: ledger.to_string(),
        head,
        count: gates.len(),
        tallies,
        never_reported: gates
            .iter()
            .filter(|g| g.last_run.is_none())
            .map(|g| g.id.clone())
            .collect(),
        gates,
        unreached: vec![FORGE_UNREACHED.to_string()],
        findings,
        evaluated_at: now.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::yaml;

    const MODEL: &str = r#"
version: 1
gates:
  - id: build
    job: rust
    runs: cargo build
    summary: the crate builds
  - id: site
    job: site
    runs: scripts/site-build
    summary: the site builds
  - id: lint
    job: structure
    always: true
    runs: scripts/lint
    summary: everything parses
  - id: probe
    job: probe
    runs: scripts/probe
    summary: every route answers
classes:
  - id: rust
    paths: [apps/**]
    gates: [build]
  - id: web
    paths: [site/**]
    gates: [site, probe]
"#;

    fn model() -> GateModel {
        yaml::parse_into(MODEL).expect("the fixture model parses")
    }

    fn run(exit: i64, hash: &str, head: &str) -> GateRun {
        GateRun {
            recorded_at: "2026-10-15T00:00:00Z".into(),
            head: head.into(),
            branch: "feature/x".into(),
            exit,
            command: "the gate".into(),
            inputs_hash: hash.into(),
            session: "s-1".into(),
        }
    }

    fn by_id<'a>(states: &'a [GateState], id: &str) -> &'a GateState {
        states.iter().find(|g| g.id == id).expect("declared")
    }

    #[test]
    fn every_gate_stands_in_exactly_the_word_its_last_run_earns() {
        let m = model();
        let mut runs = BTreeMap::new();
        runs.insert("build".to_string(), run(0, "aaaa", "c0ffee"));
        runs.insert("site".to_string(), run(0, "old", "c0ffee"));
        runs.insert("lint".to_string(), run(3, "", "beef"));
        let hashes: BTreeMap<String, Option<String>> = [
            ("build", Some("aaaa")),
            ("site", Some("new")),
            ("lint", Some("")),
            ("probe", None),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
        .collect();

        let s = state(&m, &runs, &hashes, Some("c0ffee0000"));
        assert_eq!(s.len(), 4, "every declared gate, reported or not");
        assert_eq!(by_id(&s, "build").status, GateStatus::Pass);
        assert_eq!(by_id(&s, "build").on_head, Some(true));
        assert_eq!(by_id(&s, "site").status, GateStatus::Stale);
        assert!(by_id(&s, "site").reason.contains("no longer describes"));
        assert_eq!(by_id(&s, "lint").status, GateStatus::Fail);
        assert_eq!(by_id(&s, "lint").on_head, Some(false));
        let probe = by_id(&s, "probe");
        assert_eq!(
            probe.status,
            GateStatus::Queued,
            "never reported is not a pass"
        );
        assert!(probe.last_run.is_none() && probe.on_head.is_none());

        let r = report(
            ".ai/repo/ci/gates.yaml",
            ".ai/local/state/ledger.jsonl",
            Some("c0ffee0000".into()),
            s,
            Vec::new(),
            "now",
        );
        assert_eq!(r.count, 4);
        assert_eq!(r.never_reported, ["probe"]);
        assert_eq!(r.tallies.get("pass"), Some(&1));
        assert_eq!(r.tallies.get("queued"), Some(&1));
        assert_eq!(r.unreached, [FORGE_UNREACHED]);
    }

    #[test]
    fn a_run_that_cannot_be_checked_against_this_tree_is_unknown_never_a_pass() {
        let m = model();
        let mut runs = BTreeMap::new();
        runs.insert("build".to_string(), run(0, "aaaa", "c0ffee"));
        // no hash for any gate, and no head: a checkout git cannot read
        let s = state(&m, &runs, &BTreeMap::new(), None);
        let build = by_id(&s, "build");
        assert_eq!(build.status, GateStatus::Unknown);
        assert!(build.on_head.is_none(), "no head, so no claim about it");
        assert_eq!(by_id(&s, "site").status, GateStatus::Queued);
    }

    #[test]
    fn a_run_without_a_recorded_head_is_not_on_this_head() {
        let m = model();
        let mut runs = BTreeMap::new();
        runs.insert("build".to_string(), run(0, "aaaa", ""));
        let hashes = [("build".to_string(), Some("aaaa".to_string()))].into();
        let s = state(&m, &runs, &hashes, Some("c0ffee"));
        assert_eq!(by_id(&s, "build").on_head, Some(false));
        assert!(by_id(&s, "build").reason.contains("(none)"));
    }
}
