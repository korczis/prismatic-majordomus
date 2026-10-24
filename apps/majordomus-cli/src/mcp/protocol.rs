//! JSON-RPC 2.0 and the MCP methods this server answers, over a [`Surface`]. Transport
//! agnostic: a message in, zero or one message out. The subset spoken is the server side
//! of the protocol: `initialize`, `ping`, `resources/list`, `resources/read`,
//! `resources/templates/list`, `tools/list`, `tools/call`. Prompts are not advertised.
//! Nothing here decides what a tool may change: a tool is a capability, its effect is the
//! capability's, and its annotations are the capability's classified hints.

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

/// The `serverInfo.title` this server announces.
pub const SERVER_TITLE: &str = "Majordomus";

/// Every method a request may name. A request naming any other is answered
/// `method not found` before it is looked at, so this list is what the server serves and
/// not a description of it; `mcp.projection` renders it.
pub const METHODS: &[&str] = &[
    "initialize",
    "ping",
    "resources/list",
    "resources/read",
    "resources/templates/list",
    "tools/call",
    "tools/list",
];

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
    remote: bool,
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
            remote: false,
        }
    }

    /// Whether the message about to be handled came from another host. A transport that
    /// knows the caller's address says so before every message, because the next message
    /// of one session may come from somewhere else: a session id is a name, not a proof.
    /// A caller beyond loopback reads; a tool that changes something is refused to it
    /// unless its input authenticates it (ADR 0126).
    pub(crate) fn set_remote(&mut self, remote: bool) {
        self.remote = remote;
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
            // Only real work keeps the peer's mesh claims alive (I3990): the bridge pings
            // every 20 s whatever its client is doing, so a session stuck on a permission
            // dialog would otherwise hold an exclusive claim for as long as its server ran.
            if is_progress(&message) {
                self.surface.context().peers.progress(peer);
            }
            // ... and the same message is the heartbeat of whatever episode this connection
            // holds. A client that is working never has to send anything it would not
            // otherwise send in order to keep its episode alive, which is the difference
            // between a lifecycle and a thing clients have to remember to do (ADR 0103).
            self.surface.context().episodes.touch(peer);
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
        if !METHODS.contains(&method) {
            return Err((METHOD_NOT_FOUND, format!("method not found: {method}")));
        }
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
                match self.surface.call_from(name, &args, self.remote) {
                    Ok(ToolOutcome::Ok(value)) => {
                        // compact: a client that wants the data reads structuredContent
                        let text = value.to_string();
                        Ok(
                            json!({ "content": [{ "type": "text", "text": text }], "structuredContent": value, "isError": false }),
                        )
                    }
                    // the category travels beside the text, so a client branches on a word
                    // and never parses a sentence; no structuredContent, because the
                    // tool's output schema describes a success
                    Ok(ToolOutcome::Refused { code, reason }) => Ok(json!({
                        "content": [{ "type": "text", "text": reason }],
                        "isError": true,
                        "_meta": { "majordomus": { "error": { "code": code } } }
                    })),
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
            "serverInfo": { "name": SERVER_NAME, "title": SERVER_TITLE, "version": self.version },
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
        // The tree every answer above comes from, said where every client reads it: on
        // 2026-10-16 this repository's primary checkout was 779 commits behind its remote
        // trunk and its server answered every session's plan questions from that tree, in
        // silence (project.a-checkout-serves-the-current-trunk). Only a distance worth acting
        // on is named; a current checkout costs the client no sentence.
        let trunk =
            crate::worktree::trunk::measure_checkout(std::path::Path::new(&index.repository.root));
        if let Some(sentence) = trunk_sentence(&trunk) {
            text.push_str(&sentence);
        }
        if let Some(peer) = self.surface.peer() {
            let board = &self.surface.context().peers;
            text.push_str(&format!(
                " You are peer {peer} on this checkout's board. majordomus_peers lists every worker of this repository — this board and the board of every other checkout of it, each peer stamped with the worktree it is working in; call majordomus_announce with your intent and the paths you expect to touch so that the other clients (Claude, Codex, Gemini, ...) can avoid colliding with you."
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
            // The board itself comes last, and bounded (`PeerBoard::summary`): a client cuts
            // the instructions it is handed, and on 2026-10-22 the cut fell inside a 6 kB
            // board line, so the sentences above — the ones about this caller — were the
            // ones lost. Only this checkout's board is named here: the gathered one costs a
            // lease read and a probe per registered checkout, and `initialize` is on the
            // path of every client's first millisecond.
            text.push_str(&format!(" Attached here: {}.", board.summary()));
        }
        text
    }
}

/// What the `initialize` instructions say about the tree a server answers from, when it is
/// behind the remote trunk by a distance the policy calls lagging or stale, or by one it
/// declares nothing about. Nothing when the checkout is current or the distance could not be
/// measured. (`the_trunk_sentence_names_only_a_distance_worth_acting_on` holds each case.)
pub(crate) fn trunk_sentence(d: &crate::worktree::TrunkDistance) -> Option<String> {
    use crate::worktree::TrunkStanding;
    if !matches!(
        d.standing,
        TrunkStanding::Lagging | TrunkStanding::Stale | TrunkStanding::Unjudged
    ) {
        return None;
    }
    let behind = d.checkout.map(|c| c.behind)?;
    Some(format!(
        " This server serves a tree {behind} commit(s) behind {} ({}): its answers about the plan, the rules and the code are that old. {} Remedy: {}.",
        d.remote_trunk.as_deref().unwrap_or("the remote trunk"),
        d.standing.as_str(),
        d.message,
        d.remedy.as_deref().unwrap_or("bring the checkout current")
    ))
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
/// The wording itself is [`crate::about::writes`], the one sentence the tool uses about its
/// own writers; this function only decides which tools it names.
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
    crate::about::writes(&writers)
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
        "_meta": { "majordomus": { "id": t.id, "effect": t.effect } },
        "annotations": {
            "readOnlyHint": t.hints.read_only, "destructiveHint": t.hints.destructive,
            "idempotentHint": t.hints.idempotent, "openWorldHint": t.hints.open_world
        }
    })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    // the one place a JSON-RPC error is made: it may echo a tool name or an argument
    let message = crate::redaction::redact_secrets(message).text;
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Whether a message is real work: a request or notification of any method but `ping` and
/// the `notifications/*` a client sends on its own (cancellation, progress, initialized),
/// or a batch holding one (I3990). A response from the client is not work either. This is
/// the only signal a session's mesh claims live by, so it must stay false for everything a
/// client sends while nobody is answering it.
fn is_progress(message: &Value) -> bool {
    match message {
        Value::Array(batch) => batch.iter().any(is_progress),
        Value::Object(msg) => msg
            .get("method")
            .and_then(Value::as_str)
            .is_some_and(|m| m != "ping" && !m.starts_with("notifications/")),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keep-alive is presence and not work: a session that only pings, or only sends the
    /// notifications a client sends unattended, must lose its mesh claims (I3990).
    #[test]
    fn a_ping_is_not_progress() {
        let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
        assert!(!is_progress(&ping));
        let cancelled = json!({ "jsonrpc": "2.0", "method": "notifications/cancelled" });
        assert!(!is_progress(&cancelled));
        let answer = json!({ "jsonrpc": "2.0", "id": 7, "result": {} });
        assert!(!is_progress(&answer), "a client's response is not its work");
        assert!(!is_progress(&json!([ping.clone(), cancelled])));
        let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "majordomus_brief", "arguments": {} } });
        assert!(is_progress(&call));
        assert!(is_progress(
            &json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/read" })
        ));
        assert!(
            is_progress(&json!([ping, call])),
            "a batch with work in it is work"
        );
        assert!(!is_progress(&json!("ping")));
    }

    /// A stale or lagging tree is named with its distance, its remote and its remedy; a
    /// current one, and one that could not be measured, cost the client no sentence.
    #[test]
    fn the_trunk_sentence_names_only_a_distance_worth_acting_on() {
        use crate::worktree::{Distance, TrunkDistance, TrunkStanding, TrunkThresholds};
        let mut d = TrunkDistance {
            standing: TrunkStanding::Stale,
            local_trunk_standing: TrunkStanding::Stale,
            worktree: "/r".into(),
            on_trunk: true,
            trunk: Some("master".into()),
            remote_trunk: Some("origin/master".into()),
            remote_head: None,
            checkout: Some(Distance {
                behind: 779,
                ahead: 0,
            }),
            local_trunk: None,
            fetched_at: None,
            thresholds: TrunkThresholds::default(),
            message: "m".into(),
            remedy: Some("git merge --ff-only origin/master".into()),
        };
        let s = trunk_sentence(&d).expect("a stale tree is named");
        assert!(
            s.contains("779 commit(s) behind origin/master (stale)"),
            "{s}"
        );
        assert!(
            s.contains("Remedy: git merge --ff-only origin/master."),
            "{s}"
        );
        d.standing = TrunkStanding::Lagging;
        assert!(trunk_sentence(&d).is_some());
        d.standing = TrunkStanding::Current;
        assert_eq!(trunk_sentence(&d), None);
        d.standing = TrunkStanding::Unmeasured;
        assert_eq!(trunk_sentence(&d), None);
        // a standing worth naming with no distance under it has nothing to name
        d.standing = TrunkStanding::Unjudged;
        d.checkout = None;
        assert_eq!(trunk_sentence(&d), None);
    }

    /// The instructions every client reads on `initialize` name the distance of the tree
    /// the server answers from, when it is behind its remote trunk, and say nothing of a
    /// current one.
    #[test]
    fn the_instructions_name_a_tree_behind_its_remote_trunk() {
        use crate::worktree::trunk::fixture;
        let (repo, _origin) = fixture::layered(3, None);
        let server = || {
            let surface = crate::mcp::surface::Surface::new(repo.context().expect("a context"));
            Server::new(surface, "test")
        };
        let text = server().instructions();
        assert!(
            text.contains(" This server serves a tree 3 commit(s) behind origin/master (unjudged)"),
            "{text}"
        );
        assert!(text.contains(" Remedy: git fetch;"), "{text}");
        fixture::git(repo.root(), &["merge", "-q", "--ff-only", "origin/master"]);
        let text = server().instructions();
        assert!(!text.contains("This server serves a tree"), "{text}");
    }

    /// A client cuts the instructions it is handed, so what is said about the caller comes
    /// before the board, and the board is bounded: with twelve peers holding five long
    /// claims each, the nudge to announce is still read and the board is not what fills
    /// the instructions.
    #[test]
    fn the_caller_is_told_about_itself_before_the_board_is_listed() {
        use crate::peers::Transport;
        use crate::worktree::trunk::fixture;
        let (repo, _origin) = fixture::layered(0, None);
        let surface = crate::mcp::surface::Surface::new(repo.context().expect("a context"));
        let board = surface.context().peers.clone();
        let long = "a claim whose intent runs on for a long time and says a great deal ".repeat(6);
        for _ in 0..12 {
            let other = board.attach(Transport::Http);
            for c in 0..5 {
                board.announce_claim(&other, Some(&format!("c{c}")), &long, vec![]);
            }
        }
        let me = board.attach(Transport::Http);
        let text = Server::new(surface.for_peer(me.clone()), "test").instructions();
        let nudge = text.find(" You have not announced anything.").expect(&text);
        let listed = text.find(" Attached here: ").expect(&text);
        assert!(
            nudge < listed,
            "the caller's own sentence comes first: {text}"
        );
        let board_line = &text[listed..];
        assert!(
            board_line.chars().count() < crate::peers::SUMMARY_LINE + 200,
            "the board line is bounded ({} chars): {board_line}",
            board_line.chars().count()
        );
        assert!(board_line.contains("majordomus_peers"), "{board_line}");
    }

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

    /// A tool's annotations are what a client reads to decide whether to ask a person
    /// first, and for most of this server's life three of the four were constants:
    /// never destructive, always idempotent. The first tool that unlinked files was
    /// announced as both. They are now the capability's classified hints, and this is the
    /// assertion that they stay so: every tool of the real registry is rendered, and each
    /// annotation is compared with the policy of the capability behind it.
    #[test]
    fn a_tools_annotations_are_its_capabilitys_hints_and_a_writer_is_never_announced_as_safe() {
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_builtin(crate::capability::builtin::all())
            .build()
            .unwrap();
        let mut writers = 0;
        for c in registry.iter() {
            let Some(tool) = super::super::surface::Tool::of(c) else {
                continue;
            };
            let v = tool_json(&tool);
            let a = &v["annotations"];
            let h = c.execution.hints();
            assert_eq!(a["readOnlyHint"], h.read_only, "{}", tool.name);
            assert_eq!(a["destructiveHint"], h.destructive, "{}", tool.name);
            assert_eq!(a["idempotentHint"], h.idempotent, "{}", tool.name);
            assert_eq!(a["openWorldHint"], h.open_world, "{}", tool.name);
            assert_eq!(
                v["_meta"]["majordomus"]["effect"],
                serde_json::to_value(c.execution.effect).unwrap(),
                "{}",
                tool.name
            );
            if c.execution.effect == crate::capability::model::Effect::RepositoryMutation {
                writers += 1;
                assert_eq!(
                    a["readOnlyHint"], false,
                    "{} writes the repository",
                    tool.name
                );
                assert_eq!(
                    a["destructiveHint"], true,
                    "{} writes the repository",
                    tool.name
                );
                assert_eq!(
                    a["idempotentHint"], false,
                    "{} writes the repository",
                    tool.name
                );
            }
            if a["readOnlyHint"] == true {
                assert_eq!(c.execution.effect, crate::capability::model::Effect::Read);
            }
        }
        assert!(
            writers > 0,
            "the registry exposes no writing tool, so this proved nothing about one"
        );
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
