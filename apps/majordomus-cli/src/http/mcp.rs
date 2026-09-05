//! MCP over HTTP at `/mcp`: the Streamable HTTP transport's request half. A client
//! `POST`s one JSON-RPC message (or a batch) and gets the response in the body; the
//! server answers `initialize` with an `Mcp-Session-Id` header, requires it on every later
//! request, and forgets the session on `DELETE` or after a silence longer than
//! [`SESSION_IDLE_TIMEOUT`]. There is no server-initiated stream (`GET` is 405): this
//! server sends no notifications, so a stream would carry nothing.
//!
//! Every session is one [`Server`] over the shared surface, seen from its own peer, so the
//! stdio bridges of other `majordomus mcp` processes and clients speaking this transport
//! directly are peers of one board, and `peers.announce` knows who spoke.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::capability::Context;
use crate::mcp::{Server, Surface};
use crate::peers::{PeerId, Transport};

use super::router::{Request, Response};

/// The path of the endpoint.
pub const PATH: &str = "/mcp";

/// The session header, as the transport specifies it (compared case-insensitively).
pub const SESSION_HEADER: &str = "Mcp-Session-Id";

/// A session that has been silent this long is forgotten. A `majordomus mcp` bridge
/// pings every [`crate::mcp::bridge::HEARTBEAT`], well inside it; a client that speaks
/// the transport directly and falls silent longer re-initialises on the 404 it gets, as
/// the transport prescribes.
pub const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

struct Session {
    peer: PeerId,
    server: Mutex<Server>,
    last_seen: Mutex<Instant>,
}

/// The endpoint: the sessions it holds and the surface they share.
pub struct McpEndpoint {
    surface: Surface,
    version: &'static str,
    url: String,
    sessions: Mutex<BTreeMap<String, Arc<Session>>>,
    seq: AtomicU64,
}

impl std::fmt::Debug for McpEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpEndpoint")
            .field("url", &self.url)
            .field("sessions", &self.active())
            .finish()
    }
}

impl McpEndpoint {
    /// An endpoint over a context, announcing `version`, reachable at `url`.
    pub fn new(ctx: Arc<Context>, version: &'static str, url: String) -> Self {
        McpEndpoint {
            surface: Surface::new(ctx),
            version,
            url,
            sessions: Mutex::new(BTreeMap::new()),
            seq: AtomicU64::new(0),
        }
    }

    /// The base URL of the server this endpoint belongs to.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// How many sessions are open.
    pub fn active(&self) -> usize {
        self.sessions().len()
    }

    /// Answer one request to the endpoint.
    pub fn handle(&self, req: &Request) -> Response {
        match req.method.as_str() {
            "POST" => self.post(req),
            "DELETE" => self.delete(req),
            other => Response::error(
                405,
                "method_not_allowed",
                &format!("{other} {PATH} is not served: POST a JSON-RPC message, DELETE to end the session; this server opens no stream"),
            ),
        }
    }

    fn post(&self, req: &Request) -> Response {
        let message: Value = match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(e) => {
                return Response::error(400, "invalid_json", &format!("the body is not JSON: {e}"))
            }
        };
        let (id, session) = match req.header(SESSION_HEADER) {
            Some(id) => match self.sessions().get(id) {
                Some(s) => (id.to_string(), Arc::clone(s)),
                None => {
                    return Response::error(
                        404,
                        "session_not_found",
                        "unknown or expired Mcp-Session-Id; send initialize again to open a new session",
                    )
                }
            },
            None if is_initialize(&message) => self.open(),
            None => {
                return Response::error(
                    400,
                    "session_required",
                    &format!("the {SESSION_HEADER} header is required on every request after initialize"),
                )
            }
        };
        *lock(&session.last_seen) = Instant::now();
        let response = lock(&session.server).handle(message);
        let mut out = match response {
            None => Response::new(202, "application/json", String::new()),
            Some(reply) => Response::new(
                200,
                "application/json",
                serde_json::to_string(&reply).unwrap_or_default(),
            ),
        };
        out.headers.push((SESSION_HEADER.to_string(), id));
        out
    }

    fn open(&self) -> (String, Arc<Session>) {
        let peer = self.surface.context().peers.attach(Transport::Http);
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let id = format!("{}-{seq}-{nanos:08x}", std::process::id());
        let server = Server::new(self.surface.for_peer(peer.clone()), self.version)
            .with_endpoint(Some(self.url.clone()));
        let session = Arc::new(Session {
            peer: peer.clone(),
            server: Mutex::new(server),
            last_seen: Mutex::new(Instant::now()),
        });
        self.sessions().insert(id.clone(), Arc::clone(&session));
        tracing::info!(session = %id, peer = %peer, "http session opened");
        (id, session)
    }

    fn delete(&self, req: &Request) -> Response {
        let Some(id) = req.header(SESSION_HEADER) else {
            return Response::error(
                400,
                "session_required",
                &format!("DELETE {PATH} names the session to end in {SESSION_HEADER}"),
            );
        };
        match self.sessions().remove(id) {
            Some(s) => {
                self.surface.context().peers.detach(&s.peer);
                tracing::info!(session = %id, peer = %s.peer, "http session closed by the client");
                Response::new(204, "application/json", String::new())
            }
            None => Response::error(404, "session_not_found", "no such session"),
        }
    }

    /// Forget every session silent longer than [`SESSION_IDLE_TIMEOUT`]; the peers it returns are gone.
    pub fn reap(&self) -> Vec<PeerId> {
        self.reap_idle(SESSION_IDLE_TIMEOUT)
    }

    /// Forget every session silent longer than `timeout`.
    pub fn reap_idle(&self, timeout: Duration) -> Vec<PeerId> {
        let mut gone = Vec::new();
        let mut sessions = self.sessions();
        sessions.retain(|id, s| {
            let idle = lock(&s.last_seen).elapsed();
            if idle > timeout {
                tracing::info!(session = %id, peer = %s.peer, idle_seconds = idle.as_secs(), "http session expired");
                gone.push(s.peer.clone());
                false
            } else {
                true
            }
        });
        drop(sessions);
        for p in &gone {
            self.surface.context().peers.detach(p);
        }
        gone
    }

    /// Forget every session: the server is stopping.
    pub fn close_all(&self) {
        let mut sessions = self.sessions();
        for (_, s) in std::mem::take(&mut *sessions) {
            self.surface.context().peers.detach(&s.peer);
        }
    }

    fn sessions(&self) -> MutexGuard<'_, BTreeMap<String, Arc<Session>>> {
        lock(&self.sessions)
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Is this message, or any message of this batch, an `initialize` request?
///
/// ```
/// use majordomus_cli::http::mcp::is_initialize;
/// use serde_json::json;
/// assert!(is_initialize(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" })));
/// assert!(is_initialize(&json!([{ "method": "ping" }, { "method": "initialize" }])));
/// assert!(!is_initialize(&json!({ "method": "ping" })));
/// ```
pub fn is_initialize(message: &Value) -> bool {
    match message {
        Value::Array(batch) => batch.iter().any(is_initialize),
        Value::Object(m) => m.get("method").and_then(Value::as_str) == Some("initialize"),
        _ => false,
    }
}
