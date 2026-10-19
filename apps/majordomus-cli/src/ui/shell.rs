//! The frame every screen of the UI is drawn in: the site's bar, the sidebar, the tool bar,
//! and the screen.
//!
//! The bar at the top is majordomus.dev's, the reference the product's look is taken from
//! (`project.the-ui-wears-the-sites-navbar`): the brand, the version, the site's four
//! questions, its search, its theme toggle and its call to action, as the site renders them
//! ([`super::navbar`]). The UI adds nothing to it and changes nothing in it but the version it
//! names and the entry that leads to the UI itself.
//!
//! Beneath it the UI's own: the sidebar, [`super::SCREENS`] and nothing else as its
//! navigation, and at its foot the version card, read from `distribution.releases` like any
//! other value. The tool bar holds the search and the links to the other surfaces of the
//! process — the documentation and the API — named by the constants of the modules that
//! declare them, and the Cockpit as the technical reference. On a narrow screen the sidebar
//! is a menu that opens without JavaScript (a checkbox and its label).

use crate::capability::Context;
use crate::cockpit::html::{document_on, el, El};
use crate::cockpit::view::{alert, theme_bootstrap};
use crate::design::DesignSystem;

use super::icons::icon;
use super::navbar;
use super::source::{ask, Reading, Source};
use super::view::{day, shown};
use super::{PREFIX, SCREENS, SURFACE};

/// The canonical mark, the same file the Cockpit's top bar and the favicons are written from.
pub const MARK: &str = include_str!("../cockpit/logo-mark.svg");

/// What a screen renders into the frame.
pub struct View {
    /// The title, for the document's `<title>`; the screen's heading is in its hero.
    pub title: String,
    /// The screen.
    pub main: El,
    /// The HTTP status.
    pub status: u16,
}

impl View {
    /// A screen that answered.
    pub fn new(title: impl Into<String>, main: El) -> Self {
        View {
            title: title.into(),
            main,
            status: 200,
        }
    }
}

/// Where the frame is drawn, and with which files.
pub struct Frame<'a> {
    /// The route being answered, which marks the current screen.
    pub current: &'a str,
    /// The stylesheet URL, with its digest.
    pub stylesheet: String,
    /// The favicon URL, with its digest.
    pub favicon: String,
    /// The script URL, with its digest.
    pub script: String,
    /// The site's own scripts the bar's behaviours are written in — its menus, then its theme
    /// toggle — served from the UI's files, with their digests.
    pub bar_scripts: Vec<String>,
    /// The graph library's URL, with its digest: loaded by the script only on a screen that
    /// draws a graph.
    pub graph_lib: String,
    /// The module that wires an issue's moves (the Cockpit's `moves.js`), with its digest:
    /// loaded by the script only on a screen that renders a move form (I2781).
    pub moves_lib: String,
    /// The Cockpit's one WebSocket client (`socket.js`), with its digest: loaded by the script
    /// only on a screen that follows the executions this server runs (I2783).
    pub socket_lib: String,
    /// Whether the distribution has the UI's asset directory at all.
    pub assets_present: bool,
    /// The executable's version.
    pub version: &'a str,
}

/// One whole page: the frame around a screen.
pub fn page(ctx: &Context, frame: &Frame<'_>, view: &View) -> String {
    let head: Vec<El> = vec![
        el("link")
            .attr("rel", "stylesheet")
            .attr("href", frame.stylesheet.clone()),
        el("link")
            .attr("rel", "icon")
            .attr("href", frame.favicon.clone()),
        // the one inline script, the statement every surface ships for its theme; the policy
        // allows it by its digest (crate::cockpit::csp)
        el("script").raw(theme_bootstrap()),
        el("script")
            .attr("type", "module")
            .attr("src", frame.script.clone())
            .flag("defer"),
    ]
    .into_iter()
    .chain(
        frame
            .bar_scripts
            .iter()
            .map(|src| el("script").attr("src", src.clone()).flag("defer")),
    )
    .collect();
    let design = DesignSystem::compiled().ok();
    // the theme contract on the root too, where the site's theme script reads it
    let root: Vec<(&str, String)> = design
        .map(|d| {
            vec![
                ("data-theme-key", d.theme.storage_key.clone()),
                ("data-theme-class", d.theme.class.clone()),
            ]
        })
        .unwrap_or_default();
    let body = el("body")
        .class("mj-body mj-ui")
        .attr_if(
            "data-theme-key",
            design.map(|d| d.theme.storage_key.clone()),
        )
        .attr_if("data-theme-class", design.map(|d| d.theme.class.clone()))
        .attr_if("data-design", design.map(|d| d.short_fingerprint()))
        .attr("data-graph-lib", frame.graph_lib.clone())
        .attr("data-moves-lib", frame.moves_lib.clone())
        .attr("data-socket-lib", frame.socket_lib.clone())
        .attr("data-events", crate::http::events::PATH)
        .child(
            el("a")
                .class("mj-skip")
                .attr("href", "#main")
                .text("Skip to content"),
        )
        .raw(navbar::render(frame.version, &[(SURFACE, PREFIX)]))
        .child(
            el("input")
                .class("mj-ui-menu-toggle")
                .attr("type", "checkbox")
                .attr("id", "mj-ui-menu")
                .attr("aria-label", "Show the menu"),
        )
        .child(
            el("div")
                .class("mj-ui-frame")
                .child(sidebar(ctx, frame))
                .child(
                    el("div")
                        .class("mj-ui-body")
                        .child(topbar(&crate::peers::rfc3339(std::time::SystemTime::now())))
                        .child(
                            el("main")
                                .attr("id", "main")
                                .class("mj-ui-main")
                                .when(!frame.assets_present, |m| {
                                    m.child(alert(
                                        "warn",
                                        "This distribution has no share/ui/ directory, so the UI is unstyled. Run scripts/cockpit-assets.",
                                    ))
                                })
                                .child(view.main.clone()),
                        ),
                ),
        );
    document_on(
        &format!("{} · majordomus ui", view.title),
        &root,
        head,
        body,
    )
}

/// The sidebar: the screens, and the version card.
fn sidebar(ctx: &Context, frame: &Frame<'_>) -> El {
    let screens = el("ul")
        .class("mj-ui-nav-list")
        .attr("aria-label", "Screens")
        .children(SCREENS.iter().map(|s| {
            el("li").child(
                el("a")
                    .class("mj-ui-nav-item")
                    .attr("href", s.route)
                    .attr("title", s.answers)
                    .attr_if("aria-current", (frame.current == s.route).then_some("page"))
                    .child(icon(s.icon))
                    .child(el("span").class("mj-ui-label").text(s.title)),
            )
        }));
    // no brand of its own: the bar above carries the site's
    el("aside")
        .class("mj-ui-sidebar")
        .attr("aria-label", "majordomus ui")
        .child(
            el("nav")
                .class("mj-ui-nav")
                .attr("aria-label", "Screens")
                .child(screens),
        )
        .child(version_card(ctx, frame))
}

/// The version card: what this executable is, and what was last released, as
/// `distribution.releases` answers it.
fn version_card(ctx: &Context, frame: &Frame<'_>) -> El {
    let releases = ask(ctx, Source::of("distribution.releases"));
    let latest = releases.read("/latest/version");
    let published = releases.read("/latest/published_at");
    let commit = releases.read("/latest/commit");
    el("div")
        .class("mj-ui-version-card")
        .attr("title", format!("read from {}", releases.provenance()))
        .child(
            el("p")
                .class("mj-ui-version-head")
                .child(
                    el("span")
                        .class("mj-ui-version")
                        .text(format!("v{}", frame.version)),
                )
                .child(
                    el("span")
                        .class("mj-ui-version-note")
                        .text("this executable"),
                ),
        )
        .child(
            el("p")
                .class("mj-ui-version-line")
                .text(release_line(&latest, &published, &commit)),
        )
}

/// The latest release in one line: its version, the day it was published and its short
/// commit, each one said to be missing in the reading's own words when it is.
fn release_line(latest: &Reading, published: &Reading, commit: &Reading) -> String {
    let day_of = match published.text() {
        Some(d) => day(&d),
        None => shown(published),
    };
    let short = match commit.text() {
        Some(c) => c.chars().take(10).collect::<String>(),
        None => shown(commit),
    };
    format!("Latest release {} · {day_of} · {short}", shown(latest))
}

/// The tool bar beneath the site's: the menu button of a narrow screen, the search, the other
/// surfaces. The theme toggle is the site's, in the bar above.
fn topbar(read_at: &str) -> El {
    el("header")
        .class("mj-ui-topbar")
        .child(
            el("label")
                .class("mj-ui-menu-button")
                .attr("for", "mj-ui-menu")
                .child(icon("menu"))
                .child(el("span").class("mj-sr-only").text("Menu")),
        )
        .child(
            el("form")
                .class("mj-ui-search")
                .attr("role", "search")
                .attr("method", "get")
                .attr("action", format!("{}/search", crate::cockpit::PREFIX))
                .child(icon("search"))
                .child(
                    el("label")
                        .class("mj-sr-only")
                        .attr("for", "mj-ui-q")
                        .text("Search capabilities, objects, commands"),
                )
                .child(
                    el("input")
                        .attr("id", "mj-ui-q")
                        .attr("type", "search")
                        .attr("name", "q")
                        .attr("placeholder", "Search capabilities, objects, commands…"),
                ),
        )
        .child(
            el("p")
                .class("mj-ui-fresh")
                .attr("aria-live", "polite")
                .attr("title", "when this screen read its sources; it reads them again every minute while it is open")
                .child(el("span").class("mj-ui-fresh-dot").attr("aria-hidden", "true"))
                .child(
                    el("span")
                        .text("Read at ")
                        .child(
                            el("time")
                                .attr("data-read-at", "")
                                .attr("datetime", read_at)
                                .text(read_at.replace('T', " ")),
                        ),
                )
                // what the channel a following screen holds is doing, written by the script
                // from the socket's own state and hidden until it has one (I2783)
                .child(el("span").attr("data-live-state", "").flag("hidden")),
        )
        .child(
            el("nav")
                .class("mj-ui-tools")
                .attr("aria-label", "Other surfaces")
                // the UI's own Documentation screen, which is always served and leads to the
                // built documentation when there is one; /docs itself answers 404 until
                // scripts/site-build has run, and a top bar on every screen must not lead there
                .child(tool(super::docs::ROUTE, "book", "Docs"))
                .child(tool(crate::http::swagger::SWAGGER_PATH, "code", "API"))
                .child(tool(crate::cockpit::PREFIX, "terminal", "Cockpit")),
        )
}

/// A link to another surface. Below the medium width its label is hidden and only the icon,
/// which is `aria-hidden`, is drawn, so the link carries its name itself.
fn tool(href: &str, glyph: &str, label: &str) -> El {
    el("a")
        .class("mj-ui-tool")
        .attr("href", href)
        .attr("aria-label", label)
        .child(icon(glyph))
        .child(el("span").class("mj-ui-label").text(label))
}

/// A route under the UI that answers nothing.
pub fn not_found(path: &str) -> View {
    View {
        title: "Not a screen".into(),
        main: el("div")
            .class("mj-ui-stack")
            .child(alert("fail", format!("The UI serves no screen at {path}.")))
            .child(
                el("p").child(
                    el("a")
                        .class("mj-ui-link")
                        .attr("href", PREFIX)
                        .text("Back to the dashboard"),
                ),
            ),
        status: 404,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};

    fn frame(current: &str) -> Frame<'_> {
        Frame {
            current,
            stylesheet: "/ui/assets/ui.css?v=1".into(),
            favicon: "/ui/assets/favicon.svg?v=1".into(),
            script: "/ui/assets/ui.js?v=1".into(),
            bar_scripts: vec![
                "/ui/assets/nav.js?v=1".into(),
                "/ui/assets/theme.js?v=1".into(),
            ],
            graph_lib: "/ui/assets/vendor/cytoscape.min.js?v=1".into(),
            moves_lib: "/cockpit/assets/moves.js?v=1".into(),
            socket_lib: "/cockpit/assets/socket.js?v=1".into(),
            assets_present: true,
            version: "0.0.0",
        }
    }

    /// The release line shows the day and the short commit of a release it was given, and
    /// says which reading is missing when it was not.
    #[test]
    fn the_release_line_says_what_it_read_and_what_is_missing() {
        let releases = crate::ui::source::Answer {
            source: Source::of("distribution.releases"),
            route: None,
            exposure: Default::default(),
            value: Ok(serde_json::json!({ "latest": {
                "version": "0.20.0",
                "published_at": "2026-10-10T16:38:00Z",
                "commit": "c8f0e4c50123456789",
            }})
            .into()),
            took: std::time::Duration::ZERO,
            standing: crate::ui::source::Standing::NOW,
        };
        let line = release_line(
            &releases.read("/latest/version"),
            &releases.read("/latest/published_at"),
            &releases.read("/latest/commit"),
        );
        assert_eq!(line, "Latest release 0.20.0 · 2026-10-10 · c8f0e4c501");
        let missing = releases.read("/latest/nothing");
        assert_eq!(
            release_line(&missing, &missing, &missing),
            "Latest release none · none · none"
        );
    }

    /// A distribution without the UI's files still serves every screen, and says why it
    /// is unstyled.
    #[test]
    fn an_unstyled_distribution_says_so_on_every_page() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let mut bare = frame("/ui");
        bare.assets_present = false;
        let html = page(&ctx, &bare, &View::new("Dashboard", el("p")));
        assert!(html.contains("the UI is unstyled"), "{html}");
    }

    fn render(current: &str) -> String {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        page(&ctx, &frame(current), &View::new("Dashboard", el("p")))
    }

    /// Every screen is in the sidebar with its icon, the current one is marked, and the
    /// version card says what was last released from the source that knows.
    #[test]
    fn the_sidebar_is_the_screens_and_the_version_card() {
        let html = render(PREFIX);
        for s in SCREENS {
            assert!(
                html.contains(&format!("href=\"{}\"", s.route)),
                "{}",
                s.route
            );
        }
        assert_eq!(html.matches("aria-current=\"page\"").count(), 1);
        assert!(html.contains("Latest release"));
        assert!(html.contains("<title>Dashboard · majordomus ui</title>"));
    }

    /// The UI is its own product: the Cockpit appears in its frame only as the technical
    /// reference among the other surfaces, never in its navigation.
    #[test]
    fn the_cockpit_is_a_reference_not_a_destination() {
        let html = render(PREFIX);
        let nav = html.split("mj-ui-nav-list").nth(1).unwrap_or_default();
        let nav = nav.split("</ul>").next().unwrap_or_default();
        assert!(
            !nav.contains("/cockpit"),
            "the navigation leads into the Cockpit: {nav}"
        );
        assert!(html.contains("aria-label=\"Other surfaces\""));
    }

    /// The page carries exactly one inline script, the theme statement the policy allows,
    /// and the menu opens without one.
    #[test]
    fn the_only_inline_script_is_the_one_the_policy_allows() {
        let html = render(PREFIX);
        assert_eq!(html.matches("<script>").count(), 1, "{html}");
        assert!(html.contains("for=\"mj-ui-menu\""));
        assert!(crate::cockpit::csp().contains("script-src 'self' 'sha256-"));
    }

    /// Every page wears the site's bar above the UI's own frame: the site's landmark, its
    /// theme toggle and its menus, the executable's version in the badge, the entry for the
    /// product leading to the UI itself, and the theme contract on the root where the site's
    /// script reads it. The UI draws no theme toggle and no brand of its own beside it.
    #[test]
    fn every_page_wears_the_sites_bar() {
        let html = render(PREFIX);
        assert_eq!(html.matches("aria-label=\"Main\"").count(), 1, "{html}");
        assert!(html.find("aria-label=\"Main\"") < html.find("mj-ui-frame"));
        assert!(html.contains("id=\"theme-toggle\""));
        assert!(html.contains(">v0.0.0<"), "the badge names the executable");
        assert!(
            html.contains("href=\"/ui\" data-nav-primary=\"ui\""),
            "{html}"
        );
        assert!(!html.contains("data-theme-toggle"), "a second theme toggle");
        assert!(!html.contains("mj-ui-brand"), "a second brand");
        assert!(
            html.contains("<html lang=\"en\" data-theme-key=\""),
            "{html}"
        );
        for s in ["/ui/assets/nav.js?v=1", "/ui/assets/theme.js?v=1"] {
            assert!(html.contains(&format!("<script src=\"{s}\" defer>")), "{s}");
        }
    }

    /// A route nobody answers is a 404 that leads back.
    #[test]
    fn an_unknown_route_is_a_404_that_leads_back() {
        let v = not_found("/ui/nowhere");
        assert_eq!(v.status, 404);
        assert!(v.main.render().contains("/ui/nowhere"));
    }
}
