//! What one answer about the mesh carries: a summary a worker can read whole, or the folded
//! state narrowed to what was asked.
//!
//! On 2026-10-10 `mesh.state` answered 449 kB and `mesh.peers` 313 kB on a mesh of twelve
//! runtimes: 165 claims, half of them released, 126 sessions, 109 of them closed. A worker
//! asking "who holds what" over MCP got an answer its client could not show, and a page of
//! 80 journal events was 83 000 characters, 30 of them one session reopening itself. The
//! fold is not wrong to keep its history; an answer is wrong to carry all of it by default.
//!
//! So the default is a summary that does not grow with history — per runtime its liveness,
//! its sessions counted by standing and its newest live claim; the totals; the open review
//! requests and the handovers nobody has taken — bounded by [`SUMMARY_LIST`] and
//! [`SUMMARY_TEXT`]. The whole state is still one input away, and the filters
//! narrow it without changing its shape. The digest is always the digest of the whole fold,
//! never of a page, because it is what two runtimes compare.
//!
//! Everything here is pure: the same state, liveness and query give the same answer. A
//! capability folds the journal, narrows the state to the query, and answers either the
//! narrowed state or its summary:
//!
//! ```
//! use std::sync::Arc;
//! use majordomus_cli::mesh::identity::NodeIdentity;
//! use majordomus_cli::mesh::journal::{EventBody, Journal, SessionInfo, StreamLiveness};
//! use majordomus_cli::mesh::page::{narrow, summarize, Query, View};
//! use majordomus_cli::mesh::state::fold;
//!
//! let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
//!     "repo".into(), None).unwrap();
//! for n in 0..3 {
//!     j.append_own(EventBody::SessionOpened { info: SessionInfo::named(&format!("s{n}"), "cli") })
//!         .unwrap();
//! }
//! j.append_own(EventBody::SessionClosed { session: "s0".into() }).unwrap();
//! let state = fold(&j.events(), &|_| StreamLiveness::Own);
//!
//! let query = Query { open: Some(true), ..Query::default() };
//! let narrowed = narrow(&state, &query);
//! assert_eq!(narrowed.sessions.len(), 2, "the closed session is left out");
//! assert_eq!(query.view(), View::Summary);
//! let summary = summarize(&narrowed, &[]);
//! assert_eq!(summary.sessions.active, 2);
//! assert_eq!(summary.digest, state.digest, "always the whole fold's digest");
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::journal::StreamLiveness;
use super::state::{ClaimState, CooperationState, ReviewState, SessionState};

/// The most entries a summary lists in any one list; what is past it is counted.
pub const SUMMARY_LIST: usize = 24;

/// The most characters a summary keeps of an intent or a subject; the rest is cut with `…`.
pub const SUMMARY_TEXT: usize = 160;

/// How much an answer about the mesh carries. The summary is the default because it is the
/// answer a worker can read whole whatever the mesh's history; the full fold is one word
/// away when a reader needs every record.
///
/// ```
/// use majordomus_cli::mesh::page::View;
///
/// assert_eq!(View::default(), View::Summary);
/// assert_eq!(serde_json::to_value(View::Full).unwrap(), "full");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MeshView")]
pub enum View {
    /// Counts, each runtime's newest live claim, the open reviews and the untaken handovers:
    /// bounded, whatever the history.
    #[default]
    Summary,
    /// Everything, narrowed only by the filters.
    Full,
}

/// Which claims a full answer keeps: all of them, the ended ones included, or only those
/// still live — the question an executor asks before it takes a lease.
///
/// ```
/// use majordomus_cli::mesh::page::ClaimFilter;
///
/// assert_eq!(ClaimFilter::default(), ClaimFilter::All);
/// assert_eq!(serde_json::from_str::<ClaimFilter>(r#""live""#).unwrap(), ClaimFilter::Live);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MeshClaimFilter")]
pub enum ClaimFilter {
    /// Every claim, ended ones included.
    #[default]
    All,
    /// Only claims that are still live: held, or conflicted and still held.
    Live,
}

/// One collection of the folded state, as `kind` names it: a reader that wants only the
/// reviews is not sent the claims.
///
/// ```
/// use majordomus_cli::mesh::page::Kind;
///
/// assert_eq!(serde_json::from_str::<Kind>(r#""reviews""#).unwrap(), Kind::Reviews);
/// assert!(serde_json::from_str::<Kind>(r#""everything""#).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "MeshKind")]
pub enum Kind {
    /// The sessions.
    Sessions,
    /// The claims.
    Claims,
    /// The advisory overlaps.
    Overlaps,
    /// The handovers.
    Handovers,
    /// The review requests.
    Reviews,
}

/// What a reader asks of the folded state. Every filter narrows; none changes a shape, and
/// an input it does not know is refused rather than ignored.
///
/// ```
/// use majordomus_cli::mesh::page::{ClaimFilter, Query, View};
///
/// let q: Query = serde_json::from_str(r#"{"view":"full","claims":"live","limit":5}"#).unwrap();
/// assert_eq!(q.view(), View::Full);
/// assert_eq!((q.claims, q.limit), (Some(ClaimFilter::Live), Some(5)));
/// assert!(serde_json::from_str::<Query>(r#"{"views":"full"}"#).is_err());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "MeshQuery")]
pub struct Query {
    /// `summary` (the default) or `full`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<View>,
    /// `all` (the default) or `live`: whether ended claims are kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claims: Option<ClaimFilter>,
    /// Only sessions, claims and handovers whose latest Lamport stamp is above this. Reviews
    /// and overlaps carry no stamp of their own and are kept or dropped with their sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    /// Only what belongs to this session: its key (`<stream>/<session>`) or its bare id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Only what belongs to this runtime: `<node>-<runtime>`, or a node id for every runtime
    /// of that machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// Only this collection; the others are answered empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    /// At most this many of each collection: the newest by Lamport stamp where it has one,
    /// kept in the fold's own order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Only what is still open: active sessions, live claims, review requests nobody has
    /// answered and handovers nobody has taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<bool>,
    /// Only what mentions this text, ignoring case: a path, a branch, an issue, a pull
    /// request, a word of an intent or a subject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matching: Option<String>,
}

impl crate::capability::benchmark::BenchmarkCases for Query {
    fn benchmark_cases(
        _: &crate::capability::benchmark::CaseContext<'_>,
    ) -> Vec<crate::capability::benchmark::NamedCase<Self>> {
        use crate::capability::benchmark::NamedCase;
        vec![
            NamedCase::new("summary", Query::default()),
            NamedCase::new(
                "full",
                Query {
                    view: Some(View::Full),
                    ..Query::default()
                },
            ),
        ]
    }
}

impl Query {
    /// The view asked for, the summary when none was: the one place the default is decided,
    /// so every capability that takes a query answers the same view for the same input.
    ///
    /// ```
    /// use majordomus_cli::mesh::page::{Query, View};
    ///
    /// assert_eq!(Query::default().view(), View::Summary);
    /// let full = Query { view: Some(View::Full), ..Query::default() };
    /// assert_eq!(full.view(), View::Full);
    /// ```
    pub fn view(&self) -> View {
        self.view.unwrap_or_default()
    }

    /// Whether `key` (`<stream>/<session>`) is the session asked about, or nothing was.
    pub(crate) fn admits_session(&self, key: &str) -> bool {
        self.session.as_deref().is_none_or(|wanted| {
            key == wanted || key.rsplit_once('/').is_some_and(|(_, id)| id == wanted)
        })
    }

    /// Whether `runtime` (`<node>-<runtime>`) is the runtime asked about, or nothing was.
    pub(crate) fn admits_runtime(&self, runtime: &str) -> bool {
        self.runtime
            .as_deref()
            .is_none_or(|wanted| runtime == wanted || runtime.split('-').next() == Some(wanted))
    }

    fn admits_lamport(&self, lamport: u64) -> bool {
        self.after.is_none_or(|after| lamport > after)
    }

    fn admits_kind(&self, kind: Kind) -> bool {
        self.kind.is_none_or(|k| k == kind)
    }

    fn open_only(&self) -> bool {
        self.open == Some(true)
    }

    /// Whether any of `texts` mentions the text asked for, or nothing was asked.
    fn admits_text<'a>(&self, texts: impl IntoIterator<Item = Option<&'a str>>) -> bool {
        let Some(wanted) = self.matching.as_deref().map(str::to_lowercase) else {
            return true;
        };
        texts
            .into_iter()
            .flatten()
            .any(|text| text.to_lowercase().contains(&wanted))
    }
}

/// The newest `limit` of `items` by `stamp`, in their own order; all of them without a limit.
fn newest<T>(items: Vec<T>, limit: Option<usize>, stamp: impl Fn(&T) -> u64) -> Vec<T> {
    let Some(limit) = limit else {
        return items;
    };
    // the `limit`-th highest stamp, popped from a heap rather than sorted out of a list
    let mut stamps: std::collections::BinaryHeap<u64> = items.iter().map(&stamp).collect();
    if stamps.len() <= limit {
        return items;
    }
    let floor = (0..limit.max(1))
        .filter_map(|_| stamps.pop())
        .last()
        .unwrap_or(u64::MAX);
    // the stamp at the limit admits ties, so equal stamps are kept or dropped together
    // until the count is reached, in the fold's own order
    let mut kept = 0;
    items
        .into_iter()
        .filter(|item| {
            let keep = stamp(item) >= floor && kept < limit;
            kept += usize::from(keep);
            keep
        })
        .collect()
}

/// The state narrowed to `query`. The digest is the whole fold's, carried unchanged: it is
/// what two runtimes compare, and a page of the state is not a state of its own.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::fold;
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "claude-code") })
///     .unwrap();
/// for (claim, path) in [("c1", "apps/a"), ("c2", "apps/b")] {
///     j.append_own(EventBody::ClaimAcquired { claim: claim.into(), session: "s1".into(),
///         scope: vec![path.into()], intent: Some("work".into()), mode: ClaimMode::Exclusive,
///         issue: Some("#1".into()) }).unwrap();
/// }
/// j.append_own(EventBody::ClaimReleased { claim: "c1".into() }).unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// use majordomus_cli::mesh::page::{narrow, ClaimFilter, Query};
///
/// let live = narrow(&state, &Query { claims: Some(ClaimFilter::Live), ..Query::default() });
/// assert_eq!(live.claims.len(), 1, "the released claim is left out");
/// assert_eq!(live.digest, state.digest, "the whole fold's digest");
/// ```
pub fn narrow(state: &CooperationState, query: &Query) -> CooperationState {
    let sessions: Vec<_> = state
        .sessions
        .iter()
        .filter(|s| query.admits_session(&s.key) && query.admits_runtime(&s.runtime))
        .filter(|s| query.admits_lamport(s.updated_lamport))
        .filter(|s| !query.open_only() || s.state == SessionState::Active)
        .filter(|s| {
            let i = &s.info;
            query.admits_text([
                Some(s.key.as_str()),
                i.intent.as_deref(),
                i.issue.as_deref(),
                i.milestone.as_deref(),
                i.branch.as_deref(),
                i.worker.as_deref(),
            ])
        })
        .cloned()
        .collect();
    let claims: Vec<_> = state
        .claims
        .iter()
        .filter(|c| query.admits_session(&c.session) && query.admits_runtime(&c.runtime))
        .filter(|c| query.admits_lamport(c.acquired_lamport))
        .filter(|c| query.claims != Some(ClaimFilter::Live) || c.state.is_live())
        .filter(|c| !query.open_only() || c.state.is_live())
        .filter(|c| {
            query.admits_text(
                c.scope
                    .iter()
                    .map(|p| Some(p.as_str()))
                    .chain([c.intent.as_deref(), c.issue.as_deref()]),
            )
        })
        .cloned()
        .collect();
    let kept: std::collections::BTreeSet<&str> = claims.iter().map(|c| c.key.as_str()).collect();
    let overlaps: Vec<_> = state
        .overlaps
        .iter()
        .filter(|o| kept.contains(o.first.as_str()) || kept.contains(o.second.as_str()))
        .cloned()
        .collect();
    let handovers: Vec<_> = state
        .handovers
        .iter()
        .filter(|h| query.admits_runtime(&h.runtime))
        .filter(|h| query.admits_lamport(h.published_lamport))
        .filter(|h| {
            query.session.is_none() || h.consumed_by.iter().any(|s| query.admits_session(s))
        })
        .filter(|h| !query.open_only() || h.consumed_by.is_empty())
        .filter(|h| {
            let b = &h.handover;
            query.admits_text([
                b.branch.as_deref(),
                b.issue.as_deref(),
                b.milestone.as_deref(),
                b.name.as_deref(),
            ])
        })
        .cloned()
        .collect();
    let reviews: Vec<_> = state
        .reviews
        .iter()
        .filter(|r| query.admits_session(&r.session))
        .filter(|r| query.admits_runtime(runtime_of(&r.session)))
        .filter(|r| query.after.is_none() || sessions.iter().any(|s| s.key == r.session))
        .filter(|r| !query.open_only() || r.state == ReviewState::Open)
        .filter(|r| {
            query.admits_text(
                r.scope
                    .iter()
                    .map(|p| Some(p.as_str()))
                    .chain([Some(r.subject.as_str()), r.issue.as_deref()]),
            )
        })
        .cloned()
        .collect();
    let only = |kind: Kind| query.admits_kind(kind);
    CooperationState {
        sessions: newest(sessions, query.limit, |s| s.updated_lamport)
            .into_iter()
            .filter(|_| only(Kind::Sessions))
            .collect(),
        claims: newest(claims, query.limit, |c| c.acquired_lamport)
            .into_iter()
            .filter(|_| only(Kind::Claims))
            .collect(),
        overlaps: newest(overlaps, query.limit, |_| 0)
            .into_iter()
            .filter(|_| only(Kind::Overlaps))
            .collect(),
        handovers: newest(handovers, query.limit, |h| h.published_lamport)
            .into_iter()
            .filter(|_| only(Kind::Handovers))
            .collect(),
        reviews: newest(reviews, query.limit, |_| 0)
            .into_iter()
            .filter(|_| only(Kind::Reviews))
            .collect(),
        opaque_events: state.opaque_events,
        digest: state.digest.clone(),
    }
}

/// The runtime a `<stream>/<id>` key was written by: its first 49 characters.
fn runtime_of(key: &str) -> &str {
    key.get(..49).unwrap_or(key)
}

/// `text` cut to [`SUMMARY_TEXT`] characters, with `…` where it was cut.
pub(crate) fn cut(text: &str) -> String {
    match text.char_indices().nth(SUMMARY_TEXT) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_string(),
    }
}

/// Sessions by standing: how many are active, closed and expired. A count stays the same
/// size however many sessions the mesh has seen, which is why the summary carries counts.
///
/// ```
/// use majordomus_cli::mesh::page::SessionCounts;
///
/// let none = SessionCounts::default();
/// assert_eq!(serde_json::to_value(&none).unwrap(),
///     serde_json::json!({ "active": 0, "closed": 0, "expired": 0 }));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshSessionCounts")]
pub struct SessionCounts {
    /// Open, on a runtime that beats.
    pub active: usize,
    /// Closed by their own runtime.
    pub closed: usize,
    /// Never closed, on a runtime that stopped beating.
    pub expired: usize,
}

impl SessionCounts {
    fn add(&mut self, state: SessionState) {
        match state {
            SessionState::Active => self.active += 1,
            SessionState::Closed => self.closed += 1,
            SessionState::Expired => self.expired += 1,
        }
    }
}

/// Claims by standing: held, conflicted, released and expired. Held and conflicted are
/// live; the other two are history, counted rather than listed.
///
/// ```
/// use majordomus_cli::mesh::page::ClaimCounts;
///
/// let counts = ClaimCounts { held: 2, released: 5, ..ClaimCounts::default() };
/// assert_eq!(serde_json::to_value(&counts).unwrap()["released"], 5);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshClaimCounts")]
pub struct ClaimCounts {
    /// Live and, for an exclusive claim, the winner of its scope.
    pub held: usize,
    /// Live, exclusive, and beaten to its scope.
    pub conflicted: usize,
    /// Released by the holder.
    pub released: usize,
    /// Ended without a release.
    pub expired: usize,
}

impl ClaimCounts {
    fn add(&mut self, state: &ClaimState) {
        match state {
            ClaimState::Held => self.held += 1,
            ClaimState::Conflicted(_) => self.conflicted += 1,
            ClaimState::Released => self.released += 1,
            ClaimState::Expired(_) => self.expired += 1,
        }
    }
}

/// One live claim, told briefly: whose it is, what it is for and how many paths it holds,
/// with its intent cut to [`SUMMARY_TEXT`] characters — enough to know whom to ask.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::fold;
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "claude-code") })
///     .unwrap();
/// for (claim, path) in [("c1", "apps/a"), ("c2", "apps/b")] {
///     j.append_own(EventBody::ClaimAcquired { claim: claim.into(), session: "s1".into(),
///         scope: vec![path.into()], intent: Some("work".into()), mode: ClaimMode::Exclusive,
///         issue: Some("#1".into()) }).unwrap();
/// }
/// j.append_own(EventBody::ClaimReleased { claim: "c1".into() }).unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// use majordomus_cli::mesh::page::{summarize, ClaimBrief};
///
/// let summary = summarize(&state, &[]);
/// let newest: &ClaimBrief = summary.runtimes[0].newest_claim.as_ref().unwrap();
/// assert_eq!((newest.issue.as_deref(), newest.paths), (Some("#1"), 1));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshClaimBrief")]
pub struct ClaimBrief {
    /// `<stream>/<claim>`.
    pub key: String,
    /// The holding session's key.
    pub session: String,
    /// The issue it is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// What it is for, cut to [`SUMMARY_TEXT`] characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// How many paths it claims.
    pub paths: usize,
    /// Exclusive or advisory.
    pub mode: super::journal::ClaimMode,
    /// The Lamport stamp of the acquisition.
    pub acquired_lamport: u64,
}

/// One runtime, counted: its liveness and beat, its sessions and claims by standing, and its
/// newest live claim. The summary lists these, the living runtimes first.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::fold;
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "claude-code") })
///     .unwrap();
/// for (claim, path) in [("c1", "apps/a"), ("c2", "apps/b")] {
///     j.append_own(EventBody::ClaimAcquired { claim: claim.into(), session: "s1".into(),
///         scope: vec![path.into()], intent: Some("work".into()), mode: ClaimMode::Exclusive,
///         issue: Some("#1".into()) }).unwrap();
/// }
/// j.append_own(EventBody::ClaimReleased { claim: "c1".into() }).unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// use majordomus_cli::mesh::page::{summarize, RuntimeSummary};
///
/// let summary = summarize(&state, &[]);
/// let runtime: &RuntimeSummary = &summary.runtimes[0];
/// assert_eq!((runtime.sessions.active, runtime.claims.held, runtime.claims.released), (1, 1, 1));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshRuntimeSummary")]
pub struct RuntimeSummary {
    /// `<node>-<runtime>`.
    pub runtime: String,
    /// Own, live or expired.
    pub liveness: StreamLiveness,
    /// Milliseconds since its beat last rose, on this runtime's clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_beat_ms: Option<u64>,
    /// Its sessions by standing.
    pub sessions: SessionCounts,
    /// Its claims by standing.
    pub claims: ClaimCounts,
    /// Its newest live claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest_claim: Option<ClaimBrief>,
}

/// An open review request, told briefly: its key, what is to be reviewed and whom it asks.
/// Only requests nobody has answered are listed in the summary.
///
/// ```
/// use majordomus_cli::mesh::page::ReviewBrief;
///
/// let brief = ReviewBrief { key: "s/r1".into(), subject: "feature/x".into(), issue: None,
///     reviewer: None };
/// assert!(serde_json::to_value(&brief).unwrap().get("issue").is_none(), "absent, not null");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshReviewBrief")]
pub struct ReviewBrief {
    /// `<stream>/<review>`.
    pub key: String,
    /// What is to be reviewed, cut to [`SUMMARY_TEXT`] characters.
    pub subject: String,
    /// The issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The runtime asked, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
}

/// A handover nobody has taken, told briefly: the id `mesh.handover.consume` takes, who
/// published it and what it continues. Its body stays in the full view.
///
/// ```
/// use majordomus_cli::mesh::page::HandoverBrief;
///
/// let brief = HandoverBrief { id: "ab12".into(), runtime: "n-r".into(),
///     branch: Some("feature/x".into()), issue: None, published_lamport: 7 };
/// assert_eq!(serde_json::to_value(&brief).unwrap()["branch"], "feature/x");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshHandoverBrief")]
pub struct HandoverBrief {
    /// The content digest: what `mesh.handover.consume` takes.
    pub id: String,
    /// `<node>-<runtime>` of the publisher.
    pub runtime: String,
    /// The branch it continues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The issue it continues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The Lamport stamp of the first publication.
    pub published_lamport: u64,
}

/// The mesh in an answer a worker reads whole: counts, each runtime, the open reviews and
/// the untaken handovers, every list bounded by [`SUMMARY_LIST`] with the rest counted.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::fold;
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "claude-code") })
///     .unwrap();
/// for (claim, path) in [("c1", "apps/a"), ("c2", "apps/b")] {
///     j.append_own(EventBody::ClaimAcquired { claim: claim.into(), session: "s1".into(),
///         scope: vec![path.into()], intent: Some("work".into()), mode: ClaimMode::Exclusive,
///         issue: Some("#1".into()) }).unwrap();
/// }
/// j.append_own(EventBody::ClaimReleased { claim: "c1".into() }).unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// use majordomus_cli::mesh::page::{summarize, Summary};
///
/// let summary: Summary = summarize(&state, &[]);
/// assert_eq!((summary.claims.held, summary.claims.released), (1, 1));
/// assert_eq!(summary.digest, state.digest);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "MeshSummary")]
pub struct Summary {
    /// The whole fold's digest: equal digests, equal state.
    pub digest: String,
    /// Every session by standing.
    pub sessions: SessionCounts,
    /// Every claim by standing.
    pub claims: ClaimCounts,
    /// Advisory overlaps between live claims.
    pub overlaps: usize,
    /// Each runtime counted, the living first; at most [`SUMMARY_LIST`].
    pub runtimes: Vec<RuntimeSummary>,
    /// Runtimes past the list, counted.
    pub runtimes_not_listed: usize,
    /// Review requests nobody has answered; at most [`SUMMARY_LIST`].
    pub open_reviews: Vec<ReviewBrief>,
    /// Open reviews past the list, counted.
    pub open_reviews_not_listed: usize,
    /// Handovers nobody has consumed; at most [`SUMMARY_LIST`].
    pub untaken_handovers: Vec<HandoverBrief>,
    /// Untaken handovers past the list, counted.
    pub untaken_handovers_not_listed: usize,
}

fn rank(liveness: StreamLiveness) -> u8 {
    match liveness {
        StreamLiveness::Own => 0,
        StreamLiveness::Live => 1,
        StreamLiveness::Expired => 2,
    }
}

/// The summary of `state` (already narrowed), with each runtime's liveness and the
/// milliseconds since its beat rose from `streams`. A runtime that only `streams` names is
/// listed with no sessions; one the state names and `streams` does not is `expired`.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::fold;
///
/// let j = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000001",
///     "repo".into(), None).unwrap();
/// j.append_own(EventBody::SessionOpened { info: SessionInfo::named("s1", "claude-code") })
///     .unwrap();
/// for (claim, path) in [("c1", "apps/a"), ("c2", "apps/b")] {
///     j.append_own(EventBody::ClaimAcquired { claim: claim.into(), session: "s1".into(),
///         scope: vec![path.into()], intent: Some("work".into()), mode: ClaimMode::Exclusive,
///         issue: Some("#1".into()) }).unwrap();
/// }
/// j.append_own(EventBody::ClaimReleased { claim: "c1".into() }).unwrap();
/// let state = fold(&j.events(), &|_| StreamLiveness::Own);
/// use majordomus_cli::mesh::page::summarize;
///
/// let runtime = state.sessions[0].runtime.clone();
/// let summary = summarize(&state, &[(runtime, StreamLiveness::Own, Some(12))]);
/// assert_eq!(summary.runtimes[0].liveness, StreamLiveness::Own);
/// assert_eq!(summary.runtimes[0].last_beat_ms, Some(12));
/// ```
pub fn summarize(
    state: &CooperationState,
    streams: &[(String, StreamLiveness, Option<u64>)],
) -> Summary {
    let mut runtimes: BTreeMap<String, RuntimeSummary> = BTreeMap::new();
    let entry = |key: &str| -> RuntimeSummary {
        RuntimeSummary {
            runtime: key.to_string(),
            liveness: StreamLiveness::Expired,
            last_beat_ms: None,
            sessions: SessionCounts::default(),
            claims: ClaimCounts::default(),
            newest_claim: None,
        }
    };
    for (runtime, liveness, age) in streams {
        let node = runtimes
            .entry(runtime.clone())
            .or_insert_with(|| entry(runtime));
        if rank(*liveness) < rank(node.liveness) {
            node.liveness = *liveness;
        }
        if let Some(age) = age {
            node.last_beat_ms = Some(node.last_beat_ms.map_or(*age, |b| b.min(*age)));
        }
    }
    let mut summary = Summary {
        digest: state.digest.clone(),
        overlaps: state.overlaps.len(),
        ..Summary::default()
    };
    for session in &state.sessions {
        summary.sessions.add(session.state);
        runtimes
            .entry(session.runtime.clone())
            .or_insert_with(|| entry(&session.runtime))
            .sessions
            .add(session.state);
    }
    for claim in &state.claims {
        summary.claims.add(&claim.state);
        let node = runtimes
            .entry(claim.runtime.clone())
            .or_insert_with(|| entry(&claim.runtime));
        node.claims.add(&claim.state);
        let newer = node
            .newest_claim
            .as_ref()
            .is_none_or(|b| claim.acquired_lamport > b.acquired_lamport);
        if claim.state.is_live() && newer {
            node.newest_claim = Some(ClaimBrief {
                key: claim.key.clone(),
                session: claim.session.clone(),
                issue: claim.issue.clone(),
                intent: claim.intent.as_deref().map(cut),
                paths: claim.scope.len(),
                mode: claim.mode,
                acquired_lamport: claim.acquired_lamport,
            });
        }
    }
    // the living first, then by key: the one order every surface shows
    let ranked: BTreeMap<(u8, String), RuntimeSummary> = runtimes
        .into_values()
        .map(|r| ((rank(r.liveness), r.runtime.clone()), r))
        .collect();
    let all: Vec<RuntimeSummary> = ranked.into_values().collect();
    summary.runtimes_not_listed = all.len().saturating_sub(SUMMARY_LIST);
    summary.runtimes = all.into_iter().take(SUMMARY_LIST).collect();

    let open: Vec<ReviewBrief> = state
        .reviews
        .iter()
        .filter(|r| r.state == ReviewState::Open)
        .map(|r| ReviewBrief {
            key: r.key.clone(),
            subject: cut(&r.subject),
            issue: r.issue.clone(),
            reviewer: r.reviewer.clone(),
        })
        .collect();
    summary.open_reviews_not_listed = open.len().saturating_sub(SUMMARY_LIST);
    summary.open_reviews = open.into_iter().take(SUMMARY_LIST).collect();

    let untaken: Vec<HandoverBrief> = state
        .handovers
        .iter()
        .filter(|h| h.consumed_by.is_empty())
        .map(|h| HandoverBrief {
            id: h.id.clone(),
            runtime: h.runtime.clone(),
            branch: h.handover.branch.clone(),
            issue: h.handover.issue.clone(),
            published_lamport: h.published_lamport,
        })
        .collect();
    summary.untaken_handovers_not_listed = untaken.len().saturating_sub(SUMMARY_LIST);
    summary.untaken_handovers = untaken.into_iter().take(SUMMARY_LIST).collect();
    summary
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::mesh::identity::NodeIdentity;
    use crate::mesh::journal::{
        ClaimMode, EventBody, HandoverBody, Journal, SessionInfo, StreamLiveness,
    };
    use crate::mesh::state::fold;

    fn journal() -> Journal {
        Journal::open(
            Arc::new(NodeIdentity::ephemeral().unwrap()),
            "0000000000000001",
            "repo".into(),
            None,
        )
        .unwrap()
    }

    fn claim(j: &Journal, claim: &str, session: &str, intent: &str) {
        j.append_own(EventBody::ClaimAcquired {
            claim: claim.into(),
            session: session.into(),
            scope: vec![format!("apps/{claim}")],
            intent: Some(intent.into()),
            mode: ClaimMode::Exclusive,
            issue: Some(format!("#{claim}")),
        })
        .unwrap();
    }

    /// A mesh with history: sessions that came and went, claims taken and released, a
    /// review answered and one open, a handover taken and one not.
    fn history() -> (Journal, CooperationState) {
        let j = journal();
        for n in 0..40 {
            let session = format!("s{n}");
            j.append_own(EventBody::SessionOpened {
                info: SessionInfo::named(&session, "claude-code"),
            })
            .unwrap();
            claim(&j, &format!("c{n}"), &session, &"a long intent ".repeat(30));
            if n < 38 {
                j.append_own(EventBody::ClaimReleased {
                    claim: format!("c{n}"),
                })
                .unwrap();
                j.append_own(EventBody::SessionClosed {
                    session: session.clone(),
                })
                .unwrap();
            }
        }
        for (review, answered) in [("r1", true), ("r2", false)] {
            j.append_own(EventBody::ReviewRequested {
                review: review.into(),
                session: "s39".into(),
                subject: format!("feature/{review}"),
                scope: vec![],
                issue: None,
                reviewer: None,
            })
            .unwrap();
            if answered {
                let key = fold(&j.events(), &|_| StreamLiveness::Own)
                    .reviews
                    .iter()
                    .find(|r| r.subject.ends_with(review))
                    .unwrap()
                    .key
                    .clone();
                j.append_own(EventBody::ReviewAnswered {
                    request: key,
                    session: "s38".into(),
                    verdict: "approved".into(),
                    note: None,
                })
                .unwrap();
            }
        }
        for (n, taken) in [(1, true), (2, false)] {
            let body = format!("# Objective\nhandover {n}\n");
            let handover = HandoverBody {
                id: HandoverBody::digest_of(&body),
                task: None,
                issue: Some(format!("I{n}")),
                milestone: None,
                branch: Some(format!("feature/h{n}")),
                head: None,
                created_at: None,
                name: None,
                body,
            };
            let id = handover.id.clone();
            j.append_own(EventBody::HandoverPublished { handover })
                .unwrap();
            if taken {
                j.append_own(EventBody::HandoverConsumed {
                    handover: id,
                    session: "s39".into(),
                })
                .unwrap();
            }
        }
        let state = fold(&j.events(), &|_| StreamLiveness::Own);
        (j, state)
    }

    fn streams(state: &CooperationState) -> Vec<(String, StreamLiveness, Option<u64>)> {
        vec![(
            state.sessions[0].runtime.clone(),
            StreamLiveness::Own,
            Some(5),
        )]
    }

    #[test]
    fn the_summary_counts_the_history_and_lists_only_what_is_live_or_waiting() {
        let (_j, state) = history();
        let summary = summarize(&state, &streams(&state));
        assert_eq!(
            summary.digest, state.digest,
            "the whole fold's digest, not a page's"
        );
        assert_eq!(
            summary.sessions,
            SessionCounts {
                active: 2,
                closed: 38,
                expired: 0
            }
        );
        assert_eq!((summary.claims.held, summary.claims.released), (2, 38));
        assert_eq!(summary.runtimes.len(), 1);
        let runtime = &summary.runtimes[0];
        assert_eq!(runtime.liveness, StreamLiveness::Own);
        assert_eq!(runtime.last_beat_ms, Some(5));
        let newest = runtime.newest_claim.as_ref().expect("a live claim");
        assert_eq!(
            newest.issue.as_deref(),
            Some("#c39"),
            "the newest live claim"
        );
        assert!(
            newest.intent.as_ref().unwrap().ends_with('…'),
            "the intent is cut"
        );
        assert_eq!(summary.open_reviews.len(), 1);
        assert_eq!(summary.open_reviews[0].subject, "feature/r2");
        assert_eq!(summary.untaken_handovers.len(), 1);
        assert_eq!(
            summary.untaken_handovers[0].branch.as_deref(),
            Some("feature/h2")
        );
    }

    #[test]
    fn the_summary_stays_small_however_long_the_history() {
        let (_j, state) = history();
        let full = serde_json::to_string(&state).unwrap().len();
        let summary = serde_json::to_string(&summarize(&state, &streams(&state)))
            .unwrap()
            .len();
        assert!(summary < 16 * 1024, "{summary} bytes");
        assert!(
            summary * 10 < full,
            "summary {summary} against the full {full}"
        );
    }

    #[test]
    fn a_list_past_its_bound_is_counted_not_dropped() {
        let (_j, mut state) = history();
        let template = state.reviews[0].clone();
        state.reviews = (0..SUMMARY_LIST + 3)
            .map(|n| {
                let mut r = template.clone();
                r.key = format!("{}-{n}", template.key);
                r.state = ReviewState::Open;
                r
            })
            .collect();
        let summary = summarize(&state, &[]);
        assert_eq!(summary.open_reviews.len(), SUMMARY_LIST);
        assert_eq!(summary.open_reviews_not_listed, 3);
    }

    #[test]
    fn many_runtimes_are_listed_living_first_and_the_rest_counted() {
        let (_j, state) = history();
        let own = state.sessions[0].runtime.clone();
        let mut streams: Vec<_> = (0..SUMMARY_LIST + 2)
            .map(|n| (format!("{n:0>49}"), StreamLiveness::Expired, None))
            .collect();
        streams.push((own.clone(), StreamLiveness::Live, Some(9)));
        streams.push((own.clone(), StreamLiveness::Own, Some(3)));
        let summary = summarize(&state, &streams);
        assert_eq!(summary.runtimes[0].runtime, own, "the living first");
        assert_eq!(
            summary.runtimes[0].liveness,
            StreamLiveness::Own,
            "the best verdict wins"
        );
        assert_eq!(
            summary.runtimes[0].last_beat_ms,
            Some(3),
            "and the freshest beat"
        );
        assert_eq!(summary.runtimes.len(), SUMMARY_LIST);
        assert_eq!(summary.runtimes_not_listed, 3);
    }

    #[test]
    fn live_claims_only_drops_the_ended_ones_and_their_overlaps_follow() {
        let (_j, state) = history();
        let live = narrow(
            &state,
            &Query {
                claims: Some(ClaimFilter::Live),
                ..Query::default()
            },
        );
        assert_eq!(live.claims.len(), 2);
        assert!(live.claims.iter().all(|c| c.state.is_live()));
        assert_eq!(
            live.sessions.len(),
            state.sessions.len(),
            "sessions are not claims"
        );
        assert_eq!(live.digest, state.digest);
        assert!(live.overlaps.iter().all(|o| live
            .claims
            .iter()
            .any(|c| c.key == o.first || c.key == o.second)));
    }

    #[test]
    fn a_session_is_asked_for_by_its_key_or_its_bare_id() {
        let (_j, state) = history();
        let by_id = narrow(
            &state,
            &Query {
                session: Some("s39".into()),
                ..Query::default()
            },
        );
        assert_eq!(by_id.sessions.len(), 1);
        assert_eq!(by_id.claims.len(), 1);
        assert_eq!(by_id.reviews.len(), 2, "both requests it opened");
        assert_eq!(by_id.handovers.len(), 1, "the one it took");
        let key = by_id.sessions[0].key.clone();
        let by_key = narrow(
            &state,
            &Query {
                session: Some(key),
                ..Query::default()
            },
        );
        assert_eq!(by_key, by_id);
        let nobody = narrow(
            &state,
            &Query {
                session: Some("s99".into()),
                ..Query::default()
            },
        );
        assert!(nobody.sessions.is_empty() && nobody.claims.is_empty());
    }

    #[test]
    fn a_runtime_is_asked_for_by_its_key_or_its_node() {
        let (_j, state) = history();
        let runtime = state.sessions[0].runtime.clone();
        let node = runtime.split('-').next().unwrap().to_string();
        let mine = narrow(
            &state,
            &Query {
                runtime: Some(runtime),
                ..Query::default()
            },
        );
        assert_eq!(mine.sessions.len(), state.sessions.len());
        let machine = narrow(
            &state,
            &Query {
                runtime: Some(node),
                ..Query::default()
            },
        );
        assert_eq!(machine, mine);
        let elsewhere = narrow(
            &state,
            &Query {
                runtime: Some("f".repeat(49)),
                ..Query::default()
            },
        );
        assert!(elsewhere.sessions.is_empty() && elsewhere.reviews.is_empty());
        assert!(elsewhere.handovers.is_empty());
    }

    #[test]
    fn after_keeps_only_what_moved_since_the_stamp() {
        let (_j, state) = history();
        let stamp = state
            .claims
            .iter()
            .map(|c| c.acquired_lamport)
            .max()
            .unwrap();
        let later = narrow(
            &state,
            &Query {
                after: Some(stamp - 1),
                ..Query::default()
            },
        );
        assert_eq!(later.claims.len(), 1, "only the newest acquisition");
        assert!(later.sessions.iter().all(|s| s.updated_lamport > stamp - 1));
        assert!(later
            .reviews
            .iter()
            .all(|r| later.sessions.iter().any(|s| s.key == r.session)));
        let nothing = narrow(
            &state,
            &Query {
                after: Some(u64::MAX),
                ..Query::default()
            },
        );
        assert!(nothing.sessions.is_empty() && nothing.claims.is_empty());
        assert!(nothing.handovers.is_empty() && nothing.reviews.is_empty());
    }

    #[test]
    fn a_kind_answers_one_collection_and_a_limit_the_newest_of_each() {
        let (_j, state) = history();
        let claims = narrow(
            &state,
            &Query {
                kind: Some(Kind::Claims),
                ..Query::default()
            },
        );
        assert_eq!(claims.claims.len(), state.claims.len());
        assert!(claims.sessions.is_empty() && claims.reviews.is_empty());
        assert!(claims.handovers.is_empty() && claims.overlaps.is_empty());
        assert_eq!(claims.digest, state.digest);

        let three = narrow(
            &state,
            &Query {
                limit: Some(3),
                ..Query::default()
            },
        );
        assert_eq!(three.claims.len(), 3);
        let newest = state
            .claims
            .iter()
            .map(|c| c.acquired_lamport)
            .max()
            .unwrap();
        assert!(three.claims.iter().any(|c| c.acquired_lamport == newest));
        let in_fold: Vec<_> = state
            .claims
            .iter()
            .filter(|c| three.claims.contains(c))
            .cloned()
            .collect();
        assert_eq!(three.claims, in_fold, "kept in the fold's own order");
        assert!(three.sessions.len() <= 3 && three.reviews.len() <= 3);
        let none = narrow(
            &state,
            &Query {
                limit: Some(0),
                ..Query::default()
            },
        );
        assert!(none.claims.is_empty() && none.sessions.is_empty());
        let all = narrow(
            &state,
            &Query {
                limit: Some(10_000),
                ..Query::default()
            },
        );
        assert_eq!(all, state);
    }

    #[test]
    fn a_limit_keeps_ties_in_order_until_it_is_reached() {
        let items = vec![("a", 5), ("b", 7), ("c", 7), ("d", 1)];
        assert_eq!(newest(items.clone(), Some(1), |i| i.1), [("b", 7)]);
        assert_eq!(
            newest(items.clone(), Some(2), |i| i.1),
            [("b", 7), ("c", 7)]
        );
        assert_eq!(
            newest(items.clone(), Some(3), |i| i.1),
            [("a", 5), ("b", 7), ("c", 7)]
        );
        assert_eq!(newest(items.clone(), None, |i| i.1), items);
    }

    #[test]
    fn open_keeps_only_what_is_still_open_in_every_collection() {
        let (_j, state) = history();
        let open = narrow(
            &state,
            &Query {
                open: Some(true),
                ..Query::default()
            },
        );
        assert_eq!(open.sessions.len(), 2);
        assert!(open
            .sessions
            .iter()
            .all(|s| s.state == SessionState::Active));
        assert_eq!(open.claims.len(), 2);
        assert_eq!(open.reviews.len(), 1);
        assert_eq!(open.reviews[0].subject, "feature/r2");
        assert_eq!(open.handovers.len(), 1);
        assert_eq!(
            open.handovers[0].handover.branch.as_deref(),
            Some("feature/h2")
        );
        let closed = narrow(
            &state,
            &Query {
                open: Some(false),
                ..Query::default()
            },
        );
        assert_eq!(closed, state, "open=false asks for nothing narrower");
    }

    #[test]
    fn matching_finds_a_path_a_branch_an_issue_or_a_subject_ignoring_case() {
        let (_j, state) = history();
        let path = narrow(
            &state,
            &Query {
                matching: Some("APPS/C7".into()),
                ..Query::default()
            },
        );
        assert_eq!(path.claims.len(), 1, "apps/c7 and nothing else");
        assert_eq!(path.claims[0].scope, ["apps/c7"]);
        assert!(path.handovers.is_empty() && path.reviews.is_empty());
        let issue = narrow(
            &state,
            &Query {
                matching: Some("#c39".into()),
                ..Query::default()
            },
        );
        assert_eq!(issue.claims.len(), 1);
        let branch = narrow(
            &state,
            &Query {
                matching: Some("feature/h2".into()),
                ..Query::default()
            },
        );
        assert_eq!(branch.handovers.len(), 1);
        let subject = narrow(
            &state,
            &Query {
                matching: Some("feature/r1".into()),
                ..Query::default()
            },
        );
        assert_eq!(subject.reviews.len(), 1);
        let session = narrow(
            &state,
            &Query {
                matching: Some("/s39".into()),
                ..Query::default()
            },
        );
        assert_eq!(session.sessions.len(), 1, "a session's key is text too");
        let nothing = narrow(
            &state,
            &Query {
                matching: Some("no such".into()),
                ..Query::default()
            },
        );
        assert!(nothing.claims.is_empty() && nothing.sessions.is_empty());
        assert_eq!(nothing.digest, state.digest);
    }

    #[test]
    fn twenty_thousand_events_summarise_under_sixteen_kilobytes() {
        // the acceptance of I2414: a journal of 20 000 events, the default answer under 16 kB
        let j = journal();
        let mut events = 0;
        let mut n = 0;
        while events < 20_000 {
            let session = format!("s{n}");
            j.append_own(EventBody::SessionOpened {
                info: SessionInfo::named(&session, "claude-code"),
            })
            .unwrap();
            claim(&j, &format!("c{n}"), &session, &"an intent ".repeat(40));
            j.append_own(EventBody::ClaimReleased {
                claim: format!("c{n}"),
            })
            .unwrap();
            j.append_own(EventBody::SessionClosed { session }).unwrap();
            events += 4;
            n += 1;
        }
        let state = fold(&j.events(), &|_| StreamLiveness::Own);
        assert_eq!(state.claims.len(), n);
        let summary = serde_json::to_string(&summarize(&state, &streams(&state))).unwrap();
        assert!(summary.len() < 16 * 1024, "{} bytes", summary.len());
        let full = serde_json::to_string(&state).unwrap();
        assert!(
            full.len() > 1024 * 1024,
            "the whole is what the summary spares: {}",
            full.len()
        );
    }

    #[test]
    fn a_text_is_cut_on_a_character_and_says_so() {
        assert_eq!(cut("short"), "short");
        let long = "č".repeat(SUMMARY_TEXT + 1);
        let cut = cut(&long);
        assert!(cut.ends_with('…'));
        assert_eq!(cut.chars().count(), SUMMARY_TEXT + 1);
    }

    #[test]
    fn the_view_defaults_to_the_summary() {
        assert_eq!(Query::default().view(), View::Summary);
        let full: Query = serde_json::from_str(r#"{"view":"full","claims":"live"}"#).unwrap();
        assert_eq!(full.view(), View::Full);
        assert!(serde_json::from_str::<Query>(r#"{"bogus":1}"#).is_err());
    }
}
