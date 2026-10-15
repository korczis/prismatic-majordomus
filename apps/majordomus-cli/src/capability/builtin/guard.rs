//! The `guard` module: whether a write to a file of this checkout is covered by a claim
//! (I2735). The judgement is [`crate::guard`]'s, shared with `majordomus guard hook`, which a
//! provider's pre-tool hook runs without building the index; this projects the same answer
//! to MCP and HTTP, reading the mesh's claims from this process when it is the server and
//! from the checkout's server otherwise.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::guard::{judge, server_claims, ClaimSource, EditVerdict};
use crate::{capability, module};

use super::{get, mcp};

/// Which write to judge.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardEditInput {
    /// The path about to be written: absolute, or relative to the repository root.
    pub path: String,
    /// The writing session's id on the mesh (`claude-code-<session id>` for Claude Code);
    /// without it only the active task's scope can cover the write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

impl BenchmarkCases for GuardEditInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "in-repository",
                GuardEditInput {
                    path: "docs/MESH.md".into(),
                    session: Some("claude-code-bench".into()),
                },
            ),
            NamedCase::new(
                "outside",
                GuardEditInput {
                    path: "/nonexistent/outside.txt".into(),
                    session: None,
                },
            ),
        ]
    }
}

fn guard_edit(ctx: &Context, input: GuardEditInput) -> Result<EditVerdict, CapabilityError> {
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    let claims = || match ctx.mesh.cooperation() {
        Some(c) => ClaimSource::Claims(c.state().claims),
        None if ctx.mesh.decided() => ClaimSource::Unavailable(
            ctx.mesh
                .cooperation_status()
                .reason
                .unwrap_or_else(|| "the mesh does not run in this server".into()),
        ),
        None => server_claims(&root),
    };
    Ok(judge(&root, &input.path, input.session.as_deref(), claims))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "guard",
        title: "Edit guard",
        description: "Whether a write to a file of this checkout is covered by a claim: the active task's scope, or a live mesh claim of the writing session. The judgement a provider's pre-tool hook asks before every edit (majordomus guard hook), switched by session.edits_require_claim.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "guard.edit",
                title: "Judge one edit",
                description: "Whether the given session may write the given path now: allowed when the policy's session.edits_require_claim is off, the path is outside the checkout or its local state, the repository declares no enabled mesh, the active task's scope covers it, or a live mesh claim of the session covers it; refused as unclaimed otherwise, with the remedy. What cannot be read — the policy, the mesh — is let through and named, never refused.",
                input: GuardEditInput,
                output: EditVerdict,
                stability: Stability::Experimental,
                exposure: Exposure { mcp: mcp("majordomus_guard_edit"), http: get("/api/v1/guard/edit"), cli: None },
                tags: ["guard", "mesh", "coordination"],
                handler: guard_edit,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "guard");
        let exposure = &m.capabilities[0].capability.exposure;
        assert_eq!(m.capabilities[0].capability.id.as_str(), "guard.edit");
        assert_eq!(
            exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_guard_edit")
        );
        assert_eq!(
            exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/guard/edit")
        );
    }
}
