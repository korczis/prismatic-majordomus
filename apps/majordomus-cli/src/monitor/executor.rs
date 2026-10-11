//! The executor's trail, folded into signals.
//!
//! The trail records every step of the pull-request lane: each observation, each selection,
//! each attempt before its outcome. A reader wants the outcomes — merged, refreshed, closed,
//! repaired, failed — and who holds the lane; the steps that decided nothing are counted as
//! suppressed. The trail has no sequence of its own, so a position is how many of its lines
//! were read; a trail shorter than the position (rewritten) reads as having nothing new.

use crate::integration::drain::{IntegrationAction, IntegrationEvent};
use crate::peers::parse_rfc3339;

use super::{MonitorSeverity, MonitorSignal, MonitorSource, Reader, SourceRead};

/// Fold `events` (the whole trail, oldest first) after `after` lines into signals; without
/// a position, only events at or after `since` (Unix seconds) are shown.
pub(crate) fn fold(
    events: &[IntegrationEvent],
    after: Option<usize>,
    since: u64,
    reader: &Reader,
) -> SourceRead<usize> {
    let mut read = SourceRead::default();
    let start = after.unwrap_or(0).min(events.len());
    for (offset, event) in events.iter().enumerate().skip(start) {
        let position = offset + 1;
        read.high = Some(position);
        let at = parse_rfc3339(&event.at).unwrap_or(0);
        if after.is_none() && at < since {
            continue;
        }
        match signal(event, at, reader) {
            Some(s) => read.signals.push((position, s)),
            None => read.suppressed += 1,
        }
    }
    if after.is_some() && read.high.is_none() {
        read.high = after.map(|a| a.min(events.len()));
    }
    read
}

/// The signal an executor event is, if it is an outcome.
fn signal(event: &IntegrationEvent, at: u64, reader: &Reader) -> Option<MonitorSignal> {
    use IntegrationAction as A;
    use MonitorSeverity::{Attention, Info};
    let pr = event.pr.map(|n| format!("#{n}")).unwrap_or_default();
    let (kind, severity, text) = match event.action {
        A::MergeSucceeded => ("merged", Info, format!("merged {pr}")),
        A::MergeFailed => ("merge_failed", Attention, format!("could not merge {pr}")),
        A::VerificationFailed => (
            "verification_failed",
            Attention,
            format!("{pr} failed verification"),
        ),
        A::Refreshed => ("refreshed", Info, format!("refreshed {pr} onto master")),
        A::RefreshFailed => (
            "refresh_failed",
            Attention,
            format!("could not refresh {pr}"),
        ),
        A::ClosedRedundant => (
            "closed",
            Info,
            format!("closed {pr}: its work is on master"),
        ),
        A::ClosedSuperseded => ("closed", Info, format!("closed {pr}: superseded")),
        A::CloseFailed => ("close_failed", Attention, format!("could not close {pr}")),
        A::Repaired => ("repaired", Info, format!("repaired {pr}")),
        A::RepairRefused => (
            "repair_refused",
            Attention,
            format!("refused to repair {pr}"),
        ),
        A::LeaseAcquired => ("lane_taken", Info, "took the integration lane".to_string()),
        A::LeaseReleased => (
            "lane_released",
            Info,
            "released the integration lane".to_string(),
        ),
        A::ObserveFailed => (
            "observe_failed",
            Attention,
            "could not observe the forge".to_string(),
        ),
        _ => return None,
    };
    let actor = event
        .actor
        .split(" (")
        .next()
        .unwrap_or(&event.actor)
        .to_string();
    let detail = if event.detail.is_empty() {
        String::new()
    } else {
        format!(": {}", event.detail)
    };
    let mut s = MonitorSignal::new(
        at,
        MonitorSource::Executor,
        kind,
        severity,
        &format!("executor {actor} {text}{detail}"),
    );
    s.pr = event.pr;
    s.for_you = reader.named_in(&event.detail) || reader.named_in(&event.actor);
    s.actor = Some(actor);
    if let Some(n) = event.pr {
        s.link = Some(format!("/cockpit/integration#pr-{n}"));
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(at: &str, action: &str, pr: Option<u64>, detail: &str) -> IntegrationEvent {
        serde_json::from_value(json!({
            "at": at, "actor": "Tomas Korcak (pid 7 on lundra)", "action": action,
            "pr": pr, "master_before": null, "head_sha": null, "master_after": null,
            "reasons": [], "detail": detail,
        }))
        .expect("an integration event")
    }

    #[test]
    fn outcomes_are_signals_and_steps_are_suppressed() {
        let trail = vec![
            event("2026-10-10T20:00:00Z", "observed", None, ""),
            event("2026-10-10T20:00:01Z", "merge_attempted", Some(868), ""),
            event(
                "2026-10-10T20:00:02Z",
                "merge_succeeded",
                Some(868),
                "pinned to its own run",
            ),
            event(
                "2026-10-10T20:00:03Z",
                "refresh_failed",
                Some(885),
                "conflict in serve.rs; ask 1b",
            ),
        ];
        let read = fold(&trail, None, 0, &Reader::named(Some("1b")));
        let kinds: Vec<&str> = read.signals.iter().map(|(_, s)| s.kind.as_str()).collect();
        assert_eq!(kinds, vec!["merged", "refresh_failed"]);
        assert_eq!(read.suppressed, 2);
        assert_eq!(read.high, Some(4));
        let merged = &read.signals[0].1;
        assert_eq!(
            merged.summary,
            "executor Tomas Korcak merged #868: pinned to its own run"
        );
        assert_eq!(merged.pr, Some(868));
        assert_eq!(merged.link.as_deref(), Some("/cockpit/integration#pr-868"));
        assert!(!merged.for_you);
        let failed = &read.signals[1].1;
        assert_eq!(failed.severity, MonitorSeverity::Attention);
        assert!(failed.for_you, "the detail names the reader");
    }

    #[test]
    fn a_position_resumes_and_a_rewritten_trail_reads_nothing_new() {
        let trail = vec![
            event("2026-10-10T20:00:00Z", "lease_acquired", None, ""),
            event("2026-10-10T20:00:01Z", "lease_released", None, ""),
        ];
        let reader = Reader::default();
        let read = fold(&trail, Some(1), 0, &reader);
        assert_eq!(read.signals.len(), 1);
        assert_eq!(read.signals[0].1.kind, "lane_released");
        let past = fold(&trail, Some(9), 0, &reader);
        assert!(past.signals.is_empty());
        assert_eq!(past.high, Some(2));
        let none = fold(&trail, Some(2), 0, &reader);
        assert_eq!(none.high, Some(2));
    }

    #[test]
    fn a_fresh_read_shows_only_the_window() {
        let trail = vec![
            event("2026-10-10T20:00:00Z", "merge_succeeded", Some(1), ""),
            event("2026-10-10T21:00:00Z", "merge_succeeded", Some(2), ""),
        ];
        let since = parse_rfc3339("2026-10-10T20:30:00Z").unwrap();
        let read = fold(&trail, None, since, &Reader::default());
        assert_eq!(read.signals.len(), 1);
        assert_eq!(read.signals[0].1.pr, Some(2));
        assert_eq!(read.high, Some(2));
    }
}
