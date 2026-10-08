//! The `integration` module: the pull-request integration queue, projected (ADR 0101).
//!
//! Seven questions, all read-only and all offline but the dry-run proof and the batch gate. Each renders the queue
//! [`crate::integration::queue_of`] builds from the last recorded forge observation and
//! this clone's fetched master — the value `majordomus prs status` prints — so the HTTP
//! route, the MCP tool, the Cockpit and the command line cannot disagree. None reaches the
//! network: observing the forge is `majordomus prs refresh`, and merging is
//! `majordomus prs drain`, both command-line operations of the same module, because a
//! capability that writes the repository or a remote must be declared as one and the
//! shared server does not take that on.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::integration::{
    batch::BatchCheck,
    compose::ComposeReport,
    drain::{
        CleanupItem, IntegrationEvent, IntegrationLease, IntegrationLeaseState, LeftBranchReport,
    },
    metrics::{self, IntegrationThroughput},
    IntegrationQueue, PullRequestAssessment,
};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The queue, or why there is none.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationStatus {
    /// Whether a forge observation is recorded in this checkout.
    pub observed: bool,
    /// Why there is no queue, when there is none.
    pub reason: Option<String>,
    /// The queue.
    pub queue: Option<IntegrationQueue>,
    /// The base branch's integration lease as an observer reads it: `None` when nobody
    /// holds it (or nothing is observed, so no base is known).
    #[serde(default)]
    pub lease: Option<IntegrationLeaseState>,
    /// The last merge the executor recorded, from the audit trail.
    #[serde(default)]
    pub last_merge: Option<IntegrationEvent>,
    /// How fast the executor has turned work into master over the last
    /// [`THROUGHPUT_WINDOW_DAYS`] days, folded from the same trail.
    pub throughput: IntegrationThroughput,
}

/// What cleanup would do, decided offline: the pull requests it would close or leave for a
/// person, from the recorded observation, and the last branch report `prs cleanup` recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationCleanup {
    /// Whether a forge observation is recorded in this checkout.
    pub observed: bool,
    /// Why there is no plan, when there is none.
    pub reason: Option<String>,
    /// When the forge was observed, when it was.
    pub observed_at: Option<String>,
    /// The plan, in rank order: `would_close` for what is provably on master or superseded by
    /// a successor that landed, `left_for_a_person` for weak evidence and for what a person
    /// marked obsolete. Nothing here closes anything.
    pub items: Vec<CleanupItem>,
    /// The branches merged pull requests left on origin, as `prs cleanup` last read them;
    /// `None` when it never ran in this checkout. Never read from the network here.
    pub branches: Option<LeftBranchReport>,
    /// How old that report is, in seconds, when its time reads.
    pub branches_age_seconds: Option<u64>,
}

/// [`IntegrationCleanup`] for the checkout at `root`, at `now` (seconds since the epoch):
/// [`crate::integration::drain::cleanup_plan`] over [`crate::integration::queue_of`], which
/// writes nothing, and the recorded branch report with its age. No network, no write.
pub fn cleanup_status(root: &Path, now: u64) -> IntegrationCleanup {
    let branches = crate::integration::drain::recorded_left_branches(root);
    let branches_age_seconds = branches
        .as_ref()
        .and_then(|b| crate::peers::epoch_seconds(&b.read_at))
        .and_then(|at| u64::try_from(at).ok())
        .map(|at| now.saturating_sub(at));
    match crate::integration::queue_of(root) {
        Ok(q) => IntegrationCleanup {
            observed: true,
            reason: None,
            observed_at: Some(q.observed_at.clone()),
            items: crate::integration::drain::cleanup_plan(&q),
            branches,
            branches_age_seconds,
        },
        Err(reason) => IntegrationCleanup {
            observed: false,
            reason: Some(reason),
            observed_at: None,
            items: Vec::new(),
            branches,
            branches_age_seconds,
        },
    }
}

/// The window [`IntegrationStatus::throughput`] is folded over.
pub const THROUGHPUT_WINDOW_DAYS: u64 = 7;

/// One pull request to explain.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PullRequestExplainInput {
    /// The pull request number.
    pub number: u64,
}

impl BenchmarkCases for PullRequestExplainInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "a-number",
            PullRequestExplainInput { number: 1 },
        )]
    }
}

/// One pull request's assessment and rank, or why it has none.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationExplanation {
    /// Whether the pull request is in the observed queue.
    pub found: bool,
    /// Why not, when not.
    pub reason: Option<String>,
    /// Its position in the ranked queue, 1-based.
    pub rank: Option<usize>,
    /// How many open pull requests were ranked.
    pub of: usize,
    /// When the forge was observed.
    pub observed_at: Option<String>,
    /// The assessment.
    pub assessment: Option<PullRequestAssessment>,
}

/// How large a batch to plan.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchPlanInput {
    /// At most this many members, for this plan: lowers the policy's
    /// `integration.batch.max_members` and never raises it. Absent, the policy's value is the
    /// size; with neither there is no plan, and the reason names the key. A batch has at
    /// least two: a smaller number is answered with the refusal, not with a plan.
    #[serde(default)]
    pub max: Option<usize>,
}

impl BenchmarkCases for BatchPlanInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("policy-size", BatchPlanInput { max: None }),
            NamedCase::new("four-members", BatchPlanInput { max: Some(4) }),
        ]
    }
}

/// What to judge.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchCheckInput {
    /// The base to judge against: a ref or a commit. Absent, the integration base as this
    /// clone has it (`origin/<base>`).
    #[serde(default)]
    pub base: Option<String>,
    /// The head to judge: a ref or a commit. Absent, `HEAD`.
    #[serde(default)]
    pub head: Option<String>,
}

impl BenchmarkCases for BatchCheckInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // waived where it is declared: the gate reads the forge
        vec![NamedCase::new("head", BatchCheckInput::default())]
    }
}

/// The plan of a batch, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationBatchPlan {
    /// Whether a forge observation is recorded in this checkout.
    pub observed: bool,
    /// Why there is no plan, when there is none.
    pub reason: Option<String>,
    /// The dry run of `prs compose --max <max>`: the plan, what would be done with it, and
    /// what the observation it rests on cannot vouch for.
    pub report: Option<ComposeReport>,
    /// Why `--apply` would be refused before it decided anything: the trail does not yet
    /// hold the verified merge that unlocks composition (ADR 0114 D7). `None` when it does.
    pub rollout: Option<String>,
    /// Why `--apply` would be refused whatever the trail holds: the layer does not hold
    /// ADR 0114 as accepted, which is a person's act. `None` when it does.
    #[serde(default)]
    pub decision: Option<String>,
}

/// [`IntegrationBatchPlan`] for the checkout at `root`: [`crate::integration::compose::plan`]
/// over the recorded observation, and the rollout record read from the trail. No network, no
/// lease, no write.
pub fn batch_plan(root: &Path, max: Option<usize>) -> IntegrationBatchPlan {
    use crate::integration::compose;
    let rollout = compose::rollout_refused(&crate::integration::drain::events(root));
    let decision = compose::decision_refused(root).map(|refusal| refusal.to_string());
    // the size is the policy's, lowered by `max`; with neither there is nothing to plan
    let max = match compose::max_members(root, max) {
        Ok(max) => max,
        Err(refusal) => {
            return IntegrationBatchPlan {
                observed: matches!(crate::integration::load_observation(root), Ok(Some(_))),
                reason: Some(refusal.to_string()),
                report: None,
                rollout,
                decision,
            }
        }
    };
    match compose::plan(root, max) {
        Ok(report) => IntegrationBatchPlan {
            observed: true,
            reason: None,
            report: Some(report),
            rollout,
            decision,
        },
        Err(reason) => IntegrationBatchPlan {
            observed: false,
            reason: Some(reason),
            report: None,
            rollout,
            decision,
        },
    }
}

/// The audit trail.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationEvents {
    /// Every recorded integration action, oldest first.
    pub events: Vec<IntegrationEvent>,
}

fn root(ctx: &Context) -> &Path {
    Path::new(&ctx.index.repository.root)
}

fn integration_queue(ctx: &Context, _: Empty) -> Result<IntegrationStatus, CapabilityError> {
    let root = root(ctx);
    let trail = crate::integration::drain::events(root);
    let last_merge = trail
        .iter()
        .rev()
        .find(|e| e.action == crate::integration::drain::IntegrationAction::MergeSucceeded)
        .cloned();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let throughput = metrics::throughput(&trail, now, THROUGHPUT_WINDOW_DAYS);
    Ok(match crate::integration::queue_of(root) {
        Ok(q) => IntegrationStatus {
            observed: true,
            reason: None,
            lease: IntegrationLease::read(root, &q.base).ok().flatten(),
            queue: Some(q),
            last_merge,
            throughput,
        },
        Err(reason) => IntegrationStatus {
            observed: false,
            reason: Some(reason),
            queue: None,
            lease: None,
            last_merge,
            throughput,
        },
    })
}

fn integration_explain(
    ctx: &Context,
    input: PullRequestExplainInput,
) -> Result<IntegrationExplanation, CapabilityError> {
    let q = match crate::integration::queue_of(root(ctx)) {
        Ok(q) => q,
        Err(reason) => {
            return Ok(IntegrationExplanation {
                found: false,
                reason: Some(reason),
                rank: None,
                of: 0,
                observed_at: None,
                assessment: None,
            })
        }
    };
    let rank = q
        .assessments
        .iter()
        .position(|a| a.number == input.number)
        .map(|i| i + 1);
    Ok(IntegrationExplanation {
        found: rank.is_some(),
        reason: rank.is_none().then(|| {
            format!(
                "#{} is not among the {} open pull request(s) observed",
                input.number, q.tallies.open
            )
        }),
        rank,
        of: q.assessments.len(),
        observed_at: Some(q.observed_at.clone()),
        assessment: q.get(input.number).cloned(),
    })
}

fn integration_events(ctx: &Context, _: Empty) -> Result<IntegrationEvents, CapabilityError> {
    Ok(IntegrationEvents {
        events: crate::integration::drain::events(root(ctx)),
    })
}

fn integration_cleanup(ctx: &Context, _: Empty) -> Result<IntegrationCleanup, CapabilityError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(cleanup_status(root(ctx), now))
}

fn integration_compose(
    ctx: &Context,
    input: BatchPlanInput,
) -> Result<IntegrationBatchPlan, CapabilityError> {
    Ok(batch_plan(root(ctx), input.max))
}

/// The gate, for the checkout the context serves. "Cannot run" — a base this clone does not
/// have, a git or a forge that could not answer — is a refusal with the reason, never a
/// verdict.
fn integration_batch_check(
    ctx: &Context,
    input: BatchCheckInput,
) -> Result<BatchCheck, CapabilityError> {
    use crate::integration::batch;
    let root = root(ctx);
    let base = match input.base {
        Some(base) => base,
        None => batch::default_base(root).map_err(CapabilityError::Refused)?,
    };
    let head = input.head.unwrap_or_else(|| "HEAD".to_string());
    batch::check(root, &base, &head, || batch::open_heads(root))
        .map_err(|e| CapabilityError::Refused(format!("batch-check cannot run: {e}")))
}

fn integration_prove_dry_run(
    ctx: &Context,
    _: Empty,
) -> Result<crate::integration::proof::DryRunProof, CapabilityError> {
    crate::integration::proof::prove_dry_run(root(ctx)).map_err(CapabilityError::Refused)
}

/// The `integration` module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "integration",
        title: "Pull-request integration",
        description: "Every open pull request classified against the current master — ready, needs refresh, waiting for checks, review or a dependency, draft, needs repair, conflicting, blocked, unsafe (auto-merge armed), redundant (its work is on master already), superseded (by a declared successor that landed, named in superseded_by), possibly redundant, other base or unknown — each with the master and head it was decided against, its reasons, its evidence, its risk and its overlaps, ranked deterministically; and the audit trail of the executor that merges the next provably safe one, one at a time. The relation to master is decided by git with this repository's own merge drivers, because the forge cannot run the derived-file driver. Read from the last recorded forge observation; the one exception is the dry-run proof, which observes the forge itself because the observation is part of what it proves moves nothing.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "integration.queue",
                title: "The integration queue",
                description: "The ranked queue: the repository, the base and the master commit every assessment was decided against, when the forge was observed, the policy in force (the branch protection's required checks and reviews, the label policy, the merge method: a merge commit, or none when the repository allows none), every open pull request's assessment in rank order — an actionable one with how long it has waited for the executor and how often another was chosen instead — the next merge, the pull requests that need master brought in, the starving ones, the tallies by disposition and lane, and the diagnostics, a stale observation first; beside it, who holds the base branch's integration lease and the last merge the executor recorded. `observed: false` with the reason when this checkout has recorded no observation.",
                input: Empty,
                output: IntegrationStatus,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_pull_requests"),
                    http: get("/api/v1/pull-requests"),
                    cli: Some(CliExposure { path: vec!["prs".into(), "status".into()] }),
                },
                tags: ["integration", "pull-requests", "git", "queue"],
                cache: CachePolicy::Disabled,
                handler: integration_queue,
            },
            capability! {
                id: "integration.explain",
                title: "Why one pull request is where it is",
                description: "One pull request's assessment — disposition, lane, reasons, every gate of the policy with whether it passed, next action, the master and head it was decided against and when the forge was observed, required checks, review, relation to master, dependencies, overlaps, risk with its factors, and every piece of evidence — with its rank in the queue. `found: false` with the reason when it is not open or nothing is observed.",
                input: PullRequestExplainInput,
                output: IntegrationExplanation,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_pull_request_explain"),
                    http: get("/api/v1/pull-requests/explain"),
                    cli: Some(CliExposure { path: vec!["prs".into(), "explain".into()] }),
                },
                tags: ["integration", "pull-requests", "explain"],
                cache: CachePolicy::Disabled,
                handler: integration_explain,
            },
            capability! {
                id: "integration.events",
                title: "The integration audit trail",
                description: "Every action the repository's executors recorded, from any of its worktrees, oldest first: the lease taken and given back, observations, selections, stale decisions, each act's attempt before it and its outcome after — merges with the master before and after, refusals, refreshes, verification failures and closures — each with a typed action, its actor, the pull request, the head, the decision's reasons and, on an act, the evidence it was decided on. The trail is one file under the common git directory, so every worktree reads the same one.",
                input: Empty,
                output: IntegrationEvents,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_integration_events"),
                    http: get("/api/v1/pull-requests/events"),
                    cli: Some(CliExposure { path: vec!["prs".into(), "events".into()] }),
                },
                tags: ["integration", "pull-requests", "audit"],
                cache: CachePolicy::Disabled,
                handler: integration_events,
            },
            capability! {
                id: "integration.cleanup",
                title: "What cleanup would do",
                description: "The cleanup plan, decided offline from the recorded observation: every open pull request cleanup would close — redundant (its work is on master already) or superseded (by a declared successor that landed) — and every one it leaves for a person — possibly redundant (weak evidence) or obsolete (a person marked it, owner decision D3) — each with its disposition and the reasons that decided it, in rank order; beside it, the branches merged pull requests left on origin as `majordomus prs cleanup` last read them, with when and how long ago. A read: it closes, deletes and asks the forge for nothing — `majordomus prs cleanup` reads the forge and `--apply` closes. `observed: false` with the reason when this checkout has recorded no observation.",
                input: Empty,
                output: IntegrationCleanup,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_pull_requests_cleanup"),
                    http: get("/api/v1/pull-requests/cleanup"),
                    cli: None,
                },
                tags: ["integration", "pull-requests", "cleanup"],
                cache: CachePolicy::Disabled,
                handler: integration_cleanup,
            },
            capability! {
                id: "integration.compose",
                title: "The plan of a batch",
                description: "What `majordomus prs compose` would compose, decided offline from the recorded observation (ADR 0114): the base and the master commit every member was decided against, the members in composition order — each an open pull request on the base that is not a draft, carries no holding label, has its head in this repository, satisfies the review policy and has every required check passed on its own head, which may be behind master, with every declared dependency landed or placed before it, taken in rank order up to the policy's `integration.batch.max_members`, which `max` may lower and never raise — each with its head and title, and every other open pull request with the one typed reason it is left out; then what would be done (`would_compose`, or the refusal: fewer than two eligible is not a batch), the queue's diagnostics, and whether the rollout record or the status of ADR 0114 in the layer would refuse `--apply`. With no `max` and no policy key there is no plan, and the reason names the key. A read: it merges, pushes and records nothing — `majordomus prs compose --apply` is the act, under the integration lease. `observed: false` with the reason when this checkout has recorded no observation.",
                input: BatchPlanInput,
                output: IntegrationBatchPlan,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_pull_requests_compose"),
                    http: get("/api/v1/pull-requests/compose"),
                    cli: None,
                },
                tags: ["integration", "pull-requests", "batch"],
                cache: CachePolicy::Disabled,
                handler: integration_compose,
            },
            capability! {
                id: "integration.batch_check",
                title: "A composed branch that is not a batch is refused",
                description: "The gate of ADR 0114 D5, `majordomus prs batch-check`. Walks the first-parent line from the merge base of `base` and `head`: a merge there with exactly two parents whose second is the current head of another open pull request of this repository is a member merge; a merge whose other parents are all ancestors of the base is a merge of the base. A branch is a batch to be judged when it merges two or more pull requests that way, or when it adds or changes a manifest under `.ai/repo/integration/batches/` relative to the base, whatever it merges; neither is `not_a_batch` — one merge and no manifest is a stack. A batch to be judged must add or change exactly one manifest, whose members are exactly the member merges' pull requests in first-parent order, at least two of them, each `head` the merge's second parent and each `merge_commit` the merge; and every other commit on the line that is not a merge of the base may change only that manifest, the version files `release bump` writes and paths that are `merge=derived`. Otherwise `refused`, with one typed finding per disagreement naming the commit, the path, or the manifest line and member. Answers the base, the merge base, the head that was judged (the forge's test merge of a pull request is looked through to the pull request's own head), whether the forge was read, the member merges, the manifest, the verdict and the findings. The open pull requests are read from the forge through the GitHub CLI, and only when git alone cannot decide; when they are needed and cannot be read, or git cannot answer, the call is refused with the reason — it never reports clean because it could not look. A read: nothing is fetched, stored or recorded.",
                input: BatchCheckInput,
                output: BatchCheck,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: None,
                    http: get("/api/v1/pull-requests/batch-check"),
                    cli: Some(CliExposure { path: vec!["prs".into(), "batch-check".into()] }),
                },
                tags: ["integration", "pull-requests", "batch", "gate", "live"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: integration_batch_check,
            },
            capability! {
                id: "integration.prove_dry_run",
                title: "Proof that a dry run moves nothing",
                description: "Runs the executor's non-mutating cycle — refresh, plan, drain --dry-run and cleanup without --apply — between two snapshots of everything it could move if it were wrong: every ref origin serves, every open pull request's number, head, state and labels, the integration audit trail, the executor's lease, and every local ref outside the two namespaces the refresh mirrors. `ok` is true exactly when the snapshots are equal and refs/remotes/origin/<base> and every refs/majordomus/prs/<n> equal what origin serves. A read that reaches the network: the refresh asks the forge through the GitHub CLI and fetches the base and the pull-request heads, and like every read it rewrites the observation, relation and summary caches. It merges, closes and pushes nothing, and takes no input that could make it.",
                input: Empty,
                output: crate::integration::proof::DryRunProof,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_pull_requests_prove_dry_run"),
                    http: get("/api/v1/pull-requests/prove-dry-run"),
                    cli: Some(CliExposure { path: vec!["prs".into(), "prove-dry-run".into()] }),
                },
                tags: ["integration", "pull-requests", "proof", "live"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: integration_prove_dry_run,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_capability_is_a_query_and_none_caches() {
        let m = module();
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "integration.queue",
                "integration.explain",
                "integration.events",
                "integration.cleanup",
                "integration.compose",
                "integration.batch_check",
                "integration.prove_dry_run"
            ]
        );
        for e in m.capabilities {
            assert!(
                e.capability.kind.is_read_only(),
                "{} writes",
                e.capability.id
            );
            assert!(
                !e.capability.cache.is_enabled(),
                "{} caches",
                e.capability.id
            );
        }
    }

    #[test]
    fn a_batch_plan_without_an_observation_says_so_and_names_the_rollout() {
        let dir = std::env::temp_dir().join(format!(
            "mj-capability-compose-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let init = std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(init.success());
        let plan = batch_plan(&dir, Some(4));
        assert!(!plan.observed);
        assert!(plan.report.is_none());
        assert!(
            plan.reason.as_deref().unwrap_or("").contains("prs refresh"),
            "{plan:?}"
        );
        // the trail holds no verified merge: `--apply` would be refused, and the read says so
        assert!(
            plan.rollout
                .as_deref()
                .unwrap_or("")
                .contains("ADR 0114 D7"),
            "{plan:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
