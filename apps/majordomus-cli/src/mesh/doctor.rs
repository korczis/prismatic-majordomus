//! The mesh self-check: every prerequisite proved on this machine alone, no second node
//! required. Deterministic checks in a fixed order, each with its own verdict, so "why
//! is the mesh not working" has an answer that names the broken link instead of a
//! shrug — and, when a check fails or limits cooperation, what that means and what to do.
//! Read-only toward the repository; the sockets it probes are ephemeral and closed before
//! it answers.
//!
//! What this cannot see is the running server: whether its cooperation thread beats and
//! whether its peers answer. That is `mesh verify`, which asks the server.
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
use super::protocol;
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

/// Run the self-check against the declaration as parsed (or its absence, or its error).
/// Deterministic order, no second node required, no repository writes.
pub fn doctor(declaration: Option<Result<MeshConfig, MeshError>>) -> MeshDoctorReport {
    doctor_at(declaration, None)
}

/// [`doctor`] for the repository at `root`: adds whether the repository has a mesh
/// identity cooperation can match on.
///
/// Discovery works without one — two nodes can hear each other while belonging to
/// different repositories — but a link does not, so a report that omits this check can say
/// every prerequisite holds while cooperation is unreachable. `root` is optional because
/// the caller may have no repository at hand, and then the check is not run rather than
/// failed.
///
/// ```
/// use majordomus_cli::mesh::doctor::doctor_at;
///
/// // a directory holding no history has no identity to match on, and the check says so
/// let nowhere = tempfile::tempdir().unwrap();
/// let report = doctor_at(None, Some(nowhere.path()));
/// let repository = report.checks.iter().find(|c| c.check == "repository").unwrap();
/// assert!(!repository.ok);
/// assert!(repository.remediation.as_deref().unwrap().contains("cooperation.repository"));
///
/// // with no root to ask about, the question is not asked at all
/// assert!(doctor_at(None, None).checks.iter().all(|c| c.check != "repository"));
/// ```
pub fn doctor_at(
    declaration: Option<Result<MeshConfig, MeshError>>,
    root: Option<&Path>,
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

    MeshDoctorReport {
        ok: checks.iter().all(|c| c.ok),
        checks,
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
                "link"
            ]
        );
        for check in ["protocol", "link"] {
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
        let report = doctor_at(None, Some(dir.path()));
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
}
