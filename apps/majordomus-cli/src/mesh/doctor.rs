//! The mesh self-check: every prerequisite proved on this machine alone, no second node
//! required. Deterministic checks in a fixed order, each with its own verdict, so "why
//! is the mesh not working" has an answer that names the broken link instead of a
//! shrug. Read-only toward the repository; the sockets it probes are ephemeral and
//! closed before it answers.

use std::net::{Ipv4Addr, UdpSocket};

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
pub fn doctor(declaration: Option<Result<MeshConfig, MeshError>>) -> MeshDoctorReport {
    let mut checks = Vec::new();

    // The declaration: present, parseable, and what it says.
    match &declaration {
        None => checks.push(DoctorCheck {
            check: "declaration".into(),
            ok: true,
            detail: "absent: the mesh is off, which is the default posture".into(),
        }),
        Some(Err(e)) => checks.push(DoctorCheck {
            check: "declaration".into(),
            ok: false,
            detail: e.to_string(),
        }),
        Some(Ok(config)) => checks.push(DoctorCheck {
            check: "declaration".into(),
            ok: true,
            detail: format!(
                "{}: enabled={}, multicast={}, broadcast={}, rendezvous endpoints={}, trust={}",
                config.id,
                config.enabled,
                config.multicast.enabled,
                config.broadcast.mode != super::config::BroadcastMode::Disabled,
                config.rendezvous.endpoints.len(),
                config.trust.policy.as_str()
            ),
        }),
    }

    // The identity: where it lives, and whether what is there loads.
    match default_identity_path() {
        None => checks.push(DoctorCheck {
            check: "identity".into(),
            ok: false,
            detail: "no HOME and no XDG_STATE_HOME: this process has nowhere to keep a node identity".into(),
        }),
        Some(path) if path.is_file() => match NodeIdentity::load_or_create(&path) {
            Ok(identity) => checks.push(DoctorCheck {
                check: "identity".into(),
                ok: true,
                detail: format!("{} at {}", identity.public.node_id, path.display()),
            }),
            Err(e) => checks.push(DoctorCheck {
                check: "identity".into(),
                ok: false,
                detail: e.to_string(),
            }),
        },
        Some(path) => checks.push(DoctorCheck {
            check: "identity".into(),
            ok: true,
            detail: format!("absent; created at first activation at {}", path.display()),
        }),
    }

    // A UDP socket at all.
    checks.push(match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        Ok(socket) => DoctorCheck {
            check: "udp".into(),
            ok: true,
            detail: format!(
                "ephemeral bind ok ({})",
                socket
                    .local_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "?".into())
            ),
        },
        Err(e) => DoctorCheck {
            check: "udp".into(),
            ok: false,
            detail: format!("cannot bind an ephemeral UDP socket: {e}"),
        },
    });

    // Multicast capability, against the declared (or default) group.
    let group_text = declaration
        .as_ref()
        .and_then(|d| d.as_ref().ok())
        .map(|c| c.multicast.group.clone())
        .unwrap_or_else(|| super::config::DEFAULT_GROUP.into());
    checks.push(match multicast_probe(&group_text) {
        Ok(()) => DoctorCheck {
            check: "multicast".into(),
            ok: true,
            detail: format!("joined and left {group_text} on an ephemeral socket"),
        },
        Err(e) => DoctorCheck {
            check: "multicast".into(),
            ok: false,
            detail: e.to_string(),
        },
    });

    // Broadcast capability.
    checks.push(
        match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|s| s.set_broadcast(true)) {
            Ok(()) => DoctorCheck {
                check: "broadcast".into(),
                ok: true,
                detail: "the broadcast flag sets".into(),
            },
            Err(e) => DoctorCheck {
                check: "broadcast".into(),
                ok: false,
                detail: format!("cannot enable broadcast: {e}"),
            },
        },
    );

    // The protocol, end to end in memory: sign, encode, parse, verify.
    checks.push(match protocol_probe() {
        Ok(detail) => DoctorCheck {
            check: "protocol".into(),
            ok: true,
            detail,
        },
        Err(e) => DoctorCheck {
            check: "protocol".into(),
            ok: false,
            detail: e,
        },
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
        return Err(MeshError::Provider(format!("{group} is not a multicast address")));
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
    let envelope = protocol::advertise(
        &identity,
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
    Ok(format!("sign → encode ({} bytes) → parse → verify", bytes.len()))
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
            vec!["declaration", "identity", "udp", "multicast", "broadcast", "protocol"]
        );
        let protocol = report.checks.iter().find(|c| c.check == "protocol").unwrap();
        assert!(protocol.ok, "{}", protocol.detail);
    }

    #[test]
    fn a_broken_declaration_is_one_failed_check_not_a_crash() {
        let report = doctor(Some(Err(MeshError::Config("bad".into()))));
        let declaration = report.checks.iter().find(|c| c.check == "declaration").unwrap();
        assert!(!declaration.ok);
        assert!(!report.ok);
    }
}
