//! The evidence half of a gap, drafted from the ledger rather than transcribed by hand.
//!
//! A gap (`.ai/repo/project/gaps/<intent>.yaml`, schema `majordomus.gap/v1`) answers every
//! criterion of an intent with `satisfied`, `missing`, `conflicting` or `unknown` at one
//! commit, each answer citing the observations it rests on. Until this module a worker read
//! each criterion's evidence state off `majordomus intent show` and typed it into that file: a
//! second, unchecked derivation of something [`crate::intent`] already derives. Here the
//! derivation is done once, from the same [`IntentView`] every intent surface answers with,
//! and printed as a record `intent validate` accepts unchanged.
//!
//! **A draft is printed, never written.** `intents.draft_gap` (`majordomus intent draft gap
//! <intent>`) answers the record; the worker reads it, adds what only a reader can observe —
//! risks, suggested work, a criterion the evidence cannot settle — and commits the file. The
//! repository's writers are unchanged, so the gap remains what a worker recorded, now with
//! its evidence half derived rather than copied.
//!
//! The state of a condition follows one table, over the criterion's evidence state and
//! whether the plan already says the work serving it is done:
//!
//! | evidence        | serving issue DONE | condition     |
//! |-----------------|--------------------|---------------|
//! | `current`       | either             | `satisfied`   |
//! | `failing`       | yes                | `conflicting` |
//! | `failing`       | no                 | `missing`     |
//! | `not_run`       | either             | `missing`     |
//! | `unresolved`    | either             | `missing`     |
//! | `stale`         | either             | `unknown`     |
//! | `not_derivable` | either             | `unknown`     |
//!
//! A failing run under work the plan calls DONE is a contradiction rather than an absence:
//! the record claims a delivery the evidence refutes. A stale pass neither proves nor refutes
//! anything at this tree, and a `command` or `deployment` criterion is not something the
//! ledger can settle, so both are `unknown`, which is never read as satisfied.
//!
//! Everything in the draft is a function of the commit and the records at it: the commit is
//! HEAD, the time is HEAD's committer date and not the clock, and the order is the order the
//! intent declares its criteria in. Two drafts at one commit are the same bytes.
//!
//! ```
//! use majordomus_cli::intent::IntentEvidenceState;
//! use majordomus_cli::intent_draft::condition_state;
//! use majordomus_cli::intent_review::ConditionState;
//!
//! assert_eq!(condition_state(IntentEvidenceState::Current, false), ConditionState::Satisfied);
//! // a failing run is missing work, until the plan says the work was delivered
//! assert_eq!(condition_state(IntentEvidenceState::Failing, false), ConditionState::Missing);
//! assert_eq!(condition_state(IntentEvidenceState::Failing, true), ConditionState::Conflicting);
//! assert_eq!(condition_state(IntentEvidenceState::Stale, true), ConditionState::Unknown);
//! ```

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::evidence::{Execution, Outcome, LEDGER_PATH};
use crate::intent::{IntentCriterion, IntentEvidenceState, IntentView};
use crate::intent_review::ConditionState;
use crate::plan::Plan;

// ---------------------------------------------------------------- the record

/// One observation of the draft: what was read, and where — `test:<ref>`, `claim:<id>`,
/// `command:<ref>` or `deployment:<ref>` for the criterion's own evidence, or the intent
/// record itself (`file:<path>`) when the criterion names no evidence to read.
///
/// ```
/// use majordomus_cli::intent_draft::DraftObservation;
/// let o = DraftObservation {
///     id: "the-case-passes-evidence".into(),
///     statement: "ledger: no execution of test/cases/00_x.sh is recorded".into(),
///     source: "test:test/cases/00_x.sh".into(),
/// };
/// assert!(o.source.starts_with("test:"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftObservation {
    /// `<criterion>-evidence`: unique within the draft, because criterion ids are unique
    /// within the intent.
    pub id: String,
    /// What was read: the ledger execution with its outcome, commit, time and runner, or why
    /// there was none.
    pub statement: String,
    /// Where it was read.
    pub source: String,
}

/// One criterion's state, the sentence saying which evidence state it follows from, and the
/// observation it cites.
///
/// ```
/// use majordomus_cli::intent_draft::DraftCondition;
/// use majordomus_cli::intent_review::ConditionState;
/// let c = DraftCondition {
///     criterion: "the-case-passes".into(),
///     state: ConditionState::Missing,
///     because: "the evidence is not_run: no recorded run counts for it".into(),
///     observations: vec!["the-case-passes-evidence".into()],
/// };
/// assert_eq!(serde_json::to_value(&c).unwrap()["state"], "missing");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftCondition {
    /// The criterion id of the intent.
    pub criterion: String,
    /// The state the table derives.
    pub state: ConditionState,
    /// Why: the evidence state word, in a sentence.
    pub because: String,
    /// The observation ids it rests on; one, always.
    pub observations: Vec<String>,
}

/// A work item of a gap. The draft never suggests work — what to do about an unmet criterion
/// is a reader's judgement — so the list is always empty; the type exists so that the record
/// carries every key of the schema in the schema's order.
///
/// ```
/// use majordomus_cli::intent_draft::DraftWork;
/// let w = DraftWork { id: "w".into(), title: "t".into(), serves: vec!["c".into()] };
/// assert_eq!(w.serves, ["c"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DraftWork {
    /// Unique within the gap.
    pub id: String,
    /// One line.
    pub title: String,
    /// The criterion ids it would make true.
    pub serves: Vec<String>,
}

/// A drafted gap record, its keys in the order `majordomus.gap/v1` lists them, so that its
/// YAML ([`to_yaml`]) saved as `.ai/repo/project/gaps/<intent>.yaml` is the record itself.
///
/// ```
/// use majordomus_cli::intent_draft::GapDraft;
/// let g = GapDraft {
///     intent: "x".into(),
///     observed_at: "0123456789abcdef0123456789abcdef01234567".into(),
///     recorded_by: "majordomus 1.0.0".into(),
///     recorded_at: "2026-10-21T09:00:00+02:00".into(),
///     observations: vec![],
///     conditions: vec![],
///     risks: vec![],
///     work: vec![],
/// };
/// let keys: Vec<String> = serde_json::to_value(&g).unwrap().as_object().unwrap().keys().cloned().collect();
/// assert_eq!(keys, ["intent", "observed_at", "recorded_by", "recorded_at",
///                   "observations", "conditions", "risks", "work"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GapDraft {
    /// The intent analysed; also the file name the record is saved under.
    pub intent: String,
    /// The full id of the commit the evidence was judged at: HEAD.
    pub observed_at: String,
    /// `majordomus <version>`: the executable that drafted it.
    pub recorded_by: String,
    /// HEAD's committer date, ISO 8601, so a second draft at one commit is the same bytes.
    pub recorded_at: String,
    /// One per criterion, in the order the intent declares them.
    pub observations: Vec<DraftObservation>,
    /// One per criterion, in the same order.
    pub conditions: Vec<DraftCondition>,
    /// Empty, unless the tree the evidence was judged on was not the commit.
    pub risks: Vec<String>,
    /// Always empty: work is a reader's to suggest.
    pub work: Vec<DraftWork>,
}

// ---------------------------------------------------------------- the inputs

/// The commit a draft is made at, as git states it: HEAD's full id, its committer date, and
/// whether the working tree differed from it (the evidence ledger's own file aside, which a
/// recorded run rewrites and which is not a change to anything a run measured).
///
/// ```
/// use majordomus_cli::intent_draft::DraftCommit;
/// let at = DraftCommit {
///     head: "0123456789abcdef0123456789abcdef01234567".into(),
///     recorded_at: "2026-10-21T09:00:00+02:00".into(),
///     dirty: false,
/// };
/// assert!(!at.dirty);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftCommit {
    /// HEAD's full commit id.
    pub head: String,
    /// HEAD's committer date, ISO 8601 (`git log -1 --format=%cI`).
    pub recorded_at: String,
    /// Whether the working tree differed from HEAD.
    pub dirty: bool,
}

/// What a draft reads for one criterion beyond its derived state: the ledger execution that
/// state rests on, and the issues serving the criterion that the plan derives DONE.
///
/// ```
/// use majordomus_cli::intent_draft::CriterionReading;
/// let nothing = CriterionReading::default();
/// assert!(nothing.execution.is_none() && nothing.delivered.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CriterionReading {
    /// The ledger execution, when there is one ([`crate::intent::RepositoryEvidence::execution`]).
    pub execution: Option<Execution>,
    /// The DONE issues that serve `<intent>#<criterion>`, in plan order ([`delivered`]).
    pub delivered: Vec<String>,
}

/// The issues serving `<intent>#<criterion>` that the plan derives `DONE`, in plan order: the
/// work the record says has delivered the criterion.
///
/// ```
/// use majordomus_cli::intent_draft::delivered;
/// use majordomus_cli::plan::Plan;
/// # let plan: Plan = serde_json::from_value(serde_json::json!({
/// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
/// #                 "active_milestone": ""},
/// #     "statuses": {"issue": [], "milestone": []},
/// #     "milestones": [], "waves": [], "edges": [], "milestone_edges": [], "findings": [],
/// #     "issues": [{"id": "I1", "milestone": "m", "status": "DONE", "wave": 1,
/// #         "priority": "p1", "profile": "implementation", "parallel_safe": true,
/// #         "title": "t", "slug": "", "depends_on": [], "blocked_by": [], "dependents": [],
/// #         "scope": ["src"], "serves": ["x#a"], "objective": "", "evidence_have": 1,
/// #         "evidence_need": 1, "started_at": "2026-09-16", "verified_at": "",
/// #         "completed_at": "2026-09-17"}]})).unwrap();
/// // I1 is DONE and serves x#a
/// assert_eq!(delivered(&plan, "x", "a"), ["I1"]);
/// assert!(delivered(&plan, "x", "b").is_empty());
/// ```
pub fn delivered(plan: &Plan, intent: &str, criterion: &str) -> Vec<String> {
    let link = format!("{intent}#{criterion}");
    plan.issues
        .iter()
        .filter(|i| i.status == "DONE" && i.serves.iter().any(|s| s.trim() == link))
        .map(|i| i.id.clone())
        .collect()
}

/// HEAD, its committer date and whether the working tree differs from it, read from git
/// without touching the staging index. A repository with no commit, or one git cannot read,
/// cannot be drafted at: an observation of no tree cannot be checked, and the gap validator
/// refuses a record without `observed_at`.
///
/// ```
/// use majordomus_cli::intent_draft::commit_at;
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     Command::new("git").arg("-C").arg(dir.path()).args(args).output().unwrap()
/// };
/// git(&["init", "-q"]);
/// // nothing committed yet: no commit to draft at
/// assert!(commit_at(dir.path()).is_err());
/// git(&["config", "user.email", "t@example.com"]);
/// git(&["config", "user.name", "t"]);
/// std::fs::write(dir.path().join("a.md"), "a").unwrap();
/// git(&["add", "-A"]);
/// git(&["commit", "-qm", "one"]);
/// let at = commit_at(dir.path()).unwrap();
/// assert_eq!(at.head.len(), 40);
/// assert!(!at.dirty);
/// // the time is the commit's, so asking again answers the same
/// assert_eq!(commit_at(dir.path()).unwrap(), at);
/// std::fs::write(dir.path().join("a.md"), "b").unwrap();
/// assert!(commit_at(dir.path()).unwrap().dirty);
/// ```
pub fn commit_at(root: &Path) -> Result<DraftCommit, String> {
    let head = match crate::git::inspect(root) {
        crate::git::GitState::Available(info) => info.head,
        crate::git::GitState::Unavailable { reason } => return Err(reason),
    }
    .ok_or_else(|| {
        "this repository has no commit yet, so there is nothing to observe the evidence at"
            .to_string()
    })?;
    let out = crate::git::read_only(root)
        .args(["log", "-1", "--format=%cI", &head])
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    let recorded_at = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || recorded_at.is_empty() {
        return Err(format!(
            "git could not state the committer date of {head}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let dirty = crate::git::working_tree_ignoring(root, &[LEDGER_PATH]) != "clean";
    Ok(DraftCommit {
        head,
        recorded_at,
        dirty,
    })
}

// ---------------------------------------------------------------- the derivation

/// The condition a criterion's evidence state derives, by the table in the module
/// documentation. `delivered` is whether an issue serving the criterion is DONE.
///
/// ```
/// use majordomus_cli::intent::IntentEvidenceState as E;
/// use majordomus_cli::intent_draft::condition_state;
/// use majordomus_cli::intent_review::ConditionState as C;
/// assert_eq!(condition_state(E::NotRun, true), C::Missing);
/// assert_eq!(condition_state(E::Unresolved, false), C::Missing);
/// assert_eq!(condition_state(E::NotDerivable, false), C::Unknown);
/// ```
pub fn condition_state(state: IntentEvidenceState, delivered: bool) -> ConditionState {
    match state {
        IntentEvidenceState::Current => ConditionState::Satisfied,
        IntentEvidenceState::Failing if delivered => ConditionState::Conflicting,
        IntentEvidenceState::Failing
        | IntentEvidenceState::NotRun
        | IntentEvidenceState::Unresolved => ConditionState::Missing,
        IntentEvidenceState::Stale | IntentEvidenceState::NotDerivable => ConditionState::Unknown,
    }
}

/// Who drafts: this executable, by name and version.
///
/// ```
/// let by = majordomus_cli::intent_draft::recorded_by();
/// assert!(by.starts_with("majordomus "));
/// assert!(by.ends_with(majordomus_cli::VERSION));
/// ```
pub fn recorded_by() -> String {
    format!("majordomus {}", crate::VERSION)
}

/// Draft the gap of one intent at one commit: one observation and one condition per
/// criterion, in the order the intent declares them, each condition citing its observation.
///
/// Pure: everything it reads is passed in — the intent as [`crate::intent::Intents`] derived
/// it, the commit, and for each criterion what [`CriterionReading`] holds — so the same
/// inputs give the same record, and the tests can construct every evidence state without a
/// repository.
///
/// ```
/// use majordomus_cli::intent::{verdict, IntentCriterion, IntentEvidenceState, IntentStage, IntentView};
/// use majordomus_cli::intent_draft::{draft_gap, CriterionReading, DraftCommit};
/// use majordomus_cli::intent_review::ConditionState;
///
/// let criterion = IntentCriterion {
///     id: "the-case-passes".into(),
///     criterion: "the case passes".into(),
///     evidence: "test".into(),
///     reference: "test/cases/00_x.sh".into(),
///     state: IntentEvidenceState::Failing,
///     met: false,
///     proof: None,
///     reproduce: None,
/// };
/// let intent = IntentView {
///     id: "x".into(), title: "t".into(), statement: "s".into(), invariants: vec![],
///     stage: IntentStage::Executing, milestones: vec![],
///     satisfaction: vec![criterion.clone()], met: 0, verdict: verdict(&[criterion]),
///     governance: vec![], non_goals: vec![], superseded_by: None,
///     source: ".ai/repo/project/intents/x.yaml".into(),
/// };
/// let at = DraftCommit { head: "c".repeat(40), recorded_at: "2026-10-21T09:00:00Z".into(), dirty: false };
/// let open = draft_gap(&intent, &at, |_| CriterionReading::default());
/// assert_eq!(open.conditions[0].state, ConditionState::Missing);
/// assert_eq!(open.conditions[0].observations, [open.observations[0].id.clone()]);
///
/// let done = draft_gap(&intent, &at, |_| CriterionReading {
///     execution: None,
///     delivered: vec!["I1".into()],
/// });
/// assert_eq!(done.conditions[0].state, ConditionState::Conflicting);
/// assert!(done.conditions[0].because.contains("I1"));
/// assert!(done.risks.is_empty() && done.work.is_empty());
/// ```
pub fn draft_gap(
    intent: &IntentView,
    at: &DraftCommit,
    read: impl Fn(&IntentCriterion) -> CriterionReading,
) -> GapDraft {
    let mut observations = Vec::with_capacity(intent.satisfaction.len());
    let mut conditions = Vec::with_capacity(intent.satisfaction.len());
    for c in &intent.satisfaction {
        let reading = read(c);
        let observation = observe(intent, c, reading.execution.as_ref());
        let state = condition_state(c.state, !reading.delivered.is_empty());
        conditions.push(DraftCondition {
            criterion: c.id.clone(),
            state,
            because: because(c, state, &reading.delivered),
            observations: vec![observation.id.clone()],
        });
        observations.push(observation);
    }
    let risks = if at.dirty {
        vec![format!(
            "the evidence was judged on a working tree that differed from {}; what it observed \
             may not be what that commit holds, so draft again on a clean tree before committing \
             the record",
            at.head
        )]
    } else {
        Vec::new()
    };
    GapDraft {
        intent: intent.id.clone(),
        observed_at: at.head.clone(),
        recorded_by: recorded_by(),
        recorded_at: at.recorded_at.clone(),
        observations,
        conditions,
        risks,
        work: Vec::new(),
    }
}

/// The id of a criterion's observation: its slug, then `-evidence`, so it matches the
/// schema's id pattern whatever the criterion was spelled as.
fn observation_id(criterion: &str) -> String {
    let slug = crate::entity::slug(criterion);
    if slug.is_empty() {
        "criterion-evidence".into()
    } else {
        format!("{slug}-evidence")
    }
}

/// The word a ledger outcome is read out as, in a sentence.
fn outcome_word(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pass => "passed",
        Outcome::Fail => "failed",
        Outcome::Skip => "was skipped",
        Outcome::Timeout => "timed out",
        Outcome::Error => "errored",
    }
}

/// One sentence naming an execution: its test, outcome, the full commit it ran at, when it
/// was recorded and by which runner.
fn execution_sentence(e: &Execution) -> String {
    format!(
        "ledger: {} {} at {} ({}, runner {})",
        e.test,
        outcome_word(e.outcome),
        e.commit,
        e.at,
        e.runner.prefix()
    )
}

fn observe(intent: &IntentView, c: &IntentCriterion, e: Option<&Execution>) -> DraftObservation {
    let r = c.reference.trim();
    let unresolved = c.state == IntentEvidenceState::Unresolved;
    let (source, statement) = if r.is_empty() {
        (
            format!("file:{}", intent.source),
            format!(
                "the intent record names no {} evidence for this criterion, so there is nothing \
                 to read",
                c.evidence
            ),
        )
    } else {
        match c.evidence.as_str() {
            "test" => (
                format!("test:{r}"),
                match e {
                    Some(e) => execution_sentence(e),
                    None if unresolved => {
                        format!("{r} names no test in this tree, so the ledger was not asked")
                    }
                    None => format!("ledger: no execution of {r} is recorded"),
                },
            ),
            "claim" => (
                format!("claim:{r}"),
                match e {
                    Some(e) => format!("claim {r}, {}", execution_sentence(e)),
                    None if unresolved => format!("{r} is not a claim of docs/CLAIMS.yaml"),
                    None => format!("claims matrix: claim {r} has no recorded execution"),
                },
            ),
            "deployment" => (
                format!("deployment:{r}"),
                if unresolved {
                    format!("{r} is not a deployment of this repository")
                } else {
                    format!(
                        "the ledger records no deployment evidence; deployment {r} is observed \
                         where it is served"
                    )
                },
            ),
            // `command`: the schema admits nothing else
            _ => (
                format!("command:{r}"),
                format!("the ledger records no command evidence; `{r}` reproduces it"),
            ),
        }
    };
    DraftObservation {
        id: observation_id(&c.id),
        statement,
        source,
    }
}

fn because(c: &IntentCriterion, state: ConditionState, delivered: &[String]) -> String {
    let word = c.state.as_str();
    let why = match c.state {
        IntentEvidenceState::Current => {
            "a passing run of what the criterion names is current evidence at this tree".into()
        }
        IntentEvidenceState::Failing if state == ConditionState::Conflicting => format!(
            "the latest recorded run did not pass while {} serving this criterion {} DONE, so \
             the record claims a delivery the evidence contradicts",
            delivered.join(", "),
            if delivered.len() == 1 { "is" } else { "are" }
        ),
        IntentEvidenceState::Failing => "the latest recorded run did not pass".into(),
        IntentEvidenceState::NotRun => "no recorded run counts for it".into(),
        IntentEvidenceState::Unresolved => {
            "the reference names nothing this repository holds".into()
        }
        IntentEvidenceState::Stale => {
            "a pass that is not current evidence at this tree, which neither proves nor refutes it"
                .into()
        }
        IntentEvidenceState::NotDerivable => format!(
            "{} evidence is not something the ledger records, so it cannot settle the criterion",
            c.evidence
        ),
    };
    format!("the evidence is {word}: {why}")
}

// ---------------------------------------------------------------- the text

/// The draft as the YAML document a worker saves under `.ai/repo/project/gaps/<intent>.yaml`:
/// the layer's own deterministic writer ([`crate::metadata::yaml::render`]) over the record's
/// JSON, so its keys come in the schema's order, its scalars are quoted only where reading
/// them back would change them, and the same draft is always the same bytes.
///
/// ```
/// use majordomus_cli::intent_draft::{to_yaml, GapDraft};
/// use majordomus_cli::metadata::yaml::parse_mapping;
/// let g = GapDraft {
///     intent: "x".into(),
///     observed_at: "0123456789abcdef0123456789abcdef01234567".into(),
///     recorded_by: "majordomus 1.0.0".into(),
///     recorded_at: "2026-10-21T09:00:00+02:00".into(),
///     observations: vec![],
///     conditions: vec![],
///     risks: vec![],
///     work: vec![],
/// };
/// let text = to_yaml(&g);
/// assert!(text.starts_with("intent: x\nobserved_at: "));
/// assert_eq!(text, to_yaml(&g.clone()), "the same draft is the same bytes");
/// // and the layer's own reader reads it back as the record it was
/// let back = serde_json::Value::Object(parse_mapping(&text).unwrap());
/// assert_eq!(back, serde_json::to_value(&g).unwrap());
/// ```
pub fn to_yaml(draft: &GapDraft) -> String {
    serde_json::to_value(draft)
        .map(|v| crate::metadata::yaml::render(&v))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{Origin, Runner};
    use crate::intent::{verdict, IntentStage};

    fn criterion(id: &str, evidence: &str, r: &str, state: IntentEvidenceState) -> IntentCriterion {
        IntentCriterion {
            id: id.into(),
            criterion: format!("{id} holds"),
            evidence: evidence.into(),
            reference: r.into(),
            met: state == IntentEvidenceState::Current,
            state,
            proof: None,
            reproduce: None,
        }
    }

    fn intent(criteria: Vec<IntentCriterion>) -> IntentView {
        IntentView {
            id: "fixture-intent".into(),
            title: "t".into(),
            statement: "s".into(),
            invariants: vec![],
            stage: IntentStage::Executing,
            milestones: vec![],
            verdict: verdict(&criteria),
            met: criteria.iter().filter(|c| c.met).count(),
            satisfaction: criteria,
            governance: vec![],
            non_goals: vec![],
            superseded_by: None,
            source: ".ai/repo/project/intents/fixture-intent.yaml".into(),
        }
    }

    fn at(dirty: bool) -> DraftCommit {
        DraftCommit {
            head: "a".repeat(40),
            recorded_at: "2026-10-21T09:00:00+02:00".into(),
            dirty,
        }
    }

    fn run(outcome: Outcome) -> Execution {
        Execution {
            test: "suite:00_x".into(),
            runner: Runner::Suite,
            source: "test/cases/00_x.sh".into(),
            outcome,
            seconds: 1,
            commit: "b".repeat(40),
            working_tree: "clean".into(),
            digest: "sha256:00".into(),
            at: "2026-10-20T00:00:00Z".into(),
            origin: Origin::Local,
            command: "bash test/run.sh 00_x".into(),
            run: None,
            cases: vec![],
            detail: None,
        }
    }

    /// Every row of the table, for every combination of evidence state and delivery.
    #[test]
    fn every_evidence_state_derives_the_condition_the_table_names() {
        use ConditionState as C;
        use IntentEvidenceState as E;
        let table = [
            (E::Current, C::Satisfied, C::Satisfied),
            (E::Failing, C::Missing, C::Conflicting),
            (E::NotRun, C::Missing, C::Missing),
            (E::Unresolved, C::Missing, C::Missing),
            (E::Stale, C::Unknown, C::Unknown),
            (E::NotDerivable, C::Unknown, C::Unknown),
        ];
        for (evidence, open, done) in table {
            assert_eq!(condition_state(evidence, false), open, "{evidence:?}");
            assert_eq!(
                condition_state(evidence, true),
                done,
                "{evidence:?} under DONE work"
            );
        }
    }

    /// Each observation names the source the criterion's evidence kind is read at, and the
    /// execution when the ledger holds one; every condition cites the observation it rests on.
    #[test]
    fn each_observation_cites_what_it_read_and_each_condition_cites_its_observation() {
        let view = intent(vec![
            criterion(
                "a",
                "test",
                "test/cases/00_x.sh",
                IntentEvidenceState::Current,
            ),
            criterion("b", "claim", "policy-parse", IntentEvidenceState::NotRun),
            criterion(
                "c",
                "command",
                "just ship",
                IntentEvidenceState::NotDerivable,
            ),
            criterion(
                "d",
                "deployment",
                "pages",
                IntentEvidenceState::NotDerivable,
            ),
            criterion("e", "test", "", IntentEvidenceState::Unresolved),
        ]);
        let g = draft_gap(&view, &at(false), |c| CriterionReading {
            execution: (c.id == "a").then(|| run(Outcome::Pass)),
            delivered: vec![],
        });
        let sources: Vec<&str> = g.observations.iter().map(|o| o.source.as_str()).collect();
        assert_eq!(
            sources,
            [
                "test:test/cases/00_x.sh",
                "claim:policy-parse",
                "command:just ship",
                "deployment:pages",
                "file:.ai/repo/project/intents/fixture-intent.yaml",
            ]
        );
        assert_eq!(
            g.observations[0].statement,
            format!(
                "ledger: suite:00_x passed at {} (2026-10-20T00:00:00Z, runner suite)",
                "b".repeat(40)
            )
        );
        let states: Vec<ConditionState> = g.conditions.iter().map(|c| c.state).collect();
        assert_eq!(
            states,
            [
                ConditionState::Satisfied,
                ConditionState::Missing,
                ConditionState::Unknown,
                ConditionState::Unknown,
                ConditionState::Missing,
            ]
        );
        for (c, o) in g.conditions.iter().zip(&g.observations) {
            assert_eq!(c.observations, std::slice::from_ref(&o.id));
            assert!(c.because.starts_with("the evidence is "), "{}", c.because);
        }
        assert_eq!(g.observed_at, "a".repeat(40));
        assert!(g.risks.is_empty());
    }

    /// The same inputs are the same bytes; a dirty tree adds the one risk and nothing else.
    #[test]
    fn the_same_inputs_are_the_same_bytes_and_a_dirty_tree_is_a_risk() {
        let view = intent(vec![criterion(
            "a",
            "test",
            "test/cases/00_x.sh",
            IntentEvidenceState::Failing,
        )]);
        let read = |_: &IntentCriterion| CriterionReading {
            execution: Some(run(Outcome::Fail)),
            delivered: vec!["I0001".into()],
        };
        let one = to_yaml(&draft_gap(&view, &at(false), read));
        let two = to_yaml(&draft_gap(&view, &at(false), read));
        assert_eq!(one, two);
        assert!(one.contains("state: conflicting"), "{one}");
        assert!(one.contains("risks: []\nwork: []\n"), "{one}");
        let back = crate::metadata::yaml::parse_mapping(&one).unwrap();
        assert_eq!(back["conditions"][0]["observations"][0], "a-evidence");

        let dirty = draft_gap(&view, &at(true), read);
        assert_eq!(dirty.risks.len(), 1);
        assert!(dirty.risks[0].contains(&"a".repeat(40)));
        assert_eq!(
            dirty.conditions,
            draft_gap(&view, &at(false), read).conditions
        );
    }
}
