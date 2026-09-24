//! The crate's rustdoc tree, judged against the crate it documents.
//!
//! # What it is for
//!
//! `cargo doc` proves the documentation *compiles*: `-D warnings` refuses a broken
//! intra-doc link and `missing_docs` refuses an undocumented export. It says nothing about
//! the tree it writes — whether a page exists for every item the crate exports, whether a
//! page is still there for an item that no longer exists, whether the tree was built from
//! the commit it claims, whether its relative links resolve once it is copied under a
//! mount, and whether it carries the machine it was built on. Those are properties of an
//! artifact, and this module is where they become findings with a kind, a file and a remedy.
//!
//! # How a route is derived
//!
//! Nothing here lists a page. Every expected file is derived from
//! [`super::source::Inventory`] by rustdoc's own rules, measured on rustdoc 1.98 against
//! this crate and against a fixture crate built for the purpose:
//!
//! ```text
//! module                 <crate>/<segments>/index.html
//! struct enum trait fn   <crate>/<module segments>/{struct,enum,trait,fn}.<Name>.html
//! const static type union                           {constant,static,type,union}.<Name>.html
//! exported macro         <crate>/macro.<name>.html, at the root whatever module declares it
//! method                 the owner's page #method.<n>; a trait's required one #tymethod.<n>
//! field / variant        the owner's page #structfield.<n> / #variant.<N>
//! associated type/const  the trait's page #associatedtype.<N> / #associatedconstant.<N>
//! ```
//!
//! An item exported only through a `pub use` of a private module is documented where the
//! re-export offers it, under the name it offers it by, and its original location holds a
//! redirect stub; an item whose module chain is public is documented where it is declared.
//! The crate segment is the library target's name, read from `Cargo.toml`. A binary target
//! is a crate of its own in the same tree, documented with its private items, and is read
//! by the same walk.
//!
//! # What the verifier judges
//!
//! One pass over the tree. Every file is classified — an item page, a redirect stub, a
//! rustdoc system file, the producer's landing — and every HTML page is tokenised once for
//! its links. The findings are typed ([`RustdocFindingKind`]) and name the file and the
//! subject; the verdict is `clean`, `findings` or `no_tree`, and a tree that is absent is
//! never reported clean.
//!
//! ```text
//! crate --syn--> Inventory --routes--> expected pages ─┐
//!                                                       ├─> findings, counts, module routes
//! target/web/rustdoc/ --walk--> files, pages, links ───┘
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::leaks::{LeakClass, Leaks};
use super::source::{Inventory, Item, ItemKind};
use crate::error::Error;
use crate::order::{canonical, OrderKey, Ordered};

/// The schema of a report. Bumped when a consumer would have to change.
pub const SCHEMA: &str = "majordomus/rustdoc/v1";

/// The identity of the web surface the tree is published as.
pub const SURFACE: &str = "rustdoc";

/// What writes the tree, for a reader who has to rebuild it.
pub const PRODUCER: &str = "scripts/rust-check --doc";

// ------------------------------------------------------------------------ the crate's targets

/// The documented targets of a crate, as its manifest declares them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targets {
    /// The library's crate name: `[lib] name`, else the package name with `-` as `_`. This
    /// is the directory rustdoc writes the library into and the first segment of every
    /// route.
    pub lib: String,
    /// Every binary cargo documents: its crate name and its root file, crate-relative.
    pub bins: Vec<(String, String)>,
}

/// Read the targets from `Cargo.toml`, with the same literal subset discipline the rest of
/// the executable reads manifests with: `name`, `path` and `doc` in `[package]`, `[lib]`
/// and `[[bin]]`, and nothing else. A binary cargo would not document — `doc = false`, or
/// one named as the library — is left out, as cargo leaves it out.
pub fn targets(crate_dir: &Path) -> Result<Targets, Error> {
    let manifest = crate_dir.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).map_err(|e| Error::InvalidSource {
        path: "Cargo.toml".into(),
        reason: format!("cannot be read under {}: {e}", crate_dir.display()),
    })?;
    #[derive(Default)]
    struct Section {
        name: Option<String>,
        path: Option<String>,
        doc: bool,
    }
    let mut package = Section::default();
    let mut lib = Section::default();
    let mut bins: Vec<Section> = Vec::new();
    let mut current = "";
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            current = match line {
                "[package]" => "package",
                "[lib]" => "lib",
                "[[bin]]" => {
                    bins.push(Section {
                        doc: true,
                        ..Section::default()
                    });
                    "bin"
                }
                _ => "",
            };
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim().trim_matches('"').to_string());
        let section = match current {
            "package" => &mut package,
            "lib" => &mut lib,
            "bin" => match bins.last_mut() {
                Some(b) => b,
                None => continue,
            },
            _ => continue,
        };
        match key {
            "name" => section.name = Some(value),
            "path" => section.path = Some(value),
            "doc" => section.doc = value != "false",
            _ => {}
        }
    }
    let package_name = package.name.ok_or_else(|| Error::InvalidSource {
        path: "Cargo.toml".into(),
        reason: "declares no [package] name, so no crate name can be derived".into(),
    })?;
    let lib = lib.name.unwrap_or_else(|| package_name.replace('-', "_"));
    let mut documented = Vec::new();
    if bins.is_empty() && crate_dir.join("src/main.rs").is_file() {
        // cargo's own discovery: a package with a src/main.rs and no [[bin]] builds one
        // binary named after the package
        documented.push((package_name.replace('-', "_"), "src/main.rs".to_string()));
    }
    for bin in bins.into_iter().filter(|b| b.doc) {
        let Some(name) = bin.name else { continue };
        let krate = name.replace('-', "_");
        if krate == lib {
            continue;
        }
        let path = bin.path.unwrap_or_else(|| {
            if name == package_name {
                "src/main.rs".into()
            } else {
                format!("src/bin/{name}.rs")
            }
        });
        documented.push((krate, path));
    }
    Ok(Targets {
        lib,
        bins: documented,
    })
}

// ------------------------------------------------------------------------------------ routes

/// Where rustdoc puts one item: a page of the tree, and an anchor on it for a member.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Route {
    /// The page, relative to the tree root: `majordomus_cli/quality/struct.Item.html`.
    pub file: String,
    /// The anchor on the page, for a member: `method.of_crate`.
    pub fragment: Option<String>,
}

impl Route {
    /// The route as a relative URL: the file, and `#fragment` when there is one.
    pub fn href(&self) -> String {
        match &self.fragment {
            Some(f) => format!("{}#{f}", self.file),
            None => self.file.clone(),
        }
    }
}

/// The file prefix rustdoc gives an item that owns a page of its own, or `None` for a kind
/// that is documented on another item's page (or, a module, as a directory).
pub fn page_prefix(kind: ItemKind) -> Option<&'static str> {
    Some(match kind {
        ItemKind::Struct => "struct",
        ItemKind::Enum => "enum",
        ItemKind::Union => "union",
        ItemKind::Trait => "trait",
        ItemKind::Function => "fn",
        ItemKind::Constant => "constant",
        ItemKind::Static => "static",
        ItemKind::TypeAlias => "type",
        ItemKind::Macro => "macro",
        ItemKind::Module
        | ItemKind::Method
        | ItemKind::Field
        | ItemKind::Variant
        | ItemKind::AssociatedType
        | ItemKind::AssociatedConstant => return None,
    })
}

/// The page of an item documented at the crate path `at`, in the tree's crate directory
/// `krate`. The first segment of `at` is the crate as the inventory names it and is
/// replaced by `krate`, so the derivation holds whatever the library is called.
///
/// `None` for a kind that owns no page: see [`member`] for those.
pub fn page(kind: ItemKind, at: &str, krate: &str) -> Option<Route> {
    let segments: Vec<&str> = at.split("::").skip(1).collect();
    let file = match kind {
        ItemKind::Module => {
            let mut parts = vec![krate];
            parts.extend(&segments);
            parts.push("index.html");
            parts.join("/")
        }
        // `#[macro_export]` puts a macro at the crate root, whatever module it is written in
        ItemKind::Macro => format!("{krate}/macro.{}.html", segments.last()?),
        other => {
            let prefix = page_prefix(other)?;
            let (name, module) = segments.split_last()?;
            let mut parts = vec![krate];
            parts.extend(module);
            parts.push("");
            format!("{}{prefix}.{name}.html", parts.join("/"))
        }
    };
    Some(Route {
        file,
        fragment: None,
    })
}

/// The anchor of a member on its owner's page: a method, an associated function, a field,
/// a variant, an associated type or constant. `owner` is the kind of the item that owns
/// it, which is what decides a trait method's anchor; `provided` is whether the trait
/// supplies it ([`Item::provided`]).
///
/// `None` for a kind that is not a member, or a member of an owner that owns no page.
pub fn member(
    kind: ItemKind,
    name: &str,
    owner: ItemKind,
    provided: bool,
    owner_page: &Route,
) -> Option<Route> {
    let anchor = match (kind, owner) {
        (ItemKind::Method | ItemKind::Function, ItemKind::Trait) if !provided => "tymethod",
        (
            ItemKind::Method | ItemKind::Function,
            ItemKind::Trait | ItemKind::Struct | ItemKind::Enum | ItemKind::Union,
        ) => "method",
        (ItemKind::Field, ItemKind::Struct | ItemKind::Union) => "structfield",
        (ItemKind::Variant, ItemKind::Enum) => "variant",
        (ItemKind::AssociatedType, ItemKind::Trait) => "associatedtype",
        (ItemKind::AssociatedConstant, ItemKind::Trait) => "associatedconstant",
        _ => return None,
    };
    Some(Route {
        file: owner_page.file.clone(),
        fragment: Some(format!("{anchor}.{name}")),
    })
}

/// The routes of one inventory: where rustdoc documents each of its items.
///
/// Built once over the inventory, so a route is a lookup and not a scan: the check derives
/// a route for every item of the crate, and a search of the item list per item would be
/// quadratic in the crate's size.
pub struct Router<'a> {
    krate: &'a str,
    /// Private items are documented too: what cargo does for a binary target.
    everything: bool,
    root: &'a str,
    modules: HashMap<&'a str, &'a Item>,
    owners: HashMap<&'a str, &'a Item>,
    /// Modules whose chain from the crate root is `pub` all the way down.
    public_modules: HashSet<&'a str>,
    /// A re-exported path, to every alias it is offered under, in canonical order.
    aliases: HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> Router<'a> {
    /// The router of a library: exported items only, re-exports followed.
    pub fn library(inventory: &'a Inventory, krate: &'a str) -> Self {
        Router::new(inventory, krate, false)
    }

    /// The router of a binary: cargo documents a binary with its private items, so every
    /// item the walk found is documented where it is declared.
    pub fn binary(inventory: &'a Inventory, krate: &'a str) -> Self {
        Router::new(inventory, krate, true)
    }

    fn new(inventory: &'a Inventory, krate: &'a str, everything: bool) -> Self {
        let root = inventory
            .items
            .first()
            .map(|i| i.path.as_str())
            .unwrap_or_default();
        let mut modules = HashMap::new();
        let mut owners = HashMap::new();
        for item in &inventory.items {
            match item.kind {
                ItemKind::Module => {
                    modules.insert(item.path.as_str(), item);
                }
                ItemKind::Struct | ItemKind::Enum | ItemKind::Union | ItemKind::Trait => {
                    owners.entry(item.path.as_str()).or_insert(item);
                }
                _ => {}
            }
        }
        // the walk visits a parent before its children, so one pass decides every chain
        let mut public_modules = HashSet::new();
        for item in inventory
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Module)
        {
            let public = item.path == root
                || (item.declared_pub && public_modules.contains(item.owner.as_str()));
            if public {
                public_modules.insert(item.path.as_str());
            }
        }
        let mut aliases: HashMap<&str, Vec<&str>> = HashMap::new();
        for (alias, target) in &inventory.reexport_aliases {
            aliases
                .entry(target.as_str())
                .or_default()
                .push(alias.as_str());
        }
        for list in aliases.values_mut() {
            list.sort_by(|a, b| crate::order::natural_cmp(a, b));
        }
        Router {
            krate,
            everything,
            root,
            modules,
            owners,
            public_modules,
            aliases,
        }
    }

    /// Is this item documented at all — exported by a library, or any item of a binary?
    pub fn documented(&self, item: &Item) -> bool {
        self.everything || item.exported
    }

    /// The crate paths an item is documented at: its own when its module chain is public
    /// (or the target is a binary), else every alias a `pub use` offers it or one of its
    /// ancestors under. Empty for an item that is not documented.
    pub fn documented_at(&self, item: &Item) -> Vec<String> {
        if !self.documented(item) {
            return Vec::new();
        }
        if self.everything || self.in_public_chain(item) {
            return vec![item.path.clone()];
        }
        // the nearest re-export that reaches the item: the item itself, or a module above
        // it that a `pub use` carried out whole
        let mut at = item.path.as_str();
        loop {
            if let Some(list) = self.aliases.get(at) {
                let rest = &item.path[at.len()..];
                return list.iter().map(|alias| format!("{alias}{rest}")).collect();
            }
            match at.rsplit_once("::") {
                Some((parent, _)) => at = parent,
                None => return Vec::new(),
            }
        }
    }

    fn in_public_chain(&self, item: &Item) -> bool {
        if item.kind == ItemKind::Module {
            return self.public_modules.contains(item.path.as_str());
        }
        if item.kind == ItemKind::Macro {
            return true;
        }
        item.declared_pub && self.public_modules.contains(item.owner.as_str())
    }

    /// Does this item own a page of its own? A module, a type, a trait, a free function, a
    /// constant, a static, an alias and an exported macro do; a member is an anchor.
    pub fn owns_page(&self, item: &Item) -> bool {
        match item.kind {
            ItemKind::Module => true,
            // an associated function is a member of the type it is implemented on
            ItemKind::Function => self.modules.contains_key(item.owner.as_str()),
            kind => page_prefix(kind).is_some(),
        }
    }

    /// Every route of an item: one per place it is documented. Empty when it is not
    /// documented, or when it is a member of something this inventory does not hold (an
    /// `impl` written for a type declared in another module).
    pub fn routes(&self, item: &Item) -> Vec<Route> {
        if !self.documented(item) {
            return Vec::new();
        }
        if self.owns_page(item) {
            return self
                .documented_at(item)
                .iter()
                .filter_map(|at| page(item.kind, at, self.krate))
                .collect();
        }
        let Some(owner) = self.owners.get(item.owner.as_str()) else {
            return Vec::new();
        };
        self.routes(owner)
            .iter()
            .filter_map(|p| member(item.kind, &item.name, owner.kind, item.provided, p))
            .collect()
    }

    /// The inventory's crate root path, which every item path starts with.
    pub fn root(&self) -> &str {
        self.root
    }
}

// ------------------------------------------------------------------------------- the report

/// What the tree was judged to be.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RustdocVerdict {
    /// The tree holds every page the crate derives and nothing wrong was found.
    Clean,
    /// At least one finding stands.
    Findings,
    /// Nothing could be judged: the tree, or the crate that documents it, is absent. Never
    /// a pass — the producer has not run, and a check that cannot see its subject says so.
    NoTree,
}

impl RustdocVerdict {
    /// The process exit code of the verdict: `0`, `10` findings, `12` nothing to judge —
    /// the codes every other check of this executable uses for the same three answers.
    pub fn exit_code(self) -> u8 {
        match self {
            RustdocVerdict::Clean => 0,
            RustdocVerdict::Findings => 10,
            RustdocVerdict::NoTree => 12,
        }
    }
}

/// What is wrong with the tree, as a stable code.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RustdocFindingKind {
    /// An exported item that owns a page has no page at its derived route.
    MissingPage,
    /// An item page no item of the crate derives: a leftover, or a route rule this check
    /// has wrong.
    OrphanPage,
    /// The tree declares it was built from a revision other than the repository's HEAD.
    Stale,
    /// The tree carries no declaration of what it is or what it was built from.
    Undeclared,
    /// The library's index page is absent, or does not say it is the library's.
    CrateIndex,
    /// A stylesheet, script, font or search file the library's index page needs is absent.
    Asset,
    /// A relative link of a page resolves to no file of the tree.
    Link,
    /// A file names a path of the machine it was built on.
    MachinePath,
    /// A file carries the shape of a credential.
    Secret,
}

impl RustdocFindingKind {
    /// The code as it is written everywhere: `missing-page`.
    pub fn as_str(self) -> &'static str {
        match self {
            RustdocFindingKind::MissingPage => "missing-page",
            RustdocFindingKind::OrphanPage => "orphan-page",
            RustdocFindingKind::Stale => "stale",
            RustdocFindingKind::Undeclared => "undeclared",
            RustdocFindingKind::CrateIndex => "crate-index",
            RustdocFindingKind::Asset => "asset",
            RustdocFindingKind::Link => "link",
            RustdocFindingKind::MachinePath => "machine-path",
            RustdocFindingKind::Secret => "secret",
        }
    }

    /// What to do about it, in the imperative.
    pub fn remedy(self) -> &'static str {
        match self {
            RustdocFindingKind::MissingPage | RustdocFindingKind::OrphanPage => {
                "rebuild the tree with scripts/rust-check --doc; if it persists, the item's route rule in quality::rustdoc is wrong for this rustdoc and must be corrected there, not excepted"
            }
            RustdocFindingKind::Stale | RustdocFindingKind::Undeclared => {
                "rebuild the tree with scripts/rust-check --doc, which writes surface.json with built_from = HEAD"
            }
            RustdocFindingKind::CrateIndex | RustdocFindingKind::Asset => {
                "rebuild the tree from a clean target/doc: a partial copy of rustdoc's output is not a tree"
            }
            RustdocFindingKind::Link => {
                "fix the link at its source: a doc comment that links a repository file by a path relative to the source file resolves on a forge and nowhere under /rustdoc; link the published page or the forge URL instead"
            }
            RustdocFindingKind::MachinePath => {
                "find what wrote the machine's path into the documentation — a doc comment, a constant's rendered value, a test fixture in a published source page — and make it repository-relative"
            }
            RustdocFindingKind::Secret => {
                "remove the value at its source and rotate it; if the match is a name rather than a value, make the pattern in apps/majordomus-cli/src/quality/leak-patterns.tsv more precise, with a test, for every reader"
            }
        }
    }

    /// Every kind, in declaration order.
    pub fn all() -> [RustdocFindingKind; 9] {
        use RustdocFindingKind::*;
        [
            MissingPage,
            OrphanPage,
            Stale,
            Undeclared,
            CrateIndex,
            Asset,
            Link,
            MachinePath,
            Secret,
        ]
    }
}

/// One finding about the tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RustdocFinding {
    /// What is wrong.
    pub kind: RustdocFindingKind,
    /// The file of the tree it is about, tree-relative; empty when it is about the tree as a
    /// whole.
    pub file: String,
    /// What exactly: the item path of a missing page, the target of a broken link, the
    /// pattern that matched.
    pub subject: String,
    /// One sentence, specific to this occurrence.
    pub message: String,
    /// What to do, from the kind.
    pub remedy: String,
}

impl RustdocFinding {
    fn new(
        kind: RustdocFindingKind,
        file: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        RustdocFinding {
            kind,
            file: file.into(),
            subject: subject.into(),
            message: message.into(),
            remedy: kind.remedy().to_string(),
        }
    }

    /// The finding as one line, the way the terminal shows it.
    pub fn line_summary(&self) -> String {
        let at = if self.file.is_empty() {
            String::new()
        } else {
            format!("{}: ", self.file)
        };
        format!("{at}{} {} — {}", self.kind.as_str(), self.subject, self.message)
    }
}

impl Ordered for RustdocFinding {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::grouped(self.kind.as_str(), &self.file, &self.subject)
    }
}

/// One exported module and the page rustdoc documents it on: the mapping a consumer that
/// links into the tree reads, rather than deriving a route of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RustdocModuleRoute {
    /// The module's crate path: `majordomus_cli::quality`.
    pub path: String,
    /// Its page, relative to the tree root: `majordomus_cli/quality/index.html`.
    pub route: String,
}

impl Ordered for RustdocModuleRoute {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.path, &self.route)
    }
}

/// How many external links the tree carries to one host. Counted, never fetched: a check
/// that went to the network would be a check whose verdict depended on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RustdocHostLinks {
    /// The host: `doc.rust-lang.org`.
    pub host: String,
    /// How many links point at it, across every page.
    pub links: usize,
}

impl Ordered for RustdocHostLinks {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.host, &self.host)
    }
}

/// What the check counted, so a reader can see what was joined and not only what failed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RustdocCounts {
    /// Items the library exports, of every kind.
    pub exported_items: usize,
    /// Of every documented target, the items that own a page: modules, types, traits, free
    /// functions, constants, statics, aliases, macros.
    pub page_owning_items: usize,
    /// Pages those items derive; more than the items when a re-export documents one twice.
    pub derived_pages: usize,
    /// Item pages found in the tree: every page under a crate directory that is neither a
    /// redirect stub nor a system file.
    pub item_pages: usize,
    /// Pages that only redirect to where an item is documented.
    pub redirect_stubs: usize,
    /// rustdoc's own files: static assets, the search index, source pages, implementor
    /// lists, the settings and help pages, every crate's all-items page and sidebar.
    pub system_files: usize,
    /// Every HTML file of the tree, whatever it is.
    pub html_files: usize,
    /// Every file of the tree.
    pub files: usize,
    /// Relative links followed, across every page.
    pub relative_links: usize,
    /// External links, by host.
    pub external_links: Vec<RustdocHostLinks>,
}

/// One judgement of one rustdoc tree against the crate it documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RustdocReport {
    /// [`SCHEMA`].
    pub schema: String,
    /// The tree judged, repository-relative when it is inside the repository.
    pub tree: String,
    /// The mount the tree is published at, from the web topology or its own declaration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount: Option<String>,
    /// The library's crate name: the directory of the tree it is documented in.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The revision the tree declares it was built from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built_from: Option<String>,
    /// The repository's HEAD when it was judged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// The verdict.
    pub verdict: RustdocVerdict,
    /// Why nothing was judged, when nothing was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What was counted.
    pub counts: RustdocCounts,
    /// Every exported module of the library with its page, in canonical order. Derived from
    /// the crate, so it is answered even when the tree is absent.
    pub modules: Vec<RustdocModuleRoute>,
    /// Every finding, in canonical order.
    pub findings: Vec<RustdocFinding>,
}

impl RustdocReport {
    /// The process exit code: the verdict's.
    pub fn exit_code(&self) -> u8 {
        self.verdict.exit_code()
    }

    /// The same report narrowed to what a caller asked for. The verdict and the counts are
    /// never narrowed: a filter that changed the verdict would make `--kind link` a way of
    /// passing a tree with a missing page.
    pub fn filtered(mut self, kind: Option<RustdocFindingKind>, summary_only: bool) -> Self {
        if let Some(kind) = kind {
            self.findings.retain(|f| f.kind == kind);
        }
        if summary_only {
            self.findings.clear();
        }
        self
    }
}

// ---------------------------------------------------------------------------- the judgement

/// Where the tree to judge is, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tree {
    /// The tree, absolute, and how a report names it.
    At {
        /// The directory.
        dir: PathBuf,
        /// Its name in the report: repository-relative when inside the repository.
        shown: String,
        /// The mount it is published at, when the topology says.
        mount: Option<String>,
    },
    /// There is no tree to judge, and why.
    Absent {
        /// The tree the report names, even though it is absent.
        shown: String,
        /// Why.
        reason: String,
    },
}

/// What the judgement is given: the repository, its crate, the tree and HEAD.
#[derive(Debug, Clone)]
pub struct Subject<'a> {
    /// The repository root, absolute: what a file of the tree must never name.
    pub root: &'a Path,
    /// The crate directory, when the repository has one.
    pub crate_dir: Option<&'a Path>,
    /// The tree.
    pub tree: Tree,
    /// The repository's HEAD, when git could say.
    pub head: Option<&'a str>,
}

/// Judge a tree: read the crate, derive every page, walk the tree once, report.
///
/// Errors only when the crate itself cannot be read — a file that does not parse, a
/// manifest without a name — because an expectation derived from half a crate would be
/// the one result this must not produce. An absent tree is not an error; it is the
/// `no_tree` verdict.
pub fn judge(subject: &Subject<'_>) -> Result<RustdocReport, Error> {
    let Some(crate_dir) = subject.crate_dir else {
        let (shown, _) = shown_of(&subject.tree);
        return Ok(no_tree(
            String::new(),
            shown,
            None,
            subject.head,
            Vec::new(),
            "the repository carries no Rust crate, so there is nothing a rustdoc tree documents"
                .into(),
        ));
    };
    let targets = targets(crate_dir)?;
    let library = Inventory::of_target(crate_dir, "src/lib.rs", super::source::CRATE)?;
    let mut binaries = Vec::new();
    for (name, root) in &targets.bins {
        binaries.push((
            name.clone(),
            Inventory::of_target(crate_dir, root, name)?,
        ));
    }
    let expected = Expected::derive(&library, &targets.lib, &binaries);
    match &subject.tree {
        Tree::Absent { shown, reason } => Ok(no_tree(
            targets.lib,
            shown.clone(),
            None,
            subject.head,
            expected.modules,
            reason.clone(),
        )),
        Tree::At { dir, shown, mount } => {
            if !dir.is_dir() {
                return Ok(no_tree(
                    targets.lib,
                    shown.clone(),
                    mount.clone(),
                    subject.head,
                    expected.modules,
                    format!(
                        "{shown} does not exist: the producer, {PRODUCER}, has not run in this checkout"
                    ),
                ));
            }
            let leaks = Leaks::shipped().map_err(|reason| Error::InvalidSource {
                path: super::leaks::SOURCE.into(),
                reason,
            })?;
            Ok(verify(
                dir,
                shown,
                mount.clone(),
                &expected,
                subject.head,
                subject.root,
                leaks,
            ))
        }
    }
}

fn shown_of(tree: &Tree) -> (String, Option<String>) {
    match tree {
        Tree::At { shown, mount, .. } => (shown.clone(), mount.clone()),
        Tree::Absent { shown, .. } => (shown.clone(), None),
    }
}

fn no_tree(
    krate: String,
    tree: String,
    mount: Option<String>,
    head: Option<&str>,
    modules: Vec<RustdocModuleRoute>,
    reason: String,
) -> RustdocReport {
    RustdocReport {
        schema: SCHEMA.to_string(),
        tree,
        mount,
        krate,
        built_from: None,
        head: head.map(str::to_string),
        verdict: RustdocVerdict::NoTree,
        reason: Some(reason),
        counts: RustdocCounts::default(),
        modules,
        findings: Vec::new(),
    }
}

/// Everything the crate says the tree must hold.
#[derive(Debug, Clone, Default)]
pub struct Expected {
    /// The library's crate directory in the tree.
    pub lib: String,
    /// Every crate directory the tree documents: the library, then each binary.
    pub crates: Vec<String>,
    /// Every page an item owns, to the item it documents (the first one, when two derive it).
    pub pages: BTreeMap<String, String>,
    /// Where each page-owning item was declared, for a missing page's message.
    pub declared_at: HashMap<String, String>,
    /// Every exported module of the library, with its route, in canonical order.
    pub modules: Vec<RustdocModuleRoute>,
    /// Items the library exports.
    pub exported_items: usize,
    /// Page-owning items of every target.
    pub page_owning_items: usize,
}

impl Expected {
    /// Derive the expectation from the library's inventory, its crate name and every
    /// binary's inventory.
    pub fn derive(library: &Inventory, lib: &str, binaries: &[(String, Inventory)]) -> Self {
        let mut out = Expected {
            lib: lib.to_string(),
            crates: vec![lib.to_string()],
            ..Expected::default()
        };
        out.exported_items = library.items.iter().filter(|i| i.exported).count();
        let router = Router::library(library, lib);
        out.add(&router, library);
        for item in library
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Module && i.exported)
        {
            for route in router.routes(item) {
                out.modules.push(RustdocModuleRoute {
                    path: item.path.clone(),
                    route: route.file,
                });
            }
        }
        canonical(&mut out.modules);
        out.modules.dedup();
        for (name, inventory) in binaries {
            out.crates.push(name.clone());
            out.add(&Router::binary(inventory, name), inventory);
        }
        out
    }

    fn add(&mut self, router: &Router<'_>, inventory: &Inventory) {
        for item in &inventory.items {
            if !router.documented(item) || !router.owns_page(item) {
                continue;
            }
            self.page_owning_items += 1;
            for route in router.routes(item) {
                self.declared_at
                    .entry(route.file.clone())
                    .or_insert_with(|| format!("{}:{}", item.file, item.line));
                self.pages.entry(route.file).or_insert(item.path.clone());
            }
        }
    }
}

// ------------------------------------------------------------------------------ the tree walk

/// What a file of the tree is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// The producer's own: the landing page and the declaration at the root.
    Landing,
    /// rustdoc's own machinery.
    System,
    /// A page of an item, under a crate directory.
    ItemPage,
    /// A page that only redirects.
    Redirect,
    /// An HTML page outside every crate directory that is not rustdoc's.
    Stray,
    /// Anything else: not a page, not rustdoc's.
    Other,
}

/// The top-level entries rustdoc writes for the whole tree rather than for a crate.
const SYSTEM_DIRS: &[&str] = &[
    "static.files",
    "search.index",
    "search.desc",
    "src",
    "trait.impl",
    "type.impl",
    "implementors",
];
const SYSTEM_ROOT_FILES: &[&str] = &[
    "help.html",
    "settings.html",
    "crates.js",
    "src-files.js",
    "source-files.js",
    // cargo's lock of target/doc, copied with the tree
    ".lock",
];

fn classify(rel: &str, crates: &HashSet<&str>, redirect: bool) -> Class {
    if rel == crate::web::discover::DECLARATION_FILE || rel == "index.html" {
        return Class::Landing;
    }
    let (first, rest) = rel.split_once('/').unwrap_or((rel, ""));
    if rest.is_empty() {
        if SYSTEM_ROOT_FILES.contains(&first)
            || (first.starts_with("search-index") && first.ends_with(".js"))
        {
            return Class::System;
        }
        return if first.ends_with(".html") {
            Class::Stray
        } else {
            Class::Other
        };
    }
    if SYSTEM_DIRS.contains(&first) {
        return Class::System;
    }
    let html = rel.ends_with(".html");
    if crates.contains(first) {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if rest == "all.html" || (name.starts_with("sidebar-items") && name.ends_with(".js")) {
            return Class::System;
        }
        if html {
            return if redirect {
                Class::Redirect
            } else {
                Class::ItemPage
            };
        }
        return Class::Other;
    }
    if html {
        if redirect {
            Class::Redirect
        } else {
            Class::Stray
        }
    } else {
        Class::Other
    }
}

/// Every file under `dir`, tree-relative with `/` separators, in byte order. Symbolic links
/// are not followed: a tree that links out of itself is judged by what it holds.
fn files_of(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(dir.join(&rel)) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let child = rel.join(entry.file_name());
            if kind.is_dir() {
                stack.push(child);
            } else if kind.is_file() {
                out.push(child.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

fn verify(
    dir: &Path,
    shown: &str,
    mount: Option<String>,
    expected: &Expected,
    head: Option<&str>,
    root: &Path,
    leaks: &Leaks,
) -> RustdocReport {
    let files = files_of(dir);
    let present: HashSet<&str> = files.iter().map(String::as_str).collect();
    let directories: HashSet<&str> = files
        .iter()
        .flat_map(|f| {
            f.match_indices('/')
                .map(move |(i, _)| &f[..i])
                .collect::<Vec<_>>()
        })
        .collect();
    let crates: HashSet<&str> = expected.crates.iter().map(String::as_str).collect();
    let root_text = root.to_string_lossy().into_owned();
    let mut findings = Vec::new();
    let mut counts = RustdocCounts {
        exported_items: expected.exported_items,
        page_owning_items: expected.page_owning_items,
        derived_pages: expected.pages.len(),
        files: files.len(),
        ..RustdocCounts::default()
    };
    let mut hosts: BTreeMap<String, usize> = BTreeMap::new();
    // a broken link is reported once per target, with how many pages carry it: one missing
    // stylesheet is one defect, not one per page of the crate
    let mut broken: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut item_pages: BTreeSet<&str> = BTreeSet::new();
    let mut redirects: HashSet<&str> = HashSet::new();

    for rel in &files {
        let Ok(bytes) = std::fs::read(dir.join(rel)) else {
            findings.push(RustdocFinding::new(
                RustdocFindingKind::Link,
                rel.clone(),
                rel.clone(),
                "the file is listed in the tree and cannot be read",
            ));
            continue;
        };
        let html = rel.ends_with(".html");
        let mut redirect = false;
        if html {
            let page = Page::read(&bytes);
            redirect = page.redirect;
            counts.html_files += 1;
            for link in &page.links {
                match resolve(rel, link, &present, &directories) {
                    Resolution::Internal => counts.relative_links += 1,
                    Resolution::External(host) => *hosts.entry(host).or_default() += 1,
                    Resolution::Ignored => {}
                    Resolution::Broken(target) => {
                        counts.relative_links += 1;
                        let entry = broken.entry(target).or_insert((rel.clone(), 0));
                        entry.1 += 1;
                    }
                }
            }
        }
        match classify(rel, &crates, redirect) {
            Class::System => counts.system_files += 1,
            Class::Redirect => {
                counts.redirect_stubs += 1;
                redirects.insert(rel.as_str());
            }
            Class::ItemPage => {
                counts.item_pages += 1;
                item_pages.insert(rel.as_str());
                if !expected.pages.contains_key(rel.as_str()) {
                    findings.push(RustdocFinding::new(
                        RustdocFindingKind::OrphanPage,
                        rel.clone(),
                        rel.clone(),
                        "an item page that no item of the crate derives",
                    ));
                }
            }
            Class::Stray => findings.push(RustdocFinding::new(
                RustdocFindingKind::OrphanPage,
                rel.clone(),
                rel.clone(),
                "an HTML page outside every crate directory that is neither rustdoc's nor the producer's",
            )),
            Class::Landing | Class::Other => {}
        }
        // the machine it was built on, and anything shaped like a credential
        if !root_text.is_empty() && contains(&bytes, root_text.as_bytes()) {
            findings.push(RustdocFinding::new(
                RustdocFindingKind::MachinePath,
                rel.clone(),
                "repository-root",
                "names the absolute path of the repository the tree was judged in",
            ));
        }
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for leak in leaks.scan(
            &bytes,
            &[
                LeakClass::MachinePath,
                LeakClass::RunnerPath,
                LeakClass::PrivateHost,
                LeakClass::Credential,
            ],
        ) {
            if !seen.insert(leak.name.clone()) {
                continue;
            }
            let (kind, message) = match leak.class {
                LeakClass::Credential => (
                    RustdocFindingKind::Secret,
                    format!(
                        "line {} has the shape of a credential ({}); the value is not repeated here",
                        leak.line,
                        super::leaks::SOURCE
                    ),
                ),
                _ => (
                    RustdocFindingKind::MachinePath,
                    format!(
                        "line {} names {}",
                        leak.line,
                        leak.excerpt.unwrap_or_default()
                    ),
                ),
            };
            findings.push(RustdocFinding::new(kind, rel.clone(), leak.name, message));
        }
    }

    // every page an exported item owns, at the route rustdoc gives it
    for (page, item) in &expected.pages {
        if item_pages.contains(page.as_str()) {
            continue;
        }
        let why = if redirects.contains(page.as_str()) {
            "only a redirect stub is there, so rustdoc documents the item somewhere this check did not derive"
        } else {
            "no page is there"
        };
        let at = expected
            .declared_at
            .get(page)
            .map(|d| format!(" (declared at {d})"))
            .unwrap_or_default();
        findings.push(RustdocFinding::new(
            RustdocFindingKind::MissingPage,
            page.clone(),
            item.clone(),
            format!("{why}{at}"),
        ));
    }

    for (target, (first, pages)) in broken {
        findings.push(RustdocFinding::new(
            RustdocFindingKind::Link,
            first,
            target.clone(),
            format!("resolves to no file of the tree; carried by {pages} link(s)"),
        ));
    }

    let built_from = declaration(dir, head, &mut findings);
    crate_index(dir, &expected.lib, &present, &directories, &mut findings);

    counts.external_links = hosts
        .into_iter()
        .map(|(host, links)| RustdocHostLinks { host, links })
        .collect();
    canonical(&mut counts.external_links);
    canonical(&mut findings);
    let verdict = if findings.is_empty() {
        RustdocVerdict::Clean
    } else {
        RustdocVerdict::Findings
    };
    RustdocReport {
        schema: SCHEMA.to_string(),
        tree: shown.to_string(),
        mount,
        krate: expected.lib.clone(),
        built_from,
        head: head.map(str::to_string),
        verdict,
        reason: None,
        counts,
        modules: expected.modules.clone(),
        findings,
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// The producer's declaration: what the tree says it is and what it was built from.
fn declaration(
    dir: &Path,
    head: Option<&str>,
    findings: &mut Vec<RustdocFinding>,
) -> Option<String> {
    use crate::web::discover::{Declaration, DECLARATION_FILE};
    let undeclared = |findings: &mut Vec<RustdocFinding>, why: String| {
        findings.push(RustdocFinding::new(
            RustdocFindingKind::Undeclared,
            DECLARATION_FILE,
            DECLARATION_FILE,
            why,
        ))
    };
    let Ok(text) = std::fs::read_to_string(dir.join(DECLARATION_FILE)) else {
        undeclared(
            findings,
            "the tree carries no declaration, so nothing says what it is or which commit it was built from"
                .into(),
        );
        return None;
    };
    let decl: Declaration = match serde_json::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            undeclared(findings, format!("does not parse as web-surface/v1: {e}"));
            return None;
        }
    };
    if decl.id != SURFACE {
        undeclared(
            findings,
            format!("declares the surface '{}', not '{SURFACE}'", decl.id),
        );
    }
    let Some(built) = decl.built_from.clone() else {
        undeclared(
            findings,
            "declares no built_from, so the tree cannot be proven to be of any commit".into(),
        );
        return None;
    };
    match head {
        Some(h) if h == built => {}
        Some(h) => findings.push(RustdocFinding::new(
            RustdocFindingKind::Stale,
            DECLARATION_FILE,
            built.clone(),
            format!("built from {built}, and the repository's HEAD is {h}"),
        )),
        None => findings.push(RustdocFinding::new(
            RustdocFindingKind::Stale,
            DECLARATION_FILE,
            built.clone(),
            "the repository has no HEAD to compare built_from against, so freshness is unproven",
        )),
    }
    Some(built)
}

/// The library's index page: present, titled as rustdoc titles a crate, and every asset it
/// loads present — its stylesheets and scripts, the fonts it preloads and its stylesheets
/// reference, the scripts its `rustdoc-vars` name, and the root of the search index.
fn crate_index(
    dir: &Path,
    lib: &str,
    present: &HashSet<&str>,
    directories: &HashSet<&str>,
    findings: &mut Vec<RustdocFinding>,
) {
    let index = format!("{lib}/index.html");
    let Ok(bytes) = std::fs::read(dir.join(&index)) else {
        findings.push(RustdocFinding::new(
            RustdocFindingKind::CrateIndex,
            index,
            lib,
            "the library has no index page",
        ));
        return;
    };
    let text = String::from_utf8_lossy(&bytes);
    let want = format!("{lib} - Rust");
    let title = text
        .split_once("<title>")
        .and_then(|(_, rest)| rest.split_once("</title>"))
        .map(|(t, _)| t.trim().to_string());
    if title.as_deref() != Some(want.as_str()) {
        findings.push(RustdocFinding::new(
            RustdocFindingKind::CrateIndex,
            index.clone(),
            lib,
            format!(
                "is titled {:?}, and rustdoc titles the library's index {want:?}",
                title.unwrap_or_default()
            ),
        ));
    }
    let page = Page::read(&bytes);
    let mut needed: BTreeSet<String> = BTreeSet::new();
    for asset in &page.assets {
        if let Some(target) = internal_target(&index, asset) {
            needed.insert(target);
        }
    }
    let static_root = page.vars.get("data-static-root-path").cloned();
    if let Some(static_root) = &static_root {
        for (name, value) in &page.vars {
            if name.starts_with("data-") && name.ends_with("-js") && !value.is_empty() {
                if let Some(t) = internal_target(&index, &format!("{static_root}{value}")) {
                    needed.insert(t);
                }
            }
        }
        for font in &page.fonts {
            if let Some(t) = internal_target(&index, &format!("{static_root}{font}")) {
                needed.insert(t);
            }
        }
    }
    // what the stylesheets load in turn: the fonts, by url()
    let sheets: Vec<String> = needed
        .iter()
        .filter(|t| t.ends_with(".css"))
        .cloned()
        .collect();
    for sheet in sheets {
        let Ok(css) = std::fs::read(dir.join(&sheet)) else {
            continue;
        };
        for url in css_urls(&css) {
            if let Some(t) = internal_target(&sheet, &url) {
                needed.insert(t);
            }
        }
    }
    for target in needed {
        if !present.contains(target.as_str()) {
            findings.push(RustdocFinding::new(
                RustdocFindingKind::Asset,
                index.clone(),
                target,
                "the library's index page loads it and the tree does not hold it",
            ));
        }
    }
    // the search index is loaded by script, not by an attribute: rustdoc 1.98 reads
    // search.index/root.js from the root path, an older one search-index.js at the root
    let root_path = page
        .vars
        .get("data-root-path")
        .cloned()
        .unwrap_or_else(|| "../".into());
    let modern = internal_target(&index, &format!("{root_path}search.index/root.js"));
    let has_modern = modern.as_deref().is_some_and(|t| present.contains(t));
    let has_legacy = present
        .iter()
        .any(|f| !f.contains('/') && f.starts_with("search-index") && f.ends_with(".js"));
    if !has_modern && !has_legacy {
        let _ = directories;
        findings.push(RustdocFinding::new(
            RustdocFindingKind::Asset,
            index,
            modern.unwrap_or_else(|| "search.index/root.js".into()),
            "the search index's root is absent, so the search box of every page answers nothing",
        ));
    }
}

/// Every `url(...)` a stylesheet names, quotes removed, data URIs left out.
fn css_urls(css: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(css);
    let mut out = Vec::new();
    let mut rest = text.as_ref();
    while let Some(at) = rest.find("url(") {
        rest = &rest[at + 4..];
        let Some(end) = rest.find(')') else { break };
        let url = rest[..end].trim().trim_matches(|c| c == '"' || c == '\'');
        if !url.is_empty() && !url.starts_with("data:") && !url.starts_with('#') {
            out.push(url.to_string());
        }
        rest = &rest[end..];
    }
    out
}

// ------------------------------------------------------------------------------- the links

/// What a link of a page is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Resolution {
    /// A relative link to a file of the tree.
    Internal,
    /// A link to another origin, by host: counted, never fetched.
    External(String),
    /// Not a link to a file: a same-page anchor, `mailto:`, `javascript:`, `data:`.
    Ignored,
    /// A link that resolves to nothing the tree holds, or leaves the tree; the target as
    /// resolved (or as written, when it cannot be resolved inside the tree).
    Broken(String),
}

/// Resolve one `href` or `src` of the page at `from`.
///
/// The fragment and the query are dropped before resolution, which is what accepts
/// rustdoc's `#12-40` source-line ranges and its `?search=` links. An absolute path is
/// broken by definition: the tree is mounted under a prefix, so `/x` leaves it.
fn resolve(
    from: &str,
    link: &str,
    present: &HashSet<&str>,
    directories: &HashSet<&str>,
) -> Resolution {
    let link = link.trim();
    if link.is_empty() || link.starts_with('#') {
        return Resolution::Ignored;
    }
    let lower = link.to_ascii_lowercase();
    for scheme in ["mailto:", "javascript:", "data:", "tel:"] {
        if lower.starts_with(scheme) {
            return Resolution::Ignored;
        }
    }
    if let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .or_else(|| lower.strip_prefix("//"))
    {
        let host = rest
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default()
            .rsplit('@')
            .next()
            .unwrap_or_default()
            .to_string();
        return Resolution::External(host);
    }
    if has_scheme(&lower) {
        return Resolution::Broken(link.to_string());
    }
    let Some(target) = internal_target(from, link) else {
        return Resolution::Broken(link.to_string());
    };
    if present.contains(target.as_str()) {
        return Resolution::Internal;
    }
    // a directory link is answered by its index page
    let dir = target.trim_end_matches('/');
    if (target.is_empty() || directories.contains(dir) || target.ends_with('/'))
        && present.contains(format!("{}index.html", with_slash(dir)).as_str())
    {
        return Resolution::Internal;
    }
    Resolution::Broken(target)
}

fn with_slash(dir: &str) -> String {
    if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    }
}

fn has_scheme(link: &str) -> bool {
    match link.find(':') {
        Some(i) => {
            let scheme = &link[..i];
            !scheme.is_empty()
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                && !link[..i].contains('/')
        }
        None => false,
    }
}

/// The tree-relative file a relative link of the page at `from` names, or `None` when it
/// climbs out of the tree or is written as an absolute path.
fn internal_target(from: &str, link: &str) -> Option<String> {
    let link = link.split(['#', '?']).next().unwrap_or_default();
    if link.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(&link.replace("&amp;", "&"));
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop(); // the page's own file name
    let trailing = decoded.ends_with('/');
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    let mut out = parts.join("/");
    if trailing && !out.is_empty() {
        out.push('/');
    }
    Some(out)
}

fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 0 && i + 2 <= bytes.len() - 1 {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ------------------------------------------------------------------------ reading one page

/// What one HTML page carries that the check reads: its links, whether it only redirects,
/// and — read from the library's index page — the assets it loads.
#[derive(Debug, Default)]
struct Page {
    /// Every `href` and `src` outside `<script>` and `<style>` bodies.
    links: Vec<String>,
    /// A `<meta http-equiv="refresh">`: the page is a redirect stub.
    redirect: bool,
    /// The `href` of every `<link>` and the `src` of every `<script>`.
    assets: Vec<String>,
    /// The attributes of `<meta name="rustdoc-vars">`.
    vars: BTreeMap<String, String>,
    /// Every `*.woff2` a script of the page preloads by name.
    fonts: BTreeSet<String>,
}

impl Page {
    /// Tokenise the page once. The scanner reads tags and their attributes and skips text,
    /// comments, and the bodies of `<script>` and `<style>`: a template literal like
    /// `href="../static.files/${f}"` inside a script is code, not a link, and text is
    /// escaped by rustdoc so a quoted `href=` in prose or in a source page is never a tag.
    fn read(html: &[u8]) -> Page {
        let mut page = Page::default();
        let n = html.len();
        let mut i = 0usize;
        while let Some(off) = html[i..].iter().position(|&b| b == b'<') {
            let start = i + off + 1;
            i = start;
            if html[i..].starts_with(b"!--") {
                i = find(html, i + 3, b"-->").map_or(n, |e| e + 3);
                continue;
            }
            if i < n && matches!(html[i], b'!' | b'?' | b'/') {
                i = html[i..].iter().position(|&b| b == b'>').map_or(n, |e| i + e + 1);
                continue;
            }
            let name_end = html[i..]
                .iter()
                .position(|b| !(b.is_ascii_alphanumeric() || *b == b'-'))
                .map_or(n, |e| i + e);
            if name_end == i {
                continue;
            }
            let tag = html[i..name_end].to_ascii_lowercase();
            i = name_end;
            let mut attrs: Vec<(Vec<u8>, String)> = Vec::new();
            // attributes, until the tag closes
            loop {
                while i < n && html[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= n {
                    break;
                }
                if html[i] == b'>' {
                    i += 1;
                    break;
                }
                if html[i] == b'/' {
                    i += 1;
                    continue;
                }
                let a = i;
                while i < n && !html[i].is_ascii_whitespace() && !matches!(html[i], b'=' | b'>' | b'/') {
                    i += 1;
                }
                let attr = html[a..i].to_ascii_lowercase();
                while i < n && html[i].is_ascii_whitespace() {
                    i += 1;
                }
                let mut value = String::new();
                if i < n && html[i] == b'=' {
                    i += 1;
                    while i < n && html[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    if i < n && (html[i] == b'"' || html[i] == b'\'') {
                        let q = html[i];
                        let v = i + 1;
                        let end = html[v..].iter().position(|&b| b == q).map_or(n, |e| v + e);
                        value = String::from_utf8_lossy(&html[v..end]).into_owned();
                        i = (end + 1).min(n);
                    } else {
                        let v = i;
                        while i < n && !html[i].is_ascii_whitespace() && html[i] != b'>' {
                            i += 1;
                        }
                        value = String::from_utf8_lossy(&html[v..i]).into_owned();
                    }
                }
                if matches!(
                    attr.as_slice(),
                    b"href" | b"src" | b"name" | b"http-equiv" | b"rel"
                ) || attr.starts_with(b"data-")
                {
                    attrs.push((attr, value));
                }
            }
            let get = |k: &[u8]| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
            for (a, v) in &attrs {
                if a == b"href" || a == b"src" {
                    page.links.push(v.clone());
                }
            }
            match tag.as_slice() {
                b"meta" => {
                    if get(b"http-equiv").is_some_and(|v| v.eq_ignore_ascii_case("refresh")) {
                        page.redirect = true;
                    }
                    if get(b"name") == Some("rustdoc-vars") {
                        for (a, v) in &attrs {
                            if a.starts_with(b"data-") {
                                page.vars.insert(String::from_utf8_lossy(a).into_owned(), v.clone());
                            }
                        }
                    }
                }
                b"link" => page.assets.extend(get(b"href").map(str::to_string)),
                b"script" => page.assets.extend(get(b"src").map(str::to_string)),
                _ => {}
            }
            if tag == b"script" || tag == b"style" {
                let close: &[u8] = if tag == b"script" { b"</script" } else { b"</style" };
                let end = find_ascii_ci(html, i, close).unwrap_or(n);
                if tag == b"script" {
                    page.fonts.extend(woff2_names(&html[i..end]));
                }
                i = end;
            }
        }
        page
    }
}

fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

fn find_ascii_ci(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w.eq_ignore_ascii_case(needle))
        .map(|p| from + p)
}

/// Every `name.woff2` token of a script body: how rustdoc's index page preloads its fonts.
fn woff2_names(script: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(script);
    text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .filter(|t| t.ends_with(".woff2") && t.len() > ".woff2".len())
        .map(str::to_string)
        .collect()
}
