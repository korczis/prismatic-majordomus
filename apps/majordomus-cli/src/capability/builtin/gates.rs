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
use crate::deploy::targets::{self, ApplicationFact, DeploymentPlan, Identity, PlanFacts};
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
        return IssueStanding::Undeclared;
    }
    if id == "none" {
        return IssueStanding::DeclaredNone;
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
            r.latest_stable().map(|rel| Identity {
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
}
