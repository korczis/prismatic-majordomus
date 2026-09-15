//! The mesh self-check: every prerequisite proved on this machine alone, no second node
//! required. Deterministic checks in a fixed order, each with its own verdict, so "why
//! is the mesh not working" has an answer that names the broken link instead of a
//! shrug. Read-only toward the repository; the sockets it probes are ephemeral and
//! closed before it answers.
//!
//! Two checks read what the process it runs in has decided rather than what the machine
//! can do. `trust` asks whether this machine's own key is on the allowlist the
//! declaration carries — the operator's mistake that makes a node visible everywhere
//! and trusted nowhere. `runtime` asks the mesh runtime of this process: in a shared
//! server, whether an enabled declaration actually activated, and why not when it did
//! not; in any other process nothing activates the mesh, and the check says so instead
//! of judging an absence. `majordomus mesh doctor` therefore asks the running server
//! when one serves the checkout, so that the verdict is the server's (ADR 0059).
//!
//! ```
//! use majordomus_cli::mesh::doctor::doctor;
//!
//! // No declaration is the default posture, and the self-check says so and runs on.
//! let report = doctor(None, None);
//! let declaration = report.checks.iter().find(|c| c.check == "declaration").unwrap();
//! assert!(declaration.ok);
//! assert!(declaration.detail.contains("default posture"));
//! ```

use std::net::{Ipv4Addr, UdpSocket};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::config::MeshConfig;
use super::identity::{default_identity_path, NodeIdentity};
use super::manager::MeshStatus;
use super::protocol;
use super::provider::MeshProviderState;
use super::trust::TrustPolicy;
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

/// Run the self-check against the declaration as parsed (or its absence, or its error),
/// and against the mesh runtime of this process when it has decided anything (`None`
/// for a process no shared server runs in). Deterministic order, no second node
/// required, no repository writes.
pub fn doctor(
    declaration: Option<Result<MeshConfig, MeshError>>,
    runtime: Option<&MeshStatus>,
) -> MeshDoctorReport {
    let mut checks = Vec::new();
    let config = declaration.as_ref().and_then(|d| d.as_ref().ok());

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

    // The identity: where it lives, and whether what is there loads. The public key is
    // kept for the trust check below.
    let mut own_key: Option<String> = None;
    match default_identity_path() {
        None => checks.push(DoctorCheck {
            check: "identity".into(),
            ok: false,
            detail:
                "no HOME and no XDG_STATE_HOME: this process has nowhere to keep a node identity"
                    .into(),
        }),
        Some(path) if path.is_file() => match NodeIdentity::load_or_create(&path) {
            Ok(identity) => {
                own_key = Some(identity.public.public_key.clone());
                checks.push(DoctorCheck {
                    check: "identity".into(),
                    ok: true,
                    detail: format!("{} at {}", identity.public.node_id, path.display()),
                })
            }
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

    // Trust: the allowlist names keys of the right shape, and this machine is on it. A
    // node that is not on its own repository's allowlist is observed by every peer and
    // trusted by none of them, and nothing else on this machine would ever say so.
    checks.push(trust_check(config, own_key.as_deref()));

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

    // The runtime: what this process decided about the declaration, when it decided
    // anything. Every check above says the machine *could* run a mesh; this one says
    // whether the server did.
    checks.push(runtime_check(config, runtime));

    MeshDoctorReport {
        ok: checks.iter().all(|c| c.ok),
        checks,
    }
}

/// The `trust` verdict: under an allowlist, every key is 64 hex characters and this
/// machine's own key is among them; under `tofu`, a note that every listing will name
/// it; with nothing declared, the default posture.
fn trust_check(config: Option<&MeshConfig>, own_key: Option<&str>) -> DoctorCheck {
    let Some(config) = config else {
        return DoctorCheck {
            check: "trust".into(),
            ok: true,
            detail: "no declaration: deny_unknown with nobody trusted, the default posture".into(),
        };
    };
    let allow = &config.trust.allow;
    let malformed: Vec<&String> = allow
        .iter()
        .filter(|k| k.len() != 64 || !k.chars().all(|c| c.is_ascii_hexdigit()))
        .collect();
    if let Some(bad) = malformed.first() {
        return DoctorCheck {
            check: "trust".into(),
            ok: false,
            detail: format!(
                "trust.allow carries '{}', which is not an Ed25519 public key (64 hex characters); `majordomus mesh identity` prints a machine's",
                truncate(bad, 24)
            ),
        };
    }
    if config.trust.policy == TrustPolicy::Tofu {
        return DoctorCheck {
            check: "trust".into(),
            ok: true,
            detail: format!(
                "tofu: the first key seen under a node id is trusted, and every listing names it as such; {} key(s) allowed regardless",
                allow.len()
            ),
        };
    }
    if allow.is_empty() {
        return DoctorCheck {
            check: "trust".into(),
            ok: true,
            detail: format!(
                "{}: nobody is trusted; every node is observed and trusted for nothing",
                config.trust.policy.as_str()
            ),
        };
    }
    match own_key {
        None => DoctorCheck {
            check: "trust".into(),
            ok: true,
            detail: format!(
                "{} key(s) allowed; this machine has no identity yet, so whether it is among them is decided at first activation",
                allow.len()
            ),
        },
        Some(key) if allow.iter().any(|k| k.eq_ignore_ascii_case(key)) => DoctorCheck {
            check: "trust".into(),
            ok: true,
            detail: format!(
                "{} key(s) allowed under {}, this machine's among them",
                allow.len(),
                config.trust.policy.as_str()
            ),
        },
        Some(key) => DoctorCheck {
            check: "trust".into(),
            ok: false,
            detail: format!(
                "{} key(s) allowed under {}, and this machine's ({}…) is not one of them: every peer sees this node and trusts it for nothing; add the key `majordomus mesh identity` prints to trust.allow",
                allow.len(),
                config.trust.policy.as_str(),
                truncate(key, 8)
            ),
        },
    }
}

/// The `runtime` verdict, from what this process's mesh runtime decided. `None` is a
/// process no shared server runs in — the command line, a test — where nothing
/// activates the mesh and an absence is not a failure.
fn runtime_check(config: Option<&MeshConfig>, runtime: Option<&MeshStatus>) -> DoctorCheck {
    let enabled = config.is_some_and(|c| c.enabled);
    match runtime {
        None => DoctorCheck {
            check: "runtime".into(),
            ok: true,
            detail: if enabled {
                "not decided in this process: the mesh lives in the shared server, and `majordomus mesh doctor` asks the server when one serves this checkout".into()
            } else {
                "not decided in this process, and nothing to decide: the declaration is absent or disabled".into()
            },
        },
        Some(status) if status.active => {
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
                "active as {node} since {}: {} of {} provider(s) running; {} node(s) known, {} trusted, {} present",
                status.started_at.as_deref().unwrap_or("?"),
                running,
                status.providers.len(),
                status.tallies.nodes,
                status.tallies.trusted,
                status.tallies.present
            );
            if !failed.is_empty() {
                detail.push_str(&format!("; failed: {}", failed.join(", ")));
            }
            DoctorCheck {
                check: "runtime".into(),
                // A provider that failed beside one that runs is a degraded mesh and is
                // named; every declared transport failed is a mesh that hears nothing
                // and is heard by nobody, whatever the declaration says.
                ok: failed.is_empty() || running > 0,
                detail,
            }
        }
        Some(status) => {
            let reason = status.reason.as_deref().unwrap_or("no reason recorded");
            if enabled {
                DoctorCheck {
                    check: "runtime".into(),
                    ok: false,
                    detail: format!(
                        "the declaration is enabled and this server's mesh is not active — {reason}; restart the server (`majordomus serve stop && majordomus serve ensure`) after fixing what the reason names"
                    ),
                }
            } else {
                DoctorCheck {
                    check: "runtime".into(),
                    ok: true,
                    detail: format!("off, as declared — {reason}"),
                }
            }
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect()
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
    Ok(format!(
        "sign → encode ({} bytes) → parse → verify",
        bytes.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled(allow: &[&str], policy: &str) -> MeshConfig {
        serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "docs", "enabled": true,
            "trust": { "policy": policy, "allow": allow }
        }))
        .unwrap()
    }

    fn check<'a>(report: &'a MeshDoctorReport, name: &str) -> &'a DoctorCheck {
        report.checks.iter().find(|c| c.check == name).unwrap()
    }

    #[test]
    fn the_self_check_runs_without_a_declaration_and_without_a_network() {
        let report = doctor(None, None);
        let names: Vec<&str> = report.checks.iter().map(|c| c.check.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "declaration",
                "identity",
                "trust",
                "udp",
                "multicast",
                "broadcast",
                "protocol",
                "runtime"
            ]
        );
        assert!(check(&report, "runtime").ok);
        assert!(check(&report, "runtime")
            .detail
            .contains("nothing to decide"));
        let protocol = report
            .checks
            .iter()
            .find(|c| c.check == "protocol")
            .unwrap();
        assert!(protocol.ok, "{}", protocol.detail);
    }

    #[test]
    fn a_parsed_declaration_is_summarised_in_the_verdict() {
        let config: MeshConfig = serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "docs", "enabled": true
        }))
        .unwrap();
        let report = doctor(Some(Ok(config)), None);
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
    }

    #[test]
    fn a_broken_declaration_is_one_failed_check_not_a_crash() {
        let report = doctor(Some(Err(MeshError::Config("bad".into()))), None);
        let declaration = report
            .checks
            .iter()
            .find(|c| c.check == "declaration")
            .unwrap();
        assert!(!declaration.ok);
        assert!(!report.ok);
    }

    #[test]
    fn an_enabled_declaration_nobody_activated_here_is_not_a_failure_of_this_process() {
        let report = doctor(Some(Ok(enabled(&[], "deny_unknown"))), None);
        let runtime = check(&report, "runtime");
        assert!(runtime.ok, "{}", runtime.detail);
        assert!(
            runtime.detail.contains("asks the server"),
            "{}",
            runtime.detail
        );
    }

    #[test]
    fn an_enabled_declaration_a_server_did_not_activate_is_one_failed_check_naming_why() {
        let runtime = super::super::MeshRuntime::new();
        runtime.decline("the node identity did not load: permission denied");
        let status = runtime.status();
        let report = doctor(Some(Ok(enabled(&[], "deny_unknown"))), Some(&status));
        let check = check(&report, "runtime");
        assert!(!check.ok);
        assert!(
            check.detail.contains("permission denied"),
            "{}",
            check.detail
        );
        assert!(check.detail.contains("serve ensure"), "{}", check.detail);
        assert!(!report.ok);
    }

    #[test]
    fn a_disabled_declaration_a_server_declined_is_off_as_declared() {
        let runtime = super::super::MeshRuntime::new();
        runtime.decline("the mesh declaration is disabled");
        let status = runtime.status();
        let config: MeshConfig = serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "docs", "enabled": false
        }))
        .unwrap();
        let report = doctor(Some(Ok(config)), Some(&status));
        let check = check(&report, "runtime");
        assert!(check.ok, "{}", check.detail);
        assert!(
            check.detail.starts_with("off, as declared"),
            "{}",
            check.detail
        );
    }

    #[test]
    fn a_malformed_allowlist_key_fails_the_trust_check() {
        let report = doctor(Some(Ok(enabled(&["not-a-key"], "deny_unknown"))), None);
        let trust = check(&report, "trust");
        assert!(!trust.ok);
        assert!(trust.detail.contains("not-a-key"), "{}", trust.detail);
        assert!(!report.ok);
    }

    #[test]
    fn the_trust_check_wants_this_machine_on_its_own_allowlist() {
        let key = "ab".repeat(32);
        let other = "cd".repeat(32);
        // The identity file is the machine's, so the verdict on the key is asked directly.
        let on = trust_check(Some(&enabled(&[&key], "deny_unknown")), Some(&key));
        assert!(on.ok, "{}", on.detail);
        assert!(on.detail.contains("this machine's among them"));
        let off = trust_check(Some(&enabled(&[&other], "deny_unknown")), Some(&key));
        assert!(!off.ok);
        assert!(
            off.detail.contains("trusts it for nothing"),
            "{}",
            off.detail
        );
        let unknown = trust_check(Some(&enabled(&[&other], "deny_unknown")), None);
        assert!(unknown.ok, "{}", unknown.detail);
        let nobody = trust_check(Some(&enabled(&[], "deny_unknown")), Some(&key));
        assert!(nobody.ok);
        assert!(nobody.detail.contains("nobody is trusted"));
        let tofu = trust_check(Some(&enabled(&[&other], "tofu")), Some(&key));
        assert!(tofu.ok, "{}", tofu.detail);
    }
}
