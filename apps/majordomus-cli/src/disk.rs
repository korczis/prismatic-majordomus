//! What `disk.prune` reclaims, and the one way it removes a build directory (I3833).
//!
//! The first source is the build directories nobody is using: every directory under
//! `<CARGO_HOME>/build-private`, where a build behind another build's lock falls back to
//! (`lib/rust_bin.sh`, `mj_cargo_private_dir`), and any further directory the caller names,
//! such as an `llvm-cov-target`. On 2026-10-21 this machine went from 44 to 22 GiB free in
//! an evening while such directories sat idle, and a sweep that checked a directory and
//! then removed it deleted one twice under a cargo that had just started on it: the check
//! and the removal are two moments, and a build can begin between them.
//!
//! So removal never trusts a check. A candidate is removed only while this process holds
//! every cargo lock file in it (`.cargo-lock`, `.cargo-build-lock`) exclusively: the
//! directory is renamed aside *while the locks are held*, so a cargo that starts after the
//! rename finds no directory at its path and builds a new one, and a cargo already running
//! holds a lock this process cannot take, so the directory is kept. Only then are the locks
//! released and the renamed directory removed. A directory younger than the idle bound is
//! kept without being locked at all.
//!
//! ```
//! use majordomus_cli::disk::{prune_build_dirs, Kept};
//! let home = tempfile::tempdir().unwrap();
//! let busy = home.path().join("build-private").join("1");
//! std::fs::create_dir_all(&busy).unwrap();
//! std::fs::write(busy.join("artifact"), b"x").unwrap();
//! // written just now: younger than any idle bound, so it is kept and nothing is locked
//! let report = prune_build_dirs(&[home.path().join("build-private")], &[], 30, false);
//! assert_eq!(report.candidates.len(), 1);
//! assert_eq!(report.candidates[0].kept, Some(Kept::Recent));
//! assert!(busy.exists());
//! ```

pub mod containers;

use std::fs::{self, File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The names of the lock files cargo takes in a build directory.
pub const LOCK_FILES: [&str; 2] = [".cargo-lock", ".cargo-build-lock"];

/// Why a candidate was kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kept {
    /// A file in it changed within the idle bound.
    Recent,
    /// A cargo lock in it is held by another process: a build is using it.
    Locked,
    /// It could not be read, locked or renamed; nothing was removed.
    Unreadable,
    /// The call was a check: it would have been removed.
    Check,
}

/// One candidate directory and what became of it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// The directory.
    pub path: String,
    /// Its size in bytes, measured before anything was done to it.
    pub bytes: u64,
    /// How many cargo lock files it holds (all of them were taken before a removal).
    pub locks: usize,
    /// Whether it was removed.
    pub removed: bool,
    /// Why it was kept, when it was.
    pub kept: Option<Kept>,
    /// What went wrong, when something did.
    pub error: Option<String>,
}

/// What a prune of the build directories found and did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuildDirReport {
    /// Every candidate, in path order.
    pub candidates: Vec<Candidate>,
    /// The bytes the removed candidates held.
    pub reclaimed_bytes: u64,
    /// The idle bound, in minutes.
    pub idle_minutes: u64,
    /// Whether the call was a check (nothing removed).
    pub check: bool,
}

/// The candidates: every directory directly under each root, then every extra directory,
/// each once, in path order. A root or extra that does not exist contributes nothing.
fn candidates(roots: &[PathBuf], extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let aside = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(".removing-"));
            if !aside && fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir()) {
                out.push(p);
            }
        }
    }
    for p in extra {
        if fs::symlink_metadata(p).is_ok_and(|m| m.is_dir()) {
            out.push(p.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// What a walk of a directory learns: its size, whether anything in it is newer than
/// `since`, and the cargo lock files it holds. Symbolic links are not followed.
struct Walk {
    bytes: u64,
    recent: bool,
    locks: Vec<PathBuf>,
}

fn walk(dir: &Path, since: SystemTime) -> std::io::Result<Walk> {
    let mut w = Walk {
        bytes: 0,
        recent: false,
        locks: Vec::new(),
    };
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)? {
            let e = e?;
            let m = fs::symlink_metadata(e.path())?;
            if m.modified().is_ok_and(|t| t > since) {
                w.recent = true;
            }
            if m.is_dir() {
                stack.push(e.path());
            } else {
                w.bytes += m.len();
                if e.file_name()
                    .to_str()
                    .is_some_and(|n| LOCK_FILES.contains(&n))
                {
                    w.locks.push(e.path());
                }
            }
        }
    }
    Ok(w)
}

/// Take an exclusive, non-blocking flock on every path. Every lock taken is returned as an
/// open file, which releases it when dropped; `Err` names the first path another process
/// holds, and every lock taken before it is released on the way out.
fn lock_all(paths: &[PathBuf]) -> Result<Vec<File>, (PathBuf, std::io::Error)> {
    let mut held = Vec::with_capacity(paths.len());
    for p in paths {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(p)
            .map_err(|e| (p.clone(), e))?;
        // SAFETY: flock takes a file descriptor this process owns for the life of `f`.
        let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            return Err((p.clone(), std::io::Error::last_os_error()));
        }
        held.push(f);
    }
    Ok(held)
}

/// Prune the build directories under `roots` and the `extra` ones: every candidate whose
/// files are all older than `idle_minutes` and whose cargo locks this process can all take
/// is renamed aside while the locks are held and then removed; `check` reports what would
/// be removed and touches nothing.
pub fn prune_build_dirs(
    roots: &[PathBuf],
    extra: &[PathBuf],
    idle_minutes: u64,
    check: bool,
) -> BuildDirReport {
    let since = SystemTime::now() - Duration::from_secs(idle_minutes.saturating_mul(60));
    let mut report = BuildDirReport {
        candidates: Vec::new(),
        reclaimed_bytes: 0,
        idle_minutes,
        check,
    };
    for dir in candidates(roots, extra) {
        let mut c = Candidate {
            path: dir.display().to_string(),
            bytes: 0,
            locks: 0,
            removed: false,
            kept: None,
            error: None,
        };
        let w = match walk(&dir, since) {
            Ok(w) => w,
            Err(e) => {
                // a walk with a hole is no proof of idleness: keep it
                c.kept = Some(Kept::Unreadable);
                c.error = Some(e.to_string());
                report.candidates.push(c);
                continue;
            }
        };
        c.bytes = w.bytes;
        c.locks = w.locks.len();
        if w.recent {
            c.kept = Some(Kept::Recent);
            report.candidates.push(c);
            continue;
        }
        let held = match lock_all(&w.locks) {
            Ok(h) => h,
            Err((p, e)) => {
                c.kept = Some(Kept::Locked);
                c.error = Some(format!("{}: {e}", p.display()));
                report.candidates.push(c);
                continue;
            }
        };
        if check {
            drop(held);
            c.kept = Some(Kept::Check);
            report.candidates.push(c);
            continue;
        }
        // renamed aside while every lock is held: a cargo that starts now finds no
        // directory at this path, and one that waits on a lock waits on the renamed one
        let aside = PathBuf::from(format!("{}.removing-{}", dir.display(), std::process::id()));
        let renamed = fs::rename(&dir, &aside);
        drop(held);
        match renamed {
            Ok(()) => match fs::remove_dir_all(&aside) {
                Ok(()) => {
                    c.removed = true;
                    report.reclaimed_bytes += c.bytes;
                }
                Err(e) => {
                    c.error = Some(format!(
                        "renamed to {} but not removed: {e}",
                        aside.display()
                    ));
                }
            },
            Err(e) => {
                c.kept = Some(Kept::Unreadable);
                c.error = Some(format!("not renamed: {e}"));
            }
        }
        report.candidates.push(c);
    }
    report
}

/// The free bytes of the volume holding `path`, or `None` when it cannot be measured.
pub fn free_bytes(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs writes into a zeroed struct this function owns.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c.as_ptr(), &mut s) };
    if rc != 0 {
        return None;
    }
    #[allow(clippy::unnecessary_cast)]
    Some(s.f_bavail as u64 * s.f_frsize as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory under a root, with one file and the two cargo locks, aged `minutes` ago.
    fn build_dir(root: &Path, name: &str, minutes: u64) -> PathBuf {
        let d = root.join(name);
        fs::create_dir_all(d.join("debug/deps")).unwrap();
        fs::write(d.join("debug/deps/libx.rlib"), vec![0u8; 1024]).unwrap();
        for l in LOCK_FILES {
            fs::write(d.join("debug").join(l), b"").unwrap();
        }
        let then = SystemTime::now() - Duration::from_secs(minutes * 60);
        let mut stack = vec![d.clone()];
        while let Some(p) = stack.pop() {
            for e in fs::read_dir(&p).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                }
                File::options()
                    .write(true)
                    .open(e.path())
                    .or_else(|_| File::open(e.path()))
                    .and_then(|f| f.set_modified(then))
                    .ok();
            }
        }
        d
    }

    #[test]
    fn an_idle_unlocked_directory_is_renamed_aside_and_removed() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("build-private");
        let idle = build_dir(&root, "1", 120);
        let r = prune_build_dirs(std::slice::from_ref(&root), &[], 30, false);
        assert_eq!(r.candidates.len(), 1, "{r:?}");
        assert!(r.candidates[0].removed, "{r:?}");
        assert_eq!(r.candidates[0].locks, 2);
        assert!(r.reclaimed_bytes >= 1024);
        assert!(!idle.exists());
        assert_eq!(
            fs::read_dir(&root).unwrap().count(),
            0,
            "nothing was left aside"
        );
    }

    #[test]
    fn a_directory_whose_lock_another_holder_has_is_kept() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("build-private");
        let busy = build_dir(&root, "1", 120);
        // a cargo that is running: it holds the build lock
        let holder = File::options()
            .read(true)
            .write(true)
            .open(busy.join("debug/.cargo-build-lock"))
            .unwrap();
        let fd = holder.as_raw_fd();
        // SAFETY: the test owns `holder` for the whole call
        assert_eq!(unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) }, 0);
        let r = prune_build_dirs(std::slice::from_ref(&root), &[], 30, false);
        assert_eq!(r.candidates[0].kept, Some(Kept::Locked), "{r:?}");
        assert!(!r.candidates[0].removed);
        assert!(busy.exists(), "a directory in use was removed");
        drop(holder);
    }

    #[test]
    fn a_recent_directory_is_kept_and_a_check_removes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("build-private");
        let recent = build_dir(&root, "1", 0);
        let idle = build_dir(&root, "2", 120);
        let r = prune_build_dirs(std::slice::from_ref(&root), &[], 30, true);
        let by = |p: &Path| {
            r.candidates
                .iter()
                .find(|c| c.path == p.display().to_string())
                .unwrap()
                .kept
        };
        assert_eq!(by(&recent), Some(Kept::Recent));
        assert_eq!(by(&idle), Some(Kept::Check));
        assert!(
            recent.exists() && idle.exists(),
            "a check removed something"
        );
        assert_eq!(r.reclaimed_bytes, 0);
    }

    #[test]
    fn an_extra_directory_is_a_candidate_and_a_missing_root_is_nothing() {
        let home = tempfile::tempdir().unwrap();
        let cov = build_dir(home.path(), "llvm-cov-target", 120);
        let r = prune_build_dirs(
            &[home.path().join("absent")],
            std::slice::from_ref(&cov),
            30,
            false,
        );
        assert_eq!(r.candidates.len(), 1, "{r:?}");
        assert!(r.candidates[0].removed);
        assert!(!cov.exists());
    }

    #[test]
    fn the_free_space_of_a_real_directory_is_measured() {
        let home = tempfile::tempdir().unwrap();
        assert!(free_bytes(home.path()).is_some());
        assert!(free_bytes(&home.path().join("no-such-dir")).is_none());
    }
}
