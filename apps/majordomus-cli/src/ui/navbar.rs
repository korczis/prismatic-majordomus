//! The bar at the top of every screen: the site's, not a second one.
//!
//! majordomus.dev is the reference for how the product looks
//! (`project.the-ui-wears-the-sites-navbar`), and its navbar is declared once, in
//! `site/data/nav.toml`, and rendered once, by `site/templates/partials/navbar.html`. The UI
//! does not draw a bar after that one. It wears that rendering: `scripts/ui navbar --write` cuts
//! the bar out of the built site into `navbar.html` beside this file, which is compiled in, and
//! `scripts/ui navbar` refuses a UI whose bar differs from the site's in an element, a box or a
//! computed style, in both themes and at every declared width. The UI's stylesheet compiles the
//! utilities the bar is written in from that same file (`share/ui/src/ui.css`), and the bar's
//! two behaviours are the site's own scripts, served from `share/ui/` as byte copies
//! (`scripts/cockpit-assets`).
//!
//! What the executable decides is what only it knows:
//!
//! - **the version** the badge names: the executable's, where the site names the release it
//!   offers;
//! - **the live surfaces**: an entry the site leads to a published snapshot of a surface this
//!   process serves leads to the surface itself, and stops saying "snapshot".
//!
//! Every other link leads to the site, at its own address, because it is the site's page.

/// The site's bar, as the site rendered it, with the version left to fill in.
pub const FRAGMENT: &str = include_str!("navbar.html");

/// What the fragment holds where the site's build named the release it offers.
pub const VERSION_SLOT: &str = "@MJ_VERSION@";

/// The attributes the site's bar marks an entry with when it leads to a surface's snapshot:
/// the bar's own entry for the product, and an entry of a group's menu.
const SURFACE_MARKS: [&str; 2] = ["data-nav-primary", "data-nav-snapshot"];

/// The attribute of the note that says an entry leads to a snapshot.
const SNAPSHOT_NOTE: &str = "data-nav-snapshot-note";

/// The bar for a page of this process: `version` in the badge, and every entry the site leads
/// to the snapshot of a surface in `live` (its id and the mount this process serves it at)
/// leading to the mount instead.
pub fn render(version: &str, live: &[(&str, &str)]) -> String {
    relink(&FRAGMENT.replace(VERSION_SLOT, version), live)
}

/// Every `<a>` that names a surface of `live` by one of [`SURFACE_MARKS`] leads to its mount,
/// without the snapshot note.
fn relink(bar: &str, live: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(bar.len());
    let mut rest = bar;
    while let Some(at) = rest.find("<a ") {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(close) = rest.find('>') else { break };
        let Some(end) = rest.find("</a>") else { break };
        let (tag, inner) = (&rest[..=close], &rest[close + 1..end]);
        match live.iter().find(|(id, _)| {
            SURFACE_MARKS
                .iter()
                .any(|m| tag.contains(&format!("{m}=\"{id}\"")))
        }) {
            Some((_, mount)) => {
                out.push_str(&with_href(tag, mount));
                out.push_str(&without_note(inner));
            }
            None => out.push_str(&rest[..end]),
        }
        out.push_str("</a>");
        rest = &rest[end + "</a>".len()..];
    }
    out.push_str(rest);
    out
}

/// The tag with its `href` replaced.
fn with_href(tag: &str, href: &str) -> String {
    match tag.find("href=\"") {
        Some(at) => {
            let from = at + "href=\"".len();
            let to = tag[from..].find('"').map_or(tag.len(), |n| from + n);
            format!("{}{}{}", &tag[..from], href, &tag[to..])
        }
        None => tag.to_string(),
    }
}

/// An entry's content without the note that says it is a snapshot, and the space before it.
fn without_note(inner: &str) -> String {
    match inner.find(&format!("<span {SNAPSHOT_NOTE}")) {
        Some(at) => {
            let end = inner[at..]
                .find("</span>")
                .map_or(inner.len(), |n| at + n + "</span>".len());
            format!("{}{}", inner[..at].trim_end(), &inner[end..])
        }
        None => inner.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compiled bar is the site's: its landmark, its brand, its four questions and its call
    /// to action, and a slot for the version and nothing of the build that cut it.
    #[test]
    fn the_compiled_bar_is_the_sites() {
        assert!(FRAGMENT.contains("aria-label=\"Main\""), "{FRAGMENT}");
        assert!(FRAGMENT.contains("id=\"navbar-main\""));
        assert!(FRAGMENT.contains("id=\"theme-toggle\""));
        assert!(FRAGMENT.contains("data-site-search-open"));
        assert_eq!(FRAGMENT.matches("<nav ").count(), 1);
        assert!(
            FRAGMENT.contains(VERSION_SLOT),
            "the badge names a version to fill in"
        );
        assert!(
            !FRAGMENT.contains("href=\"#"),
            "a link to the site map of a site page leads to the site"
        );
        assert!(!FRAGMENT.contains("<script"), "the bar carries no script");
    }

    /// The version is the executable's, and an entry that leads to a surface this process
    /// serves leads to it and no longer says "snapshot"; every other entry is left alone.
    #[test]
    fn a_live_surface_is_led_to_and_the_rest_is_the_sites() {
        let bar = "<span data-release=\"@MJ_VERSION@\">v@MJ_VERSION@</span>\
            <a href=\"https://site/ui/\" data-nav-primary=\"ui\" class=\"x\">Product UI</a>\
            <a href=\"https://site/ui/\" data-nav-snapshot=\"ui\" class=\"y\">Product UI \
            <span data-nav-snapshot-note class=\"z\">&middot; snapshot</span></a>\
            <a href=\"https://site/docs/\">Docs</a>";
        let out = relink(&bar.replace(VERSION_SLOT, "1.2.3"), &[("ui", "/ui")]);
        assert!(out.contains(">v1.2.3<"), "{out}");
        assert!(out.contains("<a href=\"/ui\" data-nav-primary=\"ui\" class=\"x\">Product UI</a>"));
        assert!(
            out.contains("<a href=\"/ui\" data-nav-snapshot=\"ui\" class=\"y\">Product UI</a>"),
            "{out}"
        );
        assert!(!out.contains("snapshot</span>"), "{out}");
        assert!(out.contains("<a href=\"https://site/docs/\">Docs</a>"));
        assert_eq!(relink(bar, &[]), bar, "nothing live, nothing moved");
    }

    /// The bar the UI draws keeps every link of the compiled one but those it leads home.
    #[test]
    fn the_rendered_bar_keeps_every_other_link() {
        let out = render("9.9.9", &[(super::super::SURFACE, super::super::PREFIX)]);
        assert!(!out.contains(VERSION_SLOT));
        let links = |s: &str| s.matches("<a ").count();
        assert_eq!(links(&out), links(FRAGMENT));
    }

    /// A bar whose entry is cut short is passed on as it came, and a live entry with no
    /// `href` keeps its tag while losing its snapshot note.
    #[test]
    fn a_cut_entry_is_left_as_it_came_and_one_without_a_link_keeps_its_tag() {
        let live = [("ui", "/ui")];
        let unclosed = "<b>bar</b><a data-nav-primary=\"ui\"";
        assert_eq!(relink(unclosed, &live), unclosed, "no end to the tag");
        let unended = "<b>bar</b><a data-nav-primary=\"ui\">Product UI";
        assert_eq!(relink(unended, &live), unended, "no end to the entry");
        let bare =
            "<a data-nav-snapshot=\"ui\">UI <span data-nav-snapshot-note>snapshot</span></a>";
        assert_eq!(
            relink(bare, &live),
            "<a data-nav-snapshot=\"ui\">UI</a>",
            "no href to move"
        );
    }
}
