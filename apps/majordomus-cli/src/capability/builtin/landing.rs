//! The `landing` module: what is preventing this repository from being landed and
//! delivered, right now.
//!
//! # The question this answers, and the one it does not
//!
//! [`super::obligations`] and [`super::gates`] answer about a *task*: what this piece of
//! work still owes, and whether the gates its own change set selects have reported. That is
//! the completion doctrine of ADR 0030 and of the `gates` module, and it is scoped to one
//! worker's change.
//!
//! Nothing answered about the *repository*. A repository can hold twelve tasks that each
//! closed cleanly and still be nowhere near delivered: work committed in a worktree nobody
//! pushed, a branch the trunk never reached, a derived projection stale since a merge, a
//! declared deployment that would refuse, a release whose artifacts were never published.
//! Every one of those facts already had an engine that knew it. None of them had a verdict.
//!
//! # Every stage names the engine that decided it
//!
//! This module measures nothing. Each stage is one question put to the capability that
//! already owns the answer, executed through the same executor MCP and the HTTP routes use,
//! and translated once into the status vocabulary [`crate::gates::GateStatus`] already
//! declares:
//!
//! | stage | decided by |
//! |---|---|
//! | implementation | `plan.status` |
//! | tests | `health.report`, the benchmark-coverage check |
//! | documentation | `health.report`, the layer and scope checks |
//! | generated | `artifacts.list`, reconciled with the working tree |
//! | commit | `worktree.topology`, each worktree's uncommitted work |
//! | push | `worktree.topology`, each branch's upstream |
//! | integration | `worktree.topology`, reachability from the trunk |
//! | ci | `gates.completion`, else `gates.model` |
//! | deployment | `deploy.check` |
//! | published | `distribution.status` |
//! | project | `plan.validate` |
//! | cleanup | `worktree.topology`, its own diagnostics |
//!
//! A stage whose source could not be executed is [`GateStatus::Unknown`] carrying the
//! reason, and a stage whose source has nothing recorded is [`GateStatus::Queued`]. Neither
//! is ever `pass`: this report exists because "nothing said no" was being read as delivery.
//!
//! # Nothing here is a second model
//!
//! There is no table of stages with their own thresholds, no second reading of git, no
//! second CI model and no second deployment contract. Adding a stage means naming a
//! capability that answers it; a stage this repository cannot answer is reported as such
//! rather than given a rule of its own. ADR 0045.

//!
//! ```
//! use majordomus_cli::capability::builtin::landing::module;
//!
//! // one declaration, and every surface is a projection of it
//! let m = module();
//! let c = &m.capabilities[0].capability;
//! assert_eq!(c.id.as_str(), "landing.closure");
//! assert_eq!(c.exposure.http.as_ref().unwrap().path, "/api/v1/landing");
//! assert_eq!(c.exposure.cli.as_ref().unwrap().path, vec!["landing".to_string()]);
//! assert!(c.exposure.mcp.is_some());
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::gates::GateStatus;
use crate::{capability, module};

use super::get;
use super::views::Empty;

/// The URI under which the closure is read as an MCP resource.
pub const LANDING_URI: &str = "majordomus://landing";

/// The schema this document is published under.
pub const SCHEMA: &str = "majordomus/landing-closure/v1";

// ---------------------------------------------------------------- the document

/// ```
/// use majordomus_cli::capability::builtin::landing::LandingStage;
/// use majordomus_cli::gates::GateStatus;
///
/// let stage = LandingStage {
///     id: "push".into(),
///     question: "Do the commits of every branch reach a remote?".into(),
///     status: GateStatus::Fail,
///     evidence: "2 of 9 branch(es) hold commits no remote has".into(),
///     source: "worktree.topology".into(),
///     remediation: "git push -u origin <branch>".into(),
///     findings: vec!["feature/x — 3 commit(s) not pushed".into()],
/// };
/// // a stage that refuses names the engine that decided it, what that engine read, and the
/// // individual things in the way: a verdict a reader cannot act on is the "not ready" this
/// // type exists instead of
/// assert!(stage.status.refuses());
/// assert!(!stage.source.is_empty() && !stage.remediation.is_empty());
/// assert_eq!(stage.findings.len(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One stage of delivery, and what the engine that owns it said.
pub struct LandingStage {
    /// The stage's identity, stable across reports.
    pub id: String,
    /// The question, as a person would ask it.
    pub question: String,
    /// The answer, in the vocabulary every gate of this repository is reported in.
    pub status: GateStatus,
    /// What was actually read. Never a restatement of the status.
    pub evidence: String,
    /// The capability that decided it.
    pub source: String,
    /// What would settle it, as a command. Specific to this stage and this repository.
    pub remediation: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The individual things standing in the way, each named.
    pub findings: Vec<String>,
}

/// ```
/// use majordomus_cli::capability::builtin::landing::LandingClosure;
///
/// let c: LandingClosure = serde_json::from_value(serde_json::json!({
///     "schema": "majordomus/landing-closure/v1",
///     "landed": false,
///     "verdict": "NOT LANDED — 0 stage(s) refusing, 1 unverified, 11 clear, of 12",
///     "stages": [],
///     "tallies": { "pass": 11, "queued": 1 },
///     "blocking": [],
///     "unverified": ["published"],
///     "at": "2026-01-01T00:00:00Z"
/// }))
/// .unwrap();
/// // nothing refuses and the repository is still not landed: a stage nothing answered is
/// // not a stage that passed, which is the whole reason this document exists
/// assert!(!c.landed);
/// assert!(c.blocking.is_empty());
/// assert_eq!(c.unverified, ["published"]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// Whether this repository is landed, and if not, what is holding it.
pub struct LandingClosure {
    /// [`SCHEMA`].
    pub schema: String,
    /// The commit the verdict was taken over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// The branch it was taken from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The trunk this repository lands on, as the topology decided it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
    /// True only when every stage passed or was exempt. A single unknown keeps it false:
    /// a verdict that has not arrived is not a verdict that said yes.
    pub landed: bool,
    /// The verdict in one line, naming the counts it is made of.
    pub verdict: String,
    /// Every stage, in the order delivery happens in.
    pub stages: Vec<LandingStage>,
    /// How many stages stand where, by status word.
    pub tallies: BTreeMap<String, usize>,
    /// The stages known to be wrong.
    pub blocking: Vec<String>,
    /// The stages nothing answered.
    pub unverified: Vec<String>,
    /// When the verdict was taken.
    pub at: String,
}

// ---------------------------------------------------------------- asking the owners

/// One capability's answer, or why it could not be had. A stage whose source refused is
/// reported with the refusal; nothing here turns a missing answer into a passing one.
type Answer = Result<Value, String>;

fn ask(ctx: &Context, id: &str, input: Value) -> Answer {
    ctx.execute(id, input)
        .map_err(|e| format!("{id} could not be executed: {e}"))
}

/// Build one stage from the answer its owner gave.
///
/// `read` returns the verdict, the evidence and the individual findings; returning `None`
/// means the answer arrived in a shape this reader does not understand, which is an
/// unknown and never a pass.
fn stage(
    id: &str,
    question: &str,
    source: &str,
    remediation: &str,
    answer: &Answer,
    read: impl FnOnce(&Value) -> Option<(GateStatus, String, Vec<String>)>,
) -> LandingStage {
    let (status, evidence, findings) = match answer {
        Err(why) => (GateStatus::Unknown, why.clone(), Vec::new()),
        Ok(value) => read(value).unwrap_or_else(|| {
            (
                GateStatus::Unknown,
                format!("{source} answered a document this reader cannot understand"),
                Vec::new(),
            )
        }),
    };
    LandingStage {
        id: id.to_string(),
        question: question.to_string(),
        status,
        evidence,
        source: source.to_string(),
        remediation: remediation.to_string(),
        findings,
    }
}

/// One named check of `health.report`, by id.
fn health_check<'a>(health: &'a Value, id: &str) -> Option<&'a Value> {
    health
        .get("checks")?
        .as_array()?
        .iter()
        .find(|c| c.get("id").and_then(Value::as_str) == Some(id))
}

/// Translate a health check's own word. `health.report` decides; this never re-decides.
fn from_health(health: &Value, ids: &[&str]) -> Option<(GateStatus, String, Vec<String>)> {
    let mut worst = GateStatus::Pass;
    let mut detail = Vec::new();
    let mut findings = Vec::new();
    let mut seen = false;
    for id in ids {
        let Some(check) = health_check(health, id) else {
            continue;
        };
        seen = true;
        let status = check.get("status").and_then(Value::as_str).unwrap_or("");
        let said = check.get("detail").and_then(Value::as_str).unwrap_or("");
        detail.push(format!("{id}: {said}"));
        let here = match status {
            "ok" => GateStatus::Pass,
            "warn" => GateStatus::Queued,
            "fail" => GateStatus::Fail,
            _ => GateStatus::Unknown,
        };
        if here.refuses() || (here.unverified() && !worst.refuses()) {
            worst = here;
        }
        for f in check
            .get("findings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            findings.push(format!("{id}: {f}"));
        }
    }
    if !seen {
        return None;
    }
    Some((worst, detail.join("; "), findings))
}

// ---------------------------------------------------------------- the stages

/// Every stage, in the order delivery happens in, each answered by its owner.
///
/// The argument list is the point: these are documents other capabilities produced, and
/// this function composes them. It measures nothing and reads no file.
fn stages(
    plan_status: &Answer,
    plan_validation: &Answer,
    health: &Answer,
    artifacts: &Answer,
    topology: &Answer,
    ci: &Answer,
    ci_source: &str,
    deployment: &Answer,
    distribution: &Answer,
) -> Vec<LandingStage> {
    vec![
        stage(
            "implementation",
            "Is every issue the plan declares delivered?",
            "plan.status",
            "majordomus plan next",
            plan_status,
            |v| {
                let counts = v.get("counts")?;
                let total = counts.get("total")?.as_u64()?;
                let by: &serde_json::Map<String, Value> = counts.get("by_status")?.as_object()?;
                // the plan declares its own status vocabulary and its own casing; only the
                // two words that mean "nothing is owed here" are read, and case is not one of
                // the things this report is entitled to an opinion about
                let settled =
                    |k: &str| matches!(k.to_ascii_lowercase().as_str(), "done" | "cancelled");
                let done = by
                    .iter()
                    .find(|(k, _)| k.to_ascii_lowercase() == "done")
                    .and_then(|(_, n)| n.as_u64())
                    .unwrap_or(0);
                let open: Vec<String> = by
                    .iter()
                    .filter(|(k, _)| !settled(k))
                    .filter(|(_, n)| n.as_u64().unwrap_or(0) > 0)
                    .map(|(k, n)| format!("{n} {k}"))
                    .collect();
                let status = if total == 0 {
                    GateStatus::Exempt
                } else if open.is_empty() {
                    GateStatus::Pass
                } else {
                    GateStatus::Queued
                };
                Some((
                    status,
                    format!("{done} of {total} issue(s) done"),
                    open.iter().map(|s| format!("{s} issue(s)")).collect(),
                ))
            },
        ),
        stage(
            "tests",
            "Is every target the benchmark projection declares covered by a case that runs it?",
            "health.report (benchmark-coverage)",
            "majordomus bench coverage --check",
            health,
            |v| from_health(v, &["benchmark-coverage"]),
        ),
        stage(
            "documentation",
            "Does every document of the layer parse, resolve and stay in scope?",
            "health.report (layer, scope)",
            "majordomus objects verify",
            health,
            |v| from_health(v, &["layer", "scope"]),
        ),
        stage(
            "generated",
            "Is every derived artifact current with its canonical source?",
            "artifacts.list",
            "majordomus generate && scripts/derive",
            artifacts,
            |v| {
                let t = v.get("tallies")?;
                let stale = t.get("stale")?.as_u64()?;
                let missing = t.get("missing").and_then(Value::as_u64).unwrap_or(0);
                let current = t.get("current").and_then(Value::as_u64).unwrap_or(0);
                let names: Vec<String> = v
                    .get("files")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|f| {
                        !matches!(
                            f.get("state").and_then(Value::as_str),
                            Some("current") | None
                        )
                    })
                    .filter_map(|f| {
                        Some(format!(
                            "{} ({})",
                            f.get("path")?.as_str()?,
                            f.get("state")?.as_str()?
                        ))
                    })
                    .collect();
                let status = if stale + missing == 0 {
                    GateStatus::Pass
                } else {
                    GateStatus::Stale
                };
                Some((
                    status,
                    format!("{current} artifact(s) current, {stale} stale, {missing} missing"),
                    names,
                ))
            },
        ),
        stage(
            "commit",
            "Is every worktree's work in the history rather than on the disk?",
            "worktree.topology",
            "git -C <worktree> add -A && git -C <worktree> commit",
            topology,
            |v| {
                let mut findings = Vec::new();
                let mut trees = 0usize;
                let mut ephemeral = 0usize;
                for w in v.get("worktrees")?.as_array()? {
                    // The topology's own two exceptions, in its own words: an `ephemeral`
                    // checkout is a scratch tree a test or an orchestrator made, and a
                    // `detached` one holds no branch, so nothing committed there reaches
                    // anything. Both are outside what `worktree migrate` undertakes to place,
                    // and counting their working trees would make this verdict depend on
                    // whether a suite happened to be running. They are the `cleanup` stage's,
                    // where the topology already reports them as diagnostics.
                    let scratch = w.get("standing").and_then(Value::as_str) == Some("ephemeral")
                        || w.get("detached").and_then(Value::as_bool) == Some(true);
                    if scratch {
                        ephemeral += 1;
                        continue;
                    }
                    trees += 1;
                    let Some(dirty) = w.get("dirty") else {
                        continue;
                    };
                    if dirty.get("clean").and_then(Value::as_bool) == Some(true) {
                        continue;
                    }
                    let counts: Vec<String> = ["staged", "unstaged", "untracked", "conflicted"]
                        .iter()
                        .filter_map(|k| {
                            let n = dirty.get(*k)?.as_u64()?;
                            (n > 0).then(|| format!("{n} {k}"))
                        })
                        .collect();
                    findings.push(format!(
                        "{} — {}",
                        w.get("path").and_then(Value::as_str).unwrap_or("?"),
                        counts.join(", ")
                    ));
                }
                let status = if findings.is_empty() {
                    GateStatus::Pass
                } else {
                    GateStatus::Fail
                };
                Some((
                    status,
                    format!(
                        "{} of {trees} worktree(s) hold uncommitted work; {ephemeral} scratch or detached checkout(s) not counted",
                        findings.len()
                    ),
                    findings,
                ))
            },
        ),
        stage(
            "push",
            "Do the commits of every branch reach a remote?",
            "worktree.topology",
            "git push -u origin <branch>",
            topology,
            |v| {
                let mut findings = Vec::new();
                let mut branches = 0usize;
                for b in v.get("branches")?.as_array()? {
                    branches += 1;
                    let name = b.get("name").and_then(Value::as_str).unwrap_or("?");
                    match b.get("upstream") {
                        None | Some(Value::Null) => findings.push(format!("{name} — no upstream")),
                        Some(u) => {
                            if u.get("gone").and_then(Value::as_bool) == Some(true) {
                                findings.push(format!(
                                    "{name} — upstream {} is gone",
                                    u.get("name").and_then(Value::as_str).unwrap_or("?")
                                ));
                            } else if let Some(ahead) = u.get("ahead").and_then(Value::as_u64) {
                                if ahead > 0 {
                                    findings.push(format!("{name} — {ahead} commit(s) not pushed"));
                                }
                            }
                        }
                    }
                }
                let status = if findings.is_empty() {
                    GateStatus::Pass
                } else {
                    GateStatus::Fail
                };
                Some((
                    status,
                    format!(
                        "{} of {branches} branch(es) hold commits no remote has",
                        findings.len()
                    ),
                    findings,
                ))
            },
        ),
        stage(
            "integration",
            "Does the trunk reach every branch?",
            "worktree.topology",
            "open a pull request for <branch> and land it",
            topology,
            |v| {
                let mut findings = Vec::new();
                let mut judged = 0usize;
                let mut unknown = 0usize;
                for b in v.get("branches")?.as_array()? {
                    if b.get("trunk").and_then(Value::as_bool) == Some(true) {
                        continue;
                    }
                    let name = b.get("name").and_then(Value::as_str).unwrap_or("?");
                    match b.get("merged_into_trunk").and_then(Value::as_bool) {
                        Some(true) => judged += 1,
                        Some(false) => {
                            judged += 1;
                            findings.push(format!("{name} — the trunk does not reach it"));
                        }
                        None => unknown += 1,
                    }
                }
                let status = if unknown > 0 && findings.is_empty() {
                    GateStatus::Unknown
                } else if findings.is_empty() {
                    GateStatus::Pass
                } else {
                    GateStatus::Fail
                };
                Some((
                    status,
                    format!(
                        "{} of {judged} judged branch(es) are not on the trunk; {unknown} could not be judged",
                        findings.len()
                    ),
                    findings,
                ))
            },
        ),
        stage(
            "ci",
            "Has every gate of the CI model reported over this tree, and did it pass?",
            ci_source,
            "majordomus evidence --gate <id> --exit <status>",
            ci,
            |v| {
                // gates.completion answered: its own verdict, translated once
                if let Some(gates) = v.get("gates").and_then(Value::as_array) {
                    let mut refusing = Vec::new();
                    let mut silent = Vec::new();
                    let mut passing = 0usize;
                    for g in gates
                        .iter()
                        .filter(|g| g.get("required").and_then(Value::as_bool) == Some(true))
                    {
                        let id = g.get("id").and_then(Value::as_str).unwrap_or("?");
                        match g.get("status").and_then(Value::as_str) {
                            Some("pass") => passing += 1,
                            Some("exempt") => {}
                            Some(w @ ("fail" | "stale" | "blocked")) => refusing.push(format!(
                                "{id} — {w}: {}",
                                g.get("reason").and_then(Value::as_str).unwrap_or("")
                            )),
                            Some(w) => silent.push(format!("{id} — {w}")),
                            None => silent.push(format!("{id} — no status")),
                        }
                    }
                    let status = if !refusing.is_empty() {
                        GateStatus::Fail
                    } else if !silent.is_empty() {
                        GateStatus::Queued
                    } else {
                        GateStatus::Pass
                    };
                    let mut findings = refusing;
                    findings.extend(silent);
                    return Some((
                        status,
                        format!("{passing} required gate(s) passing over this tree"),
                        findings,
                    ));
                }
                // only the model could be read: the gates exist and none has reported here
                let count = v.get("count")?.as_u64()?;
                let on_demand: Vec<String> = v
                    .get("on_demand")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|s| format!("{s} — no routine plan selects it"))
                    .collect();
                Some((
                    GateStatus::Queued,
                    format!(
                        "the CI model declares {count} gate(s) and no run is recorded against \
                         this tree; the trunk's own verdict is a fact of the forge, not of this \
                         checkout"
                    ),
                    on_demand,
                ))
            },
        ),
        stage(
            "deployment",
            "Would every deployment this repository declares work?",
            "deploy.check",
            "majordomus deploy check",
            deployment,
            |v| {
                let decided = v.get("decided")?.as_u64()?;
                let refusals: Vec<String> = v
                    .get("refusals")?
                    .as_array()?
                    .iter()
                    .filter_map(|r| {
                        Some(format!(
                            "{}:{} — {}",
                            r.get("file")?.as_str()?,
                            r.get("key")?.as_str()?,
                            r.get("problem")?.as_str()?
                        ))
                    })
                    .collect();
                let status = if decided == 0 {
                    GateStatus::Exempt
                } else if refusals.is_empty() {
                    GateStatus::Pass
                } else {
                    GateStatus::Fail
                };
                Some((
                    status,
                    format!(
                        "{decided} deployment(s) decided, {} refusing",
                        refusals.len()
                    ),
                    refusals,
                ))
            },
        ),
        stage(
            "published",
            "Does the advertised installation serve what this tree would release?",
            "distribution.status",
            "scripts/release-record && gh workflow run release.yml",
            distribution,
            |v| {
                let installable = v.get("installable")?.as_bool()?;
                let summary = v.get("summary")?.as_str()?.to_string();
                let mut unknown = false;
                let findings: Vec<String> = v
                    .get("checks")?
                    .as_array()?
                    .iter()
                    .filter(|c| {
                        !matches!(c.get("state").and_then(Value::as_str), Some("ok") | None)
                    })
                    .filter_map(|c| {
                        if c.get("state").and_then(Value::as_str) == Some("unknown") {
                            unknown = true;
                        }
                        Some(format!(
                            "{} — {}",
                            c.get("id")?.as_str()?,
                            c.get("cause").and_then(Value::as_str).unwrap_or_else(|| c
                                .get("observed")
                                .and_then(Value::as_str)
                                .unwrap_or(""))
                        ))
                    })
                    .collect();
                let status = if !installable {
                    GateStatus::Fail
                } else if unknown {
                    // the local half is complete and the public half was not looked at. That
                    // is a debt somebody owes, and calling it `pass` is exactly the
                    // substitution this report exists to refuse.
                    GateStatus::Queued
                } else {
                    GateStatus::Pass
                };
                Some((status, summary, findings))
            },
        ),
        stage(
            "project",
            "Does the project model the work is recorded against validate?",
            "plan.validate",
            "majordomus plan validate",
            plan_validation,
            |v| {
                let valid = v.get("valid")?.as_bool()?;
                let issues = v.get("issues")?.as_u64()?;
                let warnings = v.get("warnings").and_then(Value::as_u64).unwrap_or(0);
                let findings: Vec<String> = v
                    .get("findings")?
                    .as_array()?
                    .iter()
                    .filter_map(|f| {
                        Some(format!(
                            "{} {} {} — {}",
                            f.get("level")?.as_str()?,
                            f.get("code")?.as_str()?,
                            f.get("subject")?.as_str()?,
                            f.get("message")?.as_str()?
                        ))
                    })
                    .collect();
                let status = if !valid {
                    GateStatus::Fail
                } else if warnings > 0 {
                    GateStatus::Queued
                } else {
                    GateStatus::Pass
                };
                Some((
                    status,
                    format!("{issues} issue(s) modelled; {warnings} warning(s)"),
                    findings,
                ))
            },
        ),
        stage(
            "cleanup",
            "Is a worktree or a branch left behind?",
            "worktree.topology",
            "majordomus worktree migrate && git branch -d <branch>",
            topology,
            |v| {
                let t = v.get("tallies")?;
                let n = |k: &str| t.get(k).and_then(Value::as_u64).unwrap_or(0);
                // the topology grades its own diagnostics; only the two severities it
                // refuses on are things left behind. `info` is the topology saying what a
                // state is — a detached checkout has no canonical path — and nineteen copies
                // of that would bury the one error that matters.
                let mut noted = 0usize;
                let mut findings: Vec<String> = v
                    .get("diagnostics")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|d| {
                        let severity = d.get("severity")?.as_str()?;
                        if !matches!(severity, "error" | "warning" | "warn") {
                            noted += 1;
                            return None;
                        }
                        Some(format!(
                            "{severity} {} {} — {}",
                            d.get("code")?.as_str()?,
                            d.get("path")
                                .or_else(|| d.get("branch"))
                                .and_then(Value::as_str)
                                .unwrap_or("(repository)"),
                            d.get("message")?.as_str()?
                        ))
                    })
                    .collect();
                let eligible = n("cleanup_eligible");
                if eligible > 0 {
                    findings.push(format!(
                        "{eligible} branch(es) are merged into the trunk and still exist"
                    ));
                }
                let status = if n("errors") > 0 {
                    GateStatus::Fail
                } else if findings.is_empty() {
                    GateStatus::Pass
                } else {
                    GateStatus::Queued
                };
                Some((
                    status,
                    format!(
                        "{} worktree(s): {} misplaced, {} missing, {} detached; {} branch(es), {eligible} eligible for cleanup; {noted} note(s) the topology does not refuse on",
                        n("worktrees"),
                        n("misplaced"),
                        n("missing"),
                        n("detached"),
                        n("branches"),
                    ),
                    findings,
                ))
            },
        ),
    ]
}

// ---------------------------------------------------------------- the verdict

/// Fold the stages into one verdict.
///
/// `landed` is true only when nothing refuses **and** nothing is unverified. A stage whose
/// owner could not be asked leaves the repository unlanded, because the alternative is
/// reading silence as delivery — which is the failure this whole module exists against.
fn close(
    stages: Vec<LandingStage>,
    head: Option<String>,
    branch: Option<String>,
    trunk: Option<String>,
    at: String,
) -> LandingClosure {
    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    let mut blocking = Vec::new();
    let mut unverified = Vec::new();
    for s in &stages {
        *tallies.entry(s.status.as_str().to_string()).or_insert(0) += 1;
        if s.status.refuses() {
            blocking.push(s.id.clone());
        } else if s.status.unverified() {
            unverified.push(s.id.clone());
        }
    }
    let landed = blocking.is_empty() && unverified.is_empty();
    let passing = tallies.get("pass").copied().unwrap_or(0);
    let verdict = if landed {
        format!(
            "LANDED — every one of the {} stage(s) of delivery is answered and clear",
            stages.len()
        )
    } else {
        format!(
            "NOT LANDED — {} stage(s) refusing, {} unverified, {passing} clear, of {}",
            blocking.len(),
            unverified.len(),
            stages.len()
        )
    };
    LandingClosure {
        schema: SCHEMA.to_string(),
        head,
        branch,
        trunk,
        landed,
        verdict,
        stages,
        tallies,
        blocking,
        unverified,
        at,
    }
}

fn landing_closure(ctx: &Context, _: Empty) -> Result<LandingClosure, CapabilityError> {
    let topology = ask(ctx, "worktree.topology", json!({}));
    let health = ask(ctx, "health.report", json!({}));
    let artifacts = ask(ctx, "artifacts.list", json!({}));
    let plan_status = ask(ctx, "plan.status", json!({}));
    let plan_validation = ask(ctx, "plan.validate", json!({}));
    let deployment = ask(ctx, "deploy.check", json!({}));
    let distribution = ask(ctx, "distribution.status", json!({}));

    // The CI stage is the `gates` module's, asked of the trunk rather than of a task's
    // change set. When this checkout holds no task the completion question cannot be put at
    // all, and the model alone is what remains readable — which is reported as a debt and
    // never as a pass.
    let trunk = topology
        .as_ref()
        .ok()
        .and_then(|t| t.get("trunk")?.get("branch")?.as_str().map(str::to_string));
    let mut ci_source = "gates.completion";
    let mut ci = ask(
        ctx,
        "gates.completion",
        match &trunk {
            Some(b) => json!({ "base": b }),
            None => json!({}),
        },
    );
    if ci.is_err() {
        ci_source = "gates.model";
        ci = ask(ctx, "gates.model", json!({}));
    }

    let (head, branch) = match &ctx.index.repository.git {
        crate::git::GitState::Available(info) => (info.head.clone(), info.branch.clone()),
        crate::git::GitState::Unavailable { .. } => (None, None),
    };

    Ok(close(
        stages(
            &plan_status,
            &plan_validation,
            &health,
            &artifacts,
            &topology,
            &ci,
            ci_source,
            &deployment,
            &distribution,
        ),
        head,
        branch,
        trunk,
        crate::peers::rfc3339(std::time::SystemTime::now()),
    ))
}

// ---------------------------------------------------------------- the module

/// The `landing` module: one capability, every projection.
///
/// ```
/// use majordomus_cli::capability::builtin::landing::module;
/// let m = module();
/// assert_eq!(m.id.as_str(), "landing");
/// let c = &m.capabilities[0].capability;
/// assert_eq!(c.id.as_str(), "landing.closure");
/// assert_eq!(c.exposure.http.as_ref().unwrap().path, "/api/v1/landing");
/// assert_eq!(c.exposure.cli.as_ref().unwrap().path, vec!["landing".to_string()]);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "landing",
        title: "Landing closure",
        description: "What is preventing this repository from being completely landed and delivered, right now. Twelve stages of delivery — implementation, tests, documentation, generated projections, commit, push, integration, CI, deployment, publication, project model, workspace cleanup — each answered by the capability that already owns the fact, translated into one status vocabulary, and folded into one verdict. Nothing here measures anything of its own: a stage is a question put to `plan.status`, `health.report`, `artifacts.list`, `worktree.topology`, `gates.completion`, `deploy.check` or `distribution.status`, and a stage whose owner could not answer is reported as unknown rather than as clear.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "landing.closure",
                title: "What is preventing this repository from being landed",
                description: "Every stage of delivery with its verdict, the evidence that decided it, the capability that owns that evidence, the specific things standing in the way and the command that would settle each. `landed` is true only when every stage is answered and clear: a stage nothing answered leaves the repository unlanded, because absence of a refusal is not delivery.",
                input: Empty,
                output: LandingClosure,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_landing".into()),
                        resource: Some(McpResource { uri: LANDING_URI.into(), name: "landing".into() }),
                    }),
                    http: get("/api/v1/landing"),
                    cli: Some(CliExposure { path: vec!["landing".into()] }),
                },
                tags: ["landing", "completion", "delivery", "introspection"],
                cache: CachePolicy::Disabled,
                handler: landing_closure,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(id: &str, status: GateStatus) -> LandingStage {
        LandingStage {
            id: id.into(),
            question: "q".into(),
            status,
            evidence: "e".into(),
            source: "s".into(),
            remediation: "r".into(),
            findings: Vec::new(),
        }
    }

    #[test]
    fn an_unanswered_stage_leaves_the_repository_unlanded() {
        // the defect this module exists against: a stage nobody answered reading as clear
        let c = close(
            vec![ok("a", GateStatus::Pass), ok("b", GateStatus::Unknown)],
            None,
            None,
            None,
            "now".into(),
        );
        assert!(!c.landed, "one unknown stage is not a landed repository");
        assert_eq!(c.unverified, vec!["b".to_string()]);
        assert!(c.blocking.is_empty(), "unknown is a debt, not a refusal");
        assert!(c.verdict.contains("NOT LANDED"));
    }

    #[test]
    fn exempt_stages_do_not_hold_the_verdict() {
        let c = close(
            vec![ok("a", GateStatus::Pass), ok("b", GateStatus::Exempt)],
            None,
            None,
            None,
            "now".into(),
        );
        assert!(c.landed);
        assert_eq!(c.tallies.get("exempt"), Some(&1));
    }

    #[test]
    fn a_source_that_could_not_be_asked_is_unknown_and_says_why() {
        let s = stage(
            "x",
            "?",
            "plan.status",
            "majordomus plan",
            &Err("plan.status could not be executed: no plan".into()),
            |_| Some((GateStatus::Pass, String::new(), Vec::new())),
        );
        assert_eq!(s.status, GateStatus::Unknown);
        assert!(s.evidence.contains("no plan"), "the reason survives");
    }

    #[test]
    fn every_stage_of_the_invariant_is_present_and_ordered() {
        let unreachable = Err("not asked".to_string());
        let s = stages(
            &unreachable,
            &unreachable,
            &unreachable,
            &unreachable,
            &unreachable,
            &unreachable,
            "gates.model",
            &unreachable,
            &unreachable,
        );
        let ids: Vec<&str> = s.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "implementation",
                "tests",
                "documentation",
                "generated",
                "commit",
                "push",
                "integration",
                "ci",
                "deployment",
                "published",
                "project",
                "cleanup"
            ]
        );
        for stage in &s {
            assert!(
                !stage.remediation.is_empty(),
                "{} names no remedy",
                stage.id
            );
            assert!(!stage.source.is_empty(), "{} names no owner", stage.id);
            assert_eq!(
                stage.status,
                GateStatus::Unknown,
                "{}: an unreachable owner is never a pass",
                stage.id
            );
        }
    }
}
