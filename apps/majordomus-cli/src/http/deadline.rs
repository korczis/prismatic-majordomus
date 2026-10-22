//! The read deadline: how long a connection may hold this server without a request read in
//! full on it (I2156).
//!
//! `tiny_http` reads every request head on a thread of its own pool and never times a read
//! out, and the body is read by the thread answering the request. A client that opens a
//! connection and sends one byte a second therefore holds a thread forever; enough of them
//! hold every request handler, and past [`super::server::MAX_HANDLERS`] the probe a client
//! sends before taking the lease over waits behind them. `tiny_http` never hands its socket
//! out, so the deadline is kept from beside it: a sweeper lists this process's descriptors,
//! keeps the ones connected to the served address, and shuts down every connection that has
//! been waiting for a request, or reading one, for longer than the deadline. A connection
//! whose request is being answered is never closed: handler time is bounded by moving it off
//! the workers, not by cutting a tool call short.
//!
//! The deadline also bounds a wait it does not cause. `tiny_http` keeps a pool thread on each
//! connection for as long as it lives, and a connection that arrives in a burst can be queued
//! in that pool until one of its threads is free; behind slow clients, that was never. Now it
//! is at most the deadline, when a slow client's thread is released.
//!
//! Unix only: elsewhere the descriptors cannot be listed, nothing is closed, and the
//! counters stay at zero.
//!
//! The lifecycle, from a process's side: nothing is reported until a server starts, and
//! from then on the counters only grow.
//!
//! ```no_run
//! use majordomus_cli::http::deadline;
//! use majordomus_cli::http::{server, Router};
//! # fn router() -> Router { unimplemented!() }
//! assert_eq!(deadline::counts(), None, "no server, nothing to report");
//! let running = server::bind("127.0.0.1", 0).unwrap().start(router());
//! // the deadline the server holds connections to is the declared one
//! let counts = deadline::counts().expect("a serving process reports its counts");
//! assert_eq!(
//!     counts.read_deadline_seconds,
//!     majordomus_cli::lease::timings().read_deadline.as_secs()
//! );
//! running.stop();
//! ```

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Connections this process closed because they missed the read deadline.
static CLOSED: AtomicU64 = AtomicU64::new(0);

/// Requests this process refused because [`super::server::MAX_HANDLERS`] were being answered.
static REFUSED: AtomicU64 = AtomicU64::new(0);

/// The deadline in force, in seconds; zero until a server has started in this process.
static DEADLINE_SECONDS: AtomicU64 = AtomicU64::new(0);

/// What this process's HTTP server has done to clients that held a connection without using
/// it: the deadline it applies and what that deadline cost, since the process started.
/// Absent while this process serves no HTTP.
///
/// ```
/// use majordomus_cli::http::deadline::{counts, ConnectionCounts};
/// // a process that has started no server reports nothing, rather than zeroes that would
/// // read as "no slow client was seen"
/// assert_eq!(counts(), None);
/// let c = ConnectionCounts { read_deadline_seconds: 30, closed_by_deadline: 4, refused_busy: 0 };
/// let text = serde_json::to_string(&c).unwrap();
/// assert_eq!(serde_json::from_str::<ConnectionCounts>(&text).unwrap(), c);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct ConnectionCounts {
    /// `server.read_deadline_seconds:` as this process applies it.
    pub read_deadline_seconds: u64,
    /// Connections closed because no request was read in full on them within the deadline.
    pub closed_by_deadline: u64,
    /// Requests answered 503 because every request handler was busy.
    pub refused_busy: u64,
}

/// The counters of this process's HTTP server, or nothing when it serves none.
///
/// ```
/// use majordomus_cli::http::deadline::counts;
/// // asked twice, the same answer: reading the counters does not move them
/// assert_eq!(counts(), counts());
/// ```
pub fn counts() -> Option<ConnectionCounts> {
    let seconds = DEADLINE_SECONDS.load(Ordering::SeqCst);
    (seconds > 0).then(|| ConnectionCounts {
        read_deadline_seconds: seconds,
        closed_by_deadline: CLOSED.load(Ordering::SeqCst),
        refused_busy: REFUSED.load(Ordering::SeqCst),
    })
}

/// Count one request refused past the handler bound.
pub(crate) fn refused() {
    REFUSED.fetch_add(1, Ordering::SeqCst);
}

/// Where one connection stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Open with no request in progress since this instant: being read by `tiny_http`, or
    /// kept alive between requests.
    Waiting(Instant),
    /// Its request head arrived and its body is being read since this instant.
    Reading(Instant),
    /// Its request is being answered: never closed.
    Busy,
    /// Upgraded to a live channel, which waits by design: never closed.
    Live,
    /// Shut down by the sweeper; the request it was reading is abandoned.
    Closed,
}

/// The connections of one served socket, and the deadline they are held to.
pub(crate) struct Watch {
    local: SocketAddr,
    deadline: Duration,
    peers: Mutex<HashMap<SocketAddr, Phase>>,
}

impl Watch {
    /// Watch the connections accepted on `local`, closing those that miss `deadline`.
    pub(crate) fn new(local: SocketAddr, deadline: Duration) -> Self {
        DEADLINE_SECONDS.store(deadline.as_secs().max(1), Ordering::SeqCst);
        Self {
            local,
            deadline,
            peers: Mutex::new(HashMap::new()),
        }
    }

    fn peers(&self) -> std::sync::MutexGuard<'_, HashMap<SocketAddr, Phase>> {
        self.peers.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A request head arrived from `peer`: its body is being read from now on.
    pub(crate) fn reading(&self, peer: Option<&SocketAddr>) {
        if let Some(peer) = peer {
            // a connection the sweeper closed stays closed: its body can only come short
            let mut peers = self.peers();
            let phase = peers.entry(*peer).or_insert(Phase::Busy);
            if *phase != Phase::Closed {
                *phase = Phase::Reading(Instant::now());
            }
        }
    }

    /// The request of `peer` was read in full: it is answered from now on, and never closed
    /// under its handler. False when the sweeper closed the connection while the body was
    /// being read, so that a truncated body is never answered.
    pub(crate) fn busy(&self, peer: Option<&SocketAddr>) -> bool {
        let Some(peer) = peer else { return true };
        let mut peers = self.peers();
        match peers.get_mut(peer) {
            Some(Phase::Closed) => false,
            Some(phase) => {
                *phase = Phase::Busy;
                true
            }
            None => {
                peers.insert(*peer, Phase::Busy);
                true
            }
        }
    }

    /// `peer` was upgraded to a live channel: it is never held to the deadline again.
    pub(crate) fn live(&self, peer: Option<&SocketAddr>) {
        if let Some(peer) = peer {
            self.peers().insert(*peer, Phase::Live);
        }
    }

    /// Whether the client of `peer`'s live channel has hung up, asked of the connection's
    /// socket rather than learned from a failed write: the channel writes only on an event or
    /// a heartbeat, and until it learned, the idle timer counted a closed channel as open
    /// (I2131). The answer is a closure the channel's own thread asks on each quiet interval.
    ///
    /// Called before the socket is handed to the channel, while its client is certainly
    /// there, because that is when the socket can be found; the channel holds the socket
    /// open for as long as it asks. An unknown peer, or a platform where the socket cannot
    /// be found, never hangs up: the channel then learns at its next write, as before.
    pub(crate) fn hung_up(&self, peer: Option<&SocketAddr>) -> impl Fn() -> bool + Send + 'static {
        let descriptor = peer.and_then(|peer| sockets::descriptor(self.local, *peer));
        move || descriptor.is_some_and(sockets::hung_up)
    }

    /// The request of `peer` was answered: the connection waits for the next from now on.
    pub(crate) fn answered(&self, peer: Option<&SocketAddr>) {
        if let Some(peer) = peer {
            if let Some(phase) = self.peers().get_mut(peer) {
                if *phase == Phase::Busy {
                    *phase = Phase::Waiting(Instant::now());
                }
            }
        }
    }

    /// Stop reading from `peer` before a request of it is answered without its body: a 503
    /// past the handler bound, a 413 over the body bound. `tiny_http` reads whatever body is
    /// left when the request is dropped, so a refusal of a slow body would otherwise hold the
    /// thread that refused it for as long as the client trickles — a worker, for the 503.
    /// The response is still written; the connection ends with it.
    pub(crate) fn stop_reading(&self, peer: Option<&SocketAddr>) {
        if let Some(peer) = peer {
            sockets::shut_reading(self.local, *peer);
        }
    }

    /// How often the sweeper looks: a quarter of the deadline, within 100 ms and 1 s.
    pub(crate) fn interval(&self) -> Duration {
        (self.deadline / 4).clamp(Duration::from_millis(100), Duration::from_secs(1))
    }

    /// Run the sweeper until `stopping` is set.
    pub(crate) fn run(&self, stopping: &AtomicBool) {
        let step = Duration::from_millis(50);
        let mut next = Instant::now() + self.interval();
        while !stopping.load(Ordering::SeqCst) {
            std::thread::sleep(step);
            if Instant::now() >= next {
                self.sweep(Instant::now());
                next = Instant::now() + self.interval();
            }
        }
    }

    /// Close every connection of the served socket that has waited for, or been reading, a
    /// request for longer than the deadline, and forget the connections that are gone.
    fn sweep(&self, now: Instant) {
        self.sweep_with(now, sockets::connected_to(self.local), sockets::shut_down);
    }

    /// [`Watch::sweep`] over the connections `open`, closing with `close`: the system's own in
    /// earnest, a test's in a test.
    fn sweep_with(
        &self,
        now: Instant,
        open: Vec<(i32, SocketAddr)>,
        close: impl Fn(i32, SocketAddr, SocketAddr) -> bool,
    ) {
        let mut peers = self.peers();
        let mut seen = HashSet::new();
        for (descriptor, peer) in open {
            // `tiny_http` holds a reading and a writing descriptor for one connection; one
            // shutdown closes both
            if !seen.insert(peer) {
                continue;
            }
            let phase = peers.entry(peer).or_insert(Phase::Waiting(now));
            let since = match *phase {
                Phase::Waiting(since) | Phase::Reading(since) => since,
                Phase::Busy | Phase::Live | Phase::Closed => continue,
            };
            if now.saturating_duration_since(since) < self.deadline {
                continue;
            }
            if close(descriptor, self.local, peer) {
                *phase = Phase::Closed;
                CLOSED.fetch_add(1, Ordering::SeqCst);
                tracing::info!(
                    peer = %peer,
                    deadline_seconds = self.deadline.as_secs(),
                    "closed a connection that read no request within the deadline"
                );
            }
        }
        peers.retain(|peer, _| seen.contains(peer));
    }
}

#[cfg(unix)]
mod sockets {
    use std::mem::ManuallyDrop;
    use std::net::{Shutdown, SocketAddr, TcpStream};
    use std::os::fd::{FromRawFd, RawFd};

    /// This process's open descriptors, from the directory the system lists them in.
    fn descriptors() -> Vec<RawFd> {
        ["/dev/fd", "/proc/self/fd"]
            .iter()
            .find_map(|dir| std::fs::read_dir(dir).ok())
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|e| descriptor_of(&e.file_name()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The descriptor a directory entry of the system's listing names: only a number does.
    pub(super) fn descriptor_of(name: &std::ffi::OsStr) -> Option<RawFd> {
        name.to_str()?.parse().ok()
    }

    /// The addresses of `descriptor` when it is a connected TCP socket.
    fn addresses(descriptor: RawFd) -> Option<(SocketAddr, SocketAddr)> {
        // SAFETY: the descriptor is only asked for its addresses and never closed:
        // ManuallyDrop keeps this `TcpStream` from owning it. One that is not a socket, or
        // that was closed since it was listed, answers with an error.
        let stream = ManuallyDrop::new(unsafe { TcpStream::from_raw_fd(descriptor) });
        Some((stream.local_addr().ok()?, stream.peer_addr().ok()?))
    }

    fn served(local: SocketAddr, mine: SocketAddr) -> bool {
        mine.port() == local.port() && (local.ip().is_unspecified() || mine.ip() == local.ip())
    }

    /// Every descriptor connected to a peer through the served address `local`.
    pub(super) fn connected_to(local: SocketAddr) -> Vec<(RawFd, SocketAddr)> {
        descriptors()
            .into_iter()
            .filter_map(|d| {
                let (mine, peer) = addresses(d)?;
                served(local, mine).then_some((d, peer))
            })
            .collect()
    }

    /// Stop reading the connection from `peer`, leaving its writing half open.
    pub(super) fn shut_reading(local: SocketAddr, peer: SocketAddr) {
        for (descriptor, theirs) in connected_to(local) {
            if theirs == peer {
                // SAFETY: as in `addresses`
                let stream = ManuallyDrop::new(unsafe { TcpStream::from_raw_fd(descriptor) });
                let _ = stream.shutdown(Shutdown::Read);
                return;
            }
        }
    }

    /// The descriptor of the connection from `peer` to the served address `local`, while it
    /// is connected. Found once, while the client is there: a socket its client has reset
    /// no longer names its peer (macOS answers `getpeername` with an error), so a later
    /// search by address finds nothing exactly when there is something to find.
    pub(super) fn descriptor(local: SocketAddr, peer: SocketAddr) -> Option<RawFd> {
        connected_to(local)
            .into_iter()
            .find_map(|(descriptor, theirs)| (theirs == peer).then_some(descriptor))
    }

    /// Has the client at the other end of `descriptor` hung up? Whatever it sent is read and
    /// dropped — the live channel never reads, and ignores what a client sends
    /// (`super::super::ws`) — until the socket says it has nothing more: an end of stream or
    /// a reset is a client that has gone, an empty socket is one that is still there. Never
    /// blocks: every read is `MSG_DONTWAIT`, on a descriptor whose own flags are left alone,
    /// because the channel's thread writes through the same socket.
    ///
    /// The caller holds the socket `descriptor` belongs to open for as long as it asks, so
    /// the number cannot have been closed and reused in between.
    pub(super) fn hung_up(descriptor: RawFd) -> bool {
        let mut buffer = [0u8; 4096];
        // a bound, so that a client streaming bytes at the server cannot hold this thread
        for _ in 0..64 {
            // SAFETY: a read into a buffer this function owns, of at most its length, on a
            // descriptor that is only read and never closed here
            let read = unsafe {
                libc::recv(
                    descriptor,
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            match read {
                0 => return true,
                n if n > 0 => continue,
                _ => {
                    let error = std::io::Error::last_os_error();
                    return !matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    );
                }
            }
        }
        false
    }

    /// Shut the connection down, after asking again that the descriptor is still the one
    /// it was: a descriptor closed and reused since the listing is left alone.
    pub(super) fn shut_down(descriptor: RawFd, local: SocketAddr, peer: SocketAddr) -> bool {
        match addresses(descriptor) {
            Some((mine, now)) if served(local, mine) && now == peer => {
                // SAFETY: as in `addresses`; shutdown ends the connection without closing
                // the descriptor its owner will close
                let stream = ManuallyDrop::new(unsafe { TcpStream::from_raw_fd(descriptor) });
                stream.shutdown(Shutdown::Both).is_ok()
            }
            _ => false,
        }
    }
}

#[cfg(not(unix))]
mod sockets {
    use std::net::SocketAddr;

    pub(super) fn connected_to(_: SocketAddr) -> Vec<(i32, SocketAddr)> {
        Vec::new()
    }

    pub(super) fn shut_down(_: i32, _: SocketAddr, _: SocketAddr) -> bool {
        false
    }

    pub(super) fn shut_reading(_: SocketAddr, _: SocketAddr) {}

    pub(super) fn descriptor(_: SocketAddr, _: SocketAddr) -> Option<i32> {
        None
    }

    pub(super) fn hung_up(_: i32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watch() -> Watch {
        Watch {
            local: "127.0.0.1:1".parse().unwrap(),
            deadline: Duration::from_secs(2),
            peers: Mutex::new(HashMap::new()),
        }
    }

    #[test]
    fn a_body_cut_short_by_the_sweeper_is_never_answered() {
        let w = watch();
        let peer: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        w.reading(Some(&peer));
        w.peers().insert(peer, Phase::Closed);
        assert!(!w.busy(Some(&peer)));
        // nor does a head that arrived after the close reopen it
        w.reading(Some(&peer));
        assert_eq!(w.peers()[&peer], Phase::Closed);
    }

    #[test]
    fn an_answered_request_starts_the_wait_for_the_next() {
        let w = watch();
        let peer: SocketAddr = "127.0.0.1:5001".parse().unwrap();
        w.reading(Some(&peer));
        assert!(w.busy(Some(&peer)));
        assert_eq!(w.peers()[&peer], Phase::Busy);
        w.answered(Some(&peer));
        assert!(matches!(w.peers()[&peer], Phase::Waiting(_)));
    }

    #[test]
    fn a_live_channel_outlives_the_answer_that_opened_it() {
        let w = watch();
        let peer: SocketAddr = "127.0.0.1:5002".parse().unwrap();
        w.reading(Some(&peer));
        w.live(Some(&peer));
        w.answered(Some(&peer));
        assert_eq!(w.peers()[&peer], Phase::Live);
    }

    #[test]
    fn the_sweeper_looks_often_enough_to_keep_the_deadline() {
        let mut w = watch();
        assert_eq!(w.interval(), Duration::from_millis(500));
        w.deadline = Duration::from_secs(60);
        assert_eq!(w.interval(), Duration::from_secs(1));
    }

    #[test]
    fn a_request_whose_peer_is_unknown_is_answered_and_never_tracked() {
        let w = watch();
        // `tiny_http` names no remote address for some connections: nothing is held to the
        // deadline for them, and their request is never refused as cut short
        w.reading(None);
        assert!(w.busy(None));
        w.live(None);
        w.answered(None);
        w.stop_reading(None);
        assert!(w.peers().is_empty());
    }

    #[test]
    fn a_request_read_before_its_head_was_seen_is_busy_from_then_on() {
        let w = watch();
        let peer: SocketAddr = "127.0.0.1:5003".parse().unwrap();
        // the sweeper never listed it and no head was noted: it is answered all the same
        assert!(w.busy(Some(&peer)));
        assert_eq!(w.peers()[&peer], Phase::Busy);
        // an answer for a connection that is not tracked starts nothing
        let other: SocketAddr = "127.0.0.1:5004".parse().unwrap();
        w.answered(Some(&other));
        assert!(!w.peers().contains_key(&other));
    }

    #[test]
    fn a_watch_poisoned_by_a_panic_still_tracks_its_connections() {
        let w = std::sync::Arc::new(watch());
        let held = std::sync::Arc::clone(&w);
        let _ = std::thread::spawn(move || {
            let _guard = held.peers.lock().unwrap();
            panic!("a handler panicked while it held the watch");
        })
        .join();
        assert!(w.peers.is_poisoned());
        let peer: SocketAddr = "127.0.0.1:5005".parse().unwrap();
        w.reading(Some(&peer));
        assert!(matches!(w.peers()[&peer], Phase::Reading(_)));
        assert!(w.busy(Some(&peer)));
    }

    /// Everything a subscriber writes, for a test to read back.
    #[derive(Clone, Default)]
    struct Captured(std::sync::Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Captured {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap_or_else(|e| e.into_inner())).into_owned()
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_connection_idle_past_the_deadline_is_closed_counted_and_logged() {
        use std::io::Read;
        use std::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let local = listener.local_addr().unwrap();
        let w = Watch::new(local, Duration::from_secs(2));
        let mut client = TcpStream::connect(local).unwrap();
        let (accepted, peer) = listener.accept().unwrap();
        // `tiny_http` holds two descriptors on one connection; one shutdown closes both
        let twin = accepted.try_clone().unwrap();

        let start = Instant::now();
        w.sweep(start);
        assert_eq!(
            w.peers()[&peer],
            Phase::Waiting(start),
            "a connection first seen by the sweeper waits from then on"
        );
        w.sweep(start + Duration::from_secs(1));
        assert_eq!(
            w.peers()[&peer],
            Phase::Waiting(start),
            "within the deadline"
        );

        let before = CLOSED.load(Ordering::SeqCst);
        let captured = Captured::default();
        let writer = captured.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            w.sweep(start + Duration::from_secs(3));
        });
        assert_eq!(w.peers()[&peer], Phase::Closed, "past the deadline");
        assert!(
            CLOSED.load(Ordering::SeqCst) > before,
            "the close is counted"
        );
        let log = captured.text();
        assert!(
            log.contains("closed a connection that read no request within the deadline"),
            "{log}"
        );
        assert!(log.contains("deadline_seconds=2"), "{log}");
        assert!(log.contains(&peer.to_string()), "the peer is named: {log}");

        // the client sees its connection end
        client
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let mut buffer = [0u8; 16];
        assert_eq!(client.read(&mut buffer).unwrap_or(0), 0);

        // a connection that is gone is forgotten
        drop(client);
        drop(accepted);
        drop(twin);
        w.sweep(start + Duration::from_secs(4));
        assert!(!w.peers().contains_key(&peer));
    }

    #[test]
    fn a_connection_the_system_will_not_close_stays_as_it_was_and_a_gone_one_is_forgotten() {
        let w = watch();
        let peer: SocketAddr = "127.0.0.1:40000".parse().unwrap();
        let start = Instant::now();
        let asked = std::cell::Cell::new(0);
        let refuse = |descriptor: i32, _: SocketAddr, at: SocketAddr| {
            assert_eq!((descriptor, at), (7, peer));
            asked.set(asked.get() + 1);
            false
        };
        // two descriptors of one connection are one connection
        w.sweep_with(start, vec![(7, peer), (8, peer)], refuse);
        assert_eq!(asked.get(), 0, "within the deadline nothing is closed");
        w.sweep_with(start + Duration::from_secs(3), vec![(7, peer)], refuse);
        assert_eq!(asked.get(), 1, "past the deadline the system is asked once");
        assert_eq!(
            w.peers()[&peer],
            Phase::Waiting(start),
            "a connection the system did not close is not called closed"
        );
        w.sweep_with(start + Duration::from_secs(4), vec![], |_, _, _| {
            unreachable!("nothing is open")
        });
        assert!(
            w.peers().is_empty(),
            "a connection that is gone is forgotten"
        );
    }

    #[cfg(unix)]
    #[test]
    fn only_a_number_names_a_descriptor() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(sockets::descriptor_of(OsStr::new("12")), Some(12));
        assert_eq!(sockets::descriptor_of(OsStr::new("cwd")), None);
        assert_eq!(sockets::descriptor_of(OsStr::from_bytes(b"\xff")), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_descriptor_that_is_not_the_listed_connection_is_left_alone() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        use std::os::fd::AsRawFd;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let local = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(local).unwrap();
        let (mut accepted, peer) = listener.accept().unwrap();

        // a descriptor reused by a socket that is not connected (here, the listener)
        assert!(!sockets::shut_down(listener.as_raw_fd(), local, peer));
        // the descriptor now connected to another peer than the one that was listed
        let other: SocketAddr = "127.0.0.1:9".parse().unwrap();
        assert!(!sockets::shut_down(accepted.as_raw_fd(), local, other));
        // a descriptor served on another address
        let elsewhere: SocketAddr = "127.0.0.1:1".parse().unwrap();
        assert!(!sockets::shut_down(accepted.as_raw_fd(), elsewhere, peer));

        // the connection left alone still carries data both ways
        accepted.write_all(b"x").unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let mut byte = [0u8; 1];
        client.read_exact(&mut byte).unwrap();
        assert_eq!(&byte, b"x");

        // and the descriptor that is still the listed connection is shut down
        assert!(sockets::shut_down(accepted.as_raw_fd(), local, peer));
        assert_eq!(client.read(&mut byte).unwrap_or(0), 0);
    }
}
