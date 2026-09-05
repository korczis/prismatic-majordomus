//! `majordomus mcp`: serve one MCP client over stdio. By default the process joins the
//! repository's one shared server: it starts it when nobody has (loopback HTTP with
//! Swagger UI, OpenAPI and MCP over HTTP beside its own stdio session) and bridges its
//! stdio to it when someone has. `--standalone` serves the client alone, as the first
//! version of this executable did; `--inspect` prints what would be served and stops.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Value};

use crate::app::App;
use crate::cli::{McpArgs, OutputFormat, RepoArgs, Transport};
use crate::error::{Error, Result};
use crate::lease::{self, Lease, Role};
use crate::mcp::bridge::{Bridge, BridgeError, HEARTBEAT};
use crate::mcp::protocol::Reply;
use crate::mcp::{stdio, Server, Surface};
use crate::model::Severity;
use crate::peers::{ClientInfo, PeerId, Transport as PeerTransport};
use crate::repository::Repository;
use crate::shared::SharedServer;

/// Run `majordomus mcp`.
pub fn run(args: McpArgs) -> Result<u8> {
    if args.inspect {
        let app = App::load(&args.repo)?;
        return inspect(&Surface::new(app.context.clone()), args.format);
    }
    match args.transport {
        Transport::Stdio => {}
    }
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    if args.standalone {
        let mut server = standalone(&args.repo)?;
        tracing::info!("standalone: no shared server, no HTTP, no peers");
        stdio::serve(stdin.lock(), stdout.lock(), |m| server.handle(m))?;
        return Ok(0);
    }
    let repo = Repository::discover(&start_dir(&args.repo)?)?;
    let mut session = Session::open(args, repo)?;
    stdio::serve(stdin.lock(), stdout.lock(), |m| session.handle(m))?;
    session.finish();
    Ok(0)
}

/// A server answering one stdio session alone: no shared server, no HTTP, no peers.
fn standalone(repo: &RepoArgs) -> Result<Server> {
    let app = App::load(repo)?;
    let ctx = app.context.clone();
    let peer = ctx.peers.attach(PeerTransport::Stdio);
    Ok(Server::new(
        Surface::new(ctx).for_peer(peer),
        crate::VERSION,
    ))
}

fn start_dir(args: &RepoArgs) -> Result<std::path::PathBuf> {
    match &args.repo {
        Some(p) => Ok(p.clone()),
        None => std::env::current_dir().map_err(|e| Error::io(".", e)),
    }
}

/// The stdio session of this process, answered locally by the shared server this
/// process runs, or forwarded to the one another process runs.
struct Session {
    args: McpArgs,
    repo: Repository,
    backend: Backend,
}

enum Backend {
    Local(Box<Local>),
    Remote {
        bridge: Arc<Mutex<Bridge>>,
        heartbeat: Heartbeat,
    },
    /// The shared server cannot be used (the lease cannot be written or replaced, the
    /// server cannot start): this client is served alone, as `--standalone` would, and
    /// the log said why.
    Alone(Box<Server>),
}

/// This process is the shared server; its own stdio is answered here.
struct Local {
    server: Server,
    peer: PeerId,
    shared: SharedServer,
}

struct Heartbeat {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Heartbeat {
    fn start(bridge: Arc<Mutex<Bridge>>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("mcp-heartbeat".into())
            .spawn(move || {
                let tick = Duration::from_millis(500);
                let mut waited = Duration::ZERO;
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(tick);
                    waited += tick;
                    if waited >= HEARTBEAT {
                        waited = Duration::ZERO;
                        if let Err(e) = lock(&bridge).heartbeat() {
                            tracing::debug!("heartbeat: {e}");
                        }
                    }
                }
            })
            .ok();
        Heartbeat { stop, thread }
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Session {
    fn open(args: McpArgs, repo: Repository) -> Result<Self> {
        let backend = match lease::elect(&repo) {
            Ok(Role::Server(lease)) => match Self::serve(&args, lease, None) {
                Ok(backend) => backend,
                Err(e) => Self::alone(&args, None, &format!("starting it failed: {e}"))?,
            },
            Ok(Role::Peer { url }) => Self::attach(url),
            Err(e) => Self::alone(&args, None, &e.to_string())?,
        };
        Ok(Session {
            args,
            repo,
            backend,
        })
    }

    /// The shared server cannot be used: serve this client alone and say why. The layer
    /// itself still has to load; when it does not, that error is the answer, not this.
    fn alone(args: &McpArgs, resume: Option<ClientInfo>, why: &str) -> Result<Backend> {
        let mut server = standalone(&args.repo)?;
        tracing::warn!(
            "cannot use the shared server: {why}; serving this client alone (no HTTP, no Swagger UI, no peers), as --standalone would"
        );
        if let Some(client) = resume {
            server.resume(client);
        }
        Ok(Backend::Alone(Box::new(server)))
    }

    /// Become the shared server: load the layer, bind, and answer this stdio locally.
    fn serve(
        args: &McpArgs,
        lease: Lease,
        resume: Option<crate::peers::ClientInfo>,
    ) -> Result<Backend> {
        let app = App::load(&args.repo)?;
        let ctx = app.context.clone();
        let shared = SharedServer::start(
            Arc::clone(&ctx),
            crate::VERSION,
            &args.http_host,
            args.http_port,
            true,
            lease,
        )?;
        let peer = ctx.peers.attach(PeerTransport::Stdio);
        let mut server = Server::new(Surface::new(ctx).for_peer(peer.clone()), crate::VERSION)
            .with_endpoint(Some(shared.url()));
        if let Some(client) = resume {
            server.resume(client);
        }
        Ok(Backend::Local(Box::new(Local {
            server,
            peer,
            shared,
        })))
    }

    /// Attach to the shared server another process runs.
    fn attach(url: String) -> Backend {
        tracing::info!(
            url = %url,
            "a shared server for this repository is already running at {url} (swagger ui {url}/docs); bridging this stdio session to it"
        );
        let bridge = Arc::new(Mutex::new(Bridge::new(url)));
        let heartbeat = Heartbeat::start(Arc::clone(&bridge));
        Backend::Remote { bridge, heartbeat }
    }

    fn handle(&mut self, message: Value) -> Option<Reply> {
        match &mut self.backend {
            Backend::Local(local) => local.server.handle(message),
            Backend::Alone(server) => server.handle(message),
            Backend::Remote { bridge, .. } => {
                let answer = lock(bridge).handle(&message);
                match answer {
                    Ok(v) => v.map(Reply::Value),
                    Err(e) => self.failover(message, e),
                }
            }
        }
    }

    /// The server this session was bridged to is gone: elect again, and either become
    /// the server (carrying the client's session over) or attach to whoever did.
    fn failover(&mut self, message: Value, cause: BridgeError) -> Option<Reply> {
        tracing::warn!("{cause}; electing again");
        let client = match &self.backend {
            Backend::Remote { bridge, .. } => lock(bridge).client().cloned(),
            Backend::Local(_) | Backend::Alone(_) => None,
        };
        match lease::elect(&self.repo) {
            Ok(Role::Server(lease)) => match Self::serve(&self.args, lease, client.clone()) {
                Ok(backend) => {
                    self.replace(backend);
                    tracing::info!(
                        "took over as the shared server; this session continues locally"
                    );
                    self.handle(message)
                }
                Err(e) => {
                    tracing::error!("cannot take over as the shared server: {e}");
                    self.settle_alone(
                        message,
                        client,
                        &format!("{cause}; and taking over failed: {e}"),
                    )
                }
            },
            Ok(Role::Peer { url }) => {
                let Backend::Remote { bridge, .. } = &self.backend else {
                    return unavailable(&message, &cause.to_string());
                };
                let mut b = lock(bridge);
                b.move_to(url.clone());
                let retry = b.reinitialize().and_then(|()| b.handle(&message));
                match retry {
                    Ok(v) => {
                        tracing::info!(url = %url, "re-attached to the shared server");
                        v.map(Reply::Value)
                    }
                    Err(e) => unavailable(&message, &e.to_string()),
                }
            }
            Err(e) => self.settle_alone(
                message,
                client,
                &format!("{cause}; electing again failed: {e}"),
            ),
        }
    }

    /// Replace the backend, stopping the heartbeat of a bridge that is left behind.
    fn replace(&mut self, backend: Backend) {
        let old = std::mem::replace(&mut self.backend, backend);
        if let Backend::Remote { heartbeat, .. } = old {
            heartbeat.stop();
        }
    }

    /// Neither serving nor attaching worked: serve this client alone when the layer
    /// loads, and answer the request with an error naming every failure when it does not.
    fn settle_alone(
        &mut self,
        message: Value,
        client: Option<ClientInfo>,
        reason: &str,
    ) -> Option<Reply> {
        match Self::alone(&self.args, client, reason) {
            Ok(backend) => {
                self.replace(backend);
                self.handle(message)
            }
            Err(e) => {
                tracing::error!("cannot serve this client alone either: {e}");
                unavailable(&message, reason)
            }
        }
    }

    /// The client has gone. A server lingers while other peers are attached; a bridge
    /// tells the server its session is over.
    fn finish(self) {
        match self.backend {
            Backend::Local(local) => {
                let Local {
                    server,
                    peer,
                    shared,
                } = *local;
                server.surface().context().peers.detach(&peer);
                drop(server);
                shared.wait_until_peers_leave();
                shared.stop();
            }
            Backend::Remote { bridge, heartbeat } => {
                heartbeat.stop();
                lock(&bridge).close();
                tracing::info!("bridge closed");
            }
            Backend::Alone(_) => {}
        }
    }
}

/// The JSON-RPC answer for a request that no server could take; nothing for a notification.
fn unavailable(message: &Value, reason: &str) -> Option<Reply> {
    let id = message.get("id").cloned().filter(|i| !i.is_null())?;
    Some(Reply::Value(json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32603, "message": format!("shared server unavailable: {reason}") }
    })))
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn inspect(surface: &Surface, format: OutputFormat) -> Result<u8> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let index = surface.index();
    match format {
        OutputFormat::Json => {
            let v = json!({
                "repository": surface.repository_info().map_err(|e| Error::Protocol { reason: e.to_string() })?,
                "resources": &*surface.resources(),
                "tools": &*surface.tools(),
            });
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&v).map_err(|e| Error::Protocol {
                    reason: e.to_string()
                })?
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            let w = |out: &mut std::io::StdoutLock<'_>, line: String| {
                writeln!(out, "{line}").map_err(Error::Transport)
            };
            w(&mut out, format!("repository  {}", index.repository.root))?;
            w(
                &mut out,
                format!("discovery   {}", index.repository.discovery),
            )?;
            w(
                &mut out,
                format!(
                    "state       {}",
                    match index.state {
                        crate::index::State::Ok => "ok",
                        crate::index::State::Degraded => "degraded",
                    }
                ),
            )?;
            for (kind, n) in index.kinds() {
                w(&mut out, format!("kind        {kind:<12} {n}"))?;
            }
            let summary = surface.registry().summary();
            w(
                &mut out,
                format!(
                    "capabilities {} ({} builtin, {} declarative)",
                    summary.total, summary.builtin, summary.declarative
                ),
            )?;
            for r in surface.resources().iter() {
                w(&mut out, format!("resource    {}", r.uri))?;
            }
            for t in surface.tools().iter() {
                w(&mut out, format!("tool        {}", t.name))?;
            }
            for d in &index.diagnostics {
                let level = match d.severity {
                    Severity::Error => "FAIL",
                    Severity::Warning => "WARN",
                    Severity::Info => "INFO",
                };
                w(
                    &mut out,
                    format!(
                        "{level:<4} {:<22} {} — {}",
                        d.code,
                        d.path.as_deref().unwrap_or("-"),
                        d.message
                    ),
                )?;
            }
        }
    }
    Ok(if index.errors() > 0 { 10 } else { 0 })
}
