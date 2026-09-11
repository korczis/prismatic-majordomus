//! The `rks` module — the repository knowledge system — projected. (`knowledge` is the
//! declarative module of the curated records kind, which is the canonical thing; the
//! system that reads them and everything else takes the initials.) Every capability
//! here reads one scan — made once per process and shared, keyed on the index, the
//! registry, the baseline and the exceptions — and answers a slice of it: the status,
//! a listing, one node, a search, an explanation, a graph slice, the impact of a change
//! set, the gaps, the coverage, the conflicts, the check against the baseline, the
//! canonicality audit, the context an agent should read, the extractors, and the
//! reconciliation proposals.
//!
//! Nothing here writes: recording the baseline, accepting a reconciliation and running a
//! semantic provider are operations of the command line (`majordomus knowledge …`),
//! because the registry's contract is that a capability never writes to the repository.
//! The command line calls the same functions of [`crate::knowledge`] these handlers call,
//! so the two agree by construction.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::knowledge::baseline::{CheckReport, Mode};
use crate::knowledge::canonicality::Audit;
use crate::knowledge::model::{
    Conflict, Coverage, ExtractorInfo, Gap, GapCategory, KindInfo, KnowledgeModel,
    PredicateInfo, ReferenceInfo, RelationInfo, RepositoryIdentity, Resolution,
};
use crate::knowledge::query::{self, ContextBundle, Explanation, Filter, GraphSlice, Hit, NodeView, Page};
use crate::knowledge::reconcile::Reconciliation;
use crate::knowledge::{impact, scanned, Scanned};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the status is read as an MCP resource.
pub const KNOWLEDGE_URI: &str = "majordomus://knowledge";

/// The URI under which the whole model is read as an MCP resource.
pub const MODEL_URI: &str = "majordomus://knowledge/model";

/// The cache every read here uses: the scan is memoised for the same seconds, so a
/// second call within the window costs the projection and nothing else.
const CACHE: CachePolicy = CachePolicy::Process {
    max_entries: 32,
    ttl_seconds: Some(5),
};

fn scan(ctx: &Context) -> Result<std::sync::Arc<Scanned>, CapabilityError> {
    scanned(ctx)
}

// ------------------------------------------------------------------- status

/// The baseline, summarised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeBaselineSummary")]
pub struct BaselineSummary {
    /// Whether one was recorded.
    pub recorded: bool,
    /// The commit it was recorded at, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    /// Where it is, repository-relative.
    pub path: String,
    /// Nodes recorded.
    pub nodes: usize,
    /// Verified claims.
    pub verified: usize,
    /// Tolerated debt.
    pub debt: usize,
    /// Accepted conflicts.
    pub accepted: usize,
    /// Tolerated canonicality violations.
    pub canonicality: usize,
}

/// The canonicality audit, summarised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CanonicalitySummary {
    /// `pass` or `fail`.
    pub verdict: String,
    /// Capabilities audited.
    pub capabilities: usize,
    /// Capabilities with a manual maintenance surface of one.
    pub canonical: usize,
    /// Mean manual maintenance surface, times 100.
    pub mms_centi: usize,
    /// Violations found.
    pub violations: usize,
    /// Violations neither tolerated nor excepted.
    pub counting: usize,
}

/// The semantic layer, summarised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeSemanticSummary")]
pub struct SemanticSummary {
    /// Whether the policy enables it.
    pub enabled: bool,
    /// The provider the policy names.
    pub provider: String,
    /// Whether a remote provider is allowed.
    pub allow_remote: bool,
    /// Derived claims in the model.
    pub derived_claims: usize,
}

/// The status of the knowledge system: everything a person or a page asks first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeStatusReport")]
pub struct StatusReport {
    /// `majordomus/knowledge/v1`.
    pub schema: String,
    /// The repository scanned.
    pub repository: RepositoryIdentity,
    /// What freshness is judged against.
    pub reference: ReferenceInfo,
    /// The model's fingerprint: content, not freshness.
    pub fingerprint: String,
    /// The check's mode.
    pub mode: Mode,
    /// The baseline.
    pub baseline: BaselineSummary,
    /// Nodes.
    pub nodes: usize,
    /// Claims.
    pub claims: usize,
    /// Evidence.
    pub evidence: usize,
    /// Relations.
    pub relations: usize,
    /// Nodes by kind.
    pub kinds: BTreeMap<String, usize>,
    /// Nodes by freshness.
    pub freshness: BTreeMap<String, usize>,
    /// Nodes by provenance.
    pub provenance: BTreeMap<String, usize>,
    /// Open conflicts.
    pub conflicts_open: usize,
    /// Accepted conflicts.
    pub conflicts_accepted: usize,
    /// Gaps by category.
    pub gaps: BTreeMap<String, usize>,
    /// The coverage rows.
    pub coverage: Coverage,
    /// The check against the baseline, in the policy's mode.
    pub check: CheckReport,
    /// The canonicality audit, summarised.
    pub canonicality: CanonicalitySummary,
    /// The semantic layer.
    pub semantic: SemanticSummary,
    /// The extractors that ran, by id.
    pub extractors: Vec<String>,
    /// Diagnostics the scan raised.
    pub diagnostics: usize,
    /// The diagnostics, one line each.
    pub findings: Vec<String>,
    /// One line.
    pub summary: String,
}

fn status(ctx: &Context, _: Empty) -> Result<StatusReport, CapabilityError> {
    let s = scan(ctx)?;
    let m = &s.model;
    let check = crate::knowledge::baseline::check(m, &s.baseline, s.policy.mode);
    let audit = audit_of(ctx, &s)?;
    let mut gaps: BTreeMap<String, usize> = BTreeMap::new();
    for g in &m.gaps {
        *gaps.entry(crate::knowledge::extract::gap_word(g.category).to_string()).or_insert(0) += 1;
    }
    let claims = m.nodes.iter().map(|n| n.claims.len()).sum();
    let derived_claims = m
        .claims()
        .filter(|(_, c)| c.provenance == crate::knowledge::model::Provenance::Derived)
        .count();
    let open = m.conflicts.iter().filter(|c| c.resolution == Resolution::Open).count();
    let summary = format!(
        "{} nodes, {} claims, {} relations; {} current, {} in debt; {} open conflict(s); check {}; canonicality {}",
        m.nodes.len(),
        claims,
        m.relations.len(),
        m.freshness_tallies().get("current").copied().unwrap_or(0),
        m.nodes.iter().filter(|n| n.freshness.is_debt()).count(),
        open,
        check.verdict,
        audit.verdict
    );
    Ok(StatusReport {
        schema: m.schema.clone(),
        repository: m.repository.clone(),
        reference: m.reference.clone(),
        fingerprint: m.fingerprint.clone(),
        mode: s.policy.mode,
        baseline: BaselineSummary {
            recorded: !s.baseline.is_empty(),
            recorded_at: s.baseline.recorded_at.clone(),
            path: format!("{}/{}", s.section, crate::knowledge::baseline::FILE),
            nodes: s.baseline.nodes.len(),
            verified: s.baseline.verified.len(),
            debt: s.baseline.debt.len(),
            accepted: s.baseline.accepted.len(),
            canonicality: s.baseline.canonicality.len(),
        },
        nodes: m.nodes.len(),
        claims,
        evidence: m.evidence.len(),
        relations: m.relations.len(),
        kinds: m.kinds().into_iter().map(|(k, n)| (k.to_string(), n)).collect(),
        freshness: m.freshness_tallies(),
        provenance: m.provenance_tallies(),
        conflicts_open: open,
        conflicts_accepted: m.conflicts.len() - open,
        gaps,
        coverage: m.coverage.clone(),
        check,
        canonicality: CanonicalitySummary {
            verdict: audit.verdict.clone(),
            capabilities: audit.capabilities.len(),
            canonical: audit.canonical,
            mms_centi: audit.mms_centi,
            violations: audit.violations.len(),
            counting: audit.violations.iter().filter(|v| !v.tolerated && v.excepted_by.is_none()).count(),
        },
        semantic: SemanticSummary {
            enabled: s.policy.semantic.enabled,
            provider: s.policy.semantic.provider.clone(),
            allow_remote: s.policy.semantic.allow_remote,
            derived_claims,
        },
        extractors: m.extractors.iter().map(|e| e.id.clone()).collect(),
        diagnostics: m.diagnostics.len(),
        findings: m
            .diagnostics
            .iter()
            .map(|d| format!("{}: {}{}", d.code, d.message, d.path.as_deref().map(|p| format!(" ({p})")).unwrap_or_default()))
            .collect(),
        summary,
    })
}

fn audit_of(ctx: &Context, s: &Scanned) -> Result<Audit, CapabilityError> {
    let inputs = s.inputs(ctx);
    let exceptions = crate::knowledge::canonicality::Exceptions::load(&inputs.exceptions_path())
        .map_err(|e| CapabilityError::Refused(e.to_string()))?;
    Ok(crate::knowledge::canonicality::audit(
        &s.model,
        &inputs,
        &s.baseline,
        &exceptions,
        &crate::knowledge::canonicality::today(),
    ))
}

// --------------------------------------------------------------------- list

impl BenchmarkCases for Filter {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", Filter::default()),
            NamedCase::new(
                "documents",
                Filter {
                    kind: Some("document".into()),
                    limit: 20,
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "debt",
                Filter {
                    debt: true,
                    ..Default::default()
                },
            ),
        ]
    }
}

fn list(ctx: &Context, input: Filter) -> Result<Page, CapabilityError> {
    let s = scan(ctx)?;
    Ok(query::list(&s.model, &input))
}

// ---------------------------------------------------------------------- get

/// The input of `knowledge.get` and `knowledge.explain`: one node.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeNodeInput")]
pub struct NodeInput {
    /// The node id (`component:majordomus-cli`, `rule:project.alpha@1`), a capability id
    /// (`objects.get`), or an object URI (`majordomus://rule/project.alpha@1`).
    pub id: String,
}

impl BenchmarkCases for NodeInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "readme",
            NodeInput {
                id: "document:README.md".into(),
            },
        )]
    }
}

/// The node an id, a capability id or a URI names.
pub fn resolve_id(model: &KnowledgeModel, ctx: &Context, id: &str) -> Option<String> {
    if model.node(id).is_some() {
        return Some(id.to_string());
    }
    if let Some(o) = ctx.index.get(id) {
        let candidate = format!("{}:{}", o.kind, o.identity);
        if model.node(&candidate).is_some() {
            return Some(candidate);
        }
    }
    if ctx.registry.get(id).is_some() {
        let candidate = format!("capability:{id}");
        if model.node(&candidate).is_some() {
            return Some(candidate);
        }
    }
    // a path
    let candidate = format!("document:{id}");
    if model.node(&candidate).is_some() {
        return Some(candidate);
    }
    let candidate = format!("file:{id}");
    if model.node(&candidate).is_some() {
        return Some(candidate);
    }
    model
        .nodes
        .iter()
        .find(|n| n.source.as_deref() == Some(id))
        .map(|n| n.id.clone())
}

fn not_found(model: &KnowledgeModel, id: &str) -> CapabilityError {
    let near: Vec<String> = query::search(model, id, 3).into_iter().map(|h| h.id).collect();
    CapabilityError::NotFound(if near.is_empty() {
        format!("no knowledge node `{id}`; `knowledge list` shows what exists")
    } else {
        format!("no knowledge node `{id}`; nearest: {}", near.join(", "))
    })
}

fn get_node(ctx: &Context, input: NodeInput) -> Result<NodeView, CapabilityError> {
    let s = scan(ctx)?;
    let id = resolve_id(&s.model, ctx, &input.id).ok_or_else(|| not_found(&s.model, &input.id))?;
    query::show(&s.model, &id).ok_or_else(|| not_found(&s.model, &input.id))
}

fn explain(ctx: &Context, input: NodeInput) -> Result<Explanation, CapabilityError> {
    let s = scan(ctx)?;
    let id = resolve_id(&s.model, ctx, &input.id).ok_or_else(|| not_found(&s.model, &input.id))?;
    query::explain(&s.model, &id).ok_or_else(|| not_found(&s.model, &input.id))
}

// ------------------------------------------------------------------- search

/// The input of `knowledge.search`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeSearchInput")]
pub struct SearchInput {
    /// What to look for, case-insensitive.
    pub query: String,
    /// At most this many hits; `0` for the default.
    #[serde(default)]
    pub limit: usize,
}

impl BenchmarkCases for SearchInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "readme",
            SearchInput {
                query: "readme".into(),
                limit: 10,
            },
        )]
    }
}

/// The answer of `knowledge.search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeSearchResult")]
pub struct SearchResult {
    /// The query.
    pub query: String,
    /// The hits, best first.
    pub hits: Vec<Hit>,
}

fn search(ctx: &Context, input: SearchInput) -> Result<SearchResult, CapabilityError> {
    let s = scan(ctx)?;
    Ok(SearchResult {
        hits: query::search(&s.model, &input.query, input.limit),
        query: input.query,
    })
}

// -------------------------------------------------------------------- graph

/// The input of `knowledge.graph`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeGraphInput")]
pub struct GraphInput {
    /// Cut the slice around this node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// How many hops from the root; `2` when unset.
    #[serde(default)]
    pub depth: usize,
    /// Without a root: only nodes of this kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// At most this many nodes; the executable's cap applies.
    #[serde(default)]
    pub limit: usize,
}

impl BenchmarkCases for GraphInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "components",
                GraphInput {
                    kind: Some("component".into()),
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "around-readme",
                GraphInput {
                    root: Some("document:README.md".into()),
                    depth: 1,
                    ..Default::default()
                },
            ),
        ]
    }
}

fn graph(ctx: &Context, input: GraphInput) -> Result<GraphSlice, CapabilityError> {
    let s = scan(ctx)?;
    let root = match &input.root {
        Some(r) => Some(resolve_id(&s.model, ctx, r).ok_or_else(|| not_found(&s.model, r))?),
        None => None,
    };
    let filter = Filter {
        kind: input.kind.clone(),
        ..Default::default()
    };
    Ok(query::graph(&s.model, root.as_deref(), input.depth, &filter, input.limit))
}

// ------------------------------------------------------------------- impact

/// The input of `knowledge.impact`: a change set.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeImpactInput")]
pub struct ImpactInput {
    /// The base to compare with; `HEAD` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Compare `base` with this revision instead of with the working tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    /// Name the changed paths outright instead of asking git.
    #[serde(default)]
    pub paths: Vec<String>,
}

impl BenchmarkCases for ImpactInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("working-tree", ImpactInput::default()),
            NamedCase::new(
                "readme",
                ImpactInput {
                    paths: vec!["README.md".into()],
                    ..Default::default()
                },
            ),
        ]
    }
}

fn impact_report(ctx: &Context, input: ImpactInput) -> Result<impact::ImpactReport, CapabilityError> {
    let s = scan(ctx)?;
    let root = s.root.clone();
    let (change_set, base, changed) = if !input.paths.is_empty() {
        let changed = input
            .paths
            .iter()
            .map(|p| crate::git::ChangedPath {
                path: p.trim_start_matches("./").to_string(),
                status: "modified".into(),
            })
            .collect();
        ("paths", None, changed)
    } else if let Some(to) = &input.to {
        let base = input.base.clone().unwrap_or_else(|| "HEAD".into());
        let changed = crate::git::changed_between(&root, &base, to)
            .map_err(|e| CapabilityError::Refused(format!("git: {e}")))?;
        ("revisions", Some(base), changed)
    } else {
        let base = input.base.clone().unwrap_or_else(|| "HEAD".into());
        let changed = match crate::git::changed_paths(&root, Some(&base)) {
            Ok(c) => c,
            Err(e) if matches!(ctx.index.repository.git, crate::git::GitState::Unavailable { .. }) => {
                let _ = e;
                Vec::new()
            }
            Err(e) => return Err(CapabilityError::Refused(format!("git: {e}"))),
        };
        ("working-tree", Some(base), changed)
    };
    let extractors = crate::knowledge::extract::builtin::extractors();
    let base_rev = base.clone().unwrap_or_else(|| "HEAD".into());
    let to_rev = input.to.clone();
    let root_for_base = root.clone();
    let root_for_live = root.clone();
    let base_content = move |p: &str| crate::git::show(&root_for_base, &base_rev, p);
    let live_content = move |p: &str| match &to_rev {
        Some(rev) => crate::git::show(&root_for_live, rev, p),
        None => std::fs::read_to_string(root_for_live.join(p)).ok(),
    };
    Ok(impact::analyse(&s.model, &extractors, change_set, base, changed, &base_content, &live_content))
}

// --------------------------------------------------------------------- gaps

/// The input of `knowledge.gaps`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeGapsInput")]
pub struct GapsInput {
    /// Only gaps of this category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<GapCategory>,
}

impl BenchmarkCases for GapsInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", GapsInput::default()),
            NamedCase::new(
                "unresolved",
                GapsInput {
                    category: Some(GapCategory::UnresolvedReference),
                },
            ),
        ]
    }
}

/// The answer of `knowledge.gaps`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeGapsReport")]
pub struct GapsReport {
    /// The gaps, by id.
    pub gaps: Vec<Gap>,
    /// How many of each category, over the whole model.
    pub tallies: BTreeMap<String, usize>,
}

fn gaps(ctx: &Context, input: GapsInput) -> Result<GapsReport, CapabilityError> {
    let s = scan(ctx)?;
    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    for g in &s.model.gaps {
        *tallies.entry(crate::knowledge::extract::gap_word(g.category).to_string()).or_insert(0) += 1;
    }
    Ok(GapsReport {
        gaps: s
            .model
            .gaps
            .iter()
            .filter(|g| input.category.is_none_or(|c| c == g.category))
            .cloned()
            .collect(),
        tallies,
    })
}

// ----------------------------------------------------------------- coverage

/// The answer of `knowledge.coverage`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeCoverageReport")]
pub struct CoverageReport {
    /// The rows.
    pub rows: Vec<crate::knowledge::model::CoverageRow>,
}

fn coverage(ctx: &Context, _: Empty) -> Result<CoverageReport, CapabilityError> {
    let s = scan(ctx)?;
    Ok(CoverageReport {
        rows: s.model.coverage.rows.clone(),
    })
}

// ---------------------------------------------------------------- conflicts

/// The input of `knowledge.conflicts`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeConflictsInput")]
pub struct ConflictsInput {
    /// Only open conflicts.
    #[serde(default)]
    pub open_only: bool,
}

impl BenchmarkCases for ConflictsInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("all", ConflictsInput::default())]
    }
}

/// The answer of `knowledge.conflicts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeConflictsReport")]
pub struct ConflictsReport {
    /// The conflicts, by id.
    pub conflicts: Vec<Conflict>,
    /// Open ones, over the whole model.
    pub open: usize,
    /// Accepted ones.
    pub accepted: usize,
}

fn conflicts(ctx: &Context, input: ConflictsInput) -> Result<ConflictsReport, CapabilityError> {
    let s = scan(ctx)?;
    let open = s.model.conflicts.iter().filter(|c| c.resolution == Resolution::Open).count();
    Ok(ConflictsReport {
        conflicts: s
            .model
            .conflicts
            .iter()
            .filter(|c| !input.open_only || c.resolution == Resolution::Open)
            .cloned()
            .collect(),
        open,
        accepted: s.model.conflicts.len() - open,
    })
}

// -------------------------------------------------------------------- check

/// The input of `knowledge.check`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeCheckInput")]
pub struct CheckInput {
    /// The mode to check in; the policy's when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
}

impl BenchmarkCases for CheckInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("policy-mode", CheckInput::default()),
            NamedCase::new(
                "observe",
                CheckInput {
                    mode: Some(Mode::Observe),
                },
            ),
        ]
    }
}

fn check(ctx: &Context, input: CheckInput) -> Result<CheckReport, CapabilityError> {
    let s = scan(ctx)?;
    Ok(crate::knowledge::baseline::check(
        &s.model,
        &s.baseline,
        input.mode.unwrap_or(s.policy.mode),
    ))
}

// ------------------------------------------------------------- canonicality

/// The input of `rks.canonicality`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanonicalityInput {
    /// Only this capability's row, with every violation still listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
}

impl BenchmarkCases for CanonicalityInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", CanonicalityInput::default()),
            NamedCase::new(
                "one",
                CanonicalityInput {
                    capability: Some("rks.status".into()),
                },
            ),
        ]
    }
}

fn canonicality(ctx: &Context, input: CanonicalityInput) -> Result<Audit, CapabilityError> {
    let s = scan(ctx)?;
    let mut audit = audit_of(ctx, &s)?;
    if let Some(id) = &input.capability {
        let id = id.trim_start_matches("capability:");
        if !audit.capabilities.iter().any(|c| c.id == id) {
            return Err(CapabilityError::NotFound(format!(
                "no query or command `{id}` in the registry; `capabilities list` shows what exists"
            )));
        }
        audit.capabilities.retain(|c| c.id == id);
    }
    Ok(audit)
}

// ------------------------------------------------------------------ context

/// The input of `knowledge.context`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeContextInput")]
pub struct ContextInput {
    /// The paths about to be touched; the whole repository when empty.
    #[serde(default)]
    pub paths: Vec<String>,
    /// The budget in bytes of rendered JSON; the default when `0`.
    #[serde(default)]
    pub budget: usize,
    /// Only public knowledge: what may leave the machine.
    #[serde(default)]
    pub public_only: bool,
}

impl BenchmarkCases for ContextInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "docs",
                ContextInput {
                    paths: vec!["docs".into()],
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "public",
                ContextInput {
                    paths: vec![],
                    budget: 4000,
                    public_only: true,
                },
            ),
        ]
    }
}

fn context(ctx: &Context, input: ContextInput) -> Result<ContextBundle, CapabilityError> {
    let s = scan(ctx)?;
    Ok(query::context(&s.model, &input.paths, input.budget, input.public_only))
}

// --------------------------------------------------------------- extractors

/// The answer of `knowledge.extractors`: how the model is made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeExtractorsReport")]
pub struct ExtractorsReport {
    /// Every extractor, in the order they run.
    pub extractors: Vec<ExtractorInfo>,
    /// Every node kind, with the extractor that declares it.
    pub kinds: Vec<KindOwner>,
    /// Every relation kind.
    pub relations: Vec<RelationInfo>,
    /// Every predicate.
    pub predicates: Vec<PredicateInfo>,
    /// The semantic providers this executable ships.
    pub providers: Vec<ProviderInfo>,
    /// The schema versions this executable reads and writes, and the migrations it knows.
    pub schemas: Vec<Value>,
}

/// A node kind and who declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeKindOwner")]
pub struct KindOwner {
    /// The kind.
    #[serde(flatten)]
    pub kind: KindInfo,
    /// The extractor.
    pub extractor: String,
}

/// A semantic provider, described.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeProviderInfo")]
pub struct ProviderInfo {
    /// The id.
    pub id: String,
    /// The model it uses.
    pub model: String,
    /// Whether it sends text off the machine.
    pub remote: bool,
    /// The operations.
    pub operations: Vec<String>,
}

fn extractors(ctx: &Context, _: Empty) -> Result<ExtractorsReport, CapabilityError> {
    let s = scan(ctx)?;
    let m = &s.model;
    let mut kinds: Vec<KindOwner> = m
        .kind_vocabulary()
        .into_iter()
        .map(|(_, (e, k))| KindOwner {
            kind: k.clone(),
            extractor: e.to_string(),
        })
        .collect();
    kinds.sort_by(|a, b| a.kind.kind.cmp(&b.kind.kind));
    let mut relations: Vec<RelationInfo> = m.relation_kinds().into_values().cloned().collect();
    relations.sort_by(|a, b| a.kind.cmp(&b.kind));
    let mut predicates: Vec<PredicateInfo> = m.predicates().into_values().cloned().collect();
    predicates.sort_by(|a, b| a.name.cmp(&b.name));
    let providers = crate::knowledge::semantic::providers()
        .iter()
        .map(|p| ProviderInfo {
            id: p.id().into(),
            model: p.model().into(),
            remote: p.remote(),
            operations: p.operations().iter().map(|s| s.to_string()).collect(),
        })
        .collect();
    let mut schemas = vec![serde_json::json!({
        "families": [
            crate::knowledge::migrate::Family::Model.schema(),
            crate::knowledge::migrate::Family::Baseline.schema(),
            crate::knowledge::migrate::Family::Exceptions.schema(),
            crate::knowledge::migrate::Family::SemanticCache.schema(),
        ]
    })];
    schemas.extend(crate::knowledge::migrate::table());
    Ok(ExtractorsReport {
        extractors: m.extractors.clone(),
        kinds,
        relations,
        predicates,
        providers,
        schemas,
    })
}

// ---------------------------------------------------------------- proposals

fn proposals(ctx: &Context, _: Empty) -> Result<Reconciliation, CapabilityError> {
    let s = scan(ctx)?;
    let (r, _) = crate::knowledge::reconcile::reconcile(&s.model, &s.baseline, false);
    Ok(r)
}

// ------------------------------------------------------------------ inspect

/// The input of `rks.inspect`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeInspectInput")]
pub struct InspectInput {
    /// The base to compare the working tree with; `HEAD` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
}

impl BenchmarkCases for InspectInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("working-tree", InspectInput::default())]
    }
}

fn inspect(ctx: &Context, input: InspectInput) -> Result<crate::knowledge::inspect::Inspection, CapabilityError> {
    let s = scan(ctx)?;
    let base = input.base.clone().unwrap_or_else(|| "HEAD".into());
    let impact = impact_report(
        ctx,
        ImpactInput {
            base: Some(base.clone()),
            to: None,
            paths: vec![],
        },
    )?;
    let audit = audit_of(ctx, &s)?;
    crate::knowledge::inspect::inspect(ctx, &s, impact, &audit, &base)
}

// -------------------------------------------------------------------- model

/// The input of `knowledge.model`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeModelInput")]
pub struct ModelInput {
    /// Only the public projection: what may leave the repository.
    #[serde(default)]
    pub public: bool,
}

impl BenchmarkCases for ModelInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("whole", ModelInput::default()),
            NamedCase::new("public", ModelInput { public: true }),
        ]
    }
}

fn model(ctx: &Context, input: ModelInput) -> Result<KnowledgeModel, CapabilityError> {
    let s = scan(ctx)?;
    Ok(if input.public {
        query::public_projection(&s.model)
    } else {
        s.model.clone()
    })
}

fn cli(path: &[&str]) -> Option<CliExposure> {
    Some(CliExposure {
        path: path.iter().map(|s| s.to_string()).collect(),
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "rks",
        title: "Repository knowledge",
        description: "What the repository knows about itself: typed nodes with claims, evidence and relations, read off the tree by deterministic extractors and held against a committed baseline for freshness, conflicts, coverage and canonicality. One scan per process, shared; every capability here is a slice of it. Nothing here writes: the baseline is recorded and a reconciliation accepted from the command line.",
        stability: Stability::Implemented,
        capabilities: [
            capability! {
                id: "rks.status",
                title: "The knowledge status",
                description: "One scan, summarised: the repository and the reference it is judged against, nodes by kind, freshness and provenance, open conflicts, gaps by category, every coverage row, the check against the baseline in the policy's mode, the canonicality audit's verdict and the manual maintenance surface, and which extractors ran.",
                input: Empty,
                output: StatusReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_knowledge".into()),
                        resource: Some(McpResource { uri: KNOWLEDGE_URI.into(), name: "knowledge".into() }),
                    }),
                    http: get("/api/v1/knowledge"),
                    cli: cli(&["knowledge", "status"]),
                },
                tags: ["knowledge", "introspection"],
                cache: CACHE,
                handler: status,
            },
            capability! {
                id: "rks.list",
                title: "List knowledge nodes",
                description: "The nodes a filter matches — by kind, provenance, freshness, ownership, visibility, extractor, or a substring of id or title; optionally only debt — one page at a time, each with its freshness and the reason it is not current.",
                input: Filter,
                output: Page,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_list"),
                    http: get("/api/v1/knowledge/nodes"),
                    cli: cli(&["knowledge", "list"]),
                },
                tags: ["knowledge"],
                cache: CACHE,
                handler: list,
            },
            capability! {
                id: "rks.get",
                title: "One knowledge node",
                description: "One node with everything that bears on it: its claims with their provenance, evidence, verification and freshness; its evidence resolved with fingerprints; the relations in and out; the conflicts it is the subject of; the gaps about it. The id may be a node id, a capability id, an object URI or a path.",
                input: NodeInput,
                output: NodeView,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_get"),
                    http: get("/api/v1/knowledge/node"),
                    cli: cli(&["knowledge", "show"]),
                },
                tags: ["knowledge"],
                cache: CACHE,
                handler: get_node,
            },
            capability! {
                id: "rks.search",
                title: "Search the knowledge",
                description: "Ids and titles first, then summaries, then claim values; ranked by where the query matched and then by id, so that two runs agree.",
                input: SearchInput,
                output: SearchResult,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_search"),
                    http: get("/api/v1/knowledge/search"),
                    cli: cli(&["knowledge", "search"]),
                },
                tags: ["knowledge", "search"],
                cache: CACHE,
                handler: search,
            },
            capability! {
                id: "rks.explain",
                title: "Why the model says what it says",
                description: "One node explained: how it is known (observed, declared, curated, derived), what evidence it rests on with fingerprints, every claim with its freshness and the reason, what it relates to, the conflicts it is party to, the gaps about it, and what to do. For a capability, the same answer names its canonical source and every derived surface.",
                input: NodeInput,
                output: Explanation,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_explain"),
                    http: get("/api/v1/knowledge/explain"),
                    cli: cli(&["knowledge", "explain"]),
                },
                tags: ["knowledge", "why"],
                cache: CACHE,
                handler: explain,
            },
            capability! {
                id: "rks.graph",
                title: "A slice of the knowledge graph",
                description: "Nodes and typed relations: around one root to a depth, or every node of one kind, capped. Each node carries its freshness so that a drawing can colour what is stale.",
                input: GraphInput,
                output: GraphSlice,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_graph"),
                    http: get("/api/v1/knowledge/graph"),
                    cli: cli(&["knowledge", "graph"]),
                },
                tags: ["knowledge", "graph"],
                cache: CACHE,
                handler: graph,
            },
            capability! {
                id: "rks.impact",
                title: "What a change set touches",
                description: "The working tree against a base, two revisions, or named paths: the entries that changed where an extractor can tell, the nodes whose evidence changed, the claims resting on it, the nodes reached along propagating relations with the hop count, which extractors a scan would re-run, and the changed paths nothing in the model knows about.",
                input: ImpactInput,
                output: impact::ImpactReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_impact"),
                    http: get("/api/v1/knowledge/impact"),
                    cli: cli(&["knowledge", "impact"]),
                },
                tags: ["knowledge", "change"],
                cache: CACHE,
                handler: impact_report,
            },
            capability! {
                id: "rks.gaps",
                title: "The gaps",
                description: "Everything the repository could know and does not: undocumented components, unresolved references, unverified curated knowledge, unexercised capabilities, canonicality violations — each with the reason and the remedy, optionally one category.",
                input: GapsInput,
                output: GapsReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_gaps"),
                    http: get("/api/v1/knowledge/gaps"),
                    cli: cli(&["knowledge", "gaps"]),
                },
                tags: ["knowledge"],
                cache: CACHE,
                handler: gaps,
            },
            capability! {
                id: "rks.coverage",
                title: "Coverage",
                description: "Over denominators the executable defines deterministically — components, capabilities, references, curated records, generated artifacts, layer objects — how many are covered and which are missing. Numbers, never percentages.",
                input: Empty,
                output: CoverageReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_coverage"),
                    http: get("/api/v1/knowledge/coverage"),
                    cli: cli(&["knowledge", "coverage"]),
                },
                tags: ["knowledge", "coverage"],
                cache: CACHE,
                handler: coverage,
            },
            capability! {
                id: "rks.conflicts",
                title: "The conflicts",
                description: "Every subject with two values for one functional predicate from two sources: both sides with their provenance and evidence, the severity, the basis, whether a person accepted it, and the remedy.",
                input: ConflictsInput,
                output: ConflictsReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_conflicts"),
                    http: get("/api/v1/knowledge/conflicts"),
                    cli: cli(&["knowledge", "conflicts"]),
                },
                tags: ["knowledge"],
                cache: CACHE,
                handler: conflicts,
            },
            capability! {
                id: "rks.check",
                title: "The check against the baseline",
                description: "New debt the baseline does not tolerate, debt it tolerates, and debt it tolerates that is gone; the verdict in the policy's mode or the one given: observe and warn never fail, protect fails on new debt, strict fails on any. What `majordomus knowledge check` and the CI gate print.",
                input: CheckInput,
                output: CheckReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_check"),
                    http: get("/api/v1/knowledge/check"),
                    cli: cli(&["knowledge", "check"]),
                },
                tags: ["knowledge", "gate"],
                cache: CACHE,
                handler: check,
            },
            capability! {
                id: "rks.canonicality",
                title: "The canonicality audit",
                description: "Every query and command with its one canonical source, the surfaces derived from it, the hand-written files that name it and the manual maintenance surface that follows; every orphan projection, undeclared generated file, missing artifact, suspected mirror and expired exception; the typed exceptions with their status; the verdict.",
                input: CanonicalityInput,
                output: Audit,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_canonicality"),
                    http: get("/api/v1/canonicality"),
                    cli: cli(&["knowledge", "canonicality"]),
                },
                tags: ["knowledge", "canonicality", "gate"],
                cache: CACHE,
                handler: canonicality,
            },
            capability! {
                id: "rks.context",
                title: "The context for some paths",
                description: "What an agent should read before touching some paths: the nodes whose sources are under them and what those govern, describe and depend on one hop out, most governing first; the claims among them that are not current, as caveats; the open conflicts and the gaps; cut to a budget, and to public knowledge only when asked.",
                input: ContextInput,
                output: ContextBundle,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_context"),
                    http: get("/api/v1/knowledge/context"),
                    cli: cli(&["knowledge", "context"]),
                },
                tags: ["knowledge", "context"],
                cache: CACHE,
                handler: context,
            },
            capability! {
                id: "rks.extractors",
                title: "How the model is made",
                description: "Every extractor with the kinds, relations and predicates it declares; the node kind vocabulary with who owns each kind; the semantic providers this executable ships and whether each is remote; the schema versions read and written and the migrations known.",
                input: Empty,
                output: ExtractorsReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_extractors"),
                    http: get("/api/v1/knowledge/extractors"),
                    cli: cli(&["knowledge", "extractors"]),
                },
                tags: ["knowledge", "introspection"],
                cache: CACHE,
                handler: extractors,
            },
            capability! {
                id: "rks.proposals",
                title: "The reconciliation proposals",
                description: "What to do about every open conflict, stale or unverified claim, unresolved reference and gap, as proposals with an owner: which a person edits, and which `majordomus knowledge reconcile --accept` applies by recording a verification in the baseline.",
                input: Empty,
                output: Reconciliation,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_knowledge_proposals"),
                    http: get("/api/v1/knowledge/proposals"),
                    cli: cli(&["knowledge", "reconcile"]),
                },
                tags: ["knowledge"],
                cache: CACHE,
                handler: proposals,
            },
            capability! {
                id: "rks.inspect",
                title: "Inspect a change set",
                description: "What a change set means for the knowledge before it is merged: the paths that changed against a base, the nodes and claims they touch, every capability the change adds with the checklist of surfaces derived for it, and the debt it introduces — freshness the baseline does not tolerate and canonicality violations that count. The pull-request gate.",
                input: InspectInput,
                output: crate::knowledge::inspect::Inspection,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_change_inspect"),
                    http: get("/api/v1/knowledge/inspect"),
                    cli: cli(&["knowledge", "inspect"]),
                },
                tags: ["knowledge", "change", "gate"],
                cache: CACHE,
                handler: inspect,
            },
            capability! {
                id: "rks.model",
                title: "The whole model",
                description: "The knowledge model as one document, `majordomus/knowledge/v1`: every extractor, evidence, node with claims, relation, conflict, gap and coverage row, with the fingerprint. Optionally the public projection only, which is what leaves the repository.",
                input: ModelInput,
                output: KnowledgeModel,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_knowledge_model".into()),
                        resource: Some(McpResource { uri: MODEL_URI.into(), name: "knowledge-model".into() }),
                    }),
                    http: get("/api/v1/knowledge/model"),
                    cli: cli(&["knowledge", "scan"]),
                },
                tags: ["knowledge", "export"],
                cache: CACHE,
                handler: model,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_module_declares_every_read_of_the_knowledge_system_and_no_write() {
        let m = module();
        let ids: Vec<&str> = m.capabilities.iter().map(|c| c.capability.id.as_str()).collect();
        for want in [
            "rks.status",
            "rks.list",
            "rks.get",
            "rks.search",
            "rks.explain",
            "rks.graph",
            "rks.impact",
            "rks.gaps",
            "rks.coverage",
            "rks.conflicts",
            "rks.check",
            "rks.canonicality",
            "rks.context",
            "rks.extractors",
            "rks.proposals",
            "rks.inspect",
            "rks.model",
        ] {
            assert!(ids.contains(&want), "{want} missing from {ids:?}");
        }
        for c in &m.capabilities {
            assert_eq!(
                c.capability.kind,
                crate::capability::CapabilityKind::Query,
                "{} writes nothing and is a query",
                c.capability.id
            );
            assert!(c.capability.exposure.http.is_some(), "{} has a route", c.capability.id);
            assert!(c.capability.exposure.mcp.is_some(), "{} has a tool", c.capability.id);
            assert!(c.capability.exposure.cli.is_some(), "{} has a command", c.capability.id);
        }
    }

    #[test]
    fn an_id_a_capability_a_uri_and_a_path_all_resolve_to_a_node() {
        // resolution is a pure function of the model and the context's lookups; the
        // integration tests exercise it against a served repository, and this one holds
        // the fall-through order: node id, object URI, capability id, document, file
        let m = crate::knowledge::freshness::tests_support::empty_model();
        assert!(m.node("document:README.md").is_none());
    }
}
