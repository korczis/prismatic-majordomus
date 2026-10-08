//! Where published handovers are kept: one git ref, `refs/majordomus/continuity`, whose
//! commit holds a tree of `records/<id>.json` blobs.
//!
//! Git is the transport because it is already what moves this repository between machines,
//! it works offline, and every clone already has it. A ref of its own, rather than files on
//! a branch, because of what a branch would cost:
//!
//! - **No branch is touched.** A record is written with plumbing (`hash-object`,
//!   `mktree`, `commit-tree`, `update-ref`), so neither the index nor the working tree nor
//!   HEAD moves, and a dirty tree can be handed over without anything being committed to the
//!   work it describes.
//! - **No pull request carries it.** A record on a feature branch would appear in that
//!   branch's diff, in its merge, and in every derived projection of tracked files.
//! - **Every worktree sees it.** Refs live in the common git directory.
//! - **It does not travel by accident.** A plain `git push` or `git pull` leaves it alone;
//!   `continuity sync` moves it, and only when asked.
//!
//! Records are immutable and named by their content digest, so two stores are merged by
//! taking the union of their files: the same name is the same bytes, and a name holding
//! different bytes on the two sides is reported, never resolved by picking one. The ref's
//! commit history is only a transport detail; the lineage of the work is in the records.
//!
//! What another machine published arrives under
//! `refs/majordomus/remotes/<remote>/continuity` when fetched, and is merged into the local
//! ref only by `sync`.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::record::{path_of, MAX_RECORD_BYTES};

/// The local ref.
pub const REF: &str = "refs/majordomus/continuity";

/// The ref a remote's records are fetched into.
pub fn remote_ref(remote: &str) -> String {
    format!("refs/majordomus/remotes/{remote}/continuity")
}

/// The most records one store is read with; past this the store is reported as over its
/// bound rather than read in full.
pub const MAX_RECORDS: usize = 10_000;

/// Is `name` a remote name this module will put into a refspec?
pub fn valid_remote(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.contains("..")
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn git(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.env_remove("GIT_DIR");
    cmd.env_remove("GIT_WORK_TREE");
    cmd.env_remove("GIT_INDEX_FILE");
    // a push or fetch never waits for a password prompt nobody is there to answer
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    // the checkout is the working directory rather than `-C`: the same repository, and a
    // checkout that does not exist is refused by the spawn itself
    cmd.current_dir(root);
    cmd
}

/// Run git in `root` with `env` added and `input` on its stdin, and answer its stdout.
///
/// Every git call of the store goes through here, so a failure has one shape: git could not
/// be started, its stdin could not be written (it exited without reading what it was
/// given), or it refused, with what it said on stderr. Stdin is written on a thread of its
/// own, because git may start answering before it has read everything (`cat-file --batch`
/// does) and a full stdout pipe would otherwise stall both sides.
fn exec(root: &Path, args: &[&str], env: &[(&str, &str)], input: &[u8]) -> Result<Vec<u8>, String> {
    let mut child = git(root)
        .args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run git: {e}"))?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let input = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let out = child.wait_with_output();
    let written = writer.join().expect("writing a pipe does not panic");
    let out = written
        .and(out)
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    run_with_input(root, args, &[])
}

fn run_with_input(root: &Path, args: &[&str], input: &[u8]) -> Result<String, String> {
    exec(root, args, &[], input).map(|out| String::from_utf8_lossy(&out).trim().to_string())
}

/// The commit a ref points at, or `None` when the ref does not exist.
pub fn tip(root: &Path, reference: &str) -> Option<String> {
    run(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ],
    )
    .ok()
    .filter(|s| !s.is_empty())
}

/// The files of a store's tree: path → blob id. Paths other than `records/<hex>.json` are
/// returned too, so that the reader can report them rather than silently skip them.
pub fn entries(root: &Path, commit: &str) -> Result<BTreeMap<String, String>, String> {
    let listing = run(root, &["ls-tree", "-r", "-z", "--full-tree", commit])?;
    let mut out = BTreeMap::new();
    for entry in listing.split('\0').filter(|e| !e.is_empty()) {
        // <mode> SP <type> SP <object> TAB <path>
        let (meta, path) = entry.split_once('\t').unwrap_or((entry, ""));
        let mut parts = meta.split(' ');
        // a blob is a file of the store; anything else in its tree (a gitlink) is not
        if let (_, Some("blob"), Some(object)) = (parts.next(), parts.next(), parts.next()) {
            out.insert(path.to_string(), object.to_string());
            if out.len() > MAX_RECORDS {
                return Err(format!(
                    "the store holds more than {MAX_RECORDS} files; it is read no further"
                ));
            }
        }
    }
    Ok(out)
}

/// The bytes of the given blobs, read in one `cat-file --batch`.
pub fn blobs(root: &Path, ids: &[&str]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut request = String::new();
    for id in ids {
        request.push_str(id);
        request.push('\n');
    }
    parse_batch(&exec(
        root,
        &["cat-file", "--batch"],
        &[],
        request.as_bytes(),
    )?)
}

/// The answer of `cat-file --batch`: `<id> <type> <size>\n<bytes>\n` per object, or
/// `<id> missing\n` for one the repository lacks, which is left out. A blob larger than
/// any record could be is answered empty, so admission refuses it without it being held.
fn parse_batch(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut result = BTreeMap::new();
    let mut at = 0usize;
    while at < data.len() {
        let Some(nl) = data[at..].iter().position(|b| *b == b'\n') else {
            return Err("git cat-file: a truncated answer".into());
        };
        let header = String::from_utf8_lossy(&data[at..at + nl]).to_string();
        at += nl + 1;
        let mut parts = header.split(' ');
        let (id, _kind, size) = (parts.next(), parts.next(), parts.next());
        let (Some(id), Some(size)) = (id, size.and_then(|s| s.parse::<usize>().ok())) else {
            // `<id> missing`
            continue;
        };
        if at + size > data.len() {
            return Err("git cat-file: a truncated answer".into());
        }
        let bytes = if size <= MAX_RECORD_BYTES * 2 {
            data[at..at + size].to_vec()
        } else {
            Vec::new()
        };
        result.insert(id.to_string(), bytes);
        at += size + 1;
    }
    Ok(result)
}

/// Read every file of the store at `reference`: path → bytes. An absent ref is an empty
/// store.
pub fn read(root: &Path, reference: &str) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let Some(commit) = tip(root, reference) else {
        return Ok(BTreeMap::new());
    };
    // one chain: the listing, then the bytes of what it listed, and either failing is the
    // read failing
    entries(root, &commit).and_then(|files| {
        let ids: Vec<&str> = files.values().map(String::as_str).collect();
        blobs(root, &ids).map(|bytes| {
            files
                .iter()
                .map(|(path, id)| (path.clone(), bytes.get(id).cloned().unwrap_or_default()))
                .collect()
        })
    })
}

/// Two stores held different bytes under one name. Records are content-addressed, so this
/// is corruption or tampering on one side; the local copy is kept and the name reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    /// The path in the tree.
    pub path: String,
}

/// The tree of a store holding `files` (path → blob id), written into the object database.
fn tree_of(root: &Path, files: &BTreeMap<String, String>) -> Result<String, String> {
    // `mktree` builds one level; records live one level down, so build `records/` first.
    let mut nested: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    let mut top: Vec<(String, String, String)> = Vec::new();
    for (path, blob) in files {
        match path.split_once('/') {
            Some((dir, name)) if !name.contains('/') => {
                nested.entry(dir).or_default().push((name, blob));
            }
            None => top.push(("blob".into(), blob.clone(), path.clone())),
            Some(_) => return Err(format!("{path}: the store holds one level of directories")),
        }
    }
    for (dir, children) in nested {
        let mut listing = String::new();
        for (name, blob) in children {
            listing.push_str(&format!("100644 blob {blob}\t{name}\0"));
        }
        let tree = run_with_input(root, &["mktree", "-z"], listing.as_bytes())?;
        top.push(("tree".into(), tree, dir.to_string()));
    }
    let mut listing = String::new();
    for (kind, object, name) in top {
        let mode = if kind == "tree" { "040000" } else { "100644" };
        listing.push_str(&format!("{mode} {kind} {object}\t{name}\0"));
    }
    run_with_input(root, &["mktree", "-z"], listing.as_bytes())
}

fn commit(root: &Path, tree: &str, parents: &[&str], message: &str) -> Result<String, String> {
    let mut args = vec!["commit-tree", tree];
    for p in parents {
        args.push("-p");
        args.push(p);
    }
    // The committer is whoever git says it is on this machine; a fixture with no identity
    // gets a neutral one rather than a refusal, since this commit carries no authored work.
    let neutral: &[(&str, &str)] = if run(root, &["config", "user.email"]).is_err() {
        &[
            ("GIT_AUTHOR_NAME", "majordomus"),
            ("GIT_AUTHOR_EMAIL", "majordomus@localhost"),
            ("GIT_COMMITTER_NAME", "majordomus"),
            ("GIT_COMMITTER_EMAIL", "majordomus@localhost"),
        ]
    } else {
        &[]
    };
    exec(root, &args, neutral, message.as_bytes())
        .map(|out| String::from_utf8_lossy(&out).trim().to_string())
}

/// Move `reference` from `old` to `new`, or fail if another writer moved it first.
fn update(root: &Path, reference: &str, new: &str, old: Option<&str>) -> Result<(), String> {
    // the zero id says "create; it must not exist yet"
    let zero = "0".repeat(new.len());
    let old = old.unwrap_or(&zero);
    run(
        root,
        &[
            "update-ref",
            "-m",
            "majordomus continuity",
            reference,
            new,
            old,
        ],
    )
    .map(|_| ())
}

/// Write `files` as the store's tree, commit it on `parents`, and move the local ref from
/// `old` to that commit — or fail, if another writer moved the ref first.
fn write_tip(
    root: &Path,
    files: &BTreeMap<String, String>,
    parents: &[&str],
    message: &str,
    old: Option<&str>,
) -> Result<String, String> {
    let tree = tree_of(root, files)?;
    let new = commit(root, &tree, parents, message)?;
    update(root, REF, &new, old)?;
    Ok(new)
}

/// Add records to the local store: `records` is id → stored bytes. Records already present
/// are left as they are. Returns the new tip, or the old one when nothing was added.
pub fn add(root: &Path, records: &[(String, Vec<u8>)], message: &str) -> Result<String, String> {
    let old = tip(root, REF);
    let mut files = match &old {
        Some(c) => entries(root, c)?,
        None => BTreeMap::new(),
    };
    let fresh: Vec<&(String, Vec<u8>)> = records
        .iter()
        .filter(|(id, _)| !files.contains_key(&path_of(id)))
        .collect();
    if let (Some(old), true) = (&old, fresh.is_empty()) {
        return Ok(old.clone());
    }
    for (id, bytes) in fresh {
        let blob = run_with_input(root, &["hash-object", "-w", "--stdin"], bytes)?;
        files.insert(path_of(id), blob);
    }
    let parents: Vec<&str> = old.iter().map(String::as_str).collect();
    write_tip(root, &files, &parents, message, old.as_deref())
}

/// Merge the store at `other` (a commit) into the local ref: the local files and those of
/// `other`'s files that `keep` admits, as a commit with both as parents. Returns the new tip
/// and the names whose bytes differed. When nothing is left out and one side already
/// contains the other, the ref fast-forwards or stays.
///
/// `keep` is how a record that failed admission — forged, from another repository — stays
/// out of this store's tree: the remote's commit is still a parent, so the next sync with
/// that remote is a fast-forward rather than a second merge, but no reader ever sees what
/// was refused.
pub fn merge(
    root: &Path,
    other: &str,
    keep: &dyn Fn(&str) -> bool,
) -> Result<(String, Vec<Collision>), String> {
    let old = tip(root, REF);
    let theirs = entries(root, other)?;
    if let Some(local) = old
        .as_deref()
        .filter(|l| *l == other || is_ancestor(root, other, l))
    {
        return Ok((local.to_string(), Vec::new()));
    }
    let filtered = theirs.keys().any(|p| !keep(p));
    let fast_forward = old.as_deref().is_none_or(|l| is_ancestor(root, l, other));
    if fast_forward && !filtered {
        update(root, REF, other, old.as_deref())?;
        return Ok((other.to_string(), Vec::new()));
    }
    let mut files = match &old {
        Some(local) => entries(root, local)?,
        None => BTreeMap::new(),
    };
    let mut collisions = Vec::new();
    for (path, blob) in theirs.into_iter().filter(|(p, _)| keep(p)) {
        match files.get(&path) {
            Some(mine) if *mine != blob => collisions.push(Collision { path }),
            Some(_) => {}
            None => {
                files.insert(path, blob);
            }
        }
    }
    let parents: Vec<&str> = old.iter().map(String::as_str).chain([other]).collect();
    let new = write_tip(
        root,
        &files,
        &parents,
        "majordomus continuity: merge\n",
        old.as_deref(),
    )?;
    Ok((new, collisions))
}

/// Is `ancestor` reachable from `descendant`?
pub fn is_ancestor(root: &Path, ancestor: &str, descendant: &str) -> bool {
    git(root)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// What asking a remote produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// The remote holds a store; it is now at [`remote_ref`].
    Store(String),
    /// The remote answered and holds no store yet.
    Empty,
}

/// Fetch `remote`'s store into [`remote_ref`]. The only network operation besides
/// [`push`], and only `sync` calls either.
pub fn fetch(root: &Path, remote: &str) -> Result<Fetched, String> {
    if !valid_remote(remote) {
        return Err(format!("`{remote}` is not a remote name"));
    }
    let listed = run(root, &["ls-remote", "--refs", remote, REF])?;
    if listed.is_empty() {
        return Ok(Fetched::Empty);
    }
    let target = remote_ref(remote);
    run(
        root,
        &[
            "fetch",
            "--quiet",
            "--no-tags",
            "--no-write-fetch-head",
            remote,
            &format!("+{REF}:{target}"),
        ],
    )?;
    tip(root, &target)
        .map(Fetched::Store)
        .ok_or_else(|| format!("the fetch of {remote} left no {target}"))
}

/// Push the local store to `remote`, never forced: a remote that moved since the fetch
/// refuses, and the caller fetches, merges and pushes again.
pub fn push(root: &Path, remote: &str) -> Result<(), String> {
    if !valid_remote(remote) {
        return Err(format!("`{remote}` is not a remote name"));
    }
    run(root, &["push", "--quiet", remote, &format!("{REF}:{REF}")])?;
    // what the remote now holds, recorded so that the next status needs no network
    let _ = run(root, &["update-ref", &remote_ref(remote), REF]);
    Ok(())
}

/// The remote `sync` uses when none is named: the current branch's, else `origin` when it
/// exists, else none.
pub fn default_remote(root: &Path) -> Option<String> {
    let branch = run(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok();
    if let Some(branch) = branch.filter(|b| !b.is_empty()) {
        if let Ok(remote) = run(root, &["config", &format!("branch.{branch}.remote")]) {
            if valid_remote(&remote) && remote != "." {
                return Some(remote);
            }
        }
    }
    let remotes = run(root, &["remote"]).unwrap_or_default();
    remotes
        .lines()
        .find(|r| *r == "origin")
        .map(str::to_string)
        .or_else(|| remotes.lines().next().map(str::to_string))
        .filter(|r| valid_remote(r))
}

/// Stores as another writer — or a hand — could have left them, for the tests of this
/// module and of the operations that read a store.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;

    /// A commit holding the tree `mktree -z` builds from `lines`, each `<mode> <type>
    /// <object>\t<name>`.
    pub(crate) fn planted(root: &Path, lines: &[String]) -> String {
        let listing: String = lines.iter().map(|l| format!("{l}\0")).collect();
        let tree = run_with_input(root, &["mktree", "-z"], listing.as_bytes()).unwrap();
        commit(root, &tree, &[], "planted\n").unwrap()
    }

    pub(crate) fn blob(root: &Path, bytes: &[u8]) -> String {
        run_with_input(root, &["hash-object", "-w", "--stdin"], bytes).unwrap()
    }

    /// A store of `count` files that are no records: that many names for one blob.
    pub(crate) fn filled(root: &Path, count: usize) -> String {
        let one = blob(root, b"x\n");
        let listing: String = (0..count)
            .map(|i| format!("100644 blob {one}\t{i:032x}.json\0"))
            .collect();
        let tree = run_with_input(root, &["mktree", "-z"], listing.as_bytes()).unwrap();
        planted(root, &[format!("040000 tree {tree}\trecords")])
    }

    /// A store one file over the bound.
    pub(crate) fn oversized(root: &Path) -> String {
        filled(root, MAX_RECORDS + 1)
    }

    /// A store whose tree is deeper than a store is: `a/b/c`.
    pub(crate) fn deep(root: &Path) -> String {
        let one = blob(root, b"x\n");
        let tree = |line: String| {
            run_with_input(root, &["mktree", "-z"], format!("{line}\0").as_bytes()).unwrap()
        };
        let sub = tree(format!("100644 blob {one}\tc"));
        let mid = tree(format!("040000 tree {sub}\tb"));
        planted(root, &[format!("040000 tree {mid}\ta")])
    }

    /// Hold the lock git takes on `reference`, as a writer in the middle of moving it does.
    pub(crate) fn locked(root: &Path, reference: &str) -> std::path::PathBuf {
        let lock = root.join(".git").join(format!("{reference}.lock"));
        std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
        std::fs::write(&lock, "").unwrap();
        lock
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::{blob, deep, locked, oversized, planted};
    use super::*;

    #[test]
    fn only_a_plain_remote_name_reaches_a_refspec() {
        assert!(valid_remote("origin"));
        assert!(!valid_remote("../x"));
        assert!(!valid_remote("a:b"));
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success();
        assert!(ok);
        dir
    }

    #[test]
    fn adding_writes_the_ref_and_touches_no_branch() {
        let dir = repo();
        let root = dir.path();
        assert!(tip(root, REF).is_none());
        let id = "a".repeat(32);
        let first = add(root, &[(id.clone(), b"{}\n".to_vec())], "one\n").unwrap();
        assert_eq!(tip(root, REF), Some(first.clone()));
        let files = read(root, REF).unwrap();
        assert_eq!(files.get(&path_of(&id)).unwrap(), b"{}\n");
        // HEAD is still unborn: nothing was committed to a branch
        assert!(run(root, &["rev-parse", "--verify", "HEAD"]).is_err());
        // adding the same record again is no new commit
        assert_eq!(
            add(root, &[(id, b"{}\n".to_vec())], "again\n").unwrap(),
            first
        );
    }

    #[test]
    fn merging_two_stores_is_their_union_and_a_collision_is_reported() {
        let dir = repo();
        let root = dir.path();
        let base = add(root, &[("a".repeat(32), b"A\n".to_vec())], "a\n").unwrap();
        let left = add(root, &[("b".repeat(32), b"B\n".to_vec())], "b\n").unwrap();
        // a second line of history from the same base, as another machine would write it
        update(root, REF, &base, Some(&left)).unwrap();
        let right = add(
            root,
            &[
                ("c".repeat(32), b"C\n".to_vec()),
                ("b".repeat(32), b"not B\n".to_vec()),
            ],
            "c\n",
        )
        .unwrap();
        update(root, REF, &left, Some(&right)).unwrap();
        let (merged, collisions) = merge(root, &right, &|_| true).unwrap();
        let files = read(root, &merged).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(
            files.get(&path_of(&"b".repeat(32))).unwrap(),
            b"B\n",
            "local kept"
        );
        assert_eq!(
            collisions,
            vec![Collision {
                path: path_of(&"b".repeat(32))
            }]
        );
        // merging what is already contained is a no-op
        assert_eq!(merge(root, &right, &|_| true).unwrap().0, merged);
    }

    #[test]
    fn a_refused_file_stays_out_of_the_merged_tree() {
        let dir = repo();
        let root = dir.path();
        let base = add(root, &[("a".repeat(32), b"A\n".to_vec())], "a\n").unwrap();
        let theirs = add(root, &[("f".repeat(32), b"forged\n".to_vec())], "f\n").unwrap();
        update(root, REF, &base, Some(&theirs)).unwrap();
        let forged = path_of(&"f".repeat(32));
        let (merged, _) = merge(root, &theirs, &|p| p != forged).unwrap();
        let files = read(root, &merged).unwrap();
        assert!(
            !files.contains_key(&forged),
            "what admission refused is not merged"
        );
        assert!(
            is_ancestor(root, &theirs, &merged),
            "the remote's commit is still a parent"
        );
    }

    #[test]
    fn a_git_that_cannot_start_refuses_or_stops_reading_is_an_error_of_its_own() {
        let dir = repo();
        let root = dir.path();
        let err = run(&root.join("gone"), &["status"]).unwrap_err();
        assert!(err.starts_with("cannot run git: "), "{err}");
        let err = run(root, &["rev-parse", "--verify", "nope"]).unwrap_err();
        assert!(err.starts_with("git rev-parse --verify nope: "), "{err}");
        // a git that exits without reading what it was given: the write is what fails
        let err = run_with_input(root, &["--version"], &vec![b'x'; 8 << 20]).unwrap_err();
        assert!(err.starts_with("git --version: "), "{err}");
    }

    #[test]
    fn blobs_are_read_in_one_batch_and_an_answer_cut_short_is_refused() {
        let dir = repo();
        let root = dir.path();
        assert!(blobs(root, &[]).unwrap().is_empty());
        let small = blob(root, b"small\n");
        let huge = blob(root, &vec![b'x'; MAX_RECORD_BYTES * 2 + 1]);
        let absent = "0".repeat(40);
        let got = blobs(root, &[&small, &absent, &huge]).unwrap();
        assert_eq!(got.get(&small).unwrap(), b"small\n");
        assert!(!got.contains_key(&absent), "a missing object is left out");
        assert!(
            got.get(&huge).unwrap().is_empty(),
            "a blob no record could be is not held"
        );
        // a header with no end, and a header promising more bytes than arrived
        // a place git cannot read objects from is an error, not an empty answer
        assert!(blobs(Path::new("/nonexistent-majordomus-store"), &["abc"]).is_err());
        assert!(parse_batch(b"abc blob 3").is_err());
        assert!(parse_batch(b"abc blob 30\nxyz\n").is_err());
        assert_eq!(
            parse_batch(b"abc blob 3\nxyz\n")
                .unwrap()
                .get("abc")
                .unwrap(),
            b"xyz"
        );
    }

    #[test]
    fn only_blobs_are_files_of_a_store_and_the_bound_is_enforced() {
        let dir = repo();
        let root = dir.path();
        let one = blob(root, b"x\n");
        let other = planted(root, &[format!("100644 blob {one}\tnote")]);
        // a gitlink in the tree is not a file, and is passed over
        let store = planted(
            root,
            &[
                format!("100644 blob {one}\tnote"),
                format!("160000 commit {other}\tsubmodule"),
            ],
        );
        let files = entries(root, &store).unwrap();
        assert_eq!(files.keys().collect::<Vec<_>>(), ["note"]);
        assert!(entries(root, "nope").is_err());

        let big = oversized(root);
        let err = entries(root, &big).unwrap_err();
        assert!(err.contains("more than 10000 files"), "{err}");
        // every writer reads the store first, so none writes into one it cannot read
        update(root, REF, &big, None).unwrap();
        assert!(read(root, REF).is_err());
        assert!(add(root, &[("a".repeat(32), b"A\n".to_vec())], "a\n").is_err());
        let unrelated = planted(root, &[format!("100644 blob {one}\tnote")]);
        assert!(merge(root, &unrelated, &|_| true).is_err());
    }

    #[test]
    fn a_tree_is_one_level_deep_and_names_only_objects_the_repository_has() {
        let dir = repo();
        let root = dir.path();
        let one = blob(root, b"x\n");
        let mut files = BTreeMap::new();
        files.insert("README".to_string(), one.clone());
        files.insert(path_of(&"a".repeat(32)), one.clone());
        let tree = tree_of(root, &files).unwrap();
        let listed = run(root, &["ls-tree", "-r", "--name-only", &tree]).unwrap();
        assert_eq!(
            listed.lines().collect::<Vec<_>>(),
            ["README", &path_of(&"a".repeat(32))]
        );

        files.insert("a/b/c".to_string(), one.clone());
        let err = tree_of(root, &files).unwrap_err();
        assert!(err.contains("one level of directories"), "{err}");

        let mut absent = BTreeMap::new();
        absent.insert(path_of(&"b".repeat(32)), "0".repeat(40));
        assert!(tree_of(root, &absent).is_err());
        assert!(commit(root, &tree, &["nope"], "x\n").is_err());
    }

    /// Each step of a write can be refused by git, and each refusal leaves the ref where
    /// it was.
    #[test]
    fn a_write_git_refuses_leaves_the_store_where_it_was() {
        let dir = repo();
        let root = dir.path();
        let record = |c: &str| (c.repeat(32), format!("{c}\n").into_bytes());
        let first = add(root, &[record("a")], "a\n").unwrap();

        // another writer holds the ref
        let lock = locked(root, REF);
        assert!(add(root, &[record("b")], "b\n").is_err());
        std::fs::remove_file(&lock).unwrap();
        assert_eq!(tip(root, REF), Some(first.clone()));

        // an identity git will not commit as
        run(root, &["config", "user.email", "t@t"]).unwrap();
        run(root, &["config", "user.name", ""]).unwrap();
        let err = add(root, &[record("c")], "c\n").unwrap_err();
        assert!(err.starts_with("git commit-tree"), "{err}");
        run(root, &["config", "--unset", "user.name"]).unwrap();
        run(root, &["config", "--unset", "user.email"]).unwrap();
        assert_eq!(tip(root, REF), Some(first.clone()));
    }

    /// An object database that cannot be written refuses the record itself.
    #[cfg(unix)]
    #[test]
    fn a_record_that_cannot_be_stored_is_not_added() {
        use std::os::unix::fs::PermissionsExt;
        let dir = repo();
        let root = dir.path();
        let objects = root.join(".git/objects");
        let mode = |m: u32| {
            std::fs::set_permissions(&objects, std::fs::Permissions::from_mode(m)).unwrap()
        };
        mode(0o555);
        let refused = add(root, &[("a".repeat(32), b"A\n".to_vec())], "a\n");
        mode(0o755);
        // whoever may write anywhere (root, in a container) is not refused, and that is
        // not this module's to change
        if let Err(err) = refused {
            assert!(err.starts_with("git hash-object"), "{err}");
            assert!(tip(root, REF).is_none());
        }
    }

    #[test]
    fn a_merge_fast_forwards_takes_loose_files_and_refuses_what_it_cannot_write() {
        let dir = repo();
        let root = dir.path();
        let one = blob(root, b"x\n");
        assert!(merge(root, "nope", &|_| true).is_err());

        // an empty local store takes the other side as it is — unless the ref is held
        let theirs = planted(root, &[format!("100644 blob {one}\tREADME")]);
        let lock = locked(root, REF);
        assert!(merge(root, &theirs, &|_| true).is_err());
        std::fs::remove_file(&lock).unwrap();
        assert_eq!(merge(root, &theirs, &|_| true).unwrap().0, theirs);
        assert_eq!(tip(root, REF), Some(theirs.clone()));

        // unrelated histories: the union, a file at the top level included
        let mine = add(root, &[("a".repeat(32), b"A\n".to_vec())], "a\n").unwrap();
        let unrelated = planted(root, &[format!("100644 blob {one}\tNOTES")]);
        let (merged, collisions) = merge(root, &unrelated, &|_| true).unwrap();
        assert!(collisions.is_empty());
        assert!(is_ancestor(root, &mine, &merged) && is_ancestor(root, &unrelated, &merged));
        let files = read(root, REF).unwrap();
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            ["NOTES", "README", &path_of(&"a".repeat(32))]
        );

        // a tree deeper than a store is cannot be merged, and the ref stays
        let err = merge(root, &deep(root), &|_| true).unwrap_err();
        assert!(err.contains("one level of directories"), "{err}");
        assert_eq!(tip(root, REF), Some(merged));
    }

    #[test]
    fn a_remote_is_asked_by_name_and_answers_with_a_store_or_none() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("here");
        let remote = dir.path().join("remote.git");
        std::fs::create_dir_all(&root).unwrap();
        run(
            dir.path(),
            &["init", "-q", "--bare", remote.to_str().unwrap()],
        )
        .unwrap();
        run(&root, &["init", "-q", "-b", "main"]).unwrap();

        // no remote at all, then one that is not `origin`, then both
        assert_eq!(default_remote(&root), None);
        assert!(fetch(&root, "a:b")
            .unwrap_err()
            .contains("is not a remote name"));
        assert!(push(&root, "a:b")
            .unwrap_err()
            .contains("is not a remote name"));
        run(
            &root,
            &["remote", "add", "upstream", remote.to_str().unwrap()],
        )
        .unwrap();
        assert_eq!(default_remote(&root).as_deref(), Some("upstream"));
        run(
            &root,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        )
        .unwrap();
        assert_eq!(default_remote(&root).as_deref(), Some("origin"));
        // the branch's own remote wins; `.` (a local upstream) is not a remote
        run(&root, &["config", "branch.main.remote", "upstream"]).unwrap();
        assert_eq!(default_remote(&root).as_deref(), Some("upstream"));
        run(&root, &["config", "branch.main.remote", "."]).unwrap();
        assert_eq!(default_remote(&root).as_deref(), Some("origin"));

        assert_eq!(fetch(&root, "origin").unwrap(), Fetched::Empty);
        let first = add(&root, &[("a".repeat(32), b"A\n".to_vec())], "a\n").unwrap();
        push(&root, "origin").unwrap();
        assert_eq!(tip(&root, &remote_ref("origin")), Some(first.clone()));
        assert_eq!(fetch(&root, "origin").unwrap(), Fetched::Store(first));

        // the tracking ref is held by another writer: the fetch is refused, not half done
        run(&root, &["update-ref", "-d", &remote_ref("origin")]).unwrap();
        let lock = locked(&root, &remote_ref("origin"));
        assert!(fetch(&root, "origin").is_err());
        std::fs::remove_file(&lock).unwrap();

        // a remote whose ref is not a commit holds no store this one can read
        let stray = run_with_input(&remote, &["hash-object", "-w", "--stdin"], b"x\n").unwrap();
        run(&remote, &["update-ref", REF, &stray]).unwrap();
        let err = fetch(&root, "origin").unwrap_err();
        assert!(err.contains("left no"), "{err}");

        // detached: no branch to ask, so the remotes decide
        let tree = tree_of(&root, &BTreeMap::new()).unwrap();
        let head = commit(&root, &tree, &[], "base\n").unwrap();
        run(&root, &["checkout", "-q", "--detach", &head]).unwrap();
        assert_eq!(default_remote(&root).as_deref(), Some("origin"));
    }
}
