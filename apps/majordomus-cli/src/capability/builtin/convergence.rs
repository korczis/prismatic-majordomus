//! The `convergence` module: is any of this repository's work held where it can be lost?
//!
//! One question, read-only, answered by [`crate::convergence`] — which composes the
//! worktree topology, the commits no remote reaches and the stash list, and gives one
//! verdict over them. Declaring it here is what puts it on MCP, on HTTP, in the OpenAPI
//! document, in the Cockpit and in the generated reference without any of those carrying a
//! route or a schema of its own.
//!
//! The completion invariant asks the same question as `no-stale-topology`, and reads this
//! answer rather than a second one.

use std::path::Path;

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::convergence::{self, ConvergenceReport};
use crate::worktree::WorktreeError;
use crate::{capability, module};

use super::Empty;

/// The URI under which the convergence of the repository is read as an MCP resource.
pub const CONVERGENCE_URI: &str = "majordomus://convergence";

fn refused(e: WorktreeError) -> CapabilityError {
    match e.exit_code() {
        crate::worktree::EXIT_MISSING => CapabilityError::NotFound(e.to_string()),
        crate::worktree::EXIT_REFUSED => CapabilityError::Refused(e.to_string()),
        _ => CapabilityError::Internal(e.to_string()),
    }
}

fn convergence_report(ctx: &Context, _: Empty) -> Result<ConvergenceReport, CapabilityError> {
    convergence::report(Path::new(&ctx.index.repository.root)).map_err(refused)
}

/// The `convergence` module: where the work is held.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "convergence",
        title: "Convergence",
        description: "Whether every unit of work in this repository is somewhere another worker could find it. A holding is anything that can hold work — a work tree with uncommitted files, a branch with commits, a stash — and each carries a disposition read from git: integrated (the trunk reaches it), published (a remote-tracking ref reaches it), local_only (committed on this disk and nowhere else) or uncommitted. The last two are at risk: they are invisible to every other worker, and a worker that stops is not a rollback. Measured offline, from this checkout alone; whether a pull request exists for a branch is a question for the forge and is not asked here.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "convergence.report",
                title: "Is any work held where it can be lost?",
                description: "Every holding of this repository with its disposition, the evidence read for it and the command that would move it out of danger, the tallies per disposition, how many holdings exist on one disk only, and the one verdict over them. Read from git on every call: a commit, a push or an editor's save changes the answer between two calls.",
                input: Empty,
                output: ConvergenceReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_convergence".into()),
                        resource: Some(McpResource { uri: CONVERGENCE_URI.into(), name: "convergence".into() }),
                    }),
                    http: super::get("/api/v1/convergence"),
                    cli: Some(CliExposure { path: vec!["convergence".into()] }),
                },
                tags: ["convergence", "git", "worktree", "integration"],
                cache: CachePolicy::Disabled,
                handler: convergence_report,
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
        assert_eq!(m.id.as_str(), "convergence");
        assert_eq!(m.capabilities.len(), 1);
        let entry = &m.capabilities[0];
        assert_eq!(entry.capability.id.as_str(), "convergence.report");
        let exposure = &entry.capability.exposure;
        assert_eq!(
            exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_convergence")
        );
        assert_eq!(
            exposure
                .mcp
                .as_ref()
                .and_then(|m| m.resource.as_ref())
                .map(|r| r.uri.as_str()),
            Some(CONVERGENCE_URI)
        );
        assert_eq!(
            exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/convergence")
        );
        assert_eq!(
            exposure.cli.as_ref().map(|c| c.path.clone()),
            Some(vec!["convergence".to_string()])
        );
    }

    /// The answer changes outside this process — a commit, a push, an editor's save — so
    /// caching it would serve a verdict about a repository that no longer exists.
    #[test]
    fn the_verdict_is_never_cached() {
        for e in module().capabilities {
            assert!(
                !e.capability.cache.is_enabled(),
                "{} must not cache",
                e.capability.id
            );
            assert!(
                e.capability.kind.is_read_only(),
                "{} writes",
                e.capability.id
            );
        }
    }
}
