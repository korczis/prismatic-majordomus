//! The `distribution` module: how this project is packaged, published and installed, and
//! what this particular build is.
//!
//! Every value comes from the distribution model the process was started with
//! (`share/distribution.yaml`, carried on the index) and from the release records the
//! repository holds. Nothing here decides a platform, an artifact name or a URL; the
//! module exists so that the same answers reach a person at a command line, a script over
//! HTTP, an assistant over MCP and the cockpit, without any of them restating a fact.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::distribution::release::{Channel, Releases};
use crate::distribution::{Arch, Libc, Model, Os, TargetStatus};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI the model answers on, as a resource.
pub const DISTRIBUTION_URI: &str = "majordomus://distribution";

/// One target, as every projection shows it: what it is, what it is called in prose, and
/// the artifact name the naming function derives for a tag yet to be chosen.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TargetView {
    /// The target's id in the model.
    pub id: String,
    /// How it is written in prose: `Linux x86_64 musl`.
    pub title: String,
    /// The operating system.
    pub os: Os,
    /// The architecture.
    pub arch: Arch,
    /// The C library, on Linux.
    pub libc: Option<Libc>,
    /// The Rust target triple.
    pub rust_target: String,
    /// What the project promises about it.
    pub status: TargetStatus,
    /// Why, when it is not built.
    pub reason: Option<String>,
    /// The artifact name, with `{tag}` where a release's tag goes.
    pub artifact: String,
}

/// The distribution, as a person or a script asks for it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DistributionReport {
    /// The command a person types after installing.
    pub binary: String,
    /// `owner/name`, the only host release assets come from.
    pub repository: String,
    /// The installer's canonical URL.
    pub installer_url: String,
    /// The one-line install command, composed from its parts.
    pub install_command: String,
    /// The same, initialising the repository afterwards.
    pub install_and_init_command: String,
    /// What a person runs next.
    pub next_command: String,
    /// The digest algorithm the installer verifies.
    pub checksum: String,
    /// Where the launchers go by default.
    pub install_dir: String,
    /// Where the versioned trees go by default.
    pub prefix: String,
    /// Where the stable release metadata is published.
    pub latest_url: String,
    /// How many targets a release builds.
    pub supported: usize,
    /// Every declared target, in the model's order.
    pub targets: Vec<TargetView>,
}

/// One release, as every projection shows it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReleaseView {
    /// The git tag.
    pub tag: String,
    /// The semantic version.
    pub version: String,
    /// Whether an unpinned installation may resolve it.
    pub channel: Channel,
    /// When it was published, UTC.
    pub published_at: String,
    /// The commit it was built from.
    pub commit: String,
    /// True when it has been withdrawn.
    pub yanked: bool,
    /// How many artifacts it published.
    pub artifacts: usize,
    /// Where its public metadata is served.
    pub metadata_url: String,
}

/// The releases this repository recorded, and the one an unpinned installation resolves to.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReleasesReport {
    /// How many records there are.
    pub count: usize,
    /// The release an unpinned installation resolves to, when there is one.
    pub latest: Option<ReleaseView>,
    /// Every record, newest first.
    pub releases: Vec<ReleaseView>,
}

/// What this executable is: enough to tell two builds apart without asking a repository.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct BuildReport {
    /// The version of the crate this executable was built from.
    pub version: String,
    /// The Rust target triple it was built for; the model names a target by the same string.
    pub target: String,
    /// The cargo profile.
    pub profile: String,
    /// The commit, or `unknown` when it was built outside a work tree.
    pub commit: String,
    /// The target of the model this build matches, when the model declares one for the triple.
    pub distribution_target: Option<String>,
}

/// Which artifact a target and a tag name.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactInput {
    /// A target's id or its Rust target triple.
    pub target: String,
    /// The tag, `v` and a version. `{tag}` asks for the name with the placeholder left in.
    pub tag: String,
}

impl BenchmarkCases for ArtifactInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        ctx.index
            .distribution
            .as_ref()
            .and_then(|m| m.published().next())
            .map(|t| {
                vec![NamedCase::new(
                    "first-published-target",
                    ArtifactInput {
                        target: t.rust_target.clone(),
                        tag: "v0.0.0".into(),
                    },
                )]
            })
            .unwrap_or_default()
    }
}

/// The artifact a target and a tag name, and the directory it unpacks into.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReleaseArtifactView {
    /// The target's id in the model.
    pub target: String,
    /// The Rust target triple.
    pub rust_target: String,
    /// The tag the name was derived for.
    pub tag: String,
    /// The archive's file name.
    pub name: String,
    /// The directory the archive unpacks into.
    pub root: String,
    /// Where a release publishes it, when the tag is a real one.
    pub url: String,
}

/// The model this process was started with, or the reason there is none.
fn model(ctx: &Context) -> Result<&Model, CapabilityError> {
    ctx.index.distribution.as_ref().ok_or_else(|| {
        CapabilityError::NotFound(
            "this installation carries no share/distribution.yaml, so it declares no distribution"
                .into(),
        )
    })
}

fn distribution(ctx: &Context, _: Empty) -> Result<DistributionReport, CapabilityError> {
    let m = model(ctx)?;
    Ok(DistributionReport {
        binary: m.project.binary.clone(),
        repository: m.project.repository.clone(),
        installer_url: m.installer_url(),
        install_command: m.install_command(),
        install_and_init_command: m.install_and_init_command(),
        next_command: format!("{} init", m.project.binary),
        checksum: m.archive.checksum.clone(),
        install_dir: m.installer.install_dir.clone(),
        prefix: m.installer.prefix.clone(),
        latest_url: m.release_url(crate::distribution::release::LATEST),
        supported: m.published().count(),
        targets: m
            .targets
            .iter()
            .map(|t| TargetView {
                id: t.id.clone(),
                title: t.title(&m.titles),
                os: t.os,
                arch: t.arch,
                libc: t.libc,
                rust_target: t.rust_target.clone(),
                status: t.status,
                reason: t.reason.clone(),
                artifact: t.artifact_name(
                    &m.project,
                    &m.archive,
                    crate::distribution::render::TAG_PLACEHOLDER,
                ),
            })
            .collect(),
    })
}

fn view(m: &Model, r: &crate::distribution::Release) -> ReleaseView {
    ReleaseView {
        tag: r.tag.clone(),
        version: r.version.clone(),
        channel: r.channel,
        published_at: r.published_at.clone(),
        commit: r.commit.clone(),
        yanked: r.yanked,
        artifacts: r.artifacts.len(),
        metadata_url: m.release_url(&r.tag),
    }
}

fn releases(ctx: &Context, _: Empty) -> Result<ReleasesReport, CapabilityError> {
    let m = model(ctx)?;
    let records = Releases::from_index(&ctx.index).map_err(CapabilityError::Internal)?;
    Ok(ReleasesReport {
        count: records.releases.len(),
        latest: records.latest_stable().map(|r| view(m, r)),
        releases: records.releases.iter().map(|r| view(m, r)).collect(),
    })
}

fn build(ctx: &Context, _: Empty) -> Result<BuildReport, CapabilityError> {
    let distribution_target = ctx
        .index
        .distribution
        .as_ref()
        .and_then(|m| m.targets.iter().find(|t| t.rust_target == crate::TARGET))
        .map(|t| t.id.clone());
    Ok(BuildReport {
        version: crate::VERSION.to_string(),
        target: crate::TARGET.to_string(),
        profile: crate::PROFILE.to_string(),
        commit: crate::COMMIT.to_string(),
        distribution_target,
    })
}

fn artifact(ctx: &Context, input: ArtifactInput) -> Result<ReleaseArtifactView, CapabilityError> {
    let m = model(ctx)?;
    let t = m
        .targets
        .iter()
        .find(|t| t.id == input.target || t.rust_target == input.target)
        .ok_or_else(|| {
            CapabilityError::NotFound(format!(
                "the distribution model declares no target `{}`",
                input.target
            ))
        })?;
    if !t.status.is_published() {
        return Err(CapabilityError::Refused(format!(
            "`{}` is declared and not built: {}",
            t.id,
            t.reason.as_deref().unwrap_or("no reason recorded")
        )));
    }
    let tag = input.tag;
    if tag != crate::distribution::render::TAG_PLACEHOLDER
        && !(tag.starts_with('v') && tag[1..].starts_with(|c: char| c.is_ascii_digit()))
    {
        return Err(CapabilityError::InvalidInput(format!(
            "`{tag}` is not a tag: a tag is `v` followed by a version"
        )));
    }
    let name = t.artifact_name(&m.project, &m.archive, &tag);
    Ok(ReleaseArtifactView {
        target: t.id.clone(),
        rust_target: t.rust_target.clone(),
        url: format!("{}{tag}/{name}", m.project.download_prefix()),
        root: t.archive_root(&m.project, &tag),
        name,
        tag,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "distribution",
        title: "Distribution",
        description: "How this project is packaged, published and installed: the platforms a release builds, the artifact names the one naming function derives, the installer's canonical command, the releases that were published, and what this build itself is. Every answer comes from share/distribution.yaml and the release records; no surface here states a fact of its own.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "distribution.model",
                title: "The distribution model",
                description: "The one-line install command, where an installation goes, and every declared target with the artifact name it derives. This is what the installation page, the landing page's install block and the cockpit's install card render; none of them holds a platform list of its own.",
                input: Empty,
                output: DistributionReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_distribution"),
                    http: get("/api/v1/distribution"),
                    cli: Some(CliExposure { path: vec!["distribution".into(), "show".into()] }),
                },
                tags: ["distribution", "install", "introspection"],
                handler: distribution,
            },
            capability! {
                id: "distribution.releases",
                title: "Published releases",
                description: "Every release this repository recorded, newest first, and the one an unpinned installation resolves to: the highest version among the stable, unwithdrawn records. The pointer is derived here and never authored anywhere.",
                input: Empty,
                output: ReleasesReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_releases"),
                    http: get("/api/v1/distribution/releases"),
                    cli: Some(CliExposure { path: vec!["distribution".into(), "releases".into()] }),
                },
                tags: ["distribution", "release"],
                handler: releases,
            },
            capability! {
                id: "distribution.build",
                title: "This build",
                description: "What this executable is: the version of the crate it was built from, the Rust target triple, the profile, and the commit — all compiled in at build time, so an installed binary answers without a repository, a toolchain or git.",
                input: Empty,
                output: BuildReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_build"),
                    http: get("/api/v1/distribution/build"),
                    cli: Some(CliExposure { path: vec!["distribution".into(), "build".into()] }),
                },
                tags: ["distribution", "introspection"],
                handler: build,
            },
            capability! {
                id: "distribution.artifact",
                title: "The artifact of a target",
                description: "The archive name a target and a tag derive, the directory it unpacks into, and where a release publishes it. The one naming function answers; the release pipeline asks it rather than composing a name in a workflow file.",
                input: ArtifactInput,
                output: ReleaseArtifactView,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_artifact"),
                    http: get("/api/v1/distribution/artifact"),
                    cli: Some(CliExposure { path: vec!["distribution".into(), "artifact".into()] }),
                },
                tags: ["distribution", "release"],
                handler: artifact,
            },
        ],
    }
}
