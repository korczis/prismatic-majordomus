//! The `lifecycle` module: what state this repository's work is in, and what a prune may
//! safely remove.
//!
//! Both questions are read-only and both are answered by [`crate::lifecycle::LifecycleService`]
//! — the same service the command line renders. Declaring them here is what puts them on
//! MCP, on HTTP, in the OpenAPI document, in the Swagger UI, in the Cockpit and in the
//! generated reference without any of those carrying a route, a schema or a sentence of its
//! own.
//!
//! Removing is deliberately *not* here. A capability of this registry never writes, and a
//! prune deletes directories; it is a command-line operation that calls
//! [`crate::lifecycle::LifecycleService::cleanup_plan`] and refuses everything the plan did
//! not prove.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::lifecycle::{
    AgingPolicy, CleanupPlan, Clock, LifecycleReport, LifecycleService, Request,
};
use crate::{capability, module};

use super::{get, mcp};

/// The URI under which the lifecycle is read as an MCP resource.
pub const LIFECYCLE_URI: &str = "majordomus://lifecycle";

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// What to measure, and from where.
pub struct LifecycleInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where pull request facts may come from. `auto` (the default) asks the forge with a
    /// bound and falls back to git; `git` asks nothing outside this machine, and the answer
    /// then says plainly that open pull requests were not visible rather than reporting
    /// none.
    pub source: Option<PullRequestSourceInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Also report local branches that have no worktree. Default true: a branch carrying
    /// commits nothing on origin holds is exactly the work that goes unnoticed.
    pub branches: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Where pull request facts may come from.
pub enum PullRequestSourceInput {
    /// Ask the forge with a bound; fall back to git when it does not answer.
    #[default]
    Auto,
    /// Git alone. Nothing leaves this machine.
    Git,
}

impl From<PullRequestSourceInput> for crate::lifecycle::forge::Source {
    fn from(value: PullRequestSourceInput) -> Self {
        match value {
            PullRequestSourceInput::Auto => crate::lifecycle::forge::Source::Auto,
            PullRequestSourceInput::Git => crate::lifecycle::forge::Source::Git,
        }
    }
}

impl BenchmarkCases for LifecycleInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            // git only: a benchmark never reaches the network, and never waits on a forge
            NamedCase::new(
                "git-only",
                LifecycleInput {
                    source: Some(PullRequestSourceInput::Git),
                    branches: Some(false),
                },
            ),
            NamedCase::new(
                "git-only-with-branches",
                LifecycleInput {
                    source: Some(PullRequestSourceInput::Git),
                    branches: Some(true),
                },
            ),
        ]
    }
}

// ---------------------------------------------------------------- the service, from a context

fn refused(e: crate::worktree::WorktreeError) -> CapabilityError {
    match e.exit_code() {
        crate::worktree::EXIT_MISSING => CapabilityError::NotFound(e.to_string()),
        crate::worktree::EXIT_REFUSED => CapabilityError::Refused(e.to_string()),
        _ => CapabilityError::Internal(e.to_string()),
    }
}

/// The thresholds this repository declared, or the named defaults when it declared none.
fn policy_of(ctx: &Context) -> AgingPolicy {
    let declared = crate::repository::Repository::open(std::path::Path::new(
        &ctx.index.repository.root,
    ))
    .and_then(|repository| crate::policy::LoadedPolicy::load(&repository))
    .map(|loaded| loaded.policy.lifecycle)
    .unwrap_or_default();
    AgingPolicy::resolve(&declared)
}

/// What every peer of this shared server announced, as `(id, text)`. A peer that names a
/// branch owns its worktree, and an owned worktree is never called stale and never pruned.
fn peers_of(ctx: &Context) -> Vec<(String, String)> {
    ctx.peers
        .list()
        .into_iter()
        .filter(|p| p.attached)
        .filter_map(|p| {
            // every claim the peer holds, not only the most recent: a session that fans
            // work out holds several at once, and the one that names a branch may be any
            // of them
            let mut text = String::new();
            for claim in &p.claims {
                text.push_str(&claim.intent);
                for path in &claim.scope {
                    text.push(' ');
                    text.push_str(path);
                }
                text.push(' ');
            }
            if text.trim().is_empty() {
                return None;
            }
            Some((p.id.to_string(), text))
        })
        .collect()
}

fn request_of(ctx: &Context, input: &LifecycleInput) -> Request {
    Request {
        source: input.source.unwrap_or_default().into(),
        include_branches: input.branches.unwrap_or(true),
        peers: peers_of(ctx),
    }
}

fn service_of(ctx: &Context) -> LifecycleService {
    LifecycleService::new(
        std::path::PathBuf::from(&ctx.index.repository.root),
        Clock::system(),
        policy_of(ctx),
    )
}

// ---------------------------------------------------------------- handlers

fn lifecycle_report(
    ctx: &Context,
    input: LifecycleInput,
) -> Result<LifecycleReport, CapabilityError> {
    service_of(ctx)
        .report(&request_of(ctx, &input))
        .map_err(refused)
}

fn lifecycle_cleanup(ctx: &Context, input: LifecycleInput) -> Result<CleanupPlan, CapabilityError> {
    service_of(ctx)
        .cleanup_plan(&request_of(ctx, &input))
        .map_err(refused)
}

// ---------------------------------------------------------------- the module

/// The `lifecycle` module: the state of the work, projected once.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "lifecycle",
        title: "The lifecycle of work",
        description: "What state every worktree, branch and pull request of this repository is in, measured rather than remembered: commits reachable from no ref on origin, uncommitted work, the merge that landed a branch and what that merge does not contain, real mergeability decided locally with `git merge-tree` rather than taken from a forge that cannot run this repository's merge driver, the age of everything against thresholds declared once in the policy, and for each one whether removing it is provably safe or which measurement refuses.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "lifecycle.report",
                title: "Where all the work stands",
                description: "Every worktree and every local branch with its lifecycle state, the evidence that produced it, its unique commits and a sample of them, its uncommitted work, the landing that carries it and the residue that landing does not contain, the pull requests that name it, its age, its owner, and whether cleanup is proved safe; plus every pull request with a machine-readable landing state and an age, the thresholds the ages were judged against, and the findings. A report that could not see part of its subject says so in `limitations` and sets `complete` to false rather than reporting an absence as a zero.",
                input: LifecycleInput,
                output: LifecycleReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_lifecycle".into()),
                        resource: Some(McpResource { uri: LIFECYCLE_URI.into(), name: "lifecycle".into() }),
                    }),
                    http: get("/api/v1/lifecycle"),
                    cli: Some(CliExposure { path: vec!["lifecycle".into(), "report".into()] }),
                },
                tags: ["lifecycle", "worktree", "branch", "pull-request", "git"],
                cache: CachePolicy::Disabled,
                handler: lifecycle_report,
            },
            capability! {
                id: "lifecycle.cleanup_plan",
                title: "What a prune may remove, and what it refuses",
                description: "One entry per worktree, split into what a prune has proved safe to remove and what it refuses, each refusal naming the measurement that refuses it and the command that clears it. The default is refusal: a worktree is removable only when it holds no commit reachable from no origin ref, no uncommitted work, no lock, no live owner and no open pull request, and is neither the primary checkout nor the trunk. Planning changes nothing; `majordomus lifecycle prune --apply` removes only what this proved.",
                input: LifecycleInput,
                output: CleanupPlan,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_lifecycle_cleanup"),
                    http: get("/api/v1/lifecycle/cleanup"),
                    cli: Some(CliExposure { path: vec!["lifecycle".into(), "prune".into()] }),
                },
                tags: ["lifecycle", "worktree", "cleanup", "git"],
                cache: CachePolicy::Disabled,
                handler: lifecycle_cleanup,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "lifecycle");
        let expected: &[(&str, &str, &str)] = &[
            (
                "lifecycle.report",
                "majordomus_lifecycle",
                "/api/v1/lifecycle",
            ),
            (
                "lifecycle.cleanup_plan",
                "majordomus_lifecycle_cleanup",
                "/api/v1/lifecycle/cleanup",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
            assert!(
                !executable.capability.cache.is_enabled(),
                "{id} must not cache: worktrees, branches and pull requests all change \
                 outside this process"
            );
        }
    }

    /// Every capability of the module is read-only. The prune that deletes is a command,
    /// not a capability, because the registry never writes.
    #[test]
    fn every_capability_is_a_query() {
        for e in module().capabilities {
            assert!(
                e.capability.kind.is_read_only(),
                "{} writes",
                e.capability.id
            );
        }
    }

    /// The default input asks the forge, and a caller who cannot afford that says `git`.
    #[test]
    fn the_source_defaults_to_asking_the_forge_with_a_bound() {
        assert!(matches!(
            PullRequestSourceInput::default(),
            PullRequestSourceInput::Auto
        ));
        let input = LifecycleInput::default();
        assert!(input.source.is_none(), "absent means the default, not git");
    }
}
