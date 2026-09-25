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
//! this crate:
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
//! Two kinds of unresolved reference are rustdoc's and not this crate's, and each is a
//! named class ([`RustdocLinkClass`]) decided by where the reference sits, counted in the
//! report and never a finding: a link inside documentation rustdoc copied from another
//! crate's trait, and the implementors script of a trait page when rustdoc wrote none. Any
//! other relative reference that resolves to nothing is a `link` finding.
//!
//! ```text
//! crate --syn--> Inventory --routes--> expected pages ─┐
//!                                                       ├─> findings, counts, module routes
//! target/web/rustdoc/ --walk--> files, pages, links ───┘
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use regex::bytes::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::leaks::{LeakClass, Leaks};
use super::source::{Inventory, Item, ItemKind};
use crate::error::Error;
use crate::order::{canonical, OrderKey, Ordered};

/// The schema of a report. Bumped when a consumer would have to change.
pub const SCHEMA: &str = "majordomus/rustdoc/v1";

/// The identity of the web surface the tree is published as. The topology owns the name
/// ([`crate::web::discover::RUSTDOC`]); this is the same value under the name this module
/// reads it by, never a second spelling of it.
pub const SURFACE: &str = crate::web::discover::RUSTDOC;

/// What writes the tree, for a reader who has to rebuild it: the producer the topology
/// names ([`crate::web::discover::RUSTDOC_PRODUCER`]), so a remedy here and the 503 the
/// server answers an unbuilt mount with cannot name two different commands.
pub const PRODUCER: &str = crate::web::discover::RUSTDOC_PRODUCER;

// ------------------------------------------------------------------------ the crate's targets

/// The documented targets of a crate, as its manifest declares them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targets {
    /// The library's crate name: `[lib] name`, else the package name with `-` as `_`. This
    /// is the directory rustdoc writes the library into and the first segment of every
    /// route.
    pub lib: String,
    /// The library's root file, crate-relative: `[lib] path`, else `src/lib.rs`.
    pub lib_root: String,
    /// Every binary cargo documents: its crate name and its root file, crate-relative.
    pub bins: Vec<(String, String)>,
}

/// Read the targets from `Cargo.toml`, with the same literal subset discipline the rest of
/// the executable reads manifests with: `name`, `path` and `doc` in `[package]`, `[lib]`
/// and `[[bin]]`, and nothing else. A binary cargo would not document — `doc = false`, or
/// one named as the library — is left out, as cargo leaves it out.
///
/// This is the one place the library's crate name is decided; every route starts with it.
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
    let lib_name = lib.name.unwrap_or_else(|| package_name.replace('-', "_"));
    let lib_root = lib.path.unwrap_or_else(|| "src/lib.rs".into());
    let mut documented = Vec::new();
    if bins.is_empty() && crate_dir.join("src/main.rs").is_file() {
        // cargo's own discovery: a package with a src/main.rs and no [[bin]] builds one
        // binary named after the package
        let krate = package_name.replace('-', "_");
        if krate != lib_name {
            documented.push((krate, "src/main.rs".to_string()));
        }
    }
    for bin in bins.into_iter().filter(|b| b.doc) {
        let Some(name) = bin.name else { continue };
        let krate = name.replace('-', "_");
        if krate == lib_name {
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
        lib: lib_name,
        lib_root,
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
    #[cfg(test)]
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
    modules: HashSet<&'a str>,
    owners: HashMap<&'a str, &'a Item>,
    /// Modules whose chain from the crate root is `pub` all the way down.
    public_modules: HashSet<&'a str>,
    /// A re-exported path, to every alias it is offered under, in canonical order. A glob's
    /// alias is the re-exporting module itself: each item keeps its own remainder.
    aliases: HashMap<&'a str, Vec<String>>,
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
        let mut modules = HashSet::new();
        let mut owners = HashMap::new();
        for item in &inventory.items {
            match item.kind {
                ItemKind::Module => {
                    modules.insert(item.path.as_str());
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
        let mut aliases: HashMap<&str, Vec<String>> = HashMap::new();
        for (alias, target) in &inventory.reexport_aliases {
            // `m::*` offers the items of its target in `m` itself
            let offered = alias.strip_suffix("::*").unwrap_or(alias);
            aliases
                .entry(target.as_str())
                .or_default()
                .push(offered.to_string());
        }
        for list in aliases.values_mut() {
            crate::order::canonical_strings(list);
            list.dedup();
        }
        Router {
            krate,
            everything,
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
        match item.kind {
            ItemKind::Module => self.public_modules.contains(item.path.as_str()),
            ItemKind::Macro => true,
            _ => item.declared_pub && self.public_modules.contains(item.owner.as_str()),
        }
    }

    /// Does this item own a page of its own? A module does, and so does every item with a
    /// page prefix that is declared in a module — a type, a trait, a free function, a
    /// constant, a static, an alias — and an exported macro. A member is an anchor: an
    /// associated function belongs to the type it is implemented on.
    pub fn owns_page(&self, item: &Item) -> bool {
        match item.kind {
            ItemKind::Module | ItemKind::Macro => true,
            kind => page_prefix(kind).is_some() && self.modules.contains(item.owner.as_str()),
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
            RustdocFindingKind::MissingPage => {
                "rebuild the tree with scripts/rust-check --doc; if it persists, the item's route rule in quality::rustdoc is wrong for this rustdoc and must be corrected there, not excepted"
            }
            RustdocFindingKind::OrphanPage => {
                "rustdoc never deletes a page it wrote before, so an item removed from the crate keeps its page in cargo's target/doc: run cargo clean --doc in the crate and rebuild with scripts/rust-check --doc; if it persists, the route rule in quality::rustdoc is wrong for this rustdoc and must be corrected there, not excepted"
            }
            RustdocFindingKind::Stale | RustdocFindingKind::Undeclared => {
                "rebuild the tree with scripts/rust-check --doc, which writes surface.json with built_from = HEAD"
            }
            RustdocFindingKind::CrateIndex | RustdocFindingKind::Asset => {
                "rebuild the tree from a clean target/doc: a partial copy of rustdoc's output is not a tree"
            }
            RustdocFindingKind::Link => {
                "fix the link at its source: a doc comment that links a repository file by a path relative to the source file resolves nowhere under /rustdoc; use an intra-doc link, or the forge URL of the file"
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
        format!("{at}{} - {}", self.subject, self.message)
    }
}

impl Ordered for RustdocFinding {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::grouped(self.kind.as_str(), &self.file, &self.subject)
    }
}

/// A kind of unresolved reference that is rustdoc's and not the crate's: decided by where
/// it sits on its page, counted, and never a finding.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RustdocLinkClass {
    /// A relative link inside a docblock that rustdoc copied from another crate: the
    /// documentation of an implementation listed under *Auto Trait Implementations* or
    /// *Blanket Implementations* of a trait no page of this tree documents (the impl's
    /// header links no trait page of the tree). The text is the other crate's, written
    /// relative to that crate's own pages — tracing's `dispatcher#setting-the-default-subscriber`
    /// — so it resolves nowhere under this tree and nothing in this crate can change it.
    /// The same link anywhere else, or under an implementation of a trait of this crate,
    /// is a `link` finding.
    InheritedDocumentation,
    /// The implementors script a trait's page loads —
    /// `trait.impl/<crate>/<path>/trait.<Name>.js`, for the trait the page documents — when
    /// rustdoc wrote none. rustdoc references it from every trait page and writes it only
    /// when it has implementors to list from the documented crates; the page renders its
    /// own crate's implementors inline and is complete without it. Any other absent script
    /// is a `link` finding.
    UnwrittenImplementors,
}

impl RustdocLinkClass {
    /// The class as it is written everywhere: `inherited-documentation`.
    pub fn as_str(self) -> &'static str {
        match self {
            RustdocLinkClass::InheritedDocumentation => "inherited-documentation",
            RustdocLinkClass::UnwrittenImplementors => "unwritten-implementors",
        }
    }

    /// Every class, in declaration order.
    pub fn all() -> [RustdocLinkClass; 2] {
        [
            RustdocLinkClass::InheritedDocumentation,
            RustdocLinkClass::UnwrittenImplementors,
        ]
    }
}

/// The unresolved references of one [`RustdocLinkClass`]: how many, on how many pages, and
/// every distinct one as the pages write it. Every class is listed, with zeros when the
/// tree has none, so that "none" is an answer and not an omission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RustdocAcceptedLinks {
    /// The class.
    pub class: RustdocLinkClass,
    /// References of the class, across every page.
    pub links: usize,
    /// Pages carrying at least one.
    pub pages: usize,
    /// Every distinct reference as written, fragment included, in canonical order.
    pub written: Vec<String>,
}

impl Ordered for RustdocAcceptedLinks {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(self.class.as_str(), self.class.as_str())
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
    /// Relative links followed, across every page: resolved, broken and accepted alike.
    pub relative_links: usize,
    /// The unresolved references that are rustdoc's own, by class; every class is listed.
    pub accepted_links: Vec<RustdocAcceptedLinks>,
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
    /// The mount the tree is published at, from the web topology.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount: Option<String>,
    /// The library's crate name: the directory of the tree it is documented in. Empty when
    /// the repository has no crate.
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
    /// Every exported module of the library with its page, in canonical order: the one
    /// module-to-route mapping every consumer reads. Derived from the crate, so it is
    /// answered even when the tree is absent.
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
/// manifest without a name — or when the leak definition compiled into the executable does
/// not compile, because an expectation derived from half a crate, or a scan with nothing to
/// look for, would be the one result this must not produce. An absent tree is not an error;
/// it is the `no_tree` verdict.
pub fn judge(subject: &Subject<'_>) -> Result<RustdocReport, Error> {
    let shown = match &subject.tree {
        Tree::At { shown, .. } | Tree::Absent { shown, .. } => shown.clone(),
    };
    let Some(crate_dir) = subject.crate_dir else {
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
    let library = Inventory::of_target(crate_dir, &targets.lib_root, &targets.lib)?;
    let mut binaries = Vec::new();
    for (name, root) in &targets.bins {
        binaries.push((name.clone(), Inventory::of_target(crate_dir, root, name)?));
    }
    let expected = Expected::derive(&library, &targets.lib, &binaries);
    match &subject.tree {
        Tree::Absent { reason, .. } => Ok(no_tree(
            targets.lib,
            shown,
            None,
            subject.head,
            expected.modules,
            reason.clone(),
        )),
        Tree::At { dir, mount, .. } if !dir.is_dir() => Ok(no_tree(
            targets.lib,
            shown.clone(),
            mount.clone(),
            subject.head,
            expected.modules,
            format!(
                "{shown} does not exist: the producer, {PRODUCER}, has not run in this checkout"
            ),
        )),
        Tree::At { dir, mount, .. } => {
            let leaks = Leaks::shipped().map_err(|reason| Error::InvalidSource {
                path: super::leaks::SOURCE.into(),
                reason,
            })?;
            Ok(verify(
                dir,
                &shown,
                mount.clone(),
                &expected,
                subject.head,
                subject.root,
                leaks,
            ))
        }
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
    // cargo's lock of target/doc, when a tree is copied with it
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
    match (html, redirect) {
        (true, true) => Class::Redirect,
        (true, false) => Class::Stray,
        _ => Class::Other,
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
    // the canonical order, so the walk's findings come out the same on every filesystem
    crate::order::canonical_strings(&mut out);
    out
}

/// The accepted references of one class, as they are gathered.
#[derive(Default)]
struct Accepted {
    links: usize,
    pages: BTreeSet<String>,
    written: BTreeSet<String>,
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
        .flat_map(|f| f.match_indices('/').map(move |(i, _)| &f[..i]))
        .collect();
    let crates: HashSet<&str> = expected.crates.iter().map(String::as_str).collect();
    // the repository's own absolute root, as literal bytes: whatever machine the tree was
    // judged on, a page naming it was written with this checkout's location in it
    let root_text = root.to_string_lossy().into_owned();
    let root_pattern = (root_text.len() > 1)
        .then(|| Regex::new(&regex::escape(&root_text)).ok())
        .flatten();
    let mut findings = Vec::new();
    let mut counts = RustdocCounts {
        exported_items: expected.exported_items,
        page_owning_items: expected.page_owning_items,
        derived_pages: expected.pages.len(),
        files: files.len(),
        ..RustdocCounts::default()
    };
    let mut hosts: BTreeMap<String, usize> = BTreeMap::new();
    // a broken link is reported once per target, with how many links and pages carry it and
    // the first page that does: one missing stylesheet is one defect, not one per page
    let mut broken: BTreeMap<String, (String, usize, BTreeSet<String>)> = BTreeMap::new();
    let mut accepted: BTreeMap<RustdocLinkClass, Accepted> = RustdocLinkClass::all()
        .into_iter()
        .map(|c| (c, Accepted::default()))
        .collect();
    let mut item_pages: HashSet<&str> = HashSet::new();
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
        let mut redirect = false;
        if rel.ends_with(".html") {
            let page = Page::read(&bytes);
            redirect = page.redirect;
            counts.html_files += 1;
            for link in &page.links {
                match resolve(rel, &link.value, &present, &directories) {
                    Resolution::Internal => counts.relative_links += 1,
                    Resolution::External(host) => *hosts.entry(host).or_default() += 1,
                    Resolution::Ignored => {}
                    Resolution::Broken(target) => {
                        counts.relative_links += 1;
                        match accepted_class(rel, link, &target) {
                            Some(class) => {
                                let a = accepted.entry(class).or_default();
                                a.links += 1;
                                a.pages.insert(rel.clone());
                                a.written.insert(link.value.clone());
                            }
                            None => {
                                let b = broken
                                    .entry(target)
                                    .or_insert_with(|| (rel.clone(), 0, BTreeSet::new()));
                                b.1 += 1;
                                b.2.insert(rel.clone());
                            }
                        }
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
        leak_findings(rel, &bytes, root_pattern.as_ref(), leaks, &mut findings);
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

    let built_from = declaration(dir, head, &mut findings);
    let missing_assets = crate_index(dir, &expected.lib, &present, &mut findings);

    for (target, (first, links, pages)) in broken {
        // an asset the crate index needs is already its own, more specific finding
        if missing_assets.contains(&target) {
            continue;
        }
        findings.push(RustdocFinding::new(
            RustdocFindingKind::Link,
            first,
            target,
            format!(
                "resolves to no file of the tree; carried by {links} link(s) on {} page(s)",
                pages.len()
            ),
        ));
    }

    counts.accepted_links = accepted
        .into_iter()
        .map(|(class, a)| RustdocAcceptedLinks {
            class,
            links: a.links,
            pages: a.pages.len(),
            written: a.written.into_iter().collect(),
        })
        .collect();
    for a in &mut counts.accepted_links {
        crate::order::canonical_strings(&mut a.written);
    }
    canonical(&mut counts.accepted_links);
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

/// Is this unresolved reference one of rustdoc's own, and of which class?
///
/// Decided by where it sits and what it names, never by a list of files: a link inside a
/// docblock the page reader attributed to another crate's trait, or the script of the very
/// trait the page documents under `trait.impl/` of its own crate.
fn accepted_class(page: &str, link: &Link, target: &str) -> Option<RustdocLinkClass> {
    if link.inherited {
        return Some(RustdocLinkClass::InheritedDocumentation);
    }
    if link.script && is_own_implementors(page, target) {
        return Some(RustdocLinkClass::UnwrittenImplementors);
    }
    None
}

/// Is `target` the implementors script of the trait the page at `page` documents?
///
/// The page is `<crate>/<modules>/trait.<Name>.html`; rustdoc names the script after the
/// trait's own path, which for a re-exported trait is the module that declares it rather
/// than the page's, so what must agree is the crate and the trait's file name.
fn is_own_implementors(page: &str, target: &str) -> bool {
    let (krate, _) = page.split_once('/').unwrap_or(("", ""));
    let Some(name) = page.rsplit('/').next() else {
        return false;
    };
    let Some(trait_name) = name
        .strip_prefix("trait.")
        .and_then(|n| n.strip_suffix(".html"))
    else {
        return false;
    };
    !krate.is_empty()
        && target.starts_with(&format!("trait.impl/{krate}/"))
        && target.ends_with(&format!("/trait.{trait_name}.js"))
}

/// The machine a file was built on, and anything shaped like a credential, in one file.
fn leak_findings(
    rel: &str,
    bytes: &[u8],
    root: Option<&Regex>,
    leaks: &Leaks,
    findings: &mut Vec<RustdocFinding>,
) {
    if root.is_some_and(|r| r.is_match(bytes)) {
        findings.push(RustdocFinding::new(
            RustdocFindingKind::MachinePath,
            rel,
            "repository-root",
            "names the absolute path of the repository the tree was judged in",
        ));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for leak in leaks.scan(
        bytes,
        &[
            LeakClass::MachinePath,
            LeakClass::RunnerPath,
            LeakClass::PrivateHost,
            LeakClass::Credential,
        ],
    ) {
        // one finding per pattern per file: the first line names where to start
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
        findings.push(RustdocFinding::new(kind, rel, leak.name, message));
    }
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
///
/// Answers the targets it found absent, so that a missing asset is reported once, as an
/// asset, and not a second time as the link every page carries to it.
fn crate_index(
    dir: &Path,
    lib: &str,
    present: &HashSet<&str>,
    findings: &mut Vec<RustdocFinding>,
) -> BTreeSet<String> {
    let mut missing = BTreeSet::new();
    let index = format!("{lib}/index.html");
    let Ok(bytes) = std::fs::read(dir.join(&index)) else {
        findings.push(RustdocFinding::new(
            RustdocFindingKind::CrateIndex,
            index,
            lib,
            "the library has no index page",
        ));
        return missing;
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
        if !is_relative(asset) {
            continue;
        }
        if let Some(target) = internal_target(&index, asset) {
            needed.insert(target);
        }
    }
    if let Some(static_root) = page.vars.get("data-static-root-path") {
        for (name, value) in &page.vars {
            if name.ends_with("-js") && !value.is_empty() {
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
                target.clone(),
                "the library's index page loads it and the tree does not hold it",
            ));
            missing.insert(target);
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
        findings.push(RustdocFinding::new(
            RustdocFindingKind::Asset,
            index,
            modern.unwrap_or_else(|| "search.index/root.js".into()),
            "the search index's root is absent, so the search box of every page answers nothing",
        ));
    }
    missing
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
/// broken by definition: the tree is mounted under a prefix, so `/x` leaves it; so is a
/// relative path that climbs above the tree's root.
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
    if (dir.is_empty() || directories.contains(dir) || target.ends_with('/'))
        && present.contains(format!("{}index.html", with_slash(dir)).as_str())
    {
        return Resolution::Internal;
    }
    Resolution::Broken(target)
}

/// Is this a relative reference into the tree: not an anchor, not another origin, not a
/// scheme of its own?
fn is_relative(link: &str) -> bool {
    let link = link.trim();
    !link.is_empty()
        && !link.starts_with('#')
        && !link.starts_with("//")
        && !has_scheme(&link.to_ascii_lowercase())
}

fn with_slash(dir: &str) -> String {
    if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    }
}

/// Does the reference start with a URL scheme (`https:`, `mailto:`)? A colon after a slash
/// is a path, and `super::Subscriber` is no scheme either: a scheme is letters first.
fn has_scheme(link: &str) -> bool {
    let Some(i) = link.find(':') else {
        return false;
    };
    let scheme = &link[..i];
    scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        && !link[i + 1..].starts_with(':')
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

/// `%XX` escapes decoded; anything that is not a valid escape is kept as written.
fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
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

/// One link of a page, with what the check needs to know about where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Link {
    /// The `href` or `src`, as written.
    value: String,
    /// It is the `src` of a `<script>`.
    script: bool,
    /// It sits in a docblock rustdoc copied from another crate: see
    /// [`RustdocLinkClass::InheritedDocumentation`].
    inherited: bool,
}

/// The sections of a type's page that list implementations the crate did not write: the
/// auto traits the compiler implements, and the blanket implementations other crates
/// provide for every type.
const FOREIGN_IMPLEMENTATION_LISTS: [&str; 2] = [
    "synthetic-implementations-list",
    "blanket-implementations-list",
];

/// What one HTML page carries that the check reads: its links, whether it only redirects,
/// and — read from the library's index page — the assets it loads.
#[derive(Debug, Default)]
struct Page {
    /// Every `href` and `src` outside `<script>` and `<style>` bodies.
    links: Vec<Link>,
    /// A `<meta http-equiv="refresh">`: the page is a redirect stub.
    redirect: bool,
    /// The `href` of every `<link>` and the `src` of every `<script>`.
    assets: Vec<String>,
    /// The attributes of `<meta name="rustdoc-vars">`.
    vars: BTreeMap<String, String>,
    /// Every `*.woff2` a script of the page preloads by name.
    fonts: BTreeSet<String>,
}

/// Where the reader is on a page: the nesting of `<div>`s, and which of the regions the
/// inherited-documentation class is decided by are open.
#[derive(Debug, Default)]
struct Position {
    /// `<div>`s open.
    divs: usize,
    /// The depth of the open list of foreign implementations, when inside one.
    foreign: Option<usize>,
    /// The depth of the open docblock, when inside one.
    docblock: Option<usize>,
    /// Inside the `<h3>` header of an implementation in a foreign list.
    impl_header: bool,
    /// That implementation's header links a trait page of this tree: the trait is the
    /// crate's own, and so is everything documented under it.
    impl_ours: bool,
}

impl Position {
    fn inherited(&self) -> bool {
        self.foreign.is_some() && self.docblock.is_some() && !self.impl_ours
    }

    fn open(&mut self, tag: &[u8], id: Option<&str>, class: Option<&str>, href: Option<&str>) {
        let has_class = |c: &str| class.is_some_and(|v| v.split_whitespace().any(|x| x == c));
        match tag {
            b"div" => {
                self.divs += 1;
                if id.is_some_and(|i| FOREIGN_IMPLEMENTATION_LISTS.contains(&i)) {
                    self.foreign = Some(self.divs);
                    self.impl_ours = false;
                }
                if self.docblock.is_none() && has_class("docblock") {
                    self.docblock = Some(self.divs);
                }
            }
            b"h3" if self.foreign.is_some() && has_class("code-header") => {
                self.impl_header = true;
                self.impl_ours = false;
            }
            b"a" if self.impl_header && has_class("trait") && href.is_some_and(is_relative) => {
                self.impl_ours = true;
            }
            _ => {}
        }
    }

    fn close(&mut self, tag: &[u8]) {
        match tag {
            b"div" => {
                if self.foreign == Some(self.divs) {
                    self.foreign = None;
                    self.impl_ours = false;
                }
                if self.docblock == Some(self.divs) {
                    self.docblock = None;
                }
                self.divs = self.divs.saturating_sub(1);
            }
            b"h3" => self.impl_header = false,
            _ => {}
        }
    }
}

/// The name of a tag starting at `at`: ASCII letters, digits and `-`, lower-cased.
fn tag_name(html: &[u8], at: usize) -> (Vec<u8>, usize) {
    let end = html[at..]
        .iter()
        .position(|b| !(b.is_ascii_alphanumeric() || *b == b'-'))
        .map_or(html.len(), |e| at + e);
    (html[at..end].to_ascii_lowercase(), end)
}

impl Page {
    /// Tokenise the page once. The scanner reads tags and their attributes and skips text,
    /// comments, and the bodies of `<script>` and `<style>`: a template literal like
    /// `href="../static.files/${f}"` inside a script is code, not a link, and text is
    /// escaped by rustdoc so a quoted `href=` in prose or in a source page is never a tag.
    fn read(html: &[u8]) -> Page {
        let mut page = Page::default();
        let mut at = Position::default();
        let n = html.len();
        let mut i = 0usize;
        while let Some(off) = html[i..].iter().position(|&b| b == b'<') {
            i += off + 1;
            if html[i..].starts_with(b"!--") {
                i = find(html, i + 3, b"-->").map_or(n, |e| e + 3);
                continue;
            }
            if i < n && html[i] == b'/' {
                let (tag, end) = tag_name(html, i + 1);
                at.close(&tag);
                i = html[end..]
                    .iter()
                    .position(|&b| b == b'>')
                    .map_or(n, |e| end + e + 1);
                continue;
            }
            if i < n && matches!(html[i], b'!' | b'?') {
                i = html[i..]
                    .iter()
                    .position(|&b| b == b'>')
                    .map_or(n, |e| i + e + 1);
                continue;
            }
            let (tag, name_end) = tag_name(html, i);
            if tag.is_empty() {
                continue;
            }
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
                while i < n
                    && !html[i].is_ascii_whitespace()
                    && !matches!(html[i], b'=' | b'>' | b'/')
                {
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
                    b"href" | b"src" | b"name" | b"http-equiv" | b"id" | b"class"
                ) || attr.starts_with(b"data-")
                {
                    attrs.push((attr, value));
                }
            }
            let get = |k: &[u8]| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
            at.open(&tag, get(b"id"), get(b"class"), get(b"href"));
            let script = tag == b"script";
            for (a, v) in &attrs {
                if a == b"href" || a == b"src" {
                    page.links.push(Link {
                        value: v.clone(),
                        script: script && a == b"src",
                        inherited: at.inherited(),
                    });
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
                                page.vars
                                    .insert(String::from_utf8_lossy(a).into_owned(), v.clone());
                            }
                        }
                    }
                }
                b"link" => page.assets.extend(get(b"href").map(str::to_string)),
                b"script" => page.assets.extend(get(b"src").map(str::to_string)),
                _ => {}
            }
            if tag == b"script" || tag == b"style" {
                let close: &[u8] = if script { b"</script" } else { b"</style" };
                let end = find_ascii_ci(html, i, close).unwrap_or(n);
                if script {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------ fixtures

    /// A crate directory holding a manifest and the given source files.
    fn krate(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"fixture-crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        for (path, text) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        dir
    }

    /// The fixture library: one of each kind that owns a page, members of each kind that do
    /// not, a private module re-exported in part, and a macro declared in a nested module.
    const LIB: &str = r#"//! The fixture.
pub mod shapes;
mod hidden;
pub use hidden::Secretive;
pub use hidden::Renamed as Offered;
/// A constant.
pub const LIMIT: usize = 1;
"#;
    const SHAPES: &str = r#"//! Shapes.
/// A struct.
pub struct Square { /// A field.
    pub side: u32 }
impl Square {
    /// A method.
    pub fn area(&self) -> u32 { self.side * self.side }
    /// An associated function.
    pub fn unit() -> Self { Square { side: 1 } }
}
/// An enum.
pub enum Colour { /// A variant.
    Red }
/// A trait.
pub trait Shape {
    /// An associated type.
    type Unit;
    /// An associated constant.
    const SIDES: u32;
    /// A required method.
    fn sides(&self) -> u32;
    /// A provided method.
    fn name(&self) -> &str { "shape" }
}
/// A function.
pub fn draw() {}
/// A static.
pub static ORIGIN: u32 = 0;
/// An alias.
pub type Side = u32;
/// A union.
pub union Bits { /// A field of a union: the inventory records no union's fields.
    pub raw: u32 }
/// Nested.
pub mod deep {
    /// A macro, exported from the crate root whatever module declares it.
    #[macro_export]
    macro_rules! shout { () => {} }
}
"#;
    const HIDDEN: &str = r#"//! Private.
/// Re-exported under its own name.
pub struct Secretive;
/// Re-exported under another.
pub struct Renamed;
"#;

    fn fixture() -> tempfile::TempDir {
        krate(&[
            ("src/lib.rs", LIB),
            ("src/shapes.rs", SHAPES),
            ("src/hidden.rs", HIDDEN),
        ])
    }

    fn inventory(dir: &Path) -> Inventory {
        Inventory::of_target(dir, "src/lib.rs", "fixture_crate").unwrap()
    }

    fn route_of(inv: &Inventory, path: &str) -> Vec<String> {
        let router = Router::library(inv, "fixture_crate");
        let item = inv
            .items
            .iter()
            .find(|i| i.path == path)
            .unwrap_or_else(|| panic!("{path} is not in the inventory"));
        router.routes(item).iter().map(Route::href).collect()
    }

    const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

    /// A tree holding exactly what `expected` derives, with the assets and declaration a
    /// real one carries: the clean baseline every failure fixture breaks one thing of.
    fn clean_tree(expected: &Expected) -> tempfile::TempDir {
        let tree = tempfile::tempdir().unwrap();
        let write = |rel: &str, text: &str| {
            let p = tree.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        for page in expected.pages.keys() {
            if page.ends_with("/index.html") {
                continue;
            }
            // a relative link to the module page beside it: resolves in every fixture
            write(
                page,
                "<!DOCTYPE html><html><head><title>t</title></head><body><a href=\"index.html\">up</a></body></html>",
            );
        }
        for page in expected.pages.keys().filter(|p| p.ends_with("/index.html")) {
            let depth = page.matches('/').count();
            let up = "../".repeat(depth);
            let krate = page.split('/').next().unwrap();
            write(
                page,
                &format!(
                    concat!(
                        "<!DOCTYPE html><html><head><title>{krate} - Rust</title>",
                        "<script>const f=\"Serif-1.woff2\".split(\",\");</script>",
                        "<link rel=\"stylesheet\" href=\"{up}static.files/rustdoc-1.css\">",
                        "<meta name=\"rustdoc-vars\" data-root-path=\"{up}\" ",
                        "data-static-root-path=\"{up}static.files/\" data-search-js=\"search-1.js\">",
                        "<script src=\"{up}static.files/main-1.js\"></script>",
                        "</head><body><a href=\"{up}{krate}/all.html\">all</a>",
                        "<a href=\"{up}src/{krate}/lib.rs.html#1-5\">source</a></body></html>"
                    ),
                    krate = krate,
                    up = up,
                ),
            );
        }
        for krate in &expected.crates {
            write(&format!("{krate}/all.html"), "<html></html>");
            write(
                &format!("{krate}/sidebar-items.js"),
                "window.SIDEBAR_ITEMS={};",
            );
            write(&format!("src/{krate}/lib.rs.html"), "<html></html>");
        }
        write(
            "static.files/rustdoc-1.css",
            "@font-face{src:url(\"Serif-1.woff2\")}",
        );
        write("static.files/Serif-1.woff2", "font");
        write("static.files/main-1.js", "main");
        write("static.files/search-1.js", "search");
        write("search.index/root.js", "root");
        write("crates.js", "window.ALL_CRATES=[];");
        write(
            "index.html",
            &format!(
                "<!doctype html><title>landing</title><a href=\"{}/index.html\">the library</a>",
                expected.lib
            ),
        );
        write(
            "surface.json",
            &format!(
                r#"{{"schema":"web-surface/v1","id":"rustdoc","mount":"/rustdoc","built_from":"{HEAD}"}}"#
            ),
        );
        tree
    }

    fn judged(krate_dir: &Path, tree: &Path) -> RustdocReport {
        judge(&Subject {
            root: Path::new("/nonexistent/repository/root"),
            crate_dir: Some(krate_dir),
            tree: Tree::At {
                dir: tree.to_path_buf(),
                shown: "target/web/rustdoc".into(),
                mount: Some("/rustdoc".into()),
            },
            head: Some(HEAD),
        })
        .unwrap()
    }

    fn expected_of(dir: &Path) -> Expected {
        let targets = targets(dir).unwrap();
        let lib = Inventory::of_target(dir, &targets.lib_root, &targets.lib).unwrap();
        Expected::derive(&lib, &targets.lib, &[])
    }

    fn kinds(report: &RustdocReport) -> Vec<(RustdocFindingKind, String)> {
        report
            .findings
            .iter()
            .map(|f| (f.kind, f.subject.clone()))
            .collect()
    }

    fn put(tree: &Path, rel: &str, text: &str) {
        let p = tree.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    // ------------------------------------------------------------------- route derivation

    #[test]
    fn every_kind_derives_the_route_rustdoc_gives_it() {
        let dir = fixture();
        let inv = inventory(dir.path());
        let r = |path: &str| route_of(&inv, path);
        for (path, want) in [
            ("fixture_crate", vec!["fixture_crate/index.html"]),
            (
                "fixture_crate::shapes",
                vec!["fixture_crate/shapes/index.html"],
            ),
            (
                "fixture_crate::shapes::deep",
                vec!["fixture_crate/shapes/deep/index.html"],
            ),
            (
                "fixture_crate::shapes::Square",
                vec!["fixture_crate/shapes/struct.Square.html"],
            ),
            (
                "fixture_crate::shapes::Colour",
                vec!["fixture_crate/shapes/enum.Colour.html"],
            ),
            (
                "fixture_crate::shapes::Shape",
                vec!["fixture_crate/shapes/trait.Shape.html"],
            ),
            (
                "fixture_crate::shapes::draw",
                vec!["fixture_crate/shapes/fn.draw.html"],
            ),
            (
                "fixture_crate::shapes::ORIGIN",
                vec!["fixture_crate/shapes/static.ORIGIN.html"],
            ),
            (
                "fixture_crate::shapes::Side",
                vec!["fixture_crate/shapes/type.Side.html"],
            ),
            (
                "fixture_crate::shapes::Bits",
                vec!["fixture_crate/shapes/union.Bits.html"],
            ),
            (
                "fixture_crate::LIMIT",
                vec!["fixture_crate/constant.LIMIT.html"],
            ),
            // exported from the crate root, whatever module declares it
            (
                "fixture_crate::shout",
                vec!["fixture_crate/macro.shout.html"],
            ),
            // members: an anchor on the owner's page
            (
                "fixture_crate::shapes::Square::area",
                vec!["fixture_crate/shapes/struct.Square.html#method.area"],
            ),
            (
                "fixture_crate::shapes::Square::unit",
                vec!["fixture_crate/shapes/struct.Square.html#method.unit"],
            ),
            (
                "fixture_crate::shapes::Square::side",
                vec!["fixture_crate/shapes/struct.Square.html#structfield.side"],
            ),
            (
                "fixture_crate::shapes::Colour::Red",
                vec!["fixture_crate/shapes/enum.Colour.html#variant.Red"],
            ),
            (
                "fixture_crate::shapes::Shape::sides",
                vec!["fixture_crate/shapes/trait.Shape.html#tymethod.sides"],
            ),
            (
                "fixture_crate::shapes::Shape::name",
                vec!["fixture_crate/shapes/trait.Shape.html#method.name"],
            ),
            (
                "fixture_crate::shapes::Shape::Unit",
                vec!["fixture_crate/shapes/trait.Shape.html#associatedtype.Unit"],
            ),
            (
                "fixture_crate::shapes::Shape::SIDES",
                vec!["fixture_crate/shapes/trait.Shape.html#associatedconstant.SIDES"],
            ),
            // a re-export of a private module is documented where it is offered, under
            // the name it is offered by
            (
                "fixture_crate::hidden::Secretive",
                vec!["fixture_crate/struct.Secretive.html"],
            ),
            (
                "fixture_crate::hidden::Renamed",
                vec!["fixture_crate/struct.Offered.html"],
            ),
            // and the private module itself is documented nowhere
            ("fixture_crate::hidden", vec![]),
        ] {
            assert_eq!(r(path), want, "{path}");
        }
    }

    #[test]
    fn the_crate_segment_is_the_manifests_and_never_a_constant() {
        let dir = krate(&[("src/lib.rs", "//! Root.\n/// F.\npub fn f() {}\n")]);
        assert_eq!(targets(dir.path()).unwrap().lib, "fixture_crate");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"a-b\"\n[lib]\nname = \"other_name\"\npath = \"src/lib.rs\"\n",
        )
        .unwrap();
        let t = targets(dir.path()).unwrap();
        assert_eq!(t.lib, "other_name");
        let lib = Inventory::of_target(dir.path(), &t.lib_root, &t.lib).unwrap();
        let expected = Expected::derive(&lib, &t.lib, &[]);
        assert!(
            expected.pages.contains_key("other_name/fn.f.html"),
            "{:?}",
            expected.pages
        );
        assert!(expected.pages.contains_key("other_name/index.html"));
    }

    #[test]
    fn a_binary_is_a_crate_of_its_own_documented_with_its_private_items() {
        let dir = krate(&[
            ("src/lib.rs", "//! Root.\n"),
            (
                "src/main.rs",
                "//! The executable.\nfn main() {}\nfn helper() {}\n",
            ),
        ]);
        // a package whose binary would share the library's crate name: cargo documents the
        // library alone, and so does the expectation
        let t = targets(dir.path()).unwrap();
        assert!(t.bins.is_empty(), "{:?}", t.bins);
        // a named [[bin]] is a crate of its own
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"fixture-crate\"\n[[bin]]\nname = \"tool\"\npath = \"src/main.rs\"\n",
        )
        .unwrap();
        let t = targets(dir.path()).unwrap();
        assert_eq!(
            t.bins,
            vec![("tool".to_string(), "src/main.rs".to_string())]
        );
        let bin = Inventory::of_target(dir.path(), "src/main.rs", "tool").unwrap();
        let lib = Inventory::of_target(dir.path(), &t.lib_root, &t.lib).unwrap();
        let expected = Expected::derive(&lib, &t.lib, &[("tool".into(), bin)]);
        for page in [
            "tool/index.html",
            "tool/fn.main.html",
            "tool/fn.helper.html",
        ] {
            assert!(
                expected.pages.contains_key(page),
                "{page}: {:?}",
                expected.pages
            );
        }
        assert_eq!(expected.crates, vec!["fixture_crate", "tool"]);
    }

    #[test]
    fn a_glob_re_export_documents_every_item_in_the_module_that_offers_it() {
        let dir = krate(&[
            ("src/lib.rs", "//! Root.\nmod inner;\npub use inner::*;\n"),
            (
                "src/inner.rs",
                "//! Inner.\n/// A.\npub struct A;\n/// B.\npub fn b() {}\n",
            ),
        ]);
        let inv = inventory(dir.path());
        assert_eq!(
            route_of(&inv, "fixture_crate::inner::A"),
            vec!["fixture_crate/struct.A.html"]
        );
        assert_eq!(
            route_of(&inv, "fixture_crate::inner::b"),
            vec!["fixture_crate/fn.b.html"]
        );
    }

    /// The invariant over the crate this module lives in: every item rustdoc gives a page
    /// derives one, and no two items derive the same file — the two ways a route rule can
    /// be wrong without any fixture noticing.
    #[test]
    fn the_real_crate_derives_a_page_for_every_page_owning_item_and_never_one_page_twice() {
        let dir = Path::new(".");
        let t = targets(dir).unwrap();
        assert_eq!(t.lib, "majordomus_cli", "read from Cargo.toml's [lib] name");
        let lib = Inventory::of_target(dir, &t.lib_root, &t.lib).unwrap();
        let mut targets_seen: Vec<(String, Inventory, bool)> = vec![(t.lib.clone(), lib, false)];
        for (name, root) in &t.bins {
            targets_seen.push((
                name.clone(),
                Inventory::of_target(dir, root, name).unwrap(),
                true,
            ));
        }
        let mut owners: BTreeMap<String, String> = BTreeMap::new();
        let mut page_owning = 0;
        for (name, inv, binary) in &targets_seen {
            let router = if *binary {
                Router::binary(inv, name)
            } else {
                Router::library(inv, name)
            };
            assert_eq!(
                inv.items[0].path,
                name.as_str(),
                "the walk starts at the crate root"
            );
            for item in inv
                .items
                .iter()
                .filter(|i| router.documented(i) && router.owns_page(i))
            {
                page_owning += 1;
                let routes = router.routes(item);
                assert!(
                    !routes.is_empty(),
                    "{} ({}) is exported and owns a page, and derives none",
                    item.path,
                    item.kind.noun()
                );
                for route in routes {
                    assert!(route.fragment.is_none());
                    // one path declared twice under exclusive `cfg`s — the unix and the
                    // other `stdin_is_a_pipe` — is one item to rustdoc, which documents the
                    // variant the build compiles; two different paths on one page is the defect
                    if let Some(other) = owners.insert(route.file.clone(), item.path.clone()) {
                        assert_eq!(other, item.path, "both derive {}", route.file);
                    }
                }
            }
            // and every documented member lands on a page some item owns
            for item in inv
                .items
                .iter()
                .filter(|i| router.documented(i) && !router.owns_page(i))
            {
                for route in router.routes(item) {
                    assert!(route.fragment.is_some(), "{}", item.path);
                }
            }
        }
        assert!(
            page_owning > 500,
            "the crate exports a surface: {page_owning}"
        );
        assert!(owners.contains_key("majordomus_cli/index.html"));
        assert!(owners.contains_key("majordomus_cli/quality/index.html"));
        assert!(
            owners.contains_key("majordomus/fn.main.html"),
            "the binary is documented"
        );
    }

    // ------------------------------------------------------------------ the verifier

    #[test]
    fn a_clean_tree_has_no_findings_and_exits_zero() {
        let dir = fixture();
        let expected = expected_of(dir.path());
        let tree = clean_tree(&expected);
        let report = judged(dir.path(), tree.path());
        assert_eq!(report.findings, vec![], "{:#?}", report.findings);
        assert_eq!(report.verdict, RustdocVerdict::Clean);
        assert_eq!(report.exit_code(), 0);
        assert_eq!(report.krate, "fixture_crate");
        assert_eq!(report.built_from.as_deref(), Some(HEAD));
        // every derived page is there, the modules' index pages included, and nothing else
        assert_eq!(report.counts.item_pages, report.counts.derived_pages);
        assert_eq!(report.counts.redirect_stubs, 0);
        assert!(report.counts.relative_links > 0);
        // every accepted class is listed, with zero when the tree has none
        assert_eq!(
            report
                .counts
                .accepted_links
                .iter()
                .map(|a| (a.class, a.links))
                .collect::<Vec<_>>(),
            vec![
                (RustdocLinkClass::InheritedDocumentation, 0),
                (RustdocLinkClass::UnwrittenImplementors, 0)
            ]
        );
        // the module routes are the mapping every consumer reads
        let modules: Vec<(&str, &str)> = report
            .modules
            .iter()
            .map(|m| (m.path.as_str(), m.route.as_str()))
            .collect();
        assert!(modules.contains(&("fixture_crate::shapes", "fixture_crate/shapes/index.html")));
        assert!(!modules.iter().any(|(p, _)| p.contains("hidden")));
    }

    #[test]
    fn an_exported_item_without_its_page_is_a_missing_page_naming_the_item() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        std::fs::remove_file(tree.path().join("fixture_crate/shapes/struct.Square.html")).unwrap();
        let report = judged(dir.path(), tree.path());
        assert_eq!(
            kinds(&report),
            vec![(
                RustdocFindingKind::MissingPage,
                "fixture_crate::shapes::Square".to_string()
            )]
        );
        assert!(
            report.findings[0].message.contains("src/shapes.rs:"),
            "{:?}",
            report.findings
        );
        assert_eq!(report.exit_code(), 10);
    }

    #[test]
    fn a_page_no_item_derives_is_an_orphan() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        put(
            tree.path(),
            "fixture_crate/struct.Ghost.html",
            "<html></html>",
        );
        put(tree.path(), "stray.html", "<html></html>");
        let report = judged(dir.path(), tree.path());
        assert_eq!(
            kinds(&report),
            vec![
                (
                    RustdocFindingKind::OrphanPage,
                    "fixture_crate/struct.Ghost.html".into()
                ),
                (RustdocFindingKind::OrphanPage, "stray.html".into()),
            ]
        );
    }

    #[test]
    fn a_redirect_stub_is_neither_an_orphan_nor_a_page() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        // where rustdoc leaves the original path of an item it documents at its re-export
        put(
            tree.path(),
            "fixture_crate/hidden/struct.Secretive.html",
            "<!DOCTYPE html><html><head><meta http-equiv=\"refresh\" content=\"0;URL=../../fixture_crate/struct.Secretive.html\"></head><body><a href=\"../../fixture_crate/struct.Secretive.html\">x</a></body></html>",
        );
        let report = judged(dir.path(), tree.path());
        assert_eq!(report.findings, vec![]);
        assert_eq!(report.counts.redirect_stubs, 1);
        // and a stub where a page belongs is still a missing page, and says why
        std::fs::remove_file(tree.path().join("fixture_crate/struct.Secretive.html")).unwrap();
        put(
            tree.path(),
            "fixture_crate/struct.Secretive.html",
            "<meta http-equiv=\"refresh\" content=\"0;URL=elsewhere.html\">",
        );
        let report = judged(dir.path(), tree.path());
        let missing: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.kind == RustdocFindingKind::MissingPage)
            .collect();
        assert_eq!(missing.len(), 1, "{:#?}", report.findings);
        assert!(missing[0].message.contains("redirect stub"));
    }

    #[test]
    fn a_tree_built_from_another_commit_is_stale() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        let other = "f".repeat(40);
        put(
            tree.path(),
            "surface.json",
            &format!(
                r#"{{"schema":"web-surface/v1","id":"rustdoc","mount":"/rustdoc","built_from":"{other}"}}"#
            ),
        );
        let report = judged(dir.path(), tree.path());
        assert_eq!(
            kinds(&report),
            vec![(RustdocFindingKind::Stale, other.clone())]
        );
        assert!(report.findings[0].message.contains(HEAD));
        assert_eq!(report.built_from.as_deref(), Some(other.as_str()));
    }

    #[test]
    fn a_tree_without_a_declaration_is_undeclared_and_not_merely_stale() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        std::fs::remove_file(tree.path().join("surface.json")).unwrap();
        let report = judged(dir.path(), tree.path());
        assert_eq!(
            kinds(&report),
            vec![(RustdocFindingKind::Undeclared, "surface.json".into())]
        );
        assert_eq!(report.built_from, None);
        // a declaration that says nothing about its commit is the same finding
        put(
            tree.path(),
            "surface.json",
            r#"{"schema":"web-surface/v1","id":"rustdoc","mount":"/rustdoc"}"#,
        );
        assert_eq!(
            kinds(&judged(dir.path(), tree.path())),
            vec![(RustdocFindingKind::Undeclared, "surface.json".into())]
        );
    }

    #[test]
    fn an_asset_the_crate_index_loads_is_reported_once_as_an_asset() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        std::fs::remove_file(tree.path().join("static.files/main-1.js")).unwrap();
        std::fs::remove_file(tree.path().join("static.files/Serif-1.woff2")).unwrap();
        std::fs::remove_file(tree.path().join("static.files/search-1.js")).unwrap();
        let report = judged(dir.path(), tree.path());
        assert_eq!(
            kinds(&report),
            vec![
                (RustdocFindingKind::Asset, "static.files/main-1.js".into()),
                (RustdocFindingKind::Asset, "static.files/search-1.js".into()),
                (
                    RustdocFindingKind::Asset,
                    "static.files/Serif-1.woff2".into()
                ),
            ],
            "not again as the link every index page carries to it"
        );
        // the search index's root is an asset too
        let tree = clean_tree(&expected_of(dir.path()));
        std::fs::remove_file(tree.path().join("search.index/root.js")).unwrap();
        assert_eq!(
            kinds(&judged(dir.path(), tree.path())),
            vec![(RustdocFindingKind::Asset, "search.index/root.js".into())]
        );
    }

    #[test]
    fn the_crate_index_must_say_it_is_the_librarys() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        let index = tree.path().join("fixture_crate/index.html");
        let text = std::fs::read_to_string(&index)
            .unwrap()
            .replace("fixture_crate - Rust", "something else");
        std::fs::write(&index, text).unwrap();
        assert_eq!(
            kinds(&judged(dir.path(), tree.path())),
            vec![(RustdocFindingKind::CrateIndex, "fixture_crate".into())]
        );
    }

    #[test]
    fn a_broken_relative_link_in_the_crates_own_text_is_a_link_finding() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        // the shape the real tree had: a doc comment linking a repository file by a path
        // relative to the source file, which climbs out of the tree
        put(
            tree.path(),
            "fixture_crate/shapes/struct.Square.html",
            concat!(
                "<html><body><details class=\"toggle top-doc\"><summary>Expand</summary>",
                "<div class=\"docblock\"><p>See <a href=\"../../../../.ai/repo/adrs/0040-x.md\">ADR 0040</a> ",
                "and <a href=\"dispatcher#setting-the-default-subscriber\">default</a>.</p></div></details>",
                "</body></html>"
            ),
        );
        let report = judged(dir.path(), tree.path());
        assert_eq!(
            kinds(&report),
            vec![
                (
                    RustdocFindingKind::Link,
                    "../../../../.ai/repo/adrs/0040-x.md".into()
                ),
                (
                    RustdocFindingKind::Link,
                    "fixture_crate/shapes/dispatcher".into()
                ),
            ]
        );
        assert_eq!(
            report.findings[0].file,
            "fixture_crate/shapes/struct.Square.html"
        );
    }

    /// A type's page as rustdoc 1.98 renders its implementation sections, around one docblock
    /// under an implementation whose trait header is `header`.
    fn impl_page(list: &str, header: &str, docblock: &str) -> String {
        format!(
            concat!(
                "<html><body><h2 id=\"trait-implementations\">Trait Implementations</h2>",
                "<div id=\"trait-implementations-list\"></div>",
                "<div id=\"{list}\"><details class=\"toggle implementors-toggle\"><summary>",
                "<section id=\"impl-X\" class=\"impl\"><a href=\"#impl-X\" class=\"anchor\">§</a>",
                "<h3 class=\"code-header\">{header}</h3></section></summary>",
                "<div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary>",
                "<section id=\"method.m\" class=\"method trait-impl\"><h4 class=\"code-header\">fn m()</h4>",
                "</section></summary><div class='docblock'>{docblock}</div></details></div></details></div>",
                "<p>after: <a href=\"{after}\">x</a></p></body></html>"
            ),
            list = list,
            header = header,
            docblock = docblock,
            after = "index.html",
        )
    }

    #[test]
    fn a_link_in_documentation_inherited_from_another_crate_is_counted_and_not_a_finding() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        // tracing's blanket implementation, rendered into every type's page: its trait
        // links nothing in this tree, and its text is tracing's
        let inherited = "Attaches the current <a href=\"dispatcher#setting-the-default-subscriber\">default</a> <a href=\"super::Subscriber\"><code>Subscriber</code></a>.";
        put(
            tree.path(),
            "fixture_crate/shapes/struct.Square.html",
            &impl_page(
                "blanket-implementations-list",
                "impl&lt;T&gt; WithSubscriber for T",
                inherited,
            ),
        );
        put(
            tree.path(),
            "fixture_crate/shapes/enum.Colour.html",
            &impl_page(
                "synthetic-implementations-list",
                "impl <a class=\"trait\" href=\"https://doc.rust-lang.org/core/marker/trait.Send.html\">Send</a> for Colour",
                inherited,
            ),
        );
        let report = judged(dir.path(), tree.path());
        assert_eq!(report.findings, vec![], "{:#?}", report.findings);
        let inherited = &report.counts.accepted_links[0];
        assert_eq!(inherited.class, RustdocLinkClass::InheritedDocumentation);
        assert_eq!((inherited.links, inherited.pages), (4, 2));
        assert_eq!(
            inherited.written,
            vec![
                "dispatcher#setting-the-default-subscriber",
                "super::Subscriber"
            ]
        );
    }

    #[test]
    fn the_same_link_under_a_trait_of_this_crate_or_outside_the_foreign_lists_still_fails() {
        let dir = fixture();
        let bad = "<a href=\"dispatcher#setting-the-default-subscriber\">default</a>";
        // a blanket implementation of the crate's own trait: its header links the trait's
        // page in this tree, so the text under it is the crate's
        let tree = clean_tree(&expected_of(dir.path()));
        put(
            tree.path(),
            "fixture_crate/shapes/struct.Square.html",
            &impl_page(
                "blanket-implementations-list",
                "impl&lt;T&gt; <a class=\"trait\" href=\"trait.Shape.html\" title=\"trait fixture_crate::shapes::Shape\">Shape</a> for T",
                bad,
            ),
        );
        assert_eq!(
            kinds(&judged(dir.path(), tree.path())),
            vec![(
                RustdocFindingKind::Link,
                "fixture_crate/shapes/dispatcher".into()
            )]
        );
        // the trait implementations the crate writes itself are its own text as well
        let tree = clean_tree(&expected_of(dir.path()));
        put(
            tree.path(),
            "fixture_crate/shapes/struct.Square.html",
            &impl_page(
                "trait-implementations-list-2",
                "impl Display for Square",
                bad,
            ),
        );
        assert_eq!(
            kinds(&judged(dir.path(), tree.path())),
            vec![(
                RustdocFindingKind::Link,
                "fixture_crate/shapes/dispatcher".into()
            )]
        );
        // and a link after the foreign list has closed is judged as the crate's
        let tree = clean_tree(&expected_of(dir.path()));
        put(
            tree.path(),
            "fixture_crate/shapes/struct.Square.html",
            &impl_page(
                "blanket-implementations-list",
                "impl&lt;T&gt; Any for T",
                "fine",
            )
            .replace(
                "<p>after: <a href=\"index.html\">",
                &format!("<p>after: {bad}<a href=\"index.html\">"),
            ),
        );
        assert_eq!(
            kinds(&judged(dir.path(), tree.path())),
            vec![(
                RustdocFindingKind::Link,
                "fixture_crate/shapes/dispatcher".into()
            )]
        );
    }

    #[test]
    fn a_trait_pages_own_unwritten_implementors_script_is_counted_and_any_other_script_fails() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        put(
            tree.path(),
            "fixture_crate/shapes/trait.Shape.html",
            "<html><head><script src=\"../../trait.impl/fixture_crate/shapes/trait.Shape.js\" async></script></head></html>",
        );
        let report = judged(dir.path(), tree.path());
        assert_eq!(report.findings, vec![], "{:#?}", report.findings);
        let unwritten = &report.counts.accepted_links[1];
        assert_eq!(unwritten.class, RustdocLinkClass::UnwrittenImplementors);
        assert_eq!((unwritten.links, unwritten.pages), (1, 1));

        // another trait's implementors, or a script a struct page loads, is a finding
        put(
            tree.path(),
            "fixture_crate/shapes/trait.Shape.html",
            "<html><head><script src=\"../../trait.impl/fixture_crate/shapes/trait.Other.js\" async></script></head></html>",
        );
        put(
            tree.path(),
            "fixture_crate/shapes/struct.Square.html",
            "<html><head><script src=\"../../trait.impl/fixture_crate/shapes/trait.Square.js\" async></script></head></html>",
        );
        assert_eq!(
            kinds(&judged(dir.path(), tree.path())),
            // ordered by the page that carries each: struct.Square.html, then trait.Shape.html
            vec![
                (
                    RustdocFindingKind::Link,
                    "trait.impl/fixture_crate/shapes/trait.Square.js".into()
                ),
                (
                    RustdocFindingKind::Link,
                    "trait.impl/fixture_crate/shapes/trait.Other.js".into()
                ),
            ]
        );
    }

    #[test]
    fn a_source_line_range_and_a_directory_link_resolve() {
        let present: HashSet<&str> = ["src/k/lib.rs.html", "k/index.html", "k/m/index.html"]
            .into_iter()
            .collect();
        let dirs: HashSet<&str> = ["src", "src/k", "k", "k/m"].into_iter().collect();
        let r = |from: &str, link: &str| resolve(from, link, &present, &dirs);
        assert_eq!(
            r("k/index.html", "../src/k/lib.rs.html#1-5"),
            Resolution::Internal
        );
        assert_eq!(
            r("k/index.html", "../src/k/lib.rs.html#10"),
            Resolution::Internal
        );
        assert_eq!(r("k/index.html", "m/"), Resolution::Internal);
        assert_eq!(
            r("k/index.html", "m/index.html?search=x"),
            Resolution::Internal
        );
        assert_eq!(r("k/index.html", "#anchor"), Resolution::Ignored);
        assert_eq!(
            r("k/index.html", "mailto:x@example.org"),
            Resolution::Ignored
        );
        assert_eq!(
            r("k/index.html", "https://doc.rust-lang.org/std/"),
            Resolution::External("doc.rust-lang.org".into())
        );
        assert_eq!(
            r("k/index.html", "/k/index.html"),
            Resolution::Broken("/k/index.html".into())
        );
        assert_eq!(
            r("k/index.html", "../../up.html"),
            Resolution::Broken("../../up.html".into())
        );
        assert_eq!(
            r("k/index.html", "super::Subscriber"),
            Resolution::Broken("k/super::Subscriber".into())
        );
        assert_eq!(r("k/index.html", "m%2Findex.html"), Resolution::Internal);
    }

    #[test]
    fn a_machine_path_or_a_credential_in_any_file_is_a_finding() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        // assembled at run time: this file is a page of the tree it describes
        let home = format!("{}someone/dev/repo", "/Users/");
        put(
            tree.path(),
            "src/fixture_crate/lib.rs.html",
            &format!("<html>{home}</html>"),
        );
        let value = format!("{}{}", "abcdef", "0123456789");
        put(
            tree.path(),
            "search.index/data.js",
            &format!("x\napi_key{}{value}\n", ": "),
        );
        // and a constant's name assigned to a field called a token is code
        put(
            tree.path(),
            "static.files/code.js",
            &format!(
                "bytes_per_token{}{}",
                ": ",
                ["BYTES", "PER", "TOKEN"].join("_")
            ),
        );
        let report = judged(dir.path(), tree.path());
        assert_eq!(
            kinds(&report),
            vec![
                (RustdocFindingKind::MachinePath, "macos-home".into()),
                (RustdocFindingKind::Secret, "assignment".into()),
            ]
        );
        assert!(
            !report.findings[1].message.contains(&value),
            "the value is never repeated"
        );
        assert!(report.findings[1].message.contains("line 2"));
    }

    #[test]
    fn a_page_naming_the_repositorys_own_root_is_a_machine_path() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        let root = tempfile::tempdir().unwrap();
        let rooted = root.path().join("checkout");
        put(
            tree.path(),
            "fixture_crate/constant.LIMIT.html",
            &format!("<html>{}</html>", rooted.join("src/lib.rs").display()),
        );
        let report = judge(&Subject {
            root: &rooted,
            crate_dir: Some(dir.path()),
            tree: Tree::At {
                dir: tree.path().to_path_buf(),
                shown: "t".into(),
                mount: None,
            },
            head: Some(HEAD),
        })
        .unwrap();
        assert!(
            kinds(&report).contains(&(RustdocFindingKind::MachinePath, "repository-root".into())),
            "{:#?}",
            report.findings
        );
    }

    #[test]
    fn a_check_that_cannot_see_its_subject_says_so_and_never_reports_clean() {
        let dir = fixture();
        let absent = tempfile::tempdir().unwrap();
        let report = judge(&Subject {
            root: Path::new("/r"),
            crate_dir: Some(dir.path()),
            tree: Tree::At {
                dir: absent.path().join("never-built"),
                shown: "target/web/rustdoc".into(),
                mount: Some("/rustdoc".into()),
            },
            head: Some(HEAD),
        })
        .unwrap();
        assert_eq!(report.verdict, RustdocVerdict::NoTree);
        assert_eq!(report.exit_code(), 12);
        assert!(report.reason.as_deref().unwrap().contains(PRODUCER));
        // the module routes are the crate's, so they are answered without a tree
        assert!(!report.modules.is_empty());

        let report = judge(&Subject {
            root: Path::new("/r"),
            crate_dir: Some(dir.path()),
            tree: Tree::Absent {
                shown: "target/web/rustdoc".into(),
                reason: "the topology declares no such surface".into(),
            },
            head: None,
        })
        .unwrap();
        assert_eq!(report.exit_code(), 12);

        let report = judge(&Subject {
            root: Path::new("/r"),
            crate_dir: None,
            tree: Tree::Absent {
                shown: "target/web/rustdoc".into(),
                reason: "x".into(),
            },
            head: None,
        })
        .unwrap();
        assert_eq!(report.exit_code(), 12);
        assert!(report.reason.unwrap().contains("no Rust crate"));
    }

    #[test]
    fn a_filter_narrows_the_findings_and_never_the_verdict() {
        let dir = fixture();
        let tree = clean_tree(&expected_of(dir.path()));
        std::fs::remove_file(tree.path().join("surface.json")).unwrap();
        put(tree.path(), "stray.html", "<html></html>");
        let report = judged(dir.path(), tree.path());
        let narrowed = report
            .clone()
            .filtered(Some(RustdocFindingKind::Link), false);
        assert!(narrowed.findings.is_empty());
        assert_eq!(narrowed.verdict, RustdocVerdict::Findings);
        assert_eq!(narrowed.exit_code(), 10);
        let summary = report.filtered(None, true);
        assert!(summary.findings.is_empty());
        assert_eq!(summary.exit_code(), 10);
    }

    #[test]
    fn every_kind_and_class_has_its_code_and_a_remedy() {
        let codes: BTreeSet<&str> = RustdocFindingKind::all()
            .iter()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(codes.len(), RustdocFindingKind::all().len());
        for kind in RustdocFindingKind::all() {
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::Value::String(kind.as_str().into())
            );
            assert!(!kind.remedy().is_empty());
        }
        for class in RustdocLinkClass::all() {
            assert_eq!(
                serde_json::to_value(class).unwrap(),
                serde_json::Value::String(class.as_str().into())
            );
        }
    }

    #[test]
    fn the_page_reader_skips_script_bodies_comments_and_prose() {
        let page = Page::read(
            concat!(
                "<!-- <a href=\"commented.html\"> -->",
                "<script>document.write(`<a href=\"../static.files/${f}\">`)</script>",
                "<p>href=\"prose.html\"</p><a href='quoted.html'>q</a><img src=bare.png>",
                "<meta http-equiv=\"Refresh\" content=\"0;URL=x.html\">"
            )
            .as_bytes(),
        );
        let values: Vec<&str> = page.links.iter().map(|l| l.value.as_str()).collect();
        assert_eq!(values, vec!["quoted.html", "bare.png"]);
        assert!(page.redirect);
    }

    #[test]
    fn percent_escapes_are_decoded_and_a_broken_one_is_kept() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
        assert_eq!(percent_decode("%41%"), "A%");
    }
}
