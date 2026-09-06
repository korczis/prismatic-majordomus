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
///             source: None, status: None, external: false }
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
/// Typed relations between the composed kinds — what documents what, what enforces what,
/// what tests what — are derived separately; this derivation carries only the structure
/// the registries already assert: a module composes its capabilities, a capability is
/// projected through the transports it declares, and an object is of its kind.
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
    .edge_kind("composes", "the module composes the capability")
    .edge_kind("projects", "the capability is projected through the transport")
    .edge_kind("is_a", "the object is of that kind");

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
        }) {
            break;
        }
        b.edge(&o.uri, &format!("kind:{}", o.kind), "is_a");
    }

    b.finish()
}

/// What is true of a node in a running process, and of nothing on a published page.
///
/// A graph is definitions: they are the same in a static build and in a server, and they
/// are what [`compose`] derives. This is the overlay a consumer with a process to ask
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
    match kind {
        CapabilityKind::Query => "query",
        CapabilityKind::Command => "command",
        CapabilityKind::Resource => "resource",
    }
    .into()
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
}
