//! The `capabilities` module: the registry's introspection of itself.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, Capability, CapabilityKind, CliExposure, Exposure, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::capability::registry::Summary;
use crate::{capability, module};

use super::{get, mcp};

// ---------------------------------------------------------------- capabilities.list

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `capabilities.list`: optional filters by kind and by projection.
pub struct CapabilitiesInput {
    /// Only capabilities of this kind: `query`, `command` or `resource`.
    #[serde(default)]
    pub kind: Option<String>,
    /// Only capabilities exposed through this projection: `mcp`, `http` or `cli`.
    #[serde(default)]
    pub exposure: Option<String>,
}

impl BenchmarkCases for CapabilitiesInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", CapabilitiesInput::default()),
            NamedCase::new(
                "queries",
                CapabilitiesInput {
                    kind: Some("query".into()),
                    exposure: None,
                },
            ),
        ]
    }
}

/// One capability as a listing shows it: everything the descriptor says except its two
/// schemas, which `capabilities.describe` answers for one capability. A listing of a
/// repository's registry runs to hundreds of entries; their schemas would be megabytes
/// of the same object view repeated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CapabilitySummary {
    /// The canonical identity.
    pub id: crate::capability::CapabilityId,
    /// The module that composes it.
    pub module: crate::capability::ModuleId,
    /// Query, command or resource.
    pub kind: CapabilityKind,
    /// The short name.
    pub title: String,
    /// The one-paragraph description.
    pub description: String,
    /// Where it came from.
    pub provenance: crate::capability::Provenance,
    /// Where it is projected.
    pub exposure: Exposure,
    /// Where it stands.
    pub stability: Stability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Free tags.
    pub tags: Vec<String>,
    /// Whether it is a benchmark target.
    pub benchmark: crate::capability::BenchmarkPolicy,
    /// Whether the executor keeps its results.
    pub cache: CachePolicy,
}

impl From<&Capability> for CapabilitySummary {
    fn from(c: &Capability) -> Self {
        CapabilitySummary {
            id: c.id.clone(),
            module: c.module.clone(),
            kind: c.kind,
            title: c.title.clone(),
            description: c.description.clone(),
            provenance: c.provenance.clone(),
            exposure: c.exposure.clone(),
            stability: c.stability,
            tags: c.tags.clone(),
            benchmark: c.benchmark,
            cache: c.cache,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `capabilities.list`: the matching capabilities, summarised, and the
/// registry counted.
pub struct CapabilityList {
    /// How many capabilities matched the filters.
    pub count: usize,
    /// The whole registry, counted by kind, stability and projection.
    pub summary: Summary,
    /// The matching capabilities, by id, without their schemas.
    pub capabilities: Vec<CapabilitySummary>,
}

fn capabilities_list(
    ctx: &Context,
    input: CapabilitiesInput,
) -> Result<CapabilityList, CapabilityError> {
    let kind = match input.kind.as_deref() {
        None => None,
        Some("query") => Some(CapabilityKind::Query),
        Some("command") => Some(CapabilityKind::Command),
        Some("resource") => Some(CapabilityKind::Resource),
        Some(other) => {
            return Err(CapabilityError::InvalidInput(format!(
                "kind '{other}' is not query, command or resource"
            )))
        }
    };
    let exposure = match input.exposure.as_deref() {
        None | Some("mcp") | Some("http") | Some("cli") => input.exposure.clone(),
        Some(other) => {
            return Err(CapabilityError::InvalidInput(format!(
                "exposure '{other}' is not mcp, http or cli"
            )))
        }
    };
    let capabilities: Vec<CapabilitySummary> = ctx
        .registry
        .iter()
        .filter(|c| kind.is_none_or(|k| c.kind == k))
        .filter(|c| match exposure.as_deref() {
            None => true,
            Some("mcp") => c.exposure.mcp.is_some(),
            Some("http") => c.exposure.http.is_some(),
            Some("cli") => c.exposure.cli.is_some(),
            Some(_) => false,
        })
        .map(CapabilitySummary::from)
        .collect();
    Ok(CapabilityList {
        count: capabilities.len(),
        summary: ctx.registry.summary(),
        capabilities,
    })
}

// ---------------------------------------------------------------- capabilities.describe

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `capabilities.describe`: which capability.
pub struct DescribeInput {
    /// The canonical id, e.g. `repository.info` or `rule.majordomus.scope-integrity@1`.
    pub id: String,
}

impl BenchmarkCases for DescribeInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "repository-info",
            DescribeInput {
                id: "repository.info".into(),
            },
        )]
    }
}

fn capabilities_describe(
    ctx: &Context,
    input: DescribeInput,
) -> Result<Capability, CapabilityError> {
    ctx.registry
        .get(&input.id)
        .cloned()
        .ok_or_else(|| CapabilityError::NotFound(format!("unknown capability: {}", input.id)))
}

// ---------------------------------------------------------------- capabilities.projections

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `capabilities.projections`: which rows, and whether only the unmet claims.
pub struct ProjectionsInput {
    /// Only capabilities composed in this module.
    #[serde(default)]
    pub module: Option<String>,
    /// Only the capabilities whose declared exposures are not all answered by their
    /// surface. Empty is the closure the rule asks for.
    #[serde(default)]
    pub unmet: bool,
}

impl BenchmarkCases for ProjectionsInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", ProjectionsInput::default()),
            NamedCase::new(
                "unmet",
                ProjectionsInput {
                    module: None,
                    unmet: true,
                },
            ),
        ]
    }
}

fn capabilities_projections(
    ctx: &Context,
    input: ProjectionsInput,
) -> Result<crate::capability::closure::Matrix, CapabilityError> {
    // the clap tree is a pure function of the declaration compiled into this executable:
    // no repository is read, and the walk is the same one `cli::validate` runs
    let mut m = crate::capability::closure::matrix(&ctx.registry, &crate::cli::tree());
    if let Some(module) = &input.module {
        if !ctx.registry.modules().any(|k| k.id.as_str() == module) {
            return Err(CapabilityError::InvalidInput(format!(
                "no module named '{module}'"
            )));
        }
        m.rows.retain(|r| &r.module == module);
    }
    if input.unmet {
        m.rows.retain(|r| !r.closed);
    }
    Ok(m)
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "capabilities",
        title: "Capabilities",
        description: "The registry seen through itself: every capability with its kind, stability, provenance, exposures, benchmark and cache policy, and one capability in full.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "capabilities.list",
                title: "List capabilities",
                description: "Every capability of this executable and this repository, summarised: kind, module, stability, provenance, the projections it declares, its benchmark and cache policy; the schemas are answered by capabilities.describe.",
                input: CapabilitiesInput,
                output: CapabilityList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_capabilities"),
                    http: get("/api/v1/capabilities"),
                    cli: Some(CliExposure { path: vec!["capabilities".into(), "list".into()] }),
                },
                tags: ["introspection"],
                cache: CachePolicy::Process { max_entries: 16, ttl_seconds: None },
                handler: capabilities_list,
            },
            capability! {
                id: "capabilities.describe",
                title: "Describe one capability",
                description: "One capability by canonical id: its kind, schemas, provenance, stability, exposures, benchmark and cache policy.",
                input: DescribeInput,
                output: Capability,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_capability"),
                    http: get("/api/v1/capability"),
                    cli: Some(CliExposure { path: vec!["capabilities".into(), "describe".into()] }),
                },
                tags: ["introspection"],
                handler: capabilities_describe,
            },
            capability! {
                id: "capabilities.projections",
                title: "Where each capability is projected",
                description: "A row per capability with the command line, HTTP route, MCP tool and MCP resource it reaches, whether every exposure it declares is answered by that surface, and the runnable commands no capability claims. Derived from the registry and the clap declaration; nothing is written down.",
                input: ProjectionsInput,
                output: crate::capability::closure::Matrix,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_projections"),
                    http: get("/api/v1/capabilities/projections"),
                    cli: Some(CliExposure { path: vec!["capabilities".into(), "projections".into()] }),
                },
                tags: ["introspection", "projections"],
                cache: CachePolicy::Process { max_entries: 8, ttl_seconds: None },
                handler: capabilities_projections,
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
        assert_eq!(m.id.as_str(), "capabilities");
        let expected: &[(&str, &str, &str)] = &[
            (
                "capabilities.list",
                "majordomus_capabilities",
                "/api/v1/capabilities",
            ),
            (
                "capabilities.describe",
                "majordomus_capability",
                "/api/v1/capability",
            ),
            (
                "capabilities.projections",
                "majordomus_projections",
                "/api/v1/capabilities/projections",
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

    /// The handler's two filters and its one refusal, over the registry this crate ships.
    /// `Context::new` needs no repository: the filters read the registry and the clap tree,
    /// and neither touches the index.
    #[test]
    fn projections_filters_by_module_and_refuses_a_module_that_does_not_exist() {
        use crate::capability::{CapabilityRegistry, Context};
        use crate::git::GitState;
        use crate::index::{Index, RepositoryInfo, State};
        use std::sync::Arc;

        // an index with no objects: the filters read the registry and the clap tree, and
        // neither of them touches the layer
        let index = Index {
            repository: RepositoryInfo {
                root: "/tmp/projections".into(),
                layer_schema: "ai-repository/v1".into(),
                sections: Default::default(),
                git: GitState::Unavailable {
                    reason: "unit test".into(),
                },
                discovery: "filesystem".into(),
                source_classes: vec![],
                kind_sources: vec![],
                scope_origin: crate::scope::Origin::Distribution,
                scope_path: String::new(),
            },
            objects: vec![],
            diagnostics: vec![],
            state: State::Ok,
            fingerprint: String::new(),
            scoped: Default::default(),
            distribution: None,
            provider_templates: Vec::new(),
        };
        let registry = Arc::new(
            CapabilityRegistry::builder()
                .with_modules(super::super::modules())
                .build()
                .expect("the builtin registry builds"),
        );
        let ctx = Context::new(Arc::new(index), Arc::clone(&registry));

        // every row, unfiltered
        let all = capabilities_projections(&ctx, ProjectionsInput::default()).expect("all rows");
        assert_eq!(all.rows.len(), registry.len());
        assert!(
            all.unbacked.iter().any(|c| c == "majordomus serve"),
            "the process commands are the debt this reports"
        );

        // one module
        let one = capabilities_projections(
            &ctx,
            ProjectionsInput {
                module: Some("capabilities".into()),
                unmet: false,
            },
        )
        .expect("one module");
        assert!(!one.rows.is_empty());
        assert!(one.rows.iter().all(|r| r.module == "capabilities"));
        assert!(one.rows.len() < all.rows.len());

        // the closure this crate ships: nothing unmet
        let unmet = capabilities_projections(
            &ctx,
            ProjectionsInput {
                module: None,
                unmet: true,
            },
        )
        .expect("unmet");
        assert!(unmet.rows.is_empty(), "the shipped declaration is closed");

        // a module nobody composes is a refusal naming it, not an empty answer that reads
        // as "this module has no capabilities"
        let err = capabilities_projections(
            &ctx,
            ProjectionsInput {
                module: Some("nonesuch".into()),
                unmet: false,
            },
        )
        .expect_err("an unknown module is refused");
        assert!(
            matches!(err, CapabilityError::InvalidInput(ref m) if m.contains("nonesuch")),
            "{err:?}"
        );
    }
}
