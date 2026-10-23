//! The `machine` module: what one machine admits at once (I4006).
//!
//! Three capabilities over [`crate::machine_slot`]: `machine.slot.acquire` takes a slot for a
//! process, waiting in arrival order up to a bound; `machine.slot.release` gives it back; and
//! `machine.slot.status` lists the holders and the waiters with their pids, purposes and ages.
//! The limits are rule `project.the-cooperation-model-is-the-owners-word` clause 7's, read
//! from the policy's `machine.heavy_work` on every call. The slots are this machine's, so
//! every capability here answers a caller on this machine only.
//!
//! ```
//! use majordomus_cli::capability::builtin::modules;
//! let m = modules().into_iter().find(|m| m.id.as_str() == "machine").unwrap();
//! let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, ["machine.slot.status", "machine.slot.acquire", "machine.slot.release"]);
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CapabilityKind, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::machine_slot::{self, Request, SlotAnswer, SlotLimits, SlotRelease, SlotStatus};
use crate::{capability, module};

use super::{get, mcp, post};

/// What to ask of `machine.slot.status`.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlotStatusInput {
    /// A path the work writes to, whose volume's free space is measured; the slot
    /// directory's own volume when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,
}

impl BenchmarkCases for SlotStatusInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("the-machine", SlotStatusInput::default()),
            NamedCase::new(
                "a-build-directory",
                SlotStatusInput {
                    volume: Some("apps/majordomus-cli/target".into()),
                },
            ),
        ]
    }
}

/// What to ask of `machine.slot.acquire`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlotAcquireInput {
    /// The process that will hold the slot: the build's own shell, not the caller of this
    /// capability. It must be running; the slot is reclaimed when it stops.
    pub pid: u32,
    /// What the holder is about to do, in a person's words: it is what every waiter and
    /// every refusal names.
    pub purpose: String,
    /// How long to wait for a slot, in seconds; the policy's `machine.heavy_work.wait_seconds`
    /// when absent, and no wait when that is absent too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_seconds: Option<u64>,
    /// A path the work writes to, whose volume the disk floor is judged on; the slot
    /// directory's own volume when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<String>,
}

/// What to ask of `machine.slot.release`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlotReleaseInput {
    /// The process whose slot is given back.
    pub pid: u32,
}

// The two commands are waived from the benchmark (they change the machine's slots); their
// cases are the examples the OpenAPI document and the Cockpit's runner show, so the pid is a
// fixed illustration: a generated document must not change with the process that wrote it.
const EXAMPLE_PID: u32 = 48_213;
impl BenchmarkCases for SlotAcquireInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "a-build",
            SlotAcquireInput {
                pid: EXAMPLE_PID,
                purpose: "cargo build in this worktree".into(),
                wait_seconds: Some(600),
                volume: None,
            },
        )]
    }
}

impl BenchmarkCases for SlotReleaseInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "a-build",
            SlotReleaseInput { pid: EXAMPLE_PID },
        )]
    }
}

fn root(ctx: &Context) -> PathBuf {
    PathBuf::from(&ctx.index.repository.root)
}

/// The limits the policy declares, from the file on disk: a policy edited while a server runs
/// is the policy the next admission is judged by.
fn limits(root: &Path) -> SlotLimits {
    let loaded = crate::repository::Repository::open(root)
        .ok()
        .and_then(|r| crate::policy::LoadedPolicy::load(&r).ok());
    match loaded {
        Some(l) => SlotLimits::from_policy(&l.policy.machine.heavy_work, &l.path),
        None => SlotLimits::declared(None, None, None),
    }
}

fn directory(root: &Path) -> Result<PathBuf, CapabilityError> {
    machine_slot::directory(root).ok_or_else(|| {
        CapabilityError::Refused(format!(
            "no slot directory: {} is not in a git repository and {} is not set",
            root.display(),
            machine_slot::DIRECTORY_ENV
        ))
    })
}

fn internal(e: std::io::Error) -> CapabilityError {
    match e.kind() {
        std::io::ErrorKind::InvalidInput => CapabilityError::InvalidInput(e.to_string()),
        _ => CapabilityError::Internal(format!("the slot directory: {e}")),
    }
}

fn slot_status(ctx: &Context, input: SlotStatusInput) -> Result<SlotStatus, CapabilityError> {
    let root = root(ctx);
    let dir = directory(&root)?;
    let volume = input.volume.map(PathBuf::from);
    machine_slot::status(&dir, &limits(&root), volume.as_deref()).map_err(internal)
}

fn slot_acquire(ctx: &Context, input: SlotAcquireInput) -> Result<SlotAnswer, CapabilityError> {
    if input.purpose.trim().is_empty() {
        return Err(CapabilityError::InvalidInput(
            "a slot is taken for a purpose a person can read; `purpose` is empty".into(),
        ));
    }
    let root = root(ctx);
    let dir = directory(&root)?;
    let request = Request {
        wait: input.wait_seconds.map(Duration::from_secs),
        volume: input.volume.map(PathBuf::from),
        ..Request::new(input.pid, input.purpose.trim())
    };
    machine_slot::acquire(&dir, &limits(&root), &request).map_err(internal)
}

fn slot_release(ctx: &Context, input: SlotReleaseInput) -> Result<SlotRelease, CapabilityError> {
    let dir = directory(&root(ctx))?;
    machine_slot::release(&dir, input.pid).map_err(internal)
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "machine",
        title: "Machine",
        description: "What one machine admits at once: a counting slot for heavy work (a build, a test run), kept beside the derive lock where every checkout of the machine agrees on it, and held to rule project.the-cooperation-model-is-the-owners-word clause 7 — at most the declared number of holders, and none admitted below the declared free disk — with the numbers read from the policy's machine.heavy_work.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "machine.slot.status",
                title: "Who holds the machine's slots, and who waits",
                description: "The slot directory as it stands: the limits and where they are declared, the free space on the volume measured, every holder with its pid, purpose, age and whether its process still runs (a pid that runs with another start time is a reused pid, and dead), and every waiter with how long it has waited and what it waits on (`machine.slot: 5 of 5 held`) — the progress a waiting session reports. Reads only: a dead holder is reported here and reclaimed by the next acquisition.",
                input: SlotStatusInput,
                output: SlotStatus,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_machine_slot_status"),
                    http: get("/api/v1/machine/slot/status"),
                    cli: Some(CliExposure { path: vec!["machine".into(), "slot".into(), "status".into()] }),
                },
                tags: ["machine", "build", "coordination"],
                cache: CachePolicy::Disabled,
                handler: slot_status,
            }
            // the machine's own processes and what they do: a caller on this machine only
            .reads_only_locally(),
            capability! {
                id: "machine.slot.acquire",
                kind: CapabilityKind::Command,
                title: "Take a machine slot for heavy work",
                description: "Take one of the machine's slots for a running process, named by its pid, with a purpose a person reads. Holders whose process is proven gone — the pid no longer runs, or runs with another start time — are reclaimed first and named. Below the declared free disk it refuses at once; with every slot held it waits in arrival order, listed as a waiter, up to the bound (the input's, else the policy's), and then refuses. A refusal is an answer, not an error: it names the limit, the measured value, every holder with its pid, purpose and age, and what to do instead. A process that already holds a slot is told so and holds one, not two.",
                input: SlotAcquireInput,
                output: SlotAnswer,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_machine_slot_acquire"),
                    http: post("/api/v1/machine/slot/acquire"),
                    cli: Some(CliExposure { path: vec!["machine".into(), "slot".into(), "acquire".into()] }),
                },
                tags: ["machine", "build", "coordination"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: slot_acquire,
            },
            capability! {
                id: "machine.slot.release",
                kind: CapabilityKind::Command,
                title: "Give a machine slot back",
                description: "Give back the slot a process holds, and withdraw it from the queue if it waits; the next waiter is admitted on its next look. Releasing a slot nobody holds answers `released: false` and changes nothing.",
                input: SlotReleaseInput,
                output: SlotRelease,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_machine_slot_release"),
                    http: post("/api/v1/machine/slot/release"),
                    cli: Some(CliExposure { path: vec!["machine".into(), "slot".into(), "release".into()] }),
                },
                tags: ["machine", "build", "coordination"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: slot_release,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine_slot::{SlotRefusalReason, SlotVerdict};

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "machine");
        for e in &m.capabilities {
            let c = &e.capability;
            let words: Vec<&str> = c.id.as_str().split('.').collect();
            assert_eq!(
                c.exposure.cli.as_ref().unwrap().path,
                words,
                "{} is reached by its own words",
                c.id
            );
            assert_eq!(
                c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
                Some(format!("majordomus_{}", words.join("_")))
            );
            assert_eq!(
                c.exposure.http.as_ref().unwrap().path,
                format!("/api/v1/{}", words.join("/"))
            );
            assert!(!c.execution.admits_remote(), "{} is this machine's", c.id);
        }
    }

    /// A context over a synthetic repository whose policy declares the machine's limits.
    fn with_policy(policy: &str) -> (crate::synthetic::SyntheticRepository, Context) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        std::fs::write(repo.root().join(".ai/repo/policy.yaml"), policy).unwrap();
        let index = repo.index().unwrap();
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(vec![module()])
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        (repo, ctx)
    }

    /// The handlers resolve the slot directory from the environment, which a test cannot set
    /// without racing every other test; so the limits are read through the handler's own
    /// function and the judgement through the domain, against a directory of the test's own.
    #[test]
    fn the_limits_are_the_policys_and_absence_is_absence() {
        let (repo, _ctx) = with_policy(
            "version: 1\nmachine:\n  heavy_work:\n    slots: 5\n    min_free_gib: 35\n    wait_seconds: 600\n",
        );
        let l = limits(repo.root());
        assert_eq!(
            (l.slots, l.min_free_gib, l.wait_seconds),
            (Some(5), Some(35), Some(600))
        );
        assert_eq!(l.rule, machine_slot::RULE);
        assert!(
            l.declared_in.ends_with("policy.yaml machine.heavy_work"),
            "{}",
            l.declared_in
        );

        let (bare, _ctx) = with_policy("version: 1\n");
        let l = limits(bare.root());
        assert_eq!(
            (l.slots, l.min_free_gib, l.wait_seconds),
            (None, None, None)
        );
    }

    #[test]
    fn an_acquisition_through_the_executor_is_judged_by_the_policy() {
        let (repo, ctx) = with_policy(
            "version: 1\nmachine:\n  heavy_work:\n    slots: 1\n    min_free_gib: 999999999\n",
        );
        // the directory the handler resolves: beside the synthetic repository once it is a
        // git work tree, in the temporary directory, unless the environment names another
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo.root())
            .args(["init", "-q"])
            .status()
            .expect("git")
            .success());
        let dir = machine_slot::directory(repo.root()).expect("a git repository");
        if std::env::var_os(machine_slot::DIRECTORY_ENV).is_some() {
            return; // the environment points every test at one directory; nothing to prove here
        }
        let input =
            serde_json::json!({"pid": std::process::id(), "purpose": "a test", "wait_seconds": 0});
        let answer: SlotAnswer =
            serde_json::from_value(ctx.execute("machine.slot.acquire", input).unwrap()).unwrap();
        assert_eq!(answer.verdict, SlotVerdict::Refused);
        assert_eq!(answer.refusal.unwrap().reason, SlotRefusalReason::DiskFloor);
        let status: SlotStatus = serde_json::from_value(
            ctx.execute("machine.slot.status", serde_json::json!({}))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(status.held, 0);
        assert_eq!(status.directory, dir.display().to_string());
        let empty = serde_json::json!({"pid": std::process::id(), "purpose": "  "});
        assert!(matches!(
            ctx.execute("machine.slot.acquire", empty),
            Err(CapabilityError::InvalidInput(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
