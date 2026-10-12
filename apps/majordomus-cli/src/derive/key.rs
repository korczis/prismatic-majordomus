//! The key of stage B: one digest over everything `scripts/generate-site-data` can read.
//!
//! # Why it is a superset and not the generator's own list
//!
//! The generator declares a list of inputs (`--inputs`, hashed into `source_hash`). That
//! list is not what it reads: it leaves out the generator's own text, every library it
//! sources but one, its awk and jq programs, `site/content-src/`, the release records, the
//! closed sessions, and the executable its scenarios run. A skip decided by that hash would
//! keep stale output after any of those moved.
//!
//! So the key here does not try to name what the generator reads. It covers everything the
//! generator *could* read from the checkout: every path of the index with its work-tree
//! bytes, every ignored file a glob could still reach, the executable, the tools on `PATH`,
//! the environment, git's own configuration, and the one piece of ignored local state the
//! generator is known to read. An input nobody listed is in it by construction. What that
//! costs is skips, never correctness: any tracked edit outside the two exemptions below
//! moves the key and stage B runs.
//!
//! # What is not hashed by content, and why that is sound
//!
//! - The stage's own outputs. It stages a fresh generation and never reads the previous one;
//!   their bytes are verified separately, against the digests recorded with the key. Their
//!   *membership* stays in the key, because the generator resolves links against
//!   `git ls-files`, and a page that became tracked is a link that now resolves.
//! - [`MEMBERSHIP_ONLY`]: the crate's tests and benches. They reach nothing but `cargo
//!   test`; the generator and the scenarios meet the crate only through the built
//!   executable, whose generation is in the key, and through its sources, which are hashed
//!   like any other tracked file.
//!
//! # What makes a tree uncacheable
//!
//! Some states have no sound key at all, and for those nothing is skipped and nothing is
//! recorded: an untracked file that is not ignored (the generator's globs read it, the index
//! does not hold it, and the commit will not carry it), an unmerged index, a tracked symlink
//! or submodule, a share directory outside the tree, scenarios that would run another
//! executable than the one answering, and a generator that does not declare its outputs.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use crate::git::read_only;

/// The version of the key's own construction. A record made under another version is
/// unreadable by definition: what its digest covered is not what this one covers.
pub(crate) const KEY_SCHEMA: u32 = 1;

/// The node this key belongs to: stage B as a whole, the legacy stage and not yet a split.
pub(crate) const NODE: &str = "shell:site-data";

/// The generator of the node, repository-relative. It is asked for its outputs and its text
/// is the node's operation version.
pub(crate) const GENERATOR: &str = "scripts/generate-site-data";

/// How the generator runs inside a derive. `--no-scenarios` exists and a derive never
/// passes it; were that to change, this string changes with it and no old record matches.
const CONFIGURATION: &str = "scenarios=on";

/// Tracked trees whose bytes stage B cannot reach: only whether each path is there.
///
/// The generator checks that a claim's test file exists and resolves links against the
/// tracked list, so membership matters; it never opens these files. A source file of the
/// crate is not here: the shell tool greps `src/`, so its bytes are inputs like any other.
pub(crate) const MEMBERSHIP_ONLY: &[&str] =
    &["apps/majordomus-cli/tests/", "apps/majordomus-cli/benches/"];

/// The tools stage B spawns from `PATH`, whose behaviour differs between versions and
/// between their BSD and GNU forms.
const TOOLS: &[&str] = &["bash", "jq", "git", "awk", "sed"];

/// Environment variables that are named `MAJORDOMUS_*` or `MJ_*` and are not inputs of the
/// stage: the two the key reads in a canonical form of its own, the derive's own knobs, and
/// the profiler's switch and scratch file.
const ENVIRONMENT_EXEMPT: &[&str] = &[
    "MAJORDOMUS_BIN",
    "MAJORDOMUS_SHARE",
    "MJ_TIMING",
    "MJ_TIMING_FILE",
];

/// Locale and clock variables every tool the stage spawns inherits.
const ENVIRONMENT_EXTRA: &[&str] = &["LC_ALL", "LANG", "LC_COLLATE", "LC_CTYPE", "TZ"];

/// Which executable the scenarios of stage B would run, relative to the one answering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScenarioExecutable {
    /// `MAJORDOMUS_BIN` names this very executable: what `scripts/derive` exports.
    This,
    /// `MAJORDOMUS_BIN` is unset or names another file; the reason says which.
    Other(String),
}

/// Everything outside the checkout that reaches the key, read once by the caller.
///
/// It is a value so that the decision is a function of the tree and of this, and a test can
/// move one field and watch one reason appear.
#[derive(Debug, Clone)]
pub(crate) struct Ambient {
    /// What the answering executable is: its generation, profile and target triple.
    pub(crate) executable: String,
    /// Whether the scenarios would run it.
    pub(crate) scenario_executable: ScenarioExecutable,
    /// One line per tool: where `PATH` finds it, what it says its version is, and the
    /// digest of its file; then the operating system.
    pub(crate) toolchain: String,
    /// The input variables that are set, by name.
    pub(crate) environment: Vec<(String, String)>,
    /// `MAJORDOMUS_SHARE` as it was given, if it was.
    pub(crate) share: Option<PathBuf>,
    /// `git config --global --list` and `--system --list`, as git answered.
    pub(crate) git_config: String,
}

impl Ambient {
    /// Read the process's own surroundings: the executable's compiled-in identity, the
    /// tools `PATH` resolves, the environment and git's configuration.
    pub(crate) fn capture() -> Self {
        let scenario_executable = match std::env::var_os("MAJORDOMUS_BIN") {
            None => ScenarioExecutable::Other(
                "MAJORDOMUS_BIN is unset, so the scenarios would find an executable of their own"
                    .into(),
            ),
            Some(named) => {
                let named = PathBuf::from(named);
                let same = match (named.canonicalize(), std::env::current_exe()) {
                    (Ok(a), Ok(b)) => b.canonicalize().is_ok_and(|b| a == b),
                    _ => false,
                };
                if same {
                    ScenarioExecutable::This
                } else {
                    ScenarioExecutable::Other(
                        "MAJORDOMUS_BIN names another executable than the one deciding".into(),
                    )
                }
            }
        };
        // through a map: byte order by construction, which is what a digest wants
        let environment: Vec<(String, String)> = std::env::vars_os()
            .filter_map(|(k, v)| Some((k.into_string().ok()?, v.to_string_lossy().into_owned())))
            .filter(|(k, _)| is_input_variable(k))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .collect();
        Ambient {
            executable: format!("{} {} {}", crate::GENERATION, crate::PROFILE, crate::TARGET),
            scenario_executable,
            toolchain: toolchain(),
            environment,
            share: std::env::var_os("MAJORDOMUS_SHARE").map(PathBuf::from),
            git_config: format!(
                "{}\n{}",
                spoken(Command::new("git").args(["config", "--global", "--list"])),
                spoken(Command::new("git").args(["config", "--system", "--list"]))
            ),
        }
    }
}

/// Whether a variable of this name is an input of the stage.
fn is_input_variable(name: &str) -> bool {
    if ENVIRONMENT_EXTRA.contains(&name) {
        return true;
    }
    (name.starts_with("MAJORDOMUS_") || name.starts_with("MJ_"))
        && !ENVIRONMENT_EXEMPT.contains(&name)
        && !name.starts_with("MAJORDOMUS_DERIVE_")
}

/// One line per tool of [`TOOLS`], then the operating system.
fn toolchain() -> String {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let mut lines = Vec::new();
    for tool in TOOLS {
        let found = dirs.iter().map(|d| d.join(tool)).find(|p| p.is_file());
        lines.push(match found {
            None => format!("{tool}\tabsent"),
            Some(path) => format!(
                "{tool}\t{}\t{}\t{}",
                path.display(),
                first_line(&spoken(Command::new(&path).arg("--version"))),
                std::fs::read(&path)
                    .map(|b| hex(Sha256::digest(&b).as_slice()))
                    .unwrap_or_else(|_| "unreadable".into())
            ),
        });
    }
    lines.push(format!(
        "os\t{}",
        first_line(&spoken(Command::new("uname").arg("-srm")))
    ));
    lines.join("\n")
}

/// What a command said and how it ended, as one text. A tool that will not report a version
/// still answers the same way every time, and the way it answers changes when it does.
fn spoken(command: &mut Command) -> String {
    match command.stdin(Stdio::null()).output() {
        Ok(out) => format!(
            "exit {}\n{}{}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(e) => format!("could not run: {e}"),
    }
}

/// The second line of [`spoken`]'s text: the first thing the command itself said.
fn first_line(spoken: &str) -> String {
    spoken.lines().nth(1).unwrap_or("").trim().to_string()
}

/// One path of an output declaration: a file, or a tree with everything under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputRoot {
    /// Repository-relative, without a trailing slash.
    pub(crate) path: String,
    /// Whether it is a directory the stage owns whole.
    pub(crate) tree: bool,
}

impl OutputRoot {
    /// Whether `path` is this output or lies inside it.
    pub(crate) fn owns(&self, path: &str) -> bool {
        if self.tree {
            path.strip_prefix(self.path.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        } else {
            path == self.path
        }
    }
}

/// The measurement of one tree: its key, and the lists the key and the output verification
/// were taken over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fingerprint {
    /// The digest of every component.
    pub(crate) key: String,
    /// The digest of every component the stage itself does not write: the key without the
    /// local state. Equal before and after a run means nothing else moved meanwhile.
    pub(crate) settled: String,
    /// Each component's digest by name, so a difference can be named.
    pub(crate) components: BTreeMap<String, String>,
    /// Every input path with its state, sorted by path.
    pub(crate) inputs: Vec<(String, String)>,
    /// Every file on disk under the stage's outputs with its state, sorted by path.
    pub(crate) outputs: Vec<(String, String)>,
}

/// A tree either has a key or has a reason it cannot have one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Measured {
    /// The key and its lists.
    Keyed(Box<Fingerprint>),
    /// No sound key exists for this state; the sentence says why.
    Uncacheable(String),
}

/// Measure the tree at `root` against `ambient`.
///
/// Reads the index, the work tree and the stage's outputs; writes nothing, takes no lock
/// and refreshes no index entry.
pub(crate) fn measure(root: &Path, ambient: &Ambient) -> Measured {
    match fingerprint(root, ambient) {
        Ok(f) => Measured::Keyed(Box::new(f)),
        Err(reason) => Measured::Uncacheable(reason),
    }
}

fn fingerprint(root: &Path, ambient: &Ambient) -> Result<Fingerprint, String> {
    if let ScenarioExecutable::Other(why) = &ambient.scenario_executable {
        return Err(why.clone());
    }
    let share = canonical_share(root, ambient.share.as_deref())?;
    let roots = declared_outputs(root)?;
    let owned = |path: &str| roots.iter().any(|r| r.owns(path));

    let untracked = git_z(root, &["ls-files", "-z", "--others", "--exclude-standard"])?;
    if let Some(first) = untracked.first() {
        return Err(format!(
            "{} untracked file(s) are not ignored (the first: {first}); a derive projects what \
             git tracks, so nothing about this state can be reused",
            untracked.len()
        ));
    }

    // (path, how it is represented): the list the tree digest is taken over
    let mut inputs: Vec<(String, String)> = Vec::new();
    // paths whose bytes are hashed, with where the digest goes
    let mut to_hash: Vec<(String, Slot)> = Vec::new();

    for entry in git_z(root, &["ls-files", "-z", "--stage"])? {
        let (meta, path) = entry
            .split_once('\t')
            .ok_or_else(|| format!("git ls-files --stage said something unreadable: {entry}"))?;
        let mut fields = meta.split(' ');
        let (mode, stage) = (fields.next().unwrap_or(""), fields.nth(1).unwrap_or(""));
        if stage != "0" {
            return Err(format!(
                "{path} is unmerged; the index has no single content for it"
            ));
        }
        if mode != "100644" && mode != "100755" {
            return Err(format!(
                "{path} is tracked with mode {mode} (a symlink or a submodule), which this key \
                 does not model"
            ));
        }
        hashable(path)?;
        if owned(path) {
            inputs.push((path.to_string(), "tracked".into()));
            continue;
        }
        let on_disk = std::fs::symlink_metadata(root.join(path));
        if MEMBERSHIP_ONLY.iter().any(|p| path.starts_with(p)) {
            let state = if on_disk.is_ok() {
                "present"
            } else {
                "missing"
            };
            inputs.push((path.to_string(), state.into()));
            continue;
        }
        match on_disk {
            Err(_) => inputs.push((path.to_string(), "missing".into())),
            Ok(meta) if meta.is_file() => {
                to_hash.push((
                    path.to_string(),
                    Slot::Input(inputs.len(), executable(&meta)),
                ));
                inputs.push((path.to_string(), String::new()));
            }
            Ok(_) => {
                return Err(format!(
                    "{path} is tracked as a file and is not one in the work tree"
                ))
            }
        }
    }

    // What git ignores is still on disk, and a glob of the generator can still reach it.
    // A wholly ignored directory is one entry (its name); an ignored file is hashed.
    for path in git_z(
        root,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ],
    )? {
        if owned(path.trim_end_matches('/')) {
            continue;
        }
        if path.ends_with('/') {
            inputs.push((path, "ignored-directory".into()));
            continue;
        }
        hashable(&path)?;
        match std::fs::symlink_metadata(root.join(&path)) {
            Ok(meta) if meta.is_file() => {
                to_hash.push((path.clone(), Slot::Input(inputs.len(), 'i')));
                inputs.push((path, String::new()));
            }
            _ => inputs.push((path, "ignored-other".into())),
        }
    }

    let mut outputs: Vec<(String, String)> = Vec::new();
    for output in &roots {
        let mut found = Vec::new();
        walk(root, &output.path, output.tree, &mut found);
        for (path, kind) in found {
            match kind {
                Found::File => {
                    hashable(&path)?;
                    to_hash.push((path.clone(), Slot::Output(outputs.len())));
                    outputs.push((path, String::new()));
                }
                Found::Other => outputs.push((path, "not-a-file".into())),
            }
        }
    }

    let paths: Vec<&str> = to_hash.iter().map(|(p, _)| p.as_str()).collect();
    let ids = blob_ids(root, &paths)?;
    for ((_, slot), id) in to_hash.iter().zip(ids) {
        match slot {
            Slot::Input(at, tag) => inputs[*at].1 = format!("{tag}:{id}"),
            Slot::Output(at) => outputs[*at].1 = id,
        }
    }
    // each path once and in byte order, by construction: a digest is taken over these, and
    // a record is compared with them by a merge
    let inputs: Vec<(String, String)> = inputs
        .into_iter()
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect();
    let outputs: Vec<(String, String)> = outputs
        .into_iter()
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect();

    let operation = inputs
        .binary_search_by(|(p, _)| p.as_str().cmp(GENERATOR))
        .map(|at| inputs[at].1.clone())
        .map_err(|_| format!("{GENERATOR} is not a tracked file of this tree"))?;

    let mut components = BTreeMap::new();
    let mut put = |name: &str, parts: &[&str]| {
        components.insert(name.to_string(), digest(parts));
    };
    put("operation", &[NODE, CONFIGURATION, &operation]);
    put("tree", &[&digest_pairs(&inputs)]);
    put("executable", &[&ambient.executable]);
    put("toolchain", &[&ambient.toolchain]);
    put(
        "environment",
        &[&share, &digest_pairs(&ambient.environment)],
    );
    put("git_config", &[&ambient.git_config]);
    put("local_state", &[&local_state(root)]);

    let over = |skip: Option<&str>| {
        let mut h = Sha256::new();
        frame(&mut h, b"majordomus/derive-key");
        frame(&mut h, &KEY_SCHEMA.to_le_bytes());
        frame(&mut h, NODE.as_bytes());
        for (name, value) in components.iter().filter(|(n, _)| Some(n.as_str()) != skip) {
            frame(&mut h, name.as_bytes());
            frame(&mut h, value.as_bytes());
        }
        hex(h.finalize().as_slice())
    };
    Ok(Fingerprint {
        key: over(None),
        settled: over(Some("local_state")),
        components,
        inputs,
        outputs,
    })
}

/// Where a blob id belongs once git has computed it.
enum Slot {
    /// An input, with the tag that says how it is on disk: `f` a file, `x` an executable
    /// one, `i` an ignored one.
    Input(usize, char),
    /// An output.
    Output(usize),
}

/// `x` for a file anyone may execute, `f` otherwise: the one bit of a mode git tracks.
fn executable(meta: &std::fs::Metadata) -> char {
    use std::os::unix::fs::PermissionsExt;
    if meta.permissions().mode() & 0o111 != 0 {
        'x'
    } else {
        'f'
    }
}

/// A path `git hash-object --stdin-paths` reads as itself: one line, not C-quoted.
fn hashable(path: &str) -> Result<(), String> {
    if path.contains('\n') || path.starts_with('"') {
        return Err(format!(
            "{path:?} cannot be named on one line, so its bytes cannot be hashed in one pass"
        ));
    }
    Ok(())
}

/// What a walk of an output found at a path.
enum Found {
    /// A regular file.
    File,
    /// A symlink or anything else that is not a file or a directory.
    Other,
}

/// Every entry under `rel`, repository-relative, without following a link. A missing output
/// yields nothing: its absence shows as the absence of its entries.
fn walk(root: &Path, rel: &str, tree: bool, out: &mut Vec<(String, Found)>) {
    let Ok(meta) = std::fs::symlink_metadata(root.join(rel)) else {
        return;
    };
    if meta.is_file() {
        out.push((rel.to_string(), Found::File));
    } else if meta.is_dir() && tree {
        let Ok(entries) = std::fs::read_dir(root.join(rel)) else {
            return;
        };
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                walk(root, &format!("{rel}/{name}"), true, out);
            } else {
                out.push((format!("{rel}/<not UTF-8>"), Found::Other));
            }
        }
    } else {
        out.push((rel.to_string(), Found::Other));
    }
}

/// The outputs the generator says it owns: `scripts/generate-site-data --outputs`, one
/// repository-relative path per line, a trailing slash for a tree.
///
/// Asked of the generator rather than written here, because the generator is where a new
/// output is added and a second list would be behind it the same day.
pub(crate) fn declared_outputs(root: &Path) -> Result<Vec<OutputRoot>, String> {
    let generator = root.join(GENERATOR);
    let out = Command::new(&generator)
        .arg("--outputs")
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{GENERATOR} --outputs could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{GENERATOR} --outputs exited {}: the stage does not declare what it owns",
            out.status.code().unwrap_or(-1)
        ));
    }
    let mut roots = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let path = line.trim_end_matches('/');
        let inside = !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)));
        if !inside {
            return Err(format!(
                "{GENERATOR} --outputs names {line:?}, which is not a path inside the tree"
            ));
        }
        roots.push(OutputRoot {
            path: path.to_string(),
            tree: line.ends_with('/'),
        });
    }
    if roots.is_empty() {
        return Err(format!("{GENERATOR} --outputs names no output"));
    }
    Ok(roots)
}

/// `MAJORDOMUS_SHARE` as a path relative to the root, which is the only form that means the
/// same thing in every checkout. A share outside the tree is content the key does not hold.
fn canonical_share(root: &Path, share: Option<&Path>) -> Result<String, String> {
    let Some(share) = share else {
        return Ok("unset".into());
    };
    let outside = || {
        format!(
            "MAJORDOMUS_SHARE names {}, which is outside this tree, so what the stage would \
             read from it is not in the key",
            share.display()
        )
    };
    let (Some(root), Some(resolved)) = (resolved(root), resolved(share)) else {
        return Err(outside());
    };
    resolved
        .strip_prefix(&root)
        .map(|rel| format!("tree:{}", rel.display()))
        .map_err(|_| outside())
}

/// A path with its nearest existing ancestor resolved and the rest appended as written: the
/// same answer for `/var/x` and `/private/var/x`, including for a directory that does not
/// exist (a tree with no `share/` is still a tree the derive names one inside).
fn resolved(path: &Path) -> Option<PathBuf> {
    let mut rest = Vec::new();
    let mut at = path;
    loop {
        if let Ok(real) = at.canonicalize() {
            return Some(rest.iter().rev().fold(real, |p, c| p.join(c)));
        }
        rest.push(at.file_name()?.to_os_string());
        at = at.parent()?;
    }
}

/// The ignored local state stage B reads into a committed file: for every recorded
/// use-case run, its name and whether it says it passed. `usecase coverage` counts exactly
/// that, including for a use case this run does not execute.
fn local_state(root: &Path) -> String {
    let Ok(repository) = crate::Repository::open(root) else {
        return "no layer".into();
    };
    let dir = repository
        .ai_dir()
        .join(&repository.manifest().local.path)
        .join("evidence/use-cases");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return "none".into();
    };
    let seen: Vec<(String, String)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.ends_with(".json").then(|| {
                let passed = std::fs::read_to_string(e.path())
                    .is_ok_and(|t| t.contains("\"result\":\"pass\""));
                (name, passed.to_string())
            })
        })
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect();
    digest_pairs(&seen)
}

/// The NUL-separated answer of a read-only git command, as UTF-8 entries.
fn git_z(root: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let out = read_only(root)
        .args(args)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    out.stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|raw| {
            String::from_utf8(raw.to_vec())
                .map_err(|_| format!("a path is not UTF-8: {}", String::from_utf8_lossy(raw)))
        })
        .collect()
}

/// The blob id of each path's work-tree bytes, in order, from one git process.
///
/// `--no-filters`: the raw bytes on disk, which is what the generator reads, and never what
/// a clean filter would store. One process for the whole tree, because the alternative is
/// one per file or a pure-Rust hash an unoptimised build pays seconds for.
fn blob_ids(root: &Path, paths: &[&str]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = read_only(root)
        .args(["hash-object", "--no-filters", "--stdin-paths"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git hash-object could not run: {e}"))?;
    // written from a thread: git answers as it reads, and a pipe filled in one direction
    // while nobody drains the other is a deadlock
    let mut stdin = child.stdin.take().ok_or("git hash-object has no stdin")?;
    let listing = paths.join("\n") + "\n";
    let writer = std::thread::spawn(move || stdin.write_all(listing.as_bytes()));
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git hash-object: {e}"))?;
    let _ = writer.join();
    let ids: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    if !out.status.success() || ids.len() != paths.len() {
        return Err(format!(
            "git hash-object answered for {} of {} file(s): {}",
            ids.len(),
            paths.len(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(ids)
}

/// Length-framed, so that no two different sequences of fields share a digest.
fn frame(h: &mut Sha256, bytes: &[u8]) {
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}

/// The digest of a sequence of texts.
fn digest(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for part in parts {
        frame(&mut h, part.as_bytes());
    }
    hex(h.finalize().as_slice())
}

/// The digest of a list of pairs, in the order given.
pub(crate) fn digest_pairs(pairs: &[(String, String)]) -> String {
    let mut h = Sha256::new();
    for (a, b) in pairs {
        frame(&mut h, a.as_bytes());
        frame(&mut h, b.as_bytes());
    }
    hex(h.finalize().as_slice())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
