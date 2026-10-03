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
///
/// ```
/// use majordomus_cli::continuity::store::valid_remote;
/// assert!(valid_remote("origin"));
/// assert!(!valid_remote("../x"));
/// assert!(!valid_remote("a:b"));
/// ```
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
    cmd.arg("-C").arg(root);
    cmd
}

fn run(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = git(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn run_with_input(root: &Path, args: &[&str], input: &[u8]) -> Result<String, String> {
    let mut child = git(root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run git: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("git: no stdin")?
        .write_all(input)
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
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
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        let mut parts = meta.split(' ');
        let (_, kind, object) = (parts.next(), parts.next(), parts.next());
        if kind == Some("blob") {
            if let Some(object) = object {
                out.insert(path.to_string(), object.to_string());
            }
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
    let mut child = git(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run git: {e}"))?;
    let mut request = String::new();
    for id in ids {
        request.push_str(id);
        request.push('\n');
    }
    let mut stdin = child.stdin.take().ok_or("git: no stdin")?;
    let writer = std::thread::spawn(move || stdin.write_all(request.as_bytes()));
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git cat-file: {e}"))?;
    let _ = writer.join();
    if !out.status.success() {
        return Err(format!(
            "git cat-file: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let data = out.stdout;
    let mut result = BTreeMap::new();
    let mut at = 0usize;
    while at < data.len() {
        let Some(nl) = data[at..].iter().position(|b| *b == b'\n') else {
            break;
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
        if size <= MAX_RECORD_BYTES * 2 {
            result.insert(id.to_string(), data[at..at + size].to_vec());
        } else {
            result.insert(id.to_string(), Vec::new());
        }
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
    let files = entries(root, &commit)?;
    let ids: Vec<&str> = files.values().map(String::as_str).collect();
    let bytes = blobs(root, &ids)?;
    Ok(files
        .into_iter()
        .map(|(path, id)| {
            let content = bytes.get(&id).cloned().unwrap_or_default();
            (path, content)
        })
        .collect())
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
    let mut cmd = git(root);
    cmd.args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if run(root, &["config", "user.email"]).is_err() {
        cmd.env("GIT_AUTHOR_NAME", "majordomus")
            .env("GIT_AUTHOR_EMAIL", "majordomus@localhost")
            .env("GIT_COMMITTER_NAME", "majordomus")
            .env("GIT_COMMITTER_EMAIL", "majordomus@localhost");
    }
    let mut child = cmd.spawn().map_err(|e| format!("cannot run git: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("git: no stdin")?
        .write_all(message.as_bytes())
        .map_err(|e| format!("git commit-tree: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git commit-tree: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git commit-tree: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
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

/// Add records to the local store: `records` is id → stored bytes. Records already present
/// are left as they are. Returns the new tip, or the old one when nothing was added.
pub fn add(root: &Path, records: &[(String, Vec<u8>)], message: &str) -> Result<String, String> {
    let old = tip(root, REF);
    let mut files = match &old {
        Some(c) => entries(root, c)?,
        None => BTreeMap::new(),
    };
    let mut added = 0usize;
    for (id, bytes) in records {
        let path = path_of(id);
        if files.contains_key(&path) {
            continue;
        }
        let blob = run_with_input(root, &["hash-object", "-w", "--stdin"], bytes)?;
        files.insert(path, blob);
        added += 1;
    }
    if added == 0 {
        if let Some(old) = old {
            return Ok(old);
        }
    }
    let tree = tree_of(root, &files)?;
    let parents: Vec<&str> = old.iter().map(String::as_str).collect();
    let new = commit(root, &tree, &parents, message)?;
    update(root, REF, &new, old.as_deref())?;
    Ok(new)
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
    let filtered = theirs.keys().any(|p| !keep(p));
    let Some(local) = old.clone() else {
        if !filtered {
            update(root, REF, other, None)?;
            return Ok((other.to_string(), Vec::new()));
        }
        let files: BTreeMap<String, String> = theirs.into_iter().filter(|(p, _)| keep(p)).collect();
        let tree = tree_of(root, &files)?;
        let new = commit(root, &tree, &[other], "majordomus continuity: merge\n")?;
        update(root, REF, &new, None)?;
        return Ok((new, Vec::new()));
    };
    if local == other || is_ancestor(root, other, &local) {
        return Ok((local, Vec::new()));
    }
    if !filtered && is_ancestor(root, &local, other) {
        update(root, REF, other, Some(&local))?;
        return Ok((other.to_string(), Vec::new()));
    }
    let mut files = entries(root, &local)?;
    let mut collisions = Vec::new();
    for (path, blob) in theirs {
        if !keep(&path) {
            continue;
        }
        match files.get(&path) {
            Some(mine) if *mine != blob => collisions.push(Collision { path }),
            Some(_) => {}
            None => {
                files.insert(path, blob);
            }
        }
    }
    let tree = tree_of(root, &files)?;
    let new = commit(
        root,
        &tree,
        &[&local, other],
        "majordomus continuity: merge\n",
    )?;
    update(root, REF, &new, Some(&local))?;
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
    if let Some(local) = tip(root, REF) {
        let _ = run(root, &["update-ref", &remote_ref(remote), &local]);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
