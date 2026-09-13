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
/// The same handler costs three different things depending on how it is called, and the
/// difference between them is the only thing a transport measurement can tell you:
/// `Direct` is the handler's own work, `Http` adds a socket and the router's parsing, and
/// `Mcp` adds a framed protocol and a child process. Every target names one, so a result
/// is never an average over the three.
///
/// ```
/// use majordomus_cli::bench::Transport;
///
/// // The three are exactly what a name can round-trip through, and nothing else parses.
/// for t in Transport::ALL {
///     assert_eq!(Transport::parse(t.name()), Some(t));
/// }
/// assert_eq!(Transport::parse("grpc"), None);
///
/// // `Direct` sorts first, so a report that groups by transport opens with the floor
/// // the other two are read against.
/// assert_eq!(Transport::ALL.iter().min(), Some(&Transport::Direct));
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

    /// The name as serialised: the word that appears in a target key, in a coverage
    /// tally's bucket and in `--transport`.
    ///
    /// It is one word and lower case because it is a key, not a label: `<id>|<name>|<case>`
    /// is split on `|` by readers of a results file, and a name carrying a space or a
    /// capital would have to be quoted somewhere.
    ///
    /// ```
    /// use majordomus_cli::bench::Transport;
    /// assert_eq!(Transport::Mcp.name(), "mcp");
    /// assert!(Transport::ALL.iter().all(|t| t.name().chars().all(|c| c.is_ascii_lowercase())));
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            Transport::Direct => "direct",
            Transport::Mcp => "mcp",
            Transport::Http => "http",
        }
    }

    /// The transport for a name, or `None` for anything else.
    ///
    /// A name that does not parse is refused rather than defaulted, because the caller is
    /// a `--transport` filter: silently reading an unknown word as `direct` would time a
    /// third of what the operator asked for and report it as the whole run.
    ///
    /// ```
    /// use majordomus_cli::bench::Transport;
    /// assert_eq!(Transport::parse("http"), Some(Transport::Http));
    /// assert_eq!(Transport::parse("HTTP"), None, "a key, not a label: the case is part of it");
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
/// The variant carries the transport's own address rather than looking it up later:
/// `tool` is `Some` only on MCP and `route` only on HTTP, so a target holds the one
/// addressing its runner needs and holds nothing it must not use. A target for the direct
/// transport has neither, which is what "the handler's own cost" means.
///
/// ```
/// use majordomus_cli::bench::{TargetKind, Transport};
/// use majordomus_cli::capability::CachePolicy;
/// use serde_json::json;
///
/// let over_http = TargetKind::Capability {
///     id: "context.resolve".into(),
///     module: "context".into(),
///     transport: Transport::Http,
///     case: "root".into(),
///     input: json!({ "path": "." }),
///     cache: CachePolicy::Disabled,
///     tool: None,
///     route: Some(("GET".into(), "/context/resolve".into())),
/// };
/// let TargetKind::Capability { route, tool, .. } = &over_http else {
///     panic!("a capability target")
/// };
/// assert_eq!(route.as_ref().map(|(m, _)| m.as_str()), Some("GET"));
/// assert!(tool.is_none(), "an HTTP target carries no MCP tool name");
///
/// // A system target carries nothing but which one it is.
/// let system = TargetKind::System { target: majordomus_cli::bench::SystemTarget::McpPing };
/// assert!(matches!(system, TargetKind::System { .. }));
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

/// One thing to time: a key and what is behind it.
///
/// The key is the identity a result, a baseline line and a check share, so it is built
/// from the three things that make one measurement different from another — the
/// capability, the transport and the case — and from nothing else. Two targets of the same
/// capability on different transports are two keys, and a baseline that loses one reports
/// it as `STALE` rather than comparing it with the other.
///
/// ```
/// use majordomus_cli::bench::{BenchmarkTarget, SystemTarget, TargetKind, Transport};
/// use majordomus_cli::capability::CachePolicy;
/// use serde_json::json;
///
/// let direct = BenchmarkTarget {
///     key: "context.resolve|direct|root".into(),
///     kind: TargetKind::Capability {
///         id: "context.resolve".into(),
///         module: "context".into(),
///         transport: Transport::Direct,
///         case: "root".into(),
///         input: json!({ "path": "." }),
///         cache: CachePolicy::Disabled,
///         tool: None,
///         route: None,
///     },
/// };
/// assert_eq!(
///     direct.key,
///     format!("{}|{}|{}", "context.resolve", Transport::Direct.name(), "root")
/// );
///
/// let ping = BenchmarkTarget {
///     key: SystemTarget::McpPing.key().into(),
///     kind: TargetKind::System { target: SystemTarget::McpPing },
/// };
/// assert_ne!(ping.key, direct.key);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BenchmarkTarget {
    /// The stable key: `<id>|<transport>|<case>` for a capability, `system.<transport>.<name>` otherwise.
    pub key: String,
    /// What it is.
    pub kind: TargetKind,
}

impl BenchmarkTarget {
    /// The transport this target is reached over, whichever kind it is.
    ///
    /// A capability target names its transport; a system target *is* one of a transport's
    /// operations and answers from the variant. The two are asked the same question so that
    /// a `--transport` filter and a per-transport tally never have to know which kind they
    /// are holding.
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkTarget, SystemTarget, TargetKind, Transport};
    ///
    /// let ping = BenchmarkTarget {
    ///     key: SystemTarget::McpPing.key().into(),
    ///     kind: TargetKind::System { target: SystemTarget::McpPing },
    /// };
    /// assert_eq!(ping.transport(), Transport::Mcp);
    /// assert_eq!(ping.transport(), SystemTarget::McpPing.transport());
    /// ```
    pub fn transport(&self) -> Transport {
        match &self.kind {
            TargetKind::Capability { transport, .. } => *transport,
            TargetKind::System { target } => target.transport(),
        }
    }

    /// The capability id, for a capability target; `None` for a system one.
    ///
    /// The `None` is what keeps the denominators honest. Coverage counts a capability's
    /// lines per transport and the transports' own operations under a separate bucket; a
    /// system target that answered with an id would be counted twice, once as itself and
    /// once against whatever capability it claimed.
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkTarget, SystemTarget, TargetKind};
    ///
    /// let ping = BenchmarkTarget {
    ///     key: SystemTarget::McpPing.key().into(),
    ///     kind: TargetKind::System { target: SystemTarget::McpPing },
    /// };
    /// assert_eq!(ping.capability_id(), None, "a protocol method belongs to no capability");
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
/// It is a projection and not a list: nothing here names a capability, and adding one with
/// a required benchmark policy adds its targets on every transport it is exposed over, in
/// the same commit, with no inventory to update. The vector is the whole document —
/// capabilities first, by id, transport and case, then the system targets — so two runs
/// over the same registry produce the same order and a results file diffs.
///
/// ```
/// use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget, TargetKind, Transport};
/// # use majordomus_cli::capability::CachePolicy;
/// # fn cap(id: &str, transport: Transport, case: &str) -> BenchmarkTarget {
/// #     BenchmarkTarget {
/// #         key: format!("{id}|{}|{case}", transport.name()),
/// #         kind: TargetKind::Capability {
/// #             id: id.into(), module: "context".into(), transport, case: case.into(),
/// #             input: serde_json::json!({}), cache: CachePolicy::Disabled,
/// #             tool: None, route: None,
/// #         },
/// #     }
/// # }
/// let projection = BenchmarkProjection {
///     targets: vec![
///         cap("context.resolve", Transport::Direct, "root"),
///         cap("context.resolve", Transport::Http, "root"),
///         BenchmarkTarget {
///             key: SystemTarget::HttpOpenApi.key().into(),
///             kind: TargetKind::System { target: SystemTarget::HttpOpenApi },
///         },
///     ],
/// };
///
/// // One capability exposed twice is two targets, not one averaged over transports.
/// assert_eq!(projection.of_capability("context.resolve").count(), 2);
/// // The system target is in the same vector and belongs to no capability.
/// assert_eq!(projection.targets.iter().filter(|t| t.capability_id().is_none()).count(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BenchmarkProjection {
    /// The targets, capabilities first (by id, transport, case), then the system ones.
    pub targets: Vec<BenchmarkTarget>,
}

impl BenchmarkProjection {
    /// Derive the targets: the cases need the index, so the projection is of a context.
    ///
    /// Four conditions have to hold before a capability contributes anything, and each one
    /// silently removes targets rather than failing: the entry must be executable and
    /// stable, its benchmark policy must be `Required`, its input type must provide cases
    /// for *this* repository, and the transport must be one its exposure declares. The
    /// fourth is why the count is not `capabilities × 3`: a capability with no MCP tool
    /// name is not a target on MCP, and a capability with no HTTP route is not a target on
    /// HTTP. The third is why coverage exists at all — a capability whose case provider
    /// finds nothing to work on in this repository produces no target, and
    /// [`Coverage`](crate::bench::Coverage) reports it as `Missing` instead of letting it
    /// vanish.
    ///
    /// ```no_run
    /// use majordomus_cli::app::App;
    /// use majordomus_cli::bench::{BenchmarkProjection, SystemTarget};
    /// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
    ///
    /// let app = App::load(&RepoArgs {
    ///     repo: Some("<repository>".into()),
    ///     discovery: DiscoveryMode::Vcs,
    ///     strict: false,
    ///     share: None,
    /// })
    /// .unwrap();
    /// let projection = BenchmarkProjection::from_context(&app.context);
    ///
    /// // The system targets are unconditional: they are declared, not discovered.
    /// for s in SystemTarget::ALL {
    ///     assert!(projection.targets.iter().any(|t| t.key == s.key()));
    /// }
    /// // Every capability target names a case, so the key is unique per case.
    /// let mut keys: Vec<&str> = projection.targets.iter().map(|t| t.key.as_str()).collect();
    /// keys.sort_unstable();
    /// let total = keys.len();
    /// keys.dedup();
    /// assert_eq!(keys.len(), total);
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

    /// The targets of one transport, capability and system alike.
    ///
    /// What `--transport http` runs, and what the per-transport tallies count. The system
    /// targets are included deliberately: a run restricted to HTTP still pays for the
    /// router and still measures it.
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, SystemTarget, TargetKind, Transport};
    /// # use majordomus_cli::capability::CachePolicy;
    /// # fn cap(id: &str, transport: Transport) -> BenchmarkTarget {
    /// #     BenchmarkTarget {
    /// #         key: format!("{id}|{}|root", transport.name()),
    /// #         kind: TargetKind::Capability {
    /// #             id: id.into(), module: "context".into(), transport, case: "root".into(),
    /// #             input: serde_json::json!({}), cache: CachePolicy::Disabled,
    /// #             tool: None, route: None,
    /// #         },
    /// #     }
    /// # }
    /// let projection = BenchmarkProjection {
    ///     targets: vec![
    ///         cap("context.resolve", Transport::Direct),
    ///         cap("context.resolve", Transport::Http),
    ///         BenchmarkTarget {
    ///             key: SystemTarget::HttpOpenApi.key().into(),
    ///             kind: TargetKind::System { target: SystemTarget::HttpOpenApi },
    ///         },
    ///     ],
    /// };
    /// // One capability target and one system target answer to `http`.
    /// assert_eq!(projection.by_transport(Transport::Http).count(), 2);
    /// assert_eq!(projection.by_transport(Transport::Direct).count(), 1);
    /// assert_eq!(projection.by_transport(Transport::Mcp).count(), 0);
    /// ```
    pub fn by_transport(&self, transport: Transport) -> impl Iterator<Item = &BenchmarkTarget> {
        self.targets
            .iter()
            .filter(move |t| t.transport() == transport)
    }

    /// The targets of one capability, across every transport and case it produced.
    ///
    /// Coverage counts these per transport, so an empty iterator for an id the registry
    /// knows is not an absence of data — it is the finding that the capability owes a
    /// benchmark and this repository produced no case for it.
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, TargetKind, Transport};
    /// # use majordomus_cli::capability::CachePolicy;
    /// # fn cap(id: &str, case: &str) -> BenchmarkTarget {
    /// #     BenchmarkTarget {
    /// #         key: format!("{id}|direct|{case}"),
    /// #         kind: TargetKind::Capability {
    /// #             id: id.into(), module: "context".into(), transport: Transport::Direct,
    /// #             case: case.into(), input: serde_json::json!({}), cache: CachePolicy::Disabled,
    /// #             tool: None, route: None,
    /// #         },
    /// #     }
    /// # }
    /// let projection = BenchmarkProjection {
    ///     targets: vec![cap("context.resolve", "root"), cap("context.resolve", "nested"), cap("plan.show", "one")],
    /// };
    /// assert_eq!(projection.of_capability("context.resolve").count(), 2, "two cases, two targets");
    /// assert_eq!(projection.of_capability("never.declared").count(), 0);
    /// ```
    pub fn of_capability<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a BenchmarkTarget> {
        self.targets
            .iter()
            .filter(move |t| t.capability_id() == Some(id))
    }

    /// Is a capability exposed on a transport a target there? Coverage asks this.
    ///
    /// The interesting answer is the false one. A capability that is executed and exposed
    /// over HTTP but declares no MCP tool is reachable, is benchmarked, and is still not a
    /// target on MCP — it is served and never published there, so there is nothing to time
    /// and nothing is claimed. Coverage turns exactly this into the `Missing` or the
    /// absent line, per transport, rather than reporting one number for the capability.
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkProjection, BenchmarkTarget, TargetKind, Transport};
    /// use majordomus_cli::capability::CachePolicy;
    ///
    /// // `context.resolve` as the projection derives it for a capability with an HTTP
    /// // route and no MCP tool: two targets, and none of them on MCP.
    /// let target = |transport: Transport, route: Option<(String, String)>| BenchmarkTarget {
    ///     key: format!("context.resolve|{}|root", transport.name()),
    ///     kind: TargetKind::Capability {
    ///         id: "context.resolve".into(),
    ///         module: "context".into(),
    ///         transport,
    ///         case: "root".into(),
    ///         input: serde_json::json!({}),
    ///         cache: CachePolicy::Disabled,
    ///         tool: None,
    ///         route,
    ///     },
    /// };
    /// let projection = BenchmarkProjection {
    ///     targets: vec![
    ///         target(Transport::Direct, None),
    ///         target(Transport::Http, Some(("GET".into(), "/context/resolve".into()))),
    ///     ],
    /// };
    ///
    /// assert!(projection.covers("context.resolve", Transport::Direct));
    /// assert!(projection.covers("context.resolve", Transport::Http));
    /// assert!(
    ///     !projection.covers("context.resolve", Transport::Mcp),
    ///     "no tool name, so no MCP target: the capability is served, never published there"
    /// );
    /// ```
    pub fn covers(&self, id: &str, transport: Transport) -> bool {
        self.of_capability(id).any(|t| t.transport() == transport)
    }

    /// Does the kind of a target make sense for a command? Commands are targets like
    /// queries; their side effect is this process's memory.
    ///
    /// A benchmark repeats a call hundreds of times, so this is the question of whether
    /// doing that is safe. It is, for this registry's commands: they change what the
    /// process is holding — a peer's announcement, an execution's state — and nothing in
    /// the repository, which is why they are timed rather than excluded. The runner still
    /// asks, because the answer decides whether a warm cache mode means anything.
    ///
    /// ```
    /// use majordomus_cli::bench::BenchmarkProjection;
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry};
    ///
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// assert!(BenchmarkProjection::is_command(&registry, "peers.announce"));
    /// assert!(!BenchmarkProjection::is_command(&registry, "repository.info"), "a query");
    /// assert!(!BenchmarkProjection::is_command(&registry, "never.declared"), "absent is not a command");
    /// ```
    pub fn is_command(registry: &CapabilityRegistry, id: &str) -> bool {
        registry
            .get(id)
            .is_some_and(|c| c.kind == CapabilityKind::Command)
    }
}
