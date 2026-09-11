//! The `mesh` module: the discovered nodes of this process's mesh runtime, projected.
//! Every capability here reads [`crate::mesh::MeshRuntime`] on the context or the node
//! identity file; none holds state of its own, and none grants anything — a listed node
//! is an observation, not an authorization (ADR 0050).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CapabilityKind, CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::mesh::rendezvous::REGISTER_PATH;
use crate::mesh::{
    default_identity_path, MeshConfig, MeshDoctorReport, MeshStatus, NodeIdentity, NodeRecord,
    PublicIdentity, RegisterAnswer,
};
use crate::{capability, module};

use super::{get, mcp, post, Empty};

/// The MCP resource `mesh.status` is projected as.
pub const MESH_URI: &str = "majordomus://mesh";

// ---------------------------------------------------------------- mesh.status

fn mesh_status(ctx: &Context, _: Empty) -> Result<MeshStatus, CapabilityError> {
    Ok(ctx.mesh.status())
}

// ---------------------------------------------------------------- mesh.nodes

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `mesh.nodes`: every node this process has observed.
pub struct NodeList {
    /// How many nodes the registry holds.
    pub count: usize,
    /// The nodes, in node-id order — the one canonical order every surface shows.
    pub nodes: Vec<NodeRecord>,
}

fn mesh_nodes(ctx: &Context, _: Empty) -> Result<NodeList, CapabilityError> {
    let nodes = ctx.mesh.nodes();
    Ok(NodeList {
        count: nodes.len(),
        nodes,
    })
}

// ---------------------------------------------------------------- mesh.identity

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `mesh.identity`: this machine's node identity, public half only.
pub struct MeshIdentityReport {
    /// Where the identity file lives (or would).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Whether the file exists.
    pub present: bool,
    /// The public identity, when the file exists and loads. The signing key is not
    /// here, not in any projection, and not in any log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<PublicIdentity>,
    /// Why the identity did not load, when it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn mesh_identity(_: &Context, _: Empty) -> Result<MeshIdentityReport, CapabilityError> {
    let Some(path) = default_identity_path() else {
        return Ok(MeshIdentityReport {
            path: None,
            present: false,
            identity: None,
            error: Some("no HOME and no XDG_STATE_HOME: nowhere to keep a node identity".into()),
        });
    };
    if !path.is_file() {
        return Ok(MeshIdentityReport {
            path: Some(path.display().to_string()),
            present: false,
            identity: None,
            error: None,
        });
    }
    match NodeIdentity::load_or_create(&path) {
        Ok(identity) => Ok(MeshIdentityReport {
            path: Some(path.display().to_string()),
            present: true,
            identity: Some(identity.public),
            error: None,
        }),
        Err(e) => Ok(MeshIdentityReport {
            path: Some(path.display().to_string()),
            present: true,
            identity: None,
            error: Some(e.to_string()),
        }),
    }
}

// ---------------------------------------------------------------- mesh.doctor

fn mesh_doctor(ctx: &Context, _: Empty) -> Result<MeshDoctorReport, CapabilityError> {
    Ok(crate::mesh::doctor::doctor(declaration(ctx)))
}

/// The repository's mesh declaration, as the index discovered it: `None` when no object
/// of the kind exists, the parse verdict when one does.
pub fn declaration(ctx: &Context) -> Option<Result<MeshConfig, crate::mesh::MeshError>> {
    ctx.index
        .objects
        .iter()
        .find(|o| o.kind == crate::mesh::KIND)
        .map(MeshConfig::parse)
}

// ---------------------------------------------------------------- mesh.register

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `mesh.register`: one signed envelope, exactly as the wire carries it.
pub struct RegisterInput {
    /// The envelope. Verified here exactly as a datagram would be — bounds, staleness,
    /// signature — before anything is recorded.
    pub envelope: serde_json::Value,
}

impl BenchmarkCases for RegisterInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // A malformed envelope: the handler answers `accepted: false` without touching
        // the registry, which is exactly the cheap, side-effect-free path to measure.
        vec![NamedCase::new(
            "refused",
            RegisterInput {
                envelope: serde_json::json!({"v": 0}),
            },
        )]
    }
}

fn mesh_register(ctx: &Context, input: RegisterInput) -> Result<RegisterAnswer, CapabilityError> {
    Ok(ctx.mesh.register(&input.envelope, "http"))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "mesh",
        title: "Mesh",
        description: "The nodes this process discovered on the mesh: authenticated observations of other running Majordomus instances, converged into one registry, with the providers that heard them and the trust the policy assigned. Observation, never authority: a listed node can execute nothing here.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "mesh.status",
                title: "The mesh, at a glance",
                description: "Whether the mesh runs in this process and why not when it does not; this node's public identity; every discovery provider with its state and counters; the registry's tallies; and the refused datagrams by reason. The one status every surface shows.",
                input: Empty,
                output: MeshStatus,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_mesh".into()),
                        resource: Some(crate::capability::model::McpResource {
                            uri: MESH_URI.into(),
                            name: "mesh".into(),
                        }),
                    }),
                    http: get("/api/v1/mesh"),
                    cli: None,
                },
                tags: ["mesh", "coordination", "discovery"],
                handler: mesh_status,
            },
            capability! {
                id: "mesh.nodes",
                title: "The discovered nodes",
                description: "Every node this process has observed, deduplicated by node identity across every discovery source, in node-id order: identity, trust, presence, endpoints, capabilities, repositories, provenance and versions. In memory, gone with the process.",
                input: Empty,
                output: NodeList,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_nodes"), http: get("/api/v1/mesh/nodes"), cli: None },
                tags: ["mesh", "coordination", "discovery"],
                handler: mesh_nodes,
            },
            capability! {
                id: "mesh.identity",
                title: "This machine's node identity",
                description: "The node identity kept under the user's state directory, public half only: node id, public key, display name. The signing key appears in no projection. Absent is an answer, not an error — the identity is created when a mesh first activates.",
                input: Empty,
                output: MeshIdentityReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_identity"),
                    http: get("/api/v1/mesh/identity"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "identity".into()] }),
                },
                tags: ["mesh", "identity"],
                handler: mesh_identity,
            },
            capability! {
                id: "mesh.doctor",
                title: "The mesh self-check",
                description: "Every prerequisite proved on this machine alone: the declaration parses, the identity loads, a UDP socket binds, the multicast group joins, broadcast enables, and the protocol signs, encodes, parses and verifies end to end in memory. Deterministic, no second node required.",
                input: Empty,
                output: MeshDoctorReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_mesh_doctor"),
                    http: get("/api/v1/mesh/doctor"),
                    cli: Some(CliExposure { path: vec!["mesh".into(), "doctor".into()] }),
                },
                tags: ["mesh", "diagnostics"],
                handler: mesh_doctor,
            },
            capability! {
                id: "mesh.register",
                kind: CapabilityKind::Command,
                title: "Register with this node's mesh",
                description: "Present one signed envelope; it is verified exactly as a datagram — bounds, staleness, signature, trust policy — and recorded as a rendezvous observation when it holds. The answer carries this node's own envelope and the candidates its registry holds, each verifiable end to end on its own signature. Registration grants nothing: the caller becomes a record, never an authorization. Changes this process's memory only.",
                input: RegisterInput,
                output: RegisterAnswer,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_mesh_register"), http: post(REGISTER_PATH), cli: None },
                tags: ["mesh", "coordination", "discovery"],
                handler: mesh_register,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; this is the assertion a
    /// refactor that dropped a projection would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "mesh");
        let expected: &[(&str, &str, &str)] = &[
            ("mesh.status", "majordomus_mesh", "/api/v1/mesh"),
            ("mesh.nodes", "majordomus_mesh_nodes", "/api/v1/mesh/nodes"),
            (
                "mesh.identity",
                "majordomus_mesh_identity",
                "/api/v1/mesh/identity",
            ),
            (
                "mesh.doctor",
                "majordomus_mesh_doctor",
                "/api/v1/mesh/doctor",
            ),
            ("mesh.register", "majordomus_mesh_register", REGISTER_PATH),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
        }
    }

    #[test]
    fn only_register_is_a_command() {
        for e in module().capabilities {
            let is_command = e.capability.kind == CapabilityKind::Command;
            assert_eq!(
                is_command,
                e.capability.id.as_str() == "mesh.register",
                "{}: discovery surfaces are read-only; register is the one command",
                e.capability.id
            );
        }
    }
}
