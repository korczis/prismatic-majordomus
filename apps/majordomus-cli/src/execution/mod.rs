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
//! One plane per checkout, not one per process. `majordomus run` executes in its own
//! process, and when a server already serves the checkout it reports the execution there
//! ([`ExecutionStore::adopt`], [`ExecutionStore::report`]), so the server's listing, the
//! Cockpit and the live channel show it with the pid that ran it; a server that stops
//! hearing from it fails it rather than showing it running for ever. With no server, the
//! run says on stderr that nothing else can see it. A run never starts a server.
//!
//! A server's executions outlive it (ADR 0222). `majordomus serve` builds its store over a
//! [`Journal`] under `.ai/local/state/executions/`: every execution and every event is
//! written down before anybody is shown it, and a server that starts folds the journal
//! back, so a browser that reconnects after a restart reads the same execution at the same
//! cursor. A restart never runs anything again: an execution the stopped process was
//! running is recovered failed with the code `interrupted`, and one another process reports
//! is recovered as reported and continues when that process reports again. A command that
//! runs once keeps its store in memory.
//!
//! What it is not: a job system. There is no retry and no schedule; an unknown outcome is
//! never a reason to repeat a mutation. The store's limits are typed and its behaviour at
//! each limit is defined, and a forgotten execution's journal is archived, never deleted.

pub mod engine;
pub mod event;
pub mod journal;
pub mod model;
pub mod redact;
pub mod sink;
pub mod store;

pub use engine::{ExecutionEngine, SubmitError};
pub use event::{EventPayload, ExecutionEvent, PROTOCOL_VERSION};
pub use journal::Journal;
pub use model::{
    Actor, ActorKind, Execution, ExecutionDiagnostic, ExecutionError, ExecutionId, ExecutionState,
    LogStream, ProgressView, RepositoryRef, StepState, StepView,
};
pub use redact::redact;
pub use sink::Progress;
pub use store::{
    AdoptError, CancelOutcome, Delivery, EventPage, ExecutionStore, Filter, Limits, ReportOutcome,
    Seed, REPORT_HEARTBEAT, REPORT_SILENCE,
};
