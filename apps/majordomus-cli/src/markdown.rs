//! Markdown, drawn: the one renderer of every Markdown body a surface shows.
//!
//! Most of what this repository says is Markdown — a rule's rationale, a decision record, a
//! skill's procedure, a use case, a document, the prose of a milestone, a pull request's
//! description as the forge returned it. Shown as the characters it is written in, a table is
//! a row of pipes and a diagram is a paragraph of arrows. Shown by a renderer per surface,
//! the same record looks different on every page that shows it. So there is one renderer,
//! here, and every surface that shows a Markdown body calls it.
//!
//! # What it guarantees
//!
//! - **Nothing a record holds is written as markup.** The parser (`pulldown-cmark`, CommonMark
//!   with GitHub's tables, strikethrough, task lists and alerts) yields events, and the events
//!   are drawn through [`crate::cockpit::html`], which escapes every byte. Raw HTML in a body
//!   is shown as the text it is; an HTML comment is dropped, as every renderer drops it.
//! - **A link leads somewhere this decided, or nowhere.** An `http`/`https` address is followed;
//!   a fragment stays on the page; anything else is asked of the caller's [`Options::link`],
//!   which knows what the surface can resolve. What nobody resolves is text, with the address
//!   it named beside it — never an anchor to a scheme a record chose.
//! - **A record's id is a link to the record.** A word the caller's [`Options::word`] knows —
//!   an issue, a milestone — is drawn as a link to it wherever prose names it.
//! - **One document, one outline.** Headings are drawn below the level the caller says its
//!   own heading has, so a body's `# Title` never becomes a second `h1` on the page.
//! - **A diagram is still its source.** A fenced `mermaid` block is drawn as its text, marked
//!   for the page's script to draw as a figure; without script the reader has the source.
//! - **No inline style.** A table's column alignment is a class, never a `style` attribute.

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, CowStr, Event, HeadingLevel, Parser, Tag, TagEnd,
};

use crate::cockpit::html::{el, text, El, Node};

/// What a surface tells the renderer about itself.
pub(crate) struct Options<'a> {
    /// The level a body's top heading is drawn at: `3` draws `# Title` as `h3`. Clamped to
    /// the levels HTML has.
    pub heading: u8,
    /// Where a link that is not a web address or a fragment leads, when the surface can say.
    pub link: &'a dyn Fn(&str) -> Option<String>,
    /// Where a word leads, when it is the id of a record the surface can show.
    pub word: &'a dyn Fn(&str) -> Option<String>,
}

/// Resolves nothing: for a body whose links and ids the surface cannot follow.
pub(crate) fn nowhere(_: &str) -> Option<String> {
    None
}

impl Options<'static> {
    /// A body drawn under a page's own headings, following web links only.
    pub(crate) fn plain() -> Self {
        Options {
            heading: 3,
            link: &nowhere,
            word: &nowhere,
        }
    }
}

/// The Markdown below a file's front matter: what a reader reads. A file that opens with no
/// front matter, or with one that never closes, is its own body.
pub(crate) fn body(content: &str) -> &str {
    content
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(_, body)| body))
        .unwrap_or(content)
}

/// A Markdown body as a block: `<div class="mj-md">…</div>`.
pub(crate) fn render(source: &str, options: &Options<'_>) -> El {
    el("div").class("mj-md").nodes(draw(source, options, false))
}

/// One line of Markdown as inline nodes: a paragraph's content without the paragraph, for a
/// sentence a record carries in a field.
pub(crate) fn inline(source: &str, options: &Options<'_>) -> Vec<Node> {
    draw(source, options, true)
}

/// An element being filled: what it will be, and what it holds so far.
struct Frame {
    /// The element, without children; `None` for a frame that only collects.
    element: Option<El>,
    nodes: Vec<Node>,
}

impl Frame {
    fn of(element: El) -> Frame {
        Frame {
            element: Some(element),
            nodes: Vec::new(),
        }
    }

    fn close(self) -> Vec<Node> {
        match self.element {
            Some(element) => vec![Node::Element(element.nodes(self.nodes))],
            None => self.nodes,
        }
    }
}

/// The state of one drawing.
struct Drawing<'a> {
    options: &'a Options<'a>,
    /// Paragraphs are not drawn as elements: the caller wants a sentence's content.
    inline: bool,
    stack: Vec<Frame>,
    /// A fenced or indented code block being read: its language and its text so far.
    code: Option<(String, String)>,
    /// A block of raw HTML being read.
    html: Option<String>,
    /// How deep inside links and code the cursor is: ids are not linked there.
    quiet: usize,
    /// The alignment of each column of the table being drawn, and the next cell's index.
    columns: Vec<Alignment>,
    cell: usize,
    in_head: bool,
}

fn heading_tag(level: HeadingLevel, base: u8) -> &'static str {
    let own = match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    };
    match (own + base.clamp(1, 6) - 1).min(6) {
        1 => "h1",
        2 => "h2",
        3 => "h3",
        4 => "h4",
        5 => "h5",
        _ => "h6",
    }
}

/// Whether an address is one the web follows.
fn is_web(address: &str) -> bool {
    address.starts_with("https://") || address.starts_with("http://")
}

/// Whether a character can be part of a record's id (`I2400`, `M004`, `a-named-milestone`).
fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Raw HTML with its comments removed: what is left is what a reader would have been shown.
fn without_comments(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = rest[start..]
            .find("-->")
            .map_or("", |end| &rest[start + end + 3..]);
    }
    out.push_str(rest);
    out
}

impl<'a> Drawing<'a> {
    fn push(&mut self, element: El) {
        self.stack.push(Frame::of(element));
    }

    fn append(&mut self, node: Node) {
        if let Some(top) = self.stack.last_mut() {
            top.nodes.push(node);
        }
    }

    /// Close the frame on top and hand what it drew to the one below.
    fn pop(&mut self) -> Vec<Node> {
        let drawn = self.stack.pop().map(Frame::close).unwrap_or_default();
        if self.stack.is_empty() {
            // the root is never popped by an event; put it back for whoever asks next
            self.stack.push(Frame {
                element: None,
                nodes: drawn,
            });
            return Vec::new();
        }
        drawn
    }

    fn close(&mut self) {
        let drawn = self.pop();
        if let Some(top) = self.stack.last_mut() {
            top.nodes.extend(drawn);
        }
    }

    /// Prose, with every word the surface knows as a record drawn as a link to it.
    fn prose(&mut self, run: &str) {
        if self.quiet > 0 {
            return self.append(text(run));
        }
        let (mut plain, mut word) = (String::new(), String::new());
        for c in run.chars().chain(std::iter::once(' ')) {
            if is_id_char(c) {
                word.push(c);
                continue;
            }
            match (self.options.word)(&word) {
                Some(href) if !word.is_empty() => {
                    if !plain.is_empty() {
                        self.append(text(std::mem::take(&mut plain)));
                    }
                    self.append(Node::Element(
                        el("a")
                            .class("mj-md-ref")
                            .attr("href", href)
                            .text(std::mem::take(&mut word)),
                    ));
                }
                _ => plain.push_str(&std::mem::take(&mut word)),
            }
            plain.push(c);
        }
        // the sentinel space that closed the last word is not the record's
        plain.pop();
        if !plain.is_empty() {
            self.append(text(plain));
        }
    }

    /// Where the names in a diagram's source lead, as the JSON object the page's script reads:
    /// every token the surface resolves — a record's id through [`Options::word`], a path or
    /// URI through [`Options::link`] — by the token. `None` when the source names nothing the
    /// surface can follow, so a diagram of abstractions carries no empty attribute.
    fn named(&self, source: &str) -> Option<String> {
        let leads: std::collections::BTreeMap<&str, String> = source
            .split(|c: char| !(is_id_char(c) || matches!(c, '.' | '/' | ':')))
            .filter(|token| !token.is_empty())
            .filter_map(|token| {
                (self.options.word)(token)
                    .or_else(|| (self.options.link)(token))
                    .map(|href| (token, href))
            })
            .collect();
        (!leads.is_empty()).then(|| serde_json::json!(leads).to_string())
    }

    /// The element a link's label is drawn in: an anchor when the address leads somewhere
    /// this decided, the label and the address as text when it does not.
    fn link(&self, address: &str, class: &str) -> El {
        if is_web(address) {
            el("a")
                .class(class)
                .attr("href", address)
                .attr("rel", "noopener noreferrer")
        } else if address.starts_with('#') {
            el("a").class(class).attr("href", address)
        } else if let Some(href) = (self.options.link)(address) {
            el("a").class(class).attr("href", href)
        } else {
            el("span").class("mj-md-unlinked").attr("title", address)
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph if self.inline => self.stack.push(Frame {
                element: None,
                nodes: Vec::new(),
            }),
            Tag::Paragraph => self.push(el("p")),
            Tag::Heading { level, .. } => {
                self.push(el(heading_tag(level, self.options.heading)).class("mj-md-heading"))
            }
            Tag::BlockQuote(kind) => {
                let alert = match kind {
                    Some(BlockQuoteKind::Note) => " mj-md-quote--note",
                    Some(BlockQuoteKind::Tip) => " mj-md-quote--tip",
                    Some(BlockQuoteKind::Important) => " mj-md-quote--important",
                    Some(BlockQuoteKind::Warning) => " mj-md-quote--warning",
                    Some(BlockQuoteKind::Caution) => " mj-md-quote--caution",
                    None => "",
                };
                self.push(el("blockquote").class(format!("mj-md-quote{alert}")));
            }
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split(|c: char| c.is_whitespace() || c == ',')
                        .next()
                        .unwrap_or_default()
                        .to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((language, String::new()));
            }
            Tag::HtmlBlock => self.html = Some(String::new()),
            Tag::List(Some(first)) => {
                self.push(el("ol").when(first != 1, |l| l.attr("start", first.to_string())))
            }
            Tag::List(None) => self.push(el("ul")),
            Tag::Item => self.push(el("li")),
            Tag::Table(columns) => {
                self.columns = columns;
                self.push(el("table").class("mj-table"));
            }
            Tag::TableHead => {
                self.in_head = true;
                self.cell = 0;
                self.push(el("thead"));
                self.push(el("tr"));
            }
            Tag::TableRow => {
                self.cell = 0;
                self.push(el("tr"));
            }
            Tag::TableCell => {
                let aligned = match self.columns.get(self.cell) {
                    Some(Alignment::Left) => Some("mj-md-left"),
                    Some(Alignment::Center) => Some("mj-md-center"),
                    Some(Alignment::Right) => Some("mj-md-right"),
                    _ => None,
                };
                self.cell += 1;
                let cell = match self.in_head {
                    true => el("th").attr("scope", "col"),
                    false => el("td"),
                };
                self.push(cell.attr_if("class", aligned));
            }
            Tag::Emphasis => self.push(el("em")),
            Tag::Strong => self.push(el("strong")),
            Tag::Strikethrough => self.push(el("del")),
            Tag::Superscript => self.push(el("sup")),
            Tag::Subscript => self.push(el("sub")),
            Tag::Link { dest_url, .. } => {
                self.quiet += 1;
                let anchor = self.link(&dest_url, "mj-md-link");
                self.push(anchor);
            }
            // a picture is not fetched from wherever a record points: its description is
            // shown, as a link to the picture when the address is one the web follows
            Tag::Image { dest_url, .. } => {
                self.quiet += 1;
                let anchor = self.link(&dest_url, "mj-md-image");
                self.push(anchor);
            }
            // footnotes, definition lists and metadata blocks are not asked of the parser;
            // were one to arrive it is drawn as a block of its own rather than lost
            _ => self.push(el("div")),
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::CodeBlock => {
                let (language, source) = self.code.take().unwrap_or_default();
                let block = match language.as_str() {
                    // drawn by the page's script as a figure; the source is the fallback.
                    // What its nodes name is resolved here, where the surface's resolvers
                    // are, so the figure's nodes lead where the same words would in prose.
                    "mermaid" => el("pre")
                        .class("mj-md-diagram mermaid")
                        .attr("data-mj-diagram", "mermaid")
                        .attr("tabindex", "0")
                        .attr_if("data-mj-links", self.named(&source))
                        .text(source),
                    _ => el("pre")
                        .class("mj-pre mj-md-pre")
                        .attr("tabindex", "0")
                        .child(
                            el("code")
                                .when(!language.is_empty(), |c| {
                                    c.attr("data-language", language.clone())
                                })
                                .text(source),
                        ),
                };
                self.append(Node::Element(block));
            }
            TagEnd::HtmlBlock => {
                let shown = without_comments(&self.html.take().unwrap_or_default());
                if self.inline {
                    // a sentence that opens with a tag is still a sentence: no block is
                    // drawn inside the line the caller asked for
                    if !shown.trim().is_empty() {
                        self.append(text(shown.trim_end().to_string()));
                    }
                } else if !shown.trim().is_empty() {
                    self.append(Node::Element(
                        el("pre")
                            .class("mj-pre mj-md-pre")
                            .attr("tabindex", "0")
                            .child(el("code").attr("data-language", "html").text(shown)),
                    ));
                }
            }
            TagEnd::TableHead => {
                self.close(); // the row
                self.close(); // the head
                self.in_head = false;
                self.push(el("tbody"));
            }
            TagEnd::Table => {
                self.close(); // the body
                let table = self.pop();
                // the wrapper scrolls, so it is in the tab order (WCAG 2.1.1)
                self.append(Node::Element(
                    el("div")
                        .class("mj-table-wrap")
                        .attr("tabindex", "0")
                        .nodes(table),
                ));
                self.columns.clear();
            }
            TagEnd::Link | TagEnd::Image => {
                self.quiet = self.quiet.saturating_sub(1);
                self.close();
            }
            _ => self.close(),
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(run) => self.run(run),
            Event::Code(code) => self.append(Node::Element(
                el("code").class("mj-md-code").text(code.to_string()),
            )),
            Event::Html(html) => match self.html.as_mut() {
                Some(block) => block.push_str(&html),
                None => self.literal(&html),
            },
            Event::InlineHtml(html) => self.literal(&html),
            Event::SoftBreak => self.append(text(" ")),
            Event::HardBreak => self.append(Node::Element(el("br"))),
            Event::Rule => self.append(Node::Element(el("hr").class("mj-md-rule"))),
            Event::TaskListMarker(done) => self.append(Node::Element(
                el("input")
                    .attr("type", "checkbox")
                    .attr("aria-label", if done { "done" } else { "not done" })
                    .flag("disabled")
                    .when(done, |i| i.flag("checked")),
            )),
            // mathematics and footnotes are not asked of the parser; what it would have
            // marked is prose
            Event::InlineMath(run) | Event::DisplayMath(run) | Event::FootnoteReference(run) => {
                self.run(run)
            }
        }
    }

    /// Text: a code block's source while one is open, prose otherwise.
    fn run(&mut self, run: CowStr<'_>) {
        match self.code.as_mut() {
            Some((_, source)) => source.push_str(&run),
            None => self.prose(&run),
        }
    }

    /// Raw HTML met inside prose: shown as the text it is, its comments dropped.
    fn literal(&mut self, html: &str) {
        let shown = without_comments(html);
        if !shown.is_empty() {
            self.append(text(shown));
        }
    }
}

fn draw(source: &str, options: &Options<'_>, inline: bool) -> Vec<Node> {
    let mut asked = pulldown_cmark::Options::empty();
    asked.insert(pulldown_cmark::Options::ENABLE_TABLES);
    asked.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    asked.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    asked.insert(pulldown_cmark::Options::ENABLE_GFM);
    let mut drawing = Drawing {
        options,
        inline,
        stack: vec![Frame {
            element: None,
            nodes: Vec::new(),
        }],
        code: None,
        html: None,
        quiet: 0,
        columns: Vec::new(),
        cell: 0,
        in_head: false,
    };
    for event in Parser::new_ext(source, asked) {
        drawing.event(event);
    }
    // whatever the parser left open is closed, so no element is lost
    while drawing.stack.len() > 1 {
        drawing.close();
    }
    drawing
        .stack
        .pop()
        .map(|root| root.nodes)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(source: &str) -> String {
        render(source, &Options::plain()).render()
    }

    #[test]
    fn a_body_is_drawn_as_the_document_it_is() {
        let drawn = html(
            "# Rationale\n\nA **strong** point, an *aside*, ~~a mistake~~ and `code`.\n\n\
             - one\n- two\n\n1. first\n2. second\n\n---\n\n> quoted\n",
        );
        for expected in [
            "<div class=\"mj-md\">",
            "<h3 class=\"mj-md-heading\">Rationale</h3>",
            "<strong>strong</strong>",
            "<em>aside</em>",
            "<del>a mistake</del>",
            "<code class=\"mj-md-code\">code</code>",
            "<ul><li>one</li><li>two</li></ul>",
            "<ol><li>first</li><li>second</li></ol>",
            "<hr class=\"mj-md-rule\">",
            "<blockquote class=\"mj-md-quote\"><p>quoted</p></blockquote>",
        ] {
            assert!(
                drawn.contains(expected),
                "{expected} is missing from {drawn}"
            );
        }
    }

    #[test]
    fn headings_stay_below_the_pages_own_and_never_pass_the_last_level() {
        let under = |heading| Options {
            heading,
            ..Options::plain()
        };
        let drawn = render("# one\n\n## two\n\n###### six", &under(2)).render();
        assert!(
            drawn.contains("<h2 class=\"mj-md-heading\">one</h2>"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<h3 class=\"mj-md-heading\">two</h3>"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<h6 class=\"mj-md-heading\">six</h6>"),
            "{drawn}"
        );
        // a base no document has is the nearest one it has
        assert!(render("# t", &under(0)).render().contains("<h1"));
        assert!(render("### t", &under(9)).render().contains("<h6"));
        for (source, tag) in [("#### a", "<h6"), ("##### a", "<h6"), ("# a", "<h3")] {
            assert!(html(source).contains(tag), "{source}");
        }
        assert!(render("#### a", &under(1)).render().contains("<h4"));
        assert!(render("##### a", &under(1)).render().contains("<h5"));
    }

    #[test]
    fn a_table_keeps_its_alignment_as_a_class_and_scrolls_in_the_tab_order() {
        let drawn =
            html("| source | before | note | plain |\n|:---|---:|:---:|---|\n| a | 1 | x | y |\n");
        assert!(
            drawn
                .contains("<div class=\"mj-table-wrap\" tabindex=\"0\"><table class=\"mj-table\">"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<thead><tr><th scope=\"col\" class=\"mj-md-left\">source</th>"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<th scope=\"col\" class=\"mj-md-right\">before</th>"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<th scope=\"col\" class=\"mj-md-center\">note</th>"),
            "{drawn}"
        );
        assert!(drawn.contains("<th scope=\"col\">plain</th>"), "{drawn}");
        assert!(
            drawn.contains(
                "<tbody><tr><td class=\"mj-md-left\">a</td><td class=\"mj-md-right\">1</td>"
            ),
            "{drawn}"
        );
        assert!(!drawn.contains("style="), "no inline style: {drawn}");
    }

    #[test]
    fn a_code_block_names_its_language_and_a_diagram_is_marked_for_drawing() {
        let drawn = html("```rust,ignore\nfn main() {}\n```\n\n    indented\n\n```mermaid\ngraph TD; a-->b\n```\n");
        assert!(
            drawn.contains("<pre class=\"mj-pre mj-md-pre\" tabindex=\"0\"><code data-language=\"rust\">fn main() {}\n</code></pre>"),
            "{drawn}"
        );
        assert!(drawn.contains("<code>indented\n</code>"), "{drawn}");
        assert!(
            drawn.contains("<pre class=\"mj-md-diagram mermaid\" data-mj-diagram=\"mermaid\" tabindex=\"0\">graph TD; a--&gt;b\n</pre>"),
            "{drawn}"
        );
    }

    #[test]
    fn a_diagrams_nodes_lead_where_the_same_names_would_in_prose() {
        let ids = |word: &str| (word == "I2400").then(|| "/issue/I2400".to_string());
        let paths = |address: &str| (address == "docs/MESH.md").then(|| "/doc/mesh".to_string());
        let options = Options {
            word: &ids,
            link: &paths,
            ..Options::plain()
        };
        let drawn = render(
            "```mermaid\ngraph TD; I2400[\"I2400\"] --> M[docs/MESH.md]; M --> other\n```\n",
            &options,
        )
        .render();
        assert!(
            drawn.contains(
                "data-mj-links=\"{&quot;I2400&quot;:&quot;/issue/I2400&quot;,&quot;docs/MESH.md&quot;:&quot;/doc/mesh&quot;}\""
            ),
            "{drawn}"
        );
        // a diagram that names nothing the surface follows carries no attribute
        assert!(!html("```mermaid\ngraph TD; a-->b\n```\n").contains("data-mj-links"));
    }

    #[test]
    fn whatever_a_record_holds_is_text_and_a_comment_is_dropped() {
        let drawn = html(
            "<script>alert(1)</script>\n\nA <b>bold</b> word <!-- hidden --> here.\n\n<!-- a whole comment -->\n\n<div onclick=\"x()\">\nblock\n</div>\n",
        );
        assert!(
            !drawn.contains("<script>") && !drawn.contains("<b>"),
            "{drawn}"
        );
        assert!(
            drawn.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "{drawn}"
        );
        assert!(drawn.contains("&lt;b&gt;bold&lt;/b&gt;"), "{drawn}");
        assert!(
            !drawn.contains("hidden") && !drawn.contains("a whole comment"),
            "{drawn}"
        );
        assert!(drawn.contains("data-language=\"html\""), "{drawn}");
        assert!(!drawn.contains("<div onclick"), "{drawn}");
        // a comment that never closes hides the rest, as it does everywhere
        assert_eq!(without_comments("a <!-- b"), "a ");
        assert_eq!(without_comments("a <!-- b --> c <!-- d --> e"), "a  c  e");
    }

    #[test]
    fn a_link_leads_where_this_decided_or_nowhere() {
        let known = |address: &str| (address == "docs/MESH.md").then(|| "/doc/mesh".to_string());
        let options = Options {
            link: &known,
            ..Options::plain()
        };
        let drawn = render(
            "[web](https://example.org/a) [here](#part) [doc](docs/MESH.md) [gone](docs/NONE.md) \
             [bad](javascript:alert(1)) ![a picture](https://example.org/p.png) ![local](p.png)",
            &options,
        )
        .render();
        for expected in [
            "<a class=\"mj-md-link\" href=\"https://example.org/a\" rel=\"noopener noreferrer\">web</a>",
            "<a class=\"mj-md-link\" href=\"#part\">here</a>",
            "<a class=\"mj-md-link\" href=\"/doc/mesh\">doc</a>",
            "<span class=\"mj-md-unlinked\" title=\"docs/NONE.md\">gone</span>",
            "<span class=\"mj-md-unlinked\" title=\"javascript:alert(1)\">bad</span>",
            "<a class=\"mj-md-image\" href=\"https://example.org/p.png\" rel=\"noopener noreferrer\">a picture</a>",
            "<span class=\"mj-md-unlinked\" title=\"p.png\">local</span>",
        ] {
            assert!(drawn.contains(expected), "{expected} is missing from {drawn}");
        }
        assert!(!drawn.contains("href=\"javascript"), "{drawn}");
        assert!(!drawn.contains("<img"), "no picture is fetched: {drawn}");
    }

    #[test]
    fn a_records_id_is_a_link_wherever_prose_names_it_and_nowhere_else() {
        let ids = |word: &str| (word == "I2400").then(|| "/issue/I2400".to_string());
        let options = Options {
            word: &ids,
            ..Options::plain()
        };
        let drawn = render(
            "See I2400, not XI2400 nor I24000. In `I2400` and [I2400](https://x.y) it is not.\n\nI2400",
            &options,
        )
        .render();
        assert_eq!(drawn.matches("class=\"mj-md-ref\"").count(), 2, "{drawn}");
        assert!(
            drawn
                .contains("See <a class=\"mj-md-ref\" href=\"/issue/I2400\">I2400</a>, not XI2400"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<code class=\"mj-md-code\">I2400</code>"),
            "{drawn}"
        );
        assert!(
            drawn.ends_with("<p><a class=\"mj-md-ref\" href=\"/issue/I2400\">I2400</a></p></div>"),
            "{drawn}"
        );
    }

    #[test]
    fn a_task_list_an_alert_and_a_numbered_list_from_three_are_drawn() {
        let drawn = html("- [x] shipped\n- [ ] owed\n\n3. third\n\n> [!WARNING]\n> careful\n\nline one  \nline two\nsame line\n");
        assert!(
            drawn.contains("<input type=\"checkbox\" aria-label=\"done\" disabled checked>"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<input type=\"checkbox\" aria-label=\"not done\" disabled>"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<ol start=\"3\"><li>third</li></ol>"),
            "{drawn}"
        );
        assert!(
            drawn.contains("<blockquote class=\"mj-md-quote mj-md-quote--warning\">"),
            "{drawn}"
        );
        assert!(drawn.contains("line one<br>line two same line"), "{drawn}");
        for (kind, class) in [
            ("NOTE", "note"),
            ("TIP", "tip"),
            ("IMPORTANT", "important"),
            ("CAUTION", "caution"),
        ] {
            assert!(
                html(&format!("> [!{kind}]\n> x")).contains(&format!("mj-md-quote--{class}")),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_line_is_drawn_without_its_paragraph() {
        let drawn = el("p")
            .nodes(inline("a `b` and **c**", &Options::plain()))
            .render();
        assert_eq!(
            drawn,
            "<p>a <code class=\"mj-md-code\">b</code> and <strong>c</strong></p>"
        );
        assert!(inline("", &Options::plain()).is_empty());
        // a line that opens with a tag is text in the line, never a block inside it
        let tagged = el("p")
            .nodes(inline("<script>alert(1)</script> then", &Options::plain()))
            .render();
        assert_eq!(tagged, "<p>&lt;script&gt;alert(1)&lt;/script&gt; then</p>");
        assert!(inline("<!-- only a comment -->", &Options::plain()).is_empty());
    }

    #[test]
    fn the_body_is_what_follows_the_front_matter() {
        assert_eq!(body("---\nid: x\n---\n# Title\n"), "# Title\n");
        assert_eq!(body("# No front matter\n"), "# No front matter\n");
        assert_eq!(
            body("---\nid: x\nnever closed\n"),
            "---\nid: x\nnever closed\n"
        );
    }

    #[test]
    fn superscripts_subscripts_and_what_was_not_asked_for_are_still_drawn() {
        let options = Options::plain();
        let mut drawing = Drawing {
            options: &options,
            inline: false,
            stack: vec![Frame {
                element: None,
                nodes: Vec::new(),
            }],
            code: None,
            html: None,
            quiet: 0,
            columns: Vec::new(),
            cell: 0,
            in_head: false,
        };
        for event in [
            Event::Start(Tag::Superscript),
            Event::Text("up".into()),
            Event::End(TagEnd::Superscript),
            Event::Start(Tag::Subscript),
            Event::Text("down".into()),
            Event::End(TagEnd::Subscript),
            Event::Start(Tag::DefinitionList),
            Event::InlineMath("x".into()),
            Event::DisplayMath("y".into()),
            Event::FootnoteReference("n".into()),
            Event::InlineHtml("<i>".into()),
            Event::InlineHtml("<!-- gone -->".into()),
            // left open on purpose: the drawing closes what the parser did not
        ] {
            drawing.event(event);
        }
        while drawing.stack.len() > 1 {
            drawing.close();
        }
        let drawn = el("div").nodes(drawing.pop_root()).render();
        assert_eq!(
            drawn,
            "<div><sup>up</sup><sub>down</sub><div>xyn&lt;i&gt;</div></div>"
        );
        // closing with nothing open loses nothing
        let mut empty = Drawing {
            stack: Vec::new(),
            ..drawing
        };
        empty.append(text("dropped"));
        assert!(empty.pop().is_empty());
        empty.end(TagEnd::CodeBlock);
        empty.end(TagEnd::HtmlBlock);
    }

    impl Drawing<'_> {
        fn pop_root(&mut self) -> Vec<Node> {
            self.stack.pop().map(|root| root.nodes).unwrap_or_default()
        }
    }
}
