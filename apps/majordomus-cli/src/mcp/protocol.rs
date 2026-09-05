//! JSON-RPC 2.0 and the MCP methods this server answers, over a [`Surface`]. Transport
//! agnostic: a message in, zero or one message out. The subset spoken is the read-only
//! server side of the protocol: `initialize`, `ping`, `resources/list`, `resources/read`,
//! `resources/templates/list`, `tools/list`, `tools/call`. Prompts are not advertised.

use serde_json::{json, Value};

use super::surface::{Surface, SurfaceError, ToolOutcome};

/// Protocol versions this server accepts from a client, newest first. A client asking for
/// another gets the first one.
pub const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// The `serverInfo.name` this server announces.
pub const SERVER_NAME: &str = "majordomus";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;
/// MCP's code for a resource that does not exist.
const RESOURCE_NOT_FOUND: i64 = -32002;

#[derive(Debug)]
/// The protocol state over a surface: whether the client has initialised, and the version to announce.
pub struct Server {
    surface: Surface,
    version: &'static str,
    initialized: bool,
}

impl Server {
    /// A server over a surface, announcing `version` as its own.
    pub fn new(surface: Surface, version: &'static str) -> Self {
        Server {
            surface,
            version,
            initialized: false,
        }
    }

    /// The surface being served.
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    /// The response to a parse failure of an incoming line.
    pub fn parse_error(reason: &str) -> Value {
        error(Value::Null, PARSE_ERROR, &format!("parse error: {reason}"))
    }

    /// Handle one decoded message. A notification yields `None`; a batch yields a batch of
    /// the responses its requests produced, or `None` when it held only notifications.
    pub fn handle(&mut self, message: Value) -> Option<Value> {
        match message {
            Value::Array(batch) => {
                if batch.is_empty() {
                    return Some(error(Value::Null, INVALID_REQUEST, "empty batch"));
                }
                let responses: Vec<Value> = batch
                    .into_iter()
                    .filter_map(|m| self.handle_one(m))
                    .collect();
                (!responses.is_empty()).then_some(Value::Array(responses))
            }
            other => self.handle_one(other),
        }
    }

    fn handle_one(&mut self, message: Value) -> Option<Value> {
        let Value::Object(msg) = message else {
            return Some(error(
                Value::Null,
                INVALID_REQUEST,
                "a message is a JSON object",
            ));
        };
        let id = msg.get("id").cloned();
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            // A response or a malformed message; a server never answers a response.
            return id
                .filter(|i| !i.is_null())
                .map(|i| error(i, INVALID_REQUEST, "missing method"));
        };
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id.filter(|i| !i.is_null()) else {
            self.notification(method, &params);
            return None;
        };
        tracing::debug!(operation = method, "request");
        Some(match self.request(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error(id, code, &message),
        })
    }

    fn notification(&mut self, method: &str, _params: &Value) {
        match method {
            "notifications/initialized" => self.initialized = true,
            other => tracing::debug!(operation = other, "notification ignored"),
        }
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => Ok(self.initialize(params)),
            "ping" => Ok(json!({})),
            "resources/list" => Ok(
                json!({ "resources": self.surface.resources().iter().map(resource_json).collect::<Vec<_>>() }),
            ),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "resources/read" => {
                let uri = params
                    .get("uri")
                    .and_then(Value::as_str)
                    .ok_or_else(|| (INVALID_PARAMS, "params.uri is required".to_string()))?;
                match self.surface.read(uri) {
                    Ok(c) => Ok(
                        json!({ "contents": [{ "uri": c.uri, "mimeType": c.media_type, "text": c.text }] }),
                    ),
                    Err(SurfaceError::UnknownResource(u)) => {
                        Err((RESOURCE_NOT_FOUND, format!("resource not found: {u}")))
                    }
                    Err(SurfaceError::Internal(e)) => Err((INTERNAL_ERROR, e)),
                    Err(e) => Err((INVALID_PARAMS, e.to_string())),
                }
            }
            "tools/list" => Ok(
                json!({ "tools": self.surface.tools().iter().map(tool_json).collect::<Vec<_>>() }),
            ),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| (INVALID_PARAMS, "params.name is required".to_string()))?;
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                if !args.is_object() {
                    return Err((
                        INVALID_PARAMS,
                        "params.arguments must be an object".to_string(),
                    ));
                }
                match self.surface.call(name, &args) {
                    Ok(ToolOutcome::Ok(value)) => {
                        let text = serde_json::to_string_pretty(&value)
                            .unwrap_or_else(|_| value.to_string());
                        Ok(
                            json!({ "content": [{ "type": "text", "text": text }], "structuredContent": value, "isError": false }),
                        )
                    }
                    Ok(ToolOutcome::Refused(reason)) => Ok(
                        json!({ "content": [{ "type": "text", "text": reason }], "isError": true }),
                    ),
                    Err(SurfaceError::Internal(e)) => Err((INTERNAL_ERROR, e)),
                    Err(e) => Err((INVALID_PARAMS, e.to_string())),
                }
            }
            other => Err((METHOD_NOT_FOUND, format!("method not found: {other}"))),
        }
    }

    fn initialize(&mut self, params: &Value) -> Value {
        let asked = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let version = PROTOCOL_VERSIONS
            .iter()
            .find(|v| **v == asked)
            .copied()
            .unwrap_or(PROTOCOL_VERSIONS[0]);
        let index = self.surface.index();
        let summary = self.surface.registry().summary();
        json!({
            "protocolVersion": version,
            "capabilities": {
                "resources": { "subscribe": false, "listChanged": false },
                "tools": { "listChanged": false }
            },
            "serverInfo": { "name": SERVER_NAME, "title": "Majordomus", "version": self.version },
            "instructions": format!(
                "Read-only view of a repository's AI layer: {} object(s), {} capabilities ({} tools, {} resources), index state {}. Resources are majordomus://<kind>/<identity>; majordomus://repository carries the diagnostics; majordomus_capabilities lists every capability with its projections. Nothing here writes.",
                index.objects.len(),
                summary.total,
                summary.mcp_tools,
                summary.mcp_resources,
                match index.state { crate::index::State::Ok => "ok", crate::index::State::Degraded => "degraded" }
            ),
        })
    }
}

fn resource_json(r: &super::surface::Resource) -> Value {
    let mut v = json!({ "uri": r.uri, "name": r.name, "mimeType": r.media_type, "_meta": { "majordomus": r.meta } });
    if let Some(t) = &r.title {
        v["title"] = Value::String(t.clone());
    }
    if let Some(d) = &r.description {
        v["description"] = Value::String(d.clone());
    }
    v
}

fn tool_json(t: &super::surface::Tool) -> Value {
    json!({
        "name": t.name, "title": t.title, "description": t.description,
        "inputSchema": t.input_schema,
        "outputSchema": t.output_schema,
        "_meta": { "majordomus": { "id": t.id } },
        "annotations": { "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
    })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
