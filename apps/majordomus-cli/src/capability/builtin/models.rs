//! The `models` module: the model catalogue and its routing, projected. Every
//! capability reads `share/models.yaml` through [`crate::models::ModelCatalogue`] —
//! declared data, never a live client (ADR 0049, standing on ADR 0032's no-network
//! boundary) — so the CLI, HTTP, OpenAPI, MCP and the Cockpit name models from one
//! file and nothing else. What local runtimes actually serve is observed by the
//! transport under `scripts/lib/inference/`, admitted by `models.observe`, read by
//! `models.observed` and routed over by `models.route --observed` (ADR 0174): an overlay
//! on the catalogue, never written into it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, CapabilityKind, CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::models::observed::{self, CapabilityEvidence, ObservationReport, RuntimeObservation};
use crate::models::{
    route_observed, ModelCatalogue, ModelEntry, ObservedPool, Requirements, RoutingDecision, Vendor,
};
use crate::{capability, module};

use super::{get, mcp, post};

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
    /// Also choose from the checkout's observation of local runtimes (`models observed`),
    /// after every declared model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<bool>,
    /// The least evidence an observed model's required capability must rest on:
    /// `inferred`, `advertised` or `probed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_evidence: Option<CapabilityEvidence>,
    /// The age beyond which the observation is stale, seconds; 900 when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age: Option<u64>,
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

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn observation_of(ctx: &Context) -> Result<Option<RuntimeObservation>, CapabilityError> {
    observed::load(std::path::Path::new(&ctx.index.repository.root))
        .map_err(CapabilityError::Internal)
}

fn models_route(ctx: &Context, input: RouteInput) -> Result<RoutingDecision, CapabilityError> {
    let catalogue = catalogue_of(ctx)?;
    let observation = if input.observed.unwrap_or(false) {
        observation_of(ctx)?
    } else {
        None
    };
    let pool = observation.as_ref().map(|o| ObservedPool {
        observation: o,
        age_seconds: observed::age_seconds(o, now_epoch()),
        ttl_seconds: input.max_age.unwrap_or(observed::DEFAULT_TTL_SECONDS),
    });
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
        min_evidence: input.min_evidence,
    };
    Ok(route_observed(&catalogue, pool, &needs))
}

// ---------------------------------------------------------------- models.observe

impl BenchmarkCases for ObservationReport {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // A report naming a vendor no catalogue declares: the refusal path, which writes
        // nothing, so a benchmark never replaces a checkout's real observation.
        vec![NamedCase::new(
            "refused",
            ObservationReport {
                version: observed::VERSION,
                observed_at: "2026-10-17T00:00:00Z".into(),
                probe: Default::default(),
                remote_admitted: false,
                runtimes: vec![observed::RuntimeReport {
                    vendor: "benchmark-absent".into(),
                    endpoint: "http://127.0.0.1:9".into(),
                    state: observed::RuntimeState::Unreachable,
                    version: None,
                    identified_by: None,
                    diagnostic: None,
                    duration_ms: None,
                    models: Vec::new(),
                }],
                refused: Vec::new(),
            },
        )]
    }
}

/// The answer of `models.observe`: the observation as stored, and where.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ObservationRecorded {
    /// The file it replaced, relative to the repository.
    pub path: String,
    /// The observation.
    #[serde(flatten)]
    pub observation: RuntimeObservation,
}

fn models_observe(
    ctx: &Context,
    report: ObservationReport,
) -> Result<ObservationRecorded, CapabilityError> {
    let catalogue = catalogue_of(ctx)?;
    let observation = observed::admit(report, &catalogue, crate::evidence::record::now_rfc3339())
        .map_err(|findings| CapabilityError::Refused(findings.join("; ")))?;
    observed::save(
        std::path::Path::new(&ctx.index.repository.root),
        &observation,
    )
    .map_err(|e| CapabilityError::Internal(format!("{}: {e}", observed::STATE_FILE)))?;
    Ok(ObservationRecorded {
        path: observed::STATE_FILE.into(),
        observation,
    })
}

// ---------------------------------------------------------------- models.observed

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
/// The input of `models.observed`.
pub struct ObservedInput {
    /// The age beyond which the observation is stale, seconds; 900 when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age: Option<u64>,
}

impl BenchmarkCases for ObservedInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", ObservedInput::default()),
            NamedCase::new("max-age", ObservedInput { max_age: Some(60) }),
        ]
    }
}

/// The answer of `models.observed`: the checkout's current observation of its local
/// runtimes, how old it is, and whether routing would still trust it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ObservedReport {
    /// Where the observation is kept, relative to the repository.
    pub path: String,
    /// The observation; absent when this checkout has never observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<RuntimeObservation>,
    /// Its age now, seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<u64>,
    /// The age beyond which it is stale, seconds.
    pub ttl_seconds: u64,
    /// Whether it is stale, or absent: either way routing chooses no observed model.
    pub stale: bool,
    /// How many models of identified runtimes it holds.
    pub models: usize,
    /// What to run to observe again.
    pub observe_with: String,
}

fn models_observed(ctx: &Context, input: ObservedInput) -> Result<ObservedReport, CapabilityError> {
    let observation = observation_of(ctx)?;
    let ttl_seconds = input.max_age.unwrap_or(observed::DEFAULT_TTL_SECONDS);
    let age_seconds = observation
        .as_ref()
        .and_then(|o| observed::age_seconds(o, now_epoch()));
    Ok(ObservedReport {
        path: observed::STATE_FILE.into(),
        stale: age_seconds.is_none_or(|age| age > ttl_seconds),
        models: observation.as_ref().map_or(0, |o| o.models().count()),
        observation,
        age_seconds,
        ttl_seconds,
        observe_with: "scripts/inference-observe [--probe]".into(),
    })
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
                cache: CachePolicy::Disabled,
                handler: models_route,
            },
            capability! {
                id: "models.observe",
                kind: CapabilityKind::Command,
                title: "Record an observation of local runtimes",
                description: "Admit what scripts/inference-observe found at the local runtimes the catalogue declares — which endpoints answered as which runtime, what each serves, each model's capability words with their evidence (inferred, advertised, probed) and the probes made — and replace the checkout's observation under .ai/local/state/inference/. Refused, with every finding, when a vendor is not a declared local runtime, an endpoint carries a credential or leaves the loopback interface unadmitted, or a field is malformed. Secret shapes are redacted before anything is written.",
                input: ObservationReport,
                output: ObservationRecorded,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_models_observe"),
                    http: post("/api/v1/models/observations"),
                    cli: Some(CliExposure { path: vec!["models".into(), "observe".into()] }),
                },
                tags: ["models", "inference", "records"],
                cache: CachePolicy::Disabled,
                handler: models_observe,
            }
            .writes_repository(),
            capability! {
                id: "models.observed",
                title: "The observed local runtimes",
                description: "The checkout's current observation of its local inference runtimes: each endpoint with its runtime instance identity, locality and whether it answered as the runtime its vendor names; each model with its instance identity, digest, quantization, load state, context window (advertised and configured, either possibly unknown) and every capability word with the standing its evidence gives it. Says how old the observation is and whether routing still trusts it; absent and stale are answers, never guessed around.",
                input: ObservedInput,
                output: ObservedReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_models_observed"),
                    http: get("/api/v1/models/observations"),
                    cli: Some(CliExposure { path: vec!["models".into(), "observed".into()] }),
                },
                tags: ["models", "inference"],
                cache: CachePolicy::Disabled,
                handler: models_observed,
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
            (
                "models.observe",
                "majordomus_models_observe",
                "/api/v1/models/observations",
            ),
            (
                "models.observed",
                "majordomus_models_observed",
                "/api/v1/models/observations",
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
    fn only_the_observation_writer_is_a_command() {
        for e in module().capabilities {
            let want = if e.capability.id.as_str() == "models.observe" {
                CapabilityKind::Command
            } else {
                CapabilityKind::Query
            };
            assert_eq!(
                e.capability.kind, want,
                "{}: the catalogue is declared data; only the observation is recorded",
                e.capability.id
            );
        }
    }
}
