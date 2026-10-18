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
//!   changed, with the connection's `cursor`, so that a client that was sent no signal still
//!   has a point to resume from. A snapshot too large for one frame is written as a
//!   `withheld` reference to `GET /api/v1/blackboard`, which carries it whole.
//! - `blackboard.signals`: what happened after the connection's cursor, and the new cursor:
//!   one frame per page, each within the frame bound, the cursor that of the page's last
//!   signal.
//! - `stream.lagged`: changes passed while this connection was still being written to and
//!   were not each sent as a snapshot of their own; the next snapshot holds them.
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
//! for it. A slow client skips the snapshots it was too slow to receive, is told so
//! (`stream.lagged`), and is written the newest one, so its memory is bounded by one frame.
//! Whether the client hung up, and the heartbeat, are asked on every round, a change or not.

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

/// The most signals one page asks for; a page whose frame would pass the frame bound is asked
/// again with half as many, so every frame written fits.
const PAGE_LIMIT: u64 = 500;

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
        /// Where this connection's signals stand: a client that reconnects with it is told
        /// what happened after this snapshot, even when no signal was sent before it.
        cursor: &'a str,
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
        // keyed by what is asked, so that the latest signal is never the page after an
        // empty cursor
        let page = shared(&PAGE, "latest", generation, || {
            self.ctx
                .execute(SIGNALS, json!({ "limit": 1, "sources": "mesh,executor" }))
                .map_err(|e| e.to_string())
        })?;
        Ok(page["cursor"].as_str().unwrap_or_default().to_string())
    }

    /// One page of the signals after `after`, at most `limit` of them.
    fn page(&self, after: &str, limit: u64, generation: u64) -> Result<Value, String> {
        let key = format!("after|{limit}|{after}");
        let page = shared(&PAGE, &key, generation, || {
            self.ctx
                .execute(
                    SIGNALS,
                    json!({ "after": after, "limit": limit, "sources": "mesh,executor" }),
                )
                .map_err(|e| e.to_string())
        })?;
        Ok((*page).clone())
    }

    /// Write the signals after `after`, one frame per page, each within the frame bound.
    fn send_signals(
        &self,
        socket: &mut Box<dyn std::io::Write + Send>,
        trigger: Trigger,
        after: &str,
        generation: u64,
    ) -> Sent {
        send_pages(
            after,
            |cursor, limit| self.page(cursor, limit, generation),
            |text| frame_ok(socket, text),
            trigger,
        )
    }

    /// Write the snapshot, or a reference to it when it is too large for one frame, with the
    /// connection's cursor either way.
    fn write_snapshot(
        &self,
        socket: &mut Box<dyn std::io::Write + Send>,
        trigger: Trigger,
        snapshot: &Value,
        cursor: &str,
    ) -> bool {
        let frame = Frame::Snapshot {
            trigger,
            blackboard: snapshot,
            cursor,
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
                cursor,
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
                match self.send_signals(&mut socket, Trigger::Connect, after, generation) {
                    Sent::Through { cursor: next, .. } => cursor = next,
                    Sent::Unreadable(reason) => resync = Some(reason),
                    Sent::Gone => return,
                }
            }
        }
        if let Some(reason) = resync {
            if !write(&mut socket, &StreamMessage::Resync { reason }) {
                return;
            }
        }
        if !self.write_snapshot(&mut socket, Trigger::Connect, &first, &cursor) {
            return;
        }
        let mut said = substance(&first);
        let mut quiet = Instant::now();
        let mut read_at = Instant::now();
        // how long the last round of writes took: a change that passes while this connection
        // is still being written to is not sent as its own snapshot, and the client is told
        let mut busy = Duration::ZERO;
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
                let latest = BLACKBOARD.current();
                let skipped = latest.saturating_sub(generation).saturating_sub(1);
                if trigger == Trigger::Change && skipped > 0 && busy > SETTLE {
                    if !write(&mut socket, &StreamMessage::Lagged { dropped: skipped }) {
                        break;
                    }
                    quiet = Instant::now();
                }
                generation = latest;
                read_at = Instant::now();
                let writing = Instant::now();
                if let Ok(snapshot) = self.snapshot(generation) {
                    let substance = substance(&snapshot);
                    if substance != said {
                        if !self.write_snapshot(&mut socket, trigger, &snapshot, &cursor) {
                            break;
                        }
                        said = substance;
                        quiet = Instant::now();
                    }
                }
                match self.send_signals(&mut socket, trigger, &cursor, generation) {
                    Sent::Through {
                        cursor: next,
                        wrote,
                    } => {
                        if wrote {
                            quiet = Instant::now();
                        }
                        cursor = next;
                    }
                    Sent::Gone => break,
                    Sent::Unreadable(reason) => {
                        // the client starts again from a snapshot and the cursor it carries
                        if let Ok(snapshot) = self.snapshot(generation) {
                            if let Ok(now) = self.now(generation) {
                                cursor = now;
                            }
                            if !write(&mut socket, &StreamMessage::Resync { reason })
                                || !self.write_snapshot(&mut socket, trigger, &snapshot, &cursor)
                            {
                                break;
                            }
                            said = substance(&snapshot);
                            quiet = Instant::now();
                        }
                    }
                }
                busy = writing.elapsed();
            }
            // asked on every round, a change or not: a stream of changes that say nothing new
            // must not keep a departed client's thread, nor starve the heartbeat
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

/// What sending the signals after a cursor came to.
#[derive(Debug, PartialEq, Eq)]
enum Sent {
    /// Every signal after the cursor was written, in frames within the frame bound: the cursor
    /// after the last one delivered, and whether anything was written.
    Through { cursor: String, wrote: bool },
    /// They cannot be sent from there: the client starts again from a snapshot.
    Unreadable(String),
    /// A write failed: the client is gone.
    Gone,
}

/// The pages of signals after `after`, read through `fetch` and written through `emit`, one
/// frame each. A page whose frame would pass [`ws::MAX_FRAME_BYTES`] is read again with half
/// as many signals, so the cursor only ever advances through signals that were written; a
/// single signal larger than a frame, or more than [`MAX_PAGES`] pages, is a resync.
fn send_pages(
    after: &str,
    mut fetch: impl FnMut(&str, u64) -> Result<Value, String>,
    mut emit: impl FnMut(&str) -> bool,
    trigger: Trigger,
) -> Sent {
    let mut cursor = after.to_string();
    let mut limit = PAGE_LIMIT;
    let mut wrote = false;
    let mut pages = 0;
    loop {
        let page = match fetch(&cursor, limit) {
            Ok(page) => page,
            Err(reason) => return Sent::Unreadable(reason),
        };
        let next = page["cursor"].as_str().unwrap_or(&cursor).to_string();
        let more = page["more"].as_bool() == Some(true);
        let signals = page
            .get("signals")
            .cloned()
            .unwrap_or(Value::Array(Vec::new()));
        if signals.as_array().is_some_and(|s| !s.is_empty()) {
            let frame = Frame::Signals {
                trigger,
                signals: &signals,
                cursor: &next,
            };
            let Ok(text) = serde_json::to_string(&frame) else {
                return Sent::Unreadable("a page of signals could not be encoded".into());
            };
            if text.len() > ws::MAX_FRAME_BYTES {
                if limit > 1 {
                    limit = (limit / 2).max(1);
                    continue;
                }
                return Sent::Unreadable(format!(
                    "a signal after the cursor is larger than one frame ({} bytes, the bound is {})",
                    text.len(),
                    ws::MAX_FRAME_BYTES
                ));
            }
            if !emit(&text) {
                return Sent::Gone;
            }
            wrote = true;
        }
        cursor = next;
        limit = PAGE_LIMIT;
        pages += 1;
        if !more {
            return Sent::Through { cursor, wrote };
        }
        if pages >= MAX_PAGES {
            return Sent::Unreadable(format!(
                "more than {MAX_PAGES} pages of signals happened after the cursor; read the blackboard again"
            ));
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

    /// A log of signals that `fetch` pages from: each signal is `size` bytes of text, its
    /// cursor is its position, and a page holds at most `limit` of them.
    fn log(count: usize, size: usize) -> impl FnMut(&str, u64) -> Result<Value, String> {
        move |after: &str, limit: u64| {
            let from: usize = after.parse().unwrap_or(0);
            let to = (from + limit as usize).min(count);
            let signals: Vec<Value> = (from..to)
                .map(|i| json!({ "n": i, "summary": "x".repeat(size) }))
                .collect();
            Ok(json!({ "signals": signals, "cursor": to.to_string(), "more": to < count }))
        }
    }

    /// Every frame of signals fits the frame bound: a page that would not is asked again with
    /// fewer signals, every signal is written once and in order, and each frame's cursor is
    /// the position after its last signal, so a client that keeps any frame's cursor resumes
    /// after exactly what it received.
    #[test]
    fn a_backlog_is_written_in_frames_within_the_bound() {
        let mut frames: Vec<Value> = Vec::new();
        let sent = send_pages(
            "0",
            log(1200, 600),
            |text| {
                assert!(text.len() <= ws::MAX_FRAME_BYTES, "{} bytes", text.len());
                frames.push(serde_json::from_str(text).unwrap());
                true
            },
            Trigger::Change,
        );
        assert_eq!(
            sent,
            Sent::Through {
                cursor: "1200".into(),
                wrote: true
            }
        );
        assert!(frames.len() > 3, "{} frames", frames.len());
        let mut next = 0;
        for frame in &frames {
            let signals = frame["data"]["signals"].as_array().unwrap();
            for s in signals {
                assert_eq!(s["n"], json!(next));
                next += 1;
            }
            assert_eq!(frame["data"]["cursor"], json!(next.to_string()));
        }
        assert_eq!(next, 1200);
    }

    /// A signal larger than one frame cannot be sent, and the client is told to start again
    /// rather than the connection being closed by a refused write; more than the page bound is
    /// the same; a write that fails is a gone client; nothing to send writes nothing.
    #[test]
    fn what_cannot_be_sent_is_a_resync_and_a_failed_write_is_gone() {
        let huge = send_pages("0", log(1, ws::MAX_FRAME_BYTES), |_| true, Trigger::Change);
        assert!(matches!(huge, Sent::Unreadable(r) if r.contains("larger than one frame")));
        let long = send_pages("0", log(10_000, 10), |_| true, Trigger::Change);
        assert!(matches!(long, Sent::Unreadable(r) if r.contains("pages")));
        let gone = send_pages("0", log(3, 10), |_| false, Trigger::Change);
        assert_eq!(gone, Sent::Gone);
        let quiet = send_pages(
            "5",
            log(5, 10),
            |_| panic!("nothing to write"),
            Trigger::Refresh,
        );
        assert_eq!(
            quiet,
            Sent::Through {
                cursor: "5".into(),
                wrote: false
            }
        );
    }

    /// A snapshot frame carries the connection's cursor, the withheld reference included, so
    /// a client that received no signals still has a point to resume from.
    #[test]
    fn a_snapshot_frame_carries_the_cursor() {
        let frame = Frame::Snapshot {
            trigger: Trigger::Connect,
            blackboard: &json!({ "workers": [] }),
            cursor: "m1.7",
        };
        let v = serde_json::to_value(&frame).unwrap();
        assert_eq!(v["type"], "blackboard.snapshot");
        assert_eq!(v["data"]["cursor"], "m1.7");
    }
}
