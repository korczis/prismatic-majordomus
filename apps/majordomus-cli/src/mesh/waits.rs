//! Every wait of the executable on another session or runtime, with its bound and where it
//! goes when the bound is reached (I3993).
//!
//! The owner's order of 2026-10-22 is that nothing stalls on a stuck peer. A wait on a peer
//! is a place where it can: a server that does not answer its election, a host whose derive
//! lock is held by a process nobody watches, a session that holds a scope and does nothing.
//! [`PEER_WAITS`] lists every one the executable has — not the shell's, whose callers bound
//! them (`project.every-wait-is-bounded`, `scripts/liveness-check`) — each with:
//!
//! - its [`Bound`]: read from a declaration (a policy key, a mesh or offload declaration
//!   key, a command's flag) with the default the executable falls back on, a constant of
//!   the executable, or no wait at all because the call answers at once;
//! - its [`Routing`] at the bound: another host, another seat, a take-over, serving alone,
//!   or a report that names what was waited for and who held it ([`PeerWait::reached`]).
//!
//! `mesh doctor`'s `peer_waits` check ([`super::doctor::peer_waits_check`]) refuses an entry
//! that is not bounded, which a declaration can make so: `cooperation.session_ttl_seconds: 0`
//! turns the progress rule off, and a claim held by a session that does nothing then
//! excludes everybody for as long as its runtime beats.
//!
//! The registry is a declaration, not a description: each entry names the file it lives in
//! and a line of it, and the tests fail when that line is gone, so a wait that moves takes its
//! entry with it.
//!
//! ```
//! use majordomus_cli::mesh::waits::{find, Routing, PEER_WAITS};
//! use std::time::Duration;
//!
//! // the offload queue is bounded by its declaration, four hours unless it says otherwise
//! let queue = find("offload.queue").unwrap();
//! assert_eq!(queue.bound_with(&|_| None), Some(Duration::from_secs(14_400)));
//! assert_eq!(queue.routing, Routing::AnotherHost);
//!
//! // and under the defaults no wait of the executable is unbounded
//! assert!(PEER_WAITS.iter().all(|w| w.bound_with(&|_| None).is_some()));
//! ```

use std::time::Duration;

/// How long a wait may last.
///
/// ```
/// use majordomus_cli::mesh::waits::{find, Bound};
/// // the offload queue is bounded by a key of its declaration, with a default
/// match find("offload.queue").unwrap().bound {
///     Bound::Declared { default_seconds, .. } => assert_eq!(default_seconds, 14_400),
///     other => panic!("expected a declared bound, got {other:?}"),
/// }
/// // a claim admission answers at once
/// assert_eq!(find("mesh.claim.admission").unwrap().bound, Bound::NoWait);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    /// The executable never waits here: the call answers at once, with a refusal that names
    /// the holder or with a state the caller reads.
    NoWait,
    /// The value of a declaration, in seconds, and the default when nothing declares it.
    Declared {
        /// Where it is declared: a policy key, a declaration's key, or a command's flag.
        key: &'static str,
        /// What the executable uses when the declaration says nothing.
        default_seconds: u64,
        /// Whether a declared 0 turns the bound off rather than making the wait end at once.
        zero_turns_off: bool,
    },
    /// A constant of the executable, in seconds.
    Constant {
        /// The constant, as the source names it.
        name: &'static str,
        /// Its value.
        seconds: u64,
    },
    /// No bound at all: what `mesh doctor` refuses.
    Unbounded,
}

/// Where a wait goes when its bound is reached.
///
/// ```
/// use majordomus_cli::mesh::waits::{find, Routing};
/// // a full offload queue sends the work to another host
/// assert_eq!(find("offload.queue").unwrap().routing, Routing::AnotherHost);
/// assert_eq!(Routing::AnotherHost.as_str(), "another host");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    /// The holder is taken for gone and its place is taken over.
    TakeOver,
    /// This process serves its client alone instead of through the shared server.
    ServeAlone,
    /// The work goes to another host, or the caller takes it elsewhere.
    AnotherHost,
    /// The holder's seat is released to the next claimant.
    AnotherSeat,
    /// The wait stops with a report naming what it waited for and who held it.
    Report,
}

impl Routing {
    /// The word a report and the doctor use.
    ///
    /// ```
    /// use majordomus_cli::mesh::waits::Routing;
    /// assert_eq!(Routing::AnotherSeat.as_str(), "another seat");
    /// assert_eq!(Routing::Report.as_str(), "report");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Routing::TakeOver => "take over",
            Routing::ServeAlone => "serve alone",
            Routing::AnotherHost => "another host",
            Routing::AnotherSeat => "another seat",
            Routing::Report => "report",
        }
    }
}

/// One wait of the executable on another session or runtime.
///
/// ```
/// use majordomus_cli::mesh::waits::{find, PeerWait, PEER_WAITS};
/// let queue: &PeerWait = find("offload.queue").unwrap();
/// assert_eq!(queue.id, "offload.queue");
/// assert!(!queue.waits_for.is_empty() && !queue.at_bound.is_empty());
/// // every entry names the source file it lives in
/// assert!(PEER_WAITS.iter().all(|w| w.site.ends_with(".rs")));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerWait {
    /// A stable name, used in reports, in the doctor and in `docs/MESH.md`.
    pub id: &'static str,
    /// What is waited for, and on whom.
    pub waits_for: &'static str,
    /// The source file of the wait, relative to the crate's root.
    pub site: &'static str,
    /// A line of that file the wait is recognised by.
    pub anchor: &'static str,
    /// How long it may last.
    pub bound: Bound,
    /// Where it goes at the bound.
    pub routing: Routing,
    /// What happens at the bound, in a sentence.
    pub at_bound: &'static str,
}

impl PeerWait {
    /// The bound in force, reading a declared value through `declared` (a key to its seconds,
    /// `None` when nothing declares it); `None` when the wait is unbounded. A wait that never
    /// waits is bounded by zero.
    ///
    /// ```
    /// use majordomus_cli::mesh::waits::find;
    /// use std::time::Duration;
    /// let progress = find("mesh.session.progress").unwrap();
    /// assert_eq!(progress.bound_with(&|_| Some(120)), Some(Duration::from_secs(120)));
    /// // a declared 0 turns the progress rule off: nothing then bounds the wait
    /// assert_eq!(progress.bound_with(&|_| Some(0)), None);
    /// assert_eq!(find("mesh.claim.admission").unwrap().bound_with(&|_| None), Some(Duration::ZERO));
    /// ```
    pub fn bound_with(&self, declared: &dyn Fn(&str) -> Option<u64>) -> Option<Duration> {
        match self.bound {
            Bound::NoWait => Some(Duration::ZERO),
            Bound::Declared {
                key,
                default_seconds,
                zero_turns_off,
            } => {
                let seconds = declared(key).unwrap_or(default_seconds);
                (seconds > 0 || !zero_turns_off).then(|| Duration::from_secs(seconds))
            }
            Bound::Constant { seconds, .. } => Some(Duration::from_secs(seconds)),
            Bound::Unbounded => None,
        }
    }

    /// The bound as a reader is told it: its value and where it comes from.
    ///
    /// ```
    /// use majordomus_cli::mesh::waits::find;
    /// let queue = find("offload.queue").unwrap();
    /// assert_eq!(queue.bound_text(&|_| Some(60)), "60 s (derive-offload queue.wait_max_seconds)");
    /// assert_eq!(find("mesh.review.answer").unwrap().bound_text(&|_| None), "no wait: answered at once");
    /// ```
    pub fn bound_text(&self, declared: &dyn Fn(&str) -> Option<u64>) -> String {
        match (self.bound, self.bound_with(declared)) {
            (Bound::NoWait, _) => "no wait: answered at once".into(),
            (Bound::Declared { key, .. }, Some(d)) => format!("{} s ({key})", d.as_secs()),
            (Bound::Declared { key, .. }, None) => {
                format!("unbounded: {key} is 0, which turns it off")
            }
            (Bound::Constant { name, .. }, Some(d)) => format!("{} s ({name})", d.as_secs()),
            (_, _) => "unbounded".into(),
        }
    }

    /// What is said when the bound is reached: the wait, how long it lasted, what it waited
    /// for, who held it, and where it goes now. Every wait of the registry that can reach its
    /// bound says this, in these words, so that a person or a monitor can find it.
    ///
    /// ```
    /// use majordomus_cli::mesh::waits::find;
    /// use std::time::Duration;
    /// let queue = find("offload.queue").unwrap();
    /// let said = queue.reached(Duration::from_secs(14_400), "pid 4242 on lundra (alive), 2 waiter(s) ahead");
    /// assert!(said.starts_with("peer wait offload.queue reached its bound after 14400 s"), "{said}");
    /// assert!(said.contains("held by pid 4242 on lundra"));
    /// assert!(said.contains("another host:"));
    /// ```
    pub fn reached(&self, waited: Duration, held_by: &str) -> String {
        let held_by = if held_by.trim().is_empty() {
            "nobody it could name"
        } else {
            held_by
        };
        format!(
            "peer wait {} reached its bound after {} s waiting for {}, held by {held_by}; {}: {}",
            self.id,
            waited.as_secs(),
            self.waits_for,
            self.routing.as_str(),
            self.at_bound
        )
    }
}

/// The wait of the registry named `id`.
///
/// ```
/// use majordomus_cli::mesh::waits::find;
/// assert_eq!(find("worktree.lock").unwrap().site, "src/worktree/lock.rs");
/// assert!(find("no.such.wait").is_none());
/// ```
pub fn find(id: &str) -> Option<&'static PeerWait> {
    PEER_WAITS.iter().find(|w| w.id == id)
}

/// The waits of `waits` that nothing bounds once `declared` is read.
///
/// ```
/// use majordomus_cli::mesh::waits::{unbounded, PEER_WAITS};
/// assert!(unbounded(PEER_WAITS, &|_| None).is_empty());
/// let off = unbounded(PEER_WAITS, &|k| (k == "cooperation.session_ttl_seconds").then_some(0));
/// assert_eq!(off.iter().map(|w| w.id).collect::<Vec<_>>(), ["mesh.session.progress"]);
/// ```
pub fn unbounded<'a>(
    waits: &'a [PeerWait],
    declared: &dyn Fn(&str) -> Option<u64>,
) -> Vec<&'a PeerWait> {
    waits
        .iter()
        .filter(|w| w.bound_with(declared).is_none())
        .collect()
}

/// How long `monitor --follow` waits for its `--until` signal or its `--count` when no
/// `--timeout` is given: as long as the longest declared wait it usually stands for, a
/// merge behind the derive queue.
///
/// ```
/// use majordomus_cli::mesh::waits::MONITOR_UNTIL_BOUND;
/// assert_eq!(MONITOR_UNTIL_BOUND.as_secs(), 4 * 3600);
/// ```
pub const MONITOR_UNTIL_BOUND: Duration = Duration::from_secs(4 * 3600);

/// Every wait of the executable on another session or runtime.
pub const PEER_WAITS: &[PeerWait] = &[
    PeerWait {
        id: "server.election.busy",
        waits_for: "the shared server named by the lease, alive but not answering the election",
        site: "src/lease.rs",
        anchor: "if since.elapsed() < timings().busy_grace {",
        bound: Bound::Declared {
            key: "policy server.busy_grace_seconds",
            default_seconds: crate::lease::BUSY_GRACE.as_secs(),
            zero_turns_off: false,
        },
        routing: Routing::TakeOver,
        at_bound: "the lease is taken over and the log names the server that stopped answering",
    },
    PeerWait {
        id: "server.election.join",
        waits_for: "the shared server's lease to be joined or taken",
        site: "src/lease.rs",
        anchor: "if waited_since.elapsed() > timings().join_timeout {",
        bound: Bound::Declared {
            key: "policy server.join_timeout_seconds",
            default_seconds: crate::lease::JOIN_TIMEOUT.as_secs(),
            zero_turns_off: false,
        },
        routing: Routing::ServeAlone,
        at_bound: "the election fails naming the lease and its holder, and the client is served by its own process",
    },
    PeerWait {
        id: "server.lease.change",
        waits_for: "another process's change to the lease file (its flock)",
        site: "src/lease.rs",
        anchor: "const LOCK_PATIENCE: Duration",
        bound: Bound::Constant {
            name: "lease::LOCK_PATIENCE",
            seconds: crate::lease::LOCK_PATIENCE.as_secs(),
        },
        routing: Routing::Report,
        at_bound: "the change is not made and the log says so; a lease left in place is recovered by the next election",
    },
    PeerWait {
        id: "server.bridge.request",
        waits_for: "the shared server's answer to one MCP request",
        site: "src/mcp/bridge.rs",
        anchor: "pub const REQUEST_TIMEOUT: Duration",
        bound: Bound::Constant {
            name: "mcp::bridge::REQUEST_TIMEOUT",
            seconds: crate::mcp::bridge::REQUEST_TIMEOUT.as_secs(),
        },
        routing: Routing::TakeOver,
        at_bound: "the bridge fails over: it holds the election again and serves, or attaches to the server that won it",
    },
    PeerWait {
        id: "server.ensure",
        waits_for: "a starting shared server to become ready",
        site: "src/commands/serve.rs",
        anchor: "pub fn converge_with(",
        bound: Bound::Declared {
            key: "serve ensure --wait",
            default_seconds: 20,
            zero_turns_off: false,
        },
        routing: Routing::Report,
        at_bound: "the command reports the standing it found: the lease, its pid and its address",
    },
    PeerWait {
        id: "server.stop",
        waits_for: "the signalled shared server to release its lease and exit",
        site: "src/commands/serve.rs",
        anchor: "fn exited_by(pid: u32, deadline: Instant) -> bool {",
        bound: Bound::Declared {
            key: "serve stop --wait",
            default_seconds: 10,
            zero_turns_off: false,
        },
        routing: Routing::Report,
        at_bound: "the command names the pid and the address that would not stop",
    },
    PeerWait {
        id: "mesh.link.request",
        waits_for: "another runtime's answer to a hello or a sync",
        site: "src/mesh/link.rs",
        anchor: "pub const REQUEST_TIMEOUT: Duration",
        bound: Bound::Constant {
            name: "mesh::link::REQUEST_TIMEOUT",
            seconds: crate::mesh::link::REQUEST_TIMEOUT.as_secs(),
        },
        routing: Routing::AnotherHost,
        at_bound: "the round with that runtime fails and is counted; the other links go on, and a runtime silent past cooperation.expiry_seconds is expired",
    },
    PeerWait {
        id: "mesh.rendezvous.request",
        waits_for: "a rendezvous hub's answer to a registration",
        site: "src/mesh/rendezvous.rs",
        anchor: "const REQUEST_TIMEOUT: Duration",
        bound: Bound::Constant {
            name: "mesh::rendezvous::REQUEST_TIMEOUT",
            seconds: crate::mesh::rendezvous::REQUEST_TIMEOUT.as_secs(),
        },
        routing: Routing::AnotherHost,
        at_bound: "the hub is backed off and the other endpoints, seeds and discovery providers go on",
    },
    PeerWait {
        id: "mesh.runtime.expiry",
        waits_for: "a linked runtime that stopped beating",
        site: "src/mesh/cooperation.rs",
        anchor: "Duration::from_secs(self.config.expiry_seconds)",
        bound: Bound::Declared {
            key: "cooperation.expiry_seconds",
            default_seconds: super::config::DEFAULT_EXPIRY,
            zero_turns_off: false,
        },
        routing: Routing::AnotherSeat,
        at_bound: "the runtime is expired: its sessions end, its claims stop excluding, and its unanswered review requests read expired",
    },
    PeerWait {
        id: "mesh.session.progress",
        waits_for: "a session that holds an exclusive claim and makes no progress",
        site: "src/mesh/cooperation.rs",
        anchor: "Duration::from_secs(self.config.session_ttl_seconds)",
        bound: Bound::Declared {
            key: "cooperation.session_ttl_seconds",
            default_seconds: super::config::DEFAULT_SESSION_TTL,
            zero_turns_off: true,
        },
        routing: Routing::AnotherSeat,
        at_bound: "its exclusive claims read suspended and refuse nobody, so the next claimant is admitted (I3990)",
    },
    PeerWait {
        id: "mesh.claim.admission",
        waits_for: "a scope another session holds exclusively",
        site: "src/mesh/cooperation.rs",
        anchor: "return Err(CooperationError::Conflict(conflicts));",
        bound: Bound::NoWait,
        routing: Routing::Report,
        at_bound: "the claim is refused at once with the key, session and intent of every claim it met",
    },
    PeerWait {
        id: "mesh.review.answer",
        waits_for: "a reviewer's answer to a review request",
        site: "src/mesh/cooperation.rs",
        anchor: "pub fn request_review(",
        bound: Bound::NoWait,
        routing: Routing::Report,
        at_bound: "the request returns at once; an open review reads open, never agreed, and expired once its asker's runtime stops beating",
    },
    PeerWait {
        id: "mesh.handover.consume",
        waits_for: "another session to consume a published handover",
        site: "src/mesh/cooperation.rs",
        anchor: "pub fn publish_handover(",
        bound: Bound::NoWait,
        routing: Routing::Report,
        at_bound: "publishing returns at once; an unconsumed handover reads published, and consuming one already taken is refused",
    },
    PeerWait {
        id: "mesh.cli.request",
        waits_for: "the shared server's answer to a mesh command",
        site: "src/commands/mesh.rs",
        anchor: "const REQUEST_TIMEOUT: Duration",
        bound: Bound::Constant {
            name: "commands::mesh::REQUEST_TIMEOUT",
            seconds: crate::commands::mesh::REQUEST_TIMEOUT.as_secs(),
        },
        routing: Routing::Report,
        at_bound: "the command fails naming the server's address and the route it asked",
    },
    PeerWait {
        id: "mesh.verify.round",
        waits_for: "a live round with every linked runtime, through the shared server",
        site: "src/commands/mesh.rs",
        anchor: "const VERIFY_TIMEOUT: Duration",
        bound: Bound::Constant {
            name: "commands::mesh::VERIFY_TIMEOUT",
            seconds: crate::commands::mesh::VERIFY_TIMEOUT.as_secs(),
        },
        routing: Routing::Report,
        at_bound: "the command fails naming the server's address; each peer within the round is bounded by mesh.link.request",
    },
    PeerWait {
        id: "integration.lease",
        waits_for: "the integration lease of one base branch, held by another executor",
        site: "src/integration/drain.rs",
        anchor: "libc::LOCK_EX | libc::LOCK_NB",
        bound: Bound::NoWait,
        routing: Routing::Report,
        at_bound: "refused at once naming the executor that holds it; its mesh claim lapses by mesh.session.progress (I3991)",
    },
    PeerWait {
        id: "integration.claim.request",
        waits_for: "the shared server's answer to the integration claim",
        site: "src/integration/exclusive.rs",
        anchor: "const TIMEOUT: Duration",
        bound: Bound::Constant {
            name: "integration::exclusive::TIMEOUT",
            seconds: crate::integration::exclusive::TIMEOUT.as_secs(),
        },
        routing: Routing::Report,
        at_bound: "the claim is not taken and the executor stops, naming the server's address",
    },
    PeerWait {
        id: "offload.queue",
        waits_for: "the derive lock of an offload host, held by an earlier derive",
        site: "src/offload/queue.rs",
        anchor: "if started.elapsed() >= self.wait_max {",
        bound: Bound::Declared {
            key: "derive-offload queue.wait_max_seconds",
            default_seconds: crate::offload::declaration::DEFAULT_WAIT_MAX_SECONDS,
            zero_turns_off: false,
        },
        routing: Routing::AnotherHost,
        at_bound: "the ticket is withdrawn and the offload exits 75; the caller derives here or on another host",
    },
    PeerWait {
        id: "offload.host.connect",
        waits_for: "an offload host's ssh connection",
        site: "src/offload/remote.rs",
        anchor: "\"ConnectTimeout=8\".into(),",
        bound: Bound::Constant {
            name: "ssh ConnectTimeout",
            seconds: 8,
        },
        routing: Routing::AnotherHost,
        at_bound: "the host is unavailable and the offload exits 12 naming it; the caller derives here or on another host",
    },
    PeerWait {
        id: "worktree.lock",
        waits_for: "the worktree lock another session holds",
        site: "src/worktree/lock.rs",
        anchor: "pub const WAIT: Duration",
        bound: Bound::Constant {
            name: "worktree::lock::WAIT",
            seconds: crate::worktree::lock::WAIT.as_secs(),
        },
        routing: Routing::Report,
        at_bound: "refused naming the lock file and the pid that holds it",
    },
    PeerWait {
        id: "monitor.until",
        waits_for: "a signal from the stream: a merge, a review, a run another session produces",
        site: "src/commands/monitor.rs",
        anchor: "MONITOR_UNTIL_BOUND",
        bound: Bound::Declared {
            key: "monitor --timeout",
            default_seconds: MONITOR_UNTIL_BOUND.as_secs(),
            zero_turns_off: false,
        },
        routing: Routing::Report,
        at_bound: "the follow exits 10 naming the signal it waited for and the stream it followed",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn crate_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn every_wait_names_a_line_its_source_still_carries() {
        for wait in PEER_WAITS {
            let path = crate_root().join(wait.site);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {} cannot be read: {e}", wait.id, wait.site));
            assert!(
                text.contains(wait.anchor),
                "{}: {} no longer carries {:?}; move the entry with its wait",
                wait.id,
                wait.site,
                wait.anchor
            );
        }
    }

    #[test]
    fn the_ids_are_unique_and_each_is_documented_in_the_mesh_manual() {
        let manual =
            std::fs::read_to_string(crate_root().join("../../docs/MESH.md")).expect("docs/MESH.md");
        let mut seen = std::collections::BTreeSet::new();
        for wait in PEER_WAITS {
            assert!(seen.insert(wait.id), "{} is listed twice", wait.id);
            assert!(
                manual.contains(&format!("`{}`", wait.id)),
                "{} is not in docs/MESH.md's list of peer waits",
                wait.id
            );
            assert!(!wait.waits_for.is_empty() && !wait.at_bound.is_empty());
        }
    }

    #[test]
    fn a_declared_value_is_read_and_its_absence_falls_back_on_the_default() {
        let join = find("server.election.join").unwrap();
        assert_eq!(
            join.bound_with(&|k| (k == "policy server.join_timeout_seconds").then_some(7)),
            Some(Duration::from_secs(7))
        );
        assert_eq!(join.bound_with(&|_| None), Some(crate::lease::JOIN_TIMEOUT));
        // a declared 0 where 0 is not "off" is a wait that ends at once, still bounded
        assert_eq!(join.bound_with(&|_| Some(0)), Some(Duration::ZERO));
    }

    #[test]
    fn an_unbounded_wait_has_no_bound_and_says_so() {
        let wait = PeerWait {
            bound: Bound::Unbounded,
            ..*find("worktree.lock").unwrap()
        };
        assert_eq!(wait.bound_with(&|_| None), None);
        assert_eq!(wait.bound_text(&|_| None), "unbounded");
        assert_eq!(unbounded(&[wait], &|_| None).len(), 1);
        let off = find("mesh.session.progress").unwrap();
        assert_eq!(
            off.bound_text(&|_| Some(0)),
            "unbounded: cooperation.session_ttl_seconds is 0, which turns it off"
        );
        assert_eq!(
            find("worktree.lock").unwrap().bound_text(&|_| None),
            "20 s (worktree::lock::WAIT)"
        );
    }

    #[test]
    fn a_report_at_the_bound_names_the_wait_the_holder_and_the_route() {
        let lock = find("worktree.lock").unwrap();
        let said = lock.reached(Duration::from_millis(20_400), "pid 77");
        assert_eq!(
            said,
            "peer wait worktree.lock reached its bound after 20 s waiting for the worktree lock another session holds, held by pid 77; report: refused naming the lock file and the pid that holds it"
        );
        assert!(lock
            .reached(Duration::ZERO, "  ")
            .contains("held by nobody it could name"));
    }

    #[test]
    fn every_routing_has_its_word() {
        let words: Vec<&str> = [
            Routing::TakeOver,
            Routing::ServeAlone,
            Routing::AnotherHost,
            Routing::AnotherSeat,
            Routing::Report,
        ]
        .iter()
        .map(|r| r.as_str())
        .collect();
        assert_eq!(
            words,
            [
                "take over",
                "serve alone",
                "another host",
                "another seat",
                "report"
            ]
        );
    }
}
