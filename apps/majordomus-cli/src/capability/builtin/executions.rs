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
use crate::execution::engine::repository_of;
use crate::execution::{
    redact, Actor, ActorKind, CancelOutcome, Execution, ExecutionEvent, ExecutionId,
    ExecutionState, IdempotencyKey, Seed, SubmitError, PROTOCOL_VERSION, REPORT_SILENCE,
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
            pid: None,
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
    /// A key that makes a repeated start one execution (I4000): sent again with the same
    /// capability and input, it is answered the execution it started, `deduplicated`, and
    /// nothing new runs; sent again with another input, it is refused 409 `conflict`,
    /// naming that execution. 1 to 128 characters of `A-Z a-z 0-9 . _ : -`, scoped to this
    /// repository and the capability, and held for as long as the server holds the
    /// execution it started. Without one, every start starts an execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 128), regex(pattern = r"^[A-Za-z0-9._:-]+$"))]
    pub idempotency_key: Option<String>,
}

impl BenchmarkCases for StartInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "demonstrate",
            StartInput {
                capability: "executions.demonstrate".into(),
                input: Some(serde_json::json!({ "steps": 1, "delay_ms": 0 })),
                idempotency_key: None,
            },
        )]
    }
}

/// The answer of `executions.start`: the execution, flattened as every other execution
/// answer is, and whether this start began it or was answered one its idempotency key had
/// already started.
///
/// ```
/// use majordomus_cli::capability::builtin::StartedExecution;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let input = serde_json::json!({
///     "capability": "executions.demonstrate",
///     "input": { "steps": 1, "delay_ms": 0 },
///     "idempotency_key": "click-1",
/// });
/// let start = || -> StartedExecution {
///     serde_json::from_value(ctx.execute("executions.start", input.clone()).unwrap()).unwrap()
/// };
/// let (first, again) = (start(), start());
/// assert!(!first.deduplicated && again.deduplicated);
/// assert_eq!(first.view.execution.id, again.view.execution.id);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StartedExecution {
    /// The execution, with the links to follow it.
    #[serde(flatten)]
    pub view: ExecutionView,
    /// `true` when this start began nothing: its idempotency key had already started this
    /// execution with the same input, and the answer is that execution as it stands now.
    /// `false` for an execution this start began.
    pub deduplicated: bool,
}

fn executions_start(ctx: &Context, input: StartInput) -> Result<StartedExecution, CapabilityError> {
    let key = input
        .idempotency_key
        .as_deref()
        .map(IdempotencyKey::parse)
        .transpose()
        .map_err(CapabilityError::InvalidInput)?;
    let payload = input.input.unwrap_or_else(|| serde_json::json!({}));
    match ctx
        .executions
        .submit_once(ctx, &input.capability, payload, actor(ctx), key)
    {
        Ok(started) => Ok(StartedExecution {
            view: ExecutionView::of(ctx, started.execution),
            deduplicated: started.deduplicated,
        }),
        Err(SubmitError::Conflict(conflict)) => {
            Err(CapabilityError::Conflict(conflict.to_string()))
        }
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
    store.abandon_silent(REPORT_SILENCE);
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
    ctx.executions.store().abandon_silent(REPORT_SILENCE);
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
    store.abandon_silent(REPORT_SILENCE);
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

// ---------------------------------------------------------------- executions.report

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `executions.report`: what a process that runs an execution says about it to
/// the server serving the same checkout.
pub struct ReportInput {
    /// The execution's id, as the reporting process minted it.
    pub id: String,
    /// The capability it runs, by its canonical id.
    pub capability: String,
    /// That capability's title.
    pub title: String,
    /// The input it was given, as the reporting process stored it: already redacted there,
    /// and redacted again here by this registry's schema when this registry knows it.
    #[serde(default)]
    pub input: Value,
    /// Whether the capability stops when asked. When it does, a cancel asked of this server
    /// reaches the reporter in the answer to its next report.
    #[serde(default)]
    pub cancellable: bool,
    /// The process that runs it.
    pub pid: u32,
    /// Its events after the last sequence this server answered `through`, in its own
    /// sequence, oldest first; at most 500. Empty is a heartbeat.
    #[serde(default)]
    pub events: Vec<ExecutionEvent>,
}

impl BenchmarkCases for ReportInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        Vec::new()
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `executions.report`.
pub struct ReportAnswer {
    /// How many of the reported events were applied.
    pub applied: usize,
    /// The reporter's sequence this server holds everything up to: the next report sends
    /// the events after it, so a lost or repeated report costs nothing.
    pub through: u64,
    /// Whether somebody asked this server to stop the execution. The reporter runs it, so
    /// the reporter cancels it.
    pub cancel_requested: bool,
    /// The execution as this server now holds it.
    pub execution: ExecutionView,
}

fn executions_report(ctx: &Context, input: ReportInput) -> Result<ReportAnswer, CapabilityError> {
    let id = parse_id(&input.id)?;
    if input.events.len() > MAX_EVENTS {
        return Err(CapabilityError::InvalidInput(format!(
            "{} events in one report; at most {MAX_EVENTS}, and the answer's `through` says where the next one starts",
            input.events.len()
        )));
    }
    if let Some(other) = input.events.iter().find(|e| e.execution_id != id) {
        return Err(CapabilityError::InvalidInput(format!(
            "an event of {} in a report about {id}",
            other.execution_id
        )));
    }
    let store = ctx.executions.store();
    store.abandon_silent(REPORT_SILENCE);
    let stored = match ctx.registry.get(&input.capability) {
        Some(c) => redact(&c.input.schema, &input.input),
        None => input.input,
    };
    store
        .adopt(
            id.clone(),
            Seed {
                capability: &input.capability,
                title: &input.title,
                input: stored,
                cancellable: input.cancellable,
                actor: Actor::reported(input.pid),
                repository: repository_of(ctx),
            },
            input.pid,
        )
        .map_err(|e| CapabilityError::Refused(e.to_string()))?;
    // adopted just now as a reported execution, so the store answers both readings; the
    // refusal is named once, eagerly, rather than on two paths no report can take
    let refused = CapabilityError::NotFound(format!("no reported execution {id} in this process"));
    store
        .report(
            &id,
            input.events.into_iter().map(|e| (e.sequence, e.payload)),
        )
        .zip(store.get(&id))
        .map(|(outcome, execution)| ReportAnswer {
            applied: outcome.applied,
            through: outcome.through,
            cancel_requested: outcome.cancel_requested,
            execution: ExecutionView::of(ctx, execution),
        })
        .ok_or(refused)
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
        description: "Running a capability of this registry as work that can be watched: started, followed event by event over the live channel, read back afterwards, and asked to stop. One plane per checkout: `majordomus run` executes in its own process and reports the execution to the server already serving the checkout, so that server lists it beside its own; with no server it says that nothing else can see the run. In memory; an execution does not outlive the server that holds it.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "executions.start",
                kind: CapabilityKind::Command,
                title: "Start a capability as an execution",
                description: "Run any executable capability of this registry as an execution: the input is checked against that capability's own input schema, the work is queued, and this answers at once with the execution's id and the links to follow it. Nothing waits for the handler. With an idempotency_key, a repeated start is one execution: the same key with the same capability and input is answered the execution it started (deduplicated) and nothing new runs, and the same key with another input is refused as a conflict naming that execution. The capability runs through the same executor every other interface calls, so there is no second implementation of anything. On the command line (`majordomus run`) it runs in that process and is reported to the server serving the checkout, when one is, through executions.report; a run never starts a server.",
                input: StartInput,
                output: StartedExecution,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_execution_start"), http: post("/api/v1/executions/start"), cli: Some(crate::capability::CliExposure { path: vec!["run".into()] }) },
                tags: ["executions", "control-plane"],
                handler: executions_start,
            }
            // It runs any executable capability, writers included, so its effect is the
            // strongest of what it can start: a client that asks before running a writer
            // asks before this too, instead of being told it changes only this process's
            // memory while it transitions a tracked record (ADR 0126).
            .writes_repository(),
            capability! {
                id: "executions.list",
                title: "List executions",
                description: "Every execution this process remembers, newest first, narrowed by state or by capability: the ones it ran and the ones `majordomus run` in another process of the checkout reported to it, each with who started it. The counts beside them — remembered, active, queued, live channels — are what a control plane shows without asking a second question.",
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
            }
            // this process's executions are not the repository: a caller on another host never
            // reads them (ADR 0220 decision 4, I3842)
            .reads_only_locally(),
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
            }
            // this process's executions are not the repository: a caller on another host never
            // reads them (ADR 0220 decision 4, I3842)
            .reads_only_locally(),
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
            }
            // this process's executions are not the repository: a caller on another host never
            // reads them (ADR 0220 decision 4, I3842)
            .reads_only_locally(),
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
                id: "executions.report",
                kind: CapabilityKind::Command,
                title: "Report an execution another process runs",
                description: "What `majordomus run` posts while it runs a capability in its own process and a server already serves the checkout: the execution's description and its events after the last sequence this server took. The server holds it as an execution started by the command line, with the pid, in the same store, listing, Cockpit pages and live channel as its own; applies each event once and in order, answering where the next report starts; tells the reporter when somebody asked it to stop; and fails it with `unavailable` once the reporter has been silent for 30 s. Nothing runs here. Changes this process's memory only.",
                input: ReportInput,
                output: ReportAnswer,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: None, http: post("/api/v1/executions/report"), cli: None },
                tags: ["executions", "control-plane"],
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::TransientState },
                handler: executions_report,
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
            .cancellable()
            // it starts an execution, which a caller on another host may not do (I3842)
            .reads_only_locally(),
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
        let expected: &[(&str, Option<&str>, &str, CapabilityKind)] = &[
            (
                "executions.start",
                Some("majordomus_execution_start"),
                "/api/v1/executions/start",
                CapabilityKind::Command,
            ),
            (
                "executions.list",
                Some("majordomus_executions"),
                "/api/v1/executions",
                CapabilityKind::Query,
            ),
            (
                "executions.get",
                Some("majordomus_execution"),
                "/api/v1/executions/get",
                CapabilityKind::Query,
            ),
            (
                "executions.events",
                Some("majordomus_execution_events"),
                "/api/v1/executions/events",
                CapabilityKind::Query,
            ),
            (
                "executions.cancel",
                Some("majordomus_execution_cancel"),
                "/api/v1/executions/cancel",
                CapabilityKind::Command,
            ),
            (
                "executions.report",
                None,
                "/api/v1/executions/report",
                CapabilityKind::Command,
            ),
            (
                "executions.protocol",
                Some("majordomus_execution_protocol"),
                "/api/v1/executions/protocol",
                CapabilityKind::Query,
            ),
            (
                "executions.demonstrate",
                Some("majordomus_demonstrate_execution"),
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
                *tool,
                "{id} lost, renamed or gained an MCP tool"
            );
            assert_eq!(
                c.exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
            assert_eq!(c.kind, *kind, "{id} changed kind");
            let base = crate::capability::ExecutionPolicy::classify(*kind);
            // executions.start may run a writer, so it is declared one (ADR 0126)
            let writes = c.id.as_str() == "executions.start";
            // what this process runs answers this machine alone (ADR 0220 decision 4, I3842)
            let local = matches!(
                c.id.as_str(),
                "executions.list"
                    | "executions.get"
                    | "executions.events"
                    | "executions.demonstrate"
            );
            let scoped = |p: crate::capability::ExecutionPolicy| {
                if local {
                    p.reads_only_locally()
                } else {
                    p
                }
            };
            assert!(
                c.execution == scoped(base)
                    || c.execution == scoped(base.stoppable())
                    || (writes && c.execution == base.writes_repository()),
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
            [
                "stream.ready",
                "stream.heartbeat",
                "stream.lagged",
                "stream.closing"
            ],
            "the control messages"
        );
    }

    fn report_of(id: &str, capability: &str, events: Vec<ExecutionEvent>) -> ReportInput {
        ReportInput {
            id: id.to_string(),
            capability: capability.to_string(),
            title: "Demonstrate an execution".into(),
            input: serde_json::json!({ "steps": 1 }),
            cancellable: true,
            pid: 4242,
            events,
        }
    }

    fn started(id: &ExecutionId, sequence: u64) -> ExecutionEvent {
        ExecutionEvent::new(
            id.clone(),
            sequence,
            crate::execution::EventPayload::Started,
        )
    }

    /// A report is refused before anything is held when its id is not one, when it carries
    /// more events than one report may, or when an event in it is about another execution;
    /// and an id this server runs itself is never written into by a report.
    #[test]
    fn a_report_is_refused_for_a_bad_id_too_many_events_a_foreign_event_or_an_own_execution() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let refusal = |input: ReportInput| match executions_report(&ctx, input) {
            Err(e) => e,
            Ok(answer) => panic!("a report was taken: {}", answer.through),
        };

        let e = refusal(report_of("x-nope", "executions.demonstrate", Vec::new()));
        assert!(matches!(e, CapabilityError::InvalidInput(_)), "{e}");

        let id = ExecutionId::fresh();
        let many = (2..=(MAX_EVENTS as u64 + 2))
            .map(|n| started(&id, n))
            .collect();
        let e = refusal(report_of(id.as_str(), "executions.demonstrate", many));
        assert!(e.to_string().contains("events in one report"), "{e}");

        let other = ExecutionId::fresh();
        let e = refusal(report_of(
            id.as_str(),
            "executions.demonstrate",
            vec![started(&other, 2)],
        ));
        assert!(e.to_string().contains("in a report about"), "{e}");
        assert!(ctx.executions.store().get(&id).is_none(), "nothing held");

        let own = ExecutionId::fresh();
        ctx.executions.store().create(
            own.clone(),
            "executions.demonstrate",
            "Demonstrate an execution",
            serde_json::json!({}),
            true,
            Actor::of(ActorKind::Internal),
            repository_of(&ctx),
        );
        let e = refusal(report_of(
            own.as_str(),
            "executions.demonstrate",
            Vec::new(),
        ));
        assert!(matches!(e, CapabilityError::Refused(_)), "{e}");
    }

    /// A report about a capability this registry does not know is still held, its input as
    /// the reporter stored it; one this registry knows is held too, and answers how far the
    /// server holds it.
    #[test]
    fn a_report_is_held_whether_or_not_this_registry_knows_its_capability() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");

        let elsewhere = ExecutionId::fresh();
        let answer = executions_report(
            &ctx,
            report_of(
                elsewhere.as_str(),
                "nowhere.known",
                vec![started(&elsewhere, 2)],
            ),
        )
        .expect("held");
        assert_eq!((answer.applied, answer.through), (1, 2));
        assert!(!answer.cancel_requested);
        let held = ctx.executions.store().get(&elsewhere).expect("held");
        assert_eq!(held.input, serde_json::json!({ "steps": 1 }));
        assert_eq!(held.actor.pid, Some(4242));

        let known = ExecutionId::fresh();
        let answer = executions_report(
            &ctx,
            report_of(known.as_str(), "executions.demonstrate", Vec::new()),
        )
        .expect("held");
        assert_eq!((answer.applied, answer.through), (0, 1));
        assert_eq!(answer.execution.execution.state, ExecutionState::Queued);
    }

    /// I4000 through the capability every transport calls: the key on the answer, the flag
    /// that says whether this start began anything, the conflict's own error, and a key
    /// that is not one refused before anything is created.
    #[test]
    fn a_start_with_a_key_is_answered_once_and_a_bad_key_is_refused() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let start = |key: &str, steps: u64| {
            executions_start(
                &ctx,
                StartInput {
                    capability: "executions.demonstrate".into(),
                    input: Some(serde_json::json!({ "steps": steps, "delay_ms": 0 })),
                    idempotency_key: Some(key.into()),
                },
            )
        };
        let first = start("implement-I4000", 1).expect("started");
        assert!(!first.deduplicated);
        let again = start("implement-I4000", 1).expect("answered");
        assert!(again.deduplicated);
        assert_eq!(again.view.execution.id, first.view.execution.id);
        let answer = serde_json::to_value(&again).unwrap();
        assert_eq!(answer["deduplicated"], true, "{answer}");
        assert_eq!(answer["idempotency_key"], "implement-I4000", "{answer}");
        assert_eq!(
            answer["id"],
            first.view.execution.id.as_str(),
            "flattened: {answer}"
        );

        let conflict = start("implement-I4000", 2).unwrap_err();
        assert_eq!(conflict.code(), "conflict");
        assert!(conflict
            .to_string()
            .contains(first.view.execution.id.as_str()));
        for bad in ["", "two words", "../x", &"k".repeat(129)] {
            let refused = start(bad, 1).unwrap_err();
            assert_eq!(refused.code(), "invalid_input", "{bad:?}: {refused}");
        }
        assert_eq!(
            ctx.executions.store().len(),
            1,
            "one execution, whatever was sent"
        );
    }

    /// The Cockpit's page about one execution names the process that runs a reported one and
    /// says how a stop reaches it; about one this server runs, it says only that the flag is
    /// set.
    #[test]
    fn the_execution_page_says_where_a_stop_goes_for_a_reported_and_an_own_execution() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");

        let reported = ExecutionId::fresh();
        executions_report(
            &ctx,
            report_of(
                reported.as_str(),
                "executions.demonstrate",
                vec![started(&reported, 2)],
            ),
        )
        .expect("held");
        let html = crate::cockpit::pages::execution(&ctx, reported.as_str())
            .main
            .render();
        assert!(html.contains("majordomus run, process 4242"), "{html}");
        assert!(html.contains("It runs in process 4242"), "{html}");

        let own = ExecutionId::fresh();
        ctx.executions.store().create(
            own.clone(),
            "executions.demonstrate",
            "Demonstrate an execution",
            serde_json::json!({}),
            true,
            Actor::of(ActorKind::Internal),
            repository_of(&ctx),
        );
        let html = crate::cockpit::pages::execution(&ctx, own.as_str())
            .main
            .render();
        assert!(html.contains("the flag is set"), "{html}");
        assert!(!html.contains("majordomus run, process"), "{html}");
    }
}
