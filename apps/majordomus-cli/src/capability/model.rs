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
/// Grammar: the namespace matches `[a-z][a-z0-9_-]*`, the local part is non-empty, and
/// every character is printable ASCII and not whitespace. The local part is opaque: a
/// path, a versioned identity, or a name, as the kind's identity rule produced it.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Parse and validate.
    pub fn parse(text: &str) -> Result<Self, String> {
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return Err("empty".into());
        }
        if !text.is_ascii() || bytes.iter().any(|b| !(0x21..0x7f).contains(b)) {
            return Err("carries whitespace or a non-printable or non-ASCII character".into());
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

    /// For descriptors written in code; validated when the registry is built.
    pub(crate) fn unchecked(text: &str) -> Self {
        CapabilityId(text.to_string())
    }

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

/// What a capability is. Two kinds exist because two semantics exist: something that is
/// executed with an input and answers with an output, and something that is read.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityKind {
    /// Executable and read-only: a typed handler, an input schema, an output schema.
    Query,
    /// Declarative content the repository holds: read as it is, never executed.
    Resource,
}

/// Where a capability stands, in the repository's own vocabulary for claims. A capability
/// that is `Planned` or `Unsupported` may be listed but is never executable through any
/// projection; the registry refuses to build otherwise.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    Implemented,
    BehaviorallyVerified,
    Experimental,
    Planned,
    Unsupported,
}

impl Stability {
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
    Builtin { module: String },
    /// Read from the repository's layer.
    Declarative {
        /// Repository-relative path.
        path: String,
        directory: String,
        source_class: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        section: Option<String>,
        media_type: String,
        /// For one member of a collection file, its key path in the file (`claims.3`).
        #[serde(skip_serializing_if = "Option::is_none")]
        member: Option<String>,
    },
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
pub struct McpResource {
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
pub enum HttpMethod {
    Get,
    Post,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
        }
    }
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
    pub method: HttpMethod,
    pub path: String,
}

impl HttpExposure {
    /// Every capability route starts here; the version is part of the contract.
    pub const PREFIX: &'static str = "/api/v1/";

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
    pub path: Vec<String>,
}

/// The projections a capability declares. Absence is explicit: `None` means not exposed
/// there, and nothing infers an exposure a descriptor did not declare.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Exposure {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpExposure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpExposure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli: Option<CliExposure>,
}

impl Exposure {
    pub fn is_empty(&self) -> bool {
        self.mcp.is_none() && self.http.is_none() && self.cli.is_none()
    }
}

/// The canonical descriptor. Everything a projection may say about a capability is here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Capability {
    pub id: CapabilityId,
    pub kind: CapabilityKind,
    pub title: String,
    pub description: String,
    pub input: CanonicalSchema,
    pub output: CanonicalSchema,
    pub provenance: Provenance,
    pub exposure: Exposure,
    pub stability: Stability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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
            "a.b\u{e9}",
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
