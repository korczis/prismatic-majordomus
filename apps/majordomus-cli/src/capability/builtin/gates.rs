//! The `gates` module: completion as a state the repository decides.
//!
//! Two capabilities, and the split is the same one [`super::obligations`] makes for the
//! same reason.
//!
//! **The model is declared.** `.ai/repo/ci/gates.yaml` is this repository's CI model: every
//! validation gate, the job that runs it, the command it runs, and the path classes that
//! decide which change selects which gate. `gates.model` reads it and answers in any
//! checkout that has one, whether or not the lifecycle has ever run.
//!
//! **The completion is local.** Whether *this* task may be called finished depends on the
//! task record and the ledger under `.ai/local/state/`, which is one checkout's own state
//! and is never tracked or published. `gates.completion` answers about the checkout the
//! process was started in and about no other.
//!
//! **It is read, never written.** `majordomus evidence --gate` records a run; this judges
//! the record. A second writer for the same ledger would be a second account of events,
//! which is what ADR 0030 refuses.
//!
//! **One judgement, and every surface reads it.** The shell tool's `check` and `finish`
//! call this executable rather than re-deriving the answer, the HTTP route serves the same
//! execution, MCP exposes the same tool, and the Cockpit renders the same document. That is
//! the property the module exists for: a worker cannot get a different verdict by asking a
//! different surface, and no surface can be persuaded that "done" is true because an agent
//! said so.
//!
//! ```
//! use majordomus_cli::capability::builtin::gates::module;
//!
//! // the whole module, as every projection reads it: two capabilities, and neither has a
//! // command-line arm of its own because `check` and `finish` are the command line's answer
//! let m = module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|c| c.capability.id.as_str()).collect();
//! assert_eq!(ids, ["gates.model", "gates.policy", "gates.completion"]);
//! for entry in &m.capabilities {
//!     assert!(entry.capability.exposure.cli.is_none());
//!     assert!(entry.capability.exposure.http.is_some());
//! }
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::deploy::targets::{
    self, ApplicationFact, DeploymentIdentity, DeploymentPlan, PlanFacts,
};
use crate::gates::{
    self, judge, model, Completion, CompletionPolicy, HandoverStanding, IssueStanding,
    ReleaseStanding, VersionSummary,
};
use crate::{capability, module};

use super::get;
use super::obligations::Obligation;
use super::views::Empty;

/// The URI under which the CI model is read as an MCP resource.
pub const GATES_URI: &str = "majordomus://gates";

/// The URI under which this checkout's completion state is read as an MCP resource.
pub const COMPLETION_URI: &str = "majordomus://gates/completion";

/// The URI under which the completion policy is read as an MCP resource.
pub const POLICY_URI: &str = "majordomus://gates/policy";

/// The completion policy, as every projection states it: the stages, the questions and
/// the source each is answered from, plus the fragment the provider bootstraps carry.
/// The policy is flattened into the report, so a reader of `gates.policy` sees the same
/// keys the file carries with the derived fields beside them.
///
/// ```
/// use majordomus_cli::capability::builtin::gates::CompletionPolicyReport;
/// use majordomus_cli::gates::CompletionPolicy;
///
/// let policy = CompletionPolicy::parse(
///     "version: 1\nstages:\n  - id: a\n    title: Alpha\n    summary: s\nquestions:\n  - id: q\n    stage: a\n    question: Is it?\n    source: gate:site-check\n    remediation: run it\n",
///     "share/completion.yaml",
/// )
/// .unwrap();
/// let report = CompletionPolicyReport {
///     fragment: policy.bootstrap_fragment(),
///     problems: policy.validate(&[]),
///     unanswered_gates: policy.unanswered_gates(&[]),
///     policy,
/// };
/// assert_eq!(report.fragment, "- Alpha: q\n");
/// assert_eq!(report.unanswered_gates, ["q:site-check"]);
/// let json = serde_json::to_value(&report).unwrap();
/// assert_eq!(json["stages"][0]["id"], "a", "the policy's own keys, flattened");
/// assert!(json.get("problems").is_none(), "a coherent distribution writes no problems");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompletionPolicyReport {
    /// The policy as read from the distribution.
    #[serde(flatten)]
    pub policy: CompletionPolicy,
    /// The generated section of an instruction file (AGENTS.md, CLAUDE.md): one line per
    /// stage with the questions that belong to it. The shell tool renders the same bytes.
    pub fragment: String,
    /// What does not resolve against the vocabulary the policy is shipped beside: a token
    /// `share/obligations.yaml` lacks. Empty in a coherent distribution, and never silently so.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
    /// The questions answered by a gate this repository's CI model does not declare, as
    /// `question:gate`. Not a problem: the report answers them `exempt` by name. Listed so
    /// that a repository can see what the policy would ask and it never does.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unanswered_gates: Vec<String>,
}

// ---------------------------------------------------------------- output types

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The CI model this repository declares: what may refuse a change, and what selects each.
///
/// Two things about this report are worth knowing before reading one. `count` is measured
/// from `gates` rather than written down, so the two can never disagree the way a
/// hand-maintained total does. And `unreadable` is how a reader tells "this repository
/// declares no CI model" from "the call failed": a repository of the layer is not obliged
/// to declare gates, so the empty model is an answer, and it says why it is empty.
///
/// ```
/// use majordomus_cli::capability::builtin::gates::GateModelReport;
/// use serde_json::json;
///
/// // the shape every surface serves: `unreadable` is skipped when the model was read
/// let r: GateModelReport = serde_json::from_value(json!({
///     "version": 1,
///     "source": ".ai/repo/ci/gates.yaml",
///     "count": 0,
///     "on_demand": [],
///     "gates": [],
///     "classes": [],
/// })).unwrap();
/// assert_eq!(r.count, r.gates.len(), "the total is measured, never declared");
/// assert!(r.unreadable.is_none(), "read, and it declares nothing");
///
/// // and the other empty answer, which a reader must not confuse with that one
/// let absent: GateModelReport = serde_json::from_value(json!({
///     "version": 0,
///     "source": ".ai/repo/ci/gates.yaml",
///     "count": 0,
///     "on_demand": [],
///     "gates": [],
///     "classes": [],
///     "unreadable": "no such file",
/// })).unwrap();
/// assert_eq!(absent.unreadable.as_deref(), Some("no such file"));
/// ```
pub struct GateModelReport {
    /// The schema version the file states.
    pub version: u32,
    /// Where it was read from, repository-relative.
    pub source: String,
    /// How many gates it declares; measured, not written down.
    pub count: usize,
    /// The gates whose runner cannot be had on demand, so no routine plan selects them.
    /// A gate that never runs protects nothing, and this is where that is said out loud.
    pub on_demand: Vec<String>,
    /// Every gate, in declaration order, with the pathspecs its evidence is taken over.
    pub gates: Vec<GateModelEntry>,
    /// Every path class, in declaration order.
    pub classes: Vec<model::GateClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Why there is no model to report, when there is none. A repository of the layer does
    /// not have to declare a CI model, and a reader is entitled to the difference between
    /// "this repository declares no gates" and "this call failed".
    pub unreadable: Option<String>,
}

/// ```
/// use majordomus_cli::capability::builtin::gates::GateModelEntry;
/// use majordomus_cli::gates::GateDecl;
///
/// let decl: GateDecl = serde_json::from_value(serde_json::json!({
///     "id": "site-check",
///     "job": "site",
///     "runs": "scripts/site-check",
///     "summary": "the site is whole"
/// }))
/// .unwrap();
/// let entry = GateModelEntry { declared: decl, inputs: vec!["site/**".into()] };
/// // the declaration as the file carries it, plus the paths its evidence is taken over —
/// // derived from the classes that select it, never written down a second time
/// assert_eq!(entry.declared.id, "site-check");
/// assert_eq!(entry.inputs, ["site/**"]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One gate of the model, with what the model implies about it.
pub struct GateModelEntry {
    #[serde(flatten)]
    /// The declaration as the file carries it.
    pub declared: model::GateDecl,
    /// The pathspecs a run of this gate is evidence over, derived from the classes that
    /// select it. A change to any of them makes a recorded run stale.
    pub inputs: Vec<String>,
}

// ---------------------------------------------------------------- input

/// ```
/// use majordomus_cli::capability::builtin::gates::CompletionInput;
///
/// // the default asks about the active task's own change set
/// let mine = CompletionInput::default();
/// assert!(mine.changed.is_none() && mine.base.is_none());
///
/// // and a caller may name the change instead, which is how a test drives it
/// let named = CompletionInput {
///     changed: Some(vec!["docs/CLI.md".into()]),
///     ..Default::default()
/// };
/// assert_eq!(named.changed.unwrap(), ["docs/CLI.md"]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which change to judge completion over.
pub struct CompletionInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The changed paths, repository-relative. Absent means the task's own change set:
    /// everything between the commit the task started at and the working tree.
    pub changed: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit to compare with, when the task's own starting commit is not wanted.
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Also plan the gates the model marks on-demand. Off by default, because a plan that
    /// selects a gate whose runner is unavailable is a plan whose verdict never arrives.
    pub on_demand: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Settle the obligations something can establish — a commit, a push, the trunk, the
    /// published site, the deployed surfaces — live, at HEAD. On by default; a report that
    /// passes `false` reads the ledger alone and says a live token is owed, never proven.
    pub live: Option<bool>,
}

impl BenchmarkCases for CompletionInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("this-task", CompletionInput::default()),
            NamedCase::new(
                "one-source-file",
                CompletionInput {
                    changed: Some(vec!["apps/majordomus-cli/src/lib.rs".into()]),
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "a-document",
                CompletionInput {
                    changed: Some(vec!["docs/CLI.md".into()]),
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "everything-on-demand",
                CompletionInput {
                    changed: Some(vec![".ai/repo/ci/gates.yaml".into()]),
                    on_demand: Some(true),
                    ..Default::default()
                },
            ),
        ]
    }
}

// ---------------------------------------------------------------- handlers

/// The completion policy the distribution ships, located the way the vocabulary is.
fn policy(ctx: &Context) -> Result<CompletionPolicy, String> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let share = crate::share::Share::locate(ctx.index.share.as_deref(), &root)
        .map_err(|e| format!("no distribution to read the completion policy from: {e}"))?;
    CompletionPolicy::load(share.dir())
}

fn gates_policy(ctx: &Context, _: Empty) -> Result<CompletionPolicyReport, CapabilityError> {
    let policy = policy(ctx).map_err(CapabilityError::NotFound)?;
    let tokens: Vec<String> = vocabulary(ctx)
        .map(|v| v.into_iter().map(|o| o.id).collect())
        .unwrap_or_default();
    let root = PathBuf::from(&ctx.index.repository.root);
    let gates: Vec<String> = model::GateModel::load(&root)
        .map(|m| m.gates.iter().map(|g| g.id.clone()).collect())
        .unwrap_or_default();
    let problems = policy.validate(&tokens);
    let unanswered_gates = policy.unanswered_gates(&gates);
    Ok(CompletionPolicyReport {
        fragment: policy.bootstrap_fragment(),
        policy,
        problems,
        unanswered_gates,
    })
}

/// The structural release analysis, reduced. Asked through the executor so that this
/// report and `release.analysis` cannot disagree; an analysis that cannot be made (no
/// release record, no history) is a reason, never a pass.
fn release_standing(ctx: &Context) -> (ReleaseStanding, Option<VersionSummary>) {
    if !ctx
        .index
        .objects
        .iter()
        .any(|o| o.kind == crate::release::changelog::RELEASE_KIND)
    {
        return (
            ReleaseStanding::NotApplicable(
                "not applicable: this repository has published no release, so there is no \
                 baseline to measure the contract against"
                    .into(),
            ),
            None,
        );
    }
    match ctx.execute("release.analysis", serde_json::json!({})) {
        Err(e) => (ReleaseStanding::Unknown(e.to_string()), None),
        Ok(v) => {
            let plan: crate::release::compat::VersionPlan = match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return (
                        ReleaseStanding::Unknown(format!(
                            "release.analysis answered something this report cannot read: {e}"
                        )),
                        None,
                    )
                }
            };
            if plan.has_errors() {
                let why = plan
                    .diagnostics
                    .iter()
                    .map(|d| d.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ");
                return (
                    ReleaseStanding::Unknown(format!("the release state is incoherent: {why}")),
                    None,
                );
            }
            let summary = VersionSummary {
                baseline: plan.baseline.version.clone(),
                declared: plan.declared_version.clone(),
                required: plan.required_version.clone(),
                impact: plan.required.as_str().to_string(),
                status: match plan.status {
                    crate::release::compat::Status::Ok => "ok".to_string(),
                    crate::release::compat::Status::Blocked => "blocked".to_string(),
                },
                breaking: plan.breaking,
                changes: plan.changes.len(),
            };
            let ok = summary.ok() && plan.writers_agree;
            let detail = if !plan.writers_agree {
                format!(
                    "the crate declares {} and the shell tool {}: the two writers disagree",
                    plan.declared_version, plan.tool_version
                )
            } else if ok {
                format!(
                    "{} required since {} ({} movement(s)); {} declared",
                    summary.impact, summary.baseline, summary.changes, summary.declared
                )
            } else {
                format!(
                    "{} required since {} ({} movement(s)): at least {} owed, {} declared",
                    summary.impact,
                    summary.baseline,
                    summary.changes,
                    summary.required,
                    summary.declared
                )
            };
            (ReleaseStanding::Measured { ok, detail }, Some(summary))
        }
    }
}

/// The issue the task names, resolved against the plan's objects in the index.
fn issue_standing(ctx: &Context, task: Option<&super::ActiveTask>) -> IssueStanding {
    let Some(task) = task else {
        return IssueStanding::Unknown("no task is active in this checkout".into());
    };
    let id = task.issue.trim();
    if id.is_empty() {
        // what `start` was told instead of an issue (ADR 0111): an exemption with its
        // reason, or an intent named directly. Named, never judged here.
        let exemption = task.exemption.trim();
        let intent = task.intent.trim();
        return if !exemption.is_empty() {
            let because = task.exemption_because.trim();
            IssueStanding::DeclaredNone(if because.is_empty() {
                format!("exempt as {exemption}")
            } else {
                format!("exempt as {exemption}: {because}")
            })
        } else if !intent.is_empty() {
            IssueStanding::DeclaredNone(format!("it serves intent {intent}, named directly"))
        } else {
            IssueStanding::Undeclared
        };
    }
    match ctx
        .index
        .objects
        .iter()
        .find(|o| o.kind == crate::plan::ISSUE && o.identity == id)
    {
        Some(o) => IssueStanding::Resolved {
            id: id.to_string(),
            title: o.title.clone().unwrap_or_default(),
        },
        None => IssueStanding::Unresolved { id: id.to_string() },
    }
}

/// Whether a handover for this task exists: a record under the continuity store whose
/// front matter names the task. Read here rather than asked of `continuity.state`, because
/// that answers "the newest record for this worktree" and this asks "any record for this
/// task", which is a different question with a different answer.
fn handover_standing(root: &Path, task: Option<&super::ActiveTask>) -> HandoverStanding {
    let Some(task) = task else {
        return HandoverStanding::Unknown("no task is active in this checkout".into());
    };
    let dir = root.join(gates::STATE_DIR).join("handovers");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return HandoverStanding::Absent,
    };
    let needle = format!("task_id: {}", task.id);
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .filter(|e| {
            std::fs::read_to_string(e.path())
                .map(|t| t.lines().take(20).any(|l| l.trim() == needle))
                .unwrap_or(false)
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    crate::order::canonical_strings(&mut names);
    match names.pop() {
        Some(n) => HandoverStanding::Present(n),
        None => HandoverStanding::Absent,
    }
}

/// The deployment plan: which surfaces this change reaches, from what the repository
/// declares. Shared with `deploy.verify`, which asks the same plan what is live.
pub(crate) fn deployment_plan(
    ctx: &Context,
    m: Option<&gates::GateModel>,
    changed: &[String],
    expected_commit: Option<String>,
    everything: bool,
) -> DeploymentPlan {
    let root = PathBuf::from(&ctx.index.repository.root);
    let latest_release = crate::distribution::Releases::from_index(&ctx.index)
        .ok()
        .and_then(|r| {
            r.latest_stable().map(|rel| DeploymentIdentity {
                commit: Some(rel.commit.clone()),
                version: Some(rel.version.clone()),
                tag: Some(rel.tag.clone()),
            })
        });
    let applications: Vec<ApplicationFact> = ctx
        .index
        .objects
        .iter()
        .filter(|o| o.kind == crate::deploy::KIND)
        .filter_map(|o| crate::deploy::Deployment::parse(o).ok())
        .map(|d| ApplicationFact {
            id: d.id.clone(),
            status: match d.status {
                Some(crate::deploy::Status::Active) => "active".to_string(),
                Some(crate::deploy::Status::Retired) => "retired".to_string(),
                Some(crate::deploy::Status::Declared) | None => "declared".to_string(),
            },
            url: d.url.clone(),
            // a build input names a directory or a file; either way the pathspec that
            // selects a change under it is the input followed by everything below
            inputs: d
                .build
                .inputs
                .iter()
                .flat_map(|i| {
                    let i = i.trim_end_matches('/');
                    [i.to_string(), format!("{i}/**")]
                })
                .collect(),
        })
        .collect();
    let facts = PlanFacts {
        site_base_url: targets::site_base_url(&root),
        site_inputs: m.map(|m| m.inputs_of("site-build")).unwrap_or_default(),
        surface_inputs: m
            .map(|m| m.inputs_of("version-surface"))
            .unwrap_or_default(),
        latest_release,
        applications,
        expected_commit,
        declared_version: crate::release::version::declared(&root),
    };
    targets::plan(&facts, changed, everything)
}

/// The vocabulary the distribution ships, read the way every other reader locates it.
fn vocabulary(ctx: &Context) -> Result<Vec<Obligation>, String> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let share = crate::share::Share::locate(ctx.index.share.as_deref(), &root)
        .map_err(|e| format!("no distribution to read the obligation vocabulary from: {e}"))?;
    let path = share.dir().join("obligations.yaml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    #[derive(Deserialize)]
    struct File {
        obligations: Vec<Obligation>,
    }
    let file: File =
        crate::metadata::yaml::parse_into(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(file.obligations)
}

/// The CI model, or an empty one and the reason there is none.
///
/// A repository of the layer does not have to declare gates, and every reader of this module
/// already says so in its own way: `lib/gates.sh` skips with "this repository declares no
/// readable CI model" rather than failing. Refusing the call instead would make a capability
/// that cannot answer about most repositories — and the generic tests that replay every
/// capability's benchmark cases against a fixture are exactly where that showed.
fn load_model(root: &Path) -> (model::GateModel, Option<String>) {
    match model::GateModel::load(root) {
        Ok(m) => (m, None),
        Err(reason) => (
            model::GateModel {
                version: 0,
                gates: Vec::new(),
                classes: Vec::new(),
            },
            Some(reason),
        ),
    }
}

fn gates_model(ctx: &Context, _: Empty) -> Result<GateModelReport, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let (m, unreadable) = load_model(&root);
    Ok(GateModelReport {
        version: m.version,
        source: model::MODEL_PATH.to_string(),
        count: m.gates.len(),
        on_demand: m
            .gates
            .iter()
            .filter(|g| g.on_demand)
            .map(|g| g.id.clone())
            .collect(),
        gates: m
            .gates
            .iter()
            .map(|g| GateModelEntry {
                inputs: m.inputs_of(&g.id),
                declared: g.clone(),
            })
            .collect(),
        classes: m.classes.clone(),
        unreadable,
    })
}

fn gates_completion(ctx: &Context, input: CompletionInput) -> Result<Completion, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let mut findings: Vec<String> = Vec::new();

    let (m, unreadable) = load_model(&root);
    if let Some(reason) = unreadable {
        // no model is not no answer: every gate is unknown, said out loud, and `finishable`
        // is left to the obligations rather than granted by a silence
        findings.push(format!(
            "this repository declares no readable CI model at {}, so no gate can be judged              here — unknown, never a pass: {reason}",
            model::MODEL_PATH
        ));
    }
    let task = super::continuity::read_task(&gates::task_path(&root));
    if task.is_none() {
        findings.push(format!(
            "no active task in this checkout ({}); the gates are reported against the \
             working tree and nothing is attributed to a task",
            gates::task_path(&root)
                .strip_prefix(&root)
                .unwrap_or(&gates::task_path(&root))
                .display()
        ));
    }

    // the change set: what was asked for, or the task's own — from its starting commit to
    // the working tree, the uncommitted half included
    let changed = match input.changed {
        Some(list) => list,
        None => {
            let base = input
                .base
                .clone()
                .or_else(|| task.as_ref().map(|t| t.head.clone()));
            match gates::changed_paths(&root, base.as_deref()) {
                Ok(paths) => paths,
                Err(reason) => {
                    findings.push(format!(
                        "the change set could not be read from git, so applicability is \
                         computed over nothing and every gate is reported as it stands: {reason}"
                    ));
                    Vec::new()
                }
            }
        }
    };

    let vocab = match vocabulary(ctx) {
        Ok(v) => v,
        Err(reason) => {
            findings.push(format!(
                "the obligation vocabulary could not be read, so no obligation can be \
                 implied from the change: {reason}"
            ));
            Vec::new()
        }
    };

    let (runs, skipped) = match &task {
        Some(t) => judge::runs_for(&gates::ledger_path(&root), &t.id),
        None => (BTreeMap::new(), 0),
    };
    if skipped > 0 {
        findings.push(format!(
            "{skipped} ledger line(s) could not be read and were skipped; run `majordomus doctor`"
        ));
    }

    // one hash per gate, over the files that select it. Taken here rather than inside the
    // judgement so that a test drives the same function with hashes of its own. The gates'
    // inputs overlap heavily, so every file is read and hashed once for the whole report:
    // taken gate by gate, this loop read the tree once per gate and a page took half a
    // minute to say what it had not verified.
    let mut hashes: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut seen = super::obligations::FileHashes::default();
    for gate in &m.gates {
        let specs = m.inputs_of(&gate.id);
        let hash = super::obligations::inputs_hash_with(&root, &specs, &mut seen).map(|(h, _)| h);
        // a token with no inputs hashes to the empty string in the ledger, and is compared
        // as such; `None` here means the hash could not be taken at all
        hashes.insert(
            gate.id.clone(),
            if specs.is_empty() {
                Some(String::new())
            } else {
                hash
            },
        );
    }

    // The obligation half of the done invariant is `obligations.closure`'s judgement, asked
    // for through the same executor MCP and the HTTP routes use, so this report cannot
    // disagree with `majordomus check` about whether an obligation stands. Nothing is
    // re-judged here; the state word comes back and is translated once.
    let mut standing: BTreeMap<String, gates::ObligationStanding> = BTreeMap::new();
    let mut closure_reachable = false;
    match ctx.execute(
        "obligations.closure",
        serde_json::json!({ "live": input.live.unwrap_or(true) }),
    ) {
        Ok(value) => match serde_json::from_value::<super::obligations::Closure>(value) {
            Ok(closure) => {
                closure_reachable = true;
                for o in closure.obligations {
                    standing.insert(
                        o.id.clone(),
                        gates::ObligationStanding {
                            state: o.state.as_str().to_string(),
                            detail: o.detail,
                            reproduce: o.reproduce,
                        },
                    );
                }
            }
            Err(e) => findings.push(format!(
                "obligations.closure answered something this report cannot read, so every \
                 obligation question is unknown rather than passing: {e}"
            )),
        },
        Err(e) => findings.push(format!(
            "obligations.closure could not be executed, so every obligation question is \
             unknown rather than passing: {e}"
        )),
    }

    // The topology half of the done invariant — "is a branch, worktree or pull request left
    // behind?" — is `convergence.report`'s verdict, asked through the same executor for the
    // same reason: one measurement of where this repository's work is held, not two.
    let convergence = match ctx.execute("convergence.report", serde_json::json!({})) {
        Ok(value) => match serde_json::from_value::<crate::convergence::ConvergenceReport>(value) {
            Ok(report) => Some(report),
            Err(e) => {
                findings.push(format!(
                    "convergence.report answered something this report cannot read, so whether \
                     work is left behind is unknown rather than passing: {e}"
                ));
                None
            }
        },
        Err(e) => {
            findings.push(format!(
                "convergence.report could not be executed, so whether work is left behind is \
                 unknown rather than passing: {e}"
            ));
            None
        }
    };

    let policy = match policy(ctx) {
        Ok(p) => p,
        Err(e) => return Err(CapabilityError::NotFound(e)),
    };
    let (release, version) = release_standing(ctx);
    let issue = issue_standing(ctx, task.as_ref());
    let handover = handover_standing(&root, task.as_ref());
    let head = gates::head_of(&root);
    let deployment = deployment_plan(ctx, Some(&m), &changed, head, false);
    let sources = gates::Sources {
        policy: &policy,
        standing: &standing,
        closure_reachable,
        release,
        version,
        issue,
        handover,
        deployment,
        convergence: convergence.as_ref(),
    };

    Ok(gates::complete(
        &m,
        &changed,
        task.as_ref(),
        &vocab,
        &runs,
        &hashes,
        &sources,
        input.on_demand.unwrap_or(false),
        &crate::peers::rfc3339(std::time::SystemTime::now()),
        findings,
    ))
}

// ---------------------------------------------------------------- the module

/// The `gates` module: the CI model every clone shares, and the completion state only this
/// checkout can answer.
///
/// Neither has a command-line projection of its own, for the reason [`super::obligations`]
/// states about the local half: `majordomus check` and `majordomus finish` are already the
/// command line's answer to this question, and they read this module rather than deriving a
/// second one. What is new is that they now have a gate half to read.
///
/// ```
/// use majordomus_cli::capability::builtin::gates::module;
/// let m = module();
/// assert_eq!(m.id.as_str(), "gates");
/// // every projection is derived from the declaration and written nowhere else
/// let routes: Vec<&str> = m
///     .capabilities
///     .iter()
///     .filter_map(|e| e.capability.exposure.http.as_ref().map(|h| h.path.as_str()))
///     .collect();
/// assert_eq!(routes, ["/api/v1/gates", "/api/v1/gates/policy", "/api/v1/gates/completion"]);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "gates",
        title: "Completion gates",
        description: "Completion as a state the repository decides rather than a claim a worker makes: the validation gates this repository declares in its CI model, which of them a task's own change set selects, what each one last reported, whether that verdict still describes the tree it was taken over, and therefore whether the task may be called finished. A gate that has never reported is `queued` and not `pass`, a run whose files have changed since is `stale` and refuses exactly as a failure does, and a gate the change cannot affect is `exempt` rather than unknown.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "gates.model",
                title: "The CI model",
                description: "Every validation gate this repository declares — the job that runs it, the command it runs, what it proves, whether every plan selects it and whether its runner can be had on demand at all — with the path classes that decide which change selects which gate, and, derived from those classes, the pathspecs each gate's evidence is taken over.",
                input: Empty,
                output: GateModelReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_gates".into()),
                        resource: Some(McpResource { uri: GATES_URI.into(), name: "gates".into() }),
                    }),
                    http: get("/api/v1/gates"),
                    cli: None,
                },
                tags: ["gates", "ci", "completion", "introspection"],
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: Some(5) },
                handler: gates_model,
            },
            capability! {
                id: "gates.policy",
                title: "The completion policy",
                description: "The one definition of done: the lifecycle stages in order, every question a task must answer before it may be called finished, and the source each answer is taken from — an obligation of share/obligations.yaml, a gate of the CI model, the structural release analysis, the change set, the task record or the continuity store. Read from share/completion.yaml; every surface that states what done means, the generated section of the provider bootstraps included, is a projection of this answer. A source the policy names and the repository lacks is reported as a problem rather than silently unanswerable.",
                input: Empty,
                output: CompletionPolicyReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_completion_policy".into()),
                        resource: Some(McpResource { uri: POLICY_URI.into(), name: "completion-policy".into() }),
                    }),
                    http: get("/api/v1/gates/policy"),
                    cli: None,
                },
                tags: ["gates", "completion", "policy", "introspection"],
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: Some(5) },
                handler: gates_policy,
            },
            capability! {
                id: "gates.completion",
                title: "Whether this task may be called finished",
                description: "The active task's own change set — from the commit it started at to the working tree, uncommitted files included — put through the CI model: which gates it selects and why, what each one last reported and over which files, which verdicts have gone stale because the tree moved underneath them, which required gates have never reported at all, and which obligations the change implies whether or not the task declared them. `finishable` is false only when a required gate is known to be failing, stale or blocked by one that is; absence of a verdict is reported as unverified rather than silently accepted.",
                input: CompletionInput,
                output: Completion,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_completion".into()),
                        resource: Some(McpResource { uri: COMPLETION_URI.into(), name: "completion".into() }),
                    }),
                    http: get("/api/v1/gates/completion"),
                    cli: None,
                },
                tags: ["gates", "completion", "tasks", "evidence"],
                cache: CachePolicy::Disabled,
                handler: gates_completion,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_module_declares_what_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "gates");
        assert_eq!(m.capabilities.len(), 3);
        for e in &m.capabilities {
            assert_eq!(e.capability.id.namespace(), "gates");
            assert!(
                e.capability.exposure.mcp.is_some() && e.capability.exposure.http.is_some(),
                "{} must be readable on every surface without one recomputing it",
                e.capability.id
            );
        }
    }

    #[test]
    fn the_completion_answer_is_never_cached() {
        // a completion state cached for even a few seconds is a state that can say
        // `finishable` about a tree that has already changed, which is the defect
        let m = module();
        let c = m
            .capabilities
            .iter()
            .find(|e| e.capability.id.as_str() == "gates.completion")
            .expect("declared");
        assert!(matches!(c.capability.cache, CachePolicy::Disabled));
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct NotAVerdict {
        converged: String,
    }

    fn not_a_verdict(_: &Context, _: super::super::Empty) -> Result<NotAVerdict, CapabilityError> {
        Ok(NotAVerdict {
            converged: "perhaps".into(),
        })
    }

    /// The completion report over `modules`, asked of a synthetic repository with no git.
    fn completion_with(
        modules: Vec<crate::capability::module::ModuleDescriptor>,
    ) -> crate::gates::Completion {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let mut index = repo.index().unwrap();
        // the completion policy is the distribution's (share/completion.yaml), and a synthetic
        // repository carries no share of its own: the crate's is the one this build ships
        index.share = Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share"));
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(modules)
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        let value = ctx
            .execute("gates.completion", serde_json::json!({ "changed": [] }))
            .expect("the completion report answers");
        serde_json::from_value(value).expect("a completion report")
    }

    /// Whether work is left behind is `convergence.report`'s answer. When that answer cannot
    /// be had — the capability refused, or it answered in a shape this report cannot read —
    /// the question is unknown and a finding says which, never a pass.
    #[test]
    fn a_convergence_answer_that_cannot_be_had_or_read_is_unknown_and_says_why() {
        let stale = |c: &crate::gates::Completion| {
            c.questions
                .iter()
                .find(|q| q.id == "no-stale-topology")
                .expect("the invariant asks it")
                .status
        };

        // no git here, so the real capability refuses
        let refused = completion_with(super::super::modules());
        assert!(
            refused
                .findings
                .iter()
                .any(|f| f.starts_with("convergence.report could not be executed")),
            "{:?}",
            refused.findings
        );
        assert_eq!(stale(&refused), crate::gates::GateStatus::Unknown);

        let impostor = crate::module! {
            id: "convergence",
            title: "Convergence",
            description: "Answers in a shape the completion report cannot read.",
            stability: Stability::Experimental,
            capabilities: [
                crate::capability! {
                    id: "convergence.report", title: "Not a verdict",
                    description: "Something else.",
                    input: super::super::Empty, output: NotAVerdict,
                    stability: Stability::Experimental,
                    exposure: crate::capability::model::Exposure::default(), tags: [],
                    handler: not_a_verdict,
                },
            ],
        };
        let mut modules: Vec<_> = super::super::modules()
            .into_iter()
            .filter(|m| m.id.as_str() != "convergence")
            .collect();
        modules.push(impostor);
        let unreadable = completion_with(modules);
        assert!(
            unreadable
                .findings
                .iter()
                .any(|f| f
                    .starts_with("convergence.report answered something this report cannot read")),
            "{:?}",
            unreadable.findings
        );
        assert_eq!(stale(&unreadable), crate::gates::GateStatus::Unknown);
    }

    // ------------------------------------------------------------ fixtures

    use crate::capability::module::ModuleDescriptor as Module;
    use crate::synthetic::SyntheticRepository;

    fn crate_share() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")
    }

    /// A context over `repo`: its index with `objects` added, the distribution at `share`,
    /// and `modules` as the registry.
    fn context_over(
        repo: &SyntheticRepository,
        share: &Path,
        objects: Vec<crate::model::Object>,
        modules: Vec<Module>,
    ) -> Context {
        let mut index = repo.index().expect("the synthetic repository indexes");
        index.share = Some(share.to_path_buf());
        index.objects.extend(objects);
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(modules)
            .with_index(&index)
            .build()
            .expect("the registry builds");
        Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry))
    }

    /// An object as discovery would have indexed it, holding only what these readers read.
    fn object(kind: &str, identity: &str, metadata: serde_json::Value) -> crate::model::Object {
        crate::model::Object {
            kind: kind.into(),
            identity: identity.into(),
            uri: format!("majordomus://{kind}/{identity}"),
            title: None,
            description: None,
            metadata,
            body: String::new(),
            content: String::new(),
            media_type: "application/yaml",
            provenance: crate::model::Provenance {
                path: format!(".ai/repo/{kind}/{identity}.yaml"),
                directory: format!(".ai/repo/{kind}"),
                source_class: kind.into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    fn release_record() -> crate::model::Object {
        object(
            crate::release::changelog::RELEASE_KIND,
            "v0.5.0",
            serde_json::json!({
                "schema": "release/v1", "version": "0.5.0", "tag": "v0.5.0",
                "channel": "stable", "commit": "abc", "artifacts": [],
                "published_at": "2026-09-01T00:00:00Z",
            }),
        )
    }

    fn deployment(id: &str, status: Option<&str>, url: Option<&str>) -> crate::model::Object {
        let mut record = serde_json::json!({
            "schema": "deployment/v1", "kind": "deployment", "id": id,
            "title": "A deployment", "application": id,
            "build": {
                "package": "majordomus-cli", "binary": "majordomus", "profile": "release",
                "inputs": ["apps/majordomus-cli/"],
            },
            "listen": { "port": 8080, "interface": "all" },
            "health": { "liveness": "/api/v1/live", "readiness": "/api/v1/ready" },
            "resources": { "cpu_kind": "shared", "cpus": 1, "memory_mb": 256 },
            "machines": { "count": 1, "min_running": 0, "autostart": true, "autostop": true },
            "region": "fra",
            "provider": { "name": "fly" },
        });
        if let Some(status) = status {
            record["status"] = status.into();
        }
        if let Some(url) = url {
            record["url"] = url.into();
        }
        object(crate::deploy::KIND, id, record)
    }

    fn task(fields: serde_json::Value) -> super::super::ActiveTask {
        let mut record = serde_json::json!({
            "id": "t-1", "task": "work", "profile": "implementation", "outcome": "active",
        });
        for (key, value) in fields.as_object().expect("an object of fields") {
            record[key] = value.clone();
        }
        serde_json::from_value(record).expect("a task record")
    }

    /// git in `dir`, with nothing of this machine's configuration reaching the fixture.
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    const MODEL: &str = "version: 1\ngates:\n  - id: projection-closure\n    job: structure\n    always: true\n    runs: scripts/ci/projection-closure\n    summary: every projection is current\n  - id: site-build\n    job: site\n    runs: scripts/site-build\n    summary: the site builds\n  - id: version-surface\n    job: rust\n    runs: scripts/ci/version-surface\n    summary: the version covers the surface\n  - id: on-request\n    job: structure\n    runs: scripts/on-request\n    summary: nothing selects it\nclasses:\n  - id: docs\n    paths: [docs/**]\n    gates: [site-build]\n  - id: rust\n    paths: [apps/**]\n    gates: [version-surface]\n";

    fn declare_model(repo: &SyntheticRepository) {
        let path = repo.root().join(model::MODEL_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, MODEL).unwrap();
    }

    // ------------------------------------------------------------ the policy

    /// The report resolves the shipped policy against the vocabulary beside it and against
    /// this repository's own model: a gate question the model does not declare is listed,
    /// not counted as a problem.
    #[test]
    fn the_policy_report_names_the_gate_questions_this_repository_never_asks() {
        let repo = SyntheticRepository::small().unwrap();
        declare_model(&repo);
        let ctx = context_over(&repo, &crate_share(), vec![], super::super::modules());
        let report = gates_policy(&ctx, Empty {}).expect("the shipped policy is read");
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(
            report.unanswered_gates,
            ["changelog:release-check"],
            "the model declares projection-closure and no release-check"
        );
        assert_eq!(report.fragment, report.policy.bootstrap_fragment());
        assert_eq!(report.policy.source, "share/completion.yaml");
    }

    /// A distribution that ships no definition of done cannot be asked what done is, and
    /// neither surface answers with an empty policy instead.
    #[test]
    fn a_distribution_without_a_completion_policy_is_refused_and_says_which() {
        let refusal = |r: Result<String, CapabilityError>| match r {
            Err(CapabilityError::NotFound(why)) => why,
            other => panic!("expected a refusal, and it answered {other:?}"),
        };
        let repo = SyntheticRepository::small().unwrap();

        let no_policy = tempfile::tempdir().unwrap();
        std::fs::write(
            no_policy.path().join("kinds.yaml"),
            "version: 1\nkinds: []\n",
        )
        .unwrap();
        let ctx = context_over(&repo, no_policy.path(), vec![], super::super::modules());
        let why = refusal(gates_policy(&ctx, Empty {}).map(|r| r.fragment));
        assert!(why.contains("completion.yaml"), "{why}");
        let input = CompletionInput {
            changed: Some(vec![]),
            ..Default::default()
        };
        let why = refusal(gates_completion(&ctx, input).map(|c| c.policy));
        assert!(why.contains("completion.yaml"), "{why}");

        let no_distribution = tempfile::tempdir().unwrap();
        let ctx = context_over(
            &repo,
            no_distribution.path(),
            vec![],
            super::super::modules(),
        );
        let why = refusal(gates_policy(&ctx, Empty {}).map(|r| r.fragment));
        assert!(
            why.starts_with("no distribution to read the completion policy from"),
            "{why}"
        );
    }

    // ------------------------------------------------------------ the release

    fn measured_plan() -> crate::release::compat::VersionPlan {
        use crate::release::compat::{Baseline, CommitEvidence, Impact, Policy, Status};
        crate::release::compat::VersionPlan {
            policy: Policy::for_version(crate::release::version::Version::parse("0.5.0").unwrap()),
            baseline: Baseline {
                version: "0.5.0".into(),
                reference: "v0.5.0".into(),
                read_at: "abc".into(),
                commit: "abc".into(),
                recorded: true,
                atoms: 1,
                fingerprint: "sha256:a".into(),
            },
            declared_version: "0.6.0".into(),
            tool_version: "0.6.0".into(),
            writers_agree: true,
            atoms: 2,
            fingerprint: "sha256:b".into(),
            implied: Impact::Minor,
            required: Impact::Minor,
            declared: Impact::Minor,
            required_version: "0.6.0".into(),
            status: Status::Ok,
            breaking: false,
            changes: Vec::new(),
            commits: CommitEvidence {
                commits: 3,
                implied: Impact::Patch,
                breaking: Vec::new(),
            },
            understated: false,
            diagnostics: Vec::new(),
        }
    }

    type Plan = Result<crate::release::compat::VersionPlan, CapabilityError>;

    fn plan_covering(_: &Context, _: super::super::Empty) -> Plan {
        Ok(measured_plan())
    }

    fn plan_understating(_: &Context, _: super::super::Empty) -> Plan {
        Ok(crate::release::compat::VersionPlan {
            declared_version: "0.5.0".into(),
            tool_version: "0.5.0".into(),
            declared: crate::release::compat::Impact::None,
            status: crate::release::compat::Status::Blocked,
            ..measured_plan()
        })
    }

    fn plan_with_a_stale_projection(_: &Context, _: super::super::Empty) -> Plan {
        Ok(crate::release::compat::VersionPlan {
            tool_version: "0.5.9".into(),
            writers_agree: false,
            ..measured_plan()
        })
    }

    fn plan_that_is_incoherent(_: &Context, _: super::super::Empty) -> Plan {
        Ok(crate::release::compat::VersionPlan {
            diagnostics: vec![crate::release::compat::Diagnostic {
                id: "tag-commit-mismatch".into(),
                severity: crate::release::compat::Severity::Error,
                message: "the tag names another commit".into(),
            }],
            ..measured_plan()
        })
    }

    /// A `release` module whose analysis is `$handler`, in place of the real one.
    macro_rules! release_answering {
        ($output:ty, $handler:ident) => {
            crate::module! {
                id: "release",
                title: "Release",
                description: "Stands in for the structural release analysis.",
                stability: Stability::Experimental,
                capabilities: [
                    crate::capability! {
                        id: "release.analysis", title: "A stand-in",
                        description: "Answers what the test wrote down.",
                        input: super::super::Empty, output: $output,
                        stability: Stability::Experimental,
                        exposure: crate::capability::model::Exposure::default(), tags: [],
                        handler: $handler,
                    },
                ],
            }
        };
    }

    /// The release standing of a repository that has published a release, with `analysis`
    /// as the only thing that answers `release.analysis`.
    fn standing_under(analysis: Option<Module>) -> (ReleaseStanding, Option<VersionSummary>) {
        let repo = SyntheticRepository::small().unwrap();
        let mut modules: Vec<_> = super::super::modules()
            .into_iter()
            .filter(|m| m.id.as_str() != "release")
            .collect();
        modules.extend(analysis);
        let ctx = context_over(&repo, &crate_share(), vec![release_record()], modules);
        release_standing(&ctx)
    }

    #[test]
    fn the_release_standing_is_the_analysis_reduced_and_states_what_is_owed() {
        let (standing, version) = standing_under(Some(release_answering!(
            crate::release::compat::VersionPlan,
            plan_covering
        )));
        assert_eq!(
            standing,
            ReleaseStanding::Measured {
                ok: true,
                detail: "minor required since 0.5.0 (0 movement(s)); 0.6.0 declared".into(),
            }
        );
        let version = version.expect("a measured analysis is reported");
        assert!(version.ok());
        assert_eq!(
            (
                version.baseline.as_str(),
                version.required.as_str(),
                version.impact.as_str()
            ),
            ("0.5.0", "0.6.0", "minor")
        );

        let (standing, version) = standing_under(Some(release_answering!(
            crate::release::compat::VersionPlan,
            plan_understating
        )));
        assert_eq!(
            standing,
            ReleaseStanding::Measured {
                ok: false,
                detail:
                    "minor required since 0.5.0 (0 movement(s)): at least 0.6.0 owed, 0.5.0 declared"
                        .into(),
            }
        );
        let version = version.expect("an understated version is still reported");
        assert_eq!(version.status, "blocked");
        assert!(!version.ok());
    }

    /// The declared version can cover the contract and still not be what the shell tool
    /// prints; that is refused in its own words, not reported as a pass.
    #[test]
    fn a_version_projection_that_is_not_current_refuses_although_the_version_covers() {
        let (standing, version) = standing_under(Some(release_answering!(
            crate::release::compat::VersionPlan,
            plan_with_a_stale_projection
        )));
        assert_eq!(
            standing,
            ReleaseStanding::Measured {
                ok: false,
                detail:
                    "the crate declares 0.6.0 and the shell tool 0.5.9: the two writers disagree"
                        .into(),
            }
        );
        assert!(
            version.expect("reported").ok(),
            "the version itself covers the contract"
        );
    }

    #[test]
    fn a_release_analysis_that_cannot_be_trusted_or_had_is_unknown_and_says_why() {
        let unknown = |answer: (ReleaseStanding, Option<VersionSummary>)| match answer {
            (ReleaseStanding::Unknown(why), None) => why,
            other => panic!("expected unknown with no version, and it answered {other:?}"),
        };

        let why = unknown(standing_under(Some(release_answering!(
            crate::release::compat::VersionPlan,
            plan_that_is_incoherent
        ))));
        assert_eq!(
            why,
            "the release state is incoherent: the tag names another commit"
        );

        let why = unknown(standing_under(Some(release_answering!(
            NotAVerdict,
            not_a_verdict
        ))));
        assert!(
            why.starts_with("release.analysis answered something this report cannot read"),
            "{why}"
        );

        let why = unknown(standing_under(None));
        assert!(
            why.contains("release.analysis"),
            "nothing answers it: {why}"
        );
    }

    // ------------------------------------------------------------ the task record

    #[test]
    fn the_issue_standing_is_what_the_task_names_resolved_against_the_plan() {
        let repo = SyntheticRepository::planned();
        let ctx = repo.context().unwrap();
        let ask = |fields: serde_json::Value| issue_standing(&ctx, Some(&task(fields)));

        assert_eq!(
            issue_standing(&ctx, None),
            IssueStanding::Unknown("no task is active in this checkout".into())
        );
        match ask(serde_json::json!({ "issue": " I0001 " })) {
            IssueStanding::Resolved { id, .. } => assert_eq!(id, "I0001"),
            other => panic!("the plan holds I0001, and it answered {other:?}"),
        }
        assert_eq!(
            ask(serde_json::json!({ "issue": "I9999" })),
            IssueStanding::Unresolved { id: "I9999".into() }
        );

        // no issue: what `start` was told instead is named, and never judged
        assert_eq!(
            ask(serde_json::json!({ "exemption": "chore", "exemption_because": "a typo" })),
            IssueStanding::DeclaredNone("exempt as chore: a typo".into())
        );
        assert_eq!(
            ask(serde_json::json!({ "exemption": "chore", "intent": "ship-it" })),
            IssueStanding::DeclaredNone("exempt as chore".into()),
            "an exemption is stated before an intent"
        );
        assert_eq!(
            ask(serde_json::json!({ "intent": "ship-it" })),
            IssueStanding::DeclaredNone("it serves intent ship-it, named directly".into())
        );
        assert_eq!(ask(serde_json::json!({})), IssueStanding::Undeclared);
    }

    #[test]
    fn the_handover_standing_is_the_newest_record_that_names_this_task() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mine = task(serde_json::json!({}));
        assert_eq!(
            handover_standing(root, None),
            HandoverStanding::Unknown("no task is active in this checkout".into())
        );
        assert_eq!(
            handover_standing(root, Some(&mine)),
            HandoverStanding::Absent,
            "no store is no handover"
        );

        let store = root.join(gates::STATE_DIR).join("handovers");
        std::fs::create_dir_all(&store).unwrap();
        let record = |name: &str, front: &str| std::fs::write(store.join(name), front).unwrap();
        record(
            "2026-09-03-other.md",
            "---\ntask_id: t-2\n---\n# Objective\n",
        );
        record("2026-09-04-notes.txt", "---\ntask_id: t-1\n---\n");
        record(
            "2026-09-05-late.md",
            &format!("{}task_id: t-1\n", "\n".repeat(20)),
        );
        assert_eq!(
            handover_standing(root, Some(&mine)),
            HandoverStanding::Absent,
            "another task's record, a file that is not a record, a mention below the front matter"
        );

        record(
            "2026-09-01-first.md",
            "---\ntask_id: t-1\n---\n# Objective\n",
        );
        record(
            "2026-09-02-second.md",
            "---\n  task_id: t-1\n---\n# Objective\n",
        );
        assert_eq!(
            handover_standing(root, Some(&mine)),
            HandoverStanding::Present("2026-09-02-second.md".into())
        );
    }

    // ------------------------------------------------------------ the deployment plan

    /// The plan's facts are read from what the repository declares: the site's origin, the
    /// model's path classes, the newest release record, and each deployment object with the
    /// status it states. An object that does not parse is not a target.
    #[test]
    fn the_deployment_plan_is_derived_from_the_records_the_repository_holds() {
        use crate::deploy::targets::DeploymentTarget;
        let repo = SyntheticRepository::small().unwrap();
        std::fs::create_dir_all(repo.root().join("site")).unwrap();
        std::fs::write(
            repo.root().join("site/config.toml"),
            "base_url = \"https://majordomus.test/\"\n",
        )
        .unwrap();
        let ctx = context_over(
            &repo,
            &crate_share(),
            vec![
                release_record(),
                deployment("live", Some("active"), Some("https://app.test/")),
                deployment("old", Some("retired"), Some("https://old.test")),
                deployment("planned", None, None),
                object(
                    crate::deploy::KIND,
                    "broken",
                    serde_json::json!({ "id": "broken" }),
                ),
            ],
            super::super::modules(),
        );
        let m: model::GateModel = crate::metadata::yaml::parse_into(MODEL).unwrap();
        let changed = vec!["apps/majordomus-cli/src/lib.rs".to_string()];

        let plan = deployment_plan(&ctx, Some(&m), &changed, Some("def".into()), false);
        let ids: Vec<&str> = plan.targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["pages", "release", "live", "old", "planned"]);
        let by = |id: &str| -> DeploymentTarget {
            plan.targets.iter().find(|t| t.id == id).unwrap().clone()
        };

        assert!(!by("pages").applicable, "{}", by("pages").reason);
        assert_eq!(
            by("pages").inputs,
            ["docs/**"],
            "the site-build gate's classes"
        );
        assert_eq!(by("pages").expected.commit.as_deref(), Some("def"));
        assert!(by("release").applicable, "{}", by("release").reason);
        assert_eq!(
            by("release").expected,
            DeploymentIdentity {
                commit: Some("abc".into()),
                version: Some("0.5.0".into()),
                tag: Some("v0.5.0".into()),
            },
            "the newest stable record"
        );
        assert!(by("live").applicable, "{}", by("live").reason);
        assert_eq!(
            by("live").inputs,
            ["apps/majordomus-cli", "apps/majordomus-cli/**"],
            "a build input selects itself and everything below it"
        );
        assert!(
            by("old").reason.contains("`retired`"),
            "{}",
            by("old").reason
        );
        assert!(
            by("planned").reason.contains("`declared`"),
            "{}",
            by("planned").reason
        );

        // without a model nothing says what the site is built from, and the plan says so
        let unmodelled = deployment_plan(&ctx, None, &changed, None, false);
        let pages = unmodelled.targets.iter().find(|t| t.id == "pages").unwrap();
        assert!(!pages.applicable);
        assert!(
            pages.reason.contains("declares no site-build gate"),
            "{}",
            pages.reason
        );
    }

    // ------------------------------------------------------------ the report

    /// A gate's evidence is taken over the files its classes select: the report carries the
    /// hash of exactly those files, everything for a gate every plan selects, and nothing
    /// for a gate no class names.
    #[test]
    fn each_gate_is_reported_with_the_hash_of_the_files_that_select_it() {
        let repo = SyntheticRepository::small().unwrap();
        declare_model(&repo);
        let root = repo.root().to_path_buf();
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.name", "A Worker"]);
        git(&root, &["config", "user.email", "worker@example.test"]);
        git(&root, &["config", "commit.gpgsign", "false"]);
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "the first commit"]);

        let ctx = context_over(&repo, &crate_share(), vec![], super::super::modules());
        let input = CompletionInput {
            changed: Some(vec!["docs/DOC_0.md".into()]),
            ..Default::default()
        };
        let c = gates_completion(&ctx, input).expect("the completion report answers");
        let gate = |id: &str| c.gates.iter().find(|g| g.id == id).unwrap().clone();

        let (docs, files) =
            super::super::obligations::inputs_hash(&root, &["docs/**".to_string()]).unwrap();
        assert!(
            files > 0 && !docs.is_empty(),
            "the fixture tracks documents"
        );
        assert_eq!(gate("site-build").inputs_hash, docs);
        assert!(gate("site-build").required, "the change is a document");
        assert_eq!(
            gate("version-surface").inputs_hash,
            "",
            "it tracks nothing under apps/"
        );
        let (everything, _) =
            super::super::obligations::inputs_hash(&root, &["*".to_string()]).unwrap();
        assert_eq!(
            gate("projection-closure").inputs_hash,
            everything,
            "a gate every plan selects is one any change can invalidate"
        );
        assert_ne!(everything, docs);
        assert_eq!(gate("on-request").inputs_hash, "", "no class selects it");
        assert!(
            !c.findings
                .iter()
                .any(|f| f.contains("no readable CI model")),
            "{:?}",
            c.findings
        );
    }
}
