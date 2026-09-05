//! The bridge: a `majordomus mcp` process that found the repository's shared server
//! already running forwards its client's stdio frames to that server's `/mcp` endpoint
//! and writes the answers back, one HTTP request per message, and pings on the side so
//! that the server keeps the session. It carries no index and no registry of its own,
//! which is why it starts in milliseconds. The HTTP client below is the few lines a
//! loopback request needs (the server always answers with `Content-Length`, never
//! chunked); no HTTP library is pulled in for it.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::{json, Value};

use crate::peers::ClientInfo;

/// How often a bridge pings the server so that its session is not forgotten.
pub const HEARTBEAT: Duration = Duration::from_secs(20);

/// How long a bridge waits to connect.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// How long a bridge waits for one answer.
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

/// One HTTP/1.1 request to `base_url` (`http://host:port`), with `Connection: close`.
pub fn request(
    base_url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<Reply> {
    let host = base_url
        .strip_prefix("http://")
        .unwrap_or(base_url)
        .trim_end_matches('/');
    let addr = host
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other(format!("{host}: no address")))?;
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
    stream.set_read_timeout(Some(timeout))?;
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
    stream.write_all(text.as_bytes())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    parse_reply(&raw)
}

fn parse_reply(raw: &[u8]) -> std::io::Result<Reply> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| std::io::Error::other("malformed HTTP response: no header end"))?;
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
    /// No connection, a timeout, or a server-side failure: the server is gone or broken.
    Unreachable {
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

/// A stdio session forwarded to a shared server.
#[derive(Debug)]
pub struct Bridge {
    url: String,
    session: Option<String>,
    initialize: Option<Value>,
    client: Option<ClientInfo>,
}

impl Bridge {
    /// A bridge to the server at `url`, with no session yet.
    pub fn new(url: String) -> Self {
        Bridge {
            url,
            session: None,
            initialize: None,
            client: None,
        }
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
        match self.send(message)? {
            Sent::Answer(v) => Ok(v),
            Sent::SessionLost => {
                // the server forgot us (it restarted, or we were idle too long): open a
                // new session with the client's own initialize and try once more
                self.reinitialize()?;
                match self.send(message)? {
                    Sent::Answer(v) => Ok(v),
                    Sent::SessionLost => Err(BridgeError::Rejected {
                        url: self.url.clone(),
                        status: 404,
                        body: "session lost twice in a row".into(),
                    }),
                }
            }
        }
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
        let reply = request(
            &self.url,
            "POST",
            crate::http::mcp::PATH,
            &headers,
            Some(&body),
            REQUEST_TIMEOUT,
        )
        .map_err(|e| BridgeError::Unreachable {
            url: self.url.clone(),
            reason: e.to_string(),
        })?;
        if let Some(id) = reply.header(SESSION_HEADER) {
            self.session = Some(id.to_string());
        }
        match reply.status {
            200 => serde_json::from_str::<Value>(&reply.body)
                .map(|v| Sent::Answer(Some(v)))
                .map_err(|e| BridgeError::Unreachable {
                    url: self.url.clone(),
                    reason: format!("the answer is not JSON: {e}"),
                }),
            202 | 204 => Ok(Sent::Answer(None)),
            404 => Ok(Sent::SessionLost),
            s if s >= 500 => Err(BridgeError::Unreachable {
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
