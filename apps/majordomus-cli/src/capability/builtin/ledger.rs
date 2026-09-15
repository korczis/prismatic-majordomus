//! The `ledger` module: the one writer of the checkout's ledger, offered to the shell tool.
//!
//! `lib/common.sh`'s `mj_ledger_append` appended with a bare `>>` while this executable
//! appended under an exclusive lock, so two programs raced on one file and only one of them
//! took the lock (I1700). The shell now composes nothing: it hands the event name and its
//! payload to `majordomus ledger append`, and the line is composed, validated against
//! `share/events.yaml`, stamped with the episode [`crate::session::resolver`] resolves, and
//! written through [`crate::session::Ledger`]'s locked append — the same path
//! [`crate::ledger::append`] takes for a capability of this executable.
//!
//! Command line only. It writes a file in the checkout, and every surface a network reaches
//! is held to reads unless a decision says otherwise (I1704 carries that decision); the
//! caller that needs it is a process on this machine, in this checkout.
//!
//! ```
//! use majordomus_cli::capability::builtin::ledger;
//!
//! let m = ledger::module();
//! let append = &m.capabilities[0].capability;
//! assert_eq!(append.id.as_str(), "ledger.append");
//! assert!(append.exposure.mcp.is_none() && append.exposure.http.is_none());
//! assert_eq!(append.exposure.cli.as_ref().unwrap().path, ["ledger", "append"]);
//! ```

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CapabilityKind, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::ledger::{LedgerError, Vocabulary};
use crate::{capability, module};

/// The variable the shell tool reads for "now", honoured here so that a line written for the
/// shell carries the shell's clock.
pub const NOW_ENV: &str = "MAJORDOMUS_NOW";

/// One event to append.
///
/// ```
/// use majordomus_cli::capability::builtin::ledger::LedgerAppendInput;
/// let input: LedgerAppendInput =
///     serde_json::from_str(r#"{"event":"plan_start","payload":{"issue":"I0001"}}"#).unwrap();
/// assert_eq!(input.event, "plan_start");
/// assert_eq!(input.payload["issue"], "I0001");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LedgerAppendInput {
    /// The event name, as `share/events.yaml` declares it.
    pub event: String,
    /// The event's own fields, as one JSON object; the envelope is composed here.
    #[serde(default = "empty_object")]
    pub payload: serde_json::Value,
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

impl BenchmarkCases for LedgerAppendInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing: timing an append in a loop would write the ledger it is timing
        Vec::new()
    }
}

/// The line that was written, and the episode it names.
///
/// ```
/// use majordomus_cli::capability::builtin::ledger::LedgerAppendReport;
/// let r = LedgerAppendReport { event: "plan_start".into(), session: None, line: "{}".into() };
/// assert!(serde_json::to_value(&r).unwrap().get("session").is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LedgerAppendReport {
    /// The event that was appended.
    pub event: String,
    /// The episode the envelope names, when one resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The line, exactly as it was written.
    pub line: String,
}

/// Append one event to the ledger of `root`, validating it against the vocabulary under
/// `share`. The payload is a JSON object's text, copied into the line verbatim.
///
/// The one body both callers run: the capability's handler, and `majordomus ledger append`,
/// which the shell calls on every recorded event and which therefore does not build the
/// repository index a handler's context carries.
pub(crate) fn append(
    root: &Path,
    share: &Path,
    event: &str,
    payload: &str,
) -> Result<LedgerAppendReport, LedgerError> {
    let vocabulary = Vocabulary::load(&share.join("events.yaml"))?;
    let now = std::env::var(NOW_ENV)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(crate::ledger::now);
    let git = crate::ledger::head_and_branch(root);
    let line = crate::ledger::append_object(root, &vocabulary, &git, &now, event, payload)?;
    let session = serde_json::from_str::<crate::ledger::Entry>(&line)
        .ok()
        .and_then(|e| e.session);
    Ok(LedgerAppendReport {
        event: event.to_string(),
        session,
        line,
    })
}

fn append_handler(
    ctx: &Context,
    input: LedgerAppendInput,
) -> Result<LedgerAppendReport, CapabilityError> {
    if !input.payload.is_object() {
        return Err(CapabilityError::InvalidInput(
            "payload is one JSON object of the event's own fields".into(),
        ));
    }
    let share = ctx.index.share.as_ref().ok_or_else(|| {
        CapabilityError::Refused(
            "this index was built without a share directory, so the event vocabulary cannot be read; nothing is appended that cannot be validated".into(),
        )
    })?;
    let root = Path::new(&ctx.index.repository.root);
    append(root, share, &input.event, &input.payload.to_string()).map_err(|e| match e {
        LedgerError::Write { .. } => CapabilityError::Internal(e.to_string()),
        other => CapabilityError::Refused(other.to_string()),
    })
}

/// The module.
///
/// ```
/// let m = majordomus_cli::capability::builtin::ledger::module();
/// assert_eq!(m.id.as_str(), "ledger");
/// assert!(m.capabilities.iter().all(|e| !e.capability.cache.is_enabled()));
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "ledger",
        title: "Ledger",
        description: "The checkout's append-only record of what happened, and its one writer. An event is validated against share/events.yaml, wrapped in the envelope every line carries — the time, the event, the commit, the branch, the writer and the episode this process resolves to — and appended under the exclusive lock every writer of the file takes.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "ledger.append",
                kind: CapabilityKind::Command,
                title: "Append one event to the ledger",
                description: "Validates the event against share/events.yaml — a declared name, every required field, no field the envelope owns — composes the envelope (ts, event, head, branch, by, and session when an episode resolves: the hook's key strictly, then the provider session this process runs inside, then the pointer), copies the payload's members into the line as they were written, and appends it under the ledger's exclusive lock. The shell tool's every recorded event goes through it, so the file has one writer and one lock. A refusal writes nothing.",
                input: LedgerAppendInput,
                output: LedgerAppendReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["ledger".into(), "append".into()] }),
                },
                tags: ["ledger", "sessions", "lifecycle", "provenance"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: append_handler,
            }
            .writes_repository(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("events.yaml"),
            "version: 1\nevents:\n  - id: plan_start\n    requires: [issue]\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn an_append_writes_one_line_and_reports_it() {
        let share = share();
        let root = tempfile::tempdir().unwrap();
        let r = append(root.path(), share.path(), "plan_start", r#"{"issue":"I1"}"#).unwrap();
        assert!(r.line.ends_with(r#","issue":"I1"}"#), "{}", r.line);
        assert_eq!(crate::ledger::read(root.path()).0.len(), 1);
    }

    #[test]
    fn a_share_without_a_vocabulary_refuses_every_event() {
        let empty = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let err = append(root.path(), empty.path(), "plan_start", r#"{"issue":"I1"}"#).unwrap_err();
        assert!(matches!(err, LedgerError::Vocabulary { .. }), "{err}");
        assert!(crate::ledger::read(root.path()).0.is_empty());
    }
}
