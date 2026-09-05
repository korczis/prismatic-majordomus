//! Targets from the registry. Nothing here names a capability: every target is an entry
//! with a required benchmark policy, crossed with the transports its exposure declares
//! and the cases its input type provides, plus the system targets.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::{
    BenchmarkPolicy, CachePolicy, CapabilityKind, CapabilityRegistry, CaseContext, Context,
};

use super::system::SystemTarget;

/// The way a target is reached.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// The executor, in process: the handler's own cost.
    Direct,
    /// A real `majordomus mcp` child process over stdio.
    Mcp,
    /// A real loopback socket.
    Http,
}

impl Transport {
    /// Every transport, in order.
    pub const ALL: [Transport; 3] = [Transport::Direct, Transport::Mcp, Transport::Http];

    /// The name as serialised.
    pub fn name(self) -> &'static str {
        match self {
            Transport::Direct => "direct",
            Transport::Mcp => "mcp",
            Transport::Http => "http",
        }
    }

    /// The transport for a name.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "direct" => Some(Transport::Direct),
            "mcp" => Some(Transport::Mcp),
            "http" => Some(Transport::Http),
            _ => None,
        }
    }
}

/// What a target is. A data enum: the capability variant carries the case input and is
/// the common one, so it is not boxed for the sake of the small system variant.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetKind {
    /// One capability, one transport, one case.
    Capability {
        /// The canonical id.
        id: String,
        /// The module.
        module: String,
        /// The transport.
        transport: Transport,
        /// The case name, from the input type.
        case: String,
        /// The case input, as every runner serialises it.
        input: Value,
        /// The cache policy the executor applies; cold and warm are measured when it is enabled.
        cache: CachePolicy,
        /// The MCP tool name, when the transport is MCP.
        #[serde(skip_serializing_if = "Option::is_none")]
        tool: Option<String>,
        /// The HTTP method and path, when the transport is HTTP.
        #[serde(skip_serializing_if = "Option::is_none")]
        route: Option<(String, String)>,
    },
    /// A transport's own operation.
    System {
        /// Which one.
        target: SystemTarget,
    },
}

/// One thing to time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BenchmarkTarget {
    /// The stable key: `<id>|<transport>|<case>` for a capability, `system.<transport>.<name>` otherwise.
    pub key: String,
    /// What it is.
    pub kind: TargetKind,
}

impl BenchmarkTarget {
    /// The transport.
    pub fn transport(&self) -> Transport {
        match &self.kind {
            TargetKind::Capability { transport, .. } => *transport,
            TargetKind::System { target } => target.transport(),
        }
    }

    /// The capability id, for a capability target.
    pub fn capability_id(&self) -> Option<&str> {
        match &self.kind {
            TargetKind::Capability { id, .. } => Some(id),
            TargetKind::System { .. } => None,
        }
    }
}

/// Every target of a registry against one repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BenchmarkProjection {
    /// The targets, capabilities first (by id, transport, case), then the system ones.
    pub targets: Vec<BenchmarkTarget>,
}

impl BenchmarkProjection {
    /// Derive the targets: the cases need the index, so the projection is of a context.
    pub fn from_context(ctx: &Context) -> Self {
        let registry: &CapabilityRegistry = &ctx.registry;
        let case_ctx = CaseContext { index: &ctx.index };
        let mut targets = Vec::new();
        for c in registry.iter() {
            if !c.kind.is_executable() || !c.stability.executable() {
                continue;
            }
            if c.benchmark != BenchmarkPolicy::Required {
                continue;
            }
            let Some(provider) = registry.cases(c.id.as_str()) else {
                continue;
            };
            let cases = provider(&case_ctx);
            for transport in Transport::ALL {
                let tool = c.exposure.mcp.as_ref().and_then(|m| m.tool.clone());
                let route = c
                    .exposure
                    .http
                    .as_ref()
                    .map(|h| (h.method.as_str().to_string(), h.path.clone()));
                let exposed = match transport {
                    Transport::Direct => true,
                    Transport::Mcp => tool.is_some(),
                    Transport::Http => route.is_some(),
                };
                if !exposed {
                    continue;
                }
                for case in &cases {
                    targets.push(BenchmarkTarget {
                        key: format!("{}|{}|{}", c.id, transport.name(), case.name),
                        kind: TargetKind::Capability {
                            id: c.id.to_string(),
                            module: c.module.as_str().to_string(),
                            transport,
                            case: case.name.to_string(),
                            input: case.input.clone(),
                            cache: c.cache,
                            tool: (transport == Transport::Mcp)
                                .then(|| tool.clone())
                                .flatten(),
                            route: (transport == Transport::Http)
                                .then(|| route.clone())
                                .flatten(),
                        },
                    });
                }
            }
        }
        for s in SystemTarget::ALL {
            targets.push(BenchmarkTarget {
                key: s.key().to_string(),
                kind: TargetKind::System { target: s },
            });
        }
        BenchmarkProjection { targets }
    }

    /// The targets of one transport.
    pub fn by_transport(&self, transport: Transport) -> impl Iterator<Item = &BenchmarkTarget> {
        self.targets
            .iter()
            .filter(move |t| t.transport() == transport)
    }

    /// The targets of one capability.
    pub fn of_capability<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a BenchmarkTarget> {
        self.targets
            .iter()
            .filter(move |t| t.capability_id() == Some(id))
    }

    /// Is a capability exposed on a transport a target there? Coverage asks this.
    pub fn covers(&self, id: &str, transport: Transport) -> bool {
        self.of_capability(id).any(|t| t.transport() == transport)
    }

    /// Does the kind of a target make sense for a command? Commands are targets like
    /// queries; their side effect is this process's memory.
    pub fn is_command(registry: &CapabilityRegistry, id: &str) -> bool {
        registry
            .get(id)
            .is_some_and(|c| c.kind == CapabilityKind::Command)
    }
}
