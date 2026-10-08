//! The Bonjour provider: discovery through the operating system's DNS-SD service
//! (mDNSResponder on macOS, avahi on Linux), as one more implementation of the provider
//! contract (ADR 0120, amending ADR 0050).
//!
//! Why the system's service and not a socket of our own: the macOS application firewall
//! drops inbound multicast addressed to an ordinary process, which is why the multicast
//! provider hears nothing there, and a private mDNS responder on 224.0.0.251:5353 would be
//! dropped in exactly the same way. The system daemon is let through. So this module opens
//! no socket and speaks no mDNS: it asks the platform to register one service instance and
//! to browse for the others, behind the small crate-private `DnsSd` trait — which is also
//! what lets everything here be tested with a fake.
//!
//! What is advertised is one instance of [`DEFAULT_SERVICE`] in `local.`, named
//! `majordomus-<node>-<runtime>` (eight hex characters each; no hostname, no path), on the
//! port of the runtime's HTTP endpoint, whose TXT record carries the same signed envelope
//! every other provider sends. A TXT string holds at most 255 bytes, so the envelope's
//! bytes are split, unencoded, across ordered keys: `txtvers=1`, `n=<count>`,
//! `e0=…` … `e<n-1>=…`. The bytes are the envelope's JSON as signed; an envelope of the
//! protocol's largest size (1200 bytes) makes a record of 1234 bytes, inside the 1300
//! bytes DNS-SD recommends as a ceiling, which a text encoding of it would not be.
//!
//! What is browsed is handed up raw: the TXT strings of each resolved instance are joined
//! back into the envelope's bytes and sent to the manager, which parses, verifies and
//! decides exactly as it does for a multicast datagram. This module reads the envelope
//! never; an instance whose TXT record does not hold a complete, in-bounds envelope is
//! counted and dropped.
//!
//! Freshness. An envelope is replay-protected by a rising sequence, a node stays `present`
//! for 60 seconds after its last accepted advertisement, and an envelope more than 300
//! seconds from the reader's clock is refused as stale. A TXT record is a standing answer,
//! not a datagram, so it is replaced with a freshly signed envelope every declared interval
//! (15 seconds by default, 60 at most): a browser always reads one young enough to pass.
//!
//! ```
//! use majordomus_cli::mesh::bonjour::{envelope_of, txt_of};
//!
//! // the split is exact: what a browser reassembles is what was advertised, byte for byte
//! let envelope = br#"{"v":2,"pk":"00","sig":"00"}"#;
//! let txt = txt_of(envelope).unwrap();
//! assert_eq!(txt[0], b"txtvers=1");
//! assert_eq!(txt[1], b"n=1");
//! assert_eq!(envelope_of(&txt).unwrap(), envelope);
//! ```

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::config::BonjourConfig;
use super::protocol::{encode, MAX_DATAGRAM};
use super::provider::{
    Beacon, Counters, MeshProvider, MeshProviderState, Observation, ProviderContext, ProviderStatus,
};
use super::registry::MeshSource;
use super::MeshError;

/// The DNS-SD service type the mesh registers and browses.
pub const DEFAULT_SERVICE: &str = "_majordomus._tcp";

/// The one domain this provider uses: link-local multicast DNS, never a wide-area domain.
pub const DOMAIN: &str = "local.";

/// The longest interval between two advertisements, in seconds: the registry's presence
/// window. A longer one would let a runtime that is up read as absent between two
/// envelopes, and would hand a late browser an envelope ever closer to the staleness bound.
pub const MAX_INTERVAL: u64 = 60;

/// The most bytes one TXT string holds (its length is one byte on the wire).
const MAX_TXT_STRING: usize = 255;

/// The ceiling for the whole TXT record, in wire bytes: what RFC 6763 §6.2 recommends so
/// that the record fits one Ethernet frame.
pub const MAX_TXT_RECORD: usize = 1300;

/// The layout version this module writes and reads, under the conventional `txtvers` key.
const TXT_VERSION: &[u8] = b"txtvers=1";

/// How long one wait on the platform lasts before the stop flag is read again.
const POLL: Duration = Duration::from_millis(250);

/// The one advertisement of this runtime, as the platform is asked to publish it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Advertised {
    /// The instance name: `majordomus-<node>-<runtime>`.
    pub name: String,
    /// The service type, `_majordomus._tcp` unless the declaration names another.
    pub service: String,
    /// The TCP port of the runtime's HTTP endpoint.
    pub port: u16,
    /// The TXT strings, in order.
    pub txt: Vec<Vec<u8>>,
}

/// One resolved instance: its name as the platform reports it, and its TXT strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    /// The instance, as the platform names it. Provenance only.
    pub name: String,
    /// The TXT strings, in the order the record carries them.
    pub txt: Vec<Vec<u8>>,
}

/// The platform's DNS-SD service, as far as this provider needs it. An implementation
/// holds whatever the platform hands out — references, child processes — and gives every
/// one of them back when it is dropped: dropping the value withdraws the registration.
pub(crate) trait DnsSd: Send {
    /// The mechanism's name, for status lines: `mDNSResponder`, `avahi`.
    fn mechanism(&self) -> &'static str;
    /// Publish the advertisement, replacing the one published before.
    fn publish(&mut self, advertised: &Advertised) -> Result<(), String>;
    /// Start browsing for instances of `service` in `local.`.
    fn browse(&mut self, service: &str) -> Result<(), String>;
    /// Wait at most `wait` and return the instances resolved meanwhile: each one whenever
    /// its TXT record is first read or has changed. An `Err` means browsing has ended.
    fn poll(&mut self, wait: Duration) -> Result<Vec<Found>, String>;
}

/// The platform's service, or the reason this machine has none: the two things a provider
/// can be built over.
pub(crate) type Service = Result<Box<dyn DnsSd>, String>;

/// Split an envelope's bytes into the TXT strings that carry it, or say why it cannot be
/// carried. Nothing is ever truncated: an envelope that does not fit is refused whole.
///
/// ```
/// use majordomus_cli::mesh::bonjour::txt_of;
/// use majordomus_cli::mesh::protocol::MAX_DATAGRAM;
///
/// assert_eq!(txt_of(&vec![b'x'; MAX_DATAGRAM]).unwrap().len(), 2 + 5);
/// assert!(txt_of(&vec![b'x'; MAX_DATAGRAM + 1]).is_err());
/// ```
pub fn txt_of(envelope: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    txt_within(envelope, MAX_TXT_RECORD)
}

/// [`txt_of`] against an explicit record ceiling, so that the refusal is a tested
/// property and not an arithmetic accident of two constants.
fn txt_within(envelope: &[u8], ceiling: usize) -> Result<Vec<Vec<u8>>, String> {
    if envelope.is_empty() || envelope.len() > MAX_DATAGRAM {
        return Err(format!(
            "an envelope of {} bytes is outside 1..={MAX_DATAGRAM}",
            envelope.len()
        ));
    }
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut rest = envelope;
    while !rest.is_empty() {
        let mut part = format!("e{}=", parts.len()).into_bytes();
        let take = rest.len().min(MAX_TXT_STRING - part.len());
        part.extend_from_slice(&rest[..take]);
        parts.push(part);
        rest = &rest[take..];
    }
    let mut txt = vec![
        TXT_VERSION.to_vec(),
        format!("n={}", parts.len()).into_bytes(),
    ];
    txt.extend(parts);
    let wire: usize = txt.iter().map(|s| s.len() + 1).sum();
    if wire > ceiling {
        return Err(format!(
            "a TXT record of {wire} bytes exceeds the {ceiling}-byte ceiling"
        ));
    }
    Ok(txt)
}

/// The TXT strings of a registration that carries no envelope.
fn txt_without_envelope() -> Vec<Vec<u8>> {
    vec![TXT_VERSION.to_vec(), b"n=0".to_vec()]
}

/// Join an instance's TXT strings back into the envelope's bytes, or say why they do not
/// hold one. The parts must be all there, once each and in order, and add up to no more
/// than a datagram; keys this layout does not define are passed over. The bytes are not
/// read: whether they are an envelope at all is the manager's question.
///
/// ```
/// use majordomus_cli::mesh::bonjour::envelope_of;
///
/// let txt = |strings: &[&str]| strings.iter().map(|s| s.as_bytes().to_vec()).collect::<Vec<_>>();
/// assert_eq!(envelope_of(&txt(&["txtvers=1", "n=2", "e0=ab", "e1=c"])).unwrap(), b"abc");
/// assert!(envelope_of(&txt(&["txtvers=1", "n=2", "e1=c", "e0=ab"])).is_err());
/// assert!(envelope_of(&txt(&["txtvers=1", "n=2", "e0=ab"])).is_err());
/// ```
pub fn envelope_of(txt: &[Vec<u8>]) -> Result<Vec<u8>, &'static str> {
    if !txt.iter().any(|s| s == TXT_VERSION) {
        return Err("no txtvers=1");
    }
    let mut declared: Option<usize> = None;
    let mut envelope = Vec::new();
    let mut parts = 0usize;
    for string in txt {
        if let Some(count) = string.strip_prefix(b"n=") {
            let count = std::str::from_utf8(count)
                .ok()
                .and_then(|c| c.parse::<usize>().ok())
                .ok_or("n is not a count")?;
            if declared.replace(count).is_some() {
                return Err("n is stated twice");
            }
            continue;
        }
        let Some((index, payload)) = part_of(string) else {
            continue;
        };
        if index != parts {
            return Err("a part is out of order or repeated");
        }
        parts += 1;
        envelope.extend_from_slice(payload);
        if envelope.len() > MAX_DATAGRAM {
            return Err("the parts exceed a datagram");
        }
    }
    match declared {
        None => Err("no n"),
        Some(0) => Err("the instance advertises no envelope"),
        Some(count) if count != parts => Err("a part is missing or one too many"),
        Some(_) => Ok(envelope),
    }
}

/// `e<index>=<payload>`, when a TXT string is an envelope part.
fn part_of(string: &[u8]) -> Option<(usize, &[u8])> {
    let rest = string.strip_prefix(b"e")?;
    let equals = rest.iter().position(|b| *b == b'=')?;
    let digits = &rest[..equals];
    // at most three digits: no layout this module reads has a thousand parts
    if digits.is_empty() || digits.len() > 3 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let index = digits
        .iter()
        .fold(0usize, |n, d| n * 10 + usize::from(d - b'0'));
    Some((index, &rest[equals + 1..]))
}

/// The wire form of a TXT record: each string behind its length byte. A string longer
/// than 255 bytes has no wire form and is refused.
///
/// ```
/// use majordomus_cli::mesh::bonjour::{txt_rdata, txt_strings};
///
/// let rdata = txt_rdata(&[b"n=1".to_vec(), b"e0=x".to_vec()]).unwrap();
/// assert_eq!(rdata, b"\x03n=1\x04e0=x");
/// assert_eq!(txt_strings(&rdata).unwrap(), [b"n=1".to_vec(), b"e0=x".to_vec()]);
/// assert!(txt_strings(b"\x09short").is_none(), "a length past the end is not a record");
/// ```
pub fn txt_rdata(txt: &[Vec<u8>]) -> Option<Vec<u8>> {
    let mut rdata = Vec::with_capacity(txt.iter().map(|s| s.len() + 1).sum());
    for string in txt {
        rdata.push(u8::try_from(string.len()).ok()?);
        rdata.extend_from_slice(string);
    }
    Some(rdata)
}

/// The strings of a TXT record's wire form, or `None` when a length runs past its end.
/// An empty record is no strings, which is a record and not an error.
///
/// ```
/// use majordomus_cli::mesh::bonjour::txt_strings;
///
/// assert_eq!(txt_strings(b"\x02ab\x00\x01c").unwrap(), [&b"ab"[..], b"", b"c"]);
/// assert!(txt_strings(b"").unwrap().is_empty());
/// assert!(txt_strings(b"\x03ab").is_none());
/// ```
pub fn txt_strings(mut rdata: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut txt = Vec::new();
    while let Some((&len, rest)) = rdata.split_first() {
        let len = usize::from(len);
        if rest.len() < len {
            return None;
        }
        txt.push(rest[..len].to_vec());
        rdata = &rest[len..];
    }
    Some(txt)
}

/// The instance name of a runtime: the short forms of its node id and runtime slot.
/// Stable for a node and checkout, and made of nothing but hex — no hostname, no path, no
/// user name, nothing the envelope beside it does not already carry in full.
///
/// ```
/// use majordomus_cli::mesh::bonjour::instance_name;
///
/// let node = "641bdb94aaaaaaaaaaaaaaaaaaaaaaaa";
/// assert_eq!(instance_name(node, "0123456789abcdef"), "majordomus-641bdb94-01234567");
/// assert_eq!(instance_name(node, ""), "majordomus-641bdb94");
/// ```
pub fn instance_name(node: &str, runtime: &str) -> String {
    let short = |text: &str| text.chars().take(8).collect::<String>();
    if runtime.is_empty() {
        format!("majordomus-{}", short(node))
    } else {
        format!("majordomus-{}-{}", short(node), short(runtime))
    }
}

/// The port to register: that of the first advertised endpoint another machine could
/// dial, else that of a loopback endpoint (which this machine's other runtimes can), else
/// none — a runtime that answers nowhere registers nothing and only browses.
fn service_port(endpoints: &[String]) -> Option<u16> {
    let port = |endpoint: &String| endpoint.rsplit_once(':')?.1.parse::<u16>().ok();
    let loopback = |endpoint: &&String| {
        let host = endpoint.rsplit_once(':').map_or("", |(host, _)| host);
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    };
    endpoints
        .iter()
        .filter(|e| !loopback(e))
        .find_map(port)
        .or_else(|| endpoints.iter().find_map(port))
        .filter(|p| *p != 0)
}

/// The platform's DNS-SD service, or why this machine has none this provider can use.
/// Constructing it registers nothing and browses nothing.
pub(crate) fn platform() -> Service {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(super::bonjour_dnssd::Responder::new()))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        super::bonjour_avahi::Avahi::on_path(std::env::var_os("PATH"))
            .map(|avahi| Box::new(avahi) as Box<dyn DnsSd>)
    }
    #[cfg(not(unix))]
    {
        Err("this platform has no system DNS-SD service this executable uses".into())
    }
}

/// Whether [`platform`] would answer, and with which mechanism: what `mesh doctor` asks
/// without registering or browsing anything.
pub(crate) fn availability() -> Result<&'static str, String> {
    platform().map(|service| service.mechanism())
}

/// What the worker thread and the status share.
#[derive(Default)]
struct Shared {
    counters: Counters,
    /// Instances resolved: every TXT record read.
    resolved: AtomicU64,
    /// Of those, the ones that held no complete, in-bounds envelope.
    refused: AtomicU64,
    facts: Mutex<Facts>,
}

struct Facts {
    state: MeshProviderState,
    /// What is registered and through what, when running; the reason, when not.
    subject: Option<String>,
    /// Why the registration carries no envelope, or why there is no registration.
    note: Option<String>,
    /// The last error the platform returned for a publication.
    last_error: Option<String>,
}

impl Default for Facts {
    fn default() -> Self {
        Facts {
            state: MeshProviderState::Stopped,
            subject: None,
            note: None,
            last_error: None,
        }
    }
}

impl Shared {
    fn set(&self, state: MeshProviderState, subject: Option<String>) {
        let mut facts = self.facts.lock().expect("bonjour state");
        facts.state = state;
        facts.subject = subject;
    }
}

/// The Bonjour provider: the declaration, the platform service it was given, and the one
/// worker thread that owns that service once started. Dropping it ends the thread and
/// withdraws the registration.
///
/// ```
/// use majordomus_cli::mesh::bonjour::BonjourProvider;
/// use majordomus_cli::mesh::config::BonjourConfig;
/// use majordomus_cli::mesh::provider::MeshProvider;
///
/// // A provider that was never started has registered nothing, and says nothing was heard.
/// let provider = BonjourProvider::new(BonjourConfig::default());
/// let status = provider.status();
/// assert_eq!((status.sent, status.received, status.detail), (0, 0, None));
/// drop(provider);
/// ```
pub struct BonjourProvider {
    config: BonjourConfig,
    service: Option<Service>,
    shared: Arc<Shared>,
    /// This provider's own stop: set when it is dropped, beside the manager's shared flag.
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl BonjourProvider {
    /// A provider over its declaration and this machine's DNS-SD service. Nothing is
    /// registered or browsed until the manager calls `start`.
    ///
    /// ```
    /// use majordomus_cli::mesh::bonjour::BonjourProvider;
    /// use majordomus_cli::mesh::config::BonjourConfig;
    /// use majordomus_cli::mesh::provider::{MeshProvider, MeshProviderState};
    ///
    /// let provider = BonjourProvider::new(BonjourConfig::default());
    /// assert_eq!(provider.id(), "bonjour");
    /// assert_eq!(provider.status().state, MeshProviderState::Stopped);
    /// ```
    pub fn new(config: BonjourConfig) -> Self {
        Self::with_service(config, platform())
    }

    /// A provider over an explicit service, or over the reason there is none: how a test
    /// gives it a fake, and how an unavailable platform is represented.
    pub(crate) fn with_service(config: BonjourConfig, service: Service) -> Self {
        BonjourProvider {
            config,
            service: Some(service),
            shared: Arc::new(Shared::default()),
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }
}

impl MeshProvider for BonjourProvider {
    fn id(&self) -> &'static str {
        "bonjour"
    }

    fn start(&mut self, ctx: &ProviderContext) -> Result<(), MeshError> {
        let service = match self.service.take() {
            Some(Ok(service)) => service,
            // No usable service on this machine is a status, not an error: the mesh runs
            // on with its other providers and the doctor says what is missing.
            Some(Err(reason)) => {
                self.shared
                    .set(MeshProviderState::Unavailable, Some(reason));
                return Ok(());
            }
            None => return Err(MeshError::Provider("bonjour was already started".into())),
        };
        self.shared.set(
            MeshProviderState::Running,
            Some(format!("starting through {}", service.mechanism())),
        );
        let worker = Worker {
            service,
            service_type: self.config.service.clone(),
            interval: Duration::from_secs(self.config.interval_seconds),
            beacon: Arc::clone(&ctx.beacon),
            tx: ctx.tx.clone(),
            stops: [Arc::clone(&ctx.stop), Arc::clone(&self.stop)],
            shared: Arc::clone(&self.shared),
        };
        self.worker = std::thread::Builder::new()
            .name("majordomus-mesh-bonjour".into())
            .spawn(move || worker.run())
            .ok();
        Ok(())
    }

    fn status(&self) -> ProviderStatus {
        let facts = self.shared.facts.lock().expect("bonjour state");
        let mut detail: Vec<String> = facts.subject.iter().cloned().collect();
        if facts.state == MeshProviderState::Running {
            detail.push(format!(
                "{} instance(s) resolved, {} without a whole envelope",
                self.shared.resolved.load(Ordering::Relaxed),
                self.shared.refused.load(Ordering::Relaxed)
            ));
        }
        detail.extend(facts.note.iter().cloned());
        if let Some(error) = &facts.last_error {
            detail.push(format!("last error: {error}"));
        }
        ProviderStatus {
            id: self.id().into(),
            state: facts.state.clone(),
            detail: (!detail.is_empty()).then(|| detail.join("; ")),
            sent: self.shared.counters.sent.load(Ordering::Relaxed),
            received: self.shared.counters.received.load(Ordering::Relaxed),
        }
    }
}

/// Dropping the provider ends its thread and, with it, the platform service the thread
/// owns: no registration and no child process outlives the provider.
impl Drop for BonjourProvider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// The provider's thread: it owns the platform service, so that the service is used from
/// one thread and withdrawn when that thread ends.
struct Worker {
    service: Box<dyn DnsSd>,
    service_type: String,
    interval: Duration,
    beacon: Arc<Beacon>,
    tx: Sender<Observation>,
    stops: [Arc<AtomicBool>; 2],
    shared: Arc<Shared>,
}

impl Worker {
    fn stopped(&self) -> bool {
        self.stops.iter().any(|s| s.load(Ordering::SeqCst))
    }

    fn run(mut self) {
        if let Err(e) = self.service.browse(&self.service_type) {
            self.shared
                .set(MeshProviderState::Failed, Some(format!("browse: {e}")));
            return;
        }
        let node = self.beacon.identity().public.node_id.to_string();
        let name = instance_name(&node, self.beacon.runtime());
        let port = service_port(self.beacon.endpoints());
        let mechanism = self.service.mechanism();
        let subject = match port {
            Some(port) => format!(
                "{name}.{}.{DOMAIN} port {port} through {mechanism}",
                self.service_type
            ),
            None => format!(
                "browsing {}.{DOMAIN} through {mechanism}",
                self.service_type
            ),
        };
        self.shared.set(MeshProviderState::Running, Some(subject));
        if port.is_none() {
            self.shared.facts.lock().expect("bonjour state").note =
                Some("nothing registered: this runtime advertises no endpoint".into());
        }

        let mut published: Option<Vec<Vec<u8>>> = None;
        let mut due = Instant::now();
        while !self.stopped() {
            if let (Some(port), true) = (port, Instant::now() >= due) {
                due = Instant::now() + self.interval;
                self.publish(&name, port, &mut published);
            }
            match self.service.poll(POLL) {
                Ok(found) => found.into_iter().for_each(|f| self.forward(f)),
                Err(e) => {
                    self.shared
                        .set(MeshProviderState::Failed, Some(format!("browse: {e}")));
                    return;
                }
            }
        }
        // the service goes first, so that "stopped" is never read while a registration stands
        drop(self.service);
        self.shared.set(MeshProviderState::Stopped, None);
    }

    /// Sign the next envelope and replace the registration's TXT record with it. An
    /// envelope that does not fit is never cut to fit: the instance is registered without
    /// one, and the status says so.
    fn publish(&mut self, name: &str, port: u16, published: &mut Option<Vec<Vec<u8>>>) {
        let carried = encode(&self.beacon.next_envelope())
            .map_err(|e| e.to_string())
            .and_then(|bytes| txt_of(&bytes));
        let (txt, note) = match carried {
            Ok(txt) => (txt, None),
            Err(why) => (
                txt_without_envelope(),
                Some(format!("registered without the envelope: {why}")),
            ),
        };
        self.shared.facts.lock().expect("bonjour state").note = note;
        if published.as_ref() == Some(&txt) {
            return;
        }
        let outcome = self.service.publish(&Advertised {
            name: name.into(),
            service: self.service_type.clone(),
            port,
            txt: txt.clone(),
        });
        let mut facts = self.shared.facts.lock().expect("bonjour state");
        match outcome {
            Ok(()) => {
                *published = Some(txt);
                facts.last_error = None;
                self.shared.counters.sent.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => facts.last_error = Some(format!("publish: {e}")),
        }
    }

    /// One resolved instance: its envelope's bytes to the manager, raw, or a count.
    fn forward(&self, found: Found) {
        self.shared.resolved.fetch_add(1, Ordering::Relaxed);
        match envelope_of(&found.txt) {
            Ok(bytes) => {
                self.shared
                    .counters
                    .received
                    .fetch_add(1, Ordering::Relaxed);
                let _ = self.tx.send(Observation {
                    source: MeshSource::Bonjour,
                    path: found.name,
                    bytes,
                });
            }
            Err(_) => {
                self.shared.refused.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mesh::config::MeshConfig;
    use crate::mesh::identity::NodeIdentity;
    use crate::mesh::manager::MeshRuntime;
    use crate::mesh::protocol::advertise;
    use std::collections::VecDeque;
    use std::sync::mpsc::{channel, Receiver};

    /// What a fake DNS-SD service was asked to do, and what it will answer.
    #[derive(Default)]
    pub(crate) struct Net {
        pub(crate) published: Vec<Advertised>,
        pub(crate) browsing: Option<String>,
        pub(crate) inbox: VecDeque<Found>,
        /// Hand every publication back as a resolved instance, as the daemon does.
        pub(crate) echo: bool,
        pub(crate) withdrawn: bool,
        pub(crate) refuse_browse: bool,
        pub(crate) refuse_publish: bool,
        pub(crate) end_browsing: bool,
    }

    /// The fake backend: a DNS-SD service that is a struct behind a lock.
    pub(crate) struct Fake(pub(crate) Arc<Mutex<Net>>);

    impl Fake {
        pub(crate) fn service() -> (Arc<Mutex<Net>>, Service) {
            let net = Arc::new(Mutex::new(Net::default()));
            (Arc::clone(&net), Ok(Box::new(Fake(net))))
        }
    }

    impl DnsSd for Fake {
        fn mechanism(&self) -> &'static str {
            "fake"
        }
        fn publish(&mut self, advertised: &Advertised) -> Result<(), String> {
            let mut net = self.0.lock().unwrap();
            if net.refuse_publish {
                return Err("the daemon refused".into());
            }
            net.published.push(advertised.clone());
            if net.echo {
                net.inbox.push_back(Found {
                    name: format!("{}.{}.{DOMAIN}", advertised.name, advertised.service),
                    txt: advertised.txt.clone(),
                });
            }
            Ok(())
        }
        fn browse(&mut self, service: &str) -> Result<(), String> {
            let mut net = self.0.lock().unwrap();
            if net.refuse_browse {
                return Err("no daemon".into());
            }
            net.browsing = Some(service.into());
            Ok(())
        }
        fn poll(&mut self, _wait: Duration) -> Result<Vec<Found>, String> {
            std::thread::sleep(Duration::from_millis(5));
            let mut net = self.0.lock().unwrap();
            if net.end_browsing {
                return Err("the daemon went away".into());
            }
            Ok(net.inbox.drain(..).collect())
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.0.lock().unwrap().withdrawn = true;
        }
    }

    fn strings(txt: &[&str]) -> Vec<Vec<u8>> {
        txt.iter().map(|s| s.as_bytes().to_vec()).collect()
    }

    fn wait_for<F: Fn() -> bool>(what: &str, check: F) {
        for _ in 0..400 {
            if check() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for {what}");
    }

    struct Started {
        provider: BonjourProvider,
        rx: Receiver<Observation>,
        stop: Arc<AtomicBool>,
        net: Arc<Mutex<Net>>,
    }

    fn beacon(endpoints: &[&str], repos: Vec<String>) -> Arc<Beacon> {
        Arc::new(
            Beacon::new(
                Arc::new(NodeIdentity::ephemeral().unwrap()),
                endpoints.iter().map(|e| e.to_string()).collect(),
                vec!["http".into()],
                repos,
                "test",
            )
            .with_runtime("0123456789abcdef"),
        )
    }

    fn started_with(beacon: Arc<Beacon>, prepare: impl FnOnce(&mut Net)) -> Started {
        let (net, service) = Fake::service();
        prepare(&mut net.lock().unwrap());
        let mut provider = BonjourProvider::with_service(
            BonjourConfig {
                interval_seconds: 1,
                ..BonjourConfig::default()
            },
            service,
        );
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        provider
            .start(&ProviderContext {
                tx,
                stop: Arc::clone(&stop),
                beacon,
            })
            .unwrap();
        Started {
            provider,
            rx,
            stop,
            net,
        }
    }

    fn started(prepare: impl FnOnce(&mut Net)) -> Started {
        started_with(beacon(&["192.168.1.20:8741"], vec![]), prepare)
    }

    #[test]
    fn the_envelope_survives_the_txt_split_byte_for_byte_at_its_largest_and_at_one_byte() {
        for len in [1usize, 2, 251, 252, 253, 504, 505, 1199, MAX_DATAGRAM] {
            // every byte value, so that nothing in the split depends on the bytes being text
            let envelope: Vec<u8> = (0..len).map(|i| (i * 7 + len) as u8).collect();
            let txt = txt_of(&envelope).unwrap();
            assert!(txt.iter().all(|s| s.len() <= MAX_TXT_STRING), "{len}");
            let rdata = txt_rdata(&txt).unwrap();
            assert!(rdata.len() <= MAX_TXT_RECORD, "{len}: {}", rdata.len());
            let heard = txt_strings(&rdata).unwrap();
            assert_eq!(envelope_of(&heard).unwrap(), envelope, "{len}");
        }
        // the largest envelope is five parts and 1234 bytes on the wire
        let largest = txt_of(&[b'x'; MAX_DATAGRAM]).unwrap();
        assert_eq!(largest[1], b"n=5");
        assert_eq!(txt_rdata(&largest).unwrap().len(), 1234);
        // and a string no length byte can state has no wire form
        assert!(txt_rdata(&[vec![b'x'; 256]]).is_none());
    }

    #[test]
    fn an_envelope_that_cannot_fit_is_not_truncated_and_the_status_says_so() {
        assert!(txt_of(&[]).is_err());
        assert!(txt_of(&vec![b'x'; MAX_DATAGRAM + 1]).is_err());
        let over = txt_within(&[b'x'; 600], 400).unwrap_err();
        assert!(over.contains("exceeds the 400-byte ceiling"), "{over}");

        // a beacon whose advertisement outgrows a datagram: sixteen long repository ids
        let repos = (0..16).map(|i| format!("{i:064}")).collect();
        let run = started_with(beacon(&["192.168.1.20:8741"], repos), |_| {});
        wait_for("the registration", || {
            !run.net.lock().unwrap().published.is_empty()
        });
        let published = run.net.lock().unwrap().published[0].clone();
        assert_eq!(published.txt, strings(&["txtvers=1", "n=0"]));
        let detail = run.provider.status().detail.unwrap();
        assert!(
            detail.contains("registered without the envelope")
                && detail.contains("exceeds the 1200-byte datagram bound"),
            "{detail}"
        );
        // an unchanged record is not published again every interval
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(run.net.lock().unwrap().published.len(), 1);
        assert_eq!(run.provider.status().sent, 1);
        // and what such an instance advertises is counted by whoever browses it
        assert_eq!(
            envelope_of(&published.txt),
            Err("the instance advertises no envelope")
        );
    }

    #[test]
    fn an_instance_with_missing_out_of_order_duplicated_or_oversized_parts_forwards_nothing_and_is_counted(
    ) {
        let big = format!("e0={}", "x".repeat(252));
        let oversized: Vec<String> = std::iter::once("txtvers=1".to_string())
            .chain(std::iter::once("n=5".to_string()))
            .chain((0..5).map(|i| format!("e{i}={}", "x".repeat(250))))
            .collect();
        let oversized: Vec<&str> = oversized.iter().map(String::as_str).collect();
        let refused: Vec<(&str, Vec<&str>)> = vec![
            ("missing", vec!["txtvers=1", "n=2", "e0=ab"]),
            ("one too many", vec!["txtvers=1", "n=1", "e0=ab", "e1=c"]),
            ("out of order", vec!["txtvers=1", "n=2", "e1=c", "e0=ab"]),
            ("duplicated", vec!["txtvers=1", "n=2", "e0=ab", "e0=ab"]),
            ("skipped", vec!["txtvers=1", "n=2", "e0=ab", "e2=c"]),
            ("oversized", oversized),
            ("no count", vec!["txtvers=1", "e0=ab"]),
            ("two counts", vec!["txtvers=1", "n=1", "n=1", "e0=ab"]),
            ("a count that is not one", vec!["txtvers=1", "n=x", "e0=ab"]),
            ("no layout version", vec!["n=1", "e0=ab"]),
            ("another layout version", vec!["txtvers=2", "n=1", "e0=ab"]),
            ("no envelope", vec!["txtvers=1", "n=0"]),
            ("nothing at all", vec![]),
        ];
        for (why, txt) in &refused {
            assert!(envelope_of(&strings(txt)).is_err(), "{why}");
        }
        // keys the layout does not define are passed over, wherever they stand
        assert_eq!(
            envelope_of(&strings(&[
                "path=/",
                "txtvers=1",
                "e=",
                "ex=1",
                "eager",
                "e1234=x",
                "n=2",
                "e0=ab",
                "note",
                "e1=c"
            ]))
            .unwrap(),
            b"abc"
        );
        assert_eq!(
            envelope_of(&strings(&["txtvers=1", "n=1", &big]))
                .unwrap()
                .len(),
            252
        );

        let run = started(|net| {
            for (why, txt) in &refused {
                net.inbox.push_back(Found {
                    name: (*why).into(),
                    txt: strings(txt),
                });
            }
        });
        wait_for("every instance to be counted", || {
            run.provider
                .status()
                .detail
                .unwrap_or_default()
                .contains(&format!(
                    "{0} instance(s) resolved, {0} without a whole envelope",
                    refused.len()
                ))
        });
        assert_eq!(run.provider.status().received, 0);
        assert!(run.rx.try_recv().is_err(), "nothing reached the manager");
    }

    #[test]
    fn what_is_forwarded_is_exactly_what_was_advertised_under_this_providers_label() {
        let run = started(|net| net.echo = true);
        let heard = run.rx.recv_timeout(Duration::from_secs(4)).unwrap();
        let published = run.net.lock().unwrap().published[0].clone();
        assert_eq!(published.service, DEFAULT_SERVICE);
        assert_eq!(published.port, 8741);
        assert!(
            published.name.starts_with("majordomus-") && published.name.ends_with("-01234567"),
            "{}",
            published.name
        );
        assert_eq!(
            run.net.lock().unwrap().browsing.as_deref(),
            Some(DEFAULT_SERVICE)
        );
        // the bytes are the envelope as signed: the manager's parser takes them as they are
        assert_eq!(heard.bytes, envelope_of(&published.txt).unwrap());
        assert!(crate::mesh::protocol::parse(&heard.bytes).is_ok());
        assert_eq!(heard.source, MeshSource::Bonjour);
        assert_eq!(heard.source.as_str(), "bonjour");
        assert_eq!(
            heard.path,
            format!("{}.{DEFAULT_SERVICE}.{DOMAIN}", published.name)
        );
        // the next interval replaces the record with a later sequence
        let again = run.rx.recv_timeout(Duration::from_secs(4)).unwrap();
        let (first, second) = (
            crate::mesh::protocol::parse(&heard.bytes).unwrap(),
            crate::mesh::protocol::parse(&again.bytes).unwrap(),
        );
        assert!(second.adv.seq > first.adv.seq);
        let status = run.provider.status();
        assert_eq!(status.state, MeshProviderState::Running);
        assert!(status.sent >= 2 && status.received >= 2, "{status:?}");
        let detail = status.detail.unwrap();
        assert!(
            detail.starts_with(&format!(
                "{}.{DEFAULT_SERVICE}.{DOMAIN} port 8741 through fake; ",
                published.name
            )),
            "{detail}"
        );
    }

    #[test]
    fn the_manager_verifies_an_envelope_from_bonjour_on_the_path_every_provider_shares() {
        let dir = tempfile::tempdir().unwrap();
        let identity = |name: &str| NodeIdentity::load_or_create(&dir.path().join(name)).unwrap();
        let config: MeshConfig = serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "test", "enabled": true,
            "multicast": { "enabled": false },
        }))
        .unwrap();
        let runtime = MeshRuntime::new();
        runtime
            .activate(&config, identity("self.json"), vec![], vec![], "test")
            .unwrap();
        let other = identity("other.json");
        let envelope = advertise(&other, 1, &["192.168.1.30:8741".into()], &[], &[], "test");
        let bytes = serde_json::to_vec(&envelope).unwrap();
        // the same envelope saying one thing its key did not sign
        let forged =
            String::from_utf8(bytes.clone())
                .unwrap()
                .replacen("192.168.1.30", "192.168.1.66", 1);
        assert_ne!(forged.as_bytes(), bytes);
        let (net, service) = Fake::service();
        for (name, bytes) in [("other", bytes), ("forged", forged.into_bytes())] {
            net.lock().unwrap().inbox.push_back(Found {
                name: name.into(),
                txt: txt_of(&bytes).unwrap(),
            });
        }
        runtime
            .attach(Box::new(BonjourProvider::with_service(
                BonjourConfig::default(),
                service,
            )))
            .unwrap();
        wait_for("the verified node and the refused forgery", || {
            runtime.nodes().len() == 1 && runtime.status().refusals.signature == 1
        });
        let node = &runtime.nodes()[0];
        assert_eq!(node.node_id, other.public.node_id);
        assert_eq!(node.sources[0].source, MeshSource::Bonjour);
        assert_eq!(node.sources[0].path, "other");
        assert!(!node.trust.is_trusted(), "found is not trusted");
        // this runtime has no endpoint, so it browses and registers nothing
        let bonjour = &runtime.status().providers[0];
        assert_eq!((bonjour.id.as_str(), bonjour.sent), ("bonjour", 0));
        let detail = bonjour.detail.clone().unwrap();
        assert!(
            detail.starts_with("browsing _majordomus._tcp.local. through fake; ")
                && detail.contains("nothing registered: this runtime advertises no endpoint"),
            "{detail}"
        );
        assert!(net.lock().unwrap().published.is_empty());
        runtime.stop();
        assert!(net.lock().unwrap().withdrawn, "stopping the mesh withdraws");
    }

    #[test]
    fn stopping_deregisters_and_a_dropped_provider_leaves_nothing_behind() {
        let run = started(|_| {});
        wait_for("the registration", || {
            !run.net.lock().unwrap().published.is_empty()
        });
        assert!(!run.net.lock().unwrap().withdrawn);
        run.stop.store(true, Ordering::SeqCst);
        wait_for("the stop to withdraw the registration", || {
            run.net.lock().unwrap().withdrawn
        });
        wait_for("the stopped state", || {
            run.provider.status().state == MeshProviderState::Stopped
        });
        assert!(run.provider.status().detail.is_none());

        // and without the manager's flag: the drop alone ends the thread and the service
        let run = started(|_| {});
        let net = Arc::clone(&run.net);
        drop(run.provider);
        assert!(
            net.lock().unwrap().withdrawn,
            "dropped before the join returned"
        );
    }

    #[test]
    fn an_unavailable_platform_is_a_status_and_the_other_providers_still_start() {
        let dir = tempfile::tempdir().unwrap();
        let config: MeshConfig = serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "test", "enabled": true,
            "multicast": { "enabled": false },
        }))
        .unwrap();
        let runtime = MeshRuntime::new();
        let own = NodeIdentity::load_or_create(&dir.path().join("self.json")).unwrap();
        runtime
            .activate(
                &config,
                own,
                vec!["192.168.1.20:8741".into()],
                vec![],
                "test",
            )
            .unwrap();
        runtime
            .attach(Box::new(BonjourProvider::with_service(
                BonjourConfig::default(),
                Err("avahi-publish and avahi-browse are not on PATH".into()),
            )))
            .expect("unavailable is not an error");
        let (net, service) = Fake::service();
        runtime
            .attach(Box::new(BonjourProvider::with_service(
                BonjourConfig {
                    service: "_other._tcp".into(),
                    ..BonjourConfig::default()
                },
                service,
            )))
            .unwrap();
        wait_for("the provider beside it to register", || {
            !net.lock().unwrap().published.is_empty()
        });
        let providers = runtime.status().providers;
        let unavailable = providers
            .iter()
            .find(|p| p.state == MeshProviderState::Unavailable)
            .expect("the unavailable provider is listed");
        assert_eq!(unavailable.id, "bonjour");
        assert_eq!(
            unavailable.detail.as_deref(),
            Some("avahi-publish and avahi-browse are not on PATH")
        );
        assert!(providers
            .iter()
            .any(|p| p.state == MeshProviderState::Running));
        runtime.stop();
    }

    #[test]
    fn a_service_that_cannot_browse_fails_and_one_that_cannot_publish_says_so_and_runs_on() {
        let deaf = started(|net| net.refuse_browse = true);
        wait_for("the failed state", || {
            deaf.provider.status().state == MeshProviderState::Failed
        });
        assert_eq!(
            deaf.provider.status().detail.as_deref(),
            Some("browse: no daemon")
        );
        assert!(
            deaf.net.lock().unwrap().withdrawn,
            "a failed provider holds nothing"
        );

        let mute = started(|net| net.refuse_publish = true);
        wait_for("the publication error", || {
            mute.provider
                .status()
                .detail
                .unwrap_or_default()
                .ends_with("last error: publish: the daemon refused")
        });
        assert_eq!(mute.provider.status().state, MeshProviderState::Running);
        assert_eq!(mute.provider.status().sent, 0);
        // the next interval tries again, and a publication that lands clears the error
        mute.net.lock().unwrap().refuse_publish = false;
        wait_for("the retry", || mute.provider.status().sent == 1);
        assert!(!mute
            .provider
            .status()
            .detail
            .unwrap()
            .contains("last error"));
        // browsing that ends takes the provider down, registration included
        mute.net.lock().unwrap().end_browsing = true;
        wait_for("the failed state", || {
            mute.provider.status().state == MeshProviderState::Failed
        });
        assert_eq!(
            mute.provider.status().detail.as_deref(),
            Some("browse: the daemon went away")
        );
        assert!(mute.net.lock().unwrap().withdrawn);
    }

    #[test]
    fn the_cockpit_shows_a_provider_this_machine_cannot_run_by_that_word() {
        // The page asks the mesh capabilities; the runtime behind them is set up here,
        // where the mesh lives, and never from the Cockpit's own code.
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config: MeshConfig = serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "test", "enabled": true,
            "multicast": { "enabled": false },
        }))
        .unwrap();
        let own = NodeIdentity::load_or_create(&dir.path().join("self.json")).unwrap();
        ctx.mesh
            .activate(&config, own, vec![], vec![], "test")
            .unwrap();
        ctx.mesh
            .attach(Box::new(BonjourProvider::with_service(
                BonjourConfig::default(),
                Err("no DNS-SD service on this machine".into()),
            )))
            .unwrap();
        let page = crate::cockpit::pages::mesh(&ctx).main.render();
        assert!(page.contains("bonjour"), "{page}");
        assert!(page.contains("unavailable"), "{page}");
        assert!(page.contains("no DNS-SD service on this machine"), "{page}");
        ctx.mesh.stop();
    }

    #[test]
    fn a_provider_starts_once() {
        let mut run = started(|_| {});
        let (tx, _rx) = channel();
        let again = run.provider.start(&ProviderContext {
            tx,
            stop: Arc::clone(&run.stop),
            beacon: beacon(&[], vec![]),
        });
        assert_eq!(
            again.unwrap_err().to_string(),
            "provider: bonjour was already started"
        );
    }

    #[test]
    fn the_registered_port_is_one_another_machine_could_dial_when_there_is_one() {
        let endpoints = |list: &[&str]| list.iter().map(|e| e.to_string()).collect::<Vec<_>>();
        assert_eq!(
            service_port(&endpoints(&["127.0.0.1:8741", "192.168.1.20:8742"])),
            Some(8742)
        );
        assert_eq!(
            service_port(&endpoints(&["[::1]:8741", "host.lan:8743"])),
            Some(8743)
        );
        assert_eq!(service_port(&endpoints(&["localhost:8741"])), Some(8741));
        assert_eq!(service_port(&endpoints(&["127.0.0.1:8741"])), Some(8741));
        assert_eq!(service_port(&endpoints(&["no-port", "10.0.0.1:0"])), None);
        assert_eq!(service_port(&[]), None);
    }

    #[test]
    fn the_instance_name_is_hex_of_the_node_and_runtime_and_nothing_else() {
        let identity = NodeIdentity::ephemeral().unwrap();
        let node = identity.public.node_id.to_string();
        let name = instance_name(&node, "0123456789abcdef");
        assert_eq!(name, format!("majordomus-{}-01234567", &node[..8]));
        assert!(name.len() <= 63, "a DNS label");
        let rest = name.strip_prefix("majordomus-").unwrap();
        assert!(
            rest.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'),
            "no host name, no user name, no path: {name}"
        );
    }

    #[test]
    fn the_platform_answers_with_a_mechanism_or_with_the_reason_it_has_none() {
        // Constructing the platform's service registers and browses nothing.
        match availability() {
            Ok(mechanism) => assert!(["mDNSResponder", "avahi"].contains(&mechanism)),
            Err(reason) => assert!(!reason.is_empty()),
        }
        let provider = BonjourProvider::new(BonjourConfig::default());
        assert_eq!(provider.status().state, MeshProviderState::Stopped);
        assert_eq!(provider.status().detail, None);
    }

    /// The one test that needs the real daemon: register this provider's own service and
    /// browse it back. Ignored by default so that no suite depends on a daemon; run it with
    /// `cargo test --lib mesh::bonjour -- --ignored --nocapture`. The service type is a
    /// throwaway one, so that no running Majordomus server hears the test;
    /// `MJ_BONJOUR_SERVICE=_name._tcp MJ_BONJOUR_HOLD=<seconds>` names it and keeps the
    /// registration up, for a second process to browse.
    #[test]
    #[ignore = "needs the system DNS-SD daemon"]
    fn the_real_service_registers_this_runtime_and_browses_its_envelope_back() {
        let service = std::env::var("MJ_BONJOUR_SERVICE")
            .unwrap_or_else(|_| format!("_mjt{}._tcp", std::process::id() % 100_000));
        let mut provider = BonjourProvider::new(BonjourConfig {
            service: service.clone(),
            interval_seconds: 2,
            ..BonjourConfig::default()
        });
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let beacon = beacon(&["192.168.1.20:8741"], vec![]);
        provider
            .start(&ProviderContext {
                tx,
                stop: Arc::clone(&stop),
                beacon: Arc::clone(&beacon),
            })
            .unwrap();
        let first = rx
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or_else(|_| panic!("nothing browsed back: {:?}", provider.status()));
        let envelope = crate::mesh::protocol::parse(&first.bytes).expect("a verifying envelope");
        assert_eq!(
            envelope.adv.node_id(),
            Some(beacon.identity().public.node_id.clone())
        );
        assert_eq!(first.source, MeshSource::Bonjour);
        eprintln!(
            "heard {} ({} bytes) from {}",
            service,
            first.bytes.len(),
            first.path
        );
        // the record is replaced in place: a later sequence arrives without a new browse
        let later = (0..8)
            .filter_map(|_| rx.recv_timeout(Duration::from_secs(5)).ok())
            .filter_map(|o| crate::mesh::protocol::parse(&o.bytes).ok())
            .find(|e| e.adv.seq > envelope.adv.seq);
        assert!(
            later.is_some(),
            "no later sequence: {:?}",
            provider.status()
        );
        eprintln!("status {:?}", provider.status());
        // held for a second process to look, when one is asked to
        if let Some(seconds) = std::env::var("MJ_BONJOUR_HOLD")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            eprintln!("holding {service} for {seconds}s");
            std::thread::sleep(Duration::from_secs(seconds));
        }
        drop(provider);
    }
}
