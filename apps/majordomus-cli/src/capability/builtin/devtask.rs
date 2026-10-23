//! The `devtask` module: an issue and a milestone read as executable development scopes.
//!
//! # Why this is a module and not two more `plan.*` capabilities
//!
//! Every capability is composed into exactly one Rust module, and a module's id is the
//! namespace of everything in it, so `plan.task` would have to live inside
//! `builtin/plan.rs`. It does not belong there: `plan` is the projection of
//! [`crate::plan`], which is held to byte equality with `lib/project.awk` by
//! `test/cases/99_plan_capabilities.sh`, and a capability that joins the plan to git and to
//! the session records has no counterpart in the awk to be equal to. Keeping them apart
//! keeps that equality checkable.
//!
//! What it must not be is a second project model, and it is not:
//!
//! * the records are the canonical ones, read through the index like every other object;
//! * the status, wave, dependents, edges and findings are [`crate::plan`]'s, unchanged —
//!   `the_status_never_disagrees_with_the_plan` is the assertion;
//! * the branches and commits are `worktree::trace`'s, through the same [`Tracer`]
//!   `trace.issue` uses;
//! * the readiness vocabulary is a projection of the canonical statuses and refines three
//!   of them without replacing any — [`crate::devtask::readiness`] states which and why;
//! * the external projection stays `scripts/github-sync`'s. Nothing here writes, and
//!   nothing here restates its reconciliation vocabulary.
//!
//! [`Tracer`]: crate::worktree::trace::Tracer
//!
//! # Why the milestone graph does not touch git
//!
//! `devtask.milestone` is a pure function of the index, and deliberately: a graph whose
//! ordering depended on `git for-each-ref` would be a graph that differs between a full
//! clone and a shallow one, and this repository has already been bitten by a derivation
//! that was machine-dependent. `devtask.issue` consults git because a single issue's
//! branches are the question being asked; the graph does not, because they are not.
//!
//! # `devtask.implement` composes, and writes only through what it composes
//!
//! Taking an issue was a sequence a session ran by hand — is it ready, does it serve an
//! intent, is anybody on it, make the worktree, start — and two sessions could follow it at
//! once (I4003, Mission Control B4). `devtask.implement` is that sequence as one request, and
//! every step of it is a capability that already exists, called in-process through the
//! registry: readiness is `devtask.issue`'s verdict, the intent is `intents.preflight`'s, the
//! claim is the one `executions.start` takes for an issue (I4004) — asked first, so that an
//! overlap refuses before anything exists — the worktree is `worktree.create`'s (I4002), and
//! the run is `executions.start`'s, under an idempotency key (I4000). Nothing here writes a
//! file of its own, keeps a store or schedules anything.
//!
//! ```
//! use majordomus_cli::capability::builtin::devtask;
//! let ids: Vec<String> = devtask::module().capabilities.iter()
//!     .map(|e| e.capability.id.as_str().to_string()).collect();
//! assert_eq!(ids, ["devtask.issue", "devtask.milestone", "devtask.implement", devtask::RUN]);
//! // a double click is one implementation: the default key is the issue's
//! assert_eq!(devtask::default_key("I4003"), "implement-I4003");
//! ```
//!
//! ```text
//!   key ── a repeated request ──────────────────────────────► deduplicated (that execution)
//!    │
//!   readiness ── not startable ────────────────────────────► refused not_ready
//!   preflight ── refused ──────────────────────────────────► refused invalid_intent
//!   claim ────── a live exclusive claim meets the scope ───► refused claim_conflict
//!   worker ───── no attached peer asked ───────────────────► pending (nothing created)
//!   worktree.create ── feature/<id>-<slug>
//!   executions.start ── devtask.run, for the issue, under the key ──► started
//! ```
//!
//! The run, `devtask.run`, is handed to the worker that asked — the attached MCP peer whose
//! call this is — and lasts as long as that peer stays attached, within a deadline: work is
//! never pushed to a peer that did not ask for it, which is why a request over plain HTTP or
//! the command line answers `pending`.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CapabilityKind, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::devtask::task::{DevTaskModel, RecordRef, SessionRef};
use crate::devtask::{DevTask, MilestoneGraph};
use crate::execution::IdempotencyKey;
use crate::index::Index;
use crate::intent::{IntentPreflight, IntentPreflightVerdict};
use crate::plan::{Plan, ISSUE, MILESTONE};
use crate::worktree::trace::{IssueTrace, Tracer};
use crate::worktree::WorktreeError;
use crate::{capability, module};

use super::executions::{ExecutionLinks, ExecutionView, StartedExecution};
use super::{get, mcp, post};

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which issue to read as a development task.
pub struct DevTaskInput {
    /// The issue id, as the canonical model spells it (`I0901`). An id the model does not
    /// declare is answered with `declared: false` and every canonical field `unknown`,
    /// never refused and never answered with empty strings that read as authored.
    pub issue: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Consult git for the branches and commits that realised it. Default: yes. Set it to
    /// false for a deterministic answer that depends on the records alone — which is what a
    /// generated document or a comparison across machines wants.
    pub git: Option<bool>,
}

impl BenchmarkCases for DevTaskInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // An issue of the repository the benchmark runs against, so the case measures the
        // derivation rather than the refusal. A hard-coded id would be a second declaration
        // of this repository's own plan.
        let issue = first(ctx.index, ISSUE).unwrap_or_else(|| "I0001".to_string());
        vec![
            NamedCase::new(
                "records-only",
                DevTaskInput {
                    issue: issue.clone(),
                    git: Some(false),
                },
            ),
            NamedCase::new(
                "with-git",
                DevTaskInput {
                    issue,
                    git: Some(true),
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which milestone to read as a dependency graph.
pub struct DevMilestoneInput {
    /// The milestone id, as the canonical model spells it. An id the model does not declare
    /// is answered with an empty graph and every field `unknown`, not refused.
    pub milestone: String,
}

impl BenchmarkCases for DevMilestoneInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let milestone = first(ctx.index, MILESTONE).unwrap_or_else(|| "M000".to_string());
        vec![NamedCase::new(
            "first-milestone",
            DevMilestoneInput { milestone },
        )]
    }
}

// ---------------------------------------------------------------- what the index holds

/// The first object of a kind, in URI order. A benchmark case's input, and nothing else: a
/// capability never picks an object for a caller.
fn first(index: &Index, kind: &str) -> Option<String> {
    index
        .objects
        .iter()
        .find(|o| o.kind == kind)
        .map(|o| o.identity.clone())
}

/// One canonical record of a kind, by identity: its repository-relative path and its parsed
/// metadata. The index has already discovered, parsed and schema-validated it, so this is
/// the model itself rather than a second reading of the directory it lives in.
fn record<'a>(index: &'a Index, kind: &str, identity: &str) -> Option<(&'a str, &'a Value)> {
    index
        .objects
        .iter()
        .find(|o| o.kind == kind && o.identity == identity)
        .map(|o| (o.provenance.path.as_str(), &o.metadata))
}

/// Every issue id the canonical model declares, in id order. What the tracer needs in order
/// to read an issue out of a branch name.
fn issue_ids(index: &Index) -> Vec<String> {
    let mut ids: Vec<String> = index
        .objects
        .iter()
        .filter(|o| o.kind == ISSUE)
        .map(|o| o.identity.clone())
        .collect();
    crate::order::canonical(&mut ids);
    ids
}

/// The session records that reach this issue, each carrying how strongly.
///
/// Two relations, and they are deliberately not merged. A record's own `issues` key is what
/// `share/schemas/majordomus/session-record` calls "the issues it moved, by id — derived
/// from the ledger's own events for this episode; never authored": a canonical link. Its
/// branch name is a rule about a string — [`crate::worktree::state::issue_of`]'s, not
/// restated here — which a rename can break. The first is derived, the second inferred, and
/// [`SessionRef::declares_issue`] is which, so that the field carrying each can state its
/// own provenance instead of one of them borrowing the other's.
fn sessions_of(index: &Index, issue: &str) -> Vec<SessionRef> {
    let ids = vec![issue.to_string()];
    let mut out: Vec<SessionRef> = index
        .objects
        .iter()
        .filter(|o| o.kind == "session")
        .filter_map(|o| {
            let branch = o
                .metadata
                .get("branch")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let declares_issue = o
                .metadata
                .get("issues")
                .and_then(Value::as_array)
                .is_some_and(|items| items.iter().any(|i| i.as_str() == Some(issue)));
            // a record that neither declares the issue nor carries a branch naming it is
            // not this issue's session, and is left out rather than reported weakly
            if !declares_issue && crate::worktree::state::issue_of(branch, &ids).is_none() {
                return None;
            }
            Some(SessionRef {
                session_id: o.identity.clone(),
                branch: branch.to_string(),
                created_at: o
                    .metadata
                    .get("created_at")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                path: o.provenance.path.clone(),
                declares_issue,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        crate::order::natural_cmp(&a.created_at, &b.created_at)
            .then_with(|| crate::order::natural_cmp(&a.session_id, &b.session_id))
    });
    out
}

/// The repository the external projection targets, as `project.yaml` declares it.
fn repository_of(plan: &Plan) -> Option<&str> {
    Some(plan.project.repository.as_str()).filter(|r| !r.is_empty())
}

fn refused(e: WorktreeError) -> CapabilityError {
    match e.exit_code() {
        crate::worktree::EXIT_MISSING => CapabilityError::NotFound(e.to_string()),
        crate::worktree::EXIT_REFUSED => CapabilityError::Refused(e.to_string()),
        _ => CapabilityError::Internal(e.to_string()),
    }
}

/// What git says about one issue, or nothing when git could not be opened at all.
///
/// A repository this process cannot read as a git repository is not an error here: the
/// canonical half of the answer is still the whole canonical half, and the execution group
/// reports `git_consulted: false` rather than an empty list that would read as "nothing
/// realised this issue".
fn trace_of(ctx: &Context, issue: &str) -> Result<Option<IssueTrace>, CapabilityError> {
    let tracer = match Tracer::open(Path::new(&ctx.index.repository.root)) {
        Ok(t) => t.with_issues(issue_ids(&ctx.index)),
        Err(e) if e.exit_code() == crate::worktree::EXIT_MISSING => return Ok(None),
        Err(e) => return Err(refused(e)),
    };
    tracer.trace(issue).map(Some).map_err(refused)
}

// ---------------------------------------------------------------- handlers

fn devtask_issue(ctx: &Context, input: DevTaskInput) -> Result<DevTask, CapabilityError> {
    let plan = Plan::build(&ctx.index);
    let trace = if input.git.unwrap_or(true) {
        trace_of(ctx, &input.issue)?
    } else {
        None
    };
    let record = record(&ctx.index, ISSUE, &input.issue);
    let model = DevTaskModel {
        plan: &plan,
        record: record.map(|(path, metadata)| RecordRef { path, metadata }),
        trace: trace.as_ref(),
        sessions: sessions_of(&ctx.index, &input.issue),
        repository: repository_of(&plan),
    };
    Ok(DevTask::build(&model, &input.issue))
}

fn devtask_milestone(
    ctx: &Context,
    input: DevMilestoneInput,
) -> Result<MilestoneGraph, CapabilityError> {
    let plan = Plan::build(&ctx.index);
    let record = record(&ctx.index, MILESTONE, &input.milestone)
        .map(|(path, metadata)| RecordRef { path, metadata });
    Ok(MilestoneGraph::build(&plan, &input.milestone, record))
}

// ---------------------------------------------------------------- devtask.implement (I4003)

/// The capability `devtask.implement` starts as the one execution of an implementation: the
/// worker's run on the issue's worktree, under the issue's mesh claim.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::{module, RUN};
/// assert!(module().capabilities.iter().any(|e| e.capability.id.as_str() == RUN));
/// ```
pub const RUN: &str = "devtask.run";

/// How long a worker's run may last when the request names nothing: one hour.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::{DEFAULT_DEADLINE_MINUTES, MAX_DEADLINE_MINUTES};
/// assert!(DEFAULT_DEADLINE_MINUTES <= MAX_DEADLINE_MINUTES);
/// ```
pub const DEFAULT_DEADLINE_MINUTES: u64 = 60;

/// The longest a worker's run may last: a day. Every wait has a deadline.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::MAX_DEADLINE_MINUTES;
/// assert_eq!(MAX_DEADLINE_MINUTES, 1440);
/// ```
pub const MAX_DEADLINE_MINUTES: u64 = 24 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which issue to implement, and how a repeated request is recognised.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::ImplementInput;
/// let input: ImplementInput = serde_json::from_str(r#"{"issue":"I4003"}"#).unwrap();
/// assert_eq!((input.issue.as_str(), input.idempotency_key), ("I4003", None));
/// assert!(serde_json::from_str::<ImplementInput>(r#"{"issue":"I1","worktree":"/x"}"#).is_err(),
///     "the worktree is derived from the issue, never given");
/// ```
pub struct ImplementInput {
    /// The issue to implement, as the canonical model spells it (`I4003`).
    pub issue: String,
    /// What makes a repeated request one execution (I4000): sent again, it is answered the
    /// execution it started, as that execution stands now, and nothing new is created.
    /// Default: `implement-<issue>`, so that a double click is one implementation; send a
    /// new key to start the issue again after its execution ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 128), regex(pattern = r"^[A-Za-z0-9._:-]+$"))]
    pub idempotency_key: Option<String>,
    /// How many minutes the worker's run may last before it fails on its deadline; 60 when
    /// absent, a day at most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_minutes: Option<u64>,
}

impl BenchmarkCases for ImplementInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let issue = first(ctx.index, ISSUE).unwrap_or_else(|| "I0001".to_string());
        vec![NamedCase::new(
            "an-issue",
            ImplementInput {
                issue,
                idempotency_key: None,
                deadline_minutes: None,
            },
        )]
    }
}

/// Where a request to implement an issue stands once it is answered.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::ImplementStanding;
/// let words: Vec<String> = [ImplementStanding::Started, ImplementStanding::Deduplicated,
///     ImplementStanding::Pending].iter()
///     .map(|s| serde_json::to_value(s).unwrap().as_str().unwrap().to_string()).collect();
/// assert_eq!(words, ["started", "deduplicated", "pending"]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImplementStanding {
    /// This request created the worktree when it was missing and started the execution.
    Started,
    /// The key had already started an execution for this issue; that execution is the
    /// answer, as it stands now, and nothing was created.
    Deduplicated,
    /// Everything that could be judged was judged and passed, but no worker is attached to
    /// take the run: nothing was created, and the request may be sent again by a worker.
    Pending,
}

/// One step of the sequence and how it came out.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::ImplementStep;
/// let step = ImplementStep { step: "worker".into(), ok: false, detail: "nobody asked".into() };
/// assert_eq!(serde_json::to_value(&step).unwrap()["step"], "worker");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImplementStep {
    /// `readiness`, `preflight`, `claim`, `worker`, `worktree` or `execution`, in that
    /// order; a step that was not reached is absent.
    pub step: String,
    /// Whether it passed.
    pub ok: bool,
    /// What it found, in words.
    pub detail: String,
}

/// The answer of `devtask.implement`: where the request stands, every step it took, and —
/// when it started or found one — the execution with the links to follow it.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::{ImplementAnswer, ImplementStanding};
/// let pending = ImplementAnswer {
///     issue: "I4003".into(), standing: ImplementStanding::Pending,
///     idempotency_key: "implement-I4003".into(), branch: "feature/I4003-x".into(),
///     steps: vec![], pending: Some("no worker is attached".into()),
///     worker: None, worktree: None, execution: None,
/// };
/// let json = serde_json::to_value(&pending).unwrap();
/// assert_eq!(json["standing"], "pending");
/// assert!(json.get("execution").is_none(), "nothing was started, and nothing says so");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImplementAnswer {
    /// The issue.
    pub issue: String,
    /// `started`, `deduplicated` or `pending`. A refusal is not an answer: it is an error
    /// naming its cause (`not_ready`, `invalid_intent`, `claim_conflict`).
    pub standing: ImplementStanding,
    /// The key the execution is held under.
    pub idempotency_key: String,
    /// The branch the work is on: `feature/<id>-<slug>`.
    pub branch: String,
    /// The steps, in the order they ran.
    pub steps: Vec<ImplementStep>,
    /// Why the request is pending; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<String>,
    /// The worker the run was handed to: the attached peer that asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    /// The worktree the run works in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// The execution: the worker's run, holding the issue's mesh claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<StartedExecution>,
}

/// The input of `devtask.run`, which only `devtask.implement` composes: everything about the
/// run is already decided when it starts.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::RunInput;
/// let input: RunInput = serde_json::from_value(serde_json::json!({
///     "issue": "I4003", "branch": "feature/I4003-x", "worktree": "/wt", "worker": "p3",
///     "worker_attached_at": "2026-10-23T08:00:00Z", "deadline_minutes": 60,
/// })).unwrap();
/// assert_eq!(input.worker, "p3");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunInput {
    /// The issue.
    pub issue: String,
    /// The branch the work is on.
    pub branch: String,
    /// The worktree the worker works in.
    pub worktree: String,
    /// The worker: the board's id of the attached peer the run was handed to (`p3`).
    pub worker: String,
    /// When that peer attached, RFC 3339: with the id, which peer it is, since a position id
    /// is reused by the next client to connect after it.
    pub worker_attached_at: String,
    /// The run's deadline, in minutes.
    pub deadline_minutes: u64,
}

impl BenchmarkCases for RunInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // waived: a run lasts as long as a worker, and outside an execution it is refused
        Vec::new()
    }
}

/// How a worker's run ended: the issue, the worker and the worktree it was handed, why it
/// ended and how long it lasted — the output of a `devtask.run` execution that succeeded.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::RunReport;
/// let r = RunReport { issue: "I1".into(), worker: "p1".into(), worktree: "/wt".into(),
///     ended: "p1 detached".into(), elapsed_ms: 5 };
/// assert_eq!(serde_json::to_value(&r).unwrap()["ended"], "p1 detached");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunReport {
    /// The issue.
    pub issue: String,
    /// The worker.
    pub worker: String,
    /// The worktree it worked in.
    pub worktree: String,
    /// How the run ended, in words. A run ends when its worker detaches; what it did is in
    /// the worktree, and is verified by what verifies a branch, never by this.
    pub ended: String,
    /// How long it lasted, in milliseconds.
    pub elapsed_ms: u64,
}

/// The default key of a request for `issue`: `implement-<issue>`, with every character a key
/// may not hold (an issue may be `#184`) made a `-`.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::default_key;
/// assert_eq!(default_key("I4003"), "implement-I4003");
/// assert_eq!(default_key("#184"), "implement--184");
/// ```
pub fn default_key(issue: &str) -> String {
    let safe: String = issue
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("implement-{safe}")
}

/// The branch an issue's work happens on: `feature/<id>-<slug>`, the form `worktree create
/// --issue` names and the topology reads the issue back from; the id lowercased when the
/// record has no slug.
///
/// ```
/// use majordomus_cli::capability::builtin::devtask::branch_of;
/// assert_eq!(branch_of("I4003", "an-implement-request-runs-once"),
///            "feature/I4003-an-implement-request-runs-once");
/// assert_eq!(branch_of("I7", ""), "feature/I7-i7");
/// ```
pub fn branch_of(issue: &str, slug: &str) -> String {
    let slug = if slug.trim().is_empty() {
        issue.to_lowercase()
    } else {
        slug.trim().to_string()
    };
    format!("feature/{issue}-{slug}")
}

/// The attached peer that asked, or why there is none: the worker a run is handed to. Work is
/// never pushed to a peer that did not ask for it.
fn worker_of(ctx: &Context) -> Result<crate::peers::Peer, String> {
    let Some(caller) = &ctx.caller else {
        return Err("no worker is attached to this request: it came over plain HTTP or the command line, and a run is handed only to the attached MCP peer that asks for it".into());
    };
    match ctx.peers.get(caller) {
        Some(peer) if peer.attached => Ok(peer),
        _ => Err(format!(
            "no worker is attached: peer {} is no longer on this server's board",
            caller.as_str()
        )),
    }
}

fn refusal(steps: &mut Vec<ImplementStep>, step: &str, message: String) -> CapabilityError {
    steps.push(ImplementStep {
        step: step.into(),
        ok: false,
        detail: message.clone(),
    });
    CapabilityError::Refused(message)
}

fn passed(steps: &mut Vec<ImplementStep>, step: &str, detail: impl Into<String>) {
    steps.push(ImplementStep {
        step: step.into(),
        ok: true,
        detail: detail.into(),
    });
}

fn view_of(ctx: &Context, execution: crate::execution::Execution) -> StartedExecution {
    let links = ExecutionLinks::of(ctx, &execution.id);
    StartedExecution {
        view: ExecutionView { execution, links },
        deduplicated: true,
    }
}

fn devtask_implement(
    ctx: &Context,
    input: ImplementInput,
) -> Result<ImplementAnswer, CapabilityError> {
    implement(ctx, &input, worker_of(ctx))
}

/// The sequence, with the worker given: the handler passes the attached peer that asked, a
/// test passes its own.
///
/// In order, each step only when every one before it passed:
///
/// 1. the key: a request whose key already started an execution for this issue is answered
///    that execution (`deduplicated`) before anything else is asked, and one that started it
///    for another issue is refused `conflict`;
/// 2. readiness — `devtask.issue`'s verdict; `not_ready` with its reason and blockers;
/// 3. `intents.preflight`; `invalid_intent` with its refusal;
/// 4. the claim: would the issue's exclusive mesh claim be admitted now? `claim_conflict`
///    naming each holder;
/// 5. the worker: the attached peer that asked; with none the answer is `pending`;
/// 6. `worktree.create` on `feature/<id>-<slug>`;
/// 7. `executions.start` of `devtask.run` for the issue under the key, which takes the claim
///    before its handler runs (I4004).
///
/// A refusal at 1 to 4 creates nothing.
fn implement(
    ctx: &Context,
    input: &ImplementInput,
    worker: Result<crate::peers::Peer, String>,
) -> Result<ImplementAnswer, CapabilityError> {
    let id = input.issue.trim();
    let plan = Plan::build(&ctx.index);
    let Some(planned) = plan.issue(id) else {
        return Err(CapabilityError::NotFound(format!(
            "{id} is not an issue of this repository's plan; nothing was created"
        )));
    };
    let key_text = input
        .idempotency_key
        .clone()
        .unwrap_or_else(|| default_key(id));
    let key = IdempotencyKey::parse(&key_text).map_err(CapabilityError::InvalidInput)?;
    let deadline = input
        .deadline_minutes
        .unwrap_or(DEFAULT_DEADLINE_MINUTES)
        .clamp(1, MAX_DEADLINE_MINUTES);
    let branch = branch_of(id, &planned.slug);
    let mut steps = Vec::new();
    let answer = |standing, steps, pending, worker, worktree, execution| ImplementAnswer {
        issue: id.to_string(),
        standing,
        idempotency_key: key_text.clone(),
        branch: branch.clone(),
        steps,
        pending,
        worker,
        worktree,
        execution,
    };

    // 1. the key: a repeated request is the execution it already started
    let repository = crate::execution::engine::repository_of(ctx).id;
    if let Some(existing) = ctx.executions.store().keyed(&repository, RUN, &key) {
        let same = existing.input.get("issue").and_then(Value::as_str) == Some(id);
        if !same {
            return Err(CapabilityError::Conflict(format!(
                "the idempotency key {key_text} started {} for another issue; send another key to implement {id}",
                existing.id
            )));
        }
        let worktree = existing
            .input
            .get("worktree")
            .and_then(Value::as_str)
            .map(str::to_string);
        let worker = existing
            .input
            .get("worker")
            .and_then(Value::as_str)
            .map(str::to_string);
        passed(
            &mut steps,
            "execution",
            format!(
                "the key {key_text} already started {} ({}); nothing was created",
                existing.id,
                existing.state.as_str()
            ),
        );
        return Ok(answer(
            ImplementStanding::Deduplicated,
            steps,
            None,
            worker,
            worktree,
            Some(view_of(ctx, existing)),
        ));
    }

    // 2. readiness: exactly what devtask.issue reports
    let task = DevTask::build(
        &DevTaskModel {
            plan: &plan,
            record: record(&ctx.index, ISSUE, id)
                .map(|(path, metadata)| RecordRef { path, metadata }),
            trace: None,
            sessions: sessions_of(&ctx.index, id),
            repository: repository_of(&plan),
        },
        id,
    );
    let verdict = &task.readiness;
    if !verdict.startable {
        let blockers: Vec<String> = verdict
            .blockers
            .iter()
            .map(|b| format!("{} {}: {}", b.kind.as_str(), b.subject, b.detail))
            .collect();
        let detail = if blockers.is_empty() {
            verdict.reason.clone()
        } else {
            format!("{}; blocked by {}", verdict.reason, blockers.join("; "))
        };
        return Err(refusal(
            &mut steps,
            "readiness",
            format!("not_ready: {id}: {detail}; nothing was created"),
        ));
    }
    passed(&mut steps, "readiness", verdict.reason.clone());

    // 3. intents.preflight, as every surface asks it
    let preflight: IntentPreflight = serde_json::from_value(
        ctx.execute("intents.preflight", serde_json::json!({ "issue": id }))?,
    )
    .map_err(|e| CapabilityError::Internal(format!("intents.preflight answered: {e}")))?;
    if preflight.verdict == IntentPreflightVerdict::Refused {
        let why = preflight
            .refusal
            .clone()
            .unwrap_or_else(|| "the preflight refused it".into());
        return Err(refusal(
            &mut steps,
            "preflight",
            format!("invalid_intent: {id}: {why}; nothing was created"),
        ));
    }
    passed(
        &mut steps,
        "preflight",
        match preflight.verdict {
            IntentPreflightVerdict::Serves => "serves a live intent".to_string(),
            _ => "maintenance: serves no live intent and none asks it to".to_string(),
        },
    );

    // 4. the claim: asked, not taken; the execution takes it before it runs
    match crate::execution::claim::admission(ctx, id) {
        crate::execution::claim::Admission::Conflict { message } => {
            return Err(refusal(&mut steps, "claim", message));
        }
        crate::execution::claim::Admission::Free { scope } => passed(
            &mut steps,
            "claim",
            format!(
                "{} is free on the mesh; the execution takes it before it runs",
                scope.join(", ")
            ),
        ),
        crate::execution::claim::Admission::Unmeshed { scope, reason } => passed(
            &mut steps,
            "claim",
            format!(
                "{} cannot be claimed: {reason}; the run is unclaimed",
                scope.join(", ")
            ),
        ),
    }

    // 5. the worker: the attached peer that asked, or pending
    let worker = match worker {
        Ok(peer) => peer,
        Err(why) => {
            steps.push(ImplementStep {
                step: "worker".into(),
                ok: false,
                detail: why.clone(),
            });
            return Ok(answer(
                ImplementStanding::Pending,
                steps,
                Some(why),
                None,
                None,
                None,
            ));
        }
    };
    passed(
        &mut steps,
        "worker",
        format!("{} ({})", worker.id.as_str(), worker.client.name),
    );

    // 6. worktree.create
    let made: crate::worktree::CreateReport = serde_json::from_value(
        ctx.execute("worktree.create", serde_json::json!({ "branch": branch }))?,
    )
    .map_err(|e| CapabilityError::Internal(format!("worktree.create answered: {e}")))?;
    passed(
        &mut steps,
        "worktree",
        format!(
            "{} {}",
            made.path,
            if made.existed {
                "(existed)"
            } else {
                "(created)"
            }
        ),
    );

    // 7. executions.start, under the key and for the issue
    let run = RunInput {
        issue: id.to_string(),
        branch: branch.clone(),
        worktree: made.path.clone(),
        worker: worker.id.as_str().to_string(),
        worker_attached_at: worker.connected_at.clone(),
        deadline_minutes: deadline,
    };
    let started: StartedExecution = serde_json::from_value(ctx.execute(
        "executions.start",
        serde_json::json!({
            "capability": RUN,
            "input": run,
            "idempotency_key": key_text,
            "issue": id,
        }),
    )?)
    .map_err(|e| CapabilityError::Internal(format!("executions.start answered: {e}")))?;
    passed(
        &mut steps,
        "execution",
        format!(
            "{} {}",
            started.view.execution.id,
            if started.deduplicated {
                "(already started by this key)"
            } else {
                "(started)"
            }
        ),
    );
    Ok(answer(
        if started.deduplicated {
            ImplementStanding::Deduplicated
        } else {
            ImplementStanding::Started
        },
        steps,
        None,
        Some(run.worker),
        Some(made.path),
        Some(started),
    ))
}

/// How often a run looks at its worker and its cancellation.
const RUN_POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// Whether the worker a run was handed to is still attached: the same position id, attached
/// at the same instant, and attached now.
fn still_attached(ctx: &Context, input: &RunInput) -> bool {
    ctx.peers.list().iter().any(|p| {
        p.id.as_str() == input.worker && p.connected_at == input.worker_attached_at && p.attached
    })
}

fn devtask_run(ctx: &Context, input: RunInput) -> Result<RunReport, CapabilityError> {
    let deadline = std::time::Duration::from_secs(60 * input.deadline_minutes.max(1));
    run_within(ctx, input, deadline)
}

/// The run, with its deadline given: the handler passes the input's minutes, a test passes
/// a fraction of a second.
fn run_within(
    ctx: &Context,
    input: RunInput,
    deadline: std::time::Duration,
) -> Result<RunReport, CapabilityError> {
    let p = &ctx.progress;
    if p.execution_id().is_none() {
        return Err(CapabilityError::Refused(format!(
            "{RUN} runs only as the execution devtask.implement starts"
        )));
    }
    let started = std::time::Instant::now();
    let report = |ended: String| RunReport {
        issue: input.issue.clone(),
        worker: input.worker.clone(),
        worktree: input.worktree.clone(),
        ended,
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    p.step(
        "work",
        &format!(
            "{} works on {} in {}",
            input.worker, input.issue, input.worktree
        ),
    );
    if !still_attached(ctx, &input) {
        p.step_done("work", false, Some("the worker is not attached".into()));
        return Err(CapabilityError::Refused(format!(
            "no_worker: {} is not attached to this server; nothing ran",
            input.worker
        )));
    }
    loop {
        if p.cancelled() {
            p.step_done("work", false, Some("cancelled".into()));
            return Err(p.cancellation());
        }
        if !still_attached(ctx, &input) {
            let ended = format!(
                "{} detached; what it did is on {} in {}",
                input.worker, input.branch, input.worktree
            );
            p.step_done("work", true, Some(ended.clone()));
            return Ok(report(ended));
        }
        if started.elapsed() >= deadline {
            let message = format!(
                "deadline: {} was still attached after {} s",
                input.worker,
                deadline.as_secs_f64()
            );
            p.step_done("work", false, Some(message.clone()));
            return Err(CapabilityError::Internal(message));
        }
        std::thread::sleep(RUN_POLL);
    }
}

// ---------------------------------------------------------------- the module

/// The `devtask` module: the canonical development task, composed from derivations that
/// already exist.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "devtask",
        title: "Executable development scopes",
        description: "An issue and a milestone read as work to be done rather than as metadata: what a person authored, where the plan graph puts it, what happened to it locally, where its external projection stands, and whether a worker may start it — with the origin of every value on the value. Nothing here is a second project model: the records are the canonical ones, the status and the graph are `plan`'s, the branches and commits are `trace`'s, the readiness vocabulary refines three canonical statuses without replacing any, and the projection onto GitHub stays `scripts/github-sync`'s. The two readers write nothing, which is what makes a user-authored GitHub value safe from them; `devtask.implement` writes only through the capabilities it composes (`worktree.create`, `executions.start`), and `devtask.run` is the run it starts.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "devtask.issue",
                title: "One issue as an executable development task",
                description: "Identity, title, intent, status, milestone, acceptance criteria, dependencies, blockers, the commits its own evidence names and the commits git found, its branches, the sessions that worked on it, its readiness and everything wrong with its records — each field carrying whether a person authored it (`explicit`), a machine worked it out by a rule that cannot be wrong (`derived`), a machine worked it out by a rule that can (`inferred`, with the rule stated), or it is not available at all (`unknown`, with the reason). A key the record does not carry is `unknown`, never an empty string: `objective: \"\"` and a record with no objective are the same string and different facts. An id the model does not declare is answered, not refused. `git: false` answers from the records alone, which is what a comparison across machines wants.",
                input: DevTaskInput,
                output: DevTask,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_devtask"),
                    http: get("/api/v1/devtask/issue"),
                    cli: Some(CliExposure { path: vec!["devtask".into(), "issue".into()] }),
                },
                tags: ["devtask", "plan", "issue", "provenance", "readiness", "git"],
                cache: CachePolicy::Disabled,
                handler: devtask_issue,
            },
            capability! {
                id: "devtask.milestone",
                title: "One milestone as an executable dependency graph",
                description: "Every issue of the milestone as a node with its readiness, its wave and what waits on it; every dependency edge with at least one end inside, the crossing ones marked; the issues partitioned into ready, blocked, waiting, active, review, completion-blocked, complete and cancelled; the critical blockers ordered by how much unfinished work each holds back; the startable work partitioned into subsets that may genuinely run at the same time — same wave, each parallel-safe, no two sharing a scope path — with each serialisation naming the path that caused it; every dependency cycle as its strongly connected component; and every finding about the milestone or its issues. A pure function of the canonical records: no git, no clock, no network, every list in canonical order, so two runs on two machines produce the same bytes. A work surface reading this derives nothing itself.",
                input: DevMilestoneInput,
                output: MilestoneGraph,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_devtask_milestone"),
                    http: get("/api/v1/devtask/milestone"),
                    cli: Some(CliExposure { path: vec!["devtask".into(), "milestone".into()] }),
                },
                tags: ["devtask", "plan", "milestone", "graph", "readiness", "provenance"],
                cache: CachePolicy::Disabled,
                handler: devtask_milestone,
            },
            capability! {
                id: "devtask.implement",
                kind: CapabilityKind::Command,
                title: "Implement an issue: readiness, preflight, claim, worktree and one run",
                description: "Take one issue as one request (I4003): a key that already started an execution for it answers that execution (`deduplicated`) and creates nothing; otherwise the issue's readiness as `devtask.issue` derives it (refused `not_ready` with the reason and the blockers), `intents.preflight` (refused `invalid_intent` with its refusal), whether the issue's exclusive mesh claim would be admitted now (refused `claim_conflict` naming each holder), the worker — the attached MCP peer that asked; with none the answer is `pending` and nothing is created — then `worktree.create` on `feature/<id>-<slug>` and `executions.start` of `devtask.run` for the issue under the key (default `implement-<issue>`), whose execution takes the issue's claim before it runs (I4004). A refusal creates nothing. The answer is the standing, every step with what it found, the branch, the worktree, the worker and the execution with its links.",
                input: ImplementInput,
                output: ImplementAnswer,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_devtask_implement"),
                    http: post("/api/v1/devtask/implement"),
                    cli: None,
                },
                tags: ["devtask", "issue", "executions", "worktree", "mesh", "intent"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: devtask_implement,
            }
            // it creates a worktree and starts an execution: a write, asked before it runs
            .writes_repository(),
            capability! {
                id: "devtask.run",
                kind: CapabilityKind::Command,
                title: "A worker's run on an issue's worktree",
                description: "The execution `devtask.implement` starts, and nothing else: the run handed to the attached peer that asked, in the issue's worktree, under the issue's mesh claim. It lasts while that peer stays attached — the same position id, attached at the same instant — and ends `succeeded` when it detaches, `cancelled` when asked to stop, and `failed` on its deadline; refused when the worker is not attached when it starts. What the worker did is in the worktree and is verified by what verifies a branch, never by this. Reachable only through `executions.start`: it declares no route, tool or command of its own.",
                input: RunInput,
                output: RunReport,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: None, http: None, cli: None },
                tags: ["devtask", "issue", "executions", "worker"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: devtask_run,
            }
            .cancellable(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devtask::{FieldProvenance, TaskReadiness};

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "devtask");
        type Expected<'a> = (
            &'a str,
            Option<&'a str>,
            Option<&'a str>,
            Option<&'a [&'a str]>,
        );
        let expected: &[Expected] = &[
            (
                "devtask.issue",
                Some("majordomus_devtask"),
                Some("/api/v1/devtask/issue"),
                Some(&["devtask", "issue"]),
            ),
            (
                "devtask.milestone",
                Some("majordomus_devtask_milestone"),
                Some("/api/v1/devtask/milestone"),
                Some(&["devtask", "milestone"]),
            ),
            (
                "devtask.implement",
                Some("majordomus_devtask_implement"),
                Some("/api/v1/devtask/implement"),
                None,
            ),
            // reachable only through executions.start
            ("devtask.run", None, None, None),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (executable, (id, tool, path, cli)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                *tool,
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                *path,
                "{id} lost or renamed its HTTP route"
            );
            assert_eq!(
                exposure.cli.as_ref().map(|c| c.path.clone()),
                cli.map(|c| c.iter().map(|s| (*s).to_string()).collect::<Vec<String>>()),
                "{id} lost or renamed its command"
            );
        }
    }

    /// The readers are queries: a development task is derived, and one that wrote anything
    /// would be the second store of truth this module exists to avoid. `devtask.implement`
    /// writes, through what it composes, and says so; `devtask.run` stops when asked. None
    /// of them caches: the records and the history change outside this process.
    #[test]
    fn the_readers_are_queries_the_implementation_writes_and_nothing_caches() {
        for e in module().capabilities {
            let c = &e.capability;
            assert!(!c.cache.is_enabled(), "{} caches", c.id);
            match c.id.as_str() {
                "devtask.issue" | "devtask.milestone" => {
                    assert!(c.kind.is_read_only(), "{} writes", c.id)
                }
                "devtask.implement" => {
                    assert!(!c.kind.is_read_only());
                    assert_eq!(
                        c.execution.effect,
                        crate::capability::model::Effect::RepositoryMutation
                    );
                }
                "devtask.run" => {
                    assert!(!c.kind.is_read_only());
                    assert!(c.execution.cancellable, "a run stops when asked");
                }
                other => panic!("an undeclared capability {other}"),
            }
        }
    }

    /// The one property that makes this a composition rather than a parallel registry: for
    /// every issue of this repository, the task's status *is* the plan's status, and its
    /// readiness names that same status back.
    #[test]
    fn the_status_never_disagrees_with_the_plan() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("synthetic repository");
        let index = repo.index().expect("index");
        let plan = Plan::build(&index);
        for issue in &plan.issues {
            let record = record(&index, ISSUE, &issue.id)
                .map(|(path, metadata)| RecordRef { path, metadata });
            let model = DevTaskModel {
                plan: &plan,
                record,
                trace: None,
                sessions: Vec::new(),
                repository: repository_of(&plan),
            };
            let task = DevTask::build(&model, &issue.id);
            assert_eq!(
                task.position.status.text(),
                issue.status,
                "{} disagrees with the plan",
                issue.id
            );
            assert_eq!(
                task.readiness.canonical_status, issue.status,
                "{} readiness names a status the plan did not derive",
                issue.id
            );
            assert_eq!(task.position.status.provenance, FieldProvenance::Derived);
            assert!(task.is_consistent());
        }
    }

    /// An id nothing declares is answered rather than refused, and the answer cannot be
    /// mistaken for an issue nobody has started.
    #[test]
    fn an_id_the_model_does_not_have_is_answered_with_undeclared() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("synthetic repository");
        let index = repo.index().expect("index");
        let plan = Plan::build(&index);
        let model = DevTaskModel {
            plan: &plan,
            record: None,
            trace: None,
            sessions: Vec::new(),
            repository: repository_of(&plan),
        };
        let task = DevTask::build(&model, "I9999");
        assert!(!task.declared);
        assert_eq!(task.readiness.state, TaskReadiness::Undeclared);
        assert!(!task.readiness.startable);
    }

    /// The milestone graph partitions its own nodes, whatever the repository holds: an
    /// issue is in exactly one readiness bucket and every bucket names a node.
    #[test]
    fn every_milestone_of_this_repository_is_partitioned() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("synthetic repository");
        let index = repo.index().expect("index");
        let plan = Plan::build(&index);
        for m in &plan.milestones {
            let record = record(&index, MILESTONE, &m.id)
                .map(|(path, metadata)| RecordRef { path, metadata });
            let g = MilestoneGraph::build(&plan, &m.id, record);
            assert!(g.is_partitioned(), "{} is not partitioned", m.id);
            assert!(g.is_consistent(), "{} has an inconsistent field", m.id);
            assert_eq!(
                g.status.text(),
                m.status,
                "{} disagrees with the plan",
                m.id
            );
        }
    }

    // ------------------------------------------------------------ devtask.implement (I4003)

    use crate::execution::{ClaimStanding, ExecutionState};
    use crate::mesh::journal::{ClaimMode, SessionInfo};
    use crate::peers::Transport;
    use std::sync::Arc;

    /// The planned synthetic repository with one commit, so that a worktree can be made of
    /// it, and `extra` records written before its index is read.
    fn planned_with(extra: &[(&str, &str)]) -> crate::synthetic::SyntheticRepository {
        let repo = crate::synthetic::SyntheticRepository::planned();
        for (rel, body) in extra {
            let path = repo.root().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(repo.root())
                .args([
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "-c",
                    "commit.gpgsign=false"
                ])
                .args(args)
                .status()
                .unwrap()
                .success());
        };
        git(&["add", "-A"]);
        git(&["commit", "-qm", "fixture", "--no-verify"]);
        repo
    }

    fn request(issue: &str, key: Option<&str>) -> ImplementInput {
        ImplementInput {
            issue: issue.into(),
            idempotency_key: key.map(str::to_string),
            deadline_minutes: None,
        }
    }

    /// A context whose call came from a peer attached to its board, and that peer.
    fn as_worker(ctx: &Arc<Context>) -> (Context, crate::peers::Peer) {
        let id = ctx.peers.attach(Transport::Http);
        let mut c = (**ctx).clone();
        c.caller = Some(id.clone());
        let peer = ctx.peers.get(&id).unwrap();
        (c, peer)
    }

    fn until(
        ctx: &Context,
        id: &crate::execution::ExecutionId,
        want: ExecutionState,
    ) -> crate::execution::Execution {
        for _ in 0..300 {
            let now = ctx.executions.store().get(id).unwrap();
            if now.state == want {
                return now;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!(
            "{id} never reached {want:?}: {:?}",
            ctx.executions.store().get(id)
        );
    }

    fn worktrees_beside(repo: &crate::synthetic::SyntheticRepository) -> std::path::PathBuf {
        let mut name = repo.root().file_name().unwrap().to_os_string();
        name.push("-wt");
        repo.root().with_file_name(name)
    }

    #[test]
    fn the_default_key_is_the_issues_and_only_holds_what_a_key_may() {
        assert_eq!(default_key("I4003"), "implement-I4003");
        assert_eq!(default_key("#184"), "implement--184");
        assert!(IdempotencyKey::parse(&default_key("a b/c#d")).is_ok());
    }

    #[test]
    fn the_branch_is_the_issues_and_a_missing_slug_is_its_id() {
        assert_eq!(branch_of("I1", "do-it"), "feature/I1-do-it");
        assert_eq!(branch_of("I1", "  "), "feature/I1-i1");
    }

    #[test]
    fn the_worker_is_the_attached_peer_that_asked_and_nobody_else() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        // plain HTTP or the command line: nobody asked
        let none = worker_of(&ctx).unwrap_err();
        assert!(none.starts_with("no worker is attached"), "{none}");
        // another peer attached elsewhere on the board is not pushed the work
        let _bystander = ctx.peers.attach(Transport::Http);
        assert!(worker_of(&ctx).is_err());
        let (c, peer) = as_worker(&ctx);
        assert_eq!(worker_of(&c).unwrap().id, peer.id);
        // gone from the board: no longer a worker
        ctx.peers.detach(&peer.id);
        let gone = worker_of(&c).unwrap_err();
        assert!(gone.contains(peer.id.as_str()), "{gone}");
    }

    #[test]
    fn an_issue_the_plan_does_not_hold_or_a_bad_key_is_refused_before_anything() {
        let repo = planned_with(&[]);
        let ctx = repo.context().unwrap();
        assert!(matches!(
            implement(&ctx, &request("I9999", None), Err("x".into())),
            Err(CapabilityError::NotFound(_))
        ));
        assert!(matches!(
            implement(&ctx, &request("I0001", Some("a/b")), Err("x".into())),
            Err(CapabilityError::InvalidInput(_))
        ));
        assert!(ctx.executions.store().is_empty());
    }

    #[test]
    fn an_issue_that_is_not_ready_is_refused_with_its_blockers_and_nothing_is_created() {
        let repo = planned_with(&[]);
        let ctx = repo.context().unwrap();
        let (c, _) = as_worker(&ctx);
        // I0002 waits on I0001
        match implement(&c, &request("I0002", None), worker_of(&c)) {
            Err(CapabilityError::Refused(m)) => {
                assert!(m.starts_with("not_ready: I0002"), "{m}");
                assert!(m.contains("I0001"), "the blocker is named: {m}");
            }
            other => panic!("{other:?}"),
        }
        assert!(ctx.executions.store().is_empty());
        assert!(!worktrees_beside(&repo).exists(), "no worktree was made");
    }

    #[test]
    fn an_issue_whose_intent_is_invalid_is_refused_with_the_preflights_reason() {
        let repo = planned_with(&[(
            ".ai/repo/project/issues/I0020.yaml",
            "id: I0020\nmilestone: m-first\ntitle: Serves nothing real\nslug: nowhere\n\
             priority: p1\nprofile: implementation\nobjective: \"Serve a criterion nobody declared.\"\n\
             scope:\n  - elsewhere\nserves:\n  - nobody#nothing\nacceptance_criteria:\n  - done\n\
             validation:\n  - \"true\"\nevidence_required:\n  - proof\n",
        )]);
        let ctx = repo.context().unwrap();
        let (c, _) = as_worker(&ctx);
        match implement(&c, &request("I0020", None), worker_of(&c)) {
            Err(CapabilityError::Refused(m)) => {
                assert!(m.starts_with("invalid_intent: I0020"), "{m}");
                assert!(
                    m.contains("nobody"),
                    "the preflight's reason is carried: {m}"
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(ctx.executions.store().is_empty());
        assert!(!worktrees_beside(&repo).exists(), "no worktree was made");
    }

    #[test]
    fn an_overlapping_claim_is_refused_naming_its_holder_and_nothing_is_created() {
        let repo = planned_with(&[]);
        let ctx = repo.context().unwrap();
        let mesh = crate::mesh::testing::cooperation();
        crate::mesh::testing::attach(&ctx, Arc::clone(&mesh));
        // I0001's scope is `lib`; somebody else holds it
        let held = mesh
            .claim(
                &SessionInfo::named("somebody", "cli"),
                vec!["lib".into()],
                None,
                ClaimMode::Exclusive,
                None,
            )
            .unwrap();
        let (c, _) = as_worker(&ctx);
        match implement(&c, &request("I0001", None), worker_of(&c)) {
            Err(CapabilityError::Refused(m)) => {
                assert!(m.starts_with("claim_conflict: I0001"), "{m}");
                assert!(m.contains("somebody") && m.contains(&held.key), "{m}");
            }
            other => panic!("{other:?}"),
        }
        assert!(ctx.executions.store().is_empty());
        assert!(!worktrees_beside(&repo).exists(), "no worktree was made");
    }

    #[test]
    fn with_no_worker_attached_the_answer_is_pending_and_nothing_is_created() {
        let repo = planned_with(&[]);
        let ctx = repo.context().unwrap();
        let answer = implement(&ctx, &request("I0001", None), worker_of(&ctx)).unwrap();
        assert_eq!(answer.standing, ImplementStanding::Pending);
        assert!(answer.pending.unwrap().starts_with("no worker is attached"));
        assert!(answer.execution.is_none() && answer.worktree.is_none());
        let steps: Vec<(&str, bool)> = answer
            .steps
            .iter()
            .map(|s| (s.step.as_str(), s.ok))
            .collect();
        assert_eq!(
            steps,
            [
                ("readiness", true),
                ("preflight", true),
                ("claim", true),
                ("worker", false)
            ]
        );
        assert!(ctx.executions.store().is_empty());
        assert!(!worktrees_beside(&repo).exists(), "no worktree was made");
    }

    #[test]
    fn a_ready_issue_starts_one_run_holding_its_claim_in_a_new_worktree_and_twice_is_once() {
        let repo = planned_with(&[]);
        let ctx = repo.context().unwrap();
        let mesh = crate::mesh::testing::cooperation();
        crate::mesh::testing::attach(&ctx, Arc::clone(&mesh));
        let (c, peer) = as_worker(&ctx);
        let first = implement(&c, &request("I0001", Some("click-1")), worker_of(&c)).unwrap();
        assert_eq!(first.standing, ImplementStanding::Started);
        assert_eq!(first.branch, "feature/I0001-work-I0001");
        let steps: Vec<&str> = first.steps.iter().map(|s| s.step.as_str()).collect();
        assert_eq!(
            steps,
            [
                "readiness",
                "preflight",
                "claim",
                "worker",
                "worktree",
                "execution"
            ]
        );
        assert!(first.steps.iter().all(|s| s.ok));
        let worktree = first.worktree.clone().unwrap();
        assert!(
            std::path::Path::new(&worktree).join(".git").exists(),
            "a new worktree"
        );
        let execution = first.execution.clone().unwrap().view.execution;
        assert_eq!(execution.capability, RUN);
        let running = until(&c, &execution.id, ExecutionState::Running);
        let claim = running.issue_claim.unwrap();
        assert_eq!(
            claim.standing,
            ClaimStanding::Held,
            "the run holds the issue's claim"
        );
        assert_eq!(claim.scope, ["lib"]);

        // the same request again: the same execution, nothing new
        let again = implement(&c, &request("I0001", Some("click-1")), worker_of(&c)).unwrap();
        assert_eq!(again.standing, ImplementStanding::Deduplicated);
        assert_eq!(again.execution.unwrap().view.execution.id, execution.id);
        assert_eq!(again.worktree.as_deref(), Some(worktree.as_str()));
        assert_eq!(ctx.executions.store().len(), 1);
        // the key, sent for another issue, is refused naming what it started
        match implement(&c, &request("I0003", Some("click-1")), worker_of(&c)) {
            Err(CapabilityError::Conflict(m)) => assert!(m.contains(execution.id.as_str()), "{m}"),
            other => panic!("{other:?}"),
        }
        // a second worker asking for the issue while the first holds it is refused
        let (other, _) = as_worker(&ctx);
        match implement(
            &other,
            &request("I0001", Some("click-2")),
            worker_of(&other),
        ) {
            Err(CapabilityError::Refused(m)) => {
                assert!(m.starts_with("claim_conflict: I0001"), "{m}")
            }
            r => panic!("{r:?}"),
        }

        // the worker leaves: the run ends and gives its claim back
        ctx.peers.detach(&peer.id);
        let done = until(&c, &execution.id, ExecutionState::Succeeded);
        assert_eq!(done.issue_claim.unwrap().standing, ClaimStanding::Released);
        let _ = std::fs::remove_dir_all(worktrees_beside(&repo));
    }

    fn run_input(peer: &crate::peers::Peer) -> RunInput {
        RunInput {
            issue: "I0001".into(),
            branch: "feature/I0001-x".into(),
            worktree: "/nowhere".into(),
            worker: peer.id.as_str().to_string(),
            worker_attached_at: peer.connected_at.clone(),
            deadline_minutes: 1,
        }
    }

    #[test]
    fn a_run_is_refused_outside_an_execution_or_without_its_worker() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let peer = ctx.peers.get(&ctx.peers.attach(Transport::Http)).unwrap();
        // called directly, not as an execution
        assert!(matches!(
            devtask_run(&ctx, run_input(&peer)),
            Err(CapabilityError::Refused(m)) if m.contains("only as the execution")
        ));
        // as an execution, for a worker that is not attached
        let mut gone = run_input(&peer);
        gone.worker_attached_at = "1970-01-01T00:00:00Z".into();
        let started = ctx
            .executions
            .submit_once(
                &ctx,
                RUN,
                serde_json::to_value(gone).unwrap(),
                crate::execution::Actor::of(crate::execution::ActorKind::Internal),
                None,
            )
            .unwrap();
        let failed = until(&ctx, &started.execution.id, ExecutionState::Failed);
        assert!(failed.error.unwrap().message.contains("no_worker"));
    }

    #[test]
    fn a_run_lasts_while_its_worker_is_attached_and_stops_when_asked() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let peer = ctx.peers.get(&ctx.peers.attach(Transport::Http)).unwrap();
        let started = ctx
            .executions
            .submit_once(
                &ctx,
                RUN,
                serde_json::to_value(run_input(&peer)).unwrap(),
                crate::execution::Actor::of(crate::execution::ActorKind::Internal),
                None,
            )
            .unwrap();
        until(&ctx, &started.execution.id, ExecutionState::Running);
        ctx.executions.cancel(&started.execution.id, "test");
        until(&ctx, &started.execution.id, ExecutionState::Cancelled);
        // and ends on its own when the worker detaches
        let started = ctx
            .executions
            .submit_once(
                &ctx,
                RUN,
                serde_json::to_value(run_input(&peer)).unwrap(),
                crate::execution::Actor::of(crate::execution::ActorKind::Internal),
                None,
            )
            .unwrap();
        until(&ctx, &started.execution.id, ExecutionState::Running);
        ctx.peers.detach(&peer.id);
        let done = until(&ctx, &started.execution.id, ExecutionState::Succeeded);
        let report: RunReport = serde_json::from_value(done.output.unwrap()).unwrap();
        assert!(report.ended.contains("detached"), "{}", report.ended);
    }

    #[test]
    fn a_run_whose_worker_stays_fails_on_its_deadline() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let peer = ctx.peers.get(&ctx.peers.attach(Transport::Http)).unwrap();
        let store = ctx.executions.store();
        let id = crate::execution::ExecutionId::fresh();
        let cancel = store.create_authorizing(
            id.clone(),
            crate::execution::Seed {
                capability: RUN,
                title: "run",
                input: serde_json::json!({}),
                cancellable: true,
                actor: crate::execution::Actor::of(crate::execution::ActorKind::Internal),
                repository: crate::execution::engine::repository_of(&ctx),
            },
        );
        let mut c = (*ctx).clone();
        c.progress = crate::execution::Progress::reporting(Arc::clone(store), id, cancel);
        match run_within(&c, run_input(&peer), std::time::Duration::from_millis(50)) {
            Err(CapabilityError::Internal(m)) => assert!(m.starts_with("deadline:"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_worker_is_its_id_and_the_instant_it_attached() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let peer = ctx.peers.get(&ctx.peers.attach(Transport::Http)).unwrap();
        let input = run_input(&peer);
        assert!(still_attached(&ctx, &input));
        let mut reused = input.clone();
        reused.worker_attached_at = "2000-01-01T00:00:00Z".into();
        assert!(
            !still_attached(&ctx, &reused),
            "a reused position id is another peer"
        );
        ctx.peers.detach(&peer.id);
        assert!(!still_attached(&ctx, &input));
    }
}
