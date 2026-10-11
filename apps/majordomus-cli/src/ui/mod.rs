//! `majordomus ui`: the primary surface, a product of its own, server-rendered (ADR 0135).
//!
//! Two surfaces of this process are for a person. The UI (`/ui`) is the product: the
//! delivery in the project's own words — how the work stands, what is planned and blocked,
//! what is in the queue, which release is live, who is on the mesh, what holds as evidence
//! and which rules are proven. The Cockpit (`/cockpit`) is the technical layer beneath it,
//! the registry rendered, and the UI refers to it only as the low-level reference of a value
//! ("technical view" on a source line).
//!
//! It is a projection, never an application. Every screen composes capabilities through the
//! executor every transport calls, and every value carries the capability and pointer it
//! was read from ([`source`]): inferred, derived and projected, never kept. It holds no data,
//! no status vocabulary and no route list beyond [`SCREENS`] and [`PANELS`], which a test
//! holds against the dispatcher. It shares mechanics rather than copying them:
//! - the element builder and the asset server, pointed at `share/ui/`;
//! - the content-security policy, with its one hash-pinned inline script;
//! - the design tokens, the kit and the icon set every surface draws from.
//!
//! ```text
//!   capability! ── registry ── executor ──┬── MCP, HTTP, command line
//!                                         ├── UI       /ui       (the product)
//!                                         └── Cockpit  /cockpit  (its technical reference)
//! ```
//!
//! With no JavaScript, every screen is complete HTML and the menu still opens. The one
//! script adds two things: it pulls in the panels whose sources are slow, which are
//! otherwise reached by their own links, and it reveals the theme toggle.

pub(crate) mod capabilities;
pub(crate) mod command;
pub(crate) mod dashboard;
pub(crate) mod delivery;
pub(crate) mod deployments;
pub(crate) mod docs;
pub(crate) mod evidence;
pub(crate) mod governance;
pub mod icons;
pub(crate) mod mesh;
pub(crate) mod roadmap;
pub(crate) mod shell;
pub mod source;
pub(crate) mod view;
pub(crate) mod work;

use std::sync::Arc;

use crate::cockpit::assets::Assets;
use crate::http::router::{Request, Response};
use crate::live::{IntoLive, Live};

/// Every UI route starts here.
pub const PREFIX: &str = "/ui";

/// The directory of the distribution's `share/` that holds the UI's files.
pub const ASSETS_DIR: &str = "ui";

/// The URL prefix the UI's files are served under.
pub const ASSETS_PREFIX: &str = "/ui/assets/";

/// The UI's favicon, written from the canonical mark by `majordomus generate`.
pub const FAVICON: &str = "share/ui/favicon.svg";

/// The stylesheet every screen links.
pub const STYLESHEET: &str = "ui.css";

/// The script every screen loads.
pub const SCRIPT: &str = "ui.js";

/// The vendored graph library, loaded only by a screen that draws a graph.
pub const GRAPH_LIB: &str = "vendor/cytoscape.min.js";

/// The query parameter that asks a panel route for the panel alone, without the frame:
/// what the script fetches to fill a slow panel in place.
pub const FRAGMENT: &str = "fragment";

/// One screen of the UI: where it is, what the navigation calls it, its icon in the kit's
/// set, and the question it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    /// The route.
    pub route: &'static str,
    /// The navigation label, in the project's vocabulary.
    pub title: &'static str,
    /// The kit icon drawn beside the label.
    pub icon: &'static str,
    /// One line: the question the screen answers.
    pub answers: &'static str,
}

/// The screens there are, in the order the navigation shows them. A screen of the owner's
/// concept that has no route yet is not listed, because a link to nothing is a dead end
/// (`every_screen_is_answered_and_every_answered_route_is_a_screen_or_a_panel`).
pub const SCREENS: &[Screen] = &[
    Screen {
        route: PREFIX,
        title: "Dashboard",
        icon: "home",
        answers: "How the delivery stands: health, release, plan, mesh and the queue.",
    },
    Screen {
        route: command::ROUTE,
        title: "Command",
        icon: "terminal",
        answers: "Everything that can be run, what it changes, and what is running.",
    },
    Screen {
        route: roadmap::ROUTE,
        title: "Roadmap",
        icon: "target",
        answers: "Which milestones are open, in what order, and which intents they serve.",
    },
    Screen {
        route: work::ROUTE,
        title: "Issues & PRs",
        icon: "merge",
        answers: "Which issues are ready or blocked, and what the pull-request queue would do.",
    },
    Screen {
        route: capabilities::ROUTE,
        title: "Capabilities",
        icon: "puzzle",
        answers: "Everything the product can be asked, and on which surfaces.",
    },
    Screen {
        route: mesh::ROUTE,
        title: "Mesh",
        icon: "nodes",
        answers: "Which machines and runtimes are on the mesh, and what they recorded.",
    },
    Screen {
        route: delivery::ROUTE,
        title: "CI/CD & Releases",
        icon: "layers",
        answers: "What was released, what the tree declares, and what the CI model runs.",
    },
    Screen {
        route: deployments::ROUTE,
        title: "Deployments",
        icon: "server",
        answers: "What is live, and whether it is what was built.",
    },
    Screen {
        route: evidence::ROUTE,
        title: "Evidence",
        icon: "file-check",
        answers: "Which claims hold, which are stale or untested, and why.",
    },
    Screen {
        route: docs::ROUTE,
        title: "Documentation",
        icon: "book",
        answers: "What the product does, and where it is written down.",
    },
    Screen {
        route: governance::ROUTE,
        title: "Governance",
        icon: "shield-check",
        answers: "Which rules bind this repository and how each is proven.",
    },
];

/// The routes that answer one panel of a screen, each also a page of its own for a reader
/// without JavaScript.
pub const PANELS: &[&str] = &[dashboard::PULL_REQUESTS_PANEL, deployments::DELIVERY_PANEL];

/// The UI over the repository as it is now.
pub struct Ui {
    live: Arc<Live>,
    version: &'static str,
    assets: Assets,
}

impl Ui {
    /// A UI over a view of the repository, serving its assets from `share_dir/ui`.
    pub fn new(
        live: impl IntoLive,
        version: &'static str,
        share_dir: Option<&std::path::Path>,
    ) -> Self {
        let assets = match share_dir {
            Some(dir) => Assets::under(dir, ASSETS_DIR, ASSETS_PREFIX),
            None => Assets::none_under(ASSETS_DIR, ASSETS_PREFIX),
        };
        if !assets.present() {
            tracing::warn!(
                "the distribution has no share/ui/ directory: the UI will serve unstyled markup; run scripts/cockpit-assets to build it"
            );
        }
        Ui {
            live: live.into_live(),
            version,
            assets,
        }
    }

    /// Does this request belong to the UI?
    ///
    /// ```
    /// use majordomus_cli::ui::Ui;
    /// assert!(Ui::owns("/ui"));
    /// assert!(Ui::owns("/ui/panels/pull-requests"));
    /// assert!(!Ui::owns("/uix"));
    /// assert!(!Ui::owns("/cockpit"));
    /// ```
    pub fn owns(path: &str) -> bool {
        path == PREFIX || path.starts_with(&format!("{PREFIX}/"))
    }

    /// Answer one request under [`PREFIX`].
    pub fn handle(&self, req: &Request) -> Response {
        if req.method != "GET" {
            return Response::error(
                405,
                "method_not_allowed",
                "the UI's screens are read with GET; a capability that changes something is called on its own route",
            );
        }
        if let Some(name) = req.path.strip_prefix(ASSETS_PREFIX) {
            let version = req
                .query
                .iter()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.as_str());
            return self.assets.respond(name, version);
        }
        // one reading of the repository for the whole screen
        let ctx = self.live.current();
        let fragment = req.query.iter().any(|(k, _)| k == FRAGMENT);
        let path = req.path.trim_end_matches('/');
        let screen = match path {
            "" | PREFIX => dashboard::screen(&ctx),
            roadmap::ROUTE => roadmap::screen(&ctx),
            work::ROUTE => work::screen(&ctx, &req.query),
            mesh::ROUTE => mesh::screen(&ctx, &req.query),
            delivery::ROUTE => delivery::screen(&ctx),
            deployments::ROUTE => deployments::screen(&ctx),
            command::ROUTE => command::screen(&ctx, &req.query),
            capabilities::ROUTE => capabilities::screen(&ctx, &req.query),
            docs::ROUTE => docs::screen(&ctx),
            evidence::ROUTE => evidence::screen(&ctx),
            governance::ROUTE => governance::screen(&ctx),
            dashboard::PULL_REQUESTS_PANEL => dashboard::pull_requests_panel(&ctx, &req.query),
            deployments::DELIVERY_PANEL => deployments::delivery_panel(&ctx),
            other => shell::not_found(other),
        };
        let body = if fragment {
            screen.main.render()
        } else {
            shell::page(
                &ctx,
                &shell::Frame {
                    current: path,
                    stylesheet: self.assets.url(STYLESHEET),
                    favicon: self.assets.url("favicon.svg"),
                    script: self.assets.url(SCRIPT),
                    graph_lib: self.assets.url(GRAPH_LIB),
                    assets_present: self.assets.present(),
                    version: self.version,
                },
                &screen,
            )
        };
        let mut response = Response::new(screen.status, "text/html; charset=utf-8", body);
        for (name, value) in [
            ("Cache-Control", "no-cache"),
            ("Content-Security-Policy", crate::cockpit::csp()),
            ("X-Content-Type-Options", "nosniff"),
            ("Referrer-Policy", "no-referrer"),
        ] {
            response.headers.push((name.into(), value.into()));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The navigation and the dispatcher describe one UI. The dispatcher's arms are read
    /// from this file's own source, so an arm added without a screen or a panel, or a
    /// screen with no arm, fails here.
    #[test]
    fn every_screen_is_answered_and_every_answered_route_is_a_screen_or_a_panel() {
        let source = include_str!("mod.rs");
        let dispatcher = source
            .split("let screen = match path {")
            .nth(1)
            .and_then(|rest| rest.split("other =>").next())
            .expect("the dispatcher's match");
        let arms: Vec<&str> = dispatcher
            .lines()
            .filter_map(|l| l.trim().split_once(" =>").map(|(arm, _)| arm.trim()))
            .collect();
        assert_eq!(
            arms.len(),
            SCREENS.len() + PANELS.len(),
            "an arm that is neither a screen nor a panel: {arms:?}"
        );
        for screen in SCREENS {
            assert!(Ui::owns(screen.route), "{} is outside the UI", screen.route);
        }
        for panel in PANELS {
            assert!(Ui::owns(panel), "{panel} is outside the UI");
        }
        let mut routes: Vec<&str> = SCREENS.iter().map(|s| s.route).collect();
        routes.extend(PANELS);
        routes.sort_unstable();
        routes.dedup();
        assert_eq!(
            routes.len(),
            SCREENS.len() + PANELS.len(),
            "a route is declared twice"
        );
    }

    /// Every screen renders against a real registry, with its hero and at least one panel,
    /// and none of them shows a value it did not read.
    #[test]
    fn every_screen_renders_its_hero_and_panels() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        for (route, view) in [
            (PREFIX, dashboard::screen(&ctx)),
            (roadmap::ROUTE, roadmap::screen(&ctx)),
            (
                work::ROUTE,
                work::screen(&ctx, &[("pr".into(), "1".into())]),
            ),
            (
                mesh::ROUTE,
                mesh::screen(&ctx, &[("node".into(), "nowhere".into())]),
            ),
            (delivery::ROUTE, delivery::screen(&ctx)),
            (evidence::ROUTE, evidence::screen(&ctx)),
            (governance::ROUTE, governance::screen(&ctx)),
            (command::ROUTE, command::screen(&ctx, &[])),
            (
                capabilities::ROUTE,
                capabilities::screen(&ctx, &[("id".into(), "plan.status".into())]),
            ),
            (deployments::ROUTE, deployments::screen(&ctx)),
            (docs::ROUTE, docs::screen(&ctx)),
        ] {
            let html = view.main.render();
            assert_eq!(view.status, 200, "{route}");
            assert!(html.contains("mj-ui-hero"), "{route} has no hero");
            assert!(html.contains("mj-ui-panel"), "{route} has no panel");
            assert!(html.contains("mj-ui-source"), "{route} names no source");
            assert!(
                !html.contains("NaN") && !html.contains("undefined"),
                "{route}"
            );
        }
    }
}
