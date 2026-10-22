//! The `disk` module: what this machine can give back, reclaimed by name (I3833).
//!
//! One capability, `disk.prune`. Its first source is the build directories nobody is
//! using, under `<CARGO_HOME>/build-private` and any directory the caller adds (an
//! `llvm-cov-target`, say), each removed only while this process holds every cargo lock in
//! it (`crate::disk`). A sweep that checked and then removed deleted a directory under a
//! cargo that had just started on it, twice on 2026-10-21; holding the locks across the
//! rename is what makes the removal safe, not a better check. The decision and the
//! destruction live in `crate::disk`; this module declares them once, and the command
//! line, MCP and HTTP are projections of the declaration.
//!
//! The second source, asked for with `containers`, is the container runtime's cache: the
//! build cache and dangling images, then an `fstrim` inside a colima VM so that the host
//! gets the blocks back (`crate::disk::containers`). A runtime that is down is not started.
//!
//! The threshold is required and zero is refused, as for `recover.orphans`: a prune that
//! measured against an absent bound would find every directory idle.
//!
//! ```
//! use majordomus_cli::capability::builtin::modules;
//! use majordomus_cli::capability::Effect;
//! let m = modules().into_iter().find(|m| m.id.as_str() == "disk").unwrap();
//! let c = &m.capabilities[0].capability;
//! assert_eq!(c.id.as_str(), "disk.prune");
//! assert_eq!(c.exposure.cli.as_ref().unwrap().path, ["disk", "prune"]);
//! // it removes directories, so every client asks before running it
//! assert_eq!(c.execution.effect, Effect::RepositoryMutation);
//! ```

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::CapabilityKind;
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::disk::containers::{self, ContainerReport};
use crate::disk::{self, BuildDirReport};
use crate::{capability, module};

use super::{mcp, post};

/// What to prune, and whether only to say.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiskPruneInput {
    /// A build directory is a candidate only when nothing in it changed for this many
    /// minutes. Required; zero is refused.
    pub idle_minutes: u64,
    /// Report what would be removed and remove nothing.
    #[serde(default)]
    pub check: bool,
    /// Further build directories to consider beside those under
    /// `<CARGO_HOME>/build-private`, each as one candidate (an `llvm-cov-target`, say).
    #[serde(default)]
    pub extra: Vec<String>,
    /// The directories whose children are the candidates; absent means
    /// `<CARGO_HOME>/build-private`. An empty list considers only `extra`.
    #[serde(default)]
    pub roots: Option<Vec<String>>,
    /// Also prune the container runtime's cache: the build cache no build holds and the
    /// dangling images, then return the freed blocks of a colima VM to the host. Volumes,
    /// containers and tagged images are never touched, and a runtime that is down is not
    /// started.
    #[serde(default)]
    pub containers: bool,
}

impl BenchmarkCases for DiskPruneInput {
    // A benchmark never walks a developer's real build directories: no roots, a check.
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "check-nothing-named",
            DiskPruneInput {
                idle_minutes: 30,
                check: true,
                extra: Vec::new(),
                roots: Some(Vec::new()),
                containers: false,
            },
        )]
    }
}

/// What was found, what was removed, and the free space before and after.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DiskPruneResult {
    /// The build directories.
    pub build_dirs: BuildDirReport,
    /// The container runtime, when `containers` was asked for.
    pub containers: Option<ContainerReport>,
    /// Free bytes on the volume of `<CARGO_HOME>` before the prune, when measurable.
    pub free_before: Option<u64>,
    /// Free bytes on the same volume after it.
    pub free_after: Option<u64>,
}

/// `<CARGO_HOME>`, as cargo reads it.
fn cargo_home() -> PathBuf {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
        .unwrap_or_else(|| PathBuf::from(".cargo"))
}

fn prune(_ctx: &Context, input: DiskPruneInput) -> Result<DiskPruneResult, CapabilityError> {
    if input.idle_minutes == 0 {
        return Err(CapabilityError::Refused(
            "idle_minutes is zero, which would make every build directory idle the moment it \
             was written; name the bound"
                .into(),
        ));
    }
    let home = cargo_home();
    let free_before = disk::free_bytes(&home);
    let extra: Vec<PathBuf> = input.extra.iter().map(PathBuf::from).collect();
    let roots: Vec<PathBuf> = match &input.roots {
        Some(r) => r.iter().map(PathBuf::from).collect(),
        None => vec![home.join("build-private")],
    };
    let build_dirs = disk::prune_build_dirs(&roots, &extra, input.idle_minutes, input.check);
    let containers = input
        .containers
        .then(|| containers::prune_containers(&containers::Runtime::from_path(), input.check));
    Ok(DiskPruneResult {
        build_dirs,
        containers,
        free_before,
        free_after: disk::free_bytes(&home),
    })
}

/// The `disk` module: one capability, `disk.prune`, declared once so that the command line,
/// MCP and HTTP are projections of the same declaration and cannot disagree about what a
/// prune removes.
///
/// ```
/// use majordomus_cli::capability::builtin::disk::module;
/// let m = module();
/// assert_eq!(m.id.as_str(), "disk");
/// assert_eq!(m.capabilities.len(), 1);
/// assert_eq!(m.capabilities[0].capability.id.as_str(), "disk.prune");
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "disk",
        title: "Disk",
        description: "What this machine can give back, reclaimed by name: the build directories nobody is using, each removed only while every cargo lock in it is held by this process, so that a build starting at the same moment can never lose its directory under it, and, when asked, the container runtime's cache (I3833).",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "disk.prune",
                kind: CapabilityKind::Command,
                title: "Reclaim the build directories nobody is using, under their locks",
                description: "Every directory under <CARGO_HOME>/build-private and every extra one named: a directory with a file younger than idle_minutes is kept without being locked; otherwise every .cargo-lock and .cargo-build-lock in it is taken exclusively and without waiting, and a lock another process holds keeps the directory; with every lock held it is renamed aside, the locks are released and the renamed directory is removed. check reports what would be removed and removes nothing. The answer names every candidate with its size, its locks and why it was kept, and the free space before and after. idle_minutes is required and zero is refused. With containers, the container runtime's unused build cache and dangling images are pruned too and a colima VM's freed blocks are trimmed back to the host; volumes, containers and tagged images are never touched, a runtime that is down is not started, and the answer names each command run and the VM disk image's allocation before and after.",
                input: DiskPruneInput,
                output: DiskPruneResult,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_disk_prune"),
                    http: post("/api/v1/disk/prune"),
                    cli: Some(CliExposure { path: vec!["disk".into(), "prune".into()] }),
                },
                tags: ["disk", "maintenance", "build"],
                handler: prune,
            }
            // it unlinks directories; the exposure ceiling of every surface follows the effect
            .writes_repository(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "disk");
        let c = &m.capabilities[0].capability;
        assert_eq!(c.id.as_str(), "disk.prune");
        assert_eq!(
            c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_disk_prune")
        );
        assert_eq!(c.exposure.http.as_ref().unwrap().path, "/api/v1/disk/prune");
        assert_eq!(c.exposure.cli.as_ref().unwrap().path, ["disk", "prune"]);
    }
}
