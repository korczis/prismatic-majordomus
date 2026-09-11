//! The judgement: what each applicable gate said, whether it still describes this tree,
//! and whether the active task may therefore be called finished.
//!
//! # Why a status vocabulary of seven words and not two
//!
//! A colour is not a verdict. Two facts measured in this repository decide the shape of
//! this enum, and both are recorded in `.ai/repo/rules/project/never-reported-is-not-green.v1.md`:
//!
//! - On 2026-09-10 four master runs had every Linux job finished — five of them red — while
//!   three macOS jobs had been queued for between three and seven hours, so the `ci` verdict
//!   never ran at all. Master carried the defects overnight with nothing showing red. **A
//!   verdict that never arrived and a verdict that said pass must not be the same word**, and
//!   neither may be `unknown`: `unknown` is what a reader gets when the question cannot be
//!   asked, and it invites a shrug. [`GateStatus::Queued`] is what a reader gets when the
//!   question was asked of a runner that never answered, and it is a debt.
//! - The coverage gate here counts `#[cfg(test)]` code in its own denominator, so a
//!   percentage that rises after tests are added proves nothing. Nothing in this module
//!   reads a gate's *output*: it reads the gate's exit status and the hash of the inputs it
//!   was taken over, which is a claim that cannot be flattered by adding code.
//!
//! # Staleness is the whole point
//!
//! A gate's evidence is bound to the files that select it. `GateModel::inputs_of` derives those
//! from the model — the union of the classes that name the gate, and everything for a gate
//! every plan selects — and the hash is [`crate::capability::builtin::obligations`]'s, the
//! same definition `majordomus evidence` records with. Change one of those files and the
//! recorded hash stops describing the tree: the gate goes [`GateStatus::Stale`], which
//! blocks completion exactly as a failure does. Evidence that cannot expire is a claim
//! about the past presented as a claim about the present.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::model::{GateModel, GatePlan};

/// The ledger event a recorded gate run is, as `share/events.yaml` registers it.
pub const GATE_EVENT: &str = "task.gate";

// ---------------------------------------------------------------- the status vocabulary

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
/// What is known about one gate. Every word is one this repository's design tokens already
/// carry (`share/design/tokens.yaml`, `status.states`), so a surface renders it without
/// being taught a sixth vocabulary and the design gate does not refuse it.
pub enum GateStatus {
    /// It ran over these inputs and succeeded. The only word that discharges anything.
    Pass,
    /// It ran over these inputs and failed. Completion is refused.
    Fail,
    /// It ran, and the files that select it have changed since; what it proved is about a
    /// tree that no longer exists. Completion is refused, because the alternative is
    /// treating a claim about the past as a claim about the present.
    Stale,
    /// Something it cannot run without has not passed, so its own silence means nothing.
    Blocked,
    /// The plan selects it and no run has ever reported. Not `unknown`: the question was
    /// asked and no answer came, which is a debt somebody owes, and not `pass`, which is
    /// the mistake this word exists to make impossible.
    Queued,
    /// The plan does not select it: nothing the task changed can make it true or false.
    Exempt,
    /// It cannot be judged here at all — no model, no git, a ledger that would not read.
    /// An honest gap, and never a pass.
    Unknown,
}

impl GateStatus {
    /// The word this status is reported under.
    ///
    /// ```
    /// use majordomus_cli::gates::judge::GateStatus;
    /// assert_eq!(GateStatus::Queued.as_str(), "queued");
    /// // green and never-reported are different words, which is the whole point
    /// assert_ne!(GateStatus::Pass.as_str(), GateStatus::Queued.as_str());
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            GateStatus::Pass => "pass",
            GateStatus::Fail => "fail",
            GateStatus::Stale => "stale",
            GateStatus::Blocked => "blocked",
            GateStatus::Queued => "queued",
            GateStatus::Exempt => "exempt",
            GateStatus::Unknown => "unknown",
        }
    }

    /// Does this status, on a required gate, refuse canonical completion?
    ///
    /// True only for what is *known* to be wrong: a run that failed, a run whose evidence
    /// no longer describes the tree, and a gate whose prerequisite is in one of those
    /// states. Absence does not refuse — `queued` and `unknown` are reported as unverified,
    /// and a worker who claims a task is finished over them is making a claim the record
    /// contradicts rather than one the tool accepted.
    ///
    /// ```
    /// use majordomus_cli::gates::judge::GateStatus;
    /// assert!(GateStatus::Fail.refuses() && GateStatus::Stale.refuses());
    /// assert!(!GateStatus::Queued.refuses(), "absence is a debt, not a failure");
    /// assert!(!GateStatus::Exempt.refuses());
    /// ```
    pub fn refuses(self) -> bool {
        matches!(
            self,
            GateStatus::Fail | GateStatus::Stale | GateStatus::Blocked
        )
    }

    /// Is this status the absence of a verdict rather than a verdict?
    pub fn unverified(self) -> bool {
        matches!(self, GateStatus::Queued | GateStatus::Unknown)
    }
}

// ---------------------------------------------------------------- the evidence

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The `task.gate` ledger line that last reported a gate, as the ledger holds it.
pub struct GateRun {
    /// When it was recorded, RFC 3339 UTC.
    pub recorded_at: String,
    /// The commit the ledger's envelope stamped on it.
    pub head: String,
    /// The branch it was recorded on.
    pub branch: String,
    /// The exit status the gate reported. `0` is a pass.
    pub exit: i64,
    /// The command that produced it — narrative is not evidence.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    /// The hash of the files that select the gate, as they stood when it ran.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inputs_hash: String,
    /// The session that recorded it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session: String,
}

/// The newest `task.gate` line per gate for one task. A line that is not JSON is skipped
/// and counted: a ledger that has grown one bad line still holds the rest.
pub fn runs_for(ledger: &Path, task: &str) -> (BTreeMap<String, GateRun>, usize) {
    let Ok(text) = std::fs::read_to_string(ledger) else {
        return (BTreeMap::new(), 0);
    };
    let mut out: BTreeMap<String, GateRun> = BTreeMap::new();
    let mut skipped = 0usize;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            skipped += 1;
            continue;
        };
        let s = |k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        if s("event") != GATE_EVENT || s("task") != task {
            continue;
        }
        let gate = s("gate");
        if gate.is_empty() {
            skipped += 1;
            continue;
        }
        // the exit status is a number, and a line that carries it as a string is still
        // readable: the shell writes JSON by hand and one quoting mistake must not silence
        // a failing gate
        let exit = v.get("exit").and_then(|e| {
            e.as_i64()
                .or_else(|| e.as_str().and_then(|s| s.parse().ok()))
        });
        let Some(exit) = exit else {
            skipped += 1;
            continue;
        };
        // last line wins: the ledger is append-only and ordered
        out.insert(
            gate,
            GateRun {
                recorded_at: s("ts"),
                head: s("head"),
                branch: s("branch"),
                exit,
                command: s("command"),
                inputs_hash: s("inputs_hash"),
                session: s("session"),
            },
        );
    }
    (out, skipped)
}

// ---------------------------------------------------------------- one gate, judged

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One completion gate: what it is, whether it applies, what it said, and what to do.
pub struct Gate {
    /// The gate's identity in the model.
    pub id: String,
    /// One line naming it, from the model's own summary.
    pub title: String,
    /// What it is a gate over: the CI job that runs it.
    pub scope: String,
    /// True when the plan selects it and its verdict therefore decides completion. False
    /// for a gate the plan left out, and for one whose runner is only had on demand.
    pub required: bool,
    /// What is known about it.
    pub status: GateStatus,
    /// Why it is in this status, in words a worker can act on.
    pub reason: String,
    /// What would settle it, as a command.
    pub remediation: String,
    /// Where the gate is declared: the model, and the command it runs.
    pub source: String,
    /// The run that last reported, when one has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<GateRun>,
    /// The pathspecs its evidence is taken over, derived from the model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// The hash of those files as they stand now, when it could be taken.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inputs_hash: String,
    /// When this judgement was made, RFC 3339 UTC. A judgement is about a moment.
    pub evaluated_at: String,
}

/// Judge one gate of the model against the plan and the ledger.
#[allow(clippy::too_many_arguments)]
fn judge_one(
    model: &GateModel,
    plan: &GatePlan,
    id: &str,
    run: Option<&GateRun>,
    hash_now: Option<&str>,
    task: Option<&str>,
    now: &str,
) -> Gate {
    let decl = model.gate(id);
    let title = decl
        .map(|d| d.summary.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.to_string());
    let scope = decl.map_or_else(|| "unknown".to_string(), |d| d.job.clone());
    let runs = decl.map_or_else(String::new, |d| d.runs.clone());
    let inputs = model.inputs_of(id);
    let selected = plan.selects(id);
    let remediation = if runs.is_empty() {
        format!("majordomus evidence --gate {id} --exit 0 --command '<the gate>'")
    } else {
        format!("{runs} && majordomus evidence --gate {id} --exit $?")
    };

    let (status, reason) = if !selected {
        (
            GateStatus::Exempt,
            plan.excluded
                .get(id)
                .cloned()
                .unwrap_or_else(|| "the plan does not select it".to_string()),
        )
    } else if task.is_none() {
        (
            GateStatus::Unknown,
            "no active task in this checkout, so no run can be attributed to one".to_string(),
        )
    } else {
        match run {
            None => (
                GateStatus::Queued,
                format!(
                    "planned ({}) and no run has reported; the change is unverified against it, \
                     which is not the same as passing",
                    plan.selected.get(id).map_or("selected", String::as_str)
                ),
            ),
            Some(r) => {
                let now_hash = hash_now.unwrap_or("");
                if hash_now.is_none() {
                    (
                        GateStatus::Unknown,
                        "the files that select this gate could not be hashed here, so the run \
                         cannot be checked against this tree"
                            .to_string(),
                    )
                } else if r.inputs_hash != now_hash {
                    (
                        GateStatus::Stale,
                        format!(
                            "the run was taken over inputs {} and this tree hashes to {}; \
                             it no longer describes what it proved",
                            short(&r.inputs_hash),
                            short(now_hash)
                        ),
                    )
                } else if r.exit == 0 {
                    (
                        GateStatus::Pass,
                        format!(
                            "exit 0 at {} over inputs {}",
                            short(&r.head),
                            short(now_hash)
                        ),
                    )
                } else {
                    (
                        GateStatus::Fail,
                        format!(
                            "exit {} at {} over inputs {}",
                            r.exit,
                            short(&r.head),
                            short(now_hash)
                        ),
                    )
                }
            }
        }
    };

    Gate {
        id: id.to_string(),
        title,
        scope,
        required: selected && !decl.is_some_and(|d| d.on_demand),
        status,
        reason,
        remediation,
        source: if runs.is_empty() {
            super::model::MODEL_PATH.to_string()
        } else {
            format!("{} ({runs})", super::model::MODEL_PATH)
        },
        evidence: run.cloned(),
        inputs,
        inputs_hash: hash_now.unwrap_or_default().to_string(),
        evaluated_at: now.to_string(),
    }
}

/// Twelve characters of a hash or a commit, the width every report here uses.
fn short(v: &str) -> String {
    if v.is_empty() {
        "(none)".to_string()
    } else {
        v.chars().take(12).collect()
    }
}

// ---------------------------------------------------------------- the whole judgement

/// Judge every gate of the model, then let `requires` propagate: a gate whose prerequisite
/// is refused is [`GateStatus::Blocked`], because its own silence proves nothing.
///
/// A gate that has already spoken keeps what it said. Blocking replaces only the absence of
/// a verdict, so a failing prerequisite never hides a second real failure.
pub fn judge(
    model: &GateModel,
    plan: &GatePlan,
    runs: &BTreeMap<String, GateRun>,
    hashes: &BTreeMap<String, Option<String>>,
    task: Option<&str>,
    now: &str,
) -> Vec<Gate> {
    let mut out: Vec<Gate> = model
        .gates
        .iter()
        .map(|d| {
            judge_one(
                model,
                plan,
                &d.id,
                runs.get(&d.id),
                hashes.get(&d.id).and_then(Option::as_deref),
                task,
                now,
            )
        })
        .collect();

    // to a fixed point, so a chain of prerequisites propagates once
    loop {
        let refused: Vec<String> = out
            .iter()
            .filter(|g| g.status.refuses())
            .map(|g| g.id.clone())
            .collect();
        let mut changed = false;
        for gate in out.iter_mut() {
            if !gate.status.unverified() {
                continue;
            }
            let Some(decl) = model.gate(&gate.id) else {
                continue;
            };
            let Some(cause) = decl.requires.iter().find(|r| refused.contains(r)) else {
                continue;
            };
            gate.status = GateStatus::Blocked;
            gate.reason = format!(
                "{} has not passed, and this gate cannot run without it, so its silence \
                 says nothing about this change",
                cause
            );
            changed = true;
        }
        if !changed {
            break;
        }
    }
    out
}

/// Sort key: the order a reader wants — what refuses first, then what is unverified, then
/// the rest — and by id inside each band, through [`crate::order`] so the comparison is the
/// repository's one comparator.
pub fn report_order(gates: &mut [Gate]) {
    gates.sort_by(|a, b| {
        band(a.status)
            .cmp(&band(b.status))
            .then_with(|| crate::order::natural_cmp(&a.id, &b.id))
    });
}

fn band(status: GateStatus) -> u8 {
    match status {
        GateStatus::Fail | GateStatus::Stale | GateStatus::Blocked => 0,
        GateStatus::Queued | GateStatus::Unknown => 1,
        GateStatus::Pass => 2,
        GateStatus::Exempt => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::model::plan;
    use crate::metadata::yaml;

    const MODEL: &str = r#"version: 1
gates:
  - id: lint
    job: structure
    always: true
    runs: scripts/lint
    summary: every script parses
  - id: build
    job: rust
    runs: cargo build
    summary: the crate builds
  - id: probe
    job: site
    requires: [site]
    runs: scripts/probe
    summary: every route answers
  - id: site
    job: site
    runs: scripts/site-build
    summary: the site builds
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

    fn run(exit: i64, hash: &str) -> GateRun {
        GateRun {
            recorded_at: "2026-09-11T00:00:00Z".into(),
            head: "0123456789abcdef".into(),
            branch: "feature/x".into(),
            exit,
            command: "cargo build".into(),
            inputs_hash: hash.into(),
            session: "s-1".into(),
        }
    }

    fn hashes(model: &GateModel, value: &str) -> BTreeMap<String, Option<String>> {
        model
            .gates
            .iter()
            .map(|g| (g.id.clone(), Some(value.to_string())))
            .collect()
    }

    #[test]
    fn a_planned_gate_nothing_reported_is_queued_and_not_unknown() {
        let m = model();
        let p = plan(&m, &["apps/x.rs".into()], false);
        let g = judge(
            &m,
            &p,
            &BTreeMap::new(),
            &hashes(&m, "aaaa"),
            Some("t-1"),
            "now",
        );
        let build = g.iter().find(|g| g.id == "build").expect("planned");
        assert_eq!(build.status, GateStatus::Queued);
        assert!(build.required);
        assert!(!build.status.refuses(), "absence is a debt, not a failure");
        // and a gate the plan left out is exempt rather than unknown
        let site = g.iter().find(|g| g.id == "site").expect("declared");
        assert_eq!(site.status, GateStatus::Exempt);
        assert!(!site.required);
    }

    #[test]
    fn a_failing_run_refuses_and_a_passing_one_does_not() {
        let m = model();
        let p = plan(&m, &["apps/x.rs".into()], false);
        let mut runs = BTreeMap::new();
        runs.insert("build".to_string(), run(1, "aaaa"));
        let g = judge(&m, &p, &runs, &hashes(&m, "aaaa"), Some("t-1"), "now");
        let build = g.iter().find(|g| g.id == "build").unwrap();
        assert_eq!(build.status, GateStatus::Fail);
        assert!(build.status.refuses());
        assert!(build.reason.contains("exit 1"));

        runs.insert("build".to_string(), run(0, "aaaa"));
        let g = judge(&m, &p, &runs, &hashes(&m, "aaaa"), Some("t-1"), "now");
        assert_eq!(
            g.iter().find(|g| g.id == "build").unwrap().status,
            GateStatus::Pass
        );
    }

    #[test]
    fn a_passing_run_goes_stale_when_the_files_that_select_it_change() {
        let m = model();
        let p = plan(&m, &["apps/x.rs".into()], false);
        let mut runs = BTreeMap::new();
        runs.insert("build".to_string(), run(0, "aaaa"));
        // the same run, judged against a tree that has moved on
        let g = judge(&m, &p, &runs, &hashes(&m, "bbbb"), Some("t-1"), "now");
        let build = g.iter().find(|g| g.id == "build").unwrap();
        assert_eq!(build.status, GateStatus::Stale);
        assert!(build.status.refuses(), "stale refuses exactly as fail does");
        assert!(build.reason.contains("aaaa") && build.reason.contains("bbbb"));
    }

    #[test]
    fn a_gate_whose_prerequisite_failed_is_blocked_rather_than_queued() {
        let m = model();
        let p = plan(&m, &["site/index.md".into()], false);
        let mut runs = BTreeMap::new();
        runs.insert("site".to_string(), run(2, "aaaa"));
        let g = judge(&m, &p, &runs, &hashes(&m, "aaaa"), Some("t-1"), "now");
        let probe = g.iter().find(|g| g.id == "probe").unwrap();
        assert_eq!(probe.status, GateStatus::Blocked);
        assert!(probe.reason.contains("site"));
        // and a gate that already spoke keeps what it said
        assert_eq!(
            g.iter().find(|g| g.id == "site").unwrap().status,
            GateStatus::Fail
        );
    }

    #[test]
    fn nothing_can_be_judged_without_a_task_and_that_is_unknown() {
        let m = model();
        let p = plan(&m, &["apps/x.rs".into()], false);
        let g = judge(&m, &p, &BTreeMap::new(), &hashes(&m, "aaaa"), None, "now");
        let build = g.iter().find(|g| g.id == "build").unwrap();
        assert_eq!(build.status, GateStatus::Unknown);
        assert!(!build.status.refuses());
    }

    #[test]
    fn a_run_that_cannot_be_hashed_here_is_unknown_and_never_a_pass() {
        let m = model();
        let p = plan(&m, &["apps/x.rs".into()], false);
        let mut runs = BTreeMap::new();
        runs.insert("build".to_string(), run(0, "aaaa"));
        let mut h: BTreeMap<String, Option<String>> = BTreeMap::new();
        h.insert("build".to_string(), None);
        let g = judge(&m, &p, &runs, &h, Some("t-1"), "now");
        assert_eq!(
            g.iter().find(|g| g.id == "build").unwrap().status,
            GateStatus::Unknown
        );
    }

    #[test]
    fn the_ledger_reader_takes_the_newest_line_per_gate_and_skips_what_it_cannot_read() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("ledger.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"event":"task.gate","task":"t-1","gate":"build","exit":1,"inputs_hash":"aaaa","ts":"1"}"#,
                "\n",
                r#"{"event":"task.gate","task":"t-1","gate":"build","exit":0,"inputs_hash":"bbbb","ts":"2"}"#,
                "\n",
                r#"{"event":"task.gate","task":"other","gate":"build","exit":1,"inputs_hash":"cccc","ts":"3"}"#,
                "\n",
                "not json at all\n",
                r#"{"event":"task.evidence","task":"t-1","covers":"tests"}"#,
                "\n",
            ),
        )
        .expect("write");
        let (runs, skipped) = runs_for(&path, "t-1");
        assert_eq!(skipped, 1, "one unreadable line, counted rather than fatal");
        let build = runs.get("build").expect("the task's own line");
        assert_eq!(build.exit, 0, "the newest line wins");
        assert_eq!(build.inputs_hash, "bbbb");
        assert_eq!(runs.len(), 1, "another task's run is not this task's");
    }

    #[test]
    fn what_refuses_first_is_reported_first() {
        let m = model();
        let p = plan(&m, &["site/index.md".into()], false);
        let mut runs = BTreeMap::new();
        runs.insert("site".to_string(), run(2, "aaaa"));
        let mut g = judge(&m, &p, &runs, &hashes(&m, "aaaa"), Some("t-1"), "now");
        report_order(&mut g);
        assert!(
            g[0].status.refuses(),
            "a reader meets the refusal first, not the exempt list"
        );
        assert_eq!(g.last().unwrap().status, GateStatus::Exempt);
    }
}
