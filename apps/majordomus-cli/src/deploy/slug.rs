//! The tree a provider receives, assembled from the deployment object at one commit (ADR
//! 0219, decisions 3 and 4).
//!
//! What is pushed to a provider is never the repository: not its history, not the
//! checkout-local half, not a file the object does not name. It is built from three things
//! only, all read at one commit:
//!
//! - the object's `build.inputs`, except the package directory the executable is compiled
//!   from (`render::runtime_inputs`), read from git rather than from the working tree, so a
//!   file edited and not committed never reaches a provider;
//! - the provider's generated files (`deploy/<provider>/`), moved to the slug's root where
//!   the provider reads them, each held to what the object renders at that commit and the
//!   hooks made executable;
//! - the executable built at that same commit, installed at [`render::slug_executable`], and
//!   the provider's buildpack line ([`render::buildpack`]).
//!
//! Beside them the slug carries one record, [`SOURCE_FILE`], naming the deployment, the
//! commit, the executable's digest and the provider, so a person reading the slug can tell
//! what it was assembled from without asking anyone.
//!
//! The assembler writes only into a directory outside the checkout, absent or empty, and
//! never into the repository: the only process it starts is git, read-only
//! ([`crate::git::read_only`]). Pushing the result is a workflow's work
//! (`.github/workflows/provider.yml`), with a credential this module never sees.
//!
//! ```
//! use majordomus_cli::deploy::slug::{BUILDPACKS_FILE, SOURCE_FILE, SOURCE_SCHEMA};
//!
//! // the record and the buildpack list sit beside the provider files, at fixed places
//! assert_eq!(SOURCE_FILE, ".majordomus/SOURCE.json");
//! assert_eq!(BUILDPACKS_FILE, ".buildpacks");
//! assert!(SOURCE_SCHEMA.ends_with("/v1"));
//! ```

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{render, Deployment, Provider, ProviderName};

/// Where the slug records what it was assembled from, relative to its root: beside the
/// executable, in the directory the slug keeps for itself.
pub const SOURCE_FILE: &str = ".majordomus/SOURCE.json";
/// The format of [`SOURCE_FILE`].
pub const SOURCE_SCHEMA: &str = "majordomus.slug-source/v1";
/// The buildpack list a Heroku-compatible provider reads at the slug's root. It cannot carry
/// a comment, so it is written here from [`render::buildpack`] rather than generated into
/// the repository.
pub const BUILDPACKS_FILE: &str = ".buildpacks";

/// Why a slug was not assembled.
///
/// ```
/// use majordomus_cli::deploy::slug::SlugError;
/// let e = SlugError::Refused("an out directory inside the checkout".into());
/// assert_eq!(e.to_string(), "an out directory inside the checkout");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlugError {
    /// The request is well-formed and declined: an out directory inside the checkout or not
    /// empty, a deployment that names no provider, a provider file the object does not
    /// render, two files that would land on one path.
    Refused(String),
    /// Something the request names does not exist: the commit, the object at that commit,
    /// the executable.
    Missing(String),
    /// git or the filesystem failed under a request that was otherwise sound.
    Failed(String),
}

impl std::fmt::Display for SlugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlugError::Refused(s) | SlugError::Missing(s) | SlugError::Failed(s) => f.write_str(s),
        }
    }
}

/// What to assemble: which object, at which commit, with which executable, into where.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The repository root.
    pub root: &'a Path,
    /// The deployment object's repository-relative file, read at [`Request::commit`].
    pub file: &'a str,
    /// The deployment's `id`; the object read at the commit must carry it.
    pub id: &'a str,
    /// Any revision git resolves to a commit; recorded in full.
    pub commit: &'a str,
    /// The executable built at that commit.
    pub executable: &'a Path,
    /// The directory the slug is written into: outside the checkout, absent or empty.
    pub out: &'a Path,
}

/// The record a slug carries at [`SOURCE_FILE`].
///
/// ```
/// use majordomus_cli::deploy::slug::Source;
/// let s: Source = serde_json::from_str(r#"{"schema":"majordomus.slug-source/v1",
///   "deployment":"majordomus","commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
///   "executable_sha256":"00","provider":"gigalixir"}"#).unwrap();
/// assert_eq!(s.deployment, "majordomus");
/// // a field the record does not have is refused rather than carried along
/// assert!(serde_json::from_str::<Source>(r#"{"schema":"x","deployment":"d","commit":"c",
///   "executable_sha256":"0","provider":"gigalixir","token":"t"}"#).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Always [`SOURCE_SCHEMA`].
    pub schema: String,
    /// The deployment's `id`.
    pub deployment: String,
    /// The full commit every file of the slug was read at.
    pub commit: String,
    /// The hex SHA-256 of the executable the slug carries.
    pub executable_sha256: String,
    /// The provider the slug is for.
    pub provider: ProviderName,
}

/// What was assembled: where, from what, and the provider facts a publisher needs to push
/// it and to ask the deployment what it serves.
///
/// ```
/// use majordomus_cli::deploy::slug::Slug;
/// let json = r#"{"deployment":"majordomus","commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
///   "out":"/tmp/slug","entries":[".buildpacks","Procfile"],"files":2,"bytes":10,
///   "executable":".majordomus/bin/majordomus","executable_sha256":"00",
///   "buildpack":"https://example.org/bp#0","liveness":"/api/v1/live",
///   "provider":{"name":"gigalixir","app":"a-b","stack":"gigalixir-24",
///               "url":"https://a-b.gigalixirapp.com"}}"#;
/// let s: Slug = serde_json::from_str(json).unwrap();
/// assert_eq!(s.provider.app, "a-b");
/// assert_eq!(s.entries.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Slug {
    /// The deployment's `id`.
    pub deployment: String,
    /// The full commit every file was read at.
    pub commit: String,
    /// The directory the slug was written into, absolute.
    pub out: String,
    /// Every top-level entry of the slug, sorted: the whole of what a provider receives at
    /// its root.
    pub entries: Vec<String>,
    /// How many files (and links) were written.
    pub files: usize,
    /// Their total size in bytes.
    pub bytes: u64,
    /// Where the executable sits, relative to the slug's root.
    pub executable: String,
    /// Its hex SHA-256.
    pub executable_sha256: String,
    /// The buildpack line written to [`BUILDPACKS_FILE`].
    pub buildpack: String,
    /// The route the deployment answers its commit on (`health.liveness`).
    pub liveness: String,
    /// The provider block of the object, as read at the commit.
    pub provider: Provider,
}

/// How a file lands in the slug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// An ordinary file, `0644`.
    File,
    /// An executable file, `0755`.
    Executable,
    /// A symbolic link whose target is the content.
    Link,
}

/// Where a file's bytes come from.
#[derive(Debug, Clone)]
enum Content {
    /// A blob of the commit, streamed from git when the slug is written.
    Blob(String),
    /// Bytes already in hand: a provider file, the buildpack line, the record, the executable.
    Bytes(Vec<u8>),
}

/// One path of the slug: its bytes, its mode, and what put it there (for a collision's
/// message).
#[derive(Debug, Clone)]
struct Planned {
    content: Content,
    mode: Mode,
    origin: String,
}

/// Every path of the slug before any is written, so that a collision refuses the whole slug
/// rather than leaving half of one.
#[derive(Debug, Default)]
struct Plan(BTreeMap<String, Planned>);

impl Plan {
    fn add(
        &mut self,
        path: &str,
        content: Content,
        mode: Mode,
        origin: &str,
    ) -> Result<(), SlugError> {
        let safe = !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)));
        if !safe {
            return Err(SlugError::Failed(format!(
                "{origin} would write `{path}`, which is not a relative path inside the slug"
            )));
        }
        if let Some(there) = self.0.get(path) {
            return Err(SlugError::Refused(format!(
                "{origin} and {} would both write `{path}` in the slug; a provider file must not \
                 shadow a build input — rename one of them in the deployment object",
                there.origin
            )));
        }
        self.0.insert(
            path.to_string(),
            Planned {
                content,
                mode,
                origin: origin.to_string(),
            },
        );
        Ok(())
    }

    /// No file may stand where another needs a directory.
    fn check_prefixes(&self) -> Result<(), SlugError> {
        for path in self.0.keys() {
            let mut ancestor = Path::new(path).parent();
            while let Some(dir) = ancestor.filter(|d| !d.as_os_str().is_empty()) {
                if let Some(file) = self.0.get(dir.to_string_lossy().as_ref()) {
                    return Err(SlugError::Refused(format!(
                        "{} writes the file `{}`, where `{path}` needs a directory",
                        file.origin,
                        dir.display()
                    )));
                }
                ancestor = dir.parent();
            }
        }
        Ok(())
    }

    /// The sorted top-level names.
    fn entries(&self) -> Vec<String> {
        let top: std::collections::BTreeSet<&str> = self
            .0
            .keys()
            .map(|p| p.split('/').next().unwrap_or(p))
            .collect();
        top.into_iter().map(String::from).collect()
    }
}

/// Assemble the slug a request names and write it into its out directory.
///
/// Refused, with nothing written, when the out directory is inside the checkout or holds
/// anything, when the object at the commit names no provider, when a build input is not
/// tracked at the commit, when a provider file at the commit is not what the object renders,
/// and when two files would land on one path.
pub fn assemble(req: &Request<'_>) -> Result<Slug, SlugError> {
    let commit = resolve(req.root, req.commit)?;
    let short = &commit[..12];
    let d = object_at(req.root, &commit, req.file)?;
    if d.id != req.id {
        return Err(SlugError::Refused(format!(
            "{} at {short} is the deployment `{}`, not `{}`",
            req.file, d.id, req.id
        )));
    }
    let provider = d.provider.clone().ok_or_else(|| {
        SlugError::Refused(format!(
            "the deployment `{}` names no provider at {short} ({}), so nothing hosts it and \
             there is no tree to assemble; add a `provider` block (ADR 0219)",
            d.id, req.file
        ))
    })?;
    let out = outside(req.root, req.out)?;
    let executable = std::fs::read(req.executable).map_err(|e| {
        SlugError::Missing(format!(
            "the executable {} cannot be read: {e}",
            req.executable.display()
        ))
    })?;
    let executable_sha256 = crate::policy::sha256_bytes_hex(&executable);

    let mut plan = Plan::default();

    // --- the build inputs the running process reads, at the commit
    let inputs: Vec<&str> = render::runtime_inputs(&d)
        .into_iter()
        .map(|p| p.trim_end_matches('/'))
        .collect();
    let tracked = ls_tree(req.root, &commit, &inputs)?;
    for input in &inputs {
        if !tracked.iter().any(|t| under(&t.path, input)) {
            return Err(SlugError::Refused(format!(
                "build.inputs names `{input}`, which {short} does not track; a slug carries \
                 what the commit holds"
            )));
        }
    }
    for t in tracked
        .iter()
        .filter(|t| inputs.iter().any(|i| under(&t.path, i)))
    {
        let mode = match (t.kind.as_str(), t.mode.as_str()) {
            ("blob", "100755") => Mode::Executable,
            ("blob", "120000") => Mode::Link,
            ("blob", _) => Mode::File,
            _ => {
                return Err(SlugError::Refused(format!(
                    "`{}` is a {} at {short}, not a file; a slug carries files, never another \
                     repository",
                    t.path, t.kind
                )))
            }
        };
        plan.add(
            &t.path,
            Content::Blob(t.oid.clone()),
            mode,
            &format!("build.inputs `{}`", t.path),
        )?;
    }

    // --- the provider's files, held to what the object renders, moved to the root
    let dir = render::provider_dir(provider.name);
    let rendered = render::provider_artifacts(&d, req.file);
    let paths: Vec<&str> = rendered.iter().map(|a| a.path.as_str()).collect();
    let committed: BTreeMap<String, Tracked> = ls_tree(req.root, &commit, &paths)?
        .into_iter()
        .map(|t| (t.path.clone(), t))
        .collect();
    for artifact in &rendered {
        let at = committed.get(&artifact.path).ok_or_else(|| {
            SlugError::Refused(format!(
                "{} is not committed at {short}; run `majordomus generate deployment` and \
                 commit what it writes",
                artifact.path
            ))
        })?;
        let bytes = blob(req.root, &at.oid)?;
        if bytes != artifact.content.as_bytes() {
            return Err(SlugError::Refused(format!(
                "{} at {short} is not what {} renders; run `majordomus generate deployment` \
                 and commit what it writes",
                artifact.path, req.file
            )));
        }
        let rel = artifact
            .path
            .strip_prefix(dir)
            .and_then(|p| p.strip_prefix('/'))
            .unwrap_or(&artifact.path);
        // a script names its interpreter on its first line; every other file is data
        let mode = if artifact.content.starts_with("#!") {
            Mode::Executable
        } else {
            Mode::File
        };
        plan.add(rel, Content::Bytes(bytes), mode, &artifact.path)?;
    }

    // --- the buildpack line, the executable and the record
    let buildpack = render::buildpack(provider.name).to_string();
    plan.add(
        BUILDPACKS_FILE,
        Content::Bytes(format!("{buildpack}\n").into_bytes()),
        Mode::File,
        "the provider's buildpack line",
    )?;
    let installed = render::slug_executable(&d);
    plan.add(
        &installed,
        Content::Bytes(executable),
        Mode::Executable,
        &format!("the executable {}", req.executable.display()),
    )?;
    let source = Source {
        schema: SOURCE_SCHEMA.into(),
        deployment: d.id.clone(),
        commit: commit.clone(),
        executable_sha256: executable_sha256.clone(),
        provider: provider.name,
    };
    let record = serde_json::to_string_pretty(&source)
        .map_err(|e| SlugError::Failed(format!("the slug's record cannot be written: {e}")))?;
    plan.add(
        SOURCE_FILE,
        Content::Bytes(format!("{record}\n").into_bytes()),
        Mode::File,
        "the slug's record",
    )?;
    plan.check_prefixes()?;

    let (files, bytes) = write(req.root, &out, &plan)?;
    Ok(Slug {
        deployment: d.id.clone(),
        commit,
        out: out.display().to_string(),
        entries: plan.entries(),
        files,
        bytes,
        executable: installed,
        executable_sha256,
        buildpack,
        liveness: d.health.liveness.as_str().to_string(),
        provider,
    })
}

/// Is `path` the input itself or a file under it?
fn under(path: &str, input: &str) -> bool {
    path == input
        || path
            .strip_prefix(input)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The full commit a revision names.
fn resolve(root: &Path, rev: &str) -> Result<String, SlugError> {
    let out = crate::git::read_only(root)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])
        .output()
        .map_err(|e| SlugError::Failed(format!("cannot run git: {e}")))?;
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let hex = sha.chars().all(|c| c.is_ascii_hexdigit());
    if !out.status.success() || !hex || !(sha.len() == 40 || sha.len() == 64) {
        return Err(SlugError::Missing(format!(
            "`{rev}` does not name a commit this clone has"
        )));
    }
    Ok(sha)
}

/// The deployment object as the commit holds it.
fn object_at(root: &Path, commit: &str, file: &str) -> Result<Deployment, SlugError> {
    let out = crate::git::read_only(root)
        .args(["cat-file", "blob", &format!("{commit}:{file}")])
        .output()
        .map_err(|e| SlugError::Failed(format!("cannot run git: {e}")))?;
    if !out.status.success() {
        return Err(SlugError::Missing(format!(
            "{file} does not exist at {}; the slug is assembled from the object the commit holds",
            &commit[..12]
        )));
    }
    let text = String::from_utf8(out.stdout)
        .map_err(|_| SlugError::Refused(format!("{file} at {} is not UTF-8", &commit[..12])))?;
    let map = crate::metadata::yaml::parse_mapping(&text)
        .map_err(|e| SlugError::Refused(format!("{file} at {}: {e}", &commit[..12])))?;
    Deployment::from_metadata(file, serde_json::Value::Object(map))
        .map_err(|r| SlugError::Refused(r.to_string()))
}

/// The out directory, absolute and checked: outside the checkout, and absent or empty.
fn outside(root: &Path, out: &Path) -> Result<PathBuf, SlugError> {
    if out.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(SlugError::Refused(format!(
            "the out directory {} climbs with `..`; name it directly",
            out.display()
        )));
    }
    let absolute = if out.is_absolute() {
        out.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| SlugError::Failed(format!("the working directory is unreadable: {e}")))?
            .join(out)
    };
    // the nearest ancestor that exists, resolved, and the part that does not exist yet
    let mut existing = absolute.clone();
    let mut rest = Vec::new();
    while !existing.exists() {
        match existing.file_name() {
            Some(name) => rest.push(name.to_os_string()),
            None => break,
        }
        existing.pop();
    }
    let mut resolved = existing.canonicalize().map_err(|e| {
        SlugError::Failed(format!("{} cannot be resolved: {e}", existing.display()))
    })?;
    for name in rest.iter().rev() {
        resolved.push(name);
    }
    let root = root
        .canonicalize()
        .map_err(|e| SlugError::Failed(format!("{} cannot be resolved: {e}", root.display())))?;
    if resolved.starts_with(&root) {
        return Err(SlugError::Refused(format!(
            "the out directory {} is inside the checkout {}; a slug is assembled outside the \
             repository and never in it",
            resolved.display(),
            root.display()
        )));
    }
    if resolved.exists() {
        let mut entries = std::fs::read_dir(&resolved).map_err(|e| {
            SlugError::Refused(format!(
                "the out directory {} is not a readable directory: {e}",
                resolved.display()
            ))
        })?;
        if entries.next().is_some() {
            return Err(SlugError::Refused(format!(
                "the out directory {} is not empty; a slug holds what it was assembled from and \
                 nothing else",
                resolved.display()
            )));
        }
    }
    Ok(resolved)
}

/// One entry of `git ls-tree -r`.
#[derive(Debug, Clone)]
struct Tracked {
    mode: String,
    kind: String,
    oid: String,
    path: String,
}

/// Every file the commit tracks under the paths, recursively. No path is no file: an empty
/// pathspec would list the whole tree.
fn ls_tree(root: &Path, commit: &str, paths: &[&str]) -> Result<Vec<Tracked>, SlugError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let out = crate::git::read_only(root)
        .args(["ls-tree", "-r", "-z", "--full-tree", commit, "--"])
        .args(paths)
        .output()
        .map_err(|e| SlugError::Failed(format!("cannot run git: {e}")))?;
    if !out.status.success() {
        return Err(SlugError::Failed(format!(
            "git ls-tree {commit}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    let mut tracked = Vec::new();
    for raw in out.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let line = std::str::from_utf8(raw).map_err(|_| {
            SlugError::Refused(format!(
                "a tracked path is not UTF-8: {}",
                String::from_utf8_lossy(raw)
            ))
        })?;
        let (meta, path) = line
            .split_once('\t')
            .ok_or_else(|| SlugError::Failed(format!("git ls-tree said `{line}`")))?;
        let mut parts = meta.split(' ');
        let (Some(mode), Some(kind), Some(oid)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(SlugError::Failed(format!("git ls-tree said `{line}`")));
        };
        tracked.push(Tracked {
            mode: mode.into(),
            kind: kind.into(),
            oid: oid.into(),
            path: path.into(),
        });
    }
    Ok(tracked)
}

/// One blob's bytes.
fn blob(root: &Path, oid: &str) -> Result<Vec<u8>, SlugError> {
    let out = crate::git::read_only(root)
        .args(["cat-file", "blob", oid])
        .output()
        .map_err(|e| SlugError::Failed(format!("cannot run git: {e}")))?;
    if !out.status.success() {
        return Err(SlugError::Failed(format!(
            "git cat-file blob {oid}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out.stdout)
}

/// Write the plan: every blob streamed through one `git cat-file --batch`, every other file
/// from the bytes in hand. Answers how many files were written and their total size.
fn write(root: &Path, out: &Path, plan: &Plan) -> Result<(usize, u64), SlugError> {
    let io = |what: &Path, e: std::io::Error| {
        SlugError::Failed(format!("{} cannot be written: {e}", what.display()))
    };
    std::fs::create_dir_all(out).map_err(|e| io(out, e))?;
    let mut bytes = 0u64;
    let blobs: Vec<(&String, &Planned, &String)> = plan
        .0
        .iter()
        .filter_map(|(path, p)| match &p.content {
            Content::Blob(oid) => Some((path, p, oid)),
            Content::Bytes(_) => None,
        })
        .collect();
    if !blobs.is_empty() {
        let oids: Vec<String> = blobs.iter().map(|(_, _, oid)| (*oid).clone()).collect();
        let mut child = crate::git::read_only(root)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| SlugError::Failed(format!("cannot run git: {e}")))?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        // the requests go in on a thread of their own, so a full pipe on either side cannot
        // hold the other: git answers while it is still being asked
        let asking = std::thread::spawn(move || -> std::io::Result<()> {
            for oid in oids {
                writeln!(stdin, "{oid}")?;
            }
            Ok(())
        });
        let mut reader = BufReader::new(child.stdout.take().expect("piped stdout"));
        let streamed = (|| -> Result<u64, SlugError> {
            let mut total = 0u64;
            for (path, planned, oid) in &blobs {
                let mut header = String::new();
                reader
                    .read_line(&mut header)
                    .map_err(|e| SlugError::Failed(format!("git cat-file --batch: {e}")))?;
                let fields: Vec<&str> = header.trim_end().split(' ').collect();
                let size = match fields.as_slice() {
                    [o, "blob", size] if *o == oid.as_str() => size.parse::<usize>().ok(),
                    _ => None,
                }
                .ok_or_else(|| {
                    SlugError::Failed(format!(
                        "git cat-file --batch answered `{}` for {oid}",
                        header.trim_end()
                    ))
                })?;
                let mut content = vec![0u8; size + 1];
                reader
                    .read_exact(&mut content)
                    .map_err(|e| SlugError::Failed(format!("git cat-file --batch: {e}")))?;
                content.truncate(size);
                place(out, path, &content, planned.mode)?;
                total += size as u64;
            }
            Ok(total)
        })();
        drop(reader);
        if streamed.is_err() {
            let _ = child.kill();
        }
        let _ = asking.join();
        let _ = child.wait();
        bytes += streamed?;
    }
    for (path, planned) in &plan.0 {
        if let Content::Bytes(content) = &planned.content {
            place(out, path, content, planned.mode)?;
            bytes += content.len() as u64;
        }
    }
    Ok((plan.0.len(), bytes))
}

/// Write one file of the slug with its mode.
fn place(out: &Path, rel: &str, content: &[u8], mode: Mode) -> Result<(), SlugError> {
    let path = out.join(rel);
    let io =
        |e: std::io::Error| SlugError::Failed(format!("{} cannot be written: {e}", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io)?;
    }
    match mode {
        Mode::Link => link(content, &path).map_err(io),
        Mode::File | Mode::Executable => {
            std::fs::write(&path, content).map_err(io)?;
            permit(&path, mode == Mode::Executable).map_err(io)
        }
    }
}

#[cfg(unix)]
fn link(target: &[u8], path: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(target), path)
}

#[cfg(not(unix))]
fn link(_: &[u8], path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other(format!(
        "{} is a symbolic link, which this platform cannot write",
        path.display()
    )))
}

#[cfg(unix)]
fn permit(path: &Path, executable: bool) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o755 } else { 0o644 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn permit(_: &Path, _: bool) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const OBJECT: &str = ".ai/repo/deployments/hosted.yaml";

    fn yaml(inputs: &[&str], provider: bool) -> String {
        let mut s = String::from(
            "schema: deployment/v1\nkind: deployment\nid: hosted\ntitle: Hosted\n\
             application: hosted\nbuild:\n  package: majordomus-cli\n  binary: majordomus\n  \
             profile: release\n  inputs:\n",
        );
        for i in inputs {
            s.push_str(&format!("    - {i}\n"));
        }
        s.push_str(
            "listen:\n  port: 8080\n  interface: all\nhealth:\n  liveness: /api/v1/live\n  \
             readiness: /api/v1/ready\nresources:\n  cpu_kind: shared\n  cpus: 1\n  \
             memory_mb: 256\nmachines:\n  count: 1\n  min_running: 0\n  autostart: true\n  \
             autostop: true\nregion: fra\n",
        );
        if provider {
            s.push_str(
                "provider:\n  name: gigalixir\n  app: hosted-app\n  stack: gigalixir-24\n  \
                 url: https://hosted-app.gigalixirapp.com\n",
            );
        }
        s
    }

    struct Fixture {
        repo: tempfile::TempDir,
        elsewhere: tempfile::TempDir,
    }

    impl Fixture {
        fn git(&self, args: &[&str]) {
            let out = Command::new("git")
                .arg("-C")
                .arg(self.repo.path())
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {out:?}");
        }

        fn write(&self, rel: &str, content: &str) {
            let p = self.repo.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }

        /// The object and the provider files it renders, committed.
        fn object(&self, inputs: &[&str], provider: bool) {
            let text = yaml(inputs, provider);
            self.write(OBJECT, &text);
            let map = crate::metadata::yaml::parse_mapping(&text).unwrap();
            let d = Deployment::from_metadata(OBJECT, serde_json::Value::Object(map)).unwrap();
            for a in render::provider_artifacts(&d, OBJECT) {
                self.write(&a.path, &a.content);
            }
            self.git(&["add", "-A"]);
            self.git(&["commit", "-qm", "the object"]);
        }

        fn new() -> Fixture {
            let f = Fixture {
                repo: tempfile::tempdir().unwrap(),
                elsewhere: tempfile::tempdir().unwrap(),
            };
            f.git(&["init", "-q"]);
            f.git(&["config", "user.email", "t@example.com"]);
            f.git(&["config", "user.name", "t"]);
            f.git(&["config", "commit.gpgsign", "false"]);
            f.write("share/a.txt", "share\n");
            f.write(".ai/repo/x.md", "layer\n");
            f.write(
                "apps/majordomus-cli/src/lib.rs",
                "// compiled, never shipped\n",
            );
            f.write("README.md", "readme\n");
            f.write("bin/tool", "#!/bin/sh\necho tool\n");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let tool = f.repo.path().join("bin/tool");
                std::fs::set_permissions(tool, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            f.object(
                &["apps/majordomus-cli", "share", ".ai", "README.md", "bin"],
                true,
            );
            std::fs::write(f.elsewhere.path().join("exe"), b"\x7fELF fake").unwrap();
            f
        }

        fn request<'a>(&'a self, out: &'a Path, exe: &'a Path) -> Request<'a> {
            Request {
                root: self.repo.path(),
                file: OBJECT,
                id: "hosted",
                commit: "HEAD",
                executable: exe,
                out,
            }
        }
    }

    #[cfg(unix)]
    fn mode_of(p: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    /// The slug is the object's runtime inputs, the provider files at the root, the buildpack
    /// line, the executable and the record — and nothing else: not the package directory the
    /// executable was compiled from, not the object's own directory beyond what `.ai` holds.
    #[test]
    fn a_slug_holds_the_inputs_the_provider_files_the_executable_and_nothing_else() {
        let f = Fixture::new();
        let out = f.elsewhere.path().join("slug");
        let exe = f.elsewhere.path().join("exe");
        let slug = assemble(&f.request(&out, &exe)).unwrap();
        assert_eq!(
            slug.entries,
            [
                ".ai",
                ".buildpacks",
                ".majordomus",
                "Procfile",
                "README.md",
                "bin",
                "share"
            ]
        );
        assert!(
            !out.join("apps").exists(),
            "the package directory is not shipped"
        );
        assert!(
            !out.join("deploy").exists(),
            "provider files sit at the root"
        );
        assert_eq!(
            std::fs::read_to_string(out.join(BUILDPACKS_FILE)).unwrap(),
            format!("{}\n", render::INLINE_BUILDPACK)
        );
        assert_eq!(
            std::fs::read(out.join(".majordomus/bin/majordomus")).unwrap(),
            b"\x7fELF fake"
        );
        let source: Source =
            serde_json::from_slice(&std::fs::read(out.join(SOURCE_FILE)).unwrap()).unwrap();
        assert_eq!(source.commit.len(), 40);
        assert_eq!(source.commit, slug.commit);
        assert_eq!(source.provider, ProviderName::Gigalixir);
        assert_eq!(
            source.executable_sha256,
            crate::policy::sha256_bytes_hex(b"\x7fELF fake")
        );
        assert_eq!(slug.provider.app, "hosted-app");
        assert_eq!(slug.liveness, "/api/v1/live");
        assert_eq!(slug.files, 12);
        #[cfg(unix)]
        {
            for hook in ["bin/detect", "bin/compile", "bin/release"] {
                assert_eq!(mode_of(&out.join(hook)), 0o755, "{hook}");
            }
            assert_eq!(mode_of(&out.join(".majordomus/bin/majordomus")), 0o755);
            assert_eq!(
                mode_of(&out.join("bin/tool")),
                0o755,
                "a tracked mode is kept"
            );
            assert_eq!(mode_of(&out.join("Procfile")), 0o644);
            assert_eq!(mode_of(&out.join("share/a.txt")), 0o644);
        }
    }

    /// Every file is read at the commit: an uncommitted edit never reaches the slug, and an
    /// older commit gives that commit's tree.
    #[test]
    fn a_slug_is_read_at_its_commit_not_from_the_working_tree() {
        let f = Fixture::new();
        f.write("share/a.txt", "edited, not committed\n");
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("one");
        assemble(&f.request(&out, &exe)).unwrap();
        assert_eq!(
            std::fs::read_to_string(out.join("share/a.txt")).unwrap(),
            "share\n"
        );

        f.git(&["add", "-A"]);
        f.git(&["commit", "-qm", "two"]);
        let out = f.elsewhere.path().join("two");
        let req = Request {
            commit: "HEAD~1",
            ..f.request(&out, &exe)
        };
        let slug = assemble(&req).unwrap();
        assert_eq!(
            std::fs::read_to_string(out.join("share/a.txt")).unwrap(),
            "share\n"
        );
        assert_ne!(slug.commit, resolve(f.repo.path(), "HEAD").unwrap());
    }

    /// The out directory is outside the checkout and empty, or nothing is written.
    #[test]
    fn an_out_directory_inside_the_checkout_or_not_empty_is_refused() {
        let f = Fixture::new();
        let exe = f.elsewhere.path().join("exe");
        for inside in [f.repo.path().join("slug"), f.repo.path().to_path_buf()] {
            let e = assemble(&f.request(&inside, &exe)).unwrap_err();
            assert!(
                matches!(&e, SlugError::Refused(m) if m.contains("inside the checkout")),
                "{e}"
            );
        }
        assert!(!f.repo.path().join("slug").exists());

        let full = f.elsewhere.path().join("full");
        std::fs::create_dir_all(&full).unwrap();
        std::fs::write(full.join("x"), "x").unwrap();
        let e = assemble(&f.request(&full, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("not empty")),
            "{e}"
        );

        let climbing = f.elsewhere.path().join("a/../b");
        let e = assemble(&f.request(&climbing, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("..")),
            "{e}"
        );
    }

    /// A deployment that names no provider has no slug.
    #[test]
    fn a_deployment_without_a_provider_is_refused() {
        let f = Fixture::new();
        f.git(&["rm", "-rq", "deploy"]);
        f.object(&["share"], false);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("names no provider")),
            "{e}"
        );
        assert!(!out.exists());
    }

    /// A provider file the commit holds must be the one the object renders: a hand edit, or
    /// an object changed without regenerating, is refused rather than shipped.
    #[test]
    fn a_provider_file_the_object_does_not_render_is_refused() {
        let f = Fixture::new();
        f.write("deploy/gigalixir/Procfile", "web: ./elsewhere\n");
        f.git(&["commit", "-qam", "a hand edit"]);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("is not what")),
            "{e}"
        );
    }

    /// A build input that would land where a provider file goes is a collision, refused.
    #[test]
    fn a_provider_file_never_shadows_a_build_input() {
        let f = Fixture::new();
        f.write("Procfile", "web: ./mine\n");
        f.object(&["share", "Procfile"], true);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("would both write `Procfile`")),
            "{e}"
        );
    }

    /// An input the commit does not track, a commit the clone does not have, and an
    /// executable that is not there are each said, by name.
    #[test]
    fn what_is_absent_is_named() {
        let f = Fixture::new();
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let req = Request {
            commit: &"0".repeat(40),
            ..f.request(&out, &exe)
        };
        assert!(matches!(assemble(&req), Err(SlugError::Missing(_))));
        let missing = f.elsewhere.path().join("absent");
        assert!(matches!(
            assemble(&f.request(&out, &missing)),
            Err(SlugError::Missing(m)) if m.contains("absent")
        ));
        f.object(&["share", "nowhere"], true);
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("`nowhere`")),
            "{e}"
        );
    }
}
