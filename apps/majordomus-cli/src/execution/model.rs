//! What an execution is: its identity, the states it may be in, and the snapshot every
//! transport answers with. Plain data with no transport type in it and no handle to a
//! running thread: a snapshot is taken under the store's lock and read afterwards.
//!
//! The vocabulary is one value's worth: an identity a client can pass back, a state that
//! moves only where [`ExecutionState::may_move_to`] allows, and the views a handler
//! reports through. Nothing here runs anything, so all of it can be read and asserted on
//! its own.
//!
//! ```
//! use majordomus_cli::execution::model::*;
//! // an identity a client may hand back, and one it may not
//! let id = ExecutionId::fresh();
//! assert_eq!(ExecutionId::parse(id.as_str()).as_ref(), Some(&id));
//! assert!(ExecutionId::parse("../../etc/passwd").is_none());
//!
//! // the lifecycle, as the store will enforce it
//! let mut state = ExecutionState::Queued;
//! for next in [ExecutionState::Running, ExecutionState::Succeeded] {
//!     assert!(state.may_move_to(next), "{state:?} -> {next:?}");
//!     state = next;
//! }
//! assert!(state.is_final());
//! assert!(!state.may_move_to(ExecutionState::Running), "a final state is final");
//!
//! // and what a handler reported on the way
//! let progress = ProgressView { current: 3, total: Some(4), message: None };
//! assert_eq!(progress.percent(), Some(75));
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The identity of one execution: `x-<UTC timestamp>-<8 hex>`.
///
/// The shape is the layer's own — a task is `t-20260905034523-a9f1` — so that an id read
/// in a log, a URL or a browser tab is recognisable as this repository's without a
/// dependency on a UUID crate. The timestamp orders ids by creation for a reader; the
/// suffix, not the timestamp, is what makes two ids created in the same second differ.
///
/// It is a newtype and not a `String` because it arrives from a client and is then used to
/// look something up: [`ExecutionId::parse`] is the only way in from text, and it is what
/// keeps a path, a traversal or a wildcard out of a store lookup. It is not a credential —
/// the suffix is not unguessable, and nothing is authorised by holding one.
///
/// ```
/// use majordomus_cli::execution::ExecutionId;
/// let id = ExecutionId::fresh();
/// // the text form round-trips, and it is what a log line and a URL carry
/// assert_eq!(id.to_string(), id.as_str());
/// assert_eq!(ExecutionId::parse(id.as_str()).as_ref(), Some(&id));
/// // and it is serialised as that text and nothing around it
/// assert_eq!(serde_json::to_value(&id).unwrap(), serde_json::json!(id.as_str()));
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ExecutionId(String);

impl ExecutionId {
    /// A fresh id, stamped with the current UTC second.
    ///
    /// Unique within a process by construction — a counter is mixed into the suffix — and
    /// unlikely to collide across two processes over one repository, which is as much as
    /// an id that is not a credential needs. The timestamp is for a reader's benefit and
    /// is not what makes it unique.
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionId;
    /// let a = ExecutionId::fresh();
    /// let b = ExecutionId::fresh();
    /// assert_ne!(a, b, "two ids from the same second still differ");
    /// assert!(ExecutionId::parse(a.as_str()).is_some());
    /// ```
    pub fn fresh() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let stamp = compact_utc(now.as_secs());
        // the suffix is not a secret and does not need to be unguessable: the server
        // refuses a state-changing request from another origin, and an id is not a
        // credential. It needs to be unique within a process, which a counter guarantees,
        // and unlikely to repeat across two processes of one repository, which the
        // sub-second part gives it.
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mix = (now.subsec_nanos() as u64) ^ (seq << 20) ^ (seq.wrapping_mul(0x9e37_79b9));
        ExecutionId(format!("x-{stamp}-{:08x}", mix & 0xffff_ffff))
    }

    /// Parse an id a client sent. `None` when it is not one this executable would mint,
    /// which is what keeps a path, a traversal or a wildcard out of a store lookup.
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionId;
    /// assert!(ExecutionId::parse("x-20260908T010203Z-0a1b2c3d").is_some());
    /// assert!(ExecutionId::parse("../../etc/passwd").is_none());
    /// assert!(ExecutionId::parse("x-20260908T010203Z-0a1b2c3").is_none(), "eight hex digits");
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let rest = text.strip_prefix("x-")?;
        let (stamp, suffix) = rest.split_once('-')?;
        let stamp_ok = stamp.len() == 16
            && stamp.ends_with('Z')
            && stamp.as_bytes()[8] == b'T'
            && stamp[..8].bytes().all(|b| b.is_ascii_digit())
            && stamp[9..15].bytes().all(|b| b.is_ascii_digit());
        let suffix_ok = suffix.len() == 8 && suffix.bytes().all(|b| b.is_ascii_hexdigit());
        (stamp_ok && suffix_ok).then(|| ExecutionId(text.to_string()))
    }

    /// The id as text, which is the only form anything outside this layer holds.
    ///
    /// A log line, a URL, a browser tab and a correlation id all carry this. It is
    /// identical to what `Display` writes and to how the id serialises, so those three
    /// readings cannot drift apart.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `YYYYMMDDTHHMMSSZ` in UTC from a Unix second, the compact form of the layer's own
/// timestamps.
///
/// ```
/// use majordomus_cli::execution::model::compact_utc;
/// assert_eq!(compact_utc(0), "19700101T000000Z");
/// assert_eq!(compact_utc(1_788_000_000), "20260829T104000Z");
/// ```
pub fn compact_utc(secs: u64) -> String {
    let iso = crate::peers::rfc3339(UNIX_EPOCH + std::time::Duration::from_secs(secs));
    iso.chars().filter(|c| !matches!(c, '-' | ':')).collect()
}

/// Where an execution is in its life.
///
/// The transitions are the whole contract, and [`ExecutionState::may_move_to`] is the one
/// place they are written down: the engine asks before every change, the store refuses a
/// move it did not allow, and a client that reads a final state never sees it move again.
///
/// ```text
///   queued ──► running ──┬──► succeeded
///      │                 ├──► failed
///      │                 └──► cancelling ──┬──► cancelled
///      │                                   ├──► succeeded
///      └──► cancelled                      └──► failed
/// ```
///
/// There is no `starting`. An in-process engine claims an execution and enters its handler
/// in the same instant, so a state between the two would be one no client could ever
/// observe and every client would have to handle.
///
/// ```
/// use majordomus_cli::execution::ExecutionState::*;
/// // the ordinary path, and the one a client draws a progress bar for
/// assert!(Queued.may_move_to(Running) && Running.may_move_to(Succeeded));
/// // cancellation is two moves and not one: a running handler is asked, and then it stops
/// assert!(!Running.may_move_to(Cancelled));
/// assert!(Running.may_move_to(Cancelling) && Cancelling.may_move_to(Cancelled));
/// // a queued execution has no handler to ask, so it goes straight to cancelled
/// assert!(Queued.may_move_to(Cancelled));
/// // a handler that finished while being cancelled still succeeded
/// assert!(Cancelling.may_move_to(Succeeded));
/// // and nothing leaves a final state, in either direction
/// for state in [Succeeded, Failed, Cancelled] {
///     assert!(state.is_final() && !state.is_active());
///     for next in [Queued, Running, Cancelling, Succeeded, Failed, Cancelled] {
///         assert!(!state.may_move_to(next), "{state:?} moved to {next:?}");
///     }
/// }
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    /// Accepted, and no worker has picked it up.
    Queued,
    /// The handler is running.
    Running,
    /// Cancellation was asked for and the handler has not stopped yet.
    Cancelling,
    /// The handler returned an output.
    Succeeded,
    /// The handler returned an error, or the worker could not run it.
    Failed,
    /// It stopped because it was asked to.
    Cancelled,
}

impl ExecutionState {
    /// The word this state is spelled with everywhere it is written down.
    ///
    /// The same word the value serialises to, so a log line, a JSON document, a CLI
    /// listing and a page cannot disagree about what an execution is doing. It is not a
    /// sentence for a reader: a surface that wants one writes it from the state.
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionState;
    /// assert_eq!(ExecutionState::Cancelling.as_str(), "cancelling");
    /// // one spelling, whether it is written as text or as JSON
    /// let json = serde_json::to_value(ExecutionState::Cancelling).unwrap();
    /// assert_eq!(json, serde_json::json!(ExecutionState::Cancelling.as_str()));
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionState::Queued => "queued",
            ExecutionState::Running => "running",
            ExecutionState::Cancelling => "cancelling",
            ExecutionState::Succeeded => "succeeded",
            ExecutionState::Failed => "failed",
            ExecutionState::Cancelled => "cancelled",
        }
    }

    /// Is this the last thing that will be said about the execution?
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionState;
    /// assert!(ExecutionState::Succeeded.is_final());
    /// assert!(!ExecutionState::Cancelling.is_final());
    /// ```
    pub fn is_final(self) -> bool {
        matches!(
            self,
            ExecutionState::Succeeded | ExecutionState::Failed | ExecutionState::Cancelled
        )
    }

    /// Is the execution doing work, or about to?
    pub fn is_active(self) -> bool {
        !self.is_final()
    }

    /// May the execution move from here to `next`?
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionState::*;
    /// assert!(Queued.may_move_to(Running));
    /// assert!(Running.may_move_to(Cancelling));
    /// assert!(Queued.may_move_to(Cancelled), "a queued execution is cancelled without running");
    /// assert!(!Succeeded.may_move_to(Running), "a final state is final");
    /// assert!(!Running.may_move_to(Queued), "nothing goes back");
    /// ```
    pub fn may_move_to(self, next: ExecutionState) -> bool {
        use ExecutionState::*;
        matches!(
            (self, next),
            (Queued, Running)
                | (Queued, Cancelled)
                | (Queued, Failed)
                | (Running, Succeeded)
                | (Running, Failed)
                | (Running, Cancelling)
                | (Cancelling, Cancelled)
                | (Cancelling, Succeeded)
                | (Cancelling, Failed)
        )
    }
}

/// Where a step of an execution stands.
///
/// Three values, and none of them is "queued": a step exists because a handler entered it,
/// so there is no state for a phase that has not started. `Failed` here is about the phase
/// and not the execution — a handler may report a failed step and still succeed.
///
/// ```
/// use majordomus_cli::execution::StepState;
/// assert_eq!(serde_json::to_value(StepState::Completed).unwrap(), "completed");
/// // a step that failed is not an execution state and cannot be confused for one
/// assert_ne!(
///     serde_json::to_value(StepState::Failed).unwrap(),
///     serde_json::to_value(majordomus_cli::execution::ExecutionState::Cancelled).unwrap(),
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    /// Entered and not finished.
    Running,
    /// Finished, and what it was asked to do happened.
    Completed,
    /// Finished, and it did not.
    Failed,
}

/// One named phase of an execution, as the handler reported it.
///
/// `name` is the stable identity a client matches a completion against and `title` is the
/// line a person reads; the absence of `finished_at` is how a reader knows a phase is
/// still running. Nothing checks that a handler's steps are well formed — this is a report
/// of what a handler said, not a schedule it was held to.
///
/// ```
/// use majordomus_cli::execution::{StepState, StepView};
/// let running = StepView {
///     name: "scan".into(),
///     title: "Scanning the index".into(),
///     state: StepState::Running,
///     started_at: "2026-09-10T12:00:00Z".into(),
///     finished_at: None,
///     detail: None,
/// };
/// // a step still running says nothing about when it ended, rather than saying nothing
/// let json = serde_json::to_value(&running).unwrap();
/// assert_eq!(json["state"], "running");
/// assert!(json.get("finished_at").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StepView {
    /// A stable name, unique within the execution.
    pub name: String,
    /// One line for a reader.
    pub title: String,
    /// Where it stands.
    pub state: StepState,
    /// When it was entered, RFC 3339 in UTC.
    pub started_at: String,
    /// When it finished, when it has.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// What it said when it finished, when it said anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// How far along an execution is, in whatever units its handler counts in.
///
/// The total is optional because a handler often does not know it, and an open-ended count
/// is more honest than a denominator somebody invented: a client shows "3 done" rather
/// than a bar that means nothing. The units are never stated — they are the handler's, and
/// only [`ProgressView::percent`] turns them into something comparable.
///
/// ```
/// use majordomus_cli::execution::ProgressView;
/// let open = ProgressView { current: 3, total: None, message: Some("reading".into()) };
/// assert_eq!(open.percent(), None, "no total, no percentage");
/// // an absent total is absent from the document too, rather than being a zero
/// let json = serde_json::to_value(&open).unwrap();
/// assert!(json.get("total").is_none());
/// assert_eq!(json["current"], 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProgressView {
    /// Units done.
    pub current: u64,
    /// Units in total, when the handler knows how many.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// What is being done, for a reader.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ProgressView {
    /// The fraction done, 0..=100, when a total is known.
    ///
    /// ```
    /// use majordomus_cli::execution::ProgressView;
    /// let p = ProgressView { current: 3, total: Some(4), message: None };
    /// assert_eq!(p.percent(), Some(75));
    /// let open = ProgressView { current: 3, total: None, message: None };
    /// assert_eq!(open.percent(), None);
    /// ```
    pub fn percent(&self) -> Option<u64> {
        match self.total {
            Some(total) if total > 0 => Some((self.current.min(total) * 100) / total),
            _ => None,
        }
    }
}

/// Why an execution ended badly, in the repository's diagnostic vocabulary rather than in
/// Rust's: a code a program branches on, a sentence a person reads, and where to look.
///
/// A panic message and a backtrace are never in here. The engine catches a panicking
/// handler and reports `internal` with the execution's correlation id; the panic itself is
/// on the process's own error stream, where the operator running the server can read it.
///
/// The correlation id is always present and is the execution's own id, so a failure a
/// person is looking at and a line in the server's log can be matched up without either
/// side having invented an identifier.
///
/// ```
/// use majordomus_cli::execution::{ExecutionError, ExecutionId};
/// let id = ExecutionId::fresh();
/// let error = ExecutionError {
///     code: "internal".into(),
///     message: "the handler panicked".into(),
///     suggestion: None,
///     correlation_id: id.to_string(),
/// };
/// let json = serde_json::to_value(&error).unwrap();
/// // a program branches on the code; a person reads the message
/// assert_eq!(json["code"], "internal");
/// assert_eq!(json["correlation_id"], id.as_str());
/// // and nothing is suggested when there is nothing to suggest
/// assert!(json.get("suggestion").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExecutionError {
    /// `invalid_input`, `not_found`, `refused`, `internal`, `cancelled`, `unavailable`.
    pub code: String,
    /// What went wrong, for a person.
    pub message: String,
    /// What to do about it, when there is something.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// The execution this failure belongs to, so a log line and a UI agree.
    pub correlation_id: String,
}

/// A structured finding an execution reported on its way, distinct from its outcome.
///
/// The distinction is the point: an execution may report several of these and still
/// succeed, and it may fail without reporting any. A diagnostic is something a handler
/// noticed; an [`ExecutionError`] is how the call ended.
///
/// ```
/// use majordomus_cli::execution::ExecutionDiagnostic;
/// let finding = ExecutionDiagnostic {
///     severity: majordomus_cli::model::Severity::Warning,
///     code: "stale-artifact".into(),
///     summary: "docs/generated is behind the registry".into(),
///     detail: None,
///     suggestion: Some("run majordomus generate".into()),
/// };
/// let json = serde_json::to_value(&finding).unwrap();
/// assert_eq!(json["code"], "stale-artifact");
/// assert_eq!(json["suggestion"], "run majordomus generate");
/// // the severity is the repository's own vocabulary, not one invented for executions
/// assert_eq!(json["severity"], serde_json::to_value(
///     majordomus_cli::model::Severity::Warning).unwrap());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExecutionDiagnostic {
    /// How bad.
    pub severity: crate::model::Severity,
    /// A stable machine-readable code.
    pub code: String,
    /// One line, for a person.
    pub summary: String,
    /// The rest, when there is more.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// What to do about it, when there is something.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Which stream a log line came from.
///
/// Three, because a reader needs to tell a child process's output from the handler's own
/// commentary: `stdout` and `stderr` belong to something this executable ran, `handler` is
/// this executable talking. A client colours them differently and an audit trail can drop
/// one without dropping the others.
///
/// ```
/// use majordomus_cli::execution::LogStream;
/// assert_eq!(serde_json::to_value(LogStream::Handler).unwrap(), "handler");
/// // the handler's own line is not attributed to a process that never said it
/// assert_ne!(
///     serde_json::to_value(LogStream::Handler).unwrap(),
///     serde_json::to_value(LogStream::Stdout).unwrap(),
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogStream {
    /// The standard output of a child process the handler ran.
    Stdout,
    /// Its standard error.
    Stderr,
    /// The handler itself, saying something it wants a reader to see.
    Handler,
}

/// Who asked for the execution. Not an authorisation decision — this server authenticates
/// nobody — but a fact worth carrying into the audit line and the UI.
///
/// It says which door a request came through and nothing about what it was allowed to do.
/// `Internal` is this executable's own tests and benchmarks, which is worth being able to
/// tell apart from a person: a listing full of executions nobody asked for is otherwise
/// hard to read.
///
/// ```
/// use majordomus_cli::execution::{Actor, ActorKind};
/// let json = serde_json::to_value(Actor::of(ActorKind::Mcp)).unwrap();
/// assert_eq!(json["kind"], "mcp");
/// // four doors, each with its own word
/// let kinds = [ActorKind::Http, ActorKind::Mcp, ActorKind::Cli, ActorKind::Internal];
/// let mut words: Vec<String> = kinds
///     .iter()
///     .map(|k| serde_json::to_value(k).unwrap().to_string())
///     .collect();
/// words.sort();
/// words.dedup();
/// assert_eq!(words.len(), kinds.len());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// An HTTP client, which includes the Cockpit.
    Http,
    /// An MCP client attached to this process.
    Mcp,
    /// The command line of this process.
    Cli,
    /// This executable's own tests and benchmarks.
    Internal,
}

/// Who asked, and which peer they are when this process knows.
///
/// The peer is known only for a call that arrived through an MCP session, and its absence
/// is not a gap to be filled in: an HTTP request and the command line have no peer
/// identity, and inventing one would make an audit line say more than this process knows.
///
/// ```
/// use majordomus_cli::execution::{Actor, ActorKind};
/// let anonymous = Actor::of(ActorKind::Http);
/// assert!(anonymous.peer.is_none());
/// // and it is absent from the document rather than present and null
/// assert!(serde_json::to_value(&anonymous).unwrap().get("peer").is_none());
///
/// let attached = Actor { kind: ActorKind::Mcp, peer: Some("p2".into()) };
/// assert_eq!(serde_json::to_value(&attached).unwrap()["peer"], "p2");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Actor {
    /// Where the request came from.
    pub kind: ActorKind,
    /// The peer id, for a call that arrived through an MCP session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
}

impl Actor {
    /// An actor of a kind, with no peer.
    ///
    /// The right constructor for every door but MCP: a peer identity exists only where a
    /// session has one, so this is the ordinary case rather than a partial value waiting to
    /// be completed.
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, ActorKind};
    /// let cli = Actor::of(ActorKind::Cli);
    /// assert_eq!(cli.kind, ActorKind::Cli);
    /// assert_eq!(cli.peer, None, "the command line is not a peer of this process");
    /// ```
    pub fn of(kind: ActorKind) -> Self {
        Actor { kind, peer: None }
    }
}

/// Which repository, and which checkout of it, an execution ran against.
///
/// An execution is never run against "wherever this process happens to be": the engine
/// stamps the repository the index was read from, and a request that names a different one
/// is refused. The path itself is not here — this value is served to whoever can reach the
/// socket, and where the checkout sits on the host is of no use to them.
///
/// The `id` is the stable identity two processes over one checkout both compute, so a
/// client can tell "the same repository" from "the same name", which matters on a machine
/// carrying thirty worktrees of it.
///
/// ```
/// use majordomus_cli::execution::RepositoryRef;
/// let reference = RepositoryRef {
///     name: "prismatic-majordomus".into(),
///     id: "5f3a".into(),
///     branch: Some("master".into()),
/// };
/// let json = serde_json::to_value(&reference).unwrap();
/// assert_eq!(json["name"], "prismatic-majordomus");
/// // no path is served: where the checkout sits is of no use to a client
/// assert!(json.get("path").is_none());
/// assert!(json.get("root").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryRef {
    /// The repository's name: the last component of its root.
    pub name: String,
    /// The stable identity two processes over one checkout both compute.
    pub id: String,
    /// The branch checked out, when git can say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// One execution, as every transport answers it.
///
/// This is the whole of what a client may know. It is produced under the store's lock, so
/// a reader never sees a state and an output that disagree: an execution that says
/// `succeeded` carries its output in the same snapshot, and one that says `failed` carries
/// its error.
///
/// The fields that are absent carry as much meaning as the ones that are present: no
/// `started_at` means nothing has picked it up, no `output` means it has not succeeded, and
/// `events_truncated` says the retained history has lost its beginning — so a client can
/// tell "not yet" from "not measured" without asking anything else.
///
/// ```
/// use majordomus_cli::execution::*;
/// let store = ExecutionStore::new(Limits::default());
/// let id = ExecutionId::fresh();
/// store.create(id.clone(), "health.report", "Health", serde_json::json!({}), true,
///     Actor::of(ActorKind::Cli),
///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
///
/// let accepted: Execution = store.get(&id).unwrap();
/// assert_eq!(accepted.state, ExecutionState::Queued);
/// assert!(accepted.started_at.is_none() && accepted.output.is_none());
/// assert_eq!(accepted.correlation_id, id.as_str(), "a log line and a UI agree");
/// assert!(!accepted.events_truncated, "nothing has been dropped yet");
///
/// store.publish(&id, EventPayload::Started);
/// store.publish(&id, EventPayload::Completed { output: serde_json::json!({ "score": 91 }) });
/// let finished = store.get(&id).unwrap();
/// // the state and the value it produced are one fact and arrive together
/// assert_eq!(finished.state, ExecutionState::Succeeded);
/// assert!(finished.output.is_some() && finished.error.is_none());
/// assert!(finished.finished_at.is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Execution {
    /// The identity.
    pub id: ExecutionId,
    /// The capability that ran, by its canonical id.
    pub capability: String,
    /// That capability's title, so a list needs no second lookup.
    pub title: String,
    /// Where it is.
    pub state: ExecutionState,
    /// When it was accepted, RFC 3339 in UTC.
    pub created_at: String,
    /// When the handler was entered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// When it reached a final state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// How long it ran, in milliseconds, once it has finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// The input it was given, with every value the input schema marks sensitive replaced.
    pub input: Value,
    /// What the handler returned, for a succeeded execution and never before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    /// Why it failed, for a failed execution and never before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
    /// The last progress the handler reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<ProgressView>,
    /// The steps it entered, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<StepView>,
    /// The findings it reported.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ExecutionDiagnostic>,
    /// The sequence number of the last event: the cursor a client subscribes from.
    pub last_sequence: u64,
    /// How many events it has produced, including the ones no longer retained.
    pub event_count: u64,
    /// Whether the retained event history has lost its oldest entries to the bound.
    pub events_truncated: bool,
    /// Whether asking to cancel this execution will do anything: the capability's policy.
    pub cancellable: bool,
    /// Who asked for it.
    pub actor: Actor,
    /// Which repository it ran against.
    pub repository: RepositoryRef,
    /// The id used in this process's logs and tracing spans for this execution; the
    /// execution id itself, carried under the name a reader of a log expects.
    pub correlation_id: String,
}

impl Execution {
    /// The percentage a progress bar shows, when the handler said enough to compute one.
    ///
    /// `None` for an execution that has reported nothing and for one whose handler does not
    /// know its own total, which is why a client shows a count or a spinner rather than
    /// filling in a zero.
    ///
    /// ```
    /// use majordomus_cli::execution::*;
    /// let store = ExecutionStore::new(Limits::default());
    /// let id = ExecutionId::fresh();
    /// store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    ///     Actor::of(ActorKind::Internal),
    ///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// assert_eq!(store.get(&id).unwrap().percent(), None, "it has reported nothing");
    ///
    /// store.publish(&id, EventPayload::Progress(ProgressView {
    ///     current: 1, total: Some(4), message: None,
    /// }));
    /// assert_eq!(store.get(&id).unwrap().percent(), Some(25));
    /// ```
    pub fn percent(&self) -> Option<u64> {
        self.progress.as_ref().and_then(ProgressView::percent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_round_trips_and_a_path_does_not() {
        let id = ExecutionId::fresh();
        assert_eq!(ExecutionId::parse(id.as_str()).as_ref(), Some(&id));
        assert_eq!(id.to_string(), id.as_str());
        for bad in [
            "",
            "x-",
            "x--0a1b2c3d",
            "../x-20260908T010203Z-0a1b2c3d",
            "x-20260908T010203Z-0a1b2c3d/..",
            "y-20260908T010203Z-0a1b2c3d",
            "x-2026090801020Z-0a1b2c3d",
            "x-20260908T010203Z-0a1b2c3g",
        ] {
            assert!(ExecutionId::parse(bad).is_none(), "{bad:?} accepted");
        }
    }

    #[test]
    fn ids_do_not_repeat_within_a_process() {
        let ids: std::collections::BTreeSet<_> = (0..2000).map(|_| ExecutionId::fresh()).collect();
        assert_eq!(ids.len(), 2000);
    }

    #[test]
    fn every_state_is_reachable_and_no_final_state_moves() {
        use ExecutionState::*;
        let all = [Queued, Running, Cancelling, Succeeded, Failed, Cancelled];
        for state in all {
            if state.is_final() {
                assert!(
                    all.iter().all(|next| !state.may_move_to(*next)),
                    "{state:?} moved"
                );
            } else {
                assert!(
                    all.iter().any(|next| state.may_move_to(*next)),
                    "{state:?} is a dead end"
                );
            }
            // every state is reachable from queued by some path
            assert_eq!(state.is_active(), !state.is_final());
        }
        // the words are the wire format and are asserted rather than assumed
        assert_eq!(
            all.map(|s| s.as_str()),
            [
                "queued",
                "running",
                "cancelling",
                "succeeded",
                "failed",
                "cancelled"
            ]
        );
        for state in all {
            let text = serde_json::to_string(&state).unwrap();
            assert_eq!(text, format!("\"{}\"", state.as_str()));
        }
    }

    #[test]
    fn a_compact_timestamp_is_the_layers_own_shape() {
        assert_eq!(compact_utc(0), "19700101T000000Z");
        assert_eq!(compact_utc(1_788_000_000).len(), 16);
    }
}
