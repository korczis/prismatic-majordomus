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
//! GET /events?follow=blackboard&frames=signals&for=1b          the signals alone, marked for 1b
//! GET /events?follow=blackboard&sources=mesh,executor,github   the signals of these monitor
//!                                                              sources (mesh and executor when
//!                                                              not given)
//! ```
//!
//! `frames` names what the connection is written, `snapshot`, `signals` or both, so that a page
//! that shows only the stream (`/cockpit/monitor`, I2982) costs no blackboard read. `for` names
//! the reader, as `monitor.stream` takes it: its signals are marked `for_you`, and a page that
//! rendered its first rows for a reader keeps marking them the same way.
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
//!   signal. A connection that follows the signals alone is sent an empty one after a
//!   resync, carrying the cursor it starts again from.
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
//! every connection. One page of signals per cursor, sources and reader is shared the same
//! way. A hundred open
//! tabs cost one computation per change, not a hundred.
//!
//! **Slow clients.** A write blocks only its own connection's thread, and nothing is queued
//! for it. A slow client skips the snapshots it was too slow to receive, is told so
//! (`stream.lagged`), and is written the newest one, so its memory is bounded by one frame.
//! Whether the client hung up, and the heartbeat, are asked on every round, a change or not.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::capability::Context;
use crate::change::{Generation, BLACKBOARD};

use super::events::{Scope, StreamMessage, HEARTBEAT, LOOK};
use super::ws;

/// How long a follower waits for a change before it reads the blackboard anyway.
pub(crate) const REFRESH: Duration = Duration::from_secs(30);

/// How long a change is left to settle before the blackboard is read: a burst of ingested
/// events is read once.
pub(crate) const SETTLE: Duration = Duration::from_millis(150);

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
pub(crate) enum Trigger {
    /// The connection opened.
    Connect,
    /// The board or the journal changed and said so.
    Change,
    /// Nothing said so, and [`REFRESH`] passed.
    Refresh,
}

impl Trigger {
    /// What a round of a connection answers: a change the board or the journal said, or
    /// else the refresh when it is due, or nothing.
    fn of(changed: bool, refresh_due: bool) -> Option<Trigger> {
        match (changed, refresh_due) {
            (true, _) => Some(Trigger::Change),
            (false, true) => Some(Trigger::Refresh),
            (false, false) => None,
        }
    }
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
pub(crate) struct Follow {
    /// `repository` or `this`, as the capability takes it.
    pub(crate) checkouts: Option<String>,
    /// The cursor the client last read.
    pub(crate) after: Option<String>,
    /// The epoch of the server that issued it.
    pub(crate) epoch: Option<String>,
    /// The reader whose signals are marked, as `monitor.stream` takes `for`.
    pub(crate) reader: Option<String>,
    /// The monitor sources the signals come from, as `monitor.stream` takes them.
    pub(crate) sources: Option<String>,
    /// What the connection is written.
    pub(crate) frames: Frames,
}

/// What a connection is written: the snapshot, the signals, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Frames {
    /// `blackboard.snapshot` frames.
    pub(crate) snapshot: bool,
    /// `blackboard.signals` frames.
    pub(crate) signals: bool,
}

impl Default for Frames {
    fn default() -> Self {
        Self {
            snapshot: true,
            signals: true,
        }
    }
}

/// The monitor sources a connection reads when it names none: the two that say when they
/// change. GitHub is read only when asked, and only on a change or a refresh.
const DEFAULT_SOURCES: &str = "mesh,executor";

impl Follow {
    /// The query of a request that follows the blackboard: `None` when it follows something
    /// else, a refusal when it names what this channel does not take.
    pub(crate) fn parse(query: &[(String, String)]) -> Option<Result<Follow, String>> {
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
                "for" if !value.trim().is_empty() => follow.reader = Some(value.clone()),
                "sources"
                    if !value.is_empty()
                        && value
                            .split(',')
                            .all(|s| matches!(s.trim(), "mesh" | "executor" | "github")) =>
                {
                    follow.sources = Some(value.clone())
                }
                "frames" => match frames(value) {
                    Some(f) => follow.frames = f,
                    None => {
                        return Some(Err(format!(
                            "'frames={value}' names no frames this channel writes: snapshot, signals, or both"
                        )))
                    }
                },
                other => {
                    return Some(Err(format!(
                        "'{other}={value}' is not something the blackboard is followed by; it takes follow=blackboard, checkouts (this or repository), after, epoch, for, sources (mesh, executor, github) and frames (snapshot, signals)"
                    )))
                }
            }
        }
        Some(Ok(follow))
    }
}

/// The frames a comma list names, or `None` when it names nothing or something else.
fn frames(list: &str) -> Option<Frames> {
    let mut f = Frames {
        snapshot: false,
        signals: false,
    };
    for name in list.split(',') {
        match name.trim() {
            "snapshot" => f.snapshot = true,
            "signals" => f.signals = true,
            _ => return None,
        }
    }
    Some(f)
}

/// What this server process's cursors are good for: its mesh stream, which names the
/// process instance, or the process itself when the mesh is not running.
pub(crate) fn epoch(ctx: &Context) -> String {
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
    let mut held = slot.lock().unwrap_or_else(PoisonError::into_inner);
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
pub(crate) struct Follower {
    ctx: Arc<Context>,
    follow: Follow,
    /// The route of `blackboard.snapshot`, which a withheld snapshot names.
    route: String,
    /// How long a quiet connection waits for its heartbeat ([`HEARTBEAT`]).
    heartbeat: Duration,
    /// How long it waits for a change before it reads anyway ([`REFRESH`]).
    refresh: Duration,
    /// How many signals a page asks for, and how many pages a connection is written before
    /// it is told to resync ([`PAGE_LIMIT`], [`MAX_PAGES`]).
    pages: (u64, usize),
    /// The generation a change advances: the blackboard's ([`BLACKBOARD`]).
    changes: &'static Generation,
}

impl Follower {
    /// A follower of `follow`, reading through `ctx`'s registry.
    pub(crate) fn new(ctx: Arc<Context>, follow: Follow) -> Self {
        let route = ctx
            .registry
            .get(SNAPSHOT)
            .and_then(|c| c.exposure.http.as_ref())
            .map(|h| h.path.clone())
            .unwrap_or_default();
        Self {
            ctx,
            follow,
            route,
            heartbeat: HEARTBEAT,
            refresh: REFRESH,
            pages: (PAGE_LIMIT, MAX_PAGES),
            changes: &BLACKBOARD,
        }
    }

    fn snapshot(&self, generation: u64) -> Result<Arc<Value>, String> {
        // reader-neutral: what is for whom is the client's to mark, so that one read serves
        // every connection of this server; another server's board is another read (I4241)
        let checkouts = self.follow.checkouts.as_deref().unwrap_or("repository");
        let key = format!("{}|{checkouts}", epoch(&self.ctx));
        shared(&BOARD, &key, generation, || {
            self.ctx
                .execute(SNAPSHOT, json!({ "checkouts": checkouts, "signals": 0 }))
                .map_err(|e| e.to_string())
        })
    }

    /// The input of `monitor.stream` for this connection, with `rest` added.
    fn monitor_input(&self, mut rest: Value) -> Value {
        rest["sources"] = json!(self.follow.sources.as_deref().unwrap_or(DEFAULT_SOURCES));
        if let Some(reader) = &self.follow.reader {
            rest["for"] = json!(reader);
        }
        rest
    }

    /// What a shared page of signals is keyed by besides its cursor: the same cursor read for
    /// another reader, from other sources or of another server's stream is another page.
    fn page_key(&self, cursor: &str) -> String {
        format!(
            "{}|{}|{}|{cursor}",
            epoch(&self.ctx),
            self.follow.sources.as_deref().unwrap_or(DEFAULT_SOURCES),
            self.follow.reader.as_deref().unwrap_or_default()
        )
    }

    /// Where the signals stand now: the cursor a fresh connection starts from.
    fn now(&self, generation: u64) -> Result<String, String> {
        // keyed by what is asked, so that the latest signal is never the page after an
        // empty cursor
        let page = shared(
            &PAGE,
            &format!("latest|{}", self.page_key("")),
            generation,
            || {
                self.ctx
                    .execute(SIGNALS, self.monitor_input(json!({ "limit": 1 })))
                    .map_err(|e| e.to_string())
            },
        )?;
        Ok(page["cursor"].as_str().unwrap_or_default().to_string())
    }

    /// One page of the signals after `after`, at most `limit` of them.
    fn page(&self, after: &str, limit: u64, generation: u64) -> Result<Value, String> {
        let key = format!("after|{limit}|{}", self.page_key(after));
        let page = shared(&PAGE, &key, generation, || {
            self.ctx
                .execute(
                    SIGNALS,
                    self.monitor_input(json!({ "after": after, "limit": limit })),
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
            self.pages,
        )
    }

    /// An empty page of signals that carries the connection's cursor: what a connection that
    /// follows the signals alone is told after a resync, so that it resumes from there rather
    /// than from wherever the next connection finds the stream (I2982 review).
    fn write_cursor(
        &self,
        socket: &mut Box<dyn std::io::Write + Send>,
        trigger: Trigger,
        cursor: &str,
    ) -> bool {
        write(
            socket,
            &Frame::Signals {
                trigger,
                signals: &Value::Array(Vec::new()),
                cursor,
            },
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
        let text = serde_json::to_string(&frame).ok();
        if let Some(text) = text.as_deref().filter(|t| t.len() <= ws::MAX_FRAME_BYTES) {
            return frame_ok(socket, text);
        }
        let checkouts = self.follow.checkouts.as_deref().unwrap_or("repository");
        let withheld = json!({
            "withheld": "too_large",
            "bytes": text.map_or(0, |t| t.len()),
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
    pub(crate) fn run(
        self,
        mut socket: Box<dyn std::io::Write + Send>,
        hung_up: &dyn Fn() -> bool,
    ) {
        let epoch = epoch(&self.ctx);
        let mut generation = self.changes.current();
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
        let frames = self.follow.frames;
        let first = if frames.snapshot {
            match self.snapshot(generation) {
                Ok(s) => Some(s),
                Err(reason) => {
                    let _ = write(&mut socket, &StreamMessage::Closing { reason });
                    let _ = ws::write_close(&mut socket, 1011, "the blackboard could not be read");
                    return;
                }
            }
        } else {
            None
        };
        // the snapshot carries no signals: they travel by the connection's own cursor
        let mut cursor = if frames.signals {
            match self.now(generation) {
                Ok(c) => c,
                Err(reason) => {
                    let _ = write(&mut socket, &StreamMessage::Closing { reason });
                    let _ = ws::write_close(&mut socket, 1011, "the signals could not be read");
                    return;
                }
            }
        } else {
            String::new()
        };
        // a resumed connection is told what happened after its cursor, when the cursor is
        // this process's; anything else starts again from now, and is told so
        let mut resync = None;
        if let Some(after) = self.follow.after.as_ref().filter(|_| frames.signals) {
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
            // with no snapshot to carry it, the cursor it starts again from comes in its own frame
            if !frames.snapshot && !self.write_cursor(&mut socket, Trigger::Connect, &cursor) {
                return;
            }
        }
        if let Some(first) = &first {
            if !self.write_snapshot(&mut socket, Trigger::Connect, first, &cursor) {
                return;
            }
        }
        let mut said = first.as_deref().map(substance);
        let mut quiet = Instant::now();
        let mut read_at = Instant::now();
        // how long the last round of writes took: a change that passes while this connection
        // is still being written to is not sent as its own snapshot, and the client is told
        let mut busy = Duration::ZERO;
        loop {
            let wait = self
                .heartbeat
                .saturating_sub(quiet.elapsed())
                .min(self.refresh.saturating_sub(read_at.elapsed()))
                .min(LOOK);
            let changed = self.changes.wait_past(generation, wait) > generation;
            let trigger = Trigger::of(changed, read_at.elapsed() >= self.refresh);
            if changed {
                std::thread::sleep(SETTLE);
            }
            if let Some(trigger) = trigger {
                let latest = self.changes.current();
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
                // a snapshot that cannot be read now is read again on the next round
                let fresh = if frames.snapshot {
                    self.snapshot(generation).ok()
                } else {
                    None
                };
                if let Some((snapshot, substance)) = fresh
                    .map(|s| {
                        let substance = substance(&s);
                        (s, substance)
                    })
                    .filter(|(_, substance)| said.as_ref() != Some(substance))
                {
                    if !self.write_snapshot(&mut socket, trigger, &snapshot, &cursor) {
                        break;
                    }
                    said = Some(substance);
                    quiet = Instant::now();
                }
                if frames.signals {
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
                            // the client starts again from now, and is told where that is: in
                            // the snapshot when it follows one, in its own frame when not
                            cursor = self.now(generation).unwrap_or(cursor);
                            if !write(&mut socket, &StreamMessage::Resync { reason }) {
                                break;
                            }
                            // a snapshot that cannot be read leaves the cursor its own frame
                            let resent = if frames.snapshot {
                                self.snapshot(generation).ok()
                            } else {
                                None
                            };
                            match resent {
                                Some(snapshot) => {
                                    if !self.write_snapshot(
                                        &mut socket,
                                        trigger,
                                        &snapshot,
                                        &cursor,
                                    ) {
                                        break;
                                    }
                                    said = Some(substance(&snapshot));
                                }
                                None => {
                                    if !self.write_cursor(&mut socket, trigger, &cursor) {
                                        break;
                                    }
                                }
                            }
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
            if quiet.elapsed() >= self.heartbeat {
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
/// single signal larger than a frame, or more than `max_pages` pages of `page_limit` signals,
/// is a resync.
fn send_pages(
    after: &str,
    mut fetch: impl FnMut(&str, u64) -> Result<Value, String>,
    mut emit: impl FnMut(&str) -> bool,
    trigger: Trigger,
    (page_limit, max_pages): (u64, usize),
) -> Sent {
    let mut cursor = after.to_string();
    let mut limit = page_limit;
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
            let text = serde_json::to_string(&frame).unwrap_or_default();
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
        limit = page_limit;
        pages += 1;
        if !more {
            return Sent::Through { cursor, wrote };
        }
        if pages >= max_pages {
            return Sent::Unreadable(format!(
                "more than {max_pages} pages of signals happened after the cursor; read the blackboard again"
            ));
        }
    }
}

fn write<T: Serialize>(socket: &mut Box<dyn std::io::Write + Send>, value: &T) -> bool {
    // what is written here is this module's own frames, which always encode
    serde_json::to_string(value).map_or(true, |text| frame_ok(socket, &text))
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

    /// A page that shows only the stream asks for the signals alone, for its reader and from
    /// its sources; a frame or a source the channel does not know is refused, not ignored.
    #[test]
    fn frames_sources_and_the_reader_are_read() {
        let f = Follow::parse(&q(&[
            ("follow", "blackboard"),
            ("frames", "signals"),
            ("for", "1b,cd"),
            ("sources", "mesh,executor,github"),
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(
            f.frames,
            Frames {
                snapshot: false,
                signals: true
            }
        );
        assert_eq!(f.reader.as_deref(), Some("1b,cd"));
        assert_eq!(f.sources.as_deref(), Some("mesh,executor,github"));
        let both = Follow::parse(&q(&[("follow", "blackboard")]))
            .unwrap()
            .unwrap();
        assert_eq!(both.frames, Frames::default(), "both, when not asked");
        for (key, value) in [
            ("frames", "everything"),
            ("frames", ""),
            ("sources", "dots"),
        ] {
            let refused = Follow::parse(&q(&[("follow", "blackboard"), (key, value)]))
                .unwrap()
                .unwrap_err();
            assert!(refused.contains(value), "{refused}");
        }
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
            (PAGE_LIMIT, MAX_PAGES),
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
        let huge = send_pages(
            "0",
            log(1, ws::MAX_FRAME_BYTES),
            |_| true,
            Trigger::Change,
            (PAGE_LIMIT, MAX_PAGES),
        );
        assert!(matches!(huge, Sent::Unreadable(r) if r.contains("larger than one frame")));
        let long = send_pages(
            "0",
            log(10_000, 10),
            |_| true,
            Trigger::Change,
            (PAGE_LIMIT, MAX_PAGES),
        );
        assert!(matches!(long, Sent::Unreadable(r) if r.contains("pages")));
        let gone = send_pages(
            "0",
            log(3, 10),
            |_| false,
            Trigger::Change,
            (PAGE_LIMIT, MAX_PAGES),
        );
        assert_eq!(gone, Sent::Gone);
        let quiet = send_pages(
            "5",
            log(5, 10),
            |_| panic!("nothing to write"),
            Trigger::Refresh,
            (PAGE_LIMIT, MAX_PAGES),
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

    // ---------------------------------------------------------------- a follower on a socket

    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use crate::monitor::testing::{context, cooperation, session};

    /// A socket that keeps what it is written, refuses every write after `fails_after` of
    /// them, and takes `slow` over each.
    #[derive(Clone)]
    struct Sink {
        written: Arc<Mutex<Vec<u8>>>,
        writes: Arc<AtomicUsize>,
        fails_after: usize,
        slow: Duration,
        /// A write of a frame that holds this text fails, and so does every write after it
        /// when `after` is set, the frame itself then written.
        refuse: Option<(&'static str, bool)>,
        refused: Arc<AtomicBool>,
    }

    impl Sink {
        fn new() -> Self {
            Self::failing_after(usize::MAX)
        }

        fn failing_after(fails_after: usize) -> Self {
            Self {
                written: Arc::default(),
                writes: Arc::default(),
                fails_after,
                slow: Duration::ZERO,
                refuse: None,
                refused: Arc::default(),
            }
        }

        /// A sink that refuses the frame holding `text`, or, when `after`, every frame after it.
        fn refusing(text: &'static str, after: bool) -> Self {
            Self {
                refuse: Some((text, after)),
                ..Self::new()
            }
        }

        /// Every frame written so far: a text frame as its JSON, a ping and a close by name.
        fn frames(&self) -> Vec<Value> {
            let bytes = self.written.lock().unwrap().clone();
            let mut out = Vec::new();
            let mut i = 0;
            while i + 2 <= bytes.len() {
                let opcode = bytes[i] & 0x0f;
                let (len, head) = match bytes[i + 1] & 0x7f {
                    126 => (u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize, 4),
                    127 => {
                        let mut n = [0u8; 8];
                        n.copy_from_slice(&bytes[i + 2..i + 10]);
                        (u64::from_be_bytes(n) as usize, 10)
                    }
                    n => (n as usize, 2),
                };
                let payload = &bytes[i + head..i + head + len];
                out.push(match opcode {
                    0x1 => serde_json::from_slice(payload).unwrap(),
                    0x9 => json!({ "type": "ping" }),
                    _ => json!({ "type": "close" }),
                });
                i += head + len;
            }
            out
        }

        /// The frames once `done` holds of them, within twenty seconds.
        fn until(&self, done: impl Fn(&[Value]) -> bool) -> Vec<Value> {
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                let frames = self.frames();
                if done(&frames) {
                    return frames;
                }
                assert!(Instant::now() < deadline, "never came: {frames:#?}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    impl std::io::Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.writes.fetch_add(1, Ordering::SeqCst) >= self.fails_after
                || self.refused.load(Ordering::SeqCst)
            {
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }
            if let Some((text, after)) = self.refuse {
                if String::from_utf8_lossy(buf).contains(text) {
                    self.refused.store(true, Ordering::SeqCst);
                    if !after {
                        return Err(std::io::ErrorKind::BrokenPipe.into());
                    }
                }
            }
            std::thread::sleep(self.slow);
            self.written.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn kinds(frames: &[Value]) -> Vec<String> {
        frames
            .iter()
            .map(|f| f["type"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    fn has(frames: &[Value], kind: &str, trigger: &str) -> bool {
        frames
            .iter()
            .any(|f| f["type"] == kind && (trigger.is_empty() || f["data"]["trigger"] == trigger))
    }

    /// A follower of `query`, tuned by `tune`, served on `sink` on a thread of its own until
    /// the returned flag says the client hung up.
    fn serve(
        ctx: &Arc<Context>,
        query: &[(&str, &str)],
        tune: impl FnOnce(&mut Follower),
        sink: &Sink,
    ) -> (std::thread::JoinHandle<()>, Arc<AtomicBool>) {
        let mut pairs = vec![("follow", "blackboard")];
        pairs.extend_from_slice(query);
        let follow = Follow::parse(&q(&pairs)).unwrap().unwrap();
        let mut follower = Follower::new(Arc::clone(ctx), follow);
        tune(&mut follower);
        let gone = Arc::new(AtomicBool::new(false));
        let socket = sink.clone();
        let flag = Arc::clone(&gone);
        let thread = std::thread::spawn(move || {
            follower.run(Box::new(socket), &move || flag.load(Ordering::SeqCst))
        });
        (thread, gone)
    }

    fn hang_up((thread, gone): (std::thread::JoinHandle<()>, Arc<AtomicBool>)) {
        gone.store(true, Ordering::SeqCst);
        BLACKBOARD.advance();
        thread.join().unwrap();
    }

    /// A context over a synthetic repository with a mesh running in it.
    fn meshed() -> (
        crate::synthetic::SyntheticRepository,
        Arc<Context>,
        Arc<crate::mesh::cooperation::Cooperation>,
    ) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = Arc::new(context(&repo));
        let c = cooperation();
        crate::mesh::testing::attach(&ctx, Arc::clone(&c));
        (repo, ctx, c)
    }

    /// Two servers in one process, each read at the same generation, are each read their own
    /// board: a shared read is never handed to a follower of another context (I4241).
    #[test]
    fn a_shared_read_is_never_handed_to_another_server() {
        let (_first_repo, first, _first_mesh) = meshed();
        let (_second_repo, second, second_mesh) = meshed();
        session(&second_mesh, "s1", "3d", "only the second server says this");
        let follower = |ctx: &Arc<Context>| {
            let follow = Follow::parse(&q(&[("follow", "blackboard")]))
                .unwrap()
                .unwrap();
            Follower::new(Arc::clone(ctx), follow)
        };
        let generation = BLACKBOARD.current();
        let board = |ctx: &Arc<Context>| follower(ctx).snapshot(generation).unwrap().to_string();
        assert!(!board(&first).contains("only the second server says this"));
        assert!(board(&second).contains("only the second server says this"));
        assert!(!board(&first).contains("only the second server says this"));
    }

    /// A follower is written the ready frame with its epoch, then the snapshot with the
    /// connection's cursor; a change that says something new is a snapshot and signals of
    /// its own; a quiet connection is pinged and told it is alive, and is read again when the
    /// refresh passes; a client that hung up ends the connection.
    #[test]
    fn a_follower_is_written_its_snapshot_each_change_and_a_heartbeat() {
        let (_repo, ctx, c) = meshed();
        let sink = Sink::new();
        let served = serve(
            &ctx,
            &[],
            |f| {
                f.heartbeat = Duration::from_millis(200);
                f.refresh = Duration::from_millis(300);
            },
            &sink,
        );
        let frames = sink.until(|f| has(f, "blackboard.snapshot", "connect"));
        assert_eq!(frames[0]["type"], "stream.ready");
        assert_eq!(frames[0]["data"]["epoch"], json!(epoch(&ctx)));
        assert!(frames[1]["data"]["cursor"].is_string(), "{frames:#?}");

        session(&c, "s1", "3d", "the runtime train");
        BLACKBOARD.advance();
        sink.until(|f| {
            f.iter().any(|f| {
                f["type"] == "blackboard.snapshot"
                    && f["data"]["trigger"] == "change"
                    && f["data"]["blackboard"]
                        .to_string()
                        .contains("the runtime train")
            })
        });
        // a change that says nothing new is answered too: it is read, and its frame is
        // written only when what it says moved
        BLACKBOARD.advance();
        let frames = sink.until(|f| has(f, "stream.heartbeat", "") && has(f, "ping", ""));
        assert!(
            has(&frames, "blackboard.signals", ""),
            "{:?}",
            kinds(&frames)
        );
        hang_up(served);
    }

    /// A connection that resumes on this process is written what happened after its cursor;
    /// one whose cursor another process issued, or that cannot be read from, is told to resync
    /// and where it starts again: in the snapshot, or in a frame of its own when it follows the
    /// signals alone.
    #[test]
    fn a_resumed_follower_is_written_what_it_missed_or_told_where_to_start() {
        let (_repo, ctx, c) = meshed();
        session(&c, "s1", "3d", "the runtime train");
        let epoch = epoch(&ctx);

        let sink = Sink::new();
        let served = serve(
            &ctx,
            &[
                ("frames", "signals"),
                ("after", ""),
                ("epoch", &epoch),
                ("for", "3d"),
            ],
            |_| {},
            &sink,
        );
        sink.until(|f| has(f, "blackboard.signals", "connect"));
        hang_up(served);

        let sink = Sink::new();
        let served = serve(
            &ctx,
            &[("frames", "signals"), ("after", "x"), ("epoch", "another")],
            |_| {},
            &sink,
        );
        let frames = sink.until(|f| has(f, "blackboard.signals", "connect"));
        assert_eq!(
            kinds(&frames)[..3],
            ["stream.ready", "stream.resync", "blackboard.signals"]
        );
        assert_eq!(frames[2]["data"]["signals"], json!([]));
        assert!(frames[2]["data"]["cursor"].is_string());
        hang_up(served);

        let sink = Sink::new();
        let served = serve(
            &ctx,
            &[("after", "not a cursor"), ("epoch", &epoch)],
            |_| {},
            &sink,
        );
        let frames = sink.until(|f| has(f, "blackboard.snapshot", "connect"));
        assert_eq!(
            kinds(&frames)[..3],
            ["stream.ready", "stream.resync", "blackboard.snapshot"]
        );
        hang_up(served);
    }

    /// More signals than a connection is written in a round is a resync, and the connection
    /// starts again from now: told in the snapshot, or in a frame of its own when it follows
    /// the signals alone.
    #[test]
    fn a_backlog_beyond_the_bound_in_the_stream_starts_the_client_again() {
        let (_repo, ctx, c) = meshed();
        for (tag, frames) in [("both", "snapshot,signals"), ("alone", "signals")] {
            let sink = Sink::new();
            let served = serve(
                &ctx,
                &[("frames", frames)],
                |f| {
                    f.pages = (1, 1);
                    f.heartbeat = Duration::from_millis(50);
                },
                &sink,
            );
            // the connection's cursor is taken after its ready frame, and a heartbeat is only
            // written by the loop that follows: once one came, the cursor stands before the
            // backlog, however long the follower took to read where the signals stood
            sink.until(|f| has(f, "stream.heartbeat", ""));
            // every append is a change of its own, and every read of the signals waits on the
            // shared page's lock: a backlog written under it is read whole by one round, never
            // split one signal per wake-up
            {
                let _held = PAGE.lock().unwrap_or_else(PoisonError::into_inner);
                for n in 0..3 {
                    session(&c, &format!("{tag}-{n}"), "3d", "one more");
                }
            }
            BLACKBOARD.advance();
            let after = |f: &[Value]| kinds(f).iter().position(|k| k == "stream.resync");
            let frames = sink.until(|f| after(f).is_some_and(|at| f.len() > at + 1));
            let at = after(&frames).unwrap();
            assert!(frames[at + 1]["data"]["cursor"].is_string());
            hang_up(served);
        }
    }

    /// A client whose writes take longer than a change takes to settle is told it lagged, and
    /// is written the newest snapshot rather than every one it missed.
    #[test]
    fn a_slow_client_is_told_it_lagged() {
        let (_repo, ctx, c) = meshed();
        let mut sink = Sink::new();
        sink.slow = SETTLE * 2;
        let served = serve(&ctx, &[("frames", "snapshot")], |_| {}, &sink);
        sink.until(|f| has(f, "blackboard.snapshot", "connect"));
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut n = 0;
        while !has(&sink.frames(), "stream.lagged", "") {
            assert!(
                Instant::now() < deadline,
                "never told: {:?}",
                kinds(&sink.frames())
            );
            session(&c, &format!("s{n}"), "3d", &format!("change {n}"));
            BLACKBOARD.advance();
            n += 1;
            std::thread::sleep(SETTLE / 2);
        }
        hang_up(served);
    }

    /// What cannot be read closes the connection with the reason, and a client that cannot be
    /// written to ends it, at every write a connection makes.
    #[test]
    fn a_follower_that_cannot_read_or_write_ends() {
        let (_repo, ctx, c) = meshed();
        let run = |follow: Follow, sink: &Sink, tune: &dyn Fn(&mut Follower)| {
            let mut f = Follower::new(Arc::clone(&ctx), follow);
            tune(&mut f);
            f.run(Box::new(sink.clone()), &|| false);
        };
        let parsed = |pairs: &[(&str, &str)]| {
            let mut all = vec![("follow", "blackboard")];
            all.extend_from_slice(pairs);
            Follow::parse(&q(&all)).unwrap().unwrap()
        };
        let none = |_: &mut Follower| {};

        // a board that cannot be read, and signals that cannot be read
        let mut unreadable = parsed(&[]);
        unreadable.checkouts = Some("nowhere".into());
        let sink = Sink::new();
        run(unreadable, &sink, &none);
        assert_eq!(
            kinds(&sink.frames()),
            ["stream.ready", "stream.closing", "close"]
        );
        let mut unreadable = parsed(&[("frames", "signals")]);
        unreadable.sources = Some("nowhere".into());
        let sink = Sink::new();
        run(unreadable, &sink, &none);
        assert_eq!(
            kinds(&sink.frames()),
            ["stream.ready", "stream.closing", "close"]
        );

        // every write of the connection's start
        let epoch = epoch(&ctx);
        session(&c, "s1", "3d", "the runtime train");
        for (pairs, writes) in [
            (vec![], 2),
            (
                vec![("frames", "signals"), ("after", "x"), ("epoch", "another")],
                3,
            ),
            (vec![("after", "x"), ("epoch", "another")], 3),
            (
                vec![
                    ("frames", "signals"),
                    ("after", ""),
                    ("epoch", epoch.as_str()),
                ],
                2,
            ),
        ] {
            for fails_after in 0..writes {
                let sink = Sink::failing_after(fails_after);
                run(parsed(&pairs), &sink, &none);
                assert!(
                    sink.writes.load(Ordering::SeqCst) > fails_after,
                    "{pairs:?}"
                );
            }
        }

        // a write in the stream, counted from the ready frame: a snapshot, the signals, a
        // resync and what follows it; changes come two sessions at a time, so that a page of
        // one signal always has more after it
        let pages = |f: &mut Follower| f.pages = (1, 1);
        type Tune<'a> = &'a dyn Fn(&mut Follower);
        let streams: [(&str, Tune, usize); 6] = [
            ("snapshot", &none, 2),
            ("signals", &none, 1),
            ("signals", &pages, 1),
            ("signals", &pages, 2),
            ("snapshot,signals", &pages, 3),
            ("snapshot,signals", &pages, 4),
        ];
        for (n, (frames, tune, fails_after)) in streams.into_iter().enumerate() {
            let done = Arc::new(AtomicBool::new(false));
            let writer = {
                let (c, done) = (Arc::clone(&c), Arc::clone(&done));
                std::thread::spawn(move || {
                    let mut k = 0;
                    while !done.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(100));
                        for _ in 0..2 {
                            session(&c, &format!("w{n}-{k}"), "3d", &format!("change {n} {k}"));
                            k += 1;
                        }
                        BLACKBOARD.advance();
                    }
                })
            };
            let sink = Sink::failing_after(fails_after);
            run(parsed(&[("frames", frames)]), &sink, tune);
            done.store(true, Ordering::SeqCst);
            writer.join().unwrap();
        }
        let sink = Sink::failing_after(2);
        run(parsed(&[("frames", "snapshot")]), &sink, &|f| {
            f.heartbeat = Duration::from_millis(50)
        });
        let sink = Sink::failing_after(3);
        run(parsed(&[("frames", "snapshot")]), &sink, &|f| {
            f.heartbeat = Duration::from_millis(50)
        });
    }

    /// A snapshot too large for one frame is written as a reference to where it is read, with
    /// the connection's cursor.
    #[test]
    fn a_snapshot_too_large_for_a_frame_is_withheld() {
        let (_repo, ctx, _c) = meshed();
        let follower = Follower::new(
            Arc::clone(&ctx),
            Follow::parse(&q(&[("follow", "blackboard")]))
                .unwrap()
                .unwrap(),
        );
        let sink = Sink::new();
        let mut socket: Box<dyn std::io::Write + Send> = Box::new(sink.clone());
        let big = json!({ "workers": "x".repeat(ws::MAX_FRAME_BYTES) });
        assert!(follower.write_snapshot(&mut socket, Trigger::Change, &big, "m1.2"));
        let frames = sink.frames();
        assert_eq!(frames[0]["data"]["blackboard"]["withheld"], "too_large");
        assert!(
            frames[0]["data"]["blackboard"]["bytes"].as_u64().unwrap() > ws::MAX_FRAME_BYTES as u64
        );
        assert!(frames[0]["data"]["blackboard"]["self"]
            .as_str()
            .unwrap()
            .ends_with("?checkouts=repository"));
        assert_eq!(frames[0]["data"]["cursor"], "m1.2");
    }

    /// A shared read that fails is the failure, and is not kept; a frame list that names
    /// something else is no list; the substance of a snapshot drops what each worker's clock
    /// says.
    #[test]
    fn a_failed_read_is_not_kept_and_a_workers_clock_is_no_substance() {
        static SLOT: Mutex<Option<Shared>> = Mutex::new(None);
        assert_eq!(
            shared(&SLOT, "k", 1, || Err("no".into())).unwrap_err(),
            "no"
        );
        assert_eq!(*shared(&SLOT, "k", 1, || Ok(json!(1))).unwrap(), json!(1));
        assert_eq!(frames("snapshot,elsewhere"), None);
        let a =
            json!({ "workers": [{ "key": "w", "last_seen_seconds_ago": 1, "last_beat_ms": 5 }] });
        let b =
            json!({ "workers": [{ "key": "w", "last_seen_seconds_ago": 9, "last_beat_ms": 7 }] });
        assert_eq!(substance(&a), substance(&b));
    }

    /// A connection on which nothing changes is read again when the refresh passes.
    #[test]
    fn a_quiet_follower_is_read_again_when_the_refresh_passes() {
        let (_repo, ctx, _c) = meshed();
        let sink = Sink::new();
        let served = serve(&ctx, &[], |f| f.refresh = Duration::ZERO, &sink);
        sink.until(|f| has(f, "blackboard.snapshot", "connect"));
        std::thread::sleep(Duration::from_millis(100));
        hang_up(served);
    }

    /// A client that cannot be written the frame it is owed ends the connection there: the
    /// lagged notice, and what follows a resync, in the snapshot or in a frame of its own.
    #[test]
    fn a_refused_notice_ends_the_connection() {
        let (_repo, ctx, c) = meshed();
        let changes = |tag: &'static str, done: Arc<AtomicBool>| {
            let c = Arc::clone(&c);
            std::thread::spawn(move || {
                let mut k = 0;
                while !done.load(Ordering::SeqCst) {
                    for _ in 0..2 {
                        session(&c, &format!("{tag}-{k}"), "3d", &format!("{tag} {k}"));
                        k += 1;
                    }
                    BLACKBOARD.advance();
                    std::thread::sleep(SETTLE / 3);
                }
            })
        };
        let pages = |f: &mut Follower| f.pages = (1, 1);
        type Tune<'a> = &'a dyn Fn(&mut Follower);
        let cases: [(&str, &str, Sink, Tune); 3] = [
            (
                "lag",
                "snapshot",
                Sink::refusing("stream.lagged", false),
                &|_| {},
            ),
            (
                "both",
                "snapshot,signals",
                Sink::refusing("stream.resync", true),
                &pages,
            ),
            (
                "alone",
                "signals",
                Sink::refusing("stream.resync", true),
                &pages,
            ),
        ];
        for (tag, frames, mut sink, tune) in cases {
            if tag == "lag" {
                sink.slow = SETTLE * 2;
            }
            let done = Arc::new(AtomicBool::new(false));
            let writer = changes(tag, Arc::clone(&done));
            let follow = Follow::parse(&q(&[("follow", "blackboard"), ("frames", frames)]))
                .unwrap()
                .unwrap();
            let mut f = Follower::new(Arc::clone(&ctx), follow);
            tune(&mut f);
            f.run(Box::new(sink.clone()), &|| false);
            done.store(true, Ordering::SeqCst);
            writer.join().unwrap();
            assert!(sink.refused.load(Ordering::SeqCst), "{tag}");
        }
    }

    /// What is not an object, and a worker that is not one, is its own substance.
    #[test]
    fn what_is_not_an_object_is_its_own_substance() {
        assert_eq!(substance(&json!(1)), json!(1));
        assert_eq!(substance(&json!({ "at": "t", "x": 1 })), json!({ "x": 1 }));
        assert_eq!(
            substance(&json!({ "workers": [1, { "last_beat_ms": 2 }] })),
            json!({ "workers": [1, {}] })
        );
    }

    /// A change wins over a due refresh, a due refresh is a round, and nothing is none.
    #[test]
    fn a_round_answers_a_change_before_a_refresh() {
        assert_eq!(Trigger::of(true, true), Some(Trigger::Change));
        assert_eq!(Trigger::of(true, false), Some(Trigger::Change));
        assert_eq!(Trigger::of(false, true), Some(Trigger::Refresh));
        assert_eq!(Trigger::of(false, false), None);
    }

    /// A round in which nothing changed and no refresh is due writes nothing and waits again.
    #[test]
    fn a_round_with_nothing_to_answer_writes_nothing() {
        static QUIET: Generation = Generation::new();
        let (_repo, ctx, _c) = meshed();
        let sink = Sink::new();
        let served = serve(
            &ctx,
            &[("frames", "snapshot")],
            |f| f.changes = &QUIET,
            &sink,
        );
        sink.until(|f| has(f, "blackboard.snapshot", "connect"));
        std::thread::sleep(LOOK + LOOK / 2);
        assert_eq!(
            kinds(&sink.frames()),
            ["stream.ready", "blackboard.snapshot"]
        );
        hang_up(served);
    }
}
