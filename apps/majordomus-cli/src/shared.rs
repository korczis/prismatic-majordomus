//! The shared server: one per repository, holding the lease, serving every web surface the
//! process resolved — the home page, the Cockpit, Swagger UI, OpenAPI, the capability
//! routes, the documentation and every generated report — and MCP over HTTP for every
//! peer that attaches. It is started by the first `majordomus mcp` or `serve` in a repository and
//! ends when its owner's session is over and the last peer has left.

use std::sync::Arc;
use std::time::Duration;

use crate::capability::Context;
use crate::error::Result;
use crate::http::mcp::McpEndpoint;
use crate::http::server::{self, Running};
use crate::http::Router;
use crate::lease::Lease;

/// How often the server looks for expired sessions while it waits for peers to leave.
pub const REAP_INTERVAL: Duration = Duration::from_millis(500);

/// A running shared server.
pub struct SharedServer {
    running: Running,
    endpoint: Arc<McpEndpoint>,
    lease: Lease,
}

impl SharedServer {
    /// Bind, publish the URL into the lease, and start serving. With `fallback`, a taken
    /// port is replaced by a free one and said so; without it, a taken port is an error.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        ctx: Arc<Context>,
        version: &'static str,
        host: &str,
        port: u16,
        fallback: bool,
        lease: Lease,
        share_dir: Option<&std::path::Path>,
    ) -> Result<Self> {
        let bound = if fallback {
            server::bind_or_fallback(host, port)?
        } else {
            server::bind(host, port)?
        };
        let url = bound.url();
        let endpoint = Arc::new(McpEndpoint::new(Arc::clone(&ctx), version, url.clone()));
        let router = Router::new(ctx, version)
            .with_mcp(Arc::clone(&endpoint))
            .with_cockpit(share_dir);
        lease.publish(&url)?;
        // what it serves is read off the resolution, so this line cannot name a route the
        // process does not have or miss one it does
        let surfaces = match router.served() {
            Ok(served) => served.summary(&url),
            Err(e) => return Err(e),
        };
        let running = bound.start(router);
        tracing::info!(
            url = %url,
            lease = %lease.path().display(),
            surfaces = %surfaces,
            "shared server listening on {url} — {surfaces}; the one server for this repository: every later `majordomus mcp` here attaches to it, and it ends when the last peer leaves"
        );
        Ok(SharedServer {
            running,
            endpoint,
            lease,
        })
    }

    /// `http://host:port`.
    pub fn url(&self) -> String {
        self.running.url()
    }

    /// The MCP-over-HTTP endpoint, for whoever needs to count or reap its sessions.
    pub fn endpoint(&self) -> &Arc<McpEndpoint> {
        &self.endpoint
    }

    /// How many HTTP sessions are open right now, expired ones already forgotten.
    pub fn peers_attached(&self) -> usize {
        self.endpoint.reap();
        self.endpoint.active()
    }

    /// Block until no HTTP session remains.
    pub fn wait_until_peers_leave(&self) {
        let mut announced = false;
        loop {
            let n = self.peers_attached();
            if n == 0 {
                break;
            }
            if !announced {
                tracing::info!(
                    peers = n,
                    "the owner's session ended; serving until the last peer leaves"
                );
                announced = true;
            }
            std::thread::sleep(REAP_INTERVAL);
        }
    }

    /// Stop serving and release the lease.
    pub fn stop(self) {
        self.endpoint.close_all();
        self.running.stop();
        self.lease.release();
        tracing::info!("shared server stopped");
    }
}
