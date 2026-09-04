//! The surface: what the index looks like as resources and tools, independent of any
//! protocol encoding. Every resource is an object of the index or the repository itself;
//! every tool is read-only and answers from the index alone.

use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Value};

use crate::index::Index;
use crate::model::Object;

/// The URI of the resource describing the repository and the index itself.
pub const REPOSITORY_URI: &str = "majordomus://repository";

/// The default and maximum number of search hits.
pub const SEARCH_DEFAULT_LIMIT: usize = 20;
pub const SEARCH_MAX_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub media_type: &'static str,
    /// Provenance and kind, carried as metadata beside the resource.
    pub meta: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceContent {
    pub uri: String,
    pub media_type: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// What a tool call produced: a structured value the protocol renders as text, or a
/// refusal with the reason. Refusals are results, not protocol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOutcome {
    Ok(Value),
    Refused(String),
}

/// Why a request to the surface could not be answered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SurfaceError {
    #[error("unknown resource: {0}")]
    UnknownResource(String),
    #[error("unknown tool: {0}")]
    UnknownTool(String),
}

#[derive(Debug, Clone)]
pub struct Surface {
    index: Arc<Index>,
}

impl Surface {
    pub fn new(index: Index) -> Self {
        Surface {
            index: Arc::new(index),
        }
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Every resource, the repository first, then the objects by URI.
    pub fn resources(&self) -> Vec<Resource> {
        let mut out = Vec::with_capacity(self.index.objects.len() + 1);
        out.push(Resource {
            uri: REPOSITORY_URI.to_string(),
            name: "repository".to_string(),
            title: Some("Repository".to_string()),
            description: Some(
                "The repository, its layer, its git state, and the diagnostics of this index"
                    .to_string(),
            ),
            media_type: "application/json",
            meta: json!({ "kind": "repository" }),
        });
        out.extend(self.index.objects.iter().map(resource_of));
        out
    }

    pub fn read(&self, uri: &str) -> Result<ResourceContent, SurfaceError> {
        if uri == REPOSITORY_URI {
            return Ok(ResourceContent {
                uri: uri.to_string(),
                media_type: "application/json",
                text: pretty(&self.repository_info()),
            });
        }
        let object = self
            .index
            .get(uri)
            .ok_or_else(|| SurfaceError::UnknownResource(uri.to_string()))?;
        Ok(ResourceContent {
            uri: uri.to_string(),
            media_type: object.media_type,
            text: object.content.clone(),
        })
    }

    pub fn tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "majordomus_list",
                title: "List objects",
                description: "List the declarative objects of the repository's AI layer, optionally by kind or tag. Read-only.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "description": "Only objects of this kind (rule, prompt, profile, policy, milestone, issue, document)" },
                        "tag": { "type": "string", "description": "Only objects whose metadata tags include this tag" }
                    },
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "majordomus_get",
                title: "Get one object",
                description: "One object by URI (majordomus://<kind>/<identity>): metadata, provenance and content. Read-only.",
                input_schema: json!({
                    "type": "object",
                    "properties": { "uri": { "type": "string" } },
                    "required": ["uri"],
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "majordomus_search",
                title: "Search objects",
                description: "Case-insensitive substring search over identities, titles, descriptions and content. Read-only.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "kind": { "type": "string" },
                        "limit": { "type": "integer", "minimum": 1, "maximum": SEARCH_MAX_LIMIT }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }),
            },
            Tool {
                name: "majordomus_repository",
                title: "Repository and index state",
                description: "The repository root, layer sections, git state, discovery mode, kinds present, and every diagnostic. Read-only.",
                input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
            },
        ]
    }

    pub fn call(&self, name: &str, args: &Value) -> Result<ToolOutcome, SurfaceError> {
        match name {
            "majordomus_list" => Ok(self.list(args)),
            "majordomus_get" => Ok(self.get(args)),
            "majordomus_search" => Ok(self.search(args)),
            "majordomus_repository" => Ok(ToolOutcome::Ok(self.repository_info())),
            other => Err(SurfaceError::UnknownTool(other.to_string())),
        }
    }

    fn list(&self, args: &Value) -> ToolOutcome {
        let kind = args.get("kind").and_then(Value::as_str);
        let tag = args.get("tag").and_then(Value::as_str);
        let items: Vec<Value> = self
            .index
            .objects
            .iter()
            .filter(|o| kind.is_none_or(|k| o.kind == k))
            .filter(|o| tag.is_none_or(|t| o.tags().contains(&t)))
            .map(summary_of)
            .collect();
        ToolOutcome::Ok(json!({ "count": items.len(), "objects": items }))
    }

    fn get(&self, args: &Value) -> ToolOutcome {
        let Some(uri) = args.get("uri").and_then(Value::as_str) else {
            return ToolOutcome::Refused("argument 'uri' is required".into());
        };
        match self.index.get(uri) {
            Some(o) => ToolOutcome::Ok(json!({
                "uri": o.uri, "kind": o.kind, "identity": o.identity,
                "title": o.title, "description": o.description,
                "metadata": o.metadata, "provenance": o.provenance,
                "media_type": o.media_type, "content": o.content,
            })),
            None => ToolOutcome::Refused(format!("unknown resource: {uri}")),
        }
    }

    fn search(&self, args: &Value) -> ToolOutcome {
        let Some(query) = args
            .get("query")
            .and_then(Value::as_str)
            .filter(|q| !q.trim().is_empty())
        else {
            return ToolOutcome::Refused(
                "argument 'query' is required and must not be blank".into(),
            );
        };
        let kind = args.get("kind").and_then(Value::as_str);
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .map(|l| (l as usize).clamp(1, SEARCH_MAX_LIMIT))
            .unwrap_or(SEARCH_DEFAULT_LIMIT);
        let needle = query.to_lowercase();
        let mut hits = Vec::new();
        for o in self
            .index
            .objects
            .iter()
            .filter(|o| kind.is_none_or(|k| o.kind == k))
        {
            let head = [
                o.identity.as_str(),
                o.title.as_deref().unwrap_or(""),
                o.description.as_deref().unwrap_or(""),
            ]
            .iter()
            .any(|s| s.to_lowercase().contains(&needle));
            let line = o
                .content
                .lines()
                .find(|l| l.to_lowercase().contains(&needle));
            if head || line.is_some() {
                let mut hit = summary_of(o);
                if let Some(l) = line {
                    hit["snippet"] = Value::String(l.trim().chars().take(200).collect());
                }
                hits.push(hit);
                if hits.len() == limit {
                    break;
                }
            }
        }
        ToolOutcome::Ok(
            json!({ "query": query, "count": hits.len(), "limit": limit, "hits": hits }),
        )
    }

    pub fn repository_info(&self) -> Value {
        json!({
            "repository": self.index.repository,
            "state": self.index.state,
            "objects": self.index.objects.len(),
            "kinds": self.index.kinds(),
            "diagnostics": self.index.diagnostics,
        })
    }
}

fn resource_of(o: &Object) -> Resource {
    Resource {
        uri: o.uri.clone(),
        name: o.identity.clone(),
        title: o.title.clone(),
        description: o.description.clone(),
        media_type: o.media_type,
        meta: json!({ "kind": o.kind, "identity": o.identity, "provenance": o.provenance }),
    }
}

fn summary_of(o: &Object) -> Value {
    json!({
        "uri": o.uri, "kind": o.kind, "identity": o.identity,
        "title": o.title, "description": o.description, "path": o.provenance.path,
    })
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}
