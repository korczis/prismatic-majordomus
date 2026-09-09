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
    pub fn silent() -> Self {
        Progress(None)
    }

    /// A handle that publishes into a store, for one execution.
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
    pub fn execution_id(&self) -> Option<&ExecutionId> {
        self.0.as_ref().map(|r| &r.id)
    }

    /// Has this execution been asked to stop?
    ///
    /// Cancellation is cooperative and this is the whole of it: a handler that never asks
    /// runs to completion, and the capability's own policy is what tells a client whether
    /// asking will achieve anything.
    pub fn cancelled(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|r| r.cancel.load(Ordering::SeqCst))
    }

    /// The error a handler returns when it stops because it was asked to.
    pub fn cancellation(&self) -> crate::capability::CapabilityError {
        crate::capability::CapabilityError::Refused("the execution was cancelled".into())
    }

    /// Enter a named phase.
    pub fn step(&self, name: &str, title: &str) {
        self.emit(EventPayload::StepStarted {
            name: name.to_string(),
            title: title.to_string(),
        });
    }

    /// Leave a named phase, saying whether what it was asked to do happened.
    pub fn step_done(&self, name: &str, ok: bool, detail: Option<String>) {
        self.emit(EventPayload::StepCompleted {
            name: name.to_string(),
            ok,
            detail: detail.map(|d| sanitise(&d, self.limit())),
        });
    }

    /// How far along.
    pub fn progress(&self, current: u64, total: Option<u64>, message: impl AsRef<str>) {
        let message = message.as_ref();
        self.emit(EventPayload::Progress(ProgressView {
            current,
            total,
            message: (!message.is_empty()).then(|| sanitise(message, self.limit())),
        }));
    }

    /// A line of output, from the handler itself.
    pub fn log(&self, message: impl AsRef<str>) {
        self.stream(LogStream::Handler, message);
    }

    /// A line of output, from a named stream.
    pub fn stream(&self, stream: LogStream, message: impl AsRef<str>) {
        let limit = self.limit();
        self.emit(EventPayload::Log {
            stream,
            message: sanitise(message.as_ref(), limit),
        });
    }

    /// A structured finding.
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
