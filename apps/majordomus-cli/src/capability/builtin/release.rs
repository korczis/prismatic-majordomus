//! The `release` module: what this repository would publish next, why it must carry the
//! version it carries, and what its changelog says.
//!
//! Every value comes from [`crate::release::Engine`], which reads the crate's version, the
//! release records, the change records, the committed contract snapshot and the same
//! snapshot at the baseline tag. Nothing here computes a version, classifies a change or
//! renders a changelog of its own: the module exists so that the same answers reach a
//! person at a command line, a script over HTTP, an assistant over MCP and the Cockpit's
//! version display, without any of them restating a fact.
//!
//! Every capability here is read-only. Preparing a release writes to the repository, and
//! writing to the repository is not something a capability does — `majordomus release
//! prepare` is a command of the executable, like `generate`, and it asks this same engine.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::release::change::Change;
use crate::release::changelog::Section;
use crate::release::diff::{CompatibilityImpact, ContractChange, ContractDiff};
use crate::release::manifest::ReleaseManifest;
use crate::release::version::{Bump, Version};
use crate::release::{Baseline, Diagnostic, Engine, ReleaseState, VersionReport};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI the release state answers on, as a resource.
pub const RELEASE_URI: &str = "majordomus://release";

/// Build the engine for this repository.
///
/// The one construction. Every capability below calls it, so a change to what the engine
/// reads reaches all of them; a capability that built its own would be a second reading of
/// the release state.
fn engine(ctx: &Context) -> Result<Engine, CapabilityError> {
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    Engine::load(&root, &ctx.index, Some(&ctx.registry)).map_err(CapabilityError::Internal)
}

/// Why a version has to be at least what it has to be.
///
/// The whole chain in one value: the baseline it was measured against, the changes that
/// were found, the impact they add up to, the policy sentence that maps that impact to a
/// bump, and the version that comes out. A reader who disagrees with the verdict can see
/// exactly which step they disagree with.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReleaseExplanation {
    /// What this tree would release.
    pub current: Version,
    /// Which release the contract was measured against.
    pub baseline: Baseline,
    /// What the change costs, when it could be computed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<CompatibilityImpact>,
    /// The bump the policy makes of that impact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_bump: Option<Bump>,
    /// The lowest version a release may declare.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<Version>,
    /// The version this tree would publish.
    pub target: Version,
    /// The policy sentence that produced the bump, in the policy's own words.
    pub policy: String,
    /// The changes that decided it: every change carrying the strongest impact found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deciding: Vec<ContractChange>,
    /// The surfaces of the contract a reader would otherwise take for compared, and which
    /// were not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncompared: Vec<String>,
    /// One line a person reads first.
    pub summary: String,
}

/// Which slice of the contract diff to answer with.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContractDiffInput {
    /// Only the changes carrying this impact: `none`, `patch`, `additive` or `breaking`.
    #[serde(default)]
    pub impact: Option<CompatibilityImpact>,
    /// Only the changes on this surface: `capability`, `command`, `document-kind`,
    /// `target`.
    #[serde(default)]
    pub surface: Option<String>,
}

impl BenchmarkCases for ContractDiffInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "whole-diff",
                ContractDiffInput {
                    impact: None,
                    surface: None,
                },
            ),
            NamedCase::new(
                "breaking-only",
                ContractDiffInput {
                    impact: Some(CompatibilityImpact::Breaking),
                    surface: None,
                },
            ),
            // `surface` also needs an example, and `capability` is one of the four literal
            // surfaces the handler always accepts — it does not depend on a diff existing,
            // so this case answers the same in every repository.
            NamedCase::new(
                "capability-surface",
                ContractDiffInput {
                    impact: None,
                    surface: Some("capability".into()),
                },
            ),
        ]
    }
}

/// The contract diff, or the reason there is none.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContractDiffReport {
    /// Which release the contract was measured against.
    pub baseline: Baseline,
    /// The diff, when a baseline contract could be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<ContractDiff>,
    /// Why there is no diff, when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Which changelog to answer with.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ChangelogInput")]
pub struct ChangelogInput {
    /// Only this version's section; the whole changelog when absent.
    #[serde(default)]
    pub version: Option<Version>,
}

impl BenchmarkCases for ChangelogInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases = vec![NamedCase::new("whole", ChangelogInput { version: None })];
        // `version` needs an example too, and the only version this handler is guaranteed
        // to answer is one a release record actually names: the changelog's `releases`
        // section is built from the same records, so the latest stable one always has a
        // section to return.
        if let Some(version) = crate::distribution::release::Releases::from_index(ctx.index)
            .ok()
            .and_then(|releases| releases.latest_stable().and_then(|r| r.version.parse().ok()))
        {
            cases.push(NamedCase::new(
                "published-version",
                ChangelogInput {
                    version: Some(version),
                },
            ));
        }
        cases
    }
}

/// The changelog, and the Markdown a reader sees.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ChangelogReport")]
pub struct ChangelogReport {
    /// How many change records the repository holds.
    pub records: usize,
    /// The unreleased section, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreleased: Option<Section>,
    /// Every published version's section, newest first. One section when a version was
    /// asked for.
    pub releases: Vec<Section>,
    /// The document as `CHANGELOG.md` carries it, so that a caller rendering it does not
    /// have to know how a section becomes a heading.
    pub markdown: String,
}

/// Which release to plan.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleasePlanInput {
    /// The version to plan for; the one this tree would publish when absent. A version
    /// below the minimum the change requires is refused here rather than described, so
    /// that a plan is never a description of something that cannot happen.
    #[serde(default)]
    pub version: Option<Version>,
}

impl BenchmarkCases for ReleasePlanInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases = vec![NamedCase::new("target", ReleasePlanInput { version: None })];
        // `plan` refuses a version below what the change requires, and that minimum moves
        // with the repository's own history — a published version, or any version fixed
        // here by hand, would start failing the moment a breaking change landed without a
        // bump. The one version every state tolerates is the target this tree would
        // actually publish, which is what the `None` case above resolves to as well; reading
        // it from the engine gives `version` a real example without asking a second,
        // possibly-refused question. The registry is not needed to compute it — only the
        // stale-surface diagnostic, which no benchmark case reads, does.
        let root = std::path::PathBuf::from(&ctx.index.repository.root);
        if let Ok(engine) = Engine::load(&root, ctx.index, None) {
            cases.push(NamedCase::new(
                "explicit-target",
                ReleasePlanInput {
                    version: Some(engine.target_version()),
                },
            ));
        }
        cases
    }
}

/// Which release manifest to answer with.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifestInput {
    /// The version; the one this tree would publish when absent.
    #[serde(default)]
    pub version: Option<Version>,
}

impl BenchmarkCases for ReleaseManifestInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases = vec![NamedCase::new(
            "target",
            ReleaseManifestInput { version: None },
        )];
        // `manifest` builds a document for whatever version it is given — it never refuses
        // one — so the published version is a better example than an arbitrary one: it
        // comes back with a release record, a commit and artifacts attached, which is what
        // a reader of this route actually wants to see.
        if let Some(version) = crate::distribution::release::Releases::from_index(ctx.index)
            .ok()
            .and_then(|releases| releases.latest_stable().and_then(|r| r.version.parse().ok()))
        {
            cases.push(NamedCase::new(
                "published-version",
                ReleaseManifestInput {
                    version: Some(version),
                },
            ));
        }
        cases
    }
}

/// The verdict of every release check, and every reason it did not hold.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReleaseCheckReport {
    /// True when nothing stands in the way of publishing the target version.
    pub ok: bool,
    /// The version this tree would publish.
    pub target: Version,
    /// How far along the release is.
    pub readiness: String,
    /// One line for a person.
    pub summary: String,
    /// Everything wrong, in the order it matters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

/// The unreleased changes, as a person planning a release reads them.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReleasePlanReport {
    /// The version this tree would publish.
    pub target: Version,
    /// The bump that gets there from the published release, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bump: Option<Bump>,
    /// What the release would cost a caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<CompatibilityImpact>,
    /// The change records that would be stamped with the target version.
    pub changes: Vec<Change>,
    /// The files `majordomus release prepare` would rewrite itself, repository-relative:
    /// the canonical version and the record of every unreleased change. This is the whole
    /// of what preparing a release edits.
    pub writes: Vec<String>,
    /// The projections `majordomus generate` would then rewrite, repository-relative. They
    /// are listed so that a reviewer knows what the release commit will contain; nothing
    /// here is edited by hand, and `prepare` does not write them.
    pub regenerates: Vec<String>,
    /// Everything that would stop the release, from the same checks `release check` makes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

fn versions(ctx: &Context, _: Empty) -> Result<VersionReport, CapabilityError> {
    Ok(engine(ctx)?.versions(&ctx.index))
}

fn status(ctx: &Context, _: Empty) -> Result<ReleaseState, CapabilityError> {
    Ok(engine(ctx)?.state(&ctx.index))
}

fn explain(ctx: &Context, _: Empty) -> Result<ReleaseExplanation, CapabilityError> {
    let e = engine(ctx)?;
    let impact = e.impact();
    let required_bump = e.required_bump();
    let minimum = e.minimum_version();
    let target = e.target_version();
    let deciding: Vec<ContractChange> = match (&e.diff, impact) {
        (Some(diff), Some(impact)) if impact != CompatibilityImpact::None => {
            diff.with_impact(impact).cloned().collect()
        }
        _ => Vec::new(),
    };
    let policy = match (impact, e.baseline.version()) {
        (Some(impact), Some(baseline)) if baseline.is_initial_development() => format!(
            "before 1.0.0 a breaking change moves the minor and everything else moves the \
             patch, so a {impact} change requires a {}",
            required_bump.map(|b| b.as_str()).unwrap_or("none")
        ),
        (Some(impact), Some(_)) => format!(
            "at and above 1.0.0 the mapping is the specification's, so a {impact} change \
             requires a {}",
            required_bump.map(|b| b.as_str()).unwrap_or("none")
        ),
        _ => "no baseline release was compared against, so no bump is required of this tree".into(),
    };
    let summary = match (impact, &minimum) {
        (Some(impact), Some(minimum)) if e.source < *minimum => format!(
            "the contract change is {impact}: this tree states {} and must state at least {minimum}",
            e.source
        ),
        (Some(impact), Some(minimum)) => format!(
            "the contract change is {impact}: {minimum} is the minimum and this tree states {}",
            e.source
        ),
        _ => format!("{}, so no minimum was computed", e.baseline.summary()),
    };
    Ok(ReleaseExplanation {
        current: e.source.clone(),
        baseline: e.baseline.clone(),
        impact,
        required_bump,
        minimum,
        target,
        policy,
        deciding,
        uncompared: e
            .diff
            .as_ref()
            .map(|d| d.uncompared.iter().map(|s| s.noun().to_string()).collect())
            .unwrap_or_default(),
        summary,
    })
}

fn contract_diff(
    ctx: &Context,
    input: ContractDiffInput,
) -> Result<ContractDiffReport, CapabilityError> {
    let e = engine(ctx)?;
    let Some(mut diff) = e.diff.clone() else {
        return Ok(ContractDiffReport {
            reason: Some(match &e.baseline {
                Baseline::Initial => {
                    "no stable release is recorded, so there is nothing to compare against".into()
                }
                other => other.summary(),
            }),
            baseline: e.baseline,
            diff: None,
        });
    };
    if let Some(impact) = input.impact {
        diff.changes.retain(|c| c.impact == impact);
    }
    if let Some(surface) = &input.surface {
        let wanted = surface.trim();
        if !diff.changes.iter().any(|c| c.surface.as_str() == wanted)
            && !["capability", "command", "document-kind", "target"].contains(&wanted)
        {
            return Err(CapabilityError::InvalidInput(format!(
                "`{wanted}` is not a contract surface: capability, command, document-kind, target"
            )));
        }
        diff.changes.retain(|c| c.surface.as_str() == wanted);
    }
    Ok(ContractDiffReport {
        baseline: e.baseline,
        diff: Some(diff),
        reason: None,
    })
}

fn changelog(ctx: &Context, input: ChangelogInput) -> Result<ChangelogReport, CapabilityError> {
    let e = engine(ctx)?;
    let log = e.changelog();
    let markdown = log.to_markdown();
    match input.version {
        Some(version) => {
            let section = log.release(&version).cloned().ok_or_else(|| {
                CapabilityError::NotFound(format!("no release records a version {version}"))
            })?;
            Ok(ChangelogReport {
                records: e.changes.changes.len(),
                unreleased: None,
                releases: vec![section],
                markdown: log.notes(&version).unwrap_or_default(),
            })
        }
        None => Ok(ChangelogReport {
            records: e.changes.changes.len(),
            unreleased: log.unreleased.clone(),
            releases: log.releases.clone(),
            markdown,
        }),
    }
}

fn manifest(
    ctx: &Context,
    input: ReleaseManifestInput,
) -> Result<ReleaseManifest, CapabilityError> {
    let e = engine(ctx)?;
    let version = input.version.unwrap_or_else(|| e.target_version());
    Ok(ReleaseManifest::build(&e, &version, &e.changelog()))
}

fn check(ctx: &Context, _: Empty) -> Result<ReleaseCheckReport, CapabilityError> {
    let e = engine(ctx)?;
    let state = e.state(&ctx.index);
    let blocking = state.diagnostics.iter().filter(|d| d.is_blocking()).count();
    Ok(ReleaseCheckReport {
        ok: blocking == 0,
        summary: match blocking {
            0 => format!(
                "{} is ready to publish: {}",
                state.target_version,
                state.readiness.as_str()
            ),
            1 => format!("one thing stands in the way of {}", state.target_version),
            n => format!("{n} things stand in the way of {}", state.target_version),
        },
        target: state.target_version,
        readiness: state.readiness.as_str().into(),
        diagnostics: state.diagnostics,
    })
}

fn plan(ctx: &Context, input: ReleasePlanInput) -> Result<ReleasePlanReport, CapabilityError> {
    let e = engine(ctx)?;
    let target = match input.version {
        Some(asked) => {
            if let (Some(minimum), Some((impact, _))) = (e.minimum_version(), e.effective_impact())
            {
                if asked < minimum {
                    return Err(CapabilityError::Refused(format!(
                        "the change is {impact} and requires at least {minimum}; {asked} was                          asked for"
                    )));
                }
            }
            asked
        }
        None => e.target_version(),
    };
    let state = e.state(&ctx.index);
    // What `release prepare` itself writes: the canonical version, and the stamp on every
    // unreleased record. Everything else a release moves — the changelog, the manifests,
    // the contract snapshot, the version the shell tool prints, the site's data — is a
    // projection, and `majordomus generate` is the one thing that writes a projection.
    let mut writes = vec!["apps/majordomus-cli/Cargo.toml".to_string()];
    for change in e.changes.unreleased() {
        writes.push(change.path.clone());
    }
    let mut regenerates = vec![
        crate::release::SNAPSHOT_PATH.to_string(),
        crate::release::CHANGELOG_PATH.to_string(),
        crate::release::manifest_path(&target.tag()),
        crate::generate::VERSION_PATH.to_string(),
    ];
    regenerates.sort();
    Ok(ReleasePlanReport {
        bump: e.required_bump(),
        impact: e.effective_impact().map(|(i, _)| i),
        changes: e.changes.unreleased().cloned().collect(),
        writes,
        regenerates,
        diagnostics: state.diagnostics,
        target,
    })
}

/// The module descriptor.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "release",
        title: "Release",
        description: "What this repository would publish next and why it must carry the version it carries: the four versions and where they disagree, the public contract measured against the last published release, the compatibility that change implies, the minimum version the policy makes of it, the changelog every projection renders, and every reason a release is not ready. One engine; the command line, the API, MCP and the Cockpit are four readings of it.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "release.version",
                title: "The four versions",
                description: "What this tree would release, what an unpinned installation resolves to, what this process is, and what each declared deployment reports — with every disagreement named. They are four different facts and this is the one place that refuses to collapse them: a page that shows one number while the machine serving it runs another is the failure this answer exists to make visible. No network is reached; a deployment that has reported nothing says so.",
                input: Empty,
                output: VersionReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_version"),
                    http: get("/api/v1/release/version"),
                    cli: Some(CliExposure { path: vec!["release".into(), "version".into()] }),
                },
                tags: ["release", "version", "introspection"],
                handler: versions,
            },
            capability! {
                id: "release.status",
                title: "Release state",
                description: "The whole release state: the four versions, the release the contract was measured against, the compatibility of the change, the minimum version it requires, the version this tree would publish, how far along the release is, and every diagnostic. This is the model the Cockpit's version display, the API and the release commands all read; none of them computes a release fact of its own.",
                input: Empty,
                output: ReleaseState,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(crate::capability::model::McpExposure {
                        tool: Some("majordomus_release_status".into()),
                        resource: Some(crate::capability::model::McpResource {
                            uri: RELEASE_URI.into(),
                            name: "release".into(),
                        }),
                    }),
                    http: get("/api/v1/release/status"),
                    cli: Some(CliExposure { path: vec!["release".into(), "status".into()] }),
                },
                tags: ["release", "version", "introspection"],
                handler: status,
            },
            capability! {
                id: "release.explain",
                title: "Why this version",
                description: "The reasoning behind the required bump, step by step: the baseline it was measured against, the contract changes that decided it, the impact they add up to, the policy sentence that maps that impact to a bump, and the version that comes out. Derived from the contract diff and the policy table; no sentence here is written by hand for a particular case.",
                input: Empty,
                output: ReleaseExplanation,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_release_explain"),
                    http: get("/api/v1/release/explain"),
                    cli: Some(CliExposure { path: vec!["release".into(), "explain".into()] }),
                },
                tags: ["release", "version", "compatibility"],
                handler: explain,
            },
            capability! {
                id: "release.diff",
                title: "The public contract diff",
                description: "Every difference between the public contract at the last published release and the contract this tree states, with what each one costs a caller and why. Optionally narrowed to one impact or one surface. The contract is the capabilities with their input and output schemas and every projection they declare, the runnable commands with their arguments, the document schemas a repository's own files are validated against, and the platforms a release publishes for.",
                input: ContractDiffInput,
                output: ContractDiffReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_release_diff"),
                    http: get("/api/v1/release/diff"),
                    cli: Some(CliExposure { path: vec!["release".into(), "diff".into()] }),
                },
                tags: ["release", "compatibility", "contract"],
                handler: contract_diff,
            },
            capability! {
                id: "release.changelog",
                title: "The changelog",
                description: "The changelog, as a model and as the Markdown CHANGELOG.md carries: the unreleased changes and every published version's, grouped by kind in the reader's order, with breaking changes marked and their migration documents linked. Every entry is one record under .ai/repo/changes/; this file, the release notes, the Cockpit, the site and this answer are five renderings of those records and of nothing else.",
                input: ChangelogInput,
                output: ChangelogReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_changelog"),
                    http: get("/api/v1/release/changelog"),
                    cli: Some(CliExposure { path: vec!["release".into(), "changelog".into()] }),
                },
                tags: ["release", "changelog"],
                handler: changelog,
            },
            capability! {
                id: "release.manifest",
                title: "A release manifest",
                description: "One release explained: the version it carried, the release it succeeded, the commit, the compatibility, the contract fingerprints on both sides, the changes it published, the migration documents it requires and the artifacts it uploaded with their digests. Generated from the release state; enough to explain a release to somebody who was not there and to prove that the tag, the version, the artifacts and the runtime describe one release.",
                input: ReleaseManifestInput,
                output: ReleaseManifest,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_release_manifest"),
                    http: get("/api/v1/release/manifest"),
                    cli: Some(CliExposure { path: vec!["release".into(), "manifest".into()] }),
                },
                tags: ["release", "provenance"],
                handler: manifest,
            },
            capability! {
                id: "release.plan",
                title: "What a release would do",
                description: "What `majordomus release prepare` would write and what it would publish: the target version, the bump that gets there, the change records it would stamp, every file it would rewrite, and everything that would stop it. The planning and the doing read the same engine, so a plan is a description of the act and not a simulation of it.",
                input: ReleasePlanInput,
                output: ReleasePlanReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_release_plan"),
                    http: get("/api/v1/release/plan"),
                    cli: Some(CliExposure { path: vec!["release".into(), "plan".into()] }),
                },
                tags: ["release", "planning"],
                handler: plan,
            },
            capability! {
                id: "release.check",
                title: "Whether a release may go out",
                description: "Every release invariant this repository can decide locally: that the version clears the minimum its contract change requires, that every breaking change is named by a change record, that a breaking change carries migration guidance, that the change records agree with each other, that the release records agree with the distribution model, and that the committed contract is the contract this build states. Each failure names the command that shows it and the one that fixes it.",
                input: Empty,
                output: ReleaseCheckReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_release_check"),
                    http: get("/api/v1/release/check"),
                    cli: Some(CliExposure { path: vec!["release".into(), "check".into()] }),
                },
                tags: ["release", "diagnostics"],
                handler: check,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist, and every projection — the MCP
    /// tool, the HTTP route, the OpenAPI operation, the command, the benchmark target — is
    /// derived from it. A refactor that dropped an exposure or renamed a route would still
    /// compile, and the suites that exercise the behaviour behind it would still pass. This
    /// is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "release");
        let expected: &[(&str, &str, &str, &[&str])] = &[
            (
                "release.version",
                "majordomus_version",
                "/api/v1/release/version",
                &["release", "version"],
            ),
            (
                "release.status",
                "majordomus_release_status",
                "/api/v1/release/status",
                &["release", "status"],
            ),
            (
                "release.explain",
                "majordomus_release_explain",
                "/api/v1/release/explain",
                &["release", "explain"],
            ),
            (
                "release.diff",
                "majordomus_release_diff",
                "/api/v1/release/diff",
                &["release", "diff"],
            ),
            (
                "release.changelog",
                "majordomus_changelog",
                "/api/v1/release/changelog",
                &["release", "changelog"],
            ),
            (
                "release.manifest",
                "majordomus_release_manifest",
                "/api/v1/release/manifest",
                &["release", "manifest"],
            ),
            (
                "release.plan",
                "majordomus_release_plan",
                "/api/v1/release/plan",
                &["release", "plan"],
            ),
            (
                "release.check",
                "majordomus_release_check",
                "/api/v1/release/check",
                &["release", "check"],
            ),
        ];
        assert_eq!(m.capabilities.len(), expected.len());
        for (executable, (id, tool, path, command)) in m.capabilities.iter().zip(expected) {
            let c = &executable.capability;
            assert_eq!(c.id.as_str(), *id);
            assert_eq!(
                c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id}"
            );
            assert_eq!(
                c.exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id}"
            );
            assert_eq!(
                c.exposure.cli.as_ref().map(|c| c.path.as_slice()),
                Some(
                    command
                        .iter()
                        .map(|w| (*w).to_string())
                        .collect::<Vec<_>>()
                        .as_slice()
                ),
                "{id}"
            );
        }
    }

    /// Every capability of this module reads and none of them writes. A release is
    /// prepared by a command of the executable; a capability that mutated the repository
    /// would put a release behind an unauthenticated loopback socket.
    #[test]
    fn nothing_here_writes() {
        for executable in module().capabilities {
            assert!(
                executable.capability.kind.is_read_only(),
                "{} is not read-only",
                executable.capability.id.as_str()
            );
        }
    }

    #[test]
    fn the_release_resource_is_declared_once_and_on_the_state() {
        let m = module();
        let resources: Vec<&str> = m
            .capabilities
            .iter()
            .filter_map(|e| {
                e.capability
                    .exposure
                    .mcp
                    .as_ref()
                    .and_then(|m| m.resource.as_ref())
                    .map(|r| r.uri.as_str())
            })
            .collect();
        assert_eq!(resources, [RELEASE_URI]);
    }
}
