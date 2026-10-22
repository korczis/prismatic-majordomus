//! The bridge: a `majordomus mcp` process that found the repository's shared server
//! already running forwards its client's stdio frames to that server's `/mcp` endpoint
//! and writes the answers back, one HTTP request per message, and pings on the side so
//! that the server keeps the session. It carries no index and no registry of its own,
//! which is why it starts in milliseconds. The HTTP client below is the few lines a
//! request needs (the server always answers with `Content-Length`, never chunked); no HTTP
//! library is pulled in for it.
//!
//! The same client carries the mesh (ADR 0220, decision 1). An `https://` endpoint is
//! reached over TLS — rustls, with the certificate verified against the roots compiled into
//! the executable and the host name checked against it — and an `http://` endpoint only when
//! every address its host resolves to is loopback, private, link-local or an overlay's
//! (100.64.0.0/10, IPv6 unique-local): a public address is refused before a byte is sent,
//! with the correction. Plain and TLS requests share one framing over one `Read + Write`.
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
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
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

/// How an endpoint's bytes travel, once [`transport`] has allowed it.
///
/// ```
/// use majordomus_cli::mcp::bridge::{transport, Transport};
/// // a hub by name over TLS; a seed on a private network in cleartext
/// assert_eq!(transport("https://prismatic-majordomus.gigalixirapp.com").unwrap(), Transport::Tls);
/// assert_eq!(transport("http://192.168.1.20:8742").unwrap(), Transport::Cleartext);
/// assert_ne!(Transport::Tls, Transport::Cleartext);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// `https://`: over TLS, the certificate and the host name verified.
    Tls,
    /// `http://` (or no scheme): in cleartext, to addresses that are not public.
    Cleartext,
}

/// Whether cleartext may be sent to `ip`: loopback, RFC 1918 private, link-local, the
/// 100.64.0.0/10 range an overlay (a tailnet) and carrier-grade NAT hand out, the unspecified
/// address (which a connection reads as this machine), IPv6 unique-local (fc00::/7), and an
/// IPv4-mapped IPv6 address by the IPv4 address it carries. Everything else is public.
///
/// ```
/// use majordomus_cli::mcp::bridge::cleartext_allowed;
/// assert!(cleartext_allowed("192.168.100.30".parse().unwrap()));
/// assert!(cleartext_allowed("100.92.246.32".parse().unwrap()), "a tailnet address");
/// assert!(cleartext_allowed("fd7a:115c:a1e0::1".parse().unwrap()), "unique-local");
/// assert!(!cleartext_allowed("203.0.113.5".parse().unwrap()));
/// assert!(!cleartext_allowed("2001:db8::1".parse().unwrap()));
/// ```
pub fn cleartext_allowed(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_cleartext_allowed(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return v4_cleartext_allowed(v4);
            }
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}

fn v4_cleartext_allowed(v4: Ipv4Addr) -> bool {
    let [a, b, _, _] = v4.octets();
    v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_unspecified()
        || (a == 100 && (b & 0xc0) == 64)
}

/// An endpoint as written: whether it is reached over TLS, the host to resolve and to name
/// to the certificate, the port, and the authority the `Host` header carries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Endpoint {
    tls: bool,
    host: String,
    port: u16,
    authority: String,
}

impl Endpoint {
    /// The endpoint as a URL, for a message that names it.
    fn url(&self) -> String {
        let scheme = if self.tls { "https" } else { "http" };
        format!("{scheme}://{}", self.authority)
    }
}

/// Split an authority (`host`, `host:port`, `[v6]` or `[v6]:port`) into the host — without
/// the brackets — and the port it names, if any.
pub(crate) fn split_authority(authority: &str) -> Result<(String, Option<u16>), String> {
    let (host, port) = match authority.strip_prefix('[') {
        Some(v6) => {
            let (host, rest) = v6
                .split_once(']')
                .ok_or_else(|| format!("'{authority}' opens a bracket it does not close"))?;
            match rest {
                "" => (host, None),
                _ => (
                    host,
                    Some(rest.strip_prefix(':').ok_or_else(|| {
                        format!("'{authority}' has something other than a port after its address")
                    })?),
                ),
            }
        }
        None => match authority.rsplit_once(':') {
            Some((host, _)) if host.contains(':') => {
                return Err(format!(
                    "'{authority}' is an IPv6 address without brackets: write [address]:port"
                ))
            }
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        },
    };
    if host.is_empty() {
        return Err(format!("'{authority}' names no host"));
    }
    let port = match port {
        None => None,
        Some(p) => Some(
            p.parse::<u16>()
                .ok()
                .filter(|p| *p != 0)
                .ok_or_else(|| format!("'{authority}' does not name a port from 1 to 65535"))?,
        ),
    };
    Ok((host.to_string(), port))
}

/// Read `base_url` (`https://host[:port]`, `http://host[:port]`, or `host:port`, which is
/// http) into an [`Endpoint`]. A path after the authority is not the endpoint's and is
/// ignored, as it always was; any other scheme is refused.
fn parse_endpoint(base_url: &str) -> std::io::Result<Endpoint> {
    let invalid = |reason: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, reason);
    let (tls, rest) = if let Some(rest) = base_url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = base_url.strip_prefix("http://") {
        (false, rest)
    } else if base_url.contains("://") {
        return Err(invalid(format!(
            "{base_url}: the scheme is neither http:// nor https://"
        )));
    } else {
        (false, base_url)
    };
    let authority = rest.split('/').next().unwrap_or_default();
    let (host, port) =
        split_authority(authority).map_err(|e| invalid(format!("{base_url}: {e}")))?;
    let port = port.unwrap_or(if tls { 443 } else { 80 });
    Ok(Endpoint {
        tls,
        host,
        port,
        authority: authority.to_string(),
    })
}

/// Every address `endpoint`'s host resolves to, in the resolver's order.
fn resolve(endpoint: &Endpoint) -> std::io::Result<Vec<SocketAddr>> {
    let addrs: Vec<SocketAddr> = (endpoint.host.as_str(), endpoint.port)
        .to_socket_addrs()?
        .collect();
    some_address(addrs, &endpoint.authority)
}

/// `addrs`, unless the resolver answered with none: that is a failure naming `authority`, so
/// that nothing indexes an empty answer.
fn some_address(addrs: Vec<SocketAddr>, authority: &str) -> std::io::Result<Vec<SocketAddr>> {
    if addrs.is_empty() {
        return Err(std::io::Error::other(format!("{authority}: no address")));
    }
    Ok(addrs)
}

/// The refusal of cleartext to `endpoint`, when one of the addresses it resolves to is
/// public: every one must be private for the request to be sent, because the resolver, not
/// this client, picks which of them a later connection reaches.
fn cleartext_refusal(endpoint: &Endpoint, addrs: &[SocketAddr]) -> Option<std::io::Error> {
    let public = addrs.iter().find(|a| !cleartext_allowed(a.ip()))?;
    Some(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!(
            "{} was not dialed: {} is a public address, and the mesh sends nothing across a public network in cleartext; use https://{} (ADR 0220)",
            endpoint.url(),
            public.ip(),
            endpoint.authority
        ),
    ))
}

/// How `base_url` would be reached, or why it would not be: [`Transport::Tls`] for an
/// `https://` endpoint (nothing is resolved), [`Transport::Cleartext`] for an `http://` one
/// whose every address is not public, and an error otherwise — `PermissionDenied` naming the
/// public address, `InvalidInput` for an endpoint that cannot be read, or the resolver's own.
/// The check [`request`] makes before it connects, offered to a self-check that dials nothing.
///
/// ```
/// use majordomus_cli::mcp::bridge::{transport, Transport};
/// assert_eq!(transport("https://hub.example:443").unwrap(), Transport::Tls);
/// assert_eq!(transport("http://10.0.0.2:8742").unwrap(), Transport::Cleartext);
/// let refused = transport("http://203.0.113.5:8791").unwrap_err();
/// assert_eq!(refused.kind(), std::io::ErrorKind::PermissionDenied);
/// assert!(refused.to_string().contains("203.0.113.5") && refused.to_string().contains("use https://"));
/// ```
pub fn transport(base_url: &str) -> std::io::Result<Transport> {
    let endpoint = parse_endpoint(base_url)?;
    if endpoint.tls {
        return Ok(Transport::Tls);
    }
    let addrs = resolve(&endpoint)?;
    match cleartext_refusal(&endpoint, &addrs) {
        Some(refusal) => Err(refusal),
        None => Ok(Transport::Cleartext),
    }
}

/// The TLS client configuration of every `https://` request: the ring provider, its safe
/// default protocol versions, and the Mozilla roots compiled into the executable. Built on the
/// first TLS request, never on the plain path.
fn default_tls() -> std::io::Result<Arc<ClientConfig>> {
    static CONFIG: OnceLock<Result<Arc<ClientConfig>, String>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            tls_config(RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            })
        })
        .clone()
        .map_err(std::io::Error::other)
}

/// A client configuration that trusts `roots` and nothing else.
fn tls_config(roots: RootCertStore) -> Result<Arc<ClientConfig>, String> {
    ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map(|versions| Arc::new(versions.with_root_certificates(roots).with_no_client_auth()))
        .map_err(unbuildable)
}

/// Why a TLS configuration could not be had: the provider offers no protocol version the
/// safe defaults allow.
fn unbuildable(e: rustls::Error) -> String {
    format!("the TLS configuration could not be built: {e}")
}

/// An open connection: a socket, or a socket under TLS.
enum Stream {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Stream {
    fn socket(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Tls(s) => &s.sock,
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

/// One HTTP/1.1 request to `base_url` (`http://host:port` or `https://host[:port]`), with
/// `Connection: close`.
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

/// Open a connection to `base_url`. Nothing of the request has been sent when this fails,
/// which is what lets a caller send the same message again elsewhere.
fn connect(base_url: &str, timeout: Duration) -> std::io::Result<(Stream, String)> {
    connect_with(base_url, timeout, default_tls)
}

/// [`connect`] with the TLS configuration `tls` returns, asked for only when the endpoint is
/// `https://`. The tests hand it one that trusts their own authority.
fn connect_with(
    base_url: &str,
    timeout: Duration,
    tls: impl FnOnce() -> std::io::Result<Arc<ClientConfig>>,
) -> std::io::Result<(Stream, String)> {
    let endpoint = parse_endpoint(base_url)?;
    // the name the certificate is checked against, read before anything is resolved
    let name = match endpoint.tls {
        true => Some(server_name(&endpoint)?),
        false => None,
    };
    let addrs = resolve(&endpoint)?;
    if !endpoint.tls {
        if let Some(refusal) = cleartext_refusal(&endpoint, &addrs) {
            return Err(refusal);
        }
    }
    // The connection shares the caller's budget when that is the shorter: a probe on a shell
    // prompt that allows 250 ms must not wait 2 s on a server whose accept queue is full.
    let socket = TcpStream::connect_timeout(&addrs[0], CONNECT_TIMEOUT.min(timeout))?;
    let stream = match name {
        Some(name) => Stream::Tls(Box::new(handshake(
            socket,
            &endpoint,
            name,
            tls()?,
            timeout,
        )?)),
        None => Stream::Plain(socket),
    };
    Ok((stream, endpoint.authority))
}

/// The name `endpoint`'s certificate must be valid for, and the one SNI carries: its host, a
/// DNS name or an IP address.
fn server_name(endpoint: &Endpoint) -> std::io::Result<ServerName<'static>> {
    ServerName::try_from(endpoint.host.clone()).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "{}: '{}' is not a name a certificate can be checked against: {e}",
                endpoint.url(),
                endpoint.host
            ),
        )
    })
}

/// Complete the TLS handshake on `socket` within `timeout` per read and write, so that a
/// certificate that does not verify is refused here — naming the endpoint — before any byte
/// of the request is written.
fn handshake(
    mut socket: TcpStream,
    endpoint: &Endpoint,
    name: ServerName<'static>,
    config: Arc<ClientConfig>,
    timeout: Duration,
) -> std::io::Result<StreamOwned<ClientConnection, TcpStream>> {
    bound(&socket, timeout)?;
    let mut connection = ClientConnection::new(config, name).map_err(std::io::Error::other)?;
    // `complete_io` reports a server that closes mid-handshake as `UnexpectedEof`, so the loop
    // ends in a handshake or in an error, never in a spin
    while connection.is_handshaking() {
        connection.complete_io(&mut socket).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!("{}: the TLS handshake failed: {e}", endpoint.url()),
            )
        })?;
    }
    Ok(StreamOwned::new(connection, socket))
}

/// Bound every read and write on `socket` by `timeout`. A zero timeout is refused by the
/// platform, and so here, rather than read as "no bound".
fn bound(socket: &TcpStream, timeout: Duration) -> std::io::Result<()> {
    socket.set_read_timeout(Some(timeout))?;
    socket.set_write_timeout(Some(timeout))
}

/// Send one request on an open connection and read the reply. From the first byte written
/// the server may have the request, so a failure here leaves its outcome unknown.
fn exchange(
    mut stream: Stream,
    host: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    timeout: Duration,
) -> std::io::Result<Reply> {
    bound(stream.socket(), timeout)?;
    exchange_on(&mut stream, host, method, path, headers, body)
}

/// The request and its reply over any stream: the one framing plain and TLS share.
fn exchange_on<S: Read + Write>(
    stream: &mut S,
    host: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> std::io::Result<Reply> {
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
    stream.flush()?;
    let raw = read_reply(stream)?;
    parse_reply(&raw)
}

/// Read a reply to its end, bounded by [`MAX_REPLY_BYTES`]. A TLS server that closes the
/// socket without saying so (no `close_notify`) ends the stream with `UnexpectedEof`; what it
/// sent is the reply when its `Content-Length` is whole, and a truncation otherwise.
fn read_reply<S: Read>(stream: &mut S) -> std::io::Result<Vec<u8>> {
    let mut raw = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if raw.len() > MAX_REPLY_BYTES {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "the reply is over the {MAX_REPLY_BYTES}-byte bound and was not read"
                        ),
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && whole(&raw) => break,
            Err(e) => return Err(e),
        }
    }
    Ok(raw)
}

/// Whether `raw` holds a whole reply: its head, and as many body bytes as its
/// `Content-Length` says. Without a length nothing can say, and it is not whole.
fn whole(raw: &[u8]) -> bool {
    let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") else {
        return false;
    };
    let head = String::from_utf8_lossy(&raw[..end]);
    head.lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
        .is_some_and(|n| raw.len() - (end + 4) >= n)
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

    // ------------------------------------------------------------ transport (ADR 0220)

    #[test]
    fn cleartext_is_allowed_to_loopback_private_link_local_and_overlay_addresses_only() {
        for allowed in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.254",
            "192.168.100.30",
            "100.64.0.1",
            "100.127.255.254",
            "169.254.1.1",
            "0.0.0.0",
            "::1",
            "::",
            "fc00::1",
            "fd7a:115c:a1e0::1",
            "fe80::1",
            "::ffff:192.168.1.1",
        ] {
            assert!(cleartext_allowed(allowed.parse().unwrap()), "{allowed}");
        }
        for public in [
            "203.0.113.5",
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "100.63.255.255",
            "192.0.2.1",
            "2001:db8::1",
            "2606:4700::1111",
            "::ffff:8.8.8.8",
        ] {
            assert!(!cleartext_allowed(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn an_endpoint_is_read_with_its_scheme_host_and_port() {
        let read = |url: &str| parse_endpoint(url).unwrap();
        let tls = read("https://hub.example");
        assert_eq!(
            (tls.tls, tls.host.as_str(), tls.port),
            (true, "hub.example", 443)
        );
        assert_eq!(tls.url(), "https://hub.example");
        let tls = read("https://hub.example:8443/");
        assert_eq!(
            (tls.tls, tls.port, tls.authority.as_str()),
            (true, 8443, "hub.example:8443")
        );
        let plain = read("http://10.0.0.2:8742");
        assert_eq!(
            (plain.tls, plain.host.as_str(), plain.port),
            (false, "10.0.0.2", 8742)
        );
        assert_eq!(plain.url(), "http://10.0.0.2:8742");
        assert_eq!(read("http://10.0.0.2").port, 80);
        let bare = read("127.0.0.1:9");
        assert_eq!(
            (bare.tls, bare.port),
            (false, 9),
            "no scheme is http, as it always was"
        );
        let v6 = read("http://[::1]:8741/mcp");
        assert_eq!(
            (v6.host.as_str(), v6.port, v6.authority.as_str()),
            ("::1", 8741, "[::1]:8741")
        );
        assert_eq!(read("https://[2001:db8::1]").port, 443);

        let refused = |url: &str| {
            let e = parse_endpoint(url).unwrap_err();
            assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput, "{url}");
            e.to_string()
        };
        assert!(refused("ftp://hub.example:21").contains("neither http:// nor https://"));
        assert!(refused("https://:443").contains("names no host"));
        assert!(refused("http://hub.example:0").contains("from 1 to 65535"));
        assert!(refused("http://hub.example:port").contains("from 1 to 65535"));
        assert!(refused("http://[::1:8741").contains("does not close"));
        assert!(refused("http://[::1]8741").contains("other than a port"));
        assert!(refused("http://::1:8741").contains("without brackets"));
        assert!(refused("http://[]:8741").contains("names no host"));
    }

    #[test]
    fn the_transport_of_an_endpoint_is_said_without_dialing_it() {
        assert_eq!(
            transport("https://203.0.113.5:443").unwrap(),
            Transport::Tls
        );
        assert_eq!(
            transport("http://192.168.100.30:8791").unwrap(),
            Transport::Cleartext
        );
        assert_eq!(
            transport("http://100.92.246.32:8791").unwrap(),
            Transport::Cleartext
        );
        assert_eq!(
            transport("http://localhost:8791").unwrap(),
            Transport::Cleartext
        );
        assert_eq!(transport("127.0.0.1:8791").unwrap(), Transport::Cleartext);
        let public = transport("http://203.0.113.5:8791").unwrap_err();
        assert_eq!(public.kind(), std::io::ErrorKind::PermissionDenied);
        let text = public.to_string();
        assert!(
            text.contains("http://203.0.113.5:8791 was not dialed"),
            "{text}"
        );
        assert!(text.contains("203.0.113.5 is a public address"), "{text}");
        assert!(text.contains("use https://203.0.113.5:8791"), "{text}");
        assert_eq!(
            transport("gopher://hub.example:70").unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
    }

    /// Cleartext to a public address is refused before a socket is opened: nothing listens at
    /// a documentation address, so a request that were sent would time out or be unreachable,
    /// and the refusal comes back at once with the correction instead.
    #[test]
    fn a_public_http_endpoint_is_refused_before_any_byte_is_sent() {
        for (url, address) in [
            ("http://203.0.113.5:8791", "203.0.113.5"),
            ("http://[2001:db8::1]:8791", "2001:db8::1"),
            ("198.51.100.7:8791", "198.51.100.7"),
        ] {
            let started = std::time::Instant::now();
            let err = request(
                url,
                "POST",
                "/api/v1/mesh/register",
                &[],
                Some("{}"),
                Duration::from_secs(30),
            )
            .expect_err("refused");
            assert_eq!(
                err.kind(),
                std::io::ErrorKind::PermissionDenied,
                "{url}: {err}"
            );
            assert!(err.to_string().contains(address), "{err}");
            assert!(err.to_string().contains("use https://"), "{err}");
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "no connection was attempted"
            );
        }
        // through the bridge it is a server that was never reached: nothing ran
        let mut bridge = Bridge::new("http://203.0.113.5:8741".into());
        let message = call("majordomus_plan_transition");
        let err = bridge.handle(&message).expect_err("refused");
        assert!(
            matches!(&err, BridgeError::Unreachable { reason, .. } if reason.contains("public address")),
            "{err:?}"
        );
        assert!(err.may_resend(&message), "nothing was sent");
    }

    /// A reader that hands out scripted chunks and errors, in order.
    struct Chunks(std::collections::VecDeque<std::io::Result<Vec<u8>>>);

    impl Read for Chunks {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.pop_front() {
                None => Ok(0),
                Some(Ok(bytes)) => {
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    Ok(bytes.len())
                }
                Some(Err(e)) => Err(e),
            }
        }
    }

    fn script(steps: Vec<std::io::Result<&[u8]>>) -> Chunks {
        Chunks(steps.into_iter().map(|s| s.map(<[u8]>::to_vec)).collect())
    }

    fn eof() -> std::io::Error {
        std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "peer closed connection without sending TLS close_notify",
        )
    }

    #[test]
    fn a_reply_ended_without_close_notify_is_read_when_whole_and_refused_when_cut() {
        let head = b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n";
        let interrupted = std::io::Error::new(std::io::ErrorKind::Interrupted, "signal");
        let raw = read_reply(&mut script(vec![
            Ok(head),
            Err(interrupted),
            Ok(b"true"),
            Err(eof()),
        ]))
        .unwrap();
        assert_eq!(parse_reply(&raw).unwrap().body, "true");
        let cut = read_reply(&mut script(vec![Ok(head), Ok(b"tr"), Err(eof())])).unwrap_err();
        assert_eq!(
            cut.kind(),
            std::io::ErrorKind::UnexpectedEof,
            "a truncation is not a reply"
        );
        let other = read_reply(&mut script(vec![Err(std::io::Error::other("reset"))])).unwrap_err();
        assert_eq!(other.to_string(), "reset");
        // a clean end is the end, whole or not, as the plain path always read it
        let raw = read_reply(&mut script(vec![Ok(b"HTTP/1.1 204 No Content\r\n\r\n")])).unwrap();
        assert_eq!(parse_reply(&raw).unwrap().status, 204);
    }

    #[test]
    fn a_reply_is_whole_when_its_length_is_there() {
        assert!(whole(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}"));
        assert!(whole(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
        assert!(!whole(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n{}"));
        assert!(
            !whole(b"HTTP/1.1 200 OK\r\n\r\n{}"),
            "no length, nothing says it is whole"
        );
        assert!(
            !whole(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n"),
            "no end of head"
        );
        assert!(!whole(b""));
    }

    #[test]
    fn a_reply_over_the_bound_is_refused_unread() {
        struct Endless;
        impl Read for Endless {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                buf.fill(b'x');
                Ok(buf.len())
            }
        }
        let err = read_reply(&mut Endless).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("byte bound"), "{err}");
    }

    // ------------------------------------------------------------ a TLS round trip, locally

    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    use rustls::{ServerConfig, ServerConnection};

    /// A certificate authority minted for one test, the roots that trust it, and a server
    /// configuration presenting a certificate it signed for `names`.
    fn authority(names: &[&str]) -> (RootCertStore, Arc<ServerConfig>) {
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let mut ca = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        ca.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca.distinguished_name
            .push(rcgen::DnType::CommonName, "majordomus test authority");
        ca.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::DigitalSignature,
        ];
        let ca_cert = ca.self_signed(&ca_key).unwrap();
        let issuer = rcgen::Issuer::new(ca, ca_key);
        let leaf_key = rcgen::KeyPair::generate().unwrap();
        let leaf =
            rcgen::CertificateParams::new(names.iter().map(|n| n.to_string()).collect::<Vec<_>>())
                .unwrap();
        let leaf_cert = leaf.signed_by(&leaf_key, &issuer).unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(ca_cert.der().clone()).unwrap();
        let server =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(
                    vec![leaf_cert.der().clone()],
                    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
                )
                .unwrap();
        (roots, Arc::new(server))
    }

    /// What a TLS server heard on one connection: the request, and the name SNI carried.
    #[derive(Debug, Default)]
    struct Heard {
        request: String,
        sni: Option<String>,
    }

    /// A TLS server on `bind` answering `connections` connections with one JSON body each,
    /// closing with `close_notify` or without. A connection whose handshake the client
    /// refused is heard as an empty request.
    fn tls_server(
        bind: &str,
        config: Arc<ServerConfig>,
        connections: usize,
        close_notify: bool,
    ) -> (SocketAddr, std::thread::JoinHandle<Vec<Heard>>) {
        let listener = std::net::TcpListener::bind(bind).unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut heard = Vec::new();
            for _ in 0..connections {
                let (socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(30)))
                    .unwrap();
                let mut tls =
                    StreamOwned::new(ServerConnection::new(config.clone()).unwrap(), socket);
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                while !whole(&raw) {
                    match tls.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                    }
                }
                let sni = tls.conn.server_name().map(str::to_string);
                if whole(&raw) {
                    let body = r#"{"accepted":true,"candidates":[]}"#;
                    let _ = tls.write_all(
                        format!("HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len())
                            .as_bytes(),
                    );
                    if close_notify {
                        tls.conn.send_close_notify();
                    }
                    let _ = tls.flush();
                }
                heard.push(Heard {
                    request: String::from_utf8_lossy(&raw).into_owned(),
                    sni,
                });
            }
            heard
        });
        (addr, server)
    }

    fn trusting(roots: RootCertStore) -> impl FnOnce() -> std::io::Result<Arc<ClientConfig>> {
        move || tls_config(roots).map_err(std::io::Error::other)
    }

    fn post_over(
        url: &str,
        tls: impl FnOnce() -> std::io::Result<Arc<ClientConfig>>,
    ) -> std::io::Result<Reply> {
        let timeout = Duration::from_secs(30);
        let (stream, host) = connect_with(url, timeout, tls)?;
        exchange(
            stream,
            &host,
            "POST",
            "/api/v1/mesh/register",
            &[],
            Some(r#"{"envelope":{}}"#),
            timeout,
        )
    }

    #[test]
    fn an_https_endpoint_is_reached_over_tls_with_a_verified_certificate() {
        for close_notify in [true, false] {
            let (roots, server) = authority(&["127.0.0.1"]);
            let (addr, heard) = tls_server("127.0.0.1:0", server, 1, close_notify);
            let url = format!("https://{addr}");
            let reply = post_over(&url, trusting(roots)).expect("answered over TLS");
            assert_eq!(reply.status, 200);
            assert_eq!(reply.body, r#"{"accepted":true,"candidates":[]}"#);
            let heard = heard.join().unwrap();
            let request = &heard[0].request;
            assert!(
                request.starts_with("POST /api/v1/mesh/register HTTP/1.1\r\n"),
                "{request}"
            );
            assert!(request.contains(&format!("Host: {addr}\r\n")), "{request}");
            assert!(
                request.ends_with(r#"{"envelope":{}}"#),
                "the same framing: {request}"
            );
            assert_eq!(
                heard[0].sni, None,
                "an IP address is verified, and never sent as SNI"
            );
        }
    }

    /// A host name is the name the certificate is checked against, and SNI carries it.
    #[test]
    fn a_host_name_is_sent_as_sni_and_checked_against_the_certificate() {
        let Some(local) = ("localhost", 0)
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
        else {
            return; // a machine without localhost has no name to test with
        };
        let (roots, server) = authority(&["localhost"]);
        let (addr, heard) = tls_server(&local.to_string(), server, 1, true);
        let reply = post_over(
            &format!("https://localhost:{}", addr.port()),
            trusting(roots),
        )
        .expect("answered over TLS");
        assert_eq!(reply.status, 200);
        let heard = heard.join().unwrap();
        assert_eq!(heard[0].sni.as_deref(), Some("localhost"));
        assert!(heard[0]
            .request
            .contains(&format!("Host: localhost:{}\r\n", addr.port())));
    }

    #[test]
    fn an_unverifiable_certificate_is_refused_naming_the_endpoint() {
        // signed by an authority the executable's roots do not hold: the production path
        let (_, server) = authority(&["127.0.0.1"]);
        let (addr, heard) = tls_server("127.0.0.1:0", server, 1, true);
        let url = format!("https://{addr}");
        let err = request(
            &url,
            "POST",
            "/api/v1/mesh/register",
            &[],
            Some("{}"),
            Duration::from_secs(30),
        )
        .expect_err("an unknown authority is refused");
        let text = err.to_string();
        assert!(
            text.contains(&format!("{url}: the TLS handshake failed")),
            "{text}"
        );
        assert!(text.contains("certificate"), "{text}");
        assert!(
            heard.join().unwrap()[0].request.is_empty(),
            "no byte of the request was sent"
        );

        // trusted, but issued for another name: the host name is verified too
        let (roots, server) = authority(&["hub.example"]);
        let (addr, heard) = tls_server("127.0.0.1:0", server, 1, true);
        let url = format!("https://{addr}");
        let err = post_over(&url, trusting(roots)).expect_err("a certificate for another name");
        let text = err.to_string();
        assert!(
            text.contains(&format!("{url}: the TLS handshake failed")),
            "{text}"
        );
        assert!(text.contains("certificate"), "{text}");
        assert!(heard.join().unwrap()[0].request.is_empty());
    }

    #[test]
    fn a_server_that_closes_during_the_handshake_is_a_failed_handshake() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || drop(listener.accept().unwrap()));
        let (roots, _) = authority(&["127.0.0.1"]);
        let err = post_over(&format!("https://{addr}"), trusting(roots)).expect_err("closed");
        assert!(
            err.to_string().contains("the TLS handshake failed"),
            "{err}"
        );
        server.join().unwrap();
    }

    #[test]
    fn a_host_no_certificate_can_name_is_refused_before_it_is_resolved() {
        let err = request(
            "https://bad..name:443",
            "GET",
            "/",
            &[],
            None,
            Duration::from_secs(30),
        )
        .expect_err("not a name");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(
            err.to_string()
                .contains("https://bad..name:443: 'bad..name' is not a name"),
            "{err}"
        );
    }

    #[test]
    fn the_executable_trusts_its_compiled_in_roots() {
        let config = default_tls().expect("the default configuration builds");
        assert!(Arc::ptr_eq(&config, &default_tls().unwrap()), "built once");
    }

    /// A socket and the endpoint it is connected to, with nothing written on it yet.
    fn connected() -> (std::net::TcpListener, TcpStream, Endpoint) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let socket = TcpStream::connect(addr).unwrap();
        (
            listener,
            socket,
            parse_endpoint(&format!("https://{addr}")).unwrap(),
        )
    }

    /// Every way a connection can fail before its first byte is its own error, never a panic
    /// or a request sent anyway: an endpoint that does not parse, a resolver that answers with
    /// no address, a TLS configuration that could not be had, a zero bound the platform
    /// refuses, and a configuration a connection cannot be made from.
    #[test]
    fn a_connection_that_cannot_be_made_fails_before_its_first_byte() {
        let never = || -> std::io::Result<Arc<ClientConfig>> { panic!("no TLS is asked for") };
        let scheme = connect_with("ftp://h", Duration::from_secs(1), never)
            .err()
            .unwrap();
        assert!(
            scheme.to_string().contains("neither http:// nor https://"),
            "{scheme}"
        );
        assert!(unbuildable(rustls::Error::General("x".into())).contains("could not be built"));
        let none = some_address(Vec::new(), "nowhere:1").expect_err("no address");
        assert_eq!(none.to_string(), "nowhere:1: no address");

        let (listener, _, endpoint) = connected();
        let err = connect_with(&endpoint.url(), Duration::from_secs(5), || {
            Err(std::io::Error::other("no configuration"))
        })
        .err()
        .expect("no configuration");
        assert_eq!(err.to_string(), "no configuration");
        drop(listener);

        let config = tls_config(RootCertStore::empty()).expect("an empty trust store builds");
        let (_listener, socket, endpoint) = connected();
        let name = server_name(&endpoint).unwrap();
        assert!(handshake(
            socket,
            &endpoint,
            name.clone(),
            config.clone(),
            Duration::ZERO
        )
        .is_err());
        let mut unusable = (*config).clone();
        unusable.max_fragment_size = Some(1);
        let (_listener, socket, endpoint) = connected();
        assert!(handshake(
            socket,
            &endpoint,
            name,
            Arc::new(unusable),
            Duration::from_secs(5)
        )
        .is_err());

        let (_listener, socket, _) = connected();
        let err = exchange(
            Stream::Plain(socket),
            "h",
            "GET",
            "/",
            &[],
            None,
            Duration::ZERO,
        )
        .expect_err("a zero bound");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    /// A stream that takes every byte and cannot flush them.
    struct Unflushable;

    impl Read for Unflushable {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Ok(0)
        }
    }

    impl Write for Unflushable {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("cannot flush"))
        }
    }

    /// A request whose bytes could not be flushed is that failure, and no reply is read.
    #[test]
    fn a_request_that_cannot_be_flushed_reads_no_reply() {
        let err = exchange_on(&mut Unflushable, "h", "GET", "/", &[], None).expect_err("unflushed");
        assert_eq!(err.to_string(), "cannot flush");
    }
}
