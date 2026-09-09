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
use crate::distribution::{Arch, Libc, Model, Os, Status};
use crate::{capability, module};

use super::{get, mcp, Empty};

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
    pub status: Status,
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
pub struct ReleaseArtifactInput {
    /// A target's id or its Rust target triple.
    pub target: String,
    /// The tag, `v` and a version. `{tag}` asks for the name with the placeholder left in.
    pub tag: String,
}

impl BenchmarkCases for ReleaseArtifactInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        ctx.index
            .distribution
            .as_ref()
            .and_then(|m| m.published().next())
            .map(|t| {
                vec![NamedCase::new(
                    "first-published-target",
                    ReleaseArtifactInput {
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

/// The state of one check in the installability report. Three states and no more: a check
/// either holds, does not, or could not be made from what this process can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CheckState {
    /// The check holds.
    Ok,
    /// The check does not hold, and the public installation is affected.
    Failed,
    /// The check could not be made here; it says nothing either way.
    Unknown,
}

/// One check: what was asked, what was seen, and — when it does not hold — why, and the
/// command that changes it. The `cause` and `next` fields exist so that a report is
/// actionable without a second document; a check that fails without naming its remedy is
/// a diagnostic nobody can act on.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InstallCheck {
    /// A short stable name: `model`, `version`, `targets`, `release`, `artifacts`, `metadata`.
    pub id: String,
    /// Whether it holds.
    pub state: CheckState,
    /// What was observed, in one line.
    pub observed: String,
    /// Why it does not hold. Absent when it does.
    pub cause: Option<String>,
    /// The command or action that would change it. Absent when there is nothing to do.
    pub next: Option<String>,
}

/// Whether the advertised one-line installation works right now, and if not, what is missing.
///
/// This answers the operator's actual question — *can a machine that has never seen this
/// project install it with the published command?* — from local state alone. It performs no
/// network access: a repository can be offline and this still answers, because everything it
/// needs is the distribution model and the release records the repository itself carries.
/// What it cannot see it reports as `unknown` rather than guessing; the public half of the
/// question is answered by the release workflow's smoke phase, which installs from the
/// published URL on every native runner and is the only thing that proves the public path.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InstallabilityReport {
    /// True when every check holds: a stable release is recorded and complete.
    pub installable: bool,
    /// One line for a person: what the state is, in the project's own terms.
    pub summary: String,
    /// The version this tree would release.
    pub local_version: String,
    /// The tag an unpinned installation resolves to, when a stable release is recorded.
    pub stable_tag: Option<String>,
    /// How many targets a release must publish for that release to be complete.
    pub required_targets: usize,
    /// How many of them the stable release actually publishes.
    pub published_artifacts: usize,
    /// The command a person would run, whether or not it currently works.
    pub install_command: String,
    /// Where the installer is served.
    pub installer_url: String,
    /// Where the stable pointer is served.
    pub latest_url: String,
    /// Every check, in the order they are worth reading.
    pub checks: Vec<InstallCheck>,
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

fn artifact(
    ctx: &Context,
    input: ReleaseArtifactInput,
) -> Result<ReleaseArtifactView, CapabilityError> {
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

/// Whether the published one-line installation works, from what this repository can see.
///
/// The checks are ordered the way the pipeline is: the model must be valid before a target
/// list means anything, targets must exist before a release can be complete, a release must
/// be recorded before metadata can be published, and the metadata must agree with the model
/// before an installer can use it. The first failure is the one that matters, and the
/// summary names it; the rest are still reported, because an operator reading this wants the
/// whole shape and not one line at a time.
fn status(ctx: &Context, _: Empty) -> Result<InstallabilityReport, CapabilityError> {
    let m = model(ctx)?;
    let records = Releases::from_index(&ctx.index).map_err(CapabilityError::Internal)?;
    Ok(installability(m, &records))
}

/// The report itself, over a model and a set of records and nothing else.
///
/// Separated from the capability so that the answer can be examined for states this
/// repository is not in — a complete release, a partial one, a model that publishes
/// nothing — without constructing an index for each. The capability above is the only
/// caller in the executable; the tests are the rest.
pub(crate) fn installability(m: &Model, records: &Releases) -> InstallabilityReport {
    let required = m.published().count();
    let mut checks = Vec::new();

    // The model itself. `findings` is the same function `distribution validate` reports and
    // the release workflow refuses a tag over; this asks it rather than restating any of it.
    let model_findings = records.findings(m);
    let model_ok = required > 0;
    checks.push(InstallCheck {
        id: "model".into(),
        state: if model_ok {
            CheckState::Ok
        } else {
            CheckState::Failed
        },
        observed: format!(
            "{} declared target(s), {required} published",
            m.targets.len()
        ),
        cause: (!model_ok)
            .then(|| "the model publishes no target, so a release could build nothing".into()),
        next: (!model_ok)
            .then(|| "declare a target with `status: supported` in share/distribution.yaml".into()),
    });

    // The version this tree would release. It is the crate's, compiled in, so it is the same
    // string the released executable would print.
    let local_version = crate::VERSION.to_string();
    checks.push(InstallCheck {
        id: "version".into(),
        state: CheckState::Ok,
        observed: format!("this tree releases v{local_version}"),
        cause: None,
        next: None,
    });

    // A stable release, or none. This is the check that was failing while the public
    // installer answered "no stable release is currently published".
    let latest = records.latest_stable();
    let stable_tag = latest.map(|r| r.tag.clone());
    checks.push(InstallCheck {
        id: "release".into(),
        state: if latest.is_some() {
            CheckState::Ok
        } else {
            CheckState::Failed
        },
        observed: match &stable_tag {
            Some(t) => format!("the stable channel resolves to {t}"),
            None => format!(
                "no stable release is recorded ({} record(s) in total)",
                records.releases.len()
            ),
        },
        cause: latest.is_none().then(|| {
            "there is no release record for the stable channel, so nothing derives the public \
             metadata an unpinned installation reads"
                .into()
        }),
        next: latest
            .is_none()
            .then(|| format!("git tag v{local_version} && git push origin v{local_version}")),
    });

    // Completeness: a release is all of its supported targets or it is not a release.
    let published_artifacts = latest.map_or(0, |r| r.artifacts.len());
    checks.push(InstallCheck {
        id: "artifacts".into(),
        state: match latest {
            None => CheckState::Unknown,
            Some(_) if published_artifacts >= required => CheckState::Ok,
            Some(_) => CheckState::Failed,
        },
        observed: match latest {
            None => "no release to count artifacts for".into(),
            Some(_) => format!("{published_artifacts} of {required} required artifact(s)"),
        },
        cause: latest
            .filter(|_| published_artifacts < required)
            .map(|_| "the stable release does not publish every target the model requires; a partial release is not a release".into()),
        next: latest
            .filter(|_| published_artifacts < required)
            .map(|_| "withdraw the record (yanked: true) and release again; the pipeline refuses to publish a partial release, so a record like this was not written by it".into()),
    });

    // Whether every record agrees with the model. A record that disagrees renders metadata
    // the installer would refuse, so this is part of installability and not merely of hygiene.
    checks.push(InstallCheck {
        id: "metadata".into(),
        state: if model_findings.is_empty() {
            CheckState::Ok
        } else {
            CheckState::Failed
        },
        observed: if model_findings.is_empty() {
            format!("{} record(s) agree with the model", records.releases.len())
        } else {
            format!(
                "{} finding(s): {}",
                model_findings.len(),
                model_findings.join("; ")
            )
        },
        cause: (!model_findings.is_empty()).then(|| {
            "a release record states something the distribution model does not derive".into()
        }),
        next: (!model_findings.is_empty()).then(|| "majordomus distribution validate".into()),
    });

    // The public half is deliberately not guessed at. Nothing here reaches the network, so
    // the only honest thing to say about the served bytes is who proves them.
    checks.push(InstallCheck {
        id: "public".into(),
        state: CheckState::Unknown,
        observed: "not checked here; this report reaches no network".into(),
        cause: None,
        next: Some(format!(
            "curl -fsSL {} | head -1",
            m.release_url(crate::distribution::release::LATEST)
        )),
    });

    let installable = checks.iter().all(|c| c.state != CheckState::Failed);
    let summary = if installable {
        match &stable_tag {
            Some(t) => format!(
                "{t} is recorded, complete and consistent; the published command installs it"
            ),
            None => "installable".into(),
        }
    } else {
        checks
            .iter()
            .find(|c| c.state == CheckState::Failed)
            .and_then(|c| c.cause.clone())
            .unwrap_or_else(|| "the published installation command does not currently work".into())
    };

    InstallabilityReport {
        installable,
        summary,
        local_version,
        stable_tag,
        required_targets: required,
        published_artifacts,
        install_command: m.install_command(),
        installer_url: m.installer_url(),
        latest_url: m.release_url(crate::distribution::release::LATEST),
        checks,
    }
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
                id: "distribution.status",
                title: "Whether the published installation works",
                description: "Whether a machine that has never seen this project can install it right now with the advertised one-line command, and when it cannot, which link in the chain is missing and what changes it. Derived from the distribution model and the release records alone: it reaches no network, so it is as fast as any other local query and answers offline. The served bytes are proved by the release pipeline's smoke phase, not guessed at here.",
                input: Empty,
                output: InstallabilityReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_install_status"),
                    http: get("/api/v1/distribution/status"),
                    cli: Some(CliExposure { path: vec!["distribution".into(), "status".into()] }),
                },
                tags: ["distribution", "install", "release", "diagnostics"],
                handler: status,
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
                input: ReleaseArtifactInput,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist, and every projection — the MCP
    /// tool, the HTTP route, the OpenAPI operation, the benchmark target — is derived from
    /// it. A refactor that dropped an exposure or renamed a route would still compile, and
    /// the suites that exercise the behaviour behind it would still pass. This is the
    /// assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "distribution");
        let expected: &[(&str, &str, &str)] = &[
            (
                "distribution.model",
                "majordomus_distribution",
                "/api/v1/distribution",
            ),
            (
                "distribution.status",
                "majordomus_install_status",
                "/api/v1/distribution/status",
            ),
            (
                "distribution.releases",
                "majordomus_releases",
                "/api/v1/distribution/releases",
            ),
            (
                "distribution.build",
                "majordomus_build",
                "/api/v1/distribution/build",
            ),
            (
                "distribution.artifact",
                "majordomus_artifact",
                "/api/v1/distribution/artifact",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
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
}
