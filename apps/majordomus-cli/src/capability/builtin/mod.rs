//! The executable capabilities this executable ships: one Rust module per capability
//! module, each owning its typed inputs and outputs, its handlers, its benchmark cases
//! and its `module()` composition; [`modules`] composes the application from them with
//! `compose_modules!`, and that list is the only root composition there is. Adding a
//! capability to an existing module touches that module's file alone; every projection,
//! benchmark target and generated document follows from the descriptor.

pub mod capabilities;
pub mod objects;
pub mod peers;
pub mod perf;
pub mod repository;
mod views;

use crate::compose_modules;

use super::handler::Executable;
use super::model::{HttpExposure, HttpMethod, McpExposure};
use super::module::ModuleDescriptor;

pub use capabilities::{CapabilitiesInput, CapabilityList, CapabilitySummary, DescribeInput};
pub use objects::{
    GetInput, ListInput, ObjectList, SearchHit, SearchInput, SearchResult, SEARCH_DEFAULT_LIMIT,
    SEARCH_MAX_LIMIT,
};
pub use peers::{AnnounceInput, PeerList};
pub use repository::RepositoryReport;
pub use views::{Empty, ObjectSummary, ObjectView};

/// The application: its modules, in one place. A new module is one line here; a new
/// capability in an existing module is no line here.
pub fn modules() -> Vec<ModuleDescriptor> {
    compose_modules![repository, objects, capabilities, peers, perf]
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
