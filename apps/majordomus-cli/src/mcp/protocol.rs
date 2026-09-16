//! JSON-RPC 2.0 and the MCP methods this server answers, over a [`Surface`]. Transport
//! agnostic: a message in, zero or one message out. The subset spoken is the read-only
//! server side of the protocol: `initialize`, `ping`, `resources/list`, `resources/read`,
//! `resources/templates/list`, `tools/list`, `tools/call`. Prompts are not advertised.

use std::sync::Arc;

use serde::ser::SerializeMap;
use serde::Serialize;
use serde_json::value::RawValue;
use serde_json::{json, Value};

use crate::peers::ClientInfo;

/// One outgoing message. Most are built values; the two listings are prepared once and
/// carried as raw JSON, so answering `tools/list` or `resources/list` copies bytes and
/// builds nothing. A transport serialises a reply with `serde_json::to_string`; a caller
/// that needs the data as a value takes [`Reply::into_value`].
#[derive(Debug)]
pub enum Reply {
    /// A built message.
    Value(Value),
    /// A response whose result is prepared JSON.
    Prepared {
        /// The request id.
        id: Value,
        /// The serialised result object.
        result: Arc<Box<RawValue>>,
    },
    /// The responses of a batch.
    Batch(Vec<Reply>),
}

impl Reply {
    /// The reply as a value (a prepared result is parsed).
    pub fn into_value(self) -> Value {
        match self {
            Reply::Value(v) => v,
            Reply::Prepared { id, result } => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": serde_json::from_str::<Value>(result.get()).unwrap_or(Value::Null),
            }),
            Reply::Batch(items) => Value::Array(items.into_iter().map(Reply::into_value).collect()),
        }
    }
}

impl Serialize for Reply {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Reply::Value(v) => v.serialize(serializer),
            Reply::Prepared { id, result } => {
                let mut m = serializer.serialize_map(Some(3))?;
                m.serialize_entry("jsonrpc", "2.0")?;
                m.serialize_entry("id", id)?;
                m.serialize_entry("result", &**result)?;
                m.end()
            }
            Reply::Batch(items) => items.serialize(serializer),
        }
    }
}

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
/// The protocol state over a surface: whether the client has initialised, who it said it
/// was, the version to announce, and the HTTP endpoint of the shared server this session
/// belongs to, when there is one.
pub struct Server {
    surface: Surface,
    version: &'static str,
    initialized: bool,
    client: Option<ClientInfo>,
    endpoint: Option<String>,
}

impl Server {
    /// A server over a surface, announcing `version` as its own.
    pub fn new(surface: Surface, version: &'static str) -> Self {
        Server {
            surface,
            version,
            initialized: false,
            client: None,
            endpoint: None,
        }
    }

    /// The base URL of the shared server (`http://127.0.0.1:8741`) the `initialize`
    /// instructions name, so that a client learns where Swagger UI and the peers are.
    pub fn with_endpoint(mut self, url: Option<String>) -> Self {
        self.endpoint = url;
        self
    }

    /// The surface being served.
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    /// Has the client completed `initialize`?
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// What the client said in `initialize`, when it has.
    pub fn client(&self) -> Option<&ClientInfo> {
        self.client.as_ref()
    }

    /// Resume a session another process was serving: the client already initialised
    /// there and will not do so again. The peer is identified on the board as it was.
    pub fn resume(&mut self, client: ClientInfo) {
        if let Some(peer) = self.surface.peer() {
            self.surface.context().peers.identify(peer, client.clone());
        }
        self.client = Some(client);
        self.initialized = true;
    }

    /// The response to a parse failure of an incoming line.
    pub fn parse_error(reason: &str) -> Value {
        error(Value::Null, PARSE_ERROR, &format!("parse error: {reason}"))
    }

    /// Handle one decoded message. A notification yields `None`; a batch yields a batch of
    /// the responses its requests produced, or `None` when it held only notifications.
    pub fn handle(&mut self, message: Value) -> Option<Reply> {
        if let Some(peer) = self.surface.peer() {
            self.surface.context().peers.touch(peer);
        }
        match message {
            Value::Array(batch) => {
                if batch.is_empty() {
                    return Some(Reply::Value(error(
                        Value::Null,
                        INVALID_REQUEST,
                        "empty batch",
                    )));
                }
                let responses: Vec<Reply> = batch
                    .into_iter()
                    .filter_map(|m| self.handle_one(m))
                    .collect();
                (!responses.is_empty()).then_some(Reply::Batch(responses))
            }
            other => self.handle_one(other),
        }
    }

    fn handle_one(&mut self, message: Value) -> Option<Reply> {
        let Value::Object(msg) = message else {
            return Some(Reply::Value(error(
                Value::Null,
                INVALID_REQUEST,
                "a message is a JSON object",
            )));
        };
        let id = msg.get("id").cloned();
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            // A response or a malformed message; a server never answers a response.
            return id
                .filter(|i| !i.is_null())
                .map(|i| Reply::Value(error(i, INVALID_REQUEST, "missing method")));
        };
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id.filter(|i| !i.is_null()) else {
            self.notification(method, &params);
            return None;
        };
        tracing::debug!(operation = method, "request");
        // the two listings are prepared once and copied, never rebuilt
        match method {
            "tools/list" => {
                return Some(Reply::Prepared {
                    id,
                    result: self.surface.tools_result(),
                })
            }
            "resources/list" => {
                return Some(Reply::Prepared {
                    id,
                    result: self.surface.resources_result(),
                })
            }
            _ => {}
        }
        Some(Reply::Value(match self.request(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error(id, code, &message),
        }))
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
                json!({ "resources": Value::Array(self.surface.resources_json().as_ref().clone()) }),
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
            "tools/list" => {
                Ok(json!({ "tools": Value::Array(self.surface.tools_json().as_ref().clone()) }))
            }
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
                        // compact: a client that wants the data reads structuredContent
                        let text = value.to_string();
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
        let client = ClientInfo::from_initialize(params);
        if let Some(peer) = self.surface.peer() {
            self.surface.context().peers.identify(peer, client.clone());
            tracing::info!(peer = %peer, client = %client.name, version = %client.version, "peer initialized");
        }
        self.client = Some(client);
        json!({
            "protocolVersion": version,
            "capabilities": {
                "resources": { "subscribe": false, "listChanged": false },
                "tools": { "listChanged": false }
            },
            "serverInfo": { "name": SERVER_NAME, "title": "Majordomus", "version": self.version },
            "instructions": self.instructions(),
        })
    }

    /// The text a client reads after `initialize`: what is served, what of it writes, where
    /// the shared server is, and who else is attached.
    pub fn instructions(&self) -> String {
        let index = self.surface.index();
        let registry = self.surface.registry();
        let summary = registry.summary();
        let mut text = format!(
            "{} This repository: {} object(s), {} capabilities ({} tools, {} resources), index state {}. Resources are majordomus://<kind>/<identity>; majordomus://repository carries the diagnostics; majordomus_capabilities lists every capability with its projections. {}",
            crate::about::SUMMARY,
            index.objects.len(),
            summary.total,
            summary.mcp_tools,
            summary.mcp_resources,
            match index.state { crate::index::State::Ok => "ok", crate::index::State::Degraded => "degraded" },
            effects_sentence(&registry),
        );
        if let Some(url) = &self.endpoint {
            text.push_str(&format!(
                // "of this checkout", not "for this repository": a linked worktree is a
                // checkout with a server of its own, and on 2026-09-11 this repository had
                // seven of them. The sentence a client reads on every initialize was the
                // one that told nine agents they had seen each other (ADR 0044).
                " This session belongs to the shared server of this checkout at {url}: its home page at {url}/ lists every surface, Swagger UI is {url}/swagger, OpenAPI {url}/openapi.json, MCP over HTTP {url}/mcp."
            ));
        }
        if let Some(peer) = self.surface.peer() {
            let board = &self.surface.context().peers;
            text.push_str(&format!(
                // Only this checkout's board is named here: the gathered one costs a lease
                // read and a probe per registered checkout, and `initialize` is on the path
                // of every client's first millisecond. The words point at the capability
                // that pays for the wide answer rather than paying for it here.
                " You are peer {peer} on this checkout's board; attached here: {}. majordomus_peers lists every worker of this repository — this board and the board of every other checkout of it, each peer stamped with the worktree it is working in; call majordomus_announce with your intent and the paths you expect to touch so that the other clients (Claude, Codex, Gemini, ...) can avoid colliding with you.",
                board.summary()
            ));
            // The nudge that matters is delivered here, on every initialize, which is also
            // every reconnect. A session whose transport was re-established keeps its work
            // and loses its place on the board, and nothing else in the protocol will ever
            // mention it again: this repository spent three hours with a session invisible
            // to eight others for exactly that reason. So the instructions say what is
            // true of THIS caller rather than what is true in general.
            let peers = board.list();
            if !peers
                .iter()
                .any(|p| Some(&p.id) == self.surface.peer() && p.announcement.is_some())
            {
                text.push_str(" You have not announced anything. If you have worked in this repository before in another session, the board does not know it: announce now, before you start, and again if this connection is ever re-established — your peer id is a position on this board, handed out again on a reconnect, and never your identity.");
            }
            let silent = peers
                .iter()
                .filter(|p| {
                    p.attached && p.announcement.is_none() && Some(&p.id) != self.surface.peer()
                })
                .count();
            if silent > 0 {
                text.push_str(&format!(
                    " {silent} attached peer(s) have announced nothing, so the board understates who is here."
                ));
            }
            let departed = peers.iter().filter(|p| !p.attached).count();
            if departed > 0 {
                text.push_str(&format!(
                    " {departed} peer(s) on the board have gone but their announcements stand: what they claimed is still claimed until somebody says otherwise."
                ));
            }
        }
        text
    }
}

/// What the `initialize` instructions say about writing, read off the registry instead of
/// stated by hand.
///
/// The sentence used to be the constant "Nothing here writes to the repository." It was true
/// when it was written and false from [ADR 0040] onwards: `plan.transition` stamps a field
/// into a tracked issue record and appends a ledger event, and it is exposed as an MCP tool.
/// An agent that reads its instructions and believes them would then call a writer thinking
/// it was reading — the one thing the instructions exist to prevent. So the sentence is
/// derived: every capability whose MCP exposure is a tool and whose effect is
/// [`Effect::RepositoryMutation`] is named, in the registry's own order, and only a registry
/// that holds no such tool gets the old sentence back.
///
/// The assertion is `the_instructions_never_claim_read_only_while_a_writer_is_exposed`.
///
/// [ADR 0040]: ../../../.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md
/// [`Effect::RepositoryMutation`]: crate::capability::model::Effect::RepositoryMutation
pub(crate) fn effects_sentence(registry: &crate::capability::CapabilityRegistry) -> String {
    let writers: Vec<&str> = registry
        .iter()
        .filter(|c| c.execution.effect == crate::capability::model::Effect::RepositoryMutation)
        .filter_map(|c| c.exposure.mcp.as_ref()?.tool.as_deref())
        .collect();
    match writers.len() {
        0 => "Nothing here writes to the repository.".to_string(),
        1 => format!(
            "One tool here writes to the repository — {} — and every other one only reads; each tool's readOnlyHint says which it is.",
            writers[0]
        ),
        n => format!(
            "{n} tools here write to the repository — {} — and every other one only reads; each tool's readOnlyHint says which it is.",
            writers.join(", ")
        ),
    }
}

pub(crate) fn resource_json(r: &super::surface::Resource) -> Value {
    let mut v = json!({ "uri": r.uri, "name": r.name, "mimeType": r.media_type, "_meta": { "majordomus": r.meta } });
    if let Some(t) = &r.title {
        v["title"] = Value::String(t.clone());
    }
    if let Some(d) = &r.description {
        v["description"] = Value::String(d.clone());
    }
    v
}

pub(crate) fn tool_json(t: &super::surface::Tool) -> Value {
    json!({
        "name": t.name, "title": t.title, "description": t.description,
        "inputSchema": t.input_schema,
        "outputSchema": t.output_schema,
        "_meta": { "majordomus": { "id": t.id } },
        "annotations": { "readOnlyHint": t.read_only, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false }
    })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `initialize` instructions are the one thing every client reads before it does
    /// anything, and for most of this server's life they ended with the constant "Nothing
    /// here writes to the repository." That sentence stopped being true when the first
    /// mutating capability was exposed as a tool, and nothing said so: a constant cannot go
    /// stale loudly. So the sentence is derived, and this is the assertion that it stays
    /// derived — if a tool that writes the repository is exposed and the instructions still
    /// promise a read-only surface, this fails.
    #[test]
    fn the_instructions_never_claim_read_only_while_a_writer_is_exposed() {
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_builtin(crate::capability::builtin::all())
            .build()
            .unwrap();
        let writers: Vec<&str> = registry
            .iter()
            .filter(|c| c.execution.effect == crate::capability::model::Effect::RepositoryMutation)
            .filter_map(|c| c.exposure.mcp.as_ref()?.tool.as_deref())
            .collect();
        let sentence = effects_sentence(&registry);

        if writers.is_empty() {
            assert_eq!(sentence, "Nothing here writes to the repository.");
        } else {
            assert!(
                !sentence.contains("Nothing here writes"),
                "{} mutating tool(s) are exposed ({}) and the instructions still tell every \
                 client the surface writes nothing: {sentence}",
                writers.len(),
                writers.join(", ")
            );
            for w in &writers {
                assert!(
                    sentence.contains(w),
                    "the instructions do not name the writing tool {w}: {sentence}"
                );
            }
        }
    }

    /// The sentence over a registry this repository does not happen to hold, so that the
    /// assertion above cannot pass merely because the real registry has a writer today: a
    /// registry with nothing in it writes nothing, and says so.
    #[test]
    fn a_registry_with_no_writer_still_says_nothing_writes() {
        let empty = crate::capability::CapabilityRegistry::builder()
            .build()
            .unwrap();
        assert_eq!(
            effects_sentence(&empty),
            "Nothing here writes to the repository."
        );
    }
}
