//! The UI kit a repository's interface is built on, judged from markers in its tree
//! (I4334, ADR 0229).
//!
//! A kit profile (`share/design/kits/<id>.yaml`, the distribution's) names the markers by
//! which a tree shows that its interface is built on one kit: the package, the Tailwind
//! plugin and source, the attributes the kit's JavaScript drives, the shell every root
//! layout carries, and the packages of every other kit. The profile holds markers only and
//! never a line of the kit's own source, which for Flowbite Pro is licensed.
//!
//! The judgement is in two halves so that the half that decides is pure:
//!
//! - [`Tree::read`] is the only code that touches the filesystem: it walks the repository
//!   below its root, skipping what the profile calls dependencies and build output, and
//!   keeps the text of the files a signal reads.
//! - [`judge`] takes that tree, the profile and the repository's `design_kit` policy and
//!   returns a [`KitReport`]. It reads nothing else, so every signal is unit-tested on a tree
//!   built in memory.
//!
//! The signals:
//!
//! | Signal | Asks |
//! |---|---|
//! | S0 | Is there a user interface at all? None passes vacuously. |
//! | S1 | Is the kit's package a dependency (its React binding for React and Next)? |
//! | S2 | Is the Tailwind plugin configured and its source scanned, not a CDN? |
//! | S3 | Is every interactive attribute driven by the kit's JavaScript, not a copy of it? |
//! | S4 | Is no other kit a dependency or an import? (Alpine is reported, not refused.) |
//! | S5 | Does every root layout carry the shell? |
//!
//! Every finding names its signal, the path, the evidence and the remedy. An exemption is a
//! policy entry `{path, because}`; it moves a finding to the exempted list, which every
//! answer prints, so an exemption is never silent. A policy that cannot be read, or an
//! exemption without a reason, is a bad policy: nothing is judged under it.
//!
//! The module is the crate's own (its parent is `pub(crate)`), so its examples are the unit
//! tests below: `a_conformant_tree_is_clean` is the example this header would carry.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::discovery::glob::Glob;
use crate::metadata::yaml;

/// Where the kit profiles live inside the distribution's share directory.
pub const KITS_DIR: &str = "design/kits";

/// The profile judged when a repository's policy declares none.
pub const DEFAULT_PROFILE: &str = "flowbite-pro";

/// The repository's policy, where the `design_kit` block is read from.
pub const POLICY: &str = ".ai/repo/policy.yaml";

/// The policy key of the block.
pub const POLICY_KEY: &str = "design_kit";

/// The format version of a kit profile this module reads.
pub const PROFILE_SCHEMA: u64 = 1;

// ------------------------------------------------------------------ the profile

/// One kit profile, as `share/design/kits/<id>.yaml` declares it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// The format version.
    pub schema: u64,
    /// The profile's id, the file's stem.
    pub id: String,
    /// What it is, for a person.
    pub title: String,
    /// The rule the profile enforces.
    pub rule: String,
    /// The file name of the rule under `.ai/repo/rules/project/`.
    pub rule_file: String,
    /// The rule's template, relative to the share directory.
    pub rule_template: String,
    /// Directories no signal reads.
    pub skip_dirs: Vec<String>,
    /// Files larger than this are build output.
    pub max_file_bytes: u64,
    /// Extensions of markup files.
    pub markup: Vec<String>,
    /// Extensions of script files.
    pub scripts: Vec<String>,
    /// Extensions of stylesheets.
    pub styles: Vec<String>,
    /// S0.
    pub ui: UiMarkers,
    /// S1.
    pub dependency: DependencyMarkers,
    /// S2.
    pub plugin: PluginMarkers,
    /// S3.
    pub interactive: InteractiveMarkers,
    /// S4.
    pub forbidden: Vec<KitMarker>,
    /// Reported, never refused.
    pub reported: Vec<ReportedMarker>,
    /// S5.
    pub shell: ShellMarkers,
}

/// What shows that a repository has a user interface.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiMarkers {
    /// The package manifests read, repository-relative.
    pub package_manifests: Vec<String>,
    /// A dependency on any of these is a user interface.
    pub packages: Vec<String>,
    /// The Mix project file.
    pub mix_file: String,
    /// A Mix dependency on any of these is a user interface.
    pub mix_deps: Vec<String>,
    /// A file matching any of these globs is a user interface.
    pub files: Vec<String>,
}

/// The kit's package.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyMarkers {
    /// The package.
    pub package: String,
    /// Its React binding.
    pub react_package: String,
    /// The framework packages under which the binding is the dependency.
    pub react_when: Vec<String>,
}

/// The Tailwind configuration, as regular expressions.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginMarkers {
    /// A Tailwind 4 entry stylesheet.
    pub v4_entry: String,
    /// The plugin under Tailwind 4.
    pub v4_plugin: String,
    /// The source under Tailwind 4.
    pub v4_source: String,
    /// A Tailwind 3 entry stylesheet.
    pub v3_entry: String,
    /// The Tailwind 3 configuration files.
    pub v3_configs: Vec<String>,
    /// The plugin under Tailwind 3.
    pub v3_plugin: String,
    /// The content glob under Tailwind 3.
    pub v3_content: String,
    /// Tailwind compiled in the browser.
    pub play_cdn: String,
    /// The kit fetched from a CDN.
    pub cdn_flowbite: String,
}

/// The kit's JavaScript.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractiveMarkers {
    /// The attributes it drives.
    pub attributes: String,
    /// What shows it is loaded.
    pub loaded: String,
    /// What shows a script reads the attributes itself.
    pub reimplemented: String,
}

/// Another kit.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitMarker {
    /// Its name.
    pub kit: String,
    /// Its npm packages; a name ending in `/` is a scope.
    #[serde(default)]
    pub packages: Vec<String>,
    /// Its Hex packages.
    #[serde(default)]
    pub mix: Vec<String>,
    /// The files its installer leaves at the root.
    #[serde(default)]
    pub files: Vec<String>,
}

/// A runtime that is reported and never refused.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportedMarker {
    /// Its name.
    pub kit: String,
    /// Its npm packages.
    #[serde(default)]
    pub packages: Vec<String>,
    /// What shows it in markup.
    #[serde(default)]
    pub markup: Option<String>,
}

/// The shell every root layout carries.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellMarkers {
    /// The root layouts, as globs.
    pub layouts: Vec<String>,
    /// The file beside a `*.html.heex` layout that is judged with it.
    pub companion: String,
    /// A fixed top navbar.
    pub fixed_nav: String,
    /// The sidebar.
    pub sidebar: String,
    /// The sidebar as a React component.
    pub react_sidebar: String,
    /// The breadcrumb.
    pub breadcrumb: String,
    /// The breadcrumb as a React component.
    pub react_breadcrumb: String,
    /// A marketing layout's navbar.
    pub marketing_nav: String,
    /// A marketing layout's footer.
    pub marketing_footer: String,
    /// The colour roles.
    pub roles: Roles,
}

/// The three colour roles, each a list of alternatives; an alternative is a space-separated
/// class list, met when every class of it is in the layout.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Roles {
    /// The page ground.
    #[serde(default)]
    pub page: Vec<String>,
    /// A card or bar.
    #[serde(default)]
    pub surface: Vec<String>,
    /// A border.
    #[serde(default)]
    pub border: Vec<String>,
}

impl Profile {
    /// Parse a profile and compile every expression in it, so a profile that cannot be
    /// applied is refused before any tree is judged.
    pub fn parse(text: &str) -> Result<Self, String> {
        let profile: Profile =
            yaml::parse_into(text).map_err(|e| format!("the profile does not parse: {e}"))?;
        if profile.schema != PROFILE_SCHEMA {
            return Err(format!(
                "the profile declares schema {}, and this executable reads {PROFILE_SCHEMA}",
                profile.schema
            ));
        }
        Compiled::new(&profile)?;
        Ok(profile)
    }

    /// Read the profile `id` from the distribution's share directory.
    pub fn load(share: &Path, id: &str) -> Result<Self, ProfileError> {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ProfileError::Unknown(id.to_string()));
        }
        let path = share.join(KITS_DIR).join(format!("{id}.yaml"));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ProfileError::Unknown(id.to_string()))
            }
            Err(e) => return Err(ProfileError::Invalid(format!("{}: {e}", path.display()))),
        };
        Profile::parse(&text).map_err(|e| ProfileError::Invalid(format!("{}: {e}", path.display())))
    }
}

/// Why a profile could not be had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    /// The distribution ships no profile of that id.
    Unknown(String),
    /// The distribution's profile cannot be read or applied.
    Invalid(String),
}

/// The profile's expressions, compiled.
struct Compiled {
    v4_entry: Regex,
    v4_plugin: Regex,
    v4_source: Regex,
    v3_entry: Regex,
    v3_plugin: Regex,
    v3_content: Regex,
    play_cdn: Regex,
    cdn_flowbite: Regex,
    attributes: Regex,
    loaded: Regex,
    reimplemented: Regex,
    fixed_nav: Regex,
    sidebar: Regex,
    react_sidebar: Regex,
    breadcrumb: Regex,
    react_breadcrumb: Regex,
    marketing_nav: Regex,
    marketing_footer: Regex,
    reported_markup: Vec<Option<Regex>>,
    forbidden_imports: Vec<Option<Regex>>,
    aside: Regex,
    id: Regex,
}

fn compile(name: &str, pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|e| format!("the profile's {name} is not an expression: {e}"))
}

impl Compiled {
    fn new(p: &Profile) -> Result<Self, String> {
        let mut forbidden_imports = Vec::new();
        for kit in &p.forbidden {
            forbidden_imports.push(import_pattern(&kit.packages)?);
        }
        let mut reported_markup = Vec::new();
        for r in &p.reported {
            reported_markup.push(match &r.markup {
                Some(m) => Some(compile(&format!("{} markup", r.kit), m)?),
                None => None,
            });
        }
        Ok(Compiled {
            v4_entry: compile("plugin.v4_entry", &p.plugin.v4_entry)?,
            v4_plugin: compile("plugin.v4_plugin", &p.plugin.v4_plugin)?,
            v4_source: compile("plugin.v4_source", &p.plugin.v4_source)?,
            v3_entry: compile("plugin.v3_entry", &p.plugin.v3_entry)?,
            v3_plugin: compile("plugin.v3_plugin", &p.plugin.v3_plugin)?,
            v3_content: compile("plugin.v3_content", &p.plugin.v3_content)?,
            play_cdn: compile("plugin.play_cdn", &p.plugin.play_cdn)?,
            cdn_flowbite: compile("plugin.cdn_flowbite", &p.plugin.cdn_flowbite)?,
            attributes: compile("interactive.attributes", &p.interactive.attributes)?,
            loaded: compile("interactive.loaded", &p.interactive.loaded)?,
            reimplemented: compile("interactive.reimplemented", &p.interactive.reimplemented)?,
            fixed_nav: compile("shell.fixed_nav", &p.shell.fixed_nav)?,
            sidebar: compile("shell.sidebar", &p.shell.sidebar)?,
            react_sidebar: compile("shell.react_sidebar", &p.shell.react_sidebar)?,
            breadcrumb: compile("shell.breadcrumb", &p.shell.breadcrumb)?,
            react_breadcrumb: compile("shell.react_breadcrumb", &p.shell.react_breadcrumb)?,
            marketing_nav: compile("shell.marketing_nav", &p.shell.marketing_nav)?,
            marketing_footer: compile("shell.marketing_footer", &p.shell.marketing_footer)?,
            reported_markup,
            forbidden_imports,
            aside: compile("aside", r"<aside\b[^>]*>")?,
            id: compile("id", r#"\bid=["']([^"']+)["']"#)?,
        })
    }
}

/// The expression that finds an import of any of `packages` in a script or a stylesheet.
fn import_pattern(packages: &[String]) -> Result<Option<Regex>, String> {
    if packages.is_empty() {
        return Ok(None);
    }
    let names: Vec<String> = packages
        .iter()
        .map(|p| {
            if p.ends_with('/') {
                format!(r"{}[^'\x22]+", regex::escape(p))
            } else {
                format!(r"{}(/[^'\x22]*)?", regex::escape(p))
            }
        })
        .collect();
    let pattern = format!(
        r#"(\bfrom|\bimport|\brequire\(|@plugin|@import)\s*["']({})["']"#,
        names.join("|")
    );
    compile("forbidden import", &pattern).map(Some)
}

// ------------------------------------------------------------------ the policy

/// A signal of the profile.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum KitSignal {
    /// The user interface exists.
    S0,
    /// The dependency.
    S1,
    /// The plugin and the source.
    S2,
    /// The kit's JavaScript.
    S3,
    /// No other kit.
    S4,
    /// The shell on each root layout.
    S5,
}

impl KitSignal {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "S1" => Some(KitSignal::S1),
            "S2" => Some(KitSignal::S2),
            "S3" => Some(KitSignal::S3),
            "S4" => Some(KitSignal::S4),
            "S5" => Some(KitSignal::S5),
            _ => None,
        }
    }
}

/// One exemption of the policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KitExemption {
    /// The path or glob a finding's path matches.
    pub path: String,
    /// Why the path does not owe it.
    pub because: String,
    /// Only these signals; empty is every signal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<KitSignal>,
    /// `marketing`: S5 judges a navbar and a footer instead of the application shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    /// How many findings it moved to the exempted list.
    #[serde(default)]
    pub matched: usize,
}

impl KitExemption {
    fn covers(&self, path: &str) -> bool {
        let base = self.path.trim_end_matches('/');
        path == base
            || (!base.is_empty() && path.starts_with(&format!("{base}/")))
            || Glob::new(base).matches(path)
    }

    fn is_marketing(&self) -> bool {
        self.shell.as_deref() == Some("marketing")
    }

    /// Whether this exemption moves `finding` to the exempted list. A marketing exemption
    /// changes what S5 judges at its path and covers no finding of its own, unless it also
    /// names signals; a finding about the repository as a whole has no path to exempt.
    fn exempts(&self, finding: &KitFinding) -> bool {
        let signal = if self.signals.is_empty() {
            !self.is_marketing()
        } else {
            self.signals.contains(&finding.signal)
        };
        signal && !finding.path.is_empty() && self.covers(&finding.path)
    }
}

/// The repository's `design_kit` block, read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KitPolicy {
    /// Whether the policy declares the block.
    pub declared: bool,
    /// The profile it names.
    pub profile: String,
    /// The root layouts it names instead of the profile's.
    pub layouts: Option<Vec<String>>,
    /// The role aliases it adds.
    pub roles: Roles,
    /// Its exemptions.
    pub exempt: Vec<KitExemption>,
}

impl KitPolicy {
    /// The policy of a repository that declares no block: the default profile, nothing
    /// exempted.
    pub fn absent() -> Self {
        KitPolicy {
            declared: false,
            profile: DEFAULT_PROFILE.into(),
            ..KitPolicy::default()
        }
    }

    /// Read the block out of the policy's text. `None` text is a repository without a
    /// policy file. Every problem is collected, so one run names all of them.
    pub fn from_policy_text(text: Option<&str>) -> Result<Self, Vec<String>> {
        let Some(text) = text else {
            return Ok(KitPolicy::absent());
        };
        let map =
            yaml::parse_mapping(text).map_err(|e| vec![format!("{POLICY} does not parse: {e}")])?;
        match map.get(POLICY_KEY) {
            None => Ok(KitPolicy::absent()),
            Some(block) => KitPolicy::from_block(block),
        }
    }

    /// Read the block itself.
    pub fn from_block(block: &Value) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let Some(map) = block.as_object() else {
            return Err(vec![format!("{POLICY_KEY} is not a mapping")]);
        };
        for key in map.keys() {
            if !["profile", "layouts", "roles", "exempt"].contains(&key.as_str()) {
                errors.push(format!(
                    "{POLICY_KEY}.{key} is not a key of the block (profile, layouts, roles, exempt)"
                ));
            }
        }
        let profile = match map.get("profile").and_then(Value::as_str) {
            Some(p) if !p.trim().is_empty() => p.trim().to_string(),
            _ => {
                errors.push(format!("{POLICY_KEY}.profile names no profile"));
                String::new()
            }
        };
        let strings = |value: &Value, at: &str, errors: &mut Vec<String>| -> Vec<String> {
            match value.as_array() {
                Some(items) => items
                    .iter()
                    .filter_map(|i| match i.as_str() {
                        Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                        _ => {
                            errors.push(format!("{at} holds an entry that is not a string"));
                            None
                        }
                    })
                    .collect(),
                None => {
                    errors.push(format!("{at} is not a list"));
                    Vec::new()
                }
            }
        };
        let layouts = map
            .get("layouts")
            .map(|v| strings(v, &format!("{POLICY_KEY}.layouts"), &mut errors));
        let mut roles = Roles::default();
        if let Some(v) = map.get("roles") {
            match v.as_object() {
                Some(r) => {
                    for (role, list) in r {
                        let at = format!("{POLICY_KEY}.roles.{role}");
                        let values = strings(list, &at, &mut errors);
                        match role.as_str() {
                            "page" => roles.page = values,
                            "surface" => roles.surface = values,
                            "border" => roles.border = values,
                            _ => errors.push(format!("{at} is not a role (page, surface, border)")),
                        }
                    }
                }
                None => errors.push(format!("{POLICY_KEY}.roles is not a mapping")),
            }
        }
        let mut exempt = Vec::new();
        match map.get("exempt") {
            None => {}
            Some(Value::Array(items)) => {
                for (n, item) in items.iter().enumerate() {
                    let at = format!("{POLICY_KEY}.exempt[{n}]");
                    match exemption(item, &at) {
                        Ok(e) => exempt.push(e),
                        Err(mut e) => errors.append(&mut e),
                    }
                }
            }
            Some(_) => errors.push(format!("{POLICY_KEY}.exempt is not a list")),
        }
        if errors.is_empty() {
            Ok(KitPolicy {
                declared: true,
                profile,
                layouts,
                roles,
                exempt,
            })
        } else {
            Err(errors)
        }
    }
}

fn exemption(item: &Value, at: &str) -> Result<KitExemption, Vec<String>> {
    let mut errors = Vec::new();
    let Some(map) = item.as_object() else {
        return Err(vec![format!("{at} is not a mapping of path and because")]);
    };
    for key in map.keys() {
        if !["path", "because", "signals", "shell"].contains(&key.as_str()) {
            errors.push(format!(
                "{at}.{key} is not a key of an exemption (path, because, signals, shell)"
            ));
        }
    }
    let text = |key: &str| {
        map.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string()
    };
    let path = text("path");
    if path.is_empty() {
        errors.push(format!("{at} names no path"));
    }
    let because = text("because");
    if because.is_empty() {
        errors.push(format!(
            "{at} ({path}) gives no because: an exemption is a decision with its reason"
        ));
    }
    let mut signals = Vec::new();
    if let Some(v) = map.get("signals") {
        match v.as_array() {
            Some(items) => {
                for s in items {
                    match s.as_str().and_then(KitSignal::parse) {
                        Some(sig) => signals.push(sig),
                        None => errors.push(format!(
                            "{at}.signals holds {s}, which is not one of S1, S2, S3, S4, S5"
                        )),
                    }
                }
            }
            None => errors.push(format!("{at}.signals is not a list")),
        }
    }
    let shell = match map.get("shell") {
        None => None,
        Some(Value::String(s)) if s == "marketing" => Some(s.clone()),
        Some(other) => {
            errors.push(format!(
                "{at}.shell is {other}, and the one shell an exemption may name is marketing"
            ));
            None
        }
    };
    if errors.is_empty() {
        Ok(KitExemption {
            path,
            because,
            signals,
            shell,
            matched: 0,
        })
    } else {
        Err(errors)
    }
}

// ------------------------------------------------------------------ the tree

/// The repository below its root, as the signals read it: every path, and the text of the
/// files a signal reads.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    /// Every file, repository-relative with `/`.
    pub paths: BTreeSet<String>,
    /// The text of every file a signal reads.
    pub texts: BTreeMap<String, String>,
}

impl Tree {
    /// A tree built from `(path, text)` pairs, every one of them read.
    #[cfg(test)]
    pub fn from_files<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut tree = Tree::default();
        for (path, text) in files {
            tree.paths.insert(path.to_string());
            tree.texts.insert(path.to_string(), text.to_string());
        }
        tree
    }

    /// Walk `root`, skipping the profile's directories, links and oversized files, and keep
    /// the text of every file a signal reads.
    pub fn read(root: &Path, profile: &Profile) -> std::io::Result<Self> {
        let mut tree = Tree::default();
        let mut stack = vec![(root.to_path_buf(), String::new())];
        while let Some((dir, rel)) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(e) if rel.is_empty() => return Err(e),
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() {
                    if !skipped(&profile.skip_dirs, &name, &path) {
                        stack.push((entry.path(), path));
                    }
                } else if kind.is_file() {
                    if name.ends_with(".min.js") || name.ends_with(".min.css") {
                        continue;
                    }
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
                    if size > profile.max_file_bytes {
                        continue;
                    }
                    if read_by_a_signal(profile, &path) {
                        if let Ok(text) = std::fs::read_to_string(entry.path()) {
                            tree.texts.insert(path.clone(), text);
                        }
                    }
                    tree.paths.insert(path);
                }
            }
        }
        Ok(tree)
    }

    fn text(&self, path: &str) -> Option<&str> {
        self.texts.get(path).map(String::as_str)
    }

    /// The files with one of `extensions`, in order.
    fn with_extension<'a>(
        &'a self,
        extensions: &'a [String],
    ) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.texts
            .iter()
            .filter(move |(p, _)| extensions.iter().any(|e| has_extension(p, e)))
            .map(|(p, t)| (p.as_str(), t.as_str()))
    }
}

fn skipped(skip: &[String], name: &str, path: &str) -> bool {
    skip.iter().any(|s| {
        if s.contains('/') {
            path == s || path.ends_with(&format!("/{s}"))
        } else {
            name == s
        }
    })
}

fn has_extension(path: &str, extension: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, e)| e.eq_ignore_ascii_case(extension))
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn read_by_a_signal(profile: &Profile, path: &str) -> bool {
    let name = file_name(path);
    profile.ui.package_manifests.iter().any(|m| m == path)
        || path == profile.ui.mix_file
        || profile.plugin.v3_configs.iter().any(|c| c == path)
        || profile
            .markup
            .iter()
            .chain(&profile.scripts)
            .chain(&profile.styles)
            .any(|e| has_extension(name, e))
}

// ------------------------------------------------------------------ the report

/// One finding: the signal, the path it is about, what was seen and what to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KitFinding {
    /// The signal.
    pub signal: KitSignal,
    /// The path, repository-relative; empty for the repository as a whole.
    pub path: String,
    /// What was seen, with the line where there is one.
    pub evidence: String,
    /// What to do.
    pub remedy: String,
}

/// A finding a policy exemption covers, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KitExempted {
    /// The finding.
    pub finding: KitFinding,
    /// The exemption's path.
    pub exemption: String,
    /// The exemption's reason.
    pub because: String,
}

/// How the tree stands against the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KitVerdict {
    /// No user interface: the rule holds vacuously.
    NoUi,
    /// A user interface, and no finding stands.
    Clean,
    /// At least one finding stands.
    Findings,
    /// The policy cannot be read; nothing was judged.
    BadPolicy,
}

/// What `design.kit` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KitReport {
    /// The profile judged.
    pub profile: String,
    /// What the profile is, for a person.
    pub title: String,
    /// Where the profile lives in the distribution.
    pub profile_source: String,
    /// The rule the profile enforces.
    pub rule: String,
    /// Whether the repository's policy declares the `design_kit` block.
    pub policy_declared: bool,
    /// Why the policy cannot be read; non-empty only for `bad_policy`.
    pub policy_errors: Vec<String>,
    /// Whether the repository has a user interface (S0).
    pub ui: bool,
    /// What showed it.
    pub ui_evidence: Vec<String>,
    /// The stacks recognised: `tailwind-4`, `tailwind-3`, `react`, `phoenix`.
    pub stacks: Vec<String>,
    /// The root layouts S5 judged.
    pub layouts: Vec<String>,
    /// Every finding that stands.
    pub findings: Vec<KitFinding>,
    /// Every finding an exemption covers.
    pub exempted: Vec<KitExempted>,
    /// What is reported and never refused, such as Alpine.js.
    pub reported: Vec<String>,
    /// The policy's exemptions, each with how many findings it covered.
    pub exemptions: Vec<KitExemption>,
    /// The verdict.
    pub verdict: KitVerdict,
    /// Whether the rule holds: `no_ui` or `clean`.
    pub passes: bool,
}

impl KitReport {
    /// The report of a policy that cannot be read: nothing judged.
    pub fn bad_policy(profile: &str, errors: Vec<String>) -> Self {
        KitReport {
            profile: profile.to_string(),
            title: String::new(),
            profile_source: profile_source(profile),
            rule: String::new(),
            policy_declared: true,
            policy_errors: errors,
            ui: false,
            ui_evidence: Vec::new(),
            stacks: Vec::new(),
            layouts: Vec::new(),
            findings: Vec::new(),
            exempted: Vec::new(),
            reported: Vec::new(),
            exemptions: Vec::new(),
            verdict: KitVerdict::BadPolicy,
            passes: false,
        }
    }
}

/// Where profile `id` lives, as a repository-relative path of the distribution.
pub fn profile_source(id: &str) -> String {
    format!("share/{KITS_DIR}/{id}.yaml")
}

// ------------------------------------------------------------------ the judgement

/// One npm dependency: where it was declared, and the version asked for.
struct Dependency {
    manifest: String,
    version: String,
}

/// The first line of `text` an expression matches, 1-based, with the matched text.
fn first_match(re: &Regex, text: &str) -> Option<(usize, String)> {
    re.find(text).map(|m| {
        let line = text[..m.start()].matches('\n').count() + 1;
        let shown: String = m.as_str().chars().take(80).collect();
        (line, shown)
    })
}

fn dependencies(tree: &Tree, profile: &Profile) -> BTreeMap<String, Dependency> {
    let mut out = BTreeMap::new();
    for manifest in &profile.ui.package_manifests {
        let Some(text) = tree.text(manifest) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        for section in [
            "dependencies",
            "devDependencies",
            "peerDependencies",
            "optionalDependencies",
        ] {
            if let Some(map) = json.get(section).and_then(Value::as_object) {
                for (name, version) in map {
                    out.entry(name.clone()).or_insert(Dependency {
                        manifest: manifest.clone(),
                        version: version.as_str().unwrap_or("").to_string(),
                    });
                }
            }
        }
    }
    out
}

fn mix_depends_on(text: &str, package: &str) -> bool {
    text.contains(&format!("{{:{package},")) || text.contains(&format!("{{:{package} ,"))
}

/// The major version a dependency's range asks for: its first run of digits.
fn major(version: &str) -> Option<u64> {
    let digits: String = version
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Whether every class of `alternative` is a class token of `text`.
fn has_classes(tokens: &BTreeSet<&str>, alternative: &str) -> bool {
    let classes: Vec<&str> = alternative.split_whitespace().collect();
    !classes.is_empty() && classes.iter().all(|c| tokens.contains(c))
}

fn class_tokens(text: &str) -> BTreeSet<&str> {
    text.split(|c: char| c.is_whitespace() || "\"'`{}()<>,;".contains(c))
        .filter(|t| !t.is_empty())
        .collect()
}

struct Judgement<'a> {
    tree: &'a Tree,
    profile: &'a Profile,
    re: Compiled,
    deps: BTreeMap<String, Dependency>,
    findings: Vec<KitFinding>,
    reported: Vec<String>,
    policy_roles: Roles,
}

impl Judgement<'_> {
    fn finding(&mut self, signal: KitSignal, path: &str, evidence: String, remedy: &str) {
        self.findings.push(KitFinding {
            signal,
            path: path.to_string(),
            evidence,
            remedy: remedy.to_string(),
        });
    }

    fn markup(&self) -> impl Iterator<Item = (&str, &str)> {
        self.tree.with_extension(&self.profile.markup)
    }

    fn scripts_and_markup(&self) -> impl Iterator<Item = (&str, &str)> {
        let p = self.profile;
        self.tree.texts.iter().filter_map(move |(path, text)| {
            p.scripts
                .iter()
                .chain(&p.markup)
                .any(|e| has_extension(path, e))
                .then_some((path.as_str(), text.as_str()))
        })
    }

    fn react(&self) -> bool {
        self.profile
            .dependency
            .react_when
            .iter()
            .any(|p| self.deps.contains_key(p))
    }

    /// S0: whether there is a user interface, and what showed it.
    fn has_ui(&self) -> Vec<String> {
        let mut evidence = Vec::new();
        for package in &self.profile.ui.packages {
            if let Some(d) = self.deps.get(package) {
                evidence.push(format!("{} declares {package}", d.manifest));
            }
        }
        if let Some(text) = self.tree.text(&self.profile.ui.mix_file) {
            for dep in &self.profile.ui.mix_deps {
                if mix_depends_on(text, dep) {
                    evidence.push(format!("{} depends on {dep}", self.profile.ui.mix_file));
                }
            }
        }
        for glob in &self.profile.ui.files {
            let g = Glob::new(glob);
            if let Some(path) = self.tree.paths.iter().find(|p| g.matches(p)) {
                evidence.push(format!("{path} matches {glob}"));
            }
        }
        evidence
    }

    /// The Tailwind major version the tree builds with, if any.
    fn tailwind(&self) -> Option<u64> {
        if let Some(v) = self.deps.get("tailwindcss").and_then(|d| major(&d.version)) {
            return Some(v);
        }
        let styles: Vec<(&str, &str)> = self.tree.with_extension(&self.profile.styles).collect();
        if styles.iter().any(|(_, t)| self.re.v4_entry.is_match(t)) {
            return Some(4);
        }
        let v3_config = self
            .profile
            .plugin
            .v3_configs
            .iter()
            .any(|c| self.tree.paths.contains(c));
        if v3_config || styles.iter().any(|(_, t)| self.re.v3_entry.is_match(t)) {
            return Some(3);
        }
        None
    }

    fn s1(&mut self) {
        let dep = &self.profile.dependency;
        let (wanted, why) = if self.react() {
            (
                dep.react_package.clone(),
                "React and Next.js take the kit's React binding",
            )
        } else {
            (dep.package.clone(), "the kit's package")
        };
        if self.deps.contains_key(&wanted) {
            return;
        }
        let manifest = self
            .profile
            .ui
            .package_manifests
            .iter()
            .find(|m| self.tree.paths.contains(*m))
            .cloned();
        let (path, evidence) = match manifest {
            Some(m) => (m.clone(), format!("{m} does not declare {wanted} ({why})")),
            None => (
                String::new(),
                format!(
                    "no package manifest ({}) declares {wanted}",
                    self.profile.ui.package_manifests.join(", ")
                ),
            ),
        };
        let remedy = format!("add {wanted} to the dependencies of the manifest that builds the interface (npm install {wanted})");
        self.finding(KitSignal::S1, &path, evidence, &remedy);
    }

    fn s2(&mut self, tailwind: Option<u64>) {
        match tailwind {
            Some(4) => self.s2_v4(),
            Some(3) => self.s2_v3(),
            Some(other) => self.finding(
                KitSignal::S2,
                "",
                format!("tailwindcss {other} is neither Tailwind 3 nor Tailwind 4"),
                "build with Tailwind 4 and Flowbite 4 (ADR 0229)",
            ),
            None => self.finding(
                KitSignal::S2,
                "",
                "no stylesheet imports Tailwind and no package declares tailwindcss".into(),
                "build the interface with Tailwind 4: a stylesheet with @import \"tailwindcss\", @plugin \"flowbite/plugin\" and @source over flowbite",
            ),
        }
        let deps_have_kit = self.deps.contains_key(&self.profile.dependency.package)
            || self
                .deps
                .contains_key(&self.profile.dependency.react_package);
        let mut cdn = Vec::new();
        for (path, text) in self.markup() {
            if let Some((line, seen)) = first_match(&self.re.play_cdn, text) {
                cdn.push((
                    path.to_string(),
                    format!("{path}:{line} loads Tailwind compiled in the browser ({seen})"),
                    "build Tailwind from the repository's stylesheets instead of the Play CDN",
                ));
            }
            if !deps_have_kit {
                if let Some((line, seen)) = first_match(&self.re.cdn_flowbite, text) {
                    cdn.push((
                        path.to_string(),
                        format!("{path}:{line} fetches Flowbite from a CDN and no package declares it ({seen})"),
                        "declare flowbite as a dependency and bundle it with the build",
                    ));
                }
            }
        }
        for (path, evidence, remedy) in cdn {
            self.finding(KitSignal::S2, &path, evidence, remedy);
        }
    }

    fn s2_v4(&mut self) {
        let styles: Vec<(String, String)> = self
            .tree
            .with_extension(&self.profile.styles)
            .map(|(p, t)| (p.to_string(), t.to_string()))
            .collect();
        let entry = styles
            .iter()
            .find(|(_, t)| self.re.v4_entry.is_match(t))
            .map(|(p, _)| p.clone())
            .unwrap_or_default();
        if !styles.iter().any(|(_, t)| self.re.v4_plugin.is_match(t)) {
            let at = if entry.is_empty() {
                "no stylesheet".to_string()
            } else {
                entry.clone()
            };
            self.finding(
                KitSignal::S2,
                &entry,
                format!("Tailwind 4, and {at} carries no @plugin \"flowbite/plugin\""),
                "add @plugin \"flowbite/plugin\"; to the stylesheet that imports Tailwind",
            );
        }
        if !styles.iter().any(|(_, t)| self.re.v4_source.is_match(t)) {
            let at = if entry.is_empty() {
                "no stylesheet".to_string()
            } else {
                entry.clone()
            };
            self.finding(
                KitSignal::S2,
                &entry,
                format!("Tailwind 4, and {at} carries no @source over flowbite"),
                "add @source \"../node_modules/flowbite\"; (the path from the stylesheet to the package) so Tailwind sees Flowbite's classes",
            );
        }
    }

    fn s2_v3(&mut self) {
        let configs: Vec<(String, String)> = self
            .profile
            .plugin
            .v3_configs
            .iter()
            .filter_map(|c| self.tree.text(c).map(|t| (c.clone(), t.to_string())))
            .collect();
        let Some((path, text)) = configs.first().cloned() else {
            self.finding(
                KitSignal::S2,
                "",
                format!(
                    "Tailwind 3, and no configuration ({}) exists",
                    self.profile.plugin.v3_configs.join(", ")
                ),
                "add tailwind.config.js with require('flowbite/plugin') and a content glob over flowbite, or move to Tailwind 4 (ADR 0229)",
            );
            return;
        };
        if !self.re.v3_plugin.is_match(&text) {
            self.finding(
                KitSignal::S2,
                &path,
                format!("Tailwind 3, and {path} does not require flowbite/plugin"),
                "add require('flowbite/plugin') to plugins",
            );
        }
        if !self.re.v3_content.is_match(&text) {
            self.finding(
                KitSignal::S2,
                &path,
                format!("Tailwind 3, and {path} has no content glob over flowbite"),
                "add './node_modules/flowbite/**/*.js' to content",
            );
        }
    }

    fn s3(&mut self) {
        let mut carriers = Vec::new();
        for (path, text) in self.markup() {
            if let Some((line, seen)) = first_match(&self.re.attributes, text) {
                carriers.push(format!("{seen} in {path}:{line}"));
            }
        }
        let loaded = self
            .scripts_and_markup()
            .any(|(_, t)| self.re.loaded.is_match(t));
        if !carriers.is_empty() && !loaded {
            let first = self
                .markup()
                .find(|(_, t)| self.re.attributes.is_match(t))
                .map(|(p, _)| p.to_string())
                .unwrap_or_default();
            let evidence = format!(
                "{} file(s) carry attributes Flowbite's JavaScript drives (first: {}) and no script loads Flowbite",
                carriers.len(),
                carriers[0]
            );
            self.finding(
                KitSignal::S3,
                &first,
                evidence,
                "import 'flowbite' in the bundle the layout loads (Phoenix: flowbite/dist/flowbite.phoenix.js)",
            );
        }
        let mut copies = Vec::new();
        for (path, text) in self.tree.with_extension(&self.profile.scripts) {
            if let Some((line, seen)) = first_match(&self.re.reimplemented, text) {
                copies.push((
                    path.to_string(),
                    format!("{path}:{line} reads Flowbite's attributes itself ({seen})"),
                ));
            }
        }
        for (path, evidence) in copies {
            self.finding(
                KitSignal::S3,
                &path,
                evidence,
                "delete the hand-written behaviour and let Flowbite's JavaScript drive the attributes",
            );
        }
    }

    fn s4(&mut self) {
        let profile = self.profile;
        let mut found = Vec::new();
        for (n, kit) in profile.forbidden.iter().enumerate() {
            for package in &kit.packages {
                for (name, dep) in &self.deps {
                    let hit = if package.ends_with('/') {
                        name.starts_with(package.as_str())
                    } else {
                        name == package
                    };
                    if hit {
                        found.push((
                            dep.manifest.clone(),
                            format!(
                                "{} declares {name} {} ({})",
                                dep.manifest, dep.version, kit.kit
                            ),
                        ));
                    }
                }
            }
            if let Some(text) = self.tree.text(&profile.ui.mix_file) {
                for package in &kit.mix {
                    if mix_depends_on(text, package) {
                        found.push((
                            profile.ui.mix_file.clone(),
                            format!("{} depends on {package} ({})", profile.ui.mix_file, kit.kit),
                        ));
                    }
                }
            }
            for file in &kit.files {
                if self.tree.paths.contains(file) {
                    found.push((
                        file.clone(),
                        format!("{file} is {}'s configuration", kit.kit),
                    ));
                }
            }
            if let Some(re) = &self.re.forbidden_imports[n] {
                for (path, text) in self.tree.texts.iter() {
                    let read = profile
                        .scripts
                        .iter()
                        .chain(&profile.styles)
                        .chain(&profile.markup)
                        .any(|e| has_extension(path, e));
                    if !read {
                        continue;
                    }
                    if let Some((line, seen)) = first_match(re, text) {
                        found.push((
                            path.clone(),
                            format!("{path}:{line} imports {} ({seen})", kit.kit),
                        ));
                    }
                }
            }
        }
        for (path, evidence) in found {
            self.finding(
                KitSignal::S4,
                &path,
                evidence,
                "remove the other kit and rebuild the component from the matching Flowbite Pro block",
            );
        }
        for (n, r) in profile.reported.iter().enumerate() {
            for package in &r.packages {
                if let Some(dep) = self.deps.get(package) {
                    self.reported.push(format!(
                        "{} declares {package} ({}); reported, not refused",
                        dep.manifest, r.kit
                    ));
                }
            }
            if let Some(re) = &self.re.reported_markup[n] {
                let files = self.markup().filter(|(_, t)| re.is_match(t)).count();
                if files > 0 {
                    self.reported.push(format!(
                        "{files} markup file(s) use {}; reported, not refused",
                        r.kit
                    ));
                }
            }
        }
    }

    /// The root layouts, from the policy or the profile.
    fn layouts(&self, policy: &KitPolicy) -> Vec<String> {
        let globs = policy
            .layouts
            .as_ref()
            .unwrap_or(&self.profile.shell.layouts);
        let compiled: Vec<Glob> = globs.iter().map(|g| Glob::new(g)).collect();
        self.tree
            .paths
            .iter()
            .filter(|p| compiled.iter().any(|g| g.matches(p)))
            .cloned()
            .collect()
    }

    fn s5(&mut self, policy: &KitPolicy, layouts: &[String]) {
        if layouts.is_empty() {
            let globs = policy
                .layouts
                .as_ref()
                .unwrap_or(&self.profile.shell.layouts);
            self.finding(
                KitSignal::S5,
                "",
                format!("no root layout matches {}", globs.join(", ")),
                "declare the root layouts under design_kit.layouts in .ai/repo/policy.yaml",
            );
            return;
        }
        let breadcrumb_anywhere = self
            .markup()
            .any(|(_, t)| self.re.breadcrumb.is_match(t) || self.re.react_breadcrumb.is_match(t));
        let react = self.react();
        for layout in layouts {
            let mut text = self.tree.text(layout).unwrap_or("").to_string();
            if layout.ends_with(".html.heex") {
                let dir = layout.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                let companion = if dir.is_empty() {
                    self.profile.shell.companion.clone()
                } else {
                    format!("{dir}/{}", self.profile.shell.companion)
                };
                if companion != *layout {
                    if let Some(c) = self.tree.text(&companion) {
                        text.push('\n');
                        text.push_str(c);
                    }
                }
            }
            let marketing = policy
                .exempt
                .iter()
                .any(|e| e.is_marketing() && e.covers(layout));
            if marketing {
                self.marketing_shell(layout, &text);
            } else {
                self.application_shell(layout, &text, react, breadcrumb_anywhere);
            }
        }
    }

    fn marketing_shell(&mut self, layout: &str, text: &str) {
        if !self.re.marketing_nav.is_match(text) {
            self.finding(
                KitSignal::S5,
                layout,
                format!("{layout} is a marketing layout with no navbar"),
                "start the page from the Pro marketing navbar block",
            );
        }
        if !self.re.marketing_footer.is_match(text) {
            self.finding(
                KitSignal::S5,
                layout,
                format!("{layout} is a marketing layout with no footer"),
                "end the page with the Pro marketing footer block",
            );
        }
    }

    fn application_shell(&mut self, layout: &str, text: &str, react: bool, breadcrumb: bool) {
        if !self.re.fixed_nav.is_match(text) {
            self.finding(
                KitSignal::S5,
                layout,
                format!("{layout} has no fixed top navbar (<nav class=\"fixed ...\">)"),
                "start the layout from the Pro shell's navbar: fixed z-30 w-full, logo, search, icon actions, user menu",
            );
        }
        let react_sidebar = react && self.re.react_sidebar.is_match(text);
        if !react_sidebar {
            let aside = self
                .re
                .aside
                .find_iter(text)
                .map(|m| m.as_str())
                .find(|tag| self.re.sidebar.is_match(tag))
                .map(str::to_string);
            match aside {
                None => self.finding(
                    KitSignal::S5,
                    layout,
                    format!("{layout} has no <aside aria-label=\"Sidebar\">"),
                    "add the Pro sidebar drawer: <aside id=\"sidebar\" aria-label=\"Sidebar\"> with a toggle button naming it",
                ),
                Some(tag) => {
                    let id = self.re.id.captures(&tag).map(|c| c[1].to_string());
                    match id {
                        None => self.finding(
                            KitSignal::S5,
                            layout,
                            format!("{layout}'s sidebar has no id for a drawer to target"),
                            "give the aside an id and name it in data-drawer-target and data-drawer-toggle",
                        ),
                        Some(id) => {
                            for attribute in ["data-drawer-target", "data-drawer-toggle"] {
                                let named = [format!("{attribute}=\"{id}\""), format!("{attribute}='{id}'")];
                                if !named.iter().any(|n| text.contains(n.as_str())) {
                                    self.finding(
                                        KitSignal::S5,
                                        layout,
                                        format!("{layout}: no {attribute} names the sidebar #{id}"),
                                        "add the Pro drawer toggle: <button data-drawer-target=\"<id>\" data-drawer-toggle=\"<id>\" aria-controls=\"<id>\">",
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        if !breadcrumb {
            self.finding(
                KitSignal::S5,
                layout,
                "no page carries <nav aria-label=\"Breadcrumb\">".into(),
                "give every application page the Pro page header: breadcrumb, title and actions",
            );
        }
        let tokens = class_tokens(text);
        let roles = [
            ("page", &self.profile.shell.roles.page),
            ("surface", &self.profile.shell.roles.surface),
            ("border", &self.profile.shell.roles.border),
        ];
        let mut missing = Vec::new();
        for (role, alternatives) in roles {
            let extra = match role {
                "page" => &self.policy_roles.page,
                "surface" => &self.policy_roles.surface,
                _ => &self.policy_roles.border,
            };
            let met = alternatives
                .iter()
                .chain(extra.iter())
                .any(|a| has_classes(&tokens, a));
            if !met {
                missing.push(format!(
                    "{role} ({})",
                    alternatives
                        .iter()
                        .chain(extra.iter())
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" | ")
                ));
            }
        }
        if !missing.is_empty() {
            self.finding(
                KitSignal::S5,
                layout,
                format!("{layout} does not carry the role(s) {}", missing.join(", ")),
                "use the Pro grays for page, surface and border, or declare the token rule's aliases under design_kit.roles; a brand colour is the primary scale (ADR 0229)",
            );
        }
    }
}

/// Judge a tree against a profile under a policy. Pure: it reads only its arguments.
pub fn judge(tree: &Tree, profile: &Profile, policy: &KitPolicy) -> Result<KitReport, String> {
    let re = Compiled::new(profile)?;
    let mut j = Judgement {
        tree,
        profile,
        re,
        deps: dependencies(tree, profile),
        findings: Vec::new(),
        reported: Vec::new(),
        policy_roles: policy.roles.clone(),
    };
    let ui_evidence = j.has_ui();
    let mut report = KitReport {
        profile: profile.id.clone(),
        title: profile.title.clone(),
        profile_source: profile_source(&profile.id),
        rule: profile.rule.clone(),
        policy_declared: policy.declared,
        policy_errors: Vec::new(),
        ui: !ui_evidence.is_empty(),
        ui_evidence,
        stacks: Vec::new(),
        layouts: Vec::new(),
        findings: Vec::new(),
        exempted: Vec::new(),
        reported: Vec::new(),
        exemptions: policy.exempt.clone(),
        verdict: KitVerdict::NoUi,
        passes: true,
    };
    if !report.ui {
        return Ok(report);
    }
    let tailwind = j.tailwind();
    if let Some(v) = tailwind {
        report.stacks.push(format!("tailwind-{v}"));
    }
    if j.react() {
        report.stacks.push("react".into());
    }
    if tree
        .text(&profile.ui.mix_file)
        .is_some_and(|t| profile.ui.mix_deps.iter().any(|d| mix_depends_on(t, d)))
    {
        report.stacks.push("phoenix".into());
    }
    j.s1();
    j.s2(tailwind);
    j.s3();
    j.s4();
    let layouts = j.layouts(policy);
    j.s5(policy, &layouts);
    report.layouts = layouts;
    report.reported = std::mem::take(&mut j.reported);
    for e in report.exemptions.iter_mut().filter(|e| e.is_marketing()) {
        e.matched += report.layouts.iter().filter(|l| e.covers(l)).count();
    }
    for finding in std::mem::take(&mut j.findings) {
        let cover = report.exemptions.iter_mut().find(|e| e.exempts(&finding));
        match cover {
            Some(e) => {
                e.matched += 1;
                report.exempted.push(KitExempted {
                    exemption: e.path.clone(),
                    because: e.because.clone(),
                    finding,
                });
            }
            None => report.findings.push(finding),
        }
    }
    report.verdict = if report.findings.is_empty() {
        KitVerdict::Clean
    } else {
        KitVerdict::Findings
    };
    report.passes = report.findings.is_empty();
    Ok(report)
}

// ------------------------------------------------------------------ inspection

/// Read a repository's policy, the profile it names and its tree, and judge it.
///
/// A policy that cannot be read, or that names a profile the distribution does not ship,
/// is answered as `bad_policy` with every reason; the error side is for what is never the
/// repository's fault: the distribution's own profile that cannot be read, a root that
/// cannot be walked.
pub fn inspect(root: &Path, share: &Path) -> Result<KitReport, String> {
    let policy_text = match std::fs::read_to_string(root.join(POLICY)) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{POLICY}: {e}")),
    };
    let policy = match KitPolicy::from_policy_text(policy_text.as_deref()) {
        Ok(policy) => policy,
        Err(errors) => return Ok(KitReport::bad_policy(DEFAULT_PROFILE, errors)),
    };
    let profile = match Profile::load(share, &policy.profile) {
        Ok(profile) => profile,
        Err(ProfileError::Unknown(id)) if policy.declared => {
            let reason = format!(
                "{POLICY_KEY}.profile names {id}, and the distribution ships no {}",
                profile_source(&id)
            );
            return Ok(KitReport::bad_policy(&id, vec![reason]));
        }
        Err(ProfileError::Unknown(id)) => {
            return Err(format!("the distribution ships no {}", profile_source(&id)))
        }
        Err(ProfileError::Invalid(reason)) => return Err(reason),
    };
    let tree = Tree::read(root, &profile).map_err(|e| format!("{}: {e}", root.display()))?;
    judge(&tree, &profile, &policy)
}

// ------------------------------------------------------------------ adoption

/// The rule directory a profile's rule is written into.
pub const RULES_DIR: &str = ".ai/repo/rules/project";

/// The enforcement entry that runs the check before every push.
pub const ON_PUSH: &str = "design-kit-on-push";

/// The enforcement entry that runs the check in CI.
pub const IN_CI: &str = "design-kit";

/// What adoption did, or would do, to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdoptAction {
    /// The file did not exist and is written.
    Created,
    /// The file is changed.
    Updated,
    /// The file already says what adoption would write.
    Unchanged,
    /// The file differs from what adoption would write and is left as it is.
    Kept,
}

/// One file adoption touches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdoptWrite {
    /// Repository-relative.
    pub path: String,
    /// What happened to it.
    pub action: AdoptAction,
    /// What was written or found.
    pub detail: String,
}

/// What `design.kit.adopt` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdoptReport {
    /// The profile adopted.
    pub profile: String,
    /// The rule it enforces.
    pub rule: String,
    /// Whether nothing was written.
    pub dry_run: bool,
    /// Every file adoption touches, in the order they are written.
    pub writes: Vec<AdoptWrite>,
    /// The CI workflow the `design-kit` entry names, when the repository has one.
    pub ci_workflow: Option<String>,
    /// What is left for a person: the hook and workflow steps doctor will ask for.
    pub notes: Vec<String>,
}

/// Why adoption was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoptError {
    /// The profile does not exist.
    NotFound(String),
    /// The repository is in a state adoption does not change by itself.
    Refused(String),
    /// The distribution's profile or template cannot be read.
    Internal(String),
}

/// The CI workflow a repository runs, preferring the conventional names.
pub fn ci_workflow(paths: &BTreeSet<String>) -> Option<String> {
    let mut workflows: Vec<&String> = paths
        .iter()
        .filter(|p| {
            p.strip_prefix(".github/workflows/")
                .is_some_and(|n| !n.contains('/') && (n.ends_with(".yml") || n.ends_with(".yaml")))
        })
        .collect();
    crate::order::canonical_strings(&mut workflows);
    for preferred in ["ci.yml", "ci.yaml", "validate.yml", "validate.yaml"] {
        if let Some(w) = workflows.iter().find(|w| file_name(w) == preferred) {
            return Some((*w).clone());
        }
    }
    if let Some(w) = workflows.first() {
        return Some((*w).clone());
    }
    paths
        .contains(".gitlab-ci.yml")
        .then(|| ".gitlab-ci.yml".to_string())
}

/// The policy with the `design_kit` block appended when it has none.
pub fn policy_with_block(text: &str, profile: &str) -> Result<(String, AdoptAction), String> {
    let map = yaml::parse_mapping(text).map_err(|e| format!("{POLICY} does not parse: {e}"))?;
    if let Some(block) = map.get(POLICY_KEY) {
        let named = block.get("profile").and_then(Value::as_str).unwrap_or("");
        if named == profile {
            return Ok((text.to_string(), AdoptAction::Unchanged));
        }
        return Err(format!(
            "{POLICY} already declares {POLICY_KEY} with profile '{named}'; change the block by hand to adopt {profile}"
        ));
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!(
        "\n# The UI kit `majordomus design kit` judges this repository against (rule\n\
         # project.ui-uses-flowbite-pro, ADR 0229), written by `majordomus design kit adopt\n\
         # {profile}`. Declare every exemption under `exempt:`, a block list whose entries\n\
         # carry `path:` and `because:` (optionally `signals:` and `shell: marketing`).\n\
         {POLICY_KEY}:\n  profile: {profile}\n"
    ));
    Ok((out, AdoptAction::Created))
}

/// One enforcement entry, as adoption declares it.
pub struct Entry {
    /// Its name.
    pub name: &'static str,
    /// What wires it: `git-hook:pre-push`, `ci:<workflow>`.
    pub wired_by: String,
}

/// The policy with each entry added to `enforcement:`, unless one of that name is there.
pub fn policy_with_enforcement(
    text: &str,
    entries: &[Entry],
) -> (String, Vec<(String, AdoptAction)>) {
    let lines: Vec<&str> = text.lines().collect();
    let header = lines
        .iter()
        .position(|l| l.trim_end() == "enforcement:" || l.trim_end() == "enforcement: []");
    let (start, end, indent, existing) = match header {
        Some(h) => {
            let mut end = h + 1;
            while end < lines.len() {
                let l = lines[end];
                let inside = l.trim().is_empty()
                    || l.starts_with(' ')
                    || l.starts_with('-')
                    || l.starts_with('#');
                if !inside {
                    break;
                }
                end += 1;
            }
            while end > h + 1 && {
                let l = lines[end - 1];
                l.trim().is_empty() || l.starts_with('#')
            } {
                end -= 1;
            }
            let indent = lines[h + 1..end]
                .iter()
                .find(|l| l.trim_start().starts_with("- "))
                .map(|l| l.len() - l.trim_start().len())
                .unwrap_or(2);
            let existing: BTreeSet<String> = lines[h + 1..end]
                .iter()
                .filter_map(|l| {
                    let t = l.trim_start().trim_start_matches("- ").trim_start();
                    t.strip_prefix("name:")
                        .map(|n| n.trim().trim_matches('"').to_string())
                })
                .collect();
            (Some(h), end, indent, existing)
        }
        None => (None, lines.len(), 2, BTreeSet::new()),
    };
    let mut actions = Vec::new();
    let mut block = String::new();
    let pad = " ".repeat(indent);
    for e in entries {
        if existing.contains(e.name) {
            actions.push((e.name.to_string(), AdoptAction::Unchanged));
            continue;
        }
        block.push_str(&format!(
            "{pad}- name: {}\n{pad}  path: majordomus\n{pad}  args: [design, kit, --check]\n{pad}  wired_by: {}\n",
            e.name, e.wired_by
        ));
        actions.push((e.name.to_string(), AdoptAction::Created));
    }
    if block.is_empty() {
        return (text.to_string(), actions);
    }
    let mut out = String::new();
    match start {
        Some(h) => {
            for (i, l) in lines.iter().enumerate() {
                if i == end {
                    out.push_str(&block);
                }
                if i == h {
                    out.push_str("enforcement:\n");
                } else {
                    out.push_str(l);
                    out.push('\n');
                }
            }
            if end == lines.len() {
                out.push_str(&block);
            }
        }
        None => {
            out.push_str(text);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(
                "\n# The UI kit gate (rule project.ui-uses-flowbite-pro), declared by `majordomus design\n\
                 # kit adopt`; `majordomus doctor` reports whether each entry is wired.\nenforcement:\n",
            );
            out.push_str(&block);
        }
    }
    (out, actions)
}

/// Plan the adoption of `profile_id` in the repository at `root`: every file it writes,
/// with its new text, and the answer. Nothing is written here.
pub fn plan_adoption(
    root: &Path,
    share: &Path,
    profile_id: &str,
) -> Result<(AdoptReport, Vec<(String, String)>), AdoptError> {
    let profile = Profile::load(share, profile_id).map_err(|e| match e {
        ProfileError::Unknown(id) => AdoptError::NotFound(format!(
            "the distribution ships no kit profile {id} ({})",
            profile_source(&id)
        )),
        ProfileError::Invalid(reason) => AdoptError::Internal(reason),
    })?;
    let template = std::fs::read_to_string(share.join(&profile.rule_template))
        .map_err(|e| AdoptError::Internal(format!("share/{}: {e}", profile.rule_template)))?;
    let policy_path = root.join(POLICY);
    let policy = std::fs::read_to_string(&policy_path).map_err(|e| {
        AdoptError::Refused(format!(
            "{POLICY} cannot be read ({e}); adoption declares the kit in a supervised repository's policy"
        ))
    })?;
    let mut writes = Vec::new();
    let mut files = Vec::new();
    let mut notes = Vec::new();

    let rule_path = format!("{RULES_DIR}/{}", profile.rule_file);
    match std::fs::read_to_string(root.join(&rule_path)) {
        Ok(current) if current == template => writes.push(AdoptWrite {
            path: rule_path.clone(),
            action: AdoptAction::Unchanged,
            detail: format!("{} is the template", profile.rule),
        }),
        Ok(_) => {
            writes.push(AdoptWrite {
                path: rule_path.clone(),
                action: AdoptAction::Kept,
                detail: format!(
                    "{} differs from share/{}",
                    profile.rule, profile.rule_template
                ),
            });
            notes.push(format!(
                "{rule_path} differs from the template share/{}; compare them and raise the rule's version if the change is meant",
                profile.rule_template
            ));
        }
        Err(_) => {
            writes.push(AdoptWrite {
                path: rule_path.clone(),
                action: AdoptAction::Created,
                detail: format!("{} from share/{}", profile.rule, profile.rule_template),
            });
            files.push((rule_path, template));
        }
    }

    let (with_block, block_action) =
        policy_with_block(&policy, &profile.id).map_err(AdoptError::Refused)?;
    let mut tree_paths = BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(root.join(".github/workflows")) {
        for e in entries.flatten() {
            tree_paths.insert(format!(
                ".github/workflows/{}",
                e.file_name().to_string_lossy()
            ));
        }
    }
    if root.join(".gitlab-ci.yml").is_file() {
        tree_paths.insert(".gitlab-ci.yml".to_string());
    }
    let workflow = ci_workflow(&tree_paths);
    let mut entries = vec![Entry {
        name: ON_PUSH,
        wired_by: "git-hook:pre-push".into(),
    }];
    match &workflow {
        Some(w) => entries.push(Entry {
            name: IN_CI,
            wired_by: format!("ci:{w}"),
        }),
        None => notes.push(format!(
            "no CI workflow was found, so only {ON_PUSH} is declared; add an entry {IN_CI} with wired_by ci:<workflow> once a workflow runs `majordomus design kit --check`"
        )),
    }
    let (with_entries, entry_actions) = policy_with_enforcement(&with_block, &entries);
    // what is written must read back: the block, the entries, and a policy the judge accepts
    let parsed = yaml::parse_mapping(&with_entries).map_err(|e| {
        AdoptError::Refused(format!(
            "{POLICY} would not parse after adoption ({e}); add the design_kit block and the enforcement entries by hand"
        ))
    })?;
    KitPolicy::from_policy_text(Some(&with_entries))
        .map_err(|errors| AdoptError::Refused(format!("{POLICY}: {}", errors.join("; "))))?;
    let declared: BTreeSet<&str> = parsed
        .get("enforcement")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.get("name").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    for e in &entries {
        if !declared.contains(e.name) {
            return Err(AdoptError::Refused(format!(
                "{POLICY}'s enforcement block could not be extended with {}; add it by hand",
                e.name
            )));
        }
    }
    let changed = with_entries != policy;
    let mut detail = vec![format!("{POLICY_KEY}: {:?}", block_action).to_lowercase()];
    for (name, action) in &entry_actions {
        detail.push(format!("{name}: {:?}", action).to_lowercase());
    }
    writes.push(AdoptWrite {
        path: POLICY.into(),
        action: if changed {
            AdoptAction::Updated
        } else {
            AdoptAction::Unchanged
        },
        detail: detail.join(", "),
    });
    if changed {
        files.push((POLICY.to_string(), with_entries));
    }
    notes.push(format!(
        "the pre-push hook must run `majordomus design kit --check`; `majordomus doctor` names {ON_PUSH} until it does"
    ));
    if let Some(w) = &workflow {
        notes.push(format!(
            "{w} must run `majordomus design kit --check` in a step of its own; `majordomus doctor` names {IN_CI} until it does"
        ));
    }
    Ok((
        AdoptReport {
            profile: profile.id.clone(),
            rule: profile.rule.clone(),
            dry_run: false,
            writes,
            ci_workflow: workflow,
            notes,
        },
        files,
    ))
}

/// Adopt `profile_id` in the repository at `root`: plan, then write unless `dry_run`.
/// Idempotent: a second adoption writes nothing and reports every file unchanged.
pub fn adopt(
    root: &Path,
    share: &Path,
    profile_id: &str,
    dry_run: bool,
) -> Result<AdoptReport, AdoptError> {
    let (mut report, files) = plan_adoption(root, share, profile_id)?;
    report.dry_run = dry_run;
    if !dry_run {
        for (path, text) in files {
            let target = root.join(&path);
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| AdoptError::Internal(format!("{}: {e}", dir.display())))?;
            }
            std::fs::write(&target, text)
                .map_err(|e| AdoptError::Internal(format!("{path}: {e}")))?;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile::load(&crate::synthetic::crate_share(), DEFAULT_PROFILE)
            .expect("the shipped profile is valid")
    }

    const PACKAGE: &str = r#"{"devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "^4.0.1"}}"#;
    const STYLE: &str = "@import \"tailwindcss\";\n@plugin \"flowbite/plugin\";\n@source \"../node_modules/flowbite\";\n";
    const SCRIPT: &str = "import 'flowbite';\n";
    const LAYOUT: &str = r#"<!doctype html>
<html><body class="bg-gray-50 dark:bg-gray-900">
<nav class="fixed z-30 w-full bg-white border-b border-gray-200 dark:bg-gray-800 dark:border-gray-700">
  <button data-drawer-target="sidebar" data-drawer-toggle="sidebar" aria-controls="sidebar">Menu</button>
</nav>
<aside id="sidebar" class="fixed top-0 left-0 w-64" aria-label="Sidebar">links</aside>
<main><nav class="flex" aria-label="Breadcrumb"><ol></ol></nav></main>
<script type="module" src="/src/main.js"></script>
</body></html>
"#;

    /// The conformant tree every mutation below starts from.
    fn conformant() -> BTreeMap<&'static str, String> {
        [
            ("package.json", PACKAGE),
            ("src/input.css", STYLE),
            ("src/main.js", SCRIPT),
            ("index.html", LAYOUT),
            ("README.md", "# A site\n"),
        ]
        .into_iter()
        .map(|(p, t)| (p, t.to_string()))
        .collect()
    }

    fn tree(files: &BTreeMap<&'static str, String>) -> Tree {
        Tree::from_files(files.iter().map(|(p, t)| (*p, t.as_str())))
    }

    fn judged(files: &BTreeMap<&'static str, String>, policy: &KitPolicy) -> KitReport {
        judge(&tree(files), &profile(), policy).expect("a judgement")
    }

    fn signals(r: &KitReport) -> Vec<(KitSignal, String)> {
        r.findings
            .iter()
            .map(|f| (f.signal, f.path.clone()))
            .collect()
    }

    fn with(edit: impl FnOnce(&mut BTreeMap<&'static str, String>)) -> KitReport {
        let mut files = conformant();
        edit(&mut files);
        judged(&files, &KitPolicy::absent())
    }

    #[test]
    fn the_shipped_profile_parses_and_compiles() {
        let p = profile();
        assert_eq!(p.id, DEFAULT_PROFILE);
        assert_eq!(p.rule, "project.ui-uses-flowbite-pro");
        assert!(crate::synthetic::crate_share()
            .join(&p.rule_template)
            .is_file());
        assert!(Compiled::new(&p).is_ok());
        assert!(Profile::parse("schema: 2\n").is_err());
        let mut text = std::fs::read_to_string(
            crate::synthetic::crate_share().join("design/kits/flowbite-pro.yaml"),
        )
        .unwrap();
        text = text.replace("schema: 1", "schema: 9");
        assert!(Profile::parse(&text).unwrap_err().contains("schema 9"));
        assert_eq!(
            Profile::load(&crate::synthetic::crate_share(), "no-such-kit").unwrap_err(),
            ProfileError::Unknown("no-such-kit".into())
        );
        assert_eq!(
            Profile::load(&crate::synthetic::crate_share(), "../kinds").unwrap_err(),
            ProfileError::Unknown("../kinds".into()),
            "an id is a file stem, never a path"
        );
    }

    #[test]
    fn a_profile_with_a_broken_expression_is_refused() {
        let text = std::fs::read_to_string(
            crate::synthetic::crate_share().join("design/kits/flowbite-pro.yaml"),
        )
        .unwrap();
        let broken = text.replace("marketing_nav: '<nav\\b'", "marketing_nav: '(<nav'");
        assert_ne!(broken, text);
        assert!(Profile::parse(&broken)
            .unwrap_err()
            .contains("shell.marketing_nav"));
    }

    #[test]
    fn a_conformant_tree_is_clean() {
        let r = judged(&conformant(), &KitPolicy::absent());
        assert!(r.ui);
        assert_eq!(r.verdict, KitVerdict::Clean, "{:#?}", r.findings);
        assert!(r.passes);
        assert_eq!(r.layouts, ["index.html"]);
        assert_eq!(r.stacks, ["tailwind-4"]);
        assert!(!r.policy_declared);
        assert_eq!(r.profile_source, "share/design/kits/flowbite-pro.yaml");
    }

    #[test]
    fn a_tree_with_no_ui_passes_vacuously() {
        let files: BTreeMap<&str, String> = [
            ("README.md", "# A library\n".to_string()),
            ("src/lib.rs", "pub fn f() {}\n".into()),
            (
                "package.json",
                r#"{"dependencies": {"lodash": "4"}}"#.into(),
            ),
        ]
        .into_iter()
        .collect();
        let r = judged(&files, &KitPolicy::absent());
        assert!(!r.ui);
        assert_eq!(r.verdict, KitVerdict::NoUi);
        assert!(r.passes && r.findings.is_empty());
    }

    #[test]
    fn s0_recognises_every_kind_of_interface() {
        for (path, text) in [
            (
                "mix.exs",
                "defp deps do\n  [{:phoenix_live_view, \"~> 1.0\"}]\nend\n",
            ),
            ("lib/app_web/page.html.heex", "<div></div>"),
            ("templates/page.html", "<div></div>"),
            ("app/layout.tsx", "export default function L() {}"),
            (
                "assets/package.json",
                r#"{"dependencies": {"react": "19"}}"#,
            ),
        ] {
            let files: BTreeMap<&str, String> = [(path, text.to_string())].into_iter().collect();
            let r = judged(&files, &KitPolicy::absent());
            assert!(r.ui, "{path} is an interface");
            assert!(
                r.ui_evidence.iter().any(|e| e.contains(path)),
                "{:?}",
                r.ui_evidence
            );
        }
        let phoenix: BTreeMap<&str, String> =
            [("mix.exs", "[{:phoenix_live_view, \"~> 1.0\"}]".to_string())]
                .into_iter()
                .collect();
        assert!(judged(&phoenix, &KitPolicy::absent())
            .stacks
            .contains(&"phoenix".to_string()));
    }

    #[test]
    fn s1_the_dependency_is_owed_and_react_takes_its_binding() {
        let r = with(|f| {
            f.insert(
                "package.json",
                r#"{"devDependencies": {"tailwindcss": "^4.1.0"}}"#.into(),
            );
        });
        assert!(
            signals(&r).contains(&(KitSignal::S1, "package.json".into())),
            "{r:#?}"
        );
        assert_eq!(r.verdict, KitVerdict::Findings);
        assert!(!r.passes);

        let r = with(|f| {
            f.insert(
                "package.json",
                r#"{"dependencies": {"next": "15", "flowbite": "4", "tailwindcss": "4"}}"#.into(),
            );
        });
        let s1: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S1)
            .collect();
        assert_eq!(s1.len(), 1, "{r:#?}");
        assert!(s1[0].evidence.contains("flowbite-react"));
        assert!(r.stacks.contains(&"react".to_string()));

        // no manifest at all: the finding is about the repository
        let r = with(|f| {
            f.remove("package.json");
        });
        assert!(r.findings.iter().any(|f| f.signal == KitSignal::S1
            && f.path.is_empty()
            && f.evidence.contains("no package manifest")));
    }

    #[test]
    fn s2_tailwind_4_owes_the_plugin_and_the_source() {
        let r = with(|f| {
            f.insert(
                "src/input.css",
                "@import \"tailwindcss\";\n@source \"../node_modules/flowbite\";\n".into(),
            );
        });
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S2 && f.evidence.contains("@plugin")),
            "{r:#?}"
        );
        let r = with(|f| {
            f.insert(
                "src/input.css",
                "@import \"tailwindcss\";\n@plugin \"flowbite/plugin\";\n".into(),
            );
        });
        let s2: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S2)
            .collect();
        assert_eq!(s2.len(), 1, "{r:#?}");
        assert!(s2[0].evidence.contains("@source"));
        assert_eq!(s2[0].path, "src/input.css");
    }

    #[test]
    fn s2_tailwind_3_owes_the_plugin_and_a_content_glob() {
        let v3 = r#"{"devDependencies": {"tailwindcss": "^3.4.17", "flowbite": "^2.5.2"}}"#;
        let r = with(|f| {
            f.insert("package.json", v3.into());
            f.insert("src/input.css", "@tailwind base;\n".into());
        });
        assert_eq!(r.stacks, ["tailwind-3"]);
        assert!(r
            .findings
            .iter()
            .any(|f| f.signal == KitSignal::S2 && f.evidence.contains("no configuration")));
        let r = with(|f| {
            f.insert("package.json", v3.into());
            f.insert(
                "tailwind.config.js",
                "module.exports = { content: ['./src/**/*.html'] };\n".into(),
            );
        });
        let s2: Vec<String> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S2)
            .map(|f| f.evidence.clone())
            .collect();
        assert_eq!(s2.len(), 2, "{s2:?}");
        let r = with(|f| {
            f.insert("package.json", v3.into());
            f.insert(
                "tailwind.config.js",
                "module.exports = { content: ['./node_modules/flowbite/**/*.js'], plugins: [require('flowbite/plugin')] };\n".into(),
            );
        });
        assert!(
            r.findings.iter().all(|f| f.signal != KitSignal::S2),
            "{r:#?}"
        );
    }

    #[test]
    fn s2_a_cdn_is_not_a_build() {
        let r = with(|f| {
            let page = LAYOUT.replace(
                "</body>",
                "<script src=\"https://cdn.tailwindcss.com\"></script></body>",
            );
            f.insert("index.html", page);
        });
        assert!(
            r.findings.iter().any(|f| f.signal == KitSignal::S2
                && f.path == "index.html"
                && f.evidence.contains("index.html:")),
            "{r:#?}"
        );

        let r = with(|f| {
            f.insert(
                "package.json",
                r#"{"devDependencies": {"tailwindcss": "^4.1.0"}}"#.into(),
            );
            let page = LAYOUT.replace(
                "</body>",
                "<script src=\"https://cdn.jsdelivr.net/npm/flowbite@4/dist/flowbite.min.js\"></script></body>",
            );
            f.insert("index.html", page);
        });
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S2 && f.evidence.contains("CDN")),
            "{r:#?}"
        );

        let r = with(|f| {
            f.insert(
                "package.json",
                r#"{"dependencies": {"flowbite": "4", "react": "19", "flowbite-react": "0.12"}}"#
                    .into(),
            );
            f.remove("src/input.css");
        });
        assert!(r
            .findings
            .iter()
            .any(|f| f.signal == KitSignal::S2
                && f.evidence.contains("no stylesheet imports Tailwind")));
    }

    #[test]
    fn s3_the_attributes_need_flowbite_js_and_no_copy_of_it() {
        let r = with(|f| {
            f.insert("src/main.js", "console.log('no kit');\n".into());
        });
        let s3: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S3)
            .collect();
        assert_eq!(s3.len(), 1, "{r:#?}");
        assert_eq!(s3[0].path, "index.html");
        assert!(s3[0].evidence.contains("data-drawer-target"));

        let r = with(|f| {
            f.insert(
                "site/nav.js",
                "document.querySelectorAll('[data-drawer-toggle]').forEach(b => b.onclick = open);\n".into(),
            );
        });
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S3 && f.path == "site/nav.js"),
            "{r:#?}"
        );
        let r = with(|f| {
            f.insert(
                "share/ui/nav.js",
                "const t = el.getAttribute('data-dropdown-toggle');\n".into(),
            );
        });
        assert!(r
            .findings
            .iter()
            .any(|f| f.signal == KitSignal::S3 && f.path == "share/ui/nav.js"));
    }

    #[test]
    fn s4_no_other_kit_and_alpine_is_only_reported() {
        let r = with(|f| {
            f.insert(
                "package.json",
                r#"{"devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "4", "daisyui": "^5", "@radix-ui/react-dialog": "1", "alpinejs": "3"}}"#.into(),
            );
        });
        let s4: Vec<&str> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S4)
            .map(|f| f.evidence.as_str())
            .collect();
        assert_eq!(s4.len(), 2, "{s4:?}");
        assert!(s4.iter().any(|e| e.contains("daisyui")));
        assert!(s4.iter().any(|e| e.contains("@radix-ui/react-dialog")));
        assert!(
            r.reported.iter().any(|n| n.contains("alpinejs")),
            "{:?}",
            r.reported
        );

        let r = with(|f| {
            f.insert(
                "src/main.js",
                "import 'flowbite';\nimport 'bootstrap/dist/js/bootstrap.bundle';\n".into(),
            );
            f.insert("components.json", "{}".into());
            f.insert("mix.exs", "[{:petal_components, \"~> 2.0\"}]".into());
            f.insert(
                "index.html",
                LAYOUT.replace("<main>", "<main x-data=\"{}\">"),
            );
        });
        let paths: Vec<String> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S4)
            .map(|f| f.path.clone())
            .collect();
        assert!(paths.contains(&"src/main.js".to_string()), "{paths:?}");
        assert!(paths.contains(&"components.json".to_string()));
        assert!(paths.contains(&"mix.exs".to_string()));
        assert!(r.reported.iter().any(|n| n.contains("Alpine")));
        assert!(r.findings.iter().all(|f| !f.evidence.contains("Alpine")));
    }

    #[test]
    fn s5_every_part_of_the_shell_is_owed() {
        let cases: [(&str, &str, &str); 5] = [
            ("class=\"fixed z-30", "class=\"z-30", "fixed top navbar"),
            (
                "aria-label=\"Sidebar\"",
                "aria-label=\"Menu\"",
                "aria-label=\"Sidebar\"",
            ),
            ("aside id=\"sidebar\"", "aside", "no id"),
            ("data-drawer-toggle=\"sidebar\"", "", "data-drawer-toggle"),
            ("aria-label=\"Breadcrumb\"", "", "Breadcrumb"),
        ];
        for (from, to, expect) in cases {
            let r = with(|f| {
                f.insert("index.html", LAYOUT.replace(from, to));
            });
            assert!(
                r.findings
                    .iter()
                    .any(|f| f.signal == KitSignal::S5 && f.evidence.contains(expect)),
                "removing {from} should be named by '{expect}': {:#?}",
                r.findings
            );
        }
        let r = with(|f| {
            f.insert(
                "index.html",
                LAYOUT.replace("dark:bg-gray-900", "dark:bg-slate-950"),
            );
        });
        let s5: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S5)
            .collect();
        assert_eq!(s5.len(), 1, "{r:#?}");
        assert!(s5[0].evidence.contains("page"));
    }

    #[test]
    fn s5_a_token_rule_declares_its_role_aliases() {
        let mut files = conformant();
        files.insert(
            "index.html",
            LAYOUT.replace("bg-gray-50 dark:bg-gray-900", "bg-canvas"),
        );
        let absent = judged(&files, &KitPolicy::absent());
        assert!(absent.findings.iter().any(|f| f.signal == KitSignal::S5));
        let policy = KitPolicy {
            roles: Roles {
                page: vec!["bg-canvas".into()],
                ..Roles::default()
            },
            ..KitPolicy::absent()
        };
        assert!(judged(&files, &policy).passes);
    }

    #[test]
    fn s5_layouts_come_from_the_policy_and_none_is_a_finding() {
        let mut files = conformant();
        let page = files.remove("index.html").unwrap();
        files.insert("public/app.html", page);
        let r = judged(&files, &KitPolicy::absent());
        assert!(
            r.findings.iter().any(|f| f.signal == KitSignal::S5
                && f.path.is_empty()
                && f.evidence.contains("no root layout")),
            "{r:#?}"
        );
        let policy = KitPolicy {
            layouts: Some(vec!["public/*.html".into()]),
            ..KitPolicy::absent()
        };
        let r = judged(&files, &policy);
        assert_eq!(r.layouts, ["public/app.html"]);
        assert!(r.passes, "{r:#?}");
    }

    #[test]
    fn s5_a_phoenix_layout_is_judged_with_its_root() {
        let mut files = conformant();
        files.remove("index.html");
        files.insert(
            "lib/app_web/components/layouts/root.html.heex",
            "<html><body class=\"bg-gray-50 dark:bg-gray-900\"><%= @inner_content %></body></html>"
                .into(),
        );
        let shell = LAYOUT
            .replace("<body class=\"bg-gray-50 dark:bg-gray-900\">", "")
            .replace("<script type=\"module\" src=\"/src/main.js\"></script>", "");
        files.insert("lib/app_web/components/layouts/app.html.heex", shell);
        let r = judged(&files, &KitPolicy::absent());
        assert_eq!(r.layouts, ["lib/app_web/components/layouts/app.html.heex"]);
        assert!(r.passes, "{r:#?}");
    }

    #[test]
    fn s5_react_draws_the_shell_with_components() {
        let files: BTreeMap<&str, String> = [
            ("package.json", r#"{"dependencies": {"next": "15", "flowbite-react": "0.12", "tailwindcss": "4"}}"#.to_string()),
            ("app/globals.css", "@import \"tailwindcss\";\n@plugin \"flowbite-react/plugin\";\n@source \"../node_modules/flowbite-react\";\n".into()),
            (
                "app/layout.tsx",
                "import { Sidebar, Breadcrumb } from 'flowbite-react';\nexport default () => <body className=\"bg-gray-50 dark:bg-gray-900\"><nav className=\"fixed bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700\"/><Sidebar aria-label=\"Sidebar\"/><Breadcrumb/></body>;\n".into(),
            ),
        ]
        .into_iter()
        .collect();
        let r = judged(&files, &KitPolicy::absent());
        assert!(r.passes, "{r:#?}");
    }

    #[test]
    fn an_exemption_moves_a_finding_and_says_why() {
        let mut files = conformant();
        files.insert("reel.html", "<div data-modal-toggle=\"m\"></div>".into());
        files.insert("src/main.js", "console.log(1);\n".into());
        // the attribute carriers are index.html first; exempt S3 there
        let policy = KitPolicy {
            declared: true,
            exempt: vec![KitExemption {
                path: "index.html".into(),
                because: "a render canvas".into(),
                signals: vec![KitSignal::S3],
                shell: None,
                matched: 0,
            }],
            ..KitPolicy::absent()
        };
        let r = judged(&files, &policy);
        assert!(
            r.findings.iter().all(|f| f.signal != KitSignal::S3),
            "{r:#?}"
        );
        assert_eq!(r.exempted.len(), 1);
        assert_eq!(r.exempted[0].because, "a render canvas");
        assert_eq!(r.exemptions[0].matched, 1);
        assert!(r.passes);

        // a signal it does not name is not exempted
        let other = KitPolicy {
            exempt: vec![KitExemption {
                signals: vec![KitSignal::S4],
                ..policy.exempt[0].clone()
            }],
            ..policy.clone()
        };
        assert!(!judged(&files, &other).passes);
        // a glob covers a tree
        let glob = KitExemption {
            path: "*.html".into(),
            signals: Vec::new(),
            ..policy.exempt[0].clone()
        };
        assert!(glob.covers("index.html") && !glob.covers("src/index.html"));
        let dir = KitExemption {
            path: "docs/".into(),
            ..glob.clone()
        };
        assert!(dir.covers("docs/a/b.html"));
    }

    #[test]
    fn a_marketing_layout_is_judged_by_its_navbar_and_footer() {
        let mut files = conformant();
        files.insert(
            "index.html",
            "<html><body><nav>brand</nav><main></main></body></html>".into(),
        );
        files.insert("src/main.js", SCRIPT.into());
        let policy = KitPolicy {
            declared: true,
            exempt: vec![KitExemption {
                path: "index.html".into(),
                because: "the landing page".into(),
                signals: Vec::new(),
                shell: Some("marketing".into()),
                matched: 0,
            }],
            ..KitPolicy::absent()
        };
        let r = judged(&files, &policy);
        let s5: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S5)
            .collect();
        assert_eq!(
            s5.len(),
            1,
            "the footer is still owed, and the exemption does not hide it: {r:#?}"
        );
        assert!(s5[0].evidence.contains("footer"));
        assert_eq!(r.exemptions[0].matched, 1);
        files.insert("index.html", "<nav>brand</nav><footer>c</footer>".into());
        assert!(judged(&files, &policy).passes);
    }

    #[test]
    fn the_policy_block_is_read_and_every_problem_named() {
        assert_eq!(
            KitPolicy::from_policy_text(None).unwrap(),
            KitPolicy::absent()
        );
        assert_eq!(
            KitPolicy::from_policy_text(Some("version: 1\n")).unwrap(),
            KitPolicy::absent()
        );
        let ok = KitPolicy::from_policy_text(Some(
            "design_kit:\n  profile: flowbite-pro\n  layouts: [index.html]\n  roles:\n    page: [bg-canvas]\n  exempt:\n    - path: reel.html\n      because: a render canvas\n      signals: [S5]\n    - path: landing.html\n      because: marketing\n      shell: marketing\n",
        ))
        .unwrap();
        assert!(ok.declared);
        assert_eq!(ok.layouts, Some(vec!["index.html".to_string()]));
        assert_eq!(ok.roles.page, ["bg-canvas"]);
        assert_eq!(ok.exempt.len(), 2);
        assert_eq!(ok.exempt[0].signals, [KitSignal::S5]);
        assert!(ok.exempt[1].is_marketing());

        let bad = KitPolicy::from_policy_text(Some(
            "design_kit:\n  profile: ''\n  colour: blue\n  roles:\n    accent: [x]\n  exempt:\n    - path: reel.html\n    - because: why\n    - path: a\n      because: b\n      signals: [S9]\n      shell: admin\n      extra: 1\n",
        ))
        .unwrap_err();
        let all = bad.join("\n");
        for expected in [
            "profile names no profile",
            "design_kit.colour",
            "roles.accent",
            "(reel.html) gives no because",
            "exempt[1] names no path",
            "S9",
            "shell is",
            "exempt[2].extra",
        ] {
            assert!(all.contains(expected), "{expected} missing from:\n{all}");
        }
        assert!(KitPolicy::from_policy_text(Some("design_kit: flowbite\n")).is_err());
        assert!(
            KitPolicy::from_policy_text(Some("a:\n\tb: 1\n")).unwrap_err()[0]
                .contains("does not parse")
        );
        assert!(
            KitPolicy::from_block(&serde_json::json!({"profile": "p", "exempt": "x"})).is_err()
        );
        assert!(KitPolicy::from_block(&serde_json::json!({"profile": "p", "roles": "x"})).is_err());
        assert!(
            KitPolicy::from_block(&serde_json::json!({"profile": "p", "layouts": "x"})).is_err()
        );
        assert!(
            KitPolicy::from_block(&serde_json::json!({"profile": "p", "exempt": ["x"]})).is_err()
        );
        assert!(KitPolicy::from_block(&serde_json::json!({"profile": "p", "exempt": [{"path": "a", "because": "b", "signals": "S1"}]})).is_err());
        assert!(
            KitPolicy::from_block(&serde_json::json!({"profile": "p", "layouts": [1]})).is_err()
        );
    }

    fn repository(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let target = dir.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, text).unwrap();
        }
        dir
    }

    #[test]
    fn inspection_reads_the_tree_the_policy_and_the_profile() {
        let share = crate::synthetic::crate_share();
        let dir = repository(&[
            ("package.json", PACKAGE),
            ("src/input.css", STYLE),
            ("src/main.js", SCRIPT),
            ("index.html", LAYOUT),
            // what a dependency carries is never read
            ("node_modules/daisyui/index.js", "import 'daisyui';"),
            (
                "dist/app.min.js",
                "querySelectorAll('[data-drawer-toggle]')",
            ),
            (
                "priv/static/assets/app.js",
                "querySelectorAll('[data-drawer-toggle]')",
            ),
        ]);
        let r = inspect(dir.path(), &share).unwrap();
        assert!(r.passes, "{r:#?}");

        std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
        std::fs::write(
            dir.path().join(POLICY),
            "design_kit:\n  profile: flowbite-pro\n  exempt:\n    - path: index.html\n",
        )
        .unwrap();
        let r = inspect(dir.path(), &share).unwrap();
        assert_eq!(r.verdict, KitVerdict::BadPolicy);
        assert!(!r.passes);
        assert!(r.policy_errors[0].contains("because"));

        std::fs::write(
            dir.path().join(POLICY),
            "design_kit:\n  profile: no-such-kit\n",
        )
        .unwrap();
        let r = inspect(dir.path(), &share).unwrap();
        assert_eq!(r.verdict, KitVerdict::BadPolicy);
        assert!(r.policy_errors[0].contains("share/design/kits/no-such-kit.yaml"));

        std::fs::remove_file(dir.path().join(POLICY)).unwrap();
        let empty = tempfile::tempdir().unwrap();
        assert!(inspect(dir.path(), empty.path())
            .unwrap_err()
            .contains("ships no"));
        assert!(inspect(&dir.path().join("absent"), &share).is_err());
    }

    const POLICY_TEXT: &str = "version: 1\n\nenforcement:\n  - name: doctor-on-commit\n    path: majordomus\n    args: [doctor]\n    wired_by: git-hook:pre-commit\n\n# the projections\nprojections:\n  - provider: agents\n    target: AGENTS.md\n";

    #[test]
    fn adoption_writes_the_rule_the_block_and_the_entries_once() {
        let share = crate::synthetic::crate_share();
        let dir = repository(&[
            (POLICY, POLICY_TEXT),
            (".github/workflows/ci.yml", "jobs: {}\n"),
        ]);
        let dry = adopt(dir.path(), &share, DEFAULT_PROFILE, true).unwrap();
        assert!(dry.dry_run);
        assert!(
            !dir.path().join(RULES_DIR).exists(),
            "a dry run writes nothing"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(POLICY)).unwrap(),
            POLICY_TEXT
        );

        let first = adopt(dir.path(), &share, DEFAULT_PROFILE, false).unwrap();
        assert_eq!(first.writes, dry.writes);
        assert_eq!(
            first.ci_workflow.as_deref(),
            Some(".github/workflows/ci.yml")
        );
        let actions: Vec<AdoptAction> = first.writes.iter().map(|w| w.action).collect();
        assert_eq!(actions, [AdoptAction::Created, AdoptAction::Updated]);
        let rule = std::fs::read_to_string(
            dir.path()
                .join(RULES_DIR)
                .join("ui-uses-flowbite-pro.v1.md"),
        )
        .unwrap();
        assert!(rule.starts_with("---\nid: project.ui-uses-flowbite-pro\n"));
        let policy = std::fs::read_to_string(dir.path().join(POLICY)).unwrap();
        let map = yaml::parse_mapping(&policy).unwrap();
        assert_eq!(map["design_kit"]["profile"], "flowbite-pro");
        let names: Vec<&str> = map["enforcement"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["name"].as_str())
            .collect();
        assert_eq!(names, ["doctor-on-commit", ON_PUSH, IN_CI]);
        assert_eq!(
            map["enforcement"][2]["wired_by"],
            "ci:.github/workflows/ci.yml"
        );
        assert_eq!(
            map["enforcement"][1]["args"],
            serde_json::json!(["design", "kit", "--check"])
        );
        assert_eq!(
            map["projections"][0]["provider"], "agents",
            "the next block is kept"
        );
        assert!(
            policy.contains("\n# the projections\nprojections:"),
            "{policy}"
        );

        let second = adopt(dir.path(), &share, DEFAULT_PROFILE, false).unwrap();
        assert!(
            second
                .writes
                .iter()
                .all(|w| w.action == AdoptAction::Unchanged),
            "{:?}",
            second.writes
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(POLICY)).unwrap(),
            policy
        );

        // a hand-edited rule is kept and named
        std::fs::write(
            dir.path()
                .join(RULES_DIR)
                .join("ui-uses-flowbite-pro.v1.md"),
            "edited",
        )
        .unwrap();
        let third = adopt(dir.path(), &share, DEFAULT_PROFILE, false).unwrap();
        assert_eq!(third.writes[0].action, AdoptAction::Kept);
        assert!(third
            .notes
            .iter()
            .any(|n| n.contains("differs from the template")));
    }

    #[test]
    fn adoption_without_a_policy_or_a_profile_is_refused() {
        let share = crate::synthetic::crate_share();
        let dir = repository(&[("README.md", "x")]);
        assert!(matches!(
            adopt(dir.path(), &share, DEFAULT_PROFILE, false),
            Err(AdoptError::Refused(_))
        ));
        assert!(matches!(
            adopt(dir.path(), &share, "no-such-kit", false),
            Err(AdoptError::NotFound(_))
        ));
        let other = repository(&[(POLICY, "design_kit:\n  profile: another\n")]);
        let err = adopt(other.path(), &share, DEFAULT_PROFILE, false).unwrap_err();
        assert!(
            matches!(&err, AdoptError::Refused(m) if m.contains("another")),
            "{err:?}"
        );
        let broken = repository(&[(POLICY, "a:\n\tb: 1\n")]);
        assert!(matches!(
            adopt(broken.path(), &share, DEFAULT_PROFILE, false),
            Err(AdoptError::Refused(_))
        ));
        let empty = tempfile::tempdir().unwrap();
        let no_template = repository(&[(
            "design/kits/flowbite-pro.yaml",
            &std::fs::read_to_string(share.join("design/kits/flowbite-pro.yaml")).unwrap(),
        )]);
        assert!(matches!(
            adopt(empty.path(), no_template.path(), DEFAULT_PROFILE, false),
            Err(AdoptError::Internal(_))
        ));
    }

    #[test]
    fn the_enforcement_block_is_extended_in_every_shape() {
        let entries = [Entry {
            name: ON_PUSH,
            wired_by: "git-hook:pre-push".into(),
        }];
        // no block at all
        let (out, actions) = policy_with_enforcement("version: 1", &entries);
        assert_eq!(actions, [(ON_PUSH.to_string(), AdoptAction::Created)]);
        assert_eq!(
            yaml::parse_mapping(&out).unwrap()["enforcement"][0]["name"],
            ON_PUSH
        );
        // an empty inline list
        let (out, _) = policy_with_enforcement("enforcement: []\nversion: 1\n", &entries);
        let map = yaml::parse_mapping(&out).unwrap();
        assert_eq!(map["enforcement"][0]["wired_by"], "git-hook:pre-push");
        assert_eq!(map["version"], 1);
        // a list at column zero, last in the file
        let (out, _) = policy_with_enforcement(
            "enforcement:\n- name: a\n  path: b\n  wired_by: manual\n",
            &entries,
        );
        let map = yaml::parse_mapping(&out).unwrap();
        assert_eq!(map["enforcement"][1]["name"], ON_PUSH, "{out}");
        // already there
        let (same, actions) = policy_with_enforcement(&out, &entries);
        assert_eq!(same, out);
        assert_eq!(actions[0].1, AdoptAction::Unchanged);
        // the block itself
        let (text, action) = policy_with_block("version: 1", "flowbite-pro").unwrap();
        assert_eq!(action, AdoptAction::Created);
        assert_eq!(
            policy_with_block(&text, "flowbite-pro").unwrap().1,
            AdoptAction::Unchanged
        );
        assert!(policy_with_block("a:\n\tb\n", "flowbite-pro").is_err());
    }

    #[test]
    fn the_ci_workflow_prefers_the_conventional_name() {
        let set = |paths: &[&str]| {
            paths
                .iter()
                .map(|p| p.to_string())
                .collect::<BTreeSet<String>>()
        };
        assert_eq!(
            ci_workflow(&set(&[
                ".github/workflows/pages.yml",
                ".github/workflows/ci.yml"
            ]))
            .as_deref(),
            Some(".github/workflows/ci.yml")
        );
        assert_eq!(
            ci_workflow(&set(&[
                ".github/workflows/pages.yml",
                ".github/workflows/deploy.yaml"
            ]))
            .as_deref(),
            Some(".github/workflows/deploy.yaml")
        );
        assert_eq!(
            ci_workflow(&set(&[".gitlab-ci.yml"])).as_deref(),
            Some(".gitlab-ci.yml")
        );
        assert_eq!(
            ci_workflow(&set(&["README.md", ".github/workflows/x/y.yml"])),
            None
        );
    }

    #[test]
    fn small_helpers_read_what_they_say() {
        assert_eq!(major("^4.1.0"), Some(4));
        assert_eq!(major("~3.4"), Some(3));
        assert_eq!(major("latest"), None);
        assert!(skipped(&["priv/static".into()], "static", "priv/static"));
        assert!(skipped(
            &["priv/static".into()],
            "static",
            "apps/x/priv/static"
        ));
        assert!(!skipped(&["priv/static".into()], "static", "site/static"));
        assert!(skipped(
            &["node_modules".into()],
            "node_modules",
            "a/node_modules"
        ));
        assert!(has_extension("a/b.HTML", "html"));
        assert!(!has_extension("Makefile", "html"));
        let tokens = class_tokens("<div class=\"bg-white dark:bg-gray-800\">");
        assert!(has_classes(&tokens, "bg-white dark:bg-gray-800"));
        assert!(!has_classes(&tokens, ""));
        assert!(mix_depends_on(
            "{:phoenix_live_view , \"1\"}",
            "phoenix_live_view"
        ));
        assert!(import_pattern(&[]).unwrap().is_none());
        let re = import_pattern(&["@radix-ui/".into(), "antd".into()])
            .unwrap()
            .unwrap();
        assert!(re.is_match("import { Dialog } from \"@radix-ui/react-dialog\";"));
        assert!(re.is_match("import 'antd/dist/reset.css';"));
        assert!(!re.is_match("import 'antdx';"));
    }

    #[test]
    fn the_report_serialises_with_the_names_a_client_reads() {
        let r = with(|f| {
            f.insert(
                "package.json",
                r#"{"devDependencies": {"tailwindcss": "^4.1.0"}}"#.into(),
            );
        });
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["verdict"], "findings");
        assert_eq!(v["findings"][0]["signal"], "S1");
        for key in ["signal", "path", "evidence", "remedy"] {
            assert!(v["findings"][0].get(key).is_some(), "{key}");
        }
        let bad = serde_json::to_value(KitReport::bad_policy("p", vec!["x".into()])).unwrap();
        assert_eq!(bad["verdict"], "bad_policy");
    }
}
