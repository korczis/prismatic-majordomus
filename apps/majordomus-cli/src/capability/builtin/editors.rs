//! The `editors` module: what Zed and Google Antigravity read from this repository, and how
//! each file they read was derived.
//!
//! The answer is `crate::editors::report` and nothing else: the same plan
//! `majordomus generate editors` writes, with every adapter's capabilities and the evidence
//! for them, every projected file's state in this checkout and content address, and every
//! finding. Read-only — the files are written by `majordomus generate editors` and checked by
//! `majordomus generate --check` (ADR 0140).

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::editors::EditorProjection;
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which `editors.status` is read as an MCP resource.
pub const EDITORS_URI: &str = "majordomus://editors";

fn editors_status(ctx: &Context, _: Empty) -> Result<EditorProjection, CapabilityError> {
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    Ok(crate::editors::report(&ctx.index, &root, crate::VERSION))
}

/// The `editors` module: the editor and agent projections, explained.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "editors",
        title: "Editor and agent projections",
        description: "The files Zed and Google Antigravity read from this repository, derived from the skills and the tracked justfile: what each product supports and on what evidence, the state and content address of every projected file, and what to do about each finding.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "editors.status",
                title: "Editor and agent projections",
                description: "Every adapter (Zed, Antigravity) with each capability it supports, does not, or cannot be verified for, and the evidence; every projected file with the inputs it is derived from, its content address and its state here (current, stale, absent, or a conflict with a file this generator did not write); orphaned projections; and every finding with its remedy. Writes nothing: `majordomus generate editors` does.",
                input: Empty,
                output: EditorProjection,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_editors".into()),
                        resource: Some(McpResource { uri: EDITORS_URI.into(), name: "editors".into() }),
                    }),
                    http: get("/api/v1/editors"),
                    cli: None,
                },
                tags: ["editors", "projections", "introspection"],
                handler: editors_status,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the one place the tool, the route and the resource are named; a
    /// refactor that dropped one would still compile and every behavioural suite would
    /// still pass. This would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "editors");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["editors.status"]);
        let exposure = &m.capabilities[0].capability.exposure;
        let mcp = exposure.mcp.as_ref().expect("an MCP exposure");
        assert_eq!(mcp.tool.as_deref(), Some("majordomus_editors"));
        assert_eq!(
            mcp.resource.as_ref().map(|r| r.uri.as_str()),
            Some(EDITORS_URI)
        );
        assert_eq!(
            exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/editors")
        );
        assert!(
            exposure.cli.is_none(),
            "the files are written by `generate editors`"
        );
    }

    /// The capability answers with the projection's own report, over the repository the
    /// context was built from.
    #[test]
    fn the_status_is_the_projections_report_for_this_repository() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let answer = editors_status(&ctx, Empty {}).unwrap();
        assert_eq!(answer.schema, crate::editors::SCHEMA);
        let ids: Vec<&str> = answer.adapters.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["antigravity", "zed"]);
        assert!(
            answer.artifacts.is_empty(),
            "no skill, no justfile, nothing projected"
        );
    }
}
