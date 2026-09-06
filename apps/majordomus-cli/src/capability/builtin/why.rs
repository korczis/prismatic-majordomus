//! The `why` module: the operational moments this tool answers, the audiences that
//! recognise them, the areas they fall under, and what a reader's own symptoms imply.
//!
//! Every capability here reads [`crate::why::Catalogue`], which was built once from the
//! index. Nothing in this file parses a file, holds a list of moments, or knows the name
//! of one: a moment added to `.ai/repo/why/moments/` appears in every answer below
//! because the index discovered it, and this module never learns its name.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::model::Severity;
use crate::why::{Area, Audience, Catalogue, Diagnosis, Facets, Finding, Moment, Query, STABLE};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the whole catalogue is read as an MCP resource.
pub const WHY_URI: &str = "majordomus://why";
/// The URI under which the audiences are read.
pub const AUDIENCES_URI: &str = "majordomus://why/audiences";
/// The URI under which the areas are read.
pub const AREAS_URI: &str = "majordomus://why/areas";

/// The dataset's own format version, so a consumer can refuse a shape it does not read.
pub const SCHEMA: &str = "majordomus/why-catalogue/v1";

// ---------------------------------------------------------------- views

/// One moment as a listing shows it: everything a card needs and nothing a page needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MomentSummary {
    /// The identity and the slug.
    pub id: String,
    /// The moment as a heading.
    pub title: String,
    /// The name a narrow column shows: the short title, or the title.
    pub label: String,
    /// The first-person line an index shows.
    pub hook: String,
    /// One line: what is actually wrong.
    pub summary: String,
    /// `stable`, `draft` or `deprecated`.
    pub status: String,
    /// `low`, `medium` or `high`.
    pub severity: String,
    /// `rare`, `occasional`, `common` or `constant`.
    pub frequency: String,
    /// Presentation order.
    pub weight: u32,
    /// Whether the homepage shows it.
    pub featured: bool,
    /// The audiences that recognise it.
    pub audiences: Vec<String>,
    /// The operational areas it falls under.
    pub areas: Vec<String>,
    /// Free tags.
    pub tags: Vec<String>,
    /// Derived: `/why/<id>/`.
    pub route: String,
    /// How many observable symptoms it declares.
    pub signals: usize,
    /// How many concrete situations it carries.
    pub examples: usize,
}

impl MomentSummary {
    fn of(m: &Moment) -> Self {
        MomentSummary {
            id: m.id.clone(),
            title: m.title.clone(),
            label: m.label().to_string(),
            hook: m.hook.clone(),
            summary: m.summary.clone(),
            status: m.status.clone(),
            severity: m.severity.clone(),
            frequency: m.frequency.clone(),
            weight: m.weight,
            featured: m.featured,
            audiences: m.audiences.clone(),
            areas: m.areas.clone(),
            tags: m.tags.clone(),
            route: m.route.clone(),
            signals: m.signals.len(),
            examples: m.examples.len(),
        }
    }
}

/// How much the catalogue holds. Every count anywhere — a page, a heading, a report — is
/// one of these, so no number is ever written down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Counts {
    /// Public moments.
    pub moments: usize,
    /// Moments of every status, drafts included.
    pub moments_all: usize,
    /// Public audiences.
    pub audiences: usize,
    /// Public areas.
    pub areas: usize,
    /// Signals across the public moments: the size of the questionnaire.
    pub signals: usize,
    /// Concrete situations across the public moments.
    pub examples: usize,
}

/// The whole catalogue as a client reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CatalogueView {
    /// [`SCHEMA`].
    pub schema: String,
    /// The hash of the catalogue's own sources: stable for a tree, and independent of the
    /// rest of the index, so two runs over one tree agree.
    pub fingerprint: String,
    /// The counts.
    pub counts: Counts,
    /// The matching moments, in presentation order.
    pub moments: Vec<MomentSummary>,
    /// Every audience, with its derived membership.
    pub audiences: Vec<AudienceView>,
    /// Every area, with its derived membership.
    pub areas: Vec<AreaView>,
    /// The filters, derived from the records.
    pub facets: Facets,
}

/// One audience, with the membership nobody authored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AudienceView {
    /// The record as its file declares it, plus its route and source.
    #[serde(flatten)]
    pub audience: Audience,
    /// Derived: the public moments that name it, in presentation order.
    pub moments: Vec<String>,
    /// Derived: how many.
    pub count: usize,
}

/// One area, with the membership nobody authored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AreaView {
    /// The record as its file declares it, plus its route and source.
    #[serde(flatten)]
    pub area: Area,
    /// Derived: the public moments that fall under it, in presentation order.
    pub moments: Vec<String>,
    /// Derived: how many.
    pub count: usize,
}

/// One moment in full, with everything derived that a page shows and no file states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MomentDetail {
    /// The record as its file declares it, plus its route, source and body.
    #[serde(flatten)]
    pub moment: Moment,
    /// Derived: the responsibilities the claims it names belong to.
    pub responsibilities: Vec<String>,
    /// Derived: the moments that name this one in their `related`.
    pub backlinks: Vec<String>,
    /// Derived: moments sharing an area or an audience with this one and not already
    /// named by it, nearest first — most shared metadata, then presentation order.
    pub similar: Vec<String>,
    /// Derived: the explicitly related moments, summarised so a page needs one call.
    pub related_detail: Vec<MomentSummary>,
}

/// Every audience.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AudienceList {
    /// How many.
    pub count: usize,
    /// Each, with its derived membership, in presentation order.
    pub audiences: Vec<AudienceView>,
}

/// Every area.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AreaList {
    /// How many.
    pub count: usize,
    /// Each, with its derived membership, in presentation order.
    pub areas: Vec<AreaView>,
}

/// What the catalogue's own validation found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationReport {
    /// True when nothing is an error. Warnings do not make it false.
    pub valid: bool,
    /// How many findings are errors: a reference that resolves to nothing, a duplicate
    /// identity, a file name that disagrees with its id.
    pub errors: usize,
    /// How many findings are warnings: a public record that does not meet its floor.
    pub warnings: usize,
    /// The counts the catalogue reached.
    pub counts: Counts,
    /// The findings, errors first, then by file.
    pub findings: Vec<Finding>,
}

// ---------------------------------------------------------------- inputs

impl BenchmarkCases for Query {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases = vec![NamedCase::new("all", Query::default())];
        if let Some(a) = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == crate::why::AUDIENCE)
        {
            cases.push(NamedCase::new(
                "by-audience",
                Query {
                    audience: Some(a.identity.clone()),
                    ..Query::default()
                },
            ));
        }
        cases.push(NamedCase::new(
            "search",
            Query {
                q: Some("context".into()),
                ..Query::default()
            },
        ));
        cases
    }
}

/// Which moment to read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MomentInput {
    /// The moment's id, as `why.list` gives it. This is also its slug and its route.
    pub id: String,
}

impl BenchmarkCases for MomentInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        ctx.index
            .objects
            .iter()
            .find(|o| o.kind == crate::why::MOMENT)
            .map(|o| {
                vec![NamedCase::new(
                    "first-moment",
                    MomentInput {
                        id: o.identity.clone(),
                    },
                )]
            })
            .unwrap_or_default()
    }
}

/// What a reader recognised, as a comma-separated list of names.
///
/// A name is a signal id or a moment id: a person ticking symptoms and a script naming
/// moments reach the same answer, because a signal resolves to the moment that owns it.
/// The parameter is one string rather than a list because a diagnosis changes nothing and
/// is therefore a `GET`, and this repository binds a `GET` input to the query string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnoseInput {
    /// Signal ids or moment ids, separated by commas. Empty selects nothing and is
    /// answered with an empty diagnosis rather than an error.
    #[serde(default)]
    pub signals: String,
}

impl DiagnoseInput {
    /// The names, trimmed, in the order given, without empties.
    pub fn names(&self) -> Vec<String> {
        self.signals
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }
}

impl BenchmarkCases for DiagnoseInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let ids: Vec<String> = ctx
            .index
            .objects
            .iter()
            .filter(|o| o.kind == crate::why::MOMENT)
            .take(3)
            .map(|o| o.identity.clone())
            .collect();
        if ids.is_empty() {
            return Vec::new();
        }
        vec![NamedCase::new(
            "three-moments",
            DiagnoseInput {
                signals: ids.join(","),
            },
        )]
    }
}

// ---------------------------------------------------------------- handlers

fn counts(c: &Catalogue) -> Counts {
    let public: Vec<&Moment> = c.all().iter().filter(|m| m.status == STABLE).collect();
    Counts {
        moments: public.len(),
        moments_all: c.all().len(),
        audiences: c.audiences().iter().filter(|a| a.status == STABLE).count(),
        areas: c.areas().iter().filter(|a| a.status == STABLE).count(),
        signals: public.iter().map(|m| m.signals.len()).sum(),
        examples: public.iter().map(|m| m.examples.len()).sum(),
    }
}

fn audience_views(c: &Catalogue) -> Vec<AudienceView> {
    c.audiences()
        .iter()
        .map(|a| {
            let moments: Vec<String> = c
                .naming("audience", &a.id)
                .into_iter()
                .filter(|m| m.status == STABLE)
                .map(|m| m.id.clone())
                .collect();
            AudienceView {
                audience: a.clone(),
                count: moments.len(),
                moments,
            }
        })
        .collect()
}

fn area_views(c: &Catalogue) -> Vec<AreaView> {
    c.areas()
        .iter()
        .map(|a| {
            let moments: Vec<String> = c
                .naming("area", &a.id)
                .into_iter()
                .filter(|m| m.status == STABLE)
                .map(|m| m.id.clone())
                .collect();
            AreaView {
                area: a.clone(),
                count: moments.len(),
                moments,
            }
        })
        .collect()
}

fn why_list(ctx: &Context, input: Query) -> Result<CatalogueView, CapabilityError> {
    let c = &ctx.why;
    for (field, value, known) in [
        (
            "audience",
            &input.audience,
            c.audiences()
                .iter()
                .map(|a| a.id.clone())
                .collect::<Vec<_>>(),
        ),
        (
            "area",
            &input.area,
            c.areas().iter().map(|a| a.id.clone()).collect::<Vec<_>>(),
        ),
    ] {
        if let Some(v) = value {
            if !known.contains(v) {
                return Err(CapabilityError::InvalidInput(format!(
                    "{field} '{v}' is not one this catalogue has: {}",
                    known.join(", ")
                )));
            }
        }
    }
    Ok(CatalogueView {
        schema: SCHEMA.into(),
        fingerprint: c.fingerprint().into(),
        counts: counts(c),
        moments: c
            .select(&input)
            .into_iter()
            .map(MomentSummary::of)
            .collect(),
        audiences: audience_views(c),
        areas: area_views(c),
        facets: c.facets(),
    })
}

fn why_moment(ctx: &Context, input: MomentInput) -> Result<MomentDetail, CapabilityError> {
    let c = &ctx.why;
    let m = c.moment(&input.id).ok_or_else(|| {
        CapabilityError::NotFound(format!(
            "no moment '{}'; `why.list` names every one this repository holds",
            input.id
        ))
    })?;
    // "similar" is a deterministic graph traversal, not a recommendation: a moment scores
    // the number of areas and audiences it shares with this one, and ties break on the
    // presentation order the catalogue already has.
    let mut scored: Vec<(usize, &Moment)> = c
        .all()
        .iter()
        .filter(|o| o.id != m.id && o.status == STABLE && !m.related.contains(&o.id))
        .map(|o| {
            let shared = o.areas.iter().filter(|a| m.areas.contains(a)).count()
                + o.audiences
                    .iter()
                    .filter(|a| m.audiences.contains(a))
                    .count()
                + o.tags.iter().filter(|t| m.tags.contains(t)).count();
            (shared, o)
        })
        .filter(|(n, _)| *n > 0)
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(a.1.weight.cmp(&b.1.weight))
            .then(a.1.id.cmp(&b.1.id))
    });
    Ok(MomentDetail {
        moment: m.clone(),
        responsibilities: c.responsibilities(&m.id).to_vec(),
        backlinks: c.backlinks(&m.id).to_vec(),
        similar: scored
            .into_iter()
            .take(6)
            .map(|(_, o)| o.id.clone())
            .collect(),
        related_detail: m
            .related
            .iter()
            .filter_map(|r| c.moment(r))
            .map(MomentSummary::of)
            .collect(),
    })
}

fn why_audiences(ctx: &Context, _: Empty) -> Result<AudienceList, CapabilityError> {
    let audiences = audience_views(&ctx.why);
    Ok(AudienceList {
        count: audiences.len(),
        audiences,
    })
}

fn why_areas(ctx: &Context, _: Empty) -> Result<AreaList, CapabilityError> {
    let areas = area_views(&ctx.why);
    Ok(AreaList {
        count: areas.len(),
        areas,
    })
}

fn why_diagnose(ctx: &Context, input: DiagnoseInput) -> Result<Diagnosis, CapabilityError> {
    Ok(ctx.why.diagnose(&input.names()))
}

fn why_validate(ctx: &Context, _: Empty) -> Result<ValidationReport, CapabilityError> {
    let c = &ctx.why;
    let errors = c.errors();
    Ok(ValidationReport {
        valid: errors == 0,
        errors,
        warnings: c
            .findings()
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .count(),
        counts: counts(c),
        findings: c.findings().to_vec(),
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "why",
        title: "Why",
        description: "The operational failure modes this tool is a response to: the moments a reader recognises, the audiences that recognise them, the areas they fall under, and what a reader's own symptoms imply. Every entry is a file under the layer's why section; nothing here holds a list, and a moment added there is answered by all of these without a registration anywhere.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "why.list",
                title: "The catalogue",
                description: "Every operational moment this repository holds, narrowed by any of the facets the catalogue itself reports, with the audiences, the areas, the derived filters and the counts. The default is the public catalogue; pass status=any for the drafts too.",
                input: Query,
                output: CatalogueView,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_why".into()),
                        resource: Some(McpResource { uri: WHY_URI.into(), name: "why".into() }),
                    }),
                    http: get("/api/v1/why"),
                    cli: Some(crate::capability::CliExposure { path: vec!["why".into(), "list".into()] }),
                },
                tags: ["why", "catalogue"],
                cache: CachePolicy::Process { max_entries: 32, ttl_seconds: None },
                handler: why_list,
            },
            capability! {
                id: "why.moment",
                title: "One moment",
                description: "One operational moment in full: what it looks like, why it happens, what it costs, what this tool does about it, and every relation derived from its metadata — the responsibilities its claims belong to, the moments that name it, and the moments nearest it by shared area, audience and tag.",
                input: MomentInput,
                output: MomentDetail,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_why_moment"),
                    http: get("/api/v1/why/moment"),
                    cli: Some(crate::capability::CliExposure { path: vec!["why".into(), "show".into()] }),
                },
                tags: ["why"],
                cache: CachePolicy::Process { max_entries: 64, ttl_seconds: None },
                handler: why_moment,
            },
            capability! {
                id: "why.audiences",
                title: "Audiences",
                description: "Every audience the catalogue declares, each with the public moments that name it. Membership is derived from the moments and is never listed in an audience's own file.",
                input: Empty,
                output: AudienceList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_why_audiences".into()),
                        resource: Some(McpResource { uri: AUDIENCES_URI.into(), name: "why-audiences".into() }),
                    }),
                    http: get("/api/v1/why/audiences"),
                    cli: Some(crate::capability::CliExposure { path: vec!["why".into(), "audiences".into()] }),
                },
                tags: ["why", "catalogue"],
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: None },
                handler: why_audiences,
            },
            capability! {
                id: "why.areas",
                title: "Operational areas",
                description: "Every operational area the catalogue declares, each with the public moments that fall under it. Membership is derived from the moments and is never listed in an area's own file.",
                input: Empty,
                output: AreaList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_why_areas".into()),
                        resource: Some(McpResource { uri: AREAS_URI.into(), name: "why-areas".into() }),
                    }),
                    http: get("/api/v1/why/areas"),
                    cli: Some(crate::capability::CliExposure { path: vec!["why".into(), "areas".into()] }),
                },
                tags: ["why", "catalogue"],
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: None },
                handler: why_areas,
            },
            capability! {
                id: "why.diagnose",
                title: "Diagnose a selection",
                description: "What a reader's own symptoms imply: the moments the selection resolves to, the operational areas and audiences they weigh towards, and the capabilities, commands, claims, rules and use cases that answer them — each carrying the moments that produced it. Counting, not inference: there is no weighting and no percentage.",
                input: DiagnoseInput,
                output: Diagnosis,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_why_diagnose"),
                    http: get("/api/v1/why/diagnose"),
                    cli: Some(crate::capability::CliExposure { path: vec!["why".into(), "diagnose".into()] }),
                },
                tags: ["why", "diagnostics"],
                cache: CachePolicy::Process { max_entries: 32, ttl_seconds: None },
                handler: why_diagnose,
            },
            capability! {
                id: "why.validate",
                title: "Validate the catalogue",
                description: "Every finding over the catalogue: a reference that resolves to nothing, with the nearest candidate; a duplicate identity; a file name that disagrees with its id; and a public record that does not meet the floor its status promises. Errors make the catalogue invalid; warnings do not.",
                input: Empty,
                output: ValidationReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_why_validate"),
                    http: get("/api/v1/why/validate"),
                    cli: Some(crate::capability::CliExposure { path: vec!["why".into(), "validate".into()] }),
                },
                tags: ["why", "introspection"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: None },
                handler: why_validate,
            },
        ],
    }
}
