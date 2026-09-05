//! The repository scope: what a worker reads of the repository, and what it never reads.
//!
//! Declared once, as data: `.ai/repo/scope.yaml` when the manifest names a `scope`
//! section, otherwise the distribution's default, the same file `majordomus init` seeds
//! (`share/skeleton/ai/repo/scope.yaml`). `in` is the allow-list of pathspecs; `out` wins
//! over `in` and carries a reason: a path never read, a secret, a generated asset, an
//! archive, an image, a video, a PDF, a database dump, a fixture over its limit, a file
//! over the limit, binary content. A path matching nothing is out as `undeclared`.
//!
//! The declaration is read and compiled once at start-up. Discovery drops a source outside
//! it with a diagnostic, so the index, and every projection of the index, holds nothing a
//! worker must not read; `repository.scope_classify` answers for any path, with the rule that decided;
//! `repository.scope` reports the declaration and a tally of every tracked file against it.
//! Nothing here reads a file's content beyond the first [`SNIFF_BYTES`], and the tally
//! judges names and sizes only: content is judged when a file is read.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::discovery::glob::Glob;
use crate::error::{Error, Result};
use crate::model::Diagnostic;
use crate::repository::Repository;
use crate::share::Share;

/// The `version:` of `scope.yaml` this executable reads.
pub const SCOPE_VERSION: u64 = 1;

/// The file, under the tracked half, the manifest's `scope` section names by convention.
pub const SCOPE_FILE: &str = "scope.yaml";

/// The distribution's default declaration, relative to the share directory: the file
/// `majordomus init` seeds, so that a repository declaring no scope is read under the
/// scope it would have been given.
pub const SKELETON_SCOPE: &str = "skeleton/ai/repo/scope.yaml";

/// How many leading bytes decide whether content is text.
pub const SNIFF_BYTES: usize = 8192;

/// The pathspec every tracked file matches, for the tally.
const EVERYTHING: &str = ":(glob)**";

// ---------------------------------------------------------------- the declaration

/// `scope.yaml`, typed. Unknown keys are refused by the type and by the `scope` schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// The format version; only [`SCOPE_VERSION`] is read.
    pub version: u64,
    /// The allow-list: pathspecs anchored at the repository root.
    #[serde(rename = "in")]
    pub r#in: Vec<String>,
    /// What is never read; wins over `in`.
    pub out: Out,
}

/// The `out:` mapping: every category optional, each the reason a path is out.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Out {
    /// Never read, by path: version control, the local half, dependencies, build outputs.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Content that is not text (a NUL byte in the first window) is out, whatever the name says.
    #[serde(default)]
    pub binary: bool,
    /// A file over this many bytes is out, whatever it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
    /// Archives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<Category>,
    /// Images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<Category>,
    /// Video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<Category>,
    /// PDF documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf: Option<Category>,
    /// Database dumps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_dump: Option<Category>,
    /// Generated assets: changed by changing the source and regenerating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated: Option<Category>,
    /// Secrets: never read, never served, never quoted; a tracked one is reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<Category>,
    /// Fixtures are read up to a size; over it they are data, not context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixtures: Option<Fixtures>,
}

/// One category of `out`: pathspecs and file-name patterns.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Category {
    /// Pathspecs anchored at the repository root.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Patterns matched against the file name alone.
    #[serde(default)]
    pub names: Vec<String>,
}

/// The `fixtures` category: where fixtures live and how large one may be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fixtures {
    /// Pathspecs anchored at the repository root.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Patterns matched against the file name alone.
    #[serde(default)]
    pub names: Vec<String>,
    /// A fixture over this many bytes is out.
    pub max_bytes: u64,
}

impl Declaration {
    /// Parse and validate the text of a `scope.yaml`: the version, and every pattern
    /// well-formed (no leading slash, no `:(` prefix, no `..` segment, a name pattern
    /// with no slash).
    ///
    /// ```
    /// use majordomus_cli::scope::Declaration;
    /// use std::path::Path;
    /// let text = "version: 1\nin:\n  - src/**\nout:\n  paths: ['.git/']\n";
    /// let d = Declaration::parse(Path::new("scope.yaml"), text).unwrap();
    /// assert_eq!(d.r#in, vec!["src/**"]);
    /// assert!(Declaration::parse(Path::new("s"), &text.replace("version: 1", "version: 2")).is_err());
    /// assert!(Declaration::parse(Path::new("s"), &text.replace("src/**", "/src/**")).is_err());
    /// ```
    pub fn parse(path: &Path, text: &str) -> Result<Self> {
        let invalid = |reason: String| Error::InvalidScope {
            path: path.to_path_buf(),
            reason,
        };
        let d: Declaration = crate::metadata::yaml::parse_into(text).map_err(invalid)?;
        if d.version != SCOPE_VERSION {
            return Err(invalid(format!(
                "version {} is not {SCOPE_VERSION}",
                d.version
            )));
        }
        for (what, pattern) in d.patterns() {
            if let Err(reason) = check_pattern(what, pattern) {
                return Err(invalid(reason));
            }
        }
        Ok(d)
    }

    /// Every pattern with where it stands, in document order: `in`, then each `out` list.
    fn patterns(&self) -> Vec<(&'static str, &str)> {
        let mut v: Vec<(&'static str, &str)> = Vec::new();
        v.extend(self.r#in.iter().map(|p| ("in", p.as_str())));
        v.extend(self.out.paths.iter().map(|p| ("out.paths", p.as_str())));
        for (reason, c) in self.out.categories() {
            v.extend(
                c.paths
                    .iter()
                    .map(move |p| (reason.paths_key(), p.as_str())),
            );
            v.extend(
                c.names
                    .iter()
                    .map(move |p| (reason.names_key(), p.as_str())),
            );
        }
        if let Some(f) = &self.out.fixtures {
            v.extend(f.paths.iter().map(|p| ("out.fixtures.paths", p.as_str())));
            v.extend(f.names.iter().map(|p| ("out.fixtures.names", p.as_str())));
        }
        v
    }
}

impl Out {
    /// The name-and-path categories, in the order they decide.
    fn categories(&self) -> Vec<(Reason, &Category)> {
        [
            (Reason::Secret, &self.secret),
            (Reason::Generated, &self.generated),
            (Reason::Archive, &self.archive),
            (Reason::Image, &self.image),
            (Reason::Video, &self.video),
            (Reason::Pdf, &self.pdf),
            (Reason::DatabaseDump, &self.database_dump),
        ]
        .into_iter()
        .filter_map(|(r, c)| c.as_ref().map(|c| (r, c)))
        .collect()
    }
}

/// A pattern is well-formed: relative, no pathspec prefix, no parent segment; a name
/// pattern carries no slash.
fn check_pattern(what: &str, pattern: &str) -> std::result::Result<(), String> {
    if pattern.is_empty() {
        return Err(format!("{what}: an empty pattern"));
    }
    if pattern.starts_with('/') || pattern.starts_with(":(") {
        return Err(format!(
            "{what}: '{pattern}' must be relative to the repository root, with no pathspec prefix"
        ));
    }
    if pattern.split('/').any(|s| s == "..") {
        return Err(format!("{what}: '{pattern}' carries a '..' segment"));
    }
    if what.ends_with("names") && pattern.contains('/') {
        return Err(format!(
            "{what}: '{pattern}' is matched against the file name alone and may not carry a slash"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------- reasons and verdicts

/// Why a path is out. The order is the order the rules decide in.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Named under `out.paths`.
    Path,
    /// A secret.
    Secret,
    /// A generated asset.
    Generated,
    /// An archive.
    Archive,
    /// An image.
    Image,
    /// Video.
    Video,
    /// A PDF document.
    Pdf,
    /// A database dump.
    DatabaseDump,
    /// Matches no `in` pathspec.
    Undeclared,
    /// A fixture over `out.fixtures.max_bytes`.
    FixtureOverLimit,
    /// Over `out.max_bytes`.
    OverLimit,
    /// Content with a NUL byte in its first [`SNIFF_BYTES`]: not text.
    Binary,
}

impl Reason {
    /// Every reason, in deciding order.
    pub const ALL: [Reason; 12] = [
        Reason::Path,
        Reason::Secret,
        Reason::Generated,
        Reason::Archive,
        Reason::Image,
        Reason::Video,
        Reason::Pdf,
        Reason::DatabaseDump,
        Reason::Undeclared,
        Reason::FixtureOverLimit,
        Reason::OverLimit,
        Reason::Binary,
    ];

    /// The reason as it is written: `database_dump`, `fixture_over_limit`.
    ///
    /// ```
    /// use majordomus_cli::scope::Reason;
    /// assert_eq!(Reason::DatabaseDump.as_str(), "database_dump");
    /// assert_eq!(Reason::ALL.len(), 12);
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Path => "path",
            Reason::Secret => "secret",
            Reason::Generated => "generated",
            Reason::Archive => "archive",
            Reason::Image => "image",
            Reason::Video => "video",
            Reason::Pdf => "pdf",
            Reason::DatabaseDump => "database_dump",
            Reason::Undeclared => "undeclared",
            Reason::FixtureOverLimit => "fixture_over_limit",
            Reason::OverLimit => "over_limit",
            Reason::Binary => "binary",
        }
    }

    fn paths_key(self) -> &'static str {
        match self {
            Reason::Secret => "out.secret.paths",
            Reason::Generated => "out.generated.paths",
            Reason::Archive => "out.archive.paths",
            Reason::Image => "out.image.paths",
            Reason::Video => "out.video.paths",
            Reason::Pdf => "out.pdf.paths",
            Reason::DatabaseDump => "out.database_dump.paths",
            _ => "out.paths",
        }
    }

    fn names_key(self) -> &'static str {
        match self {
            Reason::Secret => "out.secret.names",
            Reason::Generated => "out.generated.names",
            Reason::Archive => "out.archive.names",
            Reason::Image => "out.image.names",
            Reason::Video => "out.video.names",
            Reason::Pdf => "out.pdf.names",
            Reason::DatabaseDump => "out.database_dump.names",
            _ => "out.names",
        }
    }
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// In or out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Read.
    In,
    /// Never read; the reason says why.
    Out,
}

/// Where the declaration was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// The repository's own, named by the manifest's `scope` section.
    Repository,
    /// The distribution's default, because the repository declares none.
    Distribution,
}

/// One path, judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Classification {
    /// The path, repository-relative, as judged.
    pub path: String,
    /// In or out.
    pub verdict: Verdict,
    /// Why it is out; absent when it is in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    /// The pattern or limit that decided: the `in` pathspec, the `out` pattern, `binary`,
    /// `max_bytes` or `fixtures.max_bytes`; absent for `undeclared`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// Whether the path exists in the work tree; a path that does not is judged by name.
    pub exists: bool,
    /// Whether the path is a directory; a directory is in when something beneath it can be.
    pub directory: bool,
    /// The size, when the path is an existing file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

impl Classification {
    fn out(path: &str, reason: Reason, rule: Option<String>) -> Self {
        Classification {
            path: path.to_string(),
            verdict: Verdict::Out,
            reason: Some(reason),
            rule,
            exists: false,
            directory: false,
            bytes: None,
        }
    }

    fn r#in(path: &str, rule: String) -> Self {
        Classification {
            path: path.to_string(),
            verdict: Verdict::In,
            reason: None,
            rule: Some(rule),
            exists: false,
            directory: false,
            bytes: None,
        }
    }

    /// Is the path in?
    pub fn is_in(&self) -> bool {
        self.verdict == Verdict::In
    }
}

// ---------------------------------------------------------------- the compiled scope

struct Rule {
    reason: Reason,
    text: String,
    glob: Glob,
}

fn compile_pathspec(text: &str) -> Glob {
    if let Some(dir) = text.strip_suffix('/') {
        Glob::new(&format!("{dir}/**"))
    } else {
        Glob::new(text)
    }
}

fn rules<'a>(reason: Reason, patterns: impl Iterator<Item = &'a String>) -> Vec<Rule> {
    patterns
        .map(|p| Rule {
            reason,
            text: p.clone(),
            glob: compile_pathspec(p),
        })
        .collect()
}

fn name_rules<'a>(reason: Reason, patterns: impl Iterator<Item = &'a String>) -> Vec<Rule> {
    patterns
        .map(|p| Rule {
            reason,
            text: p.clone(),
            glob: Glob::new(p),
        })
        .collect()
}

/// The declaration, compiled, with where it came from.
pub struct Scope {
    declaration: Declaration,
    origin: Origin,
    path: String,
    in_rules: Vec<Rule>,
    path_rules: Vec<Rule>,
    name_rules: Vec<Rule>,
    fixture_paths: Vec<Rule>,
    fixture_names: Vec<Rule>,
}

impl std::fmt::Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scope")
            .field("origin", &self.origin)
            .field("path", &self.path)
            .field("in", &self.in_rules.len())
            .field("out", &(self.path_rules.len() + self.name_rules.len()))
            .finish()
    }
}

impl Scope {
    /// Compile a declaration read from `path` (repository-relative, or the share path).
    pub fn compile(declaration: Declaration, origin: Origin, path: &str) -> Self {
        let out = &declaration.out;
        let in_rules = rules(Reason::Undeclared, declaration.r#in.iter());
        let mut path_rules = rules(Reason::Path, out.paths.iter());
        let mut name_rules = Vec::new();
        for (reason, c) in out.categories() {
            path_rules.extend(rules(reason, c.paths.iter()));
            name_rules.extend(name_rules_of(reason, c));
        }
        let (fixture_paths, fixture_names) = match &out.fixtures {
            Some(f) => (
                rules(Reason::FixtureOverLimit, f.paths.iter()),
                name_rules_of(
                    Reason::FixtureOverLimit,
                    &Category {
                        paths: Vec::new(),
                        names: f.names.clone(),
                    },
                ),
            ),
            None => (Vec::new(), Vec::new()),
        };
        Scope {
            declaration,
            origin,
            path: path.to_string(),
            in_rules,
            path_rules,
            name_rules,
            fixture_paths,
            fixture_names,
        }
    }

    /// Read the repository's declaration when its manifest names a `scope` section,
    /// otherwise the distribution's default under the share directory.
    pub fn load(share: &Share, repo: &Repository) -> Result<Self> {
        if let Some(section) = repo.section_path("scope") {
            let abs = repo.root().join(&section);
            let text = std::fs::read_to_string(&abs).map_err(|e| Error::io(&abs, e))?;
            let d = Declaration::parse(&abs, &text)?;
            return Ok(Self::compile(d, Origin::Repository, &section));
        }
        let abs = share.skeleton_scope();
        let text = std::fs::read_to_string(&abs).map_err(|e| Error::InvalidScope {
            path: abs.clone(),
            reason: format!(
                "the distribution's default scope cannot be read ({e}); the manifest of {} names no `scope` section, so the default is what applies",
                repo.root().display()
            ),
        })?;
        let d = Declaration::parse(&abs, &text)?;
        Ok(Self::compile(
            d,
            Origin::Distribution,
            &abs.display().to_string(),
        ))
    }

    /// The declaration as read.
    pub fn declaration(&self) -> &Declaration {
        &self.declaration
    }

    /// Where the declaration came from.
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// The file the declaration was read from: repository-relative for the repository's
    /// own, the share path for the distribution's.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Judge a repository-relative path by its name alone: the `out` paths and names, then
    /// the `in` pathspecs. Size and content are not consulted; [`Scope::classify`] does.
    /// A directory is in when an `in` pathspec can match beneath it.
    ///
    /// ```
    /// use majordomus_cli::scope::{Declaration, Origin, Reason, Scope, Verdict};
    /// use std::path::Path;
    /// let text = "version: 1\nin:\n  - src/**\n  - docs/**\nout:\n  paths: ['.git/', '**/target/']\n  secret:\n    names: ['.env', '*.pem']\n";
    /// let scope = Scope::compile(Declaration::parse(Path::new("s"), text).unwrap(), Origin::Repository, "s");
    /// assert_eq!(scope.classify_name("src/main.rs", false).verdict, Verdict::In);
    /// assert_eq!(scope.classify_name("src/.env", false).reason, Some(Reason::Secret));
    /// assert_eq!(scope.classify_name("src/target/x.rs", false).reason, Some(Reason::Path));
    /// assert_eq!(scope.classify_name("README.md", false).reason, Some(Reason::Undeclared));
    /// assert_eq!(scope.classify_name("docs", true).verdict, Verdict::In);
    /// ```
    pub fn classify_name(&self, path: &str, directory: bool) -> Classification {
        let name = path.rsplit('/').next().unwrap_or(path);
        for r in &self.path_rules {
            if r.glob.matches(path) {
                let mut c = Classification::out(path, r.reason, Some(r.text.clone()));
                c.directory = directory;
                return c;
            }
        }
        if !directory {
            for r in &self.name_rules {
                if r.glob.matches(name) {
                    return Classification::out(path, r.reason, Some(r.text.clone()));
                }
            }
        }
        let admitted = self.in_rules.iter().find(|r| {
            if directory {
                r.glob.could_match_under(path)
            } else {
                r.glob.matches(path)
            }
        });
        let mut c = match admitted {
            Some(r) => Classification::r#in(path, r.text.clone()),
            None => Classification::out(path, Reason::Undeclared, None),
        };
        c.directory = directory;
        c
    }

    /// Is the path a fixture, by path or by name? The rule that says so.
    fn fixture_rule(&self, path: &str) -> Option<&Rule> {
        let name = path.rsplit('/').next().unwrap_or(path);
        self.fixture_paths
            .iter()
            .find(|r| r.glob.matches(path))
            .or_else(|| self.fixture_names.iter().find(|r| r.glob.matches(name)))
    }

    /// Judge a path the name admits by its size: a fixture over its limit, a file over
    /// the limit. `None` when the size changes nothing.
    fn judge_size(&self, path: &str, bytes: u64) -> Option<Classification> {
        if let (Some(f), Some(rule)) = (&self.declaration.out.fixtures, self.fixture_rule(path)) {
            if bytes > f.max_bytes {
                return Some(Classification::out(
                    path,
                    Reason::FixtureOverLimit,
                    Some(format!(
                        "{} over fixtures.max_bytes {}",
                        rule.text, f.max_bytes
                    )),
                ));
            }
        }
        if let Some(max) = self.declaration.out.max_bytes {
            if bytes > max {
                return Some(Classification::out(
                    path,
                    Reason::OverLimit,
                    Some(format!("max_bytes {max}")),
                ));
            }
        }
        None
    }

    /// Judge a repository-relative path by name, then, when it exists as a file the name
    /// admits, by size and by the first [`SNIFF_BYTES`] of its content. A symlink is judged
    /// by name only and reported as not existing: sources are regular files.
    pub fn classify(&self, root: &Path, path: &str) -> Classification {
        let abs = root.join(path);
        let meta = std::fs::symlink_metadata(&abs).ok();
        let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
        let is_file = meta.as_ref().is_some_and(|m| m.is_file());
        let mut c = self.classify_name(path, is_dir);
        c.exists = is_dir || is_file;
        if !is_file {
            return c;
        }
        let bytes = meta.map(|m| m.len()).unwrap_or(0);
        c.bytes = Some(bytes);
        if !c.is_in() {
            return c;
        }
        if let Some(mut out) = self.judge_size(path, bytes) {
            out.exists = true;
            out.bytes = Some(bytes);
            return out;
        }
        if self.declaration.out.binary && !is_text(&abs) {
            let mut out = Classification::out(path, Reason::Binary, Some("binary".into()));
            out.exists = true;
            out.bytes = Some(bytes);
            return out;
        }
        c
    }

    /// Every tracked file judged by name and size, counted; the out ones listed with
    /// their reason. Content is not read: a binary is caught here only by its name.
    pub fn tally(&self, root: &Path, files: &[String]) -> Tally {
        let mut tally = Tally {
            files: files.len(),
            r#in: 0,
            out: 0,
            by_reason: BTreeMap::new(),
            out_files: Vec::new(),
        };
        let sized =
            self.declaration.out.max_bytes.is_some() || self.declaration.out.fixtures.is_some();
        for path in files {
            let mut c = self.classify_name(path, false);
            if c.is_in() && sized {
                if let Ok(meta) = std::fs::symlink_metadata(root.join(path)) {
                    if meta.is_file() {
                        if let Some(out) = self.judge_size(path, meta.len()) {
                            c = out;
                        }
                    }
                }
            }
            match c.reason {
                None => tally.r#in += 1,
                Some(reason) => {
                    tally.out += 1;
                    *tally
                        .by_reason
                        .entry(reason.as_str().to_string())
                        .or_insert(0) += 1;
                    tally.out_files.push(OutFile {
                        path: path.clone(),
                        reason,
                        rule: c.rule,
                    });
                }
            }
        }
        tally
    }

    /// The tracked files of the repository, through `source`, judged: the tally, and a
    /// diagnostic for every tracked secret, which is never read and should not be tracked.
    pub fn tally_repository(
        &self,
        repo: &Repository,
        source: &dyn crate::discovery::DiscoverySource,
    ) -> Result<(Tally, Vec<Diagnostic>)> {
        let mut files = source.enumerate(repo.root(), EVERYTHING)?;
        files.sort();
        files.dedup();
        let tally = self.tally(repo.root(), &files);
        let diagnostics = tally
            .out_files
            .iter()
            .filter(|f| f.reason == Reason::Secret)
            .map(|f| {
                Diagnostic::warning(
                    "tracked_secret",
                    Some(f.path.clone()),
                    format!(
                        "a secret is tracked ({}); it is never read or served, and it should not be in the repository",
                        f.rule.as_deref().unwrap_or("secret")
                    ),
                )
            })
            .collect();
        Ok((tally, diagnostics))
    }
}

fn name_rules_of(reason: Reason, c: &Category) -> Vec<Rule> {
    name_rules(reason, c.names.iter())
}

/// Is the file text? The first [`SNIFF_BYTES`] carry no NUL byte, which is the judgement
/// version control makes too; encoding is the reader's concern, so a Latin-1 file passes
/// here and is refused by name where it is read. An unreadable file is not text.
pub fn is_text(abs: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(abs) else {
        return false;
    };
    let mut buf = vec![0u8; SNIFF_BYTES];
    let mut read = 0;
    while read < buf.len() {
        match f.read(&mut buf[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(_) => return false,
        }
    }
    is_text_bytes(&buf[..read])
}

/// The text judgement over a window of bytes: no NUL byte.
///
/// ```
/// use majordomus_cli::scope::is_text_bytes;
/// assert!(is_text_bytes(b"version: 1\n"));
/// assert!(is_text_bytes("příručka".as_bytes()));
/// assert!(is_text_bytes(b"caf\xe9"), "an encoding is not a binary; the reader refuses it");
/// assert!(!is_text_bytes(b"\x89PNG\r\n\x1a\n\x00"));
/// ```
pub fn is_text_bytes(bytes: &[u8]) -> bool {
    !bytes.contains(&0)
}

// ---------------------------------------------------------------- the tally

/// Every tracked file against the scope, counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Tally {
    /// How many tracked files were judged.
    pub files: usize,
    /// How many are in.
    #[serde(rename = "in")]
    pub r#in: usize,
    /// How many are out.
    pub out: usize,
    /// How many are out for each reason present.
    pub by_reason: BTreeMap<String, usize>,
    /// Every tracked file that is out, with its reason and the rule that decided.
    pub out_files: Vec<OutFile>,
}

/// One tracked file that is out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutFile {
    /// Repository-relative.
    pub path: String,
    /// Why.
    pub reason: Reason,
    /// The rule that decided; absent for `undeclared`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

/// The scope with its tally: what the index was built under.
#[derive(Debug)]
pub struct Scoped {
    /// The compiled declaration.
    pub scope: Scope,
    /// Every tracked file judged.
    pub tally: Tally,
}

impl Default for Scoped {
    /// A scope admitting nothing and a tally of nothing: for an index built by hand in a
    /// test or a doctest, never by the application.
    fn default() -> Self {
        Scoped {
            scope: Scope::compile(
                Declaration {
                    version: SCOPE_VERSION,
                    r#in: Vec::new(),
                    out: Out::default(),
                },
                Origin::Distribution,
                "",
            ),
            tally: Tally {
                files: 0,
                r#in: 0,
                out: 0,
                by_reason: BTreeMap::new(),
                out_files: Vec::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "version: 1
in:
  - .ai/repo/**
  - src/**
  - test/**
  - docs/**
  - README.md
out:
  paths:
    - .git/
    - .ai/local/
    - '**/target/'
    - docs/generated/
  binary: true
  max_bytes: 100
  image:
    names: ['*.png', '*.svg']
  generated:
    paths: [site/data/generated/]
    names: ['*.min.js']
  secret:
    names: ['.env', '.env.*', '*.pem']
  fixtures:
    paths: ['**/fixtures/']
    names: ['*.snap']
    max_bytes: 10
";

    fn scope() -> Scope {
        Scope::compile(
            Declaration::parse(Path::new("s"), TEXT).unwrap(),
            Origin::Repository,
            ".ai/repo/scope.yaml",
        )
    }

    #[test]
    fn out_wins_over_in_and_undeclared_is_out() {
        let s = scope();
        let c = s.classify_name("src/main.rs", false);
        assert!(c.is_in());
        assert_eq!(c.rule.as_deref(), Some("src/**"));
        assert_eq!(
            s.classify_name(".ai/local/state/current.yaml", false)
                .reason,
            Some(Reason::Path)
        );
        assert_eq!(
            s.classify_name("src/target/debug/x", false).reason,
            Some(Reason::Path)
        );
        assert_eq!(
            s.classify_name("docs/generated/openapi.json", false).reason,
            Some(Reason::Path),
            "a category path is a path rule with the category's reason only when listed there"
        );
        assert_eq!(
            s.classify_name("site/data/generated/x.json", false).reason,
            Some(Reason::Generated)
        );
        assert_eq!(
            s.classify_name("src/.env", false).reason,
            Some(Reason::Secret)
        );
        assert_eq!(
            s.classify_name("src/.env.local", false).reason,
            Some(Reason::Secret)
        );
        assert_eq!(
            s.classify_name("docs/logo.png", false).reason,
            Some(Reason::Image)
        );
        assert_eq!(
            s.classify_name("src/app.min.js", false).reason,
            Some(Reason::Generated)
        );
        let c = s.classify_name("Cargo.toml", false);
        assert_eq!(c.reason, Some(Reason::Undeclared));
        assert_eq!(c.rule, None);
    }

    #[test]
    fn directories_are_in_when_something_beneath_can_be() {
        let s = scope();
        assert!(s.classify_name("src", true).is_in());
        assert!(s.classify_name("docs", true).is_in());
        assert_eq!(
            s.classify_name("docs/generated", true).reason,
            Some(Reason::Path)
        );
        assert_eq!(s.classify_name(".git", true).reason, Some(Reason::Path));
        assert_eq!(
            s.classify_name("node_modules", true).reason,
            Some(Reason::Undeclared)
        );
    }

    #[test]
    fn size_and_content_decide_after_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let write = |rel: &str, bytes: &[u8]| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        };
        write("src/small.rs", b"fn main() {}\n");
        write("src/big.rs", &[b'a'; 101]);
        write("src/blob.rs", b"\x00\x01\x02");
        write("test/fixtures/ok.json", b"{}");
        write("test/fixtures/large.json", &[b'x'; 11]);
        write("test/one.snap", &[b'x'; 11]);
        let s = scope();
        let c = s.classify(root, "src/small.rs");
        assert!(c.is_in() && c.exists && c.bytes == Some(13));
        assert_eq!(
            s.classify(root, "src/big.rs").reason,
            Some(Reason::OverLimit)
        );
        assert_eq!(s.classify(root, "src/blob.rs").reason, Some(Reason::Binary));
        assert!(s.classify(root, "test/fixtures/ok.json").is_in());
        let c = s.classify(root, "test/fixtures/large.json");
        assert_eq!(c.reason, Some(Reason::FixtureOverLimit));
        assert!(c.rule.unwrap().contains("fixtures.max_bytes 10"));
        assert_eq!(
            s.classify(root, "test/one.snap").reason,
            Some(Reason::FixtureOverLimit)
        );
        let c = s.classify(root, "src/absent.rs");
        assert!(c.is_in() && !c.exists && c.bytes.is_none());
        let c = s.classify(root, "src");
        assert!(c.is_in() && c.exists && c.directory);
    }

    #[test]
    fn tally_counts_names_and_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/big.rs"), vec![b'a'; 101]).unwrap();
        let files = vec![
            "src/a.rs".to_string(),
            "src/big.rs".to_string(),
            "docs/x.png".to_string(),
            "src/.env".to_string(),
            "Cargo.toml".to_string(),
        ];
        let s = scope();
        let t = s.tally(root, &files);
        assert_eq!((t.files, t.r#in, t.out), (5, 1, 4));
        assert_eq!(t.by_reason["image"], 1);
        assert_eq!(t.by_reason["secret"], 1);
        assert_eq!(t.by_reason["undeclared"], 1);
        assert_eq!(t.by_reason["over_limit"], 1);
        assert_eq!(t.out_files.len(), 4);
    }

    #[test]
    fn declaration_refuses_bad_shapes() {
        let bad = |t: &str| {
            assert!(
                matches!(
                    Declaration::parse(Path::new("s"), t),
                    Err(Error::InvalidScope { .. })
                ),
                "{t}"
            )
        };
        bad(&TEXT.replace("version: 1", "version: 2"));
        bad(&TEXT.replace("  - src/**", "  - /src/**"));
        bad(&TEXT.replace("  - src/**", "  - ':(glob)src/**'"));
        bad(&TEXT.replace("  - src/**", "  - src/../**"));
        bad(&TEXT.replace("'*.png'", "'img/*.png'"));
        bad(&format!("{TEXT}  colour: red\n"));
        bad("version: 1\nin: []\n");
    }
}
