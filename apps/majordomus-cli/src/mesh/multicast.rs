//! The UDP multicast provider: the preferred LAN mechanism. One socket, bound to the
//! configured port and joined to the group; a listener thread hands every datagram to
//! the manager, an announcer thread transmits the beacon on the interval with jitter.
//! IPv4 only, TTL from the declaration (1 by default: the local segment and no further).
//!
//! Multicast does not work on every network, and this provider assumes nothing: a socket
//! that cannot bind (another local Majordomus already listens on the port) or a group
//! that cannot be joined marks the provider `Failed` with the reason, and every other
//! provider runs on. The datagrams this socket hears may also be unicast or broadcast
//! aimed at the port — provenance names the socket that heard, not the routing that
//! delivered.

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::config::MulticastConfig;
use super::protocol::{encode, MAX_DATAGRAM};
use super::provider::{
    jitter_ms, Counters, MeshProvider, Observation, ProviderContext, ProviderState,
    ProviderStatus,
};
use super::registry::Source;
use super::MeshError;

/// How long a blocked read waits before checking the stop flag: every wait is bounded.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// The provider.
pub struct MulticastProvider {
    config: MulticastConfig,
    counters: Arc<Counters>,
    state: Arc<Mutex<(ProviderState, Option<String>)>>,
}

impl MulticastProvider {
    /// A provider over its declaration.
    pub fn new(config: MulticastConfig) -> Self {
        MulticastProvider {
            config,
            counters: Arc::new(Counters::default()),
            state: Arc::new(Mutex::new((ProviderState::Stopped, None))),
        }
    }

    fn open(&self) -> Result<(UdpSocket, Ipv4Addr), MeshError> {
        let group: Ipv4Addr = self
            .config
            .group
            .parse()
            .map_err(|_| MeshError::Provider(format!("'{}' is not an IPv4 group", self.config.group)))?;
        if !group.is_multicast() {
            return Err(MeshError::Provider(format!(
                "{group} is not a multicast address"
            )));
        }
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, self.config.port)).map_err(|e| {
            MeshError::Provider(format!(
                "cannot bind udp port {}: {e} (another local server may already listen for the mesh)",
                self.config.port
            ))
        })?;
        socket
            .join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED)
            .map_err(|e| MeshError::Provider(format!("cannot join {group}: {e}")))?;
        socket
            .set_multicast_ttl_v4(self.config.ttl)
            .map_err(|e| MeshError::Provider(format!("cannot set ttl {}: {e}", self.config.ttl)))?;
        socket
            .set_read_timeout(Some(READ_TIMEOUT))
            .map_err(|e| MeshError::Provider(format!("cannot bound the socket read: {e}")))?;
        Ok((socket, group))
    }
}

impl MeshProvider for MulticastProvider {
    fn id(&self) -> &'static str {
        "udp_multicast"
    }

    fn start(&mut self, ctx: &ProviderContext) -> Result<(), MeshError> {
        let (socket, group) = match self.open() {
            Ok(opened) => opened,
            Err(e) => {
                *self.state.lock().expect("multicast state") =
                    (ProviderState::Failed, Some(e.to_string()));
                return Err(e);
            }
        };
        let destination = SocketAddrV4::new(group, self.config.port);
        *self.state.lock().expect("multicast state") = (
            ProviderState::Running,
            Some(format!("group {destination}, ttl {}", self.config.ttl)),
        );

        // The listener: every datagram to the manager, raw. Parsing happens once, above.
        {
            let socket = socket.try_clone().map_err(|e| {
                MeshError::Provider(format!("cannot clone the multicast socket: {e}"))
            })?;
            let tx = ctx.tx.clone();
            let stop = Arc::clone(&ctx.stop);
            let counters = Arc::clone(&self.counters);
            let state = Arc::clone(&self.state);
            let _ = std::thread::Builder::new()
                .name("majordomus-mesh-multicast-listen".into())
                .spawn(move || {
                    let mut buffer = [0u8; MAX_DATAGRAM + 1];
                    while !stop.load(Ordering::SeqCst) {
                        match socket.recv_from(&mut buffer) {
                            Ok((len, from)) => {
                                counters.received.fetch_add(1, Ordering::Relaxed);
                                let _ = tx.send(Observation {
                                    source: Source::UdpMulticast,
                                    path: from.to_string(),
                                    bytes: buffer[..len].to_vec(),
                                });
                            }
                            Err(e)
                                if e.kind() == std::io::ErrorKind::WouldBlock
                                    || e.kind() == std::io::ErrorKind::TimedOut => {}
                            Err(e) => {
                                *state.lock().expect("multicast state") =
                                    (ProviderState::Failed, Some(format!("recv: {e}")));
                                return;
                            }
                        }
                    }
                    *state.lock().expect("multicast state") = (ProviderState::Stopped, None);
                });
        }

        // The announcer: the beacon on the interval, jittered, and once immediately —
        // a node that starts should be seen now, not an interval from now.
        {
            let tx_socket = socket;
            let stop = Arc::clone(&ctx.stop);
            let beacon = Arc::clone(&ctx.beacon);
            let counters = Arc::clone(&self.counters);
            let interval = Duration::from_secs(self.config.interval_seconds);
            let _ = std::thread::Builder::new()
                .name("majordomus-mesh-multicast-announce".into())
                .spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        if let Ok(bytes) = encode(&beacon.next_envelope()) {
                            if tx_socket.send_to(&bytes, destination).is_ok() {
                                counters.sent.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        // Sleep the interval plus jitter, in bounded slices so a stop
                        // is honoured within half a second.
                        let total = interval + Duration::from_millis(jitter_ms(2000));
                        let mut slept = Duration::ZERO;
                        while slept < total && !stop.load(Ordering::SeqCst) {
                            let slice = READ_TIMEOUT.min(total - slept);
                            std::thread::sleep(slice);
                            slept += slice;
                        }
                    }
                });
        }
        Ok(())
    }

    fn status(&self) -> ProviderStatus {
        let (state, detail) = self.state.lock().expect("multicast state").clone();
        ProviderStatus {
            id: self.id().into(),
            state,
            detail,
            sent: self.counters.sent.load(Ordering::Relaxed),
            received: self.counters.received.load(Ordering::Relaxed),
        }
    }
}
