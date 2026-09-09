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
use crate::lease::{ExecutableIdentity, Lease};

/// How often the server looks for expired sessions while it waits for peers to leave.
pub const REAP_INTERVAL: Duration = Duration::from_millis(500);

/// A running shared server.
pub struct SharedServer {
    running: Running,
    endpoint: Arc<McpEndpoint>,
    lease: Lease,
    started_as: ExecutableIdentity,
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
        let endpoint = Arc::new(McpEndpoint::new(Arc::clone(&ctx), version, url.clone()));
        let router = Router::new(ctx, version)
            .with_mcp(Arc::clone(&endpoint))
            .with_cockpit(share_dir);
        lease.publish(&url)?;
        // what it serves is read off the resolution, so this line cannot name a route the
        // process does not have or miss one it does
        let surfaces = router.served()?.summary(&url);
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
            started_as: ExecutableIdentity::now(),
        })
    }

    /// Is this process still the executable on disk?
    ///
    /// A server outlives its own binary: `cargo build` replaces the file, the process keeps
    /// the code it loaded, and every client that attaches from then on is answered by a
    /// version nobody can find in the tree. On 2026-09-09 one such process served eight
    /// sessions for four and a half hours, 58 commits behind the checkout it sat in, and
    /// the only visible symptom was that things "had worked before". Nothing outside the
    /// process can end it — a client that killed servers would kill other people's
    /// sessions — so the process ends itself, releasing the lease, and every bridged client
    /// elects again on its next message, which is the failover they already have.
    pub fn is_current(&self) -> bool {
        !self.started_as.replaced_on_disk()
    }

    /// Sleep one reap interval, reaping expired sessions; `false` when this process's
    /// executable has been replaced meanwhile and the caller should stop serving.
    pub fn tick(&self) -> bool {
        std::thread::sleep(REAP_INTERVAL);
        self.endpoint.reap();
        if self.is_current() {
            return true;
        }
        tracing::warn!(
            "the executable this server was started from has been replaced on disk; stopping, so that the next client elects a server built from the current code"
        );
        false
    }

    /// `http://host:port`.
    pub fn url(&self) -> String {
        self.running.url()
    }

    /// How many HTTP sessions are open right now, expired ones already forgotten.
    pub fn peers_attached(&self) -> usize {
        self.endpoint.reap();
        self.endpoint.active()
    }

    /// Block until no HTTP session remains — or until this executable is replaced on disk,
    /// which ends the wait the same way: the peers are better served by whoever they elect
    /// next than by a process running code the tree no longer has.
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
            if !self.tick() {
                break;
            }
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
