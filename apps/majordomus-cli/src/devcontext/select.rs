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
//! relations carry *context*. [`EDGES`] is that decision, and it is small on purpose: a
//! weight per direction for each edge kind that carries context, and [`REFUSED`] for the
//! kinds the compiler will not follow at all. `is_a` is refused because every object of a
//! kind hangs off one node and following it would select the whole layer; `composes` and
//! `projects` are refused because they describe this executable's own wiring rather than
//! the repository being worked on. `policy` projects the table so that a caller can read
//! the decision instead of inferring it from the answer.
//!
//! The two tables are complementary and not overlapping, and that is the invariant worth
//! seeing: a refusal is an edge kind's *absence* from [`EDGES`], so [`edge_policy`]
//! answering `None` is the refusal rather than a weight of zero standing for one. There is
//! therefore no way to be in both tables, and no way to be followed at weight nothing.
//!
//! ```
//! use majordomus_cli::devcontext::edge_policy;
//! use majordomus_cli::devcontext::select::{EDGES, REFUSED};
//!
//! // everything in the table is followed, in at least one direction, at a real weight
//! for e in EDGES {
//!     assert!(edge_policy(e.kind).is_some(), "{} is in the table", e.kind);
//!     assert!(e.forward > 0.0 || e.reverse > 0.0, "{} is followed at nothing", e.kind);
//! }
//!
//! // and a refused kind is absent from it, with the reason the report prints
//! let refused: Vec<&str> = REFUSED.iter().map(|(kind, _)| *kind).collect();
//! assert_eq!(refused, ["is_a", "composes", "projects"]);
//! for (kind, why) in REFUSED {
//!     assert!(edge_policy(kind).is_none(), "{kind} is both followed and refused");
//!     assert!(!why.is_empty(), "{kind} is refused and does not say why");
//! }
//! ```

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::Value;

use crate::capability::handler::Context;
use crate::graph::{Graph, Node};
use crate::index::Index;
use crate::model::Object;

use super::model::{Discovery, Selector, Tier};

/// Which way an edge is worth following, and what it means in each direction.
///
/// An edge of the composed graph has one kind and two readings, and the two are rarely
/// worth the same. From an issue, the milestone it `belongs_to` is strong context; from the
/// milestone, any one of its thirty-three issues is weak. So the weight is per direction,
/// and so is the reason — which is prose a reader of the answer sees, not a comment here.
///
/// A weight is a multiplier on the relevance of the entry the edge was followed *from*, so
/// relevance decays with distance and the rate of decay is the edge's own rather than a
/// global constant.
///
/// ```
/// use majordomus_cli::devcontext::{edge_policy, EdgePolicy};
///
/// // `belongs_to` is asymmetric, and the asymmetry is the point
/// let belongs_to: &EdgePolicy = edge_policy("belongs_to").unwrap();
/// assert!(
///     belongs_to.forward > belongs_to.reverse,
///     "an issue's milestone is context; a milestone's every issue is not",
/// );
/// assert_ne!(belongs_to.forward_reason, belongs_to.reverse_reason);
///
/// // `supersedes` is symmetric: either version is context for the other
/// let supersedes: &EdgePolicy = edge_policy("supersedes").unwrap();
/// assert_eq!(supersedes.forward, supersedes.reverse);
///
/// // and both readings of every followed kind carry words a reader of the answer sees
/// for kind in ["depends_on", "governed_by", "tested_by"] {
///     let e = edge_policy(kind).unwrap();
///     assert!(!e.forward_reason.is_empty() && !e.reverse_reason.is_empty(), "{kind}");
/// }
/// ```
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
///
/// The three scalar fields are folded rather than overwritten as paths arrive, and each
/// keeps the *best* value any path gave: the highest `relevance`, the shortest `depth`, and
/// the strongest `tier`. Strongest means lowest, because [`Tier`] is ordered by authority —
/// so a thing reached once as a seed and again three hops out along a weak edge is still a
/// `Task`-tier entry at relevance 1 and depth 0. Nothing a later path says can demote it.
///
/// `object` and `path` are the two ways a candidate can have a body: `object` indexes into
/// the index's own objects, and `path` names a file the layer pointed at without indexing.
/// Both can be absent, for something the layer names and this repository does not hold.
///
/// ```
/// use std::collections::BTreeMap;
///
/// use majordomus_cli::devcontext::select::Candidate;
/// use majordomus_cli::devcontext::{Discovery, Selector, Tier};
///
/// let way = |selector: Selector, depth: usize, confidence: f64| Discovery {
///     selector,
///     reason: "for the sake of the example".into(),
///     via: None,
///     edge: None,
///     depth,
///     confidence,
///     weight: 1.0,
/// };
///
/// // one thing, found twice: named in the request, and again by matching the intent
/// let c = Candidate {
///     uri: "majordomus://adr/0052".into(),
///     kind: "adr".into(),
///     title: Some("The episode is the boundary".into()),
///     tier: Tier::Task,
///     relevance: 1.0,
///     depth: 0,
///     discovered_by: vec![
///         way(Selector::Seed, 0, 1.0),
///         way(Selector::IntentMatch, 2, 0.4),
///     ],
///     object: None,
///     path: None,
///     status: None,
///     facts: BTreeMap::new(),
/// };
///
/// // two recorded paths is the fact worth keeping, not something to hide
/// assert_eq!(c.discovered_by.len(), 2);
/// // and the entry is trusted as far as its *best* path, not averaged down by its worst
/// assert_eq!(c.confidence(), 1.0);
/// ```
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

    /// The confidence of the most trusted path that reached it: the maximum, not the mean.
    ///
    /// Averaging would be wrong in a way that matters. Confidence here is about the
    /// *provenance of the selection* — 1.0 when the repository declared the path, less when
    /// the compiler inferred it from words — and a thing the layer explicitly declares does
    /// not become less certain because a weaker guess also found it. Finding more ways to
    /// something can only raise this number.
    ///
    /// A candidate with no recorded path is 0.0 rather than an error, which is also right:
    /// nothing vouches for it.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    ///
    /// use majordomus_cli::devcontext::select::Candidate;
    /// use majordomus_cli::devcontext::{Discovery, Selector, Tier};
    ///
    /// let mut c = Candidate {
    ///     uri: "majordomus://rule/project.x@1".into(),
    ///     kind: "rule".into(),
    ///     title: None,
    ///     tier: Tier::Governance,
    ///     relevance: 0.6,
    ///     depth: 1,
    ///     discovered_by: Vec::new(),
    ///     object: None,
    ///     path: None,
    ///     status: None,
    ///     facts: BTreeMap::new(),
    /// };
    ///
    /// // nothing vouches for it yet
    /// assert_eq!(c.confidence(), 0.0);
    ///
    /// // an inferred path: the words of the intent occurred in its own words
    /// c.discovered_by.push(Discovery {
    ///     selector: Selector::IntentMatch,
    ///     reason: "the intent's words occur in it".into(),
    ///     via: None,
    ///     edge: None,
    ///     depth: 1,
    ///     confidence: 0.4,
    ///     weight: 0.5,
    /// });
    /// assert_eq!(c.confidence(), 0.4);
    ///
    /// // and then a declared one, which the guess cannot dilute
    /// c.discovered_by.push(Discovery {
    ///     selector: Selector::Governance,
    ///     reason: "a rule the layer applies to everything".into(),
    ///     via: None,
    ///     edge: None,
    ///     depth: 0,
    ///     confidence: 1.0,
    ///     weight: 1.0,
    /// });
    /// assert_eq!(c.confidence(), 1.0, "the best path decides, never the average");
    /// ```
    pub fn confidence(&self) -> f64 {
        self.discovered_by
            .iter()
            .map(|d| d.confidence)
            .fold(0.0_f64, f64::max)
    }
}

/// The candidates a request reaches, and everything the walk learned on the way.
///
/// Keyed by canonical identifier, which is what makes structural deduplication a property
/// of the type rather than a step: one identifier cannot be in here twice, so the question
/// "was this reached more than once" is answered by a candidate's `discovered_by` and never
/// by scanning for duplicates.
///
/// The other two fields are what the walk could not do, and neither is an error.
/// `unresolved` is a name the layer declares and this repository holds nothing under — a
/// dangling reference, which is a fact about the repository worth reporting. `diagnostics`
/// is what could not be read. The budget turns both into rows of the answer rather than
/// swallowing them.
///
/// ```
/// use std::collections::BTreeMap;
///
/// use majordomus_cli::devcontext::select::{Candidate, Selection};
/// use majordomus_cli::devcontext::{Discovery, Selector, Tier};
///
/// let uri = "majordomus://adr/0052";
/// let candidate = Candidate {
///     uri: uri.into(),
///     kind: "adr".into(),
///     title: None,
///     tier: Tier::Decision,
///     relevance: 0.8,
///     depth: 1,
///     discovered_by: vec![Discovery {
///         selector: Selector::Relation,
///         reason: "the decision in force over this scope".into(),
///         via: Some("majordomus://issue/I0301".into()),
///         edge: Some("governed_by".into()),
///         depth: 1,
///         confidence: 1.0,
///         weight: 0.9,
///     }],
///     object: None,
///     path: None,
///     status: None,
///     facts: BTreeMap::new(),
/// };
///
/// let selection = Selection {
///     candidates: BTreeMap::from([(uri.to_string(), candidate)]),
///     // a reference the layer declares and nothing answers: reported, not dropped
///     unresolved: vec![("majordomus://rule/gone@1".into(), "depends_on".into())],
///     diagnostics: Vec::new(),
/// };
///
/// // the identifier is the key, so one thing is one entry however many paths found it
/// assert_eq!(selection.candidates.len(), 1);
/// assert_eq!(selection.candidates[uri].discovered_by.len(), 1);
/// assert!(selection.candidates.contains_key(uri));
///
/// // and a dangling reference survives to be reported as one
/// assert_eq!(selection.unresolved[0].0, "majordomus://rule/gone@1");
/// ```
pub struct Selection {
    /// By canonical identifier.
    pub candidates: BTreeMap<String, Candidate>,
    /// Names the layer declares that resolve to nothing this repository holds.
    pub unresolved: Vec<(String, String)>,
    /// What could not be read, never an error.
    pub diagnostics: Vec<crate::model::Diagnostic>,
}

/// What the walk was asked to start from and how far it may go.
///
/// This is the *resolved* request, not the caller's. Every short name is already a canonical
/// identifier, every path has been normalised, and the free-text intent has already become a
/// set of terms by [`intent_terms`] — so the walk parses nothing and infers nothing about
/// what it was asked. That separation is what keeps the inference in one place where its
/// confidence can be labelled, instead of spread through the traversal.
///
/// `paths` is wider than what the caller named: it is the union of the request's own paths
/// and the scope every seed declares, which is how naming an issue selects the contracts
/// over the code that issue is about without the caller listing them.
///
/// ```
/// use std::collections::BTreeSet;
///
/// use majordomus_cli::devcontext::select::Request;
/// use majordomus_cli::devcontext::{intent_terms, DEFAULT_FLOOR, DEFAULT_MAX_DEPTH};
///
/// let branch = String::from("master");
/// let req = Request {
///     seeds: vec!["majordomus://issue/I0301".into()],
///     // the request named one path; the issue's own declared scope added the other
///     paths: BTreeSet::from(["apps/majordomus-cli/src".to_string(), "lib".to_string()]),
///     terms: intent_terms("Explain the CONTEXT budget"),
///     max_depth: DEFAULT_MAX_DEPTH,
///     floor: DEFAULT_FLOOR,
///     all_blocking_rules: false,
///     branch: Some(&branch),
/// };
///
/// // the intent arrives parsed: lowercased, deduplicated, and without the short words
/// assert!(req.terms.contains("context") && req.terms.contains("budget"));
/// assert!(!req.terms.contains("CONTEXT"), "the walk never re-parses the intent");
/// assert!(!req.terms.contains("the"));
///
/// // and the branch is borrowed, because it belongs to the checkout and not to the request
/// assert_eq!(req.branch, Some("master"));
/// assert_eq!(req.seeds.len(), 1);
/// ```
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
///
/// One field, filled from whichever of four keys the object happens to declare, in a fixed
/// order of preference: `updated_at`, `version`, `date`, `created_at`. The order is "when
/// was this last true" before "which revision is it" before "when was it first written",
/// because the answer's `version` exists so that a reader can spot a stale entry, and the
/// freshest self-description is the one that serves that.
///
/// The kinds of the layer disagree about which key they carry — a rule has a `version`, an
/// ADR a `date`, an issue an `updated_at` — so this is a fallback chain rather than a
/// lookup. `None` is ordinary: plenty of objects say nothing about their own age.
///
/// ```
/// use majordomus_cli::devcontext::select::version_of;
/// use majordomus_cli::model::{Object, Provenance};
/// use serde_json::json;
///
/// // the object as the index holds it; only its metadata differs between the cases
/// let with = |metadata: serde_json::Value| -> Option<String> {
///     let o = Object {
///         kind: "rule".into(),
///         identity: "project.x@2".into(),
///         uri: "majordomus://rule/project.x@2".into(),
///         title: None,
///         description: None,
///         metadata,
///         body: String::new(),
///         content: String::new(),
///         media_type: "text/markdown",
///         provenance: Provenance {
///             path: ".ai/repo/rules/project/x.v2.md".into(),
///             directory: ".ai/repo/rules/project".into(),
///             source_class: "rule".into(),
///             section: Some("rules".into()),
///             bytes: 2048,
///             member: None,
///         },
///     };
///     version_of(&o)
/// };
///
/// // the freshest self-description wins, whichever keys are present
/// assert_eq!(
///     with(json!({"updated_at": "2026-09-12", "version": 2, "created_at": "2026-08-01"})),
///     Some("2026-09-12".into()),
/// );
/// assert_eq!(with(json!({"version": 2, "date": "2026-08-01"})), Some("2".into()));
/// assert_eq!(with(json!({"date": "2026-08-01"})), Some("2026-08-01".into()));
///
/// // and saying nothing about its own age is ordinary, not a fault
/// assert_eq!(with(json!({})), None);
/// ```
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
///
/// Nothing is read here. `graph` is [`crate::graph::COMPOSED`] already derived over the
/// index, so every typed reference the layer declares is an edge before this function
/// starts, and the walk is over two values the process is holding. That is what lets a
/// request be answered inside a handler rather than at load time.
///
/// It returns a [`Selection`] and never an error: an intent that matches nothing, a seed
/// whose scope is empty, a reference the layer declares and nothing answers — each of those
/// is a fact about the repository, carried in `unresolved` or `diagnostics`, and not a
/// failure of the call.
///
/// The governance selector is why a request that names nothing still gets an answer: the
/// policy and the scope apply to everything, so they are reached without a seed.
///
/// ```
/// use std::collections::BTreeSet;
///
/// use majordomus_cli::devcontext::select::{select, Request};
/// use majordomus_cli::devcontext::{DEFAULT_FLOOR, DEFAULT_MAX_DEPTH};
/// use majordomus_cli::graph;
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let composed = graph::derive(graph::COMPOSED, &ctx.registry, ctx.index.as_ref()).unwrap();
///
/// // a request that names nothing at all
/// let empty = Request {
///     seeds: Vec::new(),
///     paths: BTreeSet::new(),
///     terms: BTreeSet::new(),
///     max_depth: DEFAULT_MAX_DEPTH,
///     floor: DEFAULT_FLOOR,
///     all_blocking_rules: false,
///     branch: None,
/// };
/// let selection = select(&ctx, &composed, &empty);
///
/// // it still reaches the governance, because the governance applies without being asked for
/// assert!(
///     selection.candidates.values().any(|c| c.kind == "policy"),
///     "a request that names nothing still gets what every session is held to",
/// );
///
/// // and every candidate carries at least one recorded way in: nothing is in the
/// // selection without something that says how it got there
/// for c in selection.candidates.values() {
///     assert!(!c.discovered_by.is_empty(), "{} arrived unexplained", c.uri);
///     assert!(c.confidence() > 0.0, "{} is vouched for by nothing", c.uri);
/// }
/// ```
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
            format!(
                "{} names it under `{}`; {}",
                u.declared_in, u.key, u.correction
            ),
        ));
    }
    // Deduplicated without being ordered. This list is consumed once, by the budget, which
    // turns each entry into a diagnostic and sorts the diagnostics; no surface ever renders
    // the sequence built here. Sorting to make `dedup()` work was an opinion about an order
    // nobody reads, and two opinions about one collection is what `project.canonical-order`
    // exists to prevent — so the duplicates go and the discovery order stays.
    let mut seen = std::collections::HashSet::new();
    sel.unresolved.retain(|entry| seen.insert(entry.clone()));

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
                offer(sel, index, by_uri, nodes, &o.uri, Tier::Source, 0.8, 1, d);
                continue;
            }
            if o.kind != "context"
                || o.metadata.get("status").and_then(scalar).as_deref() == Some("deprecated")
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
        let bytes = std::fs::metadata(std::path::Path::new(&ctx.index.repository.root).join(path))
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
        crate::order::canonical_strings(&mut named);
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
