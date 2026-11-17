//! Targets from the registry. Nothing here names a capability: every target is an entry
//! with a required benchmark policy, crossed with the transports its exposure declares
//! and the cases its input type provides, plus the system targets, plus every page of
//! the UI: each screen and panel the UI's own route list declares (`ui::SCREENS`,
//! `ui::PANELS`, the lists its dispatcher is held to), over the pinned context a one-shot
//! process holds and, for the screens, over the live view a served process follows the
//! repository with.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::{CachePolicy, CapabilityKind, CapabilityRegistry, CaseContext, Context};

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

/// Which reading of the repository a page request is answered from.
///
/// ```
/// use majordomus_cli::bench::View;
/// assert_ne!(View::Pinned, View::Live, "a served process pays what a pinned one does not");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum View {
    /// One context, never re-read: what a benchmark or a one-shot process holds.
    Pinned,
    /// The live view `serve` answers from: every request stamps the watched git control
    /// files, and a moved stamp reloads the layer before the answer.
    Live,
}

impl View {
    /// The view's name as a result document serialises it and as the generated target
    /// list prints it: `pinned` or `live`.
    ///
    /// ```
    /// use majordomus_cli::bench::View;
    /// assert_eq!(View::Pinned.name(), "pinned");
    /// assert_eq!(View::Live.name(), "live");
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            View::Pinned => "pinned",
            View::Live => "live",
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
    /// One page of the UI, read with `GET` and `Accept: text/html` over a real socket.
    Page {
        /// The route, from the UI's own route list.
        route: String,
        /// The reading of the repository it is answered from.
        view: View,
        /// A panel: a route the UI defers, because the source it waits on is too slow for
        /// a screen to wait for. It is measured only by a profile that admits deferred
        /// panels (`full`), with fewer samples, so that it cannot dominate a quick run.
        deferred: bool,
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
            TargetKind::Page { .. } => Transport::Http,
        }
    }

    /// The capability id, for a capability target.
    pub fn capability_id(&self) -> Option<&str> {
        match &self.kind {
            TargetKind::Capability { id, .. } => Some(id),
            TargetKind::System { .. } | TargetKind::Page { .. } => None,
        }
    }

    /// Is this a panel the UI defers?
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkTarget, TargetKind, View};
    /// let page = |deferred| BenchmarkTarget {
    ///     key: "system.http.ui.panels.drain".into(),
    ///     kind: TargetKind::Page {
    ///         route: "/ui/panels/drain".into(),
    ///         view: View::Pinned,
    ///         deferred,
    ///     },
    /// };
    /// assert!(page(true).is_deferred());
    /// assert!(!page(false).is_deferred());
    /// ```
    pub fn is_deferred(&self) -> bool {
        matches!(self.kind, TargetKind::Page { deferred: true, .. })
    }
}

/// Every page of the UI as a target: each screen over the pinned context and over the live
/// view, each panel over the pinned context only (what following git costs is the same
/// stamp on every route, and the screens measure it), deferred.
fn ui_pages() -> Vec<BenchmarkTarget> {
    let page = |route: &str, view: View, deferred: bool| BenchmarkTarget {
        key: page_key(route, view),
        kind: TargetKind::Page {
            route: route.to_string(),
            view,
            deferred,
        },
    };
    let mut pages = Vec::new();
    for view in [View::Pinned, View::Live] {
        pages.extend(
            crate::ui::SCREENS
                .iter()
                .map(|screen| page(screen.route, view, false)),
        );
    }
    pages.extend(
        crate::ui::PANELS
            .iter()
            .map(|route| page(route, View::Pinned, true)),
    );
    pages
}

/// `system.http.ui.<route>` over the pinned context, `system.http.live.ui.<route>` over
/// the live view; the route's segments below the prefix joined by dots, `index` for the
/// prefix itself.
///
/// ```text
/// /ui                        -> system.http.ui.index
/// /ui/panels/pull-requests   -> system.http.ui.panels.pull_requests
/// ```
fn page_key(route: &str, view: View) -> String {
    let below = route
        .strip_prefix(crate::ui::PREFIX)
        .unwrap_or(route)
        .trim_matches('/');
    let name = if below.is_empty() {
        "index".to_string()
    } else {
        below.replace('/', ".").replace('-', "_")
    };
    match view {
        View::Pinned => format!("system.http.ui.{name}"),
        View::Live => format!("system.http.live.ui.{name}"),
    }
}

/// Every target of a registry against one repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BenchmarkProjection {
    /// The targets, capabilities first (by id, transport, case), then the system ones,
    /// then the UI's pages.
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
            if !c.benchmark.is_target() {
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
        targets.extend(ui_pages());
        BenchmarkProjection { targets }
    }

    /// The UI's pages, in the order the targets list them.
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkProjection, TargetKind};
    /// # fn example(ctx: &majordomus_cli::capability::Context) {
    /// let projection = BenchmarkProjection::from_context(ctx);
    /// assert!(projection.pages().all(|t| matches!(t.kind, TargetKind::Page { .. })));
    /// assert!(projection.pages().any(|t| t.key == "system.http.live.ui.index"));
    /// # }
    /// ```
    pub fn pages(&self) -> impl Iterator<Item = &BenchmarkTarget> {
        self.targets
            .iter()
            .filter(|t| matches!(t.kind, TargetKind::Page { .. }))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The pages are the UI's own route list, read and never repeated: every screen over
    /// both views, every panel deferred over the pinned context, and one key per page.
    #[test]
    fn every_ui_screen_and_panel_is_a_page_target_and_only_they_are() {
        let pages = ui_pages();
        for screen in crate::ui::SCREENS {
            for view in [View::Pinned, View::Live] {
                assert!(
                    pages.iter().any(|t| t.kind
                        == TargetKind::Page {
                            route: screen.route.to_string(),
                            view,
                            deferred: false,
                        }),
                    "{} over {}",
                    screen.route,
                    view.name()
                );
            }
        }
        for panel in crate::ui::PANELS {
            let of: Vec<_> = pages
                .iter()
                .filter(|t| matches!(&t.kind, TargetKind::Page { route, .. } if route == panel))
                .collect();
            assert_eq!(of.len(), 1, "{panel}: once, pinned");
            assert!(of[0].is_deferred(), "{panel} is deferred");
        }
        assert_eq!(
            pages.len(),
            2 * crate::ui::SCREENS.len() + crate::ui::PANELS.len()
        );
        let mut keys: Vec<&str> = pages.iter().map(|t| t.key.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), pages.len(), "a key per page");
        assert!(pages.iter().all(|t| t.transport() == Transport::Http));
        assert!(pages.iter().all(|t| t.capability_id().is_none()));
    }

    #[test]
    fn a_page_key_names_its_view_and_its_route() {
        assert_eq!(page_key("/ui", View::Pinned), "system.http.ui.index");
        assert_eq!(page_key("/ui/", View::Live), "system.http.live.ui.index");
        assert_eq!(
            page_key("/ui/panels/pull-requests", View::Pinned),
            "system.http.ui.panels.pull_requests"
        );
        assert_eq!(
            page_key("/ui/roadmap", View::Live),
            "system.http.live.ui.roadmap"
        );
    }
}
