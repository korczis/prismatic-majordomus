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

use crate::peers::{ClientInfo, PeerAddress};

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
    write_request(stream, host, method, path, headers, body)?;
    let raw = read_reply(stream)?;
    parse_reply(&raw)
}

/// Write one request: the head this client always sends, the caller's headers, the body.
fn write_request<S: Write>(
    stream: &mut S,
    host: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> std::io::Result<()> {
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
    stream.flush()
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
    // connected and bounded in one step: a socket that cannot take its bound fails as a
    // connection that could not be made
    let (mut stream, host) = connect(base_url, first)
        .and_then(|(stream, host)| self::bound(stream.socket(), first).map(|()| (stream, host)))?;
    write_request(&mut stream, &host, method, path, headers, body)?;
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
        // a wait that cannot be set is an error like a read that failed, and is answered so
        let read = stream
            .socket()
            .set_read_timeout(Some(left.min(bound.idle)))
            .and_then(|()| stream.read(&mut chunk));
        let n = match read {
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
    /// Where the worker that started this bridge can be reached, read once from this
    /// process's environment and carried in every `initialize` it forwards, so that the
    /// board row of its session names it (I3956).
    address: Option<PeerAddress>,
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
            address: PeerAddress::of_this_process(),
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
        let stamped;
        let message = match &self.address {
            Some(address) if crate::http::mcp::is_initialize(message) => {
                stamped = stamp_address(message, address);
                &stamped
            }
            _ => message,
        };
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

    /// Keep the session alive, and keep this client on the board.
    ///
    /// A server that restarted at the same address answers the ping with 404: it forgot the
    /// session. The heartbeat re-opens it at once with the client's own initialize and
    /// replays every announcement ([`Bridge::reinitialize`]). Before I4251 the 404 was taken
    /// for a pong, so an idle client stayed off the new server's board until it next sent a
    /// message. On 2026-10-27 two sessions of an adopter repository were missing from the board for
    /// hours after a restart, and they worked on the same pull requests without seeing each
    /// other. A session that a failed re-attach left without an id is opened again on the
    /// next beat. A client that never initialized has nothing to keep.
    pub fn heartbeat(&mut self) -> Result<(), BridgeError> {
        if self.initialize.is_none() {
            return Ok(());
        }
        if self.session.is_none() {
            return self.reinitialize();
        }
        let ping =
            json!({ "jsonrpc": "2.0", "id": "majordomus-bridge-heartbeat", "method": "ping" });
        match self.send(&ping)? {
            Sent::Answer(_) => Ok(()),
            Sent::SessionLost => self.reinitialize(),
        }
    }

    /// Follow the repository's server to where it serves now: when `serving` (the lease's
    /// answering server, read without electing) names another address than this bridge's,
    /// move there and re-open the session. `Ok(true)` when it moved. What a heartbeat does
    /// when its server is gone from its address; nobody serving is left to the next
    /// message's election, which may make this process the server.
    ///
    /// ```
    /// use majordomus_cli::mcp::bridge::Bridge;
    /// let mut bridge = Bridge::new("http://127.0.0.1:9".into());
    /// // nobody serves, or the server is still where the bridge sends: nothing to follow
    /// assert!(!bridge.follow(None).unwrap());
    /// assert!(!bridge.follow(Some("http://127.0.0.1:9".into())).unwrap());
    /// // a client that never initialized has no session to carry anywhere
    /// assert!(!bridge.follow(Some("http://127.0.0.1:10".into())).unwrap());
    /// assert_eq!(bridge.url(), "http://127.0.0.1:9");
    /// ```
    pub fn follow(&mut self, serving: Option<String>) -> Result<bool, BridgeError> {
        match serving {
            Some(url) if url != self.url && self.initialize.is_some() => {
                self.move_to(url);
                self.reinitialize()?;
                Ok(true)
            }
            _ => Ok(false),
        }
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

/// The message with the worker's address in every `initialize` it holds (I3956).
fn stamp_address(message: &Value, address: &PeerAddress) -> Value {
    let mut out = message.clone();
    match &mut out {
        Value::Array(batch) => batch
            .iter_mut()
            .filter(|m| m["method"] == "initialize")
            .for_each(|m| crate::peers::with_address(m, address)),
        m => crate::peers::with_address(m, address),
    }
    out
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

    /// Every `initialize` a bridge forwards carries the address of the worker that started
    /// it, in a batch as alone; no other message is touched (I3956).
    #[test]
    fn a_bridge_stamps_its_workers_address_on_every_initialize() {
        let a = PeerAddress::from_env(|_| None, Some(31), "mac").unwrap();
        let one = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "clientInfo": { "name": "c", "version": "1" } } });
        let stamped = stamp_address(&one, &a);
        assert_eq!(
            PeerAddress::from_initialize(&stamped["params"]),
            Some(a.clone())
        );
        assert_eq!(stamped["params"]["clientInfo"]["name"], "c");
        let batch = json!([{ "method": "ping", "params": {} }, one]);
        let stamped = stamp_address(&batch, &a);
        assert!(
            stamped[0]["params"].get("_meta").is_none(),
            "a ping is not touched"
        );
        assert_eq!(PeerAddress::from_initialize(&stamped[1]["params"]), Some(a));
    }

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

    /// A server that restarted at the same address forgets the session and answers the next
    /// ping 404. The heartbeat re-attaches on that beat, with the client's own initialize and
    /// its announcement, so that an idle client is back on the board without sending a
    /// message. Before I4251 the 404 counted as a pong; two sessions of an adopter repository
    /// stayed off the board for hours after a restart on 2026-10-27.
    #[test]
    fn a_heartbeat_the_server_forgot_reattaches_and_replays_the_announcement() {
        let answer = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18"}}"#;
        let announced = r#"{"jsonrpc":"2.0","id":3,"result":{"isError":false}}"#;
        let (url, server) = scripted(vec![
            (200, answer, Some("s-1")),
            (200, announced, None),
            // the server restarted: the old session is unknown
            (404, "", None),
            (200, answer, Some("s-2")),
            (202, "", None),
            (200, announced, None),
        ]);
        let mut bridge = Bridge::with_timeout(url, Duration::from_secs(30));
        bridge.handle(&initialize()).expect("initialized");
        let announce = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": crate::capability::builtin::peers::ANNOUNCE_TOOL,
            "arguments": { "intent": "I4251: the brief", "scope": ["docs"] } } });
        bridge.handle(&announce).expect("announced");
        bridge.heartbeat().expect("the beat re-attached");
        // asked before the server is joined: a bridge that took the 404 for a pong would
        // leave the scripted server waiting for an initialize that never comes
        assert_eq!(
            bridge.session.as_deref(),
            Some("s-2"),
            "a new session was opened"
        );
        let seen = server.join().unwrap();
        let methods: Vec<&str> = seen
            .iter()
            .map(|s| s["message"]["method"].as_str().unwrap_or_default())
            .collect();
        assert_eq!(
            methods,
            vec![
                "initialize",
                "tools/call",
                "ping",
                "initialize",
                "notifications/initialized",
                "tools/call"
            ]
        );
        assert_eq!(
            seen[2]["session"], "s-1",
            "the ping carried the forgotten session"
        );
        assert_eq!(
            seen[3]["message"]["params"]["clientInfo"]["name"],
            "bridge-test"
        );
        assert_eq!(
            seen[5]["session"], "s-2",
            "the announcement is replayed on the new one"
        );
        assert_eq!(
            seen[5]["message"]["params"]["arguments"]["intent"],
            "I4251: the brief"
        );
    }

    /// A beat with no session after a re-attach that failed opens one; a client that never
    /// initialized sends nothing at all.
    #[test]
    fn a_heartbeat_without_a_session_opens_one_and_without_an_initialize_sends_nothing() {
        let mut idle = Bridge::with_timeout("http://127.0.0.1:9".into(), Duration::from_secs(1));
        idle.heartbeat().expect("nothing to keep, nothing sent");

        let answer = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18"}}"#;
        let (url, server) = scripted(vec![
            (200, answer, Some("s-1")),
            (200, answer, Some("s-2")),
            (202, "", None),
        ]);
        let mut bridge = Bridge::with_timeout(url, Duration::from_secs(30));
        bridge.handle(&initialize()).expect("initialized");
        bridge.session = None;
        bridge.heartbeat().expect("opened again");
        let seen = server.join().unwrap();
        assert_eq!(seen[1]["message"]["method"], "initialize");
        assert_eq!(bridge.session.as_deref(), Some("s-2"));
    }

    /// A server that came back on another address is followed there by the heartbeat.
    #[test]
    fn a_bridge_follows_the_server_to_its_new_address() {
        let answer = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18"}}"#;
        let (old, first) = scripted(vec![(200, answer, Some("s-1"))]);
        let (new, second) = scripted(vec![(200, answer, Some("s-7")), (202, "", None)]);
        let mut bridge = Bridge::with_timeout(old.clone(), Duration::from_secs(30));
        bridge.handle(&initialize()).expect("initialized");
        first.join().unwrap();
        assert!(
            !bridge.follow(Some(old.clone())).unwrap(),
            "still where it was"
        );
        assert!(bridge.follow(Some(new.clone())).expect("moved"));
        assert_eq!(bridge.url(), new);
        assert_eq!(bridge.session.as_deref(), Some("s-7"));
        let seen = second.join().unwrap();
        assert_eq!(seen[0]["message"]["method"], "initialize");
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

    /// A bounded reply is asked over the same transport as any other: cleartext to a public
    /// address is refused before a byte is sent, not read within its bound (ADR 0220).
    #[test]
    fn a_bounded_reply_is_refused_in_cleartext_to_a_public_address() {
        let started = std::time::Instant::now();
        let error = request_json_within(
            "http://203.0.113.5:8791",
            "GET",
            "/",
            &[],
            None,
            within(1024),
        )
        .expect_err("refused");
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied,
            "{error}"
        );
        assert!(error.to_string().contains("use https://"), "{error}");
        assert_eq!(
            ReplyRefused::of(&error),
            None,
            "a refused transport is no refused reply"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "no connection was attempted"
        );
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

    /// A bound of 1 KiB with the two waits given.
    fn waits(idle: Duration, within: Duration) -> ReplyBound {
        ReplyBound {
            bytes: 1024,
            idle,
            within,
        }
    }

    /// A one-connection server that takes the connection and closes it with nothing read.
    fn closing() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || drop(listener.accept()));
        url
    }

    /// A server that holds the connection and says nothing until the client lets it go.
    fn silent() -> String {
        answering(|s| {
            let mut rest = [0u8; 1];
            let _ = s.read(&mut rest);
        })
    }

    #[test]
    fn a_request_the_peer_stops_taking_fails_in_the_write_and_is_no_refusal() {
        // far more than the socket buffers of both ends hold: the write meets the close
        let body = " ".repeat(32 * 1024 * 1024);
        let error = request(
            &closing(),
            "POST",
            "/",
            &[],
            Some(&body),
            Duration::from_secs(30),
        )
        .expect_err("the peer closed before the request was written");
        assert_eq!(ReplyRefused::of(&error), None, "{error}");

        let bound = waits(Duration::from_secs(30), Duration::from_secs(60));
        let error = request_json_within(&closing(), "POST", "/", &[], Some(&body), bound)
            .expect_err("the peer closed before the request was written");
        assert_eq!(ReplyRefused::of(&error), None, "{error}");
    }

    #[test]
    fn a_reply_that_never_pauses_is_refused_late_once_the_whole_is_due() {
        // whitespace costs no bytes of the bound, so only the deadline can end this reply;
        // every read finds bytes waiting, so it ends at the deadline, not at a read's wait
        let url = answering(|s| {
            if s.write_all(b"HTTP/1.1 200 OK\r\n\r\n{").is_err() {
                return;
            }
            let pad = vec![b' '; 64 * 1024];
            while s.write_all(&pad).is_ok() {}
        });
        let bound = waits(Duration::from_secs(60), Duration::from_millis(200));
        let error = request_json_within(&url, "GET", "/", &[], None, bound)
            .expect_err("a reply that never ends");
        assert_eq!(
            ReplyRefused::of(&error),
            Some(&ReplyRefused::Late { millis: 200 })
        );
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }

    #[test]
    fn a_silent_peer_is_refused_late_when_the_deadline_is_the_nearer_wait() {
        let bound = waits(Duration::from_secs(60), Duration::from_millis(300));
        let error = request_json_within(&silent(), "GET", "/", &[], None, bound)
            .expect_err("nothing arrived");
        assert_eq!(
            ReplyRefused::of(&error),
            Some(&ReplyRefused::Late { millis: 300 })
        );
    }

    #[test]
    fn a_silent_peer_past_its_idle_wait_is_a_timeout_and_no_refusal() {
        let bound = waits(Duration::from_millis(300), Duration::from_secs(60));
        let error = request_json_within(&silent(), "GET", "/", &[], None, bound)
            .expect_err("nothing arrived");
        assert_eq!(ReplyRefused::of(&error), None, "{error}");
        assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_peer_that_resets_inside_the_deadline_wait_is_its_error_and_no_refusal() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // one byte read and the rest of the request left unread: the close is a reset
            let mut one = [0u8; 1];
            let _ = stream.read_exact(&mut one);
        });
        let bound = waits(Duration::from_secs(60), Duration::from_secs(30));
        let error = request_json_within(&url, "GET", "/", &[], None, bound)
            .expect_err("the peer reset the connection");
        assert_eq!(ReplyRefused::of(&error), None, "{error}");
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::ConnectionReset,
            "{error:?}"
        );
    }

    #[test]
    fn a_reply_without_a_length_ends_where_the_peer_closes() {
        let url = answering(|s| {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\n\r\n{ \"a\": 1 }");
        });
        let reply = request_json_within(&url, "GET", "/", &[], None, within(1024))
            .expect("a reply closed after its body is read");
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, r#"{"a":1}"#);
    }

    #[test]
    fn a_reply_closed_inside_its_head_is_malformed() {
        let url = answering(|s| {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\n");
        });
        let error = request_json_within(&url, "GET", "/", &[], None, within(1024))
            .expect_err("no end of the head");
        assert_eq!(ReplyRefused::of(&error), None);
        assert!(error.to_string().contains("no header end"), "{error}");
    }

    #[test]
    fn a_reply_without_a_status_line_is_malformed_to_either_reader() {
        let serve = |s: &mut TcpStream| {
            let _ = s.write_all(b"garbage\r\n\r\n{}");
        };
        let error = request_json_within(&answering(serve), "GET", "/", &[], None, within(1024))
            .expect_err("no status line");
        assert!(error.to_string().contains("no status line"), "{error}");
        let error = request(
            &answering(serve),
            "GET",
            "/",
            &[],
            None,
            Duration::from_secs(30),
        )
        .expect_err("no status line");
        assert!(error.to_string().contains("no status line"), "{error}");
    }
}
