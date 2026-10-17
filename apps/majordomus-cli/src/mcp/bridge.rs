//! The bridge: a `majordomus mcp` process that found the repository's shared server
//! already running forwards its client's stdio frames to that server's `/mcp` endpoint
//! and writes the answers back, one HTTP request per message, and pings on the side so
//! that the server keeps the session. It carries no index and no registry of its own,
//! which is why it starts in milliseconds. The HTTP client below is the few lines a
//! loopback request needs (the server always answers with `Content-Length`, never
//! chunked); no HTTP library is pulled in for it.
//!
//! A message is sent a second time only when the server provably did not run it: the
//! connection was never made, or the server answered 404 (the session is unknown) or 409
//! (the lease is lost), both before it looks at the message. A message that was sent and
//! never answered — a timeout, a dropped connection, a 5xx — may have run, so it is sent
//! again only when it is idempotent: every method but `tools/call`, and a tool whose
//! declared execution hints say so ([`idempotent`]). Anything else is the caller's to
//! report as an outcome nobody knows ([`BridgeError::may_resend`]).

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::{json, Value};

use crate::peers::ClientInfo;

/// How often a bridge pings the server so that its session is not forgotten.
pub const HEARTBEAT: Duration = Duration::from_secs(20);

/// How long a bridge waits to connect.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// How long a bridge waits for one answer, unless it was made with [`Bridge::with_timeout`].
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

const SESSION_HEADER: &str = "Mcp-Session-Id";

/// One HTTP reply.
#[derive(Debug, Clone)]
pub struct Reply {
    /// The status code.
    pub status: u16,
    /// Header names lowercased.
    pub headers: Vec<(String, String)>,
    /// The body, as text.
    pub body: String,
}

impl Reply {
    /// A header value, by case-insensitive name.
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// The largest reply [`request`] reads. Far above what an answer of this server weighs — a
/// `tools/list` of every capability is a few megabytes — and finite, so that a peer, a
/// rendezvous hub or a broken proxy that never stops sending costs a bounded buffer (I2147).
pub const MAX_REPLY_BYTES: usize = 32 * 1024 * 1024;

/// One HTTP/1.1 request to `base_url` (`http://host:port`), with `Connection: close`.
pub fn request(
    base_url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<Reply> {
    let (stream, host) = connect(base_url, timeout)?;
    exchange(stream, &host, method, path, headers, body, timeout)
}

/// Open a connection to `base_url`. Nothing has been sent when this fails, which is what
/// lets a caller send the same message again elsewhere.
fn connect(base_url: &str, timeout: Duration) -> std::io::Result<(TcpStream, String)> {
    let host = base_url
        .strip_prefix("http://")
        .unwrap_or(base_url)
        .trim_end_matches('/');
    let addr = host
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other(format!("{host}: no address")))?;
    // The connection shares the caller's budget when that is the shorter: a probe on a shell
    // prompt that allows 250 ms must not wait 2 s on a server whose accept queue is full.
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT.min(timeout))?;
    Ok((stream, host.to_string()))
}

/// Send one request on an open connection and read the reply. From the first byte written
/// the server may have the request, so a failure here leaves its outcome unknown.
fn exchange(
    mut stream: TcpStream,
    host: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<Reply> {
    stream.set_read_timeout(Some(timeout))?;
    write_request(&mut stream, host, method, path, headers, body, timeout)?;
    let mut raw = Vec::new();
    (&mut stream)
        .take(MAX_REPLY_BYTES as u64 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > MAX_REPLY_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("the reply is over the {MAX_REPLY_BYTES}-byte bound and was not read"),
        ));
    }
    parse_reply(&raw)
}

/// Write one request: the head this client always sends, the caller's headers, the body.
fn write_request(
    stream: &mut TcpStream,
    host: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<()> {
    stream.set_write_timeout(Some(timeout))?;
    let body = body.unwrap_or("");
    let mut text = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        text.push_str(&format!("{k}: {v}\r\n"));
    }
    text.push_str("\r\n");
    text.push_str(body);
    stream.write_all(text.as_bytes())
}

/// The most bytes of a reply's head [`request_json_within`] reads before its body begins.
pub const MAX_HEAD_BYTES: usize = 16 * 1024;

/// What one reply read by [`request_json_within`] may cost: the bytes of it held, and the
/// time it may take, between two reads and in all.
///
/// The two times are different defences. `idle` is what [`request`] bounds: a peer that
/// goes quiet. `within` bounds a peer that never goes quiet — one byte every few seconds —
/// which a bound between reads alone lets hold its caller for as long as it likes.
///
/// ```
/// use std::time::Duration;
/// use majordomus_cli::mcp::bridge::ReplyBound;
///
/// let bound = ReplyBound {
///     bytes: 1024,
///     idle: Duration::from_secs(5),
///     within: Duration::from_secs(20),
/// };
/// assert!(bound.idle <= bound.within);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplyBound {
    /// The most bytes of the reply's JSON body held, without its insignificant whitespace.
    pub bytes: usize,
    /// The longest wait for the next bytes.
    pub idle: Duration,
    /// The longest wait for the whole reply, from the connection on.
    pub within: Duration,
}

/// Why [`request_json_within`] stopped reading a reply: the typed half of the
/// [`std::io::Error`] it returns, reached with [`ReplyRefused::of`]. Neither is a reply a
/// caller can act on; both say the peer sent more, or more slowly, than it may (I2704).
///
/// ```
/// use majordomus_cli::mcp::bridge::ReplyRefused;
///
/// let error = std::io::Error::new(
///     std::io::ErrorKind::InvalidData,
///     ReplyRefused::Oversized { bound: 10 },
/// );
/// assert_eq!(ReplyRefused::of(&error), Some(&ReplyRefused::Oversized { bound: 10 }));
/// assert!(error.to_string().contains("10-byte bound"), "{error}");
/// // an error of another cause is no refusal of the reply
/// assert_eq!(ReplyRefused::of(&std::io::Error::other("refused")), None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReplyRefused {
    /// The reply's head, or its body without insignificant whitespace, is over the bound.
    #[error("the reply is over the {bound}-byte bound and was not read")]
    Oversized {
        /// The bound it went over.
        bound: usize,
    },
    /// The whole reply did not arrive within the time allowed for it.
    #[error("the reply did not arrive whole within {millis} ms")]
    Late {
        /// The time allowed, in milliseconds.
        millis: u64,
    },
}

impl ReplyRefused {
    /// The refusal an error of [`request_json_within`] carries, when that was its cause.
    ///
    /// ```
    /// use majordomus_cli::mcp::bridge::ReplyRefused;
    ///
    /// let late = std::io::Error::new(
    ///     std::io::ErrorKind::TimedOut,
    ///     ReplyRefused::Late { millis: 250 },
    /// );
    /// assert_eq!(ReplyRefused::of(&late), Some(&ReplyRefused::Late { millis: 250 }));
    /// // a timeout of another cause carries no refusal
    /// let other = std::io::Error::from(std::io::ErrorKind::TimedOut);
    /// assert_eq!(ReplyRefused::of(&other), None);
    /// ```
    pub fn of(error: &std::io::Error) -> Option<&ReplyRefused> {
        error
            .get_ref()
            .and_then(|e| e.downcast_ref::<ReplyRefused>())
    }

    fn into_io(self) -> std::io::Error {
        let kind = match self {
            ReplyRefused::Oversized { .. } => std::io::ErrorKind::InvalidData,
            ReplyRefused::Late { .. } => std::io::ErrorKind::TimedOut,
        };
        std::io::Error::new(kind, self)
    }
}

/// One HTTP/1.1 request whose reply is a JSON document held at most at `bound.bytes` and
/// read whole within `bound.within`: what a dialer asks of a peer it does not control.
///
/// The body is held without the whitespace JSON allows between tokens, which makes the
/// bound the size of the document rather than of its formatting: the shared server
/// pretty-prints every answer, and a peer that pads one costs time — bounded by `within` —
/// never memory. The bound counts what is held, so the first reply refused is the one a
/// byte over it, however it was formatted. The head is bounded too, at [`MAX_HEAD_BYTES`].
/// A body that is not JSON loses its spaces: a caller that reads more than the status of
/// a non-JSON answer uses [`request`].
///
/// ```
/// use std::time::Duration;
/// use majordomus_cli::mcp::bridge::{request_json_within, ReplyBound, ReplyRefused};
///
/// let bound = ReplyBound {
///     bytes: 1024,
///     idle: Duration::from_secs(1),
///     within: Duration::from_secs(2),
/// };
/// // port 9 discards: nothing answers there, which is a failure to connect, not a refusal
/// let error = request_json_within("http://127.0.0.1:9", "GET", "/", &[], None, bound)
///     .unwrap_err();
/// assert_eq!(ReplyRefused::of(&error), None);
/// ```
pub fn request_json_within(
    base_url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    bound: ReplyBound,
) -> std::io::Result<Reply> {
    let deadline = std::time::Instant::now() + bound.within;
    let first = bound.idle.min(bound.within);
    let (mut stream, host) = connect(base_url, first)?;
    write_request(&mut stream, &host, method, path, headers, body, first)?;
    let late = || {
        ReplyRefused::Late {
            millis: u64::try_from(bound.within.as_millis()).unwrap_or(u64::MAX),
        }
        .into_io()
    };
    let mut head: Vec<u8> = Vec::new();
    let mut parsed: Option<ParsedHead> = None;
    let mut held: Vec<u8> = Vec::new();
    let mut json = Compact::default();
    let mut chunk = vec![0u8; 16 * 1024];
    loop {
        if matches!(parsed, Some(ParsedHead { due: Some(0), .. })) {
            break;
        }
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err(late());
        }
        // when what is left of the whole is the shorter wait, a timeout is the deadline's
        let deadline_wait = left <= bound.idle;
        stream.set_read_timeout(Some(left.min(bound.idle)))?;
        let n = match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e)
                if deadline_wait
                    && matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
            {
                return Err(late())
            }
            Err(e) => return Err(e),
        };
        let mut bytes = &chunk[..n];
        let rest;
        if parsed.is_none() {
            head.extend_from_slice(bytes);
            let Some(end) = head.windows(4).position(|w| w == b"\r\n\r\n") else {
                if head.len() > MAX_HEAD_BYTES {
                    return Err(ReplyRefused::Oversized {
                        bound: MAX_HEAD_BYTES,
                    }
                    .into_io());
                }
                continue;
            };
            rest = head.split_off(end + 4);
            head.truncate(end);
            let (status, headers) = parse_head(&String::from_utf8_lossy(&head))?;
            let due = headers
                .iter()
                .find(|(k, _)| k == "content-length")
                .and_then(|(_, v)| v.parse::<usize>().ok());
            parsed = Some(ParsedHead {
                status,
                headers,
                due,
            });
            bytes = &rest;
        }
        if let Some(ParsedHead { due: Some(due), .. }) = parsed.as_mut() {
            let take = bytes.len().min(*due);
            *due -= take;
            bytes = &bytes[..take];
        }
        json.feed(bytes, &mut held, bound.bytes)?;
    }
    let head =
        parsed.ok_or_else(|| std::io::Error::other("malformed HTTP response: no header end"))?;
    Ok(Reply {
        status: head.status,
        headers: head.headers,
        body: String::from_utf8_lossy(&held).into_owned(),
    })
}

/// A reply's head once read: its status, its headers, and the body bytes its
/// Content-Length says are still due, when it names one.
struct ParsedHead {
    status: u16,
    headers: Vec<(String, String)>,
    due: Option<usize>,
}

/// A JSON body held without its insignificant whitespace: every byte of a string is kept,
/// the spaces, tabs and line ends between tokens are dropped, and nothing past the bound
/// is held.
#[derive(Debug, Default)]
struct Compact {
    in_string: bool,
    escaped: bool,
}

impl Compact {
    fn feed(&mut self, bytes: &[u8], held: &mut Vec<u8>, bound: usize) -> std::io::Result<()> {
        for &b in bytes {
            if self.in_string {
                if self.escaped {
                    self.escaped = false;
                } else if b == b'\\' {
                    self.escaped = true;
                } else if b == b'"' {
                    self.in_string = false;
                }
            } else if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                continue;
            } else if b == b'"' {
                self.in_string = true;
            }
            if held.len() >= bound {
                return Err(ReplyRefused::Oversized { bound }.into_io());
            }
            held.push(b);
        }
        Ok(())
    }
}

/// The status and the lowercased headers of a reply's head.
fn parse_head(head: &str) -> std::io::Result<(u16, Vec<(String, String)>)> {
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| std::io::Error::other("malformed HTTP response: no status line"))?;
    let headers = lines
        .filter_map(|l| {
            l.split_once(':')
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect::<Vec<_>>();
    Ok((status, headers))
}

fn parse_reply(raw: &[u8]) -> std::io::Result<Reply> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| std::io::Error::other("malformed HTTP response: no header end"))?;
    let (status, headers) = parse_head(head)?;
    let length = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse::<usize>().ok());
    let body = match length {
        Some(n) if n <= body.len() => body[..n].to_string(),
        _ => body.to_string(),
    };
    Ok(Reply {
        status,
        headers,
        body,
    })
}

/// Why the bridge could not get an answer.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BridgeError {
    #[error("the shared server at {url} is unreachable: {reason}")]
    /// No connection could be made: nothing was sent, so nothing ran, and the server is gone.
    Unreachable {
        /// The server's URL.
        url: String,
        /// What happened.
        reason: String,
    },
    #[error("the shared server at {url} did not answer: {reason}")]
    /// The message was sent and no usable answer came back: a timeout, a dropped
    /// connection, a server-side failure (5xx), an answer that is not JSON. The server
    /// may have run it, and whether it did is unknown.
    Unanswered {
        /// The server's URL.
        url: String,
        /// What happened.
        reason: String,
    },
    #[error("the shared server at {url} rejected the request ({status}): {body}")]
    /// The server answered and refused.
    Rejected {
        /// The server's URL.
        url: String,
        /// The status code.
        status: u16,
        /// The body.
        body: String,
    },
}

impl BridgeError {
    /// Whether this failure says the server is no longer the one to talk to, so that the
    /// client should elect again: it could not be reached, it took the request and never
    /// answered, it forgot the session twice running (404), or it said it had lost the lease
    /// (409).
    ///
    /// Any other refusal comes from a server that is there and serving: it read the request
    /// and would not take it — a request before `initialize`, a body too large. An election
    /// cannot change that answer; it is relayed to the client.
    ///
    /// ```
    /// use majordomus_cli::mcp::bridge::BridgeError;
    /// let url = "http://127.0.0.1:1".to_string();
    /// let refused = |status| BridgeError::Rejected { url: url.clone(), status, body: String::new() };
    /// assert!(refused(404).calls_for_an_election());
    /// assert!(refused(409).calls_for_an_election());
    /// assert!(!refused(400).calls_for_an_election(), "a serving server's refusal is its answer");
    /// assert!(!refused(413).calls_for_an_election(), "a message too large is too large anywhere");
    /// assert!(BridgeError::Unanswered { url: url.clone(), reason: "timed out".into() }.calls_for_an_election());
    /// ```
    pub fn calls_for_an_election(&self) -> bool {
        match self {
            BridgeError::Unreachable { .. } | BridgeError::Unanswered { .. } => true,
            BridgeError::Rejected { status, .. } => matches!(status, 404 | 409),
        }
    }

    /// Whether `message` may be sent again after this failure without the risk of running
    /// it twice: when the server provably did not run it (no connection, a lost session or
    /// lease), or when running it twice changes nothing the first run did not
    /// ([`idempotent`]).
    ///
    /// ```
    /// use majordomus_cli::mcp::bridge::BridgeError;
    /// use serde_json::json;
    /// let url = "http://127.0.0.1:1".to_string();
    /// let call = |tool: &str| json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": tool } });
    /// let writer = call("majordomus_plan_transition");
    /// let reader = call("majordomus_repository");
    /// let lost = BridgeError::Unanswered { url: url.clone(), reason: "timed out".into() };
    /// assert!(!lost.may_resend(&writer), "a write that may have run is not run again");
    /// assert!(lost.may_resend(&reader), "a read is");
    /// let never_sent = BridgeError::Unreachable { url: url.clone(), reason: "refused".into() };
    /// assert!(never_sent.may_resend(&writer));
    /// let too_large = BridgeError::Rejected { url, status: 413, body: String::new() };
    /// assert!(!too_large.may_resend(&reader));
    /// ```
    pub fn may_resend(&self, message: &Value) -> bool {
        match self {
            BridgeError::Unreachable { .. } => true,
            BridgeError::Rejected { status, .. } => matches!(status, 404 | 409),
            BridgeError::Unanswered { .. } => idempotent(message),
        }
    }
}

/// Whether sending `message` twice changes nothing that sending it once did not.
///
/// Every MCP method but `tools/call` reads. A `tools/call` is idempotent when its tool's
/// declared execution hints say so ([`crate::capability::ExecutionPolicy::hints`]), looked
/// up in the compiled-in registry; a tool that registry does not know is assumed not to
/// be, because a call wrongly withheld is reported while a call wrongly repeated may have
/// written twice. A batch is idempotent when every message in it is.
///
/// ```
/// use majordomus_cli::mcp::bridge::idempotent;
/// use serde_json::json;
/// let call = |tool: &str| json!({ "method": "tools/call", "params": { "name": tool } });
/// assert!(idempotent(&json!({ "method": "resources/read", "params": { "uri": "majordomus://repository" } })));
/// assert!(idempotent(&call("majordomus_repository")));
/// assert!(!idempotent(&call("majordomus_plan_transition")));
/// assert!(!idempotent(&call("no_such_tool")));
/// assert!(!idempotent(&json!([call("majordomus_repository"), call("majordomus_plan_transition")])));
/// ```
pub fn idempotent(message: &Value) -> bool {
    match message {
        Value::Array(batch) => batch.iter().all(idempotent),
        m if m.get("method").and_then(Value::as_str) == Some("tools/call") => m
            .pointer("/params/name")
            .and_then(Value::as_str)
            .is_some_and(tool_is_idempotent),
        _ => true,
    }
}

fn tool_is_idempotent(name: &str) -> bool {
    // composed only on a failure, never on the path of a message that was answered: the
    // bridge keeps starting in milliseconds
    crate::capability::CapabilityRegistry::builder()
        .with_modules(crate::capability::builtin::modules())
        .build()
        .ok()
        .and_then(|r| r.by_mcp_tool(name).map(|c| c.execution.hints().idempotent))
        .unwrap_or(false)
}

/// A stdio session forwarded to a shared server.
///
/// Besides the frames, the bridge keeps two things it saw pass through: the client's
/// `initialize`, so that a session can be re-opened without the client noticing, and the
/// arguments of every accepted announcement the client has made, so that what the client
/// said it was working on outlives the server it said it to. An announcement was the one
/// fact that died with a server: on 2026-09-09 a session's intent vanished from the board
/// when its transport was re-established, and eight peers could not see it for three hours.
/// The bridge is the one place that sees both the announcement and the reconnection, so
/// the bridge replays them — after a re-attach through [`Bridge::reinitialize`], and, for a
/// takeover, through [`Bridge::announcements`] onto the new server's own board.
///
/// *Every* announcement, one per claim name, and not merely the last one: a client may
/// hold several claims at once, and replaying only the most recent would restore one of
/// them and quietly drop the rest — which is the same loss the replay exists to prevent,
/// arriving by a different route.
#[derive(Debug)]
pub struct Bridge {
    url: String,
    session: Option<String>,
    initialize: Option<Value>,
    client: Option<ClientInfo>,
    /// The arguments of the last accepted announcement per claim name, `""` for the
    /// unnamed claim. A `BTreeMap` so a replay is in a stable order and re-announcing a
    /// name replaces rather than appends, exactly as the board itself does.
    announcements: BTreeMap<String, Value>,
    /// How long one answer is waited for.
    timeout: Duration,
}

impl Bridge {
    /// A bridge to the server at `url`, with no session yet, waiting [`REQUEST_TIMEOUT`]
    /// for each answer.
    pub fn new(url: String) -> Self {
        Self::with_timeout(url, REQUEST_TIMEOUT)
    }

    /// A bridge to the server at `url` that waits `timeout` for each answer. A call that
    /// outlasts it is [`BridgeError::Unanswered`]: it was sent, and whether it ran is unknown.
    ///
    /// ```
    /// use majordomus_cli::mcp::bridge::Bridge;
    /// use std::time::Duration;
    /// let bridge = Bridge::with_timeout("http://127.0.0.1:8741".into(), Duration::from_secs(5));
    /// assert_eq!(bridge.url(), "http://127.0.0.1:8741");
    /// ```
    pub fn with_timeout(url: String, timeout: Duration) -> Self {
        Bridge {
            url,
            session: None,
            initialize: None,
            client: None,
            announcements: BTreeMap::new(),
            timeout,
        }
    }

    /// The arguments of every announcement the server accepted, one per claim name, in
    /// name order: what a takeover replays onto the new server's board.
    ///
    /// Empty when the client has announced nothing, which is every bridge that has only
    /// just been made — nothing has passed through it yet to be remembered.
    ///
    /// ```
    /// use majordomus_cli::mcp::bridge::Bridge;
    /// let bridge = Bridge::new("http://127.0.0.1:8741/mcp".into());
    /// assert_eq!(bridge.announcements().count(), 0);
    /// ```
    pub fn announcements(&self) -> impl Iterator<Item = &Value> {
        self.announcements.values()
    }

    /// The server's URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// What the client said in `initialize`, once it has.
    pub fn client(&self) -> Option<&ClientInfo> {
        self.client.as_ref()
    }

    /// Point the bridge at another server; the next message opens a session there.
    pub fn move_to(&mut self, url: String) {
        self.url = url;
        self.session = None;
    }

    /// Forward one message; `None` when the server had nothing to say (a notification).
    pub fn handle(&mut self, message: &Value) -> Result<Option<Value>, BridgeError> {
        if crate::http::mcp::is_initialize(message) {
            let params = first_initialize_params(message);
            self.client = Some(ClientInfo::from_initialize(&params));
            self.initialize = Some(params);
            self.session = None;
        }
        let answer = match self.send(message)? {
            Sent::Answer(v) => v,
            Sent::SessionLost => {
                // the server forgot us (it restarted, or we were idle too long): open a
                // new session with the client's own initialize and try once more
                self.reinitialize()?;
                match self.send(message)? {
                    Sent::Answer(v) => v,
                    Sent::SessionLost => {
                        return Err(BridgeError::Rejected {
                            url: self.url.clone(),
                            status: 404,
                            body: "session lost twice in a row".into(),
                        })
                    }
                }
            }
        };
        if let Some(arguments) = announcement_in(message, answer.as_ref()) {
            let name = arguments
                .get("claim")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .unwrap_or("")
                .to_string();
            self.announcements.insert(name, arguments);
        }
        Ok(answer)
    }

    /// Open a new session with the initialize the client sent earlier. A no-op when it
    /// has not sent one yet.
    pub fn reinitialize(&mut self) -> Result<(), BridgeError> {
        let Some(params) = self.initialize.clone() else {
            return Ok(());
        };
        self.session = None;
        let init = json!({ "jsonrpc": "2.0", "id": "majordomus-bridge-reinitialize", "method": "initialize", "params": params });
        if let Sent::SessionLost = self.send(&init)? {
            return Err(BridgeError::Rejected {
                url: self.url.clone(),
                status: 404,
                body: "initialize was not accepted".into(),
            });
        }
        let _ = self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;
        tracing::info!(url = %self.url, "session re-opened on the shared server");
        // what the client said it was working on is said again, to whoever serves now —
        // every claim it holds, because restoring one of several is still a loss
        for arguments in self.announcements.values().cloned().collect::<Vec<_>>() {
            let replay = json!({
                "jsonrpc": "2.0",
                "id": "majordomus-bridge-announce",
                "method": "tools/call",
                "params": { "name": crate::capability::builtin::peers::ANNOUNCE_TOOL, "arguments": arguments },
            });
            match self.send(&replay) {
                Ok(Sent::Answer(_)) => {
                    tracing::info!(url = %self.url, "the client's announcement was replayed to the server it is now attached to")
                }
                Ok(Sent::SessionLost) | Err(_) => {
                    tracing::warn!(url = %self.url, "the client's announcement could not be replayed; the board understates it until it announces again")
                }
            }
        }
        Ok(())
    }

    /// Keep the session alive.
    pub fn heartbeat(&mut self) -> Result<(), BridgeError> {
        if self.session.is_none() {
            return Ok(());
        }
        self.send(
            &json!({ "jsonrpc": "2.0", "id": "majordomus-bridge-heartbeat", "method": "ping" }),
        )
        .map(|_| ())
    }

    /// End the session on the server; the client has gone.
    pub fn close(&mut self) {
        if let Some(session) = self.session.take() {
            let _ = request(
                &self.url,
                "DELETE",
                crate::http::mcp::PATH,
                &[(SESSION_HEADER, &session)],
                None,
                CONNECT_TIMEOUT,
            );
        }
    }

    fn send(&mut self, message: &Value) -> Result<Sent, BridgeError> {
        let session = self.session.clone();
        let headers: Vec<(&str, &str)> = session
            .as_deref()
            .map(|s| vec![(SESSION_HEADER, s)])
            .unwrap_or_default();
        let body = message.to_string();
        // the server would refuse it unread; saying so here keeps the refusal from being
        // mistaken for a connection the server cut short while the body was still going out
        if body.len() > crate::http::server::MAX_BODY_BYTES {
            return Err(BridgeError::Rejected {
                url: self.url.clone(),
                status: 413,
                body: format!(
                    "the message is {} bytes, over the {} the server accepts; it was not sent",
                    body.len(),
                    crate::http::server::MAX_BODY_BYTES
                ),
            });
        }
        let (stream, host) =
            connect(&self.url, self.timeout).map_err(|e| BridgeError::Unreachable {
                url: self.url.clone(),
                reason: e.to_string(),
            })?;
        let reply = exchange(
            stream,
            &host,
            "POST",
            crate::http::mcp::PATH,
            &headers,
            Some(&body),
            self.timeout,
        )
        .map_err(|e| BridgeError::Unanswered {
            url: self.url.clone(),
            reason: e.to_string(),
        })?;
        if let Some(id) = reply.header(SESSION_HEADER) {
            self.session = Some(id.to_string());
        }
        match reply.status {
            200 => serde_json::from_str::<Value>(&reply.body)
                .map(|v| Sent::Answer(Some(v)))
                .map_err(|e| BridgeError::Unanswered {
                    url: self.url.clone(),
                    reason: format!("the answer is not JSON: {e}"),
                }),
            202 | 204 => Ok(Sent::Answer(None)),
            404 => Ok(Sent::SessionLost),
            // sent without a session by a bridge whose client did initialize (a re-attach
            // whose initialize failed): refused before the message was looked at, so it is
            // a session to open again, as a 404 is. A client that never initialized is told.
            400 if session.is_none()
                && self.initialize.is_some()
                && reply.body.contains("session_required") =>
            {
                Ok(Sent::SessionLost)
            }
            s if s >= 500 => Err(BridgeError::Unanswered {
                url: self.url.clone(),
                reason: format!("status {s}: {}", reply.body),
            }),
            s => Err(BridgeError::Rejected {
                url: self.url.clone(),
                status: s,
                body: reply.body,
            }),
        }
    }
}

enum Sent {
    Answer(Option<Value>),
    SessionLost,
}

/// The arguments of an announcement the server accepted: the message is a `tools/call` of
/// the announce tool, and the answer is a result rather than an error. A batch is not
/// looked into; a bridge that carried one announcement in a batch replays nothing, which
/// is what a bridge did for every announcement before this.
fn announcement_in(message: &Value, answer: Option<&Value>) -> Option<Value> {
    if message.get("method")?.as_str()? != "tools/call" {
        return None;
    }
    let params = message.get("params")?;
    if params.get("name")?.as_str()? != crate::capability::builtin::peers::ANNOUNCE_TOOL {
        return None;
    }
    let answer = answer?;
    if answer.get("error").is_some() || answer["result"]["isError"] == true {
        return None;
    }
    Some(
        params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({})),
    )
}

fn first_initialize_params(message: &Value) -> Value {
    match message {
        Value::Array(batch) => batch
            .iter()
            .find(|m| m["method"] == "initialize")
            .map(|m| m["params"].clone())
            .unwrap_or(Value::Null),
        m => m["params"].clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bridge remembers one announcement per claim name, so that a client holding several
    /// claims has all of them replayed after a re-attach rather than the newest one. Before
    /// named claims this was a single slot, and a client that announced twice replayed only
    /// the second — a loss the replay itself exists to prevent, arriving by another route.
    #[test]
    fn a_bridge_remembers_one_announcement_per_claim() {
        let mut bridge = Bridge::new("http://127.0.0.1:1/mcp".into());
        let call = |claim: Option<&str>, intent: &str| {
            let mut arguments = json!({ "intent": intent, "scope": ["docs"] });
            if let Some(c) = claim {
                arguments["claim"] = json!(c);
            }
            json!({
                "method": "tools/call",
                "params": { "name": crate::capability::builtin::peers::ANNOUNCE_TOOL, "arguments": arguments },
            })
        };
        let accepted = json!({ "result": { "isError": false } });

        // remembering is what `send` does after an answer; do the same thing here without
        // a socket, because the remembering is the behaviour under test and not the transport
        for (claim, intent) in [
            (None, "the unnamed claim"),
            (Some("worker-a"), "the guard"),
            (Some("worker-b"), "the cockpit"),
            (Some("worker-a"), "the guard, narrowed"),
        ] {
            let message = call(claim, intent);
            let arguments = announcement_in(&message, Some(&accepted)).expect("accepted");
            let name = arguments
                .get("claim")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            bridge.announcements.insert(name, arguments);
        }

        let held: Vec<&str> = bridge
            .announcements()
            .map(|a| a["intent"].as_str().unwrap())
            .collect();
        assert_eq!(
            held,
            vec!["the unnamed claim", "the guard, narrowed", "the cockpit"],
            "three claims, in name order, and the repeated name updated rather than appended"
        );
    }

    /// An announcement the server refused is not remembered, so a takeover does not replay a
    /// claim the board never accepted.
    #[test]
    fn a_refused_announcement_is_not_remembered() {
        let message = json!({
            "method": "tools/call",
            "params": {
                "name": crate::capability::builtin::peers::ANNOUNCE_TOOL,
                "arguments": { "intent": "refused", "scope": [] },
            },
        });
        let refused = json!({ "result": { "isError": true } });
        assert!(announcement_in(&message, Some(&refused)).is_none());
        let errored = json!({ "error": { "code": -32000 } });
        assert!(announcement_in(&message, Some(&errored)).is_none());
    }

    fn call(tool: &str) -> Value {
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": tool } })
    }

    #[test]
    fn a_batch_is_idempotent_only_when_every_message_in_it_is() {
        let read = call("majordomus_repository");
        let write = call("majordomus_plan_transition");
        let ping = json!({ "jsonrpc": "2.0", "id": 4, "method": "ping" });
        assert!(idempotent(&json!([read.clone(), ping])));
        assert!(!idempotent(&json!([read, write])));
        assert!(idempotent(&json!([])), "an empty batch runs nothing twice");
    }

    #[test]
    fn a_message_over_the_body_bound_is_refused_without_being_sent() {
        // nothing listens on port 1: a message that were sent would come back unreachable
        let mut bridge = Bridge::new("http://127.0.0.1:1".into());
        let pad = "x".repeat(crate::http::server::MAX_BODY_BYTES);
        let message = json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call",
            "params": { "name": "majordomus_repository", "arguments": { "pad": pad } } });
        let err = bridge.handle(&message).expect_err("refused");
        match &err {
            BridgeError::Rejected { status, body, url } => {
                assert_eq!(*status, 413);
                assert_eq!(url, "http://127.0.0.1:1");
                assert!(body.contains("it was not sent"), "{body}");
                assert!(
                    body.contains(&crate::http::server::MAX_BODY_BYTES.to_string()),
                    "{body}"
                );
            }
            other => panic!("refused before a connection was made, not {other:?}"),
        }
        assert!(
            !err.calls_for_an_election(),
            "too large is too large anywhere"
        );
        assert!(!err.may_resend(&message));
    }

    /// One scripted HTTP answer: status, body, and the session id it opens.
    type Scripted = (u16, &'static str, Option<&'static str>);

    /// A server that answers each connection with the next scripted reply and records what
    /// it received: the request line, the session header, and the body.
    fn scripted(replies: Vec<Scripted>) -> (String, std::thread::JoinHandle<Vec<Value>>) {
        use std::io::BufRead;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for (status, body, session) in replies {
                let (stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(30)))
                    .unwrap();
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let (mut length, mut header) = (0usize, Value::Null);
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    let h = h.trim_end();
                    if h.is_empty() {
                        break;
                    }
                    let (k, v) = h.split_once(':').unwrap();
                    if k.eq_ignore_ascii_case("content-length") {
                        length = v.trim().parse().unwrap();
                    }
                    if k.eq_ignore_ascii_case(SESSION_HEADER) {
                        header = json!(v.trim());
                    }
                }
                let mut raw = vec![0u8; length];
                reader.read_exact(&mut raw).unwrap();
                let message: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
                seen.push(
                    json!({ "line": line.trim_end(), "session": header, "message": message }),
                );
                let mut head = format!(
                    "HTTP/1.1 {status} X\r\nConnection: close\r\nContent-Length: {}\r\n",
                    body.len()
                );
                if let Some(s) = session {
                    head.push_str(&format!("{SESSION_HEADER}: {s}\r\n"));
                }
                let mut stream = stream;
                stream
                    .write_all(format!("{head}\r\n{body}").as_bytes())
                    .unwrap();
            }
            seen
        });
        (url, server)
    }

    fn initialize() -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "bridge-test", "version": "0" } } })
    }

    #[test]
    fn a_session_required_after_initialize_is_opened_again_with_the_clients_initialize() {
        let answer = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18"}}"#;
        let (url, server) = scripted(vec![
            // refused before it was looked at: no session, although the client initialized
            (400, r#"{"error":"session_required"}"#, None),
            // the session the bridge opens again with the client's own initialize
            (200, answer, Some("s-1")),
            (202, "", None),
            // and the message, sent once more on it
            (200, answer, None),
        ]);
        let mut bridge = Bridge::with_timeout(url, Duration::from_secs(30));
        let got = bridge
            .handle(&initialize())
            .expect("answered on the reopened session");
        assert_eq!(got.unwrap()["result"]["protocolVersion"], "2025-06-18");
        let seen = server.join().unwrap();
        let methods: Vec<&str> = seen
            .iter()
            .map(|s| s["message"]["method"].as_str().unwrap_or_default())
            .collect();
        assert_eq!(
            methods,
            vec![
                "initialize",
                "initialize",
                "notifications/initialized",
                "initialize"
            ]
        );
        assert_eq!(
            seen[1]["message"]["params"]["clientInfo"]["name"],
            "bridge-test"
        );
        assert_eq!(seen[2]["session"], "s-1", "the reopened session is used");
        assert_eq!(seen[3]["session"], "s-1");
    }

    #[test]
    fn a_client_that_never_initialized_is_told_a_session_is_required() {
        let (url, server) = scripted(vec![(400, r#"{"error":"session_required"}"#, None)]);
        let mut bridge = Bridge::with_timeout(url, Duration::from_secs(30));
        let err = bridge
            .handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }))
            .expect_err("refused");
        assert!(
            matches!(&err, BridgeError::Rejected { status: 400, body, .. } if body.contains("session_required")),
            "{err:?}"
        );
        assert!(!err.calls_for_an_election());
        assert_eq!(server.join().unwrap().len(), 1, "sent once, never again");
    }

    #[test]
    fn a_server_failure_leaves_the_outcome_unknown() {
        let (url, server) = scripted(vec![(503, "overloaded", None)]);
        let mut bridge = Bridge::with_timeout(url, Duration::from_secs(30));
        let write = call("majordomus_plan_transition");
        let err = bridge.handle(&write).expect_err("not answered");
        assert!(
            matches!(&err, BridgeError::Unanswered { reason, .. } if reason.contains("status 503") && reason.contains("overloaded")),
            "{err:?}"
        );
        assert!(err.calls_for_an_election());
        assert!(!err.may_resend(&write), "a write that may have run");
        assert_eq!(server.join().unwrap().len(), 1);
    }

    /// A one-connection server: it reads the request's first bytes and answers with `write`.
    fn answering(write: impl FnOnce(&mut TcpStream) + Send + 'static) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut head = [0u8; 4096];
            let _ = stream.read(&mut head);
            write(&mut stream);
        });
        url
    }

    fn within(bytes: usize) -> ReplyBound {
        ReplyBound {
            bytes,
            idle: Duration::from_secs(5),
            within: Duration::from_secs(10),
        }
    }

    #[test]
    fn a_bounded_reply_is_held_without_its_whitespace_and_a_byte_over_is_refused() {
        let pretty = "{\n  \"a\": \"x y\\\" z\",\n  \"b\": [\n    1,\n    2\n  ]\n}";
        let compact = r#"{"a":"x y\" z","b":[1,2]}"#;
        let serve = move |s: &mut TcpStream| {
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{pretty}",
                    pretty.len()
                )
                .as_bytes(),
            );
        };
        let url = answering(serve);
        let reply = request_json_within(&url, "GET", "/", &[], None, within(compact.len()))
            .expect("a reply at its bound is read");
        assert_eq!(reply.status, 200);
        assert_eq!(
            reply.body, compact,
            "the strings keep their spaces and escapes"
        );
        assert_eq!(
            serde_json::from_str::<Value>(&reply.body).unwrap(),
            serde_json::from_str::<Value>(pretty).unwrap()
        );

        let url = answering(serve);
        let error = request_json_within(&url, "GET", "/", &[], None, within(compact.len() - 1))
            .expect_err("a byte over the bound");
        assert_eq!(
            ReplyRefused::of(&error),
            Some(&ReplyRefused::Oversized {
                bound: compact.len() - 1
            })
        );
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_head_that_never_ends_is_refused_at_its_bound() {
        let url = answering(|s| {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\n");
            let line = format!("X-Pad: {}\r\n", "p".repeat(1000));
            for _ in 0..(4 * MAX_HEAD_BYTES / line.len()) {
                if s.write_all(line.as_bytes()).is_err() {
                    return;
                }
            }
        });
        let error = request_json_within(&url, "GET", "/", &[], None, within(1024))
            .expect_err("a head over its bound");
        assert_eq!(
            ReplyRefused::of(&error),
            Some(&ReplyRefused::Oversized {
                bound: MAX_HEAD_BYTES
            })
        );
    }
}
