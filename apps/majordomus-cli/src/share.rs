//! The tool distribution's data directory: `share/kinds.yaml` and
//! `share/schemas/<kind>.schema.json`, read at run time. Nothing about kinds or keys is
//! compiled into the executable; the directory is located per invocation, explicitly or
//! by convention, and named in every error when it is not.
//!
//! # Why nothing is compiled in
//!
//! A kind is a fact about a repository's layer, not about the executable that reads it. If
//! the kinds were compiled in, adding one would mean shipping a binary, and a repository
//! that added its own would be adding it to somebody else's program. Reading them at run
//! time is what makes `.ai/repo/knowledge/` able to declare a kind that the same binary
//! then serves.
//!
//! ```
//! use majordomus_cli::share::Share;
//!
//! // located explicitly, and the paths under it are derived rather than joined by callers
//! let dir = tempfile::tempdir().unwrap();
//! std::fs::write(dir.path().join("kinds.yaml"), "version: 1\nkinds: []\n").unwrap();
//! std::fs::create_dir(dir.path().join("schemas")).unwrap();
//!
//! let share = Share::locate(Some(dir.path()), dir.path()).unwrap();
//! assert_eq!(share.origin, "--share", "it says how it was found");
//! assert!(share.kinds_path().ends_with("kinds.yaml"));
//! assert!(share.schemas_dir().ends_with("schemas"));
//! assert_eq!(share.kinds_path().parent(), Some(share.dir()));
//!
//! // and when there is none, the error names where it looked
//! let nowhere = tempfile::tempdir().unwrap();
//! let err = Share::locate(None, nowhere.path()).unwrap_err();
//! assert_eq!(err.exit_code(), 12);
//! ```

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The environment variable that names the share directory.
pub const SHARE_ENV: &str = "MAJORDOMUS_SHARE";

/// The kinds file inside the share directory.
pub const KINDS_FILE: &str = "kinds.yaml";

/// The provider declarations inside the share directory: what a provider is beyond its
/// template — its title, the client configuration it reads, the scratch roots it creates.
pub const PROVIDERS_FILE: &str = "providers.yaml";

/// The JSON Schema directory inside the share directory.
pub const SCHEMAS_DIR: &str = "schemas";

/// The directory of the shell tool's allow-lists, which `generate allow` derives from the
/// schemas.
pub const ALLOW_DIR: &str = "allow";

/// The directory of the shell tool's body-section lists, which `generate documents`
/// derives from the `.proto` document schemas. The allow-list is the header half of a
/// contract and this is the body half; a shell cannot read either out of a schema.
pub const SECTIONS_DIR: &str = "sections";

/// The subdirectory of [`SCHEMAS_DIR`] holding one JSON Schema per generated document.
/// A kind's schema is projected from a `.proto` under `<vendor>/<name>/`, so nothing here
/// is ever read as a kind's schema.
pub const GENERATED_SCHEMAS_DIR: &str = "generated";

/// The suffix of a schema file: `<name>.schema.json`.
pub const SCHEMA_SUFFIX: &str = ".schema.json";

/// The projection of the version inside a share directory: `version.txt`.
const VERSION_FILE: &str = "version.txt";

/// The version a share directory states (`version=<x>` in its `version.txt`), when it
/// states one. A fixture or a hand-made share states none, and nothing is concluded from
/// that.
fn stated_version(share: &Path) -> Option<String> {
    let text = std::fs::read_to_string(share.join(VERSION_FILE)).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("version="))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Does an inherited share lose to the executable's own? Only when everything is known:
/// the named share states a version, it is not this executable's, and the share beside the
/// executable states exactly this executable's.
fn yields_to_own(named: Option<&str>, own: Option<&str>, executable: &str) -> bool {
    matches!((named, own), (Some(n), Some(o)) if n != executable && o == executable)
}

/// Must the share the environment names yield to the one beside the executable? The one
/// beside it has to be a share at all and another directory than the named one; the rest
/// is [`yields_to_own`] over what the two state.
fn named_yields(named: &Path, beside: &Path, executable: &str) -> bool {
    if !beside.join(KINDS_FILE).is_file() {
        return false;
    }
    if let (Ok(a), Ok(b)) = (named.canonicalize(), beside.canonicalize()) {
        if a == b {
            return false;
        }
    }
    yields_to_own(
        stated_version(named).as_deref(),
        stated_version(beside).as_deref(),
        executable,
    )
}

/// A located distribution directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    dir: PathBuf,
    /// How it was found, for diagnostics: `--share`, the environment, the repository, or
    /// the executable's own installation.
    pub origin: &'static str,
}

impl Share {
    /// Locate the share directory. In order: `explicit` (from `--share`), `MAJORDOMUS_SHARE`,
    /// `<repository root>/share` when it holds a kinds file (this repository supervising
    /// itself), and `<directory of the executable>/../share` (an installation laid out as
    /// `bin/` beside `share/`). A candidate that exists but holds no kinds file is skipped
    /// only when it came from a convention; an explicit path that lacks one is an error.
    ///
    /// One named share is not taken: a `MAJORDOMUS_SHARE` whose `version.txt` states another
    /// version than this executable's, while the share beside the executable states this
    /// one. The variable is inherited — a terminal multiplexer started in one checkout hands
    /// it to every shell it opens, in every repository — and a share of another version is
    /// another distribution's kinds, schemas and UI assets. A share that states no version,
    /// as every fixture does, and `--share` are taken as given.
    pub fn locate(explicit: Option<&Path>, repo_root: &Path) -> Result<Self> {
        let named = std::env::var_os(SHARE_ENV).map(PathBuf::from);
        let exe = std::env::current_exe().ok();
        Self::locate_among(
            explicit,
            named.as_deref(),
            repo_root,
            exe.as_deref(),
            env!("CARGO_PKG_VERSION"),
        )
    }

    /// [`Share::locate`] over what it reads from the process: the share the environment
    /// names, the path of the executable and the executable's version.
    fn locate_among(
        explicit: Option<&Path>,
        named: Option<&Path>,
        repo_root: &Path,
        exe: Option<&Path>,
        executable: &str,
    ) -> Result<Self> {
        let mut tried = Vec::new();
        if let Some(p) = explicit {
            return Self::open(p, "--share");
        }
        let beside = exe.and_then(Path::parent).map(|dir| dir.join("../share"));
        if let Some(named) = named {
            if let Some(own) = beside
                .as_deref()
                .filter(|own| named_yields(named, own, executable))
            {
                // Said once a process: a server locates its share on every capability call
                // that needs one, and a line per call would bury what it is there to say.
                static SAID: std::sync::Once = std::sync::Once::new();
                SAID.call_once(|| {
                    tracing::warn!(
                        named = %named.display(),
                        used = %own.display(),
                        "{SHARE_ENV} names the share of another version; this executable reads the one it was installed with"
                    );
                });
                return Self::open(own, "installation");
            }
            return Self::open(named, SHARE_ENV);
        }
        let in_repo = repo_root.join("share");
        if in_repo.join(KINDS_FILE).is_file() {
            return Self::open(&in_repo, "repository");
        }
        tried.push(in_repo);
        if let Some(beside) = beside {
            if beside.join(KINDS_FILE).is_file() {
                return Self::open(&beside, "installation");
            }
            tried.push(beside);
        }
        Err(Error::ShareNotFound { tried })
    }

    fn open(dir: &Path, origin: &'static str) -> Result<Self> {
        let kinds = dir.join(KINDS_FILE);
        if !kinds.is_file() {
            return Err(Error::ShareNotFound {
                tried: vec![dir.to_path_buf()],
            });
        }
        let dir = dir.canonicalize().map_err(|e| Error::io(dir, e))?;
        Ok(Share { dir, origin })
    }

    /// The directory, canonical.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The distribution's default scope declaration, the file `majordomus init` seeds.
    pub fn skeleton_scope(&self) -> PathBuf {
        self.dir.join(crate::scope::SKELETON_SCOPE)
    }

    /// `<share>/kinds.yaml`.
    pub fn kinds_path(&self) -> PathBuf {
        self.dir.join(KINDS_FILE)
    }

    /// `<share>/schemas`.
    pub fn schemas_dir(&self) -> PathBuf {
        self.dir.join(SCHEMAS_DIR)
    }

    /// `<share>/allow`.
    pub fn allow_dir(&self) -> PathBuf {
        self.dir.join(ALLOW_DIR)
    }

    /// `<share>/sections`.
    pub fn sections_dir(&self) -> PathBuf {
        self.dir.join(SECTIONS_DIR)
    }

    /// `<share>/schemas/generated`: the contracts of the generated documents, matched to a
    /// document by the `const` of its `schema` member.
    pub fn generated_schemas_dir(&self) -> PathBuf {
        self.dir.join(SCHEMAS_DIR).join(GENERATED_SCHEMAS_DIR)
    }

    /// `<share>/providers`: the provider templates the tool ships, `<provider>.tmpl` each.
    pub fn providers_dir(&self) -> PathBuf {
        self.dir.join("providers")
    }

    /// The providers the tool has a template for, by id (the file stem), sorted. A
    /// distribution without the directory has none; that is not an error, it is a
    /// distribution that ships no adapter.
    pub fn provider_templates(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.providers_dir()) else {
            return Vec::new();
        };
        let mut out: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter_map(|n| n.strip_suffix(".tmpl").map(str::to_string))
            .collect();
        crate::order::canonical(&mut out);
        out
    }

    /// `<share>/providers.yaml`.
    pub fn providers_path(&self) -> PathBuf {
        self.dir.join(PROVIDERS_FILE)
    }

    /// The provider declarations the distribution ships, joined with its templates: one
    /// [`ProviderDeclaration`] per id that has either, sorted by id. A distribution without
    /// the file declares nothing and every template is a provider named by its file; a
    /// file that does not parse is an error naming it, because a declaration half-read is
    /// worse than none.
    pub fn providers(&self) -> Result<ProviderDeclarations> {
        let path = self.providers_path();
        let file: ProvidersFile = match std::fs::read_to_string(&path) {
            Ok(text) => {
                crate::metadata::yaml::parse_into(&text).map_err(|reason| Error::KindSchema {
                    reason: format!("{}: {reason}", path.display()),
                })?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ProvidersFile::default(),
            Err(e) => return Err(Error::io(&path, e)),
        };
        let templates = self.provider_templates();
        let mut ids: Vec<String> = templates.clone();
        ids.extend(file.providers.keys().cloned());
        crate::order::canonical(&mut ids);
        ids.dedup();
        let providers = ids
            .into_iter()
            .map(|id| {
                let decl = file.providers.get(&id);
                ProviderDeclaration {
                    title: decl
                        .and_then(|d| d.title.clone())
                        .unwrap_or_else(|| id.clone()),
                    client_config: decl.and_then(|d| d.client_config.clone()),
                    scratch_roots: decl.map(|d| d.scratch_roots.clone()).unwrap_or_default(),
                    lifecycle: decl.map(|d| d.lifecycle.clone()).unwrap_or_default(),
                    prompt_capture: decl.map(|d| d.prompt_capture).unwrap_or(false),
                    offers: decl
                        .and_then(|d| d.offers.as_ref())
                        .map(offers_of)
                        .unwrap_or_default(),
                    template: templates.contains(&id),
                    declared: decl.is_some(),
                    id,
                }
            })
            .collect();
        Ok(ProviderDeclarations {
            scratch_roots: file.scratch_roots,
            providers,
        })
    }
}

/// `share/providers.yaml` as written.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvidersFile {
    #[serde(default)]
    #[allow(dead_code)]
    version: u64,
    #[serde(default)]
    scratch_roots: Vec<String>,
    #[serde(default)]
    providers: std::collections::BTreeMap<String, ProviderEntry>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderEntry {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    client_config: Option<String>,
    #[serde(default)]
    scratch_roots: Vec<String>,
    #[serde(default)]
    lifecycle: Vec<String>,
    #[serde(default)]
    prompt_capture: bool,
    #[serde(default)]
    offers: Option<ProviderOffersEntry>,
}

/// The `offers:` block of one provider entry, as written.
///
/// The two capability keys deserialise straight into their enums, so a word outside the
/// vocabulary fails the read of the whole file with the path named rather than degrading
/// to `none`. A typo that quietly means "this provider can do nothing" is the exact
/// failure this file was written to end.
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderOffersEntry {
    #[serde(default)]
    episode: Option<EpisodeSource>,
    #[serde(default)]
    episode_evidence: Option<String>,
    #[serde(default)]
    prompts: Option<PromptSource>,
    #[serde(default)]
    prompts_evidence: Option<String>,
}

fn offers_of(e: &ProviderOffersEntry) -> ProviderOffers {
    ProviderOffers {
        episode: e.episode.unwrap_or_default(),
        episode_evidence: e.episode_evidence.clone().unwrap_or_default(),
        prompts: e.prompts.unwrap_or_default(),
        prompts_evidence: e.prompts_evidence.clone().unwrap_or_default(),
    }
}

/// How an episode boundary can be drawn for one provider.
///
/// Three words, because three things are true of different providers and collapsing them
/// into "supported / not supported" is what let this tool report one provider and stay
/// silent about five others. A provider that fires its own events is not the same as one
/// that fires none but speaks MCP, and neither is the same as one that can do nothing.
///
/// The words are the declaration's own, and a word outside them is refused rather than read
/// as `none`:
///
/// ```
/// use majordomus_cli::share::EpisodeSource;
/// let read: EpisodeSource = serde_json::from_str("\"connection\"").unwrap();
/// assert_eq!(read, EpisodeSource::Connection);
/// assert_eq!(EpisodeSource::default(), EpisodeSource::None, "undeclared reads as none");
/// assert!(serde_json::from_str::<EpisodeSource>("\"hook\"").is_err(), "hooks, plural");
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeSource {
    /// The provider fires its own session events and a hook this tool installs draws the
    /// boundary from them. The strongest form: it sees a window somebody closed.
    Hooks,
    /// The provider fires no session event, but its client attaches to this repository's
    /// shared MCP server, and the connection is the boundary (ADR 0103).
    Connection,
    /// Neither. Nothing can say when this provider's episode began or ended, and the tool
    /// reports that rather than implying a capability nobody has.
    #[default]
    None,
}

impl EpisodeSource {
    /// The word as the declaration writes it, which is also how it serialises; `capture
    /// status` prints it beside the evidence that backs it.
    ///
    /// ```
    /// use majordomus_cli::share::EpisodeSource;
    /// for s in [EpisodeSource::Hooks, EpisodeSource::Connection, EpisodeSource::None] {
    ///     assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{}\"", s.as_str()));
    /// }
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            EpisodeSource::Hooks => "hooks",
            EpisodeSource::Connection => "connection",
            EpisodeSource::None => "none",
        }
    }
}

/// Whether the person's raw prompt can be captured below the model for one provider.
///
/// There is no `connection` here and there must not be: an MCP server is handed tool calls,
/// never the prompt that produced them. Prompt capture is provider-specific and the
/// declaration says so rather than pretending the connection can stand in for it.
///
/// ```
/// use majordomus_cli::share::PromptSource;
/// assert_eq!(serde_json::from_str::<PromptSource>("\"hook\"").unwrap(), PromptSource::Hook);
/// assert!(serde_json::from_str::<PromptSource>("\"connection\"").is_err(),
///         "no connection can observe a prompt");
/// assert_eq!(PromptSource::default(), PromptSource::None);
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PromptSource {
    /// The provider hands the raw prompt to a command before the model runs.
    Hook,
    /// It does not. Nothing this tool can do makes the prompt observable.
    #[default]
    None,
}

impl PromptSource {
    /// The word as the declaration writes it, which is also how it serialises.
    ///
    /// ```
    /// use majordomus_cli::share::PromptSource;
    /// assert_eq!(PromptSource::Hook.as_str(), "hook");
    /// assert_eq!(serde_json::to_string(&PromptSource::None).unwrap(), "\"none\"");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            PromptSource::Hook => "hook",
            PromptSource::None => "none",
        }
    }
}

/// What one provider can do about the session lifecycle, and where that was verified.
///
/// **Every cell carries its evidence.** A capability asserted with no citation is the defect
/// this type exists to make impossible: before it, a provider absent from a shell table was
/// silently unsupported and a provider present in one was silently capable, and no surface
/// could tell a reader which vendor page either answer came from — `capture status`
/// enumerated the one provider that had an adapter and said nothing whatever about the
/// other five this distribution declares. The rule
/// `project.a-provider-capability-cites-its-evidence` refuses a declaration without one.
///
/// A provider that declares nothing is `none` on both axes with empty evidence — the state
/// that rule refuses, so the gap is a finding rather than a silent "unsupported":
///
/// ```
/// use majordomus_cli::share::{EpisodeSource, PromptSource, ProviderOffers};
/// let undeclared = ProviderOffers::default();
/// assert_eq!((undeclared.episode, undeclared.prompts), (EpisodeSource::None, PromptSource::None));
/// assert!(undeclared.episode_evidence.is_empty() && undeclared.prompts_evidence.is_empty());
/// let generic: ProviderOffers = serde_json::from_value(serde_json::json!({
///     "episode": "connection", "episode_evidence": "ADR 0103",
///     "prompts": "none", "prompts_evidence": "MCP carries no prompt",
/// })).unwrap();
/// assert_eq!(generic.episode, EpisodeSource::Connection);
/// ```
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct ProviderOffers {
    /// How the episode boundary is drawn for this provider.
    pub episode: EpisodeSource,
    /// Where `episode` was verified: a vendor documentation URL, or the statement that no
    /// such documentation exists and what was searched for it.
    pub episode_evidence: String,
    /// Whether the raw prompt can be captured below the model.
    pub prompts: PromptSource,
    /// Where `prompts` was verified, on the same terms as `episode_evidence`.
    pub prompts_evidence: String,
}

/// What the distribution declares about its providers, joined with the templates it ships.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderDeclarations {
    /// The tool's own scratch roots, unexpanded (see the file's header for the grammar).
    pub scratch_roots: Vec<String>,
    /// Every provider, sorted by id.
    pub providers: Vec<ProviderDeclaration>,
}

/// One provider: the template's file stem, and what the declaration adds to it.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct ProviderDeclaration {
    /// The id: the template's file stem, and the key in the declarations.
    pub id: String,
    /// The name a person knows it by; the id when nothing is declared.
    pub title: String,
    /// The file it reads a project-scoped MCP client configuration from, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_config: Option<String>,
    /// The scratch roots it creates checkouts under, unexpanded.
    #[serde(default)]
    pub scratch_roots: Vec<String>,
    /// The lifecycle events the tool's adapter for it declares, in the provider's own
    /// vocabulary (`SessionStart`, `SessionEnd`, `PreCompact`). Empty means the tool ships
    /// no lifecycle adapter for it, which costs a worker the automation and none of the
    /// model. Declared, because the two other ways to answer this question are both wrong:
    /// reading `.claude/hooks/` reports an installation as a capability, and a list written
    /// into a page is a second source of truth that goes stale the day an adapter changes.
    #[serde(default)]
    pub lifecycle: Vec<String>,
    /// Whether the tool's adapter for it can archive the worker's prompts.
    #[serde(default)]
    pub prompt_capture: bool,
    /// What the provider itself can do about the session lifecycle, and where each answer
    /// was verified: the vendor's capability, where `lifecycle` is this tool's adapter. A
    /// provider with no `offers:` block declares nothing, which reads as `none` with no
    /// evidence — the state the capability rule refuses, so that the gap is a finding
    /// rather than a silent "unsupported".
    #[serde(default)]
    pub offers: ProviderOffers,
    /// The distribution ships a template for it.
    pub template: bool,
    /// The distribution declares it.
    pub declared: bool,
}

impl ProviderDeclarations {
    /// Every scratch root — the tool's own and every provider's — expanded against
    /// `primary` (the repository's primary checkout) and the environment, in declaration
    /// order, with the provider that declared each. A root whose expansion needs a variable
    /// the environment does not hold is skipped, not an error.
    pub fn scratch_roots(&self, primary: &Path) -> Vec<(Option<String>, PathBuf)> {
        let mut out = Vec::new();
        for r in &self.scratch_roots {
            if let Some(p) = expand_root(r, primary) {
                out.push((None, p));
            }
        }
        for p in &self.providers {
            for r in &p.scratch_roots {
                if let Some(path) = expand_root(r, primary) {
                    out.push((Some(p.id.clone()), path));
                }
            }
        }
        out
    }
}

/// Expand one declared root: `<primary>/`, `~/`, `${NAME:-default}` and `$NAME`, nothing
/// else. `None` when a variable without a default is unset.
///
/// ```
/// # use std::path::Path;
/// use majordomus_cli::share::expand_root;
/// std::env::set_var("MJ_DOCTEST_ROOT", "/srv/x");
/// std::env::remove_var("MJ_DOCTEST_UNSET");
/// let primary = Path::new("/repo");
/// assert_eq!(expand_root("<primary>/.claude/worktrees", primary).unwrap(), Path::new("/repo/.claude/worktrees"));
/// assert_eq!(expand_root("$MJ_DOCTEST_ROOT/wt", primary).unwrap(), Path::new("/srv/x/wt"));
/// assert_eq!(expand_root("${MJ_DOCTEST_UNSET:-/d}/wt", primary).unwrap(), Path::new("/d/wt"));
/// assert!(expand_root("$MJ_DOCTEST_UNSET/wt", primary).is_none());
/// assert_eq!(expand_root("/tmp", primary).unwrap(), Path::new("/tmp"));
/// ```
pub fn expand_root(root: &str, primary: &Path) -> Option<PathBuf> {
    let mut out = String::new();
    let mut rest = root;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("<primary>") {
            out.push_str(&primary.to_string_lossy());
            rest = r;
        } else if let Some(r) = rest.strip_prefix('~') {
            let home = std::env::var_os("HOME")?;
            out.push_str(&home.to_string_lossy());
            rest = r;
        } else if let Some(r) = rest.strip_prefix("${") {
            let end = r.find('}')?;
            let (name, default) = match r[..end].split_once(":-") {
                Some((n, d)) => (n, Some(d)),
                None => (&r[..end], None),
            };
            match std::env::var(name) {
                Ok(v) if !v.is_empty() => {
                    out.push_str(&expand_root(&v, primary)?.to_string_lossy())
                }
                _ => out.push_str(&expand_root(default?, primary)?.to_string_lossy()),
            }
            rest = &r[end + 1..];
        } else if let Some(r) = rest.strip_prefix('$') {
            let end = r
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(r.len());
            let v = std::env::var(&r[..end]).ok().filter(|v| !v.is_empty())?;
            out.push_str(&v);
            rest = &r[end..];
        } else {
            let end = rest.find(['<', '~', '$']).unwrap_or(rest.len());
            let end = if end == 0 { 1 } else { end };
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        }
    }
    Some(PathBuf::from(out))
}

/// Every `<vendor>/<name>/<name>.v<n>.schema.json` under a schema root, parsed, keyed by
/// the schema identity its path derives (`majordomus.adr/v1`) and sorted by it. A root that
/// does not exist yields nothing; that is the repository's case when it adds no schema of
/// its own.
///
/// The identity and the path fix each other, so a schema can be found from a `kinds.yaml`
/// entry without a directory scan and a file cannot quietly answer to a name it does not
/// declare. A file whose path does not have the shape is refused by name rather than
/// skipped, because a schema nobody loads validates nothing and says so nowhere.
pub fn read_schema_dir(dir: &Path) -> Result<Vec<(String, serde_json::Value)>> {
    let mut schemas = Vec::new();
    read_schema_tree(dir, dir, &mut schemas)?;
    schemas.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(schemas)
}

fn read_schema_tree(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, serde_json::Value)>,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
        let path = entry.map_err(|e| Error::io(dir, e))?.path();
        if path.is_dir() {
            // `<schemas>/generated/` holds the contracts of the *generated documents* —
            // the manifest, the registry, the benchmark matrix. Those are not kinds of the
            // layer, carry no `<vendor>/<name>/` identity, and are read by
            // [`read_generated_schema_dir`] instead.
            if dir == root
                && path.file_name().and_then(|f| f.to_str()) == Some(GENERATED_SCHEMAS_DIR)
            {
                continue;
            }
            read_schema_tree(root, &path, out)?;
            continue;
        }
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if !file.ends_with(SCHEMA_SUFFIX) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| file.to_string());
        let identity = schema_identity(&relative).ok_or_else(|| Error::KindSchema {
            reason: format!(
                "{}: a schema's path is <vendor>/<name>/<name>.v<n>{SCHEMA_SUFFIX}, which this is not",
                path.display()
            ),
        })?;
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| Error::KindSchema {
                reason: format!("{}: not JSON: {e}", path.display()),
            })?;
        out.push((identity, json));
    }
    Ok(())
}

/// Every `<name>.schema.json` directly under a directory, parsed, keyed by that `<name>`
/// and sorted by it. A directory that does not exist yields nothing.
///
/// This is the reader for `<share>/schemas/generated/`, whose files are the contracts of
/// the *generated documents* rather than of the layer's kinds: they answer to the document
/// id they pin in `properties.schema.const`, not to a `<vendor>/<name>` path, so the shape
/// [`read_schema_dir`] insists on does not apply to them.
pub fn read_generated_schema_dir(dir: &Path) -> Result<Vec<(String, serde_json::Value)>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
        let path = entry.map_err(|e| Error::io(dir, e))?.path();
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if path.is_dir() || !file.ends_with(SCHEMA_SUFFIX) {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| Error::KindSchema {
                reason: format!("{}: not JSON: {e}", path.display()),
            })?;
        out.push((file[..file.len() - SCHEMA_SUFFIX.len()].to_string(), json));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The identity a schema path carries: `majordomus/adr/adr.v1.schema.json` is
/// `majordomus.adr/v1`. A path of another shape yields nothing.
///
/// ```
/// use majordomus_cli::share::schema_identity;
/// assert_eq!(
///     schema_identity("majordomus/adr/adr.v1.schema.json").as_deref(),
///     Some("majordomus.adr/v1")
/// );
/// assert_eq!(schema_identity("adr.schema.json"), None);
/// assert_eq!(schema_identity("majordomus/adr/other.v1.schema.json"), None);
/// ```
pub fn schema_identity(relative: &str) -> Option<String> {
    let stem = relative.strip_suffix(SCHEMA_SUFFIX)?;
    let mut parts = stem.split('/');
    let vendor = parts.next()?;
    let name = parts.next()?;
    let file = parts.next()?;
    if parts.next().is_some() || vendor.is_empty() || name.is_empty() {
        return None;
    }
    let version = file.strip_prefix(&format!("{name}."))?;
    if !version.starts_with('v') || version.len() < 2 {
        return None;
    }
    Some(format!("{vendor}.{name}/{version}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A share states its version in one line of `version.txt`; a file without it, or with
    /// nothing after the sign, states none.
    #[test]
    fn a_share_states_its_version_or_none() {
        let dir = tempfile::tempdir().expect("a directory");
        assert_eq!(stated_version(dir.path()), None, "no version.txt");
        std::fs::write(
            dir.path().join(VERSION_FILE),
            "# GENERATED FILE\nversion=0.21.0\n",
        )
        .expect("written");
        assert_eq!(stated_version(dir.path()).as_deref(), Some("0.21.0"));
        std::fs::write(dir.path().join(VERSION_FILE), "version=\n").expect("written");
        assert_eq!(
            stated_version(dir.path()),
            None,
            "an empty statement is none"
        );
    }

    /// The decision is made only when both versions are known, and only one way.
    #[test]
    fn a_named_share_yields_only_when_both_versions_are_known() {
        assert!(yields_to_own(Some("0.21.0"), Some("0.22.0"), "0.22.0"));
        assert!(
            !yields_to_own(Some("0.22.0"), Some("0.22.0"), "0.22.0"),
            "the same version"
        );
        assert!(
            !yields_to_own(None, Some("0.22.0"), "0.22.0"),
            "a fixture states none"
        );
        assert!(
            !yields_to_own(Some("0.21.0"), None, "0.22.0"),
            "no share of its own to prefer"
        );
        assert!(
            !yields_to_own(Some("0.21.0"), Some("0.20.0"), "0.22.0"),
            "the share beside it is not its own either"
        );
    }

    /// A share directory for the tests below: a kinds file, and the version it states.
    fn a_share(dir: &Path, version: Option<&str>) {
        std::fs::create_dir_all(dir).expect("a directory");
        std::fs::write(dir.join(KINDS_FILE), "version: 1\nkinds: []\n").expect("kinds");
        if let Some(v) = version {
            std::fs::write(dir.join(VERSION_FILE), format!("version={v}\n")).expect("version");
        }
    }

    /// The path of an executable installed under `root`, in a directory that exists: the
    /// share beside it is found through that directory.
    fn an_executable_under(root: &Path) -> PathBuf {
        let libexec = root.join("libexec");
        std::fs::create_dir_all(&libexec).expect("a directory");
        libexec.join("majordomus-cli")
    }

    /// An installed executable handed another version's share by its environment reads the
    /// share it was installed with; the same variable naming a fixture, the same version or
    /// the very directory beside it is obeyed, and so is `--share` whatever it states.
    #[test]
    fn an_inherited_share_of_another_version_yields_to_the_installed_one() {
        let root = tempfile::tempdir().expect("a directory");
        let installed = root.path().join("installed");
        let exe = an_executable_under(&installed);
        let own = installed.join("share");
        a_share(&own, Some("0.22.0"));
        let other = root.path().join("checkout/share");
        let repo = root.path().join("repository");
        std::fs::create_dir_all(&repo).expect("a repository");
        let locate = |explicit: Option<&Path>, named: &Path| {
            Share::locate_among(explicit, Some(named), &repo, Some(&exe), "0.22.0")
                .expect("a share")
        };
        let canonical = |dir: &Path| dir.canonicalize().expect("it exists");

        a_share(&other, Some("0.21.0"));
        let share = locate(None, &other);
        assert_eq!(share.origin, "installation");
        assert_eq!(share.dir(), canonical(&own));
        let share = locate(Some(&other), &other);
        assert_eq!(
            (share.origin, share.dir()),
            ("--share", &*canonical(&other))
        );

        a_share(&other, Some("0.22.0"));
        let share = locate(None, &other);
        assert_eq!(
            (share.origin, share.dir()),
            (SHARE_ENV, &*canonical(&other)),
            "the same version is the distribution the variable is for"
        );

        std::fs::remove_file(other.join(VERSION_FILE)).expect("removed");
        let share = locate(None, &other);
        assert_eq!(
            (share.origin, share.dir()),
            (SHARE_ENV, &*canonical(&other)),
            "a fixture states no version and is taken as named"
        );

        let share = locate(None, &installed.join("libexec/../share"));
        assert_eq!(
            share.origin, SHARE_ENV,
            "the share beside the executable, named, is not a second one"
        );
    }

    /// With no share beside it that is its own, an executable has nothing to prefer: a
    /// build in a checkout, or an installation whose share states another version.
    #[test]
    fn a_named_share_is_obeyed_when_the_executable_has_none_of_its_own() {
        let root = tempfile::tempdir().expect("a directory");
        let other = root.path().join("checkout/share");
        a_share(&other, Some("0.21.0"));
        let repo = root.path().join("repository");
        std::fs::create_dir_all(&repo).expect("a repository");

        let built = root.path().join("target/debug/majordomus");
        let share = Share::locate_among(None, Some(&other), &repo, Some(&built), "0.22.0")
            .expect("a share");
        assert_eq!(share.origin, SHARE_ENV, "no share beside a build");
        let share =
            Share::locate_among(None, Some(&other), &repo, None, "0.22.0").expect("a share");
        assert_eq!(share.origin, SHARE_ENV, "no executable path at all");

        let installed = root.path().join("installed");
        a_share(&installed.join("share"), Some("0.20.0"));
        let exe = an_executable_under(&installed);
        let share =
            Share::locate_among(None, Some(&other), &repo, Some(&exe), "0.22.0").expect("a share");
        assert_eq!(
            share.origin, SHARE_ENV,
            "a share beside it of a third version is not its own"
        );
    }

    /// Without the variable the order is the one it always was: the repository's share,
    /// then the one beside the executable, then an error that names both.
    #[test]
    fn the_conventions_are_tried_in_order_when_nothing_is_named() {
        let root = tempfile::tempdir().expect("a directory");
        let repo = root.path().join("repository");
        std::fs::create_dir_all(&repo).expect("a repository");
        let installed = root.path().join("installed");
        let exe = an_executable_under(&installed);

        let err = Share::locate_among(None, None, &repo, Some(&exe), "0.22.0").unwrap_err();
        assert_eq!(err.exit_code(), 12);
        assert!(
            matches!(&err, Error::ShareNotFound { tried } if tried.len() == 2),
            "both conventions are named: {err:?}"
        );

        a_share(&installed.join("share"), Some("0.22.0"));
        let share = Share::locate_among(None, None, &repo, Some(&exe), "0.22.0").expect("a share");
        assert_eq!(share.origin, "installation");

        a_share(&repo.join("share"), None);
        let share = Share::locate_among(None, None, &repo, Some(&exe), "0.22.0").expect("a share");
        assert_eq!(share.origin, "repository");
    }

    /// The word each offer is printed as is the word it is declared and serialised as, so a
    /// reader of the table and a reader of the JSON read one vocabulary.
    #[test]
    fn every_offer_prints_as_the_word_it_is_declared_as() {
        for (source, word) in [
            (EpisodeSource::Hooks, "hooks"),
            (EpisodeSource::Connection, "connection"),
            (EpisodeSource::None, "none"),
        ] {
            assert_eq!(source.as_str(), word);
            assert_eq!(
                serde_json::to_string(&source).unwrap(),
                format!("\"{word}\"")
            );
        }
        for (source, word) in [(PromptSource::Hook, "hook"), (PromptSource::None, "none")] {
            assert_eq!(source.as_str(), word);
            assert_eq!(
                serde_json::to_string(&source).unwrap(),
                format!("\"{word}\"")
            );
        }
    }
}
