//! Bounded outside contact: the only two things the environment resolver does that it
//! cannot finish on its own — run a program, and open a socket — each with a hard budget.
//!
//! Both exist because a snapshot runs on a shell prompt. A toolchain shim that fetches a
//! release on first use, a port held by a process that is wedged rather than dead: either
//! would turn `cd` into a hang. Nothing here waits longer than it was given, and a probe
//! that ran out of time reports `unknown` rather than a verdict it did not earn.
//!
//! What is deliberately absent: name resolution. A host that is not an IP literal is not
//! probed at all, so no snapshot can reach DNS, and therefore no snapshot can reach the
//! network. The lease a Majordomus server publishes always names a loopback address.

use std::net::{IpAddr, SocketAddr, TcpStream};
use std::process::{Command, Output};
use std::time::Duration;

use super::ServiceAvailability;

/// Run a command and take its output, or give up after `budget`.
///
/// It is the bounded run every runner shares (`crate::process::run_bounded`): the pipes are
/// drained while the caller's clock runs, so a child that writes more than a pipe buffer is
/// not held on its write; the child leads a process group of its own, and on expiry the
/// group is killed — the child and whatever it started, which would otherwise keep the
/// pipes open — and `None` comes back; a caller that cannot say what it wanted must say
/// that, not guess. A child that cannot be started is `None` too.
///
/// A program written a moment ago can still be open for writing in a child another thread
/// forked meanwhile, and exec then fails with ETXTBSY; it is started through
/// `crate::process::retrying_busy`, as every other freshly written program is, rather than
/// reported as absent (#2501's coverage run: a stand-in sccache "does not answer --version").
pub fn bounded_output(command: &mut Command, budget: Duration) -> Option<Output> {
    let run = crate::process::run_bounded(command, budget, Duration::from_millis(10)).ok()?;
    Some(Output {
        status: run.status?,
        stdout: run.stdout,
        stderr: run.stderr,
    })
}

/// Does something accept a connection at `url` within `budget`?
///
/// Only the authority of the URL is used, and only when its host is an IP literal:
/// resolving a name would be a network call, and this runs on a shell prompt. A refused
/// connection is [`ServiceAvailability::NotRunning`] — the port is free, so nothing is
/// there. Anything else, expiry included, is [`ServiceAvailability::Unknown`], because a
/// probe that did not finish has not learnt that a service is down.
///
/// ```
/// use majordomus_cli::environment::probe::authority;
/// assert_eq!(authority("http://127.0.0.1:8741"), Some(("127.0.0.1".to_string(), 8741)));
/// assert_eq!(authority("http://127.0.0.1:8741/docs"), Some(("127.0.0.1".to_string(), 8741)));
/// assert_eq!(authority("http://example.test:80"), Some(("example.test".to_string(), 80)));
/// assert_eq!(authority("not a url"), None);
/// ```
pub fn reachable(url: &str, budget: Duration) -> ServiceAvailability {
    let Some((host, port)) = authority(url) else {
        return ServiceAvailability::Unknown;
    };
    let Ok(ip) = host.parse::<IpAddr>() else {
        // A name would need DNS. Nothing here resolves names.
        return ServiceAvailability::Unknown;
    };
    match TcpStream::connect_timeout(&SocketAddr::new(ip, port), budget) {
        Ok(_) => ServiceAvailability::Available,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            ServiceAvailability::NotRunning
        }
        Err(_) => ServiceAvailability::Unknown,
    }
}

/// The host and port of an `http://host:port[/path]` URL.
pub fn authority(url: &str) -> Option<(String, u16)> {
    let rest = url
        .strip_prefix("http://")
        .or(url.strip_prefix("https://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let (host, port) = authority.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    (!host.is_empty()).then(|| (host.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program still open for writing when it is asked to run is waited for, not read as
    /// absent: the writer here lets go after a moment, as a forked child does at its exec.
    #[cfg(unix)]
    #[test]
    fn a_program_still_being_written_is_run_once_it_is_let_go() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("probe-me");
        let mut held = std::fs::File::create(&program).unwrap();
        held.write_all(b"#!/bin/sh\necho answered\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(held);
        });
        let out = bounded_output(&mut Command::new(&program), Duration::from_secs(10))
            .expect("the program runs once nothing holds it open for writing");
        writer.join().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "answered\n");
    }

    #[test]
    fn a_command_that_finishes_gives_its_output() {
        let out = bounded_output(Command::new("printf").arg("hello"), Duration::from_secs(5))
            .expect("printf answers");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "hello");
    }

    #[test]
    fn a_command_that_does_not_finish_is_abandoned_within_its_budget() {
        let started = std::time::Instant::now();
        let out = bounded_output(Command::new("sleep").arg("30"), Duration::from_millis(150));
        assert!(out.is_none(), "a command over its budget yields nothing");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "it returned after {:?}, so the budget did not bound it",
            started.elapsed()
        );
    }

    /// A command over its budget is ended with what it started: a program it left running
    /// with the output pipe does not outlive the probe.
    #[cfg(unix)]
    #[test]
    fn a_command_over_its_budget_ends_with_what_it_started() {
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("pids");
        let out = bounded_output(
            Command::new("sh")
                .arg("-c")
                .arg("sleep 30 & echo $! > \"$1\"; wait")
                .arg("sh")
                .arg(&pids),
            Duration::from_millis(500),
        );
        assert!(out.is_none(), "a command over its budget yields nothing");
        let sleeper: u32 = std::fs::read_to_string(&pids)
            .expect("the sleeper started")
            .trim()
            .parse()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while crate::lease::alive(sleeper) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(
            !crate::lease::alive(sleeper),
            "the sleeper {sleeper} outlived the probe"
        );
    }

    #[test]
    fn a_command_that_does_not_exist_is_not_a_panic() {
        assert!(bounded_output(
            &mut Command::new("majordomus-no-such-program"),
            Duration::from_secs(1)
        )
        .is_none());
    }

    #[test]
    fn a_port_nothing_listens_on_is_not_running_rather_than_unknown() {
        // Bind and drop, so the port is one the kernel just had free. Between the drop and
        // the probe another process on a busy machine can take that very port, and then the
        // probe is right and the assertion is wrong — which is a race in the test, not a
        // defect in `reachable`. So the attempt is repeated: one clean answer proves the
        // property, and every attempt racing is what a failure would have to mean.
        let mut last = None;
        for _ in 0..8 {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
            let port = listener.local_addr().expect("an address").port();
            drop(listener);
            let url = format!("http://127.0.0.1:{port}");
            let answer = reachable(&url, Duration::from_millis(200));
            if answer == ServiceAvailability::NotRunning {
                return;
            }
            last = Some(answer);
        }
        panic!("every attempt found something listening on a port just freed: {last:?}");
    }

    #[test]
    fn a_port_something_listens_on_is_available() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let url = format!(
            "http://127.0.0.1:{}",
            listener.local_addr().expect("an address").port()
        );
        assert_eq!(
            reachable(&url, Duration::from_millis(500)),
            ServiceAvailability::Available
        );
    }

    /// The offline guarantee, as an assertion: a host that is not an IP literal is never
    /// looked up, so the resolver cannot reach a name server from a shell prompt.
    #[test]
    fn a_host_that_is_not_an_ip_literal_is_never_resolved() {
        let started = std::time::Instant::now();
        assert_eq!(
            reachable("http://majordomus.invalid:80", Duration::from_secs(30)),
            ServiceAvailability::Unknown
        );
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "it took {:?}, which is long enough to have asked a resolver",
            started.elapsed()
        );
    }

    #[test]
    fn a_url_without_a_port_has_no_authority_to_probe() {
        assert_eq!(authority("http://127.0.0.1"), None);
        assert_eq!(authority("majordomus://repository"), None);
    }
}
