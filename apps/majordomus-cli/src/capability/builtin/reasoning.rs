//! The `reasoning` module: provider-independent reasoning, projected (ADR 0098).
//!
//! Six capabilities over one model (`crate::reasoning`): which advisors can be asked and
//! what they can do (`reasoning.advisors`), what a stated uncertainty calls for
//! (`reasoning.plan`), the one writer of the session's reasoning records
//! (`reasoning.record`), the state those records derive (`reasoning.status`), one
//! record's provenance (`reasoning.explain`), and the checks that keep the design honest
//! (`reasoning.check`). None of them talks to an advisor: the transport is the Node layer
//! (`scripts/lib/advisors/`), which records what it did through `reasoning.record`.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, CapabilityKind, CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::reasoning::availability::LinkedPeer;
use crate::reasoning::check::{ReasoningFinding, CHECKS};
use crate::reasoning::record::{ReasoningInput, ReasoningRecord, RECORD_SCHEMA, RECORD_VERSION};
use crate::reasoning::state::{
    derive, explain, render_markdown, ReasoningExplanation, ReasoningState, ReasoningStep,
};
use crate::reasoning::store::{admit, mint_id, redact, Admission};
use crate::reasoning::{
    plan, AdvisorState, AdvisorStatus, Materiality, ModeResolution, PlanRequest,
    ReasoningConfidence, ReviewPlan,
};
use crate::{capability, module};

use super::{get, mcp, post};

/// The MCP resource of `reasoning.advisors`.
pub const ADVISORS_URI: &str = "majordomus://reasoning/advisors";
/// The MCP resource of `reasoning.status`.
pub const REASONING_URI: &str = "majordomus://reasoning";

pub(crate) use crate::reasoning::Situation as Setting;

pub(crate) fn setting(ctx: &Context) -> Result<Setting, CapabilityError> {
    let peers: Vec<LinkedPeer> = match ctx.mesh.cooperation() {
        Some(coop) => coop
            .peers()
            .into_iter()
            .filter(|p| {
                matches!(
                    p.state,
                    crate::mesh::cooperation::LinkState::Connected
                        | crate::mesh::cooperation::LinkState::Degraded
                )
            })
            .map(|p| LinkedPeer {
                runtime: p.runtime,
                name: p.name,
                features: p.features,
            })
            .collect(),
        None => Vec::new(),
    };
    crate::reasoning::Situation::resolve(
        Path::new(&ctx.index.repository.root),
        ctx.index.share.as_deref(),
        &ctx.index.providers,
        &peers,
    )
    .map_err(CapabilityError::Internal)
}

// ---------------------------------------------------------------- reasoning.advisors

/// One advisory capability and who can provide it now.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AdvisorCapacity {
    /// The capability word.
    pub capability: String,
    /// What it means.
    pub description: String,
    /// Available advisors offering it, in preference order.
    pub available: Vec<String>,
}

/// The answer of `reasoning.advisors`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AdvisorsReport {
    /// Reasoning works whatever the advisors: always `true`, stated so a reader need not
    /// infer it from a count.
    pub operational: bool,
    /// The mode in force and what decided it.
    pub mode: ModeResolution,
    /// Advisors that may be asked now.
    pub available: usize,
    /// Advisors that may not, each optional.
    pub unavailable: usize,
    /// Every advisor, declared ones in preference order, then linked peers.
    pub advisors: Vec<AdvisorState>,
    /// Per advisory capability, who can provide it now. A row with nobody is answered by
    /// the local review path.
    pub capacity: Vec<AdvisorCapacity>,
    /// The catalogue's own findings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

fn advisors_of(s: &Setting) -> AdvisorsReport {
    let available = s
        .states
        .iter()
        .filter(|a| a.status == AdvisorStatus::Available)
        .count();
    AdvisorsReport {
        operational: true,
        mode: s.mode.clone(),
        available,
        unavailable: s.states.len() - available,
        capacity: s
            .catalogue
            .capabilities
            .iter()
            .map(|c| AdvisorCapacity {
                capability: c.id.clone(),
                description: c.description.clone(),
                available: s
                    .states
                    .iter()
                    .filter(|a| {
                        a.status == AdvisorStatus::Available && a.capabilities.contains(&c.id)
                    })
                    .map(|a| a.id.clone())
                    .collect(),
            })
            .collect(),
        advisors: s.states.clone(),
        diagnostics: s.diagnostics.clone(),
    }
}

fn reasoning_advisors(ctx: &Context, _: super::Empty) -> Result<AdvisorsReport, CapabilityError> {
    Ok(advisors_of(&setting(ctx)?))
}

// ---------------------------------------------------------------- reasoning.plan

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
/// The input of `reasoning.plan`: a stated uncertainty, in transport-friendly scalars.
/// Nothing is recorded; `reasoning.record` with a `plan` records one.
pub struct PlanPreviewInput {
    /// `trivial`, `low`, `material`, `high` or `critical`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub materiality: Option<Materiality>,
    /// `low`, `medium` or `high`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ReasoningConfidence>,
    /// Advisory capabilities review needs, comma-separated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<String>,
}

impl BenchmarkCases for PlanPreviewInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "material",
            PlanPreviewInput {
                materiality: Some(Materiality::Material),
                confidence: Some(ReasoningConfidence::Medium),
                capabilities: Some("independent_reasoning,code_review".into()),
            },
        )]
    }
}

fn reasoning_plan(ctx: &Context, input: PlanPreviewInput) -> Result<ReviewPlan, CapabilityError> {
    let s = setting(ctx)?;
    let request = PlanRequest {
        materiality: input.materiality.unwrap_or_default(),
        confidence: input.confidence.unwrap_or_default(),
        capabilities: input
            .capabilities
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect(),
        prior_conclusion: None,
        new_evidence: false,
    };
    for w in &request.capabilities {
        if !s.catalogue.knows_capability(w) {
            return Err(CapabilityError::InvalidInput(format!(
                "'{w}' is not an advisory capability; `majordomus reasoning advisors` lists them"
            )));
        }
    }
    Ok(plan(&request, &s.mode, &s.states))
}

// ---------------------------------------------------------------- reasoning.record

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `reasoning.record`: one record, as authored.
pub struct ReasoningRecordInput {
    /// The record.
    pub record: ReasoningInput,
}

impl BenchmarkCases for ReasoningRecordInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // A plan for an assessment no store holds: the refusal path, which writes nothing.
        vec![NamedCase::new(
            "refused",
            ReasoningRecordInput {
                record: ReasoningInput::Plan(crate::reasoning::record::PlanInput {
                    assessment: "assessment-benchmark-absent".into(),
                }),
            },
        )]
    }
}

/// The answer of `reasoning.record`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningRecorded {
    /// The stored record.
    pub record: ReasoningRecord,
    /// Where it was written, relative to the repository.
    pub path: String,
    /// The step it adds to the timeline: the report a terminal or the Cockpit shows.
    pub step: ReasoningStep,
}

fn reasoning_record(
    ctx: &Context,
    input: ReasoningRecordInput,
) -> Result<ReasoningRecorded, CapabilityError> {
    let s = setting(ctx)?;
    let body = admit(
        input.record,
        &s.loaded,
        &Admission {
            mode: &s.mode,
            availability: &s.states,
        },
    )
    .map_err(|r| CapabilityError::Refused(r.0))?;
    let (body, redacted) = redact(body);
    let recorded_at = crate::reasoning::now_stamp();
    let head = match &ctx.index.repository.git {
        crate::git::GitState::Available(g) => g.head.clone(),
        _ => None,
    };
    let record = ReasoningRecord {
        schema: RECORD_SCHEMA.into(),
        version: RECORD_VERSION,
        id: mint_id(body.kind(), &recorded_at),
        task: s.store.current_task(),
        recorded_at,
        head,
        redacted,
        body,
    };
    let path = s
        .store
        .write(&record)
        .map_err(|e| CapabilityError::Refused(format!("the record could not be written: {e}")))?;
    let mut after = s.loaded;
    after.records.push(record.clone());
    let step = derive(&after, None)
        .timeline
        .into_iter()
        .find(|e| e.id == record.id)
        .ok_or_else(|| CapabilityError::Internal("the record has no timeline step".into()))?;
    Ok(ReasoningRecorded {
        path: path
            .strip_prefix(&ctx.index.repository.root)
            .unwrap_or(&path)
            .display()
            .to_string(),
        record,
        step,
    })
}

// ---------------------------------------------------------------- reasoning.status

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
/// The input of `reasoning.status`.
pub struct ReasoningStatusInput {
    /// A task id, or `all`; the open task when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

impl BenchmarkCases for ReasoningStatusInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("current", ReasoningStatusInput::default()),
            NamedCase::new(
                "all",
                ReasoningStatusInput {
                    task: Some("all".into()),
                },
            ),
        ]
    }
}

/// The answer of `reasoning.status`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningStatus {
    /// The mode in force.
    pub mode: ModeResolution,
    /// Advisors available now.
    pub advisors_available: usize,
    /// Advisors not available now, each optional.
    pub advisors_unavailable: usize,
    /// The state.
    pub state: ReasoningState,
    /// The state as Markdown: what a handover carries.
    pub report: String,
}

fn reasoning_status(
    ctx: &Context,
    input: ReasoningStatusInput,
) -> Result<ReasoningStatus, CapabilityError> {
    let s = setting(ctx)?;
    let task = match input.task.as_deref() {
        Some("all") => None,
        Some(t) => Some(t.to_string()),
        None => Some(s.store.current_task()),
    };
    let state = derive(&s.loaded, task.as_deref());
    let available = s
        .states
        .iter()
        .filter(|a| a.status == AdvisorStatus::Available)
        .count();
    Ok(ReasoningStatus {
        mode: s.mode.clone(),
        advisors_available: available,
        advisors_unavailable: s.states.len() - available,
        report: render_markdown(&state),
        state,
    })
}

// ---------------------------------------------------------------- reasoning.explain

#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `reasoning.explain`.
pub struct ReasoningExplainInput {
    /// A record id: a conclusion, usually.
    pub id: String,
}

impl BenchmarkCases for ReasoningExplainInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "absent",
            ReasoningExplainInput {
                id: "conclusion-benchmark-absent".into(),
            },
        )]
    }
}

fn reasoning_explain(
    ctx: &Context,
    input: ReasoningExplainInput,
) -> Result<ReasoningExplanation, CapabilityError> {
    let s = setting(ctx)?;
    explain(&s.loaded, &input.id)
        .ok_or_else(|| CapabilityError::NotFound(format!("no reasoning record {}", input.id)))
}

// ---------------------------------------------------------------- reasoning.check

/// The answer of `reasoning.check`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReasoningCheck {
    /// No finding.
    pub ok: bool,
    /// The checks performed, in order.
    pub checks: Vec<String>,
    /// What they found.
    pub findings: Vec<ReasoningFinding>,
}

fn reasoning_check(ctx: &Context, _: super::Empty) -> Result<ReasoningCheck, CapabilityError> {
    let s = setting(ctx)?;
    let findings = crate::reasoning::check::run(
        Path::new(&ctx.index.repository.root),
        &s.catalogue,
        &s.refs,
        &s.loaded,
    );
    Ok(ReasoningCheck {
        ok: findings.is_empty(),
        checks: CHECKS.iter().map(|c| c.to_string()).collect(),
        findings,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "reasoning",
        title: "Reasoning",
        description: "Provider-independent reasoning (ADR 0098): which optional advisors can be asked and what they offer, what a material uncertainty calls for, the session's typed reasoning records — assessments, plans, consultations, disagreements, resolutions, conclusions, validations, attempts — and the state, timeline and provenance they derive. Nothing here talks to an advisor, and nothing here needs one: with none available, the plan is a structured local review and the session concludes on its own evidence.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "reasoning.advisors",
                title: "The advisors and what they can do now",
                description: "Every declared advisor — then every linked mesh peer carrying the review feature — with its status (available, unavailable, not_configured, disabled, temporarily_failed, rate_limited) and why, from presence alone (an executable on PATH, a credential variable set; never a value), the reasoning mode and the recorded outcomes of earlier consultations. Plus the capacity per advisory capability. No advisor is an ordinary answer: reasoning is operational either way.",
                input: super::Empty,
                output: AdvisorsReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_reasoning_advisors".into()),
                        resource: Some(crate::capability::model::McpResource {
                            uri: ADVISORS_URI.into(),
                            name: "reasoning-advisors".into(),
                        }),
                    }),
                    http: get("/api/v1/reasoning/advisors"),
                    cli: Some(CliExposure { path: vec!["reasoning".into(), "advisors".into()] }),
                },
                tags: ["reasoning", "advisors"],
                cache: CachePolicy::Disabled,
                handler: reasoning_advisors,
            },
            capability! {
                id: "reasoning.plan",
                title: "What an uncertainty calls for",
                description: "Decide, without recording anything, whether a stated uncertainty warrants independent review, how much the mode allows, and which available advisors a capability-driven selection would ask — with every advisor left out and why. Trivial and low uncertainty is decided locally; material uncertainty with no suitable advisor gets the structured local review instead. Pure over the catalogue, the availability and the input.",
                input: PlanPreviewInput,
                output: ReviewPlan,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_reasoning_plan"),
                    http: get("/api/v1/reasoning/plan"),
                    cli: Some(CliExposure { path: vec!["reasoning".into(), "plan".into()] }),
                },
                tags: ["reasoning", "policy"],
                cache: CachePolicy::Disabled,
                handler: reasoning_plan,
            },
            capability! {
                id: "reasoning.record",
                kind: CapabilityKind::Command,
                title: "Record a reasoning step",
                description: "Write one typed reasoning record of the open task under .ai/local/state/reasoning/: an assessment (with its evidence), a plan (computed here from the availability of this moment, never supplied), a consultation (only of an advisor its plan selected), a disagreement, a resolution (evidence, never a count), a conclusion (its review count computed from the consultations it cites; refused while a disagreement on its assessment is unsettled), a validation or a failed attempt. Secret shapes are redacted before anything is written.",
                input: ReasoningRecordInput,
                output: ReasoningRecorded,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_reasoning_record"),
                    http: post("/api/v1/reasoning/records"),
                    cli: Some(CliExposure { path: vec!["reasoning".into(), "record".into()] }),
                },
                tags: ["reasoning", "records"],
                cache: CachePolicy::Disabled,
                handler: reasoning_record,
            }
            .writes_repository(),
            capability! {
                id: "reasoning.status",
                title: "The session's reasoning state",
                description: "The reasoning state of the open task (or of a named one, or of all): assessments and where each stands, consultations and how each ended, disagreements and what settled them, conclusions with their computed review and validation, the timeline, token totals the adapters reported, and the Markdown report a handover carries. Derived from the records alone, deterministically.",
                input: ReasoningStatusInput,
                output: ReasoningStatus,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_reasoning".into()),
                        resource: Some(crate::capability::model::McpResource {
                            uri: REASONING_URI.into(),
                            name: "reasoning".into(),
                        }),
                    }),
                    http: get("/api/v1/reasoning"),
                    cli: Some(CliExposure { path: vec!["reasoning".into(), "status".into()] }),
                },
                tags: ["reasoning", "session"],
                cache: CachePolicy::Disabled,
                handler: reasoning_status,
            },
            capability! {
                id: "reasoning.explain",
                title: "Why a decision was made",
                description: "One reasoning record with the whole chain of its assessment: the uncertainty and its evidence, the plan and who it selected or why nobody, each consultation, each disagreement and the experiment that settled it, the conclusion and its validation — the provenance of a decision without the conversation that produced it.",
                input: ReasoningExplainInput,
                output: ReasoningExplanation,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_reasoning_explain"),
                    http: get("/api/v1/reasoning/explain"),
                    cli: Some(CliExposure { path: vec!["reasoning".into(), "explain".into()] }),
                },
                tags: ["reasoning", "provenance"],
                cache: CachePolicy::Disabled,
                // reasoning records are checkout state under .ai/local: no benchmark
                // repository holds one to explain, so there is nothing to time but a miss
                benchmark: crate::capability::model::BenchmarkPolicy::Waived {
                    reason: crate::capability::model::WaiverReason::TransientState,
                },
                handler: reasoning_explain,
            },
            capability! {
                id: "reasoning.check",
                title: "Check that reasoning stays provider-independent",
                description: "The executable half of the rules advisors-are-optional and review-is-recorded-not-claimed: the advisor catalogue resolves against the provider table and the model catalogue; every adapter it names has its transport module; no provider-independent source names an advisor; no CI workflow or gate names a model credential; no document asserts a refused claim; every stored consultation names an advisor its plan selected, and every conclusion's review matches what it cites.",
                input: super::Empty,
                output: ReasoningCheck,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_reasoning_check"),
                    http: get("/api/v1/reasoning/check"),
                    cli: Some(CliExposure { path: vec!["reasoning".into(), "check".into()] }),
                },
                tags: ["reasoning", "governance"],
                cache: CachePolicy::Disabled,
                handler: reasoning_check,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "reasoning");
        let expected: &[(&str, &str, &str)] = &[
            (
                "reasoning.advisors",
                "majordomus_reasoning_advisors",
                "/api/v1/reasoning/advisors",
            ),
            (
                "reasoning.plan",
                "majordomus_reasoning_plan",
                "/api/v1/reasoning/plan",
            ),
            (
                "reasoning.record",
                "majordomus_reasoning_record",
                "/api/v1/reasoning/records",
            ),
            (
                "reasoning.status",
                "majordomus_reasoning",
                "/api/v1/reasoning",
            ),
            (
                "reasoning.explain",
                "majordomus_reasoning_explain",
                "/api/v1/reasoning/explain",
            ),
            (
                "reasoning.check",
                "majordomus_reasoning_check",
                "/api/v1/reasoning/check",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(
            ids,
            expected.iter().map(|(id, _, _)| *id).collect::<Vec<_>>()
        );
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id}"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id}"
            );
        }
    }

    #[test]
    fn only_the_record_writes() {
        use crate::capability::model::Effect;
        for e in module().capabilities {
            let writes = e.capability.execution.effect == Effect::RepositoryMutation;
            assert_eq!(
                writes,
                e.capability.id.as_str() == "reasoning.record",
                "{}",
                e.capability.id
            );
        }
    }
}
