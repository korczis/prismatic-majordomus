//! The shell every Cockpit page is rendered into, and the small vocabulary of components
//! the pages are built from.
//!
//! The pages carry no raw utility classes. Every class here is a semantic one the
//! Cockpit's stylesheet defines (`mj-card`, `mj-table`, `mj-badge--fail`), so the design
//! lives in one file a designer can read and the Rust says what a thing *is* rather than
//! how it looks. That is also what makes the stylesheet deterministic: it is a function of
//! its own source and nothing else, and the drift check over it is exact.
//!
//! Every `x-` attribute here names a *top-level* property or method of the one Alpine
//! component and never an expression — not even a dotted path, which the CSP build reads
//! as an expression and does not evaluate. A palette bound to `palette.open` looks right,
//! renders right, and leaves a full-page modal backdrop over every click. That is what the content-security policy requires: the Cockpit
//! ships Alpine's CSP build, which evaluates no strings, so the policy needs neither
//! `unsafe-eval` nor `unsafe-inline`. A handler that would take an argument gets its own
//! method in `cockpit.js` instead.

use crate::capability::CapabilityKind;

use super::html::{el, empty, El, Node};
use super::nav::{Area, Navigation};

/// The banner shown when the distribution has no `share/cockpit/` at all: the pages still
/// render, and the reader is told why they look like 1993.
const NO_ASSETS: &str = "This distribution has no share/cockpit/ directory, so the Cockpit is serving unstyled markup. Everything works; nothing is styled. Run `just cockpit-assets` to build it.";

/// What a page needs from the process in order to render its shell.
pub struct Shell<'a> {
    /// The page title, before the suffix.
    pub title: &'a str,
    /// One line under the heading: what this page shows.
    pub subtitle: Option<String>,
    /// The area of the navigation this page belongs to.
    pub area: Area,
    /// The trail from the Cockpit root to this page, label and href; the last has no href.
    pub breadcrumbs: Vec<(String, Option<String>)>,
    /// The navigation, derived from the registry.
    pub navigation: &'a Navigation,
    /// The stylesheet URL, with its digest.
    pub stylesheet: String,
    /// The Cockpit's own script modules, in load order, with their digests.
    pub scripts: Vec<String>,
    /// Whether the distribution has an asset directory at all.
    pub assets_present: bool,
    /// The executable's version, shown in the footer.
    pub version: &'a str,
}

/// Render one page into the shell.
pub fn page(shell: &Shell<'_>, main: El) -> String {
    let head = vec![
        el("link")
            .attr("rel", "stylesheet")
            .attr("href", shell.stylesheet.clone()),
        el("link")
            .attr("rel", "icon")
            .attr("href", "/cockpit/assets/favicon.svg"),
        // the theme is applied before first paint, so a dark-mode reader never sees a
        // white flash; this is the one inline script the Cockpit ships, and the
        // content-security policy names its digest rather than allowing inline scripts
        el("script").raw(THEME_BOOTSTRAP),
    ]
    .into_iter()
    .chain(shell.scripts.iter().map(|src| {
        el("script")
            .attr("type", "module")
            .attr("src", src.clone())
            .flag("defer")
    }))
    .collect();

    let body = el("body")
        .class("mj-body")
        .attr("x-data", "cockpit")
        .child(skip_link())
        .child(header(shell))
        .child(
            el("div").class("mj-layout").child(sidebar(shell)).child(
                el("main")
                    .attr("id", "main")
                    .class("mj-main")
                    .when(!shell.assets_present, |m| m.child(alert("warn", NO_ASSETS)))
                    .child(breadcrumbs(&shell.breadcrumbs))
                    .child(
                        el("header")
                            .class("mj-page-head")
                            .child(el("h1").class("mj-page-title").text(shell.title))
                            .node(match &shell.subtitle {
                                Some(s) => Node::Element(el("p").class("mj-page-subtitle").text(s)),
                                None => empty(),
                            }),
                    )
                    .child(main),
            ),
        )
        .child(palette())
        .child(footer(shell));

    super::html::document(&format!("{} · Majordomus Cockpit", shell.title), head, body)
}

/// Read the stored theme before the page paints. Kept to one statement so that its digest
/// is stable and the content-security policy can name it.
pub const THEME_BOOTSTRAP: &str = "try{var t=localStorage.getItem('mj-theme');if(t==='dark'||(t!=='light'&&matchMedia('(prefers-color-scheme: dark)').matches)){document.documentElement.classList.add('dark')}}catch(e){}";

fn skip_link() -> El {
    el("a")
        .class("mj-skip")
        .attr("href", "#main")
        .text("Skip to content")
}

fn header(shell: &Shell<'_>) -> El {
    el("header")
        .class("mj-topbar")
        .child(
            el("a")
                .class("mj-brand")
                .attr("href", "/cockpit")
                .child(el("span").class("mj-brand-mark").text("M"))
                .child(el("span").class("mj-brand-name").text("Majordomus"))
                .child(el("span").class("mj-brand-suffix").text("Cockpit")),
        )
        .child(
            el("button")
                .class("mj-palette-open")
                .attr("type", "button")
                .attr("x-on:click", "openPalette")
                .attr("aria-keyshortcuts", "Control+K Meta+K")
                .child(el("span").text("Search or run"))
                .child(el("kbd").class("mj-kbd").text("Ctrl K")),
        )
        .child(
            el("nav")
                .class("mj-topbar-links")
                .attr("aria-label", "External surfaces")
                .child(link(crate::http::swagger::DOCS_PATH, "Swagger"))
                .child(link("/openapi.json", "OpenAPI"))
                .child(
                    el("button")
                        .class("mj-theme-toggle")
                        .attr("type", "button")
                        .attr("x-on:click", "toggleTheme")
                        .attr("aria-label", "Switch between light and dark")
                        .attr("title", "Switch between light and dark")
                        .text("Theme"),
                ),
        )
        .child(el("span").class("mj-version").text(shell.version))
}

fn sidebar(shell: &Shell<'_>) -> El {
    let mut nav = el("nav")
        .class("mj-sidebar")
        .attr("aria-label", "Cockpit sections");
    for section in shell.navigation.sections() {
        let mut list = el("ul").class("mj-nav-list");
        for item in &section.items {
            let current = item.area == shell.area && item.current;
            list = list.child(
                el("li").child(
                    el("a")
                        .class(if current {
                            "mj-nav-link mj-nav-link--current"
                        } else {
                            "mj-nav-link"
                        })
                        .attr("href", item.href.clone())
                        .attr_if("aria-current", current.then_some("page"))
                        .child(el("span").class("mj-nav-label").text(&item.label))
                        .node(match item.count {
                            Some(n) => {
                                Node::Element(el("span").class("mj-nav-count").text(n.to_string()))
                            }
                            None => empty(),
                        }),
                ),
            );
        }
        nav = nav.child(
            el("div")
                .class("mj-nav-section")
                .child(el("h2").class("mj-nav-heading").text(&section.title))
                .child(list),
        );
    }
    nav
}

fn footer(shell: &Shell<'_>) -> El {
    el("footer")
        .class("mj-footer")
        .child(el("span").text(format!("majordomus {}", shell.version)))
        .child(el("span").text("·"))
        .child(link("/openapi.json", "openapi.json"))
        .child(el("span").text("·"))
        .child(link(crate::http::swagger::DOCS_PATH, "Swagger UI"))
        .child(el("span").text("·"))
        .child(link("/cockpit/health", "health"))
}

fn breadcrumbs(trail: &[(String, Option<String>)]) -> El {
    if trail.is_empty() {
        return el("div").class("mj-hidden");
    }
    let mut list = el("ol").class("mj-crumbs");
    for (label, href) in trail {
        list = list.child(el("li").child(match href {
            Some(h) => el("a").attr("href", h.clone()).text(label),
            None => el("span").attr("aria-current", "page").text(label),
        }));
    }
    el("nav").attr("aria-label", "Breadcrumb").child(list)
}

/// The command palette: an empty shell the script fills from the capability, graph and
/// object listings. No entry is written here, because every entry is something the
/// registry or the index already holds.
fn palette() -> El {
    el("div")
        .class("mj-palette")
        .attr("x-show", "paletteOpen")
        .attr("x-cloak", "")
        .attr("role", "dialog")
        .attr("aria-modal", "true")
        .attr("aria-label", "Command palette")
        .attr("x-on:keydown.escape.window", "closePalette")
        .child(
            el("div")
                .class("mj-palette-backdrop")
                .attr("x-on:click", "closePalette"),
        )
        .child(
            el("div")
                .class("mj-palette-panel")
                .child(
                    el("input")
                        .class("mj-palette-input")
                        .attr("type", "search")
                        .attr(
                            "placeholder",
                            "Go to a capability, an object, a graph, a page",
                        )
                        .attr("aria-label", "Command palette query")
                        .attr("x-ref", "paletteInput")
                        .attr("x-model", "paletteQuery")
                        .attr("x-on:input.debounce.120ms", "paletteFilter")
                        .attr("x-on:keydown.arrow-down.prevent", "paletteNext")
                        .attr("x-on:keydown.arrow-up.prevent", "palettePrevious")
                        .attr("x-on:keydown.enter.prevent", "paletteChoose"),
                )
                .child(
                    el("ul")
                        .class("mj-palette-results")
                        .attr("role", "listbox")
                        .attr("x-ref", "paletteResults"),
                )
                .child(
                    el("p")
                        .class("mj-palette-hint")
                        .text("Enter opens · arrows move · Escape closes"),
                ),
        )
}

// ------------------------------------------------------------------ components

/// A link.
pub fn link(href: impl Into<String>, label: impl Into<String>) -> El {
    el("a").class("mj-link").attr("href", href).text(label)
}

/// A card: the Cockpit's one container.
pub fn card(title: impl Into<String>, body: El) -> El {
    el("section")
        .class("mj-card")
        .child(el("h2").class("mj-card-title").text(title))
        .child(body)
}

/// A card with something in its header beside the title.
pub fn card_with(title: impl Into<String>, aside: El, body: El) -> El {
    el("section")
        .class("mj-card")
        .child(
            el("div")
                .class("mj-card-head")
                .child(el("h2").class("mj-card-title").text(title))
                .child(aside),
        )
        .child(body)
}

/// A key and its value, for a definition list of facts.
pub fn facts(rows: Vec<(&str, Node)>) -> El {
    let mut list = el("dl").class("mj-facts");
    for (key, value) in rows {
        list = list
            .child(el("dt").class("mj-facts-key").text(key))
            .child(el("dd").class("mj-facts-value").node(value));
    }
    list
}

/// A table with a header row.
pub fn table(headers: &[&str], rows: Vec<El>) -> El {
    let head = headers.iter().fold(el("tr"), |r, h| {
        r.child(el("th").attr("scope", "col").text(*h))
    });
    // the wrapper scrolls, so it is in the tab order: a scrolling region only a pointer can
    // reach is the accessibility defect the UI conformance check refuses (WCAG 2.1.1)
    el("div")
        .class("mj-table-wrap")
        .attr("tabindex", "0")
        .child(
            el("table")
                .class("mj-table")
                .child(el("thead").child(head))
                .child(el("tbody").children(rows)),
        )
}

/// A row of cells.
pub fn row(cells: Vec<El>) -> El {
    el("tr").children(cells)
}

/// A cell.
pub fn cell(content: El) -> El {
    el("td").child(content)
}

/// A cell of plain text.
pub fn text_cell(value: impl Into<String>) -> El {
    el("td").text(value)
}

/// A badge whose modifier is a status word the backend produced. The word is shown as
/// well as coloured, so the state is never carried by colour alone.
pub fn badge(status: &str, label: impl Into<String>) -> El {
    el("span")
        .class(format!("mj-badge mj-badge--{}", css_word(status)))
        .child(el("span").class("mj-badge-dot").attr("aria-hidden", "true"))
        .text(label)
}

/// A neutral badge.
pub fn tag(label: impl Into<String>) -> El {
    el("span").class("mj-tag").text(label)
}

/// Monospaced inline text: an id, a path, a route.
pub fn mono(value: impl Into<String>) -> El {
    el("code").class("mj-mono").text(value)
}

/// A block of text the repository holds, shown as it is. Escaped by the builder; nothing
/// in it is ever interpreted as markup.
pub fn pre(value: impl Into<String>) -> El {
    el("pre").class("mj-pre").child(el("code").text(value))
}

/// An alert: `ok`, `warn`, `fail`, `info`.
pub fn alert(level: &str, message: impl Into<String>) -> El {
    el("div")
        .class(format!("mj-alert mj-alert--{}", css_word(level)))
        .attr("role", if level == "fail" { "alert" } else { "status" })
        .text(message)
}

/// A statistic with its provenance: the number, what it counts, and what produced it.
/// A figure without the third is a vanity metric, so the parameter is not optional.
pub fn statistic(
    value: impl Into<String>,
    label: impl Into<String>,
    source: impl Into<String>,
) -> El {
    el("div")
        .class("mj-stat")
        .child(el("span").class("mj-stat-value").text(value))
        .child(el("span").class("mj-stat-label").text(label))
        .child(el("span").class("mj-stat-source").text(source))
}

/// The word for a capability kind, as a badge.
pub fn kind_badge(kind: CapabilityKind) -> El {
    let (word, label) = match kind {
        CapabilityKind::Query => ("query", "query"),
        CapabilityKind::Command => ("command", "command"),
        CapabilityKind::Resource => ("resource", "resource"),
    };
    badge(word, label)
}

/// A `<details>` block, closed by default.
pub fn details(summary: impl Into<String>, body: El) -> El {
    el("details")
        .class("mj-details")
        .child(el("summary").class("mj-summary").text(summary))
        .child(body)
}

/// An empty state: what was looked for and what to try instead.
pub fn nothing(message: impl Into<String>) -> El {
    el("p").class("mj-empty").text(message)
}

/// A class-safe form of a status word: lowercase ASCII letters, digits and dashes, so a
/// word from data can never build a class name that is not one.
fn css_word(word: &str) -> String {
    let cleaned: String = word
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_word_cannot_build_a_class_it_is_not() {
        assert_eq!(css_word("fail"), "fail");
        assert_eq!(css_word("not ok"), "not-ok");
        assert_eq!(css_word("\" onload=x"), "--onload-x");
        assert_eq!(css_word(""), "unknown");
    }

    #[test]
    fn a_badge_shows_the_word_and_not_only_the_colour() {
        let rendered = badge("fail", "fail").render();
        assert!(rendered.contains("mj-badge--fail"), "{rendered}");
        assert!(rendered.contains(">fail<"), "{rendered}");
    }

    #[test]
    fn repository_content_in_a_block_stays_text() {
        let rendered = pre("<script>alert(1)</script>").render();
        assert!(!rendered.contains("<script>alert"), "{rendered}");
    }
}
