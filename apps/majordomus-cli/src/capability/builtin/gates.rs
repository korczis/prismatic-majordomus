//! The `gates` module: completion as a state the repository decides.
//!
//! Four capabilities: the model, the policy, and the completion they decide, which split the
//! way [`super::obligations`] splits, for the same reason; and the state of every declared
//! gate's last run, which answers the question none of them does.
//!
//! **The model is declared.** `.ai/repo/ci/gates.yaml` is this repository's CI model: every
//! validation gate, the job that runs it, the command it runs, and the path classes that
//! decide which change selects which gate. `gates.model` reads it and answers in any
//! checkout that has one, whether or not the lifecycle has ever run.
//!
//! **Done is declared once.** `share/completion.yaml` is the definition of done the tool
//! ships: the stages of the work in order, every question a task must answer, and the
//! source each answer is taken from. `gates.policy` serves it with what it resolves to here;
//! `gates.completion` asks exactly its questions and folds the answers into the stage a task
//! stands at.
//!
//! **The completion is local.** Whether *this* task may be called finished depends on the
//! task record and the ledger under `.ai/local/state/`, which is one checkout's own state
//! and is never tracked or published. `gates.completion` answers about the checkout the
//! process was started in and about no other.
//!
//! **The state is the gates'.** `gates.state` answers how the last recorded run of every
//! declared gate went — whichever task recorded it, over which tree, at which head — and
//! names the gates no run has ever reported, which is the case an absent reader hides. It
//! reads this checkout's record and nothing else, and it says so: the forge's CI runs are
//! listed as not consulted rather than left out silently.
//!
//! **It is read, never written.** `majordomus evidence --gate` records a run; this judges
//! the record. A second writer for the same ledger would be a second account of events,
//! which is what ADR 0030 refuses.
//!
//! **One judgement, and every surface reads it.** The shell tool's `check` and `finish`
//! call this executable rather than re-deriving the answer, the HTTP route serves the same
//! execution, MCP exposes the same tool, and the Cockpit renders the same document. That is
//! the property the module exists for: a worker cannot get a different verdict by asking a
//! different surface, and no surface can be persuaded that "done" is true because an agent
//! said so.
//!
//! ```
//! use majordomus_cli::capability::builtin::gates::module;
//!
//! // the whole module, as every projection reads it: four capabilities, none with a
//! // command-line arm of its own (`majordomus run <id>` reaches each, and `check` and
//! // `finish` are the command line's answer about completion)
//! let m = module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|c| c.capability.id.as_str()).collect();
//! assert_eq!(ids, ["gates.model", "gates.policy", "gates.completion", "gates.state"]);
//! for entry in &m.capabilities {
//!     assert!(entry.capability.exposure.cli.is_none());
//!     assert!(entry.capability.exposure.http.is_some());
//! }
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::gates::{self, judge, model, Completion, CompletionPolicy, GateStateReport};
use crate::{capability, module};

use super::get;
use super::obligations::Obligation;
use super::views::Empty;

/// The URI under which the CI model is read as an MCP resource.
pub const GATES_URI: &str = "majordomus://gates";

/// The URI under which this checkout's completion state is read as an MCP resource.
pub const COMPLETION_URI: &str = "majordomus://gates/completion";

/// The URI under which the completion policy is read as an MCP resource.
pub const POLICY_URI: &str = "majordomus://gates/policy";

/// The completion policy, as `gates.policy` serves it: the stages, the questions and the
/// source each is answered from, with what the policy resolves to in this repository.
/// The policy is flattened into the report, so a reader sees the keys the file carries
/// with the derived fields beside them.
///
/// ```
/// use majordomus_cli::capability::builtin::gates::CompletionPolicyReport;
/// use majordomus_cli::gates::CompletionPolicy;
///
/// let policy = CompletionPolicy::parse(
///     "version: 1\nstages:\n  - id: a\n    title: Alpha\n    summary: s\nquestions:\n  - id: q\n    stage: a\n    question: Is it?\n    source: gate:site-check\n    remediation: run it\n",
///     "share/completion.yaml",
/// )
/// .unwrap();
/// let report = CompletionPolicyReport {
///     outline: policy.outline(),
///     problems: policy.validate(&[]),
///     unanswered_gates: policy.unanswered_gates(&[]),
///     policy,
/// };
/// assert_eq!(report.outline, "- Alpha: q\n");
/// assert_eq!(report.unanswered_gates, ["q:site-check"]);
/// let json = serde_json::to_value(&report).unwrap();
/// assert_eq!(json["stages"][0]["id"], "a", "the policy's own keys, flattened");
/// assert!(json.get("problems").is_none(), "a coherent distribution writes no problems");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompletionPolicyReport {
    /// The policy as read from the distribution.
    #[serde(flatten)]
    pub policy: CompletionPolicy,
    /// The definition of done in a dozen lines: one per stage that asks anything, its
    /// questions after a colon.
    pub outline: String,
    /// What does not resolve against the vocabulary the policy is shipped beside: a token
    /// `share/obligations.yaml` lacks. Empty in a coherent distribution, and never silently so.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// The questions answered by a gate this repository's CI model does not declare, as
    /// `question:gate`. Not a problem of the policy — the model is the repository's — but a
    /// question this repository never gets a verdict for, listed so that it is seen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unanswered_gates: Vec<String>,
}

/// The URI under which the state of every declared gate is read as an MCP resource.
pub const STATE_URI: &str = "majordomus://gates/state";

// ---------------------------------------------------------------- output types

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The CI model this repository declares: what may refuse a change, and what selects each.
///
/// Two things about this report are worth knowing before reading one. `count` is measured
/// from `gates` rather than written down, so the two can never disagree the way a
/// hand-maintained total does. And `unreadable` is how a reader tells "this repository
/// declares no CI model" from "the call failed": a repository of the layer is not obliged
/// to declare gates, so the empty model is an answer, and it says why it is empty.
///
/// ```
/// use majordomus_cli::capability::builtin::gates::GateModelReport;
/// use serde_json::json;
///
/// // the shape every surface serves: `unreadable` is skipped when the model was read
/// let r: GateModelReport = serde_json::from_value(json!({
///     "version": 1,
///     "source": ".ai/repo/ci/gates.yaml",
///     "count": 0,
///     "on_demand": [],
///     "gates": [],
///     "classes": [],
/// })).unwrap();
/// assert_eq!(r.count, r.gates.len(), "the total is measured, never declared");
/// assert!(r.unreadable.is_none(), "read, and it declares nothing");
///
/// // and the other empty answer, which a reader must not confuse with that one
/// let absent: GateModelReport = serde_json::from_value(json!({
///     "version": 0,
///     "source": ".ai/repo/ci/gates.yaml",
///     "count": 0,
///     "on_demand": [],
///     "gates": [],
///     "classes": [],
///     "unreadable": "no such file",
/// })).unwrap();
/// assert_eq!(absent.unreadable.as_deref(), Some("no such file"));
/// ```
pub struct GateModelReport {
    /// The schema version the file states.
    pub version: u32,
    /// Where it was read from, repository-relative.
    pub source: String,
    /// How many gates it declares; measured, not written down.
    pub count: usize,
    /// The gates whose runner cannot be had on demand, so no routine plan selects them.
    /// A gate that never runs protects nothing, and this is where that is said out loud.
    pub on_demand: Vec<String>,
    /// Every gate, in declaration order, with the pathspecs its evidence is taken over.
    pub gates: Vec<GateModelEntry>,
    /// Every path class, in declaration order.
    pub classes: Vec<model::GateClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Why there is no model to report, when there is none. A repository of the layer does
    /// not have to declare a CI model, and a reader is entitled to the difference between
    /// "this repository declares no gates" and "this call failed".
    pub unreadable: Option<String>,
}

/// ```
/// use majordomus_cli::capability::builtin::gates::GateModelEntry;
/// use majordomus_cli::gates::GateDecl;
///
/// let decl: GateDecl = serde_json::from_value(serde_json::json!({
///     "id": "site-check",
///     "job": "site",
///     "runs": "scripts/site-check",
///     "summary": "the site is whole"
/// }))
/// .unwrap();
/// let entry = GateModelEntry { declared: decl, inputs: vec!["site/**".into()] };
/// // the declaration as the file carries it, plus the paths its evidence is taken over —
/// // derived from the classes that select it, never written down a second time
/// assert_eq!(entry.declared.id, "site-check");
/// assert_eq!(entry.inputs, ["site/**"]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One gate of the model, with what the model implies about it.
pub struct GateModelEntry {
    #[serde(flatten)]
    /// The declaration as the file carries it.
    pub declared: model::GateDecl,
    /// The pathspecs a run of this gate is evidence over, derived from the classes that
    /// select it. A change to any of them makes a recorded run stale.
    pub inputs: Vec<String>,
}

// ---------------------------------------------------------------- input

/// ```
/// use majordomus_cli::capability::builtin::gates::CompletionInput;
///
/// // the default asks about the active task's own change set
/// let mine = CompletionInput::default();
/// assert!(mine.changed.is_none() && mine.base.is_none());
///
/// // and a caller may name the change instead, which is how a test drives it
/// let named = CompletionInput {
///     changed: Some(vec!["docs/CLI.md".into()]),
///     ..Default::default()
/// };
/// assert_eq!(named.changed.unwrap(), ["docs/CLI.md"]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which change to judge completion over.
pub struct CompletionInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The changed paths, repository-relative. Absent means the task's own change set:
    /// everything between the commit the task started at and the working tree.
    pub changed: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit to compare with, when the task's own starting commit is not wanted.
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Also plan the gates the model marks on-demand. Off by default, because a plan that
    /// selects a gate whose runner is unavailable is a plan whose verdict never arrives.
    pub on_demand: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Plan at this tier of the model's `tiers` (ADR 0186): a gate whose lane the tier does
    /// not run is left out, and nothing implies or requires it back. Absent plans every lane.
    /// A model that declares no tiers, or a tier it does not declare, is refused as invalid
    /// input, in the words `scripts/ci-plan --tier` refuses it with.
    pub tier: Option<String>,
}

impl BenchmarkCases for CompletionInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("this-task", CompletionInput::default()),
            NamedCase::new(
                "one-source-file",
                CompletionInput {
                    changed: Some(vec!["apps/majordomus-cli/src/lib.rs".into()]),
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "a-document",
                CompletionInput {
                    changed: Some(vec!["docs/CLI.md".into()]),
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "everything-on-demand",
                CompletionInput {
                    changed: Some(vec![".ai/repo/ci/gates.yaml".into()]),
                    on_demand: Some(true),
                    ..Default::default()
                },
            ),
        ]
    }
}

// ---------------------------------------------------------------- handlers

/// The vocabulary the distribution ships, read the way every other reader locates it.
fn vocabulary(ctx: &Context) -> Result<Vec<Obligation>, String> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let share = crate::share::Share::locate(ctx.index.share.as_deref(), &root)
        .map_err(|e| format!("no distribution to read the obligation vocabulary from: {e}"))?;
    let path = share.dir().join("obligations.yaml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    #[derive(Deserialize)]
    struct File {
        obligations: Vec<Obligation>,
    }
    let file: File =
        crate::metadata::yaml::parse_into(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(file.obligations)
}

/// The completion policy the distribution ships, located the way the vocabulary is.
fn policy(ctx: &Context) -> Result<CompletionPolicy, String> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let share = crate::share::Share::locate(ctx.index.share.as_deref(), &root)
        .map_err(|e| format!("no distribution to read the completion policy from: {e}"))?;
    CompletionPolicy::load(share.dir())
}

/// The policy, resolved against the vocabulary beside it and this repository's CI model.
fn gates_policy(ctx: &Context, _: Empty) -> Result<CompletionPolicyReport, CapabilityError> {
    let policy = policy(ctx).map_err(CapabilityError::NotFound)?;
    let tokens: Vec<String> = vocabulary(ctx)
        .map(|v| v.into_iter().map(|o| o.id).collect())
        .unwrap_or_default();
    let root = PathBuf::from(&ctx.index.repository.root);
    let (m, _) = load_model(&root);
    let declared: Vec<String> = m.gates.iter().map(|g| g.id.clone()).collect();
    Ok(CompletionPolicyReport {
        outline: policy.outline(),
        problems: policy.validate(&tokens),
        unanswered_gates: policy.unanswered_gates(&declared),
        policy,
    })
}

/// The CI model, or an empty one and the reason there is none.
///
/// A repository of the layer does not have to declare gates, and every reader of this module
/// already says so in its own way: `lib/gates.sh` skips with "this repository declares no
/// readable CI model" rather than failing. Refusing the call instead would make a capability
/// that cannot answer about most repositories — and the generic tests that replay every
/// capability's benchmark cases against a fixture are exactly where that showed.
fn load_model(root: &Path) -> (model::GateModel, Option<String>) {
    match model::GateModel::load(root) {
        Ok(m) => (m, None),
        Err(reason) => (
            model::GateModel {
                version: 0,
                gates: Vec::new(),
                classes: Vec::new(),
                tiers: None,
            },
            Some(reason),
        ),
    }
}

fn gates_model(ctx: &Context, _: Empty) -> Result<GateModelReport, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let (m, unreadable) = load_model(&root);
    Ok(GateModelReport {
        version: m.version,
        source: model::MODEL_PATH.to_string(),
        count: m.gates.len(),
        on_demand: m
            .gates
            .iter()
            .filter(|g| g.on_demand)
            .map(|g| g.id.clone())
            .collect(),
        gates: m
            .gates
            .iter()
            .map(|g| GateModelEntry {
                inputs: m.inputs_of(&g.id),
                declared: g.clone(),
            })
            .collect(),
        classes: m.classes.clone(),
        unreadable,
    })
}

/// One hash per gate of `m`, over the files that select it, as they stand under `root`.
///
/// The gates overlap, so each file's digest is taken once for the whole report. A gate with
/// no inputs hashes to the empty string, which is how the ledger records it; `None` means
/// the hash could not be taken at all.
fn gate_hashes(root: &Path, m: &model::GateModel) -> BTreeMap<String, Option<String>> {
    let mut hashes: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut digests = super::obligations::FileHashes::default();
    for gate in &m.gates {
        let specs = m.inputs_of(&gate.id);
        let hash = if specs.is_empty() {
            Some(String::new())
        } else {
            super::obligations::inputs_hash_with(root, &specs, &mut digests).map(|(h, _)| h)
        };
        hashes.insert(gate.id.clone(), hash);
    }
    hashes
}

/// The commit `root` is at, or why git could not say.
fn head_of(root: &Path) -> Result<String, String> {
    let out = crate::git::read_only(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("git rev-parse HEAD: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git rev-parse HEAD exited {}",
            out.status.code().unwrap_or(-1)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn gates_state(ctx: &Context, _: Empty) -> Result<GateStateReport, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let mut findings: Vec<String> = Vec::new();

    let (m, unreadable) = load_model(&root);
    if let Some(reason) = unreadable {
        findings.push(format!(
            "this repository declares no readable CI model at {}, so there is no gate set to \
             report — an empty answer, never a passing one: {reason}",
            model::MODEL_PATH
        ));
    }

    let ledger = gates::ledger_path(&root);
    let ledger_rel = ledger
        .strip_prefix(&root)
        .unwrap_or(&ledger)
        .display()
        .to_string();
    let (runs, skipped) = judge::latest_runs(&ledger);
    if !ledger.is_file() {
        findings.push(format!(
            "no ledger at {ledger_rel}: no gate run has ever been recorded in this checkout, \
             so every gate is reported as never reported"
        ));
    } else if runs.is_empty() {
        findings.push(format!(
            "the ledger at {ledger_rel} holds no gate run: no gate run has ever been recorded \
             in this checkout, so every gate is reported as never reported"
        ));
    }
    if skipped > 0 {
        findings.push(format!(
            "{skipped} ledger line(s) could not be read and were skipped; run `majordomus doctor`"
        ));
    }

    let head = match head_of(&root) {
        Ok(h) => Some(h),
        Err(reason) => {
            findings.push(format!(
                "the current commit could not be read, so no run is said to be on it: {reason}"
            ));
            None
        }
    };

    let states = gates::state::state(&m, &runs, &gate_hashes(&root, &m), head.as_deref());
    Ok(gates::state::report(
        model::MODEL_PATH,
        &ledger_rel,
        head,
        states,
        findings,
        &crate::peers::rfc3339(std::time::SystemTime::now()),
    ))
}

fn gates_completion(ctx: &Context, input: CompletionInput) -> Result<Completion, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let mut findings: Vec<String> = Vec::new();

    let (m, unreadable) = load_model(&root);
    // the tier is resolved before anything is read or measured: a tier the model cannot plan
    // is the caller's mistake, refused as ci-plan refuses it, never a plan of every lane
    let tier = input
        .tier
        .as_deref()
        .map(|t| m.tier(t))
        .transpose()
        .map_err(CapabilityError::InvalidInput)?;
    if let Some(reason) = unreadable {
        // no model is not no answer: every gate is unknown, said out loud, and `finishable`
        // is left to the obligations rather than granted by a silence
        findings.push(format!(
            "this repository declares no readable CI model at {}, so no gate can be judged              here — unknown, never a pass: {reason}",
            model::MODEL_PATH
        ));
    }
    let task = super::continuity::read_task(&gates::task_path(&root));
    if task.is_none() {
        findings.push(format!(
            "no active task in this checkout ({}); the gates are reported against the \
             working tree and nothing is attributed to a task",
            gates::task_path(&root)
                .strip_prefix(&root)
                .unwrap_or(&gates::task_path(&root))
                .display()
        ));
    }

    // the change set: what was asked for, or the task's own — from its starting commit to
    // the working tree, the uncommitted half included
    let changed = match input.changed {
        Some(list) => list,
        None => {
            let base = input
                .base
                .clone()
                .or_else(|| task.as_ref().map(|t| t.head.clone()));
            match gates::changed_paths(&root, base.as_deref()) {
                Ok(paths) => paths,
                Err(reason) => {
                    findings.push(format!(
                        "the change set could not be read from git, so applicability is \
                         computed over nothing and every gate is reported as it stands: {reason}"
                    ));
                    Vec::new()
                }
            }
        }
    };

    let vocab = match vocabulary(ctx) {
        Ok(v) => v,
        Err(reason) => {
            findings.push(format!(
                "the obligation vocabulary could not be read, so no obligation can be \
                 implied from the change: {reason}"
            ));
            Vec::new()
        }
    };

    let (runs, skipped) = match &task {
        Some(t) => judge::runs_for(&gates::ledger_path(&root), &t.id),
        None => (BTreeMap::new(), 0),
    };
    if skipped > 0 {
        findings.push(format!(
            "{skipped} ledger line(s) could not be read and were skipped; run `majordomus doctor`"
        ));
    }

    // one hash per gate, over the files that select it. Taken here rather than inside the
    // judgement so that a test drives the same function with hashes of its own.
    let hashes = gate_hashes(&root, &m);

    // The obligation half of the done invariant is `obligations.closure`'s judgement, asked
    // for through the same executor MCP and the HTTP routes use, so this report cannot
    // disagree with `majordomus check` about whether an obligation stands. Nothing is
    // re-judged here; the state word comes back and is translated once.
    let mut standing: BTreeMap<String, gates::ObligationStanding> = BTreeMap::new();
    let mut closure_reachable = false;
    match ctx.execute("obligations.closure", serde_json::json!({})) {
        Ok(value) => match serde_json::from_value::<super::obligations::Closure>(value) {
            Ok(closure) => {
                closure_reachable = true;
                for o in closure.obligations {
                    standing.insert(
                        o.id.clone(),
                        gates::ObligationStanding {
                            state: o.state.as_str().to_string(),
                            detail: o.detail,
                            reproduce: o.reproduce,
                        },
                    );
                }
            }
            Err(e) => findings.push(format!(
                "obligations.closure answered something this report cannot read, so every \
                 obligation question is unknown rather than passing: {e}"
            )),
        },
        Err(e) => findings.push(format!(
            "obligations.closure could not be executed, so every obligation question is \
             unknown rather than passing: {e}"
        )),
    }

    // The topology half of the done invariant — "is a branch, worktree or pull request left
    // behind?" — is `convergence.report`'s verdict, asked through the same executor for the
    // same reason: one measurement of where this repository's work is held, not two.
    let convergence = match ctx.execute("convergence.report", serde_json::json!({})) {
        Ok(value) => match serde_json::from_value::<crate::convergence::ConvergenceReport>(value) {
            Ok(report) => Some(report),
            Err(e) => {
                findings.push(format!(
                    "convergence.report answered something this report cannot read, so whether \
                     work is left behind is unknown rather than passing: {e}"
                ));
                None
            }
        },
        Err(e) => {
            findings.push(format!(
                "convergence.report could not be executed, so whether work is left behind is \
                 unknown rather than passing: {e}"
            ));
            None
        }
    };

    // The closing half of the done invariant — did the recorded debt grow, is a branch or a
    // worktree created since the task started still here, is a pull request opened since
    // still open — is read from git and the recorded forge observation, measured from where
    // the task started. With no task there is no start, and the three stay unknown.
    let closing = task
        .as_ref()
        .map(|t| gates::closing::read(&root, &t.head, &t.started_at));

    // The questions themselves are the completion policy's. A distribution whose policy
    // cannot be read is not answered with no questions — that would read as nothing owed —
    // but with the one question of whether it can be, unknown, and nothing complete.
    let policy = match policy(ctx) {
        Ok(p) => Some(p),
        Err(reason) => {
            findings.push(format!(
                "the completion policy could not be read, so no question of the done \
                 invariant can be asked and nothing is complete: {reason}"
            ));
            None
        }
    };

    Ok(gates::complete(
        &m,
        &changed,
        task.as_ref(),
        &vocab,
        &runs,
        &hashes,
        &standing,
        closure_reachable,
        convergence.as_ref(),
        closing.as_ref(),
        input.on_demand.unwrap_or(false),
        tier.as_ref(),
        &crate::peers::rfc3339(std::time::SystemTime::now()),
        policy.as_ref(),
        findings,
    ))
}

// ---------------------------------------------------------------- the module

/// The `gates` module: the CI model every clone shares, and the completion state only this
/// checkout can answer.
///
/// Neither has a command-line projection of its own, for the reason [`super::obligations`]
/// states about the local half: `majordomus check` and `majordomus finish` are already the
/// command line's answer to this question, and they read this module rather than deriving a
/// second one. What is new is that they now have a gate half to read.
///
/// ```
/// use majordomus_cli::capability::builtin::gates::module;
/// let m = module();
/// assert_eq!(m.id.as_str(), "gates");
/// // every projection is derived from the declaration and written nowhere else
/// let routes: Vec<&str> = m
///     .capabilities
///     .iter()
///     .filter_map(|e| e.capability.exposure.http.as_ref().map(|h| h.path.as_str()))
///     .collect();
/// assert_eq!(routes, ["/api/v1/gates", "/api/v1/gates/policy", "/api/v1/gates/completion", "/api/v1/gates/state"]);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "gates",
        title: "Completion gates",
        description: "Completion as a state the repository decides rather than a claim a worker makes: the validation gates this repository declares in its CI model, which of them a task's own change set selects, what each one last reported, whether that verdict still describes the tree it was taken over, and therefore whether the task may be called finished. A gate that has never reported is `queued` and not `pass`, a run whose files have changed since is `stale` and refuses exactly as a failure does, and a gate the change cannot affect is `exempt` rather than unknown.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "gates.model",
                title: "The CI model",
                description: "Every validation gate this repository declares — the job that runs it, the command it runs, what it proves, whether every plan selects it and whether its runner can be had on demand at all — with the path classes that decide which change selects which gate, and, derived from those classes, the pathspecs each gate's evidence is taken over.",
                input: Empty,
                output: GateModelReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_gates".into()),
                        resource: Some(McpResource { uri: GATES_URI.into(), name: "gates".into() }),
                    }),
                    http: get("/api/v1/gates"),
                    cli: None,
                },
                tags: ["gates", "ci", "completion", "introspection"],
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: Some(5) },
                handler: gates_model,
            },
            capability! {
                id: "gates.policy",
                title: "The completion policy",
                description: "The one definition of done: the stages of the work in order, every question a task must answer before it may be called finished, the stage each question belongs to and the source its answer is taken from — an obligation of share/obligations.yaml, a gate of the CI model, the change set, the repository's convergence or the closing readings. Read from share/completion.yaml, the file `gates.completion` takes its questions from. A token the policy names and the vocabulary lacks is reported as a problem, and a gate question this repository's CI model never answers is listed by name.",
                input: Empty,
                output: CompletionPolicyReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_completion_policy".into()),
                        resource: Some(McpResource { uri: POLICY_URI.into(), name: "completion-policy".into() }),
                    }),
                    http: get("/api/v1/gates/policy"),
                    cli: None,
                },
                tags: ["gates", "completion", "policy", "introspection"],
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: Some(5) },
                handler: gates_policy,
            },
            capability! {
                id: "gates.completion",
                title: "Whether this task may be called finished",
                description: "The active task's own change set — from the commit it started at to the working tree, uncommitted files included — put through the CI model: which gates it selects and why, what each one last reported and over which files, which verdicts have gone stale because the tree moved underneath them, which required gates have never reported at all, and which obligations the change implies whether or not the task declared them. `finishable` is false only when a required gate is known to be failing, stale or blocked by one that is; absence of a verdict is reported as unverified rather than silently accepted. The done invariant is the questions of share/completion.yaml, in its order, each answered from the source it names; `stage` is the stage of the work those answers fold to, and `complete` is true only when every question passes or is exempt — finishable and complete are two different words.",
                input: CompletionInput,
                output: Completion,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_completion".into()),
                        resource: Some(McpResource { uri: COMPLETION_URI.into(), name: "completion".into() }),
                    }),
                    http: get("/api/v1/gates/completion"),
                    cli: None,
                },
                tags: ["gates", "completion", "tasks", "evidence"],
                cache: CachePolicy::Disabled,
                handler: gates_completion,
            },
            capability! {
                id: "gates.state",
                title: "How each gate last went",
                description: "Every gate the CI model declares with the standing of its last recorded run in this checkout, whichever task recorded it: `pass` or `fail` over the files as they stand now, `stale` when those files have changed since the run, `queued` when no run has ever reported — listed again under `never_reported`, because a missing verdict is the case an absent reader renders green — and `unknown` when a run exists and cannot be checked against this tree. Each run carries the commit it was taken at and whether that is the commit checked out now. Read offline from this checkout's own record; what it did not consult, the forge's CI runs, is named under `unreached` rather than left out.",
                input: Empty,
                output: GateStateReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_gate_state".into()),
                        resource: Some(McpResource { uri: STATE_URI.into(), name: "gate-state".into() }),
                    }),
                    http: get("/api/v1/gates/state"),
                    cli: None,
                },
                tags: ["gates", "ci", "evidence", "introspection"],
                cache: CachePolicy::Disabled,
                handler: gates_state,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_module_declares_what_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "gates");
        assert_eq!(m.capabilities.len(), 4);
        for e in &m.capabilities {
            assert_eq!(e.capability.id.namespace(), "gates");
            assert!(
                e.capability.exposure.mcp.is_some() && e.capability.exposure.http.is_some(),
                "{} must be readable on every surface without one recomputing it",
                e.capability.id
            );
        }
    }

    /// `gates.state` over a synthetic repository with no git: one gate reported by a task
    /// that is no longer open, one gate never reported. The reported one stands on its run,
    /// the other is `queued` and named as never reported, and what could not be had — the
    /// commit, the forge — is said rather than left out.
    #[test]
    fn the_gate_state_answers_offline_and_never_reported_is_not_passed() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = repo.root().to_path_buf();
        std::fs::create_dir_all(root.join(".ai/repo/ci")).unwrap();
        std::fs::write(
            root.join(model::MODEL_PATH),
            "version: 1\ngates:\n  - id: lint\n    job: structure\n    runs: \"true\"\n    \
             summary: lints\n  - id: docs\n    job: structure\n    runs: \"true\"\n    \
             summary: docs\nclasses: []\n",
        )
        .unwrap();
        let state = || -> GateStateReport {
            let index = repo.index().unwrap();
            let registry = crate::capability::CapabilityRegistry::builder()
                .with_modules(super::super::modules())
                .with_index(&index)
                .build()
                .expect("the registry builds");
            let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
            serde_json::from_value(
                ctx.execute("gates.state", serde_json::json!({}))
                    .expect("the gate state answers"),
            )
            .expect("a gate state report")
        };

        // nothing recorded at all: every gate never reported, and the missing ledger is said
        let none = state();
        assert_eq!(none.count, 2);
        assert_eq!(none.never_reported, ["lint", "docs"]);
        assert!(none
            .gates
            .iter()
            .all(|g| g.status == gates::GateStatus::Queued));
        assert!(
            none.findings.iter().any(|f| f.starts_with("no ledger at")),
            "{:?}",
            none.findings
        );
        assert!(none.head.is_none(), "no git here");
        assert!(none.unreached.iter().any(|u| u.contains("forge")));

        // a ledger that holds other events and no gate run says the same, in its own words
        let ledger = gates::ledger_path(&root);
        std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        std::fs::write(&ledger, "{\"event\":\"task.start\",\"task\":\"t-old\"}\n").unwrap();
        let empty = state();
        assert_eq!(empty.never_reported, ["lint", "docs"]);
        assert!(
            empty
                .findings
                .iter()
                .any(|f| f.contains("holds no gate run")),
            "{:?}",
            empty.findings
        );

        // a run recorded by a task that has since finished still is the gate's last word
        std::fs::write(
            &ledger,
            "{\"event\":\"task.gate\",\"task\":\"t-old\",\"gate\":\"lint\",\"exit\":0,\
             \"head\":\"abc\",\"ts\":\"2026-10-15T00:00:00Z\"}\n",
        )
        .unwrap();
        let some = state();
        assert_eq!(some.never_reported, ["docs"]);
        let lint = some.gates.iter().find(|g| g.id == "lint").unwrap();
        assert_eq!(lint.status, gates::GateStatus::Pass, "{}", lint.reason);
        assert!(
            lint.on_head.is_none(),
            "the commit could not be read, so no claim"
        );
        let docs = some.gates.iter().find(|g| g.id == "docs").unwrap();
        assert_eq!(docs.status, gates::GateStatus::Queued);
        assert_ne!(docs.status, lint.status, "never reported is not passed");
        assert_eq!(some.tallies.get("pass"), Some(&1));
        assert!(some
            .findings
            .iter()
            .any(|f| f.starts_with("the current commit could not be read")));
    }

    /// A repository with two gates: `lint` is `always` (any change invalidates it) and
    /// `docs` belongs to no class (nothing does).
    fn two_gate_repository() -> crate::synthetic::SyntheticRepository {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let path = repo.root().join(model::MODEL_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "version: 1\ngates:\n  - id: lint\n    job: structure\n    runs: \"true\"\n    \
             summary: lints\n    always: true\n  - id: docs\n    job: structure\n    \
             runs: \"true\"\n    summary: docs\nclasses: []\n",
        )
        .unwrap();
        repo
    }

    #[test]
    fn a_gate_with_inputs_is_hashed_from_them_and_one_without_hashes_to_nothing() {
        let repo = two_gate_repository();
        let root = repo.root().to_path_buf();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .expect("git");
            assert!(out.status.success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["add", "-A"]);
        let (m, unreadable) = load_model(&root);
        assert_eq!(unreadable, None);
        let hashes = gate_hashes(&root, &m);
        assert_eq!(
            hashes["docs"],
            Some(String::new()),
            "no inputs, the empty hash"
        );
        let lint = hashes["lint"].clone().expect("an always gate is hashed");
        assert!(!lint.is_empty(), "the tracked files were hashed");
    }

    #[test]
    fn a_ledger_line_that_cannot_be_read_is_counted_and_said_by_the_gate_state() {
        let repo = two_gate_repository();
        let ledger = gates::ledger_path(repo.root());
        std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        std::fs::write(&ledger, "this is not json\n{\"event\":\"task.start\"}\n").unwrap();
        let index = repo.index().unwrap();
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(super::super::modules())
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        let report: GateStateReport = serde_json::from_value(
            ctx.execute("gates.state", serde_json::json!({}))
                .expect("the gate state answers"),
        )
        .expect("a gate state report");
        assert!(
            report.findings.iter().any(|f| f
                == "1 ledger line(s) could not be read and were skipped; run `majordomus doctor`"),
            "{:?}",
            report.findings
        );
    }

    #[test]
    fn the_completion_answer_is_never_cached() {
        // a completion state cached for even a few seconds is a state that can say
        // `finishable` about a tree that has already changed, which is the defect
        let m = module();
        let c = m
            .capabilities
            .iter()
            .find(|e| e.capability.id.as_str() == "gates.completion")
            .expect("declared");
        assert!(matches!(c.capability.cache, CachePolicy::Disabled));
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct NotAVerdict {
        converged: String,
    }

    fn not_a_verdict(_: &Context, _: super::super::Empty) -> Result<NotAVerdict, CapabilityError> {
        Ok(NotAVerdict {
            converged: "perhaps".into(),
        })
    }

    /// The completion report over `modules`, asked of a synthetic repository with no git.
    fn completion_with(
        modules: Vec<crate::capability::module::ModuleDescriptor>,
    ) -> crate::gates::Completion {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let mut index = repo.index().unwrap();
        // the completion policy is the distribution's, and a synthetic repository carries no
        // share of its own: the crate's is the one this build ships
        index.share = Some(crate::synthetic::crate_share());
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(modules)
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        let value = ctx
            .execute("gates.completion", serde_json::json!({ "changed": [] }))
            .expect("the completion report answers");
        serde_json::from_value(value).expect("a completion report")
    }

    /// With a task record there is a start to measure from, so the closing readings are taken
    /// and reach the report. Here there is no git, so the debt cannot be compared — and the
    /// question says that, where before the task existed it said there was no start.
    #[test]
    fn a_task_brings_the_closing_readings_into_the_report() {
        let debt_of = |c: &crate::gates::Completion| {
            c.questions
                .iter()
                .find(|q| q.id == "no-new-debt")
                .expect("the invariant asks it")
                .clone()
        };
        let without = debt_of(&completion_with(super::super::modules()));
        assert_eq!(without.status, crate::gates::GateStatus::Unknown);
        assert!(
            without.evidence.contains("no task record"),
            "{}",
            without.evidence
        );

        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let record = crate::gates::task_path(repo.root());
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        std::fs::write(
            &record,
            "id: t-1\ntask: \"closing\"\nprofile: implementation\noutcome: active\n\
             started_at: 2026-10-10T12:00:00Z\nhead: 0123456789012345678901234567890123456789\n",
        )
        .unwrap();
        let mut index = repo.index().unwrap();
        index.share = Some(crate::synthetic::crate_share());
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(super::super::modules())
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        let value = ctx
            .execute("gates.completion", serde_json::json!({ "changed": [] }))
            .expect("the completion report answers");
        let with: crate::gates::Completion = serde_json::from_value(value).unwrap();
        assert!(with.present, "the task record was read");
        let debt = debt_of(&with);
        assert_eq!(
            debt.status,
            crate::gates::GateStatus::Unknown,
            "unread is never a pass"
        );
        assert!(
            debt.evidence.contains("could not be compared"),
            "{}",
            debt.evidence
        );
    }

    /// Whether work is left behind is `convergence.report`'s answer. When that answer cannot
    /// be had — the capability refused, or it answered in a shape this report cannot read —
    /// the question is unknown and a finding says which, never a pass.
    #[test]
    fn a_convergence_answer_that_cannot_be_had_or_read_is_unknown_and_says_why() {
        let stale = |c: &crate::gates::Completion| {
            c.questions
                .iter()
                .find(|q| q.id == "no-stale-topology")
                .expect("the invariant asks it")
                .status
        };

        // no git here, so the real capability refuses
        let refused = completion_with(super::super::modules());
        assert!(
            refused
                .findings
                .iter()
                .any(|f| f.starts_with("convergence.report could not be executed")),
            "{:?}",
            refused.findings
        );
        assert_eq!(stale(&refused), crate::gates::GateStatus::Unknown);

        let impostor = crate::module! {
            id: "convergence",
            title: "Convergence",
            description: "Answers in a shape the completion report cannot read.",
            stability: Stability::Experimental,
            capabilities: [
                crate::capability! {
                    id: "convergence.report", title: "Not a verdict",
                    description: "Something else.",
                    input: super::super::Empty, output: NotAVerdict,
                    stability: Stability::Experimental,
                    exposure: crate::capability::model::Exposure::default(), tags: [],
                    handler: not_a_verdict,
                },
            ],
        };
        let mut modules: Vec<_> = super::super::modules()
            .into_iter()
            .filter(|m| m.id.as_str() != "convergence")
            .collect();
        modules.push(impostor);
        let unreadable = completion_with(modules);
        assert!(
            unreadable
                .findings
                .iter()
                .any(|f| f
                    .starts_with("convergence.report answered something this report cannot read")),
            "{:?}",
            unreadable.findings
        );
        assert_eq!(stale(&unreadable), crate::gates::GateStatus::Unknown);
    }

    /// A tier the model cannot plan is the caller's mistake, refused before anything is read,
    /// never answered as a plan of every lane. The synthetic repository declares no CI model,
    /// so it declares no tiers either, and any tier asked of it is refused in ci-plan's words.
    #[test]
    fn a_tier_the_model_cannot_plan_is_refused_not_answered() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let index = repo.index().unwrap();
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(super::super::modules())
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        let err = ctx
            .execute(
                "gates.completion",
                serde_json::json!({ "changed": [], "tier": "pull-request" }),
            )
            .expect_err("a tier the model does not declare is refused");
        let said = format!("{err:?}");
        assert!(said.contains("tier pull-request"), "{said}");
    }

    // ------------------------------------------------------------ the policy

    /// A context over `repo` whose distribution is `share`.
    fn context_over(
        repo: &crate::synthetic::SyntheticRepository,
        share: &Path,
    ) -> std::sync::Arc<Context> {
        let mut index = repo.index().unwrap();
        index.share = Some(share.to_path_buf());
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(super::super::modules())
            .with_index(&index)
            .build()
            .expect("the registry builds");
        std::sync::Arc::new(Context::new(
            std::sync::Arc::new(index),
            std::sync::Arc::new(registry),
        ))
    }

    /// The report resolves the shipped policy against the vocabulary beside it and against
    /// this repository's own model: a gate question the model does not declare is listed,
    /// not counted as a problem.
    #[test]
    fn the_policy_report_names_the_gate_questions_this_repository_never_asks() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let path = repo.root().join(model::MODEL_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            "version: 1\ngates:\n  - id: release-check\n    job: structure\n    runs: \"true\"\n    \
             summary: the release record is current\nclasses: []\n",
        )
        .unwrap();
        let ctx = context_over(&repo, &crate::synthetic::crate_share());
        let report = gates_policy(&ctx, Empty {}).expect("the shipped policy is read");
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(report.unanswered_gates, ["version:version-surface"]);
        assert_eq!(report.outline, report.policy.outline());
        assert_eq!(report.policy.source, "share/completion.yaml");

        // and the completion report over the same repository asks exactly those questions
        let value = ctx
            .execute("gates.completion", serde_json::json!({ "changed": [] }))
            .expect("the completion report answers");
        let c: crate::gates::Completion = serde_json::from_value(value).unwrap();
        let asked: Vec<&str> = c.questions.iter().map(|q| q.id.as_str()).collect();
        let declared: Vec<&str> = report
            .policy
            .questions
            .iter()
            .map(|q| q.id.as_str())
            .collect();
        assert_eq!(asked, declared);
        assert_eq!(c.policy.as_deref(), Some("share/completion.yaml"));
        assert!(!c.complete, "no task is no completion");
    }

    /// A token the policy names and the vocabulary beside it lacks is a problem of the
    /// distribution, reported by question.
    #[test]
    fn a_policy_naming_a_token_the_vocabulary_lacks_reports_the_question() {
        let share = tempfile::tempdir().unwrap();
        for f in ["kinds.yaml", "obligations.yaml"] {
            std::fs::copy(
                crate::synthetic::crate_share().join(f),
                share.path().join(f),
            )
            .unwrap();
        }
        let text = std::fs::read_to_string(
            crate::synthetic::crate_share().join(crate::gates::POLICY_FILE),
        )
        .unwrap()
        .replace(
            "source: obligation:pages",
            "source: obligation:no-such-token",
        );
        std::fs::write(share.path().join(crate::gates::POLICY_FILE), text).unwrap();
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = context_over(&repo, share.path());
        let report = gates_policy(&ctx, Empty {}).expect("the policy parses");
        assert_eq!(
            report.problems,
            [
                "question 'published' is answered by obligation 'no-such-token', which \
              share/obligations.yaml does not declare"
            ]
        );
    }

    /// A distribution that ships no definition of done cannot be asked what done is, and the
    /// completion report over it asks only whether it can be — never nothing.
    #[test]
    fn a_distribution_without_a_completion_policy_is_refused_and_completes_nothing() {
        let refusal = |r: Result<CompletionPolicyReport, CapabilityError>| match r {
            Err(CapabilityError::NotFound(why)) => why,
            other => panic!("expected a refusal, and it answered {other:?}"),
        };
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();

        let no_policy = tempfile::tempdir().unwrap();
        std::fs::copy(
            crate::synthetic::crate_share().join("kinds.yaml"),
            no_policy.path().join("kinds.yaml"),
        )
        .unwrap();
        let ctx = context_over(&repo, no_policy.path());
        let why = refusal(gates_policy(&ctx, Empty {}));
        assert!(why.contains("completion.yaml"), "{why}");
        let c = gates_completion(
            &ctx,
            CompletionInput {
                changed: Some(vec![]),
                ..Default::default()
            },
        )
        .expect("the completion report still answers");
        assert_eq!(c.policy, None);
        assert_eq!(
            c.questions
                .iter()
                .map(|q| q.id.as_str())
                .collect::<Vec<_>>(),
            ["policy"]
        );
        assert!(!c.complete);
        assert!(
            c.findings
                .iter()
                .any(|f| f.starts_with("the completion policy could not be read")),
            "{:?}",
            c.findings
        );

        let no_distribution = tempfile::tempdir().unwrap();
        let ctx = context_over(&repo, no_distribution.path());
        let why = refusal(gates_policy(&ctx, Empty {}));
        assert!(
            why.starts_with("no distribution to read the completion policy from"),
            "{why}"
        );
    }
}
