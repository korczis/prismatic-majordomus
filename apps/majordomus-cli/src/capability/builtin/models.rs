//! The `models` module: the model catalogue and its routing, projected. Both
//! capabilities read `share/models.yaml` through [`crate::models::ModelCatalogue`] —
//! declared data, never a live client (ADR 0049, standing on ADR 0032's no-network
//! boundary) — so the CLI, HTTP, OpenAPI, MCP and the Cockpit name models from one
//! file and nothing else.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::models::{route, ModelCatalogue, ModelEntry, Requirements, RoutingDecision, Vendor};
use crate::{capability, module};

use super::{get, mcp};

/// The MCP resource `models.list` is projected as.
pub const MODELS_URI: &str = "majordomus://models";

// ---------------------------------------------------------------- models.list

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
/// The input of `models.list`: optional narrowing. Everything empty lists everything.
pub struct ModelsFilter {
    /// Only this vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// Only models declaring this capability word.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    /// One model, by canonical id or alias.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl BenchmarkCases for ModelsFilter {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", ModelsFilter::default()),
            // Every parameter carries a value here so the OpenAPI document has an
            // example for each; the filter names entries the shipped catalogue holds.
            NamedCase::new(
                "narrowed",
                ModelsFilter {
                    vendor: Some("anthropic".into()),
                    capability: Some("text".into()),
                    id: Some("claude-opus-5".into()),
                },
            ),
        ]
    }
}

/// One vendor with whether its credential is configured — presence of the named
/// environment variable and nothing more. No value is read.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct VendorView {
    /// The declaration.
    #[serde(flatten)]
    pub vendor: Vendor,
    /// Whether the named credential variable is set in this process's environment.
    /// `None` when the vendor names no variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_configured: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `models.list`.
pub struct ModelsReport {
    /// How many models matched.
    pub count: usize,
    /// The vendors, declaration order.
    pub vendors: Vec<VendorView>,
    /// The matching models, declaration order — which is routing's preference order.
    pub models: Vec<ModelEntry>,
    /// The catalogue's own findings: duplicate names, undeclared vendors. Empty is
    /// the ordinary case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

fn catalogue_of(ctx: &Context) -> Result<ModelCatalogue, CapabilityError> {
    let Some(share) = &ctx.index.share else {
        return Ok(ModelCatalogue::default());
    };
    ModelCatalogue::load(share).map_err(CapabilityError::Internal)
}

fn models_list(ctx: &Context, filter: ModelsFilter) -> Result<ModelsReport, CapabilityError> {
    let catalogue = catalogue_of(ctx)?;
    let models: Vec<ModelEntry> = catalogue
        .models
        .iter()
        .filter(|m| filter.vendor.as_ref().is_none_or(|v| &m.vendor == v))
        .filter(|m| {
            filter
                .capability
                .as_ref()
                .is_none_or(|c| m.capabilities.iter().any(|has| has == c))
        })
        .filter(|m| {
            filter
                .id
                .as_ref()
                .is_none_or(|id| &m.id == id || m.aliases.iter().any(|a| a == id))
        })
        .cloned()
        .collect();
    Ok(ModelsReport {
        count: models.len(),
        vendors: catalogue
            .vendors
            .iter()
            .map(|v| VendorView {
                credential_configured: v
                    .credential_env
                    .as_deref()
                    .map(|name| std::env::var_os(name).is_some()),
                vendor: v.clone(),
            })
            .collect(),
        models,
        diagnostics: catalogue.diagnostics(),
    })
}

// ---------------------------------------------------------------- models.route

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
/// The input of `models.route`: what the task needs, in transport-friendly scalars
/// (`require` is comma-separated so a GET query string can carry it).
pub struct RouteInput {
    /// Capability words the model must declare, comma-separated: `vision,tools`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require: Option<String>,
    /// The least context window, tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_context: Option<u64>,
    /// Only this vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// Only local inference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_only: Option<bool>,
    /// A model named outright, by canonical id or alias — still checked against the
    /// other requirements.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl BenchmarkCases for RouteInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "default",
            RouteInput {
                require: Some("text".into()),
                ..RouteInput::default()
            },
        )]
    }
}

fn models_route(ctx: &Context, input: RouteInput) -> Result<RoutingDecision, CapabilityError> {
    let catalogue = catalogue_of(ctx)?;
    let needs = Requirements {
        require: input
            .require
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect(),
        min_context: input.min_context,
        vendor: input.vendor,
        local_only: input.local_only.unwrap_or(false),
        model: input.model,
    };
    Ok(route(&catalogue, &needs))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "models",
        title: "Models",
        description: "The model catalogue the distribution declares — vendors, canonical model references, typed capabilities, context windows, lifecycle — and the explainable routing over it: what a stated need selects, what stands behind it, and why every excluded model fell out. Declared data; nothing here calls a model or reads a credential's value.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "models.list",
                title: "The model catalogue",
                description: "Every declared vendor and model, optionally narrowed by vendor, capability word, or one id or alias. Vendors carry whether their named credential variable is set — presence only, never a value. The order is the declaration's, which is also routing's preference order; the catalogue's own findings (a duplicate alias, an undeclared vendor) ride along.",
                input: ModelsFilter,
                output: ModelsReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_models".into()),
                        resource: Some(crate::capability::model::McpResource {
                            uri: MODELS_URI.into(),
                            name: "models".into(),
                        }),
                    }),
                    http: get("/api/v1/models"),
                    cli: Some(CliExposure { path: vec!["models".into(), "list".into()] }),
                },
                tags: ["models", "catalogue"],
                cache: crate::capability::model::CachePolicy::Process { max_entries: 4, ttl_seconds: Some(5) },
                handler: models_list,
            },
            capability! {
                id: "models.route",
                title: "Route a need to a model",
                description: "Decide which declared model a stated need selects: the first in declaration order satisfying every requirement, the qualifying rest as the fallback chain, and every excluded model with the first check it failed. Pure over the catalogue and the input — the same question gets the same answer, and 'why' is in the answer itself.",
                input: RouteInput,
                output: RoutingDecision,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_models_route"),
                    http: get("/api/v1/models/route"),
                    cli: Some(CliExposure { path: vec!["models".into(), "route".into()] }),
                },
                tags: ["models", "routing"],
                handler: models_route,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; this is the assertion a
    /// refactor that dropped a projection would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "models");
        let expected: &[(&str, &str, &str)] = &[
            ("models.list", "majordomus_models", "/api/v1/models"),
            (
                "models.route",
                "majordomus_models_route",
                "/api/v1/models/route",
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

    #[test]
    fn every_capability_is_a_query() {
        for e in module().capabilities {
            assert_eq!(
                e.capability.kind,
                crate::capability::model::CapabilityKind::Query,
                "{}: the catalogue is declared data; nothing here mutates",
                e.capability.id
            );
        }
    }
}
