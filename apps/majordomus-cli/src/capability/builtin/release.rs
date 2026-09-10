//! The `release` module: the changelog and the version, both derived.
//!
//! Two capabilities, both read-only, both answered by [`crate::release`] — the same code the
//! command line renders, the generated document is written from, and the site page shows.
//! Neither reads a file somebody maintains: the changelog composes the layer's own release
//! records, decisions and the repository's commits, and the version report reads the two
//! places the version is stated and says whether they agree.
//!
//! Raising the version is not here. It writes tracked files, which makes it a repository
//! mutation, and the exposure policy keeps repository mutations off every machine surface —
//! so `release bump` is a command and nothing more, and this module is the read half.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::registry::CapabilityRegistry;
use crate::release::{self, model::ProducedBy, model::VersionReport, Changelog};
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which the changelog is read as an MCP resource.
pub const CHANGELOG_URI: &str = "majordomus://changelog";

/// Which part of the changelog to answer with.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ReleaseChangelogInput")]
pub struct ChangelogInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One version, or `unreleased`; every section when absent.
    pub version: Option<String>,
}

impl BenchmarkCases for ChangelogInput {
    /// One real version from the layer's own records, so the narrow case is timed against
    /// something that exists rather than against a filter that matches nothing. A
    /// repository with no records has a narrow case all the same — the coverage gate counts
    /// targets, not repositories, and the OpenAPI operation's example of `version` *is* this
    /// case, which case 92 refuses to see missing — so it falls back to `unreleased`, the
    /// one section a repository has whether it has released anything or not.
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let version = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == crate::release::changelog::RELEASE_KIND)
            .and_then(|o| o.metadata.get("version"))
            .and_then(|v| v.as_str())
            .map_or_else(|| "unreleased".to_string(), str::to_string);
        vec![
            NamedCase::new("all", ChangelogInput::default()),
            NamedCase::new(
                "one-version",
                ChangelogInput {
                    version: Some(version),
                },
            ),
        ]
    }
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "release",
        title: "Release",
        description: "What this project has shipped and what it would ship next, derived rather than maintained: the changelog composes the layer's release records, the decisions dated inside each release's window and the conventional commits in its range; the version report reads the two places the version is stated and says what the commits since the last release imply it should become.",
        stability: Stability::Implemented,
        capabilities: [
            capability! {
                id: CHANGELOG_ID,
                title: "The changelog",
                description: "Every release the layer records, newest first, with the work that has not been released leading. A section's decisions are the ADRs dated inside that release's window, its changes the conventional commits in its range, its artifacts the record's own evidence. Nothing in it is authored, and a section that could not be read says so rather than appearing empty.",
                input: ChangelogInput,
                output: Changelog,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_changelog".into()),
                        resource: Some(McpResource { uri: CHANGELOG_URI.into(), name: "changelog".into() }),
                    }),
                    http: get("/api/v1/changelog"),
                    // Not a CLI projection: `majordomus release changelog` is a local
                    // command that *renders* this capability for a person at a terminal,
                    // and cli::LOCAL says so once (`RendersCapability`). Declaring the
                    // path here as well is the double accounting `tests/quality.rs`
                    // refuses (OPERATION_CLASSIFICATION_CONFLICT).
                    cli: None,
                },
                tags: ["release", "changelog"],
                handler: changelog,
            },
            capability! {
                id: "release.version",
                title: "The version, and the one the commits imply",
                description: "The version the crate manifest declares, the version the shell tool prints, and whether they agree — the same question `scripts/release-version --check` gates on. Then the bump the conventional commits since the last release imply, the version it would produce, and the commits themselves as the evidence for it.",
                input: Empty,
                output: VersionReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_release_version".into()),
                        resource: None,
                    }),
                    http: get("/api/v1/release/version"),
                    cli: None,
                },
                tags: ["release", "version"],
                handler: version,
            },
        ],
    }
}

/// The capability's own id. Beside its declaration, so the two cannot drift apart without
/// the test below noticing; the document carries it so that no page has to enumerate where
/// this value is served.
pub const CHANGELOG_ID: &str = "release.changelog";

/// Where else the changelog can be had, read off the declarations that already say so.
///
/// One answer for every writer of the document: the handler that serves it and the generator
/// that commits it both ask here, so the committed artifact and the live answer name the same
/// surfaces. The route, the tool and the resource are the capability's own declaration; the
/// command line is the local command that renders this capability, which `cli::LOCAL`
/// declares once beside the reason it is local. Nothing is spelled out a second time, and a
/// page that lists these resolves each against the datasets the registry generates rather
/// than typing a route of its own. `None` only when the registry does not carry the
/// capability at all, which the module's own test refuses.
///
/// ```
/// use majordomus_cli::capability::builtin::release::{module, produced_by};
/// use majordomus_cli::capability::registry::CapabilityRegistry;
///
/// let registry = CapabilityRegistry::builder()
///     .with_modules(vec![module()])
///     .build()
///     .expect("the release module composes on its own");
/// let by = produced_by(&registry).expect("the registry carries release.changelog");
/// assert_eq!(by.capability, "release.changelog");
/// assert_eq!(by.cli.as_deref(), Some("majordomus release changelog"));
/// assert_eq!(by.http.as_deref(), Some("/api/v1/changelog"));
/// assert_eq!(by.mcp_tool.as_deref(), Some("majordomus_changelog"));
/// assert_eq!(by.mcp_resource.as_deref(), Some("majordomus://changelog"));
/// ```
pub fn produced_by(registry: &CapabilityRegistry) -> Option<ProducedBy> {
    let c = registry.get(CHANGELOG_ID)?;
    let cli = crate::cli::local::LOCAL
        .iter()
        .find(|l| l.reason.renders() == Some(CHANGELOG_ID))
        .map(|l| format!("majordomus {}", l.command));
    Some(ProducedBy {
        capability: c.id.to_string(),
        cli,
        http: c.exposure.http.as_ref().map(|h| h.path.clone()),
        mcp_tool: c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
        mcp_resource: c
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref().map(|r| r.uri.clone())),
    })
}

fn changelog(ctx: &Context, input: ChangelogInput) -> Result<Changelog, CapabilityError> {
    let root = std::path::Path::new(&ctx.index.repository.root);
    let mut log = release::compose(root, &ctx.index.objects);
    log.produced_by = produced_by(&ctx.registry);
    if let Some(wanted) = input.version.as_deref() {
        log.sections.retain(|s| {
            s.version == wanted
                || s.tag.as_deref() == Some(wanted)
                || (wanted == "unreleased" && s.unreleased)
        });
        if log.sections.is_empty() {
            return Err(CapabilityError::NotFound(format!(
                "no release section carries the version '{wanted}'"
            )));
        }
    }
    Ok(log)
}

fn version(ctx: &Context, _: Empty) -> Result<VersionReport, CapabilityError> {
    let root = std::path::Path::new(&ctx.index.repository.root);
    Ok(release::version::report(root, &ctx.index.objects))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two capabilities, and every surface that carries them is derived from this
    /// declaration: the MCP tools, the changelog resource, the HTTP routes, the OpenAPI
    /// operations. A refactor that renamed a route, dropped the resource or added a third
    /// would still compile, and the suites that exercise the changelog behind it would
    /// still pass. This is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "release");
        let expected: &[(&str, &str, &str)] = &[
            (
                "release.changelog",
                "majordomus_changelog",
                "/api/v1/changelog",
            ),
            (
                "release.version",
                "majordomus_release_version",
                "/api/v1/release/version",
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
        // the changelog is also a resource, so a client may read it without calling a tool
        let changelog = &m.capabilities[0].capability.exposure;
        assert_eq!(
            changelog
                .mcp
                .as_ref()
                .and_then(|m| m.resource.as_ref())
                .map(|r| r.uri.as_str()),
            Some(CHANGELOG_URI),
            "the changelog stopped being an MCP resource"
        );
    }
}
