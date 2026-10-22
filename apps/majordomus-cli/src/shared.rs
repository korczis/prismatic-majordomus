//! The shared server: one per checkout (ADR 0035, ADR 0044), holding the lease, serving every web surface the
//! process resolved — the home page, the Cockpit, Swagger UI, OpenAPI, the capability
//! routes, the documentation and every generated report — and MCP over HTTP for every
//! peer that attaches. It is started by the first `majordomus mcp` or `serve` in a repository and
//! ends when its owner's session is over and the last peer has left.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::capability::Context;
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
    /// The mesh runtime this server activated (or left inactive, with the reason).
    mesh: Arc<crate::mesh::MeshRuntime>,
}

/// Activate the mesh from the repository's declaration, when there is one and it is
/// enabled. Every failure is a reason on `mesh.status`, never a failed server.
fn activate_mesh(ctx: &Arc<Context>, version: &str, url: &str) {
    let Some(parsed) = crate::capability::builtin::mesh::declaration(ctx) else {
        return;
    };
    let config = match parsed {
        Ok(config) => config,
        Err(e) => {
            tracing::warn!(error = %e, "the mesh declaration does not parse; the mesh stays off");
            ctx.mesh
                .decline(&format!("the declaration does not parse: {e}"));
            return;
        }
    };
    if !config.enabled {
        ctx.mesh.decline("the mesh declaration is disabled");
        return;
    }
    let Some(identity_path) = crate::mesh::default_identity_path() else {
        ctx.mesh
            .decline("no HOME and no XDG_STATE_HOME: nowhere to keep a node identity");
        return;
    };
    let identity = match crate::mesh::NodeIdentity::load_or_create(&identity_path) {
        Ok(identity) => identity,
        Err(e) => {
            tracing::warn!(error = %e, "the node identity did not load; the mesh stays off");
            ctx.mesh.decline(&e.to_string());
            return;
        }
    };
    let endpoints = vec![url
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()];
    let root = std::path::Path::new(&ctx.index.repository.root);
    let repos = crate::repository::git_identity(root)
        .map(|g| vec![g.id])
        .unwrap_or_default();
    let node = identity.public.node_id.clone();
    match ctx
        .mesh
        .activate(&config, identity, endpoints, repos, version)
    {
        Ok(()) => {
            tracing::info!(node = %node, "mesh active: this node announces and listens per the declaration")
        }
        Err(e) => {
            tracing::warn!(error = %e, "the mesh did not activate");
            ctx.mesh.decline(&e.to_string());
        }
    }
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
        let ctx_for_mesh = live.current();
        let endpoint = Arc::new(McpEndpoint::new(Arc::clone(&live), version, url.clone()));
        let router = Router::new(live, version)
            .with_mcp(Arc::clone(&endpoint))
            .with_cockpit(share_dir);
        lease.publish(&url)?;
        // The mesh, when the repository declares one — before the workers pick up their
        // first request, so a client that connects on the "listening" line already sees
        // the activated runtime. Never blocking: providers open sockets on their own
        // threads, and a declaration that is absent, disabled or malformed leaves the
        // runtime inactive with the reason `mesh.status` reports.
        activate_mesh(&ctx_for_mesh, version, &url);
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
            mesh: ctx_for_mesh.mesh.clone(),
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
        self.mesh.stop();
        self.stopping.store(true, Ordering::SeqCst);
        self.endpoint.close_all();
        self.running.stop();
        self.lease.release();
        tracing::info!("shared server stopped");
    }
}
