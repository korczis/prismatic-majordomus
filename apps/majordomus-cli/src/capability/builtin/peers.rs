//! The `peers` module: the clients attached to this shared server, and what they say
//! they are working on. The board lives in memory; nothing here writes to the repository.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CapabilityKind, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::peers::{Announced, Overlap, Peer, PeerId};
use crate::{capability, module};

use super::{get, mcp, post, Empty};

// ---------------------------------------------------------------- peers.list

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `peers.list`: every client attached to this shared server.
pub struct PeerList {
    /// How many peers are attached, the caller included.
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The caller's own peer id, when the call came through an MCP session.
    pub caller: Option<PeerId>,
    /// The peers, in attachment order; `p1` started the server. A peer that announced
    /// something and then went away is still here, with `attached: false`.
    pub peers: Vec<Peer>,
    /// Every pair of peers whose claimed scope meets, each pair once. Empty is the
    /// ordinary case, and a reader who sees an entry here is looking at two sessions
    /// about to do the same work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlaps: Vec<Overlap>,
}

fn peers_list(ctx: &Context, _: Empty) -> Result<PeerList, CapabilityError> {
    let peers = ctx.peers.list();
    Ok(PeerList {
        count: peers.iter().filter(|p| p.attached).count(),
        caller: ctx.caller.clone(),
        peers,
        overlaps: ctx.peers.overlaps(),
    })
}

// ---------------------------------------------------------------- peers.announce

/// The MCP tool `peers.announce` is projected as. Named once, here, because the bridge
/// recognises the frame that carries an announcement by this name in order to replay it
/// after a takeover or a re-attach; the exposure below and the bridge read the same word.
pub const ANNOUNCE_TOOL: &str = "majordomus_announce";

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `peers.announce`: what the calling peer is working on.
pub struct AnnounceInput {
    /// One line, in the peer's words: the task, the question, the intent.
    pub intent: String,
    /// Repository-relative paths the peer expects to touch. Informational: other peers
    /// read it to avoid a collision; nothing here enforces it.
    #[serde(default)]
    pub scope: Vec<String>,
}

impl BenchmarkCases for AnnounceInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "default",
            AnnounceInput {
                intent: "benchmark: announcing".into(),
                scope: vec!["apps/majordomus-cli".into()],
            },
        )]
    }
}

fn peers_announce(ctx: &Context, input: AnnounceInput) -> Result<Announced, CapabilityError> {
    let Some(caller) = &ctx.caller else {
        return Err(CapabilityError::Refused(
            "announce needs an MCP session: this call came through an interface with no peer identity (call the majordomus_announce tool)".into(),
        ));
    };
    if input.intent.trim().is_empty() {
        return Err(CapabilityError::InvalidInput(
            "argument 'intent' is required and must not be blank".into(),
        ));
    }
    let scope: Vec<String> = input
        .scope
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    ctx.peers
        .announce(caller, input.intent.trim(), scope)
        .ok_or_else(|| CapabilityError::Internal(format!("peer {caller} is not attached")))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "peers",
        title: "Peers",
        description: "The clients attached to this repository's shared server, named by their own initialize, and what each announced it is working on. In memory; gone with the process.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "peers.list",
                title: "List peers",
                description: "Every client attached to this shared server: id, the client's own name and version from its initialize, transport, when it attached, when it was last seen, and what it announced. In-memory, gone with the process.",
                input: Empty,
                output: PeerList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_peers"), http: get("/api/v1/peers"), cli: None },
                tags: ["peers", "coordination"],
                handler: peers_list,
            },
            capability! {
                id: "peers.announce",
                kind: CapabilityKind::Command,
                title: "Announce what this peer is working on",
                description: "Tell the other peers of this shared server what the calling session is doing and which paths it expects to touch. Changes this process's memory only; the repository is never written. Needs an MCP session: over plain HTTP there is no caller.",
                input: AnnounceInput,
                output: Announced,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp(ANNOUNCE_TOOL), http: post("/api/v1/peers/announce"), cli: None },
                tags: ["peers", "coordination"],
                handler: peers_announce,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist, and every projection — the MCP
    /// tool, the HTTP route, the OpenAPI operation, the benchmark target — is derived from
    /// it. A refactor that dropped an exposure or renamed a route would still compile, and
    /// the suites that exercise the behaviour behind it would still pass. This is the
    /// assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "peers");
        let expected: &[(&str, &str, &str)] = &[
            ("peers.list", "majordomus_peers", "/api/v1/peers"),
            (
                "peers.announce",
                "majordomus_announce",
                "/api/v1/peers/announce",
            ),
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
}
