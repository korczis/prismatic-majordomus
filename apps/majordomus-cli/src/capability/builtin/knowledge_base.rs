//! The `knowledge_base` module: what the knowledge deriver left for review and whether it is
//! still writing — the candidate records awaiting promotion, one record by id with every
//! reference it names resolved, and the derivation status of this checkout judged against
//! `session.freshness`.
//!
//! **It is read, never written.** The writer is the shell tool's `lib/knowledge.sh`, which
//! derives a candidate from the ledger and git at every episode boundary (ADR 0058), and a
//! person promotes or rejects one. This process reads three things and restates none of
//! them: the candidates from the index, where they are tracked files discovered by the
//! `candidates` source class like every other object of the layer; the ledger through
//! [`crate::session::Ledger`], which is the one reader of that file; and the thresholds and
//! switches from the policy, through the reading [`super::continuity`] already does.
//!
//! **The module id is not `knowledge`.** The registry refuses a builtin module named like a
//! declarative kind, and `knowledge` is the kind these records are. The command line still
//! reads `majordomus knowledge <verb>`, because the words a person types are a projection
//! and the module id is not.
//!
//! **Absence is an answer.** A fresh clone has no ledger and no candidates, a repository
//! whose policy predates the keys declares no thresholds, and a record can name an episode
//! this checkout never saw. Each is reported as itself, never as a failure, and the
//! stopped-writer judgement is withheld — and says so — until a derivation has run here at
//! least once, so that the day the deriver arrives no checkout turns red for what it could
//! not yet have done.
//!
//! ```
//! use majordomus_cli::capability::builtin::knowledge_base::{module, KNOWLEDGE_STATUS_URI};
//! let m = module();
//! assert_eq!(m.id.as_str(), "knowledge_base");
//! assert!(m.capabilities.iter().any(|e| {
//!     e.capability.exposure.mcp.as_ref().and_then(|x| x.resource.as_ref())
//!         .is_some_and(|r| r.uri == KNOWLEDGE_STATUS_URI)
//! }));
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::git::GitState;
use crate::model::Object;
use crate::session::Ledger;
use crate::{capability, module};

use super::continuity::{thresholds_of, Freshness, Thresholds};
use super::{get, mcp, Empty};

/// The URI under which `knowledge_base.candidates` is read as an MCP resource.
pub const KNOWLEDGE_CANDIDATES_URI: &str = "majordomus://knowledge-candidates";
/// The URI under which `knowledge_base.status` is read as an MCP resource.
pub const KNOWLEDGE_STATUS_URI: &str = "majordomus://knowledge-status";

/// The kind a record is, and the source class the deriver writes into. The class id is the
/// one both `sources.yaml` files declare; a record of the kind under any other class is
/// curated, or is somebody's, and is not awaiting review.
const KIND: &str = "knowledge";
const CANDIDATES_CLASS: &str = "candidates";
/// The local half of the layer, relative to the repository root; the same path the
/// continuity module reads, stated once more only because the task store under it is
/// where a `task:` reference resolves when the ledger no longer names it.
const STATE_DIR: &str = ".ai/local/state";

// --------------------------------------------------------------------- candidates

/// Where the age of a candidate in the review queue was read from. A candidate's `date` is
/// the day of the evidence it came from, deliberately, so that the same evidence yields the
/// same bytes; the day it entered the queue is a different fact and is read from the ledger
/// first, then from git, and only then from the record.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::AgeSource;
/// let from_ledger: AgeSource = serde_json::from_str(r#""derivation""#).unwrap();
/// assert_eq!(from_ledger, AgeSource::Derivation);
/// assert!(AgeSource::Derivation < AgeSource::Date, "the ledger is asked before the record");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AgeSource {
    /// The oldest `knowledge.derived` line in this checkout's ledger whose paths name it.
    Derivation,
    /// The commit that added the file to this history.
    Commit,
    /// The record's own `date`, at midnight UTC: the evidence day, not the queue day.
    Date,
}

impl AgeSource {
    /// The word as serialised, for a text rendering that names its source.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::knowledge_base::AgeSource;
    /// assert_eq!(AgeSource::Derivation.as_str(), "derivation");
    /// assert_eq!(AgeSource::Date.as_str(), "date");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            AgeSource::Derivation => "derivation",
            AgeSource::Commit => "commit",
            AgeSource::Date => "date",
        }
    }
}

/// One candidate record awaiting review: what a reader needs to decide whether to open it.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::KnowledgeCandidate;
/// let c: KnowledgeCandidate = serde_json::from_str(r#"{
///   "id": "e1-0123456789ab", "uri": "majordomus://knowledge/e1-0123456789ab",
///   "path": ".ai/repo/knowledge/candidates/e1-0123456789ab.md", "class": "convention",
///   "status": "candidate", "epistemics": "decided", "title": "Tests run in disposable repositories",
///   "date": "2026-09-12", "derived_from": ["session:e1"], "episode": "e1",
///   "freshness": "unknown", "freshness_reason": "no timestamp", "age_source": "date"
/// }"#).unwrap();
/// assert_eq!(c.episode, "e1");
/// assert_eq!(c.branch, None, "an episode this checkout never saw has no branch, and says so");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeCandidate {
    /// The record id, which is also its file name.
    pub id: String,
    /// `majordomus://knowledge/<id>`.
    pub uri: String,
    /// Repository-relative path.
    pub path: String,
    /// `fact`, `convention`, `constraint`, `memory` or `lesson`.
    pub class: String,
    /// Always `candidate` here; carried so that a reader sees the same record shape as `record`.
    pub status: String,
    /// `observed`, `inferred` or `decided`.
    pub epistemics: String,
    /// The assertion, in one line.
    pub title: String,
    /// The day of the evidence, `YYYY-MM-DD`.
    pub date: String,
    /// The typed references the record names as its evidence, as declared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<String>,
    /// The episode the record came from: the `session:` reference, or empty when it names none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub episode: String,
    /// The branch that episode ran on, joined to the tracked session record or, while that
    /// is not yet written, to the ledger's `session.started` line. `None` when this checkout
    /// knows nothing about the episode — named rather than hidden, so that a reader can tell
    /// "not this branch" from "not known here". Serialised as `null` in that case, never
    /// omitted.
    #[serde(default)]
    pub branch: Option<String>,
    /// How long it has waited, judged against `session.freshness`.
    pub freshness: Freshness,
    /// The wait in whole minutes, when it could be measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_minutes: Option<i64>,
    /// Why it was judged that way, in one phrase. Never empty.
    pub freshness_reason: String,
    /// Where the wait was read from.
    pub age_source: AgeSource,
}

/// The queue orders by id, which the deriver makes total: the episode, then a digest of the
/// evidence.
impl crate::order::Ordered for KnowledgeCandidate {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id)
    }
}

/// The candidates awaiting review, against the policy's cap.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::KnowledgeCandidates;
/// let empty: KnowledgeCandidates = serde_json::from_str(
///     r#"{"total":0,"on_this_branch":0,"unattributed":0,"branch":"master","candidates":[],"over_cap":false}"#,
/// ).unwrap();
/// assert_eq!(empty.total, 0, "an empty queue is an answer");
/// assert_eq!(empty.cap, None, "and a policy without the key declares no cap");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeCandidates {
    /// How many records carry `status: candidate` under the candidates class.
    pub total: usize,
    /// How many of them came from an episode on this checkout's branch.
    pub on_this_branch: usize,
    /// How many name an episode this checkout knows nothing about.
    pub unattributed: usize,
    /// The branch this answer is about, or `DETACHED`.
    pub branch: String,
    /// Every candidate, in canonical order of id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<KnowledgeCandidate>,
    /// `knowledge.candidates_max_files`, when the policy declares it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<usize>,
    /// True when `total` exceeds `cap`. False when no cap is declared: nobody wrote one down.
    pub over_cap: bool,
    /// What a reader should know before trusting the above: a malformed record that was
    /// skipped, a cap that was exceeded, a policy that declares none. Empty is the good case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

// --------------------------------------------------------------------- record

/// Which knowledge record: the input of `knowledge_base.record`. The id is the file name
/// and the identity the index keys the record by, so one string names it on every surface.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::KnowledgeRecordInput;
/// let input: KnowledgeRecordInput = serde_json::from_str(r#"{"id":"e1-0123456789ab"}"#).unwrap();
/// assert_eq!(input.id, "e1-0123456789ab");
/// // a key the input does not declare is refused, never ignored
/// assert!(serde_json::from_str::<KnowledgeRecordInput>(r#"{"id":"x","uri":"y"}"#).is_err());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeRecordInput {
    /// The record id, as the file name and `majordomus://knowledge/<id>` carry it.
    pub id: String,
}

impl BenchmarkCases for KnowledgeRecordInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        ctx.index
            .objects
            .iter()
            .find(|o| o.kind == KIND)
            .map(|o| {
                vec![NamedCase::new(
                    "first-record",
                    KnowledgeRecordInput {
                        id: o.identity.clone(),
                    },
                )]
            })
            .unwrap_or_else(|| {
                vec![NamedCase::new(
                    "absent",
                    KnowledgeRecordInput {
                        id: "absent".into(),
                    },
                )]
            })
    }
}

/// How a typed reference resolved.
///
/// Three words and no fourth: a reference names an object of the index, or something only
/// the ledger and git can vouch for, or nothing. `external` is not a weaker `object` — a
/// commit or a task is exactly as real as a rule — it says which reader answered.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::ReferenceResolution;
/// let r: ReferenceResolution = serde_json::from_str(r#""external""#).unwrap();
/// assert_eq!(r, ReferenceResolution::External);
/// assert!(r.resolves(), "vouched for by the ledger or git is resolved");
/// assert!(!ReferenceResolution::Missing.resolves());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceResolution {
    /// An object of the index: the `target` is its URI, or its path for a file.
    Object,
    /// Vouched for by the ledger, the task store or git: a session, a task, a decision, a commit.
    External,
    /// Nothing answers to it. A dangling reference is the integrity validator's finding.
    Missing,
}

impl ReferenceResolution {
    /// The word as serialised, for a text rendering that prints it beside the reference.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::knowledge_base::ReferenceResolution;
    /// assert_eq!(ReferenceResolution::External.as_str(), "external");
    /// assert!(!ReferenceResolution::Missing.resolves());
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ReferenceResolution::Object => "object",
            ReferenceResolution::External => "external",
            ReferenceResolution::Missing => "missing",
        }
    }

    /// Whether something answered to the reference.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::knowledge_base::ReferenceResolution;
    /// assert!(ReferenceResolution::Object.resolves());
    /// assert!(!ReferenceResolution::Missing.resolves());
    /// ```
    pub fn resolves(self) -> bool {
        !matches!(self, ReferenceResolution::Missing)
    }
}

/// One `derived_from` reference of a record, with how it resolved and against what.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::{ReferenceResolution, ResolvedReference};
/// let r: ResolvedReference = serde_json::from_str(
///     r#"{"reference":"task:none","resolution":"missing","reason":"`none` is not a task"}"#,
/// ).unwrap();
/// assert_eq!(r.resolution, ReferenceResolution::Missing);
/// assert_eq!(r.target, None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedReference {
    /// The reference as written: `<prefix>:<value>`.
    pub reference: String,
    /// How it resolved.
    pub resolution: ReferenceResolution,
    /// What it resolved to: a URI for an object, a path for a file, the reader that vouched
    /// for an external reference. Absent when nothing did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Why it did not resolve, when it did not.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// One `relations` entry, resolved the same way.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::{ReferenceResolution, ResolvedRelation};
/// let r: ResolvedRelation = serde_json::from_str(
///     r#"{"relation_type":"relates_to","reference":"file:docs/CLI.md","resolution":"object","target":"docs/CLI.md"}"#,
/// ).unwrap();
/// assert_eq!(r.relation_type, "relates_to");
/// assert!(r.resolution.resolves());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedRelation {
    /// `relates_to`, `depends_on`, `documents`, `supports`, `contradicts`, `supersedes` or `derived_from`.
    pub relation_type: String,
    /// The target as written.
    pub reference: String,
    /// How it resolved.
    pub resolution: ReferenceResolution,
    /// What it resolved to, when something did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Why it did not resolve, when it did not.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// One knowledge record, with every reference it names resolved.
///
/// `objects.get` already serves the same file as metadata and content. What this adds is
/// the resolution: which of the record's references answer, through which reader, and which
/// dangle — the question a reviewer asks before promoting a candidate, and the question the
/// integrity validator asks of every record.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::KnowledgeRecord;
/// let r: KnowledgeRecord = serde_json::from_str(r#"{
///   "id": "e1-0123456789ab", "uri": "majordomus://knowledge/e1-0123456789ab",
///   "path": ".ai/repo/knowledge/candidates/e1-0123456789ab.md", "source_class": "candidates",
///   "class": "convention", "status": "candidate", "epistemics": "decided", "origin": "extracted",
///   "date": "2026-09-12", "title": "An assertion", "resolved": true, "content": "---\n---\n"
/// }"#).unwrap();
/// assert!(r.derived_from.is_empty(), "a record that names nothing has nothing to resolve");
/// assert_eq!(r.superseded_by, None, "and is not superseded");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeRecord {
    /// The record id.
    pub id: String,
    /// `majordomus://knowledge/<id>`.
    pub uri: String,
    /// Repository-relative path.
    pub path: String,
    /// The source class that discovered it: `candidates` or `curated`.
    pub source_class: String,
    /// `fact`, `convention`, `constraint`, `memory` or `lesson`.
    pub class: String,
    /// `candidate`, `verified` or `superseded`.
    pub status: String,
    /// `observed`, `inferred` or `decided`.
    pub epistemics: String,
    /// `authored` or `extracted`; empty when the record carries no provenance.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub origin: String,
    /// The day the record reached its status.
    pub date: String,
    /// The assertion.
    pub title: String,
    /// Every `provenance.derived_from` reference, resolved, in the record's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<ResolvedReference>,
    /// Every `relations` entry, resolved, in the record's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<ResolvedRelation>,
    /// The record that replaced this one, when the status is `superseded`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    /// True when every reference above resolved. The one bit a reviewer reads first.
    #[serde(default)]
    pub resolved: bool,
    /// The whole file, front matter and body.
    pub content: String,
}

// --------------------------------------------------------------------- status

/// One ledger line that matters to the judgement: when, and which episode.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::LedgerMark;
/// let m: LedgerMark = serde_json::from_str(r#"{"ts":"2026-09-12T10:00:00Z","episode":"e1"}"#).unwrap();
/// assert_eq!(m.episode, "e1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerMark {
    /// The line's timestamp, RFC 3339 in UTC.
    pub ts: String,
    /// The episode the line is about; empty when the line names none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub episode: String,
}

/// Whether the deriver is still writing, and what it last wrote.
///
/// The judgement is ADR 0052's, applied to this writer: while episodes keep closing, a
/// derivation follows every close. It compares episode ids, never line order or timestamps
/// between the two events, because the close derives before it appends `session.closed`.
/// It is the freshness half of the judgement only; whether the close path and the
/// compaction adapter still call the deriver is read from the source by the shell
/// validator, which is where the source is.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::KnowledgeStatus;
/// let fresh: KnowledgeStatus = serde_json::from_str(r#"{
///   "present": false, "branch": "master", "closed_without_derivation": 0, "candidates": 0,
///   "on_this_branch": 0, "freshness": "unknown", "freshness_reason": "no episode has closed here",
///   "judged": false, "stopped_writer": false, "knowledge_on_end": true, "knowledge_on_compact": true
/// }"#).unwrap();
/// assert!(!fresh.present, "a fresh clone has no ledger, and that is not a fault");
/// assert!(!fresh.stopped_writer);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeStatus {
    /// Whether anything is here to judge: a ledger line or a candidate. False in a fresh clone.
    pub present: bool,
    /// The branch this checkout is on, or `DETACHED`.
    pub branch: String,
    /// The newest `knowledge.derived` line, or `None` when the deriver has never run here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_derived: Option<LedgerMark>,
    /// The newest `session.closed` line, or `None` when no episode has closed here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_closed: Option<LedgerMark>,
    /// How many closed episodes in this ledger no `knowledge.derived` line names.
    pub closed_without_derivation: usize,
    /// How many records carry `status: candidate` under the candidates class.
    pub candidates: usize,
    /// How many of those came from an episode on this branch.
    pub on_this_branch: usize,
    /// How old the newest closed episode is, judged against `session.freshness`.
    pub freshness: Freshness,
    /// That age in whole minutes, when it could be measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_minutes: Option<i64>,
    /// Why it was judged that way. Never empty.
    pub freshness_reason: String,
    /// Whether the stopped-writer judgement was made at all. False, with the reason among
    /// `findings`, while no episode has closed here, while the switch is off, or while no
    /// derivation has yet run in this checkout.
    pub judged: bool,
    /// The finding: episodes close here and no derivation followed the newest one within
    /// the stale threshold. True only when `judged`.
    pub stopped_writer: bool,
    /// `session.knowledge_on_end`, as the policy declares it; absent means on.
    pub knowledge_on_end: bool,
    /// `session.knowledge_on_compact`, likewise.
    pub knowledge_on_compact: bool,
    /// What a reader should know: why the judgement was withheld, or what it found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

// --------------------------------------------------------------------- reading

/// A scalar of an object's metadata, or empty.
fn field(o: &Object, key: &str) -> String {
    o.metadata
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The `provenance.derived_from` list of a record, as written.
fn derived_from_of(o: &Object) -> Vec<String> {
    o.metadata
        .get("provenance")
        .and_then(|p| p.get("derived_from"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The `relations` list of a record, as `(type, target)` pairs.
fn relations_of(o: &Object) -> Vec<(String, String)> {
    o.metadata
        .get("relations")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|r| {
                    let s = |k: &str| {
                        r.get(k)
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string()
                    };
                    (s("type"), s("target"))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The episode a record names: its first `session:` reference.
fn episode_of(refs: &[String]) -> String {
    refs.iter()
        .find_map(|r| r.strip_prefix("session:"))
        .unwrap_or_default()
        .to_string()
}

/// The branch this checkout is on, as git reports it, or `DETACHED`.
fn branch_of(ctx: &Context) -> String {
    match &ctx.index.repository.git {
        GitState::Available(info) => info.branch.clone().unwrap_or_else(|| "DETACHED".into()),
        GitState::Unavailable { .. } => "DETACHED".into(),
    }
}

/// One reading of the clock per call, in Unix seconds.
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The policy, typed, or `None` when the repository or its policy cannot be read. Absence is
/// carried as absence: the switches then take their declared defaults and the cap is none.
fn policy_of(ctx: &Context) -> Option<crate::policy::Policy> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let repo = crate::repository::Repository::open(&root).ok()?;
    Some(crate::policy::LoadedPolicy::load(&repo).ok()?.policy)
}

/// Every episode's branch, as far as this checkout knows it: the tracked session record
/// first, because it is what every clone shares, then the ledger's `session.started` line,
/// because the record is written only when the episode closes. One map for the whole call,
/// so that the ledger is walked once however many candidates name episodes.
fn episode_branches(ctx: &Context, events: &[Value]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for e in events {
        if e["event"].as_str() == Some("session.started") {
            if let (Some(sid), Some(branch)) = (e["session"].as_str(), e["branch"].as_str()) {
                out.entry(sid.to_string())
                    .or_insert_with(|| branch.to_string());
            }
        }
    }
    for o in ctx.index.objects.iter().filter(|o| o.kind == "session") {
        let sid = field(o, "session_id");
        let branch = field(o, "branch");
        if !sid.is_empty() && !branch.is_empty() {
            out.insert(sid, branch);
        }
    }
    out
}

/// When each candidate entered the review queue, by path: the oldest `knowledge.derived`
/// line whose `paths` names it. The ledger is the first source because it is the one that
/// knows the derivation happened; a candidate somebody hand-copied into the directory has
/// no such line and falls through to git.
fn derivation_times(events: &[Value]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for e in events {
        if e["event"].as_str() != Some("knowledge.derived") {
            continue;
        }
        let Some(ts) = e["ts"].as_str() else {
            continue;
        };
        for path in e["paths"].as_str().unwrap_or_default().split_whitespace() {
            out.entry(path.to_string())
                .or_insert_with(|| ts.to_string());
        }
    }
    out
}

/// When git first saw each of `paths`: the commit that added it, as an RFC 3339 instant.
/// One `git log` for every path at once, never one per candidate, and nothing when git
/// cannot answer — a candidate the hook just wrote is not yet in any commit, and that is an
/// answer rather than an error.
fn added_times(root: &Path, paths: &[&str]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if paths.is_empty() {
        return out;
    }
    let Ok(run) = crate::git::read_only(root)
        .args([
            "log",
            "--diff-filter=A",
            "--format=commit:%ct",
            "--name-only",
            "--",
        ])
        .args(paths)
        .output()
    else {
        return out;
    };
    if !run.status.success() {
        return out;
    }
    let mut current: Option<u64> = None;
    for line in String::from_utf8_lossy(&run.stdout).lines() {
        if let Some(secs) = line.strip_prefix("commit:") {
            current = secs.trim().parse().ok();
            continue;
        }
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        if let Some(secs) = current {
            let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
            // the log walks newest first, so the last write for a path is its oldest
            // addition — which is when it entered the queue
            out.insert(path.to_string(), crate::peers::rfc3339(at));
        }
    }
    out
}

/// The candidate objects of the index: the kind, under the class the deriver writes into,
/// carrying the status. A record under the class that claims another status is the
/// integrity validator's finding and is not awaiting review.
fn candidate_objects(ctx: &Context) -> Vec<&Object> {
    ctx.index
        .objects
        .iter()
        .filter(|o| {
            o.kind == KIND
                && o.provenance.source_class == CANDIDATES_CLASS
                && field(o, "status") == "candidate"
        })
        .collect()
}

/// The candidates of this checkout, judged. Shared by `candidates` and `status`, which
/// count the same queue; one derivation so the two cannot disagree about its size.
fn candidates_of(ctx: &Context, events: &[Value], now: i64) -> KnowledgeCandidates {
    let root = PathBuf::from(&ctx.index.repository.root);
    let branch = branch_of(ctx);
    let thresholds = thresholds_of(ctx);
    let policy = policy_of(ctx);
    let cap = policy
        .as_ref()
        .and_then(|p| p.knowledge.candidates_max_files);

    let objects = candidate_objects(ctx);
    let branches = episode_branches(ctx, events);
    let derived = derivation_times(events);
    let paths: Vec<&str> = objects
        .iter()
        .filter(|o| !derived.contains_key(&o.provenance.path))
        .map(|o| o.provenance.path.as_str())
        .collect();
    let added = added_times(&root, &paths);

    let mut findings = Vec::new();
    let mut candidates: Vec<KnowledgeCandidate> = objects
        .iter()
        .map(|o| {
            let refs = derived_from_of(o);
            let episode = episode_of(&refs);
            let path = o.provenance.path.clone();
            let date = field(o, "date");
            let (entered, age_source) = match derived.get(&path) {
                Some(ts) => (ts.clone(), AgeSource::Derivation),
                None => match added.get(&path) {
                    Some(ts) => (ts.clone(), AgeSource::Commit),
                    None if date.is_empty() => (String::new(), AgeSource::Date),
                    None => (format!("{date}T00:00:00Z"), AgeSource::Date),
                },
            };
            let (freshness, age_minutes, freshness_reason) = thresholds.judge(&entered, now);
            KnowledgeCandidate {
                id: o.identity.clone(),
                uri: o.uri.clone(),
                path,
                class: field(o, "class"),
                status: field(o, "status"),
                epistemics: field(o, "epistemics"),
                title: o.title.clone().unwrap_or_default(),
                date,
                branch: if episode.is_empty() {
                    None
                } else {
                    branches.get(&episode).cloned()
                },
                episode,
                derived_from: refs,
                freshness,
                age_minutes,
                freshness_reason,
                age_source,
            }
        })
        .collect();
    crate::order::canonical(&mut candidates);

    let total = candidates.len();
    let on_this_branch = candidates
        .iter()
        .filter(|c| c.branch.as_deref() == Some(branch.as_str()))
        .count();
    let unattributed = candidates.iter().filter(|c| c.branch.is_none()).count();
    if unattributed > 0 {
        findings.push(format!(
            "{unattributed} candidate(s) name an episode this checkout does not know; their branch is reported as absent, not guessed"
        ));
    }
    let over_cap = cap.is_some_and(|cap| total > cap);
    if over_cap {
        findings.push(format!(
            "{total} candidates await review, over the {} the policy calls the cap (knowledge.candidates_max_files); the queue is measured, never emptied by a machine",
            cap.unwrap_or_default()
        ));
    }
    if let Some(age) = policy
        .as_ref()
        .and_then(|p| p.knowledge.candidate_max_age_minutes)
    {
        let old = candidates
            .iter()
            .filter(|c| c.age_minutes.is_some_and(|m| m >= age))
            .count();
        if old > 0 {
            findings.push(format!(
                "{old} candidate(s) have waited longer than the {} the policy calls the age (knowledge.candidate_max_age_minutes); review them or reject them",
                super::continuity::span(age)
            ));
        }
    }

    KnowledgeCandidates {
        total,
        on_this_branch,
        unattributed,
        branch,
        candidates,
        cap,
        over_cap,
        findings,
    }
}

fn candidates(ctx: &Context, _: Empty) -> Result<KnowledgeCandidates, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let read = Ledger::of(&root).read();
    Ok(candidates_of(ctx, &read.events, now()))
}

// --------------------------------------------------------------------- resolution

/// Resolve one typed reference against the index, the ledger, the task store and git.
///
/// The prefixes are the schema's; a prefix the schema does not admit resolves to nothing,
/// with the reason, rather than to a guess. `task:none` and `decision:none` are refused by
/// name: `none` is the literal the ledger stamps on a line recorded outside a task, and a
/// record that names it as its evidence names nothing.
fn resolve(
    ctx: &Context,
    root: &Path,
    events: &[Value],
    reference: &str,
) -> (ReferenceResolution, Option<String>, String) {
    use ReferenceResolution::{External, Missing, Object};
    let Some((prefix, value)) = reference.split_once(':') else {
        return (
            Missing,
            None,
            "a reference is `<prefix>:<value>`; this one has no prefix".into(),
        );
    };
    if value.is_empty() {
        return (Missing, None, format!("`{prefix}:` names nothing"));
    }
    let by_kind = |kind: &str| -> Option<String> {
        ctx.index
            .objects
            .iter()
            .find(|o| {
                o.kind == kind
                    && (o.identity == value || o.identity.starts_with(&format!("{value}@")))
            })
            .map(|o| o.uri.clone())
    };
    match prefix {
        "session" => {
            let object = ctx
                .index
                .objects
                .iter()
                .find(|o| o.kind == "session" && field(o, "session_id") == value)
                .map(|o| o.uri.clone());
            if let Some(uri) = object {
                return (Object, Some(uri), String::new());
            }
            if events.iter().any(|e| e["session"].as_str() == Some(value)) {
                return (External, Some("ledger".into()), String::new());
            }
            (
                Missing,
                None,
                format!("no session record and no ledger line names the episode {value}"),
            )
        }
        "task" | "decision" => {
            if value == "none" {
                return (
                    Missing,
                    None,
                    format!("`{prefix}:none` is not a reference: `none` is the ledger's word for a line recorded outside a task"),
                );
            }
            if events.iter().any(|e| e["task_id"].as_str() == Some(value)) {
                return (External, Some("ledger".into()), String::new());
            }
            let state = root.join(STATE_DIR);
            let current = super::continuity::read_task(&state.join("current.yaml"))
                .is_some_and(|t| t.id == value);
            if current {
                return (External, Some("task store".into()), String::new());
            }
            if state
                .join("completed")
                .join(format!("{value}.md"))
                .is_file()
            {
                return (External, Some("task store".into()), String::new());
            }
            (
                Missing,
                None,
                format!("no ledger line and no task record names the task {value}"),
            )
        }
        "commit" => {
            let ok = crate::git::read_only(root)
                .args([
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{value}^{{commit}}"),
                ])
                .output()
                .is_ok_and(|o| o.status.success());
            if ok {
                (External, Some("git".into()), String::new())
            } else {
                (Missing, None, format!("this history has no commit {value}"))
            }
        }
        "file" | "test" => {
            if let Some(o) = ctx
                .index
                .objects
                .iter()
                .find(|o| o.provenance.path == value)
            {
                return (Object, Some(o.uri.clone()), String::new());
            }
            if crate::policy::is_safe_relative(value) && root.join(value).exists() {
                return (External, Some(value.to_string()), String::new());
            }
            (Missing, None, format!("no such path in the tree: {value}"))
        }
        "knowledge" | "rule" | "adr" | "issue" => match by_kind(prefix) {
            Some(uri) => (Object, Some(uri), String::new()),
            None => (
                Missing,
                None,
                format!("the index holds no {prefix} with the identity {value}"),
            ),
        },
        other => (
            Missing,
            None,
            format!("`{other}:` is not a reference prefix the knowledge schema admits"),
        ),
    }
}

fn record(ctx: &Context, input: KnowledgeRecordInput) -> Result<KnowledgeRecord, CapabilityError> {
    let uri = crate::model::uri_for(KIND, &input.id);
    let o = ctx.index.get(&uri).ok_or_else(|| {
        CapabilityError::NotFound(format!(
            "no knowledge record '{}'; `knowledge candidates` lists the ones awaiting review",
            input.id
        ))
    })?;
    let root = PathBuf::from(&ctx.index.repository.root);
    let events = Ledger::of(&root).read().events;

    let derived_from: Vec<ResolvedReference> = derived_from_of(o)
        .into_iter()
        .map(|reference| {
            let (resolution, target, reason) = resolve(ctx, &root, &events, &reference);
            ResolvedReference {
                reference,
                resolution,
                target,
                reason,
            }
        })
        .collect();
    let relations: Vec<ResolvedRelation> = relations_of(o)
        .into_iter()
        .map(|(relation_type, reference)| {
            let (resolution, target, reason) = resolve(ctx, &root, &events, &reference);
            ResolvedRelation {
                relation_type,
                reference,
                resolution,
                target,
                reason,
            }
        })
        .collect();
    let resolved = derived_from.iter().all(|r| r.resolution.resolves())
        && relations.iter().all(|r| r.resolution.resolves());
    let superseded_by = Some(field(o, "superseded_by")).filter(|s| !s.is_empty());

    Ok(KnowledgeRecord {
        id: o.identity.clone(),
        uri: o.uri.clone(),
        path: o.provenance.path.clone(),
        source_class: o.provenance.source_class.clone(),
        class: field(o, "class"),
        status: field(o, "status"),
        epistemics: field(o, "epistemics"),
        origin: o
            .metadata
            .get("provenance")
            .and_then(|p| p.get("origin"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        date: field(o, "date"),
        title: o.title.clone().unwrap_or_default(),
        derived_from,
        relations,
        superseded_by,
        resolved,
        content: o.content.clone(),
    })
}

// --------------------------------------------------------------------- status

/// The `ts` and the episode of a ledger line, the episode read from the field the event
/// carries it in: the envelope's `session` stamp for a close, the explicit `episode` for a
/// derivation, which is explicit precisely because the stamp is absent once the episode is
/// closed.
fn mark(e: &Value, episode_field: &str) -> LedgerMark {
    LedgerMark {
        ts: e["ts"].as_str().unwrap_or_default().to_string(),
        episode: e[episode_field].as_str().unwrap_or_default().to_string(),
    }
}

/// The stopped-writer judgement over a ledger already read: what the handler assembles into
/// [`KnowledgeStatus`], separated from it so that the in-file tests drive the arithmetic
/// without a repository.
struct Judgement {
    last_derived: Option<LedgerMark>,
    last_closed: Option<LedgerMark>,
    closed_without_derivation: usize,
    judged: bool,
    stopped: bool,
    verdict: (Freshness, Option<i64>, String),
    findings: Vec<String>,
}

/// The judgement, over a ledger already read and thresholds already taken.
fn judge(events: &[Value], knowledge_on_end: bool, thresholds: Thresholds, now: i64) -> Judgement {
    let closes: Vec<&Value> = events
        .iter()
        .filter(|e| e["event"].as_str() == Some("session.closed"))
        .collect();
    let derivations: Vec<&Value> = events
        .iter()
        .filter(|e| e["event"].as_str() == Some("knowledge.derived"))
        .collect();
    let derived_episodes: std::collections::BTreeSet<&str> = derivations
        .iter()
        .filter_map(|e| e["episode"].as_str())
        .collect();
    let last_closed = closes.last().map(|e| mark(e, "session"));
    let last_derived = derivations.last().map(|e| mark(e, "episode"));
    let closed_without_derivation = closes
        .iter()
        .filter(|e| {
            e["session"]
                .as_str()
                .is_some_and(|s| !derived_episodes.contains(s))
        })
        .count();

    let mut findings = Vec::new();
    let withheld = |findings: Vec<String>, why: &str| Judgement {
        last_derived: last_derived.clone(),
        last_closed: last_closed.clone(),
        closed_without_derivation,
        judged: false,
        stopped: false,
        verdict: (Freshness::Unknown, None, why.to_string()),
        findings,
    };

    let Some(closed) = last_closed.clone() else {
        findings.push("no episode has closed here; nothing to judge".to_string());
        return withheld(findings, "no episode has closed here");
    };
    if !knowledge_on_end {
        findings.push(
            "session.knowledge_on_end is false; the deriver is switched off here and its silence is not judged".to_string(),
        );
        return withheld(findings, "the deriver is switched off");
    }
    if derivations.is_empty() {
        findings.push(
            "no knowledge derivation has run in this checkout yet, so the newest close is not judged; the first `majordomus knowledge derive` starts the judgement".to_string(),
        );
        return withheld(findings, "no derivation has run here yet");
    }
    if closed.episode.is_empty() {
        findings.push(
            "the newest session.closed line names no episode; nothing to compare a derivation against".to_string(),
        );
        return withheld(findings, "the newest close names no episode");
    }
    let verdict = thresholds.judge(&closed.ts, now);
    let covered = derived_episodes.contains(closed.episode.as_str());
    let stopped = !covered && verdict.0 == Freshness::Stale;
    if !covered {
        match verdict.0 {
            Freshness::Stale => findings.push(format!(
                "episodes close and no knowledge.derived followed the newest ({}, {}); the writer has stopped. fix: majordomus knowledge derive --episode {}",
                closed.episode, verdict.2, closed.episode
            )),
            Freshness::Unknown | Freshness::Invalid => findings.push(format!(
                "the newest closed episode ({}) has no derivation and cannot be judged: {}",
                closed.episode, verdict.2
            )),
            Freshness::Fresh | Freshness::Aging => findings.push(format!(
                "the newest closed episode ({}) awaits its derivation: {}",
                closed.episode, verdict.2
            )),
        }
    }
    Judgement {
        last_derived,
        last_closed,
        closed_without_derivation,
        judged: true,
        stopped,
        verdict,
        findings,
    }
}

fn status(ctx: &Context, _: Empty) -> Result<KnowledgeStatus, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let read = Ledger::of(&root).read();
    let now = now();
    let policy = policy_of(ctx);
    let (knowledge_on_end, knowledge_on_compact) = policy
        .as_ref()
        .map(|p| (p.session.knowledge_on_end, p.session.knowledge_on_compact))
        .unwrap_or((true, true));
    let thresholds = thresholds_of(ctx);
    let queue = candidates_of(ctx, &read.events, now);

    let Judgement {
        last_derived,
        last_closed,
        closed_without_derivation,
        judged,
        stopped,
        verdict: (freshness, age_minutes, freshness_reason),
        mut findings,
    } = judge(&read.events, knowledge_on_end, thresholds, now);
    if !read.corrupt.is_empty() {
        findings.push(format!(
            "{} ledger line(s) did not parse and were not judged; run `majordomus doctor`",
            read.corrupt.len()
        ));
    }
    findings.extend(queue.findings.iter().cloned());

    Ok(KnowledgeStatus {
        present: !read.events.is_empty() || queue.total > 0,
        branch: queue.branch.clone(),
        last_derived,
        last_closed,
        closed_without_derivation,
        candidates: queue.total,
        on_this_branch: queue.on_this_branch,
        freshness,
        age_minutes,
        freshness_reason,
        judged,
        stopped_writer: stopped,
        knowledge_on_end,
        knowledge_on_compact,
        findings,
    })
}

// --------------------------------------------------------------------- module

/// The module: three queries, declared once, from which every projection — the MCP tools
/// and resources, the HTTP routes, the command line, the OpenAPI operations, the cockpit's
/// catalogue entry and the benchmark targets — is derived.
///
/// ```
/// use majordomus_cli::capability::builtin::knowledge_base::module;
/// let m = module();
/// let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
/// assert_eq!(ids, ["knowledge_base.candidates", "knowledge_base.record", "knowledge_base.status"]);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "knowledge_base",
        title: "Knowledge base",
        description: "What the knowledge deriver left for review and whether it is still writing: the candidate records awaiting promotion with the branch of the episode each came from, one record by id with every reference it names resolved against the index, the ledger and git, and the derivation status of this checkout judged against the policy's freshness thresholds. Read from the index and the ledger; written by nothing here — the deriver is the shell tool's, and a person promotes.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "knowledge_base.candidates",
                title: "Candidates awaiting review",
                description: "Every knowledge record with status candidate under the candidates source class: its class, its assertion, the episode it came from and that episode's branch, and how long it has waited — measured from the derivation that wrote it, then the commit that added it, then its own date — judged against session.freshness; against the policy's cap. A candidate whose episode this checkout does not know is reported with no branch rather than hidden. Ordered by id.",
                input: Empty,
                output: KnowledgeCandidates,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_knowledge_candidates".into()),
                        resource: Some(McpResource { uri: KNOWLEDGE_CANDIDATES_URI.into(), name: "knowledge-candidates".into() }),
                    }),
                    http: get("/api/v1/knowledge/candidates"),
                    cli: Some(CliExposure { path: vec!["knowledge".into(), "candidates".into()] }),
                },
                tags: ["knowledge", "continuity"],
                handler: candidates,
            },
            capability! {
                id: "knowledge_base.record",
                title: "One knowledge record",
                description: "One knowledge record by id — candidate or curated — with every reference it names resolved: a session against the tracked session records and the ledger, a task or decision against the ledger and the task store, a commit against git, a file or test against the index and the tree, a rule, an ADR, an issue or another record against the index. Which references dangle is the question a reviewer asks before promoting, and the one the integrity validator asks of every record. A `task:none` or `decision:none` reference is refused by name.",
                input: KnowledgeRecordInput,
                output: KnowledgeRecord,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_record"),
                    http: get("/api/v1/knowledge/record"),
                    cli: Some(CliExposure { path: vec!["knowledge".into(), "record".into()] }),
                },
                tags: ["knowledge"],
                handler: record,
            },
            capability! {
                id: "knowledge_base.status",
                title: "Whether the deriver is still writing",
                description: "The freshness half of the stopped-writer judgement (ADR 0052, applied to the knowledge deriver by ADR 0058): the newest knowledge.derived line, the newest session.closed line, how many closed episodes no derivation names, the candidate counts, and — once a derivation has run in this checkout and the switch is on — whether the newest closed episode went underived past session.freshness.stale_minutes. Episode ids are compared, never line order. Whether the close path still calls the deriver is read from the source by the shell validator, not here.",
                input: Empty,
                output: KnowledgeStatus,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_knowledge_status".into()),
                        resource: Some(McpResource { uri: KNOWLEDGE_STATUS_URI.into(), name: "knowledge-status".into() }),
                    }),
                    http: get("/api/v1/knowledge/status"),
                    cli: Some(CliExposure { path: vec!["knowledge".into(), "status".into()] }),
                },
                tags: ["knowledge", "continuity", "session"],
                // Short-lived: the ledger is written by another process, and a reader that
                // cached it for a minute would report a writer that had just written as stopped.
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(2) },
                handler: status,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The declaration is the only place these names exist, and every projection — the MCP
    /// tool, the HTTP route, the command line, the OpenAPI operation, the benchmark target —
    /// is derived from it. A refactor that dropped an exposure or renamed a route would
    /// still compile; this is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "knowledge_base");
        let expected: &[(&str, &str, &str, &str)] = &[
            (
                "knowledge_base.candidates",
                "majordomus_knowledge_candidates",
                "/api/v1/knowledge/candidates",
                "knowledge candidates",
            ),
            (
                "knowledge_base.record",
                "majordomus_knowledge_record",
                "/api/v1/knowledge/record",
                "knowledge record",
            ),
            (
                "knowledge_base.status",
                "majordomus_knowledge_status",
                "/api/v1/knowledge/status",
                "knowledge status",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _, _)| *id).collect();
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
                exposure.cli.as_ref().map(|c| c.path.join(" ")),
                Some((*cli).to_string()),
                "{id} lost or renamed its command line"
            );
        }
    }

    fn line(event: &str, ts: &str, extra: Value) -> Value {
        let mut v = json!({ "ts": ts, "event": event, "head": "h", "branch": "master", "by": "t" });
        if let Some(map) = extra.as_object() {
            for (k, val) in map {
                v[k] = val.clone();
            }
        }
        v
    }

    const T: Thresholds = Thresholds {
        fresh_minutes: Some(720),
        stale_minutes: Some(2880),
    };
    // 2026-08-29T10:40:00Z, the instant the continuity examples use
    const NOW: i64 = 1_788_000_000;

    /// A ledger nobody has written, and a ledger in which no episode closed, are not judged
    /// and say why; nothing about them is a stopped writer.
    #[test]
    fn nothing_closed_is_not_judged() {
        let j = judge(&[], true, T, NOW);
        assert!(j.last_derived.is_none() && j.last_closed.is_none());
        assert_eq!(j.closed_without_derivation, 0);
        assert!(!j.judged && !j.stopped);
        assert_eq!(j.verdict.0, Freshness::Unknown);
        assert!(j.findings[0].contains("no episode has closed"));
    }

    /// The upgrade gate: a close with no derivation anywhere in the ledger is the day the
    /// deriver arrived, not a stopped writer. The finding says which command starts the
    /// judgement.
    #[test]
    fn a_close_before_any_derivation_is_withheld_not_red() {
        let events = [line(
            "session.closed",
            "2026-01-01T00:00:00Z",
            json!({ "session": "e1", "outcome": "closed" }),
        )];
        let j = judge(&events, true, T, NOW);
        assert_eq!(j.last_closed.unwrap().episode, "e1");
        assert_eq!(
            j.closed_without_derivation, 1,
            "counted, even while not judged"
        );
        assert!(!j.judged && !j.stopped);
        assert!(j.findings[0].contains("majordomus knowledge derive"));
    }

    /// The invariant: once a derivation has run here, the newest close must be named by one
    /// or be younger than the stale threshold. Episode ids are compared, never line order —
    /// the derivation of e2 precedes its close in the ledger and still covers it.
    #[test]
    fn a_covered_close_is_fine_and_an_uncovered_stale_one_is_a_stopped_writer() {
        let covered = [
            line(
                "knowledge.derived",
                "2026-01-01T00:00:00Z",
                json!({ "episode": "e2", "written": 1, "unchanged": 0, "skipped": 0, "paths": "" }),
            ),
            line(
                "session.closed",
                "2026-01-01T00:00:01Z",
                json!({ "session": "e2", "outcome": "closed" }),
            ),
        ];
        let j = judge(&covered, true, T, NOW);
        assert_eq!(j.last_derived.unwrap().episode, "e2");
        assert_eq!(j.closed_without_derivation, 0);
        assert!(j.judged && !j.stopped, "{:?}", j.findings);
        assert!(j.findings.is_empty());

        let uncovered = [
            line(
                "knowledge.derived",
                "2026-01-01T00:00:00Z",
                json!({ "episode": "e1", "written": 0, "unchanged": 0, "skipped": 0, "paths": "" }),
            ),
            line(
                "session.closed",
                "2026-01-02T00:00:00Z",
                json!({ "session": "e2", "outcome": "closed" }),
            ),
        ];
        let j = judge(&uncovered, true, T, NOW);
        assert_eq!(j.last_closed.unwrap().episode, "e2");
        assert_eq!(j.closed_without_derivation, 1);
        assert!(j.judged && j.stopped);
        assert_eq!(j.verdict.0, Freshness::Stale);
        assert!(j.findings[0].contains("the writer has stopped"));
        assert!(j.findings[0].contains("--episode e2"));

        // the switch off withholds the judgement and says so
        let j = judge(&uncovered, false, T, NOW);
        assert!(!j.judged && !j.stopped);
        assert!(j.findings[0].contains("knowledge_on_end is false"));

        // a close younger than the threshold is awaited, not reported
        let young = [
            line(
                "knowledge.derived",
                "2026-01-01T00:00:00Z",
                json!({ "episode": "e1", "written": 0, "unchanged": 0, "skipped": 0, "paths": "" }),
            ),
            line(
                "session.closed",
                "2026-08-29T10:00:00Z",
                json!({ "session": "e2", "outcome": "closed" }),
            ),
        ];
        let j = judge(&young, true, T, NOW);
        assert!(j.judged && !j.stopped);
        assert_eq!(j.verdict.0, Freshness::Fresh);
    }

    /// The queue entry time is the oldest derivation naming the path, so a re-run that
    /// rewrites the same file does not restart the clock.
    #[test]
    fn the_oldest_derivation_naming_a_path_is_when_it_entered_the_queue() {
        let events = [
            line(
                "knowledge.derived",
                "2026-01-01T00:00:00Z",
                json!({ "episode": "e1", "paths": ".ai/repo/knowledge/candidates/a.md .ai/repo/knowledge/candidates/b.md" }),
            ),
            line(
                "knowledge.derived",
                "2026-02-01T00:00:00Z",
                json!({ "episode": "e1", "paths": ".ai/repo/knowledge/candidates/a.md" }),
            ),
        ];
        let times = derivation_times(&events);
        assert_eq!(
            times
                .get(".ai/repo/knowledge/candidates/a.md")
                .map(String::as_str),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(
            times
                .get(".ai/repo/knowledge/candidates/b.md")
                .map(String::as_str),
            Some("2026-01-01T00:00:00Z")
        );
        assert!(!times.contains_key("elsewhere.md"));
    }

    /// The tracked session record wins over the ledger, and an episode neither knows has no
    /// branch.
    #[test]
    fn an_episode_is_the_session_reference_and_nothing_else() {
        assert_eq!(
            episode_of(&["decision:t-1".into(), "session:e1".into()]),
            "e1"
        );
        assert_eq!(episode_of(&["task:t-1".into()]), "");
    }
}
