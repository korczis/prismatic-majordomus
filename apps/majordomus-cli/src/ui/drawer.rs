//! The drawer: the one narrow-screen menu shell of `/ui` and `/cockpit` (I3214).
//!
//! Each surface renders the same four parts and fills the panel with its own entries: the
//! checkbox that holds whether the drawer is open ([`toggle`], first in the body, so that the
//! stylesheet can read it from anywhere after it), the button that is its label ([`button`], in
//! the surface's tool bar), the panel (the surface's own sidebar, given [`PANEL`]) and the
//! backdrop ([`scrim`]), a second label of the same checkbox. It opens and closes with no
//! script; `share/design/drawer.js` closes it on Escape, on the backdrop and on a choice and
//! gives the focus back, and `share/design/drawer.css` decides which toggle shows at which
//! width, so that only one ever does (below md that is the site bar's own menu).

use crate::cockpit::html::{el, El};

/// The id of the checkbox, which the button, the backdrop and `drawer.js` name.
pub const ID: &str = "mj-drawer";

/// The class that makes a surface's sidebar the drawer's panel below lg.
pub const PANEL: &str = "mj-drawer";

/// The checkbox that holds the drawer's state: visually hidden, still focusable, so a keyboard
/// reader can open the drawer with no script.
pub fn toggle() -> El {
    el("input")
        .class("mj-drawer-toggle")
        .attr("type", "checkbox")
        .attr("id", ID)
        .attr("aria-label", "Show the menu")
}

/// The button that opens and closes the drawer: a label of the checkbox, with three lines
/// drawn and the word read.
pub fn button() -> El {
    el("label")
        .class("mj-drawer-button")
        .attr("for", ID)
        .child(
            el("svg")
                .class("mj-drawer-icon")
                .attr("viewBox", "0 0 24 24")
                .attr("fill", "none")
                .attr("stroke", "currentColor")
                .attr("stroke-width", "1.75")
                .attr("stroke-linecap", "round")
                .attr("aria-hidden", "true")
                .child(el("path").attr("d", "M4 6h16M4 12h16M4 18h16")),
        )
        .child(el("span").class("mj-drawer-label").text("Menu"))
}

/// The backdrop behind the open drawer: a click on it closes the drawer, with or without a
/// script. It is not a control of its own to a screen reader, which closes the drawer from the
/// button or with Escape.
pub fn scrim() -> El {
    el("label")
        .class("mj-drawer-scrim")
        .attr("for", ID)
        .attr("aria-hidden", "true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_button_and_the_backdrop_are_labels_of_the_one_checkbox() {
        let toggle = toggle().render();
        assert!(toggle.contains("type=\"checkbox\""), "{toggle}");
        assert!(toggle.contains(&format!("id=\"{ID}\"")), "{toggle}");
        let button = button().render();
        assert!(button.starts_with(&format!("<label class=\"mj-drawer-button\" for=\"{ID}\">")));
        assert!(
            button.contains(">Menu</span>"),
            "the button is named: {button}"
        );
        let scrim = scrim().render();
        assert!(scrim.contains(&format!("for=\"{ID}\"")), "{scrim}");
        assert!(scrim.contains("aria-hidden=\"true\""), "{scrim}");
    }
}
