//! The executable capabilities this executable ships: one Rust module per capability
//! module, each owning its typed inputs and outputs, its handlers, its benchmark cases
//! and its `module()` composition; [`modules`] composes the application from them with
//! `compose_modules!`, and that list is the only root composition there is. Adding a
//! capability to an existing module touches that module's file alone; every projection,
//! benchmark target and generated document follows from the descriptor.
//!
//! ```
//! use majordomus_cli::capability::builtin;
//!
//! // the application is its modules, and each module stamps its namespace on what it holds
//! let modules = builtin::modules();
//! assert!(modules.iter().any(|m| m.id.as_str() == "repository"));
//! for module in &modules {
//!     for e in &module.capabilities {
//!         assert_eq!(
//!             e.capability.id.namespace(),
//!             module.id.as_str(),
//!             "{} is composed into the wrong module",
//!             e.capability.id
//!         );
//!     }
//! }
//!
//! // and `all` is the same set flattened, for a registry built without module metadata
//! let flattened = builtin::all().len();
//! assert_eq!(
//!     flattened,
//!     modules.iter().map(|m| m.capabilities.len()).sum::<usize>()
//! );
//! ```

pub mod artifacts;
pub(crate) mod capabilities;
pub mod commands;
pub mod commit;
pub mod continuity;
pub(crate) mod deploy;
pub(crate) mod design;
pub(crate) mod devcontext;
pub mod devtask;
pub(crate) mod directories;
pub(crate) mod distribution;
pub mod environment;
pub mod evidence;
pub(crate) mod executions;
pub mod gates;
pub(crate) mod graph;
pub mod health;
pub mod lifecycle;
pub(crate) mod mesh;
pub(crate) mod models;
pub mod objects;
pub mod obligations;
pub(crate) mod peers;
pub(crate) mod perf;
pub mod plan;
pub(crate) mod product;
pub mod quality;
pub mod release;
pub mod repository;
pub mod rules;
mod scope;
pub mod server;
pub mod session_domain;
pub mod trace;
mod views;
pub mod web;
pub(crate) mod worktree;

use crate::compose_modules;

use super::handler::Executable;
use super::model::{HttpExposure, HttpMethod, McpExposure};
use super::module::ModuleDescriptor;

pub use artifacts::{ArtifactReport, ArtifactState, ArtifactView, ArtifactsInput, ARTIFACTS_URI};
pub use capabilities::{CapabilitiesInput, CapabilityList, CapabilitySummary, DescribeInput};
pub use continuity::{ActiveTask, Continuity, Divergence, OpenSession, Record, CONTINUITY_URI};
pub use deploy::{
    DeploymentCheck, DeploymentList, DeploymentView, GetDeploymentInput, DEPLOYMENTS_URI,
};
// the Cockpit's Design page renders these two; everything else the module declares is
// read as JSON through the executor, like every other capability's output
pub(crate) use design::{DesignReport, TokenList};
pub use devcontext::DEVCONTEXT_POLICY_URI;
pub use devtask::{DevMilestoneInput, DevTaskInput};
pub use directories::{
    ContractView, DirectoriesInput, DirectoryNode, DirectoryReport, DirectoryState,
    DirectoryTallies, EffectiveEntry, DIRECTORIES_URI,
};
// A release artifact and a generated artifact are different things, and `artifacts`
// already answers to the plain names; distribution's carry the `Release` prefix so that
// the two never collide — in this module, and in the one schema component namespace the
// OpenAPI document has.
pub use distribution::{
    BuildReport, CheckState, DistributionReport, InstallCheck, InstallabilityReport,
    ReleaseArtifactInput, ReleaseArtifactView, ReleaseView, ReleasesReport, TargetView,
};
pub use environment::{EnvironmentInput, EnvironmentProvenance, ExplainInput, ENVIRONMENT_URI};
pub use executions::{
    CancelReport, EventHistory, ExecutionLinks, ExecutionList, ExecutionView, ProtocolReport,
    EXECUTIONS_URI, EXECUTION_PROTOCOL_URI,
};
pub use gates::{CompletionInput, GateModelEntry, GateModelReport, COMPLETION_URI, GATES_URI};
pub use graph::{GraphInput, GraphList, GRAPHS_URI};
pub use health::{Health, HealthCheck, HealthStatus, HEALTH_URI};
pub use lifecycle::{
    Balance, ClosedSession, ClosedSessions, Episode, EpisodeStanding, Episodes, Orphan, Pointer,
    PointerLayout, ProviderLifecycle, ProviderLifecycles, Recovery, RuntimeView, Stranded,
    EPISODES_URI, RECOVERY_URI,
};
pub use mesh::{MeshIdentityReport, NodeList, RegisterInput, MESH_URI};
pub use models::{ModelsFilter, ModelsReport, RouteInput, VendorView, MODELS_URI};
pub use objects::{
    resolve, AnswerView, Comparison, DriftedObject, GetInput, ListInput, ObjectList,
    ObjectStanding, Resolved, ResourceView, SearchHit, SearchInput, SearchResult, VerifyInput,
    VerifyReport, SEARCH_DEFAULT_LIMIT, SEARCH_MAX_LIMIT,
};
pub use obligations::{
    Closure, Evidence, Obligation, ObligationClosure, ObligationState, Vocabulary, CLOSURE_URI,
    OBLIGATIONS_URI,
};
pub use peers::{AnnounceInput, PeerList};
pub use plan::{PlanIssueFilter, PlanMilestoneFilter, PlanRecordInput, PLAN_URI};
pub use quality::{QualityAnswer, QualityInput, QUALITY_URI};
pub use repository::{RepositoryReport, REPOSITORY_URI};
pub use scope::{normalise_path, ClassifyInput, ScopeReport, SCOPE_URI};
pub use server::{
    Checkouts, Desired, LeaseView, ServerStanding, ServerStatus, ServerStatusInput, ServerView,
    SERVER_URI,
};
pub use session_domain::{IdentityReport, MachineReport, IDENTITY_URI, MACHINE_URI};
pub use trace::{TraceCommitInput, TraceIssueInput, TraceReportInput, TRACEABILITY_URI};
pub(crate) mod why;

pub use commands::{CommandGraphReport, CommandIndex, CommandSummary};
pub use views::{Empty, ObjectSummary, ObjectView};
pub use web::{SurfaceReport, SURFACES_URI};
pub use worktree::{
    InspectInput as WorktreeInspectInput, StatusInput as WorktreeStatusInput, WORKTREES_URI,
};

/// The application: its modules, in one place. A new module is one line here; a new
/// capability in an existing module is no line here.
pub fn modules() -> Vec<ModuleDescriptor> {
    compose_modules![
        release,
        commit,
        repository,
        objects,
        capabilities,
        commands,
        graph,
        health,
        continuity,
        lifecycle,
        obligations,
        gates,
        deploy,
        evidence,
        rules,
        executions,
        mesh,
        models,
        peers,
        server,
        session_domain,
        perf,
        plan,
        directories,
        devcontext,
        artifacts,
        environment,
        quality,
        distribution,
        why,
        web,
        design,
        devtask,
        worktree,
        trace,
        product
    ]
}

/// Every executable of every module, flattened, for a registry built without module
/// metadata (tests, benchmarks, doctests). The application builds from [`modules`].
pub fn all() -> Vec<Executable> {
    modules().into_iter().flat_map(|m| m.capabilities).collect()
}

pub(crate) fn mcp(tool: &str) -> Option<McpExposure> {
    Some(McpExposure {
        tool: Some(tool.into()),
        resource: None,
    })
}
pub(crate) fn get(path: &str) -> Option<HttpExposure> {
    Some(HttpExposure {
        method: HttpMethod::Get,
        path: path.into(),
    })
}
pub(crate) fn post(path: &str) -> Option<HttpExposure> {
    Some(HttpExposure {
        method: HttpMethod::Post,
        path: path.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{CapabilityKind, Effect};

    /// Only a command may claim to write the repository, and one that does must be a POST.
    ///
    /// Two facts about the whole builtin registry rather than about one module, because the
    /// harm of getting either wrong is not local: the exposure ceiling of every surface is
    /// derived from the effect, and a capability whose effect is understated is projected
    /// onto surfaces whose ceiling exists to exclude it. A query that claimed the effect
    /// would be worse still — bound to GET, announced to MCP as read-only, and followed by
    /// every crawler and prefetcher that ever met the Cockpit.
    ///
    /// The registry refuses both at construction; this states them where a reader looking
    /// for the invariant will find it, and fails on a declaration that slipped past.
    #[test]
    fn only_a_command_writes_the_repository_and_only_by_post() {
        for e in all() {
            let c = &e.capability;
            if c.execution.effect != Effect::RepositoryMutation {
                continue;
            }
            assert_eq!(
                c.kind,
                CapabilityKind::Command,
                "{} claims to write the repository and is not a command",
                c.id
            );
            if let Some(http) = &c.exposure.http {
                assert_eq!(
                    http.method.as_str(),
                    "POST",
                    "{} writes the repository and is reachable by {}",
                    c.id,
                    http.method.as_str()
                );
            }
        }
    }

    /// Every capability that writes the repository, named.
    ///
    /// A list rather than a count, and it is meant to be edited: adding one is a deliberate
    /// widening of what this executable may do to a tracked file, and it should be visible
    /// in a diff rather than absorbed silently. The entries converge as ADR 0040 is worked
    /// through, so this grows — one line per lifecycle command that stops being the shell
    /// tool's alone.
    #[test]
    fn the_capabilities_that_write_the_repository_are_these() {
        // A BTreeSet rather than a Vec and a sort: the set is ordered by construction, and
        // `order-check` counts every sort site in the crate against a baseline it may not
        // exceed. A test that reached for one would spend that budget on itself.
        let executables = all();
        let writers: std::collections::BTreeSet<&str> = executables
            .iter()
            .filter(|e| e.capability.execution.effect == Effect::RepositoryMutation)
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(writers.into_iter().collect::<Vec<_>>(), ["plan.transition"]);
    }
}
