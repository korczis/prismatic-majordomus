//! Diagrams, canonically: nodes, edges and groups in Rust, derived from whatever already
//! states the topology, and rendered to Mermaid by a function that is one reader among
//! several.
//!
//! # The authority this takes away
//!
//! Before this module the model *was* the Mermaid. `site/data/generated/diagrams.json`
//! carried six records shaped `{mermaid, source, title}`, and `mermaid` was a source string
//! composed by `printf` in `scripts/generate-site-data`:
//!
//! ```text
//!   lib/finish.sh ── grep ──> "completed|partial|…" ── printf ──> "stateDiagram-v2\n …"
//!                                                                          │
//!                                                            the only statement of structure
//! ```
//!
//! Nothing downstream could read that. A second rendering of the same lifecycle — a
//! Graphviz export, a Cockpit panel, a check asking whether every outcome the tool accepts
//! is actually drawn — had to re-type the topology, and a diagram the repository publishes
//! is then an independent claim about the system rather than a projection of it. That is
//! exactly what `project.a-diagram-is-a-projection` forbids everywhere else.
//!
//! So the canonical form is [`Diagram`], and Mermaid is [`Diagram::to_mermaid`]: one
//! renderer, replaceable, with no privilege over any other.
//!
//! ```
//! use majordomus_cli::diagram::{Diagram, Edge, Kind, Node, Provenance};
//!
//! let drawn = Diagram::new("greeting", "Two states", Kind::State,
//!         Provenance::new("lib/finish.sh", "the outcome vocabulary"))
//!     .with_node(Node::new("active", "active"))
//!     .with_node(Node::new("done", "done"))
//!     .with_edge(Edge::new("active", "done").labelled("finish"));
//!
//! assert_eq!(drawn.to_mermaid(), "stateDiagram-v2\n    active --> done: finish");
//! assert!(drawn.validate().is_empty(), "a well-formed model has nothing to report");
//! ```
//!
//! # Why the order is the derivation's and not the ids'
//!
//! [`crate::graph`] sorts its nodes, because a graph is a set and two runs must agree. A
//! diagram is a *picture*, and Mermaid lays one out in declaration order: sorting would make
//! the drawing a function of how the nodes happen to be named. So a [`Diagram`] keeps the
//! order its derivation built it in, and determinism is the derivation's obligation — the
//! same source text builds the same model, and the same model renders to the same bytes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The shape every projection of a diagram declares itself to be, so that a record written
/// today can still be read when the fields below change.
pub const DIAGRAM_SCHEMA: &str = "majordomus/diagram/v1";

/// What kind of thing the diagram draws, which is also what Mermaid dialect renders it.
///
/// The two are one choice rather than two, because a state machine drawn as a flowchart
/// loses its start and end and a flow drawn as a state machine cannot carry a subgraph:
///
/// ```
/// use majordomus_cli::diagram::Kind;
/// assert_eq!(Kind::State.as_str(), "state");
/// assert_eq!(Kind::Flow.as_str(), "flow");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DiagramKind")]
pub enum Kind {
    /// Boxes and arrows: something moves from one place to another.
    Flow,
    /// A state machine: a thing is in exactly one of these at a time.
    State,
}

impl Kind {
    /// The word every surface writes for this kind — the JSON projection, a report, a
    /// filter — so that no caller invents a second spelling.
    ///
    /// ```
    /// use majordomus_cli::diagram::Kind;
    /// assert_eq!(Kind::Flow.as_str(), "flow");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Flow => "flow",
            Kind::State => "state",
        }
    }
}

/// Which way a flowchart runs. Ignored by [`Kind::State`], which Mermaid always lays out
/// top-down.
///
/// ```
/// use majordomus_cli::diagram::Orientation;
/// assert_eq!(Orientation::LeftRight.as_str(), "LR", "the token Mermaid itself reads");
/// assert_eq!(Orientation::default(), Orientation::TopDown);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DiagramOrientation")]
pub enum Orientation {
    /// Top to bottom: the default, and what a hierarchy wants.
    #[default]
    TopDown,
    /// Left to right: what a pipeline wants.
    LeftRight,
}

impl Orientation {
    /// The token Mermaid reads after `flowchart`.
    ///
    /// ```
    /// use majordomus_cli::diagram::Orientation;
    /// assert_eq!(Orientation::TopDown.as_str(), "TD");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Orientation::TopDown => "TD",
            Orientation::LeftRight => "LR",
        }
    }
}

/// What a node is drawn as. A shape is meaning, not decoration: a [`Shape::Cylinder`] says
/// the thing is stored, a [`Shape::Terminal`] says the diagram begins or ends there.
///
/// ```
/// use majordomus_cli::diagram::Shape;
/// assert_eq!(Shape::default(), Shape::Box, "an unremarkable thing is a box");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DiagramShape")]
pub enum Shape {
    /// A plain rectangle: a step, a component, a file.
    #[default]
    Box,
    /// A rounded rectangle: something softer than a step — a note, a context.
    Rounded,
    /// A stadium: an entry point a person reaches for.
    Stadium,
    /// A cylinder: data at rest.
    Cylinder,
    /// The start or the end. Rendered `[*]` in a state diagram, where Mermaid decides from
    /// the arrow's direction which of the two it is, and as a circle in a flowchart.
    Terminal,
}

/// Whether an edge is what happens or merely what is implied.
///
/// ```
/// use majordomus_cli::diagram::EdgeStyle;
/// assert_eq!(EdgeStyle::default(), EdgeStyle::Solid);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DiagramEdgeStyle")]
pub enum EdgeStyle {
    /// It happens: control or data really moves along this arrow.
    #[default]
    Solid,
    /// It is implied: a stamp, an observation, a relationship that is not a transfer.
    Dotted,
}

/// One node: a state, a step, a store. `id` is unique within the diagram and is what an
/// [`Edge`] names; `label` is what a reader sees. The two are separate because a label is
/// prose that may hold spaces, punctuation and a line break, and an id is a token a
/// renderer writes into its own syntax:
///
/// ```
/// use majordomus_cli::diagram::{Diagram, Kind, Node, Provenance};
/// let node = Node::new("handed_over", "handed over to the next session");
/// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b")).with_node(node);
/// assert!(drawn.to_mermaid().contains(r#"handed_over["handed over to the next session"]"#));
/// assert!(drawn.validate().is_empty(), "the id is a token even though the label is not");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DiagramNode")]
pub struct Node {
    /// Unique within the diagram, and safe to write into Mermaid source unquoted.
    pub id: String,
    /// The short text drawn inside the node.
    pub label: String,
    /// What it is drawn as.
    #[serde(default)]
    pub shape: Shape,
}

impl Node {
    /// A box with this id and label — the node most derivations want.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Node, Shape};
    /// let node = Node::new("active", "active");
    /// assert_eq!(node.shape, Shape::Box);
    /// assert_eq!(node.id, "active");
    /// ```
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Node {
        Node {
            id: id.into(),
            label: label.into(),
            shape: Shape::default(),
        }
    }

    /// The same node with a different [`Shape`] — which is a statement about the thing and
    /// not about the picture: a derivation says a node is a store or a terminal, and every
    /// renderer then decides for itself how to draw that.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Node, Shape};
    /// let node = Node::new("entry", "entry").shaped(Shape::Terminal);
    /// assert_eq!(node.shape, Shape::Terminal);
    /// ```
    pub fn shaped(mut self, shape: Shape) -> Node {
        self.shape = shape;
        self
    }
}

/// One edge, from a node id to a node id, with the transition or the reason written on it.
///
/// An edge names ids and never nodes, so a derivation can state a relationship before it has
/// built both ends — and [`Diagram::validate`] is what refuses an end that never arrived,
/// rather than a renderer discovering it while writing broken syntax:
///
/// ```
/// use majordomus_cli::diagram::{Diagram, Edge, Kind, Node, Provenance};
/// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"))
///     .with_node(Node::new("a", "A"))
///     .with_node(Node::new("b", "B"))
///     .with_edge(Edge::new("a", "b").labelled("moves").dotted());
/// assert!(drawn.to_mermaid().ends_with("a -.->|moves| b"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DiagramEdge")]
pub struct Edge {
    /// The id of the node the arrow leaves.
    pub from: String,
    /// The id of the node the arrow reaches.
    pub to: String,
    /// What is written on the arrow, when anything is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Whether it happens or is implied.
    #[serde(default)]
    pub style: EdgeStyle,
}

impl Edge {
    /// A plain, unlabelled arrow between two node ids.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Edge, EdgeStyle};
    /// let edge = Edge::new("active", "failed");
    /// assert_eq!(edge.label, None);
    /// assert_eq!(edge.style, EdgeStyle::Solid);
    /// ```
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Edge {
        Edge {
            from: from.into(),
            to: to.into(),
            label: None,
            style: EdgeStyle::default(),
        }
    }

    /// The same arrow with text on it: the command, the event, the condition.
    ///
    /// ```
    /// use majordomus_cli::diagram::Edge;
    /// let edge = Edge::new("active", "failed").labelled("finish --outcome failed");
    /// assert_eq!(edge.label.as_deref(), Some("finish --outcome failed"));
    /// ```
    pub fn labelled(mut self, label: impl Into<String>) -> Edge {
        self.label = Some(label.into());
        self
    }

    /// The same arrow, implied rather than travelled.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Edge, EdgeStyle};
    /// assert_eq!(Edge::new("a", "b").dotted().style, EdgeStyle::Dotted);
    /// ```
    pub fn dotted(mut self) -> Edge {
        self.style = EdgeStyle::Dotted;
        self
    }
}

/// A box drawn around several nodes: Mermaid's `subgraph`. A flowchart concept — a state
/// machine has composite states instead, which this model does not carry, and
/// [`Diagram::validate`] says so rather than dropping the group in silence.
///
/// ```
/// use majordomus_cli::diagram::{Diagram, Group, Kind, Node, Provenance};
/// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"))
///     .with_node(Node::new("a", "A"))
///     .with_node(Node::new("loose", "Loose"))
///     .with_group(Group::new("canonical", "canonical", ["a"]));
/// let mermaid = drawn.to_mermaid();
/// assert!(mermaid.contains("    subgraph canonical [\"canonical\"]\n        a[\"A\"]\n    end"));
/// assert!(mermaid.contains("\n    loose[\"Loose\"]"), "an ungrouped node stays outside");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DiagramGroup")]
pub struct Group {
    /// Unique within the diagram, and distinct from every node id.
    pub id: String,
    /// The text drawn on the box.
    pub label: String,
    /// The ids of the nodes inside it, in the order they should be drawn.
    pub members: Vec<String>,
}

impl Group {
    /// A group over the node ids given, in that order.
    ///
    /// ```
    /// use majordomus_cli::diagram::Group;
    /// let group = Group::new("canonical", "canonical", ["a", "b"]);
    /// assert_eq!(group.members, vec!["a".to_string(), "b".to_string()]);
    /// ```
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> Group {
        Group {
            id: id.into(),
            label: label.into(),
            members: members.into_iter().map(Into::into).collect(),
        }
    }
}

/// Where the topology came from. A diagram that cannot say this is an authored claim about
/// the system, which is the thing this module exists to stop.
///
/// It is a required field of [`Diagram`] rather than an optional one for that reason: a
/// reader who doubts the picture must be able to go to the file it was read out of.
///
/// ```
/// use majordomus_cli::diagram::{lifecycle, Provenance};
/// let from = Provenance::new("lib/finish.sh", "the outcome vocabulary");
/// assert_eq!(from.derivation, "the outcome vocabulary");
/// let drawn = lifecycle("case \"$outcome\" in completed|failed) ;;").expect("derives");
/// assert_eq!(drawn.provenance, from, "the derivation records the file it read");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DiagramProvenance")]
pub struct Provenance {
    /// The repository-relative path the structure was read out of.
    pub source: String,
    /// One line saying what was read out of it, so a reader can check the derivation
    /// against the file rather than against the picture.
    pub derivation: String,
}

impl Provenance {
    /// The file and what was taken from it.
    ///
    /// ```
    /// use majordomus_cli::diagram::Provenance;
    /// let from = Provenance::new("lib/finish.sh", "the outcome vocabulary");
    /// assert_eq!(from.source, "lib/finish.sh");
    /// ```
    pub fn new(source: impl Into<String>, derivation: impl Into<String>) -> Provenance {
        Provenance {
            source: source.into(),
            derivation: derivation.into(),
        }
    }
}

/// A diagram, canonically. Every renderer is a function of this; nothing is a function of a
/// renderer's output.
///
/// The whole point is that the structure survives without the syntax: a caller can ask this
/// value a question — is every outcome the tool accepts drawn? — which a Mermaid string
/// could only be asked by re-parsing a picture.
///
/// ```
/// use majordomus_cli::diagram::{lifecycle, outcomes, Diagram};
///
/// let shell = "  case \"$outcome\" in completed|partial|failed) ;;";
/// let drawn: Diagram = lifecycle(shell).expect("the vocabulary is readable");
///
/// // the question, asked of the model rather than of the picture
/// for outcome in outcomes(shell).expect("a vocabulary") {
///     assert!(drawn.nodes.iter().any(|n| n.id == outcome), "{outcome} is not drawn");
///     assert!(drawn.edges.iter().any(|e| e.to == outcome), "nothing reaches {outcome}");
/// }
/// assert!(drawn.to_mermaid().starts_with("stateDiagram-v2"), "Mermaid is one reader of it");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "Diagram")]
pub struct Diagram {
    /// [`DIAGRAM_SCHEMA`].
    pub schema: String,
    /// Stable identity: the key the site and every other surface holds this diagram under.
    pub id: String,
    /// The heading a reader sees above the drawing.
    pub title: String,
    /// What it draws, and therefore which dialect renders it.
    pub kind: Kind,
    /// Which way a flowchart runs; ignored for [`Kind::State`].
    #[serde(default)]
    pub orientation: Orientation,
    /// What the structure was derived from.
    pub provenance: Provenance,
    /// The nodes, in the order the derivation built them, which is the order they are drawn.
    pub nodes: Vec<Node>,
    /// The edges, in derivation order.
    pub edges: Vec<Edge>,
    /// The boxes drawn around groups of nodes; empty for most diagrams.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
}

impl Diagram {
    /// An empty diagram of this kind, which a derivation then fills.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Diagram, Kind, Provenance, DIAGRAM_SCHEMA};
    /// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"));
    /// assert_eq!(drawn.schema, DIAGRAM_SCHEMA);
    /// assert!(drawn.nodes.is_empty());
    /// ```
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        kind: Kind,
        provenance: Provenance,
    ) -> Diagram {
        Diagram {
            schema: DIAGRAM_SCHEMA.to_string(),
            id: id.into(),
            title: title.into(),
            kind,
            orientation: Orientation::default(),
            provenance,
            nodes: Vec::new(),
            edges: Vec::new(),
            groups: Vec::new(),
        }
    }

    /// The same diagram laid out the other way.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Diagram, Kind, Orientation, Provenance};
    /// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"))
    ///     .oriented(Orientation::LeftRight);
    /// assert_eq!(drawn.orientation, Orientation::LeftRight);
    /// ```
    pub fn oriented(mut self, orientation: Orientation) -> Diagram {
        self.orientation = orientation;
        self
    }

    /// The same diagram with one more node, appended in drawing order.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Diagram, Kind, Node, Provenance};
    /// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"))
    ///     .with_node(Node::new("a", "A"));
    /// assert_eq!(drawn.nodes.len(), 1);
    /// ```
    pub fn with_node(mut self, node: Node) -> Diagram {
        self.nodes.push(node);
        self
    }

    /// The same diagram with one more edge, appended in drawing order.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Diagram, Edge, Kind, Node, Provenance};
    /// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"))
    ///     .with_node(Node::new("a", "A"))
    ///     .with_node(Node::new("b", "B"))
    ///     .with_edge(Edge::new("a", "b"));
    /// assert_eq!(drawn.edges.len(), 1);
    /// ```
    pub fn with_edge(mut self, edge: Edge) -> Diagram {
        self.edges.push(edge);
        self
    }

    /// The same diagram with one more group.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Diagram, Group, Kind, Node, Provenance};
    /// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"))
    ///     .with_node(Node::new("a", "A"))
    ///     .with_group(Group::new("g", "Group", ["a"]));
    /// assert_eq!(drawn.groups.len(), 1);
    /// ```
    pub fn with_group(mut self, group: Group) -> Diagram {
        self.groups.push(group);
        self
    }

    /// Everything wrong with the model, in the order it was found, as lines a report can
    /// print. Empty means the model renders to Mermaid a parser accepts.
    ///
    /// A renderer must never be the thing that discovers a dangling edge, because by then
    /// the only way to say so is broken output:
    ///
    /// ```
    /// use majordomus_cli::diagram::{Diagram, Edge, Kind, Node, Provenance};
    /// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"))
    ///     .with_node(Node::new("a", "A"))
    ///     .with_edge(Edge::new("a", "nowhere"));
    /// let found = drawn.validate();
    /// assert_eq!(found.len(), 1);
    /// assert!(found[0].contains("nowhere"), "{found:?}");
    /// ```
    pub fn validate(&self) -> Vec<String> {
        let mut found = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for node in &self.nodes {
            if node.id.is_empty() || !node.id.chars().all(is_id_char) {
                found.push(format!(
                    "node id {:?} is not a Mermaid identifier ([A-Za-z0-9_] only)",
                    node.id
                ));
            }
            if MERMAID_KEYWORDS.contains(&node.id.as_str()) {
                found.push(format!("node id {:?} is a Mermaid keyword", node.id));
            }
            if seen.contains(&node.id.as_str()) {
                found.push(format!("node id {:?} is declared twice", node.id));
            }
            seen.push(&node.id);
        }
        for edge in &self.edges {
            for end in [&edge.from, &edge.to] {
                if !seen.contains(&end.as_str()) {
                    found.push(format!(
                        "edge {:?} -> {:?} names {end:?}, which is not a node",
                        edge.from, edge.to
                    ));
                }
            }
            if edge.style == EdgeStyle::Dotted && self.kind == Kind::State {
                found.push(format!(
                    "edge {:?} -> {:?} is dotted, which a state diagram cannot draw",
                    edge.from, edge.to
                ));
            }
        }
        for group in &self.groups {
            if self.kind == Kind::State {
                found.push(format!(
                    "group {:?} is a flowchart concept; a state diagram has composite \
                     states, which this model does not carry",
                    group.id
                ));
            }
            for member in &group.members {
                if !seen.contains(&member.as_str()) {
                    found.push(format!(
                        "group {:?} names {member:?}, which is not a node",
                        group.id
                    ));
                }
            }
        }
        found
    }

    /// This diagram as Mermaid source — one renderer of the model, with no authority over
    /// it. No trailing newline: the result is a value a JSON field holds, not a file.
    ///
    /// ```
    /// use majordomus_cli::diagram::{Diagram, Edge, Kind, Node, Orientation, Provenance};
    /// let drawn = Diagram::new("x", "X", Kind::Flow, Provenance::new("a", "b"))
    ///     .oriented(Orientation::LeftRight)
    ///     .with_node(Node::new("a", "A"))
    ///     .with_node(Node::new("b", "B"))
    ///     .with_edge(Edge::new("a", "b").labelled("moves"));
    /// assert_eq!(
    ///     drawn.to_mermaid(),
    ///     "flowchart LR\n    a[\"A\"]\n    b[\"B\"]\n    a -->|moves| b",
    /// );
    /// ```
    ///
    /// A diagram with no nodes still renders to something a parser accepts, rather than to
    /// a bare header that may or may not be a document:
    ///
    /// ```
    /// use majordomus_cli::diagram::{Diagram, Kind, Provenance};
    /// let empty = Diagram::new("x", "X", Kind::State, Provenance::new("a", "b"));
    /// assert_eq!(empty.to_mermaid(), "stateDiagram-v2\n    %% no nodes were derived");
    /// ```
    pub fn to_mermaid(&self) -> String {
        let mut lines = vec![match self.kind {
            Kind::Flow => format!("flowchart {}", self.orientation.as_str()),
            Kind::State => "stateDiagram-v2".to_string(),
        }];

        if self.nodes.is_empty() {
            lines.push("    %% no nodes were derived".to_string());
            return lines.join("\n");
        }

        match self.kind {
            Kind::Flow => self.render_flow(&mut lines),
            Kind::State => self.render_state(&mut lines),
        }
        lines.join("\n")
    }

    /// Boxes, subgraphs and arrows.
    fn render_flow(&self, lines: &mut Vec<String>) {
        let grouped: Vec<&str> = self
            .groups
            .iter()
            .flat_map(|g| g.members.iter().map(String::as_str))
            .collect();
        for group in &self.groups {
            lines.push(format!(
                "    subgraph {} [{}]",
                group.id,
                quoted(&group.label)
            ));
            for member in &group.members {
                if let Some(node) = self.nodes.iter().find(|n| &n.id == member) {
                    lines.push(format!("        {}", flow_node(node)));
                }
            }
            lines.push("    end".to_string());
        }
        for node in self
            .nodes
            .iter()
            .filter(|n| !grouped.contains(&n.id.as_str()))
        {
            lines.push(format!("    {}", flow_node(node)));
        }
        for edge in &self.edges {
            let arrow = match edge.style {
                EdgeStyle::Solid => "-->",
                EdgeStyle::Dotted => "-.->",
            };
            let arrow = match &edge.label {
                Some(label) => format!("{arrow}|{}|", inline(label)),
                None => arrow.to_string(),
            };
            lines.push(format!("    {} {arrow} {}", edge.from, edge.to));
        }
    }

    /// States and transitions. `[*]` is written for every [`Shape::Terminal`] node: Mermaid
    /// reads the arrow's direction to decide whether that is the start or the end, which is
    /// why a lifecycle carries two terminal nodes and not one.
    fn render_state(&self, lines: &mut Vec<String>) {
        for node in &self.nodes {
            if node.shape != Shape::Terminal && node.label != node.id {
                lines.push(format!("    state {} as {}", quoted(&node.label), node.id));
            }
        }
        let terminals: Vec<&str> = self
            .nodes
            .iter()
            .filter(|n| n.shape == Shape::Terminal)
            .map(|n| n.id.as_str())
            .collect();
        let name = |id: &str| {
            if terminals.contains(&id) {
                "[*]".to_string()
            } else {
                id.to_string()
            }
        };
        for edge in &self.edges {
            let tail = match &edge.label {
                Some(label) => format!(": {}", inline(label)),
                None => String::new(),
            };
            lines.push(format!(
                "    {} --> {}{tail}",
                name(&edge.from),
                name(&edge.to)
            ));
        }
    }
}

/// Characters a Mermaid identifier may hold. Deliberately narrow: everything outside it is
/// a [`Diagram::validate`] finding rather than an escaping problem at render time.
fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The words Mermaid reserves. `end` is the dangerous one: as a node id inside a subgraph it
/// closes the subgraph instead.
const MERMAID_KEYWORDS: &[&str] = &[
    "end",
    "graph",
    "subgraph",
    "state",
    "class",
    "click",
    "style",
    "flowchart",
    "direction",
];

/// A label as a Mermaid quoted string. `"` becomes the entity Mermaid reads, and a newline
/// becomes the break a label can actually hold.
fn quoted(label: &str) -> String {
    format!(
        "\"{}\"",
        label.replace('"', "#quot;").replace('\n', "<br/>")
    )
}

/// A label written on an arrow, where the text runs to the end of the line: it cannot hold a
/// newline, and in a flowchart it cannot hold the `|` that closes it.
fn inline(label: &str) -> String {
    label
        .replace(['\n', '\r'], " ")
        .replace('|', "/")
        .trim()
        .to_string()
}

/// One node's declaration in a flowchart, in the brackets its shape asks for.
fn flow_node(node: &Node) -> String {
    let label = quoted(&node.label);
    match node.shape {
        Shape::Box => format!("{}[{label}]", node.id),
        Shape::Rounded => format!("{}({label})", node.id),
        Shape::Stadium => format!("{}([{label}])", node.id),
        Shape::Cylinder => format!("{}[({label})]", node.id),
        Shape::Terminal => format!("{}(({label}))", node.id),
    }
}

/// The outcome vocabulary `majordomus finish` accepts, read out of the shell source that
/// enforces it.
///
/// This is the canonical statement of the set: the `case` arm in `lib/finish.sh` is what
/// actually refuses an unknown outcome, so a drawing derived from anything else — a second
/// list, a hand-written diagram — can disagree with the tool and look authoritative while
/// doing it. `None` when the source carries no vocabulary this can recognise, because a
/// wrong set silently drawn is worse than no drawing.
///
/// ```
/// use majordomus_cli::diagram::outcomes;
/// let shell = "  case \"$outcome\" in completed|partial|failed) ;;\n";
/// assert_eq!(outcomes(shell), Some(vec!["completed".into(), "partial".into(), "failed".into()]));
/// assert_eq!(outcomes("nothing of the sort"), None);
/// ```
pub fn outcomes(shell: &str) -> Option<Vec<String>> {
    const MARKER: &str = "case \"$outcome\" in";
    let after = shell.lines().find_map(|line| line.split_once(MARKER))?.1;
    let words: Vec<String> = after
        .split_once(')')?
        .0
        .split('|')
        .map(|word| word.trim().to_string())
        .collect();
    let named = |word: &String| {
        !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase() || c == '_')
    };
    if words.is_empty() || !words.iter().all(named) {
        return None;
    }
    Some(words)
}

/// The task lifecycle as a typed diagram, derived from the text of `lib/finish.sh`.
///
/// The states a task can end in are not written here: they are [`outcomes`] of the source
/// passed in, so an outcome added to the tool appears in every rendering of the lifecycle
/// without anyone drawing it. The transitions that are not outcomes — start, checkpoint,
/// handover — are the lifecycle the workflow defines, and they are stated once, here.
///
/// ```
/// use majordomus_cli::diagram::{lifecycle, Kind};
///
/// let shell = "  case \"$outcome\" in completed|failed) ;;\n";
/// let drawn = lifecycle(shell).expect("the vocabulary is readable");
///
/// assert_eq!(drawn.kind, Kind::State);
/// assert_eq!(drawn.provenance.source, "lib/finish.sh");
/// assert!(drawn.nodes.iter().any(|n| n.id == "failed"), "every outcome is a state");
/// assert!(drawn.to_mermaid().contains("active --> failed: finish --outcome failed"));
/// assert!(drawn.validate().is_empty());
///
/// assert!(lifecycle("no case arm here").is_none(), "no vocabulary, no diagram");
/// ```
pub fn lifecycle(shell: &str) -> Option<Diagram> {
    let outcomes = outcomes(shell)?;
    let mut drawn = Diagram::new(
        "lifecycle",
        "Task lifecycle",
        Kind::State,
        Provenance::new("lib/finish.sh", "the outcome vocabulary"),
    )
    .with_node(Node::new("entry", "entry").shaped(Shape::Terminal))
    .with_node(Node::new("active", "active"))
    .with_node(Node::new("handed_over", "handed_over"));
    for outcome in &outcomes {
        drawn = drawn.with_node(Node::new(outcome.clone(), outcome.clone()));
    }
    drawn = drawn
        .with_node(Node::new("exit", "exit").shaped(Shape::Terminal))
        .with_edge(Edge::new("entry", "active").labelled("start"))
        .with_edge(Edge::new("active", "active").labelled("check --checkpoint"))
        .with_edge(Edge::new("active", "handed_over").labelled("handover --close"))
        .with_edge(Edge::new("handed_over", "exit").labelled("next start archives the record"));
    for outcome in &outcomes {
        drawn = drawn
            .with_edge(
                Edge::new("active", outcome.clone())
                    .labelled(format!("finish --outcome {outcome}")),
            )
            .with_edge(Edge::new(outcome.clone(), "exit"));
    }
    Some(drawn)
}

#[cfg(test)]
mod tests {
    //! What a renderer must not do, as tests.
    //!
    //! They live in this file because `project.rust-command-tested-in-file` asks for it, and
    //! because the quality scanner counts a module as behaviourally tested only from the
    //! tests it can see beside the code.
    use super::*;

    /// The vocabulary `lib/finish.sh` carries in this repository, as the derivation reads it.
    const FINISH_CASE: &str =
        "  case \"$outcome\" in completed|partial|blocked|no_match|failed) ;;\n    \"\") mj_die\n";

    /// A flowchart with a group, a shape of every kind and both edge styles.
    fn pipeline() -> Diagram {
        Diagram::new(
            "pipeline",
            "How this site is derived",
            Kind::Flow,
            Provenance::new("scripts/generate-site-data", "its declared inputs"),
        )
        .oriented(Orientation::LeftRight)
        .with_node(Node::new("a", "share/skeleton"))
        .with_node(Node::new("b", "docs + README"))
        .with_node(Node::new("g", "generate-site-data").shaped(Shape::Stadium))
        .with_node(Node::new("d", "site/data/generated").shaped(Shape::Cylinder))
        .with_node(Node::new("p", "GitHub Pages").shaped(Shape::Rounded))
        .with_group(Group::new("canonical", "canonical", ["a", "b"]))
        .with_edge(Edge::new("a", "g"))
        .with_edge(Edge::new("b", "g"))
        .with_edge(Edge::new("g", "d").labelled("writes"))
        .with_edge(Edge::new("d", "p"))
        .with_edge(Edge::new("g", "p").dotted().labelled("stamps"))
    }

    #[test]
    fn a_state_diagram_renders_the_dialect_and_writes_both_terminals_as_the_same_token() {
        let drawn = lifecycle(FINISH_CASE).expect("the vocabulary parses");
        let mermaid = drawn.to_mermaid();
        assert!(mermaid.starts_with("stateDiagram-v2\n"), "{mermaid}");
        assert!(mermaid.contains("    [*] --> active: start\n"), "{mermaid}");
        assert!(mermaid.contains("\n    failed --> [*]"), "{mermaid}");
        assert!(
            !mermaid.contains("entry") && !mermaid.contains("exit"),
            "the terminal ids are internal to the model: {mermaid}"
        );
        assert!(
            !mermaid.ends_with('\n'),
            "the renderer yields a value, not a file"
        );
    }

    #[test]
    fn a_flow_diagram_renders_every_shape_the_group_and_both_arrows() {
        let mermaid = pipeline().to_mermaid();
        assert_eq!(
            mermaid,
            concat!(
                "flowchart LR\n",
                "    subgraph canonical [\"canonical\"]\n",
                "        a[\"share/skeleton\"]\n",
                "        b[\"docs + README\"]\n",
                "    end\n",
                "    g([\"generate-site-data\"])\n",
                "    d[(\"site/data/generated\")]\n",
                "    p(\"GitHub Pages\")\n",
                "    a --> g\n",
                "    b --> g\n",
                "    g -->|writes| d\n",
                "    d --> p\n",
                "    g -.->|stamps| p",
            )
        );
    }

    #[test]
    fn rendering_is_deterministic() {
        for drawn in [lifecycle(FINISH_CASE).expect("parses"), pipeline()] {
            let once = drawn.to_mermaid();
            assert_eq!(
                once,
                drawn.to_mermaid(),
                "the same model renders the same bytes"
            );
            // and the model itself is a function of its source, not of the run
            let round: Diagram =
                serde_json::from_str(&serde_json::to_string(&drawn).expect("serialises"))
                    .expect("deserialises");
            assert_eq!(round, drawn, "the JSON projection loses nothing");
            assert_eq!(round.to_mermaid(), once);
        }
        assert_eq!(
            lifecycle(FINISH_CASE).map(|d| d.to_mermaid()),
            lifecycle(FINISH_CASE).map(|d| d.to_mermaid()),
            "two derivations from one source agree",
        );
    }

    #[test]
    fn a_model_with_no_nodes_renders_a_document_rather_than_a_bare_header() {
        for kind in [Kind::Flow, Kind::State] {
            let empty = Diagram::new("nothing", "Nothing", kind, Provenance::new("a", "b"));
            let mermaid = empty.to_mermaid();
            assert!(
                mermaid.ends_with("\n    %% no nodes were derived"),
                "{mermaid}"
            );
            assert!(
                empty.validate().is_empty(),
                "empty is not malformed, only empty"
            );
        }
    }

    #[test]
    fn validate_names_what_a_renderer_must_never_be_the_first_to_notice() {
        let bad = Diagram::new("bad", "Bad", Kind::State, Provenance::new("a", "b"))
            .with_node(Node::new("a b", "A"))
            .with_node(Node::new("end", "End"))
            .with_node(Node::new("end", "End again"))
            .with_group(Group::new("g", "G", ["ghost"]))
            .with_edge(Edge::new("a b", "nowhere").dotted());
        let found = bad.validate();
        let joined = found.join("\n");
        for expected in [
            "is not a Mermaid identifier",
            "is a Mermaid keyword",
            "is declared twice",
            "which is not a node",
            "a state diagram cannot draw",
            "flowchart concept",
        ] {
            assert!(
                joined.contains(expected),
                "missing {expected:?} in:\n{joined}"
            );
        }
    }

    #[test]
    fn a_label_never_breaks_the_line_it_is_written_on() {
        let drawn = Diagram::new("q", "Q", Kind::Flow, Provenance::new("a", "b"))
            .with_node(Node::new("a", "a \"quoted\"\nlabel"))
            .with_node(Node::new("b", "B"))
            .with_edge(Edge::new("a", "b").labelled("one\ntwo | three"));
        let mermaid = drawn.to_mermaid();
        assert!(
            mermaid.contains("a[\"a #quot;quoted#quot;<br/>label\"]"),
            "{mermaid}"
        );
        assert!(mermaid.contains("a -->|one two / three| b"), "{mermaid}");
        assert_eq!(
            mermaid.lines().count(),
            4,
            "nothing spilled onto a new line"
        );
    }

    #[test]
    fn the_outcome_vocabulary_is_read_and_never_guessed() {
        assert_eq!(
            outcomes(FINISH_CASE),
            Some(vec![
                "completed".into(),
                "partial".into(),
                "blocked".into(),
                "no_match".into(),
                "failed".into()
            ])
        );
        assert_eq!(
            outcomes("case \"$outcome\" in $VAR) ;;"),
            None,
            "a variable is not a word"
        );
        assert_eq!(
            outcomes("case \"$other\" in a|b) ;;"),
            None,
            "a different case is not it"
        );
        assert_eq!(lifecycle("nothing"), None);
    }

    #[test]
    fn the_model_reproduces_the_lifecycle_the_site_already_publishes() {
        // The point of the slice: the typed model is not a second statement of the
        // lifecycle. It renders, byte for byte, what `scripts/generate-site-data`
        // composes by `printf` today — so replacing that producer changes no published
        // page, and everything else that wants this topology can stop re-typing it.
        let expected = concat!(
            "stateDiagram-v2\n",
            "    [*] --> active: start\n",
            "    active --> active: check --checkpoint\n",
            "    active --> handed_over: handover --close\n",
            "    handed_over --> [*]: next start archives the record\n",
            "    active --> completed: finish --outcome completed\n",
            "    completed --> [*]\n",
            "    active --> partial: finish --outcome partial\n",
            "    partial --> [*]\n",
            "    active --> blocked: finish --outcome blocked\n",
            "    blocked --> [*]\n",
            "    active --> no_match: finish --outcome no_match\n",
            "    no_match --> [*]\n",
            "    active --> failed: finish --outcome failed\n",
            "    failed --> [*]",
        );
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib/finish.sh"),
        )
        .expect("the repository's own lib/finish.sh");
        let drawn = lifecycle(&source).expect("the real source carries the vocabulary");
        assert_eq!(drawn.to_mermaid(), expected);
        assert_eq!(drawn.title, "Task lifecycle");
    }
}
