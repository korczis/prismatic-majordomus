//! Episodes: the execution episode of a client that has no provider hooks, drawn by the
//! MCP connection itself (ADR 0043).
//!
//! # The defect this exists for
//!
//! Majordomus draws the episode boundary below the model, so that the episode which
//! mattered — the one that ended in a crash, a compaction, or somebody closing the window —
//! is recorded whether or not a model remembered to record it. Until this module that was
//! wired for exactly one provider. Claude Code fires `SessionStart` and `SessionEnd`, a
//! shim runs `capture session`, and the repository gets an episode. Every other client got
//! nothing at all, and the repository's claim to draw the boundary below the model was a
//! claim about one vendor.
//!
//! A client with no hooks still has a boundary, and this server is standing on it: the
//! client speaks `initialize`, makes calls, and goes. That is an episode. The connection is
//! the event source, the peer board already tracks attach, activity and detach, and the
//! repository's own `majordomus capture session` already opens and closes episodes for a
//! provider that names its session. Nothing new needs inventing; the three had to be joined.
//!
//! # A peer is not an episode
//!
//! They are deliberately separate types and the distinction has cost real time here. A peer
//! id (`p1`, `p2`, ...) is *this process's* name for *this connection*, handed out in
//! attachment order and reused by the next server. It is not a worker identity: a session
//! that reconnects comes back as a different peer, and on 2026-09-09 a session in this
//! repository announced, was re-established under a new id, and was invisible to eight
//! others for three hours because everybody had been treating the id as durable.
//!
//! So an episode is keyed by an **external identity** the client supplies — its own
//! conversation id, thread id, whatever it durably calls itself — and the peer holding it is
//! a field that changes. The invariants:
//!
//! * a connection holds **zero or one** episode: `initialize` alone opens nothing, and a
//!   client that never calls `episodes.attach` is a peer and not a worker with a record;
//! * a client that reconnects **re-associates** with its own episode by external identity,
//!   under whatever peer id it now has;
//! * an episode outlives the connection that opened it, for a bounded grace, because a
//!   dropped socket is not the end of somebody's work — it is the most ordinary thing that
//!   happens to one.
//!
//! # What is honestly not here
//!
//! Raw prompt capture. An MCP server is handed `initialize`, tool calls and notifications;
//! the person's prompt is never among them, in any version of the protocol. Prompt capture
//! stays provider-specific, `share/providers.yaml` declares `prompts: none` for the generic
//! provider with that reasoning written down, and no surface implies otherwise.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::peers::PeerId;

/// How long an episode whose connection has gone is kept before it is closed.
///
/// Longer than the transport's own idle timeout on purpose. `http::mcp::SESSION_IDLE_TIMEOUT`
/// decides when a *connection* is forgotten — ninety seconds, which is right for a socket —
/// and a client that is restarted, rebuilt or resumed takes longer than that to come back.
/// Closing the episode at the same moment as the connection would publish a record saying
/// the work ended, for work that resumed four minutes later under a new peer id, which is
/// precisely the fiction this whole subsystem exists to avoid.
pub const REATTACH_GRACE: Duration = Duration::from_secs(15 * 60);

/// Why an episode was closed. Reported to the repository as the end event's `reason`, where
/// `share/providers.yaml`'s deliberate-end list decides whether the record says `closed` or
/// `interrupted` — the same table the provider hooks are read through, so a connection
/// episode and a hook episode cannot disagree about what "ended deliberately" means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    /// The client said so: `episodes.detach`, or `DELETE /mcp`. A deliberate end.
    Detach,
    /// The server is stopping and every episode goes with it. Deliberate: nothing was cut
    /// short by this, the machine was.
    Shutdown,
    /// The connection went and did not come back inside [`REATTACH_GRACE`]. Not deliberate,
    /// and the record says `interrupted`, because calling a crashed episode complete is the
    /// worse of the two mistakes.
    Expired,
}

impl CloseReason {
    /// The word sent to the repository's end event.
    pub fn as_str(self) -> &'static str {
        match self {
            CloseReason::Detach => "detach",
            CloseReason::Shutdown => "shutdown",
            CloseReason::Expired => "expired",
        }
    }
}

/// Where an episode stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeState {
    /// A connection holds it and the client is speaking.
    Open,
    /// The connection has gone; the episode is kept for [`REATTACH_GRACE`] so the client
    /// can come back to it. This is the state a crash lands in, and the one a reconnect
    /// recovers from.
    Detached,
}

/// One episode: what the client calls itself, which connection holds it now, and what the
/// repository did about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Episode {
    /// The client's own durable name for this piece of work, and the key everything here is
    /// addressed by. Never a peer id: see the module documentation.
    pub external_id: String,
    /// The provider id this episode is recorded under, as `share/providers.yaml` names one.
    pub provider: String,
    /// The connection holding it, when one does. `None` while detached — which is a state
    /// an episode is *supposed* to reach and come back from, not a fault.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<PeerId>,
    /// When it was first opened, RFC 3339, UTC. Unchanged by a re-attach: the episode is
    /// the work, and the work did not restart because a socket did.
    pub opened_at: String,
    /// Seconds since the client last spoke on it, through any connection.
    pub last_activity_seconds_ago: u64,
    /// How many times a connection has taken it over, the first attach included.
    pub attachments: u32,
    /// Open, or detached and waiting for its client.
    pub state: EpisodeState,
    /// What the repository's own episode command reported, verbatim — the record it opened,
    /// or the reason it could not. Never interpreted here, and never empty: a subsystem
    /// that silently does nothing is the failure mode this field exists to make visible.
    pub repository: String,
}

/// The episodes of one server.
///
/// In memory, like the peer board, and for a narrower reason: what survives the process is
/// the repository's own record, written by the same command a provider hook runs. This board
/// holds only the association between a live connection and that record, which is a fact
/// about now and stops being true when the process does.
pub struct EpisodeBoard {
    episodes: Mutex<BTreeMap<String, Entry>>,
    /// How the repository's episode boundary is driven. Injected so that the unit suites can
    /// drive the board without a repository and the real server drives the real command.
    driver: Box<dyn EpisodeDriver>,
}

struct Entry {
    episode: Episode,
    last_activity: Instant,
    detached_since: Option<Instant>,
}

/// What opens and closes the repository's own episode.
///
/// One trait with one real implementation, because the alternative was for this module to
/// learn how a session record is written — and a second writer of the record the provider
/// hooks already write is exactly the repeated semantic definition this repository refuses
/// (ADR 0004). [`ToolDriver`] runs `majordomus capture session`, the same command, with the
/// same payload shape, through the same adapter resolution.
pub trait EpisodeDriver: Send + Sync {
    /// Open the episode for `external_id` under `provider`. Returns what to record in
    /// [`Episode::repository`] — a description of what happened, successful or not.
    fn open(&self, provider: &str, external_id: &str) -> String;
    /// Close it, with the reason the record is to carry.
    fn close(&self, provider: &str, external_id: &str, reason: CloseReason) -> String;
}

/// The driver that runs the repository's own `majordomus capture session`.
///
/// It finds the tool the way a provider hook's shim does and in the same order — the
/// repository's `bin/majordomus`, an installation under `.majordomus/`, then whatever is on
/// the path — because a second resolution order would be a second answer to "which tool is
/// this repository's", and the shims are the ones that have been right about it for months.
#[derive(Debug, Clone)]
pub struct ToolDriver {
    root: PathBuf,
}

impl ToolDriver {
    /// A driver for the repository at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        ToolDriver { root: root.into() }
    }

    /// The shell tool, resolved as `.claude/hooks/majordomus-session-start` resolves it.
    fn tool(&self) -> Option<PathBuf> {
        for candidate in ["bin/majordomus", ".majordomus/bin/majordomus"] {
            let p = self.root.join(candidate);
            if is_executable(&p) {
                return Some(p);
            }
        }
        which("majordomus")
    }

    fn run(&self, provider: &str, event: &str, payload: &str) -> String {
        let Some(tool) = self.tool() else {
            return format!(
                "no episode was recorded: neither bin/majordomus nor .majordomus/bin/majordomus is executable under {} and none is on the path",
                self.root.display()
            );
        };
        // The payload goes in on stdin exactly as a provider's own event does, so the one
        // reader in lib/capture.sh parses both. Writing it with a pipe rather than an
        // argument is not decoration: `capture session` reads the episode key out of the
        // payload, and a key that arrived as an argument would be a second way in.
        let mut child = match Command::new(&tool)
            .arg("--repo")
            .arg(&self.root)
            .args(["capture", "session", "--provider", provider, "--event", event])
            .current_dir(&self.root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return format!("no episode was recorded: {} did not start: {e}", tool.display()),
        };
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(payload.as_bytes());
        }
        match child.wait_with_output() {
            // `capture session` writes nothing to stdout by contract and reports what it did
            // on stderr, because on a start event its stdout would be loaded into a model's
            // context. Its last line is the one that says what happened.
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stderr);
                let last = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
                if last.is_empty() {
                    format!("`capture session --event {event}` exited {} and said nothing", out.status)
                } else {
                    last.trim().to_string()
                }
            }
            Err(e) => format!("no episode was recorded: {} did not finish: {e}", tool.display()),
        }
    }
}

impl EpisodeDriver for ToolDriver {
    fn open(&self, provider: &str, external_id: &str) -> String {
        self.run(
            provider,
            "start",
            &format!(
                "{{\"session_id\":{},\"source\":\"attach\"}}\n",
                json_string(external_id)
            ),
        )
    }

    fn close(&self, provider: &str, external_id: &str, reason: CloseReason) -> String {
        self.run(
            provider,
            "end",
            &format!(
                "{{\"session_id\":{},\"reason\":\"{}\"}}\n",
                json_string(external_id),
                reason.as_str()
            ),
        )
    }
}

/// A driver that records nothing: the board without a repository behind it.
///
/// Used by the unit suites and by a server serving a checkout whose tool cannot be found. It
/// says so in every episode's `repository` field rather than leaving it blank, because an
/// episode that quietly recorded nothing is indistinguishable, to a reader, from one that
/// recorded everything.
#[derive(Debug, Clone, Default)]
pub struct NullDriver;

impl EpisodeDriver for NullDriver {
    fn open(&self, _: &str, _: &str) -> String {
        "no repository episode: this board is running without a driver".into()
    }
    fn close(&self, _: &str, _: &str, _: CloseReason) -> String {
        "no repository episode: this board is running without a driver".into()
    }
}

impl std::fmt::Debug for EpisodeBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpisodeBoard")
            .field("episodes", &self.list().len())
            .finish()
    }
}

impl Default for EpisodeBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl EpisodeBoard {
    /// A board that records nothing in the repository.
    pub fn new() -> Self {
        EpisodeBoard {
            episodes: Mutex::new(BTreeMap::new()),
            driver: Box::new(NullDriver),
        }
    }

    /// A board that drives the repository at `root` through its own `capture session`.
    pub fn for_repository(root: impl Into<PathBuf>) -> Self {
        EpisodeBoard {
            episodes: Mutex::new(BTreeMap::new()),
            driver: Box::new(ToolDriver::new(root)),
        }
    }

    /// A board over an arbitrary driver.
    pub fn with_driver(driver: Box<dyn EpisodeDriver>) -> Self {
        EpisodeBoard {
            episodes: Mutex::new(BTreeMap::new()),
            driver,
        }
    }

    /// Open an episode for `external_id`, or take an existing one over.
    ///
    /// The whole point is in the second half. A client that comes back — after a crash, a
    /// rebuild, a dropped socket, a client restart — names itself the same way and gets the
    /// same episode, with a new peer holding it and `attachments` one higher. Nothing is
    /// re-opened, no second record is written, and the ledger lines on either side of the
    /// gap belong to one episode, which is what makes the record of it true.
    ///
    /// Returns the episode and whether it was resumed rather than opened.
    pub fn attach(&self, peer: &PeerId, external_id: &str, provider: &str) -> (Episode, bool) {
        let mut episodes = self.lock();
        // A connection holds zero or one episode. A peer attaching to a second external
        // identity is not a second worker, it is the same connection changing its mind —
        // the first is released (and left detached, recoverable) rather than held by a peer
        // that has stopped speaking for it.
        let previously_held: Vec<String> = episodes
            .values()
            .filter(|e| e.episode.peer.as_ref() == Some(peer) && e.episode.external_id != external_id)
            .map(|e| e.episode.external_id.clone())
            .collect();
        for id in previously_held {
            if let Some(entry) = episodes.get_mut(&id) {
                entry.episode.peer = None;
                entry.episode.state = EpisodeState::Detached;
                entry.detached_since = Some(Instant::now());
            }
        }

        if let Some(entry) = episodes.get_mut(external_id) {
            entry.episode.peer = Some(peer.clone());
            entry.episode.state = EpisodeState::Open;
            entry.episode.attachments = entry.episode.attachments.saturating_add(1);
            entry.detached_since = None;
            entry.last_activity = Instant::now();
            entry.episode.last_activity_seconds_ago = 0;
            return (entry.episode.clone(), true);
        }

        // The driver runs while the map is locked, which is deliberate and not an oversight:
        // two connections racing to attach under one external identity must produce one
        // episode, and releasing the lock across the subprocess is how they would produce
        // two. The cost is one attach at a time per server, which is the correct price.
        //
        // It is re-entrant across processes and safe to be. `capture session --event start`
        // runs `serve ensure`, which probes this very server over HTTP from a child process
        // — so an attach handled on the stdio thread is answered by the HTTP listener's
        // thread, and one handled by the HTTP endpoint is answered by another of its
        // workers. Nothing on that path touches this mutex, so the probe cannot wait on the
        // attach that started it. `test/cases/200_generic_mcp_episode.sh` exercises both
        // directions — a stdio client that is the server, and a client bridged to somebody
        // else's — because an argument about threads is worth exactly as much as the run
        // that confirms it.
        let repository = self.driver.open(provider, external_id);
        let episode = Episode {
            external_id: external_id.to_string(),
            provider: provider.to_string(),
            peer: Some(peer.clone()),
            opened_at: now_rfc3339(),
            last_activity_seconds_ago: 0,
            attachments: 1,
            state: EpisodeState::Open,
            repository,
        };
        episodes.insert(
            external_id.to_string(),
            Entry {
                episode: episode.clone(),
                last_activity: Instant::now(),
                detached_since: None,
            },
        );
        (episode, false)
    }

    /// The client spoke. Every message on a connection holding an episode is a heartbeat for
    /// it, so a working client never has its episode reaped and no client has to send
    /// anything it would not otherwise send.
    pub fn touch(&self, peer: &PeerId) {
        let mut episodes = self.lock();
        for entry in episodes.values_mut() {
            if entry.episode.peer.as_ref() == Some(peer) {
                entry.last_activity = Instant::now();
            }
        }
    }

    /// The connection went. The episode is **not** closed: it is detached, and the client
    /// can come back to it by name for [`REATTACH_GRACE`].
    ///
    /// This is the difference between a lifecycle and a session counter. Closing here would
    /// publish a record saying the work ended every time a socket dropped, and the work
    /// almost never ends when a socket drops.
    pub fn detach(&self, peer: &PeerId) -> Vec<Episode> {
        let mut episodes = self.lock();
        let mut out = Vec::new();
        for entry in episodes.values_mut() {
            if entry.episode.peer.as_ref() == Some(peer) {
                entry.episode.peer = None;
                entry.episode.state = EpisodeState::Detached;
                entry.detached_since = Some(Instant::now());
                out.push(entry.episode.clone());
            }
        }
        out
    }

    /// Close one episode deliberately, by name: the client said it was done.
    pub fn close(&self, external_id: &str, reason: CloseReason) -> Option<Episode> {
        let mut episodes = self.lock();
        let entry = episodes.remove(external_id)?;
        let mut episode = entry.episode;
        episode.repository = self.driver.close(&episode.provider, &episode.external_id, reason);
        episode.peer = None;
        episode.state = EpisodeState::Detached;
        Some(episode)
    }

    /// Close every episode detached longer than `grace`. The reaper, and the answer to the
    /// client that never comes back.
    pub fn reap(&self, grace: Duration) -> Vec<Episode> {
        let expired: Vec<String> = {
            let episodes = self.lock();
            episodes
                .values()
                .filter(|e| e.detached_since.is_some_and(|t| t.elapsed() > grace))
                .map(|e| e.episode.external_id.clone())
                .collect()
        };
        expired
            .into_iter()
            .filter_map(|id| self.close(&id, CloseReason::Expired))
            .collect()
    }

    /// Close every episode: the server is stopping.
    ///
    /// An episode left open by a server that has gone is an episode nothing will ever close,
    /// and the repository would carry it as open for ever — the failure ADR 0041 names on
    /// the hook side, reached here by a different road.
    pub fn close_all(&self) -> Vec<Episode> {
        let ids: Vec<String> = self.lock().keys().cloned().collect();
        ids.into_iter()
            .filter_map(|id| self.close(&id, CloseReason::Shutdown))
            .collect()
    }

    /// Every episode this server holds, open and detached, in external-identity order.
    pub fn list(&self) -> Vec<Episode> {
        let episodes = self.lock();
        episodes
            .values()
            .map(|e| {
                let mut episode = e.episode.clone();
                episode.last_activity_seconds_ago = e.last_activity.elapsed().as_secs();
                episode
            })
            .collect()
    }

    /// The episode a connection holds, if it holds one.
    pub fn of_peer(&self, peer: &PeerId) -> Option<Episode> {
        self.list()
            .into_iter()
            .find(|e| e.peer.as_ref() == Some(peer))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Entry>> {
        self.episodes.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| is_executable(p))
}

fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    let rest = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// Howard Hinnant's `civil_from_days`, the same conversion the peer board uses.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// One JSON string, escaped. The payload is this tool's own and the identity in it came from
/// a client, so it is escaped rather than trusted: a quote in an external identity that
/// reached a shell's stdin unescaped would be a payload of somebody else's making.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct Recording {
        calls: StdMutex<Vec<String>>,
    }

    impl EpisodeDriver for std::sync::Arc<Recording> {
        fn open(&self, provider: &str, external_id: &str) -> String {
            self.calls
                .lock()
                .unwrap()
                .push(format!("open {provider} {external_id}"));
            format!("episode opened for {external_id}")
        }
        fn close(&self, provider: &str, external_id: &str, reason: CloseReason) -> String {
            self.calls
                .lock()
                .unwrap()
                .push(format!("close {provider} {external_id} {}", reason.as_str()));
            format!("episode closed for {external_id}")
        }
    }

    fn board() -> (EpisodeBoard, std::sync::Arc<Recording>) {
        let rec = std::sync::Arc::new(Recording::default());
        (EpisodeBoard::with_driver(Box::new(rec.clone())), rec)
    }

    /// Peer ids come from a real board, because they are handed out in attachment order and
    /// a test that invented them would be testing a numbering this code never sees.
    fn peers(n: usize) -> Vec<PeerId> {
        let board = crate::peers::PeerBoard::new();
        (0..n)
            .map(|_| board.attach(crate::peers::Transport::Stdio))
            .collect()
    }

    /// THE REGRESSION this module exists for. A client that loses its connection and comes
    /// back gets *its own* episode, not a second one, and the repository is asked to open an
    /// episode exactly once.
    #[test]
    fn a_reconnect_resumes_the_episode_by_external_identity() {
        let (board, rec) = board();
        let ids = peers(2);
        let first = ids[0].clone();
        let (opened, resumed) = board.attach(&first, "thread-7", "generic");
        assert!(!resumed, "the first attach is not a resume");
        assert_eq!(opened.attachments, 1);

        board.detach(&first);
        assert_eq!(board.list().len(), 1, "a detach must not discard the episode");
        assert_eq!(board.list()[0].state, EpisodeState::Detached);

        // a new connection: a different peer id, the same worker
        let second = ids[1].clone();
        let (again, resumed) = board.attach(&second, "thread-7", "generic");
        assert!(resumed, "the same external identity must resume, not re-open");
        assert_eq!(again.opened_at, opened.opened_at, "it is the same episode");
        assert_eq!(again.attachments, 2);
        assert_eq!(again.peer.as_ref(), Some(&second));

        assert_eq!(
            *rec.calls.lock().unwrap(),
            vec!["open generic thread-7".to_string()],
            "the repository episode was opened once, not once per connection"
        );
    }

    /// A connection holds zero or one episode, and `initialize` alone holds none.
    #[test]
    fn a_connection_holds_at_most_one_episode() {
        let (board, _) = board();
        let p = peers(1).remove(0);
        assert!(board.of_peer(&p).is_none(), "attaching nothing holds nothing");
        board.attach(&p, "one", "generic");
        board.attach(&p, "two", "generic");
        assert_eq!(board.of_peer(&p).map(|e| e.external_id), Some("two".into()));
        let one = board.list().into_iter().find(|e| e.external_id == "one").unwrap();
        assert_eq!(
            one.state,
            EpisodeState::Detached,
            "the episode it stopped speaking for is released, and recoverable"
        );
    }

    /// The reaper closes what never came back, and only that.
    #[test]
    fn the_reaper_closes_only_what_is_past_its_grace() {
        let (board, rec) = board();
        let p = peers(1).remove(0);
        board.attach(&p, "gone", "generic");
        assert!(board.reap(Duration::from_secs(1)).is_empty(), "an attached episode is never reaped");
        board.detach(&p);
        assert!(
            board.reap(Duration::from_secs(3600)).is_empty(),
            "inside the grace it waits"
        );
        let closed = board.reap(Duration::ZERO);
        assert_eq!(closed.len(), 1);
        assert!(board.list().is_empty());
        assert_eq!(
            rec.calls.lock().unwrap().last().map(String::as_str),
            Some("close generic gone expired"),
            "a cut-short episode is closed as expired, never as a deliberate end"
        );
    }

    /// A stopping server closes what it holds; an episode nothing will ever close is an
    /// episode the repository carries as open for ever.
    #[test]
    fn shutdown_closes_every_episode() {
        let (board, rec) = board();
        let ids = peers(2);
        board.attach(&ids[0], "a", "generic");
        board.attach(&ids[1], "b", "generic");
        assert_eq!(board.close_all().len(), 2);
        assert!(board.list().is_empty());
        let calls = rec.calls.lock().unwrap();
        assert!(calls.contains(&"close generic a shutdown".to_string()));
        assert!(calls.contains(&"close generic b shutdown".to_string()));
    }

    #[test]
    fn an_external_identity_is_escaped_into_the_payload() {
        assert_eq!(json_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(json_string("line\nbreak"), "\"line\\nbreak\"");
    }
}
