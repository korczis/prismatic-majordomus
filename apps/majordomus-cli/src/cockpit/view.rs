//! The shell every Cockpit page is rendered into, and the small vocabulary of components
//! the pages are built from.
//!
//! The pages carry no raw utility classes. Every class here is a semantic one — the shell
//! around a page and the components inside it in `share/design/shell.css` and
//! `share/design/primitives.css`, which the published site renders with too, and what the
//! Cockpit alone has in `share/cockpit/src/cockpit.css` — so the design lives in files a
//! designer can read and the Rust says what a thing *is* rather than how it looks. That is
//! also what makes the stylesheet deterministic: it is a function of its own sources and
//! nothing else, and the drift check over it is exact.
//!
//! Every `x-` attribute here names a *top-level* property or method of the one Alpine
//! component and never an expression — not even a dotted path, which the CSP build reads
//! as an expression and does not evaluate. A palette bound to `palette.open` looks right,
//! renders right, and leaves a full-page modal backdrop over every click. That is what the content-security policy requires: the Cockpit
//! ships Alpine's CSP build, which evaluates no strings, so the policy needs neither
//! `unsafe-eval` nor `unsafe-inline`. A handler that would take an argument gets its own
//! method in `cockpit.js` instead.

use std::sync::LazyLock;

use crate::capability::CapabilityKind;
use crate::design::DesignSystem;

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
        el("script").raw(theme_bootstrap()),
    ]
    .into_iter()
    .chain(shell.scripts.iter().map(|src| {
        el("script")
            .attr("type", "module")
            .attr("src", src.clone())
            .flag("defer")
    }))
    .collect();

    // the theme contract and the design fingerprint, from the declaration compiled into
    // this executable: the script reads the key and the class here rather than naming
    // them, and compares the fingerprint with the one the stylesheet carries
    let design = DesignSystem::compiled().ok();
    let body = el("body")
        .class("mj-body")
        .attr("x-data", "cockpit")
        .attr_if(
            "data-theme-key",
            design.map(|d| d.theme.storage_key.clone()),
        )
        .attr_if("data-theme-class", design.map(|d| d.theme.class.clone()))
        .attr_if("data-design", design.map(|d| d.short_fingerprint()))
        .child(skip_link())
        .child(header(shell))
        .child(
            el("div").class("mj-layout").child(sidebar(shell)).child(
                el("main")
                    .attr("id", "main")
                    .class("mj-main")
                    .when(!shell.assets_present, |m| m.child(alert("warn", NO_ASSETS)))
                    .child(
                        el("header")
                            .class("mj-page-head")
                            .child(breadcrumbs(&shell.breadcrumbs))
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

/// Read the stored theme before the page paints: the one statement the design declaration
/// generates for every surface, so the site and the Cockpit honour one storage key and one
/// class. One statement, so that its digest is stable and the content-security policy can
/// name it. An executable whose compiled declaration is invalid ships no statement and
/// follows the system through the stylesheet alone; the design capability says why.
pub fn theme_bootstrap() -> &'static str {
    static BOOTSTRAP: LazyLock<String> = LazyLock::new(|| {
        DesignSystem::compiled()
            .map(|d| d.theme_bootstrap())
            .unwrap_or_default()
    });
    BOOTSTRAP.as_str()
}

/// The canonical mark, compiled in from its generated copy of `share/design/brand/`, so
/// the brand in the top bar is the same file the favicon, the site and the social card
/// are cut from. It draws in `currentColor`.
const MARK: &str = include_str!("logo-mark.svg");

fn skip_link() -> El {
    el("a")
        .class("mj-skip")
        .attr("href", "#main")
        .text("Skip to content")
}

/// The top bar: the same band, the same brand lockup and the same controls the site's
/// navbar renders, from share/design/shell.css. What is the Cockpit's own is what sits
/// between them — the button that opens the command palette.
fn header(shell: &Shell<'_>) -> El {
    el("header").class("mj-topbar").child(
        el("div")
            .class("mj-topbar-inner")
            .child(
                el("a")
                    .class("mj-brand")
                    .attr("href", "/cockpit")
                    .child(
                        el("span")
                            .class("mj-brand-mark")
                            .attr("aria-hidden", "true")
                            .raw(MARK),
                    )
                    .child(el("span").class("mj-brand-name").text("Majordomus"))
                    .child(el("span").class("mj-brand-suffix").text(shell.version)),
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
                    .class("mj-topbar-actions")
                    .attr("aria-label", "External surfaces")
                    .child(
                        el("button")
                            .class("mj-theme-toggle")
                            .attr("type", "button")
                            .attr("x-on:click", "toggleTheme")
                            .attr("aria-label", "Switch between light and dark")
                            .attr("title", "Switch between light and dark")
                            .text("Theme"),
                    )
                    .child(
                        el("a")
                            .class("mj-action")
                            .attr("href", crate::http::swagger::SWAGGER_PATH)
                            .text("Swagger"),
                    )
                    .child(
                        el("a")
                            .class("mj-action")
                            .attr("href", "/openapi.json")
                            .text("OpenAPI"),
                    ),
            ),
    )
}

fn sidebar(shell: &Shell<'_>) -> El {
    let mut nav = el("nav")
        .class("mj-sidebar")
        .attr("aria-label", "Cockpit sections");
    for section in shell.navigation.sections() {
        let mut list = el("ul").class("mj-nav-list");
        // A grouped section shows its headings as the group changes. The items arrive in
        // the canonical order, which puts a group's members together and the ungrouped
        // ones last, so one pass is enough and nothing here decides the sequence.
        let mut group: Option<&str> = None;
        for item in &section.items {
            if item.group.as_deref() != group {
                group = item.group.as_deref();
                if let Some(heading) = group {
                    list = list.child(
                        el("li")
                            .class("mj-nav-group")
                            .attr("role", "presentation")
                            .text(heading),
                    );
                }
            }
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
        // a catalogue long enough to bury the sections under it folds away, and unfolds
        // itself when what the reader is looking at is inside it
        let holds_current = section
            .items
            .iter()
            .any(|i| i.area == shell.area && i.current);
        nav = nav.child(if section.items.len() > FOLD_ABOVE {
            el("details")
                .class("mj-nav-section mj-nav-section--foldable")
                .when(holds_current, |d| d.flag("open"))
                .child(
                    el("summary")
                        .class("mj-nav-heading mj-nav-summary")
                        .child(el("span").text(&section.title))
                        .child(
                            el("span")
                                .class("mj-nav-count")
                                .text(section.items.len().to_string()),
                        ),
                )
                .child(list)
        } else {
            el("div")
                .class("mj-nav-section")
                .child(el("h2").class("mj-nav-heading").text(&section.title))
                .child(list)
        });
    }
    nav
}

/// How many entries a sidebar catalogue may have before it is folded. Nine areas and a
/// dozen modules are a menu; thirty object kinds under them are a wall.
const FOLD_ABOVE: usize = 12;

/// The footer: the site's band, inner measure and link row, from share/design/shell.css.
fn footer(shell: &Shell<'_>) -> El {
    el("footer").class("mj-footer").child(
        el("div")
            .class("mj-footer-inner")
            .child(
                el("ul")
                    .class("mj-footer-links")
                    .child(el("li").child(link("/openapi.json", "openapi.json")))
                    .child(el("li").child(link(crate::http::swagger::SWAGGER_PATH, "Swagger UI")))
                    .child(el("li").child(link("/cockpit/health", "health"))),
            )
            .child(
                el("p")
                    .class("mj-footer-note")
                    .text(format!("majordomus {}", shell.version)),
            ),
    )
}

/// The trail, composed as the site composes it: the separator is markup — one
/// `<li aria-hidden="true">/</li>` between the crumbs — rather than a `::before`, so a
/// screen reader is never read a slash and both surfaces emit the same trail.
fn breadcrumbs(trail: &[(String, Option<String>)]) -> El {
    if trail.is_empty() {
        return el("div").class("mj-hidden");
    }
    let mut list = el("ol").class("mj-crumbs");
    for (index, (label, href)) in trail.iter().enumerate() {
        if index > 0 {
            list = list.child(el("li").attr("aria-hidden", "true").text("/"));
        }
        list = list.child(match href {
            Some(h) => el("li").class("mj-crumb").child(
                el("a")
                    .class("mj-crumb-link")
                    .attr("href", h.clone())
                    .text(label),
            ),
            None => el("li")
                .class("mj-crumb-current")
                .attr("aria-current", "page")
                .text(label),
        });
    }
    el("nav")
        .class("mj-crumbs-nav")
        .attr("aria-label", "Breadcrumb")
        .child(list)
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

/// A badge for a status word that arrived as *data* — an object's own `status` field, a
/// graph node's — rather than one this code chose. Filed under a status by the design
/// declaration, it is a badge and carries that status's colour; filed under nothing, it is
/// a plain tag, so that an arbitrary word never wears a meaning nobody gave it and the
/// browser probe's vocabulary check stays exact. The words this code chooses itself go
/// through [`badge`], where an unfiled word is a defect the probe reports.
pub fn word_badge(word: &str) -> El {
    let filed = DesignSystem::compiled()
        .ok()
        .is_some_and(|d| d.role_of_state(&css_word(word)).is_some());
    if filed {
        badge(word, word)
    } else {
        tag(word)
    }
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
///
/// `tabindex="0"` because `.mj-pre` scrolls inside itself: a box a mouse can scroll and a
/// keyboard cannot reach fails WCAG 2.1.1, and the audit found exactly that on every object
/// page that shows a record. The site solves the same problem in its build, because it does
/// not write the markup its highlighter emits; here the markup is ours, so it is solved at
/// its source.
pub fn pre(value: impl Into<String>) -> El {
    el("pre")
        .class("mj-pre")
        .attr("tabindex", "0")
        .child(el("code").text(value))
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
    badge(kind.as_str(), kind.as_str())
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

// ------------------------------------------------------------------- listings

/// How many rows a listing shows at once before it is paged. A control plane is read a
/// screenful at a time; a page that renders every one of nine hundred rows is a file
/// dump with a header on it.
pub const PER_PAGE: usize = 50;

/// The window a listing is showing of its result set. The page number is a view
/// parameter and lives in the URL, so a window is a deep link like every other state
/// the Cockpit holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The page being shown, 1-based and never past the last one.
    pub page: usize,
    /// How many rows a page holds.
    pub per_page: usize,
    /// How many rows matched, before the window was taken.
    pub total: usize,
}

impl Window {
    /// The window a request asks for, clamped to what the result set has: page 0 and page
    /// 900 of a 3-page listing are both a page that exists.
    pub fn new(page: usize, per_page: usize, total: usize) -> Self {
        let per_page = per_page.max(1);
        let pages = total.div_ceil(per_page).max(1);
        Self {
            page: page.clamp(1, pages),
            per_page,
            total,
        }
    }

    /// How many pages the result set has; never zero, because an empty listing still has
    /// a page to show its empty state on.
    pub fn pages(&self) -> usize {
        self.total.div_ceil(self.per_page).max(1)
    }

    /// The half-open range of rows this window covers, for slicing the matches.
    pub fn range(&self) -> std::ops::Range<usize> {
        let start = (self.page - 1) * self.per_page;
        start..(start + self.per_page).min(self.total)
    }
}

/// The page numbers a pagination control offers: the first, the last, the neighbours of
/// the current one, and a gap marker for what is skipped. Nine hundred pages must not
/// render nine hundred links.
fn page_numbers(current: usize, pages: usize) -> Vec<Option<usize>> {
    let mut wanted: Vec<usize> = [1, pages]
        .into_iter()
        .chain(current.saturating_sub(1).max(1)..=(current + 1).min(pages))
        .filter(|n| *n >= 1 && *n <= pages)
        .collect();
    wanted.sort_unstable();
    wanted.dedup();

    let mut out = Vec::with_capacity(wanted.len() + 2);
    let mut previous = 0usize;
    for n in wanted {
        if previous != 0 && n > previous + 1 {
            out.push(None);
        }
        out.push(Some(n));
        previous = n;
    }
    out
}

/// Pagination over a listing: what is being shown, and the way to the rest of it.
///
/// `href` builds the URL of one page — the caller owns the listing's filters and puts
/// them back into every link, so paging never drops a filter and a link can be sent to
/// somebody as it is.
pub fn pagination(w: Window, href: impl Fn(usize) -> String) -> El {
    if w.total == 0 {
        return el("div").class("mj-hidden");
    }
    let range = w.range();
    let summary = el("p").class("mj-pagination-summary").text(format!(
        "{}–{} of {}",
        range.start + 1,
        range.end,
        w.total
    ));
    if w.pages() == 1 {
        return el("nav").class("mj-pagination").child(summary);
    }

    let step = |label: &str, to: Option<usize>| {
        let li = el("li");
        match to {
            Some(n) => li.child(
                el("a")
                    .class("mj-page-link")
                    .attr("href", href(n))
                    .attr("rel", if n < w.page { "prev" } else { "next" })
                    .text(label),
            ),
            None => li.child(
                el("span")
                    .class("mj-page-link mj-page-link--disabled")
                    .attr("aria-disabled", "true")
                    .text(label),
            ),
        }
    };

    let mut list = el("ul")
        .class("mj-pagination-list")
        .child(step("Previous", (w.page > 1).then(|| w.page - 1)));
    for slot in page_numbers(w.page, w.pages()) {
        list = list.child(match slot {
            Some(n) if n == w.page => el("li").child(
                el("span")
                    .class("mj-page-link mj-page-link--current")
                    .attr("aria-current", "page")
                    .text(n.to_string()),
            ),
            Some(n) => el("li").child(
                el("a")
                    .class("mj-page-link")
                    .attr("href", href(n))
                    .attr("aria-label", format!("Page {n}"))
                    .text(n.to_string()),
            ),
            None => el("li").child(
                el("span")
                    .class("mj-page-gap")
                    .attr("aria-hidden", "true")
                    .text("…"),
            ),
        });
    }
    list = list.child(step("Next", (w.page < w.pages()).then(|| w.page + 1)));

    el("nav")
        .class("mj-pagination")
        .attr("aria-label", "Pagination")
        .child(summary)
        .child(list)
}

/// A row of filters a reader browses by: a label, where it goes, how many are behind it,
/// and whether it is the one in force. It is the same catalogue the sidebar shows, put
/// where the listing is, so a set of nine hundred rows is entered by its parts rather
/// than scrolled.
pub fn chips(items: Vec<(String, String, usize, bool)>) -> El {
    if items.is_empty() {
        return el("div").class("mj-hidden");
    }
    let mut list = el("ul").class("mj-chips");
    for (label, href, count, current) in items {
        list = list.child(
            el("li").child(
                el("a")
                    .class(if current {
                        "mj-chip mj-chip--current"
                    } else {
                        "mj-chip"
                    })
                    .attr("href", href)
                    .attr_if("aria-current", current.then_some("true"))
                    .child(el("span").class("mj-chip-label").text(label))
                    .child(el("span").class("mj-chip-count").text(count.to_string())),
            ),
        );
    }
    list
}

/// A cell holding an identifier: monospaced, on one line, and truncated with the whole
/// of it in the tooltip. An identifier broken across three lines mid-word is what makes
/// a table of them unreadable.
pub fn id_cell(href: impl Into<String>, id: impl Into<String>) -> El {
    let id = id.into();
    el("td").class("mj-cell-id").child(
        el("a")
            .class("mj-link mj-mono mj-id")
            .attr("href", href)
            .attr("title", id.clone())
            .text(id),
    )
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
    fn a_window_is_clamped_to_the_pages_that_exist() {
        // page 0 and page 900 of a three-page listing are both a page there is
        assert_eq!(Window::new(0, 50, 120).page, 1);
        assert_eq!(Window::new(900, 50, 120).page, 3);
        assert_eq!(Window::new(2, 50, 120).range(), 50..100);
        // the last page is short, and its range stops at what there is
        assert_eq!(Window::new(3, 50, 120).range(), 100..120);
        // an empty listing still has a page, and slicing it takes nothing
        let empty = Window::new(1, 50, 0);
        assert_eq!(empty.pages(), 1);
        assert_eq!(empty.range(), 0..0);
    }

    #[test]
    fn a_thousand_pages_do_not_render_a_thousand_links() {
        // the first, the last, the neighbours, and a gap for what is skipped
        assert_eq!(
            page_numbers(7, 19),
            vec![Some(1), None, Some(6), Some(7), Some(8), None, Some(19)]
        );
        // no gap marker where nothing is skipped
        assert_eq!(page_numbers(2, 3), vec![Some(1), Some(2), Some(3)]);
        assert_eq!(page_numbers(1, 1), vec![Some(1)]);
    }

    #[test]
    fn a_listing_of_one_page_offers_no_page_links() {
        let rendered = pagination(Window::new(1, 50, 12), |n| format!("?page={n}")).render();
        assert!(rendered.contains("1–12 of 12"), "{rendered}");
        assert!(!rendered.contains("mj-page-link"), "{rendered}");
    }

    #[test]
    fn repository_content_in_a_block_stays_text() {
        let rendered = pre("<script>alert(1)</script>").render();
        assert!(!rendered.contains("<script>alert"), "{rendered}");
    }
}
