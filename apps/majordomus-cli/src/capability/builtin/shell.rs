//! The `shell` module: the repository's shell automation against the inventory that tracks
//! its migration.
//!
//! One capability, `shell.check`, and it is a gate: every shell unit under the governed
//! directories must be declared in `.ai/repo/automation/inventory.jsonl` with an exemption,
//! and every record there must name something the tree still has. The judgement lives in
//! [`crate::automation`]; this module declares it once and every projection — the command
//! line, MCP, HTTP, the benchmark — is derived from the declaration.
//!
//! ```
//! use majordomus_cli::capability::builtin::modules;
//! let m = modules().into_iter().find(|m| m.id.as_str() == "shell").unwrap();
//! let c = &m.capabilities[0].capability;
//! assert_eq!(c.id.as_str(), "shell.check");
//! assert_eq!(c.exposure.cli.as_ref().unwrap().path, ["shell", "check"]);
//! ```

use crate::automation::{self, ShellReport};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::{get, mcp, Empty};

fn check(ctx: &Context, _: Empty) -> Result<ShellReport, CapabilityError> {
    Ok(automation::check(std::path::Path::new(
        &ctx.index.repository.root,
    )))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "shell",
        title: "Shell automation",
        description: "The repository's shell automation measured against the tracked migration inventory: new shell is refused unless an exemption declares why it has to be shell and when it stops being, and an exemption whose file is gone is refused too, so the list only shrinks as units move into typed and scripted capabilities (ADR 0069).",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "shell.check",
                title: "Refuse undeclared shell",
                description: "Every shell unit under bin/, lib/, scripts/, share/, .githooks/ and .claude/hooks/ — by interpreter line or by a .sh name, as git would commit it, links not followed — against .ai/repo/automation/inventory.jsonl: an undeclared unit, a record whose unit is gone, an exemption missing its reason or its removal condition, a disposition outside A-F, a duplicate and a record out of canonical order are each a finding that names the file and the remedy. Reads the tree and git's index only; no build and no network.",
                input: Empty,
                output: ShellReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_shell_check"),
                    http: get("/api/v1/shell/check"),
                    cli: Some(CliExposure { path: vec!["shell".into(), "check".into()] }),
                },
                tags: ["shell", "governance", "migration"],
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
        assert_eq!(m.id.as_str(), "shell");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["shell.check"]);
        let exposure = &m.capabilities[0].capability.exposure;
        assert_eq!(
            exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_shell_check")
        );
        assert_eq!(exposure.http.as_ref().unwrap().path, "/api/v1/shell/check");
        assert_eq!(exposure.cli.as_ref().unwrap().path, ["shell", "check"]);
    }
}
