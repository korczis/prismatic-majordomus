//! Executions: watching a capability call that takes longer than a request should be held
//! open for.
//!
//! Nothing here is a second way to do anything. An execution runs a capability of the one
//! registry, through the one executor, and the only thing it adds is that the call has an
//! identity, a lifecycle, a stream of typed events and a cancellation flag — so that a
//! browser, an agent or a terminal can watch it happen instead of waiting for a response.
//!
//! ```text
//!   capability!  ──►  CapabilityRegistry  ──►  Context::execute  ──►  the handler
//!                            │                        ▲                    │
//!                            │                        │                    │ Progress
//!                            └──►  ExecutionEngine ───┘                    ▼
//!                                        │                          ExecutionEvent
//!                                        ▼                                 │
//!                                  ExecutionStore ◄─────────────────────────┘
//!                                        │
//!                     ┌──────────────────┼───────────────────┐
//!                     ▼                  ▼                   ▼
//!            executions.get      executions.events      the WebSocket
//!            (HTTP · MCP · CLI)  (HTTP · MCP · CLI)     (the Cockpit)
//! ```
//!
//! The layering is the point, and it runs one way. The domain produces events; the store
//! keeps and fans them out; a transport renders them. [`crate::http::events`] is an
//! adapter over this module and contains no execution logic, which is what makes an audit
//! trail, a notifier or a second protocol something that subscribes rather than something
//! that has to be threaded through the engine.
//!
//! What it is not: a job system. There is no durable queue, no retry, no schedule and no
//! execution that outlives the process that accepted it. The store says so — its limits
//! are typed and its behaviour at each limit is defined — and [`store::Limits`] is where a
//! durable store would be substituted, because nothing above it names a `Mutex`.
//!
//! The layer's own lifecycle, without a capability or a transport in it: an execution is
//! created, events are published against it, the snapshot and the stream are two readings
//! of those events, and a final state is final.
//!
//! ```
//! use majordomus_cli::execution::*;
//! let store = ExecutionStore::new(Limits::default());
//! let id = ExecutionId::fresh();
//! store.create(id.clone(), "health.report", "Health", serde_json::json!({}), true,
//!     Actor::of(ActorKind::Cli),
//!     RepositoryRef { name: "majordomus".into(), id: "abc".into(), branch: None });
//!
//! let (_, events) = store.subscribe(Filter::Only(vec![id.clone()]));
//! store.publish(&id, EventPayload::Started);
//! store.publish(&id, EventPayload::Progress(ProgressView {
//!     current: 1, total: Some(2), message: Some("reading the index".into()),
//! }));
//! store.publish(&id, EventPayload::Completed { output: serde_json::json!({ "score": 91 }) });
//!
//! let done = store.get(&id).unwrap();
//! assert_eq!(done.state, ExecutionState::Succeeded);
//! assert_eq!(done.percent(), Some(50), "the last progress the handler reported");
//! assert_eq!(done.output, Some(serde_json::json!({ "score": 91 })));
//! // a client watching saw the same three events, in order
//! let seen: Vec<String> = events
//!     .try_iter()
//!     .filter_map(|d| match d {
//!         Delivery::Event(e) => Some(e.type_name().to_string()),
//!         Delivery::Lagged { .. } => None,
//!     })
//!     .collect();
//! assert_eq!(seen, ["execution.started", "execution.progress", "execution.completed"]);
//! // and nothing may be added after the end
//! assert!(store.publish(&id, EventPayload::Started).is_none());
//! ```

pub mod engine;
pub mod event;
pub mod model;
pub mod redact;
pub mod sink;
pub mod store;

pub use engine::{ExecutionEngine, SubmitError};
pub use event::{EventPayload, ExecutionEvent, PROTOCOL_VERSION};
pub use model::{
    Actor, ActorKind, Execution, ExecutionDiagnostic, ExecutionError, ExecutionId, ExecutionState,
    LogStream, ProgressView, RepositoryRef, StepState, StepView,
};
pub use redact::redact;
pub use sink::Progress;
pub use store::{CancelOutcome, Delivery, EventPage, ExecutionStore, Filter, Limits};
