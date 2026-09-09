//! Discovery: how the repository's web surfaces are found without anybody listing them.
//!
//! Three inference sources, none of them a register a person maintains:
//!
//! 1. the **executable's own routes**, from the constants that already declare them once —
//!    the capability prefix, the OpenAPI document, the Swagger UI, MCP and the Cockpit;
//! 2. the **generated web root**, `target/web/<id>/`, where a producer writes its output
//!    and a `surface.json` beside it declaring the intent no directory walk can infer;
//! 3. the **site configuration**, which says the Zola application exists and where it is
//!    built, so the application is discovered rather than assumed.
//!
//! Everything a consumer can be surprised by carries its [`Provenance`], so `web explain`
//! answers "why is this here?" from data rather than from this module's source.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::model::{
    Availability, Category, Feature, Mount, Provenance, Surface, SurfaceKind, Topology, Visibility,
};
use crate::capability::model::HttpExposure;
use crate::cockpit;
use crate::error::{Error, Result};
use crate::http::{mcp, swagger};

/// The generated root every static producer writes into, repository-relative.
///
/// One directory per surface, owned by its producer alone: two generators never write into
/// one tree, so a rebuild of one cannot delete the other's output.
pub const GENERATED_ROOT: &str = "target/web";

/// The contract of a producer's declaration.
pub const DECLARATION_SCHEMA: &str = "web-surface/v1";

/// The file a producer writes beside its output to declare what it is.
pub const DECLARATION_FILE: &str = "surface.json";

/// What the running executable offers, which changes what it can answer for.
///
/// The static surfaces do not depend on it; the native ones do, because a process that
/// serves no MCP endpoint must not advertise one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Runtime {
    /// This process answers MCP over HTTP.
    pub mcp: bool,
    /// This process serves the Cockpit.
    pub cockpit: bool,
}

impl Runtime {
    /// Everything the executable can offer: what `web list` describes when it is not the
    /// server itself, so a listing shows the topology rather than one process's slice.
    pub fn full() -> Self {
        Runtime {
            mcp: true,
            cockpit: true,
        }
    }

    /// Does this process have the capability a surface needs?
    ///
    /// ```
    /// use majordomus_cli::web::discover::Runtime;
    /// use majordomus_cli::web::model::Feature;
    /// assert!(Runtime::full().has(Feature::Cockpit));
    /// assert!(!Runtime::default().has(Feature::Cockpit));
    /// ```
    pub fn has(self, feature: Feature) -> bool {
        match feature {
            Feature::Mcp => self.mcp,
            Feature::Cockpit => self.cockpit,
        }
    }
}

/// A producer's declaration: the intent that no walk of its output can infer.
///
/// `id` and `mount` are the intent. Everything else has a documented default, because a
/// field a producer must fill in to say what the convention already says is a field that
/// will disagree with reality one day.
///
/// ```
/// use majordomus_cli::web::discover::Declaration;
/// let d: Declaration = serde_json::from_str(r#"{
///     "schema": "web-surface/v1", "id": "tests", "mount": "/tests"
/// }"#).unwrap();
/// assert_eq!(d.id, "tests");
/// assert_eq!(d.index.as_deref(), None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// The contract this declaration follows.
    pub schema: String,
    /// The surface's identity, which is also its selector.
    pub id: String,
    /// Where the producer intends its output to be mounted.
    pub mount: String,
    /// One line for a listing; the id when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The file the mount itself answers with; `index.html` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    /// What produced the directory, for a reader who has to rebuild it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
    /// Whether the surface is published, served, or both; both when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability: Option<Availability>,
    /// What the surface is for; a generated directory is a report when it says nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    /// Whether a person is shown it; public when absent, because a producer that went to
    /// the trouble of declaring a mount meant it to be found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    /// The revision the output was built from, when the producer knows it: what makes a
    /// stale artifact a finding instead of a surprise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built_from: Option<String>,
}

/// The whole topology of a repository: the executable's routes, the generated reports and
/// the application.
///
/// Discovery never fails on an absent producer — a report that has not been generated is
/// simply not a surface yet, and [`super::validate`] is where a *required* artifact being
/// missing becomes a finding.
pub fn discover(root: &Path, runtime: Runtime) -> Result<Topology> {
    let mut surfaces = native(runtime);
    surfaces.extend(application(root));
    surfaces.extend(generated(root)?);
    Ok(Topology::new(surfaces))
}

/// The routes the executable answers itself, from the constants that declare them.
///
/// This function names no path of its own: every mount below is the same constant the
/// router, the OpenAPI document and the Cockpit already use, so a prefix that moves moves
/// here too. It is the whole list — a process that does not offer a feature simply does
/// not serve the surfaces that declare it, which [`Runtime`] decides and nothing here
/// repeats.
pub fn native_all() -> Vec<Surface> {
    vec![
        Surface {
            id: HOME.into(),
            title: "This process, and everything it serves".into(),
            category: Category::Interface,
            visibility: Visibility::Public,
            kind: SurfaceKind::NativeRoute,
            mount: Mount::root(),
            producer: "web::home".into(),
            feature: None,
            artifact: None,
            index: None,
            availability: Availability::ServedOnly,
            built_from: None,
            provenance: provenance([("mount", Provenance::Default)]),
        },
        Surface {
            id: "api".into(),
            title: "The capability registry over HTTP".into(),
            category: Category::Api,
            visibility: Visibility::Public,
            kind: SurfaceKind::NativeRoute,
            mount: Mount::parse(HttpExposure::PREFIX.trim_end_matches('/'))
                .expect("the capability prefix is a mount"),
            producer: "capability registry".into(),
            feature: None,
            artifact: None,
            index: None,
            availability: Availability::ServedOnly,
            built_from: None,
            provenance: provenance([
                ("mount", Provenance::Registry),
                ("kind", Provenance::Registry),
            ]),
        },
        Surface {
            id: "openapi".into(),
            title: "The OpenAPI document of the capability registry".into(),
            category: Category::Api,
            visibility: Visibility::Public,
            kind: SurfaceKind::NativeRoute,
            mount: Mount::parse(swagger::SPEC_PATH).expect("the OpenAPI path is a mount"),
            producer: "capability registry".into(),
            feature: None,
            artifact: None,
            index: None,
            // the running server renders it per request and `scripts/site-build` copies the
            // committed document into the publication at the same mount, so a published
            // page may link it — the one native route of which that is true
            availability: Availability::Both,
            built_from: None,
            provenance: provenance([("mount", Provenance::Registry)]),
        },
        Surface {
            id: "swagger".into(),
            title: "Swagger UI over the OpenAPI document".into(),
            category: Category::Documentation,
            visibility: Visibility::Public,
            kind: SurfaceKind::NativeRoute,
            mount: Mount::parse(swagger::SWAGGER_PATH).expect("the Swagger path is a mount"),
            producer: "http::swagger".into(),
            feature: None,
            artifact: None,
            index: None,
            availability: Availability::ServedOnly,
            built_from: None,
            provenance: provenance([("mount", Provenance::Registry)]),
        },
        Surface {
            id: "mcp".into(),
            title: "MCP over HTTP for attached clients".into(),
            category: Category::Protocol,
            visibility: Visibility::Internal,
            kind: SurfaceKind::NativeRoute,
            mount: Mount::parse(mcp::PATH).expect("the MCP path is a mount"),
            producer: "mcp endpoint".into(),
            feature: Some(Feature::Mcp),
            artifact: None,
            index: None,
            availability: Availability::ServedOnly,
            built_from: None,
            provenance: provenance([("mount", Provenance::Registry)]),
        },
        Surface {
            id: "events".into(),
            title: "The live channel: what this process's executions are doing".into(),
            category: Category::Protocol,
            visibility: Visibility::Internal,
            kind: SurfaceKind::NativeRoute,
            mount: Mount::parse(crate::http::events::PATH).expect("the events path is a mount"),
            producer: "http::events".into(),
            feature: None,
            artifact: None,
            index: None,
            availability: Availability::ServedOnly,
            built_from: None,
            provenance: provenance([("mount", Provenance::Registry)]),
        },
        Surface {
            id: "cockpit".into(),
            title: "The registry, rendered for a person".into(),
            category: Category::Interface,
            visibility: Visibility::Public,
            kind: SurfaceKind::NativeRoute,
            mount: Mount::parse(cockpit::PREFIX).expect("the Cockpit prefix is a mount"),
            producer: "cockpit".into(),
            feature: Some(Feature::Cockpit),
            artifact: None,
            index: None,
            availability: Availability::ServedOnly,
            built_from: None,
            provenance: provenance([("mount", Provenance::Registry)]),
        },
    ]
}

/// The identity of the surface that answers `/`: the page this process is entered through.
pub const HOME: &str = "home";

/// The identity of the served documentation build, mounted under [`DOCS_MOUNT`].
pub const DOCS: &str = "docs";

/// The identity of the site as it is published, which owns `/` of a deployment.
pub const APPLICATION: &str = "app";

/// Where the documentation is served by a running process, and the base URL its build is
/// made for. `/` belongs to the process's own home page, so the site is mounted below it.
pub const DOCS_MOUNT: &str = "/docs";

/// Where the documentation build for [`DOCS_MOUNT`] is written, repository-relative.
pub const DOCS_ARTIFACT: &str = "target/web/docs";

/// The site's own configuration, repository-relative: what says this repository has a site
/// at all.
pub const SITE_CONFIG: &str = "site/config.toml";

/// Where the site generator writes the build made for deployment, repository-relative.
pub const SITE_PUBLIC: &str = "site/public";

/// A mount whose meaning is fixed: the role it serves, the path it is at, and the id of the
/// surface that must hold it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reserved {
    /// What the mount is for, in one word; the key the generated topology publishes it under.
    pub role: &'static str,
    /// The path, exactly as the constant that declares it carries it.
    pub path: &'static str,
    /// The surface that must own it.
    pub owner: &'static str,
}

/// The mounts this repository reserves.
///
/// A path is a name, and this repository has already paid once for a name meaning two
/// things: the Swagger UI held `/docs` because the word was free, and the day documentation
/// arrived the obvious path was occupied. These pairs are that decision written down where
/// something reads it — the validator refuses a topology that breaks one, the generated
/// topology publishes them, and the documentation renders what is published rather than
/// restating it.
///
/// Each path is the same constant its surface is declared with, so a mount that moves moves
/// here and the reservation follows it.
///
/// ```
/// use majordomus_cli::web::discover::reserved;
/// let docs = reserved().into_iter().find(|r| r.role == "documentation").unwrap();
/// assert_eq!(docs.path, "/docs");
/// assert_eq!(docs.owner, "docs");
/// let swagger = reserved().into_iter().find(|r| r.role == "swagger").unwrap();
/// assert_eq!(swagger.path, "/swagger");
/// ```
pub fn reserved() -> Vec<Reserved> {
    vec![
        Reserved {
            role: "home",
            path: "/",
            owner: HOME,
        },
        Reserved {
            role: "documentation",
            path: DOCS_MOUNT,
            owner: DOCS,
        },
        Reserved {
            role: "swagger",
            path: swagger::SWAGGER_PATH,
            owner: "swagger",
        },
        Reserved {
            role: "openapi",
            path: swagger::SPEC_PATH,
            owner: "openapi",
        },
        Reserved {
            role: "capabilities",
            path: HttpExposure::PREFIX,
            owner: "api",
        },
        Reserved {
            role: "events",
            path: crate::http::events::PATH,
            owner: "events",
        },
    ]
}

/// The routes the executable answers itself, narrowed to what this process offers.
pub fn native(runtime: Runtime) -> Vec<Surface> {
    native_all()
        .into_iter()
        .filter(|s| s.feature.is_none_or(|f| runtime.has(f)))
        .collect()
}

/// The site, when this repository has one: the deployment and the local build of it.
///
/// One configuration, two surfaces, because the same source is built twice for two
/// different mounts and neither build is a copy of the source. The deployment owns `/` of
/// whatever origin it is published to and is never served by this process; the local build
/// is made for [`DOCS_MOUNT`] and is never published. Nothing about the site is assumed:
/// its existence is read from its own configuration, and a repository without one has
/// neither surface.
pub fn application(root: &Path) -> Vec<Surface> {
    let config = root.join(SITE_CONFIG);
    if !config.is_file() {
        return Vec::new();
    }
    let from_config = || Provenance::SiteConfig {
        path: SITE_CONFIG.into(),
    };
    vec![
        Surface {
            id: APPLICATION.into(),
            title: "The site as it is deployed".into(),
            category: Category::Documentation,
            visibility: Visibility::Public,
            kind: SurfaceKind::StaticDirectory,
            mount: Mount::root(),
            producer: "scripts/site-build".into(),
            feature: None,
            artifact: Some(PathBuf::from(SITE_PUBLIC)),
            index: Some("index.html".into()),
            availability: Availability::PublishedOnly,
            built_from: None,
            provenance: provenance([
                ("kind", from_config()),
                ("mount", Provenance::Default),
                ("artifact", from_config()),
            ]),
        },
        Surface {
            id: DOCS.into(),
            title: "The documentation, as this process serves it".into(),
            category: Category::Documentation,
            visibility: Visibility::Public,
            kind: SurfaceKind::StaticDirectory,
            mount: Mount::parse(DOCS_MOUNT).expect("the documentation mount is a mount"),
            producer: "scripts/site-build --serve".into(),
            feature: None,
            artifact: Some(PathBuf::from(DOCS_ARTIFACT)),
            index: Some("index.html".into()),
            availability: Availability::ServedOnly,
            built_from: built_from(&root.join(DOCS_ARTIFACT)),
            provenance: provenance([
                ("kind", from_config()),
                ("mount", Provenance::Default),
                (
                    "artifact",
                    Provenance::Filesystem {
                        path: DOCS_ARTIFACT.into(),
                    },
                ),
            ]),
        },
    ]
}

/// The revision a producer recorded beside its output, when it recorded one.
fn built_from(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(DECLARATION_FILE)).ok()?;
    let declaration: Declaration = serde_json::from_str(&text).ok()?;
    declaration.built_from
}

/// Every generated static surface: one directory under the generated root, each declaring
/// itself.
///
/// A directory without a declaration is not a surface — it is somebody's scratch space, and
/// serving it because it happened to be there is how a repository publishes what it did not
/// mean to.
pub fn generated(root: &Path) -> Result<Vec<Surface>> {
    let dir = root.join(GENERATED_ROOT);
    let mut entries: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(read) => read
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => return Ok(Vec::new()),
    };
    entries.sort();
    let mut out = Vec::new();
    for path in entries {
        let declaration = path.join(DECLARATION_FILE);
        if !declaration.is_file() {
            continue;
        }
        // the documentation build is discovered from the site's configuration, which knows
        // it should exist even when it has not been built; its declaration carries the
        // revision and nothing else this walk would add
        if path.file_name().is_some_and(|n| n == DOCS) {
            continue;
        }
        let rel = format!(
            "{GENERATED_ROOT}/{}/{DECLARATION_FILE}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        let text = std::fs::read_to_string(&declaration).map_err(|e| Error::InvalidSurface {
            surface: rel.clone(),
            reason: format!("cannot be read: {e}"),
        })?;
        let decl: Declaration = serde_json::from_str(&text).map_err(|e| Error::InvalidSurface {
            surface: rel.clone(),
            reason: format!("does not parse: {e}; the contract is {DECLARATION_SCHEMA}"),
        })?;
        if decl.schema != DECLARATION_SCHEMA {
            return Err(Error::InvalidSurface {
                surface: rel.clone(),
                reason: format!(
                    "declares schema '{}', and this executable reads {DECLARATION_SCHEMA}",
                    decl.schema
                ),
            });
        }
        let mount = Mount::parse(&decl.mount).map_err(|reason| Error::InvalidSurface {
            surface: decl.id.clone(),
            reason: format!("{reason} (declared in {rel})"),
        })?;
        let artifact = PathBuf::from(GENERATED_ROOT).join(path.file_name().ok_or_else(|| {
            Error::InvalidSurface {
                surface: decl.id.clone(),
                reason: format!("the directory of {rel} has no name"),
            }
        })?);
        out.push(Surface {
            id: decl.id.clone(),
            title: decl.title.unwrap_or_else(|| decl.id.clone()),
            category: decl.category.unwrap_or(Category::Report),
            visibility: decl.visibility.unwrap_or(Visibility::Public),
            kind: SurfaceKind::StaticDirectory,
            mount,
            producer: decl.producer.unwrap_or_else(|| "unknown".into()),
            feature: None,
            artifact: Some(artifact),
            index: Some(decl.index.unwrap_or_else(|| "index.html".into())),
            availability: decl.availability.unwrap_or(Availability::Both),
            built_from: decl.built_from,
            provenance: provenance([
                (
                    "mount",
                    Provenance::ProducerDeclaration { path: rel.clone() },
                ),
                ("kind", Provenance::Filesystem { path: rel.clone() }),
                ("artifact", Provenance::Filesystem { path: rel }),
            ]),
        });
    }
    Ok(out)
}

fn provenance<const N: usize>(pairs: [(&str, Provenance); N]) -> BTreeMap<String, Provenance> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(dir.join(name).join(DECLARATION_FILE), body).unwrap();
    }

    #[test]
    fn a_generated_directory_declaring_itself_is_a_surface() {
        let tmp = tempfile::tempdir().unwrap();
        let generated = tmp.path().join(GENERATED_ROOT);
        write(
            &generated,
            "example-report",
            r#"{"schema":"web-surface/v1","id":"example-report","mount":"/example-report","title":"Example"}"#,
        );
        let found = generated_surfaces(tmp.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "example-report");
        assert_eq!(found[0].mount.as_str(), "/example-report");
        assert_eq!(found[0].index.as_deref(), Some("index.html"));
    }

    #[test]
    fn a_directory_without_a_declaration_is_not_a_surface() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(GENERATED_ROOT).join("scratch")).unwrap();
        assert!(generated_surfaces(tmp.path()).is_empty());
    }

    #[test]
    fn an_unknown_schema_is_refused_rather_than_guessed_at() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join(GENERATED_ROOT),
            "future",
            r#"{"schema":"web-surface/v2","id":"future","mount":"/future"}"#,
        );
        let err = generated(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("web-surface/v1"), "{err}");
    }

    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join(GENERATED_ROOT),
            "typo",
            r#"{"schema":"web-surface/v1","id":"typo","mount":"/typo","moutn":"/oops"}"#,
        );
        assert!(generated(tmp.path()).is_err());
    }

    #[test]
    fn the_application_is_discovered_from_its_configuration() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(application(tmp.path()).is_empty());
        std::fs::create_dir_all(tmp.path().join("site")).unwrap();
        std::fs::write(tmp.path().join("site/config.toml"), "base_url = \"/\"\n").unwrap();
        let surfaces = application(tmp.path());
        let app = surfaces
            .iter()
            .find(|s| s.id == APPLICATION)
            .expect("a site is deployed");
        assert!(app.mount.is_root());
        assert_eq!(app.artifact.as_deref(), Some(Path::new(SITE_PUBLIC)));
        assert!(
            !app.availability.is_served(),
            "the deployment is not served"
        );
        let docs = surfaces
            .iter()
            .find(|s| s.id == DOCS)
            .expect("the same site is served under its own mount");
        assert_eq!(docs.mount.as_str(), DOCS_MOUNT);
        assert_eq!(docs.artifact.as_deref(), Some(Path::new(DOCS_ARTIFACT)));
        assert!(
            !docs.availability.is_published(),
            "the local build is never deployed"
        );
    }

    #[test]
    fn the_native_routes_come_from_the_constants_that_declare_them() {
        let all = native(Runtime::full());
        let mounts: Vec<&str> = all.iter().map(|s| s.mount.as_str()).collect();
        assert!(mounts.contains(&swagger::SWAGGER_PATH));
        assert!(mounts.contains(&mcp::PATH));
        assert!(mounts.contains(&cockpit::PREFIX));
        // a process that serves neither advertises neither
        let bare = native(Runtime::default());
        assert!(!bare.iter().any(|s| s.id == "mcp" || s.id == "cockpit"));
    }

    fn generated_surfaces(root: &Path) -> Vec<Surface> {
        generated(root).unwrap()
    }
}
