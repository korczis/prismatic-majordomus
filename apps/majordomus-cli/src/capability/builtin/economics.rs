//! The `economics` module: token economics, measured, projected. Every capability here is a
//! read of [`crate::economics`] — the one calculator — so the command line, HTTP, the
//! OpenAPI document, MCP and the Cockpit show the same metric, value, class and sample
//! count, because none of them computes one.
//!
//! A repository that declares no benchmark methodology is answered with `present: false` and
//! the statement that no claim is available, never with an error and never with a number.
//!
//! ```
//! use majordomus_cli::capability::builtin::economics::module;
//! let m = module();
//! let c = m.capabilities.iter().find(|c| c.capability.id.as_str() == "economics.summary").unwrap();
//! assert_eq!(c.capability.exposure.http.as_ref().unwrap().path, "/api/v1/economics");
//! assert_eq!(
//!     c.capability.exposure.cli.as_ref().unwrap().path,
//!     vec!["economics".to_string(), "summary".to_string()]
//! );
//! ```

use std::path::Path;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::economics::claims::{EconomicsCheckInput, EconomicsCheckReport};
use crate::economics::model::{
    EconomicsExplainInput, EconomicsExplanation, EconomicsQuery, EconomicsRunList,
    EconomicsRunsQuery, EconomicsSummary,
};
use crate::{capability, module};

use super::{get, mcp};

/// The URI under which the summary is read as an MCP resource.
pub const ECONOMICS_URI: &str = "majordomus://economics";

impl BenchmarkCases for EconomicsQuery {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", EconomicsQuery::default()),
            // every parameter carries a value so that the OpenAPI document has an example
            // for each; the values name the shipped pilot suite and one of its tasks
            NamedCase::new(
                "narrowed",
                EconomicsQuery {
                    suite: Some("pilot".into()),
                    category: Some("bug-fix".into()),
                    task: Some("vat-rounding".into()),
                    model: Some("claude-sonnet-5".into()),
                },
            ),
        ]
    }
}

impl BenchmarkCases for EconomicsExplainInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "primary",
            EconomicsExplainInput {
                metric: crate::economics::EFFECTIVE_TOKEN_REDUCTION.into(),
            },
        )]
    }
}

impl BenchmarkCases for EconomicsRunsQuery {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", EconomicsRunsQuery::default()),
            NamedCase::new(
                "narrowed",
                EconomicsRunsQuery {
                    suite: Some("pilot".into()),
                    task: Some("vat-rounding".into()),
                    variant: Some("baseline".into()),
                },
            ),
        ]
    }
}

impl BenchmarkCases for EconomicsCheckInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("default", EconomicsCheckInput::default())]
    }
}

fn root(ctx: &Context) -> &Path {
    Path::new(&ctx.index.repository.root)
}

fn summary(ctx: &Context, q: EconomicsQuery) -> Result<EconomicsSummary, CapabilityError> {
    Ok(crate::economics::summarize(root(ctx), &q))
}

fn explain(ctx: &Context, input: EconomicsExplainInput) -> Result<EconomicsExplanation, CapabilityError> {
    crate::economics::explain(root(ctx), &input.metric).map_err(CapabilityError::NotFound)
}

fn runs(ctx: &Context, q: EconomicsRunsQuery) -> Result<EconomicsRunList, CapabilityError> {
    Ok(crate::economics::runs(root(ctx), &q))
}

fn check(ctx: &Context, _: EconomicsCheckInput) -> Result<EconomicsCheckReport, CapabilityError> {
    Ok(crate::economics::claims::check(root(ctx)))
}

/// The `economics` module: four read-only capabilities, each a thin call into
/// [`crate::economics`], so no surface computes a metric of its own. The summary, the
/// explanation and the run list are cached per process for five seconds; the claim check is
/// never cached, because a gate that answered from memory could pass prose edited a moment ago.
///
/// ```
/// use majordomus_cli::capability::builtin::economics::module;
/// use majordomus_cli::capability::{CachePolicy, Effect};
/// let m = module();
/// let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
/// assert_eq!(
///     ids,
///     ["economics.summary", "economics.explain", "economics.runs", "economics.check"]
/// );
/// // every capability reads; none of them changes the repository
/// assert!(m.capabilities.iter().all(|e| e.capability.kind.is_read_only()));
/// assert!(m.capabilities.iter().all(|e| e.capability.execution.effect == Effect::Read));
/// assert_eq!(m.capabilities[3].capability.cache, CachePolicy::Disabled);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "economics",
        title: "Token economics",
        description: "What a coding session consumes with Majordomus installed and without it, from recorded runs: matched control and treatment sessions judged by the same hidden acceptance tests, the provider's usage as it reported it, and every number labelled with its measurement class (observed, counted, derived, estimated, counterfactual). One calculator; every surface is a projection of it, and the verdict states only what the methodology's publication rule allows.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "economics.summary",
                title: "Token economics summary",
                description: "Every metric of the economics benchmark with its value, measurement class, sample size, distribution and bootstrap interval; every pair of runs with its status (valid, control failed, treatment failed, incomparable, ...); results by segment; the state of each suite's evidence (current, stale, incompatible); and the one statement the publication rule allows. Context reduction and total-token reduction are separate metrics and are never merged.",
                input: EconomicsQuery,
                output: EconomicsSummary,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_economics".into()),
                        resource: Some(McpResource { uri: ECONOMICS_URI.into(), name: "economics".into() }),
                    }),
                    http: get("/api/v1/economics"),
                    cli: Some(CliExposure { path: vec!["economics".into(), "summary".into()] }),
                },
                tags: ["economics", "evidence", "benchmark"],
                cache: CachePolicy::Process { max_entries: 8, ttl_seconds: Some(5) },
                handler: summary,
            },
            capability! {
                id: "economics.explain",
                title: "Explain one economics metric",
                description: "One metric and everything it rests on: its formula, its measurement class and what that class means, the pairs and runs behind it (including the invalid ones), excluded runs, the suites and revisions, and the commands that reproduce it.",
                input: EconomicsExplainInput,
                output: EconomicsExplanation,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_economics_explain"),
                    http: get("/api/v1/economics/explain"),
                    cli: Some(CliExposure { path: vec!["economics".into(), "explain".into()] }),
                },
                tags: ["economics", "evidence"],
                cache: CachePolicy::Process { max_entries: 32, ttl_seconds: Some(5) },
                handler: explain,
            },
            capability! {
                id: "economics.runs",
                title: "Recorded benchmark runs",
                description: "The raw facts every economics metric is computed from: each recorded run with its task, variant, repetition, model, revision, success-gate verdicts and the totals derived from its provider usage, and the path of its record.",
                input: EconomicsRunsQuery,
                output: EconomicsRunList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_economics_runs"),
                    http: get("/api/v1/economics/runs"),
                    cli: Some(CliExposure { path: vec!["economics".into(), "runs".into()] }),
                },
                tags: ["economics", "evidence"],
                cache: CachePolicy::Process { max_entries: 16, ttl_seconds: Some(5) },
                handler: runs,
            },
            capability! {
                id: "economics.check",
                title: "Refuse unsupported economics claims",
                description: "Scan the repository's hand-written prose and claim sentences for a quantity (a percentage, 'N times fewer') stated next to the economics vocabulary, and check every claim the methodology binds to a metric: a bound claim may be guaranteed only while its metric's evidence meets the binding and is current. `ok: false` names every finding.",
                input: EconomicsCheckInput,
                output: EconomicsCheckReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_economics_check"),
                    http: get("/api/v1/economics/check"),
                    cli: Some(CliExposure { path: vec!["economics".into(), "check".into()] }),
                },
                tags: ["economics", "claims", "gate"],
                handler: check,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place the ids and projection names exist; a refactor
    /// that dropped one would still compile.
    #[test]
    fn the_declaration_yields_the_identity_and_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "economics");
        let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
        assert_eq!(ids, ["economics.summary", "economics.explain", "economics.runs", "economics.check"]);
        for (e, route, tool) in [
            (&m.capabilities[0], "/api/v1/economics", "majordomus_economics"),
            (&m.capabilities[1], "/api/v1/economics/explain", "majordomus_economics_explain"),
            (&m.capabilities[2], "/api/v1/economics/runs", "majordomus_economics_runs"),
            (&m.capabilities[3], "/api/v1/economics/check", "majordomus_economics_check"),
        ] {
            let c = &e.capability;
            assert_eq!(c.exposure.http.as_ref().unwrap().path, route);
            assert_eq!(c.exposure.mcp.as_ref().unwrap().tool.as_deref(), Some(tool));
            assert!(c.kind.is_read_only() && c.kind.is_executable(), "{} must only read", c.id.as_str());
        }
        assert_eq!(
            m.capabilities[0].capability.exposure.mcp.as_ref().unwrap().resource.as_ref().unwrap().uri,
            ECONOMICS_URI
        );
    }
}
