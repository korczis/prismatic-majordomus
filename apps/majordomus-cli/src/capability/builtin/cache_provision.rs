//! The input, the report and the handler of `mesh.build_cache.provision` (I2674, ADR 0157): a
//! node provisioned as the mesh build cache's S3 provider. The capability itself is declared
//! in the `mesh` module (a capability's id begins with its module's id); it is offered on the
//! command line only, because it installs services and writes credentials, which nothing
//! reachable over HTTP or MCP may do.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::mesh::cache_provision::{apply, observe, plan, ApplyReport, Plan, Policy};

/// The input of `mesh.build_cache.provision`.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProvisionInput {
    /// Carry the plan out. Absent, the capability only plans and changes nothing. Under the
    /// default root `/` it needs root and refuses without it.
    #[serde(default)]
    pub apply: bool,
    /// The directory the host's paths are read from and written under; default `/`. Any other
    /// root is a staging directory: files only, no privilege, no command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

impl BenchmarkCases for ProvisionInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("plan", ProvisionInput::default())]
    }
}

/// The answer: the plan against the host as observed, and the apply's report when asked.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProvisionReport {
    /// `plan` or `apply`.
    pub mode: String,
    /// The root read.
    pub root: String,
    /// The policy in force: declared, not a secret.
    pub policy: Policy,
    /// What the host owed when asked.
    pub plan: Plan,
    /// What the apply did; absent for a plan.
    pub apply: Option<ApplyReport>,
    /// A plan always holds; an apply holds when nothing failed and nothing is owed after it.
    pub ok: bool,
}

pub(crate) fn provision(
    _: &Context,
    input: ProvisionInput,
) -> Result<ProvisionReport, CapabilityError> {
    let policy = Policy::declared();
    let root = PathBuf::from(input.root.as_deref().unwrap_or("/"));
    let owed = plan(&policy, &observe(&policy, &root));
    let applied = input.apply.then(|| apply(&policy, &root));
    Ok(ProvisionReport {
        mode: if input.apply { "apply" } else { "plan" }.into(),
        root: root.display().to_string(),
        ok: applied.as_ref().is_none_or(|a| a.ok),
        policy,
        plan: owed,
        apply: applied,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};

    #[test]
    fn a_plan_over_an_empty_root_changes_nothing_and_an_apply_into_it_converges() {
        let root = std::env::temp_dir().join(format!("mj-provision-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let repo = SyntheticRepository::new(Shape::default()).unwrap();
        let ctx = repo.context().expect("a context");
        let input = |apply| ProvisionInput {
            apply,
            root: Some(root.display().to_string()),
        };
        let planned = provision(&ctx, input(false)).unwrap();
        assert!(planned.ok && planned.mode == "plan" && planned.apply.is_none());
        assert!(planned.plan.changes > 0 && !root.exists());
        let applied = provision(&ctx, input(true)).unwrap();
        assert!(applied.ok && applied.mode == "apply");
        assert_eq!(applied.apply.unwrap().after.unwrap().changes, 0);
        assert_eq!(
            ProvisionInput::benchmark_cases(&CaseContext { index: &ctx.index }).len(),
            1
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
