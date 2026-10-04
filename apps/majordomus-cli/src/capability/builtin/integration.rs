//! The `integration` module: the pull-request integration queue, projected (ADR 0101).
//!
//! Three questions, all read-only and all offline. Each renders the queue
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
    drain::{IntegrationEvent, IntegrationLease, IntegrationLeaseState},
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
}
