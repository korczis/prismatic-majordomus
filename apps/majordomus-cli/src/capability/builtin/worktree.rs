//! The `worktree` module: the branch-to-worktree topology of this repository, projected.
//!
//! Five questions, all read-only, all answered by [`crate::worktree::WorktreeService`] —
//! the same service the command line renders and the git hooks ask. Declaring them here is
//! what puts them on MCP (as tools and as a resource), on HTTP, in the OpenAPI document, in
//! the Swagger UI, in the Cockpit and in the generated reference, without any of those
//! carrying a route, a schema or a sentence of their own.
//!
//! Creating, migrating, repairing and removing are deliberately *not* here. A capability that
//! writes the repository must declare `Effect::RepositoryMutation`, and the set of those is
//! pinned by a test in `builtin`; moving worktrees on disk is not a thing the shared MCP
//! server takes on. They are command-line operations, they call the same service, and the
//! Cockpit names the exact command for each.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::worktree::{
    migrate, Detail, InspectReport, MigrationPlan, RepositoryTopology, StatusReport, Threshold,
    UncommittedReport, WorktreeError, WorktreeService,
};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the topology is read as an MCP resource.
pub const WORKTREES_URI: &str = "majordomus://worktrees";

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Where to answer from.
pub struct StatusInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A directory inside one of this repository's worktrees; the answer is about that
    /// worktree. Relative to the primary checkout when relative (`.` is the primary
    /// checkout itself). Default: the repository the server was started for. A path in
    /// another repository is refused: the topology answers only about its own.
    pub path: Option<String>,
}

impl BenchmarkCases for StatusInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", StatusInput { path: None }),
            NamedCase::new(
                "primary-checkout",
                StatusInput {
                    path: Some(".".into()),
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// A branch to inspect.
pub struct InspectInput {
    /// The branch, full name (`feature/improve-cli`). It need not exist: the canonical path
    /// derives from the name alone.
    pub branch: String,
}

impl BenchmarkCases for InspectInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "feature-branch",
            InspectInput {
                branch: "feature/example".into(),
            },
        )]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// What to judge uncommitted work against.
pub struct UncommittedInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The age past which work nothing holds is abandoned, as a person writes it (`90s`,
    /// `30m`, `12h`, `1d`). Default: the policy's `session.stranded_after`, which is also
    /// how long an open episode may be silent and still hold the work.
    pub older_than: Option<String>,
}

impl BenchmarkCases for UncommittedInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("policy-threshold", UncommittedInput { older_than: None }),
            NamedCase::new(
                "one-hour",
                UncommittedInput {
                    older_than: Some("1h".into()),
                },
            ),
        ]
    }
}

// ---------------------------------------------------------------- the service, from a context

fn refused(e: WorktreeError) -> CapabilityError {
    match e.exit_code() {
        crate::worktree::EXIT_MISSING => CapabilityError::NotFound(e.to_string()),
        crate::worktree::EXIT_REFUSED => CapabilityError::Refused(e.to_string()),
        _ => CapabilityError::Internal(e.to_string()),
    }
}

/// The one service, over the repository this process serves.
pub fn service_of(ctx: &Context) -> Result<WorktreeService, CapabilityError> {
    WorktreeService::open(Path::new(&ctx.index.repository.root)).map_err(refused)
}

/// The service over a directory the caller named, refused unless it is inside a worktree
/// of the repository this process serves.
fn service_at(ctx: &Context, path: Option<&str>) -> Result<WorktreeService, CapabilityError> {
    let own = service_of(ctx)?;
    let Some(path) = path else {
        return Ok(own);
    };
    // relative to the primary checkout, never to this process's working directory
    let path = {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            Path::new(&ctx.index.repository.root).join(p)
        }
    };
    let other = WorktreeService::open(&path).map_err(refused)?;
    if !other
        .identity()
        .git_common_dir()
        .same_as(own.identity().git_common_dir())
    {
        return Err(CapabilityError::Refused(
            WorktreeError::NotThisRepository {
                path,
                git_common_dir: own.identity().git_common_dir().path.clone(),
            }
            .to_string(),
        ));
    }
    Ok(other)
}

// ---------------------------------------------------------------- handlers

fn worktree_topology(ctx: &Context, _: Empty) -> Result<RepositoryTopology, CapabilityError> {
    service_of(ctx)?.topology(Detail::Full).map_err(refused)
}

fn worktree_status(ctx: &Context, input: StatusInput) -> Result<StatusReport, CapabilityError> {
    service_at(ctx, input.path.as_deref())?
        .status()
        .map_err(refused)
}

fn worktree_inspect(ctx: &Context, input: InspectInput) -> Result<InspectReport, CapabilityError> {
    service_of(ctx)?.inspect(&input.branch).map_err(refused)
}

fn worktree_migration_plan(ctx: &Context, _: Empty) -> Result<MigrationPlan, CapabilityError> {
    migrate::plan(&service_of(ctx)?).map_err(refused)
}

fn worktree_uncommitted(
    ctx: &Context,
    input: UncommittedInput,
) -> Result<UncommittedReport, CapabilityError> {
    let threshold = Threshold::declared_at(
        Path::new(&ctx.index.repository.root),
        input.older_than.as_deref(),
    )
    .map_err(CapabilityError::InvalidInput)?;
    service_of(ctx)?.uncommitted(&threshold).map_err(refused)
}

// ---------------------------------------------------------------- the module

/// The `worktree` module: the topology, projected once.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "worktree",
        title: "Worktree topology",
        description: "Where every linked git worktree of this repository belongs and where each one is. The container is the primary checkout's sibling named with `-wt`, the path under it is the branch name with its hierarchy kept, and both are derived from git's own identity — the common directory, the registered worktrees, the branches — never from a registry, a configuration or the current directory. A worktree somewhere else is a typed diagnostic with a remedy; the migration that repairs it is a command-line operation of the same service.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "worktree.topology",
                title: "The whole topology",
                description: "The repository, the container, the trunk and how it was decided, every registered worktree with its standing (primary, canonical, misplaced, detached, missing), its uncommitted work, its upstream distance and the issue its branch provably names, every local branch with or without a worktree and whether it is eligible for cleanup, every diagnostic with its code and remedy, and the tallies. Read from git on every call: the topology changes outside this process.",
                input: Empty,
                output: RepositoryTopology,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_worktrees".into()),
                        resource: Some(McpResource { uri: WORKTREES_URI.into(), name: "worktrees".into() }),
                    }),
                    http: get("/api/v1/worktrees"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "topology".into()] }),
                },
                tags: ["worktree", "git", "topology", "introspection"],
                cache: CachePolicy::Disabled,
                handler: worktree_topology,
            },
            capability! {
                id: "worktree.status",
                title: "Where this is, and whether that is where it belongs",
                description: "One worktree — the repository's own, or the one holding the directory the caller names — with its standing, its branch, its canonical path, its uncommitted work counted, whether it is where it belongs, and how many errors the whole topology carries. What an agent reads before it starts, and what the guard decides on.",
                input: StatusInput,
                output: StatusReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_status"),
                    http: get("/api/v1/worktrees/status"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "status".into()] }),
                },
                tags: ["worktree", "git", "topology"],
                cache: CachePolicy::Disabled,
                handler: worktree_status,
            },
            capability! {
                id: "worktree.inspect",
                title: "One branch: where its worktree belongs and what is there",
                description: "The canonical path of a branch, derived from its name alone, whether the branch exists, whether something occupies that path, the worktree holding the branch when one does, and what stands in the way of creating or migrating it. The answer for a branch that does not exist yet is the path `worktree create` would use.",
                input: InspectInput,
                output: InspectReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_inspect"),
                    http: get("/api/v1/worktrees/inspect"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "inspect".into()] }),
                },
                tags: ["worktree", "git", "topology"],
                cache: CachePolicy::Disabled,
                handler: worktree_inspect,
            },
            capability! {
                id: "worktree.migration_plan",
                title: "What it would take to bring every worktree home",
                description: "One step per misplaced worktree with a branch: where it is, where it belongs, how it would move, the uncommitted work that moves with it, and what blocks it; plus the exceptions the migration cannot address by design — detached worktrees, stale registrations, the primary checkout off the trunk — each with what a person does about it. Planning changes nothing; `majordomus worktree migrate` applies it with a fingerprint taken before and after every move.",
                input: Empty,
                output: MigrationPlan,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_migration_plan"),
                    http: get("/api/v1/worktrees/migration"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "migrate".into()] }),
                },
                tags: ["worktree", "git", "topology", "migration"],
                cache: CachePolicy::Disabled,
                handler: worktree_migration_plan,
            },
            capability! {
                id: "worktree.uncommitted",
                title: "Uncommitted work in the linked worktrees, and what holds it",
                description: "Every linked worktree whose index or working tree holds changes no commit carries: how many paths (staged, unstaged, untracked, conflicted), when the oldest change was made (the earliest modification time among the changed paths, the index file's standing in for a deleted one), and what holds the work — an active task record, an open episode that is not stranded, or a process whose working directory is inside. Work nothing holds and older than the threshold is `abandoned`, with the command that would put it on a remote `rescue/` ref; it is suggested, never run. The threshold is the policy's `session.stranded_after` unless `older_than` names another. Reads every worktree and writes none: every status runs without optional locks, so no index is refreshed.",
                input: UncommittedInput,
                output: UncommittedReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_uncommitted"),
                    http: get("/api/v1/worktrees/uncommitted"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "uncommitted".into()] }),
                },
                tags: ["worktree", "git", "continuity", "introspection"],
                cache: CachePolicy::Disabled,
                handler: worktree_uncommitted,
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
        assert_eq!(m.id.as_str(), "worktree");
        let expected: &[(&str, &str, &str)] = &[
            (
                "worktree.topology",
                "majordomus_worktrees",
                "/api/v1/worktrees",
            ),
            (
                "worktree.status",
                "majordomus_worktree_status",
                "/api/v1/worktrees/status",
            ),
            (
                "worktree.inspect",
                "majordomus_worktree_inspect",
                "/api/v1/worktrees/inspect",
            ),
            (
                "worktree.migration_plan",
                "majordomus_worktree_migration_plan",
                "/api/v1/worktrees/migration",
            ),
            (
                "worktree.uncommitted",
                "majordomus_worktree_uncommitted",
                "/api/v1/worktrees/uncommitted",
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
                "{id} must not cache: the topology changes outside this process"
            );
        }
        let resource = m.capabilities[0]
            .capability
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref());
        assert_eq!(resource.map(|r| r.uri.as_str()), Some(WORKTREES_URI));
    }

    /// Every capability of the module is read-only: the registry's contract, and the
    /// reason the mutations live on the command line.
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
}
