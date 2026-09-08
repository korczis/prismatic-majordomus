//! Graphs, canonically: a node and edge model in Rust, derived from the registry and the
//! index, and projected to whatever a reader wants. Nothing here knows about Cytoscape,
//! Mermaid or Graphviz; a rendering library is a consumer of [`Graph`], never its shape.
//!
//! Every graph is derived. None is authored: a node exists because a capability, an
//! object or a projection exists, and an edge exists because one of them names another.
//! A derivation that would need a file listing its own nodes belongs somewhere else.
//!
//! Determinism is a contract: the same tree and the same executable produce the same
//! bytes. Nodes and edges are sorted by their identities, never by discovery order, and
//! no derivation reads a clock.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::{Capability, CapabilityKind, CapabilityRegistry, Provenance};
use crate::index::Index;
use crate::model::Object;

/// One node. `id` is unique within the graph; `kind` is what the graph's `node_kinds`
/// declares it to be; `route` is where the Cockpit shows the thing itself, when it shows
/// it anywhere.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    /// Unique within the graph.
    pub id: String,
    /// One of the graph's declared node kinds.
    pub kind: String,
    /// The short label a renderer draws.
    pub label: String,
    /// One line about the thing, when the source holds one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Where the Cockpit shows this thing, when it shows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// The repository-relative file the node was derived from, when one file owns it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// A status word the graph's own vocabulary defines (`accepted`, `active`, `query`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// True when the node stands for something the graph names but does not hold: a
    /// reference that resolves to nothing in this repository.
    #[serde(default, skip_serializing_if = "is_false")]
    pub external: bool,
    /// What the object's own kind declares about it, as its schema validated it: the
    /// fields of its front matter that are facts about the thing rather than the document
    /// itself. A rule carries its class and its statement, a skill its inputs and
    /// outputs, a decision its date — the vocabulary that makes each kind worth having,
    /// rather than the four fields every kind happens to share.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub facts: BTreeMap<String, Fact>,
}

/// One fact a node carries from its object's front matter, in the shapes a node can hold.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Fact {
    /// A yes or a no.
    Flag(bool),
    /// A whole number: an order, a version, a weight.
    Count(i64),
    /// One short value.
    Text(String),
    /// Several short values.
    List(Vec<String>),
}

/// The longest value a node carries as a fact. Beyond it the field is prose — an
/// objective, a rationale, an acceptance criterion — and prose belongs to the object,
/// which `objects.get` serves whole and which every node links to by its route. Copying
/// it here would put the layer in the graph twice.
const FACT_LIMIT: usize = 200;

/// The fields the node already shows through `label`, `summary`, `status` and `kind`.
/// Carrying them again would say the same thing twice in one node.
const PROJECTED_FIELDS: &[&str] = &[
    "id",
    "version",
    "kind",
    "title",
    "description",
    "status",
    "schema",
];

/// The facts of one object's front matter: every declared field that is a fact about the
/// thing, in the object's own vocabulary. Nothing is named here — the fields are whatever
/// the kind's schema let the object declare, minus the ones the node already shows.
fn facts(metadata: &Value) -> BTreeMap<String, Fact> {
    let Some(map) = metadata.as_object() else {
        return BTreeMap::new();
    };
    let short = |s: &str| s.len() <= FACT_LIMIT;
    map.iter()
        .filter(|(k, _)| !PROJECTED_FIELDS.contains(&k.as_str()))
        .filter_map(|(k, v)| {
            let fact = match v {
                Value::Bool(b) => Fact::Flag(*b),
                Value::Number(n) => Fact::Count(n.as_i64()?),
                Value::String(s) if short(s) => Fact::Text(s.clone()),
                Value::Array(items) => {
                    // a list is a fact only whole: half a list of criteria is a claim the
                    // reader cannot check
                    let texts: Option<Vec<String>> = items
                        .iter()
                        .map(|i| match i {
                            Value::String(s) if short(s) => Some(s.clone()),
                            Value::Bool(b) => Some(b.to_string()),
                            Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        })
                        .collect();
                    Fact::List(texts?)
                }
                _ => return None,
            };
            Some((k.clone(), fact))
        })
        .collect()
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One directed edge. Both ends are node ids of the same graph; a derivation that cannot
/// resolve an end adds an external node rather than a dangling edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    /// The node the edge leaves.
    pub source: String,
    /// The node the edge enters.
    pub target: String,
    /// One of the graph's declared edge kinds.
    pub kind: String,
}

/// What a reader needs to know about the graph as a whole before drawing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphMetadata {
    /// How many nodes.
    pub nodes: usize,
    /// How many edges.
    pub edges: usize,
    /// Whether the edges form a directed acyclic graph. Derived, never declared: a graph
    /// whose contract is a DAG and whose answer here is `false` is a defect in the data.
    pub acyclic: bool,
    /// True when the derivation stopped at [`MAX_NODES`] and the graph is a prefix of
    /// what the repository holds.
    pub truncated: bool,
}

/// The most nodes a derivation emits. A graph past this is unreadable in a browser and
/// unhelpful in a terminal; the derivation stops, says so in the metadata, and the page
/// that shows it says so too.
pub const MAX_NODES: usize = 2000;

/// A derived graph: what it is, where it came from, what its vocabularies mean, and the
/// nodes and edges themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Graph {
    /// The identity, `[a-z][a-z0-9-]*`; the last segment of its route.
    pub id: String,
    /// The short name.
    pub title: String,
    /// One paragraph: what a reader learns from it.
    pub description: String,
    /// What it was derived from, in the repository's own words.
    pub source: String,
    /// Node kind to what that kind means.
    pub node_kinds: BTreeMap<String, String>,
    /// Edge kind to what that edge asserts.
    pub edge_kinds: BTreeMap<String, String>,
    /// Sorted by id.
    pub nodes: Vec<Node>,
    /// Sorted.
    pub edges: Vec<Edge>,
    /// Counts and invariants.
    pub metadata: GraphMetadata,
}

/// A graph under construction. Adding an edge whose ends are not both present is refused
/// by [`Builder::finish`], not silently dropped: an edge to nothing is a derivation bug.
#[derive(Debug)]
pub struct Builder {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    source: String,
    node_kinds: BTreeMap<String, String>,
    edge_kinds: BTreeMap<String, String>,
    nodes: BTreeMap<String, Node>,
    edges: BTreeSet<Edge>,
    truncated: bool,
}

impl Builder {
    /// A builder for a graph with this identity and these words.
    pub fn new(
        id: &'static str,
        title: &'static str,
        description: &'static str,
        source: impl Into<String>,
    ) -> Self {
        Builder {
            id,
            title,
            description,
            source: source.into(),
            node_kinds: BTreeMap::new(),
            edge_kinds: BTreeMap::new(),
            nodes: BTreeMap::new(),
            edges: BTreeSet::new(),
            truncated: false,
        }
    }

    /// Declare what a node kind means. Every kind a node carries must be declared.
    pub fn node_kind(mut self, kind: &str, meaning: &str) -> Self {
        self.node_kinds.insert(kind.into(), meaning.into());
        self
    }

    /// Declare what an edge kind asserts. Every kind an edge carries must be declared.
    pub fn edge_kind(mut self, kind: &str, meaning: &str) -> Self {
        self.edge_kinds.insert(kind.into(), meaning.into());
        self
    }

    /// Add a node, or keep the one already there. Answers whether there is room for more:
    /// a derivation that walks a large collection stops when this says no.
    pub fn node(&mut self, node: Node) -> bool {
        if self.nodes.len() >= MAX_NODES && !self.nodes.contains_key(&node.id) {
            self.truncated = true;
            return false;
        }
        self.nodes.entry(node.id.clone()).or_insert(node);
        true
    }

    /// Add an edge. Both ends are checked when the graph is finished.
    pub fn edge(&mut self, source: &str, target: &str, kind: &str) {
        self.edges.insert(Edge {
            source: source.into(),
            target: target.into(),
            kind: kind.into(),
        });
    }

    /// Is this node already in the graph?
    pub fn has(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    /// Finish: drop edges whose ends the derivation never added, sort everything, and
    /// compute the invariants.
    ///
    /// An edge to a missing node is dropped rather than kept, because a renderer given a
    /// dangling edge draws a phantom; the property suite proves no derivation produces
    /// one, so the drop is a safety net and not a policy.
    pub fn finish(self) -> Graph {
        let nodes: Vec<Node> = self.nodes.into_values().collect();
        let present: BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        let edges: Vec<Edge> = self
            .edges
            .into_iter()
            .filter(|e| present.contains(e.source.as_str()) && present.contains(e.target.as_str()))
            .collect();
        let acyclic = is_acyclic(&nodes, &edges);
        Graph {
            id: self.id.into(),
            title: self.title.into(),
            description: self.description.into(),
            source: self.source,
            node_kinds: self.node_kinds,
            edge_kinds: self.edge_kinds,
            metadata: GraphMetadata {
                nodes: nodes.len(),
                edges: edges.len(),
                acyclic,
                truncated: self.truncated,
            },
            nodes,
            edges,
        }
    }
}

/// Does the edge set form a directed acyclic graph? Kahn's algorithm over the node set.
///
/// ```
/// use majordomus_cli::graph::{is_acyclic, Edge, Node};
/// fn n(id: &str) -> Node {
///     Node { id: id.into(), kind: "x".into(), label: id.into(), summary: None, route: None,
///             source: None, status: None, external: false, facts: Default::default() }
/// }
/// fn e(a: &str, b: &str) -> Edge { Edge { source: a.into(), target: b.into(), kind: "k".into() } }
/// assert!(is_acyclic(&[n("a"), n("b")], &[e("a", "b")]));
/// assert!(!is_acyclic(&[n("a"), n("b")], &[e("a", "b"), e("b", "a")]));
/// ```
pub fn is_acyclic(nodes: &[Node], edges: &[Edge]) -> bool {
    let mut outgoing: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> = nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
    for e in edges {
        outgoing
            .entry(e.source.as_str())
            .or_default()
            .push(e.target.as_str());
        *indegree.entry(e.target.as_str()).or_insert(0) += 1;
    }
    let mut ready: Vec<&str> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut seen = 0usize;
    while let Some(id) = ready.pop() {
        seen += 1;
        for target in outgoing.get(id).into_iter().flatten() {
            let d = indegree.entry(target).or_insert(0);
            *d = d.saturating_sub(1);
            if *d == 0 {
                ready.push(target);
            }
        }
    }
    seen == indegree.len()
}

/// One graph the executable can derive: what it is, without deriving it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphInfo {
    /// The identity.
    pub id: String,
    /// The short name.
    pub title: String,
    /// One paragraph.
    pub description: String,
    /// What it is derived from.
    pub source: String,
}

/// Every graph this executable derives, by id, with the function that derives it. The one
/// list there is: the capability that lists graphs and the one that reads them both read
/// it, and a graph added here is a graph everywhere.
type Derivation = fn(&CapabilityRegistry, &Index) -> Graph;

const DERIVATIONS: &[(&str, Derivation)] = &[
    ("registry", registry_graph),
    ("layer", layer_graph),
    ("rules", rules_graph),
    ("adrs", adrs_graph),
    ("use-cases", use_cases_graph),
    ("why", why_graph),
    ("composed", composed_graph),
];

/// The ids of every graph, in the order they are listed.
///
/// ```
/// assert!(majordomus_cli::graph::ids().contains(&"registry"));
/// ```
pub fn ids() -> Vec<&'static str> {
    DERIVATIONS.iter().map(|(id, _)| *id).collect()
}

/// Derive one graph by id; `None` when no such graph exists.
pub fn derive(id: &str, registry: &CapabilityRegistry, index: &Index) -> Option<Graph> {
    let _phase = crate::perf::phase(crate::perf::Phase::GraphBuild);
    crate::perf::Counters::bump(&crate::perf::COUNTERS.graph_builds);
    DERIVATIONS
        .iter()
        .find(|(known, _)| *known == id)
        .map(|(_, f)| f(registry, index))
}

/// Every graph, described but not derived. Deriving all of them to list them would read
/// the whole index for a menu.
pub fn list(registry: &CapabilityRegistry, index: &Index) -> Vec<GraphInfo> {
    DERIVATIONS
        .iter()
        .map(|(_id, f)| {
            // the derivation is the only place a graph's words live; describing one
            // without deriving it would be a second copy of them, so the description
            // comes from an empty derivation over the same sources
            let g = f(registry, index);
            GraphInfo {
                id: g.id,
                title: g.title,
                description: g.description,
                source: g.source,
            }
        })
        .collect()
}

fn capability_route(id: &str) -> String {
    format!(
        "/cockpit/capabilities/{}",
        crate::http::router::percent_encode(id)
    )
}

fn object_route(uri: &str) -> String {
    format!(
        "/cockpit/object?uri={}",
        crate::http::router::percent_encode(uri)
    )
}

/// The registry as a graph: every module, every capability it composes, the file the
/// capability was declared in, and one node per projection it is exposed through. This is
/// the executable describing itself, and it is derived from the same descriptors every
/// other projection is derived from.
fn registry_graph(registry: &CapabilityRegistry, _index: &Index) -> Graph {
    let mut b = Builder::new(
        "registry",
        "Capability registry",
        "Every module, the capabilities it composes, the source file each was declared in, and the MCP, HTTP, command-line and Cockpit projections derived from it. The executable's own architecture, read back from the registry that produces every interface — this page among them.",
        "the capability registry: builtin descriptors and the declarative objects of the layer",
    )
    .node_kind("module", "a module composed with module! and named in compose_modules!, or one kind of declarative object")
    .node_kind("capability", "one canonical declaration: a query, a command, or a declarative resource")
    .node_kind("source", "the file the declaration was read from")
    .node_kind("projection", "an external interface the descriptor is projected onto, the Cockpit included")
    .edge_kind("composes", "the module composes the capability")
    .edge_kind("declared_in", "the capability was declared in that file")
    .edge_kind("projects", "the capability is exposed through that interface");

    for m in registry.modules() {
        b.node(Node {
            id: format!("module:{}", m.id),
            kind: "module".into(),
            label: m.id.to_string(),
            summary: Some(m.title.clone()),
            route: Some(format!(
                "/cockpit/capabilities?module={}",
                crate::http::router::percent_encode(m.id.as_str())
            )),
            source: None,
            status: Some(enum_word(&m.source)),
            external: false,
            facts: BTreeMap::new(),
        });
    }

    // one node per projection surface, so that "what does HTTP serve" is a neighbourhood
    for (id, label, meaning) in [
        (
            "projection:mcp",
            "MCP",
            "tools and resources an MCP client sees",
        ),
        (
            "projection:http",
            "HTTP",
            "routes under /api/v1/, and the OpenAPI document over them",
        ),
        ("projection:cli", "CLI", "the words after `majordomus`"),
        (
            "projection:cockpit",
            "Cockpit",
            "a page for every capability, and a form generated from the input schema for every one the Cockpit can call",
        ),
    ] {
        b.node(Node {
            id: id.into(),
            kind: "projection".into(),
            label: label.into(),
            summary: Some(meaning.into()),
            route: None,
            source: None,
            status: None,
            external: false,
            facts: BTreeMap::new(),
        });
    }

    for c in registry.iter() {
        // the declarative objects are the layer graph's subject; here they would add six
        // hundred leaves that say only "this kind has members"
        if !matches!(c.provenance, Provenance::Builtin { .. }) {
            continue;
        }
        let node_id = format!("capability:{}", c.id);
        if !b.node(Node {
            id: node_id.clone(),
            kind: "capability".into(),
            label: c.id.to_string(),
            summary: Some(c.title.clone()),
            route: Some(capability_route(c.id.as_str())),
            source: Some(c.provenance.source_path()),
            status: Some(kind_word(c.kind)),
            external: false,
            facts: BTreeMap::new(),
        }) {
            break;
        }
        b.edge(&format!("module:{}", c.module), &node_id, "composes");

        let path = c.provenance.source_path();
        let source_id = format!("source:{path}");
        b.node(Node {
            id: source_id.clone(),
            kind: "source".into(),
            label: path.rsplit('/').next().unwrap_or(&path).to_string(),
            summary: Some(path.clone()),
            route: None,
            source: Some(path.clone()),
            status: None,
            external: false,
            facts: BTreeMap::new(),
        });
        b.edge(&node_id, &source_id, "declared_in");

        if c.exposure.mcp.is_some() {
            b.edge(&node_id, "projection:mcp", "projects");
        }
        if c.exposure.http.is_some() {
            b.edge(&node_id, "projection:http", "projects");
        }
        if c.exposure.cli.is_some() {
            b.edge(&node_id, "projection:cli", "projects");
        }
        // the Cockpit is a projection like the others, and it is on this graph so that the
        // executable's own picture of itself is complete rather than complete-except-the-
        // part-a-person-looks-at. Every capability has a page; the ones with an HTTP
        // exposure also get a form that calls them.
        b.edge(&node_id, "projection:cockpit", "projects");
    }
    b.finish()
}

/// The layer as a graph: the directories the objects were discovered in, nested, with one
/// node per kind under the directory that holds it. The shape of `.ai/`, read back from
/// what was actually indexed rather than from a listing of what should be there.
fn layer_graph(_registry: &CapabilityRegistry, index: &Index) -> Graph {
    let mut b = Builder::new(
        "layer",
        "The layer's shape",
        "Every directory an object was discovered in, nested as the tree nests them, and the kinds each directory holds with how many objects of that kind. Derived from the provenance of the indexed objects, so a directory appears here exactly when something in it was read.",
        "the provenance of every object of the index",
    )
    .node_kind("directory", "a directory at least one indexed object sits in")
    .node_kind("kind", "one kind of object, under the directory that holds it")
    .edge_kind("contains", "the directory contains the directory or the kind")
    .edge_kind("read_as", "the kind's objects in that directory");

    let mut per_directory: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for o in &index.objects {
        *per_directory
            .entry(o.provenance.directory.as_str())
            .or_default()
            .entry(o.kind.as_str())
            .or_insert(0) += 1;
    }

    for (dir, kinds) in &per_directory {
        // every ancestor, so the tree is connected without a second walk of the filesystem
        let mut prefix: Vec<&str> = Vec::new();
        for segment in dir.split('/').filter(|s| !s.is_empty() && *s != ".") {
            prefix.push(segment);
            let path = prefix.join("/");
            let id = format!("directory:{path}");
            if !b.node(Node {
                id: id.clone(),
                kind: "directory".into(),
                label: segment.to_string(),
                summary: Some(path.clone()),
                route: None,
                source: Some(path.clone()),
                status: None,
                external: false,
                facts: BTreeMap::new(),
            }) {
                break;
            }
            if prefix.len() > 1 {
                let parent = prefix[..prefix.len() - 1].join("/");
                b.edge(&format!("directory:{parent}"), &id, "contains");
            }
        }
        for (kind, count) in kinds {
            let id = format!("kind:{dir}:{kind}");
            if !b.node(Node {
                id: id.clone(),
                kind: "kind".into(),
                label: (*kind).to_string(),
                summary: Some(format!("{count} object(s)")),
                route: Some(format!(
                    "/cockpit/objects?kind={}",
                    crate::http::router::percent_encode(kind)
                )),
                source: Some((*dir).to_string()),
                status: None,
                external: false,
                facts: BTreeMap::new(),
            }) {
                break;
            }
            if *dir != "." {
                b.edge(&format!("directory:{dir}"), &id, "read_as");
            }
        }
    }
    b.finish()
}

/// The effective rule set as a graph: every rule of the index and the rules it declares
/// it depends on. The dependency is the rule's own `depends_on`, resolved against the
/// index; a dependency the repository does not hold becomes an external node, so a broken
/// reference is visible rather than absent.
fn rules_graph(_registry: &CapabilityRegistry, index: &Index) -> Graph {
    let mut b = Builder::new(
        "rules",
        "Rule dependencies",
        "Every rule of the effective set and the rules it declares it depends on, with the enforcement class each carries. A rule whose dependency the repository does not hold is drawn against an external node rather than left dangling.",
        "the `depends_on` front matter of every object of kind `rule`",
    )
    .node_kind("rule", "a rule of the effective set, vendored or the project's own")
    .node_kind("missing", "a rule named as a dependency that the index does not hold")
    .edge_kind("depends_on", "the rule states it depends on the other");

    for o in index.objects.iter().filter(|o| o.kind == "rule") {
        let id = format!("rule:{}", o.identity);
        if !b.node(Node {
            id: id.clone(),
            kind: "rule".into(),
            label: o.identity.clone(),
            summary: o.title.clone(),
            route: Some(object_route(&o.uri)),
            source: Some(o.provenance.path.clone()),
            status: metadata_string(&o.metadata, "class"),
            external: false,
            facts: BTreeMap::new(),
        }) {
            break;
        }
    }
    for o in index.objects.iter().filter(|o| o.kind == "rule") {
        for dep in metadata_strings(&o.metadata, "depends_on") {
            let target = format!("rule:{dep}");
            if !b.has(&target) {
                b.node(Node {
                    id: target.clone(),
                    kind: "missing".into(),
                    label: dep.clone(),
                    summary: Some("named as a dependency; not in this index".into()),
                    route: None,
                    source: None,
                    status: None,
                    external: true,
                    facts: BTreeMap::new(),
                });
            }
            b.edge(&format!("rule:{}", o.identity), &target, "depends_on");
        }
    }
    b.finish()
}

/// The decisions as a graph: every ADR, what it supersedes, and what it put in force.
/// The `related` references are typed (`rule:`, `claim:`, `file:`, `test:`), so the
/// reverse index — what a rule was decided by — is an edge and never a second document.
fn adrs_graph(_registry: &CapabilityRegistry, index: &Index) -> Graph {
    let mut b = Builder::new(
        "adrs",
        "Decisions and what they put in force",
        "Every architecture decision, the decisions it stands in for, and the rules, claims, files and tests it names as what it put in force. The reverse question — what decided this rule — is the same edge read backwards.",
        "the front matter of every object of kind `adr`",
    )
    .node_kind("adr", "one architecture decision record")
    .node_kind("rule", "a rule the decision put in force")
    .node_kind("claim", "a claim of docs/CLAIMS.yaml the decision backs")
    .node_kind("file", "a document or an implementation the decision names")
    .node_kind("test", "a behavioural case the decision names")
    .edge_kind("supersedes", "the decision stands in for the other")
    .edge_kind("put_in_force", "the decision put the thing in force");

    for o in index.objects.iter().filter(|o| o.kind == "adr") {
        let id = format!("adr:{}", o.identity);
        if !b.node(Node {
            id: id.clone(),
            kind: "adr".into(),
            label: metadata_string(&o.metadata, "id").unwrap_or_else(|| o.identity.clone()),
            summary: o.title.clone(),
            route: Some(object_route(&o.uri)),
            source: Some(o.provenance.path.clone()),
            status: metadata_string(&o.metadata, "status"),
            external: false,
            facts: BTreeMap::new(),
        }) {
            break;
        }
    }
    let by_adr_id: BTreeMap<String, String> = index
        .objects
        .iter()
        .filter(|o| o.kind == "adr")
        .filter_map(|o| {
            metadata_string(&o.metadata, "id").map(|id| (id, format!("adr:{}", o.identity)))
        })
        .collect();

    for o in index.objects.iter().filter(|o| o.kind == "adr") {
        let from = format!("adr:{}", o.identity);
        for superseded in metadata_strings(&o.metadata, "supersedes") {
            if let Some(target) = by_adr_id.get(&superseded) {
                b.edge(&from, target, "supersedes");
            }
        }
        for reference in metadata_strings(&o.metadata, "related") {
            let Some((prefix, rest)) = reference.split_once(':') else {
                continue;
            };
            if !["rule", "claim", "file", "test"].contains(&prefix) {
                continue;
            }
            let target = format!("{prefix}:{rest}");
            if !b.has(&target) {
                // a `rule:` reference carries no version; the index's rule identities do,
                // so resolve by prefix and fall back to an external node
                let resolved = index
                    .objects
                    .iter()
                    .find(|c| {
                        c.kind == prefix
                            && (c.identity == rest || c.identity.starts_with(&format!("{rest}@")))
                    })
                    .or_else(|| index.objects.iter().find(|c| c.provenance.path == rest));
                let node = match resolved {
                    Some(c) => Node {
                        id: target.clone(),
                        kind: prefix.into(),
                        label: rest.to_string(),
                        summary: c.title.clone(),
                        route: Some(object_route(&c.uri)),
                        source: Some(c.provenance.path.clone()),
                        status: None,
                        external: false,
                        facts: BTreeMap::new(),
                    },
                    None => Node {
                        id: target.clone(),
                        kind: prefix.into(),
                        label: rest.to_string(),
                        summary: Some("named by a decision; not an object of this index".into()),
                        route: None,
                        source: (prefix == "file" || prefix == "test").then(|| rest.to_string()),
                        status: None,
                        external: true,
                        facts: BTreeMap::new(),
                    },
                };
                if !b.node(node) {
                    break;
                }
            }
            b.edge(&from, &target, "put_in_force");
        }
    }
    b.finish()
}

/// The use cases as a graph: every executable use case and the commands, doctrines and
/// claims it names. What a command is proved by, and what a doctrine is exercised by, are
/// this graph read backwards.
fn use_cases_graph(_registry: &CapabilityRegistry, index: &Index) -> Graph {
    let mut b = Builder::new(
        "use-cases",
        "Use cases and what they exercise",
        "Every executable use case with the commands it runs, the doctrines it exercises and the claims it stands behind. Read backwards it answers the coverage question: what proves this command, what exercises this doctrine.",
        "the front matter of every object of kind `use-case`",
    )
    .node_kind("use-case", "one executable use case")
    .node_kind("command", "a command the scenario runs")
    .node_kind("doctrine", "a doctrine the scenario exercises")
    .node_kind("claim", "a claim the use case stands behind")
    .edge_kind("runs", "the scenario runs that command")
    .edge_kind("exercises", "the scenario exercises that doctrine")
    .edge_kind("evidences", "the use case is evidence for that claim");

    for o in index.objects.iter().filter(|o| o.kind == "use-case") {
        let id = format!("use-case:{}", o.identity);
        if !b.node(Node {
            id: id.clone(),
            kind: "use-case".into(),
            label: o.identity.clone(),
            summary: o.title.clone(),
            route: Some(object_route(&o.uri)),
            source: Some(o.provenance.path.clone()),
            status: metadata_string(&o.metadata, "status"),
            external: false,
            facts: BTreeMap::new(),
        }) {
            break;
        }
        for (field, kind, edge) in [
            ("commands", "command", "runs"),
            ("doctrines", "doctrine", "exercises"),
            ("claims", "claim", "evidences"),
        ] {
            for name in metadata_strings(&o.metadata, field) {
                let target = format!("{kind}:{name}");
                if !b.has(&target)
                    && !b.node(Node {
                        id: target.clone(),
                        kind: kind.into(),
                        label: name.clone(),
                        summary: None,
                        route: None,
                        source: None,
                        status: None,
                        external: false,
                        facts: BTreeMap::new(),
                    })
                {
                    break;
                }
                b.edge(&id, &target, edge);
            }
        }
    }
    b.finish()
}

/// Everything, composed: the capability registry and every object of the index in one
/// model, with the node kinds taken from what was actually indexed rather than from a
/// list written here. Adding a kind to the layer adds it to this graph; adding a
/// capability module adds its capabilities. Nothing in this derivation names a kind.
///
/// The graph holds definitions only. What is true of a running process — whether a
/// capability answered, what the health of a surface is now — is [`RuntimeState`], laid
/// over these nodes by a consumer that has a process to ask. Keeping the two apart is
/// what lets the published site render this graph with no server behind it.
///
/// The edges are the point. Beside the structure the registries assert — a module
/// composes its capabilities, a capability is projected through the transports it
/// declares, an object is of its kind — every reference the layer's own front matter
/// already carries becomes a typed edge: a rule to the rules it depends on, a decision to
/// what it put in force and what it stands in for, a use case to what it runs, exercises
/// and evidences, a context document to what it tracks, a claim to the file that
/// implements it and the case that proves it. The conventions are in [`RELATIONS`] and
/// nowhere else, and
/// [`unresolved_relations`] is the same resolution read as a verdict: a reference that
/// names a kind this repository holds and resolves to nothing is a finding, not an edge.
fn composed_graph(registry: &CapabilityRegistry, index: &Index) -> Graph {
    compose(registry, &index.objects)
}

/// The composition itself, over the two registries rather than over an index: the same
/// derivation, testable without a repository on disk.
fn compose(registry: &CapabilityRegistry, objects: &[Object]) -> Graph {
    let mut b = Builder::new(
        "composed",
        "Everything, composed",
        "The capability registry and every object of the layer in one graph: each module with the capabilities it composes, each capability with the transports it is projected through, and every indexed object under the kind that owns it. The node kinds are the kinds that were indexed, so a kind added to the layer appears here without this derivation being edited.",
        "the capability registry and every object of the index",
    )
    .node_kind("module", "a capability module of this executable")
    .node_kind("capability", "one capability, declared once and projected")
    .node_kind(
        "projection",
        "a transport a capability is projected through",
    )
    .node_kind("kind", "one kind of object the layer indexed")
    .node_kind("file", "a file the layer names and does not hold as an object")
    .node_kind("test", "a behavioural case the layer names")
    .node_kind("command", "a command a scenario runs")
    .node_kind("doctrine", "a doctrine a scenario exercises")
    .node_kind("claim", "a claim the layer stands behind")
    .edge_kind("composes", "the module composes the capability")
    .edge_kind("projects", "the capability is projected through the transport")
    .edge_kind("is_a", "the object is of that kind")
    .edge_kind("depends_on", "the rule states it depends on the other")
    .edge_kind("supersedes", "the decision stands in for the other")
    .edge_kind("put_in_force", "the decision put the thing in force")
    .edge_kind("related_to", "the object names the other as related")
    .edge_kind("runs", "the scenario runs that command")
    .edge_kind("exercises", "the scenario exercises that doctrine")
    .edge_kind("evidences", "the object is evidence for that claim")
    .edge_kind("tracks", "the document tracks that file")
    .edge_kind("defined_in", "the claim is defined by that file")
    .edge_kind("implemented_by", "the claim is implemented by that file")
    .edge_kind("tested_by", "the claim is proved by that case")
    .edge_kind("belongs_to", "the issue belongs to that milestone")
    .edge_kind("serves", "the use case serves that application")
    .edge_kind("governed_by", "the application is governed by that doctrine");

    // the kinds are read off what was indexed; a kind named here would be a second
    // declaration of something share/kinds.yaml already owns
    let kinds: BTreeSet<&str> = objects.iter().map(|o| o.kind.as_str()).collect();
    for kind in &kinds {
        b = b.node_kind(kind, &format!("an object of kind `{kind}`"));
    }

    for kind in &kinds {
        let id = format!("kind:{kind}");
        if !b.node(Node {
            id,
            kind: "kind".into(),
            label: (*kind).to_string(),
            summary: Some(format!("every object of kind `{kind}`")),
            route: None,
            source: None,
            status: None,
            external: false,
            facts: BTreeMap::new(),
        }) {
            break;
        }
    }

    for projection in ["mcp", "http", "cli", "cockpit"] {
        b.node(Node {
            id: format!("projection:{projection}"),
            kind: "projection".into(),
            label: projection.into(),
            summary: Some(format!("the {projection} projection of the registry")),
            route: None,
            source: None,
            status: None,
            external: false,
            facts: BTreeMap::new(),
        });
    }

    for c in registry.iter() {
        // the objects of the layer are on this graph as themselves; the resource
        // capabilities that read them would say the same thing a second time
        if !matches!(c.provenance, Provenance::Builtin { .. }) {
            continue;
        }
        let module = format!("module:{}", c.module);
        if !b.has(&module)
            && !b.node(Node {
                id: module.clone(),
                kind: "module".into(),
                label: c.module.to_string(),
                summary: None,
                route: None,
                source: None,
                status: None,
                external: false,
                facts: BTreeMap::new(),
            })
        {
            break;
        }
        let id = format!("capability:{}", c.id);
        if !b.node(Node {
            id: id.clone(),
            kind: "capability".into(),
            label: c.id.to_string(),
            summary: Some(c.title.clone()),
            route: Some(capability_route(c.id.as_str())),
            source: Some(c.provenance.source_path()),
            status: Some(kind_word(c.kind)),
            external: false,
            facts: BTreeMap::new(),
        }) {
            break;
        }
        b.edge(&module, &id, "composes");
        if c.exposure.mcp.is_some() {
            b.edge(&id, "projection:mcp", "projects");
        }
        if c.exposure.http.is_some() {
            b.edge(&id, "projection:http", "projects");
        }
        if c.exposure.cli.is_some() {
            b.edge(&id, "projection:cli", "projects");
        }
        b.edge(&id, "projection:cockpit", "projects");
    }

    for o in objects {
        // the URI is the identity: it survives a retitling, which a label does not
        if !b.node(Node {
            id: o.uri.clone(),
            kind: o.kind.clone(),
            label: o.identity.clone(),
            summary: o.title.clone().or_else(|| o.description.clone()),
            route: Some(object_route(&o.uri)),
            source: Some(o.provenance.path.clone()),
            status: metadata_string(&o.metadata, "status"),
            external: false,
            facts: facts(&o.metadata),
        }) {
            break;
        }
        b.edge(&o.uri, &format!("kind:{}", o.kind), "is_a");
    }

    // the references the layer already carries, resolved through the same table the
    // check reads: an edge appears here exactly when `unresolved_relations` says nothing
    // about it
    let resolver = Resolver::new(registry, objects);
    for o in objects {
        for rel in RELATIONS {
            if !rel.kinds.is_empty() && !rel.kinds.contains(&o.kind.as_str()) {
                continue;
            }
            for reference in metadata_strings(&o.metadata, rel.field) {
                if is_absence(&reference) {
                    continue;
                }
                // the edge runs the way the relationship does, which is not always the
                // way the file that declared it is read
                let target = match resolver.resolve(rel, &reference) {
                    Outcome::Node(target) => Some(target),
                    Outcome::External(node) => {
                        let id = node.id.clone();
                        if !b.has(&id) && !b.node(node) {
                            break;
                        }
                        Some(id)
                    }
                    // a reference that resolves to nothing is a finding, and a finding is
                    // not drawn: an edge to a phantom reads as an answer
                    Outcome::Missing(_) => None,
                };
                if let Some(target) = target {
                    if rel.inverted {
                        b.edge(&target, &o.uri, rel.edge);
                    } else {
                        b.edge(&o.uri, &target, rel.edge);
                    }
                }
            }
        }
    }

    b.finish()
}

/// What a front matter field means when an object of some kind declares it: the one place
/// the layer's reference conventions are written down, read both by the composition that
/// draws the edges and by the check that refuses the ones that resolve to nothing.
///
/// A convention is here exactly when the metadata already carries it. Nothing in this
/// table asks a contributor to restate a relation the layer implies, and nothing in it
/// matches on a title or a substring: every reference resolves through a stable identity,
/// a declared id or a repository-relative path.
struct Relation {
    /// The kinds that declare the field; empty means any kind that carries it.
    kinds: &'static [&'static str],
    /// The front matter key.
    field: &'static str,
    /// The edge the reference asserts.
    edge: &'static str,
    /// How the reference names its target.
    target: Target,
    /// True when the edge runs from the thing named towards the object that named it.
    /// Two kinds sometimes declare one relationship from both ends — an application names
    /// the use cases that serve it and each of those names the application — and one
    /// relationship deserves one edge in one direction rather than two half-truths.
    inverted: bool,
}

/// How a reference names what it points at.
#[derive(Clone, Copy)]
enum Target {
    /// An object of one kind, by identity; a versioned identity resolves by its stem.
    Object(&'static str),
    /// An architecture decision, by the `id` it declares rather than by its file name.
    DeclaredAdr,
    /// `<kind>:<name>`, where the kind is one the layer holds or `file` or `test`.
    Prefixed,
    /// An object of one kind by identity, or a bare name when the layer holds no such
    /// object: a doctrine of the vendored package the index did not read.
    ObjectOrName(&'static str, &'static str),
    /// A name that stands for something outside the layer: a command, a claim.
    Name(&'static str),
    /// A repository-relative path.
    Path,
    /// A capability of the registry, by the MCP tool name it is exposed under.
    McpTool,
}

/// The layer writes a reference that points at nothing on purpose as `-`: a claim with no
/// implementation yet says so rather than omitting the field. That is an absence, not a
/// reference, and neither an edge nor a finding follows from it.
fn is_absence(reference: &str) -> bool {
    reference == "-"
}

const RELATIONS: &[Relation] = &[
    Relation {
        kinds: &["rule"],
        field: "depends_on",
        edge: "depends_on",
        target: Target::Object("rule"),
        inverted: false,
    },
    Relation {
        kinds: &["adr"],
        field: "supersedes",
        edge: "supersedes",
        target: Target::DeclaredAdr,
        inverted: false,
    },
    Relation {
        kinds: &["adr"],
        field: "related",
        edge: "put_in_force",
        target: Target::Prefixed,
        inverted: false,
    },
    // a skill names its siblings by their bare identity, not as `<kind>:<name>`: the
    // convention is the layer's, and this table follows it rather than correcting it
    Relation {
        kinds: &["skill"],
        field: "related",
        edge: "related_to",
        target: Target::Object("skill"),
        inverted: false,
    },
    Relation {
        kinds: &["use-case"],
        field: "commands",
        edge: "runs",
        target: Target::Name("command"),
        inverted: false,
    },
    Relation {
        kinds: &["use-case"],
        field: "doctrines",
        edge: "exercises",
        target: Target::ObjectOrName("rule", "doctrine"),
        inverted: false,
    },
    // the claims of docs/CLAIMS.yaml are indexed objects, so a use case's claim is the
    // claim itself rather than a string that looks like one
    Relation {
        kinds: &["use-case"],
        field: "claims",
        edge: "evidences",
        target: Target::Object("claim"),
        inverted: false,
    },
    Relation {
        kinds: &[],
        field: "tracks",
        edge: "tracks",
        target: Target::Path,
        inverted: false,
    },
    // A claim names the file that defines it, the one that implements it and the case
    // that proves it. The layer indexes all three as objects for exactly this reason:
    // sources.yaml says the implementation and test classes exist so that the chain a
    // claim declares resolves to real nodes instead of to three strings.
    Relation {
        kinds: &["claim"],
        field: "source",
        edge: "defined_in",
        target: Target::Path,
        inverted: false,
    },
    Relation {
        kinds: &["claim"],
        field: "implementation",
        edge: "implemented_by",
        target: Target::Path,
        inverted: false,
    },
    Relation {
        kinds: &["claim"],
        field: "test",
        edge: "tested_by",
        target: Target::Path,
        inverted: false,
    },
    // The plan is a graph the repository already draws for itself; until now it was the
    // one graph missing from the graph.
    Relation {
        kinds: &["issue"],
        field: "milestone",
        edge: "belongs_to",
        target: Target::Object("milestone"),
        inverted: false,
    },
    Relation {
        kinds: &["issue"],
        field: "depends_on",
        edge: "depends_on",
        target: Target::Object("issue"),
        inverted: false,
    },
    Relation {
        kinds: &["milestone"],
        field: "depends_on",
        edge: "depends_on",
        target: Target::Object("milestone"),
        inverted: false,
    },
    Relation {
        kinds: &["milestone"],
        field: "claims",
        edge: "evidences",
        target: Target::Object("claim"),
        inverted: false,
    },
    // An application and a use case name each other; the relationship is one, and it runs
    // from the scenario to the situation it serves.
    Relation {
        kinds: &["use-case"],
        field: "applications",
        edge: "serves",
        target: Target::Object("application"),
        inverted: false,
    },
    Relation {
        kinds: &["application"],
        field: "use_cases",
        edge: "serves",
        target: Target::Object("use-case"),
        inverted: true,
    },
    Relation {
        kinds: &["application"],
        field: "doctrines",
        edge: "governed_by",
        target: Target::ObjectOrName("rule", "doctrine"),
        inverted: false,
    },
    // A use case that names an MCP tool names a capability of this registry, under the
    // name the registry exposes it by.
    Relation {
        kinds: &["use-case"],
        field: "mcp_tools",
        edge: "exercises",
        target: Target::McpTool,
        inverted: false,
    },
];

/// One reference that names something this repository does not hold.
///
/// A reference to a file or a test outside the layer is not one of these: it resolves to
/// an external node, which is what an edge out of the layer looks like. This is the other
/// case — a rule, a decision or a capability named by identity that no object and no
/// declaration answers to, which is a defect in the naming rather than a boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct Unresolved {
    /// The repository-relative file that declared it.
    pub declared_in: String,
    /// The front matter key it was declared under.
    pub key: String,
    /// What was named.
    pub reference: String,
    /// What to do about it.
    pub correction: String,
}

/// Every reference the layer declares that resolves to nothing this repository holds.
///
/// Empty is the healthy answer. A consumer that generates an artifact from the graph
/// refuses to write one while this is not empty, which is why the finding names the file,
/// the key and the correction rather than only the missing name.
pub fn unresolved_relations(registry: &CapabilityRegistry, objects: &[Object]) -> Vec<Unresolved> {
    let r = Resolver::new(registry, objects);
    let mut out = Vec::new();
    for o in objects {
        for rel in RELATIONS {
            if !rel.kinds.is_empty() && !rel.kinds.contains(&o.kind.as_str()) {
                continue;
            }
            for reference in metadata_strings(&o.metadata, rel.field) {
                if is_absence(&reference) {
                    continue;
                }
                if let Outcome::Missing(correction) = r.resolve(rel, &reference) {
                    out.push(Unresolved {
                        declared_in: o.provenance.path.clone(),
                        key: rel.field.into(),
                        reference,
                        correction,
                    });
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// What a reference resolved to.
enum Outcome {
    /// A node of the graph, by id.
    Node(String),
    /// Something outside the layer, drawn as an external node.
    External(Node),
    /// Nothing, with what to do about it.
    Missing(String),
}

/// The index read once into the lookups every reference needs, so that resolving three
/// hundred references does not walk eight hundred objects three hundred times.
struct Resolver<'a> {
    registry: &'a CapabilityRegistry,
    by_kind_identity: BTreeMap<(&'a str, &'a str), &'a Object>,
    by_kind_stem: BTreeMap<(&'a str, &'a str), &'a Object>,
    by_path: BTreeMap<&'a str, &'a Object>,
    adr_by_declared_id: BTreeMap<String, &'a Object>,
    kinds: BTreeSet<&'a str>,
}

impl<'a> Resolver<'a> {
    fn new(registry: &'a CapabilityRegistry, objects: &'a [Object]) -> Self {
        let mut by_kind_identity = BTreeMap::new();
        let mut by_kind_stem = BTreeMap::new();
        let mut by_path = BTreeMap::new();
        let mut adr_by_declared_id = BTreeMap::new();
        let mut kinds = BTreeSet::new();
        for o in objects {
            kinds.insert(o.kind.as_str());
            by_kind_identity.insert((o.kind.as_str(), o.identity.as_str()), o);
            // an identity carries its version (`project.thing@1`); a reference names the
            // rule, not the version it was written against
            if let Some((stem, _)) = o.identity.split_once('@') {
                by_kind_stem.entry((o.kind.as_str(), stem)).or_insert(o);
            }
            by_path.insert(o.provenance.path.as_str(), o);
            if o.kind == "adr" {
                if let Some(id) = metadata_string(&o.metadata, "id") {
                    adr_by_declared_id.insert(id, o);
                }
            }
        }
        Resolver {
            registry,
            by_kind_identity,
            by_kind_stem,
            by_path,
            adr_by_declared_id,
            kinds,
        }
    }

    fn object(&self, kind: &str, name: &str) -> Option<&'a Object> {
        self.by_kind_identity
            .get(&(kind, name))
            .or_else(|| self.by_kind_stem.get(&(kind, name)))
            .copied()
    }

    fn resolve(&self, rel: &Relation, reference: &str) -> Outcome {
        match rel.target {
            Target::Object(kind) => match self.object(kind, reference) {
                Some(o) => Outcome::Node(o.uri.clone()),
                None => Outcome::Missing(format!(
                    "no object of kind `{kind}` has the identity `{reference}`; register it or correct the name"
                )),
            },
            Target::DeclaredAdr => match self.adr_by_declared_id.get(reference) {
                Some(o) => Outcome::Node(o.uri.clone()),
                None => Outcome::Missing(format!(
                    "no decision declares the id `{reference}`; correct the name or record the decision"
                )),
            },
            Target::Prefixed => {
                let Some((prefix, rest)) = reference.split_once(':') else {
                    return Outcome::Missing(format!(
                        "`{reference}` is not `<kind>:<name>`; a reference names what kind of thing it points at"
                    ));
                };
                // a path out of the layer is a boundary, not a defect
                if prefix == "file" || prefix == "test" {
                    return match self.by_path.get(rest) {
                        Some(o) => Outcome::Node(o.uri.clone()),
                        None => Outcome::External(external_node(prefix, rest, Some(rest))),
                    };
                }
                match self.object(prefix, rest) {
                    Some(o) => Outcome::Node(o.uri.clone()),
                    None if !self.kinds.contains(prefix) => {
                        // a kind this repository holds no objects of at all: the claims of
                        // docs/CLAIMS.yaml are named this way and are not indexed objects
                        Outcome::External(external_node(prefix, rest, None))
                    }
                    None => Outcome::Missing(format!(
                        "no object of kind `{prefix}` has the identity `{rest}`; register it or correct the reference"
                    )),
                }
            }
            Target::ObjectOrName(kind, fallback) => match self.object(kind, reference) {
                Some(o) => Outcome::Node(o.uri.clone()),
                None => Outcome::External(external_node(fallback, reference, None)),
            },
            Target::Name(kind) => Outcome::External(external_node(kind, reference, None)),
            Target::Path => match self.by_path.get(reference) {
                Some(o) => Outcome::Node(o.uri.clone()),
                None => Outcome::External(external_node("file", reference, Some(reference))),
            },
            Target::McpTool => match self.registry.by_mcp_tool(reference) {
                Some(c) => Outcome::Node(format!("capability:{}", c.id)),
                None => Outcome::Missing(format!(
                    "no capability is exposed as the MCP tool `{reference}`; correct the name or declare the exposure"
                )),
            },
        }
    }
}

/// A node for something the graph names and this repository does not hold as an object.
fn external_node(kind: &str, name: &str, path: Option<&str>) -> Node {
    Node {
        id: format!("{kind}:{name}"),
        kind: kind.into(),
        label: name.rsplit('/').next().unwrap_or(name).to_string(),
        summary: Some("named by the layer; not an object of this index".to_string()),
        route: None,
        source: path.map(str::to_string),
        status: None,
        external: true,
        facts: BTreeMap::new(),
    }
}

/// What is true of a node in a running process, and of nothing on a published page.
///
/// A graph is definitions: they are the same in a static build and in a server, and they
/// are what `compose` derives. This is the overlay a consumer with a process to ask
/// lays over them, keyed by node id. It is deliberately not a field of [`Node`]: a node
/// that could carry runtime state would carry it into the static projection, where it
/// would be a value nobody can refresh and a reader cannot distinguish from a current
/// one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeState {
    /// Node id to what the process says about it now.
    pub nodes: BTreeMap<String, NodeState>,
}

/// One node's runtime state: what a process observed, never what a file declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NodeState {
    /// A status word the observing process defines.
    pub status: String,
    /// One line about the observation, when there is something to say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A string field of an object's parsed front matter.
fn metadata_string(metadata: &Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// A list-of-strings field of an object's parsed front matter; empty when absent or of
/// another shape. A single string is read as a list of one, because the layer's YAML
/// subset writes both.
fn metadata_strings(metadata: &Value, key: &str) -> Vec<String> {
    match metadata.get(key) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn kind_word(kind: CapabilityKind) -> String {
    kind.as_str().into()
}

fn enum_word<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Where the Cockpit shows an object of the index, and where it shows a capability: the
/// one place a route is built from an identity, so a page that moves moves once.
pub fn routes(object: &Object) -> String {
    object_route(&object.uri)
}

/// The Cockpit route of a capability.
pub fn capability_page(c: &Capability) -> String {
    capability_route(c.id.as_str())
}

/// The Why catalogue as a graph: every operational moment with the audiences that
/// recognise it, the areas it falls under, and the mechanisms of this repository it names
/// — the commands, the capabilities of this executable, the claims, the rules and the use
/// cases. Read backwards it answers the question a page asks: which moments does this
/// capability answer, and which does this rule govern.
///
/// Derived from the front matter of the moments and from nothing else. No edge here is
/// authored twice: a moment names an audience, and the audience's membership is this
/// graph read in the other direction.
fn why_graph(registry: &CapabilityRegistry, index: &Index) -> Graph {
    let catalogue = crate::why::Catalogue::build(index, registry);
    let mut b = Builder::new(
        "why",
        "The moments, and what answers them",
        "Every operational moment this tool is a response to, with who recognises it, the operational area it falls under, and the commands, capabilities, claims, rules and use cases that answer it. A moment that named something this repository does not have would fail validation rather than draw an edge to nothing.",
        "the front matter of every object of kind `moment`",
    )
    .node_kind("moment", "one operational failure mode")
    .node_kind("audience", "who recognises it")
    .node_kind("area", "the operational area it falls under")
    .node_kind("command", "a command that answers it")
    .node_kind("capability", "a capability of this executable that answers it")
    .node_kind("claim", "a claim that says what is guaranteed here")
    .node_kind("rule", "a rule of the effective set that governs it")
    .node_kind("use-case", "a use case that shows the way out")
    .edge_kind("recognised_by", "that audience recognises this moment")
    .edge_kind("falls_under", "this moment falls under that area")
    .edge_kind("answered_by", "that command or capability answers this moment")
    .edge_kind("backed_by", "that claim says what is guaranteed here")
    .edge_kind("governed_by", "that rule governs this moment")
    .edge_kind("resolved_by", "that use case shows the way out")
    .edge_kind("related_to", "the moment names that one as related");

    'outer: for m in catalogue
        .all()
        .iter()
        .filter(|m| m.status == crate::why::STABLE)
    {
        let id = format!("moment:{}", m.id);
        if !b.node(Node {
            id: id.clone(),
            kind: "moment".into(),
            label: m.label().to_string(),
            summary: Some(m.summary.clone()),
            route: Some(m.route.clone()),
            source: Some(m.source.clone()),
            status: Some(m.severity.clone()),
            external: false,
            // the moment's own vocabulary is not modelled as facts yet; an empty map is
            // the honest statement, and `skip_serializing_if` keeps it out of the output
            facts: Default::default(),
        }) {
            break;
        }
        for (values, kind, edge) in [
            (&m.audiences, "audience", "recognised_by"),
            (&m.areas, "area", "falls_under"),
            (&m.commands, "command", "answered_by"),
            (&m.capabilities, "capability", "answered_by"),
            (&m.claims, "claim", "backed_by"),
            (&m.doctrines, "rule", "governed_by"),
            (&m.use_cases, "use-case", "resolved_by"),
            (&m.related, "moment", "related_to"),
        ] {
            for name in values {
                let target = format!("{kind}:{name}");
                if !b.has(&target) {
                    let route = match kind {
                        "audience" => Some(format!("{}audiences/{name}/", crate::why::ROUTE)),
                        "area" => Some(format!("{}areas/{name}/", crate::why::ROUTE)),
                        "moment" => Some(format!("{}{name}/", crate::why::ROUTE)),
                        "command" => Some(format!("/commands/{name}/")),
                        "claim" => Some(format!("/guarantees/{name}/")),
                        "use-case" => Some(format!("/use-cases/{name}/")),
                        _ => None,
                    };
                    if !b.node(Node {
                        id: target.clone(),
                        kind: kind.into(),
                        label: name.clone(),
                        summary: None,
                        route,
                        source: None,
                        status: None,
                        external: false,
                        facts: Default::default(),
                    }) {
                        break 'outer;
                    }
                }
                b.edge(&id, &target, edge);
            }
        }
    }
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> Node {
        Node {
            id: id.into(),
            kind: "x".into(),
            label: id.into(),
            summary: None,
            route: None,
            source: None,
            status: None,
            external: false,
            facts: BTreeMap::new(),
        }
    }

    #[test]
    fn an_edge_to_a_missing_node_is_dropped() {
        let mut b = Builder::new("t", "T", "d", "s");
        b.node(node("a"));
        b.edge("a", "gone", "k");
        let g = b.finish();
        assert_eq!(g.edges, vec![]);
        assert!(g.metadata.acyclic);
    }

    #[test]
    fn a_cycle_is_reported_and_not_hidden() {
        let mut b = Builder::new("t", "T", "d", "s");
        b.node(node("a"));
        b.node(node("b"));
        b.edge("a", "b", "k");
        b.edge("b", "a", "k");
        assert!(!b.finish().metadata.acyclic);
    }

    #[test]
    fn nodes_are_sorted_and_deduplicated() {
        let mut b = Builder::new("t", "T", "d", "s");
        b.node(node("b"));
        b.node(node("a"));
        b.node(node("a"));
        let g = b.finish();
        assert_eq!(
            g.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
    }

    #[test]
    fn metadata_strings_reads_a_list_and_a_lone_string() {
        let v = serde_json::json!({ "a": ["x", "y"], "b": "z", "c": 1 });
        assert_eq!(metadata_strings(&v, "a"), ["x", "y"]);
        assert_eq!(metadata_strings(&v, "b"), ["z"]);
        assert!(metadata_strings(&v, "c").is_empty());
        assert!(metadata_strings(&v, "missing").is_empty());
    }
    fn object(kind: &str, identity: &str, title: &str) -> Object {
        Object {
            kind: kind.into(),
            identity: identity.into(),
            uri: format!("majordomus://{kind}/{identity}"),
            title: Some(title.into()),
            description: None,
            metadata: serde_json::json!({}),
            body: String::new(),
            content: String::new(),
            media_type: "text/markdown",
            provenance: crate::model::Provenance {
                path: format!(".ai/repo/{kind}s/{identity}.md"),
                directory: format!(".ai/repo/{kind}s"),
                source_class: kind.into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    fn registry() -> CapabilityRegistry {
        CapabilityRegistry::builder()
            .with_builtin(crate::capability::builtin::all())
            .build()
            .unwrap()
    }

    #[test]
    fn a_kind_nobody_wrote_here_is_still_a_kind_of_the_graph() {
        // the point of the composition: the derivation names no kind, so a kind the layer
        // gains appears without this file being edited
        let g = compose(&registry(), &[object("gizmo", "one", "A gizmo")]);
        assert!(g.node_kinds.contains_key("gizmo"));
        assert!(g.nodes.iter().any(|n| n.id == "kind:gizmo"));
        assert!(g
            .nodes
            .iter()
            .any(|n| n.id == "majordomus://gizmo/one" && n.kind == "gizmo"));
        assert!(g
            .edges
            .iter()
            .any(|e| e.source == "majordomus://gizmo/one" && e.target == "kind:gizmo"));
    }

    #[test]
    fn an_object_is_identified_by_its_uri_and_not_by_its_title() {
        let before = compose(&registry(), &[object("rule", "one", "The old words")]);
        let after = compose(&registry(), &[object("rule", "one", "Entirely new words")]);
        let id = |g: &Graph| {
            g.nodes
                .iter()
                .find(|n| n.kind == "rule")
                .map(|n| n.id.clone())
        };
        assert_eq!(id(&before), id(&after));
        assert_eq!(id(&before).as_deref(), Some("majordomus://rule/one"));
    }

    #[test]
    fn many_objects_of_one_kind_share_the_one_kind_node() {
        let g = compose(
            &registry(),
            &[
                object("rule", "one", "One"),
                object("rule", "two", "Two"),
                object("rule", "three", "Three"),
            ],
        );
        assert_eq!(g.nodes.iter().filter(|n| n.id == "kind:rule").count(), 1);
        assert_eq!(g.nodes.iter().filter(|n| n.kind == "rule").count(), 3);
    }

    #[test]
    fn the_composition_does_not_depend_on_the_order_it_read_things_in() {
        let r = registry();
        let forwards = vec![
            object("rule", "a", "A"),
            object("adr", "b", "B"),
            object("skill", "c", "C"),
        ];
        let backwards: Vec<Object> = forwards.iter().rev().cloned().collect();
        assert_eq!(compose(&r, &forwards), compose(&r, &backwards));
    }

    #[test]
    fn nodes_and_edges_come_out_sorted() {
        let g = compose(
            &registry(),
            &[object("rule", "z", "Z"), object("rule", "a", "A")],
        );
        let mut sorted = g.nodes.clone();
        sorted.sort();
        assert_eq!(g.nodes, sorted);
        let mut edges = g.edges.clone();
        edges.sort();
        assert_eq!(g.edges, edges);
    }

    #[test]
    fn every_capability_reaches_its_module_and_its_projections() {
        let g = compose(&registry(), &[]);
        let c = g
            .nodes
            .iter()
            .find(|n| n.kind == "capability")
            .expect("the registry composes capabilities");
        assert!(g
            .edges
            .iter()
            .any(|e| e.target == c.id && e.kind == "composes"));
        assert!(g
            .edges
            .iter()
            .any(|e| e.source == c.id && e.target == "projection:cockpit"));
    }

    #[test]
    fn the_graph_holds_definitions_and_the_overlay_holds_what_a_process_saw() {
        let g = compose(&registry(), &[object("rule", "one", "One")]);
        let json = serde_json::to_string(&g).unwrap();
        assert!(!json.contains("\"runtime\""));
        let mut state = RuntimeState::default();
        state.nodes.insert(
            "majordomus://rule/one".into(),
            NodeState {
                status: "ok".into(),
                detail: None,
            },
        );
        assert_eq!(state.nodes.len(), 1);
    }
    fn object_with(kind: &str, identity: &str, metadata: serde_json::Value) -> Object {
        Object {
            metadata,
            ..object(kind, identity, "A thing")
        }
    }

    #[test]
    fn a_rule_reaches_the_rule_it_depends_on_across_its_version() {
        // identities carry a version; a reference names the rule, not the version it was
        // written against
        let objects = vec![
            object_with("rule", "project.first@1", serde_json::json!({})),
            object_with(
                "rule",
                "project.second@1",
                serde_json::json!({ "depends_on": ["project.first"] }),
            ),
        ];
        let g = compose(&registry(), &objects);
        assert!(g
            .edges
            .iter()
            .any(|e| e.source == "majordomus://rule/project.second@1"
                && e.target == "majordomus://rule/project.first@1"
                && e.kind == "depends_on"));
        assert_eq!(unresolved_relations(&registry(), &objects), vec![]);
    }

    #[test]
    fn a_reference_out_of_the_layer_is_a_boundary_and_not_a_finding() {
        let objects = vec![object_with(
            "adr",
            "0001-a-decision",
            serde_json::json!({ "id": "adr-0001", "related": ["file:docs/COCKPIT.md"] }),
        )];
        let g = compose(&registry(), &objects);
        let node = g
            .nodes
            .iter()
            .find(|n| n.id == "file:docs/COCKPIT.md")
            .expect("a file the layer names is drawn as an external node");
        assert!(node.external);
        assert!(g
            .edges
            .iter()
            .any(|e| e.target == node.id && e.kind == "put_in_force"));
        assert_eq!(unresolved_relations(&registry(), &objects), vec![]);
    }

    #[test]
    fn a_reference_to_a_kind_we_hold_that_names_nothing_is_a_finding() {
        let objects = vec![
            object_with("rule", "project.real@1", serde_json::json!({})),
            object_with(
                "adr",
                "0001-a-decision",
                serde_json::json!({ "id": "adr-0001", "related": ["rule:project.imaginary"] }),
            ),
        ];
        let found = unresolved_relations(&registry(), &objects);
        assert_eq!(found.len(), 1, "one reference names nothing: {found:?}");
        let f = &found[0];
        assert_eq!(f.declared_in, ".ai/repo/adrs/0001-a-decision.md");
        assert_eq!(f.key, "related");
        assert_eq!(f.reference, "rule:project.imaginary");
        assert!(
            f.correction.contains("project.imaginary") && f.correction.contains("register"),
            "the correction says what to do: {}",
            f.correction
        );
    }

    #[test]
    fn a_finding_is_never_drawn_as_an_edge() {
        let objects = vec![
            object_with("rule", "project.real@1", serde_json::json!({})),
            object_with(
                "adr",
                "0001-a-decision",
                serde_json::json!({ "id": "adr-0001", "related": ["rule:project.imaginary"] }),
            ),
        ];
        let g = compose(&registry(), &objects);
        assert!(
            !g.nodes.iter().any(|n| n.label == "project.imaginary"),
            "a reference that resolves to nothing is a finding, not a phantom node"
        );
        assert!(!g.edges.iter().any(|e| e.kind == "put_in_force"));
    }

    #[test]
    fn a_decision_stands_in_for_the_one_it_names_by_declared_id() {
        let objects = vec![
            object_with(
                "adr",
                "0001-the-first",
                serde_json::json!({ "id": "adr-0001" }),
            ),
            object_with(
                "adr",
                "0002-the-second",
                serde_json::json!({ "id": "adr-0002", "supersedes": ["adr-0001"] }),
            ),
        ];
        let g = compose(&registry(), &objects);
        assert!(g
            .edges
            .iter()
            .any(|e| e.source == "majordomus://adr/0002-the-second"
                && e.target == "majordomus://adr/0001-the-first"
                && e.kind == "supersedes"));
    }

    #[test]
    fn a_use_case_reaches_what_it_runs_exercises_and_evidences() {
        let objects = vec![
            object_with("rule", "majordomus.a-doctrine@1", serde_json::json!({})),
            object_with("claim", "claim-one", serde_json::json!({})),
            object_with(
                "use-case",
                "a-scenario",
                serde_json::json!({
                    "commands": ["doctor"],
                    "doctrines": ["majordomus.a-doctrine"],
                    "claims": ["claim-one"],
                }),
            ),
        ];
        let g = compose(&registry(), &objects);
        let from = "majordomus://use-case/a-scenario";
        let edge = |kind: &str| g.edges.iter().find(|e| e.source == from && e.kind == kind);
        assert_eq!(
            edge("runs").map(|e| e.target.as_str()),
            Some("command:doctor")
        );
        assert_eq!(
            edge("exercises").map(|e| e.target.as_str()),
            Some("majordomus://rule/majordomus.a-doctrine@1"),
            "a doctrine the layer holds resolves to the rule itself"
        );
        assert_eq!(
            edge("evidences").map(|e| e.target.as_str()),
            Some("majordomus://claim/claim-one"),
            "the claims of docs/CLAIMS.yaml are indexed objects, so the edge reaches the claim itself"
        );
        assert_eq!(unresolved_relations(&registry(), &objects), vec![]);
    }

    #[test]
    fn every_edge_has_both_ends_in_the_graph() {
        let objects = vec![
            object_with(
                "context",
                "a-document",
                serde_json::json!({ "tracks": ["lib/session.sh"] }),
            ),
            object_with("rule", "project.real@1", serde_json::json!({})),
        ];
        let g = compose(&registry(), &objects);
        let ids: BTreeSet<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        for e in &g.edges {
            assert!(ids.contains(e.source.as_str()), "source missing: {e:?}");
            assert!(ids.contains(e.target.as_str()), "target missing: {e:?}");
        }
        assert!(g.edges.iter().any(|e| e.kind == "tracks"));
    }
    #[test]
    fn a_skill_names_its_sibling_by_identity_and_that_resolves() {
        // the convention this test pins was learned from the layer rather than assumed:
        // read as `<kind>:<name>` these two references were findings, and they were not
        let objects = vec![
            object_with("skill", "repo-review", serde_json::json!({})),
            object_with(
                "skill",
                "implement",
                serde_json::json!({ "related": ["repo-review"] }),
            ),
        ];
        let g = compose(&registry(), &objects);
        assert!(g
            .edges
            .iter()
            .any(|e| e.source == "majordomus://skill/implement"
                && e.target == "majordomus://skill/repo-review"
                && e.kind == "related_to"));
        assert_eq!(unresolved_relations(&registry(), &objects), vec![]);
    }
    #[test]
    fn a_rule_carries_its_class_and_its_statement_and_an_issue_does_not_carry_its_prose() {
        let long = "x".repeat(FACT_LIMIT + 1);
        let objects = vec![
            object_with(
                "rule",
                "project.thing@1",
                serde_json::json!({
                    "id": "project.thing",
                    "class": "blocking",
                    "statement": "A capability is defined once and every interface is derived from it.",
                    "tags": ["architecture", "capabilities"],
                }),
            ),
            object_with(
                "issue",
                "I0001",
                serde_json::json!({ "priority": "p0", "objective": long }),
            ),
        ];
        let g = compose(&registry(), &objects);
        let rule = g
            .nodes
            .iter()
            .find(|n| n.kind == "rule")
            .expect("the rule is a node");
        assert_eq!(
            rule.facts.get("class"),
            Some(&Fact::Text("blocking".into())),
            "how a rule is enforced is a fact about it"
        );
        assert!(matches!(rule.facts.get("statement"), Some(Fact::Text(_))));
        assert_eq!(
            rule.facts.get("tags"),
            Some(&Fact::List(vec![
                "architecture".into(),
                "capabilities".into()
            ]))
        );
        assert!(
            !rule.facts.contains_key("id") && !rule.facts.contains_key("title"),
            "the node already shows these; carrying them again says one thing twice"
        );
        let issue = g.nodes.iter().find(|n| n.kind == "issue").unwrap();
        assert_eq!(issue.facts.get("priority"), Some(&Fact::Text("p0".into())));
        assert!(
            !issue.facts.contains_key("objective"),
            "prose belongs to the object, which the node's route reaches"
        );
    }

    #[test]
    fn a_claim_reaches_what_defines_implements_and_proves_it() {
        // sources.yaml indexes implementations and tests so that this chain resolves to
        // real nodes instead of to three strings
        let mut implementation = object("implementation", "lib/session.sh", "session");
        implementation.provenance.path = "lib/session.sh".into();
        let mut case = object("test", "test/cases/01_a.sh", "a case");
        case.provenance.path = "test/cases/01_a.sh".into();
        let mut source = object("document", "docs/A.md", "a document");
        source.provenance.path = "docs/A.md".into();
        let objects = vec![
            implementation,
            case,
            source,
            object_with(
                "claim",
                "a-claim",
                serde_json::json!({
                    "source": "docs/A.md",
                    "implementation": "lib/session.sh",
                    "test": "test/cases/01_a.sh",
                }),
            ),
        ];
        let g = compose(&registry(), &objects);
        let from = "majordomus://claim/a-claim";
        for (edge, target) in [
            ("defined_in", "majordomus://document/docs/A.md"),
            (
                "implemented_by",
                "majordomus://implementation/lib/session.sh",
            ),
            ("tested_by", "majordomus://test/test/cases/01_a.sh"),
        ] {
            assert!(
                g.edges
                    .iter()
                    .any(|e| e.source == from && e.kind == edge && e.target == target),
                "the claim should reach {target} by {edge}"
            );
        }
        assert_eq!(unresolved_relations(&registry(), &objects), vec![]);
    }

    #[test]
    fn a_claim_that_nothing_implements_yet_says_so_and_that_is_not_a_finding() {
        let objects = vec![object_with(
            "claim",
            "a-claim",
            serde_json::json!({ "implementation": "-", "test": "-" }),
        )];
        let g = compose(&registry(), &objects);
        assert!(
            !g.edges.iter().any(|e| e.kind == "implemented_by"),
            "an absence is not a reference"
        );
        assert_eq!(unresolved_relations(&registry(), &objects), vec![]);
    }

    #[test]
    fn the_plan_is_a_graph_of_the_graph() {
        let objects = vec![
            object_with("milestone", "a-milestone", serde_json::json!({})),
            object_with(
                "issue",
                "I0001",
                serde_json::json!({ "milestone": "a-milestone" }),
            ),
            object_with(
                "issue",
                "I0002",
                serde_json::json!({ "milestone": "a-milestone", "depends_on": ["I0001"] }),
            ),
        ];
        let g = compose(&registry(), &objects);
        assert!(g
            .edges
            .iter()
            .any(|e| e.source == "majordomus://issue/I0002"
                && e.target == "majordomus://issue/I0001"
                && e.kind == "depends_on"));
        assert_eq!(
            g.edges
                .iter()
                .filter(
                    |e| e.kind == "belongs_to" && e.target == "majordomus://milestone/a-milestone"
                )
                .count(),
            2
        );
        assert_eq!(unresolved_relations(&registry(), &objects), vec![]);
    }

    #[test]
    fn one_relationship_gets_one_edge_however_many_files_declare_it() {
        // the application names the use case and the use case names the application; that
        // is one relationship, and it runs from the scenario to the situation it serves
        let objects = vec![
            object_with(
                "application",
                "an-application",
                serde_json::json!({ "use_cases": ["a-scenario"] }),
            ),
            object_with(
                "use-case",
                "a-scenario",
                serde_json::json!({ "applications": ["an-application"] }),
            ),
        ];
        let g = compose(&registry(), &objects);
        let serves: Vec<&Edge> = g.edges.iter().filter(|e| e.kind == "serves").collect();
        assert_eq!(serves.len(), 1, "one relationship, one edge: {serves:?}");
        assert_eq!(serves[0].source, "majordomus://use-case/a-scenario");
        assert_eq!(serves[0].target, "majordomus://application/an-application");
    }

    #[test]
    fn a_use_case_reaches_the_capability_behind_the_tool_it_names() {
        let objects = vec![object_with(
            "use-case",
            "a-scenario",
            serde_json::json!({ "mcp_tools": ["majordomus_get"] }),
        )];
        let g = compose(&registry(), &objects);
        assert!(
            g.edges
                .iter()
                .any(|e| e.source == "majordomus://use-case/a-scenario"
                    && e.kind == "exercises"
                    && e.target == "capability:objects.get"),
            "the registry knows which capability answers to a tool name"
        );
        let named_wrongly = vec![object_with(
            "use-case",
            "a-scenario",
            serde_json::json!({ "mcp_tools": ["majordomus_imaginary"] }),
        )];
        let found = unresolved_relations(&registry(), &named_wrongly);
        assert_eq!(found.len(), 1);
        assert!(found[0].correction.contains("MCP tool"));
    }
}
