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
///
/// Three, and they measure three different things: `Direct` is the handler's own cost
/// through the executor, `Http` adds a real loopback socket and the route's binding, and
/// `Mcp` adds a real child process and the protocol frame. The difference between them is
/// the point — a capability that is fast in process and slow over a transport is a fact
/// about the transport, and one number for both would hide it.
///
/// ```
/// use majordomus_cli::bench::Transport;
/// // the name is the serialised word, and parsing it is the inverse
/// for transport in Transport::ALL {
///     assert_eq!(Transport::parse(transport.name()), Some(transport));
///     assert_eq!(serde_json::to_value(transport).unwrap(), transport.name());
/// }
/// assert_eq!(Transport::parse("carrier-pigeon"), None);
/// ```
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

    /// The name as serialised, which is also the name a target key and a coverage tally
    /// are written with.
    ///
    /// One word per transport, used by the key (`<id>|<transport>|<case>`), by the coverage
    /// denominators and by the document a client reads — so renaming one would rename
    /// every baseline entry with it. That is why the word is here and not formatted at each
    /// call site.
    ///
    /// ```
    /// use majordomus_cli::bench::Transport;
    /// assert_eq!(Transport::Direct.name(), "direct");
    /// // the word on the wire is the same word
    /// assert_eq!(serde_json::to_value(Transport::Mcp).unwrap(), Transport::Mcp.name());
    /// // and no two transports share it
    /// let mut names: Vec<&str> = Transport::ALL.iter().map(|t| t.name()).collect();
    /// names.sort_unstable();
    /// names.dedup();
    /// assert_eq!(names.len(), Transport::ALL.len());
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            Transport::Direct => "direct",
            Transport::Mcp => "mcp",
            Transport::Http => "http",
        }
    }

    /// The transport for a name, or nothing.
    ///
    /// Where a `--transport` argument and a policy file's text become a value. Exact and
    /// case-sensitive, and `None` rather than a default for anything else: silently
    /// measuring the direct transport because somebody wrote `HTTP` would produce a run
    /// whose numbers answer a question nobody asked.
    ///
    /// ```
    /// use majordomus_cli::bench::Transport;
    /// assert_eq!(Transport::parse("http"), Some(Transport::Http));
    /// assert_eq!(Transport::parse("HTTP"), None, "exact, so a typo is not a default");
    /// assert_eq!(Transport::parse(""), None);
    /// ```
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
///
/// Two kinds, because the denominator has two halves: a capability's operations are derived
/// from the registry, and a transport's own operations are declared as data. The capability
/// variant carries everything a runner needs to make the call — the case input, the tool
/// name for MCP, the method and path for HTTP — so no runner has to consult the registry
/// again while it is timing something.
///
/// ```
/// use majordomus_cli::bench::{SystemTarget, TargetKind};
/// let kind = TargetKind::System { target: SystemTarget::HttpOpenApi };
/// // the tag is on the wire, so a result document says which kind it measured
/// let wire = serde_json::to_value(&kind).unwrap();
/// assert_eq!(wire["kind"], "system");
/// assert_eq!(wire["target"], "http_open_api");
/// ```
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

/// One thing to time: a key, and everything needed to make the call it names.
///
/// The key is the identity a result, an accepted baseline and a per-target policy
/// allowance all refer to, which makes it a contract rather than a label: renaming a
/// capability, a transport or a case turns the baseline's entry into a reported stale line
/// instead of a comparison. That is deliberate — a silent re-attachment would compare two
/// different things and call the difference a regression.
///
/// ```
/// # use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget,
/// #     TargetKind, Transport};
/// # use majordomus_cli::capability::CachePolicy;
/// # fn target(id: &str, transport: Transport, case: &str) -> BenchmarkTarget {
/// #     BenchmarkTarget {
/// #         key: format!("{id}|{}|{case}", transport.name()),
/// #         kind: TargetKind::Capability { id: id.into(), module: "demo".into(),
/// #             transport, case: case.into(), input: serde_json::json!({}),
/// #             cache: CachePolicy::Disabled, tool: None, route: None },
/// #     }
/// # }
/// # let projection = BenchmarkProjection { targets: vec![
/// #     target("demo.echo", Transport::Direct, "small"),
/// #     target("demo.echo", Transport::Http, "small"),
/// #     target("demo.scan", Transport::Direct, "small"),
/// #     BenchmarkTarget { key: SystemTarget::McpPing.key().into(),
/// #         kind: TargetKind::System { target: SystemTarget::McpPing } },
/// # ] };
/// let http = projection
///     .by_transport(Transport::Http)
///     .next()
///     .expect("this projection has an HTTP target");
/// // the key is the three things that identify the call, in one string
/// assert_eq!(http.key, "demo.echo|http|small");
/// assert_eq!(http.transport(), Transport::Http);
/// assert_eq!(http.capability_id(), Some("demo.echo"));
/// // and a system target is identified the other way, and owns no capability
/// let system = projection.targets.last().unwrap();
/// assert_eq!(system.capability_id(), None);
/// assert_eq!(system.key, SystemTarget::McpPing.key());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BenchmarkTarget {
    /// The stable key: `<id>|<transport>|<case>` for a capability, `system.<transport>.<name>` otherwise.
    pub key: String,
    /// What it is.
    pub kind: TargetKind,
}

impl BenchmarkTarget {
    /// The transport this target is reached through, whichever kind of target it is.
    ///
    /// Asked of the kind rather than parsed out of the key, so the two cannot disagree —
    /// and it is what lets a run be narrowed to one transport without the runner knowing
    /// how either kind of target is built.
    ///
    /// ```
    /// # use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget,
    /// #     TargetKind, Transport};
    /// # use majordomus_cli::capability::CachePolicy;
    /// # fn target(id: &str, transport: Transport, case: &str) -> BenchmarkTarget {
    /// #     BenchmarkTarget {
    /// #         key: format!("{id}|{}|{case}", transport.name()),
    /// #         kind: TargetKind::Capability { id: id.into(), module: "demo".into(),
    /// #             transport, case: case.into(), input: serde_json::json!({}),
    /// #             cache: CachePolicy::Disabled, tool: None, route: None },
    /// #     }
    /// # }
    /// # let projection = BenchmarkProjection { targets: vec![
    /// #     target("demo.echo", Transport::Direct, "small"),
    /// #     target("demo.echo", Transport::Http, "small"),
    /// #     target("demo.scan", Transport::Direct, "small"),
    /// #     BenchmarkTarget { key: SystemTarget::McpPing.key().into(),
    /// #         kind: TargetKind::System { target: SystemTarget::McpPing } },
    /// # ] };
    /// let direct: Vec<&str> = projection
    ///     .by_transport(Transport::Direct)
    ///     .map(|t| t.key.as_str())
    ///     .collect();
    /// assert_eq!(direct, ["demo.echo|direct|small", "demo.scan|direct|small"]);
    /// // a system target answers with the transport it belongs to, not with none
    /// assert_eq!(projection.targets.last().unwrap().transport(), Transport::Mcp);
    /// ```
    pub fn transport(&self) -> Transport {
        match &self.kind {
            TargetKind::Capability { transport, .. } => *transport,
            TargetKind::System { target } => target.transport(),
        }
    }

    /// The capability id, for a capability target.
    ///
    /// `None` for a system target, and that is a fact rather than a gap: an MCP
    /// `initialize` is not any capability's cost, and attributing it to one would put it in
    /// that capability's coverage line.
    ///
    /// ```
    /// # use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget,
    /// #     TargetKind, Transport};
    /// # use majordomus_cli::capability::CachePolicy;
    /// # fn target(id: &str, transport: Transport, case: &str) -> BenchmarkTarget {
    /// #     BenchmarkTarget {
    /// #         key: format!("{id}|{}|{case}", transport.name()),
    /// #         kind: TargetKind::Capability { id: id.into(), module: "demo".into(),
    /// #             transport, case: case.into(), input: serde_json::json!({}),
    /// #             cache: CachePolicy::Disabled, tool: None, route: None },
    /// #     }
    /// # }
    /// # let projection = BenchmarkProjection { targets: vec![
    /// #     target("demo.echo", Transport::Direct, "small"),
    /// #     target("demo.echo", Transport::Http, "small"),
    /// #     target("demo.scan", Transport::Direct, "small"),
    /// #     BenchmarkTarget { key: SystemTarget::McpPing.key().into(),
    /// #         kind: TargetKind::System { target: SystemTarget::McpPing } },
    /// # ] };
    /// assert_eq!(projection.targets[0].capability_id(), Some("demo.echo"));
    /// assert_eq!(projection.targets.last().unwrap().capability_id(), None);
    /// ```
    pub fn capability_id(&self) -> Option<&str> {
        match &self.kind {
            TargetKind::Capability { id, .. } => Some(id),
            TargetKind::System { .. } => None,
        }
    }
}

/// Every target of a registry against one repository.
///
/// A projection and not a list: the targets are the registry's own executable entries with
/// a required policy, crossed with the transports their exposure declares and the cases
/// their input types provide, plus the system targets. Nothing here names a capability, so
/// a capability added tomorrow is measured tomorrow — and one that declares no case
/// produces no target, which is what coverage then reports as missing.
///
/// It is a projection *of one repository*: the cases come from the index, so the same
/// registry against a different checkout can legitimately produce different targets.
///
/// ```
/// # use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget,
/// #     TargetKind, Transport};
/// # use majordomus_cli::capability::CachePolicy;
/// # fn target(id: &str, transport: Transport, case: &str) -> BenchmarkTarget {
/// #     BenchmarkTarget {
/// #         key: format!("{id}|{}|{case}", transport.name()),
/// #         kind: TargetKind::Capability { id: id.into(), module: "demo".into(),
/// #             transport, case: case.into(), input: serde_json::json!({}),
/// #             cache: CachePolicy::Disabled, tool: None, route: None },
/// #     }
/// # }
/// # let projection = BenchmarkProjection { targets: vec![
/// #     target("demo.echo", Transport::Direct, "small"),
/// #     target("demo.echo", Transport::Http, "small"),
/// #     target("demo.scan", Transport::Direct, "small"),
/// #     BenchmarkTarget { key: SystemTarget::McpPing.key().into(),
/// #         kind: TargetKind::System { target: SystemTarget::McpPing } },
/// # ] };
/// // capability targets first, the transports' own operations last
/// assert_eq!(projection.targets.len(), 4);
/// assert_eq!(projection.of_capability("demo.echo").count(), 2);
/// // exposure decides the crossing: this capability is not exposed over MCP
/// assert!(projection.covers("demo.echo", Transport::Http));
/// assert!(!projection.covers("demo.echo", Transport::Mcp));
/// // and a capability nobody declared has no targets rather than an error
/// assert_eq!(projection.of_capability("demo.absent").count(), 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BenchmarkProjection {
    /// The targets, capabilities first (by id, transport, case), then the system ones.
    pub targets: Vec<BenchmarkTarget>,
}

impl BenchmarkProjection {
    /// Derive the targets: the cases need the index, so the projection is of a context.
    ///
    /// Four filters and no list: an entry must be executable, must be of a stability a
    /// projection executes, must declare a *required* benchmark policy, and must have an
    /// input type that provides cases. Anything that fails one of those produces no target
    /// — and the coverage document is where that absence is reported, so a capability
    /// cannot quietly leave the denominator.
    ///
    /// ```no_run
    /// use majordomus_cli::bench::{BenchmarkProjection, Transport};
    /// use majordomus_cli::capability::Context;
    /// // compiled and not run: deriving needs this process's own registry and index
    /// fn derive(ctx: &Context) {
    ///     let projection = BenchmarkProjection::from_context(ctx);
    ///     // every capability target is reachable through the executor it was derived from
    ///     assert!(projection.by_transport(Transport::Direct).count() > 0);
    ///     // and the transports' own operations are in it whatever the registry holds
    ///     assert!(projection.targets.iter().any(|t| t.capability_id().is_none()));
    /// }
    /// ```
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

    /// The targets of one transport, in the projection's own order.
    ///
    /// What `--transport` narrows a run to, and it is a filter over the derived value
    /// rather than a second derivation: a target that reaches a runner this way is one the
    /// coverage document already counted.
    ///
    /// ```
    /// # use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget,
    /// #     TargetKind, Transport};
    /// # use majordomus_cli::capability::CachePolicy;
    /// # fn target(id: &str, transport: Transport, case: &str) -> BenchmarkTarget {
    /// #     BenchmarkTarget {
    /// #         key: format!("{id}|{}|{case}", transport.name()),
    /// #         kind: TargetKind::Capability { id: id.into(), module: "demo".into(),
    /// #             transport, case: case.into(), input: serde_json::json!({}),
    /// #             cache: CachePolicy::Disabled, tool: None, route: None },
    /// #     }
    /// # }
    /// # let projection = BenchmarkProjection { targets: vec![
    /// #     target("demo.echo", Transport::Direct, "small"),
    /// #     target("demo.echo", Transport::Http, "small"),
    /// #     target("demo.scan", Transport::Direct, "small"),
    /// #     BenchmarkTarget { key: SystemTarget::McpPing.key().into(),
    /// #         kind: TargetKind::System { target: SystemTarget::McpPing } },
    /// # ] };
    /// assert_eq!(projection.by_transport(Transport::Http).count(), 1);
    /// assert_eq!(projection.by_transport(Transport::Direct).count(), 2);
    /// // every target belongs to exactly one transport, so the parts add up to the whole
    /// let counted: usize = Transport::ALL
    ///     .iter()
    ///     .map(|t| projection.by_transport(*t).count())
    ///     .sum();
    /// assert_eq!(counted, projection.targets.len());
    /// ```
    pub fn by_transport(&self, transport: Transport) -> impl Iterator<Item = &BenchmarkTarget> {
        self.targets
            .iter()
            .filter(move |t| t.transport() == transport)
    }

    /// The targets of one capability: every transport and every case it produced.
    ///
    /// By canonical id and nothing looser, because this is what coverage counts with: a
    /// prefix or a fuzzy match here would attribute one capability's cases to another and
    /// make a missing requirement look covered.
    ///
    /// ```
    /// # use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget,
    /// #     TargetKind, Transport};
    /// # use majordomus_cli::capability::CachePolicy;
    /// # fn target(id: &str, transport: Transport, case: &str) -> BenchmarkTarget {
    /// #     BenchmarkTarget {
    /// #         key: format!("{id}|{}|{case}", transport.name()),
    /// #         kind: TargetKind::Capability { id: id.into(), module: "demo".into(),
    /// #             transport, case: case.into(), input: serde_json::json!({}),
    /// #             cache: CachePolicy::Disabled, tool: None, route: None },
    /// #     }
    /// # }
    /// # let projection = BenchmarkProjection { targets: vec![
    /// #     target("demo.echo", Transport::Direct, "small"),
    /// #     target("demo.echo", Transport::Http, "small"),
    /// #     target("demo.scan", Transport::Direct, "small"),
    /// #     BenchmarkTarget { key: SystemTarget::McpPing.key().into(),
    /// #         kind: TargetKind::System { target: SystemTarget::McpPing } },
    /// # ] };
    /// let keys: Vec<&str> = projection
    ///     .of_capability("demo.echo")
    ///     .map(|t| t.key.as_str())
    ///     .collect();
    /// assert_eq!(keys, ["demo.echo|direct|small", "demo.echo|http|small"]);
    /// // exact: a capability whose id merely starts the same way is a different capability
    /// assert_eq!(projection.of_capability("demo").count(), 0);
    /// ```
    pub fn of_capability<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a BenchmarkTarget> {
        self.targets
            .iter()
            .filter(move |t| t.capability_id() == Some(id))
    }

    /// Is a capability exposed on a transport a target there? Coverage asks this.
    ///
    /// This is the numerator of the coverage document, and it is derived from the same
    /// projection a run measures — so "covered" means a target genuinely exists rather than
    /// that somebody recorded it as covered. It says nothing about whether that target was
    /// *measured*: a filtered run leaves coverage exactly where it was.
    ///
    /// ```
    /// # use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget,
    /// #     TargetKind, Transport};
    /// # use majordomus_cli::capability::CachePolicy;
    /// # fn target(id: &str, transport: Transport, case: &str) -> BenchmarkTarget {
    /// #     BenchmarkTarget {
    /// #         key: format!("{id}|{}|{case}", transport.name()),
    /// #         kind: TargetKind::Capability { id: id.into(), module: "demo".into(),
    /// #             transport, case: case.into(), input: serde_json::json!({}),
    /// #             cache: CachePolicy::Disabled, tool: None, route: None },
    /// #     }
    /// # }
    /// # let projection = BenchmarkProjection { targets: vec![
    /// #     target("demo.echo", Transport::Direct, "small"),
    /// #     target("demo.echo", Transport::Http, "small"),
    /// #     target("demo.scan", Transport::Direct, "small"),
    /// #     BenchmarkTarget { key: SystemTarget::McpPing.key().into(),
    /// #         kind: TargetKind::System { target: SystemTarget::McpPing } },
    /// # ] };
    /// assert!(projection.covers("demo.echo", Transport::Direct));
    /// // declared nowhere on this transport: what a coverage line reports as missing
    /// assert!(!projection.covers("demo.scan", Transport::Http));
    /// assert!(!projection.covers("demo.absent", Transport::Direct));
    /// ```
    pub fn covers(&self, id: &str, transport: Transport) -> bool {
        self.of_capability(id).any(|t| t.transport() == transport)
    }

    /// Does the kind of a target make sense for a command? Commands are targets like
    /// queries; their side effect is this process's memory.
    ///
    /// Asked of the registry rather than of a target, because a target carries no kind: a
    /// runner that needed to know whether repeating a call is safe asks here. `false` for
    /// an id the registry does not hold, which is the same answer as "not a command" on
    /// purpose — nothing may be run on the strength of an id nobody declared.
    ///
    /// ```no_run
    /// use majordomus_cli::bench::BenchmarkProjection;
    /// use majordomus_cli::capability::CapabilityRegistry;
    /// // compiled and not run: it reads this process's own registry
    /// fn check(registry: &CapabilityRegistry) {
    ///     assert!(!BenchmarkProjection::is_command(registry, "no.such.capability"));
    /// }
    /// ```
    pub fn is_command(registry: &CapabilityRegistry, id: &str) -> bool {
        registry
            .get(id)
            .is_some_and(|c| c.kind == CapabilityKind::Command)
    }
}
