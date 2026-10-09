//! The evidence figure: a drawing in which every element answers for itself (ADR 0122).
//!
//! A figure here is not an illustration beside the data. It is a map of the data, and
//! every box and every line on it is a control that says, when it is chosen, what it is,
//! how sure the figure is of it and where that came from. The grammar is the rule
//! `project.figures-are-maps-of-evidence`:
//!
//! - **the head names the question** the figure answers, and the caption says what on it
//!   is recorded and what is inferred;
//! - **every element carries a claim** — [`Claim`] — shown as a word and drawn as a line
//!   style, so that a derived edge never looks like a declared one and the legend says
//!   which is which;
//! - **choosing an element explains it** in the information box under the drawing, which
//!   works with a pointer, a finger and a keyboard alike;
//! - **detail lives in subtrees**, not in the drawing: a box with members carries their
//!   count, and the members are a native `<details>` tree under the figure;
//! - **the drawing is an enhancement**: the same facts are in a table under it, and a
//!   reader with no script and no SVG has all of them.
//!
//! The layout is computed here, in columns, and is a function of the data alone: the same
//! answer draws the same SVG, byte for byte, and no layout library is shipped to the
//! browser. `share/cockpit/flow.js` adds the information box, keyboard selection and
//! expanding every subtree at once; nothing else depends on it.
//!
//! ```
//! use majordomus_cli::cockpit::figure::{Claim, Column, Flow, FlowEdge, FlowNode};
//!
//! let flow = Flow {
//!     id: "owners".into(),
//!     question: "Who owns the company?".into(),
//!     caption: "Solid lines are registry entries; the dashed one is our inference.".into(),
//!     columns: vec![
//!         Column::new("Owner", vec![FlowNode::new("fund", "The fund", Claim::Declared)]),
//!         Column::new("Company", vec![FlowNode::new("co", "The company", Claim::Declared)]),
//!     ],
//!     edges: vec![FlowEdge::new("fund", "co", "100 %", Claim::Derived)],
//!     data: None,
//! };
//! let markup = flow.render().render();
//! assert!(markup.contains(r#"data-mj-figure="owners""#));
//! assert!(markup.contains("Who owns the company?"));
//! // the dashed edge says what it is in words, not only in a dash
//! assert!(markup.contains("mj-status--derived"));
//! ```

use super::html::{el, El};
use super::view::{badge, link, row, table, text_cell};

/// How sure a figure is of one of its elements: the claim it makes, in the design's own
/// status words, so the colour and the word are the ones every other surface uses.
///
/// The order is the order a legend lists them in: from what is recorded to what is not
/// known.
///
/// ```
/// use majordomus_cli::cockpit::figure::Claim;
///
/// assert_eq!(Claim::Declared.word(), "declared");
/// assert_eq!(Claim::Derived.dash(), Some("6 4"));
/// assert_eq!(Claim::Declared.dash(), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Claim {
    /// Recorded where it lives: a front matter key, a registry entry, a file in the tree.
    Declared,
    /// Inferred by reading other records: a backlink, a sum, a match across sources.
    Derived,
    /// A model's estimate: not a record of anything, and drawn so.
    Estimated,
    /// Context, not the subject: outside the layer the figure reads, named and not read,
    /// and drawn in grey so it is never compared with what is.
    External,
    /// Was so and is no longer: a former owner, a superseded version. Drawn dotted and
    /// grey, and never counted with the present.
    Historical,
    /// Looked for and not found: the reference points at nothing that exists. The figure
    /// says where it looked.
    Missing,
    /// The source could not be read, or nothing decided it. Never drawn as clean, healthy
    /// or empty: an unread source is not a negative.
    Unknown,
}

impl Claim {
    /// Every claim, in legend order.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::Claim;
    /// assert_eq!(Claim::ALL.first(), Some(&Claim::Declared));
    /// assert_eq!(Claim::ALL.len(), 7);
    /// ```
    pub const ALL: [Claim; 7] = [
        Claim::Declared,
        Claim::Derived,
        Claim::Estimated,
        Claim::External,
        Claim::Historical,
        Claim::Missing,
        Claim::Unknown,
    ];

    /// The status word this claim is shown as; `share/design/tokens.yaml` files every one
    /// of them under a status, so the badge is coloured by the declaration, not by this
    /// file.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::Claim;
    /// assert_eq!(Claim::Missing.word(), "missing");
    /// assert_eq!(Claim::Estimated.word(), "estimated");
    /// ```
    pub fn word(self) -> &'static str {
        match self {
            Claim::Declared => "declared",
            Claim::Derived => "derived",
            Claim::Estimated => "estimated",
            Claim::External => "external",
            Claim::Historical => "historical",
            Claim::Missing => "missing",
            Claim::Unknown => "unknown",
        }
    }

    /// What the claim means, in the sentence a legend shows beside its line.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::Claim;
    /// assert!(Claim::Derived.meaning().contains("inferred"));
    /// ```
    pub fn meaning(self) -> &'static str {
        match self {
            Claim::Declared => "read at the source that records it",
            Claim::Derived => "inferred from records, by a computation that can be repeated",
            Claim::Estimated => "a judgement with its basis stated, not a record",
            Claim::External => "context outside the subject; named, not read",
            Claim::Historical => "was so, and is no longer",
            Claim::Missing => "looked for and not found",
            Claim::Unknown => "the source could not be read; not a negative",
        }
    }

    /// The dash pattern the claim is drawn with; a solid line is a record.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::Claim;
    /// assert_eq!(Claim::Missing.dash(), Some("2 3"));
    /// assert_eq!(Claim::External.dash(), None);
    /// ```
    pub fn dash(self) -> Option<&'static str> {
        match self {
            Claim::Declared | Claim::External => None,
            Claim::Derived => Some("6 4"),
            Claim::Estimated => Some("8 3 2 3"),
            Claim::Historical => Some("1 5"),
            Claim::Missing => Some("2 3"),
            Claim::Unknown => Some("4 2 1 2"),
        }
    }
}

/// One member of a box's subtree: a row with its own claim, an optional address, and
/// members of its own, which open one level at a time.
///
/// ```
/// use majordomus_cli::cockpit::figure::{Claim, Member};
///
/// let m = Member::new("project.x@1", Claim::Declared)
///     .detail("rule")
///     .href("/cockpit/objects/rule/project-x-1");
/// assert_eq!(m.href.as_deref(), Some("/cockpit/objects/rule/project-x-1"));
/// assert!(m.members.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// What the row shows first.
    pub label: String,
    /// The line under it: a kind, a role, a date.
    pub detail: String,
    /// How sure the figure is of this member.
    pub claim: Claim,
    /// Where the member's own page is, when it has one.
    pub href: Option<String>,
    /// The next level down.
    pub members: Vec<Member>,
}

impl Member {
    /// A member with a label and a claim and nothing else yet.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, Member};
    /// assert_eq!(Member::new("a", Claim::Derived).claim, Claim::Derived);
    /// ```
    pub fn new(label: impl Into<String>, claim: Claim) -> Self {
        Member {
            label: label.into(),
            detail: String::new(),
            claim,
            href: None,
            members: Vec::new(),
        }
    }

    /// The same member with a line of detail under its label.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, Member};
    /// assert_eq!(Member::new("a", Claim::Declared).detail("rule").detail, "rule");
    /// ```
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// The same member with the address of its own page.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, Member};
    /// let m = Member::new("a", Claim::Declared).href("/x");
    /// assert_eq!(m.href.as_deref(), Some("/x"));
    /// ```
    pub fn href(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }

    /// The same member with a level of its own under it.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, Member};
    /// let m = Member::new("fund", Claim::Declared)
    ///     .members(vec![Member::new("holding", Claim::Declared)]);
    /// assert_eq!(m.members[0].label, "holding");
    /// ```
    pub fn members(mut self, members: Vec<Member>) -> Self {
        self.members = members;
        self
    }
}

/// One box of the drawing: a thing, or a group of things that share a relation, with the
/// claim the figure makes about it, what its information box says when it is chosen, and
/// the members that open under the figure when it stands for more than one.
///
/// ```
/// use majordomus_cli::cockpit::figure::{Claim, FlowNode, Member};
///
/// let n = FlowNode::new("rpp", "RPP Holding", Claim::Declared)
///     .detail("100 % of the company")
///     .note("The registry names it the sole shareholder.")
///     .members(vec![Member::new("Na Jelenách a.s.", Claim::Declared)]);
/// assert_eq!(n.key, "rpp");
/// assert_eq!(n.members.len(), 1);
/// assert!(!n.focus);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowNode {
    /// The key edges name it by; unique within the figure.
    pub key: String,
    /// The first line of the box.
    pub label: String,
    /// The second line of the box.
    pub detail: String,
    /// How sure the figure is of the box.
    pub claim: Claim,
    /// What the information box says about it when it is chosen.
    pub note: String,
    /// Where its own page is.
    pub href: Option<String>,
    /// What opens under the figure when the box is chosen.
    pub members: Vec<Member>,
    /// The subject of the figure: drawn in the accent, and there is at most one.
    pub focus: bool,
}

impl FlowNode {
    /// A box with a key, a label and a claim.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, FlowNode};
    /// assert_eq!(FlowNode::new("k", "Label", Claim::Unknown).label, "Label");
    /// ```
    pub fn new(key: impl Into<String>, label: impl Into<String>, claim: Claim) -> Self {
        FlowNode {
            key: key.into(),
            label: label.into(),
            detail: String::new(),
            claim,
            note: String::new(),
            href: None,
            members: Vec::new(),
            focus: false,
        }
    }

    /// The same box with a second line.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, FlowNode};
    /// assert_eq!(FlowNode::new("k", "L", Claim::Declared).detail("d").detail, "d");
    /// ```
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// The same box with what its information box says.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, FlowNode};
    /// assert_eq!(FlowNode::new("k", "L", Claim::Declared).note("n").note, "n");
    /// ```
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }

    /// The same box with the address of its own page.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, FlowNode};
    /// let n = FlowNode::new("k", "L", Claim::Declared).href("/x");
    /// assert_eq!(n.href.as_deref(), Some("/x"));
    /// ```
    pub fn href(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }

    /// The same box with a subtree.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, FlowNode, Member};
    /// let n = FlowNode::new("k", "L", Claim::Declared)
    ///     .members(vec![Member::new("m", Claim::Declared)]);
    /// assert_eq!(n.members[0].label, "m");
    /// ```
    pub fn members(mut self, members: Vec<Member>) -> Self {
        self.members = members;
        self
    }

    /// The same box as the subject of the figure.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, FlowNode};
    /// assert!(FlowNode::new("k", "L", Claim::Declared).focused().focus);
    /// ```
    pub fn focused(mut self) -> Self {
        self.focus = true;
        self
    }
}

/// One column of boxes, left to right in the order the figure reads.
///
/// ```
/// use majordomus_cli::cockpit::figure::{Claim, Column, FlowNode};
/// let c = Column::new("Owners", vec![FlowNode::new("a", "A", Claim::Declared)]);
/// assert_eq!(c.heading, "Owners");
/// assert_eq!(c.nodes.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// The words over the column.
    pub heading: String,
    /// Its boxes, top to bottom.
    pub nodes: Vec<FlowNode>,
}

impl Column {
    /// A column with a heading and its boxes.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::Column;
    /// assert!(Column::new("Empty", Vec::new()).nodes.is_empty());
    /// ```
    pub fn new(heading: impl Into<String>, nodes: Vec<FlowNode>) -> Self {
        Column {
            heading: heading.into(),
            nodes,
        }
    }
}

/// One line of the drawing, from one box to another.
///
/// ```
/// use majordomus_cli::cockpit::figure::{Claim, FlowEdge};
/// let e = FlowEdge::new("a", "b", "owns", Claim::Declared).note("Registry entry of 2024.");
/// assert_eq!((e.from.as_str(), e.to.as_str()), ("a", "b"));
/// assert_eq!(e.note, "Registry entry of 2024.");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowEdge {
    /// The key of the box it starts at.
    pub from: String,
    /// The key of the box it ends at.
    pub to: String,
    /// The words on the line.
    pub label: String,
    /// How sure the figure is of it; also its dash.
    pub claim: Claim,
    /// What the information box says about it when it is chosen.
    pub note: String,
}

impl FlowEdge {
    /// A line from one box to another, with its words and its claim.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, FlowEdge};
    /// assert_eq!(FlowEdge::new("a", "b", "x", Claim::Derived).claim, Claim::Derived);
    /// ```
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        label: impl Into<String>,
        claim: Claim,
    ) -> Self {
        FlowEdge {
            from: from.into(),
            to: to.into(),
            label: label.into(),
            claim,
            note: String::new(),
        }
    }

    /// The same line with what its information box says.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, FlowEdge};
    /// assert_eq!(FlowEdge::new("a", "b", "x", Claim::Declared).note("n").note, "n");
    /// ```
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }
}

/// A whole figure: the question, the columns and lines, the caption, and the data the
/// drawing is a view of.
///
/// ```
/// use majordomus_cli::cockpit::figure::{Claim, Column, Flow, FlowEdge, FlowNode};
///
/// let flow = Flow {
///     id: "who".into(),
///     question: "Who holds the land?".into(),
///     caption: "The register is read; the restriction is inferred.".into(),
///     columns: vec![
///         Column::new("Owner", vec![FlowNode::new("o", "Two people", Claim::Declared)]),
///         Column::new("Beneficiary", vec![FlowNode::new("b", "The investor", Claim::Declared)]),
///     ],
///     edges: vec![FlowEdge::new("o", "b", "may not sell", Claim::Derived)],
///     data: None,
/// };
/// let html = flow.render().render();
/// // with no table handed over, the figure lists its lines as one
/// assert!(html.contains(">may not sell</td>"));
/// ```
#[derive(Debug, Clone)]
pub struct Flow {
    /// The figure's identity on the page; every id inside it is prefixed with it.
    pub id: String,
    /// The question the figure answers, shown over it.
    pub question: String,
    /// What on it is recorded and what is inferred, shown under it.
    pub caption: String,
    /// The boxes, by column.
    pub columns: Vec<Column>,
    /// The lines.
    pub edges: Vec<FlowEdge>,
    /// The data under the drawing. `None` lists every line as a table, which is the least
    /// a figure owes; a page that already has a richer table hands it over instead.
    pub data: Option<El>,
}

// The geometry, in SVG user units. The drawing scales to its frame and scrolls in it on a
// narrow screen rather than shrinking its words below what can be read.
const BOX_W: u32 = 232;
const BOX_H: u32 = 58;
const COL_GAP: u32 = 200;
const ROW_GAP: u32 = 18;
const PAD: u32 = 12;
const HEAD: u32 = 26;
/// Characters that fit on a box's first line at the label size, and on its second line.
const LABEL_FIT: usize = 24;
/// What is left of the first line when the box also carries the count of its members.
const LABEL_FIT_WITH_COUNT: usize = 20;
const DETAIL_FIT: usize = 34;
/// The width of one character of a line's words, at the label size, in user units.
const EDGE_CHAR: u32 = 7;

/// Where one box landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placed {
    column: usize,
    x: u32,
    y: u32,
}

impl Flow {
    /// The figure as markup: head, drawing, information box, legend, subtrees, caption and
    /// data, in that order. A line that names a box the figure does not hold is left out of
    /// the drawing and kept in the data, where it says what it names.
    ///
    /// ```
    /// use majordomus_cli::cockpit::figure::{Claim, Column, Flow, FlowEdge, FlowNode, Member};
    ///
    /// let flow = Flow {
    ///     id: "f".into(),
    ///     question: "What does it name?".into(),
    ///     caption: "Recorded in its front matter.".into(),
    ///     columns: vec![
    ///         Column::new("This", vec![FlowNode::new("me", "me", Claim::Declared).focused()]),
    ///         Column::new("Names", vec![FlowNode::new("deps", "depends_on", Claim::Declared)
    ///             .members(vec![Member::new("project.x@1", Claim::Declared)])]),
    ///     ],
    ///     edges: vec![FlowEdge::new("me", "deps", "depends_on", Claim::Declared)],
    ///     data: None,
    /// };
    /// let html = flow.render().render();
    /// // every box is a control a keyboard reaches
    /// assert!(html.contains(r#"data-k="me" data-claim="declared" tabindex="0" role="button""#));
    /// // a box with members opens a native subtree, which needs no script
    /// assert!(html.contains(r#"<details class="mj-subtree" id="f-sub-deps""#));
    /// // and the data is under the drawing
    /// assert!(html.contains("mj-figure-data"));
    /// ```
    pub fn render(&self) -> El {
        let placed = self.place();
        let (width, height) = self.size();
        let arrow = format!("{}-arrow", self.id);

        let mut svg = el("svg")
            .class("mj-flow")
            .attr("viewBox", format!("0 0 {width} {height}"))
            .attr("width", width.to_string())
            .attr("role", "group")
            .attr("aria-label", &self.question)
            .child(
                el("defs").child(
                    el("marker")
                        .attr("id", &arrow)
                        .attr("viewBox", "0 0 10 10")
                        .attr("refX", "9")
                        .attr("refY", "5")
                        .attr("markerWidth", "7")
                        .attr("markerHeight", "7")
                        .attr("orient", "auto")
                        .child(
                            el("path")
                                .class("mj-flow-arrow")
                                .attr("d", "M0,0 L10,5 L0,10 z"),
                        ),
                ),
            );
        for (c, column) in self.columns.iter().enumerate() {
            svg = svg.child(
                el("text")
                    .class("mj-flow-heading")
                    .attr("x", (PAD + c as u32 * (BOX_W + COL_GAP)).to_string())
                    .attr("y", (PAD + 12).to_string())
                    .text(&column.heading),
            );
        }
        let mut notes = Vec::new();
        for (i, edge) in self.edges.iter().enumerate() {
            let (Some(a), Some(b)) = (placed_of(&placed, &edge.from), placed_of(&placed, &edge.to))
            else {
                continue;
            };
            let key = format!("e{i}");
            // a line into a box other lines also end at shares its last run with them, so
            // its words go on its first run, which is its own
            let converging = self
                .edges
                .iter()
                .filter(|e| e.to == edge.to && placed_of(&placed, &e.from).is_some())
                .count()
                > 1;
            svg = svg.child(self.edge(&key, edge, a, b, converging, &arrow));
            notes.push(note(
                &key,
                &format!(
                    "{} → {}",
                    self.label_of(&edge.from),
                    self.label_of(&edge.to)
                ),
                &edge.label,
                edge.claim,
                &edge.note,
                None,
            ));
        }
        // `place` lists every box once, in the order the columns hold them
        let boxes = self.columns.iter().flat_map(|c| c.nodes.iter());
        for (n, (_, p)) in boxes.zip(placed.iter()) {
            svg = svg.child(self.node(n, *p));
            notes.push(note(
                &n.key,
                &n.label,
                &n.detail,
                n.claim,
                &n.note,
                n.href.as_deref(),
            ));
        }

        let subtrees: Vec<El> = self
            .columns
            .iter()
            .flat_map(|c| c.nodes.iter())
            .filter(|n| !n.members.is_empty())
            .map(|n| self.subtree(n))
            .collect();
        let has_subtrees = !subtrees.is_empty();

        el("figure")
            .class("mj-figure")
            .attr("id", &self.id)
            .attr("data-mj-figure", &self.id)
            .child(el("div").class("mj-figure-head").text(&self.question))
            .child(
                el("div")
                    .class("mj-figure-tools")
                    .attr("data-mj-js", "")
                    .flag("hidden")
                    .when(has_subtrees, |t| {
                        t.child(
                            el("button")
                                .class("mj-button mj-button--quiet")
                                .attr("type", "button")
                                .attr("data-mj-figure-all", "open")
                                .text("Open every subtree"),
                        )
                        .child(
                            el("button")
                                .class("mj-button mj-button--quiet")
                                .attr("type", "button")
                                .attr("data-mj-figure-all", "close")
                                .text("Close every subtree"),
                        )
                    })
                    .child(el("span").class("mj-figure-hint").text(
                        "Choose a box or a line: what it is, how sure the figure is of it, and where it came from appear below.",
                    )),
            )
            .child(el("div").class("mj-figure-canvas").child(svg))
            .child(
                el("div")
                    .class("mj-figure-info")
                    .attr("data-mj-figure-info", "")
                    .attr("aria-live", "polite")
                    .attr("data-mj-js", "")
                    .flag("hidden")
                    .child(el("p").class("mj-figure-hint").text("Nothing is chosen.")),
            )
            .children(notes)
            .child(self.legend())
            .when(has_subtrees, |f| {
                f.child(el("div").class("mj-figure-subtrees").children(subtrees))
            })
            .child(el("figcaption").class("mj-figure-caption").text(&self.caption))
            .child(
                el("details")
                    .class("mj-figure-data")
                    .flag("open")
                    .child(el("summary").text("The data and its sources"))
                    .child(match &self.data {
                        Some(d) => d.clone(),
                        None => self.edge_table(),
                    }),
            )
    }

    /// Every box's place: columns left to right, each column centred on the tallest.
    fn place(&self) -> Vec<(String, Placed)> {
        let tallest = self
            .columns
            .iter()
            .map(|c| c.nodes.len())
            .max()
            .unwrap_or(0) as u32;
        let step = BOX_H + ROW_GAP;
        let mut out = Vec::new();
        for (c, column) in self.columns.iter().enumerate() {
            let offset = (tallest - column.nodes.len() as u32) * step / 2;
            for (r, n) in column.nodes.iter().enumerate() {
                out.push((
                    n.key.clone(),
                    Placed {
                        column: c,
                        x: PAD + c as u32 * (BOX_W + COL_GAP),
                        y: PAD + HEAD + offset + r as u32 * step,
                    },
                ));
            }
        }
        out
    }

    fn size(&self) -> (u32, u32) {
        let columns = self.columns.len().max(1) as u32;
        let tallest = self
            .columns
            .iter()
            .map(|c| c.nodes.len())
            .max()
            .unwrap_or(0)
            .max(1) as u32;
        (
            PAD * 2 + columns * BOX_W + (columns - 1) * COL_GAP,
            PAD * 2 + HEAD + tallest * BOX_H + (tallest - 1) * ROW_GAP,
        )
    }

    fn label_of(&self, key: &str) -> String {
        self.columns
            .iter()
            .flat_map(|c| c.nodes.iter())
            .find(|n| n.key == key)
            .map(|n| n.label.clone())
            .unwrap_or_else(|| key.to_string())
    }

    fn node(&self, n: &FlowNode, p: Placed) -> El {
        let (x, y) = (p.x, p.y);
        let mut g = el("g")
            .class(format!(
                "mj-flow-node mj-status--{}{}",
                n.claim.word(),
                if n.focus { " mj-flow-node--focus" } else { "" }
            ))
            .attr("data-k", &n.key)
            .attr("data-claim", n.claim.word())
            .attr("tabindex", "0")
            .attr("role", "button")
            .attr("aria-pressed", "false")
            .attr(
                "aria-label",
                format!("{}: {} ({})", n.label, n.detail, n.claim.word()),
            )
            .child(el("title").text(format!("{} — {}", n.label, n.claim.word())))
            .child(
                el("rect")
                    .class("mj-flow-box")
                    .attr("x", x.to_string())
                    .attr("y", y.to_string())
                    .attr("width", BOX_W.to_string())
                    .attr("height", BOX_H.to_string())
                    .attr("rx", "6")
                    .attr_if("stroke-dasharray", n.claim.dash()),
            )
            .child(
                el("rect")
                    .class("mj-flow-stripe")
                    .attr("x", x.to_string())
                    .attr("y", y.to_string())
                    .attr("width", "4")
                    .attr("height", BOX_H.to_string()),
            )
            .child(
                el("text")
                    .class("mj-flow-label")
                    .attr("x", (x + 14).to_string())
                    .attr("y", (y + 24).to_string())
                    .text(fit(
                        &n.label,
                        if n.members.is_empty() {
                            LABEL_FIT
                        } else {
                            LABEL_FIT_WITH_COUNT
                        },
                    )),
            )
            .child(
                el("text")
                    .class("mj-flow-detail")
                    .attr("x", (x + 14).to_string())
                    .attr("y", (y + 43).to_string())
                    .text(fit(&n.detail, DETAIL_FIT)),
            );
        if !n.members.is_empty() {
            let count = n.members.len();
            g = g.child(
                el("g")
                    .class("mj-flow-chip")
                    .attr("data-mj-sub", &n.key)
                    .attr("aria-hidden", "true")
                    .child(
                        el("rect")
                            .attr("x", (x + BOX_W - 36).to_string())
                            .attr("y", (y + 8).to_string())
                            .attr("width", "28")
                            .attr("height", "18")
                            .attr("rx", "9"),
                    )
                    .child(
                        el("text")
                            .attr("x", (x + BOX_W - 22).to_string())
                            .attr("y", (y + 21).to_string())
                            .attr("text-anchor", "middle")
                            .text(count.to_string()),
                    ),
            );
        }
        g
    }

    fn edge(
        &self,
        key: &str,
        e: &FlowEdge,
        a: Placed,
        b: Placed,
        converging: bool,
        arrow: &str,
    ) -> El {
        let mid_y = |p: Placed| p.y + BOX_H / 2;
        // where the words go, and how many characters the run they sit on has room for
        let (d, label_at, room) = if a.column == b.column {
            let x = a.x + BOX_W / 2;
            let (top, bottom) = if a.y < b.y {
                (a.y + BOX_H, b.y - 2)
            } else {
                (a.y, b.y + BOX_H + 2)
            };
            (
                format!("M{x},{top} V{bottom}"),
                (x + 6, (top + bottom) / 2),
                LABEL_FIT,
            )
        } else {
            let forward = a.column < b.column;
            let x1 = if forward { a.x + BOX_W } else { a.x };
            let x2 = if forward { b.x - 2 } else { b.x + BOX_W + 2 };
            let (y1, y2) = (mid_y(a), mid_y(b));
            let mid = (x1 + x2) / 2;
            let d = if y1 == y2 {
                format!("M{x1},{y1} H{x2}")
            } else {
                format!("M{x1},{y1} H{mid} V{y2} H{x2}")
            };
            // the words sit on the last run, which no line into a different box shares,
            // unless other lines end in the same box; then on the first, which is this
            // line's alone
            let (start, end, y) = match (forward, converging) {
                (true, false) => (mid, x2, y2),
                (true, true) => (x1, mid, y1),
                (false, false) => (x2, mid, y2),
                (false, true) => (mid, x1, y1),
            };
            let room = (end.saturating_sub(start + 12) / EDGE_CHAR) as usize;
            (d, (start + 6, y - 6), room.max(4))
        };
        el("g")
            .class(format!("mj-flow-edge mj-status--{}", e.claim.word()))
            .attr("data-k", key)
            .attr("data-claim", e.claim.word())
            .attr("data-from", &e.from)
            .attr("data-to", &e.to)
            .attr("tabindex", "0")
            .attr("role", "button")
            .attr("aria-pressed", "false")
            .attr(
                "aria-label",
                format!(
                    "{} → {}: {} ({})",
                    self.label_of(&e.from),
                    self.label_of(&e.to),
                    e.label,
                    e.claim.word()
                ),
            )
            .child(el("title").text(format!("{} — {}", e.label, e.claim.word())))
            .child(
                el("path")
                    .class("mj-flow-line")
                    .attr("d", &d)
                    .attr("marker-end", format!("url(#{arrow})"))
                    .attr_if("stroke-dasharray", e.claim.dash()),
            )
            .child(el("path").class("mj-flow-hit").attr("d", &d))
            .child(
                el("text")
                    .class("mj-flow-edge-label")
                    .attr("x", label_at.0.to_string())
                    .attr("y", label_at.1.to_string())
                    .text(fit(&e.label, room)),
            )
    }

    fn legend(&self) -> El {
        let used: std::collections::BTreeSet<Claim> = self
            .edges
            .iter()
            .map(|e| e.claim)
            .chain(
                self.columns
                    .iter()
                    .flat_map(|c| c.nodes.iter().map(|n| n.claim)),
            )
            .collect();
        el("ul")
            .class("mj-figure-legend")
            .children(used.into_iter().map(|claim| {
                // the switch is a span inside the item: a script that gives the item
                // itself a button's role takes it out of the list it belongs to
                el("li")
                    .class(format!("mj-figure-key mj-status--{}", claim.word()))
                    .child(
                        el("span")
                            .class("mj-figure-switch")
                            .attr("data-mj-claim", claim.word())
                            .child(
                                el("svg")
                                    .attr("viewBox", "0 0 28 8")
                                    .attr("width", "28")
                                    .attr("height", "8")
                                    .attr("aria-hidden", "true")
                                    .child(
                                        el("path")
                                            .class("mj-flow-line")
                                            .attr("d", "M0,4 H28")
                                            .attr_if("stroke-dasharray", claim.dash()),
                                    ),
                            )
                            .child(badge(claim.word(), claim.word()))
                            .child(el("span").class("mj-figure-meaning").text(claim.meaning())),
                    )
            }))
    }

    fn subtree(&self, n: &FlowNode) -> El {
        el("details")
            .class("mj-subtree")
            .attr("id", format!("{}-sub-{}", self.id, n.key))
            .attr("data-mj-sub", &n.key)
            .child(
                el("summary")
                    .child(el("span").class("mj-subtree-title").text(&n.label))
                    .child(el("span").class("mj-subtree-count").text(format!(
                        "{} {}",
                        n.members.len(),
                        if n.members.len() == 1 {
                            "member"
                        } else {
                            "members"
                        }
                    ))),
            )
            .child(tree(&n.members))
    }

    fn edge_table(&self) -> El {
        table(
            &["From", "Line", "To", "Claim"],
            self.edges
                .iter()
                .map(|e| {
                    row(vec![
                        text_cell(self.label_of(&e.from)),
                        text_cell(&e.label),
                        text_cell(self.label_of(&e.to)),
                        el("td").child(badge(e.claim.word(), e.claim.word())),
                    ])
                })
                .collect(),
        )
    }
}

fn placed_of(placed: &[(String, Placed)], key: &str) -> Option<Placed> {
    placed.iter().find(|(k, _)| k == key).map(|(_, p)| *p)
}

/// What the information box shows for one element, kept inert in a `<template>` until it
/// is chosen. A template is markup the browser parses and never renders, so a reader with
/// no script loses nothing the data table does not also say.
fn note(key: &str, title: &str, detail: &str, claim: Claim, body: &str, href: Option<&str>) -> El {
    el("template").attr("data-mj-note", key).child(
        el("div")
            .class("mj-figure-note")
            .child(
                el("p")
                    .class("mj-figure-note-title")
                    .child(el("strong").text(title))
                    .text(" ")
                    .child(badge(claim.word(), claim.word())),
            )
            .when(!detail.is_empty(), |d| {
                d.child(el("p").class("mj-figure-note-detail").text(detail))
            })
            .when(!body.is_empty(), |d| d.child(el("p").text(body)))
            .child(el("p").class("mj-figure-meaning").text(format!(
                "{}: {}.",
                claim.word(),
                claim.meaning()
            )))
            .when(href.is_some(), |d| {
                d.child(el("p").child(link(href.unwrap_or_default(), "Open its page")))
            }),
    )
}

/// A level of a subtree. A member with members of its own opens one level at a time.
fn tree(members: &[Member]) -> El {
    el("ul").class("mj-tree").children(members.iter().map(|m| {
        let label = match &m.href {
            Some(h) => link(h, &m.label).class("mj-tree-label"),
            None => el("span").class("mj-tree-label").text(&m.label),
        };
        let row = el("span")
            .class("mj-tree-row")
            .child(label)
            .when(!m.detail.is_empty(), |r| {
                r.child(el("span").class("mj-tree-detail").text(&m.detail))
            })
            .child(badge(m.claim.word(), m.claim.word()));
        if m.members.is_empty() {
            el("li").child(row)
        } else {
            el("li").child(
                el("details")
                    .class("mj-tree-branch")
                    .child(el("summary").child(row))
                    .child(tree(&m.members)),
            )
        }
    }))
}

/// A line cut to what fits, with the cut said; the whole line is in the element's title
/// and its information box.
fn fit(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_string();
    }
    let kept: String = text.chars().take(room.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_columns(edges: Vec<FlowEdge>) -> Flow {
        Flow {
            id: "t".into(),
            question: "Q?".into(),
            caption: "C.".into(),
            columns: vec![
                Column::new(
                    "Left",
                    vec![
                        FlowNode::new("a", "Alpha", Claim::Declared),
                        FlowNode::new("b", "Beta", Claim::Derived),
                    ],
                ),
                Column::new(
                    "Right",
                    vec![FlowNode::new("c", "Gamma", Claim::Declared)
                        .focused()
                        .members(vec![Member::new("m1", Claim::Declared)
                            .href("/m1")
                            .detail("rule")
                            .members(vec![Member::new("m2", Claim::Missing)])])],
                ),
            ],
            edges,
            data: None,
        }
    }

    #[test]
    fn the_same_answer_draws_the_same_figure() {
        let f = two_columns(vec![FlowEdge::new("a", "c", "owns", Claim::Declared)]);
        assert_eq!(f.render().render(), f.render().render());
    }

    #[test]
    fn a_column_is_centred_on_the_tallest() {
        let f = two_columns(Vec::new());
        let placed = f.place();
        let a = placed_of(&placed, "a").unwrap();
        let b = placed_of(&placed, "b").unwrap();
        let c = placed_of(&placed, "c").unwrap();
        assert_eq!(c.y, (a.y + b.y) / 2);
        assert_eq!(c.x, PAD + BOX_W + COL_GAP);
        assert_eq!(
            f.size(),
            (
                PAD * 2 + 2 * BOX_W + COL_GAP,
                PAD * 2 + HEAD + 2 * BOX_H + ROW_GAP
            )
        );
    }

    #[test]
    fn a_line_runs_from_the_side_that_faces_its_target() {
        let f = two_columns(vec![
            FlowEdge::new("a", "c", "forward", Claim::Declared),
            FlowEdge::new("c", "b", "back", Claim::Derived),
            FlowEdge::new("a", "b", "down", Claim::Estimated),
            FlowEdge::new("b", "a", "up", Claim::Unknown),
        ]);
        let html = f.render().render();
        // forward: from a's right side, elbowed into c's left side
        assert!(html.contains(&format!("M{},", PAD + BOX_W)));
        // backward: from c's left side into b's right side
        assert!(html.contains(&format!(" H{}\"", PAD + BOX_W + 2)));
        // within a column: a vertical run
        assert!(html.contains(" V"));
        // every claim is drawn with its dash, and named in words
        assert!(html.contains(r#"stroke-dasharray="6 4""#));
        assert!(html.contains(r#"stroke-dasharray="8 3 2 3""#));
        assert!(html.contains("mj-status--unknown"));
    }

    #[test]
    fn lines_that_meet_in_one_box_carry_their_words_on_their_own_runs() {
        let f = two_columns(vec![
            FlowEdge::new("a", "c", "first_relation", Claim::Declared),
            FlowEdge::new("b", "c", "second_relation", Claim::Derived),
        ]);
        let html = f.render().render();
        // both lines end in c, so each label starts just after its own source's side
        let x = PAD + BOX_W + 6;
        assert_eq!(
            html.matches(&format!(r#"class="mj-flow-edge-label" x="{x}""#))
                .count(),
            2
        );
        // and is cut to the run it sits on, saying so
        assert!(html.contains("…</text>"));
    }

    #[test]
    fn a_line_back_to_a_box_alone_carries_its_words_on_its_last_run() {
        let f = two_columns(vec![FlowEdge::new("c", "a", "back", Claim::Historical)]);
        let html = f.render().render();
        // the last run of a line from c back to a ends at a's right side
        let x = PAD + BOX_W + 2 + 6;
        assert!(
            html.contains(&format!(r#"class="mj-flow-edge-label" x="{x}""#)),
            "{html}"
        );
        // history is dotted and named
        assert!(html.contains(r#"stroke-dasharray="1 5""#));
        assert!(html.contains(r#"data-mj-claim="historical""#));
    }

    #[test]
    fn a_subtree_counts_its_members_in_words() {
        let f = Flow {
            id: "m".into(),
            question: "Q".into(),
            caption: "C".into(),
            columns: vec![Column::new(
                "Only",
                vec![FlowNode::new("a", "A", Claim::Declared).members(vec![
                    Member::new("one", Claim::Declared),
                    Member::new("two", Claim::Estimated),
                ])],
            )],
            edges: Vec::new(),
            data: None,
        };
        let html = f.render().render();
        assert!(html.contains("2 members"));
        assert!(html.contains(r#"class="mj-badge mj-badge--estimated""#));
    }

    #[test]
    fn a_line_to_a_box_that_is_not_there_is_kept_in_the_data_only() {
        let f = two_columns(vec![FlowEdge::new("a", "nowhere", "lost", Claim::Missing)]);
        let html = f.render().render();
        assert!(!html.contains(r#"data-k="e0""#));
        // the data still says what the line names
        assert!(html.contains("<td>nowhere</td>"));
    }

    #[test]
    fn every_element_has_an_explanation_and_the_legend_names_only_what_is_drawn() {
        let f = two_columns(vec![
            FlowEdge::new("a", "c", "owns", Claim::Declared).note("why")
        ]);
        let html = f.render().render();
        for key in ["a", "b", "c", "e0"] {
            assert!(
                html.contains(&format!(r#"<template data-mj-note="{key}">"#)),
                "{key}"
            );
        }
        assert!(html.contains("<p>why</p>"));
        assert!(html.contains(r#"href="/m1""#));
        assert!(html.contains("mj-figure-key mj-status--declared"));
        assert!(html.contains("mj-figure-key mj-status--derived"));
        assert!(!html.contains("mj-figure-key mj-status--estimated"));
    }

    #[test]
    fn a_subtree_opens_one_level_at_a_time_and_a_page_may_hand_over_its_data() {
        let mut f = two_columns(Vec::new());
        let html = f.render().render();
        assert!(html.contains(r#"<details class="mj-subtree" id="t-sub-c" data-mj-sub="c">"#));
        assert!(html.contains(r#"<details class="mj-tree-branch">"#));
        assert!(html.contains("1 member"));
        assert!(html.contains(r#"data-mj-sub="c" aria-hidden="true""#));
        assert!(html.contains("mj-flow-node--focus"));
        f.data = Some(el("p").text("the page's own table"));
        let html = f.render().render();
        assert!(html.contains("the page&#39;s own table") || html.contains("the page's own table"));
        assert!(!html.contains(">From</th>"));
    }

    #[test]
    fn a_figure_without_subtrees_offers_no_control_for_them() {
        let f = Flow {
            id: "x".into(),
            question: "Q".into(),
            caption: "C".into(),
            columns: vec![Column::new(
                "Only",
                vec![FlowNode::new("a", "A", Claim::Declared)],
            )],
            edges: Vec::new(),
            data: None,
        };
        let html = f.render().render();
        assert!(!html.contains("data-mj-figure-all"));
        assert!(!html.contains("mj-figure-subtrees"));
    }

    #[test]
    fn an_empty_figure_still_has_a_size() {
        let f = Flow {
            id: "e".into(),
            question: "Q".into(),
            caption: "C".into(),
            columns: Vec::new(),
            edges: Vec::new(),
            data: None,
        };
        assert_eq!(f.size(), (PAD * 2 + BOX_W, PAD * 2 + HEAD + BOX_H));
        assert!(f.render().render().contains("<svg"));
    }

    #[test]
    fn a_long_line_is_cut_and_says_so() {
        assert_eq!(fit("short", 10), "short");
        assert_eq!(fit("abcdefghijkl", 6), "abcde…");
        assert_eq!(fit("abc  defgh", 6), "abc…");
        for claim in Claim::ALL {
            assert!(!claim.meaning().is_empty());
            assert!(!claim.word().is_empty());
        }
    }
}
