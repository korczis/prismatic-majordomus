//! The shared server: one per repository, holding the lease, serving the HTTP projection
//! (the Cockpit, Swagger UI, OpenAPI, the capability routes) and MCP over HTTP for every
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
        // The static surfaces of the repository, resolved once at startup: whatever the
        // producers have generated is served from its own directory, and nothing here names
        // one of them (ADR 0013). A repository with none mounts none.
        let root = ctx.index.repository.root.clone();
        let surfaces = crate::web::discover::discover(
            std::path::Path::new(&root),
            crate::web::discover::Runtime::full(),
        )
        .map(|topology| {
            Arc::new(crate::web::serve::StaticSurfaces::new(
                &topology,
                std::path::Path::new(&root),
            ))
        })
        .ok();
        let mounted: Vec<String> = surfaces
            .as_ref()
            .map(|s| s.ids().into_iter().map(str::to_string).collect())
            .unwrap_or_default();
        let mut router = Router::new(ctx, version)
            .with_mcp(Arc::clone(&endpoint))
            .with_cockpit(share_dir);
        if let Some(surfaces) = surfaces {
            if !surfaces.is_empty() {
                router = router.with_surfaces(surfaces);
            }
        }
        lease.publish(&url)?;
        let running = bound.start(router);
        tracing::info!(
            url = %url,
            lease = %lease.path().display(),
            surfaces = %if mounted.is_empty() { "none".to_string() } else { mounted.join(", ") },
            // Each path comes from the constant the router binds it at, never from a
            // second spelling here: the line said `{url}/docs` for the Swagger UI, and
            // would have gone on saying it after that mount moved.
            "shared server listening on {url} (cockpit {url}{cockpit}, swagger ui {url}{swagger}, openapi {url}{spec}, mcp over http {url}{mcp}); the one server for this repository: every later `majordomus mcp` here attaches to it, and it ends when the last peer leaves",
            cockpit = crate::cockpit::PREFIX,
            swagger = crate::http::swagger::DOCS_PATH,
            spec = crate::http::swagger::SPEC_PATH,
            mcp = crate::http::mcp::PATH,
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
