//! A server runs from a copy of a build output, so that a rebuild never replaces the file a
//! live lease names (I2059).
//!
//! `cargo build` writes `target/debug/majordomus` in place. A shared server started from that
//! path is, from the next build on, serving code that is no longer on disk: since #747 its
//! lease is superseded and since I1502 it stops calling itself ready, so a diagnostic build in
//! the primary checkout took the owner's server — and the address it had been told to bind —
//! away from every session attached to it (2026-10-08, `.ai/local/state/mcp/server.log`).
//!
//! The process that is about to serve — `majordomus mcp`, or `majordomus serve` with no
//! subcommand — therefore re-executes itself from a copy when the file it was started from is
//! a cargo build output: `<profile dir>/served/majordomus-<mtime>-<size>`, named by the
//! identity of the build it copies — a second link to the build's file where the filesystem
//! allows one — made once per build, never replaced, and shared by every server started from it. A rebuild replaces `target/debug/majordomus` and leaves the copy alone; the
//! running server stays ready on its address until something restarts it on purpose
//! (`serve stop`, its idle timeout), and a launcher of the new build attaches to it rather
//! than superseding it, because a lease naming another path makes no claim
//! (`lease::superseded`).
//!
//! A build output is recognised by cargo's own mark, `CACHEDIR.TAG` in the directory above the
//! profile directory. An installed release (a versioned directory of its own), an executable
//! named through a directory without that mark, and a copy itself (whose parent is `served/`)
//! are run where they are. Copies older than a week are removed when a new one is made.
//! Every failure — a full disk, a read-only target directory — runs the server in place and
//! says so on stderr, which is the behaviour before this module existed.
//!
//! ```
//! use majordomus_cli::served_executable::copy_target;
//! use std::path::Path;
//! // a release install, or any directory cargo did not mark, is served where it is
//! assert_eq!(copy_target(Path::new("/opt/majordomus/libexec/majordomus-cli")), None);
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::cli::Command;

/// How long a copy no build names any more is kept: a server running from it longer than this
/// loses its file and says so the way a replaced one does.
const KEEP_FOR: Duration = Duration::from_secs(7 * 24 * 3600);

/// The directory, beside the build output, that holds the copies.
const SERVED_DIR: &str = "served";

/// Where the server started from `exe` should run from instead, when `exe` is a cargo build
/// output: `<dir of exe>/served/<name>-<mtime>-<size>`. `None` for anything else, and for an
/// executable that cannot be stat'ed.
///
/// ```
/// use majordomus_cli::served_executable::copy_target;
/// let dir = tempfile::tempdir().unwrap();
/// let debug = dir.path().join("debug");
/// std::fs::create_dir_all(&debug).unwrap();
/// std::fs::write(dir.path().join("CACHEDIR.TAG"), "Signature: 8a477f597d28d172789f06886806bc55\n").unwrap();
/// std::fs::write(debug.join("majordomus"), b"bytes").unwrap();
/// let copy = copy_target(&debug.join("majordomus")).expect("a build output is copied");
/// assert_eq!(copy.parent().unwrap(), debug.join("served"));
/// assert!(copy.file_name().unwrap().to_string_lossy().ends_with("-5"), "named by its size");
/// ```
pub fn copy_target(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?;
    if !dir.parent()?.join("CACHEDIR.TAG").is_file() {
        return None;
    }
    let meta = fs::metadata(exe).ok()?;
    // a time the platform cannot give and one before the epoch name no copy alike
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())?
        .as_secs();
    let name = exe.file_name()?.to_string_lossy();
    Some(
        dir.join(SERVED_DIR)
            .join(format!("{name}-{mtime}-{}", meta.len())),
    )
}

/// Make the copy at `target` of `exe`, unless it is already there, and remove the copies
/// beside it that are older than [`KEEP_FOR`].
///
/// The copy is a second name for the build's own file wherever the filesystem allows one: no
/// bytes are copied, which matters to a test run that starts a server from a fresh build in a
/// dozen processes at once. That is enough because a build never rewrites its output in
/// place — the linker writes a new file and cargo links it to `target/<profile>/<name>` anew —
/// so the second name keeps the bytes the server loaded. Where a link cannot be made the
/// bytes are copied under a name of its own and linked into place.
///
/// Either way nothing already at `target` is replaced: two servers starting from one build at
/// the same moment both run the file that got there first. A rename over it would have
/// replaced the first server's file under it, and that server would then be serving code no
/// longer on disk — the very thing the copy exists to prevent.
fn ensure_copy(exe: &Path, target: &Path, now: SystemTime) -> io::Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| io::Error::other("a copy target has a directory"))?;
    fs::create_dir_all(dir)?;
    let size = fs::metadata(exe)?.len();
    if fs::metadata(target).is_err() {
        fs::hard_link(exe, target).or_else(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                copy_into_place(exe, target, dir)
            }
        })?;
    }
    if fs::metadata(target)?.len() != size {
        return Err(io::Error::other(format!(
            "{} is not a whole copy of the build",
            target.display()
        )));
    }
    prune(dir, target, now);
    Ok(())
}

/// The bytes of `exe` at `target`, written under a name of this process's own in `dir` and
/// linked into place, so that a file already at `target` is kept rather than replaced.
fn copy_into_place(exe: &Path, target: &Path, dir: &Path) -> io::Result<()> {
    let partial = dir.join(format!(".partial-{}", std::process::id()));
    let made = fs::copy(exe, &partial).and_then(|_| match fs::hard_link(&partial, target) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        linked => linked,
    });
    let _ = fs::remove_file(&partial);
    made
}

/// When the file behind `meta` was given the name it has: its status-change time where there
/// is one, which a new link or copy sets and nothing after it moves back, and its modification
/// time elsewhere. A link keeps the build's modification time, which can be weeks old on the
/// day a server starts from it.
fn named_at(meta: &fs::Metadata) -> Option<SystemTime> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        u64::try_from(meta.ctime())
            .ok()
            .map(|secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }
    #[cfg(not(unix))]
    {
        meta.modified().ok()
    }
}

/// Where the copy at `path` was made from: `<profile dir>/<name>` for
/// `<profile dir>/served/<name>-<mtime>-<size>`, and `None` for a path that is not a copy. A
/// server started before copies existed names the build output itself, and a launcher running
/// a copy of that output still recognises it as its own when the build has replaced it.
pub(crate) fn copied_from(path: &Path) -> Option<PathBuf> {
    let dir = path.parent()?;
    if dir.file_name()? != SERVED_DIR {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    let (rest, size) = name.rsplit_once('-')?;
    let (stem, mtime) = rest.rsplit_once('-')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits(size) && digits(mtime) && !stem.is_empty())
        .then(|| dir.parent().map(|p| p.join(stem)))?
}

/// Remove the copies in `dir`, other than `keep`, that were made longer than [`KEEP_FOR`]
/// before `now`. Best effort: a copy that cannot be read or removed is left.
fn prune(dir: &Path, keep: &Path, now: SystemTime) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // another process's copy in the making is its own to remove
        if path == keep || entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let old = entry
            .metadata()
            .ok()
            .as_ref()
            .and_then(named_at)
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age > KEEP_FOR);
        if old {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Does `command` run a process that may take the lease: `mcp`, and `serve` with no
/// subcommand. `serve status`, `serve ensure` and `serve stop` are clients of one; `ensure`
/// starts its server as a `serve` process of its own, which then makes this decision.
fn may_hold_the_lease(command: &Command) -> bool {
    match command {
        Command::Mcp(_) => true,
        Command::Serve(args) => args.command.is_none(),
        _ => false,
    }
}

/// When `command` may hold the lease and this executable is a cargo build output, run this
/// process from a copy of it by replacing the process image (same pid, same standard streams,
/// same arguments). Returns only when it does not: another command, an executable that is not
/// a build output, or a copy that could not be made or run — the last two say why on stderr,
/// and the caller then serves in place.
///
/// Called by `main` once the command line is parsed, before anything is served.
///
/// ```no_run
/// use clap::Parser;
/// use majordomus_cli::cli::Cli;
/// use majordomus_cli::served_executable::serve_from_a_copy;
/// let cli = Cli::try_parse_from(["majordomus", "serve"]).unwrap();
/// // from target/debug/majordomus this does not return: the process continues as the copy
/// serve_from_a_copy(&cli.command);
/// ```
pub fn serve_from_a_copy(command: &Command) {
    if !may_hold_the_lease(command) {
        return;
    }
    // an executable that cannot name itself names no build output, so it has no copy
    let exe = std::env::current_exe().unwrap_or_default();
    let Some(target) = copy_target(&exe) else {
        return;
    };
    if let Err(e) = ensure_copy(&exe, &target, SystemTime::now()) {
        eprintln!(
            "majordomus: serving from {} itself, not a copy ({e}): a rebuild will supersede this server",
            exe.display()
        );
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // the process keeps the name it was started by, so the process table reads as it
        // always has; the kernel still knows the file it runs, which is what the lease records
        let mut args = std::env::args_os();
        let mut copy = std::process::Command::new(&target);
        // a process started with no argv at all was started by no name, and keeps none
        copy.arg0(args.next().unwrap_or_default());
        let err = copy.args(args).exec();
        eprintln!(
            "majordomus: could not run the copy {} ({err}); serving from {}",
            target.display(),
            exe.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rebuild as a build makes one: a new file renamed over the output, never the old
    /// file rewritten.
    fn rebuild(exe: &Path, bytes: &[u8]) {
        let fresh = exe.with_extension("new");
        fs::write(&fresh, bytes).unwrap();
        fs::rename(&fresh, exe).unwrap();
    }

    fn build_output(root: &Path) -> PathBuf {
        let debug = root.join("debug");
        fs::create_dir_all(&debug).unwrap();
        fs::write(root.join("CACHEDIR.TAG"), "Signature: x\n").unwrap();
        let exe = debug.join("majordomus");
        fs::write(&exe, b"first build").unwrap();
        exe
    }

    #[test]
    fn only_a_marked_build_output_is_copied() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        assert!(copy_target(&exe).is_some());
        // a copy is never copied again: its parent's parent carries no mark
        let copy = copy_target(&exe).unwrap();
        ensure_copy(&exe, &copy, SystemTime::now()).unwrap();
        assert_eq!(copy_target(&copy), None);
        // an unmarked directory, and a missing file, are served in place
        fs::remove_file(dir.path().join("CACHEDIR.TAG")).unwrap();
        assert_eq!(copy_target(&exe), None);
        assert_eq!(copy_target(Path::new("/nowhere/at/all/majordomus")), None);
    }

    #[test]
    fn a_build_output_that_is_gone_has_no_copy() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        fs::remove_file(&exe).unwrap();
        assert_eq!(
            copy_target(&exe),
            None,
            "the mark is there and the file is not"
        );
    }

    #[test]
    fn a_rebuild_names_a_new_copy_and_leaves_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let first = copy_target(&exe).unwrap();
        ensure_copy(&exe, &first, SystemTime::now()).unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"first build");
        // the same build asks again: the copy is reused, not rewritten
        let before = fs::metadata(&first).unwrap().modified().unwrap();
        ensure_copy(&exe, &first, SystemTime::now()).unwrap();
        assert_eq!(fs::metadata(&first).unwrap().modified().unwrap(), before);
        // a rebuild: a new file at the build output's path
        rebuild(&exe, b"the second build");
        let second = copy_target(&exe).unwrap();
        assert_ne!(first, second, "a different build names a different copy");
        ensure_copy(&exe, &second, SystemTime::now()).unwrap();
        assert_eq!(
            fs::read(&first).unwrap(),
            b"first build",
            "the leased copy is untouched"
        );
        assert_eq!(fs::read(&second).unwrap(), b"the second build");
    }

    #[test]
    fn copies_older_than_the_keep_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let first = copy_target(&exe).unwrap();
        ensure_copy(&exe, &first, SystemTime::now()).unwrap();
        rebuild(&exe, b"the second build");
        let second = copy_target(&exe).unwrap();
        // a week and a day later, the first copy is pruned and the one being made is kept
        let later = SystemTime::now() + KEEP_FOR + Duration::from_secs(86_400);
        ensure_copy(&exe, &second, later).unwrap();
        assert!(!first.exists(), "an old copy is removed");
        assert!(second.exists(), "the copy being served is kept");
    }

    #[test]
    fn only_a_process_that_may_hold_the_lease_is_copied() {
        use clap::Parser;
        let parse = |args: &[&str]| crate::cli::Cli::try_parse_from(args).unwrap().command;
        assert!(may_hold_the_lease(&parse(&["majordomus", "mcp"])));
        assert!(may_hold_the_lease(&parse(&["majordomus", "serve"])));
        assert!(!may_hold_the_lease(&parse(&[
            "majordomus",
            "serve",
            "status"
        ])));
        assert!(!may_hold_the_lease(&parse(&[
            "majordomus",
            "serve",
            "ensure"
        ])));
        // a client command returns at once; so does a server here, because a test binary
        // runs from target/<profile>/deps, which carries no mark of its own
        serve_from_a_copy(&parse(&["majordomus", "serve", "status"]));
        serve_from_a_copy(&parse(&["majordomus", "serve"]));
    }

    #[test]
    fn a_copy_knows_the_build_output_it_was_made_from() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let copy = copy_target(&exe).unwrap();
        assert_eq!(copied_from(&copy), Some(exe.clone()));
        assert_eq!(copied_from(&exe), None, "a build output is no copy");
        let served = dir.path().join("debug").join(SERVED_DIR);
        assert_eq!(copied_from(&served.join("majordomus")), None);
        assert_eq!(copied_from(&served.join("majordomus-1-x")), None);
        assert_eq!(copied_from(&served.join("-1-2")), None);
    }

    #[test]
    fn a_copy_already_there_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let target = copy_target(&exe).unwrap();
        ensure_copy(&exe, &target, SystemTime::now()).unwrap();
        // a second server of the same build, which finds the copy made: nothing is rewritten
        fs::remove_file(&target).unwrap();
        fs::write(&target, b"first build").unwrap();
        let held = fs::metadata(&target).unwrap().modified().unwrap();
        ensure_copy(&exe, &target, SystemTime::now()).unwrap();
        assert_eq!(fs::metadata(&target).unwrap().modified().unwrap(), held);
        // a file of another size at the copy's name is not run
        fs::write(&target, b"short").unwrap();
        assert!(ensure_copy(&exe, &target, SystemTime::now()).is_err());
        // a copy in the making is never pruned, however old
        let partial = target.parent().unwrap().join(".partial-1");
        fs::write(&partial, b"x").unwrap();
        prune(
            target.parent().unwrap(),
            &target,
            SystemTime::now() + KEEP_FOR * 2,
        );
        assert!(partial.exists());
    }

    #[test]
    fn a_copy_of_the_bytes_keeps_a_file_already_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let served = dir.path().join("debug").join(SERVED_DIR);
        fs::create_dir_all(&served).unwrap();
        let target = served.join("majordomus-1-11");
        copy_into_place(&exe, &target, &served).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"first build");
        // a second copier finds the first one's file and keeps it
        rebuild(&exe, b"another one");
        copy_into_place(&exe, &target, &served).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"first build");
        let left: Vec<_> = fs::read_dir(&served).unwrap().flatten().collect();
        assert_eq!(left.len(), 1, "no partial file is left behind");
    }

    #[test]
    fn a_copy_that_cannot_be_made_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let target = copy_target(&exe).unwrap();
        fs::remove_file(&exe).unwrap();
        assert!(ensure_copy(&exe, &target, SystemTime::now()).is_err());
    }
    #[test]
    fn an_executable_that_names_no_build_output_has_no_copy() {
        // no parent at all
        assert_eq!(copy_target(Path::new("/")), None);
        // the name an executable gets when it cannot name itself
        assert_eq!(copy_target(Path::new("")), None);
        // a parent with no parent: a bare file name
        assert_eq!(copy_target(Path::new("majordomus")), None);
        // a marked build output whose path ends in `..` names no file
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let dotdot = exe.parent().unwrap().join("..");
        assert_eq!(copy_target(&dotdot), None);
        assert!(
            copy_target(&exe).is_some(),
            "the marked output still is one"
        );
    }

    #[test]
    fn a_build_older_than_the_epoch_has_no_copy_name() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let before = SystemTime::UNIX_EPOCH - Duration::from_secs(1000);
        let set = fs::File::options()
            .write(true)
            .open(&exe)
            .and_then(|f| f.set_modified(before));
        // a filesystem that cannot hold such a time cannot produce the case either
        if set.is_err() || fs::metadata(&exe).unwrap().modified().unwrap() >= SystemTime::UNIX_EPOCH
        {
            return;
        }
        assert_eq!(copy_target(&exe), None);
    }

    #[test]
    fn a_copy_that_cannot_be_placed_is_an_error_naming_what_failed() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        // a target with no directory to hold it
        let e = ensure_copy(&exe, Path::new("/"), SystemTime::now()).unwrap_err();
        assert!(e.to_string().contains("has a directory"), "{e}");
        // a directory that cannot be made, because a file is where it goes
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, b"x").unwrap();
        let e = ensure_copy(&exe, &blocker.join("served/copy"), SystemTime::now()).unwrap_err();
        assert_ne!(e.kind(), io::ErrorKind::AlreadyExists, "{e}");
        assert!(!blocker.join("served").exists());
        // a "build" that cannot be linked or copied, because it is a directory
        let served = dir.path().join("served");
        let target = served.join("copy-1-1");
        assert!(ensure_copy(dir.path(), &target, SystemTime::now()).is_err());
        assert!(!target.exists(), "nothing was placed");
        let left: Vec<_> = fs::read_dir(&served).unwrap().flatten().collect();
        assert!(left.is_empty(), "no partial file is left behind");
    }

    #[cfg(unix)]
    #[test]
    fn a_name_that_exists_but_leads_nowhere_is_not_a_whole_copy() {
        let dir = tempfile::tempdir().unwrap();
        let exe = build_output(dir.path());
        let served = dir.path().join("debug").join(SERVED_DIR);
        fs::create_dir_all(&served).unwrap();
        let target = served.join("majordomus-1-11");
        // the name is taken (by a link to nothing), so the hard link finds it there and the
        // copy is then judged by what the name leads to
        std::os::unix::fs::symlink(dir.path().join("nowhere"), &target).unwrap();
        let e = ensure_copy(&exe, &target, SystemTime::now()).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound, "{e}");
    }

    #[test]
    fn a_path_is_a_copy_only_when_it_is_named_as_one() {
        // no parent
        assert_eq!(copied_from(Path::new("/")), None);
        // a parent with no name
        assert_eq!(copied_from(Path::new("/majordomus-1-2")), None);
        // a copy's name that is not text
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::OsStr::from_bytes(b"majordomus-\xff-2");
            assert_eq!(copied_from(&Path::new("debug/served").join(name)), None);
        }
        // a path that ends in `..` has no file name
        assert_eq!(copied_from(Path::new("debug/served/..")), None);
        // no dash at all, and one dash only
        assert_eq!(copied_from(Path::new("debug/served/majordomus")), None);
        assert_eq!(copied_from(Path::new("debug/served/majordomus-2")), None);
        // a size that is not digits, a time that is not, and an empty stem
        assert_eq!(copied_from(Path::new("debug/served/majordomus-1-x")), None);
        assert_eq!(copied_from(Path::new("debug/served/majordomus-x-1")), None);
        assert_eq!(copied_from(Path::new("debug/served/-1-2")), None);
        // and the shape that is one
        assert_eq!(
            copied_from(Path::new("debug/served/majordomus-1-2")),
            Some(PathBuf::from("debug/majordomus"))
        );
    }

    #[test]
    fn pruning_a_directory_that_cannot_be_listed_leaves_it_alone() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("served");
        prune(
            &missing,
            &missing.join("keep"),
            SystemTime::now() + KEEP_FOR * 2,
        );
        assert!(!missing.exists(), "nothing was made");
    }

    /// The entry the child processes of the tests below run: with no mode in the environment
    /// it does nothing, which is what an ordinary run of the suite sees.
    #[test]
    fn serve_from_a_copy_child_entry() {
        use clap::Parser;
        let Ok(mode) = std::env::var(CHILD_MODE) else {
            return;
        };
        let serve = crate::cli::Cli::try_parse_from(["majordomus", "serve"])
            .unwrap()
            .command;
        let exe = std::env::current_exe().unwrap();
        match mode.as_str() {
            "ensure-fails" => {
                // the copy's name is taken by a file of another size
                let target = copy_target(&exe).expect("this binary is a marked build output");
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::write(&target, b"short").unwrap();
            }
            #[cfg(unix)]
            "exec-fails" => {
                use std::os::unix::fs::PermissionsExt;
                // the copy is a second name of this very file, which can no longer be run
                fs::set_permissions(&exe, fs::Permissions::from_mode(0o644)).unwrap();
            }
            _ => {}
        }
        serve_from_a_copy(&serve);
        println!("returned: {mode}");
    }

    const CHILD_MODE: &str = "MJ_SERVED_EXECUTABLE_TEST_MODE";

    /// Run this test binary, copied to `exe`, as the one test that serves from a copy, in
    /// `mode`; its status and what it wrote to standard error and output.
    fn run_child(exe: &Path, mode: &str) -> (bool, String, String) {
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::copy(std::env::current_exe().unwrap(), exe).unwrap();
        let out = std::process::Command::new(exe)
            .args([
                "--exact",
                "served_executable::tests::serve_from_a_copy_child_entry",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_MODE, mode)
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    }

    #[test]
    fn a_server_that_is_no_build_output_serves_in_place_without_a_word() {
        let dir = tempfile::tempdir().unwrap();
        // no mark above the profile directory
        let exe = dir.path().join("plain").join("bin").join("majordomus");
        let (ok, err, out) = run_child(&exe, "unmarked");
        assert!(ok, "{err}{out}");
        assert!(out.contains("returned: unmarked"), "{out}");
        assert!(!err.contains("majordomus: "), "{err}");
        assert!(!exe.parent().unwrap().join(SERVED_DIR).exists());
    }

    #[test]
    fn a_copy_that_cannot_be_made_is_said_on_stderr_and_the_server_serves_in_place() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CACHEDIR.TAG"), "Signature: x\n").unwrap();
        let exe = dir.path().join("debug").join("majordomus");
        let (ok, err, out) = run_child(&exe, "ensure-fails");
        assert!(ok, "{err}{out}");
        assert!(out.contains("returned: ensure-fails"), "{out}");
        assert!(err.contains("serving from "), "{err}");
        assert!(err.contains("itself, not a copy"), "{err}");
        assert!(err.contains("is not a whole copy of the build"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_copy_that_cannot_be_run_is_said_on_stderr_and_the_server_serves_in_place() {
        // permissions do not bind a privileged process, which would run the copy
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CACHEDIR.TAG"), "Signature: x\n").unwrap();
        let exe = dir.path().join("debug").join("majordomus");
        let (ok, err, out) = run_child(&exe, "exec-fails");
        assert!(ok, "{err}{out}");
        assert!(out.contains("returned: exec-fails"), "{out}");
        assert!(err.contains("could not run the copy "), "{err}");
        assert!(err.contains("); serving from "), "{err}");
        let served: Vec<_> = fs::read_dir(dir.path().join("debug").join(SERVED_DIR))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(
            served.len(),
            1,
            "the copy was made, and then refused to run"
        );
    }
}
