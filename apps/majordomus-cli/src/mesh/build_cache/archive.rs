//! The archive of one cached directory: written with the system's `tar` and gzip, read back
//! only after every entry has been judged safe (I3952, ADR 0157 decision 9).
//!
//! **Extracted beside, never over.** An archive is unpacked into a fresh directory next to its
//! target, on the same filesystem, so that putting it in place is a rename: a reader of the
//! target sees the old content or the new, never half of one.
//!
//! **Judged twice.** The names `tar` lists are refused when one is absolute or climbs with
//! `..`, before anything is written; then the unpacked tree is walked without following a
//! link, and a link that resolves outside the tree, or a file that is neither a regular file,
//! a directory nor a link, refuses the whole archive. A refusal is a miss: the caller runs
//! the install itself, exactly as it would have without the cache.
//!
//! **Bounded.** Every `tar` this module runs carries [`TAR_BUDGET`]; one that does not finish
//! within it is killed and counted as a failure (rule `project.every-wait-is-bounded`).

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::s3::hex;
use crate::environment::probe::bounded_output;

/// The longest one `tar` may run: a cargo build directory of three gigabytes is written in
/// about a minute on the machines this was measured on, so this is a stall, not a slow disk.
pub(crate) const TAR_BUDGET: Duration = Duration::from_secs(1800);

/// The lock file cargo holds on a build directory's profile from its first compile to its
/// last, and the one a target directory of an older cargo has.
pub(crate) const CARGO_LOCKS: [&str; 2] = [".cargo-build-lock", ".cargo-lock"];

/// The SHA-256 of a file, streamed, as lowercase hex.
pub(crate) fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// A directory that is removed when this value is dropped, whatever happened in between.
#[derive(Debug)]
pub(crate) struct Scratch {
    pub(crate) path: PathBuf,
}

impl Scratch {
    /// A new, empty directory beside `target` (same parent, so on the same filesystem), named
    /// after it, this process and the moment.
    pub(crate) fn beside(target: &Path, purpose: &str) -> io::Result<Scratch> {
        let parent = target.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "a target needs a parent")
        })?;
        fs::create_dir_all(parent)?;
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "target".into());
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = parent.join(format!(
            ".{name}.mj-{purpose}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Scratch { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_tar(args: &[&std::ffi::OsStr]) -> Result<Vec<u8>, String> {
    run_tar_within(args, TAR_BUDGET)
}

/// [`run_tar`] with the budget as a parameter, so that a stall is testable in milliseconds.
fn run_tar_within(args: &[&std::ffi::OsStr], budget: Duration) -> Result<Vec<u8>, String> {
    let mut command = Command::new("tar");
    command.args(args);
    // macOS tar otherwise adds an AppleDouble `._name` entry for every file with extended
    // attributes, which another platform would unpack as files of its own
    command.env("COPYFILE_DISABLE", "1");
    let output = bounded_output(&mut command, budget)
        .ok_or_else(|| "tar did not start, or did not finish within its budget".to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "tar failed: {}",
            err.lines().next().unwrap_or("no message").trim()
        ))
    }
}

/// Write `source`'s content (not `source` itself) into the gzip archive `dest`, leaving out
/// every entry whose name is one of `exclude`.
pub(crate) fn create(source: &Path, dest: &Path, exclude: &[&str]) -> Result<(), String> {
    let excludes: Vec<std::ffi::OsString> = exclude
        .iter()
        .map(|e| std::ffi::OsString::from(format!("--exclude={e}")))
        .collect();
    let mut args: Vec<&std::ffi::OsStr> = vec!["-czf".as_ref(), dest.as_os_str()];
    args.extend(excludes.iter().map(|e| e.as_os_str()));
    args.extend(["-C".as_ref(), source.as_os_str(), ".".as_ref()]);
    run_tar(&args).map(|_| ())
}

/// Why one listed name may not be unpacked, or `None` when it may.
pub(crate) fn refuse_name(name: &str) -> Option<String> {
    if name.starts_with('/') {
        return Some(format!("{name}: an absolute name"));
    }
    if Path::new(name)
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Some(format!("{name}: climbs out with .."));
    }
    None
}

/// Every name the archive lists, judged before anything is written.
pub(crate) fn check_listing(archive: &Path) -> Result<usize, String> {
    let listing = run_tar(&["-tzf".as_ref(), archive.as_os_str()])?;
    let text = String::from_utf8_lossy(&listing);
    let mut n = 0;
    for name in text.lines().filter(|l| !l.is_empty()) {
        if let Some(why) = refuse_name(name) {
            return Err(format!("the archive was refused: {why}"));
        }
        n += 1;
    }
    Ok(n)
}

/// Where a link at `link` (relative to the tree's root) with the relative target `to` lands,
/// lexically; `None` when it leaves the tree.
pub(crate) fn resolve_inside(link: &Path, to: &Path) -> Option<PathBuf> {
    if to.is_absolute() {
        return None;
    }
    let mut at: Vec<std::ffi::OsString> = link
        .parent()
        .map(|p| {
            p.components()
                .filter_map(|c| match c {
                    Component::Normal(s) => Some(s.to_os_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    for c in to.components() {
        match c {
            Component::Normal(s) => at.push(s.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                at.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(at.iter().collect())
}

/// Walk an unpacked tree without following a link: every entry is a directory, a regular
/// file or a link that stays inside the tree.
pub(crate) fn check_tree(root: &Path) -> Result<(), String> {
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let dir = root.join(&rel);
        let entries = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", rel.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let rel_entry = rel.join(entry.file_name());
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                stack.push(rel_entry);
            } else if kind.is_symlink() {
                let to = fs::read_link(entry.path()).map_err(|e| e.to_string())?;
                if resolve_inside(&rel_entry, &to).is_none() {
                    return Err(format!(
                        "the archive was refused: {} links to {}, outside its target",
                        rel_entry.display(),
                        to.display()
                    ));
                }
            } else if !kind.is_file() {
                return Err(format!(
                    "the archive was refused: {} is neither a file, a directory nor a link",
                    rel_entry.display()
                ));
            }
        }
    }
    Ok(())
}

/// Unpack `archive` into a fresh directory beside `target`, judged before and after. The
/// returned directory is removed when dropped unless its content was moved out first.
pub(crate) fn unpack_beside(archive: &Path, target: &Path) -> Result<Scratch, String> {
    check_listing(archive)?;
    let scratch = Scratch::beside(target, "restore").map_err(|e| e.to_string())?;
    run_tar(&[
        "-xzf".as_ref(),
        archive.as_os_str(),
        "-C".as_ref(),
        scratch.path.as_os_str(),
    ])?;
    check_tree(&scratch.path)?;
    Ok(scratch)
}

/// Put an unpacked tree in place of `target`: the old target, if any, is moved aside, the
/// new one renamed in, and the old one removed. A failed rename puts the old one back.
pub(crate) fn replace(scratch: Scratch, target: &Path) -> Result<(), String> {
    let aside = if target.symlink_metadata().is_ok() {
        let trash = Scratch::beside(target, "old").map_err(|e| e.to_string())?;
        let into = trash.path.join("old");
        fs::rename(target, &into)
            .map_err(|e| format!("{} cannot be moved aside: {e}", target.display()))?;
        Some((trash, into))
    } else {
        None
    };
    if let Err(e) = fs::rename(&scratch.path, target) {
        if let Some((_, into)) = &aside {
            let _ = fs::rename(into, target);
        }
        return Err(format!("{} cannot be put in place: {e}", target.display()));
    }
    // the scratch path is now the target: nothing is left for its guard to remove
    std::mem::forget(scratch);
    Ok(())
}

/// Is a build directory cold: absent, or holding nothing but directories, cargo's lock files
/// and its `CACHEDIR.TAG`?
pub(crate) fn is_cold(dir: &Path) -> bool {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            // absent is cold; a directory that cannot be read is not known to be
            return d == dir && !dir.exists();
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(_) if CARGO_LOCKS.contains(&name.as_ref()) || name == "CACHEDIR.TAG" => {}
                _ => return false,
            }
        }
    }
    true
}

/// How many times a lock is tried, 20 ms apart, before it counts as held.
const LOCK_TRIES: u32 = 10;

/// An exclusive flock(2) held on one file until dropped.
#[derive(Debug)]
pub(crate) struct Held {
    _file: File,
}

/// Take cargo's lock on `profile` (its `.cargo-build-lock`, created when absent) without
/// waiting: `None` when some build holds it.
pub(crate) fn lock_profile(profile: &Path, create: bool) -> Result<Option<Held>, String> {
    use std::os::fd::AsRawFd;
    let path = profile.join(CARGO_LOCKS[0]);
    if !create && !path.exists() {
        return Ok(Some(Held {
            _file: File::open(profile).map_err(|e| e.to_string())?,
        }));
    }
    fs::create_dir_all(profile).map_err(|e| e.to_string())?;
    let file = fs::OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    // A few tries a moment apart, never a wait on a build: cargo holds its lock for the whole
    // build, while the copy of a released descriptor that a child forked in another thread
    // keeps until it execs is gone within milliseconds.
    for attempt in 0..LOCK_TRIES {
        // SAFETY: the descriptor is this open file's own, and flock only reads it.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            return Ok(Some(Held { _file: file }));
        }
        if attempt + 1 < LOCK_TRIES {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    Ok(None)
}

/// Hold cargo's lock on every profile directory directly under `build_dir` that `names`
/// lists; `Err` naming the first one some build holds.
pub(crate) fn lock_profiles(
    build_dir: &Path,
    names: &[String],
    create: bool,
) -> Result<Vec<Held>, String> {
    let mut held = Vec::new();
    for name in names {
        match lock_profile(&build_dir.join(name), create)? {
            Some(h) => held.push(h),
            None => {
                return Err(format!(
                    "a build holds cargo's lock on {}",
                    build_dir.join(name).display()
                ))
            }
        }
    }
    Ok(held)
}

/// The directories directly under `dir`, by name.
pub(crate) fn subdirectories(dir: &Path) -> Vec<String> {
    let names: std::collections::BTreeSet<String> = fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.into_iter().collect()
}

/// Move every entry of `from` into `into`, renaming whole subtrees where `into` has none and
/// descending where both have a directory; an entry `into` already has is kept as it is.
pub(crate) fn merge_into(from: &Path, into: &Path) -> io::Result<()> {
    fs::create_dir_all(into)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = into.join(entry.file_name());
        match dest.symlink_metadata() {
            Err(_) => fs::rename(entry.path(), &dest)?,
            Ok(meta) if meta.is_dir() && entry.file_type()?.is_dir() => {
                merge_into(&entry.path(), &dest)?
            }
            Ok(_) => {}
        }
    }
    Ok(())
}

/// Put an unpacked build directory in place of a cold one, under cargo's own lock on each of
/// its profiles: the build directory is judged cold again while the locks are held, so a build
/// that started meanwhile is never written under.
pub(crate) fn place_build_dir(scratch: Scratch, build_dir: &Path) -> Result<(), String> {
    let profiles = subdirectories(&scratch.path);
    let _held = lock_profiles(build_dir, &profiles, true)?;
    if !is_cold(build_dir) {
        return Err(format!(
            "{} is no longer cold: a build wrote to it meanwhile",
            build_dir.display()
        ));
    }
    merge_into(&scratch.path, build_dir).map_err(|e| {
        format!(
            "{} could not be filled from the archive: {e}",
            build_dir.display()
        )
    })
}

/// Total bytes of the regular files under `dir`, links not followed.
pub(crate) fn tree_bytes(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(t) if t.is_file() => total += entry.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(dir: &Path, files: &[(&str, &str)]) {
        for (rel, body) in files {
            let p = dir.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
    }

    #[test]
    fn names_that_are_absolute_or_climb_are_refused() {
        assert!(refuse_name("/etc/passwd").unwrap().contains("absolute"));
        assert!(refuse_name("a/../../x").unwrap().contains(".."));
        assert!(refuse_name("../x").is_some());
        assert_eq!(refuse_name("./a/b"), None);
        assert_eq!(refuse_name("a..b/c"), None);
    }

    #[test]
    fn a_link_resolves_lexically_inside_or_not_at_all() {
        let p = Path::new;
        assert_eq!(
            resolve_inside(p(".bin/x"), p("../pkg/bin/x")),
            Some(PathBuf::from("pkg/bin/x"))
        );
        assert_eq!(resolve_inside(p("a"), p("./b")), Some(PathBuf::from("b")));
        assert_eq!(resolve_inside(p("a"), p("../outside")), None);
        assert_eq!(resolve_inside(p("a/b"), p("/etc")), None);
        assert_eq!(resolve_inside(p("a/b"), p("../../..")), None);
    }

    #[test]
    fn an_archive_round_trips_and_leaves_out_what_it_was_told_to() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        tree(&src, &[("a/b.txt", "b"), ("c.txt", "c"), ("skip.me", "x")]);
        std::os::unix::fs::symlink("a/b.txt", src.join("link")).unwrap();
        let archive = dir.path().join("x.tar.gz");
        create(&src, &archive, &["skip.me"]).unwrap();
        assert_eq!(sha256_file(&archive).unwrap().len(), 64);
        let target = dir.path().join("out");
        let scratch = unpack_beside(&archive, &target).unwrap();
        assert!(scratch.path.starts_with(dir.path()));
        replace(scratch, &target).unwrap();
        assert_eq!(fs::read_to_string(target.join("a/b.txt")).unwrap(), "b");
        assert!(!target.join("skip.me").exists());
        assert_eq!(fs::read_to_string(target.join("link")).unwrap(), "b");
        // nothing is left beside the target
        let siblings: Vec<_> = fs::read_dir(dir.path()).unwrap().flatten().collect();
        assert_eq!(siblings.len(), 3, "{siblings:?}");
    }

    #[test]
    fn replacing_an_existing_target_swaps_it_whole() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("node_modules");
        tree(&target, &[("old.js", "old")]);
        let scratch = Scratch::beside(&target, "restore").unwrap();
        tree(&scratch.path, &[("new.js", "new")]);
        replace(scratch, &target).unwrap();
        assert!(target.join("new.js").exists());
        assert!(!target.join("old.js").exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_link_leaving_the_tree_and_a_fifo_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("t");
        tree(&root, &[("ok/x", "x")]);
        std::os::unix::fs::symlink("../ok/x", root.join("ok/inside")).unwrap();
        assert_eq!(check_tree(&root), Ok(()));
        std::os::unix::fs::symlink("../../escape", root.join("ok/out")).unwrap();
        assert!(check_tree(&root)
            .unwrap_err()
            .contains("outside its target"));
        fs::remove_file(root.join("ok/out")).unwrap();
        let fifo = std::ffi::CString::new(root.join("pipe").to_string_lossy().as_bytes()).unwrap();
        // SAFETY: a valid NUL-terminated path; mkfifo only creates the node.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(check_tree(&root).unwrap_err().contains("neither a file"));
    }

    #[test]
    fn an_archive_carrying_an_escaping_link_is_refused_and_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        std::os::unix::fs::symlink("/etc", src.join("evil")).unwrap();
        let archive = dir.path().join("evil.tar.gz");
        create(&src, &archive, &[]).unwrap();
        let target = dir.path().join("out");
        let err = unpack_beside(&archive, &target).unwrap_err();
        assert!(err.contains("outside its target"), "{err}");
        assert!(!target.exists());
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            2,
            "the scratch is gone"
        );
    }

    #[test]
    fn something_that_is_not_an_archive_fails_and_says_tar() {
        let dir = tempfile::tempdir().unwrap();
        let bogus = dir.path().join("bogus.tar.gz");
        fs::write(&bogus, b"not gzip at all").unwrap();
        assert!(check_listing(&bogus).unwrap_err().contains("tar failed"));
        assert!(
            create(&dir.path().join("absent"), &dir.path().join("o.tgz"), &[])
                .unwrap_err()
                .contains("tar failed")
        );
    }

    #[test]
    fn coldness_admits_only_locks_the_tag_and_empty_directories() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path().join("build");
        assert!(is_cold(&b), "absent is cold");
        fs::create_dir_all(b.join("debug")).unwrap();
        fs::write(b.join("debug/.cargo-build-lock"), "").unwrap();
        fs::write(b.join("CACHEDIR.TAG"), "Signature").unwrap();
        assert!(is_cold(&b));
        fs::write(b.join("debug/dep.rlib"), "x").unwrap();
        assert!(!is_cold(&b));
    }

    #[test]
    fn a_build_directory_is_filled_under_cargos_lock_and_not_while_a_build_holds_it() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path().join("build");
        let scratch = Scratch::beside(&b, "restore").unwrap();
        tree(
            &scratch.path,
            &[("CACHEDIR.TAG", "Signature"), ("debug/deps/a.rlib", "a")],
        );
        place_build_dir(scratch, &b).unwrap();
        assert_eq!(
            fs::read_to_string(b.join("debug/deps/a.rlib")).unwrap(),
            "a"
        );
        assert!(b.join("debug/.cargo-build-lock").exists());
        assert!(b.join("CACHEDIR.TAG").exists());
        assert!(tree_bytes(&b) >= 2);

        // now warm: a second restore is refused
        let scratch = Scratch::beside(&b, "restore").unwrap();
        tree(&scratch.path, &[("debug/deps/b.rlib", "b")]);
        assert!(place_build_dir(scratch, &b)
            .unwrap_err()
            .contains("no longer cold"));
        assert!(!b.join("debug/deps/b.rlib").exists());

        // a held lock refuses, without waiting
        let c = dir.path().join("cold");
        let holder = lock_profile(&c.join("debug"), true)
            .unwrap()
            .expect("first lock");
        let scratch = Scratch::beside(&c, "restore").unwrap();
        tree(&scratch.path, &[("debug/deps/c.rlib", "c")]);
        assert!(place_build_dir(scratch, &c)
            .unwrap_err()
            .contains("holds cargo's lock"));
        drop(holder);
        assert!(
            lock_profiles(&c, &["debug".into()], false).is_ok(),
            "released, it is taken again"
        );
        assert_eq!(subdirectories(&c), vec!["debug".to_string()]);
    }

    fn is_root() -> bool {
        // SAFETY: geteuid has no preconditions and reads the process's own credentials.
        unsafe { libc::geteuid() == 0 }
    }

    fn mode(path: &Path, bits: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(bits)).unwrap();
    }

    /// A gzip tar archive written by hand, so that it can carry what `tar -c` never writes:
    /// `(name, typeflag, link name, content)`.
    fn forged(entries: &[(&str, u8, &str, &str)]) -> Vec<u8> {
        let mut raw = Vec::new();
        for (name, flag, link, body) in entries {
            let mut h = [0u8; 512];
            h[..name.len()].copy_from_slice(name.as_bytes());
            h[100..107].copy_from_slice(b"0000644");
            h[108..115].copy_from_slice(b"0000000");
            h[116..123].copy_from_slice(b"0000000");
            h[124..135].copy_from_slice(format!("{:011o}", body.len()).as_bytes());
            h[136..147].copy_from_slice(b"00000000000");
            h[156] = *flag;
            h[157..157 + link.len()].copy_from_slice(link.as_bytes());
            h[257..263].copy_from_slice(b"ustar\0");
            h[263..265].copy_from_slice(b"00");
            h[148..156].copy_from_slice(b"        ");
            let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
            h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
            raw.extend_from_slice(&h);
            raw.extend_from_slice(body.as_bytes());
            raw.resize(raw.len().div_ceil(512) * 512, 0);
        }
        raw.extend_from_slice(&[0u8; 1024]);
        let mut gzip = Command::new("gzip")
            .arg("-c")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = gzip.stdin.take().unwrap();
        let writer = std::thread::spawn(move || {
            use std::io::Write;
            stdin.write_all(&raw).unwrap();
        });
        let out = gzip.wait_with_output().unwrap();
        writer.join().unwrap();
        out.stdout
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_not_a_digest() {
        let dir = tempfile::tempdir().unwrap();
        // absent: the open fails
        assert!(sha256_file(&dir.path().join("none")).is_err());
        // a directory opens but cannot be read
        assert!(sha256_file(dir.path()).is_err());
    }

    #[test]
    fn a_scratch_directory_needs_a_parent_it_can_make_and_write_in() {
        let dir = tempfile::tempdir().unwrap();
        // no parent at all
        assert!(Scratch::beside(Path::new("/"), "x").is_err());
        // a parent that is a file cannot be made a directory
        let file = dir.path().join("file");
        fs::write(&file, "x").unwrap();
        assert!(Scratch::beside(&file.join("t"), "x").is_err());
        // a target with no final name (`..`) is named "target"
        let odd = dir.path().join("p/..");
        let scratch = Scratch::beside(&odd, "x").unwrap();
        let name = scratch
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(name.starts_with(".target.mj-x-"), "{name}");
        drop(scratch);
        // a parent that cannot be written in refuses the directory itself
        if !is_root() {
            let locked = dir.path().join("locked");
            fs::create_dir(&locked).unwrap();
            mode(&locked, 0o500);
            let refused = Scratch::beside(&locked.join("t"), "x");
            mode(&locked, 0o700);
            assert!(refused.is_err());
        }
    }

    #[test]
    fn a_tar_that_stalls_is_killed_and_counted_as_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("pipe");
        let c = std::ffi::CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
        // SAFETY: a valid NUL-terminated path; mkfifo only creates the node.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        // tar blocks opening a pipe nobody writes to
        let err = run_tar_within(
            &["-tzf".as_ref(), fifo.as_os_str()],
            Duration::from_millis(300),
        )
        .unwrap_err();
        assert!(err.contains("did not start, or did not finish"), "{err}");
    }

    #[test]
    fn an_archive_naming_a_path_outside_its_target_is_refused_before_anything_is_written() {
        let dir = tempfile::tempdir().unwrap();
        for (name, why) in [("/abs/file", "absolute"), ("a/../../up", "..")] {
            let archive = dir.path().join("bad.tar.gz");
            fs::write(&archive, forged(&[(name, b'0', "", "x")])).unwrap();
            let err = check_listing(&archive).unwrap_err();
            assert!(err.starts_with("the archive was refused"), "{err}");
            assert!(err.contains(why), "{err}");
            // and the unpack stops there: no scratch is made
            let target = dir.path().join("out");
            assert!(unpack_beside(&archive, &target).is_err());
            assert_eq!(
                fs::read_dir(dir.path()).unwrap().count(),
                1,
                "only the archive"
            );
        }
        let fine = dir.path().join("fine.tar.gz");
        fs::write(&fine, forged(&[("a", b'0', "", "x"), ("d/", b'5', "", "")])).unwrap();
        assert_eq!(check_listing(&fine), Ok(2));
    }

    #[test]
    fn a_link_in_a_dotted_path_resolves_against_its_normal_components_only() {
        assert_eq!(
            resolve_inside(Path::new("./a/./b"), Path::new("c")),
            Some(PathBuf::from("a/c"))
        );
        assert_eq!(
            resolve_inside(Path::new("../a/b"), Path::new("c")),
            Some(PathBuf::from("a/c"))
        );
    }

    #[test]
    fn a_tree_that_cannot_be_read_is_refused_with_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let err = check_tree(&dir.path().join("absent")).unwrap_err();
        assert!(
            err.starts_with(": "),
            "the root is named by its empty relative path: {err}"
        );
    }

    #[test]
    fn an_archive_that_lists_fine_but_cannot_be_unpacked_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("hard.tar.gz");
        // a hard link to an entry the archive does not carry: listed, then refused by tar
        fs::write(&archive, forged(&[("link", b'1', "missing-target", "")])).unwrap();
        assert_eq!(check_listing(&archive), Ok(1));
        let target = dir.path().join("out");
        let err = unpack_beside(&archive, &target).unwrap_err();
        assert!(err.contains("tar failed"), "{err}");
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "the scratch is gone"
        );
    }

    #[test]
    fn unpacking_beside_a_target_without_a_parent_fails_after_the_listing() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        tree(&src, &[("a", "a")]);
        let archive = dir.path().join("ok.tar.gz");
        create(&src, &archive, &[]).unwrap();
        let err = unpack_beside(&archive, Path::new("/")).unwrap_err();
        assert!(err.contains("needs a parent"), "{err}");
    }

    #[test]
    fn replacing_a_target_reports_what_could_not_be_moved_and_puts_the_old_one_back() {
        let dir = tempfile::tempdir().unwrap();
        // the target exists but has no parent to put a trash beside
        let scratch = Scratch::beside(&dir.path().join("s"), "restore").unwrap();
        let err = replace(scratch, Path::new("/")).unwrap_err();
        assert!(err.contains("needs a parent"), "{err}");

        // a directory that cannot be moved aside
        if !is_root() {
            let target = dir.path().join("fixed");
            tree(&target, &[("old", "old")]);
            let scratch = Scratch::beside(&target, "restore").unwrap();
            mode(&target, 0o500);
            let err = replace(scratch, &target).unwrap_err();
            mode(&target, 0o700);
            assert!(err.contains("cannot be moved aside"), "{err}");
            assert!(target.join("old").exists());
        }

        // nothing to put in place, and no old target: the refusal names the target
        let fresh = dir.path().join("fresh");
        let scratch = Scratch::beside(&fresh, "restore").unwrap();
        fs::remove_dir(&scratch.path).unwrap();
        let err = replace(scratch, &fresh).unwrap_err();
        assert!(err.contains("cannot be put in place"), "{err}");
        assert!(!fresh.exists());

        // nothing to put in place, with an old target: the old one is back where it was
        let kept = dir.path().join("kept");
        tree(&kept, &[("old", "old")]);
        let scratch = Scratch::beside(&kept, "restore").unwrap();
        fs::remove_dir(&scratch.path).unwrap();
        let err = replace(scratch, &kept).unwrap_err();
        assert!(err.contains("cannot be put in place"), "{err}");
        assert_eq!(fs::read_to_string(kept.join("old")).unwrap(), "old");
    }

    #[test]
    fn a_profile_lock_says_why_it_could_not_be_taken() {
        let dir = tempfile::tempdir().unwrap();
        // without creating, an absent profile has no directory to stand for
        assert!(lock_profile(&dir.path().join("absent"), false).is_err());
        // without creating, an existing profile with no lock file is taken as it is
        let bare = dir.path().join("bare");
        fs::create_dir(&bare).unwrap();
        assert!(lock_profile(&bare, false).unwrap().is_some());
        // creating under a file cannot make the directory
        let file = dir.path().join("file");
        fs::write(&file, "x").unwrap();
        assert!(lock_profile(&file.join("debug"), true).is_err());
        assert!(lock_profiles(&file, &["debug".into()], true).is_err());
        // a lock path that is a directory cannot be opened for appending
        let odd = dir.path().join("odd");
        fs::create_dir_all(odd.join(CARGO_LOCKS[0])).unwrap();
        assert!(lock_profile(&odd, true).unwrap_err().contains("odd"));
    }

    #[test]
    fn merging_keeps_what_is_there_descends_into_shared_directories_and_reports_failures() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("from");
        let into = dir.path().join("into");
        tree(
            &from,
            &[
                ("shared/new", "n"),
                ("shared/same", "from"),
                ("only", "o"),
                ("clash", "from"),
            ],
        );
        tree(&into, &[("shared/same", "into"), ("clash/inside", "i")]);
        merge_into(&from, &into).unwrap();
        assert_eq!(fs::read_to_string(into.join("shared/new")).unwrap(), "n");
        assert_eq!(
            fs::read_to_string(into.join("shared/same")).unwrap(),
            "into"
        );
        assert_eq!(fs::read_to_string(into.join("only")).unwrap(), "o");
        assert!(
            into.join("clash").is_dir(),
            "an entry already there is kept as it is"
        );

        // a source that is not there
        assert!(merge_into(&dir.path().join("none"), &dir.path().join("x")).is_err());
        // a destination that cannot be made
        let file = dir.path().join("file");
        fs::write(&file, "x").unwrap();
        assert!(merge_into(&from, &file.join("d")).is_err());
        // a destination that cannot be written in
        if !is_root() {
            let locked = dir.path().join("locked");
            fs::create_dir(&locked).unwrap();
            mode(&locked, 0o500);
            let refused = merge_into(&from, &locked);
            mode(&locked, 0o700);
            assert!(refused.is_err());
        }
    }

    #[test]
    fn a_build_directory_that_cannot_be_filled_says_so() {
        if is_root() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path().join("build");
        fs::create_dir_all(b.join("debug")).unwrap();
        fs::write(b.join("debug").join(CARGO_LOCKS[0]), "").unwrap();
        mode(&b.join("debug"), 0o500);
        let scratch = Scratch::beside(&b, "restore").unwrap();
        tree(&scratch.path, &[("debug/deps/a.rlib", "a")]);
        let err = place_build_dir(scratch, &b).unwrap_err();
        mode(&b.join("debug"), 0o700);
        assert!(
            err.contains("could not be filled from the archive"),
            "{err}"
        );
    }

    #[test]
    fn the_size_of_a_tree_counts_regular_files_only() {
        let dir = tempfile::tempdir().unwrap();
        tree(dir.path(), &[("a/b", "12345"), ("c", "123")]);
        std::os::unix::fs::symlink("a/b", dir.path().join("link")).unwrap();
        assert_eq!(tree_bytes(dir.path()), 8, "a link adds nothing");
        assert_eq!(tree_bytes(&dir.path().join("absent")), 0);
    }
}
