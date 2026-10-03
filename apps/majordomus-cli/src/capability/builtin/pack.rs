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
use crate::pack::{PackPlan, PackVerdict, Profiles};
use crate::{capability, module};

use super::{get, mcp};

/// The input of `pack.plan`.
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

/// The input of `pack.verify`.
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

fn plan(ctx: &Context, input: PackPlanInput) -> Result<PackPlan, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let share = share_dir(ctx, &root)?;
    Ok(crate::pack::plan(&root, &share, input.profile.as_deref()).plan)
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

/// The module.
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
