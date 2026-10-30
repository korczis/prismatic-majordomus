//! The UI kit a repository's interface is built on, judged from markers in its tree
//! (I4334, ADR 0230).
//!
//! A kit profile (`share/design/kits/<id>.yaml`, the distribution's) names the markers by
//! which a tree shows that its interface is built on one kit. They come in two kinds:
//!
//! - **wiring markers** connect the kit to a build: the package, the Tailwind plugin and
//!   source, the script that loads the kit's JavaScript;
//! - **usage markers** show the kit in use: the attributes its JavaScript drives, its React
//!   components, its JavaScript API.
//!
//! Wiring is owed and is never enough. The tree is cut into **surfaces**, one per UI manifest
//! (the root's, every declared workspace's, every nested app's), and a surface passes S1 to S3
//! only when its own files carry a usage marker. The profile also names the shell every root
//! layout carries, the packages of every other kit and, for a licensed kit, the licensed
//! markers whose use the licence evidence decides. It holds markers only and never a line of
//! the kit's own source, which for Flowbite Pro is licensed.
//!
//! The judgement is in two halves so that the half that decides is pure:
//!
//! - [`Tree::read`] is the only code that touches the filesystem: it lists the repository's
//!   files (in a Git work tree the tracked ones and the untracked ones Git does not ignore,
//!   elsewhere every file below the root), skips what the profile calls dependencies and
//!   build output, and keeps the text of the files a signal reads. A Git submodule, and a
//!   Git repository nested in the work tree that it does not track, is judged in its own
//!   repository: it is listed as a notice that names it, and nothing below it is read.
//! - [`judge`] takes that tree, the profile, the repository's policy and the design
//!   licence's [`LicenceState`] and returns a [`KitReport`]. It reads nothing else, so every
//!   signal is unit-tested on a tree built in memory. [`licence_answer`] asks the design
//!   licence (I4336) through its library function.
//!
//! The signals:
//!
//! | Signal | Asks |
//! |---|---|
//! | S0 | Is there a user interface at all, and which surfaces? None passes vacuously. |
//! | S1 | Is the kit's package a dependency of each surface, and does the surface use the kit? |
//! | S2 | Is the Tailwind plugin configured and its source scanned, not a CDN? |
//! | S3 | Is every interactive attribute driven by the kit's JavaScript, not a copy of it? |
//! | S4 | Is no other kit a dependency or an import? (Alpine is reported, not refused.) |
//! | S5 | Does every root layout carry the shell? |
//!
//! Every finding names its signal, the path, the evidence and the remedy. The policy's `ui:`
//! block, which the design licence declares, names the profile (`ui.kit`) and the
//! exemptions (`ui.exempt`); an exemption is exactly `{path, because}`, it moves every
//! finding at its path to the exempted list, which every answer prints, so an exemption is
//! never silent. The root layouts, the role aliases and which layouts are marketing pages
//! are the profile's alone. A policy that cannot be read, or an exemption without a reason,
//! is a bad policy: nothing is judged under it. A licensed marker the design licence does not
//! decide as active is a finding no exemption moves.
//!
//! The module is the crate's own (its parent is `pub(crate)`), so its examples are the unit
//! tests below: `a_conformant_tree_is_clean` is the example this header would carry.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::design::licence;
use crate::discovery::glob::Glob;
use crate::metadata::yaml;

/// Where the kit profiles live inside the distribution's share directory.
pub const KITS_DIR: &str = "design/kits";

/// The profile judged when a repository's policy declares none.
pub const DEFAULT_PROFILE: &str = "flowbite-pro";

/// The repository's policy, where the kit's block is read from.
pub const POLICY: &str = ".ai/repo/policy.yaml";

/// The policy key of the block, the design licence's `ui:` (I4336). Read only through
/// [`policy_block`].
pub const POLICY_KEY: &str = "ui";

/// The format version of a kit profile this module reads.
pub const PROFILE_SCHEMA: u64 = 1;

/// Where a repository's workflows live; a workflow can publish a directory as a site.
const WORKFLOWS_DIR: &str = ".github/workflows/";

/// A Pages deploy's artifact directory, in a workflow.
const PAGES_ARTIFACT: &str = r#"(?s)upload-pages-artifact.{0,400}?path:\s*["']?([^\s"']+)"#;

/// A deploy, build, publish or release script, a justfile or a Makefile, by its path.
const PUBLISH_SCRIPT: &str =
    r"(?i)(^|/)((deploy|publish|build|release)[\w.-]*\.(sh|m?js|ts|py)|justfile|makefile)$";

/// A directory such a script copies or deploys.
const SCRIPT_DIR: &str =
    r"(?m)(?:rsync|cp|scp|publish_dir|deploy)\b[^\n]*?\s\.?/?([\w.-]+)/(?:\*|\s|$)";

/// What a script copies that is never a site.
const NOT_PUBLISHED: [&str; 5] = ["node_modules", "deps", "_build", "tmp", ".git"];

/// A gh-pages deploy's publish directory, in a workflow.
const PUBLISH_DIR: &str = r#"(?m)\bpublish_dir:\s*["']?([^\s"'#]+)"#;

/// An entry of a pnpm workspace file's package list.
const PNPM_ENTRY: &str = r#"(?m)^\s*-\s*["']?([^"'\s]+)"#;

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
    /// Of `skip_dirs`, the vendored ones: their files are listed by path, never read.
    #[serde(default)]
    pub vendored: Vec<String>,
    /// Files larger than this are build output.
    pub max_file_bytes: u64,
    /// Extensions of markup files.
    pub markup: Vec<String>,
    /// Of `markup`, the source files whose markup is embedded, read only with the sigil.
    #[serde(default)]
    pub embedded_markup: Vec<EmbeddedMarkup>,
    /// Extensions of script files.
    pub scripts: Vec<String>,
    /// Extensions of stylesheets.
    pub styles: Vec<String>,
    /// S0, and the surfaces.
    pub ui: UiMarkers,
    /// What connects the kit to a surface's build (S1, S2, S3).
    pub wiring: WiringMarkers,
    /// What shows a surface's own files use the kit.
    pub usage: UsageMarkers,
    /// The licence a licensed kit's markers are decided by; absent for a free kit.
    #[serde(default)]
    pub licence: Option<LicenceMarkers>,
    /// S4.
    pub forbidden: Vec<KitMarker>,
    /// Reported, never refused.
    pub reported: Vec<ReportedMarker>,
    /// S5.
    pub shell: ShellMarkers,
}

/// Source files that carry markup only inside a sigil, such as Elixir's `~H`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedMarkup {
    /// The extension.
    pub extension: String,
    /// The text a file of it carries when it holds markup.
    pub sigil: String,
}

/// What shows that a repository has a user interface, and where its surfaces are.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiMarkers {
    /// The root surface's manifests, repository-relative.
    pub package_manifests: Vec<String>,
    /// The file name of a package manifest.
    pub manifest: String,
    /// The pnpm workspace file at the root.
    pub pnpm_workspace: String,
    /// A manifest below a directory of one of these names is never a nested app.
    pub not_app_dirs: Vec<String>,
    /// Extensions whose presence below a manifest makes it a nested app.
    pub app_markup: Vec<String>,
    /// A dependency on any of these is a user interface.
    pub packages: Vec<String>,
    /// The Mix project file.
    pub mix_file: String,
    /// A Mix dependency on any of these is a user interface.
    pub mix_deps: Vec<String>,
    /// A file matching any of these globs is a user interface.
    pub files: Vec<String>,
    /// What a published site is.
    pub published: PublishedMarkers,
}

/// The pages a repository publishes, which are an interface whatever builds them.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedMarkers {
    /// The conventional site directories.
    pub dirs: Vec<String>,
    /// Extensions of a page.
    pub pages: Vec<String>,
    /// Directories below a site directory that hold build output.
    pub skip: Vec<String>,
    /// The documentation directory, published only when it says so.
    pub docs: String,
    /// The files that say the documentation directory is published.
    pub docs_markers: Vec<String>,
}

/// The wiring markers.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WiringMarkers {
    /// S1.
    pub dependency: DependencyMarkers,
    /// S2.
    pub plugin: PluginMarkers,
    /// S3: what shows the kit's JavaScript is loaded.
    pub loaded: String,
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
    /// The Tailwind 3 configuration files, relative to a surface's directory.
    pub v3_configs: Vec<String>,
    /// The plugin under Tailwind 3.
    pub v3_plugin: String,
    /// The content glob under Tailwind 3.
    pub v3_content: String,
    /// The identifier a Tailwind 3 configuration binds to the kit's helper (flowbite-react's
    /// `flowbite-react/tailwind`), as capture group 1 or 2; the configuration then passes the
    /// plugin and the content only by calling `<identifier>.plugin()` and
    /// `<identifier>.content()`, never another object's method of that name.
    pub v3_binding: String,
    /// Tailwind compiled in the browser.
    pub play_cdn: String,
    /// The kit fetched from a CDN.
    pub cdn_flowbite: String,
}

/// The usage markers, as regular expressions.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageMarkers {
    /// The attributes the kit's JavaScript drives.
    pub attributes: String,
    /// A component of the kit's React binding in use.
    pub components: String,
    /// The kit's JavaScript API called by name.
    pub api: String,
    /// What shows a script reads the attributes itself.
    pub reimplemented: String,
}

/// The licence of a licensed kit: its evidence format and its licensed markers.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenceMarkers {
    /// The format of the evidence the design licence reads.
    pub evidence: String,
    /// What in a tree is the licensed distribution itself; never empty.
    pub markers: Vec<LicensedMarker>,
}

/// One licensed marker: an expression over a path, over a read file's text, or both.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicensedMarker {
    /// What it is, for a person.
    pub name: String,
    /// Matched against every path of the tree.
    #[serde(default)]
    pub path: Option<String>,
    /// Matched against the text of every file a signal reads.
    #[serde(default)]
    pub text: Option<String>,
    /// Matched against the name of every dependency a package manifest declares under
    /// dependencies, devDependencies, peerDependencies or optionalDependencies, and never
    /// against another key of the manifest.
    #[serde(default)]
    pub dependency: Option<String>,
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
    /// Its own files, as an expression over every path, vendored ones included.
    #[serde(default)]
    pub vendored: Option<String>,
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
    /// The templates an embedded-markup layout module (`layouts.ex`) draws its app and root
    /// layouts with, in its templates directory: only these are judged with it, so another
    /// template there (an email's) never supplies the shell.
    pub embedded_templates: Vec<String>,
    /// The root layouts judged as a marketing page, by a navbar and a footer, instead of the
    /// application shell: a published site's.
    pub marketing_layouts: Vec<String>,
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

/// One licensed marker, compiled.
struct LicensedRe {
    name: String,
    path: Option<Regex>,
    text: Option<Regex>,
    dependency: Option<Regex>,
}

/// The profile's expressions, compiled.
struct Compiled {
    v4_entry: Regex,
    v4_plugin: Regex,
    v4_source: Regex,
    v3_entry: Regex,
    v3_plugin: Regex,
    v3_content: Regex,
    v3_binding: Regex,
    play_cdn: Regex,
    cdn_flowbite: Regex,
    loaded: Regex,
    attributes: Regex,
    components: Regex,
    api: Regex,
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
    forbidden_vendored: Vec<Option<Regex>>,
    licensed: Vec<LicensedRe>,
    aside: Regex,
    id: Regex,
    pages_artifact: Regex,
    publish_script: Regex,
    script_dir: Regex,
    publish_dir: Regex,
    pnpm_entry: Regex,
}

fn compile(name: &str, pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|e| format!("the profile's {name} is not an expression: {e}"))
}

impl Compiled {
    fn new(p: &Profile) -> Result<Self, String> {
        let mut forbidden_imports = Vec::new();
        let mut forbidden_vendored = Vec::new();
        for kit in &p.forbidden {
            forbidden_imports.push(import_pattern(&kit.packages)?);
            forbidden_vendored.push(match &kit.vendored {
                Some(v) => Some(compile(&format!("{} vendored", kit.kit), v)?),
                None => None,
            });
        }
        let mut reported_markup = Vec::new();
        for r in &p.reported {
            reported_markup.push(match &r.markup {
                Some(m) => Some(compile(&format!("{} markup", r.kit), m)?),
                None => None,
            });
        }
        let mut licensed = Vec::new();
        if let Some(licence) = &p.licence {
            if licence.markers.is_empty() {
                return Err(
                    "the profile's licence declares no licensed markers: a profile with licence evidence names what it licenses"
                        .into(),
                );
            }
            for (n, m) in licence.markers.iter().enumerate() {
                if m.path.is_none() && m.text.is_none() && m.dependency.is_none() {
                    return Err(format!(
                        "the profile's licence.markers[{n}] ({}) has neither a path, a text nor a dependency expression",
                        m.name
                    ));
                }
                let at = |what: &str| format!("licence.markers[{n}].{what}");
                licensed.push(LicensedRe {
                    name: m.name.clone(),
                    path: m
                        .path
                        .as_deref()
                        .map(|r| compile(&at("path"), r))
                        .transpose()?,
                    text: m
                        .text
                        .as_deref()
                        .map(|r| compile(&at("text"), r))
                        .transpose()?,
                    dependency: m
                        .dependency
                        .as_deref()
                        .map(|r| compile(&at("dependency"), r))
                        .transpose()?,
                });
            }
        }
        let plugin = &p.wiring.plugin;
        Ok(Compiled {
            v4_entry: compile("wiring.plugin.v4_entry", &plugin.v4_entry)?,
            v4_plugin: compile("wiring.plugin.v4_plugin", &plugin.v4_plugin)?,
            v4_source: compile("wiring.plugin.v4_source", &plugin.v4_source)?,
            v3_entry: compile("wiring.plugin.v3_entry", &plugin.v3_entry)?,
            v3_plugin: compile("wiring.plugin.v3_plugin", &plugin.v3_plugin)?,
            v3_content: compile("wiring.plugin.v3_content", &plugin.v3_content)?,
            v3_binding: compile("wiring.plugin.v3_binding", &plugin.v3_binding)?,
            play_cdn: compile("wiring.plugin.play_cdn", &plugin.play_cdn)?,
            cdn_flowbite: compile("wiring.plugin.cdn_flowbite", &plugin.cdn_flowbite)?,
            loaded: compile("wiring.loaded", &p.wiring.loaded)?,
            attributes: compile("usage.attributes", &p.usage.attributes)?,
            components: compile("usage.components", &p.usage.components)?,
            api: compile("usage.api", &p.usage.api)?,
            reimplemented: compile("usage.reimplemented", &p.usage.reimplemented)?,
            fixed_nav: compile("shell.fixed_nav", &p.shell.fixed_nav)?,
            sidebar: compile("shell.sidebar", &p.shell.sidebar)?,
            react_sidebar: compile("shell.react_sidebar", &p.shell.react_sidebar)?,
            breadcrumb: compile("shell.breadcrumb", &p.shell.breadcrumb)?,
            react_breadcrumb: compile("shell.react_breadcrumb", &p.shell.react_breadcrumb)?,
            marketing_nav: compile("shell.marketing_nav", &p.shell.marketing_nav)?,
            marketing_footer: compile("shell.marketing_footer", &p.shell.marketing_footer)?,
            reported_markup,
            forbidden_imports,
            forbidden_vendored,
            licensed,
            aside: compile("aside", r"<aside\b[^>]*>")?,
            id: compile("id", r#"\bid=["']([^"']+)["']"#)?,
            pages_artifact: compile("pages artifact", PAGES_ARTIFACT)?,
            publish_script: compile("publish script", PUBLISH_SCRIPT)?,
            script_dir: compile("script directory", SCRIPT_DIR)?,
            publish_dir: compile("publish directory", PUBLISH_DIR)?,
            pnpm_entry: compile("pnpm entry", PNPM_ENTRY)?,
        })
    }
}

/// The expression that finds an import of any of `packages` in a script or a stylesheet:
/// the package by name, or a path through a `vendor/` or `node_modules/` directory that ends
/// in it, as a vendored copy is imported (`@plugin "../vendor/daisyui"`). A relative path
/// that is no such directory (`import './bootstrap'`) is a file of the repository's own.
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
                format!(r"{}(/[^'\x22]*|\.m?js|\.css)?", regex::escape(p))
            }
        })
        .collect();
    let pattern = format!(
        r#"(\bfrom|\bimport|\brequire\(|@plugin|@import)\s*["']((?:[^'"]*/)?(?:vendor|node_modules)/)?({})["']"#,
        names.join("|")
    );
    compile("forbidden import", &pattern).map(Some)
}

// ------------------------------------------------------------------ the licence

/// What the design licence (I4336, [`crate::design::licence`]) says about a repository,
/// as the judge of a licensed kit's markers reads it. [`licence_answer`] maps its verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LicenceState {
    /// The design licence judged the tree with the evidence held and found it clean, or the
    /// tree is private: the licence holds.
    Active,
    /// The design licence judged nothing and needed to judge nothing: no kit is declared, a
    /// private tree without evidence, or no UI file. Licensed markers there are unlicensed.
    Inactive,
    /// The design licence owed a judgement and could not give one, or could not be asked:
    /// no evidence where a public tree owes it, a `ui:` block it cannot use, a tree it
    /// cannot list.
    NotJudged,
    /// The design licence refused the tree: a public tree holds a licensed file's bytes.
    Refused,
}

/// Which variant of a licensed kit the tree is, as the judge decides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KitVariant {
    /// The tree carries no licensed marker: nothing the licence covers is used.
    NotUsed,
    /// Licensed markers, and the licence is active.
    Licensed,
    /// Licensed markers, and the licence is inactive or the design licence refused the
    /// tree: a finding.
    Unlicensed,
    /// Licensed markers, and a licence nobody could judge: the judge cannot decide, and that
    /// is a finding, never a pass.
    Undecided,
}

impl KitVariant {
    /// Whether the variant passes.
    pub fn passes(self) -> bool {
        matches!(self, KitVariant::NotUsed | KitVariant::Licensed)
    }
}

/// Decide the variant from whether the tree carries a licensed marker and what the
/// licence says. Only an active licence turns licensed markers into a pass.
pub fn decide(carries_licensed_markers: bool, state: LicenceState) -> KitVariant {
    if !carries_licensed_markers {
        return KitVariant::NotUsed;
    }
    match state {
        LicenceState::Active => KitVariant::Licensed,
        LicenceState::Inactive | LicenceState::Refused => KitVariant::Unlicensed,
        LicenceState::NotJudged => KitVariant::Undecided,
    }
}

/// The design licence's verdict as a [`LicenceState`]: active and inactive pass through,
/// refused stays refused, and not judged stays undecided.
pub fn licence_state_of(verdict: licence::Verdict) -> LicenceState {
    match verdict {
        licence::Verdict::Active => LicenceState::Active,
        licence::Verdict::Inactive => LicenceState::Inactive,
        licence::Verdict::Refused => LicenceState::Refused,
        licence::Verdict::NotJudged => LicenceState::NotJudged,
    }
}

/// What the design licence says for `profile` in the repository at `root`, and why when it
/// says, asked through its library function ([`licence::assess_root`]) with the policy's
/// `ui:` block, the evidence file `evidence` names (the `--evidence` option; in CI the file
/// the shell decoded `MAJORDOMUS_KIT_EVIDENCE_<KIT>` into) or else the evidence its
/// `ui.evidence` entries name under `$HOME`, and `MAJORDOMUS_VISIBILITY`. A free kit's
/// profile owes no answer; an evidence format the design licence does not read, a file that
/// says it is another kit's, and a question it cannot answer at all, are `not_judged`, never
/// a pass. The reason is that of an inactive or unjudged licence, or the error that stopped
/// it; it names a file and a line at most, never what a line of evidence holds.
pub fn licence_answer(
    root: &Path,
    profile: &Profile,
    evidence: Option<&Path>,
) -> (LicenceState, Option<String>) {
    let Some(markers) = &profile.licence else {
        return (
            LicenceState::NotJudged,
            Some(format!("the profile {} declares no licence", profile.id)),
        );
    };
    if markers.evidence != licence::EVIDENCE_SCHEMA {
        return (
            LicenceState::NotJudged,
            Some(format!(
                "the profile names {} evidence, which the design licence does not read",
                markers.evidence
            )),
        );
    }
    let named = evidence.map(|p| p.to_string_lossy().into_owned());
    if let (Some(path), Some(name)) = (evidence, &named) {
        // the design licence's own reading of the header; a file it cannot read or parse
        // names no kit here, and the design licence then says why it cannot judge it
        let parsed = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| licence::Evidence::parse(&text).ok());
        if let Some(kit) = parsed.as_ref().and_then(licence::Evidence::kit) {
            if kit != profile.id {
                return (
                    LicenceState::NotJudged,
                    Some(format!(
                        "{name} is the licence evidence of another kit, not of {}",
                        profile.id
                    )),
                );
            }
        }
    }
    let policy = std::fs::read_to_string(root.join(POLICY)).ok();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let tightened = std::env::var(licence::VISIBILITY_ENV).ok();
    match licence::assess_root(
        root,
        policy.as_deref(),
        named.as_deref(),
        home.as_deref(),
        tightened.as_deref(),
    ) {
        Ok(report) => (licence_state_of(report.verdict), report.reason),
        Err(e) => (LicenceState::NotJudged, Some(e.to_string())),
    }
}

/// How the tree stands against a licensed kit's licence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KitLicence {
    /// The evidence format the profile names.
    pub evidence: String,
    /// What the licence evidence said.
    pub state: LicenceState,
    /// What the judge decided.
    pub variant: KitVariant,
    /// Where each licensed marker was seen, with its name.
    pub markers: Vec<String>,
    /// Why the design licence answered as it did, when it says: the reason of an inactive
    /// or unjudged licence, or what stopped it. It names a file and a line at most, never
    /// what the evidence holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ------------------------------------------------------------------ the policy

/// The `ui:` block of a parsed policy, which the design licence (I4336) declares and this
/// judge shares with it. With [`kit_line`] (what adoption writes), this is where the policy's
/// key for the kit lives: judgement and adoption both read and write it through these two.
///
/// The block is the repository's, and two of its keys are this judge's: `ui.kit`, the
/// profile's id (empty, or absent, when no kit is adopted), and `ui.exempt`, each entry
/// exactly `{path, because}`. The root layouts and the role aliases are the profile's alone,
/// and an exemption covers every exemptable signal at its path (ADR 0230).
pub fn policy_block(map: &Map<String, Value>) -> Option<&Value> {
    map.get(POLICY_KEY)
}

/// The line adoption writes under `ui:` to name `profile`.
pub fn kit_line(profile: &str) -> String {
    format!("kit: {profile}")
}

/// A signal of the profile.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum KitSignal {
    /// The user interface exists.
    S0,
    /// The dependency, and the kit in use.
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

/// One exemption of the policy: `ui.exempt[n]`, exactly `{path, because}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KitExemption {
    /// The path or glob a finding's path matches.
    pub path: String,
    /// Why the path is not held to the kit.
    pub because: String,
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

    /// Whether this exemption moves `finding` to the exempted list: every finding at a path
    /// it covers. A finding about the repository as a whole has no path to exempt, and a
    /// licensed marker's finding never reaches this question.
    fn exempts(&self, finding: &KitFinding) -> bool {
        !finding.path.is_empty() && self.covers(&finding.path)
    }
}

/// The repository's `ui:` block, as this judge reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KitPolicy {
    /// Whether the policy adopts a kit: `ui.kit` names a profile.
    pub declared: bool,
    /// The profile judged: the one `ui.kit` names, else the default.
    pub profile: String,
    /// Its exemptions.
    pub exempt: Vec<KitExemption>,
}

impl KitPolicy {
    /// The policy of a repository that adopts no kit: the default profile, nothing exempted.
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
        match policy_block(&map) {
            None => Ok(KitPolicy::absent()),
            Some(block) => KitPolicy::from_block(block),
        }
    }

    /// Read the block itself. Only `kit` and `exempt` are read; the block's other keys are
    /// the design licence's (`visibility`, `publish`, `evidence`), and the policy schema
    /// closes the block to the keys it declares.
    pub fn from_block(block: &Value) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        let Some(map) = block.as_object() else {
            return Err(vec![format!("{POLICY_KEY} is not a mapping")]);
        };
        let profile = match map.get("kit") {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(p)) => p.trim().to_string(),
            Some(other) => {
                errors.push(format!(
                    "{POLICY_KEY}.kit is {other}, and it names a kit profile of the distribution or is empty"
                ));
                String::new()
            }
        };
        let mut exempt = Vec::new();
        match map.get("exempt") {
            None | Some(Value::Null) => {}
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
        if !errors.is_empty() {
            return Err(errors);
        }
        let declared = !profile.is_empty();
        Ok(KitPolicy {
            declared,
            profile: if declared {
                profile
            } else {
                DEFAULT_PROFILE.into()
            },
            exempt,
        })
    }
}

fn exemption(item: &Value, at: &str) -> Result<KitExemption, Vec<String>> {
    let mut errors = Vec::new();
    let Some(map) = item.as_object() else {
        return Err(vec![format!("{at} is not a mapping of path and because")]);
    };
    for key in map.keys() {
        if !["path", "because"].contains(&key.as_str()) {
            errors.push(format!(
                "{at}.{key} is not a key of an exemption, which is exactly path and because"
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
    if errors.is_empty() {
        Ok(KitExemption {
            path,
            because,
            matched: 0,
        })
    } else {
        Err(errors)
    }
}

// ------------------------------------------------------------------ the tree

/// The repository below its root, as the signals read it: every path, the text of the files
/// a signal reads, and the paths of the vendored files no signal reads.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    /// Every file, repository-relative with `/`.
    pub paths: BTreeSet<String>,
    /// The text of every file a signal reads.
    pub texts: BTreeMap<String, String>,
    /// Every file below a vendored directory of the profile, by path only.
    pub vendored: BTreeSet<String>,
    /// Every Git repository of its own inside this one, by path: a submodule (`true`) or an
    /// untracked nested repository (`false`). Nothing below one is read: a submodule is
    /// judged in its own repository, and the answer names each one in a notice.
    pub nested: BTreeMap<String, bool>,
}

/// Where a listed file stands against the profile's directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// The signals see it.
    Read,
    /// Below a vendored directory: listed by path, never read.
    Vendored,
    /// Below a dependency or build directory: not seen at all.
    Skipped,
}

/// Where `path` stands: skipped below any skipped directory that is not vendored, vendored
/// below a vendored one with nothing skipped further down.
fn placement(profile: &Profile, path: &str) -> Placement {
    let mut at = Placement::Read;
    let mut prefix = String::new();
    let segments: Vec<&str> = path.split('/').collect();
    for seg in &segments[..segments.len().saturating_sub(1)] {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(seg);
        if skipped(&profile.skip_dirs, seg, &prefix) {
            if profile.vendored.iter().any(|v| v == seg) {
                at = Placement::Vendored;
            } else {
                return Placement::Skipped;
            }
        }
    }
    at
}

/// The files of the Git work tree whose top is `root`: the tracked ones and the untracked
/// ones Git does not ignore, with the submodules Git tracks. `None` when `root` is not the
/// top of a work tree, or Git cannot say, and the walk lists the files instead.
///
/// Git lists a submodule as its gitlink, one path that is a directory on disk, and an
/// untracked repository nested in the work tree as its directory with a trailing `/`; it
/// lists nothing below either, and neither is read here.
fn git_files(root: &Path) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
    };
    let prefix = git(&["rev-parse", "--show-prefix"])?;
    if !String::from_utf8_lossy(&prefix.stdout).trim().is_empty() {
        return None;
    }
    let listed = git(&[
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ])?;
    let staged = git(&["ls-files", "-z", "--stage"])?;
    let submodules: BTreeSet<String> = staged
        .stdout
        .split(|b| *b == 0)
        .filter_map(|line| {
            let line = String::from_utf8_lossy(line);
            let (meta, path) = line.split_once('\t')?;
            meta.starts_with("160000 ").then(|| path.to_string())
        })
        .collect();
    Some((
        listed
            .stdout
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect(),
        submodules,
    ))
}

/// Every file below `root`, never descending into a skipped directory that is not vendored.
fn walked_files(root: &Path, profile: &Profile) -> std::io::Result<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if rel.is_empty() => return Err(e),
            Err(_) => continue,
        };
        // an entry whose type cannot be read is neither a directory nor a file
        let typed = entries
            .flatten()
            .filter_map(|entry| entry.file_type().ok().map(|kind| (entry, kind)));
        for (entry, kind) in typed {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if kind.is_dir() {
                let pruned =
                    skipped(&profile.skip_dirs, &name, &path) && !profile.vendored.contains(&name);
                if !pruned {
                    stack.push((entry.path(), path));
                }
            } else if kind.is_file() {
                files.insert(path);
            }
        }
    }
    Ok(files)
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

    /// List the repository's files (in a Git work tree the tracked and the untracked ones
    /// Git does not ignore, elsewhere every file below `root`), set aside the profile's
    /// skipped and vendored directories, links and oversized files, and keep the text of
    /// every file a signal reads.
    pub fn read(root: &Path, profile: &Profile) -> std::io::Result<Self> {
        let publish_script = Regex::new(PUBLISH_SCRIPT).map_err(std::io::Error::other)?;
        std::fs::read_dir(root)?;
        let (files, submodules) = match git_files(root) {
            Some(listed) => listed,
            None => (walked_files(root, profile)?, BTreeSet::new()),
        };
        let mut tree = Tree::default();
        for path in files {
            let submodule = submodules.contains(&path);
            if submodule || path.ends_with('/') {
                let path = path.trim_end_matches('/').to_string();
                if placement(profile, &format!("{path}/x")) != Placement::Skipped {
                    tree.nested.insert(path, submodule);
                }
                continue;
            }
            match placement(profile, &path) {
                Placement::Skipped => continue,
                Placement::Vendored => {
                    tree.vendored.insert(path);
                    continue;
                }
                Placement::Read => {}
            }
            let name = file_name(&path);
            if name.ends_with(".min.js") || name.ends_with(".min.css") {
                continue;
            }
            let full = root.join(&path);
            let Ok(meta) = std::fs::symlink_metadata(&full) else {
                continue;
            };
            if !meta.is_file() || meta.len() > profile.max_file_bytes {
                continue;
            }
            if read_by_a_signal(profile, &publish_script, &path) {
                if let Ok(text) = std::fs::read_to_string(&full) {
                    let embedded = profile
                        .embedded_markup
                        .iter()
                        .find(|e| has_extension(name, &e.extension));
                    if embedded.is_none_or(|e| text.contains(e.sigil.as_str())) {
                        tree.texts.insert(path.clone(), text);
                    }
                }
            }
            tree.paths.insert(path);
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

/// The directory of a repository-relative path; `""` at the root.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

/// Whether `path` lies below `dir`; everything lies below the root.
fn below(path: &str, dir: &str) -> bool {
    dir.is_empty() || path.starts_with(&format!("{dir}/"))
}

/// A workflow file of the repository.
fn is_workflow(path: &str) -> bool {
    path.strip_prefix(WORKFLOWS_DIR)
        .is_some_and(|n| !n.contains('/') && (n.ends_with(".yml") || n.ends_with(".yaml")))
}

fn read_by_a_signal(profile: &Profile, publish_script: &Regex, path: &str) -> bool {
    let name = file_name(path);
    profile.ui.package_manifests.iter().any(|m| m == path)
        || name == profile.ui.manifest
        || path == profile.ui.pnpm_workspace
        || name == profile.ui.mix_file
        || is_workflow(path)
        || publish_script.is_match(path)
        || profile
            .wiring
            .plugin
            .v3_configs
            .iter()
            .any(|c| file_name(c) == name)
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

/// How a surface was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KitSurfaceKind {
    /// The root's manifests and every file no other surface owns.
    Root,
    /// A workspace the root manifest or the pnpm workspace file declares.
    Workspace,
    /// A nested manifest with markup in its own subtree.
    App,
}

/// One surface S1 to S3 judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KitSurface {
    /// Its directory, repository-relative; empty for the root.
    pub dir: String,
    /// How it was found.
    pub kind: KitSurfaceKind,
    /// Its manifests.
    pub manifests: Vec<String>,
    /// The first usage markers its own files carry; empty is a finding.
    pub usage: Vec<String>,
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
    /// Whether the repository's policy declares the kit's block.
    pub policy_declared: bool,
    /// Why the policy cannot be read; non-empty only for `bad_policy`.
    pub policy_errors: Vec<String>,
    /// Whether the repository has a user interface (S0).
    pub ui: bool,
    /// What showed it.
    pub ui_evidence: Vec<String>,
    /// The surfaces S1 to S3 judged, each with the usage markers its own files carry.
    pub surfaces: Vec<KitSurface>,
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
    /// What S0 left out and a reader should know: a manifest with UI packages that is no
    /// surface, pages that are no interface. Never a finding.
    pub notices: Vec<String>,
    /// How the tree stands against a licensed kit's licence; absent for a free kit.
    pub licence: Option<KitLicence>,
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
            surfaces: Vec::new(),
            stacks: Vec::new(),
            layouts: Vec::new(),
            findings: Vec::new(),
            exempted: Vec::new(),
            reported: Vec::new(),
            notices: Vec::new(),
            licence: None,
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
#[derive(Debug, Clone)]
struct Dependency {
    manifest: String,
    version: String,
}

/// The dependencies `manifests` declare, the first declaration of a name winning.
fn manifest_deps<'a>(
    tree: &Tree,
    manifests: impl IntoIterator<Item = &'a String>,
) -> BTreeMap<String, Dependency> {
    let mut out = BTreeMap::new();
    for manifest in manifests {
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

/// One surface, as the judgement holds it.
#[derive(Debug, Clone)]
struct Surface {
    dir: String,
    kind: KitSurfaceKind,
    manifests: Vec<String>,
    /// What its manifests declare and, for a workspace, what the root's resolve for it.
    deps: BTreeMap<String, Dependency>,
    /// What its own manifests declare.
    own: BTreeMap<String, Dependency>,
    /// The Mix project file in its directory, if any.
    mix: Option<String>,
}

/// Surfaces are reported in the canonical order of their directories, which are unique.
impl crate::order::Ordered for Surface {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.dir, &self.dir)
    }
}

impl Surface {
    /// The path a finding about the surface as a whole names: its first manifest, its Mix
    /// project file, or the repository as a whole for a root with neither.
    fn path(&self) -> String {
        self.manifests
            .first()
            .or(self.mix.as_ref())
            .cloned()
            .unwrap_or_default()
    }

    /// The surface, for a person.
    fn label(&self) -> String {
        if self.dir.is_empty() {
            "the root surface".to_string()
        } else {
            format!("the surface {}/", self.dir)
        }
    }
}

/// What S0 found.
struct Interface {
    evidence: Vec<String>,
    surfaces: Vec<Surface>,
    /// Every manifest S1 to S4 count: the root's, every declared workspace's, every app's.
    counted: Vec<String>,
    notices: Vec<String>,
}

fn first_match(re: &Regex, text: &str) -> Option<(usize, String)> {
    re.find(text).map(|m| {
        let line = text[..m.start()].matches('\n').count() + 1;
        let shown: String = m.as_str().chars().take(80).collect();
        (line, shown)
    })
}

/// `name` in directory `dir`; `name` itself at the root.
fn in_dir(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
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

/// The workspace globs the root declares: `workspaces` in its manifest (a list, or a
/// mapping with `packages`) and the pnpm workspace file's list, each as the glob of a
/// workspace's manifest.
fn workspace_globs(tree: &Tree, profile: &Profile, re: &Compiled) -> Vec<Glob> {
    let mut globs = Vec::new();
    if let Some(json) = tree
        .text(&profile.ui.manifest)
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
    {
        let list = match json.get("workspaces") {
            Some(Value::Array(items)) => items.clone(),
            Some(Value::Object(map)) => map
                .get("packages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        globs.extend(list.iter().filter_map(Value::as_str).map(str::to_string));
    }
    if let Some(text) = tree.text(&profile.ui.pnpm_workspace) {
        globs.extend(
            re.pnpm_entry
                .captures_iter(text)
                .map(|c| c[1].to_string())
                .filter(|g| !g.starts_with('!')),
        );
    }
    globs
        .iter()
        .map(|g| {
            let g = g.trim_start_matches("./").trim_end_matches('/');
            Glob::new(&format!("{g}/{}", profile.ui.manifest))
        })
        .collect()
}

struct Judgement<'a> {
    tree: &'a Tree,
    profile: &'a Profile,
    re: Compiled,
    /// The dependencies of every counted manifest.
    deps: BTreeMap<String, Dependency>,
    surfaces: Vec<Surface>,
    findings: Vec<KitFinding>,
    reported: Vec<String>,
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

    fn is_script_or_markup(&self, path: &str) -> bool {
        self.profile
            .scripts
            .iter()
            .chain(&self.profile.markup)
            .any(|e| has_extension(path, e))
    }

    fn scripts_and_markup(&self) -> impl Iterator<Item = (&str, &str)> {
        self.tree
            .texts
            .iter()
            .filter(|(path, _)| self.is_script_or_markup(path))
            .map(|(p, t)| (p.as_str(), t.as_str()))
    }

    /// The surface that owns `path`: the deepest surface whose directory holds it, the root
    /// when none does and the root is a surface.
    fn owner(&self, path: &str) -> Option<usize> {
        let mut best: Option<(usize, usize)> = None;
        for (i, s) in self.surfaces.iter().enumerate() {
            if below(path, &s.dir) && best.is_none_or(|(_, len)| s.dir.len() > len) {
                best = Some((i, s.dir.len()));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Whether `dir` lies below a directory that holds no app (examples, fixtures, tests).
    fn not_app(&self, dir: &str) -> bool {
        let ui = &self.profile.ui;
        dir.split('/')
            .any(|seg| ui.not_app_dirs.iter().any(|n| n == seg))
    }

    /// Every Mix project file the tree carries outside a directory that holds no app, with
    /// its text: the candidates S0 makes surfaces of.
    fn mix_files(&self) -> impl Iterator<Item = (&str, &str)> {
        let mix = &self.profile.ui.mix_file;
        self.tree
            .texts
            .iter()
            .filter(move |(p, _)| file_name(p) == mix && !self.not_app(parent(p)))
            .map(|(p, t)| (p.as_str(), t.as_str()))
    }

    /// The Mix project files S4 and the stacks count, as the counted package manifests are:
    /// the root's and each surface's, never one under examples or fixtures.
    fn counted_mix(&self) -> Vec<(&str, &str)> {
        let root = self.profile.ui.mix_file.as_str();
        self.mix_files()
            .filter(|(p, _)| {
                *p == root || self.surfaces.iter().any(|s| s.mix.as_deref() == Some(*p))
            })
            .collect()
    }

    /// Whether a counted Mix project depends on Phoenix LiveView.
    fn phoenix(&self) -> bool {
        self.counted_mix().iter().any(|(_, t)| {
            self.profile
                .ui
                .mix_deps
                .iter()
                .any(|d| mix_depends_on(t, d))
        })
    }

    /// The directory of the project a nested manifest builds: the Mix project above it when
    /// the manifest is a Mix project's bundle manifest (`<dir>/assets/package.json` beside
    /// `<dir>/mix.exs`, as Phoenix keeps it), else the manifest's own directory.
    fn project_dir(&self, path: &str) -> String {
        let ui = &self.profile.ui;
        for m in ui.package_manifests.iter().filter(|m| **m != ui.manifest) {
            if let Some(dir) = path.strip_suffix(&format!("/{m}")) {
                if self.tree.paths.contains(&in_dir(dir, &ui.mix_file)) {
                    return dir.to_string();
                }
            }
        }
        parent(path).to_string()
    }

    /// The files of `extensions` surface `i` owns.
    fn owned<'b>(
        &'b self,
        i: usize,
        extensions: &'b [String],
    ) -> impl Iterator<Item = (&'b str, &'b str)> + 'b {
        self.tree
            .with_extension(extensions)
            .filter(move |(p, _)| self.owner(p) == Some(i))
    }

    fn owned_scripts_and_markup(&self, i: usize) -> impl Iterator<Item = (&str, &str)> {
        self.scripts_and_markup()
            .filter(move |(p, _)| self.owner(p) == Some(i))
    }

    fn react_in(&self, deps: &BTreeMap<String, Dependency>) -> bool {
        self.profile
            .wiring
            .dependency
            .react_when
            .iter()
            .any(|p| deps.contains_key(p))
    }

    fn react(&self) -> bool {
        self.react_in(&self.deps)
    }

    /// S0: whether there is a user interface, which surfaces it has, and what showed it.
    fn interface(&self) -> Interface {
        let ui = &self.profile.ui;
        let tree = self.tree;
        let root_manifests: Vec<String> = ui
            .package_manifests
            .iter()
            .filter(|m| tree.paths.contains(*m))
            .cloned()
            .collect();
        let globs = workspace_globs(tree, self.profile, &self.re);
        let mut evidence = Vec::new();
        let mut surfaces = Vec::new();
        let mut counted = root_manifests.clone();
        let mut notices = Vec::new();
        let not_app = |dir: &str| {
            dir.split('/')
                .any(|seg| ui.not_app_dirs.iter().any(|n| n == seg))
        };
        for path in &tree.paths {
            if file_name(path) != ui.manifest || ui.package_manifests.contains(path) {
                continue;
            }
            let workspace = globs.iter().any(|g| g.matches(path));
            let project = self.project_dir(path);
            let dir = if workspace {
                parent(path)
            } else {
                project.as_str()
            };
            let app = !workspace
                && !not_app(dir)
                && tree
                    .paths
                    .iter()
                    .any(|f| below(f, dir) && ui.app_markup.iter().any(|e| has_extension(f, e)));
            if workspace || app {
                counted.push(path.clone());
            }
            let deps = manifest_deps(tree, [path]);
            let packages: Vec<&str> = ui
                .packages
                .iter()
                .filter(|p| deps.contains_key(*p))
                .map(String::as_str)
                .collect();
            if packages.is_empty() {
                continue;
            }
            if !(workspace || app) {
                notices.push(format!(
                    "{path} declares {} and is no surface: it is no declared workspace, and no markup ({}) lies in its own subtree outside {}; S1 to S4 do not count it",
                    packages.join(", "),
                    ui.app_markup.join(", "),
                    ui.not_app_dirs.join(", ")
                ));
                continue;
            }
            let (kind, how) = if workspace {
                (KitSurfaceKind::Workspace, "a declared workspace")
            } else {
                (KitSurfaceKind::App, "an app with markup in its own subtree")
            };
            evidence.push(format!("{path} declares {} ({how})", packages.join(", ")));
            let own = deps.clone();
            let mut deps = deps;
            if workspace {
                // a workspace resolves what the root declares
                for (name, d) in manifest_deps(tree, &root_manifests) {
                    deps.entry(name).or_insert(d);
                }
            }
            let mix = Some(in_dir(dir, &ui.mix_file)).filter(|m| tree.paths.contains(m));
            match surfaces.iter_mut().find(|s: &&mut Surface| s.dir == dir) {
                Some(s) => {
                    s.manifests.push(path.clone());
                    for (name, d) in deps {
                        s.deps.entry(name).or_insert(d);
                    }
                    for (name, d) in own {
                        s.own.entry(name).or_insert(d);
                    }
                }
                None => surfaces.push(Surface {
                    dir: dir.to_string(),
                    kind,
                    manifests: vec![path.clone()],
                    deps,
                    own,
                    mix,
                }),
            }
        }
        // a nested Mix project on Phoenix LiveView is an app of its own, with its bundle's
        // manifest when it has one
        for (path, text) in self.mix_files() {
            let dir = parent(path);
            if dir.is_empty() || not_app(dir) || surfaces.iter().any(|s| s.dir == dir) {
                continue;
            }
            let live: Vec<&str> = ui
                .mix_deps
                .iter()
                .filter(|d| mix_depends_on(text, d))
                .map(String::as_str)
                .collect();
            if live.is_empty() {
                continue;
            }
            evidence.push(format!(
                "{path} depends on {} (a Mix project of its own)",
                live.join(", ")
            ));
            let manifests: Vec<String> = ui
                .package_manifests
                .iter()
                .map(|m| in_dir(dir, m))
                .filter(|m| tree.paths.contains(m))
                .collect();
            counted.extend(manifests.iter().cloned());
            let deps = manifest_deps(tree, &manifests);
            surfaces.push(Surface {
                dir: dir.to_string(),
                kind: KitSurfaceKind::App,
                manifests,
                own: deps.clone(),
                deps,
                mix: Some(path.to_string()),
            });
        }
        crate::order::canonical(&mut surfaces);
        let nested = |p: &str| surfaces.iter().any(|s| below(p, &s.dir));
        let root_deps = manifest_deps(tree, &root_manifests);
        let mut root_evidence = Vec::new();
        for package in &ui.packages {
            if let Some(d) = root_deps.get(package) {
                root_evidence.push(format!("{} declares {package}", d.manifest));
            }
        }
        if let Some(text) = tree.text(&ui.mix_file) {
            for dep in &ui.mix_deps {
                if mix_depends_on(text, dep) {
                    root_evidence.push(format!("{} depends on {dep}", ui.mix_file));
                }
            }
        }
        let root_mix = Some(ui.mix_file.clone()).filter(|m| tree.paths.contains(m));
        for glob in &ui.files {
            let g = Glob::new(glob);
            if let Some(path) = tree.paths.iter().find(|p| g.matches(p) && !nested(p)) {
                root_evidence.push(format!("{path} matches {glob}"));
            }
        }
        let (dirs, pages) = self.published();
        let pages: Vec<&String> = pages.iter().filter(|p| !nested(p)).collect();
        if let Some(first) = pages.first() {
            root_evidence.push(format!(
                "{} published page(s) ({}), such as {first}",
                pages.len(),
                dirs.join(", ")
            ));
        }
        if !root_evidence.is_empty() {
            surfaces.insert(
                0,
                Surface {
                    dir: String::new(),
                    kind: KitSurfaceKind::Root,
                    manifests: root_manifests,
                    own: root_deps.clone(),
                    deps: root_deps,
                    mix: root_mix,
                },
            );
        }
        root_evidence.append(&mut evidence);
        if root_evidence.is_empty() {
            let pages: Vec<&String> = tree
                .paths
                .iter()
                .filter(|p| ui.app_markup.iter().any(|e| has_extension(p, e)))
                .collect();
            if let Some(first) = pages.first() {
                notices.push(format!(
                    "{} markup file(s), such as {first}, and none of them is an interface by S0 (a published page, a surface's manifest, a template or a layout); if one is a product's interface, publish it or give it a manifest",
                    pages.len()
                ));
            }
        }
        Interface {
            evidence: root_evidence,
            surfaces,
            counted,
            notices,
        }
    }

    /// The directories the repository publishes as a site, and every page in them: the
    /// conventional site directories, an upload-pages-artifact step's path, a directory a
    /// deploy or build script copies, docs/ when it says it is published, and a root
    /// `index.html` always.
    fn published(&self) -> (Vec<String>, Vec<String>) {
        let p = &self.profile.ui.published;
        let mut dirs: BTreeSet<String> = p.dirs.iter().cloned().collect();
        for (path, text) in &self.tree.texts {
            if is_workflow(path) {
                let artifacts = self.re.pages_artifact.captures_iter(text);
                for c in artifacts.chain(self.re.publish_dir.captures_iter(text)) {
                    let d = c[1].trim_start_matches("./").trim_end_matches('/');
                    dirs.insert(if d.is_empty() { ".".into() } else { d.into() });
                }
            } else if self.re.publish_script.is_match(path) {
                for c in self.re.script_dir.captures_iter(text) {
                    if !NOT_PUBLISHED.contains(&&c[1]) {
                        dirs.insert(c[1].to_string());
                    }
                }
            }
        }
        if p.docs_markers.iter().any(|m| self.tree.paths.contains(m)) {
            dirs.insert(p.docs.clone());
        }
        let pages: Vec<String> = self
            .tree
            .paths
            .iter()
            .filter(|f| p.pages.iter().any(|e| has_extension(f, e)))
            .filter(|f| !p.skip.iter().any(|s| below(f, s)))
            .filter(|f| f.as_str() == "index.html" || dirs.iter().any(|d| d == "." || below(f, d)))
            .cloned()
            .collect();
        let used: Vec<String> = dirs
            .into_iter()
            .filter(|d| pages.iter().any(|f| d == "." || below(f, d)))
            .chain(
                pages
                    .iter()
                    .any(|f| f == "index.html")
                    .then_some("index.html".to_string()),
            )
            .collect();
        (used, pages)
    }

    /// The Tailwind major version surface `i` builds with, if any.
    fn tailwind(&self, i: usize) -> Option<u64> {
        let s = &self.surfaces[i];
        if let Some(v) = s.deps.get("tailwindcss").and_then(|d| major(&d.version)) {
            return Some(v);
        }
        let styles: Vec<(&str, &str)> = self.owned(i, &self.profile.styles).collect();
        if styles.iter().any(|(_, t)| self.re.v4_entry.is_match(t)) {
            return Some(4);
        }
        let v3_config = !self.v3_configs(i).is_empty();
        if v3_config || styles.iter().any(|(_, t)| self.re.v3_entry.is_match(t)) {
            return Some(3);
        }
        None
    }

    /// The Tailwind 3 configurations of surface `i`, relative to its directory and to the
    /// directory of each of its manifests, in that order and each once.
    fn v3_configs(&self, i: usize) -> Vec<String> {
        let s = &self.surfaces[i];
        let mut dirs = vec![s.dir.as_str()];
        dirs.extend(s.manifests.iter().map(|m| parent(m)));
        let mut out: Vec<String> = Vec::new();
        for dir in dirs {
            for c in &self.profile.wiring.plugin.v3_configs {
                let path = in_dir(dir, c);
                if self.tree.paths.contains(&path) && !out.contains(&path) {
                    out.push(path);
                }
            }
        }
        out
    }

    fn s1(&mut self, i: usize) {
        let s = self.surfaces[i].clone();
        let dep = &self.profile.wiring.dependency;
        // the binding is decided by what the surface itself declares; the root's
        // dependencies only resolve the one it wants
        let (wanted, why) = if self.react_in(&s.own) {
            (
                dep.react_package.clone(),
                "React and Next.js take the kit's React binding",
            )
        } else {
            (dep.package.clone(), "the kit's package")
        };
        if s.deps.contains_key(&wanted) {
            return;
        }
        let (path, evidence) = match s.manifests.first() {
            Some(m) => (m.clone(), format!("{m} does not declare {wanted} ({why})")),
            None => (
                s.path(),
                format!(
                    "no package manifest ({}) declares {wanted}",
                    self.profile
                        .ui
                        .package_manifests
                        .iter()
                        .map(|m| in_dir(&s.dir, m))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ),
        };
        let remedy = format!("add {wanted} to the dependencies of the manifest that builds the interface (npm install {wanted})");
        self.finding(KitSignal::S1, &path, evidence, &remedy);
    }

    /// The usage markers surface `i`'s own files carry, and a finding when there is none:
    /// wiring alone never passes S1 to S3.
    fn usage(&mut self, i: usize) -> Vec<String> {
        let mut seen = Vec::new();
        for (path, text) in self.owned_scripts_and_markup(i) {
            for re in [&self.re.attributes, &self.re.components, &self.re.api] {
                if let Some((line, shown)) = first_match(re, text) {
                    seen.push(format!("{shown} in {path}:{line}"));
                    break;
                }
            }
            if seen.len() >= 3 {
                break;
            }
        }
        if seen.is_empty() {
            let s = &self.surfaces[i];
            let (path, label) = (s.path(), s.label());
            self.finding(
                KitSignal::S1,
                &path,
                format!(
                    "{label} uses the kit in none of its own files: no attribute Flowbite's JavaScript drives, no flowbite-react component and no call of Flowbite's API; wiring alone does not pass S1 to S3"
                ),
                "build the surface's pages from the Flowbite Pro blocks and components, whose data-* attributes or flowbite-react components are what shows the kit in use",
            );
        }
        seen
    }

    fn s2(&mut self, i: usize, tailwind: Option<u64>) {
        let at = self.surfaces[i].path();
        match tailwind {
            Some(4) => self.s2_v4(i),
            Some(3) => self.s2_v3(i),
            Some(other) => self.finding(
                KitSignal::S2,
                &at,
                format!("tailwindcss {other} is neither Tailwind 3 nor Tailwind 4"),
                "build with Tailwind 4 and Flowbite 4 (ADR 0230)",
            ),
            None => self.finding(
                KitSignal::S2,
                &at,
                format!(
                    "no stylesheet imports Tailwind in {} and no package declares tailwindcss",
                    self.surfaces[i].label()
                ),
                "build the interface with Tailwind 4: a stylesheet with @import \"tailwindcss\", @plugin \"flowbite/plugin\" and @source over flowbite",
            ),
        }
        let deps = &self.surfaces[i].deps;
        let deps_have_kit = deps.contains_key(&self.profile.wiring.dependency.package)
            || deps.contains_key(&self.profile.wiring.dependency.react_package);
        let mut cdn = Vec::new();
        for (path, text) in self.owned(i, &self.profile.markup) {
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

    fn s2_v4(&mut self, i: usize) {
        let styles: Vec<(String, String)> = self
            .owned(i, &self.profile.styles)
            .map(|(p, t)| (p.to_string(), t.to_string()))
            .collect();
        let found = styles
            .iter()
            .find(|(_, t)| self.re.v4_entry.is_match(t))
            .map(|(p, _)| p.clone());
        let at = found.clone().unwrap_or_else(|| "no stylesheet".to_string());
        // without an entry stylesheet the finding is the surface's, at its manifest
        let entry = found.unwrap_or_else(|| self.surfaces[i].path());
        if !styles.iter().any(|(_, t)| self.re.v4_plugin.is_match(t)) {
            self.finding(
                KitSignal::S2,
                &entry,
                format!("Tailwind 4, and {at} carries no @plugin \"flowbite/plugin\""),
                "add @plugin \"flowbite/plugin\"; to the stylesheet that imports Tailwind",
            );
        }
        if !styles.iter().any(|(_, t)| self.re.v4_source.is_match(t)) {
            self.finding(
                KitSignal::S2,
                &entry,
                format!("Tailwind 4, and {at} carries no @source over flowbite"),
                "add @source \"../node_modules/flowbite\"; (the path from the stylesheet to the package) so Tailwind sees Flowbite's classes",
            );
        }
    }

    fn s2_v3(&mut self, i: usize) {
        let configs: Vec<(String, String)> = self
            .v3_configs(i)
            .into_iter()
            .filter_map(|c| self.tree.text(&c).map(|t| (c.clone(), t.to_string())))
            .collect();
        let Some((path, text)) = configs.first().cloned() else {
            self.finding(
                KitSignal::S2,
                &self.surfaces[i].path(),
                format!(
                    "Tailwind 3, and no configuration ({}) exists in {}",
                    self.profile.wiring.plugin.v3_configs.join(", "),
                    self.surfaces[i].label()
                ),
                "add tailwind.config.js with require('flowbite/plugin') and a content glob over flowbite, or move to Tailwind 4 (ADR 0230)",
            );
            return;
        };
        let (bound_plugin, bound_content) = self.bound_helper_calls(&text);
        if !self.re.v3_plugin.is_match(&text) && !bound_plugin {
            self.finding(
                KitSignal::S2,
                &path,
                format!("Tailwind 3, and {path} does not require flowbite/plugin"),
                "add require('flowbite/plugin') to plugins",
            );
        }
        if !self.re.v3_content.is_match(&text) && !bound_content {
            self.finding(
                KitSignal::S2,
                &path,
                format!("Tailwind 3, and {path} has no content glob over flowbite"),
                "add './node_modules/flowbite/**/*.js' to content",
            );
        }
    }

    /// Whether a Tailwind 3 configuration calls `.plugin()` and `.content()` on the very
    /// identifier it binds to the kit's helper (`v3_binding`). The `regex` crate has no
    /// backreference, so the identifier is captured first and the calls are then matched on
    /// it by name: another object's `.plugin()` (`forms.plugin()`) passes nothing.
    fn bound_helper_calls(&self, text: &str) -> (bool, bool) {
        let mut plugin = false;
        let mut content = false;
        let names = self
            .re
            .v3_binding
            .captures_iter(text)
            .filter_map(|c| c.get(1).or_else(|| c.get(2)));
        for name in names {
            let name = regex::escape(name.as_str());
            let call = |method: &str, args: &str| {
                Regex::new(&format!(
                    r"(^|[^\w.$]){name}\s*\.\s*{method}\(\s*{args}\s*\)"
                ))
                .is_ok_and(|re| re.is_match(text))
            };
            plugin |= call("plugin", "");
            content |= call("content", r"(\{[^}]*\})?");
        }
        (plugin, content)
    }

    fn s3(&mut self, i: usize) {
        let carriers: Vec<(String, String)> = self
            .owned(i, &self.profile.markup)
            .filter_map(|(path, text)| {
                first_match(&self.re.attributes, text)
                    .map(|(line, seen)| (path.to_string(), format!("{seen} in {path}:{line}")))
            })
            .collect();
        let loaded = self
            .owned_scripts_and_markup(i)
            .any(|(_, t)| self.re.loaded.is_match(t));
        if let Some((first, shown)) = carriers.first().cloned() {
            if !loaded {
                let evidence = format!(
                    "{} file(s) of {} carry attributes Flowbite's JavaScript drives (first: {shown}) and none of its scripts loads Flowbite",
                    carriers.len(),
                    self.surfaces[i].label()
                );
                self.finding(
                    KitSignal::S3,
                    &first,
                    evidence,
                    "import 'flowbite' in the bundle the layout loads (Phoenix: flowbite/dist/flowbite.phoenix.js)",
                );
            }
        }
    }

    /// S3's other half, over every script: a script that reads the attributes itself.
    fn s3_copies(&mut self) {
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
            for (mix, text) in self.counted_mix() {
                for package in &kit.mix {
                    if mix_depends_on(text, package) {
                        found.push((
                            mix.to_string(),
                            format!("{mix} depends on {package} ({})", kit.kit),
                        ));
                    }
                }
            }
            if let Some(re) = &self.re.forbidden_vendored[n] {
                let files = self.tree.paths.iter().chain(&self.tree.vendored);
                for path in files.filter(|p| re.is_match(p)) {
                    found.push((
                        path.clone(),
                        format!("{path} is {}'s own file, vendored", kit.kit),
                    ));
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

    /// Whether `path` is a source file whose markup is embedded in a sigil.
    fn embedded(&self, path: &str) -> bool {
        self.profile
            .embedded_markup
            .iter()
            .any(|e| has_extension(path, &e.extension))
    }

    /// The directory whose templates an embedded-markup layout draws with: the layout's path
    /// without its extension (`layouts.ex` embeds `layouts/*.html.heex`).
    fn templates_of(layout: &str) -> &str {
        layout
            .rsplit_once('.')
            .map(|(stem, _)| stem)
            .unwrap_or(layout)
    }

    /// The root layouts, from the profile. An embedded-markup layout is one only when it
    /// carries the sigil, and the templates of its directory are then no layouts of their
    /// own: the app and root templates are judged with it, and the rest with nothing.
    fn layouts(&self) -> Vec<String> {
        let globs = &self.profile.shell.layouts;
        let compiled: Vec<Glob> = globs.iter().map(|g| Glob::new(g)).collect();
        let matched: Vec<String> = self
            .tree
            .paths
            .iter()
            .filter(|p| compiled.iter().any(|g| g.matches(p)))
            .filter(|p| !self.embedded(p) || self.tree.text(p).is_some())
            .cloned()
            .collect();
        let embedding: Vec<&str> = matched
            .iter()
            .filter(|p| self.embedded(p))
            .map(|p| Self::templates_of(p))
            .collect();
        matched
            .iter()
            .filter(|p| !embedding.iter().any(|dir| parent(p) == *dir))
            .cloned()
            .collect()
    }

    fn s5(&mut self, layouts: &[String]) {
        if layouts.is_empty() {
            let globs = self.profile.shell.layouts.join(", ");
            self.finding(
                KitSignal::S5,
                "",
                format!("no root layout matches {globs}"),
                &format!(
                    "draw the shell in a root layout at a path the profile names ({globs}); the layouts are the profile's, and the policy declares none"
                ),
            );
            return;
        }
        let breadcrumb_anywhere = self
            .markup()
            .any(|(_, t)| self.re.breadcrumb.is_match(t) || self.re.react_breadcrumb.is_match(t));
        let react = self.react();
        for layout in layouts {
            let mut text = self.tree.text(layout).unwrap_or("").to_string();
            if self.embedded(layout) {
                let dir = Self::templates_of(layout);
                for name in &self.profile.shell.embedded_templates {
                    if let Some(t) = self.tree.text(&in_dir(dir, name)) {
                        text.push('\n');
                        text.push_str(t);
                    }
                }
            } else if layout.ends_with(".html.heex") {
                let dir = parent(layout);
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
            let marketing = self
                .profile
                .shell
                .marketing_layouts
                .iter()
                .any(|g| Glob::new(g).matches(layout));
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
            if !alternatives.iter().any(|a| has_classes(&tokens, a)) {
                missing.push(format!("{role} ({})", alternatives.join(" | ")));
            }
        }
        if !missing.is_empty() {
            self.finding(
                KitSignal::S5,
                layout,
                format!("{layout} does not carry the role(s) {}", missing.join(", ")),
                "use the Pro grays for page, surface and border, or the token rule's role aliases the profile names; a brand colour is the primary scale (ADR 0230)",
            );
        }
    }

    /// The licence of a licensed kit: where the tree carries a licensed marker, and what the
    /// judge decides from the evidence. The findings it returns are no exemption's to move.
    fn licence(&self, state: LicenceState) -> Option<(KitLicence, Vec<KitFinding>)> {
        let licence = self.profile.licence.as_ref()?;
        let mut markers = Vec::new();
        let mut first_path = String::new();
        for m in &self.re.licensed {
            if let Some(re) = &m.path {
                // a vendored copy of the distribution is the case the markers exist for
                let mut paths = self.tree.paths.iter().chain(&self.tree.vendored);
                if let Some(p) = paths.find(|p| re.is_match(p)) {
                    markers.push(format!("{p} ({})", m.name));
                    if first_path.is_empty() {
                        first_path = p.clone();
                    }
                }
            }
            if let Some(re) = &m.text {
                for (path, text) in &self.tree.texts {
                    if let Some((line, _)) = first_match(re, text) {
                        markers.push(format!("{path}:{line} ({})", m.name));
                        if first_path.is_empty() {
                            first_path = path.clone();
                        }
                        break;
                    }
                }
            }
            if let Some(re) = &m.dependency {
                let manifests = self
                    .tree
                    .texts
                    .keys()
                    .filter(|p| file_name(p) == self.profile.ui.manifest);
                'manifests: for manifest in manifests {
                    for name in manifest_deps(self.tree, [manifest]).keys() {
                        if re.is_match(name) {
                            markers.push(format!("{manifest} declares {name} ({})", m.name));
                            if first_path.is_empty() {
                                first_path = manifest.clone();
                            }
                            break 'manifests;
                        }
                    }
                }
            }
        }
        let variant = decide(!markers.is_empty(), state);
        let mut findings = Vec::new();
        let why = (!variant.passes()).then_some(match (variant, state) {
            (_, LicenceState::Refused) => {
                "the design licence refused the tree: a public tree holds a licensed file's bytes"
            }
            (KitVariant::Unlicensed, _) => {
                "the design licence is inactive (no kit declared, a private tree without evidence, or no UI file), so they are unlicensed"
            }
            _ => "the design licence could not judge the tree, so the judge cannot decide the variant",
        });
        if let Some(why) = why {
            findings.push(KitFinding {
                signal: KitSignal::S1,
                path: first_path,
                evidence: format!(
                    "the tree carries licensed markers of {} ({}) and {why}; an undecided licence is never a pass",
                    self.profile.title,
                    markers.join("; ")
                ),
                remedy: format!(
                    "keep the licensed distribution out of the tree and adapt its markup into the product, or declare the kit under ui.kit and provide the {} evidence the design licence reads (majordomus design licence)",
                    licence.evidence
                ),
            });
        }
        Some((
            KitLicence {
                evidence: licence.evidence.clone(),
                state,
                variant,
                markers,
                reason: None,
            },
            findings,
        ))
    }
}

/// Judge a tree against a profile under a policy, with what the licence evidence says.
/// Pure: it reads only its arguments.
pub fn judge(
    tree: &Tree,
    profile: &Profile,
    policy: &KitPolicy,
    licence: LicenceState,
) -> Result<KitReport, String> {
    let re = Compiled::new(profile)?;
    let mut j = Judgement {
        tree,
        profile,
        re,
        deps: BTreeMap::new(),
        surfaces: Vec::new(),
        findings: Vec::new(),
        reported: Vec::new(),
    };
    let interface = j.interface();
    j.deps = manifest_deps(tree, &interface.counted);
    j.surfaces = interface.surfaces;
    let licence = j.licence(licence);
    let mut report = KitReport {
        profile: profile.id.clone(),
        title: profile.title.clone(),
        profile_source: profile_source(&profile.id),
        rule: profile.rule.clone(),
        policy_declared: policy.declared,
        policy_errors: Vec::new(),
        ui: !interface.evidence.is_empty(),
        ui_evidence: interface.evidence,
        surfaces: Vec::new(),
        stacks: Vec::new(),
        layouts: Vec::new(),
        findings: Vec::new(),
        exempted: Vec::new(),
        reported: Vec::new(),
        notices: interface.notices,
        licence: licence.as_ref().map(|(l, _)| l.clone()),
        exemptions: policy.exempt.clone(),
        verdict: KitVerdict::NoUi,
        passes: true,
    };
    for (path, submodule) in &tree.nested {
        let what = if *submodule {
            "a Git submodule"
        } else {
            "a Git repository nested in this one and not tracked by it"
        };
        report.notices.push(format!(
            "{path} is {what}: it is judged in its own repository (majordomus design kit --repo {path}), and nothing below it is judged here"
        ));
    }
    let unexemptable = licence.map(|(_, f)| f).unwrap_or_default();
    if !report.ui {
        if !unexemptable.is_empty() {
            report.findings = unexemptable;
            report.verdict = KitVerdict::Findings;
            report.passes = false;
        }
        return Ok(report);
    }
    for i in 0..j.surfaces.len() {
        if let Some(v) = j.tailwind(i) {
            let stack = format!("tailwind-{v}");
            if !report.stacks.contains(&stack) {
                report.stacks.push(stack);
            }
        }
    }
    if j.react() {
        report.stacks.push("react".into());
    }
    if j.phoenix() {
        report.stacks.push("phoenix".into());
    }
    for i in 0..j.surfaces.len() {
        j.s1(i);
        let usage = j.usage(i);
        let tailwind = j.tailwind(i);
        j.s2(i, tailwind);
        j.s3(i);
        let s = &j.surfaces[i];
        report.surfaces.push(KitSurface {
            dir: s.dir.clone(),
            kind: s.kind,
            manifests: s.manifests.clone(),
            usage,
        });
    }
    j.s3_copies();
    j.s4();
    let layouts = j.layouts();
    j.s5(&layouts);
    report.layouts = layouts;
    report.reported = std::mem::take(&mut j.reported);
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
    report.findings.extend(unexemptable);
    report.verdict = if report.findings.is_empty() {
        KitVerdict::Clean
    } else {
        KitVerdict::Findings
    };
    report.passes = report.findings.is_empty();
    Ok(report)
}

// ------------------------------------------------------------------ inspection

/// Read a repository's policy, the profile it names, its tree and its licence evidence, and
/// judge it.
///
/// A policy that cannot be read, or that names a profile the distribution does not ship,
/// is answered as `bad_policy` with every reason; the error side is for what is never the
/// repository's fault: the distribution's own profile that cannot be read, a root that
/// cannot be walked.
pub fn inspect(root: &Path, share: &Path, evidence: Option<&Path>) -> Result<KitReport, String> {
    let policy_text = match std::fs::read_to_string(root.join(POLICY)) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{POLICY}: {e}")),
    };
    let retired = retired_notice(policy_text.as_deref());
    let with_notice = |mut report: KitReport| {
        report.notices.extend(retired.clone());
        report
    };
    let policy = match KitPolicy::from_policy_text(policy_text.as_deref()) {
        Ok(policy) => policy,
        Err(errors) => return Ok(with_notice(KitReport::bad_policy(DEFAULT_PROFILE, errors))),
    };
    let profile = match Profile::load(share, &policy.profile) {
        Ok(profile) => profile,
        Err(ProfileError::Unknown(id)) if policy.declared => {
            let reason = format!(
                "{POLICY_KEY}.kit names {id}, and the distribution ships no {}",
                profile_source(&id)
            );
            return Ok(with_notice(KitReport::bad_policy(&id, vec![reason])));
        }
        Err(ProfileError::Unknown(id)) => {
            return Err(format!("the distribution ships no {}", profile_source(&id)))
        }
        Err(ProfileError::Invalid(reason)) => return Err(reason),
    };
    let tree = Tree::read(root, &profile).map_err(|e| format!("{}: {e}", root.display()))?;
    let (licence, reason) = licence_answer(root, &profile, evidence);
    let mut report = judge(&tree, &profile, &policy, licence)?;
    if let Some(l) = &mut report.licence {
        l.reason = reason;
    }
    Ok(with_notice(report))
}

/// The policy key the kit lived under before the design licence's `ui:` block held it.
pub const RETIRED_POLICY_KEY: &str = "design_kit";

/// The notice a policy that still carries the retired top-level `design_kit:` block earns:
/// nothing reads it, and `ui.kit` and `ui.exempt` are where its kit and exemptions belong.
/// A policy that does not parse earns none here; its reading says why.
pub fn retired_notice(policy_text: Option<&str>) -> Option<String> {
    let map = yaml::parse_mapping(policy_text?).ok()?;
    map.contains_key(RETIRED_POLICY_KEY).then(|| {
        format!(
            "{POLICY} still carries the retired {RETIRED_POLICY_KEY}: block, which nothing reads; its replacement is {POLICY_KEY}.kit (and {POLICY_KEY}.exempt for exemptions): move the kit there and remove the block"
        )
    })
}

// ------------------------------------------------------------------ adoption

/// The rule directory a profile's rule is written into.
pub const RULES_DIR: &str = ".ai/repo/rules/project";

/// The enforcement entry that runs the check before every push.
pub const ON_PUSH: &str = "design-kit-on-push";

/// The enforcement entry that runs the check in CI.
pub const IN_CI: &str = "design-kit";

/// The heading of the adopted rule's last section, which is the repository's own: adoption
/// writes it from the template once, and a later adoption keeps it as the repository wrote
/// it, whatever else of the rule it brings up to the template.
pub const SLOT_HEADING: &str = "# In this repository";

/// A rule's text split at [`SLOT_HEADING`]: what the distribution owns, and the slot from its
/// heading line to the end of the file when the text has one.
pub fn split_slot(text: &str) -> (&str, Option<&str>) {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == SLOT_HEADING {
            return (&text[..at], Some(&text[at..]));
        }
        at += line.len();
    }
    (text, None)
}

/// What adoption does to a rule that is already there, and the text it writes, if any: the
/// template's text with the repository's slot kept. A rule without a slot is kept and named,
/// because nothing tells adoption which of its text is the repository's.
fn adopted_rule(current: &str, template: &str) -> (AdoptAction, Option<String>) {
    let (theirs, _) = split_slot(template);
    match split_slot(current) {
        _ if current == template => (AdoptAction::Unchanged, None),
        (ours, Some(_)) if ours == theirs => (AdoptAction::Unchanged, None),
        (_, Some(slot)) => (AdoptAction::Updated, Some(format!("{theirs}{slot}"))),
        (_, None) => (AdoptAction::Kept, None),
    }
}

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

/// The policy with `ui.kit` naming `profile`: set in the `ui:` block when the block names
/// no kit, the block appended when there is none, and refused when it names another kit.
/// Only the `kit` line is written; `ui.exempt` and the block's other keys are the
/// repository's, and are left as they are.
pub fn policy_with_kit(text: &str, profile: &str) -> Result<(String, AdoptAction), String> {
    let map = yaml::parse_mapping(text).map_err(|e| format!("{POLICY} does not parse: {e}"))?;
    let Some(block) = policy_block(&map) else {
        let mut out = text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!(
            "\n# The user interface and its kit (I4336). `majordomus design kit adopt {profile}` wrote\n\
             # `kit` (rule project.ui-uses-flowbite-pro, ADR 0230). Declare every exemption under\n\
             # `exempt:`, each `- {{path: <glob>, because: <why>}}` with both keys.\n\
             {POLICY_KEY}:\n  {}\n",
            kit_line(profile)
        ));
        return Ok((out, AdoptAction::Created));
    };
    let named = block
        .get("kit")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if named == profile {
        return Ok((text.to_string(), AdoptAction::Unchanged));
    }
    if !named.is_empty() {
        return Err(format!(
            "{POLICY} already declares {POLICY_KEY}.kit '{named}'; change it by hand to adopt {profile}"
        ));
    }
    let refuse = || {
        format!(
            "{POLICY}'s {POLICY_KEY}: block is not written as a block mapping adoption can extend; add `{}` under it by hand",
            kit_line(profile)
        )
    };
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let header = lines
        .iter()
        .position(|l| l.trim_end() == format!("{POLICY_KEY}:"))
        .ok_or_else(refuse)?;
    let mut end = header + 1;
    while end < lines.len() {
        let l = lines[end];
        if !(l.trim().is_empty() || l.starts_with(' ') || l.starts_with('#')) {
            break;
        }
        end += 1;
    }
    let children: Vec<usize> = (header + 1..end)
        .filter(|&i| {
            let l = lines[i];
            !l.trim().is_empty() && !l.trim_start().starts_with('#')
        })
        .collect();
    let indent = children
        .first()
        .map(|&i| lines[i].len() - lines[i].trim_start().len())
        .unwrap_or(2);
    let kit_at = children.iter().copied().find(|&i| {
        let l = lines[i];
        l.len() - l.trim_start().len() == indent && l.trim_start().starts_with("kit:")
    });
    let line = format!("{}{}\n", " ".repeat(indent), kit_line(profile));
    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        if Some(i) == kit_at {
            out.push_str(&line);
            continue;
        }
        out.push_str(l);
        if i == header && kit_at.is_none() {
            if !l.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&line);
        }
    }
    let written = yaml::parse_mapping(&out)
        .ok()
        .and_then(|m| {
            policy_block(&m)
                .and_then(|b| b.get("kit"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|k| k == profile);
    if written.is_none() {
        return Err(refuse());
    }
    Ok((out, AdoptAction::Updated))
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
        Ok(current) => match adopted_rule(&current, &template) {
            (AdoptAction::Unchanged, _) => writes.push(AdoptWrite {
                path: rule_path.clone(),
                action: AdoptAction::Unchanged,
                detail: format!(
                    "{} is the template, and its section '{}' is the repository's",
                    profile.rule,
                    SLOT_HEADING.trim_start_matches("# ")
                ),
            }),
            (AdoptAction::Updated, Some(text)) => {
                writes.push(AdoptWrite {
                    path: rule_path.clone(),
                    action: AdoptAction::Updated,
                    detail: format!(
                        "{} brought up to share/{}, keeping the repository's section '{}' as it was",
                        profile.rule,
                        profile.rule_template,
                        SLOT_HEADING.trim_start_matches("# ")
                    ),
                });
                files.push((rule_path, text));
            }
            _ => {
                writes.push(AdoptWrite {
                    path: rule_path.clone(),
                    action: AdoptAction::Kept,
                    detail: format!(
                        "{} differs from share/{} and has no section '{}'",
                        profile.rule,
                        profile.rule_template,
                        SLOT_HEADING.trim_start_matches("# ")
                    ),
                });
                notes.push(format!(
                    "{rule_path} differs from the template share/{} and has no '{SLOT_HEADING}' section; move what is the repository's own under that heading at the end, and adoption brings the rest up to the template",
                    profile.rule_template
                ));
            }
        },
        Err(_) => {
            writes.push(AdoptWrite {
                path: rule_path.clone(),
                action: AdoptAction::Created,
                detail: format!("{} from share/{}", profile.rule, profile.rule_template),
            });
            files.push((rule_path, template));
        }
    }

    // `ui.kit` is the one key adoption writes; the policy schema declares the block
    // (I4336), and the exemptions under it are the repository's to declare
    let (with_block, block_detail) = {
        let (text, action) = policy_with_kit(&policy, &profile.id).map_err(AdoptError::Refused)?;
        (text, format!("{POLICY_KEY}.kit: {action:?}").to_lowercase())
    };
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
    reads_back(&with_entries, &entries)?;
    let changed = with_entries != policy;
    let mut detail = vec![block_detail];
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

/// What adoption would write must read back: a policy that parses, that the judge accepts,
/// and whose enforcement block declares every one of `entries`.
fn reads_back(with_entries: &str, entries: &[Entry]) -> Result<(), AdoptError> {
    let parsed = yaml::parse_mapping(with_entries).map_err(|e| {
        AdoptError::Refused(format!(
            "{POLICY} would not parse after adoption ({e}); add ui.kit and the enforcement entries by hand"
        ))
    })?;
    KitPolicy::from_policy_text(Some(with_entries))
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
    for e in entries {
        if !declared.contains(e.name) {
            return Err(AdoptError::Refused(format!(
                "{POLICY}'s enforcement block could not be extended with {}; add it by hand",
                e.name
            )));
        }
    }
    Ok(())
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
            // a path joined onto the root always has a parent
            let dir = target.parent().unwrap_or(root);
            std::fs::create_dir_all(dir)
                .map_err(|e| AdoptError::Internal(format!("{}: {e}", dir.display())))?;
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
        judge(&tree(files), &profile(), policy, LicenceState::NotJudged).expect("a judgement")
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
    fn s5_the_role_aliases_are_the_profile_s() {
        let mut files = conformant();
        // the token rule's alias the profile names passes
        files.insert(
            "index.html",
            LAYOUT.replace("bg-gray-50 dark:bg-gray-900", "bg-page"),
        );
        assert!(judged(&files, &KitPolicy::absent()).passes);
        // a class the profile does not name is a finding, and no policy key adds one
        files.insert(
            "index.html",
            LAYOUT.replace("bg-gray-50 dark:bg-gray-900", "bg-canvas"),
        );
        let r = judged(&files, &KitPolicy::absent());
        let s5: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S5)
            .collect();
        assert_eq!(s5.len(), 1, "{r:#?}");
        assert!(s5[0]
            .evidence
            .contains("page (bg-gray-50 dark:bg-gray-900 | bg-page)"));
        assert!(s5[0].remedy.contains("the profile names"), "{:?}", s5[0]);
        assert!(
            KitPolicy::from_policy_text(Some(
                "ui:\n  kit: flowbite-pro\n  roles:\n    page: [bg-canvas]\n"
            ))
            .is_ok(),
            "the block's other keys are not the judge's to read"
        );
    }

    #[test]
    fn s5_layouts_come_from_the_profile_and_none_is_a_finding() {
        let mut files = conformant();
        let page = files.remove("index.html").unwrap();
        files.insert("public/app.html", page);
        let r = judged(&files, &KitPolicy::absent());
        let none = r
            .findings
            .iter()
            .find(|f| f.signal == KitSignal::S5 && f.evidence.contains("no root layout"))
            .expect("no layout is a finding");
        assert!(none.path.is_empty(), "{none:?}");
        assert!(none.remedy.contains("the profile names"), "{none:?}");
        let mut profile = profile();
        profile.shell.layouts.push("public/*.html".into());
        let r = judge(
            &tree(&files),
            &profile,
            &KitPolicy::absent(),
            LicenceState::NotJudged,
        )
        .unwrap();
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
            ("app/globals.css", "@import \"tailwindcss\";\n@plugin \"flowbite-react/plugin/tailwindcss\";\n@source \"../.flowbite-react/class-list.json\";\n".into()),
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
    fn an_exemption_moves_every_finding_at_its_path_and_says_why() {
        let mut files = conformant();
        files.insert("reel.html", "<div data-modal-toggle=\"m\"></div>".into());
        files.insert("src/main.js", "console.log(1);\n".into());
        // the attribute carriers are index.html first: S3 is owed there
        let unexempted = judged(&files, &KitPolicy::absent());
        assert_eq!(
            signals(&unexempted),
            [(KitSignal::S3, "index.html".to_string())],
            "{unexempted:#?}"
        );
        let policy = KitPolicy {
            declared: true,
            exempt: vec![KitExemption {
                path: "index.html".into(),
                because: "a render canvas".into(),
                matched: 0,
            }],
            ..KitPolicy::absent()
        };
        let r = judged(&files, &policy);
        assert!(r.findings.is_empty(), "{r:#?}");
        assert_eq!(r.exempted.len(), 1);
        assert_eq!(r.exempted[0].because, "a render canvas");
        assert_eq!(r.exempted[0].finding.signal, KitSignal::S3);
        assert_eq!(r.exemptions[0].matched, 1);
        assert!(r.passes);

        // another path is not exempted
        let other = KitPolicy {
            exempt: vec![KitExemption {
                path: "reel.html".into(),
                ..policy.exempt[0].clone()
            }],
            ..policy.clone()
        };
        let r = judged(&files, &other);
        assert!(!r.passes && r.exemptions[0].matched == 0, "{r:#?}");
        // a glob covers a tree
        let glob = KitExemption {
            path: "*.html".into(),
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
    fn a_published_site_s_layout_is_judged_by_its_navbar_and_footer() {
        let mut files = conformant();
        // a published site's layout, a marketing page by the profile, beside the app's
        files.insert(
            "site/templates/base.html",
            "<html><body><nav data-collapse-toggle=\"menu\">brand</nav><main></main></body></html>"
                .into(),
        );
        let r = judged(&files, &KitPolicy::absent());
        assert_eq!(
            r.layouts,
            ["index.html", "site/templates/base.html"],
            "{r:#?}"
        );
        let s5: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S5)
            .collect();
        assert_eq!(
            s5.len(),
            1,
            "the footer is owed, and the application shell is not: {r:#?}"
        );
        assert_eq!(s5[0].path, "site/templates/base.html");
        assert!(s5[0].evidence.contains("footer"));
        files.insert(
            "site/templates/base.html",
            "<nav data-collapse-toggle=\"menu\">brand</nav><footer>c</footer>".into(),
        );
        let r = judged(&files, &KitPolicy::absent());
        assert!(r.passes, "{r:#?}");
        // a layout outside the marketing globs owes the application shell
        files.insert(
            "templates/base.html",
            "<nav data-collapse-toggle=\"menu\">brand</nav><footer>c</footer>".into(),
        );
        let r = judged(&files, &KitPolicy::absent());
        assert!(
            r.findings
                .iter()
                .any(|f| f.path == "templates/base.html" && f.evidence.contains("fixed top navbar")),
            "{r:#?}"
        );
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
        // the skeleton's block declares no kit: nothing is adopted
        let skeleton = "ui:\n  kit: \"\"\n  visibility: public\n  publish: []\n  evidence: []\n";
        assert_eq!(
            KitPolicy::from_policy_text(Some(skeleton)).unwrap(),
            KitPolicy::absent()
        );
        let ok = KitPolicy::from_policy_text(Some(
            "ui:\n  kit: flowbite-pro\n  visibility: private\n  exempt:\n    - path: reel.html\n      because: a render canvas\n    - path: landing.html\n      because: the agency page\n",
        ))
        .unwrap();
        assert!(ok.declared);
        assert_eq!(ok.profile, "flowbite-pro");
        assert_eq!(ok.exempt.len(), 2);
        assert_eq!(ok.exempt[1].path, "landing.html");
        assert_eq!(ok.exempt[1].because, "the agency page");
        // exemptions are read whether or not a kit is adopted
        let unadopted =
            KitPolicy::from_policy_text(Some("ui:\n  exempt:\n    - path: a\n      because: b\n"))
                .unwrap();
        assert!(!unadopted.declared && unadopted.exempt.len() == 1);
        assert_eq!(unadopted.profile, DEFAULT_PROFILE);

        let bad = KitPolicy::from_policy_text(Some(
            "ui:\n  kit: [x]\n  exempt:\n    - path: reel.html\n    - because: why\n    - path: a\n      because: b\n      signals: [S1]\n      shell: marketing\n",
        ))
        .unwrap_err();
        let all = bad.join("\n");
        for expected in [
            "ui.kit is",
            "(reel.html) gives no because",
            "exempt[1] names no path",
            "exempt[2].signals is not a key",
            "exempt[2].shell is not a key",
        ] {
            assert!(all.contains(expected), "{expected} missing from:\n{all}");
        }
        assert!(KitPolicy::from_policy_text(Some("ui: flowbite\n")).is_err());
        assert!(
            KitPolicy::from_policy_text(Some("a:\n\tb: 1\n")).unwrap_err()[0]
                .contains("does not parse")
        );
        assert!(KitPolicy::from_block(&serde_json::json!({"kit": "p", "exempt": "x"})).is_err());
        assert!(KitPolicy::from_block(&serde_json::json!({"kit": "p", "exempt": ["x"]})).is_err());
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
        let r = inspect(dir.path(), &share, None).unwrap();
        assert!(r.passes, "{r:#?}");
        assert!(!r.policy_declared);

        std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
        std::fs::write(
            dir.path().join(POLICY),
            "ui:\n  kit: flowbite-pro\n  exempt:\n    - path: index.html\n",
        )
        .unwrap();
        let r = inspect(dir.path(), &share, None).unwrap();
        assert_eq!(r.verdict, KitVerdict::BadPolicy);
        assert!(!r.passes);
        assert!(r.policy_errors[0].contains("because"));
        assert!(
            r.findings.is_empty() && r.exempted.is_empty() && r.surfaces.is_empty() && !r.ui,
            "a bad policy judges nothing: {r:#?}"
        );

        std::fs::write(dir.path().join(POLICY), "ui:\n  kit: no-such-kit\n").unwrap();
        let r = inspect(dir.path(), &share, None).unwrap();
        assert_eq!(r.verdict, KitVerdict::BadPolicy);
        assert!(r.policy_errors[0].contains("ui.kit names no-such-kit"));
        assert!(r.policy_errors[0].contains("share/design/kits/no-such-kit.yaml"));

        std::fs::write(dir.path().join(POLICY), "ui:\n  kit: flowbite-pro\n").unwrap();
        let r = inspect(dir.path(), &share, None).unwrap();
        assert!(r.policy_declared && r.passes, "{r:#?}");

        std::fs::remove_file(dir.path().join(POLICY)).unwrap();
        let empty = tempfile::tempdir().unwrap();
        assert!(inspect(dir.path(), empty.path(), None)
            .unwrap_err()
            .contains("ships no"));
        assert!(inspect(&dir.path().join("absent"), &share, None).is_err());
    }

    const POLICY_TEXT: &str = "version: 1\n\nenforcement:\n  - name: doctor-on-commit\n    path: majordomus\n    args: [doctor]\n    wired_by: git-hook:pre-commit\n\n# the projections\nprojections:\n  - provider: agents\n    target: AGENTS.md\n";

    /// A distribution share with the shipped profile and rule template.
    fn shipped_share() -> tempfile::TempDir {
        let crate_share = crate::synthetic::crate_share();
        let read = |p: &str| std::fs::read_to_string(crate_share.join(p)).unwrap();
        repository(&[
            (
                "design/kits/flowbite-pro.yaml",
                &read("design/kits/flowbite-pro.yaml"),
            ),
            (
                "design/kits/flowbite-pro.rule.md",
                &read("design/kits/flowbite-pro.rule.md"),
            ),
        ])
    }

    #[test]
    fn adoption_sets_the_kit_in_the_ui_block_and_writes_nothing_else_there() {
        let share = shipped_share();
        // the skeleton's block names no kit: adoption sets it and keeps every other key
        let skeleton = format!(
            "{POLICY_TEXT}\n# the user interface\nui:\n  kit: \"\"\n  visibility: public\n  publish: []\n  evidence: []\n\nscope:\n  - src\n"
        );
        let dir = repository(&[(POLICY, &skeleton)]);
        let r = adopt(dir.path(), share.path(), DEFAULT_PROFILE, false).unwrap();
        assert_eq!(
            r.writes[1].detail,
            "ui.kit: updated, design-kit-on-push: created"
        );
        let policy = std::fs::read_to_string(dir.path().join(POLICY)).unwrap();
        let map = yaml::parse_mapping(&policy).unwrap();
        assert_eq!(map["ui"]["kit"], "flowbite-pro", "{policy}");
        assert_eq!(map["ui"]["visibility"], "public");
        assert_eq!(map["ui"]["evidence"], serde_json::json!([]));
        assert!(
            map["ui"].get("exempt").is_none(),
            "exemptions are the repository's"
        );
        assert_eq!(map["scope"][0], "src", "the next block is kept");
        assert!(policy.contains("# the user interface\nui:\n  kit: flowbite-pro\n  visibility"));
        assert!(!policy.contains("design_kit"));
        let read = KitPolicy::from_policy_text(Some(&policy)).unwrap();
        assert!(read.declared && read.profile == DEFAULT_PROFILE);
        let again = adopt(dir.path(), share.path(), DEFAULT_PROFILE, false).unwrap();
        assert!(again
            .writes
            .iter()
            .all(|w| w.action == AdoptAction::Unchanged));

        // a block without a kit line gains one, at the indentation of its keys, first
        let (text, action) = policy_with_kit(
            "ui:\n  visibility: private\n  exempt:\n    - path: a\n      because: b\nversion: 1\n",
            "flowbite-pro",
        )
        .unwrap();
        assert_eq!(action, AdoptAction::Updated);
        assert!(
            text.starts_with("ui:\n  kit: flowbite-pro\n  visibility: private\n"),
            "{text}"
        );
        let read = KitPolicy::from_policy_text(Some(&text)).unwrap();
        assert_eq!(read.exempt.len(), 1);
        // a kit another adoption named is the repository's decision, and is refused
        assert!(policy_with_kit("ui:\n  kit: other-kit\n", "flowbite-pro")
            .unwrap_err()
            .contains("other-kit"));
        // a block adoption cannot extend is refused rather than rewritten
        assert!(
            policy_with_kit("ui: {visibility: public}\n", "flowbite-pro")
                .unwrap_err()
                .contains("by hand")
        );
    }

    #[test]
    fn adoption_writes_the_rule_the_kit_and_the_entries_once() {
        let open = shipped_share();
        let share = open.path().to_path_buf();
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
        assert!(
            first.writes[1].detail.starts_with("ui.kit: created"),
            "{:?}",
            first.writes
        );
        let rule = std::fs::read_to_string(
            dir.path()
                .join(RULES_DIR)
                .join("ui-uses-flowbite-pro.v1.md"),
        )
        .unwrap();
        assert!(rule.starts_with("---\nid: project.ui-uses-flowbite-pro\n"));
        let policy = std::fs::read_to_string(dir.path().join(POLICY)).unwrap();
        let map = yaml::parse_mapping(&policy).unwrap();
        assert_eq!(map["ui"]["kit"], "flowbite-pro");
        assert_eq!(
            map["ui"].as_object().unwrap().len(),
            1,
            "adoption writes ui.kit and nothing else under ui: {policy}"
        );
        assert!(map.get("design_kit").is_none(), "{policy}");
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

        // another profile the distribution ships is adopted the same way
        let kit = share.join("design/kits/other-kit.yaml");
        let text = std::fs::read_to_string(share.join("design/kits/flowbite-pro.yaml"))
            .unwrap()
            .replace("id: flowbite-pro", "id: other-kit");
        std::fs::write(&kit, text).unwrap();
        let other = repository(&[(POLICY, POLICY_TEXT)]);
        adopt(other.path(), &share, "other-kit", false).unwrap();
        let map = yaml::parse_mapping(&std::fs::read_to_string(other.path().join(POLICY)).unwrap())
            .unwrap();
        assert_eq!(map["ui"]["kit"], "other-kit");
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
        let other = repository(&[(POLICY, "ui:\n  kit: another\n")]);
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
        // the kit itself
        let (text, action) = policy_with_kit("version: 1", "flowbite-pro").unwrap();
        assert_eq!(action, AdoptAction::Created);
        assert_eq!(
            policy_with_kit(&text, "flowbite-pro").unwrap().1,
            AdoptAction::Unchanged
        );
        assert!(policy_with_kit("a:\n\tb\n", "flowbite-pro").is_err());
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
        for key in ["surfaces", "notices", "licence"] {
            assert!(v.get(key).is_some(), "{key}");
        }
        assert_eq!(v["surfaces"][0]["kind"], "root");
        assert_eq!(v["licence"]["state"], "not_judged");
        assert_eq!(v["licence"]["variant"], "not_used");
        let bad = serde_json::to_value(KitReport::bad_policy("p", vec!["x".into()])).unwrap();
        assert_eq!(bad["verdict"], "bad_policy");
    }

    /// A tree of `(path, text)` pairs, judged with no policy and the given licence state.
    fn judged_files(files: &[(&str, &str)], licence: LicenceState) -> KitReport {
        judge(
            &Tree::from_files(files.iter().copied()),
            &profile(),
            &KitPolicy::absent(),
            licence,
        )
        .expect("a judgement")
    }

    #[test]
    fn s0_a_published_page_is_an_interface_whatever_builds_it() {
        let ui = |files: &[(&str, &str)]| judged_files(files, LicenceState::NotJudged);
        // a conventional site directory
        let r = ui(&[("site/about.html", "<p></p>")]);
        assert!(r.ui, "{r:#?}");
        assert!(r
            .ui_evidence
            .iter()
            .any(|e| e.contains("published page") && e.contains("site/about.html")));
        // build output below a site directory is not a page
        assert!(!ui(&[("public/assets/x.html", "<p></p>")]).ui);
        // the directory a Pages workflow uploads
        let pages = "jobs:\n  deploy:\n    steps:\n      - uses: actions/upload-pages-artifact@v3\n        with:\n          path: ./book\n";
        assert!(!ui(&[("book/x.html", "<p></p>")]).ui);
        assert!(
            ui(&[
                ("book/x.html", "<p></p>"),
                (".github/workflows/pages.yml", pages)
            ])
            .ui
        );
        // the directory a deploy script copies
        assert!(
            ui(&[
                ("html/x.html", "<p></p>"),
                ("scripts/deploy.sh", "rsync -a html/ host:/var/www\n")
            ])
            .ui
        );
        // docs/ only when it says it is published
        assert!(!ui(&[("docs/guide.html", "<p></p>")]).ui);
        assert!(ui(&[("docs/guide.html", "<p></p>"), ("docs/.nojekyll", "")]).ui);
        // a single-file app
        assert!(ui(&[("web/index.html", "<p></p>")]).ui);
        // pages that are no interface are a notice, never silent
        let r = ui(&[("notes/x.html", "<p></p>")]);
        assert!(!r.ui && r.passes);
        assert!(
            r.notices.iter().any(|n| n.contains("notes/x.html")),
            "{:?}",
            r.notices
        );
    }

    const WEB_PACKAGE: &str =
        r#"{"dependencies": {"next": "15", "flowbite-react": "0.12", "tailwindcss": "4"}}"#;
    const WEB_STYLE: &str = "@import \"tailwindcss\";\n@plugin \"flowbite-react/plugin/tailwindcss\";\n@source \"../.flowbite-react/class-list.json\";\n";
    const WEB_LAYOUT: &str = "import { Sidebar, Breadcrumb } from 'flowbite-react';\nexport default () => <Sidebar aria-label=\"Sidebar\"/>;\n";

    /// The conformant root, a workspace `apps/web` and whatever `extra` adds.
    fn monorepo(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, String)> {
        let mut files: Vec<(&'static str, String)> = conformant().into_iter().collect();
        files.retain(|(p, _)| *p != "package.json");
        files.push((
            "package.json",
            r#"{"workspaces": ["apps/*"], "devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "^4.0.1"}}"#
                .into(),
        ));
        files.push(("apps/web/package.json", WEB_PACKAGE.into()));
        files.push(("apps/web/app/globals.css", WEB_STYLE.into()));
        files.push(("apps/web/app/page.tsx", WEB_LAYOUT.into()));
        for (p, t) in extra {
            files.retain(|(q, _)| q != p);
            files.push((*p, t.to_string()));
        }
        files
    }

    fn judged_owned(files: &[(&'static str, String)]) -> KitReport {
        let pairs: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
        judged_files(&pairs, LicenceState::NotJudged)
    }

    #[test]
    fn s0_the_surfaces_are_the_root_every_declared_workspace_and_every_app() {
        let r = judged_owned(&monorepo(&[
            // an example is never a surface, and its kit is never counted
            (
                "examples/demo/package.json",
                r#"{"dependencies": {"react": "19", "daisyui": "5"}}"#,
            ),
            ("examples/demo/index.html", "<div></div>"),
            // a nested app: a manifest with markup in its own subtree
            (
                "tools/admin/package.json",
                r#"{"dependencies": {"vue": "3", "flowbite": "4", "tailwindcss": "4"}}"#,
            ),
            (
                "tools/admin/src/App.vue",
                "<button data-modal-toggle=\"m\"></button><script>import 'flowbite';</script>",
            ),
            ("tools/admin/src/app.css", STYLE),
            // a manifest with UI packages and no markup of its own is a notice
            (
                "lib/preset/package.json",
                r#"{"peerDependencies": {"tailwindcss": "4"}}"#,
            ),
        ]));
        let dirs: Vec<(&str, KitSurfaceKind)> = r
            .surfaces
            .iter()
            .map(|s| (s.dir.as_str(), s.kind))
            .collect();
        assert_eq!(
            dirs,
            [
                ("", KitSurfaceKind::Root),
                ("apps/web", KitSurfaceKind::Workspace),
                ("tools/admin", KitSurfaceKind::App)
            ],
            "{r:#?}"
        );
        assert!(
            r.findings.iter().all(|f| f.signal != KitSignal::S4),
            "{r:#?}"
        );
        for notice in ["examples/demo/package.json", "lib/preset/package.json"] {
            assert!(
                r.notices.iter().any(|n| n.starts_with(notice)),
                "{notice}: {:?}",
                r.notices
            );
        }
        assert!(r.passes, "{r:#?}");

        // a pnpm workspace file declares workspaces too
        let files = [
            ("pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n"),
            (
                "packages/ui/package.json",
                r#"{"dependencies": {"svelte": "5"}}"#,
            ),
        ];
        let r = judged_files(&files, LicenceState::NotJudged);
        assert_eq!(r.surfaces.len(), 1, "{r:#?}");
        assert_eq!(r.surfaces[0].kind, KitSurfaceKind::Workspace);
        assert!(r
            .findings
            .iter()
            .any(|f| f.signal == KitSignal::S1 && f.path == "packages/ui/package.json"));
    }

    #[test]
    fn a_surface_passes_s1_to_s3_only_with_usage_in_its_own_files() {
        // the workspace is wired and uses nothing; the root's usage is not its own
        let unused = "export default () => <main className=\"p-4\"/>;\n";
        let r = judged_owned(&monorepo(&[("apps/web/app/page.tsx", unused)]));
        let usage: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.evidence.contains("wiring alone"))
            .collect();
        assert_eq!(usage.len(), 1, "{r:#?}");
        assert_eq!(usage[0].signal, KitSignal::S1);
        assert_eq!(usage[0].path, "apps/web/package.json");
        assert!(
            r.surfaces[0].usage[0].contains("data-drawer-target"),
            "{r:#?}"
        );
        assert!(r.surfaces[1].usage.is_empty());

        // a declared workspace resolves the root's dependency; an app does not
        let r = judged_owned(&monorepo(&[(
            "apps/web/package.json",
            r#"{"dependencies": {"next": "15", "tailwindcss": "4"}}"#,
        )]));
        assert!(
            r.findings.iter().any(|f| f.signal == KitSignal::S1
                && f.path == "apps/web/package.json"
                && f.evidence.contains("flowbite-react")),
            "next takes the binding even when the root declares flowbite: {r:#?}"
        );
        let r = judged_owned(&monorepo(&[(
            "apps/web/package.json",
            r#"{"dependencies": {"vue": "3", "tailwindcss": "4"}}"#,
        )]));
        assert!(
            r.findings.iter().all(|f| f.path != "apps/web/package.json"),
            "the workspace takes flowbite from the root: {r:#?}"
        );

        // S2 and S3 are the surface's own: the root's stylesheet does not configure it
        let r = judged_owned(&monorepo(&[(
            "apps/web/app/globals.css",
            "@import \"tailwindcss\";\n",
        )]));
        assert!(r
            .findings
            .iter()
            .any(|f| f.signal == KitSignal::S2 && f.path == "apps/web/app/globals.css"));

        // every usage marker counts: attributes, components, the API
        for text in [
            "<div data-tooltip-target=\"t\"></div>",
            "import { Button } from 'flowbite-react';",
            "import { Modal } from 'flowbite';\nnew Modal(el);",
        ] {
            let r = judged_owned(&monorepo(&[("apps/web/app/page.tsx", text)]));
            assert!(
                r.findings
                    .iter()
                    .all(|f| !f.evidence.contains("wiring alone")),
                "{text}: {r:#?}"
            );
        }
    }

    const PRO_DIR: &str = "design/flowbite-pro-admin-dashboard-1.2.0/README.md";

    #[test]
    fn the_licence_decides_a_licensed_marker_and_an_undecided_one_fails() {
        use KitVariant::*;
        use LicenceState::*;
        for (carries, state, variant) in [
            (false, NotJudged, NotUsed),
            (false, Refused, NotUsed),
            (true, Active, Licensed),
            (true, Inactive, Unlicensed),
            (true, NotJudged, Undecided),
            (true, Refused, Unlicensed),
        ] {
            assert_eq!(decide(carries, state), variant, "{carries} {state:?}");
            assert_eq!(variant.passes(), matches!(variant, NotUsed | Licensed));
        }
        // the design licence's verdict, as agreed: active and inactive pass through, refused
        // stays refused, not judged is undecided
        for (verdict, state) in [
            (licence::Verdict::Active, Active),
            (licence::Verdict::Inactive, Inactive),
            (licence::Verdict::Refused, Refused),
            (licence::Verdict::NotJudged, NotJudged),
        ] {
            assert_eq!(licence_state_of(verdict), state);
        }

        let mut files: Vec<(&str, &str)> = vec![
            ("package.json", PACKAGE),
            ("src/input.css", STYLE),
            ("src/main.js", SCRIPT),
            ("index.html", LAYOUT),
        ];
        let clean = judged_files(&files, NotJudged);
        assert!(clean.passes, "{clean:#?}");
        assert_eq!(clean.licence.as_ref().unwrap().variant, NotUsed);

        files.push((PRO_DIR, "the distribution"));
        let undecided = judged_files(&files, NotJudged);
        assert!(!undecided.passes, "an undecided licence is never a pass");
        let l = undecided.licence.as_ref().unwrap();
        assert_eq!((l.state, l.variant), (NotJudged, Undecided));
        assert!(l.markers[0].starts_with(PRO_DIR), "{l:#?}");
        assert!(undecided.findings.iter().any(|f| f.signal == KitSignal::S1
            && f.path == PRO_DIR
            && f.evidence.contains("cannot decide")));
        let refused = judged_files(&files, Refused);
        assert!(!refused.passes);
        assert!(
            refused
                .findings
                .iter()
                .any(|f| f.path == PRO_DIR && f.evidence.contains("refused the tree")),
            "{refused:#?}"
        );
        let inactive = judged_files(&files, Inactive);
        assert_eq!(inactive.licence.unwrap().variant, Unlicensed);
        assert!(!inactive.passes);
        let active = judged_files(&files, Active);
        assert!(active.passes, "{active:#?}");
        assert_eq!(active.licence.unwrap().variant, Licensed);

        // no exemption moves it
        let policy = KitPolicy {
            declared: true,
            exempt: vec![KitExemption {
                path: "design/".into(),
                because: "kept for reference".into(),
                matched: 0,
            }],
            ..KitPolicy::absent()
        };
        let r = judge(
            &Tree::from_files(files.iter().copied()),
            &profile(),
            &policy,
            NotJudged,
        )
        .unwrap();
        assert!(!r.passes && r.exempted.is_empty(), "{r:#?}");

        // a manifest named after a Pro product is a licensed marker by its text
        let named = r#"{"name": "flowbite-pro-nextjs-admin-dashboard", "dependencies": {"tailwindcss": "4", "flowbite": "4"}}"#;
        let r = judged_files(&[("package.json", named)], NotJudged);
        assert!(r.licence.unwrap().markers[0].starts_with("package.json:1"));
        // and it is judged even where S0 finds no interface
        let r = judged_files(&[(PRO_DIR, "x")], NotJudged);
        assert!(
            !r.ui && !r.passes && r.verdict == KitVerdict::Findings,
            "{r:#?}"
        );
    }

    #[test]
    fn a_profile_with_licence_evidence_declares_its_licensed_markers() {
        let text = std::fs::read_to_string(
            crate::synthetic::crate_share().join("design/kits/flowbite-pro.yaml"),
        )
        .unwrap();
        let start = text.find("licence:\n").unwrap();
        let end = text.find("\nforbidden:").unwrap();
        let none = format!(
            "{}licence:\n  evidence: kit-evidence/v1\n  markers: []\n{}",
            &text[..start],
            &text[end..]
        );
        assert!(Profile::parse(&none)
            .unwrap_err()
            .contains("declares no licensed markers"));
        let empty = format!(
            "{}licence:\n  evidence: kit-evidence/v1\n  markers:\n    - name: nothing\n{}",
            &text[..start],
            &text[end..]
        );
        assert!(Profile::parse(&empty)
            .unwrap_err()
            .contains("neither a path, a text nor a dependency"));
        let free = format!("{}{}", &text[..start], &text[end + 1..]);
        let p = Profile::parse(&free).expect("a free kit declares no licence");
        let r = judge(
            &Tree::from_files([("index.html", LAYOUT), (PRO_DIR, "x")]),
            &p,
            &KitPolicy::absent(),
            LicenceState::NotJudged,
        )
        .unwrap();
        assert!(r.licence.is_none());
        let shipped = profile();
        let licence = shipped.licence.as_ref().expect("Flowbite Pro is licensed");
        assert_eq!(licence.evidence, "kit-evidence/v1");
        assert!(!licence.markers.is_empty());
    }

    #[test]
    fn the_policy_is_read_through_one_function() {
        let map = yaml::parse_mapping("ui:\n  kit: flowbite-pro\n").unwrap();
        assert_eq!(policy_block(&map).unwrap()["kit"], "flowbite-pro");
        assert!(policy_block(&yaml::parse_mapping("version: 1\n").unwrap()).is_none());
        // adoption and judgement agree on the block, because both read it there
        let (text, action) = policy_with_kit("version: 1\n", "flowbite-pro").unwrap();
        assert_eq!(action, AdoptAction::Created);
        assert!(text.ends_with(&format!("ui:\n  {}\n", kit_line("flowbite-pro"))));
        assert!(KitPolicy::from_policy_text(Some(&text)).unwrap().declared);
        assert_eq!(
            policy_with_kit(&text, "flowbite-pro").unwrap(),
            (text.clone(), AdoptAction::Unchanged)
        );
        assert!(policy_with_kit("a:\n\tb\n", "flowbite-pro").is_err());
        // and the design licence reads the same key from the same block
        assert_eq!(licence::UiPolicy::parse(&text).unwrap().kit, "flowbite-pro");
    }

    #[test]
    fn the_rule_s_last_section_is_the_repository_s_and_adoption_keeps_it() {
        let template = std::fs::read_to_string(
            crate::synthetic::crate_share().join("design/kits/flowbite-pro.rule.md"),
        )
        .unwrap();
        let (theirs, slot) = split_slot(&template);
        let slot = slot.expect("the template carries the slot");
        assert!(slot.starts_with(SLOT_HEADING));
        assert!(theirs.contains("# Verification"));
        assert_eq!(split_slot("no slot\n"), ("no slot\n", None));
        assert_eq!(
            split_slot("a\n# In this repository\nours\n"),
            ("a\n", Some("# In this repository\nours\n"))
        );
        assert_eq!(split_slot("a\n## In this repository\n").1, None);

        let ours = format!("{theirs}{SLOT_HEADING}\n\nThe site is built from the Pro blocks.\n");
        assert_eq!(
            adopted_rule(&template, &template),
            (AdoptAction::Unchanged, None)
        );
        assert_eq!(
            adopted_rule(&ours, &template),
            (AdoptAction::Unchanged, None)
        );
        let stale = ours.replace("version: 1", "version: 0");
        assert_eq!(
            adopted_rule(&stale, &template),
            (AdoptAction::Updated, Some(ours.clone()))
        );
        assert_eq!(adopted_rule("edited", &template), (AdoptAction::Kept, None));

        // through adoption on disk
        let share = crate::synthetic::crate_share();
        let dir = repository(&[(POLICY, POLICY_TEXT)]);
        adopt(dir.path(), &share, DEFAULT_PROFILE, false).unwrap();
        let rule = dir
            .path()
            .join(RULES_DIR)
            .join("ui-uses-flowbite-pro.v1.md");
        std::fs::write(&rule, &stale).unwrap();
        let again = adopt(dir.path(), &share, DEFAULT_PROFILE, false).unwrap();
        assert_eq!(again.writes[0].action, AdoptAction::Updated, "{again:#?}");
        assert_eq!(std::fs::read_to_string(&rule).unwrap(), ours);
        let third = adopt(dir.path(), &share, DEFAULT_PROFILE, false).unwrap();
        assert_eq!(third.writes[0].action, AdoptAction::Unchanged);
        assert_eq!(std::fs::read_to_string(&rule).unwrap(), ours);
    }

    const SHELL_HEEX: &str = "<nav class=\"fixed z-30 w-full bg-white border-b border-gray-200 dark:bg-gray-800 dark:border-gray-700\"><button data-drawer-target=\"sidebar\" data-drawer-toggle=\"sidebar\" aria-controls=\"sidebar\">Menu</button></nav>\n<aside id=\"sidebar\" aria-label=\"Sidebar\">links</aside>\n<main><nav aria-label=\"Breadcrumb\"></nav><%= @inner_content %></main>\n";
    const ROOT_HEEX: &str =
        "<html><body class=\"bg-gray-50 dark:bg-gray-900\"><%= @inner_content %></body></html>";
    const V3_CONFIG: &str = "module.exports = { content: ['./js/**/*.js', '../lib/**/*.*ex', '../../../deps/flowbite/**/*.js'], plugins: [require('flowbite/plugin')] };\n";

    /// A Phoenix umbrella: a root manifest for tooling, and a Phoenix app whose bundle's
    /// manifest is apps/x_web/assets/package.json.
    fn umbrella(extra: &[(&'static str, &'static str)]) -> Vec<(&'static str, String)> {
        let mut files: Vec<(&'static str, String)> = vec![
            ("mix.exs", "def project, do: [apps_path: \"apps\"]\n".into()),
            (
                "package.json",
                r#"{"devDependencies": {"semantic-release": "24", "eslint": "9"}}"#.into(),
            ),
            (
                "apps/x_web/mix.exs",
                "defp deps, do: [{:phoenix_live_view, \"~> 1.0\"}]\n".into(),
            ),
            (
                "apps/x_web/assets/package.json",
                r#"{"dependencies": {"flowbite": "^2.5.1", "tailwindcss": "^3.4.13"}}"#.into(),
            ),
            ("apps/x_web/assets/tailwind.config.js", V3_CONFIG.into()),
            (
                "apps/x_web/assets/css/app.css",
                "@import 'tailwindcss/base';\n@import 'tailwindcss/components';\n@import 'tailwindcss/utilities';\n".into(),
            ),
            ("apps/x_web/assets/js/app.js", "import 'flowbite';\n".into()),
            (
                "apps/x_web/lib/x_web/components/layouts/app.html.heex",
                SHELL_HEEX.into(),
            ),
            (
                "apps/x_web/lib/x_web/components/layouts/root.html.heex",
                ROOT_HEEX.into(),
            ),
        ];
        for (p, t) in extra {
            files.retain(|(q, _)| q != p);
            files.push((*p, t.to_string()));
        }
        files
    }

    #[test]
    fn s0_a_phoenix_app_of_an_umbrella_is_a_surface_with_its_bundle_manifest() {
        let r = judged_owned(&umbrella(&[]));
        let dirs: Vec<(&str, KitSurfaceKind, Vec<String>)> = r
            .surfaces
            .iter()
            .map(|s| (s.dir.as_str(), s.kind, s.manifests.clone()))
            .collect();
        assert_eq!(
            dirs,
            [(
                "apps/x_web",
                KitSurfaceKind::App,
                vec!["apps/x_web/assets/package.json".to_string()]
            )],
            "{r:#?}"
        );
        assert!(r.stacks.contains(&"tailwind-3".to_string()), "{r:#?}");
        assert!(r.stacks.contains(&"phoenix".to_string()), "{r:#?}");
        assert!(r.passes, "{r:#?}");

        // the bundle manifest is counted: another kit in it is S4's
        let r = judged_owned(&umbrella(&[(
            "apps/x_web/assets/package.json",
            r#"{"dependencies": {"flowbite": "2", "tailwindcss": "3", "daisyui": "4"}}"#,
        )]));
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S4 && f.path == "apps/x_web/assets/package.json"),
            "{r:#?}"
        );
        // and so is the Mix project of each surface
        let r = judged_owned(&umbrella(&[(
            "apps/x_web/mix.exs",
            "defp deps, do: [{:phoenix_live_view, \"~> 1.0\"}, {:petal_components, \"~> 2.0\"}]\n",
        )]));
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S4 && f.path == "apps/x_web/mix.exs"),
            "{r:#?}"
        );

        // a Mix project with no bundle manifest is a surface that owes one, at its mix.exs
        let mut files = umbrella(&[]);
        files.retain(|(p, _)| *p != "apps/x_web/assets/package.json");
        let r = judged_owned(&files);
        assert_eq!(r.surfaces.len(), 1, "{r:#?}");
        let s1 = r
            .findings
            .iter()
            .find(|f| f.signal == KitSignal::S1 && f.evidence.contains("no package manifest"))
            .expect("the dependency is owed");
        assert_eq!(s1.path, "apps/x_web/mix.exs");
        assert!(
            s1.evidence.contains("apps/x_web/assets/package.json"),
            "{s1:?}"
        );
        // and with no configuration either, the postcss-import form of the Tailwind 3 entry
        // still says Tailwind 3
        files.retain(|(p, _)| *p != "apps/x_web/assets/tailwind.config.js");
        let r = judged_owned(&files);
        assert_eq!(r.stacks, ["tailwind-3", "phoenix"], "{r:#?}");
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S2 && f.evidence.contains("no configuration")),
            "{r:#?}"
        );
    }

    #[test]
    fn phoenix_markup_in_elixir_is_read_and_its_layout_is_layouts_ex() {
        let base = [
            ("mix.exs", "[{:phoenix_live_view, \"~> 1.1\"}]"),
            (
                "assets/package.json",
                r#"{"dependencies": {"flowbite": "4", "tailwindcss": "4"}}"#,
            ),
            ("assets/css/app.css", STYLE),
            ("assets/js/app.js", SCRIPT),
            ("lib/app_web/components/layouts/root.html.heex", ROOT_HEEX),
        ];
        let layouts_ex = format!("defmodule AppWeb.Layouts do\n  def app(assigns) do\n    ~H\"\"\"\n{SHELL_HEEX}\"\"\"\n  end\nend\n");
        let mut files: Vec<(&str, &str)> = base.to_vec();
        files.push(("lib/app_web/components/layouts.ex", &layouts_ex));
        let r = judged_files(&files, LicenceState::NotJudged);
        assert_eq!(r.layouts, ["lib/app_web/components/layouts.ex"], "{r:#?}");
        assert!(r.passes, "{r:#?}");

        // usage in ~H counts, and the attribute in ~H owes Flowbite's JavaScript
        let live = "def render(assigns), do: ~H\"<button data-modal-toggle=\\\"m\\\">x</button>\"";
        let mut files: Vec<(&str, &str)> = base.to_vec();
        files.push(("lib/app_web/components/layouts.ex", &layouts_ex));
        files.retain(|(p, _)| *p != "assets/js/app.js");
        files.push(("lib/app_web/live/page_live.ex", live));
        let r = judged_files(&files, LicenceState::NotJudged);
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S3 && f.evidence.contains("data-")),
            "{r:#?}"
        );

        // a layouts.ex with no ~H (Phoenix 1.7 embeds its templates) is no layout, and its
        // app.html.heex is judged as before
        let mut tree = Tree::from_files(base.iter().copied());
        tree.paths
            .insert("lib/app_web/components/layouts.ex".into());
        tree.paths
            .insert("lib/app_web/components/layouts/app.html.heex".into());
        tree.texts.insert(
            "lib/app_web/components/layouts/app.html.heex".into(),
            SHELL_HEEX.into(),
        );
        let r = judge(
            &tree,
            &profile(),
            &KitPolicy::absent(),
            LicenceState::NotJudged,
        )
        .unwrap();
        assert_eq!(
            r.layouts,
            ["lib/app_web/components/layouts/app.html.heex"],
            "{r:#?}"
        );
        assert!(r.passes, "{r:#?}");
    }

    #[test]
    fn s4_a_vendored_kit_is_found_by_its_import_and_its_file() {
        let base = [
            ("package.json", PACKAGE),
            ("src/main.js", SCRIPT),
            ("index.html", LAYOUT),
        ];
        let css = format!("{STYLE}@plugin \"../vendor/daisyui\" {{ themes: false; }}\n");
        let mut files: Vec<(&str, &str)> = base.to_vec();
        files.push(("src/input.css", &css));
        let r = judged_files(&files, LicenceState::NotJudged);
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S4 && f.path == "src/input.css"),
            "{r:#?}"
        );
        let mut tree = Tree::from_files(conformant().iter().map(|(p, t)| (*p, t.as_str())));
        tree.vendored.insert("assets/vendor/daisyui.js".into());
        let r = judge(
            &tree,
            &profile(),
            &KitPolicy::absent(),
            LicenceState::NotJudged,
        )
        .unwrap();
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S4 && f.path == "assets/vendor/daisyui.js"),
            "{r:#?}"
        );
        // a file of the repository's own named like a kit is no import of it
        let re = import_pattern(&["bootstrap".into(), "daisyui".into()])
            .unwrap()
            .unwrap();
        assert!(!re.is_match("import './bootstrap';"));
        assert!(re.is_match("@import \"../node_modules/bootstrap/scss/bootstrap\";"));
        assert!(re.is_match("@plugin \"../vendor/daisyui.js\";"));
    }

    #[test]
    fn s2_flowbite_react_is_configured_as_its_init_writes_it() {
        let v3 = r#"{"dependencies": {"react": "18", "flowbite-react": "0.10", "tailwindcss": "^3.4.0"}}"#;
        let config = "const flowbite = require(\"flowbite-react/tailwind\");\nmodule.exports = {\n  content: ['./src/**/*.tsx', flowbite.content()],\n  plugins: [flowbite.plugin()],\n};\n";
        let page = "import { Button } from 'flowbite-react';\nexport default () => <Button/>;\n";
        let r = judged_files(
            &[
                ("package.json", v3),
                ("tailwind.config.js", config),
                ("src/index.html", "<div id=\"root\"></div>"),
                ("src/App.tsx", page),
            ],
            LicenceState::NotJudged,
        );
        assert!(r.stacks.contains(&"tailwind-3".to_string()), "{r:#?}");
        assert!(
            r.findings.iter().all(|f| f.signal != KitSignal::S2),
            "{r:#?}"
        );
        let without = config
            .replace("flowbite.plugin()", "")
            .replace("flowbite.content()", "");
        let r = judged_files(
            &[
                ("package.json", v3),
                ("tailwind.config.js", &without),
                ("src/index.html", "<div id=\"root\"></div>"),
                ("src/App.tsx", page),
            ],
            LicenceState::NotJudged,
        );
        let s2 = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S2)
            .count();
        assert_eq!(s2, 2, "{r:#?}");
    }

    #[test]
    fn a_workspace_takes_the_react_binding_only_when_it_declares_react() {
        let r = judged_files(
            &[
                (
                    "package.json",
                    r#"{"workspaces": ["packages/*"], "devDependencies": {"react": "19", "storybook": "8"}}"#,
                ),
                (
                    "packages/site/package.json",
                    r#"{"dependencies": {"svelte": "5", "flowbite": "4", "tailwindcss": "4"}}"#,
                ),
                (
                    "packages/site/src/App.svelte",
                    "<button data-dropdown-toggle=\"d\"></button><script>import 'flowbite';</script>",
                ),
                (
                    "packages/site/src/app.css",
                    "@import \"tailwindcss\";\n@plugin \"flowbite/plugin\";\n@source \"../node_modules/flowbite\";\n",
                ),
            ],
            LicenceState::NotJudged,
        );
        // the root declares react, so it is a surface of its own, and the binding it owes is
        // React's; the workspace resolves the root's react and still owes the kit's package
        let surfaces: Vec<(&str, KitSurfaceKind, Vec<String>, bool)> = r
            .surfaces
            .iter()
            .map(|s| {
                (
                    s.dir.as_str(),
                    s.kind,
                    s.manifests.clone(),
                    s.usage.is_empty(),
                )
            })
            .collect();
        assert_eq!(
            surfaces,
            [
                (
                    "",
                    KitSurfaceKind::Root,
                    vec!["package.json".to_string()],
                    true
                ),
                (
                    "packages/site",
                    KitSurfaceKind::Workspace,
                    vec!["packages/site/package.json".to_string()],
                    false
                ),
            ],
            "{r:#?}"
        );
        assert_eq!(r.verdict, KitVerdict::Findings);
        assert!(!r.passes);
        assert_eq!(r.stacks, ["tailwind-4", "react"], "{r:#?}");
        // every finding is the root's: none at the workspace, whose binding is the kit's own
        let findings: Vec<(KitSignal, &str)> = r
            .findings
            .iter()
            .map(|f| (f.signal, f.path.as_str()))
            .collect();
        assert_eq!(
            findings,
            [
                (KitSignal::S1, "package.json"),
                (KitSignal::S1, "package.json"),
                (KitSignal::S2, "package.json"),
                (KitSignal::S5, ""),
            ],
            "{r:#?}"
        );
        assert!(r.findings[0].evidence.contains("flowbite-react"), "{r:#?}");
        assert!(r.findings.iter().all(|f| !f.path.starts_with("packages/")));
    }

    #[test]
    fn s0_docs_is_published_only_by_a_pages_or_publish_dir_step() {
        let ui = |files: &[(&str, &str)]| judged_files(files, LicenceState::NotJudged).ui;
        let checkout = "jobs:\n  b:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          path: docs\n      - uses: actions/cache@v4\n        with:\n          path: docs-cache\n";
        assert!(!ui(&[
            ("docs/index.html", "<p></p>"),
            (".github/workflows/wiki.yml", checkout),
        ]));
        let gh_pages = "      - uses: peaceiris/actions-gh-pages@v4\n        with:\n          publish_dir: ./docs\n";
        assert!(ui(&[
            ("docs/index.html", "<p></p>"),
            (".github/workflows/pages.yml", gh_pages),
        ]));
        let artifact = "      - uses: actions/upload-pages-artifact@v3\n        with:\n          path: docs/\n";
        assert!(ui(&[
            ("docs/index.html", "<p></p>"),
            (".github/workflows/pages.yml", artifact),
        ]));
        // and nothing at all publishes it
        assert!(!ui(&[("docs/index.html", "<p></p>")]));
    }

    #[test]
    fn a_root_s2_finding_is_at_its_manifest_and_can_be_exempted() {
        let mut files = conformant();
        files.remove("src/input.css");
        let r = judged(&files, &KitPolicy::absent());
        let s2: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S2)
            .collect();
        assert_eq!(s2.len(), 2, "{r:#?}");
        assert!(s2.iter().all(|f| f.path == "package.json"), "{s2:?}");
        let policy = KitPolicy {
            declared: true,
            exempt: vec![KitExemption {
                path: "package.json".into(),
                because: "the stylesheet is built by the host".into(),
                matched: 0,
            }],
            ..KitPolicy::absent()
        };
        let r = judged(&files, &policy);
        assert!(r.passes, "{r:#?}");
        assert_eq!(r.exemptions[0].matched, 2);
    }

    #[test]
    fn the_tree_is_the_repository_s_files_and_vendored_paths() {
        let share = crate::synthetic::crate_share();
        let git = |dir: &Path, args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        // an ignored directory is not the repository's, and its templates are no interface
        let dir = repository(&[
            (".gitignore", "generated/\n"),
            ("src/model.py", "x = 1\n"),
            ("generated/templates/page.html", "<html></html>"),
        ]);
        assert!(
            inspect(dir.path(), &share, None).unwrap().ui,
            "walked, it is read"
        );
        git(dir.path(), &["init", "-q", "."]);
        let r = inspect(dir.path(), &share, None).unwrap();
        assert!(!r.ui, "{r:#?}");
        // an untracked page Git does not ignore is the repository's
        std::fs::write(dir.path().join("index.html"), "<p></p>").unwrap();
        assert!(inspect(dir.path(), &share, None).unwrap().ui);
        let profile = profile();
        assert!(Tree::read(dir.path(), &profile)
            .unwrap()
            .paths
            .contains("index.html"));

        // a vendored distribution is listed by path, with Git and without it
        let pro = "assets/vendor/flowbite-pro-2.3.0/js/app.js";
        for with_git in [false, true] {
            let dir = repository(&[(pro, "x"), ("assets/vendor/node_modules/a/b.js", "x")]);
            if with_git {
                git(dir.path(), &["init", "-q", "."]);
            }
            let tree = Tree::read(dir.path(), &profile).unwrap();
            assert!(tree.vendored.contains(pro), "{tree:?}");
            assert!(!tree.paths.contains(pro) && !tree.texts.contains_key(pro));
            assert_eq!(
                tree.vendored.len(),
                1,
                "a skipped directory stays skipped: {tree:?}"
            );
            let r = judge(
                &tree,
                &profile,
                &KitPolicy::absent(),
                LicenceState::NotJudged,
            )
            .unwrap();
            let l = r.licence.unwrap();
            assert_eq!(l.variant, KitVariant::Undecided, "{l:?}");
            assert!(l.markers[0].starts_with(pro), "{l:?}");
        }

        // Elixir is read only where it carries ~H
        let dir = repository(&[
            ("lib/a.ex", "defmodule A do\nend\n"),
            ("lib/b.ex", "def render(a), do: ~H\"<p></p>\"\n"),
        ]);
        let tree = Tree::read(dir.path(), &profile).unwrap();
        assert!(tree.paths.contains("lib/a.ex") && !tree.texts.contains_key("lib/a.ex"));
        assert!(tree.texts.contains_key("lib/b.ex"));
    }

    #[test]
    fn a_dependency_on_a_pro_package_is_a_licensed_marker() {
        for section in [
            "dependencies",
            "devDependencies",
            "peerDependencies",
            "optionalDependencies",
        ] {
            let manifest = format!(
                r#"{{"{section}": {{"flowbite-pro-admin": "file:../pro", "flowbite": "4", "tailwindcss": "4"}}}}"#
            );
            let r = judged_files(&[("package.json", &manifest)], LicenceState::NotJudged);
            let l = r.licence.unwrap();
            assert_eq!(l.variant, KitVariant::Undecided, "{section}: {l:?}");
            assert_eq!(
                l.markers,
                ["package.json declares flowbite-pro-admin (a dependency on a Flowbite Pro package)"],
                "{section}"
            );
        }
        // any other key named like a Pro product is no dependency, and no marker
        let scripts = r#"{"scripts": {"flowbite-pro-build": "node build.js"}, "config": {"flowbite-react-blocks": "x"}, "dependencies": {"flowbite": "4", "tailwindcss": "4"}}"#;
        let r = judged_files(&[("package.json", scripts)], LicenceState::NotJudged);
        let l = r.licence.unwrap();
        assert_eq!(l.variant, KitVariant::NotUsed, "{l:?}");
        assert!(l.markers.is_empty(), "{l:?}");
    }

    #[test]
    fn a_mix_project_under_examples_is_no_surface_and_counts_for_nothing() {
        let demo =
            "defp deps, do: [{:phoenix_live_view, \"~> 1.0\"}, {:petal_components, \"~> 2.0\"}]\n";
        let mut files = conformant();
        files.insert("examples/demo/mix.exs", demo.into());
        let r = judged(&files, &KitPolicy::absent());
        assert!(r.passes, "{r:#?}");
        assert_eq!(r.stacks, ["tailwind-4"], "no phoenix: {r:#?}");
        assert_eq!(r.surfaces.len(), 1, "{r:#?}");
        // the same project outside a not-app directory is an app, and its kit is S4's
        let mut files = conformant();
        files.insert("apps/demo/mix.exs", demo.into());
        let r = judged(&files, &KitPolicy::absent());
        assert!(r.stacks.contains(&"phoenix".to_string()), "{r:#?}");
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S4 && f.path == "apps/demo/mix.exs"),
            "{r:#?}"
        );
        // and the root's own mix.exs always counts
        let mut files = conformant();
        files.insert("mix.exs", demo.into());
        let r = judged(&files, &KitPolicy::absent());
        assert!(
            r.findings
                .iter()
                .any(|f| f.signal == KitSignal::S4 && f.path == "mix.exs"),
            "{r:#?}"
        );
    }

    #[test]
    fn a_submodule_or_a_nested_repository_is_named_and_judged_in_its_own() {
        let git = |dir: &Path, args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@invalid"])
                .args(args)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {out:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let share = crate::synthetic::crate_share();
        let profile = profile();
        let dir = repository(&[
            ("README.md", "# a library\n"),
            ("web/index.html", LAYOUT),
            ("node_modules/x/index.html", "<p></p>"),
        ]);
        git(&dir.path().join("web"), &["init", "-q", "."]);
        git(&dir.path().join("web"), &["add", "-A"]);
        git(&dir.path().join("web"), &["commit", "-q", "-m", "web"]);
        git(&dir.path().join("node_modules/x"), &["init", "-q", "."]);
        git(dir.path(), &["init", "-q", "."]);
        // untracked: a repository of its own nested in this one
        let tree = Tree::read(dir.path(), &profile).unwrap();
        assert_eq!(
            tree.nested,
            BTreeMap::from([("web".to_string(), false)]),
            "one below a skipped directory is not named: {tree:?}"
        );
        assert!(!tree.paths.iter().any(|p| p.starts_with("web")), "{tree:?}");
        let r = inspect(dir.path(), &share, None).unwrap();
        assert!(!r.ui, "{r:#?}");
        let notice = r
            .notices
            .iter()
            .find(|n| n.starts_with("web is"))
            .expect("the nested repository is named");
        assert!(notice.contains("judged in its own repository"), "{notice}");
        assert!(notice.contains("--repo web"), "{notice}");
        // tracked as a submodule: its gitlink
        let head = git(&dir.path().join("web"), &["rev-parse", "HEAD"]);
        git(
            dir.path(),
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{head},web"),
            ],
        );
        let tree = Tree::read(dir.path(), &profile).unwrap();
        assert_eq!(tree.nested, BTreeMap::from([("web".to_string(), true)]));
        let r = inspect(dir.path(), &share, None).unwrap();
        assert!(
            r.notices
                .iter()
                .any(|n| n.starts_with("web is a Git submodule")),
            "{r:#?}"
        );
        // without Git the walk reads it as the repository's own files
        let walked = repository(&[("web/index.html", LAYOUT)]);
        let tree = Tree::read(walked.path(), &profile).unwrap();
        assert!(tree.nested.is_empty() && tree.paths.contains("web/index.html"));
    }

    #[test]
    fn s2_flowbite_react_counts_only_the_calls_of_the_bound_helper() {
        let v3 = r#"{"dependencies": {"react": "18", "flowbite-react": "0.10", "tailwindcss": "^3.4.0"}}"#;
        let page = "import { Button } from 'flowbite-react';\nexport default () => <Button/>;\n";
        let s2 = |config: &str| {
            let r = judged_files(
                &[
                    ("package.json", v3),
                    ("tailwind.config.js", config),
                    ("src/index.html", "<div id=\"root\"></div>"),
                    ("src/App.tsx", page),
                ],
                LicenceState::NotJudged,
            );
            r.findings
                .iter()
                .filter(|f| f.signal == KitSignal::S2)
                .map(|f| f.evidence.clone())
                .collect::<Vec<_>>()
        };
        // another object's plugin() and content() configure nothing of Flowbite's
        let other = "const flowbite = require('flowbite-react/tailwind');\nconst forms = require('@tailwindcss/forms');\nconst typo = require('./typo');\nmodule.exports = {\n  content: ['./src/**/*.tsx', typo.content()],\n  plugins: [forms.plugin()],\n};\n";
        let found = s2(other);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found.iter().any(|e| e.contains("flowbite/plugin")),
            "{found:?}"
        );
        assert!(
            found.iter().any(|e| e.contains("content glob")),
            "{found:?}"
        );
        // a method of a longer name, or of a member, is not the helper's call either
        let member = "const fb = require('flowbite-react/tailwind');\nmodule.exports = { content: [x.fb.content()], plugins: [myfb.plugin()] };\n";
        assert_eq!(s2(member).len(), 2);
        // the helper bound by an ES import, under any name
        let esm = "import fbr from \"flowbite-react/tailwind\";\nexport default {\n  content: ['./src/**/*.tsx', fbr.content({ base: '../' })],\n  plugins: [fbr.plugin()],\n};\n";
        assert!(s2(esm).is_empty(), "{:?}", s2(esm));
        let star = "import * as fbr from 'flowbite-react/tailwind';\nexport default { content: [fbr.content()], plugins: [fbr.plugin()] };\n";
        assert!(s2(star).is_empty(), "{:?}", s2(star));
    }

    #[test]
    fn an_embedded_layout_is_judged_with_its_app_and_root_templates_only() {
        let base = [
            ("mix.exs", "[{:phoenix_live_view, \"~> 1.1\"}]"),
            (
                "assets/package.json",
                r#"{"dependencies": {"flowbite": "4", "tailwindcss": "4"}}"#,
            ),
            ("assets/css/app.css", STYLE),
            ("assets/js/app.js", SCRIPT),
            ("lib/app_web/components/layouts/root.html.heex", ROOT_HEEX),
        ];
        // Layouts.app draws no shell; an email template in layouts/ carries one
        let bare = "defmodule AppWeb.Layouts do\n  embed_templates \"layouts/*\"\n  def app(assigns) do\n    ~H\"\"\"\n<main><%= @inner_content %></main>\n\"\"\"\n  end\nend\n";
        let mut files: Vec<(&str, &str)> = base.to_vec();
        files.push(("lib/app_web/components/layouts.ex", bare));
        files.push(("lib/app_web/components/layouts/email.html.heex", SHELL_HEEX));
        let r = judged_files(&files, LicenceState::NotJudged);
        assert_eq!(r.layouts, ["lib/app_web/components/layouts.ex"], "{r:#?}");
        let s5: Vec<&KitFinding> = r
            .findings
            .iter()
            .filter(|f| f.signal == KitSignal::S5)
            .collect();
        assert!(
            s5.iter().any(|f| f.evidence.contains("fixed top navbar")),
            "the email template supplies no shell: {r:#?}"
        );
        assert!(!r.passes);
        // the app template the module embeds does
        files.retain(|(p, _)| !p.ends_with("email.html.heex"));
        files.push(("lib/app_web/components/layouts/app.html.heex", SHELL_HEEX));
        let r = judged_files(&files, LicenceState::NotJudged);
        assert_eq!(r.layouts, ["lib/app_web/components/layouts.ex"], "{r:#?}");
        assert!(r.passes, "{r:#?}");
    }

    #[test]
    fn the_licence_seam_is_the_design_licence_s_answer() {
        let git = |dir: &Path, args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        let profile = profile();
        // a kit declared on a public tree with a UI file and no evidence: not judged
        let dir = repository(&[
            (POLICY, "ui:\n  kit: flowbite-pro\n  visibility: public\n"),
            ("index.html", LAYOUT),
        ]);
        git(dir.path(), &["init", "-q", "."]);
        git(dir.path(), &["add", "-A"]);
        assert_eq!(
            licence_answer(dir.path(), &profile, None).0,
            LicenceState::NotJudged
        );
        // no kit declared: inactive, which leaves licensed markers unlicensed
        std::fs::write(dir.path().join(POLICY), "ui:\n  kit: \"\"\n").unwrap();
        assert_eq!(
            licence_answer(dir.path(), &profile, None).0,
            LicenceState::Inactive
        );
        // a tree git cannot list cannot be judged
        let bare = repository(&[(POLICY, "ui:\n  kit: flowbite-pro\n")]);
        let state = licence_answer(bare.path(), &profile, None).0;
        assert!(
            matches!(state, LicenceState::NotJudged | LicenceState::Inactive),
            "{state:?}"
        );
        // evidence held and a licensed file in the public tree: refused
        let home = tempfile::tempdir().unwrap();
        let licensed = b"<aside>the licensed sidebar</aside>\n";
        let digest = |b: &[u8]| {
            use sha2::Digest;
            let d = sha2::Sha256::digest(b);
            d.iter().map(|x| format!("{x:02x}")).collect::<String>()
        };
        let norm: Vec<u8> = licensed
            .iter()
            .map(|&b| if b == b'\n' { b' ' } else { b })
            .collect();
        std::fs::write(
            home.path().join("kit.tsv"),
            format!(
                "{}\t{}\tpro/sidebar.html\n",
                digest(licensed),
                digest(&norm)
            ),
        )
        .unwrap();
        std::fs::write(
            dir.path().join(POLICY),
            "ui:\n  kit: flowbite-pro\n  evidence: [\"~/kit.tsv\"]\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("copied.html"), licensed).unwrap();
        git(dir.path(), &["add", "-A"]);
        let policy = std::fs::read_to_string(dir.path().join(POLICY)).unwrap();
        let report =
            licence::assess_root(dir.path(), Some(&policy), None, Some(home.path()), None).unwrap();
        assert_eq!(
            licence_state_of(report.verdict),
            LicenceState::Refused,
            "{report:#?}"
        );
        std::fs::remove_file(dir.path().join("copied.html")).unwrap();
        git(dir.path(), &["add", "-A"]);
        let report =
            licence::assess_root(dir.path(), Some(&policy), None, Some(home.path()), None).unwrap();
        assert_eq!(licence_state_of(report.verdict), LicenceState::Active);
        // a block the design licence cannot use is not judged
        let report = licence::assess_root(
            dir.path(),
            Some("ui:\n  kit: flowbite-pro\n  evidence: [\"/abs/kit.tsv\"]\n"),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(licence_state_of(report.verdict), LicenceState::NotJudged);
    }

    /// A public tree that adopts the kit, has a UI and carries a licensed marker, staged so
    /// that the design licence can list it.
    fn licensed_marker_tree() -> tempfile::TempDir {
        let dir = repository(&[
            (POLICY, "ui:\n  kit: flowbite-pro\n  visibility: public\n"),
            ("package.json", PACKAGE),
            ("src/input.css", STYLE),
            ("src/main.js", SCRIPT),
            ("index.html", LAYOUT),
            (
                "design/flowbite-pro-admin-dashboard-1.2.0/README.md",
                "the distribution\n",
            ),
        ]);
        for args in [&["init", "-q", "."][..], &["add", "-A"][..]] {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        }
        dir
    }

    /// A kit-evidence/v1 file whose one fingerprint is no file of the tree, with `head` as
    /// its leading lines.
    fn evidence_file(dir: &Path, name: &str, head: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(
            &path,
            format!("{head}aaaaaaaaaaaaaaaa\tbbbbbbbbbbbbbbbb\tpro/secret-sidebar.html\n"),
        )
        .unwrap();
        path
    }

    #[test]
    fn the_evidence_file_decides_the_licence_of_a_licensed_marker() {
        let share = crate::synthetic::crate_share();
        let profile = profile();
        let tree = licensed_marker_tree();
        let held = tempfile::tempdir().unwrap();

        // the file the CI secret was decoded into: judged, clean, so the licence is active
        // and the licensed marker passes
        let ours = evidence_file(
            held.path(),
            "ours.tsv",
            "# kit: flowbite-pro\n# kit-evidence/v1\n",
        );
        assert_eq!(
            licence_answer(tree.path(), &profile, Some(&ours)).0,
            LicenceState::Active
        );
        let r = inspect(tree.path(), &share, Some(&ours)).unwrap();
        let l = r
            .licence
            .as_ref()
            .expect("a licensed kit's profile answers");
        assert_eq!(
            (l.state, l.variant),
            (LicenceState::Active, KitVariant::Licensed),
            "{r:#?}"
        );
        assert!(r.passes, "{r:#?}");
        // a file that names no kit is anybody's, and judges as well
        let anybody = evidence_file(held.path(), "anybody.tsv", "# kit-evidence/v1\n");
        assert_eq!(
            licence_answer(tree.path(), &profile, Some(&anybody)).0,
            LicenceState::Active
        );

        // a file that says it is another kit's judges nothing: not judged, undecided
        let other = evidence_file(
            held.path(),
            "other.tsv",
            "# kit: examplekit\n# kit-evidence/v1\n",
        );
        let (state, reason) = licence_answer(tree.path(), &profile, Some(&other));
        assert_eq!(state, LicenceState::NotJudged);
        let reason = reason.expect("the reason is said");
        assert!(
            reason.contains("another kit") && reason.contains("flowbite-pro"),
            "{reason}"
        );
        let r = inspect(tree.path(), &share, Some(&other)).unwrap();
        let l = r.licence.as_ref().unwrap();
        assert_eq!(
            (l.state, l.variant),
            (LicenceState::NotJudged, KitVariant::Undecided)
        );
        assert_eq!(r.verdict, KitVerdict::Findings);

        // no file, and no ui.evidence: a public tree owes a judgement nobody can give
        let (state, reason) = licence_answer(tree.path(), &profile, None);
        assert_eq!(state, LicenceState::NotJudged);
        assert!(
            reason.unwrap().contains("no licence"),
            "the reason says what is missing"
        );
        let r = inspect(tree.path(), &share, None).unwrap();
        let l = r.licence.as_ref().unwrap();
        assert_eq!(l.variant, KitVariant::Undecided, "{r:#?}");
        assert!(!r.passes);

        // a file that is not kit-evidence/v1 is not judged, and the reason names the file and
        // the line, never what the line holds
        let bad = held.path().join("bad.tsv");
        std::fs::write(&bad, "# kit: flowbite-pro\nsupersecretvalue\n").unwrap();
        let (state, reason) = licence_answer(tree.path(), &profile, Some(&bad));
        assert_eq!(state, LicenceState::NotJudged);
        let reason = reason.unwrap();
        assert!(
            reason.contains("bad.tsv") && reason.contains("line 2"),
            "{reason}"
        );
        // and no answer carries a line of the evidence
        for path in [&ours, &other, &bad] {
            let text =
                serde_json::to_string(&inspect(tree.path(), &share, Some(path)).unwrap()).unwrap();
            for secret in [
                "aaaaaaaaaaaaaaaa",
                "bbbbbbbbbbbbbbbb",
                "secret-sidebar",
                "supersecretvalue",
            ] {
                assert!(
                    !text.contains(secret),
                    "{secret} reached the report: {text}"
                );
            }
        }
    }

    #[test]
    fn only_a_first_line_header_makes_evidence_another_kits() {
        let profile = profile();
        let tree = licensed_marker_tree();
        let held = tempfile::tempdir().unwrap();
        let state = |name: &str, head: &str| {
            let path = evidence_file(held.path(), name, head);
            licence_answer(tree.path(), &profile, Some(&path))
        };
        // the header first: this kit's judges, another kit's judges nothing
        assert_eq!(
            state("ours.tsv", "\n# kit: flowbite-pro\n").0,
            LicenceState::Active
        );
        let (s, reason) = state("other.tsv", "# kit: examplekit \n");
        assert_eq!(s, LicenceState::NotJudged);
        assert!(reason.unwrap().contains("another kit"));
        // no header, or a `# kit:` comment that is not the first line: anybody's evidence
        assert_eq!(state("none.tsv", "").0, LicenceState::Active);
        assert_eq!(
            state("late.tsv", "# kit-evidence/v1\n# kit: examplekit\n").0,
            LicenceState::Active
        );
    }

    #[test]
    fn a_retired_design_kit_block_is_a_notice_naming_ui_kit() {
        assert_eq!(retired_notice(None), None);
        assert_eq!(retired_notice(Some("ui:\n  kit: flowbite-pro\n")), None);
        let notice = retired_notice(Some(
            "design_kit:\n  profile: flowbite-pro\nui:\n  kit: \"\"\n",
        ))
        .expect("the retired block is named");
        assert!(
            notice.contains("design_kit:") && notice.contains("ui.kit"),
            "{notice}"
        );

        let share = crate::synthetic::crate_share();
        let dir = repository(&[(POLICY, "design_kit:\n  profile: flowbite-pro\n")]);
        let r = inspect(dir.path(), &share, None).unwrap();
        assert_eq!(r.verdict, KitVerdict::NoUi);
        assert!(
            r.notices.iter().any(|n| n.contains("retired design_kit")),
            "{r:#?}"
        );
        // a bad policy carries it too, beside the reason nothing was judged
        std::fs::write(
            dir.path().join(POLICY),
            "design_kit:\n  profile: flowbite-pro\nui:\n  kit: [x]\n",
        )
        .unwrap();
        let r = inspect(dir.path(), &share, None).unwrap();
        assert_eq!(r.verdict, KitVerdict::BadPolicy);
        assert!(r.notices.iter().any(|n| n.contains("ui.kit")), "{r:#?}");
    }

    /// Run git in `dir` with nothing of the calling repository's environment.
    fn git_in(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    /// The shipped profile's text with its `licence:` block taken out.
    fn profile_text_without_licence() -> String {
        let text = std::fs::read_to_string(
            crate::synthetic::crate_share().join("design/kits/flowbite-pro.yaml"),
        )
        .unwrap();
        let mut out = String::new();
        let mut inside = false;
        for line in text.split_inclusive('\n') {
            if line.trim_end() == "licence:" {
                inside = true;
                continue;
            }
            if inside && (line.trim().is_empty() || line.starts_with(' ')) {
                continue;
            }
            inside = false;
            out.push_str(line);
        }
        out
    }

    #[test]
    fn a_profile_or_a_policy_that_cannot_be_read_is_an_error_naming_the_file() {
        // a profile that is a directory is invalid, not unknown, and names its path
        let share = repository(&[("README.md", "x")]);
        std::fs::create_dir_all(share.path().join("design/kits/odd.yaml")).unwrap();
        let err = Profile::load(share.path(), "odd").unwrap_err();
        assert!(
            matches!(&err, ProfileError::Invalid(m) if m.contains("design/kits/odd.yaml")),
            "{err:?}"
        );
        // a profile that does not parse stops the judgement and the adoption alike
        let broken = repository(&[("design/kits/flowbite-pro.yaml", "id: [unclosed\n")]);
        let dir = repository(&[(POLICY, POLICY_TEXT), ("index.html", "<p></p>")]);
        let err = inspect(dir.path(), broken.path(), None).unwrap_err();
        assert!(err.contains("flowbite-pro.yaml"), "{err}");
        assert!(matches!(
            adopt(dir.path(), broken.path(), DEFAULT_PROFILE, true),
            Err(AdoptError::Internal(_))
        ));
        // a policy path that cannot be read is an error, where an absent one is no policy
        let odd = repository(&[("index.html", "<p></p>")]);
        std::fs::create_dir_all(odd.path().join(POLICY)).unwrap();
        let err = inspect(odd.path(), &crate::synthetic::crate_share(), None).unwrap_err();
        assert!(err.starts_with(POLICY), "{err}");
    }

    #[test]
    fn a_profile_that_declares_no_readable_licence_judges_none_and_says_why() {
        let dir = repository(&[("index.html", "<p></p>")]);
        let mut p = profile();
        p.licence.as_mut().unwrap().evidence = "other-evidence/v9".into();
        let (state, reason) = licence_answer(dir.path(), &p, None);
        assert_eq!(state, LicenceState::NotJudged);
        assert!(reason.unwrap().contains("names other-evidence/v9 evidence"));
        p.licence = None;
        let (state, reason) = licence_answer(dir.path(), &p, None);
        assert_eq!(state, LicenceState::NotJudged);
        assert!(reason.unwrap().contains("declares no licence"));
        // inspected under such a profile, the report carries no licence at all
        let share = repository(&[(
            "design/kits/flowbite-pro.yaml",
            &profile_text_without_licence(),
        )]);
        let r = inspect(dir.path(), share.path(), None).unwrap();
        assert!(r.ui && r.licence.is_none(), "{r:#?}");
    }

    #[test]
    fn a_reported_runtime_without_markup_is_reported_by_its_package_alone() {
        let mut p = profile();
        for r in &mut p.reported {
            r.markup = None;
        }
        let mut files = conformant();
        files.insert(
            "package.json",
            r#"{"devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "^4.0.1", "alpinejs": "3"}}"#
                .into(),
        );
        files.insert("about.html", "<div x-data=\"{}\"></div>".into());
        let r = judge(
            &tree(&files),
            &p,
            &KitPolicy::absent(),
            LicenceState::NotJudged,
        )
        .unwrap();
        assert!(
            r.reported.iter().any(|n| n.contains("declares alpinejs")),
            "{r:#?}"
        );
        assert!(
            !r.reported.iter().any(|n| n.contains("markup file(s)")),
            "no markup expression, no markup count: {r:#?}"
        );
    }

    #[test]
    fn a_git_tree_is_listed_only_from_its_top_and_only_its_readable_files_are_read() {
        let dir = repository(&[
            ("index.html", "<p></p>"),
            ("src/app.min.js", "x"),
            ("src/main.js", "import 'flowbite';\n"),
            ("gone.html", "<p></p>"),
        ]);
        std::fs::write(dir.path().join("binary.html"), [0xff_u8, 0xfe, 0x00]).unwrap();
        git_in(dir.path(), &["init", "-q", "."]);
        git_in(dir.path(), &["add", "-A"]);
        // a file Git still lists that is gone from the work tree is not there
        std::fs::remove_file(dir.path().join("gone.html")).unwrap();
        // a link Git lists is no file of the tree
        std::os::unix::fs::symlink("index.html", dir.path().join("link.html")).unwrap();
        let tree = Tree::read(dir.path(), &profile()).unwrap();
        assert!(tree.paths.contains("index.html") && tree.paths.contains("src/main.js"));
        for absent in ["src/app.min.js", "gone.html", "link.html"] {
            assert!(!tree.paths.contains(absent), "{absent}: {tree:?}");
        }
        // a file that is no UTF-8 is listed, and its text is not read
        assert!(tree.paths.contains("binary.html") && !tree.texts.contains_key("binary.html"));
        // below the top of the work tree Git's listing is not used
        assert!(git_files(dir.path()).is_some());
        assert!(git_files(&dir.path().join("src")).is_none());
        // nor where Git cannot list the index
        std::fs::write(dir.path().join(".git/index"), "not an index").unwrap();
        assert!(git_files(dir.path()).is_none());
    }

    #[test]
    fn a_walk_fails_only_at_its_root_and_lists_only_files_within_the_size_limit() {
        let mut p = profile();
        assert!(walked_files(&std::env::temp_dir().join("no-such-tree-of-the-kit"), &p).is_err());
        let dir = repository(&[("index.html", "<p></p>"), ("locked/a.html", "<p></p>")]);
        std::os::unix::fs::symlink("index.html", dir.path().join("link.html")).unwrap();
        let locked = dir.path().join("locked");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let walked = walked_files(dir.path(), &p);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        // a directory below the root that cannot be read is passed over, and a link is no file
        let walked = walked.unwrap();
        assert!(walked.contains("index.html") && !walked.contains("link.html"));
        // a file over the profile's limit is not listed
        p.max_file_bytes = 1;
        assert!(!Tree::read(dir.path(), &p)
            .unwrap()
            .paths
            .contains("index.html"));
    }

    #[test]
    fn s0_nested_manifests_count_by_their_packages_and_share_a_surface_by_directory() {
        let r = with(|f| {
            f.insert(
                "package.json",
                r#"{"workspaces": {"packages": ["packages/*"]}, "devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "^4.0.1"}}"#
                    .into(),
            );
            f.insert(
                "packages/ui/package.json",
                r#"{"dependencies": {"flowbite": "^4.0.1"}}"#.into(),
            );
            // a manifest declaring none of the packages is neither a surface nor a notice
            f.insert(
                "tools/package.json",
                r#"{"dependencies": {"lodash": "4"}}"#.into(),
            );
            f.insert("tools/index.html", "<p></p>".into());
            // a Mix project not on LiveView is no app
            f.insert(
                "tools/mix.exs",
                "defp deps, do: [{:jason, \"~> 1.4\"}]\n".into(),
            );
            // a bundle manifest with no Mix project above it is its own directory's
            f.insert(
                "apps/a/assets/package.json",
                r#"{"dependencies": {"tailwindcss": "^4.1.0"}}"#.into(),
            );
            f.insert("apps/a/assets/index.html", "<p></p>".into());
            // a Phoenix app's two manifests are one surface
            f.insert(
                "apps/web/mix.exs",
                "{:phoenix_live_view, \"~> 1.0\"}\n".into(),
            );
            f.insert(
                "apps/web/assets/package.json",
                r#"{"dependencies": {"tailwindcss": "^4.1.0"}}"#.into(),
            );
            f.insert(
                "apps/web/package.json",
                r#"{"dependencies": {"flowbite": "^4.0.1"}}"#.into(),
            );
            f.insert("apps/web/lib/page.html.heex", "<p></p>".into());
            // what a deploy script copies out of node_modules is no published site
            f.insert("deploy.sh", "rsync -a node_modules/ host:/srv\n".into());
        });
        let surface = |dir: &str| r.surfaces.iter().find(|s| s.dir == dir);
        assert_eq!(
            surface("packages/ui").map(|s| s.kind),
            Some(KitSurfaceKind::Workspace),
            "{r:#?}"
        );
        assert!(surface("tools").is_none(), "{r:#?}");
        assert!(!r.notices.iter().any(|n| n.contains("tools/")), "{r:#?}");
        assert!(surface("apps/a/assets").is_some(), "{r:#?}");
        let web = surface("apps/web").expect("the Phoenix app");
        assert_eq!(
            web.manifests,
            ["apps/web/assets/package.json", "apps/web/package.json"],
            "{r:#?}"
        );
        assert!(
            !r.ui_evidence.iter().any(|e| e.contains("node_modules")),
            "{r:#?}"
        );
    }

    #[test]
    fn s2_a_tailwind_neither_3_nor_4_is_a_finding() {
        let r = with(|f| {
            f.insert(
                "package.json",
                r#"{"devDependencies": {"tailwindcss": "^2.2.0", "flowbite": "^4.0.1"}}"#.into(),
            );
        });
        assert!(
            r.findings.iter().any(|f| f.signal == KitSignal::S2
                && f.evidence == "tailwindcss 2 is neither Tailwind 3 nor Tailwind 4"),
            "{r:#?}"
        );
    }

    #[test]
    fn s5_a_heex_layout_is_judged_with_its_companion_only_when_that_is_another_file() {
        for layout in ["app.html.heex", "root.html.heex"] {
            let mut p = profile();
            p.shell.layouts.push(layout.to_string());
            let mut files = conformant();
            files.remove("index.html");
            files.insert(layout, "<main></main>".into());
            let r = judge(
                &tree(&files),
                &p,
                &KitPolicy::absent(),
                LicenceState::NotJudged,
            )
            .unwrap();
            assert!(
                r.layouts.iter().any(|l| l == layout)
                    && signals(&r).contains(&(KitSignal::S5, layout.to_string())),
                "{layout}: {r:#?}"
            );
        }
    }

    #[test]
    fn s5_a_marketing_layout_owes_a_navbar() {
        let r = with(|f| {
            f.insert(
                "site/templates/base.html",
                "<html><body><p>x</p></body></html>".into(),
            );
        });
        assert!(
            r.findings.iter().any(|f| f.signal == KitSignal::S5
                && f.path == "site/templates/base.html"
                && f.evidence.contains("is a marketing layout with no navbar")),
            "{r:#?}"
        );
    }

    #[test]
    fn every_licensed_marker_is_named_and_the_first_one_found_is_where_the_finding_is() {
        let mut p = profile();
        let l = p.licence.as_mut().unwrap();
        let again = l.markers.clone();
        l.markers.extend(again);
        let mut files = conformant();
        files.insert(
            "package.json",
            r#"{"name": "flowbite-pro-site", "devDependencies": {"tailwindcss": "^4.1.0", "flowbite": "^4.0.1", "flowbite-pro-blocks": "1"}}"#
                .into(),
        );
        files.insert("lib/flowbite-pro-2.3.0/app.js", "x".into());
        let r = judge(
            &tree(&files),
            &p,
            &KitPolicy::absent(),
            LicenceState::NotJudged,
        )
        .unwrap();
        let licence = r.licence.expect("a licence");
        assert_eq!(licence.markers.len(), 6, "{licence:#?}");
        assert!(
            r.findings
                .iter()
                .any(|f| f.path == "lib/flowbite-pro-2.3.0/app.js"),
            "{:#?}",
            r.findings
        );
    }

    #[test]
    fn small_readers_pass_over_what_they_cannot_read() {
        let tree = Tree::from_files([("bad/package.json", "not json")]);
        let manifests = vec![
            "absent/package.json".to_string(),
            "bad/package.json".to_string(),
        ];
        assert!(manifest_deps(&tree, &manifests).is_empty());
        // a ui: block that is no mapping cannot be extended with the kit
        let err = policy_with_kit("ui:\n  - a\n", DEFAULT_PROFILE).unwrap_err();
        assert!(err.contains("not written as a block mapping"), "{err}");
        // what adoption would write must read back
        let entries = [Entry {
            name: ON_PUSH,
            wired_by: "git-hook:pre-push".into(),
        }];
        let err = reads_back("a:\n\tb: 1\n", &entries).unwrap_err();
        assert!(
            matches!(&err, AdoptError::Refused(m) if m.contains("would not parse after adoption")),
            "{err:?}"
        );
        let err = reads_back("version: 1\n", &entries).unwrap_err();
        assert!(
            matches!(&err, AdoptError::Refused(m) if m.contains("could not be extended with design-kit-on-push")),
            "{err:?}"
        );
    }

    #[test]
    fn adoption_declares_a_gitlab_pipeline_as_the_ci_workflow() {
        let share = shipped_share();
        let dir = repository(&[
            (POLICY, POLICY_TEXT),
            (".gitlab-ci.yml", "stages: [test]\n"),
        ]);
        let r = adopt(dir.path(), share.path(), DEFAULT_PROFILE, true).unwrap();
        assert_eq!(r.ci_workflow.as_deref(), Some(".gitlab-ci.yml"), "{r:#?}");
    }
}
