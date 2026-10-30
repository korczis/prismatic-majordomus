//! Prose a record carries in a field, drawn as prose: the sentences of a milestone's problem
//! and outcome, an issue's objective, a list of acceptance criteria.
//!
//! The plan's records are written by people, in the Markdown people write without thinking,
//! and they name other records by their ids. Drawn as one escaped string, the markup is noise
//! and every id is a dead end. Here a field is drawn by the one renderer every Markdown body
//! goes through ([`crate::markdown`]), and every id of a record the plan actually holds is a
//! link to it — so a reader walks from a milestone's problem to the issue it names by clicking
//! the name.
//!
//! Nothing is parsed here. This module only says which words are records.

use std::collections::BTreeSet;

use crate::cockpit::html::{el, El, Node};
use crate::markdown::{self, Options};

/// The records prose may name: an id is a link only when the plan holds the record.
#[derive(Debug, Default, Clone)]
pub struct Known {
    /// Every issue id of the plan.
    pub issues: BTreeSet<String>,
    /// Every milestone id of the plan.
    pub milestones: BTreeSet<String>,
}

impl Known {
    /// Where a word leads, when it names a record the plan holds.
    fn href(&self, word: &str) -> Option<String> {
        if self.issues.contains(word) {
            Some(format!("{}?issue={word}#issue", super::prs::ROUTE))
        } else if self.milestones.contains(word) {
            Some(super::milestone::href(word))
        } else {
            None
        }
    }
}

/// One line of prose as nodes: its Markdown drawn, every known record id a link.
pub fn inline(line: &str, known: &Known) -> Vec<Node> {
    markdown::inline(
        line.trim(),
        &Options {
            word: &|word| known.href(word),
            ..Options::plain()
        },
    )
}

/// A paragraph of prose.
pub fn paragraph(prose: &str, known: &Known) -> El {
    el("p").class("mj-ui-prose").nodes(inline(prose, known))
}

/// A list of prose lines, each drawn as [`inline`].
pub fn bullets(lines: &[String], known: &Known) -> El {
    el("ul")
        .class("mj-ui-prose-list")
        .children(lines.iter().map(|line| el("li").nodes(inline(line, known))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Known {
        Known {
            issues: ["I2400".to_string()].into(),
            milestones: ["M004".to_string(), "development-runtime".to_string()].into(),
        }
    }

    fn drawn(line: &str) -> String {
        el("p").nodes(inline(line, &known())).render()
    }

    #[test]
    fn markup_is_drawn_and_a_known_record_is_a_link_to_it() {
        let html = drawn("Run `majordomus bench` for **every** command, see I2400 and M004.");
        assert!(
            html.contains("<code class=\"mj-md-code\">majordomus bench</code>"),
            "{html}"
        );
        assert!(html.contains("<strong>every</strong>"), "{html}");
        assert!(html.contains("?issue=I2400#issue\">I2400</a>"), "{html}");
        assert!(
            html.contains("?milestone=M004#milestone\">M004</a>"),
            "{html}"
        );
        assert!(html.ends_with(".</p>"), "{html}");
    }

    #[test]
    fn an_id_the_plan_does_not_hold_is_text_and_a_longer_word_is_not_an_id() {
        let html = drawn("I9999 is unknown, and XI2400 and I24000 are other words.");
        assert!(!html.contains("<a"), "{html}");
        // an id made of words is matched whole
        assert!(drawn("part of development-runtime now").contains(">development-runtime</a>"));
    }

    #[test]
    fn a_web_link_is_followed_and_any_other_address_is_only_named() {
        let web = drawn("see [the docs](https://example.org/a?b=1)");
        assert!(web.contains("href=\"https://example.org/a?b=1\""), "{web}");
        assert!(web.contains("rel=\"noopener noreferrer\""), "{web}");
        let hostile = drawn("click [here](javascript:alert(1)) or [there](/etc/passwd)");
        assert!(!hostile.contains("href"), "{hostile}");
        assert!(hostile.contains(">here</span>"), "{hostile}");
    }

    #[test]
    fn whatever_a_record_holds_is_escaped() {
        let html = drawn("a <script>alert(1)</script> and `<b>`");
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(
            html.contains("<code class=\"mj-md-code\">&lt;b&gt;</code>"),
            "{html}"
        );
    }

    #[test]
    fn a_paragraph_and_a_list_draw_every_line() {
        let k = known();
        assert_eq!(
            paragraph("  one `two`  ", &k).render(),
            "<p class=\"mj-ui-prose\">one <code class=\"mj-md-code\">two</code></p>"
        );
        let list = bullets(&["first I2400".into(), "second".into()], &k).render();
        assert!(
            list.starts_with("<ul class=\"mj-ui-prose-list\"><li>first <a"),
            "{list}"
        );
        assert!(list.ends_with("<li>second</li></ul>"), "{list}");
    }
}
