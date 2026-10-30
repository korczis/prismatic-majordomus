//! The machine section: free disk against the builder floor, swap, and builders against
//! their limit. No network, one `ps`, no write.
//!
//! The night of 2026-10-28 was lost to the machine rather than to the code: the disk under
//! the 35 GiB floor, swap at 91 %, and builders waiting on one lock. None of it was in the
//! brief, so no session reported it. The limits are the policy's `machine.heavy_work`
//! (`slots`, `min_free_gib`), the same two numbers the machine slot admits builders by
//! (I4006); the brief only reads them.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{
    BriefMachineSection, BriefObservation, BriefProcess, BriefSectionState, BriefVerdict,
    BriefVolume,
};

/// How long the one `ps` may take.
pub(super) const PS_BUDGET: Duration = Duration::from_secs(1);

/// One row of the process table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Proc {
    pub(crate) pid: u32,
    pub(crate) ppid: u32,
    pub(crate) age_seconds: u64,
    pub(crate) command: String,
}

/// The process table from one `ps -Ao pid,ppid,etime,comm`, bounded by [`PS_BUDGET`].
pub(crate) fn process_table() -> Result<Vec<Proc>, String> {
    let mut child = Command::new("ps")
        .args(["-Ao", "pid=,ppid=,etime=,comm="])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("ps could not be started: {e}"))?;
    let mut out = child.stdout.take().ok_or("ps has no output")?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut out, &mut s);
        s
    });
    let deadline = Instant::now() + PS_BUDGET;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            _ if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("ps took longer than {} ms", PS_BUDGET.as_millis()));
            }
        }
    }
    let text = reader.join().unwrap_or_default();
    Ok(parse_ps(&text))
}

/// The rows of `ps -Ao pid=,ppid=,etime=,comm=` output.
pub(crate) fn parse_ps(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let pid = w.next()?.parse().ok()?;
            let ppid = w.next()?.parse().ok()?;
            let age_seconds = etime_seconds(w.next()?)?;
            let command = w.collect::<Vec<_>>().join(" ");
            Some(Proc {
                pid,
                ppid,
                age_seconds,
                command,
            })
        })
        .collect()
}

/// `ps` elapsed time (`[[dd-]hh:]mm:ss`) in seconds.
pub(crate) fn etime_seconds(e: &str) -> Option<u64> {
    let (days, rest) = match e.split_once('-') {
        Some((d, r)) => (d.parse::<u64>().ok()?, r),
        None => (0, e),
    };
    let parts: Vec<u64> = rest
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let (h, m, s) = match parts.as_slice() {
        [m, s] => (0, *m, *s),
        [h, m, s] => (*h, *m, *s),
        _ => return None,
    };
    Some(days * 86_400 + h * 3600 + m * 60 + s)
}

/// The basename of a command path.
pub(crate) fn base(command: &str) -> &str {
    command.rsplit('/').next().unwrap_or(command)
}

/// Whether a process is a builder: cargo or rustc.
pub(crate) fn is_builder(p: &Proc) -> bool {
    matches!(base(&p.command), "cargo" | "rustc")
}

/// Swap used and total, in bytes: `vm.swapusage` on macOS, `/proc/meminfo` on Linux.
pub(crate) fn swap() -> Option<(u64, u64)> {
    #[cfg(target_os = "macos")]
    {
        swap_macos()
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|t| parse_meminfo(&t))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn swap_macos() -> Option<(u64, u64)> {
    // struct xsw_usage { u_int64_t xsu_total, xsu_avail, xsu_used; u_int32_t pagesize; boolean_t encrypted; }
    #[repr(C)]
    struct XswUsage {
        total: u64,
        avail: u64,
        used: u64,
        pagesize: u32,
        encrypted: i32,
    }
    let name = std::ffi::CString::new("vm.swapusage").ok()?;
    let mut value = std::mem::MaybeUninit::<XswUsage>::zeroed();
    let mut len = std::mem::size_of::<XswUsage>();
    // SAFETY: `name` is NUL-terminated, `value` is a writable buffer of `len` bytes, and it is
    // read only when the call reports success with the length it wrote.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            value.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || len < 24 {
        return None;
    }
    // SAFETY: the call succeeded and wrote at least the three counters.
    let v = unsafe { value.assume_init() };
    Some((v.used, v.total))
}

/// Swap used and total from `/proc/meminfo` text, in bytes.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let kib = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|r| r.split_whitespace().next())
            .and_then(|n| n.parse::<u64>().ok())
    };
    let total = kib("SwapTotal:")? * 1024;
    let free = kib("SwapFree:")? * 1024;
    Some((total.saturating_sub(free), total))
}

/// The volumes the brief measures: the primary checkout's, and the build directory's when
/// it is another volume.
fn volumes(primary: &Path) -> Vec<PathBuf> {
    let mut v = vec![primary.to_path_buf()];
    let build = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join("dev/build-shared")));
    if let Some(b) = build {
        if device(&b) != device(primary) && device(&b).is_some() {
            v.push(b);
        }
    }
    v
}

fn device(p: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    p.ancestors()
        .find_map(|a| std::fs::metadata(a).ok())
        .map(|m| m.dev())
}

/// The limits the policy declares: `machine.heavy_work.min_free_gib` and `.slots`.
fn limits(root: &Path) -> (Option<u64>, Option<u64>) {
    crate::repository::Repository::open(root)
        .ok()
        .and_then(|r| crate::policy::LoadedPolicy::load(&r).ok())
        .map(|p| {
            let h = p.policy.machine.heavy_work;
            (h.min_free_gib, h.slots.map(u64::from))
        })
        .unwrap_or((None, None))
}

/// The machine section from what was measured: kept apart from the measuring so that a test
/// hands it the figures.
pub(crate) fn section(
    volumes: Vec<BriefVolume>,
    swap: Option<(u64, u64)>,
    procs: &Result<Vec<Proc>, String>,
    floor_gib: Option<u64>,
    max_builders: Option<u64>,
) -> BriefMachineSection {
    let free_bytes = volumes.iter().map(|v| v.free_bytes).min();
    let builders_list: Vec<BriefProcess> = procs
        .as_ref()
        .map(|ps| {
            ps.iter()
                .filter(|p| is_builder(p))
                .map(|p| BriefProcess {
                    pid: p.pid,
                    command: base(&p.command).to_string(),
                    age_seconds: p.age_seconds,
                })
                .collect()
        })
        .unwrap_or_default();
    let builders = procs.as_ref().ok().map(|_| {
        // a cargo and the rustc it runs are one build: count the cargo processes, and the
        // rustc ones only when no cargo is there to own them
        let cargo = builders_list
            .iter()
            .filter(|b| b.command == "cargo")
            .count() as u64;
        if cargo > 0 {
            cargo
        } else {
            builders_list.len() as u64
        }
    });
    let gib = super::report::gib;
    let mut words: Vec<String> = Vec::new();
    let mut worst = "current";
    let mut verdict_of = |w: &'static str| {
        let rank = |w: &str| match w {
            "blocked" => 2,
            "degraded" => 1,
            _ => 0,
        };
        if rank(w) > rank(worst) {
            worst = w;
        }
    };
    match (free_bytes, floor_gib) {
        (Some(f), Some(floor)) => {
            let below = f < floor.saturating_mul(1 << 30);
            if below {
                verdict_of("blocked");
            }
            words.push(format!(
                "disk {} GiB free (floor {floor} GiB) [{}]",
                gib(f),
                if below { "blocked" } else { "current" }
            ));
        }
        (Some(f), None) => words.push(format!("disk {} GiB free (no floor declared)", gib(f))),
        (None, _) => words.push("disk [unread]".into()),
    }
    match swap.and_then(|(used, total)| {
        used.saturating_mul(100)
            .checked_div(total)
            .map(|p| (used, total, p))
    }) {
        Some((used, total, pct)) => {
            let w = if pct > super::SWAP_DEGRADED_PERCENT {
                "degraded"
            } else {
                "current"
            };
            if w == "degraded" {
                verdict_of("degraded");
            }
            words.push(format!(
                "swap {}/{} GiB ({pct} %) [{w}]",
                gib(used),
                gib(total)
            ));
        }
        None if swap.is_some() => words.push("swap none".into()),
        None => words.push("swap [unread]".into()),
    }
    match (builders, max_builders) {
        (Some(n), Some(max)) => {
            let full = max > 0 && n >= max;
            if full {
                verdict_of("blocked");
            }
            words.push(format!(
                "builders {n}/{max}{}",
                if full { " [blocked]" } else { "" }
            ));
        }
        (Some(n), None) => words.push(format!("builders {n} (no limit declared)")),
        (None, _) => words.push(format!(
            "builders [unread] ({})",
            procs.as_ref().err().cloned().unwrap_or_default()
        )),
    }
    let state = if free_bytes.is_some() && swap.is_some() && builders.is_some() {
        BriefSectionState::Read
    } else {
        BriefSectionState::Partial
    };
    let summary = format!(
        "[{worst}] {}; statvfs, sysctl and ps, read now",
        words.join("; ")
    );
    BriefMachineSection {
        state,
        observed: BriefObservation {
            source: "statvfs, sysctl vm.swapusage or /proc/meminfo, ps".into(),
            verdict: if state == BriefSectionState::Read {
                BriefVerdict::Current
            } else {
                BriefVerdict::Partial
            },
            remedy: (state != BriefSectionState::Read)
                .then(|| "df -g / ; sysctl vm.swapusage".into()),
            ..BriefObservation::now()
        },
        volumes,
        free_bytes,
        floor_gib,
        swap_used_bytes: swap.map(|s| s.0),
        swap_total_bytes: swap.map(|s| s.1),
        builders,
        max_builders,
        builder_processes: builders_list,
        word: worst.to_string(),
        summary,
    }
}

/// Measure the machine now.
pub(crate) fn measure(root: &Path, procs: &Result<Vec<Proc>, String>) -> BriefMachineSection {
    // the primary checkout: the directory holding the common git directory
    let primary = super::refs::git_dir(root)
        .map(|g| super::refs::common_dir(&g))
        .and_then(|c| c.canonicalize().ok())
        .and_then(|c| c.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| root.to_path_buf());
    let measured: Vec<BriefVolume> = volumes(&primary)
        .into_iter()
        .filter_map(|p| {
            crate::mesh::build_cache::ensure::free_bytes(&p).map(|free_bytes| BriefVolume {
                path: p.display().to_string(),
                free_bytes,
            })
        })
        .collect();
    let (floor, max) = limits(root);
    section(measured, swap(), procs, floor, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    fn procs(n: usize) -> Result<Vec<Proc>, String> {
        Ok((0..n)
            .map(|i| Proc {
                pid: 100 + i as u32,
                ppid: 1,
                age_seconds: 60,
                command: "/x/bin/cargo".into(),
            })
            .collect())
    }

    fn vol(free_gib: u64) -> Vec<BriefVolume> {
        vec![BriefVolume {
            path: "/".into(),
            free_bytes: free_gib * GIB,
        }]
    }

    #[test]
    fn machine_verdicts() {
        let fine = section(vol(41), Some((GIB, 10 * GIB)), &procs(0), Some(35), Some(5));
        assert_eq!(fine.word, "current", "{}", fine.summary);
        let low = section(vol(20), Some((GIB, 10 * GIB)), &procs(0), Some(35), Some(5));
        assert_eq!(low.word, "blocked", "{}", low.summary);
        assert!(
            low.summary
                .contains("disk 20.0 GiB free (floor 35 GiB) [blocked]"),
            "{}",
            low.summary
        );
        let swapping = section(
            vol(41),
            Some((9 * GIB, 10 * GIB)),
            &procs(0),
            Some(35),
            Some(5),
        );
        assert_eq!(swapping.word, "degraded");
        assert!(
            swapping.summary.contains("(90 %) [degraded]"),
            "{}",
            swapping.summary
        );
        let full = section(vol(41), Some((GIB, 10 * GIB)), &procs(5), Some(35), Some(5));
        assert_eq!(full.word, "blocked");
        assert!(
            full.summary.contains("builders 5/5 [blocked]"),
            "{}",
            full.summary
        );
        let unread = section(
            vol(41),
            None,
            &Err("ps took longer than 1000 ms".into()),
            None,
            None,
        );
        assert_eq!(unread.state, BriefSectionState::Partial);
        assert_eq!(unread.observed.verdict, BriefVerdict::Partial);
        assert!(unread.observed.remedy.is_some());
    }

    /// The floor and the limit are the policy's: this repository declares them.
    #[test]
    fn the_floor_is_read_from_the_policy() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (floor, max) = limits(&root);
        assert_eq!((floor, max), (Some(35), Some(5)));
    }

    #[test]
    fn the_process_table_and_its_times_parse() {
        assert_eq!(etime_seconds("05:03"), Some(303));
        assert_eq!(etime_seconds("1:00:00"), Some(3600));
        assert_eq!(etime_seconds("2-01:00:00"), Some(2 * 86_400 + 3600));
        assert_eq!(etime_seconds("x"), None);
        let rows = parse_ps("  10     1    01:00 /usr/bin/cargo\n 11 10 00:05 rustc\nbad\n");
        assert_eq!(rows.len(), 2);
        assert!(is_builder(&rows[0]) && is_builder(&rows[1]));
        assert_eq!(rows[1].ppid, 10);
        assert_eq!(
            parse_meminfo("SwapTotal: 100 kB\nSwapFree: 25 kB\n"),
            Some((75 * 1024, 100 * 1024))
        );
        assert!(process_table().is_ok(), "this machine has a ps");
    }
}
