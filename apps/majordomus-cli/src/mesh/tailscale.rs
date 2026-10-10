//! The Tailscale provider: the tailnet as a discovery source (I2410). Every peer
//! `tailscale status --json` lists online is asked as a rendezvous at
//! `http://<tailnet address>:<port>` — the same `mesh.register` exchange the rendezvous
//! provider makes with a declared endpoint — so a machine that joins the tailnet is found
//! with no address written by hand. It proposes addresses and nothing else: what comes
//! back is a set of signed envelopes, each verified on its own signature, and what links is
//! decided by the trust list as for every other source.
//!
//! A machine without the Tailscale CLI has a provider in the `Stopped` state with the reason:
//! absence is not a fault, so `mesh doctor` never fails a repository for it. A CLI that is
//! there and answers with an error is a `Failed` provider with the reason; the other
//! providers run on either way.
//!
//! ```
//! use majordomus_cli::mesh::config::TailscaleConfig;
//! use majordomus_cli::mesh::provider::{MeshProvider, MeshProviderState};
//! use majordomus_cli::mesh::tailscale::TailscaleProvider;
//!
//! let provider = TailscaleProvider::new(TailscaleConfig::default());
//! assert_eq!(provider.id(), "tailscale");
//! assert_eq!(provider.status().state, MeshProviderState::Stopped);
//! ```

use std::net::IpAddr;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::config::TailscaleConfig;
use super::provider::{
    jitter_ms, Counters, MeshProvider, MeshProviderState, Observation, ProviderContext,
    ProviderStatus,
};
use super::registry::MeshSource;
use super::rendezvous::register_once;
use super::MeshError;

/// How long `tailscale status --json` may take before the reading counts as failed.
const STATUS_TIMEOUT: Duration = Duration::from_secs(10);

/// A stop check happens at least this often while waiting.
const SLICE: Duration = Duration::from_millis(500);

/// The provider: its declaration, the counters its status reports, and its state.
pub struct TailscaleProvider {
    config: TailscaleConfig,
    counters: Arc<Counters>,
    state: Arc<Mutex<(MeshProviderState, Option<String>)>>,
}

impl TailscaleProvider {
    /// A provider over its declaration; nothing is asked until the manager calls `start`.
    pub fn new(config: TailscaleConfig) -> Self {
        TailscaleProvider {
            config,
            counters: Arc::new(Counters::default()),
            state: Arc::new(Mutex::new((MeshProviderState::Stopped, None))),
        }
    }
}

impl MeshProvider for TailscaleProvider {
    fn id(&self) -> &'static str {
        "tailscale"
    }

    fn start(&mut self, ctx: &ProviderContext) -> Result<(), MeshError> {
        // a machine without the CLI has no tailnet to ask: stopped, with the reason, and no
        // fault, so a repository whose machines do not run Tailscale is not a failing mesh
        if cli_missing(&self.config.command) {
            *self.state.lock().expect("tailscale state") = (
                MeshProviderState::Stopped,
                Some(format!(
                    "`{}` is not installed here, so the tailnet is not asked",
                    self.config.command
                )),
            );
            return Ok(());
        }
        // asked once now, so a CLI that answers an error says so at start rather than later
        if let Err(why) = read_status(&self.config.command) {
            *self.state.lock().expect("tailscale state") =
                (MeshProviderState::Failed, Some(why.clone()));
            return Err(MeshError::Provider(why));
        }
        *self.state.lock().expect("tailscale state") = (
            MeshProviderState::Running,
            Some(format!("tailnet peers on port {}", self.config.port)),
        );
        let config = self.config.clone();
        let tx = ctx.tx.clone();
        let stop = Arc::clone(&ctx.stop);
        let beacon = Arc::clone(&ctx.beacon);
        let counters = Arc::clone(&self.counters);
        let state = Arc::clone(&self.state);
        let interval = Duration::from_secs(config.interval_seconds.max(1));
        let _ = std::thread::Builder::new()
            .name("majordomus-mesh-tailscale".into())
            .spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match read_status(&config.command) {
                        Ok(json) => {
                            let endpoints = endpoints(&peers_of(&json), config.port);
                            *state.lock().expect("tailscale state") = (
                                MeshProviderState::Running,
                                Some(format!(
                                    "{} online tailnet peer(s) on port {}",
                                    endpoints.len(),
                                    config.port
                                )),
                            );
                            for endpoint in endpoints {
                                if stop.load(Ordering::SeqCst) {
                                    break;
                                }
                                // a peer with no server on the port is the common case, and
                                // the next reading asks it again
                                let Ok(answer) = register_once(&endpoint, &beacon.next_envelope())
                                else {
                                    continue;
                                };
                                counters.sent.fetch_add(1, Ordering::Relaxed);
                                for candidate in answer.candidates {
                                    if let Ok(bytes) = serde_json::to_vec(&candidate) {
                                        counters.received.fetch_add(1, Ordering::Relaxed);
                                        let _ = tx.send(Observation {
                                            source: MeshSource::Tailscale,
                                            path: endpoint.clone(),
                                            bytes,
                                        });
                                    }
                                }
                            }
                        }
                        Err(why) => {
                            *state.lock().expect("tailscale state") =
                                (MeshProviderState::Failed, Some(why));
                        }
                    }
                    let total = interval + Duration::from_millis(jitter_ms(2000));
                    let mut slept = Duration::ZERO;
                    while slept < total && !stop.load(Ordering::SeqCst) {
                        let slice = SLICE.min(total - slept);
                        std::thread::sleep(slice);
                        slept += slice;
                    }
                }
            });
        Ok(())
    }

    fn status(&self) -> ProviderStatus {
        let (state, detail) = self.state.lock().expect("tailscale state").clone();
        ProviderStatus {
            id: self.id().to_string(),
            state,
            detail,
            sent: self.counters.sent.load(Ordering::Relaxed),
            received: self.counters.received.load(Ordering::Relaxed),
        }
    }
}

/// Whether `command` names nothing this machine can run: spawning it finds no executable.
fn cli_missing(command: &str) -> bool {
    match Command::new(command)
        .arg("version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            let _ = child.kill();
            let _ = child.wait();
            false
        }
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}

/// The output of `<command> status --json`, or why it could not be read. Bounded by
/// [`STATUS_TIMEOUT`]: a CLI that hangs is killed and the reading counts as failed.
fn read_status(command: &str) -> Result<String, String> {
    let mut child = Command::new(command)
        .args(["status", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("`{command} status --json` could not run: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() >= STATUS_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "`{command} status --json` did not answer within {}s",
                    STATUS_TIMEOUT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                return Err(format!(
                    "`{command} status --json` could not be waited on: {e}"
                ))
            }
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("`{command} status --json` could not be read: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`{command} status --json` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The tailnet addresses of the peers a `tailscale status --json` answer lists online, one
/// per peer, IPv4 first: this machine (`Self`) is not a peer, and a peer offline or without
/// a tailnet address offers nothing. What cannot be read as JSON offers nothing either.
///
/// ```
/// use majordomus_cli::mesh::tailscale::peers_of;
///
/// let status = r#"{"Self":{"TailscaleIPs":["100.64.0.1"],"Online":true},
///   "Peer":{"a":{"TailscaleIPs":["fd7a::2","100.64.0.2"],"Online":true},
///           "b":{"TailscaleIPs":["100.64.0.3"],"Online":false},
///           "c":{"TailscaleIPs":[],"Online":true}}}"#;
/// let peers: Vec<String> = peers_of(status).iter().map(|ip| ip.to_string()).collect();
/// assert_eq!(peers, ["100.64.0.2"]);
/// assert!(peers_of("not json").is_empty());
/// ```
pub fn peers_of(status_json: &str) -> Vec<IpAddr> {
    let Ok(status) = serde_json::from_str::<serde_json::Value>(status_json) else {
        return Vec::new();
    };
    let Some(peers) = status.get("Peer").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    let mut found: Vec<IpAddr> = peers
        .values()
        .filter(|peer| peer.get("Online").and_then(|o| o.as_bool()) == Some(true))
        .filter_map(|peer| {
            let addresses: Vec<IpAddr> = peer
                .get("TailscaleIPs")?
                .as_array()?
                .iter()
                .filter_map(|a| a.as_str()?.parse().ok())
                .collect();
            addresses
                .iter()
                .find(|a| a.is_ipv4())
                .or_else(|| addresses.first())
                .copied()
        })
        .collect();
    found.sort();
    found.dedup();
    found
}

/// The rendezvous endpoints of `peers` on `port`: `http://<address>:<port>`, an IPv6 address
/// in brackets.
///
/// ```
/// use majordomus_cli::mesh::tailscale::endpoints;
///
/// let peers = ["100.64.0.2".parse().unwrap(), "fd7a::2".parse().unwrap()];
/// assert_eq!(endpoints(&peers, 8791), ["http://100.64.0.2:8791", "http://[fd7a::2]:8791"]);
/// ```
pub fn endpoints(peers: &[IpAddr], port: u16) -> Vec<String> {
    peers
        .iter()
        .map(|ip| match ip {
            IpAddr::V4(v4) => format!("http://{v4}:{port}"),
            IpAddr::V6(v6) => format!("http://[{v6}]:{port}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> (ProviderContext, std::sync::mpsc::Receiver<Observation>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let identity = crate::mesh::identity::NodeIdentity::ephemeral().unwrap();
        let beacon =
            crate::mesh::provider::Beacon::new(Arc::new(identity), vec![], vec![], vec![], "test");
        let ctx = ProviderContext {
            tx,
            stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            beacon: Arc::new(beacon),
        };
        (ctx, rx)
    }

    #[test]
    fn a_machine_without_the_cli_stops_the_provider_with_the_reason_and_is_no_fault() {
        let mut provider = TailscaleProvider::new(TailscaleConfig {
            command: "/nonexistent/majordomus-no-tailscale".into(),
            ..TailscaleConfig::default()
        });
        let (ctx, _rx) = context();
        provider
            .start(&ctx)
            .expect("a missing CLI is absence, not a provider error");
        let status = provider.status();
        assert_eq!(status.state, MeshProviderState::Stopped);
        assert!(
            status
                .detail
                .as_deref()
                .is_some_and(|d| d.contains("is not installed here")),
            "{status:?}"
        );
        ctx.stop.store(true, Ordering::SeqCst);
    }

    #[test]
    fn a_cli_that_answers_with_an_error_is_a_failed_reading_that_says_why() {
        // `false` exits 1 without output: the shape of `tailscale status` on a stopped daemon
        let why = read_status("false").unwrap_err();
        assert!(why.contains("failed"), "{why}");
    }

    #[test]
    fn a_running_provider_names_its_port_and_asks_nobody_when_no_peer_is_online() {
        // a stand-in CLI that prints a tailnet with no peer online
        let dir = tempfile::tempdir().unwrap();
        let cli = dir.path().join("tailscale");
        std::fs::write(
            &cli,
            "#!/bin/sh\necho '{\"Self\":{\"Online\":true},\"Peer\":{\"x\":{\"Online\":false,\"TailscaleIPs\":[\"100.64.0.9\"]}}}'\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut provider = TailscaleProvider::new(TailscaleConfig {
            command: cli.display().to_string(),
            port: 9,
            interval_seconds: 1,
            ..TailscaleConfig::default()
        });
        let (ctx, rx) = context();
        provider
            .start(&ctx)
            .expect("a CLI that answers starts the provider");
        assert_eq!(provider.status().state, MeshProviderState::Running);
        assert!(
            provider
                .status()
                .detail
                .as_deref()
                .is_some_and(|d| d.contains("port 9")),
            "{:?}",
            provider.status()
        );
        std::thread::sleep(Duration::from_millis(300));
        ctx.stop.store(true, Ordering::SeqCst);
        assert!(
            rx.try_recv().is_err(),
            "no peer online, so nothing observed"
        );
        assert_eq!(provider.status().sent, 0);
    }

    #[test]
    fn peers_are_sorted_unique_and_ipv4_first() {
        let status = r#"{"Peer":{
            "b":{"TailscaleIPs":["100.64.0.20"],"Online":true},
            "a":{"TailscaleIPs":["fd7a::1","100.64.0.10"],"Online":true},
            "c":{"TailscaleIPs":["100.64.0.10"],"Online":true},
            "d":{"TailscaleIPs":["fd7a::4"],"Online":true},
            "e":{"Online":true}}}"#;
        let got: Vec<String> = peers_of(status).iter().map(|p| p.to_string()).collect();
        assert_eq!(got, ["100.64.0.10", "100.64.0.20", "fd7a::4"]);
        assert!(
            peers_of(r#"{"Self":{}}"#).is_empty(),
            "no Peer map, no peer"
        );
    }
}
