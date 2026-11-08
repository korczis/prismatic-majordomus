//! A mesh link over QUIC (ADR 0233): the same signed Hello, Welcome, Sync and SyncAnswer as
//! the HTTP link, one bidirectional stream per exchange, on a connection encrypted to the two
//! runtimes' node keys and kept open between rounds.
//!
//! Nothing about the link protocol changes here. A runtime's TLS identity is a self-signed
//! certificate over its Ed25519 node key ([`NodeIdentity::tls_certificate`]); a certificate is
//! accepted for the key inside it and for nothing else, so no certificate authority and no web
//! PKI are consulted. Two bindings tie the channel to the protocol:
//!
//! - a dialer that knows whom it is calling (a hello addressed with `to`) refuses, during the
//!   handshake, a peer whose key is not that node's, before a link message is read; a dialer
//!   that has reached an address before pins the node it found there;
//! - every message must be signed by the key that holds the connection: the answerer refuses
//!   a request signed by any other key as `signature`, and the dialer treats a reply signed by
//!   any other key as a transport failure, so a relay in the middle with a key of its own can
//!   neither answer nor be answered.
//!
//! [`SelectingTransport`] is what a runtime dials with: a plain `host:port` is tried over QUIC
//! on the same port number (UDP and TCP ports are separate), and over HTTP when QUIC does not
//! answer there; an address that did not answer over QUIC is not tried again for
//! [`QUIC_RETRY_AFTER`]. Which path each endpoint took, and how long its last round took, is
//! [`SelectingTransport::paths`]. The async runtime quinn needs is one tokio runtime owned by
//! each [`QuicNode`], so the rest of the crate stays synchronous.
//!
//! ```
//! use std::sync::Arc;
//! use majordomus_cli::mesh::identity::NodeIdentity;
//! use majordomus_cli::mesh::link::{sign, Domain, LinkReply, LinkTransport, HELLO_PATH};
//! use majordomus_cli::mesh::quic::QuicNode;
//!
//! // an answerer that welcomes every hello with an empty signed body
//! let answerer = Arc::new(NodeIdentity::ephemeral().unwrap());
//! let signer = Arc::clone(&answerer);
//! let server = QuicNode::bind(&answerer, "127.0.0.1:0".parse().unwrap(), Some(Arc::new(
//!     move |_path: &str, _message: &_| LinkReply::accepted(sign(&signer, Domain::Welcome, serde_json::json!({}))),
//! ))).unwrap();
//!
//! let dialer = NodeIdentity::ephemeral().unwrap();
//! let client = QuicNode::bind(&dialer, "127.0.0.1:0".parse().unwrap(), None).unwrap();
//! let hello = sign(&dialer, Domain::Hello, serde_json::json!({}));
//! let reply = client.post(&server.endpoint(), HELLO_PATH, &hello).unwrap();
//! assert!(reply.signed.is_some(), "a welcome came back over QUIC");
//! ```

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls13_signature, CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::identity::{node_id_of_key, NodeIdentity};
use super::link::{
    verify_signed, Domain, HttpTransport, LinkReply, LinkTransport, RefusalCode, Signed,
    HELLO_PATH, LINK_REPLY_WITHIN, MAX_LINK_MESSAGE, MAX_LINK_REPLY, REQUEST_TIMEOUT, SYNC_PATH,
};

/// The scheme an endpoint is written with when it names QUIC explicitly. A plain `host:port`
/// is tried over QUIC as well, so the scheme only forces the path.
///
/// ```
/// assert_eq!(majordomus_cli::mesh::quic::SCHEME, "quic://");
/// ```
pub const SCHEME: &str = "quic://";

/// The name a QUIC link's certificate is issued to and a dialer asks for. It identifies
/// nothing: the node key inside the certificate does.
///
/// ```
/// assert_eq!(majordomus_cli::mesh::quic::SERVER_NAME, "majordomus-mesh");
/// ```
pub const SERVER_NAME: &str = "majordomus-mesh";

/// The application protocol both ends agree on in the handshake. A peer speaking anything
/// else is refused by TLS before a stream opens.
///
/// ```
/// assert!(majordomus_cli::mesh::quic::ALPN.ends_with(b"/1"));
/// ```
pub const ALPN: &[u8] = b"majordomus-mesh-link/1";

/// How long a dialer waits for a QUIC handshake before it falls back to HTTP. Short, because
/// a peer that does not speak QUIC never answers at all, and the fallback pays this once per
/// [`QUIC_RETRY_AFTER`].
///
/// ```
/// assert!(majordomus_cli::mesh::quic::QUIC_HANDSHAKE_WITHIN.as_secs() <= 5);
/// ```
pub const QUIC_HANDSHAKE_WITHIN: Duration = Duration::from_secs(2);

/// How long an address that did not answer over QUIC is reached over HTTP alone before QUIC
/// is tried there again.
///
/// ```
/// assert_eq!(majordomus_cli::mesh::quic::QUIC_RETRY_AFTER.as_secs(), 600);
/// ```
pub const QUIC_RETRY_AFTER: Duration = Duration::from_secs(600);

/// The most connections a QUIC listener holds at once; one more is refused at the handshake.
const MAX_CONNECTIONS: usize = 64;

/// The most link exchanges in flight on one connection.
const MAX_STREAMS: u32 = 8;

/// A request is a link message and its route: the message bound plus room for the route.
const MAX_REQUEST: usize = MAX_LINK_MESSAGE + 256;

/// A connection nobody uses closes after this long; the dialer's keep-alive holds an idle
/// one open below it, so the connection is there for the next round and for the answerer.
const IDLE_MILLIS: u32 = 60_000;
const KEEP_ALIVE: Duration = Duration::from_secs(15);

/// What answers a link message: the route and the signed message in, the reply out. The
/// shared server's is the cooperation's `accept_hello` / `accept_sync`.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::link::{LinkReply, RefusalCode};
/// use majordomus_cli::mesh::quic::LinkHandler;
///
/// let handler: LinkHandler = Arc::new(|_path, _message| LinkReply::refused(RefusalCode::NotActive, "off"));
/// let message = majordomus_cli::mesh::link::Signed { body: serde_json::json!({}), sig: String::new() };
/// assert!(handler("/api/v1/mesh/link/hello", &message).refusal.is_some());
/// ```
pub type LinkHandler = Arc<dyn Fn(&str, &Signed) -> LinkReply + Send + Sync>;

/// One exchange on the wire: the route the HTTP link would have posted to, and the message.
#[derive(Serialize, Deserialize)]
struct Wire {
    path: String,
    message: Signed,
}

/// The Ed25519 public key (64 hex) a mesh certificate carries, found by its
/// SubjectPublicKeyInfo; `None` for a certificate over any other kind of key.
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::quic::node_key_of_certificate;
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let (certificate, _) = node.tls_certificate().unwrap();
/// assert_eq!(node_key_of_certificate(&certificate).unwrap(), node.public.public_key);
/// assert_eq!(node_key_of_certificate(b"not a certificate"), None);
/// ```
pub fn node_key_of_certificate(der: &[u8]) -> Option<String> {
    // SEQUENCE { SEQUENCE { OID 1.3.101.112 }, BIT STRING (0 unused bits, 32 bytes) }
    const SPKI: [u8; 12] = [
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
    ];
    let at = der.windows(SPKI.len()).position(|w| w == SPKI)? + SPKI.len();
    let key = der.get(at..at + 32)?;
    Some(key.iter().map(|b| format!("{b:02x}")).collect())
}

fn node_of(key: &str) -> String {
    node_id_of_key(key)
        .map(|n| n.as_str().to_string())
        .unwrap_or_default()
}

/// The node a hello is addressed to: the first 32 hex of the runtime key in its signed `to`.
fn addressed_node(message: &Signed) -> Option<String> {
    let to = message.body.get("to")?.as_str()?;
    let node = to.split('-').next()?;
    (node.len() == 32).then(|| node.to_string())
}

fn request_domain(path: &str) -> Option<Domain> {
    match path {
        HELLO_PATH => Some(Domain::Hello),
        SYNC_PATH => Some(Domain::SyncRequest),
        _ => None,
    }
}

fn reply_domain(path: &str) -> Option<Domain> {
    match path {
        HELLO_PATH => Some(Domain::Welcome),
        SYNC_PATH => Some(Domain::SyncAnswer),
        _ => None,
    }
}

/// Accepts a certificate for the Ed25519 node key inside it, and, when a node is expected,
/// for that node's key alone.
#[derive(Debug)]
struct NodeKeys {
    expected: Option<String>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl NodeKeys {
    fn judge(&self, end_entity: &CertificateDer<'_>) -> Result<(), rustls::Error> {
        let key = node_key_of_certificate(end_entity.as_ref()).ok_or_else(|| {
            rustls::Error::General("the peer's certificate carries no Ed25519 node key".into())
        })?;
        if let Some(expected) = &self.expected {
            let node = node_of(&key);
            if &node != expected {
                return Err(rustls::Error::General(format!(
                    "expected node {expected}, the peer holds node {node}"
                )));
            }
        }
        Ok(())
    }

    fn tls12() -> Result<HandshakeSignatureValid, rustls::Error> {
        Err(rustls::Error::General("a mesh link speaks TLS 1.3 only".into()))
    }
}

impl ServerCertVerifier for NodeKeys {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        self.judge(end_entity)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Self::tls12()
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

impl ClientCertVerifier for NodeKeys {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        self.judge(end_entity)?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Self::tls12()
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::ED25519]
    }
}

fn transport_config() -> quinn::TransportConfig {
    let mut transport = quinn::TransportConfig::default();
    transport
        .max_concurrent_bidi_streams(quinn::VarInt::from_u32(MAX_STREAMS))
        .max_concurrent_uni_streams(quinn::VarInt::from_u32(0))
        .max_idle_timeout(Some(quinn::IdleTimeout::from(quinn::VarInt::from_u32(
            IDLE_MILLIS,
        ))))
        .keep_alive_interval(Some(KEEP_ALIVE));
    transport
}

/// The certificate and key a node presents, and the provider both sides verify with.
struct Credentials {
    certificate: Vec<u8>,
    key: Vec<u8>,
    provider: Arc<CryptoProvider>,
}

impl Credentials {
    fn chain(&self) -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
        (
            vec![CertificateDer::from(self.certificate.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone())),
        )
    }

    fn verifier(&self, expected: Option<String>) -> Arc<NodeKeys> {
        Arc::new(NodeKeys {
            expected,
            algorithms: self.provider.signature_verification_algorithms,
        })
    }

    fn server(&self) -> Result<quinn::ServerConfig, String> {
        let (chain, key) = self.chain();
        let mut tls = rustls::ServerConfig::builder_with_provider(Arc::clone(&self.provider))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| e.to_string())?
            .with_client_cert_verifier(self.verifier(None))
            .with_single_cert(chain, key)
            .map_err(|e| e.to_string())?;
        tls.alpn_protocols = vec![ALPN.to_vec()];
        let crypto = QuicServerConfig::try_from(tls).map_err(|e| e.to_string())?;
        let mut config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        config.transport_config(Arc::new(transport_config()));
        Ok(config)
    }

    fn client(&self, expected: Option<String>) -> Result<quinn::ClientConfig, String> {
        let (chain, key) = self.chain();
        let mut tls = rustls::ClientConfig::builder_with_provider(Arc::clone(&self.provider))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| e.to_string())?
            .dangerous()
            .with_custom_certificate_verifier(self.verifier(expected))
            .with_client_auth_cert(chain, key)
            .map_err(|e| e.to_string())?;
        tls.alpn_protocols = vec![ALPN.to_vec()];
        let crypto = QuicClientConfig::try_from(tls).map_err(|e| e.to_string())?;
        let mut config = quinn::ClientConfig::new(Arc::new(crypto));
        config.transport_config(Arc::new(transport_config()));
        Ok(config)
    }
}

/// The node key a connection's peer proved in its handshake.
fn peer_key(connection: &quinn::Connection) -> Option<String> {
    let identity = connection.peer_identity()?;
    let chain = identity.downcast::<Vec<CertificateDer<'static>>>().ok()?;
    node_key_of_certificate(chain.first()?.as_ref())
}

/// An open connection to an address and the node key that holds it.
struct Held {
    connection: quinn::Connection,
    key: String,
}

/// One QUIC endpoint: a UDP socket that dials, and answers when it was bound with a
/// [`LinkHandler`], on one tokio runtime of its own. Connections it dialled stay open and
/// are reused for the next round to the same address.
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::quic::QuicNode;
///
/// let node = QuicNode::bind(&NodeIdentity::ephemeral().unwrap(), "127.0.0.1:0".parse().unwrap(), None).unwrap();
/// assert!(node.endpoint().starts_with("quic://127.0.0.1:"));
/// assert!(!node.answers(), "bound without a handler, it only dials");
/// ```
pub struct QuicNode {
    runtime: Option<tokio::runtime::Runtime>,
    endpoint: quinn::Endpoint,
    local: SocketAddr,
    answers: bool,
    credentials: Credentials,
    held: Mutex<HashMap<SocketAddr, Held>>,
}

impl QuicNode {
    /// Bind a QUIC endpoint at `address` with `identity`'s node key as its TLS identity.
    /// With a `handler` it also answers link messages; without one it only dials.
    ///
    /// ```
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::quic::QuicNode;
    ///
    /// let node = NodeIdentity::ephemeral().unwrap();
    /// let first = QuicNode::bind(&node, "127.0.0.1:0".parse().unwrap(), None).unwrap();
    /// // the same port twice is a bind failure that names the address
    /// let error = QuicNode::bind(&node, first.local_addr(), None).err().unwrap();
    /// assert!(error.contains(&first.local_addr().to_string()), "{error}");
    /// ```
    pub fn bind(
        identity: &NodeIdentity,
        address: SocketAddr,
        handler: Option<LinkHandler>,
    ) -> Result<Arc<Self>, String> {
        let (certificate, key) = identity.tls_certificate().map_err(|e| e.to_string())?;
        let credentials = Credentials {
            certificate,
            key,
            provider: Arc::new(rustls::crypto::ring::default_provider()),
        };
        let server = match handler {
            Some(_) => Some(credentials.server()?),
            None => None,
        };
        let socket = UdpSocket::bind(address).map_err(|e| format!("udp {address}: {e}"))?;
        let local = socket
            .local_addr()
            .map_err(|e| format!("udp {address}: {e}"))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(4)
            .thread_name("mesh-quic")
            .enable_all()
            .build()
            .map_err(|e| format!("the QUIC runtime: {e}"))?;
        let endpoint = {
            let _entered = runtime.enter();
            quinn::Endpoint::new(
                quinn::EndpointConfig::default(),
                server,
                socket,
                Arc::new(quinn::TokioRuntime),
            )
            .map_err(|e| format!("the QUIC endpoint at {local}: {e}"))?
        };
        let answers = handler.is_some();
        if let Some(handler) = handler {
            runtime.spawn(serve(endpoint.clone(), handler));
        }
        Ok(Arc::new(QuicNode {
            runtime: Some(runtime),
            endpoint,
            local,
            answers,
            credentials,
            held: Mutex::new(HashMap::new()),
        }))
    }

    /// The UDP address this endpoint is bound to.
    ///
    /// ```
    /// # use majordomus_cli::mesh::{identity::NodeIdentity, quic::QuicNode};
    /// let node = QuicNode::bind(&NodeIdentity::ephemeral().unwrap(), "127.0.0.1:0".parse().unwrap(), None).unwrap();
    /// assert_ne!(node.local_addr().port(), 0, "the port the system chose");
    /// ```
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// This endpoint as a dialer writes it: `quic://host:port`.
    ///
    /// ```
    /// # use majordomus_cli::mesh::{identity::NodeIdentity, quic::QuicNode};
    /// let node = QuicNode::bind(&NodeIdentity::ephemeral().unwrap(), "127.0.0.1:0".parse().unwrap(), None).unwrap();
    /// assert_eq!(node.endpoint(), format!("quic://{}", node.local_addr()));
    /// ```
    pub fn endpoint(&self) -> String {
        format!("{SCHEME}{}", self.local)
    }

    /// Whether this endpoint answers link messages, or only dials.
    ///
    /// ```
    /// # use majordomus_cli::mesh::{identity::NodeIdentity, quic::QuicNode};
    /// let node = QuicNode::bind(&NodeIdentity::ephemeral().unwrap(), "127.0.0.1:0".parse().unwrap(), None).unwrap();
    /// assert!(!node.answers());
    /// ```
    pub fn answers(&self) -> bool {
        self.answers
    }

    async fn connection(
        &self,
        address: SocketAddr,
        expected: Option<String>,
    ) -> Result<(quinn::Connection, String), String> {
        let held = self.held.lock().ok().and_then(|held| {
            held.get(&address)
                .map(|h| (h.connection.clone(), h.key.clone()))
        });
        let pinned = match (&held, expected) {
            (_, Some(expected)) => Some(expected),
            (Some((_, key)), None) => Some(node_of(key)),
            (None, None) => None,
        };
        if let Some((connection, key)) = held {
            let fits = pinned.as_deref().is_none_or(|node| node_of(&key) == node);
            if connection.close_reason().is_none() && fits {
                return Ok((connection, key));
            }
        }
        let config = self.credentials.client(pinned)?;
        let connecting = self
            .endpoint
            .connect_with(config, address, SERVER_NAME)
            .map_err(|e| e.to_string())?;
        let connection = tokio::time::timeout(QUIC_HANDSHAKE_WITHIN, connecting)
            .await
            .map_err(|_| {
                format!(
                    "no QUIC handshake within {}s",
                    QUIC_HANDSHAKE_WITHIN.as_secs()
                )
            })?
            .map_err(|e| e.to_string())?;
        let key = peer_key(&connection).ok_or("the peer proved no node key")?;
        if let Ok(mut held) = self.held.lock() {
            held.insert(
                address,
                Held {
                    connection: connection.clone(),
                    key: key.clone(),
                },
            );
        }
        Ok((connection, key))
    }

    async fn exchange(
        &self,
        address: SocketAddr,
        expected: Option<String>,
        request: &[u8],
    ) -> Result<(LinkReply, String), String> {
        let (connection, key) = self.connection(address, expected).await?;
        let (mut send, mut recv) = tokio::time::timeout(REQUEST_TIMEOUT, connection.open_bi())
            .await
            .map_err(|_| "no stream within the request timeout".to_string())?
            .map_err(|e| e.to_string())?;
        send.write_all(request).await.map_err(|e| e.to_string())?;
        send.finish().map_err(|e| e.to_string())?;
        let bytes = recv.read_to_end(MAX_LINK_REPLY).await.map_err(|e| match e {
            quinn::ReadToEndError::TooLong => {
                format!("a link reply exceeds {MAX_LINK_REPLY} bytes")
            }
            e => e.to_string(),
        })?;
        let reply: LinkReply =
            serde_json::from_slice(&bytes).map_err(|e| format!("not a link reply: {e}"))?;
        Ok((reply, key))
    }
}

impl LinkTransport for QuicNode {
    fn post(&self, endpoint: &str, path: &str, message: &Signed) -> Result<LinkReply, String> {
        let size = serde_json::to_string(message)
            .map_err(|e| e.to_string())?
            .len();
        if size > MAX_LINK_MESSAGE {
            return Err(format!(
                "a link message of {size} bytes exceeds {MAX_LINK_MESSAGE}"
            ));
        }
        let reply_domain =
            reply_domain(path).ok_or_else(|| format!("{endpoint}{path}: not a link route"))?;
        let authority = endpoint.strip_prefix(SCHEME).unwrap_or(endpoint);
        let address = authority
            .to_socket_addrs()
            .map_err(|e| format!("{endpoint}: {e}"))?
            .next()
            .ok_or_else(|| format!("{endpoint}: no address"))?;
        let request = serde_json::to_vec(&Wire {
            path: path.to_string(),
            message: message.clone(),
        })
        .map_err(|e| e.to_string())?;
        let expected = if path == HELLO_PATH {
            addressed_node(message)
        } else {
            None
        };
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| format!("{endpoint}: the QUIC runtime has stopped"))?;
        let (reply, key) = runtime
            .block_on(async {
                tokio::time::timeout(
                    LINK_REPLY_WITHIN,
                    self.exchange(address, expected, &request),
                )
                .await
                .map_err(|_| {
                    format!("no whole reply within {}s", LINK_REPLY_WITHIN.as_secs())
                })?
            })
            .map_err(|e| format!("{endpoint}{path}: {e}"))?;
        if let Some(signed) = &reply.signed {
            if !verify_signed(&key, reply_domain, signed) {
                return Err(format!(
                    "{endpoint}{path}: the reply is not signed by the key that holds the connection"
                ));
            }
        }
        Ok(reply)
    }
}

impl Drop for QuicNode {
    fn drop(&mut self) {
        self.endpoint.close(quinn::VarInt::from_u32(0), b"gone");
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

async fn serve(endpoint: quinn::Endpoint, handler: LinkHandler) {
    let open = Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));
    while let Some(incoming) = endpoint.accept().await {
        let Ok(permit) = Arc::clone(&open).try_acquire_owned() else {
            incoming.refuse();
            continue;
        };
        let handler = Arc::clone(&handler);
        tokio::spawn(async move {
            let _permit = permit;
            if let Ok(connection) = incoming.await {
                answer(connection, handler).await;
            }
        });
    }
}

async fn answer(connection: quinn::Connection, handler: LinkHandler) {
    let Some(peer) = peer_key(&connection) else {
        connection.close(quinn::VarInt::from_u32(1), b"no node key");
        return;
    };
    while let Ok((send, recv)) = connection.accept_bi().await {
        let (handler, peer) = (Arc::clone(&handler), peer.clone());
        tokio::spawn(async move {
            let _ = answer_one(send, recv, peer, handler).await;
        });
    }
}

async fn answer_one(
    mut send: quinn::SendStream,
    mut recv: quinn::RecvStream,
    peer: String,
    handler: LinkHandler,
) -> Result<(), String> {
    let bytes = tokio::time::timeout(LINK_REPLY_WITHIN, recv.read_to_end(MAX_REQUEST))
        .await
        .map_err(|_| "the request did not arrive in time".to_string())?;
    let reply = match bytes {
        Err(quinn::ReadToEndError::TooLong) => LinkReply::refused(
            RefusalCode::Oversized,
            format!("a link request exceeds {MAX_REQUEST} bytes"),
        ),
        Err(e) => return Err(e.to_string()),
        Ok(bytes) => match serde_json::from_slice::<Wire>(&bytes) {
            Err(e) => LinkReply::refused(RefusalCode::Malformed, format!("not a link request: {e}")),
            Ok(wire) => match request_domain(&wire.path) {
                None => LinkReply::refused(
                    RefusalCode::Malformed,
                    format!("{} is not a link route", wire.path),
                ),
                Some(domain) if !verify_signed(&peer, domain, &wire.message) => {
                    LinkReply::refused(
                        RefusalCode::Signature,
                        "the message is not signed by the key that holds this connection",
                    )
                }
                Some(_) => {
                    let Wire { path, message } = wire;
                    tokio::task::spawn_blocking(move || handler(&path, &message))
                        .await
                        .unwrap_or_else(|e| {
                            LinkReply::refused(
                                RefusalCode::Capacity,
                                format!("the answer did not complete: {e}"),
                            )
                        })
                }
            },
        },
    };
    let body = serde_json::to_vec(&reply).map_err(|e| e.to_string())?;
    send.write_all(&body).await.map_err(|e| e.to_string())?;
    send.finish().map_err(|e| e.to_string())?;
    let _ = send.stopped().await;
    Ok(())
}

/// The path one endpoint's last exchange took, for `mesh status` and route choice.
///
/// ```
/// use majordomus_cli::mesh::quic::LinkPath;
///
/// let path = LinkPath { endpoint: "10.0.0.2:8741".into(), transport: "quic".into(), round_ms: 3, fallback: None };
/// assert_eq!(serde_json::to_value(&path).unwrap()["transport"], "quic");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkPath {
    /// The endpoint as dialled.
    pub endpoint: String,
    /// `quic` or `http`.
    pub transport: String,
    /// How long the last exchange took, end to end, in milliseconds.
    pub round_ms: u64,
    /// Why QUIC was not used, when the path is `http` after QUIC was tried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// What `mesh status` says about the link transport.
///
/// ```
/// use majordomus_cli::mesh::quic::LinkStatus;
///
/// let status = LinkStatus { listening: None, reason: Some("udp 0.0.0.0:8741: in use".into()), paths: vec![] };
/// assert_eq!(serde_json::to_value(&status).unwrap()["reason"], "udp 0.0.0.0:8741: in use");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkStatus {
    /// The QUIC endpoint this runtime answers at, when it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listening: Option<String>,
    /// Why it does not, when it does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The path each endpoint this runtime dialled took last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<LinkPath>,
}

/// The transport a runtime dials with: QUIC first where it can be, HTTP where it cannot, and
/// a record of which path each endpoint took. An `https://` endpoint is HTTP's alone (a
/// hosted hub behind a TLS ingress); a `quic://` endpoint is QUIC's alone; a plain
/// `host:port` or an `http://` one is tried over QUIC on the same port number and then over
/// HTTP.
///
/// ```
/// use majordomus_cli::mesh::link::{LinkTransport, Signed, HELLO_PATH};
/// use majordomus_cli::mesh::quic::SelectingTransport;
///
/// // without a QUIC endpoint it is the HTTP transport, and says where it failed
/// let transport = SelectingTransport::new(None);
/// let message = Signed { body: serde_json::json!({}), sig: String::new() };
/// let error = transport.post("127.0.0.1:9", HELLO_PATH, &message).unwrap_err();
/// assert!(error.contains("127.0.0.1:9"));
/// assert_eq!(transport.paths(), vec![], "a failure is no path");
/// ```
pub struct SelectingTransport {
    quic: Option<Arc<QuicNode>>,
    http: Arc<dyn LinkTransport>,
    quic_down: Mutex<HashMap<String, (Instant, String)>>,
    paths: Mutex<HashMap<String, LinkPath>>,
    reason: Option<String>,
}

impl SelectingTransport {
    /// A transport over `quic` when there is one, and HTTP always.
    ///
    /// ```
    /// use majordomus_cli::mesh::quic::SelectingTransport;
    /// assert!(!SelectingTransport::new(None).speaks_quic());
    /// ```
    pub fn new(quic: Option<Arc<QuicNode>>) -> Self {
        Self::over(quic, Arc::new(HttpTransport))
    }

    /// [`SelectingTransport::new`] with another HTTP-side transport, for tests that put
    /// runtimes in one process.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::mesh::link::HttpTransport;
    /// use majordomus_cli::mesh::quic::SelectingTransport;
    /// assert!(!SelectingTransport::over(None, Arc::new(HttpTransport)).speaks_quic());
    /// ```
    pub fn over(quic: Option<Arc<QuicNode>>, http: Arc<dyn LinkTransport>) -> Self {
        SelectingTransport {
            quic,
            http,
            quic_down: Mutex::new(HashMap::new()),
            paths: Mutex::new(HashMap::new()),
            reason: None,
        }
    }

    /// Say why this runtime has no QUIC listener, for `mesh status`.
    ///
    /// ```
    /// use majordomus_cli::mesh::quic::SelectingTransport;
    ///
    /// let transport = SelectingTransport::new(None).because("QUIC is switched off");
    /// assert_eq!(transport.status().reason.as_deref(), Some("QUIC is switched off"));
    /// ```
    pub fn because(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// The listener, the reason there is none, and every path.
    ///
    /// ```
    /// use majordomus_cli::mesh::quic::SelectingTransport;
    /// assert!(SelectingTransport::new(None).status().paths.is_empty());
    /// ```
    pub fn status(&self) -> LinkStatus {
        let listening = self
            .quic
            .as_ref()
            .filter(|q| q.answers())
            .map(|q| q.endpoint());
        LinkStatus {
            reason: if listening.is_some() {
                None
            } else {
                self.reason.clone()
            },
            listening,
            paths: self.paths(),
        }
    }

    /// Whether this transport has a QUIC endpoint to dial from.
    ///
    /// ```
    /// use majordomus_cli::mesh::quic::SelectingTransport;
    /// assert!(!SelectingTransport::new(None).speaks_quic());
    /// ```
    pub fn speaks_quic(&self) -> bool {
        self.quic.is_some()
    }

    /// The path each endpoint's last successful exchange took, sorted by endpoint.
    ///
    /// ```
    /// use majordomus_cli::mesh::quic::SelectingTransport;
    /// assert!(SelectingTransport::new(None).paths().is_empty());
    /// ```
    pub fn paths(&self) -> Vec<LinkPath> {
        let mut paths: Vec<LinkPath> = self
            .paths
            .lock()
            .map(|p| p.values().cloned().collect())
            .unwrap_or_default();
        paths.sort_by(|a, b| a.endpoint.cmp(&b.endpoint));
        paths
    }

    fn record(&self, endpoint: &str, transport: &str, started: Instant, fallback: Option<String>) {
        if let Ok(mut paths) = self.paths.lock() {
            paths.insert(
                endpoint.to_string(),
                LinkPath {
                    endpoint: endpoint.to_string(),
                    transport: transport.to_string(),
                    round_ms: started.elapsed().as_millis() as u64,
                    fallback,
                },
            );
        }
    }

    fn quic_is_down(&self, endpoint: &str) -> Option<String> {
        let down = self.quic_down.lock().ok()?;
        let (since, why) = down.get(endpoint)?;
        (since.elapsed() < QUIC_RETRY_AFTER).then(|| why.clone())
    }
}

impl LinkTransport for SelectingTransport {
    fn post(&self, endpoint: &str, path: &str, message: &Signed) -> Result<LinkReply, String> {
        let started = Instant::now();
        if endpoint.starts_with("https://") {
            let reply = self.http.post(endpoint, path, message)?;
            self.record(endpoint, "http", started, None);
            return Ok(reply);
        }
        let forced = endpoint.starts_with(SCHEME);
        let authority = endpoint
            .strip_prefix(SCHEME)
            .or_else(|| endpoint.strip_prefix("http://"))
            .unwrap_or(endpoint)
            .trim_end_matches('/');
        let quic_endpoint = format!("{SCHEME}{authority}");
        let fallback = match (&self.quic, self.quic_is_down(endpoint)) {
            (None, _) if forced => {
                return Err(format!("{endpoint}: this runtime has no QUIC endpoint"))
            }
            (None, _) => None,
            (Some(_), Some(why)) if !forced => Some(why),
            (Some(quic), _) => match quic.post(&quic_endpoint, path, message) {
                Ok(reply) => {
                    if let Ok(mut down) = self.quic_down.lock() {
                        down.remove(endpoint);
                    }
                    self.record(endpoint, "quic", started, None);
                    return Ok(reply);
                }
                Err(e) if forced => return Err(e),
                Err(e) => {
                    if let Ok(mut down) = self.quic_down.lock() {
                        down.insert(endpoint.to_string(), (Instant::now(), e.clone()));
                    }
                    Some(e)
                }
            },
        };
        let started = Instant::now();
        let reply = self.http.post(endpoint, path, message)?;
        self.record(endpoint, "http", started, fallback);
        Ok(reply)
    }
}

/// The transport a shared server's cooperation dials with: a QUIC endpoint on the server's
/// own address (the same port number, over UDP) that answers link messages with the
/// runtime's cooperation, beside HTTP. A port QUIC cannot bind leaves a dial-only endpoint;
/// `off` leaves HTTP alone; either way `mesh status` says why. The handler holds the runtime
/// weakly, so the endpoint never keeps a runtime alive; the endpoint closes when the last
/// holder of the transport (the runtime and its cooperation) lets it go.
///
/// ```
/// use std::sync::Arc;
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::manager::MeshRuntime;
/// use majordomus_cli::mesh::quic::runtime_transport;
///
/// let mesh = Arc::new(MeshRuntime::new());
/// let identity = NodeIdentity::ephemeral().unwrap();
/// runtime_transport(&mesh, &identity, "127.0.0.1:0", false);
/// assert!(mesh.status().links.unwrap().listening.unwrap().starts_with("quic://127.0.0.1:"));
///
/// let off = Arc::new(MeshRuntime::new());
/// runtime_transport(&off, &identity, "127.0.0.1:0", true);
/// assert_eq!(off.status().links.unwrap().reason.as_deref(), Some("MAJORDOMUS_MESH_QUIC=off"));
/// ```
pub fn runtime_transport(
    mesh: &Arc<super::manager::MeshRuntime>,
    identity: &NodeIdentity,
    bound: &str,
    off: bool,
) -> Arc<dyn LinkTransport> {
    let transport = if off {
        SelectingTransport::new(None).because("MAJORDOMUS_MESH_QUIC=off")
    } else {
        match bound.parse::<SocketAddr>() {
            Err(_) => SelectingTransport::new(None)
                .because(format!("{bound} is not a socket address to listen on")),
            Ok(address) => {
                let runtime = Arc::downgrade(mesh);
                let handler: LinkHandler = Arc::new(move |path, message| {
                    match runtime.upgrade().and_then(|m| m.cooperation()) {
                        Some(c) if path == HELLO_PATH => c.accept_hello(message),
                        Some(c) => c.accept_sync(message),
                        None => LinkReply::refused(
                            RefusalCode::NotActive,
                            "cooperation is not running",
                        ),
                    }
                });
                match QuicNode::bind(identity, address, Some(handler)) {
                    Ok(node) => {
                        tracing::info!(endpoint = %node.endpoint(), "mesh links answer over QUIC");
                        SelectingTransport::new(Some(node))
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "QUIC does not listen; links dial it and are answered over HTTP");
                        let any = SocketAddr::new(
                            if address.is_ipv6() {
                                std::net::Ipv6Addr::UNSPECIFIED.into()
                            } else {
                                std::net::Ipv4Addr::UNSPECIFIED.into()
                            },
                            0,
                        );
                        match QuicNode::bind(identity, any, None) {
                            Ok(node) => SelectingTransport::new(Some(node)).because(e),
                            Err(again) => SelectingTransport::new(None)
                                .because(format!("{e}; dialling: {again}")),
                        }
                    }
                }
            }
        }
    };
    let transport = Arc::new(transport);
    mesh.attach_transport(Arc::clone(&transport));
    transport
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::link::sign;

    fn welcomer(identity: Arc<NodeIdentity>) -> LinkHandler {
        Arc::new(move |path, _message| {
            let domain = reply_domain(path).unwrap_or(Domain::Welcome);
            LinkReply::accepted(sign(&identity, domain, serde_json::json!({ "path": path })))
        })
    }

    fn listener() -> (Arc<NodeIdentity>, Arc<QuicNode>) {
        let identity = Arc::new(NodeIdentity::ephemeral().unwrap());
        let node = QuicNode::bind(
            &identity,
            "127.0.0.1:0".parse().unwrap(),
            Some(welcomer(Arc::clone(&identity))),
        )
        .unwrap();
        (identity, node)
    }

    fn dialer() -> (NodeIdentity, Arc<QuicNode>) {
        let identity = NodeIdentity::ephemeral().unwrap();
        let node = QuicNode::bind(&identity, "127.0.0.1:0".parse().unwrap(), None).unwrap();
        (identity, node)
    }

    #[test]
    fn a_hello_and_a_sync_travel_on_one_connection() {
        let (_, server) = listener();
        let (identity, client) = dialer();
        let hello = sign(&identity, Domain::Hello, serde_json::json!({}));
        let sync = sign(&identity, Domain::SyncRequest, serde_json::json!({}));
        assert!(client.post(&server.endpoint(), HELLO_PATH, &hello).unwrap().signed.is_some());
        assert!(client.post(&server.endpoint(), SYNC_PATH, &sync).unwrap().signed.is_some());
        assert_eq!(client.held.lock().unwrap().len(), 1, "the second round reused the connection");
    }

    #[test]
    fn a_hello_addressed_to_another_node_is_refused_in_the_handshake() {
        let (_, server) = listener();
        let (identity, client) = dialer();
        let somebody = NodeIdentity::ephemeral().unwrap();
        let to = format!("{}-0000000000000001", somebody.public.node_id.as_str());
        let hello = sign(&identity, Domain::Hello, serde_json::json!({ "to": to }));
        let error = client.post(&server.endpoint(), HELLO_PATH, &hello).unwrap_err();
        assert!(
            error.contains(&format!("expected node {}", somebody.public.node_id.as_str())),
            "the refusal names the node it expected: {error}"
        );
    }

    #[test]
    fn a_request_signed_by_another_key_than_the_connections_is_refused() {
        let (_, server) = listener();
        let (_, client) = dialer();
        let impostor = NodeIdentity::ephemeral().unwrap();
        let hello = sign(&impostor, Domain::Hello, serde_json::json!({}));
        let reply = client.post(&server.endpoint(), HELLO_PATH, &hello).unwrap();
        assert_eq!(reply.refusal.unwrap().code, RefusalCode::Signature);
    }

    #[test]
    fn a_reply_signed_by_another_key_than_the_connections_is_a_transport_failure() {
        let identity = Arc::new(NodeIdentity::ephemeral().unwrap());
        let other = Arc::new(NodeIdentity::ephemeral().unwrap());
        let server = QuicNode::bind(&identity, "127.0.0.1:0".parse().unwrap(), Some(welcomer(other))).unwrap();
        let (dialling, client) = dialer();
        let hello = sign(&dialling, Domain::Hello, serde_json::json!({}));
        let error = client.post(&server.endpoint(), HELLO_PATH, &hello).unwrap_err();
        assert!(error.contains("not signed by the key that holds the connection"), "{error}");
    }

    #[test]
    fn an_oversized_message_is_refused_before_it_is_sent() {
        let (identity, client) = dialer();
        let big = sign(&identity, Domain::Hello, serde_json::json!("x".repeat(MAX_LINK_MESSAGE)));
        let error = client.post("quic://127.0.0.1:9", HELLO_PATH, &big).unwrap_err();
        assert!(error.contains("exceeds"), "{error}");
    }

    #[test]
    fn a_plain_endpoint_takes_quic_where_it_answers() {
        let (_, server) = listener();
        let (identity, client) = dialer();
        let transport = SelectingTransport::new(Some(client));
        let plain = server.local_addr().to_string();
        let hello = sign(&identity, Domain::Hello, serde_json::json!({}));
        transport.post(&plain, HELLO_PATH, &hello).unwrap();
        let paths = transport.paths();
        assert_eq!(paths[0].transport, "quic");
        assert_eq!(paths[0].endpoint, plain);
    }

    #[test]
    fn a_plain_endpoint_falls_back_to_http_when_quic_does_not_answer_and_says_why() {
        struct Echo(Arc<NodeIdentity>);
        impl LinkTransport for Echo {
            fn post(&self, _e: &str, _p: &str, _m: &Signed) -> Result<LinkReply, String> {
                Ok(LinkReply::accepted(sign(&self.0, Domain::Welcome, serde_json::json!({}))))
            }
        }
        let (identity, client) = dialer();
        // a UDP port nobody answers QUIC on
        let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        let endpoint = silent.local_addr().unwrap().to_string();
        let transport = SelectingTransport::over(
            Some(client),
            Arc::new(Echo(Arc::new(NodeIdentity::ephemeral().unwrap()))),
        );
        let hello = sign(&identity, Domain::Hello, serde_json::json!({}));
        transport.post(&endpoint, HELLO_PATH, &hello).unwrap();
        let path = transport.paths().remove(0);
        assert_eq!(path.transport, "http");
        assert!(path.fallback.is_some(), "the fallback says why QUIC was not used");
        // the second round does not wait for QUIC again
        let started = Instant::now();
        transport.post(&endpoint, HELLO_PATH, &hello).unwrap();
        assert!(started.elapsed() < QUIC_HANDSHAKE_WITHIN);
    }
}
