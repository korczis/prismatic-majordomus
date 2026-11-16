//! When a server that no client owns is idle, and when it leaves (I4007).
//!
//! A server `serve ensure` starts for a checkout carries an idle life: the declared
//! `server.idle_seconds:` in `.ai/repo/policy.yaml` (900 when nothing is declared), passed to
//! it as `serve --idle`. It is meant to end when nobody uses it. On 2026-10-22 fifteen of them
//! had run for up to eight hours on one Mac with no peer attached, each holding a port, a
//! lease and 60-450 MB, because the idle timer counted *every* HTTP request as use — and the
//! machine's servers talk to each other all the time. Every server on a machine hears the
//! mesh's multicast and links to the others (`POST /api/v1/mesh/link/hello`, `.../sync`), a
//! board gather from any checkout reads every sibling's board (`GET /api/v1/peers?
//! checkouts=this`), and `serve status`, `serve ensure` and the gather probe every lease's
//! server (`GET /`). Fifteen servers kept each other alive.
//!
//! Idle is decided here from state the server already holds, never from a guess:
//!
//! - no MCP session attached and no row on this checkout's peer board;
//! - no execution queued, starting, running or cancelling;
//! - no live `/events` channel open;
//! - no request from a client answered since the last look: a request another Majordomus
//!   process sends to keep the machine's servers in touch ([`counts_as_use`]) is not one.
//!
//! The lease is not part of the question. Every `serve` that is still running holds its own
//! checkout's lease — the election ends any other at once — and a server whose lease was
//! taken over ends when its last session has been handed on (I2127). A server started without
//! an idle life (plain `serve`, a hosted deployment, the server `majordomus mcp` hosts for its
//! client's stdio session) never idle-exits: its owner is whoever started it.
//!
//! When the bound is reached the server logs why ([`IdleExit`]), runs its ordered stop —
//! sessions and episodes closed, listeners closed, mesh drained, lease released — and `serve`
//! returns 0. That is the server leaving on its own, which ADR 0200 permits; nothing here
//! signals or kills another process.
//!
//! The lifecycle, as the serve loop runs it once a second:
//!
//! ```
//! use std::time::{Duration, Instant};
//! use majordomus_cli::idle::{counts_as_use, Activity, IdleTimer};
//!
//! let start = Instant::now();
//! let mut timer = IdleTimer::new(Duration::from_secs(3), start);
//! let at = |s| start + Duration::from_secs(s);
//! // every second another server's mesh sync arrives: it is not a client
//! let sync = counts_as_use("POST", "/api/v1/mesh/link/sync", false);
//! let look = Activity { client_request: sync, ..Activity::default() };
//! assert!(timer.observe(&look, at(1)).is_none());
//! assert!(timer.observe(&look, at(2)).is_none());
//! // three idle seconds: the server logs why, stops in order and exits 0
//! let why = timer.observe(&look, at(3)).unwrap();
//! assert!(why.to_string().contains("machine traffic"));
//! ```

use std::fmt;
use std::time::{Duration, Instant};

/// The header this executable's own probes carry (`lease::probe` and every caller of it), so
/// that a server tells "another Majordomus process asked whether I am here" apart from a
/// person or a client reading its home page. A probe from an older executable lacks it and
/// still counts as use, which errs on the side of keeping a server.
pub const PROBE_HEADER: &str = "X-Majordomus-Probe";

/// The paths of the mesh's runtime-to-runtime transport: a link's hello and sync, and a
/// registration with a rendezvous hub. Only other runtimes send these.
const MESH_TRANSPORT: [&str; 3] = [
    crate::mesh::link::HELLO_PATH,
    crate::mesh::link::SYNC_PATH,
    crate::mesh::rendezvous::REGISTER_PATH,
];

/// Whether an HTTP request is a client using this server, or another Majordomus process
/// keeping the machine's servers in touch — which must not keep an idle server alive.
///
/// Machine traffic, and nothing else, is:
///
/// - the mesh transport: `POST /api/v1/mesh/link/hello`, `.../link/sync`, `/api/v1/mesh/register`;
/// - a probe: `GET` or `HEAD /` carrying [`PROBE_HEADER`];
/// - a sibling's board gather: `GET /api/v1/peers?checkouts=this`, the one question a server
///   asks another checkout's server for its own board.
///
/// `target` is the request target as received, path and query.
///
/// ```
/// use majordomus_cli::idle::counts_as_use;
/// // another runtime linking, a probe, a sibling reading this board: not use
/// assert!(!counts_as_use("POST", "/api/v1/mesh/link/sync", false));
/// assert!(!counts_as_use("GET", "/", true));
/// assert!(!counts_as_use("GET", "/api/v1/peers?checkouts=this", false));
/// // a person on the home page, a client of the REST API, an MCP request: use
/// assert!(counts_as_use("GET", "/", false));
/// assert!(counts_as_use("GET", "/api/v1/peers", false));
/// assert!(counts_as_use("POST", "/mcp", false));
/// assert!(counts_as_use("GET", "/api/v1/mesh/state", false));
/// ```
pub fn counts_as_use(method: &str, target: &str, probe_header: bool) -> bool {
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (target, None),
    };
    if MESH_TRANSPORT.contains(&path) {
        return false;
    }
    let read = method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD");
    if read && path == "/" && probe_header {
        return false;
    }
    let sibling_board = read
        && path == "/api/v1/peers"
        && query.is_some_and(|q| q.split('&').any(|pair| pair == "checkouts=this"));
    !sibling_board
}

/// What a server holds at one look: the state idleness is decided from.
///
/// ```
/// use majordomus_cli::idle::Activity;
/// assert_eq!(Activity::default().busy_because(), None);
/// let busy = Activity { executions: 1, ..Activity::default() };
/// assert_eq!(busy.busy_because().as_deref(), Some("1 execution(s) queued or running"));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Activity {
    /// MCP sessions attached, or rows on this checkout's peer board, whichever is more.
    pub peers: usize,
    /// Executions queued, starting, running or cancelling.
    pub executions: usize,
    /// Live `/events` channels open.
    pub channels: usize,
    /// Whether a client's request was answered since the last look ([`counts_as_use`]).
    pub client_request: bool,
}

impl Activity {
    /// What keeps the server in use, in words; `None` when nothing does.
    ///
    /// ```
    /// use majordomus_cli::idle::Activity;
    /// let watched = Activity { peers: 1, channels: 1, ..Activity::default() };
    /// assert_eq!(
    ///     watched.busy_because().as_deref(),
    ///     Some("1 peer(s) attached, 1 live channel(s) open")
    /// );
    /// assert_eq!(Activity::default().busy_because(), None);
    /// ```
    pub fn busy_because(&self) -> Option<String> {
        let mut why = Vec::new();
        if self.peers > 0 {
            why.push(format!("{} peer(s) attached", self.peers));
        }
        if self.executions > 0 {
            why.push(format!(
                "{} execution(s) queued or running",
                self.executions
            ));
        }
        if self.channels > 0 {
            why.push(format!("{} live channel(s) open", self.channels));
        }
        if self.client_request {
            why.push("a client request answered".to_string());
        }
        (!why.is_empty()).then(|| why.join(", "))
    }

    /// Whether nothing keeps the server in use.
    ///
    /// ```
    /// use majordomus_cli::idle::Activity;
    /// assert!(Activity::default().is_idle());
    /// assert!(!Activity { peers: 1, ..Activity::default() }.is_idle());
    /// ```
    pub fn is_idle(&self) -> bool {
        self.busy_because().is_none()
    }
}

/// The idle clock of one server: reset by every look that finds it in use, and due when it
/// has been idle for the whole bound. Deterministic: the caller passes the time.
///
/// ```
/// use std::time::{Duration, Instant};
/// use majordomus_cli::idle::{Activity, IdleTimer};
/// let start = Instant::now();
/// let mut timer = IdleTimer::new(Duration::from_secs(10), start);
/// // idle, but not for long enough
/// assert!(timer.observe(&Activity::default(), start + Duration::from_secs(9)).is_none());
/// // a peer attached resets the clock
/// let busy = Activity { peers: 1, ..Activity::default() };
/// assert!(timer.observe(&busy, start + Duration::from_secs(9)).is_none());
/// assert!(timer.observe(&Activity::default(), start + Duration::from_secs(18)).is_none());
/// // ten idle seconds after the peer left, the server is due to leave
/// let due = timer.observe(&Activity::default(), start + Duration::from_secs(19)).unwrap();
/// assert_eq!(due.idle_for, Duration::from_secs(10));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct IdleTimer {
    bound: Duration,
    since: Instant,
}

impl IdleTimer {
    /// A clock with `bound`, idle from `now` (a server is idle from the moment it starts).
    ///
    /// ```
    /// use std::time::{Duration, Instant};
    /// use majordomus_cli::idle::{Activity, IdleTimer};
    /// let start = Instant::now();
    /// let mut timer = IdleTimer::new(Duration::from_secs(2), start);
    /// // nobody ever came: due two seconds after the start
    /// assert!(timer.observe(&Activity::default(), start + Duration::from_secs(2)).is_some());
    /// ```
    pub fn new(bound: Duration, now: Instant) -> Self {
        Self { bound, since: now }
    }

    /// Look once: `Some` when the server has been idle for the whole bound.
    ///
    /// ```
    /// use std::time::{Duration, Instant};
    /// use majordomus_cli::idle::{Activity, IdleTimer};
    /// let start = Instant::now();
    /// let mut timer = IdleTimer::new(Duration::from_secs(5), start);
    /// // an execution queued or running holds the clock at every look
    /// let running = Activity { executions: 1, ..Activity::default() };
    /// assert!(timer.observe(&running, start + Duration::from_secs(60)).is_none());
    /// assert!(timer.observe(&Activity::default(), start + Duration::from_secs(64)).is_none());
    /// let due = timer.observe(&Activity::default(), start + Duration::from_secs(65)).unwrap();
    /// assert_eq!(due.idle_for, Duration::from_secs(5));
    /// ```
    pub fn observe(&mut self, activity: &Activity, now: Instant) -> Option<IdleExit> {
        if !activity.is_idle() {
            self.since = now;
            return None;
        }
        let idle_for = now.saturating_duration_since(self.since);
        (idle_for >= self.bound).then_some(IdleExit {
            idle_for,
            bound: self.bound,
        })
    }
}

/// Why an idle server leaves: what the log line says.
///
/// ```
/// use std::time::Duration;
/// use majordomus_cli::idle::IdleExit;
/// let why = IdleExit { idle_for: Duration::from_secs(900), bound: Duration::from_secs(900) };
/// assert!(why.to_string().starts_with("idle for 900 s (bound 900 s)"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleExit {
    /// How long nothing used the server.
    pub idle_for: Duration,
    /// The idle life it was given.
    pub bound: Duration,
}

impl fmt::Display for IdleExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "idle for {} s (bound {} s): no peer attached, no execution queued or running, \
             no live channel, no client request; machine traffic (mesh links, probes, sibling \
             board reads) is not use",
            self.idle_for.as_secs(),
            self.bound.as_secs()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_traffic_is_not_use() {
        for (method, target, probe) in [
            ("POST", "/api/v1/mesh/link/hello", false),
            ("POST", "/api/v1/mesh/link/sync", false),
            ("POST", "/api/v1/mesh/register", false),
            ("GET", "/", true),
            ("HEAD", "/", true),
            ("GET", "/api/v1/peers?checkouts=this", false),
            ("GET", "/api/v1/peers?format=json&checkouts=this", false),
        ] {
            assert!(
                !counts_as_use(method, target, probe),
                "{method} {target} (probe header {probe}) was counted as use"
            );
        }
    }

    #[test]
    fn a_client_is_use() {
        for (method, target, probe) in [
            ("GET", "/", false),
            ("GET", "/cockpit", false),
            ("POST", "/mcp", false),
            ("GET", "/api/v1/peers", false),
            ("GET", "/api/v1/peers?checkouts=repository", false),
            ("POST", "/api/v1/executions/start", false),
            ("GET", "/api/v1/mesh/state", false),
            // a probe header on anything but the home page proves nothing
            ("GET", "/api/v1/server", true),
        ] {
            assert!(
                counts_as_use(method, target, probe),
                "{method} {target} (probe header {probe}) was not counted as use"
            );
        }
    }

    #[test]
    fn each_kind_of_activity_keeps_the_server() {
        let idle = Activity::default();
        assert!(idle.is_idle());
        for busy in [
            Activity { peers: 2, ..idle },
            Activity {
                executions: 1,
                ..idle
            },
            Activity {
                channels: 1,
                ..idle
            },
            Activity {
                client_request: true,
                ..idle
            },
        ] {
            assert!(!busy.is_idle(), "{busy:?} was judged idle");
        }
        let all = Activity {
            peers: 1,
            executions: 2,
            channels: 3,
            client_request: true,
        };
        assert_eq!(
            all.busy_because().unwrap(),
            "1 peer(s) attached, 2 execution(s) queued or running, 3 live channel(s) open, \
             a client request answered"
        );
    }

    #[test]
    fn the_clock_is_due_only_after_a_whole_idle_bound() {
        let t0 = Instant::now();
        let bound = Duration::from_secs(5);
        let mut timer = IdleTimer::new(bound, t0);
        assert!(timer.observe(&Activity::default(), t0).is_none());
        assert!(timer
            .observe(&Activity::default(), t0 + Duration::from_millis(4_999))
            .is_none());
        let due = timer
            .observe(&Activity::default(), t0 + bound)
            .expect("due at the bound");
        assert_eq!(
            due,
            IdleExit {
                idle_for: bound,
                bound
            }
        );
    }

    #[test]
    fn a_running_execution_holds_the_clock_back() {
        let t0 = Instant::now();
        let mut timer = IdleTimer::new(Duration::from_secs(1), t0);
        let running = Activity {
            executions: 1,
            ..Activity::default()
        };
        for s in 0..10 {
            assert!(timer
                .observe(&running, t0 + Duration::from_secs(s))
                .is_none());
        }
        // it finished at 9 s: idle from then, due one second later and not before
        assert!(timer
            .observe(&Activity::default(), t0 + Duration::from_millis(9_500))
            .is_none());
        assert!(timer
            .observe(&Activity::default(), t0 + Duration::from_secs(10))
            .is_some());
    }

    #[test]
    fn a_time_before_the_start_is_not_idle_time() {
        let t0 = Instant::now() + Duration::from_secs(60);
        let mut timer = IdleTimer::new(Duration::from_secs(1), t0);
        assert!(timer
            .observe(&Activity::default(), t0 - Duration::from_secs(30))
            .is_none());
    }
}
