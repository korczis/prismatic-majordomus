//! The views more than one module answers with: an object of the layer as a client reads
//! it, its summary for a listing, and the empty input.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::model::{Object, Provenance as ObjectProvenance};

/// One declarative object of the repository's layer, as a client reads it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObjectView {
    /// `majordomus://<kind>/<identity>`.
    pub uri: String,
    /// The capability id, `<kind>.<identity>`.
    pub id: String,
    /// The kind the object was read as (`rule`, `prompt`, `document`, ...).
    pub kind: String,
    /// The identity within the kind (`majordomus.scope-integrity@1`, `continue`, a path).
    pub identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The title the kind's title rule found, when it found one.
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The one-line description the kind's description field held, when it held one.
    pub description: Option<String>,
    /// The parsed front matter or YAML, keys in the file's order.
    pub metadata: Value,
    /// Where the object came from: path, directory, source class, section, size, member.
    pub provenance: ObjectProvenance,
    /// IANA media type of `content` (`text/markdown`, `application/yaml`, `application/json`, `text/plain`).
    pub media_type: String,
    /// The file as read.
    pub content: String,
}

impl ObjectView {
    pub(crate) fn of(o: &Object) -> Self {
        ObjectView {
            uri: o.uri.clone(),
            id: format!("{}.{}", o.kind, o.identity),
            kind: o.kind.clone(),
            identity: o.identity.clone(),
            title: o.title.clone(),
            description: o.description.clone(),
            metadata: o.metadata.clone(),
            provenance: o.provenance.clone(),
            media_type: o.media_type.to_string(),
            content: o.content.clone(),
        }
    }
}

/// One object, summarised for a listing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObjectSummary {
    /// `majordomus://<kind>/<identity>`.
    pub uri: String,
    /// The capability id, `<kind>.<identity>`.
    pub id: String,
    /// The kind the object was read as.
    pub kind: String,
    /// The identity within the kind.
    pub identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The title, when the kind's title rule found one.
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The one-line description, when the kind holds one.
    pub description: Option<String>,
    /// Repository-relative source path.
    pub path: String,
}

impl ObjectSummary {
    pub(crate) fn of(o: &Object) -> Self {
        ObjectSummary {
            uri: o.uri.clone(),
            id: format!("{}.{}", o.kind, o.identity),
            kind: o.kind.clone(),
            identity: o.identity.clone(),
            title: o.title.clone(),
            description: o.description.clone(),
            path: o.provenance.path.clone(),
        }
    }
}

/// No input.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

impl BenchmarkCases for Empty {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("default", Empty {})]
    }
}
