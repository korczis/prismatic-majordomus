//! One lock per repository, around every operation that changes the worktree topology.
//!
//! Two agents working the same repository is the normal case here, not the exotic one, and
//! `git worktree add` twice at once on the same destination is how a half-registered
//! worktree happens. The lock lives under the *common* git directory —
//! `<git-common-dir>/majordomus/locks/worktrees.lock` — which is the one directory every
//! linked worktree of a repository shares and which no other repository can see. So the
//! lock is visible from every worktree, there is exactly one of it, and two different
//! repositories never wait on each other.
//!
//! It is not under `.ai/local/`: that is per-checkout, so a lock there would be one lock per
//! worktree, which is no lock at all. It is not the MCP server's lease either: that elects
//! a process to serve, which is a different question with a different lifetime. Reads take
//! no lock; only a mutation serialises.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::error::{Result, WorktreeError};

/// Where the lock lives under the common git directory.
pub const LOCK_RELATIVE: &str = "majordomus/locks/worktrees.lock";

/// How long a caller waits for the lock before saying who has it.
pub const WAIT: Duration = Duration::from_secs(20);

/// A lock file older than this is treated as abandoned even when its owner cannot be judged
/// (a file with no owner written in it, an owner of another machine's boot that might still
/// be alive). Generous, because a migration of a large worktree across devices can take a
/// while.
pub const STALE_AFTER: Duration = Duration::from_secs(900);

/// The held lock. Dropping it releases it; a panic or an early return therefore cannot leave
/// the repository locked. A process that dies without dropping it — a signal, `SIGKILL`, a
/// crash — leaves the file, and the file names its owner: the next caller sees that the owner
/// is gone (`Owner::is_gone`) and takes the lock at once, instead of waiting out
/// `STALE_AFTER` (OpenTTD, 2026-11-22: `worktrees.lock` held by dead pid 74771 until a
/// person removed it).
#[derive(Debug)]
pub struct WorktreeLock {
    path: PathBuf,
    token: String,
}

impl WorktreeLock {
    /// The lock file for a repository, given its common git directory.
    pub fn path_for(git_common_dir: &Path) -> PathBuf {
        git_common_dir.join(LOCK_RELATIVE)
    }

    /// Take the lock, waiting up to `WAIT` for another holder to finish.
    ///
    /// The primitive is `O_CREAT|O_EXCL`, which is atomic on every filesystem this runs on:
    /// exactly one caller creates the file. A file whose owner is gone, or older than
    /// `STALE_AFTER`, is removed and the attempt repeats, so a killed process is recovered
    /// from without a person being asked to delete anything.
    pub fn acquire(git_common_dir: &Path) -> Result<Self> {
        Self::acquire_with(git_common_dir, WAIT)
    }

    /// The same, with the waiting time as an argument, so a test does not wait twenty
    /// seconds to prove that a held lock is held.
    pub fn acquire_with(git_common_dir: &Path, wait: Duration) -> Result<Self> {
        let path = Self::path_for(git_common_dir);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| WorktreeError::io(dir, &e))?;
        }
        let token = Owner::this_process().token();
        let deadline = Instant::now() + wait;
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    // Best effort: the content is for the person who reads the message, and
                    // the lock is the file's existence, not its content.
                    let _ = f.write_all(token.as_bytes());
                    return Ok(WorktreeLock { path, token });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if Self::is_stale(&path) {
                        // Whoever wrote it is gone. Removing it races with another recoverer
                        // taking the same decision; both then contend on create_new, which
                        // only one of them wins, so the race has no bad outcome.
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    if Instant::now() >= deadline {
                        return Err(WorktreeError::LockUnavailable {
                            holder: fs::read_to_string(&path).ok().map(|s| s.trim().to_string()),
                            path,
                        });
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(WorktreeError::io(&path, &e)),
            }
        }
    }

    fn is_stale(path: &Path) -> bool {
        let owner = fs::read_to_string(path).ok();
        if owner
            .as_deref()
            .and_then(Owner::parse)
            .is_some_and(|o| o.is_gone())
        {
            return true;
        }
        let Ok(meta) = fs::metadata(path) else {
            return false;
        };
        let Ok(modified) = meta.modified() else {
            return false;
        };
        SystemTime::now()
            .duration_since(modified)
            .map(|age| age > STALE_AFTER)
            .unwrap_or(false)
    }

    /// The lock file, for a message.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Who wrote a lock file: `pid <pid> at <seconds> boot <boot>`, the boot as
/// [`crate::process::boot_id`] names it. A file written before the boot was recorded reads
/// `pid <pid> at <seconds>` and is judged by its pid alone.
///
/// The lock lives under the common git directory, which is on this machine's disk, as git's
/// own `index.lock` is; its pid is therefore a pid of this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pid: u32,
    at: u64,
    boot: Option<String>,
}

impl Owner {
    /// This process, now, in this boot.
    pub fn this_process() -> Self {
        Owner {
            pid: std::process::id(),
            at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            boot: crate::process::boot_id(),
        }
    }

    /// The text written into the lock file.
    pub fn token(&self) -> String {
        match &self.boot {
            Some(boot) => format!("pid {} at {} boot {boot}", self.pid, self.at),
            None => format!("pid {} at {}", self.pid, self.at),
        }
    }

    /// The owner a lock file names, or `None` for a file that names none (empty, written
    /// only in part, or not this format).
    pub fn parse(text: &str) -> Option<Self> {
        let mut words = text.split_whitespace();
        let (Some("pid"), Some(pid), Some("at"), Some(at)) =
            (words.next(), words.next(), words.next(), words.next())
        else {
            return None;
        };
        let boot = match (words.next(), words.next(), words.next()) {
            (None, _, _) => None,
            (Some("boot"), Some(boot), None) => Some(boot.to_string()),
            _ => return None,
        };
        Some(Owner {
            pid: pid.parse().ok()?,
            at: at.parse().ok()?,
            boot,
        })
    }

    /// Whether the process that wrote the lock cannot be holding it: it wrote it in another
    /// boot of this machine, or no process with its pid is alive.
    pub fn is_gone(&self) -> bool {
        let other_boot = match (&self.boot, crate::process::boot_id()) {
            (Some(theirs), Some(ours)) => *theirs != ours,
            _ => false,
        };
        other_boot || !crate::lease::alive(self.pid)
    }
}

impl Drop for WorktreeLock {
    fn drop(&mut self) {
        // Only remove the file if it is still this process's: a lock reclaimed as stale by
        // someone else must not then be deleted by the process that lost it.
        if let Ok(content) = fs::read_to_string(&self.path) {
            if content.trim() != self.token {
                return;
            }
        }
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_is_under_the_common_directory_so_every_worktree_sees_one_lock() {
        let p = WorktreeLock::path_for(Path::new("/a/foo/.git"));
        assert_eq!(
            p,
            PathBuf::from("/a/foo/.git/majordomus/locks/worktrees.lock")
        );
    }

    #[test]
    fn a_second_holder_waits_and_then_names_the_first() {
        let dir = tempfile::tempdir().unwrap();
        let held = WorktreeLock::acquire(dir.path()).unwrap();
        let e = WorktreeLock::acquire_with(dir.path(), Duration::from_millis(120)).unwrap_err();
        assert_eq!(e.code(), "LockUnavailable");
        assert!(
            format!("{e}").contains("pid "),
            "the message says who holds it: {e}"
        );
        drop(held);
        let _again = WorktreeLock::acquire_with(dir.path(), Duration::from_millis(10)).unwrap();
    }

    #[test]
    fn releasing_removes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = WorktreeLock::path_for(dir.path());
        {
            let _l = WorktreeLock::acquire(dir.path()).unwrap();
            assert!(path.exists());
        }
        assert!(!path.exists());
    }

    /// A pid no process has: one this test started and reaped.
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        child.wait().unwrap();
        child.id()
    }

    /// Plant a lock file with `content`, as fresh as a lock just taken.
    fn plant(dir: &Path, content: &str) -> PathBuf {
        let path = WorktreeLock::path_for(dir);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    /// A lock whose owner died without releasing it is taken at once, in both the format
    /// that names the boot and the older one that does not.
    #[test]
    fn a_lock_whose_owner_is_dead_is_taken_at_once() {
        let dead = dead_pid();
        let boot = crate::process::boot_id();
        for content in [
            Owner {
                pid: dead,
                at: 1,
                boot: boot.clone(),
            }
            .token(),
            format!("pid {dead} at 1"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            plant(dir.path(), &content);
            let taken = WorktreeLock::acquire_with(dir.path(), Duration::from_millis(120))
                .unwrap_or_else(|e| panic!("a dead owner's lock ({content}) was not taken: {e}"));
            let now = fs::read_to_string(taken.path()).unwrap();
            assert_eq!(Owner::parse(&now).unwrap().pid, std::process::id());
        }
    }

    /// A lock written in another boot is taken although a live process now has its pid.
    #[test]
    fn a_lock_from_another_boot_is_taken_although_its_pid_lives_again() {
        let dir = tempfile::tempdir().unwrap();
        let earlier = Owner {
            pid: std::process::id(),
            at: 1,
            boot: Some("another-boot".into()),
        };
        let expected = crate::process::boot_id().is_some();
        plant(dir.path(), &earlier.token());
        let taken = WorktreeLock::acquire_with(dir.path(), Duration::from_millis(120));
        assert_eq!(taken.is_ok(), expected, "{taken:?}");
    }

    /// A lock whose owner lives, and a fresh one that names no owner, are held.
    #[test]
    fn a_lock_whose_owner_lives_or_cannot_be_named_is_held() {
        for content in [
            Owner::this_process().token(),
            String::new(),
            "pid x at y".into(),
        ] {
            let dir = tempfile::tempdir().unwrap();
            plant(dir.path(), &content);
            let e = WorktreeLock::acquire_with(dir.path(), Duration::from_millis(120))
                .expect_err("a live or unnamed holder keeps the lock");
            assert_eq!(e.code(), "LockUnavailable", "{content:?}");
        }
    }

    /// The owner's text reads back as the owner, and text that is not one names nobody.
    #[test]
    fn an_owner_reads_back_from_its_token() {
        let o = Owner {
            pid: 7,
            at: 9,
            boot: Some("b".into()),
        };
        assert_eq!(o.token(), "pid 7 at 9 boot b");
        assert_eq!(Owner::parse(&o.token()), Some(o));
        let legacy = Owner {
            pid: 7,
            at: 9,
            boot: None,
        };
        assert_eq!(legacy.token(), "pid 7 at 9");
        assert_eq!(Owner::parse("pid 7 at 9\n"), Some(legacy));
        for text in [
            "",
            "pid 7",
            "pid 7 at",
            "lock 7 at 9",
            "pid 7 at 9 boot",
            "pid 7 at 9 x y",
        ] {
            assert_eq!(Owner::parse(text), None, "{text:?}");
        }
        assert_eq!(Owner::parse("pid 7 at 9 boot b extra"), None);
    }

    /// A panic while the lock is held releases it on the way out.
    #[test]
    fn a_panic_while_holding_the_lock_releases_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = WorktreeLock::path_for(dir.path());
        let root = dir.path().to_path_buf();
        let panicked = std::panic::catch_unwind(move || {
            let _held = WorktreeLock::acquire(&root).unwrap();
            panic!("the operation under the lock failed (on purpose)");
        });
        assert!(panicked.is_err());
        assert!(!path.exists(), "the lock is released by the unwinding");
    }

    /// The variable that makes a run of this test binary the child process below.
    #[cfg(unix)]
    const CHILD: &str = "MAJORDOMUS_WORKTREE_LOCK_TEST_CHILD";

    /// A process killed while it holds the lock (`SIGKILL`: no drop, no handler) leaves the
    /// file, and the next caller takes the lock at once rather than after `STALE_AFTER`.
    #[cfg(unix)]
    #[test]
    fn a_holder_killed_outright_does_not_keep_the_lock() {
        use std::os::unix::process::ExitStatusExt;
        if let Some(dir) = std::env::var_os(CHILD) {
            let held = WorktreeLock::acquire(Path::new(&dir)).unwrap();
            std::mem::forget(held);
            // SAFETY: ending this process with a signal no handler can take
            unsafe { libc::raise(libc::SIGKILL) };
            unreachable!("SIGKILL ends the process");
        }
        let dir = tempfile::tempdir().unwrap();
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "worktree::lock::tests::a_holder_killed_outright_does_not_keep_the_lock",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(CHILD, dir.path())
            .output()
            .expect("the child process runs");
        assert_eq!(out.status.signal(), Some(libc::SIGKILL), "{out:?}");
        let left = WorktreeLock::path_for(dir.path());
        assert!(left.exists(), "the killed holder left its lock file");
        let taken = WorktreeLock::acquire_with(dir.path(), Duration::from_millis(500))
            .expect("the dead holder's lock is taken at once");
        drop(taken);
        assert!(!left.exists());
    }

    #[test]
    fn a_lock_reclaimed_by_someone_else_is_not_removed_by_the_loser() {
        let dir = tempfile::tempdir().unwrap();
        let path = WorktreeLock::path_for(dir.path());
        let mine = WorktreeLock::acquire(dir.path()).unwrap();
        fs::write(&path, "pid 999999 at 0").unwrap();
        drop(mine);
        assert!(path.exists(), "the other holder's lock file survives");
        fs::remove_file(&path).unwrap();
    }
}
