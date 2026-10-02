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
//! ```
//! use majordomus_cli::intent::IntentStage;
//! assert_eq!(IntentStage::Satisfied.as_str(), "satisfied");
//! assert!(IntentStage::Verifying < IntentStage::Satisfied);
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::evidence::{self, Ledger, ProofState, TestId};
use crate::index::Index;
use crate::intent_plan::{coverage, IntentCoverage, IntentOutline};
use crate::intent_review::{observed_satisfied, review, CritiqueRecord, GapRecord};
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
/// ```
/// use majordomus_cli::intent::IntentEvidenceState;
/// assert_eq!(serde_json::to_string(&IntentEvidenceState::NotRun).unwrap(), "\"not_run\"");
/// assert_ne!(IntentEvidenceState::Stale, IntentEvidenceState::Current);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentEvidenceState {
    /// A passing run of the test that is in the tree now, or a claim proven or with its
    /// inputs unchanged since its passing run. The only state that meets a criterion.
    Current,
    /// A passing run of a test whose source has changed since, or a claim whose inputs
    /// have.
    Stale,
    /// The latest recorded run did not pass.
    Failing,
    /// The reference resolves and nothing has been recorded for it.
    NotRun,
    /// The reference resolves, and this kind of evidence (`command`, `deployment`) is not
    /// derivable from the ledger, so it never meets a criterion by itself.
    NotDerivable,
    /// The reference is empty or names nothing this repository holds.
    Unresolved,
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

/// What one test's recorded evidence says: whether its source is in the tree at all, and what
/// its latest recorded run came to. A test that is not there cannot settle anything.
///
/// ```
/// use majordomus_cli::intent::{IntentEvidenceState, TestStanding};
/// let standing = TestStanding { present: false, state: IntentEvidenceState::Unresolved };
/// assert!(!standing.present);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestStanding {
    /// Whether the test's source is in the tree.
    pub present: bool,
    /// The state of its latest recorded run.
    pub state: IntentEvidenceState,
}

/// The facts the derivation reads beyond the plan. The repository answers them from the
/// ledger, the claim matrix and the tree; a test answers them from a table.
///
/// ```
/// use majordomus_cli::evidence::{ProofState, TestId};
/// use majordomus_cli::intent::{EvidenceLookup, IntentEvidenceState, TestStanding};
/// struct NothingRecorded;
/// impl EvidenceLookup for NothingRecorded {
///     fn test(&self, _: &TestId) -> TestStanding {
///         TestStanding { present: true, state: IntentEvidenceState::NotRun }
///     }
///     fn claim(&self, _: &str) -> Option<ProofState> { None }
///     fn deployment(&self, _: &str) -> bool { false }
///     fn object(&self, _: &str, _: &str) -> bool { false }
///     fn file(&self, _: &str) -> bool { false }
/// }
/// let id = TestId::of("test/cases/00_x.sh").unwrap();
/// assert_eq!(NothingRecorded.test(&id).state, IntentEvidenceState::NotRun);
/// ```
pub trait EvidenceLookup {
    /// What the ledger holds for one test, by its identity: whether its source is in the tree
    /// and what its latest run came to.
    ///
    /// ```
    /// use majordomus_cli::evidence::TestId;
    /// use majordomus_cli::intent::{EvidenceLookup, TestStanding};
    /// # fn demo(ev: &dyn EvidenceLookup) {
    /// let standing: TestStanding = ev.test(&TestId::of("test/cases/00_x.sh").unwrap());
    /// assert!(standing.present || !standing.present);
    /// # }
    /// ```
    fn test(&self, id: &TestId) -> TestStanding;
    /// A claim's proof state, `None` when no such claim is declared — which is a refusal, not
    /// an unproven claim.
    ///
    /// ```
    /// use majordomus_cli::intent::EvidenceLookup;
    /// # fn demo(ev: &dyn EvidenceLookup) {
    /// assert!(ev.claim("no-such-claim-exists").is_none());
    /// # }
    /// ```
    fn claim(&self, id: &str) -> Option<ProofState>;
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
/// its index and its tree. The claim join runs git, so it is computed only when a
/// criterion or a governance entry asks for a claim.
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
    claims: std::cell::OnceCell<BTreeMap<String, ProofState>>,
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
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

impl EvidenceLookup for RepositoryEvidence<'_> {
    fn test(&self, id: &TestId) -> TestStanding {
        let present = self.root().join(id.source()).is_file();
        let state = match self.ledger.latest(&id.as_string()) {
            None => IntentEvidenceState::NotRun,
            Some(e) if !e.outcome.proves() => IntentEvidenceState::Failing,
            Some(e) => match e.digest_matches(self.root()) {
                Some(true) => IntentEvidenceState::Current,
                _ => IntentEvidenceState::Stale,
            },
        };
        TestStanding { present, state }
    }

    fn claim(&self, id: &str) -> Option<ProofState> {
        self.claims
            .get_or_init(|| {
                evidence::report(self.index, &self.ledger)
                    .claims
                    .into_iter()
                    .map(|c| (c.id, c.state))
                    .collect()
            })
            .get(id)
            .copied()
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
/// use majordomus_cli::intent::{IntentCriterion, IntentEvidenceState};
/// let c = IntentCriterion {
///     id: "stage-is-derived".into(),
///     criterion: "the stage follows the plan".into(),
///     evidence: "test".into(),
///     reference: "apps/majordomus-cli/tests/intent.rs".into(),
///     state: IntentEvidenceState::NotRun,
///     met: false,
///     reproduce: None,
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
    /// The command that produces the evidence again, when one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
}

/// One intent, as its record declares it and as the plan and the ledger derive it: the
/// statement and invariants as authored, each milestone with its derived status, each criterion
/// with the state of its evidence, and the stage that follows from both.
///
/// ```
/// use majordomus_cli::intent::{IntentStage, IntentView};
/// let view = IntentView {
///     id: "intent-lifecycle".into(),
///     title: "Work is traceable to the intent it serves".into(),
///     statement: "A reader can ask what work is for.".into(),
///     invariants: vec!["no intent status is stored".into()],
///     stage: IntentStage::Declared,
///     milestones: vec![],
///     satisfaction: vec![],
///     met: 0,
///     governance: vec!["adr:adr-0070".into()],
///     non_goals: vec![],
///     superseded_by: None,
///     source: ".ai/repo/project/intents/intent-lifecycle.yaml".into(),
/// };
/// // a record naming no milestone that resolves has not been planned
/// assert_eq!(view.stage, IntentStage::Declared);
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
    ev: &dyn EvidenceLookup,
    findings: &mut Vec<IntentFinding>,
) -> IntentCriterion {
    let r = c.reference.trim();
    let mut reproduce = None;
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
                    let standing = ev.test(&id);
                    if standing.present {
                        reproduce = Some(id.reproduce());
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
                Some(ProofState::Proven | ProofState::InputsUnchanged) => {
                    IntentEvidenceState::Current
                }
                Some(ProofState::Stale) => IntentEvidenceState::Stale,
                Some(ProofState::Failing) => IntentEvidenceState::Failing,
                Some(_) => IntentEvidenceState::NotRun,
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
        reproduce,
    }
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
        out.findings.extend(coverage(&outlines, plan).findings);
        out.findings
            .extend(review(&outlines, plan, &gaps, &critiques));
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
    /// use majordomus_cli::evidence::{ProofState, TestId};
    /// use majordomus_cli::intent::{
    ///     EvidenceLookup, IntentEvidenceState, IntentRecord, Intents, TestStanding,
    /// };
    /// use majordomus_cli::plan::Plan;
    /// struct NothingRecorded;
    /// impl EvidenceLookup for NothingRecorded {
    ///     fn test(&self, _: &TestId) -> TestStanding {
    ///         TestStanding { present: true, state: IntentEvidenceState::NotRun }
    ///     }
    ///     fn claim(&self, _: &str) -> Option<ProofState> { None }
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
                .map(|c| criterion_of(&r.id, c, ev, &mut findings))
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

            let met = satisfaction.iter().filter(|c| c.met).count();
            let all_met = !satisfaction.is_empty() && met == satisfaction.len();
            let statuses: Vec<Option<String>> =
                milestones.iter().map(|m| m.status.clone()).collect();
            intents.push(IntentView {
                id: r.id.clone(),
                title: r.title.clone(),
                statement: r.statement.clone(),
                invariants: r.invariants.clone(),
                stage: stage(r.cancelled, !r.superseded_by.is_empty(), &statuses, all_met),
                milestones,
                satisfaction,
                met,
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

    /// The intents that name a milestone, in identity order: the link a preflight follows from
    /// an issue's milestone up to what the work is for.
    ///
    /// ```
    /// use majordomus_cli::intent::Intents;
    /// let intents = Intents { intents: vec![], findings: vec![] };
    /// // a milestone no intent names is where a preflight refuses
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

/// One intent a piece of work serves, and the link that says so: issue to milestone to intent,
/// each step named rather than asserted.
///
/// ```
/// use majordomus_cli::intent::{IntentPreflightMatch, IntentStage};
/// let m = IntentPreflightMatch {
///     intent: "intent-lifecycle".into(),
///     title: "Work is traceable to the intent it serves".into(),
///     stage: IntentStage::Executing,
///     milestone: "intent-lifecycle".into(),
///     issue: "I1900".into(),
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
}

/// Which intent a piece of work serves, or why it serves none: the issues it resolved to, every
/// intent reached with its link, the governance those intents load, and — when nothing is
/// reached — the first link that is missing.
///
/// ```
/// use majordomus_cli::intent::IntentPreflight;
/// let refused = IntentPreflight {
///     verdict: "refused".into(),
///     issues: vec![],
///     matches: vec![],
///     governance: vec![],
///     refusal: Some("no open issue's scope covers README.md".into()),
/// };
/// assert_eq!(refused.verdict, "refused");
/// assert!(refused.refusal.is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPreflight {
    /// `serves` or `refused`.
    pub verdict: String,
    /// The issues the work was resolved to: the one named, or the open issues whose scope
    /// covers a path.
    pub issues: Vec<String>,
    /// Every intent reached, with the link.
    pub matches: Vec<IntentPreflightMatch>,
    /// The governance of every intent reached, each entry once, in first-seen order.
    pub governance: Vec<String>,
    /// When refused: the link that is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

impl Intents {
    /// Which intent work on `issue`, or on `paths`, serves.
    ///
    /// ```
    /// use majordomus_cli::intent::Intents;
    /// use majordomus_cli::plan::Plan;
    /// # let plan: Plan = serde_json::from_value(serde_json::json!({
    /// #     "project": {"name": "p", "repository": "o/p", "default_branch": "master",
    /// #                 "active_milestone": ""},
    /// #     "statuses": {"issue": [], "milestone": []},
    /// #     "milestones": [], "issues": [], "waves": [], "edges": [],
    /// #     "milestone_edges": [], "findings": []})).unwrap();
    /// let intents = Intents { intents: vec![], findings: vec![] };
    /// let answer = intents.preflight(&plan, Some("I0001"), &[]);
    /// // an issue this plan does not hold is the first missing link, and it is named
    /// assert_eq!(answer.verdict, "refused");
    /// assert!(answer.refusal.unwrap().contains("I0001"));
    /// ```
    ///
    /// An issue is followed to its milestone and the milestone to the intents that name it.
    /// Paths are followed to the open issues whose scope covers one, then the same way. The
    /// first link that is missing is the refusal.
    pub fn preflight(&self, plan: &Plan, issue: Option<&str>, paths: &[String]) -> IntentPreflight {
        let refused = |issues: Vec<String>, why: String| IntentPreflight {
            verdict: "refused".into(),
            issues,
            matches: Vec::new(),
            governance: Vec::new(),
            refusal: Some(why),
        };
        let issues: Vec<&crate::plan::PlanIssue> = match issue {
            Some(id) => match plan.issue(id) {
                Some(i) => vec![i],
                None => {
                    return refused(
                        Vec::new(),
                        format!("`{id}` is not an issue under .ai/repo/project/issues/"),
                    )
                }
            },
            None => plan
                .issues
                .iter()
                .filter(|i| !matches!(i.status.as_str(), "DONE" | "CANCELLED"))
                .filter(|i| i.scope.iter().any(|s| paths.iter().any(|p| overlap(s, p))))
                .collect(),
        };
        let ids: Vec<String> = issues.iter().map(|i| i.id.clone()).collect();
        if issues.is_empty() {
            return refused(
                ids,
                format!(
                    "no open issue's scope covers {}; work that names no issue serves no \
                     declared intent",
                    paths.join(", ")
                ),
            );
        }
        let mut matches = Vec::new();
        let mut unserved = Vec::new();
        for i in &issues {
            let serving = self.serving(&i.milestone);
            if serving.is_empty() {
                unserved.push(format!("{} (issue {})", i.milestone, i.id));
            }
            for intent in serving {
                matches.push(IntentPreflightMatch {
                    intent: intent.id.clone(),
                    title: intent.title.clone(),
                    stage: intent.stage,
                    milestone: i.milestone.clone(),
                    issue: i.id.clone(),
                });
            }
        }
        if matches.is_empty() {
            return refused(
                ids,
                format!(
                    "the milestone {} serves no declared intent; name it in an intent under \
                     {INTENTS_DIR}/",
                    unserved.join(", ")
                ),
            );
        }
        let mut governance: Vec<String> = Vec::new();
        for m in &matches {
            for g in self
                .intent(&m.intent)
                .map(|i| i.governance.as_slice())
                .unwrap_or_default()
            {
                if !governance.contains(g) {
                    governance.push(g.clone());
                }
            }
        }
        IntentPreflight {
            verdict: "serves".into(),
            issues: ids,
            matches,
            governance,
            refusal: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlanCounts, PlanIssue, PlanMilestone, PlanProject, PlanVocabulary};

    struct Table {
        tests: BTreeMap<String, TestStanding>,
        claims: BTreeMap<String, ProofState>,
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
                },
            );
            self
        }
        fn with_claim(mut self, id: &str, state: ProofState) -> Self {
            self.claims.insert(id.into(), state);
            self
        }
    }

    impl EvidenceLookup for Table {
        fn test(&self, id: &TestId) -> TestStanding {
            self.tests
                .get(&id.as_string())
                .cloned()
                .unwrap_or(TestStanding {
                    present: false,
                    state: IntentEvidenceState::NotRun,
                })
        }
        fn claim(&self, id: &str) -> Option<ProofState> {
            self.claims.get(id).copied()
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

    #[test]
    fn evidence_satisfies_nothing_while_a_milestone_is_open() {
        let p = plan(vec![milestone("m", "ACTIVE")], vec![]);
        let ev = Table::new().with_test("suite:1_x", IntentEvidenceState::Current);
        let i = Intents::derive(vec![record("x", &["m"], &[CASE])], &p, &ev);
        assert_eq!(i.intents[0].met, 1, "the criterion itself is met");
        assert_eq!(i.intents[0].stage, IntentStage::Executing);
    }

    #[test]
    fn a_claim_is_current_only_when_proven_or_its_inputs_are_unchanged() {
        let p = plan(vec![milestone("m", "DONE")], vec![]);
        for (state, want) in [
            (ProofState::Proven, IntentEvidenceState::Current),
            (ProofState::InputsUnchanged, IntentEvidenceState::Current),
            (ProofState::Stale, IntentEvidenceState::Stale),
            (ProofState::Failing, IntentEvidenceState::Failing),
            (ProofState::NotRun, IntentEvidenceState::NotRun),
        ] {
            let ev = Table::new().with_claim("c", state);
            let i = Intents::derive(vec![record("x", &["m"], &[("k", "claim", "c")])], &p, &ev);
            assert_eq!(i.intents[0].satisfaction[0].state, want, "{state:?}");
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

    #[test]
    fn preflight_follows_issue_to_milestone_to_intent_and_names_the_missing_link() {
        let p = plan(
            vec![milestone("m", "ACTIVE"), milestone("lonely", "ACTIVE")],
            vec![
                issue("I1", "m", "ACTIVE", &["apps/cli/src"]),
                issue("I2", "lonely", "READY", &["lib"]),
                issue("I3", "m", "DONE", &["docs"]),
            ],
        );
        let mut x = record("x", &["m"], &[CASE]);
        x.governance = vec!["rule:project.alpha".into()];
        let i = Intents::derive(vec![x], &p, &Table::new());

        let by_issue = i.preflight(&p, Some("I1"), &[]);
        assert_eq!(by_issue.verdict, "serves");
        assert_eq!(by_issue.matches[0].intent, "x");
        assert_eq!(by_issue.governance, ["rule:project.alpha"]);

        let by_path = i.preflight(&p, None, &["apps/cli/src/main.rs".into()]);
        assert_eq!(by_path.issues, ["I1"]);
        assert_eq!(by_path.verdict, "serves");

        let unserved = i.preflight(&p, Some("I2"), &[]);
        assert_eq!(unserved.verdict, "refused");
        assert!(unserved.refusal.unwrap().contains("lonely"));

        let unknown = i.preflight(&p, Some("I404"), &[]);
        assert!(unknown.refusal.unwrap().contains("I404"));

        // a closed issue's scope claims nothing
        let closed = i.preflight(&p, None, &["docs/X.md".into()]);
        assert_eq!(closed.verdict, "refused");
        assert!(closed.refusal.unwrap().contains("no open issue"));
    }
}
