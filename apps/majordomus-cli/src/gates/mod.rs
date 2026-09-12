//! Completion as a provable state: which gates a task's own change set must pass, what
//! each one said, whether that verdict still describes the tree, and therefore whether the
//! task may be called finished.
//!
//! # The problem this exists for
//!
//! A worker saying "done" is a claim about a repository, and until this module the
//! repository had no way to contradict it at the level of the *validation pipeline*.
//! `finish` already refused over the task's `requires` — the obligation closure of ADR
//! 0030 — but the eighteen-odd gates the CI model declares had no bearing on whether a task
//! could be completed at all: they were a property of a pull request, not of a task. So a
//! task could be finished with `rust-check` red, or with `shell-suite` never having run.
//!
//! # Nothing here is a second declaration
//!
//! - **The gates** are `.ai/repo/ci/gates.yaml`, read by [`model::GateModel`]. `scripts/ci-plan`
//!   is the other reader; the behavioural case asserts they select the same set.
//! - **Applicability** is that model's own rules, applied to the task's own change set —
//!   the paths between the task's starting commit and the working tree. Not a table of
//!   "source implies tests": the classes in the model already say which paths select which
//!   gates, and a class edited tomorrow changes the answer tomorrow.
//! - **The obligations** are `share/obligations.yaml`, judged by
//!   [`crate::capability::builtin::obligations`]. This module derives which of them the
//!   change set *implies* — from each token's own declared `inputs` — and reports whether
//!   the task declared it. It does not re-judge a declared one; `obligations.closure` and
//!   `mj_validate_obligations` own that and there is one judgement, not two.
//! - **The evidence and its staleness** are the ledger and the inputs hash, the mechanism
//!   `majordomus evidence` already writes and ADR 0030 already argues for.
//!
//! # The one thing that is new
//!
//! A gate run is now a fact the record can hold: `majordomus evidence --gate <id> --exit <n>`
//! writes a `task.gate` line carrying the hash of the files that select that gate, and
//! [`judge`] turns those lines into a status. Recording is the shell tool's, because the
//! ledger has one writer (ADR 0030); judging is here, because a judgement with two
//! implementations is how `check` and a gate come to disagree.
//!
//! # One lifecycle
//!
//! A change set goes through the model to a plan, the plan and the recorded runs through the
//! judgement to a status per gate, and the statuses fold into one verdict.
//!
//! ```
//! use majordomus_cli::gates::GateStatus;
//!
//! // the vocabulary the whole module reports in, and the two rules that make it useful
//! assert!(GateStatus::Fail.refuses() && GateStatus::Stale.refuses());
//! assert!(GateStatus::Queued.unverified() && !GateStatus::Queued.refuses());
//! // and the one word that discharges anything
//! assert!(!GateStatus::Pass.refuses() && !GateStatus::Pass.unverified());
//! ```
//!

// The three halves of the judgement are the crate's own. What a reader outside the crate
// needs is the *documents* — the completion report and the types inside it — and those are
// re-exported below, so the public surface is the answer and not the machinery that makes
// it.
pub(crate) mod done;
pub(crate) mod judge;
pub(crate) mod model;
pub(crate) mod policy;
pub(crate) mod stage;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::builtin::obligations::Obligation;
use crate::capability::builtin::ActiveTask;
use crate::discovery::glob::Glob;

pub use done::{
    DoneInputs, DoneQuestion, HandoverStanding, IssueStanding, ObligationStanding,
    ReleaseStanding,
};
pub use judge::{Gate, GateRun, GateStatus};
pub(crate) use model::GateModel;
pub use model::{GateClass, GateClassMatch, GateDecl, GatePlan, GatePlanMode};
pub use policy::{CompletionPolicy, QuestionDecl, QuestionSource, StageDecl, POLICY_FILE};
pub use stage::{derive_stage, LifecycleStage, StageReport, StageState};

use crate::deploy::targets::DeploymentPlan;

/// ```
/// use majordomus_cli::gates::VersionSummary;
/// let v: VersionSummary = serde_json::from_value(serde_json::json!({
///     "baseline": "0.5.0", "declared": "0.6.0", "required": "0.6.0",
///     "impact": "minor", "status": "ok", "breaking": false, "changes": 12
/// })).unwrap();
/// // the one sentence every surface shows about the version, reduced from the plan
/// // `release.analysis` answers with; nothing here is measured a second time
/// assert!(v.ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The structural version analysis, reduced to what a completion report states.
pub struct VersionSummary {
    /// The last released version the tree was measured against.
    pub baseline: String,
    /// The version the tree declares.
    pub declared: String,
    /// The smallest version the contract allows the tree to declare.
    pub required: String,
    /// The impact the contract movement requires (`none`, `patch`, `minor`, `major`).
    pub impact: String,
    /// `ok` when the declared version is at least the required one, else `blocked`.
    pub status: String,
    /// Whether anything a caller could hold is gone.
    pub breaking: bool,
    /// How many movements of the contract were found.
    pub changes: usize,
}

impl VersionSummary {
    /// Whether the declared version satisfies the contract.
    pub fn ok(&self) -> bool {
        self.status == "ok"
    }
}

/// The judgements the done invariant is composed from, each made by the subsystem that
/// owns the fact. Passed as one value so that the capability, a test and a benchmark drive
/// [`complete`] identically.
pub(crate) struct Sources<'a> {
    /// The completion policy, `share/completion.yaml`.
    pub policy: &'a CompletionPolicy,
    /// The obligation closure's word per token.
    pub standing: &'a BTreeMap<String, ObligationStanding>,
    /// Whether the closure could be read at all.
    pub closure_reachable: bool,
    /// The release analysis, reduced.
    pub release: ReleaseStanding,
    /// The same analysis, as the report states it.
    pub version: Option<VersionSummary>,
    /// The issue the task names.
    pub issue: IssueStanding,
    /// The continuation record.
    pub handover: HandoverStanding,
    /// Which deployment targets the change reaches.
    pub deployment: DeploymentPlan,
}

/// The local half of the layer, relative to the repository root. The same constant
/// `obligations` and `continuity` state, for the same reason: the shell tool decides where
/// its state lives and a second opinion about the path would be a second source of truth.
pub(crate) const STATE_DIR: &str = ".ai/local/state";

// ---------------------------------------------------------------- derived obligations

/// ```
/// use majordomus_cli::gates::ImpliedObligation;
///
/// let owed = ImpliedObligation {
///     id: "tests".into(),
///     title: "The cases the change obliges were run".into(),
///     applicable: true,
///     declared: false,
///     reason: "lib/check.sh is an input of this obligation".into(),
///     because: vec!["lib/check.sh".into()],
///     remediation: "majordomus start --requires tests".into(),
/// };
/// // applicable and undeclared is the gap nothing else reports: the change owes it and
/// // the task promised nothing, so no validator holds anybody to it
/// assert!(owed.applicable && !owed.declared);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One obligation the change set implies, and whether the task promised it.
///
/// Applicability is derived and nothing else: a token's own `inputs` in
/// `share/obligations.yaml` name the files it is about, so a change to one of them is what
/// makes the token owed. A token with no `inputs` names a fact outside the tree — a push, a
/// publication, a deployment — which no change to a tree can imply, so those are owed when
/// the task declares them and reported as not applicable when it does not.
pub struct ImpliedObligation {
    /// The token.
    pub id: String,
    /// The obligation as a heading, from the vocabulary.
    pub title: String,
    /// True when the change set, or the task's own declaration, makes it owed.
    pub applicable: bool,
    /// True when the task's `requires` names it, so `finish` already holds the task to it.
    pub declared: bool,
    /// Why it applies, or why it does not.
    pub reason: String,
    /// The changed paths that implied it, when paths did.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub because: Vec<String>,
    /// What would discharge it.
    pub remediation: String,
}

/// Which obligations a change set implies, against the shipped vocabulary, the task's
/// own declaration and the deployment plan.
///
/// The derivation, in one sentence each:
///
/// - a token whose declared `inputs` cover a changed path is owed, which is how source
///   modification comes to imply `tests`, a document change to imply `docs`, and a change
///   under `share/` or `.ai/repo/` to imply `generated` — every one of those from the
///   token's own data and none from a table here;
/// - `commit`, `push` and `target` are owed whenever anything changed at all, because work
///   that exists only in a working tree, or only on a branch, is not work the repository
///   has: a completed task's work is on the trunk;
/// - `pages`, `deploy` and `verify` are owed when the deployment plan says the change
///   reaches the published site, an active deployment, or any surface at all — the plan is
///   derived from the CI model's path classes and the deployment objects, so a task that
///   touches what the site is built from owes its publication whether or not it said so;
/// - every other token is owed exactly when the task declared it, which is the difference
///   between "not applicable" and "unknown".
pub(crate) fn implied(
    vocabulary: &[Obligation],
    changed: &[String],
    task: Option<&ActiveTask>,
    deployment: &DeploymentPlan,
) -> Vec<ImpliedObligation> {
    let reaches = |pred: &dyn Fn(&crate::deploy::targets::DeploymentTarget) -> bool| -> Vec<String> {
        deployment
            .targets
            .iter()
            .filter(|t| t.applicable && pred(t))
            .map(|t| t.id.clone())
            .collect()
    };
    let pages = reaches(&|t| t.kind == crate::deploy::targets::TargetKind::Pages);
    let apps = reaches(&|t| t.kind == crate::deploy::targets::TargetKind::Application);
    let any = reaches(&|_| true);
    let declared: BTreeSet<&str> = task
        .map(|t| t.requires.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for o in vocabulary {
        let hits: Vec<String> = changed
            .iter()
            .filter(|p| o.inputs.iter().any(|spec| Glob::new(spec).matches(p)))
            .cloned()
            .collect();
        let is_declared = declared.contains(o.id.as_str());
        let (applicable, reason) = if !hits.is_empty() {
            (
                true,
                format!(
                    "the change touches {} file(s) this obligation is taken over",
                    hits.len()
                ),
            )
        } else if o.id == "commit" && !changed.is_empty() {
            (
                true,
                "the change exists, so it owes being in the branch's history".to_string(),
            )
        } else if (o.id == "push" || o.id == "target") && !changed.is_empty() {
            (
                true,
                "the change exists, so it owes reaching the remote and the trunk: a branch \
                 nobody integrated is not finished work"
                    .to_string(),
            )
        } else if o.id == "pages" && !pages.is_empty() {
            (
                true,
                "the deployment plan says the change reaches the published site".to_string(),
            )
        } else if o.id == "deploy" && !apps.is_empty() {
            (
                true,
                format!(
                    "the deployment plan says the change reaches an active deployment ({})",
                    apps.join(", ")
                ),
            )
        } else if o.id == "verify" && !any.is_empty() {
            (
                true,
                format!(
                    "the deployment plan says the change reaches {}; what was deployed is \
                     looked at afterwards",
                    any.join(", ")
                ),
            )
        } else if is_declared {
            (true, "the task's own `requires` declares it".to_string())
        } else if o.inputs.is_empty() {
            (
                false,
                format!(
                    "a fact outside this tree that no change to it can imply; declare it in \
                     the task's `requires` to owe it ({})",
                    o.discharged_by
                ),
            )
        } else {
            (
                false,
                "the change touches nothing this obligation is taken over".to_string(),
            )
        };
        out.push(ImpliedObligation {
            id: o.id.clone(),
            title: o.title.clone(),
            applicable,
            declared: is_declared,
            reason,
            because: hits,
            remediation: if o.discharged_by == "none" {
                format!(
                    "majordomus evidence --covers {} --command '<what proved it>'",
                    o.id
                )
            } else {
                format!(
                    "{} && majordomus evidence --covers {} --command '{}'",
                    o.discharged_by, o.id, o.discharged_by
                )
            },
        });
    }
    out
}

// ---------------------------------------------------------------- the report

/// ```
/// use majordomus_cli::gates::Completion;
///
/// // the document every surface reads, as `gates.completion` answers with it. The property
/// // that matters is the one asserted here: a gate that never reported leaves the task
/// // finishable and *unverified*, which is a different thing from a task that passed.
/// let c: Completion = serde_json::from_value(serde_json::json!({
///     "present": true,
///     "model": ".ai/repo/ci/gates.yaml",
///     "plan": { "mode": "affected", "reason": "1 path in 1 class", "changed": ["docs/CLI.md"],
///               "classes": [], "unclassified": [], "selected": {}, "excluded": {} },
///     "finishable": true,
///     "unverified": ["reference-check"],
///     "tallies": { "queued": 1 },
///     "gates": [],
///     "obligations": [],
///     "questions": [],
///     "policy": "share/completion.yaml",
///     "stage": { "id": "gates", "title": "Gates", "state": "pending", "complete": false,
///                "owing": ["ci"], "stages": [] },
///     "verified": false,
///     "complete": false,
///     "deployment": { "targets": [] }
/// }))
/// .unwrap();
/// assert!(c.finishable, "absence of a verdict does not refuse");
/// assert_eq!(c.unverified, ["reference-check"], "and it is never silently accepted");
/// assert!(c.blocking.is_empty());
/// // and the three words a reader acts on are three different facts
/// assert!(!c.verified, "finishable and unverified: nothing refused, something never reported");
/// assert!(!c.complete, "and nothing is complete while a question is owed");
/// assert_eq!(c.stage.id, "gates");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// Whether the active task may be called finished, and everything the answer rests on.
///
/// `finishable` is the field a caller acts on and the one thing no surface may recompute:
/// the shell tool's `check` and `finish` read it, the HTTP API serves it, the Cockpit
/// renders it, and all of them are reading this one execution.
pub struct Completion {
    /// True when a task record was found in this checkout.
    pub present: bool,
    /// The task, as its record declares it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<ActiveTask>,
    /// The CI model this was judged against.
    pub model: String,
    /// Affected or full, and why.
    pub plan: GatePlan,
    /// False when a required gate is known to be failing, stale, or blocked by one that is.
    /// Absence of a verdict does not make this false — it makes `unverified` non-empty.
    pub finishable: bool,
    /// The required gates whose status refuses completion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocking: Vec<String>,
    /// The required gates that have never reported, or cannot be judged here. A task
    /// finished over these is finished unverified, and the record says so.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unverified: Vec<String>,
    /// How many gates stand in each status, by word.
    pub tallies: BTreeMap<String, usize>,
    /// Every gate of the model, what refuses first.
    pub gates: Vec<Gate>,
    /// Which obligations the change set implies, and whether the task promised them. The
    /// judgement of a declared one is `obligations.closure`'s, not this module's.
    pub obligations: Vec<ImpliedObligation>,
    /// The done invariant: the questions a task must answer before anybody may call it
    /// finished, each with the source that answered it and the evidence it read. Composed
    /// from the obligation closure, the gates and the change set; a question nothing here
    /// reaches is `unknown` and names the command that would answer it.
    pub questions: Vec<DoneQuestion>,
    /// The completion policy the questions were taken from.
    pub policy: String,
    /// Where the task stands in the lifecycle, derived from the questions in the policy's
    /// order: the first stage blocked, else the first stage pending, else complete.
    pub stage: LifecycleStage,
    /// True when `finishable` holds and every required gate has reported over this tree:
    /// nothing refuses and nothing is silent.
    pub verified: bool,
    /// True only when every question of the policy passes or is exempt. This is the one
    /// field that may be read as "done", and no surface computes it a second time.
    pub complete: bool,
    /// The structural version analysis, when it could be made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionSummary>,
    /// Which deployment targets the change reaches, and why each applies or does not.
    pub deployment: DeploymentPlan,
    /// What could not be established, each as one line. Never empty when something was
    /// skipped: a gap is reported rather than left to be inferred.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

/// The paths a task changed: everything between the commit it started at and the working
/// tree, the uncommitted half included.
///
/// The same two questions `scripts/ci-plan` asks of git without a base, and for the same
/// reason: a plan computed over the committed half only would go green on a worker who has
/// not committed, which is the failure mode this whole module exists to close.
pub(crate) fn changed_paths(root: &Path, base: Option<&str>) -> Result<Vec<String>, String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    let git = |args: &[&str]| -> Result<String, String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
        if !out.status.success() {
            return Err(format!(
                "git {} exited {}",
                args.join(" "),
                out.status.code().unwrap_or(-1)
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    if let Some(base) = base.filter(|b| !b.is_empty() && *b != "NONE") {
        for line in git(&["diff", "--name-only", base, "HEAD"])?.lines() {
            if !line.trim().is_empty() {
                out.insert(line.to_string());
            }
        }
    }
    for line in git(&["status", "--porcelain=v1"])?.lines() {
        if line.len() < 4 {
            continue;
        }
        let path = &line[3..];
        // a rename reports "old -> new"; the new name is the one a gate would read
        let path = path.rsplit(" -> ").next().unwrap_or(path);
        if !path.trim().is_empty() {
            out.insert(path.to_string());
        }
    }
    Ok(out.into_iter().collect())
}

/// Compose the whole answer.
///
/// Everything expensive is passed in rather than fetched here, so that the capability, a
/// test and a benchmark all drive the same function: the model, the change set, the task,
/// the vocabulary and the per-gate hashes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn complete(
    model: &GateModel,
    changed: &[String],
    task: Option<&ActiveTask>,
    vocabulary: &[Obligation],
    runs: &BTreeMap<String, GateRun>,
    hashes: &BTreeMap<String, Option<String>>,
    sources: &Sources<'_>,
    on_demand: bool,
    now: &str,
    mut findings: Vec<String>,
) -> Completion {
    let plan = model::plan(model, changed, on_demand);
    let mut gates = judge::judge(model, &plan, runs, hashes, task.map(|t| t.id.as_str()), now);
    judge::report_order(&mut gates);

    let blocking: Vec<String> = gates
        .iter()
        .filter(|g| g.required && g.status.refuses())
        .map(|g| g.id.clone())
        .collect();
    let unverified: Vec<String> = gates
        .iter()
        .filter(|g| g.required && g.status.unverified())
        .map(|g| g.id.clone())
        .collect();
    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    for g in &gates {
        *tallies.entry(g.status.as_str().to_string()).or_default() += 1;
    }
    if !unverified.is_empty() {
        findings.push(format!(
            "{} required gate(s) have never reported: {}. Never-reported is not green \
             (project.never-reported-is-not-green); a task completed over these is completed \
             unverified",
            unverified.len(),
            unverified.join(" ")
        ));
    }
    let obligations = implied(vocabulary, changed, task, &sources.deployment);
    for o in &obligations {
        if o.applicable && !o.declared {
            findings.push(format!(
                "the change implies the obligation '{}' and the task does not declare it, \
                 so `finish` will not hold it to that: {}",
                o.id, o.reason
            ));
        }
    }

    let questions = done::answer(
        sources.policy,
        &DoneInputs {
            standing: sources.standing,
            implied: &obligations,
            gates: &gates,
            changed,
            closure_reachable: sources.closure_reachable,
            release: sources.release.clone(),
            issue: sources.issue.clone(),
            handover: sources.handover.clone(),
        },
    );
    let stage = derive_stage(&sources.policy.stages, &questions);
    let finishable = blocking.is_empty();
    let verified = finishable && unverified.is_empty();
    // `complete` is the stage fold's word and nothing else: every question pass or exempt.
    // It implies `verified` (the ci question is one of them) and is never recomputed by a
    // surface from the parts.
    let complete = task.is_some() && stage.complete;

    Completion {
        present: task.is_some(),
        task: task.cloned(),
        model: model::MODEL_PATH.to_string(),
        plan,
        finishable,
        blocking,
        unverified,
        tallies,
        gates,
        obligations,
        questions,
        policy: sources.policy.source.clone(),
        stage,
        verified,
        complete,
        version: sources.version.clone(),
        deployment: sources.deployment.clone(),
        findings,
    }
}

/// The commit this checkout is at, as git states it; `None` outside a repository or
/// before the first commit. What a deployment target is expected to serve when nothing
/// names another revision.
pub(crate) fn head_of(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let head = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!head.is_empty()).then_some(head)
}

/// Where this checkout's ledger is.
pub(crate) fn ledger_path(root: &Path) -> std::path::PathBuf {
    root.join(STATE_DIR).join("ledger.jsonl")
}

/// Where this checkout's task record is.
pub(crate) fn task_path(root: &Path) -> std::path::PathBuf {
    root.join(STATE_DIR).join("current.yaml")
}

#[cfg(test)]
mod tests {
    use super::*;
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
classes:
  - id: rust
    paths: [apps/**]
    gates: [build]
  - id: docs
    paths: [docs/**]
    gates: []
"#;

    fn model() -> GateModel {
        yaml::parse_into(MODEL).expect("the fixture model parses")
    }

    fn vocabulary() -> Vec<Obligation> {
        serde_json::from_str(
            r#"[
              {"id":"tests","title":"The cases were run","summary":"s",
               "discharged_by":"usecase impact","inputs":["apps/**","lib/**"],"remote":false},
              {"id":"docs","title":"The documentation was written","summary":"s",
               "discharged_by":"scripts/ci/reference-check","inputs":["docs/**"],"remote":false},
              {"id":"commit","title":"The work is committed","summary":"s",
               "discharged_by":"git","remote":false},
              {"id":"deploy","title":"The application is running it","summary":"s",
               "discharged_by":"majordomus deployment","remote":true}
            ]"#,
        )
        .expect("the fixture vocabulary parses")
    }

    fn task(requires: &[&str]) -> ActiveTask {
        ActiveTask {
            id: "t-1".into(),
            task: "work".into(),
            profile: "implementation".into(),
            outcome: "active".into(),
            scope: vec![],
            requires: requires.iter().map(|s| (*s).to_string()).collect(),
            started_at: "2026-09-11T00:00:00Z".into(),
            head: "0123456789ab".into(),
            issue: String::new(),
        }
    }

    fn sources() -> Sources<'static> {
        use std::sync::OnceLock;
        static POLICY: OnceLock<CompletionPolicy> = OnceLock::new();
        static STANDING: OnceLock<BTreeMap<String, ObligationStanding>> = OnceLock::new();
        let policy = POLICY.get_or_init(|| {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
            CompletionPolicy::load(&root).unwrap()
        });
        Sources {
            policy,
            standing: STANDING.get_or_init(BTreeMap::new),
            closure_reachable: false,
            release: ReleaseStanding::Unknown("not asked".into()),
            version: None,
            issue: IssueStanding::Unknown("no plan".into()),
            handover: HandoverStanding::Unknown("no store".into()),
            deployment: DeploymentPlan::default(),
        }
    }

    fn hashes(m: &GateModel, v: &str) -> BTreeMap<String, Option<String>> {
        m.gates
            .iter()
            .map(|g| (g.id.clone(), Some(v.to_string())))
            .collect()
    }

    #[test]
    fn source_modification_implies_tests_and_a_document_change_does_not() {
        let v = vocabulary();
        let i = implied(&v, &["apps/majordomus-cli/src/lib.rs".into()], None, &DeploymentPlan::default());
        let tests = i.iter().find(|o| o.id == "tests").unwrap();
        assert!(tests.applicable, "{}", tests.reason);
        assert!(!tests.declared, "the task declared nothing");
        assert!(!i.iter().find(|o| o.id == "docs").unwrap().applicable);
        // and the reverse
        let i = implied(&v, &["docs/CLI.md".into()], None, &DeploymentPlan::default());
        assert!(i.iter().find(|o| o.id == "docs").unwrap().applicable);
        assert!(!i.iter().find(|o| o.id == "tests").unwrap().applicable);
    }

    #[test]
    fn a_deployment_is_owed_when_the_task_declares_it_and_not_applicable_otherwise() {
        let v = vocabulary();
        let changed = vec!["apps/x.rs".to_string()];

        let none = implied(&v, &changed, Some(&task(&[])), &DeploymentPlan::default());
        let d = none.iter().find(|o| o.id == "deploy").unwrap();
        assert!(!d.applicable, "no change to a tree can imply a deployment");
        assert!(d.reason.contains("outside this tree"));

        let declared = implied(&v, &changed, Some(&task(&["deploy"])), &DeploymentPlan::default());
        let d = declared.iter().find(|o| o.id == "deploy").unwrap();
        assert!(d.applicable && d.declared);
    }

    #[test]
    fn the_deployment_plan_implies_publication_and_verification_without_a_declaration() {
        use crate::deploy::targets::{DeploymentTarget, Identity, TargetKind};
        let v: Vec<Obligation> = serde_json::from_str(
            r#"[
              {"id":"pages","title":"p","summary":"s","discharged_by":"scripts/pages verify","remote":true},
              {"id":"verify","title":"v","summary":"s","discharged_by":"deploy.verify","remote":true},
              {"id":"deploy","title":"d","summary":"s","discharged_by":"deploy.verify","remote":true},
              {"id":"push","title":"u","summary":"s","discharged_by":"git","remote":true},
              {"id":"target","title":"t","summary":"s","discharged_by":"git","remote":true}
            ]"#,
        )
        .unwrap();
        let target = |id: &str, kind: TargetKind, applicable: bool| DeploymentTarget {
            id: id.into(),
            kind,
            url: None,
            identity_url: None,
            applicable,
            reason: "r".into(),
            expected: Identity::default(),
            inputs: vec![],
            because: vec![],
        };
        let plan = DeploymentPlan {
            targets: vec![
                target("pages", TargetKind::Pages, true),
                target("majordomus", TargetKind::Application, false),
            ],
            findings: vec![],
        };
        let i = implied(&v, &["docs/x.md".into()], None, &plan);
        let by = |id: &str| i.iter().find(|o| o.id == id).unwrap().clone();
        assert!(by("pages").applicable, "{}", by("pages").reason);
        assert!(by("verify").applicable, "{}", by("verify").reason);
        assert!(!by("deploy").applicable, "no active deployment is reached");
        assert!(by("push").applicable && by("target").applicable, "a change owes the trunk");
        // and nothing changed: nothing owed
        let none = implied(&v, &[], None, &DeploymentPlan::default());
        assert!(none.iter().all(|o| !o.applicable));
    }

    #[test]
    fn anything_changed_owes_being_committed() {
        let v = vocabulary();
        assert!(
            implied(&v, &["apps/x.rs".into()], None, &DeploymentPlan::default())
                .iter()
                .find(|o| o.id == "commit")
                .unwrap()
                .applicable
        );
        assert!(
            !implied(&v, &[], None, &DeploymentPlan::default())
                .iter()
                .find(|o| o.id == "commit")
                .unwrap()
                .applicable
        );
    }

    #[test]
    fn a_known_failure_makes_the_task_unfinishable_and_silence_does_not() {
        let m = model();
        let v = vocabulary();
        let t = task(&["tests"]);
        let changed = vec!["apps/x.rs".to_string()];

        // nothing reported: the task is not refused, and the report says it is unverified
        let c = complete(
            &m,
            &changed,
            Some(&t),
            &v,
            &BTreeMap::new(),
            &hashes(&m, "aaaa"),
            &sources(),
            false,
            "now",
            vec![],
        );
        assert!(c.finishable, "absence is not a failure");
        assert!(c.unverified.contains(&"build".to_string()));
        assert!(c.findings.iter().any(|f| f.contains("never-reported")));

        // a failing run: refused, and it names the gate
        let mut runs = BTreeMap::new();
        runs.insert(
            "build".to_string(),
            GateRun {
                recorded_at: "t".into(),
                head: "abc".into(),
                branch: "b".into(),
                exit: 101,
                command: "cargo build".into(),
                inputs_hash: "aaaa".into(),
                session: String::new(),
            },
        );
        let c = complete(
            &m,
            &changed,
            Some(&t),
            &v,
            &runs,
            &hashes(&m, "aaaa"),
            &sources(),
            false,
            "now",
            vec![],
        );
        assert!(!c.finishable);
        assert_eq!(c.blocking, vec!["build".to_string()]);
        assert_eq!(c.tallies.get("fail"), Some(&1));
    }

    #[test]
    fn a_passing_run_stops_finishing_the_task_once_the_code_moves_underneath_it() {
        let m = model();
        let v = vocabulary();
        let t = task(&[]);
        let changed = vec!["apps/x.rs".to_string()];
        let mut runs = BTreeMap::new();
        runs.insert(
            "build".to_string(),
            GateRun {
                recorded_at: "t".into(),
                head: "abc".into(),
                branch: "b".into(),
                exit: 0,
                command: "cargo build".into(),
                inputs_hash: "aaaa".into(),
                session: String::new(),
            },
        );
        // over the tree it was taken on
        let ok = complete(
            &m,
            &changed,
            Some(&t),
            &v,
            &runs,
            &hashes(&m, "aaaa"),
            &sources(),
            false,
            "now",
            vec![],
        );
        assert!(ok.finishable && ok.blocking.is_empty());
        assert_eq!(ok.tallies.get("pass"), Some(&1));

        // one edit later, the same run proves nothing
        let moved = complete(
            &m,
            &changed,
            Some(&t),
            &v,
            &runs,
            &hashes(&m, "bbbb"),
            &sources(),
            false,
            "now",
            vec![],
        );
        assert!(
            !moved.finishable,
            "evidence that no longer describes the tree cannot finish a task"
        );
        assert_eq!(moved.blocking, vec!["build".to_string()]);
        assert_eq!(moved.tallies.get("stale"), Some(&1));
    }

    #[test]
    fn with_no_task_nothing_is_judged_and_nothing_passes() {
        let m = model();
        let v = vocabulary();
        let c = complete(
            &m,
            &["apps/x.rs".to_string()],
            None,
            &v,
            &BTreeMap::new(),
            &hashes(&m, "aaaa"),
            &sources(),
            false,
            "now",
            vec![],
        );
        assert!(!c.present);
        assert_eq!(c.tallies.get("pass"), None);
        assert!(c.unverified.contains(&"build".to_string()));
    }
}
