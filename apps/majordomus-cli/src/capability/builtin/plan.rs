//! The `plan` module: the milestone and issue model's *derivations*, projected.
//!
//! The records themselves have always been served — `majordomus://issue/I0001` returns the
//! file through `objects.get` — and that is the least interesting half of the model. What
//! makes it worth having is everything nobody authored: which issues are READY, which are
//! BLOCKED and on what, the topological waves the graph allows to run at once, the roadmap
//! order the milestone graph implies, the milestone a worker is on and the one issue to
//! take next. Until this module those lived only in `lib/plan.sh`, so an agent asking the
//! shared MCP server what to work on could be handed 184 documents and no answer.
//!
//! Every capability here reads [`crate::plan::Plan`], built on the call out of records the
//! index already holds. Nothing is cached: the plan changes under the process — a
//! transition writes a lifecycle marker into a record between two calls — and a `next`
//! that answered from a snapshot would send two workers to one issue.
//!
//! One capability here writes: `plan.transition` moves an issue through `start`, `verify`
//! and `done`. It is the first development operation of the runtime, and it is here because
//! [ADR 0040] decided that development semantics are capabilities of this registry and every
//! surface a consumer of them — not because the registry's read-only habit was an accident.
//! Until it landed, an agent could be told what to work on and had no way to say it had
//! started; the derivation and the transition were in two programs, and only one of them was
//! reachable from MCP, HTTP or the Cockpit.
//!
//! What it does not do is own the semantics. The guards are [`crate::plan::check`], the
//! resulting status is computed by the one `derive` every reader uses, and the record it
//! writes is the same `.ai/repo/project/issues/<id>.yaml` `lib/plan.sh` writes, to the byte.
//! `plan evidence` stays a command-line operation for now: it appends a block rather than
//! setting a field, and it is the next transition to converge, not this one.
//!
//! [ADR 0040]: ../../../../../.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::model::CapabilityKind;
use crate::plan::{
    Plan, PlanCounts, PlanFinding, PlanIssue, PlanMilestone, PlanProject, PlanVocabulary, PlanWave,
    Transition, TransitionError,
};
use crate::{capability, module};

use super::{get, mcp, post, Empty};

/// The URI under which the whole derived plan is read as an MCP resource.
pub const PLAN_URI: &str = "majordomus://plan";

// ---------------------------------------------------------------- the plan, from a context

/// The derived plan of the repository this process serves. Built on the call: the plan
/// changes under the process.
///
/// A repository with no `.ai/repo/project/` answers with an empty plan rather than a
/// refusal. The shell tool exits 12 there because it is telling a person where the model
/// would live; a projection is answering what this repository holds, and "no milestones and
/// no issues" is that answer. Only a named record that does not exist is a `not found`.
fn plan_of(ctx: &Context) -> Result<Plan, CapabilityError> {
    Ok(Plan::build(&ctx.index))
}

// ---------------------------------------------------------------- inputs

/// The milestone the benchmark cases name, declared once because three case sets need it.
///
/// A repository with no plan still has to be timed on these capabilities — the coverage
/// gate counts targets, not repositories — and its OpenAPI operation still has to show an
/// example of every parameter it declares, because the examples *are* the cases and case
/// 92 refuses a parameter no case ever sets. So the fallback names a milestone that exists
/// nowhere: the empty answer is the operation being measured there, and it is the answer a
/// caller filtering on a milestone that does not exist gets.
fn case_milestone(plan: &Plan) -> String {
    plan.milestones
        .first()
        .map_or_else(|| "M000".to_string(), |m| m.id.clone())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which issues to answer with.
pub struct PlanIssueFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only issues of this milestone. Default: every issue of the plan.
    pub milestone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only issues in this derived status — `READY` for the ready set, `BLOCKED` for the
    /// blocked set. The vocabulary travels with every answer, so a caller never has to know
    /// which statuses exist.
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only issues in this execution wave.
    pub wave: Option<u32>,
}

impl BenchmarkCases for PlanIssueFilter {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", PlanIssueFilter::default()),
            NamedCase::new(
                "ready",
                PlanIssueFilter {
                    status: Some("READY".into()),
                    ..PlanIssueFilter::default()
                },
            ),
            NamedCase::new(
                "one-milestone",
                PlanIssueFilter {
                    milestone: Some(case_milestone(&Plan::build(ctx.index))),
                    ..PlanIssueFilter::default()
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which part of the plan to answer about.
pub struct PlanMilestoneFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Restrict the answer to one milestone. Default: the whole plan, and for `next` the
    /// active milestone with the rest of the plan as the fallback.
    pub milestone: Option<String>,
}

impl BenchmarkCases for PlanMilestoneFilter {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("whole-plan", PlanMilestoneFilter::default()),
            NamedCase::new(
                "one-milestone",
                PlanMilestoneFilter {
                    milestone: Some(case_milestone(&Plan::build(ctx.index))),
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One record of the plan.
pub struct PlanRecordInput {
    /// The id of a milestone or an issue, as its file is named (`I0001`, `work-graph-github`).
    pub id: String,
}

impl BenchmarkCases for PlanRecordInput {
    /// The fallback of [`case_milestone`], for the issue half too: a repository with no
    /// plan still has to be timed on this capability, and the refusal is the operation
    /// being measured there.
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let plan = Plan::build(ctx.index);
        vec![
            NamedCase::new(
                "issue",
                PlanRecordInput {
                    id: plan
                        .issues
                        .first()
                        .map_or_else(|| "I0001".to_string(), |i| i.id.clone()),
                },
            ),
            NamedCase::new(
                "milestone",
                PlanRecordInput {
                    id: case_milestone(&plan),
                },
            ),
        ]
    }
}

// ---------------------------------------------------------------- outputs

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// Issues matching a filter, with the vocabulary that names their statuses.
pub struct PlanIssueList {
    /// The matching issues, in id order.
    pub issues: Vec<PlanIssue>,
    /// How many matched.
    pub total: usize,
    /// The declared status vocabularies.
    pub statuses: PlanVocabulary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One milestone's progress, without its prose.
pub struct PlanMilestoneProgress {
    /// The identity.
    pub id: String,
    /// The derived status.
    pub status: String,
    /// One line naming the outcome.
    pub title: String,
    /// Its issues, counted by derived status.
    pub counts: PlanCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// Where the plan stands: every milestone's progress, the milestone being executed, and the
/// one issue to take next.
pub struct PlanStatusReport {
    /// The plan's header, with the active milestone derived.
    pub project: PlanProject,
    /// The declared status vocabularies.
    pub statuses: PlanVocabulary,
    /// Every milestone, in id order.
    pub milestones: Vec<PlanMilestoneProgress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The next ready issue, when there is one.
    pub next_ready: Option<PlanIssue>,
    /// Every issue of the plan, counted by derived status.
    pub counts: PlanCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The execution waves, with the overlaps that serialise issues the graph would let run
/// together.
pub struct PlanWaveReport {
    /// The waves, lowest first, with the issues of each.
    pub waves: Vec<PlanWaveView>,
    /// Scope overlaps between two issues of one wave. Two issues sharing a wave is a
    /// necessary condition for running them at once, not a sufficient one: overlapping
    /// scope serialises them, and the overlap is reported here rather than left for two
    /// workers to discover in a conflict.
    pub serialised_by_scope: Vec<PlanFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One wave, with the issues in it.
pub struct PlanWaveView {
    /// The layer, from zero.
    pub wave: u32,
    /// The issues in it, in id order.
    pub issues: Vec<PlanIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The one issue a worker should take now.
pub struct PlanNextIssue {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The issue, when the plan has one that is executable.
    pub issue: Option<PlanIssue>,
    /// The milestone the search started in.
    pub active_milestone: String,
    /// Why there is none, when there is none: what to run to see what is in the way.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The milestones in derived order, with the one being executed and the one after it.
pub struct PlanRoadmap {
    /// The milestones ordered by rank, then order, then id. The sequence is derived from
    /// the milestone graph; no list of versions is maintained anywhere.
    pub milestones: Vec<PlanMilestone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The first unfinished, unblocked milestone in that sequence.
    pub now: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The one after it, blocked or not.
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The model's own validation: what the graph refuses and what it merely warns about.
pub struct PlanValidation {
    /// Whether the model is valid: no finding is a failure.
    pub valid: bool,
    /// How many milestones the plan holds.
    pub milestones: usize,
    /// How many issues.
    pub issues: usize,
    /// How many findings are failures.
    pub failures: usize,
    /// How many are warnings.
    pub warnings: usize,
    /// Every finding, in derivation order.
    pub findings: Vec<PlanFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One record of the plan, milestone or issue, with everything derived about it.
#[serde(untagged)]
pub enum PlanRecord {
    /// A milestone.
    PlanMilestone {
        /// The milestone, with its counts, its rank and both directions of its graph.
        milestone: Box<PlanMilestone>,
        /// Its issues in full, in id order.
        issues: Vec<PlanIssue>,
    },
    /// An issue.
    PlanIssue {
        /// The issue, with its status, its wave and both directions of its graph.
        issue: Box<PlanIssue>,
        /// The issues it waits on, in full.
        depends_on: Vec<PlanIssue>,
    },
}

// ---------------------------------------------------------------- handlers

fn plan_model(ctx: &Context, _: Empty) -> Result<Plan, CapabilityError> {
    plan_of(ctx)
}

fn plan_status(
    ctx: &Context,
    input: PlanMilestoneFilter,
) -> Result<PlanStatusReport, CapabilityError> {
    let plan = plan_of(ctx)?;
    let only = input.milestone.as_deref();
    if let Some(m) = only {
        if plan.milestone(m).is_none() {
            return Err(CapabilityError::NotFound(format!("no milestone '{m}'")));
        }
    }
    let mut counts = PlanCounts {
        total: 0,
        required: 0,
        by_status: plan.statuses.issue.iter().map(|s| (s.clone(), 0)).collect(),
    };
    for i in &plan.issues {
        if only.is_some_and(|m| i.milestone != m) {
            continue;
        }
        counts.total += 1;
        *counts.by_status.entry(i.status.clone()).or_insert(0) += 1;
    }
    counts.required = counts.total - counts.by_status.get("CANCELLED").copied().unwrap_or(0);
    Ok(PlanStatusReport {
        milestones: plan
            .milestones
            .iter()
            .filter(|m| only.is_none_or(|only| m.id == only))
            .map(|m| PlanMilestoneProgress {
                id: m.id.clone(),
                status: m.status.clone(),
                title: m.title.clone(),
                counts: m.counts.clone(),
            })
            .collect(),
        next_ready: plan.next_ready(only).cloned(),
        project: plan.project.clone(),
        statuses: plan.statuses.clone(),
        counts,
    })
}

fn plan_issues(ctx: &Context, input: PlanIssueFilter) -> Result<PlanIssueList, CapabilityError> {
    let plan = plan_of(ctx)?;
    if let Some(s) = input.status.as_deref() {
        if !plan.statuses.issue.iter().any(|v| v == s) {
            return Err(CapabilityError::InvalidInput(format!(
                "'{s}' is not an issue status; the vocabulary is {}",
                plan.statuses.issue.join(", ")
            )));
        }
    }
    let issues: Vec<PlanIssue> = plan
        .issues
        .iter()
        .filter(|i| input.milestone.as_deref().is_none_or(|m| i.milestone == m))
        .filter(|i| input.status.as_deref().is_none_or(|s| i.status == s))
        .filter(|i| input.wave.is_none_or(|w| i.wave == w))
        .cloned()
        .collect();
    Ok(PlanIssueList {
        total: issues.len(),
        issues,
        statuses: plan.statuses,
    })
}

fn plan_waves(
    ctx: &Context,
    input: PlanMilestoneFilter,
) -> Result<PlanWaveReport, CapabilityError> {
    let plan = plan_of(ctx)?;
    let only = input.milestone.as_deref();
    let view = |w: &PlanWave| PlanWaveView {
        wave: w.wave,
        issues: plan
            .issues
            .iter()
            .filter(|i| w.issues.contains(&i.id))
            .filter(|i| only.is_none_or(|m| i.milestone == m))
            .cloned()
            .collect(),
    };
    Ok(PlanWaveReport {
        waves: plan
            .waves
            .iter()
            .map(view)
            .filter(|w| !w.issues.is_empty())
            .collect(),
        serialised_by_scope: plan
            .findings
            .iter()
            .filter(|f| f.code == "scope_conflict")
            .cloned()
            .collect(),
    })
}

fn plan_next(ctx: &Context, input: PlanMilestoneFilter) -> Result<PlanNextIssue, CapabilityError> {
    let plan = plan_of(ctx)?;
    let issue = plan.next_ready(input.milestone.as_deref()).cloned();
    Ok(PlanNextIssue {
        reason: issue.is_none().then(|| {
            "no issue is READY; every one of them waits on a dependency or on its milestone's gate"
                .to_string()
        }),
        active_milestone: input
            .milestone
            .unwrap_or_else(|| plan.project.active_milestone.clone()),
        issue,
    })
}

fn plan_roadmap(ctx: &Context, _: Empty) -> Result<PlanRoadmap, CapabilityError> {
    let plan = plan_of(ctx)?;
    let ordered: Vec<PlanMilestone> = plan.roadmap().into_iter().cloned().collect();
    let mut open = ordered
        .iter()
        .filter(|m| !matches!(m.status.as_str(), "DONE" | "CANCELLED" | "SUPERSEDED"));
    let now = open
        .next()
        .filter(|m| m.blocked_by.is_empty())
        .map(|m| m.id.clone());
    let next = if now.is_some() {
        open.next().map(|m| m.id.clone())
    } else {
        // The first open milestone is itself blocked: it is not "now", it is "next".
        ordered
            .iter()
            .find(|m| !matches!(m.status.as_str(), "DONE" | "CANCELLED" | "SUPERSEDED"))
            .map(|m| m.id.clone())
    };
    Ok(PlanRoadmap {
        milestones: ordered,
        now,
        next,
    })
}

fn plan_validate(ctx: &Context, _: Empty) -> Result<PlanValidation, CapabilityError> {
    let plan = plan_of(ctx)?;
    Ok(PlanValidation {
        valid: plan.failures() == 0,
        milestones: plan.milestones.len(),
        issues: plan.issues.len(),
        failures: plan.failures(),
        warnings: plan.warnings(),
        findings: plan.findings,
    })
}

fn plan_record(ctx: &Context, input: PlanRecordInput) -> Result<PlanRecord, CapabilityError> {
    let plan = plan_of(ctx)?;
    if let Some(m) = plan.milestone(&input.id) {
        let issues = plan
            .issues
            .iter()
            .filter(|i| i.milestone == m.id)
            .cloned()
            .collect();
        return Ok(PlanRecord::PlanMilestone {
            milestone: Box::new(m.clone()),
            issues,
        });
    }
    if let Some(i) = plan.issue(&input.id) {
        let depends_on = i
            .depends_on
            .iter()
            .filter_map(|d| plan.issue(d))
            .cloned()
            .collect();
        return Ok(PlanRecord::PlanIssue {
            issue: Box::new(i.clone()),
            depends_on,
        });
    }
    Err(CapabilityError::NotFound(format!(
        "no milestone or issue '{}'",
        input.id
    )))
}

// ---------------------------------------------------------------- the module

/// The `plan` module: the derivations of the milestone and issue model, projected once.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "plan",
        title: "The plan and its derivations",
        description: "The milestone and issue model of this repository, and everything derived from it that nobody authored: the status of each record, the dependency graphs above and below the milestone boundary, the topological execution waves, the roadmap order, the milestone being executed and the one issue to take next. Status is never stored — a record says what happened to it and the status follows from that and from the state of its dependencies — so no file can contradict the graph. The four operations that write a lifecycle marker into a record stay on the command line: a capability of this registry never writes to the repository.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "plan.model",
                title: "The whole derived plan",
                description: "Every milestone and issue with its derived status, wave, rank, both directions of its graph and its counts; the execution waves; both dependency graphs as edges; every validation finding; and the plan's header with the active milestone derived. The one value every other capability of this module answers out of. Derived on every call: a transition writes a lifecycle marker into a record between two calls, and a plan answered from a snapshot would send two workers to one issue.",
                input: Empty,
                output: Plan,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_plan".into()),
                        resource: Some(McpResource { uri: PLAN_URI.into(), name: "plan".into() }),
                    }),
                    http: get("/api/v1/plan"),
                    cli: None,
                },
                tags: ["plan", "project", "graph", "introspection"],
                cache: CachePolicy::Disabled,
                handler: plan_model,
            },
            capability! {
                id: "plan.status",
                title: "Where the plan stands",
                description: "Every milestone with its derived status and its issues counted by status, the milestone a worker is executing now, the next ready issue in full, and the plan's own totals. The counts are keyed by the declared vocabulary, which travels with the answer, so a status added to the engine appears here without anything being edited.",
                input: PlanMilestoneFilter,
                output: PlanStatusReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_plan_status"),
                    http: get("/api/v1/plan/status"),
                    cli: None,
                },
                tags: ["plan", "project", "status"],
                cache: CachePolicy::Disabled,
                handler: plan_status,
            },
            capability! {
                id: "plan.issues",
                title: "The issues, filtered by what the graph derived",
                description: "One record per issue with its derived status, its wave, the dependencies it declares, the ones that are not DONE (plus `milestone:<id>` when the gate holds the whole outcome back), the issues that depend on it, the paths it touches and its evidence tally. Filtering by `status: READY` is the ready set and by `status: BLOCKED` the blocked set; nothing here is a separate derivation.",
                input: PlanIssueFilter,
                output: PlanIssueList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_plan_issues"),
                    http: get("/api/v1/plan/issues"),
                    cli: None,
                },
                tags: ["plan", "project", "issues"],
                cache: CachePolicy::Disabled,
                handler: plan_issues,
            },
            capability! {
                id: "plan.waves",
                title: "What may run at the same time",
                description: "The topological layering of the issue graph: an issue enters a wave only once every dependency has left it, so its wave is one past the longest path to it. Sharing a wave is a necessary condition for running two issues at once, not a sufficient one — overlapping scope serialises them, and every such overlap is reported beside the waves rather than left for two workers to discover in a merge conflict.",
                input: PlanMilestoneFilter,
                output: PlanWaveReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_plan_waves"),
                    http: get("/api/v1/plan/waves"),
                    cli: None,
                },
                tags: ["plan", "project", "graph", "waves"],
                cache: CachePolicy::Disabled,
                handler: plan_waves,
            },
            capability! {
                id: "plan.next",
                title: "The one issue to take now",
                description: "The lowest-wave READY issue of the active milestone, highest priority first, then id. The active milestone can have nothing ready while another one does — one waiting on its own acceptance evidence, for instance — so the search widens to the whole plan rather than answering `none` and sending a worker away from work that is genuinely executable. This is what an agent asks before it starts.",
                input: PlanMilestoneFilter,
                output: PlanNextIssue,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_plan_next"),
                    http: get("/api/v1/plan/next"),
                    cli: None,
                },
                tags: ["plan", "project", "next"],
                cache: CachePolicy::Disabled,
                handler: plan_next,
            },
            capability! {
                id: "plan.roadmap",
                title: "The milestones in derived order",
                description: "The milestone graph laid out by rank, with `order` breaking ties inside a rank only, and the first unblocked unfinished milestone as `now` and the one after it as `next`. Nothing in the sequence is authored: a milestone whose prerequisites are not real cannot be nominated, which is what makes `each step is gated by the previous one being real` an invariant rather than a sentence.",
                input: Empty,
                output: PlanRoadmap,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_plan_roadmap"),
                    http: get("/api/v1/plan/roadmap"),
                    cli: None,
                },
                tags: ["plan", "project", "roadmap"],
                cache: CachePolicy::Disabled,
                handler: plan_roadmap,
            },
            capability! {
                id: "plan.validate",
                title: "What the model refuses",
                description: "Every finding the derivation produced, in the order it produced them: a dependency on something that is not an issue, a cycle, an issue executing ahead of its dependencies or of its milestone's gate, an issue with no acceptance criteria, evidence missing under a completion date, a milestone whose graph contradicts itself, two issues of one wave sharing a path. A failure means the model is invalid; a warning means it is legal and worth reading.",
                input: Empty,
                output: PlanValidation,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_plan_validate"),
                    http: get("/api/v1/plan/validate"),
                    cli: None,
                },
                tags: ["plan", "project", "validation"],
                cache: CachePolicy::Disabled,
                handler: plan_validate,
            },
            capability! {
                id: "plan.record",
                title: "One milestone or issue, with everything derived about it",
                description: "A milestone with its issues in full, or an issue with the issues it waits on in full. The record's own prose stays where it has always been — `majordomus://issue/<id>` returns the file — and this answers what the file cannot say about itself: what its status is, where it sits in the graph, and what is between it and being executable.",
                input: PlanRecordInput,
                output: PlanRecord,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_plan_record"),
                    http: get("/api/v1/plan/record"),
                    cli: None,
                },
                tags: ["plan", "project", "issues", "milestones"],
                cache: CachePolicy::Disabled,
                handler: plan_record,
            },
            capability! {
                id: "plan.transition",
                kind: CapabilityKind::Command,
                title: "Move one issue through the lifecycle",
                description: "Record that execution of an issue began (`start`), that implementation is complete with evidence outstanding (`verify`), or that it is finished (`done`). One field of the issue's own record is stamped and one event is appended to the ledger. The move is refused when the model says it is illegal — an issue that is not READY cannot start, one that was never ACTIVE cannot be verified, and one whose dependencies are unfinished or whose required evidence is absent cannot be done — and the refusal names what is in the way. The status that comes back is derived from the record afterwards, not announced by the move: a `done` whose evidence is missing leaves the issue in VERIFY and says so.",
                input: PlanTransitionInput,
                output: PlanTransitionResult,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_plan_transition"),
                    http: post("/api/v1/plan/transition"),
                    cli: None,
                },
                tags: ["plan", "project", "issues", "lifecycle"],
                handler: plan_transition,
            },
        ],
    }
}

// ---------------------------------------------------------------- plan.transition

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `plan.transition`.
pub struct PlanTransitionInput {
    /// The issue to move, by its id — which is also its file name.
    pub issue: String,
    /// Which move: `start`, `verify` or `done`.
    pub transition: Transition,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// What one transition did.
///
/// `from` and `to` are both derived statuses, read before and after the write. They are
/// reported rather than assumed because the move does not determine the status on its own:
/// `done` on a record whose evidence is incomplete leaves it in VERIFY, and the caller is
/// told that rather than told it succeeded.
pub struct PlanTransitionResult {
    /// The issue that moved.
    pub issue: String,
    /// The move that was performed.
    pub transition: Transition,
    /// The status the issue was in before.
    pub from: String,
    /// The status it is in now, derived from the record as written.
    pub to: String,
    /// The field that was stamped.
    pub field: String,
    /// The timestamp written into it, and into `updated_at`.
    pub at: String,
    /// The ledger event appended.
    pub event: String,
    /// The issue a worker should take next, derived after the move. Absent when the plan
    /// has none — which after a `start` is the ordinary case, because the issue just taken
    /// is no longer READY.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_ready: Option<String>,
}

/// The benchmark cases of the one capability here that writes.
///
/// Both are refusals, and that is deliberate rather than a gap. A benchmark case is
/// executed — `executions.start`'s case really starts an execution — so a case that moved an
/// issue would mutate this repository's plan every time the suite ran, and a suite whose
/// cost is a changed record is not a measurement. The runner states the principle that makes
/// this sound: *"a refusal is an answer, and answering is the work a benchmark exists to
/// measure"*, and only `Internal` is fatal. The refusal path is also the honest thing to
/// time: it builds the plan, resolves the issue and evaluates the guard, which is everything
/// the write path does except the two syscalls at the end.
impl BenchmarkCases for PlanTransitionInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let plan = Plan::build(ctx.index);
        let mut cases = vec![NamedCase::new(
            "refused-unknown-issue",
            PlanTransitionInput {
                // An id no record can carry: issue ids are `I` and four digits, and the
                // loader refuses a record whose id disagrees with its file name, so this
                // cannot come to exist and the case cannot start writing later.
                issue: "no-such-issue".into(),
                transition: Transition::Start,
            },
        )];
        // A real issue that is not READY, when the plan holds one: the guard then runs
        // against a record rather than against a lookup miss. Absent such an issue the
        // repository's plan is entirely startable, and the case above is the whole set.
        if let Some(i) = plan.issues.iter().find(|i| i.status != "READY") {
            cases.push(NamedCase::new(
                "refused-not-ready",
                PlanTransitionInput {
                    issue: i.id.clone(),
                    transition: Transition::Start,
                },
            ));
        }
        cases
    }
}

fn plan_transition(
    ctx: &Context,
    input: PlanTransitionInput,
) -> Result<PlanTransitionResult, CapabilityError> {
    let plan = Plan::build(&ctx.index);
    let before = plan
        .issue(&input.issue)
        .ok_or_else(|| {
            CapabilityError::NotFound(format!(
                "no issue '{}'; plan.issues lists every one",
                input.issue
            ))
        })?
        .status
        .clone();

    // The record's own file, as the index recorded it. Located here rather than composed
    // from the id so that there is one answer to where a record lives.
    let rel = ctx
        .index
        .objects
        .iter()
        .find(|o| o.kind == crate::plan::ISSUE && o.identity == input.issue)
        .map(|o| o.provenance.path.clone())
        .ok_or_else(|| {
            CapabilityError::NotFound(format!(
                "issue '{}' is in the plan but no object carries its file",
                input.issue
            ))
        })?;

    let root = Path::new(&ctx.index.repository.root);
    let events = ctx
        .index
        .share
        .as_ref()
        .map(|s| s.join("events.yaml"))
        .ok_or_else(|| {
            CapabilityError::Refused(
                "this index was built without a share directory, so the event vocabulary cannot be read; a transition may not write an event it cannot validate".into(),
            )
        })?;
    let vocabulary = crate::ledger::Vocabulary::load(&events)
        .map_err(|e| CapabilityError::Refused(e.to_string()))?;

    let at = crate::ledger::now();
    crate::plan::transition(
        root,
        &root.join(&rel),
        &ctx.index,
        &plan,
        &input.issue,
        input.transition,
        &at,
        &vocabulary,
        &ctx.index.repository.git,
    )
    .map_err(|e| match e {
        TransitionError::NoSuchIssue(_) => CapabilityError::NotFound(e.to_string()),
        TransitionError::Refused(_) => CapabilityError::Refused(e.to_string()),
        TransitionError::Write { .. } => CapabilityError::Refused(e.to_string()),
    })?;

    // What the plan says now, through the same derivation every reader uses.
    let after = Plan::after(&ctx.index, &input.issue, input.transition, &at);
    Ok(PlanTransitionResult {
        to: after
            .issue(&input.issue)
            .map(|i| i.status.clone())
            .unwrap_or_default(),
        next_ready: after.next_ready(None).map(|i| i.id.clone()),
        issue: input.issue,
        transition: input.transition,
        from: before,
        field: input.transition.field().to_string(),
        at,
        event: input.transition.event().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "plan");
        let expected: &[(&str, &str, &str)] = &[
            ("plan.model", "majordomus_plan", "/api/v1/plan"),
            (
                "plan.status",
                "majordomus_plan_status",
                "/api/v1/plan/status",
            ),
            (
                "plan.issues",
                "majordomus_plan_issues",
                "/api/v1/plan/issues",
            ),
            ("plan.waves", "majordomus_plan_waves", "/api/v1/plan/waves"),
            ("plan.next", "majordomus_plan_next", "/api/v1/plan/next"),
            (
                "plan.roadmap",
                "majordomus_plan_roadmap",
                "/api/v1/plan/roadmap",
            ),
            (
                "plan.validate",
                "majordomus_plan_validate",
                "/api/v1/plan/validate",
            ),
            (
                "plan.record",
                "majordomus_plan_record",
                "/api/v1/plan/record",
            ),
            (
                "plan.transition",
                "majordomus_plan_transition",
                "/api/v1/plan/transition",
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
            assert!(
                !executable.capability.cache.is_enabled(),
                "{id} must not cache: a transition changes the plan between two calls"
            );
        }
        let resource = m.capabilities[0]
            .capability
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref());
        assert_eq!(resource.map(|r| r.uri.as_str()), Some(PLAN_URI));
    }

    /// Exactly one capability of the module writes, and it is named here.
    ///
    /// This assertion used to read "every capability is a query", which was the registry's
    /// contract until [ADR 0040] made development semantics capabilities of this runtime. It
    /// is kept rather than deleted, and inverted rather than loosened: a module that reads
    /// the plan and one that changes it are different things to depend on, and the next
    /// capability to gain a write should have to say so here.
    ///
    /// [ADR 0040]: ../../../../../.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md
    #[test]
    fn only_the_transition_writes() {
        let m = module();
        let writers: Vec<&str> = m
            .capabilities
            .iter()
            .filter(|e| !e.capability.kind.is_read_only())
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(writers, ["plan.transition"]);
    }

    /// A transition is an HTTP POST, never a GET.
    ///
    /// Stated separately because the projection is what a caller meets: a mutating
    /// capability reachable by GET would be followed by every crawler, prefetcher and
    /// link-checker that ever met the Cockpit.
    #[test]
    fn the_transition_is_not_reachable_by_get() {
        let e = module()
            .capabilities
            .into_iter()
            .find(|e| e.capability.id.as_str() == "plan.transition")
            .expect("the module declares plan.transition");
        let http = e.capability.exposure.http.expect("it is exposed over HTTP");
        assert_eq!(http.method.as_str(), "POST");
    }
}
