//! The monitor: everything that happens around this repository, followed in one stream
//! (ADR 0133).
//!
//! A worker that wants to know what is happening reads several records, each with its own
//! cursor: the mesh journal (which the board is projected into), the executor's trail, and
//! GitHub's pull requests and workflow runs. The monitor joins them into one page of
//! [`MonitorSignal`]s after one [`Cursor`], and it is the fold that makes the page worth
//! reading:
//!
//! - a session that re-announces itself unchanged on every heartbeat is counted as
//!   `suppressed`, never shown, and so is an executor step that decided nothing;
//! - a signal whose reviewer, handover, branch or text names the reader is marked
//!   `for_you`;
//! - a source that cannot be read is a [`MonitorSourceState`] with its reason, never an
//!   empty list, so that silence means nothing happened rather than nothing was asked.
//!
//! The monitor writes nothing and keeps no store: every signal is derived from a record that
//! already exists, and the cursor carries each source's own position, so a page after a
//! cursor repeats nothing and a reconnect is the same question asked again.

pub(crate) mod executor;
pub(crate) mod github;
pub(crate) mod mesh;

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The longest summary a signal carries, in characters.
pub(crate) const SUMMARY_LIMIT: usize = 200;

/// Where a signal comes from.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MonitorSource {
    /// The mesh journal: sessions on every linked machine (this checkout's board among
    /// them), their claims, handovers and reviews.
    Mesh,
    /// The executor's trail: the pull-request lane's merges, refreshes, repairs, closures
    /// and failures.
    Executor,
    /// GitHub: pull requests opened, merged and closed, and finished workflow runs.
    Github,
}

impl MonitorSource {
    /// Every source, in the order a page reports them.
    pub(crate) const ALL: [MonitorSource; 3] = [
        MonitorSource::Mesh,
        MonitorSource::Executor,
        MonitorSource::Github,
    ];

    /// The name a reader writes in `sources=`.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            MonitorSource::Mesh => "mesh",
            MonitorSource::Executor => "executor",
            MonitorSource::Github => "github",
        }
    }

    /// The source a name means, if it means one.
    pub(crate) fn parse(name: &str) -> Option<MonitorSource> {
        MonitorSource::ALL
            .into_iter()
            .find(|s| s.as_str() == name.trim())
    }
}

/// How much a signal asks of its reader.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum MonitorSeverity {
    /// Something happened; nothing is asked.
    Info,
    /// Something failed or waits for someone: worth a look.
    Attention,
    /// Something waits for the reader: a review asked of them, a handover for them.
    Action,
}

/// One thing that happened, as one line a person or an agent can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MonitorSignal {
    /// When it happened, in Unix seconds, on the clock of whoever recorded it.
    pub at: u64,
    /// Which record it comes from.
    pub source: MonitorSource,
    /// What happened, e.g. `session_joined`, `review_requested`, `merged`, `run_failed`.
    pub kind: String,
    /// How much it asks of the reader.
    pub severity: MonitorSeverity,
    /// The machine it happened on, by its display name when the mesh knows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// The session that did it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Who did it: a session's worker or client, the executor's actor, a GitHub login.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// The pull request it is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// The branch it is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The issue it is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The repository paths it is about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// What happened, in one line of at most 200 characters.
    pub summary: String,
    /// Whether it names the reader: a review asked of them, an answer to their request, a
    /// handover or a text that names them.
    #[serde(default)]
    pub for_you: bool,
    /// Where to read more, when there is a page for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl MonitorSignal {
    /// A signal with what every signal has; the rest is filled by the reader that made it.
    pub(crate) fn new(
        at: u64,
        source: MonitorSource,
        kind: &str,
        severity: MonitorSeverity,
        summary: &str,
    ) -> MonitorSignal {
        MonitorSignal {
            at,
            source,
            kind: kind.to_string(),
            severity,
            machine: None,
            session: None,
            actor: None,
            pr: None,
            branch: None,
            issue: None,
            paths: Vec::new(),
            summary: one_line(summary),
            for_you: false,
            link: None,
        }
    }
}

/// Whether a source could be read, and what it contributed to the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MonitorSourceHealth {
    /// Read.
    Ok,
    /// Not there to read: the mesh is not running, there is no trail, `gh` is missing.
    Absent,
    /// There, but reading it failed.
    Error,
    /// Not asked for in `sources`.
    Skipped,
}

/// One source's standing on a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MonitorSourceState {
    /// The source.
    pub source: MonitorSource,
    /// Whether it was read.
    pub state: MonitorSourceHealth,
    /// Why it was not, when it was not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// How many of its signals are on this page.
    pub signals: usize,
    /// How many of its records were read and folded away as noise.
    pub suppressed: u64,
}

/// A page of the stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MonitorPage {
    /// The signals, oldest first.
    pub signals: Vec<MonitorSignal>,
    /// How many of them are for the reader.
    pub for_you: usize,
    /// How many records were read and folded away as noise.
    pub suppressed: u64,
    /// Every source, read or not, with its reason.
    pub sources: Vec<MonitorSourceState>,
    /// The cursor of the next page: ask with `after` set to it and nothing on this page
    /// comes back.
    pub cursor: String,
    /// Whether more signals wait after this page than its limit let through.
    pub more: bool,
}

/// Where a reader stands in every source; one token, so a page names the next one.
///
/// The token is `m<lamport>.<seq>.<stream>~x<line>~g<unix>`, with a part left out for a
/// source that has no position yet. A reader treats it as opaque.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Cursor {
    /// The mesh journal's causal key of the last event read.
    pub(crate) mesh: Option<mesh::Position>,
    /// How many lines of the executor's trail were read.
    pub(crate) executor: Option<usize>,
    /// The time of the last GitHub signal read, in Unix seconds.
    pub(crate) github: Option<u64>,
}

impl Cursor {
    /// The cursor a token names; a token that names nothing a reader can resume from is an
    /// error, never a silent fresh start.
    pub(crate) fn parse(token: &str) -> Result<Cursor, String> {
        let mut cursor = Cursor::default();
        for part in token.split('~').filter(|p| !p.is_empty()) {
            let (tag, rest) = part.split_at(1);
            match tag {
                "m" => {
                    let mut it = rest.splitn(3, '.');
                    let lamport = it.next().and_then(|s| s.parse().ok());
                    let seq = it.next().and_then(|s| s.parse().ok());
                    let stream = it.next().filter(|s| !s.is_empty());
                    match (lamport, seq, stream) {
                        (Some(lamport), Some(seq), Some(stream)) => {
                            cursor.mesh = Some(mesh::Position {
                                lamport,
                                stream: stream.to_string(),
                                seq,
                            })
                        }
                        _ => return Err(format!("the mesh part of cursor {token:?} is malformed")),
                    }
                }
                "x" => {
                    cursor.executor = Some(rest.parse().map_err(|_| {
                        format!("the executor part of cursor {token:?} is not a line count")
                    })?)
                }
                "g" => {
                    cursor.github = Some(rest.parse().map_err(|_| {
                        format!("the github part of cursor {token:?} is not a Unix time")
                    })?)
                }
                _ => return Err(format!("cursor {token:?} has an unknown part {part:?}")),
            }
        }
        Ok(cursor)
    }

    /// The token of this cursor.
    pub(crate) fn token(&self) -> String {
        let mut parts = Vec::new();
        if let Some(m) = &self.mesh {
            parts.push(format!("m{}.{}.{}", m.lamport, m.seq, m.stream));
        }
        if let Some(x) = self.executor {
            parts.push(format!("x{x}"));
        }
        if let Some(g) = self.github {
            parts.push(format!("g{g}"));
        }
        parts.join("~")
    }
}

/// Who the reader is, so that what is addressed to them can be marked.
#[derive(Debug, Clone, Default)]
pub(crate) struct Reader {
    /// Names that mark a signal when a text names them as a word: a worker (`1b`), a role,
    /// a login. Lower-case.
    pub(crate) words: Vec<String>,
    /// Session ids that are the reader's own: a review they asked, a session they are.
    pub(crate) sessions: Vec<String>,
}

impl Reader {
    /// A reader named by a comma-separated list; empty names are dropped.
    pub(crate) fn named(list: Option<&str>) -> Reader {
        Reader {
            words: list
                .unwrap_or_default()
                .split(',')
                .map(|w| w.trim().to_lowercase())
                .filter(|w| !w.is_empty())
                .collect(),
            sessions: Vec::new(),
        }
    }

    /// Whether `text` names the reader as a whole word: `1b` matches "ask 1b" and
    /// "1b:", not "p1b" or "1bx".
    pub(crate) fn named_in(&self, text: &str) -> bool {
        let lower = text.to_lowercase();
        self.words.iter().any(|w| contains_word(&lower, w))
    }

    /// Whether `session` is one of the reader's own, or names the reader.
    pub(crate) fn is(&self, session: &str) -> bool {
        self.sessions.iter().any(|s| s == session) || self.named_in(session)
    }
}

fn contains_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(i) = haystack[from..].find(word) {
        let start = from + i;
        let end = start + word.len();
        let before = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before && after {
            return true;
        }
        from = start + 1;
        if from >= haystack.len() {
            break;
        }
    }
    false
}

/// `text` on one line, at most [`SUMMARY_LIMIT`] characters, ending in `…` when cut.
pub(crate) fn one_line(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= SUMMARY_LIMIT {
        return flat;
    }
    let mut cut: String = flat.chars().take(SUMMARY_LIMIT - 1).collect();
    cut.push('…');
    cut
}

/// What one source contributed: its signals in its own order, each with the cursor position
/// after it, the position after everything it read, and what it folded away.
#[derive(Debug, Clone)]
pub(crate) struct SourceRead<P> {
    /// The signals, in the source's own order, each with its position.
    pub(crate) signals: Vec<(P, MonitorSignal)>,
    /// The position after everything read, noise included.
    pub(crate) high: Option<P>,
    /// How many records were read and folded away.
    pub(crate) suppressed: u64,
}

impl<P> Default for SourceRead<P> {
    fn default() -> Self {
        SourceRead {
            signals: Vec::new(),
            high: None,
            suppressed: 0,
        }
    }
}

/// What a page is assembled from: each source's read or the reason it has none.
pub(crate) struct Reads {
    pub(crate) mesh: Result<SourceRead<mesh::Position>, (MonitorSourceHealth, String)>,
    pub(crate) executor: Result<SourceRead<usize>, (MonitorSourceHealth, String)>,
    pub(crate) github: Result<SourceRead<u64>, (MonitorSourceHealth, String)>,
}

/// Assemble a page from what the sources read.
///
/// With a cursor the page is the oldest `limit` signals after it, merged across sources by
/// time while each source keeps its own order, so that every source's taken signals are a
/// prefix of what it read and its next position is exact. Without one it is the newest
/// `limit` signals (the reads were already bounded by the time window), and the next cursor
/// stands after everything read.
pub(crate) fn assemble(
    reads: Reads,
    previous: &Cursor,
    resuming: bool,
    limit: usize,
) -> MonitorPage {
    let mut states = Vec::new();
    let mut lanes: Vec<Vec<MonitorSignal>> = Vec::new();
    let mut suppressed = 0;
    let mut cursor = previous.clone();

    // each lane is (signals, positions) of one source; positions are kept to cut the cursor
    let mesh = lane(
        reads.mesh,
        MonitorSource::Mesh,
        &mut states,
        &mut suppressed,
    );
    let executor = lane(
        reads.executor,
        MonitorSource::Executor,
        &mut states,
        &mut suppressed,
    );
    let github = lane(
        reads.github,
        MonitorSource::Github,
        &mut states,
        &mut suppressed,
    );
    lanes.push(mesh.iter().map(|(_, s)| s.clone()).collect());
    lanes.push(executor.iter().map(|(_, s)| s.clone()).collect());
    lanes.push(github.iter().map(|(_, s)| s.clone()).collect());

    let total: usize = lanes.iter().map(Vec::len).sum();
    let mut taken = [0usize; 3];
    let mut signals = Vec::new();
    if resuming {
        // k-way merge by time, each lane in its own order; stop at the limit
        while signals.len() < limit {
            let next = (0..3)
                .filter(|&l| taken[l] < lanes[l].len())
                .min_by_key(|&l| (lanes[l][taken[l]].at, l));
            let Some(l) = next else { break };
            signals.push(lanes[l][taken[l]].clone());
            taken[l] += 1;
        }
        // a GitHub position is a second: never cut between two signals of the same second
        while taken[2] > 0
            && taken[2] < lanes[2].len()
            && lanes[2][taken[2]].at == lanes[2][taken[2] - 1].at
        {
            signals.push(lanes[2][taken[2]].clone());
            taken[2] += 1;
        }
        signals.sort_by_key(|s| s.at);
    } else {
        let mut all: Vec<MonitorSignal> = lanes.iter().flatten().cloned().collect();
        all.sort_by_key(|s| s.at);
        let skip = all.len().saturating_sub(limit);
        signals = all.split_off(skip);
        taken = [lanes[0].len(), lanes[1].len(), lanes[2].len()];
    }

    advance(&mut cursor.mesh, &mesh.positions, taken[0], mesh.high);
    advance(
        &mut cursor.executor,
        &executor.positions,
        taken[1],
        executor.high,
    );
    advance(&mut cursor.github, &github.positions, taken[2], github.high);
    for state in &mut states {
        let index = MonitorSource::ALL
            .iter()
            .position(|s| *s == state.source)
            .unwrap_or(0);
        state.signals = if resuming { taken[index] } else { 0 };
    }
    if !resuming {
        let mut counts: HashMap<MonitorSource, usize> = HashMap::new();
        for s in &signals {
            *counts.entry(s.source).or_default() += 1;
        }
        for state in &mut states {
            state.signals = counts.get(&state.source).copied().unwrap_or(0);
        }
    }
    let for_you = signals.iter().filter(|s| s.for_you).count();
    MonitorPage {
        more: resuming && signals.len() < total,
        signals,
        for_you,
        suppressed,
        sources: states,
        cursor: cursor.token(),
    }
}

/// One source's lane: its signals with their positions and its high-water position.
struct Lane<P> {
    positions: Vec<P>,
    high: Option<P>,
    signals: Vec<MonitorSignal>,
}

impl<P> Lane<P> {
    fn iter(&self) -> impl Iterator<Item = (&P, &MonitorSignal)> {
        self.positions.iter().zip(self.signals.iter())
    }
}

fn lane<P>(
    read: Result<SourceRead<P>, (MonitorSourceHealth, String)>,
    source: MonitorSource,
    states: &mut Vec<MonitorSourceState>,
    suppressed: &mut u64,
) -> Lane<P> {
    match read {
        Ok(read) => {
            *suppressed += read.suppressed;
            states.push(MonitorSourceState {
                source,
                state: MonitorSourceHealth::Ok,
                reason: None,
                signals: 0,
                suppressed: read.suppressed,
            });
            let (positions, signals) = read.signals.into_iter().unzip();
            Lane {
                positions,
                high: read.high,
                signals,
            }
        }
        Err((state, reason)) => {
            states.push(MonitorSourceState {
                source,
                state,
                reason: Some(reason),
                signals: 0,
                suppressed: 0,
            });
            Lane {
                positions: Vec::new(),
                high: None,
                signals: Vec::new(),
            }
        }
    }
}

/// Move a source's position: to its last taken signal when the page cut it short, else past
/// everything it read. A source that read nothing keeps the position it had.
fn advance<P: Clone>(slot: &mut Option<P>, positions: &[P], taken: usize, high: Option<P>) {
    if taken < positions.len() {
        if taken > 0 {
            *slot = Some(positions[taken - 1].clone());
        }
    } else if let Some(high) = high {
        *slot = Some(high);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(at: u64, source: MonitorSource) -> MonitorSignal {
        MonitorSignal::new(at, source, "k", MonitorSeverity::Info, "s")
    }

    #[test]
    fn a_cursor_survives_its_token() {
        let cursor = Cursor {
            mesh: Some(mesh::Position {
                lamport: 31247,
                stream: "641bdb94-b10f19aa-80c5335b".into(),
                seq: 12,
            }),
            executor: Some(540),
            github: Some(1_791_661_234),
        };
        assert_eq!(
            cursor.token(),
            "m31247.12.641bdb94-b10f19aa-80c5335b~x540~g1791661234"
        );
        assert_eq!(Cursor::parse(&cursor.token()).unwrap(), cursor);
        assert_eq!(Cursor::parse("").unwrap(), Cursor::default());
        assert_eq!(Cursor::parse("x7").unwrap().executor, Some(7));
    }

    #[test]
    fn a_malformed_cursor_is_refused_not_restarted() {
        assert!(Cursor::parse("m12").is_err());
        assert!(Cursor::parse("xseven").is_err());
        assert!(Cursor::parse("gsoon").is_err());
        assert!(Cursor::parse("q1").is_err());
    }

    #[test]
    fn a_name_is_matched_as_a_word() {
        let reader = Reader::named(Some("1b, Cd"));
        assert!(reader.named_in("ask 1b to review"));
        assert!(reader.named_in("1b: done"));
        assert!(reader.named_in("for CD"));
        assert!(!reader.named_in("p1b is someone else"));
        assert!(!reader.named_in("1bx"));
        assert!(!Reader::named(None).named_in("anything"));
        assert!(Reader::named(Some(" , ")).words.is_empty());
    }

    #[test]
    fn a_long_summary_is_one_cut_line() {
        let long = "word ".repeat(100);
        let line = one_line(&format!("first\nsecond {long}"));
        assert!(line.starts_with("first second"));
        assert_eq!(line.chars().count(), SUMMARY_LIMIT);
        assert!(line.ends_with('…'));
        assert_eq!(one_line("  a \n b  "), "a b");
    }

    #[test]
    fn sources_parse_by_name() {
        assert_eq!(MonitorSource::parse(" mesh "), Some(MonitorSource::Mesh));
        assert_eq!(MonitorSource::parse("github"), Some(MonitorSource::Github));
        assert_eq!(
            MonitorSource::parse("executor"),
            Some(MonitorSource::Executor)
        );
        assert_eq!(MonitorSource::parse("slack"), None);
    }

    fn reads(mesh: Vec<(u64, u64)>, executor: Vec<(usize, u64)>, github: Vec<(u64, u64)>) -> Reads {
        let pos = |l: u64| mesh::Position {
            lamport: l,
            stream: "s".into(),
            seq: l,
        };
        Reads {
            mesh: Ok(SourceRead {
                high: mesh.last().map(|(l, _)| pos(*l)),
                signals: mesh
                    .into_iter()
                    .map(|(l, at)| (pos(l), signal(at, MonitorSource::Mesh)))
                    .collect(),
                suppressed: 3,
            }),
            executor: Ok(SourceRead {
                high: executor.last().map(|(n, _)| *n),
                signals: executor
                    .into_iter()
                    .map(|(n, at)| (n, signal(at, MonitorSource::Executor)))
                    .collect(),
                suppressed: 1,
            }),
            github: Ok(SourceRead {
                high: github.last().map(|(g, _)| *g),
                signals: github
                    .into_iter()
                    .map(|(g, at)| (g, signal(at, MonitorSource::Github)))
                    .collect(),
                suppressed: 0,
            }),
        }
    }

    #[test]
    fn a_resumed_page_is_cut_per_source_and_the_next_repeats_nothing() {
        let page = assemble(
            reads(
                vec![(10, 100), (11, 300)],
                vec![(1, 200), (2, 400)],
                vec![(150, 150)],
            ),
            &Cursor::default(),
            true,
            3,
        );
        let ats: Vec<u64> = page.signals.iter().map(|s| s.at).collect();
        assert_eq!(ats, vec![100, 150, 200]);
        assert!(page.more);
        assert_eq!(page.suppressed, 4);
        let next = Cursor::parse(&page.cursor).unwrap();
        assert_eq!(
            next.mesh.unwrap().lamport,
            10,
            "mesh cut after its taken prefix"
        );
        assert_eq!(next.executor, Some(1));
        assert_eq!(
            next.github,
            Some(150),
            "github read everything: past its high water"
        );
    }

    #[test]
    fn a_github_second_is_never_split() {
        let page = assemble(
            reads(vec![], vec![], vec![(5, 5), (5, 5), (6, 6)]),
            &Cursor::default(),
            true,
            1,
        );
        assert_eq!(page.signals.len(), 2, "both signals of second 5");
        assert_eq!(Cursor::parse(&page.cursor).unwrap().github, Some(5));
    }

    #[test]
    fn a_fresh_page_is_the_newest_and_stands_after_everything() {
        let page = assemble(
            reads(vec![(10, 100), (11, 300)], vec![(1, 200), (2, 400)], vec![]),
            &Cursor::default(),
            false,
            2,
        );
        let ats: Vec<u64> = page.signals.iter().map(|s| s.at).collect();
        assert_eq!(ats, vec![300, 400]);
        assert!(!page.more);
        let next = Cursor::parse(&page.cursor).unwrap();
        assert_eq!(next.mesh.unwrap().lamport, 11);
        assert_eq!(next.executor, Some(2));
        assert_eq!(
            next.github, None,
            "a source that read nothing keeps no position"
        );
        assert_eq!(page.sources[0].signals, 1);
    }

    #[test]
    fn an_unreadable_source_is_reported_with_its_reason() {
        let mut r = reads(vec![(1, 1)], vec![], vec![]);
        r.github = Err((MonitorSourceHealth::Absent, "gh is not installed".into()));
        let page = assemble(r, &Cursor::default(), true, 10);
        let github = page
            .sources
            .iter()
            .find(|s| s.source == MonitorSource::Github)
            .unwrap();
        assert_eq!(github.state, MonitorSourceHealth::Absent);
        assert_eq!(github.reason.as_deref(), Some("gh is not installed"));
        assert_eq!(page.signals.len(), 1);
    }

    #[test]
    fn signals_for_the_reader_are_counted() {
        let mut r = reads(vec![(1, 1), (2, 2)], vec![], vec![]);
        if let Ok(read) = &mut r.mesh {
            read.signals[1].1.for_you = true;
        }
        assert_eq!(assemble(r, &Cursor::default(), true, 10).for_you, 1);
    }
}
