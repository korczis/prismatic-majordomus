//! How a command reaches a host: one POSIX `sh -c` script with positional arguments, run
//! either over ssh or, for a host that is this machine, directly.
//!
//! Every value travels as an argument, quoted here, and never as part of a script's text, so
//! a branch name or a path cannot become code on the host. The scripts are short constants
//! beside the decision they serve (`queue.rs`, `run.rs`); they observe and act, and every
//! decision is made in Rust over what they report.

use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// The environment variable naming the ssh program; a test points it at a local stand-in.
pub const SSH_PROGRAM_ENV: &str = "MAJORDOMUS_OFFLOAD_SSH";

/// What a script printed and how it ended.
#[derive(Debug, Clone)]
pub struct RemoteOutput {
    /// The exit status; `None` when a signal ended it.
    pub code: Option<i32>,
    /// Standard output, lossily decoded.
    pub stdout: String,
    /// Standard error, lossily decoded.
    pub stderr: String,
}

impl RemoteOutput {
    /// Whether the script exited 0.
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

/// A host commands can be run on.
#[derive(Debug, Clone)]
pub struct Remote {
    /// The program and arguments before the remote command: `ssh -o BatchMode=yes … user@host`.
    /// Empty for this machine.
    prefix: Vec<String>,
    /// What a report calls the host.
    pub label: String,
}

/// Quote one word for a POSIX shell: unchanged when it holds only characters no shell
/// interprets, otherwise single-quoted with every `'` closed, escaped and reopened.
pub fn quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:@%+=,".contains(c))
    {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// Expand a leading `~/` against the invoking user's home, which is what ssh would do.
fn expand_home(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => path.to_string(),
    }
}

impl Remote {
    /// This machine, as a host: the script runs under `sh` here. What the tests use as a
    /// host, with real processes standing for the waiters.
    #[cfg(test)]
    pub fn local() -> Remote {
        Remote {
            prefix: Vec::new(),
            label: "local".into(),
        }
    }

    /// A host reached through `program`, which is handed the remote command as one string
    /// the way ssh is: a test's stand-in that fails, vanishes or prints what a host would.
    #[cfg(test)]
    pub fn through(program: &Path) -> Remote {
        Remote {
            prefix: vec![program.to_string_lossy().into_owned()],
            label: "stand-in".into(),
        }
    }

    /// A host reached over ssh, without a terminal and without a password prompt. The ssh
    /// program is `$MAJORDOMUS_OFFLOAD_SSH` when set, so a test can stand a local shim in.
    pub fn ssh(address: &str, identity_file: Option<&str>, label: &str) -> Remote {
        let program = std::env::var(SSH_PROGRAM_ENV).unwrap_or_else(|_| "ssh".into());
        let mut prefix = vec![
            program,
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            "ConnectTimeout=8".into(),
            "-o".into(),
            "ServerAliveInterval=30".into(),
        ];
        if let Some(f) = identity_file {
            prefix.push("-i".into());
            prefix.push(expand_home(f));
        }
        prefix.push(address.to_string());
        Remote {
            prefix,
            label: label.to_string(),
        }
    }

    fn command(&self, script: &str, args: &[&str]) -> Command {
        if self.prefix.is_empty() {
            let mut c = Command::new("sh");
            c.arg("-c").arg(script).arg("sh").args(args);
            c
        } else {
            // ssh hands the remote login shell one string, whatever that shell is (zsh on a
            // Mac); `sh -c` pins the interpreter and every word is quoted for it.
            let mut words = vec!["sh".to_string(), "-c".into(), quote(script), "sh".into()];
            words.extend(args.iter().map(|a| quote(a)));
            let mut c = Command::new(&self.prefix[0]);
            c.args(&self.prefix[1..]).arg(words.join(" "));
            c
        }
    }

    /// Run a script to its end, feeding it a file on standard input when one is given.
    pub fn run(
        &self,
        script: &str,
        args: &[&str],
        stdin: Option<&Path>,
    ) -> std::io::Result<RemoteOutput> {
        let mut c = self.command(script, args);
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
        match stdin {
            Some(p) => c.stdin(Stdio::from(std::fs::File::open(p)?)),
            None => c.stdin(Stdio::null()),
        };
        let out = c.output()?;
        Ok(RemoteOutput {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    /// Run a script to its end with its standard input held open by this process until the
    /// script ends: a script that watches its input (the derive does) learns that this
    /// process is gone — killed, disconnected, crashed — when the input reaches its end, and
    /// stops what it started (I3956).
    pub fn run_held(&self, script: &str, args: &[&str]) -> std::io::Result<RemoteOutput> {
        let mut child = self
            .command(script, args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        // taken out of the child so that waiting does not close it: it is closed after
        let held = child.stdin.take();
        let out = child.wait_with_output();
        drop(held);
        let out = out?;
        Ok(RemoteOutput {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    /// Run a script with its standard output written to a local file, as a download.
    pub fn run_to_file(
        &self,
        script: &str,
        args: &[&str],
        out: &Path,
    ) -> std::io::Result<RemoteOutput> {
        let file = std::fs::File::create(out)?;
        let o = self
            .command(script, args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(file))
            .stderr(Stdio::piped())
            .output()?;
        Ok(RemoteOutput {
            code: o.status.code(),
            stdout: String::new(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        })
    }

    /// Start a script that runs for as long as its standard input stays open: the holder
    /// of a ticket. The caller reads what it prints and drops the child to end it.
    pub fn spawn(&self, script: &str, args: &[&str]) -> std::io::Result<Child> {
        self.command(script, args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
}

/// A process on a host that lives exactly as long as this one holds it: its pid is what a
/// ticket and the lock name, so a waiter that dies — killed, disconnected, crashed — leaves
/// a ticket every other waiter can see is dead.
pub struct Holder {
    child: Child,
    /// The holder's pid on the host.
    pub pid: u32,
}

/// The holder: print its own pid, then wait for end of input. `exec` keeps the pid the one
/// printed.
const HOLD: &str = r#"echo "$$"; exec cat >/dev/null"#;

impl Holder {
    /// Start a holder on the host and read its pid.
    pub fn start(remote: &Remote) -> std::io::Result<Holder> {
        use std::io::BufRead;
        let mut child = remote.spawn(HOLD, &[])?;
        let mut line = String::new();
        let stdout = child.stdout.take().expect("piped");
        std::io::BufReader::new(stdout).read_line(&mut line)?;
        match line.trim().parse() {
            Ok(pid) => Ok(Holder { child, pid }),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(std::io::Error::other(format!(
                    "the host's holder did not print its pid: {:?}",
                    line.trim()
                )))
            }
        }
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        if let Some(mut stdin) = self.child.stdin.take() {
            let _ = stdin.flush();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_reach_the_script_as_they_are() {
        let out = Remote::local()
            .run("printf '%s|%s' \"$1\" \"$2\"", &["a b", "c'd"], None)
            .unwrap();
        assert!(out.ok());
        assert_eq!(out.stdout, "a b|c'd");
    }

    #[test]
    fn a_word_is_quoted_only_when_a_shell_would_read_it() {
        for w in ["abc", "a/b.c", "user@host", "-o", "x=1,y"] {
            assert_eq!(quote(w), w);
        }
        for w in ["a b", "$HOME", "a;b", "`x`", "a'b", "*", ""] {
            let q = quote(w);
            let out = Remote::local()
                .run(&format!("printf %s {q}"), &[], None)
                .unwrap();
            assert_eq!(out.stdout, w, "{w} quoted as {q}");
        }
    }

    #[test]
    fn a_holder_lives_until_it_is_dropped() {
        let holder = Holder::start(&Remote::local()).unwrap();
        let pid = holder.pid.to_string();
        let alive = |p: &str| {
            Remote::local()
                .run("kill -0 \"$1\"", &[p], None)
                .unwrap()
                .ok()
        };
        assert!(alive(&pid), "the holder is not running");
        drop(holder);
        let mut gone = false;
        for _ in 0..50 {
            if !alive(&pid) {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(gone, "the holder outlived its owner");
    }

    fn alive(pid: u32) -> bool {
        Remote::local()
            .run("kill -0 \"$1\"", &[&pid.to_string()], None)
            .unwrap()
            .ok()
    }

    fn stand_in(dir: &Path, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("stand-in");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// The ssh prefix: batch mode, the identity file with `~/` expanded against the invoking
    /// user's home and any other path kept as written, then the address.
    #[test]
    fn an_ssh_host_is_reached_in_batch_mode_with_its_identity() {
        let home = std::env::var("HOME").unwrap();
        let r = Remote::ssh("u@lundra", Some("~/.ssh/cluster"), "lundra");
        assert_eq!(r.label, "lundra");
        let n = r.prefix.len();
        assert_eq!(
            r.prefix[n - 3..],
            [
                "-i".to_string(),
                format!("{home}/.ssh/cluster"),
                "u@lundra".into()
            ]
        );
        assert!(r.prefix.contains(&"BatchMode=yes".to_string()));
        if std::env::var(SSH_PROGRAM_ENV).is_err() {
            assert_eq!(r.prefix[0], "ssh", "the default program is ssh");
        }
        let r = Remote::ssh("u@h", Some("/etc/keys/k"), "h");
        assert_eq!(r.prefix[r.prefix.len() - 2], "/etc/keys/k");
        let r = Remote::ssh("u@h", None, "h");
        assert!(!r.prefix.contains(&"-i".to_string()));
        assert_eq!(expand_home("relative/k"), "relative/k");
    }

    /// The ssh form hands the program one string that `sh -c` reads back into the script
    /// and its arguments, each word quoted.
    #[test]
    fn a_remote_command_reaches_its_program_as_one_quoted_string() {
        let tmp = tempfile::tempdir().unwrap();
        let r = Remote::through(&stand_in(tmp.path(), r#"exec sh -c "$1""#));
        let out = r
            .run("printf '%s|%s' \"$1\" \"$2\"", &["a b", "it's"], None)
            .unwrap();
        assert_eq!((out.ok(), out.stdout.as_str()), (true, "a b|it's"));
    }

    /// What cannot run is an error, never an empty answer: a program that is not there, a
    /// standard input that cannot be opened, a download target that cannot be created.
    #[test]
    fn what_cannot_be_run_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let gone = Remote::through(&tmp.path().join("no-such-ssh"));
        let e = gone.run("true", &[], None).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
        let e = gone
            .run_to_file("true", &[], &tmp.path().join("out"))
            .unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
        assert!(Holder::start(&gone).is_err(), "a holder started on no host");

        let e = Remote::local()
            .run("cat", &[], Some(&tmp.path().join("no-such-input")))
            .unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
        let e = Remote::local()
            .run_to_file("echo x", &[], &tmp.path().join("no-dir/out"))
            .unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);

        let input = tmp.path().join("in");
        std::fs::write(&input, "fed\n").unwrap();
        let out = Remote::local().run("cat", &[], Some(&input)).unwrap();
        assert_eq!(out.stdout, "fed\n");
        let file = tmp.path().join("down");
        let out = Remote::local()
            .run_to_file("echo down; echo said >&2; exit 4", &[], &file)
            .unwrap();
        assert_eq!((out.code, out.stderr.as_str()), (Some(4), "said\n"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "down\n");
    }

    /// A holder that does not print its pid — a login banner, a broken connection, bytes that
    /// are not text — is refused and its process ended.
    #[test]
    fn a_holder_that_does_not_print_its_pid_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let banner = Remote::through(&stand_in(
            tmp.path(),
            "echo 'Welcome to lundra'; exec cat >/dev/null",
        ));
        let e = Holder::start(&banner).err().expect("a banner is not a pid");
        assert_eq!(
            e.to_string(),
            "the host's holder did not print its pid: \"Welcome to lundra\""
        );
        let binary = Remote::through(&stand_in(
            tmp.path(),
            r"printf '\377\376\n'; exec cat >/dev/null",
        ));
        let e = Holder::start(&binary).err().expect("bytes are not a pid");
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
    }

    /// A holder whose standard input was never piped still ends with its owner.
    #[test]
    fn a_holder_without_an_input_still_ends_when_dropped() {
        let child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        assert!(alive(pid));
        drop(Holder { child, pid });
        assert!(!alive(pid), "the holder outlived its owner");
    }
}
