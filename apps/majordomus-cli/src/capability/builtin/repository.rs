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

/// The capability `majordomus scope <PATHS>` answers with when it is given paths.
///
/// Named here, beside the declaration, because the command module must reach it by
/// identity: it has no command line of its own to be found by.
///
/// ```
/// use majordomus_cli::capability::builtin::repository::SCOPE_CLASSIFY;
/// assert_eq!(SCOPE_CLASSIFY, "repository.scope_classify");
/// ```
pub const SCOPE_CLASSIFY: &str = "repository.scope_classify";

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
                // No CLI exposure: `majordomus scope <PATHS>` is one command that answers
                // with this capability when it is given paths and with `repository.scope`
                // when it is not, and clap has no `scope classify` subcommand for anybody
                // to type. The path used to be declared here so that the command module
                // could find the id with `by_cli`, which made a lookup key look like a
                // public command line — `capabilities describe`, `docs/generated/cli.md`
                // and the site's registry all repeated a command that does not exist. The
                // command module names [`SCOPE_CLASSIFY`] instead, and this exposure says
                // what is true: the capability is reached through the command above.
                exposure: Exposure {
                    mcp: mcp("majordomus_scope_classify"),
                    http: get("/api/v1/scope/classify"),
                    cli: None,
                },
                tags: ["repository", "scope"],
                handler: scope_classify,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist, and every projection — the MCP
    /// tool, the HTTP route, the OpenAPI operation, the benchmark target — is derived from
    /// it. A refactor that dropped an exposure or renamed a route would still compile, and
    /// the suites that exercise the behaviour behind it would still pass. This is the
    /// assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "repository");
        let expected: &[(&str, &str, &str)] = &[
            (
                "repository.info",
                "majordomus_repository",
                "/api/v1/repository",
            ),
            ("repository.scope", "majordomus_scope", "/api/v1/scope"),
            (
                "repository.scope_classify",
                "majordomus_scope_classify",
                "/api/v1/scope/classify",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
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
        }
    }
}
