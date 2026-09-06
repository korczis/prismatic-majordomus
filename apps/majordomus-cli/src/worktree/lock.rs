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

/// A lock file older than this is treated as abandoned: a process that died between taking
/// the lock and releasing it must not stop the repository for ever. Generous, because a
/// migration of a large worktree across devices can take a while.
pub const STALE_AFTER: Duration = Duration::from_secs(900);

/// The held lock. Dropping it releases it; a panic or an early return therefore cannot leave
/// the repository locked.
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

    /// Take the lock, waiting up to [`WAIT`] for another holder to finish.
    ///
    /// The primitive is `O_CREAT|O_EXCL`, which is atomic on every filesystem this runs on:
    /// exactly one caller creates the file. A file older than [`STALE_AFTER`] is removed and
    /// the attempt repeats, so a killed process is recovered from without a person being
    /// asked to delete anything.
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
        let token = format!(
            "pid {} at {}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );
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
