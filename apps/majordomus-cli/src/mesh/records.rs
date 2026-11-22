//! Typed coordination records in the mesh journal (I4665).
//!
//! The owner decided on 2026-11-20 that the typed records agents coordinate through live in
//! the mesh journal, the one coordination mechanism, and not in a second blackboard beside
//! it. A record is one event kind, `record_written`, carried, signed and replicated like a
//! claim or a handover; everything else here is the fold that reads it and the admission
//! that refuses a write before it is signed.
//!
//! What a record is:
//!
//! - **Typed**: a [`RecordKind`] from a closed set — observation, hypothesis, plan, decision,
//!   blocker, handoff, outcome — about one task (a task, intent or issue identifier) and a
//!   scope of repository paths.
//! - **Authored by the journal**: its author is the `<stream>/<session>` key of the event
//!   that created it. The stream is the signature's, so no caller can name another runtime;
//!   within a runtime, a session over MCP writes for itself alone.
//! - **Versioned**: each write carries a revision. Only the author's session revises its
//!   record, and only at the revision it read (compare-and-swap); another session supersedes
//!   it with a new record that links back, and never overwrites it. A runtime that restarts
//!   is a new stream: its earlier records are superseded, not revised.
//! - **Evidenced or asserted**: the evidence state is derived, never written — a record is
//!   an assertion unless one of its sources is retrievable (a file at a commit, or a tool's
//!   observed result).
//! - **Leased**: a record with an expiry is stale after it; a record whose author's session
//!   closed or whose runtime stopped beating is stale too. Stale records are flagged, never
//!   deleted: the journal is append-only, and every revision stays in the record's history.
//!
//! An older runtime relays `record_written` opaquely, as it relays every kind it does not
//! know, so replication stays compatible; it simply does not fold the records.
//!
//! The lifecycle, on one journal: a record written, revised by its author, superseded by
//! another session, and folded with its history kept.
//!
//! ```
//! use std::sync::Arc;
//! use majordomus_cli::mesh::identity::NodeIdentity;
//! use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
//! use majordomus_cli::mesh::records::{RecordBody, RecordKind};
//! use majordomus_cli::mesh::state::fold;
//!
//! let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
//!     "repo".into(), None).unwrap();
//! let plan = RecordBody::new("r-1", "s1", RecordKind::Plan, "I1", &["docs"], "write it");
//! j.append_own(EventBody::RecordWritten { record: plan.clone() }).unwrap();
//! let revised = RecordBody { revision: 2, title: "write it twice".into(), ..plan };
//! j.append_own(EventBody::RecordWritten { record: revised }).unwrap();
//! let key = format!("{}/r-1", j.own_stream());
//! let mut other = RecordBody::new("r-2", "s2", RecordKind::Plan, "I1", &["docs"], "no");
//! other.supersedes = Some(key.clone());
//! j.append_own(EventBody::RecordWritten { record: other }).unwrap();
//!
//! let state = fold(&j.events(), &|_| StreamLiveness::Own);
//! let first = state.records.iter().find(|r| r.key == key).unwrap();
//! assert_eq!((first.revision, first.history.len()), (2, 2), "every revision kept");
//! assert_eq!(first.superseded_by.len(), 1, "superseded, never overwritten");
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::journal::{canonical_json, ident, line, paths, text, Durability, MeshEvent, StreamId};
use super::state::{ClaimView, SessionState, SessionView};

/// The most a record's free-text body carries.
pub const RECORD_BODY_MAX: usize = 8 * 1024;

/// The most a record's title carries: one line.
pub const RECORD_TITLE_MAX: usize = 256;

/// The most a record's task identifier carries: one line.
pub const RECORD_TASK_MAX: usize = 128;

/// The most sources one record cites.
pub const MAX_RECORD_SOURCES: usize = 16;

/// The most evidence lines or blockers one handoff lists, each.
pub const MAX_HANDOFF_ITEMS: usize = 16;

/// The most one handoff paragraph (objective, current state, next action) carries.
pub const HANDOFF_TEXT_MAX: usize = 2048;

/// The most one handoff list item carries.
pub const HANDOFF_ITEM_MAX: usize = 512;

/// The most a whole record carries on the wire, well under the journal's event bound.
pub const RECORD_MAX_BYTES: usize = 32 * 1024;

/// What a record is about. The set is closed: a peer that writes another word is refused
/// at ingest, never guessed at.
///
/// ```
/// use majordomus_cli::mesh::records::RecordKind;
/// assert_eq!(serde_json::to_value(RecordKind::Decision).unwrap(), "decision");
/// assert!(serde_json::from_str::<RecordKind>("\"rumour\"").is_err());
/// assert!(RecordKind::Plan.contends() && !RecordKind::Observation.contends());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MeshRecordKind")]
pub enum RecordKind {
    /// Something seen.
    Observation,
    /// Something believed and not yet shown.
    Hypothesis,
    /// What a session intends to do.
    Plan,
    /// What a session decided.
    Decision,
    /// What stops the work.
    Blocker,
    /// The work handed on: objective, current state, evidence, blockers, next action.
    Handoff,
    /// What the work ended with.
    Outcome,
}

impl RecordKind {
    /// The kind's wire word, written out rather than derived from the variant name: it is
    /// protocol, and renaming a variant must not change what another runtime reads.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::RecordKind;
    /// assert_eq!(RecordKind::Handoff.as_str(), "handoff");
    /// assert_eq!(serde_json::to_value(RecordKind::Handoff).unwrap(), "handoff");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            RecordKind::Observation => "observation",
            RecordKind::Hypothesis => "hypothesis",
            RecordKind::Plan => "plan",
            RecordKind::Decision => "decision",
            RecordKind::Blocker => "blocker",
            RecordKind::Handoff => "handoff",
            RecordKind::Outcome => "outcome",
        }
    }

    /// Whether two live records of this kind by different authors contend: two plans or two
    /// decisions for one task, or over meeting scopes, are a conflict to report. Findings
    /// (observations, hypotheses) may disagree without contending; they are superseded.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::RecordKind;
    /// assert!(RecordKind::Decision.contends());
    /// assert!(!RecordKind::Hypothesis.contends(), "findings disagree without contending");
    /// ```
    pub fn contends(self) -> bool {
        matches!(self, RecordKind::Plan | RecordKind::Decision)
    }
}

/// Where a record stands, as its author says. Only an open record is in force: a resolved or
/// withdrawn one contends with nothing and keeps no stream from compaction.
///
/// ```
/// use majordomus_cli::mesh::records::RecordStatus;
/// assert_eq!(RecordStatus::default(), RecordStatus::Open);
/// assert_eq!(serde_json::to_value(RecordStatus::Withdrawn).unwrap(), "withdrawn");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MeshRecordStatus")]
pub enum RecordStatus {
    /// In force.
    #[default]
    Open,
    /// Done with: answered, carried out, no longer blocking.
    Resolved,
    /// Taken back by its author.
    Withdrawn,
}

/// What a record cites, with its version: a file at a commit, a tool's observed result, or
/// another record at a revision.
///
/// ```
/// use majordomus_cli::mesh::records::RecordSource;
/// let file = RecordSource::File { path: "src/a.rs".into(), commit: "8df53bc26e9".into(),
///     lines: Some("10-20".into()) };
/// assert!(file.validate().is_ok() && file.is_retrievable());
/// let link = RecordSource::Record { record: "x/r-1".into(), revision: 2 };
/// assert!(!link.is_retrievable(), "a link to a record is provenance, not evidence");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[schemars(rename = "MeshRecordSource")]
pub enum RecordSource {
    /// A repository file at a commit, optionally a line range (`12` or `12-40`).
    File {
        /// Repository-relative path.
        path: String,
        /// The commit it was read at: 7 to 64 lowercase hex.
        commit: String,
        /// The lines: `N` or `N-M`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lines: Option<String>,
    },
    /// A tool run and what it observed.
    Tool {
        /// The command or tool call, one line.
        command: String,
        /// What it answered: an exit code, a digest, a short excerpt.
        result: String,
        /// When it was observed, RFC 3339 UTC.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        observed_at: Option<String>,
    },
    /// Another record at a revision.
    Record {
        /// The record's key, `<stream>/<id>`.
        record: String,
        /// The revision cited.
        revision: u32,
    },
}

impl RecordSource {
    /// Whether the source can be retrieved and checked by someone else: a file at a commit,
    /// or a tool's result. A record citing only links or nothing is an assertion.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::RecordSource;
    /// let ran = RecordSource::Tool { command: "cargo test".into(), result: "exit 0".into(),
    ///     observed_at: None };
    /// assert!(ran.is_retrievable());
    /// let blank = RecordSource::Tool { command: "cargo test".into(), result: " ".into(),
    ///     observed_at: None };
    /// assert!(!blank.is_retrievable(), "a tool source without its result proves nothing");
    /// ```
    pub fn is_retrievable(&self) -> bool {
        match self {
            RecordSource::File { .. } | RecordSource::Tool { .. } => self.validate().is_ok(),
            RecordSource::Record { .. } => false,
        }
    }

    /// Check the source's bounds, on the writer and on every reader: a file names a
    /// repository-relative path and a 7-64 hex commit, a line range is `N` or `N-M` from 1, a
    /// tool names its command and its result, and a cited record is a key at a revision.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::RecordSource;
    /// let file = |lines: &str| RecordSource::File { path: "src/a.rs".into(),
    ///     commit: "8df53bc".into(), lines: Some(lines.into()) };
    /// assert!(file("3-9").validate().is_ok());
    /// assert!(file("9-3").validate().unwrap_err().contains("lines"));
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        match self {
            RecordSource::File {
                path,
                commit,
                lines,
            } => {
                paths(std::slice::from_ref(path), true)?;
                let hex = (7..=64).contains(&commit.len())
                    && commit
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
                if !hex {
                    return Err(format!(
                        "source commit '{commit}' is not 7 to 64 lowercase hex"
                    ));
                }
                if let Some(range) = lines {
                    line_range(range)?;
                }
                Ok(())
            }
            RecordSource::Tool {
                command,
                result,
                observed_at,
            } => {
                if command.trim().is_empty() || result.trim().is_empty() {
                    return Err("a tool source names its command and its result".into());
                }
                line("source command", command, 512)?;
                text("source result", result, 2048)?;
                if let Some(at) = observed_at {
                    rfc3339("source observed_at", at)?;
                }
                Ok(())
            }
            RecordSource::Record { record, revision } => {
                record_key(record)?;
                if *revision == 0 {
                    return Err("a cited record's revision starts at 1".into());
                }
                Ok(())
            }
        }
    }
}

fn line_range(range: &str) -> Result<(), String> {
    let bad = || format!("source lines '{range}' are not N or N-M with 1 <= N <= M");
    let (a, b) = match range.split_once('-') {
        Some((a, b)) => (a, b),
        None => (range, range),
    };
    let (a, b): (u32, u32) = (a.parse().map_err(|_| bad())?, b.parse().map_err(|_| bad())?);
    if a == 0 || b < a {
        return Err(bad());
    }
    Ok(())
}

fn rfc3339(name: &str, value: &str) -> Result<u64, String> {
    crate::peers::parse_rfc3339(value)
        .ok_or_else(|| format!("{name} '{value}' is not RFC 3339 UTC (YYYY-MM-DDTHH:MM:SSZ)"))
}

/// A record key, `<stream>/<id>`: the stream parsed, the id an identifier.
fn record_key(key: &str) -> Result<(), String> {
    let ok = key.split_once('/').is_some_and(|(stream, id)| {
        StreamId::parse(stream).is_some() && ident("record", id).is_ok()
    });
    if ok {
        Ok(())
    } else {
        Err(format!("'{key}' is not a record key, <stream>/<id>"))
    }
}

/// What a handoff must carry. Every field is required: a handoff that leaves one out is
/// refused, because the next worker cannot ask the one who wrote it.
///
/// ```
/// use majordomus_cli::mesh::records::Handoff;
/// let mut h = Handoff { objective: "ship".into(), current_state: "built".into(),
///     evidence: vec!["cargo test: ok".into()], blockers: Some(vec![]),
///     next_action: "open the PR".into() };
/// assert!(h.validate().is_ok());
/// h.evidence.clear();
/// assert!(h.validate().unwrap_err().contains("evidence"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshHandoff")]
pub struct Handoff {
    /// What the work is for.
    pub objective: String,
    /// Where it stands now.
    pub current_state: String,
    /// What shows it: commands run with their results, commits, records. At least one.
    #[serde(default)]
    pub evidence: Vec<String>,
    /// What blocks it; `[]` says nothing does. Required: silence is not "no blockers".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blockers: Option<Vec<String>>,
    /// What the next worker does first.
    pub next_action: String,
}

impl Handoff {
    /// Check that every part is present and bounded: the three paragraphs non-empty, at
    /// least one evidence item, and the blockers stated (`[]` for none) rather than omitted.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::Handoff;
    /// let h = Handoff { objective: "ship".into(), current_state: "built".into(),
    ///     evidence: vec!["cargo test: ok".into()], blockers: None,
    ///     next_action: "open the PR".into() };
    /// assert!(h.validate().unwrap_err().contains("blockers"), "silence is not none");
    /// assert!(Handoff { blockers: Some(vec![]), ..h }.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("objective", &self.objective),
            ("current_state", &self.current_state),
            ("next_action", &self.next_action),
        ] {
            if value.trim().is_empty() {
                return Err(format!("a handoff carries its {name}"));
            }
            text(name, value, HANDOFF_TEXT_MAX)?;
        }
        if self.evidence.is_empty() {
            return Err("a handoff carries its evidence: at least one item".into());
        }
        let Some(blockers) = &self.blockers else {
            return Err("a handoff says its blockers: [] when nothing blocks".into());
        };
        for (name, items) in [("evidence", &self.evidence), ("blockers", blockers)] {
            if items.len() > MAX_HANDOFF_ITEMS {
                return Err(format!(
                    "a handoff lists at most {MAX_HANDOFF_ITEMS} {name}"
                ));
            }
            for item in items {
                if item.trim().is_empty() {
                    return Err(format!("a handoff's {name} item is empty"));
                }
                line(name, item, HANDOFF_ITEM_MAX)?;
            }
        }
        Ok(())
    }
}

/// A record as the journal carries it: one revision of it. The author is not in it — the
/// event's stream and the `session` within it are — and neither is its evidence state, which
/// is derived from its sources.
///
/// ```
/// use majordomus_cli::mesh::journal::EventBody;
/// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
/// let body = RecordBody::new("r-1", "s1", RecordKind::Blocker, "I1", &["ci"], "CI is red");
/// let wire = serde_json::to_value(EventBody::RecordWritten { record: body }).unwrap();
/// assert_eq!(wire["kind"], "record_written");
/// assert_eq!(wire["record"]["kind"], "blocker", "the record's own kind, inside it");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshRecordBody")]
pub struct RecordBody {
    /// The record's id within its stream; its key is `<stream>/<id>`.
    pub id: String,
    /// The writing session within the stream.
    pub session: String,
    /// The revision this write makes, from 1.
    pub revision: u32,
    /// What the record is.
    pub kind: RecordKind,
    /// The task, intent or issue it is about.
    pub task: String,
    /// The repository paths it is about; inside the writing session's claims.
    pub scope: Vec<String>,
    /// Where it stands.
    #[serde(default)]
    pub status: RecordStatus,
    /// One line.
    pub title: String,
    /// The text, up to [`RECORD_BODY_MAX`] bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// What it cites, with versions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<RecordSource>,
    /// A handoff's parts; present exactly when the kind is `handoff`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<Handoff>,
    /// The record this one supersedes, `<stream>/<id>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// When it stops holding, RFC 3339 UTC: stale after it on the reader's clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// The writer's retry key: a write repeated with the same key and content is answered
    /// with the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<String>,
    /// When it was written, RFC 3339, as its author's clock said; informational.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_at: Option<String>,
}

impl RecordBody {
    /// A first revision: open, citing nothing, superseding nothing, with no lease. The
    /// constructor of a test or a script; admission builds what a session writes.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::{RecordBody, RecordKind, RecordStatus};
    /// let r = RecordBody::new("r-1", "s1", RecordKind::Plan, "I1", &["docs"], "write it");
    /// assert_eq!((r.revision, r.status), (1, RecordStatus::Open));
    /// assert!(r.validate().is_ok());
    /// ```
    pub fn new(
        id: &str,
        session: &str,
        kind: RecordKind,
        task: &str,
        scope: &[&str],
        title: &str,
    ) -> Self {
        RecordBody {
            id: id.into(),
            session: session.into(),
            revision: 1,
            kind,
            task: task.into(),
            scope: scope.iter().map(|p| p.to_string()).collect(),
            status: RecordStatus::Open,
            title: title.into(),
            body: None,
            sources: Vec::new(),
            handoff: None,
            supersedes: None,
            expires_at: None,
            idempotency: None,
            written_at: None,
        }
    }

    /// Check every bound; run on the writer before signing and on every reader at ingest,
    /// so that nothing a peer sends is stored unchecked and nothing signed here is refused
    /// there. A handoff must carry its parts, and only a handoff may.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
    /// let r = RecordBody::new("r-1", "s1", RecordKind::Handoff, "I1", &["docs"], "over");
    /// assert!(r.validate().unwrap_err().contains("handoff carries"));
    /// let escaping = RecordBody::new("r-1", "s1", RecordKind::Plan, "I1", &["../etc"], "x");
    /// assert!(escaping.validate().is_err(), "a scope is repository-relative");
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        ident("record", &self.id)?;
        ident("session", &self.session)?;
        if self.revision == 0 {
            return Err("a record's revision starts at 1".into());
        }
        if self.task.trim().is_empty() {
            return Err("a record names its task, intent or issue".into());
        }
        line("task", &self.task, RECORD_TASK_MAX)?;
        paths(&self.scope, true)?;
        if self.title.trim().is_empty() {
            return Err("a record has a title".into());
        }
        line("title", &self.title, RECORD_TITLE_MAX)?;
        if let Some(body) = &self.body {
            text("body", body, RECORD_BODY_MAX)?;
        }
        if self.sources.len() > MAX_RECORD_SOURCES {
            return Err(format!(
                "a record cites at most {MAX_RECORD_SOURCES} sources"
            ));
        }
        for source in &self.sources {
            source.validate()?;
        }
        match (self.kind, &self.handoff) {
            (RecordKind::Handoff, Some(h)) => h.validate()?,
            (RecordKind::Handoff, None) => return Err(
                "a handoff carries objective, current_state, evidence, blockers and next_action"
                    .into(),
            ),
            (_, Some(_)) => return Err("only a handoff carries handoff parts".into()),
            (_, None) => {}
        }
        if let Some(key) = &self.supersedes {
            record_key(key)?;
        }
        if let Some(at) = &self.expires_at {
            rfc3339("expires_at", at)?;
        }
        if let Some(key) = &self.idempotency {
            ident("idempotency", key)?;
        }
        if let Some(at) = &self.written_at {
            line("written_at", at, 64)?;
        }
        let size = serde_json::to_vec(self)
            .map(|v| v.len())
            .unwrap_or(usize::MAX);
        if size > RECORD_MAX_BYTES {
            return Err(format!(
                "a record of {size} bytes exceeds {RECORD_MAX_BYTES}"
            ));
        }
        Ok(())
    }

    /// The digest of what the record says, leaving out who wrote it, which revision and
    /// when: two writes with equal digests say the same thing. An idempotent retry is
    /// recognised by it.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
    /// let a = RecordBody::new("r-1", "s1", RecordKind::Plan, "I1", &["docs"], "write it");
    /// let b = RecordBody { id: "r-2".into(), session: "s2".into(), revision: 3, ..a.clone() };
    /// assert_eq!(a.content_digest(), b.content_digest(), "who and which revision aside");
    /// let c = RecordBody { title: "write less".into(), ..a.clone() };
    /// assert_ne!(a.content_digest(), c.content_digest());
    /// ```
    pub fn content_digest(&self) -> String {
        use sha2::{Digest, Sha256};
        // what it says, and nothing of who wrote it, which revision or when
        let value = serde_json::json!({
            "kind": self.kind,
            "task": self.task,
            "scope": self.scope,
            "status": self.status,
            "title": self.title,
            "body": self.body,
            "sources": self.sources,
            "handoff": self.handoff,
            "supersedes": self.supersedes,
            "expires_at": self.expires_at,
        });
        let hash = Sha256::digest(canonical_json(&value));
        hash.iter().take(16).map(|b| format!("{b:02x}")).collect()
    }

    /// The derived evidence state of this revision: evidenced when any source is
    /// retrievable, an assertion otherwise. Never written by the author, so never claimed.
    ///
    /// ```
    /// use majordomus_cli::mesh::records::{EvidenceState, RecordBody, RecordKind, RecordSource};
    /// let mut r = RecordBody::new("r-1", "s1", RecordKind::Observation, "I1", &["src"], "leaks");
    /// assert_eq!(r.evidence(), EvidenceState::Asserted);
    /// r.sources.push(RecordSource::File { path: "src/a.rs".into(), commit: "8df53bc".into(),
    ///     lines: Some("12".into()) });
    /// assert_eq!(r.evidence(), EvidenceState::Evidenced);
    /// ```
    pub fn evidence(&self) -> EvidenceState {
        if self.sources.iter().any(RecordSource::is_retrievable) {
            EvidenceState::Evidenced
        } else {
            EvidenceState::Asserted
        }
    }
}

/// Whether a record rests on something another worker can retrieve. Derived from the
/// sources by [`RecordBody::evidence`]; a record carries no field that says it.
///
/// ```
/// use majordomus_cli::mesh::records::EvidenceState;
/// assert_eq!(serde_json::to_value(EvidenceState::Asserted).unwrap(), "asserted");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MeshEvidenceState")]
pub enum EvidenceState {
    /// No retrievable source: what its author says.
    Asserted,
    /// At least one retrievable source: a file at a commit, or a tool's observed result.
    Evidenced,
}

/// Where a record's author stands, as this runtime reads it now: a reading of the session's
/// close and its runtime's beat, so a record whose author is gone is flagged on every reader.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
/// use majordomus_cli::mesh::state::fold;
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::RecordWritten { record:
///     RecordBody::new("r-1", "s1", RecordKind::Plan, "I1", &["docs"], "p") }).unwrap();
/// use majordomus_cli::mesh::records::AuthorState;
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// assert_eq!(state.records[0].author_state, AuthorState::Unknown, "s1 never opened here");
/// assert!(!state.records[0].stale.is_empty(), "and the record says so");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MeshAuthorState")]
pub enum AuthorState {
    /// Its session is open and its runtime beats.
    Active,
    /// Its session closed.
    Closed,
    /// Its runtime stopped beating.
    Expired,
    /// This runtime never heard the session open.
    Unknown,
}

/// One revision in a record's history: what it said and where it was written. Every
/// revision is kept, which is the record's provenance.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
/// use majordomus_cli::mesh::state::fold;
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// let plan = RecordBody::new("r-1", "s1", RecordKind::Plan, "I1", &["docs"], "first");
/// j.append_own(EventBody::RecordWritten { record: plan.clone() }).unwrap();
/// j.append_own(EventBody::RecordWritten { record:
///     RecordBody { revision: 2, title: "second".into(), ..plan } }).unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// use majordomus_cli::mesh::records::RecordRevision;
/// let history: &[RecordRevision] = &state.records[0].history;
/// let titles: Vec<&str> = history.iter().map(|h| h.title.as_str()).collect();
/// assert_eq!(titles, ["first", "second"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshRecordRevision")]
pub struct RecordRevision {
    /// The revision.
    pub revision: u32,
    /// `<stream>/<seq>` of the event.
    pub event: String,
    /// The event's Lamport stamp.
    pub lamport: u64,
    /// The status it set.
    pub status: RecordStatus,
    /// The title it set.
    pub title: String,
    /// The sources it cited.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<RecordSource>,
    /// Its evidence state.
    pub evidence: EvidenceState,
    /// The digest of what it said ([`RecordBody::content_digest`]).
    pub digest: String,
    /// Its retry key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<String>,
    /// When its author wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_at: Option<String>,
}

/// A record as every runtime folds it: its current revision, its author, its links, its
/// standing, and every revision before.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
/// use majordomus_cli::mesh::state::fold;
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::RecordWritten { record:
///     RecordBody::new("r-1", "s1", RecordKind::Decision, "I1", &["docs"], "d") }).unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// use majordomus_cli::mesh::records::RecordView;
/// let view: &RecordView = &state.records[0];
/// assert_eq!(view.key, format!("{}/r-1", j.own_stream()));
/// assert_eq!(view.author, format!("{}/s1", j.own_stream()), "the stream's, not a name");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshRecordView")]
pub struct RecordView {
    /// `<stream>/<id>`.
    pub key: String,
    /// `<stream>/<session>` of the session that created it: the only one that revises it.
    pub author: String,
    /// `<node>-<runtime>` of its stream.
    pub runtime: String,
    /// The repository identity the event was signed for.
    pub repository: String,
    /// What it is.
    pub kind: RecordKind,
    /// The task, intent or issue.
    pub task: String,
    /// Its paths.
    pub scope: Vec<String>,
    /// Its status.
    pub status: RecordStatus,
    /// Its title.
    pub title: String,
    /// Its text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Its sources.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<RecordSource>,
    /// A handoff's parts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<Handoff>,
    /// Derived from its sources.
    pub evidence: EvidenceState,
    /// The current revision: what a revision must name to replace it.
    pub revision: u32,
    /// When it stops holding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// The record it supersedes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// The records that supersede it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub superseded_by: Vec<String>,
    /// Where its author stands.
    pub author_state: AuthorState,
    /// Why it is stale; empty while it holds. Stale records are flagged, never removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale: Vec<String>,
    /// Every revision, oldest first.
    pub history: Vec<RecordRevision>,
}

impl RecordView {
    /// In force: open, not superseded, not stale. Only live records contend, and only they
    /// are what a reader should act on.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::journal::{EventBody, Journal, SessionInfo, StreamLiveness};
    /// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
    /// use majordomus_cli::mesh::state::fold;
    /// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
    ///     "repo".into(), None).unwrap();
    /// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "cli") }).unwrap();
    /// j.append_own(EventBody::RecordWritten { record:
    ///     RecordBody::new("r-1", "s1", RecordKind::Plan, "I1", &["docs"], "p") }).unwrap();
    /// assert!(fold(&j.events(), &|_| StreamLiveness::Own).records[0].is_live());
    /// assert!(!fold(&j.events(), &|_| StreamLiveness::Expired).records[0].is_live(),
    ///     "its author's runtime stopped beating");
    /// ```
    pub fn is_live(&self) -> bool {
        self.status == RecordStatus::Open && self.superseded_by.is_empty() && self.stale.is_empty()
    }
}

/// Why two live plans or decisions of different authors contend: the same task, which is
/// checked first, or scopes that meet, when the tasks differ.
///
/// ```
/// use majordomus_cli::mesh::records::ConflictReason;
/// assert_eq!(serde_json::to_value(ConflictReason::SameTask).unwrap(), "same_task");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MeshConflictReason")]
pub enum ConflictReason {
    /// Both are about the same task.
    SameTask,
    /// Their scopes meet.
    OverlappingScope,
}

/// Two live plans or two live decisions of different authors for one task or over meeting
/// scopes: reported, not refused — a supersession settles it.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
/// use majordomus_cli::mesh::state::fold;
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// use majordomus_cli::mesh::records::ConflictReason;
/// use majordomus_cli::mesh::journal::SessionInfo;
/// for s in ["s1", "s2"] {
///     j.append_own(EventBody::SessionOpened { info: SessionInfo::named(s, "cli") }).unwrap();
/// }
/// for (id, s) in [("r-1", "s1"), ("r-2", "s2")] {
///     j.append_own(EventBody::RecordWritten { record:
///         RecordBody::new(id, s, RecordKind::Decision, "I1", &["docs"], id) }).unwrap();
/// }
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// use majordomus_cli::mesh::records::RecordConflict;
/// let conflicts: &[RecordConflict] = &state.record_conflicts;
/// assert_eq!(conflicts.len(), 1);
/// assert_eq!(conflicts[0].reason, ConflictReason::SameTask);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshRecordConflict")]
pub struct RecordConflict {
    /// The kind both are.
    pub kind: RecordKind,
    /// The earlier record's key.
    pub first: String,
    /// The later record's key.
    pub second: String,
    /// Why they contend.
    pub reason: ConflictReason,
    /// The paths that meet, the first's and the second's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<(String, String)>,
}

/// A record write the fold did not apply, and why: a revision by a session other than the
/// author, out of order, after the author's session closed, or of another kind. A peer can
/// sign whatever its own stream says; it cannot make another session's record say it.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::records::{RecordBody, RecordKind};
/// use majordomus_cli::mesh::state::fold;
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// let mine = RecordBody::new("r-1", "s1", RecordKind::Plan, "I1", &["docs"], "mine");
/// j.append_own(EventBody::RecordWritten { record: mine.clone() }).unwrap();
/// j.append_own(EventBody::RecordWritten { record:
///     RecordBody { session: "s2".into(), revision: 2, title: "theirs".into(), ..mine } })
///     .unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// assert_eq!(state.records[0].title, "mine");
/// use majordomus_cli::mesh::records::IgnoredRecordWrite;
/// let ignored: &IgnoredRecordWrite = &state.records_ignored[0];
/// assert!(ignored.reason.contains("only its author"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshIgnoredRecordWrite")]
pub struct IgnoredRecordWrite {
    /// `<stream>/<seq>` of the event.
    pub event: String,
    /// The record it named.
    pub record: String,
    /// Why it was not applied.
    pub reason: String,
}

/// The records of a fold, built event by event in causal order.
#[derive(Debug, Default)]
pub(crate) struct RecordFold {
    records: BTreeMap<String, RecordView>,
    created: BTreeMap<String, u64>,
    ignored: Vec<IgnoredRecordWrite>,
}

impl RecordFold {
    /// Apply one `record_written` event. `closed_at` is the fold's map of session closes so
    /// far: a revision after its author's session closed is not the author's.
    pub(crate) fn apply(
        &mut self,
        event: &MeshEvent,
        record: RecordBody,
        closed_at: &BTreeMap<String, u64>,
    ) {
        let stream = &event.stream;
        let key = format!("{stream}/{}", record.id);
        let author = format!("{stream}/{}", record.session);
        let revision = RecordRevision {
            revision: record.revision,
            event: event.id(),
            lamport: event.lamport,
            status: record.status,
            title: record.title.clone(),
            sources: record.sources.clone(),
            evidence: record.evidence(),
            digest: record.content_digest(),
            idempotency: record.idempotency.clone(),
            written_at: record.written_at.clone(),
        };
        let mut ignore = |reason: String| {
            self.ignored.push(IgnoredRecordWrite {
                event: event.id(),
                record: key.clone(),
                reason,
            })
        };
        match self.records.get_mut(&key) {
            None if record.revision == 1 => {
                self.created.insert(key.clone(), event.lamport);
                self.records.insert(
                    key.clone(),
                    RecordView {
                        key,
                        author,
                        runtime: stream.runtime_key(),
                        repository: event.repo.clone(),
                        kind: record.kind,
                        task: record.task,
                        scope: record.scope,
                        status: record.status,
                        title: record.title,
                        body: record.body,
                        sources: record.sources,
                        handoff: record.handoff,
                        evidence: revision.evidence,
                        revision: 1,
                        expires_at: record.expires_at,
                        supersedes: record.supersedes,
                        superseded_by: Vec::new(),
                        author_state: AuthorState::Unknown,
                        stale: Vec::new(),
                        history: vec![revision],
                    },
                );
            }
            None => ignore(format!(
                "revision {} of a record this runtime never saw created",
                record.revision
            )),
            Some(_) if record.revision == 1 => ignore("a second creation of one record".into()),
            Some(view) => {
                if view.author != author {
                    ignore(format!(
                        "written by {author}, and only its author {} revises it",
                        view.author
                    ));
                } else if record.revision != view.revision + 1 {
                    ignore(format!(
                        "revision {} does not follow revision {}",
                        record.revision, view.revision
                    ));
                } else if view.kind != record.kind {
                    ignore(format!(
                        "a {} cannot be revised into a {}",
                        view.kind.as_str(),
                        record.kind.as_str()
                    ));
                } else if closed_at
                    .get(&author)
                    .is_some_and(|closed| self.created.get(&view.key).is_some_and(|c| c < closed))
                {
                    ignore("its author's session closed after creating it".into());
                } else {
                    view.revision = record.revision;
                    view.task = record.task;
                    view.scope = record.scope;
                    view.status = record.status;
                    view.title = record.title;
                    view.body = record.body;
                    view.sources = record.sources;
                    view.handoff = record.handoff;
                    view.evidence = revision.evidence;
                    view.expires_at = record.expires_at;
                    view.supersedes = record.supersedes;
                    view.history.push(revision);
                }
            }
        }
    }

    /// The records with their links, their authors' standing and what contends, and the
    /// writes the fold ignored.
    pub(crate) fn finish(
        mut self,
        sessions: &BTreeMap<String, SessionView>,
    ) -> (
        Vec<RecordView>,
        Vec<RecordConflict>,
        Vec<IgnoredRecordWrite>,
    ) {
        let links: Vec<(String, String)> = self
            .records
            .values()
            .filter_map(|r| r.supersedes.clone().map(|old| (old, r.key.clone())))
            .collect();
        for (old, new) in links {
            if old == new {
                continue;
            }
            // each record names one predecessor, so a link is pushed once; a predecessor not
            // heard here (yet) is a dangling link the superseding record still shows
            if let Some(view) = self.records.get_mut(&old) {
                view.superseded_by.push(new);
            }
        }
        for view in self.records.values_mut() {
            view.author_state = match sessions.get(&view.author).map(|s| s.state) {
                Some(SessionState::Active) => AuthorState::Active,
                Some(SessionState::Closed) => AuthorState::Closed,
                Some(SessionState::Expired) => AuthorState::Expired,
                None => AuthorState::Unknown,
            };
            match view.author_state {
                AuthorState::Active => {}
                AuthorState::Closed => view.stale.push("its author's session closed".into()),
                AuthorState::Expired => view
                    .stale
                    .push("its author's runtime stopped beating".into()),
                AuthorState::Unknown => view
                    .stale
                    .push("its author's session is not known here".into()),
            }
        }
        let records: Vec<RecordView> = self.records.into_values().collect();
        let conflicts = conflicts(&records);
        (records, conflicts, self.ignored)
    }
}

/// Mark every record whose expiry has passed at `now` (seconds since the epoch) stale. A
/// reading of the reader's clock, like a stream's liveness; nothing is removed.
pub(crate) fn judge_leases(records: &mut [RecordView], now: u64) -> bool {
    let mut changed = false;
    for view in records.iter_mut() {
        let Some(at) = view.expires_at.as_deref() else {
            continue;
        };
        if crate::peers::parse_rfc3339(at).is_some_and(|expiry| expiry <= now) {
            let why = format!("its lease expired at {at}");
            if !view.stale.contains(&why) {
                view.stale.push(why);
                changed = true;
            }
        }
    }
    changed
}

/// The contending pairs among live records, in key order.
pub(crate) fn conflicts(records: &[RecordView]) -> Vec<RecordConflict> {
    let live: Vec<&RecordView> = records
        .iter()
        .filter(|r| r.is_live() && r.kind.contends())
        .collect();
    let mut out = Vec::new();
    for (i, first) in live.iter().enumerate() {
        for second in &live[i + 1..] {
            if first.kind != second.kind || first.author == second.author {
                continue;
            }
            let paths = meeting(&first.scope, &second.scope);
            let reason = if first.task == second.task {
                ConflictReason::SameTask
            } else if !paths.is_empty() {
                ConflictReason::OverlappingScope
            } else {
                continue;
            };
            out.push(RecordConflict {
                kind: first.kind,
                first: first.key.clone(),
                second: second.key.clone(),
                reason,
                paths,
            });
        }
    }
    out
}

fn meeting(a: &[String], b: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for x in a {
        for y in b {
            if crate::peers::claims_meet(x, y) {
                out.push((x.clone(), y.clone()));
            }
        }
    }
    out
}

/// Whether `path` lies inside `claimed`: equal, or under it.
///
/// ```
/// use majordomus_cli::mesh::records::within;
/// assert!(within("apps/cli/src", "apps"));
/// assert!(within("docs/", "docs"));
/// assert!(!within("apps", "apps/cli"), "a wider path is not inside a narrower claim");
/// assert!(!within("appsx", "apps"));
/// ```
pub fn within(path: &str, claimed: &str) -> bool {
    let (p, c) = (
        path.trim().trim_end_matches('/'),
        claimed.trim().trim_end_matches('/'),
    );
    !p.is_empty() && !c.is_empty() && (p == c || p.starts_with(&format!("{c}/")))
}

/// What a session asks to write: a new record, or a revision of its own. The author is
/// never in it — the session is named apart and the stream is the runtime's own.
///
/// ```
/// use majordomus_cli::mesh::records::{RecordKind, RecordWrite};
/// let w: RecordWrite = serde_json::from_value(serde_json::json!({
///     "kind": "decision", "task": "I1", "scope": ["docs"], "title": "one kind"
/// })).unwrap();
/// assert_eq!(w.kind, Some(RecordKind::Decision));
/// assert!(w.record.is_none(), "a new record, not a revision");
/// assert!(serde_json::from_value::<RecordWrite>(serde_json::json!({"author": "x"})).is_err(),
///     "an author is never an input");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "MeshRecordWrite")]
pub struct RecordWrite {
    /// The writing session; the calling MCP session's own when omitted. Over MCP a session
    /// writes for itself alone; over the command line and HTTP it must be named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// To revise: the record's key, `<stream>/<id>`, as the first write answered it.
    /// Absent: a new record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// To revise: the revision the writer read. A revision that is no longer current is
    /// refused as `stale_revision` with the current one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u32>,
    /// `observation`, `hypothesis`, `plan`, `decision`, `blocker`, `handoff` or `outcome`.
    pub kind: Option<RecordKind>,
    /// The task, intent or issue it is about.
    #[serde(default)]
    pub task: String,
    /// Repository-relative paths, each inside a live claim of the writing session.
    #[serde(default)]
    pub scope: Vec<String>,
    /// `open` (the default), `resolved` or `withdrawn`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<RecordStatus>,
    /// One line.
    #[serde(default)]
    pub title: String,
    /// The text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// What it cites: `{"type":"file","path","commit","lines"}`,
    /// `{"type":"tool","command","result","observed_at"}` or `{"type":"record","record","revision"}`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<RecordSource>,
    /// A handoff's parts: objective, current_state, evidence, blockers, next_action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<Handoff>,
    /// A new record superseding another, `<stream>/<id>`: the old one is kept and linked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// When it stops holding, RFC 3339 UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// A retry key: the same key with the same content answers the first write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<String>,
}

/// Why a write was refused: every refusal fails closed and names what to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecordRefusal {
    /// The record is malformed.
    Invalid(String),
    /// A path lies outside the writing session's live claims.
    OutOfScope(String),
    /// The record is another session's.
    NotOwn(String),
    /// The revision read is not the current one.
    Stale {
        /// The record.
        record: String,
        /// Its current revision.
        current: u32,
        /// Why.
        why: String,
    },
    /// The record or the one superseded is not known here.
    NotFound(String),
    /// The retry key was used for other content.
    IdempotencyConflict(String),
}

/// What admission decided: a body to sign and append, or the answer a retry already got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Admission {
    /// Append this body.
    Append(Box<RecordBody>),
    /// A retry of a write already made: answer with that revision of that record.
    Repeat {
        /// The record's key.
        key: String,
        /// The revision first written.
        revision: RecordRevision,
    },
}

/// Decide a write of `session` of the runtime whose stream is `stream` against the folded
/// `records` and `claims`, with `fresh_id` the id a new record takes. Pure: the caller holds
/// the admission lock and appends what comes back.
pub(crate) fn admit(
    records: &[RecordView],
    claims: &[ClaimView],
    stream: &StreamId,
    session: &str,
    write: RecordWrite,
    fresh_id: &str,
    written_at: String,
) -> Result<Admission, RecordRefusal> {
    let author = format!("{stream}/{session}");
    let Some(kind) = write.kind else {
        return Err(RecordRefusal::Invalid(
            "a record names its kind: observation, hypothesis, plan, decision, blocker, handoff or outcome".into(),
        ));
    };
    let target = match &write.record {
        Some(key) => Some(
            records
                .iter()
                .find(|r| &r.key == key)
                .ok_or_else(|| RecordRefusal::NotFound(format!("no record {key} here")))?,
        ),
        None => None,
    };
    let (id, revision) = match target {
        Some(view) => (
            view.key.rsplit('/').next().unwrap_or_default().to_string(),
            view.revision + 1,
        ),
        None => (fresh_id.to_string(), 1),
    };
    let body = RecordBody {
        id,
        session: session.to_string(),
        revision,
        kind,
        task: write.task,
        scope: write.scope,
        status: write.status.unwrap_or_default(),
        title: write.title,
        body: write.body,
        sources: write.sources,
        handoff: write.handoff,
        supersedes: write.supersedes,
        expires_at: write.expires_at,
        idempotency: write.idempotency,
        written_at: Some(written_at),
    };
    body.validate().map_err(RecordRefusal::Invalid)?;

    // a retry: the same key from the same author answers what it answered the first time
    if let Some(retry) = &body.idempotency {
        let digest = body.content_digest();
        let earlier = records
            .iter()
            .filter(|r| r.author == author)
            .flat_map(|r| r.history.iter().map(move |h| (r, h)))
            .find(|(_, h)| h.idempotency.as_deref() == Some(retry.as_str()));
        if let Some((view, first)) = earlier {
            let same_target = write.record.as_deref().is_none_or(|k| k == view.key);
            if first.digest == digest && same_target {
                return Ok(Admission::Repeat {
                    key: view.key.clone(),
                    revision: first.clone(),
                });
            }
            return Err(RecordRefusal::IdempotencyConflict(format!(
                "retry key '{retry}' already wrote revision {} of {} with other content",
                first.revision, view.key
            )));
        }
    }

    if let Some(view) = target {
        if view.author != author {
            return Err(RecordRefusal::NotOwn(format!(
                "{} is {}'s record; only its author revises it — write a new record with supersedes: {} instead",
                view.key, view.author, view.key
            )));
        }
        let Some(read) = write.revision else {
            return Err(RecordRefusal::Invalid(format!(
                "a revision names the revision it read: revision {} is current",
                view.revision
            )));
        };
        if read != view.revision {
            return Err(RecordRefusal::Stale {
                record: view.key.clone(),
                current: view.revision,
                why: format!("revision {read} was read and {} is current", view.revision),
            });
        }
        if !view.superseded_by.is_empty() {
            return Err(RecordRefusal::Stale {
                record: view.key.clone(),
                current: view.revision,
                why: format!("it is superseded by {}", view.superseded_by.join(", ")),
            });
        }
        if view.kind != kind {
            return Err(RecordRefusal::Invalid(format!(
                "a {} cannot be revised into a {}; write a new record",
                view.kind.as_str(),
                kind.as_str()
            )));
        }
        if view.author_state != AuthorState::Active {
            return Err(RecordRefusal::NotOwn(format!(
                "{}'s author session is no longer active; supersede it",
                view.key
            )));
        }
    } else if write.revision.is_some() {
        return Err(RecordRefusal::Invalid(
            "a revision is given only with the record it revises".into(),
        ));
    }

    if let Some(old) = &body.supersedes {
        if target.is_some_and(|t| &t.key == old) || !records.iter().any(|r| &r.key == old) {
            return Err(RecordRefusal::NotFound(format!(
                "supersedes {old}, which is not another record known here"
            )));
        }
    }

    // the scope: every path inside a live claim of this very session
    let held: Vec<&ClaimView> = claims
        .iter()
        .filter(|c| c.session == author && c.state.is_live())
        .collect();
    let outside: Vec<&String> = body
        .scope
        .iter()
        .filter(|p| !held.iter().any(|c| c.scope.iter().any(|s| within(p, s))))
        .collect();
    if !outside.is_empty() {
        return Err(RecordRefusal::OutOfScope(format!(
            "{} lie outside every live claim of session {author}; claim them first (mesh.claim, or announce them)",
            outside.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(Admission::Append(Box::new(body)))
}

/// The answer of a record write: the record's key, the revision now current, the event that
/// says so, and the live records it contends with — reported, not refused.
///
/// ```
/// use majordomus_cli::mesh::records::{EvidenceState, RecordWriteAnswer};
/// let a = RecordWriteAnswer { key: "s/r-1".into(), revision: 1, event: "s/7".into(),
///     lamport: 7, durability: Default::default(), evidence: EvidenceState::Asserted,
///     repeated: false, conflicts: vec![] };
/// let wire = serde_json::to_value(&a).unwrap();
/// assert!(wire.get("repeated").is_none(), "said only when it answers a retry");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshRecordWriteAnswer")]
pub struct RecordWriteAnswer {
    /// `<stream>/<id>`: how the record is addressed from here on.
    pub key: String,
    /// The revision this write made; the next revision names it.
    pub revision: u32,
    /// `<stream>/<seq>` of the event.
    pub event: String,
    /// The event's Lamport stamp.
    pub lamport: u64,
    /// Whether the event reached this machine's disk; `unknown` for a repeated answer.
    #[serde(default)]
    pub durability: Durability,
    /// Derived from its sources.
    pub evidence: EvidenceState,
    /// True when this answers a retry with the first write, and nothing was written now.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeated: bool,
    /// The live plans or decisions of other authors it contends with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<RecordConflict>,
}

/// What a reader asks of the records. Every filter narrows; none is required, and an input
/// it does not know is refused rather than ignored.
///
/// ```
/// use majordomus_cli::mesh::records::RecordQuery;
/// let q: RecordQuery = serde_json::from_str(r#"{"kind":"blocker","live":true}"#).unwrap();
/// assert_eq!(q.live, Some(true));
/// assert!(serde_json::from_str::<RecordQuery>(r#"{"kinds":"blocker"}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "MeshRecordQuery")]
pub struct RecordQuery {
    /// One record by its key, `<stream>/<id>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// Only records about this task, intent or issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Only records of this kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<RecordKind>,
    /// Only records of this author, `<stream>/<session>`, or of a session of that name on
    /// any runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Only records whose scope meets this path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Only live records: open, not superseded, not stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live: Option<bool>,
}

/// The answer of `mesh.records`, built by [`select`]: the records the query admits, the
/// conflicts among them, the writes the fold ignored, and the whole fold's digest.
///
/// ```
/// use majordomus_cli::mesh::records::RecordList;
/// let off = RecordList { reason: Some("no shared server activated the mesh".into()),
///     ..RecordList::default() };
/// let wire = serde_json::to_value(&off).unwrap();
/// assert_eq!(wire["active"], false, "an inactive mesh is an answer, not an error");
/// assert!(wire.get("conflicts").is_none() && wire["records"].as_array().unwrap().is_empty());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshRecordList")]
pub struct RecordList {
    /// Whether this process runs the mesh. `false` is an answer, not an error: the records
    /// are then empty and `reason` says why, as `mesh.state` says it.
    pub active: bool,
    /// Why the mesh is not active here, when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The whole fold's digest: equal digests, equal records on two runtimes.
    pub digest: String,
    /// The records, in key order.
    pub records: Vec<RecordView>,
    /// The conflicts that name a listed record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<RecordConflict>,
    /// The ignored writes that name a listed record, or every one when nothing narrows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<IgnoredRecordWrite>,
}

/// The records of `records` the query admits, with the conflicts that name one of them and
/// the ignored writes about them (all of them when nothing narrows), under the whole fold's
/// digest.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::records::{select, RecordBody, RecordKind, RecordQuery};
/// use majordomus_cli::mesh::state::fold;
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// for (id, kind) in [("r-1", RecordKind::Plan), ("r-2", RecordKind::Blocker)] {
///     j.append_own(EventBody::RecordWritten { record:
///         RecordBody::new(id, "s1", kind, "I1", &["docs"], id) }).unwrap();
/// }
/// let s = fold(&j.events(), &|_| StreamLiveness::Own);
/// let q = RecordQuery { kind: Some(RecordKind::Blocker), ..RecordQuery::default() };
/// let list = select(&s.records, &s.record_conflicts, &s.records_ignored, &s.digest, &q);
/// assert_eq!(list.records.len(), 1);
/// assert_eq!(list.digest, s.digest, "the whole fold's");
/// ```
pub fn select(
    records: &[RecordView],
    conflicts: &[RecordConflict],
    ignored: &[IgnoredRecordWrite],
    digest: &str,
    query: &RecordQuery,
) -> RecordList {
    let admitted: Vec<RecordView> = records
        .iter()
        .filter(|r| query.record.as_deref().is_none_or(|k| r.key == k))
        .filter(|r| query.task.as_deref().is_none_or(|t| r.task == t))
        .filter(|r| query.kind.is_none_or(|k| r.kind == k))
        .filter(|r| {
            query.author.as_deref().is_none_or(|a| {
                r.author == a || r.author.rsplit('/').next().is_some_and(|s| s == a)
            })
        })
        .filter(|r| {
            query
                .path
                .as_deref()
                .is_none_or(|p| r.scope.iter().any(|s| crate::peers::claims_meet(s, p)))
        })
        .filter(|r| query.live.is_none_or(|live| r.is_live() == live))
        .cloned()
        .collect();
    let keys: std::collections::BTreeSet<&str> = admitted.iter().map(|r| r.key.as_str()).collect();
    let narrowed = *query != RecordQuery::default();
    RecordList {
        active: true,
        reason: None,
        digest: digest.to_string(),
        conflicts: conflicts
            .iter()
            .filter(|c| keys.contains(c.first.as_str()) || keys.contains(c.second.as_str()))
            .cloned()
            .collect(),
        ignored: ignored
            .iter()
            .filter(|i| !narrowed || keys.contains(i.record.as_str()))
            .cloned()
            .collect(),
        records: admitted,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::mesh::config::{CooperationConfig, TrustConfig};
    use crate::mesh::cooperation::{
        CheckoutFacts, Cooperation, CooperationError, CooperationSetup,
    };
    use crate::mesh::identity::NodeIdentity;
    use crate::mesh::journal::{
        ClaimMode, EventBody, Journal, MeshEvent, SessionInfo, StreamLiveness,
    };
    use crate::mesh::link::HttpTransport;
    use crate::mesh::registry::MeshRegistry;
    use crate::mesh::repository::of_root_commits;
    use crate::mesh::state::fold;
    use crate::mesh::testing;

    const COMMIT: &str = "8df53bc26e9589492cb1bfe2b7aff7fa0d26a79e";

    fn claim(c: &Cooperation, session: &str, scope: &[&str]) -> String {
        c.claim(
            &SessionInfo::named(session, "test"),
            scope.iter().map(|p| p.to_string()).collect(),
            None,
            ClaimMode::Advisory,
            None,
        )
        .unwrap()
        .key
    }

    fn record(kind: RecordKind, task: &str, scope: &[&str], title: &str) -> RecordWrite {
        RecordWrite {
            kind: Some(kind),
            task: task.into(),
            scope: scope.iter().map(|p| p.to_string()).collect(),
            title: title.into(),
            ..RecordWrite::default()
        }
    }

    fn file(path: &str) -> RecordSource {
        RecordSource::File {
            path: path.into(),
            commit: COMMIT.into(),
            lines: Some("10-20".into()),
        }
    }

    fn revise(key: &str, read: u32, write: RecordWrite) -> RecordWrite {
        RecordWrite {
            record: Some(key.into()),
            revision: Some(read),
            ..write
        }
    }

    fn view(c: &Cooperation, key: &str) -> RecordView {
        c.state()
            .records
            .into_iter()
            .find(|r| r.key == key)
            .unwrap_or_else(|| panic!("no record {key}"))
    }

    /// Two sessions that decide the same task at once are both kept: neither write is
    /// refused, and the fold reports them as contending — on one runtime, and on two
    /// linked runtimes that decided before either heard the other, which converge on the
    /// same records, the same conflict and the same digest.
    #[test]
    fn two_sessions_deciding_one_task_are_both_kept_and_reported() {
        let c = testing::cooperation();
        claim(&c, "s1", &["apps/majordomus-cli"]);
        claim(&c, "s2", &["apps/majordomus-cli/src/mesh"]);
        let first = c
            .write_record(
                "s1",
                record(
                    RecordKind::Decision,
                    "I4665",
                    &["apps/majordomus-cli/src/mesh"],
                    "one kind",
                ),
            )
            .unwrap();
        assert!(first.conflicts.is_empty());
        let second = c
            .write_record(
                "s2",
                record(
                    RecordKind::Decision,
                    "I4665",
                    &["apps/majordomus-cli/src/mesh/records.rs"],
                    "two kinds",
                ),
            )
            .unwrap();
        assert_eq!(second.conflicts.len(), 1, "{:?}", second.conflicts);
        assert_eq!(second.conflicts[0].reason, ConflictReason::SameTask);
        let state = c.state();
        assert_eq!(state.records.len(), 2, "both kept");
        assert_eq!(state.record_conflicts.len(), 1);

        // plans of other tasks whose scopes meet contend too; findings never do
        c.write_record(
            "s1",
            record(RecordKind::Plan, "A", &["apps/majordomus-cli"], "a"),
        )
        .unwrap();
        let plan = c
            .write_record(
                "s2",
                record(
                    RecordKind::Plan,
                    "B",
                    &["apps/majordomus-cli/src/mesh"],
                    "b",
                ),
            )
            .unwrap();
        assert_eq!(plan.conflicts[0].reason, ConflictReason::OverlappingScope);
        assert!(!plan.conflicts[0].paths.is_empty());
        c.write_record(
            "s1",
            record(
                RecordKind::Observation,
                "I4665",
                &["apps/majordomus-cli"],
                "x",
            ),
        )
        .unwrap();
        let seen = c
            .write_record(
                "s2",
                record(
                    RecordKind::Observation,
                    "I4665",
                    &["apps/majordomus-cli/src/mesh"],
                    "y",
                ),
            )
            .unwrap();
        assert!(
            seen.conflicts.is_empty(),
            "findings are superseded, they do not contend"
        );

        // two runtimes, each deciding before it heard the other
        let net = Arc::new(testing::Net::default());
        let (a, b) = (testing::runtime(&net, "a:1"), testing::runtime(&net, "b:1"));
        let link = a.dial(&["b:1".into()]).unwrap();
        claim(&a, "s1", &["docs"]);
        claim(&b, "s2", &["docs"]);
        a.write_record("s1", record(RecordKind::Decision, "T", &["docs"], "left"))
            .unwrap();
        b.write_record("s2", record(RecordKind::Decision, "T", &["docs"], "right"))
            .unwrap();
        for _ in 0..3 {
            a.journal().beat_own();
            b.journal().beat_own();
            a.sync_with(&link).unwrap();
        }
        let (sa, sb) = (a.state(), b.state());
        assert_eq!(sa.records.len(), 2);
        assert_eq!(sa.record_conflicts.len(), 1, "{:?}", sa.record_conflicts);
        assert_eq!(
            sa.record_conflicts, sb.record_conflicts,
            "both name the same conflict"
        );
        assert_eq!(sa.digest, sb.digest, "one state, two runtimes");
    }

    /// A finding is superseded by another session's, never overwritten: the old record stays
    /// with its link to what superseded it, and a decision superseded stops contending.
    #[test]
    fn conflicting_findings_are_superseded_and_kept() {
        let c = testing::cooperation();
        claim(&c, "s1", &["src"]);
        claim(&c, "s2", &["src"]);
        let old = c
            .write_record(
                "s1",
                record(RecordKind::Observation, "T", &["src/a.rs"], "a leaks"),
            )
            .unwrap();
        assert_eq!(
            old.evidence,
            EvidenceState::Asserted,
            "no source: an assertion"
        );
        let new = c
            .write_record(
                "s2",
                RecordWrite {
                    supersedes: Some(old.key.clone()),
                    sources: vec![file("src/a.rs")],
                    ..record(
                        RecordKind::Observation,
                        "T",
                        &["src/a.rs"],
                        "a does not leak",
                    )
                },
            )
            .unwrap();
        assert_eq!(new.evidence, EvidenceState::Evidenced);
        let kept = view(&c, &old.key);
        assert_eq!(kept.superseded_by, vec![new.key.clone()]);
        assert_eq!(kept.title, "a leaks", "superseded, not overwritten");
        assert!(!kept.is_live());
        assert_eq!(
            view(&c, &new.key).supersedes.as_deref(),
            Some(old.key.as_str())
        );
        // the superseded record is not revised any more: its author is told what replaced it
        let late = c
            .write_record(
                "s1",
                revise(
                    &old.key,
                    1,
                    record(RecordKind::Observation, "T", &["src/a.rs"], "still"),
                ),
            )
            .unwrap_err();
        assert!(
            matches!(&late, CooperationError::StaleRevision { why, .. } if why.contains("superseded")),
            "{late}"
        );

        // two decisions contend until one is superseded
        let d1 = c
            .write_record("s1", record(RecordKind::Decision, "T", &["src"], "use A"))
            .unwrap();
        let d2 = c
            .write_record("s2", record(RecordKind::Decision, "T", &["src"], "use B"))
            .unwrap();
        assert_eq!(d2.conflicts.len(), 1);
        c.write_record(
            "s2",
            revise(
                &d2.key,
                1,
                RecordWrite {
                    status: Some(RecordStatus::Withdrawn),
                    ..record(RecordKind::Decision, "T", &["src"], "use B")
                },
            ),
        )
        .unwrap();
        assert!(
            c.state().record_conflicts.is_empty(),
            "a withdrawn decision contends with nothing"
        );
        let d3 = c
            .write_record(
                "s2",
                RecordWrite {
                    supersedes: Some(d1.key.clone()),
                    ..record(RecordKind::Decision, "T", &["src"], "use C")
                },
            )
            .unwrap();
        assert!(d3.conflicts.is_empty());
        assert_eq!(c.state().records.len(), 5, "nothing removed");
    }

    /// Compare-and-swap: a revision names the revision it read, and a stale one is refused
    /// with the current revision; a revision with no revision read is refused too.
    #[test]
    fn a_stale_revision_is_refused_with_the_current_one() {
        let c = testing::cooperation();
        claim(&c, "s1", &["docs"]);
        let plan = record(RecordKind::Plan, "T", &["docs"], "write the docs");
        let first = c.write_record("s1", plan.clone()).unwrap();
        let second = c
            .write_record("s1", revise(&first.key, 1, plan.clone()))
            .unwrap();
        assert_eq!(
            (second.key.as_str(), second.revision),
            (first.key.as_str(), 2)
        );
        match c
            .write_record("s1", revise(&first.key, 1, plan.clone()))
            .unwrap_err()
        {
            CooperationError::StaleRevision {
                record, current, ..
            } => {
                assert_eq!((record.as_str(), current), (first.key.as_str(), 2));
            }
            other => panic!("expected a stale revision, got {other}"),
        }
        let unread = RecordWrite {
            record: Some(first.key.clone()),
            ..plan.clone()
        };
        assert!(matches!(
            c.write_record("s1", unread),
            Err(CooperationError::Invalid(_))
        ));
        let kind = revise(
            &first.key,
            2,
            RecordWrite {
                kind: Some(RecordKind::Decision),
                ..plan
            },
        );
        assert!(matches!(
            c.write_record("s1", kind),
            Err(CooperationError::Invalid(_))
        ));
        assert_eq!(view(&c, &first.key).revision, 2);
    }

    /// A record past its lease is stale on the reader's clock and kept; it stops contending.
    /// A record whose author's session closed, or whose runtime stopped beating, is flagged
    /// the same way, and is no longer revised.
    #[test]
    fn a_stale_lease_and_a_gone_author_are_flagged_not_deleted() {
        let c = testing::cooperation();
        claim(&c, "s1", &["docs"]);
        claim(&c, "s2", &["docs"]);
        let lapsed = c
            .write_record(
                "s1",
                RecordWrite {
                    expires_at: Some("2000-01-01T00:00:00Z".into()),
                    ..record(RecordKind::Decision, "T", &["docs"], "old")
                },
            )
            .unwrap();
        let holding = c
            .write_record(
                "s2",
                RecordWrite {
                    expires_at: Some("2999-01-01T00:00:00Z".into()),
                    ..record(RecordKind::Decision, "T", &["docs"], "new")
                },
            )
            .unwrap();
        assert!(
            holding.conflicts.is_empty(),
            "a lapsed decision contends with nothing"
        );
        let state = c.state();
        let old = state.records.iter().find(|r| r.key == lapsed.key).unwrap();
        assert!(
            old.stale.iter().any(|w| w.contains("lease expired")),
            "{:?}",
            old.stale
        );
        assert!(state
            .records
            .iter()
            .find(|r| r.key == holding.key)
            .unwrap()
            .stale
            .is_empty());

        c.close_session("s2").unwrap();
        let closed = view(&c, &holding.key);
        assert_eq!(closed.author_state, AuthorState::Closed);
        assert!(closed.stale.iter().any(|w| w.contains("session closed")));
        let refused = c
            .write_record(
                "s2",
                revise(
                    &holding.key,
                    1,
                    record(RecordKind::Decision, "T", &["docs"], "again"),
                ),
            )
            .unwrap_err();
        assert!(matches!(refused, CooperationError::NotOwn(_)), "{refused}");

        let dead = fold(&c.journal().events(), &|_| StreamLiveness::Expired);
        assert_eq!(
            dead.records.len(),
            2,
            "a dead runtime's records are still there"
        );
        assert!(dead
            .records
            .iter()
            .all(|r| r.author_state != AuthorState::Active && !r.stale.is_empty()));
    }

    /// A record's scope lies inside a live claim of the writing session, or it is refused:
    /// another session's claim, a released claim, a sibling path and no claim at all are not.
    #[test]
    fn a_write_outside_the_claimed_scope_is_refused() {
        let c = testing::cooperation();
        let held = claim(&c, "s1", &["docs"]);
        claim(&c, "s2", &["apps"]);
        let out = |session: &str, path: &str| {
            matches!(
                c.write_record(session, record(RecordKind::Observation, "T", &[path], "x")),
                Err(CooperationError::OutOfScope(_))
            )
        };
        assert!(out("s1", "apps"), "another session's claim");
        assert!(out("s1", "docsx"), "a sibling, not inside");
        assert!(out("s3", "docs"), "no claim at all");
        assert!(!out("s1", "docs/MESH.md"));
        c.release(&held).unwrap();
        assert!(out("s1", "docs/MESH.md"), "a released claim");
        assert_eq!(c.state().records.len(), 1);
    }

    /// A handoff carries objective, current state, evidence, blockers and next action; one
    /// missing is refused, and only a handoff carries them.
    #[test]
    fn a_handoff_missing_evidence_is_refused() {
        let c = testing::cooperation();
        claim(&c, "s1", &["docs"]);
        let parts = Handoff {
            objective: "document records".into(),
            current_state: "drafted".into(),
            evidence: vec!["cargo test --lib mesh::records: ok".into()],
            blockers: Some(vec![]),
            next_action: "open the pull request".into(),
        };
        let handoff = |h: Option<Handoff>| RecordWrite {
            handoff: h,
            ..record(RecordKind::Handoff, "T", &["docs"], "over to you")
        };
        let invalid = |w: RecordWrite, word: &str| match c.write_record("s1", w) {
            Err(CooperationError::Invalid(m)) => assert!(m.contains(word), "{m}"),
            other => panic!("expected invalid ({word}), got {other:?}"),
        };
        invalid(
            handoff(Some(Handoff {
                evidence: vec![],
                ..parts.clone()
            })),
            "evidence",
        );
        invalid(
            handoff(Some(Handoff {
                blockers: None,
                ..parts.clone()
            })),
            "blockers",
        );
        invalid(
            handoff(Some(Handoff {
                next_action: " ".into(),
                ..parts.clone()
            })),
            "next_action",
        );
        invalid(handoff(None), "handoff carries");
        invalid(
            RecordWrite {
                handoff: Some(parts.clone()),
                ..record(RecordKind::Plan, "T", &["docs"], "p")
            },
            "only a handoff",
        );
        assert!(c.state().records.is_empty(), "nothing written");
        c.write_record("s1", handoff(Some(parts))).unwrap();
    }

    /// A write repeated with its retry key and the same content is answered with the first
    /// and writes nothing; the same key with other content is refused.
    #[test]
    fn an_idempotent_retry_answers_the_first_write() {
        let c = testing::cooperation();
        claim(&c, "s1", &["docs"]);
        let write = RecordWrite {
            idempotency: Some("k-1".into()),
            ..record(RecordKind::Blocker, "T", &["docs"], "CI is red")
        };
        let first = c.write_record("s1", write.clone()).unwrap();
        let events = c.journal().events().len();
        let again = c.write_record("s1", write.clone()).unwrap();
        assert!(again.repeated && !first.repeated);
        assert_eq!(
            (again.key, again.event, again.revision),
            (first.key.clone(), first.event, 1)
        );
        assert_eq!(
            c.journal().events().len(),
            events,
            "nothing written for a retry"
        );
        let other = RecordWrite {
            title: "CI is green".into(),
            ..write
        };
        assert!(matches!(
            c.write_record("s1", other),
            Err(CooperationError::IdempotencyConflict(_))
        ));
        // another session's key space is its own
        claim(&c, "s2", &["docs"]);
        let theirs = RecordWrite {
            idempotency: Some("k-1".into()),
            ..record(RecordKind::Blocker, "T", &["docs"], "x")
        };
        assert!(!c.write_record("s2", theirs).unwrap().repeated);
    }

    /// A plan revised keeps every revision: what each said, what each cited and its
    /// evidence state, in order, with the event that wrote it.
    #[test]
    fn a_revised_plan_keeps_its_provenance() {
        let c = testing::cooperation();
        claim(&c, "s1", &["src"]);
        let draft = RecordWrite {
            sources: vec![file("src/a.rs")],
            ..record(RecordKind::Plan, "T", &["src"], "split a.rs")
        };
        let first = c.write_record("s1", draft.clone()).unwrap();
        let tool = RecordSource::Tool {
            command: "cargo test --lib a".into(),
            result: "exit 0, 12 passed".into(),
            observed_at: Some("2026-11-20T01:00:00Z".into()),
        };
        c.write_record(
            "s1",
            revise(
                &first.key,
                1,
                RecordWrite {
                    sources: vec![tool.clone()],
                    ..record(RecordKind::Plan, "T", &["src"], "split a.rs in two")
                },
            ),
        )
        .unwrap();
        let plan = view(&c, &first.key);
        assert_eq!(plan.revision, 2);
        assert_eq!(plan.history.len(), 2);
        assert_eq!(plan.history[0].title, "split a.rs");
        assert_eq!(plan.history[0].sources, vec![file("src/a.rs")]);
        assert_eq!(plan.history[0].event, first.event);
        assert_eq!(plan.history[1].sources, vec![tool.clone()]);
        assert_eq!(plan.sources, vec![tool]);
        assert!(plan
            .history
            .iter()
            .all(|h| h.evidence == EvidenceState::Evidenced));
        assert_ne!(plan.history[0].digest, plan.history[1].digest);
    }

    /// A session cannot revise another's record through admission, and a revision written
    /// into the journal anyway — by another session of the same runtime, after the author's
    /// session closed, or from another runtime's stream under the author's session name — is
    /// not applied: the fold names it among the ignored writes.
    #[test]
    fn a_session_cannot_forge_another_sessions_record() {
        let c = testing::cooperation();
        claim(&c, "s1", &["docs"]);
        claim(&c, "s2", &["docs"]);
        let mine = c
            .write_record("s1", record(RecordKind::Decision, "T", &["docs"], "mine"))
            .unwrap();
        let refused = c
            .write_record(
                "s2",
                revise(
                    &mine.key,
                    1,
                    record(RecordKind::Decision, "T", &["docs"], "theirs"),
                ),
            )
            .unwrap_err();
        assert!(
            matches!(&refused, CooperationError::NotOwn(m) if m.contains("supersedes")),
            "{refused}"
        );

        let id = mine.key.rsplit('/').next().unwrap().to_string();
        let forged = |session: &str| RecordBody {
            id: id.clone(),
            session: session.into(),
            revision: 2,
            kind: RecordKind::Decision,
            task: "T".into(),
            scope: vec!["docs".into()],
            status: RecordStatus::Open,
            title: "forged".into(),
            body: None,
            sources: vec![],
            handoff: None,
            supersedes: None,
            expires_at: None,
            idempotency: None,
            written_at: None,
        };
        c.journal()
            .append_own(EventBody::RecordWritten {
                record: forged("s2"),
            })
            .unwrap();
        let state = c.state();
        assert_eq!(view(&c, &mine.key).title, "mine");
        assert!(state
            .records_ignored
            .iter()
            .any(|i| i.record == mine.key && i.reason.contains("only its author")));

        // another runtime signs only its own stream: the author's name there is another key
        let other = Journal::open(
            Arc::new(NodeIdentity::ephemeral().unwrap()),
            "0000000000000002",
            c.journal().repository().into(),
            None,
        )
        .unwrap();
        other
            .append_own(EventBody::RecordWritten {
                record: forged("s1"),
            })
            .unwrap();
        let mut events = c.journal().events();
        events.extend(other.events());
        let both = fold(&events, &|_| StreamLiveness::Own);
        let kept = both.records.iter().find(|r| r.key == mine.key).unwrap();
        assert_eq!((kept.revision, kept.title.as_str()), (1, "mine"));
        assert!(both
            .records_ignored
            .iter()
            .any(|i| i.reason.contains("never saw created")));

        // and the author itself, once its session closed, revises nothing
        c.close_session("s1").unwrap();
        c.journal()
            .append_own(EventBody::RecordWritten {
                record: forged("s1"),
            })
            .unwrap();
        assert_eq!(view(&c, &mine.key).title, "mine");
        assert!(c
            .state()
            .records_ignored
            .iter()
            .any(|i| i.reason.contains("closed")));
    }

    /// The records live in the journal on disk: a runtime restarted on the same file folds
    /// the same records, revisions, links and history it wrote, with their authors gone.
    #[test]
    fn the_records_come_back_from_the_journal_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        let machine = Arc::new(NodeIdentity::ephemeral().unwrap());
        let open = |runtime: &str| {
            Cooperation::new(CooperationSetup {
                identity: Arc::clone(&machine),
                runtime: runtime.into(),
                repository: of_root_commits(&["root".into()]),
                endpoints: vec!["127.0.0.1:9".into()],
                version: "test".into(),
                config: CooperationConfig::default(),
                trust: TrustConfig::default(),
                journal_path: Some(path.clone()),
                registry: Arc::new(MeshRegistry::new()),
                transport: Arc::new(HttpTransport),
                board: None,
                checkout: CheckoutFacts::default(),
            })
            .unwrap()
        };
        let before = open("0000000000000001");
        claim(&before, "s1", &["docs"]);
        let plan = before
            .write_record("s1", record(RecordKind::Plan, "T", &["docs"], "a"))
            .unwrap();
        assert_eq!(
            plan.durability,
            Durability::Synced,
            "on disk before it is answered"
        );
        before
            .write_record(
                "s1",
                revise(&plan.key, 1, record(RecordKind::Plan, "T", &["docs"], "b")),
            )
            .unwrap();
        before
            .write_record(
                "s1",
                RecordWrite {
                    supersedes: Some(plan.key.clone()),
                    ..record(RecordKind::Decision, "T", &["docs"], "c")
                },
            )
            .unwrap();
        let written = before.state().records;
        drop(before);

        let after = open("0000000000000002");
        let reloaded = after.state().records;
        assert_eq!(reloaded.len(), written.len());
        for (was, is) in written.iter().zip(&reloaded) {
            assert_eq!(
                (&was.key, was.revision, &was.history),
                (&is.key, is.revision, &is.history)
            );
            assert_eq!(
                (&was.supersedes, &was.superseded_by),
                (&is.supersedes, &is.superseded_by)
            );
            assert_ne!(is.author_state, AuthorState::Active, "its run is over");
            assert!(!is.stale.is_empty(), "flagged, not lost");
        }
    }

    /// The bounds hold on every field a peer could send.
    #[test]
    fn a_malformed_record_is_refused() {
        let good = RecordBody {
            id: "r-1".into(),
            session: "s1".into(),
            revision: 1,
            kind: RecordKind::Observation,
            task: "T".into(),
            scope: vec!["docs".into()],
            status: RecordStatus::Open,
            title: "t".into(),
            body: None,
            sources: vec![file("docs/MESH.md")],
            handoff: None,
            supersedes: None,
            expires_at: None,
            idempotency: None,
            written_at: None,
        };
        assert!(good.validate().is_ok());
        assert!(EventBody::RecordWritten {
            record: good.clone()
        }
        .validate()
        .is_ok());
        let bad = [
            RecordBody {
                revision: 0,
                ..good.clone()
            },
            RecordBody {
                id: "not an id".into(),
                ..good.clone()
            },
            RecordBody {
                scope: vec![],
                ..good.clone()
            },
            RecordBody {
                scope: vec!["../etc".into()],
                ..good.clone()
            },
            RecordBody {
                title: "two\nlines".into(),
                ..good.clone()
            },
            RecordBody {
                task: String::new(),
                ..good.clone()
            },
            RecordBody {
                supersedes: Some("nope".into()),
                ..good.clone()
            },
            RecordBody {
                expires_at: Some("tomorrow".into()),
                ..good.clone()
            },
            RecordBody {
                sources: vec![RecordSource::File {
                    path: "a".into(),
                    commit: "XYZ".into(),
                    lines: None,
                }],
                ..good.clone()
            },
            RecordBody {
                sources: vec![RecordSource::File {
                    path: "a".into(),
                    commit: COMMIT.into(),
                    lines: Some("9-2".into()),
                }],
                ..good.clone()
            },
            RecordBody {
                sources: vec![file("a"); MAX_RECORD_SOURCES + 1],
                ..good.clone()
            },
            RecordBody {
                body: Some("x".repeat(RECORD_BODY_MAX + 1)),
                ..good.clone()
            },
        ];
        for body in bad {
            assert!(body.validate().is_err(), "{body:?}");
        }
        assert_eq!(
            EventBody::RecordWritten { record: good }.kind(),
            "record_written"
        );
    }

    fn journal() -> Journal {
        Journal::open(
            Arc::new(NodeIdentity::ephemeral().unwrap()),
            "0000000000000001",
            "repo".into(),
            None,
        )
        .unwrap()
    }

    fn written(j: &Journal, record: RecordBody) {
        j.append_own(EventBody::RecordWritten { record }).unwrap();
    }

    fn opened(j: &Journal, session: &str) {
        j.append_own(EventBody::SessionOpened {
            info: SessionInfo::named(session, "test"),
        })
        .unwrap();
    }

    /// Every word of the closed vocabulary is its own, and every bound a peer could cross
    /// names the field it crossed: a malformed record is refused, never stored.
    #[test]
    fn every_kind_has_its_word_and_every_bound_names_its_field() {
        let kinds = [
            (RecordKind::Observation, "observation"),
            (RecordKind::Hypothesis, "hypothesis"),
            (RecordKind::Plan, "plan"),
            (RecordKind::Decision, "decision"),
            (RecordKind::Blocker, "blocker"),
            (RecordKind::Handoff, "handoff"),
            (RecordKind::Outcome, "outcome"),
        ];
        for (kind, word) in kinds {
            assert_eq!(kind.as_str(), word);
            assert_eq!(serde_json::to_value(kind).unwrap(), word);
        }

        let refused = |source: RecordSource, word: &str| {
            let why = source.validate().unwrap_err();
            assert!(why.contains(word), "{why}");
        };
        let file = |path: &str, lines: Option<&str>| RecordSource::File {
            path: path.into(),
            commit: COMMIT.into(),
            lines: lines.map(Into::into),
        };
        assert!(
            file("src/a.rs", None).validate().is_ok(),
            "lines are optional"
        );
        assert!(file("src/a.rs", Some("12")).validate().is_ok(), "one line");
        refused(file("../a.rs", None), "repository-relative");
        refused(file("a.rs", Some("x-5")), "lines");
        refused(file("a.rs", Some("5-y")), "lines");
        refused(file("a.rs", Some("0")), "lines");
        let tool = |command: &str, result: &str, at: Option<&str>| RecordSource::Tool {
            command: command.into(),
            result: result.into(),
            observed_at: at.map(Into::into),
        };
        assert!(tool("cargo test", "ok", None).validate().is_ok());
        refused(tool(" ", "ok", None), "command and its result");
        refused(tool("two\nlines", "ok", None), "source command");
        refused(tool("cargo test", &"x".repeat(2049), None), "source result");
        refused(tool("cargo test", "ok", Some("yesterday")), "observed_at");
        let cited = |record: &str, revision| RecordSource::Record {
            record: record.into(),
            revision,
        };
        let key = format!(
            "{}-{}-{}/r-1",
            "a".repeat(32),
            "b".repeat(16),
            "c".repeat(16)
        );
        assert!(cited(&key, 2).validate().is_ok());
        assert!(!cited(&key, 2).is_retrievable(), "a link, not evidence");
        refused(cited("not-a-key", 1), "record key");
        refused(cited(&key, 0), "starts at 1");

        let parts = Handoff {
            objective: "o".into(),
            current_state: "c".into(),
            evidence: vec!["e".into()],
            blockers: Some(vec![]),
            next_action: "n".into(),
        };
        let handoff = |h: Handoff| h.validate().unwrap_err();
        assert!(handoff(Handoff {
            objective: "x".repeat(HANDOFF_TEXT_MAX + 1),
            ..parts.clone()
        })
        .contains("objective"));
        assert!(handoff(Handoff {
            evidence: vec!["e".into(); MAX_HANDOFF_ITEMS + 1],
            ..parts.clone()
        })
        .contains("at most"));
        assert!(handoff(Handoff {
            blockers: Some(vec![" ".into()]),
            ..parts.clone()
        })
        .contains("blockers item is empty"));
        assert!(handoff(Handoff {
            evidence: vec!["two\nlines".into()],
            ..parts.clone()
        })
        .contains("evidence"));

        let good = RecordBody::new("r-1", "s1", RecordKind::Plan, "T", &["docs"], "t");
        assert!(good.validate().is_ok());
        let body = |b: RecordBody| b.validate().unwrap_err();
        assert!(body(RecordBody {
            session: "a b".into(),
            ..good.clone()
        })
        .contains("session"));
        assert!(body(RecordBody {
            task: "a\nb".into(),
            ..good.clone()
        })
        .contains("task"));
        assert!(body(RecordBody {
            title: " ".into(),
            ..good.clone()
        })
        .contains("title"));
        assert!(body(RecordBody {
            idempotency: Some("a b".into()),
            ..good.clone()
        })
        .contains("idempotency"));
        assert!(body(RecordBody {
            written_at: Some("a\nb".into()),
            ..good.clone()
        })
        .contains("written_at"));
        let huge = RecordBody {
            kind: RecordKind::Handoff,
            scope: (0..64).map(|i| format!("{i:0>250}")).collect(),
            body: Some("x".repeat(RECORD_BODY_MAX)),
            handoff: Some(Handoff {
                evidence: vec!["e".repeat(HANDOFF_ITEM_MAX); MAX_HANDOFF_ITEMS],
                blockers: Some(vec!["b".repeat(HANDOFF_ITEM_MAX); MAX_HANDOFF_ITEMS]),
                ..parts
            }),
            ..good
        };
        assert!(body(huge).contains("exceeds"));
    }

    /// What a runtime signs out of order or out of shape changes no record: a second
    /// creation, a skipped revision, a change of kind; a record superseding itself links to
    /// nothing, a record superseding one never heard keeps its dangling link, and a record of
    /// a session never heard to open is flagged.
    #[test]
    fn the_fold_applies_only_what_follows_and_flags_what_it_cannot_place() {
        let j = journal();
        opened(&j, "s1");
        let plan = RecordBody::new("r-1", "s1", RecordKind::Plan, "T", &["docs"], "plan");
        written(&j, plan.clone());
        written(
            &j,
            RecordBody {
                title: "again".into(),
                ..plan.clone()
            },
        );
        written(
            &j,
            RecordBody {
                revision: 3,
                title: "skipped".into(),
                ..plan.clone()
            },
        );
        written(
            &j,
            RecordBody {
                revision: 2,
                kind: RecordKind::Decision,
                ..plan.clone()
            },
        );
        let own = format!("{}/r-2", j.own_stream());
        written(
            &j,
            RecordBody {
                supersedes: Some(own.clone()),
                ..RecordBody::new("r-2", "s1", RecordKind::Observation, "T", &["docs"], "self")
            },
        );
        let elsewhere = format!(
            "{}-{}-{}/r-9",
            "a".repeat(32),
            "b".repeat(16),
            "c".repeat(16)
        );
        written(
            &j,
            RecordBody {
                supersedes: Some(elsewhere.clone()),
                ..RecordBody::new(
                    "r-3",
                    "s1",
                    RecordKind::Observation,
                    "T",
                    &["docs"],
                    "later",
                )
            },
        );
        written(
            &j,
            RecordBody::new("r-4", "s9", RecordKind::Blocker, "T", &["docs"], "who"),
        );

        let state = fold(&j.events(), &|_| StreamLiveness::Own);
        let view = |id: &str| {
            state
                .records
                .iter()
                .find(|r| r.key.ends_with(&format!("/{id}")))
                .unwrap()
        };
        assert_eq!(
            (view("r-1").revision, view("r-1").title.as_str()),
            (1, "plan")
        );
        let reasons: Vec<&str> = state
            .records_ignored
            .iter()
            .map(|i| i.reason.as_str())
            .collect();
        assert!(
            reasons.iter().any(|r| r.contains("second creation")),
            "{reasons:?}"
        );
        assert!(
            reasons.iter().any(|r| r.contains("does not follow")),
            "{reasons:?}"
        );
        assert!(
            reasons.iter().any(|r| r.contains("cannot be revised into")),
            "{reasons:?}"
        );
        assert!(
            view("r-2").superseded_by.is_empty(),
            "a record does not supersede itself"
        );
        assert_eq!(view("r-3").supersedes.as_deref(), Some(elsewhere.as_str()));
        assert_eq!(view("r-4").author_state, AuthorState::Unknown);
        assert!(view("r-4")
            .stale
            .iter()
            .any(|w| w.contains("not known here")));
    }

    /// Admission refuses, before anything is signed, a write that names no kind, a revision
    /// of a record not known here, a revision read given for a new record, and a
    /// supersession of an unknown record or of the record being revised; a retried revision
    /// is answered with its first write.
    #[test]
    fn admission_refuses_what_cannot_be_placed_and_answers_a_retried_revision() {
        let c = testing::cooperation();
        claim(&c, "s1", &["docs"]);
        let plan = record(RecordKind::Plan, "T", &["docs"], "p");
        let nameless = RecordWrite {
            kind: None,
            ..plan.clone()
        };
        assert!(
            matches!(c.write_record("s1", nameless), Err(CooperationError::Invalid(m)) if m.contains("kind"))
        );
        let ghost = format!("{}/r-none", c.journal().own_stream());
        assert!(matches!(
            c.write_record("s1", revise(&ghost, 1, plan.clone())),
            Err(CooperationError::NotFound(_))
        ));
        let unread = RecordWrite {
            revision: Some(1),
            ..plan.clone()
        };
        assert!(
            matches!(c.write_record("s1", unread), Err(CooperationError::Invalid(m)) if m.contains("only with the record"))
        );
        let orphan = RecordWrite {
            supersedes: Some(ghost),
            ..plan.clone()
        };
        assert!(matches!(
            c.write_record("s1", orphan),
            Err(CooperationError::NotFound(_))
        ));
        let first = c.write_record("s1", plan.clone()).unwrap();
        let onto_itself = RecordWrite {
            supersedes: Some(first.key.clone()),
            ..revise(&first.key, 1, plan.clone())
        };
        assert!(matches!(
            c.write_record("s1", onto_itself),
            Err(CooperationError::NotFound(_))
        ));
        let retried = RecordWrite {
            idempotency: Some("k-rev".into()),
            ..revise(
                &first.key,
                1,
                record(RecordKind::Plan, "T", &["docs"], "p2"),
            )
        };
        let once = c.write_record("s1", retried.clone()).unwrap();
        let twice = c.write_record("s1", retried).unwrap();
        assert!(twice.repeated && !once.repeated);
        assert_eq!((twice.revision, twice.event), (2, once.event));
    }

    /// A lease is judged once per reading: judging again adds no second reason and reports no
    /// change; a record without a lease is passed over. Two decisions of different tasks over
    /// scopes that do not meet do not contend.
    #[test]
    fn a_lease_is_judged_once_and_unrelated_decisions_do_not_contend() {
        let j = journal();
        opened(&j, "s1");
        opened(&j, "s2");
        written(
            &j,
            RecordBody {
                expires_at: Some("2000-01-01T00:00:00Z".into()),
                ..RecordBody::new("r-1", "s1", RecordKind::Decision, "A", &["docs"], "a")
            },
        );
        written(
            &j,
            RecordBody::new("r-2", "s2", RecordKind::Decision, "B", &["src"], "b"),
        );
        written(
            &j,
            RecordBody::new(
                "r-3",
                "s2",
                RecordKind::Decision,
                "C",
                &["docs/x", "src"],
                "c",
            ),
        );
        let mut state = fold(&j.events(), &|_| StreamLiveness::Own);
        assert!(judge_leases(&mut state.records, 1_788_000_000));
        assert!(
            !judge_leases(&mut state.records, 1_788_000_000),
            "judged once"
        );
        let lapsed = state
            .records
            .iter()
            .find(|r| r.key.ends_with("/r-1"))
            .unwrap();
        assert_eq!(
            lapsed.stale.iter().filter(|w| w.contains("lease")).count(),
            1
        );
        let live = conflicts(&state.records);
        assert!(
            live.is_empty(),
            "B and C share no task and are both s2's: {live:?}"
        );
        let state = fold(&j.events(), &|_| StreamLiveness::Own);
        assert_eq!(state.record_conflicts.len(), 1, "A (docs) meets C (docs/x)");
        assert_eq!(
            state.record_conflicts[0].reason,
            ConflictReason::OverlappingScope
        );
    }

    /// One record by its key, and the ignored writes of what a narrowed query lists.
    #[test]
    fn a_query_names_one_record_and_its_ignored_writes() {
        let j = journal();
        opened(&j, "s1");
        let plan = RecordBody::new("r-1", "s1", RecordKind::Plan, "T", &["docs"], "p");
        written(&j, plan.clone());
        written(
            &j,
            RecordBody {
                revision: 5,
                ..plan
            },
        );
        written(
            &j,
            RecordBody::new("r-2", "s1", RecordKind::Plan, "U", &["src"], "q"),
        );
        let s = fold(&j.events(), &|_| StreamLiveness::Own);
        let key = format!("{}/r-1", j.own_stream());
        let q = RecordQuery {
            record: Some(key.clone()),
            ..RecordQuery::default()
        };
        let one = select(
            &s.records,
            &s.record_conflicts,
            &s.records_ignored,
            &s.digest,
            &q,
        );
        assert_eq!(one.records.len(), 1);
        assert_eq!(one.ignored.len(), 1, "its skipped revision");
        let other = RecordQuery {
            task: Some("U".into()),
            ..RecordQuery::default()
        };
        let none = select(
            &s.records,
            &s.record_conflicts,
            &s.records_ignored,
            &s.digest,
            &other,
        );
        assert!(
            none.ignored.is_empty(),
            "the ignored write is about another record"
        );
        assert!(one.active && none.active);
    }

    /// The state's page narrows records like every other collection, and the summary counts
    /// them, the live ones and the conflicts.
    #[test]
    fn the_page_narrows_and_counts_records() {
        use crate::mesh::page::{narrow, summarize, Query};
        let j = journal();
        opened(&j, "s1");
        opened(&j, "s2");
        written(
            &j,
            RecordBody::new(
                "r-1",
                "s1",
                RecordKind::Decision,
                "T",
                &["docs"],
                "one kind",
            ),
        );
        written(
            &j,
            RecordBody::new("r-2", "s2", RecordKind::Decision, "T", &["docs"], "two"),
        );
        let plan = RecordBody::new("r-3", "s2", RecordKind::Plan, "U", &["src"], "plan");
        written(
            &j,
            RecordBody {
                status: RecordStatus::Resolved,
                ..plan.clone()
            },
        );
        written(
            &j,
            RecordBody {
                revision: 9,
                ..plan
            },
        );
        let state = fold(&j.events(), &|_| StreamLiveness::Own);
        let q = |v: serde_json::Value| -> Query { serde_json::from_value(v).unwrap() };
        let all = narrow(&state, &Query::default());
        assert_eq!((all.records.len(), all.record_conflicts.len()), (3, 1));
        assert_eq!(all.records_ignored.len(), 1);
        let open = narrow(&state, &q(serde_json::json!({"open": true})));
        assert_eq!(open.records.len(), 2, "the resolved plan is not open");
        let found = narrow(&state, &q(serde_json::json!({"matching": "ONE KIND"})));
        assert_eq!(found.records.len(), 1);
        assert_eq!(
            found.record_conflicts.len(),
            1,
            "a conflict naming it stays"
        );
        let theirs = narrow(&state, &q(serde_json::json!({"session": "s2", "limit": 1})));
        assert_eq!(theirs.records.len(), 1);
        let other = narrow(&state, &q(serde_json::json!({"kind": "claims"})));
        assert!(other.records.is_empty() && other.record_conflicts.is_empty());
        assert!(other.records_ignored.is_empty());
        let summary = summarize(&state, &[]);
        assert_eq!(
            (
                summary.records,
                summary.live_records,
                summary.record_conflicts
            ),
            (3, 2, 1)
        );
        assert_eq!(serde_json::to_value(&summary).unwrap()["records"], 3);
    }

    /// Compaction never deletes a record still in force: a dead stream holding an open record
    /// stays, one whose record is resolved goes.
    #[test]
    fn compaction_keeps_a_stream_while_it_holds_an_open_record() {
        use crate::mesh::journal::HANDOVER_RETENTION;
        use std::time::Duration;
        let (open, done, reader) = (
            journal(),
            Journal::open(
                Arc::new(NodeIdentity::ephemeral().unwrap()),
                "0000000000000003",
                "repo".into(),
                None,
            )
            .unwrap(),
            Journal::open(
                Arc::new(NodeIdentity::ephemeral().unwrap()),
                "0000000000000002",
                "repo".into(),
                None,
            )
            .unwrap(),
        );
        written(
            &open,
            RecordBody::new("r-1", "s1", RecordKind::Blocker, "T", &["ci"], "red"),
        );
        written(
            &done,
            RecordBody {
                status: RecordStatus::Resolved,
                ..RecordBody::new("r-1", "s1", RecordKind::Blocker, "T", &["ci"], "was red")
            },
        );
        let accept = |_: &MeshEvent| -> Result<(), crate::mesh::journal::Rejection> { Ok(()) };
        reader.ingest(&open.missing_for(&reader.marks(), usize::MAX), &accept);
        reader.ingest(&done.missing_for(&reader.marks(), usize::MAX), &accept);
        let dropped = reader.compact(Duration::ZERO, Duration::ZERO, HANDOVER_RETENTION, true);
        assert_eq!(dropped, 1, "the resolved record's stream goes");
        assert_eq!(&reader.events()[0].stream, open.own_stream());
    }

    /// `select` narrows by every filter and keeps the conflicts of what it lists.
    #[test]
    fn a_query_narrows_the_records() {
        let c = testing::cooperation();
        claim(&c, "s1", &["docs"]);
        claim(&c, "s2", &["docs"]);
        c.write_record("s1", record(RecordKind::Decision, "T", &["docs"], "a"))
            .unwrap();
        c.write_record(
            "s2",
            record(RecordKind::Decision, "T", &["docs/MESH.md"], "b"),
        )
        .unwrap();
        c.write_record("s2", record(RecordKind::Observation, "U", &["docs"], "c"))
            .unwrap();
        let state = c.state();
        let ask = |q: RecordQuery| {
            select(
                &state.records,
                &state.record_conflicts,
                &state.records_ignored,
                &state.digest,
                &q,
            )
        };
        assert_eq!(ask(RecordQuery::default()).records.len(), 3);
        let decisions = ask(RecordQuery {
            kind: Some(RecordKind::Decision),
            ..RecordQuery::default()
        });
        assert_eq!((decisions.records.len(), decisions.conflicts.len()), (2, 1));
        assert_eq!(
            ask(RecordQuery {
                task: Some("U".into()),
                ..RecordQuery::default()
            })
            .records
            .len(),
            1
        );
        assert_eq!(
            ask(RecordQuery {
                author: Some("s1".into()),
                ..RecordQuery::default()
            })
            .records
            .len(),
            1
        );
        assert_eq!(
            ask(RecordQuery {
                path: Some("docs/MESH.md".into()),
                ..RecordQuery::default()
            })
            .records
            .len(),
            3
        );
        assert_eq!(
            ask(RecordQuery {
                live: Some(false),
                ..RecordQuery::default()
            })
            .records
            .len(),
            0
        );
    }
}
