//! The mesh as work, for a session that is starting: who works where on every machine, the
//! handovers waiting to be taken and the reviews waiting to be answered (I2285, ADR 0128).
//!
//! The start of a session printed one line about the mesh — its health — and an agent that
//! wanted to know who else was working, on this machine or another, had to remember to ask.
//! It mostly did not, which is how two sessions built the same thing in one afternoon. The
//! briefing is the fold read from the starting session's side: it is computed from
//! [`super::state::CooperationState`] alone, so the command line, the MCP `initialize`
//! instructions, the start hook and the Cockpit say the same thing, and it is bounded, so it
//! costs a starting session a few lines whatever the mesh holds.
//!
//! ```
//! use majordomus_cli::mesh::briefing::{brief, render_text};
//! use majordomus_cli::mesh::state::CooperationState;
//! // a mesh with nobody in it briefs nothing, and says so in one line
//! let empty = brief(&CooperationState::default(), "aa-01", 5);
//! assert!(empty.machines.is_empty() && empty.handovers.is_empty() && empty.reviews.is_empty());
//! assert_eq!(render_text(&empty), "nobody else works in this repository's mesh; nothing waits for this machine");
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::state::{ClaimState, CooperationState, ReviewState, SessionState};

/// One live session another worker holds, with what it said and the scope it claims.
///
/// ```
/// use majordomus_cli::mesh::briefing::MeshBriefingSession;
/// let s = MeshBriefingSession { key: "n-r-s/board-p2".into(), client: "codex".into(),
///     worker: None, intent: Some("the release".into()), branch: None, issue: None,
///     claims: vec!["docs".into()] };
/// assert_eq!(serde_json::to_value(&s).unwrap()["client"], "codex");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshBriefingSession {
    /// `<stream>/<session>`.
    pub key: String,
    /// The client it runs in.
    pub client: String,
    /// The worker's name for itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    /// What it said it is doing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The branch it works on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The issue it works on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The paths its live claims name, exclusive ones marked `!`.
    pub claims: Vec<String>,
}

/// The live sessions of one machine.
///
/// ```
/// use majordomus_cli::mesh::briefing::MeshBriefingMachine;
/// let m = MeshBriefingMachine { node: "641bdb94".into(), this_machine: true, sessions: vec![], more: 0 };
/// assert!(m.this_machine);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshBriefingMachine {
    /// The node id.
    pub node: String,
    /// Whether it is the machine this runtime runs on.
    pub this_machine: bool,
    /// Its live sessions, at most the briefing's limit.
    pub sessions: Vec<MeshBriefingSession>,
    /// How many live sessions the limit left out.
    pub more: usize,
}

/// A handover another runtime published that no session of this runtime has taken.
///
/// ```
/// use majordomus_cli::mesh::briefing::MeshBriefingHandover;
/// let h = MeshBriefingHandover { id: "ab12".into(), runtime: "n-r".into(), task: None,
///     issue: Some("I1".into()), branch: None, objective: "ship it".into() };
/// assert_eq!(h.objective, "ship it");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshBriefingHandover {
    /// The handover's id, which `mesh.handover.consume` takes.
    pub id: String,
    /// The runtime that published it.
    pub runtime: String,
    /// Its task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Its issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// Its branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The first line of its `# Objective`.
    pub objective: String,
}

/// A review another runtime asked for and nobody has answered.
///
/// ```
/// use majordomus_cli::mesh::briefing::MeshBriefingReview;
/// let r = MeshBriefingReview { key: "n-r-s/r-1".into(), subject: "PR #1".into(),
///     issue: None, addressed_here: true };
/// assert!(r.addressed_here);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshBriefingReview {
    /// The request's key, which `mesh.review.answer` takes.
    pub key: String,
    /// What to review.
    pub subject: String,
    /// Its issue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// Whether it names this runtime as the reviewer.
    pub addressed_here: bool,
}

/// What a starting session is told about the mesh.
///
/// ```
/// use majordomus_cli::mesh::briefing::MeshBriefing;
/// let b = MeshBriefing::default();
/// assert_eq!(b.handovers_more + b.reviews_more, 0);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshBriefing {
    /// This runtime, `<node>-<runtime>`.
    pub runtime: String,
    /// Every machine with a live session, this one first.
    pub machines: Vec<MeshBriefingMachine>,
    /// Handovers waiting to be taken here, newest first.
    pub handovers: Vec<MeshBriefingHandover>,
    /// How many the limit left out.
    pub handovers_more: usize,
    /// Open reviews of other runtimes, those addressed here first.
    pub reviews: Vec<MeshBriefingReview>,
    /// How many the limit left out.
    pub reviews_more: usize,
}

/// The briefing of the runtime `runtime` (`<node>-<runtime>`), at most `limit` entries in each
/// list.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::briefing::brief;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::journal::{ClaimMode, EventBody, Journal, SessionInfo, StreamLiveness};
/// use majordomus_cli::mesh::state::fold;
///
/// let elsewhere = Journal::open(Arc::new(NodeIdentity::ephemeral().unwrap()), "0000000000000002",
///     "repo".into(), None).unwrap();
/// let mut info = SessionInfo::named("s1", "codex");
/// info.intent = Some("the release".into());
/// elsewhere.append_own(EventBody::SessionOpened { info }).unwrap();
/// elsewhere.append_own(EventBody::ClaimAcquired { claim: "c1".into(),
///     session: "s1".into(), scope: vec!["docs".into()], intent: None,
///     mode: ClaimMode::Exclusive, issue: None }).unwrap();
/// let state = fold(&elsewhere.events(), &|_| StreamLiveness::Live);
/// let b = brief(&state, "ffff-0000000000000001", 5);
/// assert_eq!(b.machines.len(), 1);
/// assert!(!b.machines[0].this_machine, "the session is on another machine");
/// assert_eq!(b.machines[0].sessions[0].claims, vec!["!docs".to_string()]);
/// ```
pub fn brief(state: &CooperationState, runtime: &str, limit: usize) -> MeshBriefing {
    let node = runtime.split('-').next().unwrap_or_default();
    let mut machines: BTreeMap<(bool, String), Vec<MeshBriefingSession>> = BTreeMap::new();
    for session in state
        .sessions
        .iter()
        .filter(|s| s.state == SessionState::Active)
    {
        let claims: Vec<String> = state
            .claims
            .iter()
            .filter(|c| c.session == session.key && c.state.is_live())
            .flat_map(|c| {
                let exclusive = matches!(c.state, ClaimState::Held | ClaimState::Conflicted(_))
                    && c.mode == super::journal::ClaimMode::Exclusive;
                c.scope.iter().map(move |p| {
                    if exclusive {
                        format!("!{p}")
                    } else {
                        p.clone()
                    }
                })
            })
            .collect();
        machines
            .entry((session.node != node, session.node.clone()))
            .or_default()
            .push(MeshBriefingSession {
                key: session.key.clone(),
                client: session.info.client.clone(),
                worker: session.info.worker.clone(),
                intent: session.info.intent.clone(),
                branch: session.info.branch.clone(),
                issue: session.info.issue.clone(),
                claims,
            });
    }
    let machines = machines
        .into_iter()
        .map(|((remote, node), mut sessions)| {
            sessions.sort_by(|a, b| a.key.cmp(&b.key));
            let more = sessions.len().saturating_sub(limit);
            sessions.truncate(limit);
            MeshBriefingMachine {
                node,
                this_machine: !remote,
                sessions,
                more,
            }
        })
        .collect();

    let mut handovers: Vec<(u64, MeshBriefingHandover)> = state
        .handovers
        .iter()
        .filter(|h| h.runtime != runtime)
        .filter(|h| !h.consumed_by.iter().any(|c| c.starts_with(runtime)))
        .map(|h| {
            let objective = objective_of(&h.handover.body);
            (
                h.published_lamport,
                MeshBriefingHandover {
                    id: h.id.clone(),
                    runtime: h.runtime.clone(),
                    task: h.handover.task.clone(),
                    issue: h.handover.issue.clone(),
                    branch: h.handover.branch.clone(),
                    objective,
                },
            )
        })
        .collect();
    handovers.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let handovers_more = handovers.len().saturating_sub(limit);
    let handovers = handovers.into_iter().take(limit).map(|(_, h)| h).collect();

    let mut reviews: Vec<MeshBriefingReview> = state
        .reviews
        .iter()
        .filter(|r| r.state == ReviewState::Open && !r.session.starts_with(runtime))
        .map(|r| MeshBriefingReview {
            key: r.key.clone(),
            subject: r.subject.clone(),
            issue: r.issue.clone(),
            addressed_here: r.reviewer.as_deref() == Some(runtime),
        })
        .collect();
    reviews.sort_by(|a, b| {
        b.addressed_here
            .cmp(&a.addressed_here)
            .then_with(|| a.key.cmp(&b.key))
    });
    let reviews_more = reviews.len().saturating_sub(limit);
    reviews.truncate(limit);

    MeshBriefing {
        runtime: runtime.to_string(),
        machines,
        handovers,
        handovers_more,
        reviews,
        reviews_more,
    }
}

/// The first non-empty line under `# Objective`, or the first line of the body.
fn objective_of(body: &str) -> String {
    let mut lines = body.lines().map(str::trim);
    let after = lines
        .by_ref()
        .skip_while(|l| !l.eq_ignore_ascii_case("# objective"))
        .skip(1)
        .find(|l| !l.is_empty());
    let line = after
        .or_else(|| {
            body.lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))
        })
        .unwrap_or_default();
    line.chars().take(160).collect()
}

/// The briefing as one paragraph for a client's first read — the MCP `initialize`
/// instructions (I2287): how many work where, what waits here, and the four calls of the
/// protocol. Bounded whatever the mesh holds.
///
/// ```
/// use majordomus_cli::mesh::briefing::{summarize, MeshBriefing, MeshBriefingMachine};
/// let mut b = MeshBriefing::default();
/// b.machines.push(MeshBriefingMachine { node: "5d81".into(), this_machine: false,
///     sessions: vec![], more: 2 });
/// let text = summarize(&b);
/// assert!(text.contains("2 live session(s) on 1 other machine(s)"));
/// assert!(text.contains("majordomus_mesh_claim"));
/// assert!(text.contains("majordomus_mesh_briefing"));
/// ```
pub fn summarize(b: &MeshBriefing) -> String {
    let count = |m: &MeshBriefingMachine| m.sessions.len() + m.more;
    let here: usize = b
        .machines
        .iter()
        .filter(|m| m.this_machine)
        .map(count)
        .sum();
    let remote: Vec<&MeshBriefingMachine> = b.machines.iter().filter(|m| !m.this_machine).collect();
    let elsewhere: usize = remote.iter().map(|m| count(m)).sum();
    let handovers = b.handovers.len() + b.handovers_more;
    let reviews = b.reviews.len() + b.reviews_more;
    let addressed = b.reviews.iter().filter(|r| r.addressed_here).count();
    let mut text = format!(
        " This repository's mesh links its runtimes on every machine: {here} live session(s) on this machine and {elsewhere} live session(s) on {} other machine(s); {handovers} handover(s) wait to be taken here and {reviews} review(s) are open",
        remote.len()
    );
    if addressed > 0 {
        text.push_str(&format!(", {addressed} of them asked of this machine"));
    }
    text.push_str(". Before you build, claim the paths you will change with majordomus_mesh_claim (exclusive; a refusal names who holds them on any machine); take a handover meant for you with majordomus_mesh_handover_consume; answer a review with majordomus_mesh_review_answer; majordomus_mesh_briefing says who works where and what waits here (ADR 0128).");
    text
}

/// The briefing as the lines a starting session reads: one per machine with its sessions,
/// then the handovers and reviews waiting here, each with the call that acts on it.
///
/// ```
/// use majordomus_cli::mesh::briefing::{render_text, MeshBriefing, MeshBriefingReview};
/// let mut b = MeshBriefing::default();
/// b.reviews.push(MeshBriefingReview { key: "n-r-s/r-1".into(), subject: "PR #9".into(),
///     issue: None, addressed_here: true });
/// let text = render_text(&b);
/// assert!(text.contains("review asked of this machine: PR #9"));
/// assert!(text.contains("majordomus_mesh_review_answer"));
/// ```
pub fn render_text(b: &MeshBriefing) -> String {
    if b.machines.is_empty() && b.handovers.is_empty() && b.reviews.is_empty() {
        return "nobody else works in this repository's mesh; nothing waits for this machine"
            .into();
    }
    let mut out = Vec::new();
    for m in &b.machines {
        let place = if m.this_machine {
            format!("this machine ({})", m.node)
        } else {
            format!("machine {}", m.node)
        };
        let mut line = format!("{place}: {} session(s)", m.sessions.len() + m.more);
        for s in &m.sessions {
            let what = s.intent.as_deref().unwrap_or("(no intent announced)");
            let what: String = what.chars().take(90).collect();
            let scope = if s.claims.is_empty() {
                "no claim".to_string()
            } else {
                s.claims
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            line.push_str(&format!("\n  - {} {}: {what} [{scope}]", s.client, s.key));
        }
        if m.more > 0 {
            line.push_str(&format!("\n  - and {} more", m.more));
        }
        out.push(line);
    }
    for h in &b.handovers {
        out.push(format!(
            "handover waiting ({}): {} — take it with majordomus_mesh_handover_consume {}",
            h.issue.as_deref().unwrap_or("no issue"),
            h.objective,
            h.id
        ));
    }
    if b.handovers_more > 0 {
        out.push(format!("and {} more handover(s)", b.handovers_more));
    }
    for r in &b.reviews {
        let whom = if r.addressed_here {
            "review asked of this machine"
        } else {
            "open review"
        };
        out.push(format!(
            "{whom}: {} — answer with majordomus_mesh_review_answer {}",
            r.subject, r.key
        ));
    }
    if b.reviews_more > 0 {
        out.push(format!("and {} more review(s)", b.reviews_more));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::identity::NodeIdentity;
    use crate::mesh::journal::{
        ClaimMode, EventBody, HandoverBody, Journal, SessionInfo, StreamLiveness,
    };
    use crate::mesh::state::fold;
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

    #[test]
    fn a_handover_taken_here_or_published_here_is_not_waiting() {
        let there = journal("0000000000000002");
        let body = "# Objective\nland the pack\n\n# Current State\nx\n".to_string();
        let handover = HandoverBody {
            id: HandoverBody::digest_of(&body),
            task: None,
            issue: Some("I1".into()),
            milestone: None,
            branch: None,
            head: None,
            created_at: None,
            name: None,
            body,
        };
        there
            .append_own(EventBody::HandoverPublished {
                handover: handover.clone(),
            })
            .unwrap();
        let here = journal("0000000000000001");
        here.ingest(&there.events(), &|_| Ok(()));
        let me = here.own_stream().runtime_key();
        let state = fold(&here.events(), &|_| StreamLiveness::Live);
        let waiting = brief(&state, &me, 5);
        assert_eq!(waiting.handovers.len(), 1);
        assert_eq!(waiting.handovers[0].objective, "land the pack");
        // taken by a session of this runtime: no longer waiting here
        here.append_own(EventBody::HandoverConsumed {
            handover: handover.id.clone(),
            session: "s1".into(),
        })
        .unwrap();
        let state = fold(&here.events(), &|_| StreamLiveness::Live);
        assert!(brief(&state, &me, 5).handovers.is_empty());
        // and the publisher is never told its own handover waits for it
        let theirs = fold(&there.events(), &|_| StreamLiveness::Live);
        assert!(brief(&theirs, &there.own_stream().runtime_key(), 5)
            .handovers
            .is_empty());
    }

    #[test]
    fn the_limit_bounds_every_list_and_says_how_much_it_left_out() {
        let there = journal("0000000000000002");
        for n in 0..7 {
            there
                .append_own(EventBody::SessionOpened {
                    info: SessionInfo::named(&format!("s{n}"), "cli"),
                })
                .unwrap();
            there
                .append_own(EventBody::ClaimAcquired {
                    claim: format!("c{n}"),
                    session: format!("s{n}"),
                    scope: vec![format!("p{n}")],
                    intent: None,
                    mode: ClaimMode::Advisory,
                    issue: None,
                })
                .unwrap();
        }
        let state = fold(&there.events(), &|_| StreamLiveness::Live);
        let b = brief(&state, "ffff-0000000000000001", 3);
        assert_eq!(b.machines[0].sessions.len(), 3);
        assert_eq!(b.machines[0].more, 4);
        assert!(render_text(&b).contains("7 session(s)"));
        assert!(render_text(&b).contains("and 4 more"));
    }

    #[test]
    fn a_closed_session_is_not_briefed() {
        let there = journal("0000000000000002");
        there
            .append_own(EventBody::SessionOpened {
                info: SessionInfo::named("s1", "cli"),
            })
            .unwrap();
        there
            .append_own(EventBody::SessionClosed {
                session: "s1".into(),
            })
            .unwrap();
        let state = fold(&there.events(), &|_| StreamLiveness::Live);
        assert!(brief(&state, "ffff-0000000000000001", 5)
            .machines
            .is_empty());
    }
}
