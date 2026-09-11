//! What a handler reports with, and how it learns it should stop.
//!
//! A handler is written once and reached by every transport. Most of them have no
//! execution behind them — a command line call, an MCP tool call, a benchmark sample — so
//! reporting has to be free to call and do nothing when nobody is listening. That is what
//! [`Progress`] is: a handle every [`crate::capability::Context`] carries, silent unless
//! the call came through the execution engine.
//!
//! The consequence is the point of the design. `health.report` does not know whether it is
//! answering `GET /api/v1/health`, an MCP tool call or a browser watching it run; it
//! reports its steps the same way in all three, and only the execution engine turns those
//! reports into events anybody can see.
//!
//! ```
//! use std::sync::Arc;
//! use std::sync::atomic::AtomicBool;
//! use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
//!     Progress, RepositoryRef};
//!
//! // the handler's own code, written once and unaware of who is watching
//! fn handler(progress: &Progress) {
//!     progress.step("scan", "Scanning the index");
//!     progress.progress(1, Some(2), "half way");
//!     progress.step_done("scan", true, None);
//! }
//!
//! // called outside an execution: every report is a no-op
//! handler(&Progress::silent());
//!
//! // and called through the engine: the same reports become events somebody can read
//! let store = Arc::new(ExecutionStore::new(Limits::default()));
//! let id = ExecutionId::fresh();
//! let cancel = store.create(id.clone(), "demo.scan", "Scan", serde_json::json!({}), true,
//!     Actor::of(ActorKind::Internal),
//!     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
//! handler(&Progress::reporting(Arc::clone(&store), id.clone(), cancel));
//!
//! let page = store.events(&id, 0, 10).unwrap();
//! let types: Vec<&str> = page.events.iter().map(|e| e.type_name()).collect();
//! assert_eq!(types, ["execution.created", "execution.step.started",
//!                    "execution.progress", "execution.step.completed"]);
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::event::{sanitise, EventPayload};
use super::model::{ExecutionDiagnostic, ExecutionId, LogStream, ProgressView};
use super::store::ExecutionStore;

/// The one thing a handler is given to say what it is doing.
///
/// Cheap to clone and safe to hold: cloning it does not create a second stream, and a
/// silent handle costs nothing to call.
///
/// ```
/// use majordomus_cli::execution::Progress;
/// // what a handler outside an execution is given
/// let silent = Progress::silent();
/// silent.step("scan", "Scanning");
/// silent.progress(1, Some(2), "half way");
/// silent.step_done("scan", true, None);
/// assert!(!silent.cancelled(), "nothing can cancel a call that is not an execution");
/// assert!(!silent.is_reporting());
/// ```
#[derive(Clone, Default)]
pub struct Progress(Option<Arc<Reporter>>);

struct Reporter {
    store: Arc<ExecutionStore>,
    id: ExecutionId,
    cancel: Arc<AtomicBool>,
    max_log_chars: usize,
}

impl std::fmt::Debug for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(r) => write!(f, "Progress({})", r.id),
            None => f.write_str("Progress(silent)"),
        }
    }
}

impl Progress {
    /// A handle that reports nothing: what every call outside an execution is given.
    ///
    /// Not an error case and not a fallback: most calls have no execution behind them, so
    /// this is the ordinary handle and reporting into it has to be free. It is also the
    /// [`Default`], so a `Context` assembled without one is silent rather than broken.
    ///
    /// ```
    /// use majordomus_cli::execution::Progress;
    /// let silent = Progress::silent();
    /// assert!(!silent.is_reporting());
    /// assert_eq!(silent.execution_id(), None);
    /// // every report is accepted and goes nowhere; none of these can fail
    /// silent.step("scan", "Scanning");
    /// silent.log("a line nobody reads");
    /// assert!(!silent.cancelled(), "there is nothing to cancel");
    /// ```
    pub fn silent() -> Self {
        Progress(None)
    }

    /// A handle that publishes into a store, for one execution.
    ///
    /// The engine builds one of these per execution and hands it to the handler; nothing
    /// else should. The log bound is read from the store once, here, so a handler cannot
    /// be made to hold the store's lock by logging.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
    ///     Progress, RepositoryRef};
    /// let store = Arc::new(ExecutionStore::new(Limits::default()));
    /// let id = ExecutionId::fresh();
    /// let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
    ///     Actor::of(ActorKind::Internal),
    ///     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
    ///
    /// let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
    /// assert!(progress.is_reporting());
    /// assert_eq!(progress.execution_id(), Some(&id));
    /// // a clone is the same stream and not a second one
    /// progress.clone().log("from a clone");
    /// assert_eq!(store.get(&id).unwrap().last_sequence, 2, "the created event, then the log");
    /// ```
    pub fn reporting(store: Arc<ExecutionStore>, id: ExecutionId, cancel: Arc<AtomicBool>) -> Self {
        let max_log_chars = store.limits().max_log_chars;
        Progress(Some(Arc::new(Reporter {
            store,
            id,
            cancel,
            max_log_chars,
        })))
    }

    /// Is anybody listening? A handler that would do real work to produce a report — count
    /// the files it is about to walk, say — asks first.
    pub fn is_reporting(&self) -> bool {
        self.0.is_some()
    }

    /// The execution this reports into, when there is one.
    ///
    /// `None` is the ordinary case rather than an error: it says this call is not an
    /// execution, so a handler that wants to name the execution in its own output — a
    /// correlation id in a log line — has something to check first.
    ///
    /// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
/// #     Progress, RepositoryRef};
/// # let store = Arc::new(ExecutionStore::new(Limits::default()));
/// # let id = ExecutionId::fresh();
/// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
/// #     Actor::of(ActorKind::Internal),
/// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// # let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
/// assert_eq!(progress.execution_id(), Some(&id));
    /// assert_eq!(majordomus_cli::execution::Progress::silent().execution_id(), None);
    /// ```
    pub fn execution_id(&self) -> Option<&ExecutionId> {
        self.0.as_ref().map(|r| &r.id)
    }

    /// Has this execution been asked to stop?
    ///
    /// Cancellation is cooperative and this is the whole of it: a handler that never asks
    /// runs to completion, and the capability's own policy is what tells a client whether
    /// asking will achieve anything.
    ///
    /// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
/// #     Progress, RepositoryRef};
/// # let store = Arc::new(ExecutionStore::new(Limits::default()));
/// # let id = ExecutionId::fresh();
/// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
/// #     Actor::of(ActorKind::Internal),
/// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// # let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
/// assert!(!progress.cancelled());
    /// // somebody asks the store to stop it, and the handler's next check sees that
    /// store.request_cancel(&id, "a client");
    /// assert!(progress.cancelled(), "the token the handler watches is the store's own");
    /// ```
    pub fn cancelled(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|r| r.cancel.load(Ordering::SeqCst))
    }

    /// The error a handler returns when it stops because it was asked to.
    ///
    /// One error for every handler, so a client can tell a cancelled call from a failed one
    /// without every capability inventing its own word for it. A handler that noticed
    /// [`Progress::cancelled`] returns this instead of finishing.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityError;
    /// use majordomus_cli::execution::Progress;
    /// let error = Progress::silent().cancellation();
    /// assert!(matches!(error, CapabilityError::Refused(_)));
    /// assert!(error.to_string().contains("cancelled"), "{error}");
    /// ```
    pub fn cancellation(&self) -> crate::capability::CapabilityError {
        crate::capability::CapabilityError::Refused("the execution was cancelled".into())
    }

    /// Enter a named phase.
    ///
    /// `name` is a stable identity a client matches the completion against, and `title` is
    /// the line a person reads; a step that is entered and never completed is a step a
    /// reader can see is still running. Nothing here checks that the name is unique or
    /// that the phase is ever left — a handler that lies about its own structure produces
    /// a confusing report and not a broken store.
    ///
    /// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
/// #     Progress, RepositoryRef};
/// # let store = Arc::new(ExecutionStore::new(Limits::default()));
/// # let id = ExecutionId::fresh();
/// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
/// #     Actor::of(ActorKind::Internal),
/// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// # let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
/// progress.step("scan", "Scanning the index");
    /// let snapshot = store.get(&id).unwrap();
    /// assert_eq!(snapshot.steps.len(), 1);
    /// assert_eq!(snapshot.steps[0].name, "scan");
    /// assert!(snapshot.steps[0].finished_at.is_none(), "it has been entered, not left");
    /// ```
    pub fn step(&self, name: &str, title: &str) {
        self.emit(EventPayload::StepStarted {
            name: name.to_string(),
            title: title.to_string(),
        });
    }

    /// Leave a named phase, saying whether what it was asked to do happened.
    ///
    /// A step that failed is not an execution that failed: a handler may report a failed
    /// phase and still succeed. The outcome of the call is what the handler returns, and
    /// this is what it did on the way.
    ///
    /// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
/// #     Progress, RepositoryRef};
/// # let store = Arc::new(ExecutionStore::new(Limits::default()));
/// # let id = ExecutionId::fresh();
/// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
/// #     Actor::of(ActorKind::Internal),
/// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// # let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
/// progress.step("scan", "Scanning the index");
    /// progress.step_done("scan", false, Some("two files could not be read".into()));
    /// let step = &store.get(&id).unwrap().steps[0];
    /// assert!(step.finished_at.is_some());
    /// assert_eq!(step.detail.as_deref(), Some("two files could not be read"));
    /// // and the execution itself has not failed because a phase did
    /// assert!(!store.get(&id).unwrap().state.is_final());
    /// ```
    pub fn step_done(&self, name: &str, ok: bool, detail: Option<String>) {
        self.emit(EventPayload::StepCompleted {
            name: name.to_string(),
            ok,
            detail: detail.map(|d| sanitise(&d, self.limit())),
        });
    }

    /// How far along, in whatever units the handler counts in.
    ///
    /// The total is optional because a handler often does not know it, and an open-ended
    /// report is more honest than a made-up denominator: a client shows a count rather
    /// than a bar. An empty message is carried as no message rather than as an empty line.
    ///
    /// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
/// #     Progress, RepositoryRef};
/// # let store = Arc::new(ExecutionStore::new(Limits::default()));
/// # let id = ExecutionId::fresh();
/// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
/// #     Actor::of(ActorKind::Internal),
/// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// # let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
/// progress.progress(3, Some(4), "rendering");
    /// let snapshot = store.get(&id).unwrap();
    /// assert_eq!(snapshot.percent(), Some(75));
    ///
    /// // a handler that does not know how much work there is says so
    /// progress.progress(3, None, "");
    /// let open = store.get(&id).unwrap();
    /// assert_eq!(open.percent(), None, "no total, no percentage");
    /// assert!(open.progress.unwrap().message.is_none(), "an empty line is not a message");
    /// ```
    pub fn progress(&self, current: u64, total: Option<u64>, message: impl AsRef<str>) {
        let message = message.as_ref();
        self.emit(EventPayload::Progress(ProgressView {
            current,
            total,
            message: (!message.is_empty()).then(|| sanitise(message, self.limit())),
        }));
    }

    /// A line of output, from the handler itself.
    ///
    /// For what the handler wants a reader to see, as distinct from what a child process
    /// said — that is [`Progress::stream`]. Either way the text is sanitised and bounded
    /// before anybody sees it.
    ///
    /// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
/// #     Progress, RepositoryRef};
/// # let store = Arc::new(ExecutionStore::new(Limits::default()));
/// # let id = ExecutionId::fresh();
/// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
/// #     Actor::of(ActorKind::Internal),
/// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// # let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
/// progress.log("reading 109 submodules");
    /// let page = store.events(&id, 1, 10).unwrap();
    /// let event = serde_json::to_value(&*page.events[0]).unwrap();
    /// assert_eq!(event["type"], "execution.log");
    /// assert_eq!(event["data"]["stream"], "handler");
    /// assert_eq!(event["data"]["message"], "reading 109 submodules");
    /// ```
    pub fn log(&self, message: impl AsRef<str>) {
        self.stream(LogStream::Handler, message);
    }

    /// A line of output, from a named stream.
    ///
    /// This is where a child process's output enters the event stream, which is why the
    /// sanitising happens here and not in a consumer: a terminal escape from somebody
    /// else's tool would otherwise repaint a terminal that never chose it, and a browser
    /// would render a control character.
    ///
    /// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
/// #     Progress, RepositoryRef};
/// # let store = Arc::new(ExecutionStore::new(Limits::default()));
/// # let id = ExecutionId::fresh();
/// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
/// #     Actor::of(ActorKind::Internal),
/// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// # let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
/// use majordomus_cli::execution::LogStream;
    /// progress.stream(LogStream::Stderr, "\u{1b}[31mwarning: two files\u{1b}[0m");
    /// let page = store.events(&id, 1, 10).unwrap();
    /// let event = serde_json::to_value(&*page.events[0]).unwrap();
    /// assert_eq!(event["data"]["stream"], "stderr");
    /// assert_eq!(event["data"]["message"], "warning: two files", "no escape reaches a client");
    /// ```
    pub fn stream(&self, stream: LogStream, message: impl AsRef<str>) {
        let limit = self.limit();
        self.emit(EventPayload::Log {
            stream,
            message: sanitise(message.as_ref(), limit),
        });
    }

    /// A structured finding, which is not the same thing as an outcome.
    ///
    /// A handler that noticed something worth reporting — a stale artifact, a rule only
    /// warned about — says it here and still succeeds. The findings are kept on the
    /// snapshot, so a client that joined late reads them without replaying the stream.
    ///
    /// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::execution::{Actor, ActorKind, ExecutionId, ExecutionStore, Limits,
/// #     Progress, RepositoryRef};
/// # let store = Arc::new(ExecutionStore::new(Limits::default()));
/// # let id = ExecutionId::fresh();
/// # let cancel = store.create(id.clone(), "demo.echo", "Echo", serde_json::json!({}), true,
/// #     Actor::of(ActorKind::Internal),
/// #     RepositoryRef { name: "r".into(), id: "i".into(), branch: None });
/// # let progress = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
/// use majordomus_cli::execution::ExecutionDiagnostic;
    /// progress.diagnostic(ExecutionDiagnostic {
    ///     severity: majordomus_cli::model::Severity::Warning,
    ///     code: "stale-artifact".into(),
    ///     summary: "docs/generated is behind the registry".into(),
    ///     detail: None,
    ///     suggestion: Some("run majordomus generate".into()),
    /// });
    /// let snapshot = store.get(&id).unwrap();
    /// assert_eq!(snapshot.diagnostics.len(), 1);
    /// assert_eq!(snapshot.diagnostics[0].code, "stale-artifact");
    /// assert!(!snapshot.state.is_final(), "a finding is not an outcome");
    /// ```
    pub fn diagnostic(&self, diagnostic: ExecutionDiagnostic) {
        self.emit(EventPayload::Diagnostic(diagnostic));
    }

    fn limit(&self) -> usize {
        self.0.as_ref().map(|r| r.max_log_chars).unwrap_or(2_000)
    }

    fn emit(&self, payload: EventPayload) {
        if let Some(r) = &self.0 {
            r.store.publish(&r.id, payload);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::model::{Actor, ActorKind, RepositoryRef};
    use crate::execution::store::{Delivery, Filter, Limits};

    #[test]
    fn a_silent_handle_reports_nothing_and_costs_nothing() {
        let p = Progress::silent();
        assert!(!p.is_reporting());
        assert_eq!(p.execution_id(), None);
        assert!(!p.cancelled());
        p.step("a", "A");
        p.progress(1, Some(2), "x");
        p.log("y");
        p.stream(LogStream::Stderr, "z");
        p.step_done("a", true, Some("done".into()));
        p.diagnostic(ExecutionDiagnostic {
            severity: crate::model::Severity::Info,
            code: "c".into(),
            summary: "s".into(),
            detail: None,
            suggestion: None,
        });
        assert_eq!(format!("{p:?}"), "Progress(silent)");
    }

    #[test]
    fn a_reporting_handle_publishes_what_the_handler_says_and_sanitises_it() {
        let store = Arc::new(ExecutionStore::new(Limits::default()));
        let id = ExecutionId::fresh();
        let cancel = store.create(
            id.clone(),
            "demo.x",
            "Demo",
            serde_json::json!({}),
            true,
            Actor::of(ActorKind::Internal),
            RepositoryRef {
                name: "r".into(),
                id: "i".into(),
                branch: None,
            },
        );
        let (_, rx) = store.subscribe(Filter::All);
        let p = Progress::reporting(Arc::clone(&store), id.clone(), Arc::clone(&cancel));
        assert!(p.is_reporting());
        assert_eq!(p.execution_id(), Some(&id));
        assert!(format!("{p:?}").contains(id.as_str()));
        p.step("scan", "Scanning");
        p.stream(LogStream::Stderr, "\u{1b}[31mred\u{1b}[0m");
        p.step_done("scan", true, None);
        let types: Vec<String> = rx
            .try_iter()
            .filter_map(|d| match d {
                Delivery::Event(e) => Some(e.type_name().to_string()),
                Delivery::Lagged { .. } => None,
            })
            .collect();
        assert_eq!(
            types,
            [
                "execution.step.started",
                "execution.log",
                "execution.step.completed"
            ]
        );
        let page = store.events(&id, 1, 10).unwrap();
        let log = page
            .events
            .iter()
            .find(|e| e.type_name() == "execution.log")
            .expect("the log event");
        let value = serde_json::to_value(&**log).unwrap();
        assert_eq!(
            value["data"]["message"], "red",
            "escapes never reach a client"
        );
        assert_eq!(value["data"]["stream"], "stderr");

        assert!(!p.cancelled());
        cancel.store(true, Ordering::SeqCst);
        assert!(p.cancelled());
        assert!(matches!(
            p.cancellation(),
            crate::capability::CapabilityError::Refused(_)
        ));
    }
}
