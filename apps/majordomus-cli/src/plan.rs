//! The plan: milestones, issues, the two graphs between them, and everything derived from
//! them — execution waves, issue status, milestone status, the roadmap order, the active
//! milestone, the next executable issue and every validation finding.
//!
//! **This is a second reader of one set of semantics, not a second set.** `lib/project.awk`
//! is the incumbent engine: the shell tool, the site generator and the GitHub adapter all
//! call it, and it has been the only implementation since the model existed. This module
//! derives the same values in Rust so that the capability registry can answer them — an
//! agent asking the shared MCP server what to work on next could not be told otherwise —
//! and the two are held to byte equality by `test/cases/99_plan_capabilities.sh`, which
//! runs both over every record in this repository and over a fixture built to hit the
//! edges. Where the two disagree the awk is right; this file is the one that changes.
//!
//! The order of derivation below follows `lib/project.awk` step for step, including the
//! order findings are produced in, because that order is observable: a reader diffing the
//! two engines diffs a sequence, not a set. Comments name the awk block each step mirrors.
//!
//! Nothing here reads a file. Milestones and issues are ordinary declarative objects of the
//! layer — kinds `milestone` and `issue`, whole-YAML metadata — already discovered, parsed
//! and schema-validated by the time the index holds them, so building the plan is a walk of
//! values already in memory. That is what lets it be built on demand, inside the handler of
//! a plan capability, rather than eagerly when a [`crate::capability::Context`] is composed:
//! `App::load` has a stated budget and 200 project records have no business inside it.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::index::Index;

/// The kind of a milestone record.
pub const MILESTONE: &str = "milestone";
/// The kind of an issue record.
pub const ISSUE: &str = "issue";
/// The kind of the plan's header record.
pub const PROJECT: &str = "project";

/// The issue status vocabulary, in derivation order. The same list, in the same order, as
/// `ISTATUS` in `lib/project.awk`; a status assigned that this does not declare is a
/// finding, in both engines.
pub const ISSUE_STATUSES: &[&str] = &["READY", "BLOCKED", "ACTIVE", "VERIFY", "DONE", "CANCELLED"];

/// The milestone status vocabulary, in derivation order (`MSTATUS` in `lib/project.awk`).
pub const MILESTONE_STATUSES: &[&str] = &[
    "PLANNED",
    "BLOCKED",
    "ACTIVE",
    "VERIFY",
    "DONE",
    "CANCELLED",
    "SUPERSEDED",
];

// ---------------------------------------------------------------- scalars out of metadata

/// One YAML scalar as the flattener would have written it: the text an awk comparison sees.
///
/// `lib/yaml.sh` flattens every value to a string, so `order: 3` is `"3"`, `cancelled: true`
/// is `"true"` and a missing key is `""`. The index holds the same records as typed JSON,
/// so the comparisons this module makes have to be made against the same text the awk
/// compares — otherwise `parallel_safe: false` would be a bool here and the string `"false"`
/// there, and only one of them would be right.
///
/// ```
/// use majordomus_cli::plan::scalar;
/// use serde_json::json;
/// assert_eq!(scalar(Some(&json!(3))), "3");
/// assert_eq!(scalar(Some(&json!(true))), "true");
/// assert_eq!(scalar(Some(&json!("x"))), "x");
/// assert_eq!(scalar(Some(&json!(null))), "");
/// assert_eq!(scalar(None), "");
/// ```
pub fn scalar(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(other) => other.to_string(),
    }
}

fn field(meta: &Value, key: &str) -> String {
    scalar(meta.get(key))
}

/// A list field as the flattener's `key.0`, `key.1` … would have carried it: every item as a
/// scalar, in the file's order. A scalar where a list was expected is one item, which is
/// what the flattener produces for `scope: apps/` too.
fn list(meta: &Value, key: &str) -> Vec<String> {
    match meta.get(key) {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.iter().map(|v| scalar(Some(v))).collect(),
        Some(other) => vec![scalar(Some(other))],
    }
}

/// The `covers` token of every evidence entry that carries one, in file order. The awk
/// counts the `evidence.N.covers` keys, so an entry without one is not evidence.
fn evidence_covers(meta: &Value) -> Vec<String> {
    match meta.get("evidence") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|e| e.get("covers"))
            .map(|v| scalar(Some(v)))
            .collect(),
        _ => Vec::new(),
    }
}

/// Tabs out of a value that becomes a column: `clean()` in `lib/project.awk`.
fn clean(s: &str) -> String {
    s.replace('\t', " ")
}

// ---------------------------------------------------------------- the authored records

/// One issue as its file declares it, with nothing derived.
#[derive(Debug, Clone)]
struct PlanIssueRaw {
    id: String,
    milestone: String,
    title: String,
    slug: String,
    priority: String,
    profile: String,
    parallel_safe: String,
    cancelled: bool,
    started_at: String,
    verified_at: String,
    completed_at: String,
    objective: String,
    depends_on: Vec<String>,
    scope: Vec<String>,
    acceptance_criteria: usize,
    validation: usize,
    evidence_required: Vec<String>,
    evidence_have: Vec<String>,
}

/// One milestone as its file declares it, with nothing derived.
#[derive(Debug, Clone)]
struct PlanMilestoneRaw {
    id: String,
    title: String,
    slug: String,
    order: String,
    priority: String,
    version: String,
    cancelled: bool,
    superseded_by: String,
    outcome: String,
    depends_on: Vec<String>,
    claims: Vec<String>,
    evidence_required: Vec<String>,
    evidence_have: Vec<String>,
}

impl PlanIssueRaw {
    fn of(id: &str, meta: &Value) -> Self {
        PlanIssueRaw {
            id: id.to_string(),
            milestone: field(meta, "milestone"),
            title: clean(&field(meta, "title")),
            slug: field(meta, "slug"),
            priority: field(meta, "priority"),
            profile: field(meta, "profile"),
            parallel_safe: field(meta, "parallel_safe"),
            cancelled: field(meta, "cancelled") == "true",
            started_at: field(meta, "started_at"),
            verified_at: field(meta, "verified_at"),
            completed_at: field(meta, "completed_at"),
            objective: field(meta, "objective"),
            depends_on: list(meta, "depends_on"),
            scope: list(meta, "scope"),
            acceptance_criteria: list(meta, "acceptance_criteria").len(),
            validation: list(meta, "validation").len(),
            evidence_required: list(meta, "evidence_required"),
            evidence_have: evidence_covers(meta),
        }
    }
}

impl PlanMilestoneRaw {
    fn of(id: &str, meta: &Value) -> Self {
        PlanMilestoneRaw {
            id: id.to_string(),
            title: clean(&field(meta, "title")),
            slug: field(meta, "slug"),
            order: field(meta, "order"),
            priority: field(meta, "priority"),
            version: field(meta, "version"),
            cancelled: field(meta, "cancelled") == "true",
            superseded_by: field(meta, "superseded_by"),
            outcome: field(meta, "outcome"),
            depends_on: list(meta, "depends_on"),
            claims: list(meta, "claims"),
            evidence_required: list(meta, "evidence_required"),
            evidence_have: evidence_covers(meta),
        }
    }
}

// ---------------------------------------------------------------- the derived model

/// The plan's header: what this repository is, plus the one field nobody authors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanProject {
    /// The project's name, from `project.yaml`.
    pub name: String,
    /// The repository it belongs to, `owner/name`.
    pub repository: String,
    /// The branch the plan is executed on.
    pub default_branch: String,
    /// The milestone a worker is executing now: the lowest-ranked unblocked milestone that
    /// is ACTIVE, else the lowest-ranked unblocked one not finished. Derived on every read,
    /// stored nowhere, and never authored — the plan cannot nominate a milestone whose
    /// prerequisites are not real.
    pub active_milestone: String,
}

/// The status vocabularies, so a reader never has to know which statuses exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanVocabulary {
    /// The issue statuses, in derivation order.
    pub issue: Vec<String>,
    /// The milestone statuses, in derivation order.
    pub milestone: Vec<String>,
}

/// A milestone's issues counted: the two denominators, then one entry per declared status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanCounts {
    /// Every issue naming this milestone.
    pub total: u32,
    /// The denominator the milestone's own status derivation uses: total less cancelled, so
    /// no surface prints "n of total" for a milestone the engine calls DONE.
    pub required: u32,
    /// One entry per declared issue status, keyed by the vocabulary.
    pub by_status: BTreeMap<String, u32>,
}

/// One issue, as its record declares it and as the graph derives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanIssue {
    /// The identity, which is also the file name.
    pub id: String,
    /// The milestone it belongs to.
    pub milestone: String,
    /// The derived status. Never stored: an issue records what happened to it and the
    /// status follows from that and from the state of its dependencies.
    pub status: String,
    /// The execution wave: one past the longest path to it through the dependency graph.
    pub wave: u32,
    /// `p0` … `p3`.
    pub priority: String,
    /// The execution profile the issue is worked under.
    pub profile: String,
    /// Whether it may run beside another issue of its wave.
    pub parallel_safe: bool,
    /// One line naming the outcome.
    pub title: String,
    /// The slug, when the record carries one.
    pub slug: String,
    /// Every issue it declares a dependency on, as declared — including one that does not
    /// exist, which is a finding rather than a silent omission.
    pub depends_on: Vec<String>,
    /// The dependencies that are not DONE, plus `milestone:<id>` when the milestone gate
    /// holds the whole outcome back.
    pub blocked_by: Vec<String>,
    /// The issues that depend on this one.
    pub dependents: Vec<String>,
    /// The paths it touches; two issues of one wave that share a path are serialised.
    pub scope: Vec<String>,
    /// One line: what the issue is for.
    pub objective: String,
    /// Evidence entries attached.
    pub evidence_have: u32,
    /// Evidence tokens the record requires before it may be DONE.
    pub evidence_need: u32,
    /// When execution began, when it did.
    pub started_at: String,
    /// When implementation was declared complete.
    pub verified_at: String,
    /// When completion was recorded.
    pub completed_at: String,
}

/// One milestone, as its record declares it and as the two graphs derive it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanMilestone {
    /// The identity, which is also the file name; a stable slug, never a version.
    pub id: String,
    /// The derived status.
    pub status: String,
    /// Tie-break inside one rank; the roadmap is ordered by rank first.
    pub order: i64,
    /// `p0` … `p3`.
    pub priority: String,
    /// One line naming the outcome.
    pub title: String,
    /// The slug, when the record carries one.
    pub slug: String,
    /// The release the milestone belongs to, when it declares one.
    pub version: String,
    /// Its layer in the milestone graph: what orders the roadmap.
    pub rank: u32,
    /// The milestones it requires.
    pub depends_on: Vec<String>,
    /// Those of them that are not DONE. Non-empty means the gate holds every issue of this
    /// milestone back, whatever the issue graph says.
    pub blocked_by: Vec<String>,
    /// The milestones that require it.
    pub dependents: Vec<String>,
    /// The claims of the repository this outcome makes true.
    pub claims: Vec<String>,
    /// Its issues, counted by derived status.
    pub counts: PlanCounts,
    /// Its issues, in id order.
    pub issues: Vec<String>,
    /// One line: what is true once the milestone is real.
    pub outcome: String,
}

/// One execution wave: the issues the graph allows to run at the same time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanWave {
    /// The layer, from zero.
    pub wave: u32,
    /// The issues in it, in id order.
    pub issues: Vec<String>,
}

/// One dependency edge, `from` before `to`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct PlanEdge {
    /// The record that must be real first.
    pub from: String,
    /// The record that waits for it.
    pub to: String,
}

/// One validation finding, in the shape `project.finding-carries-reproduce` asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanFinding {
    /// `FAIL` or `WARN`. A failure means the model is invalid.
    pub level: String,
    /// The stable code a reader greps for (`unknown_dependency`, `scope_conflict`, …).
    pub code: String,
    /// The record the finding is about, or `graph` when it is about the whole graph.
    pub subject: String,
    /// What is wrong, in one line.
    pub message: String,
}

/// The whole derived plan: what every plan capability answers out of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Plan {
    /// The plan's header, with the active milestone derived.
    pub project: PlanProject,
    /// The declared status vocabularies.
    pub statuses: PlanVocabulary,
    /// Every milestone, in id order.
    pub milestones: Vec<PlanMilestone>,
    /// Every issue, in id order.
    pub issues: Vec<PlanIssue>,
    /// The execution waves, lowest first.
    pub waves: Vec<PlanWave>,
    /// The issue dependency graph, sorted.
    pub edges: Vec<PlanEdge>,
    /// The milestone dependency graph, sorted.
    pub milestone_edges: Vec<PlanEdge>,
    /// Every finding, in derivation order.
    pub findings: Vec<PlanFinding>,
}

impl Plan {
    /// One issue by id.
    pub fn issue(&self, id: &str) -> Option<&PlanIssue> {
        self.issues.iter().find(|i| i.id == id)
    }

    /// One milestone by id.
    pub fn milestone(&self, id: &str) -> Option<&PlanMilestone> {
        self.milestones.iter().find(|m| m.id == id)
    }

    /// How many findings are failures. A model with one is invalid.
    pub fn failures(&self) -> usize {
        self.findings.iter().filter(|f| f.level == "FAIL").count()
    }

    /// How many findings are warnings.
    pub fn warnings(&self) -> usize {
        self.findings.iter().filter(|f| f.level == "WARN").count()
    }

    /// Whether this repository holds a canonical project model at all.
    pub fn is_present(&self) -> bool {
        !self.milestones.is_empty() || !self.issues.is_empty()
    }

    /// The milestone ids in roadmap order: rank, then order, then id. Derived from the
    /// graph; no list of versions is maintained anywhere.
    pub fn roadmap(&self) -> Vec<&PlanMilestone> {
        let mut out: Vec<&PlanMilestone> = self.milestones.iter().collect();
        out.sort_by(|a, b| {
            (a.rank, a.order, &a.id)
                .partial_cmp(&(b.rank, b.order, &b.id))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    /// The one issue a worker should take now: the lowest-wave READY issue of the active
    /// milestone, highest priority first, then id.
    ///
    /// The active milestone can have nothing ready while another one does — a milestone
    /// waiting on its own acceptance evidence, for instance — so the search widens to the
    /// whole plan rather than answering "none" and sending a worker away from executable
    /// work. `mj_pj_next_ready` in `lib/project.sh` does exactly this.
    pub fn next_ready(&self, milestone: Option<&str>) -> Option<&PlanIssue> {
        let scope = milestone.unwrap_or(self.project.active_milestone.as_str());
        self.ready_ranked(Some(scope))
            .or_else(|| self.ready_ranked(None))
    }

    fn ready_ranked(&self, milestone: Option<&str>) -> Option<&PlanIssue> {
        self.issues
            .iter()
            .filter(|i| i.status == "READY")
            .filter(|i| milestone.is_none_or(|m| m.is_empty() || i.milestone == m))
            .min_by_key(|i| (i.wave, priority_rank(&i.priority), i.id.clone()))
    }

    /// Build the plan from an index. A walk of values already in memory: no file is opened,
    /// no subprocess is started, nothing is cached.
    pub fn build(index: &Index) -> Plan {
        let (header, milestones, issues) = raws(index);
        derive(header, milestones, issues)
    }

    /// The plan as it will read once `transition` has been written to `id`.
    ///
    /// The same records, the same [`derive`], one timestamp changed: the status a caller is
    /// told after a write is computed by the one derivation every reader uses, never
    /// announced by the writer. This matters because the resulting status is not a function
    /// of the transition alone — a `done` whose required evidence is absent stays VERIFY,
    /// and a record marked `cancelled` stays CANCELLED whatever is written to it.
    ///
    /// Nothing is read from disk and nothing is written: this is the projection of a write,
    /// and [`transition`] is what performs one.
    pub fn after(index: &Index, id: &str, transition: Transition, now: &str) -> Plan {
        let (header, milestones, mut issues) = raws(index);
        if let Some(r) = issues.iter_mut().find(|r| r.id == id) {
            match transition {
                Transition::Start => r.started_at = now.to_string(),
                Transition::Verify => r.verified_at = now.to_string(),
                Transition::Done => r.completed_at = now.to_string(),
            }
        }
        derive(header, milestones, issues)
    }
}

/// Every record the plan is derived from, as its file declares it.
///
/// Split out of [`Plan::build`] so that [`Plan::after`] derives from the same collection
/// rather than a second reading of the index.
fn raws(index: &Index) -> (PlanProject, Vec<PlanMilestoneRaw>, Vec<PlanIssueRaw>) {
    let mut milestones: Vec<PlanMilestoneRaw> = Vec::new();
    let mut issues: Vec<PlanIssueRaw> = Vec::new();
    let mut header = PlanProject {
        name: String::new(),
        repository: String::new(),
        default_branch: String::new(),
        active_milestone: String::new(),
    };
    for o in &index.objects {
        match o.kind.as_str() {
            MILESTONE => milestones.push(PlanMilestoneRaw::of(&o.identity, &o.metadata)),
            ISSUE => issues.push(PlanIssueRaw::of(&o.identity, &o.metadata)),
            PROJECT if o.provenance.path.ends_with("project/project.yaml") => {
                header.name = field(&o.metadata, "name");
                header.repository = field(&o.metadata, "repository");
                header.default_branch = field(&o.metadata, "default_branch");
            }
            _ => {}
        }
    }
    // The awk reads the files in `LC_ALL=C sort` order of their names, and the loader
    // refuses a record whose id disagrees with its file name, so id order is file order.
    milestones.sort_by(|a, b| a.id.cmp(&b.id));
    issues.sort_by(|a, b| a.id.cmp(&b.id));
    (header, milestones, issues)
}

fn priority_rank(p: &str) -> u8 {
    match p {
        "p0" => 0,
        "p1" => 1,
        "p2" => 2,
        _ => 3,
    }
}

// ---------------------------------------------------------------- the derivation

struct PlanFindings(Vec<PlanFinding>);

impl PlanFindings {
    fn fail(&mut self, code: &str, subject: &str, message: String) {
        self.0.push(PlanFinding {
            level: "FAIL".into(),
            code: code.into(),
            subject: subject.into(),
            message,
        });
    }
    fn warn(&mut self, code: &str, subject: &str, message: String) {
        self.0.push(PlanFinding {
            level: "WARN".into(),
            code: code.into(),
            subject: subject.into(),
            message,
        });
    }
}

/// Does path `a` contain path `b`, or the reverse? Trailing slashes and `./` are normalised
/// so that a hand-written scope entry cannot silently miss an overlap. `overlap()` in
/// `lib/project.awk`.
///
/// ```
/// use majordomus_cli::plan::overlap;
/// assert!(overlap("apps/cli", "apps/cli/src/main.rs"));
/// assert!(overlap("./lib/", "lib"));
/// assert!(!overlap("lib", "libexec"));
/// ```
pub fn overlap(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.strip_prefix("./").unwrap_or(s);
        s.trim_end_matches('/').to_string()
    };
    let (a, b) = (norm(a), norm(b));
    a == b || b.starts_with(&format!("{a}/")) || a.starts_with(&format!("{b}/"))
}

#[allow(clippy::too_many_lines)] // one derivation, in the order lib/project.awk derives it
fn derive(mut header: PlanProject, mraw: Vec<PlanMilestoneRaw>, iraw: Vec<PlanIssueRaw>) -> Plan {
    let mut v = PlanFindings(Vec::new());
    let iids: Vec<String> = iraw.iter().map(|i| i.id.clone()).collect();
    let mids: Vec<String> = mraw.iter().map(|m| m.id.clone()).collect();
    let iseen: BTreeSet<&str> = iids.iter().map(String::as_str).collect();
    let mseen: BTreeSet<&str> = mids.iter().map(String::as_str).collect();
    let ix = |id: &str| -> Option<usize> { iraw.iter().position(|r| r.id == id) };

    // --- local facts: is this issue DONE on its own terms?
    let mut covered = vec![true; iraw.len()];
    let mut done = vec![false; iraw.len()];
    for (n, r) in iraw.iter().enumerate() {
        covered[n] = r
            .evidence_required
            .iter()
            .all(|need| r.evidence_have.contains(need));
        done[n] = !r.cancelled && !r.completed_at.is_empty() && covered[n];
    }

    // --- edges, and the errors visible in them
    let mut edge: BTreeSet<(String, String)> = BTreeSet::new();
    let mut dependents: Vec<Vec<String>> = vec![Vec::new(); iraw.len()];
    let mut edges: Vec<PlanEdge> = Vec::new();
    for (n, r) in iraw.iter().enumerate() {
        let mut seen_here: BTreeSet<&str> = BTreeSet::new();
        for d in &r.depends_on {
            if d == &r.id {
                v.fail("self_dependency", &r.id, "depends on itself".into());
                continue;
            }
            if !iseen.contains(d.as_str()) {
                v.fail(
                    "unknown_dependency",
                    &r.id,
                    format!("depends on {d}, which is not an issue"),
                );
                continue;
            }
            if !seen_here.insert(d.as_str()) {
                v.warn(
                    "duplicate_dependency",
                    &r.id,
                    format!("names {d} more than once"),
                );
                continue;
            }
            edge.insert((r.id.clone(), d.clone()));
            if let Some(dn) = ix(d) {
                dependents[dn].push(r.id.clone());
            }
            edges.push(PlanEdge {
                from: d.clone(),
                to: r.id.clone(),
            });
        }
        if r.milestone.is_empty() {
            v.fail("no_milestone", &r.id, "names no milestone".into());
        } else if !mseen.contains(r.milestone.as_str()) {
            v.fail(
                "unknown_milestone",
                &r.id,
                format!("names milestone {}, which does not exist", r.milestone),
            );
        }
        if r.acceptance_criteria == 0 && !r.cancelled {
            v.fail(
                "no_acceptance",
                &r.id,
                "has no acceptance criteria; an issue without one is a placeholder".into(),
            );
        }
        if r.validation == 0 && !r.cancelled {
            v.fail("no_validation", &r.id, "names no validation command".into());
        }
        if r.evidence_required.is_empty() && !r.cancelled {
            v.warn(
                "no_evidence_required",
                &r.id,
                "requires no evidence; nothing gates its completion".into(),
            );
        }
        if !r.completed_at.is_empty() && !covered[n] && !r.cancelled {
            let miss: Vec<&str> = r
                .evidence_required
                .iter()
                .filter(|need| !r.evidence_have.contains(need))
                .map(String::as_str)
                .collect();
            v.warn(
                "evidence_missing",
                &r.id,
                format!(
                    "completed_at is set but evidence is missing for {}; status stays VERIFY",
                    miss.join(",")
                ),
            );
        }
    }

    // --- waves: Kahn layering. A node enters a wave only once every dependency has left,
    //     so its wave is one past the longest path to it. Nodes that never leave are in or
    //     downstream of a cycle, and are reported rather than silently given a wave.
    let mut left = vec![true; iraw.len()];
    let mut nleft = iraw.len();
    let mut wave: Vec<Option<u32>> = vec![None; iraw.len()];
    let mut wavelines: Vec<PlanWave> = Vec::new();
    let mut layer: u32 = 0;
    while nleft > 0 {
        let mut front: Vec<usize> = Vec::new();
        for (n, r) in iraw.iter().enumerate() {
            if !left[n] {
                continue;
            }
            let blocked = r.depends_on.iter().any(|d| {
                d != &r.id && iseen.contains(d.as_str()) && ix(d).is_some_and(|dn| left[dn])
            });
            if !blocked {
                front.push(n);
            }
        }
        if front.is_empty() {
            let stuck: Vec<&str> = (0..iraw.len())
                .filter(|n| left[*n])
                .map(|n| iids[n].as_str())
                .collect();
            v.fail(
                "cycle",
                "graph",
                format!(
                    "a dependency cycle prevents these issues from ever becoming ready: {}",
                    stuck.join(" ")
                ),
            );
            break;
        }
        let mut line: Vec<String> = Vec::new();
        for n in front {
            wave[n] = Some(layer);
            left[n] = false;
            nleft -= 1;
            line.push(iids[n].clone());
        }
        wavelines.push(PlanWave {
            wave: layer,
            issues: line,
        });
        layer += 1;
    }

    // --- issue status
    let mut status: Vec<String> = vec![String::new(); iraw.len()];
    let mut blocked_by: Vec<Vec<String>> = vec![Vec::new(); iraw.len()];
    for (n, r) in iraw.iter().enumerate() {
        // Every declared dependency, not every edge: a duplicate is a warning, and the awk
        // walks the declaration here, so a record naming one twice blocks on it twice.
        let bb: Vec<String> = r
            .depends_on
            .iter()
            .filter(|d| {
                *d != &r.id && iseen.contains(d.as_str()) && ix(d).is_some_and(|dn| !done[dn])
            })
            .cloned()
            .collect();
        let st = if r.cancelled {
            "CANCELLED"
        } else if done[n] {
            "DONE"
        } else if !r.completed_at.is_empty() || !r.verified_at.is_empty() {
            "VERIFY"
        } else if !r.started_at.is_empty() {
            "ACTIVE"
        } else if !bb.is_empty() {
            "BLOCKED"
        } else {
            "READY"
        };
        if !bb.is_empty() && matches!(st, "DONE" | "ACTIVE" | "VERIFY") {
            v.fail(
                "premature_execution",
                &r.id,
                format!("is {st} while {} is not DONE", bb.join(",")),
            );
        }
        status[n] = st.to_string();
        blocked_by[n] = bb;
        if !left[n] && wave[n].is_none() && st != "CANCELLED" {
            wave[n] = Some(0);
        }
    }

    // --- the milestone graph: validated before anything derives from it, under the same
    //     rules the issue graph lives under, one level up.
    let mut medge: BTreeSet<(String, String)> = BTreeSet::new();
    let mut mdependents: Vec<Vec<String>> = vec![Vec::new(); mraw.len()];
    let mut medges: Vec<PlanEdge> = Vec::new();
    let mx = |id: &str| -> Option<usize> { mraw.iter().position(|r| r.id == id) };
    for r in &mraw {
        for d in &r.depends_on {
            if d == &r.id {
                v.fail("milestone_self_dependency", &r.id, "requires itself".into());
                continue;
            }
            if !mseen.contains(d.as_str()) {
                v.fail(
                    "milestone_unknown_dependency",
                    &r.id,
                    format!("requires {d}, which is not a milestone here"),
                );
                continue;
            }
            if medge.contains(&(r.id.clone(), d.clone())) {
                v.warn(
                    "milestone_duplicate_dependency",
                    &r.id,
                    format!("requires {d} more than once"),
                );
                continue;
            }
            medge.insert((r.id.clone(), d.clone()));
            if let Some(dn) = mx(d) {
                mdependents[dn].push(r.id.clone());
            }
            medges.push(PlanEdge {
                from: d.clone(),
                to: r.id.clone(),
            });
        }
    }

    // --- roadmap rank: Kahn layering over the milestone graph. Rank is what orders the
    //     roadmap; `order` only breaks ties inside one rank.
    let mut mleft = vec![true; mraw.len()];
    let mut mnleft = mraw.len();
    let mut rank: Vec<u32> = vec![0; mraw.len()];
    let mut mlayer: u32 = 0;
    while mnleft > 0 {
        let mut front: Vec<usize> = Vec::new();
        for (n, r) in mraw.iter().enumerate() {
            if !mleft[n] {
                continue;
            }
            let blocked = r.depends_on.iter().any(|d| {
                medge.contains(&(r.id.clone(), d.clone())) && mx(d).is_some_and(|dn| mleft[dn])
            });
            if !blocked {
                front.push(n);
            }
        }
        if front.is_empty() {
            break;
        }
        for n in front {
            rank[n] = mlayer;
            mleft[n] = false;
            mnleft -= 1;
        }
        mlayer += 1;
    }
    if mnleft > 0 {
        let cyc: Vec<&str> = (0..mraw.len())
            .filter(|n| mleft[*n])
            .map(|n| mids[n].as_str())
            .collect();
        v.fail(
            "milestone_cycle",
            &cyc.join(" "),
            "milestone dependencies form a cycle; no roadmap order exists".into(),
        );
        for n in 0..mraw.len() {
            if mleft[n] {
                rank[n] = 0;
            }
        }
    }

    // --- milestone status
    let mut mstatus: Vec<String> = vec![String::new(); mraw.len()];
    let mut counts: Vec<PlanCounts> = Vec::with_capacity(mraw.len());
    let mut mempty = vec![false; mraw.len()];
    for (n, r) in mraw.iter().enumerate() {
        let c = tally(&iraw, &status, &r.id);
        let (nd, nr, na, nv) = (
            *c.by_status.get("DONE").unwrap_or(&0),
            *c.by_status.get("READY").unwrap_or(&0),
            *c.by_status.get("ACTIVE").unwrap_or(&0),
            *c.by_status.get("VERIFY").unwrap_or(&0),
        );
        let mcov = r
            .evidence_required
            .iter()
            .all(|need| r.evidence_have.contains(need));
        let (tot, req) = (c.total, c.required);
        let ms = if r.cancelled {
            "CANCELLED"
        } else if !r.superseded_by.is_empty() {
            "SUPERSEDED"
        } else if tot == 0 && !r.evidence_required.is_empty() && mcov {
            "DONE"
        } else if tot == 0 {
            "PLANNED"
        } else if req > 0 && nd == req && mcov {
            "DONE"
        } else if req > 0 && nd == req {
            "VERIFY"
        } else if na > 0 || nv > 0 || nd > 0 {
            "ACTIVE"
        } else if nr == 0 {
            "BLOCKED"
        } else {
            "PLANNED"
        };
        mstatus[n] = ms.to_string();
        mempty[n] = tot == 0;
        if req > 0 && nd == req && !mcov {
            let miss: Vec<&str> = r
                .evidence_required
                .iter()
                .filter(|need| !r.evidence_have.contains(need))
                .map(String::as_str)
                .collect();
            v.warn(
                "milestone_evidence_missing",
                &r.id,
                format!(
                    "every issue is DONE but milestone evidence is missing for {}; status stays VERIFY",
                    miss.join(",")
                ),
            );
        }
        counts.push(c);
    }

    // --- the gate. A milestone whose required outcomes are not yet real cannot be
    //     executable, whatever its own issues say. Run in rank order, so a dependency is
    //     always decided before the milestone requiring it and the block cascades.
    let mut mblocked: Vec<Vec<String>> = vec![Vec::new(); mraw.len()];
    for r in 0..=mlayer {
        for (n, m) in mraw.iter().enumerate() {
            if rank[n] != r {
                continue;
            }
            let block: Vec<String> = m
                .depends_on
                .iter()
                .filter(|d| medge.contains(&(m.id.clone(), (*d).clone())))
                .filter(|d| mx(d).is_some_and(|dn| mstatus[dn] != "DONE"))
                .cloned()
                .collect();
            mblocked[n] = block.clone();
            if block.is_empty() {
                continue;
            }
            if mstatus[n] == "CANCELLED" || mstatus[n] == "SUPERSEDED" {
                continue;
            }
            if matches!(mstatus[n].as_str(), "DONE" | "VERIFY" | "ACTIVE") {
                v.fail(
                    "milestone_premature",
                    &m.id,
                    format!("is {} while {} is not DONE", mstatus[n], block.join(",")),
                );
            }
            mstatus[n] = "BLOCKED".into();
        }
    }

    for (n, m) in mraw.iter().enumerate() {
        if !mempty[n] {
            continue;
        }
        if matches!(mstatus[n].as_str(), "DONE" | "CANCELLED" | "SUPERSEDED") {
            continue;
        }
        if !mblocked[n].is_empty() {
            continue;
        }
        v.warn(
            "empty_milestone",
            &m.id,
            "is reachable and has no issues; it is an outcome nobody is executing".into(),
        );
    }

    // --- the gate reaches the work. An issue inside a milestone the gate has blocked is not
    //     executable, whatever its own dependencies say: the outcome it belongs to is not
    //     reachable yet.
    for (n, r) in iraw.iter().enumerate() {
        if r.milestone.is_empty() {
            continue;
        }
        let Some(mn) = mx(&r.milestone) else { continue };
        if mblocked[mn].is_empty() {
            continue;
        }
        if status[n] == "CANCELLED" || status[n] == "DONE" {
            continue;
        }
        if status[n] == "ACTIVE" || status[n] == "VERIFY" {
            v.fail(
                "premature_execution",
                &r.id,
                format!(
                    "is {} while its milestone {} waits on {}",
                    status[n],
                    r.milestone,
                    mblocked[mn].join(",")
                ),
            );
        }
        status[n] = "BLOCKED".into();
        blocked_by[n].push(format!("milestone:{}", r.milestone));
    }

    // counts follow the statuses, so a blocked milestone cannot report ready work
    for (n, m) in mraw.iter().enumerate() {
        if mblocked[n].is_empty() {
            continue;
        }
        counts[n] = tally(&iraw, &status, &m.id);
    }

    // --- scope conflict: two issues that could run together but touch the same paths.
    //     Reported conservatively — an uncertain overlap serialises rather than races.
    for a in 0..iraw.len() {
        for b in (a + 1)..iraw.len() {
            if !matches!(status[a].as_str(), "READY" | "ACTIVE") {
                continue;
            }
            if !matches!(status[b].as_str(), "READY" | "ACTIVE") {
                continue;
            }
            if wave[a] != wave[b] {
                continue;
            }
            'pair: for x in &iraw[a].scope {
                for y in &iraw[b].scope {
                    if overlap(x, y) {
                        v.warn(
                            "scope_conflict",
                            &iraw[a].id,
                            format!(
                                "shares {x} with {}; they may not run concurrently",
                                iraw[b].id
                            ),
                        );
                        break 'pair;
                    }
                }
            }
        }
    }

    // --- the active milestone: the lowest-ranked unblocked milestone that is ACTIVE, else
    //     the lowest-ranked unblocked one not finished.
    let mut best: Option<usize> = None;
    for pass in 1..=2 {
        for n in 0..mraw.len() {
            if mstatus[n] == "DONE" || mstatus[n] == "CANCELLED" {
                continue;
            }
            if pass == 1 && mstatus[n] != "ACTIVE" {
                continue;
            }
            if !mblocked[n].is_empty() {
                continue;
            }
            let o = i64::from(rank[n]) * 1_000_000 + order_of(&mraw[n].order);
            if best.is_none_or(|b| o < i64::from(rank[b]) * 1_000_000 + order_of(&mraw[b].order)) {
                best = Some(n);
            }
        }
        if best.is_some() {
            break;
        }
    }
    header.active_milestone = best.map(|n| mids[n].clone()).unwrap_or_default();

    // --- reconciliation: a status assigned but not declared would reach a surface as a
    //     count nobody renders and a label nobody has a colour for.
    for (n, r) in iraw.iter().enumerate() {
        if !ISSUE_STATUSES.contains(&status[n].as_str()) {
            v.fail(
                "undeclared_status",
                &r.id,
                format!(
                    "is {}, which the issue status vocabulary does not declare",
                    status[n]
                ),
            );
        }
    }
    for (n, m) in mraw.iter().enumerate() {
        if !MILESTONE_STATUSES.contains(&mstatus[n].as_str()) {
            v.fail(
                "undeclared_status",
                &m.id,
                format!(
                    "is {}, which the milestone status vocabulary does not declare",
                    mstatus[n]
                ),
            );
        }
    }

    // ---------------------------------------------------------------- assemble
    edges.sort();
    edges.dedup();
    medges.sort();
    medges.dedup();

    let issues: Vec<PlanIssue> = iraw
        .iter()
        .enumerate()
        .map(|(n, r)| PlanIssue {
            id: r.id.clone(),
            milestone: r.milestone.clone(),
            status: status[n].clone(),
            wave: wave[n].unwrap_or(0),
            priority: r.priority.clone(),
            profile: r.profile.clone(),
            parallel_safe: r.parallel_safe != "false",
            title: r.title.clone(),
            slug: r.slug.clone(),
            depends_on: r.depends_on.clone(),
            blocked_by: blocked_by[n].clone(),
            dependents: dependents[n].clone(),
            scope: r.scope.clone(),
            objective: r.objective.clone(),
            evidence_have: u32::try_from(r.evidence_have.len()).unwrap_or(u32::MAX),
            evidence_need: u32::try_from(r.evidence_required.len()).unwrap_or(u32::MAX),
            started_at: r.started_at.clone(),
            verified_at: r.verified_at.clone(),
            completed_at: r.completed_at.clone(),
        })
        .collect();

    let milestones: Vec<PlanMilestone> = mraw
        .iter()
        .enumerate()
        .map(|(n, r)| PlanMilestone {
            id: r.id.clone(),
            status: mstatus[n].clone(),
            order: order_of(&r.order),
            priority: r.priority.clone(),
            title: r.title.clone(),
            slug: r.slug.clone(),
            version: r.version.clone(),
            rank: rank[n],
            depends_on: r
                .depends_on
                .iter()
                .filter(|d| medge.contains(&(r.id.clone(), (*d).clone())))
                .cloned()
                .collect(),
            blocked_by: mblocked[n].clone(),
            dependents: mdependents[n].clone(),
            claims: r.claims.clone(),
            counts: counts[n].clone(),
            issues: iraw
                .iter()
                .filter(|i| i.milestone == r.id)
                .map(|i| i.id.clone())
                .collect(),
            outcome: r.outcome.clone(),
        })
        .collect();

    Plan {
        project: header,
        statuses: PlanVocabulary {
            issue: ISSUE_STATUSES.iter().map(|s| (*s).to_string()).collect(),
            milestone: MILESTONE_STATUSES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        },
        milestones,
        issues,
        waves: wavelines,
        edges,
        milestone_edges: medges,
        findings: v.0,
    }
}

/// `order` as a number: the awk compares it with `+ 0`, which makes anything unparseable
/// zero. A milestone with no `order` is not an error, it is order zero.
fn order_of(raw: &str) -> i64 {
    raw.trim().parse::<i64>().unwrap_or(0)
}

/// The counts of one milestone: the two denominators, then one entry per declared status.
/// `mcounts()` in `lib/project.awk`, and computed the same twice — once from the statuses
/// before the gate and once after it, because the gate changes statuses underneath it.
fn tally(iraw: &[PlanIssueRaw], status: &[String], milestone: &str) -> PlanCounts {
    let mut by: BTreeMap<String, u32> = ISSUE_STATUSES
        .iter()
        .map(|s| ((*s).to_string(), 0))
        .collect();
    let mut total = 0u32;
    for (n, r) in iraw.iter().enumerate() {
        if r.milestone != milestone {
            continue;
        }
        total += 1;
        *by.entry(status[n].clone()).or_insert(0) += 1;
    }
    let cancelled = *by.get("CANCELLED").unwrap_or(&0);
    PlanCounts {
        total,
        required: total - cancelled,
        by_status: by,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn issue(id: &str, milestone: &str, deps: &[&str]) -> PlanIssueRaw {
        PlanIssueRaw::of(
            id,
            &json!({
                "id": id,
                "milestone": milestone,
                "title": format!("PlanIssue {id}"),
                "priority": "p1",
                "profile": "implementation",
                "depends_on": deps,
                "acceptance_criteria": ["it works"],
                "validation": ["true"],
                "evidence_required": ["proof"],
            }),
        )
    }

    fn milestone(id: &str, deps: &[&str]) -> PlanMilestoneRaw {
        PlanMilestoneRaw::of(
            id,
            &json!({
                "id": id,
                "title": format!("PlanMilestone {id}"),
                "order": 0,
                "priority": "p1",
                "depends_on": deps,
                "evidence_required": ["proof"],
            }),
        )
    }

    fn header() -> PlanProject {
        PlanProject {
            name: "Fixture".into(),
            repository: "example/fixture".into(),
            default_branch: "master".into(),
            active_milestone: String::new(),
        }
    }

    /// A diamond places each issue one layer past its deepest dependency, and the tip waits
    /// on both middles. The same graph `test/cases/42_dag_waves.sh` asserts on in the shell.
    #[test]
    fn a_diamond_layers_into_three_waves() {
        let p = derive(
            header(),
            vec![milestone("M000", &[])],
            vec![
                issue("I0001", "M000", &[]),
                issue("I0002", "M000", &["I0001"]),
                issue("I0003", "M000", &["I0001"]),
                issue("I0004", "M000", &["I0002", "I0003"]),
            ],
        );
        assert_eq!(p.issue("I0001").unwrap().wave, 0);
        assert_eq!(p.issue("I0002").unwrap().wave, 1);
        assert_eq!(p.issue("I0003").unwrap().wave, 1);
        assert_eq!(p.issue("I0004").unwrap().wave, 2);
        assert_eq!(p.issue("I0001").unwrap().status, "READY");
        assert_eq!(p.issue("I0004").unwrap().status, "BLOCKED");
        assert_eq!(
            p.issue("I0004").unwrap().blocked_by,
            vec!["I0002".to_string(), "I0003".to_string()]
        );
        assert_eq!(p.issue("I0001").unwrap().dependents.len(), 2);
        assert_eq!(p.waves.len(), 3);
        assert_eq!(p.next_ready(None).unwrap().id, "I0001");
        assert_eq!(p.failures(), 0);
    }

    /// Status is derived from what happened to a record, never stored, and evidence is what
    /// separates VERIFY from DONE.
    #[test]
    fn completion_without_evidence_stays_in_verify() {
        let mut i = issue("I0001", "M000", &[]);
        i.completed_at = "2026-01-01T00:00:00Z".into();
        let p = derive(header(), vec![milestone("M000", &[])], vec![i.clone()]);
        assert_eq!(p.issue("I0001").unwrap().status, "VERIFY");
        assert!(p
            .findings
            .iter()
            .any(|f| f.code == "evidence_missing" && f.subject == "I0001"));

        i.evidence_have = vec!["proof".into()];
        let p = derive(header(), vec![milestone("M000", &[])], vec![i]);
        assert_eq!(p.issue("I0001").unwrap().status, "DONE");
        assert_eq!(p.milestone("M000").unwrap().status, "VERIFY");
    }

    /// A cycle is reported by name and nobody in it is given a wave, rather than the whole
    /// graph quietly producing an empty ready set.
    #[test]
    fn a_cycle_is_a_named_failure() {
        let p = derive(
            header(),
            vec![milestone("M000", &[])],
            vec![
                issue("I0001", "M000", &["I0002"]),
                issue("I0002", "M000", &["I0001"]),
            ],
        );
        let f = p.findings.iter().find(|f| f.code == "cycle").unwrap();
        assert_eq!(f.level, "FAIL");
        assert_eq!(f.subject, "graph");
        assert!(f.message.contains("I0001"));
        assert!(f.message.contains("I0002"));
        assert!(p.waves.is_empty());
    }

    /// The milestone gate reaches the work: an issue whose own dependencies are all done is
    /// still not executable inside a milestone whose prerequisites are not real.
    #[test]
    fn the_milestone_gate_blocks_the_issues_underneath_it() {
        let p = derive(
            header(),
            vec![milestone("M000", &[]), milestone("M001", &["M000"])],
            vec![issue("I0001", "M000", &[]), issue("I0002", "M001", &[])],
        );
        assert_eq!(p.milestone("M001").unwrap().status, "BLOCKED");
        assert_eq!(p.milestone("M001").unwrap().blocked_by, vec!["M000"]);
        assert_eq!(p.issue("I0002").unwrap().status, "BLOCKED");
        // the marker names the milestone whose gate holds the issue — its own — not the
        // milestone that milestone is waiting on; `plan.milestone(...).blocked_by` says that
        assert_eq!(p.issue("I0002").unwrap().blocked_by, vec!["milestone:M001"]);
        // and the blocked milestone's counts follow the statuses the gate assigned
        assert_eq!(
            *p.milestone("M001")
                .unwrap()
                .counts
                .by_status
                .get("BLOCKED")
                .unwrap(),
            1
        );
        assert_eq!(
            *p.milestone("M001")
                .unwrap()
                .counts
                .by_status
                .get("READY")
                .unwrap(),
            0
        );
        // the roadmap ranks the gated milestone after the one it waits on
        assert_eq!(p.milestone("M000").unwrap().rank, 0);
        assert_eq!(p.milestone("M001").unwrap().rank, 1);
        assert_eq!(p.project.active_milestone, "M000");
        assert_eq!(p.next_ready(None).unwrap().id, "I0001");
    }

    /// Two issues of one wave that touch the same paths are serialised, and said so.
    #[test]
    fn overlapping_scope_in_one_wave_is_a_warning() {
        let mut a = issue("I0001", "M000", &[]);
        let mut b = issue("I0002", "M000", &[]);
        a.scope = vec!["apps/cli".into()];
        b.scope = vec!["apps/cli/src/main.rs".into()];
        let p = derive(header(), vec![milestone("M000", &[])], vec![a, b]);
        let f = p
            .findings
            .iter()
            .find(|f| f.code == "scope_conflict")
            .unwrap();
        assert_eq!(f.level, "WARN");
        assert_eq!(f.subject, "I0001");
        assert!(f.message.contains("I0002"));
    }

    /// A record naming a dependency that does not exist is refused by name, and the
    /// declaration is still reported so a reader sees what the file says.
    #[test]
    fn an_unknown_dependency_is_refused_by_name() {
        let p = derive(
            header(),
            vec![milestone("M000", &[])],
            vec![issue("I0001", "M000", &["I9999"])],
        );
        let f = p
            .findings
            .iter()
            .find(|f| f.code == "unknown_dependency")
            .unwrap();
        assert_eq!(f.level, "FAIL");
        assert_eq!(f.message, "depends on I9999, which is not an issue");
        assert_eq!(p.issue("I0001").unwrap().depends_on, vec!["I9999"]);
        assert!(p.issue("I0001").unwrap().blocked_by.is_empty());
    }

    /// The next issue is the lowest wave, then the highest priority, then the id — and the
    /// search widens past the active milestone rather than answering "none".
    #[test]
    fn the_next_issue_is_ranked_not_picked() {
        let mut low = issue("I0002", "M000", &[]);
        low.priority = "p0".into();
        let p = derive(
            header(),
            vec![milestone("M000", &[])],
            vec![issue("I0001", "M000", &[]), low],
        );
        assert_eq!(p.next_ready(None).unwrap().id, "I0002");
        assert_eq!(p.next_ready(Some("M999")).unwrap().id, "I0002");
    }
}

// ---------------------------------------------------------------- transitions
//
// The write half of the plan, and the first mutating capability of the development
// lifecycle. Everything above this line derives; everything below it changes one field of
// one record and records that it did.
//
// `lib/plan.sh`'s `mj_plan_transition` has been the only implementation of these three
// moves. This is the second, and the contract it is held to is byte equality of the file it
// writes — `test/cases/133_plan_transition.sh` performs the same transition through both
// engines over the same fixture and diffs the record. The guards are stated here in the
// same order the shell states them, because the *first* refusal a caller meets is part of
// the behaviour: an issue that is both blocked and missing its evidence must be told about
// the blocker, which is the thing it can act on.

/// One lifecycle move.
///
/// Three, not four: `evidence` appends a record rather than moving an issue, and it belongs
/// with the record it proves rather than with the moves it unblocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Transition {
    /// Execution began. Refused unless the issue is READY.
    Start,
    /// Implementation is complete and the evidence is still outstanding.
    Verify,
    /// Complete. Refused while anything it waits on is unfinished, or while any evidence
    /// its own record requires is absent.
    Done,
}

impl Transition {
    /// The field this move stamps.
    pub fn field(self) -> &'static str {
        match self {
            Self::Start => "started_at",
            Self::Verify => "verified_at",
            Self::Done => "completed_at",
        }
    }

    /// The event `share/events.yaml` declares for it.
    pub fn event(self) -> &'static str {
        match self {
            Self::Start => "plan_start",
            Self::Verify => "plan_verify",
            Self::Done => "plan_done",
        }
    }
}

/// Why a transition did not happen.
///
/// Each variant carries what the caller needs to act, never only that it failed: which
/// status the issue is actually in, which dependency holds it, which evidence token is
/// missing. The messages are the shell's messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// No issue of that id.
    NoSuchIssue(String),
    /// The move is illegal from the status the issue is in.
    Refused(String),
    /// The record could not be rewritten.
    Write {
        /// The record.
        path: std::path::PathBuf,
        /// What the filesystem said.
        reason: String,
    },
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchIssue(id) => write!(f, "no issue '{id}'"),
            Self::Refused(m) => f.write_str(m),
            Self::Write { path, reason } => write!(f, "{}: {reason}", path.display()),
        }
    }
}

impl std::error::Error for TransitionError {}

/// Whether `transition` is legal for `issue` as the plan reads now, and why not when it is
/// not.
///
/// Separate from the write so that a surface can offer or withhold a control without
/// performing it: the Cockpit disables a button by asking this, and never by reimplementing
/// the rule.
pub fn check(
    index: &Index,
    plan: &Plan,
    id: &str,
    transition: Transition,
    now: &str,
) -> Result<(), TransitionError> {
    let issue = plan
        .issue(id)
        .ok_or_else(|| TransitionError::NoSuchIssue(id.to_string()))?;
    match transition {
        Transition::Start => {
            if issue.status != "READY" {
                let waiting = if issue.blocked_by.is_empty() {
                    String::new()
                } else {
                    format!(" (waiting on {})", issue.blocked_by.join(" "))
                };
                return Err(TransitionError::Refused(format!(
                    "{id} is {}, not READY{waiting}",
                    issue.status
                )));
            }
        }
        Transition::Verify => {
            if !matches!(issue.status.as_str(), "ACTIVE" | "VERIFY") {
                return Err(TransitionError::Refused(format!(
                    "{id} is {}; only an ACTIVE issue can move to VERIFY",
                    issue.status
                )));
            }
        }
        Transition::Done => {
            // The blocker first: an issue that is both blocked and short of evidence is
            // told about the blocker, because that is the one it cannot act on alone.
            if !issue.blocked_by.is_empty() {
                return Err(TransitionError::Refused(format!(
                    "{id} cannot be DONE while {} is not DONE",
                    issue.blocked_by.join(" ")
                )));
            }
            // Whether the evidence is complete is not a count — a record may carry two
            // entries covering one token and none covering another — and the rule for it
            // already exists inside `derive`. So rather than restate it, ask what the plan
            // would say: write the stamp in memory and see whether the issue reaches DONE.
            // An issue that stays VERIFY with `completed_at` set is exactly the record
            // `derive` reports as "completed_at is set but evidence is missing".
            let after = Plan::after(index, id, transition, now);
            if after.issue(id).is_some_and(|i| i.status != "DONE") {
                return Err(TransitionError::Refused(format!(
                    "{id} has evidence for {} of {} requirement(s) (run: majordomus plan evidence {id} --covers <token> ...)",
                    issue.evidence_have, issue.evidence_need
                )));
            }
        }
    }
    Ok(())
}

/// Perform one transition: check it, stamp the record, append the event.
///
/// The order is the shell's and is not an implementation detail. The guard runs against the
/// plan as it reads now; the record is rewritten; only then is the event appended. A ledger
/// line therefore never describes a change that did not reach the file, which is the
/// property that lets the record be read as history rather than as intent.
///
/// `path` is the issue's own file, as the index recorded it. It is passed in rather than
/// derived from the id here, because the index already knows where every record lives and a
/// second convention for locating one is how two readers come to disagree about which file
/// is the record.
#[allow(clippy::too_many_arguments)]
pub fn transition(
    root: &std::path::Path,
    path: &std::path::Path,
    index: &Index,
    plan: &Plan,
    id: &str,
    transition: Transition,
    now: &str,
    vocabulary: &crate::ledger::Vocabulary,
    git: &crate::git::GitState,
) -> Result<(), TransitionError> {
    check(index, plan, id, transition, now)?;
    let text = std::fs::read_to_string(path).map_err(|e| TransitionError::Write {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    // Both fields, in the order the shell writes them: the move's own stamp, then
    // `updated_at`. A record whose `updated_at` preceded its `completed_at` would be a
    // record that says it was finished before it was last touched.
    let text = set_field(&text, transition.field(), now);
    let text = set_field(&text, "updated_at", now);
    std::fs::write(path, text).map_err(|e| TransitionError::Write {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    crate::ledger::append(
        root,
        vocabulary,
        git,
        now,
        transition.event(),
        &[("issue", id.to_string())],
    )
    .map_err(|e| TransitionError::Write {
        path: root.join(".ai/local/state/ledger.jsonl"),
        reason: e.to_string(),
    })?;
    Ok(())
}

/// Set one top-level scalar field of a record, exactly as `mj_pj_set_field` in
/// `lib/project.sh` does.
///
/// The rule is small and every part of it is load-bearing:
///
/// * a line beginning `key:` is replaced in place, keeping its position in the file, so a
///   record's field order survives every write;
/// * otherwise the field is inserted immediately *before* `evidence:`, because evidence is
///   a block and a scalar appended after it would be read as one of its entries;
/// * with no `evidence:` line, the field is appended at the end.
///
/// Only the first `evidence:` line is an anchor, and matching is on the line's start, which
/// is what `grep -qE "^$key:"` and the awk beside it do. A nested key of the same name is
/// indented and is therefore not matched — the same blindness, deliberately, because the
/// two engines must be blind in the same places.
fn set_field(text: &str, key: &str, value: &str) -> String {
    let prefix = format!("{key}:");
    let line = format!("{key}: {value}");
    if text.lines().any(|l| l.starts_with(&prefix)) {
        let mut out = String::with_capacity(text.len() + line.len());
        for l in text.lines() {
            if l.starts_with(&prefix) {
                out.push_str(&line);
            } else {
                out.push_str(l);
            }
            out.push('\n');
        }
        return out;
    }
    let mut out = String::with_capacity(text.len() + line.len() + 1);
    let mut inserted = false;
    for l in text.lines() {
        if !inserted && l.starts_with("evidence:") {
            out.push_str(&line);
            out.push('\n');
            inserted = true;
        }
        out.push_str(l);
        out.push('\n');
    }
    if !inserted {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod transition_tests {
    use super::*;

    /// Replacing in place keeps the field where the record put it: a write must not reorder
    /// a record a person reads.
    #[test]
    fn an_existing_field_is_replaced_where_it_stands() {
        let before = "id: I0001\nstarted_at: \ntitle: one\n";
        assert_eq!(
            set_field(before, "started_at", "2026-09-11T12:00:00Z"),
            "id: I0001\nstarted_at: 2026-09-11T12:00:00Z\ntitle: one\n"
        );
    }

    /// A new scalar goes before `evidence:`, never after: appended at the end it would be
    /// indented under nothing and read as part of the evidence block.
    #[test]
    fn a_new_field_is_inserted_before_the_evidence_block() {
        let before = "id: I0001\ntitle: one\nevidence:\n  - covers: t\n";
        assert_eq!(
            set_field(before, "completed_at", "2026-09-11T12:00:00Z"),
            "id: I0001\ntitle: one\ncompleted_at: 2026-09-11T12:00:00Z\nevidence:\n  - covers: t\n"
        );
    }

    /// With no evidence block there is nothing to sit before, and the field is appended.
    #[test]
    fn a_new_field_is_appended_when_there_is_no_evidence_block() {
        let before = "id: I0001\ntitle: one\n";
        assert_eq!(
            set_field(before, "completed_at", "2026-09-11T12:00:00Z"),
            "id: I0001\ntitle: one\ncompleted_at: 2026-09-11T12:00:00Z\n"
        );
    }

    /// An indented key of the same name is not a top-level field and is not touched — the
    /// shell's `^key:` is anchored the same way, and the two must be blind together.
    #[test]
    fn an_indented_key_of_the_same_name_is_not_the_field() {
        let before = "id: I0001\nevidence:\n  - started_at: x\n";
        assert_eq!(
            set_field(before, "started_at", "T"),
            "id: I0001\nstarted_at: T\nevidence:\n  - started_at: x\n"
        );
    }

    /// A record with no trailing newline gets one: every writer of this repository's YAML
    /// ends the file, and a record that lost its newline would move every later diff.
    #[test]
    fn the_record_ends_with_a_newline() {
        assert_eq!(set_field("id: I0001", "x", "y"), "id: I0001\nx: y\n");
    }
}
