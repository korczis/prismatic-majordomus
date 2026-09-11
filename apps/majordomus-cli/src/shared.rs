//! The shared server: one per checkout (ADR 0035, ADR 0044), holding the lease, serving every web surface the
//! process resolved — the home page, the Cockpit, Swagger UI, OpenAPI, the capability
//! routes, the documentation and every generated report — and MCP over HTTP for every
//! peer that attaches. It is started by the first `majordomus mcp` or `serve` in a repository and
//! ends when its owner's session is over and the last peer has left.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::error::Result;
use crate::http::mcp::McpEndpoint;
use crate::http::server::{self, Running};
use crate::http::Router;
use crate::lease::{Lease, LeaseFile};

/// How often the server looks for expired sessions while it waits for peers to leave.
pub const REAP_INTERVAL: Duration = Duration::from_millis(500);

/// A running shared server.
pub struct SharedServer {
    running: Running,
    endpoint: Arc<McpEndpoint>,
    lease: Lease,
    /// Set by [`SharedServer::stop`]; the reader thread ends at its next tick.
    stopping: Arc<AtomicBool>,
}

impl SharedServer {
    /// Bind, publish the URL into the lease, and start serving. With `fallback`, a taken
    /// port is replaced by a free one and said so; without it, a taken port is an error.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        live: Arc<crate::live::Live>,
        version: &'static str,
        host: &str,
        port: u16,
        fallback: bool,
        declared_by: Option<&str>,
        lease: Lease,
        share_dir: Option<&std::path::Path>,
    ) -> Result<Self> {
        // Three ways to arrive at a socket, and they are not interchangeable: a canonical
        // object declared this address (bind it, say which object said so), the port is a
        // convenience (take a free one when it is taken), or the port is what was asked
        // for (bind it or fail).
        let bound = match declared_by {
            Some(source) => server::bind_declared(host, port, source)?,
            None if fallback => server::bind_or_fallback(host, port)?,
            None => server::bind(host, port)?,
        };
        let url = bound.url();
        let endpoint = Arc::new(McpEndpoint::new(Arc::clone(&live), version, url.clone()));
        let router = Router::new(live, version)
            .with_mcp(Arc::clone(&endpoint))
            .with_cockpit(share_dir);
        lease.publish(&url)?;
        // what it serves is read off the resolution, so this line cannot name a route the
        // process does not have or miss one it does
        let surfaces = router.served()?.summary(&url);
        let running = bound.start(router);
        // The server's own reader: every REAP_INTERVAL it forgets the HTTP sessions that
        // stopped pinging — on every path, not only while the owner waits for peers to
        // leave, so that a dead peer never stays `attached` on the board — and it checks
        // that the lease is still its own. A lease another process took over is that
        // process's to remove; this one stops claiming it, serves the peers it has, and
        // ends with them.
        let stopping = Arc::new(AtomicBool::new(false));
        {
            let endpoint = Arc::clone(&endpoint);
            let stopping = Arc::clone(&stopping);
            let path = lease.path().to_path_buf();
            let token = lease.token().to_string();
            let _ = std::thread::Builder::new()
                .name("majordomus-server-tick".into())
                .spawn(move || {
                    let mut lost = false;
                    while !stopping.load(Ordering::SeqCst) {
                        std::thread::sleep(REAP_INTERVAL);
                        let gone = endpoint.reap();
                        if !gone.is_empty() {
                            tracing::info!(peers = ?gone, "peer(s) expired: no message within the idle timeout");
                        }
                        if !lost
                            && LeaseFile::read(&path)
                                .document()
                                .is_none_or(|d| d.token != token)
                        {
                            lost = true;
                            crate::lease::lost();
                            tracing::warn!(
                                lease = %path.display(),
                                "the lease is no longer this server's: another process took it over; this server serves the peers it has and ends with them"
                            );
                        }
                    }
                });
        }
        tracing::info!(
            url = %url,
            lease = %lease.path().display(),
            surfaces = %surfaces,
            // "of this checkout", not "for this repository": a linked worktree is a
            // checkout with a lease and a server of its own, and this line is the first
            // thing a person reads when a client starts one (ADR 0044).
            "shared server listening on {url} — {surfaces}; the one server of this checkout: every later `majordomus mcp` here attaches to it, and it ends when the last peer leaves"
        );
        Ok(SharedServer {
            running,
            endpoint,
            lease,
            stopping,
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
        self.stopping.store(true, Ordering::SeqCst);
        self.endpoint.close_all();
        self.running.stop();
        self.lease.release();
        tracing::info!("shared server stopped");
    }
}
