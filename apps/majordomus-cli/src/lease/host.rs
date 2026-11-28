//! The host's half of the lease: where the servers of this user on this machine are, found
//! by a reader that runs no process of its own (I3184; ADR 0199, amendment of 2026-11-24:
//! "the host's context inventory is a process inventory built from leases").
//!
//! A lease is a file of one checkout, and nothing said which checkouts a host has. Two
//! sources answer that here, and neither is a registry:
//!
//! - **The index**, `checkout-roots` in the user's state directory beside `node.json`
//!   ([`crate::mesh::identity::default_state_dir`]): the root of every checkout whose server
//!   published a lease, appended by that server ([`remember`], from
//!   [`crate::lease::Lease::publish`]). It is a cache of roots and nothing else. No row is
//!   believed: the reader re-reads every root's lease, so a row whose server is gone is
//!   listed as its lease says (stale, or not running), and deleting the file loses nothing
//!   but speed.
//! - **The process table**: every `majordomus serve` and `majordomus mcp` of this machine,
//!   identified by executable path and subcommand the way `scripts/reap-orphans` identifies
//!   them, never by a bare name match ([`read_process_table`]). It finds the servers the
//!   index cannot: those of an executable older than the index (0.29.0 and earlier), and
//!   those whose state directory is not the reader's.
//!
//! Nothing here starts a server, stops one, takes a lease or holds a peer: peers stay
//! `mesh/registry.rs`'s alone. The inventory itself is composed in
//! [`crate::capability::builtin::server::host_runtimes`].
//!
//! ```
//! use majordomus_cli::lease::host::{read_index, remember_at};
//! let state = tempfile::tempdir().unwrap();
//! let index = state.path().join("checkout-roots");
//! let checkout = tempfile::tempdir().unwrap();
//! assert!(read_index(&index).unwrap().is_empty(), "no index is an empty cache, not an error");
//! assert!(remember_at(&index, checkout.path()).unwrap());
//! assert!(!remember_at(&index, checkout.path()).unwrap(), "a root already listed is not written twice");
//! assert_eq!(read_index(&index).unwrap(), vec![checkout.path().to_path_buf()]);
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The index's file name, in the user's state directory beside `node.json`.
pub const INDEX_FILE: &str = "checkout-roots";

/// The first line of the index: what it is, for a person who opens it.
const INDEX_HEADER: &str = "# majordomus host index (I3184): a disposable cache of the \
checkout roots whose servers published a lease. Every row is re-read from that checkout's \
lease; deleting this file loses nothing but speed.";

/// How long an executable is given to say its version.
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(3);

/// Where this user's index lives on this machine; `None` when there is no state directory
/// (neither `XDG_STATE_HOME` nor `HOME` is set).
///
/// ```
/// use majordomus_cli::lease::host::{index_path, INDEX_FILE};
/// if let Some(path) = index_path() {
///     assert!(path.ends_with(INDEX_FILE));
///     assert_eq!(path.parent(), majordomus_cli::mesh::identity::default_state_dir().as_deref());
/// }
/// ```
pub fn index_path() -> Option<PathBuf> {
    crate::mesh::identity::default_state_dir().map(|dir| dir.join(INDEX_FILE))
}

/// The roots the index at `path` lists, in the order they were added, each once. An absent
/// file is an empty cache; a file that cannot be read is the reason, for the reader to say.
/// A line that is blank, a comment or not an absolute path is not a root.
///
/// ```
/// use majordomus_cli::lease::host::read_index;
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("checkout-roots");
/// std::fs::write(&path, "# header\n/a\n\nrelative\n/b\n/a\n").unwrap();
/// let roots = read_index(&path).unwrap();
/// assert_eq!(roots, vec![std::path::PathBuf::from("/a"), std::path::PathBuf::from("/b")]);
/// // a directory where the file should be cannot be read, and says so
/// assert!(read_index(dir.path()).unwrap_err().contains("cannot be read"));
/// ```
pub fn read_index(path: &Path) -> Result<Vec<PathBuf>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{} cannot be read ({e})", path.display())),
    };
    let mut roots: Vec<PathBuf> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let root = PathBuf::from(line);
        if root.is_absolute() && !roots.contains(&root) {
            roots.push(root);
        }
    }
    Ok(roots)
}

/// Add `root` to the index at `path`: `Ok(false)` when it is listed already, which writes
/// nothing. A new root is written with the roots whose directory still exists, through a
/// file of this process renamed into place, so that a reader never sees half an index and
/// the cache does not grow with checkouts that were deleted. Two servers adding at the same
/// moment can lose one of the two roots: that costs the reader nothing it cannot find in the
/// process table, which is why a cache may be this simple.
///
/// ```
/// use majordomus_cli::lease::host::{read_index, remember_at};
/// let state = tempfile::tempdir().unwrap();
/// let index = state.path().join("nested").join("checkout-roots");
/// let kept = tempfile::tempdir().unwrap();
/// let gone = tempfile::tempdir().unwrap();
/// let gone_path = gone.path().to_path_buf();
/// remember_at(&index, kept.path()).unwrap();
/// remember_at(&index, &gone_path).unwrap();
/// drop(gone);
/// let third = tempfile::tempdir().unwrap();
/// remember_at(&index, third.path()).unwrap();
/// assert_eq!(read_index(&index).unwrap(), vec![kept.path().to_path_buf(), third.path().to_path_buf()]);
/// ```
pub fn remember_at(path: &Path, root: &Path) -> io::Result<bool> {
    let known = read_index(path).unwrap_or_default();
    if known.iter().any(|r| r == root) {
        return Ok(false);
    }
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the index has no directory"))?;
    let mut text = format!("{INDEX_HEADER}\n");
    for kept in known
        .iter()
        .filter(|r| r.is_dir())
        .chain(std::iter::once(&root.to_path_buf()))
    {
        match kept.to_str() {
            Some(line) if !line.contains('\n') => {
                text.push_str(line);
                text.push('\n');
            }
            _ => {}
        }
    }
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{INDEX_FILE}.{}", std::process::id()));
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    Ok(true)
}

/// Add `root` to this user's index, as a server does once it has published its lease. Never
/// fails the caller: a server that cannot write the cache is still found by the process
/// table, so the reason goes to the log at debug level and nowhere else.
///
/// ```
/// use majordomus_cli::lease::host::{index_path, read_index, remember};
/// let state = tempfile::tempdir().unwrap();
/// let (one, two) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
/// // this doctest is its own process: the state directory is the temporary one
/// std::env::set_var("XDG_STATE_HOME", state.path());
/// let index = index_path().unwrap();
/// assert!(index.starts_with(state.path()));
/// remember(one.path());
/// remember(one.path());
/// remember(two.path());
/// // each root once, in the order the servers added themselves
/// assert_eq!(read_index(&index).unwrap(), vec![one.path(), two.path()]);
/// ```
pub fn remember(root: &Path) {
    remember_in(index_path(), root);
}

/// [`remember`] into the index at `path`, when there is one.
fn remember_in(path: Option<PathBuf>, root: &Path) {
    let Some(path) = path else {
        return;
    };
    if let Err(e) = remember_at(&path, root) {
        tracing::debug!(
            index = %path.display(),
            "the host index was not updated: {e}; the process table still finds this server"
        );
    }
}

/// Which Majordomus process a row of the process table is.
///
/// ```
/// use majordomus_cli::lease::host::Subcommand;
/// assert_eq!(serde_json::to_value(Subcommand::Serve).unwrap(), "serve");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Subcommand {
    /// `majordomus serve`: a server and nothing else.
    Serve,
    /// `majordomus mcp`: a client's session, which serves its checkout when it won the
    /// election and attaches to the server that did otherwise.
    Mcp,
}

/// A `majordomus serve` or `majordomus mcp` process of this machine, as the process table
/// shows it. Only a Majordomus executable running `serve`, or `mcp` serving, is one: any
/// other command line is not a row.
///
/// ```
/// use majordomus_cli::lease::host::{server_of, HostProcess, Subcommand};
/// use std::path::PathBuf;
/// let p: HostProcess =
///     server_of(42, &["/opt/bin/majordomus", "serve", "--repo", "/srv/one"]).unwrap();
/// assert_eq!((p.pid, p.subcommand), (42, Subcommand::Serve));
/// assert_eq!(p.repo, Some(PathBuf::from("/srv/one")));
/// assert!(p.cwd.is_none(), "the process table does not say where a process works");
/// assert!(server_of(43, &["/usr/bin/vim", "serve"]).is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProcess {
    /// The process id.
    pub pid: u32,
    /// Which of the two it is.
    pub subcommand: Subcommand,
    /// The executable, as the process table names it (the first word of its command line).
    pub executable: PathBuf,
    /// What its `--repo` names, when it was given one.
    pub repo: Option<PathBuf>,
    /// Its working directory, when the system says.
    pub cwd: Option<PathBuf>,
}

impl HostProcess {
    /// The directory the process works on: what `--repo` names (against its working
    /// directory when relative), else its working directory.
    ///
    /// ```
    /// use majordomus_cli::lease::host::{HostProcess, Subcommand};
    /// use std::path::{Path, PathBuf};
    /// let mut p = HostProcess { pid: 1, subcommand: Subcommand::Serve, executable: "/bin/majordomus".into(),
    ///     repo: Some("r".into()), cwd: Some("/home/u".into()) };
    /// assert_eq!(p.works_on(), Some(PathBuf::from("/home/u/r")));
    /// p.repo = Some("/abs".into());
    /// assert_eq!(p.works_on(), Some(PathBuf::from("/abs")));
    /// p.repo = None;
    /// assert_eq!(p.works_on().as_deref(), Some(Path::new("/home/u")));
    /// p.cwd = None;
    /// assert_eq!(p.works_on(), None);
    /// ```
    pub fn works_on(&self) -> Option<PathBuf> {
        match (&self.repo, &self.cwd) {
            (Some(repo), _) if repo.is_absolute() => Some(repo.clone()),
            (Some(repo), Some(cwd)) => Some(cwd.join(repo)),
            (Some(repo), None) => Some(repo.clone()),
            (None, cwd) => cwd.clone(),
        }
    }
}

/// The server a command line is, when it is one: its executable is a Majordomus executable
/// by its file name (`majordomus`, `majordomus-cli`, or the copy a server runs from,
/// `majordomus-<mtime>-<size>`), and its subcommand is `serve` or `mcp` serving — not
/// `serve status`, `serve ensure` or `serve stop`, which are short-lived readers, and not
/// `mcp --inspect` or a request for help, which serve nothing.
///
/// ```
/// use majordomus_cli::lease::host::{server_of, Subcommand};
/// let p = server_of(7, &["/opt/bin/majordomus", "serve", "--repo", "/r", "--idle", "900"]).unwrap();
/// assert_eq!((p.pid, p.subcommand), (7, Subcommand::Serve));
/// assert_eq!(p.repo.as_deref(), Some(std::path::Path::new("/r")));
/// assert!(server_of(7, &["/t/debug/served/majordomus-1700000000-42", "mcp", "--repo=/r"]).unwrap().repo.is_some());
/// assert!(server_of(7, &["majordomus", "serve", "status"]).is_none());
/// assert!(server_of(7, &["/opt/bin/majordomus", "mcp", "--inspect"]).is_none());
/// assert!(server_of(7, &["/usr/bin/vim", "serve"]).is_none(), "another program is not one");
/// assert!(server_of(7, &["/opt/bin/majordomus-env", "serve"]).is_none(), "nor is another Majordomus tool");
/// assert!(server_of(7, &["/opt/bin/majordomus", "plan", "status"]).is_none());
/// assert!(server_of(7, &["/opt/bin/majordomus"]).is_none());
/// ```
pub fn server_of(pid: u32, argv: &[&str]) -> Option<HostProcess> {
    let (executable, rest) = argv.split_first()?;
    // a word of a `&str` command line is UTF-8, so its file name reads without loss
    let name = Path::new(executable).file_name()?.to_string_lossy();
    if !is_majordomus_executable(&name) {
        return None;
    }
    let subcommand = match rest.first() {
        Some(&"serve") => Subcommand::Serve,
        Some(&"mcp") => Subcommand::Mcp,
        _ => return None,
    };
    let readers = [
        "status",
        "ensure",
        "stop",
        "--inspect",
        "--help",
        "-h",
        "--version",
        "-V",
    ];
    if rest[1..].iter().any(|a| readers.contains(a)) {
        return None;
    }
    let mut repo = None;
    let mut words = rest[1..].iter();
    while let Some(word) = words.next() {
        if *word == "--repo" {
            repo = words.next().map(PathBuf::from);
        } else if let Some(value) = word.strip_prefix("--repo=") {
            repo = Some(PathBuf::from(value));
        }
    }
    Some(HostProcess {
        pid,
        subcommand,
        executable: PathBuf::from(executable),
        repo,
        cwd: None,
    })
}

/// `majordomus`, `majordomus-cli`, and either followed by the `-<mtime>-<size>` of the copy
/// a server runs from (`served_executable`).
fn is_majordomus_executable(name: &str) -> bool {
    let rest = name
        .strip_prefix("majordomus-cli")
        .or_else(|| name.strip_prefix("majordomus"));
    let Some(rest) = rest else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    let mut parts = rest.split('-');
    parts.next() == Some("")
        && matches!(
            (parts.next(), parts.next(), parts.next()),
            (Some(a), Some(b), None)
                if !a.is_empty() && !b.is_empty()
                    && a.bytes().all(|c| c.is_ascii_digit())
                    && b.bytes().all(|c| c.is_ascii_digit())
        )
}

/// Every server [`server_of`] finds in `ps -A -o pid=,args=` output: one process a line,
/// its id and then its command line, split on white space (a path with a space in it is
/// read as two words, and such a process is not found here).
///
/// ```
/// use majordomus_cli::lease::host::parse_process_table;
/// let table = "  1 /sbin/launchd\n 42 /opt/bin/majordomus serve --repo /r\n 43 /opt/bin/majordomus serve status\nnot a row\n";
/// let found = parse_process_table(table);
/// assert_eq!(found.len(), 1);
/// assert_eq!(found[0].pid, 42);
/// ```
pub fn parse_process_table(text: &str) -> Vec<HostProcess> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let pid: u32 = words.next()?.parse().ok()?;
            let argv: Vec<&str> = words.collect();
            server_of(pid, &argv)
        })
        .collect()
}

/// Every `majordomus serve` and `majordomus mcp` of this machine, with its working
/// directory where the system says. `Err` is the reason the table could not be read, which
/// the inventory reports rather than calling the host empty.
///
/// ```
/// use majordomus_cli::lease::host::read_process_table;
/// // this test process is no server, so whatever is listed is somebody else's
/// let table = read_process_table().unwrap();
/// assert!(table.iter().all(|p| p.pid != std::process::id()));
/// ```
pub fn read_process_table() -> Result<Vec<HostProcess>, String> {
    read_process_table_with("ps")
}

/// [`read_process_table`], asking the `ps` at `program`.
fn read_process_table_with(program: &str) -> Result<Vec<HostProcess>, String> {
    let out = crate::process::retrying_busy(|| {
        Command::new(program)
            .env("LC_ALL", "C")
            .args(["-A", "-o", "pid=,args="])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
    })
    .map_err(|e| format!("the process table cannot be read: {program}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "the process table cannot be read: {program} exited {}",
            out.status
        ));
    }
    let mut processes = parse_process_table(&String::from_utf8_lossy(&out.stdout));
    let pids: Vec<u32> = processes.iter().map(|p| p.pid).collect();
    let cwds = working_directories(&pids);
    for p in &mut processes {
        p.cwd = cwds.get(&p.pid).cloned();
    }
    Ok(processes)
}

/// The working directory of each of `pids` the system will say: `/proc/<pid>/cwd` where
/// there is a `/proc`, else one `lsof` for all of them. A process the system says nothing
/// about is absent from the answer.
///
/// ```
/// use majordomus_cli::lease::host::working_directories;
/// let me = std::process::id();
/// let found = working_directories(&[me]);
/// if let Some(cwd) = found.get(&me) {
///     assert_eq!(cwd.canonicalize().unwrap(), std::env::current_dir().unwrap().canonicalize().unwrap());
/// }
/// assert!(working_directories(&[]).is_empty());
/// ```
pub fn working_directories(pids: &[u32]) -> BTreeMap<u32, PathBuf> {
    working_directories_from(pids, Path::new("/proc"), "lsof")
}

/// [`working_directories`], reading the `proc` file system mounted at `proc` when it has a
/// `self/cwd`, else asking the `lsof` at `lsof`: the seam that lets each of the two ways be
/// exercised on any system.
fn working_directories_from(pids: &[u32], proc: &Path, lsof: &str) -> BTreeMap<u32, PathBuf> {
    if pids.is_empty() {
        return BTreeMap::new();
    }
    if proc.join("self").join("cwd").exists() {
        return pids
            .iter()
            .filter_map(|pid| {
                fs::read_link(proc.join(pid.to_string()).join("cwd"))
                    .ok()
                    .map(|cwd| (*pid, cwd))
            })
            .collect();
    }
    let list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let Ok(out) = Command::new(lsof)
        .args(["-a", "-d", "cwd", "-Fn", "-p", &list])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return BTreeMap::new();
    };
    parse_lsof_cwds(&String::from_utf8_lossy(&out.stdout))
}

/// `lsof -Fn` output: a `p<pid>` line starts each process, an `n<path>` line names its file.
///
/// ```
/// use majordomus_cli::lease::host::parse_lsof_cwds;
/// let cwds = parse_lsof_cwds("p42\nfcwd\nn/r\np43\nfcwd\nn/s\nn/ignored\nq\n");
/// assert_eq!(cwds.get(&42).unwrap().to_str(), Some("/r"));
/// assert_eq!(cwds.get(&43).unwrap().to_str(), Some("/s"));
/// assert!(parse_lsof_cwds("n/orphan\n").is_empty(), "a name before any process belongs to none");
/// ```
pub fn parse_lsof_cwds(text: &str) -> BTreeMap<u32, PathBuf> {
    let mut cwds = BTreeMap::new();
    let mut current: Option<u32> = None;
    for line in text.lines() {
        if let Some(pid) = line.strip_prefix('p') {
            current = pid.parse().ok();
        } else if let (Some(name), Some(pid)) = (line.strip_prefix('n'), current) {
            cwds.entry(pid).or_insert_with(|| PathBuf::from(name));
        }
    }
    cwds
}

/// The version the executable at `exe` reports: the last word of the first line of
/// `<exe> --version`, given [`VERSION_TIMEOUT`]. `None` when it cannot be run, does not
/// answer in time, fails, or prints nothing. A read: the process asked is the executable
/// saying what it is and exiting, and one that does not exit in time is killed.
///
/// ```
/// use majordomus_cli::lease::host::executable_version;
/// use std::time::Duration;
/// assert_eq!(executable_version(std::path::Path::new("/nonexistent/majordomus"), Duration::from_secs(1)), None);
/// ```
pub fn executable_version(exe: &Path, timeout: Duration) -> Option<String> {
    let mut child = crate::process::retrying_busy(|| {
        Command::new(exe)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    })
    .ok()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    // output that is not UTF-8 is no version
    let text = child
        .stdout
        .take()
        .and_then(|stdout| io::read_to_string(stdout).ok())?;
    if !status.success() {
        return None;
    }
    text.lines()
        .next()?
        .split_whitespace()
        .last()
        .map(str::to_string)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A runtime that predates the index is listed with the version its executable reports,
    /// and an executable that fails, prints nothing or never answers gives no version
    /// rather than a guess.
    #[test]
    fn an_executable_says_its_version_or_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let old = script(dir.path(), "majordomus", "echo 'majordomus 0.29.0'");
        assert_eq!(
            executable_version(&old, VERSION_TIMEOUT).as_deref(),
            Some("0.29.0")
        );
        let failing = script(dir.path(), "failing", "echo 'majordomus 9'; exit 3");
        assert_eq!(executable_version(&failing, VERSION_TIMEOUT), None);
        let silent = script(dir.path(), "silent", "true");
        assert_eq!(executable_version(&silent, VERSION_TIMEOUT), None);
        let hung = script(dir.path(), "hung", "exec sleep 30");
        let t0 = Instant::now();
        assert_eq!(executable_version(&hung, Duration::from_millis(200)), None);
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "a hung executable is not waited on"
        );
    }

    /// A `ps` that cannot be run, or that fails, is a reason and never an empty host.
    #[test]
    fn a_process_table_that_cannot_be_read_says_why() {
        let missing = read_process_table_with("/nonexistent/ps").unwrap_err();
        assert!(missing.contains("cannot be read"), "{missing}");
        let dir = tempfile::tempdir().unwrap();
        let failing = script(dir.path(), "ps", "exit 1");
        let failed = read_process_table_with(failing.to_str().unwrap()).unwrap_err();
        assert!(failed.contains("exited"), "{failed}");
        let listing = script(
            dir.path(),
            "ps-listing",
            &format!(
                "echo '{} /opt/bin/majordomus serve --repo /r'",
                std::process::id()
            ),
        );
        let found = read_process_table_with(listing.to_str().unwrap()).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].cwd.is_some() || !Path::new("/proc/self/cwd").exists());
    }

    /// The copy a server runs from is a Majordomus executable; names that merely begin the
    /// same way are not.
    #[test]
    fn only_majordomus_executables_are_servers() {
        for name in [
            "majordomus",
            "majordomus-cli",
            "majordomus-1700000000-42",
            "majordomus-cli-1-2",
        ] {
            assert!(is_majordomus_executable(name), "{name}");
        }
        for name in [
            "majordomus-env",
            "majordomus-mcp",
            "majordomus-1",
            "majordomus-1-2-3",
            "majordomus-a-1",
            "majordomus--1",
            "vim",
        ] {
            assert!(!is_majordomus_executable(name), "{name}");
        }
    }

    /// A writer whose every clone appends to one buffer: what a test reads the log from.
    #[derive(Clone, Default)]
    struct Logged(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl io::Write for Logged {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A root that cannot be written into the index is a logged reason, never a failure of
    /// the server that tried; and an index whose directory cannot be made is an error from
    /// `remember_at`.
    #[test]
    fn an_index_that_cannot_be_written_fails_nobody() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        fs::write(&blocker, "").unwrap();
        assert!(remember_at(&blocker.join("checkout-roots"), dir.path()).is_err());
        assert!(
            remember_at(Path::new("/"), dir.path()).is_err(),
            "the root directory has no parent"
        );
        // and the writer a server calls never fails it: no state directory, or one that
        // cannot be written, is a line in the log at debug level. This test is the only one
        // of the library that reaches that line, so no other thread caches its interest
        // while the subscriber is being registered (see `crate::logging::with_scoped`).
        let logged = Logged::default();
        let writer = logged.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || writer.clone())
            .finish();
        crate::logging::with_scoped(subscriber, || {
            remember_in(None, dir.path());
            remember_in(Some(blocker.join("checkout-roots")), dir.path());
        });
        let log = String::from_utf8(logged.0.lock().unwrap().clone()).unwrap();
        assert!(log.contains("the host index was not updated"), "{log}");
        assert!(log.contains(&blocker.display().to_string()), "{log}");
        let index = dir.path().join("state").join(INDEX_FILE);
        remember_in(Some(index.clone()), dir.path());
        assert_eq!(read_index(&index).unwrap(), vec![dir.path().to_path_buf()]);
    }

    /// An index whose temporary file cannot be written, or cannot be renamed into place, is
    /// an error from `remember_at` that leaves no temporary file behind; a root no line can
    /// hold is not written, and the roots around it are.
    #[test]
    fn an_index_written_in_place_or_not_at_all() {
        let dir = tempfile::tempdir().unwrap();
        // a directory where the temporary file would go: nothing can be written
        let index = dir.path().join("blocked").join(INDEX_FILE);
        let tmp = dir
            .path()
            .join("blocked")
            .join(format!(".{INDEX_FILE}.{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        assert!(remember_at(&index, dir.path()).is_err());
        assert!(!index.exists());
        // a directory where the index stands: written, then not renamed, then removed
        let standing = dir.path().join("standing");
        fs::create_dir_all(standing.join("occupied")).unwrap();
        assert!(remember_at(&standing, dir.path()).is_err());
        let left: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(
            left.is_empty(),
            "a temporary file was left behind: {left:?}"
        );
        // a root with a line break in its name cannot be a line of the index
        let index = dir.path().join("state").join(INDEX_FILE);
        assert!(remember_at(&index, dir.path()).unwrap());
        assert!(remember_at(&index, Path::new("/a\nb")).unwrap());
        assert_eq!(read_index(&index).unwrap(), vec![dir.path().to_path_buf()]);
    }

    /// What a process works on, from its `--repo` and its working directory.
    #[test]
    fn a_process_works_on_its_repo_or_its_directory() {
        let mut p = HostProcess {
            pid: 1,
            subcommand: Subcommand::Mcp,
            executable: "/bin/majordomus".into(),
            repo: Some("r".into()),
            cwd: Some("/home/u".into()),
        };
        assert_eq!(p.works_on(), Some(PathBuf::from("/home/u/r")));
        p.cwd = None;
        assert_eq!(
            p.works_on(),
            Some(PathBuf::from("r")),
            "a relative --repo, no cwd"
        );
        p.repo = Some("/abs".into());
        assert_eq!(p.works_on(), Some(PathBuf::from("/abs")));
        p.repo = None;
        assert_eq!(p.works_on(), None);
        p.cwd = Some("/w".into());
        assert_eq!(p.works_on(), Some(PathBuf::from("/w")));
    }

    /// A command line is a server only when it is a Majordomus executable serving; its
    /// `--repo` is read in both spellings, the last one winning.
    #[test]
    fn a_command_line_is_a_server_or_not() {
        assert!(server_of(1, &[]).is_none(), "an empty command line");
        assert!(
            server_of(1, &["/"]).is_none(),
            "an executable with no file name"
        );
        assert!(server_of(1, &["/opt/bin/majordomus"]).is_none());
        assert!(server_of(1, &["/opt/bin/vim", "serve"]).is_none());
        assert!(server_of(1, &["/opt/bin/majordomus", "plan"]).is_none());
        for reader in [
            "status",
            "ensure",
            "stop",
            "--help",
            "-h",
            "--version",
            "-V",
        ] {
            assert!(
                server_of(1, &["/opt/bin/majordomus", "serve", reader]).is_none(),
                "{reader}"
            );
        }
        assert!(server_of(1, &["majordomus", "mcp", "--inspect"]).is_none());
        let p = server_of(7, &["/t/majordomus-1-2", "mcp", "--repo=/r"]).unwrap();
        assert_eq!(
            (p.pid, p.subcommand, p.repo.as_deref()),
            (7, Subcommand::Mcp, Some(Path::new("/r")))
        );
        let p = server_of(8, &["majordomus-cli", "serve", "--repo", "/a", "--repo=/b"]).unwrap();
        assert_eq!(p.repo.as_deref(), Some(Path::new("/b")));
        assert_eq!(p.executable, PathBuf::from("majordomus-cli"));
        let p = server_of(9, &["majordomus", "serve", "--repo"]).unwrap();
        assert!(p.repo.is_none(), "a --repo with no value names nothing");
    }

    /// A process table's rows: an id and a command line; anything else is not a row.
    #[test]
    fn a_process_table_has_rows_and_nothing_else() {
        let table = "\n  1 /sbin/launchd\n 42 /opt/bin/majordomus serve --repo /r\n\
                     43 /opt/bin/majordomus serve status\nnot a row\n-1 majordomus serve\n\
                     44 majordomus mcp\n";
        let found = parse_process_table(table);
        let pids: Vec<u32> = found.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![42, 44]);
        assert!(found.iter().all(|p| p.cwd.is_none()));
    }

    /// `lsof -Fn` lines: each name belongs to the process line before it, the first name
    /// of each process wins, and a name before any process, or after an unreadable one,
    /// belongs to none.
    #[test]
    fn lsof_names_belong_to_their_process() {
        let cwds = parse_lsof_cwds("n/orphan\np42\nfcwd\nn/r\nn/second\npx\nn/lost\np43\nn/s\n");
        assert_eq!(
            cwds,
            BTreeMap::from([(42, PathBuf::from("/r")), (43, PathBuf::from("/s"))])
        );
        assert!(parse_lsof_cwds("").is_empty());
    }

    /// Working directories come from a `proc` file system where there is one, else from
    /// one `lsof` for every process; an `lsof` that cannot be run says nothing, and no
    /// process asked is no question.
    #[test]
    fn working_directories_from_proc_or_from_lsof() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let none = dir.path().join("no-proc");
        assert!(working_directories_from(&[], &none, "/nonexistent/lsof").is_empty());
        // a proc file system: a self/cwd that resolves says there is one, and <pid>/cwd is
        // each answer, read as a link whether or not it resolves
        let proc = dir.path().join("proc");
        fs::create_dir_all(proc.join("self")).unwrap();
        fs::create_dir_all(proc.join("42")).unwrap();
        symlink(dir.path(), proc.join("self").join("cwd")).unwrap();
        symlink("/w/42", proc.join("42").join("cwd")).unwrap();
        assert_eq!(
            working_directories_from(&[42, 43], &proc, "/nonexistent/lsof"),
            BTreeMap::from([(42, PathBuf::from("/w/42"))])
        );
        // no proc: one lsof, asked for every process at once
        let args = dir.path().join("lsof-args");
        let lsof = script(
            dir.path(),
            "lsof",
            &format!(
                "echo \"$@\" > '{}'\nprintf 'p42\\nfcwd\\nn/l/42\\np43\\nfcwd\\nn/l/43\\n'",
                args.display()
            ),
        );
        assert_eq!(
            working_directories_from(&[42, 43], &none, lsof.to_str().unwrap()),
            BTreeMap::from([(42, PathBuf::from("/l/42")), (43, PathBuf::from("/l/43"))])
        );
        assert_eq!(
            fs::read_to_string(&args).unwrap().trim(),
            "-a -d cwd -Fn -p 42,43"
        );
        assert!(working_directories_from(&[42], &none, "/nonexistent/lsof").is_empty());
    }

    /// An executable that cannot be started, or whose answer is not text, gives no version.
    #[test]
    fn an_executable_that_cannot_answer_has_no_version() {
        assert_eq!(
            executable_version(Path::new("/nonexistent/majordomus"), VERSION_TIMEOUT),
            None
        );
        let dir = tempfile::tempdir().unwrap();
        let binary = script(dir.path(), "binary", "printf 'majordomus \\377\\n'");
        assert_eq!(executable_version(&binary, VERSION_TIMEOUT), None);
    }
}
