//! The blackboard on the live channel: `GET /events?follow=blackboard` (ADR 0176, I2981).
//!
//! The channel is the one `super::events` serves, with the same handshake, origin check,
//! connection bound and heartbeat. What it carries here is the blackboard, read through the
//! registry like any other caller reads it (`blackboard.snapshot`), and the monitor's signals
//! after the connection's cursor (`monitor.stream`). The follower holds no state of its own
//! beyond what it last wrote.
//!
//! ```text
//! GET /events?follow=blackboard                                the blackboard, then each change
//! GET /events?follow=blackboard&after=<cursor>&epoch=<epoch>   the same, resumed: the signals after
//!                                                              the cursor first
//! GET /events?follow=blackboard&checkouts=this                 this checkout's board alone
//! ```
//!
//! The frames, each one JSON document of a `type` and a `data`:
//!
//! - `stream.ready`: the protocol, the heartbeat, and the server's `epoch`. A cursor is only
//!   good on the server process that issued it, so a client that reconnects sends both back.
//! - `blackboard.snapshot`: the whole blackboard, on connect and whenever what it says
//!   changed. A snapshot too large for one frame is written as a `withheld` reference to
//!   `GET /api/v1/blackboard`, which carries it whole.
//! - `blackboard.signals`: what happened after the connection's cursor, and the new cursor.
//! - `stream.resync`: the cursor or epoch the client sent cannot be served. The snapshot that
//!   follows is the client's new starting point.
//! - `stream.heartbeat` and `stream.closing`, as on every connection of the channel.
//!
//! Every `blackboard.*` frame says what caused it in `trigger`:
//!
//! - `connect`: the connection opened.
//! - `change`: the board or the mesh journal changed and said so (`crate::change`).
//! - `refresh`: nothing said so and [`REFRESH`] passed. That is how a source that cannot
//!   push is followed: a runtime whose beat stopped, the executor's trail, GitHub. A reader
//!   can tell what was pushed from what was found.
//!
//! **Shared reads.** One read of the blackboard per generation and per width is shared by
//! every connection. One page of signals per cursor is shared the same way. A hundred open
//! tabs cost one computation per change, not a hundred.
//!
//! **Slow clients.** A write blocks only its own connection's thread, and nothing is queued
//! for it. A slow client skips the snapshots it was too slow to receive and is written the
//! newest one, so its memory is bounded by one frame.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::capability::Context;
use crate::change::BLACKBOARD;

use super::events::{Scope, StreamMessage, HEARTBEAT, LOOK};
use super::ws;

/// How long a follower waits for a change before it reads the blackboard anyway.
pub const REFRESH: Duration = Duration::from_secs(30);

/// How long a change is left to settle before the blackboard is read: a burst of ingested
/// events is read once.
pub const SETTLE: Duration = Duration::from_millis(150);

/// The most pages of signals a resumed connection is replayed before it is told to resync.
const MAX_PAGES: usize = 10;

/// The capability that reads the blackboard and the one that reads the signals, by id.
const SNAPSHOT: &str = "blackboard.snapshot";
const SIGNALS: &str = "monitor.stream";

/// What caused a `blackboard.*` frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// The connection opened.
    Connect,
    /// The board or the journal changed and said so.
    Change,
    /// Nothing said so, and [`REFRESH`] passed.
    Refresh,
}

/// A frame about the blackboard.
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "data")]
enum Frame<'a> {
    #[serde(rename = "blackboard.snapshot")]
    Snapshot {
        trigger: Trigger,
        blackboard: &'a Value,
    },
    #[serde(rename = "blackboard.signals")]
    Signals {
        trigger: Trigger,
        signals: &'a Value,
        cursor: &'a str,
    },
}

/// The query a connection asked: how wide, and where it resumes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Follow {
    /// `repository` or `this`, as the capability takes it.
    pub checkouts: Option<String>,
    /// The cursor the client last read.
    pub after: Option<String>,
    /// The epoch of the server that issued it.
    pub epoch: Option<String>,
}

impl Follow {
    /// The query of a request that follows the blackboard: `None` when it follows something
    /// else, a refusal when it names what this channel does not take.
    pub fn parse(query: &[(String, String)]) -> Option<Result<Follow, String>> {
        if !query
            .iter()
            .any(|(k, v)| k == "follow" && v == "blackboard")
        {
            return None;
        }
        let mut follow = Follow::default();
        for (key, value) in query {
            match key.as_str() {
                "follow" if value == "blackboard" => {}
                "checkouts" if value == "this" || value == "repository" => {
                    follow.checkouts = Some(value.clone())
                }
                "after" => follow.after = Some(value.clone()),
                "epoch" => follow.epoch = Some(value.clone()),
                other => {
                    return Some(Err(format!(
                        "'{other}={value}' is not something the blackboard is followed by; it takes follow=blackboard, checkouts (this or repository), after and epoch"
                    )))
                }
            }
        }
        Some(Ok(follow))
    }
}

/// What this server process's cursors are good for: its mesh stream, which names the
/// process instance, or the process itself when the mesh is not running.
pub fn epoch(ctx: &Context) -> String {
    ctx.mesh
        .cooperation()
        .map(|c| c.journal().own_stream().as_str().to_string())
        .unwrap_or_else(|| format!("process-{}-{}", std::process::id(), started()))
}

fn started() -> u64 {
    static STARTED: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *STARTED.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    })
}

/// One read shared by every connection that asks for the same thing at the same generation.
struct Shared {
    key: String,
    generation: u64,
    at: Instant,
    value: Arc<Value>,
}

/// The newest blackboard read, and the newest page of signals, shared by every follower.
static BOARD: Mutex<Option<Shared>> = Mutex::new(None);
static PAGE: Mutex<Option<Shared>> = Mutex::new(None);

/// The value under `slot` for `key` at `generation`, read once: a caller that finds it read
/// at this generation within [`REFRESH`] takes it; otherwise it reads, under the slot's lock,
/// so that every concurrent caller waits for the one read rather than repeating it.
fn shared(
    slot: &Mutex<Option<Shared>>,
    key: &str,
    generation: u64,
    read: impl FnOnce() -> Result<Value, String>,
) -> Result<Arc<Value>, String> {
    let mut held = slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = held.as_ref() {
        if s.key == key && s.generation >= generation && s.at.elapsed() < REFRESH {
            return Ok(Arc::clone(&s.value));
        }
    }
    let value = Arc::new(read()?);
    *held = Some(Shared {
        key: key.to_string(),
        generation,
        at: Instant::now(),
        value: Arc::clone(&value),
    });
    Ok(value)
}

/// What a snapshot says, without what changes on every read: the time of the read, the
/// seconds since a peer was seen, the runtime beats and the signals, which travel in their
/// own frame. Two snapshots that say the same thing compare equal.
fn substance(snapshot: &Value) -> Value {
    let mut v = snapshot.clone();
    if let Some(o) = v.as_object_mut() {
        for key in ["at", "signals", "cursor", "for_you"] {
            o.remove(key);
        }
        if let Some(workers) = o.get_mut("workers").and_then(Value::as_array_mut) {
            for w in workers {
                if let Some(w) = w.as_object_mut() {
                    w.remove("last_seen_seconds_ago");
                    w.remove("last_beat_ms");
                }
            }
        }
    }
    v
}

/// A follower of the blackboard on one connection.
pub struct Follower {
    ctx: Arc<Context>,
    follow: Follow,
    /// The route of `blackboard.snapshot`, which a withheld snapshot names.
    route: String,
}

impl Follower {
    /// A follower of `follow`, reading through `ctx`'s registry.
    pub fn new(ctx: Arc<Context>, follow: Follow) -> Self {
        let route = ctx
            .registry
            .get(SNAPSHOT)
            .and_then(|c| c.exposure.http.as_ref())
            .map(|h| h.path.clone())
            .unwrap_or_default();
        Self { ctx, follow, route }
    }

    fn snapshot(&self, generation: u64) -> Result<Arc<Value>, String> {
        // reader-neutral: what is for whom is the client's to mark, so that one read serves
        // every connection
        let checkouts = self.follow.checkouts.as_deref().unwrap_or("repository");
        shared(&BOARD, checkouts, generation, || {
            self.ctx
                .execute(SNAPSHOT, json!({ "checkouts": checkouts, "signals": 0 }))
                .map_err(|e| e.to_string())
        })
    }

    /// Where the signals stand now: the cursor a fresh connection starts from.
    fn now(&self, generation: u64) -> Result<String, String> {
        let page = shared(&PAGE, "", generation, || {
            self.ctx
                .execute(SIGNALS, json!({ "limit": 1, "sources": "mesh,executor" }))
                .map_err(|e| e.to_string())
        })?;
        Ok(page["cursor"].as_str().unwrap_or_default().to_string())
    }

    /// The signals after `after`, every page of them, or why they cannot be read from there.
    fn signals(&self, after: &str, generation: u64) -> Result<(Vec<Value>, String), String> {
        let mut cursor = after.to_string();
        let mut signals = Vec::new();
        for _ in 0..MAX_PAGES {
            let page = shared(&PAGE, &cursor, generation, || {
                self.ctx
                    .execute(
                        SIGNALS,
                        json!({ "after": cursor, "limit": 500, "sources": "mesh,executor" }),
                    )
                    .map_err(|e| e.to_string())
            })?;
            signals.extend(page["signals"].as_array().cloned().unwrap_or_default());
            cursor = page["cursor"].as_str().unwrap_or(&cursor).to_string();
            if page["more"].as_bool() != Some(true) {
                return Ok((signals, cursor));
            }
        }
        Err(format!(
            "more than {MAX_PAGES} pages of signals happened after the cursor; read the blackboard again"
        ))
    }

    /// Write the snapshot, or a reference to it when it is too large for one frame.
    fn write_snapshot(
        &self,
        socket: &mut Box<dyn std::io::Write + Send>,
        trigger: Trigger,
        snapshot: &Value,
    ) -> bool {
        let frame = Frame::Snapshot {
            trigger,
            blackboard: snapshot,
        };
        let Ok(text) = serde_json::to_string(&frame) else {
            return true;
        };
        if text.len() <= ws::MAX_FRAME_BYTES {
            return frame_ok(socket, &text);
        }
        let checkouts = self.follow.checkouts.as_deref().unwrap_or("repository");
        let withheld = json!({
            "withheld": "too_large",
            "bytes": text.len(),
            "limit": ws::MAX_FRAME_BYTES,
            "self": format!("{}?checkouts={checkouts}", self.route),
        });
        write(
            socket,
            &Frame::Snapshot {
                trigger,
                blackboard: &withheld,
            },
        )
    }

    /// Serve the connection until the client hangs up or a write fails.
    pub fn run(self, mut socket: Box<dyn std::io::Write + Send>, hung_up: impl Fn() -> bool) {
        let epoch = epoch(&self.ctx);
        let mut generation = BLACKBOARD.current();
        let ready = StreamMessage::Ready {
            protocol_version: crate::execution::PROTOCOL_VERSION.to_string(),
            scope: Scope::Blackboard,
            executions: Vec::new(),
            replayed: 0,
            truncated: false,
            heartbeat_seconds: HEARTBEAT.as_secs(),
            epoch: Some(epoch.clone()),
        };
        if !write(&mut socket, &ready) {
            return;
        }
        let first = match self.snapshot(generation) {
            Ok(s) => s,
            Err(reason) => {
                let _ = write(&mut socket, &StreamMessage::Closing { reason });
                let _ = ws::write_close(&mut socket, 1011, "the blackboard could not be read");
                return;
            }
        };
        // the snapshot carries no signals: they travel by the connection's own cursor
        let mut cursor = match self.now(generation) {
            Ok(c) => c,
            Err(reason) => {
                let _ = write(&mut socket, &StreamMessage::Closing { reason });
                let _ = ws::write_close(&mut socket, 1011, "the signals could not be read");
                return;
            }
        };
        // a resumed connection is told what happened after its cursor, when the cursor is
        // this process's; anything else starts again from the snapshot, and is told so
        let mut resync = None;
        if let Some(after) = &self.follow.after {
            if self.follow.epoch.as_deref() != Some(epoch.as_str()) {
                resync = Some(format!(
                    "the cursor was issued by another server process (epoch {}); this one is {epoch}",
                    self.follow.epoch.as_deref().unwrap_or("not given")
                ));
            } else {
                match self.signals(after, generation) {
                    Ok((signals, next)) => {
                        if !signals.is_empty()
                            && !write(
                                &mut socket,
                                &Frame::Signals {
                                    trigger: Trigger::Connect,
                                    signals: &Value::Array(signals),
                                    cursor: &next,
                                },
                            )
                        {
                            return;
                        }
                        cursor = next;
                    }
                    Err(reason) => resync = Some(reason),
                }
            }
        }
        if let Some(reason) = resync {
            if !write(&mut socket, &StreamMessage::Resync { reason }) {
                return;
            }
        }
        if !self.write_snapshot(&mut socket, Trigger::Connect, &first) {
            return;
        }
        let mut said = substance(&first);
        let mut quiet = Instant::now();
        let mut read_at = Instant::now();
        loop {
            let wait = HEARTBEAT
                .saturating_sub(quiet.elapsed())
                .min(REFRESH.saturating_sub(read_at.elapsed()))
                .min(LOOK);
            let now = BLACKBOARD.wait_past(generation, wait);
            let trigger = if now > generation {
                std::thread::sleep(SETTLE);
                Some(Trigger::Change)
            } else if read_at.elapsed() >= REFRESH {
                Some(Trigger::Refresh)
            } else {
                None
            };
            if let Some(trigger) = trigger {
                generation = BLACKBOARD.current();
                read_at = Instant::now();
                if let Ok(snapshot) = self.snapshot(generation) {
                    let substance = substance(&snapshot);
                    if substance != said {
                        if !self.write_snapshot(&mut socket, trigger, &snapshot) {
                            break;
                        }
                        said = substance;
                        quiet = Instant::now();
                    }
                }
                match self.signals(&cursor, generation) {
                    Ok((signals, next)) => {
                        if !signals.is_empty() {
                            if !write(
                                &mut socket,
                                &Frame::Signals {
                                    trigger,
                                    signals: &Value::Array(signals),
                                    cursor: &next,
                                },
                            ) {
                                break;
                            }
                            quiet = Instant::now();
                        }
                        cursor = next;
                    }
                    Err(reason) => {
                        // the client starts again from a snapshot with a cursor of its own
                        let Ok(snapshot) = self.snapshot(generation) else {
                            continue;
                        };
                        if !write(&mut socket, &StreamMessage::Resync { reason })
                            || !self.write_snapshot(&mut socket, trigger, &snapshot)
                        {
                            break;
                        }
                        if let Ok(now) = self.now(generation) {
                            cursor = now;
                        }
                        said = substance(&snapshot);
                        quiet = Instant::now();
                    }
                }
                continue;
            }
            if hung_up() {
                tracing::debug!("the client of a blackboard follower hung up");
                break;
            }
            if quiet.elapsed() >= HEARTBEAT {
                let alive = ws::write_ping(&mut socket).is_ok()
                    && write(
                        &mut socket,
                        &StreamMessage::Heartbeat {
                            at: crate::peers::rfc3339(std::time::SystemTime::now()),
                        },
                    );
                if !alive {
                    break;
                }
                quiet = Instant::now();
            }
        }
    }
}

fn write<T: Serialize>(socket: &mut Box<dyn std::io::Write + Send>, value: &T) -> bool {
    match serde_json::to_string(value) {
        Ok(text) => frame_ok(socket, &text),
        Err(_) => true,
    }
}

fn frame_ok(socket: &mut Box<dyn std::io::Write + Send>, text: &str) -> bool {
    ws::write_text(socket.as_mut(), text).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn a_follow_query_is_read_and_anything_else_is_left_alone() {
        assert_eq!(Follow::parse(&q(&[("execution", "e1")])), None);
        let f = Follow::parse(&q(&[
            ("follow", "blackboard"),
            ("after", "m1.1.x"),
            ("epoch", "e"),
            ("checkouts", "this"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(f.after.as_deref(), Some("m1.1.x"));
        assert_eq!(f.epoch.as_deref(), Some("e"));
        assert_eq!(f.checkouts.as_deref(), Some("this"));
        let refused = Follow::parse(&q(&[("follow", "blackboard"), ("execution", "e1")]))
            .unwrap()
            .unwrap_err();
        assert!(refused.contains("'execution=e1'"), "{refused}");
        assert!(
            Follow::parse(&q(&[("follow", "blackboard"), ("checkouts", "all")]))
                .unwrap()
                .is_err()
        );
    }

    /// Two snapshots that differ only in what changes on every read say the same thing.
    #[test]
    fn the_substance_of_a_snapshot_ignores_the_clock() {
        let a = json!({"at": "1", "cursor": "x1", "signals": [1], "for_you": 0,
                       "workers": [{"key": "k", "last_seen_seconds_ago": 1, "last_beat_ms": 5}]});
        let b = json!({"at": "2", "cursor": "x2", "signals": [], "for_you": 1,
                       "workers": [{"key": "k", "last_seen_seconds_ago": 9, "last_beat_ms": 50}]});
        assert_eq!(substance(&a), substance(&b));
        let c = json!({"at": "2", "workers": [{"key": "other"}]});
        assert_ne!(substance(&a), substance(&c));
    }

    /// A shared read is read once per generation: a second caller at the same generation
    /// takes the first one's value, a newer generation reads again.
    #[test]
    fn a_shared_read_is_read_once_per_generation() {
        let slot = Mutex::new(None);
        let reads = std::cell::Cell::new(0);
        let read = || {
            reads.set(reads.get() + 1);
            Ok(json!(reads.get()))
        };
        assert_eq!(*shared(&slot, "k", 1, read).unwrap(), json!(1));
        assert_eq!(*shared(&slot, "k", 1, read).unwrap(), json!(1), "shared");
        assert_eq!(
            *shared(&slot, "k", 2, read).unwrap(),
            json!(2),
            "a new generation"
        );
        assert_eq!(
            *shared(&slot, "other", 2, read).unwrap(),
            json!(3),
            "another key"
        );
        assert_eq!(reads.get(), 3);
    }
}
