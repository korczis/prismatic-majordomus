//! What an execution is: its identity, the states it may be in, and the snapshot every
//! transport answers with. Plain data with no transport type in it and no handle to a
//! running thread: a snapshot is taken under the store's lock and read afterwards.

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
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ExecutionId(String);

impl ExecutionId {
    /// A fresh id for now.
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

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The longest idempotency key a start accepts, in characters.
pub const MAX_IDEMPOTENCY_KEY: usize = 128;

/// The key a client sends with a start so that sending it again — a double click, a retry
/// after a timeout whose answer never arrived — answers the execution the first send
/// started instead of starting a second one (I4000).
///
/// One to [`MAX_IDEMPOTENCY_KEY`] characters of `A-Z a-z 0-9 . _ : -`: enough for a UUID,
/// a ULID or `issue:I4000:implement`, and nothing a log line, a URL or a shell has to
/// escape. A key names a request, not a secret, and is shown in the snapshot it started.
///
/// ```
/// use majordomus_cli::execution::IdempotencyKey;
/// assert_eq!(IdempotencyKey::parse("issue:I4000:implement").unwrap().as_str(), "issue:I4000:implement");
/// assert!(IdempotencyKey::parse("").is_err(), "empty");
/// assert!(IdempotencyKey::parse("two words").is_err(), "a space");
/// assert!(IdempotencyKey::parse(&"k".repeat(129)).is_err(), "too long");
/// assert!(IdempotencyKey::parse(&"k".repeat(128)).is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Parse a key a client sent, or say why it is not one. Nothing is trimmed or folded:
    /// a key is compared byte for byte, so `Click-1` and `click-1` are two requests.
    ///
    /// ```
    /// use majordomus_cli::execution::IdempotencyKey;
    /// assert_ne!(IdempotencyKey::parse("Click-1"), IdempotencyKey::parse("click-1"));
    /// let refused = IdempotencyKey::parse("a/b").unwrap_err();
    /// assert!(refused.contains("'/'"), "the refusal names the character: {refused}");
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let length = text.chars().count();
        if length == 0 || length > MAX_IDEMPOTENCY_KEY {
            return Err(format!(
                "an idempotency key is 1 to {MAX_IDEMPOTENCY_KEY} characters; this one has {length}"
            ));
        }
        if let Some(bad) = text
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-')))
        {
            return Err(format!(
                "an idempotency key is made of A-Z a-z 0-9 . _ : - only, and {bad:?} is not"
            ));
        }
        Ok(IdempotencyKey(text.to_string()))
    }

    /// The key exactly as the client sent it, for the snapshot and for a refusal that
    /// quotes it back.
    ///
    /// ```
    /// use majordomus_cli::execution::IdempotencyKey;
    /// assert_eq!(IdempotencyKey::parse("retry.7").unwrap().as_str(), "retry.7");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IdempotencyKey {
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
/// The transitions are the whole contract, and [`ExecutionState::EDGES`] is the one place
/// they are written down: [`ExecutionState::may_move_to`] reads it, the engine asks before
/// every change, the store refuses a move it does not allow, and a client that reads a
/// final state never sees it move again.
///
/// ```text
///   authorizing     ──► queued | refused | failed | cancelled
///   queued          ──► running | claimed | refused | failed | cancelled
///   claimed         ──► running | queued | refused | failed | cancelling | unknown_outcome
///   running         ──► verifying | succeeded | failed | cancelling | unknown_outcome
///   verifying       ──► succeeded | failed | cancelling | unknown_outcome
///   cancelling      ──► cancelled | succeeded | failed | unknown_outcome
///   unknown_outcome ──► verifying | succeeded | failed | cancelled        (never running)
///   succeeded, failed, cancelled, refused: final
/// ```
///
/// The in-process engine moves `queued ──► running` directly: it claims an execution and
/// enters its handler in the same instant, so `claimed` would be a state no client could
/// observe. `authorizing`, `claimed`, `verifying`, `refused` and `unknown_outcome` are
/// entered by a worker that decides, holds, checks or loses an execution in its own time
/// (Mission Control B3): a policy decision before the queue, a lease on a queued ticket, a
/// check of what the handler produced, a refusal, and a run whose outcome cannot be read.
///
/// `unknown_outcome` is the one state a retry may not leave by running again. Whether the
/// work happened is not known, so the next move is a reconciliation that reads evidence
/// — to `verifying`, or straight to the outcome it found — and never `running`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    /// Accepted, and a policy decision on whether it may run has not been taken yet.
    Authorizing,
    /// Accepted, and no worker has picked it up.
    Queued,
    /// A worker holds it and has not entered the handler yet.
    Claimed,
    /// The handler is running.
    Running,
    /// The handler returned, and what it produced is being checked.
    Verifying,
    /// Cancellation was asked for and the handler has not stopped yet.
    Cancelling,
    /// Whether the work happened is not known: the worker that ran it went silent or its
    /// answer could not be read. Only a reconciliation moves it on, never a rerun.
    UnknownOutcome,
    /// The handler returned an output.
    Succeeded,
    /// The handler returned an error, or the worker could not run it.
    Failed,
    /// It stopped because it was asked to.
    Cancelled,
    /// A policy refused it before it ran, at the request or when a worker claimed it.
    Refused,
}

impl ExecutionState {
    /// Every state, in the order of an execution's life.
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionState;
    /// assert_eq!(ExecutionState::ALL.len(), 11);
    /// assert_eq!(ExecutionState::ALL[0], ExecutionState::Authorizing);
    /// ```
    pub const ALL: [ExecutionState; 11] = [
        ExecutionState::Authorizing,
        ExecutionState::Queued,
        ExecutionState::Claimed,
        ExecutionState::Running,
        ExecutionState::Verifying,
        ExecutionState::Cancelling,
        ExecutionState::UnknownOutcome,
        ExecutionState::Succeeded,
        ExecutionState::Failed,
        ExecutionState::Cancelled,
        ExecutionState::Refused,
    ];

    /// The transition table: every move an execution may make, and no other. A pair that
    /// is not here is refused.
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionState::{self, *};
    /// assert!(ExecutionState::EDGES.contains(&(Queued, Running)));
    /// assert!(!ExecutionState::EDGES.contains(&(UnknownOutcome, Running)));
    /// ```
    pub const EDGES: &'static [(ExecutionState, ExecutionState)] = {
        use ExecutionState::*;
        &[
            // a policy decision before the queue
            (Authorizing, Queued),
            (Authorizing, Refused),
            (Authorizing, Failed),
            (Authorizing, Cancelled),
            // the queue: the in-process engine enters the handler at once; a worker claims
            (Queued, Running),
            (Queued, Claimed),
            (Queued, Refused),
            (Queued, Failed),
            (Queued, Cancelled),
            // a claim: entered, released before anything ran, refused when the worker
            // authorises the ticket it consumes, or lost with its holder
            (Claimed, Running),
            (Claimed, Queued),
            (Claimed, Refused),
            (Claimed, Failed),
            (Claimed, Cancelling),
            (Claimed, UnknownOutcome),
            // the handler
            (Running, Verifying),
            (Running, Succeeded),
            (Running, Failed),
            (Running, Cancelling),
            (Running, UnknownOutcome),
            // the check of what it produced
            (Verifying, Succeeded),
            (Verifying, Failed),
            (Verifying, Cancelling),
            (Verifying, UnknownOutcome),
            // a stop asked for
            (Cancelling, Cancelled),
            (Cancelling, Succeeded),
            (Cancelling, Failed),
            (Cancelling, UnknownOutcome),
            // a reconciliation, which reads evidence and never runs the work again
            (UnknownOutcome, Verifying),
            (UnknownOutcome, Succeeded),
            (UnknownOutcome, Failed),
            (UnknownOutcome, Cancelled),
        ]
    };

    /// The word as serialised.
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionState::Authorizing => "authorizing",
            ExecutionState::Queued => "queued",
            ExecutionState::Claimed => "claimed",
            ExecutionState::Running => "running",
            ExecutionState::Verifying => "verifying",
            ExecutionState::Cancelling => "cancelling",
            ExecutionState::UnknownOutcome => "unknown_outcome",
            ExecutionState::Succeeded => "succeeded",
            ExecutionState::Failed => "failed",
            ExecutionState::Cancelled => "cancelled",
            ExecutionState::Refused => "refused",
        }
    }

    /// Is this the last thing that will be said about the execution?
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionState;
    /// assert!(ExecutionState::Succeeded.is_final());
    /// assert!(ExecutionState::Refused.is_final());
    /// assert!(!ExecutionState::Cancelling.is_final());
    /// assert!(!ExecutionState::UnknownOutcome.is_final(), "it waits for a reconciliation");
    /// ```
    pub fn is_final(self) -> bool {
        matches!(
            self,
            ExecutionState::Succeeded
                | ExecutionState::Failed
                | ExecutionState::Cancelled
                | ExecutionState::Refused
        )
    }

    /// Is the execution not final yet: doing work, about to, or waiting to be reconciled?
    pub fn is_active(self) -> bool {
        !self.is_final()
    }

    /// May the execution move from here to `next`? Exactly the pairs of
    /// [`ExecutionState::EDGES`].
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionState::*;
    /// assert!(Queued.may_move_to(Running));
    /// assert!(Running.may_move_to(Cancelling));
    /// assert!(Queued.may_move_to(Cancelled), "a queued execution is cancelled without running");
    /// assert!(!Succeeded.may_move_to(Running), "a final state is final");
    /// assert!(!Running.may_move_to(Queued), "nothing that ran goes back");
    /// assert!(Authorizing.may_move_to(Refused));
    /// assert!(!UnknownOutcome.may_move_to(Running), "reconciled first, never rerun");
    /// ```
    pub fn may_move_to(self, next: ExecutionState) -> bool {
        Self::EDGES.contains(&(self, next))
    }

    /// The states this one may move to, in the order of [`ExecutionState::ALL`].
    ///
    /// ```
    /// use majordomus_cli::execution::ExecutionState::*;
    /// assert_eq!(UnknownOutcome.successors(), vec![Verifying, Succeeded, Failed, Cancelled]);
    /// assert!(Refused.successors().is_empty());
    /// ```
    pub fn successors(self) -> Vec<ExecutionState> {
        Self::ALL
            .into_iter()
            .filter(|next| self.may_move_to(*next))
            .collect()
    }
}

/// Where a step of an execution stands.
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

/// How far along an execution is.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExecutionError {
    /// `invalid_input`, `not_found`, `refused`, `forbidden`, `conflict`, `internal`, `cancelled`,
    /// `unavailable`.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// An HTTP client, which includes the Cockpit.
    Http,
    /// An MCP client attached to this process.
    Mcp,
    /// A command line: this process's own, or `majordomus run` in another process of the
    /// same checkout that reported its execution to this server, with its pid.
    Cli,
    /// This executable's own tests and benchmarks.
    Internal,
}

/// Who asked, and which peer they are when this process knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Actor {
    /// Where the request came from.
    pub kind: ActorKind,
    /// The peer id, for a call that arrived through an MCP session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    /// The process that ran it, for an execution another process ran and reported to this
    /// one: the work happened there, and this process holds what it was told about it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

impl Actor {
    /// An actor of a kind, with no peer.
    pub fn of(kind: ActorKind) -> Self {
        Actor {
            kind,
            peer: None,
            pid: None,
        }
    }

    /// `majordomus run` in another process, which ran the execution and reported it here.
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, ActorKind};
    /// let a = Actor::reported(4242);
    /// assert_eq!((a.kind, a.pid), (ActorKind::Cli, Some(4242)));
    /// ```
    pub fn reported(pid: u32) -> Self {
        Actor {
            kind: ActorKind::Cli,
            peer: None,
            pid: Some(pid),
        }
    }
}

/// Which repository, and which checkout of it, an execution ran against.
///
/// An execution is never run against "wherever this process happens to be": the engine
/// stamps the repository the index was read from, and a request that names a different one
/// is refused. The path itself is not here — this value is served to whoever can reach the
/// socket, and where the checkout sits on the host is of no use to them.
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
    /// Why it failed, for a failed execution and never before; why it was refused, for a
    /// refused one; and why its outcome is not known, for one whose outcome is not.
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
    /// The idempotency key the start was sent with, when it was sent with one: a repeated
    /// start with this key answers this execution instead of starting another (I4000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl Execution {
    /// The percentage a progress bar shows, when the handler said enough to compute one.
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

    /// The table, written out pair by pair as the contract states it: a change to
    /// [`ExecutionState::EDGES`] that is not also made here fails, in both directions.
    const DECLARED: &[(&str, &[&str])] = &[
        ("authorizing", &["queued", "failed", "cancelled", "refused"]),
        (
            "queued",
            &["claimed", "running", "failed", "cancelled", "refused"],
        ),
        (
            "claimed",
            &[
                "queued",
                "running",
                "cancelling",
                "unknown_outcome",
                "failed",
                "refused",
            ],
        ),
        (
            "running",
            &[
                "verifying",
                "cancelling",
                "unknown_outcome",
                "succeeded",
                "failed",
            ],
        ),
        (
            "verifying",
            &["cancelling", "unknown_outcome", "succeeded", "failed"],
        ),
        (
            "cancelling",
            &["unknown_outcome", "succeeded", "failed", "cancelled"],
        ),
        (
            "unknown_outcome",
            &["verifying", "succeeded", "failed", "cancelled"],
        ),
        ("succeeded", &[]),
        ("failed", &[]),
        ("cancelled", &[]),
        ("refused", &[]),
    ];

    #[test]
    fn may_move_to_admits_exactly_the_declared_edges_and_refuses_every_other_pair() {
        use ExecutionState::*;
        assert_eq!(
            DECLARED.len(),
            ExecutionState::ALL.len(),
            "every state is declared"
        );
        let mut admitted = 0;
        for from in ExecutionState::ALL {
            let (_, allowed) = DECLARED
                .iter()
                .find(|(name, _)| *name == from.as_str())
                .unwrap_or_else(|| panic!("{from:?} is not declared"));
            for to in ExecutionState::ALL {
                let declared = allowed.contains(&to.as_str());
                assert_eq!(
                    from.may_move_to(to),
                    declared,
                    "{} -> {}: the table says {}, the contract says {}",
                    from.as_str(),
                    to.as_str(),
                    from.may_move_to(to),
                    declared
                );
                admitted += usize::from(declared);
            }
        }
        assert_eq!(
            admitted,
            ExecutionState::EDGES.len(),
            "no edge is listed twice"
        );
        // the contract's own sentences, as assertions of their own
        for to in ExecutionState::ALL {
            assert!(!to.may_move_to(to), "{to:?} -> itself is not a move");
            assert!(
                !to.may_move_to(Authorizing),
                "nothing enters authorizing: it is a start"
            );
        }
        for from in [Running, Verifying, Cancelling, UnknownOutcome] {
            assert!(
                !from.may_move_to(Queued),
                "{from:?}: what may have run is not requeued"
            );
            assert!(!from.may_move_to(Claimed), "{from:?}: nor claimed again");
        }
        assert!(
            !UnknownOutcome.may_move_to(Running),
            "reconcile first, never rerun"
        );
        for state in ExecutionState::ALL {
            assert_eq!(
                state.successors(),
                ExecutionState::ALL
                    .into_iter()
                    .filter(|n| state.may_move_to(*n))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn every_state_is_reachable_and_no_final_state_moves() {
        use ExecutionState::*;
        let all = ExecutionState::ALL;
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
            assert_eq!(state.is_active(), !state.is_final());
        }
        // every state is reachable from a start: authorizing, or queued for a start that
        // needs no decision
        let mut reached = std::collections::BTreeSet::from([Authorizing, Queued]);
        loop {
            let before = reached.len();
            let next: Vec<_> = reached.iter().flat_map(|s| s.successors()).collect();
            reached.extend(next);
            if reached.len() == before {
                break;
            }
        }
        assert_eq!(reached.len(), all.len(), "unreachable: {:?}", {
            all.iter()
                .filter(|s| !reached.contains(s))
                .collect::<Vec<_>>()
        });
        // the words are the wire format and are asserted rather than assumed
        assert_eq!(
            all.map(|s| s.as_str()),
            [
                "authorizing",
                "queued",
                "claimed",
                "running",
                "verifying",
                "cancelling",
                "unknown_outcome",
                "succeeded",
                "failed",
                "cancelled",
                "refused"
            ]
        );
        for state in all {
            let text = serde_json::to_string(&state).unwrap();
            assert_eq!(text, format!("\"{}\"", state.as_str()));
        }
    }

    #[test]
    fn the_flows_that_existed_before_the_new_states_are_unchanged() {
        use ExecutionState::*;
        // the six states and nine moves of the engine as it was, every one still allowed
        for (from, to) in [
            (Queued, Running),
            (Queued, Cancelled),
            (Queued, Failed),
            (Running, Succeeded),
            (Running, Failed),
            (Running, Cancelling),
            (Cancelling, Cancelled),
            (Cancelling, Succeeded),
            (Cancelling, Failed),
        ] {
            assert!(
                from.may_move_to(to),
                "{from:?} -> {to:?} was allowed and still is"
            );
        }
        // and every move among those six that was refused still is
        let old = [Queued, Running, Cancelling, Succeeded, Failed, Cancelled];
        for from in old {
            for to in old {
                let was = matches!(
                    (from, to),
                    (Queued, Running)
                        | (Queued, Cancelled)
                        | (Queued, Failed)
                        | (Running, Succeeded)
                        | (Running, Failed)
                        | (Running, Cancelling)
                        | (Cancelling, Cancelled)
                        | (Cancelling, Succeeded)
                        | (Cancelling, Failed)
                );
                assert_eq!(from.may_move_to(to), was, "{from:?} -> {to:?}");
            }
            assert_eq!(
                from.is_final(),
                matches!(from, Succeeded | Failed | Cancelled),
                "{from:?}"
            );
        }
    }

    #[test]
    fn a_compact_timestamp_is_the_layers_own_shape() {
        assert_eq!(compact_utc(0), "19700101T000000Z");
        assert_eq!(compact_utc(1_788_000_000).len(), 16);
    }
}
