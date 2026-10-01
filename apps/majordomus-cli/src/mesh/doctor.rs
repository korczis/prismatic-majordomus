//! The mesh self-check: every prerequisite proved on this machine alone, no second node
//! required. Deterministic checks in a fixed order, each with its own verdict, so "why
//! is the mesh not working" has an answer that names the broken link instead of a
//! shrug — and, when a check fails or limits cooperation, what that means and what to do.
//! Read-only toward the repository; the sockets it probes are ephemeral and closed before
//! it answers.
//!
//! One check judges a decision rather than the machine. `runtime` reads what the mesh
//! runtime of the process it runs in decided: in a shared server, whether an enabled
//! declaration actually activated, and why not when it did not — the failure a declaration
//! that says on and a server that is off would otherwise be, silently (ADR 0059). In any
//! other process nothing activates the mesh, and the check reports that absence instead of
//! judging it; `majordomus mesh doctor` therefore asks the running server for the report
//! when one serves the checkout, so that the verdict is the server's.
//!
//! What this cannot see is whether the server's cooperation thread beats and whether its
//! peers answer. That is `mesh verify`, which asks the server for a live round.
//!
//! ```
//! use majordomus_cli::mesh::doctor::doctor;
//!
//! // No declaration is the default posture, and the self-check says so and runs on.
//! let report = doctor(None);
//! let declaration = report.checks.iter().find(|c| c.check == "declaration").unwrap();
//! assert!(declaration.ok);
//! assert!(declaration.detail.contains("default posture"));
//! ```

use std::net::{Ipv4Addr, UdpSocket};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::config::MeshConfig;
use super::identity::{default_identity_path, NodeIdentity};
use super::manager::MeshStatus;
use super::protocol;
use super::provider::MeshProviderState;
use super::MeshError;

/// One check's verdict.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DoctorCheck {
    /// What was checked.
    pub check: String,
    /// Whether it holds.
    pub ok: bool,
    /// The evidence: a path, an address, an error.
    pub detail: String,
    /// What a failure or a limitation means for the mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<String>,
    /// What to do about it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl DoctorCheck {
    fn pass(check: &str, detail: impl Into<String>) -> Self {
        DoctorCheck {
            check: check.into(),
            ok: true,
            detail: detail.into(),
            impact: None,
            remediation: None,
        }
    }

    fn fail(check: &str, detail: impl Into<String>, impact: &str, remediation: &str) -> Self {
        DoctorCheck {
            check: check.into(),
            ok: false,
            detail: detail.into(),
            impact: Some(impact.into()),
            remediation: Some(remediation.into()),
        }
    }
}

/// The whole self-check.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeshDoctorReport {
    /// Whether every check holds.
    pub ok: bool,
    /// The checks, in the order they ran.
    pub checks: Vec<DoctorCheck>,
}

/// Run the self-check against the declaration as parsed (or its absence, or its error),
/// in a process no shared server runs in. Deterministic order, no second node required, no
/// repository writes.
pub fn doctor(declaration: Option<Result<MeshConfig, MeshError>>) -> MeshDoctorReport {
    doctor_at(declaration, None, None)
}

/// [`doctor`] for the repository at `root`, in the process whose mesh runtime decided
/// `runtime`: adds whether the repository has a mesh identity cooperation can match on, and
/// judges what the runtime decided about the declaration.
///
/// Discovery works without a repository identity — two nodes can hear each other while
/// belonging to different repositories — but a link does not, so a report that omits this
/// check can say every prerequisite holds while cooperation is unreachable. `root` is
/// optional because the caller may have no repository at hand, and then the check is not
/// run rather than failed.
///
/// `runtime` is the status of a runtime a shared server decided on — activated, or declined
/// with a reason — and `None` in any process no server runs in
/// ([`MeshRuntime::decided`](super::manager::MeshRuntime::decided)). The `runtime` check
/// fails exactly when the declaration is enabled and a server's mesh is not active.
///
/// ```
/// use majordomus_cli::mesh::doctor::doctor_at;
/// use majordomus_cli::mesh::manager::MeshRuntime;
/// use majordomus_cli::mesh::MeshConfig;
///
/// // a directory holding no history has no identity to match on, and the check says so
/// let nowhere = tempfile::tempdir().unwrap();
/// let report = doctor_at(None, Some(nowhere.path()), None);
/// let repository = report.checks.iter().find(|c| c.check == "repository").unwrap();
/// assert!(!repository.ok);
/// assert!(repository.remediation.as_deref().unwrap().contains("cooperation.repository"));
///
/// // with no root to ask about, the question is not asked at all
/// assert!(doctor_at(None, None, None).checks.iter().all(|c| c.check != "repository"));
///
/// // an enabled declaration a server declined is the runtime's failure, with the reason
/// let enabled: MeshConfig = serde_json::from_value(serde_json::json!({
///     "schema": "mesh/v1", "kind": "mesh-declaration", "id": "doc", "enabled": true,
/// })).unwrap();
/// let server = MeshRuntime::new();
/// server.decline("the node identity did not load");
/// let report = doctor_at(Some(Ok(enabled)), None, Some(&server.status()));
/// let runtime = report.checks.iter().find(|c| c.check == "runtime").unwrap();
/// assert!(!runtime.ok && !report.ok);
/// assert!(runtime.detail.ends_with("the node identity did not load"));
/// ```
pub fn doctor_at(
    declaration: Option<Result<MeshConfig, MeshError>>,
    root: Option<&Path>,
    runtime: Option<&MeshStatus>,
) -> MeshDoctorReport {
    let mut checks = Vec::new();

    // The declaration: present, parseable, and what it says.
    match &declaration {
        None => checks.push(DoctorCheck::pass(
            "declaration",
            "absent: the mesh is off, which is the default posture",
        )),
        Some(Err(e)) => checks.push(DoctorCheck::fail(
            "declaration",
            e.to_string(),
            "the mesh stays off: no discovery socket opens and no link is made",
            "fix the declaration under .ai/repo/mesh/ (majordomus check names the schema violation), then restart the server",
        )),
        Some(Ok(config)) => checks.push(DoctorCheck::pass(
            "declaration",
            format!(
                "{}: enabled={}, multicast={}, broadcast={}, rendezvous endpoints={}, trust={} ({} allowed key(s)), cooperation={} (heartbeat {}s, expiry {}s, {} seed(s))",
                config.id,
                config.enabled,
                config.multicast.enabled,
                config.broadcast.mode != super::config::BroadcastMode::Disabled,
                config.rendezvous.endpoints.len(),
                config.trust.policy.as_str(),
                config.trust.allow.len(),
                config.cooperation.enabled,
                config.cooperation.heartbeat_seconds,
                config.cooperation.expiry_seconds,
                config.cooperation.seeds.len()
            ),
        )),
    }

    // The identity: where it lives, and whether what is there loads.
    let identity = match default_identity_path() {
        None => {
            checks.push(DoctorCheck::fail(
                "identity",
                "no HOME and no XDG_STATE_HOME: this process has nowhere to keep a node identity",
                "the mesh cannot activate: a node without a key cannot be told apart from an impostor",
                "run the server with HOME or XDG_STATE_HOME set",
            ));
            None
        }
        Some(path) if path.is_file() => match NodeIdentity::load_or_create(&path) {
            Ok(identity) => {
                checks.push(DoctorCheck::pass(
                    "identity",
                    format!("{} at {}", identity.public.node_id, path.display()),
                ));
                Some(identity)
            }
            Err(e) => {
                checks.push(DoctorCheck::fail(
                    "identity",
                    e.to_string(),
                    "the mesh cannot activate with a broken identity, and replacing it silently would make this machine a new node every peer distrusts",
                    "restore the file from a backup, or remove it deliberately — a new key is a new node, and every allowlist naming the old key must be updated",
                ));
                None
            }
        },
        Some(path) => {
            checks.push(DoctorCheck::pass(
                "identity",
                format!("absent; created at first activation at {}", path.display()),
            ));
            None
        }
    };

    // Trust: under an allowlist-shaped declaration, is this machine's own key listed? A
    // fleet whose declaration forgot a machine links around it.
    if let (Some(Ok(config)), Some(identity)) = (&declaration, &identity) {
        if config.enabled && !config.trust.allow.is_empty() {
            let listed = config.trust.allow.contains(&identity.public.public_key);
            checks.push(if listed {
                DoctorCheck::pass("trust", "this machine's key is on the allowlist")
            } else {
                DoctorCheck::fail(
                    "trust",
                    format!(
                        "this machine's key {} is not among the {} allowed",
                        identity.public.public_key,
                        config.trust.allow.len()
                    ),
                    "the other machines of the fleet refuse this one's links as untrusted; this machine can still dial them if it trusts their keys",
                    "add the key above to trust.allow in the mesh declaration and commit it",
                )
            });
        }
    }

    // The repository's mesh identity: what links are matched on.
    if let Some(root) = root {
        let declared = declaration
            .as_ref()
            .and_then(|d| d.as_ref().ok())
            .and_then(|c| c.cooperation.repository.clone());
        checks.push(match super::repository::resolve(root, declared.as_deref()) {
            Ok(identity) => DoctorCheck::pass(
                "repository",
                format!(
                    "{} from {:?}: {}",
                    identity.id, identity.basis, identity.detail
                ),
            ),
            Err(e) => DoctorCheck::fail(
                "repository",
                e.to_string(),
                "cooperation stays off: without a repository identity no peer can be matched to this repository",
                "declare cooperation.repository in the mesh declaration, or fetch the full history (git fetch --unshallow)",
            ),
        });
    }

    // A UDP socket at all.
    checks.push(match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        Ok(socket) => DoctorCheck::pass(
            "udp",
            format!(
                "ephemeral bind ok ({})",
                socket
                    .local_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "?".into())
            ),
        ),
        Err(e) => DoctorCheck::fail(
            "udp",
            format!("cannot bind an ephemeral UDP socket: {e}"),
            "multicast and broadcast discovery cannot run; seeds and rendezvous still can",
            "allow UDP for this process (a sandbox or a firewall is refusing it), or declare cooperation.seeds",
        ),
    });

    // Multicast capability, against the declared (or default) group.
    let group_text = declaration
        .as_ref()
        .and_then(|d| d.as_ref().ok())
        .map(|c| c.multicast.group.clone())
        .unwrap_or_else(|| super::config::DEFAULT_GROUP.into());
    checks.push(match multicast_probe(&group_text) {
        Ok(()) => DoctorCheck::pass(
            "multicast",
            format!("joined and left {group_text} on an ephemeral socket"),
        ),
        Err(e) => DoctorCheck::fail(
            "multicast",
            e.to_string(),
            "runtimes on this segment will not hear each other by multicast",
            "check the interface's multicast route, or declare cooperation.seeds or rendezvous endpoints instead",
        ),
    });

    // Broadcast capability.
    checks.push(
        match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|s| s.set_broadcast(true)) {
            Ok(()) => DoctorCheck::pass("broadcast", "the broadcast flag sets"),
            Err(e) => DoctorCheck::fail(
                "broadcast",
                format!("cannot enable broadcast: {e}"),
                "the broadcast fallback cannot run",
                "leave broadcast disabled and rely on multicast, seeds or rendezvous",
            ),
        },
    );

    // The discovery protocol, end to end in memory: sign, encode, parse, verify.
    checks.push(match protocol_probe() {
        Ok(detail) => DoctorCheck::pass("protocol", detail),
        Err(e) => DoctorCheck::fail(
            "protocol",
            e,
            "no advertisement this executable writes would verify anywhere",
            "this is a defect of the executable: reinstall it and report the detail",
        ),
    });

    // The link protocol, end to end in memory: a hello signed, verified, refused when
    // tampered — the handshake's own cryptography, with no peer.
    checks.push(match link_probe() {
        Ok(detail) => DoctorCheck::pass("link", detail),
        Err(e) => DoctorCheck::fail(
            "link",
            e,
            "no link this executable opens would be admitted anywhere",
            "this is a defect of the executable: reinstall it and report the detail",
        ),
    });

    // The runtime: what the process's mesh runtime decided about the declaration, when
    // anything decided. Every check above says the machine could run a mesh; this one says
    // whether the server did.
    checks.push(runtime_check(declared(&declaration), runtime));

    MeshDoctorReport {
        ok: checks.iter().all(|c| c.ok),
        checks,
    }
}

/// What the declaration says about the mesh, as far as the `runtime` verdict needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Declared {
    /// No mesh-declaration object in the layer.
    Absent,
    /// An object that does not parse.
    Broken,
    /// `enabled: false`.
    Disabled,
    /// `enabled: true`.
    Enabled,
}

fn declared(declaration: &Option<Result<MeshConfig, MeshError>>) -> Declared {
    match declaration {
        None => Declared::Absent,
        Some(Err(_)) => Declared::Broken,
        Some(Ok(config)) if config.enabled => Declared::Enabled,
        Some(Ok(_)) => Declared::Disabled,
    }
}

/// The `runtime` verdict. `None` is a runtime nobody decided on — the command line's, a
/// test's — whose inactivity is an absence and never a failure. A decided runtime is
/// active (a degraded one names its failed providers, and fails only when no provider
/// runs at all), off as declared, off because there is nothing to activate, or — the one
/// failure — off under an enabled declaration, carrying the server's reason.
fn runtime_check(declared: Declared, runtime: Option<&MeshStatus>) -> DoctorCheck {
    let Some(status) = runtime else {
        return DoctorCheck::pass(
            "runtime",
            match declared {
                Declared::Enabled => {
                    "not decided in this process: the mesh lives in the shared server, and `majordomus mesh doctor` asks the server when one serves this checkout"
                }
                Declared::Disabled => {
                    "not decided in this process, and nothing to decide: the declaration is disabled"
                }
                Declared::Broken => {
                    "not decided in this process, and nothing to decide: the declaration does not parse"
                }
                Declared::Absent => {
                    "not decided in this process, and nothing to decide: the repository declares no mesh"
                }
            },
        );
    };
    let reason = status.reason.as_deref().unwrap_or("no reason recorded");
    let why = reason.strip_prefix("not active: ").unwrap_or(reason);
    if !status.active {
        return match declared {
            Declared::Enabled => DoctorCheck::fail(
                "runtime",
                format!("the declaration is enabled and this server's mesh is not active — {why}"),
                "this checkout's server announces nothing, hears nobody and links to no peer, while the committed declaration says the fleet sees it",
                "fix what the reason names, then restart the server (`majordomus serve stop && majordomus serve ensure`)",
            ),
            Declared::Disabled => DoctorCheck::pass("runtime", format!("off, as declared — {why}")),
            Declared::Broken => DoctorCheck::pass(
                "runtime",
                format!("off: the declaration does not parse, so nothing was activated — {why}"),
            ),
            Declared::Absent => DoctorCheck::pass(
                "runtime",
                format!("off: the repository declares no mesh — {why}"),
            ),
        };
    }
    let running = status
        .providers
        .iter()
        .filter(|p| matches!(p.state, MeshProviderState::Running))
        .count();
    let failed: Vec<String> = status
        .providers
        .iter()
        .filter(|p| matches!(p.state, MeshProviderState::Failed))
        .map(|p| format!("{} ({})", p.id, p.detail.as_deref().unwrap_or("no detail")))
        .collect();
    let node = status
        .identity
        .as_ref()
        .map(|i| i.node_id.to_string())
        .unwrap_or_else(|| "?".into());
    let mut detail = format!(
        "active as {node} since {}: {running} of {} provider(s) running; {} node(s) known, {} trusted, {} present",
        status.started_at.as_deref().unwrap_or("?"),
        status.providers.len(),
        status.tallies.nodes,
        status.tallies.trusted,
        status.tallies.present
    );
    if !failed.is_empty() {
        detail.push_str(&format!("; failed: {}", failed.join(", ")));
    }
    // A provider that failed beside one that runs is a degraded mesh, named and holding;
    // every declared transport failed is a mesh that hears nothing and is heard by nobody.
    if failed.is_empty() || running > 0 {
        DoctorCheck::pass("runtime", detail)
    } else {
        DoctorCheck::fail(
            "runtime",
            detail,
            "the mesh is active and every discovery transport it declares has failed: it hears nothing and is heard by nobody",
            "read each failed provider's detail (`majordomus mesh status`), fix what it names, then restart the server",
        )
    }
}

fn multicast_probe(group_text: &str) -> Result<(), MeshError> {
    let group: Ipv4Addr = group_text
        .parse()
        .map_err(|_| MeshError::Provider(format!("'{group_text}' is not an IPv4 group")))?;
    if !group.is_multicast() {
        return Err(MeshError::Provider(format!(
            "{group} is not a multicast address"
        )));
    }
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .map_err(|e| MeshError::Provider(format!("cannot bind: {e}")))?;
    socket
        .join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED)
        .map_err(|e| MeshError::Provider(format!("cannot join {group}: {e}")))?;
    let _ = socket.leave_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED);
    Ok(())
}

fn protocol_probe() -> Result<String, String> {
    let identity = NodeIdentity::ephemeral().map_err(|e| e.to_string())?;
    let envelope = protocol::advertise_as(
        &identity,
        "0000000000000001",
        1,
        &["127.0.0.1:1".into()],
        &["http".into()],
        &[],
        "self-check",
    );
    let bytes = protocol::encode(&envelope).map_err(|e| e.to_string())?;
    let parsed = protocol::parse(&bytes).map_err(|r| format!("a fresh envelope refused: {r}"))?;
    if parsed.adv.node_id() != Some(identity.public.node_id.clone()) {
        return Err("the parsed envelope proves a different node".into());
    }
    Ok(format!(
        "sign → encode ({} bytes) → parse → verify, protocol {}",
        bytes.len(),
        protocol::PROTOCOL_VERSION
    ))
}

fn link_probe() -> Result<String, String> {
    use super::link::{sign, verify_signed, Domain, LINK_PROTOCOL_MAX, LINK_PROTOCOL_MIN};
    let identity = NodeIdentity::ephemeral().map_err(|e| e.to_string())?;
    let mut signed = sign(
        &identity,
        Domain::Hello,
        serde_json::json!({"probe": super::link::fresh_token()}),
    );
    if !verify_signed(&identity.public.public_key, Domain::Hello, &signed) {
        return Err("a fresh hello does not verify".into());
    }
    signed.body = serde_json::json!({"probe": "tampered"});
    if verify_signed(&identity.public.public_key, Domain::Hello, &signed) {
        return Err("a tampered hello verifies".into());
    }
    Ok(format!(
        "hello signed → verified → tampered copy refused, link protocol {LINK_PROTOCOL_MIN}..{LINK_PROTOCOL_MAX}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_self_check_runs_without_a_declaration_and_without_a_network() {
        let report = doctor(None);
        let names: Vec<&str> = report.checks.iter().map(|c| c.check.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "declaration",
                "identity",
                "udp",
                "multicast",
                "broadcast",
                "protocol",
                "link",
                "runtime"
            ]
        );
        for check in ["protocol", "link", "runtime"] {
            let c = report.checks.iter().find(|c| c.check == check).unwrap();
            assert!(c.ok, "{}", c.detail);
        }
    }

    #[test]
    fn a_parsed_declaration_is_summarised_in_the_verdict() {
        let config: MeshConfig = serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "docs", "enabled": true
        }))
        .unwrap();
        let report = doctor(Some(Ok(config)));
        let declaration = report
            .checks
            .iter()
            .find(|c| c.check == "declaration")
            .unwrap();
        assert!(declaration.ok);
        assert!(
            declaration.detail.contains("enabled=true"),
            "{}",
            declaration.detail
        );
        assert!(declaration.detail.contains("trust=deny_unknown"));
        assert!(declaration.detail.contains("cooperation=true"));
    }

    #[test]
    fn a_broken_declaration_is_one_failed_check_with_its_impact_and_remedy() {
        let report = doctor(Some(Err(MeshError::Config("bad".into()))));
        let declaration = report
            .checks
            .iter()
            .find(|c| c.check == "declaration")
            .unwrap();
        assert!(!declaration.ok);
        assert!(declaration.impact.is_some() && declaration.remediation.is_some());
        assert!(!report.ok);
    }

    #[test]
    fn a_repository_without_an_identity_is_named_with_the_remedy() {
        let dir = tempfile::tempdir().unwrap();
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        let report = doctor_at(None, Some(dir.path()), None);
        let repository = report
            .checks
            .iter()
            .find(|c| c.check == "repository")
            .unwrap();
        assert!(!repository.ok, "an empty repository has no root commit");
        assert!(repository
            .remediation
            .as_deref()
            .unwrap_or_default()
            .contains("cooperation.repository"));
    }

    fn config(enabled: bool) -> MeshConfig {
        serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "docs", "enabled": enabled
        }))
        .unwrap()
    }

    fn check<'a>(report: &'a MeshDoctorReport, name: &str) -> &'a DoctorCheck {
        report.checks.iter().find(|c| c.check == name).unwrap()
    }

    #[test]
    fn an_enabled_declaration_nobody_decided_on_is_an_absence_not_a_failure() {
        let report = doctor_at(Some(Ok(config(true))), None, None);
        let runtime = check(&report, "runtime");
        assert!(runtime.ok, "{}", runtime.detail);
        assert!(
            runtime.detail.starts_with("not decided in this process")
                && runtime.detail.contains("asks the server"),
            "{}",
            runtime.detail
        );
    }

    #[test]
    fn an_enabled_declaration_a_server_declined_fails_with_the_servers_reason_and_the_remedy() {
        let server = super::super::MeshRuntime::new();
        server.decline("the node identity did not load: permission denied");
        let report = doctor_at(Some(Ok(config(true))), None, Some(&server.status()));
        let runtime = check(&report, "runtime");
        assert!(!runtime.ok);
        assert_eq!(
            runtime.detail,
            "the declaration is enabled and this server's mesh is not active — the node identity did not load: permission denied"
        );
        assert!(runtime.impact.is_some());
        assert!(runtime
            .remediation
            .as_deref()
            .unwrap_or_default()
            .contains("majordomus serve ensure"));
        assert!(!report.ok);
    }

    #[test]
    fn a_disabled_declaration_a_server_declined_is_off_as_declared() {
        let server = super::super::MeshRuntime::new();
        server.decline("the mesh declaration is disabled");
        let report = doctor_at(Some(Ok(config(false))), None, Some(&server.status()));
        let runtime = check(&report, "runtime");
        assert!(runtime.ok, "{}", runtime.detail);
        assert_eq!(
            runtime.detail,
            "off, as declared — the mesh declaration is disabled"
        );
    }

    #[test]
    fn a_broken_or_absent_declaration_is_not_the_runtimes_failure() {
        let server = super::super::MeshRuntime::new();
        server.decline("the declaration does not parse: bad");
        let broken = doctor_at(
            Some(Err(MeshError::Config("bad".into()))),
            None,
            Some(&server.status()),
        );
        let runtime = check(&broken, "runtime");
        assert!(
            runtime.ok,
            "the declaration check carries that failure, once"
        );
        assert!(runtime
            .detail
            .starts_with("off: the declaration does not parse"));
        assert!(!check(&broken, "declaration").ok);
        let absent = doctor_at(None, None, None);
        assert!(check(&absent, "runtime")
            .detail
            .ends_with("the repository declares no mesh"));
    }

    #[test]
    fn a_server_that_activated_its_mesh_is_active_and_every_provider_failing_is_a_failure() {
        let server = super::super::MeshRuntime::new();
        let dir = tempfile::tempdir().unwrap();
        let identity = NodeIdentity::load_or_create(&dir.path().join("node.json")).unwrap();
        let mut quiet = config(true);
        quiet.multicast.enabled = false;
        server
            .activate(&quiet, identity, vec![], vec![], "test")
            .unwrap();
        let mut status = server.status();
        server.stop();
        let active = doctor_at(Some(Ok(config(true))), None, Some(&status));
        let runtime = check(&active, "runtime");
        assert!(runtime.ok, "{}", runtime.detail);
        assert!(
            runtime.detail.starts_with("active as ")
                && runtime.detail.contains(": 0 of 0 provider(s) running;"),
            "{}",
            runtime.detail
        );
        status.providers.push(super::super::ProviderStatus {
            id: "udp_multicast".into(),
            state: MeshProviderState::Failed,
            detail: Some("address in use".into()),
            sent: 0,
            received: 0,
        });
        let deaf = doctor_at(Some(Ok(config(true))), None, Some(&status));
        let runtime = check(&deaf, "runtime");
        assert!(!runtime.ok);
        assert!(
            runtime
                .detail
                .contains("failed: udp_multicast (address in use)"),
            "{}",
            runtime.detail
        );
    }
}
