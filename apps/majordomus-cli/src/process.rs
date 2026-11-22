//! Starting the programs this process runs, and ending them with everything they started.
//!
//! Linux refuses to execute a file that any process holds open for writing (ETXTBSY). A
//! program this process has just written is held so by every child forked while the write
//! was open, until that child execs: a test that writes a stand-in tool and runs it at once
//! falls into that window whenever another test thread forks at the same moment (CI run
//! 39049547899). Writing a copy and renaming it into place does not close the window, since
//! the inherited descriptor holds the inode; waiting does, because the holder lets go as
//! soon as it execs.
//!
//! A program run under a time bound is the leader of a process group of its own
//! ([`spawn_group`]), and the bound ends the group, not the program alone ([`wait_group`]).
//! Killing only the child left what it had started behind: a `git fetch` killed at its bound
//! left its `git-remote-https` running with no parent, and that orphan, still holding the
//! output pipe, held the caller's reader too, so the bound did not bound the call either.
//! The groups this process leads are also ended when it dies of a signal
//! ([`kill_registered_groups`]).
//!
//! The identity of this boot ([`boot_id`]) and whether a pid is alive
//! ([`crate::lease::alive`]) tell a lock file whose owner is gone from one that is held.

use std::io;
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

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

/// Start `cmd` as the leader of a process group of its own, recorded until the returned
/// [`Group`] is dropped so that a signal that ends this process ends the group too. A program
/// whose file is still open for writing somewhere is waited for rather than refused
/// ([`retrying_busy`]).
///
/// The price of a group of its own is that a Ctrl-C at the terminal no longer reaches the
/// child; [`kill_registered_groups`] is what pays it back.
pub(crate) fn spawn_group(cmd: &mut Command) -> io::Result<(Child, Group)> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = retrying_busy(|| cmd.spawn())?;
    let group = groups::register(child.id());
    Ok((child, group))
}

/// End the group `child` leads — the child and everything it started — and reap the child.
///
/// Called on a child [`spawn_group`] started. The group's id is the leader's pid, and the
/// kernel does not hand that id to a new process while any member of the group lives, so the
/// signal reaches this group or nobody, even once the leader is reaped.
pub(crate) fn kill_group(child: &mut Child) {
    #[cfg(unix)]
    if let Ok(pgid) = libc::pid_t::try_from(child.id()) {
        // SAFETY: killpg(2) on a group this process created; an id with no group left answers
        // ESRCH, which is ignored.
        unsafe { libc::killpg(pgid, libc::SIGKILL) };
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Wait for `child`, started by [`spawn_group`], at most `limit`, asking every `poll`. On
/// expiry its whole group is ended and the child reaped ([`kill_group`]), and `Ok(None)`
/// says it never finished; an error is the wait's own.
pub(crate) fn wait_group(
    child: &mut Child,
    limit: Duration,
    poll: Duration,
) -> io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            kill_group(child);
            return Ok(None);
        }
        std::thread::sleep(poll);
    }
}

/// End every group this process leads and still holds a [`Group`] for. Async-signal-safe:
/// what a signal handler that ends the process calls before it goes.
pub(crate) fn kill_registered_groups() {
    groups::kill_all();
}

/// A group [`spawn_group`] started, recorded for [`kill_registered_groups`]; dropping it
/// forgets the group.
pub(crate) use groups::Group;

/// The process groups this process started and has not let go of, ended with the process
/// when it dies of `SIGINT`, `SIGTERM` or `SIGHUP`.
///
/// The handler is installed for a signal only while that signal's disposition is still the
/// default: a server's ordered stop ([`crate::lease`]) installs handlers of its own and ends
/// the groups itself, and a signal a caller ignores (`nohup`) stays ignored. The handler does
/// only what is safe inside a signal handler: `killpg` each recorded group, restore the
/// default disposition, and raise the signal again so that the exit status still says which
/// signal it was.
#[cfg(unix)]
mod groups {
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Once;

    const SLOTS: usize = 64;
    static LIVE: [AtomicI32; SLOTS] = [const { AtomicI32::new(0) }; SLOTS];
    static INSTALL: Once = Once::new();

    /// A recorded group; dropping it forgets the group.
    #[derive(Debug)]
    pub struct Group(Option<usize>);

    /// Record `pgid` until the returned value is dropped. More groups at once than there
    /// are slots is not an error: the surplus is simply not ended on a signal.
    pub fn register(pgid: u32) -> Group {
        INSTALL.call_once(|| {
            for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
                install_if_default(signal);
            }
        });
        let Ok(pgid) = i32::try_from(pgid) else {
            return Group(None);
        };
        for (i, slot) in LIVE.iter().enumerate() {
            if slot
                .compare_exchange(0, pgid, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Group(Some(i));
            }
        }
        Group(None)
    }

    /// Take `signal` only when nobody else has: its disposition is still the default.
    fn install_if_default(signal: libc::c_int) {
        if disposition(signal) == Some(libc::SIG_DFL) {
            let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            // SAFETY: installing a handler that only calls async-signal-safe functions
            unsafe { libc::signal(signal, handler) };
        }
    }

    /// The current disposition of `signal`, read without changing it.
    pub fn disposition(signal: libc::c_int) -> Option<libc::sighandler_t> {
        // SAFETY: sigaction(2) with no new action only reads the current one into `old`,
        // a zeroed sigaction owned by this frame.
        unsafe {
            let mut old: libc::sigaction = std::mem::zeroed();
            (libc::sigaction(signal, std::ptr::null(), &mut old) == 0).then_some(old.sa_sigaction)
        }
    }

    impl Drop for Group {
        fn drop(&mut self) {
            if let Some(i) = self.0 {
                LIVE[i].store(0, Ordering::SeqCst);
            }
        }
    }

    /// `killpg` every recorded group: loads and a system call, async-signal-safe.
    pub fn kill_all() {
        for slot in &LIVE {
            let pgid = slot.load(Ordering::SeqCst);
            if pgid > 0 {
                // SAFETY: killpg is async-signal-safe; the group is one this process started
                unsafe { libc::killpg(pgid, libc::SIGKILL) };
            }
        }
    }

    extern "C" fn on_signal(signal: libc::c_int) {
        kill_all();
        // SAFETY: restoring the default disposition and re-raising are async-signal-safe
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
            libc::raise(signal);
        }
    }

    /// Whether `pgid` is recorded now.
    #[cfg(test)]
    pub fn recorded(pgid: u32) -> bool {
        i32::try_from(pgid).is_ok_and(|p| LIVE.iter().any(|s| s.load(Ordering::SeqCst) == p))
    }
}

#[cfg(not(unix))]
mod groups {
    #[derive(Debug)]
    pub struct Group;

    pub fn register(_: u32) -> Group {
        Group
    }

    pub fn kill_all() {}
}

/// This boot of this machine, as the system names it: Linux's `boot_id`, the time macOS
/// booted at, `None` where neither is there. A lock written under another boot was written
/// by a process that cannot be running now, whatever its pid has since come to name.
pub(crate) fn boot_id() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
    #[cfg(target_os = "macos")]
    {
        let mut tv = libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        let mut len = std::mem::size_of::<libc::timeval>();
        // SAFETY: the name is a NUL-terminated literal; `tv` is a timeval of the size passed
        // in `len`, which is what kern.boottime answers with; no new value is set.
        let rc = unsafe {
            libc::sysctlbyname(
                c"kern.boottime".as_ptr(),
                (&mut tv as *mut libc::timeval).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        (rc == 0 && tv.tv_sec > 0).then(|| format!("{}.{:06}", tv.tv_sec, tv.tv_usec))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

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

    /// A program that starts a program of its own, which records its pid in `pids` and then
    /// waits: the shape of `git fetch` and its `git-remote-https`.
    #[cfg(unix)]
    fn parent_of_a_sleeper(pids: &std::path::Path) -> Command {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("sleep 30 & echo $! > \"$1\"; wait")
            .arg("sh")
            .arg(pids)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        cmd
    }

    /// The pid `parent_of_a_sleeper` wrote, once it is there.
    #[cfg(unix)]
    fn sleeper(pids: &std::path::Path) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(pid) = std::fs::read_to_string(pids)
                .ok()
                .and_then(|s| s.trim().parse().ok())
            {
                return pid;
            }
            assert!(Instant::now() < deadline, "the sleeper never started");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Whether `pid` is gone within a few seconds: a killed orphan is reaped by init, and
    /// until then it still answers signal 0.
    #[cfg(unix)]
    fn gone_soon(pid: u32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if !crate::lease::alive(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }

    /// The bound ends the whole group: the program and the program it started are gone, the
    /// child is reaped, and the pipe the grandchild held reaches its end at once — the caller
    /// that reads it is not held for the grandchild's thirty seconds.
    #[cfg(unix)]
    #[test]
    fn a_bound_ends_the_program_and_everything_it_started() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("pids");
        let (mut child, group) = spawn_group(&mut parent_of_a_sleeper(&pids)).unwrap();
        assert!(
            groups::recorded(child.id()),
            "the group is recorded while held"
        );
        let grandchild = sleeper(&pids);
        let started = Instant::now();
        let waited = wait_group(
            &mut child,
            Duration::from_millis(200),
            Duration::from_millis(10),
        )
        .unwrap();
        assert_eq!(waited, None, "the program did not finish within its bound");
        let mut out = Vec::new();
        child.stdout.take().unwrap().read_to_end(&mut out).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the reader was held by an orphan: {:?}",
            started.elapsed()
        );
        assert!(
            gone_soon(grandchild),
            "the grandchild {grandchild} survived its group"
        );
        let pgid = child.id();
        drop(group);
        assert!(!groups::recorded(pgid), "a dropped group is forgotten");
    }

    /// A program that ends within its bound is answered with its status, and nothing is
    /// killed.
    #[cfg(unix)]
    #[test]
    fn a_program_within_its_bound_is_answered_with_its_status() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "exit 3"]);
        let (mut child, _group) = spawn_group(&mut cmd).unwrap();
        let status = wait_group(
            &mut child,
            Duration::from_secs(10),
            Duration::from_millis(10),
        )
        .unwrap()
        .expect("it finished");
        assert_eq!(status.code(), Some(3));
    }

    /// Killing a group ends a program that never finishes and the program it started.
    #[cfg(unix)]
    #[test]
    fn killing_a_group_ends_a_program_that_never_finishes() {
        use std::os::unix::process::ExitStatusExt;
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("pids");
        let (mut child, _group) = spawn_group(&mut parent_of_a_sleeper(&pids)).unwrap();
        let grandchild = sleeper(&pids);
        // only this test's own group: the slots are the process's, and a concurrent test's
        // groups must not be ended by this one
        kill_group(&mut child);
        assert_eq!(child.wait().unwrap().signal(), Some(libc::SIGKILL));
        assert!(
            gone_soon(grandchild),
            "the grandchild {grandchild} survived"
        );
    }

    /// A program that cannot start is the start's error, and no group is recorded.
    #[test]
    fn a_program_that_cannot_start_records_no_group() {
        let e = spawn_group(&mut Command::new("/nonexistent/majordomus-no-such-tool"))
            .expect_err("an absent program does not start");
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
    }

    /// The boot is named, and named the same way twice.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn this_boot_has_one_name() {
        let boot = boot_id().expect("the system names its boot");
        assert!(!boot.is_empty());
        assert_eq!(boot_id().as_deref(), Some(boot.as_str()));
    }

    /// The variable that makes a run of this test binary the child process below.
    #[cfg(unix)]
    const CHILD: &str = "MAJORDOMUS_PROCESS_TEST_CHILD";

    /// A process that dies of `SIGTERM` ends the groups it leads before it goes, and still
    /// dies of the signal; a signal it was told to ignore stays ignored, because the handler is
    /// installed only over a default disposition. In a process of its own, because a signal
    /// and the slots are the process's.
    #[cfg(unix)]
    #[test]
    fn a_process_ended_by_a_signal_ends_its_groups_first() {
        use std::os::unix::process::ExitStatusExt;
        if let Some(dir) = std::env::var_os(CHILD) {
            // SAFETY: setting a standard disposition before any thread of this test waits on it
            unsafe { libc::signal(libc::SIGHUP, libc::SIG_IGN) };
            let pids = std::path::Path::new(&dir).join("pids");
            let (_child, _group) = spawn_group(&mut parent_of_a_sleeper(&pids)).unwrap();
            sleeper(&pids);
            assert_eq!(groups::disposition(libc::SIGHUP), Some(libc::SIG_IGN));
            assert_ne!(groups::disposition(libc::SIGTERM), Some(libc::SIG_DFL));
            // SAFETY: raising a signal in this process, whose handler is the one under test
            unsafe { libc::raise(libc::SIGTERM) };
            unreachable!("the signal ends the process");
        }
        let dir = tempfile::tempdir().unwrap();
        let out = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process::tests::a_process_ended_by_a_signal_ends_its_groups_first",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(CHILD, dir.path())
            .output()
            .expect("the child process runs");
        assert_eq!(out.status.signal(), Some(libc::SIGTERM), "{out:?}");
        let grandchild = sleeper(&dir.path().join("pids"));
        assert!(
            gone_soon(grandchild),
            "the grandchild {grandchild} outlived the process"
        );
    }
}
