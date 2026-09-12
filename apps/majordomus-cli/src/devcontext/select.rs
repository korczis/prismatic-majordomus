//! Which of the repository's objects a request reaches, and by which declared path.
//!
//! # Nothing here discovers anything
//!
//! Selection is a walk of two values the process already holds: the index, and the composed
//! graph derived from it. The index was built once when the process loaded; the graph is
//! [`crate::graph::COMPOSED`], the repository's own dependency graph, in which every typed
//! reference the layer declares is already resolved to an edge. So there is no second
//! relation table here, no glob, no read of a file and no walk of the working tree — which
//! is what lets a request be answered inside a handler rather than at load time. The
//! precedent and the reason are `crate::plan`'s: "`App::load` has a stated budget and 200
//! project records have no business inside it."
//!
//! # The traversal policy is the only thing this module decides
//!
//! The graph says which things are related and how; it does not say which of those
//! relations carry *context*. [`EDGES`] is that decision, and it is small on purpose:
//! twelve edge kinds with a direction and a weight each, and three the compiler refuses to
//! follow. `is_a` is refused because every object of a kind hangs off one node and
//! following it would select the whole layer; `composes` and `projects` are refused because
//! they describe this executable's own wiring rather than the repository being worked on.
//! [`policy`] projects the table so that a caller can read the decision instead of
//! inferring it from the answer.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::Value;

use crate::capability::handler::Context;
use crate::graph::{Graph, Node};
use crate::index::Index;
use crate::model::Object;

use super::model::{Discovery, Selector, Tier};

/// Which way an edge is worth following, and what it means in each direction.
#[derive(Debug, Clone, Copy)]
pub struct EdgePolicy {
    /// The edge kind, as the composed graph names it.
    pub kind: &'static str,
    /// Relevance multiplier when followed from source to target; `0.0` to refuse.
    pub forward: f64,
    /// Relevance multiplier when followed from target to source; `0.0` to refuse.
    pub reverse: f64,
    /// Why the target is context for the source.
    pub forward_reason: &'static str,
    /// Why the source is context for the target.
    pub reverse_reason: &'static str,
}

/// Which of the composed graph's edges carry context, and how strongly.
///
/// A weight is a multiplier on the relevance of the entry the edge was followed from, so
/// relevance decays with distance and the decay is the edge's own. An edge absent from this
/// table is not followed at all, and `policy` reports it as refused with the reason.
pub const EDGES: &[EdgePolicy] = &[
    EdgePolicy {
        kind: "belongs_to",
        forward: 0.95,
        reverse: 0.45,
        forward_reason: "the milestone this work belongs to",
        reverse_reason: "another issue of the same milestone",
    },
    EdgePolicy {
        kind: "depends_on",
        forward: 0.9,
        reverse: 0.6,
        forward_reason: "it declares a dependency on this",
        reverse_reason: "this declares a dependency on it",
    },
    EdgePolicy {
        kind: "governed_by",
        forward: 0.9,
        reverse: 0.5,
        forward_reason: "it is governed by this",
        reverse_reason: "this is governed by it",
    },
    EdgePolicy {
        kind: "put_in_force",
        forward: 0.8,
        reverse: 0.85,
        forward_reason: "the decision put this in force",
        reverse_reason: "the decision that put it in force",
    },
    EdgePolicy {
        kind: "supersedes",
        forward: 0.7,
        reverse: 0.7,
        forward_reason: "it stands in for this",
        reverse_reason: "this stands in for it",
    },
    EdgePolicy {
        kind: "tracks",
        forward: 0.7,
        reverse: 0.9,
        forward_reason: "the contract tracks this file",
        reverse_reason: "the contract that tracks it",
    },
    EdgePolicy {
        kind: "implemented_by",
        forward: 0.85,
        reverse: 0.85,
        forward_reason: "the claim is implemented by this",
        reverse_reason: "a claim this implements",
    },
    EdgePolicy {
        kind: "tested_by",
        forward: 0.85,
        reverse: 0.85,
        forward_reason: "the claim is proved by this case",
        reverse_reason: "a claim this case proves",
    },
    EdgePolicy {
        kind: "defined_in",
        forward: 0.8,
        reverse: 0.8,
        forward_reason: "the claim is defined in this file",
        reverse_reason: "a claim defined in it",
    },
    EdgePolicy {
        kind: "exercises",
        forward: 0.6,
        reverse: 0.7,
        forward_reason: "the scenario exercises this",
        reverse_reason: "a scenario that exercises it",
    },
    EdgePolicy {
        kind: "evidences",
        forward: 0.6,
        reverse: 0.6,
        forward_reason: "it is evidence for this claim",
        reverse_reason: "what stands as evidence for it",
    },
    EdgePolicy {
        kind: "runs",
        forward: 0.6,
        reverse: 0.5,
        forward_reason: "the scenario runs this command",
        reverse_reason: "a scenario that runs it",
    },
    EdgePolicy {
        kind: "related_to",
        forward: 0.7,
        reverse: 0.7,
        forward_reason: "it names this as related",
        reverse_reason: "it is named as related by this",
    },
    EdgePolicy {
        kind: "serves",
        forward: 0.5,
        reverse: 0.4,
        forward_reason: "the scenario serves this application",
        reverse_reason: "a scenario that serves it",
    },
];

/// The edges the compiler refuses to follow, and why. Read by `devcontext.policy` so that
/// the refusal is inspectable state rather than an absence a reader has to notice.
pub const REFUSED: &[(&str, &str)] = &[
    (
        "is_a",
        "every object of a kind hangs off one node; following it would select the whole layer",
    ),
    (
        "composes",
        "this executable's own wiring, not the repository being worked on",
    ),
    (
        "projects",
        "this executable's own wiring, not the repository being worked on",
    ),
];

/// The policy for one edge kind, or `None` when the compiler refuses to follow it.
///
/// ```
/// use majordomus_cli::devcontext::edge_policy;
/// assert!(edge_policy("depends_on").is_some());
/// assert!(edge_policy("is_a").is_none());
/// ```
pub fn edge_policy(kind: &str) -> Option<&'static EdgePolicy> {
    EDGES.iter().find(|e| e.kind == kind)
}

/// Which tier an object of this kind is spent from, once it is not a seed.
///
/// ```
/// use majordomus_cli::devcontext::{tier_for_kind, Tier};
/// assert_eq!(tier_for_kind("adr"), Tier::Decision);
/// assert_eq!(tier_for_kind("test"), Tier::Source);
/// assert_eq!(tier_for_kind("policy"), Tier::Governance);
/// ```
pub fn tier_for_kind(kind: &str) -> Tier {
    match kind {
        "issue" | "milestone" | "project" => Tier::Task,
        "policy" | "scope" | "rule" | "context" | "profile" => Tier::Governance,
        "implementation" | "test" | "file" | "command" => Tier::Source,
        "adr" => Tier::Decision,
        "session" | "session-context" | "knowledge" | "prompt" | "skill" => Tier::Knowledge,
        _ => Tier::History,
    }
}

// ---------------------------------------------------------------- candidates

/// One thing the compiler reached, before any budget was spent on it. The compiler folds
/// every path that reached the same identifier into one of these, which is where
/// deduplication happens: the [`Candidate`] is the unit, and `discovered_by` is the
/// evidence of how many ways there were to find it.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The canonical identifier.
    pub uri: String,
    /// The kind, as the layer declared it.
    pub kind: String,
    /// The title, when there is one.
    pub title: Option<String>,
    /// The tier its cost comes out of.
    pub tier: Tier,
    /// The best relevance any path gave it.
    pub relevance: f64,
    /// The shortest depth any path reached it at.
    pub depth: usize,
    /// Every path that reached it.
    pub discovered_by: Vec<Discovery>,
    /// The object, when the index holds one under this identifier.
    pub object: Option<usize>,
    /// The file the thing lives in, when it is not an object of the index.
    pub path: Option<String>,
    /// The status word the object declares.
    pub status: Option<String>,
    /// Facts worth carrying.
    pub facts: BTreeMap<String, String>,
}

impl Candidate {
    fn merge(&mut self, relevance: f64, depth: usize, tier: Tier, d: Discovery) {
        if relevance > self.relevance {
            self.relevance = relevance;
        }
        if depth < self.depth {
            self.depth = depth;
        }
        // a seed stays a seed however else it was reached; otherwise the strongest
        // authority wins, and Task is the strongest
        if tier < self.tier {
            self.tier = tier;
        }
        // A path is a *kind* of way in, not an edge traversal. Thirty-three issues of one
        // milestone all reach that milestone along `belongs_to`, and recording that
        // thirty-three times would say the answer had thirty-three reasons when it has
        // one. So a discovery already recorded for this selector, edge and reason is kept
        // rather than repeated, and replaced only by a shorter or stronger way in.
        if let Some(existing) = self
            .discovered_by
            .iter_mut()
            .find(|e| e.selector == d.selector && e.edge == d.edge && e.reason == d.reason)
        {
            if d.depth < existing.depth || (d.depth == existing.depth && d.weight > existing.weight)
            {
                *existing = d;
            }
            return;
        }
        self.discovered_by.push(d);
    }

    /// The confidence of the most trusted path that reached it.
    pub fn confidence(&self) -> f64 {
        self.discovered_by
            .iter()
            .map(|d| d.confidence)
            .fold(0.0_f64, f64::max)
    }
}

/// The candidates a request reaches, and everything the walk learned on the way.
pub struct Selection {
    /// By canonical identifier.
    pub candidates: BTreeMap<String, Candidate>,
    /// Names the layer declares that resolve to nothing this repository holds.
    pub unresolved: Vec<(String, String)>,
    /// What could not be read, never an error.
    pub diagnostics: Vec<crate::model::Diagnostic>,
}

/// What the walk was asked to start from and how far it may go.
pub struct Request<'a> {
    /// The canonical identifiers the request named, already resolved.
    pub seeds: Vec<String>,
    /// The repository-relative paths the request named, plus every seed's declared scope.
    pub paths: BTreeSet<String>,
    /// The words of the intent, lowercased and deduplicated.
    pub terms: BTreeSet<String>,
    /// The furthest from a seed the walk goes.
    pub max_depth: usize,
    /// Relevance below this is reached and not offered.
    pub floor: f64,
    /// Every blocking rule of the layer, not only the ones a seed reaches.
    pub all_blocking_rules: bool,
    /// The branch the checkout is on, for selecting the sessions that ran on it.
    pub branch: Option<&'a str>,
}

/// The words of a free-text intent, lowercased, three characters or more, deduplicated.
///
/// ```
/// use majordomus_cli::devcontext::intent_terms;
/// let t = intent_terms("Make the CONTEXT compiler explain a budget");
/// assert!(t.contains("context"));
/// assert!(t.contains("budget"));
/// assert!(!t.contains("a"));
/// ```
pub fn intent_terms(intent: &str) -> BTreeSet<String> {
    intent
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.chars().count() >= 3)
        .map(str::to_lowercase)
        .filter(|w| !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// Words too common to mean anything about relevance. Short on purpose: a longer list is a
/// language model, and this is a term filter.
const STOP_WORDS: &[&str] = &[
    "and", "are", "but", "for", "from", "has", "have", "into", "not", "one", "that", "the",
    "their", "them", "then", "there", "this", "was", "were", "what", "when", "which", "will",
    "with", "you", "your",
];

/// Is `path` the same as `under`, or below it?
fn is_under(under: &str, path: &str) -> bool {
    path == under || path.starts_with(&format!("{under}/"))
}

/// A metadata value as one line of text, for a fact or a term match.
fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(scalar).collect())
        .unwrap_or_default()
}

/// The facts the compiler carries about an object: never its prose, only the short
/// declared values a reader of the answer would otherwise have to open the file for.
fn facts_of(o: &Object) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for key in [
        "class",
        "status",
        "priority",
        "milestone",
        "profile",
        "date",
        "version",
        "updated_at",
        "branch",
        "head",
        "task_id",
    ] {
        if let Some(v) = o.metadata.get(key).and_then(scalar) {
            out.insert(key.to_string(), v);
        }
    }
    let tags = strings(&o.metadata, "tags");
    if !tags.is_empty() {
        out.insert("tags".into(), tags.join(", "));
    }
    let scope = strings(&o.metadata, "scope");
    if !scope.is_empty() {
        out.insert("scope".into(), scope.join(", "));
    }
    out
}

/// The version or date an object states about itself, for the answer's `version` field.
pub fn version_of(o: &Object) -> Option<String> {
    for key in ["updated_at", "version", "date", "created_at"] {
        if let Some(v) = o.metadata.get(key).and_then(scalar) {
            return Some(v);
        }
    }
    None
}

// ---------------------------------------------------------------- the walk

/// Reach every candidate the request implies: the seeds, the graph around them, the paths
/// they declare, the governance the layer applies, the session that ran before, and — when
/// the request gave an intent — what its words match.
pub fn select(ctx: &Context, graph: &Graph, req: &Request<'_>) -> Selection {
    let index = ctx.index.as_ref();
    let by_uri: BTreeMap<&str, usize> = index
        .objects
        .iter()
        .enumerate()
        .map(|(i, o)| (o.uri.as_str(), i))
        .collect();
    let nodes: BTreeMap<&str, &Node> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut sel = Selection {
        candidates: BTreeMap::new(),
        unresolved: Vec::new(),
        diagnostics: Vec::new(),
    };

    // 1. the seeds themselves: what was asked for, at relevance 1
    for uri in &req.seeds {
        let d = Discovery {
            selector: Selector::Seed,
            reason: "named in the request".into(),
            via: None,
            edge: None,
            depth: 0,
            confidence: 1.0,
            weight: 1.0,
        };
        offer(&mut sel, index, &by_uri, &nodes, uri, Tier::Task, 1.0, 0, d);
    }

    // 2. the graph around them, breadth first, so the first time a thing is reached is by
    //    its shortest path and the walk visits each node once
    walk(&mut sel, index, &by_uri, &nodes, graph, req);

    // 3. the paths the work is about: the ones the request named, plus the scope the seeds
    //    and their immediate neighbours declare. An issue states its own scope, so a
    //    request that names an issue never has to restate it.
    let mut effective = req.paths.clone();
    for c in sel.candidates.values() {
        if c.depth > 1 || !matches!(c.kind.as_str(), "issue" | "milestone") {
            continue;
        }
        if let Some(i) = c.object {
            for s in strings(&index.objects[i].metadata, "scope") {
                let s = s.trim().trim_end_matches('/');
                if !s.is_empty() && !s.starts_with('/') && !s.split('/').any(|p| p == "..") {
                    effective.insert(s.to_string());
                }
            }
        }
    }

    // 4. what those paths reach: the code, the cases, and the contracts over them
    paths(&mut sel, index, &by_uri, &nodes, &effective);

    // 5. the governance the layer applies to everything it holds
    governance(&mut sel, index, &by_uri, &nodes, req);

    // 6. what ran before: the indexed session records of this branch or of these paths,
    //    and this checkout's own local records through `continuity.state`
    sessions(&mut sel, index, &by_uri, &nodes, &effective, req.branch);
    local_state(ctx, &mut sel);

    // 7. the intent, which is the only inference in the whole selection
    if !req.terms.is_empty() {
        intent(&mut sel, index, &by_uri, &nodes, req);
    }

    // 8. a source *this context* names and the repository does not hold. The layer's own
    //    resolver answers it, and the answer is narrowed to the objects that were selected:
    //    a reference dangling somewhere else in the repository is a finding for `doctor`,
    //    not a thing wrong with this context. Reported, never raised — an absent source is
    //    an answer, and a context short of one that says so beats one quietly short of it.
    let declared_here: BTreeSet<&str> = sel
        .candidates
        .values()
        .filter_map(|c| c.path.as_deref())
        .collect();
    for u in crate::graph::unresolved_relations(&ctx.registry, &index.objects) {
        if !declared_here.contains(u.declared_in.as_str()) {
            continue;
        }
        sel.unresolved.push((
            u.reference.clone(),
            format!("{} names it under `{}`; {}", u.declared_in, u.key, u.correction),
        ));
    }
    sel.unresolved.sort();
    sel.unresolved.dedup();

    sel
}

/// Put a candidate in, or fold another discovery into the one already there.
#[allow(clippy::too_many_arguments)]
fn offer(
    sel: &mut Selection,
    index: &Index,
    by_uri: &BTreeMap<&str, usize>,
    nodes: &BTreeMap<&str, &Node>,
    uri: &str,
    tier: Tier,
    relevance: f64,
    depth: usize,
    d: Discovery,
) {
    if let Some(existing) = sel.candidates.get_mut(uri) {
        existing.merge(relevance, depth, tier, d);
        return;
    }
    let object = by_uri.get(uri).copied();
    let (kind, title, path, status, facts) = match object {
        Some(i) => {
            let o = &index.objects[i];
            (
                o.kind.clone(),
                o.title.clone(),
                Some(o.provenance.path.clone()),
                o.metadata.get("status").and_then(scalar),
                facts_of(o),
            )
        }
        None => match nodes.get(uri) {
            Some(n) => (
                n.kind.clone(),
                Some(n.label.clone()),
                n.source.clone(),
                n.status.clone(),
                BTreeMap::new(),
            ),
            // a name with nothing behind it is not a candidate; it is an unresolved
            // reference, and step 7 of `select` reports those from the graph's own resolver
            None => return,
        },
    };
    sel.candidates.insert(
        uri.to_string(),
        Candidate {
            uri: uri.to_string(),
            kind,
            title,
            tier,
            relevance,
            depth,
            discovered_by: vec![d],
            object,
            path,
            status,
            facts,
        },
    );
}

/// The breadth-first walk over the edges [`EDGES`] allows.
fn walk(
    sel: &mut Selection,
    index: &Index,
    by_uri: &BTreeMap<&str, usize>,
    nodes: &BTreeMap<&str, &Node>,
    graph: &Graph,
    req: &Request<'_>,
) {
    // adjacency in both directions, built once for the request
    let mut out: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    let mut inc: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for e in &graph.edges {
        if edge_policy(&e.kind).is_none() {
            continue;
        }
        out.entry(e.source.as_str())
            .or_default()
            .push((e.target.as_str(), e.kind.as_str()));
        inc.entry(e.target.as_str())
            .or_default()
            .push((e.source.as_str(), e.kind.as_str()));
    }

    let mut queue: VecDeque<(String, f64, usize)> = req
        .seeds
        .iter()
        .filter(|u| sel.candidates.contains_key(u.as_str()))
        .map(|u| (u.clone(), 1.0, 0usize))
        .collect();
    let mut seen: BTreeSet<String> = req.seeds.iter().cloned().collect();

    while let Some((from, relevance, depth)) = queue.pop_front() {
        if depth >= req.max_depth {
            continue;
        }
        let steps = [(&out, true), (&inc, false)];
        for (adjacency, forward) in steps {
            for (to, kind) in adjacency.get(from.as_str()).into_iter().flatten() {
                let Some(policy) = edge_policy(kind) else {
                    continue;
                };
                let weight = if forward {
                    policy.forward
                } else {
                    policy.reverse
                };
                if weight <= 0.0 {
                    continue;
                }
                let next = relevance * weight;
                if next < req.floor {
                    continue;
                }
                let reason = if forward {
                    policy.forward_reason
                } else {
                    policy.reverse_reason
                };
                let d = Discovery {
                    selector: Selector::Relation,
                    reason: reason.to_string(),
                    via: Some(from.clone()),
                    edge: Some(kind.to_string()),
                    depth: depth + 1,
                    confidence: 1.0,
                    weight,
                };
                let tier = nodes
                    .get(*to)
                    .map(|n| tier_for_kind(&n.kind))
                    .unwrap_or(Tier::History);
                offer(
                    sel,
                    index,
                    by_uri,
                    nodes,
                    to,
                    tier,
                    next,
                    depth + 1,
                    d.clone(),
                );
                if seen.insert(to.to_string()) {
                    queue.push_back((to.to_string(), next, depth + 1));
                }
            }
        }
    }
}

/// What the declared paths reach: the code and the cases under them, and the directory
/// contracts that reach them.
///
/// The contract half reads the same three declared fields `majordomus context resolve`
/// reads — a subtree document's directory, an explicit document's `paths`, a document's
/// `tracks` — and applies them the way that command does. It is the projection of one rule,
/// not a second enforcement of it: the command is the authority and the gate.
fn paths(
    sel: &mut Selection,
    index: &Index,
    by_uri: &BTreeMap<&str, usize>,
    nodes: &BTreeMap<&str, &Node>,
    scope: &BTreeSet<String>,
) {
    for path in scope {
        for (i, o) in index.objects.iter().enumerate() {
            // the code and the cases: an object whose own file lies under the path
            if matches!(o.kind.as_str(), "implementation" | "test")
                && is_under(path, &o.provenance.path)
            {
                let d = Discovery {
                    selector: Selector::ScopePath,
                    reason: format!("under `{path}`, which the work declares as its scope"),
                    via: Some(path.clone()),
                    edge: None,
                    depth: 1,
                    confidence: 1.0,
                    weight: 0.8,
                };
                offer(
                    sel,
                    index,
                    by_uri,
                    nodes,
                    &o.uri,
                    Tier::Source,
                    0.8,
                    1,
                    d,
                );
                continue;
            }
            if o.kind != "context" || o.metadata.get("status").and_then(scalar).as_deref()
                == Some("deprecated")
            {
                continue;
            }
            let scope = o
                .metadata
                .get("scope")
                .and_then(scalar)
                .unwrap_or_else(|| "directory".into());
            let reason = if scope == "subtree" && is_under(&o.provenance.directory, path) {
                Some(format!(
                    "the contract of `{}` reaches `{path}` (scope subtree)",
                    o.provenance.directory
                ))
            } else if scope == "directory" && o.provenance.directory == *path {
                Some(format!("the contract of `{path}` itself (scope directory)"))
            } else if scope == "explicit"
                && strings(&o.metadata, "paths")
                    .iter()
                    .any(|p| is_under(p, path))
            {
                Some(format!("`{path}` is a declared path of this contract"))
            } else if strings(&o.metadata, "tracks")
                .iter()
                .any(|p| is_under(p, path) || is_under(path, p))
            {
                Some(format!("the contract tracks `{path}`"))
            } else {
                None
            };
            let Some(reason) = reason else { continue };
            let d = Discovery {
                selector: Selector::Contract,
                reason,
                via: Some(path.clone()),
                edge: None,
                depth: 1,
                confidence: 1.0,
                weight: 0.85,
            };
            let _ = i;
            offer(
                sel,
                index,
                by_uri,
                nodes,
                &o.uri,
                Tier::Governance,
                0.85,
                1,
                d,
            );
        }
    }
}

/// The governance the layer applies to everything: the policy and the scope, which are one
/// object each and are never dropped, and — when the request asks for them — every blocking
/// rule rather than only the ones a seed reaches.
fn governance(
    sel: &mut Selection,
    index: &Index,
    by_uri: &BTreeMap<&str, usize>,
    nodes: &BTreeMap<&str, &Node>,
    req: &Request<'_>,
) {
    for o in &index.objects {
        let (relevance, reason) = match o.kind.as_str() {
            "policy" => (
                1.0,
                "the repository's policy: it decides what every session is held to".to_string(),
            ),
            "scope" => (
                1.0,
                "the repository's scope: what a worker reads and what it never reads".to_string(),
            ),
            "rule" if req.all_blocking_rules => {
                let blocking =
                    o.metadata.get("class").and_then(scalar).as_deref() == Some("blocking");
                if !blocking {
                    continue;
                }
                (
                    0.6,
                    "a blocking rule of the layer; the request asked for all of them".to_string(),
                )
            }
            _ => continue,
        };
        let d = Discovery {
            selector: Selector::Governance,
            reason,
            via: None,
            edge: None,
            depth: 0,
            confidence: 1.0,
            weight: relevance,
        };
        offer(
            sel,
            index,
            by_uri,
            nodes,
            &o.uri,
            Tier::Governance,
            relevance,
            0,
            d,
        );
    }
}

/// The indexed session records that ran on this branch, and the ones that changed a file
/// under a declared path. Both facts are the record's own; nothing is inferred.
fn sessions(
    sel: &mut Selection,
    index: &Index,
    by_uri: &BTreeMap<&str, usize>,
    nodes: &BTreeMap<&str, &Node>,
    scope: &BTreeSet<String>,
    branch: Option<&str>,
) {
    for o in index.objects.iter().filter(|o| o.kind == "session") {
        let record_branch = o.metadata.get("branch").and_then(scalar);
        let same_branch = match (branch, record_branch.as_deref()) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        let touched: Vec<String> = strings(&o.metadata, "changed_files")
            .into_iter()
            .filter(|f| scope.iter().any(|p| is_under(p, f)))
            .collect();
        let (relevance, reason) = if !touched.is_empty() {
            (
                0.75,
                format!(
                    "a recorded session that changed {} file(s) under the declared scope",
                    touched.len()
                ),
            )
        } else if same_branch {
            (
                0.5,
                format!(
                    "a recorded session of this branch ({})",
                    record_branch.as_deref().unwrap_or("?")
                ),
            )
        } else {
            continue;
        };
        let d = Discovery {
            selector: Selector::SessionState,
            reason,
            via: None,
            edge: None,
            depth: 1,
            confidence: 1.0,
            weight: relevance,
        };
        offer(
            sel,
            index,
            by_uri,
            nodes,
            &o.uri,
            Tier::Knowledge,
            relevance,
            1,
            d,
        );
    }
}

/// The local records of this checkout, read through `continuity.state` rather than by
/// opening the files: the handover the resolver chose, the newest checkpoint, and the
/// active task. They are not objects of the index — `.ai/local/` is this checkout's own
/// state and never shared context — so they enter under a `local:` identifier, the way the
/// composed graph names a file it does not hold as an object.
fn local_state(ctx: &Context, sel: &mut Selection) {
    let state = match ctx.execute("continuity.state", serde_json::json!({})) {
        Ok(v) => v,
        Err(e) => {
            sel.diagnostics.push(crate::model::Diagnostic::warning(
                "continuity",
                None,
                format!("the local continuity state could not be read: {e}"),
            ));
            return;
        }
    };
    let add = |sel: &mut Selection, key: &str, what: &str, relevance: f64| {
        let Some(record) = state.get(key).filter(|v| !v.is_null()) else {
            return;
        };
        let Some(path) = record.get("path").and_then(Value::as_str) else {
            return;
        };
        let mut facts = BTreeMap::new();
        for f in [
            "created_at",
            "branch",
            "head",
            "task_id",
            "matched",
            "divergence",
            "next_action",
            "working_tree",
        ] {
            if let Some(v) = record.get(f).and_then(scalar) {
                facts.insert(f.to_string(), v);
            }
        }
        let divergence = record.get("divergence").and_then(Value::as_str);
        let trustworthy = matches!(divergence, Some("exact") | Some("advanced") | None);
        if !trustworthy {
            sel.diagnostics.push(crate::model::Diagnostic::warning(
                "stale_record",
                Some(path.to_string()),
                format!(
                    "the {what} was written against a different context ({}); trust git, not this record",
                    divergence.unwrap_or("unknown")
                ),
            ));
        }
        let bytes = std::fs::metadata(
            std::path::Path::new(&ctx.index.repository.root).join(path),
        )
        .map(|m| m.len())
        .unwrap_or(0);
        facts.insert("bytes".into(), bytes.to_string());
        let uri = format!("local:{path}");
        let reason = match record.get("matched").and_then(Value::as_str) {
            Some(m) => format!("the {what} the two-tier resolver chose ({m})"),
            None => format!("the {what} of this checkout"),
        };
        sel.candidates.insert(
            uri.clone(),
            Candidate {
                uri,
                kind: format!("local-{key}"),
                title: Some(path.to_string()),
                tier: Tier::Knowledge,
                relevance: if trustworthy {
                    relevance
                } else {
                    relevance / 2.0
                },
                depth: 0,
                discovered_by: vec![Discovery {
                    selector: Selector::SessionState,
                    reason,
                    via: None,
                    edge: None,
                    depth: 0,
                    confidence: if trustworthy { 1.0 } else { 0.5 },
                    weight: relevance,
                }],
                object: None,
                path: Some(path.to_string()),
                status: divergence.map(str::to_string),
                facts,
            },
        );
    };
    add(sel, "handover", "handover the last session left", 0.95);
    add(sel, "checkpoint", "newest checkpoint", 0.8);
}

/// What the intent's words match. This is the compiler's only inference, and it says so:
/// the confidence is the share of the intent's terms that occur in the object's own title,
/// description and tags, never in its body, so a long file cannot match by accident.
fn intent(
    sel: &mut Selection,
    index: &Index,
    by_uri: &BTreeMap<&str, usize>,
    nodes: &BTreeMap<&str, &Node>,
    req: &Request<'_>,
) {
    for o in &index.objects {
        let mut haystack = String::new();
        if let Some(t) = &o.title {
            haystack.push_str(t);
            haystack.push(' ');
        }
        if let Some(d) = &o.description {
            haystack.push_str(d);
            haystack.push(' ');
        }
        haystack.push_str(&o.identity);
        haystack.push(' ');
        for t in strings(&o.metadata, "tags") {
            haystack.push_str(&t);
            haystack.push(' ');
        }
        let haystack = haystack.to_lowercase();
        let hits: Vec<&String> = req.terms.iter().filter(|t| haystack.contains(*t)).collect();
        if hits.is_empty() {
            continue;
        }
        // the share of the intent that this object answers, capped: an inference never
        // outranks something the repository declared
        let confidence = (hits.len() as f64 / req.terms.len() as f64).min(1.0);
        let relevance = 0.6 * confidence;
        if relevance < req.floor {
            continue;
        }
        let mut named: Vec<&str> = hits.iter().map(|s| s.as_str()).collect();
        named.sort();
        let d = Discovery {
            selector: Selector::IntentMatch,
            reason: format!(
                "the intent's term(s) {} occur in its own title, description, identity or tags",
                named.join(", ")
            ),
            via: None,
            edge: None,
            depth: 1,
            confidence,
            weight: relevance,
        };
        offer(
            sel,
            index,
            by_uri,
            nodes,
            &o.uri,
            tier_for_kind(&o.kind),
            relevance,
            1,
            d,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hub_edges_are_refused_and_the_rest_have_a_direction() {
        for (kind, why) in REFUSED {
            assert!(
                edge_policy(kind).is_none(),
                "{kind} is refused and also followed"
            );
            assert!(!why.is_empty());
        }
        for e in EDGES {
            assert!(
                e.forward > 0.0 || e.reverse > 0.0,
                "{} is in the table and followed in neither direction",
                e.kind
            );
            assert!(e.forward <= 1.0 && e.reverse <= 1.0, "{} decays", e.kind);
            assert!(!e.forward_reason.is_empty() && !e.reverse_reason.is_empty());
        }
        // and the table names each edge once
        let mut kinds: Vec<&str> = EDGES.iter().map(|e| e.kind).collect();
        kinds.sort();
        let before = kinds.len();
        kinds.dedup();
        assert_eq!(before, kinds.len(), "an edge kind is in the table twice");
    }

    #[test]
    fn the_intent_keeps_words_that_mean_something() {
        let t = intent_terms("The budget and the ADR that put it in force");
        assert!(t.contains("budget"));
        assert!(t.contains("adr"));
        assert!(t.contains("force"));
        assert!(!t.contains("the"), "a stop word survived");
        assert!(!t.contains("it"), "a two-letter word survived");
        // and it is a set, so a repeated word does not weigh twice
        assert_eq!(intent_terms("budget budget budget").len(), 1);
    }

    #[test]
    fn every_kind_lands_in_a_tier_and_the_declared_ones_are_not_history() {
        assert_eq!(tier_for_kind("issue"), Tier::Task);
        assert_eq!(tier_for_kind("rule"), Tier::Governance);
        assert_eq!(tier_for_kind("implementation"), Tier::Source);
        assert_eq!(tier_for_kind("adr"), Tier::Decision);
        assert_eq!(tier_for_kind("knowledge"), Tier::Knowledge);
        assert_eq!(tier_for_kind("claim"), Tier::History);
        // an unknown kind is supporting evidence, never governance
        assert_eq!(tier_for_kind("something-new"), Tier::History);
    }

    #[test]
    fn containment_is_by_segment_not_by_prefix() {
        assert!(is_under("lib", "lib/context.sh"));
        assert!(is_under("lib", "lib"));
        assert!(!is_under("lib", "library/x.sh"));
        assert!(!is_under("lib/a", "lib"));
    }
}
