//! The `product` module: what this repository's product does for a person, as the features
//! under the layer declare it, with every surface, count and moment derived; the matrix of
//! features against interfaces; the providers; and the model's own validation.
//!
//! Every capability here reads [`crate::product::ProductModel`], which was built once from the
//! index, the registry, the Why catalogue and the web topology. Nothing in this file parses
//! a file, holds a list of features, or knows the name of one: a feature added to
//! `.ai/repo/features/` appears in every answer below because the index discovered it, and
//! this module never learns its name. The website's homepage is one more reader of the
//! same value (`majordomus generate site`), which is what makes the page a projection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::model::Severity;
use crate::product::{
    ProductCoverage, ProductFinding, ProductModel, ProductProvider, ResolvedRefs, Surfaces, STABLE,
    SURFACES,
};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the whole product model is read as an MCP resource.
pub const PRODUCT_URI: &str = "majordomus://product";
/// The URI under which the matrix is read.
pub const MATRIX_URI: &str = "majordomus://product/matrix";
/// The URI under which the providers are read.
pub const PROVIDERS_URI: &str = "majordomus://product/providers";

/// The dataset's own format version, so a consumer can refuse a shape it does not read.
pub const SCHEMA: &str = "majordomus/product/v1";

// ---------------------------------------------------------------- views

/// One feature as a listing shows it: everything a card needs and nothing a page needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureSummary {
    /// The identity and the slug.
    pub id: String,
    /// The feature as a heading.
    pub title: String,
    /// The name a narrow column shows: the short title, or the title.
    pub label: String,
    /// The promise a visitor reads first.
    pub headline: String,
    /// One line: what it does.
    pub summary: String,
    /// `stable`, `draft` or `deprecated`.
    pub status: String,
    /// Presentation order.
    pub weight: u32,
    /// Whether the homepage shows it.
    pub featured: bool,
    /// The operational areas it serves.
    pub areas: Vec<String>,
    /// The audiences it is written for.
    pub audiences: Vec<String>,
    /// Free tags.
    pub tags: Vec<String>,
    /// Derived: `/features/<id>/`.
    pub route: String,
    /// Derived: the interfaces it is exposed through.
    pub surfaces: Surfaces,
    /// Derived: how much stands behind it.
    pub counts: crate::product::FeatureCounts,
    /// Derived: what is guaranteed.
    pub evidence: crate::product::FeatureEvidence,
}

impl FeatureSummary {
    fn of(r: &ResolvedRefs) -> Self {
        let f = &r.feature;
        FeatureSummary {
            id: f.id.clone(),
            title: f.title.clone(),
            label: f.label().to_string(),
            headline: f.headline.clone(),
            summary: f.summary.clone(),
            status: f.status.clone(),
            weight: f.weight,
            featured: f.featured,
            areas: f.areas.clone(),
            audiences: f.audiences.clone(),
            tags: f.tags.clone(),
            route: f.route.clone(),
            surfaces: r.surfaces,
            counts: r.counts.clone(),
            evidence: r.evidence.clone(),
        }
    }
}

/// One surface of the vocabulary, as a client reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SurfaceInfo {
    /// `cli`, `api`, `mcp`, `cockpit` or `docs`.
    pub id: String,
    /// The name a person reads.
    pub title: String,
    /// The route on the website where the surface is documented.
    pub route: String,
    /// How many stable features it exposes.
    pub features: usize,
}

/// How much the model holds. Every count anywhere is one of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProductCounts {
    /// Stable features.
    pub features: usize,
    /// Features of every status, drafts included.
    pub features_all: usize,
    /// Stable features the homepage shows.
    pub featured: usize,
    /// Providers the tool has an adapter for.
    pub providers: usize,
    /// Builtin modules of the executable, and how many a stable feature names.
    pub modules: usize,
    /// Modules a stable feature names.
    pub modules_covered: usize,
    /// Public commands of the shell tool.
    pub commands: usize,
    /// Commands a stable feature names.
    pub commands_covered: usize,
    /// Kinds of the layer a feature can present.
    pub kinds: usize,
    /// Kinds a stable feature names.
    pub kinds_covered: usize,
}

/// The features as a client reads them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureList {
    /// [`SCHEMA`].
    pub schema: String,
    /// The hash of the model's sources and derived facts.
    pub fingerprint: String,
    /// The counts.
    pub counts: ProductCounts,
    /// The surface vocabulary, with how many stable features each exposes.
    pub surfaces: Vec<SurfaceInfo>,
    /// The matching features, in presentation order.
    pub features: Vec<FeatureSummary>,
}

/// One row of the matrix: a feature and the surfaces it is exposed through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatrixRow {
    /// The feature id.
    pub id: String,
    /// The name a narrow column shows.
    pub label: String,
    /// `/features/<id>/`.
    pub route: String,
    /// `stable` or `draft`.
    pub status: String,
    /// The surfaces, by id, in the vocabulary's order.
    pub surfaces: Surfaces,
    /// The surface ids that are true.
    pub exposed: Vec<String>,
}

/// Every feature against every interface, and every module, command and kind against the
/// features that name it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Matrix {
    /// [`SCHEMA`].
    pub schema: String,
    /// The surface vocabulary.
    pub surfaces: Vec<SurfaceInfo>,
    /// One row per feature of any status but deprecated, in presentation order.
    pub rows: Vec<MatrixRow>,
    /// Every builtin module of the executable with the stable features that name it.
    pub modules: Vec<ProductCoverage>,
    /// Every public command of the shell tool with the stable features that name it.
    pub commands: Vec<ProductCoverage>,
    /// Every kind of the layer a feature can present, with the stable features that name it.
    pub kinds: Vec<ProductCoverage>,
}

/// Every provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderList {
    /// How many.
    pub count: usize,
    /// Each, in id order.
    pub providers: Vec<ProductProvider>,
}

/// What the model's own validation found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProductValidationReport {
    /// True when nothing is an error. Warnings do not make it false.
    pub valid: bool,
    /// How many findings are errors.
    pub errors: usize,
    /// How many findings are warnings.
    pub warnings: usize,
    /// The counts the model reached.
    pub counts: ProductCounts,
    /// The findings, errors first, then by file.
    pub findings: Vec<ProductFinding>,
}

// ---------------------------------------------------------------- inputs

/// What a listing is narrowed by. Every field is optional and they compose as a
/// conjunction; the values a caller may pass are the facets the model derives, never a list
/// written anywhere.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductQuery {
    #[serde(default)]
    /// Only the features the homepage shows.
    pub featured: Option<bool>,
    #[serde(default)]
    /// Only features of this status. Absent means the stable ones; pass `any` for
    /// everything the model holds.
    pub status: Option<String>,
    #[serde(default)]
    /// Only features serving this operational area.
    pub area: Option<String>,
    #[serde(default)]
    /// Only features made of this capability module.
    pub module: Option<String>,
    #[serde(default)]
    /// Only features made of this shell command.
    pub command: Option<String>,
    #[serde(default)]
    /// Only features exposed through this surface: `cli`, `api`, `mcp`, `cockpit`, `docs`.
    pub surface: Option<String>,
    #[serde(default)]
    /// Case-insensitive text, matched against the identity, the titles, the headline, the
    /// summary, the tags and the body.
    pub q: Option<String>,
}

impl BenchmarkCases for ProductQuery {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases = vec![NamedCase::new("all", ProductQuery::default())];
        cases.push(NamedCase::new(
            "featured",
            ProductQuery {
                featured: Some(true),
                ..ProductQuery::default()
            },
        ));
        if let Some(a) = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == crate::why::AREA)
        {
            cases.push(NamedCase::new(
                "by-area",
                ProductQuery {
                    area: Some(a.identity.clone()),
                    ..ProductQuery::default()
                },
            ));
        }
        cases
    }
}

/// Which feature to read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FeatureInput {
    /// The feature's id, as `product.features` gives it. This is also its slug and its route.
    pub id: String,
}

impl BenchmarkCases for FeatureInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        ctx.index
            .objects
            .iter()
            .find(|o| o.kind == crate::product::FEATURE)
            .map(|o| {
                vec![NamedCase::new(
                    "first-feature",
                    FeatureInput {
                        id: o.identity.clone(),
                    },
                )]
            })
            // A repository that declares no feature yet still exposes this route, and a
            // capability with no case is a capability nobody measures. The refusal is the
            // measurable path there, so the case names a feature that is not there.
            .unwrap_or_else(|| {
                vec![NamedCase::new(
                    "absent",
                    FeatureInput {
                        id: "absent".into(),
                    },
                )]
            })
    }
}

// ---------------------------------------------------------------- handlers

fn counts(m: &ProductModel) -> ProductCounts {
    let covered = |c: &[ProductCoverage]| c.iter().filter(|x| !x.features.is_empty()).count();
    ProductCounts {
        features: m.public().len(),
        features_all: m.all().len(),
        featured: m.featured().len(),
        providers: m.providers().len(),
        modules: m.module_coverage().len(),
        modules_covered: covered(m.module_coverage()),
        commands: m.command_coverage().len(),
        commands_covered: covered(m.command_coverage()),
        kinds: m.kind_coverage().len(),
        kinds_covered: covered(m.kind_coverage()),
    }
}

fn surfaces(m: &ProductModel) -> Vec<SurfaceInfo> {
    SURFACES
        .iter()
        .map(|(id, title, route)| SurfaceInfo {
            id: (*id).to_string(),
            title: (*title).to_string(),
            route: (*route).to_string(),
            features: m.public().iter().filter(|r| r.surfaces.has(id)).count(),
        })
        .collect()
}

fn matches(r: &ResolvedRefs, needle: &str) -> bool {
    let f = &r.feature;
    let mut text = String::new();
    for part in [
        f.id.as_str(),
        f.title.as_str(),
        f.short_title.as_deref().unwrap_or(""),
        f.headline.as_str(),
        f.summary.as_str(),
        f.body.as_str(),
    ] {
        text.push_str(part);
        text.push('\n');
    }
    for t in &f.tags {
        text.push_str(t);
        text.push('\n');
    }
    text.to_lowercase().contains(needle)
}

fn product_features(ctx: &Context, input: ProductQuery) -> Result<FeatureList, CapabilityError> {
    let m = &ctx.product;
    if let Some(s) = input.surface.as_deref() {
        if !SURFACES.iter().any(|(id, _, _)| *id == s) {
            return Err(CapabilityError::InvalidInput(format!(
                "surface '{s}' is not one of {}",
                SURFACES
                    .iter()
                    .map(|(id, _, _)| *id)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    let status = input.status.as_deref().unwrap_or(STABLE);
    let text = input.q.as_ref().map(|s| s.to_lowercase());
    let features: Vec<FeatureSummary> = m
        .all()
        .iter()
        .filter(|r| status == "any" || r.feature.status == status)
        .filter(|r| input.featured.is_none_or(|v| r.feature.featured == v))
        .filter(|r| {
            input
                .area
                .as_ref()
                .is_none_or(|v| r.feature.areas.contains(v))
        })
        .filter(|r| {
            input
                .module
                .as_ref()
                .is_none_or(|v| r.feature.modules.contains(v))
        })
        .filter(|r| {
            input
                .command
                .as_ref()
                .is_none_or(|v| r.feature.commands.contains(v))
        })
        .filter(|r| input.surface.as_deref().is_none_or(|v| r.surfaces.has(v)))
        .filter(|r| text.as_deref().is_none_or(|t| matches(r, t)))
        .map(FeatureSummary::of)
        .collect();
    Ok(FeatureList {
        schema: SCHEMA.into(),
        fingerprint: m.fingerprint().into(),
        counts: counts(m),
        surfaces: surfaces(m),
        features,
    })
}

fn product_feature(ctx: &Context, input: FeatureInput) -> Result<ResolvedRefs, CapabilityError> {
    ctx.product.feature(&input.id).cloned().ok_or_else(|| {
        CapabilityError::NotFound(format!(
            "no feature '{}'; `product.features` names every one this repository holds",
            input.id
        ))
    })
}

fn product_matrix(ctx: &Context, _: Empty) -> Result<Matrix, CapabilityError> {
    let m = &ctx.product;
    Ok(Matrix {
        schema: SCHEMA.into(),
        surfaces: surfaces(m),
        rows: m
            .all()
            .iter()
            .filter(|r| r.feature.status != "deprecated")
            .map(|r| MatrixRow {
                id: r.feature.id.clone(),
                label: r.feature.label().to_string(),
                route: r.feature.route.clone(),
                status: r.feature.status.clone(),
                surfaces: r.surfaces,
                exposed: r.surfaces.present().iter().map(|s| s.to_string()).collect(),
            })
            .collect(),
        modules: m.module_coverage().to_vec(),
        commands: m.command_coverage().to_vec(),
        kinds: m.kind_coverage().to_vec(),
    })
}

fn product_providers(ctx: &Context, _: Empty) -> Result<ProviderList, CapabilityError> {
    let providers = ctx.product.providers().to_vec();
    Ok(ProviderList {
        count: providers.len(),
        providers,
    })
}

fn product_validate(ctx: &Context, _: Empty) -> Result<ProductValidationReport, CapabilityError> {
    let m = &ctx.product;
    let errors = m.errors();
    Ok(ProductValidationReport {
        valid: errors == 0,
        errors,
        warnings: m
            .findings()
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .count(),
        counts: counts(m),
        findings: m.findings().to_vec(),
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "product",
        title: "Product",
        description: "What this repository's product does for a person, as the features under the layer's features section declare it: each feature made of modules, commands, kinds, rules, documents, decisions, claims, use cases, Cockpit areas and web surfaces it names, with the interfaces it is exposed through, every count, the moments it answers and what is guaranteed derived from those references. The matrix of features against interfaces, the providers the tool has an adapter for, and the model's own validation. The homepage is a reader of this module and holds no inventory of its own.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "product.features",
                title: "The features",
                description: "Every product feature this repository declares, narrowed by any of the facets the model derives — featured, area, module, command, surface, text — with the interfaces each is exposed through, the counts behind it and what is guaranteed about it, none of which its file states. The default is the stable set; pass status=any for the drafts too.",
                input: ProductQuery,
                output: FeatureList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_features".into()),
                        resource: Some(McpResource { uri: PRODUCT_URI.into(), name: "product".into() }),
                    }),
                    http: get("/api/v1/product/features"),
                    cli: Some(crate::capability::CliExposure { path: vec!["product".into(), "list".into()] }),
                },
                tags: ["product", "features", "introspection"],
                cache: CachePolicy::Process { max_entries: 32, ttl_seconds: None },
                handler: product_features,
            },
            capability! {
                id: "product.feature",
                title: "One feature",
                description: "One product feature in full: the record as its file declares it, and everything derived from what it names — the capabilities of its modules with their tools, routes and command-line paths, the commands with their summaries, the objects of its kinds counted, the rules with their class and whether the tool enforces them, the documents, the decisions, the claims with their status, the use cases, the Cockpit areas and web surfaces with their routes, the moments it answers, and the interfaces all of that adds up to.",
                input: FeatureInput,
                output: ResolvedRefs,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_feature"),
                    http: get("/api/v1/product/feature"),
                    cli: Some(crate::capability::CliExposure { path: vec!["product".into(), "show".into()] }),
                },
                tags: ["product", "features"],
                cache: CachePolicy::Process { max_entries: 64, ttl_seconds: None },
                handler: product_feature,
            },
            capability! {
                id: "product.matrix",
                title: "Features against interfaces",
                description: "Every feature against the command line, the HTTP API, MCP, the Cockpit and the documentation, each mark derived from what the feature names; then every builtin module of the executable, every public command of the shell tool and every kind of the layer with the stable features that name it. A row with no feature is reported as a gap rather than hidden.",
                input: Empty,
                output: Matrix,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_product_matrix".into()),
                        resource: Some(McpResource { uri: MATRIX_URI.into(), name: "product-matrix".into() }),
                    }),
                    http: get("/api/v1/product/matrix"),
                    cli: Some(crate::capability::CliExposure { path: vec!["product".into(), "matrix".into()] }),
                },
                tags: ["product", "features", "coverage"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: None },
                handler: product_matrix,
            },
            capability! {
                id: "product.providers",
                title: "The providers",
                description: "Every provider the tool has an adapter for — one per template the distribution ships — with the bootstraps this repository's policy renders through it, the client configuration it carries for the shared MCP server, and the hooks the policy wires. The set is the templates; nothing here is a list of vendors.",
                input: Empty,
                output: ProviderList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_providers".into()),
                        resource: Some(McpResource { uri: PROVIDERS_URI.into(), name: "providers".into() }),
                    }),
                    http: get("/api/v1/product/providers"),
                    cli: Some(crate::capability::CliExposure { path: vec!["product".into(), "providers".into()] }),
                },
                tags: ["product", "providers"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: None },
                handler: product_providers,
            },
            capability! {
                id: "product.validate",
                title: "Validate the model",
                description: "Every finding over the product model: a reference that resolves to nothing, with the nearest candidate; a duplicate identity; a file name that disagrees with its id; a draft that is featured; a stable feature under its floors; and every module, command or kind that no stable feature names. Errors make the model invalid; warnings do not.",
                input: Empty,
                output: ProductValidationReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_product_validate"),
                    http: get("/api/v1/product/validate"),
                    cli: Some(crate::capability::CliExposure { path: vec!["product".into(), "validate".into()] }),
                },
                tags: ["product", "introspection"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: None },
                handler: product_validate,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "product");
        let expected: &[(&str, &str, &str)] = &[
            (
                "product.features",
                "majordomus_features",
                "/api/v1/product/features",
            ),
            (
                "product.feature",
                "majordomus_feature",
                "/api/v1/product/feature",
            ),
            (
                "product.matrix",
                "majordomus_product_matrix",
                "/api/v1/product/matrix",
            ),
            (
                "product.providers",
                "majordomus_providers",
                "/api/v1/product/providers",
            ),
            (
                "product.validate",
                "majordomus_product_validate",
                "/api/v1/product/validate",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(ids, want);
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
        let resource = m.capabilities[0]
            .capability
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref());
        assert_eq!(resource.map(|r| r.uri.as_str()), Some(PRODUCT_URI));
    }

    /// Every capability of the module is read-only: the registry's contract.
    #[test]
    fn every_capability_is_a_query() {
        for e in module().capabilities {
            assert!(
                e.capability.kind.is_read_only(),
                "{} writes",
                e.capability.id
            );
        }
    }
}
