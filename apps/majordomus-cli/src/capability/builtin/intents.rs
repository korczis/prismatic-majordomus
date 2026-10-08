//! The `intents` module:
//!
//! The module is `intents` and not `intent` because the registry composes builtin modules and
//! declarative kinds into one namespace, and `intent` is the **kind** of the records under
//! `.ai/repo/project/intents/`; the registry refuses the collision, as it did for `session`.
//! The command line keeps the singular word, `majordomus intent <verb>`.
//!
//! What it answers: what must become true, derived against the plan and the evidence.
//!
//! Every capability here reads [`crate::intent::Intents`], built on demand from the index,
//! the plan [`crate::plan::Plan::build`] derives from the same index, and the evidence
//! ledger. All six are read-only: there is no intent status to write, because the stage is
//! derived, and no transition, because the plan already owns the lifecycle of the work an
//! intent is realised by (ADR 0070), and the coverage of its criteria by the plan (ADR 0073).
//!
//! ```
//! use majordomus_cli::capability::builtin::intents;
//! let m = intents::module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, ["intents.list", "intents.record", "intents.validate",
//!                  "intents.coverage", "intents.preflight", "intents.binding"]);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::intent::{
    IntentFinding, IntentPreflight, IntentView, Intents, RepositoryEvidence, INTENT,
};
use crate::intent_binding::{bind, BindingRequest, IntentBinding};
use crate::intent_plan::IntentCoverage;
use crate::intent_review::{CritiqueRecord, GapRecord};
use crate::plan::Plan;
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which every intent, derived, is read as an MCP resource.
pub const INTENTS_URI: &str = "majordomus://intents";

// ---------------------------------------------------------------- views

/// Every intent, with its derived stage and verdict, and how many there are in each: what
/// `majordomus intent list`, `GET /api/v1/intents` and the `majordomus_intents` tool all answer
/// with, out of one derivation.
///
/// ```
/// use majordomus_cli::capability::builtin::intents::IntentList;
/// let empty = IntentList {
///     count: 0,
///     stages: std::collections::BTreeMap::new(),
///     verdicts: std::collections::BTreeMap::new(),
///     intents: vec![],
/// };
/// // a repository that declares no intent answers the route, with nothing in it
/// assert_eq!(empty.count, 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentList {
    /// How many intents.
    pub count: usize,
    /// How many in each stage, keyed by the stage word.
    pub stages: std::collections::BTreeMap<String, usize>,
    /// How many with each verdict, keyed by the verdict word.
    pub verdicts: std::collections::BTreeMap<String, usize>,
    /// Every intent, in identity order.
    pub intents: Vec<IntentView>,
}

/// What the intent model refuses: whether it is valid, how many intents were examined, and
/// every finding with its level. Warnings are reported and do not make it invalid.
///
/// ```
/// use majordomus_cli::capability::builtin::intents::IntentValidation;
/// let v = IntentValidation {
///     valid: true,
///     intents: 2,
///     failures: 0,
///     warnings: 20,
///     findings: vec![],
/// };
/// // twenty warnings and no failure is a valid model
/// assert!(v.valid);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentValidation {
    /// True when no finding is a failure. Warnings do not make it false.
    pub valid: bool,
    /// How many intents were examined.
    pub intents: usize,
    /// How many findings are failures.
    pub failures: usize,
    /// How many findings are warnings.
    pub warnings: usize,
    /// Every finding.
    pub findings: Vec<IntentFinding>,
}

// ---------------------------------------------------------------- inputs

/// Which intent to read, by the id that is also its file name. An id this repository does not
/// hold is a refusal naming what was looked for, never an empty answer.
///
/// ```
/// use majordomus_cli::capability::builtin::intents::IntentRecordInput;
/// let input = IntentRecordInput { id: "intent-lifecycle".into() };
/// assert_eq!(input.id, "intent-lifecycle");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentRecordInput {
    /// The intent's id, which is also its file name under `.ai/repo/project/intents/`.
    pub id: String,
}

impl BenchmarkCases for IntentRecordInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let id = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == INTENT)
            .map(|o| o.identity.clone());
        // a repository without an intent still answers the route, with the refusal, so the
        // required parameter keeps an example in the OpenAPI document
        vec![NamedCase::new(
            if id.is_some() {
                "first-intent"
            } else {
                "absent"
            },
            IntentRecordInput {
                id: id.unwrap_or_else(|| "absent".into()),
            },
        )]
    }
}

/// The work to preflight: an issue, or the paths it will touch.
///
/// `paths` is one comma-separated string rather than a list because a preflight changes
/// nothing and is therefore a `GET`, and this repository binds a `GET` input to the query
/// string.
///
/// ```
/// use majordomus_cli::capability::builtin::intents::IntentPreflightInput;
/// // an issue, when the work executes one
/// let by_issue = IntentPreflightInput { issue: Some("I1900".into()), paths: String::new() };
/// assert!(by_issue.path_list().is_empty());
/// // or the paths it will touch, when it names no issue yet
/// let by_paths = IntentPreflightInput {
///     issue: None,
///     paths: "apps/majordomus-cli/src,docs".into(),
/// };
/// assert_eq!(by_paths.path_list(), ["apps/majordomus-cli/src", "docs"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentPreflightInput {
    /// The issue the work executes, by id. When given, the paths are not consulted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// Repository-relative paths the work will touch, separated by commas.
    #[serde(default)]
    pub paths: String,
}

impl IntentPreflightInput {
    /// The paths as a list: trimmed, with the empties a trailing or doubled comma leaves
    /// dropped, so a query string written by hand resolves the same way as one written by a
    /// program.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::intents::IntentPreflightInput;
    /// let i = IntentPreflightInput { issue: None, paths: " lib, ,docs/X.md".into() };
    /// assert_eq!(i.path_list(), ["lib", "docs/X.md"]);
    /// ```
    pub fn path_list(&self) -> Vec<String> {
        self.paths
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }
}

impl BenchmarkCases for IntentPreflightInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let plan = Plan::build(ctx.index);
        match plan.issues.first() {
            Some(i) => vec![NamedCase::new(
                "first-issue",
                IntentPreflightInput {
                    issue: Some(i.id.clone()),
                    paths: String::new(),
                },
            )],
            None => vec![NamedCase::new(
                "paths",
                IntentPreflightInput {
                    issue: None,
                    paths: "README.md".into(),
                },
            )],
        }
    }
}

/// The input of `intents.binding`: what a worker names when it asks what its work serves.
/// Flat strings, because the same value arrives as a query string, as MCP arguments and as
/// the flags of `majordomus intent binding`.
///
/// ```
/// use majordomus_cli::capability::builtin::intents::IntentBindingInput;
/// let i = IntentBindingInput { issue: Some("I1900".into()), paths: "lib, docs".into(), ..Default::default() };
/// let r = i.request();
/// assert_eq!(r.issue.as_deref(), Some("I1900"));
/// assert_eq!(r.paths, ["lib", "docs"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentBindingInput {
    /// The issue the work executes, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The intent the work serves, by id, when the worker names it directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// Repository-relative paths the work will touch, separated by commas.
    #[serde(default)]
    pub paths: String,
    /// An exemption class the policy declares under `intent.exemptions`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exemption: Option<String>,
    /// Why the exemption applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub because: Option<String>,
}

impl IntentBindingInput {
    /// The typed request: the paths split at commas, everything else as given.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::intents::IntentBindingInput;
    /// assert!(IntentBindingInput::default().request().trimmed().names_nothing());
    /// ```
    pub fn request(&self) -> BindingRequest {
        BindingRequest {
            issue: self.issue.clone(),
            intent: self.intent.clone(),
            paths: self
                .paths
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect(),
            exemption: self.exemption.clone(),
            because: self.because.clone(),
        }
    }
}

impl BenchmarkCases for IntentBindingInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let plan = Plan::build(ctx.index);
        match plan.issues.first() {
            Some(i) => vec![NamedCase::new(
                "first-issue",
                IntentBindingInput {
                    issue: Some(i.id.clone()),
                    ..Default::default()
                },
            )],
            None => vec![NamedCase::new(
                "paths",
                IntentBindingInput {
                    paths: "README.md".into(),
                    ..Default::default()
                },
            )],
        }
    }
}

// ---------------------------------------------------------------- handlers

fn derived(ctx: &Context) -> Result<(Plan, Intents), CapabilityError> {
    let plan = Plan::build(&ctx.index);
    let evidence = RepositoryEvidence::load(&ctx.index)
        .map_err(|e| CapabilityError::Internal(e.to_string()))?;
    let intents = Intents::build(&ctx.index, &plan, &evidence);
    Ok((plan, intents))
}

fn intent_list(ctx: &Context, _: Empty) -> Result<IntentList, CapabilityError> {
    let (_, intents) = derived(ctx)?;
    let mut stages = std::collections::BTreeMap::new();
    let mut verdicts = std::collections::BTreeMap::new();
    for i in &intents.intents {
        *stages.entry(i.stage.as_str().to_string()).or_insert(0) += 1;
        *verdicts
            .entry(i.verdict.state.as_str().to_string())
            .or_insert(0) += 1;
    }
    Ok(IntentList {
        count: intents.intents.len(),
        stages,
        verdicts,
        intents: intents.intents,
    })
}

fn intent_record(ctx: &Context, input: IntentRecordInput) -> Result<IntentView, CapabilityError> {
    let (_, intents) = derived(ctx)?;
    intents.intent(&input.id).cloned().ok_or_else(|| {
        CapabilityError::NotFound(format!(
            "no intent '{}'; `intents.list` names every one this repository holds",
            input.id
        ))
    })
}

fn intent_validate(ctx: &Context, _: Empty) -> Result<IntentValidation, CapabilityError> {
    let (_, intents) = derived(ctx)?;
    Ok(IntentValidation {
        valid: intents.failures() == 0,
        intents: intents.intents.len(),
        failures: intents.failures(),
        warnings: intents.warnings(),
        findings: intents.findings,
    })
}

fn intent_coverage(ctx: &Context, _: Empty) -> Result<IntentCoverage, CapabilityError> {
    let plan = Plan::build(&ctx.index);
    Ok(Intents::coverage(&ctx.index, &plan))
}

fn intent_preflight(
    ctx: &Context,
    input: IntentPreflightInput,
) -> Result<IntentPreflight, CapabilityError> {
    let paths = input.path_list();
    let issue = input
        .issue
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if issue.is_none() && paths.is_empty() {
        return Err(CapabilityError::InvalidInput(
            "name the issue the work executes, or the paths it will touch".into(),
        ));
    }
    let (plan, intents) = derived(ctx)?;
    Ok(intents.preflight(
        &plan,
        &GapRecord::all(&ctx.index),
        &CritiqueRecord::all(&ctx.index),
        issue,
        &paths,
    ))
}

/// The policy's `intent:` block, or its default when the repository or the policy cannot be
/// read: no exemption class is then declared, so none can be given, which is the refusal an
/// unreadable policy owes rather than a class invented for it.
fn intent_policy(ctx: &Context) -> crate::policy::IntentPolicy {
    crate::intent_binding::policy_of(&ctx.index)
}

fn intent_binding(
    ctx: &Context,
    input: IntentBindingInput,
) -> Result<IntentBinding, CapabilityError> {
    derived(ctx).map(|(plan, intents)| {
        bind(
            &intents,
            &plan,
            &GapRecord::all(&ctx.index),
            &CritiqueRecord::all(&ctx.index),
            &intent_policy(ctx),
            input.request(),
        )
    })
}

/// The module the registry composes: six read-only capabilities over one derivation, each
/// declared once here and projected to the command line, HTTP, MCP and OpenAPI from that
/// declaration.
///
/// ```
/// use majordomus_cli::capability::builtin::intents;
/// let m = intents::module();
/// assert_eq!(m.id.as_str(), "intents");
/// // every capability of this module is declared under its own name
/// assert!(m
///     .capabilities
///     .iter()
///     .all(|e| e.capability.id.as_str().starts_with("intents.")));
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "intents",
        title: "Intent",
        description: "What must become true above the milestones that realise it: each intent's statement, invariants and satisfaction criteria, its stage derived from the plan's milestone status, each criterion's state derived from the evidence ledger, and its verdict derived from those criteria alone. Nothing is stored and nothing transitions; an intent added under the project model is answered by all of these without a registration anywhere.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "intents.list",
                title: "Every intent, with its derived stage and verdict",
                description: "Every intent the project model declares, each with the status the plan derives for its milestones, the state of the evidence behind each satisfaction criterion, the stage those two derive — declared, planned, executing, verifying or satisfied, or cancelled or superseded when the record says so — and the verdict the criteria alone derive (ADR 0107): satisfied when every criterion is met, unsatisfied when a test or claim criterion is not, unknown when only command or deployment criteria are unmet or none is declared, with the criteria holding it back.",
                input: Empty,
                output: IntentList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_intents".into()),
                        resource: Some(McpResource { uri: INTENTS_URI.into(), name: "intents".into() }),
                    }),
                    http: get("/api/v1/intents"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "list".into()] }),
                },
                tags: ["intent", "project"],
                cache: CachePolicy::Disabled,
                handler: intent_list,
            },
            capability! {
                id: "intents.record",
                title: "One intent, with everything derived about it",
                description: "One intent in full: its statement and invariants as authored, each milestone with the status the plan derives, each satisfaction criterion with the state of its evidence and the command that reproduces it, the stage, and the verdict the criteria alone derive with the criteria holding it back. The record's own file stays at `majordomus://intent/<id>`.",
                input: IntentRecordInput,
                output: IntentView,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_record"),
                    http: get("/api/v1/intents/record"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "show".into()] }),
                },
                tags: ["intent", "project"],
                cache: CachePolicy::Disabled,
                handler: intent_record,
            },
            capability! {
                id: "intents.validate",
                title: "What the intent model refuses",
                description: "Every finding over the intents: an intent naming no milestone or no criterion, a milestone that does not resolve, a criterion with no evidence reference or one that names nothing this repository holds, governance that does not resolve, a successor that is not an intent, a file name that disagrees with its id — each a failure — and a milestone no intent serves, a warning.",
                input: Empty,
                output: IntentValidation,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_validate"),
                    http: get("/api/v1/intents/validate"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "validate".into()] }),
                },
                tags: ["intent", "project", "validation"],
                cache: CachePolicy::Disabled,
                handler: intent_validate,
            },
            capability! {
                id: "intents.coverage",
                title: "Which work carries which criterion, and why each issue exists",
                description: "The plan read against the intents (ADR 0073): every criterion of every live intent with the live issues that serve it, the milestones they belong to, and its strength — covered, weakly covered when no serving issue requires evidence, observed when the recorded gap saw it already true, or uncovered when nothing in the plan will make it true — and every issue with the reason it exists: the criteria it serves, maintenance under a milestone no intent names, or unexplained under one that realises an intent.",
                input: Empty,
                output: IntentCoverage,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_coverage"),
                    http: get("/api/v1/intents/coverage"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "coverage".into()] }),
                },
                tags: ["intent", "project", "planning"],
                cache: CachePolicy::Disabled,
                handler: intent_coverage,
            },
            capability! {
                id: "intents.preflight",
                title: "Which intent a piece of work serves, or why it may not proceed",
                description: "Given the issue a piece of work executes, or the paths it will touch, one verdict: `serves` when no issue is refused and at least one serves a criterion of a live intent through a link that holds, the plan of each such intent critiqued with no blocking finding open (issues judged maintenance beside it do not change the verdict); `maintenance` when the issues sit under milestones no live intent names and serve nothing, as `intent validate` allows; `refused` otherwise, each refusal with its issue, a cause — unknown_issue, no_issue_covers_paths, issue_serves_nothing, serves_another_intent, serves_unknown_criterion, intent_not_critiqued, open_blocking_finding — in path mode every issue judged and the worst verdict answered. The answer carries, for the intents reached and no others, the statement, the served criteria with the live state of their evidence, the invariants, non-goals and governance, the critique with its open blocking findings, and the recorded gap bounded to those criteria.",
                input: IntentPreflightInput,
                output: IntentPreflight,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_preflight"),
                    http: get("/api/v1/intents/preflight"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "preflight".into()] }),
                },
                tags: ["intent", "project", "governance"],
                cache: CachePolicy::Disabled,
                handler: intent_preflight,
            },
            capability! {
                id: "intents.binding",
                title: "What a piece of work is bound to before it starts",
                description: "What a task asks before it starts (ADR 0111): given the issue the work executes, the intent it serves, the paths it will touch, or an exemption class with its reason, one standing — `bound` when the work serves a criterion of a live intent through links the preflight accepts, `maintenance` when its issues sit under milestones no live intent names, `exempt` when the class is one the policy declares under `intent.exemptions` and a reason was given, `refused` otherwise, each refusal with a cause: the preflight's own seven, and nothing_named, unknown_intent, intent_retired, intent_has_no_open_work, issue_outside_intent, ambiguous_intent (paths alone reached more than one intent), unknown_exemption, exemption_without_reason, exemption_names_work. The answer carries what was named, the issues and intents resolved with what each intent asks of the worker, a note when the paths lie outside the named issue's scope, and two pins a later reader compares: `plan_revision`, which moves when the intent, a link or the critique is edited, and `evidence_standing`, which moves when a served criterion's evidence changes state. Nothing is stored.",
                input: IntentBindingInput,
                output: IntentBinding,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_binding"),
                    http: get("/api/v1/intents/binding"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "binding".into()] }),
                },
                tags: ["intent", "project", "governance"],
                cache: CachePolicy::Disabled,
                handler: intent_binding,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist, and every projection is derived
    /// from it. A refactor that dropped an exposure or renamed a route would still compile;
    /// this is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "intents");
        let expected: &[(&str, &str, &str, &[&str])] = &[
            (
                "intents.list",
                "majordomus_intents",
                "/api/v1/intents",
                &["intent", "list"],
            ),
            (
                "intents.record",
                "majordomus_intent_record",
                "/api/v1/intents/record",
                &["intent", "show"],
            ),
            (
                "intents.validate",
                "majordomus_intent_validate",
                "/api/v1/intents/validate",
                &["intent", "validate"],
            ),
            (
                "intents.coverage",
                "majordomus_intent_coverage",
                "/api/v1/intents/coverage",
                &["intent", "coverage"],
            ),
            (
                "intents.preflight",
                "majordomus_intent_preflight",
                "/api/v1/intents/preflight",
                &["intent", "preflight"],
            ),
            (
                "intents.binding",
                "majordomus_intent_binding",
                "/api/v1/intents/binding",
                &["intent", "binding"],
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, ..)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
        for (executable, (id, tool, path, cli)) in m.capabilities.iter().zip(expected) {
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
            assert_eq!(
                exposure.cli.as_ref().map(|c| c.path.clone()),
                Some(cli.iter().map(|w| w.to_string()).collect::<Vec<_>>()),
                "{id} lost or renamed its command"
            );
        }
    }

    /// Nothing in this module writes: the stage is derived, so there is nothing to record.
    #[test]
    fn no_intent_capability_writes_the_repository() {
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
