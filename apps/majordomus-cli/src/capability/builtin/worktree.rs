//! The `worktree` module: the branch-to-worktree topology of this repository, projected.
//!
//! Six questions, all read-only, and one command, all answered by
//! [`crate::worktree::WorktreeService`] — the same service the command line renders and the
//! git hooks ask. Declaring them here is what puts them on MCP (as tools and as a resource),
//! on HTTP, in the OpenAPI document, in the Swagger UI, in the Cockpit and in the generated
//! reference, without any of those carrying a route, a schema or a sentence of their own.
//!
//! The one command is `worktree.create` (I4002): the canonical worktree of a branch, made or
//! found, which is what an executor needs before it can do any work in isolation. It is
//! [`WorktreeService::ensure`] and nothing else — the path is derived, a misplaced or
//! conflicting worktree is refused with the service's own reason, `git worktree add` runs
//! under a deadline — and it declares `Effect::RepositoryMutation`, so every surface asks
//! before it runs. Its command-line projection is `majordomus worktree ensure`, which has
//! the same idempotent contract; `worktree create` stays the command that refuses a worktree
//! already there.
//!
//! Migrating, repairing and removing are deliberately *not* here: they move or delete work
//! that exists, which is not a thing the shared MCP server takes on. They are command-line
//! operations, they call the same service, and the Cockpit names the exact command for each.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CapabilityKind, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::worktree::{
    migrate, CreateReport, CreateRequest, Detail, InspectReport, MigrationPlan, RepositoryTopology,
    StatusReport, Threshold, TrunkDistance, UncommittedReport, WorktreeError, WorktreeService,
};
use crate::{capability, module};

use super::{get, mcp, post, Empty};

/// The URI under which the topology is read as an MCP resource.
pub const WORKTREES_URI: &str = "majordomus://worktrees";

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Where to answer from.
pub struct StatusInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A directory inside one of this repository's worktrees; the answer is about that
    /// worktree. Relative to the primary checkout when relative (`.` is the primary
    /// checkout itself). Default: the repository the server was started for. A path in
    /// another repository is refused: the topology answers only about its own.
    pub path: Option<String>,
}

impl BenchmarkCases for StatusInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", StatusInput { path: None }),
            NamedCase::new(
                "primary-checkout",
                StatusInput {
                    path: Some(".".into()),
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// A branch to inspect.
pub struct InspectInput {
    /// The branch, full name (`feature/improve-cli`). It need not exist: the canonical path
    /// derives from the name alone.
    pub branch: String,
}

impl BenchmarkCases for InspectInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "feature-branch",
            InspectInput {
                branch: "feature/example".into(),
            },
        )]
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// What to judge uncommitted work against.
pub struct UncommittedInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The age past which work nothing holds is abandoned, as a person writes it (`90s`,
    /// `30m`, `12h`, `1d`). Default: the policy's `session.stranded_after`, which is also
    /// how long an open episode may be silent and still hold the work.
    pub older_than: Option<String>,
}

impl BenchmarkCases for UncommittedInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("policy-threshold", UncommittedInput { older_than: None }),
            NamedCase::new(
                "one-hour",
                UncommittedInput {
                    older_than: Some("1h".into()),
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The branch whose canonical worktree is wanted.
pub struct CreateInput {
    /// The branch, full name (`feature/improve-cli`). Checked out when it exists here,
    /// taken up from the remote when the remote has it, otherwise created at `base`.
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit or ref a new branch starts from, as given and never fetched. Default: the
    /// remote's trunk, fetched now within a deadline, then the local trunk, said in `start`.
    /// Ignored when the branch exists.
    pub base: Option<String>,
}

impl BenchmarkCases for CreateInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // A benchmark must not create worktrees: the one case is a name the service refuses
        // before git is asked anything, so it measures the whole path to the refusal and
        // leaves the repository as it was.
        vec![NamedCase::new(
            "refused-name",
            CreateInput {
                branch: "feature/../escape".into(),
                base: None,
            },
        )]
    }
}

// ---------------------------------------------------------------- the service, from a context

fn refused(e: WorktreeError) -> CapabilityError {
    match e.exit_code() {
        crate::worktree::EXIT_MISSING => CapabilityError::NotFound(e.to_string()),
        crate::worktree::EXIT_REFUSED => CapabilityError::Refused(e.to_string()),
        _ => CapabilityError::Internal(e.to_string()),
    }
}

/// The one service, over the repository this process serves.
pub fn service_of(ctx: &Context) -> Result<WorktreeService, CapabilityError> {
    WorktreeService::open(Path::new(&ctx.index.repository.root)).map_err(refused)
}

/// The service over a directory the caller named, refused unless it is inside a worktree
/// of the repository this process serves.
fn service_at(ctx: &Context, path: Option<&str>) -> Result<WorktreeService, CapabilityError> {
    let own = service_of(ctx)?;
    let Some(path) = path else {
        return Ok(own);
    };
    // relative to the primary checkout, never to this process's working directory
    let path = {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            Path::new(&ctx.index.repository.root).join(p)
        }
    };
    let other = WorktreeService::open(&path).map_err(refused)?;
    if !other
        .identity()
        .git_common_dir()
        .same_as(own.identity().git_common_dir())
    {
        return Err(CapabilityError::Refused(
            WorktreeError::NotThisRepository {
                path,
                git_common_dir: own.identity().git_common_dir().path.clone(),
            }
            .to_string(),
        ));
    }
    Ok(other)
}

// ---------------------------------------------------------------- handlers

fn worktree_topology(ctx: &Context, _: Empty) -> Result<RepositoryTopology, CapabilityError> {
    service_of(ctx)?.topology(Detail::Full).map_err(refused)
}

fn worktree_status(ctx: &Context, input: StatusInput) -> Result<StatusReport, CapabilityError> {
    service_at(ctx, input.path.as_deref())?
        .status()
        .map_err(refused)
}

fn worktree_inspect(ctx: &Context, input: InspectInput) -> Result<InspectReport, CapabilityError> {
    service_of(ctx)?.inspect(&input.branch).map_err(refused)
}

fn worktree_create(ctx: &Context, input: CreateInput) -> Result<CreateReport, CapabilityError> {
    service_of(ctx)?
        .ensure(&CreateRequest {
            branch: input.branch,
            base: input.base,
        })
        .map_err(refused)
}

fn worktree_migration_plan(ctx: &Context, _: Empty) -> Result<MigrationPlan, CapabilityError> {
    migrate::plan(&service_of(ctx)?).map_err(refused)
}

fn worktree_trunk(ctx: &Context, input: StatusInput) -> Result<TrunkDistance, CapabilityError> {
    Ok(service_at(ctx, input.path.as_deref())?.trunk_distance())
}

fn worktree_uncommitted(
    ctx: &Context,
    input: UncommittedInput,
) -> Result<UncommittedReport, CapabilityError> {
    let threshold = Threshold::declared_at(
        Path::new(&ctx.index.repository.root),
        input.older_than.as_deref(),
    )
    .map_err(CapabilityError::InvalidInput)?;
    service_of(ctx)?.uncommitted(&threshold).map_err(refused)
}

// ---------------------------------------------------------------- the module

/// The `worktree` module: the topology, projected once.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "worktree",
        title: "Worktree topology",
        description: "Where every linked git worktree of this repository belongs and where each one is. The container is the primary checkout's sibling named with `-wt`, the path under it is the branch name with its hierarchy kept, and both are derived from git's own identity — the common directory, the registered worktrees, the branches — never from a registry, a configuration or the current directory. A worktree somewhere else is a typed diagnostic with a remedy; the migration that repairs it is a command-line operation of the same service.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "worktree.topology",
                title: "The whole topology",
                description: "The repository, the container, the trunk and how it was decided, every registered worktree with its standing (primary, canonical, misplaced, detached, missing), its uncommitted work, its upstream distance and the issue its branch provably names, every local branch with or without a worktree and whether it is eligible for cleanup, every diagnostic with its code and remedy, and the tallies. Read from git on every call: the topology changes outside this process.",
                input: Empty,
                output: RepositoryTopology,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_worktrees".into()),
                        resource: Some(McpResource { uri: WORKTREES_URI.into(), name: "worktrees".into() }),
                    }),
                    http: get("/api/v1/worktrees"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "topology".into()] }),
                },
                tags: ["worktree", "git", "topology", "introspection"],
                cache: CachePolicy::Disabled,
                handler: worktree_topology,
            },
            capability! {
                id: "worktree.status",
                title: "Where this is, and whether that is where it belongs",
                description: "One worktree — the repository's own, or the one holding the directory the caller names — with its standing, its branch, its canonical path, its uncommitted work counted, whether it is where it belongs, and how many errors the whole topology carries. What an agent reads before it starts, and what the guard decides on.",
                input: StatusInput,
                output: StatusReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_status"),
                    http: get("/api/v1/worktrees/status"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "status".into()] }),
                },
                tags: ["worktree", "git", "topology"],
                cache: CachePolicy::Disabled,
                handler: worktree_status,
            },
            capability! {
                id: "worktree.inspect",
                title: "One branch: where its worktree belongs and what is there",
                description: "The canonical path of a branch, derived from its name alone, whether the branch exists, whether something occupies that path, the worktree holding the branch when one does, and what stands in the way of creating or migrating it. The answer for a branch that does not exist yet is the path `worktree create` would use.",
                input: InspectInput,
                output: InspectReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_inspect"),
                    http: get("/api/v1/worktrees/inspect"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "inspect".into()] }),
                },
                tags: ["worktree", "git", "topology"],
                cache: CachePolicy::Disabled,
                handler: worktree_inspect,
            },
            capability! {
                id: "worktree.create",
                kind: CapabilityKind::Command,
                title: "The canonical worktree of a branch, made or found",
                description: "Make the worktree of a branch at its canonical path — the container beside the primary checkout, then the branch name with its hierarchy kept, derived and never given — or find it there. Idempotent: the canonical worktree already holding the branch is the answer (`existed: true`), with nothing created. Refused with the service's reason and nothing changed: the branch checked out somewhere else (`BranchAlreadyCheckedOut`, with the path it belongs at), the canonical path occupied by something else (`DestinationConflict`), a name git or the topology does not accept. A branch that does not exist here starts at `base` when given, else where the remote has it, else at the remote's trunk fetched now within a deadline, else the local trunk with how far behind it is (`start`). `git worktree add` runs under the worktree lock and a deadline, and is killed and refused when it passes it. The answer is the branch, the path, the `head` read back from the worktree, and whether it was created or existed; direnv's approval and the per-worktree hooks are carried to it as `worktree ensure` does, because this is that command.",
                input: CreateInput,
                output: CreateReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_create"),
                    http: post("/api/v1/worktrees/create"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "ensure".into()] }),
                },
                tags: ["worktree", "git", "topology", "isolation"],
                handler: worktree_create,
            }
            // It adds a branch, a registration under the common directory and a directory
            // beside the checkout: a write, and every surface asks before it runs.
            .writes_repository(),
            capability! {
                id: "worktree.migration_plan",
                title: "What it would take to bring every worktree home",
                description: "One step per misplaced worktree with a branch: where it is, where it belongs, how it would move, the uncommitted work that moves with it, and what blocks it; plus the exceptions the migration cannot address by design — detached worktrees, stale registrations, the primary checkout off the trunk — each with what a person does about it. Planning changes nothing; `majordomus worktree migrate` applies it with a fingerprint taken before and after every move.",
                input: Empty,
                output: MigrationPlan,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_migration_plan"),
                    http: get("/api/v1/worktrees/migration"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "migrate".into()] }),
                },
                tags: ["worktree", "git", "topology", "migration"],
                cache: CachePolicy::Disabled,
                handler: worktree_migration_plan,
            },
            capability! {
                id: "worktree.uncommitted",
                title: "Uncommitted work in the linked worktrees, and what holds it",
                description: "Every linked worktree whose index or working tree holds changes no commit carries: how many paths (staged, unstaged, untracked, conflicted), when the oldest change was made (the earliest modification time among the changed paths, the index file's standing in for a deleted one), and what holds the work — an active task record, an open episode that is not stranded, or a process whose working directory is inside. Work nothing holds and older than the threshold is `abandoned`, with the command that would put it on a remote `rescue/` ref; it is suggested, never run. The threshold is the policy's `session.stranded_after` unless `older_than` names another. Reads every worktree and writes none: every status runs without optional locks, so no index is refreshed.",
                input: UncommittedInput,
                output: UncommittedReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_uncommitted"),
                    http: get("/api/v1/worktrees/uncommitted"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "uncommitted".into()] }),
                },
                tags: ["worktree", "git", "continuity", "introspection"],
                cache: CachePolicy::Disabled,
                handler: worktree_uncommitted,
            },
            capability! {
                id: "worktree.trunk",
                title: "How far a checkout is from the remote trunk",
                description: "The checkout's `HEAD` and the local trunk branch, each counted against the remote-tracking trunk — commits behind and ahead, a merge in progress counting what it brings in — as of the last fetch, whose time is part of the answer; judged against the policy's `checkout.trunk_distance` as `current`, `lagging`, `stale`, `unjudged` (no threshold declared) or `unmeasured` (no remote trunk), with the commands that bring it current. What doctor renders and what `server.status` and the MCP instructions say about the tree a server serves (`project.a-checkout-serves-the-current-trunk`). Fetches nothing and writes nothing.",
                input: StatusInput,
                output: TrunkDistance,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_worktree_trunk"),
                    http: get("/api/v1/worktrees/trunk"),
                    cli: Some(CliExposure { path: vec!["worktree".into(), "trunk".into()] }),
                },
                tags: ["worktree", "git", "freshness", "introspection"],
                cache: CachePolicy::Disabled,
                handler: worktree_trunk,
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
        assert_eq!(m.id.as_str(), "worktree");
        let expected: &[(&str, &str, &str)] = &[
            (
                "worktree.topology",
                "majordomus_worktrees",
                "/api/v1/worktrees",
            ),
            (
                "worktree.status",
                "majordomus_worktree_status",
                "/api/v1/worktrees/status",
            ),
            (
                "worktree.inspect",
                "majordomus_worktree_inspect",
                "/api/v1/worktrees/inspect",
            ),
            (
                "worktree.create",
                "majordomus_worktree_create",
                "/api/v1/worktrees/create",
            ),
            (
                "worktree.migration_plan",
                "majordomus_worktree_migration_plan",
                "/api/v1/worktrees/migration",
            ),
            (
                "worktree.uncommitted",
                "majordomus_worktree_uncommitted",
                "/api/v1/worktrees/uncommitted",
            ),
            (
                "worktree.trunk",
                "majordomus_worktree_trunk",
                "/api/v1/worktrees/trunk",
            ),
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
                "{id} must not cache: the topology changes outside this process"
            );
        }
        let resource = m.capabilities[0]
            .capability
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref());
        assert_eq!(resource.map(|r| r.uri.as_str()), Some(WORKTREES_URI));
    }

    /// An `older_than` that is not a duration is the caller's error, refused as invalid
    /// input before any work tree is read.
    #[test]
    fn an_uncommitted_reading_against_no_duration_is_invalid_input() {
        let fixture = crate::synthetic::SyntheticRepository::small().unwrap();
        let app = crate::app::App::load(&crate::cli::RepoArgs {
            repo: Some(fixture.root().to_path_buf()),
            discovery: crate::cli::DiscoveryMode::Filesystem,
            share: Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
            ..Default::default()
        })
        .unwrap();
        let refused = app
            .context
            .execute(
                "worktree.uncommitted",
                serde_json::json!({ "older_than": "soon" }),
            )
            .unwrap_err();
        assert!(
            matches!(&refused, CapabilityError::InvalidInput(m) if m.contains("older_than")),
            "{refused:?}"
        );
    }

    /// Every capability of the module is read-only but the one that creates, and that one
    /// says it writes the repository, by POST: migrating, repairing and removing stay on
    /// the command line.
    #[test]
    fn every_capability_is_a_query_but_the_create() {
        for e in module().capabilities {
            let c = &e.capability;
            if c.id.as_str() == "worktree.create" {
                assert_eq!(
                    c.execution.effect,
                    crate::capability::Effect::RepositoryMutation
                );
                assert!(c.execution.needs_confirmation());
                assert_eq!(
                    c.exposure.http.as_ref().map(|h| h.method.as_str()),
                    Some("POST")
                );
                continue;
            }
            assert!(c.kind.is_read_only(), "{} writes", c.id);
        }
    }

    /// A git repository with one commit, a bare `origin` holding it as its trunk, and the
    /// executable's context over it.
    fn served() -> (tempfile::TempDir, std::path::PathBuf, crate::app::App) {
        let fixture = crate::synthetic::SyntheticRepository::small().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let repo = root.join("repo");
        let git = |cwd: &Path, args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(cwd)
                .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        copy_tree(fixture.root(), &repo);
        git(&repo, &["init", "-q", "-b", "master"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        let origin = root.join("origin.git");
        git(
            &root,
            &["init", "-q", "--bare", "-b", "master", "origin.git"],
        );
        git(
            &repo,
            &["remote", "add", "origin", &origin.to_string_lossy()],
        );
        git(&repo, &["push", "-q", "-u", "origin", "master"]);
        let app = crate::app::App::load(&crate::cli::RepoArgs {
            repo: Some(repo.clone()),
            discovery: crate::cli::DiscoveryMode::Filesystem,
            share: Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
            ..Default::default()
        })
        .unwrap();
        drop(fixture);
        (tmp, repo, app)
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                if entry.file_name() == ".git" {
                    continue;
                }
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    /// Created at the canonical path from the remote's trunk, then found there on the
    /// second call with nothing created, and refused for a branch checked out elsewhere.
    #[test]
    fn a_create_makes_finds_and_refuses() {
        let (_tmp, repo, app) = served();
        let ask = |branch: &str| {
            app.context
                .execute("worktree.create", serde_json::json!({ "branch": branch }))
        };
        let canonical = repo
            .parent()
            .unwrap()
            .join("repo-wt/feature/x/y")
            .to_string_lossy()
            .to_string();

        let made: CreateReport = serde_json::from_value(ask("feature/x/y").unwrap()).unwrap();
        assert_eq!(made.path, canonical);
        assert_eq!(made.branch, "feature/x/y");
        assert!(!made.existed && made.branch_created);
        assert_eq!(made.base.as_deref(), Some("origin/master"));
        let head = made.head.clone().expect("a head");
        assert_eq!(made.start_commit.as_deref(), Some(head.as_str()));

        let found: CreateReport = serde_json::from_value(ask("feature/x/y").unwrap()).unwrap();
        assert!(found.existed && !found.branch_created);
        assert_eq!(found.path, canonical);
        assert_eq!(found.head.as_deref(), Some(head.as_str()));

        // the branch checked out by hand somewhere else is refused, and stays there
        let away = repo.parent().unwrap().join("away");
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["worktree", "add", "-q", "-b", "fix/elsewhere"])
            .arg(&away)
            .output()
            .unwrap();
        assert!(out.status.success());
        let refused = ask("fix/elsewhere").unwrap_err();
        assert!(
            matches!(&refused, CapabilityError::Refused(m) if m.contains("fix/elsewhere")),
            "{refused:?}"
        );
        assert!(away.is_dir());
        assert!(!repo
            .parent()
            .unwrap()
            .join("repo-wt/fix/elsewhere")
            .exists());

        // a name with no place in the container is refused before git is asked
        let escape = ask("feature/../escape").unwrap_err();
        assert!(matches!(escape, CapabilityError::Refused(_)), "{escape:?}");
    }
}
