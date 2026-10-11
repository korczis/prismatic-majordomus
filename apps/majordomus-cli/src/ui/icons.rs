//! The UI's icons: the kit's set, declared once in `site/data/kit/icons.toml` beside the
//! site's templates and compiled into the crate by `majordomus generate`, so the site and
//! the UI draw one icon set. Paths follow Lucide (ISC licence), drawn in `currentColor`.
//!
//! An icon carries no meaning of its own: the word beside it always says what it is, and a
//! name the set does not hold draws nothing rather than a guess.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::cockpit::html::{el, El};

/// The one declaration of the icons, repository-relative.
pub const SOURCE: &str = "site/data/kit/icons.toml";

/// Its compiled copy, written by `majordomus generate`.
pub const COMPILED: &str = "apps/majordomus-cli/src/ui/icons.toml";

static ICONS: LazyLock<BTreeMap<String, String>> =
    LazyLock::new(|| parse(include_str!("icons.toml")));

/// The `name = '<markup>'` entries of the `[icons]` table: the only shape the file uses.
fn parse(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == "[icons]";
            continue;
        }
        if !inside || t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = t.split_once(" = ") {
            let markup = value.trim().trim_matches('\'');
            out.insert(name.trim().to_string(), markup.to_string());
        }
    }
    out
}

/// Whether the set holds `name`.
pub fn has(name: &str) -> bool {
    ICONS.contains_key(name)
}

/// One icon as an inline SVG, hidden from assistive technology because the word beside it
/// is what is read.
pub fn icon(name: &str) -> El {
    el("svg")
        .class("mj-ui-icon")
        .attr("viewBox", "0 0 24 24")
        .attr("fill", "none")
        .attr("stroke", "currentColor")
        .attr("stroke-width", "1.75")
        .attr("stroke-linecap", "round")
        .attr("stroke-linejoin", "round")
        .attr("aria-hidden", "true")
        .attr("focusable", "false")
        .raw(ICONS.get(name).cloned().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The compiled copy is the declaration's: every name the declaration holds is here
    /// with the same markup, so the site and the UI draw one set.
    #[test]
    fn the_compiled_set_is_the_declared_set() {
        let declared = parse(include_str!("../../../../site/data/kit/icons.toml"));
        assert!(!declared.is_empty(), "the declaration holds no icon");
        assert_eq!(*ICONS, declared);
    }

    /// Every icon the UI's shell names exists in the set.
    #[test]
    fn every_icon_the_shell_names_exists() {
        for s in crate::ui::SCREENS {
            assert!(
                has(s.icon),
                "{} names icon {} the set does not hold",
                s.route,
                s.icon
            );
        }
    }

    /// A name the set does not hold draws an empty glyph, never another icon.
    #[test]
    fn an_unknown_name_draws_nothing() {
        let svg = icon("no-such-icon").render();
        assert!(svg.starts_with("<svg") && svg.ends_with("></svg>"), "{svg}");
    }
}
