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
//! was read from (`source`): inferred, derived and projected, never kept. It holds no data,
//! no status vocabulary and no route list beyond `SCREENS` and `PANELS`, which a test
//! holds against the dispatcher. It shares mechanics rather than copying them:
//! - the element builder and the asset server, pointed at `share/ui/`;
//! - the content-security policy, with its one hash-pinned inline script;
//! - the design tokens, the kit and the icon set every surface draws from.
//!
//! ```mermaid
//! flowchart LR
//!   declaration["capability!"] --> registry["registry"] --> executor["executor"]
//!   executor --> api["MCP, HTTP, command line"]
//!   executor --> ui["UI · /ui<br/>(the product)"]
//!   executor --> cockpit["Cockpit · /cockpit<br/>(its technical reference)"]
//! ```
//!
//! With no JavaScript, every screen is complete HTML and the menu still opens. The one
//! script adds two things: it pulls in the panels whose sources are slow, which are
//! otherwise reached by their own links, and it reveals the theme toggle.
//!
//! The UI is mounted on its own prefix, named in the web topology and among the local
//! services by one id, and is a surface apart from the Cockpit:
//!
//! ```
//! use majordomus_cli::ui::{PREFIX, SURFACE};
//! assert_eq!(PREFIX, "/ui");
//! assert_eq!(SURFACE, "ui");
//! assert_ne!(PREFIX, majordomus_cli::cockpit::PREFIX, "the Cockpit is another surface");
//! ```

pub(crate) mod capabilities;
pub(crate) mod claim;
pub(crate) mod command;
pub(crate) mod dashboard;
pub(crate) mod delivery;
pub(crate) mod deployments;
pub(crate) mod docs;
pub(crate) mod evidence;
pub(crate) mod governance;
pub(crate) mod icons;
pub(crate) mod idd;
pub(crate) mod intent;
pub(crate) mod issues;
pub(crate) mod knowledge;
pub(crate) mod mesh;
pub(crate) mod milestone;
pub(crate) mod navbar;
pub(crate) mod observatory;
pub(crate) mod prose;
pub(crate) mod prs;
pub(crate) mod roadmap;
pub(crate) mod rule;
pub(crate) mod shell;
pub(crate) mod skills;
pub(crate) mod source;
pub(crate) mod timings;
pub(crate) mod view;
pub(crate) mod work;

use std::sync::Arc;

use crate::cockpit::assets::Assets;
use crate::http::router::{Request, Response};
use crate::live::{IntoLive, Live};

/// Every UI route starts here.
pub const PREFIX: &str = "/ui";

/// The UI's id in the web topology and among the local services, the one name every
/// reader of "which surface is the UI" asks.
pub const SURFACE: &str = "ui";

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
/// The site's own scripts the bar every screen wears behaves by — its menus, then its theme
/// toggle — served as byte copies of `site/nav.js` and `site/theme.js` that
/// `scripts/cockpit-assets` writes and checks ([`navbar`]).
pub(crate) const BAR_SCRIPTS: &[&str] = &["nav.js", "theme.js"];

/// The vendored graph library, loaded only by a screen that draws a graph.
pub const GRAPH_LIB: &str = "vendor/cytoscape.min.js";

/// The query parameter that asks a panel route for the panel alone, without the frame:
/// what the script fetches to fill a slow panel in place.
pub const FRAGMENT: &str = "fragment";

/// The request header a snapshot reads every page with: a screen asked with it carries each
/// of its slow panels already read, since a published page has no script to fetch them.
pub const SNAPSHOT_HEADER: &str = "X-Majordomus-Snapshot";

/// One screen of the UI: where it is, what the navigation calls it, its icon in the kit's
/// set, and the question it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Screen {
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
pub(crate) const SCREENS: &[Screen] = &[
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
        route: idd::ROUTE,
        title: "Intent workspace",
        icon: "list-checks",
        answers: "One intent and what serves it: milestones, issues, waves, dependencies and the next move.",
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
        route: issues::ROUTE,
        title: "Issues",
        icon: "list",
        answers: "Every issue of the plan, by status, milestone, priority or text.",
    },
    Screen {
        route: prs::ROUTE,
        title: "PR Drain",
        icon: "branch",
        answers: "Every open pull request, what the lane would do with it, and why.",
    },
    Screen {
        route: capabilities::ROUTE,
        title: "Capabilities",
        icon: "puzzle",
        answers: "Everything the product can be asked, and on which surfaces.",
    },
    Screen {
        route: skills::ROUTE,
        title: "Skills",
        icon: "sparkles",
        answers: "What an agent can be asked by name, and where each name comes from.",
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
        route: knowledge::ROUTE,
        title: "Knowledge",
        icon: "sparkles",
        answers: "What the work left for review, and whether the deriver is still writing.",
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
    Screen {
        route: observatory::ROUTE,
        title: "Observatory",
        icon: "eye",
        answers: "What the repository and its mesh are doing, drawn as rotating scenes for a wall display.",
    },
];

/// The routes that answer one panel of a screen, each also a page of its own for a reader
/// without JavaScript.
pub(crate) const PANELS: &[&str] = &[
    dashboard::PULL_REQUESTS_PANEL,
    deployments::DELIVERY_PANEL,
    prs::PANEL,
];

/// The UI over the repository as it is now.
pub(crate) struct Ui {
    live: Arc<Live>,
    version: &'static str,
    assets: Assets,
    /// Whether this UI's screens have been read once behind its first reader.
    warmed: std::sync::atomic::AtomicBool,
}

impl Ui {
    /// A UI over a view of the repository, serving its assets from `share_dir/ui`.
    pub(crate) fn new(
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
            warmed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Read every screen and every panel once, behind the first reader of this UI, so that
    /// the screen they open next has its sources read before they ask for it.
    ///
    /// The routes are [`SCREENS`] and [`PANELS`], the lists the navigation and the
    /// dispatcher are held to, so a screen added tomorrow is warmed with no edit here. What
    /// each request renders is discarded: its effect is that the sources it names are now
    /// remembered and kept current (`crate::capability::readings`). A process that runs
    /// every capability without its cache does not warm, and neither do the crate's own
    /// unit tests, whose fixtures are gone before a background reader would finish.
    pub(crate) fn warm_once(ui: &Arc<Ui>) {
        use std::sync::atomic::Ordering;
        if cfg!(test) || crate::capability::executor::bypassed() {
            return;
        }
        if ui.warmed.swap(true, Ordering::SeqCst) {
            return;
        }
        let ui = Arc::clone(ui);
        let warm = move || {
            let routes = SCREENS
                .iter()
                .map(|s| s.route)
                .chain(PANELS.iter().copied());
            for route in routes {
                let _ = ui.handle(&Request::parse_target("GET", route, Vec::new()));
            }
        };
        if let Err(refused) = std::thread::Builder::new()
            .name("mj-ui-warm".into())
            .spawn(warm)
        {
            tracing::warn!("no thread to warm the UI's screens: {refused}");
        }
    }

    /// Does this request belong to the UI? The prefix itself and every path below it, and
    /// nothing that merely starts with the same letters.
    #[cfg(test)]
    pub(crate) fn owns(path: &str) -> bool {
        path == PREFIX || path.starts_with(&format!("{PREFIX}/"))
    }

    /// Answer one request under [`PREFIX`].
    pub(crate) fn handle(&self, req: &Request) -> Response {
        if req.method != "GET" {
            return Response::error(
                405,
                "method_not_allowed",
                "the UI's screens are read with GET; a capability that changes something is called on its own route",
            );
        }
        if let Some(name) = req.path.strip_prefix(ASSETS_PREFIX) {
            return self.assets.respond(name, req);
        }
        // one reading of the repository for the whole screen: the generation there is, never
        // a wait for the next one (ADR 0142); a screen drawn from a generation the repository
        // has moved past says so, and the page's script reads it again
        let (reading, behind) = self.live.view_without_waiting();
        let ctx = reading.ctx;
        let fragment = req.query.iter().any(|(k, _)| k == FRAGMENT);
        let path = req.path.trim_end_matches('/');
        // an earlier or mistyped name leads to the route it meant, query kept
        if let Some((_, target)) = observatory::REDIRECTS.iter().find(|(old, _)| *old == path) {
            let query = req
                .query
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&");
            let to = match query.is_empty() {
                true => target.to_string(),
                false => format!("{target}?{query}"),
            };
            return Response::new(308, "text/plain; charset=utf-8", format!("{to}\n"))
                .with_header("Location", to);
        }
        // the kiosk is the Observatory's screen under a second name, never a second screen
        let kiosk = path == observatory::KIOSK;
        let path = if kiosk { observatory::ROUTE } else { path };
        let mut screen = self.view(&ctx, path, &req.query, kiosk);
        // A snapshot has no script to fetch a slow panel after the screen has painted, so the
        // panel is read now and stands where its placeholder stood, as the script would have
        // put it: a published screen carries what the live one shows (ADR 0141).
        if req.header(SNAPSHOT_HEADER).is_some() {
            screen.main = screen.main.replacing("data-panel-src", &mut |src| {
                let panel = Request::parse_target("GET", src, Vec::new());
                let route = panel.path.trim_end_matches('/');
                PANELS
                    .contains(&route)
                    .then(|| self.view(&ctx, route, &panel.query, false).main)
            });
        }
        if behind {
            screen.main = crate::cockpit::html::el("div")
                .class("mj-ui-stack")
                .child(
                    crate::cockpit::html::el("p")
                        .class("mj-ui-dim")
                        .attr(view::CATCHING_UP, "")
                        .text("The repository changed: this screen shows it as it was, and is being read again."),
                )
                .child(screen.main);
        }
        // A panel read as a page of its own has no hero, so it carries the page's one heading
        // itself; the fragment a screen pulls in does not, or the screen would have two.
        if !fragment && PANELS.contains(&path) {
            screen.main = crate::cockpit::html::el("div")
                .class("mj-ui-stack")
                .child(
                    crate::cockpit::html::el("h1")
                        .class("mj-ui-title")
                        .text(&screen.title),
                )
                .child(screen.main);
        }
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
                    bar_scripts: BAR_SCRIPTS.iter().map(|s| self.assets.url(s)).collect(),
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

    /// The view a route answers with: a screen, a panel, or the page that says the route is
    /// not one.
    fn view(
        &self,
        ctx: &crate::capability::Context,
        path: &str,
        query: &[(String, String)],
        kiosk: bool,
    ) -> shell::View {
        match path {
            "" | PREFIX => dashboard::screen(ctx),
            idd::ROUTE => idd::screen(ctx, query),
            roadmap::ROUTE => roadmap::screen(ctx, query),
            work::ROUTE => work::screen(ctx, query),
            issues::ROUTE => issues::screen(ctx, query),
            prs::ROUTE => prs::screen(ctx, query),
            mesh::ROUTE => mesh::screen(ctx, query),
            delivery::ROUTE => delivery::screen(ctx),
            deployments::ROUTE => deployments::screen(ctx),
            command::ROUTE => command::screen(ctx, query),
            capabilities::ROUTE => capabilities::screen(ctx, query),
            skills::ROUTE => skills::screen(ctx),
            docs::ROUTE => docs::screen(ctx),
            evidence::ROUTE => evidence::screen(ctx, query),
            knowledge::ROUTE => knowledge::screen(ctx),
            governance::ROUTE => governance::screen(ctx, query),
            observatory::ROUTE => {
                let libs = observatory::Libs {
                    director: self.assets.url(observatory::LIB),
                    scenes: self.assets.url(observatory::SCENES_LIB),
                    p5: self.assets.url(observatory::P5_LIB),
                };
                observatory::screen(ctx, query, &libs, kiosk)
            }
            dashboard::PULL_REQUESTS_PANEL => dashboard::pull_requests_panel(ctx, query),
            deployments::DELIVERY_PANEL => deployments::delivery_panel(ctx),
            prs::PANEL => prs::panel_view(ctx, query),
            other => shell::not_found(other),
        }
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
            .split("fn view(")
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
        // the prefix and what is below it, and nothing that only shares its letters
        assert!(Ui::owns("/ui") && Ui::owns("/ui/panels/pull-requests"));
        assert!(!Ui::owns("/uix") && !Ui::owns("/cockpit"));
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

    /// The Observatory has one screen and three more names: the kiosk is the same screen told
    /// to run unattended, its earlier name redirects to it, and the misspelling to the screen,
    /// each with its query kept.
    #[test]
    fn the_observatory_is_one_screen_under_its_names() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ui = Ui::new(repo.context().expect("a context"), "0.0.0-test", None);
        let get = |target: &str| ui.handle(&Request::parse_target("GET", target, Vec::new()));
        let text = |r: &Response| r.body.text().into_owned();
        let screen = get(observatory::ROUTE);
        assert_eq!(screen.status, 200);
        assert!(text(&screen).contains("data-observatory-mode=\"screen\""));
        let kiosk = get(observatory::KIOSK);
        assert_eq!(kiosk.status, 200);
        assert!(text(&kiosk).contains("data-observatory-mode=\"kiosk\""));
        for (old, target) in observatory::REDIRECTS {
            let r = get(&format!("{old}?scene=delivery"));
            assert_eq!(r.status, 308, "{old}");
            let to = r
                .headers
                .iter()
                .find(|(k, _)| k == "Location")
                .map(|(_, v)| v.clone());
            assert_eq!(to, Some(format!("{target}?scene=delivery")), "{old}");
        }
        assert_eq!(
            observatory::REDIRECTS,
            &[
                ("/ui/screensaver", "/ui/kiosk"),
                ("/ui/obsrvatory", "/ui/observatory")
            ]
        );
    }

    /// The UI answers in-process exactly as it does over a socket: every screen and panel as
    /// a page with one heading, a fragment without the frame, an asset by its digest, a
    /// route it does not serve as a page that says so, and anything but GET refused.
    #[test]
    fn the_ui_answers_every_route_in_process() {
        use crate::http::router::Request;
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let ui = Ui::new(std::sync::Arc::clone(&ctx), "0.0.0-test", Some(&share));
        let get = |target: &str| ui.handle(&Request::parse_target("GET", target, Vec::new()));

        for route in SCREENS
            .iter()
            .map(|s| s.route)
            .chain(PANELS.iter().copied())
        {
            let r = get(route);
            let html = r.body.text();
            assert_eq!(r.status, 200, "{route}");
            assert!(
                html.to_ascii_lowercase().starts_with("<!doctype html>"),
                "{route} is a page"
            );
            assert_eq!(html.matches("<h1").count(), 1, "{route} has one heading");
        }
        let fragment = get(&format!("{}?{FRAGMENT}", dashboard::PULL_REQUESTS_PANEL));
        let html = fragment.body.text();
        assert!(
            !html.to_ascii_lowercase().contains("<!doctype") && !html.contains("<h1"),
            "{html}"
        );

        let asset = get(&format!("{ASSETS_PREFIX}{SCRIPT}?v=anything"));
        assert_eq!(asset.status, 200, "the UI's script is served");

        let missing = get("/ui/nowhere");
        assert_eq!(missing.status, 404);
        assert!(missing
            .body
            .text()
            .contains("serves no screen at /ui/nowhere"));

        let post = ui.handle(&Request::parse_target("POST", PREFIX, Vec::new()));
        assert_eq!(post.status, 405);

        // without the distribution's files the screens still answer, unstyled and saying so
        let bare = Ui::new(ctx, "0.0.0-test", None);
        let r = bare.handle(&Request::parse_target("GET", PREFIX, Vec::new()));
        assert!(r.body.text().contains("the UI is unstyled"));
    }

    /// Every class a screen's source gives an element is one the UI's stylesheet defines, or
    /// a block whose parts it defines (`mj-ui-graph` beside `.mj-ui-graph-canvas`): a class
    /// nothing styles is a promise the page does not keep, as the search labels were visible
    /// while `mj-ui-sr-only` was meant to hide them (I2852). Read from the sources, so a
    /// screen added later is held too.
    #[test]
    fn every_class_a_screen_emits_is_one_the_stylesheet_defines() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // the stylesheet the UI ships, compiled from its source and the design files it imports
        let css = std::fs::read_to_string(root.join("../../share/ui/ui.css"))
            .expect("the UI's stylesheet");
        let defined: std::collections::BTreeSet<&str> = css
            .split('.')
            .skip(1)
            .filter_map(|s| {
                let end = s
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                    .unwrap_or(s.len());
                s[..end].starts_with("mj-").then(|| &s[..end])
            })
            .collect();
        assert!(
            defined.len() > 50,
            "the stylesheet was not read: {defined:?}"
        );
        let accounted = |c: &str| {
            defined.contains(c)
                || defined
                    .iter()
                    .any(|d| d.starts_with(&format!("{c}-")) || d.starts_with(&format!("{c}--")))
        };
        let mut undefined: Vec<String> = Vec::new();
        let mut seen = 0;
        for entry in std::fs::read_dir(root.join("src/ui")).expect("the UI's sources") {
            let path = entry.expect("a source").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("a source file");
            for (n, line) in source.lines().enumerate() {
                for call in line.split(".class(\"").skip(1) {
                    let value = call.split('"').next().unwrap_or_default();
                    for class in value.split_whitespace().filter(|c| c.starts_with("mj-")) {
                        seen += 1;
                        if !class.contains('{') && !accounted(class) {
                            let file = path.file_name().and_then(|f| f.to_str()).unwrap_or("?");
                            undefined.push(format!("{file}:{} {class}", n + 1));
                        }
                    }
                }
            }
        }
        assert!(
            seen > 100,
            "only {seen} class(es) were read from the sources"
        );
        assert!(
            undefined.is_empty(),
            "classes the stylesheet does not define: {undefined:?}"
        );
    }

    /// A screen asked as a snapshot carries every slow panel already read where its
    /// placeholder stood, as the script would have put it, and keeps its one heading; asked
    /// live it still paints first and defers the panel (I2850).
    #[test]
    fn a_snapshot_reads_every_slow_panel_into_its_screen() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ui = Ui::new(repo.context().expect("a context"), "0.0.0-test", None);
        let mut deferring = 0;
        for route in SCREENS.iter().map(|s| s.route) {
            let live = ui.handle(&Request::parse_target("GET", route, Vec::new()));
            if !live.body.text().contains("data-panel-src=") {
                continue;
            }
            deferring += 1;
            let mut asked = Request::parse_target("GET", route, Vec::new());
            asked
                .headers
                .push((SNAPSHOT_HEADER.to_string(), "1".to_string()));
            let r = ui.handle(&asked);
            let html = r.body.text();
            assert_eq!(r.status, 200, "{route}");
            assert!(
                !html.contains("data-panel-src="),
                "{route} defers a panel: {html}"
            );
            assert_eq!(html.matches("<h1").count(), 1, "{route} has one heading");
        }
        // the PR Drain, the Dashboard's pull requests, Deployments' delivery and Work
        assert!(deferring >= 3, "only {deferring} screen(s) defer a panel");
        let mut prs = Request::parse_target("GET", prs::ROUTE, Vec::new());
        prs.headers
            .push((SNAPSHOT_HEADER.to_string(), "1".to_string()));
        assert!(
            ui.handle(&prs)
                .body
                .text()
                .contains("forge was not observed"),
            "the drain's own answer stands in the snapshot's PR Drain"
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
            (
                idd::ROUTE,
                idd::screen(&ctx, &[("intent".into(), "no-such-intent".into())]),
            ),
            (
                roadmap::ROUTE,
                roadmap::screen(&ctx, &[("milestone".into(), "M001".into())]),
            ),
            (
                work::ROUTE,
                work::screen(&ctx, &[("pr".into(), "1".into())]),
            ),
            (
                issues::ROUTE,
                issues::screen(&ctx, &[("status".into(), "READY".into())]),
            ),
            (
                prs::ROUTE,
                prs::screen(&ctx, &[("issue".into(), "I0001".into())]),
            ),
            (
                mesh::ROUTE,
                mesh::screen(&ctx, &[("node".into(), "nowhere".into())]),
            ),
            (delivery::ROUTE, delivery::screen(&ctx)),
            (
                evidence::ROUTE,
                evidence::screen(&ctx, &[("claim".into(), "no-such-claim".into())]),
            ),
            (
                governance::ROUTE,
                governance::screen(&ctx, &[("rule".into(), "no.such-rule".into())]),
            ),
            (command::ROUTE, command::screen(&ctx, &[])),
            (
                capabilities::ROUTE,
                capabilities::screen(&ctx, &[("id".into(), "plan.status".into())]),
            ),
            (deployments::ROUTE, deployments::screen(&ctx)),
            (skills::ROUTE, skills::screen(&ctx)),
            (docs::ROUTE, docs::screen(&ctx)),
            (knowledge::ROUTE, knowledge::screen(&ctx)),
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
