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
//! assert_eq!(ids, ["gates.model", "gates.completion"]);
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
use crate::gates::{self, judge, model, Completion};
use crate::{capability, module};

use super::get;
use super::obligations::Obligation;
use super::views::Empty;

/// The URI under which the CI model is read as an MCP resource.
pub const GATES_URI: &str = "majordomus://gates";

/// The URI under which this checkout's completion state is read as an MCP resource.
pub const COMPLETION_URI: &str = "majordomus://gates/completion";

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
    // judgement so that a test drives the same function with hashes of its own.
    let mut hashes: BTreeMap<String, Option<String>> = BTreeMap::new();
    for gate in &m.gates {
        let specs = m.inputs_of(&gate.id);
        let hash = super::obligations::inputs_hash(&root, &specs).map(|(h, _)| h);
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
    match ctx.execute("obligations.closure", serde_json::json!({})) {
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

    Ok(gates::complete(
        &m,
        &changed,
        task.as_ref(),
        &vocab,
        &runs,
        &hashes,
        &standing,
        closure_reachable,
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
/// assert_eq!(routes, ["/api/v1/gates", "/api/v1/gates/completion"]);
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
        assert_eq!(m.capabilities.len(), 2);
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
