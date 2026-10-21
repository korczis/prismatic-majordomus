//! The deployment objects of the layer, typed: `.ai/repo/deployments/*.yaml` parsed into
//! values that cannot hold nonsense, and validated against what this repository actually
//! contains before anything is built from them.
//!
//! Nothing here reaches the network or talks to a hosting provider. The question this
//! module answers is the local one — would this deployment work — and it is answered while
//! a test is cheap rather than after a rollout is not. The one process anything here starts
//! is git, read-only, when [`slug`] assembles the tree a provider receives from one commit;
//! pushing that tree is a workflow's, never this module's. The projections (the
//! image definition, its ignore file and the files of the provider the object names) read
//! the values this module produces; they never re-read the file.
//!
//! An object may name the hosting provider that runs it ([`Provider`], ADR 0219). Only a
//! provider the contract knows is accepted — today Gigalixir — and a name it does not know
//! is refused like any other value outside its type. Fly.io, which the object once named,
//! stays no target (owner order, 2026-10-12); the owner's order of 2026-10-21 put Prismatic
//! Majordomus on Gigalixir, derived rather than assembled by hand. A deployment that names
//! no provider is what it was: the image, published to GHCR and run by nobody. The
//! credential that authorises a push is never a field of the object.
//!
//! Every refusal names four things: the file, the key, the value observed and the
//! correction. A message that names fewer sends the reader looking.

use std::collections::BTreeSet;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Object;

pub(crate) mod observed;
pub mod render;
pub mod slug;

/// The kind a deployment object is discovered as.
pub const KIND: &str = "deployment";

/// The only format version this executable reads.
pub const SCHEMA_VERSION: &str = "deployment/v1";

/// Why a deployment object is refused: what was read, where, and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Refusal {
    /// The repository-relative file the value came from.
    pub file: String,
    /// The key path within it, as the contract names it.
    pub key: String,
    /// The value observed, rendered as it was read.
    pub found: String,
    /// What is wrong with it, in one line.
    pub problem: String,
    /// What to do instead.
    pub correction: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {} is {}: {}; {}",
            self.file, self.key, self.found, self.problem, self.correction
        )
    }
}

impl Refusal {
    fn new(
        file: &str,
        key: &str,
        found: impl std::fmt::Display,
        problem: &str,
        correction: &str,
    ) -> Self {
        Refusal {
            file: file.into(),
            key: key.into(),
            found: found.to_string(),
            problem: problem.into(),
            correction: correction.into(),
        }
    }
}

// ---------------------------------------------------------------- the scalars
//
// Each of these refuses its own nonsense at construction, so no later stage has to ask
// whether the number it holds is a number the deployment could survive.

/// A TCP port a deployed process may bind. Privileged ports are not among them: the
/// process runs as a non-root user and could not bind one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Port(u16);

impl Port {
    /// The lowest port an unprivileged process may bind.
    pub const LOWEST: u32 = 1024;

    /// The port, or why this number is not one.
    ///
    /// ```
    /// use majordomus_cli::deploy::Port;
    /// assert!(Port::new(8080).is_ok());
    /// assert!(Port::new(80).is_err());
    /// assert!(Port::new(70000).is_err());
    /// ```
    pub fn new(n: u32) -> Result<Self, String> {
        if !(Self::LOWEST..=65535).contains(&n) {
            return Err(format!(
                "{n} is outside {}-65535; the deployed process runs unprivileged and cannot bind a lower port",
                Self::LOWEST
            ));
        }
        Ok(Port(n as u16))
    }

    /// The number.
    pub fn get(self) -> u16 {
        self.0
    }
}

impl std::fmt::Display for Port {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for Port {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Port::new(u32::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// A count that is at least one: memory in megabytes, CPUs, machines. Zero is refused
/// wherever zero would mean "a deployment that cannot run".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Positive(u32);

impl Positive {
    /// The value, or why zero is not one.
    ///
    /// ```
    /// use majordomus_cli::deploy::Positive;
    /// assert_eq!(Positive::new(256).unwrap().get(), 256);
    /// assert!(Positive::new(0).is_err());
    /// ```
    pub fn new(n: u32) -> Result<Self, String> {
        if n == 0 {
            return Err("0, and a deployment cannot run on none of it".into());
        }
        Ok(Positive(n))
    }

    /// The number.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for Positive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for Positive {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Positive::new(u32::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// A route a platform polls: an absolute path on this service, never a URL. The host is
/// the deployment's to know and not the object's to state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Route(String);

impl Route {
    /// The route, or why this string is not one.
    ///
    /// ```
    /// use majordomus_cli::deploy::Route;
    /// assert!(Route::new("/api/v1/live").is_ok());
    /// assert!(Route::new("https://example.invalid/live").is_err());
    /// ```
    pub fn new(s: &str) -> Result<Self, String> {
        if !s.starts_with('/') {
            return Err(format!(
                "'{s}', which is not a path beginning with /; a health route names a path on this service, not a URL"
            ));
        }
        if s.contains("//") || s.contains('?') || s.contains('#') || s.contains(' ') {
            return Err(format!(
                "'{s}', which carries an empty segment, a query, a fragment or a space"
            ));
        }
        Ok(Route(s.into()))
    }

    /// The path.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Route {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Route::new(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Which interface a process listens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Interface {
    /// Reachable from this host alone: the local default, and what every local invocation
    /// keeps.
    Loopback,
    /// Every interface. A hosted process needs it and a local one never does; stating it
    /// is the intent that replaces suppressing the bind warning.
    All,
}

impl Interface {
    /// The address a process binds for this interface.
    ///
    /// ```
    /// use majordomus_cli::deploy::Interface;
    /// assert_eq!(Interface::Loopback.host(), "127.0.0.1");
    /// assert_eq!(Interface::All.host(), "0.0.0.0");
    /// ```
    pub fn host(self) -> &'static str {
        match self {
            Interface::Loopback => "127.0.0.1",
            Interface::All => "0.0.0.0",
        }
    }
}

/// What a machine's CPU is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CpuKind {
    /// A fraction of a core: the cheap profile.
    Shared,
    /// A dedicated one.
    Performance,
}

/// Where a deployment stands.
///
/// `Status` is the right name inside this module; the schema component namespace is flat,
/// and `distribution::Status` answers a different question, so each says which it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "DeploymentStatus")]
pub enum Status {
    /// The object exists and nothing is deployed from it yet.
    Declared,
    /// It is deployed.
    Active,
    /// It was, and the object is kept for the record.
    Retired,
}

// ---------------------------------------------------------------- the object

/// What is shipped and what it is built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Build {
    /// The Cargo package that is built.
    pub package: String,
    /// The binary target the image runs.
    pub binary: String,
    /// The Cargo profile the image is built under.
    pub profile: String,
    /// The repository-relative paths the build context carries, and the only ones.
    pub inputs: Vec<String>,
    /// The directory the canonical site pipeline writes; absent means the image serves no
    /// site.
    #[serde(default)]
    pub site: Option<String>,
}

/// The address the process listens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Listen {
    /// The port, stated once for the process and the image.
    pub port: Port,
    /// Which interface.
    pub interface: Interface,
}

impl Listen {
    /// `host:port` as the deployed process binds it.
    ///
    /// ```
    /// use majordomus_cli::deploy::{Interface, Listen, Port};
    /// let l = Listen { port: Port::new(8080).unwrap(), interface: Interface::All };
    /// assert_eq!(l.address(), "0.0.0.0:8080");
    /// ```
    pub fn address(&self) -> String {
        format!("{}:{}", self.interface.host(), self.port)
    }
}

/// The routes a platform polls, and how often.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthRoutes {
    /// Is this process alive.
    pub liveness: Route,
    /// Can this process serve traffic.
    pub readiness: Route,
    /// How long the platform waits before the first check counts.
    #[serde(default)]
    pub grace_seconds: Option<u32>,
    /// How often the platform polls liveness.
    #[serde(default)]
    pub interval_seconds: Option<Positive>,
    /// How long one check may take before it counts as failed.
    #[serde(default)]
    pub timeout_seconds: Option<Positive>,
}

/// What one machine is granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    /// Shared or dedicated.
    pub cpu_kind: CpuKind,
    /// How many CPUs one machine has.
    pub cpus: Positive,
    /// Memory per machine, in megabytes.
    pub memory_mb: Positive,
}

/// How many machines run, and whether the platform may stop them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Machines {
    /// How many machines the application has.
    pub count: Positive,
    /// How many stay running when idle.
    pub min_running: u32,
    /// Whether a request to a stopped machine starts it.
    pub autostart: bool,
    /// Whether an idle machine is stopped.
    pub autostop: bool,
}

/// The measured figures this deployment is held to. Each is written by the run that
/// measured it; an absent one has not been measured yet and is not a licence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    /// Largest accepted compressed image size.
    #[serde(default)]
    pub image_bytes: Option<Positive>,
    /// Largest accepted stripped binary size.
    #[serde(default)]
    pub binary_bytes: Option<Positive>,
    /// Largest accepted build context.
    #[serde(default)]
    pub build_context_bytes: Option<Positive>,
    /// Longest accepted time from a stopped machine to a served response.
    #[serde(default)]
    pub cold_start_ms: Option<Positive>,
    /// Largest accepted resident set under load.
    #[serde(default)]
    pub resident_memory_mb: Option<Positive>,
    /// Longest accepted time for one liveness or readiness check.
    #[serde(default)]
    pub blocking_check_ms: Option<Positive>,
    /// Longest accepted p99 for a served request.
    #[serde(default)]
    pub request_p99_ms: Option<Positive>,
}

/// A hosting provider the contract knows. A name outside this list is refused at parse time:
/// a provider whose files nothing renders would be a block nothing reads.
///
/// ```
/// use majordomus_cli::deploy::ProviderName;
///
/// let name: ProviderName = serde_json::from_value(serde_json::json!("gigalixir")).unwrap();
/// assert_eq!(name, ProviderName::Gigalixir);
/// assert_eq!(name.as_str(), "gigalixir");
/// assert!(serde_json::from_value::<ProviderName>(serde_json::json!("fly")).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderName {
    /// Gigalixir: a Heroku-compatible platform that runs a Procfile over a buildpack.
    Gigalixir,
}

impl ProviderName {
    /// The name as the object spells it, and the directory its files are rendered under
    /// (`deploy/<name>/`).
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderName::Gigalixir => "gigalixir",
        }
    }
}

impl std::fmt::Display for ProviderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The hosting provider that runs the deployment, and the facts that are the provider's
/// alone. Every file the provider reads is rendered from this block and the generic keys
/// (see [`render`]); its credential is never one of these fields.
///
/// ```
/// use majordomus_cli::deploy::{Provider, ProviderName};
///
/// let p: Provider = serde_json::from_value(serde_json::json!({
///     "name": "gigalixir",
///     "app": "prismatic-majordomus",
///     "stack": "gigalixir-24",
///     "url": "https://prismatic-majordomus.gigalixirapp.com",
/// }))
/// .unwrap();
/// assert_eq!(p.name, ProviderName::Gigalixir);
/// assert_eq!(p.app, "prismatic-majordomus");
///
/// // a key the provider block does not have is refused, a token above all
/// assert!(serde_json::from_value::<Provider>(serde_json::json!({
///     "name": "gigalixir", "app": "a-b-c", "stack": "gigalixir-24",
///     "url": "https://a-b-c.gigalixirapp.com", "api_key": "x",
/// }))
/// .is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    /// Which provider.
    pub name: ProviderName,
    /// The application's name at the provider.
    pub app: String,
    /// The provider's stack the application runs on (`gigalixir-24`).
    pub stack: String,
    /// The public URL the deployment answers on; always `https://`.
    pub url: String,
}

/// A deployment is presented by its id, which is its identity within the repository; the
/// title is prose and two deployments may well be given the same one.
impl crate::order::Ordered for Deployment {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id)
    }
}

/// One deployment of this repository's executable, as the canonical object states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    /// The format version; a version this executable does not read is refused.
    pub schema: String,
    /// Always `deployment`.
    pub kind: String,
    /// The identity within the repository.
    pub id: String,
    /// One line naming it.
    pub title: String,
    /// One line: what it serves and to whom.
    #[serde(default)]
    pub description: Option<String>,
    /// Where it stands.
    #[serde(default)]
    pub status: Option<Status>,
    /// The application's name: what the deployment is known by in a listing.
    pub application: String,
    /// What is shipped and what it is built from.
    pub build: Build,
    /// The address the process listens on.
    pub listen: Listen,
    /// The routes a platform polls.
    pub health: HealthRoutes,
    /// What one machine is granted.
    pub resources: Resources,
    /// How many machines run.
    pub machines: Machines,
    /// The region they run in.
    pub region: String,
    /// The measured figures it is held to.
    #[serde(default)]
    pub budgets: Budgets,
    /// The hosting provider that runs it, when one does; absent, nobody hosts it.
    #[serde(default)]
    pub provider: Option<Provider>,
    /// When the object was written.
    #[serde(default)]
    pub created_at: Option<String>,
    /// When it was last changed.
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ---------------------------------------------------------------- reading and checking

/// What the local checks need to know about the repository they are deciding against.
pub struct Workspace<'a> {
    /// The repository root.
    pub root: &'a Path,
    /// Every HTTP path the capability registry serves.
    pub routes: BTreeSet<String>,
    /// Every Cargo package in the workspace, with the binaries it produces.
    pub packages: Vec<(String, Vec<String>)>,
}

impl<'a> Workspace<'a> {
    /// The workspace as this process sees it: the routes from the registry it built, the
    /// packages from the manifests the repository tracks. Reading the registry that
    /// already exists is the point; a second list of routes would be the defect.
    pub fn read(root: &'a Path, routes: BTreeSet<String>) -> Self {
        Workspace {
            root,
            routes,
            packages: packages(root),
        }
    }
}

/// The Cargo packages under `apps/`, each with the binaries it declares. Read with the
/// same subset discipline the rest of the executable reads YAML with: the two keys that
/// matter, taken literally, with no TOML dependency for the sake of two of them.
fn packages(root: &Path) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root.join("apps")) else {
        return out;
    };
    let mut dirs: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    dirs.sort();
    for dir in dirs {
        let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let mut package = None;
        let mut bins = Vec::new();
        let mut section = "";
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                section = if line == "[package]" {
                    "package"
                } else if line == "[[bin]]" {
                    "bin"
                } else {
                    ""
                };
                continue;
            }
            let Some(value) = line
                .strip_prefix("name")
                .and_then(|r| r.trim().strip_prefix('='))
            else {
                continue;
            };
            let value = value.trim().trim_matches('"').to_string();
            match section {
                "package" if package.is_none() => package = Some(value),
                "bin" => bins.push(value),
                _ => {}
            }
        }
        if let Some(package) = package {
            if bins.is_empty() {
                bins.push(package.clone());
            }
            out.push((package, bins));
        }
    }
    out
}

impl Deployment {
    /// Parse one indexed object into a deployment, or say why it is not one. The object's
    /// metadata is the whole YAML file, so this is where an unknown key, a missing field
    /// and a value outside its type are all refused at once.
    pub fn parse(object: &Object) -> Result<Deployment, Refusal> {
        Deployment::from_metadata(&object.provenance.path, object.metadata.clone())
    }

    /// Parse the whole YAML file of a deployment, already read into a value, as
    /// [`Deployment::parse`] does for an indexed object. The slug assembler reads the object
    /// at a commit rather than from the index ([`slug`]), and the two must refuse alike.
    pub(crate) fn from_metadata(
        file: &str,
        metadata: serde_json::Value,
    ) -> Result<Deployment, Refusal> {
        let parsed: Deployment = serde_json::from_value(metadata).map_err(|e| {
            Refusal::new(
                file,
                "(the object)",
                e,
                "not a deployment this executable can read",
                "compare it with share/schemas/majordomus/deployment/deployment.v1.schema.json, or run: majordomus doctor",
            )
        })?;
        if parsed.schema != SCHEMA_VERSION {
            return Err(Refusal::new(
                file,
                "schema",
                &parsed.schema,
                &format!(
                    "a format version this executable does not read; it reads {SCHEMA_VERSION}"
                ),
                "write the version this executable reads, or upgrade the executable",
            ));
        }
        if parsed.kind != KIND {
            return Err(Refusal::new(
                file,
                "kind",
                &parsed.kind,
                &format!("not {KIND}"),
                &format!("write `kind: {KIND}`"),
            ));
        }
        Ok(parsed)
    }

    /// Every local refusal this deployment earns, in the order the keys appear. An empty
    /// list is the whole verdict this module can give: what remains is decided where the
    /// image runs, and by then a refusal costs a rollout.
    pub fn check(&self, file: &str, ws: &Workspace<'_>) -> Vec<Refusal> {
        let mut out = Vec::new();

        // --- the routes a platform will poll must be routes something answers
        for (key, route) in [
            ("health.liveness", &self.health.liveness),
            ("health.readiness", &self.health.readiness),
        ] {
            if !ws.routes.contains(route.as_str()) {
                out.push(Refusal::new(
                    file,
                    key,
                    route,
                    "a route no capability registers; the platform would poll a 404 and stop the machine",
                    "name a registered route (majordomus capabilities list --format json), or declare the capability that serves it",
                ));
            }
        }

        // --- the package and the binary are workspace facts, not aspirations
        match ws.packages.iter().find(|(p, _)| *p == self.build.package) {
            None => out.push(Refusal::new(
                file,
                "build.package",
                &self.build.package,
                "a package this workspace does not contain",
                &format!(
                    "name one of: {}",
                    ws.packages
                        .iter()
                        .map(|(p, _)| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
            Some((_, bins)) if !bins.contains(&self.build.binary) => out.push(Refusal::new(
                file,
                "build.binary",
                &self.build.binary,
                &format!("not a binary {} produces", self.build.package),
                &format!("name one of: {}", bins.join(", ")),
            )),
            Some(_) => {}
        }

        // --- a build input that does not resolve makes an image out of nothing
        for (n, input) in self.build.inputs.iter().enumerate() {
            if input.starts_with('/') || input.split('/').any(|s| s == "..") {
                out.push(Refusal::new(
                    file,
                    &format!("build.inputs.{n}"),
                    input,
                    "not a path inside the repository",
                    "give a repository-relative path with no leading slash and no ..",
                ));
            } else if !ws.root.join(input).exists() {
                out.push(Refusal::new(
                    file,
                    &format!("build.inputs.{n}"),
                    input,
                    "a path this repository does not have",
                    "correct the path, or remove it from the build context",
                ));
            }
        }
        if let Some(site) = &self.build.site {
            if site.starts_with('/') || site.split('/').any(|s| s == "..") {
                out.push(Refusal::new(
                    file,
                    "build.site",
                    site,
                    "not a path inside the repository",
                    "give a repository-relative path with no leading slash and no ..",
                ));
            }
        }

        // --- more machines running than the application has is a bill nobody agreed to
        if self.machines.min_running > self.machines.count.get() {
            out.push(Refusal::new(
                file,
                "machines.min_running",
                self.machines.min_running,
                &format!(
                    "above machines.count ({}); the platform cannot keep more running than exist",
                    self.machines.count
                ),
                "lower min_running, or raise count",
            ));
        }

        // --- a hosted deployment that binds loopback is unreachable inside its machine; one
        // that names its provider is hosted by its own statement, so the refusal says which
        // provider could not reach it
        if self.listen.interface == Interface::Loopback {
            let problem = match &self.provider {
                Some(p) => format!(
                    "unreachable from outside the machine it runs in, and {} routes to every interface of it",
                    p.name
                ),
                None => "unreachable from outside the machine it runs in".to_string(),
            };
            out.push(Refusal::new(
                file,
                "listen.interface",
                "loopback",
                &problem,
                "state `interface: all`, which is what a hosted process means",
            ));
        }

        // --- the provider's public address is https, or a caller's traffic travels in clear
        if let Some(p) = &self.provider {
            if !p.url.starts_with("https://") {
                out.push(Refusal::new(
                    file,
                    "provider.url",
                    &p.url,
                    "not an https URL; a hosted deployment answers its callers over TLS",
                    "write the provider's https:// address, as the provider publishes it",
                ));
            }
        }

        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// The object this repository ships, as the parser sees it, with one key replaced.
    fn object_text() -> serde_json::Value {
        serde_json::json!({
            "schema": "deployment/v1",
            "kind": "deployment",
            "id": "example",
            "title": "The example deployment",
            "application": "example",
            "build": {
                "package": "majordomus-cli",
                "binary": "majordomus",
                "profile": "release",
                "inputs": ["apps/majordomus-cli"],
            },
            "listen": { "port": 8080, "interface": "all" },
            "health": { "liveness": "/api/v1/live", "readiness": "/api/v1/ready" },
            "resources": { "cpu_kind": "shared", "cpus": 1, "memory_mb": 256 },
            "machines": { "count": 1, "min_running": 0, "autostart": true, "autostop": true },
            "region": "fra",
        })
    }

    fn parse(v: serde_json::Value) -> Result<Deployment, String> {
        serde_json::from_value::<Deployment>(v).map_err(|e| e.to_string())
    }

    #[test]
    fn the_shipped_shape_parses() {
        let d = parse(object_text()).expect("the example object parses");
        assert_eq!(d.listen.port.get(), 8080);
        assert_eq!(d.listen.address(), "0.0.0.0:8080");
        assert_eq!(d.resources.memory_mb.get(), 256);
    }

    #[test]
    fn an_unknown_key_is_refused() {
        let mut v = object_text();
        v["fly_api_token"] = serde_json::json!("x");
        assert!(parse(v).unwrap_err().contains("fly_api_token"));
    }

    #[test]
    fn a_privileged_port_cannot_be_constructed() {
        let mut v = object_text();
        v["listen"]["port"] = serde_json::json!(80);
        assert!(parse(v).unwrap_err().contains("unprivileged"));
    }

    #[test]
    fn zero_memory_cannot_be_constructed() {
        let mut v = object_text();
        v["resources"]["memory_mb"] = serde_json::json!(0);
        assert!(parse(v).unwrap_err().contains("cannot run on none of it"));
    }

    #[test]
    fn a_health_route_that_is_a_url_cannot_be_constructed() {
        let mut v = object_text();
        v["health"]["liveness"] = serde_json::json!("https://example.invalid/live");
        assert!(parse(v)
            .unwrap_err()
            .contains("not a path beginning with /"));
    }

    /// One indexed object carrying `metadata`, which is all `parse` reads of it.
    fn object(metadata: serde_json::Value) -> crate::model::Object {
        crate::model::Object {
            kind: KIND.into(),
            identity: "example".into(),
            uri: "majordomus://deployment/example".into(),
            title: None,
            description: None,
            metadata,
            body: String::new(),
            content: String::new(),
            media_type: "application/yaml",
            provenance: crate::model::Provenance {
                path: ".ai/repo/deployments/example.yaml".into(),
                directory: ".ai/repo/deployments".into(),
                source_class: "deployment".into(),
                section: Some("deployments".into()),
                bytes: 0,
                member: None,
            },
        }
    }

    #[test]
    fn an_indexed_object_parses_and_names_its_own_file() {
        let d = Deployment::parse(&object(object_text())).expect("parses");
        assert_eq!(d.id, "example");
    }

    #[test]
    fn a_format_version_this_executable_does_not_read_is_refused() {
        let mut v = object_text();
        v["schema"] = serde_json::json!("deployment/v2");
        let r = Deployment::parse(&object(v)).expect_err("refused");
        assert_eq!(r.file, ".ai/repo/deployments/example.yaml");
        assert_eq!(r.key, "schema");
        assert_eq!(r.found, "deployment/v2");
        assert!(r.problem.contains("does not read"));
        assert!(!r.correction.is_empty());
    }

    /// An object of another kind is refused by `from_metadata`, which the slug assembler
    /// reads an object at a commit through, exactly as `parse` refuses it from the index.
    #[test]
    fn an_object_of_another_kind_is_refused_naming_the_kind() {
        let mut v = object_text();
        v["kind"] = serde_json::json!("rule");
        let r = Deployment::from_metadata("at/a/commit.yaml", v.clone()).expect_err("refused");
        assert_eq!(r.file, "at/a/commit.yaml");
        assert_eq!(r.key, "kind");
        assert_eq!(r.found, "rule");
        assert!(r.correction.contains("kind: deployment"), "{r}");
        assert_eq!(
            Deployment::parse(&object(v)).expect_err("refused").key,
            "kind"
        );
    }

    #[test]
    fn a_malformed_object_is_refused_naming_its_file() {
        let mut v = object_text();
        v["listen"]["port"] = serde_json::json!(80);
        let r = Deployment::parse(&object(v)).expect_err("refused");
        assert_eq!(r.file, ".ai/repo/deployments/example.yaml");
        assert!(r.found.contains("unprivileged"));
    }

    /// A provider block the contract knows, as the shipped object states it.
    fn with_provider() -> serde_json::Value {
        let mut v = object_text();
        v["provider"] = serde_json::json!({
            "name": "gigalixir",
            "app": "prismatic-majordomus",
            "stack": "gigalixir-24",
            "url": "https://prismatic-majordomus.gigalixirapp.com",
        });
        v
    }

    #[test]
    fn no_provider_is_none() {
        assert_eq!(parse(object_text()).expect("parses").provider, None);
    }

    #[test]
    fn a_known_provider_parses() {
        let d = parse(with_provider()).expect("a known provider parses");
        let p = d.provider.expect("the provider");
        assert_eq!(p.name, ProviderName::Gigalixir);
        assert_eq!(p.stack, "gigalixir-24");
    }

    #[test]
    fn an_unknown_provider_is_refused_naming_it() {
        let mut v = with_provider();
        v["provider"]["name"] = serde_json::json!("fly");
        let r = Deployment::parse(&object(v)).expect_err("refused");
        assert_eq!(r.file, ".ai/repo/deployments/example.yaml");
        assert!(r.found.contains("fly"), "{r}");
        assert!(
            r.found.contains("gigalixir"),
            "the refusal names the known one: {r}"
        );
    }

    #[test]
    fn a_key_the_provider_block_does_not_have_is_refused() {
        let mut v = with_provider();
        v["provider"]["api_key"] = serde_json::json!("x");
        assert!(parse(v).unwrap_err().contains("api_key"));
    }

    #[test]
    fn an_unknown_interface_cannot_be_constructed() {
        let mut v = object_text();
        v["listen"]["interface"] = serde_json::json!("public");
        assert!(parse(v).is_err());
    }

    mod checks {
        use super::super::*;
        use std::collections::BTreeSet;

        fn workspace(root: &std::path::Path) -> Workspace<'_> {
            Workspace {
                root,
                routes: ["/api/v1/live", "/api/v1/ready"]
                    .into_iter()
                    .map(String::from)
                    .collect::<BTreeSet<_>>(),
                packages: vec![("majordomus-cli".to_string(), vec!["majordomus".to_string()])],
            }
        }

        fn deployment(v: serde_json::Value) -> Deployment {
            serde_json::from_value(v).expect("parses")
        }

        fn tree() -> tempfile::TempDir {
            let d = tempfile::tempdir().expect("a temporary root");
            std::fs::create_dir_all(d.path().join("apps/majordomus-cli")).expect("a package dir");
            d
        }

        #[test]
        fn a_valid_deployment_earns_no_refusal() {
            let d = tree();
            let dep = deployment(super::object_text());
            assert_eq!(dep.check("x.yaml", &workspace(d.path())), Vec::new());
        }

        #[test]
        fn a_route_no_capability_registers_is_refused_with_the_correction() {
            let d = tree();
            let mut v = super::object_text();
            v["health"]["readiness"] = serde_json::json!("/nope");
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].key, "health.readiness");
            assert_eq!(out[0].found, "/nope");
            assert!(out[0].problem.contains("no capability registers"));
            assert!(!out[0].correction.is_empty());
        }

        #[test]
        fn a_package_the_workspace_does_not_have_is_refused() {
            let d = tree();
            let mut v = super::object_text();
            v["build"]["package"] = serde_json::json!("nothing-cli");
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert_eq!(out[0].key, "build.package");
            assert!(out[0].correction.contains("majordomus-cli"));
        }

        #[test]
        fn a_binary_the_package_does_not_produce_is_refused() {
            let d = tree();
            let mut v = super::object_text();
            v["build"]["binary"] = serde_json::json!("not-a-binary");
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert_eq!(out[0].key, "build.binary");
        }

        #[test]
        fn a_build_input_that_does_not_resolve_is_refused() {
            let d = tree();
            let mut v = super::object_text();
            v["build"]["inputs"] = serde_json::json!(["apps/majordomus-cli", "not/here"]);
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert_eq!(out[0].key, "build.inputs.1");
            assert!(out[0].problem.contains("does not have"));
        }

        #[test]
        fn a_build_input_that_leaves_the_repository_is_refused() {
            let d = tree();
            let mut v = super::object_text();
            v["build"]["inputs"] = serde_json::json!(["../elsewhere"]);
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert!(out[0].problem.contains("inside the repository"));
        }

        #[test]
        fn more_running_than_exist_is_refused() {
            let d = tree();
            let mut v = super::object_text();
            v["machines"]["min_running"] = serde_json::json!(2);
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert_eq!(out[0].key, "machines.min_running");
            assert!(out[0].problem.contains("above machines.count"));
        }

        #[test]
        fn a_hosted_deployment_that_binds_loopback_is_refused() {
            let d = tree();
            let mut v = super::object_text();
            v["listen"]["interface"] = serde_json::json!("loopback");
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert_eq!(out[0].key, "listen.interface");
            assert!(out[0].correction.contains("interface: all"));
        }

        #[test]
        fn a_deployment_with_a_known_provider_earns_no_refusal() {
            let d = tree();
            let dep = deployment(super::with_provider());
            assert_eq!(dep.check("x.yaml", &workspace(d.path())), Vec::new());
        }

        #[test]
        fn a_provider_url_that_is_not_https_is_refused() {
            let d = tree();
            let mut v = super::with_provider();
            v["provider"]["url"] =
                serde_json::json!("http://prismatic-majordomus.gigalixirapp.com");
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert_eq!(out.len(), 1, "{out:?}");
            assert_eq!(out[0].file, "x.yaml");
            assert_eq!(out[0].key, "provider.url");
            assert_eq!(out[0].found, "http://prismatic-majordomus.gigalixirapp.com");
            assert!(out[0].problem.contains("https"));
            assert!(out[0].correction.contains("https://"));
        }

        #[test]
        fn a_provider_with_a_loopback_interface_is_refused_naming_the_provider() {
            let d = tree();
            let mut v = super::with_provider();
            v["listen"]["interface"] = serde_json::json!("loopback");
            let out = deployment(v).check("x.yaml", &workspace(d.path()));
            assert_eq!(out.len(), 1, "{out:?}");
            assert_eq!(out[0].key, "listen.interface");
            assert_eq!(out[0].found, "loopback");
            assert!(out[0].problem.contains("gigalixir"), "{}", out[0]);
            assert!(out[0].correction.contains("interface: all"));
        }

        #[test]
        fn every_refusal_names_the_file_the_key_the_value_and_the_correction() {
            let d = tree();
            let mut v = super::with_provider();
            v["health"]["liveness"] = serde_json::json!("/gone");
            v["build"]["package"] = serde_json::json!("nothing-cli");
            v["machines"]["min_running"] = serde_json::json!(9);
            v["listen"]["interface"] = serde_json::json!("loopback");
            v["provider"]["url"] = serde_json::json!("http://example.invalid");
            let out = deployment(v).check("the-file.yaml", &workspace(d.path()));
            assert!(out.len() >= 5);
            for r in &out {
                assert_eq!(r.file, "the-file.yaml");
                assert!(!r.key.is_empty(), "a refusal with no key: {r}");
                assert!(!r.found.is_empty(), "a refusal with no value: {r}");
                assert!(!r.problem.is_empty(), "a refusal with no problem: {r}");
                assert!(
                    !r.correction.is_empty(),
                    "a refusal with no correction: {r}"
                );
            }
        }
    }

    mod workspace {
        use super::super::*;

        /// The workspace reads the packages it actually has, with the binaries each
        /// declares — the two facts a build specification can be wrong about, taken from
        /// the manifests rather than assumed.
        #[test]
        fn the_packages_are_read_from_the_manifests() {
            let d = tempfile::tempdir().expect("a temporary root");
            let a = d.path().join("apps/one");
            let b = d.path().join("apps/two");
            std::fs::create_dir_all(&a).expect("a package dir");
            std::fs::create_dir_all(&b).expect("a package dir");
            std::fs::write(
                a.join("Cargo.toml"),
                "[package]\nname = \"one-cli\"\nversion = \"0.1.0\"\n\n[[bin]]\nname = \"one\"\npath = \"src/main.rs\"\n\n[dependencies]\nserde = \"1\"\n",
            )
            .expect("a manifest");
            // no [[bin]]: the package's own name is the binary it produces
            std::fs::write(
                b.join("Cargo.toml"),
                "[package]\nname = \"two\"\nversion = \"0.1.0\"\n",
            )
            .expect("a manifest");

            let ws = Workspace::read(d.path(), Default::default());
            assert_eq!(
                ws.packages,
                vec![
                    ("one-cli".to_string(), vec!["one".to_string()]),
                    ("two".to_string(), vec!["two".to_string()]),
                ]
            );
        }

        /// A repository with no packages is read as one, not as a failure: the refusal
        /// then names what the workspace has, which is nothing, rather than crashing on
        /// the way to saying so.
        #[test]
        fn a_tree_with_no_packages_reads_as_none() {
            let d = tempfile::tempdir().expect("a temporary root");
            assert!(Workspace::read(d.path(), Default::default())
                .packages
                .is_empty());
        }

        /// A refusal renders as one line naming all four things, so a log or a terminal
        /// carries the whole of it.
        #[test]
        fn a_refusal_renders_as_one_line() {
            let d = tempfile::tempdir().expect("a temporary root");
            let mut v = super::object_text();
            v["machines"]["min_running"] = serde_json::json!(4);
            let dep: Deployment = serde_json::from_value(v).expect("parses");
            let out = dep.check(
                "the-file.yaml",
                &Workspace::read(d.path(), Default::default()),
            );
            let line = out
                .iter()
                .find(|r| r.key == "machines.min_running")
                .expect("the refusal")
                .to_string();
            assert!(
                line.starts_with("the-file.yaml: machines.min_running is 4:"),
                "{line}"
            );
            assert!(line.contains("lower min_running"), "{line}");
        }
    }

    mod invariants {
        use super::super::*;
        use proptest::prelude::*;

        proptest! {
            /// Whatever a port is constructed from, an accepted one is bindable by an
            /// unprivileged process and a refused one is not.
            #[test]
            fn a_port_is_accepted_exactly_when_it_is_unprivileged(n in 0u32..70000) {
                let expected = (1024..=65535).contains(&n);
                prop_assert_eq!(Port::new(n).is_ok(), expected);
            }

            /// A route is accepted exactly when it is a path on this service.
            #[test]
            fn a_route_is_accepted_exactly_when_it_is_a_clean_path(s in "[/a-z0-9?# ]{1,12}") {
                let ok = s.starts_with('/')
                    && !s.contains("//") && !s.contains('?') && !s.contains('#') && !s.contains(' ');
                prop_assert_eq!(Route::new(&s).is_ok(), ok);
            }

            /// Zero is the only refused count.
            #[test]
            fn a_positive_is_accepted_exactly_when_it_is_not_zero(n in 0u32..1000) {
                prop_assert_eq!(Positive::new(n).is_ok(), n != 0);
            }

            /// Any accepted deployment binds an address its own interface and port describe,
            /// whatever the two are — the process and the image read one fact and cannot
            /// disagree about it.
            #[test]
            fn the_bound_address_is_the_declared_one(port in 1024u32..65535, all in any::<bool>()) {
                let listen = Listen {
                    port: Port::new(port).unwrap(),
                    interface: if all { Interface::All } else { Interface::Loopback },
                };
                prop_assert_eq!(
                    listen.address(),
                    format!("{}:{}", if all { "0.0.0.0" } else { "127.0.0.1" }, port)
                );
            }
        }
    }
}
