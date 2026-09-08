//! The crate read as a syntax tree: which items this crate actually exports, where each
//! one is declared, what its documentation says, and which of its doc examples run.
//!
//! # Why a parser and not a search
//!
//! Rust is not line-oriented text and the question this subsystem asks cannot be answered
//! by matching lines. `pub fn` in a private module exports nothing; `pub mod` under a
//! `pub(crate) mod` exports nothing; a doc comment is attached to the item that follows
//! it, an inner doc comment to the item that contains it, and a fenced block inside a doc
//! comment is executed by `cargo test --doc` only when it is a Rust block without
//! `ignore`. Every one of those is a syntactic fact, so the inventory is built from
//! [`syn`]'s syntax tree of every file the module graph reaches, walked from the crate
//! root exactly as `rustc` walks it.
//!
//! # What it produces
//!
//! [`Inventory::of_crate`] returns the crate's [effectively public](Item::exported)
//! surface: one [`Item`] per module, type, function, trait, macro, constant and
//! associated item, each carrying its crate path, its declaration site, its documentation
//! and its [`Example`]s. It also records, per module, the in-file `#[cfg(test)]` tests
//! that stand as behavioural evidence beside a declaration, and the module paths named by
//! the crate's integration tests, which is how a module's behavioural coverage is found
//! without any list being kept by hand.
//!
//! Nothing here judges: the policy that decides which items owe an example lives in
//! [`super::policy`], and the codes a finding carries are [`super::ViolationCode`]. This
//! module answers only *what is there*.
//!
//! # Determinism
//!
//! The walk is a depth-first traversal of the module graph in declaration order, and file
//! reads are the only I/O. Two runs over the same tree produce equal inventories, which is
//! what lets the report be compared, cached and committed.
//!
//! ```
//! use majordomus_cli::quality::source::Inventory;
//!
//! // A crate root that declares one exported module with one exported function in it.
//! let dir = tempfile::tempdir().unwrap();
//! let src = dir.path().join("src");
//! std::fs::create_dir_all(&src).unwrap();
//! std::fs::write(src.join("lib.rs"), "//! Root.\npub mod thing;\n").unwrap();
//! std::fs::write(src.join("thing.rs"), "//! A thing.\n/// Answers.\npub fn go() {}\n").unwrap();
//!
//! let inventory = Inventory::of_crate(dir.path()).unwrap();
//! let go = inventory.item("majordomus_cli::thing::go").unwrap();
//! assert!(go.exported);
//! assert_eq!(go.file, "src/thing.rs");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// The crate name every path in an inventory starts with. The library target's name, which
/// is what a doc example writes in its `use`.
pub const CRATE: &str = "majordomus_cli";

/// What an item is, in the vocabulary rustdoc and the reference use. The kind decides what
/// the policy may ask of an item, so it is recorded rather than inferred later.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// A module: `mod`, inline or in its own file.
    Module,
    /// A `struct`.
    Struct,
    /// An `enum`.
    Enum,
    /// A `union`.
    Union,
    /// A `trait`.
    Trait,
    /// A free function, or an associated function with no receiver.
    Function,
    /// A method: an associated function taking a receiver.
    Method,
    /// A `const`.
    Constant,
    /// A `static`.
    Static,
    /// A `type` alias.
    TypeAlias,
    /// A `macro_rules!` carrying `#[macro_export]`.
    Macro,
    /// A field of a public struct.
    Field,
    /// A variant of a public enum.
    Variant,
    /// An associated type or constant of a trait.
    Associated,
}

impl ItemKind {
    /// The word a finding uses for an item of this kind.
    ///
    /// ```
    /// use majordomus_cli::quality::source::ItemKind;
    /// assert_eq!(ItemKind::Method.noun(), "method");
    /// assert_eq!(ItemKind::TypeAlias.noun(), "type alias");
    /// ```
    pub fn noun(self) -> &'static str {
        match self {
            ItemKind::Module => "module",
            ItemKind::Struct => "struct",
            ItemKind::Enum => "enum",
            ItemKind::Union => "union",
            ItemKind::Trait => "trait",
            ItemKind::Function => "function",
            ItemKind::Method => "method",
            ItemKind::Constant => "constant",
            ItemKind::Static => "static",
            ItemKind::TypeAlias => "type alias",
            ItemKind::Macro => "macro",
            ItemKind::Field => "field",
            ItemKind::Variant => "variant",
            ItemKind::Associated => "associated item",
        }
    }

    /// Does an item of this kind carry behaviour a reader could be shown running?
    ///
    /// A module, a type, a function, a method and an exported macro do. A field, a
    /// variant, a constant, a static, a type alias and an associated item are values and
    /// names: an example of one is an example of whatever uses it, and demanding one buys
    /// the placeholder this subsystem exists to refuse.
    ///
    /// ```
    /// use majordomus_cli::quality::source::ItemKind;
    /// assert!(ItemKind::Function.carries_behaviour());
    /// assert!(!ItemKind::Constant.carries_behaviour());
    /// ```
    pub fn carries_behaviour(self) -> bool {
        matches!(
            self,
            ItemKind::Module
                | ItemKind::Struct
                | ItemKind::Enum
                | ItemKind::Union
                | ItemKind::Trait
                | ItemKind::Function
                | ItemKind::Method
                | ItemKind::Macro
        )
    }
}

/// One fenced block of a doc comment, classified by what `cargo test --doc` does with it.
///
/// The distinction matters and is the whole reason the block is modelled rather than
/// counted: a ```` ```text ```` block is prose in a box, a ```` ```ignore ```` block is a
/// example that is compiled by nobody, and only a Rust block without `ignore` is a test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Example {
    /// The info string of the fence, as written (`""`, `"no_run"`, `"ignore"`, `"text"`).
    pub info: String,
    /// The lines of the block, hidden `#`-prefixed lines included, as written.
    pub lines: Vec<String>,
    /// Compiled and run by `cargo test --doc`.
    pub runs: bool,
    /// Compiled but not run: `no_run`, and `compile_fail` which must fail to compile.
    pub compiles: bool,
}

impl Example {
    /// Is this block executable evidence — compiled by the toolchain, either run or
    /// deliberately not run?
    ///
    /// ```
    /// use majordomus_cli::quality::source::Example;
    /// let runs = Example::parse("", &["assert_eq!(1 + 1, 2);".into()]);
    /// assert!(runs.executable());
    /// let ignored = Example::parse("ignore", &["nothing();".into()]);
    /// assert!(!ignored.executable(), "an ignored block is compiled by nobody");
    /// ```
    pub fn executable(&self) -> bool {
        self.runs || self.compiles
    }

    /// Classify a fenced block from its info string and its lines.
    ///
    /// The rules are rustdoc's: a block whose info string is empty, or names only Rust
    /// attributes, is Rust; `ignore` is neither compiled nor run; `no_run` and
    /// `compile_fail` are compiled; anything else (`text`, `json`, `console`) is prose.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Example;
    /// assert!(Example::parse("", &["let x = 1;".into()]).runs);
    /// assert!(!Example::parse("text", &["a diagram".into()]).executable());
    /// assert!(Example::parse("no_run", &["serve();".into()]).compiles);
    /// assert!(!Example::parse("no_run", &["serve();".into()]).runs);
    /// ```
    pub fn parse(info: &str, lines: &[String]) -> Self {
        let tokens: Vec<&str> = info
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t| !t.is_empty())
            .collect();
        // rustdoc's own set; anything outside it names a language and makes the block prose
        const ATTRS: [&str; 8] = [
            "rust",
            "ignore",
            "should_panic",
            "no_run",
            "compile_fail",
            "edition2015",
            "edition2018",
            "edition2021",
        ];
        let is_rust = tokens
            .iter()
            .all(|t| ATTRS.contains(t) || t.starts_with("edition") || t.starts_with("ignore-"));
        let ignored = tokens
            .iter()
            .any(|t| *t == "ignore" || t.starts_with("ignore-"));
        let no_run = tokens
            .iter()
            .any(|t| *t == "no_run" || *t == "compile_fail");
        Example {
            info: info.to_string(),
            lines: lines.to_vec(),
            runs: is_rust && !ignored && !no_run,
            compiles: is_rust && !ignored,
        }
    }

    /// The lines a reader sees: rustdoc hides a line whose first non-space character is
    /// `#` followed by a space or nothing.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Example;
    /// let e = Example::parse("", &["# use std::fmt;".into(), "let x = 1;".into()]);
    /// assert_eq!(e.visible_lines(), vec!["let x = 1;".to_string()]);
    /// ```
    pub fn visible_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter(|l| {
                let t = l.trim_start();
                !(t == "#" || t.starts_with("# ") || t.starts_with("#\t"))
            })
            .cloned()
            .collect()
    }

    /// The block's code with the hidden marker stripped: what the toolchain compiles.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Example;
    /// let e = Example::parse("", &["# let a = 1;".into(), "assert_eq!(a, 1);".into()]);
    /// assert_eq!(e.code(), "let a = 1;\nassert_eq!(a, 1);");
    /// ```
    pub fn code(&self) -> String {
        self.lines
            .iter()
            .map(|l| {
                let t = l.trim_start();
                if t == "#" {
                    ""
                } else if let Some(rest) = t.strip_prefix("# ") {
                    rest
                } else if let Some(rest) = t.strip_prefix("#\t") {
                    rest
                } else {
                    l.as_str()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One item of the crate's surface, as declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Item {
    /// The full path a doc example would write: `majordomus_cli::capability::model::Capability`.
    pub path: String,
    /// The last segment: `Capability`.
    pub name: String,
    /// What it is.
    pub kind: ItemKind,
    /// Reachable from outside the crate: declared `pub`, in a module chain that is `pub`
    /// all the way to the crate root, or re-exported by one. This is what `missing_docs`
    /// calls public and what a consumer can actually name.
    pub exported: bool,
    /// Declared `pub` on the item itself, whatever the modules above it are. A `pub use`
    /// of a type carries its `pub` members out of a private module and leaves the private
    /// ones behind, so the two facts have to be kept apart.
    pub declared_pub: bool,
    /// The file it is declared in, repository-relative to the crate directory.
    pub file: String,
    /// The line of the declaration, 1-based.
    pub line: usize,
    /// The documentation attached to it, doc-comment markers stripped, fences included.
    pub doc: String,
    /// The fenced blocks of that documentation, in order.
    pub examples: Vec<Example>,
    /// The path of the item that owns this one: the module of a free item, the type of a
    /// method, the enum of a variant.
    pub owner: String,
    /// A method or function whose body is one expression that only hands back what the
    /// receiver already holds. Recorded because an example of one shows nothing that the
    /// signature does not; see [`ItemKind::carries_behaviour`].
    pub trivial_accessor: bool,
    /// The `#[deprecated]` note, when the item carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
}

impl Item {
    /// The words of the documentation, fenced blocks and the markers around them removed:
    /// the prose a reader is actually given.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Inventory;
    /// let dir = tempfile::tempdir().unwrap();
    /// let src = dir.path().join("src");
    /// std::fs::create_dir_all(&src).unwrap();
    /// std::fs::write(src.join("lib.rs"),
    ///     "//! Root.\n/// Answers, and says with what.\n///\n/// ```\n/// let x = 1;\n/// ```\npub fn go() {}\n").unwrap();
    /// let item = Inventory::of_crate(dir.path()).unwrap().item("majordomus_cli::go").unwrap().clone();
    /// assert_eq!(item.prose_words(), 5, "the fenced block is not prose");
    /// ```
    pub fn prose_words(&self) -> usize {
        let mut words = 0usize;
        let mut fenced = false;
        for line in self.doc.lines() {
            let t = line.trim();
            if t.starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if !fenced {
                words += t.split_whitespace().count();
            }
        }
        words
    }

    /// The examples of this item that the toolchain compiles.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Inventory;
    /// let dir = tempfile::tempdir().unwrap();
    /// let src = dir.path().join("src");
    /// std::fs::create_dir_all(&src).unwrap();
    /// std::fs::write(src.join("lib.rs"),
    ///     "//! Root.\n/// Answers.\n///\n/// ```ignore\n/// go();\n/// ```\npub fn go() {}\n").unwrap();
    /// let item = Inventory::of_crate(dir.path()).unwrap().item("majordomus_cli::go").unwrap().clone();
    /// assert_eq!(item.examples.len(), 1);
    /// assert!(item.executable_examples().is_empty(), "an ignored block is not evidence");
    /// ```
    pub fn executable_examples(&self) -> Vec<&Example> {
        self.examples.iter().filter(|e| e.executable()).collect()
    }
}

/// The in-file evidence a module carries: the `#[cfg(test)]` tests declared beside the
/// declarations they are about.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InFileTests {
    /// The names of the `#[test]` functions declared in the module's own file.
    pub names: Vec<String>,
}

/// Everything the walk found: the items, the in-file tests per module, and what the
/// crate's integration tests name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Inventory {
    /// Every item found, exported or not, in declaration order of the walk.
    pub items: Vec<Item>,
    /// Module path to the in-file tests declared in that module's file.
    pub in_file_tests: BTreeMap<String, InFileTests>,
    /// Module paths named by the crate's integration tests and benches: every
    /// `majordomus_cli::a::b` path those files mention, expanded to every prefix.
    pub named_by_tests: BTreeSet<String>,
    /// The test files that were read, crate-relative, so a report can say what it looked at.
    pub test_files: Vec<String>,
    /// The crate paths a `pub use` in an exported module makes reachable, resolved to the
    /// item they name. An item in a private module that a public module re-exports is part
    /// of the crate's surface, which is what `missing_docs` also thinks, so the inventory
    /// has to think it too.
    pub reexported: BTreeSet<String>,
    /// The name a re-export offers an item under, mapped to the item it names:
    /// `majordomus_cli::capability::builtin::GetInput` to
    /// `majordomus_cli::capability::builtin::objects::GetInput`.
    ///
    /// A consumer writes the alias, so a test that exercises a module through one would
    /// otherwise look like a test that names no module at all. This is what carries the
    /// credit back to the module that declares the item.
    pub reexport_aliases: BTreeMap<String, String>,
}

impl Inventory {
    /// Walk a crate directory: `src/lib.rs` and every file its module graph reaches, then
    /// `tests/` and `benches/` for the module paths they name.
    ///
    /// The directory is the crate's, the one holding `Cargo.toml`. A crate whose root is
    /// missing is an error rather than an empty inventory: a quality report over nothing
    /// is the one result that must never be mistaken for a clean one.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Inventory;
    /// let dir = tempfile::tempdir().unwrap();
    /// assert!(Inventory::of_crate(dir.path()).is_err(), "no crate root, no inventory");
    /// ```
    pub fn of_crate(crate_dir: &Path) -> Result<Self, Error> {
        let root = crate_dir.join("src/lib.rs");
        if !root.is_file() {
            return Err(Error::InvalidSource {
                path: "src/lib.rs".into(),
                reason: format!(
                    "not a file under {}; the quality inventory needs the crate directory, the one holding Cargo.toml",
                    crate_dir.display()
                ),
            });
        }
        let mut inv = Inventory::default();
        let mut walker = Walker {
            crate_dir: crate_dir.to_path_buf(),
            inv: &mut inv,
        };
        walker.file(&root, CRATE, true)?;
        apply_reexports(&mut inv);
        collect_test_references(crate_dir, &mut inv)?;
        Ok(inv)
    }

    /// One item by its full path.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Inventory;
    /// let dir = tempfile::tempdir().unwrap();
    /// let src = dir.path().join("src");
    /// std::fs::create_dir_all(&src).unwrap();
    /// std::fs::write(src.join("lib.rs"), "//! Root.\n/// A type.\npub struct T;\n").unwrap();
    /// let inv = Inventory::of_crate(dir.path()).unwrap();
    /// assert!(inv.item("majordomus_cli::T").is_some());
    /// assert!(inv.item("majordomus_cli::Nope").is_none());
    /// ```
    pub fn item(&self, path: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.path == path)
    }

    /// One item by its path and its kind.
    ///
    /// A struct's field and one of its methods can share a path — `T::n` names both — because
    /// Rust keeps them in different namespaces and a path does not say which. Every caller
    /// that cares says so.
    ///
    /// ```
    /// use majordomus_cli::quality::source::{Inventory, ItemKind};
    /// let dir = tempfile::tempdir().unwrap();
    /// let src = dir.path().join("src");
    /// std::fs::create_dir_all(&src).unwrap();
    /// std::fs::write(src.join("lib.rs"), concat!(
    ///     "//! Root.\n",
    ///     "/// A counter.\n",
    ///     "pub struct T { n: usize }\n",
    ///     "impl T { /// The count.\n pub fn n(&self) -> usize { self.n } }\n",
    /// )).unwrap();
    /// let inv = Inventory::of_crate(dir.path()).unwrap();
    /// let method = inv.item_of("majordomus_cli::T::n", ItemKind::Method).unwrap();
    /// assert!(method.trivial_accessor);
    /// assert!(inv.item_of("majordomus_cli::T::n", ItemKind::Field).is_some());
    /// ```
    pub fn item_of(&self, path: &str, kind: ItemKind) -> Option<&Item> {
        self.items.iter().find(|i| i.path == path && i.kind == kind)
    }

    /// Every exported item, in walk order.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Inventory;
    /// let dir = tempfile::tempdir().unwrap();
    /// let src = dir.path().join("src");
    /// std::fs::create_dir_all(&src).unwrap();
    /// std::fs::write(src.join("lib.rs"), "//! Root.\nmod hidden;\n").unwrap();
    /// std::fs::write(src.join("hidden.rs"), "/// Not reachable.\npub fn inner() {}\n").unwrap();
    /// let inv = Inventory::of_crate(dir.path()).unwrap();
    /// assert!(inv.item("majordomus_cli::hidden::inner").unwrap().exported == false);
    /// assert!(inv.exported().iter().all(|i| i.path != "majordomus_cli::hidden::inner"));
    /// ```
    pub fn exported(&self) -> Vec<&Item> {
        self.items.iter().filter(|i| i.exported).collect()
    }

    /// Does any test of this crate exercise the module at `path`?
    ///
    /// A test counts when it names the module, names something under it, or names an item
    /// through the alias a `pub use` offers it under — which is how these tests are actually
    /// written: `use majordomus_cli::capability::builtin::GetInput` exercises
    /// `capability::builtin::objects`, and nothing in the text says so.
    ///
    /// ```
    /// use majordomus_cli::quality::source::Inventory;
    /// let dir = tempfile::tempdir().unwrap();
    /// let src = dir.path().join("src");
    /// std::fs::create_dir_all(src.join("outer")).unwrap();
    /// std::fs::write(src.join("lib.rs"), "//! Root.\npub mod outer;\n").unwrap();
    /// std::fs::write(src.join("outer/mod.rs"),
    ///     "//! Outer.\npub mod inner;\npub use inner::Thing;\n").unwrap();
    /// std::fs::write(src.join("outer/inner.rs"), "//! Inner.\n/// A thing.\npub struct Thing;\n").unwrap();
    /// std::fs::create_dir_all(dir.path().join("tests")).unwrap();
    /// std::fs::write(dir.path().join("tests/it.rs"),
    ///     "use majordomus_cli::outer::Thing;\n#[test]\nfn t() { let _ = Thing; }\n").unwrap();
    ///
    /// let inv = Inventory::of_crate(dir.path()).unwrap();
    /// assert!(inv.exercised_by_a_test("majordomus_cli::outer::inner"),
    ///         "the test reaches inner through the alias outer::Thing");
    /// ```
    pub fn exercised_by_a_test(&self, path: &str) -> bool {
        if self
            .in_file_tests
            .get(path)
            .is_some_and(|t| !t.names.is_empty())
        {
            return true;
        }
        if self.named_by_tests.contains(path) {
            return true;
        }
        let under = format!("{path}::");
        self.reexport_aliases.iter().any(|(alias, target)| {
            target.starts_with(&under) && self.named_by_tests.contains(alias)
        })
    }

    /// Every exported module, in walk order. The crate root is one of them.
    pub fn exported_modules(&self) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|i| i.exported && i.kind == ItemKind::Module)
            .collect()
    }
}

struct Walker<'a> {
    crate_dir: PathBuf,
    inv: &'a mut Inventory,
}

impl Walker<'_> {
    /// Parse one file and walk the items in it as the module `mod_path`.
    fn file(&mut self, path: &Path, mod_path: &str, exported: bool) -> Result<(), Error> {
        let rel = self.relative(path);
        let text = std::fs::read_to_string(path).map_err(|e| Error::InvalidSource {
            path: rel.clone(),
            reason: e.to_string(),
        })?;
        let ast = syn::parse_file(&text).map_err(|e| Error::InvalidSource {
            path: rel.clone(),
            reason: format!("does not parse: {e}"),
        })?;
        // the module itself, from the file's inner attributes
        let doc = doc_of(&ast.attrs);
        let owner = mod_path
            .rsplit_once("::")
            .map(|(o, _)| o.to_string())
            .unwrap_or_default();
        self.inv.items.push(Item {
            path: mod_path.to_string(),
            name: mod_path.rsplit("::").next().unwrap_or(mod_path).to_string(),
            kind: ItemKind::Module,
            exported,
            declared_pub: true,
            file: rel.clone(),
            line: 1,
            doc: doc.clone(),
            examples: examples_of(&doc),
            owner,
            trivial_accessor: false,
            deprecated: deprecation(&ast.attrs),
        });
        self.items(&ast.items, mod_path, exported, path, &rel)
    }

    /// Walk a list of items belonging to `mod_path`, declared in `file`.
    fn items(
        &mut self,
        items: &[syn::Item],
        mod_path: &str,
        exported: bool,
        file: &Path,
        rel: &str,
    ) -> Result<(), Error> {
        for item in items {
            match item {
                syn::Item::Mod(m) => {
                    let name = m.ident.to_string();
                    let child_path = format!("{mod_path}::{name}");
                    let child_exported = exported && is_pub(&m.vis);
                    if is_test_only(&m.attrs) {
                        // evidence, not surface: the tests declared beside the declarations
                        if let Some((_, inner)) = &m.content {
                            let names = test_fn_names(inner);
                            if !names.is_empty() {
                                self.inv
                                    .in_file_tests
                                    .entry(mod_path.to_string())
                                    .or_default()
                                    .names
                                    .extend(names);
                            }
                        }
                        continue;
                    }
                    match &m.content {
                        Some((_, inner)) => {
                            let doc = doc_of(&m.attrs);
                            self.inv.items.push(Item {
                                path: child_path.clone(),
                                name,
                                kind: ItemKind::Module,
                                exported: child_exported,
                                declared_pub: is_pub(&m.vis),
                                file: rel.to_string(),
                                line: line_of(m.mod_token.span),
                                doc: doc.clone(),
                                examples: examples_of(&doc),
                                owner: mod_path.to_string(),
                                trivial_accessor: false,
                                deprecated: deprecation(&m.attrs),
                            });
                            self.items(inner, &child_path, child_exported, file, rel)?;
                        }
                        None => {
                            let child = self.resolve(file, &m.ident.to_string())?;
                            self.file(&child, &child_path, child_exported)?;
                        }
                    }
                }
                other => self.item(other, mod_path, exported, rel),
            }
        }
        Ok(())
    }

    /// One non-module item.
    fn item(&mut self, item: &syn::Item, mod_path: &str, exported: bool, rel: &str) {
        let push = |name: String,
                    kind: ItemKind,
                    vis_pub: bool,
                    attrs: &[syn::Attribute],
                    line: usize,
                    inv: &mut Inventory| {
            let doc = doc_of(attrs);
            inv.items.push(Item {
                path: format!("{mod_path}::{name}"),
                name,
                kind,
                exported: exported && vis_pub,
                declared_pub: vis_pub,
                file: rel.to_string(),
                line,
                examples: examples_of(&doc),
                doc,
                owner: mod_path.to_string(),
                trivial_accessor: false,
                deprecated: deprecation(attrs),
            });
        };
        match item {
            syn::Item::Struct(s) => {
                push(
                    s.ident.to_string(),
                    ItemKind::Struct,
                    is_pub(&s.vis),
                    &s.attrs,
                    line_of(s.struct_token.span),
                    self.inv,
                );
                let owner = format!("{mod_path}::{}", s.ident);
                let owner_pub = exported && is_pub(&s.vis);
                for f in s.fields.iter() {
                    let Some(id) = &f.ident else { continue };
                    let doc = doc_of(&f.attrs);
                    self.inv.items.push(Item {
                        path: format!("{owner}::{id}"),
                        name: id.to_string(),
                        kind: ItemKind::Field,
                        exported: owner_pub && is_pub(&f.vis),
                        declared_pub: is_pub(&f.vis),
                        file: rel.to_string(),
                        line: line_of(id.span()),
                        examples: examples_of(&doc),
                        doc,
                        owner: owner.clone(),
                        trivial_accessor: false,
                        deprecated: deprecation(&f.attrs),
                    });
                }
            }
            syn::Item::Enum(e) => {
                push(
                    e.ident.to_string(),
                    ItemKind::Enum,
                    is_pub(&e.vis),
                    &e.attrs,
                    line_of(e.enum_token.span),
                    self.inv,
                );
                let owner = format!("{mod_path}::{}", e.ident);
                let owner_pub = exported && is_pub(&e.vis);
                for v in &e.variants {
                    let doc = doc_of(&v.attrs);
                    self.inv.items.push(Item {
                        path: format!("{owner}::{}", v.ident),
                        name: v.ident.to_string(),
                        kind: ItemKind::Variant,
                        exported: owner_pub,
                        declared_pub: true,
                        file: rel.to_string(),
                        line: line_of(v.ident.span()),
                        examples: examples_of(&doc),
                        doc,
                        owner: owner.clone(),
                        trivial_accessor: false,
                        deprecated: deprecation(&v.attrs),
                    });
                }
            }
            syn::Item::Union(u) => push(
                u.ident.to_string(),
                ItemKind::Union,
                is_pub(&u.vis),
                &u.attrs,
                line_of(u.union_token.span),
                self.inv,
            ),
            syn::Item::Trait(t) => {
                push(
                    t.ident.to_string(),
                    ItemKind::Trait,
                    is_pub(&t.vis),
                    &t.attrs,
                    line_of(t.trait_token.span),
                    self.inv,
                );
                let owner = format!("{mod_path}::{}", t.ident);
                let owner_pub = exported && is_pub(&t.vis);
                for i in &t.items {
                    let (name, kind, attrs, line) = match i {
                        syn::TraitItem::Fn(f) => (
                            f.sig.ident.to_string(),
                            receiver_kind(&f.sig),
                            &f.attrs,
                            line_of(f.sig.ident.span()),
                        ),
                        syn::TraitItem::Const(c) => (
                            c.ident.to_string(),
                            ItemKind::Associated,
                            &c.attrs,
                            line_of(c.ident.span()),
                        ),
                        syn::TraitItem::Type(ty) => (
                            ty.ident.to_string(),
                            ItemKind::Associated,
                            &ty.attrs,
                            line_of(ty.ident.span()),
                        ),
                        _ => continue,
                    };
                    let doc = doc_of(attrs);
                    self.inv.items.push(Item {
                        path: format!("{owner}::{name}"),
                        name,
                        kind,
                        exported: owner_pub,
                        // a trait's items are as public as the trait
                        declared_pub: true,
                        file: rel.to_string(),
                        line,
                        examples: examples_of(&doc),
                        doc,
                        owner: owner.clone(),
                        trivial_accessor: false,
                        deprecated: deprecation(attrs),
                    });
                }
            }
            syn::Item::Fn(f) => {
                let doc = doc_of(&f.attrs);
                self.inv.items.push(Item {
                    path: format!("{mod_path}::{}", f.sig.ident),
                    name: f.sig.ident.to_string(),
                    kind: ItemKind::Function,
                    exported: exported && is_pub(&f.vis),
                    declared_pub: is_pub(&f.vis),
                    file: rel.to_string(),
                    line: line_of(f.sig.ident.span()),
                    examples: examples_of(&doc),
                    doc,
                    owner: mod_path.to_string(),
                    trivial_accessor: false,
                    deprecated: deprecation(&f.attrs),
                });
            }
            syn::Item::Const(c) => push(
                c.ident.to_string(),
                ItemKind::Constant,
                is_pub(&c.vis),
                &c.attrs,
                line_of(c.ident.span()),
                self.inv,
            ),
            syn::Item::Static(s) => push(
                s.ident.to_string(),
                ItemKind::Static,
                is_pub(&s.vis),
                &s.attrs,
                line_of(s.ident.span()),
                self.inv,
            ),
            syn::Item::Type(t) => push(
                t.ident.to_string(),
                ItemKind::TypeAlias,
                is_pub(&t.vis),
                &t.attrs,
                line_of(t.ident.span()),
                self.inv,
            ),
            syn::Item::Macro(m) => {
                // `macro_rules!` with `#[macro_export]` is exported from the crate root,
                // whatever module it is written in — which is exactly how the crate's
                // `capability!` and `module!` are reached.
                let exported_macro = m.attrs.iter().any(|a| a.path().is_ident("macro_export"));
                let Some(id) = &m.ident else { return };
                if !exported_macro {
                    return;
                }
                let doc = doc_of(&m.attrs);
                self.inv.items.push(Item {
                    path: format!("{CRATE}::{id}"),
                    name: id.to_string(),
                    kind: ItemKind::Macro,
                    exported: true,
                    declared_pub: true,
                    file: rel.to_string(),
                    line: line_of(id.span()),
                    examples: examples_of(&doc),
                    doc,
                    owner: CRATE.to_string(),
                    trivial_accessor: false,
                    deprecated: deprecation(&m.attrs),
                });
            }
            syn::Item::Impl(i) => {
                // an inherent impl of a named type in this module; a trait impl adds no
                // API of its own, because the trait already declares every item in it
                if i.trait_.is_some() {
                    return;
                }
                let Some(ty) = type_name(&i.self_ty) else {
                    return;
                };
                let owner = format!("{mod_path}::{ty}");
                for it in &i.items {
                    let syn::ImplItem::Fn(f) = it else { continue };
                    let doc = doc_of(&f.attrs);
                    self.inv.items.push(Item {
                        path: format!("{owner}::{}", f.sig.ident),
                        name: f.sig.ident.to_string(),
                        kind: receiver_kind(&f.sig),
                        // the impl cannot widen what the type offers: a method is exported
                        // when it is `pub` and its type is
                        exported: exported && is_pub(&f.vis),
                        declared_pub: is_pub(&f.vis),
                        file: rel.to_string(),
                        line: line_of(f.sig.ident.span()),
                        examples: examples_of(&doc),
                        doc,
                        owner: owner.clone(),
                        trivial_accessor: is_trivial_accessor(&f.sig, &f.block),
                        deprecated: deprecation(&f.attrs),
                    });
                }
            }
            syn::Item::Use(u) => {
                if exported && is_pub(&u.vis) {
                    let mut targets = Vec::new();
                    resolve_use(&u.tree, mod_path, String::new(), &mut targets);
                    for target in targets {
                        // the name the re-export offers it under: this module, then the
                        // item's own last segment
                        if let Some(last) = target.rsplit("::").next() {
                            self.inv
                                .reexport_aliases
                                .insert(format!("{mod_path}::{last}"), target.clone());
                        }
                        self.inv.reexported.insert(target);
                    }
                }
            }
            _ => {}
        }
    }

    /// The file a `mod name;` in `parent` refers to, by rustc's rules.
    fn resolve(&self, parent: &Path, name: &str) -> Result<PathBuf, Error> {
        let dir = if parent.file_name().and_then(|n| n.to_str()) == Some("lib.rs")
            || parent.file_name().and_then(|n| n.to_str()) == Some("mod.rs")
        {
            parent.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            parent.with_extension("")
        };
        let flat = dir.join(format!("{name}.rs"));
        if flat.is_file() {
            return Ok(flat);
        }
        let nested = dir.join(name).join("mod.rs");
        if nested.is_file() {
            return Ok(nested);
        }
        Err(Error::InvalidSource {
            path: self.relative(parent),
            reason: format!(
                "`mod {name};` resolves to neither {} nor {}",
                self.relative(&flat),
                self.relative(&nested)
            ),
        })
    }

    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(&self.crate_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

/// Resolve one `use` tree into the crate paths it re-exports.
///
/// `at` is the module the `use` is written in; `prefix` is what has been resolved so far.
/// A leading `crate` or `self` anchors at the crate root or at `at`; `super` climbs one;
/// anything else is taken relative to `at`, which is how `pub use docs::tree;` reads. A
/// glob contributes the module itself, and [`apply_reexports`] takes everything under it.
fn resolve_use(tree: &syn::UseTree, at: &str, prefix: String, out: &mut Vec<String>) {
    let base = |seg: &str| -> String {
        if !prefix.is_empty() {
            return format!("{prefix}::{seg}");
        }
        match seg {
            "crate" => CRATE.to_string(),
            "self" => at.to_string(),
            "super" => at
                .rsplit_once("::")
                .map(|(o, _)| o.to_string())
                .unwrap_or(CRATE.to_string()),
            other => format!("{at}::{other}"),
        }
    };
    match tree {
        syn::UseTree::Path(p) => {
            // `base` already handles `crate`, `self` and `super` at the head of a path
            let next = base(&p.ident.to_string());
            resolve_use(&p.tree, at, next, out);
        }
        syn::UseTree::Name(n) => out.push(base(&n.ident.to_string())),
        // `pub use x as y` re-exports the item under a new name; the item itself is what
        // becomes reachable, and that is what the inventory is about
        syn::UseTree::Rename(r) => out.push(base(&r.ident.to_string())),
        syn::UseTree::Glob(_) => {
            if !prefix.is_empty() {
                out.push(prefix)
            }
        }
        syn::UseTree::Group(g) => {
            for t in &g.items {
                resolve_use(t, at, prefix.clone(), out);
            }
        }
    }
}

/// Mark as exported every item a `pub use` made reachable, and everything under it: a
/// re-exported module brings its public items with it.
fn apply_reexports(inv: &mut Inventory) {
    if inv.reexported.is_empty() {
        return;
    }
    let targets = inv.reexported.clone();
    let reached = |path: &str| {
        targets.contains(path) || targets.iter().any(|t| path.starts_with(&format!("{t}::")))
    };
    // Indices and not paths: a struct's private field and one of its public methods can
    // share a path — `Served::topology` names both — so marking every item *at* a path
    // would carry the private one out along with the public one.
    let reachable: Vec<usize> = inv
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| {
            // a re-export carries out what is declared `pub`; a private member of a
            // re-exported type stays private, which is what rustc thinks too
            !i.exported && i.declared_pub && reached(&i.path)
        })
        .map(|(n, _)| n)
        .collect();
    // the owner chain has to be public for the item to be, and a re-export is exactly what
    // makes it so: the item is exported, its private module is not. One pass is enough
    // because reachability is decided against the re-export targets, not against what this
    // pass has already marked.
    for n in reachable {
        inv.items[n].exported = true;
    }
}

/// Every `majordomus_cli::a::b` path the crate's integration tests and benches name, and
/// every prefix of it, so a module counts as covered when a test names it or anything
/// under it.
fn collect_test_references(crate_dir: &Path, inv: &mut Inventory) -> Result<(), Error> {
    for sub in ["tests", "benches"] {
        let dir = crate_dir.join(sub);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("rs") {
                files.push(p);
            } else if p.is_dir() {
                if let Ok(inner) = std::fs::read_dir(&p) {
                    for e in inner.flatten() {
                        let p = e.path();
                        if p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("rs") {
                            files.push(p);
                        }
                    }
                }
            }
        }
        files.sort();
        for p in files {
            let rel = p
                .strip_prefix(crate_dir)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&p).map_err(|e| Error::InvalidSource {
                path: rel.clone(),
                reason: e.to_string(),
            })?;
            inv.test_files.push(rel);
            for path in crate_paths_in(&text) {
                let mut prefix = String::new();
                for seg in path.split("::") {
                    if prefix.is_empty() {
                        prefix.push_str(seg);
                    } else {
                        prefix.push_str("::");
                        prefix.push_str(seg);
                    }
                    inv.named_by_tests.insert(prefix.clone());
                }
            }
        }
    }
    Ok(())
}

/// Every `majordomus_cli::…` path mentioned in a source text, including the members of a
/// `use majordomus_cli::a::{b, c}` group, which is how these tests are written.
fn crate_paths_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while let Some(found) = text[i..].find(CRATE) {
        let start = i + found;
        // not part of a longer identifier
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        i = start + CRATE.len();
        if !before_ok {
            continue;
        }
        let mut path = String::from(CRATE);
        let mut j = i;
        loop {
            let rest = &text[j..];
            let trimmed = rest.trim_start();
            let skipped = rest.len() - trimmed.len();
            if let Some(after) = trimmed.strip_prefix("::") {
                let after_trim = after.trim_start();
                let lead = after.len() - after_trim.len();
                if after_trim.starts_with('{') {
                    // a group: every member is a path under what we have so far
                    let close = match_brace(&text[j + skipped + 2 + lead..]);
                    let inner = &text[j + skipped + 2 + lead + 1..j + skipped + 2 + lead + close];
                    for member in inner.split(',') {
                        let m = member.trim().trim_start_matches("self").trim();
                        let m = m.split(" as ").next().unwrap_or(m).trim();
                        if m.is_empty() {
                            continue;
                        }
                        out.push(format!("{path}::{m}"));
                    }
                    j = j + skipped + 2 + lead + close + 1;
                    break;
                }
                let ident_len = after_trim.bytes().take_while(|b| is_ident_byte(*b)).count();
                if ident_len == 0 {
                    break;
                }
                path.push_str("::");
                path.push_str(&after_trim[..ident_len]);
                j = j + skipped + 2 + lead + ident_len;
                continue;
            }
            break;
        }
        out.push(path);
        i = j.max(i);
    }
    out
}

fn match_brace(text: &str) -> usize {
    let mut depth = 0usize;
    for (idx, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return idx;
                }
            }
            _ => {}
        }
    }
    text.len().saturating_sub(1)
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// `pub` and nothing narrower. `pub(crate)`, `pub(super)` and `pub(in path)` are the
/// crate's own business and export nothing.
fn is_pub(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

fn line_of(span: proc_macro2::Span) -> usize {
    span.start().line
}

/// A `#[cfg(test)]` item: evidence rather than surface.
fn is_test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("cfg")
            && a.meta
                .require_list()
                .ok()
                .map(|l| l.tokens.to_string().replace(' ', "") == "test")
                .unwrap_or(false)
    })
}

fn test_fn_names(items: &[syn::Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|i| match i {
            syn::Item::Fn(f)
                if f.attrs.iter().any(|a| {
                    a.path().is_ident("test")
                        || a.path()
                            .segments
                            .last()
                            .is_some_and(|s| s.ident == "test" || s.ident == "proptest")
                }) =>
            {
                Some(f.sig.ident.to_string())
            }
            syn::Item::Mod(m) => m
                .content
                .as_ref()
                .map(|(_, inner)| test_fn_names(inner))
                .unwrap_or_default()
                .first()
                .cloned(),
            _ => None,
        })
        .collect()
}

/// The doc comment of an item, markers stripped, one line per `///`.
fn doc_of(attrs: &[syn::Attribute]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for a in attrs {
        if !a.path().is_ident("doc") {
            continue;
        }
        let syn::Meta::NameValue(nv) = &a.meta else {
            continue;
        };
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) = &nv.value
        else {
            continue;
        };
        let raw = s.value();
        lines.push(raw.strip_prefix(' ').unwrap_or(&raw).to_string());
    }
    lines.join("\n")
}

fn deprecation(attrs: &[syn::Attribute]) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.path().is_ident("deprecated"))
        .map(|a| match &a.meta {
            syn::Meta::Path(_) => String::new(),
            other => other
                .require_list()
                .map(|l| l.tokens.to_string())
                .unwrap_or_default(),
        })
}

/// The fenced blocks of a doc comment, in order.
fn examples_of(doc: &str) -> Vec<Example> {
    let mut out = Vec::new();
    let mut open: Option<(String, Vec<String>)> = None;
    for line in doc.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            match open.take() {
                Some((info, lines)) => out.push(Example::parse(&info, &lines)),
                None => open = Some((rest.trim().to_string(), Vec::new())),
            }
        } else if let Some((_, lines)) = open.as_mut() {
            lines.push(line.to_string());
        }
    }
    if let Some((info, lines)) = open {
        out.push(Example::parse(&info, &lines));
    }
    out
}

fn receiver_kind(sig: &syn::Signature) -> ItemKind {
    if sig.receiver().is_some() {
        ItemKind::Method
    } else {
        ItemKind::Function
    }
}

fn type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        syn::Type::Reference(r) => type_name(&r.elem),
        _ => None,
    }
}

/// A function whose body hands back what the receiver already holds and takes nothing but
/// the receiver: `self.0`, `&self.name`, `self.field.as_str()`, `self.len() == 0`.
///
/// Syntactic and deliberately narrow. It exists so that the example policy can be
/// demanded of everything else without exception: an accessor's example shows a reader
/// nothing its signature has not already said, and a rule that demands one is a rule that
/// buys placeholders.
fn is_trivial_accessor(sig: &syn::Signature, block: &syn::Block) -> bool {
    if sig.receiver().is_none() || sig.inputs.len() != 1 {
        return false;
    }
    if block.stmts.len() != 1 {
        return false;
    }
    let syn::Stmt::Expr(expr, None) = &block.stmts[0] else {
        return false;
    };
    fn simple(expr: &syn::Expr, depth: usize) -> bool {
        if depth > 3 {
            return false;
        }
        match expr {
            syn::Expr::Field(f) => simple(&f.base, depth + 1),
            syn::Expr::Path(p) => p.path.is_ident("self"),
            syn::Expr::Reference(r) => simple(&r.expr, depth + 1),
            syn::Expr::Unary(u) => simple(&u.expr, depth + 1),
            syn::Expr::Paren(p) => simple(&p.expr, depth + 1),
            syn::Expr::Lit(_) => true,
            // `self.field.as_str()`, `self.0.clone()`: a no-argument call on something simple
            syn::Expr::MethodCall(m) => m.args.is_empty() && simple(&m.receiver, depth + 1),
            // `self.len() == 0`, `!self.done`
            syn::Expr::Binary(b) => simple(&b.left, depth + 1) && simple(&b.right, depth + 1),
            _ => false,
        }
    }
    simple(expr, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    #[test]
    fn a_pub_item_under_a_private_module_is_not_exported() {
        let dir = fixture(&[
            ("src/lib.rs", "//! Root.\npub mod open;\nmod closed;\n"),
            ("src/open/mod.rs", "//! Open.\npub fn seen() {}\n"),
            ("src/closed.rs", "//! Closed.\npub fn unseen() {}\n"),
        ]);
        let inv = Inventory::of_crate(dir.path()).unwrap();
        assert!(inv.item("majordomus_cli::open::seen").unwrap().exported);
        assert!(!inv.item("majordomus_cli::closed::unseen").unwrap().exported);
        // and the private module itself is not part of the surface
        assert!(!inv.item("majordomus_cli::closed").unwrap().exported);
    }

    #[test]
    fn pub_crate_is_not_exported_and_neither_is_what_it_contains() {
        let dir = fixture(&[
            ("src/lib.rs", "//! Root.\npub(crate) mod internal;\n"),
            ("src/internal.rs", "//! Internal.\npub struct T;\n"),
        ]);
        let inv = Inventory::of_crate(dir.path()).unwrap();
        assert!(!inv.item("majordomus_cli::internal::T").unwrap().exported);
        assert!(inv.exported().iter().all(|i| i.name != "T"));
    }

    #[test]
    fn only_a_rust_block_without_ignore_counts_as_an_executable_example() {
        let doc = "Prose.\n\n```text\na diagram\n```\n\n```ignore\nnever_compiled();\n```\n\n```no_run\nserve();\n```\n";
        let found = examples_of(doc);
        assert_eq!(found.len(), 3);
        assert!(!found[0].executable(), "text");
        assert!(!found[1].executable(), "ignore");
        assert!(
            found[2].compiles && !found[2].runs,
            "no_run compiles and does not run"
        );
    }

    #[test]
    fn an_in_file_test_module_is_evidence_and_not_surface() {
        let dir = fixture(&[(
            "src/lib.rs",
            "//! Root.\npub fn go() {}\n#[cfg(test)]\nmod tests {\n #[test]\n fn it_goes() {}\n}\n",
        )]);
        let inv = Inventory::of_crate(dir.path()).unwrap();
        assert!(inv.item("majordomus_cli::tests").is_none());
        assert_eq!(
            inv.in_file_tests["majordomus_cli"].names,
            vec!["it_goes".to_string()]
        );
    }

    #[test]
    fn a_trivial_accessor_is_recognised_and_a_computation_is_not() {
        let dir = fixture(&[(
            "src/lib.rs",
            r#"//! Root.
/// A type.
pub struct T { n: usize }
impl T {
    /// The count.
    pub fn n(&self) -> usize { self.n }
    /// Whether it is empty.
    pub fn empty(&self) -> bool { self.n == 0 }
    /// Doubled.
    pub fn doubled(&self) -> usize { let m = self.n; m * 2 }
}
"#,
        )]);
        let inv = Inventory::of_crate(dir.path()).unwrap();
        let method = |name: &str| {
            inv.item_of(&format!("majordomus_cli::T::{name}"), ItemKind::Method)
                .unwrap_or_else(|| panic!("no method {name}"))
        };
        assert!(method("n").trivial_accessor);
        assert!(method("empty").trivial_accessor);
        assert!(!method("doubled").trivial_accessor);
    }

    #[test]
    fn a_test_file_naming_a_module_covers_it_and_every_prefix() {
        let dir = fixture(&[
            ("src/lib.rs", "//! Root.\npub mod a;\n"),
            ("src/a/mod.rs", "//! A.\npub mod b;\n"),
            ("src/a/b.rs", "//! B.\npub fn go() {}\n"),
            (
                "tests/it.rs",
                "use majordomus_cli::a::b::go;\n#[test]\nfn t() { go(); }\n",
            ),
        ]);
        let inv = Inventory::of_crate(dir.path()).unwrap();
        assert!(inv.named_by_tests.contains("majordomus_cli::a"));
        assert!(inv.named_by_tests.contains("majordomus_cli::a::b"));
        assert!(inv.named_by_tests.contains("majordomus_cli::a::b::go"));
    }

    #[test]
    fn a_use_group_names_every_member_of_it() {
        let found = crate_paths_in("use majordomus_cli::capability::{Capability, Exposure};");
        assert!(found.contains(&"majordomus_cli::capability::Capability".to_string()));
        assert!(found.contains(&"majordomus_cli::capability::Exposure".to_string()));
    }

    #[test]
    fn a_macro_export_is_reached_from_the_crate_root_wherever_it_is_written() {
        let dir = fixture(&[
            ("src/lib.rs", "//! Root.\npub mod deep;\n"),
            (
                "src/deep.rs",
                "//! Deep.\n/// Does a thing.\n#[macro_export]\nmacro_rules! thing { () => {} }\n",
            ),
        ]);
        let inv = Inventory::of_crate(dir.path()).unwrap();
        let m = inv.item("majordomus_cli::thing").unwrap();
        assert_eq!(m.kind, ItemKind::Macro);
        assert!(m.exported);
    }
}
