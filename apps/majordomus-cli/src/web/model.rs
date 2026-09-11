//! The typed model of a web surface: what it is, where it is mounted, what produced it,
//! and where every value came from.
//!
//! One vocabulary serves every consumer — the router, the static composition, the
//! validator, the listing and the generated manifest — so a surface is described once and
//! read many times. The kinds are behavioural rather than nominal: a test report and a
//! benchmark report are both [`SurfaceKind::StaticDirectory`] and differ only in data, and
//! nothing in this module knows the name of any surface this repository happens to have.
//!
//! The lifecycle is: describe a surface, normalise its mount on the way in, hold the set as
//! a [`Topology`], and then read that one value as many times as there are consumers.
//!
//! ```
//! use majordomus_cli::web::model::*;
//! # use std::collections::BTreeMap;
//! let report = Surface {
//!     id: "tests".into(),
//!     title: "The suite, as it last ran".into(),
//!     category: Category::Report,
//!     visibility: Visibility::Public,
//!     kind: SurfaceKind::StaticDirectory,
//!     mount: Mount::parse("/tests/").unwrap(),
//!     producer: "scripts/test-report".into(),
//!     feature: None,
//!     artifact: Some("target/web/tests".into()),
//!     index: Some("index.html".into()),
//!     availability: Availability::Both,
//!     built_from: None,
//!     provenance: BTreeMap::new(),
//! };
//! // the mount was normalised when it was parsed, not repaired when it is read
//! assert_eq!(report.mount.as_str(), "/tests");
//!
//! let topology = Topology::new(vec![report]);
//! // one value, read as a router reads it and as a publication reads it
//! assert_eq!(topology.owner("/tests/index.html").map(|s| s.id.as_str()), Some("tests"));
//! assert!(topology.owner("/elsewhere").is_none(), "nothing here claims the root");
//! assert_eq!(topology.published().ids(), vec!["tests"]);
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How a surface answers a request.
///
/// Only what this repository serves: a directory of generated files, a path the executable
/// answers itself, and a redirect. A new *kind* is a new behaviour, never a new name for
/// the same behaviour with different data.
///
/// The kind decides who answers a request, and nothing else does: a consumer that wanted to
/// know whether to open a file or call a handler asks this and never the mount.
///
/// ```
/// use majordomus_cli::web::model::SurfaceKind;
/// // a listing shows the short word; a document carries the variant's own name, and the
/// // two are readings of one kind rather than two kinds
/// assert_eq!(SurfaceKind::StaticDirectory.to_string(), "static");
/// assert_eq!(
///     serde_json::to_value(SurfaceKind::StaticDirectory).unwrap(),
///     serde_json::json!("static-directory")
/// );
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum SurfaceKind {
    /// A generated directory, mounted under its prefix and served from disk.
    StaticDirectory,
    /// A path the executable answers itself, from its own registry or handlers.
    NativeRoute,
    /// A path that answers with a redirect to another one.
    Redirect,
}

impl fmt::Display for SurfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SurfaceKind::StaticDirectory => "static",
            SurfaceKind::NativeRoute => "native",
            SurfaceKind::Redirect => "redirect",
        })
    }
}

/// What a surface is for, which is how a reader is shown it.
///
/// A category is the one piece of intent that a mount cannot carry: `/openapi.json` and
/// `/swagger` sit beside each other and are a document and a viewer for it. Grouping is
/// derived from this field and never from a list of paths kept somewhere else.
///
/// ```
/// use majordomus_cli::web::model::Category;
/// // every category reaches a heading, and no two share one: a duplicate title would
/// // silently merge two groups into one section of a page
/// let mut titles: Vec<&str> = Category::ALL.iter().map(|c| c.title()).collect();
/// titles.sort_unstable();
/// titles.dedup();
/// assert_eq!(titles.len(), Category::ALL.len());
/// // and the word a document carries is the word `Display` writes
/// let report = serde_json::to_value(Category::Report).unwrap();
/// assert_eq!(report, serde_json::json!(Category::Report.to_string()));
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
// the schema component is named for what it categorises: `Category` alone already names
// the scope's classes, and two components of one name is a document that cannot be built
#[schemars(rename = "SurfaceCategory")]
pub enum Category {
    /// Something a person opens and looks at: the home page, the Cockpit.
    Interface,
    /// Prose and reference written for a person: the site, the Swagger UI.
    Documentation,
    /// A machine-readable surface of the capability registry.
    Api,
    /// A wire protocol another program speaks.
    Protocol,
    /// Generated evidence of a run: a test report, a benchmark report.
    Report,
}

impl Category {
    /// Every category, in the order a listing shows them.
    pub const ALL: [Category; 5] = [
        Category::Interface,
        Category::Documentation,
        Category::Api,
        Category::Protocol,
        Category::Report,
    ];

    /// The heading a person reads, which is the one place a category's name is written for
    /// a human rather than for a machine.
    ///
    /// It is plural because it names a group and not a member, and it is not the
    /// serialised word: `api` is what a document carries and "API" is what a reader sees.
    /// A page renders this and never a heading of its own, so a category that is renamed
    /// is renamed everywhere it appears.
    ///
    /// ```
    /// use majordomus_cli::web::model::Category;
    /// assert_eq!(Category::Api.title(), "API");
    /// assert_ne!(Category::Api.title(), Category::Api.to_string());
    /// ```
    pub fn title(self) -> &'static str {
        match self {
            Category::Interface => "Interfaces",
            Category::Documentation => "Documentation",
            Category::Api => "API",
            Category::Protocol => "Protocols",
            Category::Report => "Reports",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Category::Interface => "interface",
            Category::Documentation => "documentation",
            Category::Api => "api",
            Category::Protocol => "protocol",
            Category::Report => "report",
        })
    }
}

/// Who a surface is offered to.
///
/// Two values and no more: a surface is either offered for a person to discover, or it is
/// part of the topology without being advertised. Both are always in the machine-readable
/// answer — hiding a served route from introspection would only hide it from the people
/// maintaining it.
///
/// ```
/// use majordomus_cli::web::{discover, model::Visibility, Topology};
/// // `Internal` narrows what a person is shown and never what a diagnostic can see: the
/// // protocol routes are unlisted, and the topology still holds them
/// let t = Topology::new(discover::native(discover::Runtime::full()));
/// let mcp = t.get("mcp").expect("this process's MCP endpoint is a surface");
/// assert_eq!(mcp.visibility, Visibility::Internal);
/// assert_eq!(serde_json::to_value(Visibility::Internal).unwrap(), "internal");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
// A surface's visibility and a capability's are different vocabularies — that one has a
// third value, `developer`. The schema component namespace is flat, so each says which of
// the two it is (see `crate::capability::model::Visibility`).
#[schemars(rename = "SurfaceVisibility")]
pub enum Visibility {
    /// Listed for a person: it appears on the home page.
    Public,
    /// Served and introspectable, not advertised: a machine speaks to it, or another
    /// surface links to it.
    Internal,
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Visibility::Public => "public",
            Visibility::Internal => "internal",
        })
    }
}

/// A capability of the running process a surface needs in order to exist.
///
/// The registry describes the effective process, not the maximum one: a build or an
/// invocation that answers no MCP has no MCP surface, and the home page cannot link to
/// one. Stating the dependency as data is what keeps that automatic.
///
/// ```
/// use majordomus_cli::web::discover::Runtime;
/// use majordomus_cli::web::model::Feature;
/// // the surface declares what it needs and the process declares what it has; neither
/// // side carries a list of the other's names
/// assert!(Runtime::full().has(Feature::Mcp));
/// assert!(!Runtime::default().has(Feature::Mcp));
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// This process answers MCP over HTTP.
    Mcp,
    /// This process serves the Cockpit.
    Cockpit,
}

impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Feature::Mcp => "mcp",
            Feature::Cockpit => "cockpit",
        })
    }
}

/// Where a resolved value came from.
///
/// Kept for every field a consumer can be surprised by, so that `web explain` can answer
/// "why is this mounted here?" without anybody reading the discovery code.
///
/// The variants are the inference sources and not free text: a value came from the
/// capability registry, from a producer's declaration, from a file being where the
/// convention says, from the site's configuration, or from a documented default. There is
/// no variant for "somebody wrote it down here", because that is what this type exists to
/// make impossible.
///
/// ```
/// use majordomus_cli::web::model::Provenance;
/// let inferred = Provenance::Filesystem { path: "target/web/tests".into() };
/// // the tag is the source and the payload is what that source needed to name
/// let json = serde_json::to_value(&inferred).unwrap();
/// assert_eq!(json["source"], "filesystem");
/// assert_eq!(json["path"], "target/web/tests");
/// assert_eq!(inferred.to_string(), "filesystem target/web/tests");
/// // a source that names nothing carries nothing
/// assert_eq!(serde_json::to_value(Provenance::Default).unwrap()["source"], "default");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum Provenance {
    /// Read from the capability registry: the routes the executable already declares once.
    Registry,
    /// Read from a producer's own declaration beside its output (`surface.json`).
    ProducerDeclaration {
        /// The declaration file, repository-relative.
        path: String,
    },
    /// Inferred from a file or directory being where the convention says it is.
    Filesystem {
        /// What was found, repository-relative.
        path: String,
    },
    /// Inferred from the site generator's own configuration.
    SiteConfig {
        /// The configuration file, repository-relative.
        path: String,
    },
    /// The model's documented default for a value nobody stated.
    Default,
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Provenance::Registry => f.write_str("capability registry"),
            Provenance::ProducerDeclaration { path } => write!(f, "producer declaration {path}"),
            Provenance::Filesystem { path } => write!(f, "filesystem {path}"),
            Provenance::SiteConfig { path } => write!(f, "site config {path}"),
            Provenance::Default => f.write_str("default"),
        }
    }
}

/// A normalised mount path: absolute, without a trailing slash, without `.` or `..`.
///
/// Normalisation is idempotent and total — an unacceptable path is refused rather than
/// repaired, because a mount is intent and a guess at intent is how a surface ends up
/// somewhere nobody meant.
///
/// ```
/// use majordomus_cli::web::model::Mount;
/// assert_eq!(Mount::parse("/tests/").unwrap().as_str(), "/tests");
/// assert_eq!(Mount::parse("/").unwrap().as_str(), "/");
/// assert_eq!(Mount::parse("tests").unwrap().as_str(), "/tests");
/// assert!(Mount::parse("/a/../b").is_err());
/// assert!(Mount::parse("//a").is_err());
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
#[schemars(
    with = "String",
    description = "An absolute mount path, without a trailing slash."
)]
pub struct Mount(String);

impl Mount {
    /// The root mount, which owns everything no other surface owns.
    ///
    /// It is the fallback of the topology rather than a prefix within it: it is sorted last
    /// so that every declared mount is consulted before it, and it composes into a
    /// destination root rather than into a directory below one.
    ///
    /// ```
    /// use majordomus_cli::web::model::Mount;
    /// assert!(Mount::root().is_root());
    /// assert_eq!(Mount::root(), Mount::parse("/").unwrap());
    /// assert!(Mount::root().owns("/whatever/nobody/declared"));
    /// assert_eq!(Mount::root().relative(), "", "the root composes into the destination");
    /// ```
    pub fn root() -> Self {
        Mount("/".into())
    }

    /// Parse and normalise a mount path, or say why it is not one.
    ///
    /// ```
    /// use majordomus_cli::web::model::Mount;
    /// // normalisation is idempotent
    /// let once = Mount::parse("/benchmarks/").unwrap();
    /// let twice = Mount::parse(once.as_str()).unwrap();
    /// assert_eq!(once, twice);
    /// ```
    pub fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("a mount path is empty".into());
        }
        if trimmed.contains(char::is_whitespace) {
            return Err(format!("mount '{raw}' contains whitespace"));
        }
        if trimmed.contains('\\') {
            return Err(format!(
                "mount '{raw}' contains a backslash; paths are '/'-separated"
            ));
        }
        let with_slash = if trimmed.starts_with('/') {
            trimmed.to_string()
        } else {
            format!("/{trimmed}")
        };
        let body = with_slash.trim_end_matches('/');
        if body.is_empty() {
            return Ok(Mount::root());
        }
        for segment in body.split('/').skip(1) {
            if segment.is_empty() {
                return Err(format!("mount '{raw}' has an empty segment"));
            }
            if segment == "." || segment == ".." {
                return Err(format!("mount '{raw}' walks the path with '{segment}'"));
            }
            if segment.contains('?') || segment.contains('#') {
                return Err(format!("mount '{raw}' carries a query or a fragment"));
            }
        }
        Ok(Mount(body.to_string()))
    }

    /// The mount as it is written in a route table.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Is this the root mount, the one surface allowed to answer what nothing else claims?
    pub fn is_root(&self) -> bool {
        self.0 == "/"
    }

    /// The prefix a request path must carry to belong to this mount, with its trailing slash.
    ///
    /// ```
    /// use majordomus_cli::web::model::Mount;
    /// assert_eq!(Mount::parse("/tests").unwrap().prefix(), "/tests/");
    /// assert_eq!(Mount::root().prefix(), "/");
    /// ```
    pub fn prefix(&self) -> String {
        if self.is_root() {
            "/".into()
        } else {
            format!("{}/", self.0)
        }
    }

    /// Does this mount own `path` — the mount itself, or anything below it?
    ///
    /// ```
    /// use majordomus_cli::web::model::Mount;
    /// let m = Mount::parse("/tests").unwrap();
    /// assert!(m.owns("/tests"));
    /// assert!(m.owns("/tests/"));
    /// assert!(m.owns("/tests/coverage/index.html"));
    /// assert!(!m.owns("/tests-of-something"));
    /// assert!(Mount::root().owns("/anything"));
    /// ```
    pub fn owns(&self, path: &str) -> bool {
        if self.is_root() {
            return path.starts_with('/');
        }
        path == self.0 || path.starts_with(&self.prefix())
    }

    /// Is `other` inside this mount? The root contains everything, itself included.
    ///
    /// ```
    /// use majordomus_cli::web::model::Mount;
    /// let outer = Mount::parse("/tests").unwrap();
    /// let inner = Mount::parse("/tests/coverage").unwrap();
    /// assert!(outer.contains(&inner));
    /// assert!(!inner.contains(&outer));
    /// ```
    pub fn contains(&self, other: &Mount) -> bool {
        if self.is_root() {
            return true;
        }
        other.0 == self.0 || other.0.starts_with(&self.prefix())
    }

    /// The path under a destination root this mount composes into, relative and without a
    /// leading slash: the root mount composes into the destination itself.
    ///
    /// ```
    /// use majordomus_cli::web::model::Mount;
    /// assert_eq!(Mount::parse("/tests").unwrap().relative(), "tests");
    /// assert_eq!(Mount::root().relative(), "");
    /// ```
    pub fn relative(&self) -> String {
        self.0.trim_start_matches('/').to_string()
    }
}

impl TryFrom<String> for Mount {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Mount::parse(&value)
    }
}

impl From<Mount> for String {
    fn from(value: Mount) -> Self {
        value.0
    }
}

impl fmt::Display for Mount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether a surface is part of the static publication, served only while a process runs,
/// or both.
///
/// The two worlds are what a link on a page can and cannot promise. A published page that
/// linked a `ServedOnly` surface would be offering an address nothing answers, which is why
/// this field and not a template decides what is offered as a link.
///
/// ```
/// use majordomus_cli::web::model::Availability;
/// // there is no fourth variant, because a surface nobody answers for is not a surface
/// for a in [Availability::Both, Availability::ServedOnly, Availability::PublishedOnly] {
///     assert!(a.is_served() || a.is_published(), "{a:?} answers in neither world");
/// }
/// assert!(!Availability::ServedOnly.is_published());
/// assert!(!Availability::PublishedOnly.is_served());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "SurfaceAvailability")]
pub enum Availability {
    /// Served by the running executable and published as files.
    Both,
    /// Answered by the running executable and never published: a route the process
    /// computes, or a build made for this server's own mount rather than for deployment.
    ServedOnly,
    /// Published as files; the running executable serves it from the same directory.
    PublishedOnly,
}

impl Availability {
    /// Does this surface contribute files to a static publication?
    ///
    /// This is the question a published page asks before offering a link, and the answer a
    /// running server never needs.
    ///
    /// ```
    /// use majordomus_cli::web::model::Availability;
    /// assert!(Availability::PublishedOnly.is_published());
    /// assert!(Availability::Both.is_published());
    /// assert!(!Availability::ServedOnly.is_published());
    /// ```
    pub fn is_published(&self) -> bool {
        matches!(self, Availability::Both | Availability::PublishedOnly)
    }

    /// Does the running server answer for this surface?
    ///
    /// Being served is necessary and not sufficient: a surface whose [`Feature`] this
    /// process does not have is not answered either, which is what [`Surface::served_by`]
    /// adds on top of this.
    ///
    /// ```
    /// use majordomus_cli::web::model::Availability;
    /// assert!(Availability::ServedOnly.is_served());
    /// assert!(Availability::Both.is_served());
    /// // the deployment is a tree of files somebody else hosts; this process never has it
    /// assert!(!Availability::PublishedOnly.is_served());
    /// ```
    pub fn is_served(&self) -> bool {
        matches!(self, Availability::Both | Availability::ServedOnly)
    }
}

/// One resolved surface: everything a consumer needs, with the provenance of what it could
/// be surprised by.
///
/// A resolved surface is the end of inference and the beginning of use: by the time one
/// exists, every default has been applied and every value that was inferred says where it
/// came from. The invariant a consumer may rely on is that the fields agree — a surface
/// that publishes has a directory to publish, and one that needs a runtime feature says
/// which — so no consumer has to re-derive what discovery already decided.
///
/// ```
/// use majordomus_cli::web::model::*;
/// # use std::collections::BTreeMap;
/// let mut benchmarks = Surface {
///     id: "benchmarks".into(),
///     title: "Recorded runs of the capability surface".into(),
///     category: Category::Report,
///     visibility: Visibility::Public,
///     kind: SurfaceKind::StaticDirectory,
///     mount: Mount::parse("/benchmarks").unwrap(),
///     producer: "bench report".into(),
///     feature: None,
///     artifact: None,
///     index: Some("index.html".into()),
///     availability: Availability::Both,
///     built_from: None,
///     provenance: BTreeMap::new(),
/// };
/// // saying "published" is not enough: with no directory there are no files to publish,
/// // so a publication would carry an empty promise
/// assert!(!benchmarks.publishes());
/// benchmarks.artifact = Some("target/web/benchmarks".into());
/// assert!(benchmarks.publishes());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Surface {
    /// Identity, unique across the topology; the selector `--only` and `--exclude` use it.
    pub id: String,
    /// One line: what a reader sees in a listing.
    pub title: String,
    /// What it is for, which is how a listing groups it.
    pub category: Category,
    /// Whether a person is shown it.
    pub visibility: Visibility,
    /// How it answers.
    pub kind: SurfaceKind,
    /// Where it answers.
    pub mount: Mount,
    /// What produced it: a command, a module, or the generator that writes the directory.
    pub producer: String,
    /// The runtime capability it needs; absent when the process always has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<Feature>,
    /// The generated directory, repository-relative, for a static surface.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<PathBuf>,
    /// The file served for the mount itself, when the surface has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    /// Where the surface's files go and who answers for it.
    pub availability: Availability,
    /// The revision the artifact was built from, when its producer recorded one: what
    /// makes a stale build a finding rather than a surprise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub built_from: Option<String>,
    /// Where each interesting value came from, by field name.
    pub provenance: BTreeMap<String, Provenance>,
}

impl Surface {
    /// Does this surface put files into a publication?
    pub fn publishes(&self) -> bool {
        self.availability.is_published() && self.artifact.is_some()
    }

    /// Does a process with these runtime capabilities answer for this surface?
    ///
    /// ```
    /// use majordomus_cli::web::discover::Runtime;
    /// use majordomus_cli::web::model::Feature;
    /// # use majordomus_cli::web::model::*;
    /// # use std::collections::BTreeMap;
    /// let mut s = Surface { id: "mcp".into(), title: "MCP".into(), category: Category::Protocol,
    ///     visibility: Visibility::Internal, kind: SurfaceKind::NativeRoute,
    ///     mount: Mount::parse("/mcp").unwrap(), producer: "t".into(), feature: Some(Feature::Mcp),
    ///     artifact: None, index: None, availability: Availability::ServedOnly,
    ///     built_from: None, provenance: BTreeMap::new() };
    /// assert!(s.served_by(Runtime::full()));
    /// assert!(!s.served_by(Runtime::default()));
    /// s.feature = None;
    /// assert!(s.served_by(Runtime::default()));
    /// ```
    pub fn served_by(&self, runtime: crate::web::discover::Runtime) -> bool {
        self.availability.is_served() && self.feature.is_none_or(|f| runtime.has(f))
    }
}

/// Every surface this repository exposes, resolved together.
///
/// The order is by mount, most specific first, then by id: the order a router must consult
/// them in, computed rather than left to whoever inserted a route last.
///
/// Every reading of the topology — what a process serves, what a publication carries, what
/// a person is shown — is a filter over this one value, so nothing can be served that was
/// not discovered and no reading can hold a surface the whole does not.
///
/// ```
/// use majordomus_cli::web::{discover, Topology};
/// let t = Topology::new(discover::native(discover::Runtime::full()));
/// // the specific mounts come first and the fallback last, whatever order they arrived in
/// assert_eq!(t.ids().last(), Some(&discover::HOME));
/// assert_eq!(t.owner("/swagger").map(|s| s.id.as_str()), Some("swagger"));
/// // and every reading is a subset of the whole
/// let served = t.served(discover::Runtime::default());
/// assert!(served.ids().iter().all(|id| t.get(id).is_some()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Topology {
    /// The resolved surfaces, in route-precedence order.
    pub surfaces: Vec<Surface>,
}

impl Topology {
    /// Sort surfaces into route-precedence order and hold them.
    ///
    /// Precedence is depth first — a longer mount wins over a shorter one, so the root
    /// application is consulted last — and identity breaks ties, so the order never depends
    /// on discovery order, filesystem order or a hash map.
    ///
    /// ```
    /// use majordomus_cli::web::model::*;
    /// # use std::collections::BTreeMap;
    /// # fn s(id: &str, mount: &str) -> Surface {
    /// #     Surface { id: id.into(), title: id.into(), category: Category::Report,
    /// #         visibility: Visibility::Public, kind: SurfaceKind::StaticDirectory,
    /// #         mount: Mount::parse(mount).unwrap(), producer: "t".into(), feature: None,
    /// #         artifact: None, index: None, availability: Availability::Both,
    /// #         built_from: None, provenance: BTreeMap::new() }
    /// # }
    /// let t = Topology::new(vec![s("app", "/"), s("tests", "/tests")]);
    /// assert_eq!(t.surfaces[0].id, "tests");   // the specific one is consulted first
    /// assert_eq!(t.surfaces[1].id, "app");
    /// // the root is last whatever it is compared with: `owner()` rests on this, and the
    /// // root's own slash must never be counted as depth
    /// let deep = Topology::new(vec![s("api", "/api/v1"), s("root", "/"), s("docs", "/docs")]);
    /// assert_eq!(deep.ids(), vec!["api", "docs", "root"]);
    /// ```
    pub fn new(mut surfaces: Vec<Surface>) -> Self {
        surfaces.sort_by(|a, b| {
            let depth = |m: &Mount| m.as_str().matches('/').count() + usize::from(!m.is_root());
            depth(&b.mount)
                .cmp(&depth(&a.mount))
                .then_with(|| a.mount.cmp(&b.mount))
                .then_with(|| a.id.cmp(&b.id))
        });
        Topology { surfaces }
    }

    /// The surface that owns a request path, or none when nothing does.
    ///
    /// ```
    /// use majordomus_cli::web::model::*;
    /// # use std::collections::BTreeMap;
    /// # fn s(id: &str, mount: &str) -> Surface {
    /// #     Surface { id: id.into(), title: id.into(), category: Category::Report,
    /// #         visibility: Visibility::Public, kind: SurfaceKind::StaticDirectory,
    /// #         mount: Mount::parse(mount).unwrap(), producer: "t".into(), feature: None,
    /// #         artifact: None, index: None, availability: Availability::Both,
    /// #         built_from: None, provenance: BTreeMap::new() }
    /// # }
    /// let t = Topology::new(vec![s("app", "/"), s("tests", "/tests")]);
    /// assert_eq!(t.owner("/tests/index.html").unwrap().id, "tests");
    /// assert_eq!(t.owner("/anything-else").unwrap().id, "app");
    /// ```
    pub fn owner(&self, path: &str) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.mount.owns(path))
    }

    /// One surface by id, or none when *this* topology does not hold it.
    ///
    /// Identity and not path: a selector (`--only`, `--exclude`) and a diagnostic both
    /// resolve through this, and the answer is relative to the reading it is asked of. A
    /// narrowed topology says `None` for a surface the whole one holds, which is the
    /// difference between "no such surface" and "not in this process" — a caller that needs
    /// to tell those apart must ask the unnarrowed value.
    ///
    /// ```
    /// use majordomus_cli::web::{discover, Topology};
    /// let full = Topology::new(discover::native(discover::Runtime::full()));
    /// assert!(full.get("mcp").is_some());
    /// // absent from a process that answers no MCP, and still not an unknown name
    /// assert!(full.served(discover::Runtime::default()).get("mcp").is_none());
    /// assert!(full.get("no-such-surface").is_none());
    /// ```
    pub fn get(&self, id: &str) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    /// The topology narrowed to a selection: `only` when it is not empty, minus `exclude`.
    ///
    /// Selection is by discovered id and nothing else, so a surface that did not exist when
    /// this code was written is selectable the day it is discovered.
    ///
    /// An empty `only` selects everything rather than nothing — the useful reading for a
    /// flag nobody passed — and `exclude` is applied afterwards, so naming one id in both
    /// selects nothing. An id that matches no surface is not an error here: it narrows to
    /// less, and whoever cares that it matched nothing compares the ids.
    ///
    /// ```
    /// use majordomus_cli::web::{discover, Topology};
    /// let t = Topology::new(discover::native(discover::Runtime::full()));
    /// assert_eq!(t.select(&["swagger".to_string()], &[]).ids(), vec!["swagger"]);
    /// assert_eq!(t.select(&[], &[]).ids(), t.ids(), "no selector is every surface");
    /// let both = t.select(&["swagger".to_string()], &["swagger".to_string()]);
    /// assert!(both.ids().is_empty(), "exclusion is applied after selection");
    /// assert!(t.select(&["nothing-is-called-this".to_string()], &[]).ids().is_empty());
    /// ```
    pub fn select(&self, only: &[String], exclude: &[String]) -> Topology {
        Topology::new(
            self.surfaces
                .iter()
                .filter(|s| only.is_empty() || only.iter().any(|o| o == &s.id))
                .filter(|s| !exclude.iter().any(|e| e == &s.id))
                .cloned()
                .collect(),
        )
    }

    /// The ids this topology holds, in precedence order.
    ///
    /// The order is the router's, so this is also the cheapest way to assert that a
    /// narrowing changed what it was meant to change and left the rest alone.
    ///
    /// ```
    /// use majordomus_cli::web::{discover, Topology};
    /// let t = Topology::new(discover::native(discover::Runtime::full()));
    /// assert_eq!(t.ids().len(), t.surfaces.len());
    /// assert_eq!(t.ids().last(), Some(&discover::HOME), "the fallback is consulted last");
    /// ```
    pub fn ids(&self) -> Vec<&str> {
        self.surfaces.iter().map(|s| s.id.as_str()).collect()
    }

    /// The topology as a process with these runtime capabilities serves it.
    ///
    /// One resolution, two worlds: a surface that is only published (the site as it is
    /// deployed) is not served, and a surface whose feature this process does not have is
    /// not there at all. Both narrowings are filters over the same value, so nothing can
    /// be served that was not discovered.
    ///
    /// ```
    /// use majordomus_cli::web::discover::{self, Runtime};
    /// let all = discover::native(Runtime::full());
    /// let none = majordomus_cli::web::Topology::new(all).served(Runtime::default());
    /// assert!(!none.ids().contains(&"mcp"));
    /// ```
    pub fn served(&self, runtime: crate::web::discover::Runtime) -> Topology {
        Topology::new(
            self.surfaces
                .iter()
                .filter(|s| s.served_by(runtime))
                .cloned()
                .collect(),
        )
    }

    /// The topology as a publication holds it: every surface that contributes files.
    ///
    /// Contributing files is stricter than being published: a surface that declares itself
    /// publishable and names no directory is dropped here rather than carried as an empty
    /// mount, so a deployment cannot claim a path it has nothing to serve at.
    ///
    /// ```
    /// use majordomus_cli::web::{discover, Topology};
    /// // the routes the executable computes have no directory behind them, so a
    /// // publication of nothing but them carries nothing at all
    /// let native = Topology::new(discover::native(discover::Runtime::full()));
    /// assert!(!native.ids().is_empty());
    /// assert!(native.published().ids().is_empty());
    /// ```
    pub fn published(&self) -> Topology {
        Topology::new(
            self.surfaces
                .iter()
                .filter(|s| s.publishes())
                .cloned()
                .collect(),
        )
    }

    /// The public surfaces of one category, in precedence order: what a person is shown.
    ///
    /// This is the only reading that drops a surface for being unlisted, and it is a page's
    /// question rather than a router's: an internal surface is served and introspectable
    /// and simply not offered to a reader.
    ///
    /// ```
    /// use majordomus_cli::web::{discover, model::Category, Topology};
    /// let t = Topology::new(discover::native(discover::Runtime::full()));
    /// let api: Vec<&str> = t
    ///     .public_in(Category::Api)
    ///     .iter()
    ///     .map(|s| s.id.as_str())
    ///     .collect();
    /// assert!(api.contains(&"openapi"), "the document a reader can open is offered");
    /// // the wire protocols are served and unadvertised, so a listing shows none of them
    /// assert!(t.public_in(Category::Protocol).is_empty());
    /// ```
    pub fn public_in(&self, category: Category) -> Vec<&Surface> {
        self.surfaces
            .iter()
            .filter(|s| s.visibility == Visibility::Public && s.category == category)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(id: &str, mount: &str, category: Category, visibility: Visibility) -> Surface {
        Surface {
            id: id.into(),
            title: format!("the {id}"),
            category,
            visibility,
            kind: SurfaceKind::NativeRoute,
            mount: Mount::parse(mount).unwrap(),
            producer: "test".into(),
            feature: None,
            artifact: None,
            index: None,
            availability: Availability::ServedOnly,
            built_from: None,
            provenance: BTreeMap::new(),
        }
    }

    #[test]
    fn every_vocabulary_word_has_exactly_one_spelling() {
        // the words reach a listing, a JSON document and a page; a variant whose Display
        // and whose serialisation disagree is a value that means two things
        for (value, word) in [
            (SurfaceKind::StaticDirectory, "static"),
            (SurfaceKind::NativeRoute, "native"),
            (SurfaceKind::Redirect, "redirect"),
        ] {
            assert_eq!(value.to_string(), word);
        }
        for category in Category::ALL {
            assert_eq!(
                serde_json::to_value(category).unwrap(),
                serde_json::Value::String(category.to_string()),
                "{category}"
            );
            assert!(!category.title().is_empty());
        }
        assert_eq!(Visibility::Public.to_string(), "public");
        assert_eq!(Visibility::Internal.to_string(), "internal");
        assert_eq!(Feature::Mcp.to_string(), "mcp");
        assert_eq!(Feature::Cockpit.to_string(), "cockpit");
    }

    #[test]
    fn provenance_says_where_a_value_came_from_in_words() {
        assert_eq!(Provenance::Registry.to_string(), "capability registry");
        assert_eq!(Provenance::Default.to_string(), "default");
        assert_eq!(
            Provenance::ProducerDeclaration {
                path: "target/web/x/surface.json".into()
            }
            .to_string(),
            "producer declaration target/web/x/surface.json"
        );
        assert_eq!(
            Provenance::Filesystem {
                path: "target/web/x".into()
            }
            .to_string(),
            "filesystem target/web/x"
        );
        assert_eq!(
            Provenance::SiteConfig {
                path: "site/config.toml".into()
            }
            .to_string(),
            "site config site/config.toml"
        );
    }

    #[test]
    fn a_mount_round_trips_through_its_serialised_form() {
        let mount = Mount::parse("/docs").unwrap();
        let text = serde_json::to_string(&mount).unwrap();
        assert_eq!(text, "\"/docs\"");
        assert_eq!(serde_json::from_str::<Mount>(&text).unwrap(), mount);
        assert!(serde_json::from_str::<Mount>("\"/a/../b\"").is_err());
        assert_eq!(String::from(mount.clone()), "/docs");
        assert_eq!(mount.to_string(), "/docs");
        assert_eq!(Mount::try_from("/docs/".to_string()).unwrap(), mount);
    }

    #[test]
    fn availability_is_the_two_worlds_and_nothing_else() {
        assert!(Availability::Both.is_served() && Availability::Both.is_published());
        assert!(Availability::ServedOnly.is_served() && !Availability::ServedOnly.is_published());
        assert!(
            !Availability::PublishedOnly.is_served() && Availability::PublishedOnly.is_published()
        );
    }

    #[test]
    fn a_selection_narrows_by_id_and_leaves_the_order_alone() {
        let topology = Topology::new(vec![
            surface("home", "/", Category::Interface, Visibility::Public),
            surface("api", "/api/v1", Category::Api, Visibility::Public),
            surface("mcp", "/mcp", Category::Protocol, Visibility::Internal),
        ]);
        assert_eq!(topology.ids(), vec!["api", "mcp", "home"]);
        assert_eq!(topology.get("mcp").map(|s| s.id.as_str()), Some("mcp"));
        assert!(topology.get("nothing").is_none());

        let only = topology.select(&["api".to_string()], &[]);
        assert_eq!(only.ids(), vec!["api"]);
        let without = topology.select(&[], &["api".to_string()]);
        assert_eq!(without.ids(), vec!["mcp", "home"]);
        // an empty selector is every surface, not none
        assert_eq!(topology.select(&[], &[]).ids(), topology.ids());
    }

    #[test]
    fn a_person_is_shown_the_public_surfaces_of_a_category_and_no_others() {
        let topology = Topology::new(vec![
            surface("home", "/", Category::Interface, Visibility::Public),
            surface("api", "/api/v1", Category::Api, Visibility::Public),
            surface("mcp", "/mcp", Category::Protocol, Visibility::Internal),
        ]);
        assert_eq!(
            topology
                .public_in(Category::Interface)
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            vec!["home"]
        );
        assert!(
            topology.public_in(Category::Protocol).is_empty(),
            "an internal surface is served and not advertised"
        );
        assert!(topology.public_in(Category::Report).is_empty());
    }

    #[test]
    fn the_published_world_holds_only_what_contributes_files() {
        let mut app = surface("app", "/", Category::Documentation, Visibility::Public);
        app.kind = SurfaceKind::StaticDirectory;
        app.availability = Availability::PublishedOnly;
        app.artifact = Some(PathBuf::from("site/public"));
        let topology = Topology::new(vec![
            app,
            surface("home", "/", Category::Interface, Visibility::Public),
        ]);
        assert_eq!(topology.published().ids(), vec!["app"]);
        assert_eq!(
            topology.served(crate::web::discover::Runtime::full()).ids(),
            vec!["home"]
        );
        // a static surface that names no directory publishes nothing, whatever it says
        let mut nameless = topology.get("app").unwrap().clone();
        nameless.artifact = None;
        assert!(!nameless.publishes());
    }
}
