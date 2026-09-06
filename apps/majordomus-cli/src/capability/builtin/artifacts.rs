//! The `artifacts` module: what `majordomus generate` writes, read back.
//!
//! The generator declares every artifact it produces — the document it projects, the
//! encoding it is written in, the JSON Schema its content satisfies, and where it came
//! from — and commits that declaration as `docs/generated/artifacts.json`. This module is
//! the reading half: it takes the committed manifest and reconciles it with the working
//! tree, so that a person, an MCP client, an HTTP caller and the Cockpit all learn from
//! one answer which generated files exist, which encodings a document is committed in,
//! and which are stale.
//!
//! It does not regenerate anything and it holds no list of its own: a document added to
//! the generator appears here on the next `majordomus generate`, and a file this module
//! reports as stale is a file `majordomus generate --check` refuses.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::generate::{ArtifactFormat, MANIFEST_ID, MANIFEST_SCHEMA, OUT_DIR};
use crate::policy::sha256_hex;
use crate::{capability, module};

use super::get;

/// The URI under which the manifest is read as an MCP resource.
pub const ARTIFACTS_URI: &str = "majordomus://artifacts";

/// Where one generated file stands against the tree it is committed in.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactState {
    /// The file is there and its bytes hash to what the manifest recorded.
    Current,
    /// The file is there and its bytes differ: it was edited, or the generator moved on.
    Stale,
    /// The manifest names it and the tree does not have it.
    Missing,
    /// The file is there and the manifest records no hash for it: the manifest's own
    /// encodings, which cannot hash themselves. `generate --check` compares them.
    Present,
}

impl ArtifactState {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::artifacts::ArtifactState;
    /// assert_eq!(ArtifactState::Stale.as_str(), "stale");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactState::Current => "current",
            ArtifactState::Stale => "stale",
            ArtifactState::Missing => "missing",
            ArtifactState::Present => "present",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `artifacts.list`: the whole manifest, or one slice of it.
pub struct ArtifactsInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only the artifacts of this document (`registry`, `cli`, `openapi`, ...).
    pub document: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only the artifacts written in this encoding.
    pub format: Option<ArtifactFormat>,
}

impl BenchmarkCases for ArtifactsInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "all",
                ArtifactsInput {
                    document: None,
                    format: None,
                },
            ),
            NamedCase::new(
                "one-encoding",
                ArtifactsInput {
                    document: None,
                    format: Some(ArtifactFormat::Yaml),
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One document, and the encodings it is committed in.
pub struct DocumentView {
    /// The document id, shared by every encoding of it.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The schema id its structured encodings carry.
    pub schema: Option<String>,
    /// One line: what it was derived from.
    pub source: String,
    /// The encodings, in the manifest's order.
    pub formats: Vec<ArtifactFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One generated file, as the manifest declares it and as the tree has it.
pub struct ArtifactView {
    /// Repository-relative path.
    pub path: String,
    /// The document it projects.
    pub document: String,
    /// The encoding.
    pub format: ArtifactFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The JSON Schema its content satisfies, when it declares one.
    pub schema: Option<String>,
    /// One line: what it was derived from.
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The size the manifest recorded.
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The hash the manifest recorded.
    pub sha256: Option<String>,
    /// Where the file stands against it.
    pub state: ArtifactState,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// How many of each, so a caller needs no arithmetic of its own.
pub struct ArtifactTallies {
    /// Documents in the manifest.
    pub documents: usize,
    /// Files in the manifest, after any filter.
    pub artifacts: usize,
    /// Hashes that match.
    pub current: usize,
    /// Hashes that do not.
    pub stale: usize,
    /// Files the manifest names and the tree lacks.
    pub missing: usize,
    /// Files the manifest records no hash for.
    pub present: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The answer of `artifacts.list`.
pub struct ArtifactReport {
    /// The manifest this was read from, repository-relative.
    pub manifest: String,
    /// Whether the manifest is committed in this repository at all. A repository that has
    /// never run `majordomus generate` has no generated tree, which is a fact about it and
    /// not a failure of this call: every list below is then empty and every tally zero.
    pub present: bool,
    /// The schema the manifest carries, empty when there is none to read.
    pub schema: String,
    /// The documents, in the manifest's order.
    pub documents: Vec<DocumentView>,
    /// The files, in the manifest's order, after any filter.
    pub artifacts: Vec<ArtifactView>,
    /// The counts.
    pub tallies: ArtifactTallies,
    /// The command that rewrites every one of them.
    pub regenerate: String,
    /// The command that decides staleness byte for byte, which is stronger than the hash.
    pub verify: String,
}

/// The manifest as committed, deserialised into the shape the generator wrote.
#[derive(Debug, Deserialize)]
struct Manifest {
    schema: String,
    documents: Vec<DocumentView>,
    artifacts: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    path: String,
    document: String,
    format: ArtifactFormat,
    #[serde(default)]
    schema: Option<String>,
    source: String,
    #[serde(default)]
    bytes: Option<u64>,
    #[serde(default)]
    sha256: Option<String>,
}

fn artifacts_list(ctx: &Context, input: ArtifactsInput) -> Result<ArtifactReport, CapabilityError> {
    let rel = format!("{OUT_DIR}/{MANIFEST_ID}.json");
    let root = std::path::Path::new(&ctx.index.repository.root);
    let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
        return Ok(ArtifactReport {
            manifest: rel,
            present: false,
            schema: String::new(),
            documents: Vec::new(),
            artifacts: Vec::new(),
            tallies: ArtifactTallies {
                documents: 0,
                artifacts: 0,
                current: 0,
                stale: 0,
                missing: 0,
                present: 0,
            },
            regenerate: "majordomus generate".into(),
            verify: "majordomus generate --check".into(),
        });
    };
    let manifest: Manifest = serde_json::from_str(&text).map_err(|e| {
        CapabilityError::Internal(format!(
            "{rel} is not the manifest this executable writes: {e}"
        ))
    })?;
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(CapabilityError::Internal(format!(
            "{rel} carries schema '{}', and this executable reads {MANIFEST_SCHEMA}",
            manifest.schema
        )));
    }

    let mut artifacts = Vec::new();
    let mut tallies = ArtifactTallies {
        documents: manifest.documents.len(),
        artifacts: 0,
        current: 0,
        stale: 0,
        missing: 0,
        present: 0,
    };
    for e in manifest.artifacts {
        if input.document.as_deref().is_some_and(|d| d != e.document) {
            continue;
        }
        if input.format.is_some_and(|f| f != e.format) {
            continue;
        }
        let state = match std::fs::read_to_string(root.join(&e.path)) {
            Err(_) => ArtifactState::Missing,
            Ok(on_disk) => match &e.sha256 {
                None => ArtifactState::Present,
                Some(hash) if *hash == sha256_hex(&on_disk) => ArtifactState::Current,
                Some(_) => ArtifactState::Stale,
            },
        };
        match state {
            ArtifactState::Current => tallies.current += 1,
            ArtifactState::Stale => tallies.stale += 1,
            ArtifactState::Missing => tallies.missing += 1,
            ArtifactState::Present => tallies.present += 1,
        }
        tallies.artifacts += 1;
        artifacts.push(ArtifactView {
            path: e.path,
            document: e.document,
            format: e.format,
            schema: e.schema,
            source: e.source,
            bytes: e.bytes,
            sha256: e.sha256,
            state,
        });
    }

    let documents = match &input.document {
        Some(id) => manifest
            .documents
            .into_iter()
            .filter(|d| &d.id == id)
            .collect(),
        None => manifest.documents,
    };
    tallies.documents = documents.len();

    Ok(ArtifactReport {
        manifest: rel,
        present: true,
        schema: manifest.schema,
        documents,
        artifacts,
        tallies,
        regenerate: "majordomus generate".into(),
        verify: "majordomus generate --check".into(),
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "artifacts",
        title: "Generated artifacts",
        description: "What `majordomus generate` writes: every document with the encodings it is committed in — JSON for a program, YAML beside it, Markdown for a reader — each with its schema, its source and its hash, reconciled with the working tree. The declaration is the generator's own manifest; nothing here keeps a list.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "artifacts.list",
                title: "List the generated artifacts",
                description: "The manifest `majordomus generate` commits as docs/generated/artifacts.json, reconciled with the working tree: every document with the encodings it is written in, and every file with its format, schema, source, size, hash and whether the file on disk still matches. Optionally narrowed to one document or one encoding. Reads only; `majordomus generate` writes and `majordomus generate --check` is the byte-for-byte verdict.",
                input: ArtifactsInput,
                output: ArtifactReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_artifacts".into()),
                        resource: Some(McpResource { uri: ARTIFACTS_URI.into(), name: "artifacts".into() }),
                    }),
                    http: get("/api/v1/artifacts"),
                    cli: None,
                },
                tags: ["artifacts", "generation", "introspection"],
                cache: CachePolicy::Process { max_entries: 8, ttl_seconds: Some(5) },
                handler: artifacts_list,
            },
        ],
    }
}
