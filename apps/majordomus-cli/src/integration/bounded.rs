//! Subprocesses that reach the network, bounded in time.
//!
//! The executor asks the forge and the remote through `gh` and `git`, and both can hang: a
//! connection that neither answers nor closes, a credential helper waiting for a person who
//! is not there. A call that never returns holds the base branch's integration lease and
//! every drain behind it, and says nothing. [`output_within`] gives such a call a limit:
//! past it the child is killed and the call fails with an error that says it timed out,
//! which [`super::retry::transient`] reads as an outage, so a read is asked again and a merge
//! whose answer was lost is verified rather than repeated.
//!
//! Only calls that reach the network are bounded. Local git answers at once or fails, a
//! commit runs the repository's hooks, and `scripts/derive` builds: none of them has one
//! limit that is right for it, and the lease is kept alive while they run.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A read of the forge or of the remote's refs: `gh` listing or viewing, `git ls-remote`.
pub const FORGE_READ: Duration = Duration::from_secs(60);
/// An act on the forge: `gh pr merge`, `gh pr close`.
pub const FORGE_WRITE: Duration = Duration::from_secs(120);
/// A transfer of objects: `git fetch`, `git push`.
pub const GIT_TRANSFER: Duration = Duration::from_secs(300);

/// How often a running child is asked whether it has ended.
const POLL: Duration = Duration::from_millis(25);
/// How long the output of a child that has ended is waited for. A grandchild that kept the
/// pipe open would otherwise hold the call after the child itself is gone.
const DRAINED_WITHIN: Duration = Duration::from_secs(2);

/// One stream of a child, read on a thread of its own so a child that writes more than a
/// pipe holds never waits on a reader that is waiting on it. What has been read is shared,
/// so it is still there when the stream never ends.
struct Stream {
    read: Arc<Mutex<Vec<u8>>>,
    ended: Receiver<()>,
}

fn reader<R: Read + Send + 'static>(pipe: Option<R>) -> Stream {
    let read = Arc::new(Mutex::new(Vec::new()));
    let (tx, ended) = channel();
    let into = Arc::clone(&read);
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        let mut pipe = pipe;
        // until the stream ends: a read that fails ends it like its end does
        while let Some(n) = pipe
            .as_mut()
            .and_then(|p| p.read(&mut chunk).ok())
            .filter(|n| *n > 0)
        {
            let _ = into
                .lock()
                .map(|mut all| all.extend_from_slice(&chunk[..n]));
        }
        let _ = tx.send(());
    });
    Stream { read, ended }
}

impl Stream {
    /// What the stream held, waiting until `until` for it to end. A descendant of the child
    /// that kept the pipe open never ends it: what was read by then is what is returned.
    fn drained(self, until: Instant) -> Vec<u8> {
        let _ = self
            .ended
            .recv_timeout(until.saturating_duration_since(Instant::now()));
        self.read
            .lock()
            .map(|mut all| std::mem::take(&mut *all))
            .unwrap_or_default()
    }
}

/// Run `cmd` to its end and return what it wrote, or kill it once `limit` has passed.
///
/// A child that ends in time is returned whole, whatever its exit status: a refusal is an
/// answer. One that does not is killed, and the error names `what` and says it timed out. A
/// program that cannot be started is the error starting it gave.
///
/// ```text
/// use crate::integration::bounded::output_within;
/// use std::process::Command;
/// use std::time::Duration;
/// let mut said = Command::new("sh");
/// said.args(["-c", "printf answered; exit 3"]);
/// let out = output_within(&mut said, "sh", Duration::from_secs(30)).unwrap();
/// assert_eq!((out.status.code(), out.stdout.as_slice()), (Some(3), &b"answered"[..]));
///
/// let mut hung = Command::new("sleep");
/// hung.arg("30");
/// let err = output_within(&mut hung, "sleep 30", Duration::from_millis(100)).unwrap_err();
/// assert!(err.to_string().contains("sleep 30 timed out"), "{err}");
/// ```
pub fn output_within(cmd: &mut Command, what: &str, limit: Duration) -> std::io::Result<Output> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = reader(child.stdout.take());
    let stderr = reader(child.stderr.take());
    let deadline = Instant::now() + limit;
    // a child whose state cannot be read is treated as still running, and the limit ends it
    let ended = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            _ if Instant::now() >= deadline => break None,
            _ => std::thread::sleep(POLL),
        }
    };
    // one wait for both streams of a child that has ended
    let until = Instant::now() + DRAINED_WITHIN;
    ended
        .map(|status| Output {
            status,
            stdout: stdout.drained(until),
            stderr: stderr.drained(until),
        })
        .ok_or_else(|| {
            let _ = child.kill();
            let _ = child.wait();
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "{what} timed out after {} s and was killed",
                    limit.as_secs()
                ),
            )
        })
}

/// What a bounded call answered, as the callers that only want its words read it: what it
/// wrote when it succeeded, what it complained of when it did not, and the error when it
/// could not be run or did not end in time.
pub fn answered(out: std::io::Result<Output>) -> Result<String, String> {
    match out {
        Ok(out) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// The two bounds a `git` call that reaches the remote is given, as methods beside
/// `Command::output`, so a call site changes one word.
pub trait Bounded {
    /// [`output_within`] the limit of a read of the remote ([`FORGE_READ`]).
    fn output_within_read(&mut self, what: &str) -> std::io::Result<Output>;
    /// [`output_within`] the limit of a transfer of objects ([`GIT_TRANSFER`]).
    fn output_within_transfer(&mut self, what: &str) -> std::io::Result<Output>;
}

impl Bounded for Command {
    fn output_within_read(&mut self, what: &str) -> std::io::Result<Output> {
        output_within(self, what, FORGE_READ)
    }
    fn output_within_transfer(&mut self, what: &str) -> std::io::Result<Output> {
        output_within(self, what, GIT_TRANSFER)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    }

    #[test]
    fn a_call_that_ends_in_time_is_returned_whole_whatever_it_answered() {
        let out = output_within(
            &mut sh("printf out; printf err >&2; exit 3"),
            "sh",
            Duration::from_secs(30),
        )
        .unwrap();
        assert_eq!(out.status.code(), Some(3));
        assert_eq!(
            (out.stdout.as_slice(), out.stderr.as_slice()),
            (&b"out"[..], &b"err"[..])
        );
        // more than a pipe holds, on both streams: neither waits on the other
        let out = output_within(
            &mut sh("head -c 300000 /dev/zero; head -c 300000 /dev/zero >&2"),
            "sh",
            Duration::from_secs(30),
        )
        .unwrap();
        assert_eq!((out.stdout.len(), out.stderr.len()), (300_000, 300_000));
    }

    #[test]
    fn a_call_that_hangs_is_killed_and_says_it_timed_out() {
        let began = Instant::now();
        let err = output_within(
            &mut sh("sleep 30"),
            "gh pr merge 7",
            Duration::from_millis(200),
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert_eq!(
            err.to_string(),
            "gh pr merge 7 timed out after 0 s and was killed"
        );
        // an outage, to the classifier that decides whether a call is asked again
        assert!(crate::integration::retry::transient(&err.to_string()));
        assert!(
            began.elapsed() < Duration::from_secs(10),
            "{:?}",
            began.elapsed()
        );
    }

    #[test]
    fn a_child_that_left_something_holding_its_pipe_does_not_hold_the_call() {
        // the shell ends at once and its background child keeps the pipe open
        let began = Instant::now();
        let out = output_within(
            &mut sh("sleep 30 & printf said"),
            "sh",
            Duration::from_secs(30),
        )
        .unwrap();
        assert!(out.status.success());
        // and what it wrote before it ended is not lost with the stream that never ends
        assert_eq!(out.stdout, b"said");
        assert!(
            began.elapsed() < Duration::from_secs(15),
            "{:?}",
            began.elapsed()
        );
    }

    #[test]
    fn an_answer_is_its_words_its_complaint_or_why_it_could_not_be_had() {
        let within =
            |script: &str, limit: Duration| output_within(&mut sh(script), "the call", limit);
        assert_eq!(
            answered(within("printf ' said '", Duration::from_secs(30))),
            Ok("said".into())
        );
        assert_eq!(
            answered(within(
                "printf refused >&2; exit 1",
                Duration::from_secs(30)
            )),
            Err("refused".into())
        );
        assert_eq!(
            answered(within("sleep 30", Duration::from_millis(100))),
            Err("the call timed out after 0 s and was killed".into())
        );
    }

    #[test]
    fn a_program_that_cannot_be_started_is_that_error_and_not_a_timeout() {
        let err = output_within(
            &mut Command::new("/nonexistent/majordomus-no-such-program"),
            "nothing",
            Duration::from_secs(30),
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(!err.to_string().contains("timed out"), "{err}");
    }
}
