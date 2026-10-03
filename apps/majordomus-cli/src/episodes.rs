//! Episodes: the execution episode of a client that has no provider hooks, drawn by the
//! MCP connection itself (ADR 0103).
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
//!
//! # The lifecycle, end to end
//!
//! A client attaches under its own name, its connection drops, it comes back as a different
//! peer under the same name and gets the same episode, and then it says it is done:
//!
//! ```
//! use majordomus_cli::episodes::{CloseReason, ConnectionEpisodeState, EpisodeBoard};
//! use majordomus_cli::peers::{PeerBoard, Transport};
//!
//! let peers = PeerBoard::new();
//! let board = EpisodeBoard::new(); // records nothing in a repository
//! let first = peers.attach(Transport::Http);
//! let (opened, resumed) = board.attach(&first, "thread-7", "generic");
//! assert!(!resumed);
//!
//! board.detach(&first); // the socket went; the work did not
//! assert_eq!(board.list()[0].state, ConnectionEpisodeState::Detached);
//!
//! let second = peers.attach(Transport::Http); // a new peer id, the same worker
//! let (again, resumed) = board.attach(&second, "thread-7", "generic");
//! assert!(resumed);
//! assert_eq!((again.opened_at.as_str(), again.attachments), (opened.opened_at.as_str(), 2));
//!
//! let closed = board.close("thread-7", CloseReason::Detach).unwrap();
//! assert_eq!(closed.external_id, "thread-7");
//! assert!(board.list().is_empty());
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

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
///
/// The serialised word and [`CloseReason::as_str`] are one vocabulary:
///
/// ```
/// use majordomus_cli::episodes::CloseReason;
/// for reason in [CloseReason::Detach, CloseReason::Shutdown, CloseReason::Expired] {
///     let json = serde_json::to_string(&reason).unwrap();
///     assert_eq!(json, format!("\"{}\"", reason.as_str()));
/// }
/// ```
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
    /// The word sent to the repository's end event, as its `reason`. Only `expired` is
    /// outside the deliberate-end list, so only it records the episode as interrupted.
    ///
    /// ```
    /// use majordomus_cli::episodes::CloseReason;
    /// assert_eq!(CloseReason::Detach.as_str(), "detach");
    /// assert_eq!(CloseReason::Expired.as_str(), "expired");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            CloseReason::Detach => "detach",
            CloseReason::Shutdown => "shutdown",
            CloseReason::Expired => "expired",
        }
    }
}

/// Where an episode stands: held by a live connection, or waiting for its client to come
/// back. There is no closed state, because a closed episode leaves the board; what survives
/// it is the repository's own record.
///
/// ```
/// use majordomus_cli::episodes::{ConnectionEpisodeState, EpisodeBoard};
/// use majordomus_cli::peers::{PeerBoard, Transport};
/// let peer = PeerBoard::new().attach(Transport::Stdio);
/// let board = EpisodeBoard::new();
/// assert_eq!(board.attach(&peer, "w", "generic").0.state, ConnectionEpisodeState::Open);
/// assert_eq!(board.detach(&peer)[0].state, ConnectionEpisodeState::Detached);
/// assert_eq!(serde_json::to_string(&ConnectionEpisodeState::Detached).unwrap(), "\"detached\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionEpisodeState {
    /// A connection holds it and the client is speaking.
    Open,
    /// The connection has gone; the episode is kept for [`REATTACH_GRACE`] so the client
    /// can come back to it. This is the state a crash lands in, and the one a reconnect
    /// recovers from.
    Detached,
}

/// One episode: what the client calls itself, which connection holds it now, and what the
/// repository did about it.
///
/// ```
/// use majordomus_cli::episodes::{ConnectionEpisode, EpisodeBoard};
/// use majordomus_cli::peers::{PeerBoard, Transport};
/// let peer = PeerBoard::new().attach(Transport::Http);
/// let (episode, _) = EpisodeBoard::new().attach(&peer, "conv-42", "generic");
/// let episode: ConnectionEpisode = episode;
/// assert_eq!((episode.external_id.as_str(), episode.provider.as_str()), ("conv-42", "generic"));
/// assert_eq!(episode.peer.as_ref(), Some(&peer));
/// // never empty: a board with no repository behind it says so
/// assert!(episode.repository.contains("without a driver"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConnectionEpisode {
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
    pub state: ConnectionEpisodeState,
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
///
/// ```
/// use majordomus_cli::episodes::EpisodeBoard;
/// use majordomus_cli::peers::{PeerBoard, Transport};
/// let peers = PeerBoard::new();
/// let (a, b) = (peers.attach(Transport::Stdio), peers.attach(Transport::Http));
/// let board = EpisodeBoard::default();
/// board.attach(&b, "zeta", "generic");
/// board.attach(&a, "alpha", "generic");
/// // listed by external identity, never by peer or attachment order
/// let ids: Vec<String> = board.list().into_iter().map(|e| e.external_id).collect();
/// assert_eq!(ids, ["alpha", "zeta"]);
/// assert_eq!(board.close_all().len(), 2);
/// ```
pub struct EpisodeBoard {
    episodes: Mutex<BTreeMap<String, Entry>>,
    /// How the repository's episode boundary is driven. Injected so that the unit suites can
    /// drive the board without a repository and the real server drives the real command.
    driver: Box<dyn EpisodeDriver>,
}

struct Entry {
    episode: ConnectionEpisode,
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
///
/// A driver of one's own is how a board is driven without a repository; what it returns is
/// what the episode's `repository` field carries:
///
/// ```
/// use majordomus_cli::episodes::{CloseReason, EpisodeBoard, EpisodeDriver};
/// use majordomus_cli::peers::{PeerBoard, Transport};
///
/// struct Echo;
/// impl EpisodeDriver for Echo {
///     fn open(&self, provider: &str, id: &str) -> String { format!("opened {provider} {id}") }
///     fn close(&self, _: &str, id: &str, reason: CloseReason) -> String {
///         format!("closed {id} ({})", reason.as_str())
///     }
/// }
///
/// let board = EpisodeBoard::with_driver(Box::new(Echo));
/// let peer = PeerBoard::new().attach(Transport::Stdio);
/// assert_eq!(board.attach(&peer, "t1", "generic").0.repository, "opened generic t1");
/// assert_eq!(board.close("t1", CloseReason::Shutdown).unwrap().repository, "closed t1 (shutdown)");
/// ```
pub trait EpisodeDriver: Send + Sync {
    /// Open the episode for `external_id` under `provider`. Returns what to record in
    /// [`ConnectionEpisode::repository`] — a description of what happened, successful or not.
    /// It is called once per episode, on the first attach; a resume does not call it again.
    ///
    /// ```
    /// use majordomus_cli::episodes::{EpisodeDriver, NullDriver};
    /// let said = NullDriver.open("generic", "t1");
    /// assert!(said.starts_with("no repository episode"), "a driver never answers blank");
    /// ```
    fn open(&self, provider: &str, external_id: &str) -> String;
    /// Close it, with the reason the record is to carry. Returns what the repository did,
    /// which replaces the episode's `repository` field on the way out.
    ///
    /// ```
    /// use majordomus_cli::episodes::{CloseReason, EpisodeDriver, NullDriver};
    /// let said = NullDriver.close("generic", "t1", CloseReason::Expired);
    /// assert!(said.contains("without a driver"));
    /// ```
    fn close(&self, provider: &str, external_id: &str, reason: CloseReason) -> String;
}

/// The driver that runs the repository's own `majordomus capture session`.
///
/// It finds the tool the way a provider hook's shim does and in the same order — the
/// repository's `bin/majordomus`, an installation under `.majordomus/`, then whatever is on
/// the path — because a second resolution order would be a second answer to "which tool is
/// this repository's", and the shims are the ones that have been right about it for months.
///
/// When no tool is found it records nothing and says why, in the episode's own field:
///
/// ```
/// use majordomus_cli::episodes::{EpisodeDriver, ToolDriver};
/// let empty = std::env::temp_dir().join("mj-tooldriver-doctest-no-tool");
/// std::env::remove_var("PATH"); // nor any majordomus on the path
/// let said = ToolDriver::new(&empty).open("generic", "t1");
/// assert!(said.starts_with("no episode was recorded"), "{said}");
/// ```
#[derive(Debug, Clone)]
pub struct ToolDriver {
    root: PathBuf,
}

impl ToolDriver {
    /// A driver for the repository at `root`. Nothing is resolved or run here: the tool is
    /// looked for on every event, so a `bin/majordomus` built after the server started is the
    /// one used.
    ///
    /// ```
    /// use majordomus_cli::episodes::ToolDriver;
    /// let driver = ToolDriver::new("/srv/repo");
    /// assert!(format!("{driver:?}").contains("/srv/repo"));
    /// ```
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
        self.run_tool(self.tool(), provider, event, payload)
    }

    /// Run `tool`'s `capture session` with `payload` on stdin, or say that there is no tool.
    fn run_tool(
        &self,
        tool: Option<PathBuf>,
        provider: &str,
        event: &str,
        payload: &str,
    ) -> String {
        let Some(tool) = tool else {
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
            .args([
                "capture",
                "session",
                "--provider",
                provider,
                "--event",
                event,
            ])
            .current_dir(&self.root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                return format!(
                    "no episode was recorded: {} did not start: {e}",
                    tool.display()
                )
            }
        };
        // stdin was asked for as a pipe, so it is there; it is closed when the write is done,
        // and a tool that exited without reading it has answered all the same
        let _ = child.stdin.take().map(|mut stdin| {
            use std::io::Write;
            stdin.write_all(payload.as_bytes())
        });
        reported(event, &tool, child.wait_with_output())
    }
}

/// What a finished `capture session` run says happened: its last line on stderr, or, when it
/// said nothing or could not be waited for, a sentence that says so and names the tool.
fn reported(event: &str, tool: &Path, finished: std::io::Result<std::process::Output>) -> String {
    match finished {
        // `capture session` writes nothing to stdout by contract and reports what it did
        // on stderr, because on a start event its stdout would be loaded into a model's
        // context. Its last line is the one that says what happened.
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stderr);
            let last = text
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("");
            if last.is_empty() {
                format!(
                    "`capture session --event {event}` exited {} and said nothing",
                    out.status
                )
            } else {
                last.trim().to_string()
            }
        }
        Err(e) => format!(
            "no episode was recorded: {} did not finish: {e}",
            tool.display()
        ),
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
///
/// ```
/// use majordomus_cli::episodes::{EpisodeBoard, NullDriver};
/// use majordomus_cli::peers::{PeerBoard, Transport};
/// let board = EpisodeBoard::with_driver(Box::new(NullDriver));
/// let peer = PeerBoard::new().attach(Transport::Stdio);
/// let (episode, _) = board.attach(&peer, "t1", "generic");
/// assert_eq!(episode.repository, "no repository episode: this board is running without a driver");
/// ```
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
    /// A board that records nothing in the repository: every episode's `repository` field
    /// says so. The board the unit suites and a repository-less server use.
    ///
    /// ```
    /// use majordomus_cli::episodes::EpisodeBoard;
    /// let board = EpisodeBoard::new();
    /// assert!(board.list().is_empty(), "a new board holds no episode");
    /// ```
    pub fn new() -> Self {
        EpisodeBoard {
            episodes: Mutex::new(BTreeMap::new()),
            driver: Box::new(NullDriver),
        }
    }

    /// A board that drives the repository at `root` through its own `capture session`, by
    /// a [`ToolDriver`]. Constructing it runs nothing; the first attach does.
    ///
    /// ```
    /// use majordomus_cli::episodes::EpisodeBoard;
    /// let board = EpisodeBoard::for_repository(std::env::temp_dir());
    /// assert!(board.list().is_empty(), "no command ran and no episode exists yet");
    /// assert_eq!(board.close_all().len(), 0);
    /// ```
    pub fn for_repository(root: impl Into<PathBuf>) -> Self {
        EpisodeBoard {
            episodes: Mutex::new(BTreeMap::new()),
            driver: Box::new(ToolDriver::new(root)),
        }
    }

    /// A board over an arbitrary driver: the seam a test, or a server with another way of
    /// recording, plugs into. The driver is asked once per opened and once per closed episode.
    ///
    /// ```
    /// use majordomus_cli::episodes::{CloseReason, EpisodeBoard, EpisodeDriver};
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// struct Fixed;
    /// impl EpisodeDriver for Fixed {
    ///     fn open(&self, _: &str, _: &str) -> String { "opened".into() }
    ///     fn close(&self, _: &str, _: &str, _: CloseReason) -> String { "closed".into() }
    /// }
    /// let board = EpisodeBoard::with_driver(Box::new(Fixed));
    /// let peer = PeerBoard::new().attach(Transport::Stdio);
    /// assert_eq!(board.attach(&peer, "t", "generic").0.repository, "opened");
    /// ```
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
    ///
    /// ```
    /// use majordomus_cli::episodes::EpisodeBoard;
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let peers = PeerBoard::new();
    /// let board = EpisodeBoard::new();
    /// let (p1, p2) = (peers.attach(Transport::Stdio), peers.attach(Transport::Http));
    /// assert!(!board.attach(&p1, "work", "generic").1, "first: opened");
    /// let (episode, resumed) = board.attach(&p2, "work", "generic");
    /// assert!(resumed, "same identity, another peer: resumed");
    /// assert_eq!((episode.attachments, episode.peer.as_ref()), (2, Some(&p2)));
    /// assert_eq!(board.list().len(), 1, "one episode, not two");
    /// ```
    pub fn attach(
        &self,
        peer: &PeerId,
        external_id: &str,
        provider: &str,
    ) -> (ConnectionEpisode, bool) {
        let mut episodes = self.lock();
        // A connection holds zero or one episode. A peer attaching to a second external
        // identity is not a second worker, it is the same connection changing its mind —
        // the first is released (and left detached, recoverable) rather than held by a peer
        // that has stopped speaking for it.
        for entry in episodes.values_mut().filter(|e| {
            e.episode.peer.as_ref() == Some(peer) && e.episode.external_id != external_id
        }) {
            entry.episode.peer = None;
            entry.episode.state = ConnectionEpisodeState::Detached;
            entry.detached_since = Some(Instant::now());
        }

        if let Some(entry) = episodes.get_mut(external_id) {
            entry.episode.peer = Some(peer.clone());
            entry.episode.state = ConnectionEpisodeState::Open;
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
        let episode = ConnectionEpisode {
            external_id: external_id.to_string(),
            provider: provider.to_string(),
            peer: Some(peer.clone()),
            opened_at: crate::peers::rfc3339(SystemTime::now()),
            last_activity_seconds_ago: 0,
            attachments: 1,
            state: ConnectionEpisodeState::Open,
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
    ///
    /// ```
    /// use std::time::Duration;
    /// use majordomus_cli::episodes::EpisodeBoard;
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let peer = PeerBoard::new().attach(Transport::Http);
    /// let board = EpisodeBoard::new();
    /// board.attach(&peer, "busy", "generic");
    /// board.touch(&peer);
    /// assert_eq!(board.list()[0].last_activity_seconds_ago, 0);
    /// assert!(board.reap(Duration::ZERO).is_empty(), "a held episode is never reaped");
    /// ```
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
    ///
    /// ```
    /// use majordomus_cli::episodes::{ConnectionEpisodeState, EpisodeBoard};
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let peer = PeerBoard::new().attach(Transport::Http);
    /// let board = EpisodeBoard::new();
    /// board.attach(&peer, "kept", "generic");
    /// let detached = board.detach(&peer);
    /// assert_eq!(detached.len(), 1);
    /// assert!(detached[0].peer.is_none());
    /// assert_eq!(board.list()[0].state, ConnectionEpisodeState::Detached, "still on the board");
    /// assert!(board.detach(&peer).is_empty(), "nothing is held twice");
    /// ```
    pub fn detach(&self, peer: &PeerId) -> Vec<ConnectionEpisode> {
        let mut episodes = self.lock();
        let mut out = Vec::new();
        for entry in episodes.values_mut() {
            if entry.episode.peer.as_ref() == Some(peer) {
                entry.episode.peer = None;
                entry.episode.state = ConnectionEpisodeState::Detached;
                entry.detached_since = Some(Instant::now());
                out.push(entry.episode.clone());
            }
        }
        out
    }

    /// Close one episode deliberately, by name: the client said it was done.
    ///
    /// ```
    /// use majordomus_cli::episodes::{CloseReason, EpisodeBoard};
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let peer = PeerBoard::new().attach(Transport::Stdio);
    /// let board = EpisodeBoard::new();
    /// board.attach(&peer, "done", "generic");
    /// let closed = board.close("done", CloseReason::Detach).expect("it was open");
    /// assert!(closed.peer.is_none());
    /// assert!(board.close("done", CloseReason::Detach).is_none(), "closed once only");
    /// assert!(board.of_peer(&peer).is_none());
    /// ```
    pub fn close(&self, external_id: &str, reason: CloseReason) -> Option<ConnectionEpisode> {
        let mut episodes = self.lock();
        let entry = episodes.remove(external_id)?;
        let mut episode = entry.episode;
        episode.repository = self
            .driver
            .close(&episode.provider, &episode.external_id, reason);
        episode.peer = None;
        episode.state = ConnectionEpisodeState::Detached;
        Some(episode)
    }

    /// Close every episode detached longer than `grace`. The reaper, and the answer to the
    /// client that never comes back. Returns what it closed, each as
    /// [`CloseReason::Expired`], which the record reads as interrupted.
    ///
    /// ```
    /// use std::time::Duration;
    /// use majordomus_cli::episodes::EpisodeBoard;
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let peer = PeerBoard::new().attach(Transport::Http);
    /// let board = EpisodeBoard::new();
    /// board.attach(&peer, "gone", "generic");
    /// board.detach(&peer);
    /// assert!(board.reap(Duration::from_secs(3600)).is_empty(), "inside the grace it waits");
    /// assert_eq!(board.reap(Duration::ZERO).len(), 1);
    /// assert!(board.list().is_empty());
    /// ```
    pub fn reap(&self, grace: Duration) -> Vec<ConnectionEpisode> {
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
    ///
    /// ```
    /// use majordomus_cli::episodes::EpisodeBoard;
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let peers = PeerBoard::new();
    /// let board = EpisodeBoard::new();
    /// board.attach(&peers.attach(Transport::Stdio), "held", "generic");
    /// let detached = peers.attach(Transport::Http);
    /// board.attach(&detached, "waiting", "generic");
    /// board.detach(&detached);
    /// assert_eq!(board.close_all().len(), 2, "held and detached alike");
    /// assert!(board.list().is_empty());
    /// ```
    pub fn close_all(&self) -> Vec<ConnectionEpisode> {
        let ids: Vec<String> = self.lock().keys().cloned().collect();
        ids.into_iter()
            .filter_map(|id| self.close(&id, CloseReason::Shutdown))
            .collect()
    }

    /// Every episode this server holds, open and detached, in external-identity order, each
    /// with its idle time measured now rather than when it was last written.
    ///
    /// ```
    /// use majordomus_cli::episodes::{ConnectionEpisodeState, EpisodeBoard};
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let peers = PeerBoard::new();
    /// let board = EpisodeBoard::new();
    /// let p = peers.attach(Transport::Stdio);
    /// board.attach(&p, "b", "generic");
    /// board.detach(&p);
    /// board.attach(&peers.attach(Transport::Http), "a", "generic");
    /// let states: Vec<_> = board.list().into_iter().map(|e| (e.external_id, e.state)).collect();
    /// assert_eq!(states, [("a".to_string(), ConnectionEpisodeState::Open),
    ///                     ("b".to_string(), ConnectionEpisodeState::Detached)]);
    /// ```
    pub fn list(&self) -> Vec<ConnectionEpisode> {
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

    /// The episode a connection holds, if it holds one. A connection that only initialised
    /// holds none, and neither does one whose episode was detached from it.
    ///
    /// ```
    /// use majordomus_cli::episodes::EpisodeBoard;
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let peer = PeerBoard::new().attach(Transport::Http);
    /// let board = EpisodeBoard::new();
    /// assert!(board.of_peer(&peer).is_none(), "a peer is not a worker until it attaches");
    /// board.attach(&peer, "mine", "generic");
    /// assert_eq!(board.of_peer(&peer).map(|e| e.external_id).as_deref(), Some("mine"));
    /// ```
    pub fn of_peer(&self, peer: &PeerId) -> Option<ConnectionEpisode> {
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
    std::env::var_os("PATH").and_then(|path| find_in(&path, name))
}

/// The first executable `name` in the directories of a `PATH`-shaped list.
fn find_in(path: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|d| d.join(name))
        .find(|p| is_executable(p))
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
            self.calls.lock().unwrap().push(format!(
                "close {provider} {external_id} {}",
                reason.as_str()
            ));
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
        assert_eq!(
            board.list().len(),
            1,
            "a detach must not discard the episode"
        );
        assert_eq!(board.list()[0].state, ConnectionEpisodeState::Detached);

        // a new connection: a different peer id, the same worker
        let second = ids[1].clone();
        let (again, resumed) = board.attach(&second, "thread-7", "generic");
        assert!(
            resumed,
            "the same external identity must resume, not re-open"
        );
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
        assert!(
            board.of_peer(&p).is_none(),
            "attaching nothing holds nothing"
        );
        board.attach(&p, "one", "generic");
        board.attach(&p, "two", "generic");
        assert_eq!(board.of_peer(&p).map(|e| e.external_id), Some("two".into()));
        let one = board
            .list()
            .into_iter()
            .find(|e| e.external_id == "one")
            .unwrap();
        assert_eq!(
            one.state,
            ConnectionEpisodeState::Detached,
            "the episode it stopped speaking for is released, and recoverable"
        );
    }

    /// The reaper closes what never came back, and only that.
    #[test]
    fn the_reaper_closes_only_what_is_past_its_grace() {
        let (board, rec) = board();
        let p = peers(1).remove(0);
        board.attach(&p, "gone", "generic");
        assert!(
            board.reap(Duration::from_secs(1)).is_empty(),
            "an attached episode is never reaped"
        );
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

    /// Every control character is escaped, the named ones by name and the rest by code, so
    /// no identity can end the payload's line or smuggle a byte a JSON reader refuses.
    #[test]
    fn every_control_character_is_escaped() {
        assert_eq!(json_string("a\rb\tc"), "\"a\\rb\\tc\"");
        assert_eq!(json_string("\u{1}\u{1f}"), "\"\\u0001\\u001f\"");
        let parsed: String = serde_json::from_str(&json_string("x\r\t\u{7}y")).unwrap();
        assert_eq!(
            parsed, "x\r\t\u{7}y",
            "what is escaped reads back as it was"
        );
    }

    /// A board built without a driver still keeps the episodes; what it reports for the
    /// repository is that there is no driver, never a record that was not made.
    #[test]
    fn a_board_without_a_driver_keeps_episodes_and_says_none_was_recorded() {
        for board in [EpisodeBoard::new(), EpisodeBoard::default()] {
            let p = peers(1).remove(0);
            let (episode, _) = board.attach(&p, "quiet", "generic");
            assert_eq!(
                episode.repository,
                "no repository episode: this board is running without a driver"
            );
            assert_eq!(format!("{board:?}"), "EpisodeBoard { episodes: 1 }");
            let closed = board.close("quiet", CloseReason::Detach).unwrap();
            assert_eq!(closed.repository, episode.repository);
            assert_eq!(format!("{board:?}"), "EpisodeBoard { episodes: 0 }");
        }
    }

    /// Activity, a lost connection and a deliberate close each reach only the episode they
    /// name; closing one nobody holds is nothing, not an error and not somebody else's.
    #[test]
    fn touch_detach_and_close_reach_only_their_own_episode() {
        let (board, _) = board();
        let ids = peers(2);
        board.attach(&ids[0], "mine", "generic");
        board.attach(&ids[1], "theirs", "generic");
        let activity = |id: &str| board.lock().get(id).unwrap().last_activity;
        let theirs_before = activity("theirs");
        let mine_before = activity("mine");
        std::thread::sleep(Duration::from_millis(5));
        board.touch(&ids[0]);
        assert!(
            activity("mine") > mine_before,
            "the toucher's episode moved"
        );
        assert_eq!(activity("theirs"), theirs_before, "the other did not");

        let detached = board.detach(&ids[0]);
        assert_eq!(detached.len(), 1);
        assert_eq!(detached[0].external_id, "mine");
        assert_eq!(
            board.of_peer(&ids[1]).map(|e| e.state),
            Some(ConnectionEpisodeState::Open),
            "a detach of one connection leaves the other open"
        );

        assert!(board
            .close("nobody-holds-this", CloseReason::Detach)
            .is_none());
        assert_eq!(board.list().len(), 2);
    }

    /// A thread that panicked while holding the board does not take the board with it: the
    /// map it guarded is still whole and the next attach is served.
    #[test]
    fn a_poisoned_board_still_answers() {
        let board = std::sync::Arc::new(EpisodeBoard::new());
        let held = std::sync::Arc::clone(&board);
        let _ = std::thread::spawn(move || {
            let _guard = held.episodes.lock().unwrap();
            panic!("a handler died holding the board");
        })
        .join();
        assert!(board.episodes.is_poisoned());
        let p = peers(1).remove(0);
        board.attach(&p, "after", "generic");
        assert_eq!(board.list().len(), 1);
    }

    /// The tool on the path is the first executable of that name, in path order; a file that
    /// is there but cannot be run is not a tool.
    #[test]
    fn the_tool_on_the_path_is_the_first_executable_one() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let (plain, runnable) = (dir.path().join("plain"), dir.path().join("runnable"));
        for (d, mode) in [(&plain, 0o644), (&runnable, 0o755)] {
            std::fs::create_dir_all(d).unwrap();
            let tool = d.join("majordomus");
            std::fs::write(&tool, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        let path = std::env::join_paths([&plain, &runnable]).unwrap();
        assert_eq!(
            find_in(&path, "majordomus"),
            Some(runnable.join("majordomus"))
        );
        assert_eq!(find_in(&path, "absent"), None);
        assert_eq!(
            find_in(&std::env::join_paths([&plain]).unwrap(), "majordomus"),
            None
        );
    }

    /// Write `body` as the repository's own `bin/majordomus`, runnable.
    fn tool_at(root: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let tool = root.join("bin/majordomus");
        std::fs::create_dir_all(tool.parent().unwrap()).unwrap();
        std::fs::write(&tool, body).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        tool
    }

    /// The repository's own tool is asked exactly as a provider hook asks it — the
    /// repository, `capture session`, the provider and the event as arguments, the payload
    /// on stdin — and what the episode reports is the last thing it said.
    #[test]
    fn the_repository_tool_is_asked_as_a_hook_asks_it_and_its_last_word_is_the_answer() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        tool_at(
            root,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\ncat > stdin.txt\nprintf 'noise\\n\\n  episode e1 recorded  \\n\\n' >&2\n",
        );
        let driver = ToolDriver::new(root);
        assert_eq!(driver.open("generic", "e1"), "episode e1 recorded");
        let args = std::fs::read_to_string(root.join("args.txt")).unwrap();
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            vec![
                "--repo",
                root.to_str().unwrap(),
                "capture",
                "session",
                "--provider",
                "generic",
                "--event",
                "start"
            ]
        );
        assert_eq!(
            std::fs::read_to_string(root.join("stdin.txt")).unwrap(),
            "{\"session_id\":\"e1\",\"source\":\"attach\"}\n"
        );
        assert_eq!(
            driver.close("generic", "e1", CloseReason::Expired),
            "episode e1 recorded"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("stdin.txt")).unwrap(),
            "{\"session_id\":\"e1\",\"reason\":\"expired\"}\n"
        );
    }

    /// Every way the tool can fail to say what it did is said instead: no tool at all, a
    /// tool that cannot start, a tool that said nothing, a tool that could not be waited for.
    #[test]
    fn every_way_the_tool_fails_to_answer_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let driver = ToolDriver::new(root);

        assert_eq!(
            driver.run_tool(None, "generic", "start", "{}"),
            format!(
                "no episode was recorded: neither bin/majordomus nor .majordomus/bin/majordomus is executable under {} and none is on the path",
                root.display()
            )
        );

        tool_at(root, "#!/bin/sh\nexit 3\n");
        assert_eq!(
            driver.close("generic", "e1", CloseReason::Detach),
            "`capture session --event end` exited exit status: 3 and said nothing"
        );

        // a tool that was found and is gone by the time it is started
        let gone = root.join("bin/gone");
        let refused = driver.run_tool(Some(gone.clone()), "generic", "start", "{}");
        assert!(
            refused.starts_with(&format!(
                "no episode was recorded: {} did not start: ",
                gone.display()
            )),
            "{refused}"
        );

        assert_eq!(
            reported(
                "end",
                Path::new("/bin/majordomus"),
                Err(std::io::Error::other("the pipe broke"))
            ),
            "no episode was recorded: /bin/majordomus did not finish: the pipe broke"
        );
    }
}
