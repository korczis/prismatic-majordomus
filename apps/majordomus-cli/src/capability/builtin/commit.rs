//! The `commit` module: what this repository holds a commit to, what the working tree
//! would commit, and which scope the history gives a set of paths.
//!
//! Three read-only capabilities over [`crate::commit`]. Nothing here decides anything: the
//! grammar, the policy, the judge and the planner are the domain's, and this module is the
//! declaration that carries them onto the surfaces — the MCP tool, the HTTP route, the
//! OpenAPI operation, the Cockpit payload and the generated reference — without any of them
//! holding a second copy of a rule.
//!
//! The mutation is deliberately not here. Making a commit is a repository mutation, and the
//! exposure policy (`command_graph::policy`) stops every machine surface at local mutation:
//! an agent may ask what a commit *would* be and whether a message passes, and a person at a
//! terminal is who commits. That is not a limitation worked around, it is the arrangement
//! this repository chose, and the commit subsystem is shaped to it — which is why the
//! planner and the judge are pure functions of a tree and a message.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::commit::verdict::{HistoryReport, JudgedCommit};
use crate::commit::{self, CommitPolicy, CommitPlan, CommitVerdict, ScopeVocabulary};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the scope vocabulary is read as an MCP resource.
pub const SCOPES_URI: &str = "majordomus://commit/scopes";
/// The URI under which the plan over the working tree is read as an MCP resource.
pub const PLAN_URI: &str = "majordomus://commit/plan";

/// The repository root this process serves, as a path.
fn root(ctx: &Context) -> std::path::PathBuf {
    std::path::PathBuf::from(&ctx.index.repository.root)
}

/// The commit policy this repository declares, or the default when the policy cannot be
/// read. An unreadable policy is `doctor`'s finding; it is not a reason for a validator to
/// silently stop checking.
fn policy(ctx: &Context) -> CommitPolicy {
    let Ok(repo) = crate::repository::Repository::open(&root(ctx)) else {
        return CommitPolicy::default();
    };
    crate::policy::LoadedPolicy::load(&repo)
        .map(|l| l.policy.commit)
        .unwrap_or_default()
}

/// The record ids the layer holds, for resolving what a message names.
fn records(ctx: &Context) -> Vec<String> {
    ctx.index
        .objects
        .iter()
        .filter(|o| o.kind == "issue" || o.kind == "milestone")
        .map(|o| o.identity.clone())
        .collect()
}

// ---------------------------------------------------------------- commit.scopes

/// The vocabulary, with the policy that reads it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScopesReport {
    /// Every scope the history uses, most used first, with the directories it is used about.
    pub vocabulary: ScopeVocabulary,
    /// How many commits back the vocabulary is learned from.
    pub sample: usize,
}

pub(super) fn scopes(ctx: &Context, _: Empty) -> Result<ScopesReport, CapabilityError> {
    Ok(ScopesReport {
        vocabulary: commit::scopes::derive(&root(ctx)),
        sample: commit::scopes::SAMPLE,
    })
}

// ---------------------------------------------------------------- commit.plan

pub(super) fn plan(ctx: &Context, _: Empty) -> Result<CommitPlan, CapabilityError> {
    let root = root(ctx);
    let tree = commit::plan::working_tree(&root).ok_or_else(|| {
        CapabilityError::NotFound(
            "this is not a git work tree, or git could not be asked; there is nothing to commit \
             and that is not the same as a clean tree"
                .into(),
        )
    })?;
    let identity = crate::worktree::identity::RepositoryIdentity::discover(&root).ok();
    let (repository, worktree) = match &identity {
        Some(id) => (
            id.git_common_dir().real.display().to_string(),
            id.current_worktree().real.display().to_string(),
        ),
        // A repository git can describe but whose worktree topology could not be resolved
        // still fingerprints; it fingerprints on what is known, and says so by carrying the
        // root in both fields rather than by inventing an identity.
        None => (root.display().to_string(), root.display().to_string()),
    };
    let fingerprint = commit::PlanFingerprint::of(&repository, &worktree, &tree);
    let vocabulary = commit::scopes::derive(&root);
    let staged = tree.staged();
    let derived = commit::plan::derived_among(&root, &staged);
    let groups = commit::plan::group(&staged, &vocabulary, &derived);
    let mut diagnostics = Vec::new();
    if let Some(what) = &tree.in_progress {
        diagnostics.push(crate::model::Diagnostic::warning(
            "commit.in_progress",
            None,
            format!("a {what} is in progress; finish or abort it before planning a commit"),
        ));
    }
    if staged.is_empty() && !tree.is_clean() {
        diagnostics.push(crate::model::Diagnostic::warning(
            "commit.nothing_staged",
            None,
            format!(
                "{} change(s) in the working tree and nothing staged; a commit now would be empty",
                tree.changes.len()
            ),
        ));
    }
    for change in tree.changes.iter().filter(|c| c.partial) {
        diagnostics.push(crate::model::Diagnostic::warning(
            "commit.partially_staged",
            Some(change.path.clone()),
            "staged and modified again since; the commit would carry the staged half only",
        ));
    }
    Ok(CommitPlan {
        fingerprint,
        tree,
        groups,
        diagnostics,
    })
}

// ---------------------------------------------------------------- commit.validate

/// A message to judge, and what is known about the commit it would make.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateInput {
    /// The commit message, as it would be stored. Comment lines are ignored, as git ignores
    /// them, so what is judged is what would be committed.
    pub message: String,
    /// The paths the commit would contain. Omit them and the judgements that need them —
    /// whether a fix carries a test — are not made rather than guessed.
    #[serde(default)]
    pub paths: Option<Vec<String>>,
}

impl BenchmarkCases for ValidateInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "conventional",
                ValidateInput {
                    message: "feat(commit): the rule is decided by a command".into(),
                    paths: None,
                },
            ),
            NamedCase::new(
                "not-conventional",
                ValidateInput {
                    message: "update stuff".into(),
                    paths: None,
                },
            ),
            NamedCase::new(
                "merge",
                ValidateInput {
                    message: "Merge pull request #249 from korczis/fix".into(),
                    paths: None,
                },
            ),
        ]
    }
}

pub(super) fn validate(ctx: &Context, input: ValidateInput) -> Result<CommitVerdict, CapabilityError> {
    if input.message.trim().is_empty() {
        return Err(CapabilityError::InvalidInput(
            "the message is empty; there is nothing to judge".into(),
        ));
    }
    let vocabulary = commit::scopes::derive(&root(ctx));
    let subject = commit::CommitSubject {
        text: input.message,
        scopes: (!vocabulary.scopes.is_empty()).then(|| vocabulary.words()),
        paths: input.paths,
        records: records(ctx),
    };
    Ok(commit::judge(&subject, &policy(ctx)))
}

// ---------------------------------------------------------------- commit.history

/// Which range of history to judge.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryInput {
    /// Anything `git log` accepts: `origin/master..HEAD`, `v0.6.0..`, a bare `HEAD`.
    /// Default `HEAD`, which is the whole history reachable from here.
    #[serde(default = "head")]
    pub range: String,
}

fn head() -> String {
    "HEAD".into()
}

impl BenchmarkCases for HistoryInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            // Bounded, because a benchmark that reads the whole history measures how old
            // the repository is rather than how fast the judge is.
            NamedCase::new(
                "recent",
                HistoryInput {
                    range: "HEAD~50..HEAD".into(),
                },
            ),
        ]
    }
}

pub(super) fn history(ctx: &Context, input: HistoryInput) -> Result<HistoryReport, CapabilityError> {
    let root = root(ctx);
    let messages = commit::plan::messages_in(&root, &input.range).ok_or_else(|| {
        CapabilityError::InvalidInput(format!(
            "git does not know the range '{}'",
            input.range
        ))
    })?;
    let vocabulary = commit::scopes::derive(&root);
    let words = (!vocabulary.scopes.is_empty()).then(|| vocabulary.words());
    let records = records(ctx);
    let policy = policy(ctx);
    let mut report = HistoryReport {
        range: input.range,
        total: messages.len(),
        exempt: 0,
        failing: 0,
        commits: Vec::new(),
    };
    for (commit_name, text) in messages {
        let subject = commit::CommitSubject {
            text,
            scopes: words.clone(),
            // A commit's paths are knowable from git, and are deliberately not read: the
            // judgements that need them are about what a commit *should have carried*, and
            // asking that of a commit already in the history is asking somebody to rewrite
            // it. The gate judges what a message says, and the hook — which runs while the
            // commit is still being made — is where the paths are.
            paths: None,
            records: records.clone(),
        };
        let verdict = commit::judge(&subject, &policy);
        if verdict.exempt.is_some() {
            report.exempt += 1;
            continue;
        }
        if !verdict.passed {
            report.failing += 1;
        }
        if !verdict.findings.is_empty() {
            report.commits.push(JudgedCommit {
                commit: commit_name,
                subject: verdict.message.header.subject.clone(),
                passed: verdict.passed,
                exempt: verdict.exempt,
                findings: verdict.findings,
            });
        }
    }
    Ok(report)
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "commit",
        title: "Commit",
        description: "The commit as a value: the scope vocabulary this repository's own history yields, what the working tree would commit and how it divides, and the verdict on one message against the repository's commit policy.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "commit.scopes",
                title: "The scope vocabulary",
                description: "Every scope this repository's commit history uses, how often, and the directories each one is written about — learned from the history rather than declared in a table, so that a subsystem committed today is in the vocabulary today and one nobody has touched falls to the bottom on its own.",
                input: Empty,
                output: ScopesReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_commit_scopes".into()),
                        resource: Some(McpResource { uri: SCOPES_URI.into(), name: "commit-scopes".into() }),
                    }),
                    http: get("/api/v1/commit/scopes"),
                    cli: Some(CliExposure { path: vec!["commit".into(), "scopes".into()] }),
                },
                tags: ["commit", "git", "introspection"],
                handler: scopes,
            },
            capability! {
                id: "commit.plan",
                title: "What the working tree would commit",
                description: "The working tree as git reports it — branch, upstream, divergence, every staged, unstaged and untracked path, and any merge or rebase in progress — divided into the commits the history's own scoping supports, each with the evidence for it, under a fingerprint of the repository, the worktree, HEAD and the exact change set, so that a plan acted on later can be refused rather than applied to a tree it was not made for.",
                input: Empty,
                output: CommitPlan,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_commit_plan".into()),
                        resource: Some(McpResource { uri: PLAN_URI.into(), name: "commit-plan".into() }),
                    }),
                    http: get("/api/v1/commit/plan"),
                    cli: Some(CliExposure { path: vec!["commit".into(), "plan".into()] }),
                },
                tags: ["commit", "git", "plan"],
                handler: plan,
            },
            capability! {
                id: "commit.validate",
                title: "Judge one commit message",
                description: "One message against this repository's commit policy: the grammar, the subject width, whether the scope is one the history uses, whether every record the message names exists, whether a fix carries a test, and whether a breaking change explains itself — as typed findings with severities, or an exemption for a subject git composed. The same verdict the commit-msg hook refuses with and the gate reads the history through.",
                input: ValidateInput,
                output: CommitVerdict,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_commit_validate"),
                    http: get("/api/v1/commit/validate"),
                    cli: Some(CliExposure { path: vec!["commit".into(), "validate".into()] }),
                },
                tags: ["commit", "git", "governance"],
                handler: validate,
            },
            capability! {
                id: "commit.history",
                title: "Judge a range of history",
                description: "Every commit in a git range against the commit policy, in one pass: how many were read, how many git composed and are exempt, how many carry an error, and every commit that has a finding with what it is. What the gate reads to refuse new debt, and what answers `is the history of this branch clean?` without a process per commit.",
                input: HistoryInput,
                output: HistoryReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_commit_history"),
                    http: get("/api/v1/commit/history"),
                    cli: Some(CliExposure { path: vec!["commit".into(), "history".into()] }),
                },
                tags: ["commit", "git", "governance"],
                handler: history,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; every projection derives from
    /// it. A refactor that dropped an exposure would still compile and every behavioural
    /// suite would still pass. This is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "commit");
        let expected: &[(&str, &str, &str)] = &[
            (
                "commit.scopes",
                "majordomus_commit_scopes",
                "/api/v1/commit/scopes",
            ),
            ("commit.plan", "majordomus_commit_plan", "/api/v1/commit/plan"),
            (
                "commit.validate",
                "majordomus_commit_validate",
                "/api/v1/commit/validate",
            ),
            (
                "commit.history",
                "majordomus_commit_history",
                "/api/v1/commit/history",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(ids, want, "the module declares a different set");
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
        }
    }

    /// Every capability here reads; none of them commits. The line between asking what a
    /// commit would be and making one is what lets these reach an agent at all, and it is
    /// worth an assertion in the file that draws it.
    #[test]
    fn nothing_in_this_module_writes_the_repository() {
        use crate::capability::Effect;
        for e in module().capabilities {
            assert_eq!(
                e.capability.execution.effect,
                Effect::Read,
                "{} would mutate; making a commit is the command line's, not a machine surface's",
                e.capability.id.as_str()
            );
        }
    }
}
