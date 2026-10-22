//! The socket: bind, read requests, hand them to the router, write responses. Loopback by
//! default; the bound address is reported so that a caller who asked for port 0, or whose
//! port was taken, learns the one in use. A bound socket is accepted by a few worker
//! threads, and each request is answered on a thread of its own up to a bound, so a slow
//! request queues nothing behind it — not even the probe a client sends before it would
//! take the lease over — and the owner's stdio session never waits on HTTP. Stopping is cooperative: every
//! worker is unblocked and joined, and an in-flight response is finished first. A stopped
//! server refuses the next connection: `stop` returns after the listening socket is closed. A connection
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

/// How many requests are answered at once. A bound, so that a flood costs a fixed number of
/// threads; far above what clients of one checkout send, so that a few slow tool calls never
/// hold the probe up. Past it a request is answered 503, except the probe (`GET /`), which a
/// worker answers itself: it is cheap, and it is what decides whether this server keeps its
/// lease (I2156).
pub const MAX_HANDLERS: usize = 64;

/// Requests being answered right now, on threads of their own.
static HANDLERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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

/// Bind `ip:port` for the mesh: a private address of this machine a trusted peer dials
/// (I2411). Not the accidental bind [`bind`] warns about: the repository's mesh declaration
/// asked for it, and a caller there is held to the remote-caller rule like any other
/// non-loopback caller (I2123).
pub fn bind_private(ip: std::net::Ipv4Addr, port: u16) -> Result<Bound> {
    let server = Arc::new(Server::http((ip, port)).map_err(|e| Error::Http {
        reason: format!("cannot bind {ip}:{port}: {e}"),
    })?);
    let address = format!("{ip}:{port}");
    tracing::info!(
        address = %address,
        "bound {address} for the mesh: the declaration is enabled and lists this machine, so trusted peers on this network can dial it; a caller here reads, and changes nothing it cannot authenticate"
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
            .unwrap_or(std::net::SocketAddr::from(([0, 0, 0, 0], 0)));
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

    /// Unblock every worker and wait for it; an in-flight response is finished first. When
    /// this returns the address refuses a connection: the next caller learns that nothing was
    /// sent, not that an answer was lost.
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
        drop(self.server);
        closed(&self.address);
        tracing::info!(address = %self.address, "http stopped");
    }
}

/// How long [`Running::stop`] waits for the listening socket to close.
const CLOSE_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// Wait until `address` refuses a connection, within [`CLOSE_GRACE`].
///
/// Dropping the server only asks its accept thread to end: the listening socket is closed by
/// that thread, some time after the drop returns. Until then the kernel still completes a
/// connection nobody will read, and the caller is reset after it has sent its request, so it
/// cannot tell whether the request ran. A stopped server is one that refuses, and `stop`
/// returns only when this one does. Another process bound to the same port keeps answering;
/// the bound is what ends the wait then.
fn closed(address: &str) {
    let Ok(address) = address.parse::<std::net::SocketAddr>() else {
        return;
    };
    let deadline = std::time::Instant::now() + CLOSE_GRACE;
    let attempt = std::time::Duration::from_millis(100);
    while std::time::Instant::now() < deadline {
        if std::net::TcpStream::connect_timeout(&address, attempt).is_err() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    tracing::warn!(%address, "the listening socket still accepts after the server stopped");
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
    if HANDLERS.fetch_add(1, Ordering::SeqCst) >= MAX_HANDLERS {
        HANDLERS.fetch_sub(1, Ordering::SeqCst);
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
    }
    let router = router.clone();
    let watch = Arc::clone(watch);
    let spawned = std::thread::Builder::new()
        .name("http-request".into())
        .spawn(move || {
            answer(&router, &watch, request);
            HANDLERS.fetch_sub(1, Ordering::SeqCst);
        });
    released_unless_spawned(spawned, &HANDLERS);
}

/// A request whose thread could not be spawned went with the closure, and the client sees its
/// connection close: give back the slot it took in `handlers`, and say so.
fn released_unless_spawned(spawned: std::io::Result<JoinHandle<()>>, handlers: &AtomicUsize) {
    if let Err(e) = spawned {
        handlers.fetch_sub(1, Ordering::SeqCst);
        tracing::warn!("a request could not be given a thread: {e}");
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
    // and it ends when its client hangs up, not at its next write: an open channel keeps the
    // server in use, so a closed one must stop counting within the idle timer's tick (I2131)
    let hung_up = watch.hung_up(request.remote_addr());
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
            accepted.serve_until(Box::new(socket), hung_up);
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
    let (body, length) = wire_body(&response, head);
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
        // the length is the server's to write, and `wire_body` has already read it
        if name.eq_ignore_ascii_case("content-length") {
            continue;
        }
        if let Ok(h) = Header::from_bytes(name.as_bytes(), value.as_bytes()) {
            out.add_header(h);
        }
    }
    if let Err(e) = request.respond(out) {
        tracing::debug!("client went away before the response was written: {e}");
    }
}

/// The bytes to write and the `Content-Length` to announce. A HEAD writes nothing. A
/// `304 Not Modified` writes nothing either, and announces the length its producer names in
/// a `Content-Length` header — the 200's, the only value RFC 9110 section 8.6 allows a 304
/// to carry — or none of its own, an empty body's 0.
fn wire_body(response: &super::router::Response, head: bool) -> (Vec<u8>, usize) {
    if response.status == 304 {
        let named = response
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse().ok());
        return (Vec::new(), named.unwrap_or(0));
    }
    let body: Vec<u8> = if head {
        Vec::new()
    } else {
        response.body.as_bytes().to_vec()
    };
    let length = body.len();
    (body, length)
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

    #[test]
    fn a_304_writes_no_body_and_announces_the_length_it_names() {
        use super::super::router::Response;
        let not_modified = Response::new(304, "text/css", "").with_header("Content-Length", "1234");
        assert_eq!(wire_body(&not_modified, false), (Vec::new(), 1234));
        let unnamed = Response::new(304, "text/css", "ignored");
        assert_eq!(wire_body(&unnamed, false), (Vec::new(), 0));
        let ok = Response::new(200, "text/css", "body{}");
        assert_eq!(wire_body(&ok, false), (b"body{}".to_vec(), 6));
        assert_eq!(
            wire_body(&ok, true),
            (Vec::new(), 0),
            "a HEAD writes nothing"
        );
    }

    #[test]
    fn a_request_that_could_not_be_given_a_thread_gives_back_its_slot() {
        let handlers = AtomicUsize::new(3);
        released_unless_spawned(Err(std::io::Error::other("no threads left")), &handlers);
        assert_eq!(handlers.load(Ordering::SeqCst), 2);
        let spawned = std::thread::Builder::new().spawn(|| {});
        released_unless_spawned(spawned, &handlers);
        assert_eq!(
            handlers.load(Ordering::SeqCst),
            2,
            "a request on its thread keeps its slot until it is answered"
        );
    }
}
