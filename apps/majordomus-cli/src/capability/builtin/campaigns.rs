//! The `campaigns` module: the prompt packs staged before they are kept (ADR 0236, I4715).
//!
//! One capability, `campaigns.staged`, a read: every pack in the primary checkout's staging
//! places, reached from any linked worktree, with the state the tracked tree derives for it.
//! The judgement lives in [`crate::campaigns`]; every projection is derived from this
//! declaration.
//!
//! ```
//! use majordomus_cli::capability::builtin::modules;
//! let m = modules().into_iter().find(|m| m.id.as_str() == "campaigns").unwrap();
//! let c = &m.capabilities[0].capability;
//! assert_eq!(c.id.as_str(), "campaigns.staged");
//! assert_eq!(c.exposure.cli.as_ref().unwrap().path, ["campaigns", "staged"]);
//! ```

use crate::campaigns::{self, StagedReport};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::{get, mcp, Empty};

fn staged(ctx: &Context, _: Empty) -> Result<StagedReport, CapabilityError> {
    let repository = ctx.repository().map_err(|e| {
        CapabilityError::Internal(format!("the repository could not be re-read: {e}"))
    })?;
    let bound = crate::policy::LoadedPolicy::load(&repository)
        .ok()
        .and_then(|p| p.policy.campaigns.staged_max_age_days);
    let root = repository.root();
    Ok(campaigns::staged(
        root,
        campaigns::primary_checkout(root),
        std::time::SystemTime::now(),
        bound,
    ))
}

/// The `campaigns` module, composed into the registry by `builtin::modules`.
///
/// ```
/// use majordomus_cli::capability::builtin::campaigns;
/// use majordomus_cli::capability::Effect;
/// let m = campaigns::module();
/// let staged = &m.capabilities[0].capability;
/// assert_eq!(staged.execution.effect, Effect::Read);
/// assert_eq!(staged.exposure.http.as_ref().unwrap().path, "/api/v1/campaigns/staged");
/// let tool = staged.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref());
/// assert_eq!(tool, Some("majordomus_campaigns_staged"));
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "campaigns",
        title: "Prompt packs in progress",
        description: "The prompt packs staged in the primary checkout's git-ignored tmp/owner-packs/ before they are kept byte for byte under campaigns/, each with the state the tracked tree derives for it (ADR 0236).",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "campaigns.staged",
                title: "The prompt packs staged before they are kept",
                description: "Every pack in the staging places ADR 0236 declares — tmp/owner-packs/<pack>/ and the legacy tmp/prompt-packs/<pack>/ of the primary checkout, derived from the git common directory so that every linked worktree reads the same place — with a derived state: staged (campaigns/MANIFEST.sha256 records no file of it), kept (every staged file has the recorded bytes), diverged (a staged file differs or is not recorded), planned (kept and cited as campaigns/<pack> by a record under .ai/repo/project/ or an ADR), unplanned (kept and named in .ai/repo/ci/campaigns-unplanned.txt). Each pack carries its file count, first heading, newest modification time, age, the records that name it, and whether it is staged past the policy's campaigns.staged_max_age_days. A missing staging place is reported with its reason, never an error. Reads files and git; writes nothing and executes nothing a pack contains.",
                input: Empty,
                output: StagedReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_campaigns_staged"),
                    http: get("/api/v1/campaigns/staged"),
                    cli: Some(CliExposure { path: vec!["campaigns".into(), "staged".into()] }),
                },
                tags: ["campaigns", "provenance", "intake"],
                handler: staged,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    /// The declaration is the only place these names exist.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "campaigns");
        let exposure = &m.capabilities[0].capability.exposure;
        assert_eq!(exposure.http.as_ref().unwrap().path, "/api/v1/campaigns/staged");
        assert_eq!(exposure.cli.as_ref().unwrap().path, ["campaigns", "staged"]);
    }

    /// The handler answers for a repository: a synthetic one is no git work tree, so the
    /// answer is an empty list with the reason, never an error.
    #[test]
    fn the_handler_answers_with_the_reason_when_there_is_no_primary() {
        let repo = SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let report = staged(&ctx, Empty {}).unwrap();
        assert!(report.packs.is_empty());
        assert!(report.primary.is_some() || report.reason.is_some());
    }

    /// A repository whose manifest is gone by the time the capability runs is an internal
    /// error naming the re-read.
    #[test]
    fn a_repository_that_cannot_be_re_read_is_refused() {
        let repo = SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        std::fs::remove_file(repo.root().join(".ai/manifest.yaml")).unwrap();
        let err = staged(&ctx, Empty {}).unwrap_err();
        assert!(
            matches!(&err, CapabilityError::Internal(m) if m.contains("could not be re-read")),
            "{err:?}"
        );
    }
}
