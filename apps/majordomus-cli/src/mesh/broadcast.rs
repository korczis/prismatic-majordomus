//! The UDP broadcast provider: the controlled fallback for segments where multicast is
//! filtered. Same protocol, same port, no second serialization — only the destination
//! differs: the limited broadcast address in `auto` mode, or the directed broadcast
//! addresses the declaration lists in `explicit` mode. Never `auto` by default, and it
//! does not spray interfaces: one socket, the destinations named above, nothing more.
//!
//! Transmit-first by design: when the multicast provider runs, its socket already hears
//! everything aimed at the port and a second bound socket would fight it for the
//! address. This provider binds a listener only when it is the sole UDP provider.

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::config::{BroadcastConfig, BroadcastMode};
use super::protocol::{encode, MAX_DATAGRAM};
use super::provider::{
    jitter_ms, Counters, MeshProvider, Observation, ProviderContext, ProviderState,
    ProviderStatus,
};
use super::registry::Source;
use super::MeshError;

const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// The provider.
pub struct BroadcastProvider {
    config: BroadcastConfig,
    /// Whether this provider should also listen (no multicast provider is running).
    listen: bool,
    counters: Arc<Counters>,
    state: Arc<Mutex<(ProviderState, Option<String>)>>,
}

impl BroadcastProvider {
    /// A provider over its declaration; `listen` when no other UDP socket hears the port.
    pub fn new(config: BroadcastConfig, listen: bool) -> Self {
        BroadcastProvider {
            config,
            listen,
            counters: Arc::new(Counters::default()),
            state: Arc::new(Mutex::new((ProviderState::Stopped, None))),
        }
    }

    fn destinations(&self) -> Result<Vec<SocketAddrV4>, MeshError> {
        match self.config.mode {
            BroadcastMode::Disabled => Ok(Vec::new()),
            BroadcastMode::Auto => Ok(vec![SocketAddrV4::new(
                Ipv4Addr::BROADCAST,
                self.config.port,
            )]),
            BroadcastMode::Explicit => self
                .config
                .networks
                .iter()
                .map(|n| {
                    n.parse::<Ipv4Addr>()
                        .map(|ip| SocketAddrV4::new(ip, self.config.port))
                        .map_err(|_| {
                            MeshError::Provider(format!("'{n}' is not an IPv4 broadcast address"))
                        })
                })
                .collect(),
        }
    }
}

impl MeshProvider for BroadcastProvider {
    fn id(&self) -> &'static str {
        "udp_broadcast"
    }

    fn start(&mut self, ctx: &ProviderContext) -> Result<(), MeshError> {
        let destinations = match self.destinations() {
            Ok(d) => d,
            Err(e) => {
                *self.state.lock().expect("broadcast state") =
                    (ProviderState::Failed, Some(e.to_string()));
                return Err(e);
            }
        };
        if destinations.is_empty() {
            let reason = "mode is disabled".to_string();
            *self.state.lock().expect("broadcast state") =
                (ProviderState::Stopped, Some(reason.clone()));
            return Err(MeshError::Provider(reason));
        }
        // The sender binds an ephemeral port (the listener, when this provider has one,
        // owns the well-known port); directed and limited broadcast need the flag.
        let bind_port = if self.listen { self.config.port } else { 0 };
        let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, bind_port)) {
            Ok(s) => s,
            Err(e) => {
                let reason = format!("cannot bind udp port {bind_port}: {e}");
                *self.state.lock().expect("broadcast state") =
                    (ProviderState::Failed, Some(reason.clone()));
                return Err(MeshError::Provider(reason));
            }
        };
        if let Err(e) = socket.set_broadcast(true) {
            let reason = format!("cannot enable broadcast: {e}");
            *self.state.lock().expect("broadcast state") =
                (ProviderState::Failed, Some(reason.clone()));
            return Err(MeshError::Provider(reason));
        }
        let _ = socket.set_read_timeout(Some(READ_TIMEOUT));
        *self.state.lock().expect("broadcast state") = (
            ProviderState::Running,
            Some(format!(
                "{} destination(s), {}",
                destinations.len(),
                if self.listen { "listening" } else { "transmit only" }
            )),
        );

        if self.listen {
            let socket = socket.try_clone().map_err(|e| {
                MeshError::Provider(format!("cannot clone the broadcast socket: {e}"))
            })?;
            let tx = ctx.tx.clone();
            let stop = Arc::clone(&ctx.stop);
            let counters = Arc::clone(&self.counters);
            let state = Arc::clone(&self.state);
            let _ = std::thread::Builder::new()
                .name("majordomus-mesh-broadcast-listen".into())
                .spawn(move || {
                    let mut buffer = [0u8; MAX_DATAGRAM + 1];
                    while !stop.load(Ordering::SeqCst) {
                        match socket.recv_from(&mut buffer) {
                            Ok((len, from)) => {
                                counters.received.fetch_add(1, Ordering::Relaxed);
                                let _ = tx.send(Observation {
                                    source: Source::UdpBroadcast,
                                    path: from.to_string(),
                                    bytes: buffer[..len].to_vec(),
                                });
                            }
                            Err(e)
                                if e.kind() == std::io::ErrorKind::WouldBlock
                                    || e.kind() == std::io::ErrorKind::TimedOut => {}
                            Err(e) => {
                                *state.lock().expect("broadcast state") =
                                    (ProviderState::Failed, Some(format!("recv: {e}")));
                                return;
                            }
                        }
                    }
                    *state.lock().expect("broadcast state") = (ProviderState::Stopped, None);
                });
        }

        {
            let stop = Arc::clone(&ctx.stop);
            let beacon = Arc::clone(&ctx.beacon);
            let counters = Arc::clone(&self.counters);
            let interval = Duration::from_secs(self.config.interval_seconds);
            let _ = std::thread::Builder::new()
                .name("majordomus-mesh-broadcast-announce".into())
                .spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        if let Ok(bytes) = encode(&beacon.next_envelope()) {
                            for destination in &destinations {
                                if socket.send_to(&bytes, destination).is_ok() {
                                    counters.sent.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
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
        let (state, detail) = self.state.lock().expect("broadcast state").clone();
        ProviderStatus {
            id: self.id().into(),
            state,
            detail,
            sent: self.counters.sent.load(Ordering::Relaxed),
            received: self.counters.received.load(Ordering::Relaxed),
        }
    }
}
