//! The `executions` module: starting a capability as an execution, reading what it is
//! doing, and asking it to stop.
//!
//! Every capability here is about executions; none of them is an execution engine of its
//! own. What runs the work is [`crate::execution::ExecutionEngine`], reached through the
//! context every handler already has, so a browser posting to `/api/v1/executions/start`,
//! an agent calling `majordomus_execution_start` and a terminal running `majordomus run`
//! reach one implementation and one store.
//!
//! There is no capability here that lists what may be run: that is `capabilities.list`,
//! which already answers it, and a second list would be the same fact written twice. Every
//! executable capability of the registry can be started as an execution, and the
//! descriptor's own `execution` policy is what says what running it means.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CapabilityKind, Exposure, McpExposure, McpResource, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CanonicalSchema;
use crate::execution::{
    Actor, ActorKind, CancelOutcome, Execution, ExecutionEvent, ExecutionId, ExecutionState,
    SubmitError, PROTOCOL_VERSION,
};
use crate::{capability, module};

use super::{get, mcp, post, Empty};

/// The MCP resource the list of executions is read at.
pub const EXECUTIONS_URI: &str = "majordomus://executions";

/// The MCP resource the live channel's own contract is read at.
pub const EXECUTION_PROTOCOL_URI: &str = "majordomus://executions/protocol";

/// The most executions one listing answers with.
pub const MAX_LIST: usize = 200;

/// The most events one page of history answers with.
pub const MAX_EVENTS: usize = 500;

// ---------------------------------------------------------------- shared views

/// Where to read more about one execution, in this server's own terms.
///
/// Every value is derived from the registry's declared routes and from the live channel's
/// own constant, so a route that moves moves here too and no client holds a path this
/// server does not serve.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecutionLinks {
    /// The snapshot: `GET /api/v1/executions/get?id=…`.
    #[serde(rename = "self")]
    pub itself: String,
    /// The retained history: `GET /api/v1/executions/events?id=…`.
    pub events: String,
    /// Where to ask it to stop: `POST /api/v1/executions/cancel`.
    pub cancel: String,
    /// The live channel, from the beginning: `GET /events?execution=…`.
    pub websocket: String,
    /// The page a person opens.
    pub cockpit: String,
}

impl ExecutionLinks {
    /// The links for one execution, built from what the registry declares.
    pub fn of(ctx: &Context, id: &ExecutionId) -> Self {
        let route = |capability: &str| {
            ctx.registry
                .get(capability)
                .and_then(|c| c.exposure.http.as_ref())
                .map(|h| h.path.clone())
                .unwrap_or_default()
        };
        let query = crate::http::router::percent_encode(id.as_str());
        ExecutionLinks {
            itself: format!("{}?id={query}", route("executions.get")),
            events: format!("{}?id={query}", route("executions.events")),
            cancel: route("executions.cancel"),
            websocket: format!("{}?execution={query}", crate::http::events::PATH),
            cockpit: format!("{}/executions/{query}", crate::cockpit::PREFIX),
        }
    }
}

/// One execution with the links to everything else about it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecutionView {
    /// The execution.
    #[serde(flatten)]
    pub execution: Execution,
    /// Where to read more.
    pub links: ExecutionLinks,
}

impl ExecutionView {
    fn of(ctx: &Context, execution: Execution) -> Self {
        let links = ExecutionLinks::of(ctx, &execution.id);
        ExecutionView { execution, links }
    }
}

fn parse_id(text: &str) -> Result<ExecutionId, CapabilityError> {
    ExecutionId::parse(text).ok_or_else(|| {
        CapabilityError::InvalidInput(format!(
            "'{text}' is not an execution id; they look like x-20260908T010203Z-0a1b2c3d"
        ))
    })
}

fn actor(ctx: &Context) -> Actor {
    match &ctx.caller {
        Some(peer) => Actor {
            kind: ActorKind::Mcp,
            peer: Some(peer.to_string()),
        },
        None => Actor::of(ActorKind::Http),
    }
}

// ---------------------------------------------------------------- executions.start

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `executions.start`: which capability to run, and with what.
pub struct StartInput {
    /// The canonical id of the capability to run (`health.report`, `objects.verify`).
    pub capability: String,
    /// Its input, as its own input schema describes it; an empty object when it takes none.
    #[serde(default)]
    pub input: Option<Value>,
}

impl BenchmarkCases for StartInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "demonstrate",
            StartInput {
                capability: "executions.demonstrate".into(),
                input: Some(serde_json::json!({ "steps": 1, "delay_ms": 0 })),
            },
        )]
    }
}

fn executions_start(ctx: &Context, input: StartInput) -> Result<ExecutionView, CapabilityError> {
    let payload = input.input.unwrap_or_else(|| serde_json::json!({}));
    match ctx
        .executions
        .submit(ctx, &input.capability, payload, actor(ctx))
    {
        Ok(execution) => Ok(ExecutionView::of(ctx, execution)),
        Err(SubmitError::UnknownCapability(id)) => Err(CapabilityError::NotFound(format!(
            "no capability '{id}'; the executable ones are listed by capabilities.list"
        ))),
        Err(e @ SubmitError::NotExecutable(_)) => Err(CapabilityError::Refused(e.to_string())),
        Err(SubmitError::InvalidInput(m)) => Err(CapabilityError::InvalidInput(m)),
        Err(e @ SubmitError::Unavailable(_)) => Err(CapabilityError::Refused(e.to_string())),
    }
}

// ---------------------------------------------------------------- executions.list

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `executions.list`.
pub struct ListInput {
    /// Only executions in this state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<ExecutionState>,
    /// Only executions of this capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    /// How many, newest first; the default and the bound are both `200`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl BenchmarkCases for ListInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // a case that binds a parameter: a listing with none bound is the same request with
        // an empty query string, and the OpenAPI document would then show an operation with
        // parameters and no example of any of them
        vec![NamedCase::new(
            "recent",
            ListInput {
                state: None,
                capability: None,
                limit: Some(50),
            },
        )]
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `executions.list`.
pub struct ExecutionList {
    /// How many are answered here.
    pub count: usize,
    /// How many this process remembers in total, before the filter.
    pub remembered: usize,
    /// How many are not finished.
    pub active: usize,
    /// How many are waiting for a worker.
    pub queued: usize,
    /// How many live channels are following them.
    pub live_channels: usize,
    /// The executions, newest first.
    pub executions: Vec<ExecutionView>,
}

fn executions_list(ctx: &Context, input: ListInput) -> Result<ExecutionList, CapabilityError> {
    let store = ctx.executions.store();
    let limit = input.limit.unwrap_or(MAX_LIST).min(MAX_LIST);
    let executions = store.list(input.state, input.capability.as_deref(), limit);
    Ok(ExecutionList {
        count: executions.len(),
        remembered: store.len(),
        active: store.active(),
        queued: ctx.executions.queued(),
        live_channels: crate::http::events::open_connections(),
        executions: executions
            .into_iter()
            .map(|e| ExecutionView::of(ctx, e))
            .collect(),
    })
}

// ---------------------------------------------------------------- executions.get

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `executions.get`.
pub struct GetInput {
    /// The execution's id.
    pub id: String,
}

impl BenchmarkCases for GetInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        Vec::new()
    }
}

fn executions_get(ctx: &Context, input: GetInput) -> Result<ExecutionView, CapabilityError> {
    let id = parse_id(&input.id)?;
    ctx.executions
        .store()
        .get(&id)
        .map(|e| ExecutionView::of(ctx, e))
        .ok_or_else(|| {
            CapabilityError::NotFound(format!(
                "no execution {id} in this process; executions live as long as the server that ran them"
            ))
        })
}

// ---------------------------------------------------------------- executions.events

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `executions.events`.
pub struct EventsInput {
    /// The execution's id.
    pub id: String,
    /// Only events after this sequence number: the cursor a reconnecting client holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    /// How many, oldest first; the default and the bound are both `500`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl BenchmarkCases for EventsInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        Vec::new()
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `executions.events`: one page of an execution's retained history.
pub struct EventHistory {
    /// The execution the events belong to.
    pub execution_id: String,
    /// Where the execution stands now, so a client needs one request rather than two.
    pub state: ExecutionState,
    /// The events, oldest first.
    pub events: Vec<ExecutionEvent>,
    /// The sequence of the last event here: the cursor for the next page, and the one to
    /// open the live channel with.
    pub last_sequence: u64,
    /// Whether more events follow this page right now.
    pub more: bool,
    /// Whether events before this page had already been dropped by the store's bound.
    pub truncated: bool,
}

fn executions_events(ctx: &Context, input: EventsInput) -> Result<EventHistory, CapabilityError> {
    let id = parse_id(&input.id)?;
    let store = ctx.executions.store();
    let snapshot = store
        .get(&id)
        .ok_or_else(|| CapabilityError::NotFound(format!("no execution {id} in this process")))?;
    let limit = input.limit.unwrap_or(MAX_EVENTS).min(MAX_EVENTS);
    let page = store
        .events(&id, input.after.unwrap_or(0), limit)
        .ok_or_else(|| CapabilityError::NotFound(format!("no execution {id} in this process")))?;
    Ok(EventHistory {
        execution_id: id.to_string(),
        state: snapshot.state,
        events: page.events.iter().map(|e| (**e).clone()).collect(),
        last_sequence: page.last_sequence,
        more: page.more,
        truncated: page.truncated,
    })
}

// ---------------------------------------------------------------- executions.cancel

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `executions.cancel`.
pub struct CancelInput {
    /// The execution's id.
    pub id: String,
}

impl BenchmarkCases for CancelInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        Vec::new()
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `executions.cancel`.
pub struct CancelReport {
    /// What asking achieved: `requested`, `already_requested` or `already_finished`.
    pub outcome: String,
    /// Whether the capability declares that it looks at its cancellation flag. When it
    /// does not, the request is recorded and the execution runs to completion.
    pub cancellable: bool,
    /// The execution as it stands after the request.
    pub execution: ExecutionView,
}

fn executions_cancel(ctx: &Context, input: CancelInput) -> Result<CancelReport, CapabilityError> {
    let id = parse_id(&input.id)?;
    let by = match &ctx.caller {
        Some(peer) => peer.to_string(),
        None => "client".to_string(),
    };
    let outcome = match ctx.executions.cancel(&id, &by) {
        CancelOutcome::Requested => "requested",
        CancelOutcome::AlreadyRequested => "already_requested",
        CancelOutcome::AlreadyFinished(_) => "already_finished",
        CancelOutcome::Unknown => {
            return Err(CapabilityError::NotFound(format!(
                "no execution {id} in this process"
            )))
        }
    };
    let execution =
        ctx.executions.store().get(&id).ok_or_else(|| {
            CapabilityError::NotFound(format!("no execution {id} in this process"))
        })?;
    let cancellable = execution.cancellable;
    Ok(CancelReport {
        outcome: outcome.into(),
        cancellable,
        execution: ExecutionView::of(ctx, execution),
    })
}

// ---------------------------------------------------------------- executions.protocol

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `executions.protocol`: the live channel's whole contract, derived from
/// the types that implement it.
pub struct ProtocolReport {
    /// The version of the event protocol this server speaks.
    pub protocol_version: String,
    /// Where the live channel is: a path on this same server, not a second daemon.
    pub websocket: String,
    /// How the subscription is expressed, and what a reconnect sends.
    pub subscription: Vec<ParameterView>,
    /// How many seconds of quiet before the server pings.
    pub heartbeat_seconds: u64,
    /// How many live channels this process serves at once.
    pub max_connections: usize,
    /// How many retained events a scoped connection replays before going live.
    pub max_replay: usize,
    /// The event types a client may receive, from the one enum that defines them.
    pub event_types: Vec<String>,
    /// The stream's own control messages.
    pub stream_types: Vec<String>,
    /// The JSON Schema of an event, derived from the Rust type.
    pub event_schema: Value,
    /// The JSON Schema of a control message, derived from the Rust type.
    pub stream_schema: Value,
    /// What the store keeps, so a client knows what it may ask for.
    pub limits: LimitsView,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// One query parameter of the live channel.
pub struct ParameterView {
    /// Its name.
    pub name: String,
    /// What it does.
    pub description: String,
    /// Whether it must be given.
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// What this process's execution store keeps.
pub struct LimitsView {
    /// How many executions are remembered.
    pub max_executions: usize,
    /// How many events are retained per execution.
    pub max_events: usize,
    /// The longest a single log line may be.
    pub max_log_chars: usize,
    /// How many executions run at once.
    pub max_running: usize,
    /// How far a live channel may fall behind before it is told to resynchronise.
    pub max_subscriber_queue: usize,
}

fn variants_of(schema: &Value) -> Vec<String> {
    schema
        .get("oneOf")
        .and_then(Value::as_array)
        .map(|all| {
            all.iter()
                .filter_map(|v| {
                    v.pointer("/properties/type/const")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn executions_protocol(ctx: &Context, _: Empty) -> Result<ProtocolReport, CapabilityError> {
    let event_schema = CanonicalSchema::of::<ExecutionEvent>().schema;
    let stream_schema = CanonicalSchema::of::<crate::http::events::StreamMessage>().schema;
    let limits = ctx.executions.store().limits();
    Ok(ProtocolReport {
        protocol_version: PROTOCOL_VERSION.into(),
        websocket: crate::http::events::PATH.into(),
        subscription: vec![
            ParameterView {
                name: "execution".into(),
                description: "An execution id to follow. Absent, the channel follows every execution of this process.".into(),
                required: false,
            },
            ParameterView {
                name: "after".into(),
                description: "The sequence number the client last saw. The server replays the retained events after it, then goes live with no gap and nothing written twice.".into(),
                required: false,
            },
        ],
        heartbeat_seconds: crate::http::events::HEARTBEAT.as_secs(),
        max_connections: crate::http::events::MAX_CONNECTIONS,
        max_replay: crate::http::events::MAX_REPLAY,
        event_types: variants_of(&event_schema),
        stream_types: variants_of(&stream_schema),
        event_schema,
        stream_schema,
        limits: LimitsView {
            max_executions: limits.max_executions,
            max_events: limits.max_events,
            max_log_chars: limits.max_log_chars,
            max_running: limits.max_running,
            max_subscriber_queue: limits.max_subscriber_queue,
        },
    })
}

// ---------------------------------------------------------------- executions.demonstrate

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `executions.demonstrate`.
pub struct DemonstrateInput {
    /// How many steps to walk through.
    #[serde(default = "three")]
    pub steps: u64,
    /// How long each step takes, in milliseconds. Bounded at ten seconds a step, so this
    /// cannot be used to hold a worker.
    #[serde(default)]
    pub delay_ms: u64,
    /// Fail on this step instead of completing, to show what a failure looks like.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_at: Option<u64>,
}

fn three() -> u64 {
    3
}

impl BenchmarkCases for DemonstrateInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "immediate",
            DemonstrateInput {
                steps: 1,
                delay_ms: 0,
                fail_at: None,
            },
        )]
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `executions.demonstrate`.
pub struct DemonstrateReport {
    /// How many steps ran.
    pub steps: u64,
    /// How long it took, in milliseconds.
    pub elapsed_ms: u64,
    /// Whether anything was watching: false when it was called directly rather than as an
    /// execution, which is the difference this capability exists to show.
    pub observed: bool,
}

/// The longest one step of the demonstration may take.
const MAX_STEP_MS: u64 = 10_000;

/// The most steps the demonstration will walk.
const MAX_STEPS: u64 = 100;

fn executions_demonstrate(
    ctx: &Context,
    input: DemonstrateInput,
) -> Result<DemonstrateReport, CapabilityError> {
    let steps = input.steps.min(MAX_STEPS);
    let delay = std::time::Duration::from_millis(input.delay_ms.min(MAX_STEP_MS));
    let started = std::time::Instant::now();
    let p = &ctx.progress;
    for step in 1..=steps {
        if p.cancelled() {
            return Err(p.cancellation());
        }
        let name = format!("step-{step}");
        p.step(&name, &format!("Step {step} of {steps}"));
        p.log(format!("step {step} of {steps} is working"));
        if !delay.is_zero() {
            // slept in slices so that cancellation is noticed within a tenth of a second
            // however long a step is
            let slice = std::time::Duration::from_millis(100);
            let mut left = delay;
            while !left.is_zero() {
                if p.cancelled() {
                    p.step_done(&name, false, Some("cancelled".into()));
                    return Err(p.cancellation());
                }
                let take = left.min(slice);
                std::thread::sleep(take);
                left -= take;
            }
        }
        if input.fail_at == Some(step) {
            p.step_done(&name, false, Some("asked to fail here".into()));
            return Err(CapabilityError::Internal(format!(
                "the demonstration was asked to fail at step {step} of {steps}"
            )));
        }
        p.step_done(&name, true, None);
        p.progress(step, Some(steps), format!("{step} of {steps} done"));
    }
    Ok(DemonstrateReport {
        steps,
        elapsed_ms: started.elapsed().as_millis() as u64,
        observed: p.is_reporting(),
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "executions",
        title: "Executions",
        description: "Running a capability of this registry as work that can be watched: started, followed event by event over the live channel, read back afterwards, and asked to stop. In memory; an execution does not outlive the process that accepted it.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "executions.start",
                kind: CapabilityKind::Command,
                title: "Start a capability as an execution",
                description: "Run any executable capability of this registry as an execution: the input is checked against that capability's own input schema, the work is queued, and this answers at once with the execution's id and the links to follow it. Nothing waits for the handler. The capability runs through the same executor every other interface calls, so there is no second implementation of anything.",
                input: StartInput,
                output: ExecutionView,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_execution_start"), http: post("/api/v1/executions/start"), cli: Some(crate::capability::CliExposure { path: vec!["run".into()] }) },
                tags: ["executions", "control-plane"],
                handler: executions_start,
            },
            capability! {
                id: "executions.list",
                title: "List executions",
                description: "Every execution this process remembers, newest first, narrowed by state or by capability. The counts beside them — remembered, active, queued, live channels — are what a control plane shows without asking a second question.",
                input: ListInput,
                output: ExecutionList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure { tool: Some("majordomus_executions".into()), resource: Some(McpResource { uri: EXECUTIONS_URI.into(), name: "executions".into() }) }),
                    http: get("/api/v1/executions"),
                    cli: Some(crate::capability::CliExposure { path: vec!["executions".into(), "list".into()] }),
                },
                tags: ["executions", "control-plane"],
                handler: executions_list,
            },
            capability! {
                id: "executions.get",
                title: "One execution",
                description: "The whole of what is known about one execution: its state, its input as it was stored, its steps, its progress, its diagnostics, and its output or its error. Taken under one lock, so a snapshot that says it succeeded carries what it produced.",
                input: GetInput,
                output: ExecutionView,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_execution"), http: get("/api/v1/executions/get"), cli: Some(crate::capability::CliExposure { path: vec!["executions".into(), "show".into()] }) },
                tags: ["executions", "control-plane"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: executions_get,
            },
            capability! {
                id: "executions.events",
                title: "An execution's event history",
                description: "The retained events of one execution, oldest first, after a sequence number. This is what a browser reads after a reload and what a client reads after a reconnect: the page carries the cursor to open the live channel with, so nothing is missed between the history and the stream.",
                input: EventsInput,
                output: EventHistory,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_execution_events"), http: get("/api/v1/executions/events"), cli: Some(crate::capability::CliExposure { path: vec!["executions".into(), "events".into()] }) },
                tags: ["executions", "control-plane"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: executions_events,
            },
            capability! {
                id: "executions.cancel",
                kind: CapabilityKind::Command,
                title: "Ask an execution to stop",
                description: "Set the execution's cancellation flag and say so on its stream. Cancellation is cooperative: a task looks at its flag and stops, and a capability whose policy says it is not cancellable runs to completion — which the answer says rather than pretending otherwise.",
                input: CancelInput,
                output: CancelReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_execution_cancel"), http: post("/api/v1/executions/cancel"), cli: Some(crate::capability::CliExposure { path: vec!["executions".into(), "cancel".into()] }) },
                tags: ["executions", "control-plane"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: executions_cancel,
            },
            capability! {
                id: "executions.protocol",
                title: "The live channel's contract",
                description: "Where the WebSocket is, how a subscription and a reconnect are expressed, what the server writes, and the JSON Schema of every message — derived from the Rust types that implement it, so a client validating against this is validating against the implementation. OpenAPI cannot describe a socket; this is where that contract lives.",
                input: Empty,
                output: ProtocolReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure { tool: Some("majordomus_execution_protocol".into()), resource: Some(McpResource { uri: EXECUTION_PROTOCOL_URI.into(), name: "execution-protocol".into() }) }),
                    http: get("/api/v1/executions/protocol"),
                    cli: Some(crate::capability::CliExposure { path: vec!["executions".into(), "protocol".into()] }),
                },
                tags: ["executions", "control-plane", "protocol"],
                handler: executions_protocol,
            },
            capability! {
                id: "executions.demonstrate",
                title: "Demonstrate an execution",
                description: "Walk a given number of steps, reporting each one, logging a line and advancing progress, then finish — or fail at a step you name. It exists so that an operator, a probe and an end-to-end test can prove the whole path works without waiting for real work: it reads nothing, writes nothing, and its only effect is the events it produces. It looks at its cancellation flag between steps and while it waits, so cancelling it stops it.",
                input: DemonstrateInput,
                output: DemonstrateReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_demonstrate_execution"), http: get("/api/v1/executions/demonstrate"), cli: None },
                tags: ["executions", "control-plane", "diagnostic"],
                handler: executions_demonstrate,
            }
            .cancellable(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::CapabilityKind;

    /// The declaration is the only place these names exist; every projection is derived
    /// from it. A dropped exposure would still compile and every behavioural test would
    /// still pass. This is the assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "executions");
        let expected: &[(&str, &str, &str, CapabilityKind)] = &[
            (
                "executions.start",
                "majordomus_execution_start",
                "/api/v1/executions/start",
                CapabilityKind::Command,
            ),
            (
                "executions.list",
                "majordomus_executions",
                "/api/v1/executions",
                CapabilityKind::Query,
            ),
            (
                "executions.get",
                "majordomus_execution",
                "/api/v1/executions/get",
                CapabilityKind::Query,
            ),
            (
                "executions.events",
                "majordomus_execution_events",
                "/api/v1/executions/events",
                CapabilityKind::Query,
            ),
            (
                "executions.cancel",
                "majordomus_execution_cancel",
                "/api/v1/executions/cancel",
                CapabilityKind::Command,
            ),
            (
                "executions.protocol",
                "majordomus_execution_protocol",
                "/api/v1/executions/protocol",
                CapabilityKind::Query,
            ),
            (
                "executions.demonstrate",
                "majordomus_demonstrate_execution",
                "/api/v1/executions/demonstrate",
                CapabilityKind::Query,
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, expected.iter().map(|(id, ..)| *id).collect::<Vec<_>>());
        for (executable, (id, tool, path, kind)) in m.capabilities.iter().zip(expected) {
            let c = &executable.capability;
            assert_eq!(
                c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                c.exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
            assert_eq!(c.kind, *kind, "{id} changed kind");
            let base = crate::capability::ExecutionPolicy::classify(*kind);
            assert!(
                c.execution == base || c.execution == base.stoppable(),
                "{id} carries a policy neither its kind nor a cancellation declaration gives it"
            );
        }
    }

    /// Exactly one capability here looks at its cancellation flag, and it is the one that
    /// waits. The Cockpit's Cancel button is derived from this and from nothing else.
    #[test]
    fn only_what_stops_when_asked_says_it_is_cancellable() {
        let m = module();
        let cancellable: Vec<&str> = m
            .capabilities
            .iter()
            .filter(|e| e.capability.execution.cancellable)
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(cancellable, ["executions.demonstrate"]);
        for e in &m.capabilities {
            assert!(
                !e.capability.cache.is_enabled(),
                "{} is about state that moves and is never cached",
                e.capability.id
            );
        }
    }

    #[test]
    fn an_id_that_is_not_one_is_refused_before_anything_is_looked_up() {
        for bad in ["", "../../etc/passwd", "x-nope"] {
            let e = parse_id(bad).expect_err(bad);
            assert!(matches!(e, CapabilityError::InvalidInput(_)), "{bad}");
        }
        assert!(parse_id(ExecutionId::fresh().as_str()).is_ok());
    }

    #[test]
    fn the_protocol_report_names_every_variant_the_enums_define() {
        let events = variants_of(&CanonicalSchema::of::<ExecutionEvent>().schema);
        assert!(
            events.contains(&"execution.completed".to_string()),
            "{events:?}"
        );
        assert!(events.contains(&"execution.progress".to_string()));
        assert_eq!(
            events.len(),
            12,
            "every payload variant is described: {events:?}"
        );
        let stream =
            variants_of(&CanonicalSchema::of::<crate::http::events::StreamMessage>().schema);
        assert_eq!(
            stream,
            ["stream.ready", "stream.lagged", "stream.closing"],
            "the control messages"
        );
    }
}
