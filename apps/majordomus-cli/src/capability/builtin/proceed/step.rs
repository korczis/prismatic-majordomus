//! `proceed.step`: one step of a session that proceeds, answered as one work order (I4005).
//!
//! # What it composes, and what it adds
//!
//! Everything a step needs has an owner already. Which issue comes next, and which of them
//! another worker holds, is `proceed.plan`'s decision (the plan's hand-out order read against
//! every board and every linked machine's claims). Whether the intent a session serves is met,
//! and which issues serve what it still lacks, is `intent_realization.work`'s. Which model a
//! worker runs on is `models.route`'s, and how an issue is taken, claimed, given a worktree and
//! run once is `devtask.implement`'s. This capability asks each of them once and adds only
//! the shape: a work order a loop can act on without reading any of them itself.
//!
//! * `do` — the one capability to run next and its input: `devtask.implement` for the issue,
//!   under the idempotency key `devtask.implement` itself defaults to, so a step repeated
//!   after its work started is answered by the execution it started and creates nothing;
//! * `fan_out` — the other READY issues that may run beside it at once: declared
//!   `parallel_safe`, refused by nothing, sharing no path with what is already ordered, and at
//!   most four of them, so a machine runs at most five builders;
//! * `report` — what to announce on the board, as `peers.announce` takes it;
//! * `next_due_at` — when to step again: the next boundary of a five-minute cadence, so that a
//!   loop reports at that interval and two steps inside one interval answer the same order.
//!
//! # The same state answers the same order
//!
//! The order is a function of what it reads and of the moment it is asked at (`at`, the clock
//! when absent). Its `key` is the digest of the order itself without `next_due_at`, so a step
//! taken again over unchanged state carries the same key at any later moment, and a change of
//! any fact the order rests on changes it. Nothing is stored: there is no queue of orders and
//! no second claim ledger.
//!
//! # When it stops
//!
//! A step for an intent whose every criterion has current evidence, or whose stage is
//! satisfied, cancelled or superseded, is `done`: no `do`, no `fan_out` and no `next_due_at`,
//! so a loop that follows it ends. A step for the whole plan is `done` when the plan holds no
//! unfinished issue.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::builtin::devtask::{default_key, ImplementInput};
use crate::capability::handler::{CapabilityError, Context};
use crate::intent::IntentStage;
use crate::intent_realization::{IntentRealization, IntentRealizationView};
use crate::models::RoutingDecision;
use crate::plan::Plan;

use super::{proceed_plan, ProceedCandidate, ProceedDecision, ProceedPlanInput, ProceedSource};

/// How often a session that proceeds steps again while work is under way or waits: the
/// interval the owner asked every peer to report at.
pub(crate) const CADENCE_SECONDS: u64 = 300;
/// The most issues a step fans out beside the one it orders: with it, five builders, the
/// most one machine runs.
pub(crate) const MAX_FAN_OUT: usize = 4;
/// The capability a step orders for an issue.
pub(crate) const IMPLEMENT: &str = "devtask.implement";
/// What a worker must be able to do, as `models.route` words it: a worker calls tools.
pub(crate) const WORKER_NEEDS: &str = "tools";
/// The name of the board claim a step's report is announced under.
pub(crate) const REPORT_CLAIM: &str = "proceed";

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What to ask of `proceed.step`.
pub struct ProceedStepInput {
    /// The intent the session realises, by id. The step is `done` when its realisation is met, and only issues serving its unmet criteria are ordered. Empty: the whole plan, done when no unfinished issue is left.
    #[serde(default)]
    pub intent: String,
    /// The milestone whose READY issues come first, as for `proceed.plan`; the active one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The mesh session the step is for, whose own claims are not held against it, as for `proceed.plan`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The moment the step is taken at, RFC 3339 in UTC to the second (`2026-10-23T10:00:00Z`); the clock when absent. Only `next_due_at` depends on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Compose in the process that answers and never ask the checkout's leaseholder, as for `proceed.plan`.
    #[serde(default)]
    pub local: bool,
}

impl BenchmarkCases for ProceedStepInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let at = Some("2026-10-23T10:00:00Z".to_string());
        let mut cases = vec![NamedCase::new(
            "whole_plan",
            Self {
                at: at.clone(),
                local: true,
                ..Self::default()
            },
        )];
        if let Some(intent) = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == crate::intent::INTENT)
        {
            cases.push(NamedCase::new(
                "one_intent",
                Self {
                    intent: intent.identity.clone(),
                    at,
                    local: true,
                    ..Self::default()
                },
            ));
        }
        cases
    }
}

// ---------------------------------------------------------------- output

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One step of a session that proceeds: what to run now, what may run beside it, what to tell the board and when to step again.
pub struct WorkOrder {
    /// The order's identity: the digest of every field but `next_due_at`, `at` and `sources`. The same state answers the same key at any moment.
    pub key: String,
    /// What the step decided.
    pub outcome: OrderOutcome,
    /// Whether there is nothing left to do: the intent's realisation is met (or it was cancelled or superseded), or, for the whole plan, no unfinished issue is left. A done order runs and schedules nothing.
    pub done: bool,
    /// One sentence a reader can check against the decision and the intent.
    pub reason: String,
    /// The capability to run next and its input; absent when the step waits or is done.
    #[serde(rename = "do")]
    pub work: Option<WorkItem>,
    /// Independent items that may run in parallel with `do`, each a worker of its own; empty when nothing may.
    pub fan_out: Vec<WorkItem>,
    /// What to announce on the board, as `peers.announce` takes it.
    pub report: StepReport,
    /// When to step again, RFC 3339 in UTC: the next boundary of the cadence after `at`. Absent when the order is done.
    pub next_due_at: Option<String>,
    /// The intent the step was taken for, as its realisation stands; absent for the whole plan.
    pub intent: Option<StepIntent>,
    /// The moment the step was taken at.
    pub at: String,
    /// What the decision was read from, as `proceed.plan` says it.
    pub sources: Vec<ProceedSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// What one step decided.
pub enum OrderOutcome {
    /// An issue is ordered: run `do`, and `fan_out` beside it.
    Implement,
    /// Nothing may be taken now — every candidate is held, waits on something, or the decision is provisional — so step again at `next_due_at`.
    Wait,
    /// Nothing is left to do; schedule nothing.
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One unit of ordered work: a capability, its input, and what a worker needs to run it.
pub struct WorkItem {
    /// The capability to run: `devtask.implement`.
    pub capability: String,
    /// Its input, exactly as the capability takes it: the issue, and the idempotency key that makes a repeated order one execution.
    pub input: Value,
    /// The issue it realises.
    pub issue: String,
    /// The issue's title.
    pub title: String,
    /// The paths it touches.
    pub scope: Vec<String>,
    /// The execution profile the issue declares.
    pub profile: String,
    /// The effort that profile declares, when it declares one.
    pub effort: Option<String>,
    /// The model `models.route` selects for a worker, when one qualifies.
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What a step tells the board: the input of `peers.announce`.
pub struct StepReport {
    /// The claim the announcement stands under.
    pub claim: String,
    /// One line: what is ordered or why nothing is.
    pub intent: String,
    /// The paths of every ordered item.
    pub scope: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The intent a step serves, as `intent_realization.work` answers it.
pub struct StepIntent {
    /// The intent's id.
    pub intent: String,
    /// Its derived stage.
    pub stage: IntentStage,
    /// How many criteria it declares.
    pub criteria: usize,
    /// How many have current evidence.
    pub met: usize,
    /// The criteria without current evidence, by id.
    pub unmet: Vec<String>,
    /// The issues declaring they serve an unmet criterion, in plan order: the only ones the step orders.
    pub serving: Vec<String>,
}

// ---------------------------------------------------------------- the step

/// Everything a step reads, already read: the decision is `proceed.plan`'s, the intent's
/// realisation `intent_realization.work`'s, the model `models.route`'s.
pub(crate) struct StepFacts<'a> {
    pub(crate) plan: &'a Plan,
    pub(crate) decision: &'a ProceedDecision,
    pub(crate) intent: Option<&'a IntentRealizationView>,
    /// The effort each execution profile declares, by profile name.
    pub(crate) efforts: &'a BTreeMap<String, String>,
    pub(crate) model: Option<&'a str>,
    /// Seconds since the epoch.
    pub(crate) at: u64,
}

/// The fields a work order is identified by: everything but when it was asked and when it is
/// next due.
#[derive(Serialize)]
struct Identity<'a> {
    outcome: OrderOutcome,
    done: bool,
    reason: &'a str,
    work: &'a Option<WorkItem>,
    fan_out: &'a [WorkItem],
    report: &'a StepReport,
    intent: &'a Option<StepIntent>,
}

/// The step over what was read. Pure: the same facts answer the same order.
pub(crate) fn step(f: &StepFacts<'_>) -> WorkOrder {
    let intent = f.intent.map(summary);
    let (outcome, reason, work, fan_out) = decide(f, intent.as_ref());
    let done = outcome == OrderOutcome::Done;
    let mut scope: Vec<String> = Vec::new();
    for item in work.iter().chain(fan_out.iter()) {
        for p in &item.scope {
            if !scope.contains(p) {
                scope.push(p.clone());
            }
        }
    }
    let line = match (&work, fan_out.len()) {
        (Some(w), 0) => format!("{}: {} ({})", w.issue, w.title, reason),
        (Some(w), n) => format!("{}: {}, and {n} beside it ({})", w.issue, w.title, reason),
        (None, _) => reason.clone(),
    };
    let report = StepReport {
        claim: REPORT_CLAIM.into(),
        intent: line,
        scope,
    };
    let key = key_of(&Identity {
        outcome,
        done,
        reason: &reason,
        work: &work,
        fan_out: &fan_out,
        report: &report,
        intent: &intent,
    });
    WorkOrder {
        key,
        outcome,
        done,
        reason,
        work,
        fan_out,
        report,
        next_due_at: (!done).then(|| stamp(next_due(f.at, CADENCE_SECONDS))),
        intent,
        at: stamp(f.at),
        sources: f.decision.sources.clone(),
    }
}

fn decide(
    f: &StepFacts<'_>,
    intent: Option<&StepIntent>,
) -> (OrderOutcome, String, Option<WorkItem>, Vec<WorkItem>) {
    if let Some(i) = intent {
        if let Some(why) = finished(i) {
            return (OrderOutcome::Done, why, None, Vec::new());
        }
    }
    let d = f.decision;
    let serves = |c: &ProceedCandidate| intent.is_none_or(|i| i.serving.contains(&c.issue));
    let free: Vec<&ProceedCandidate> = d
        .candidates
        .iter()
        .filter(|c| c.refused.is_empty() && serves(c))
        .collect();
    let Some(first) = free.first() else {
        return waiting(f, intent);
    };
    if d.provisional {
        return (
            OrderOutcome::Wait,
            format!(
                "{} would be ordered, but {}",
                first.issue,
                provisional_words(&d.reason)
            ),
            None,
            Vec::new(),
        );
    }
    let reason = match intent {
        None => d.reason.clone(),
        Some(i) => format!(
            "{} is the first READY issue serving an unmet criterion of {} that nothing refuses ({} of {} criteria met)",
            first.issue, i.intent, i.met, i.criteria
        ),
    };
    let fan_out = fan_out(f, first, &free[1..]);
    (
        OrderOutcome::Implement,
        reason,
        Some(item(f, first)),
        fan_out,
    )
}

/// Why an intent needs no further step, or `None` while it does.
fn finished(i: &StepIntent) -> Option<String> {
    match i.stage {
        IntentStage::Cancelled | IntentStage::Superseded => Some(format!(
            "the intent {} is {}: nothing realises it any more",
            i.intent,
            i.stage.as_str()
        )),
        IntentStage::Satisfied => Some(format!(
            "the intent {} is satisfied: {} of {} criteria have current evidence",
            i.intent, i.met, i.criteria
        )),
        _ if i.criteria > 0 && i.unmet.is_empty() => Some(format!(
            "the intent {} is met: all {} criteria have current evidence",
            i.intent, i.criteria
        )),
        _ => None,
    }
}

/// The order when nothing may be taken: done when nothing is left, otherwise a wait that
/// says on what.
fn waiting(
    f: &StepFacts<'_>,
    intent: Option<&StepIntent>,
) -> (OrderOutcome, String, Option<WorkItem>, Vec<WorkItem>) {
    let d = f.decision;
    let unfinished = !d.candidates.is_empty() || !d.waiting.is_empty();
    match intent {
        None if !unfinished => (OrderOutcome::Done, d.reason.clone(), None, Vec::new()),
        None => (OrderOutcome::Wait, d.reason.clone(), None, Vec::new()),
        Some(i) if i.serving.is_empty() => (
            OrderOutcome::Wait,
            format!(
                "no issue of the plan declares it serves an unmet criterion of {} ({}): plan one",
                i.intent,
                i.unmet.join(", ")
            ),
            None,
            Vec::new(),
        ),
        Some(i) => (
            OrderOutcome::Wait,
            format!(
                "none of the issues serving {} ({}) is READY and free: each waits on a dependency, its milestone's gate or another worker",
                i.intent,
                i.serving.join(", ")
            ),
            None,
            Vec::new(),
        ),
    }
}

fn provisional_words(reason: &str) -> String {
    match reason.split_once("; provisional, ") {
        Some((_, rest)) => format!("the decision is provisional, {rest}"),
        None => "the decision is provisional".to_string(),
    }
}

/// The free issues that may run beside `first`: declared parallel safe, as `first` must be
/// too, and sharing no path with anything already ordered.
fn fan_out(
    f: &StepFacts<'_>,
    first: &ProceedCandidate,
    rest: &[&ProceedCandidate],
) -> Vec<WorkItem> {
    if !parallel_safe(f.plan, &first.issue) {
        return Vec::new();
    }
    let mut taken: Vec<&ProceedCandidate> = vec![first];
    for c in rest {
        if taken.len() > MAX_FAN_OUT {
            break;
        }
        let apart = taken
            .iter()
            .all(|t| !crate::mesh::state::scopes_meet(&t.scope, &c.scope));
        if apart && parallel_safe(f.plan, &c.issue) {
            taken.push(c);
        }
    }
    taken[1..].iter().map(|c| item(f, c)).collect()
}

fn parallel_safe(plan: &Plan, issue: &str) -> bool {
    plan.issues.iter().any(|i| i.id == issue && i.parallel_safe)
}

fn item(f: &StepFacts<'_>, c: &ProceedCandidate) -> WorkItem {
    let profile = f
        .plan
        .issues
        .iter()
        .find(|i| i.id == c.issue)
        .map(|i| i.profile.clone())
        .unwrap_or_default();
    let input = ImplementInput {
        issue: c.issue.clone(),
        idempotency_key: Some(default_key(&c.issue)),
        deadline_minutes: None,
    };
    WorkItem {
        capability: IMPLEMENT.into(),
        input: serde_json::to_value(input).unwrap_or(Value::Null),
        issue: c.issue.clone(),
        title: c.title.clone(),
        scope: c.scope.clone(),
        effort: f.efforts.get(&profile).cloned(),
        profile,
        model: f.model.map(str::to_string),
    }
}

fn summary(v: &IntentRealizationView) -> StepIntent {
    let mut serving: Vec<String> = Vec::new();
    for u in &v.unmet {
        for i in &u.issues {
            if !serving.contains(i) {
                serving.push(i.clone());
            }
        }
    }
    StepIntent {
        intent: v.intent.clone(),
        stage: v.stage,
        criteria: v.criteria,
        met: v.met,
        unmet: v.unmet.iter().map(|u| u.id.clone()).collect(),
        serving,
    }
}

/// The next boundary of `cadence` strictly after `at`: every moment inside one interval
/// answers the same boundary.
fn next_due(at: u64, cadence: u64) -> u64 {
    (at / cadence + 1) * cadence
}

fn stamp(secs: u64) -> String {
    crate::peers::rfc3339(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

fn key_of(identity: &Identity<'_>) -> String {
    let text = serde_json::to_string(identity).unwrap_or_default();
    format!("step-{}", &crate::policy::sha256_hex(&text)[..16])
}

// ---------------------------------------------------------------- the handler

pub(crate) fn proceed_step(
    ctx: &Context,
    input: ProceedStepInput,
) -> Result<WorkOrder, CapabilityError> {
    let at = match input.at.as_deref() {
        Some(s) => crate::peers::parse_rfc3339(s).ok_or_else(|| {
            CapabilityError::InvalidInput(format!(
                "at '{s}' is not RFC 3339 in UTC to the second, such as 2026-10-23T10:00:00Z"
            ))
        })?,
        None => crate::peers::parse_rfc3339(&crate::ledger::now()).unwrap_or(0),
    };
    let intent_id = input.intent.trim().to_string();
    let realization = if intent_id.is_empty() {
        None
    } else {
        let v = ctx.execute("intent_realization.work", json!({ "intent": intent_id }))?;
        let r: IntentRealization = serde_json::from_value(v)
            .map_err(|e| CapabilityError::Internal(format!("intent_realization.work: {e}")))?;
        Some(r)
    };
    let view = match &realization {
        Some(r) => Some(
            r.intents
                .iter()
                .find(|v| v.intent == intent_id)
                .ok_or_else(|| {
                    CapabilityError::NotFound(format!(
                        "intent_realization.work answered no intent '{intent_id}'"
                    ))
                })?,
        ),
        None => None,
    };
    let decision = proceed_plan(
        ctx,
        ProceedPlanInput {
            milestone: input.milestone.clone(),
            session: input.session.clone(),
            local: input.local,
        },
    )?;
    // a catalogue that declares no model, or none a worker can use, leaves the choice to the
    // worker: the order says so by naming none
    let route: Option<RoutingDecision> = ctx
        .execute("models.route", json!({ "require": WORKER_NEEDS }))
        .ok()
        .and_then(|v| serde_json::from_value(v).ok());
    Ok(step(&StepFacts {
        plan: &Plan::build(&ctx.index),
        decision: &decision,
        intent: view,
        efforts: &efforts(ctx),
        model: route.as_ref().and_then(|r| r.selected.as_deref()),
        at,
    }))
}

/// The effort each execution profile of the layer declares, by its name.
fn efforts(ctx: &Context) -> BTreeMap<String, String> {
    ctx.index
        .objects
        .iter()
        .filter(|o| o.kind == "profile")
        .filter_map(|o| {
            let name = o.metadata.get("name").and_then(Value::as_str);
            let effort = o.metadata.get("effort").and_then(Value::as_str)?;
            Some((
                name.unwrap_or(o.identity.as_str()).to_string(),
                effort.to_string(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{ProceedConstraint, ProceedEvidence, ProceedRefusal, ProceedTerms};
    use super::*;
    use crate::intent_realization::IntentUnmetCriterion;

    fn plan(issues: &[(&str, bool, &str)]) -> Plan {
        let issues: Vec<Value> = issues
            .iter()
            .map(|(id, safe, profile)| {
                json!({"id": id, "milestone": "M", "status": "READY", "wave": 0,
                       "priority": "p1", "profile": profile, "parallel_safe": safe,
                       "title": format!("t {id}"), "slug": "s", "depends_on": [],
                       "blocked_by": [], "dependents": [], "scope": [], "serves": [],
                       "objective": "o", "evidence_have": 0, "evidence_need": 1,
                       "started_at": "", "verified_at": "", "completed_at": ""})
            })
            .collect();
        serde_json::from_value(json!({
            "project": {"name": "p", "repository": "o/p", "default_branch": "master",
                        "active_milestone": "M"},
            "statuses": {"issue": [], "milestone": []},
            "milestones": [], "issues": issues, "waves": [], "edges": [],
            "milestone_edges": [], "findings": []}))
        .expect("a plan")
    }

    fn candidate(issue: &str, scope: &[&str], refused: bool) -> ProceedCandidate {
        ProceedCandidate {
            issue: issue.into(),
            title: format!("t {issue}"),
            milestone: "M".into(),
            terms: ProceedTerms {
                in_milestone: true,
                wave: 0,
                priority: "p1".into(),
                unblocks: Vec::new(),
            },
            refused: if refused {
                vec![ProceedRefusal {
                    constraint: ProceedConstraint::Claimed,
                    hold: "m/c".into(),
                    holder: "another".into(),
                    detail: format!("claims {issue} as its issue"),
                }]
            } else {
                Vec::new()
            },
            shared_with: Vec::new(),
            scope: scope.iter().map(|s| s.to_string()).collect(),
            finishes_with: ProceedEvidence {
                objective: "o".into(),
                have: 0,
                need: 1,
            },
        }
    }

    fn decision(candidates: Vec<ProceedCandidate>) -> ProceedDecision {
        ProceedDecision {
            selected: candidates.iter().find(|c| c.refused.is_empty()).cloned(),
            reason: "the plan's reason".into(),
            provisional: false,
            next: Vec::new(),
            milestone: "M".into(),
            candidates,
            waiting: BTreeMap::new(),
            sources: Vec::new(),
        }
    }

    fn view(
        stage: IntentStage,
        criteria: usize,
        unmet: &[(&str, &[&str])],
    ) -> IntentRealizationView {
        IntentRealizationView {
            intent: "probe".into(),
            title: "t".into(),
            stage,
            criteria,
            met: criteria - unmet.len(),
            unmet: unmet
                .iter()
                .map(|(id, issues)| IntentUnmetCriterion {
                    id: id.to_string(),
                    criterion: "c".into(),
                    evidence: "test".into(),
                    state: crate::intent::IntentEvidenceState::NotRun,
                    reproduce: None,
                    issues: issues.iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
            work: Vec::new(),
            providers: Vec::new(),
            findings: Vec::new(),
        }
    }

    const AT: u64 = 1_792_749_600; // 2026-10-23T10:00:00Z

    fn order(
        plan: &Plan,
        decision: &ProceedDecision,
        intent: Option<&IntentRealizationView>,
        at: u64,
    ) -> WorkOrder {
        let efforts = BTreeMap::from([("implementation".to_string(), "medium".to_string())]);
        step(&StepFacts {
            plan,
            decision,
            intent,
            efforts: &efforts,
            model: Some("m-1"),
            at,
        })
    }

    #[test]
    fn the_same_state_answers_the_same_order_and_a_changed_one_another() {
        let p = plan(&[
            ("I1", false, "implementation"),
            ("I2", false, "implementation"),
        ]);
        let d = decision(vec![
            candidate("I1", &["a"], false),
            candidate("I2", &["b"], false),
        ]);
        let one = order(&p, &d, None, AT);
        assert_eq!(
            one,
            order(&p, &d, None, AT),
            "the same facts, the same order"
        );
        assert_eq!(stamp(AT), "2026-10-23T10:00:00Z");
        assert_eq!(one.at, "2026-10-23T10:00:00Z");
        assert_eq!(one.next_due_at.as_deref(), Some("2026-10-23T10:05:00Z"));
        // inside the interval nothing differs but the moment asked at
        let later = order(&p, &d, None, AT + 299);
        assert_eq!(later.next_due_at, one.next_due_at);
        assert_eq!(later.key, one.key);
        // in the next interval it is due later and is still the same order
        let next = order(&p, &d, None, AT + 300);
        assert_eq!(next.next_due_at.as_deref(), Some("2026-10-23T10:10:00Z"));
        assert_eq!(next.key, one.key, "the key does not depend on the clock");
        assert!(
            one.key.starts_with("step-") && one.key.len() == 21,
            "{}",
            one.key
        );
        // a claim on the first issue is another state, and another order
        let held = decision(vec![
            candidate("I1", &["a"], true),
            candidate("I2", &["b"], false),
        ]);
        let other = order(&p, &held, None, AT);
        assert_ne!(other.key, one.key);
        assert_eq!(other.work.as_ref().unwrap().issue, "I2");
    }

    #[test]
    fn an_issue_is_ordered_as_devtask_implement_under_its_own_key() {
        let p = plan(&[("I1", false, "implementation")]);
        let d = decision(vec![candidate("I1", &["src/a"], false)]);
        let o = order(&p, &d, None, AT);
        assert_eq!(o.outcome, OrderOutcome::Implement);
        assert!(!o.done);
        assert_eq!(o.reason, "the plan's reason");
        let w = o.work.expect("an order");
        assert_eq!(w.capability, "devtask.implement");
        assert_eq!(
            w.input,
            json!({"issue": "I1", "idempotency_key": "implement-I1"})
        );
        assert_eq!(
            (w.profile.as_str(), w.effort.as_deref(), w.model.as_deref()),
            ("implementation", Some("medium"), Some("m-1"))
        );
        assert!(o.fan_out.is_empty(), "I1 is not parallel safe");
        assert_eq!(o.report.claim, "proceed");
        assert_eq!(o.report.scope, ["src/a"]);
        assert_eq!(o.report.intent, "I1: t I1 (the plan's reason)");
        // the wire name of the order is `do`
        let v = serde_json::to_value(order(&p, &d, None, AT)).unwrap();
        assert_eq!(v["do"]["issue"], "I1");
        // a profile that declares no effort leaves it unnamed
        let bare = plan(&[("I1", false, "unknown")]);
        assert_eq!(order(&bare, &d, None, AT).work.unwrap().effort, None);
    }

    #[test]
    fn only_parallel_safe_issues_apart_from_the_order_fan_out_and_at_most_four() {
        let p = plan(&[
            ("I1", true, "implementation"),
            ("I2", true, "implementation"),
            ("I3", true, "implementation"),
            ("I4", false, "implementation"),
            ("I5", true, "implementation"),
            ("I6", true, "implementation"),
            ("I7", true, "implementation"),
            ("I8", true, "implementation"),
        ]);
        let d = decision(vec![
            candidate("I1", &["src/a"], false),
            candidate("I2", &["src/a/x.rs"], false), // shares a path with I1
            candidate("I3", &["src/c"], true),       // held
            candidate("I4", &["src/d"], false),      // not parallel safe
            candidate("I5", &["src/e"], false),
            candidate("I6", &["src/f"], false),
            candidate("I7", &["src/g"], false),
            candidate("I8", &["src/h"], false),
        ]);
        let o = order(&p, &d, None, AT);
        let fanned: Vec<&str> = o.fan_out.iter().map(|w| w.issue.as_str()).collect();
        assert_eq!(fanned, ["I5", "I6", "I7", "I8"]);
        assert_eq!(o.fan_out.len(), MAX_FAN_OUT);
        assert_eq!(
            o.report.scope,
            ["src/a", "src/e", "src/f", "src/g", "src/h"]
        );
        assert!(
            o.report.intent.contains("and 4 beside it"),
            "{}",
            o.report.intent
        );
        // an ordered issue that must run alone fans nothing out
        let alone = plan(&[
            ("I1", false, "implementation"),
            ("I5", true, "implementation"),
        ]);
        assert!(order(&alone, &d, None, AT).fan_out.is_empty());
    }

    #[test]
    fn an_intent_orders_only_what_serves_its_unmet_criteria_and_stops_when_met() {
        let p = plan(&[
            ("I1", false, "implementation"),
            ("I2", false, "implementation"),
        ]);
        let d = decision(vec![
            candidate("I1", &["a"], false),
            candidate("I2", &["b"], false),
        ]);
        let open = view(IntentStage::Executing, 2, &[("b", &["I2"])]);
        let o = order(&p, &d, Some(&open), AT);
        assert_eq!(
            o.work.as_ref().unwrap().issue,
            "I2",
            "I1 serves nothing unmet"
        );
        assert_eq!(
            o.reason,
            "I2 is the first READY issue serving an unmet criterion of probe that nothing refuses (1 of 2 criteria met)"
        );
        let i = o.intent.as_ref().unwrap();
        assert_eq!(
            (i.unmet.clone(), i.serving.clone()),
            (vec!["b".to_string()], vec!["I2".to_string()])
        );

        // met: done, nothing ordered, nothing scheduled
        let met = view(IntentStage::Verifying, 2, &[]);
        let o = order(&p, &d, Some(&met), AT);
        assert_eq!(o.outcome, OrderOutcome::Done);
        assert!(o.done && o.work.is_none() && o.fan_out.is_empty() && o.next_due_at.is_none());
        assert_eq!(
            o.reason,
            "the intent probe is met: all 2 criteria have current evidence"
        );
        assert_eq!(o.report.intent, o.reason);
        assert!(o.report.scope.is_empty());
        let later = order(&p, &d, Some(&met), AT + 10_000);
        assert_eq!(
            (later.key.as_str(), later.next_due_at.as_deref()),
            (o.key.as_str(), None),
            "a done order is the same order at any time, and schedules nothing"
        );

        let satisfied = view(IntentStage::Satisfied, 1, &[]);
        assert!(order(&p, &d, Some(&satisfied), AT)
            .reason
            .contains("is satisfied: 1 of 1"));
        for stage in [IntentStage::Cancelled, IntentStage::Superseded] {
            let o = order(&p, &d, Some(&view(stage, 1, &[("a", &["I1"])])), AT);
            assert!(o.done, "{stage:?}");
            assert!(
                o.reason.ends_with("nothing realises it any more"),
                "{}",
                o.reason
            );
        }
        // an intent that declares no criterion is never met by having none unmet
        let empty = view(IntentStage::Planned, 0, &[]);
        let o = order(&p, &d, Some(&empty), AT);
        assert_eq!(o.outcome, OrderOutcome::Wait);
        assert!(
            o.reason
                .starts_with("no issue of the plan declares it serves"),
            "{}",
            o.reason
        );
    }

    #[test]
    fn a_step_waits_with_its_reason_and_the_whole_plan_is_done_only_when_empty() {
        let p = plan(&[("I1", false, "implementation")]);
        // the serving issue is held
        let held = decision(vec![candidate("I1", &["a"], true)]);
        let open = view(IntentStage::Executing, 1, &[("a", &["I1"])]);
        let o = order(&p, &held, Some(&open), AT);
        assert_eq!(o.outcome, OrderOutcome::Wait);
        assert!(
            o.reason
                .starts_with("none of the issues serving probe (I1)"),
            "{}",
            o.reason
        );
        assert_eq!(o.next_due_at.as_deref(), Some("2026-10-23T10:05:00Z"));
        // the whole plan, every issue held: a wait in the decision's words
        let o = order(&p, &held, None, AT);
        assert_eq!(
            (o.outcome, o.reason.as_str()),
            (OrderOutcome::Wait, "the plan's reason")
        );
        // the whole plan, something waits on a dependency: a wait
        let mut waits = decision(Vec::new());
        waits.waiting.insert("BLOCKED".into(), 1);
        assert_eq!(order(&p, &waits, None, AT).outcome, OrderOutcome::Wait);
        // the whole plan, nothing unfinished: done
        let empty = decision(Vec::new());
        let o = order(&p, &empty, None, AT);
        assert!(o.done && o.next_due_at.is_none());
        // a provisional decision orders nothing, and says what it would have ordered
        let mut provisional = decision(vec![candidate("I1", &["a"], false)]);
        provisional.provisional = true;
        provisional.reason =
            "I1 is first; provisional, decided without all of: mesh (see sources)".into();
        let o = order(&p, &provisional, None, AT);
        assert_eq!(o.outcome, OrderOutcome::Wait);
        assert!(o.work.is_none());
        assert_eq!(
            o.reason,
            "I1 would be ordered, but the decision is provisional, decided without all of: mesh (see sources)"
        );
        assert_eq!(
            provisional_words("no marker"),
            "the decision is provisional"
        );
    }

    #[test]
    fn next_due_is_the_next_boundary_strictly_after() {
        assert_eq!(next_due(0, 300), 300);
        assert_eq!(next_due(299, 300), 300);
        assert_eq!(next_due(300, 300), 600);
        assert!(parallel_safe(&plan(&[("I1", true, "x")]), "I1"));
        assert!(!parallel_safe(&plan(&[("I1", true, "x")]), "I9"));
    }

    #[test]
    fn the_handler_answers_an_order_and_refuses_a_bad_moment_or_intent() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().unwrap();
        let input = |intent: &str, at: Option<&str>| ProceedStepInput {
            intent: intent.into(),
            at: at.map(str::to_string),
            local: true,
            ..ProceedStepInput::default()
        };
        let o = proceed_step(&ctx, input("", Some("2026-10-23T10:00:00Z"))).unwrap();
        assert_eq!(
            o,
            proceed_step(&ctx, input("", Some("2026-10-23T10:00:00Z"))).unwrap()
        );
        assert_eq!(o.at, "2026-10-23T10:00:00Z");
        // the clock, when no moment is given
        assert!(proceed_step(&ctx, input("", None))
            .unwrap()
            .at
            .ends_with('Z'));
        assert!(matches!(
            proceed_step(&ctx, input("", Some("yesterday"))),
            Err(CapabilityError::InvalidInput(_))
        ));
        assert!(matches!(
            proceed_step(&ctx, input("no-such-intent", None)),
            Err(CapabilityError::NotFound(_))
        ));
        let m = super::super::module();
        let e = m
            .capabilities
            .iter()
            .find(|e| e.capability.id.as_str() == "proceed.step")
            .expect("declared");
        let skill = e.capability.skill.as_ref().expect("a skill");
        assert_eq!(
            (skill.alias.as_str(), skill.argument.as_str()),
            ("proceed", "intent")
        );
    }
}
