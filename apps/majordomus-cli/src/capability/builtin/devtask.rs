//! The `devtask` module: an issue and a milestone read as executable development scopes.
//!
//! # Why this is a module and not two more `plan.*` capabilities
//!
//! Every capability is composed into exactly one Rust module, and a module's id is the
//! namespace of everything in it, so `plan.task` would have to live inside
//! `builtin/plan.rs`. It does not belong there: `plan` is the projection of
//! [`crate::plan`], which is held to byte equality with `lib/project.awk` by
//! `test/cases/99_plan_capabilities.sh`, and a capability that joins the plan to git and to
//! the session records has no counterpart in the awk to be equal to. Keeping them apart
//! keeps that equality checkable.
//!
//! What it must not be is a second project model, and it is not:
//!
//! * the records are the canonical ones, read through the index like every other object;
//! * the status, wave, dependents, edges and findings are [`crate::plan`]'s, unchanged —
//!   `the_status_never_disagrees_with_the_plan` is the assertion;
//! * the branches and commits are [`crate::worktree::trace`]'s, through the same [`Tracer`]
//!   `trace.issue` uses;
//! * the readiness vocabulary is a projection of the canonical statuses and refines three
//!   of them without replacing any — [`crate::devtask::readiness`] states which and why;
//! * the external projection stays `scripts/github-sync`'s. Nothing here writes, and
//!   nothing here restates its reconciliation vocabulary.
//!
//! [`Tracer`]: crate::worktree::trace::Tracer
//!
//! # Why the milestone graph does not touch git
//!
//! `devtask.milestone` is a pure function of the index, and deliberately: a graph whose
//! ordering depended on `git for-each-ref` would be a graph that differs between a full
//! clone and a shallow one, and this repository has already been bitten by a derivation
//! that was machine-dependent. `devtask.issue` consults git because a single issue's
//! branches are the question being asked; the graph does not, because they are not.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::devtask::task::{DevTaskModel, RecordRef, SessionRef};
use crate::devtask::{DevTask, MilestoneGraph};
use crate::index::Index;
use crate::plan::{Plan, ISSUE, MILESTONE};
use crate::worktree::trace::{IssueTrace, Tracer};
use crate::worktree::WorktreeError;
use crate::{capability, module};

use super::{get, mcp};

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which issue to read as a development task.
pub struct DevTaskInput {
    /// The issue id, as the canonical model spells it (`I0901`). An id the model does not
    /// declare is answered with `declared: false` and every canonical field `unknown`,
    /// never refused and never answered with empty strings that read as authored.
    pub issue: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Consult git for the branches and commits that realised it. Default: yes. Set it to
    /// false for a deterministic answer that depends on the records alone — which is what a
    /// generated document or a comparison across machines wants.
    pub git: Option<bool>,
}

impl BenchmarkCases for DevTaskInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // An issue of the repository the benchmark runs against, so the case measures the
        // derivation rather than the refusal. A hard-coded id would be a second declaration
        // of this repository's own plan.
        let issue = first(ctx.index, ISSUE).unwrap_or_else(|| "I0001".to_string());
        vec![
            NamedCase::new(
                "records-only",
                DevTaskInput {
                    issue: issue.clone(),
                    git: Some(false),
                },
            ),
            NamedCase::new(
                "with-git",
                DevTaskInput {
                    issue,
                    git: Some(true),
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which milestone to read as a dependency graph.
pub struct DevMilestoneInput {
    /// The milestone id, as the canonical model spells it. An id the model does not declare
    /// is answered with an empty graph and every field `unknown`, not refused.
    pub milestone: String,
}

impl BenchmarkCases for DevMilestoneInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let milestone = first(ctx.index, MILESTONE).unwrap_or_else(|| "M000".to_string());
        vec![NamedCase::new(
            "first-milestone",
            DevMilestoneInput { milestone },
        )]
    }
}

// ---------------------------------------------------------------- what the index holds

/// The first object of a kind, in URI order. A benchmark case's input, and nothing else: a
/// capability never picks an object for a caller.
fn first(index: &Index, kind: &str) -> Option<String> {
    index
        .objects
        .iter()
        .find(|o| o.kind == kind)
        .map(|o| o.identity.clone())
}

/// One canonical record of a kind, by identity: its repository-relative path and its parsed
/// metadata. The index has already discovered, parsed and schema-validated it, so this is
/// the model itself rather than a second reading of the directory it lives in.
fn record<'a>(index: &'a Index, kind: &str, identity: &str) -> Option<(&'a str, &'a Value)> {
    index
        .objects
        .iter()
        .find(|o| o.kind == kind && o.identity == identity)
        .map(|o| (o.provenance.path.as_str(), &o.metadata))
}

/// Every issue id the canonical model declares, in id order. What the tracer needs in order
/// to read an issue out of a branch name.
fn issue_ids(index: &Index) -> Vec<String> {
    let mut ids: Vec<String> = index
        .objects
        .iter()
        .filter(|o| o.kind == ISSUE)
        .map(|o| o.identity.clone())
        .collect();
    crate::order::canonical(&mut ids);
    ids
}

/// The session records that reach this issue, each carrying how strongly.
///
/// Two relations, and they are deliberately not merged. A record's own `issues` key is what
/// `share/schemas/majordomus/session-record` calls "the issues it moved, by id — derived
/// from the ledger's own events for this episode; never authored": a canonical link. Its
/// branch name is a rule about a string — [`crate::worktree::state::issue_of`]'s, not
/// restated here — which a rename can break. The first is derived, the second inferred, and
/// [`SessionRef::declares_issue`] is which, so that the field carrying each can state its
/// own provenance instead of one of them borrowing the other's.
fn sessions_of(index: &Index, issue: &str) -> Vec<SessionRef> {
    let ids = vec![issue.to_string()];
    let mut out: Vec<SessionRef> = index
        .objects
        .iter()
        .filter(|o| o.kind == "session")
        .filter_map(|o| {
            let branch = o
                .metadata
                .get("branch")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let declares_issue = o
                .metadata
                .get("issues")
                .and_then(Value::as_array)
                .is_some_and(|items| items.iter().any(|i| i.as_str() == Some(issue)));
            // a record that neither declares the issue nor carries a branch naming it is
            // not this issue's session, and is left out rather than reported weakly
            if !declares_issue && crate::worktree::state::issue_of(branch, &ids).is_none() {
                return None;
            }
            Some(SessionRef {
                session_id: o.identity.clone(),
                branch: branch.to_string(),
                created_at: o
                    .metadata
                    .get("created_at")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                path: o.provenance.path.clone(),
                declares_issue,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        crate::order::natural_cmp(&a.created_at, &b.created_at)
            .then_with(|| crate::order::natural_cmp(&a.session_id, &b.session_id))
    });
    out
}

/// The repository the external projection targets, as `project.yaml` declares it.
fn repository_of(plan: &Plan) -> Option<&str> {
    Some(plan.project.repository.as_str()).filter(|r| !r.is_empty())
}

fn refused(e: WorktreeError) -> CapabilityError {
    match e.exit_code() {
        crate::worktree::EXIT_MISSING => CapabilityError::NotFound(e.to_string()),
        crate::worktree::EXIT_REFUSED => CapabilityError::Refused(e.to_string()),
        _ => CapabilityError::Internal(e.to_string()),
    }
}

/// What git says about one issue, or nothing when git could not be opened at all.
///
/// A repository this process cannot read as a git repository is not an error here: the
/// canonical half of the answer is still the whole canonical half, and the execution group
/// reports `git_consulted: false` rather than an empty list that would read as "nothing
/// realised this issue".
fn trace_of(ctx: &Context, issue: &str) -> Result<Option<IssueTrace>, CapabilityError> {
    let tracer = match Tracer::open(Path::new(&ctx.index.repository.root)) {
        Ok(t) => t.with_issues(issue_ids(&ctx.index)),
        Err(e) if e.exit_code() == crate::worktree::EXIT_MISSING => return Ok(None),
        Err(e) => return Err(refused(e)),
    };
    tracer.trace(issue).map(Some).map_err(refused)
}

// ---------------------------------------------------------------- handlers

fn devtask_issue(ctx: &Context, input: DevTaskInput) -> Result<DevTask, CapabilityError> {
    let plan = Plan::build(&ctx.index);
    let trace = if input.git.unwrap_or(true) {
        trace_of(ctx, &input.issue)?
    } else {
        None
    };
    let record = record(&ctx.index, ISSUE, &input.issue);
    let model = DevTaskModel {
        plan: &plan,
        record: record.map(|(path, metadata)| RecordRef { path, metadata }),
        trace: trace.as_ref(),
        sessions: sessions_of(&ctx.index, &input.issue),
        repository: repository_of(&plan),
    };
    Ok(DevTask::build(&model, &input.issue))
}

fn devtask_milestone(
    ctx: &Context,
    input: DevMilestoneInput,
) -> Result<MilestoneGraph, CapabilityError> {
    let plan = Plan::build(&ctx.index);
    let record = record(&ctx.index, MILESTONE, &input.milestone)
        .map(|(path, metadata)| RecordRef { path, metadata });
    Ok(MilestoneGraph::build(&plan, &input.milestone, record))
}

// ---------------------------------------------------------------- the module

/// The `devtask` module: the canonical development task, composed from derivations that
/// already exist.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "devtask",
        title: "Executable development scopes",
        description: "An issue and a milestone read as work to be done rather than as metadata: what a person authored, where the plan graph puts it, what happened to it locally, where its external projection stands, and whether a worker may start it — with the origin of every value on the value. Nothing here is a second project model: the records are the canonical ones, the status and the graph are `plan`'s, the branches and commits are `trace`'s, the readiness vocabulary refines three canonical statuses without replacing any, and the projection onto GitHub stays `scripts/github-sync`'s. Nothing here writes anything, which is what makes a user-authored GitHub value safe from it.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "devtask.issue",
                title: "One issue as an executable development task",
                description: "Identity, title, intent, status, milestone, acceptance criteria, dependencies, blockers, the commits its own evidence names and the commits git found, its branches, the sessions that worked on it, its readiness and everything wrong with its records — each field carrying whether a person authored it (`explicit`), a machine worked it out by a rule that cannot be wrong (`derived`), a machine worked it out by a rule that can (`inferred`, with the rule stated), or it is not available at all (`unknown`, with the reason). A key the record does not carry is `unknown`, never an empty string: `objective: \"\"` and a record with no objective are the same string and different facts. An id the model does not declare is answered, not refused. `git: false` answers from the records alone, which is what a comparison across machines wants.",
                input: DevTaskInput,
                output: DevTask,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_devtask"),
                    http: get("/api/v1/devtask/issue"),
                    cli: Some(CliExposure { path: vec!["devtask".into(), "issue".into()] }),
                },
                tags: ["devtask", "plan", "issue", "provenance", "readiness", "git"],
                cache: CachePolicy::Disabled,
                handler: devtask_issue,
            },
            capability! {
                id: "devtask.milestone",
                title: "One milestone as an executable dependency graph",
                description: "Every issue of the milestone as a node with its readiness, its wave and what waits on it; every dependency edge with at least one end inside, the crossing ones marked; the issues partitioned into ready, blocked, waiting, active, review, completion-blocked, complete and cancelled; the critical blockers ordered by how much unfinished work each holds back; the startable work partitioned into subsets that may genuinely run at the same time — same wave, each parallel-safe, no two sharing a scope path — with each serialisation naming the path that caused it; every dependency cycle as its strongly connected component; and every finding about the milestone or its issues. A pure function of the canonical records: no git, no clock, no network, every list in canonical order, so two runs on two machines produce the same bytes. A work surface reading this derives nothing itself.",
                input: DevMilestoneInput,
                output: MilestoneGraph,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_devtask_milestone"),
                    http: get("/api/v1/devtask/milestone"),
                    cli: Some(CliExposure { path: vec!["devtask".into(), "milestone".into()] }),
                },
                tags: ["devtask", "plan", "milestone", "graph", "readiness", "provenance"],
                cache: CachePolicy::Disabled,
                handler: devtask_milestone,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devtask::{FieldProvenance, TaskReadiness};

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "devtask");
        let expected: &[(&str, &str, &str, &[&str])] = &[
            (
                "devtask.issue",
                "majordomus_devtask",
                "/api/v1/devtask/issue",
                &["devtask", "issue"],
            ),
            (
                "devtask.milestone",
                "majordomus_devtask_milestone",
                "/api/v1/devtask/milestone",
                &["devtask", "milestone"],
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (executable, (id, tool, path, cli)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
            assert_eq!(
                exposure.cli.as_ref().map(|c| c.path.as_slice()),
                Some(
                    cli.iter()
                        .map(|s| (*s).to_string())
                        .collect::<Vec<String>>()
                        .as_slice()
                ),
                "{id} lost or renamed its command"
            );
        }
    }

    /// Every capability here is a query. A development task is derived; one that wrote
    /// anything would be the second store of truth this whole module exists to avoid — and
    /// the external half in particular must never be writable from a reader.
    #[test]
    fn every_capability_is_a_query_and_nothing_caches() {
        for e in module().capabilities {
            assert!(
                e.capability.kind.is_read_only(),
                "{} writes",
                e.capability.id
            );
            assert!(
                !e.capability.cache.is_enabled(),
                "{} caches: the records and the history change outside this process",
                e.capability.id
            );
        }
    }

    /// The one property that makes this a composition rather than a parallel registry: for
    /// every issue of this repository, the task's status *is* the plan's status, and its
    /// readiness names that same status back.
    #[test]
    fn the_status_never_disagrees_with_the_plan() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("synthetic repository");
        let index = repo.index().expect("index");
        let plan = Plan::build(&index);
        for issue in &plan.issues {
            let record = record(&index, ISSUE, &issue.id)
                .map(|(path, metadata)| RecordRef { path, metadata });
            let model = DevTaskModel {
                plan: &plan,
                record,
                trace: None,
                sessions: Vec::new(),
                repository: repository_of(&plan),
            };
            let task = DevTask::build(&model, &issue.id);
            assert_eq!(
                task.position.status.text(),
                issue.status,
                "{} disagrees with the plan",
                issue.id
            );
            assert_eq!(
                task.readiness.canonical_status, issue.status,
                "{} readiness names a status the plan did not derive",
                issue.id
            );
            assert_eq!(task.position.status.provenance, FieldProvenance::Derived);
            assert!(task.is_consistent());
        }
    }

    /// An id nothing declares is answered rather than refused, and the answer cannot be
    /// mistaken for an issue nobody has started.
    #[test]
    fn an_id_the_model_does_not_have_is_answered_with_undeclared() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("synthetic repository");
        let index = repo.index().expect("index");
        let plan = Plan::build(&index);
        let model = DevTaskModel {
            plan: &plan,
            record: None,
            trace: None,
            sessions: Vec::new(),
            repository: repository_of(&plan),
        };
        let task = DevTask::build(&model, "I9999");
        assert!(!task.declared);
        assert_eq!(task.readiness.state, TaskReadiness::Undeclared);
        assert!(!task.readiness.startable);
    }

    /// The milestone graph partitions its own nodes, whatever the repository holds: an
    /// issue is in exactly one readiness bucket and every bucket names a node.
    #[test]
    fn every_milestone_of_this_repository_is_partitioned() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("synthetic repository");
        let index = repo.index().expect("index");
        let plan = Plan::build(&index);
        for m in &plan.milestones {
            let record = record(&index, MILESTONE, &m.id)
                .map(|(path, metadata)| RecordRef { path, metadata });
            let g = MilestoneGraph::build(&plan, &m.id, record);
            assert!(g.is_partitioned(), "{} is not partitioned", m.id);
            assert!(g.is_consistent(), "{} has an inconsistent field", m.id);
            assert_eq!(
                g.status.text(),
                m.status,
                "{} disagrees with the plan",
                m.id
            );
        }
    }
}
