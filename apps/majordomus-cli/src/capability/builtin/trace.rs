//! The `trace` module: the work graph above the branch, both ways.
//!
//! The canonical model reached as far as a branch — `worktree::state::issue_of` reads an
//! issue id out of a branch name, and the topology gate enforces it. These three
//! capabilities continue that one edge in both directions, deriving everything and storing
//! nothing: an issue's branches and commits come from `git for-each-ref` and `git log`, and
//! a commit's issue is the issue whose branches contain it.
//!
//! The milestone is the one field here that git does not hold. It is not derived either: it
//! is read from the canonical issue record through the index, which is where the project
//! model already declares it, and it is the reason these handlers take a [`Context`] rather
//! than a path.
//!
//! # Where the pull requests are
//!
//! A pull request is a GitHub fact, and this executable makes no network call —
//! `test/cases/08_no_forbidden_constructs.sh` is the proof, and the rule is the reason
//! `scripts/github-sync` exists in `scripts/` rather than in `lib/`. So the pull-request
//! half of the same derivation is `scripts/traceability`, which reads pull requests with
//! `gh` (or from a fixture), maps each head branch to an issue by this module's rule, and
//! joins the two answers. The seam between them is the branch name, which both sides read
//! and neither side stores.
//!
//! # Why no command line
//!
//! Every exposure of a capability is declared here, and the command line is the one
//! projection that is declared twice — a `cli` exposure has to have a matching clap command
//! in `cli.rs`, which `capability::closure` proves. These capabilities claim MCP and HTTP
//! only; `scripts/traceability` reaches the git half over MCP, the way any other client
//! would.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::index::Index;
use crate::worktree::trace::{
    self, CommitAttribution, IssueTrace, TraceReport, Tracer, DEFAULT_REPORT_COMMITS,
    MAX_REPORT_COMMITS,
};
use crate::worktree::WorktreeError;
use crate::{capability, module};

use super::{get, mcp};

/// The URI under which the whole traceability report is read as an MCP resource.
pub const TRACEABILITY_URI: &str = "majordomus://traceability";

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One issue of the project model.
pub struct TraceIssueInput {
    /// The issue id, as the project model spells it (`I1305`). An id the model does not
    /// declare is answered with `declared: false` rather than refused, and rather than
    /// answered with a bare empty trace: a typo that read as "nothing realised this issue"
    /// is the one answer this capability must never give.
    pub issue: String,
}

impl BenchmarkCases for TraceIssueInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // An issue that exists in the repository the benchmark runs against, so the case
        // measures the derivation rather than the refusal; a hard-coded id would be a
        // second declaration of this repository's own plan. A repository with no issues at
        // all still gets a case — an empty list would read to the coverage gate as a
        // capability nobody benchmarks, and timing the refusal is a real measurement.
        let issue = first_issue(ctx.index).unwrap_or_else(|| "I0001".to_string());
        vec![NamedCase::new("first-issue", TraceIssueInput { issue })]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One commit.
pub struct TraceCommitInput {
    /// Anything git resolves to a commit: a full or abbreviated object name, a ref, `HEAD`,
    /// `master~3`.
    pub commit: String,
}

impl BenchmarkCases for TraceCommitInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "head",
            TraceCommitInput {
                commit: "HEAD".into(),
            },
        )]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// How much of the trunk to attribute.
pub struct TraceReportInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// How many trunk commits to attribute, newest first. Default: 50. Anything above 2000
    /// is read as 2000 — a traceability report is a reading, not a history export.
    pub limit: Option<usize>,
}

impl BenchmarkCases for TraceReportInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", TraceReportInput { limit: None }),
            NamedCase::new("ten", TraceReportInput { limit: Some(10) }),
        ]
    }
}

// ---------------------------------------------------------------- the canonical edges the index holds

/// The first issue the index holds, in URI order. The benchmark case's input, and nothing
/// else: a capability never picks an object for a caller.
fn first_issue(index: &Index) -> Option<String> {
    index
        .objects
        .iter()
        .find(|o| o.kind == "issue")
        .map(|o| o.identity.clone())
}

/// The milestone one issue record names. Git does not hold this edge, and neither does
/// GitHub in any form this repository trusts: the canonical record under
/// `.ai/repo/project/issues/` declares it, the index has already parsed it, and reading it
/// here is reading the model rather than repeating it.
fn milestone_of(index: &Index, issue: &str) -> Option<String> {
    index
        .objects
        .iter()
        .find(|o| o.kind == "issue" && o.identity == issue)
        .and_then(|o| o.metadata.get("milestone"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Does the project model declare this issue at all?
fn declared(index: &Index, issue: &str) -> bool {
    index
        .objects
        .iter()
        .any(|o| o.kind == "issue" && o.identity == issue)
}

/// Every issue id the project model declares, in id order. The index has already read and
/// validated every issue record, so this is the model itself rather than a second listing
/// of the directory it lives in.
fn issue_ids(index: &Index) -> Vec<String> {
    let mut ids: Vec<String> = index
        .objects
        .iter()
        .filter(|o| o.kind == "issue")
        .map(|o| o.identity.clone())
        .collect();
    crate::order::canonical(&mut ids);
    ids
}

/// Fill in every canonical edge the git derivation deliberately left empty.
fn with_milestones(index: &Index, mut traces: Vec<IssueTrace>) -> Vec<IssueTrace> {
    for t in &mut traces {
        t.milestone = milestone_of(index, &t.issue);
    }
    traces
}

/// The same, for an attribution: the milestone follows the issue it named.
fn attribution_milestones(
    index: &Index,
    mut commits: Vec<CommitAttribution>,
) -> Vec<CommitAttribution> {
    for c in &mut commits {
        c.milestone = c.issue.as_deref().and_then(|i| milestone_of(index, i));
    }
    commits
}

fn refused(e: WorktreeError) -> CapabilityError {
    match e.exit_code() {
        crate::worktree::EXIT_MISSING => CapabilityError::NotFound(e.to_string()),
        crate::worktree::EXIT_REFUSED => CapabilityError::Refused(e.to_string()),
        _ => CapabilityError::Internal(e.to_string()),
    }
}

fn tracer_of(ctx: &Context) -> Result<Tracer, CapabilityError> {
    Tracer::open(Path::new(&ctx.index.repository.root))
        .map(|t| t.with_issues(issue_ids(&ctx.index)))
        .map_err(refused)
}

// ---------------------------------------------------------------- handlers

fn trace_issue(ctx: &Context, input: TraceIssueInput) -> Result<IssueTrace, CapabilityError> {
    let mut t = tracer_of(ctx)?.trace(&input.issue).map_err(refused)?;
    t.declared = declared(&ctx.index, &input.issue);
    t.milestone = milestone_of(&ctx.index, &input.issue);
    Ok(t)
}

fn trace_commit(
    ctx: &Context,
    input: TraceCommitInput,
) -> Result<CommitAttribution, CapabilityError> {
    let tracer = tracer_of(ctx)?;
    let Some(commit) = tracer.commit(&input.commit).map_err(refused)? else {
        return Err(CapabilityError::NotFound(format!(
            "`{}` does not resolve to a commit in this repository",
            input.commit
        )));
    };
    let traces = with_milestones(&ctx.index, tracer.traces().map_err(refused)?);
    let answer = trace::attribute(&[commit], &traces)
        .into_iter()
        .next()
        .expect("attribute answers for every commit it is given");
    Ok(attribution_milestones(&ctx.index, vec![answer])
        .into_iter()
        .next()
        .expect("one in, one out"))
}

fn trace_report(ctx: &Context, input: TraceReportInput) -> Result<TraceReport, CapabilityError> {
    let limit = input
        .limit
        .unwrap_or(DEFAULT_REPORT_COMMITS)
        .min(MAX_REPORT_COMMITS);
    let mut report = tracer_of(ctx)?.report(limit).map_err(refused)?;
    report.issues = with_milestones(&ctx.index, report.issues);
    report.commits = attribution_milestones(&ctx.index, report.commits);
    Ok(report)
}

// ---------------------------------------------------------------- the module

/// The `trace` module: the issue-to-commit edge, derived on every call.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "trace",
        title: "Traceability",
        description: "Which branches and commits realised an issue, and which issue and milestone a commit served — derived from git and from the canonical project model on every call, stored nowhere. A branch names an issue when one of its path components is an issue id; a commit belongs to the issue whose branches hold it; a commit no such branch holds is reported as unattributed rather than left out, because work with no execution contract is what a traceability report exists to make visible. Pull requests are a GitHub fact and this executable makes no network call: `scripts/traceability` reads them and joins them to this answer over the branch name.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "trace.issue",
                title: "What realised this issue",
                description: "One issue with the branches that name it — local, and remote-tracking where only the remote still has the branch — and, for each, the commits it holds that the trunk did not: measured against the trunk while the branch is open, and against the first parent of the merge commit that brought it in once it is merged. A branch that reached the trunk without a merge commit of its own says so and claims nothing, because its commits cannot be told from the trunk's. The milestone comes from the canonical issue record, which is the one edge here that git does not hold, and `declared` says whether the project model has this id at all — a repository with no plan still gets the branches, and a typo still cannot read as work nobody did.",
                input: TraceIssueInput,
                output: IssueTrace,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_trace_issue"),
                    http: get("/api/v1/trace/issue"),
                    cli: None,
                },
                tags: ["trace", "git", "plan", "issue", "provenance"],
                cache: CachePolicy::Disabled,
                handler: trace_issue,
            },
            capability! {
                id: "trace.commit",
                title: "What contract this commit served",
                description: "One commit with the issue and milestone it served, or the fact that none can be found. Attributed when exactly one issue's branches hold it, ambiguous when branches naming two issues do, and unattributed when no branch naming an issue holds it at all — which is either work committed without an execution contract or a branch deleted after its merge, and the answer says so rather than guessing between them.",
                input: TraceCommitInput,
                output: CommitAttribution,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_trace_commit"),
                    http: get("/api/v1/trace/commit"),
                    cli: None,
                },
                tags: ["trace", "git", "plan", "commit", "provenance"],
                cache: CachePolicy::Disabled,
                handler: trace_commit,
            },
            capability! {
                id: "trace.report",
                title: "The whole work graph above the branch",
                description: "Every issue at least one branch names with its branches and commits, every declared issue no branch names, and the newest stretch of the trunk with each commit attributed to the issue whose branches hold it or reported as unattributed. The tallies count both sides, so the proportion of the trunk that no execution contract accounts for is a number rather than an impression. Read from git on every call: the history changes outside this process.",
                input: TraceReportInput,
                output: TraceReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_traceability".into()),
                        resource: Some(McpResource { uri: TRACEABILITY_URI.into(), name: "traceability".into() }),
                    }),
                    http: get("/api/v1/trace"),
                    cli: None,
                },
                tags: ["trace", "git", "plan", "provenance", "introspection"],
                cache: CachePolicy::Disabled,
                handler: trace_report,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "trace");
        let expected: &[(&str, &str, &str)] = &[
            (
                "trace.issue",
                "majordomus_trace_issue",
                "/api/v1/trace/issue",
            ),
            (
                "trace.commit",
                "majordomus_trace_commit",
                "/api/v1/trace/commit",
            ),
            ("trace.report", "majordomus_traceability", "/api/v1/trace"),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
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
            assert!(
                !executable.capability.cache.is_enabled(),
                "{id} must not cache: the history changes outside this process"
            );
        }
    }

    /// Every capability of the module is read-only. Traceability is derived; a capability
    /// here that wrote anything would be the second store of truth the whole module exists
    /// to avoid.
    #[test]
    fn every_capability_is_a_query() {
        for e in module().capabilities {
            assert!(
                e.capability.kind.is_read_only(),
                "{} writes",
                e.capability.id
            );
        }
    }

    /// The command line is declared twice — here and in clap — so a `cli` exposure this
    /// module has no clap command for would fail the projection closure. It claims none.
    #[test]
    fn nothing_here_claims_a_command_line() {
        for e in module().capabilities {
            assert!(
                e.capability.exposure.cli.is_none(),
                "{} claims a command line that cli.rs does not declare",
                e.capability.id
            );
        }
    }
}
