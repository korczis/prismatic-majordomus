//! The frame every screen of the UI is drawn in: the sidebar, the top bar, and the screen.
//!
//! The sidebar is the product's: the brand, [`super::SCREENS`] and nothing else as its
//! navigation, and at its foot the version card, read from `distribution.releases` like any
//! other value. The top bar holds the search and the links to the other surfaces of the
//! process — the documentation and the API — named by the constants of the modules that
//! declare them, and the Cockpit as the technical reference. On a narrow screen the sidebar
//! is a menu that opens without JavaScript (a checkbox and its label).
//!
//! It is an application shell: the screens stand in the groups of [`super::GROUPS`], and
//! under the top bar a trail says where the reader is — the dashboard, the group, the
//! screen, and what the query selected — beside the kinds of record the screen's facts are
//! declared as, each with how many this repository declares (`repository.info`). A screen
//! that is empty therefore names what would fill it, and a kind that is declared leads to
//! its records.

use crate::capability::Context;
use crate::cockpit::html::{document, el, El};
use crate::cockpit::view::{alert, theme_bootstrap};
use crate::design::DesignSystem;

use super::icons::icon;
use super::source::{ask, Reading, Source};
use super::view::{day, shown};
use super::{screens_of, Screen, FRAGMENT, GROUPS, PREFIX, SCREENS};

/// The source of the declared kinds: how many objects of each kind this repository holds.
const REPOSITORY: Source = Source::of("repository.info");

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
    /// The query the route was asked with: what the reader selected or filtered by, which
    /// the trail names after the screen.
    pub query: &'a [(String, String)],
    /// The stylesheet URL, with its digest.
    pub stylesheet: String,
    /// The favicon URL, with its digest.
    pub favicon: String,
    /// The script URL, with its digest.
    pub script: String,
    /// The graph library's URL, with its digest: loaded by the script only on a screen that
    /// draws a graph.
    pub graph_lib: String,
    /// Whether the distribution has the UI's asset directory at all.
    pub assets_present: bool,
    /// The executable's version.
    pub version: &'a str,
}

/// One whole page: the frame around a screen.
pub fn page(ctx: &Context, frame: &Frame<'_>, view: &View) -> String {
    let head = vec![
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
    ];
    let design = DesignSystem::compiled().ok();
    let body = el("body")
        .class("mj-body mj-ui")
        .attr_if(
            "data-theme-key",
            design.map(|d| d.theme.storage_key.clone()),
        )
        .attr_if("data-theme-class", design.map(|d| d.theme.class.clone()))
        .attr_if("data-design", design.map(|d| d.short_fingerprint()))
        .attr("data-graph-lib", frame.graph_lib.clone())
        .child(
            el("a")
                .class("mj-skip")
                .attr("href", "#main")
                .text("Skip to content"),
        )
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
                        .child(wayfinding(ctx, frame, view))
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
    document(&format!("{} · majordomus ui", view.title), head, body)
}

/// The sidebar: the brand, the screens group by group, and the version card.
fn sidebar(ctx: &Context, frame: &Frame<'_>) -> El {
    let item = |s: &Screen| {
        el("li").child(
            el("a")
                .class("mj-ui-nav-item")
                .attr("href", s.route)
                .attr("title", s.answers)
                .attr_if("aria-current", (frame.current == s.route).then_some("page"))
                .child(icon(s.icon))
                .child(el("span").class("mj-ui-label").text(s.title)),
        )
    };
    let groups = GROUPS.iter().map(|group| {
        let id = format!("mj-ui-group-{}", group.to_lowercase());
        el("div")
            .class("mj-ui-nav-group")
            .child(
                el("p")
                    .class("mj-ui-nav-heading")
                    .attr("id", id.clone())
                    .child(el("span").class("mj-ui-label").text(*group)),
            )
            .child(
                el("ul")
                    .class("mj-ui-nav-list")
                    .attr("aria-labelledby", id)
                    .children(screens_of(group).map(item)),
            )
    });
    el("aside")
        .class("mj-ui-sidebar")
        .attr("aria-label", "majordomus ui")
        .child(
            el("a")
                .class("mj-ui-brand")
                .attr("href", PREFIX)
                .child(
                    el("span")
                        .class("mj-ui-mark")
                        .attr("aria-hidden", "true")
                        .raw(MARK),
                )
                .child(
                    el("span")
                        .class("mj-ui-brand-text")
                        .child(el("span").class("mj-ui-brand-name").text("Majordomus"))
                        .child(el("span").class("mj-ui-brand-sub").text("majordomus ui")),
                ),
        )
        .child(
            el("nav")
                .class("mj-ui-nav")
                .attr("aria-label", "Screens")
                .children(groups),
        )
        .child(version_card(ctx, frame))
}

/// The bar under the top bar: where the reader is, and what the screen is declared as.
fn wayfinding(ctx: &Context, frame: &Frame<'_>, view: &View) -> El {
    let screen = SCREENS.iter().find(|s| s.route == frame.current);
    el("div")
        .class("mj-ui-wayfinding")
        .child(trail(frame, screen, &view.title))
        .children(screen.and_then(|s| declared(ctx, s)))
        .child(el("noscript").child(el("p").class("mj-ui-dim").text(
            "Without JavaScript nothing here is read again by itself, and a panel that arrives after its screen is opened by its own link.",
        )))
}

/// The trail: the dashboard, the screen's group, the screen, and each thing the query
/// selected. The last step is where the reader is and is not a link; a route that is no
/// screen (a panel read as a page, or nothing at all) is named by its own title.
fn trail(frame: &Frame<'_>, screen: Option<&Screen>, title: &str) -> El {
    let selected: Vec<String> = frame
        .query
        .iter()
        .filter(|(k, v)| k != FRAGMENT && !v.is_empty())
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    let mut steps: Vec<(String, Option<String>)> = Vec::new();
    if frame.current != PREFIX {
        steps.push(("Dashboard".into(), Some(PREFIX.into())));
    }
    match screen {
        Some(s) => {
            if s.route != PREFIX {
                steps.push((s.group.into(), None));
            }
            steps.push((s.title.into(), Some(s.route.into())));
        }
        None => steps.push((title.into(), None)),
    }
    steps.extend(selected.into_iter().map(|s| (s, None)));
    let last = steps.len().saturating_sub(1);
    el("nav")
        .class("mj-ui-crumbs")
        .attr("aria-label", "Where you are")
        .child(
            el("ol")
                .class("mj-ui-crumb-list")
                .children(steps.into_iter().enumerate().map(|(i, (label, href))| {
                    let step = match href {
                        Some(href) if i != last => {
                            el("a").class("mj-ui-crumb").attr("href", href).text(label)
                        }
                        _ => el("span")
                            .class("mj-ui-crumb")
                            .attr_if("aria-current", (i == last).then_some("location"))
                            .text(label),
                    };
                    el("li").class("mj-ui-crumb-item").child(step)
                })),
        )
}

/// What the screen's facts are declared as: each kind with how many objects of it this
/// repository holds, as `repository.info` counted them, leading to those objects. A kind
/// with none says so, which is what an empty screen is waiting for; a count that could not
/// be read is unknown and never zero. A screen that names no kind draws nothing.
fn declared(ctx: &Context, screen: &Screen) -> Option<El> {
    if screen.kinds.is_empty() {
        return None;
    }
    let info = ask(ctx, REPOSITORY);
    let kinds = info.ok().and_then(|v| v.get("kinds"));
    let chips = screen.kinds.iter().map(|kind| {
        let count = kinds.map(|k| k.get(*kind).and_then(serde_json::Value::as_u64));
        let (word, state) = match count {
            Some(Some(n)) => (n.to_string(), "declared"),
            Some(None) => ("none".to_string(), "absent"),
            None => ("unknown".to_string(), "unknown"),
        };
        el("li").child(
            el("a")
                .class("mj-ui-declared-kind")
                .attr("data-state", state)
                .attr("href", format!("{}/objects?kind={kind}", crate::cockpit::PREFIX))
                .attr(
                    "title",
                    match state {
                        "declared" => format!("the {word} object(s) of kind {kind} this repository declares"),
                        "absent" => format!("this repository declares no object of kind {kind}: this screen has nothing of it to show until one is recorded"),
                        _ => format!("{} could not say how many objects of kind {kind} there are", REPOSITORY.capability),
                    },
                )
                .child(el("span").class("mj-ui-declared-name").text(*kind))
                .child(el("span").class("mj-ui-declared-count").text(word)),
        )
    });
    Some(
        el("div")
            .class("mj-ui-declared")
            .attr("title", format!("read from {}", info.provenance()))
            .child(el("span").class("mj-ui-declared-label").text("Declared as"))
            .child(
                el("ul")
                    .class("mj-ui-declared-list")
                    .attr(
                        "aria-label",
                        "The kinds of record this screen is drawn from",
                    )
                    .children(chips),
            ),
    )
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

/// The top bar: the menu button of a narrow screen, the search, the other surfaces, the
/// theme toggle the script reveals.
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
                ),
        )
        .child(
            el("nav")
                .class("mj-ui-tools")
                .attr("aria-label", "Other surfaces")
                .child(tool(crate::web::discover::DOCS_MOUNT, "book", "Docs"))
                .child(tool(crate::http::swagger::SWAGGER_PATH, "code", "API"))
                .child(tool(crate::cockpit::PREFIX, "terminal", "Cockpit"))
                .child(
                    el("button")
                        .class("mj-ui-tool mj-ui-theme")
                        .attr("type", "button")
                        .attr("data-theme-toggle", "")
                        .attr("aria-label", "Switch between light and dark")
                        .flag("hidden")
                        .child(icon("eye"))
                        .child(el("span").class("mj-ui-label").text("Theme")),
                ),
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
            query: &[],
            stylesheet: "/ui/assets/ui.css?v=1".into(),
            favicon: "/ui/assets/favicon.svg?v=1".into(),
            script: "/ui/assets/ui.js?v=1".into(),
            graph_lib: "/ui/assets/vendor/cytoscape.min.js?v=1".into(),
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
            }})),
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
        let nav = html.split("class=\"mj-ui-nav\"").nth(1).unwrap_or_default();
        let nav = nav.split("</nav>").next().unwrap_or_default();
        assert!(nav.contains("mj-ui-nav-item"), "no navigation was read");
        assert!(
            !nav.contains("/cockpit"),
            "the navigation leads into the Cockpit: {nav}"
        );
        assert!(html.contains("aria-label=\"Other surfaces\""));
    }

    /// The navigation is the groups in their order, each with its screens, and the trail says
    /// where the reader is: the group and the screen, and what the query selected, with the
    /// last step no link.
    #[test]
    fn the_shell_groups_the_screens_and_says_where_the_reader_is() {
        let html = render(PREFIX);
        let mut at = 0;
        for group in GROUPS {
            let found = html[at..]
                .find(&format!("<span class=\"mj-ui-label\">{group}</span>"))
                .unwrap_or_else(|| panic!("{group} is missing or out of order"));
            at += found;
        }
        assert_eq!(html.matches("aria-current=\"location\"").count(), 1);

        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let query = vec![
            ("milestone".to_string(), "M001".to_string()),
            (FRAGMENT.to_string(), String::new()),
        ];
        let mut inspecting = frame(crate::ui::roadmap::ROUTE);
        inspecting.query = &query;
        let html = page(&ctx, &inspecting, &View::new("Roadmap", el("p")));
        let trail = html.split("mj-ui-crumb-list").nth(1).unwrap_or_default();
        let trail = trail.split("</ol>").next().unwrap_or_default();
        let dashboard = format!("href=\"{PREFIX}\"");
        let roadmap = format!("href=\"{}\"", crate::ui::roadmap::ROUTE);
        assert!(
            trail.contains(&dashboard) && trail.contains(&roadmap),
            "{trail}"
        );
        assert!(trail.contains(">Plan<"), "{trail}");
        assert!(
            trail.contains("aria-current=\"location\">milestone M001<"),
            "{trail}"
        );
        assert!(!trail.contains(FRAGMENT), "{trail}");

        // a route that is no screen is named by its own title
        let v = not_found("/ui/nowhere");
        let html = page(&ctx, &frame("/ui/nowhere"), &v);
        assert!(html.contains("aria-current=\"location\">Not a screen<"));
    }

    /// A screen says which kinds of record it is drawn from and how many of each the
    /// repository declares, as `repository.info` counted them: a kind with none is said to
    /// be absent, never drawn as zero records of something present.
    #[test]
    fn a_screen_names_the_kinds_it_is_declared_as_and_how_many_there_are() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let info = ask(&ctx, REPOSITORY);
        let kinds = info
            .ok()
            .and_then(|v| v.get("kinds"))
            .cloned()
            .expect("the kinds");
        for screen in SCREENS {
            let html = page(
                &ctx,
                &frame(screen.route),
                &View::new(screen.title, el("p")),
            );
            assert_eq!(
                html.matches("mj-ui-declared-kind").count(),
                screen.kinds.len(),
                "{}",
                screen.title
            );
            for kind in screen.kinds {
                let expected = match kinds.get(*kind).and_then(serde_json::Value::as_u64) {
                    Some(n) => format!(
                        "data-state=\"declared\" href=\"/cockpit/objects?kind={kind}\" title=\"the {n} object(s)"
                    ),
                    None => format!(
                        "data-state=\"absent\" href=\"/cockpit/objects?kind={kind}\""
                    ),
                };
                assert!(html.contains(&expected), "{}: {kind}: {html}", screen.title);
            }
        }
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

    /// A route nobody answers is a 404 that leads back.
    #[test]
    fn an_unknown_route_is_a_404_that_leads_back() {
        let v = not_found("/ui/nowhere");
        assert_eq!(v.status, 404);
        assert!(v.main.render().contains("/ui/nowhere"));
    }
}
