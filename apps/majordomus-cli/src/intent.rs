//! Intents: what must become true above the milestones that realise it, and whether the
//! recorded evidence says it has.
//!
//! An intent is a record under `.ai/repo/project/intents/` (kind `intent`, schema
//! `majordomus.intent/v1`): a statement, the invariants that must stay true, the milestones
//! that realise it, and the satisfaction criteria that settle it, each naming its evidence.
//!
//! **This is a derivation, not a lifecycle.** No stage is stored, nothing here transitions
//! and nothing here writes. The stage of an intent is a pure function of two things that
//! already exist: the status [`crate::plan::Plan`] derives for each milestone the intent
//! names, and the proof state the evidence ledger holds for each criterion's test or
//! claim. A second task model beside the plan would be a second source of truth about the
//! same work, which is the defect the plan was built to remove (ADR 0070).
//!
//! The stages, in order, each implying the one before:
//!
//! | stage       | derived when                                                        |
//! |-------------|---------------------------------------------------------------------|
//! | `declared`  | a named milestone does not resolve, or none is named                 |
//! | `planned`   | every milestone resolves                                             |
//! | `executing` | any milestone is ACTIVE, VERIFY or DONE                              |
//! | `verifying` | every milestone that is not cancelled or superseded is DONE          |
//! | `satisfied` | verifying, and every criterion has current evidence                  |
//!
//! `cancelled` and `superseded` are what the record says happened to it, as for a
//! milestone. Nothing else is.
//!
//! Beside the stage stands the **verdict** (ADR 0107): the evidence question alone, derived
//! from the criteria and never from the plan. It is `satisfied` when every criterion is met,
//! `unsatisfied` when a criterion the ledger or the claim join can settle (`test`, `claim`) is
//! not, and `unknown` when every unmet criterion is of a kind the ledger cannot yet settle
//! (`command`, `deployment`) or the intent declares none. DONE neither implies it nor is
//! required for it, so an intent whose criteria all hold while a milestone is open reads a
//! `satisfied` verdict at stage `executing` — a disagreement the realization reports as drift.
//!
//! ```
//! use majordomus_cli::intent::{IntentStage, IntentVerdictState};
//! assert_eq!(IntentStage::Satisfied.as_str(), "satisfied");
//! assert!(IntentStage::Verifying < IntentStage::Satisfied);
//! assert_eq!(IntentVerdictState::Unknown.as_str(), "unknown");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::evidence::{
    self, compare, freshness, Comparison, EvidenceReport, Execution, Ledger, Presented, ProofState,
    Recorded, TestId, TreeState,
};
use crate::index::Index;
use crate::intent_plan::{coverage, IntentCoverage, IntentOutline};
use crate::intent_review::{
    observed_satisfied, review, CritiqueFinding, CritiqueRecord, GapCondition, GapRecord,
    ResolutionState,
};
use crate::plan::{overlap, Plan};

/// The kind of an intent record.
pub const INTENT: &str = "intent";

/// The directory intent records live in, repository-relative.
pub const INTENTS_DIR: &str = ".ai/repo/project/intents";

/// A failure: the intent model is invalid.
pub const FAIL: &str = "FAIL";
/// A warning: legal, and worth reading.
pub const WARN: &str = "WARN";

// ---------------------------------------------------------------- the derived vocabulary

/// Where an intent stands, derived on every read and stored nowhere.
///
/// ```
/// use majordomus_cli::intent::IntentStage;
/// // one word every surface prints, and an order that follows the work
/// assert_eq!(IntentStage::Executing.as_str(), "executing");
/// assert!(IntentStage::Declared < IntentStage::Satisfied);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IntentStage {
    /// A named milestone does not resolve, or none is named.
    Declared,
    /// Every milestone resolves and none has started.
    Planned,
    /// A milestone is under way.
    Executing,
    /// Every milestone that counts is DONE; satisfaction is not yet evidenced.
    Verifying,
    /// Verifying, and every criterion has current evidence.
    Satisfied,
    /// The record says it was abandoned.
    Cancelled,
    /// The record names the intent that replaced it.
    Superseded,
}

impl IntentStage {
    /// The word every surface prints for this stage: the command line, the API, MCP and the
    /// generated documentation all say the same one, so no surface spells a stage its own way.
    ///
    /// ```
    /// use majordomus_cli::intent::IntentStage;
    /// assert_eq!(IntentStage::Declared.as_str(), "declared");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            IntentStage::Declared => "declared",
            IntentStage::Planned => "planned",
            IntentStage::Executing => "executing",
            IntentStage::Verifying => "verifying",
            IntentStage::Satisfied => "satisfied",
            IntentStage::Cancelled => "cancelled",
            IntentStage::Superseded => "superseded",
        }
    }
}

/// What the evidence behind one criterion says, as a word. Only `Current` meets a criterion:
/// a stale, failing, unrecorded or underivable reference never does, and neither does one that
/// resolves to nothing.
///
/// Whether a recorded run is current is not decided here. A `test` or `claim` criterion is
/// judged by the evidence module's one truth table ([`evidence::freshness()`]) at the working
/// tree and read through [`evidence::current`], the same judgement the claim report and the
/// subject pages make; [`IntentEvidenceState::judged`] is the whole of the translation. So
/// the state follows the evidence both ways: a criterion met today reads stale or failing
/// the moment the ledger or the tree says so, and met again once a current pass is recorded.
///
/// ```
/// use majordomus_cli::intent::IntentEvidenceState;
/// assert_eq!(serde_json::to_string(&IntentEvidenceState::NotRun).unwrap(), "\"not_run\"");
/// assert_ne!(IntentEvidenceState::Stale, IntentEvidenceState::Current);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentEvidenceState {
    /// A passing run that is current evidence at the working tree: its commit is in this
    /// history, it measured a tree that was its commit, the test still hashes to what ran,
    /// and nothing the criterion names has changed since — for a test, its source and the
    /// source and implementation of every claim that test proves; for a claim, what the
    /// claim names. A change the criterion does not name, the evidence ledger's own included,
    /// leaves it current. A test that proves no claim naming an implementation names no code
    /// under test, so for it nothing but the ledger may have changed since the run. The only
    /// state that meets a criterion; the criterion's `proof` says whether it rests on
    /// `proven` or on `inputs_unchanged`.
    Current,
    /// A pass that is not current evidence: something the criterion names has changed since
    /// it, it names no code under test and anything but the ledger and the plan's own records
    /// has changed since, the
    /// test no longer hashes to what ran, the run's commit is not in this history (or git
    /// could not compare it), or the run measured a tree that was not its commit — a run on a
    /// dirty tree, which no later tree can be matched against.
    Stale,
    /// The latest recorded run failed, timed out or errored.
    Failing,
    /// The reference resolves and no run counts for it: none was recorded, the latest run
    /// declined to run (a skip), or the claim names no test a runner drives.
    NotRun,
    /// The reference resolves, and this kind of evidence (`command`, `deployment`) is not
    /// derivable from the ledger, so it never meets a criterion by itself.
    NotDerivable,
    /// The reference is empty or names nothing this repository holds.
    Unresolved,
}

impl IntentEvidenceState {
    /// The word every surface prints for this state, the same one serde writes.
    ///
    /// ```
    /// use majordomus_cli::intent::IntentEvidenceState;
    /// assert_eq!(IntentEvidenceState::NotDerivable.as_str(), "not_derivable");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            IntentEvidenceState::Current => "current",
            IntentEvidenceState::Stale => "stale",
            IntentEvidenceState::Failing => "failing",
            IntentEvidenceState::NotRun => "not_run",
            IntentEvidenceState::NotDerivable => "not_derivable",
            IntentEvidenceState::Unresolved => "unresolved",
        }
    }

    /// A verdict of the evidence module's truth table, in this vocabulary: current only when
    /// [`evidence::current`] says so, of a run on the tree `recorded` (the execution's own
    /// `working_tree`; [`TreeState::Unknown`] when there is none). A pass that is not current
    /// is stale — including a run on a dirty tree whose inputs read unchanged — a skip and an
    /// absence are not run, and a failure is failing.
    ///
    /// ```
    /// use majordomus_cli::evidence::{ProofState, TreeState};
    /// use majordomus_cli::intent::IntentEvidenceState;
    ///
    /// let judged = IntentEvidenceState::judged;
    /// assert_eq!(judged(ProofState::InputsUnchanged, TreeState::Clean), IntentEvidenceState::Current);
    /// // a run on a tree that was not its commit proves nothing about a later one
    /// assert_eq!(judged(ProofState::InputsUnchanged, TreeState::Dirty), IntentEvidenceState::Stale);
    /// assert_eq!(judged(ProofState::Failing, TreeState::Clean), IntentEvidenceState::Failing);
    /// ```
    pub fn judged(state: ProofState, recorded: TreeState) -> IntentEvidenceState {
        if evidence::current(state, recorded, Presented::WorkingTree.tree()) {
            return IntentEvidenceState::Current;
        }
        match state {
            ProofState::Proven | ProofState::InputsUnchanged | ProofState::Stale => {
                IntentEvidenceState::Stale
            }
            ProofState::Failing => IntentEvidenceState::Failing,
            ProofState::NotRun | ProofState::Unrunnable | ProofState::NoTest => {
                IntentEvidenceState::NotRun
            }
        }
    }
}

/// What the evidence alone says about an intent, as a word: derived from its criteria and
/// never from the plan, so it can read `satisfied` while a milestone is still open (ADR 0107).
///
/// ```
/// use majordomus_cli::intent::IntentVerdictState;
/// let word = serde_json::to_string(&IntentVerdictState::Unsatisfied).unwrap();
/// assert_eq!(word, "\"unsatisfied\"");
/// assert_eq!(IntentVerdictState::Satisfied.as_str(), "satisfied");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentVerdictState {
    /// Every criterion has current evidence.
    Satisfied,
    /// A criterion the ledger or the claim join can settle (`test`, `claim`) is not met.
    Unsatisfied,
    /// Every unmet criterion is of a kind the ledger cannot yet settle (`command`,
    /// `deployment`), or the intent declares no criterion: the evidence cannot answer.
    Unknown,
}

impl IntentVerdictState {
    /// The word every surface prints for this verdict, the same one serde writes.
    ///
    /// ```
    /// use majordomus_cli::intent::IntentVerdictState;
    /// assert_eq!(IntentVerdictState::Unsatisfied.as_str(), "unsatisfied");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            IntentVerdictState::Satisfied => "satisfied",
            IntentVerdictState::Unsatisfied => "unsatisfied",
            IntentVerdictState::Unknown => "unknown",
        }
    }
}

/// One criterion that holds a verdict back from `satisfied`: which one, the kind of evidence
/// it names, and the state that evidence is in — the criterion's own words, not new ones.
///
/// ```
/// use majordomus_cli::intent::{IntentEvidenceState, IntentVerdictReason};
/// let r = IntentVerdictReason {
///     criterion: "probe-passes".into(),
///     evidence: "test".into(),
///     state: IntentEvidenceState::NotRun,
/// };
/// assert_eq!(r.state, IntentEvidenceState::NotRun);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentVerdictReason {
    /// The criterion's id.
    pub criterion: String,
    /// Its kind of evidence: `test`, `claim`, `command` or `deployment`.
    pub evidence: String,
    /// The state of that evidence, as the criterion reports it.
    pub state: IntentEvidenceState,
}

/// Whether the evidence alone settles an intent, and which criteria hold it back: every
/// unmet criterion, in the order the record declares them. Empty when the verdict is
/// `satisfied`, and when it is `unknown` because the intent declares no criterion.
///
/// ```
/// use majordomus_cli::intent::{verdict, IntentVerdict, IntentVerdictState};
/// // an intent that declares no criterion cannot be settled by evidence
/// let v: IntentVerdict = verdict(&[]);
/// assert_eq!(v.state, IntentVerdictState::Unknown);
/// assert!(v.reasons.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentVerdict {
    /// Derived: what the evidence says.
    pub state: IntentVerdictState,
    /// Every required criterion that is not met, with the state of its evidence.
    pub reasons: Vec<IntentVerdictReason>,
    /// Every guard that is violated: its evidence is failing (ADR 0113). Absent when none
    /// is, so a verdict over an intent with no guards reads as it always did.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<IntentVerdictGuard>,
}

/// One violated guard in a verdict: which, the kind of its evidence, and its state.
///
/// ```
/// use majordomus_cli::intent::{IntentEvidenceState, IntentVerdictGuard};
/// let g = IntentVerdictGuard {
///     guard: "engines-agree".into(),
///     evidence: "test".into(),
///     state: IntentEvidenceState::Failing,
/// };
/// assert_eq!(serde_json::to_value(&g).unwrap()["state"], "failing");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentVerdictGuard {
    /// The guard's id.
    pub guard: String,
    /// `test` or `claim`.
    pub evidence: String,
    /// The state of its evidence: `failing`, or it would not be here.
    pub state: IntentEvidenceState,
}

/// Whether the ledger or the claim join can settle a criterion of this evidence kind.
fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn ledger_settles(evidence: &str) -> bool {
    matches!(evidence, "test" | "claim")
}

/// The verdict of an intent from its criteria alone. Pure: no plan, no index, no ledger — the
/// criteria already carry what the evidence said.
///
/// ```
/// use majordomus_cli::intent::{verdict, IntentCriterion, IntentEvidenceState, IntentVerdictState};
/// let c = |evidence: &str, state: IntentEvidenceState| IntentCriterion {
///     id: evidence.into(), criterion: "c".into(), evidence: evidence.into(),
///     reference: "r".into(), met: state == IntentEvidenceState::Current, state,
///     proof: None, reproduce: None, optional: false, evaluation: None,
/// };
/// let met = c("test", IntentEvidenceState::Current);
/// assert_eq!(verdict(&[met.clone()]).state, IntentVerdictState::Satisfied);
/// // a command the ledger cannot run leaves the evidence unable to answer
/// let cmd = c("command", IntentEvidenceState::NotDerivable);
/// assert_eq!(verdict(&[met.clone(), cmd.clone()]).state, IntentVerdictState::Unknown);
/// // a test the ledger can settle, and has not, says no
/// let unmet = c("test", IntentEvidenceState::NotRun);
/// let v = verdict(&[met, cmd, unmet]);
/// assert_eq!(v.state, IntentVerdictState::Unsatisfied);
/// assert_eq!(v.reasons.len(), 2);
/// ```
pub fn verdict(criteria: &[IntentCriterion]) -> IntentVerdict {
    verdict_with(criteria, &[])
}

/// The verdict over the criteria and the guards (ADR 0113). The required criteria decide it
/// as [`verdict`] says; an optional criterion decides nothing; and a guard whose evidence is
/// failing makes it `unsatisfied` whatever the criteria say, because an intent is not
/// satisfied while what it said must stay true has stopped being true. A guard that is
/// stale, was never run or does not resolve is not judged, and violates nothing: only a
/// recorded failing run is a violation.
///
/// ```
/// use majordomus_cli::intent::{
///     verdict_with, IntentCriterion, IntentEvidenceState, IntentGuard, IntentVerdictState,
/// };
/// let met = IntentCriterion {
///     id: "a".into(), criterion: "c".into(), evidence: "test".into(),
///     reference: "test/cases/1_x.sh".into(), state: IntentEvidenceState::Current, met: true,
///     proof: None, reproduce: None, optional: false, evaluation: None,
/// };
/// let guard = |state| IntentGuard {
///     id: "g".into(), invariant: "stays true".into(), evidence: "test".into(),
///     reference: "test/cases/2_y.sh".into(), state, violated: state == IntentEvidenceState::Failing,
///     reproduce: None, evaluation: None,
/// };
/// // a failing guard outranks met criteria; a stale one is not judged
/// let violated = verdict_with(&[met.clone()], &[guard(IntentEvidenceState::Failing)]);
/// assert_eq!(violated.state, IntentVerdictState::Unsatisfied);
/// assert_eq!(violated.guards[0].guard, "g");
/// let stale = verdict_with(&[met.clone()], &[guard(IntentEvidenceState::Stale)]);
/// assert_eq!(stale.state, IntentVerdictState::Satisfied);
/// // an optional criterion that is not met holds nothing back
/// let optional = IntentCriterion { id: "b".into(), optional: true, met: false,
///     state: IntentEvidenceState::NotRun, ..met.clone() };
/// assert_eq!(verdict_with(&[met, optional.clone()], &[]).state, IntentVerdictState::Satisfied);
/// // and with no required criterion there is no verdict to claim
/// assert_eq!(verdict_with(&[optional], &[]).state, IntentVerdictState::Unknown);
/// ```
pub fn verdict_with(criteria: &[IntentCriterion], guards: &[IntentGuard]) -> IntentVerdict {
    let reasons: Vec<IntentVerdictReason> = criteria
        .iter()
        .filter(|c| !c.optional && !c.met)
        .map(|c| IntentVerdictReason {
            criterion: c.id.clone(),
            evidence: c.evidence.clone(),
            state: c.state,
        })
        .collect();
    let violated: Vec<IntentVerdictGuard> = guards
        .iter()
        .filter(|g| g.violated)
        .map(|g| IntentVerdictGuard {
            guard: g.id.clone(),
            evidence: g.evidence.clone(),
            state: g.state,
        })
        .collect();
    let state = if !violated.is_empty() {
        IntentVerdictState::Unsatisfied
    } else if !criteria.iter().any(|c| !c.optional) {
        IntentVerdictState::Unknown
    } else if reasons.is_empty() {
        IntentVerdictState::Satisfied
    } else if reasons.iter().any(|r| ledger_settles(&r.evidence)) {
        IntentVerdictState::Unsatisfied
    } else {
        IntentVerdictState::Unknown
    };
    IntentVerdict {
        state,
        reasons,
        guards: violated,
    }
}

// ---------------------------------------------------------------- the record as authored

/// One satisfaction criterion as the record declares it: what must be observably true, the
/// kind of evidence that settles it, and what that evidence names.
///
/// ```
/// use majordomus_cli::intent::CriterionRecord;
/// let c = CriterionRecord {
///     id: "surfaces-agree".into(),
///     criterion: "the command line, HTTP and MCP answer the same intents".into(),
///     evidence: "test".into(),
///     reference: "apps/majordomus-cli/tests/intent.rs".into(),
///     optional: false,
/// };
/// assert_eq!(c.evidence, "test");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CriterionRecord {
    /// Unique within the intent.
    pub id: String,
    /// What is observably true when the criterion is met.
    pub criterion: String,
    /// `test`, `claim`, `command` or `deployment`.
    pub evidence: String,
    /// What the evidence names.
    pub reference: String,
    /// Whether the intent declares it optional: evaluated and shown like any other, and
    /// holding neither the verdict nor the satisfied stage back (ADR 0113).
    pub optional: bool,
}

/// A guard as authored: an invariant that names its evidence, so that what must stay true
/// is judged and not only said (ADR 0113). A key of its own beside `invariants`, which
/// stays a list of plain statements.
///
/// ```
/// use majordomus_cli::intent::GuardRecord;
/// let g = GuardRecord {
///     id: "engines-agree".into(),
///     invariant: "The two plan engines stay identical".into(),
///     evidence: "test".into(),
///     reference: "test/cases/99_plan_capabilities.sh".into(),
/// };
/// assert_eq!(g.evidence, "test");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuardRecord {
    /// Unique within the intent, and never a criterion's id.
    pub id: String,
    /// What must stay true.
    pub invariant: String,
    /// `test` or `claim`.
    pub evidence: String,
    /// The test or the claim that would fail if it stopped being true.
    pub reference: String,
}

/// One intent as its file declares it: the statement that must become true, the invariants that
/// must stay true, the milestones that realise it and the criteria that settle it. Nothing
/// derived is stored here.
///
/// ```
/// use majordomus_cli::intent::IntentRecord;
/// let r = IntentRecord::from_metadata(".ai/repo/project/intents/x.yaml", &serde_json::json!({
///     "id": "x", "title": "X", "statement": "It becomes true.", "milestones": ["m"],
///     "satisfaction": [{"id": "c", "criterion": "c", "evidence": "test", "ref": "t"}],
/// }));
/// assert_eq!(r.id, "x");
/// assert!(!r.cancelled);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntentRecord {
    /// The identity.
    pub id: String,
    /// Repository-relative path of the file.
    pub source: String,
    /// One line.
    pub title: String,
    /// What must become true.
    pub statement: String,
    /// What must stay true.
    pub invariants: Vec<String>,
    /// The criteria.
    pub satisfaction: Vec<CriterionRecord>,
    /// The guards: invariants that name their evidence, judged like a criterion (ADR 0113).
    pub guards: Vec<GuardRecord>,
    /// Milestone ids.
    pub milestones: Vec<String>,
    /// Governance references.
    pub governance: Vec<String>,
    /// What is deliberately not required.
    pub non_goals: Vec<String>,
    /// Abandoned.
    pub cancelled: bool,
    /// The replacement, when there is one.
    pub superseded_by: String,
}

fn text(meta: &Value, key: &str) -> String {
    meta.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn texts(meta: &Value, key: &str) -> Vec<String> {
    meta.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

impl IntentRecord {
    /// The record an object of kind `intent` carries. The schema has already refused a
    /// malformed file by the time the index holds it, so this reads values, never guesses.
    ///
    /// ```
    /// use majordomus_cli::intent::IntentRecord;
    /// use serde_json::json;
    /// let r = IntentRecord::from_metadata(
    ///     ".ai/repo/project/intents/x.yaml",
    ///     &json!({"id": "x", "title": "X", "milestones": ["m"],
    ///             "satisfaction": [{"id": "c", "criterion": "c", "evidence": "test",
    ///                               "ref": "test/cases/1_x.sh"}]}),
    /// );
    /// assert_eq!(r.milestones, ["m"]);
    /// assert_eq!(r.satisfaction[0].reference, "test/cases/1_x.sh");
    /// ```
    pub fn from_metadata(source: &str, meta: &Value) -> IntentRecord {
        let satisfaction = meta
            .get("satisfaction")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|c| CriterionRecord {
                        id: text(c, "id"),
                        criterion: text(c, "criterion"),
                        evidence: text(c, "evidence"),
                        reference: text(c, "ref"),
                        optional: c.get("optional").and_then(Value::as_bool).unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let guards = meta
            .get("guards")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|g| GuardRecord {
                        id: text(g, "id"),
                        invariant: text(g, "invariant"),
                        evidence: text(g, "evidence"),
                        reference: text(g, "ref"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        IntentRecord {
            id: text(meta, "id"),
            source: source.to_string(),
            title: text(meta, "title"),
            statement: text(meta, "statement"),
            invariants: texts(meta, "invariants"),
            guards,
            satisfaction,
            milestones: texts(meta, "milestones"),
            governance: texts(meta, "governance"),
            non_goals: texts(meta, "non_goals"),
            cancelled: meta
                .get("cancelled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            superseded_by: text(meta, "superseded_by"),
        }
    }

    /// Every intent record an index holds, in identity order, so nothing downstream depends
    /// on the order the files were read.
    ///
    /// ```
    /// use majordomus_cli::index::Index;
    /// use majordomus_cli::intent::IntentRecord;
    /// # fn demo(index: &Index) {
    /// let records = IntentRecord::all(index);
    /// assert!(records.windows(2).all(|w| w[0].id <= w[1].id));
    /// # }
    /// ```
    pub fn all(index: &Index) -> Vec<IntentRecord> {
        let by_id: BTreeMap<&str, IntentRecord> = index
            .objects
            .iter()
            .filter(|o| o.kind == INTENT)
            .map(|o| {
                (
                    o.identity.as_str(),
                    IntentRecord::from_metadata(&o.provenance.path, &o.metadata),
                )
            })
            .collect();
        by_id.into_values().collect()
    }
}

// ---------------------------------------------------------------- where evidence comes from

/// What a criterion's or a guard's evidence was judged by: the latest recorded run, and what
/// the evidence module said of it. Derived with the state it explains and stored nowhere
/// (ADR 0113). "Considered" is one run — the latest — because that is the only one the
/// judgement reads.
///
/// ```
/// use majordomus_cli::intent::IntentEvaluation;
/// let e = IntentEvaluation {
///     commit: "abc1234".into(), working_tree: "clean".into(), outcome: "pass".into(),
///     at: "2026-10-08T00:00:00Z".into(), changed: vec!["lib/a.sh".into()],
///     detail: Some("an input changed since the run".into()),
/// };
/// assert_eq!(serde_json::to_value(&e).unwrap()["changed"][0], "lib/a.sh");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct IntentEvaluation {
    /// The commit the run was recorded at.
    pub commit: String,
    /// The state of the working tree it ran over: `clean`, `dirty` or `unknown`.
    pub working_tree: String,
    /// What the run reported: `pass`, `fail`, `skip`, `timeout` or `error`.
    pub outcome: String,
    /// When it was recorded.
    pub at: String,
    /// The inputs of the evidence that changed since the run; empty when none did or the
    /// comparison could not be made.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
    /// The evidence module's own sentence for its judgement, when it gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl IntentEvaluation {
    /// The evaluation of a recorded run under the judgement the evidence module made of it.
    fn of(e: &Execution, changed: &[String], detail: Option<&str>) -> Self {
        IntentEvaluation {
            commit: e.commit.clone(),
            working_tree: e.working_tree.clone(),
            outcome: serde_json::to_value(e.outcome)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            at: e.at.clone(),
            changed: changed.to_vec(),
            detail: detail.map(str::to_string),
        }
    }
}

/// What one test's recorded evidence says: whether its source is in the tree at all, and what
/// its latest recorded run came to. A test that is not there cannot settle anything.
///
/// ```
/// use majordomus_cli::intent::{IntentEvidenceState, TestStanding};
/// let standing = TestStanding {
///     present: false,
///     state: IntentEvidenceState::Unresolved,
///     proof: None,
///     evaluation: None,
/// };
/// assert!(!standing.present);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestStanding {
    /// Whether the test's source is in the tree.
    pub present: bool,
    /// The state of its latest recorded run.
    pub state: IntentEvidenceState,
    /// The evidence module's own verdict behind `state`, when the run was judged by it.
    pub proof: Option<ProofState>,
    /// The run that was judged and what the judgement said of it; `None` when nothing ran.
    pub evaluation: Option<IntentEvaluation>,
}

/// What a declared claim's evidence says: the criterion's state, and the claim report's own
/// verdict behind it — `proven` and `inputs_unchanged` both meet a criterion, and stay two
/// answers.
///
/// ```
/// use majordomus_cli::evidence::ProofState;
/// use majordomus_cli::intent::{ClaimStanding, IntentEvidenceState};
/// let standing = ClaimStanding {
///     state: IntentEvidenceState::Current,
///     proof: ProofState::InputsUnchanged,
///     evaluation: None,
/// };
/// assert_ne!(standing.proof, ProofState::Proven);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimStanding {
    /// What the claim's evidence means for a criterion.
    pub state: IntentEvidenceState,
    /// The claim report's verdict.
    pub proof: ProofState,
    /// The run the claim's proof rests on and what its judgement said; `None` when the
    /// claim's test never ran.
    pub evaluation: Option<IntentEvaluation>,
}

/// The facts the derivation reads beyond the plan. The repository answers them from the
/// ledger, the claim matrix and the tree; a test answers them from a table.
///
/// ```
/// use majordomus_cli::evidence::TestId;
/// use majordomus_cli::intent::{ClaimStanding, EvidenceLookup, IntentEvidenceState, TestStanding};
/// struct NothingRecorded;
/// impl EvidenceLookup for NothingRecorded {
///     fn test(&self, _: &TestId, _: &[String]) -> TestStanding {
///         TestStanding { present: true, state: IntentEvidenceState::NotRun, proof: None, evaluation: None }
///     }
///     fn claim(&self, _: &str) -> Option<ClaimStanding> { None }
///     fn deployment(&self, _: &str) -> bool { false }
///     fn object(&self, _: &str, _: &str) -> bool { false }
///     fn file(&self, _: &str) -> bool { false }
/// }
/// let id = TestId::of("test/cases/00_x.sh").unwrap();
/// assert_eq!(NothingRecorded.test(&id, &[]).state, IntentEvidenceState::NotRun);
/// ```
pub trait EvidenceLookup {
    /// What the ledger holds for one test, by its identity: whether its source is in the tree
    /// and what its latest run came to. `scope` is the code the plan declares for the
    /// criterion — the `scope` of every issue that `serves` it — which the judgement counts
    /// among the test's inputs beside what the claims that test proves name.
    ///
    /// ```
    /// use majordomus_cli::evidence::TestId;
    /// use majordomus_cli::intent::{EvidenceLookup, TestStanding};
    /// # fn demo(ev: &dyn EvidenceLookup) {
    /// let id = TestId::of("test/cases/00_x.sh").unwrap();
    /// let standing: TestStanding = ev.test(&id, &["lib/a".to_string()]);
    /// assert!(standing.present || !standing.present);
    /// # }
    /// ```
    fn test(&self, id: &TestId, scope: &[String]) -> TestStanding;
    /// What a claim's evidence says, judged as [`IntentEvidenceState::judged`] judges it,
    /// with the claim report's verdict behind it; `None` when no such claim is declared —
    /// which is a refusal, not an unproven claim.
    ///
    /// ```
    /// use majordomus_cli::intent::EvidenceLookup;
    /// # fn demo(ev: &dyn EvidenceLookup) {
    /// assert!(ev.claim("no-such-claim-exists").is_none());
    /// # }
    /// ```
    fn claim(&self, id: &str) -> Option<ClaimStanding>;
    /// Whether a deployment object with this id exists. A `deployment` criterion resolves
    /// through this and is still never counted as met.
    ///
    /// ```
    /// use majordomus_cli::intent::EvidenceLookup;
    /// # fn demo(ev: &dyn EvidenceLookup) {
    /// assert!(!ev.deployment("no-such-deployment"));
    /// # }
    /// ```
    fn deployment(&self, id: &str) -> bool;
    /// Whether an object of `kind` answers to `identity` (`rule` matches with or without
    /// its `@version`). This is what makes a `governance:` entry resolve or fail.
    ///
    /// ```
    /// use majordomus_cli::intent::EvidenceLookup;
    /// # fn demo(ev: &dyn EvidenceLookup) {
    /// assert!(!ev.object("rule", "project.no-such-rule"));
    /// # }
    /// ```
    fn object(&self, kind: &str, identity: &str) -> bool;
    /// Whether a repository-relative file exists, for a `file:` governance entry and for the
    /// test sources a criterion names.
    ///
    /// ```
    /// use majordomus_cli::intent::EvidenceLookup;
    /// # fn demo(ev: &dyn EvidenceLookup) {
    /// assert!(!ev.file("no/such/file.md"));
    /// # }
    /// ```
    fn file(&self, path: &str) -> bool;
}

/// The evidence of a real repository: its ledger, its claim matrix joined to that ledger,
/// its index and its tree, judged at the working tree by the evidence module's own
/// derivation — [`evidence::report`] for a claim, [`evidence::freshness()`] over the same
/// [`evidence::compare`] for a test — so a criterion is current exactly when the rest of
/// Majordomus would call its evidence current. The claim join and the comparisons run git,
/// so each is computed once, and only when a criterion or a governance entry asks.
///
/// ```
/// use majordomus_cli::index::Index;
/// use majordomus_cli::intent::{EvidenceLookup, RepositoryEvidence};
/// # fn demo(index: &Index) {
/// let evidence = RepositoryEvidence::load(index).expect("the ledger is readable");
/// // a file the repository does not hold is answered without running git
/// assert!(!evidence.file("no/such/file.md"));
/// # }
/// ```
pub struct RepositoryEvidence<'a> {
    index: &'a Index,
    root: PathBuf,
    ledger: Ledger,
    claims: std::cell::OnceCell<EvidenceReport>,
    comparisons: std::cell::RefCell<BTreeMap<String, Comparison>>,
}

impl<'a> RepositoryEvidence<'a> {
    /// The evidence of the repository an index was built from. A ledger this executable
    /// cannot read is an error, never an empty ledger: a misread one could report a pass
    /// that was never recorded.
    ///
    /// ```
    /// use majordomus_cli::index::Index;
    /// use majordomus_cli::intent::RepositoryEvidence;
    /// # fn demo(index: &Index) {
    /// // a ledger that cannot be read is an error here, never an empty ledger
    /// let evidence = RepositoryEvidence::load(index).expect("the ledger is readable");
    /// let _ = &evidence;
    /// # }
    /// ```
    pub fn load(index: &'a Index) -> crate::error::Result<RepositoryEvidence<'a>> {
        let root = PathBuf::from(&index.repository.root);
        let ledger = Ledger::load(&root)?;
        Ok(RepositoryEvidence {
            index,
            root,
            ledger,
            claims: std::cell::OnceCell::new(),
            comparisons: std::cell::RefCell::new(BTreeMap::new()),
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    /// The claim join at the working tree, computed once.
    fn claims(&self) -> &EvidenceReport {
        self.claims
            .get_or_init(|| evidence::report(self.index, &self.ledger))
    }

    /// The comparison of one evidence commit with the working tree, computed once per commit.
    fn comparison(&self, commit: &str) -> Comparison {
        self.comparisons
            .borrow_mut()
            .entry(commit.to_string())
            .or_insert_with(|| compare(self.root(), commit, &Presented::WorkingTree))
            .clone()
    }

    /// What a test criterion names: the test's own source; the source and implementation of
    /// every claim that same test proves; and every path, among those `changed` since the run,
    /// under the `scope` of an issue serving the criterion — the code under test, as the
    /// repository declares it. A criterion names nothing else, so a change elsewhere, the
    /// plan's own records included, does not move it.
    ///
    /// `None` when no claim that test proves names an implementation and no issue serving the
    /// criterion declares a scope: the repository then declares nothing about the code under
    /// test, and the test's own file is not that code. The truth table reads `None` as a route
    /// that names no inputs (its row 14), so any change since the run, the code under test
    /// included, leaves the pass stale rather than current; a run at the checkout's own commit
    /// is still `proven`.
    fn inputs(
        &self,
        id: &TestId,
        scope: &[String],
        changed: Option<&BTreeSet<String>>,
    ) -> Option<Vec<String>> {
        let test = id.as_string();
        let mut inputs = vec![id.source()];
        let mut declared = !scope.is_empty();
        let mut push = |p: &String| {
            if !inputs.contains(p) {
                inputs.push(p.clone());
            }
        };
        for c in &self.claims().claims {
            if c.test.as_deref() == Some(test.as_str()) {
                declared |= c.implementation.is_some();
                [&c.source, &c.implementation]
                    .into_iter()
                    .flatten()
                    .for_each(&mut push);
            }
        }
        changed
            .into_iter()
            .flatten()
            .filter(|p| scope.iter().any(|s| within(p, s)))
            .for_each(&mut push);
        declared.then_some(inputs)
    }
}

/// Where the plan keeps its records: a change there is a change to the plan, never to the code a
/// criterion's test exercises.
const PLAN_RECORDS: &str = ".ai/repo/project";

impl EvidenceLookup for RepositoryEvidence<'_> {
    fn test(&self, id: &TestId, scope: &[String]) -> TestStanding {
        let source = id.source();
        let present = self.root().join(&source).is_file();
        let execution = self.ledger.latest(&id.as_string());
        let Some(e) = execution else {
            return TestStanding {
                present,
                state: IntentEvidenceState::NotRun,
                proof: Some(ProofState::NotRun),
                evaluation: None,
            };
        };
        // the claim report's own reading of a run: a test that no longer hashes to what ran
        // did not run in the form it is in now, whatever the diff says
        let moved = e.outcome.proves() && e.digest_matches(self.root()) == Some(false);
        // the plan's own records are not the code under test: closing the work a criterion is
        // served by is a change to `.ai/repo/project/`, and it must not stale the pass that
        // proves the criterion, whether or not the repository declares the code under test
        let mut comparison = self.comparison(&e.commit);
        // a comparison git could not make has no list to filter, and stays unknown
        comparison
            .changed
            .iter_mut()
            .for_each(|changed| changed.retain(|p| !within(p, PLAN_RECORDS)));
        let inputs = self.inputs(id, scope, comparison.changed.as_ref());
        let judgement = freshness(
            Recorded::Ran(e),
            inputs.as_deref(),
            &source,
            moved,
            Some(&comparison),
            Presented::WorkingTree.tree(),
        );
        TestStanding {
            present,
            state: IntentEvidenceState::judged(judgement.state, TreeState::parse(&e.working_tree)),
            proof: Some(judgement.state),
            evaluation: Some(IntentEvaluation::of(
                e,
                &judgement.changed,
                judgement.detail.as_deref(),
            )),
        }
    }

    fn claim(&self, id: &str) -> Option<ClaimStanding> {
        let c = self.claims().claims.iter().find(|c| c.id == id)?;
        let recorded = c
            .execution
            .as_ref()
            .map_or(TreeState::Unknown, |e| TreeState::parse(&e.working_tree));
        Some(ClaimStanding {
            state: IntentEvidenceState::judged(c.state, recorded),
            proof: c.state,
            evaluation: c
                .execution
                .as_ref()
                .map(|e| IntentEvaluation::of(e, &c.changed, c.detail.as_deref())),
        })
    }

    fn deployment(&self, id: &str) -> bool {
        self.object("deployment", id)
    }

    fn object(&self, kind: &str, identity: &str) -> bool {
        self.index.objects.iter().any(|o| {
            o.kind == kind
                && (o.identity == identity
                    || (kind == "rule"
                        && o.identity
                            .strip_prefix(identity)
                            .is_some_and(|rest| rest.starts_with('@'))))
        })
    }

    fn file(&self, path: &str) -> bool {
        !path.contains("..") && self.root().join(path).exists()
    }
}

// ---------------------------------------------------------------- the derived model

/// One milestone an intent names, as the plan derives it. A milestone that does not resolve is
/// carried with `resolved: false` rather than dropped, because a name that points at nothing is
/// a finding and not an absence.
///
/// ```
/// use majordomus_cli::intent::IntentMilestone;
/// let named = IntentMilestone { id: "no-such".into(), resolved: false, status: None };
/// assert!(!named.resolved);
/// assert!(named.status.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentMilestone {
    /// The id the intent names.
    pub id: String,
    /// Whether the plan holds a milestone with that id.
    pub resolved: bool,
    /// The status the plan derives for it; absent when it does not resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// One criterion with the evidence behind it: what the record declared, what the ledger says
/// about it, whether that meets it, and the command that would produce the evidence again.
///
/// ```
/// use majordomus_cli::evidence::ProofState;
/// use majordomus_cli::intent::{IntentCriterion, IntentEvidenceState};
/// let c = IntentCriterion {
///     id: "stage-is-derived".into(),
///     criterion: "the stage follows the plan".into(),
///     evidence: "test".into(),
///     reference: "apps/majordomus-cli/tests/intent.rs".into(),
///     state: IntentEvidenceState::NotRun,
///     met: false,
///     proof: Some(ProofState::NotRun),
///     reproduce: None,
///     optional: false,
///     evaluation: None,
/// };
/// // nothing recorded is not a pass
/// assert!(!c.met);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentCriterion {
    /// Unique within the intent.
    pub id: String,
    /// What is observably true when it is met.
    pub criterion: String,
    /// `test`, `claim`, `command` or `deployment`.
    pub evidence: String,
    /// What the evidence names, as authored.
    #[serde(rename = "ref")]
    pub reference: String,
    /// Derived: what the evidence says.
    pub state: IntentEvidenceState,
    /// Derived: the state is `current`.
    pub met: bool,
    /// Derived: the evidence module's own verdict behind `state`, for a `test` or `claim`
    /// criterion whose reference resolves. A met criterion rests on `proven` (a pass at this
    /// revision) or `inputs_unchanged` (a pass whose named inputs have not moved since, which
    /// is not proof at this revision), and every surface shows which, so `met` is never the
    /// one tick for both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<ProofState>,
    /// The command that produces the evidence again, when one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
    /// Whether the intent declares it optional: shown and evaluated like any other, and
    /// holding neither the verdict nor the satisfied stage back. Absent when it is required.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// The run its evidence was judged by, and what the judgement said; absent when nothing
    /// was judged — the evidence never ran, or is of a kind the ledger does not record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<IntentEvaluation>,
}

/// A guard, judged: an invariant with the state of the evidence that would fail if it
/// stopped being true (ADR 0113). `violated` is true for a failing run and for nothing
/// else: a guard that is stale, was never run or does not resolve is not judged.
///
/// ```
/// use majordomus_cli::intent::{IntentEvidenceState, IntentGuard};
/// let g = IntentGuard {
///     id: "engines-agree".into(), invariant: "The plan engines stay identical".into(),
///     evidence: "test".into(), reference: "test/cases/99_plan_capabilities.sh".into(),
///     state: IntentEvidenceState::Stale, violated: false, reproduce: None, evaluation: None,
/// };
/// assert!(!g.violated, "stale is not judged, and violates nothing");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentGuard {
    /// Unique within the intent, and never a criterion's id.
    pub id: String,
    /// What must stay true.
    pub invariant: String,
    /// `test` or `claim`.
    pub evidence: String,
    /// The test or the claim.
    #[serde(rename = "ref")]
    pub reference: String,
    /// The state of its evidence now.
    pub state: IntentEvidenceState,
    /// Whether it is violated: its evidence is failing.
    pub violated: bool,
    /// The command that runs its evidence, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
    /// The run it was judged by; absent when nothing was judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<IntentEvaluation>,
}

/// One intent, as its record declares it and as the plan and the ledger derive it: the
/// statement and invariants as authored, each milestone with its derived status, each criterion
/// with the state of its evidence, the stage that follows from both, and the verdict that
/// follows from the evidence alone.
///
/// ```
/// use majordomus_cli::intent::{verdict, IntentStage, IntentVerdictState, IntentView};
/// let view = IntentView {
///     id: "intent-lifecycle".into(),
///     title: "Work is traceable to the intent it serves".into(),
///     statement: "A reader can ask what work is for.".into(),
///     invariants: vec!["no intent status is stored".into()],
///     stage: IntentStage::Declared,
///     milestones: vec![],
///     satisfaction: vec![],
///     met: 0,
///     optional: 0,
///     guards: vec![],
///     verdict: verdict(&[]),
///     governance: vec!["adr:adr-0070".into()],
///     non_goals: vec![],
///     superseded_by: None,
///     source: ".ai/repo/project/intents/intent-lifecycle.yaml".into(),
/// };
/// // a record naming no milestone that resolves has not been planned
/// assert_eq!(view.stage, IntentStage::Declared);
/// // and one declaring no criterion cannot be settled by evidence
/// assert_eq!(view.verdict.state, IntentVerdictState::Unknown);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentView {
    /// The identity, which is also the file name.
    pub id: String,
    /// One line.
    pub title: String,
    /// What must become true.
    pub statement: String,
    /// What must stay true.
    pub invariants: Vec<String>,
    /// Derived: the stage.
    pub stage: IntentStage,
    /// The milestones, each with its derived status.
    pub milestones: Vec<IntentMilestone>,
    /// The criteria, each with its evidence state.
    pub satisfaction: Vec<IntentCriterion>,
    /// Derived: how many criteria are met.
    pub met: usize,
    /// How many criteria are optional; `met` and the verdict are over the others. Absent
    /// when none is.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub optional: usize,
    /// The guards, each judged. Absent when the intent declares none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<IntentGuard>,
    /// Derived from the criteria alone, never from the plan: whether the evidence settles the
    /// intent, and which criteria hold it back.
    pub verdict: IntentVerdict,
    /// The governance a worker on this intent loads.
    pub governance: Vec<String>,
    /// What is deliberately not required.
    pub non_goals: Vec<String>,
    /// The replacement, when the record names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    /// Repository-relative path of the record.
    pub source: String,
}

/// One validation finding: its level, a stable code a reader greps for, the intent or milestone
/// it is about, one line of what is wrong, and the command that shows it again.
///
/// ```
/// use majordomus_cli::intent::{IntentFinding, FAIL};
/// let f = IntentFinding {
///     level: FAIL.into(),
///     code: "unknown_milestone".into(),
///     subject: "intent-lifecycle".into(),
///     message: "`no-such` is not a milestone".into(),
///     reproduce: "majordomus intent validate".into(),
/// };
/// assert_eq!(f.level, "FAIL");
/// assert_eq!(f.reproduce, "majordomus intent validate");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentFinding {
    /// `FAIL` or `WARN`. A failure means the intent model is invalid.
    pub level: String,
    /// The stable code a reader greps for.
    pub code: String,
    /// The intent or milestone the finding is about.
    pub subject: String,
    /// What is wrong, in one line.
    pub message: String,
    /// The command that shows it again.
    pub reproduce: String,
}

/// Every intent, derived, with every finding: what `majordomus intent validate` answers out of,
/// and what the list, the record and the preflight are read from. Nothing here is stored.
///
/// ```
/// use majordomus_cli::intent::Intents;
/// let empty = Intents { intents: vec![], findings: vec![] };
/// // a repository that declares no intent is valid, and says so
/// assert_eq!(empty.failures(), 0);
/// assert!(empty.intent("intent-lifecycle").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Intents {
    /// Every intent, in identity order.
    pub intents: Vec<IntentView>,
    /// Every finding: per intent in identity order, then the milestones no intent serves.
    pub findings: Vec<IntentFinding>,
}

/// The command that shows every intent finding again.
pub const REPRODUCE: &str = "majordomus intent validate";

fn finding(out: &mut Vec<IntentFinding>, level: &str, code: &str, subject: &str, msg: String) {
    out.push(IntentFinding {
        level: level.into(),
        code: code.into(),
        subject: subject.into(),
        message: msg,
        reproduce: REPRODUCE.into(),
    });
}

/// The milestone statuses that mean nothing is required of the milestone any more.
fn retired(status: &str) -> bool {
    matches!(status, "CANCELLED" | "SUPERSEDED")
}

/// The stage of an intent from the facts it rests on. Pure: no index, no ledger.
///
/// ```
/// use majordomus_cli::intent::{stage, IntentStage};
/// let st = |s: &str| Some(s.to_string());
/// assert_eq!(stage(false, false, &[None], true), IntentStage::Declared);
/// assert_eq!(stage(false, false, &[st("PLANNED")], true), IntentStage::Planned);
/// assert_eq!(stage(false, false, &[st("ACTIVE"), st("PLANNED")], true), IntentStage::Executing);
/// assert_eq!(stage(false, false, &[st("DONE")], false), IntentStage::Verifying);
/// assert_eq!(stage(false, false, &[st("DONE")], true), IntentStage::Satisfied);
/// ```
pub fn stage(
    cancelled: bool,
    superseded: bool,
    milestones: &[Option<String>],
    all_met: bool,
) -> IntentStage {
    if cancelled {
        return IntentStage::Cancelled;
    }
    if superseded {
        return IntentStage::Superseded;
    }
    if milestones.is_empty() || milestones.iter().any(Option::is_none) {
        return IntentStage::Declared;
    }
    let statuses: Vec<&str> = milestones.iter().flatten().map(String::as_str).collect();
    let counted: Vec<&str> = statuses.iter().copied().filter(|s| !retired(s)).collect();
    if !counted.is_empty() && counted.iter().all(|s| *s == "DONE") {
        return if all_met {
            IntentStage::Satisfied
        } else {
            IntentStage::Verifying
        };
    }
    if counted
        .iter()
        .any(|s| matches!(*s, "ACTIVE" | "VERIFY" | "DONE"))
    {
        return IntentStage::Executing;
    }
    IntentStage::Planned
}

fn criterion_of(
    intent: &str,
    c: &CriterionRecord,
    scope: &[String],
    ev: &dyn EvidenceLookup,
    findings: &mut Vec<IntentFinding>,
) -> IntentCriterion {
    let r = c.reference.trim();
    let mut reproduce = None;
    let mut proof = None;
    let mut evaluation = None;
    let unresolved = |findings: &mut Vec<IntentFinding>, why: String| {
        finding(
            findings,
            FAIL,
            "unresolved_evidence_ref",
            intent,
            format!("criterion `{}`: {why}", c.id),
        );
        IntentEvidenceState::Unresolved
    };
    let state = if r.is_empty() {
        finding(
            findings,
            FAIL,
            "criterion_without_ref",
            intent,
            format!(
                "criterion `{}` names no {} evidence, so nothing could ever settle it",
                c.id, c.evidence
            ),
        );
        IntentEvidenceState::Unresolved
    } else {
        match c.evidence.as_str() {
            "test" => match test_id(r) {
                None => unresolved(
                    findings,
                    format!(
                        "`{r}` names no test: give a path under test/cases/ or \
                         apps/majordomus-cli/tests/, or `suite:<case>` / `crate:<binary>`"
                    ),
                ),
                Some(id) => {
                    let standing = ev.test(&id, scope);
                    if standing.present {
                        reproduce = Some(id.reproduce());
                        proof = standing.proof;
                        evaluation = standing.evaluation;
                        standing.state
                    } else {
                        unresolved(
                            findings,
                            format!("the test `{r}` is not in the tree ({})", id.source()),
                        )
                    }
                }
            },
            "claim" => match ev.claim(r) {
                None => unresolved(
                    findings,
                    format!("`{r}` is not a claim of docs/CLAIMS.yaml"),
                ),
                Some(standing) => {
                    proof = Some(standing.proof);
                    evaluation = standing.evaluation;
                    standing.state
                }
            },
            "deployment" => {
                if ev.deployment(r) {
                    IntentEvidenceState::NotDerivable
                } else {
                    unresolved(
                        findings,
                        format!("`{r}` is not a deployment of this repository"),
                    )
                }
            }
            // `command`: the schema admits nothing else
            _ => {
                reproduce = Some(r.to_string());
                IntentEvidenceState::NotDerivable
            }
        }
    };
    IntentCriterion {
        id: c.id.clone(),
        criterion: c.criterion.clone(),
        evidence: c.evidence.clone(),
        reference: c.reference.clone(),
        met: state == IntentEvidenceState::Current,
        state,
        proof,
        reproduce,
        optional: c.optional,
        evaluation,
    }
}

/// The code the plan declares for one criterion: the `scope` of every issue that `serves`
/// `<intent>#<criterion>`, in plan order, each path once.
fn serving_scope(plan: &Plan, intent: &str, criterion: &str) -> Vec<String> {
    let link = format!("{intent}#{criterion}");
    let mut scope: Vec<String> = Vec::new();
    for i in plan
        .issues
        .iter()
        .filter(|i| i.serves.iter().any(|s| s.trim() == link))
    {
        for p in &i.scope {
            let p = p.trim().trim_end_matches('/').to_string();
            if !p.is_empty() && !scope.contains(&p) {
                scope.push(p);
            }
        }
    }
    scope
}

/// Whether a repository path is a declared scope entry or lies under it; `.` is the whole tree.
fn within(path: &str, scope: &str) -> bool {
    scope == "."
        || path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn test_id(r: &str) -> Option<TestId> {
    TestId::of(r).or_else(|| {
        let (prefix, name) = r.split_once(':')?;
        let runner = match prefix {
            "suite" => evidence::Runner::Suite,
            "crate" => evidence::Runner::Crate,
            _ => return None,
        };
        TestId::named(runner, name)
    })
}

fn governance_resolves(g: &str, ev: &dyn EvidenceLookup) -> bool {
    let Some((kind, what)) = g.split_once(':') else {
        return false;
    };
    match kind {
        "rule" => ev.object("rule", what),
        "adr" => ev.object("adr", what),
        "claim" => ev.claim(what).is_some(),
        "file" => ev.file(what),
        _ => false,
    }
}

/// `gap_observation_contradicted` (ADR 0117): a required criterion of a live intent that a gap
/// observed satisfied, that no live issue serves — so coverage counts it `observed` and owes no
/// work for it — and whose evidence now fails. The one answer of what remains that no other
/// finding reports; a warning, because the change that broke the test is not the change that
/// planned around it.
///
/// ```
/// use majordomus_cli::intent::{contradicted_observations, Intents};
/// use majordomus_cli::intent_plan::IntentCoverage;
/// let none = Intents { intents: vec![], findings: vec![] };
/// let coverage = IntentCoverage { criteria: vec![], issues: vec![], findings: vec![] };
/// assert!(contradicted_observations(&none, &coverage).is_empty());
/// ```
pub fn contradicted_observations(
    intents: &Intents,
    coverage: &crate::intent_plan::IntentCoverage,
) -> Vec<IntentFinding> {
    use crate::intent_remains::{remains, Basis, Remains};
    let mut out = Vec::new();
    for view in &intents.intents {
        if matches!(view.stage, IntentStage::Cancelled | IntentStage::Superseded) {
            continue;
        }
        for c in view.satisfaction.iter().filter(|c| !c.met && !c.optional) {
            let Some(entry) = coverage
                .criteria
                .iter()
                .find(|e| e.intent == view.id && e.criterion == c.id)
            else {
                continue;
            };
            if remains(c.state, &entry.issues, entry.strength)
                == (Remains::Failed, Basis::GapObservation)
            {
                finding(
                    &mut out,
                    WARN,
                    "gap_observation_contradicted",
                    &format!("{}#{}", view.id, c.id),
                    "a gap observed it satisfied, so no issue serves it, and its evidence now \
                     fails: the plan rests on an observation that no longer holds; plan an issue \
                     that serves it"
                        .into(),
                );
            }
        }
    }
    out
}

/// What the planning half needs of each record: its criteria, its milestones, whether it is
/// retired, and the criteria its recorded gap observed already satisfied.
///
/// ```
/// use majordomus_cli::intent::{outlines, IntentRecord};
/// use serde_json::json;
/// let r = IntentRecord::from_metadata(".ai/repo/project/intents/x.yaml", &json!({
///     "id": "x", "milestones": ["m"],
///     "satisfaction": [{"id": "c", "evidence": "test", "ref": "t"}]}));
/// let o = outlines(std::slice::from_ref(&r), &[]);
/// assert_eq!(o[0].criteria, ["c"]);
/// assert!(!o[0].retired);
/// ```
pub fn outlines(records: &[IntentRecord], gaps: &[GapRecord]) -> Vec<IntentOutline> {
    let observed = observed_satisfied(gaps);
    records
        .iter()
        .map(|r| IntentOutline {
            id: r.id.clone(),
            criteria: r.satisfaction.iter().map(|c| c.id.clone()).collect(),
            milestones: r.milestones.clone(),
            retired: r.cancelled || !r.superseded_by.is_empty(),
            observed_satisfied: observed.get(&r.id).cloned().unwrap_or_default(),
            optional: r
                .satisfaction
                .iter()
                .filter(|c| c.optional)
                .map(|c| c.id.clone())
                .collect(),
        })
        .collect()
}

impl Intents {
    /// Derive every intent of an index against its plan and its evidence. Nothing is read
    /// but what the three already hold, and nothing is written.
    ///
    /// The findings are the record's own (this module) followed by the planning half's
    /// (ADR 0073): whether the plan carries every criterion, why each issue exists, and
    /// whether the recorded gap and critique hold and were made before execution.
    ///
    /// ```
    /// use majordomus_cli::index::Index;
    /// use majordomus_cli::intent::{Intents, RepositoryEvidence};
    /// use majordomus_cli::plan::Plan;
    /// # fn demo(index: &Index) {
    /// let plan = Plan::build(index);
    /// let evidence = RepositoryEvidence::load(index).expect("the ledger is readable");
    /// let intents = Intents::build(index, &plan, &evidence);
    /// // the record's findings, then the coverage of its criteria by the plan
    /// assert!(intents.failures() >= intents.findings.iter().filter(|f| f.level == "FAIL").count());
    /// # }
    /// ```
    pub fn build(index: &Index, plan: &Plan, ev: &dyn EvidenceLookup) -> Intents {
        let records = IntentRecord::all(index);
        let gaps = GapRecord::all(index);
        let critiques = CritiqueRecord::all(index);
        let outlines = outlines(&records, &gaps);
        let mut out = Intents::derive(records, plan, ev);
        let covered = coverage(&outlines, plan);
        let contradicted = contradicted_observations(&out, &covered);
        out.findings.extend(covered.findings);
        out.findings.extend(contradicted);
        out.findings
            .extend(review(&outlines, plan, &gaps, &critiques));
        // the stamp of each critique against the plan as it now stands (ADR 0112): a review
        // of another plan, and a review nobody ran, are failures where the policy requires
        // opposition and warnings where it does not
        let required = crate::intent_binding::policy_of(index)
            .opposition
            .is_required();
        let level = if required { FAIL } else { WARN };
        let stamps: Vec<IntentFinding> = critiques
            .iter()
            .filter(|c| {
                out.intent(&c.intent).is_some_and(|i| {
                    !matches!(i.stage, IntentStage::Cancelled | IntentStage::Superseded)
                })
            })
            .filter_map(|c| {
                crate::intent_opposition::revision_of(&out, plan, &gaps, &c.intent)
                    .map(|now| (c, now))
            })
            .filter_map(|(c, now)| {
                let (code, message) = if c.reviewed_revision.is_empty() {
                    (
                        "critique_not_stamped",
                        format!(
                            "its critique carries no stamp, so nothing says which plan it reviewed; run `majordomus-cli intent stamp {}`",
                            c.intent
                        ),
                    )
                } else if c.reviewed_revision != now {
                    (
                        "critique_stale",
                        format!(
                            "its critique reviewed another plan (stamped {}, the plan is now {}); review it again and run `majordomus-cli intent stamp {}`",
                            c.reviewed_revision.chars().take(12).collect::<String>(),
                            now.chars().take(12).collect::<String>(),
                            c.intent
                        ),
                    )
                } else {
                    return None;
                };
                Some(IntentFinding {
                    level: level.into(),
                    code: code.into(),
                    subject: c.intent.clone(),
                    message,
                    reproduce: "majordomus intent validate".into(),
                })
            })
            .collect();
        out.findings.extend(stamps);
        // where opposition is required, a resolution says who made it: a blocking finding
        // dismissed or planned by nobody in particular is a required change nobody accepted
        if required {
            let unnamed: Vec<IntentFinding> = critiques
                .iter()
                .flat_map(|c| c.findings.iter().map(move |x| (c, x)))
                .filter(|(_, x)| {
                    matches!(
                        x.resolution,
                        Some(ResolutionState::Planned | ResolutionState::Rejected)
                    ) && x.resolved_by.trim().is_empty()
                })
                .map(|(c, x)| IntentFinding {
                    level: FAIL.into(),
                    code: "resolution_names_no_resolver".into(),
                    subject: format!("{}:{}", c.intent, x.id),
                    message: format!(
                        "is {} and names nobody who resolved it; add resolved_by to its resolution",
                        x.resolution_text
                    ),
                    reproduce: "majordomus intent validate".into(),
                })
                .collect();
            out.findings.extend(unnamed);
        }
        out
    }

    /// The coverage of every criterion by the plan, and the reason every issue exists: the
    /// same derivation [`Intents::build`] reports findings from, answered on its own so a
    /// reader can ask which work carries which criterion (ADR 0073).
    ///
    /// ```
    /// use majordomus_cli::index::Index;
    /// use majordomus_cli::intent::Intents;
    /// use majordomus_cli::plan::Plan;
    /// # fn demo(index: &Index) {
    /// let plan = Plan::build(index);
    /// let coverage = Intents::coverage(index, &plan);
    /// // every issue of the plan answers why it exists, including the maintenance ones
    /// assert_eq!(coverage.issues.len(), plan.issues.len());
    /// # }
    /// ```
    pub fn coverage(index: &Index, plan: &Plan) -> IntentCoverage {
        let records = IntentRecord::all(index);
        coverage(&outlines(&records, &GapRecord::all(index)), plan)
    }

    /// Derive from records already read: what [`Intents::build`] does after reading them,
    /// and what a test calls with records it wrote.
    ///
    /// ```
    /// use majordomus_cli::evidence::TestId;
    /// use majordomus_cli::intent::{
    ///     ClaimStanding, EvidenceLookup, IntentEvidenceState, IntentRecord, Intents,
    ///     TestStanding,
    /// };
    /// use majordomus_cli::plan::Plan;
    /// struct NothingRecorded;
    /// impl EvidenceLookup for NothingRecorded {
    ///     fn test(&self, _: &TestId, _: &[String]) -> TestStanding {
    ///         TestStanding { present: true, state: IntentEvidenceState::NotRun, proof: None, evaluation: None }
    ///     }
    ///     fn claim(&self, _: &str) -> Option<ClaimStanding> { None }
    ///     fn deployment(&self, _: &str) -> bool { false }
    ///     fn object(&self, _: &str, _: &str) -> bool { false }
    ///     fn file(&self, _: &str) -> bool { false }
    /// }
    /// # let plan: Plan = serde_json::from_value(serde_json::json!({
    /// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
    /// #                 "active_milestone": ""},
    /// #     "statuses": {"issue": [], "milestone": []},
    /// #     "milestones": [], "issues": [], "waves": [], "edges": [],
    /// #     "milestone_edges": [], "findings": []})).unwrap();
    /// let record = IntentRecord::from_metadata(
    ///     ".ai/repo/project/intents/x.yaml",
    ///     &serde_json::json!({"id": "x", "title": "X", "milestones": ["absent"],
    ///         "satisfaction": [{"id": "c", "criterion": "c", "evidence": "test",
    ///                           "ref": "test/cases/00_x.sh"}]}),
    /// );
    /// let intents = Intents::derive(vec![record], &plan, &NothingRecorded);
    /// // the milestone it names is not in the plan, and that is a failure by name
    /// assert!(intents.findings.iter().any(|f| f.code == "unknown_milestone"));
    /// ```
    pub fn derive(records: Vec<IntentRecord>, plan: &Plan, ev: &dyn EvidenceLookup) -> Intents {
        let mut findings = Vec::new();
        let ids: BTreeSet<&str> = records.iter().map(|r| r.id.as_str()).collect();
        let mut served: BTreeSet<String> = BTreeSet::new();
        let mut intents = Vec::with_capacity(records.len());

        for r in &records {
            let expected = format!("{INTENTS_DIR}/{}.yaml", r.id);
            if !r.source.is_empty() && r.source != expected {
                finding(
                    &mut findings,
                    FAIL,
                    "file_name_mismatch",
                    &r.id,
                    format!("the record is at {} and its id says {expected}", r.source),
                );
            }
            if r.milestones.is_empty() {
                finding(
                    &mut findings,
                    FAIL,
                    "intent_without_milestone",
                    &r.id,
                    "the intent names no milestone, so nothing realises it".into(),
                );
            }
            let milestones: Vec<IntentMilestone> = r
                .milestones
                .iter()
                .map(|m| {
                    served.insert(m.clone());
                    let status = plan.milestone(m).map(|pm| pm.status.clone());
                    if status.is_none() {
                        finding(
                            &mut findings,
                            FAIL,
                            "unknown_milestone",
                            &r.id,
                            format!("`{m}` is not a milestone under .ai/repo/project/milestones/"),
                        );
                    }
                    IntentMilestone {
                        id: m.clone(),
                        resolved: status.is_some(),
                        status,
                    }
                })
                .collect();

            let mut seen = BTreeSet::new();
            for c in &r.satisfaction {
                if !seen.insert(c.id.as_str()) {
                    finding(
                        &mut findings,
                        FAIL,
                        "duplicate_criterion",
                        &r.id,
                        format!("criterion `{}` is declared twice", c.id),
                    );
                }
            }
            if r.satisfaction.is_empty() {
                finding(
                    &mut findings,
                    FAIL,
                    "intent_without_criterion",
                    &r.id,
                    "the intent declares no satisfaction criterion, so it can never be satisfied"
                        .into(),
                );
            }
            let satisfaction: Vec<IntentCriterion> = r
                .satisfaction
                .iter()
                .map(|c| {
                    let scope = serving_scope(plan, &r.id, &c.id);
                    criterion_of(&r.id, c, &scope, ev, &mut findings)
                })
                .collect();

            for g in &r.governance {
                if !governance_resolves(g, ev) {
                    finding(
                        &mut findings,
                        FAIL,
                        "unresolved_governance",
                        &r.id,
                        format!("governance `{g}` names nothing this repository holds"),
                    );
                }
            }
            if !r.superseded_by.is_empty() && !ids.contains(r.superseded_by.as_str()) {
                finding(
                    &mut findings,
                    FAIL,
                    "unknown_successor",
                    &r.id,
                    format!("superseded_by `{}` is not an intent", r.superseded_by),
                );
            }

            // a guard is judged by the function that judges a criterion, over no scope: nothing
            // serves a guard, so its evidence has no declared inputs of the plan's
            let guards: Vec<IntentGuard> = r
                .guards
                .iter()
                .map(|g| {
                    if !seen.insert(g.id.as_str()) {
                        finding(
                            &mut findings,
                            FAIL,
                            "duplicate_guard",
                            &r.id,
                            format!(
                                "guard `{}` repeats the id of a criterion or of another guard",
                                g.id
                            ),
                        );
                    }
                    let judged = criterion_of(
                        &r.id,
                        &CriterionRecord {
                            id: g.id.clone(),
                            criterion: g.invariant.clone(),
                            evidence: g.evidence.clone(),
                            reference: g.reference.clone(),
                            optional: false,
                        },
                        &[],
                        ev,
                        &mut findings,
                    );
                    IntentGuard {
                        id: judged.id,
                        invariant: judged.criterion,
                        evidence: judged.evidence,
                        reference: judged.reference,
                        violated: judged.state == IntentEvidenceState::Failing,
                        state: judged.state,
                        reproduce: judged.reproduce,
                        evaluation: judged.evaluation,
                    }
                })
                .collect();
            let optional = satisfaction.iter().filter(|c| c.optional).count();
            if !satisfaction.is_empty() && optional == satisfaction.len() {
                finding(
                    &mut findings,
                    FAIL,
                    "intent_without_required_criterion",
                    &r.id,
                    "every criterion is optional, so nothing the intent requires could ever be met"
                        .into(),
                );
            }
            let met = satisfaction.iter().filter(|c| !c.optional && c.met).count();
            // one verdict, and the stage reads it: whether the evidence is satisfied is
            // decided in one place (ADR 0113)
            let verdict = verdict_with(&satisfaction, &guards);
            let all_met = verdict.state == IntentVerdictState::Satisfied;
            let statuses: Vec<Option<String>> =
                milestones.iter().map(|m| m.status.clone()).collect();
            intents.push(IntentView {
                id: r.id.clone(),
                title: r.title.clone(),
                statement: r.statement.clone(),
                invariants: r.invariants.clone(),
                stage: stage(r.cancelled, !r.superseded_by.is_empty(), &statuses, all_met),
                milestones,
                verdict,
                satisfaction,
                met,
                optional,
                guards,
                governance: r.governance.clone(),
                non_goals: r.non_goals.clone(),
                superseded_by: (!r.superseded_by.is_empty()).then(|| r.superseded_by.clone()),
                source: r.source.clone(),
            });
        }

        for m in &plan.milestones {
            if !served.contains(&m.id) && !retired(&m.status) {
                finding(
                    &mut findings,
                    WARN,
                    "milestone_serves_no_intent",
                    &m.id,
                    format!("the milestone is {} and no intent names it", m.status),
                );
            }
        }
        Intents { intents, findings }
    }

    /// One intent by id, or `None` when this repository declares no such intent — which the
    /// capability turns into a refusal naming what it looked for.
    ///
    /// ```
    /// use majordomus_cli::intent::Intents;
    /// let intents = Intents { intents: vec![], findings: vec![] };
    /// assert!(intents.intent("absent").is_none());
    /// ```
    pub fn intent(&self, id: &str) -> Option<&IntentView> {
        self.intents.iter().find(|i| i.id == id)
    }

    /// How many of the findings are failures rather than warnings: the count that decides
    /// whether the model is valid and whether `intent validate` exits 10.
    ///
    /// ```
    /// use majordomus_cli::intent::Intents;
    /// assert_eq!(Intents { intents: vec![], findings: vec![] }.failures(), 0);
    /// ```
    pub fn failures(&self) -> usize {
        self.findings.iter().filter(|f| f.level == FAIL).count()
    }

    /// How many of the findings are warnings: reported, never fatal — a milestone no intent
    /// serves, an intent nobody has planned yet.
    ///
    /// ```
    /// use majordomus_cli::intent::Intents;
    /// assert_eq!(Intents { intents: vec![], findings: vec![] }.warnings(), 0);
    /// ```
    pub fn warnings(&self) -> usize {
        self.findings.iter().filter(|f| f.level == WARN).count()
    }

    /// The intents that name a milestone, in identity order: the intents whose plan an issue
    /// under that milestone is part of, whatever it declares it serves.
    ///
    /// ```
    /// use majordomus_cli::intent::Intents;
    /// let intents = Intents { intents: vec![], findings: vec![] };
    /// // a milestone no intent names carries maintenance work
    /// assert!(intents.serving("fixture-milestone").is_empty());
    /// ```
    pub fn serving(&self, milestone: &str) -> Vec<&IntentView> {
        self.intents
            .iter()
            .filter(|i| i.milestones.iter().any(|m| m.id == milestone))
            .collect()
    }
}

// ---------------------------------------------------------------- preflight

/// What a preflight concluded, about one issue or about the whole piece of work.
///
/// ```
/// use majordomus_cli::intent::IntentPreflightVerdict;
/// // the words every surface answers with
/// assert_eq!(
///     serde_json::to_string(&IntentPreflightVerdict::Maintenance).unwrap(),
///     "\"maintenance\""
/// );
/// assert_ne!(IntentPreflightVerdict::Serves, IntentPreflightVerdict::Refused);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentPreflightVerdict {
    /// The work serves at least one criterion of a live intent, every link holds, and the
    /// plan of every intent it serves was critiqued with no blocking finding left open.
    Serves,
    /// The work sits under milestones no live intent names and serves no criterion of a live
    /// intent: operational work, allowed where `majordomus intent validate` allows it.
    Maintenance,
    /// A link is missing or points at nothing, or an intent it serves may not be executed yet.
    Refused,
}

/// Why a preflight refused, as a word a reader greps for.
///
/// ```
/// use majordomus_cli::intent::IntentPreflightCause;
/// assert_eq!(
///     serde_json::to_string(&IntentPreflightCause::OpenBlockingFinding).unwrap(),
///     "\"open_blocking_finding\""
/// );
/// assert_ne!(
///     IntentPreflightCause::IntentNotCritiqued,
///     IntentPreflightCause::IssueServesNothing
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentPreflightCause {
    /// The issue named is not an issue of the plan.
    UnknownIssue,
    /// No open issue's scope covers any of the paths.
    NoIssueCoversPaths,
    /// The issue's milestone realises a live intent and the issue serves none of its criteria.
    IssueServesNothing,
    /// The issue serves a criterion of an intent whose milestones do not include its own.
    ServesAnotherIntent,
    /// The issue serves an intent or a criterion that does not exist, or a token that is not
    /// `<intent>#<criterion>`.
    ServesUnknownCriterion,
    /// An intent the issue serves has no critique: its plan was never reviewed.
    IntentNotCritiqued,
    /// The critique of an intent the issue serves has a blocking finding still open.
    OpenBlockingFinding,
}

/// One reason a preflight refused: which issue, the cause, and one sentence of what is wrong.
///
/// ```
/// use majordomus_cli::intent::{IntentPreflightCause, IntentPreflightRefusal};
/// let r = IntentPreflightRefusal::new(
///     Some("I1900"),
///     IntentPreflightCause::IntentNotCritiqued,
///     "intent x has no critique".into(),
/// );
/// assert_eq!(r.cause, IntentPreflightCause::IntentNotCritiqued);
/// assert_eq!(r.issue.as_deref(), Some("I1900"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPreflightRefusal {
    /// The issue it is about; absent when no issue was resolved at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The cause.
    pub cause: IntentPreflightCause,
    /// What is wrong, in one sentence.
    pub message: String,
}

impl IntentPreflightRefusal {
    /// A refusal about `issue`, or about no issue when none was resolved.
    ///
    /// ```
    /// use majordomus_cli::intent::{IntentPreflightCause, IntentPreflightRefusal};
    /// let r = IntentPreflightRefusal::new(None, IntentPreflightCause::UnknownIssue, "x".into());
    /// assert!(r.issue.is_none());
    /// ```
    pub fn new(issue: Option<&str>, cause: IntentPreflightCause, message: String) -> Self {
        IntentPreflightRefusal {
            issue: issue.map(str::to_string),
            cause,
            message,
        }
    }
}

/// One issue the work resolved to, and what the preflight concluded about it.
///
/// ```
/// use majordomus_cli::intent::{IntentPreflightIssue, IntentPreflightVerdict};
/// let i = IntentPreflightIssue {
///     issue: "I0007".into(),
///     milestone: "ops".into(),
///     verdict: IntentPreflightVerdict::Maintenance,
///     serves: vec![],
///     intents: vec![],
/// };
/// // a milestone no intent names carries maintenance, which is not a refusal
/// assert_eq!(i.verdict, IntentPreflightVerdict::Maintenance);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPreflightIssue {
    /// The issue id.
    pub issue: String,
    /// Its milestone.
    pub milestone: String,
    /// What the preflight concluded about this issue.
    pub verdict: IntentPreflightVerdict,
    /// The `<intent>#<criterion>` references it declares, as declared.
    pub serves: Vec<String>,
    /// The live intents whose milestones include its milestone, sorted.
    pub intents: Vec<String>,
}

/// One intent a piece of work serves, and the link that says so: issue to the criteria it
/// declares in `serves`, each criterion's intent naming the issue's milestone.
///
/// ```
/// use majordomus_cli::intent::{IntentPreflightMatch, IntentStage};
/// let m = IntentPreflightMatch {
///     intent: "intent-lifecycle".into(),
///     title: "Work is traceable to the intent it serves".into(),
///     stage: IntentStage::Executing,
///     milestone: "intent-lifecycle".into(),
///     issue: "I1900".into(),
///     criteria: vec!["stage-is-derived".into()],
/// };
/// assert_eq!(m.issue, "I1900");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPreflightMatch {
    /// The intent.
    pub intent: String,
    /// Its title.
    pub title: String,
    /// Its derived stage.
    pub stage: IntentStage,
    /// The milestone that links the work to it.
    pub milestone: String,
    /// The issue that links the work to that milestone.
    pub issue: String,
    /// The criteria of the intent the issue declares it serves, in the intent's order.
    pub criteria: Vec<String>,
}

/// The critique of an intent as a preflight reads it: where it is, when and by whom the plan
/// was reviewed, and the blocking findings still open.
///
/// ```
/// use majordomus_cli::intent::IntentPreflightCritique;
/// let c = IntentPreflightCritique {
///     source: ".ai/repo/project/critiques/x.yaml".into(),
///     reviewed_at: "c3f20da".into(),
///     reviewed_by: "another session".into(),
///     open_blocking: vec![],
/// };
/// assert!(c.open_blocking.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPreflightCritique {
    /// Repository-relative path of the record.
    pub source: String,
    /// The commit the plan was reviewed at.
    pub reviewed_at: String,
    /// Who or what reviewed it.
    pub reviewed_by: String,
    /// Every blocking finding whose resolution is `open`; work may not start while one is.
    pub open_blocking: Vec<CritiqueFinding>,
}

/// The recorded gap of an intent as a preflight reads it, bounded to the criteria the work
/// serves: what was observed of each, at which commit, and the risks.
///
/// ```
/// use majordomus_cli::intent::IntentPreflightGap;
/// let g = IntentPreflightGap {
///     source: ".ai/repo/project/gaps/x.yaml".into(),
///     observed_at: "c3f20da".into(),
///     conditions: vec![],
///     risks: vec!["the schema may refuse old records".into()],
/// };
/// assert_eq!(g.risks.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPreflightGap {
    /// Repository-relative path of the record.
    pub source: String,
    /// The commit the observations were made on.
    pub observed_at: String,
    /// The gap's answer for each criterion the work serves, in the gap's order.
    pub conditions: Vec<GapCondition>,
    /// The risks it recorded.
    pub risks: Vec<String>,
}

/// One intent the work is held to, with what a worker needs before touching it: the statement,
/// the criteria the work serves with the live state of their evidence, the invariants and
/// non-goals, the governance, the critique and the gap. Only the intents the work reaches are
/// carried, never every intent.
///
/// ```
/// use majordomus_cli::intent::{IntentPreflightIntent, IntentStage};
/// let i = IntentPreflightIntent {
///     id: "x".into(),
///     title: "X".into(),
///     stage: IntentStage::Planned,
///     statement: "It becomes true.".into(),
///     criteria: vec![],
///     invariants: vec![],
///     guards: vec![],
///     non_goals: vec![],
///     governance: vec![],
///     critique: None,
///     gap: None,
/// };
/// // no critique recorded is carried as an absence, never as an empty review
/// assert!(i.critique.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPreflightIntent {
    /// The identity.
    pub id: String,
    /// One line.
    pub title: String,
    /// Its derived stage.
    pub stage: IntentStage,
    /// What must become true.
    pub statement: String,
    /// The criteria the work serves, each with the state of its evidence now, in the intent's
    /// order.
    pub criteria: Vec<IntentCriterion>,
    /// What must stay true.
    pub invariants: Vec<String>,
    /// The guards of the intent, each judged: the invariants that name their evidence
    /// (ADR 0113). Absent when the intent declares none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<IntentGuard>,
    /// What is deliberately not required.
    pub non_goals: Vec<String>,
    /// The governance a worker on this intent loads.
    pub governance: Vec<String>,
    /// The critique of its plan; absent when none was ever recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub critique: Option<IntentPreflightCritique>,
    /// Its recorded gap, bounded to the criteria above; absent when none was recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<IntentPreflightGap>,
}

/// Which intent a piece of work serves, whether it is maintenance, or why it may not proceed:
/// the issues it resolved to with a verdict each, every intent reached with its link, what
/// those intents require of the worker, and every refusal with its cause.
///
/// ```
/// use majordomus_cli::intent::{
///     IntentPreflight, IntentPreflightCause, IntentPreflightRefusal, IntentPreflightVerdict,
/// };
/// let refused = IntentPreflight {
///     verdict: IntentPreflightVerdict::Refused,
///     issues: vec![],
///     matches: vec![],
///     intents: vec![],
///     governance: vec![],
///     refusals: vec![IntentPreflightRefusal::new(
///         None,
///         IntentPreflightCause::NoIssueCoversPaths,
///         "no open issue's scope covers README.md".into(),
///     )],
///     refusal: Some("no open issue's scope covers README.md".into()),
/// };
/// assert_eq!(refused.verdict, IntentPreflightVerdict::Refused);
/// assert_eq!(refused.refusals[0].cause, IntentPreflightCause::NoIssueCoversPaths);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPreflight {
    /// `serves`, `maintenance` or `refused`: refused when any issue is, else serves when any
    /// issue serves an intent, else maintenance.
    pub verdict: IntentPreflightVerdict,
    /// The issues the work was resolved to — the one named, or the open issues whose scope
    /// covers a path — each with its own verdict.
    pub issues: Vec<IntentPreflightIssue>,
    /// Every intent an issue serves, with the link.
    pub matches: Vec<IntentPreflightMatch>,
    /// The intents the work serves, and no other, each with what it asks of the worker.
    pub intents: Vec<IntentPreflightIntent>,
    /// The governance of those intents, each entry once, in first-seen order.
    pub governance: Vec<String>,
    /// Every reason the preflight refused, in issue order; empty unless the verdict is
    /// `refused`.
    pub refusals: Vec<IntentPreflightRefusal>,
    /// When refused: every refusal's sentence, joined, for a reader who wants one line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// The coverage codes of the failures about an issue's own link, each as the preflight cause it
/// is. These are every `FAIL` finding coverage makes with an issue as its subject (its other
/// failure, `criterion_uncovered`, is about a criterion), so every broken link validation
/// reports is refused here. A failure coverage learns to make about an issue belongs in this
/// table too, or the preflight would let that broken link through.
const LINK_CAUSES: [(&str, IntentPreflightCause); 5] = [
    (
        "issue_without_purpose",
        IntentPreflightCause::IssueServesNothing,
    ),
    (
        "serves_outside_milestone",
        IntentPreflightCause::ServesAnotherIntent,
    ),
    (
        "malformed_serves",
        IntentPreflightCause::ServesUnknownCriterion,
    ),
    (
        "serves_unknown_intent",
        IntentPreflightCause::ServesUnknownCriterion,
    ),
    (
        "serves_unknown_criterion",
        IntentPreflightCause::ServesUnknownCriterion,
    ),
];

/// The coverage code of a finding about an issue's own link, as the preflight cause it is.
fn link_cause(code: &str) -> Option<IntentPreflightCause> {
    LINK_CAUSES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, cause)| *cause)
}

impl Intents {
    /// Which intent work on `issue`, or on `paths`, serves; whether it is maintenance; or why
    /// it may not proceed. This is the one join a session or a transition consumes, answered
    /// on every surface by the `intents.preflight` capability.
    ///
    /// ```
    /// use majordomus_cli::intent::{IntentPreflightCause, IntentPreflightVerdict, Intents};
    /// use majordomus_cli::plan::Plan;
    /// # let plan: Plan = serde_json::from_value(serde_json::json!({
    /// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
    /// #                 "active_milestone": ""},
    /// #     "statuses": {"issue": [], "milestone": []},
    /// #     "milestones": [], "issues": [], "waves": [], "edges": [],
    /// #     "milestone_edges": [], "findings": []})).unwrap();
    /// let intents = Intents { intents: vec![], findings: vec![] };
    /// let answer = intents.preflight(&plan, &[], &[], Some("I0001"), &[]);
    /// // an issue this plan does not hold is the first missing link, and it is named
    /// assert_eq!(answer.verdict, IntentPreflightVerdict::Refused);
    /// assert_eq!(answer.refusals[0].cause, IntentPreflightCause::UnknownIssue);
    /// assert!(answer.refusal.unwrap().contains("I0001"));
    /// ```
    ///
    /// An issue is followed through the criteria it declares in `serves` — never through its
    /// milestone alone — and each link is judged by the same coverage `intent validate`
    /// reports from, so every link validation fails is refused here too, and the preflight
    /// is at least as strict (it also refuses work whose only links point at retired intents):
    /// an issue
    /// that serves nothing while its milestone realises an intent, a criterion of an intent
    /// that does not name its milestone, a criterion or an intent that does not exist. An
    /// issue under milestones no live intent names, serving nothing, is maintenance. Every
    /// intent served must have a critique with no blocking finding open. Paths are followed to
    /// the open issues whose scope covers one, and every one of them is judged: one refused
    /// issue refuses the work.
    pub fn preflight(
        &self,
        plan: &Plan,
        gaps: &[GapRecord],
        critiques: &[CritiqueRecord],
        issue: Option<&str>,
        paths: &[String],
    ) -> IntentPreflight {
        let refused = |r: IntentPreflightRefusal| IntentPreflight {
            verdict: IntentPreflightVerdict::Refused,
            issues: Vec::new(),
            matches: Vec::new(),
            intents: Vec::new(),
            governance: Vec::new(),
            refusal: Some(r.message.clone()),
            refusals: vec![r],
        };
        let issues: Vec<&crate::plan::PlanIssue> = match issue {
            Some(id) => match plan.issue(id) {
                Some(i) => vec![i],
                None => {
                    return refused(IntentPreflightRefusal::new(
                        Some(id),
                        IntentPreflightCause::UnknownIssue,
                        format!("`{id}` is not an issue under .ai/repo/project/issues/"),
                    ))
                }
            },
            None => plan
                .issues
                .iter()
                .filter(|i| !matches!(i.status.as_str(), "DONE" | "CANCELLED"))
                .filter(|i| i.scope.iter().any(|s| paths.iter().any(|p| overlap(s, p))))
                .collect(),
        };
        if issues.is_empty() {
            return refused(IntentPreflightRefusal::new(
                None,
                IntentPreflightCause::NoIssueCoversPaths,
                format!(
                    "no open issue's scope covers {}; work that names no issue serves no \
                     declared intent",
                    paths.join(", ")
                ),
            ));
        }
        self.preflight_issues(plan, gaps, critiques, &issues)
    }

    /// The preflight over issues already resolved: what [`Intents::preflight`] answers once
    /// it knows which issues the work executes, and what [`crate::intent_binding`] asks when
    /// it resolved them another way — the open issues serving an intent a worker named.
    pub(crate) fn preflight_issues(
        &self,
        plan: &Plan,
        gaps: &[GapRecord],
        critiques: &[CritiqueRecord],
        issues: &[&crate::plan::PlanIssue],
    ) -> IntentPreflight {
        // The links are judged by the coverage validation reports from, over outlines of the
        // intents already derived, so a link `intent validate` calls broken is refused here
        // too; the preflight may refuse more, never less.
        let observed = observed_satisfied(gaps);
        let outlines: Vec<IntentOutline> = self
            .intents
            .iter()
            .map(|i| IntentOutline {
                id: i.id.clone(),
                criteria: i.satisfaction.iter().map(|c| c.id.clone()).collect(),
                milestones: i.milestones.iter().map(|m| m.id.clone()).collect(),
                retired: matches!(i.stage, IntentStage::Cancelled | IntentStage::Superseded),
                observed_satisfied: observed.get(&i.id).cloned().unwrap_or_default(),
                optional: i
                    .satisfaction
                    .iter()
                    .filter(|c| c.optional)
                    .map(|c| c.id.clone())
                    .collect(),
            })
            .collect();
        let cov = coverage(&outlines, plan);

        let mut out_issues = Vec::with_capacity(issues.len());
        let mut matches = Vec::new();
        let mut refusals = Vec::new();
        // intent id -> the criteria the work serves of it, in first-reached order: the payload
        // is bounded to these, never every intent
        let mut reached: Vec<(String, BTreeSet<String>)> = Vec::new();

        for i in issues {
            let mut mine: Vec<IntentPreflightRefusal> = cov
                .findings
                .iter()
                .filter(|f| f.level == FAIL && f.subject == i.id)
                .filter_map(|f| {
                    link_cause(&f.code).map(|cause| {
                        IntentPreflightRefusal::new(
                            Some(&i.id),
                            cause,
                            format!("{} {}", i.id, f.message),
                        )
                    })
                })
                .collect();
            let under: Vec<String> = cov
                .purpose(&i.id)
                .map(|p| p.intents.clone())
                .unwrap_or_default();
            // the criteria this issue carries, each a link coverage accepted
            let mut served: Vec<(&str, &str)> = cov
                .criteria
                .iter()
                .filter(|c| c.issues.iter().any(|x| x.id == i.id))
                .map(|c| (c.intent.as_str(), c.criterion.as_str()))
                .collect();
            // a cancelled issue is not live, so coverage lists it under no criterion; its
            // declared links are still what it was for
            if served.is_empty() && i.status == "CANCELLED" {
                served = i
                    .serves
                    .iter()
                    .filter_map(|t| t.split_once('#'))
                    .filter(|(iid, cid)| {
                        outlines.iter().any(|o| {
                            o.id == *iid
                                && !o.retired
                                && o.milestones.contains(&i.milestone)
                                && o.criteria.iter().any(|c| c == cid)
                        })
                    })
                    .collect();
            }
            if served.is_empty() && !under.is_empty() && mine.is_empty() {
                mine.push(IntentPreflightRefusal::new(
                    Some(&i.id),
                    IntentPreflightCause::IssueServesNothing,
                    format!(
                        "{} is under milestone {}, which realises {}, but serves none of its \
                         live criteria",
                        i.id,
                        i.milestone,
                        under.join(", ")
                    ),
                ));
            }
            let mut by_intent: Vec<(&str, Vec<String>)> = Vec::new();
            for (iid, cid) in &served {
                match reached.iter_mut().find(|(x, _)| x == iid) {
                    Some((_, cs)) => {
                        cs.insert((*cid).to_string());
                    }
                    None => {
                        reached.push(((*iid).to_string(), BTreeSet::from([(*cid).to_string()])))
                    }
                }
                match by_intent.iter_mut().find(|e| e.0 == *iid) {
                    Some((_, cs)) => cs.push((*cid).to_string()),
                    None => by_intent.push((*iid, vec![(*cid).to_string()])),
                }
            }
            // every intent reached is one of these intents: coverage lists criteria only of the
            // outlines built from them, and a cancelled issue's links are filtered by the same
            for (iid, view, criteria) in by_intent
                .into_iter()
                .filter_map(|(iid, criteria)| self.intent(iid).map(|view| (iid, view, criteria)))
            {
                matches.push(IntentPreflightMatch {
                    intent: view.id.clone(),
                    title: view.title.clone(),
                    stage: view.stage,
                    milestone: i.milestone.clone(),
                    issue: i.id.clone(),
                    criteria,
                });
                match critiques.iter().find(|c| c.intent == iid) {
                    None => mine.push(IntentPreflightRefusal::new(
                        Some(&i.id),
                        IntentPreflightCause::IntentNotCritiqued,
                        format!(
                            "{} serves intent {iid}, whose plan was never critiqued; record \
                             .ai/repo/project/critiques/{iid}.yaml before its work starts",
                            i.id
                        ),
                    )),
                    Some(c) => {
                        let open: Vec<&str> = open_blocking(c).map(|f| f.id.as_str()).collect();
                        if !open.is_empty() {
                            mine.push(IntentPreflightRefusal::new(
                                Some(&i.id),
                                IntentPreflightCause::OpenBlockingFinding,
                                format!(
                                    "{} serves intent {iid}, whose critique has blocking \
                                     finding(s) {} open; plan or reject them first",
                                    i.id,
                                    open.join(", ")
                                ),
                            ));
                        }
                    }
                }
            }

            let verdict = if !mine.is_empty() {
                IntentPreflightVerdict::Refused
            } else if !served.is_empty() {
                IntentPreflightVerdict::Serves
            } else {
                IntentPreflightVerdict::Maintenance
            };
            refusals.extend(mine);
            out_issues.push(IntentPreflightIssue {
                issue: i.id.clone(),
                milestone: i.milestone.clone(),
                verdict,
                serves: i.serves.clone(),
                intents: under,
            });
        }

        let intents: Vec<IntentPreflightIntent> = reached
            .iter()
            .filter_map(|(id, criteria)| {
                self.intent(id)
                    .map(|view| self.held_to(view, criteria, gaps, critiques))
            })
            .collect();
        let mut governance: Vec<String> = Vec::new();
        for g in intents.iter().flat_map(|i| i.governance.iter()) {
            if !governance.contains(g) {
                governance.push(g.clone());
            }
        }
        let verdict = if !refusals.is_empty() {
            IntentPreflightVerdict::Refused
        } else if out_issues
            .iter()
            .any(|i| i.verdict == IntentPreflightVerdict::Serves)
        {
            IntentPreflightVerdict::Serves
        } else {
            IntentPreflightVerdict::Maintenance
        };
        let refusal = (!refusals.is_empty()).then(|| {
            refusals
                .iter()
                .map(|r| r.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        });
        IntentPreflight {
            verdict,
            issues: out_issues,
            matches,
            intents,
            governance,
            refusals,
            refusal,
        }
    }

    /// One intent as the work is held to it: the criteria it serves, and the critique and the
    /// gap bounded to the same criteria.
    fn held_to(
        &self,
        view: &IntentView,
        served: &BTreeSet<String>,
        gaps: &[GapRecord],
        critiques: &[CritiqueRecord],
    ) -> IntentPreflightIntent {
        let wanted = |c: &str| served.contains(c);
        IntentPreflightIntent {
            id: view.id.clone(),
            title: view.title.clone(),
            stage: view.stage,
            statement: view.statement.clone(),
            criteria: view
                .satisfaction
                .iter()
                .filter(|c| wanted(&c.id))
                .cloned()
                .collect(),
            invariants: view.invariants.clone(),
            guards: view.guards.clone(),
            non_goals: view.non_goals.clone(),
            governance: view.governance.clone(),
            critique: critiques.iter().find(|c| c.intent == view.id).map(|c| {
                IntentPreflightCritique {
                    source: c.source.clone(),
                    reviewed_at: c.reviewed_at.clone(),
                    reviewed_by: c.reviewed_by.clone(),
                    open_blocking: open_blocking(c).cloned().collect(),
                }
            }),
            gap: gaps
                .iter()
                .find(|g| g.intent == view.id)
                .map(|g| IntentPreflightGap {
                    source: g.source.clone(),
                    observed_at: g.observed_at.clone(),
                    conditions: g
                        .conditions
                        .iter()
                        .filter(|c| wanted(&c.criterion))
                        .cloned()
                        .collect(),
                    risks: g.risks.clone(),
                }),
        }
    }
}

/// The blocking findings of a critique still open: the ones that refuse execution.
fn open_blocking(c: &CritiqueRecord) -> impl Iterator<Item = &CritiqueFinding> {
    c.findings
        .iter()
        .filter(|f| f.blocking && f.resolution == Some(ResolutionState::Open))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlanCounts, PlanIssue, PlanMilestone, PlanProject, PlanVocabulary};

    struct Table {
        tests: BTreeMap<String, TestStanding>,
        claims: BTreeMap<String, ClaimStanding>,
    }

    impl Table {
        fn new() -> Self {
            Table {
                tests: BTreeMap::new(),
                claims: BTreeMap::new(),
            }
        }
        fn with_test(mut self, id: &str, state: IntentEvidenceState) -> Self {
            self.tests.insert(
                id.into(),
                TestStanding {
                    present: true,
                    state,
                    proof: None,
                    evaluation: None,
                },
            );
            self
        }
        fn with_claim(mut self, id: &str, state: IntentEvidenceState) -> Self {
            let proof = match state {
                IntentEvidenceState::Current => ProofState::Proven,
                IntentEvidenceState::Stale => ProofState::Stale,
                IntentEvidenceState::Failing => ProofState::Failing,
                _ => ProofState::NotRun,
            };
            self.claims.insert(
                id.into(),
                ClaimStanding {
                    state,
                    proof,
                    evaluation: None,
                },
            );
            self
        }
    }

    impl EvidenceLookup for Table {
        fn test(&self, id: &TestId, _: &[String]) -> TestStanding {
            self.tests
                .get(&id.as_string())
                .cloned()
                .unwrap_or(TestStanding {
                    present: false,
                    state: IntentEvidenceState::NotRun,
                    proof: None,
                    evaluation: None,
                })
        }
        fn claim(&self, id: &str) -> Option<ClaimStanding> {
            self.claims.get(id).cloned()
        }
        fn deployment(&self, id: &str) -> bool {
            id == "fly"
        }
        fn object(&self, kind: &str, identity: &str) -> bool {
            kind == "rule"
                && (identity == "project.alpha" || identity.starts_with("project.alpha@"))
        }
        fn file(&self, path: &str) -> bool {
            path == "docs/INTENT.md"
        }
    }

    fn milestone(id: &str, status: &str) -> PlanMilestone {
        PlanMilestone {
            id: id.into(),
            status: status.into(),
            order: 0,
            priority: "p1".into(),
            title: id.into(),
            slug: id.into(),
            version: String::new(),
            rank: 0,
            depends_on: vec![],
            blocked_by: vec![],
            dependents: vec![],
            claims: vec![],
            counts: PlanCounts {
                total: 0,
                required: 0,
                by_status: BTreeMap::new(),
            },
            issues: vec![],
            outcome: String::new(),
        }
    }

    fn issue(id: &str, milestone: &str, status: &str, scope: &[&str]) -> PlanIssue {
        PlanIssue {
            id: id.into(),
            milestone: milestone.into(),
            status: status.into(),
            wave: 0,
            priority: "p1".into(),
            profile: "implementation".into(),
            parallel_safe: true,
            title: id.into(),
            slug: id.into(),
            serves: vec![],
            depends_on: vec![],
            blocked_by: vec![],
            dependents: vec![],
            scope: scope.iter().map(|s| s.to_string()).collect(),
            objective: String::new(),
            evidence_have: 0,
            evidence_need: 0,
            started_at: String::new(),
            verified_at: String::new(),
            completed_at: String::new(),
        }
    }

    fn plan(milestones: Vec<PlanMilestone>, issues: Vec<PlanIssue>) -> Plan {
        Plan {
            project: PlanProject {
                name: "p".into(),
                repository: "o/p".into(),
                default_branch: "master".into(),
                active_milestone: String::new(),
            },
            statuses: PlanVocabulary {
                issue: vec![],
                milestone: vec![],
            },
            milestones,
            issues,
            waves: vec![],
            edges: vec![],
            milestone_edges: vec![],
            findings: vec![],
        }
    }

    fn record(id: &str, milestones: &[&str], criteria: &[(&str, &str, &str)]) -> IntentRecord {
        IntentRecord {
            id: id.into(),
            source: format!("{INTENTS_DIR}/{id}.yaml"),
            title: id.into(),
            statement: "it becomes true".into(),
            invariants: vec![],
            satisfaction: criteria
                .iter()
                .map(|(cid, kind, r)| CriterionRecord {
                    id: (*cid).into(),
                    criterion: "observable".into(),
                    evidence: (*kind).into(),
                    reference: (*r).into(),
                    optional: false,
                })
                .collect(),
            milestones: milestones.iter().map(|m| m.to_string()).collect(),
            ..IntentRecord::default()
        }
    }

    fn codes(i: &Intents) -> Vec<&str> {
        i.findings.iter().map(|f| f.code.as_str()).collect()
    }

    const CASE: (&str, &str, &str) = ("case", "test", "test/cases/1_x.sh");

    #[test]
    fn the_stage_follows_the_milestones_and_only_then_the_evidence() {
        let cases: &[(&[&str], IntentEvidenceState, IntentStage)] = &[
            (
                &["PLANNED"],
                IntentEvidenceState::Current,
                IntentStage::Planned,
            ),
            (
                &["BLOCKED"],
                IntentEvidenceState::Current,
                IntentStage::Planned,
            ),
            (
                &["ACTIVE"],
                IntentEvidenceState::Current,
                IntentStage::Executing,
            ),
            (
                &["VERIFY"],
                IntentEvidenceState::NotRun,
                IntentStage::Executing,
            ),
            (
                &["DONE", "PLANNED"],
                IntentEvidenceState::Current,
                IntentStage::Executing,
            ),
            (
                &["DONE"],
                IntentEvidenceState::NotRun,
                IntentStage::Verifying,
            ),
            (
                &["DONE"],
                IntentEvidenceState::Stale,
                IntentStage::Verifying,
            ),
            (
                &["DONE"],
                IntentEvidenceState::Failing,
                IntentStage::Verifying,
            ),
            (
                &["DONE"],
                IntentEvidenceState::Current,
                IntentStage::Satisfied,
            ),
            (
                &["DONE", "CANCELLED"],
                IntentEvidenceState::Current,
                IntentStage::Satisfied,
            ),
        ];
        for (statuses, evidence, want) in cases {
            let ms: Vec<PlanMilestone> = statuses
                .iter()
                .enumerate()
                .map(|(n, s)| milestone(&format!("m{n}"), s))
                .collect();
            let names: Vec<String> = ms.iter().map(|m| m.id.clone()).collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            let p = plan(ms, vec![]);
            let ev = Table::new().with_test("suite:1_x", *evidence);
            let i = Intents::derive(vec![record("x", &names, &[CASE])], &p, &ev);
            assert_eq!(i.intents[0].stage, *want, "{statuses:?} with {evidence:?}");
            assert_eq!(i.failures(), 0, "{:?}", i.findings);
        }
    }

    /// The stage still waits for the plan, and since ADR 0107 the evidence's own answer is no
    /// longer hidden behind it: with every criterion current and the milestone ACTIVE, the
    /// stage reads `executing` and the verdict reads `satisfied`. The stage expectation is
    /// unchanged; the verdict is what this test now adds.
    #[test]
    fn evidence_satisfies_nothing_while_a_milestone_is_open() {
        let p = plan(vec![milestone("m", "ACTIVE")], vec![]);
        let ev = Table::new().with_test("suite:1_x", IntentEvidenceState::Current);
        let i = Intents::derive(vec![record("x", &["m"], &[CASE])], &p, &ev);
        assert_eq!(i.intents[0].met, 1, "the criterion itself is met");
        assert_eq!(i.intents[0].stage, IntentStage::Executing);
        assert_eq!(i.intents[0].verdict.state, IntentVerdictState::Satisfied);
        assert!(i.intents[0].verdict.reasons.is_empty());
    }

    #[test]
    fn the_verdict_is_unknown_when_only_a_kind_the_ledger_cannot_settle_is_unmet() {
        let p = plan(vec![milestone("m", "ACTIVE")], vec![]);
        let ev = Table::new().with_test("suite:1_x", IntentEvidenceState::Current);
        let i = Intents::derive(
            vec![record(
                "x",
                &["m"],
                &[
                    CASE,
                    ("ships", "command", "just verify"),
                    ("live", "deployment", "fly"),
                ],
            )],
            &p,
            &ev,
        );
        let v = &i.intents[0].verdict;
        assert_eq!(v.state, IntentVerdictState::Unknown);
        let held: Vec<(&str, &str, IntentEvidenceState)> = v
            .reasons
            .iter()
            .map(|r| (r.criterion.as_str(), r.evidence.as_str(), r.state))
            .collect();
        assert_eq!(
            held,
            [
                ("ships", "command", IntentEvidenceState::NotDerivable),
                ("live", "deployment", IntentEvidenceState::NotDerivable),
            ]
        );
        assert_eq!(
            i.intents[0].stage,
            IntentStage::Executing,
            "the stage is the plan's"
        );
    }

    #[test]
    fn the_verdict_is_unsatisfied_when_a_test_the_ledger_can_settle_is_unmet() {
        // a DONE milestone neither implies the verdict nor is required for it
        for status in ["ACTIVE", "DONE"] {
            let p = plan(vec![milestone("m", status)], vec![]);
            for state in [
                IntentEvidenceState::NotRun,
                IntentEvidenceState::Stale,
                IntentEvidenceState::Failing,
            ] {
                let ev = Table::new().with_test("suite:1_x", state);
                let i = Intents::derive(
                    vec![record(
                        "x",
                        &["m"],
                        &[CASE, ("ships", "command", "just verify")],
                    )],
                    &p,
                    &ev,
                );
                let v = &i.intents[0].verdict;
                assert_eq!(
                    v.state,
                    IntentVerdictState::Unsatisfied,
                    "{status} {state:?}"
                );
                assert_eq!(v.reasons[0].criterion, "case");
                assert_eq!(v.reasons[0].state, state);
                assert_eq!(v.reasons.len(), 2, "every unmet criterion is named");
            }
        }
        // an unproven claim says no the same way
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let ev = Table::new().with_claim("c", IntentEvidenceState::Stale);
        let i = Intents::derive(vec![record("x", &["m"], &[("k", "claim", "c")])], &p, &ev);
        assert_eq!(i.intents[0].verdict.state, IntentVerdictState::Unsatisfied);
        assert_eq!(i.intents[0].stage, IntentStage::Verifying);
    }

    #[test]
    fn the_verdict_of_an_intent_declaring_no_criterion_is_unknown() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let i = Intents::derive(vec![record("x", &["m"], &[])], &p, &Table::new());
        assert_eq!(i.intents[0].verdict.state, IntentVerdictState::Unknown);
        assert!(i.intents[0].verdict.reasons.is_empty());
        assert!(codes(&i).contains(&"intent_without_criterion"));
    }

    #[test]
    fn every_verdict_and_evidence_state_is_printed_as_the_word_it_serialises_to() {
        for v in [
            IntentVerdictState::Satisfied,
            IntentVerdictState::Unsatisfied,
            IntentVerdictState::Unknown,
        ] {
            assert_eq!(
                serde_json::to_value(v).unwrap(),
                serde_json::Value::from(v.as_str())
            );
        }
        for s in [
            IntentEvidenceState::Current,
            IntentEvidenceState::Stale,
            IntentEvidenceState::Failing,
            IntentEvidenceState::NotRun,
            IntentEvidenceState::NotDerivable,
            IntentEvidenceState::Unresolved,
        ] {
            assert_eq!(
                serde_json::to_value(s).unwrap(),
                serde_json::Value::from(s.as_str())
            );
        }
    }

    #[test]
    fn evidence_is_current_only_when_the_evidence_module_calls_it_current() {
        use IntentEvidenceState::{Current, Failing, NotRun, Stale};
        use TreeState::{Clean, Dirty, Unknown};
        for (state, tree, want) in [
            (ProofState::Proven, Clean, Current),
            (ProofState::InputsUnchanged, Clean, Current),
            // a pass on a tree that was not its commit: nothing later can be matched to it
            (ProofState::InputsUnchanged, Dirty, Stale),
            (ProofState::InputsUnchanged, Unknown, Stale),
            (ProofState::Stale, Clean, Stale),
            (ProofState::Failing, Clean, Failing),
            (ProofState::NotRun, Unknown, NotRun),
            (ProofState::Unrunnable, Unknown, NotRun),
            (ProofState::NoTest, Unknown, NotRun),
        ] {
            assert_eq!(
                IntentEvidenceState::judged(state, tree),
                want,
                "{state:?} on a {tree:?} tree"
            );
            assert_eq!(
                want == Current,
                evidence::current(state, tree, TreeState::Clean),
                "{state:?} on a {tree:?} tree: one answer to whether it is current"
            );
        }
    }

    #[test]
    fn a_claim_criterion_is_met_only_by_current_evidence() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        for state in [
            IntentEvidenceState::Current,
            IntentEvidenceState::Stale,
            IntentEvidenceState::Failing,
            IntentEvidenceState::NotRun,
        ] {
            let ev = Table::new().with_claim("c", state);
            let i = Intents::derive(vec![record("x", &["m"], &[("k", "claim", "c")])], &p, &ev);
            let c = &i.intents[0].satisfaction[0];
            assert_eq!(c.state, state);
            assert_eq!(c.met, state == IntentEvidenceState::Current, "{state:?}");
        }
    }

    #[test]
    fn a_command_or_deployment_never_meets_a_criterion() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let i = Intents::derive(
            vec![record(
                "x",
                &["m"],
                &[("a", "command", "just verify"), ("b", "deployment", "fly")],
            )],
            &p,
            &Table::new(),
        );
        let v = &i.intents[0];
        assert!(v
            .satisfaction
            .iter()
            .all(|c| c.state == IntentEvidenceState::NotDerivable && !c.met));
        assert_eq!(v.stage, IntentStage::Verifying);
        assert_eq!(i.failures(), 0, "{:?}", i.findings);
    }

    #[test]
    fn cancelled_and_superseded_are_what_the_record_says() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let mut a = record("a", &["m"], &[CASE]);
        a.cancelled = true;
        let mut b = record("b", &["m"], &[CASE]);
        b.superseded_by = "a".into();
        let i = Intents::derive(vec![a, b], &p, &Table::new());
        assert_eq!(i.intents[0].stage, IntentStage::Cancelled);
        assert_eq!(i.intents[1].stage, IntentStage::Superseded);
    }

    #[test]
    fn an_intent_with_no_milestone_is_declared_and_a_failure() {
        let p = plan(vec![], vec![]);
        let i = Intents::derive(vec![record("x", &[], &[CASE])], &p, &Table::new());
        assert_eq!(i.intents[0].stage, IntentStage::Declared);
        assert!(codes(&i).contains(&"intent_without_milestone"));
        assert!(i.failures() > 0);
    }

    #[test]
    fn an_unknown_milestone_is_a_failure_and_holds_the_stage_at_declared() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let ev = Table::new().with_test("suite:1_x", IntentEvidenceState::Current);
        let i = Intents::derive(vec![record("x", &["m", "ghost"], &[CASE])], &p, &ev);
        assert_eq!(i.intents[0].stage, IntentStage::Declared);
        assert!(!i.intents[0].milestones[1].resolved);
        assert_eq!(codes(&i), ["unknown_milestone"]);
    }

    #[test]
    fn a_criterion_without_a_ref_is_a_failure() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let i = Intents::derive(
            vec![record("x", &["m"], &[("c", "test", " ")])],
            &p,
            &Table::new(),
        );
        assert_eq!(codes(&i), ["criterion_without_ref"]);
        assert_eq!(
            i.intents[0].satisfaction[0].state,
            IntentEvidenceState::Unresolved
        );
    }

    #[test]
    fn a_ref_that_does_not_resolve_is_a_failure_for_every_kind() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        for (kind, r) in [
            ("test", "lib/not-a-test.sh"),
            ("test", "test/cases/2_absent.sh"),
            ("test", "suite:../x"),
            ("claim", "no-such-claim"),
            ("deployment", "nowhere"),
        ] {
            let i = Intents::derive(
                vec![record("x", &["m"], &[("c", kind, r)])],
                &p,
                &Table::new(),
            );
            assert_eq!(codes(&i), ["unresolved_evidence_ref"], "{kind} {r}");
            assert!(i.findings[0].message.contains("criterion `c`"));
        }
    }

    #[test]
    fn a_milestone_serving_no_intent_is_a_warning_unless_it_is_retired() {
        let p = plan(
            vec![
                milestone("m", "DONE"),
                milestone("orphan", "ACTIVE"),
                milestone("gone", "CANCELLED"),
            ],
            vec![],
        );
        let ev = Table::new().with_test("suite:1_x", IntentEvidenceState::Current);
        let i = Intents::derive(vec![record("x", &["m"], &[CASE])], &p, &ev);
        assert_eq!(codes(&i), ["milestone_serves_no_intent"]);
        assert_eq!(i.findings[0].subject, "orphan");
        assert_eq!(i.findings[0].level, WARN);
        assert_eq!(i.failures(), 0);
        assert_eq!(i.warnings(), 1);
    }

    #[test]
    fn the_remaining_findings_are_each_named() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let mut dup = record("dup", &["m"], &[CASE, CASE]);
        dup.governance = vec![
            "rule:project.alpha".into(),
            "rule:project.alpha@1".into(),
            "file:docs/INTENT.md".into(),
            "adr:adr-9999".into(),
        ];
        dup.superseded_by = "ghost".into();
        let mut moved = record("moved", &["m"], &[]);
        moved.source = format!("{INTENTS_DIR}/elsewhere.yaml");
        let i = Intents::derive(vec![dup, moved], &p, &Table::new());
        assert_eq!(
            codes(&i),
            [
                "duplicate_criterion",
                "unresolved_evidence_ref",
                "unresolved_evidence_ref",
                "unresolved_governance",
                "unknown_successor",
                "file_name_mismatch",
                "intent_without_criterion",
            ]
        );
        assert!(i.findings[3].message.contains("adr:adr-9999"));
        assert!(i.findings.iter().all(|f| f.reproduce == REPRODUCE));
    }

    /// An issue under `milestone` serving each of `serves`.
    fn serving(id: &str, milestone: &str, scope: &[&str], serves: &[&str]) -> PlanIssue {
        PlanIssue {
            serves: serves.iter().map(|s| s.to_string()).collect(),
            ..issue(id, milestone, "READY", scope)
        }
    }

    /// A critique of `intent` with `findings` as `(id, blocking, resolution)`.
    fn critique(intent: &str, findings: &[(&str, bool, &str)]) -> CritiqueRecord {
        let findings: Vec<serde_json::Value> = findings
            .iter()
            .map(|(id, blocking, state)| {
                serde_json::json!({"id": id, "class": "regression_risk", "subject": intent,
                    "finding": "f", "blocking": blocking,
                    "resolution": {"state": state, "issue": "I1", "because": "b"}})
            })
            .collect();
        CritiqueRecord::from_metadata(
            &format!(".ai/repo/project/critiques/{intent}.yaml"),
            &serde_json::json!({"intent": intent, "reviewed_at": "c0ffee",
                "reviewed_by": "the test", "findings": findings}),
        )
    }

    fn causes(answer: &IntentPreflight) -> Vec<(&str, IntentPreflightCause)> {
        answer
            .refusals
            .iter()
            .map(|r| (r.issue.as_deref().unwrap_or(""), r.cause))
            .collect()
    }

    #[test]
    fn preflight_follows_what_an_issue_serves_and_never_its_milestone_alone() {
        use IntentPreflightCause::*;
        let p = plan(
            vec![
                milestone("m", "ACTIVE"),
                milestone("n", "ACTIVE"),
                milestone("ops", "ACTIVE"),
            ],
            vec![
                serving("I1", "m", &["apps/cli/src"], &["x#case"]),
                serving("I2", "ops", &["lib"], &[]),
                serving("I3", "m", &["docs"], &[]),
                serving("I4", "m", &["share"], &["y#case"]),
                serving("I5", "m", &["share"], &["x#nope"]),
                serving("I6", "m", &["share"], &["ghost#case"]),
                serving("I7", "m", &["share"], &["not-a-reference"]),
                issue("I8", "m", "DONE", &["site"]),
            ],
        );
        let mut x = record("x", &["m"], &[CASE]);
        x.governance = vec!["rule:project.alpha".into()];
        let y = record("y", &["n"], &[CASE]);
        let i = Intents::derive(vec![x, y], &p, &Table::new());
        let reviewed = [critique("x", &[]), critique("y", &[])];
        let one = |id: &str| i.preflight(&p, &[], &reviewed, Some(id), &[]);

        let served = one("I1");
        assert_eq!(served.verdict, IntentPreflightVerdict::Serves);
        assert_eq!(served.matches[0].intent, "x");
        assert_eq!(served.matches[0].criteria, ["case"]);
        assert_eq!(served.governance, ["rule:project.alpha"]);
        assert!(served.refusals.is_empty() && served.refusal.is_none());

        // a milestone no intent names carries maintenance, exactly as validation allows it
        let maintenance = one("I2");
        assert_eq!(maintenance.verdict, IntentPreflightVerdict::Maintenance);
        assert!(maintenance.intents.is_empty() && maintenance.refusals.is_empty());
        assert_eq!(
            maintenance.issues[0].verdict,
            IntentPreflightVerdict::Maintenance
        );

        // under the milestone of x, serving nothing: the milestone alone serves nothing
        for (id, cause) in [
            ("I3", IssueServesNothing),
            ("I4", ServesAnotherIntent),
            ("I5", ServesUnknownCriterion),
            ("I6", ServesUnknownCriterion),
            ("I7", ServesUnknownCriterion),
        ] {
            let answer = one(id);
            assert_eq!(answer.verdict, IntentPreflightVerdict::Refused, "{id}");
            assert_eq!(causes(&answer), [(id, cause)], "{id}");
            assert!(answer.refusal.unwrap().contains(id), "{id}");
            assert!(answer.matches.is_empty(), "{id} serves nothing it may");
            assert!(answer.intents.is_empty(), "{id}: no intent is served");
        }
        // a DONE issue under an intent milestone, serving nothing, is refused like any other
        assert_eq!(causes(&one("I8")), [("I8", IssueServesNothing)]);

        let unknown = one("I404");
        assert_eq!(causes(&unknown), [("I404", UnknownIssue)]);
        assert!(unknown.refusal.unwrap().contains("I404"));

        // a closed issue's scope claims nothing
        let closed = i.preflight(&p, &[], &reviewed, None, &["site/x.md".into()]);
        assert_eq!(causes(&closed), [("", NoIssueCoversPaths)]);
        assert!(closed.refusal.unwrap().contains("no open issue"));
    }

    #[test]
    fn preflight_keeps_a_cancelled_issues_links_and_gathers_the_criteria_of_one_intent() {
        use IntentPreflightCause::*;
        let cancelled = |i: PlanIssue| PlanIssue {
            status: "CANCELLED".into(),
            ..i
        };
        let p = plan(
            vec![milestone("m", "ACTIVE")],
            vec![
                serving("I1", "m", &["lib"], &["x#case", "x#other"]),
                cancelled(serving("I2", "m", &["docs"], &["x#case"])),
                cancelled(serving("I3", "m", &["site"], &[])),
                serving("I4", "m", &["lib/x"], &["x#other"]),
            ],
        );
        let x = record("x", &["m"], &[CASE, ("other", "test", "test/cases/2_y.sh")]);
        let i = Intents::derive(vec![x], &p, &Table::new());
        let reviewed = [critique("x", &[])];
        let one = |id: &str| i.preflight(&p, &[], &reviewed, Some(id), &[]);

        // two criteria of one intent are one match carrying both
        let both = one("I1");
        assert_eq!(both.verdict, IntentPreflightVerdict::Serves);
        assert_eq!(both.matches.len(), 1, "{:?}", both.matches);
        assert_eq!(both.matches[0].criteria, ["case", "other"]);
        assert_eq!(both.intents.len(), 1);

        // a cancelled issue is not live, so coverage lists it under no criterion: what it
        // declared it served is still what it was for
        let gone = one("I2");
        assert_eq!(
            gone.verdict,
            IntentPreflightVerdict::Serves,
            "{:?}",
            gone.refusals
        );
        assert_eq!(gone.matches[0].criteria, ["case"]);
        // and a cancelled issue that served nothing under the milestone of x is refused
        assert_eq!(causes(&one("I3")), [("I3", IssueServesNothing)]);

        // by path, two issues reach the same intent: it is held to once, with both criteria
        let by_path = i.preflight(&p, &[], &reviewed, None, &["lib/x/a.rs".into()]);
        assert_eq!(by_path.verdict, IntentPreflightVerdict::Serves);
        assert_eq!(by_path.issues.len(), 2);
        assert_eq!(by_path.intents.len(), 1);
        let held: Vec<&str> = by_path.intents[0]
            .criteria
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(held, ["case", "other"]);
    }

    #[test]
    fn preflight_refuses_work_on_an_intent_whose_plan_was_not_reviewed_or_has_a_blocker_open() {
        use IntentPreflightCause::*;
        let p = plan(
            vec![milestone("m", "ACTIVE")],
            vec![serving("I1", "m", &["lib"], &["x#case"])],
        );
        let i = Intents::derive(vec![record("x", &["m"], &[CASE])], &p, &Table::new());
        let with = |critiques: &[CritiqueRecord]| i.preflight(&p, &[], critiques, Some("I1"), &[]);

        let unreviewed = with(&[]);
        assert_eq!(causes(&unreviewed), [("I1", IntentNotCritiqued)]);
        // the intent is still carried, so the worker sees what it is held to
        assert_eq!(unreviewed.intents[0].id, "x");
        assert!(unreviewed.intents[0].critique.is_none());
        // a critique of another intent reviews nothing here
        assert_eq!(
            causes(&with(&[critique("other", &[])])),
            [("I1", IntentNotCritiqued)]
        );

        let blocked = with(&[critique(
            "x",
            &[("open-one", true, "open"), ("advisory", false, "open")],
        )]);
        assert_eq!(causes(&blocked), [("I1", OpenBlockingFinding)]);
        assert!(blocked.refusal.unwrap().contains("open-one"));
        let held = blocked.intents[0].critique.as_ref().unwrap();
        let open: Vec<&str> = held.open_blocking.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(
            open,
            ["open-one"],
            "a finding that blocks nothing is not listed"
        );

        // planned or rejected, a blocking finding no longer holds the work
        let resolved = with(&[critique(
            "x",
            &[("planned", true, "planned"), ("rejected", true, "rejected")],
        )]);
        assert_eq!(resolved.verdict, IntentPreflightVerdict::Serves);
        let held = resolved.intents[0].critique.as_ref().unwrap();
        assert!(held.open_blocking.is_empty());
        assert_eq!(held.reviewed_at, "c0ffee");
    }

    #[test]
    fn preflight_by_path_judges_every_issue_and_answers_the_worst() {
        use IntentPreflightVerdict::*;
        let p = plan(
            vec![milestone("m", "ACTIVE"), milestone("ops", "ACTIVE")],
            vec![
                serving("I1", "m", &["apps/cli"], &["x#case"]),
                serving("I2", "ops", &["apps/cli/src/ops"], &[]),
                serving("I3", "m", &["apps/cli/src/legacy"], &[]),
            ],
        );
        let i = Intents::derive(vec![record("x", &["m"], &[CASE])], &p, &Table::new());
        let reviewed = [critique("x", &[])];
        let at = |paths: &[&str]| {
            let paths: Vec<String> = paths.iter().map(|s| s.to_string()).collect();
            i.preflight(&p, &[], &reviewed, None, &paths)
        };
        let verdicts = |a: &IntentPreflight| -> Vec<(String, IntentPreflightVerdict)> {
            a.issues
                .iter()
                .map(|x| (x.issue.clone(), x.verdict))
                .collect()
        };

        assert_eq!(at(&["apps/cli/src/main.rs"]).verdict, Serves);
        // one served issue never vouches for another that serves nothing
        let mixed = at(&["apps/cli/src/legacy/old.rs"]);
        assert_eq!(mixed.verdict, Refused);
        assert_eq!(
            verdicts(&mixed),
            [("I1".to_string(), Serves), ("I3".to_string(), Refused)]
        );
        assert_eq!(
            causes(&mixed),
            [("I3", IntentPreflightCause::IssueServesNothing)]
        );
        // served work beside maintenance is held to the intent it serves
        let beside = at(&["apps/cli/src/ops/run.rs"]);
        assert_eq!(beside.verdict, Serves);
        assert_eq!(
            verdicts(&beside),
            [("I1".to_string(), Serves), ("I2".to_string(), Maintenance)]
        );
        assert_eq!(beside.intents.len(), 1);
    }

    #[test]
    fn preflight_carries_only_the_served_intents_bounded_to_the_served_criteria() {
        let p = plan(
            vec![milestone("m", "ACTIVE"), milestone("elsewhere", "ACTIVE")],
            vec![
                serving("I1", "m", &["lib"], &["x#a"]),
                serving("I2", "elsewhere", &["share"], &["z#a"]),
            ],
        );
        let mut x = record(
            "x",
            &["m"],
            &[
                ("a", "test", "test/cases/1_x.sh"),
                ("b", "test", "test/cases/2_x.sh"),
            ],
        );
        x.invariants = vec!["nothing breaks".into()];
        x.non_goals = vec!["speed".into()];
        let z = record("z", &["elsewhere"], &[("a", "test", "test/cases/3_x.sh")]);
        let ev = Table::new().with_test("suite:1_x", IntentEvidenceState::Failing);
        let i = Intents::derive(vec![x, z], &p, &ev);
        let gap = GapRecord::from_metadata(
            ".ai/repo/project/gaps/x.yaml",
            &serde_json::json!({"intent": "x", "observed_at": "c0ffee",
                "conditions": [{"criterion": "a", "state": "missing"},
                               {"criterion": "b", "state": "unknown"}],
                "risks": ["the schema may refuse old records"]}),
        );
        let answer = i.preflight(
            &p,
            &[gap],
            &[critique("x", &[]), critique("z", &[])],
            Some("I1"),
            &[],
        );
        assert_eq!(answer.verdict, IntentPreflightVerdict::Serves);
        let ids: Vec<&str> = answer.intents.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(
            ids,
            ["x"],
            "an intent the work does not serve is not carried"
        );
        let x = &answer.intents[0];
        assert_eq!(x.statement, "it becomes true");
        assert_eq!(x.invariants, ["nothing breaks"]);
        assert_eq!(x.non_goals, ["speed"]);
        let criteria: Vec<(&str, IntentEvidenceState)> = x
            .criteria
            .iter()
            .map(|c| (c.id.as_str(), c.state))
            .collect();
        assert_eq!(criteria, [("a", IntentEvidenceState::Failing)]);
        let gap = x.gap.as_ref().unwrap();
        let conditions: Vec<&str> = gap
            .conditions
            .iter()
            .map(|c| c.criterion.as_str())
            .collect();
        assert_eq!(conditions, ["a"]);
        assert_eq!(gap.risks, ["the schema may refuse old records"]);
    }

    #[test]
    fn preflight_names_a_governance_entry_two_served_intents_share_once() {
        let p = plan(
            vec![milestone("m", "ACTIVE")],
            vec![serving("I1", "m", &["lib"], &["a#case", "b#case"])],
        );
        let mut a = record("a", &["m"], &[CASE]);
        a.governance = vec!["rule:project.alpha".into(), "file:docs/INTENT.md".into()];
        let mut b = record("b", &["m"], &[CASE]);
        b.governance = vec!["file:docs/INTENT.md".into(), "rule:project.alpha@1".into()];
        let i = Intents::derive(vec![a, b], &p, &Table::new());
        let reviewed = [critique("a", &[]), critique("b", &[])];
        let answer = i.preflight(&p, &[], &reviewed, Some("I1"), &[]);
        assert_eq!(answer.verdict, IntentPreflightVerdict::Serves);
        let served: Vec<&str> = answer.matches.iter().map(|m| m.intent.as_str()).collect();
        assert_eq!(served, ["a", "b"]);
        assert_eq!(
            answer.governance,
            [
                "rule:project.alpha",
                "file:docs/INTENT.md",
                "rule:project.alpha@1"
            ]
        );
    }

    #[test]
    fn every_stage_is_printed_as_the_word_it_serialises_to() {
        for stage in [
            IntentStage::Declared,
            IntentStage::Planned,
            IntentStage::Executing,
            IntentStage::Verifying,
            IntentStage::Satisfied,
            IntentStage::Cancelled,
            IntentStage::Superseded,
        ] {
            assert_eq!(
                serde_json::to_value(stage).unwrap(),
                serde_json::Value::from(stage.as_str()),
                "{stage:?} is spelled one way by serde and another by as_str"
            );
        }
    }

    #[test]
    fn a_named_test_is_a_suite_case_or_a_crate_test_and_no_other_runner() {
        assert_eq!(
            test_id("suite:1_x"),
            TestId::named(evidence::Runner::Suite, "1_x")
        );
        assert_eq!(
            test_id("crate:intent"),
            TestId::named(evidence::Runner::Crate, "intent")
        );
        assert_ne!(test_id("crate:intent"), test_id("suite:intent"));
        assert_eq!(test_id("pytest:intent"), None);
        assert_eq!(test_id("no-prefix-at-all"), None);

        // and a criterion naming a crate test is met by that test's current run
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let ev = Table::new().with_test(
            &TestId::named(evidence::Runner::Crate, "intent")
                .unwrap()
                .as_string(),
            IntentEvidenceState::Current,
        );
        let i = Intents::derive(
            vec![record("x", &["m"], &[("c", "test", "crate:intent")])],
            &p,
            &ev,
        );
        assert_eq!(codes(&i), Vec::<&str>::new());
        assert_eq!(i.intents[0].stage, IntentStage::Satisfied);
        let i = Intents::derive(
            vec![record("x", &["m"], &[("c", "test", "pytest:intent")])],
            &p,
            &ev,
        );
        assert_eq!(codes(&i), ["unresolved_evidence_ref"]);
    }

    #[test]
    fn governance_resolves_only_a_known_kind_naming_what_the_repository_holds() {
        let ev = Table::new()
            .with_claim("known", IntentEvidenceState::Current)
            .with_test("suite:1_x", IntentEvidenceState::Current);
        for (entry, resolves) in [
            ("rule:project.alpha", true),
            ("rule:project.beta", false),
            ("claim:known", true),
            ("claim:unknown", false),
            ("file:docs/INTENT.md", true),
            ("file:docs/OTHER.md", false),
            ("adr:adr-0001", false),
            ("no-kind-at-all", false),
            ("ticket:42", false),
        ] {
            assert_eq!(governance_resolves(entry, &ev), resolves, "{entry}");
        }
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let mut x = record("x", &["m"], &[CASE]);
        x.governance = vec![
            "claim:known".into(),
            "no-kind-at-all".into(),
            "ticket:42".into(),
        ];
        let i = Intents::derive(vec![x], &p, &ev);
        assert_eq!(
            codes(&i),
            ["unresolved_governance", "unresolved_governance"]
        );
        assert!(i.findings[0].message.contains("`no-kind-at-all`"));
        assert!(i.findings[1].message.contains("`ticket:42`"));
    }
    /// The binding (ADR 0111) over the same plan the preflight is judged over: every way
    /// work is named resolves or says which link is missing, and every preflight cause
    /// reaches the binding under its own word.
    #[test]
    fn the_binding_resolves_every_way_work_is_named() {
        use crate::intent_binding::{bind, BindingCause, BindingRequest, BindingStanding};
        use crate::policy::IntentPolicy;

        let mut finished = serving("I4", "zm", &["zed"], &["z#case"]);
        finished.status = "DONE".into();
        let p = plan(
            vec![
                milestone("m", "ACTIVE"),
                milestone("n", "ACTIVE"),
                milestone("zm", "ACTIVE"),
                milestone("rm", "ACTIVE"),
                milestone("ops", "ACTIVE"),
            ],
            vec![
                serving("I1", "m", &["lib"], &["x#case"]),
                serving("I2", "ops", &["ops"], &[]),
                serving("I3", "n", &["lib"], &["y#case"]),
                finished,
                serving("I5", "m", &["five"], &[]),
                serving("I6", "m", &["six"], &["y#case"]),
                serving("I7", "m", &["seven"], &["ghost#case"]),
            ],
        );
        let mut retired = record("r", &["rm"], &[CASE]);
        retired.cancelled = true;
        let i = Intents::derive(
            vec![
                record("x", &["m"], &[CASE]),
                record("y", &["n"], &[CASE]),
                record("z", &["zm"], &[CASE]),
                retired,
            ],
            &p,
            // the criteria's test is known and has not run: nothing structural stands in the
            // way of the plan, so what refuses below is what each request names
            &Table::new().with_test("suite:1_x", IntentEvidenceState::NotRun),
        );
        let reviewed = [critique("x", &[]), critique("y", &[]), critique("z", &[])];
        let policy = IntentPolicy::default();
        let ask = |critiques: &[CritiqueRecord], r: BindingRequest| {
            bind(&i, &p, &[], critiques, &policy, r)
        };
        let by_issue = |id: &str| BindingRequest {
            issue: Some(id.into()),
            ..Default::default()
        };
        let by_intent = |id: &str| BindingRequest {
            intent: Some(id.into()),
            ..Default::default()
        };
        let by_path = |path: &str| BindingRequest {
            paths: vec![path.into()],
            ..Default::default()
        };
        let first = |b: &crate::intent_binding::IntentBinding| b.refusals[0].cause;

        // an issue, the intent it serves, and both together are one binding
        let bound = ask(&reviewed, by_issue("I1"));
        assert_eq!(bound.standing, BindingStanding::Bound);
        assert_eq!(bound.standing.as_str(), "bound");
        assert!(!bound.standing.refuses());
        assert_eq!(bound.plan_revision.len(), 64);
        assert_eq!(bound.evidence_standing.len(), 64);
        let named = ask(&reviewed, by_intent("x"));
        assert_eq!(named.standing, BindingStanding::Bound);
        assert_eq!(named.plan_revision, bound.plan_revision);
        let both = ask(
            &reviewed,
            BindingRequest {
                intent: Some("x".into()),
                ..by_issue("I1")
            },
        );
        assert_eq!(both.standing, BindingStanding::Bound);
        // an issue beside an intent it does not serve is a contradiction
        let contradiction = ask(
            &reviewed,
            BindingRequest {
                intent: Some("y".into()),
                ..by_issue("I1")
            },
        );
        assert_eq!(first(&contradiction), BindingCause::IssueOutsideIntent);
        assert!(contradiction.standing.refuses());
        assert_eq!(contradiction.standing.as_str(), "refused");

        // paths that reach two intents are ambiguous; paths under no intent are maintenance
        let ambiguous = ask(&reviewed, by_path("lib/a.rs"));
        assert_eq!(first(&ambiguous), BindingCause::AmbiguousIntent);
        assert!(ambiguous.refusal.unwrap().contains("x, y"));
        let maintenance = ask(&reviewed, by_path("ops/run.sh"));
        assert_eq!(maintenance.standing, BindingStanding::Maintenance);
        assert_eq!(maintenance.standing.as_str(), "maintenance");
        assert!(maintenance.plan_revision.is_empty() && maintenance.evidence_standing.is_empty());
        assert_eq!(BindingStanding::Exempt.as_str(), "exempt");

        // an intent with nothing open to do, and one that was retired
        assert_eq!(
            first(&ask(&reviewed, by_intent("z"))),
            BindingCause::IntentHasNoOpenWork
        );
        let gone = ask(&reviewed, by_intent("r"));
        assert_eq!(first(&gone), BindingCause::IntentRetired);
        assert!(gone.refusal.unwrap().contains("cancelled"));

        // a path the named issue's scope does not cover is said, and refuses nothing
        let noted = ask(
            &reviewed,
            BindingRequest {
                paths: vec!["docs/x.md".into(), "lib/a.rs".into()],
                ..by_issue("I1")
            },
        );
        assert_eq!(noted.standing, BindingStanding::Bound);
        assert_eq!(noted.notes, ["docs/x.md lie outside the scope of I1"]);

        // each of the preflight's causes is the binding's cause, under the same word
        assert_eq!(
            first(&ask(&[], by_issue("I1"))),
            BindingCause::IntentNotCritiqued
        );
        let blocked = [critique("x", &[("F1", true, "open")])];
        assert_eq!(
            first(&ask(&blocked, by_issue("I1"))),
            BindingCause::OpenBlockingFinding
        );
        for (id, cause) in [
            ("I5", BindingCause::IssueServesNothing),
            ("I6", BindingCause::ServesAnotherIntent),
            ("I7", BindingCause::ServesUnknownCriterion),
        ] {
            assert_eq!(first(&ask(&reviewed, by_issue(id))), cause, "{id}");
        }
        for (from, to) in [
            (
                IntentPreflightCause::UnknownIssue,
                BindingCause::UnknownIssue,
            ),
            (
                IntentPreflightCause::NoIssueCoversPaths,
                BindingCause::NoIssueCoversPaths,
            ),
            (
                IntentPreflightCause::IssueServesNothing,
                BindingCause::IssueServesNothing,
            ),
            (
                IntentPreflightCause::ServesAnotherIntent,
                BindingCause::ServesAnotherIntent,
            ),
            (
                IntentPreflightCause::ServesUnknownCriterion,
                BindingCause::ServesUnknownCriterion,
            ),
            (
                IntentPreflightCause::IntentNotCritiqued,
                BindingCause::IntentNotCritiqued,
            ),
            (
                IntentPreflightCause::OpenBlockingFinding,
                BindingCause::OpenBlockingFinding,
            ),
        ] {
            assert_eq!(BindingCause::from(from), to);
            assert_eq!(
                serde_json::to_value(from).unwrap(),
                serde_json::to_value(to).unwrap(),
                "the word changed on the way"
            );
        }
    }
    /// The opposition (ADR 0112) over an in-memory plan: what the engine already found about
    /// one intent is its structural half, once; what a reviewer recorded keeps its
    /// resolution, whatever it is; and the gap's conditions are in the brief and the revision.
    #[test]
    fn the_opposition_selects_what_was_found_and_derives_nothing() {
        use crate::intent_opposition::{
            oppose, revision_of, OppositionDisposition, OppositionReviewState,
        };

        let p = plan(
            vec![milestone("m", "ACTIVE"), milestone("n", "ACTIVE")],
            vec![
                serving("I1", "m", &["lib"], &["x#case"]),
                serving("I3", "n", &["docs"], &["y#case"]),
            ],
        );
        let mut i = Intents::derive(
            vec![record("x", &["m"], &[CASE]), record("y", &["n"], &[CASE])],
            &p,
            &Table::new().with_test("suite:1_x", IntentEvidenceState::NotRun),
        );
        // the same finding said twice — by the engine and by the plan — is one finding;
        // one about another intent is not this intent's; one about the review is not its own
        for (level, code, subject) in [
            (FAIL, "criterion_uncovered", "x#other"),
            (FAIL, "criterion_uncovered", "x#other"),
            (WARN, "criterion_weakly_covered", "x#case"),
            (FAIL, "criterion_uncovered", "y#other"),
            (WARN, "plan_not_critiqued", "x"),
            (FAIL, "scope_conflict", "I1"),
        ] {
            i.findings.push(IntentFinding {
                level: level.into(),
                code: code.into(),
                subject: subject.into(),
                message: "said".into(),
                reproduce: REPRODUCE.into(),
            });
        }
        let gap = GapRecord::from_metadata(
            ".ai/repo/project/gaps/x.yaml",
            &serde_json::json!({"intent": "x", "observed_at": "abc",
                "conditions": [{"criterion": "case", "state": "missing"}]}),
        );
        let reviewed = CritiqueRecord::from_metadata(
            ".ai/repo/project/critiques/x.yaml",
            &serde_json::json!({"intent": "x", "reviewed_at": "c0ffee", "reviewed_by": "r",
            "reviewed_revision": "0000",
            "findings": [
                {"id": "a", "class": "regression_risk", "subject": "x", "finding": "f",
                 "blocking": true, "resolution": {"state": "rejected", "because": "b"}},
                {"id": "b", "class": "regression_risk", "subject": "x", "finding": "f",
                 "blocking": false, "resolution": {"state": "someday"}},
                {"id": "c", "class": "regression_risk", "subject": "x", "finding": "f",
                 "blocking": false, "resolution": {"state": "open"}},
                {"id": "d", "class": "regression_risk", "subject": "x", "finding": "f",
                 "blocking": true, "resolution": {"state": "planned", "issue": "I1"}}
            ]}),
        );
        let gaps = [gap];
        let critiques = [reviewed];
        let o = oppose(&i, &p, &gaps, &critiques, "x").expect("x is declared");

        let structural: Vec<(&str, &str, bool)> = o
            .structural
            .iter()
            .map(|f| (f.id.as_str(), f.subject.as_str(), f.blocking))
            .collect();
        assert_eq!(
            structural,
            [
                ("criterion_uncovered", "x#other", true),
                ("criterion_weakly_covered", "x#case", false),
                ("scope_conflict", "I1", true),
            ]
        );
        let resolutions: Vec<&str> = o.recorded.iter().map(|f| f.resolution.as_str()).collect();
        assert_eq!(resolutions, ["rejected", "someday", "open", "planned"]);
        assert_eq!(o.gap, ["case missing"]);
        assert_eq!(
            o.issues.len(),
            1,
            "only the issues serving x: {:?}",
            o.issues
        );
        assert_eq!(o.disposition, OppositionDisposition::Reject);
        assert_eq!(
            o.rejecting,
            [
                "structural criterion_uncovered x#other",
                "structural scope_conflict I1"
            ]
        );
        // stamped against something that is not this plan
        assert_eq!(o.review.state, OppositionReviewState::Stale);
        assert_eq!(
            revision_of(&i, &p, &gaps, "x").as_deref(),
            Some(o.reviewed_plan.as_str())
        );
        // the gap's conditions are part of what a review judges
        assert_ne!(revision_of(&i, &p, &[], "x"), Some(o.reviewed_plan));
    }
    const OTHER: (&str, &str, &str) = ("other", "test", "test/cases/2_y.sh");

    fn guard(id: &str, reference: &str) -> GuardRecord {
        GuardRecord {
            id: id.into(),
            invariant: format!("{id} stays true"),
            evidence: "test".into(),
            reference: reference.into(),
        }
    }

    fn judged_by(outcome: &str, changed: &[&str]) -> IntentEvaluation {
        IntentEvaluation {
            commit: "c0ffee".into(),
            working_tree: "clean".into(),
            outcome: outcome.into(),
            at: "2026-10-08T00:00:00Z".into(),
            changed: changed.iter().map(|c| c.to_string()).collect(),
            detail: (!changed.is_empty()).then(|| "an input changed since the run".into()),
        }
    }

    /// ADR 0113: a judged criterion carries the run that decided it, an optional criterion
    /// holds neither the verdict nor the stage back, and the stage reads the one verdict.
    #[test]
    fn an_evaluation_travels_and_an_optional_criterion_blocks_nothing() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let mut r = record("x", &["m"], &[CASE, OTHER]);
        r.satisfaction[1].optional = true;
        let mut ev = Table::new()
            .with_test("suite:1_x", IntentEvidenceState::Current)
            .with_test("suite:2_y", IntentEvidenceState::Stale);
        ev.tests.get_mut("suite:1_x").unwrap().evaluation = Some(judged_by("pass", &[]));
        ev.tests.get_mut("suite:2_y").unwrap().evaluation = Some(judged_by("pass", &["lib/a.sh"]));
        let derive = || Intents::derive(vec![r.clone()], &p, &ev);
        let i = derive();
        let x = i.intent("x").unwrap();

        // the required criterion is met, the optional one is stale: satisfied all the same
        assert_eq!(x.verdict.state, IntentVerdictState::Satisfied);
        assert!(
            x.verdict.reasons.is_empty(),
            "an optional criterion is no reason"
        );
        assert_eq!(x.stage, IntentStage::Satisfied);
        assert_eq!((x.met, x.optional, x.satisfaction.len()), (1, 1, 2));
        assert!(x.satisfaction[1].optional && !x.satisfaction[1].met);
        // each carries the run it was judged by, and the stale one what changed
        let judged = x.satisfaction[0]
            .evaluation
            .as_ref()
            .expect("a judged criterion");
        assert_eq!(
            (judged.commit.as_str(), judged.outcome.as_str()),
            ("c0ffee", "pass")
        );
        assert!(judged.changed.is_empty() && judged.detail.is_none());
        let stale = x.satisfaction[1].evaluation.as_ref().unwrap();
        assert_eq!(stale.changed, ["lib/a.sh"]);
        assert_eq!(
            stale.detail.as_deref(),
            Some("an input changed since the run")
        );
        // the same state evaluates to the same answer, every time
        assert_eq!(
            serde_json::to_value(&i.intents).unwrap(),
            serde_json::to_value(&derive().intents).unwrap()
        );
        // on the wire an optional criterion says so, and a required one says nothing
        let wire = serde_json::to_value(x).unwrap();
        assert_eq!(wire["satisfaction"][1]["optional"], true);
        assert!(wire["satisfaction"][0].get("optional").is_none());
        assert_eq!(wire["optional"], 1);
        assert!(wire.get("guards").is_none(), "no guard, no key");
        assert!(wire["verdict"].get("guards").is_none());

        // the required one stops being met: unsatisfied, and the optional one is still no reason
        let failing = Table::new()
            .with_test("suite:1_x", IntentEvidenceState::Failing)
            .with_test("suite:2_y", IntentEvidenceState::Current);
        let i = Intents::derive(vec![r.clone()], &p, &failing);
        let x = i.intent("x").unwrap();
        assert_eq!(x.verdict.state, IntentVerdictState::Unsatisfied);
        assert_eq!(x.verdict.reasons.len(), 1);
        assert_eq!(x.verdict.reasons[0].criterion, "case");
        assert_eq!(x.stage, IntentStage::Verifying);
        assert_eq!(x.met, 0);
        // nothing was judged for a criterion nothing ran for
        let never = Intents::derive(
            vec![r.clone()],
            &p,
            &Table::new()
                .with_test("suite:1_x", IntentEvidenceState::NotRun)
                .with_test("suite:2_y", IntentEvidenceState::NotRun),
        );
        assert!(never.intent("x").unwrap().satisfaction[0]
            .evaluation
            .is_none());

        // every criterion optional: nothing is required, which is refused and unknown
        r.satisfaction[0].optional = true;
        let i = Intents::derive(vec![r], &p, &ev);
        assert!(codes(&i).contains(&"intent_without_required_criterion"));
        let x = i.intent("x").unwrap();
        assert_eq!(x.verdict.state, IntentVerdictState::Unknown);
        assert_eq!(x.stage, IntentStage::Verifying, "unknown is not satisfied");
        assert_eq!((x.met, x.optional), (0, 2));
    }

    /// ADR 0113: a guard is judged like a criterion, only a failing run violates it, and a
    /// violated guard keeps the intent unsatisfied although every criterion is met and every
    /// milestone is DONE.
    #[test]
    fn a_failing_guard_keeps_the_intent_unsatisfied_and_a_stale_one_is_not_judged() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        let mut r = record("x", &["m"], &[CASE]);
        r.guards = vec![guard("stays", "test/cases/2_y.sh")];
        let with = |state| {
            let mut ev = Table::new()
                .with_test("suite:1_x", IntentEvidenceState::Current)
                .with_test("suite:2_y", state);
            ev.tests.get_mut("suite:2_y").unwrap().evaluation = Some(judged_by("fail", &[]));
            Intents::derive(vec![r.clone()], &p, &ev)
        };

        let i = with(IntentEvidenceState::Failing);
        let x = i.intent("x").unwrap();
        assert!(x.satisfaction[0].met, "every criterion is met");
        assert_eq!(x.verdict.state, IntentVerdictState::Unsatisfied);
        assert!(x.verdict.reasons.is_empty(), "no criterion holds it back");
        assert_eq!(x.verdict.guards.len(), 1);
        assert_eq!(x.verdict.guards[0].guard, "stays");
        assert_eq!(x.verdict.guards[0].state, IntentEvidenceState::Failing);
        assert_eq!(
            x.stage,
            IntentStage::Verifying,
            "DONE and met, and still not satisfied"
        );
        assert!(x.guards[0].violated);
        assert_eq!(x.guards[0].invariant, "stays stays true");
        assert_eq!(x.guards[0].evaluation.as_ref().unwrap().outcome, "fail");
        let wire = serde_json::to_value(x).unwrap();
        assert_eq!(wire["verdict"]["guards"][0]["guard"], "stays");
        assert_eq!(wire["guards"][0]["ref"], "test/cases/2_y.sh");

        // it passes again: the guard holds and the intent is satisfied
        let i = with(IntentEvidenceState::Current);
        let x = i.intent("x").unwrap();
        assert_eq!(x.verdict.state, IntentVerdictState::Satisfied);
        assert_eq!(x.stage, IntentStage::Satisfied);
        assert!(!x.guards[0].violated && x.verdict.guards.is_empty());

        // stale and never run are not judged: an unrelated commit violates nothing
        for state in [IntentEvidenceState::Stale, IntentEvidenceState::NotRun] {
            let i = with(state);
            let x = i.intent("x").unwrap();
            assert!(!x.guards[0].violated, "{state:?}");
            assert_eq!(x.verdict.state, IntentVerdictState::Satisfied, "{state:?}");
        }

        // a guard may not take a criterion's id, nor another guard's, and its evidence must resolve
        let mut clash = record("x", &["m"], &[CASE]);
        clash.guards = vec![
            guard("case", "test/cases/2_y.sh"),
            guard("twice", "test/cases/2_y.sh"),
            guard("twice", "test/cases/2_y.sh"),
            guard("nowhere", "not a test"),
        ];
        let i = Intents::derive(
            vec![clash],
            &p,
            &Table::new()
                .with_test("suite:1_x", IntentEvidenceState::Current)
                .with_test("suite:2_y", IntentEvidenceState::Current),
        );
        let found = codes(&i);
        assert_eq!(
            found.iter().filter(|c| **c == "duplicate_guard").count(),
            2,
            "{found:?}"
        );
        assert!(found.contains(&"unresolved_evidence_ref"), "{found:?}");
        let x = i.intent("x").unwrap();
        assert!(!x.guards[3].violated, "an unresolved guard is not judged");
    }

    /// The record's reader: `optional` on a criterion and the `guards` key, and a record
    /// with neither reading as it always did.
    #[test]
    fn a_record_reads_its_optional_criteria_and_its_guards() {
        let r = IntentRecord::from_metadata(
            ".ai/repo/project/intents/x.yaml",
            &serde_json::json!({
                "id": "x", "title": "X", "statement": "s",
                "invariants": ["plain text stays plain"],
                "guards": [{"id": "stays", "invariant": "It stays true", "evidence": "test",
                            "ref": "test/cases/2_y.sh"}],
                "milestones": ["m"],
                "satisfaction": [
                    {"id": "a", "criterion": "A", "evidence": "test", "ref": "test/cases/1_x.sh"},
                    {"id": "b", "criterion": "B", "evidence": "test", "ref": "test/cases/1_x.sh",
                     "optional": true}
                ]
            }),
        );
        assert_eq!(r.invariants, ["plain text stays plain"]);
        assert_eq!(
            r.guards,
            [guard_of("stays", "It stays true", "test/cases/2_y.sh")]
        );
        assert!(!r.satisfaction[0].optional && r.satisfaction[1].optional);
        let plain = IntentRecord::from_metadata(
            ".ai/repo/project/intents/y.yaml",
            &serde_json::json!({"id": "y", "satisfaction": [{"id": "a"}]}),
        );
        assert!(plain.guards.is_empty() && !plain.satisfaction[0].optional);
    }

    fn guard_of(id: &str, invariant: &str, reference: &str) -> GuardRecord {
        GuardRecord {
            id: id.into(),
            invariant: invariant.into(),
            evidence: "test".into(),
            reference: reference.into(),
        }
    }
    /// ADR 0113: making a criterion optional, and adding or changing a guard, change the
    /// plan a review judged and a task was pinned to; a record that declares neither keeps
    /// the revisions it had; and a guard is in the evidence standing only while violated.
    #[test]
    fn a_review_and_a_pin_cover_optional_criteria_and_guards() {
        use crate::intent_binding::{bind, BindingRequest};
        use crate::intent_opposition::revision_of;
        use crate::policy::IntentPolicy;

        let p = plan(
            vec![milestone("m", "ACTIVE")],
            vec![serving("I1", "m", &["lib"], &["x#case"])],
        );
        let reviewed = [critique("x", &[])];
        let ask = |r: IntentRecord, guard_state: IntentEvidenceState| {
            let ev = Table::new()
                .with_test("suite:1_x", IntentEvidenceState::NotRun)
                .with_test("suite:2_y", guard_state);
            let i = Intents::derive(vec![r], &p, &ev);
            let binding = bind(
                &i,
                &p,
                &[],
                &reviewed,
                &IntentPolicy::default(),
                BindingRequest {
                    issue: Some("I1".into()),
                    ..Default::default()
                },
            );
            (revision_of(&i, &p, &[], "x").unwrap(), binding)
        };
        let plain = record("x", &["m"], &[CASE, OTHER]);
        let (review, pinned) = ask(plain.clone(), IntentEvidenceState::NotRun);
        // derived twice, the same
        assert_eq!(ask(plain.clone(), IntentEvidenceState::NotRun).0, review);

        let mut optional = plain.clone();
        optional.satisfaction[0].optional = true;
        let (review_optional, pinned_optional) = ask(optional, IntentEvidenceState::NotRun);
        assert_ne!(
            review_optional, review,
            "optional is part of what a review judges"
        );
        assert_ne!(pinned_optional.plan_revision, pinned.plan_revision);

        let mut guarded = plain.clone();
        guarded.guards = vec![guard("stays", "test/cases/2_y.sh")];
        let (review_guarded, held) = ask(guarded.clone(), IntentEvidenceState::Current);
        assert_ne!(
            review_guarded, review,
            "a guard is part of what a review judges"
        );
        assert_ne!(held.plan_revision, pinned.plan_revision);
        assert_eq!(held.intents[0].guards[0].id, "stays");
        // a guard that holds leaves the evidence standing where it was without one
        assert_eq!(held.evidence_standing, pinned.evidence_standing);
        // and one that starts failing moves it: a resumed worker is told
        let (_, violated) = ask(guarded.clone(), IntentEvidenceState::Failing);
        assert_ne!(violated.evidence_standing, held.evidence_standing);
        assert_eq!(
            violated.plan_revision, held.plan_revision,
            "the plan did not change"
        );
        assert!(violated.intents[0].guards[0].violated);
        // changing what the guard names is a change of the plan
        guarded.guards[0].invariant = "something else stays true".into();
        assert_ne!(ask(guarded, IntentEvidenceState::Current).0, review_guarded);
    }
}
