//! The repository knowledge system: what the repository knows about itself, read off
//! the tree by extractors, held as typed nodes with claims, evidence and relations, and
//! checked for freshness, conflicts, coverage and canonicality against a committed
//! baseline.
//!
//! One scan is one deterministic function of the checkout: the same tree, the same
//! baseline and the same executable answer the same model, fingerprint included. Nothing
//! here writes to the repository except the two explicit operations a person runs —
//! recording the baseline and accepting a reconciliation — and nothing here calls a
//! network; the semantic layer is a trait behind a policy switch, and its cached
//! derivations are read like any other fact of the checkout.
//!
//! The model is [`model::KnowledgeModel`]; the extractors are under [`extract`]; the
//! passes that turn an extraction into a model are the sibling modules, each one
//! function of the model and the baseline. The capability module under
//! `capability::builtin::knowledge` projects the model over MCP, HTTP and the command
//! line from this one place.

pub mod baseline;
pub mod canonicality;
pub mod conflict;
pub mod coverage;
pub mod extract;
pub mod freshness;
pub mod impact;
pub mod inspect;
pub mod migrate;
pub mod model;
pub mod query;
pub mod reconcile;
pub mod semantic;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::app::App;
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::CapabilityRegistry;
use crate::error::Result;
use crate::git::GitState;
use crate::index::Index;

use self::baseline::Baseline;
use self::extract::{Extraction, ExtractionContext, Extractor};
use self::model::{
    Evidence, KnowledgeModel, Node, Provenance, ReferenceInfo, Relation, RepositoryIdentity,
    SCHEMA,
};

/// Everything a scan reads, gathered once from a loaded application so that the passes
/// take values and not the world.
pub struct Inputs<'a> {
    /// The repository root, absolute.
    pub root: &'a Path,
    /// The layer's index.
    pub index: &'a Index,
    /// The capability registry.
    pub registry: &'a CapabilityRegistry,
    /// The local half of the layer, repository-relative.
    pub local: String,
    /// The knowledge section, repository-relative: where the baseline and the exceptions
    /// live.
    pub section: String,
    /// Every tracked path, sorted.
    pub tracked: Arc<Vec<String>>,
    /// The knowledge policy.
    pub policy: baseline::KnowledgePolicy,
}

impl<'a> Inputs<'a> {
    /// Gather the inputs of a scan from a loaded application: the tracked files from git,
    /// the policy from the layer.
    pub fn from_app(app: &'a App) -> Result<Self> {
        let root = app.repository.root();
        let tracked = match &app.index().repository.git {
            GitState::Available(_) => crate::git::ls_files_all(root).unwrap_or_default(),
            GitState::Unavailable { .. } => Vec::new(),
        };
        let policy = baseline::KnowledgePolicy::load(&app.repository);
        Ok(Inputs {
            root,
            index: app.index(),
            registry: app.registry(),
            local: app.repository.local_path(),
            section: section_of(&app.repository),
            tracked: Arc::new(tracked),
            policy,
        })
    }

    /// Gather the inputs from a capability context: the repository is opened from the
    /// index's root, which is the one fact about it the context carries.
    pub fn from_context(ctx: &'a Context) -> Result<Self> {
        let root = Path::new(&ctx.index.repository.root);
        let repository = crate::repository::Repository::open(root)?;
        let tracked = match &ctx.index.repository.git {
            GitState::Available(_) => crate::git::ls_files_all(root).unwrap_or_default(),
            GitState::Unavailable { .. } => Vec::new(),
        };
        Ok(Inputs {
            root,
            index: &ctx.index,
            registry: &ctx.registry,
            local: repository.local_path(),
            section: section_of(&repository),
            tracked: Arc::new(tracked),
            policy: baseline::KnowledgePolicy::load(&repository),
        })
    }

    /// The extraction context over these inputs.
    pub fn context(&self) -> ExtractionContext<'_> {
        ExtractionContext {
            root: self.root,
            index: self.index,
            registry: self.registry,
            tracked: &self.tracked,
            git: &self.index.repository.git,
            scope: &self.index.scoped.scope,
            local: &self.local,
        }
    }

    /// The path of the baseline file.
    pub fn baseline_path(&self) -> PathBuf {
        self.root.join(&self.section).join(baseline::FILE)
    }

    /// The path of the exceptions file.
    pub fn exceptions_path(&self) -> PathBuf {
        self.root.join(&self.section).join(canonicality::EXCEPTIONS_FILE)
    }

    /// The baseline, as committed; empty when there is none.
    pub fn baseline(&self) -> Result<Baseline> {
        Baseline::load(&self.baseline_path())
    }
}

/// The knowledge section of a repository, repository-relative.
fn section_of(repository: &crate::repository::Repository) -> String {
    repository
        .section_path("knowledge")
        .unwrap_or_else(|| format!("{}/knowledge", repository.repo_path()))
}

/// One scan and what it was made from, shared by every capability of one process.
pub struct Scanned {
    /// The model.
    pub model: KnowledgeModel,
    /// The baseline it was held against.
    pub baseline: Baseline,
    /// The repository root.
    pub root: PathBuf,
    /// The local half of the layer.
    pub local: String,
    /// The knowledge section.
    pub section: String,
    /// The tracked paths.
    pub tracked: Arc<Vec<String>>,
    /// The policy.
    pub policy: baseline::KnowledgePolicy,
    /// What the scan was keyed on.
    key: String,
    /// When it was made.
    at: Instant,
}

impl Scanned {
    /// The inputs of this scan, over a context: for a pass that runs after the scan.
    pub fn inputs<'a>(&'a self, ctx: &'a Context) -> Inputs<'a> {
        Inputs {
            root: &self.root,
            index: &ctx.index,
            registry: &ctx.registry,
            local: self.local.clone(),
            section: self.section.clone(),
            tracked: Arc::clone(&self.tracked),
            policy: self.policy.clone(),
        }
    }
}

/// How long a served process trusts one scan before it looks at the tree again.
pub const SCAN_TTL_SECONDS: u64 = 5;

static MEMO: Mutex<Option<Arc<Scanned>>> = Mutex::new(None);

/// What a scan is keyed on: the index, the registry, and the two files the passes read
/// beside them. A change to any of them is a new scan; anything else waits for the TTL.
fn scan_key(ctx: &Context, inputs: &Inputs<'_>) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(ctx.index.fingerprint.as_bytes());
    h.update(ctx.registry.fingerprint().as_bytes());
    h.update(inputs.root.as_os_str().as_encoded_bytes());
    for p in [inputs.baseline_path(), inputs.exceptions_path()] {
        h.update(std::fs::read(&p).unwrap_or_default());
    }
    let cache = semantic::cache_path(inputs.root, &inputs.local);
    if let Ok(m) = std::fs::metadata(&cache).and_then(|m| m.modified()) {
        h.update(format!("{m:?}").as_bytes());
    }
    format!("{:x}", h.finalize())
}

/// The scan of a process, made once and shared until the tree it was made from moves
/// or the TTL runs out. The capabilities read this; nothing else scans in a served
/// process.
pub fn scanned(ctx: &Context) -> std::result::Result<Arc<Scanned>, CapabilityError> {
    let inputs = Inputs::from_context(ctx).map_err(|e| CapabilityError::Internal(e.to_string()))?;
    let key = scan_key(ctx, &inputs);
    let mut memo = MEMO.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = memo.as_ref() {
        if s.key == key && s.at.elapsed().as_secs() < SCAN_TTL_SECONDS {
            return Ok(Arc::clone(s));
        }
    }
    let baseline = inputs
        .baseline()
        .map_err(|e| CapabilityError::Refused(e.to_string()))?;
    let model = scan(&inputs, &baseline);
    let s = Arc::new(Scanned {
        model,
        baseline,
        root: inputs.root.to_path_buf(),
        local: inputs.local.clone(),
        section: inputs.section.clone(),
        tracked: Arc::clone(&inputs.tracked),
        policy: inputs.policy.clone(),
        key,
        at: Instant::now(),
    });
    *memo = Some(Arc::clone(&s));
    Ok(s)
}

/// Forget the memoised scan, so that the next capability call scans again: what a
/// command that wrote the baseline calls before it reads.
pub fn forget() {
    *MEMO.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The model the extractors produce before any pass runs: merged, sorted, validated.
pub fn extract(inputs: &Inputs<'_>, extractors: &[Box<dyn Extractor>]) -> KnowledgeModel {
    let _phase = crate::perf::phase(crate::perf::Phase::IndexBuild);
    let ctx = inputs.context();
    let mut merged = Extraction::default();
    let mut infos = Vec::new();
    for e in extractors {
        infos.push(e.info(&ctx));
        merge(&mut merged, e.extract(&ctx));
    }
    let Extraction {
        mut evidence,
        mut nodes,
        mut relations,
        mut gaps,
        diagnostics,
    } = merged;
    // every node a relation names exists: a target nothing extracted is a phantom, and
    // a phantom is a gap, not a node
    let ids: std::collections::BTreeSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    relations.retain(|r| {
        let ok = ids.contains(&r.source) && ids.contains(&r.target);
        if !ok {
            let missing = if ids.contains(&r.source) { &r.target } else { &r.source };
            let id = format!("unresolved:{}", missing);
            if !gaps.iter().any(|g| g.id == id) {
                gaps.push(model::Gap {
                    id,
                    category: model::GapCategory::UnresolvedReference,
                    subject: missing.clone(),
                    reason: format!(
                        "{} {} {} names a node no extractor produced",
                        r.source, r.kind, r.target
                    ),
                    remedy: "register what the reference names, or correct it".into(),
                });
            }
        }
        ok
    });
    evidence.sort_by(|a, b| a.id.cmp(&b.id));
    evidence.dedup_by(|a, b| a.id == b.id);
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    for n in &mut nodes {
        n.claims.sort_by(|a, b| a.id.cmp(&b.id));
        n.claims.dedup_by(|a, b| a.id == b.id);
        n.evidence.sort();
        n.evidence.dedup();
    }
    relations.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then(a.kind.cmp(&b.kind))
            .then(a.target.cmp(&b.target))
    });
    relations.dedup_by(|a, b| a.source == b.source && a.kind == b.kind && a.target == b.target);
    gaps.sort_by(|a, b| a.id.cmp(&b.id));
    let fingerprint = model::fingerprint(&nodes, &evidence, &relations);
    let git = &inputs.index.repository.git;
    let (branch, head, working_tree) = match git {
        GitState::Available(g) => (g.branch.clone(), g.head.clone(), g.working_tree.clone()),
        GitState::Unavailable { reason } => (None, None, format!("unavailable: {reason}")),
    };
    let mut model = KnowledgeModel {
        schema: SCHEMA.into(),
        repository: RepositoryIdentity {
            name: extract::git::repository_name(inputs.root),
            id: crate::repository::identity(inputs.root),
            branch,
            head,
            working_tree,
        },
        reference: ReferenceInfo {
            kind: "baseline".into(),
            revision: None,
            mode: inputs.policy.mode.as_str().into(),
            path: Some(format!("{}/{}", inputs.section, baseline::FILE)),
        },
        extractors: infos,
        evidence,
        nodes,
        relations,
        conflicts: Vec::new(),
        gaps,
        coverage: model::Coverage::default(),
        diagnostics,
        fingerprint,
    };
    let mut found = model::validate(&model);
    model.diagnostics.append(&mut found);
    model
}

/// One scan: extract, then every pass against the baseline. The result is the model the
/// capabilities serve.
pub fn scan(inputs: &Inputs<'_>, baseline: &Baseline) -> KnowledgeModel {
    let extractors = extract::builtin::extractors();
    let mut model = extract(inputs, &extractors);
    conflict::detect(&mut model, baseline);
    freshness::compute(&mut model, baseline);
    coverage::compute(&mut model, inputs);
    canonicality::annotate(&mut model, inputs, baseline);
    model
}

/// One scan against whatever baseline the repository carries.
pub fn scan_app(app: &App) -> Result<(KnowledgeModel, Baseline)> {
    let inputs = Inputs::from_app(app)?;
    let baseline = inputs.baseline()?;
    let model = scan(&inputs, &baseline);
    Ok((model, baseline))
}

/// Merge one extractor's output into the running extraction. A node two extractors both
/// emitted is one node: the identity fields come from the one with the stronger
/// provenance (observed over declared over curated over derived), the claims and the
/// evidence are the union.
fn merge(into: &mut Extraction, from: Extraction) {
    let Extraction {
        evidence,
        nodes,
        relations,
        gaps,
        diagnostics,
    } = from;
    for e in evidence {
        merge_evidence(&mut into.evidence, e);
    }
    for n in nodes {
        merge_node(&mut into.nodes, n);
    }
    for r in relations {
        into.relation(r);
    }
    for g in gaps {
        if !into.gaps.iter().any(|x| x.id == g.id) {
            into.gaps.push(g);
        }
    }
    into.diagnostics.extend(diagnostics);
}

fn merge_evidence(list: &mut Vec<Evidence>, e: Evidence) {
    match list.iter_mut().find(|x| x.id == e.id) {
        Some(existing) => {
            // the narrower visibility wins: one extractor's caution covers the other's
            existing.visibility = existing.visibility.narrower(e.visibility);
            existing.remote_processing = existing.remote_processing && e.remote_processing;
        }
        None => list.push(e),
    }
}

fn strength(p: Provenance) -> u8 {
    match p {
        Provenance::Observed => 3,
        Provenance::Declared => 2,
        Provenance::Curated => 1,
        Provenance::Derived => 0,
    }
}

fn merge_node(list: &mut Vec<Node>, n: Node) {
    let Some(existing) = list.iter_mut().find(|x| x.id == n.id) else {
        list.push(n);
        return;
    };
    let incoming_stronger = strength(n.provenance) > strength(existing.provenance)
        || (strength(n.provenance) == strength(existing.provenance)
            && existing.source.is_none()
            && n.source.is_some());
    let Node {
        title,
        summary,
        provenance,
        ownership,
        visibility,
        confidence,
        evidence,
        claims,
        source,
        extractor,
        route,
        ..
    } = n;
    if incoming_stronger {
        existing.title = title;
        existing.provenance = provenance;
        existing.ownership = ownership;
        existing.confidence = confidence;
        existing.source = source;
        existing.extractor = extractor;
        if summary.is_some() {
            existing.summary = summary;
        }
        if route.is_some() {
            existing.route = route;
        }
    } else {
        if existing.summary.is_none() {
            existing.summary = summary;
        }
        if existing.route.is_none() {
            existing.route = route;
        }
    }
    existing.visibility = existing.visibility.narrower(visibility);
    for ev in evidence {
        if !existing.evidence.contains(&ev) {
            existing.evidence.push(ev);
        }
    }
    for c in claims {
        if !existing.claims.iter().any(|x| x.id == c.id) {
            existing.claims.push(c);
        }
    }
}

/// The relations of the model indexed both ways, for the passes that walk them.
pub(crate) struct Adjacency<'a> {
    pub outgoing: BTreeMap<&'a str, Vec<&'a Relation>>,
    pub incoming: BTreeMap<&'a str, Vec<&'a Relation>>,
}

impl<'a> Adjacency<'a> {
    pub fn of(relations: &'a [Relation]) -> Self {
        let mut outgoing: BTreeMap<&str, Vec<&Relation>> = BTreeMap::new();
        let mut incoming: BTreeMap<&str, Vec<&Relation>> = BTreeMap::new();
        for r in relations {
            outgoing.entry(r.source.as_str()).or_default().push(r);
            incoming.entry(r.target.as_str()).or_default().push(r);
        }
        Adjacency { outgoing, incoming }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::model::{Ownership, Visibility};
    use extract::NodeSpec;

    fn node(id: &str, p: Provenance, source: Option<&str>) -> Node {
        NodeSpec {
            id: id.into(),
            title: id.into(),
            summary: None,
            provenance: p,
            ownership: Ownership::External,
            visibility: Visibility::Public,
            evidence: vec![],
            source: source.map(str::to_string),
            extractor: "t",
        }
        .build()
    }

    #[test]
    fn the_stronger_provenance_owns_the_merged_node_and_claims_are_unioned() {
        let mut list = vec![node("component:a", Provenance::Curated, None)];
        let mut observed = node("component:a", Provenance::Observed, Some("Cargo.toml"));
        observed.title = "A".into();
        merge_node(&mut list, observed);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].provenance, Provenance::Observed);
        assert_eq!(list[0].title, "A");
        assert_eq!(list[0].source.as_deref(), Some("Cargo.toml"));
        // and a weaker one arriving later changes nothing but the claim set
        let mut derived = node("component:a", Provenance::Derived, None);
        derived.title = "guess".into();
        derived.visibility = Visibility::Internal;
        merge_node(&mut list, derived);
        assert_eq!(list[0].title, "A");
        assert_eq!(list[0].visibility, Visibility::Internal);
    }

    #[test]
    fn evidence_seen_twice_keeps_the_narrower_visibility() {
        let mut list = vec![];
        let mut a = Extraction::default();
        a.file_evidence("t", "x", b"1", Visibility::Public);
        let mut b = Extraction::default();
        b.file_evidence("t", "x", b"1", Visibility::Restricted);
        merge_evidence(&mut list, a.evidence.remove(0));
        merge_evidence(&mut list, b.evidence.remove(0));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].visibility, Visibility::Restricted);
        assert!(!list[0].remote_processing);
    }
}
