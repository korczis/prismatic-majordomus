//! Prose a record carries, drawn as prose: the sentences of a milestone's problem and
//! outcome, an issue's objective, a list of acceptance criteria.
//!
//! The plan's records are written by people, in the light markup people write without
//! thinking — a `code span`, a **strong** word, a [link](https://example.org), a list — and
//! they name other records by their ids. Drawn as one escaped string, all of that is noise
//! and every id is a dead end. Here the markup is drawn as markup and every id of a record
//! the plan actually holds is a link to it, so a reader walks from a milestone's problem to
//! the issue it names by clicking the name.
//!
//! This is not a Markdown implementation and does not grow into one: it draws inline markup
//! and flat lists, which is everything a YAML string of the plan holds. Nothing it is given
//! is ever written as markup — every byte of a record goes through the builder's escaping —
//! and a link is drawn only to an `http`/`https` address or to a record known to exist.

use std::collections::BTreeSet;

use crate::cockpit::html::{el, text, El, Node};

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

/// A link target a record may carry: an absolute web address and nothing else. A record is
/// text somebody typed, and `javascript:` is text somebody can type.
fn is_web(href: &str) -> bool {
    href.starts_with("https://") || href.starts_with("http://")
}

/// Whether a character can be part of a record's id (`I2400`, `M004`, `development-runtime`).
fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Plain text with every word that names a known record drawn as a link to it.
fn linked(run: &str, known: &Known, out: &mut Vec<Node>) {
    let mut plain = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, plain: &mut String, out: &mut Vec<Node>| match known.href(word)
    {
        Some(href) => {
            if !plain.is_empty() {
                out.push(text(std::mem::take(plain)));
            }
            out.push(Node::Element(
                el("a")
                    .class("mj-ui-link mj-ui-mono")
                    .attr("href", href)
                    .text(std::mem::take(word)),
            ));
        }
        None => plain.push_str(&std::mem::take(word)),
    };
    for c in run.chars() {
        if is_id_char(c) {
            word.push(c);
        } else {
            flush(&mut word, &mut plain, out);
            plain.push(c);
        }
    }
    flush(&mut word, &mut plain, out);
    if !plain.is_empty() {
        out.push(text(plain));
    }
}

/// The first `open … close` span in `s`: what precedes it, what it holds, and what follows.
fn span<'a>(s: &'a str, open: &str, close: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let start = s.find(open)?;
    let inner_from = start + open.len();
    let end = s[inner_from..].find(close)? + inner_from;
    (end > inner_from).then(|| (&s[..start], &s[inner_from..end], &s[end + close.len()..]))
}

/// A `[label](address)` at the start of what follows `before`.
fn link(s: &str) -> Option<(&str, &str, &str, &str)> {
    let (before, label, rest) = span(s, "[", "]")?;
    let rest = rest.strip_prefix('(')?;
    let (address, after) = rest.split_once(')')?;
    Some((before, label, address, after))
}

/// One line of prose as nodes: code spans, strong words and links drawn as what they are,
/// every known record id a link, everything else text.
pub fn inline(line: &str, known: &Known) -> Vec<Node> {
    let mut out = Vec::new();
    let mut rest = line;
    loop {
        let code = span(rest, "`", "`").map(|(b, i, a)| (b.len(), 0, b, i, "", a));
        let strong = span(rest, "**", "**").map(|(b, i, a)| (b.len(), 1, b, i, "", a));
        let anchor = link(rest).map(|(b, l, h, a)| (b.len(), 2, b, l, h, a));
        // the markup that opens first is the one this stretch of the line is
        let Some((_, kind, before, inner, address, after)) =
            [code, strong, anchor].into_iter().flatten().min()
        else {
            linked(rest, known, &mut out);
            return out;
        };
        linked(before, known, &mut out);
        out.push(match kind {
            0 => Node::Element(el("code").class("mj-ui-code").text(inner)),
            1 => Node::Element(el("strong").nodes(inline(inner, known))),
            _ if is_web(address) => Node::Element(
                el("a")
                    .class("mj-ui-link")
                    .attr("href", address)
                    .attr("rel", "noopener noreferrer")
                    .nodes(inline(inner, known)),
            ),
            // an address this does not follow: the label is read, the address is shown
            _ => text(format!("{inner} ({address})")),
        });
        rest = after;
    }
}

/// One line of text that may carry a `code span` or a **strong** word, drawn with both and
/// with no link at all: what a summary, a title or a worker's intent says when it sits in a
/// table cell, or inside a link of its own, where an anchor in it would be an anchor inside
/// an anchor. A `[label](address)` stays the text it is. The line is never markup: every
/// byte of it goes through the builder's escaping, as in [`inline`].
pub fn marks(line: &str) -> Vec<Node> {
    let mut out = Vec::new();
    let mut rest = line;
    loop {
        let code = span(rest, "`", "`").map(|(b, i, a)| (b.len(), 0, b, i, a));
        let strong = span(rest, "**", "**").map(|(b, i, a)| (b.len(), 1, b, i, a));
        let Some((_, kind, before, inner, after)) = [code, strong].into_iter().flatten().min()
        else {
            if !rest.is_empty() {
                out.push(text(rest));
            }
            return out;
        };
        if !before.is_empty() {
            out.push(text(before));
        }
        out.push(match kind {
            0 => Node::Element(el("code").class("mj-ui-code").text(inner)),
            _ => Node::Element(el("strong").nodes(marks(inner))),
        });
        rest = after;
    }
}

/// A paragraph of prose; the empty string draws nothing a reader would take for content.
pub fn paragraph(prose: &str, known: &Known) -> El {
    el("p")
        .class("mj-ui-prose")
        .nodes(inline(prose.trim(), known))
}

/// A list of prose lines, each drawn as [`inline`].
pub fn bullets(lines: &[String], known: &Known) -> El {
    el("ul").class("mj-ui-prose-list").children(
        lines
            .iter()
            .map(|line| el("li").nodes(inline(line.trim(), known))),
    )
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
            html.contains("<code class=\"mj-ui-code\">majordomus bench</code>"),
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
    fn a_web_link_is_followed_and_any_other_address_is_only_shown() {
        let web = drawn("see [the docs](https://example.org/a?b=1) for **[more](https://x.y)**");
        assert!(web.contains("href=\"https://example.org/a?b=1\""), "{web}");
        assert!(web.contains("rel=\"noopener noreferrer\""), "{web}");
        assert!(web.contains("<strong><a"), "{web}");
        let hostile = drawn("click [here](javascript:alert(1)) or [there](/etc/passwd)");
        assert!(!hostile.contains("href"), "{hostile}");
        assert!(hostile.contains("here (javascript:alert(1"), "{hostile}");
    }

    #[test]
    fn whatever_a_record_holds_is_escaped_and_unclosed_markup_is_text() {
        let html = drawn("<script>alert(1)</script> `<b>` and a lone ` or ** or [x](");
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(
            html.contains("<code class=\"mj-ui-code\">&lt;b&gt;</code>"),
            "{html}"
        );
        assert!(html.contains("a lone ` or ** or [x]("), "{html}");
        // empty markup is the characters it is made of
        assert!(drawn("an empty `` span").contains("an empty `` span"));
    }

    #[test]
    fn marks_draw_code_and_strong_and_never_a_link() {
        let html = el("td")
            .nodes(marks(
                "run `majordomus ui` for **the** UI, see I2400 and [x](https://e.org)",
            ))
            .render();
        assert!(
            html.contains("<code class=\"mj-ui-code\">majordomus ui</code>"),
            "{html}"
        );
        assert!(html.contains("<strong>the</strong>"), "{html}");
        assert!(!html.contains("<a"), "{html}");
        assert!(html.contains("see I2400 and [x](https://e.org)"), "{html}");
        let hostile = el("td").nodes(marks("`<b>` and <i> and a lone `")).render();
        assert!(
            hostile
                .contains("<code class=\"mj-ui-code\">&lt;b&gt;</code> and &lt;i&gt; and a lone `"),
            "{hostile}"
        );
        assert_eq!(el("td").nodes(marks("")).render(), "<td></td>");
    }

    #[test]
    fn a_paragraph_and_a_list_draw_every_line() {
        let k = known();
        assert_eq!(
            paragraph("  one `two`  ", &k).render(),
            "<p class=\"mj-ui-prose\">one <code class=\"mj-ui-code\">two</code></p>"
        );
        let list = bullets(&["first I2400".into(), "second".into()], &k).render();
        assert!(
            list.starts_with("<ul class=\"mj-ui-prose-list\"><li>first <a"),
            "{list}"
        );
        assert!(list.ends_with("<li>second</li></ul>"), "{list}");
    }
}
