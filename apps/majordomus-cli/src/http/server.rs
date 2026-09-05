//! The socket: bind, read requests, hand them to the router, write responses. Loopback by
//! default; the bound address is reported so that a caller who asked for port 0, or whose
//! port was taken, learns the one in use. A bound socket is served by a few worker
//! threads that share the immutable router, so a slow request does not queue the rest
//! and the owner's stdio session never waits on HTTP. Stopping is cooperative: every
//! worker is unblocked and joined, and an in-flight response is finished first.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use tiny_http::{Header, Response as HttpResponse, Server};

use crate::error::{Error, Result};

use super::Router;

/// The largest request body accepted.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// How many threads answer requests on one socket.
pub const WORKERS: usize = 4;

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
            "bound to {address}, which is not a loopback address: every host that can reach this interface can read this repository's AI layer, its diagnostics and its peers; bind 127.0.0.1 unless that is intended"
        );
    }
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

    /// Start [`WORKERS`] threads answering requests through `router`.
    pub fn start(self, router: Router) -> Running {
        let stopping = Arc::new(AtomicBool::new(false));
        let threads = (0..WORKERS)
            .map(|n| {
                let server = Arc::clone(&self.server);
                let router = router.clone();
                let stopping = Arc::clone(&stopping);
                std::thread::Builder::new()
                    .name(format!("http-{n}"))
                    .spawn(move || worker(&server, &router, &stopping))
                    .expect("spawn an http worker")
            })
            .collect();
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
        for _ in &self.threads {
            self.server.unblock();
        }
        for t in self.threads {
            let _ = t.join();
        }
        tracing::info!(address = %self.address, "http stopped");
    }
}

fn worker(server: &Server, router: &Router, stopping: &AtomicBool) {
    loop {
        match server.recv() {
            Ok(request) => answer(router, request),
            Err(_) if stopping.load(Ordering::SeqCst) => break,
            Err(e) => tracing::warn!("accepting a connection failed: {e}"),
        }
    }
}

fn answer(router: &Router, mut request: tiny_http::Request) {
    let method = request.method().to_string();
    let head = method == "HEAD";
    let method = if head { "GET".to_string() } else { method };
    let target = request.url().to_string();
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
    let response = if body.len() > MAX_BODY_BYTES {
        super::router::Response::error(
            413,
            "too_large",
            &format!("the body is over {MAX_BODY_BYTES} bytes"),
        )
    } else {
        router.handle(&super::Request::parse_target(&method, &target, body).with_headers(headers))
    };
    tracing::debug!(method = %method, target = %target, status = response.status, "response");
    // always Content-Length, never chunked: one less thing a small client must decode;
    // a HEAD gets the GET's headers and no body
    let body = if head { String::new() } else { response.body };
    let mut out = HttpResponse::from_string(body)
        .with_chunked_threshold(usize::MAX)
        .with_status_code(response.status)
        .with_header(
            Header::from_bytes("Content-Type", response.content_type).expect("static header"),
        )
        .with_header(Header::from_bytes("Cache-Control", "no-store").expect("static header"));
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
