//! Markup that cannot be unescaped by accident. There is no template engine here and no
//! template string: a page is a tree of [`El`] values, and the only way text reaches the
//! output is through a method that escapes it.
//!
//! The Cockpit renders what the repository holds — rule bodies, prompt text, ADR prose,
//! file paths, a peer's announced intent — and every one of those is content this process
//! did not write. A template string with a `{}` in it puts the escaping decision at every
//! call site; this puts it in one place, and a page that wants raw markup has to say
//! [`El::raw`] and be read for it.

/// Escape text for an element body or an attribute value. The five characters that can
/// leave either context are replaced; nothing else is touched, so UTF-8 stays UTF-8.
///
/// ```
/// use majordomus_cli::cockpit::html::escape;
/// assert_eq!(escape("<script>&\"'"), "&lt;script&gt;&amp;&quot;&#39;");
/// assert_eq!(escape("příkaz"), "příkaz");
/// ```
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_into(&mut out, text);
    out
}

/// `text` escaped as [`escape`] does, appended to `out` with no string of its own: a page writes
/// every text and attribute through this, and a page can hold the whole plan (I4200). The five
/// characters it replaces are ASCII, so every cut falls on a character boundary, and a run with
/// nothing to replace is copied whole.
fn escape_into(out: &mut String, text: &str) {
    let mut from = 0;
    for (i, b) in text.bytes().enumerate() {
        let entity = match b {
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'&' => "&amp;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            _ => continue,
        };
        out.push_str(&text[from..i]);
        out.push_str(entity);
        from = i + 1;
    }
    out.push_str(&text[from..]);
}

/// Void elements: they take no children and are written without a closing tag.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/// One node of a page: an element with attributes and children, or a piece of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// An element.
    Element(El),
    /// Text, escaped when written.
    Text(String),
    /// Markup this process produced itself and vouches for.
    Raw(String),
}

/// An element under construction. Every builder method takes and returns `self`, so a
/// page reads as one expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct El {
    tag: &'static str,
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

/// An element with this tag.
///
/// ```
/// use majordomus_cli::cockpit::html::el;
/// let markup = el("p").class("lead").text("a < b").render();
/// assert_eq!(markup, r#"<p class="lead">a &lt; b</p>"#);
/// ```
pub fn el(tag: &'static str) -> El {
    El {
        tag,
        attributes: Vec::new(),
        children: Vec::new(),
    }
}

/// A text node.
pub fn text(value: impl Into<String>) -> Node {
    Node::Text(value.into())
}

/// Markup this process produced itself: an inlined SVG icon, a fragment already rendered
/// by another function here. Never repository content, never a request parameter.
pub fn raw(markup: impl Into<String>) -> Node {
    Node::Raw(markup.into())
}

/// Nothing. For a conditional branch that renders no element.
pub fn empty() -> Node {
    Node::Raw(String::new())
}

impl El {
    /// Set an attribute. A later set of the same name replaces the earlier one, so a
    /// helper can set a default that a caller overrides.
    pub fn attr(mut self, name: &str, value: impl Into<String>) -> Self {
        let value = value.into();
        match self.attributes.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = value,
            None => self.attributes.push((name.to_string(), value)),
        }
        self
    }

    /// Set an attribute only when there is a value.
    pub fn attr_if(self, name: &str, value: Option<impl Into<String>>) -> Self {
        match value {
            Some(v) => self.attr(name, v),
            None => self,
        }
    }

    /// A boolean attribute, present without a value (`hidden`, `open`, `disabled`).
    pub fn flag(mut self, name: &str) -> Self {
        if !self.attributes.iter().any(|(n, _)| n == name) {
            self.attributes.push((name.to_string(), String::new()));
        }
        self
    }

    /// Set `class`.
    pub fn class(self, value: impl Into<String>) -> Self {
        self.attr("class", value)
    }

    /// Add a child element.
    pub fn child(mut self, child: El) -> Self {
        self.children.push(Node::Element(child));
        self
    }

    /// Add a node.
    pub fn node(mut self, node: Node) -> Self {
        self.children.push(node);
        self
    }

    /// Add several children.
    pub fn children(mut self, children: impl IntoIterator<Item = El>) -> Self {
        self.children
            .extend(children.into_iter().map(Node::Element));
        self
    }

    /// Add several nodes.
    pub fn nodes(mut self, nodes: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(nodes);
        self
    }

    /// Add escaped text.
    pub fn text(mut self, value: impl Into<String>) -> Self {
        self.children.push(Node::Text(value.into()));
        self
    }

    /// Add markup this process produced itself. Never repository content.
    pub fn raw(mut self, markup: impl Into<String>) -> Self {
        self.children.push(Node::Raw(markup.into()));
        self
    }

    /// Apply a function when the condition holds: for a branch inside a builder chain.
    pub fn when(self, condition: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if condition {
            f(self)
        } else {
            self
        }
    }

    /// This element with every element that carries the attribute `name` replaced by what
    /// `with` makes of its value, when it makes anything; an element it leaves is searched
    /// on, one it replaces is not.
    ///
    /// ```
    /// use majordomus_cli::cockpit::html::el;
    /// let page = el("div").child(el("p").attr("data-src", "/a").text("later"));
    /// let filled = page.replacing("data-src", &mut |src| Some(el("p").text(src)));
    /// assert_eq!(filled.render(), "<div><p>/a</p></div>");
    /// ```
    pub fn replacing(self, name: &str, with: &mut dyn FnMut(&str) -> Option<El>) -> El {
        let value = self
            .attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone());
        if let Some(replacement) = value.and_then(|v| with(&v)) {
            return replacement;
        }
        let El {
            tag,
            attributes,
            children,
        } = self;
        let children = children
            .into_iter()
            .map(|c| match c {
                Node::Element(e) => Node::Element(e.replacing(name, with)),
                other => other,
            })
            .collect();
        El {
            tag,
            attributes,
            children,
        }
    }

    /// The element as markup.
    pub fn render(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        out.push('<');
        out.push_str(self.tag);
        for (name, value) in &self.attributes {
            out.push(' ');
            out.push_str(name);
            if !value.is_empty() {
                out.push_str("=\"");
                escape_into(out, value);
                out.push('"');
            }
        }
        out.push('>');
        if VOID.contains(&self.tag) {
            return;
        }
        for child in &self.children {
            match child {
                Node::Element(e) => e.write(out),
                Node::Text(t) => escape_into(out, t),
                Node::Raw(r) => out.push_str(r),
            }
        }
        out.push_str("</");
        out.push_str(self.tag);
        out.push('>');
    }
}

/// A whole page: the doctype, the head this Cockpit always emits, and a body.
///
/// The head is not a template a page may replace. Its content — the character set, the
/// viewport, the stylesheet, the theme script that runs before first paint — is the same
/// on every page by construction, so a new page cannot forget the parts that make the
/// Cockpit behave.
pub fn document(title: &str, head: Vec<El>, body: El) -> String {
    document_on(title, &[], head, body)
}

/// A whole page whose root element carries `root`'s attributes besides its language: the
/// theme contract, where a script written for the site reads it from `<html>` as the site's
/// pages carry it.
///
/// ```
/// use majordomus_cli::cockpit::html::{document_on, el};
/// let page = document_on("t", &[("data-theme-key", "a\"b".into())], vec![], el("body"));
/// assert!(page.contains("<html lang=\"en\" data-theme-key=\"a&quot;b\">"), "{page}");
/// ```
pub fn document_on(title: &str, root: &[(&str, String)], head: Vec<El>, body: El) -> String {
    let mut out = String::from("<!doctype html>\n<html lang=\"en\"");
    for (name, value) in root {
        out.push_str(&format!(" {name}=\"{}\"", escape(value)));
    }
    out.push_str(">\n<head>\n");
    out.push_str(&el("meta").attr("charset", "utf-8").render());
    out.push_str(
        &el("meta")
            .attr("name", "viewport")
            .attr("content", "width=device-width, initial-scale=1")
            .render(),
    );
    out.push_str(&el("title").text(title).render());
    for e in head {
        out.push_str(&e.render());
    }
    out.push_str("\n</head>\n");
    out.push_str(&body.render());
    out.push_str("\n</html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_carries_its_attributes_escaped_and_a_plain_page_none() {
        let page = document_on("t", &[("data-x", "<y>".into())], vec![], el("body"));
        assert!(
            page.contains("<html lang=\"en\" data-x=\"&lt;y&gt;\">"),
            "{page}"
        );
        assert!(document("t", vec![], el("body")).contains("<html lang=\"en\">\n<head>"));
    }

    #[test]
    fn repository_content_cannot_escape_an_element_body() {
        let hostile = "</script><img src=x onerror=alert(1)>";
        let rendered = el("pre").text(hostile).render();
        assert!(!rendered.contains("<img"), "{rendered}");
        assert!(rendered.contains("&lt;img"), "{rendered}");
    }

    #[test]
    fn repository_content_cannot_escape_an_attribute() {
        let hostile = "\" onmouseover=\"alert(1)";
        let rendered = el("a").attr("href", hostile).text("x").render();
        assert!(!rendered.contains("onmouseover=\"alert"), "{rendered}");
        assert!(rendered.contains("&quot;"), "{rendered}");
    }

    #[test]
    fn a_void_element_has_no_closing_tag() {
        assert_eq!(el("br").render(), "<br>");
        assert_eq!(
            el("input").attr("type", "text").flag("required").render(),
            "<input type=\"text\" required>"
        );
    }

    #[test]
    fn a_later_attribute_replaces_an_earlier_one() {
        assert_eq!(
            el("div").class("a").class("b").render(),
            "<div class=\"b\"></div>"
        );
    }

    #[test]
    fn a_document_carries_the_head_every_page_gets() {
        let page = document("T", vec![el("link").attr("rel", "stylesheet")], el("body"));
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.contains("<title>T</title>"));
        assert!(page.contains("charset=\"utf-8\""));
        assert!(page.contains("<link rel=\"stylesheet\">"));
        assert!(page.trim_end().ends_with("</html>"));
    }
}
