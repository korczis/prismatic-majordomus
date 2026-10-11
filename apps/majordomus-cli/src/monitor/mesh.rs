//! The mesh journal, folded into signals.
//!
//! The journal is read whole and in causal order, because what an event means depends on
//! what came before it: a session's opening is news only when the session is new or what it
//! says about itself changed (the server re-projects every attached board peer on each
//! heartbeat, and those re-projections are most of the journal); a claim is news only when
//! its session did not already hold the same one; an answer is for the reader when the
//! request was theirs. Events up to the cursor only build that memory; events after it
//! become signals or are counted as suppressed.

use std::collections::{HashMap, HashSet};

use crate::mesh::journal::{ClaimMode, EventBody, HandoverBody, MeshEvent, SessionInfo};

use super::{MonitorSeverity, MonitorSignal, MonitorSource, Reader, SourceRead};

/// A place in the journal: an event's causal key, unique and totally ordered.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Position {
    pub(crate) lamport: u64,
    pub(crate) stream: String,
    pub(crate) seq: u64,
}

impl Position {
    pub(crate) fn of(event: &MeshEvent) -> Position {
        Position {
            lamport: event.lamport,
            stream: event.stream.as_str().to_string(),
            seq: event.seq,
        }
    }
}

/// What the fold needs to know beyond the events: who the reader is, which machine a
/// node id is, and the oldest time a fresh page reaches back to.
pub(crate) struct Context<'a> {
    pub(crate) reader: &'a Reader,
    /// Display names of nodes, by node id.
    pub(crate) machines: &'a HashMap<String, String>,
    /// Without a cursor, events older than this (Unix seconds) only build memory.
    pub(crate) since: u64,
}

/// What makes two openings of one session the same news.
type Fingerprint = [Option<String>; 7];

/// What a session last said about itself that makes a change in it news. The head, the
/// dirtiness and the context revision move on every commit and are not news.
fn fingerprint(info: &SessionInfo) -> Fingerprint {
    [
        Some(info.client.clone()),
        info.worker.clone(),
        info.intent.clone(),
        info.task.clone(),
        info.issue.clone(),
        info.milestone.clone(),
        info.branch.clone(),
    ]
}

#[derive(Default)]
struct Memory {
    /// Each session's fingerprint and how to call it, by (stream, session).
    sessions: HashMap<(String, String), (Fingerprint, String)>,
    /// The sessions whose arrival was shown, so that their departure is too.
    shown: HashSet<(String, String)>,
    /// Live claims by id: (stream, session, what makes it the same claim).
    claims: HashMap<String, (String, String, String)>,
    /// Review requests by `<stream>/<review>`: the requester and the subject, and whether
    /// it was the reader's.
    reviews: HashMap<String, (String, String, bool)>,
    /// Handovers by content digest: the branch.
    handovers: HashMap<String, String>,
    /// Review requests the reader was asked for or answered, by `<stream>/<review>`: a
    /// withdrawal of one of them is the reader's news.
    involved: HashSet<String>,
}

/// Fold `events` (in causal order) after `after` into signals.
pub(crate) fn fold(
    events: &[MeshEvent],
    after: Option<&Position>,
    ctx: &Context<'_>,
) -> SourceRead<Position> {
    let mut memory = Memory::default();
    let mut read = SourceRead::default();
    for event in events {
        let position = Position::of(event);
        let news = match after {
            Some(after) => position > *after,
            None => event.at >= ctx.since,
        };
        let signal = memory.absorb(event, ctx);
        if news {
            match signal {
                Some(signal) => read.signals.push((position.clone(), signal)),
                None => read.suppressed += 1,
            }
            read.high = Some(position);
        } else if after.is_none() {
            // a fresh page stands after everything, even what it did not show
            read.high = Some(position);
        }
    }
    read
}

impl Memory {
    /// Remember what `event` says and return the signal it is, if it is news.
    fn absorb(&mut self, event: &MeshEvent, ctx: &Context<'_>) -> Option<MonitorSignal> {
        let stream = event.stream.as_str().to_string();
        let node = stream.split('-').next().unwrap_or_default().to_string();
        let machine = ctx
            .machines
            .get(&node)
            .cloned()
            .unwrap_or_else(|| node.chars().take(8).collect());
        // a kind newer than this executable's EventBody (ADR 0133 meets I2415): read raw
        if event.kind() == "review_withdrawn" {
            let mut s = self.withdrawn(event, &stream, ctx)?;
            s.machine = Some(machine);
            return Some(s);
        }
        let body = event.body()?;
        let mut signal = match body {
            EventBody::SessionOpened { info } => {
                let key = (stream.clone(), info.session.clone());
                let print = fingerprint(&info);
                let name = who(&info);
                let previous = self
                    .sessions
                    .insert(key.clone(), (print.clone(), name.clone()));
                if previous.is_some_and(|(old, _)| old == print) {
                    return None;
                }
                // a session that has said nothing about itself is a connection, not news:
                // its arrival is shown when it first says what it does, and its departure
                // only if its arrival was
                let said = info.intent.as_deref().is_some_and(|i| !i.trim().is_empty());
                if !said {
                    return None;
                }
                let (kind, verb) = if self.shown.insert(key) {
                    ("session_joined", "joined")
                } else {
                    ("session_changed", "now")
                };
                let intent = info.intent.clone().unwrap_or_default();
                let mut s = MonitorSignal::new(
                    event.at,
                    MonitorSource::Mesh,
                    kind,
                    MonitorSeverity::Info,
                    &format!("{name} {verb}: {intent}"),
                );
                s.session = Some(info.session.clone());
                s.actor = Some(name);
                s.branch = info.branch.clone();
                s.issue = info.issue.clone();
                s.for_you = ctx.reader.is(&info.session);
                s
            }
            EventBody::SessionClosed { session } => {
                let key = (stream.clone(), session.clone());
                let name = self
                    .sessions
                    .remove(&key)
                    .map(|(_, name)| name)
                    .unwrap_or_else(|| session.clone());
                if !self.shown.remove(&key) {
                    return None;
                }
                let mut s = MonitorSignal::new(
                    event.at,
                    MonitorSource::Mesh,
                    "session_left",
                    MonitorSeverity::Info,
                    &format!("{name} left"),
                );
                s.session = Some(session);
                s.actor = Some(name);
                s
            }
            EventBody::ClaimAcquired {
                claim,
                session,
                scope,
                intent,
                mode,
                issue,
            } => {
                let same = format!(
                    "{mode:?}|{}|{}",
                    scope.join(","),
                    intent.clone().unwrap_or_default()
                );
                let held = self
                    .claims
                    .values()
                    .any(|(st, se, sa)| *st == stream && *se == session && *sa == same);
                self.claims
                    .insert(claim, (stream.clone(), session.clone(), same));
                if held {
                    return None;
                }
                let name = self.name_of(&stream, &session);
                let mode = match mode {
                    ClaimMode::Exclusive => "exclusively",
                    ClaimMode::Advisory => "advisory",
                };
                let what = intent.clone().unwrap_or_default();
                let mut s = MonitorSignal::new(
                    event.at,
                    MonitorSource::Mesh,
                    "claimed",
                    MonitorSeverity::Info,
                    &format!("{name} claims {mode} {}: {what}", short_scope(&scope)),
                );
                s.for_you = ctx.reader.named_in(&what);
                s.session = Some(session);
                s.actor = Some(name);
                s.issue = issue;
                s.paths = scope;
                s
            }
            EventBody::ClaimReleased { claim } => {
                let (st, session, _) = self.claims.remove(&claim)?;
                let name = self.name_of(&st, &session);
                let mut s = MonitorSignal::new(
                    event.at,
                    MonitorSource::Mesh,
                    "released",
                    MonitorSeverity::Info,
                    &format!("{name} released claim {claim}"),
                );
                s.session = Some(session);
                s.actor = Some(name);
                s
            }
            EventBody::HandoverPublished { handover } => {
                let HandoverBody {
                    branch,
                    head,
                    body,
                    issue,
                    ..
                } = handover;
                let branch_name = branch.clone().unwrap_or_else(|| "(no branch)".into());
                self.handovers
                    .insert(HandoverBody::digest_of(&body), branch_name.clone());
                let next = next_action(&body);
                let head = head
                    .map(|h| h.chars().take(10).collect::<String>())
                    .unwrap_or_default();
                let mut s = MonitorSignal::new(
                    event.at,
                    MonitorSource::Mesh,
                    "handover",
                    MonitorSeverity::Attention,
                    &format!("{machine} handed over {branch_name}@{head}: {next}"),
                );
                s.for_you = ctx.reader.named_in(&body) || ctx.reader.named_in(&branch_name);
                s.branch = branch;
                s.issue = issue;
                s
            }
            EventBody::HandoverConsumed { handover, session } => {
                let branch = self.handovers.get(&handover).cloned();
                let name = self.name_of(&stream, &session);
                let mut s = MonitorSignal::new(
                    event.at,
                    MonitorSource::Mesh,
                    "handover_taken",
                    MonitorSeverity::Info,
                    &format!(
                        "{name} took the handover of {}",
                        branch
                            .clone()
                            .unwrap_or_else(|| handover.chars().take(12).collect())
                    ),
                );
                s.session = Some(session);
                s.actor = Some(name);
                s.branch = branch;
                s
            }
            EventBody::ReviewRequested {
                review,
                session,
                subject,
                scope,
                issue,
                reviewer,
            } => {
                let name = self.name_of(&stream, &session);
                let mine = ctx.reader.is(&session) || ctx.reader.is(&name);
                // I2415 adds a body (up to 8 KiB): it can name the reader where the subject does not
                let text = event
                    .body
                    .get("body")
                    .and_then(|b| b.as_str())
                    .unwrap_or_default();
                let asked = reviewer.as_deref().is_some_and(|r| ctx.reader.is(r))
                    || ctx.reader.named_in(&subject)
                    || ctx.reader.named_in(text);
                if asked {
                    self.involved.insert(format!("{stream}/{review}"));
                }
                self.reviews.insert(
                    format!("{stream}/{review}"),
                    (name.clone(), subject.clone(), mine),
                );
                let mut s = MonitorSignal::new(
                    event.at,
                    MonitorSource::Mesh,
                    "review_requested",
                    if asked {
                        MonitorSeverity::Action
                    } else {
                        MonitorSeverity::Attention
                    },
                    &format!(
                        "{name} asks {} to review ({review}): {subject}",
                        reviewer.as_deref().unwrap_or("anyone")
                    ),
                );
                s.for_you = asked;
                s.session = Some(session);
                s.actor = Some(name);
                s.issue = issue;
                s.paths = scope;
                s
            }
            EventBody::ReviewAnswered {
                request,
                session,
                verdict,
                note,
            } => {
                let name = self.name_of(&stream, &session);
                if ctx.reader.is(&session) || ctx.reader.is(&name) {
                    self.involved.insert(request.clone());
                }
                let (requester, subject, mine) = self
                    .reviews
                    .get(&request)
                    .cloned()
                    .unwrap_or_else(|| (String::new(), String::new(), false));
                let review = request.rsplit('/').next().unwrap_or(&request).to_string();
                let note = note.unwrap_or_default();
                let severity = if mine {
                    MonitorSeverity::Action
                } else if verdict == "approved" || verdict == "approve" {
                    MonitorSeverity::Info
                } else {
                    MonitorSeverity::Attention
                };
                let to = if requester.is_empty() {
                    String::new()
                } else {
                    format!(" to {requester}")
                };
                let mut s = MonitorSignal::new(
                    event.at,
                    MonitorSource::Mesh,
                    "review_answered",
                    severity,
                    &format!(
                        "{name} answered {review}{to}: {verdict} — {}",
                        if note.is_empty() { subject } else { note }
                    ),
                );
                s.for_you = mine;
                s.session = Some(session);
                s.actor = Some(name);
                s
            }
        };
        signal.machine = Some(machine);
        Some(signal)
    }

    /// A review withdrawn by its asker (I2415's `review_withdrawn` {request, session, note?}):
    /// news to whoever was asked for it or answered it, a note to everyone else.
    fn withdrawn(
        &mut self,
        event: &MeshEvent,
        stream: &str,
        ctx: &Context<'_>,
    ) -> Option<MonitorSignal> {
        let body = &event.body;
        let request = body.get("request")?.as_str()?.to_string();
        let session = body
            .get("session")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let note = body
            .get("note")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let name = self.name_of(stream, &session);
        let subject = self
            .reviews
            .remove(&request)
            .map(|(_, subject, _)| subject)
            .unwrap_or_default();
        let concerns_reader = self.involved.remove(&request);
        let review = request.rsplit('/').next().unwrap_or(&request).to_string();
        let mut s = MonitorSignal::new(
            event.at,
            MonitorSource::Mesh,
            "review_withdrawn",
            if concerns_reader {
                MonitorSeverity::Attention
            } else {
                MonitorSeverity::Info
            },
            &format!(
                "{name} withdrew {review}: {}",
                if note.is_empty() {
                    subject.as_str()
                } else {
                    note
                }
            ),
        );
        s.for_you = concerns_reader;
        s.session = Some(session);
        s.actor = Some(name);
        Some(s)
    }

    fn name_of(&self, stream: &str, session: &str) -> String {
        self.sessions
            .get(&(stream.to_string(), session.to_string()))
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| session.to_string())
    }
}

/// How to call a session: its worker when it named one, else its session and client.
fn who(info: &SessionInfo) -> String {
    match &info.worker {
        Some(worker) if !worker.trim().is_empty() && worker != "Claude Code" => {
            format!("{worker} ({})", info.session)
        }
        _ => format!("{} ({})", info.session, info.client),
    }
}

/// The first line of a handover's `# Next Action`, or its first line when it has none.
fn next_action(body: &str) -> String {
    let after = body
        .split("# Next Action")
        .nth(1)
        .unwrap_or(body)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'));
    after.unwrap_or_default().to_string()
}

/// A scope as a reader can take in at a glance: the first two paths and how many more.
fn short_scope(scope: &[String]) -> String {
    let unique: Vec<&String> = {
        let mut seen = HashSet::new();
        scope.iter().filter(|p| seen.insert(*p)).collect()
    };
    match unique.len() {
        0 => "nothing".into(),
        1 | 2 => unique
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        n => format!("{}, {} +{}", unique[0], unique[1], n - 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::journal::StreamId;
    use serde_json::{json, Value};

    const NODE: &str = "aaaa1111000000000000000000000000";
    const STREAM: &str = "aaaa1111000000000000000000000000-bbbbbbbbbbbbbbbb-cccccccccccccccc";

    fn event(lamport: u64, at: u64, body: Value) -> MeshEvent {
        MeshEvent {
            v: 1,
            stream: StreamId::try_from(STREAM.to_string()).expect("stream id"),
            seq: lamport,
            lamport,
            at,
            repo: "r".into(),
            pk: "k".into(),
            body,
            sig: "s".into(),
        }
    }

    fn opened(lamport: u64, session: &str, intent: &str, head: &str) -> MeshEvent {
        event(
            lamport,
            lamport,
            json!({"kind": "session_opened", "info": {
                "session": session, "client": "claude-code", "worker": "1b",
                "intent": intent, "head": head }}),
        )
    }

    fn ctx<'a>(reader: &'a Reader, machines: &'a HashMap<String, String>) -> Context<'a> {
        Context {
            reader,
            machines,
            since: 0,
        }
    }

    #[test]
    fn a_reprojection_is_suppressed_and_a_change_is_news() {
        let events = vec![
            opened(1, "board-p1", "train", "a"),
            opened(2, "board-p1", "train", "a"),
            opened(3, "board-p1", "train", "b"),
            opened(4, "board-p1", "release", "b"),
        ];
        let reader = Reader::default();
        let machines = HashMap::from([(NODE.to_string(), "lundra".to_string())]);
        let read = fold(&events, None, &ctx(&reader, &machines));
        let kinds: Vec<&str> = read.signals.iter().map(|(_, s)| s.kind.as_str()).collect();
        assert_eq!(kinds, vec!["session_joined", "session_changed"]);
        assert_eq!(read.suppressed, 2, "an unchanged opening and a head move");
        assert_eq!(read.signals[0].1.machine.as_deref(), Some("lundra"));
        assert!(read.signals[1]
            .1
            .summary
            .contains("1b (board-p1) now: release"));
        assert_eq!(read.high.unwrap().lamport, 4);
    }

    #[test]
    fn events_up_to_the_cursor_only_build_memory() {
        let events = vec![
            opened(1, "board-p1", "train", "a"),
            opened(2, "board-p1", "train", "a"),
        ];
        let reader = Reader::default();
        let machines = HashMap::new();
        let after = Position::of(&events[0]);
        let read = fold(&events, Some(&after), &ctx(&reader, &machines));
        assert!(
            read.signals.is_empty(),
            "the session was already known before the cursor"
        );
        assert_eq!(read.suppressed, 1);
    }

    #[test]
    fn a_review_asked_of_the_reader_is_an_action_and_its_answer_too() {
        let events = vec![
            event(
                1,
                1,
                json!({"kind": "session_opened", "info": {"session": "board-p3", "client": "claude-code", "worker": "3d", "intent": "lane"}}),
            ),
            event(
                2,
                2,
                json!({"kind": "review_requested", "review": "r-1", "session": "board-p3",
                "subject": "PR #882 worktree reconcile", "reviewer": "1b"}),
            ),
            event(
                3,
                3,
                json!({"kind": "review_requested", "review": "r-2", "session": "board-p3",
                "subject": "someone else's"}),
            ),
            event(
                4,
                4,
                json!({"kind": "review_answered", "request": format!("{STREAM}/r-2"),
                "session": "board-p9", "verdict": "changes_requested", "note": "core-check fails"}),
            ),
        ];
        let reader = Reader::named(Some("1b"));
        let machines = HashMap::new();
        let read = fold(&events, None, &ctx(&reader, &machines));
        let asked = &read.signals[1].1;
        assert_eq!(asked.kind, "review_requested");
        assert!(asked.for_you);
        assert_eq!(asked.severity, MonitorSeverity::Action);
        let other = &read.signals[2].1;
        assert!(!other.for_you);
        assert_eq!(other.severity, MonitorSeverity::Attention);
        let answer = &read.signals[3].1;
        assert_eq!(answer.severity, MonitorSeverity::Attention);
        assert!(answer
            .summary
            .contains("changes_requested — core-check fails"));
    }

    #[test]
    fn the_answer_to_my_own_request_is_for_me() {
        let mut reader = Reader::default();
        reader.sessions.push("board-p7".into());
        let events = vec![
            event(
                1,
                1,
                json!({"kind": "review_requested", "review": "r-9", "session": "board-p7",
                "subject": "read my branch"}),
            ),
            event(
                2,
                2,
                json!({"kind": "review_answered", "request": format!("{STREAM}/r-9"),
                "session": "board-p2", "verdict": "approved"}),
            ),
        ];
        let machines = HashMap::new();
        let read = fold(&events, None, &ctx(&reader, &machines));
        assert!(read.signals[1].1.for_you);
        assert_eq!(read.signals[1].1.severity, MonitorSeverity::Action);
    }

    #[test]
    fn a_claim_held_again_is_not_news_and_a_release_names_its_holder() {
        let claim = |lamport: u64, id: &str| {
            event(
                lamport,
                lamport,
                json!({"kind": "claim_acquired", "claim": id,
                "session": "board-p1", "scope": ["a", "b", "c"], "intent": "work", "mode": "advisory"}),
            )
        };
        let events = vec![
            opened(1, "board-p1", "x", "a"),
            claim(2, "c-1"),
            claim(3, "c-2"),
            event(4, 4, json!({"kind": "claim_released", "claim": "c-1"})),
            event(
                5,
                5,
                json!({"kind": "claim_released", "claim": "c-unknown"}),
            ),
        ];
        let reader = Reader::default();
        let machines = HashMap::new();
        let read = fold(&events, None, &ctx(&reader, &machines));
        let kinds: Vec<&str> = read.signals.iter().map(|(_, s)| s.kind.as_str()).collect();
        assert_eq!(kinds, vec!["session_joined", "claimed", "released"]);
        assert!(read.signals[1]
            .1
            .summary
            .contains("claims advisory a, b +1: work"));
        assert!(read.signals[2]
            .1
            .summary
            .starts_with("1b (board-p1) released"));
        assert_eq!(read.suppressed, 2);
    }

    #[test]
    fn a_handover_shows_its_next_action_and_who_took_it() {
        let body = "# Objective\nx\n# Next Action\n\n1. merge #885 after 1b's link\n";
        let events = vec![
            event(
                1,
                1,
                json!({"kind": "handover_published", "handover": {
                "id": "h", "branch": "fix/x", "head": "0123456789abcdef", "body": body }}),
            ),
            event(
                2,
                2,
                json!({"kind": "handover_consumed",
                "handover": HandoverBody::digest_of(body), "session": "board-p4"}),
            ),
            event(
                3,
                3,
                json!({"kind": "session_closed", "session": "board-p4"}),
            ),
            event(4, 4, json!({"kind": "a_kind_from_the_future"})),
        ];
        let reader = Reader::named(Some("1b"));
        let machines = HashMap::new();
        let read = fold(&events, None, &ctx(&reader, &machines));
        let handover = &read.signals[0].1;
        assert!(handover
            .summary
            .ends_with("fix/x@0123456789: 1. merge #885 after 1b's link"));
        assert!(handover.for_you);
        assert_eq!(
            read.signals[1].1.summary,
            "board-p4 took the handover of fix/x"
        );
        assert_eq!(read.signals.len(), 2, "a session never shown leaves unseen");
        assert_eq!(
            read.suppressed, 2,
            "its departure and an unknown kind are read, not shown"
        );
    }

    #[test]
    fn a_connection_that_says_nothing_is_not_news_until_it_does() {
        let silent = |lamport: u64, session: &str| {
            event(
                lamport,
                lamport,
                json!({"kind": "session_opened", "info": {
                "session": session, "client": "claude-code"}}),
            )
        };
        let events = vec![
            silent(1, "board-p97"),
            event(
                2,
                2,
                json!({"kind": "session_closed", "session": "board-p97"}),
            ),
            silent(3, "board-p98"),
            opened(4, "board-p98", "the release train", "a"),
            event(
                5,
                5,
                json!({"kind": "session_closed", "session": "board-p98"}),
            ),
        ];
        let reader = Reader::default();
        let machines = HashMap::new();
        let read = fold(&events, None, &ctx(&reader, &machines));
        let kinds: Vec<&str> = read.signals.iter().map(|(_, s)| s.kind.as_str()).collect();
        assert_eq!(kinds, vec!["session_joined", "session_left"]);
        assert!(read.signals[0]
            .1
            .summary
            .ends_with("joined: the release train"));
        assert_eq!(read.suppressed, 3);
    }

    #[test]
    fn a_withdrawn_review_is_news_to_whoever_was_asked_or_answered() {
        let events = vec![
            event(
                1,
                1,
                json!({"kind": "review_requested", "review": "r-1", "session": "s1",
                "subject": "read my branch", "body": "1b, the mesh part is yours"}),
            ),
            event(
                2,
                2,
                json!({"kind": "review_requested", "review": "r-2", "session": "s1",
                "subject": "someone else's"}),
            ),
            event(
                3,
                3,
                json!({"kind": "review_withdrawn", "request": format!("{STREAM}/r-1"),
                "session": "s1", "note": "landed without it"}),
            ),
            event(
                4,
                4,
                json!({"kind": "review_withdrawn", "request": format!("{STREAM}/r-2"),
                "session": "s1"}),
            ),
        ];
        let reader = Reader::named(Some("1b"));
        let machines = HashMap::new();
        let read = fold(&events, None, &ctx(&reader, &machines));
        assert!(read.signals[0].1.for_you, "the body names the reader");
        let mine = &read.signals[2].1;
        assert_eq!(mine.kind, "review_withdrawn");
        assert!(mine.for_you);
        assert_eq!(mine.severity, MonitorSeverity::Attention);
        assert!(
            mine.summary.ends_with("withdrew r-1: landed without it"),
            "{}",
            mine.summary
        );
        let other = &read.signals[3].1;
        assert!(!other.for_you);
        assert!(
            other.summary.ends_with("withdrew r-2: someone else's"),
            "{}",
            other.summary
        );
    }

    #[test]
    fn a_fresh_read_shows_only_the_window_but_stands_after_everything() {
        let events = vec![
            opened(1, "board-p1", "old", "a"),
            opened(2, "board-p2", "new", "a"),
        ];
        let reader = Reader::default();
        let machines = HashMap::new();
        let mut c = ctx(&reader, &machines);
        c.since = 2;
        let read = fold(&events, None, &c);
        assert_eq!(read.signals.len(), 1);
        assert_eq!(read.high.unwrap().lamport, 2);
    }

    #[test]
    fn a_scope_is_shortened_without_repeats() {
        assert_eq!(short_scope(&[]), "nothing");
        assert_eq!(short_scope(&["a".into(), "a".into()]), "a");
        assert_eq!(
            short_scope(&["a".into(), "b".into(), "c".into(), "d".into()]),
            "a, b +2"
        );
    }
}
