//! Reading the model: listing, searching, showing one node with everything that bears on
//! it, explaining why the model says what it says, walking the graph, and assembling the
//! context an agent should read before touching a path. Every function here is a pure
//! function of the model; the capabilities and the command line call them and render.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::{
    Claim, Conflict, Evidence, Freshness, Gap, KnowledgeModel, Node, Ownership, Provenance,
    Relation, Visibility,
};
use super::Adjacency;

/// A filter over nodes. Every field is optional; an unset one matches everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeFilter")]
pub struct Filter {
    /// One node kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// One provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    /// One freshness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<Freshness>,
    /// One ownership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ownership: Option<Ownership>,
    /// One visibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    /// One extractor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor: Option<String>,
    /// A substring of the id or the title, case-insensitive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Only nodes whose freshness is debt.
    #[serde(default)]
    pub debt: bool,
    /// Skip this many matches.
    #[serde(default)]
    pub offset: usize,
    /// Answer at most this many; `0` means the default.
    #[serde(default)]
    pub limit: usize,
}

/// The default page size.
pub const DEFAULT_LIMIT: usize = 100;

/// One node as a listing shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeSummary")]
pub struct Summary {
    /// The id.
    pub id: String,
    /// The kind.
    pub kind: String,
    /// The title.
    pub title: String,
    /// The provenance.
    pub provenance: Provenance,
    /// The ownership.
    pub ownership: Ownership,
    /// The freshness.
    pub freshness: Freshness,
    /// Why it is not current, when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_reason: Option<String>,
    /// How many claims it carries.
    pub claims: usize,
    /// The source path, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The Cockpit route, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

impl Summary {
    /// Summarise a node.
    pub fn of(n: &Node) -> Self {
        Summary {
            id: n.id.clone(),
            kind: n.kind.clone(),
            title: n.title.clone(),
            provenance: n.provenance,
            ownership: n.ownership,
            freshness: n.freshness,
            freshness_reason: n.freshness_reason.clone(),
            claims: n.claims.len(),
            source: n.source.clone(),
            route: n.route.clone(),
        }
    }
}

/// A page of a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgePage")]
pub struct Page {
    /// The nodes on the page.
    pub nodes: Vec<Summary>,
    /// How many matched in all.
    pub total: usize,
    /// The offset of the page.
    pub offset: usize,
    /// The page size asked for.
    pub limit: usize,
}

fn matches(n: &Node, f: &Filter) -> bool {
    if let Some(k) = &f.kind {
        if &n.kind != k {
            return false;
        }
    }
    if f.provenance.is_some_and(|p| p != n.provenance) {
        return false;
    }
    if f.freshness.is_some_and(|x| x != n.freshness) {
        return false;
    }
    if f.ownership.is_some_and(|x| x != n.ownership) {
        return false;
    }
    if f.visibility.is_some_and(|x| x != n.visibility) {
        return false;
    }
    if let Some(e) = &f.extractor {
        if &n.extractor != e {
            return false;
        }
    }
    if f.debt && !n.freshness.is_debt() {
        return false;
    }
    if let Some(q) = &f.query {
        let q = q.to_lowercase();
        if !q.is_empty() && !n.id.to_lowercase().contains(&q) && !n.title.to_lowercase().contains(&q) {
            return false;
        }
    }
    true
}

/// List the nodes a filter matches, one page.
pub fn list(model: &KnowledgeModel, f: &Filter) -> Page {
    let limit = if f.limit == 0 { DEFAULT_LIMIT } else { f.limit };
    let all: Vec<&Node> = model.nodes.iter().filter(|n| matches(n, f)).collect();
    let total = all.len();
    let nodes = all
        .into_iter()
        .skip(f.offset)
        .take(limit)
        .map(Summary::of)
        .collect();
    Page {
        nodes,
        total,
        offset: f.offset,
        limit,
    }
}

/// One search hit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeHit")]
pub struct Hit {
    /// The node.
    pub id: String,
    /// Its kind.
    pub kind: String,
    /// Its title.
    pub title: String,
    /// Its freshness.
    pub freshness: Freshness,
    /// Where the query matched: `id`, `title`, `summary`, `claim`.
    pub matched: String,
    /// The matching text, shortened.
    pub excerpt: String,
    /// Higher is better; deterministic.
    pub score: u32,
}

/// Search the model: ids and titles first, then summaries, then claim values. Ranked by
/// where the query matched, then by id, so that two runs agree.
pub fn search(model: &KnowledgeModel, query: &str, limit: usize) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let limit = if limit == 0 { 25 } else { limit };
    let mut hits = Vec::new();
    for n in &model.nodes {
        let id = n.id.to_lowercase();
        let title = n.title.to_lowercase();
        let (matched, excerpt, score) = if id == q || id.split_once(':').map(|(_, l)| l) == Some(&q) {
            ("id", n.id.clone(), 100)
        } else if title == q {
            ("title", n.title.clone(), 90)
        } else if id.contains(&q) {
            ("id", n.id.clone(), 70)
        } else if title.contains(&q) {
            ("title", n.title.clone(), 60)
        } else if n.summary.as_deref().is_some_and(|s| s.to_lowercase().contains(&q)) {
            ("summary", excerpt_of(n.summary.as_deref().unwrap_or(""), &q), 40)
        } else if let Some(c) = n.claims.iter().find(|c| value_text(&c.value).to_lowercase().contains(&q)) {
            ("claim", format!("{} = {}", c.predicate, excerpt_of(&value_text(&c.value), &q)), 20)
        } else {
            continue;
        };
        hits.push(Hit {
            id: n.id.clone(),
            kind: n.kind.clone(),
            title: n.title.clone(),
            freshness: n.freshness,
            matched: matched.into(),
            excerpt,
            score,
        });
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
    hits.truncate(limit);
    hits
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn excerpt_of(text: &str, q: &str) -> String {
    let lower = text.to_lowercase();
    let at = lower.find(q).unwrap_or(0);
    let start = text
        .char_indices()
        .map(|(i, _)| i)
        .filter(|&i| i <= at.saturating_sub(40))
        .last()
        .unwrap_or(0);
    let end = text
        .char_indices()
        .map(|(i, _)| i)
        .find(|&i| i >= at + q.len() + 60)
        .unwrap_or(text.len());
    let mut s = text[start..end].to_string();
    if start > 0 {
        s.insert(0, '…');
    }
    if end < text.len() {
        s.push('…');
    }
    s
}

/// One node with everything that bears on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeNodeView")]
pub struct NodeView {
    /// The node.
    pub node: Node,
    /// Its evidence, resolved.
    pub evidence: Vec<Evidence>,
    /// Relations leaving it.
    pub outgoing: Vec<Relation>,
    /// Relations entering it.
    pub incoming: Vec<Relation>,
    /// Conflicts it is the subject of.
    pub conflicts: Vec<Conflict>,
    /// Gaps about it.
    pub gaps: Vec<Gap>,
}

/// Show one node.
pub fn show(model: &KnowledgeModel, id: &str) -> Option<NodeView> {
    let node = model.node(id)?.clone();
    let evidence = node
        .evidence
        .iter()
        .chain(node.claims.iter().flat_map(|c| c.evidence.iter()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|e| model.evidence(e).cloned())
        .collect();
    Some(NodeView {
        outgoing: model.relations_from(id).cloned().collect(),
        incoming: model.relations_to(id).cloned().collect(),
        conflicts: model.conflicts.iter().filter(|c| c.subject == id).cloned().collect(),
        gaps: model.gaps.iter().filter(|g| g.subject == id || g.subject.starts_with(&format!("{id}#"))).cloned().collect(),
        evidence,
        node,
    })
}

/// One line of an explanation's evidence chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeEvidenceLine")]
pub struct EvidenceLine {
    /// The evidence id.
    pub id: String,
    /// Its kind.
    pub kind: String,
    /// The path, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The fingerprint, shortened.
    pub fingerprint: String,
    /// The extractor that read it.
    pub extractor: String,
    /// Whether it may leave the machine.
    pub remote_processing: bool,
}

/// One claim as an explanation states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeClaimLine")]
pub struct ClaimLine {
    /// The claim id.
    pub id: String,
    /// The predicate.
    pub predicate: String,
    /// The value.
    pub value: Value,
    /// The provenance.
    pub provenance: Provenance,
    /// The freshness.
    pub freshness: Freshness,
    /// The state.
    pub state: String,
    /// The evidence ids.
    pub evidence: Vec<String>,
    /// Why, when it is not current.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Why the model says what it says about one node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeExplanation")]
pub struct Explanation {
    /// The node.
    pub id: String,
    /// Its kind.
    pub kind: String,
    /// Its title.
    pub title: String,
    /// How it is known.
    pub provenance: Provenance,
    /// Who owns its source.
    pub ownership: Ownership,
    /// Who may see it.
    pub visibility: Visibility,
    /// Which extractor produced it.
    pub extractor: String,
    /// Where it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Its freshness.
    pub freshness: Freshness,
    /// Why, when it is not current.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_reason: Option<String>,
    /// The confidence, in words.
    pub confidence: String,
    /// The explanation, one sentence per line.
    pub why: Vec<String>,
    /// The evidence chain.
    pub evidence: Vec<EvidenceLine>,
    /// Every claim.
    pub claims: Vec<ClaimLine>,
    /// What it relates to, as `kind -> target`.
    pub relates_to: Vec<String>,
    /// What relates to it, as `source -> kind`.
    pub related_from: Vec<String>,
    /// Conflicts it is party to.
    pub conflicts: Vec<Conflict>,
    /// Gaps about it.
    pub gaps: Vec<Gap>,
    /// What to do, when something is wrong.
    pub remedies: Vec<String>,
}

/// Explain one node.
pub fn explain(model: &KnowledgeModel, id: &str) -> Option<Explanation> {
    let view = show(model, id)?;
    let n = &view.node;
    let mut why = Vec::new();
    why.push(match n.provenance {
        Provenance::Observed => format!(
            "{} was read off the tree by the `{}` extractor this scan; nobody wrote it down.",
            n.id, n.extractor
        ),
        Provenance::Declared => format!(
            "{} is declared in {} and validated against its kind's schema; the layer says so.",
            n.id,
            n.source.as_deref().unwrap_or("the layer")
        ),
        Provenance::Curated => format!(
            "{} is what a person wrote in {}; the tree does not say it, the record does.",
            n.id,
            n.source.as_deref().unwrap_or("a curated record")
        ),
        Provenance::Derived => format!(
            "{} was proposed by a semantic provider; it is an inference, not an observation.",
            n.id
        ),
    });
    why.push(format!(
        "It rests on {} piece(s) of evidence and carries {} claim(s).",
        view.evidence.len(),
        n.claims.len()
    ));
    why.push(match n.freshness {
        Freshness::Current => "Every claim is current: its evidence carries the fingerprint the claim was made against.".into(),
        f => format!(
            "It is {}{}.",
            f.as_str().replace('_', " "),
            n.freshness_reason.as_deref().map(|r| format!(" because {r}")).unwrap_or_default()
        ),
    });
    why.push(match n.ownership {
        Ownership::External => "Its source is owned outside Majordomus: the tool reports and proposes, never rewrites it.".into(),
        Ownership::Majordomus => "Its source is generated by Majordomus: regenerate it, never edit it.".into(),
        Ownership::Hybrid => "Its source is shared: Majordomus writes part of it and a person the rest.".into(),
    });
    let mut remedies = Vec::new();
    for g in &view.gaps {
        remedies.push(format!("{}: {}", g.id, g.remedy));
    }
    for c in &view.conflicts {
        remedies.push(format!("{}: {}", c.id, c.remedy));
    }
    if n.freshness == Freshness::Stale || n.freshness == Freshness::Unverified {
        if n.provenance == Provenance::Curated {
            remedies.push("check the record against its evidence, then run `majordomus knowledge reconcile --accept`".into());
        } else if n.provenance == Provenance::Derived {
            remedies.push("verify the derivation, then record the baseline".into());
        }
    }
    if n.freshness == Freshness::PossiblyStale {
        remedies.push("read what changed (`majordomus knowledge impact`) and update the text if it no longer holds, then record the baseline".into());
    }
    let confidence = format!(
        "{:?} ({})",
        n.confidence.level,
        n.confidence
            .basis
            .iter()
            .map(|b| format!("{b:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .to_lowercase();
    Some(Explanation {
        id: n.id.clone(),
        kind: n.kind.clone(),
        title: n.title.clone(),
        provenance: n.provenance,
        ownership: n.ownership,
        visibility: n.visibility,
        extractor: n.extractor.clone(),
        source: n.source.clone(),
        freshness: n.freshness,
        freshness_reason: n.freshness_reason.clone(),
        confidence,
        why,
        evidence: view
            .evidence
            .iter()
            .map(|e| EvidenceLine {
                id: e.id.clone(),
                kind: format!("{:?}", e.kind).to_lowercase(),
                path: e.locator.path.clone(),
                fingerprint: e.fingerprint.value.chars().take(12).collect(),
                extractor: e.extractor.clone(),
                remote_processing: e.remote_processing,
            })
            .collect(),
        claims: n.claims.iter().map(claim_line).collect(),
        relates_to: view.outgoing.iter().map(|r| format!("{} -> {}", r.kind, r.target)).collect(),
        related_from: view.incoming.iter().map(|r| format!("{} -> {}", r.source, r.kind)).collect(),
        conflicts: view.conflicts,
        gaps: view.gaps,
        remedies,
    })
}

fn claim_line(c: &Claim) -> ClaimLine {
    ClaimLine {
        id: c.id.clone(),
        predicate: c.predicate.clone(),
        value: c.value.clone(),
        provenance: c.provenance,
        freshness: c.freshness,
        state: format!("{:?}", c.state).to_lowercase(),
        evidence: c.evidence.clone(),
        reason: c.reason.clone(),
    }
}

/// A node of a graph slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeGraphNode")]
pub struct GraphNode {
    /// The id.
    pub id: String,
    /// The kind.
    pub kind: String,
    /// The title.
    pub title: String,
    /// The short label a renderer draws: the local part of the id.
    pub label: String,
    /// Where the Cockpit shows the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// The freshness.
    pub freshness: Freshness,
    /// The provenance.
    pub provenance: Provenance,
    /// Distance from the root, when the slice has one.
    pub depth: usize,
}

/// An edge of a graph slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeGraphEdge")]
pub struct GraphEdge {
    /// The source.
    pub source: String,
    /// The target.
    pub target: String,
    /// The kind.
    pub kind: String,
    /// The provenance.
    pub provenance: Provenance,
}

/// A slice of the knowledge graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeGraphSlice")]
pub struct GraphSlice {
    /// The root, when the slice was cut around one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The nodes.
    pub nodes: Vec<GraphNode>,
    /// The edges among them.
    pub edges: Vec<GraphEdge>,
    /// How many nodes the cut left out.
    pub omitted: usize,
}

/// The largest slice `graph` answers.
pub const MAX_SLICE: usize = 500;

/// Cut a slice: around a root to a depth, or the nodes a filter matches, capped.
pub fn graph(model: &KnowledgeModel, root: Option<&str>, depth: usize, filter: &Filter, limit: usize) -> GraphSlice {
    let limit = if limit == 0 { MAX_SLICE } else { limit.min(MAX_SLICE) };
    let adjacency = Adjacency::of(&model.relations);
    let mut chosen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut omitted = 0;
    if let Some(root) = root {
        let depth = if depth == 0 { 2 } else { depth };
        let mut queue = VecDeque::new();
        if model.node(root).is_some() {
            chosen.insert(root, 0);
            queue.push_back((root, 0));
        }
        while let Some((id, d)) = queue.pop_front() {
            if d >= depth {
                continue;
            }
            let neighbours = adjacency
                .outgoing
                .get(id)
                .into_iter()
                .flatten()
                .map(|r| r.target.as_str())
                .chain(adjacency.incoming.get(id).into_iter().flatten().map(|r| r.source.as_str()));
            for n in neighbours {
                if chosen.contains_key(n) {
                    continue;
                }
                if chosen.len() >= limit {
                    omitted += 1;
                    continue;
                }
                chosen.insert(n, d + 1);
                queue.push_back((n, d + 1));
            }
        }
    } else {
        for n in model.nodes.iter().filter(|n| matches(n, filter)) {
            if chosen.len() >= limit {
                omitted += 1;
                continue;
            }
            chosen.insert(n.id.as_str(), 0);
        }
    }
    let nodes = chosen
        .iter()
        .filter_map(|(id, d)| {
            model.node(id).map(|n| GraphNode {
                id: n.id.clone(),
                kind: n.kind.clone(),
                title: n.title.clone(),
                label: n.id.split_once(':').map(|(_, l)| l.to_string()).unwrap_or_else(|| n.id.clone()),
                route: Some(format!("/cockpit/knowledge/node?id={}", crate::http::router::percent_encode(&n.id))),
                freshness: n.freshness,
                provenance: n.provenance,
                depth: *d,
            })
        })
        .collect();
    let edges = model
        .relations
        .iter()
        .filter(|r| chosen.contains_key(r.source.as_str()) && chosen.contains_key(r.target.as_str()))
        .map(|r| GraphEdge {
            source: r.source.clone(),
            target: r.target.clone(),
            kind: r.kind.clone(),
            provenance: r.provenance,
        })
        .collect();
    GraphSlice {
        root: root.map(str::to_string),
        nodes,
        edges,
        omitted,
    }
}

/// What an agent should read before touching some paths: the nodes whose sources are
/// under them, what those relate to one hop out, the open conflicts and the gaps among
/// them — ordered so that the governing things come first, and cut to a budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeContextBundle")]
pub struct ContextBundle {
    /// The paths asked about.
    pub paths: Vec<String>,
    /// The budget in bytes of rendered JSON the bundle was cut to.
    pub budget: usize,
    /// Only public knowledge was included.
    pub public_only: bool,
    /// The nodes, most governing first.
    pub nodes: Vec<Summary>,
    /// The claims of those nodes that are not current: what the agent must not trust.
    pub caveats: Vec<ClaimLine>,
    /// Open conflicts among them.
    pub conflicts: Vec<Conflict>,
    /// Gaps about them.
    pub gaps: Vec<Gap>,
    /// How many nodes the budget left out.
    pub omitted: usize,
}

/// The order kinds are offered in: what governs before what describes before what is.
fn kind_rank(kind: &str) -> u8 {
    match kind {
        "context" => 0,
        "rule" => 1,
        "policy" | "scope" => 2,
        "adr" => 3,
        "knowledge" => 4,
        "use-case" | "application" => 5,
        "document" => 6,
        "component" | "capability" | "module" => 7,
        "workflow" | "deployment_manifest" | "artifact" => 8,
        _ => 9,
    }
}

/// Assemble the context for some paths.
pub fn context(model: &KnowledgeModel, paths: &[String], budget: usize, public_only: bool) -> ContextBundle {
    let budget = if budget == 0 { 16_000 } else { budget };
    let under = |p: &str| {
        paths.iter().any(|q| {
            let q = q.trim_end_matches('/');
            q.is_empty() || p == q || p.starts_with(&format!("{q}/"))
        })
    };
    let adjacency = Adjacency::of(&model.relations);
    let mut chosen: BTreeSet<&str> = BTreeSet::new();
    for n in &model.nodes {
        let hit = n.source.as_deref().is_some_and(under)
            || n.evidence.iter().any(|e| {
                model
                    .evidence(e)
                    .and_then(|e| e.locator.path.as_deref())
                    .is_some_and(under)
            });
        if hit {
            chosen.insert(n.id.as_str());
        }
    }
    let direct: Vec<&str> = chosen.iter().copied().collect();
    for id in direct {
        for r in adjacency.outgoing.get(id).into_iter().flatten() {
            chosen.insert(r.target.as_str());
        }
        for r in adjacency.incoming.get(id).into_iter().flatten() {
            if matches!(r.kind.as_str(), "governs" | "applies_to" | "describes" | "documents") {
                chosen.insert(r.source.as_str());
            }
        }
    }
    let mut nodes: Vec<&Node> = chosen
        .iter()
        .filter_map(|id| model.node(id))
        .filter(|n| !public_only || n.visibility == Visibility::Public)
        .collect();
    nodes.sort_by(|a, b| kind_rank(&a.kind).cmp(&kind_rank(&b.kind)).then(a.id.cmp(&b.id)));
    let mut out = Vec::new();
    let mut caveats = Vec::new();
    let mut used = 0usize;
    let mut omitted = 0;
    for n in nodes {
        let s = Summary::of(n);
        let cost = serde_json::to_string(&s).map(|t| t.len()).unwrap_or(0);
        if used + cost > budget {
            omitted += 1;
            continue;
        }
        used += cost;
        for c in n.claims.iter().filter(|c| c.freshness.is_debt()) {
            caveats.push(claim_line(c));
        }
        out.push(s);
    }
    let ids: BTreeSet<&str> = out.iter().map(|s| s.id.as_str()).collect();
    ContextBundle {
        paths: paths.to_vec(),
        budget,
        public_only,
        conflicts: model
            .conflicts
            .iter()
            .filter(|c| ids.contains(c.subject.as_str()) && c.resolution == super::model::Resolution::Open)
            .cloned()
            .collect(),
        gaps: model
            .gaps
            .iter()
            .filter(|g| ids.contains(g.subject.as_str()))
            .cloned()
            .collect(),
        nodes: out,
        caveats,
        omitted,
    }
}

/// The public projection of the model: every node, claim, evidence and relation whose
/// visibility is public, for a dataset that leaves the repository. Nothing internal or
/// restricted survives, and no evidence marked as not for remote processing.
pub fn public_projection(model: &KnowledgeModel) -> KnowledgeModel {
    let mut m = model.clone();
    m.evidence.retain(|e| e.visibility == Visibility::Public && e.remote_processing);
    let evidence_ids: BTreeSet<&str> = m.evidence.iter().map(|e| e.id.as_str()).collect();
    m.nodes.retain(|n| n.visibility == Visibility::Public);
    let node_ids: BTreeSet<String> = m.nodes.iter().map(|n| n.id.clone()).collect();
    for n in &mut m.nodes {
        n.evidence.retain(|e| evidence_ids.contains(e.as_str()));
        n.claims.retain(|c| c.provenance != Provenance::Derived || c.evidence.iter().all(|e| evidence_ids.contains(e.as_str())));
        for c in &mut n.claims {
            c.evidence.retain(|e| evidence_ids.contains(e.as_str()));
        }
    }
    m.relations.retain(|r| node_ids.contains(&r.source) && node_ids.contains(&r.target));
    m.conflicts.retain(|c| node_ids.contains(&c.subject));
    m.gaps.retain(|g| node_ids.contains(&g.subject) || g.subject.split('#').next().is_some_and(|s| node_ids.contains(s)));
    m.diagnostics.clear();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::extract::{claim, NodeSpec};

    fn model() -> KnowledgeModel {
        let mut m = crate::knowledge::freshness::tests_support::empty_model();
        let mut a = NodeSpec { id: "component:alpha".into(), title: "Alpha".into(), summary: Some("the first thing".into()), provenance: Provenance::Observed, ownership: Ownership::External, visibility: Visibility::Public, evidence: vec![], source: Some("apps/alpha/Cargo.toml".into()), extractor: "t" }.build();
        a.claims.push(claim("component:alpha", "version", Value::String("1.2.3".into()), Provenance::Observed, vec![], super::super::model::Verification::Content));
        a.freshness = Freshness::Current;
        let mut b = NodeSpec { id: "knowledge:secret-note".into(), title: "note".into(), summary: None, provenance: Provenance::Curated, ownership: Ownership::External, visibility: Visibility::Restricted, evidence: vec![], source: Some(".ai/repo/knowledge/curated/secret-note.md".into()), extractor: "t" }.build();
        b.freshness = Freshness::Unverified;
        m.nodes = vec![a, b];
        m.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        m.relations.push(Relation { source: "knowledge:secret-note".into(), target: "component:alpha".into(), kind: "documents".into(), provenance: Provenance::Curated, evidence: vec![] });
        m
    }

    #[test]
    fn listing_filters_and_pages() {
        let m = model();
        let page = list(&m, &Filter { kind: Some("component".into()), ..Default::default() });
        assert_eq!(page.total, 1);
        let page = list(&m, &Filter { debt: true, ..Default::default() });
        assert_eq!(page.nodes[0].id, "knowledge:secret-note");
        let page = list(&m, &Filter { limit: 1, offset: 1, ..Default::default() });
        assert_eq!(page.nodes.len(), 1);
        assert_eq!(page.total, 2);
    }

    #[test]
    fn search_ranks_ids_over_titles_over_claims() {
        let m = model();
        let hits = search(&m, "alpha", 10);
        assert_eq!(hits[0].matched, "id");
        let hits = search(&m, "1.2.3", 10);
        assert_eq!(hits[0].matched, "claim");
        assert!(search(&m, "", 10).is_empty());
    }

    #[test]
    fn the_public_projection_drops_restricted_nodes_and_their_relations() {
        let m = model();
        let p = public_projection(&m);
        assert_eq!(p.nodes.len(), 1);
        assert!(p.relations.is_empty());
        let c = context(&m, &["apps/alpha".into()], 0, true);
        assert_eq!(c.nodes.len(), 1);
        let c = context(&m, &["apps/alpha".into()], 0, false);
        assert_eq!(c.nodes.len(), 2, "{:?}", c.nodes);
        assert_eq!(c.nodes[0].kind, "knowledge");
    }

    #[test]
    fn an_explanation_names_provenance_freshness_and_remedies() {
        let m = model();
        let e = explain(&m, "knowledge:secret-note").unwrap();
        assert_eq!(e.provenance, Provenance::Curated);
        assert!(e.why[0].contains("a person wrote"));
        assert!(e.remedies.iter().any(|r| r.contains("reconcile")));
        assert!(explain(&m, "nothing:here").is_none());
    }
}
