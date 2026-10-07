//! The source pack: the tracked tree as a few text files a language model's file search can
//! index, with nothing in it that is not source.
//!
//! `majordomus archive` (the shell tool) hands a reader a zip. A chat model's project files
//! are indexed one text file at a time and a zip is not text, so a model that is given the
//! archive can unpack it in a sandbox but cannot search it. The pack is the same selection,
//! written as Markdown shards that each stay under a token budget, with an index that
//! orients the reader and a manifest that lets [`verify`] prove, afterwards, that what
//! leaves the machine is what was selected and nothing else.
//!
//! # What is selected
//!
//! The profiles are `share/archive.yaml`'s: one declaration of what a snapshot is for, read
//! by the shell's archive and by this module alike. The file set is the git index and the
//! content is the index's blobs, never the working tree, so an ignored file, a build output,
//! a cache and the local half of the AI layer cannot travel by construction. On top of that
//! a profile that sets `artifacts: drop` refuses, whatever the index holds:
//!
//! - a gitlink (mode `160000`): a nested repository or worktree whose content is not here;
//! - a symbolic link (mode `120000`): its content is a path on the machine that wrote it;
//! - a path matching `artifact_paths` (`target/`, `node_modules/`, a `-wt/` worktree
//!   container, minified bundles, ...): build output that was committed by accident;
//! - content that is not UTF-8 text or holds a NUL byte: a binary whatever its extension.
//!
//! An `include` re-admits what `exclude` or `derived: drop` left out, never one of these:
//! a list that could re-admit a binary is a list that eventually will.
//!
//! # What is refused
//!
//! The plan is a verdict, not only a selection. A single file over the shard budget, more
//! shards than the profile allows, a selected file that names this machine (the checkout's
//! own absolute path or the home directory of the account packing it), and an empty
//! selection are findings, and a pack with a finding is not built.
//!
//! The content is carried exactly as committed and never rewritten. A pack holds nothing
//! the tracked tree does not, and what the tree may hold is decided where it is committed
//! (`scripts/ci/no-machine-paths`, the publication checks). Credential shapes are therefore
//! not redacted here: measured on this repository, the published-text table matches `sk-`
//! inside ordinary words (`task-in-progress-…`), so redacting would hand the reader prose
//! that was never written, and a digest of content that was never committed. The one leak
//! that can arise at packing time and not at commit time is the machine doing the packing,
//! which is what is refused, with no false positive by construction.
//!
//! ```
//! use majordomus_cli::pack::{DropReason, fence_for};
//! assert_eq!(DropReason::Worktree.as_str(), "worktree");
//! // a fence is always longer than the longest run of backticks in what it holds
//! assert_eq!(fence_for("no ticks"), "```");
//! assert_eq!(fence_for("a ```` b"), "`````");
//! ```

mod shard;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::discovery::glob::Glob;
use crate::order::{canonical, OrderKey, Ordered};

pub use shard::{fence_for, verify, PackVerdict};

/// The profiles file of the tool distribution, relative to the share directory.
pub const PROFILES: &str = "archive.yaml";
/// A repository's own profiles, relative to its root. A profile declared there under an id
/// the distribution ships replaces the shipped one whole, as the shell's archive reads it;
/// the default and the two lists are the distribution's.
pub const OVERLAY: &str = ".ai/repo/archive.yaml";
/// The schema word of a pack's manifest.
pub const MANIFEST_SCHEMA: &str = "majordomus.pack/v1";
/// The manifest's file name inside a pack.
pub const MANIFEST: &str = "pack.json";
/// The orientation document's file name inside a pack; it is shard zero.
pub const INDEX: &str = "00-INDEX.md";
/// The share of the token budget a shard is filled to, in percent. The rest absorbs the
/// difference between counting each file's block and counting the shard it lands in.
const FILL_PERCENT: u64 = 95;
/// The paths that name the machine a pack is built on: the checkout itself and the home
/// directory of the account, each with a trailing separator so that a sibling with the
/// same prefix is not one.
///
/// A path has more than one spelling — the one a shell's `$PWD` prints and the one the
/// file system resolves it to, and on macOS `/var` and `/tmp` are `/private/var` and
/// `/private/tmp` — so each is listed in every spelling it has.
pub(crate) fn machine_paths(root: &Path) -> Vec<String> {
    machine_paths_for(root, std::env::var_os("HOME").map(PathBuf::from))
}

/// [`machine_paths`] for the home directory `home`: none, `/`, and a hosted runner's home
/// name no person's machine and add nothing to the checkout.
fn machine_paths_for(root: &Path, home: Option<PathBuf>) -> Vec<String> {
    let mut bases = vec![root.to_path_buf()];
    bases.extend(home.filter(|h| h.as_os_str().len() > 1 && !is_runner_home(h)));
    let mut out = BTreeSet::new();
    for base in bases {
        let resolved = std::fs::canonicalize(&base).unwrap_or_else(|_| base.clone());
        for p in [base, resolved] {
            let s = p.display().to_string();
            if let Some(short) = s.strip_prefix("/private") {
                if short.starts_with('/') {
                    out.insert(format!("{short}/"));
                }
            }
            out.insert(format!("{s}/"));
        }
    }
    out.into_iter().collect()
}

/// Why a tracked file is not in the pack. Every reason but `derived` and `excluded` is
/// final: no `include` of a profile re-admits a link, a gitlink, an artifact or a binary.
///
/// ```
/// use majordomus_cli::pack::DropReason;
/// assert_eq!(DropReason::Binary.as_str(), "binary");
/// let json = serde_json::to_string(&DropReason::Artifact).unwrap();
/// assert_eq!(json, "\"artifact\"");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    /// A gitlink: a nested repository or worktree whose content is not in this index.
    Worktree,
    /// A symbolic link, whose content is a path on the machine that committed it.
    Link,
    /// A path the distribution names as build output, a cache or a worktree container.
    Artifact,
    /// Not text: a NUL byte, content that is not UTF-8, or an extension on the binary list.
    Binary,
    /// Marked `merge=derived` in `.gitattributes`: a projection of a file that is here.
    Derived,
    /// Matched by the profile's `exclude`.
    Excluded,
}

impl DropReason {
    /// The word a report and the manifest use, the same as its serialised form.
    ///
    /// ```
    /// use majordomus_cli::pack::DropReason;
    /// assert_eq!(DropReason::Link.as_str(), "link");
    /// assert_eq!(DropReason::Excluded.as_str(), "excluded");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            DropReason::Worktree => "worktree",
            DropReason::Link => "link",
            DropReason::Artifact => "artifact",
            DropReason::Binary => "binary",
            DropReason::Derived => "derived",
            DropReason::Excluded => "excluded",
        }
    }

    /// Every reason, in the order of their words, which is the order a report lists them in.
    pub(crate) const ALL: [DropReason; 6] = [
        DropReason::Artifact,
        DropReason::Binary,
        DropReason::Derived,
        DropReason::Excluded,
        DropReason::Link,
        DropReason::Worktree,
    ];

    /// What the reason means to a reader of the pack's index.
    pub(crate) fn meaning(self) -> &'static str {
        match self {
            DropReason::Derived => {
                "generated projections of files that are here (`merge=derived` in `.gitattributes`)"
            }
            DropReason::Binary => {
                "not text: a binary extension, a NUL byte or content that is not UTF-8"
            }
            DropReason::Artifact => {
                "build output, caches and worktree containers committed by accident"
            }
            DropReason::Worktree => {
                "gitlinks: nested repositories or worktrees whose content is not in this index"
            }
            DropReason::Link => "symbolic links, whose content is a path on the committing machine",
            DropReason::Excluded => "matched by the profile's exclude list",
        }
    }

    /// Whether an `include` may re-admit a file dropped for this reason.
    fn reincludable(self) -> bool {
        matches!(self, DropReason::Derived | DropReason::Excluded)
    }
}

/// The shard limits of a profile: the reader's own limits on what it can index, the file
/// count of a project and the tokens one file of it may hold.
///
/// ```
/// use majordomus_cli::pack::{Profiles, ShardLimits};
/// let p = Profiles::parse(
///     "default: a\nprofiles:\n  - id: a\n    shards:\n      max_count: 20\n      max_tokens: 900\n",
/// )
/// .unwrap();
/// let want = ShardLimits { max_count: 20, max_tokens: 900 };
/// assert_eq!(p.profile(None).unwrap().shards, Some(want));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShardLimits {
    /// The most files the pack may hold, the index included.
    pub max_count: usize,
    /// The most tokens one file of the pack may hold, counted with `o200k_base`.
    pub max_tokens: u64,
}

/// One profile of `share/archive.yaml`, as far as the pack reads it: what it drops, what it
/// re-admits, what the index says first and the limits it is cut into.
///
/// ```
/// use majordomus_cli::pack::{PackProfile, Profiles};
/// let p = Profiles::parse("default: a\nprofiles:\n  - id: a\n    derived: drop\n").unwrap();
/// let a: &PackProfile = p.profile(Some("a")).unwrap();
/// assert_eq!(a.derived, "drop");
/// assert!(a.shards.is_none(), "a profile without limits cannot be packed");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackProfile {
    /// The profile's id.
    pub id: String,
    /// One line: what the snapshot is for.
    pub title: String,
    /// `drop` leaves out what `.gitattributes` marks `merge=derived`.
    pub derived: String,
    /// `drop` leaves out what is not text.
    pub binary: String,
    /// `drop` leaves out gitlinks, links and the distribution's artifact paths.
    pub artifacts: String,
    /// Globs left out.
    pub exclude: Vec<String>,
    /// Globs re-admitted after `exclude` and `derived`.
    pub include: Vec<String>,
    /// The paragraphs the index opens with.
    pub orientation: Vec<String>,
    /// The shard limits; a profile without them cannot be packed.
    pub shards: Option<ShardLimits>,
}

/// What the distribution declares once for every profile: the default, the binary
/// extensions, the artifact paths, and the profiles themselves.
///
/// ```
/// use majordomus_cli::pack::Profiles;
/// let p = Profiles::parse(
///     "default: a\nbinary_extensions: [\"PNG\"]\nprofiles:\n  - id: a\n  - id: b\n",
/// )
/// .unwrap();
/// assert_eq!(p.default, "a");
/// assert!(p.binary_extensions.contains("png"), "extensions are compared in lower case");
/// assert_eq!(p.profiles.len(), 2);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profiles {
    /// The profile used when none is named.
    pub default: String,
    /// Extensions, without the dot, that are binary whatever their content.
    pub binary_extensions: BTreeSet<String>,
    /// Globs of build output, caches and worktree containers.
    pub artifact_paths: Vec<String>,
    /// Every profile, in file order.
    pub profiles: Vec<PackProfile>,
}

#[derive(Deserialize)]
struct RawLimits {
    max_count: usize,
    max_tokens: u64,
}

#[derive(Deserialize)]
struct RawProfile {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    derived: String,
    #[serde(default)]
    binary: String,
    #[serde(default)]
    artifacts: String,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    orientation: Vec<String>,
    #[serde(default)]
    shards: Option<RawLimits>,
}

#[derive(Deserialize)]
struct RawOverlay {
    #[serde(default)]
    profiles: Vec<RawProfile>,
}

#[derive(Deserialize)]
struct RawProfiles {
    default: String,
    #[serde(default)]
    binary_extensions: Vec<String>,
    #[serde(default)]
    artifact_paths: Vec<String>,
    profiles: Vec<RawProfile>,
}

impl From<RawProfile> for PackProfile {
    fn from(p: RawProfile) -> Self {
        PackProfile {
            id: p.id,
            title: p.title,
            derived: p.derived,
            binary: p.binary,
            artifacts: p.artifacts,
            exclude: p.exclude,
            include: p.include,
            orientation: p.orientation,
            shards: p.shards.map(|s| ShardLimits {
                max_count: s.max_count,
                max_tokens: s.max_tokens,
            }),
        }
    }
}

impl Profiles {
    /// Read the profiles from the share directory, then the repository's overlay
    /// (`.ai/repo/archive.yaml`), whose profiles replace the shipped ones of the same id.
    ///
    /// ```
    /// use majordomus_cli::pack::Profiles;
    /// let dir = tempfile::tempdir().unwrap();
    /// let (share, root) = (dir.path().join("share"), dir.path().join("repo"));
    /// std::fs::create_dir_all(&share).unwrap();
    /// std::fs::create_dir_all(root.join(".ai/repo")).unwrap();
    /// std::fs::write(share.join("archive.yaml"), "default: a\nprofiles:\n  - id: a\n").unwrap();
    /// std::fs::write(
    ///     root.join(".ai/repo/archive.yaml"),
    ///     "profiles:\n  - id: a\n    title: ours\n",
    /// )
    /// .unwrap();
    /// let p = Profiles::load(&share, &root).unwrap();
    /// assert_eq!(p.profile(None).unwrap().title, "ours");
    /// assert!(Profiles::load(&root, &root).is_err(), "no archive.yaml there");
    /// ```
    pub fn load(share: &Path, root: &Path) -> Result<Profiles, String> {
        let path = share.join(PROFILES);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut profiles =
            Profiles::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let overlay = root.join(OVERLAY);
        if let Ok(text) = std::fs::read_to_string(&overlay) {
            let raw: RawOverlay =
                crate::metadata::yaml::parse_into(&text).map_err(|e| format!("{OVERLAY}: {e}"))?;
            profiles.overlay(raw.profiles.into_iter().map(PackProfile::from).collect());
        }
        Ok(profiles)
    }

    /// Put a repository's profiles over the shipped ones: the same id replaces, whole.
    pub(crate) fn overlay(&mut self, local: Vec<PackProfile>) {
        for p in local.into_iter().rev() {
            self.profiles.retain(|q| q.id != p.id);
            self.profiles.insert(0, p);
        }
    }

    /// Parse the text of a profiles file, without the overlay: the `default`, the two lists
    /// and the profiles, with extensions lowered so that `PNG` and `png` are one.
    ///
    /// ```
    /// use majordomus_cli::pack::Profiles;
    /// let p = Profiles::parse("default: a\nprofiles:\n  - id: a\n    exclude: [\"x/**\"]\n").unwrap();
    /// assert_eq!(p.profile(None).unwrap().id, "a");
    /// assert!(p.profile(Some("b")).is_err());
    /// ```
    pub fn parse(text: &str) -> Result<Profiles, String> {
        let raw: RawProfiles = crate::metadata::yaml::parse_into(text)?;
        let profiles = raw.profiles.into_iter().map(PackProfile::from).collect();
        Ok(Profiles {
            default: raw.default,
            binary_extensions: raw
                .binary_extensions
                .into_iter()
                .map(|e| e.to_ascii_lowercase())
                .collect(),
            artifact_paths: raw.artifact_paths,
            profiles,
        })
    }

    /// The named profile, or the default one when no name is given; an unknown name is
    /// an error that lists the profiles there are.
    ///
    /// ```
    /// use majordomus_cli::pack::Profiles;
    /// let p = Profiles::parse("default: a\nprofiles:\n  - id: a\n  - id: b\n").unwrap();
    /// assert_eq!(p.profile(Some("b")).unwrap().id, "b");
    /// let e = p.profile(Some("c")).unwrap_err();
    /// assert!(e.contains("a, b"), "{e}");
    /// ```
    pub fn profile(&self, id: Option<&str>) -> Result<&PackProfile, String> {
        let id = id.unwrap_or(&self.default);
        self.profiles.iter().find(|p| p.id == id).ok_or_else(|| {
            let known: Vec<&str> = self.profiles.iter().map(|p| p.id.as_str()).collect();
            format!("no profile '{id}'; the profiles are {}", known.join(", "))
        })
    }

    /// Is `path` one of the distribution's artifact paths: build output, a cache or a
    /// worktree container that was committed by accident?
    ///
    /// ```
    /// use majordomus_cli::pack::Profiles;
    /// let p = Profiles::parse(
    ///     "default: a\nartifact_paths: [\"**/target/**\"]\nprofiles:\n  - id: a\n",
    /// )
    /// .unwrap();
    /// assert!(p.is_artifact("apps/x/target/debug/y.rs"));
    /// assert!(!p.is_artifact("apps/x/src/target.rs"));
    /// ```
    pub fn is_artifact(&self, path: &str) -> bool {
        self.artifact_paths
            .iter()
            .any(|g| Glob::new(g).matches(path))
    }

    /// Does `path` carry an extension on the binary list? The extension is the file name's
    /// last, compared in lower case; a dot file such as `.png` has none.
    ///
    /// ```
    /// use majordomus_cli::pack::Profiles;
    /// let p = Profiles::parse("default: a\nbinary_extensions: [png]\nprofiles:\n  - id: a\n")
    ///     .unwrap();
    /// assert!(p.has_binary_extension("site/logo.PNG"));
    /// assert!(!p.has_binary_extension("dir.png/readme.md"));
    /// assert!(!p.has_binary_extension(".png"));
    /// ```
    pub fn has_binary_extension(&self, path: &str) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => {
                self.binary_extensions.contains(&ext.to_ascii_lowercase())
            }
            _ => false,
        }
    }
}

/// Is this content text the pack can carry? UTF-8 and free of NUL bytes.
///
/// ```
/// use majordomus_cli::pack::is_text;
/// assert!(is_text(b"fn main() {}\n"));
/// assert!(!is_text(b"PNG\0\x01"));
/// assert!(!is_text(&[0xff, 0xfe, 0x41]));
/// ```
pub fn is_text(content: &[u8]) -> bool {
    !content.contains(&0) && std::str::from_utf8(content).is_ok()
}

/// One entry of the git index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexEntry {
    /// The mode, as git prints it (`100644`, `100755`, `120000`, `160000`).
    pub mode: String,
    /// The blob or commit id.
    pub oid: String,
    /// The path, relative to the repository root.
    pub path: String,
}

/// A file the pack carries, with its content.
#[derive(Debug, Clone)]
pub(crate) struct Selected {
    /// The index entry.
    pub entry: IndexEntry,
    /// The content of the index's blob.
    pub text: String,
    /// The tokens of the block that carries it in a shard.
    pub tokens: u64,
}

/// One planned shard: a Markdown file of the pack, the paths it carries from first to last,
/// and what it weighs in bytes and in tokens.
///
/// The examples below plan a repository holding `main.rs` and a `logo.png` whose content is
/// binary, with a distribution whose `chat` profile has limits and whose `bare` one has none.
///
/// ```
/// use majordomus_cli::pack::{plan, ShardPlan};
/// # use std::process::Command;
/// # let dir = tempfile::tempdir().unwrap();
/// # let (root, share) = (dir.path().join("repo"), dir.path().join("share"));
/// # std::fs::create_dir_all(&root).unwrap();
/// # std::fs::create_dir_all(&share).unwrap();
/// # std::fs::write(share.join("archive.yaml"), "default: chat\nprofiles:\n  - id: chat\n    binary: drop\n    artifacts: drop\n    shards:\n      max_count: 5\n      max_tokens: 4000\n  - id: bare\n    binary: drop\n").unwrap();
/// # let git = |a: &[&str]| assert!(Command::new("git").arg("-C").arg(&root)
/// #     .args(["-c", "user.email=t@example.com", "-c", "user.name=t"]).args(a)
/// #     .status().unwrap().success());
/// # git(&["init", "-q"]);
/// # std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
/// # std::fs::write(root.join("logo.png"), b"\x89PNG\0\x01").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-q", "-m", "one"]);
/// let planned = plan(&root, &share, None);
/// let first: &ShardPlan = &planned.plan.shards[0];
/// assert_eq!((first.files, first.first.as_str(), first.last.as_str()), (1, "main.rs", "main.rs"));
/// assert!(first.file.ends_with(".md") && first.tokens > 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ShardPlan {
    /// The file name inside the pack.
    pub file: String,
    /// How many files it carries.
    pub files: usize,
    /// The first path it carries.
    pub first: String,
    /// The last path it carries.
    pub last: String,
    /// The bytes of the content it carries.
    pub bytes: u64,
    /// The tokens of the blocks it carries.
    pub tokens: u64,
}

/// One dropped file the report names. Derived files are counted, not listed: they are a
/// function of files that are in the pack.
///
/// The examples below plan a repository holding `main.rs` and a `logo.png` whose content is
/// binary, with a distribution whose `chat` profile has limits and whose `bare` one has none.
///
/// ```
/// use majordomus_cli::pack::{plan, DropReason, DroppedFile};
/// # use std::process::Command;
/// # let dir = tempfile::tempdir().unwrap();
/// # let (root, share) = (dir.path().join("repo"), dir.path().join("share"));
/// # std::fs::create_dir_all(&root).unwrap();
/// # std::fs::create_dir_all(&share).unwrap();
/// # std::fs::write(share.join("archive.yaml"), "default: chat\nprofiles:\n  - id: chat\n    binary: drop\n    artifacts: drop\n    shards:\n      max_count: 5\n      max_tokens: 4000\n  - id: bare\n    binary: drop\n").unwrap();
/// # let git = |a: &[&str]| assert!(Command::new("git").arg("-C").arg(&root)
/// #     .args(["-c", "user.email=t@example.com", "-c", "user.name=t"]).args(a)
/// #     .status().unwrap().success());
/// # git(&["init", "-q"]);
/// # std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
/// # std::fs::write(root.join("logo.png"), b"\x89PNG\0\x01").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-q", "-m", "one"]);
/// let dropped = plan(&root, &share, None).plan.dropped_files;
/// let want = DroppedFile { path: "logo.png".into(), reason: DropReason::Binary };
/// assert_eq!(dropped, [want]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DroppedFile {
    /// The path.
    pub path: String,
    /// Why.
    pub reason: DropReason,
}

/// A dropped file is listed by its path; the path is unique in the index, so it is both the
/// label and the identity.
impl Ordered for DroppedFile {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.path, &self.path)
    }
}

/// A reason the pack cannot be built as planned, or a written pack is not what its
/// manifest says: a stable code, the path when there is one, and the remedy.
///
/// The examples below plan a repository holding `main.rs` and a `logo.png` whose content is
/// binary, with a distribution whose `chat` profile has limits and whose `bare` one has none.
///
/// ```
/// use majordomus_cli::pack::{plan, PackFinding};
/// # use std::process::Command;
/// # let dir = tempfile::tempdir().unwrap();
/// # let (root, share) = (dir.path().join("repo"), dir.path().join("share"));
/// # std::fs::create_dir_all(&root).unwrap();
/// # std::fs::create_dir_all(&share).unwrap();
/// # std::fs::write(share.join("archive.yaml"), "default: chat\nprofiles:\n  - id: chat\n    binary: drop\n    artifacts: drop\n    shards:\n      max_count: 5\n      max_tokens: 4000\n  - id: bare\n    binary: drop\n").unwrap();
/// # let git = |a: &[&str]| assert!(Command::new("git").arg("-C").arg(&root)
/// #     .args(["-c", "user.email=t@example.com", "-c", "user.name=t"]).args(a)
/// #     .status().unwrap().success());
/// # git(&["init", "-q"]);
/// # std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
/// # std::fs::write(root.join("logo.png"), b"\x89PNG\0\x01").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-q", "-m", "one"]);
/// let findings: Vec<PackFinding> = plan(&root, &share, Some("bare")).plan.findings;
/// assert_eq!(findings[0].code, "pack.no_limits");
/// assert!(findings[0].path.is_none() && !findings[0].remedy.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackFinding {
    /// A stable code: `pack.file_too_large`, `pack.too_many_shards`, `pack.index_too_large`, `pack.leak`,
    /// `pack.empty`, `pack.no_limits`, and from verification `pack.tampered`,
    /// `pack.not_text`, `pack.forbidden_path`, `pack.missing`, `pack.stray`,
    /// `pack.over_budget`, `pack.manifest`.
    pub code: String,
    /// The path the finding is about, when it is about one.
    pub path: Option<String>,
    /// What is wrong.
    pub message: String,
    /// What settles it.
    pub remedy: String,
}

impl PackFinding {
    fn new(code: &str, path: Option<&str>, message: String, remedy: &str) -> Self {
        PackFinding {
            code: code.into(),
            path: path.map(str::to_string),
            message,
            remedy: remedy.into(),
        }
    }
}

/// The plan of one pack: what it carries, what it leaves out and why, how it is cut into
/// shards, and whether it may be built.
///
/// The examples below plan a repository holding `main.rs` and a `logo.png` whose content is
/// binary, with a distribution whose `chat` profile has limits and whose `bare` one has none.
///
/// ```
/// use majordomus_cli::pack::{plan, PackPlan};
/// # use std::process::Command;
/// # let dir = tempfile::tempdir().unwrap();
/// # let (root, share) = (dir.path().join("repo"), dir.path().join("share"));
/// # std::fs::create_dir_all(&root).unwrap();
/// # std::fs::create_dir_all(&share).unwrap();
/// # std::fs::write(share.join("archive.yaml"), "default: chat\nprofiles:\n  - id: chat\n    binary: drop\n    artifacts: drop\n    shards:\n      max_count: 5\n      max_tokens: 4000\n  - id: bare\n    binary: drop\n").unwrap();
/// # let git = |a: &[&str]| assert!(Command::new("git").arg("-C").arg(&root)
/// #     .args(["-c", "user.email=t@example.com", "-c", "user.name=t"]).args(a)
/// #     .status().unwrap().success());
/// # git(&["init", "-q"]);
/// # std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
/// # std::fs::write(root.join("logo.png"), b"\x89PNG\0\x01").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-q", "-m", "one"]);
/// let p: PackPlan = plan(&root, &share, None).plan;
/// assert!(p.measured && p.passes && p.index_matches_head);
/// assert_eq!((p.tracked, p.selected), (2, 1));
/// assert_eq!(p.dropped["binary"], 1);
/// // an unknown profile is not measured, and not measured is never a pass
/// let p: PackPlan = plan(&root, &share, Some("nope")).plan;
/// assert!(!p.measured && !p.passes);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackPlan {
    /// Whether the tree could be read at all. `false` is not a pass.
    pub measured: bool,
    /// Why it could not, when it could not.
    pub reason: Option<String>,
    /// The profile planned.
    pub profile: String,
    /// The commit `HEAD` names; the content is the index's, which equals it when
    /// `index_matches_head` is true.
    pub commit: Option<String>,
    /// Whether the index holds exactly what `HEAD` holds. `false` means staged changes
    /// travel with the pack under a commit id that does not contain them.
    pub index_matches_head: bool,
    /// How many files the index tracks.
    pub tracked: usize,
    /// How many the pack carries.
    pub selected: usize,
    /// The bytes of the content carried.
    pub bytes: u64,
    /// The tokens of the content carried, in `o200k_base`.
    pub tokens: u64,
    /// The tokenizer the counts are in.
    pub tokenizer: String,
    /// The limits applied.
    pub limits: Option<ShardLimits>,
    /// How many files were left out, by reason.
    pub dropped: BTreeMap<String, usize>,
    /// Every file left out for a reason other than `derived`.
    pub dropped_files: Vec<DroppedFile>,
    /// The shards, in order; the index is not one of them.
    pub shards: Vec<ShardPlan>,
    /// Every reason the pack cannot be built. Empty is the passing answer.
    pub findings: Vec<PackFinding>,
    /// `true` when the tree was measured and nothing was found.
    pub passes: bool,
}

impl PackPlan {
    fn unmeasured(profile: &str, reason: String) -> PackPlan {
        PackPlan {
            measured: false,
            reason: Some(reason),
            profile: profile.into(),
            commit: None,
            index_matches_head: false,
            tracked: 0,
            selected: 0,
            bytes: 0,
            tokens: 0,
            tokenizer: crate::economics::context::TOKENIZER_ENCODING.into(),
            limits: None,
            dropped: BTreeMap::new(),
            dropped_files: Vec::new(),
            shards: Vec::new(),
            findings: Vec::new(),
            passes: false,
        }
    }
}

/// A plan with the content it was made from, which is what [`build`] writes: the verdict, the
/// profile it was made with, and the selected blobs in shard order.
///
/// The examples below plan a repository holding `main.rs` and a `logo.png` whose content is
/// binary, with a distribution whose `chat` profile has limits and whose `bare` one has none.
///
/// ```
/// use majordomus_cli::pack::{plan, Planned};
/// # use std::process::Command;
/// # let dir = tempfile::tempdir().unwrap();
/// # let (root, share) = (dir.path().join("repo"), dir.path().join("share"));
/// # std::fs::create_dir_all(&root).unwrap();
/// # std::fs::create_dir_all(&share).unwrap();
/// # std::fs::write(share.join("archive.yaml"), "default: chat\nprofiles:\n  - id: chat\n    binary: drop\n    artifacts: drop\n    shards:\n      max_count: 5\n      max_tokens: 4000\n  - id: bare\n    binary: drop\n").unwrap();
/// # let git = |a: &[&str]| assert!(Command::new("git").arg("-C").arg(&root)
/// #     .args(["-c", "user.email=t@example.com", "-c", "user.name=t"]).args(a)
/// #     .status().unwrap().success());
/// # git(&["init", "-q"]);
/// # std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
/// # std::fs::write(root.join("logo.png"), b"\x89PNG\0\x01").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-q", "-m", "one"]);
/// let planned: Planned = plan(&root, &share, Some("chat"));
/// assert_eq!(planned.profile.id, "chat");
/// assert_eq!(planned.plan.profile, planned.profile.id);
/// ```
pub struct Planned {
    /// The verdict and the shape.
    pub plan: PackPlan,
    /// The profile it was planned with.
    pub profile: PackProfile,
    /// The profiles it was chosen from, which verify what [`build`] writes; empty when the
    /// profiles could not be read.
    pub profiles: Profiles,
    /// The selected files, in shard order, with their shard's index into `plan.shards`.
    pub(crate) files: Vec<(usize, Selected)>,
}

/// Plan a pack of the repository at `root` with a profile of the distribution at `share`:
/// the index's blobs, selected by the profile, cut into shards and judged. Nothing is
/// written; a tree or a profile that cannot be read is an unmeasured plan, never a pass.
///
/// The examples below plan a repository holding `main.rs` and a `logo.png` whose content is
/// binary, with a distribution whose `chat` profile has limits and whose `bare` one has none.
///
/// ```
/// use majordomus_cli::pack::plan;
/// # use std::process::Command;
/// # let dir = tempfile::tempdir().unwrap();
/// # let (root, share) = (dir.path().join("repo"), dir.path().join("share"));
/// # std::fs::create_dir_all(&root).unwrap();
/// # std::fs::create_dir_all(&share).unwrap();
/// # std::fs::write(share.join("archive.yaml"), "default: chat\nprofiles:\n  - id: chat\n    binary: drop\n    artifacts: drop\n    shards:\n      max_count: 5\n      max_tokens: 4000\n  - id: bare\n    binary: drop\n").unwrap();
/// # let git = |a: &[&str]| assert!(Command::new("git").arg("-C").arg(&root)
/// #     .args(["-c", "user.email=t@example.com", "-c", "user.name=t"]).args(a)
/// #     .status().unwrap().success());
/// # git(&["init", "-q"]);
/// # std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
/// # std::fs::write(root.join("logo.png"), b"\x89PNG\0\x01").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-q", "-m", "one"]);
/// let planned = plan(&root, &share, None);
/// assert!(planned.plan.passes, "{:?}", planned.plan.findings);
/// assert_eq!(planned.plan.profile, "chat");
/// assert_eq!(planned.plan.shards.len(), 1);
/// ```
pub fn plan(root: &Path, share: &Path, profile: Option<&str>) -> Planned {
    let label = profile.unwrap_or("").to_string();
    let profiles = match Profiles::load(share, root) {
        Ok(p) => p,
        Err(e) => return unmeasured(&label, e),
    };
    let chosen = match profiles.profile(profile) {
        Ok(p) => p.clone(),
        Err(e) => return unmeasured(&label, e),
    };
    plan_with(root, &profiles, &chosen).unwrap_or_else(|e| unmeasured(&chosen.id, e))
}

/// A plan that could not be made, with the reason: never a pass, and never built.
pub(crate) fn unmeasured(profile: &str, reason: String) -> Planned {
    Planned {
        plan: PackPlan::unmeasured(profile, reason),
        profile: PackProfile {
            id: profile.into(),
            title: String::new(),
            derived: String::new(),
            binary: String::new(),
            artifacts: String::new(),
            exclude: Vec::new(),
            include: Vec::new(),
            orientation: Vec::new(),
            shards: None,
        },
        profiles: Profiles::default(),
        files: Vec::new(),
    }
}

/// Plan with profiles already read: the body of [`plan`], for a caller that holds the
/// profiles and the chosen one.
pub(crate) fn plan_with(
    root: &Path,
    profiles: &Profiles,
    profile: &PackProfile,
) -> Result<Planned, String> {
    let (entries, derived) = index_entries(root, profile.derived == "drop")?;
    let exclude: Vec<Glob> = profile.exclude.iter().map(|g| Glob::new(g)).collect();
    let include: Vec<Glob> = profile.include.iter().map(|g| Glob::new(g)).collect();
    let artifacts = profile.artifacts == "drop";
    let binary = profile.binary == "drop";

    let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
    let mut dropped_files = Vec::new();
    let mut kept = Vec::new();
    let mut drop = |path: &str, reason: DropReason, dropped: &mut BTreeMap<String, usize>| {
        *dropped.entry(reason.as_str().to_string()).or_insert(0) += 1;
        if reason != DropReason::Derived {
            dropped_files.push(DroppedFile {
                path: path.to_string(),
                reason,
            });
        }
    };
    for e in &entries {
        let reason = if artifacts && e.mode == "160000" {
            Some(DropReason::Worktree)
        } else if artifacts && e.mode == "120000" {
            Some(DropReason::Link)
        } else if artifacts && profiles.is_artifact(&e.path) {
            Some(DropReason::Artifact)
        } else if binary && profiles.has_binary_extension(&e.path) {
            Some(DropReason::Binary)
        } else if derived.contains(&e.path) {
            Some(DropReason::Derived)
        } else if exclude.iter().any(|g| g.matches(&e.path)) {
            Some(DropReason::Excluded)
        } else {
            None
        };
        let reason =
            reason.filter(|r| !(r.reincludable() && include.iter().any(|g| g.matches(&e.path))));
        // a gitlink names a commit, not a blob: there is nothing to read, so one that is
        // kept (a profile that keeps artifacts) is still left out, and said so
        match reason {
            Some(r) => drop(&e.path, r, &mut dropped),
            None if e.mode == "160000" => drop(&e.path, DropReason::Worktree, &mut dropped),
            None => kept.push(e.clone()),
        }
    }

    let commit = head(root);
    let blobs = read_blobs(root, &kept)?;
    let machine = machine_paths(root);
    let mut findings = Vec::new();
    let mut selected = Vec::new();
    for (e, content) in kept.into_iter().zip(blobs) {
        if binary && !is_text(&content) {
            drop(&e.path, DropReason::Binary, &mut dropped);
            continue;
        }
        let Ok(text) = String::from_utf8(content) else {
            // a profile that keeps binaries cannot carry one in a text pack either
            drop(&e.path, DropReason::Binary, &mut dropped);
            continue;
        };
        if let Some(line) = names_machine(&text, &machine) {
            findings.push(PackFinding::new(
                "pack.leak",
                Some(&e.path),
                format!("line {line} names this machine: the checkout's path or the account's home directory"),
                "write what the path means rather than the path, commit, and plan again; a pack never carries it",
            ));
        }
        let tokens = shard::block_tokens(&e, &text);
        selected.push(Selected {
            entry: e,
            text,
            tokens,
        });
    }

    let limits = profile.shards;
    let mut shards = Vec::new();
    let mut files = Vec::new();
    match limits {
        None => findings.push(PackFinding::new(
            "pack.no_limits",
            None,
            format!("profile '{}' declares no `shards:` limits", profile.id),
            "add `shards: {max_count, max_tokens}` to the profile in share/archive.yaml, or pack with a profile that has them",
        )),
        Some(limits) => {
            let budget = limits.max_tokens * FILL_PERCENT / 100;
            for s in selected {
                if s.tokens > budget {
                    findings.push(PackFinding::new(
                        "pack.file_too_large",
                        Some(&s.entry.path),
                        format!(
                            "{} tokens, more than one shard's budget of {budget}",
                            s.tokens
                        ),
                        "exclude the file in the profile, or raise `shards.max_tokens` if the reader accepts it",
                    ));
                    continue;
                }
                let open = shards
                    .last()
                    .is_some_and(|sh: &ShardPlan| sh.tokens + s.tokens <= budget);
                if !open {
                    shards.push(ShardPlan {
                        file: String::new(),
                        files: 0,
                        first: s.entry.path.clone(),
                        last: String::new(),
                        bytes: 0,
                        tokens: shard::header_tokens(&s.entry.path, commit.as_deref(), &profile.id),
                    });
                }
                let i = shards.len() - 1;
                let sh = &mut shards[i];
                sh.files += 1;
                sh.last = s.entry.path.clone();
                sh.bytes += s.text.len() as u64;
                sh.tokens += s.tokens;
                files.push((i, s));
            }
            for (i, sh) in shards.iter_mut().enumerate() {
                sh.file = shard::shard_name(i + 1, &sh.first);
            }
            if shards.len() + 1 > limits.max_count {
                findings.push(PackFinding::new(
                    "pack.too_many_shards",
                    None,
                    format!(
                        "{} shard(s) and the index, more than the {} file(s) the profile allows",
                        shards.len(),
                        limits.max_count
                    ),
                    "exclude more of the tree in the profile, or raise `shards.max_tokens` so fewer shards hold it",
                ));
            }
        }
    }
    if files.is_empty() && limits.is_some() {
        findings.push(PackFinding::new(
            "pack.empty",
            None,
            "the profile selects no file".into(),
            "a pack of nothing is not a pack; check the profile's exclude list",
        ));
    }

    let bytes = files.iter().map(|(_, s)| s.text.len() as u64).sum();
    let tokens = files.iter().map(|(_, s)| s.tokens).sum();
    canonical(&mut dropped_files);
    let mut plan = PackPlan {
        measured: true,
        reason: None,
        profile: profile.id.clone(),
        commit,
        index_matches_head: index_matches_head(root),
        tracked: entries.len(),
        selected: files.len(),
        bytes,
        tokens,
        tokenizer: crate::economics::context::TOKENIZER_ENCODING.into(),
        limits,
        dropped,
        dropped_files,
        shards,
        passes: findings.is_empty(),
        findings,
    };
    // the index is a file of the pack like any shard, and is held to the same budget
    if let Some(l) = limits.filter(|_| plan.passes) {
        let tokens = shard::index_tokens(profile, &plan);
        if tokens > l.max_tokens * FILL_PERCENT / 100 {
            plan.findings.push(PackFinding::new(
                "pack.index_too_large",
                Some(INDEX),
                format!("the index would hold {tokens} tokens, more than one file's budget"),
                "exclude whole directories rather than single files, so fewer files are listed as left out",
            ));
            plan.passes = false;
        }
    }
    Ok(Planned {
        plan,
        profile: profile.clone(),
        profiles: profiles.clone(),
        files,
    })
}

/// Is `home` the home of a hosted CI runner, as the repository's one definition of that
/// shape says (`runner-path` in `src/quality/leak-patterns.tsv`)? Such a home names no
/// person's machine, and the definition itself, which the pack carries, spells it out.
fn is_runner_home(home: &Path) -> bool {
    let text = format!("{}/", home.display());
    crate::quality::leaks::Leaks::shipped()
        .map(|l| {
            !l.scan(
                text.as_bytes(),
                &[crate::quality::leaks::LeakClass::RunnerPath],
            )
            .is_empty()
        })
        .unwrap_or(false)
}

/// The first line of `text` naming one of `machine`, 1-based.
pub(crate) fn names_machine(text: &str, machine: &[String]) -> Option<usize> {
    if !machine.iter().any(|m| text.contains(m.as_str())) {
        return None;
    }
    text.lines()
        .position(|l| machine.iter().any(|m| l.contains(m.as_str())))
        .map(|n| n + 1)
}

/// Run git in `root` with `input` on its standard input and answer what it printed. A git
/// that cannot be started (a root that does not exist) and a git that fails are both an
/// error naming the subcommand, so every read below has one way to fail.
fn git_output(root: &Path, args: &[&str], input: Vec<u8>) -> Result<Vec<u8>, String> {
    // absolute, so that `-C` and the working directory name the same place; a root with no
    // absolute form becomes the empty path, which no process can be started in
    let root = std::path::absolute(root).unwrap_or_default();
    let run = crate::git::read_only(&root)
        .current_dir(&root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            let writer = child
                .stdin
                .take()
                .map(|mut stdin| std::thread::spawn(move || stdin.write_all(&input)));
            let out = child.wait_with_output();
            let _ = writer.map(std::thread::JoinHandle::join);
            out
        });
    match run {
        Ok(out) if out.status.success() => Ok(out.stdout),
        Ok(out) => Err(format!(
            "git {}: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(e) => Err(format!("cannot run git {}: {e}", args[0])),
    }
}

/// Every entry of the index in git's order, and, when `derived` is asked, the paths
/// `.gitattributes` marks `merge=derived`, asked of git itself.
fn index_entries(
    root: &Path,
    derived: bool,
) -> Result<(Vec<IndexEntry>, BTreeSet<String>), String> {
    git_output(root, &["ls-files", "-s", "-z"], Vec::new())
        .and_then(|out| parse_index(&out))
        .and_then(|entries| {
            if derived {
                derived_paths(root, &entries).map(|d| (entries, d))
            } else {
                Ok((entries, BTreeSet::new()))
            }
        })
}

/// The records of `git ls-files -s -z`: `<mode> <oid> <stage>\t<path>`. A record that is not
/// a stage-0 entry is a conflict nobody resolved, and a pack of it would carry one side.
fn parse_index(out: &[u8]) -> Result<Vec<IndexEntry>, String> {
    let mut entries = Vec::new();
    for record in out.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let record = String::from_utf8_lossy(record);
        let (meta, path) = record.split_once('\t').unwrap_or((&record, ""));
        let mut parts = meta.splitn(3, ' ');
        let (mode, oid, stage) = (
            parts.next().unwrap_or_default(),
            parts.next().unwrap_or_default(),
            parts.next().unwrap_or_default(),
        );
        if stage != "0" {
            return Err(format!(
                "{path} is unmerged in the index; resolve the conflict before packing"
            ));
        }
        entries.push(IndexEntry {
            mode: mode.into(),
            oid: oid.into(),
            path: path.into(),
        });
    }
    Ok(entries)
}

/// The paths `.gitattributes` marks `merge=derived`, asked of git itself.
fn derived_paths(root: &Path, entries: &[IndexEntry]) -> Result<BTreeSet<String>, String> {
    let paths: Vec<u8> = entries
        .iter()
        .flat_map(|e| e.path.bytes().chain(std::iter::once(0)))
        .collect();
    git_output(root, &["check-attr", "-z", "--stdin", "merge"], paths).map(|out| {
        // "<path>\0merge\0<value>\0" per path
        let fields: Vec<&[u8]> = out.split(|b| *b == 0).collect();
        fields
            .chunks(3)
            .filter(|c| c.len() == 3 && c[2] == b"derived")
            .map(|c| String::from_utf8_lossy(c[0]).into_owned())
            .collect()
    })
}

/// The content of every entry's blob, in order, through one `git cat-file --batch`.
fn read_blobs(root: &Path, entries: &[IndexEntry]) -> Result<Vec<Vec<u8>>, String> {
    let request: Vec<u8> = entries
        .iter()
        .flat_map(|e| e.oid.bytes().chain(std::iter::once(b'\n')))
        .collect();
    git_output(root, &["cat-file", "--batch"], request).and_then(|out| split_blobs(&out, entries))
}

/// The objects of a `git cat-file --batch` answer, one per entry: `<oid> blob <size>\n`,
/// the content and a newline. Anything else — a missing object, an answer cut short — is
/// named by the entry it was the answer for.
fn split_blobs(out: &[u8], entries: &[IndexEntry]) -> Result<Vec<Vec<u8>>, String> {
    let mut blobs = Vec::with_capacity(entries.len());
    let mut at = 0;
    for e in entries {
        let rest = out.get(at..).unwrap_or_default();
        let nl = rest.iter().position(|b| *b == b'\n').unwrap_or(rest.len());
        let header = String::from_utf8_lossy(&rest[..nl]).into_owned();
        let start = at + nl + 1;
        let end = header
            .strip_prefix(&format!("{} blob ", e.oid))
            .and_then(|size| size.parse::<usize>().ok())
            .map(|size| start + size)
            .filter(|end| *end <= out.len());
        let Some(end) = end else {
            return Err(format!("{}: git cat-file answered '{header}'", e.path));
        };
        blobs.push(out[start..end].to_vec());
        at = end + 1;
    }
    Ok(blobs)
}

fn head(root: &Path) -> Option<String> {
    git_output(root, &["rev-parse", "--verify", "-q", "HEAD"], Vec::new())
        .ok()
        .map(|out| String::from_utf8_lossy(&out).trim().to_string())
}

fn index_matches_head(root: &Path) -> bool {
    git_output(
        root,
        &["diff", "--cached", "--quiet", "HEAD", "--"],
        Vec::new(),
    )
    .is_ok()
}

/// Where a pack of `plan` is written when no directory is named: under the repository's
/// ignored `tmp/`, named for the repository, the profile and the commit. The repository's
/// name is its primary checkout's, which a linked worktree shares: a worktree's directory
/// is named for its branch.
pub(crate) fn default_out(root: &Path, plan: &PackPlan) -> PathBuf {
    let common = git_output(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        Vec::new(),
    )
    .ok()
    .map(|o| PathBuf::from(String::from_utf8_lossy(&o).trim()));
    let named = common
        .as_deref()
        .filter(|c| c.file_name().is_some_and(|n| n == ".git"))
        .and_then(Path::parent)
        .unwrap_or(root);
    let repo = named
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("repository"))
        .to_string_lossy();
    let short: String = plan
        .commit
        .as_deref()
        .unwrap_or("uncommitted")
        .chars()
        .take(12)
        .collect();
    root.join("tmp")
        .join("packs")
        .join(format!("{repo}-{}-{short}", plan.profile))
}

/// Write a planned pack into `out`, which must be absent, empty, or a pack this command
/// wrote (it holds a manifest); then verify what was written.
///
/// The examples below plan a repository holding `main.rs` and a `logo.png` whose content is
/// binary, with a distribution whose `chat` profile has limits and whose `bare` one has none.
///
/// ```
/// use majordomus_cli::pack::{build, plan, INDEX, MANIFEST};
/// # use std::process::Command;
/// # let dir = tempfile::tempdir().unwrap();
/// # let (root, share) = (dir.path().join("repo"), dir.path().join("share"));
/// # std::fs::create_dir_all(&root).unwrap();
/// # std::fs::create_dir_all(&share).unwrap();
/// # std::fs::write(share.join("archive.yaml"), "default: chat\nprofiles:\n  - id: chat\n    binary: drop\n    artifacts: drop\n    shards:\n      max_count: 5\n      max_tokens: 4000\n  - id: bare\n    binary: drop\n").unwrap();
/// # let git = |a: &[&str]| assert!(Command::new("git").arg("-C").arg(&root)
/// #     .args(["-c", "user.email=t@example.com", "-c", "user.name=t"]).args(a)
/// #     .status().unwrap().success());
/// # git(&["init", "-q"]);
/// # std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
/// # std::fs::write(root.join("logo.png"), b"\x89PNG\0\x01").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-q", "-m", "one"]);
/// let out = dir.path().join("pack");
/// let verdict = build(&plan(&root, &share, None), &root, &out).unwrap();
/// assert!(verdict.passes, "{:?}", verdict.findings);
/// assert!(out.join(INDEX).is_file() && out.join(MANIFEST).is_file());
/// // a plan with a finding is never written
/// let refused = build(&plan(&root, &share, Some("bare")), &root, &out);
/// assert!(refused.is_err());
/// ```
pub fn build(planned: &Planned, root: &Path, out: &Path) -> Result<PackVerdict, String> {
    if !planned.plan.passes {
        return Err("the plan has findings; a pack with a finding is not built".into());
    }
    if out.exists() {
        let names: Vec<String> = std::fs::read_dir(out)
            .map_err(|e| format!("cannot read {}: {e}", out.display()))?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        // every name is judged before anything is removed, so a refusal removes nothing
        if !names.is_empty() && !out.join(MANIFEST).is_file() {
            return Err(format!(
                "{} is not empty and holds no {MANIFEST}: it is not a pack, and nothing in it is removed",
                out.display()
            ));
        }
        if let Some(name) = names.iter().find(|n| *n != MANIFEST && !n.ends_with(".md")) {
            return Err(format!(
                "{} holds {name}, which no pack writes; remove it by hand",
                out.display()
            ));
        }
        for name in &names {
            std::fs::remove_file(out.join(name))
                .map_err(|e| format!("cannot replace {name}: {e}"))?;
        }
    }
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    shard::write(planned, out)?;
    Ok(verify(out, &planned.profiles, root))
}

#[cfg(test)]
mod tests;
