//! The domain model: what an object is, where it came from, and what was wrong with what
//! could not become one. Nothing here knows about files being read or protocols being
//! spoken; those layers produce and consume these types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How bad a diagnostic is. `Error` excludes the file it concerns from the index and puts
/// the index into the degraded state; `Warning` and `Info` do neither.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Worth knowing; changes nothing.
    Info,
    /// Something to look at; the object is still served.
    Warning,
    /// The file is excluded and the index is degraded.
    Error,
}

/// One finding about the declarative state, named by a stable code, tied to a path where
/// there is one, and carrying the command that reproduces it where there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    /// How bad.
    pub severity: Severity,
    /// A stable machine-readable code, e.g. `unknown_key`, `duplicate_identity`.
    pub code: String,
    /// Repository-relative path of the file concerned, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What is wrong, for a person; the code is for a program.
    pub message: String,
}

impl Diagnostic {
    /// An error diagnostic.
    ///
    /// ```
    /// use majordomus_cli::{Diagnostic, Severity};
    /// let d = Diagnostic::error("unknown_key", Some("docs/x.md".into()), "key(s) not in schema 'rule': owner");
    /// assert_eq!(d.severity, Severity::Error);
    /// assert_eq!(d.code, "unknown_key");
    /// ```
    pub fn error(code: &'static str, path: Option<String>, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code: code.into(),
            path,
            message: message.into(),
        }
    }
    /// A warning diagnostic.
    pub fn warning(code: &'static str, path: Option<String>, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code: code.into(),
            path,
            message: message.into(),
        }
    }
    /// An informational diagnostic.
    pub fn info(code: &'static str, path: Option<String>, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Info,
            code: code.into(),
            path,
            message: message.into(),
        }
    }
}

/// Where an object came from. Every field is computed from the repository, never authored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ObjectProvenance")]
pub struct Provenance {
    /// Repository-relative path, forward slashes, as the version-control index names it.
    pub path: String,
    /// The directory the path sits in, repository-relative; `.` for the root. This is the
    /// hierarchy position a client orders by; no merge semantics are implied.
    pub directory: String,
    /// The `sources.yaml` class that discovered the file (`rule`, `readme`, ...).
    pub source_class: String,
    /// The manifest section the path falls under (`rules`, `prompts`, ...), when it falls
    /// under one; a root `README.md` falls under none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Size of the file in bytes.
    pub bytes: u64,
    /// For an object that is one member of a collection file: the member's key path in
    /// the file (`claims.3`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
}

/// One declarative object of the layer: a rule, a prompt, a profile, a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Object {
    /// The kind the repository declared for the class that discovered it.
    pub kind: String,
    /// Stable identity within the kind: the identity fields joined with `@`, or the path.
    pub identity: String,
    /// `majordomus://<kind>/<identity>`; unique across the index.
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The title, when the kind's title rule found one.
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The one-line description, when the kind holds one.
    pub description: Option<String>,
    /// The parsed metadata: the front matter of a Markdown file, or the whole of a YAML
    /// file. Key order is the file's.
    pub metadata: Value,
    /// The Markdown body without its front matter, or the raw text of a YAML file.
    #[serde(skip)]
    pub body: String,
    /// The whole file as read.
    #[serde(skip)]
    pub content: String,
    /// IANA media type of `content`.
    pub media_type: &'static str,
    /// Where it came from.
    pub provenance: Provenance,
}

impl Object {
    /// The tags an object declares, when its metadata carries a `tags` list.
    pub fn tags(&self) -> Vec<&str> {
        self.metadata
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }
}

/// Build the canonical URI for a kind and identity.
pub fn uri_for(kind: &str, identity: &str) -> String {
    format!("majordomus://{kind}/{identity}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_carry_their_severity_and_uri_is_canonical() {
        assert_eq!(
            Diagnostic::warning("claimed_twice", None, "x").severity,
            Severity::Warning
        );
        assert_eq!(
            Diagnostic::info("note", Some("a".into()), "x").severity,
            Severity::Info
        );
        assert!(Severity::Error > Severity::Warning && Severity::Warning > Severity::Info);
        assert_eq!(
            uri_for("rule", "project.x@1"),
            "majordomus://rule/project.x@1"
        );
    }
}
