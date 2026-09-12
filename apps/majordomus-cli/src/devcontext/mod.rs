//! The context compiler: what a development session should be given, derived from the
//! repository rather than composed by hand.
//!
//! # What this is for
//!
//! A session that is handed a large hand-written prompt is handed somebody's memory of the
//! repository; a session that reads the repository from scratch on every request pays for
//! discovery it has already paid for. This module is the third option: given an issue, a
//! milestone, a free-text intent, some paths, or nothing at all, it derives the set of
//! canonical objects that bear on the work, says why each one is in the set, says what it
//! left out and why, and reports what the whole thing cost against a budget.
//!
//! # It is not a second context system
//!
//! `majordomus context` already assembles what a worker needs *now*: git, the active task,
//! its profile, the directory contracts over its scope, the open questions, the decisions,
//! the newest checkpoint, the most relevant handover, recent history — in authority order,
//! within a line budget, printing what it dropped. That command is the session briefing and
//! it stays the authority for it. What it cannot do is answer *about a piece of work* rather
//! than about the present moment, and its output is text: its own JSON projection carries
//! `sections[]` of `{id, lines, text}`, which is the assembled prose, not the selection.
//!
//! So this module keeps the same authority order — [`Tier`] is that order applied to
//! objects instead of to sections — and changes three things:
//!
//! - **The unit is an object, not a section.** Every entry carries the canonical
//!   identifier, the index's own provenance, the selector that reached it, the reason, the
//!   confidence, the version and the cost. Nothing is flattened to text here; rendering a
//!   prompt is a projection of this value and lives downstream of it.
//! - **It is keyed on the work.** An issue, a milestone, an intent or a path, not only the
//!   session that happens to be open.
//! - **It deduplicates and explains.** The same ADR reached by four paths is one entry with
//!   four recorded discoveries, and the answer says which paths those were.
//!
//! It also reuses rather than reimplements: the local records come from `continuity.state`,
//! the typed relations from [`crate::graph::COMPOSED`] — the repository's own dependency
//! graph, where every reference the layer declares is already resolved — and the objects
//! from the index. There is no second relation table, no glob and no walk of the tree.
//!
//! # The performance contract
//!
//! Nothing here is built when a [`crate::capability::Context`] is composed. The compiler
//! runs inside its handler, over values already in memory, for the reason
//! [`crate::plan`](crate::plan) states about the plan: "`App::load` has a stated budget and
//! 200 project records have no business inside it." Deriving the composed graph over this
//! repository is milliseconds; building the index is seconds, and it is built once per
//! process. The answer carries the index fingerprint, so an answer is valid exactly as long
//! as the index it was computed from, and a caller can tell two trees apart without asking.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};

pub mod budget;
pub mod model;
pub mod select;

pub use model::{
    tokens_for, Budget, CommitReference, CompiledContext, Conflict, ContextEntry, Deduplicated,
    Discovery, EntryProvenance, Excluded, ExclusionReason, GitContext, Seed, Selector, Tier,
    TierSpend, BYTES_PER_TOKEN, DEFAULT_BUDGET_TOKENS, DEFAULT_MAX_DEPTH,
};
pub use select::{edge_policy, intent_terms, tier_for_kind, EdgePolicy};

/// The lowest relevance the compiler offers unless the request says otherwise. Below it an
/// entry is reached and reported as excluded rather than given.
pub const DEFAULT_FLOOR: f64 = 0.25;

/// What to compile a context about.
///
/// Every field is optional. A request that names nothing still gets an answer: the
/// governance the layer applies to everything, and what the last session left — which is
/// the smallest true context there is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompileInput {
    /// An issue id (`I0301`) or its canonical identifier. Its milestone, its dependencies,
    /// its declared scope and the decisions over that scope follow from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// A milestone id or slug, or its canonical identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// What the session is trying to do, in words. The only input the compiler infers
    /// from, and every entry it produces says so and carries a confidence below one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// Repository-relative paths the work touches, added to whatever the seeds declare.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    /// Canonical identifiers to seed with directly, for a request about something that is
    /// neither an issue nor a milestone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uris: Vec<String>,
    /// The ceiling in estimated tokens; [`DEFAULT_BUDGET_TOKENS`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u64>,
    /// How far from a seed the walk goes; [`DEFAULT_MAX_DEPTH`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<usize>,
    /// Relevance below which an entry is reported rather than given; [`DEFAULT_FLOOR`]
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<f64>,
    /// Every blocking rule of the layer, not only the ones the work reaches. Honest and
    /// usually over budget: the answer then says `over_budget` rather than dropping what it
    /// may not drop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_blocking_rules: Option<bool>,
}

impl BenchmarkCases for CompileInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // the cases are this repository's own records, chosen by the same total order the
        // index holds them in, so the benchmark measures a real answer rather than an
        // empty one and does not go stale when a record is added
        let first = |kind: &str| -> Option<String> {
            ctx.index
                .objects
                .iter()
                .filter(|o| o.kind == kind)
                .map(|o| o.identity.clone())
                .min()
        };
        let mut cases = vec![NamedCase::new("no-seed", CompileInput::default())];
        if let Some(issue) = first("issue") {
            cases.push(NamedCase::new(
                "issue",
                CompileInput {
                    issue: Some(issue),
                    ..Default::default()
                },
            ));
        }
        if let Some(milestone) = first("milestone") {
            cases.push(NamedCase::new(
                "milestone",
                CompileInput {
                    milestone: Some(milestone),
                    ..Default::default()
                },
            ));
        }
        cases.push(NamedCase::new(
            "intent",
            CompileInput {
                intent: Some("the context a session is given and the budget it costs".into()),
                ..Default::default()
            },
        ));
        cases
    }
}

/// What one identifier's standing in a compiled context is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
// `worktree` already answers with a `Standing` about a branch, and the OpenAPI document has
// one component namespace: two enums under one name would be one component defined twice.
#[schemars(rename = "DevContextStanding")]
pub enum Standing {
    /// In the answer.
    Selected,
    /// Reached and not given; the exclusion says why.
    Excluded,
    /// Folded into another identifier, or dropped for one.
    Deduplicated,
    /// The index holds it and this request never reached it.
    NotReached,
    /// The index holds nothing under that identifier.
    Unknown,
}

impl Standing {
    /// The word this standing is reported under.
    ///
    /// ```
    /// use majordomus_cli::devcontext::Standing;
    /// assert_eq!(Standing::NotReached.as_str(), "not_reached");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Standing::Selected => "selected",
            Standing::Excluded => "excluded",
            Standing::Deduplicated => "deduplicated",
            Standing::NotReached => "not_reached",
            Standing::Unknown => "unknown",
        }
    }
}

/// The input of `devcontext.explain`: one identifier, and the request to judge it under.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplainInput {
    /// The canonical identifier to explain.
    pub uri: String,
    /// The request it would be compiled under; the same fields as `devcontext.compile`.
    #[serde(default, flatten)]
    pub request: CompileInput,
}

impl BenchmarkCases for ExplainInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let policy = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == "policy")
            .map(|o| o.uri.clone());
        let mut cases = Vec::new();
        if let Some(uri) = policy {
            cases.push(NamedCase::new(
                "policy",
                ExplainInput {
                    uri,
                    request: CompileInput::default(),
                },
            ));
        }
        cases.push(NamedCase::new(
            "absent",
            ExplainInput {
                uri: "majordomus://rule/nothing-like-this@1".into(),
                request: CompileInput::default(),
            },
        ));
        cases
    }
}

/// Why one identifier is, or is not, in a compiled context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Explanation {
    /// What was asked about.
    pub uri: String,
    /// Where it stands.
    pub standing: Standing,
    /// The same, in words.
    pub detail: String,
    /// The entry, when it is in the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<ContextEntry>,
    /// The exclusion, when it was reached and not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded: Option<Excluded>,
    /// The collapse it was part of, when it was folded or dropped for another.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deduplicated: Option<Deduplicated>,
    /// The budget the judgement was made under, so that "excluded for budget" can be acted
    /// on without a second call.
    pub budget: Budget,
}

/// One tier, as the policy reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TierRule {
    /// The tier.
    pub tier: Tier,
    /// Its position in the order the budget spends in; 1 is spent first.
    pub position: usize,
    /// What it is for.
    pub meaning: String,
    /// The kinds of object that land in it.
    pub kinds: Vec<String>,
}

/// One edge of the composed graph, and what the compiler does with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EdgeRule {
    /// The edge kind, as the composed graph names it.
    pub edge: String,
    /// What the graph asserts by it.
    pub means: String,
    /// The relevance multiplier when followed source to target; absent when refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward: Option<f64>,
    /// The multiplier when followed target to source; absent when refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reverse: Option<f64>,
    /// Why the target is context for the source, or why the edge is refused.
    pub reason: String,
    /// True when the compiler will not follow it in either direction.
    pub refused: bool,
}

/// One selector, and whether what it produces was declared or inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SelectorRule {
    /// The selector.
    pub selector: Selector,
    /// Declared by the repository, or inferred by the compiler.
    pub declared: bool,
}

/// The compiler's own rules, so that a caller reads the decision instead of inferring it
/// from an answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CompilerPolicy {
    /// The format of this answer.
    pub schema: u64,
    /// The graph the relations are read from.
    pub graph: String,
    /// The tiers, in the order the budget spends in.
    pub tiers: Vec<TierRule>,
    /// Every edge kind the composed graph has, followed or refused.
    pub edges: Vec<EdgeRule>,
    /// The selectors, and which of them infer.
    pub selectors: Vec<SelectorRule>,
    /// The default ceiling in estimated tokens.
    pub default_budget_tokens: u64,
    /// The default depth limit.
    pub default_max_depth: usize,
    /// The default relevance floor.
    pub default_floor: f64,
    /// Bytes per estimated token.
    pub bytes_per_token: u64,
}

// ---------------------------------------------------------------- compiling

fn normalise(path: &str) -> Result<String, CapabilityError> {
    let p = path.trim().trim_start_matches("./").trim_end_matches('/');
    if p.is_empty() {
        return Err(CapabilityError::InvalidInput(
            "an empty path is not a scope".into(),
        ));
    }
    if p.starts_with('/') || p.split('/').any(|s| s == "..") {
        return Err(CapabilityError::InvalidInput(format!(
            "'{path}' is not repository-relative; a path outside the repository is not context"
        )));
    }
    Ok(p.to_string())
}

/// Resolve one short name to the canonical identifier of an object of `kind`.
fn resolve_seed(
    ctx: &Context,
    kind: &str,
    named: &str,
    field: &str,
) -> Result<Seed, CapabilityError> {
    let named = named.trim();
    if named.is_empty() {
        return Err(CapabilityError::InvalidInput(format!(
            "{field}: name one, or leave it out"
        )));
    }
    let hit = ctx.index.objects.iter().find(|o| {
        o.kind == kind
            && (o.uri == named
                || o.identity == named
                || o.metadata.get("slug").and_then(|v| v.as_str()) == Some(named))
    });
    match hit {
        Some(o) => Ok(Seed {
            uri: o.uri.clone(),
            kind: o.kind.clone(),
            from: field.to_string(),
            resolved: (o.identity != named).then(|| o.identity.clone()),
        }),
        None => Err(CapabilityError::NotFound(format!(
            "no {kind} '{named}' in this repository; `objects.list` with kind `{kind}` names every one"
        ))),
    }
}

/// Compile a context. The one entry point; every projection calls this.
pub fn compile(ctx: &Context, input: CompileInput) -> Result<CompiledContext, CapabilityError> {
    let index = ctx.index.as_ref();
    let limit = input.budget_tokens.unwrap_or(DEFAULT_BUDGET_TOKENS);
    if limit == 0 {
        return Err(CapabilityError::InvalidInput(
            "a budget of zero tokens buys nothing; leave it out for the default".into(),
        ));
    }
    let max_depth = input.max_depth.unwrap_or(DEFAULT_MAX_DEPTH);
    let floor = input.floor.unwrap_or(DEFAULT_FLOOR);
    if !(0.0..=1.0).contains(&floor) {
        return Err(CapabilityError::InvalidInput(format!(
            "the relevance floor is between 0 and 1; got {floor}"
        )));
    }

    // ---- the seeds
    let mut seeds: Vec<Seed> = Vec::new();
    if let Some(named) = &input.issue {
        seeds.push(resolve_seed(ctx, "issue", named, "issue")?);
    }
    if let Some(named) = &input.milestone {
        seeds.push(resolve_seed(ctx, "milestone", named, "milestone")?);
    }
    for uri in &input.uris {
        let o = index
            .objects
            .iter()
            .find(|o| o.uri == *uri)
            .ok_or_else(|| {
                CapabilityError::NotFound(format!("the index holds nothing under `{uri}`"))
            })?;
        seeds.push(Seed {
            uri: o.uri.clone(),
            kind: o.kind.clone(),
            from: "uris".into(),
            resolved: None,
        });
    }
    let mut requested_paths = BTreeSet::new();
    for p in &input.paths {
        let p = normalise(p)?;
        requested_paths.insert(p.clone());
        seeds.push(Seed {
            uri: p.clone(),
            kind: "path".into(),
            from: "paths".into(),
            resolved: None,
        });
    }
    if let Some(intent) = &input.intent {
        if !intent.trim().is_empty() {
            seeds.push(Seed {
                uri: intent.trim().to_string(),
                kind: "intent".into(),
                from: "intent".into(),
                resolved: None,
            });
        }
    }

    let object_seeds: BTreeSet<String> = seeds
        .iter()
        .filter(|s| s.kind != "path" && s.kind != "intent")
        .map(|s| s.uri.clone())
        .collect();

    let terms = input
        .intent
        .as_deref()
        .map(intent_terms)
        .unwrap_or_default();

    let graph =
        crate::graph::derive(crate::graph::COMPOSED, &ctx.registry, index).ok_or_else(|| {
            CapabilityError::Internal("the composed graph is no longer derived".into())
        })?;

    let request = select::Request {
        seeds: object_seeds.iter().cloned().collect(),
        paths: requested_paths,
        terms,
        max_depth,
        floor,
        all_blocking_rules: input.all_blocking_rules.unwrap_or(false),
        branch: match &index.repository.git {
            crate::git::GitState::Available(info) => info.branch.as_deref(),
            _ => None,
        },
    };
    let selection = select::select(ctx, &graph, &request);

    // what stands in for what, from the graph's own edges rather than from a second read
    let mut supersedes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in &graph.edges {
        if e.kind == "supersedes" {
            supersedes
                .entry(e.source.clone())
                .or_default()
                .push(e.target.clone());
        }
    }

    let (spend, diagnostics) = budget::spend(index, selection, &object_seeds, limit, &supersedes);

    // the commits the layer itself names, gathered from what was selected
    let mut commits = Vec::new();
    for entry in &spend.selected {
        let Some(i) = index.objects.iter().position(|o| o.uri == entry.uri) else {
            continue;
        };
        let Some(evidence) = index.objects[i]
            .metadata
            .get("evidence")
            .and_then(|v| v.as_array())
        else {
            continue;
        };
        for e in evidence {
            let Some(commit) = e.get("commit").and_then(|v| v.as_str()) else {
                continue;
            };
            commits.push(CommitReference {
                commit: commit.to_string(),
                named_by: entry.uri.clone(),
                covers: e.get("covers").and_then(|v| v.as_str()).map(str::to_string),
            });
        }
    }
    commits.sort();
    commits.dedup();

    let git = match &index.repository.git {
        crate::git::GitState::Available(info) => GitContext {
            branch: info.branch.clone(),
            head: info.head.clone(),
            working_tree: info.working_tree.clone(),
            commits,
        },
        crate::git::GitState::Unavailable { reason } => GitContext {
            branch: None,
            head: None,
            working_tree: format!("unavailable: {reason}"),
            commits,
        },
    };

    Ok(CompiledContext {
        schema: 1,
        fingerprint: index.fingerprint.clone(),
        git,
        seeds,
        selected: spend.selected,
        excluded: spend.excluded,
        deduplicated: spend.deduplicated,
        conflicts: spend.conflicts,
        budget: spend.budget,
        diagnostics,
    })
}

/// Why one identifier is or is not in the context a request compiles to.
pub fn explain(ctx: &Context, input: ExplainInput) -> Result<Explanation, CapabilityError> {
    let uri = input.uri.trim().to_string();
    if uri.is_empty() {
        return Err(CapabilityError::InvalidInput(
            "name the canonical identifier to explain".into(),
        ));
    }
    let compiled = compile(ctx, input.request)?;
    if let Some(entry) = compiled.selected.iter().find(|e| e.uri == uri) {
        let ways: Vec<String> = entry
            .discovered_by
            .iter()
            .map(|d| format!("{}: {}", d.selector.as_str(), d.reason))
            .collect();
        return Ok(Explanation {
            uri,
            standing: Standing::Selected,
            detail: format!(
                "in the {} tier at relevance {} and confidence {}, costing {} token(s); reached by {} path(s) — {}",
                entry.tier.as_str(),
                entry.relevance,
                entry.confidence,
                entry.cost_tokens,
                entry.discovered_by.len(),
                ways.join("; ")
            ),
            entry: Some(entry.clone()),
            excluded: None,
            deduplicated: None,
            budget: compiled.budget,
        });
    }
    if let Some(x) = compiled.excluded.iter().find(|e| e.uri == uri) {
        return Ok(Explanation {
            uri,
            standing: Standing::Excluded,
            detail: format!(
                "reached and not given: {} — {}",
                x.reason.as_str(),
                x.detail
            ),
            entry: None,
            excluded: Some(x.clone()),
            deduplicated: None,
            budget: compiled.budget,
        });
    }
    if let Some(d) = compiled
        .deduplicated
        .iter()
        .find(|d| d.dropped.contains(&uri))
    {
        return Ok(Explanation {
            uri,
            standing: Standing::Deduplicated,
            detail: format!("folded into `{}` by {}: {}", d.kept, d.key, d.detail),
            entry: None,
            excluded: None,
            deduplicated: Some(d.clone()),
            budget: compiled.budget,
        });
    }
    let known = ctx.index.objects.iter().any(|o| o.uri == uri);
    Ok(Explanation {
        uri: uri.clone(),
        standing: if known {
            Standing::NotReached
        } else {
            Standing::Unknown
        },
        detail: if known {
            format!("the index holds `{uri}` and no selector of this request reached it: no seed names it, no followed edge leads to it, no declared path contains it, and it is not governance")
        } else {
            format!("the index holds nothing under `{uri}`")
        },
        entry: None,
        excluded: None,
        deduplicated: None,
        budget: compiled.budget,
    })
}

/// The compiler's rules: the tiers, every edge of the composed graph and what is done with
/// it, the selectors, and the defaults.
pub fn policy(ctx: &Context) -> Result<CompilerPolicy, CapabilityError> {
    let graph = crate::graph::derive(crate::graph::COMPOSED, &ctx.registry, ctx.index.as_ref())
        .ok_or_else(|| {
            CapabilityError::Internal("the composed graph is no longer derived".into())
        })?;
    let mut kinds: BTreeMap<Tier, Vec<String>> = BTreeMap::new();
    for kind in graph.node_kinds.keys() {
        kinds
            .entry(tier_for_kind(kind))
            .or_default()
            .push(kind.clone());
    }
    let tiers = Tier::ORDER
        .iter()
        .enumerate()
        .map(|(i, t)| TierRule {
            tier: *t,
            position: i + 1,
            meaning: t.meaning().to_string(),
            kinds: kinds.get(t).cloned().unwrap_or_default(),
        })
        .collect();

    // every edge the graph declares, judged: the table says which are followed, and the
    // refusals say why. An edge the graph gains and this table does not name is reported
    // as unjudged rather than silently unfollowed.
    let mut edges: Vec<EdgeRule> = Vec::new();
    for (edge, means) in &graph.edge_kinds {
        match edge_policy(edge) {
            Some(p) => edges.push(EdgeRule {
                edge: edge.clone(),
                means: means.clone(),
                forward: (p.forward > 0.0).then_some(p.forward),
                reverse: (p.reverse > 0.0).then_some(p.reverse),
                reason: format!("{}; reversed: {}", p.forward_reason, p.reverse_reason),
                refused: false,
            }),
            None => {
                let why = select::REFUSED
                    .iter()
                    .find(|(k, _)| k == edge)
                    .map(|(_, w)| (*w).to_string())
                    .unwrap_or_else(|| {
                        "the graph declares it and the compiler has no rule for it; it is not followed".into()
                    });
                edges.push(EdgeRule {
                    edge: edge.clone(),
                    means: means.clone(),
                    forward: None,
                    reverse: None,
                    reason: why,
                    refused: true,
                });
            }
        }
    }

    Ok(CompilerPolicy {
        schema: 1,
        graph: crate::graph::COMPOSED.to_string(),
        tiers,
        edges,
        selectors: Selector::ALL
            .iter()
            .map(|s| SelectorRule {
                selector: *s,
                declared: s.declared(),
            })
            .collect(),
        default_budget_tokens: DEFAULT_BUDGET_TOKENS,
        default_max_depth: DEFAULT_MAX_DEPTH,
        default_floor: DEFAULT_FLOOR,
        bytes_per_token: BYTES_PER_TOKEN,
    })
}

/// A repository with a plan in it, for the compiler's own tests.
///
/// [`crate::synthetic::SyntheticRepository`] is the shared generator and it deliberately
/// holds only a policy, some rules, some prompts and some documents — its shape is what the
/// scaling benchmarks measure, so a milestone added to it would move every baseline. This
/// fixture is its own thing: the smallest repository in which an issue belongs to a
/// milestone, a decision puts a rule in force, a later decision stands in for an earlier
/// one, a contract reaches a directory, and a closed session changed a file in scope.
#[cfg(test)]
mod fixture {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::capability::{builtin, CapabilityRegistry, Context};
    use crate::discovery::{FileSystem, Sources};
    use crate::git::GitState;
    use crate::index::Index;
    use crate::metadata::KindSchema;
    use crate::repository::Repository;
    use crate::share::Share;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// A generated repository on disk, removed when dropped.
    pub struct Planned {
        root: PathBuf,
    }

    impl Drop for Planned {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn rule(id: &str, title: &str, class: &str, status: &str, extra: &str) -> String {
        format!(
            "---\nid: {id}\nversion: 1\nkind: rule\ntitle: {title}\ndescription: What {id} requires, in one sentence.\nstatement: Do what {id} says.\nstatus: {status}\nclass: {class}\n{extra}---\n\n# Rationale\n\nBecause the answer has to be checkable.\n\n# Required behaviour\n\nDo what the statement says.\n\n# Failure behaviour\n\nA reviewer decides it.\n\n# Verification\n\nReview.\n"
        )
    }

    fn adr(id: &str, title: &str, extra: &str) -> String {
        format!(
            "---\nschema: adr/v1\nid: {id}\nkind: adr\ntitle: {title}\nstatus: accepted\ndate: 2026-09-10\ntags:\n  - context\n{extra}---\n\n## Context\n\nSomething had to be decided.\n\n## Decision\n\nIt was decided.\n\n## Consequences\n\nThings follow from it.\n"
        )
    }

    fn contract(id: &str, title: &str, scope: &str, order: u32, extra: &str) -> String {
        format!(
            "---\nschema: context/v1\nid: {id}\nkind: context\ntitle: {title}\ndescription: What this directory is for.\nstatus: active\nscope: {scope}\nproviders: [\"*\"]\naudience: [human, agent]\ncomposition: extend\norder: {order}\n{extra}---\n\n## Purpose\n\nWhat this directory is for.\n\n## Structure\n\nWhat is in it.\n\n## Naming conventions\n\nWhat a new file here is called.\n"
        )
    }

    const MANIFEST: &str = "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  scope: repo/scope.yaml\n  profiles: repo/profiles\n  rules: repo/rules\n  knowledge: repo/knowledge\n  adrs: repo/adrs\n  sessions: repo/sessions\n  project: repo/project\ncontext:\n  documents: [README.md]\n";

    const SOURCES: &str = "version: 1\nsources:\n  - id: context\n    kind: context\n    discovery: vcs\n    pathspec: ':(glob).ai/**/README.md'\n    required: true\n  - id: policy\n    kind: policy\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/policy.yaml'\n    required: true\n  - id: scope\n    kind: scope\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/scope.yaml'\n    required: false\n  - id: profile\n    kind: profile\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/profiles/*.yaml'\n    required: true\n  - id: rule\n    kind: rule\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/rules/**/*.md'\n    required: true\n  - id: milestone\n    kind: milestone\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/project/milestones/*.yaml'\n    required: false\n  - id: issue\n    kind: issue\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/project/issues/*.yaml'\n    required: false\n  - id: adr\n    kind: adr\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/adrs/????-*.md'\n    required: false\n  - id: session\n    kind: session\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/sessions/*.md'\n    required: false\n  - id: library\n    kind: implementation\n    discovery: vcs\n    pathspec: ':(glob)lib/*.sh'\n    required: false\n  - id: case\n    kind: test\n    discovery: vcs\n    pathspec: ':(glob)test/cases/*.sh'\n    required: false\n";

    const SCOPE: &str = "version: 1\nin:\n  - .ai/manifest.yaml\n  - .ai/README.md\n  - .ai/repo/**\n  - lib/**\n  - test/**\nout:\n  paths: []\n  binary: true\n  max_bytes: 1048576\n";

    impl Planned {
        /// Write the repository and return it.
        pub fn build() -> std::io::Result<Self> {
            let seq = SEQ.fetch_add(1, Ordering::SeqCst);
            let root = std::env::temp_dir()
                .join(format!("majordomus-planned-{}-{seq}", std::process::id()));
            std::fs::create_dir_all(&root)?;
            let me = Planned { root };
            me.write(".ai/manifest.yaml", MANIFEST)?;
            me.write(".ai/repo/knowledge/sources.yaml", SOURCES)?;
            me.write(".ai/repo/scope.yaml", SCOPE)?;
            me.write(
                ".ai/repo/policy.yaml",
                "version: 1\ncontext:\n  always_loaded_budget_lines: 150\n  builder_budget_lines: 300\n",
            )?;
            me.write(".ai/repo/profiles/implementation.yaml", "name: implementation\ndescription: The default.\ncapability: standard\neffort: medium\nverbosity: concise\npresentation: engineering\ncheckpoint_interval: 15m\n")?;

            // the contracts: the layer's root, and one over the rules directory
            me.write(
                ".ai/README.md",
                &contract("ai.layer", "The layer", "subtree", 10, "tracks: [lib]\n"),
            )?;
            me.write(
                ".ai/repo/rules/README.md",
                &contract("ai.repo.rules", "Rules", "subtree", 100, ""),
            )?;

            // the rules: one blocking, one that depends on it, one deprecated
            me.write(
                ".ai/repo/rules/project/alpha.v1.md",
                &rule(
                    "project.alpha",
                    "Alpha",
                    "blocking",
                    "active",
                    "depends_on: []\ntags: [context, budget]\n",
                ),
            )?;
            me.write(
                ".ai/repo/rules/project/beta.v1.md",
                &rule(
                    "project.beta",
                    "Beta",
                    "advisory",
                    "active",
                    "depends_on: [project.alpha]\ntags: [context]\n",
                ),
            )?;
            me.write(
                ".ai/repo/rules/project/gamma.v1.md",
                &rule(
                    "project.gamma",
                    "Gamma",
                    "advisory",
                    "deprecated",
                    "depends_on: [project.alpha]\ntags: [context]\n",
                ),
            )?;

            // the decisions: one puts the blocking rule in force, a later one stands in for it
            me.write(
                ".ai/repo/adrs/0001-alpha.md",
                &adr(
                    "adr-0001",
                    "The first decision",
                    "related:\n  - rule:project.alpha\n",
                ),
            )?;
            me.write(
                ".ai/repo/adrs/0002-beta.md",
                &adr(
                    "adr-0002",
                    "The decision that stands in for the first",
                    "supersedes:\n  - adr-0001\nrelated:\n  - rule:project.beta\n",
                ),
            )?;

            // the plan: one milestone, two issues, the second depending on the first
            me.write(".ai/repo/project/milestones/m1.yaml", "id: m1\ntitle: The milestone\nslug: the-milestone\norder: 0\npriority: p1\nproblem: \"A problem worth solving.\"\noutcome: \"The outcome once it is solved.\"\nacceptance_criteria:\n  - The outcome is reached\nvalidation:\n  - \"true\"\nevidence_required:\n  - proof\n")?;
            me.write(".ai/repo/project/issues/I0001.yaml", "id: I0001\nmilestone: m1\ntitle: The bounded piece of work\nslug: issue-I0001\npriority: p1\nprofile: implementation\nobjective: \"Do the bounded piece of work in lib.\"\nscope:\n  - lib\nacceptance_criteria:\n  - The work is done\nvalidation:\n  - \"true\"\nevidence_required:\n  - proof\nevidence:\n  - covers: proof\n    type: manual\n    command: \"true\"\n    result: \"it was done\"\n    commit: 0123456789abcdef0123456789abcdef01234567\n    recorded_at: 2026-09-10T00:00:00Z\n")?;
            me.write(".ai/repo/project/issues/I0002.yaml", "id: I0002\nmilestone: m1\ntitle: The piece that waits for the first\nslug: issue-I0002\npriority: p2\nprofile: implementation\nobjective: \"Do the second piece.\"\nscope:\n  - lib\ndepends_on: [I0001]\nacceptance_criteria:\n  - The work is done\nvalidation:\n  - \"true\"\nevidence_required:\n  - proof\n")?;

            // a closed session that changed a file in the issue's scope
            me.write(".ai/repo/sessions/s1.md", "---\nschema: session/v1\nkind: session\nsession_id: s-20260909000000-aaaa\ntitle: What the last session did\nstarted_at: 2026-09-09T00:00:00Z\nclosed_at: 2026-09-09T01:00:00Z\noutcome: closed\nbranch: master\nhead: 0123456789abcdef0123456789abcdef01234567\nworking_tree: clean\nchanged_files:\n  - lib/thing.sh\n---\n\n# Session\n\nIt changed one file.\n")?;

            // the code and the case the work is about
            me.write(
                "lib/thing.sh",
                "#!/usr/bin/env bash\n# the thing\nmj_thing() { :; }\n",
            )?;
            me.write(
                "test/cases/01_thing.sh",
                "#!/usr/bin/env bash\n# the case for the thing\nset -eu\n",
            )?;
            Ok(me)
        }

        fn write(&self, rel: &str, content: &str) -> std::io::Result<()> {
            let p = self.root.join(rel);
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(p, content)
        }

        /// The index of this repository, discovered through the filesystem.
        pub fn index(&self) -> crate::error::Result<Index> {
            let repo = Repository::discover(&self.root)?;
            let sources = Sources::load(&repo)?;
            let share = Share::locate(Some(&crate::synthetic::crate_share()), &self.root)?;
            let schema = KindSchema::load(&share, &repo)?;
            let fs = FileSystem {
                excluded: vec![".git".into(), repo.local_path()],
            };
            let scope = crate::scope::Scope::load(&share, &repo)?;
            Index::build(
                &repo,
                &sources,
                &schema,
                &fs,
                GitState::Available(crate::git::GitInfo {
                    toplevel: self.root.clone(),
                    head: Some("0123456789abcdef0123456789abcdef01234567".into()),
                    branch: Some("master".into()),
                    working_tree: "clean".into(),
                }),
                scope,
            )
        }

        /// A context over this repository.
        pub fn context(&self) -> crate::error::Result<std::sync::Arc<Context>> {
            let index = self.index()?;
            let registry = CapabilityRegistry::builder()
                .with_modules(builtin::modules())
                .with_index(&index)
                .build()
                .map_err(|errors| crate::error::Error::Registry { errors })?;
            Ok(std::sync::Arc::new(Context::new(
                std::sync::Arc::new(index),
                std::sync::Arc::new(registry),
            )))
        }

        /// The repository root, for a test that changes the tree.
        pub fn root(&self) -> &Path {
            &self.root
        }

        /// Change a file, so that the tree and every fingerprint move.
        pub fn touch(&self, rel: &str, suffix: &str) -> std::io::Result<()> {
            let text = std::fs::read_to_string(self.root.join(rel))?;
            self.write(rel, &format!("{text}{suffix}\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::Planned;
    use super::*;

    fn planned() -> (Planned, std::sync::Arc<Context>) {
        let repo = Planned::build().expect("a repository");
        let ctx = repo.context().expect("a context");
        assert_eq!(
            ctx.index.errors(),
            0,
            "the fixture does not validate: {:?}",
            ctx.index.diagnostics
        );
        (repo, ctx)
    }

    #[test]
    fn a_request_that_names_nothing_still_answers_with_the_governance() {
        let (_r, ctx) = planned();
        let c = compile(&ctx, CompileInput::default()).expect("an answer");
        assert_eq!(c.schema, 1);
        assert!(!c.fingerprint.is_empty(), "the answer names no tree");
        assert!(c.seeds.is_empty());
        let required: Vec<&str> = c
            .selected
            .iter()
            .filter(|e| e.required)
            .map(|e| e.kind.as_str())
            .collect();
        assert!(required.contains(&"policy"), "no policy: {required:?}");
        assert!(required.contains(&"scope"), "no scope: {required:?}");
        assert_eq!(c.budget.limit_tokens, DEFAULT_BUDGET_TOKENS);
        assert_eq!(c.budget.tiers.len(), Tier::ORDER.len());
        // and the answer says what tree it is true of
        assert_eq!(c.git.branch.as_deref(), Some("master"));
        assert_eq!(c.git.working_tree, "clean");
    }

    #[test]
    fn an_issue_pulls_its_milestone_and_says_why() {
        let (_r, ctx) = planned();
        let c = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                ..Default::default()
            },
        )
        .expect("an answer");
        let issue = c
            .selected
            .iter()
            .find(|e| e.kind == "issue" && e.uri.ends_with("I0001"))
            .expect("the seed is in its own answer");
        assert_eq!(issue.tier, Tier::Task);
        assert!(issue.required, "a seed may not be dropped for budget");
        assert_eq!(issue.discovered_by[0].selector, Selector::Seed);
        assert_eq!(issue.relevance, 1.0);
        assert_eq!(issue.provenance.source_class, "issue");
        assert_eq!(issue.facts.get("milestone").map(String::as_str), Some("m1"));

        let milestone = c
            .selected
            .iter()
            .find(|e| e.kind == "milestone")
            .expect("the milestone the issue belongs to");
        let via = milestone
            .discovered_by
            .iter()
            .find(|d| d.edge.as_deref() == Some("belongs_to"))
            .expect("reached along the declared edge");
        assert_eq!(via.selector, Selector::Relation);
        assert_eq!(via.depth, 1);
        assert!(!via.reason.is_empty());
        assert_eq!(via.confidence, 1.0);
        assert_eq!(milestone.relevance, 0.95, "the edge's own weight");

        // the second issue is reached backwards along `depends_on`, at a lower weight
        let second = c
            .selected
            .iter()
            .find(|e| e.uri.ends_with("I0002"))
            .expect("the issue that depends on the seed");
        assert!(second.relevance < issue.relevance);

        // every entry carries the index's own provenance, a cost and a bounded confidence
        for e in &c.selected {
            assert!(!e.provenance.path.is_empty(), "{} has no path", e.uri);
            assert!(e.cost_tokens > 0, "{} costs nothing", e.uri);
            assert!(
                (0.0..=1.0).contains(&e.confidence),
                "{} has confidence {}",
                e.uri,
                e.confidence
            );
            assert!(
                !e.discovered_by.is_empty(),
                "{} is in the answer for no reason",
                e.uri
            );
        }
    }

    #[test]
    fn a_milestone_reaches_the_issues_that_belong_to_it() {
        let (_r, ctx) = planned();
        let c = compile(
            &ctx,
            CompileInput {
                milestone: Some("m1".into()),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        assert!(c.seeds.iter().any(|s| s.kind == "milestone"));
        let issues: Vec<&ContextEntry> = c.selected.iter().filter(|e| e.kind == "issue").collect();
        assert_eq!(issues.len(), 2, "both issues of the milestone");
        for i in &issues {
            let via = i
                .discovered_by
                .iter()
                .find(|d| d.edge.as_deref() == Some("belongs_to"))
                .expect("reached backwards along belongs_to");
            assert!(via.reason.contains("same milestone"), "{}", via.reason);
        }
        // a milestone's issues declare `lib`, so the code and the case under it come too
        assert!(
            c.selected.iter().any(|e| e.kind == "implementation"),
            "the milestone's issues declare a scope and nothing under it was selected"
        );
    }

    #[test]
    fn a_seed_that_does_not_exist_says_so_rather_than_answering_emptily() {
        let (_r, ctx) = planned();
        match compile(
            &ctx,
            CompileInput {
                issue: Some("I9999".into()),
                ..Default::default()
            },
        ) {
            Err(CapabilityError::NotFound(m)) => assert!(m.contains("I9999"), "{m}"),
            other => panic!("{other:?}"),
        }
        match compile(
            &ctx,
            CompileInput {
                milestone: Some("nothing".into()),
                ..Default::default()
            },
        ) {
            Err(CapabilityError::NotFound(m)) => assert!(m.contains("milestone"), "{m}"),
            other => panic!("{other:?}"),
        }
        match compile(
            &ctx,
            CompileInput {
                uris: vec!["majordomus://rule/nothing@1".into()],
                ..Default::default()
            },
        ) {
            Err(CapabilityError::NotFound(m)) => assert!(m.contains("nothing"), "{m}"),
            other => panic!("{other:?}"),
        }
        match compile(
            &ctx,
            CompileInput {
                paths: vec!["../outside".into()],
                ..Default::default()
            },
        ) {
            Err(CapabilityError::InvalidInput(m)) => assert!(m.contains("outside"), "{m}"),
            other => panic!("{other:?}"),
        }
        match compile(
            &ctx,
            CompileInput {
                budget_tokens: Some(0),
                ..Default::default()
            },
        ) {
            Err(CapabilityError::InvalidInput(m)) => assert!(m.contains("zero"), "{m}"),
            other => panic!("{other:?}"),
        }
        match compile(
            &ctx,
            CompileInput {
                floor: Some(2.0),
                ..Default::default()
            },
        ) {
            Err(CapabilityError::InvalidInput(m)) => assert!(m.contains("floor"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_order_is_total_and_the_same_answer_twice() {
        let (_r, ctx) = planned();
        let input = CompileInput {
            issue: Some("I0001".into()),
            intent: Some("the budget and the decision that put alpha in force".into()),
            budget_tokens: Some(1_000_000),
            ..Default::default()
        };
        let a = compile(&ctx, input.clone()).expect("an answer");
        let b = compile(&ctx, input).expect("the same answer");
        assert_eq!(a, b, "the compiler is not deterministic");
        for w in a.selected.windows(2) {
            let (x, y) = (&w[0], &w[1]);
            assert!(
                x.tier < y.tier
                    || (x.tier == y.tier && x.relevance > y.relevance)
                    || (x.tier == y.tier && x.relevance == y.relevance && x.uri < y.uri),
                "{} ({}/{}) is ordered before {} ({}/{})",
                x.uri,
                x.tier.as_str(),
                x.relevance,
                y.uri,
                y.tier.as_str(),
                y.relevance
            );
        }
    }

    #[test]
    fn a_budget_that_cannot_hold_everything_says_what_it_dropped() {
        let (_r, ctx) = planned();
        let tight = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                budget_tokens: Some(1),
                ..Default::default()
            },
        )
        .expect("an answer");
        assert!(
            tight
                .excluded
                .iter()
                .any(|e| e.reason == ExclusionReason::Budget),
            "a one-token budget dropped nothing"
        );
        for e in &tight.excluded {
            assert!(
                !e.detail.is_empty(),
                "{} was dropped without a reason",
                e.uri
            );
            assert!(e.cost_tokens > 0, "{} would have cost nothing", e.uri);
        }
        // what may not be dropped is still there, and the answer admits it went over
        assert!(tight.selected.iter().all(|e| e.required));
        assert!(tight.budget.required_tokens > 1);
        assert!(tight.budget.over_budget);

        let loose = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        assert!(loose.selected.len() > tight.selected.len());
        assert!(!loose.budget.over_budget);
        assert_eq!(
            loose.budget.used_tokens + loose.budget.remaining_tokens,
            1_000_000
        );
        // and the per-tier spend adds up to what was used
        let summed: u64 = loose.budget.tiers.iter().map(|t| t.tokens).sum();
        assert_eq!(summed, loose.budget.used_tokens);
        let counted: usize = loose.budget.tiers.iter().map(|t| t.selected).sum();
        assert_eq!(counted, loose.selected.len());
    }

    #[test]
    fn the_budget_spends_the_authority_order() {
        let (_r, ctx) = planned();
        // a budget large enough for the task tier and little else keeps the task tier
        let c = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                budget_tokens: Some(400),
                ..Default::default()
            },
        )
        .expect("an answer");
        // First fit spends the tiers in order, so a later tier can only survive an earlier
        // tier's exclusion by being *cheaper* than what was dropped: the ceiling had room
        // for the small thing and not for the large one. A later, larger entry keeping its
        // place while an earlier one was dropped would mean the order was not spent.
        for dropped in c
            .excluded
            .iter()
            .filter(|e| e.reason == ExclusionReason::Budget)
        {
            for kept in c
                .selected
                .iter()
                .filter(|e| !e.required && e.tier > dropped.tier)
            {
                assert!(
                    kept.cost_tokens <= dropped.cost_tokens,
                    "{} ({}, {} tokens) was kept over {} ({}, {} tokens) from an earlier tier",
                    kept.uri,
                    kept.tier.as_str(),
                    kept.cost_tokens,
                    dropped.uri,
                    dropped.tier.as_str(),
                    dropped.cost_tokens
                );
            }
        }
        // and what the request was about is never what a budget drops
        assert!(c.selected.iter().any(|e| e.tier == Tier::Task));
        assert!(!c
            .excluded
            .iter()
            .any(|e| e.reason == ExclusionReason::Budget && e.tier == Tier::Task));
    }

    #[test]
    fn the_same_object_reached_twice_is_one_entry_with_two_reasons() {
        let (_r, ctx) = planned();
        // the blocking rule is reachable as governance, backwards from the decision that
        // put it in force, and by the intent's own words
        let c = compile(
            &ctx,
            CompileInput {
                intent: Some("alpha budget context".into()),
                all_blocking_rules: Some(true),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        let mut uris: Vec<&str> = c.selected.iter().map(|e| e.uri.as_str()).collect();
        let before = uris.len();
        uris.sort();
        uris.dedup();
        assert_eq!(before, uris.len(), "an identifier is in the answer twice");

        let multi = c
            .selected
            .iter()
            .find(|e| e.discovered_by.len() > 1)
            .expect("nothing was reached twice; the fixture cannot prove deduplication");
        let record = c
            .deduplicated
            .iter()
            .find(|d| d.kept == multi.uri && d.key == "uri")
            .expect("a folded entry with no record of the fold");
        assert_eq!(record.paths, multi.discovered_by.len());
        assert!(
            record.detail.contains("discovery path"),
            "{}",
            record.detail
        );
        let selectors: BTreeSet<Selector> =
            multi.discovered_by.iter().map(|d| d.selector).collect();
        assert!(
            selectors.len() > 1,
            "{} was reached twice by one selector only",
            multi.uri
        );
        // the entry keeps the best relevance of its paths, not the last one
        let best = multi
            .discovered_by
            .iter()
            .map(|d| d.confidence)
            .fold(0.0_f64, f64::max);
        assert_eq!(multi.confidence, best);
    }

    #[test]
    fn a_later_decision_stands_in_for_an_earlier_one_and_the_answer_says_so() {
        let (_r, ctx) = planned();
        let c = compile(
            &ctx,
            CompileInput {
                uris: vec!["majordomus://adr/adr-0002".into()],
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        let conflict = c
            .conflicts
            .iter()
            .find(|k| k.kind == "supersedes")
            .expect("two decisions in one answer and no conflict recorded");
        assert!(conflict.current.ends_with("adr-0002"));
        assert!(conflict.against.ends_with("adr-0001"));
        assert!(!c.selected.iter().any(|e| e.uri.ends_with("adr-0001")));
        let out = c
            .excluded
            .iter()
            .find(|e| e.uri.ends_with("adr-0001"))
            .expect("the superseded decision left the answer silently");
        assert_eq!(out.reason, ExclusionReason::Superseded);
        assert!(out.detail.contains("stands in for"), "{}", out.detail);
    }

    #[test]
    fn what_the_layer_says_is_no_longer_current_is_left_out_with_the_reason() {
        let (_r, ctx) = planned();
        let c = compile(
            &ctx,
            CompileInput {
                uris: vec!["majordomus://rule/project.alpha@1".into()],
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        // the deprecated rule depends on the seed, so it is reached — and refused
        let out = c
            .excluded
            .iter()
            .find(|e| e.uri.contains("project.gamma"))
            .expect("the deprecated rule was reached and not reported");
        assert_eq!(out.reason, ExclusionReason::Stale);
        assert!(out.detail.contains("deprecated"), "{}", out.detail);
        assert!(!c.selected.iter().any(|e| e.uri.contains("project.gamma")));

        // but a request that names it explicitly keeps it, and says it is stale
        let named = compile(
            &ctx,
            CompileInput {
                uris: vec!["majordomus://rule/project.gamma@1".into()],
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        assert!(named
            .selected
            .iter()
            .any(|e| e.uri.contains("project.gamma")));
        assert!(
            named
                .diagnostics
                .iter()
                .any(|d| d.code == "stale_seed" && d.message.contains("gamma")),
            "a stale seed was kept without a word about it: {:?}",
            named.diagnostics
        );
    }

    #[test]
    fn an_inference_is_never_as_confident_as_a_declaration() {
        let (_r, ctx) = planned();
        let c = compile(
            &ctx,
            CompileInput {
                intent: Some("alpha".into()),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        let inferred: Vec<&ContextEntry> = c
            .selected
            .iter()
            .filter(|e| e.discovered_by.iter().all(|d| !d.selector.declared()))
            .collect();
        assert!(
            !inferred.is_empty(),
            "the intent matched nothing; the test proves nothing"
        );
        for e in inferred {
            assert!(
                e.relevance < 1.0,
                "{} was inferred and is as relevant as a seed",
                e.uri
            );
            assert!(e.confidence <= 1.0);
            for d in &e.discovered_by {
                assert_eq!(d.selector, Selector::IntentMatch);
                assert!(d.reason.contains("intent"), "{}", d.reason);
            }
        }
    }

    #[test]
    fn a_declared_path_selects_the_contracts_the_code_and_the_cases_over_it() {
        let (_r, ctx) = planned();
        let c = compile(
            &ctx,
            CompileInput {
                paths: vec!["lib".into()],
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        // the code under the path
        let code = c
            .selected
            .iter()
            .find(|e| e.kind == "implementation")
            .expect("nothing under the declared path");
        assert_eq!(code.tier, Tier::Source);
        assert_eq!(code.discovered_by[0].selector, Selector::ScopePath);
        assert!(code.discovered_by[0].reason.contains("lib"));
        // the contract that tracks it
        let contract = c
            .selected
            .iter()
            .find(|e| {
                e.kind == "context"
                    && e.discovered_by
                        .iter()
                        .any(|d| d.selector == Selector::Contract)
            })
            .expect("no directory contract over the declared path");
        assert_eq!(contract.tier, Tier::Governance);
        // and the path is recorded as a seed, so the answer says what it was about
        assert!(c.seeds.iter().any(|s| s.kind == "path" && s.uri == "lib"));

        // a path inside the layer resolves its own directory's contract and its ancestors
        let inside = compile(
            &ctx,
            CompileInput {
                paths: vec![".ai/repo/rules".into()],
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        let ids: BTreeSet<&str> = inside
            .selected
            .iter()
            .filter(|e| e.kind == "context")
            .map(|e| e.uri.as_str())
            .collect();
        assert!(
            ids.iter().any(|u| u.ends_with("ai.repo.rules")),
            "the directory's own contract is missing: {ids:?}"
        );
        assert!(
            ids.iter().any(|u| u.ends_with("ai.layer")),
            "the ancestor's contract is missing: {ids:?}"
        );
    }

    #[test]
    fn a_previous_session_that_touched_the_scope_is_knowledge() {
        let (_r, ctx) = planned();
        let c = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        let session = c
            .selected
            .iter()
            .find(|e| e.kind == "session")
            .expect("the recorded session that changed a file in scope");
        assert_eq!(session.tier, Tier::Knowledge);
        assert_eq!(session.discovered_by[0].selector, Selector::SessionState);
        assert!(
            session.discovered_by[0].reason.contains("changed"),
            "{}",
            session.discovered_by[0].reason
        );
    }

    #[test]
    fn the_commits_the_layer_names_are_carried_with_the_record_that_named_them() {
        let (_r, ctx) = planned();
        let c = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        let commit = c
            .git
            .commits
            .iter()
            .find(|k| k.commit.starts_with("0123456789"))
            .expect("the issue's own evidence names a commit and the answer dropped it");
        assert!(commit.named_by.ends_with("I0001"));
        assert_eq!(commit.covers.as_deref(), Some("proof"));
    }

    #[test]
    fn a_changed_tree_is_a_different_answer() {
        let repo = Planned::build().expect("a repository");
        let before = {
            let ctx = repo.context().expect("a context");
            compile(&ctx, CompileInput::default()).expect("an answer")
        };
        repo.touch(".ai/repo/rules/project/alpha.v1.md", "\nOne more line.")
            .expect("the tree moves");
        let after = {
            let ctx = repo.context().expect("a context");
            compile(&ctx, CompileInput::default()).expect("an answer")
        };
        assert_ne!(
            before.fingerprint, after.fingerprint,
            "the tree changed and the answer claims the same index"
        );
        assert!(repo.root().exists());
    }

    #[test]
    fn explaining_covers_selected_excluded_deduplicated_and_never_reached() {
        let (_r, ctx) = planned();
        let base = CompileInput {
            issue: Some("I0001".into()),
            budget_tokens: Some(1_000_000),
            ..Default::default()
        };
        let compiled = compile(&ctx, base.clone()).expect("an answer");
        let chosen = compiled.selected[0].uri.clone();
        let e = explain(
            &ctx,
            ExplainInput {
                uri: chosen.clone(),
                request: base.clone(),
            },
        )
        .expect("an explanation");
        assert_eq!(e.standing, Standing::Selected);
        assert!(e.entry.is_some());
        assert!(e.detail.contains("reached by"), "{}", e.detail);
        assert_eq!(e.budget.limit_tokens, 1_000_000);

        let unknown = explain(
            &ctx,
            ExplainInput {
                uri: "majordomus://rule/nothing-like-this@1".into(),
                request: base.clone(),
            },
        )
        .expect("an explanation");
        assert_eq!(unknown.standing, Standing::Unknown);
        assert!(
            unknown.detail.contains("holds nothing"),
            "{}",
            unknown.detail
        );

        // excluded for budget, judged under a budget that cannot hold it
        let tight = CompileInput {
            issue: Some("I0001".into()),
            budget_tokens: Some(1),
            ..Default::default()
        };
        let dropped = compile(&ctx, tight.clone()).expect("an answer").excluded[0]
            .uri
            .clone();
        let x = explain(
            &ctx,
            ExplainInput {
                uri: dropped,
                request: tight,
            },
        )
        .expect("an explanation");
        assert_eq!(x.standing, Standing::Excluded);
        assert!(x.excluded.is_some());
        assert!(x.detail.contains("reached and not given"), "{}", x.detail);

        // something the index holds that this request has no reason to reach
        let far = ctx.index.objects.iter().map(|o| o.uri.clone()).find(|u| {
            !compiled.selected.iter().any(|e| e.uri == *u)
                && !compiled.excluded.iter().any(|e| e.uri == *u)
                && !compiled.deduplicated.iter().any(|d| d.dropped.contains(u))
        });
        if let Some(uri) = far {
            let n = explain(&ctx, ExplainInput { uri, request: base }).expect("an explanation");
            assert_eq!(n.standing, Standing::NotReached);
            assert!(n.detail.contains("no selector"), "{}", n.detail);
        }

        match explain(
            &ctx,
            ExplainInput {
                uri: "   ".into(),
                request: CompileInput::default(),
            },
        ) {
            Err(CapabilityError::InvalidInput(m)) => assert!(m.contains("identifier"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_policy_judges_every_edge_the_graph_declares() {
        let (_r, ctx) = planned();
        let p = policy(&ctx).expect("the policy");
        assert_eq!(p.graph, crate::graph::COMPOSED);
        assert_eq!(p.tiers.len(), Tier::ORDER.len());
        assert_eq!(p.tiers[0].position, 1);
        assert_eq!(p.tiers[0].tier, Tier::Task);
        assert!(!p.edges.is_empty());
        let is_a = p.edges.iter().find(|e| e.edge == "is_a").expect("is_a");
        assert!(is_a.refused);
        assert!(is_a.reason.contains("whole layer"), "{}", is_a.reason);
        assert!(is_a.forward.is_none() && is_a.reverse.is_none());
        let dep = p
            .edges
            .iter()
            .find(|e| e.edge == "depends_on")
            .expect("depends_on");
        assert!(!dep.refused);
        assert_eq!(dep.forward, Some(0.9));
        assert!(!dep.means.is_empty());
        assert_eq!(p.selectors.len(), Selector::ALL.len());
        assert!(p.selectors.iter().any(|s| !s.declared));
        assert_eq!(p.default_budget_tokens, DEFAULT_BUDGET_TOKENS);
        assert_eq!(p.bytes_per_token, BYTES_PER_TOKEN);
        // every tier says which kinds land in it, and the layer's kinds are all placed
        let placed: usize = p.tiers.iter().map(|t| t.kinds.len()).sum();
        assert!(placed > 0);
    }

    #[test]
    fn the_depth_limit_and_the_floor_both_narrow_the_answer() {
        let (_r, ctx) = planned();
        let deep = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                max_depth: Some(4),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        let shallow = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                max_depth: Some(1),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        assert!(
            shallow.selected.len() <= deep.selected.len(),
            "a shallower walk reached more"
        );
        assert!(shallow.selected.iter().all(|e| e.depth <= 1));

        let strict = compile(
            &ctx,
            CompileInput {
                issue: Some("I0001".into()),
                floor: Some(0.9),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        // the floor keeps out what the walk decayed below it; the seeds and the governance
        // are at 1.0 and stay
        assert!(strict.selected.iter().any(|e| e.relevance >= 0.9));
        assert!(strict.selected.len() < deep.selected.len());
    }

    #[test]
    fn asking_for_every_blocking_rule_is_honest_about_the_cost() {
        let (_r, ctx) = planned();
        let some = compile(
            &ctx,
            CompileInput {
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        let all = compile(
            &ctx,
            CompileInput {
                all_blocking_rules: Some(true),
                budget_tokens: Some(1_000_000),
                ..Default::default()
            },
        )
        .expect("an answer");
        assert!(all.budget.used_tokens > some.budget.used_tokens);
        let rule = all
            .selected
            .iter()
            .find(|e| e.kind == "rule")
            .expect("no blocking rule when every one was asked for");
        assert_eq!(rule.tier, Tier::Governance);
        assert_eq!(
            rule.facts.get("class").map(String::as_str),
            Some("blocking")
        );
        assert_eq!(rule.discovered_by[0].selector, Selector::Governance);
        // an advisory rule is not governance the layer applies to everything
        assert!(!all
            .selected
            .iter()
            .any(
                |e| e.facts.get("class").map(String::as_str) == Some("advisory")
                    && e.discovered_by
                        .iter()
                        .all(|d| d.selector == Selector::Governance)
            ));
    }
}
