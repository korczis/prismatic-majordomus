//! The executable capabilities this executable ships: one Rust module per capability
//! module, each owning its typed inputs and outputs, its handlers, its benchmark cases
//! and its `module()` composition; [`modules`] composes the application from them with
//! `compose_modules!`, and that list is the only root composition there is. Adding a
//! capability to an existing module touches that module's file alone; every projection,
//! benchmark target and generated document follows from the descriptor.

pub mod artifacts;
pub mod capabilities;
pub mod continuity;
pub mod deploy;
pub mod directories;
pub mod distribution;
pub mod environment;
pub mod graph;
pub mod health;
pub mod objects;
pub mod peers;
pub mod perf;
pub mod product;
pub mod repository;
mod scope;
mod views;
pub mod web;
pub mod worktree;

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
pub use graph::{GraphInput, GraphList, GRAPHS_URI};
pub use health::{Health, HealthCheck, HealthStatus, HEALTH_URI};
pub use objects::{
    resolve, AnswerView, GetInput, ListInput, ObjectList, Resolved, ResourceView, SearchHit,
    SearchInput, SearchResult, SEARCH_DEFAULT_LIMIT, SEARCH_MAX_LIMIT,
};
pub use peers::{AnnounceInput, PeerList};
pub use repository::{RepositoryReport, REPOSITORY_URI};
pub use scope::{normalise_path, ClassifyInput, ScopeReport, SCOPE_URI};
pub mod why;

pub use views::{Empty, ObjectSummary, ObjectView};
pub use web::{SurfaceReport, SURFACES_URI};
pub use worktree::{
    InspectInput as WorktreeInspectInput, StatusInput as WorktreeStatusInput, WORKTREES_URI,
};

/// The application: its modules, in one place. A new module is one line here; a new
/// capability in an existing module is no line here.
pub fn modules() -> Vec<ModuleDescriptor> {
    compose_modules![
        repository,
        objects,
        capabilities,
        graph,
        health,
        continuity,
        deploy,
        peers,
        perf,
        directories,
        artifacts,
        environment,
        distribution,
        why,
        web,
        worktree,
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
