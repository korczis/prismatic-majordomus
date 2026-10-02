//! The `episodes` module: the execution episode of a client that has no provider hooks,
//! opened and closed by the MCP connection itself (ADR 0103).
//!
//! Three capabilities, and the shape of them is the argument. `attach` is what a client
//! calls to say *this connection is carrying this piece of work*, naming the work with its
//! own durable identity; `detach` is what it calls when the work is done; `list` is what
//! anybody reads. `initialize` is not on that list, deliberately: a connection that never
//! attaches holds no episode and writes nothing, so a client that merely wants to read the
//! repository's AI layer is a peer and never becomes a worker with a record. That is also
//! what keeps `test/cases/90_mcp_shared_server.sh` true — serving changes the repository not
//! at all — while an episode still opens for the client that asks for one.
//!
//! Everything after `attach` is automatic and needs no cooperation: the client's own traffic
//! is the heartbeat, losing the connection detaches rather than closes, and coming back under
//! the same identity resumes. See [`crate::episodes`] for why a peer id cannot be that
//! identity.
//!
//! ```
//! use majordomus_cli::capability::builtin::episodes;
//! let m = episodes::module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, ["episodes.list", "episodes.attach", "episodes.detach"]);
//! // only the listing reads; attaching and detaching are commands with an effect
//! let reads: Vec<bool> = m.capabilities.iter().map(|e| e.capability.kind.is_read_only()).collect();
//! assert_eq!(reads, [true, false, false]);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CapabilityKind, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::episodes::{CloseReason, ConnectionEpisode, REATTACH_GRACE};
use crate::{capability, module};

use super::{get, mcp, post, Empty};

/// The provider an episode drawn by a connection is recorded under.
///
/// Always `generic`, and never the vendor behind the client, because the provider field says
/// *what drew the boundary* and what drew this one was the connection. A Claude Code window
/// whose own `SessionEnd` hook closes its episode is a `claude-code` episode; the same window
/// calling `attach` here would be a second, generic one, which is why a client with working
/// hooks has no reason to call it. `share/providers.yaml` declares
/// `generic.offers.episode: connection` with the evidence, and `capture session` admits it
/// without an adapter line for exactly that reason.
pub const CONNECTION_PROVIDER: &str = "generic";

// ---------------------------------------------------------------- episodes.attach

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `episodes.attach`: which piece of work this connection is carrying.
///
/// The identity is the only field, and an unknown one is refused rather than ignored, so a
/// client that sends a peer id under another name learns it at once:
///
/// ```
/// use majordomus_cli::capability::builtin::episodes::AttachInput;
/// let input: AttachInput = serde_json::from_str(r#"{"external_id":"conv-42"}"#).unwrap();
/// assert_eq!(input.external_id, "conv-42");
/// assert!(serde_json::from_str::<AttachInput>(r#"{"external_id":"x","peer":"p1"}"#).is_err());
/// assert!(serde_json::from_str::<AttachInput>("{}").is_err(), "the identity is required");
/// ```
pub struct AttachInput {
    /// The client's own durable name for this episode — its conversation id, thread id, or
    /// whatever it calls the sitting it is in.
    ///
    /// **It must survive a reconnect**, because that is the whole of its job: a client that
    /// comes back under the same identity is given its own episode again rather than a
    /// second one. Anything the client can reproduce will do; what will not do is this
    /// server's peer id, which is handed out per connection and is a different string every
    /// time the client reconnects.
    pub external_id: String,
}

impl BenchmarkCases for AttachInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "default",
            AttachInput {
                external_id: "benchmark-episode".into(),
            },
        )]
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `episodes.attach`: the episode the connection now holds, whether it was
/// resumed rather than opened, and how long it outlives a lost connection.
///
/// ```
/// use majordomus_cli::capability::builtin::episodes::Attached;
/// let schema = serde_json::to_value(schemars::schema_for!(Attached)).unwrap();
/// for field in ["episode", "resumed", "reattach_grace_seconds"] {
///     assert!(schema["required"].as_array().unwrap().iter().any(|f| f == field), "{field}");
/// }
/// ```
pub struct Attached {
    /// The episode this connection now holds.
    pub episode: ConnectionEpisode,
    /// Whether an episode already under this identity was resumed rather than a new one
    /// opened. True after a reconnect, a client restart or a crash, and the field a caller
    /// checks to know whether it is continuing something.
    pub resumed: bool,
    /// How long this episode survives its connection before the reaper closes it, in
    /// seconds. Stated rather than left to be discovered: a client that knows the grace can
    /// decide whether reconnecting is worth it.
    pub reattach_grace_seconds: u64,
}

fn episodes_attach(ctx: &Context, input: AttachInput) -> Result<Attached, CapabilityError> {
    let Some(caller) = &ctx.caller else {
        return Err(CapabilityError::Refused(
            "attach needs an MCP session: this call came through an interface with no connection to bind an episode to (call the majordomus_session_attach tool)".into(),
        ));
    };
    let id = input.external_id.trim();
    if id.is_empty() {
        return Err(CapabilityError::InvalidInput(
            "argument 'external_id' is required and must not be blank: it is what a reconnecting client is recognised by".into(),
        ));
    }
    // A newline in the identity would split the payload the driver writes on stdin into two
    // events. The escaping in `episodes::json_string` would survive it, but an identity with
    // a line break in it is a mistake on the client's side either way and is better refused
    // here, where the message can say so, than carried into a record nobody can read back.
    if id.contains('\n') || id.contains('\r') {
        return Err(CapabilityError::InvalidInput(
            "argument 'external_id' must be one line".into(),
        ));
    }
    let (episode, resumed) = ctx.episodes.attach(caller, id, CONNECTION_PROVIDER);
    Ok(Attached {
        episode,
        resumed,
        reattach_grace_seconds: REATTACH_GRACE.as_secs(),
    })
}

// ---------------------------------------------------------------- episodes.detach

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `episodes.detach`: the episode to close.
///
/// Empty is a valid input, meaning the episode the calling connection holds; over an
/// interface with no connection that input is refused, since there is nothing to take
/// "this one's episode" from.
///
/// ```
/// use majordomus_cli::capability::builtin::episodes::DetachInput;
/// let mine: DetachInput = serde_json::from_str("{}").unwrap();
/// assert!(mine.external_id.is_none());
/// let named: DetachInput = serde_json::from_str(r#"{"external_id":"conv-42"}"#).unwrap();
/// assert_eq!(named.external_id.as_deref(), Some("conv-42"));
/// assert_eq!(serde_json::to_string(&mine).unwrap(), "{}", "an omitted id is not written as null");
/// ```
pub struct DetachInput {
    /// The episode to close. Omitted, it is whichever one this connection holds, which is
    /// what a client ending its own sitting means.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
}

impl BenchmarkCases for DetachInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("default", DetachInput { external_id: None })]
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `episodes.detach`: the episode as it was closed, no longer held by any
/// connection, with what the repository's end event reported.
///
/// ```
/// use majordomus_cli::capability::builtin::episodes::Detached;
/// let schema = serde_json::to_value(schemars::schema_for!(Detached)).unwrap();
/// assert_eq!(schema["required"], serde_json::json!(["episode"]));
/// ```
pub struct Detached {
    /// The episode as it was closed, its `repository` field carrying what the repository's
    /// own end event reported.
    pub episode: ConnectionEpisode,
}

fn episodes_detach(ctx: &Context, input: DetachInput) -> Result<Detached, CapabilityError> {
    let id = match input.external_id.as_deref().map(str::trim) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => {
            // Refused, as `attach` is over the same interface: the input is well formed, and
            // what is missing is a connection to take "this one's episode" from.
            let Some(caller) = &ctx.caller else {
                return Err(CapabilityError::Refused(
                    "detach without 'external_id' needs an MCP session: this call came through an interface with no connection, so there is no 'whichever episode this one holds'; name the episode to close".into(),
                ));
            };
            ctx.episodes
                .of_peer(caller)
                .map(|e| e.external_id)
                .ok_or_else(|| {
                    CapabilityError::Refused(format!(
                        "peer {caller} holds no episode; call majordomus_session_attach first, or name the episode to close"
                    ))
                })?
        }
    };
    let episode = ctx
        .episodes
        .close(&id, CloseReason::Detach)
        .ok_or_else(|| CapabilityError::NotFound(format!("no episode '{id}' on this server")))?;
    Ok(Detached { episode })
}

// ---------------------------------------------------------------- episodes.list

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `episodes.list`: every episode this server holds, counted by state, and the
/// one the caller holds. In memory only — it is the association between live connections and
/// the repository's records, which stops being true when the process stops.
///
/// The two counts partition the list, and `mine` is left out of the JSON rather than written
/// as `null` when the caller holds nothing, which is always so over plain HTTP:
///
/// ```
/// use majordomus_cli::capability::builtin::episodes::EpisodeList;
/// let none = EpisodeList { open: 0, detached: 0, episodes: vec![], mine: None };
/// let json = serde_json::to_value(&none).unwrap();
/// assert_eq!(json, serde_json::json!({"open": 0, "detached": 0, "episodes": []}));
/// let back: EpisodeList = serde_json::from_value(json).unwrap();
/// assert_eq!(back.open + back.detached, back.episodes.len());
/// ```
pub struct EpisodeList {
    /// How many are open — held by a connection that is still there.
    pub open: usize,
    /// How many are detached, waiting for a client to come back inside the grace. Not a
    /// fault count: this is where a crashed client's work waits to be resumed.
    pub detached: usize,
    /// Every episode, open and detached, in external-identity order.
    pub episodes: Vec<ConnectionEpisode>,
    /// The episode the calling connection holds, when it holds one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mine: Option<ConnectionEpisode>,
}

fn episodes_list(ctx: &Context, _: Empty) -> Result<EpisodeList, CapabilityError> {
    let episodes = ctx.episodes.list();
    Ok(EpisodeList {
        open: episodes
            .iter()
            .filter(|e| e.state == crate::episodes::ConnectionEpisodeState::Open)
            .count(),
        detached: episodes
            .iter()
            .filter(|e| e.state == crate::episodes::ConnectionEpisodeState::Detached)
            .count(),
        mine: ctx.caller.as_ref().and_then(|c| ctx.episodes.of_peer(c)),
        episodes,
    })
}

/// The module the registry composes: one read and two commands, each declared once here and
/// projected to HTTP and MCP; none to the command line, since a command-line call has no
/// connection to hold an episode.
///
/// ```
/// use majordomus_cli::capability::builtin::episodes;
/// let m = episodes::module();
/// assert_eq!(m.id.as_str(), "episodes");
/// assert!(m.capabilities.iter().all(|e| e.capability.id.as_str().starts_with("episodes.")));
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "episodes",
        title: "Episodes",
        description: "The execution episodes this shared server holds: one per client that asked for one, opened when the client attaches, kept alive by its own traffic, detached rather than closed when the connection goes, and closed on a deliberate detach, on shutdown, or by the reaper when nothing came back. The episode boundary for a client with no provider hooks of its own (ADR 0103).",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "episodes.list",
                title: "List episodes",
                description: "Every execution episode this shared server holds, open and detached, with the connection holding each, when its client last spoke, how many connections have carried it, and what the repository's own episode command reported. In-memory: what survives the process is the repository's session record.",
                input: Empty,
                output: EpisodeList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_episodes"), http: get("/api/v1/episodes"), cli: None },
                tags: ["episodes", "session", "coordination"],
                handler: episodes_list,
            },
            capability! {
                id: "episodes.attach",
                kind: CapabilityKind::Command,
                title: "Attach this connection to an episode",
                description: "Open an execution episode for the calling client, or resume the one it already had. 'external_id' is the client's own durable name for the sitting — its conversation or thread id — and must survive a reconnect: a client that comes back under the same identity is given its own episode again, under whatever peer id it now has, rather than a second one. This server's peer id will not do; it is handed out per connection. After this the client's own traffic keeps the episode alive, losing the connection detaches rather than closes it, and it is closed by majordomus_session_detach, by the server stopping, or by the reaper once the reattach grace has passed. The repository's episode is opened by the same command a provider hook runs, and what it reported is in the answer. Needs an MCP session: over plain HTTP there is no connection to bind to.",
                input: AttachInput,
                output: Attached,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_session_attach"), http: post("/api/v1/episodes/attach"), cli: None },
                tags: ["episodes", "session", "coordination"],
                handler: episodes_attach,
            },
            capability! {
                id: "episodes.detach",
                kind: CapabilityKind::Command,
                title: "Close this connection's episode",
                description: "Close an execution episode deliberately: the work is done, not merely interrupted, and the repository's record says so. Without 'external_id' it is whichever episode the calling connection holds, which needs an MCP session: over plain HTTP there is no connection, so the call is refused and the episode must be named. A client that simply goes away does not need this — its episode detaches and the reaper closes it as interrupted — and the difference between those two records is the one thing about an ended episode that changes what somebody does next.",
                input: DetachInput,
                output: Detached,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_session_detach"), http: post("/api/v1/episodes/detach"), cli: None },
                tags: ["episodes", "session", "coordination"],
                handler: episodes_detach,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist and every projection is derived
    /// from it; a refactor that dropped an exposure would still compile. This is the
    /// assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "episodes");
        let expected: &[(&str, &str, &str)] = &[
            ("episodes.list", "majordomus_episodes", "/api/v1/episodes"),
            (
                "episodes.attach",
                "majordomus_session_attach",
                "/api/v1/episodes/attach",
            ),
            (
                "episodes.detach",
                "majordomus_session_detach",
                "/api/v1/episodes/detach",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
        }
    }
}
