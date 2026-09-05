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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `capabilities.list`: the matching capabilities and the registry counted.
pub struct CapabilityList {
    /// How many capabilities matched the filters.
    pub count: usize,
    /// The whole registry, counted by kind, stability and projection.
    pub summary: Summary,
    /// The matching capabilities, by id.
    pub capabilities: Vec<Capability>,
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
    let capabilities: Vec<Capability> = ctx
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
        .cloned()
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
                description: "Every capability of this executable and this repository, with its kind, stability, provenance and the projections it declares.",
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
        ],
    }
}
