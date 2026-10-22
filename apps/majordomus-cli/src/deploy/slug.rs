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
use std::io::{BufRead, BufReader, Write};
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
pub(crate) struct Request<'a> {
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
pub(crate) fn assemble(req: &Request<'_>) -> Result<Slug, SlugError> {
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
    // strings and a unit variant: a record that cannot fail to serialize
    let record = serde_json::to_string_pretty(&source).expect("the slug's record serializes");
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
    let absolute = std::path::absolute(out).map_err(|e| {
        SlugError::Refused(format!(
            "the out directory `{}` cannot be made absolute: {e}; name a directory",
            out.display()
        ))
    })?;
    // the nearest ancestor that resolves, and the part below it that does not exist yet; the
    // filesystem's root always resolves, so the fallback is the path as named
    let (existing, mut resolved) = absolute
        .ancestors()
        .find_map(|a| a.canonicalize().ok().map(|c| (a, c)))
        .unwrap_or((absolute.as_path(), absolute.clone()));
    for name in absolute
        .strip_prefix(existing)
        .unwrap_or(Path::new(""))
        .components()
    {
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
    parse_ls_tree(&out.stdout)
}

/// The entries of what `git ls-tree -r -z` printed.
fn parse_ls_tree(stdout: &[u8]) -> Result<Vec<Tracked>, SlugError> {
    let mut tracked = Vec::new();
    for raw in stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
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
        let streamed = stream(&mut reader, out, &blobs);
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

/// Read what `git cat-file --batch` answers for each planned blob, in the order they were
/// asked for, and place each. Answers the bytes placed.
fn stream(
    reader: &mut impl BufRead,
    out: &Path,
    blobs: &[(&String, &Planned, &String)],
) -> Result<u64, SlugError> {
    let mut total = 0u64;
    for (path, planned, oid) in blobs {
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
}

/// Write one file of the slug with its mode.
fn place(out: &Path, rel: &str, content: &[u8], mode: Mode) -> Result<(), SlugError> {
    let path = out.join(rel);
    let io =
        |e: std::io::Error| SlugError::Failed(format!("{} cannot be written: {e}", path.display()));
    // `rel` is never empty (`Plan::add`), so the path always has a parent
    std::fs::create_dir_all(path.parent().unwrap_or(out)).map_err(io)?;
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

/// What the tests of the slug, of `deploy.slug` and of `majordomus deploy` build their
/// repositories from: one deployment object's text, and the object with the provider files it
/// renders written and committed.
#[cfg(test)]
pub(crate) mod testing {
    use std::path::Path;
    use std::process::Command;

    use super::{render, Deployment};

    /// A deployment object `id` with these build inputs, hosted on Gigalixir as `<id>-app`
    /// when `provider` is set.
    pub(crate) fn object_yaml(id: &str, inputs: &[&str], provider: bool) -> String {
        let mut s = format!(
            "schema: deployment/v1\nkind: deployment\nid: {id}\ntitle: Hosted\n\
             application: {id}\nbuild:\n  package: majordomus-cli\n  binary: majordomus\n  \
             profile: release\n  inputs:\n"
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
            s.push_str(&format!(
                "provider:\n  name: gigalixir\n  app: {id}-app\n  stack: gigalixir-24\n  \
                 url: https://{id}-app.gigalixirapp.com\n"
            ));
        }
        s
    }

    /// Run git in `root`, asserting it succeeded, and answer what it printed.
    pub(crate) fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// Make `root` a repository a test can commit in.
    pub(crate) fn init(root: &Path, extra: &[&str]) {
        let mut args = vec!["init", "-q"];
        args.extend_from_slice(extra);
        git(root, &args);
        git(root, &["config", "user.email", "t@example.com"]);
        git(root, &["config", "user.name", "t"]);
        git(root, &["config", "commit.gpgsign", "false"]);
    }

    /// Write a file under `root`, creating its directories.
    pub(crate) fn write(root: &Path, rel: &str, content: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    /// Write the object `text` at `file` and every provider file it renders.
    pub(crate) fn write_object(root: &Path, file: &str, text: &str) {
        write(root, file, text.as_bytes());
        let map = crate::metadata::yaml::parse_mapping(text).unwrap();
        let d = Deployment::from_metadata(file, serde_json::Value::Object(map)).unwrap();
        for a in render::provider_artifacts(&d, file) {
            write(root, &a.path, a.content.as_bytes());
        }
    }

    /// Stage and commit everything.
    pub(crate) fn commit(root: &Path, message: &str) {
        git(root, &["add", "-A"]);
        git(root, &["commit", "-qm", message]);
    }

    /// A synthetic layer that discovers deployments and declares `ids`, each hosted and
    /// shipping `docs`, with the provider files rendered, all committed.
    pub(crate) fn hosted_layer(ids: &[&str]) -> crate::synthetic::SyntheticRepository {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        init(repo.root(), &[]);
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).unwrap();
        text.push_str(
            "  - id: deployment\n    kind: deployment\n    discovery: vcs\n    \
             pathspec: ':(glob).ai/repo/deployments/*.yaml'\n    required: false\n",
        );
        std::fs::write(&sources, text).unwrap();
        for id in ids {
            let file = format!(".ai/repo/deployments/{id}.yaml");
            write_object(repo.root(), &file, &object_yaml(id, &["docs"], true));
        }
        commit(repo.root(), "the layer");
        repo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const OBJECT: &str = ".ai/repo/deployments/hosted.yaml";

    fn yaml(inputs: &[&str], provider: bool) -> String {
        testing::object_yaml("hosted", inputs, provider)
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

    /// An error reads as its message, whatever its class.
    #[test]
    fn a_slug_error_reads_as_its_message() {
        for e in [
            SlugError::Refused("r".into()),
            SlugError::Missing("m".into()),
            SlugError::Failed("f".into()),
        ] {
            let (SlugError::Refused(m) | SlugError::Missing(m) | SlugError::Failed(m)) = &e;
            assert_eq!(&e.to_string(), m);
        }
    }

    fn bytes(b: &[u8]) -> Content {
        Content::Bytes(b.to_vec())
    }

    /// A planned path is relative and stays inside the slug: an empty one, an absolute one
    /// and one that climbs are failures of the assembler, never written.
    #[test]
    fn a_deploy_plan_path_outside_the_slug_is_a_failure() {
        let mut plan = Plan::default();
        for path in ["", "/etc/passwd", "../x", "a/../../x"] {
            let e = plan.add(path, bytes(b"x"), Mode::File, "t").unwrap_err();
            assert!(
                matches!(&e, SlugError::Failed(m) if m.contains("not a relative path")),
                "{path}: {e}"
            );
        }
        assert!(plan.0.is_empty());
    }

    /// A file where another path needs a directory is refused before anything is written.
    #[test]
    fn a_deploy_plan_with_a_file_where_a_directory_is_needed_is_refused() {
        let mut plan = Plan::default();
        plan.add("a", bytes(b"x"), Mode::File, "the first").unwrap();
        plan.add("a/b/c", bytes(b"y"), Mode::File, "the second")
            .unwrap();
        let e = plan.check_prefixes().unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("the first writes the file `a`")),
            "{e}"
        );
    }

    /// A build input that is a file where the slug keeps its own directory is refused.
    #[test]
    fn a_build_input_where_the_slug_needs_a_directory_is_refused() {
        let f = Fixture::new();
        f.write(".majordomus", "a file\n");
        f.object(&["share", ".majordomus"], true);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("needs a directory")),
            "{e}"
        );
        assert!(!out.exists());
    }

    /// The buildpack line, the executable and the record are the slug's own: a build input
    /// that would land on any of them is a collision, refused.
    #[test]
    fn a_build_input_never_takes_the_place_of_the_slugs_own_files() {
        let f = Fixture::new();
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        for own in [BUILDPACKS_FILE, ".majordomus/bin/majordomus", SOURCE_FILE] {
            f.write(own, "mine\n");
            f.object(&["share", own], true);
            let e = assemble(&f.request(&out, &exe)).unwrap_err();
            assert!(
                matches!(&e, SlugError::Refused(m) if m.contains(&format!("would both write `{own}`"))),
                "{own}: {e}"
            );
            f.git(&[
                "rm",
                "-rqf",
                "--ignore-unmatch",
                ".majordomus",
                BUILDPACKS_FILE,
            ]);
        }
        assert!(!out.exists());
    }

    /// The object at the file must be the deployment the request names.
    #[test]
    fn another_deployment_at_the_file_is_refused() {
        let f = Fixture::new();
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let req = Request {
            id: "other",
            ..f.request(&out, &exe)
        };
        let e = assemble(&req).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("not `other`")),
            "{e}"
        );
    }

    /// An object file the commit does not hold is missing; one that is not UTF-8, not YAML
    /// this executable reads, or not a deployment is refused.
    #[test]
    fn an_object_the_commit_does_not_hold_or_cannot_read_is_named() {
        let f = Fixture::new();
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let req = Request {
            file: ".ai/repo/deployments/absent.yaml",
            ..f.request(&out, &exe)
        };
        assert!(matches!(assemble(&req), Err(SlugError::Missing(m)) if m.contains("absent.yaml")),);

        testing::write(f.repo.path(), "bad/latin1.yaml", b"id: \xff\xfe\n");
        testing::write(f.repo.path(), "bad/tab.yaml", b"a:\tb\n");
        testing::write(f.repo.path(), "bad/other.yaml", b"a: b\n");
        f.git(&["add", "-A"]);
        f.git(&["commit", "-qm", "bad objects"]);
        let commit = resolve(f.repo.path(), "HEAD").unwrap();
        for (file, said) in [
            ("bad/latin1.yaml", "not UTF-8"),
            ("bad/tab.yaml", "tab character"),
            ("bad/other.yaml", "bad/other.yaml"),
        ] {
            let e = object_at(f.repo.path(), &commit, file).unwrap_err();
            assert!(
                matches!(&e, SlugError::Refused(m) if m.contains(said)),
                "{file}: {e}"
            );
        }
    }

    /// A symbolic link the commit tracks lands as a link, not as the file it names.
    #[cfg(unix)]
    #[test]
    fn a_tracked_link_lands_in_the_deploy_slug_as_a_link() {
        let f = Fixture::new();
        std::os::unix::fs::symlink("a.txt", f.repo.path().join("share/link")).unwrap();
        f.git(&["add", "-A"]);
        f.git(&["commit", "-qm", "a link"]);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        assemble(&f.request(&out, &exe)).unwrap();
        assert_eq!(
            std::fs::read_link(out.join("share/link")).unwrap(),
            Path::new("a.txt")
        );
    }

    /// A submodule among the inputs is another repository, never shipped.
    #[test]
    fn a_submodule_among_the_inputs_is_refused() {
        let f = Fixture::new();
        let head = testing::git(f.repo.path(), &["rev-parse", "HEAD"]);
        f.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{head},share/sub"),
        ]);
        f.git(&["commit", "-qm", "a submodule"]);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("`share/sub` is a commit")),
            "{e}"
        );
    }

    /// git's plumbing can record a tree entry named `..`; a path through it would write
    /// outside the slug, and is a failure rather than a file.
    #[test]
    fn a_tracked_path_that_climbs_out_of_the_deploy_slug_is_a_failure() {
        let f = Fixture::new();
        let mktree = |text: &str| -> String {
            let mut child = Command::new("git")
                .arg("-C")
                .arg(f.repo.path())
                .arg("mktree")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(text.as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{out:?}");
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        };
        let a = testing::git(f.repo.path(), &["rev-parse", "HEAD:share/a.txt"]);
        let inner = mktree(&format!("100644 blob {a}\tx\n"));
        let share = mktree(&format!(
            "040000 tree {inner}\t..\n100644 blob {a}\ta.txt\n"
        ));
        let root: String = testing::git(f.repo.path(), &["ls-tree", "HEAD"])
            .lines()
            .map(|l| {
                if l.ends_with("\tshare") {
                    format!("040000 tree {share}\tshare\n")
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        let tree = mktree(&root);
        let commit = testing::git(f.repo.path(), &["commit-tree", &tree, "-m", "climbs"]);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let req = Request {
            commit: &commit,
            ..f.request(&out, &exe)
        };
        let e = assemble(&req).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("`share/../x`")),
            "{e}"
        );
        assert!(!out.exists());
    }

    /// A provider file the object renders and the commit does not hold is refused.
    #[test]
    fn a_provider_file_the_commit_does_not_hold_is_refused() {
        let f = Fixture::new();
        f.git(&["rm", "-q", "deploy/gigalixir/Procfile"]);
        f.git(&["commit", "-qm", "no Procfile"]);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("Procfile is not committed")),
            "{e}"
        );
    }

    /// An input git cannot list is git's failure, said with what git said.
    #[test]
    fn an_input_git_cannot_list_is_a_failure() {
        let f = Fixture::new();
        f.object(&["share", "../elsewhere"], true);
        let exe = f.elsewhere.path().join("exe");
        let out = f.elsewhere.path().join("slug");
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("git ls-tree")),
            "{e}"
        );
    }

    /// The loose object `rev` names, removed from the store.
    fn lose(f: &Fixture, rev: &str) {
        let oid = testing::git(f.repo.path(), &["rev-parse", rev]);
        let path = f
            .repo
            .path()
            .join(".git/objects")
            .join(&oid[..2])
            .join(&oid[2..]);
        std::fs::remove_file(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }

    /// A store that lost the provider's tree or one of its files is a failure, and no slug
    /// is written from what remains.
    #[test]
    fn a_damaged_object_store_is_a_failure_not_a_deploy_slug() {
        let exe_of = |f: &Fixture| f.elsewhere.path().join("exe");
        for lost in [
            "HEAD:deploy",
            "HEAD:deploy/gigalixir/Procfile",
            "HEAD:share/a.txt",
        ] {
            let f = Fixture::new();
            lose(&f, lost);
            let out = f.elsewhere.path().join("slug");
            let e = assemble(&f.request(&out, &exe_of(&f))).unwrap_err();
            assert!(matches!(&e, SlugError::Failed(_)), "{lost}: {e}");
        }
    }

    /// A repository of SHA-256 objects names its commits in 64 hex digits, recorded whole.
    #[test]
    fn a_sha256_commit_resolves_in_full() {
        let repo = tempfile::tempdir().unwrap();
        testing::init(repo.path(), &["--object-format=sha256"]);
        testing::write(repo.path(), "a", b"a\n");
        testing::commit(repo.path(), "one");
        let sha = resolve(repo.path(), "HEAD").unwrap();
        assert_eq!(sha.len(), 64);
    }

    /// A git that cannot be started is a failure of every read, said as such.
    #[test]
    fn a_git_that_cannot_start_fails_every_deploy_read() {
        // an argument holding NUL is refused by the process launcher before git runs
        let root = Path::new("no\0where");
        let commit = "0".repeat(40);
        let started = |r: Result<(), SlugError>| {
            assert!(
                matches!(&r, Err(SlugError::Failed(m)) if m.contains("cannot run git")),
                "{r:?}"
            );
        };
        started(resolve(root, "HEAD").map(drop));
        started(object_at(root, &commit, OBJECT).map(drop));
        started(ls_tree(root, &commit, &["share"]).map(drop));
        started(blob(root, &commit).map(drop));
        let out = tempfile::tempdir().unwrap();
        let mut plan = Plan::default();
        plan.add("a", Content::Blob(commit.clone()), Mode::File, "t")
            .unwrap();
        started(write(root, out.path(), &plan).map(drop));
    }

    /// What the commit does not have is git's failure; no path asks for no file.
    #[test]
    fn what_git_does_not_have_is_a_deploy_failure() {
        let f = Fixture::new();
        let absent = "0".repeat(40);
        assert!(matches!(
            ls_tree(f.repo.path(), &absent, &["share"]),
            Err(SlugError::Failed(m)) if m.contains("git ls-tree")
        ));
        assert!(ls_tree(f.repo.path(), "HEAD", &[]).unwrap().is_empty());
        assert!(matches!(
            blob(f.repo.path(), &absent),
            Err(SlugError::Failed(m)) if m.contains("git cat-file blob")
        ));
    }

    /// `git ls-tree` output this assembler does not understand is never guessed at.
    #[test]
    fn ls_tree_output_that_is_not_understood_is_never_guessed_at() {
        let ok = parse_ls_tree(b"100644 blob abc\tshare/a.txt\0\0").unwrap();
        assert_eq!(ok.len(), 1);
        assert_eq!(
            (ok[0].mode.as_str(), ok[0].kind.as_str(), ok[0].oid.as_str()),
            ("100644", "blob", "abc")
        );
        assert_eq!(ok[0].path, "share/a.txt");
        assert!(matches!(
            parse_ls_tree(b"100644 blob abc\tshare/\xff\0"),
            Err(SlugError::Refused(m)) if m.contains("not UTF-8")
        ));
        for said in [&b"no tab here\0"[..], b"100644 blob\tshare/a.txt\0"] {
            assert!(matches!(
                parse_ls_tree(said),
                Err(SlugError::Failed(m)) if m.contains("git ls-tree said")
            ));
        }
    }

    /// The out directory is made absolute against the working directory and resolved
    /// through its nearest existing ancestor; an empty name, an existing file and a root
    /// that cannot be resolved are each refused or failed, and an empty directory is taken.
    #[test]
    fn the_deploy_out_directory_is_resolved_and_checked() {
        let f = Fixture::new();
        let e = outside(f.repo.path(), Path::new("")).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("cannot be made absolute")),
            "{e}"
        );

        let relative = Path::new("no-such-slug-directory/below");
        let resolved = outside(f.repo.path(), relative).unwrap();
        assert!(resolved.is_absolute() && resolved.ends_with(relative));
        assert!(!resolved.exists());

        let file = f.elsewhere.path().join("exe");
        let e = outside(f.repo.path(), &file).unwrap_err();
        assert!(
            matches!(&e, SlugError::Refused(m) if m.contains("not a readable directory")),
            "{e}"
        );

        let empty = f.elsewhere.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        assert_eq!(
            outside(f.repo.path(), &empty).unwrap(),
            empty.canonicalize().unwrap()
        );

        let e = outside(Path::new("no\0where"), &empty).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("cannot be resolved")),
            "{e}"
        );
    }

    /// An out directory that cannot be created fails the slug with what the filesystem said.
    #[test]
    fn an_out_directory_below_a_file_fails_the_deploy_slug() {
        let f = Fixture::new();
        let exe = f.elsewhere.path().join("exe");
        let out = exe.join("slug");
        let e = assemble(&f.request(&out, &exe)).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("cannot be written")),
            "{e}"
        );
    }

    /// git is asked for every blob while it answers: a blob it does not have is a failure,
    /// and the asking stops when git is stopped, however much was left to ask.
    #[test]
    fn a_blob_git_does_not_answer_fails_the_deploy_write() {
        let f = Fixture::new();
        let out = f.elsewhere.path().join("slug");
        let mut plan = Plan::default();
        // far more requests than a pipe holds, so the asking is still blocked when git stops
        for n in 0..20_000 {
            plan.add(
                &format!("f{n:05}"),
                Content::Blob("0".repeat(40)),
                Mode::File,
                "t",
            )
            .unwrap();
        }
        let e = write(f.repo.path(), &out, &plan).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("answered")),
            "{e}"
        );
    }

    /// A slug of bytes alone asks git for nothing; a file where a directory is needed fails
    /// the write, whether its bytes are in hand or streamed from git.
    #[test]
    fn the_deploy_write_places_bytes_and_fails_where_a_path_cannot_be_written() {
        let f = Fixture::new();
        let mut plan = Plan::default();
        plan.add("only", bytes(b"abc"), Mode::File, "t").unwrap();
        let out = f.elsewhere.path().join("bytes");
        assert_eq!(write(Path::new("no\0where"), &out, &plan).unwrap(), (1, 3));
        assert_eq!(std::fs::read(out.join("only")).unwrap(), b"abc");

        let mut plan = Plan::default();
        plan.add("a", bytes(b"x"), Mode::File, "t").unwrap();
        plan.add("a/b", bytes(b"y"), Mode::File, "t").unwrap();
        let e = write(f.repo.path(), &f.elsewhere.path().join("held"), &plan).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("cannot be written")),
            "{e}"
        );

        let a = testing::git(f.repo.path(), &["rev-parse", "HEAD:share/a.txt"]);
        let mut plan = Plan::default();
        plan.add("a", Content::Blob(a.clone()), Mode::File, "t")
            .unwrap();
        plan.add("a/b", Content::Blob(a), Mode::File, "t").unwrap();
        let e = write(f.repo.path(), &f.elsewhere.path().join("streamed"), &plan).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("cannot be written")),
            "{e}"
        );
    }

    /// An answer that is not text, or shorter than the size it announced, is a failure.
    #[test]
    fn a_truncated_or_garbled_answer_fails_the_deploy_stream() {
        let out = tempfile::tempdir().unwrap();
        let oid = "a".repeat(40);
        let path = "f".to_string();
        let planned = Planned {
            content: Content::Blob(oid.clone()),
            mode: Mode::File,
            origin: "t".into(),
        };
        let blobs = [(&path, &planned, &oid)];
        for answer in [
            b"\xff\xfe\n".to_vec(),
            format!("{oid} blob 10\nabc").into_bytes(),
            format!("{oid} missing\n").into_bytes(),
        ] {
            let mut reader = std::io::Cursor::new(answer);
            let e = stream(&mut reader, out.path(), &blobs).unwrap_err();
            assert!(
                matches!(&e, SlugError::Failed(m) if m.contains("git cat-file --batch")),
                "{e}"
            );
        }
        let mut reader = std::io::Cursor::new(format!("{oid} blob 3\nabc\n").into_bytes());
        assert_eq!(stream(&mut reader, out.path(), &blobs).unwrap(), 3);
        assert_eq!(std::fs::read(out.path().join("f")).unwrap(), b"abc");
    }

    /// A file of the slug that cannot be written is refused naming its path, and nothing is
    /// placed beside what blocked it.
    #[test]
    fn a_slug_file_that_cannot_be_placed_names_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("a-file");
        std::fs::write(&blocker, b"x").unwrap();
        let e = place(&blocker, "sub/f", b"abc", Mode::File).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("sub/f cannot be written")),
            "{e}"
        );
        std::fs::create_dir(dir.path().join("a-directory")).unwrap();
        let e = place(dir.path(), "a-directory", b"abc", Mode::File).unwrap_err();
        assert!(
            matches!(&e, SlugError::Failed(m) if m.contains("a-directory cannot be written")),
            "{e}"
        );
    }
}
