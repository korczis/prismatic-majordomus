//! The second source `disk.prune` reclaims: the container runtime's cache (I3833).
//!
//! On 2026-10-21 the colima VM's disk image held 73 GiB of this machine's 460, most of it
//! build cache and layers no container used, and it gave 30 GiB back to a builder prune and
//! an `fstrim` inside the VM. What is pruned is only what is cache by construction: the
//! build cache no running build holds (`docker builder prune --all`) and the dangling
//! images no tag names (`docker image prune`). Volumes, containers and tagged images are
//! never touched: a volume is somebody's database, and a tagged image is what a stack
//! comes back up from without a network. Then, under colima, `fstrim` inside the VM hands
//! the freed blocks back to the host, without which the prune frees space only inside a
//! disk image that does not shrink.
//!
//! A runtime that is not running is not started to be pruned: a start runs every
//! container with a restart policy. Its disk image's allocation is reported instead, so the
//! reader knows what a prune would be working on.
//!
//! ```
//! use majordomus_cli::disk::containers::reclaimable_of;
//! let df = r#"{"Type":"Images","Reclaimable":"1.2GB (40%)"}
//! {"Type":"Build Cache","Reclaimable":"9.8GB"}"#;
//! let r = reclaimable_of(df);
//! assert_eq!(r[1].kind, "Build Cache");
//! assert_eq!(r[1].reclaimable, "9.8GB");
//! ```

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::environment::probe::bounded_output;

/// What docker says one kind of object would give back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Reclaimable {
    /// `Images`, `Containers`, `Local Volumes`, `Build Cache`.
    pub kind: String,
    /// As docker writes it (`9.8GB (40%)`).
    pub reclaimable: String,
}

/// One command run against the runtime.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    /// The command line.
    pub command: String,
    /// It exited zero within its budget.
    pub ok: bool,
    /// Its last line of output, or why it did not finish.
    pub detail: String,
}

/// What the container runtime held, what was run against it, and what it holds after.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ContainerReport {
    /// `colima` or `docker`; absent when there is no docker on PATH.
    pub runtime: Option<String>,
    /// The runtime answered.
    pub running: bool,
    /// Nothing was run; the report says what would be.
    pub check: bool,
    /// The VM's disk image, when colima keeps one.
    pub disk_image: Option<String>,
    /// Bytes the disk image occupies on the host before the prune.
    pub allocated_before: Option<u64>,
    /// And after it.
    pub allocated_after: Option<u64>,
    /// Docker's own reading of what each kind would give back, before the prune.
    pub reclaimable: Vec<Reclaimable>,
    /// The commands run, in order.
    pub steps: Vec<Step>,
    /// Why nothing was run, when nothing was.
    pub note: Option<String>,
}

/// The programs the prune talks to; resolved from PATH by [`Runtime::from_path`], named
/// directly by a test.
#[derive(Debug, Clone)]
pub struct Runtime {
    /// The docker client.
    pub docker: Option<PathBuf>,
    /// The colima executable, when installed.
    pub colima: Option<PathBuf>,
    /// The VM's disk image, when there is one.
    pub disk_image: Option<PathBuf>,
}

impl Runtime {
    /// `docker` and `colima` from PATH, and colima's data disk under `$COLIMA_HOME` (else
    /// `~/.colima`).
    pub fn from_path() -> Self {
        let colima_home = std::env::var_os("COLIMA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".colima")));
        let disk_image = colima_home.and_then(|h| {
            [
                h.join("_lima/_disks/colima/datadisk"),
                h.join("_lima/colima/diffdisk"),
            ]
            .into_iter()
            .find(|p| p.is_file())
        });
        Runtime {
            docker: on_path("docker"),
            colima: on_path("colima"),
            disk_image,
        }
    }
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// The bytes a sparse file occupies, not the size it claims.
fn allocated(p: &Path) -> Option<u64> {
    std::fs::metadata(p).ok().map(|m| m.blocks() * 512)
}

/// `docker system df --format '{{json .}}'`, read: one object per line.
pub fn reclaimable_of(df: &str) -> Vec<Reclaimable> {
    df.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| {
            Some(Reclaimable {
                kind: v.get("Type")?.as_str()?.to_string(),
                reclaimable: v.get("Reclaimable")?.as_str()?.to_string(),
            })
        })
        .collect()
}

fn step(program: &Path, args: &[&str], budget: Duration) -> Step {
    let command = std::iter::once(
        program
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
    .chain(args.iter().map(|a| a.to_string()))
    .collect::<Vec<_>>()
    .join(" ");
    match bounded_output(Command::new(program).args(args), budget) {
        None => Step {
            command,
            ok: false,
            detail: format!("did not finish within {} s", budget.as_secs()),
        },
        Some(o) => {
            let text = String::from_utf8_lossy(&o.stdout).into_owned()
                + String::from_utf8_lossy(&o.stderr).as_ref();
            let last = text
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("");
            Step {
                command,
                ok: o.status.success(),
                detail: last.trim().chars().take(200).collect(),
            }
        }
    }
}

/// Prune the container runtime's cache, or with `check` only say what it holds.
pub fn prune_containers(rt: &Runtime, check: bool) -> ContainerReport {
    let mut r = ContainerReport {
        check,
        disk_image: rt.disk_image.as_ref().map(|p| p.display().to_string()),
        allocated_before: rt.disk_image.as_deref().and_then(allocated),
        ..ContainerReport::default()
    };
    let Some(docker) = rt.docker.as_deref() else {
        r.note = Some("no docker on PATH".into());
        return r;
    };
    r.runtime = Some(
        if rt.colima.is_some() {
            "colima"
        } else {
            "docker"
        }
        .into(),
    );
    let df = bounded_output(
        Command::new(docker).args(["system", "df", "--format", "{{json .}}"]),
        Duration::from_secs(30),
    );
    match df {
        Some(o) if o.status.success() => {
            r.running = true;
            r.reclaimable = reclaimable_of(&String::from_utf8_lossy(&o.stdout));
        }
        _ => {
            r.note = Some(
                "the container runtime is not running, and it is not started to be pruned: a \
                 start runs every container with a restart policy"
                    .into(),
            );
            r.allocated_after = r.allocated_before;
            return r;
        }
    }
    if check {
        r.note = Some("check: nothing was run".into());
        r.allocated_after = r.allocated_before;
        return r;
    }
    let long = Duration::from_secs(600);
    r.steps.push(step(
        docker,
        &["builder", "prune", "--all", "--force"],
        long,
    ));
    r.steps
        .push(step(docker, &["image", "prune", "--force"], long));
    if let Some(colima) = rt.colima.as_deref() {
        // the freed blocks stay inside the VM's disk image until the VM discards them
        r.steps
            .push(step(colima, &["ssh", "--", "sudo", "fstrim", "-a"], long));
    }
    r.allocated_after = rt.disk_image.as_deref().and_then(allocated);
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// An executable script at `dir/name` that appends its arguments to `dir/calls` and
    /// runs `body`.
    fn fake(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        let calls = dir.join("calls");
        std::fs::write(
            &p,
            format!(
                "#!/bin/sh\necho \"{name} $*\" >> '{}'\n{body}\n",
                calls.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn calls(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    const DF: &str =
        r#"[ "$1" = system ] && echo '{"Type":"Build Cache","Reclaimable":"3GB"}'; exit 0"#;

    #[test]
    fn a_runtime_that_is_down_is_not_started_and_nothing_is_run() {
        let t = tempfile::tempdir().unwrap();
        let rt = Runtime {
            docker: Some(fake(t.path(), "docker", "exit 1")),
            colima: Some(fake(t.path(), "colima", "exit 0")),
            disk_image: None,
        };
        let r = prune_containers(&rt, false);
        assert!(!r.running);
        assert!(r.steps.is_empty());
        assert!(r.note.unwrap().contains("not started"));
        assert_eq!(calls(t.path()), ["docker system df --format {{json .}}"]);
    }

    #[test]
    fn a_check_reads_the_runtime_and_runs_nothing() {
        let t = tempfile::tempdir().unwrap();
        let rt = Runtime {
            docker: Some(fake(t.path(), "docker", DF)),
            colima: None,
            disk_image: None,
        };
        let r = prune_containers(&rt, true);
        assert!(r.running);
        assert_eq!(r.reclaimable[0].reclaimable, "3GB");
        assert!(r.steps.is_empty());
        assert_eq!(calls(t.path()).len(), 1);
    }

    #[test]
    fn a_prune_touches_cache_only_and_trims_under_colima() {
        let t = tempfile::tempdir().unwrap();
        let image = t.path().join("datadisk");
        std::fs::write(&image, b"x").unwrap();
        let rt = Runtime {
            docker: Some(fake(t.path(), "docker", DF)),
            colima: Some(fake(t.path(), "colima", "exit 0")),
            disk_image: Some(image),
        };
        let r = prune_containers(&rt, false);
        assert_eq!(r.runtime.as_deref(), Some("colima"));
        assert!(r.steps.iter().all(|s| s.ok), "{:?}", r.steps);
        let c = calls(t.path());
        assert_eq!(
            c[1..],
            [
                "docker builder prune --all --force",
                "docker image prune --force",
                "colima ssh -- sudo fstrim -a",
            ]
        );
        // never a volume, a container, a system prune or every image
        assert!(!c.iter().any(|l| l.contains("volume")
            || l.contains("container")
            || l.contains("system prune")
            || l.contains("image prune --all")
            || l.contains("image prune -a")));
        assert!(r.allocated_before.is_some() && r.allocated_after.is_some());
    }

    #[test]
    fn without_docker_the_report_says_so() {
        let rt = Runtime {
            docker: None,
            colima: None,
            disk_image: None,
        };
        let r = prune_containers(&rt, false);
        assert_eq!(r.runtime, None);
        assert_eq!(r.note.as_deref(), Some("no docker on PATH"));
    }
}
