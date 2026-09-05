//! The `repository` module: what the repository and this process's index are.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::registry::Summary;
use crate::index::{RepositoryInfo, State};
use crate::model::Diagnostic;
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which `repository.info` is read as an MCP resource, and the one URI
/// `objects.get` answers by executing a query rather than by reading the index.
pub const REPOSITORY_URI: &str = "majordomus://repository";

/// The repository, its layer, its git state, and the state of this process's index.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryReport {
    /// The repository: root, layer schema, sections, git state, discovery mode, source classes, kind sources.
    pub repository: RepositoryInfo,
    /// `ok` when every discovered file became an object, `degraded` otherwise.
    pub state: State,
    /// How many objects the index holds.
    pub objects: usize,
    /// Objects per kind.
    pub kinds: std::collections::BTreeMap<String, usize>,
    /// Every diagnostic the index produced.
    pub diagnostics: Vec<Diagnostic>,
    /// The capability registry, counted.
    pub capabilities: Summary,
}

fn repository_info(ctx: &Context, _: Empty) -> Result<RepositoryReport, CapabilityError> {
    let index = &ctx.index;
    Ok(RepositoryReport {
        repository: index.repository.clone(),
        state: index.state,
        objects: index.objects.len(),
        kinds: index
            .kinds()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        diagnostics: index.diagnostics.clone(),
        capabilities: ctx.registry.summary(),
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "repository",
        title: "Repository",
        description: "The repository this process serves: its layer, its git state, and the state of the index built from it.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "repository.info",
                title: "Repository and index state",
                description: "The repository root, layer sections, git state, discovery mode, kinds present, every diagnostic, and the capability registry counted.",
                input: Empty,
                output: RepositoryReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_repository".into()),
                        resource: Some(McpResource { uri: REPOSITORY_URI.into(), name: "repository".into() }),
                    }),
                    http: get("/api/v1/repository"),
                    cli: None,
                },
                tags: ["repository", "introspection"],
                handler: repository_info,
            },
        ],
    }
}
