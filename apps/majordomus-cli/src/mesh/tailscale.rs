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
///
/// ```
/// use majordomus_cli::mesh::config::TailscaleConfig;
/// use majordomus_cli::mesh::provider::{MeshProvider, MeshProviderState};
/// use majordomus_cli::mesh::tailscale::TailscaleProvider;
///
/// let provider = TailscaleProvider::new(TailscaleConfig::default());
/// assert_eq!(provider.id(), "tailscale");
/// // nothing asked yet: stopped, and nothing sent
/// let status = provider.status();
/// assert!(matches!(status.state, MeshProviderState::Stopped));
/// assert_eq!(status.sent, 0);
/// ```
pub struct TailscaleProvider {
    config: TailscaleConfig,
    counters: Arc<Counters>,
    state: Arc<Mutex<(MeshProviderState, Option<String>)>>,
}

impl TailscaleProvider {
    /// A provider over its declaration; nothing is asked until the manager calls `start`.
    ///
    /// ```
    /// use majordomus_cli::mesh::config::TailscaleConfig;
    /// use majordomus_cli::mesh::provider::MeshProvider;
    /// use majordomus_cli::mesh::tailscale::TailscaleProvider;
    ///
    /// // a declared command that does not exist is only read at `start`, never here
    /// let config = TailscaleConfig { command: "no-such-tailscale".into(), ..TailscaleConfig::default() };
    /// assert_eq!(TailscaleProvider::new(config).status().id, "tailscale");
    /// ```
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
    read_status_within(command, STATUS_TIMEOUT)
}

/// [`read_status`] with its deadline. Standard output is read on a thread of its own while
/// the process is waited for: a large tailnet answers with more than a pipe holds, and a
/// CLI that cannot finish writing never exits, so reading only after the exit would turn
/// every large answer into a timeout. Standard error is discarded; the exit status says
/// that the reading failed.
fn read_status_within(command: &str, deadline: Duration) -> Result<String, String> {
    let mut child = Command::new(command)
        .args(["status", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("`{command} status --json` could not run: {e}"))?;
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut stdout, &mut text);
        text
    });
    let started = Instant::now();
    let status = loop {
        // an error asking the process is treated as "still running": the deadline ends it
        if let Ok(Some(status)) = child.try_wait() {
            break status;
        }
        if started.elapsed() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "`{command} status --json` did not answer within {}s",
                deadline.as_secs_f32()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let text = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(format!("`{command} status --json` failed: {status}"));
    }
    Ok(text)
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
    // a set, not a sorted list: one candidate per address, in the address's own order,
    // and nothing here is a presentation order (`crate::order` is for those)
    let found: std::collections::BTreeSet<IpAddr> = peers
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
    found.into_iter().collect()
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
        let (tx, rx) = crate::mesh::provider::ObservationSender::bounded(64);
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

    /// A stand-in `tailscale` CLI: `body` is the shell after the shebang.
    fn stand_in(dir: &std::path::Path, body: &str) -> String {
        let cli = dir.join("tailscale");
        std::fs::write(&cli, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        cli.display().to_string()
    }

    fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
        let until = Instant::now() + Duration::from_secs(15);
        while !done() {
            assert!(Instant::now() < until, "gave up waiting for {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn a_cli_that_fails_at_start_fails_the_provider_with_its_reason() {
        let mut provider = TailscaleProvider::new(TailscaleConfig {
            command: "false".into(),
            ..TailscaleConfig::default()
        });
        let (ctx, _rx) = context();
        let err = provider
            .start(&ctx)
            .expect_err("a failing CLI is a provider error");
        assert!(err.to_string().contains("failed"), "{err}");
        assert_eq!(provider.status().state, MeshProviderState::Failed);
    }

    #[test]
    fn an_online_peer_is_asked_as_a_rendezvous_and_its_candidates_are_observed() {
        // a hub on loopback that answers every registration with one candidate
        let hub = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = hub.server_addr().to_ip().unwrap().port();
        let other = crate::mesh::provider::Beacon::new(
            Arc::new(crate::mesh::identity::NodeIdentity::ephemeral().unwrap()),
            vec!["127.0.0.1:1".into()],
            vec![],
            vec![],
            "peer",
        );
        let answer = serde_json::to_string(&super::super::rendezvous::RegisterAnswer {
            accepted: true,
            refusal: None,
            candidates: vec![other.next_envelope()],
        })
        .unwrap();
        let served = std::thread::spawn(move || {
            let request = hub.recv().unwrap();
            assert_eq!(request.url(), super::super::rendezvous::REGISTER_PATH);
            let _ = request.respond(tiny_http::Response::from_string(answer));
        });

        let dir = tempfile::tempdir().unwrap();
        let cli = stand_in(
            dir.path(),
            "echo '{\"Peer\":{\"hub\":{\"Online\":true,\"TailscaleIPs\":[\"127.0.0.1\"]}}}'",
        );
        let mut provider = TailscaleProvider::new(TailscaleConfig {
            command: cli,
            port,
            interval_seconds: 60,
            ..TailscaleConfig::default()
        });
        let (ctx, rx) = context();
        provider.start(&ctx).unwrap();
        let seen = rx
            .recv_timeout(Duration::from_secs(15))
            .expect("the hub's candidate is observed");
        ctx.stop.store(true, Ordering::SeqCst);
        served.join().unwrap();
        assert_eq!(seen.source, MeshSource::Tailscale);
        assert_eq!(seen.path, format!("http://127.0.0.1:{port}"));
        assert_eq!(provider.status().sent, 1);
        assert_eq!(provider.status().received, 1);
        assert!(provider
            .status()
            .detail
            .is_some_and(|d| d.starts_with("1 online tailnet peer(s)")));
    }

    #[test]
    fn a_reading_that_fails_later_fails_the_provider_until_one_succeeds() {
        // answers once, then exits 1: the daemon stopped after the provider started
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("asked");
        let cli = stand_in(
            dir.path(),
            &format!(
                "if [ -e '{m}' ]; then exit 1; fi; touch '{m}'; echo '{{}}'",
                m = marker.display()
            ),
        );
        let mut provider = TailscaleProvider::new(TailscaleConfig {
            command: cli,
            interval_seconds: 1,
            ..TailscaleConfig::default()
        });
        let (ctx, _rx) = context();
        provider.start(&ctx).unwrap();
        wait_for("the failed reading", || {
            provider.status().state == MeshProviderState::Failed
        });
        ctx.stop.store(true, Ordering::SeqCst);
        assert!(provider
            .status()
            .detail
            .is_some_and(|d| d.contains("failed")));
    }

    #[test]
    fn a_reading_is_bounded_and_a_cli_that_cannot_run_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let slow = stand_in(dir.path(), "sleep 5");
        let why = read_status_within(&slow, Duration::from_millis(200)).unwrap_err();
        assert!(why.contains("did not answer within"), "{why}");
        let why = read_status_within("/nonexistent/majordomus-no-cli", Duration::from_secs(1))
            .unwrap_err();
        assert!(why.contains("could not run"), "{why}");
        // a large answer is read whole: more than a pipe holds, and still no timeout
        let big = stand_in(
            dir.path(),
            "i=0; while [ $i -lt 4000 ]; do printf '%064d\\n' 0; i=$((i+1)); done",
        );
        let text = read_status_within(&big, Duration::from_secs(10)).unwrap();
        assert!(text.len() > 200_000, "{}", text.len());
    }

    #[test]
    fn an_answer_that_is_not_a_tailnet_offers_nobody_and_v6_is_bracketed() {
        assert!(peers_of("not json").is_empty());
        assert!(peers_of("{}").is_empty());
        let peers: Vec<IpAddr> = vec!["100.64.0.2".parse().unwrap(), "fd7a::2".parse().unwrap()];
        assert_eq!(
            endpoints(&peers, 8791),
            ["http://100.64.0.2:8791", "http://[fd7a::2]:8791"]
        );
        assert_eq!(MeshSource::Tailscale.as_str(), "tailscale");
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
