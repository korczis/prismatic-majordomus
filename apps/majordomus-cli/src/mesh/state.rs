//! The cooperation state: what the journal's events mean, folded. Pure: the same set of
//! events and the same liveness verdicts yield the same state on every runtime, whatever
//! order the events arrived in and however often — events are applied in `(lamport,
//! stream, seq)` order, and a set has no duplicates. That is the whole convergence
//! argument, and a property test holds it.
//!
//! Exclusivity is decided here, and decided the same way everywhere:
//!
//! - A claim **holds** while it is not released, its session is not closed, and its
//!   stream is alive (its beat rose within the expiry on this runtime's clock). A dead
//!   runtime's claims therefore end on their own; nobody releases them for it.
//! - Two live **exclusive** claims of different sessions whose scopes meet are a
//!   **conflict**. The claim with the lower `(lamport, stream, seq)` of its acquisition
//!   wins; the other is `conflicted` and names the winner. Admission refuses such a claim
//!   up front, so a conflict can only arise from genuinely concurrent claims — two sides
//!   of a partition, or two runtimes in the same round — and then every runtime names the
//!   same winner. Never "last packet wins".
//! - Scopes meet by [`crate::peers::claims_meet`], the peer board's own predicate: one
//!   semantics for a claim on this machine and on another.
//!
//! ```
//! use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, StreamLiveness};
//! use majordomus_cli::mesh::identity::NodeIdentity;
//! use majordomus_cli::mesh::state::{fold, ClaimState};
//! use std::sync::Arc;
//!
//! let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
//!     "repo".into(), None).unwrap();
//! j.append_own(EventBody::ClaimAcquired { claim: "c1".into(), session: "s1".into(),
//!     scope: vec!["apps".into()], intent: None, mode: ClaimMode::Exclusive, issue: None }).unwrap();
//! let state = fold(&j.events(), &|_| StreamLiveness::Own);
//! assert_eq!(state.claims[0].state, ClaimState::Held);
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::journal::{
    canonical_json, causal_key, ClaimMode, EventBody, HandoverBody, MeshEvent, SessionInfo,
    StreamId, StreamLiveness,
};

/// A session's standing, as every runtime of the mesh reads it. The distinction that
/// matters is between `Closed` and `Expired`: the first is a session that said it was
/// finished, the second one whose runtime stopped beating and can no longer say anything.
/// Both end the session's claims, but only the second is a machine nobody should expect an
/// answer from.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, SessionState};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "cli") }).unwrap();
/// assert_eq!(fold(&j.events(), &|_| StreamLiveness::Own).sessions[0].state, SessionState::Active);
///
/// // the same events, read on a runtime that no longer hears the writer
/// let gone = fold(&j.events(), &|_| StreamLiveness::Expired);
/// assert_eq!(gone.sessions[0].state, SessionState::Expired);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Open, and its runtime is alive.
    Active,
    /// Closed by its own runtime.
    Closed,
    /// Never closed, but its runtime stopped beating: gone, as far as anyone can tell.
    Expired,
}

/// One session anywhere in the mesh, as the fold sees it: what the session said about
/// itself, where it is writing from, and whether it still counts. The key is
/// `<stream>/<session>` rather than the session's own id, because two runtimes may both
/// call a session `s1` and the mesh has to hold them apart.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, SessionView};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "claude-code") })
///     .unwrap();
///
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// let view: &SessionView = &state.sessions[0];
/// assert!(view.key.ends_with("/s1"), "qualified by the stream that wrote it: {}", view.key);
/// assert_eq!(view.info.client, "claude-code");
/// assert_eq!(view.runtime, view.stream.runtime_key());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionView {
    /// `<stream>/<session>`: unique across the mesh.
    pub key: String,
    /// The writing stream.
    pub stream: StreamId,
    /// `<node>-<runtime>`.
    pub runtime: String,
    /// The node (machine).
    pub node: String,
    /// What the session last said about itself.
    pub info: SessionInfo,
    /// Its standing.
    pub state: SessionState,
    /// The Lamport stamp of its latest word.
    pub updated_lamport: u64,
}

/// A claim's standing, and — where it ended or lost — what ended it. The two variants that
/// carry a string exist because the bare verdict is not actionable: a worker told its claim
/// expired needs to know whether its session closed or its runtime stopped beating, and one
/// told its claim is conflicted needs the key of the claim that beat it, which is the same
/// key on every runtime.
///
/// ```
/// use majordomus_cli::mesh::state::ClaimState;
///
/// assert!(ClaimState::Held.is_live());
/// assert!(ClaimState::Conflicted("s/c1".into()).is_live(), "still held, and not the winner");
/// assert!(!ClaimState::Released.is_live());
///
/// // the detail travels with the verdict, so a reader is never left asking who won
/// let value = serde_json::to_value(ClaimState::Conflicted("s/c1".into())).unwrap();
/// assert_eq!(value["state"], "conflicted");
/// assert_eq!(value["detail"], "s/c1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "state", content = "detail")]
pub enum ClaimState {
    /// Live; for an exclusive claim, the winner of its scope.
    Held,
    /// Released by its holder.
    Released,
    /// Ended without a release: the holder's session closed or its runtime expired.
    Expired(String),
    /// Live, exclusive, and beaten to its scope by the claim whose key this names.
    Conflicted(String),
}

impl ClaimState {
    /// Whether the claim is live (held or conflicted). A conflicted claim counts as live
    /// because it has not gone away: its holder is still working, and the next claim over
    /// that scope has to meet it too. Only a release, a closed session or an expired
    /// runtime ends a claim.
    ///
    /// ```
    /// use majordomus_cli::mesh::state::ClaimState;
    ///
    /// assert!(ClaimState::Held.is_live());
    /// assert!(ClaimState::Conflicted("s/c1".into()).is_live(), "losing is not ending");
    /// assert!(!ClaimState::Released.is_live());
    /// assert!(!ClaimState::Expired("its runtime stopped beating".into()).is_live());
    /// ```
    pub fn is_live(&self) -> bool {
        matches!(self, ClaimState::Held | ClaimState::Conflicted(_))
    }
}

/// One claim anywhere in the mesh: who took what, for what, and where it stands. The
/// acquisition's Lamport stamp and sequence are carried rather than derived on demand,
/// because they are what exclusivity is decided by — every runtime ranks two competing
/// claims by the same triple and so names the same winner, with no message passing and no
/// "last packet wins".
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, ClaimState, ClaimView};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::ClaimAcquired { claim: "c1".into(), session: "s1".into(),
///     scope: vec!["apps/majordomus-cli".into()], intent: Some("document the mesh".into()),
///     mode: ClaimMode::Exclusive, issue: Some("#184".into()) }).unwrap();
///
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// let claim: &ClaimView = &state.claims[0];
/// assert_eq!(claim.scope, ["apps/majordomus-cli"]);
/// assert_eq!(claim.state, ClaimState::Held);
/// assert!(claim.acquired_lamport > 0, "the order exclusivity is decided in");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimView {
    /// `<stream>/<claim>`.
    pub key: String,
    /// The writing stream.
    pub stream: StreamId,
    /// `<node>-<runtime>`.
    pub runtime: String,
    /// The holding session's key.
    pub session: String,
    /// The claimed paths.
    pub scope: Vec<String>,
    /// What it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// Exclusive or advisory.
    pub mode: ClaimMode,
    /// The issue it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The Lamport stamp of the acquisition.
    pub acquired_lamport: u64,
    /// The sequence of the acquisition within its stream: with the stamp and the stream,
    /// the total order in which exclusivity is decided.
    pub acquired_seq: u64,
    /// Its standing.
    pub state: ClaimState,
}

impl ClaimView {
    /// Where the claim stands in the one order exclusivity is decided by: the Lamport stamp
    /// of its acquisition, then its stream, then its sequence. Owned, because a collection
    /// keys a map by it rather than sorting itself with a comparator of its own.
    fn order_key(&self) -> (u64, StreamId, u64) {
        (
            self.acquired_lamport,
            self.stream.clone(),
            self.acquired_seq,
        )
    }
}

/// Two live claims of different sessions whose scopes meet, at least one advisory:
/// reported, never refused.
///
/// An overlap is information, not a verdict — it is how two workers find out they are in
/// the same place without either being stopped. The meeting paths are listed pairwise
/// rather than summarized, because the useful question is which path of mine meets which
/// path of theirs.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, ClaimOverlap};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// for (claim, session, mode) in [
///     ("c1", "s1", ClaimMode::Exclusive),
///     ("c2", "s2", ClaimMode::Advisory),
/// ] {
///     j.append_own(EventBody::ClaimAcquired { claim: claim.into(), session: session.into(),
///         scope: vec!["apps".into()], intent: None, mode, issue: None }).unwrap();
/// }
///
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// let overlap: &ClaimOverlap = &state.overlaps[0];
/// assert_ne!(overlap.first, overlap.second, "two claims, two sessions");
/// assert_eq!(overlap.paths, [("apps".to_string(), "apps".to_string())]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimOverlap {
    /// The earlier claim's key.
    pub first: String,
    /// The later claim's key.
    pub second: String,
    /// The paths that meet, as `first ↔ second` pairs.
    pub paths: Vec<(String, String)>,
}

/// A handover anywhere in the mesh, with everyone who has taken it. The id is the digest
/// of the body, so the same handover published twice — by a retry, or by two runtimes that
/// both saw it — is one handover here; and `consumed_by` is what keeps two workers from
/// silently picking up the same continuity, since the list is folded from events every
/// runtime holds.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, HandoverBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, HandoverView};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// let body = "# Objective\nship\n".to_string();
/// let handover = HandoverBody { id: HandoverBody::digest_of(&body), task: None, issue: None,
///     milestone: None, branch: Some("feature/x".into()), head: None, created_at: None,
///     name: None, body };
/// let id = handover.id.clone();
/// j.append_own(EventBody::HandoverPublished { handover }).unwrap();
/// j.append_own(EventBody::HandoverConsumed { handover: id.clone(), session: "s1".into() })
///     .unwrap();
///
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// let view: &HandoverView = &state.handovers[0];
/// assert_eq!(view.id, id, "identified by what it says, not by who said it");
/// assert_eq!(view.consumed_by.len(), 1, "somebody has picked this one up");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HandoverView {
    /// The content digest.
    pub id: String,
    /// The stream that first published it.
    pub stream: StreamId,
    /// `<node>-<runtime>` of the publisher.
    pub runtime: String,
    /// The Lamport stamp of the first publication.
    pub published_lamport: u64,
    /// The record's facts and body.
    pub handover: HandoverBody,
    /// Every session that consumed it, by key, in Lamport order.
    pub consumed_by: Vec<String>,
}

/// A review request's standing: whether anybody has answered it yet. There is deliberately
/// no `Approved` or `Rejected` here — a request may be answered by several runtimes and the
/// verdicts are the answers' own, so collapsing them into one state would throw away the
/// disagreement that makes a second reviewer worth asking.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, ReviewState};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::ReviewRequested { review: "r1".into(), session: "s1".into(),
///     subject: "feature/x".into(), scope: vec![], issue: None, reviewer: None }).unwrap();
/// let asked = fold(&j.events(), &|_| StreamLiveness::Own);
/// assert_eq!(asked.reviews[0].state, ReviewState::Open);
///
/// let request = asked.reviews[0].key.clone();
/// j.append_own(EventBody::ReviewAnswered { request, session: "s2".into(),
///     verdict: "approved".into(), note: None }).unwrap();
/// assert_eq!(fold(&j.events(), &|_| StreamLiveness::Own).reviews[0].state, ReviewState::Answered);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    /// Nobody has answered.
    Open,
    /// At least one answer.
    Answered,
}

/// One answer to a review request: who answered, what they decided, and why. The verdict
/// is one of the three declared words, so a requester can act on it without reading the
/// note; the note is where the reasoning goes and is optional, because "approved" often
/// needs none.
///
/// ```
/// use majordomus_cli::mesh::state::ReviewAnswer;
///
/// let answer = ReviewAnswer {
///     session: "a1b2c3d4-r1-0f/s2".into(),
///     verdict: "changes_requested".into(),
///     note: Some("the fold is not deterministic".into()),
/// };
/// assert_eq!(answer.verdict, "changes_requested");
///
/// // the note is omitted rather than carried empty when there is nothing to say
/// let bare = ReviewAnswer { note: None, ..answer };
/// let value = serde_json::to_value(&bare).unwrap();
/// assert!(value.get("note").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewAnswer {
    /// The answering session's key.
    pub session: String,
    /// The verdict.
    pub verdict: String,
    /// The note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One review request anywhere in the mesh, with every answer it has drawn. A request with
/// no `reviewer` is an offer to the whole mesh rather than to nobody, which is why the
/// answers are a list: any linked runtime carrying the `reviews` feature may add one, and
/// they are kept in Lamport order so that every runtime reads them in the same sequence.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, ReviewView};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::ReviewRequested { review: "r1".into(), session: "s1".into(),
///     subject: "feature/mesh-cooperation".into(), scope: vec!["src/mesh".into()],
///     issue: Some("#184".into()), reviewer: None }).unwrap();
/// let request = fold(&j.events(), &|_| StreamLiveness::Own).reviews[0].key.clone();
/// j.append_own(EventBody::ReviewAnswered { request, session: "s2".into(),
///     verdict: "approved".into(), note: Some("reads well".into()) }).unwrap();
///
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// let review: &ReviewView = &state.reviews[0];
/// assert_eq!(review.subject, "feature/mesh-cooperation");
/// assert_eq!(review.reviewer, None, "offered to the mesh, not addressed");
/// assert_eq!(review.answers[0].verdict, "approved");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewView {
    /// `<stream>/<review>`.
    pub key: String,
    /// The requesting session's key.
    pub session: String,
    /// What is to be reviewed.
    pub subject: String,
    /// Paths it covers.
    pub scope: Vec<String>,
    /// The issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The runtime asked, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    /// Its standing.
    pub state: ReviewState,
    /// The answers, in Lamport order.
    pub answers: Vec<ReviewAnswer>,
}

/// The whole folded state, every collection in key order, and its digest: two runtimes
/// that agree print the same digest.
///
/// The digest is the point. Convergence is otherwise a claim nobody can check from one
/// machine; with it, two operators compare one short string and know whether their
/// runtimes are looking at the same world. `opaque_events` is the other half of that
/// honesty: events of kinds this executable does not interpret are counted and relayed
/// rather than dropped, so a newer peer's traffic is visible instead of silently missing.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, CooperationState};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "cli") }).unwrap();
///
/// let events = j.events();
/// let state: CooperationState = fold(&events, &|_| StreamLiveness::Own);
/// assert_eq!(state.digest.len(), 32);
///
/// // the same events in another order, and twice over, are the same state
/// let mut shuffled = events.clone();
/// shuffled.reverse();
/// shuffled.extend(events);
/// assert_eq!(fold(&shuffled, &|_| StreamLiveness::Own).digest, state.digest);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CooperationState {
    /// Every session.
    pub sessions: Vec<SessionView>,
    /// Every claim.
    pub claims: Vec<ClaimView>,
    /// Advisory overlaps between live claims.
    pub overlaps: Vec<ClaimOverlap>,
    /// Every handover.
    pub handovers: Vec<HandoverView>,
    /// Every review request.
    pub reviews: Vec<ReviewView>,
    /// Events of kinds this executable does not interpret.
    pub opaque_events: usize,
    /// 32 hex of the SHA-256 of the canonical JSON of everything above.
    pub digest: String,
}

/// Fold events into state. `liveness` answers for each stream; the caller supplies the
/// journal's verdicts, a test supplies whatever it is testing.
///
/// Liveness is a parameter rather than something read here because it is the one input
/// that is not in the events: whether a stream still beats is a local observation, and
/// making it an argument is what keeps the fold itself pure — the same events with the
/// same verdicts give the same state, in any order and however often they are repeated.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::state::{fold, ClaimState};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::ClaimAcquired { claim: "c1".into(), session: "s1".into(),
///     scope: vec!["apps".into()], intent: None, mode: ClaimMode::Exclusive, issue: None })
///     .unwrap();
///
/// // read where the writer is alive, the claim holds
/// let here = fold(&j.events(), &|_| StreamLiveness::Own);
/// assert_eq!(here.claims[0].state, ClaimState::Held);
///
/// // read where its beat has stopped, it ended on its own; nobody released it
/// let elsewhere = fold(&j.events(), &|_| StreamLiveness::Expired);
/// assert!(matches!(elsewhere.claims[0].state, ClaimState::Expired(_)));
/// ```
pub fn fold(
    events: &[MeshEvent],
    liveness: &dyn Fn(&StreamId) -> StreamLiveness,
) -> CooperationState {
    // Keyed by the causal position, so the map both orders and deduplicates: an event is
    // identified by `(stream, seq)`, and the same event twice is one entry.
    let ordered: Vec<&MeshEvent> = events
        .iter()
        .map(|e| (causal_key(e), e))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect();

    let mut sessions: BTreeMap<String, SessionView> = BTreeMap::new();
    let mut claims: BTreeMap<String, ClaimView> = BTreeMap::new();
    let mut handovers: BTreeMap<String, HandoverView> = BTreeMap::new();
    let mut reviews: BTreeMap<String, ReviewView> = BTreeMap::new();
    let mut opaque = 0usize;
    let mut closed_at: BTreeMap<String, u64> = BTreeMap::new();

    for event in ordered {
        let Some(body) = event.body() else {
            opaque += 1;
            continue;
        };
        let stream = &event.stream;
        let key = |local: &str| format!("{stream}/{local}");
        match body {
            EventBody::SessionOpened { info } => {
                let k = key(&info.session);
                sessions.insert(
                    k.clone(),
                    SessionView {
                        key: k,
                        stream: stream.clone(),
                        runtime: stream.runtime_key(),
                        node: stream.node().to_string(),
                        info,
                        state: SessionState::Active,
                        updated_lamport: event.lamport,
                    },
                );
            }
            EventBody::SessionClosed { session } => {
                let k = key(&session);
                // A close ends every claim acquired before it, even if the same session id
                // opens again later (a board position is reused): those claims stay ended.
                let closed = closed_at.entry(k.clone()).or_insert(0);
                *closed = (*closed).max(event.lamport);
                if let Some(view) = sessions.get_mut(&k) {
                    view.state = SessionState::Closed;
                    view.updated_lamport = event.lamport;
                }
            }
            EventBody::ClaimAcquired {
                claim,
                session,
                scope,
                intent,
                mode,
                issue,
            } => {
                let k = key(&claim);
                claims.insert(
                    k.clone(),
                    ClaimView {
                        key: k,
                        stream: stream.clone(),
                        runtime: stream.runtime_key(),
                        session: key(&session),
                        scope,
                        intent,
                        mode,
                        issue,
                        acquired_lamport: event.lamport,
                        acquired_seq: event.seq,
                        state: ClaimState::Held,
                    },
                );
            }
            EventBody::ClaimReleased { claim } => {
                if let Some(view) = claims.get_mut(&key(&claim)) {
                    view.state = ClaimState::Released;
                }
            }
            EventBody::HandoverPublished { handover } => {
                handovers
                    .entry(handover.id.clone())
                    .or_insert_with(|| HandoverView {
                        id: handover.id.clone(),
                        stream: stream.clone(),
                        runtime: stream.runtime_key(),
                        published_lamport: event.lamport,
                        handover,
                        consumed_by: Vec::new(),
                    });
            }
            EventBody::HandoverConsumed { handover, session } => {
                if let Some(view) = handovers.get_mut(&handover) {
                    let consumer = key(&session);
                    if !view.consumed_by.contains(&consumer) {
                        view.consumed_by.push(consumer);
                    }
                }
            }
            EventBody::ReviewRequested {
                review,
                session,
                subject,
                scope,
                issue,
                reviewer,
            } => {
                let k = key(&review);
                reviews.entry(k.clone()).or_insert(ReviewView {
                    key: k,
                    session: key(&session),
                    subject,
                    scope,
                    issue,
                    reviewer,
                    state: ReviewState::Open,
                    answers: Vec::new(),
                });
            }
            EventBody::ReviewAnswered {
                request,
                session,
                verdict,
                note,
            } => {
                // A review is an independent verdict: the session that asked for it cannot
                // answer it. Admission refuses that; a peer that writes one anyway is ignored.
                if let Some(view) = reviews
                    .get_mut(&request)
                    .filter(|view| view.session != key(&session))
                {
                    view.state = ReviewState::Answered;
                    view.answers.push(ReviewAnswer {
                        session: key(&session),
                        verdict,
                        note,
                    });
                }
            }
        }
    }

    // StreamLiveness: a session or claim of a dead stream has ended, whatever it last said.
    for view in sessions.values_mut() {
        if view.state == SessionState::Active && !liveness(&view.stream).is_alive() {
            view.state = SessionState::Expired;
        }
    }
    for view in claims.values_mut() {
        if view.state != ClaimState::Held {
            continue;
        }
        if !liveness(&view.stream).is_alive() {
            view.state = ClaimState::Expired("its runtime stopped beating".into());
        } else if closed_at
            .get(&view.session)
            .is_some_and(|closed| view.acquired_lamport < *closed)
        {
            view.state = ClaimState::Expired("its session closed".into());
        } else if let Some(session) = sessions.get(&view.session) {
            if session.state != SessionState::Active {
                view.state = ClaimState::Expired("its session closed".into());
            }
        }
    }

    // Exclusivity: live exclusive claims in acquisition order; the first to a scope wins.
    // The map is what orders them — the same `(lamport, stream, seq)` on every runtime, so
    // every runtime names the same winner.
    let exclusive: Vec<String> = claims
        .values()
        .filter(|c| c.state.is_live() && c.mode == ClaimMode::Exclusive)
        .map(|c| (c.order_key(), c.key.clone()))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect();
    let mut winners: Vec<(String, String, Vec<String>)> = Vec::new();
    for key in exclusive {
        let Some(claim) = claims.get_mut(&key) else {
            continue;
        };
        let beaten = winners.iter().find(|(_, session, scope)| {
            *session != claim.session && scopes_meet(scope, &claim.scope)
        });
        match beaten {
            Some((winner, _, _)) => claim.state = ClaimState::Conflicted(winner.clone()),
            None => {
                claim.state = ClaimState::Held;
                winners.push((
                    claim.key.clone(),
                    claim.session.clone(),
                    claim.scope.clone(),
                ));
            }
        }
    }

    // Advisory overlaps: live claims of different sessions that meet, not both exclusive.
    let live: Vec<&ClaimView> = claims
        .values()
        .filter(|c| c.state.is_live())
        .map(|c| (c.order_key(), c))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect();
    let mut overlaps = Vec::new();
    for (i, first) in live.iter().enumerate() {
        for second in &live[i + 1..] {
            if first.session == second.session
                || (first.mode == ClaimMode::Exclusive && second.mode == ClaimMode::Exclusive)
            {
                continue;
            }
            let paths = meeting_paths(&first.scope, &second.scope);
            if !paths.is_empty() {
                overlaps.push(ClaimOverlap {
                    first: first.key.clone(),
                    second: second.key.clone(),
                    paths,
                });
            }
        }
    }

    let mut state = CooperationState {
        sessions: sessions.into_values().collect(),
        claims: claims.into_values().collect(),
        overlaps,
        handovers: handovers.into_values().collect(),
        reviews: reviews.into_values().collect(),
        opaque_events: opaque,
        digest: String::new(),
    };
    state.digest = digest(&state);
    state
}

fn digest(state: &CooperationState) -> String {
    use sha2::{Digest, Sha256};
    let value = serde_json::to_value(state).unwrap_or_default();
    let hash = Sha256::digest(canonical_json(&value));
    hash.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// Whether any path of one scope meets any path of the other. Paths meet by
/// [`crate::peers::claims_meet`], the peer board's own predicate, so that a claim means the
/// same thing on this machine and on another: a directory contains what is under it, and
/// two claims that share no path do not meet however similar they look.
///
/// ```
/// use majordomus_cli::mesh::state::scopes_meet;
///
/// assert!(scopes_meet(&["apps".into()], &["apps/majordomus-cli/src".into()]), "a tree contains it");
/// assert!(!scopes_meet(&["apps".into()], &["docs".into()]));
/// assert!(!scopes_meet(&[], &["apps".into()]), "a claim over nothing meets nothing");
/// ```
pub fn scopes_meet(a: &[String], b: &[String]) -> bool {
    a.iter()
        .any(|x| b.iter().any(|y| crate::peers::claims_meet(x, y)))
}

fn meeting_paths(a: &[String], b: &[String]) -> Vec<(String, String)> {
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

/// The live exclusive claims of other sessions a new exclusive claim of `session` over
/// `scope` would meet: empty means admissible. An advisory claim is always admissible.
///
/// This is the difference between refusing a claim and reporting a conflict after the
/// fact. Admission is checked against the folded state before anything is written, so a
/// claim that could have been refused is refused; what is left — genuinely concurrent
/// claims from two sides of a partition — is what the fold ranks afterwards, and both
/// runtimes name the same winner.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, StreamLiveness};
/// use majordomus_cli::mesh::state::{admission_conflicts, fold};
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::ClaimAcquired { claim: "c1".into(), session: "s1".into(),
///     scope: vec!["apps".into()], intent: None, mode: ClaimMode::Exclusive, issue: None })
///     .unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// let held = &state.claims[0];
///
/// // another session cannot take a scope inside one already held exclusively
/// let met = admission_conflicts(&state, "s2", &["apps/majordomus-cli".into()],
///     ClaimMode::Exclusive);
/// assert_eq!(met.len(), 1);
/// assert_eq!(met[0].key, held.key, "the refusal names what it met");
///
/// // the holder is not in its own way, and an advisory claim is never refused
/// assert!(admission_conflicts(&state, &held.session, &["apps".into()], ClaimMode::Exclusive)
///     .is_empty());
/// assert!(admission_conflicts(&state, "s2", &["apps".into()], ClaimMode::Advisory).is_empty());
/// ```
pub fn admission_conflicts<'a>(
    state: &'a CooperationState,
    session: &str,
    scope: &[String],
    mode: ClaimMode,
) -> Vec<&'a ClaimView> {
    if mode == ClaimMode::Advisory {
        return Vec::new();
    }
    state
        .claims
        .iter()
        .filter(|c| {
            c.state.is_live()
                && c.mode == ClaimMode::Exclusive
                && c.session != session
                && scopes_meet(&c.scope, scope)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::identity::NodeIdentity;
    use crate::mesh::journal::{Journal, Rejection};
    use std::sync::Arc;

    fn journal(runtime: &str) -> Journal {
        Journal::open(
            Arc::new(NodeIdentity::ephemeral().unwrap()),
            runtime,
            "repo".into(),
            None,
        )
        .unwrap()
    }

    fn claim(j: &Journal, claim: &str, session: &str, scope: &[&str], mode: ClaimMode) {
        j.append_own(EventBody::ClaimAcquired {
            claim: claim.into(),
            session: session.into(),
            scope: scope.iter().map(|s| s.to_string()).collect(),
            intent: None,
            mode,
            issue: Some("#184".into()),
        })
        .unwrap();
    }

    fn sync(from: &Journal, to: &Journal) {
        to.ingest(
            &from.missing_for(&to.marks(), usize::MAX),
            &|_: &MeshEvent| Ok::<(), Rejection>(()),
        );
    }

    fn all_live(_: &StreamId) -> StreamLiveness {
        StreamLiveness::Live
    }

    #[test]
    fn a_remote_claim_is_seen_and_refuses_a_conflicting_admission() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        claim(
            &a,
            "c1",
            "s1",
            &["apps/majordomus-cli"],
            ClaimMode::Exclusive,
        );
        sync(&a, &b);
        let state = fold(&b.events(), &all_live);
        assert_eq!(state.claims.len(), 1);
        assert_eq!(state.claims[0].state, ClaimState::Held);
        let conflicts = admission_conflicts(
            &state,
            &format!("{}/s9", b.own_stream()),
            &["apps".into()],
            ClaimMode::Exclusive,
        );
        assert_eq!(
            conflicts.len(),
            1,
            "B's exclusive claim over apps meets A's"
        );
        assert!(
            admission_conflicts(&state, "x", &["docs".into()], ClaimMode::Exclusive).is_empty()
        );
        assert!(
            admission_conflicts(&state, "x", &["apps".into()], ClaimMode::Advisory).is_empty(),
            "advisory never refuses"
        );
    }

    #[test]
    fn a_dead_holder_releases_nothing_yet_its_claim_expires() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        claim(&a, "c1", "s1", &["apps"], ClaimMode::Exclusive);
        sync(&a, &b);
        let dead = fold(&b.events(), &|_| StreamLiveness::Expired);
        assert!(matches!(dead.claims[0].state, ClaimState::Expired(_)));
        assert!(
            admission_conflicts(&dead, "other", &["apps".into()], ClaimMode::Exclusive).is_empty(),
            "an expired lease no longer excludes"
        );
    }

    #[test]
    fn concurrent_claims_name_the_same_winner_everywhere() {
        // Two sides of a partition claim the same scope; after healing, both runtimes fold
        // the same events and agree on the winner, regardless of who synced first.
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        claim(&a, "c1", "s1", &["apps"], ClaimMode::Exclusive);
        claim(
            &b,
            "c1",
            "s1",
            &["apps/majordomus-cli"],
            ClaimMode::Exclusive,
        );
        sync(&a, &b);
        sync(&b, &a);
        let at_a = fold(&a.events(), &all_live);
        let at_b = fold(&b.events(), &all_live);
        assert_eq!(at_a.digest, at_b.digest, "both runtimes converge");
        let held: Vec<_> = at_a
            .claims
            .iter()
            .filter(|c| c.state == ClaimState::Held)
            .collect();
        let conflicted: Vec<_> = at_a
            .claims
            .iter()
            .filter(|c| matches!(c.state, ClaimState::Conflicted(_)))
            .collect();
        assert_eq!((held.len(), conflicted.len()), (1, 1));
        assert_eq!(
            conflicted[0].state,
            ClaimState::Conflicted(held[0].key.clone())
        );
    }

    #[test]
    fn release_and_session_close_end_claims() {
        let a = journal("0000000000000001");
        claim(&a, "c1", "s1", &["apps"], ClaimMode::Exclusive);
        claim(&a, "c2", "s2", &["docs"], ClaimMode::Exclusive);
        a.append_own(EventBody::SessionOpened {
            info: SessionInfo::named("s2", "test"),
        })
        .unwrap();
        a.append_own(EventBody::ClaimReleased { claim: "c1".into() })
            .unwrap();
        a.append_own(EventBody::SessionClosed {
            session: "s2".into(),
        })
        .unwrap();
        let state = fold(&a.events(), &|_| StreamLiveness::Own);
        assert_eq!(state.claims[0].state, ClaimState::Released);
        assert!(matches!(state.claims[1].state, ClaimState::Expired(_)));
        assert_eq!(state.sessions[0].state, SessionState::Closed);
    }

    #[test]
    fn a_reopened_session_id_does_not_bring_back_the_claims_its_close_ended() {
        let a = journal("0000000000000001");
        claim(&a, "c1", "p1", &["apps"], ClaimMode::Exclusive);
        a.append_own(EventBody::SessionClosed {
            session: "p1".into(),
        })
        .unwrap();
        a.append_own(EventBody::SessionOpened {
            info: SessionInfo::named("p1", "test"),
        })
        .unwrap();
        let state = fold(&a.events(), &|_| StreamLiveness::Own);
        assert!(matches!(state.claims[0].state, ClaimState::Expired(_)));
        claim(&a, "c2", "p1", &["apps"], ClaimMode::Exclusive);
        let state = fold(&a.events(), &|_| StreamLiveness::Own);
        let second = state
            .claims
            .iter()
            .find(|c| c.key.ends_with("/c2"))
            .unwrap();
        assert_eq!(
            second.state,
            ClaimState::Held,
            "a claim made after the reopen holds"
        );
    }

    #[test]
    fn advisory_overlaps_are_reported_not_refused() {
        let a = journal("0000000000000001");
        claim(&a, "c1", "s1", &["apps"], ClaimMode::Advisory);
        claim(&a, "c2", "s2", &["apps/x"], ClaimMode::Exclusive);
        let state = fold(&a.events(), &|_| StreamLiveness::Own);
        assert!(state.claims.iter().all(|c| c.state == ClaimState::Held));
        assert_eq!(state.overlaps.len(), 1);
        assert_eq!(
            state.overlaps[0].paths,
            vec![("apps".into(), "apps/x".into())]
        );
    }

    #[test]
    fn handovers_and_reviews_link_across_streams() {
        let a = journal("0000000000000001");
        let b = journal("0000000000000002");
        let body = "# Objective\nship\n# Current State\nhalf\n# Next Action\nrest\n".to_string();
        let id = HandoverBody::digest_of(&body);
        a.append_own(EventBody::HandoverPublished {
            handover: HandoverBody {
                id: id.clone(),
                task: Some("t-1".into()),
                issue: Some("#184".into()),
                milestone: None,
                branch: Some("feature/x".into()),
                head: Some("abc".into()),
                created_at: None,
                name: None,
                body: body.clone(),
            },
        })
        .unwrap();
        a.append_own(EventBody::ReviewRequested {
            review: "r1".into(),
            session: "s1".into(),
            subject: "feature/x".into(),
            scope: vec!["apps".into()],
            issue: Some("#184".into()),
            reviewer: None,
        })
        .unwrap();
        sync(&a, &b);
        b.append_own(EventBody::HandoverConsumed {
            handover: id.clone(),
            session: "s2".into(),
        })
        .unwrap();
        b.append_own(EventBody::ReviewAnswered {
            request: format!("{}/r1", a.own_stream()),
            session: "s2".into(),
            verdict: "approved".into(),
            note: None,
        })
        .unwrap();
        sync(&b, &a);
        let at_a = fold(&a.events(), &all_live);
        assert_eq!(
            at_a.handovers[0].consumed_by,
            vec![format!("{}/s2", b.own_stream())]
        );
        assert_eq!(at_a.reviews[0].state, ReviewState::Answered);
        assert_eq!(at_a.reviews[0].answers[0].verdict, "approved");
        assert_eq!(at_a.digest, fold(&b.events(), &all_live).digest);
    }

    #[test]
    fn the_session_that_asked_for_a_review_cannot_answer_it() {
        let a = journal("0000000000000001");
        a.append_own(EventBody::ReviewRequested {
            review: "r1".into(),
            session: "s1".into(),
            subject: "feature/x".into(),
            scope: vec![],
            issue: None,
            reviewer: None,
        })
        .unwrap();
        a.append_own(EventBody::ReviewAnswered {
            request: format!("{}/r1", a.own_stream()),
            session: "s1".into(),
            verdict: "approved".into(),
            note: None,
        })
        .unwrap();
        let state = fold(&a.events(), &|_| StreamLiveness::Own);
        assert_eq!(state.reviews[0].state, ReviewState::Open);
        assert!(state.reviews[0].answers.is_empty());
    }

    proptest::proptest! {
        /// Folding any permutation of the same events, with any duplicates, yields the same
        /// state: arrival order can never change canonical state.
        #[test]
        fn fold_is_order_and_duplicate_independent(
            order in proptest::collection::vec(0usize..8, 8..32),
            dead in proptest::prelude::any::<bool>(),
        ) {
            let a = journal("0000000000000001");
            let b = journal("0000000000000002");
            claim(&a, "c1", "s1", &["apps"], ClaimMode::Exclusive);
            claim(&b, "c1", "s1", &["apps/majordomus-cli"], ClaimMode::Exclusive);
            claim(&a, "c2", "s1", &["docs"], ClaimMode::Advisory);
            claim(&b, "c2", "s2", &["docs/MESH.md"], ClaimMode::Advisory);
            a.append_own(EventBody::ClaimReleased { claim: "c2".into() }).unwrap();
            b.append_own(EventBody::SessionOpened { info: SessionInfo::named("s2", "t") }).unwrap();
            a.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "t") }).unwrap();
            b.append_own(EventBody::SessionClosed { session: "s2".into() }).unwrap();
            let mut events = a.events();
            events.extend(b.events());
            let dead_stream = a.own_stream().clone();
            let live = move |s: &StreamId| if dead && *s == dead_stream { StreamLiveness::Expired } else { StreamLiveness::Live };
            let reference = fold(&events, &live);
            let shuffled: Vec<MeshEvent> = order.iter().map(|i| events[*i].clone()).chain(events.iter().cloned()).collect();
            proptest::prop_assert_eq!(fold(&shuffled, &live), reference);
        }

        /// However claims interleave, no scope ever has two held exclusive winners from
        /// different sessions.
        #[test]
        fn exclusivity_holds_for_any_interleaving(picks in proptest::collection::vec((0usize..3, 0usize..4), 1..20)) {
            let journals = [journal("0000000000000001"), journal("0000000000000002"), journal("0000000000000003")];
            let scopes = ["apps", "apps/x", "docs", "apps/x/y"];
            for (i, (who, scope)) in picks.iter().enumerate() {
                claim(&journals[*who], &format!("c{i}"), &format!("s{who}"), &[scopes[*scope]], ClaimMode::Exclusive);
            }
            let events: Vec<MeshEvent> = journals.iter().flat_map(|j| j.events()).collect();
            let state = fold(&events, &all_live);
            let held: Vec<&ClaimView> = state.claims.iter().filter(|c| c.state == ClaimState::Held).collect();
            for (i, x) in held.iter().enumerate() {
                for y in &held[i + 1..] {
                    proptest::prop_assert!(x.session == y.session || !scopes_meet(&x.scope, &y.scope));
                }
            }
        }
    }
}
