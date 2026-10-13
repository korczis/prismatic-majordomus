//! The `accumulation` module: what this repository has piled up, against what its policy
//! allows (ADR 0150).
//!
//! One question, read-only, answered by [`crate::accumulation`]: the registered worktrees,
//! the local branches the trunk already reaches and the free space under them, each with
//! the budget the policy declares and the command that reduces it. Declaring it here is
//! what puts it on MCP, on HTTP, in the OpenAPI document, in the Cockpit and in the
//! generated reference without any of those carrying a route or a schema of its own.
//!
//! Reducing is deliberately *not* here. Removing a worktree is a thing a command line does
//! in the repository it stands in, on a consent the policy or a person gave; the shared
//! server measures, and names the command.

use std::path::Path;

use crate::accumulation::{self, AccumulationPolicy, AccumulationReport};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::worktree::WorktreeError;
use crate::{capability, module};

use super::Empty;

/// The URI under which the accumulation of the repository is read as an MCP resource.
pub const ACCUMULATION_URI: &str = "majordomus://accumulation";

/// Where the layer keeps its local half when the repository cannot say.
const LOCAL: &str = ".ai/local";

fn refused(e: WorktreeError) -> CapabilityError {
    match e.exit_code() {
        crate::worktree::EXIT_MISSING => CapabilityError::NotFound(e.to_string()),
        crate::worktree::EXIT_REFUSED => CapabilityError::Refused(e.to_string()),
        _ => CapabilityError::Internal(e.to_string()),
    }
}

/// The budgets this repository declares and where its local half is. An unreadable policy
/// is `doctor`'s finding; here it is the policy that declares nothing, so every quantity is
/// still measured and none is judged.
pub(crate) fn declared(root: &Path) -> (AccumulationPolicy, String) {
    let Ok(repo) = crate::repository::Repository::open(root) else {
        return (AccumulationPolicy::default(), LOCAL.to_string());
    };
    let policy = crate::policy::LoadedPolicy::load(&repo)
        .map(|l| l.policy.accumulation)
        .unwrap_or_default();
    (policy, repo.local_path())
}

fn accumulation_status(ctx: &Context, _: Empty) -> Result<AccumulationReport, CapabilityError> {
    let root = Path::new(&ctx.index.repository.root);
    let (policy, local) = declared(root);
    accumulation::measure(root, &policy, &local).map_err(refused)
}

/// The `accumulation` module: what grows on its own, and whether it is within budget.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "accumulation",
        title: "Accumulation",
        description: "What this repository has piled up without anyone deciding that it should, against the budgets its policy declares: the worktrees git registers, the local branches the trunk already reaches, and the free space on the volume under them. Each quantity carries its reading, its budget, its state — ok, over, unbudgeted when the policy declares no budget, unknown when the reading could not be taken — and the command that reduces it, which re-measures every candidate before it removes one. A provider's episode events read the same answer and say what is over budget before a worker's first tool call (ADR 0150).",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "accumulation.status",
                title: "Is anything piling up past its budget?",
                description: "Every accumulating quantity of this repository with its reading, the budget the policy declares for it under `accumulation:`, where it stands and the command that reduces it; how many are over budget and how many could not be measured; what the policy lets a crossed budget do (report, or reduce) and how often; and what the watch last removed here. Read from git and the volume on every call, with no subprocess per worktree. Reads only: nothing is removed by asking.",
                input: Empty,
                output: AccumulationReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_accumulation".into()),
                        resource: Some(McpResource { uri: ACCUMULATION_URI.into(), name: "accumulation".into() }),
                    }),
                    http: super::get("/api/v1/accumulation"),
                    cli: Some(CliExposure { path: vec!["accumulation".into()] }),
                },
                tags: ["accumulation", "worktree", "git", "disk", "operations"],
                cache: CachePolicy::Disabled,
                handler: accumulation_status,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accumulation::BudgetState;
    use crate::synthetic::SyntheticRepository;

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .status()
            .expect("git runs")
            .success();
        assert!(ok, "git {args:?}");
    }

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "accumulation");
        assert_eq!(m.capabilities.len(), 1);
        let entry = &m.capabilities[0];
        assert_eq!(entry.capability.id.as_str(), "accumulation.status");
        let exposure = &entry.capability.exposure;
        assert_eq!(
            exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_accumulation")
        );
        assert_eq!(
            exposure
                .mcp
                .as_ref()
                .and_then(|m| m.resource.as_ref())
                .map(|r| r.uri.as_str()),
            Some(ACCUMULATION_URI)
        );
        assert_eq!(
            exposure.cli.as_ref().map(|c| c.path.clone()),
            Some(vec!["accumulation".to_string()])
        );
    }

    /// Through the capability: a repository whose policy declares a budget is judged against
    /// it, one whose policy declares none is measured and not judged, and one git cannot
    /// read is refused rather than answered as empty.
    #[test]
    fn the_capability_judges_against_the_policy_the_repository_declares() {
        let repo = SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        assert!(
            accumulation_status(&ctx, Empty {}).is_err(),
            "no git, no answer"
        );

        git(repo.root(), &["init", "-q", "-b", "master"]);
        git(repo.root(), &["add", "-A"]);
        git(repo.root(), &["commit", "-q", "-m", "base"]);
        let report = accumulation_status(&ctx, Empty {}).unwrap();
        assert!(
            !report.watched,
            "a policy without the block watches nothing"
        );
        assert!(report
            .quantities
            .iter()
            .all(|q| q.state == BudgetState::Unbudgeted && q.measured.is_some()));

        let policy = repo.root().join(".ai/repo/policy.yaml");
        let mut text = std::fs::read_to_string(&policy).unwrap();
        text.push_str("\naccumulation:\n  watch: true\n  act: reduce\n  worktrees_max: 0\n  cooldown_minutes: 3\n");
        std::fs::write(&policy, text).unwrap();
        let report = accumulation_status(&ctx, Empty {}).unwrap();
        assert!(report.watched);
        assert_eq!(report.act, crate::accumulation::AccumulationAct::Reduce);
        assert_eq!(report.cooldown_minutes, 3);
        assert_eq!(
            report.over_ids(),
            ["worktrees"],
            "one checkout is more than none"
        );

        let nowhere = tempfile::tempdir().unwrap();
        let (absent, local) = declared(nowhere.path());
        assert_eq!(absent, AccumulationPolicy::default());
        assert_eq!(local, LOCAL);
    }
}
