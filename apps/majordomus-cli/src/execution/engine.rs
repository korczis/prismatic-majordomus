//! The engine: what turns a capability call into an execution anybody can watch.
//!
//! It owns no behaviour of its own. Everything it runs is a capability of the registry,
//! reached through [`crate::capability::Context::execute`] — the same executor MCP, the
//! HTTP routes, the command line and the benchmarks call — so a browser, an agent and a
//! terminal reach one implementation and there is no second copy of anything to drift.
//!
//! ```text
//!   submit ── validate ── create ── queue ──► worker thread
//!                                               │
//!                          Context.progress ────┤ ctx.execute(id, input)
//!                                               │        │
//!                                               │        └─► the capability's own handler
//!                                               ▼
//!                                       ExecutionStore ──► subscribers ──► WebSocket
//! ```
//!
//! It is deliberately small: an in-memory store, a bounded number of worker threads, and
//! a cooperative cancellation flag. There is no durable queue, no retry and no scheduler,
//! because nothing here is a background job system — it is a way to watch a call that
//! takes longer than a request should be held open for.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::capability::{Capability, CapabilityError, CapabilityKind, Concurrency, Context};

use super::event::EventPayload;
use super::model::{Actor, Execution, ExecutionError, ExecutionId, RepositoryRef};
use super::redact::redact;
use super::store::{ExecutionStore, Limits};

/// Why an execution was not accepted. Refused before anything is created, so a client that
/// is told no knows nothing started.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubmitError {
    /// No capability of that id.
    #[error("unknown capability: {0}")]
    UnknownCapability(String),
    /// A resource is read, not executed; a planned or unsupported capability is listed and
    /// never run.
    #[error("not executable: {0}")]
    NotExecutable(String),
    /// The input does not satisfy the capability's input schema.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The engine is stopping and accepts nothing further.
    #[error("unavailable: {0}")]
    Unavailable(String),
}

impl SubmitError {
    /// The stable code a transport maps to its own vocabulary.
    pub fn code(&self) -> &'static str {
        match self {
            SubmitError::UnknownCapability(_) => "unknown_capability",
            SubmitError::NotExecutable(_) => "action_unavailable",
            SubmitError::InvalidInput(_) => "validation_error",
            SubmitError::Unavailable(_) => "unavailable",
        }
    }
}

struct Pending {
    id: ExecutionId,
    capability: String,
    input: Value,
    ctx: Context,
}

struct Inner {
    queue: VecDeque<Pending>,
    /// How many workers are inside a handler right now.
    running: usize,
    /// Which capabilities have an execution in flight, for the serial policy.
    busy: BTreeMap<String, usize>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

/// The execution engine of one process.
///
/// One per process, held by [`crate::capability::Context`], so that the command line, an
/// MCP session and every HTTP worker see the same executions.
pub struct ExecutionEngine {
    store: Arc<ExecutionStore>,
    inner: Mutex<Inner>,
    stopping: AtomicBool,
    validators: Mutex<BTreeMap<String, Arc<jsonschema::Validator>>>,
}

impl std::fmt::Debug for ExecutionEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExecutionEngine({} active)", self.store.active())
    }
}

impl Default for ExecutionEngine {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}

impl ExecutionEngine {
    /// An engine with these limits.
    pub fn new(limits: Limits) -> Self {
        ExecutionEngine {
            store: Arc::new(ExecutionStore::new(limits)),
            inner: Mutex::new(Inner {
                queue: VecDeque::new(),
                running: 0,
                busy: BTreeMap::new(),
                workers: Vec::new(),
            }),
            stopping: AtomicBool::new(false),
            validators: Mutex::new(BTreeMap::new()),
        }
    }

    /// The store, for the capabilities that read it.
    pub fn store(&self) -> &Arc<ExecutionStore> {
        &self.store
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Check an input against a capability's schema, with the compiled validator kept.
    ///
    /// The handler validates again when it deserialises, and that is what is authoritative;
    /// this exists so that a client learns its input is wrong from the request that sent
    /// it, with the constraint named, rather than from an execution that fails a moment
    /// later.
    fn validate(&self, c: &Capability, input: &Value) -> Result<(), SubmitError> {
        let validator = {
            let mut cache = self.validators.lock().unwrap_or_else(|e| e.into_inner());
            match cache.get(c.id.as_str()) {
                Some(v) => Arc::clone(v),
                None => match jsonschema::validator_for(&c.input.schema) {
                    Ok(v) => {
                        let v = Arc::new(v);
                        cache.insert(c.id.to_string(), Arc::clone(&v));
                        v
                    }
                    // a schema this crate derived that will not compile is a defect of the
                    // crate, not of the caller: say so and let the handler decide
                    Err(e) => {
                        tracing::warn!(capability_id = %c.id, "the input schema does not compile: {e}");
                        return Ok(());
                    }
                },
            }
        };
        let problems: Vec<String> = validator
            .iter_errors(input)
            .take(5)
            .map(|e| {
                let at = e.instance_path().to_string();
                if at.is_empty() {
                    e.to_string()
                } else {
                    format!("{at}: {e}")
                }
            })
            .collect();
        if problems.is_empty() {
            Ok(())
        } else {
            Err(SubmitError::InvalidInput(problems.join("; ")))
        }
    }

    /// Accept a capability call as an execution and answer with its first snapshot.
    ///
    /// Fast on purpose: the input is checked, the record is created, the work is queued and
    /// this returns. Nothing waits for the handler.
    pub fn submit(
        &self,
        ctx: &Context,
        capability: &str,
        input: Value,
        actor: Actor,
    ) -> Result<Execution, SubmitError> {
        if self.stopping.load(Ordering::SeqCst) {
            return Err(SubmitError::Unavailable(
                "this server is shutting down and is accepting no further executions".into(),
            ));
        }
        let c = ctx
            .registry
            .get(capability)
            .ok_or_else(|| SubmitError::UnknownCapability(capability.to_string()))?;
        if !c.kind.is_executable() {
            return Err(SubmitError::NotExecutable(format!(
                "'{}' is a resource: it is read, never executed. Read it with objects.get",
                c.id
            )));
        }
        if !c.stability.executable() {
            return Err(SubmitError::NotExecutable(format!(
                "'{}' is {:?} and no projection executes it",
                c.id, c.stability
            )));
        }
        let input = if input.is_null() {
            Value::Object(serde_json::Map::new())
        } else {
            input
        };
        self.validate(c, &input)?;

        let id = ExecutionId::fresh();
        let stored = redact(&c.input.schema, &input);
        self.store.create(
            id.clone(),
            c.id.as_str(),
            &c.title,
            stored,
            c.execution.cancellable,
            actor,
            repository_of(ctx),
        );
        let pending = Pending {
            id: id.clone(),
            capability: c.id.to_string(),
            input,
            ctx: ctx.clone(),
        };
        {
            let mut inner = self.lock();
            inner.queue.push_back(pending);
            let ahead = inner.queue.len() - 1;
            drop(inner);
            self.store.publish(&id, EventPayload::Queued { ahead });
        }
        self.dispatch();
        self.store.get(&id).ok_or_else(|| {
            SubmitError::Unavailable("the execution was forgotten as it was created".into())
        })
    }

    /// Start whatever may start now: the bound on concurrent handlers, and the serial
    /// policy of a capability that must not overlap with itself.
    fn dispatch(&self) {
        loop {
            let next = {
                let mut inner = self.lock();
                if inner.running >= self.store.limits().max_running {
                    return;
                }
                let position = inner.queue.iter().position(|p| {
                    let serial = self
                        .concurrency_of(&p.ctx, &p.capability)
                        .is_some_and(|c| c == Concurrency::Serial);
                    !serial || !inner.busy.contains_key(&p.capability)
                });
                let Some(position) = position else { return };
                let Some(pending) = inner.queue.remove(position) else {
                    return;
                };
                inner.running += 1;
                *inner.busy.entry(pending.capability.clone()).or_insert(0) += 1;
                inner.workers.retain(|w| !w.is_finished());
                pending
            };
            let store = Arc::clone(&self.store);
            let id = next.id.clone();
            let capability = next.capability.clone();
            let done_with = capability.clone();
            let engine = Arc::clone(&next.ctx.executions);
            let worker = std::thread::Builder::new()
                .name(format!(
                    "execution-{}",
                    &id.as_str()[..12.min(id.as_str().len())]
                ))
                .spawn(move || {
                    run(&store, next);
                    let mut inner = engine.lock();
                    inner.running = inner.running.saturating_sub(1);
                    if let Some(count) = inner.busy.get_mut(&done_with) {
                        *count -= 1;
                        if *count == 0 {
                            inner.busy.remove(&done_with);
                        }
                    }
                    drop(inner);
                    engine.dispatch();
                });
            match worker {
                Ok(handle) => self.lock().workers.push(handle),
                Err(e) => {
                    // the thread could not be spawned: the execution fails now rather than
                    // sitting queued for a worker that will never come
                    let mut inner = self.lock();
                    inner.running = inner.running.saturating_sub(1);
                    inner.busy.remove(&capability);
                    drop(inner);
                    self.store.publish(
                        &id,
                        EventPayload::Failed {
                            error: ExecutionError {
                                code: "internal".into(),
                                message: format!("no worker could be started: {e}"),
                                suggestion: None,
                                correlation_id: id.to_string(),
                            },
                        },
                    );
                }
            }
        }
    }

    fn concurrency_of(&self, ctx: &Context, capability: &str) -> Option<Concurrency> {
        ctx.registry
            .get(capability)
            .map(|c| c.execution.concurrency)
    }

    /// Ask an execution to stop.
    pub fn cancel(&self, id: &ExecutionId, by: &str) -> super::store::CancelOutcome {
        self.store.request_cancel(id, by)
    }

    /// Stop accepting executions, ask every one that is running to stop, and wait for the
    /// workers, up to `grace`.
    ///
    /// A handler that does not look at its cancellation flag is not killed: this process
    /// is about to end, and interrupting a read half way leaves nothing behind that
    /// matters. What the wait buys is that a client watching gets the final event.
    pub fn shutdown(&self, grace: std::time::Duration) {
        self.stopping.store(true, Ordering::SeqCst);
        for execution in self.store.list(None, None, usize::MAX) {
            if execution.state.is_active() {
                self.store.request_cancel(&execution.id, "server");
            }
        }
        let deadline = std::time::Instant::now() + grace;
        loop {
            let done = {
                let mut inner = self.lock();
                inner.workers.retain(|w| !w.is_finished());
                inner.workers.is_empty() && inner.queue.is_empty()
            };
            if done || std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let stranded: Vec<ExecutionId> = self
            .store
            .list(None, None, usize::MAX)
            .into_iter()
            .filter(|e| e.state.is_active())
            .map(|e| e.id)
            .collect();
        for id in stranded {
            self.store.publish(&id, EventPayload::Cancelled);
        }
    }

    /// Is this engine still accepting executions?
    pub fn accepting(&self) -> bool {
        !self.stopping.load(Ordering::SeqCst)
    }

    /// How many executions are waiting for a worker.
    pub fn queued(&self) -> usize {
        self.lock().queue.len()
    }
}

/// Run one execution to its end, whatever happens inside the handler.
fn run(store: &Arc<ExecutionStore>, pending: Pending) {
    let Pending {
        id,
        capability,
        input,
        ctx,
    } = pending;
    // it may have been cancelled while it waited
    if store.get(&id).is_none_or(|e| e.state.is_final()) {
        return;
    }
    let Some(cancel) = store.token(&id) else {
        return;
    };
    store.publish(&id, EventPayload::Started);
    let progress =
        super::sink::Progress::reporting(Arc::clone(store), id.clone(), Arc::clone(&cancel));
    let ctx = ctx.reporting(progress);
    let span = tracing::info_span!("execution", execution_id = %id, capability_id = %capability);
    let entered = span.enter();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.execute_observed(&capability, input)
    }));
    drop(entered);
    let cancelled = cancel.load(Ordering::SeqCst);
    let payload = match outcome {
        Ok(Ok(output)) => EventPayload::Completed { output },
        Ok(Err(error)) if cancelled => {
            tracing::debug!(execution_id = %id, "the handler stopped after cancellation: {error}");
            EventPayload::Cancelled
        }
        Ok(Err(error)) => EventPayload::Failed {
            error: failure(&id, &error),
        },
        Err(panic) => {
            let what = panic
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "a handler panicked".to_string());
            // the message goes to this process's log, where an operator can read it; the
            // client is told which execution it was and nothing about this crate's insides
            tracing::error!(execution_id = %id, capability_id = %capability, "a handler panicked: {what}");
            EventPayload::Failed {
                error: ExecutionError {
                    code: "internal".into(),
                    message: "the handler failed unexpectedly; the server's log carries the detail"
                        .into(),
                    suggestion: Some(format!("search the server's output for execution_id={id}")),
                    correlation_id: id.to_string(),
                },
            }
        }
    };
    store.publish(&id, payload);
}

/// A capability error as an execution reports it.
fn failure(id: &ExecutionId, error: &CapabilityError) -> ExecutionError {
    let (code, suggestion) = match error {
        CapabilityError::InvalidInput(_) => (
            "validation_error",
            Some("check the input against the capability's input schema".to_string()),
        ),
        CapabilityError::NotFound(_) => ("not_found", None),
        CapabilityError::Refused(_) => ("refused", None),
        CapabilityError::Internal(_) => (
            "internal",
            Some(format!("search the server's output for execution_id={id}")),
        ),
    };
    ExecutionError {
        code: code.into(),
        message: match error {
            CapabilityError::InvalidInput(m)
            | CapabilityError::NotFound(m)
            | CapabilityError::Refused(m)
            | CapabilityError::Internal(m) => m.clone(),
        },
        suggestion,
        correlation_id: id.to_string(),
    }
}

/// Which repository this process runs executions against. Never the request's idea of it:
/// a process serves the one repository it read at start-up.
fn repository_of(ctx: &Context) -> RepositoryRef {
    let root = std::path::Path::new(&ctx.index.repository.root);
    RepositoryRef {
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "repository".into()),
        id: crate::repository::identity(root),
        branch: match &ctx.index.repository.git {
            crate::git::GitState::Available(info) => info.branch.clone(),
            crate::git::GitState::Unavailable { .. } => None,
        },
    }
}

/// Is this capability one an execution may run at all? The same question `submit` asks,
/// answered without submitting, for a projection that lists what a client may start.
///
/// ```
/// use majordomus_cli::capability::{CapabilityKind, Stability};
/// use majordomus_cli::execution::engine::runnable;
/// assert!(runnable(CapabilityKind::Query, Stability::Implemented).is_none());
/// assert!(runnable(CapabilityKind::Resource, Stability::Implemented).is_some());
/// assert!(runnable(CapabilityKind::Query, Stability::Planned).is_some());
/// ```
pub fn runnable(kind: CapabilityKind, stability: crate::capability::Stability) -> Option<String> {
    if !kind.is_executable() {
        return Some("it is a resource: read it rather than running it".into());
    }
    if !stability.executable() {
        return Some(format!(
            "it is {}, and no projection executes it",
            format!("{stability:?}").to_lowercase()
        ));
    }
    None
}
