//! The typed model of a web surface: what it is, where it is mounted, what produced it,
//! and where every value came from.
//!
//! One vocabulary serves every consumer — the router, the static composition, the
//! validator, the listing and the generated manifest — so a surface is described once and
//! read many times. The kinds are behavioural rather than nominal: a test report and a
//! benchmark report are both [`SurfaceKind::StaticDirectory`] and differ only in data, and
//! nothing in this module knows the name of any surface this repository happens to have.

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

    /// The heading a person reads.
    ///
    /// ```
    /// use majordomus_cli::web::model::Category;
    /// assert_eq!(Category::Api.title(), "API");
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
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
    pub fn is_published(&self) -> bool {
        matches!(self, Availability::Both | Availability::PublishedOnly)
    }

    /// Does the running server answer for this surface?
    pub fn is_served(&self) -> bool {
        matches!(self, Availability::Both | Availability::ServedOnly)
    }
}

/// One resolved surface: everything a consumer needs, with the provenance of what it could
/// be surprised by.
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

    /// One surface by id.
    pub fn get(&self, id: &str) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    /// The topology narrowed to a selection: `only` when it is not empty, minus `exclude`.
    ///
    /// Selection is by discovered id and nothing else, so a surface that did not exist when
    /// this code was written is selectable the day it is discovered.
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
    pub fn public_in(&self, category: Category) -> Vec<&Surface> {
        self.surfaces
            .iter()
            .filter(|s| s.visibility == Visibility::Public && s.category == category)
            .collect()
    }
}
