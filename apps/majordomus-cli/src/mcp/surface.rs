//! The surface: the registry as resources and tools, independent of any protocol
//! encoding. Every resource is a capability with an MCP resource exposure; every tool is
//! a capability with an MCP tool exposure; every call goes through the registry to the
//! one handler the capability has. Nothing is declared here.

use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Value};

use crate::capability::{CapabilityError, CapabilityKind, CapabilityRegistry, Context};
use crate::index::Index;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// One resource as a client lists it.
pub struct Resource {
    /// The URI a client reads.
    pub uri: String,
    /// The short name: the identity, or `repository`.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The title, when there is one.
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The description, when there is one.
    pub description: Option<String>,
    /// IANA media type of what a read returns.
    pub media_type: String,
    /// Canonical id, kind and provenance, carried as metadata beside the resource.
    pub meta: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// What a read returns.
pub struct ResourceContent {
    /// The URI read.
    pub uri: String,
    /// IANA media type of `text`.
    pub media_type: String,
    /// The content.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// One tool as a client lists it.
pub struct Tool {
    /// The tool name a client calls.
    pub name: String,
    /// The title.
    pub title: String,
    /// The description.
    pub description: String,
    /// The canonical id the tool projects.
    pub id: String,
    /// The canonical input schema.
    pub input_schema: Value,
    /// The canonical output schema.
    pub output_schema: Value,
}

/// What a tool call produced: a value, or a refusal with the reason. Refusals are
/// results, not protocol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOutcome {
    /// The value the handler produced.
    Ok(Value),
    /// The handler declined, with the reason.
    Refused(String),
}

/// Why a request to the surface could not be answered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SurfaceError {
    #[error("unknown resource: {0}")]
    /// No capability projects this URI.
    UnknownResource(String),
    #[error("unknown tool: {0}")]
    /// No capability projects this tool name.
    UnknownTool(String),
    #[error("internal: {0}")]
    /// The handler itself failed.
    Internal(String),
}

#[derive(Clone)]
/// The registry seen as resources and tools.
pub struct Surface {
    ctx: Arc<Context>,
}

impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Surface")
            .field("capabilities", &self.ctx.registry.len())
            .finish()
    }
}

impl Surface {
    /// A surface over a loaded context.
    pub fn new(ctx: Arc<Context>) -> Self {
        Surface { ctx }
    }

    /// The index behind the surface.
    pub fn index(&self) -> &Index {
        &self.ctx.index
    }

    /// The registry behind the surface.
    pub fn registry(&self) -> &CapabilityRegistry {
        &self.ctx.registry
    }

    /// Every resource: executable ones (a query read as a document) first, then the
    /// declarative ones, each group by canonical id.
    pub fn resources(&self) -> Vec<Resource> {
        let mut out: Vec<(u8, Resource)> = Vec::new();
        for c in self.ctx.registry.iter() {
            let Some(res) = c.exposure.mcp.as_ref().and_then(|m| m.resource.as_ref()) else {
                continue;
            };
            let (rank, media_type, meta) = match c.kind {
                CapabilityKind::Query => (
                    0,
                    "application/json".to_string(),
                    json!({ "id": c.id, "kind": "query", "provenance": c.provenance }),
                ),
                CapabilityKind::Resource => {
                    let object = self.ctx.index.get(&res.uri);
                    (
                        1,
                        object
                            .map(|o| o.media_type.to_string())
                            .unwrap_or_else(|| "text/plain".into()),
                        json!({
                            "id": c.id,
                            "kind": object.map(|o| o.kind.as_str()).unwrap_or("resource"),
                            "identity": object.map(|o| o.identity.as_str()).unwrap_or(&res.name),
                            "provenance": object.map(|o| serde_json::to_value(&o.provenance).unwrap_or(Value::Null)).unwrap_or(Value::Null),
                        }),
                    )
                }
            };
            out.push((
                rank,
                Resource {
                    uri: res.uri.clone(),
                    name: res.name.clone(),
                    title: Some(c.title.clone()),
                    description: (!c.description.is_empty()).then(|| c.description.clone()),
                    media_type,
                    meta,
                },
            ));
        }
        out.sort_by_key(|(rank, _)| *rank);
        out.into_iter().map(|(_, r)| r).collect()
    }

    /// Read one resource: an object's content, or a query with a resource exposure answered as JSON.
    pub fn read(&self, uri: &str) -> Result<ResourceContent, SurfaceError> {
        let c = self
            .ctx
            .registry
            .by_mcp_uri(uri)
            .ok_or_else(|| SurfaceError::UnknownResource(uri.to_string()))?;
        match c.kind {
            CapabilityKind::Resource => {
                let object = self
                    .ctx
                    .index
                    .get(uri)
                    .ok_or_else(|| SurfaceError::UnknownResource(uri.to_string()))?;
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    media_type: object.media_type.to_string(),
                    text: object.content.clone(),
                })
            }
            CapabilityKind::Query => {
                let value = self
                    .ctx
                    .registry
                    .call(&self.ctx, c.id.as_str(), json!({}))
                    .map_err(|e| SurfaceError::Internal(e.to_string()))?;
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    media_type: "application/json".into(),
                    text: pretty(&value),
                })
            }
        }
    }

    /// Every tool, by canonical id.
    pub fn tools(&self) -> Vec<Tool> {
        self.ctx
            .registry
            .iter()
            .filter_map(|c| {
                let name = c.exposure.mcp.as_ref()?.tool.clone()?;
                Some(Tool {
                    name,
                    title: c.title.clone(),
                    description: c.description.clone(),
                    id: c.id.to_string(),
                    input_schema: c.input.for_mcp(),
                    output_schema: c.output.for_mcp(),
                })
            })
            .collect()
    }

    /// Call a tool by name.
    pub fn call(&self, name: &str, args: &Value) -> Result<ToolOutcome, SurfaceError> {
        let c = self
            .ctx
            .registry
            .by_mcp_tool(name)
            .ok_or_else(|| SurfaceError::UnknownTool(name.to_string()))?;
        match self
            .ctx
            .registry
            .call(&self.ctx, c.id.as_str(), args.clone())
        {
            Ok(v) => Ok(ToolOutcome::Ok(v)),
            Err(CapabilityError::Internal(e)) => Err(SurfaceError::Internal(e)),
            Err(e) => Ok(ToolOutcome::Refused(e.to_string())),
        }
    }

    /// The repository report, as the `repository.info` capability answers it.
    pub fn repository_info(&self) -> Result<Value, SurfaceError> {
        self.ctx
            .registry
            .call(&self.ctx, "repository.info", json!({}))
            .map_err(|e| SurfaceError::Internal(e.to_string()))
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}
