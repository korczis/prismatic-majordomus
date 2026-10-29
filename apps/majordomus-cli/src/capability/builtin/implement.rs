//! The `implement` module: one issue, implemented by one execution (ADR 0225, I3983).
//!
//! `implement.issue` is the capability behind the UI's **Implement**. It is started only as
//! an execution, through `executions.start` (idempotent under its key, I3981), and every
//! decision it takes is a step of that execution, in this order:
//!
//! ```text
//!   readiness ── the issue's derived readiness, exactly devtask.issue's; refused otherwise
//!   lease ────── an exclusive mesh claim of the issue's scope, held by the execution's own
//!                session; refused naming the claim it meets; released when it ends
//!   worktree ─── the issue's canonical worktree, created or found (worktree create)
//!   dispatch ─── the first available worker adapter runs in that worktree; its output is
//!                the execution's events; with none available, refused naming each reason
//! ```
//!
//! Nothing here is a second scheduler or a second project model: readiness is
//! [`crate::devtask`]'s, the claim is the mesh's, the worktree is
//! [`crate::worktree::WorktreeService`]'s and the run is [`crate::worker::run`]'s. Whether
//! the work is then verified is the next step's (I3986); delivering it is another execution
//! entirely.
//!
//! ```
//! use majordomus_cli::capability::builtin::implement::branch_of;
//! assert_eq!(branch_of("I3983", "an-issue-is-implemented-by-one-capability"),
//!            "feature/I3983-an-issue-is-implemented-by-one-capability");
//! ```

use std::path::Path;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CapabilityKind, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::devtask::task::{DevTaskModel, RecordRef};
use crate::devtask::DevTask;
use crate::mesh::journal::{ClaimMode, SessionInfo};
use crate::plan::{Plan, ISSUE};
use crate::worker::{Adapter, Assignment, Outcome};
use crate::worktree::{CreateRequest, WorktreeService};
use crate::{capability, module};

use super::devtask::{record, refused, repository_of, sessions_of};

/// The longest a worker may run when the input says nothing: one hour.
const DEFAULT_DEADLINE_MINUTES: u64 = 60;

/// The input of `implement.issue`: which issue, and how long its worker may run. Nothing
/// else is the caller's to choose — the worktree, the branch, the lease and the adapter
/// are derived — so an execution's input names only what a person picked.
///
/// ```
/// use majordomus_cli::capability::builtin::implement::ImplementInput;
/// let input: ImplementInput = serde_json::from_str(r#"{"issue":"I3983"}"#).unwrap();
/// assert_eq!((input.issue.as_str(), input.deadline_minutes), ("I3983", None));
/// assert!(serde_json::from_str::<ImplementInput>(r#"{"issue":"I1","worktree":"/x"}"#).is_err(),
///     "the worktree is derived, never given");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImplementInput {
    /// The issue to implement, as the canonical model spells it (`I3983`).
    pub issue: String,
    /// How many minutes the worker may run before its whole process group is stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_minutes: Option<u64>,
}

impl BenchmarkCases for ImplementInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "an-issue",
            ImplementInput {
                issue: "I0001".into(),
                deadline_minutes: Some(1),
            },
        )]
    }
}

/// What an implementation ended with: where it ran, under which lease, by which worker,
/// and how the worker ended. Done here means the worker ended successfully, never verified
/// and never delivered.
///
/// ```
/// use majordomus_cli::capability::builtin::implement::ImplementReport;
/// let report = ImplementReport {
///     issue: "I0001".into(), branch: "feature/I0001-x".into(), worktree: "/wt".into(),
///     lease: None, adapter: "claude-code".into(), outcome: "the worker succeeded".into(),
///     verified: Vec::new(), state: "done".into(),
/// };
/// let json = serde_json::to_value(&report).unwrap();
/// assert!(json.get("lease").is_none(), "no mesh in the process: no lease is claimed");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImplementReport {
    /// The issue.
    pub issue: String,
    /// The branch the work is on.
    pub branch: String,
    /// The worktree it ran in.
    pub worktree: String,
    /// The mesh claim that was the lease, when the mesh runs in this process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<String>,
    /// The adapter that ran the worker.
    pub adapter: String,
    /// How the worker ended, in words.
    pub outcome: String,
    /// Every validation command the issue declares, run in the worktree after the worker,
    /// each with how it ended: the execution's own evidence (I3986).
    #[serde(default)]
    pub verified: Vec<Verified>,
    /// `done`: the worker succeeded and every validation passed. Never `delivered`, which
    /// is another execution's word (ADR 0225).
    pub state: String,
}

/// One validation command the issue declares, and how it ended in the worktree.
///
/// ```
/// use majordomus_cli::capability::builtin::implement::Verified;
/// let v = Verified { command: "true".into(), ok: true, outcome: "the worker succeeded".into() };
/// assert!(v.ok);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Verified {
    /// The command, as the issue's `validation` declares it.
    pub command: String,
    /// Whether it exited 0.
    pub ok: bool,
    /// How it ended, in words.
    pub outcome: String,
}

/// The canonical branch of an issue: `feature/<id>-<slug>`, the form the worktree
/// topology reads the issue back from (`majordomus worktree create --issue`).
///
/// ```
/// use majordomus_cli::capability::builtin::implement::branch_of;
/// assert_eq!(branch_of("I0001", "first"), "feature/I0001-first");
/// ```
pub fn branch_of(issue: &str, slug: &str) -> String {
    format!("feature/{issue}-{slug}")
}

fn issue_implement(
    ctx: &Context,
    input: ImplementInput,
) -> Result<ImplementReport, CapabilityError> {
    implement(ctx, &input, &crate::worker::declared())
}

/// The capability's body, with the adapters to choose from given: the handler passes the
/// declared ones, a test passes a stand-in.
///
/// Refused, with nothing done after the refusing step:
/// * outside an execution — the steps, the lease and the cancellation need one;
/// * an issue that is not ready, naming devtask's reason and blockers;
/// * an issue with no scope, which cannot be leased;
/// * a scope another live exclusive claim holds, naming that claim;
/// * no available adapter, naming each adapter's reason.
///
/// ```no_run
/// use majordomus_cli::capability::builtin::implement::{implement, ImplementInput};
/// use majordomus_cli::capability::Context;
/// use majordomus_cli::worker::{Adapter, Program};
/// fn run(ctx: &Context) {
///     let worker: Vec<Box<dyn Adapter>> = vec![Box::new(Program::new("echo", "echo", &[]))];
///     let input = ImplementInput { issue: "I0001".into(), deadline_minutes: Some(5) };
///     let report = implement(ctx, &input, &worker).unwrap();
///     assert!(report.branch.starts_with("feature/I0001-"));
/// }
/// ```
pub fn implement(
    ctx: &Context,
    input: &ImplementInput,
    adapters: &[Box<dyn Adapter>],
) -> Result<ImplementReport, CapabilityError> {
    let p = &ctx.progress;
    let execution = p.execution_id().cloned().ok_or_else(|| {
        CapabilityError::Refused(
            "implement.issue runs only as an execution: start it with executions.start".into(),
        )
    })?;
    let id = input.issue.as_str();

    // readiness: exactly what devtask.issue reports
    p.step("readiness", "Is the issue ready?");
    let plan = Plan::build(&ctx.index);
    let Some(planned) = plan.issue(id) else {
        p.step_done("readiness", false, Some(format!("{id} is not declared")));
        return Err(CapabilityError::NotFound(format!(
            "{id} is not an issue of this repository's plan"
        )));
    };
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
        p.step_done("readiness", false, Some(detail.clone()));
        return Err(CapabilityError::Refused(format!(
            "not_ready: {id}: {detail}"
        )));
    }
    p.step_done("readiness", true, Some(verdict.reason.clone()));

    // lease: the issue's scope, claimed exclusively for the execution's own session
    p.step("lease", "Claim the issue's scope");
    if planned.scope.is_empty() {
        p.step_done("lease", false, Some("the issue declares no scope".into()));
        return Err(CapabilityError::Refused(format!(
            "no_scope: {id} declares no scope, so nothing can be claimed for it"
        )));
    }
    let session = SessionInfo::named(&format!("exec-{execution}"), "majordomus");
    let lease = match ctx.mesh.cooperation() {
        Some(cooperation) => {
            match cooperation.claim(
                &session,
                planned.scope.clone(),
                Some(format!("{id}: {}", planned.title)),
                ClaimMode::Exclusive,
                Some(id.to_string()),
            ) {
                Ok(written) => Some((cooperation, written.key)),
                Err(e) => {
                    p.step_done("lease", false, Some(e.to_string()));
                    return Err(CapabilityError::Refused(format!("claim_conflict: {e}")));
                }
            }
        }
        None => None,
    };
    p.step_done(
        "lease",
        true,
        Some(match &lease {
            Some((_, key)) => format!("claimed {key}"),
            None => {
                "the mesh does not run in this process: nothing else can hold the scope here".into()
            }
        }),
    );
    // the lease ends with this call, however it ends
    let _release = Release(lease.as_ref().map(|(c, k)| (c.clone(), k.clone())));

    // worktree: the issue's canonical one
    p.step("worktree", "Create the issue's worktree");
    let branch = branch_of(id, &planned.slug);
    let report = WorktreeService::open(Path::new(&ctx.index.repository.root))
        .and_then(|service| {
            service.ensure(&CreateRequest {
                branch: branch.clone(),
                base: None,
            })
        })
        .map_err(|e| {
            p.step_done("worktree", false, Some(e.to_string()));
            refused(e)
        })?;
    p.step_done("worktree", true, Some(report.path.clone()));

    // dispatch: the first available worker
    p.step("dispatch", "Hand the work to a worker");
    let adapter = crate::worker::choose(adapters).map_err(|reasons| {
        let reasons: Vec<String> = reasons
            .iter()
            .map(|(adapter, reason)| format!("{adapter}: {reason}"))
            .collect();
        let detail = if reasons.is_empty() {
            "no worker adapter is declared".to_string()
        } else {
            reasons.join("; ")
        };
        p.step_done("dispatch", false, Some(detail.clone()));
        CapabilityError::Refused(format!("no_worker: {detail}"))
    })?;
    let deadline = Duration::from_secs(
        60 * input
            .deadline_minutes
            .unwrap_or(DEFAULT_DEADLINE_MINUTES)
            .max(1),
    );
    let assignment =
        Assignment::new(execution.as_str(), &report.path, prompt(id, planned)).within(deadline);
    let outcome = crate::worker::run(adapter, &assignment, p).map_err(|e| {
        p.step_done("dispatch", false, Some(e.clone()));
        CapabilityError::Internal(e)
    })?;
    p.step_done("dispatch", outcome.ok(), Some(outcome.to_string()));
    match outcome {
        Outcome::Succeeded => {}
        Outcome::Cancelled => return Err(p.cancellation()),
        other => return Err(CapabilityError::Internal(format!("worker_failed: {other}"))),
    }

    // verify: the issue's own validation, in the same worktree, as the same execution
    p.step("verify", "Run the issue's validation in the worktree");
    let commands = validation_of(ctx, id);
    if commands.is_empty() {
        p.step_done(
            "verify",
            false,
            Some("the issue declares no validation".into()),
        );
        return Err(CapabilityError::Refused(format!(
            "no_validation: {id} declares no validation, so its work cannot be verified"
        )));
    }
    let mut verified = Vec::new();
    for command in commands {
        let shell =
            crate::worker::Program::new("validation", "sh", &["-c", &command, "validation"]);
        let run = Assignment::new(execution.as_str(), &report.path, "").within(deadline);
        // the prompt is the empty last argument `sh -c` ignores after `$0`
        let ended = crate::worker::run(&shell, &run, p).map_err(|e| {
            p.step_done("verify", false, Some(e.clone()));
            CapabilityError::Internal(e)
        })?;
        if ended == Outcome::Cancelled {
            p.step_done("verify", false, Some("cancelled".into()));
            return Err(p.cancellation());
        }
        let ok = ended.ok();
        verified.push(Verified {
            command: command.clone(),
            ok,
            outcome: ended.to_string(),
        });
        if !ok {
            p.step_done("verify", false, Some(format!("`{command}`: {ended}")));
            return Err(CapabilityError::Internal(format!(
                "verification_failed: `{command}`: {ended}"
            )));
        }
    }
    p.step_done(
        "verify",
        true,
        Some(format!("{} validation command(s) passed", verified.len())),
    );
    Ok(ImplementReport {
        issue: id.to_string(),
        branch,
        worktree: report.path,
        lease: lease.map(|(_, key)| key),
        adapter: adapter.id().to_string(),
        outcome: outcome.to_string(),
        verified,
        state: "done".into(),
    })
}

/// The validation commands the issue's record declares, in order.
fn validation_of(ctx: &Context, id: &str) -> Vec<String> {
    record(&ctx.index, ISSUE, id)
        .and_then(|(_, metadata)| metadata.get("validation"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// What the worker is asked: the issue's own words, never a prompt written here.
fn prompt(id: &str, issue: &crate::plan::PlanIssue) -> String {
    format!(
        "Implement {id}: {}\n\nObjective: {}\n\nWork only inside this worktree and only within the issue's scope: {}.",
        issue.title,
        issue.objective,
        issue.scope.join(", ")
    )
}

/// Releases the lease when dropped, so a refusal, a failure and a panic all end it.
struct Release(
    Option<(
        std::sync::Arc<crate::mesh::cooperation::Cooperation>,
        String,
    )>,
);

impl Drop for Release {
    fn drop(&mut self) {
        if let Some((cooperation, key)) = self.0.take() {
            if let Err(e) = cooperation.release(&key) {
                tracing::warn!(claim = %key, "the lease of an implementation was not released: {e}");
            }
        }
    }
}

/// The `implement` module: `implement.issue`, reachable only through `executions.start`
/// because it declares no route, tool or command of its own.
///
/// ```
/// use majordomus_cli::capability::builtin::implement::module;
/// let m = module();
/// assert_eq!(m.id.as_str(), "implement");
/// assert_eq!(m.capabilities.len(), 1);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "implement",
        title: "Implement",
        description: "One issue implemented by one execution: readiness, lease, worktree, worker.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "implement.issue",
                kind: CapabilityKind::Command,
                title: "Implement an issue",
                description: "Judge the issue's readiness, claim its scope as the execution's lease, create its worktree and hand the work to the first available worker adapter, each a step of one execution (ADR 0225). Started only as an execution, through executions.start.",
                input: ImplementInput,
                output: ImplementReport,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: None, http: None, cli: None },
                tags: ["issue", "executions", "worker"],
                cache: crate::capability::model::CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: issue_implement,
            }
            .cancellable(),
        ],
    }
}
