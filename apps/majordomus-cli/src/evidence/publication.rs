//! The site's evidence publication: an allow-listed projection of the ledger committed at the
//! presented commit (ADR 0087, decisions 3 and 14; I1955).
//!
//! `majordomus evidence publish` runs in the build's checkout of the presented commit P. It
//! judges every claim from the ledger P holds, exactly as `evidence show --presented` does,
//! then reads the run records it is given (P's own CI run record, or an ancestor's) through
//! the one monotone rule, which can withhold `proven` and never grant it. What it writes is
//! an [`EvidencePublication`], and four things hold of it:
//!
//! * **one state**, the first matching row of decision 3's publication table ([`decide`]):
//!   `dirty`, `stale`, `current`, `pending`, `unknown`. Whatever row matched, every record
//!   that capped a verdict is named beside it, so a dirty build that a record also
//!   contradicts reads `dirty` and still names that record;
//! * **only allow-listed fields**: the document is projected through [`ALLOW_LIST`] by
//!   [`project`], and a field the list does not name is not published, whatever the type
//!   carries;
//! * **every text through the one redactor**: each string is passed through
//!   [`published_text`], which is `crate::redaction::public_text`; a text the redactor
//!   refuses is replaced by [`WITHHELD`] and counted, and the rendered artifact still passes
//!   `generate::forbidden_in` ([`render`]);
//! * **no verdict stronger than the ledger committed at P**: a claim's state is the committed
//!   ledger's, weakened or left as it is, never strengthened.
//!
//! ```
//! use majordomus_cli::evidence::freshness::TreeState;
//! use majordomus_cli::evidence::publication::{decide, Observed, PublicationState, Run};
//!
//! // a record of an ancestor only can cap a verdict and nothing else: it never makes the
//! // publication current, so a clean build with no record of its own and a concluded run
//! // is unknown
//! let observed = Observed {
//!     presented_tree: TreeState::Clean,
//!     capping: vec![],
//!     own: vec![],
//!     run: Some(Run::Concluded),
//! };
//! assert_eq!(decide(&observed).0, PublicationState::Unknown);
//! ```

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::freshness::{changed_between, Judgement, Presented, TreeState};
use super::run::EvidenceRunRecord;
use super::supplementary::weaken_naming;
use super::{EvidenceReport, Origin, ProofState};
use crate::git::Containment;

/// The version of the [`EvidencePublication`] document.
pub const SCHEMA: u32 = 1;

/// The file `evidence publish` writes into its `--out` directory.
pub const FILE: &str = "evidence-publication.json";

/// What a text the redactor refuses is published as: a fixed sentence, which names neither
/// the text nor what the redactor found in it, since either could be the secret itself.
pub const WITHHELD: &str = "[withheld: the redactor could not make this text safe to publish]";

/// Every field the publication may carry, by its path in the document: `a.b` for a member of
/// an object, `a[]` for each element of an array, and `a.*` for every member of a map whose
/// keys are data. A field whose path is not here is not published ([`project`]).
///
/// This is the one declaration of what the site may know about evidence (ADR 0087, decision
/// 14). It names no execution, no command, no claim sentence and no captured output.
///
/// ```
/// use majordomus_cli::evidence::publication::ALLOW_LIST;
/// assert!(ALLOW_LIST.contains(&"state"));
/// assert!(ALLOW_LIST.contains(&"claims[].state"));
/// // no execution is published: the ledger's count of them is the one field that names one
/// let named: Vec<&&str> = ALLOW_LIST.iter().filter(|f| f.contains("execution")).collect();
/// assert_eq!(named, [&"ledger.executions"]);
/// ```
pub const ALLOW_LIST: &[&str] = &[
    "schema",
    "presented.commit",
    "presented.tree",
    "state",
    "reasons[]",
    "run",
    "ledger.path",
    "ledger.executions",
    "ledger.newest",
    "records[].id",
    "records[].origin",
    "records[].commit",
    "records[].relation",
    "records[].working_tree",
    "records[].recorded_at",
    "records[].url",
    "records[].commits_between",
    "records[].changed[]",
    "records[].capped[]",
    "capped_by[]",
    "claims[].id",
    "claims[].state",
    "claims[].test",
    "claims[].detail",
    "claims[].capped_by[]",
    "totals.*",
    "unread[].file",
    "unread[].reason",
    "withheld",
];

/// The one state of a publication: the first matching row of ADR 0087 decision 3's
/// publication table. These are site-level words, not proof states.
///
/// ```
/// use majordomus_cli::evidence::publication::PublicationState;
/// let words: Vec<&str> = PublicationState::ALL.iter().map(|s| s.as_str()).collect();
/// assert_eq!(words, ["dirty", "stale", "current", "pending", "unknown"]);
/// assert_eq!(serde_json::to_value(PublicationState::Pending).unwrap(), "pending");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidencePublicationState")]
pub enum PublicationState {
    /// Row 1: the presented tree is not clean (`build.json.dirty` is not `false`).
    Dirty,
    /// Row 2: a run record read, P's own or an ancestor's, caps at least one verdict.
    Stale,
    /// Row 3: P's own run record is read, and every tree it carries was measured clean.
    Current,
    /// Row 4: P's own run record is not read, and P's CI run has not concluded.
    Pending,
    /// Row 5: anything else, naming which.
    Unknown,
}

impl PublicationState {
    /// The five states in the table's row order, which is the order they are tried in.
    pub const ALL: [PublicationState; 5] = [
        PublicationState::Dirty,
        PublicationState::Stale,
        PublicationState::Current,
        PublicationState::Pending,
        PublicationState::Unknown,
    ];

    /// The word, as the document spells it.
    ///
    /// ```
    /// use majordomus_cli::evidence::publication::PublicationState;
    /// assert_eq!(PublicationState::Current.as_str(), "current");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            PublicationState::Dirty => "dirty",
            PublicationState::Stale => "stale",
            PublicationState::Current => "current",
            PublicationState::Pending => "pending",
            PublicationState::Unknown => "unknown",
        }
    }
}

/// What the publisher observed of P's CI run: the one input the table needs that no file
/// in the checkout holds.
///
/// ```
/// use majordomus_cli::evidence::publication::Run;
/// let r: Run = serde_json::from_str(r#""concluded""#).unwrap();
/// assert_eq!(r, Run::Concluded);
/// assert_eq!(Run::Absent.as_str(), "absent");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidencePublicationRun")]
pub enum Run {
    /// P's CI run has not concluded yet.
    Pending,
    /// P's CI run has concluded, whatever its conclusion.
    Concluded,
    /// No CI run of P is found.
    Absent,
}

impl Run {
    /// The word, as the input and the document spell it.
    ///
    /// ```
    /// use majordomus_cli::evidence::publication::Run;
    /// assert_eq!(Run::Pending.as_str(), "pending");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Run::Pending => "pending",
            Run::Concluded => "concluded",
            Run::Absent => "absent",
        }
    }
}

/// Where a run record stands against the presented commit.
///
/// ```
/// use majordomus_cli::evidence::publication::Relation;
/// assert_eq!(serde_json::to_value(Relation::Ancestor).unwrap(), "ancestor");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidencePublicationRelation")]
pub enum Relation {
    /// Recorded at the presented commit itself.
    Own,
    /// Recorded at a commit the presented commit contains: it can cap a verdict and
    /// nothing else.
    Ancestor,
    /// Recorded at a commit outside the presented history, or one this clone cannot place:
    /// listed, and it says nothing about this history.
    Unrelated,
}

/// The revision the publication is about: `build.json.commit` and its tree.
///
/// ```
/// use majordomus_cli::evidence::freshness::TreeState;
/// use majordomus_cli::evidence::publication::PublishedPresented;
/// let p = PublishedPresented { commit: "c".repeat(40), tree: TreeState::Clean };
/// assert_eq!(serde_json::to_value(&p).unwrap()["tree"], "clean");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidencePublicationPresented")]
pub struct PublishedPresented {
    /// The full id of the presented commit.
    pub commit: String,
    /// Its tree as measured, weakened by what the caller knew.
    pub tree: TreeState,
}

/// The ledger the verdicts were read from: the one committed at the presented commit.
///
/// ```
/// use majordomus_cli::evidence::publication::PublishedLedger;
/// let l = PublishedLedger { path: ".ai/repo/evidence/ledger.json".into(), executions: 0, newest: None };
/// assert!(serde_json::to_value(&l).unwrap().get("newest").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidencePublicationLedger")]
pub struct PublishedLedger {
    /// Where it lives, repository-relative.
    pub path: String,
    /// How many executions it holds.
    pub executions: usize,
    /// Its newest `at`, when it holds any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest: Option<String>,
}

/// One run record read, listed whether or not it capped anything.
///
/// ```
/// use majordomus_cli::evidence::freshness::TreeState;
/// use majordomus_cli::evidence::publication::{PublishedRecord, Relation};
/// use majordomus_cli::evidence::Origin;
/// let r = PublishedRecord {
///     id: "ci:github:1:1".into(), origin: Origin::Ci, commit: "a".repeat(40),
///     relation: Relation::Ancestor, working_tree: TreeState::Clean,
///     recorded_at: "2026-10-22T12:00:00Z".into(), url: None, commits_between: Some(2),
///     changed: vec!["docs/A.md".into()], capped: vec![],
/// };
/// assert_eq!(serde_json::to_value(&r).unwrap()["commits_between"], 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidencePublicationRecord")]
pub struct PublishedRecord {
    /// The record's id.
    pub id: String,
    /// Where its run happened.
    pub origin: Origin,
    /// The commit it was recorded at.
    pub commit: String,
    /// Where that commit stands against the presented one.
    pub relation: Relation,
    /// The weakest tree it carries: its own, each report's and its coverage's.
    pub working_tree: TreeState,
    /// When it was recorded, RFC 3339 UTC.
    pub recorded_at: String,
    /// Where a reader finds its CI run, when it was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// For an ancestor's record: how many commits lie between it and the presented commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commits_between: Option<usize>,
    /// For an ancestor's record: the files changed between it and the presented commit.
    #[serde(default)]
    pub changed: Vec<String>,
    /// The claims whose verdict this record capped through the monotone rule.
    #[serde(default)]
    pub capped: Vec<String>,
}

/// One claim's verdict as published: the committed ledger's, or weaker.
///
/// ```
/// use majordomus_cli::evidence::publication::PublishedClaim;
/// use majordomus_cli::evidence::ProofState;
/// let c = PublishedClaim {
///     id: "policy-parse".into(), state: ProofState::Stale, test: Some("suite:00_x".into()),
///     detail: None, capped_by: vec!["ci:github:1:1".into()],
/// };
/// assert_eq!(serde_json::to_value(&c).unwrap()["state"], "stale");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidencePublicationClaim")]
pub struct PublishedClaim {
    /// The claim's id.
    pub id: String,
    /// Its verdict at the presented commit.
    pub state: ProofState,
    /// The test that carries it, when a runner owns one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    /// Why the state is what it is, when the state alone does not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The records that capped it, each by its id.
    #[serde(default)]
    pub capped_by: Vec<String>,
}

/// A file of the source that is not a run record, and why.
///
/// ```
/// use majordomus_cli::evidence::publication::PublishedUnread;
/// let u = PublishedUnread { file: "manifest.json".into(), reason: "not a run record".into() };
/// assert_eq!(serde_json::to_value(&u).unwrap()["file"], "manifest.json");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidencePublicationUnread")]
pub struct PublishedUnread {
    /// The file, relative to the source it was read from.
    pub file: String,
    /// Why it was not read.
    pub reason: String,
}

/// What the site publishes about evidence: the one state, why, the records read, and every
/// claim's verdict at the presented commit. Every field is on [`ALLOW_LIST`].
///
/// ```
/// use majordomus_cli::evidence::freshness::TreeState;
/// use majordomus_cli::evidence::publication::{
///     EvidencePublication, PublicationState, PublishedLedger, PublishedPresented,
/// };
/// let p = EvidencePublication {
///     schema: 1,
///     presented: PublishedPresented { commit: "c".repeat(40), tree: TreeState::Dirty },
///     state: PublicationState::Dirty,
///     reasons: vec!["the presented tree is dirty".into()],
///     run: None,
///     ledger: PublishedLedger { path: ".ai/repo/evidence/ledger.json".into(), executions: 0, newest: None },
///     records: vec![], capped_by: vec![], claims: vec![], totals: Default::default(),
///     unread: vec![], withheld: 0,
/// };
/// assert_eq!(serde_json::to_value(&p).unwrap()["state"], "dirty");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidencePublication {
    /// [`SCHEMA`].
    pub schema: u32,
    /// The commit published, and its tree.
    pub presented: PublishedPresented,
    /// The first matching row of the publication table.
    pub state: PublicationState,
    /// Why that row matched, and every record that capped a verdict, whatever row matched.
    pub reasons: Vec<String>,
    /// What the publisher observed of P's CI run, when it said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Run>,
    /// The ledger the verdicts were read from.
    pub ledger: PublishedLedger,
    /// Every run record read, in the canonical order of when it was recorded.
    pub records: Vec<PublishedRecord>,
    /// The records that capped at least one verdict, in the order of `records`.
    pub capped_by: Vec<String>,
    /// Every claim, in the matrix's order.
    pub claims: Vec<PublishedClaim>,
    /// How many claims are in each state.
    pub totals: BTreeMap<String, usize>,
    /// The files of the source that are not run records.
    pub unread: Vec<PublishedUnread>,
    /// How many texts the redactor refused, each published as [`WITHHELD`].
    pub withheld: usize,
}

/// What the publication observes, as the table reads it.
///
/// ```
/// use majordomus_cli::evidence::freshness::TreeState;
/// use majordomus_cli::evidence::publication::Observed;
/// let o = Observed { presented_tree: TreeState::Clean, capping: vec![], own: vec![], run: None };
/// assert!(o.own.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    /// The presented tree: `build.json.dirty` as the caller gave it, weakening the checkout
    /// as measured.
    pub presented_tree: TreeState,
    /// The records, P's own or an ancestor's, that capped at least one verdict.
    pub capping: Vec<String>,
    /// P's own records read, each with the weakest tree it carries.
    pub own: Vec<(String, TreeState)>,
    /// What the publisher observed of P's CI run.
    pub run: Option<Run>,
}

/// The publication table of ADR 0087 decision 3: the first matching row wins, with the
/// reasons it matched. Whatever row matched, the records that capped a verdict are named.
///
/// ```
/// use majordomus_cli::evidence::freshness::TreeState;
/// use majordomus_cli::evidence::publication::{decide, Observed, PublicationState, Run};
///
/// let mut o = Observed {
///     presented_tree: TreeState::Clean,
///     capping: vec![],
///     own: vec![("ci:github:1:1".into(), TreeState::Clean)],
///     run: Some(Run::Concluded),
/// };
/// assert_eq!(decide(&o).0, PublicationState::Current);
///
/// // a dirty build that a record also contradicts reads dirty, and names the record
/// o.presented_tree = TreeState::Dirty;
/// o.capping = vec!["ci:github:1:1".into()];
/// let (state, reasons) = decide(&o);
/// assert_eq!(state, PublicationState::Dirty);
/// assert!(reasons.iter().any(|r| r.contains("ci:github:1:1")));
/// ```
pub fn decide(o: &Observed) -> (PublicationState, Vec<String>) {
    let capped = (!o.capping.is_empty()).then(|| {
        format!(
            "verdicts capped through the monotone rule by the run record(s) {}",
            o.capping.join(", ")
        )
    });
    // row 1
    if o.presented_tree != TreeState::Clean {
        let mut reasons = vec![format!(
            "the presented tree is {}: the build is not its commit as committed, so nothing it \
             shows is current",
            o.presented_tree.as_str()
        )];
        reasons.extend(capped);
        return (PublicationState::Dirty, reasons);
    }
    // row 2
    if let Some(capped) = capped {
        return (PublicationState::Stale, vec![capped]);
    }
    // row 3
    let unclean: Vec<String> = o
        .own
        .iter()
        .filter(|(_, tree)| *tree != TreeState::Clean)
        .map(|(id, tree)| {
            format!(
                "the presented commit's run record {id} carries a {} tree",
                tree.as_str()
            )
        })
        .collect();
    if !o.own.is_empty() && unclean.is_empty() {
        let ids: Vec<&str> = o.own.iter().map(|(id, _)| id.as_str()).collect();
        return (
            PublicationState::Current,
            vec![format!(
                "the presented commit's run record {} is read, and every tree it carries was \
                 measured clean",
                ids.join(", ")
            )],
        );
    }
    // row 4
    if o.own.is_empty() && o.run == Some(Run::Pending) {
        return (
            PublicationState::Pending,
            vec![
                "no run record of the presented commit is read, and its CI run has not \
                 concluded"
                    .to_string(),
            ],
        );
    }
    // row 5, naming which
    let mut reasons = unclean;
    if o.own.is_empty() {
        reasons.push(match o.run {
            Some(Run::Concluded) => "the presented commit's CI run concluded without a run \
                record read: its evidence job skipped, was cancelled or failed, or its record \
                is unreachable"
                .to_string(),
            Some(Run::Absent) => "no CI run of the presented commit is found".to_string(),
            // `Pending` matched row 4 above; what is left is a run nobody described
            _ => "no run record of the presented commit is read, and what became of its CI \
                run was not given"
                .to_string(),
        });
    }
    (PublicationState::Unknown, reasons)
}

/// The weakest tree a run record carries: its own, each report's measurement and its
/// coverage's. A record is clean only when every one of them is.
///
/// ```
/// use majordomus_cli::evidence::publication::tree_of;
/// use majordomus_cli::evidence::{EvidenceRunRecord, TreeState};
/// let r: EvidenceRunRecord = serde_json::from_value(serde_json::json!({
///     "schema": 1, "id": "x", "origin": "ci", "commit": "a".repeat(40),
///     "working_tree": "clean", "recorded_at": "2026-10-22T12:00:00Z", "ledger": "repo"
/// })).unwrap();
/// assert_eq!(tree_of(&r), TreeState::Clean);
/// ```
pub fn tree_of(record: &EvidenceRunRecord) -> TreeState {
    record
        .provenance
        .iter()
        .map(|p| p.working_tree)
        .chain(record.coverage.iter().map(|c| c.working_tree))
        .fold(record.working_tree, TreeState::weaker)
}

/// Keep only what [`ALLOW_LIST`] names: a member of an object whose path is not on the list,
/// or under a path on it, is dropped, and so is everything below it.
///
/// ```
/// use majordomus_cli::evidence::publication::project;
/// let doc = serde_json::json!({
///     "state": "current",
///     "claims": [{ "id": "a", "state": "proven", "execution": { "command": "x" } }],
///     "secret": "not on the list",
/// });
/// let kept = project(&doc);
/// assert_eq!(kept, serde_json::json!({ "state": "current", "claims": [{ "id": "a", "state": "proven" }] }));
/// ```
pub fn project(document: &Value) -> Value {
    keep(document, "")
}

/// The part of `value`, found at `path`, that the list allows.
fn keep(value: &Value, path: &str) -> Value {
    match value {
        Value::Object(members) => {
            let mut kept = Map::new();
            for (key, member) in members {
                let named = child(path, key);
                let any = child(path, "*");
                let at = [named, any].into_iter().find(|p| listed(p));
                if let Some(at) = at {
                    kept.insert(key.clone(), keep(member, &at));
                }
            }
            Value::Object(kept)
        }
        Value::Array(items) => {
            let each = format!("{path}[]");
            Value::Array(items.iter().map(|item| keep(item, &each)).collect())
        }
        scalar => scalar.clone(),
    }
}

/// The path of a member `key` of the object at `path`.
fn child(path: &str, key: &str) -> String {
    match path {
        "" => key.to_string(),
        _ => format!("{path}.{key}"),
    }
}

/// Is `path` on the list, or a container of something on it?
fn listed(path: &str) -> bool {
    let (member, element) = (format!("{path}."), format!("{path}[]"));
    ALLOW_LIST
        .iter()
        .any(|a| *a == path || a.starts_with(&member) || a.starts_with(&element))
}

/// A text fit to publish, through the one redactor: machine paths normalised, credentials
/// redacted, and refused (`None`) when what is left is still not safe.
///
/// ```
/// use majordomus_cli::evidence::publication::published_text;
/// let dir = tempfile::tempdir().unwrap();
/// let token = format!("{}{}", "ghp_", "a".repeat(24));
/// let text = format!("{}/target pushed with {token}", dir.path().display());
/// assert_eq!(
///     published_text(&text, dir.path(), None).as_deref(),
///     Some("<repo>/target pushed with [redacted:github-token]")
/// );
/// let elsewhere = format!("{}{}", "/ho", "me/someone/.cache");
/// assert_eq!(published_text(&elsewhere, dir.path(), None), None);
/// ```
pub fn published_text(text: &str, root: &Path, home: Option<&Path>) -> Option<String> {
    crate::redaction::public_text(text, root, home)
        .ok()
        .map(|r| r.text)
}

/// Every string of `value` through [`published_text`]; a refused one becomes [`WITHHELD`].
/// Returns how many were refused.
fn redact(value: &mut Value, root: &Path, home: Option<&Path>) -> usize {
    match value {
        Value::String(text) => match published_text(text, root, home) {
            Some(safe) => {
                *text = safe;
                0
            }
            None => {
                *text = WITHHELD.to_string();
                1
            }
        },
        Value::Array(items) => items.iter_mut().map(|v| redact(v, root, home)).sum(),
        Value::Object(members) => members.values_mut().map(|v| redact(v, root, home)).sum(),
        _ => 0,
    }
}

/// One claim's verdict after the monotone rule over `records`, and the records that capped
/// it, each found by applying the rule to that record alone: the one application a subject's
/// answer makes as well ([`super::supplementary::weaken_naming`]).
fn capped(
    claim: &super::ClaimProof,
    records: &[EvidenceRunRecord],
    presented: &str,
    contains: &dyn Fn(&str, &str) -> Containment,
) -> (Judgement, Vec<String>) {
    let judgement = Judgement {
        state: claim.state,
        changed: claim.changed.clone(),
        detail: claim.detail.clone(),
    };
    weaken_naming(
        judgement,
        claim.execution.as_ref(),
        records,
        Some(presented),
        contains,
    )
}

/// The publication of `report` (the claims judged from the ledger committed at the
/// presented commit) with the run records read, before anything is written.
///
/// The verdicts are the report's, weakened through the monotone rule by `records` and never
/// strengthened; the state is [`decide`]'s; the document is projected through
/// [`ALLOW_LIST`] and every text passed through [`published_text`]. Refused when the report
/// presents no commit.
///
/// ```
/// use majordomus_cli::evidence::freshness::{presented_commit, ledger_at};
/// use majordomus_cli::evidence::publication::{publication, PublicationState, Run};
/// use majordomus_cli::evidence::report_at;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// use std::process::Command;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let git = |args: &[&str]| {
///     Command::new("git").arg("-C").arg(repo.root()).args(args).output().unwrap()
/// };
/// std::fs::write(repo.root().join(".gitignore"), ".ai/local/\n").unwrap();
/// git(&["init", "-q"]);
/// git(&["config", "user.email", "t@example.com"]);
/// git(&["config", "user.name", "t"]);
/// git(&["add", "-A"]);
/// git(&["commit", "-qm", "init"]);
/// let ctx = repo.context().unwrap();
/// // a clean build, no record of its own, and a run that has not concluded
/// let presented = presented_commit(repo.root(), "HEAD", None).unwrap();
/// let ledger = ledger_at(repo.root(), presented.commit().unwrap()).unwrap();
/// let report = report_at(&ctx.index, &ledger, &presented, &[]);
/// let p = publication(repo.root(), &report, &presented, &[], vec![], Some(Run::Pending)).unwrap();
/// assert_eq!(p.state, PublicationState::Pending);
/// ```
pub fn publication(
    root: &Path,
    report: &EvidenceReport,
    presented: &Presented,
    records: &[EvidenceRunRecord],
    unread: Vec<PublishedUnread>,
    run: Option<Run>,
) -> Result<EvidencePublication, String> {
    let commit = presented
        .commit()
        .ok_or("a publication is of a commit, and none is presented")?
        .to_string();
    let cache: RefCell<BTreeMap<(String, String), Containment>> = RefCell::new(BTreeMap::new());
    let contains = |descendant: &str, ancestor: &str| {
        *cache
            .borrow_mut()
            .entry((descendant.to_string(), ancestor.to_string()))
            .or_insert_with(|| crate::git::contains(root, descendant, ancestor))
    };
    let mut ordered: Vec<EvidenceRunRecord> = records.to_vec();
    crate::order::canonical(&mut ordered);

    let mut claims = Vec::new();
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut capped_claims: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in &report.claims {
        let (j, by) = capped(c, &ordered, &commit, &contains);
        for id in &by {
            capped_claims
                .entry(id.clone())
                .or_default()
                .push(c.id.clone());
        }
        *totals.entry(j.state.label().to_string()).or_insert(0) += 1;
        claims.push(PublishedClaim {
            id: c.id.clone(),
            state: j.state,
            test: c.test.clone(),
            detail: j.detail,
            capped_by: by,
        });
    }

    let mut published = Vec::new();
    let mut own = Vec::new();
    for r in &ordered {
        let relation = if r.commit == commit {
            Relation::Own
        } else if contains(&commit, &r.commit) == Containment::Contains {
            Relation::Ancestor
        } else {
            Relation::Unrelated
        };
        let tree = tree_of(r);
        if relation == Relation::Own {
            own.push((r.id.clone(), tree));
        }
        let (commits_between, changed) = match relation {
            Relation::Ancestor => (
                commits_between(root, &r.commit, &commit),
                changed_between(root, &r.commit, presented)
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
            ),
            Relation::Own | Relation::Unrelated => (None, Vec::new()),
        };
        published.push(PublishedRecord {
            id: r.id.clone(),
            origin: r.origin,
            commit: r.commit.clone(),
            relation,
            working_tree: tree,
            recorded_at: r.recorded_at.clone(),
            url: r.run.as_ref().map(|x| x.url.clone()),
            commits_between,
            changed,
            capped: capped_claims.get(&r.id).cloned().unwrap_or_default(),
        });
    }
    let capping: Vec<String> = published
        .iter()
        .filter(|r| !r.capped.is_empty())
        .map(|r| r.id.clone())
        .collect();
    let observed = Observed {
        presented_tree: presented.tree(),
        capping: capping.clone(),
        own,
        run,
    };
    let (state, reasons) = decide(&observed);
    let typed = EvidencePublication {
        schema: SCHEMA,
        presented: PublishedPresented {
            commit,
            tree: presented.tree(),
        },
        state,
        reasons,
        run,
        ledger: PublishedLedger {
            path: report.ledger.path.clone(),
            executions: report.ledger.executions,
            newest: report.ledger.newest.clone(),
        },
        records: published,
        capped_by: capping,
        claims,
        totals,
        unread,
        withheld: 0,
    };
    let document = serde_json::to_value(&typed).map_err(|e| e.to_string())?;
    let mut projected = project(&document);
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let withheld = redact(&mut projected, root, home.as_deref());
    projected["withheld"] = Value::from(withheld);
    serde_json::from_value(projected).map_err(|e| {
        format!("the projection dropped a field the publication requires, which is off the allow-list: {e}")
    })
}

/// How many commits lie between `ancestor` and `descendant`; `None` when git cannot say.
fn commits_between(root: &Path, ancestor: &str, descendant: &str) -> Option<usize> {
    let out = crate::git::read_only(root)
        .args(["rev-list", "--count", &format!("{ancestor}..{descendant}")])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// The publication as the file carries it, refused when it still holds anything
/// `generate::forbidden_in` refuses on a published artifact.
///
/// ```
/// use majordomus_cli::evidence::freshness::TreeState;
/// use majordomus_cli::evidence::publication::{
///     render, EvidencePublication, PublicationState, PublishedLedger, PublishedPresented,
/// };
/// let mut p = EvidencePublication {
///     schema: 1,
///     presented: PublishedPresented { commit: "c".repeat(40), tree: TreeState::Clean },
///     state: PublicationState::Unknown,
///     reasons: vec![],
///     run: None,
///     ledger: PublishedLedger { path: ".ai/repo/evidence/ledger.json".into(), executions: 0, newest: None },
///     records: vec![], capped_by: vec![], claims: vec![], totals: Default::default(),
///     unread: vec![], withheld: 0,
/// };
/// assert!(render(&p).unwrap().ends_with("}\n"));
/// p.reasons.push(format!("{}{}", "/Us", "ers/someone/dev"));
/// assert!(render(&p).unwrap_err().contains("not published"));
/// ```
pub fn render(publication: &EvidencePublication) -> Result<String, String> {
    let text = serde_json::to_string_pretty(publication).map_err(|e| e.to_string())? + "\n";
    match crate::generate::forbidden_in(&text) {
        None => Ok(text),
        Some((marker, what)) => Err(format!(
            "the publication carries {what} (`{marker}`), so it is not published"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row of the table, and the order they are tried in: an earlier row wins over
    /// every later one that also matches.
    #[test]
    fn the_first_matching_row_wins() {
        let clean = |id: &str| (id.to_string(), TreeState::Clean);
        let o = |tree, capping: &[&str], own: Vec<(String, TreeState)>, run| Observed {
            presented_tree: tree,
            capping: capping.iter().map(|s| s.to_string()).collect(),
            own,
            run,
        };
        let cases = [
            (
                o(TreeState::Unknown, &[], vec![clean("r")], None),
                PublicationState::Dirty,
            ),
            (
                o(TreeState::Dirty, &["r"], vec![], None),
                PublicationState::Dirty,
            ),
            (
                o(TreeState::Clean, &["r"], vec![clean("r")], None),
                PublicationState::Stale,
            ),
            (
                o(TreeState::Clean, &[], vec![clean("r")], None),
                PublicationState::Current,
            ),
            (
                o(TreeState::Clean, &[], vec![], Some(Run::Pending)),
                PublicationState::Pending,
            ),
            (
                o(
                    TreeState::Clean,
                    &[],
                    vec![("r".into(), TreeState::Dirty)],
                    Some(Run::Pending),
                ),
                PublicationState::Unknown,
            ),
            (
                o(TreeState::Clean, &[], vec![], Some(Run::Concluded)),
                PublicationState::Unknown,
            ),
            (
                o(TreeState::Clean, &[], vec![], Some(Run::Absent)),
                PublicationState::Unknown,
            ),
            (
                o(TreeState::Clean, &[], vec![], None),
                PublicationState::Unknown,
            ),
        ];
        for (observed, want) in cases {
            let (state, reasons) = decide(&observed);
            assert_eq!(state, want, "{observed:?}");
            assert!(!reasons.is_empty(), "{observed:?} gave no reason");
        }
        let (_, reasons) = decide(&o(TreeState::Clean, &[], vec![], Some(Run::Absent)));
        assert!(reasons[0].contains("no CI run"), "{reasons:?}");
        let (_, reasons) = decide(&o(TreeState::Clean, &[], vec![], None));
        assert!(reasons[0].contains("was not given"), "{reasons:?}");
        let unclean = vec![("r".to_string(), TreeState::Unknown)];
        let (_, reasons) = decide(&o(TreeState::Clean, &[], unclean, None));
        assert_eq!(
            reasons,
            ["the presented commit's run record r carries a unknown tree"]
        );
    }

    /// A member off the list is dropped wherever it stands: at the root, inside an element of
    /// a list, and inside a map member; a map whose keys are data keeps every key.
    #[test]
    fn the_projection_keeps_only_the_list() {
        let doc = serde_json::json!({
            "schema": 1,
            "presented": { "commit": "c", "tree": "clean", "host": "x" },
            "records": [{ "id": "r", "executions": [1], "changed": ["a"] }],
            "totals": { "proven": 1, "stale": 2 },
            "extra": true,
        });
        let kept = project(&doc);
        assert_eq!(
            kept,
            serde_json::json!({
                "schema": 1,
                "presented": { "commit": "c", "tree": "clean" },
                "records": [{ "id": "r", "changed": ["a"] }],
                "totals": { "proven": 1, "stale": 2 },
            })
        );
    }

    /// Each string is redacted, a refused one is withheld and counted, and a number or a
    /// boolean passes as it is.
    #[test]
    fn every_string_passes_the_redactor() {
        let dir = tempfile::tempdir().unwrap();
        let token = format!("{}{}", "ghp_", "a".repeat(24));
        let elsewhere = format!("{}{}", "/ho", "me/someone/.cache");
        let mut v = serde_json::json!({ "a": [token, elsewhere], "n": 3, "b": true });
        let withheld = redact(&mut v, dir.path(), None);
        assert_eq!(withheld, 1);
        assert_eq!(v["a"][0], "[redacted:github-token]");
        assert_eq!(v["a"][1], WITHHELD);
        assert_eq!((v["n"].clone(), v["b"].clone()), (3.into(), true.into()));
    }
}
