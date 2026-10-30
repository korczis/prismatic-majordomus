//! Starting a program whose file was written a moment ago.
//!
//! Linux refuses to execute a file that any process holds open for writing (ETXTBSY). A
//! program this process has just written is held so by every child forked while the write
//! was open, until that child execs: a test that writes a stand-in tool and runs it at once
//! falls into that window whenever another test thread forks at the same moment (CI run
//! 39049547899). Writing a copy and renaming it into place does not close the window, since
//! the inherited descriptor holds the inode; waiting does, because the holder lets go as
//! soon as it execs.

use std::io;
use std::time::Duration;

/// How often a busy program is asked again, and how long apart: five seconds in all, far
/// longer than a forked child takes to reach its exec.
const ATTEMPTS: u32 = 50;
const PAUSE: Duration = Duration::from_millis(100);

/// Start a program with `start` — a `Command`'s `output`, `status` or `spawn` — and ask
/// again while the program's file is still open for writing somewhere; the last attempt's
/// answer is returned, so a program that stays busy fails as it would have.
pub(crate) fn retrying_busy<T>(mut start: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut attempts = 1;
    loop {
        match start() {
            Err(e) if e.kind() == io::ErrorKind::ExecutableFileBusy && attempts < ATTEMPTS => {
                attempts += 1;
                std::thread::sleep(PAUSE);
            }
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// A program that cannot be started for another reason is answered at once, with that
    /// reason: only a busy file is waited for.
    #[test]
    fn a_program_that_cannot_start_is_not_asked_again() {
        let mut attempts = 0;
        let r = retrying_busy(|| {
            attempts += 1;
            Command::new("/nonexistent/majordomus-no-such-tool").output()
        });
        assert_eq!(r.unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!(attempts, 1);
    }

    /// A start that is busy twice is asked a third time and its answer returned, and one that
    /// stays busy is answered with the busy error after the bound — on every platform, since
    /// the start is the caller's and the busy error is what the kernel would have said.
    #[test]
    fn a_busy_start_is_asked_again_until_it_answers_or_the_bound_is_spent() {
        let busy = || io::Error::from(io::ErrorKind::ExecutableFileBusy);
        let mut attempts = 0;
        let r = retrying_busy(|| {
            attempts += 1;
            if attempts < 3 {
                Err(busy())
            } else {
                Ok(attempts)
            }
        });
        assert_eq!(r.unwrap(), 3);

        let mut attempts = 0;
        let r: io::Result<()> = retrying_busy(|| {
            attempts += 1;
            Err(busy())
        });
        assert_eq!(r.unwrap_err().kind(), io::ErrorKind::ExecutableFileBusy);
        assert_eq!(
            attempts, ATTEMPTS,
            "a start that stays busy is asked exactly the bound"
        );
    }

    /// Linux alone refuses to exec a file held open for writing, so this holds there: the
    /// program is busy while the writer holds it, and runs once the writer lets go.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_program_still_open_for_writing_runs_once_its_writer_lets_go() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("tool");
        std::fs::write(&tool, "#!/bin/sh\necho ran\n").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        let held = std::fs::OpenOptions::new().write(true).open(&tool).unwrap();
        let busy = Command::new(&tool).output().unwrap_err();
        assert_eq!(busy.kind(), io::ErrorKind::ExecutableFileBusy, "{busy}");
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(held);
        });
        let mut attempts = 0;
        let out = retrying_busy(|| {
            attempts += 1;
            Command::new(&tool).output()
        });
        release.join().unwrap();
        let out = out.expect("the tool runs once its writer lets go");
        assert!(out.status.success(), "{out:?}");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "ran\n");
        assert!(attempts > 1, "the busy tool was asked only once");
    }
}
