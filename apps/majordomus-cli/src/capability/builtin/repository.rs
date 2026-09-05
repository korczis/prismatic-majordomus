//! The `repository` module: what the repository and this process's index are.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::registry::Summary;
use crate::index::{RepositoryInfo, State};
use crate::model::Diagnostic;
use crate::scope::Classification;
use crate::{capability, module};

use super::scope::{scope_classify, scope_info, ClassifyInput, ScopeReport, SCOPE_URI};
use super::{get, mcp, Empty};

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
        description: "The repository this process serves: its layer, its git state, the state of the index built from it, and its scope: what a worker reads of it and what it never reads.",
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
            capability! {
                id: "repository.scope",
                title: "The repository scope",
                description: "The scope declaration as read, where it came from (the repository's own or the distribution's default), and every tracked file tallied against it: how many are in, how many are out for each reason, and which.",
                input: Empty,
                output: ScopeReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_scope".into()),
                        resource: Some(McpResource { uri: SCOPE_URI.into(), name: "scope".into() }),
                    }),
                    http: get("/api/v1/scope"),
                    cli: Some(CliExposure { path: vec!["scope".into()] }),
                },
                tags: ["repository", "scope", "introspection"],
                handler: scope_info,
            },
            capability! {
                id: "repository.scope_classify",
                title: "Judge a path against the scope",
                description: "Whether a repository-relative path is in or out of the scope, the reason when it is out, and the pattern or limit that decided; an existing file is judged by name, then size, then content.",
                input: ClassifyInput,
                output: Classification,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_scope_classify"),
                    http: get("/api/v1/scope/classify"),
                    cli: Some(CliExposure { path: vec!["scope".into(), "classify".into()] }),
                },
                tags: ["repository", "scope"],
                handler: scope_classify,
            },
        ],
    }
}
