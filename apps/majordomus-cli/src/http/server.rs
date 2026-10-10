//! The socket: bind, read requests, hand them to the router, write responses. Loopback by
//! default; the bound address is reported so that a caller who asked for port 0, or whose
//! port was taken, learns the one in use. A bound socket is accepted by a few worker
//! threads, and each request is answered on a thread of its own up to a bound, so a slow
//! request queues nothing behind it — not even the probe a client sends before it would
//! take the lease over — and the owner's stdio session never waits on HTTP. Stopping is cooperative: every
//! worker is unblocked and joined, and an in-flight response is finished first. A connection
//! that reads no request within the declared deadline is closed ([`super::deadline`]), and a
//! request that finds every handler busy is refused rather than answered on a worker.

use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use tiny_http::{Header, Response as HttpResponse, Server};

use crate::error::{Error, Result};

use super::deadline::Watch;
use super::Router;

/// The largest request body accepted.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// How many threads accept requests on one socket. They only accept: each request is answered
/// on a thread of its own (up to [`MAX_HANDLERS`]), so a worker is free again at once and a
/// lease probe is answered while tool calls run (I2126).
pub const WORKERS: usize = 4;

/// When this process last answered a request, in milliseconds since this process started;
/// zero before the first. Read by the idle timer, so that a person reading the Cockpit or a
/// client calling the REST API keeps the server they are using (I2131).
static LAST_REQUEST_MS: AtomicU64 = AtomicU64::new(0);

/// Live `/events` channels open right now: a person watching an execution is using the server
/// for as long as the socket stays open, whether or not they send anything.
static OPEN_CHANNELS: AtomicUsize = AtomicUsize::new(0);

fn epoch() -> std::time::Instant {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}

fn note_request() {
    let ms = epoch().elapsed().as_millis() as u64;
    LAST_REQUEST_MS.store(ms.max(1), Ordering::Relaxed);
}

/// Whether this process was used over HTTP within the last `window`: a request answered in
/// it, or a live channel open now. The idle timer asks this beside the MCP sessions.
///
/// ```
/// use std::time::Duration;
/// use majordomus_cli::http::server::used_within;
/// // a process that has answered nothing over HTTP and holds no channel has not been used
/// assert!(!used_within(Duration::from_secs(60)));
/// ```
pub fn used_within(window: std::time::Duration) -> bool {
    if OPEN_CHANNELS.load(Ordering::Relaxed) > 0 {
        return true;
    }
    let last = LAST_REQUEST_MS.load(Ordering::Relaxed);
    last != 0 && epoch().elapsed().as_millis() as u64 <= last + window.as_millis() as u64
}

/// How many requests are answered at once. A bound, so that a flood costs a fixed number of
/// threads; far above what clients of one checkout send, so that a few slow tool calls never
/// hold the probe up. Past it a request is answered 503, except the probe (`GET /`), which a
/// worker answers itself: it is cheap, and it is what decides whether this server keeps its
/// lease (I2156).
pub const MAX_HANDLERS: usize = 64;

/// Requests being answered right now, on threads of their own.
static HANDLERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A socket that is bound and not yet served.
pub struct Bound {
    server: Arc<Server>,
    address: String,
}

/// Bind `host:port`; `0` picks a free port. The error names the address when it fails.
pub fn bind(host: &str, port: u16) -> Result<Bound> {
    let server = Arc::new(Server::http((host, port)).map_err(|e| Error::Http {
        reason: format!("cannot bind {host}:{port}: {e}"),
    })?);
    let address = server
        .server_addr()
        .to_ip()
        .map(|a| a.to_string())
        .unwrap_or_else(|| format!("{host}:{port}"));
    if address
        .parse::<std::net::SocketAddr>()
        .is_ok_and(|a| !a.ip().is_loopback())
    {
        tracing::warn!(
            address = %address,
            "bound to {address}, which is not a loopback address: every host that can reach this interface can read this repository's AI layer, its diagnostics and its peers, unencrypted; it can change nothing but through a signed mesh message (ADR 0126); bind 127.0.0.1 unless that is intended"
        );
    }
    Ok(Bound { server, address })
}

/// Bind the address a deployment object declared. The accidental-bind warning does not
/// fire here and is not suppressed either: it asks "did you mean this", and `source` is the
/// answer — the canonical object that said so, named in the log so the operator can read
/// which file decided the address. A hosted process that bound loopback would be
/// unreachable inside its own machine, which is why this path exists at all.
pub fn bind_declared(host: &str, port: u16, source: &str) -> Result<Bound> {
    let server = Arc::new(Server::http((host, port)).map_err(|e| Error::Http {
        reason: format!("cannot bind {host}:{port}: {e}"),
    })?);
    let address = server
        .server_addr()
        .to_ip()
        .map(|a| a.to_string())
        .unwrap_or_else(|| format!("{host}:{port}"));
    tracing::info!(
        address = %address,
        source = %source,
        "bound {address}, the address {source} declares; this repository's AI layer is readable by every host that can reach this interface, which is what a deployment means"
    );
    Ok(Bound { server, address })
}

/// Bind `host:port`, and when that port is taken bind a free one instead, saying so on
/// stderr. For a server whose port is a convenience rather than a contract.
pub fn bind_or_fallback(host: &str, port: u16) -> Result<Bound> {
    match bind(host, port) {
        Ok(b) => Ok(b),
        Err(e) if port != 0 => {
            tracing::warn!("{e}; taking a free port instead");
            bind(host, 0)
        }
        Err(e) => Err(e),
    }
}

impl Bound {
    /// `host:port` as bound.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// `http://host:port`.
    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    /// Start [`WORKERS`] threads answering requests through `router`, and the sweeper that
    /// holds their connections to the declared read deadline.
    pub fn start(self, router: Router) -> Running {
        let stopping = Arc::new(AtomicBool::new(false));
        let local = self
            .address
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([0, 0, 0, 0], 0)));
        let watch = Arc::new(Watch::new(local, crate::lease::timings().read_deadline));
        let mut threads: Vec<JoinHandle<()>> = (0..WORKERS)
            .map(|n| {
                let server = Arc::clone(&self.server);
                let router = router.clone();
                let stopping = Arc::clone(&stopping);
                let watch = Arc::clone(&watch);
                std::thread::Builder::new()
                    .name(format!("http-{n}"))
                    .spawn(move || worker(&server, &router, &watch, &stopping))
                    .expect("spawn an http worker")
            })
            .collect();
        let sweeper = {
            let stopping = Arc::clone(&stopping);
            std::thread::Builder::new()
                .name("http-deadline".into())
                .spawn(move || watch.run(&stopping))
                .expect("spawn the http deadline sweeper")
        };
        threads.push(sweeper);
        Running {
            server: self.server,
            address: self.address,
            stopping,
            threads,
        }
    }
}

/// A served socket.
pub struct Running {
    server: Arc<Server>,
    address: String,
    stopping: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Running {
    /// `host:port` as bound.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// `http://host:port`.
    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    /// Unblock every worker and wait for it; an in-flight response is finished first.
    pub fn stop(self) {
        self.stopping.store(true, Ordering::SeqCst);
        // one more than there are workers is harmless; the sweeper reads `stopping` itself
        for _ in &self.threads {
            self.server.unblock();
        }
        for t in self.threads {
            let _ = t.join();
        }
        // the requests already handed off are answered before the process goes on to stop,
        // within a bound: a handler stuck forever does not hold the stop forever
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while HANDLERS.load(Ordering::SeqCst) > 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        tracing::info!(address = %self.address, "http stopped");
    }
}

fn worker(server: &Server, router: &Router, watch: &Arc<Watch>, stopping: &AtomicBool) {
    loop {
        match server.recv() {
            Ok(request) => hand_off(router, watch, request),
            Err(_) if stopping.load(Ordering::SeqCst) => break,
            Err(e) => tracing::warn!("accepting a connection failed: {e}"),
        }
    }
}

/// Answer `request` on a thread of its own while fewer than [`MAX_HANDLERS`] are, so that the
/// worker accepts the next request at once. Past the bound the probe is answered here and
/// anything else is refused 503: a worker that read a slow body itself would be held by it.
fn hand_off(router: &Router, watch: &Arc<Watch>, request: tiny_http::Request) {
    let Some(slot) = Slot::take(&HANDLERS, MAX_HANDLERS) else {
        let probe = matches!(
            request.method(),
            tiny_http::Method::Get | tiny_http::Method::Head
        ) && request.url() == "/";
        if probe {
            answer(router, watch, request);
        } else {
            super::deadline::refused();
            watch.stop_reading(request.remote_addr());
            let busy = super::router::Response::error(
                503,
                "busy",
                &format!("{MAX_HANDLERS} requests are being answered; ask again"),
            )
            .with_header("Retry-After", "1");
            respond(request, busy, false);
        }
        return;
    };
    let router = router.clone();
    let watch = Arc::clone(watch);
    let spawned = std::thread::Builder::new()
        .name("http-request".into())
        .spawn(move || {
            let _slot = slot;
            answer(&router, &watch, request);
        });
    if let Err(e) = spawned {
        // the request and the slot went with the closure: the client sees its connection
        // close, and the slot is given back as the closure is dropped
        tracing::warn!("a request could not be given a thread: {e}");
    }
}

/// One request being answered on a thread of its own: counted while it lives and given back
/// when it ends, however it ends. The count was decremented after the handler returned, so a
/// handler that panicked kept its slot forever; enough of them sent every request back onto
/// the workers and made [`Running::stop`] wait its whole bound (review of #884, I2126).
struct Slot(&'static std::sync::atomic::AtomicUsize);

impl Slot {
    /// A slot of `counter`, or none when `bound` are taken.
    fn take(counter: &'static std::sync::atomic::AtomicUsize, bound: usize) -> Option<Slot> {
        if counter.fetch_add(1, Ordering::SeqCst) >= bound {
            counter.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Slot(counter))
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn answer(router: &Router, watch: &Watch, request: tiny_http::Request) {
    note_request();
    let peer = request.remote_addr().copied();
    watch.reading(peer.as_ref());
    if let Some(request) = upgrade(router, watch, request) {
        answer_http(router, watch, request);
    }
    watch.answered(peer.as_ref());
}

/// Hand the socket to the live channel when the request asks for it, on a thread of its
/// own so the worker goes back to answering requests.
///
/// Returns the request when it was not an upgrade, and nothing when the socket has been
/// given away. This is the one thing the router cannot do for itself: `tiny_http` yields
/// the stream only by consuming the request.
fn upgrade(
    router: &Router,
    watch: &Watch,
    request: tiny_http::Request,
) -> Option<tiny_http::Request> {
    let headers: Vec<(String, String)> = request
        .headers()
        .iter()
        .map(|h| (h.field.as_str().to_string(), h.value.as_str().to_string()))
        .collect();
    let probe =
        super::Request::parse_target(&request.method().to_string(), request.url(), Vec::new())
            .with_headers(headers)
            .with_remote(request.remote_addr().map(|a| a.ip()));
    let accepted = match router.websocket(&probe) {
        None => return Some(request),
        Some(Ok(accepted)) => accepted,
        Some(Err(response)) => {
            watch.busy(request.remote_addr());
            respond(request, response, false);
            return None;
        }
    };
    // a live channel is never closed by the read deadline: it waits by design
    watch.live(request.remote_addr());
    let key = accepted.accept.clone();
    let response = HttpResponse::empty(101).with_header(
        Header::from_bytes("Sec-WebSocket-Accept", key.as_bytes())
            .expect("the accept value is header-safe"),
    );
    let socket = request.upgrade("websocket", response);
    let spawned = std::thread::Builder::new()
        .name("events".into())
        .spawn(move || {
            OPEN_CHANNELS.fetch_add(1, Ordering::Relaxed);
            accepted.serve(Box::new(socket));
            OPEN_CHANNELS.fetch_sub(1, Ordering::Relaxed);
            note_request();
        });
    if let Err(e) = spawned {
        tracing::warn!("a live channel could not be started: {e}");
    }
    None
}

fn answer_http(router: &Router, watch: &Watch, mut request: tiny_http::Request) {
    let method = request.method().to_string();
    let head = method == "HEAD";
    let method = if head { "GET".to_string() } else { method };
    let target = request.url().to_string();
    let remote = request.remote_addr().map(|a| a.ip());
    let headers: Vec<(String, String)> = request
        .headers()
        .iter()
        .map(|h| (h.field.as_str().to_string(), h.value.as_str().to_string()))
        .collect();
    let mut body = Vec::new();
    if let Err(e) = request
        .as_reader()
        .take(MAX_BODY_BYTES as u64 + 1)
        .read_to_end(&mut body)
    {
        tracing::warn!("cannot read a request body: {e}");
        return;
    }
    if !watch.busy(request.remote_addr()) {
        // the deadline closed the connection under the body: what was read is short
        return;
    }
    let response = if body.len() > MAX_BODY_BYTES {
        // the rest of the body is never read, and dropping the request would read it
        watch.stop_reading(request.remote_addr());
        super::router::Response::error(
            413,
            "too_large",
            &format!("the body is over {MAX_BODY_BYTES} bytes"),
        )
    } else {
        router.handle(
            &super::Request::parse_target(&method, &target, body)
                .with_headers(headers)
                .with_remote(remote),
        )
    };
    // the target carries the query string, where a careless client puts a token
    tracing::debug!(
        method = %method,
        target = %crate::redaction::redact_secrets(&target).text,
        status = response.status,
        "response"
    );
    respond(request, response, head);
}

/// Write one response and let the client go.
fn respond(request: tiny_http::Request, response: super::router::Response, head: bool) {
    // always Content-Length, never chunked: one less thing a small client must decode;
    // a HEAD gets the GET's headers and no body
    let body: Vec<u8> = if head {
        Vec::new()
    } else {
        response.body.as_bytes().to_vec()
    };
    let length = body.len();
    let mut out = HttpResponse::new(
        response.status.into(),
        Vec::new(),
        std::io::Cursor::new(body),
        Some(length),
        None,
    )
    .with_chunked_threshold(usize::MAX)
    .with_status_code(response.status)
    .with_header(Header::from_bytes("Content-Type", response.content_type).expect("static header"));
    // `no-store` is right for an answer derived from a repository that a person is editing;
    // a response that named its own caching (a Cockpit asset whose URL carries its digest)
    // keeps what it said
    if !response
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("cache-control"))
    {
        out.add_header(Header::from_bytes("Cache-Control", "no-store").expect("static header"));
    }
    for (name, value) in &response.headers {
        if let Ok(h) = Header::from_bytes(name.as_bytes(), value.as_bytes()) {
            out.add_header(h);
        }
    }
    if let Err(e) = request.respond(out) {
        tracing::debug!("client went away before the response was written: {e}");
    }
}

/// Is stdin a pipe or a socket, as when a parent process spawned us and holds the other end?
/// Asked of descriptor 0 itself (`fstat`), never of the path `/dev/stdin`: the path
/// resolves through `/dev/fd`, which some harnesses (a coverage runner, a sandbox) do not
/// expose, and a wrong "no" here sends `serve` into the run-forever branch while its
/// parent waits for it to end.
#[cfg(unix)]
pub fn stdin_is_a_pipe() -> bool {
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::FileTypeExt;
    // SAFETY: descriptor 0 is the process's stdin for the process's lifetime; ManuallyDrop
    // keeps this `File` from closing it.
    let stdin = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(0) });
    stdin
        .metadata()
        .map(|m| m.file_type().is_fifo() || m.file_type().is_socket())
        .unwrap_or(false)
}

/// Is stdin a pipe or a socket? Not known on this platform.
#[cfg(not(unix))]
pub fn stdin_is_a_pipe() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// A handler that panics gives its slot back, and a full bound refuses the next one.
    #[test]
    fn a_slot_is_given_back_whatever_happens_to_its_handler() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let panicked = std::thread::spawn(|| {
            let _slot = Slot::take(&COUNT, 2).expect("a free slot");
            panic!("the handler fails");
        })
        .join();
        assert!(panicked.is_err());
        assert_eq!(
            COUNT.load(Ordering::SeqCst),
            0,
            "the panic gave the slot back"
        );
        let a = Slot::take(&COUNT, 2).expect("first");
        let b = Slot::take(&COUNT, 2).expect("second");
        assert!(Slot::take(&COUNT, 2).is_none(), "the bound holds");
        assert_eq!(COUNT.load(Ordering::SeqCst), 2, "a refusal takes nothing");
        drop((a, b));
        assert_eq!(COUNT.load(Ordering::SeqCst), 0);
    }
}
