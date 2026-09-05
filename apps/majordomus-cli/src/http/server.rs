//! The socket: bind, read requests, hand them to the router, write responses. Loopback by
//! default; the bound address is logged so that a caller who asked for port 0 learns it.
//!
//! Lifecycle: the server lives as long as its stdin. When stdin reaches end of file (the
//! parent closed the pipe, or a person pressed Ctrl-D) the accept loop is unblocked and
//! the process ends with 0. There is no daemon mode: whoever started the process owns it,
//! as with `majordomus mcp`.

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
    let stop = Arc::clone(&server);
    std::thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = std::io::stdin().lock().read_to_end(&mut sink);
        tracing::info!("stdin closed; stopping");
        stop.unblock();
    });
    for mut request in server.incoming_requests() {
        let method = request.method().to_string();
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
        // always Content-Length, never chunked: one less thing a small client must decode
        let out = HttpResponse::from_string(response.body)
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
