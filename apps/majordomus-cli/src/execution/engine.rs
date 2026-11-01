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

use super::claim::{HeldClaim, Taken};
use super::event::EventPayload;
use super::model::{
    Actor, Execution, ExecutionError, ExecutionId, ExecutionState, IdempotencyKey, IssueClaim,
    RepositoryRef,
};
use super::redact::redact;
use super::store::{Admission, ExecutionStore, Idempotency, KeyConflict, Limits, Seed};

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
    /// The idempotency key already started an execution of this capability with a
    /// different input (I4000).
    #[error("conflict: {0}")]
    Conflict(KeyConflict),
}

impl SubmitError {
    /// The stable code a transport maps to its own vocabulary.
    pub fn code(&self) -> &'static str {
        match self {
            SubmitError::UnknownCapability(_) => "unknown_capability",
            SubmitError::NotExecutable(_) => "action_unavailable",
            SubmitError::InvalidInput(_) => "validation_error",
            SubmitError::Unavailable(_) => "unavailable",
            SubmitError::Conflict(_) => "conflict",
        }
    }
}

/// What a start was answered with: the execution, and whether it is one an earlier start
/// with the same idempotency key began rather than one this start did.
///
/// ```
/// use majordomus_cli::execution::{Actor, ActorKind, IdempotencyKey, Submitted};
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let start = || ctx.executions.submit_once(
///     &ctx,
///     "executions.demonstrate",
///     serde_json::json!({ "steps": 1, "delay_ms": 0 }),
///     Actor::of(ActorKind::Internal),
///     Some(IdempotencyKey::parse("click-1").unwrap()),
/// );
/// let first: Submitted = start().unwrap();
/// let again: Submitted = start().unwrap();
/// assert!(!first.deduplicated && again.deduplicated);
/// assert_eq!(first.execution.id, again.execution.id, "one execution");
/// ```
#[derive(Debug, Clone)]
pub struct Submitted {
    /// The execution, as accepted, or as it stands when the key had already started it.
    pub execution: Execution,
    /// `true` when nothing was started: the key had already started this execution with
    /// the same input, and this is that execution.
    pub deduplicated: bool,
}

/// What a start asks beyond the capability and its input.
///
/// ```
/// use majordomus_cli::execution::{IdempotencyKey, StartOptions};
/// let plain = StartOptions::default();
/// assert!(plain.idempotency_key.is_none() && plain.issue.is_none());
/// let for_issue = StartOptions {
///     idempotency_key: Some(IdempotencyKey::parse("issue:I4004").unwrap()),
///     issue: Some("I4004".into()),
/// };
/// assert_eq!(for_issue.issue.as_deref(), Some("I4004"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct StartOptions {
    /// The idempotency key (I4000): a repeated start with it is the execution it started.
    pub idempotency_key: Option<IdempotencyKey>,
    /// The issue the execution works for (I4004): its exclusive mesh claim is taken before
    /// the handler is entered and given back at the final state.
    pub issue: Option<String>,
}

struct Pending {
    id: ExecutionId,
    capability: String,
    title: String,
    input: Value,
    ctx: Context,
    issue: Option<String>,
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
    ///
    /// The answer is the execution *as accepted* — `queued`, sequence 2, the `execution.queued`
    /// event already published — and never whatever a worker has since made of it. That is
    /// what `docs/EXECUTIONS.md` promises a client of `executions.start`, and it is not a
    /// courtesy: reading the record again after the work became dispatchable would answer
    /// `queued`, `running` or even `succeeded` depending on how the machine was loaded that
    /// millisecond, and the same window would let a worker publish `execution.started` before
    /// `execution.queued`, whereupon the state machine refuses the queued event and the
    /// stream loses it. Acceptance is therefore recorded and read while the queue lock is
    /// held, before `Self::dispatch` can hand the pending to anybody.
    pub fn submit(
        &self,
        ctx: &Context,
        capability: &str,
        input: Value,
        actor: Actor,
    ) -> Result<Execution, SubmitError> {
        self.submit_once(ctx, capability, input, actor, None)
            .map(|s| s.execution)
    }

    /// [`Self::submit`], held to an idempotency key when the client sent one (I4000).
    ///
    /// The key is scoped to this repository and this capability, and is matched with the
    /// digest of the input in canonical JSON: the same key with the same input is answered
    /// the execution it started, `deduplicated`, and nothing is queued; the same key with
    /// another input is refused with [`SubmitError::Conflict`], naming that execution. The
    /// lookup and the record are one step of the store, so two starts sent at once with
    /// one key start one execution. The store holds a key for as long as it holds the
    /// execution the key started, and is in memory: a key does not outlive the server.
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, ActorKind, IdempotencyKey};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let start = || ctx.executions.submit_once(
    ///     &ctx,
    ///     "executions.demonstrate",
    ///     serde_json::json!({ "steps": 1, "delay_ms": 0 }),
    ///     Actor::of(ActorKind::Internal),
    ///     Some(IdempotencyKey::parse("click-1").unwrap()),
    /// );
    /// let first = start().unwrap();
    /// let again = start().unwrap();
    /// assert!(!first.deduplicated && again.deduplicated);
    /// assert_eq!(first.execution.id, again.execution.id, "one execution");
    /// ```
    pub fn submit_once(
        &self,
        ctx: &Context,
        capability: &str,
        input: Value,
        actor: Actor,
        idempotency_key: Option<IdempotencyKey>,
    ) -> Result<Submitted, SubmitError> {
        self.submit_with(
            ctx,
            capability,
            input,
            actor,
            StartOptions {
                idempotency_key,
                issue: None,
            },
        )
    }

    /// [`Self::submit_once`], for an issue when the start names one (I4004).
    ///
    /// An execution started for an issue holds that issue's exclusive mesh claim while it
    /// runs ([`super::claim`]): the worker that picks it up takes the claim before entering
    /// the handler and moves it `queued ──► claimed ──► running`; a claim another session
    /// holds refuses it, with the holder named, and nothing runs; the claim is given back at
    /// the final state, and kept for an outcome that is not known. With no mesh cooperation
    /// in this process it runs unclaimed, and its `execution.issue_claim` event says so.
    /// The issue is part of what an idempotency key is matched against.
    ///
    /// ```
    /// use majordomus_cli::execution::{Actor, ActorKind, ClaimStanding, StartOptions};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let started = ctx.executions.submit_with(
    ///     &ctx,
    ///     "executions.demonstrate",
    ///     serde_json::json!({ "steps": 1, "delay_ms": 0 }),
    ///     Actor::of(ActorKind::Internal),
    ///     StartOptions { issue: Some("I4004".into()), ..StartOptions::default() },
    /// ).unwrap();
    /// let claim = started.execution.issue_claim.unwrap();
    /// assert_eq!((claim.issue.as_str(), claim.standing), ("I4004", ClaimStanding::Wanted));
    /// ```
    pub fn submit_with(
        &self,
        ctx: &Context,
        capability: &str,
        input: Value,
        actor: Actor,
        options: StartOptions,
    ) -> Result<Submitted, SubmitError> {
        let StartOptions {
            idempotency_key,
            issue,
        } = options;
        let issue = issue
            .as_deref()
            .map(super::model::parse_issue)
            .transpose()
            .map_err(SubmitError::InvalidInput)?;
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
        // the issue is part of the request a key names: one key, one input, two issues is
        // two requests. Without an issue the digest is the input's alone, as it always was.
        let digested = match &issue {
            Some(issue) => serde_json::json!({ "input": &input, "issue": issue }),
            None => input.clone(),
        };
        let idempotency = idempotency_key.map(|key| Idempotency {
            key,
            digest: crate::policy::sha256_hex(&crate::capability::executor::canonical_json(
                &digested,
            )),
        });
        let seed = Seed {
            capability: c.id.as_str(),
            title: &c.title,
            input: stored,
            cancellable: c.execution.cancellable,
            actor,
            repository: repository_of(ctx),
        };
        match self.store.create_once(id.clone(), seed, idempotency) {
            Ok(Admission::Created(_)) => {}
            Ok(Admission::Existing(execution)) => {
                return Ok(Submitted {
                    execution: *execution,
                    deduplicated: true,
                })
            }
            Err(conflict) => return Err(SubmitError::Conflict(conflict)),
        }
        if let Some(issue) = &issue {
            self.store
                .publish(&id, EventPayload::IssueClaim(IssueClaim::wanted(issue)));
        }
        let pending = Pending {
            id: id.clone(),
            capability: c.id.to_string(),
            title: c.title.to_string(),
            input,
            ctx: ctx.clone(),
            issue,
        };
        // The queue lock is held across the queued event and the snapshot: a worker
        // finishing another execution calls `dispatch` too, and it needs this lock to take
        // the pending, so nothing can start what has not yet been announced as accepted.
        // The nesting is one way — this is the only place that takes the store's lock while
        // holding the engine's, the store knows nothing of the engine, and the store's
        // fan-out never blocks — so it cannot deadlock.
        let accepted = {
            let mut inner = self.lock();
            inner.queue.push_back(pending);
            let ahead = inner.queue.len() - 1;
            self.store.publish(&id, EventPayload::Queued { ahead });
            self.store.get(&id)
        };
        self.dispatch();
        accepted
            .map(|execution| Submitted {
                execution,
                deduplicated: false,
            })
            .ok_or_else(|| {
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
        title,
        input,
        ctx,
        issue,
    } = pending;
    let Some(cancel) = store.token(&id) else {
        return;
    };
    // an execution for an issue holds the issue's claim before anything runs; `held` lives
    // to the end of this function, so a worker that unwinds gives the claim back on its way
    let held = match &issue {
        Some(issue) => match claim_issue(store, &ctx, &id, &title, issue) {
            Some(held) => held,
            None => return,
        },
        None => None,
    };
    // it may have been cancelled while it waited, up to this very instant: the start is
    // refused from a final state under the lock a cancel takes, and a refused start runs
    // nothing, where a look at the state first would leave that instant open
    if store.publish(&id, EventPayload::Started).is_none() {
        // claimed, and asked to stop before the handler was entered: nothing ran
        let state = store.get(&id).map(|e| e.state);
        if state == Some(ExecutionState::Cancelling) {
            finish(store, &id, held, EventPayload::Cancelled);
        } else if let Some(held) = held {
            store.publish(&id, EventPayload::IssueClaim(held.settle(state)));
        }
        return;
    }
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
    finish(store, &id, held, payload);
}

/// Take the issue's claim for an execution a worker picked up (I4004). `Some(held)` to go on
/// — `held` is `None` when it runs unclaimed — and `None` when nothing may run: the claim was
/// refused, or the execution was stopped while the claim was taken.
fn claim_issue(
    store: &Arc<ExecutionStore>,
    ctx: &Context,
    id: &ExecutionId,
    title: &str,
    issue: &str,
) -> Option<Option<HeldClaim>> {
    // stopped while it waited: nothing to claim for
    if store.get(id).is_none_or(|e| e.state.is_final()) {
        return None;
    }
    match super::claim::take(ctx, id, title, issue) {
        Taken::Held(held) => {
            store.publish(id, EventPayload::IssueClaim(held.claim().clone()));
            let holder = held.claim().session.clone().unwrap_or_default();
            if store
                .publish(id, EventPayload::Claimed { holder })
                .is_none()
            {
                // cancelled between the look above and the claim: give it back at once
                let state = store.get(id).map(|e| e.state);
                store.publish(id, EventPayload::IssueClaim(held.settle(state)));
                return None;
            }
            Some(Some(held))
        }
        Taken::Unclaimed(claim) => {
            store.publish(id, EventPayload::IssueClaim(claim));
            let holder = "this process's execution engine, without a mesh claim".to_string();
            store.publish(id, EventPayload::Claimed { holder })?;
            Some(None)
        }
        Taken::Refused {
            claim,
            conflict,
            message,
        } => {
            store.publish(id, EventPayload::IssueClaim(claim));
            let error = ExecutionError {
                code: if conflict { "refused" } else { "unavailable" }.into(),
                message,
                suggestion: Some(if conflict {
                    "wait for the holder to finish, or ask it with majordomus_mesh_peers; the claim ends when its execution does".into()
                } else {
                    "read majordomus mesh doctor; nothing ran".into()
                }),
                correlation_id: id.to_string(),
            };
            let payload = if conflict {
                EventPayload::Refused {
                    error,
                    policy: Some("mesh.claim".into()),
                }
            } else {
                EventPayload::Failed { error }
            };
            store.publish(id, payload);
            None
        }
    }
}

/// Publish an execution's last event, settling its issue claim first: released for a final
/// state, kept for an outcome that is not known, so that the claim's event is said before the
/// event a subscriber stops reading at.
fn finish(
    store: &Arc<ExecutionStore>,
    id: &ExecutionId,
    held: Option<HeldClaim>,
    payload: EventPayload,
) {
    if let Some(held) = held {
        let settled = held.settle(payload.implies_state());
        store.publish(id, EventPayload::IssueClaim(settled));
    }
    store.publish(id, payload);
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
        CapabilityError::Forbidden(_) => ("forbidden", None),
        CapabilityError::Conflict(_) => ("conflict", None),
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
            | CapabilityError::Internal(m)
            | CapabilityError::Forbidden(m)
            | CapabilityError::Conflict(m) => m.clone(),
        },
        suggestion,
        correlation_id: id.to_string(),
    }
}

/// Which repository this process runs executions against. Never the request's idea of it:
/// a process serves the one repository it read at start-up.
pub(crate) fn repository_of(ctx: &Context) -> RepositoryRef {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::{ActorKind, ClaimStanding};
    use crate::mesh::state::ClaimState;

    /// A context over a synthetic repository, with a mesh runtime attached when `mesh`.
    fn context(mesh: bool) -> (crate::synthetic::SyntheticRepository, Arc<Context>) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        if mesh {
            ctx.mesh
                .attach_cooperation(crate::mesh::testing::cooperation());
        }
        (repo, ctx)
    }

    fn start(ctx: &Context, issue: &str, input: Value) -> Execution {
        ctx.executions
            .submit_with(
                ctx,
                "executions.demonstrate",
                input,
                Actor::of(ActorKind::Internal),
                StartOptions {
                    issue: Some(issue.into()),
                    ..StartOptions::default()
                },
            )
            .unwrap()
            .execution
    }

    /// Wait, bounded, for an execution to reach a state that satisfies `done`.
    fn until(ctx: &Context, id: &ExecutionId, done: impl Fn(&Execution) -> bool) -> Execution {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let e = ctx.executions.store().get(id).expect("held");
            if done(&e) {
                return e;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "never reached: {e:#?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn types(ctx: &Context, id: &ExecutionId) -> Vec<&'static str> {
        ctx.executions
            .store()
            .events(id, 0, 1000)
            .unwrap()
            .events
            .iter()
            .map(|e| e.type_name())
            .collect()
    }

    fn live_claims(ctx: &Context, issue: &str) -> usize {
        ctx.mesh
            .cooperation()
            .unwrap()
            .state()
            .claims
            .iter()
            .filter(|c| c.issue.as_deref() == Some(issue) && c.state == ClaimState::Held)
            .count()
    }

    #[test]
    fn an_issue_execution_is_claimed_then_runs_and_gives_the_claim_back() {
        let (_repo, ctx) = context(true);
        let e = start(
            &ctx,
            "I4004",
            serde_json::json!({ "steps": 1, "delay_ms": 0 }),
        );
        let done = until(&ctx, &e.id, |e| e.state.is_final());
        assert_eq!(done.state, ExecutionState::Succeeded);
        let claim = done.issue_claim.expect("the claim is on the snapshot");
        assert_eq!(claim.standing, ClaimStanding::Released);
        assert_eq!(claim.scope, ["issue/I4004"]);
        assert_eq!(
            claim.session.as_deref(),
            Some(format!("execution-{}", e.id).as_str())
        );
        assert!(claim.key.is_some());
        let order: Vec<_> = types(&ctx, &e.id)
            .into_iter()
            .filter(|t| {
                matches!(
                    *t,
                    "execution.issue_claim"
                        | "execution.queued"
                        | "execution.claimed"
                        | "execution.started"
                        | "execution.completed"
                )
            })
            .collect();
        assert_eq!(
            order,
            [
                "execution.issue_claim", // wanted
                "execution.queued",
                "execution.issue_claim", // held
                "execution.claimed",
                "execution.started",
                "execution.issue_claim", // released
                "execution.completed",
            ]
        );
        assert_eq!(live_claims(&ctx, "I4004"), 0, "released");
    }

    #[test]
    fn a_second_start_of_a_held_issue_is_refused_naming_the_holder_and_nothing_runs() {
        let (_repo, ctx) = context(true);
        let first = start(
            &ctx,
            "I4004",
            serde_json::json!({ "steps": 50, "delay_ms": 1000 }),
        );
        until(&ctx, &first.id, |e| e.state == ExecutionState::Running);
        assert_eq!(live_claims(&ctx, "I4004"), 1);
        let second = start(
            &ctx,
            "I4004",
            serde_json::json!({ "steps": 1, "delay_ms": 0 }),
        );
        let refused = until(&ctx, &second.id, |e| e.state.is_final());
        assert_eq!(refused.state, ExecutionState::Refused);
        assert_eq!(refused.started_at, None, "nothing ran");
        let error = refused.error.expect("why");
        assert_eq!(error.code, "refused");
        assert!(
            error.message.starts_with("claim_conflict: I4004"),
            "{}",
            error.message
        );
        assert!(
            error.message.contains(&format!("execution-{}", first.id)),
            "the holder is named: {}",
            error.message
        );
        let claim = refused.issue_claim.unwrap();
        assert_eq!(claim.standing, ClaimStanding::Refused);
        assert!(!types(&ctx, &second.id).contains(&"execution.started"));
        // another issue is not held by it
        let other = start(
            &ctx,
            "I4005",
            serde_json::json!({ "steps": 1, "delay_ms": 0 }),
        );
        assert_eq!(
            until(&ctx, &other.id, |e| e.state.is_final()).state,
            ExecutionState::Succeeded
        );
        ctx.executions.cancel(&first.id, "test");
        let stopped = until(&ctx, &first.id, |e| e.state.is_final());
        assert_eq!(stopped.state, ExecutionState::Cancelled);
        assert_eq!(
            stopped.issue_claim.unwrap().standing,
            ClaimStanding::Released
        );
        assert_eq!(live_claims(&ctx, "I4004"), 0, "released when cancelled");
    }

    #[test]
    fn a_failed_issue_execution_releases_its_claim_too() {
        let (_repo, ctx) = context(true);
        let e = start(
            &ctx,
            "I7",
            serde_json::json!({ "steps": 2, "delay_ms": 0, "fail_at": 1 }),
        );
        let done = until(&ctx, &e.id, |e| e.state.is_final());
        assert_eq!(done.state, ExecutionState::Failed);
        assert_eq!(done.issue_claim.unwrap().standing, ClaimStanding::Released);
        assert_eq!(live_claims(&ctx, "I7"), 0);
    }

    #[test]
    fn without_a_mesh_an_issue_execution_runs_unclaimed_and_says_so() {
        let (_repo, ctx) = context(false);
        let e = start(
            &ctx,
            "I4004",
            serde_json::json!({ "steps": 1, "delay_ms": 0 }),
        );
        let done = until(&ctx, &e.id, |e| e.state.is_final());
        assert_eq!(done.state, ExecutionState::Succeeded, "never a failure");
        let claim = done.issue_claim.unwrap();
        assert_eq!(claim.standing, ClaimStanding::Unclaimed);
        assert!(claim.key.is_none());
        assert!(claim
            .detail
            .unwrap()
            .starts_with("ran without a mesh claim"));
        assert!(types(&ctx, &e.id).contains(&"execution.claimed"));
        // and a start that names no issue is exactly what it was
        let plain = ctx
            .executions
            .submit(
                &ctx,
                "executions.demonstrate",
                serde_json::json!({ "steps": 1, "delay_ms": 0 }),
                Actor::of(ActorKind::Internal),
            )
            .unwrap();
        assert_eq!(plain.last_sequence, 2, "created, queued");
        let plain = until(&ctx, &plain.id, |e| e.state.is_final());
        assert!(plain.issue_claim.is_none());
        assert!(!types(&ctx, &plain.id).contains(&"execution.claimed"));
    }

    #[test]
    fn a_bad_issue_id_is_refused_before_anything_is_created() {
        let (_repo, ctx) = context(false);
        for bad in ["", "I1/../x", "two words", &"I".repeat(65)] {
            let refused = ctx
                .executions
                .submit_with(
                    &ctx,
                    "executions.demonstrate",
                    serde_json::json!({}),
                    Actor::of(ActorKind::Internal),
                    StartOptions {
                        issue: Some(bad.to_string()),
                        ..StartOptions::default()
                    },
                )
                .unwrap_err();
            assert_eq!(refused.code(), "validation_error", "{bad:?}");
        }
        assert!(ctx.executions.store().is_empty());
    }

    #[test]
    fn a_forbidden_capability_fails_its_execution_with_the_refusal_it_gave() {
        let id = ExecutionId::fresh();
        let e = failure(&id, &CapabilityError::remote("plan.transition"));
        assert_eq!(e.code, "forbidden");
        assert!(e.message.contains("plan.transition"), "{}", e.message);
        assert_eq!(e.suggestion, None);
        assert_eq!(e.correlation_id, id.to_string());
        for (error, code) in [
            (
                CapabilityError::InvalidInput("m".into()),
                "validation_error",
            ),
            (CapabilityError::NotFound("m".into()), "not_found"),
            (CapabilityError::Refused("m".into()), "refused"),
            (CapabilityError::Internal("m".into()), "internal"),
        ] {
            let e = failure(&id, &error);
            assert_eq!((e.code.as_str(), e.message.as_str()), (code, "m"));
        }
    }
}
