//! The live channel: one WebSocket over which this process reports what its executions are
//! doing.
//!
//! It is an adapter and holds no execution logic. It subscribes to
//! [`crate::execution::ExecutionStore`], renders what arrives, and closes; every decision
//! about what an execution is, what may happen to it and what an event means was already
//! taken in [`crate::execution`]. That is what makes an audit trail, a notifier or a
//! second protocol something that subscribes here rather than something that has to be
//! threaded through the engine.
//!
//! # The protocol
//!
//! ```text
//! GET /events                                  every execution of this process, live
//! GET /events?execution=<id>                   one execution, its whole retained history, then live
//! GET /events?execution=<id>&after=<sequence>  the same, from a cursor: what a reconnect asks for
//! ```
//!
//! The subscription is the request target, so there is nothing to send and nothing to
//! acknowledge: a reconnect is a new URL with the sequence the client last saw, and the
//! server replays the gap before it goes live. What the server writes is one JSON document
//! per text frame, each with a `type` and a `data`: either a
//! [`crate::execution::ExecutionEvent`], or one of the [`StreamMessage`] control messages
//! below. A client that does not recognise a `type` must ignore that frame; that is what
//! lets a variant be added without a version change.
//!
//! The server pings on a heartbeat and never reads. A client that goes away is noticed
//! when the next write fails, which the heartbeat guarantees happens; a client that sends
//! anything is ignored, and the reason is in [`super::ws`].

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::execution::{Delivery, ExecutionId, ExecutionStore, Filter, PROTOCOL_VERSION};

use super::router::{Request, Response};
use super::ws;

/// Where the live channel is mounted. Not under the capability prefix: it is not a
/// capability, it declares no input or output schema, and it answers no JSON over HTTP.
pub const PATH: &str = "/events";

/// How often the server pings a quiet connection.
pub const HEARTBEAT: Duration = Duration::from_secs(20);

/// How many live channels one process serves at once. Beyond it, a connection is refused
/// with a reason rather than accepted and starved.
pub const MAX_CONNECTIONS: usize = 64;

/// How many retained events a single-execution subscription replays before going live.
pub const MAX_REPLAY: usize = 1_000;

/// A message about the stream itself, rather than about an execution.
///
/// Same envelope as an event — a `type` and a `data` — so a client has one reader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum StreamMessage {
    /// The first frame of every connection: what was subscribed to, what was replayed, and
    /// what the client should expect.
    #[serde(rename = "stream.ready")]
    Ready {
        /// The event protocol this server speaks, [`PROTOCOL_VERSION`].
        protocol_version: String,
        /// What this connection follows.
        scope: Scope,
        /// The executions followed, for a scoped connection.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        executions: Vec<String>,
        /// How many retained events were written before this connection went live.
        replayed: usize,
        /// Whether the history before the replay had already been dropped by the store's
        /// bound, so a client knows its picture starts in the middle.
        truncated: bool,
        /// How many seconds of quiet before the server pings.
        heartbeat_seconds: u64,
    },
    /// The client fell behind and events were dropped rather than buffered. It must read
    /// the snapshot and the history again, from the last sequence it saw.
    #[serde(rename = "stream.lagged")]
    Lagged {
        /// How many events were dropped.
        dropped: u64,
    },
    /// The server is closing this connection.
    #[serde(rename = "stream.closing")]
    Closing {
        /// Why.
        reason: String,
    },
}

/// What a connection follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Every execution of this process.
    All,
    /// The executions named in the request target.
    Executions,
}

/// How many live channels this process has open.
static OPEN: AtomicUsize = AtomicUsize::new(0);

/// How many live channels are open right now, for the health and perf projections.
pub fn open_connections() -> usize {
    OPEN.load(Ordering::Relaxed)
}

/// An accepted connection, before the socket is handed over.
pub struct Accepted {
    /// The value to answer `Sec-WebSocket-Accept` with.
    pub accept: String,
    stream: LiveStream,
}

impl Accepted {
    /// Serve the connection on an upgraded socket until it ends. Blocking; the caller runs
    /// it on a thread of its own.
    pub fn serve(self, socket: Box<dyn std::io::Write + Send>) {
        self.stream.run(socket);
    }
}

struct LiveStream {
    store: Arc<ExecutionStore>,
    filter: Filter,
    after: u64,
}

/// Decide what a request to [`PATH`] means: an accepted connection, or the response to
/// send instead.
///
/// The origin is checked here for the same reason it is checked for a state-changing
/// request: a page on another origin can open a WebSocket to this server, and the
/// same-origin policy does not stop it. This does.
pub fn accept(store: &Arc<ExecutionStore>, req: &Request) -> Result<Accepted, Response> {
    if !ws::is_upgrade(req) {
        return Err(Response::error(
            426,
            "upgrade_required",
            &format!(
                "{PATH} is a WebSocket. Open it with a WebSocket client; the same events are readable over HTTP at {}executions/events",
                crate::capability::model::HttpExposure::PREFIX
            ),
        ));
    }
    if let Some(origin) = req.header("origin") {
        let host = req.header("host").unwrap_or_default();
        let ours = [format!("http://{host}"), format!("https://{host}")];
        if !ours.iter().any(|a| a == origin) {
            tracing::warn!(
                origin = origin,
                "a live channel from another origin was refused"
            );
            return Err(Response::error(
                403,
                "forbidden",
                &format!("a WebSocket from origin '{origin}' is refused: this server accepts one from its own origin only"),
            ));
        }
    }
    let accept = match ws::handshake(req) {
        ws::Handshake::Accept(value) => value,
        ws::Handshake::Refuse { status, reason } => {
            return Err(Response::error(status, "invalid_input", &reason))
        }
    };
    if OPEN.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
        return Err(Response::error(
            503,
            "unavailable",
            &format!("this process serves {MAX_CONNECTIONS} live channels at once and they are all in use"),
        ));
    }
    let mut executions = Vec::new();
    let mut after = 0u64;
    for (key, value) in &req.query {
        match key.as_str() {
            "execution" => match ExecutionId::parse(value) {
                Some(id) => executions.push(id),
                None => {
                    return Err(Response::error(
                        400,
                        "invalid_input",
                        &format!("'{value}' is not an execution id"),
                    ))
                }
            },
            "after" => match value.parse::<u64>() {
                Ok(n) => after = n,
                Err(_) => {
                    return Err(Response::error(
                        400,
                        "invalid_input",
                        &format!("after '{value}' is not a sequence number"),
                    ))
                }
            },
            other => {
                return Err(Response::error(
                    400,
                    "invalid_input",
                    &format!("unknown parameter '{other}'; {PATH} takes 'execution' and 'after'"),
                ))
            }
        }
    }
    let filter = if executions.is_empty() {
        Filter::All
    } else {
        Filter::Only(executions)
    };
    Ok(Accepted {
        accept,
        stream: LiveStream {
            store: Arc::clone(store),
            filter,
            after,
        },
    })
}

impl LiveStream {
    fn run(self, mut socket: Box<dyn std::io::Write + Send>) {
        let open = OPEN.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::debug!(connections = open, "a live channel opened");
        let (subscriber, rx) = self.store.subscribe(self.filter.clone());
        // subscribe first, replay second, and skip in the live stream anything the replay
        // already wrote: that is what leaves no gap between the history and the stream and
        // no event written twice.
        let mut written = self.after;
        let (replayed, truncated) = match &self.filter {
            Filter::Only(ids) if ids.len() == 1 => {
                match self.store.events(&ids[0], self.after, MAX_REPLAY) {
                    Some(page) => (page.events, page.truncated),
                    None => (Vec::new(), false),
                }
            }
            // an all-executions connection has no single cursor to replay from; a client
            // that needs history asks for it per execution over HTTP
            _ => (Vec::new(), false),
        };
        let ready = StreamMessage::Ready {
            protocol_version: PROTOCOL_VERSION.to_string(),
            scope: match self.filter {
                Filter::All => Scope::All,
                Filter::Only(_) => Scope::Executions,
            },
            executions: match &self.filter {
                Filter::All => Vec::new(),
                Filter::Only(ids) => ids.iter().map(|i| i.to_string()).collect(),
            },
            replayed: replayed.len(),
            truncated,
            heartbeat_seconds: HEARTBEAT.as_secs(),
        };
        let mut alive = write(&mut socket, &ready);
        for event in replayed {
            if !alive {
                break;
            }
            written = written.max(event.sequence);
            alive = write(&mut socket, &*event);
        }
        while alive {
            match rx.recv_timeout(HEARTBEAT) {
                Ok(Delivery::Event(event)) => {
                    if event.sequence <= written && matches!(self.filter, Filter::Only(_)) {
                        continue; // the replay already wrote it
                    }
                    written = written.max(event.sequence);
                    alive = write(&mut socket, &*event);
                }
                Ok(Delivery::Lagged { dropped }) => {
                    alive = write(&mut socket, &StreamMessage::Lagged { dropped });
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    alive = ws::write_ping(&mut socket).is_ok();
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = write(
                        &mut socket,
                        &StreamMessage::Closing {
                            reason: "this server is shutting down".into(),
                        },
                    );
                    let _ = ws::write_close(&mut socket, 1001, "going away");
                    break;
                }
            }
        }
        self.store.unsubscribe(subscriber);
        let open = OPEN.fetch_sub(1, Ordering::Relaxed) - 1;
        tracing::debug!(connections = open, "a live channel closed");
    }
}

/// Write one value as a text frame; `false` when the client is gone.
fn write<T: Serialize>(socket: &mut Box<dyn std::io::Write + Send>, value: &T) -> bool {
    let Ok(text) = serde_json::to_string(value) else {
        tracing::error!("an execution event could not be rendered as JSON");
        return true;
    };
    match ws::write_text(socket.as_mut(), &text) {
        Ok(()) => true,
        Err(e) => {
            tracing::debug!("a live channel went away: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::{
        Actor, ActorKind, EventPayload, ExecutionState, Limits, ProgressView, RepositoryRef,
    };

    fn upgrade_request(target: &str) -> Request {
        Request::parse_target("GET", target, vec![]).with_headers(vec![
            ("Upgrade".into(), "websocket".into()),
            ("Connection".into(), "Upgrade".into()),
            (
                "Sec-WebSocket-Key".into(),
                "dGhlIHNhbXBsZSBub25jZQ==".into(),
            ),
            ("Sec-WebSocket-Version".into(), "13".into()),
        ])
    }

    fn store_with_one() -> (Arc<ExecutionStore>, ExecutionId) {
        let store = Arc::new(ExecutionStore::new(Limits::default()));
        let id = ExecutionId::fresh();
        store.create(
            id.clone(),
            "demo.x",
            "Demo",
            serde_json::json!({}),
            true,
            Actor::of(ActorKind::Internal),
            RepositoryRef {
                name: "r".into(),
                id: "i".into(),
                branch: None,
            },
        );
        store.publish(&id, EventPayload::Started);
        store.publish(
            &id,
            EventPayload::Progress(ProgressView {
                current: 1,
                total: Some(1),
                message: None,
            }),
        );
        store.publish(
            &id,
            EventPayload::Completed {
                output: serde_json::json!({}),
            },
        );
        (store, id)
    }

    #[test]
    fn a_plain_get_is_told_what_this_path_is_and_where_the_same_events_are() {
        let (store, _) = store_with_one();
        let response = accept(&store, &Request::parse_target("GET", PATH, vec![]))
            .err()
            .expect("a plain GET is not a connection");
        assert_eq!(response.status, 426);
        assert!(response.body.text().contains("executions/events"));
    }

    #[test]
    fn a_foreign_origin_is_refused_before_the_handshake() {
        let (store, _) = store_with_one();
        let req = upgrade_request(PATH).with_headers({
            let mut h = upgrade_request(PATH).headers;
            h.push(("Host".into(), "127.0.0.1:8741".into()));
            h.push(("Origin".into(), "https://evil.example".into()));
            h
        });
        let response = accept(&store, &req).err().expect("refused");
        assert_eq!(response.status, 403);
        let ours = upgrade_request(PATH).with_headers({
            let mut h = upgrade_request(PATH).headers;
            h.push(("Host".into(), "127.0.0.1:8741".into()));
            h.push(("Origin".into(), "http://127.0.0.1:8741".into()));
            h
        });
        assert!(accept(&store, &ours).is_ok(), "our own page is not foreign");
    }

    #[test]
    fn the_target_is_the_whole_subscription_and_is_validated() {
        let (store, id) = store_with_one();
        assert!(
            accept(&store, &upgrade_request(PATH)).is_ok(),
            "every execution"
        );
        assert!(accept(
            &store,
            &upgrade_request(&format!("{PATH}?execution={id}&after=2"))
        )
        .is_ok());
        for bad in [
            format!("{PATH}?execution=../../etc/passwd"),
            format!("{PATH}?execution={id}&after=soon"),
            format!("{PATH}?follow=everything"),
        ] {
            let response = accept(&store, &upgrade_request(&bad)).err().expect(&bad);
            assert_eq!(response.status, 400, "{bad}");
        }
    }

    /// The whole loop over a socket that is a `Vec`: the ready frame, the replay from a
    /// cursor, and the end when the client goes away. It proves the ordering contract —
    /// history first, live second, nothing written twice — without a network, and it proves
    /// the only way a connection ends in production: the next write fails.
    #[test]
    fn a_connection_replays_from_its_cursor_and_ends_when_the_client_goes() {
        let (store, id) = store_with_one();
        let accepted = accept(
            &store,
            &upgrade_request(&format!("{PATH}?execution={id}&after=2")),
        )
        .expect("accepted");
        assert_eq!(accepted.accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        // the client reads three frames and then goes away, which is what a closed tab is
        let sink = SharedSink::new(3);
        let writer = Box::new(sink.clone());
        let handle = std::thread::spawn(move || accepted.serve(writer));
        // one more event, so the connection finds the client gone at its next write rather
        // than at its next heartbeat: the same code path, without the wait
        nudge(&store, &id);
        handle
            .join()
            .expect("the connection thread ended by itself");

        let frames = sink.frames();
        let types: Vec<String> = frames
            .iter()
            .filter_map(|f| serde_json::from_str::<serde_json::Value>(f).ok())
            .map(|v| v["type"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(
            types,
            ["stream.ready", "execution.progress", "execution.completed"],
            "the first frame says what was subscribed to, then the cursor is honoured"
        );
        let ready: serde_json::Value = serde_json::from_str(&frames[0]).unwrap();
        assert_eq!(ready["data"]["protocol_version"], PROTOCOL_VERSION);
        assert_eq!(ready["data"]["scope"], "executions");
        assert_eq!(
            ready["data"]["replayed"], 2,
            "sequences 3 and 4, not 1 and 2"
        );
        assert_eq!(ready["data"]["truncated"], false);
        assert_eq!(ready["data"]["heartbeat_seconds"], HEARTBEAT.as_secs());
        assert_eq!(store.subscribers(), 0, "its subscription was ended");
        // the events describe the state the snapshot holds
        assert_eq!(
            store.get(&id).map(|e| e.state),
            Some(ExecutionState::Succeeded)
        );
    }

    /// A connection that follows every execution replays nothing: there is no single
    /// cursor across executions, and a client that needs history asks per execution.
    #[test]
    fn an_unscoped_connection_replays_nothing_and_says_so() {
        let (store, _) = store_with_one();
        let accepted = accept(&store, &upgrade_request(PATH)).expect("accepted");
        let sink = SharedSink::new(1);
        let writer = Box::new(sink.clone());
        let handle = std::thread::spawn(move || accepted.serve(writer));
        let id = store.list(None, None, 1)[0].id.clone();
        nudge(&store, &id);
        handle.join().expect("ended");
        let ready: serde_json::Value = serde_json::from_str(&sink.frames()[0]).unwrap();
        assert_eq!(ready["type"], "stream.ready");
        assert_eq!(ready["data"]["scope"], "all");
        assert_eq!(ready["data"]["replayed"], 0);
        assert!(ready["data"]["executions"].is_null(), "it names none");
    }

    /// One more event on an execution, so a connection whose client has gone finds out at
    /// its next write instead of at its next heartbeat. A log line changes no state.
    fn nudge(store: &Arc<ExecutionStore>, id: &crate::execution::ExecutionId) {
        // the subscriber is registered before the replay, so this arrives after it
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(10));
            if store.subscribers() > 0 {
                break;
            }
        }
        store.publish(
            id,
            EventPayload::Log {
                stream: crate::execution::LogStream::Handler,
                message: "the client is gone by now".into(),
            },
        );
    }

    /// A `Write` that keeps the WebSocket frames it was given and then goes away, so the
    /// connection loop can be run without a socket. It decodes only what this server
    /// writes: unmasked, unfragmented frames.
    #[derive(Clone)]
    struct SharedSink {
        bytes: Arc<std::sync::Mutex<Vec<u8>>>,
        /// How many frames it accepts before the writes start failing, as a closed tab's
        /// socket does.
        accepts: Arc<AtomicUsize>,
    }

    impl SharedSink {
        fn new(accepts: usize) -> Self {
            SharedSink {
                bytes: Arc::new(std::sync::Mutex::new(Vec::new())),
                accepts: Arc::new(AtomicUsize::new(accepts)),
            }
        }

        fn frames(&self) -> Vec<String> {
            let bytes = self.bytes.lock().unwrap();
            let mut out = Vec::new();
            let mut i = 0usize;
            while i + 2 <= bytes.len() {
                let opcode = bytes[i] & 0x0f;
                let short = bytes[i + 1] as usize;
                let (len, header) = match short {
                    126 => (u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize, 4),
                    127 => (
                        u64::from_be_bytes(bytes[i + 2..i + 10].try_into().unwrap()) as usize,
                        10,
                    ),
                    n => (n, 2),
                };
                let start = i + header;
                let end = start + len;
                if end > bytes.len() {
                    break;
                }
                if opcode == 0x1 {
                    out.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
                }
                i = end;
            }
            out
        }
    }

    impl std::io::Write for SharedSink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.accepts.load(Ordering::SeqCst) == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "the client went away",
                ));
            }
            self.accepts.fetch_sub(1, Ordering::SeqCst);
            self.bytes.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
