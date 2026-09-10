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
pub mod continuity;
pub(crate) mod deploy;
pub(crate) mod directories;
pub(crate) mod distribution;
pub mod environment;
pub(crate) mod executions;
pub(crate) mod graph;
pub mod health;
pub mod objects;
pub mod obligations;
pub(crate) mod peers;
pub(crate) mod perf;
pub mod plan;
pub(crate) mod product;
pub mod quality;
pub mod release;
pub mod repository;
mod scope;
pub mod server;
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
pub use graph::{GraphInput, GraphList, GRAPHS_URI};
pub use health::{Health, HealthCheck, HealthStatus, HEALTH_URI};
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
pub use server::{Desired, LeaseView, ServerStanding, ServerStatus, ServerView, SERVER_URI};
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
        repository,
        objects,
        capabilities,
        commands,
        graph,
        health,
        continuity,
        obligations,
        deploy,
        executions,
        peers,
        server,
        perf,
        plan,
        directories,
        artifacts,
        environment,
        quality,
        distribution,
        why,
        web,
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
