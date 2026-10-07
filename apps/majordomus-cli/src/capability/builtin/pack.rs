//! The `pack` module: the tracked tree as token-bounded text shards for a reader whose file
//! search indexes text, with nothing in it that is not source.
//!
//! Two capabilities. `pack.plan` is the verdict before anything is written: what a profile
//! of `share/archive.yaml` carries, what it leaves out and why, how it is cut into shards and
//! every reason it cannot be built. `pack.verify` reads a pack that was written and refuses
//! one whose files are not what its manifest names, or that carries a binary, an artifact, a
//! link, a worktree or a leak. Writing the pack is `majordomus pack build`, which the command
//! line alone offers, because every projection of the registry is read-only. The selection
//! and the judgement live in [`crate::pack`].
//!
//! ```
//! use majordomus_cli::capability::builtin::modules;
//! let m = modules().into_iter().find(|m| m.id.as_str() == "pack").unwrap();
//! let ids: Vec<&str> = m.capabilities.iter().map(|c| c.capability.id.as_str()).collect();
//! assert_eq!(ids, ["pack.plan", "pack.verify"]);
//! ```

use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::pack::{PackPlan, PackVerdict, Planned, Profiles};
use crate::{capability, module};

use super::{get, mcp};

/// The input of `pack.plan`: which profile of `share/archive.yaml` to plan, or the
/// distribution's default one when none is named.
///
/// ```
/// use majordomus_cli::capability::builtin::pack::PackPlanInput;
/// let input = PackPlanInput { profile: Some("chatgpt".into()) };
/// assert_eq!(input.profile.as_deref(), Some("chatgpt"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackPlanInput {
    /// A profile of `share/archive.yaml`; its `default` when absent.
    #[serde(default)]
    pub profile: Option<String>,
}

impl BenchmarkCases for PackPlanInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default-profile", PackPlanInput { profile: None }),
            NamedCase::new(
                "chatgpt",
                PackPlanInput {
                    profile: Some("chatgpt".into()),
                },
            ),
        ]
    }
}

/// The input of `pack.verify`: the directory of a pack that was written, which must lie
/// inside the repository it was built from.
///
/// ```
/// use majordomus_cli::capability::builtin::pack::PackVerifyInput;
/// let input = PackVerifyInput { dir: "tmp/packs/x".into() };
/// assert_eq!(input.dir, "tmp/packs/x");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackVerifyInput {
    /// The pack's directory, inside the repository: relative to its root, or absolute
    /// under it.
    pub dir: String,
}

impl BenchmarkCases for PackVerifyInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // no pack exists in a fresh checkout, so the route answers with the unmeasured
        // verdict, which still gives the required parameter its example
        vec![NamedCase::new(
            "absent",
            PackVerifyInput {
                dir: "tmp/packs/absent".into(),
            },
        )]
    }
}

fn share_dir(ctx: &Context, root: &Path) -> Result<PathBuf, CapabilityError> {
    crate::share::Share::locate(ctx.index.share.as_deref(), root)
        .map(|s| s.dir().to_path_buf())
        .map_err(|e| CapabilityError::Internal(e.to_string()))
}

/// The plan of `profile` for the repository the context reads, with the content it was
/// made from: what `pack.plan` answers and what `majordomus pack build` writes. A
/// distribution that cannot be found is a plan that could not be measured, as an unreadable
/// profiles file is.
pub(crate) fn planned(ctx: &Context, profile: Option<&str>) -> Planned {
    let root = PathBuf::from(&ctx.index.repository.root);
    match share_dir(ctx, &root) {
        Ok(share) => crate::pack::plan(&root, &share, profile),
        Err(e) => crate::pack::unmeasured(profile.unwrap_or(""), e.to_string()),
    }
}

fn plan(ctx: &Context, input: PackPlanInput) -> Result<PackPlan, CapabilityError> {
    Ok(planned(ctx, input.profile.as_deref()).plan)
}

/// The directory a verify input names, refused when it leaves the repository.
fn inside(root: &Path, dir: &str) -> Result<PathBuf, CapabilityError> {
    let p = Path::new(dir);
    let rel = if p.is_absolute() {
        p.strip_prefix(root)
            .map_err(|_| CapabilityError::Refused(format!("{dir} is not inside the repository")))?
            .to_path_buf()
    } else {
        p.to_path_buf()
    };
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(CapabilityError::Refused(format!(
            "{dir} leaves the repository; a pack is verified where it was built"
        )));
    }
    Ok(root.join(rel))
}

fn verify(ctx: &Context, input: PackVerifyInput) -> Result<PackVerdict, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let dir = inside(&root, &input.dir)?;
    let share = share_dir(ctx, &root)?;
    let profiles = Profiles::load(&share, &root).map_err(CapabilityError::Internal)?;
    let mut v = crate::pack::verify(&dir, &profiles, &root);
    v.dir = input.dir;
    Ok(v)
}

/// The module: `pack.plan` and `pack.verify`, each exposed over MCP, HTTP and the command
/// line; writing a pack stays with `majordomus pack build`.
///
/// ```
/// use majordomus_cli::capability::builtin::pack::module;
/// let m = module();
/// assert_eq!(m.id.as_str(), "pack");
/// let tools: Vec<_> = m
///     .capabilities
///     .iter()
///     .filter_map(|c| c.capability.exposure.mcp.as_ref()?.tool.clone())
///     .collect();
/// assert_eq!(tools, ["majordomus_pack_plan", "majordomus_pack_verify"]);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "pack",
        title: "Source pack",
        description: "The tracked tree as a few token-bounded Markdown shards a language model's file search can index, with an index that orients the reader and a manifest that proves afterwards what left the machine: the git index's blobs only, never the working tree, and never a binary, a gitlink or worktree, a symbolic link, build output or a credential, whatever the index holds.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "pack.plan",
                title: "Plan a source pack",
                description: "What a profile of share/archive.yaml would pack: the files carried and their bytes and o200k_base tokens, every file left out by reason (worktree, link, artifact, binary, derived, excluded; derived counted, the rest listed), the shards it is cut into within the profile's token budget and file count, and every finding that refuses the build — a file over the budget, too many shards, a machine path or credential in a selected file, an empty selection, a profile without limits. Reads git's index and blobs only; writes nothing.",
                input: PackPlanInput,
                output: PackPlan,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_pack_plan"),
                    http: get("/api/v1/pack/plan"),
                    cli: Some(CliExposure { path: vec!["pack".into(), "plan".into()] }),
                },
                tags: ["pack", "archive", "review", "privacy"],
                handler: plan,
            },
            capability! {
                id: "pack.verify",
                title: "Verify a written source pack",
                description: "Read a pack directory inside the repository against its pack.json: every file it names is present with its digest and nothing else is, every carried file's content has the digest its marker records, no carried path is a binary, an artifact, a link or a gitlink under the profile the manifest names, no file is over the profile's token budget or holds a NUL byte or a leak, and there are no more files than the profile allows. An unreadable manifest is unmeasured, never a pass.",
                input: PackVerifyInput,
                output: PackVerdict,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_pack_verify"),
                    http: get("/api/v1/pack/verify"),
                    cli: Some(CliExposure { path: vec!["pack".into(), "verify".into()] }),
                },
                tags: ["pack", "archive", "privacy", "evidence"],
                handler: verify,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verify_input_never_leaves_the_repository() {
        let root = Path::new("/r/repo");
        assert_eq!(
            inside(root, "tmp/packs/a").unwrap(),
            root.join("tmp/packs/a")
        );
        assert_eq!(inside(root, "/r/repo/tmp/p").unwrap(), root.join("tmp/p"));
        assert!(inside(root, "../other").is_err());
        assert!(inside(root, "tmp/../../x").is_err());
        assert!(inside(root, "/etc").is_err());
    }

    /// A context whose distribution is named and is not there: the one way a share can
    /// fail to be found whatever the environment of the run holds.
    #[test]
    fn a_distribution_that_cannot_be_found_is_unmeasured_and_verifies_nothing() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let built = repo.context().expect("a context");
        let mut index = repo.index().expect("an index");
        index.share = Some(PathBuf::from("/nonexistent/share"));
        let ctx = Context::new(std::sync::Arc::new(index), built.registry.clone());

        let p = plan(&ctx, PackPlanInput::default()).expect("answered");
        assert!(!p.measured && !p.passes, "{p:?}");
        assert!(p.reason.is_some_and(|r| r.contains("/nonexistent/share")));

        let input = PackVerifyInput {
            dir: "tmp/packs/x".into(),
        };
        let e = verify(&ctx, input).expect_err("refused");
        assert!(matches!(e, CapabilityError::Internal(_)), "{e}");
    }

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        let plan = &m.capabilities[0].capability.exposure;
        assert_eq!(plan.http.as_ref().unwrap().path, "/api/v1/pack/plan");
        assert_eq!(plan.cli.as_ref().unwrap().path, ["pack", "plan"]);
        let verify = &m.capabilities[1].capability.exposure;
        assert_eq!(
            verify.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_pack_verify")
        );
    }
}
