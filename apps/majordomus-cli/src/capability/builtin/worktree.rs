//! The `worktree` module: where linked git worktrees of this repository belong, which ones
//! exist, and whether the layout rule holds.
//!
//! Three questions, all read-only, all answered by [`crate::worktree::WorktreeService`] —
//! the same service the command line and `majordomus doctor` read. Declaring them here is
//! what puts them on MCP (as tools and as a resource), on HTTP, in the OpenAPI document, in
//! the Swagger UI, in the cockpit and in the generated reference, without any of those
//! carrying a route, a schema or a sentence of their own.
//!
//! Creating, removing, migrating and pruning are deliberately *not* here. A capability of
//! this registry never writes — that is the registry's contract, and the shared MCP server
//! rests on it — and those four write to the filesystem. They are command-line operations,
//! and they call the same service, so there is still one implementation of each.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::index::Index;
use crate::worktree::{RootReport, StatusReport, WorktreePolicy, WorktreeReport, WorktreeService};
use crate::{capability, module};

use super::{get, mcp};

/// The URI under which the topology is read as an MCP resource.
pub const WORKTREES_URI: &str = "majordomus://worktrees";

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Nothing to ask: the repository decides the answer, not the caller.
pub struct WorktreeInput {}

impl BenchmarkCases for WorktreeInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("default", WorktreeInput {})]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// How much of each work tree's state to read.
pub struct ListInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Also report whether each work tree has uncommitted or untracked content. It costs one
    /// `git status` per existing work tree, and the layout rule never needs it, so it is off
    /// unless asked for.
    pub status: Option<bool>,
}

impl BenchmarkCases for ListInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("topology-only", ListInput { status: None }),
            NamedCase::new("with-status", ListInput { status: Some(true) }),
        ]
    }
}

// ---------------------------------------------------------------- the service, from an index

/// The worktree policy this repository configured: the `worktree:` section of the canonical
/// policy, read from the object the index already holds. Absent means the defaults, which
/// are the doctrine.
pub fn policy_of(index: &Index) -> WorktreePolicy {
    index
        .objects
        .iter()
        .find(|o| o.kind == "policy")
        .and_then(|o| o.metadata.get("worktree").cloned())
        .and_then(|v| serde_json::from_value::<WorktreePolicy>(v).ok())
        .unwrap_or_default()
}

/// Where the policy was read from, for a message that says where to edit.
pub fn policy_path_of(index: &Index) -> String {
    index
        .repository
        .sections
        .get("policy")
        .cloned()
        .unwrap_or_else(|| ".ai/repo/policy.yaml".to_string())
}

/// The one service, built from the index every projection already has.
pub fn service_of(index: &Index) -> Result<WorktreeService, CapabilityError> {
    WorktreeService::open(
        std::path::Path::new(&index.repository.root),
        policy_of(index),
        &policy_path_of(index),
    )
    .map_err(|e| CapabilityError::Internal(e.to_string()))
}

// ---------------------------------------------------------------- handlers

fn worktree_root(ctx: &Context, _: WorktreeInput) -> Result<RootReport, CapabilityError> {
    Ok(service_of(&ctx.index)?.root_report())
}

fn worktree_list(ctx: &Context, input: ListInput) -> Result<WorktreeReport, CapabilityError> {
    service_of(&ctx.index)?
        .report(input.status.unwrap_or(false))
        .map_err(|e| CapabilityError::Internal(e.to_string()))
}

fn worktree_status(ctx: &Context, _: WorktreeInput) -> Result<StatusReport, CapabilityError> {
    service_of(&ctx.index)?
        .status()
        .map_err(|e| CapabilityError::Internal(e.to_string()))
}

// ---------------------------------------------------------------- the module

/// The `worktree` module: the topology and the layout rule, projected once.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "worktree",
        title: "Worktree topology",
        description: "Where this repository's linked git worktrees belong, which ones exist, and whether each is where the canonical policy says it should be. The container is derived from the repository's identity and the policy's suffix, so the answer is the same from the primary checkout and from inside any linked worktree.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "worktree.root",
                title: "The canonical worktree container",
                description: "The directory every linked worktree of this repository belongs under, derived from the primary checkout's name and the policy's suffix — never from the current directory. Also names the primary checkout, the work tree the call came from, and the common git directory that identifies the repository.",
                input: WorktreeInput,
                output: RootReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_root"),
                    http: get("/api/v1/worktree/root"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "root".into()] }),
                },
                tags: ["worktree", "git", "policy"],
                // The topology changes outside this process — a person runs `git worktree
                // add` in another terminal — so a cached answer would be an answer about a
                // repository that no longer exists.
                cache: CachePolicy::Disabled,
                handler: worktree_root,
            },
            capability! {
                id: "worktree.list",
                title: "Every registered worktree, judged against the policy",
                description: "Every work tree git has registered for this repository, primary first: its path, branch, HEAD, whether it is locked, prunable or the one this call came from, and whether the layout rule holds for it. The primary checkout is exempt by definition. Violations are listed separately with the destination each one would move to.",
                input: ListInput,
                output: WorktreeReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_worktrees".into()),
                        resource: Some(McpResource { uri: WORKTREES_URI.into(), name: "worktrees".into() }),
                    }),
                    http: get("/api/v1/worktrees"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "list".into()] }),
                },
                tags: ["worktree", "git", "policy", "introspection"],
                cache: CachePolicy::Disabled,
                handler: worktree_list,
            },
            capability! {
                id: "worktree.status",
                title: "Where this call is, and whether that is where it belongs",
                description: "The current work tree in one answer: the repository it belongs to, whether it is the primary checkout or a linked worktree, its branch and HEAD, the canonical container, whether the layout rule holds here, whether there is uncommitted work, and how many worktrees of the repository are out of place.",
                input: WorktreeInput,
                output: StatusReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_status"),
                    http: get("/api/v1/worktree/status"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "status".into()] }),
                },
                tags: ["worktree", "git", "policy"],
                cache: CachePolicy::Disabled,
                handler: worktree_status,
            },
        ],
    }
}
