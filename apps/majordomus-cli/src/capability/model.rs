//! The descriptor: identity, kind, schemas, provenance, exposure, stability. Plain data,
//! serialisable, with no handler and no transport type in it.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::schema::CanonicalSchema;

/// A stable, globally meaningful identity: a namespace, a dot, and a local part.
/// `repository.info` and `objects.get` for executables; `<kind>.<identity>` for a
/// declarative object (`rule.majordomus.scope-integrity@1`, `document.docs/CLI.md`,
/// `policy..ai/repo/policy.yaml`).
///
/// Grammar: the namespace matches `[a-z][a-z0-9_-]*`; the local part is non-empty and
/// carries no whitespace or control character, any other Unicode included, because it is
/// opaque: a path, a versioned identity, or a name, as the kind's identity rule produced it.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Parse and validate.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityId;
    /// let id = CapabilityId::parse("rule.majordomus.scope-integrity@1").unwrap();
    /// assert_eq!(id.namespace(), "rule");
    /// assert!(CapabilityId::parse("Repository.info").is_err(), "namespace is lowercase");
    /// assert!(CapabilityId::parse("repository").is_err(), "a dot is required");
    /// assert!(CapabilityId::parse("document.docs/Příručka.md").is_ok(), "the local part is opaque");
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return Err("empty".into());
        }
        if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err("carries whitespace or a control character".into());
        }
        let Some((namespace, local)) = text.split_once('.') else {
            return Err("needs a namespace, a dot, and a local part".into());
        };
        let mut chars = namespace.chars();
        let ok_namespace = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !ok_namespace {
            return Err(format!("namespace '{namespace}' is not [a-z][a-z0-9_-]*"));
        }
        if local.is_empty() {
            return Err("the local part after the dot is empty".into());
        }
        Ok(CapabilityId(text.to_string()))
    }

    /// For descriptors written in code (the `capability!` macro); validated when the
    /// registry is built, never before.
    pub fn unchecked(text: &str) -> Self {
        CapabilityId(text.to_string())
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first segment.
    pub fn namespace(&self) -> &str {
        self.0.split('.').next().unwrap_or_default()
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A module identity: the namespace of every capability the module composes, matching
/// `[a-z][a-z0-9_-]*`. Builtin modules declare theirs in `module!`; a declarative
/// object's module is its kind.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ModuleId(String);

impl ModuleId {
    /// Parse and validate.
    ///
    /// ```
    /// use majordomus_cli::capability::ModuleId;
    /// assert!(ModuleId::parse("repository").is_ok());
    /// assert!(ModuleId::parse("Repository").is_err());
    /// assert!(ModuleId::parse("").is_err());
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut chars = text.chars();
        let ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if ok {
            Ok(ModuleId(text.to_string()))
        } else {
            Err(format!("module id '{text}' is not [a-z][a-z0-9_-]*"))
        }
    }

    /// For descriptors written in code; validated when the registry is built.
    pub fn unchecked(text: &str) -> Self {
        ModuleId(text.to_string())
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why an executable capability is not benchmarked. Typed, so that a waiver is a
/// reviewable statement and never a convenience; `not_executable` is the registry's own
/// reason for resources and is never written by hand.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WaiverReason {
    /// A resource: read, never executed; nothing to time but `objects.get`, which is.
    NotExecutable,
    /// The capability changes something outside this process and cannot be run in a loop.
    Destructive,
    /// The capability talks to something the benchmark host cannot provide.
    ExternalDependency,
}

/// Whether the capability is a benchmark target. `Required` is the default and the norm:
/// every executable capability is timed directly and through every transport it is
/// exposed on, with the cases its input type provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum BenchmarkPolicy {
    /// Timed directly and through every exposure; coverage fails without a case.
    Required,
    /// Not timed, for the typed reason; coverage reports it as waived, never as covered.
    Waived {
        /// Why.
        reason: WaiverReason,
    },
}

/// Whether, and how, the executor keeps results of this capability. Cache lives in the
/// executor and nowhere else, so MCP, HTTP and the command line share one; the key is the
/// canonical id, the normalised input and the registry fingerprint, so a changed
/// repository never answers from an old entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum CachePolicy {
    /// Every call runs the handler.
    Disabled,
    /// Results are kept in this process's memory, bounded, for equal inputs.
    Process {
        /// The most entries kept for this capability; the oldest is evicted first.
        max_entries: usize,
        /// Seconds an entry stays valid; `None` for the life of the process.
        #[serde(skip_serializing_if = "Option::is_none")]
        ttl_seconds: Option<u64>,
    },
}

impl CachePolicy {
    /// Is the policy well-formed? The reason when it is not.
    ///
    /// ```
    /// use majordomus_cli::capability::CachePolicy;
    /// assert!(CachePolicy::Disabled.validate().is_ok());
    /// assert!(CachePolicy::Process { max_entries: 0, ttl_seconds: None }.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        match self {
            CachePolicy::Disabled => Ok(()),
            CachePolicy::Process { max_entries: 0, .. } => {
                Err("a process cache with max_entries 0 keeps nothing".into())
            }
            CachePolicy::Process {
                ttl_seconds: Some(0),
                ..
            } => Err("a process cache with ttl_seconds 0 keeps nothing".into()),
            CachePolicy::Process { .. } => Ok(()),
        }
    }

    /// Does the policy keep anything?
    pub fn is_enabled(&self) -> bool {
        !matches!(self, CachePolicy::Disabled)
    }
}

/// What a capability is. Three kinds exist because three semantics exist: something that
/// is executed and changes nothing, something that is executed and changes this process's
/// own memory, and something that is read. Nothing of any kind writes to the repository.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityKind {
    /// Executable and read-only: a typed handler, an input schema, an output schema.
    Query,
    /// Executable with an effect on this process's in-memory state and nowhere else (a
    /// peer announcing itself): a typed handler, bound to `POST` over HTTP, and announced
    /// to MCP clients as not read-only.
    Command,
    /// Declarative content the repository holds: read as it is, never executed.
    Resource,
}

impl CapabilityKind {
    /// Is a capability of this kind called, with a handler, rather than read?
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityKind;
    /// assert!(CapabilityKind::Query.is_executable() && CapabilityKind::Command.is_executable());
    /// assert!(!CapabilityKind::Resource.is_executable());
    /// ```
    pub fn is_executable(self) -> bool {
        !matches!(self, CapabilityKind::Resource)
    }

    /// Does a call of this kind leave the process as it found it?
    pub fn is_read_only(self) -> bool {
        !matches!(self, CapabilityKind::Command)
    }
}

/// Where a capability stands, in the repository's own vocabulary for claims. A capability
/// that is `Planned` or `Unsupported` may be listed but is never executable through any
/// projection; the registry refuses to build otherwise.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    /// Implemented, and no behavioural test names it yet.
    Implemented,
    /// Implemented and proved by a behavioural test.
    BehaviorallyVerified,
    /// Implemented, executable, and expected to change.
    Experimental,
    /// Specified and not implemented: listed, never executable.
    Planned,
    /// Considered and refused: listed with the reason, never executable.
    Unsupported,
}

impl Stability {
    /// May this capability be exposed as executable through any projection?
    pub fn executable(self) -> bool {
        matches!(
            self,
            Stability::Implemented | Stability::BehaviorallyVerified | Stability::Experimental
        )
    }
}

/// Where a capability came from. Never an absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "lowercase")]
#[schemars(rename = "CapabilityProvenance")]
pub enum Provenance {
    /// Written in Rust, in the named module of this executable.
    /// Written in Rust, composed in `builtin.rs`.
    Builtin {
        /// The Rust module the descriptor was composed in.
        module: String,
    },
    /// Read from the repository's layer.
    Declarative {
        /// Repository-relative path.
        path: String,
        /// The directory the path sits in, repository-relative; `.` for the root.
        directory: String,
        /// The `sources.yaml` class that discovered the file.
        source_class: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        /// The manifest section the path falls under, when it falls under one.
        section: Option<String>,
        /// IANA media type of the object's content.
        media_type: String,
        /// For one member of a collection file, its key path in the file (`claims.3`).
        #[serde(skip_serializing_if = "Option::is_none")]
        member: Option<String>,
    },
}

/// Where this crate lives in the repository, so that a builtin descriptor's Rust module
/// path names a file a reader can open. A fact of the tree's layout; the site generator
/// refuses a path that does not exist.
pub const CRATE_DIR: &str = "apps/majordomus-cli";

impl Provenance {
    /// The repository-relative path of the source: a declarative object's file, or the
    /// Rust file a builtin descriptor was composed in, by the crate's layout
    /// (`majordomus_cli::capability::builtin::objects` is
    /// `apps/majordomus-cli/src/capability/builtin/objects.rs`).
    ///
    /// ```
    /// use majordomus_cli::capability::Provenance;
    /// let p = Provenance::Builtin { module: "majordomus_cli::capability::builtin::objects".into() };
    /// assert_eq!(p.source_path(), "apps/majordomus-cli/src/capability/builtin/objects.rs");
    /// ```
    pub fn source_path(&self) -> String {
        match self {
            Provenance::Builtin { module } => {
                let inner = module
                    .strip_prefix("majordomus_cli::")
                    .unwrap_or(module.as_str());
                format!("{CRATE_DIR}/src/{}.rs", inner.replace("::", "/"))
            }
            Provenance::Declarative { path, .. } => path.clone(),
        }
    }
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Provenance::Builtin { module } => write!(f, "builtin {module}"),
            Provenance::Declarative {
                path,
                member: Some(m),
                ..
            } => write!(f, "{path}#{m}"),
            Provenance::Declarative {
                path, member: None, ..
            } => write!(f, "{path}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// An MCP resource: its URI and the short name a client lists.
pub struct McpResource {
    /// `majordomus://<kind>/<identity>`, or `majordomus://repository`.
    pub uri: String,
    /// The short name a client lists; the identity for a declarative object.
    pub name: String,
}

/// How, if at all, a capability appears to an MCP client.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct McpExposure {
    /// As a tool with this name (`[a-z0-9_]+`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// As a readable resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<McpResource>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "UPPERCASE")]
/// The HTTP methods a capability may be bound to.
pub enum HttpMethod {
    /// Read-only; the input is bound from the query string.
    Get,
    /// The input is bound from the JSON body. No builtin uses it yet.
    Post,
}

impl HttpMethod {
    /// The method as it appears on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
        }
    }
    /// The method for a wire name; `None` for one this projection does not serve.
    ///
    /// ```
    /// use majordomus_cli::capability::HttpMethod;
    /// assert_eq!(HttpMethod::parse("GET"), Some(HttpMethod::Get));
    /// assert_eq!(HttpMethod::parse("DELETE"), None);
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "GET" => Some(HttpMethod::Get),
            "POST" => Some(HttpMethod::Post),
            _ => None,
        }
    }
}

/// How a capability appears over HTTP. `GET` binds every top-level input property as a
/// query parameter; `POST` binds the input as the JSON request body. Paths are absolute
/// and live under [`HttpExposure::PREFIX`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HttpExposure {
    /// The method.
    pub method: HttpMethod,
    /// The absolute path, under [`HttpExposure::PREFIX`].
    pub path: String,
}

impl HttpExposure {
    /// Every capability route starts here; the version is part of the contract.
    pub const PREFIX: &'static str = "/api/v1/";

    /// Is the path one this projection can serve? The reason when it is not.
    ///
    /// ```
    /// use majordomus_cli::capability::{HttpExposure, HttpMethod};
    /// let ok = HttpExposure { method: HttpMethod::Get, path: "/api/v1/objects".into() };
    /// assert!(ok.validate().is_ok());
    /// let bad = HttpExposure { method: HttpMethod::Get, path: "/objects".into() };
    /// assert!(bad.validate().unwrap_err().contains("/api/v1/"));
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        let p = &self.path;
        if !p.starts_with(Self::PREFIX) {
            return Err(format!("path '{p}' is not under {}", Self::PREFIX));
        }
        if p.ends_with('/') || p.contains("//") || p.contains('?') || p.contains('#') {
            return Err(format!(
                "path '{p}' has a trailing slash, an empty segment, or a query or fragment"
            ));
        }
        if !p
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
        {
            return Err(format!(
                "path '{p}' carries a character outside [A-Za-z0-9/-_.]"
            ));
        }
        Ok(())
    }
}

/// How a capability appears on the command line: the words after `majordomus`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CliExposure {
    /// The words after `majordomus`, e.g. `["capabilities", "list"]`.
    pub path: Vec<String>,
}

/// The projections a capability declares. Absence is explicit: `None` means not exposed
/// there, and nothing infers an exposure a descriptor did not declare.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Exposure {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The MCP projection, when declared.
    pub mcp: Option<McpExposure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The HTTP projection, when declared.
    pub http: Option<HttpExposure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The command-line projection, when declared.
    pub cli: Option<CliExposure>,
}

impl Exposure {
    /// Exposed nowhere?
    pub fn is_empty(&self) -> bool {
        self.mcp.is_none() && self.http.is_none() && self.cli.is_none()
    }
}

/// The canonical descriptor. Everything a projection may say about a capability is here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Capability {
    /// The canonical identity.
    pub id: CapabilityId,
    /// The module that composes it: the id's namespace for a builtin, the kind for a
    /// declarative object.
    pub module: ModuleId,
    /// Query, command or resource.
    pub kind: CapabilityKind,
    /// The short name every projection shows.
    pub title: String,
    /// The one-paragraph description every projection shows.
    pub description: String,
    /// The canonical schema of the input; an empty object for a resource.
    pub input: CanonicalSchema,
    /// The canonical schema of the output; the object view for a resource.
    pub output: CanonicalSchema,
    /// Where it came from.
    pub provenance: Provenance,
    /// Where it is projected; absence is explicit.
    pub exposure: Exposure,
    /// Where it stands.
    pub stability: Stability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Free tags, from the declarative object's `tags` or the descriptor.
    pub tags: Vec<String>,
    /// Whether it is a benchmark target; the cases come from the input type.
    pub benchmark: BenchmarkPolicy,
    /// Whether the executor keeps its results.
    pub cache: CachePolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_grammar() {
        assert!(CapabilityId::parse("repository.info").is_ok());
        assert!(CapabilityId::parse("rule.majordomus.scope-integrity@1").is_ok());
        assert!(CapabilityId::parse("document.docs/CLI.md").is_ok());
        for bad in [
            "",
            "repository",
            "Repository.info",
            "a.",
            "a b.c",
            "1a.b",
            "a.b\u{7}",
        ] {
            assert!(CapabilityId::parse(bad).is_err(), "{bad:?} accepted");
        }
        assert_eq!(
            CapabilityId::parse("objects.get").unwrap().namespace(),
            "objects"
        );
    }

    #[test]
    fn http_exposure_validation() {
        let ok = HttpExposure {
            method: HttpMethod::Get,
            path: "/api/v1/objects".into(),
        };
        assert!(ok.validate().is_ok());
        for bad in [
            "/objects",
            "/api/v1/objects/",
            "/api/v1//x",
            "/api/v1/x?y",
            "/api/v1/x y",
        ] {
            assert!(
                HttpExposure {
                    method: HttpMethod::Get,
                    path: bad.into()
                }
                .validate()
                .is_err(),
                "{bad}"
            );
        }
    }
}
