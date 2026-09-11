//! Where executions live while this process does: the snapshots, the retained event
//! history, and the subscribers.
//!
//! Everything is bounded. A store that grew with the work it observed would be this
//! executable's own memory leak, so the number of retained executions, the number of
//! events kept per execution and the depth of a subscriber's queue all have a limit, and
//! each limit has a defined behaviour when it is reached rather than an unwritten one.
//!
//! State moves in one place. [`ExecutionStore::publish`] applies an event to the snapshot
//! under the lock, refusing a transition [`ExecutionState::may_move_to`] does not allow,
//! assigns the sequence, retains the event and hands it to the subscribers — so a client
//! reading the snapshot and a client reading the stream can never disagree about what
//! happened, only about how much of it they have seen.
//!
//! The lifecycle of one execution, from this module's side: it is created queued, events
//! are published against it, and each publish moves the snapshot and reaches the
//! subscribers. Nothing here runs anything — the engine does that — and nothing here
//! outlives the process.
//!
//! ```
//! use majordomus_cli::execution::*;
//! let store = ExecutionStore::new(Limits::default());
//! let id = ExecutionId::fresh();
//! let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({"n": 1}),
//!     true, Actor::of(ActorKind::Internal),
//!     RepositoryRef { name: "majordomus".into(), id: "abc".into(), branch: None });
//! assert_eq!(store.get(&id).unwrap().state, ExecutionState::Queued);
//! assert!(!cancel.load(std::sync::atomic::Ordering::SeqCst));
//!
//! let (_, events) = store.subscribe(Filter::All);
//! store.publish(&id, EventPayload::Started);
//! store.publish(&id, EventPayload::Completed { output: serde_json::json!({"ok": true}) });
//!
//! // the snapshot and the stream are two readings of the same applied events
//! let finished = store.get(&id).unwrap();
//! assert_eq!(finished.state, ExecutionState::Succeeded);
//! assert_eq!(finished.output, Some(serde_json::json!({"ok": true})));
//! let seen: Vec<String> = events
//!     .try_iter()
//!     .filter_map(|d| match d {
//!         Delivery::Event(e) => Some(e.type_name().to_string()),
//!         Delivery::Lagged { .. } => None,
//!     })
//!     .collect();
//! assert_eq!(seen, ["execution.started", "execution.completed"]);
//!
//! // and nothing may be said after the last thing: the state machine refuses it
//! assert!(store.publish(&id, EventPayload::Started).is_none());
//! ```

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::Value;

use super::event::{EventPayload, ExecutionEvent};
use super::model::{
    Actor, Execution, ExecutionError, ExecutionId, ExecutionState, ProgressView, RepositoryRef,
    StepState, StepView,
};

/// What the store keeps, and how much of it.
///
/// The defaults are for a development server a person is watching: enough history that a
/// browser reload during a long run loses nothing, and little enough that a process left
/// running for a week is not holding a transcript of it.
///
/// Every field is a bound with a defined behaviour at it, which is the whole point of
/// naming them here rather than writing constants into the store: what happens when a
/// limit is reached is part of the contract, and a reader can find all five in one place.
///
/// ```
/// use majordomus_cli::execution::{ExecutionStore, Limits};
/// // a store answers with the limits it was built with, so a diagnostic can say them
/// let tight = Limits { max_events: 3, ..Limits::default() };
/// assert_eq!(ExecutionStore::new(tight).limits().max_events, 3);
/// // and the defaults are all bounded: an unbounded one would be this process's own leak
/// let d = Limits::default();
/// for bound in [d.max_executions, d.max_events, d.max_log_chars, d.max_subscriber_queue,
///               d.max_running] {
///     assert!(bound > 0);
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How many executions are remembered. The oldest finished one is forgotten first; an
    /// execution that is still running is never forgotten.
    pub max_executions: usize,
    /// How many events are retained per execution. The oldest are dropped and the
    /// snapshot says so, so a client that asks for history knows it did not get all of it.
    pub max_events: usize,
    /// The longest a single log line may be; longer is cut with a marker.
    pub max_log_chars: usize,
    /// How many events a subscriber may be behind before it is told to resynchronise
    /// instead of being waited for.
    pub max_subscriber_queue: usize,
    /// How many executions may be running at once. Beyond it, an execution waits queued.
    pub max_running: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_executions: 200,
            max_events: 2_000,
            max_log_chars: 2_000,
            max_subscriber_queue: 512,
            max_running: 4,
        }
    }
}

/// What a subscriber receives: an event, or the news that it missed some.
///
/// The second variant is why this is an enum rather than the event itself. A subscriber
/// that falls behind is told so instead of being waited for — waiting would let a slow
/// client hold up the execution it is watching — and `Lagged` is an instruction: read the
/// snapshot and the history again from the last sequence you saw. A stream that dropped
/// events silently would let a client believe it had seen an execution it had not.
///
/// ```
/// use majordomus_cli::execution::*;
/// let store = ExecutionStore::new(Limits::default());
/// let id = ExecutionId::fresh();
/// store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
///     Actor::of(ActorKind::Internal),
///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// let (_, events) = store.subscribe(Filter::All);
/// store.publish(&id, EventPayload::Started);
/// match events.try_recv().expect("the event was handed over as it was published") {
///     Delivery::Event(event) => assert_eq!(event.type_name(), "execution.started"),
///     Delivery::Lagged { dropped } => panic!("nothing was dropped, and yet {dropped} were"),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivery {
    /// One event, in order.
    Event(Arc<ExecutionEvent>),
    /// The subscriber fell behind and events were dropped rather than buffered. It must
    /// read the snapshot and the history again from the sequence it last saw.
    Lagged {
        /// How many events were dropped.
        dropped: u64,
    },
}

/// A page of an execution's retained history.
///
/// Three of the four fields are about what the page is *not*: `last_sequence` is where to
/// ask from next, `more` says there is already more to read, and `truncated` says the
/// caller asked from a point the bound has since dropped. A client can therefore tell "you
/// are up to date" from "you have missed the beginning", which a bare list of events
/// cannot.
///
/// ```
/// use majordomus_cli::execution::*;
/// let store = ExecutionStore::new(Limits::default());
/// let id = ExecutionId::fresh();
/// store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
///     Actor::of(ActorKind::Internal),
///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// store.publish(&id, EventPayload::Started);
///
/// // asking for one of the two events says so, and says where to continue
/// let page: EventPage = store.events(&id, 0, 1).unwrap();
/// assert_eq!(page.events.len(), 1);
/// assert_eq!(page.last_sequence, 1);
/// assert!(page.more);
/// assert!(!page.truncated, "nothing has been dropped");
/// // continuing from that cursor reaches the end
/// let rest = store.events(&id, page.last_sequence, 10).unwrap();
/// assert!(!rest.more);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPage {
    /// The events, oldest first.
    pub events: Vec<Arc<ExecutionEvent>>,
    /// The sequence of the last event in this page; the cursor for the next.
    pub last_sequence: u64,
    /// Whether more events follow this page right now.
    pub more: bool,
    /// Whether events before this page were dropped by the bound.
    pub truncated: bool,
}

/// Which executions a subscriber wants.
///
/// The narrowing is the store's and not the client's: a browser watching one run is sent
/// that run's events rather than every event with instructions to discard most of them.
/// `Only` holds ids and nothing else — there is deliberately no filter by capability or by
/// state, because either would have to be re-evaluated as an execution moved and a
/// subscriber would silently stop hearing about the thing it was watching.
///
/// ```
/// use majordomus_cli::execution::{ExecutionId, Filter};
/// let watched = ExecutionId::fresh();
/// let other = ExecutionId::fresh();
/// let filter = Filter::Only(vec![watched.clone()]);
/// assert!(filter.wants(&watched));
/// assert!(!filter.wants(&other));
/// assert!(Filter::All.wants(&other));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    /// Every execution this process runs.
    All,
    /// Only these.
    Only(Vec<ExecutionId>),
}

impl Filter {
    /// Does this filter want events about `id`?
    ///
    /// Asked once per subscriber per event, under the store's lock, so it is deliberately
    /// a comparison of ids and nothing that could read the execution's state.
    ///
    /// ```
    /// use majordomus_cli::execution::{ExecutionId, Filter};
    /// let id = ExecutionId::fresh();
    /// assert!(Filter::All.wants(&id));
    /// assert!(!Filter::Only(Vec::new()).wants(&id), "a subscriber to nothing wants nothing");
    /// assert!(Filter::Only(vec![id.clone()]).wants(&id));
    /// ```
    pub fn wants(&self, id: &ExecutionId) -> bool {
        match self {
            Filter::All => true,
            Filter::Only(ids) => ids.contains(id),
        }
    }
}

/// The outcome of asking to cancel.
///
/// Four answers, because a client that asked to stop something needs to know which of them
/// happened: it was asked, it had already been asked, it was already over, or there is no
/// such execution. Collapsing them into a bool would make "already finished" and "no such
/// execution" the same answer, and they are not.
///
/// None of them says the execution *has* stopped. Cancellation is cooperative; the final
/// state is what says how it ended.
///
/// ```
/// use majordomus_cli::execution::*;
/// let store = ExecutionStore::new(Limits::default());
/// let id = ExecutionId::fresh();
/// store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
///     Actor::of(ActorKind::Internal),
///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
///
/// // a queued execution has no worker to notice the token, so asking finishes it
/// assert_eq!(store.request_cancel(&id, "a client"), CancelOutcome::Requested);
/// assert_eq!(store.get(&id).unwrap().state, ExecutionState::Cancelled);
/// assert_eq!(
///     store.request_cancel(&id, "a client"),
///     CancelOutcome::AlreadyFinished(ExecutionState::Cancelled),
/// );
/// // and an id nothing knows about is a different answer again
/// assert_eq!(store.request_cancel(&ExecutionId::fresh(), "a client"), CancelOutcome::Unknown);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelOutcome {
    /// The token is set and a cancelling event was published.
    Requested,
    /// It had already been asked for.
    AlreadyRequested,
    /// It had already finished; nothing was done.
    AlreadyFinished(ExecutionState),
    /// No such execution.
    Unknown,
}

struct Subscriber {
    id: u64,
    filter: Filter,
    tx: std::sync::mpsc::SyncSender<Delivery>,
    dropped: u64,
}

struct Record {
    snapshot: Execution,
    events: VecDeque<Arc<ExecutionEvent>>,
    /// The sequence of the oldest retained event; 1 while nothing has been dropped.
    first_retained: u64,
    cancel: Arc<AtomicBool>,
    cancel_requested: bool,
    started: Option<Instant>,
}

struct Inner {
    records: BTreeMap<ExecutionId, Record>,
    order: VecDeque<ExecutionId>,
    subscribers: Vec<Subscriber>,
}

/// The executions of one process, and the only place their state moves.
///
/// It holds three things: a bounded set of snapshots, a bounded event history per
/// execution, and the subscribers. Every change goes through [`ExecutionStore::publish`]
/// under one lock, which is what makes the snapshot and the stream two readings of one
/// value rather than two pieces of state that have to be kept in step.
///
/// What it is not is durable. Nothing here survives the process, there is no queue and no
/// retry, and [`Limits`] says exactly how much is remembered — this is where a durable
/// store would be substituted, because nothing above it names a `Mutex`.
///
/// ```
/// use majordomus_cli::execution::*;
/// let store = ExecutionStore::new(Limits::default());
/// assert!(store.is_empty());
/// let id = ExecutionId::fresh();
/// store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
///     Actor::of(ActorKind::Internal),
///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// assert_eq!(store.len(), 1);
/// assert_eq!(store.active(), 1, "a queued execution is not finished");
/// store.publish(&id, EventPayload::Started);
/// store.publish(&id, EventPayload::Failed { error: ExecutionError {
///     code: "internal".into(), message: "it broke".into(), suggestion: None,
///     correlation_id: id.to_string(),
/// }});
/// assert_eq!(store.active(), 0);
/// assert_eq!(store.get(&id).unwrap().error.unwrap().code, "internal");
/// ```
pub struct ExecutionStore {
    limits: Limits,
    inner: Mutex<Inner>,
    next_subscriber: AtomicU64,
}

impl ExecutionStore {
    /// A store with these limits, remembering nothing.
    ///
    /// The limits are fixed for the life of the store: they are read on every publish and
    /// handed to every [`Progress`](crate::execution::Progress), and a store whose bounds
    /// could change under a running execution would make the history's own claims about
    /// truncation untrue.
    ///
    /// ```
    /// use majordomus_cli::execution::{ExecutionStore, Limits};
    /// let store = ExecutionStore::new(Limits { max_running: 1, ..Limits::default() });
    /// assert!(store.is_empty());
    /// assert_eq!(store.subscribers(), 0);
    /// assert_eq!(store.limits().max_running, 1, "the store keeps what it was built with");
    /// ```
    pub fn new(limits: Limits) -> Self {
        ExecutionStore {
            limits,
            inner: Mutex::new(Inner {
                records: BTreeMap::new(),
                order: VecDeque::new(),
                subscribers: Vec::new(),
            }),
            next_subscriber: AtomicU64::new(1),
        }
    }

    /// The limits this store was built with.
    pub fn limits(&self) -> Limits {
        self.limits
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // a poisoned lock means a thread panicked while holding it. The snapshot it was
        // editing is one execution's, the rest of the store is intact, and refusing every
        // further execution because one handler panicked would turn a contained failure
        // into an outage.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Record a new execution and publish its first event. Returns its cancellation token.
    ///
    /// The execution is created queued: what runs it decides when it starts.
    ///
    /// The first event carries the capability, the title and the input, so a client that
    /// joins at sequence 1 can render a heading without asking anything else. The input
    /// arrives already redacted — this store does not know which of a capability's fields
    /// are sensitive, and a store that redacted would be a second place that had to be
    /// taught.
    ///
    /// Creating one may forget another: the bound on remembered executions is applied here,
    /// oldest finished first, and an execution that is still running is never forgotten.
    ///
    /// ```
    /// use majordomus_cli::execution::*;
    /// let store = ExecutionStore::new(Limits::default());
    /// let id = ExecutionId::fresh();
    /// let cancel = store.create(id.clone(), "demo.echo", "Echo",
    ///     serde_json::json!({"text": "hi"}), true, Actor::of(ActorKind::Internal),
    ///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    ///
    /// let snapshot = store.get(&id).unwrap();
    /// assert_eq!(snapshot.state, ExecutionState::Queued);
    /// assert_eq!(snapshot.last_sequence, 1, "the created event is the first one");
    /// assert!(snapshot.started_at.is_none(), "nothing has picked it up");
    /// assert!(!cancel.load(std::sync::atomic::Ordering::SeqCst));
    /// // the heading a late client renders comes off that first event
    /// let created = serde_json::to_value(&*store.events(&id, 0, 1).unwrap().events[0]).unwrap();
    /// assert_eq!(created["type"], "execution.created");
    /// assert_eq!(created["data"]["input"], serde_json::json!({"text": "hi"}));
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        id: ExecutionId,
        capability: &str,
        title: &str,
        input: Value,
        cancellable: bool,
        actor: Actor,
        repository: RepositoryRef,
    ) -> Arc<AtomicBool> {
        let cancel = Arc::new(AtomicBool::new(false));
        let now = crate::peers::rfc3339(std::time::SystemTime::now());
        let snapshot = Execution {
            id: id.clone(),
            capability: capability.to_string(),
            title: title.to_string(),
            state: ExecutionState::Queued,
            created_at: now,
            started_at: None,
            finished_at: None,
            duration_ms: None,
            input: input.clone(),
            output: None,
            error: None,
            progress: None,
            steps: Vec::new(),
            diagnostics: Vec::new(),
            last_sequence: 0,
            event_count: 0,
            events_truncated: false,
            cancellable,
            actor,
            repository,
            correlation_id: id.to_string(),
        };
        {
            let mut inner = self.lock();
            inner.records.insert(
                id.clone(),
                Record {
                    snapshot,
                    events: VecDeque::new(),
                    first_retained: 1,
                    cancel: Arc::clone(&cancel),
                    cancel_requested: false,
                    started: None,
                },
            );
            inner.order.push_back(id.clone());
            self.evict(&mut inner);
        }
        self.publish(
            &id,
            EventPayload::Created {
                capability: capability.to_string(),
                title: title.to_string(),
                input,
            },
        );
        cancel
    }

    /// Forget the oldest finished executions until the bound holds. A running execution is
    /// never forgotten, however old: the bound protects memory, and dropping the record of
    /// work that is still happening would protect nothing and lose the only handle on it.
    fn evict(&self, inner: &mut Inner) {
        while inner.records.len() > self.limits.max_executions {
            let Some(position) = inner.order.iter().position(|id| {
                inner
                    .records
                    .get(id)
                    .is_some_and(|r| r.snapshot.state.is_final())
            }) else {
                break;
            };
            if let Some(id) = inner.order.remove(position) {
                inner.records.remove(&id);
            }
        }
    }

    /// Apply an event to an execution: move its state, update its snapshot, retain it, and
    /// hand it to every subscriber that wants it.
    ///
    /// Returns the sequence the event was given, or `None` when there is no such execution
    /// or the payload asks for a transition the state machine does not allow — which is
    /// how a second `completed` after a `cancelled` is refused rather than believed.
    ///
    /// Sequences are dense and start at 1, so a client that has seen `n` knows it has
    /// missed something the moment it reads anything but `n + 1`. Saying the same thing
    /// twice is not a transition and is not refused: a handler that reports `started`
    /// twice is redundant, not wrong.
    ///
    /// ```
    /// use majordomus_cli::execution::*;
    /// let store = ExecutionStore::new(Limits::default());
    /// let id = ExecutionId::fresh();
    /// store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    ///     Actor::of(ActorKind::Internal),
    ///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    ///
    /// assert_eq!(store.publish(&id, EventPayload::Started), Some(2));
    /// assert_eq!(store.publish(&id, EventPayload::Started), Some(3), "repetition is allowed");
    /// // a running execution is not cancelled in one move: it is asked, and then it stops
    /// assert!(store.publish(&id, EventPayload::Cancelled).is_none());
    /// let by = EventPayload::Cancelling { by: "a client".into() };
    /// assert_eq!(store.publish(&id, by), Some(4));
    /// assert_eq!(store.publish(&id, EventPayload::Cancelled), Some(5));
    /// // it is over, so nothing further about it is believed
    /// assert_eq!(store.publish(&id, EventPayload::Completed { output: serde_json::json!(1) }),
    ///            None);
    /// assert_eq!(store.get(&id).unwrap().state, ExecutionState::Cancelled);
    /// // and an execution this store never had cannot be spoken about at all
    /// assert!(store.publish(&ExecutionId::fresh(), EventPayload::Started).is_none());
    /// ```
    pub fn publish(&self, id: &ExecutionId, payload: EventPayload) -> Option<u64> {
        let (event, deliveries) = {
            let mut inner = self.lock();
            let limits = self.limits;
            let record = inner.records.get_mut(id)?;
            if let Some(next) = payload.implies_state() {
                let current = record.snapshot.state;
                if current == next {
                    // saying the same thing twice is not a transition and not an error
                } else if !current.may_move_to(next) {
                    tracing::debug!(
                        execution_id = %id,
                        from = current.as_str(),
                        to = next.as_str(),
                        "an execution event was refused: the state machine does not allow the move"
                    );
                    return None;
                }
            }
            let sequence = record.snapshot.last_sequence + 1;
            apply(record, &payload, sequence, limits);
            let event = Arc::new(ExecutionEvent::new(id.clone(), sequence, payload));
            record.events.push_back(Arc::clone(&event));
            while record.events.len() > limits.max_events {
                record.events.pop_front();
                record.first_retained += 1;
                record.snapshot.events_truncated = true;
            }
            let deliveries = inner.fan_out(id, &event);
            (event, deliveries)
        };
        tracing::debug!(
            execution_id = %event.execution_id,
            sequence = event.sequence,
            event = event.type_name(),
            subscribers = deliveries,
            "execution event"
        );
        Some(event.sequence)
    }

    /// The snapshot of one execution, taken now.
    ///
    /// A clone and not a view: it is produced under the lock and read afterwards, so a
    /// caller never sees a state and an output that disagree, and a snapshot it is still
    /// holding does not change under it as the execution moves on. Whoever wants the
    /// current state asks again.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// let queued = store.get(&id).expect("the execution this store was just given");
    /// assert_eq!(queued.state, ExecutionState::Queued);
    /// store.publish(&id, EventPayload::Started);
    /// // the value taken earlier is still the execution as it was then
    /// assert_eq!(queued.state, ExecutionState::Queued);
    /// assert_eq!(store.get(&id).unwrap().state, ExecutionState::Running);
    /// assert!(store.get(&ExecutionId::fresh()).is_none());
    /// ```
    pub fn get(&self, id: &ExecutionId) -> Option<Execution> {
        self.lock().records.get(id).map(|r| r.snapshot.clone())
    }

    /// Every execution this process remembers, newest first, optionally narrowed.
    ///
    /// Newest first because a person opening a list wants what just happened; the order is
    /// creation order reversed and not a sort by state, so an execution does not move about
    /// in the list as it runs. The narrowing is applied before the limit, so asking for ten
    /// failed executions gives ten failed ones rather than the failures among the last ten.
    ///
    /// ```
    /// use majordomus_cli::execution::*;
    /// let store = ExecutionStore::new(Limits::default());
    /// let mut ids = Vec::new();
    /// for capability in ["demo.first", "demo.second"] {
    ///     let id = ExecutionId::fresh();
    ///     store.create(id.clone(), capability, capability, serde_json::json!({}), true,
    ///         Actor::of(ActorKind::Internal),
    ///         RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    ///     ids.push(id);
    /// }
    /// store.publish(&ids[0], EventPayload::Started);
    ///
    /// let newest_first: Vec<String> =
    ///     store.list(None, None, 10).into_iter().map(|e| e.capability).collect();
    /// assert_eq!(newest_first, ["demo.second", "demo.first"]);
    /// // narrowed by state, and by capability
    /// assert_eq!(store.list(Some(ExecutionState::Running), None, 10).len(), 1);
    /// assert_eq!(store.list(None, Some("demo.second"), 10).len(), 1);
    /// assert_eq!(store.list(None, None, 1).len(), 1, "the limit is the caller's");
    /// ```
    pub fn list(
        &self,
        state: Option<ExecutionState>,
        capability: Option<&str>,
        limit: usize,
    ) -> Vec<Execution> {
        let inner = self.lock();
        inner
            .order
            .iter()
            .rev()
            .filter_map(|id| inner.records.get(id))
            .map(|r| &r.snapshot)
            .filter(|s| state.is_none_or(|want| s.state == want))
            .filter(|s| capability.is_none_or(|want| s.capability == want))
            .take(limit)
            .cloned()
            .collect()
    }

    /// How many executions this store remembers.
    pub fn len(&self) -> usize {
        self.lock().records.len()
    }

    /// Does it remember no execution at all?
    ///
    /// True for a process that has been asked to run nothing, which is the ordinary state
    /// of a freshly started server rather than a fault. It is not a claim about idleness —
    /// a store that remembers only finished executions is not empty; see
    /// [`ExecutionStore::active`] for that.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many executions are in a state right now.
    ///
    /// Over what the store still remembers, which is bounded: a long-lived process has
    /// forgotten its oldest finished executions, so this is a gauge of the present and
    /// never a total of everything the process has ever run.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// assert_eq!(store.count(ExecutionState::Queued), 1);
    /// assert_eq!(store.count(ExecutionState::Succeeded), 0);
    /// store.publish(&id, EventPayload::Started);
    /// store.publish(&id, EventPayload::Completed { output: serde_json::json!(null) });
    /// assert_eq!(store.count(ExecutionState::Queued), 0);
    /// assert_eq!(store.count(ExecutionState::Succeeded), 1);
    /// ```
    pub fn count(&self, state: ExecutionState) -> usize {
        self.lock()
            .records
            .values()
            .filter(|r| r.snapshot.state == state)
            .count()
    }

    /// How many are not finished: queued, running or cancelling.
    ///
    /// The number that decides whether this process is busy, and the one a shutdown waits
    /// on. Derived from each snapshot's state rather than counted as executions come and
    /// go, so it cannot drift away from what the snapshots say.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// assert_eq!(store.active(), 1, "a queued execution is work this process owes");
    /// store.publish(&id, EventPayload::Started);
    /// assert_eq!(store.active(), 1);
    /// store.publish(&id, EventPayload::Cancelling { by: "a client".into() });
    /// assert_eq!(store.active(), 1, "asking it to stop has not stopped it");
    /// store.publish(&id, EventPayload::Cancelled);
    /// assert_eq!(store.active(), 0);
    /// assert!(!store.is_empty(), "it is remembered, and it is not active");
    /// ```
    pub fn active(&self) -> usize {
        self.lock()
            .records
            .values()
            .filter(|r| r.snapshot.state.is_active())
            .count()
    }

    /// A page of one execution's retained history, after a sequence.
    ///
    /// `after` is exclusive and is the cursor a client keeps: 0 means "from the beginning",
    /// and the page's own `last_sequence` is what to pass next. Asking from a point the
    /// bound has dropped is answered rather than refused — the page says `truncated`, so a
    /// client knows the gap is the store's doing and not its own.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// store.publish(&id, EventPayload::Started);
    /// let all = store.events(&id, 0, 10).unwrap();
    /// assert_eq!(all.events.len(), 2, "the created event and the started one");
    /// assert_eq!(all.last_sequence, 2);
    /// // from the cursor a client already holds, there is nothing new
    /// let nothing_new = store.events(&id, 2, 10).unwrap();
    /// assert!(nothing_new.events.is_empty());
    /// assert_eq!(nothing_new.last_sequence, 2, "the cursor is still where it was");
    /// assert!(store.events(&ExecutionId::fresh(), 0, 10).is_none());
    /// ```
    pub fn events(&self, id: &ExecutionId, after: u64, limit: usize) -> Option<EventPage> {
        let inner = self.lock();
        let record = inner.records.get(id)?;
        let all: Vec<Arc<ExecutionEvent>> = record
            .events
            .iter()
            .filter(|e| e.sequence > after)
            .cloned()
            .collect();
        let more = all.len() > limit;
        let events: Vec<Arc<ExecutionEvent>> = all.into_iter().take(limit).collect();
        Some(EventPage {
            last_sequence: events
                .last()
                .map(|e| e.sequence)
                .unwrap_or_else(|| after.max(record.snapshot.last_sequence)),
            more,
            // the client asked from a point the store no longer holds
            truncated: after + 1 < record.first_retained,
            events,
        })
    }

    /// The cancellation token of an execution, for the worker that runs it.
    ///
    /// The same token [`ExecutionStore::create`] returned, so a worker that was started
    /// without it can still find the one flag the handler watches. Reading it is all a
    /// worker may do with it — asking to cancel goes through
    /// [`ExecutionStore::request_cancel`], which also tells the subscribers.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// use std::sync::atomic::Ordering;
    /// let token = store.token(&id).expect("an execution this store remembers has one");
    /// assert!(!token.load(Ordering::SeqCst));
    /// store.request_cancel(&id, "a client");
    /// assert!(token.load(Ordering::SeqCst), "the worker's token is the store's own");
    /// assert!(std::sync::Arc::ptr_eq(&token, &cancel), "and the creator's, too");
    /// assert!(store.token(&ExecutionId::fresh()).is_none());
    /// ```
    pub fn token(&self, id: &ExecutionId) -> Option<Arc<AtomicBool>> {
        self.lock().records.get(id).map(|r| Arc::clone(&r.cancel))
    }

    /// Ask an execution to stop. Sets the token and publishes `execution.cancelling`; the
    /// handler decides when it stops, and a handler that never looks at its token will
    /// finish normally, which the final state will say.
    ///
    /// A queued execution is the exception: nothing is holding it, so no handler will ever
    /// look at the token, and it is finished here instead of being left cancelling forever.
    /// Asking twice is not an error and asks nothing further — the answer says which of
    /// the four things happened, and none of them claims the execution has stopped.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// store.publish(&id, EventPayload::Started);
    /// assert_eq!(store.request_cancel(&id, "a client"), CancelOutcome::Requested);
    /// // it has been asked, and it is still running: stopping is the handler's to do
    /// assert_eq!(store.get(&id).unwrap().state, ExecutionState::Cancelling);
    /// assert_eq!(store.request_cancel(&id, "a client"), CancelOutcome::AlreadyRequested);
    /// // and a handler that stopped when it noticed
    /// store.publish(&id, EventPayload::Cancelled);
    /// assert_eq!(
    ///     store.request_cancel(&id, "a client"),
    ///     CancelOutcome::AlreadyFinished(ExecutionState::Cancelled),
    /// );
    /// ```
    pub fn request_cancel(&self, id: &ExecutionId, by: &str) -> CancelOutcome {
        let outcome = {
            let mut inner = self.lock();
            let Some(record) = inner.records.get_mut(id) else {
                return CancelOutcome::Unknown;
            };
            if record.snapshot.state.is_final() {
                return CancelOutcome::AlreadyFinished(record.snapshot.state);
            }
            if record.cancel_requested {
                CancelOutcome::AlreadyRequested
            } else {
                record.cancel_requested = true;
                record.cancel.store(true, Ordering::SeqCst);
                CancelOutcome::Requested
            }
        };
        if outcome == CancelOutcome::Requested {
            // a queued execution has no worker to notice the token, so it is finished here
            let queued = self
                .get(id)
                .is_some_and(|s| s.state == ExecutionState::Queued);
            if queued {
                self.publish(id, EventPayload::Cancelled);
            } else {
                self.publish(id, EventPayload::Cancelling { by: by.to_string() });
            }
        }
        outcome
    }

    /// Listen. The receiver is dropped to unsubscribe; the store forgets a subscriber the
    /// first time a send finds it gone, so nothing has to be cleaned up by hand.
    ///
    /// The returned id is for [`ExecutionStore::resubscribe`] and
    /// [`ExecutionStore::unsubscribe`]; the receiver is the stream. Only events published
    /// after this call arrive — the history before it is read with
    /// [`ExecutionStore::events`], which is why a client subscribes first and then reads
    /// from its cursor rather than the other way round.
    ///
    /// The channel is bounded by [`Limits::max_subscriber_queue`]: a subscriber that stops
    /// reading is told it lagged rather than allowed to hold up the execution.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// let (subscriber, events) = store.subscribe(Filter::Only(vec![id.clone()]));
    /// assert_eq!(store.subscribers(), 1);
    /// // the created event happened before this subscription and is not replayed
    /// assert!(events.try_recv().is_err());
    /// store.publish(&id, EventPayload::Started);
    /// assert!(matches!(events.try_recv(), Ok(Delivery::Event(_))));
    /// store.unsubscribe(subscriber);
    /// assert_eq!(store.subscribers(), 0);
    /// ```
    pub fn subscribe(&self, filter: Filter) -> (u64, std::sync::mpsc::Receiver<Delivery>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(self.limits.max_subscriber_queue);
        let id = self.next_subscriber.fetch_add(1, Ordering::Relaxed);
        self.lock().subscribers.push(Subscriber {
            id,
            filter,
            tx,
            dropped: 0,
        });
        (id, rx)
    }

    /// Change what a subscriber wants, without reconnecting.
    ///
    /// A browser that opens a second execution changes its filter rather than opening a
    /// second socket. `false` says there is no such subscriber — it has already gone — and
    /// is the caller's cue to stop, not a failure to retry.
    ///
    /// Only what arrives *next* is affected: an event already handed to the channel stays
    /// there, so a client that narrows its filter may still read one more event about
    /// something it no longer wants.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// let other = ExecutionId::fresh();
    /// let (subscriber, events) = store.subscribe(Filter::Only(vec![other]));
    /// store.publish(&id, EventPayload::Started);
    /// assert!(events.try_recv().is_err(), "it was watching something else");
    ///
    /// assert!(store.resubscribe(subscriber, Filter::Only(vec![id.clone()])));
    /// store.publish(&id, EventPayload::Cancelling { by: "a client".into() });
    /// assert!(matches!(events.try_recv(), Ok(Delivery::Event(_))));
    /// // and a subscriber that has gone cannot be redirected
    /// store.unsubscribe(subscriber);
    /// assert!(!store.resubscribe(subscriber, Filter::All));
    /// ```
    pub fn resubscribe(&self, subscriber: u64, filter: Filter) -> bool {
        let mut inner = self.lock();
        match inner.subscribers.iter_mut().find(|s| s.id == subscriber) {
            Some(s) => {
                s.filter = filter;
                true
            }
            None => false,
        }
    }

    /// Stop listening, by subscriber id.
    ///
    /// Idempotent and never an error: dropping the receiver unsubscribes anyway — the store
    /// forgets a subscriber the first time a send finds it gone — so this is for a caller
    /// that wants the slot released now rather than at the next event. Unsubscribing
    /// something that was never there, or that has already gone, does nothing.
    ///
    /// ```
    /// # use majordomus_cli::execution::*;
    /// # let store = ExecutionStore::new(Limits::default());
    /// # let id = ExecutionId::fresh();
    /// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    /// #     Actor::of(ActorKind::Internal),
    /// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    /// let (subscriber, events) = store.subscribe(Filter::All);
    /// assert_eq!(store.subscribers(), 1);
    /// store.unsubscribe(subscriber);
    /// assert_eq!(store.subscribers(), 0);
    /// store.unsubscribe(subscriber); // twice is not an error
    /// store.unsubscribe(9_999);      // and nor is a subscriber that never existed
    /// assert_eq!(store.subscribers(), 0);
    /// // the events published after it stopped listening never reach the channel
    /// store.publish(&id, EventPayload::Started);
    /// assert!(events.try_recv().is_err());
    /// ```
    pub fn unsubscribe(&self, subscriber: u64) {
        self.lock().subscribers.retain(|s| s.id != subscriber);
    }

    /// How many subscribers this store is feeding.
    pub fn subscribers(&self) -> usize {
        self.lock().subscribers.len()
    }
}

impl Inner {
    /// Hand one event to every subscriber that wants it, and forget the ones that are
    /// gone. Returns how many received it.
    fn fan_out(&mut self, id: &ExecutionId, event: &Arc<ExecutionEvent>) -> usize {
        let mut delivered = 0usize;
        let mut gone = Vec::new();
        for subscriber in self.subscribers.iter_mut() {
            if !subscriber.filter.wants(id) {
                continue;
            }
            if subscriber.dropped > 0 {
                // tell it once that it missed something, before anything else
                match subscriber.tx.try_send(Delivery::Lagged {
                    dropped: subscriber.dropped,
                }) {
                    Ok(()) => subscriber.dropped = 0,
                    Err(std::sync::mpsc::TrySendError::Full(_)) => {
                        subscriber.dropped += 1;
                        continue;
                    }
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                        gone.push(subscriber.id);
                        continue;
                    }
                }
            }
            match subscriber.tx.try_send(Delivery::Event(Arc::clone(event))) {
                Ok(()) => delivered += 1,
                // a slow client never stalls an execution: the event is dropped for that
                // client alone and it is told to resynchronise
                Err(std::sync::mpsc::TrySendError::Full(_)) => subscriber.dropped += 1,
                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => gone.push(subscriber.id),
            }
        }
        if !gone.is_empty() {
            self.subscribers.retain(|s| !gone.contains(&s.id));
        }
        delivered
    }
}

/// Fold one event into a snapshot. The only place a snapshot changes.
fn apply(record: &mut Record, payload: &EventPayload, sequence: u64, limits: Limits) {
    let now = crate::peers::rfc3339(std::time::SystemTime::now());
    let s = &mut record.snapshot;
    s.last_sequence = sequence;
    s.event_count += 1;
    if let Some(next) = payload.implies_state() {
        s.state = next;
    }
    match payload {
        EventPayload::Started => {
            s.started_at = Some(now.clone());
            record.started = Some(Instant::now());
        }
        EventPayload::Progress(p) => {
            s.progress = Some(ProgressView {
                current: p.current,
                total: p.total,
                message: p.message.clone(),
            });
        }
        EventPayload::StepStarted { name, title } => {
            s.steps.push(StepView {
                name: name.clone(),
                title: title.clone(),
                state: StepState::Running,
                started_at: now.clone(),
                finished_at: None,
                detail: None,
            });
        }
        EventPayload::StepCompleted { name, ok, detail } => {
            let state = if *ok {
                StepState::Completed
            } else {
                StepState::Failed
            };
            match s.steps.iter_mut().rev().find(|s| &s.name == name) {
                Some(step) => {
                    step.state = state;
                    step.finished_at = Some(now.clone());
                    step.detail.clone_from(detail);
                }
                // a handler that reports a phase as decided without having announced it
                // first is reporting the truth; inventing a start it never had would not be
                None => s.steps.push(StepView {
                    name: name.clone(),
                    title: name.clone(),
                    state,
                    started_at: now.clone(),
                    finished_at: Some(now.clone()),
                    detail: detail.clone(),
                }),
            }
        }
        EventPayload::Diagnostic(d) => {
            // bounded like everything else: a handler that reports a finding per file of a
            // large repository must not grow the snapshot every list request answers
            if s.diagnostics.len() < limits.max_events.min(200) {
                s.diagnostics.push(d.clone());
            }
        }
        EventPayload::Completed { output } => {
            s.output = Some(output.clone());
            s.error = None;
        }
        EventPayload::Failed { error } => {
            s.error = Some(error.clone());
            s.output = None;
        }
        // A cancellation after a failure keeps the failure: the guard, not a nested `if`,
        // because the arm below it is the no-op the other case already falls to.
        EventPayload::Cancelled if s.error.is_none() => {
            s.error = Some(ExecutionError {
                code: "cancelled".into(),
                message: "the execution was cancelled".into(),
                suggestion: None,
                correlation_id: s.correlation_id.clone(),
            });
        }
        _ => {}
    }
    if s.state.is_final() && s.finished_at.is_none() {
        s.finished_at = Some(now);
        s.duration_ms = record.started.map(|t| t.elapsed().as_millis() as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::model::{ActorKind, LogStream};

    fn store() -> ExecutionStore {
        ExecutionStore::new(Limits::default())
    }

    fn repository() -> RepositoryRef {
        RepositoryRef {
            name: "r".into(),
            id: "id".into(),
            branch: None,
        }
    }

    fn start(store: &ExecutionStore) -> ExecutionId {
        let id = ExecutionId::fresh();
        store.create(
            id.clone(),
            "demo.x",
            "Demo",
            serde_json::json!({}),
            true,
            Actor::of(ActorKind::Internal),
            repository(),
        );
        id
    }

    #[test]
    fn a_snapshot_and_the_stream_agree_at_every_step() {
        let store = store();
        let (_, rx) = store.subscribe(Filter::All);
        let id = start(&store);
        store.publish(&id, EventPayload::Started);
        store.publish(
            &id,
            EventPayload::Progress(ProgressView {
                current: 1,
                total: Some(2),
                message: Some("half".into()),
            }),
        );
        store.publish(
            &id,
            EventPayload::Completed {
                output: serde_json::json!({ "ok": true }),
            },
        );
        let snapshot = store.get(&id).expect("a snapshot");
        assert_eq!(snapshot.state, ExecutionState::Succeeded);
        assert_eq!(snapshot.output, Some(serde_json::json!({ "ok": true })));
        assert_eq!(snapshot.percent(), Some(50));
        assert!(snapshot.finished_at.is_some());
        assert_eq!(snapshot.last_sequence, 4);
        assert_eq!(snapshot.event_count, 4);

        let seen: Vec<Arc<ExecutionEvent>> = rx
            .try_iter()
            .map(|d| match d {
                Delivery::Event(e) => e,
                Delivery::Lagged { .. } => panic!("a reader that kept up was told it lagged"),
            })
            .collect();
        assert_eq!(seen.len(), 4);
        assert_eq!(
            seen.iter().map(|e| e.sequence).collect::<Vec<_>>(),
            [1, 2, 3, 4],
            "sequences are dense and ordered"
        );
        assert_eq!(seen.last().unwrap().type_name(), "execution.completed");
    }

    #[test]
    fn a_refused_transition_changes_nothing() {
        let store = store();
        let id = start(&store);
        store.publish(&id, EventPayload::Started);
        store.publish(
            &id,
            EventPayload::Completed {
                output: Value::Null,
            },
        );
        let before = store.get(&id).unwrap();
        assert_eq!(
            store.publish(&id, EventPayload::Started),
            None,
            "a finished execution does not start again"
        );
        assert_eq!(
            store.publish(
                &id,
                EventPayload::Failed {
                    error: ExecutionError {
                        code: "internal".into(),
                        message: "no".into(),
                        suggestion: None,
                        correlation_id: id.to_string()
                    }
                }
            ),
            None
        );
        assert_eq!(store.get(&id).unwrap(), before);
        assert_eq!(
            store.publish(&ExecutionId::fresh(), EventPayload::Started),
            None
        );
    }

    #[test]
    fn history_replays_from_a_cursor_and_says_when_it_lost_the_start() {
        let limits = Limits {
            max_events: 4,
            ..Limits::default()
        };
        let store = ExecutionStore::new(limits);
        let id = start(&store);
        store.publish(&id, EventPayload::Started);
        for n in 0..6 {
            store.publish(
                &id,
                EventPayload::Log {
                    stream: LogStream::Handler,
                    message: format!("line {n}"),
                },
            );
        }
        let page = store.events(&id, 0, 100).expect("a page");
        assert_eq!(page.events.len(), 4, "the bound holds");
        assert!(page.truncated, "and says the start is gone");
        let snapshot = store.get(&id).unwrap();
        assert!(snapshot.events_truncated);
        assert_eq!(snapshot.event_count, 8, "counted, not retained");

        let last = page.last_sequence;
        let after = store.events(&id, last, 100).unwrap();
        assert!(after.events.is_empty());
        assert!(!after.more);
        assert_eq!(after.last_sequence, last);

        let paged = store.events(&id, 4, 2).unwrap();
        assert_eq!(paged.events.len(), 2);
        assert!(paged.more);
    }

    #[test]
    fn a_slow_subscriber_is_told_to_resynchronise_and_never_stalls_the_execution() {
        let store = ExecutionStore::new(Limits {
            max_subscriber_queue: 2,
            ..Limits::default()
        });
        let (_, rx) = store.subscribe(Filter::All);
        let id = start(&store); // fills the queue: created is 1 of 2
        for n in 0..20 {
            store.publish(
                &id,
                EventPayload::Log {
                    stream: LogStream::Handler,
                    message: n.to_string(),
                },
            );
        }
        // the execution kept going regardless of the reader
        assert_eq!(store.get(&id).unwrap().event_count, 21);
        let deliveries: Vec<Delivery> = rx.try_iter().collect();
        assert!(deliveries.len() <= 3, "the queue stayed bounded");
        // drain and then publish once more: the reader is told what it missed
        store.publish(&id, EventPayload::Started);
        let mut told = false;
        for d in rx.try_iter() {
            if let Delivery::Lagged { dropped } = d {
                assert!(dropped > 0);
                told = true;
            }
        }
        assert!(told, "a lagging subscriber is told, not silently starved");
    }

    #[test]
    fn a_dropped_receiver_is_forgotten_on_the_next_event() {
        let store = store();
        let (_, rx) = store.subscribe(Filter::All);
        assert_eq!(store.subscribers(), 1);
        drop(rx);
        let id = start(&store);
        store.publish(&id, EventPayload::Started);
        assert_eq!(store.subscribers(), 0);
    }

    #[test]
    fn a_filter_narrows_and_can_be_changed_without_reconnecting() {
        let store = store();
        let a = start(&store);
        let (sub, rx) = store.subscribe(Filter::Only(vec![a.clone()]));
        let b = start(&store);
        store.publish(&b, EventPayload::Started);
        assert!(rx.try_iter().next().is_none(), "b was not wanted");
        assert!(store.resubscribe(sub, Filter::Only(vec![b.clone()])));
        store.publish(
            &b,
            EventPayload::Progress(ProgressView {
                current: 1,
                total: None,
                message: None,
            }),
        );
        assert!(matches!(rx.try_recv(), Ok(Delivery::Event(_))));
        store.unsubscribe(sub);
        assert_eq!(store.subscribers(), 0);
        assert!(!store.resubscribe(sub, Filter::All));
    }

    #[test]
    fn cancelling_a_queued_execution_finishes_it_and_cancelling_twice_says_so() {
        let store = store();
        let id = start(&store);
        assert_eq!(store.request_cancel(&id, "test"), CancelOutcome::Requested);
        assert_eq!(store.get(&id).unwrap().state, ExecutionState::Cancelled);
        assert_eq!(
            store.request_cancel(&id, "test"),
            CancelOutcome::AlreadyFinished(ExecutionState::Cancelled)
        );
        assert_eq!(
            store.request_cancel(&ExecutionId::fresh(), "test"),
            CancelOutcome::Unknown
        );

        let running = start(&store);
        store.publish(&running, EventPayload::Started);
        assert_eq!(
            store.request_cancel(&running, "test"),
            CancelOutcome::Requested
        );
        assert!(store.token(&running).unwrap().load(Ordering::SeqCst));
        assert_eq!(
            store.get(&running).unwrap().state,
            ExecutionState::Cancelling
        );
        assert_eq!(
            store.request_cancel(&running, "test"),
            CancelOutcome::AlreadyRequested
        );
        store.publish(&running, EventPayload::Cancelled);
        let snapshot = store.get(&running).unwrap();
        assert_eq!(snapshot.state, ExecutionState::Cancelled);
        assert_eq!(
            snapshot.error.map(|e| e.code),
            Some("cancelled".to_string())
        );
    }

    #[test]
    fn the_oldest_finished_execution_is_forgotten_and_a_running_one_never_is() {
        let store = ExecutionStore::new(Limits {
            max_executions: 2,
            ..Limits::default()
        });
        let running = start(&store);
        store.publish(&running, EventPayload::Started);
        let mut finished = Vec::new();
        for _ in 0..4 {
            let id = start(&store);
            store.publish(&id, EventPayload::Started);
            store.publish(
                &id,
                EventPayload::Completed {
                    output: Value::Null,
                },
            );
            finished.push(id);
        }
        assert!(
            store.get(&running).is_some(),
            "work in flight is not evicted"
        );
        assert!(
            store.get(finished.last().unwrap()).is_some(),
            "the newest is kept"
        );
        assert!(
            store.get(&finished[0]).is_none(),
            "the oldest finished is gone"
        );
        assert_eq!(store.active(), 1);
        assert_eq!(
            store.count(ExecutionState::Succeeded),
            store.list(Some(ExecutionState::Succeeded), None, 100).len()
        );
    }

    #[test]
    fn listing_is_newest_first_and_narrows_by_state_and_capability() {
        let store = store();
        let a = start(&store);
        let b = start(&store);
        store.publish(&b, EventPayload::Started);
        let all = store.list(None, None, 10);
        assert_eq!(all.first().map(|e| e.id.clone()), Some(b.clone()));
        assert_eq!(all.len(), 2);
        assert_eq!(store.list(Some(ExecutionState::Queued), None, 10)[0].id, a);
        assert_eq!(store.list(None, Some("demo.x"), 10).len(), 2);
        assert!(store.list(None, Some("nothing.here"), 10).is_empty());
        assert_eq!(store.list(None, None, 1).len(), 1);
    }
}
