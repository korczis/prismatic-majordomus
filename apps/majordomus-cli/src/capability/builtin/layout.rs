//! The `layout` module: the boundary between the layer's two halves, checked against the
//! layouts their contracts declare (I2871, I1102).
//!
//! One capability, `layout.check`, and it is a gate: a path a contract excludes is a
//! finding that names the path and the contract. The judgement lives in [`crate::layout`];
//! this module declares it once and every projection — the command line, MCP, HTTP, the
//! benchmark — is derived from the declaration. `directories` answers what a contract says;
//! this answers whether the tree keeps it.
//!
//! ```
//! use majordomus_cli::capability::builtin::modules;
//! let m = modules().into_iter().find(|m| m.id.as_str() == "layout").unwrap();
//! let c = &m.capabilities[0].capability;
//! assert_eq!(c.id.as_str(), "layout.check");
//! assert_eq!(c.exposure.cli.as_ref().unwrap().path, ["layout", "check"]);
//! ```

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::layout::{self, LayoutReport};
use crate::{capability, module};

use super::{get, mcp, Empty};

fn check(ctx: &Context, _: Empty) -> Result<LayoutReport, CapabilityError> {
    let repository = ctx.repository().map_err(|e| {
        CapabilityError::Internal(format!("the repository could not be re-read: {e}"))
    })?;
    let share = crate::share::Share::locate(None, repository.root())
        .map_err(|e| CapabilityError::Internal(format!("the distribution was not found: {e}")))?;
    let kinds = crate::metadata::KindSchema::load(&share, &repository)
        .map_err(|e| CapabilityError::Internal(format!("the kinds could not be read: {e}")))?;
    Ok(layout::check(&repository, &ctx.index, &kinds))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "layout",
        title: "Layer layout",
        description: "The boundary between the layer's tracked and local halves, checked against the layouts their contracts declare: what lives where, and what must never appear there (I1102).",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "layout.check",
                title: "Refuse a misplaced artifact",
                description: "Every path a declared layout excludes, named with the contract it violates: a tracked file under the tracked half that git's ignore rules match (state that does not travel, under a contract that excludes cache or runtime), and a file under the local half that is a byte copy of a tracked canonical file or declares the schema of a kind share/kinds.yaml calls canonical (under the manifest's `local.excludes: [canonical]`). Only what a contract declares is judged. Reads git and the local half's files; no build and no network.",
                input: Empty,
                output: LayoutReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_layout_check"),
                    http: get("/api/v1/layout/check"),
                    cli: Some(CliExposure { path: vec!["layout".into(), "check".into()] }),
                },
                tags: ["layout", "governance", "two-folder"],
                handler: check,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; a refactor that dropped an
    /// exposure would still compile, and this is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "layout");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["layout.check"]);
        let exposure = &m.capabilities[0].capability.exposure;
        assert_eq!(
            exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_layout_check")
        );
        assert_eq!(exposure.http.as_ref().unwrap().path, "/api/v1/layout/check");
        assert_eq!(exposure.cli.as_ref().unwrap().path, ["layout", "check"]);
    }
}
