//! The socket: bind, read requests, hand them to the router, write responses. Loopback by
//! default; the bound address is logged so that a caller who asked for port 0 learns it.
//!
//! Lifecycle: when stdin is a pipe or a socket, the server lives as long as it: end of file
//! (the parent closed its end) unblocks the accept loop and the process ends with 0, the
//! lifecycle `majordomus mcp` has. When stdin is anything else (a terminal, `/dev/null`
//! under nohup or a service manager, a file) nothing is watched and the process runs until
//! it is stopped. There is no daemon mode either way: whoever started the process owns it.

use std::io::Read;
use std::sync::Arc;

use tiny_http::{Header, Response as HttpResponse, Server};

use crate::error::{Error, Result};

use super::Router;

/// The largest request body accepted.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Serve until stdin reaches end of file. Returns `Err` only on a bind failure.
pub fn serve(router: Router, host: &str, port: u16) -> Result<()> {
    let server = Arc::new(Server::http((host, port)).map_err(|e| Error::Http {
        reason: format!("cannot bind {host}:{port}: {e}"),
    })?);
    let addr = server
        .server_addr()
        .to_ip()
        .map(|a| a.to_string())
        .unwrap_or_else(|| format!("{host}:{port}"));
    tracing::info!(address = %addr, "listening on http://{addr} (openapi at /openapi.json, docs at /docs); ends when stdin closes");
    if stdin_is_a_pipe() {
        tracing::info!("stdin is a pipe; the server stops when it closes");
        let stop = Arc::clone(&server);
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = std::io::stdin().lock().read_to_end(&mut sink);
            tracing::info!("stdin closed; stopping");
            stop.unblock();
        });
    } else {
        tracing::info!("stdin is not a pipe; the server runs until the process is stopped");
    }
    for mut request in server.incoming_requests() {
        let method = request.method().to_string();
        let head = method == "HEAD";
        let method = if head { "GET".to_string() } else { method };
        let target = request.url().to_string();
        let mut body = Vec::new();
        if let Err(e) = request
            .as_reader()
            .take(MAX_BODY_BYTES as u64 + 1)
            .read_to_end(&mut body)
        {
            tracing::warn!("cannot read a request body: {e}");
            continue;
        }
        let response = if body.len() > MAX_BODY_BYTES {
            super::router::Response { status: 413, content_type: "application/json", body: format!("{{\"error\":{{\"code\":\"too_large\",\"message\":\"the body is over {MAX_BODY_BYTES} bytes\"}}}}") }
        } else {
            router.handle(&super::Request::parse_target(&method, &target, body))
        };
        tracing::debug!(method = %method, target = %target, status = response.status, "response");
        // always Content-Length, never chunked: one less thing a small client must decode;
        // a HEAD gets the GET's headers and no body
        let body = if head { String::new() } else { response.body };
        let out = HttpResponse::from_string(body)
            .with_chunked_threshold(usize::MAX)
            .with_status_code(response.status)
            .with_header(
                Header::from_bytes("Content-Type", response.content_type).expect("static header"),
            )
            .with_header(Header::from_bytes("Cache-Control", "no-store").expect("static header"));
        if let Err(e) = request.respond(out) {
            tracing::debug!("client went away before the response was written: {e}");
        }
    }
    tracing::info!("stopped");
    Ok(())
}

/// Is stdin a pipe or a socket, as when a parent process spawned us and holds the other end?
#[cfg(unix)]
fn stdin_is_a_pipe() -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::metadata("/dev/stdin")
        .map(|m| m.file_type().is_fifo() || m.file_type().is_socket())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn stdin_is_a_pipe() -> bool {
    false
}
