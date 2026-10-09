//! The shared server: one per checkout (ADR 0035, ADR 0044), holding the lease, serving every web surface the
//! process resolved — the home page, the Cockpit, Swagger UI, OpenAPI, the capability
//! routes, the documentation and every generated report — and MCP over HTTP for every
//! peer that attaches. It is started by the first `majordomus mcp` or `serve` in a repository and
//! ends when its owner's session is over and the last peer has left.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::capability::Context;
use crate::error::Result;
use crate::http::mcp::McpEndpoint;
use crate::http::server::{self, Running};
use crate::http::Router;
use crate::lease::{Lease, LeaseFile};

/// How often the server looks for expired sessions while it waits for peers to leave.
pub const REAP_INTERVAL: Duration = Duration::from_millis(500);

/// How long a server asked to stop by a signal waits for its peers to leave before it closes
/// their sessions itself: long enough for a request in flight to be answered, short enough
/// that the whole ordered stop sits well inside `serve stop`'s default wait and the
/// [`crate::lease::STOP_BOUND`] after which the process is ended anyway.
pub(crate) const STOP_PEER_GRACE: Duration = Duration::from_secs(2);

/// A running shared server.
pub struct SharedServer {
    running: Running,
    endpoint: Arc<McpEndpoint>,
    lease: Lease,
    /// Set by [`SharedServer::stop`]; the reader thread ends at its next tick.
    stopping: Arc<AtomicBool>,
    /// The mesh runtime this server activated (or left inactive, with the reason).
    mesh: Arc<crate::mesh::MeshRuntime>,
    /// Set by whichever of the two stoppers runs the ordered stop first — [`SharedServer::stop`]
    /// on the owner's thread, or the reader thread answering a signal — so that it runs once.
    claimed: Arc<AtomicBool>,
    /// Set by [`SharedServer::take_stop_requests`]: the owner's loop answers a stop signal
    /// itself, and the reader thread leaves it to it.
    owner_answers: Arc<AtomicBool>,
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
    // Where peers can reach this server: the bound address, or every interface's address
    // when it is bound to all of them. A loopback bind is reachable from this machine
    // only, and `mesh doctor` says so.
    let bound = url.trim_start_matches("http://").trim_end_matches('/');
    let endpoints = crate::mesh::address::advertised_endpoints(bound);
    let root = std::path::Path::new(&ctx.index.repository.root);
    // One runtime per checkout, and one repository identity per repository on every
    // machine — derived from content or declared, never from a path.
    let runtime = crate::mesh::repository::runtime_id(&crate::repository::identity(root));
    let repository =
        crate::mesh::repository::resolve(root, config.cooperation.repository.as_deref());
    let repos = repository
        .as_ref()
        .map(|r| vec![r.id.clone()])
        .unwrap_or_default();
    let node = identity.public.node_id.clone();
    let identity = Arc::new(identity);
    match ctx.mesh.activate_as(
        &config,
        Arc::clone(&identity),
        &runtime,
        endpoints.clone(),
        repos,
        version,
    ) {
        Ok(()) => {
            tracing::info!(node = %node, runtime_id = %runtime, "mesh active: this node announces and listens per the declaration")
        }
        Err(e) => {
            tracing::warn!(error = %e, "the mesh did not activate");
            ctx.mesh.decline(&e.to_string());
            return;
        }
    }
    if !config.cooperation.enabled {
        ctx.mesh
            .decline_cooperation("cooperation is disabled in the mesh declaration");
        return;
    }
    let repository = match repository {
        Ok(repository) => repository,
        Err(e) => {
            tracing::warn!(error = %e, "the repository has no mesh identity; cooperation stays off");
            ctx.mesh.decline_cooperation(&e.to_string());
            return;
        }
    };
    let setup = crate::mesh::cooperation::CooperationSetup {
        identity,
        runtime,
        repository,
        endpoints,
        version: version.into(),
        config: config.cooperation.clone(),
        trust: config.trust.clone(),
        journal_path: Some(root.join(".ai/local/state/mesh/journal.jsonl")),
        registry: Arc::clone(ctx.mesh.registry()),
        transport: Arc::new(crate::mesh::link::HttpTransport),
        board: Some(Arc::clone(&ctx.peers)),
        checkout: crate::mesh::cooperation::CheckoutFacts {
            id: Some(crate::repository::identity(root)),
            root: Some(root.to_path_buf()),
        },
    };
    match crate::mesh::cooperation::Cooperation::new(setup) {
        Ok(cooperation) => {
            cooperation.start();
            ctx.mesh.attach_cooperation(cooperation);
        }
        Err(e) => {
            tracing::warn!(error = %e, "cooperation did not start");
            ctx.mesh.decline_cooperation(&e.to_string());
        }
    }
}

impl SharedServer {
    /// Bind, start serving, then publish the URL into the lease. With `fallback`, a taken
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
        // The mesh, when the repository declares one — before the workers pick up their
        // first request, so a client that connects on the "listening" line already sees
        // the activated runtime. Never blocking: providers open sockets on their own
        // threads, and a declaration that is absent, disabled or malformed leaves the
        // runtime inactive with the reason `mesh.status` reports.
        activate_mesh(&ctx_for_mesh, version, &url);
        // what it serves is read off the resolution, so this line cannot name a route the
        // process does not have or miss one it does
        let surfaces = router.served()?.summary(&url);
        // A published URL must already answer. Publishing before mesh activation and
        // surface resolution lets direnv probe a bound socket with no request workers,
        // report a failed server, and reload before anything can answer it.
        let running = start_published(bound, router, || lease.publish(&url)).inspect_err(|_| {
            ctx_for_mesh.mesh.stop();
        })?;
        // The server's own reader: every REAP_INTERVAL it forgets the HTTP sessions that
        // stopped pinging — on every path, not only while the owner waits for peers to
        // leave, so that a dead peer never stays `attached` on the board — and it checks
        // that the lease is still its own. A lease another process took over is that
        // process's to remove; this one stops claiming it, serves the peers it has, and
        // ends with them.
        //
        // It is also the stopper of last resort. A signal asking this process to stop is only
        // recorded by its handler (`crate::lease` says why); when the owner's loop does not
        // answer it — `majordomus mcp`, whose main thread is its client's stdio session — this
        // thread runs the ordered stop and then ends the process with that signal.
        let stopping = Arc::new(AtomicBool::new(false));
        let claimed = Arc::new(AtomicBool::new(false));
        let owner_answers = Arc::new(AtomicBool::new(false));
        {
            let endpoint = Arc::clone(&endpoint);
            let stopping = Arc::clone(&stopping);
            let claimed = Arc::clone(&claimed);
            let owner_answers = Arc::clone(&owner_answers);
            let mesh = ctx_for_mesh.mesh.clone();
            let path = lease.path().to_path_buf();
            let token = lease.token().to_string();
            let _ = std::thread::Builder::new()
                .name("majordomus-server-tick".into())
                .spawn(move || {
                    let mut lost = false;
                    while !stopping.load(Ordering::SeqCst) {
                        std::thread::sleep(REAP_INTERVAL);
                        if let Some(signal) = crate::lease::stop_requested() {
                            if !owner_answers.load(Ordering::SeqCst)
                                && !claimed.swap(true, Ordering::SeqCst)
                            {
                                tracing::info!(
                                    signal,
                                    "asked to stop by signal {signal}; stopping in order"
                                );
                                mesh.begin_stop();
                                endpoint.close_all();
                                crate::lease::release_held();
                                mesh.stop();
                                tracing::info!("shared server stopped");
                                crate::lease::end_by_signal(signal);
                            }
                        }
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
        crate::lease::answer_stop_requests();
        Ok(SharedServer {
            running,
            endpoint,
            lease,
            stopping,
            mesh: ctx_for_mesh.mesh.clone(),
            claimed,
            owner_answers,
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

    /// Block until no HTTP session remains, or a signal asks this process to stop: a server
    /// lingering for its peers is still a server that `serve stop` must be able to end in
    /// order.
    pub fn wait_until_peers_leave(&self) {
        self.wait_for_peers(None);
    }

    /// The owner's loop answers a stop signal itself: it polls
    /// [`crate::lease::stop_requested`] and calls [`SharedServer::stop_within`], so that the
    /// process ends by returning from `main`. Without this, the server's reader thread answers
    /// the signal (see [`SharedServer::start`]).
    pub(crate) fn take_stop_requests(&self) {
        self.owner_answers.store(true, Ordering::SeqCst);
    }

    /// The ordered stop a signal asks for: wait up to `grace` for the attached peers to leave,
    /// then [`SharedServer::stop`], which closes whatever sessions and episodes remain.
    pub(crate) fn stop_within(self, grace: Duration) {
        self.wait_for_peers(Some(Instant::now() + grace));
        self.stop();
    }

    /// Wait until no HTTP session remains: until `deadline` when there is one, and otherwise
    /// until a stop signal arrives.
    fn wait_for_peers(&self, deadline: Option<Instant>) {
        let mut announced = false;
        loop {
            let n = self.peers_attached();
            if n == 0 {
                break;
            }
            match deadline {
                Some(deadline) if Instant::now() >= deadline => {
                    tracing::info!(
                        peers = n,
                        "closing the sessions of the peers still attached"
                    );
                    break;
                }
                Some(_) => {}
                None if crate::lease::stop_requested().is_some() => break,
                None => {}
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

    /// Stop serving and release the lease. This is the ordered stop: the owner's session
    /// ending runs it, and so does `serve stop` — its `SIGTERM` is answered by the `serve`
    /// loop through `SharedServer::stop_within`, or, in a process whose loop does not
    /// answer it, by the reader thread [`SharedServer::start`] spawns, which runs the same
    /// steps short of joining the listeners and then ends the process with the signal.
    ///
    /// The order is what matters. Telling the mesh to stop is immediate, but draining its
    /// link table needs a lock a worker can hold across a dial that waits out the link
    /// timeout — several seconds. Everything a waiting `serve stop` measures happens before
    /// that: every session and every episode is closed, the listeners close (an answer in
    /// flight is finished first) and the lease is released — only if the file is still this
    /// server's, checked under the lock a take-over takes — and only then does the mesh
    /// drain. A lease released late is read by the next process as a server still holding
    /// the port, which is what `serve stop` answers 10 for; `serve stop` then waits for the
    /// process to exit, which is after the drain.
    pub fn stop(self) {
        if self.claimed.swap(true, Ordering::SeqCst) {
            // the reader thread is running this stop for a signal and ends the process when
            // it is done; there is nothing left for this thread to do but not interfere
            loop {
                std::thread::sleep(REAP_INTERVAL);
            }
        }
        self.mesh.begin_stop();
        self.stopping.store(true, Ordering::SeqCst);
        self.endpoint.close_all();
        self.running.stop();
        self.lease.release();
        self.mesh.stop();
        tracing::info!("shared server stopped");
    }
}

/// Publish only after requests can be handled, and close the socket if publication fails.
fn start_published(
    bound: server::Bound,
    router: Router,
    publish: impl FnOnce() -> Result<()>,
) -> Result<Running> {
    let running = bound.start(router);
    if let Err(error) = publish() {
        running.stop();
        return Err(error);
    }
    Ok(running)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::cli::{DiscoveryMode, RepoArgs};
    use crate::error::Error;
    use crate::synthetic::SyntheticRepository;

    /// Whether the listening socket at `address` stops accepting within `deadline`. Dropping
    /// a `tiny_http::Server` closes its listener on the library's own accept thread, which
    /// nothing joins, so the socket outlives `stop` by a scheduling delay; under a loaded
    /// test run an immediate connect can still land in the backlog. The wait is bounded and
    /// a listener that never closes still fails.
    fn listener_closes(address: std::net::SocketAddr, deadline: Duration) -> bool {
        let until = std::time::Instant::now() + deadline;
        loop {
            if std::net::TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_err() {
                return true;
            }
            if std::time::Instant::now() >= until {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn publication_observes_a_serving_socket_and_failure_closes_it() {
        let fixture = SyntheticRepository::small().unwrap();
        let app = App::load(&RepoArgs {
            repo: Some(fixture.root().to_path_buf()),
            discovery: DiscoveryMode::Filesystem,
            share: Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
            ..Default::default()
        })
        .unwrap();
        let router = Router::new(Arc::clone(&app.context), crate::VERSION);
        router.served().unwrap();

        // Check at the publication boundary, not after start returns or its log appears.
        // No retry can hide a URL being announced before requests are handled.
        for refused in [false, true] {
            let bound = server::bind("127.0.0.1", 0).unwrap();
            let url = bound.url();
            let address: std::net::SocketAddr = bound.address().parse().unwrap();
            let result = start_published(bound, router.clone(), || {
                let reply =
                    crate::lease::probe_reply(&url, app.repository.root(), Duration::from_secs(2));
                assert!(reply.is_some(), "publication preceded request handling");
                if refused {
                    Err(Error::Lease {
                        reason: "publication refused".into(),
                    })
                } else {
                    Ok(())
                }
            });
            if refused {
                assert!(result.is_err());
            } else {
                result.unwrap().stop();
            }
            assert!(
                listener_closes(address, Duration::from_secs(5)),
                "request workers retained the socket after shutdown"
            );
        }

        // Losing the lease while routes are prepared must also unwind the real startup.
        let crate::lease::Role::Server(lease) = crate::lease::elect(&app.repository).unwrap()
        else {
            panic!("fixture unexpectedly has a server");
        };
        std::fs::remove_file(lease.path()).unwrap();
        assert!(SharedServer::start(
            Arc::new(crate::live::Live::pinned(app.context)),
            crate::VERSION,
            "127.0.0.1",
            0,
            false,
            None,
            lease,
            None,
        )
        .is_err());
    }
}
