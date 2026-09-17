//! The `intent_realization` module: which work realises which intent, and why an intent stands
//! where it stands.
//!
//! Two read-only capabilities over [`crate::intent_realization`]. `intent_realization.work`
//! joins this checkout's ledger tasks (with every episode, provider and handover), the shared
//! layer's closed session records and the peer board to the intents, each link carrying its
//! provenance, and reports each intent's unmet criteria and drift. `intent_realization.explain`
//! answers one intent: the sentences its stage rests on, the plan's coverage of its criteria and
//! the work realising it. Nothing is stored: a session ending or a provider changing moves no
//! intent, because no intent state was ever held by either (ADR 0075).
//!
//! ```
//! use majordomus_cli::capability::builtin::intent_realization;
//! let m = intent_realization::module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, ["intent_realization.work", "intent_realization.explain"]);
//! ```

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::intent::{IntentFinding, Intents, RepositoryEvidence, INTENT};
use crate::intent_realization::{explain, gather, realize, IntentExplanation, IntentRealization};
use crate::plan::Plan;
use crate::{capability, module};

use super::{get, mcp};

/// Which intent to answer the realization of; every intent when absent.
///
/// ```
/// use majordomus_cli::capability::builtin::intent_realization::IntentRealizationInput;
/// let all = IntentRealizationInput::default();
/// assert!(all.intent.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentRealizationInput {
    /// An intent id. When given, only that intent, the work linked to it and its findings are
    /// answered; the unlinked work is left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
}

fn first_intent(ctx: &CaseContext<'_>) -> Option<String> {
    ctx.index
        .objects
        .iter()
        .find(|o| o.kind == INTENT)
        .map(|o| o.identity.clone())
}

impl BenchmarkCases for IntentRealizationInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases = vec![NamedCase::new(
            "every-intent",
            IntentRealizationInput::default(),
        )];
        if let Some(id) = first_intent(ctx) {
            cases.push(NamedCase::new(
                "first-intent",
                IntentRealizationInput { intent: Some(id) },
            ));
        }
        cases
    }
}

/// Which intent to explain, by the id that is also its file name.
///
/// ```
/// use majordomus_cli::capability::builtin::intent_realization::IntentExplainInput;
/// let input = IntentExplainInput { id: "intent-lifecycle".into() };
/// assert_eq!(input.id, "intent-lifecycle");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentExplainInput {
    /// The intent's id.
    pub id: String,
}

impl BenchmarkCases for IntentExplainInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let id = first_intent(ctx);
        vec![NamedCase::new(
            if id.is_some() {
                "first-intent"
            } else {
                "absent"
            },
            IntentExplainInput {
                id: id.unwrap_or_else(|| "absent".into()),
            },
        )]
    }
}

fn derived(ctx: &Context) -> Result<(Plan, Intents, IntentRealization), CapabilityError> {
    let plan = Plan::build(&ctx.index);
    let evidence = RepositoryEvidence::load(&ctx.index)
        .map_err(|e| CapabilityError::Internal(e.to_string()))?;
    let intents = Intents::build(&ctx.index, &plan, &evidence);
    let root = PathBuf::from(&ctx.index.repository.root);
    let (units, skipped) = gather(&root, &ctx.index, &ctx.peers.list());
    let mut realization = realize(&intents, &plan, units);
    if skipped > 0 {
        realization.findings.push(IntentFinding {
            level: crate::intent::WARN.into(),
            code: "ledger_lines_skipped".into(),
            subject: ".ai/local/state/ledger.jsonl".into(),
            message: format!(
                "{skipped} ledger line(s) could not be read, so the work they recorded is not \
                 joined; run `majordomus doctor`"
            ),
            reproduce: crate::intent_realization::REPRODUCE.into(),
        });
    }
    Ok((plan, intents, realization))
}

fn not_found(id: &str) -> CapabilityError {
    CapabilityError::NotFound(format!(
        "no intent '{id}'; `intents.list` names every one this repository holds"
    ))
}

fn realization_work(
    ctx: &Context,
    input: IntentRealizationInput,
) -> Result<IntentRealization, CapabilityError> {
    let (_, intents, mut r) = derived(ctx)?;
    let Some(id) = input
        .intent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(r);
    };
    if intents.intent(id).is_none() {
        return Err(not_found(id));
    }
    r.intents.retain(|v| v.intent == id);
    r.work.retain(|w| w.links.iter().any(|l| l.intent == id));
    r.findings.retain(|f| f.subject == id);
    r.orphans = 0;
    Ok(r)
}

fn realization_explain(
    ctx: &Context,
    input: IntentExplainInput,
) -> Result<IntentExplanation, CapabilityError> {
    let (plan, intents, r) = derived(ctx)?;
    let view = intents
        .intent(input.id.trim())
        .ok_or_else(|| not_found(&input.id))?;
    let coverage = Intents::coverage(&ctx.index, &plan);
    Ok(explain(view, &coverage, &r))
}

/// The module the registry composes: two read-only capabilities over one join, each declared
/// once here and projected to the command line, HTTP, MCP and OpenAPI from that declaration.
///
/// ```
/// use majordomus_cli::capability::builtin::intent_realization;
/// let m = intent_realization::module();
/// assert_eq!(m.id.as_str(), "intent_realization");
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "intent_realization",
        title: "Intent realization",
        description: "Which work realises which intent: every task of this checkout's ledger with the episodes, providers and handovers that carried it, every closed session record and every claim on the peer board, joined through the plan to the intents they serve, each link marked declared, observed, derived or inferred; each intent's unmet criteria with the issues serving them; and the drift between closed work and current evidence. Derived on every read and stored nowhere, so an intent outlives every session and provider that worked on it.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "intent_realization.work",
                title: "Which work realises which intent, and what each still lacks",
                description: "Every unit of work this checkout can see — ledger tasks with their episodes, providers and handovers, closed session records, peer claims — with every intent it realises through issue, milestone and criterion, each link's provenance (declared: the work cites the issue; observed: its episode moved the issue; derived: its branch names the issue; inferred: only an open issue's scope overlaps) or the first missing link; every intent with its unmet criteria and the issues serving each, the work and providers realising it, and its drift: closed_work_contradicted when every milestone is DONE and a criterion's evidence is stale or failing, closed_work_unproven when it never existed, criterion_closed_unmet when every serving issue is DONE and the criterion is not met. Live work serving no intent is a warning.",
                input: IntentRealizationInput,
                output: IntentRealization,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_realization"),
                    http: get("/api/v1/intents/realization"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "realization".into()] }),
                },
                tags: ["intent", "session", "handover", "peers"],
                cache: CachePolicy::Disabled,
                handler: realization_work,
            },
            capability! {
                id: "intent_realization.explain",
                title: "Why an intent stands where it stands",
                description: "One intent explained from the derivations alone: a sentence for its stage naming each milestone's derived status, a sentence per criterion naming its evidence state, the plan's coverage of it and the command that reproduces it, a sentence for the work and providers realising it, and each drift finding; beside them the derived intent, the coverage of its criteria, its realization and the work linked to it.",
                input: IntentExplainInput,
                output: IntentExplanation,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_explain"),
                    http: get("/api/v1/intents/explain"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "explain".into()] }),
                },
                tags: ["intent", "evidence", "explain"],
                cache: CachePolicy::Disabled,
                handler: realization_explain,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; a refactor that dropped an exposure
    /// would still compile, and this is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        let expected = [
            (
                "intent_realization.work",
                "majordomus_intent_realization",
                "/api/v1/intents/realization",
                ["intent", "realization"],
            ),
            (
                "intent_realization.explain",
                "majordomus_intent_explain",
                "/api/v1/intents/explain",
                ["intent", "explain"],
            ),
        ];
        assert_eq!(m.capabilities.len(), expected.len());
        for (e, (id, tool, path, cli)) in m.capabilities.iter().zip(expected) {
            let x = &e.capability.exposure;
            assert_eq!(e.capability.id.as_str(), id);
            assert_eq!(x.mcp.as_ref().and_then(|m| m.tool.as_deref()), Some(tool));
            assert_eq!(x.http.as_ref().map(|h| h.path.as_str()), Some(path));
            assert_eq!(
                x.cli.as_ref().map(|c| c.path.clone()),
                Some(cli.iter().map(|w| w.to_string()).collect::<Vec<_>>())
            );
        }
    }

    /// Nothing here writes: realization is a join over records other processes wrote.
    #[test]
    fn no_realization_capability_writes_the_repository() {
        for e in module().capabilities {
            assert_ne!(
                e.capability.execution.effect,
                crate::capability::Effect::RepositoryMutation,
                "{} claims to write",
                e.capability.id
            );
        }
    }
}
