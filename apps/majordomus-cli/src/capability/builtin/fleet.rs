//! The `fleet` module: the machines that run this repository's mesh, and the operation
//! that brings every one of them to this executable's release (ADR 0121).
//!
//! The domain is [`crate::fleet`]; every handler here reads the declaration and the
//! distribution model off the index and calls into it. `fleet.plan` reaches nothing.
//! `fleet.status` and `fleet.rollout` reach other machines over ssh, so they are projected
//! on the command line only, and the rollout is classified as changing other machines.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CapabilityKind, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::fleet::{self, Declaration, Release};
use crate::{capability, module};

fn cli(words: &[&str]) -> Option<CliExposure> {
    Some(CliExposure {
        path: words.iter().map(|w| w.to_string()).collect(),
    })
}

/// The declaration and its address, or why there is none to read.
fn declaration(ctx: &Context) -> Result<(Declaration, String), CapabilityError> {
    let object = ctx
        .index
        .objects
        .iter()
        .find(|o| o.kind == fleet::KIND)
        .ok_or_else(|| {
            CapabilityError::NotFound(
                "this repository declares no fleet; a fleet is one object under .ai/repo/fleet/ (docs/FLEET.md)".into(),
            )
        })?;
    let d = Declaration::parse(object).map_err(CapabilityError::Refused)?;
    Ok((d, object.uri.clone()))
}

/// The release a rollout installs: the version asked for, or this executable's own.
fn release(
    ctx: &Context,
    version: Option<&str>,
    keep_servers: bool,
) -> Result<Release, CapabilityError> {
    let model = ctx.index.distribution.as_ref().ok_or_else(|| {
        CapabilityError::NotFound(
            "this installation carries no share/distribution.yaml, so it names no installer".into(),
        )
    })?;
    let pinned = version.map(|v| v.trim_start_matches('v').to_string());
    if let Some(v) = &pinned {
        if v.is_empty()
            || !v
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c))
        {
            return Err(CapabilityError::InvalidInput(format!(
                "{v} is not a version"
            )));
        }
    }
    Ok(Release {
        version: pinned.clone().unwrap_or_else(|| crate::VERSION.to_string()),
        pinned: pinned.is_some(),
        installer: model.installer_url(),
        install_dir: model.installer.install_dir.clone(),
        prefix: model.installer.prefix.clone(),
        keep_servers,
    })
}

/// The input of `fleet.plan` and `fleet.rollout`.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RolloutInput {
    /// The machines to bring, by their names in the fleet; every machine when absent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub machines: Vec<String>,
    /// The release to install (`0.17.0` or `v0.17.0`); this executable's own when absent.
    /// Named, it is installed even where a newer one is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Leave running the servers a machine runs from an older installed tree. By default
    /// each is stopped and started again at the version on the port it had; a session
    /// attached to one reconnects.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep_servers: bool,
}

impl BenchmarkCases for RolloutInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("default", RolloutInput::default())]
    }
}

/// `fleet.plan`.
pub fn plan(ctx: &Context, input: RolloutInput) -> Result<fleet::Plan, CapabilityError> {
    let (d, uri) = declaration(ctx)?;
    let release = release(ctx, input.version.as_deref(), input.keep_servers)?;
    let mut plan = fleet::plan(&d, &uri, &release);
    if !input.machines.is_empty() {
        for id in &input.machines {
            if !plan.machines.iter().any(|m| &m.id == id) {
                return Err(CapabilityError::InvalidInput(format!(
                    "the fleet declares no machine {id}"
                )));
            }
        }
        plan.machines.retain(|m| input.machines.contains(&m.id));
    }
    Ok(plan)
}

/// `fleet.status`.
pub fn status(ctx: &Context, _: super::Empty) -> Result<fleet::Status, CapabilityError> {
    let (d, uri) = declaration(ctx)?;
    let release = release(ctx, None, true)?;
    Ok(fleet::status(&d, &uri, &release))
}

/// `fleet.rollout`.
pub fn rollout(ctx: &Context, input: RolloutInput) -> Result<fleet::Rollout, CapabilityError> {
    let (d, uri) = declaration(ctx)?;
    let release = release(ctx, input.version.as_deref(), input.keep_servers)?;
    fleet::rollout(&d, &uri, &release, &input.machines).map_err(CapabilityError::InvalidInput)
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "fleet",
        title: "Fleet",
        description: "The machines that run this repository's mesh, as .ai/repo/fleet/ declares them, and the one operation that brings every one of them to this executable's release: install it with the published installer, fast-forward each hub's checkout, write and restart each hub's service, and verify that each hub answers at the version and that the hubs see each other (ADR 0121).",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "fleet.plan",
                title: "What a rollout would do on every machine",
                description: "Reads the fleet declaration and the distribution model and reaches nothing: the version every machine would be brought to, the installer, this machine's mesh node, and for each machine whether it is this one, the ssh destinations tried in order, the hub it serves and the steps a rollout takes there.",
                input: RolloutInput,
                output: fleet::Plan,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: cli(&["fleet", "plan"]),
                },
                tags: ["fleet", "mesh", "distribution"],
                cache: CachePolicy::Disabled,
                handler: plan,
            },
            capability! {
                id: "fleet.status",
                title: "What every machine of the fleet runs",
                description: "Asks every machine, in parallel and over ssh unless it is this one, what it is and runs: the destination that answered, its platform, the version its launcher runs, every majordomus serve running there, and for a hub its checkout's branch, commit and cleanliness and the version the hub answers at. A machine no destination reaches is reported with every destination's reason. Changes nothing.",
                input: super::Empty,
                output: fleet::Status,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: cli(&["fleet", "status"]),
                },
                tags: ["fleet", "mesh", "distribution"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: status,
            },
            capability! {
                id: "fleet.rollout",
                kind: CapabilityKind::Command,
                title: "Bring every machine of the fleet to this release, and form the mesh",
                description: "On every machine named (every one by default), in parallel: reach it (this machine directly, the others by the first ssh destination that answers, never prompting); install the release with the published installer, which verifies the archive before touching anything, unless that version, or a newer one when none was named, is already there; for a hub, fast-forward its checkout to its remote's default branch (cloning it when absent, declining a dirty, diverged or off-branch one and leaving it as it is), write its service — a systemd user unit on Linux, a launchd agent on macOS — and restart it when anything changed or it does not answer at the version, then wait for it to answer at the version with its mesh active. Last, asks the converged hubs which nodes they see until every hub is seen by every other. Each machine's steps and verdict are reported; one machine's failure stops only that machine.",
                input: RolloutInput,
                output: fleet::Rollout,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: cli(&["fleet", "rollout"]),
                },
                tags: ["fleet", "mesh", "distribution"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: rollout,
            }
            .changes_other_machines(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rollout_input_names_machines_and_a_version_and_nothing_else() {
        let i: RolloutInput =
            serde_json::from_str(r#"{"machines":["lundra"],"version":"v0.17.0"}"#).unwrap();
        assert_eq!(i.machines, vec!["lundra".to_string()]);
        assert_eq!(i.version.as_deref(), Some("v0.17.0"));
        assert!(serde_json::from_str::<RolloutInput>(r#"{"force":true}"#).is_err());
    }

    #[test]
    fn the_rollout_changes_other_machines_and_is_on_the_command_line_only() {
        let m = module();
        let rollout = m
            .capabilities
            .iter()
            .find(|e| {
                e.capability.id
                    == crate::capability::model::CapabilityId::parse("fleet.rollout").unwrap()
            })
            .expect("declared");
        assert_eq!(
            rollout.capability.execution.effect,
            crate::capability::Effect::RemoteMutation
        );
        for e in &m.capabilities {
            assert!(e.capability.exposure.mcp.is_none(), "{}", e.capability.id);
            assert!(e.capability.exposure.http.is_none(), "{}", e.capability.id);
        }
    }
}
