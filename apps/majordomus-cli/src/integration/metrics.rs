//! How fast the integrator turns work into master, folded from its trail.
//!
//! Like [`super::wait`], everything here is a fold over the audit trail
//! ([`super::drain::events`]); nothing is stored beside it, so the numbers cannot drift from
//! the record they describe. A window bounds the fold: only events at or after `now - window`
//! count, so a quiet week reads as a quiet week rather than as the average of the whole
//! history.
//!
//! Five facts are folded:
//!
//! - **merges** in the window, and merges per day;
//! - **actionable → merged**: per merged pull request, from the last time it became actionable
//!   to its merge, as a median — how long ready work waits for the executor;
//! - **CI rounds per merge**: the `refreshed` events since the previous merge, per merge, as a
//!   median. Every refresh pushes a new head that the required checks must run on again, and
//!   that is where the time between merges goes;
//! - **cycle**: from a selection to its outcome (merged, failed, refused as stale), as a
//!   median;
//! - **failures** by kind: stale decisions, failed merges, failed verifications.
//!
//! A median with nothing to take the median of is `None`, never zero: zero would claim that
//! work moved instantly.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::drain::IntegrationEvent;
use crate::peers::parse_rfc3339;

/// The integrator's throughput over a window of the trail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationThroughput {
    /// The window, in days, ending at the time the fold was asked for.
    pub window_days: u64,
    /// Pull requests merged in the window.
    pub merges: u64,
    /// `merges / window_days`.
    pub merges_per_day: f64,
    /// Median seconds from becoming actionable to being merged; `None` when no merge in the
    /// window had a recorded `became_actionable` before it.
    pub median_actionable_to_merged_secs: Option<u64>,
    /// Median `refreshed` events between one merge and the next; `None` without a merge.
    pub ci_rounds_per_merge: Option<u64>,
    /// Median seconds from a selection to its outcome; `None` when no selection in the window
    /// reached one.
    pub median_cycle_secs: Option<u64>,
    /// Decisions refused because master or the head moved after they were taken.
    pub stale_decisions: u64,
    /// Merge attempts that failed.
    pub merge_failures: u64,
    /// Merges whose resulting master failed verification.
    pub verification_failures: u64,
    /// Events in the trail whose time could not be read; they count toward nothing.
    pub unreadable_events: u64,
}

/// What the fold needs to know about one event. The trail's own action words are mapped here
/// and nowhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    BecameActionable,
    Selected,
    Refreshed,
    MergeSucceeded,
    MergeFailed,
    VerificationFailed,
    StaleDecision,
    Other,
}

fn kind_of(e: &IntegrationEvent) -> Kind {
    use super::drain::IntegrationAction as A;
    match e.action {
        A::BecameActionable => Kind::BecameActionable,
        A::Selected | A::RefreshSelected => Kind::Selected,
        // a repaired head is a CI round exactly like a refreshed one
        A::Refreshed | A::Repaired => Kind::Refreshed,
        A::MergeSucceeded => Kind::MergeSucceeded,
        A::MergeFailed => Kind::MergeFailed,
        A::VerificationFailed => Kind::VerificationFailed,
        A::StaleDecision => Kind::StaleDecision,
        _ => Kind::Other,
    }
}

/// The median of `v`, lower of the two middle values for an even count, so it is always a
/// value that was observed. `None` for an empty slice.
fn median(mut v: Vec<u64>) -> Option<u64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[(v.len() - 1) / 2])
}

/// Fold `trail` (oldest first, as [`super::drain::events`] returns it) over the `window_days`
/// ending at `now` (seconds since the epoch). Deterministic for a given trail and `now`; the
/// unit tests below hold each fact.
pub fn throughput(trail: &[IntegrationEvent], now: u64, window_days: u64) -> IntegrationThroughput {
    let since = now.saturating_sub(window_days.saturating_mul(86_400));
    let mut out = IntegrationThroughput {
        window_days,
        merges: 0,
        merges_per_day: 0.0,
        median_actionable_to_merged_secs: None,
        ci_rounds_per_merge: None,
        median_cycle_secs: None,
        stale_decisions: 0,
        merge_failures: 0,
        verification_failures: 0,
        unreadable_events: 0,
    };
    // Facts that open before the window still close inside it: a pull request that became
    // actionable last week and merged today waited across the boundary, so state is folded
    // over the whole trail and only outcomes are filtered by the window.
    let mut actionable_since: BTreeMap<u64, u64> = BTreeMap::new();
    let mut selected_at: BTreeMap<u64, u64> = BTreeMap::new();
    let mut refreshes_since_merge: u64 = 0;
    let (mut waits, mut rounds, mut cycles) = (Vec::new(), Vec::new(), Vec::new());

    for e in trail {
        let Some(at) = parse_rfc3339(&e.at) else {
            out.unreadable_events += 1;
            continue;
        };
        let inside = at >= since && at <= now;
        let kind = kind_of(e);
        match (kind, e.pr) {
            (Kind::BecameActionable, Some(n)) => {
                actionable_since.insert(n, at);
            }
            (Kind::Selected, Some(n)) => {
                selected_at.insert(n, at);
            }
            (Kind::Refreshed, _) => refreshes_since_merge += 1,
            (Kind::MergeSucceeded, pr) => {
                if inside {
                    out.merges += 1;
                    rounds.push(refreshes_since_merge);
                    if let Some(from) = pr.and_then(|n| actionable_since.get(&n)) {
                        waits.push(at.saturating_sub(*from));
                    }
                }
                refreshes_since_merge = 0;
            }
            _ => {}
        }
        // a selection's outcome closes its cycle
        if matches!(
            kind,
            Kind::MergeSucceeded
                | Kind::MergeFailed
                | Kind::VerificationFailed
                | Kind::StaleDecision
        ) {
            if let Some(n) = e.pr {
                if let Some(from) = selected_at.remove(&n) {
                    if inside {
                        cycles.push(at.saturating_sub(from));
                    }
                }
                if kind == Kind::MergeSucceeded {
                    actionable_since.remove(&n);
                }
            }
            if inside {
                match kind {
                    Kind::MergeFailed => out.merge_failures += 1,
                    Kind::VerificationFailed => out.verification_failures += 1,
                    Kind::StaleDecision => out.stale_decisions += 1,
                    _ => {}
                }
            }
        }
    }

    if window_days > 0 {
        out.merges_per_day = out.merges as f64 / window_days as f64;
    }
    out.median_actionable_to_merged_secs = median(waits);
    out.ci_rounds_per_merge = median(rounds);
    out.median_cycle_secs = median(cycles);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::rfc3339;
    use std::time::{Duration, UNIX_EPOCH};

    const T0: u64 = 1_790_000_000;
    const DAY: u64 = 86_400;

    fn ev(at: u64, action: &str, pr: Option<u64>) -> IntegrationEvent {
        // the trail's wire word, read the way a trail line is read
        let action = serde_json::from_value(serde_json::Value::String(action.into()))
            .unwrap_or_else(|e| panic!("{action} is not an integration action: {e}"));
        IntegrationEvent {
            at: rfc3339(UNIX_EPOCH + Duration::from_secs(at)),
            actor: "test".into(),
            pr,
            ..IntegrationEvent::of(action)
        }
    }

    #[test]
    fn an_empty_trail_moved_nothing_and_claims_no_speed() {
        let t = throughput(&[], T0, 7);
        assert_eq!(t.merges, 0);
        assert_eq!(t.merges_per_day, 0.0);
        assert_eq!(t.median_actionable_to_merged_secs, None);
        assert_eq!(t.ci_rounds_per_merge, None);
        assert_eq!(t.median_cycle_secs, None);
    }

    #[test]
    fn a_merge_is_timed_from_becoming_actionable_and_from_its_selection() {
        let trail = [
            ev(T0, "became_actionable", Some(1)),
            ev(T0 + 600, "selected", Some(1)),
            ev(T0 + 660, "merge_attempted", Some(1)),
            ev(T0 + 900, "merge_succeeded", Some(1)),
        ];
        let t = throughput(&trail, T0 + DAY, 7);
        assert_eq!(t.merges, 1);
        assert_eq!(t.median_actionable_to_merged_secs, Some(900));
        assert_eq!(t.median_cycle_secs, Some(300));
        assert_eq!(t.ci_rounds_per_merge, Some(0));
    }

    #[test]
    fn a_repaired_head_is_a_ci_round_like_a_refreshed_one() {
        let trail = [
            ev(T0, "repaired", Some(1)),
            ev(T0 + 10, "refreshed", Some(1)),
            ev(T0 + 20, "merge_succeeded", Some(1)),
        ];
        let t = throughput(&trail, T0 + DAY, 7);
        assert_eq!(t.merges, 1);
        assert_eq!(t.ci_rounds_per_merge, Some(2));
    }

    #[test]
    fn ci_rounds_are_the_refreshes_between_one_merge_and_the_next() {
        let trail = [
            ev(T0, "refreshed", Some(2)),
            ev(T0 + 10, "refreshed", Some(2)),
            ev(T0 + 20, "merge_succeeded", Some(1)),
            ev(T0 + 30, "refreshed", Some(2)),
            ev(T0 + 40, "merge_succeeded", Some(2)),
            ev(T0 + 50, "merge_succeeded", Some(3)),
        ];
        let t = throughput(&trail, T0 + DAY, 7);
        assert_eq!(t.merges, 3);
        // rounds per merge: 2, 1, 0 -> median 1
        assert_eq!(t.ci_rounds_per_merge, Some(1));
    }

    #[test]
    fn the_median_is_an_observed_value() {
        assert_eq!(median(vec![]), None);
        assert_eq!(median(vec![5]), Some(5));
        assert_eq!(median(vec![9, 1, 5]), Some(5));
        // even count: the lower middle, a value that happened
        assert_eq!(median(vec![1, 2, 3, 100]), Some(2));
    }

    #[test]
    fn only_outcomes_inside_the_window_count_but_waits_may_start_before_it() {
        let trail = [
            ev(T0, "became_actionable", Some(7)),
            ev(T0 + 10, "merge_succeeded", Some(6)),
            ev(T0 + 10 * DAY, "merge_succeeded", Some(7)),
        ];
        let t = throughput(&trail, T0 + 10 * DAY, 7);
        assert_eq!(
            t.merges, 1,
            "the merge ten days ago is outside a 7-day window"
        );
        assert_eq!(t.median_actionable_to_merged_secs, Some(10 * DAY));
        assert!((t.merges_per_day - 1.0 / 7.0).abs() < 1e-9);
    }

    #[test]
    fn failures_are_counted_by_kind_and_close_their_cycle() {
        let trail = [
            ev(T0, "selected", Some(1)),
            ev(T0 + 5, "stale_decision", Some(1)),
            ev(T0 + 10, "selected", Some(2)),
            ev(T0 + 70, "merge_failed", Some(2)),
            ev(T0 + 80, "verification_failed", Some(3)),
        ];
        let t = throughput(&trail, T0 + DAY, 7);
        assert_eq!(
            (t.stale_decisions, t.merge_failures, t.verification_failures),
            (1, 1, 1)
        );
        assert_eq!(t.merges, 0);
        // cycles 5 and 60 -> lower middle 5
        assert_eq!(t.median_cycle_secs, Some(5));
    }

    #[test]
    fn an_unreadable_time_counts_toward_nothing_and_is_reported() {
        let mut bad = ev(T0, "merge_succeeded", Some(1));
        bad.at = "yesterday".into();
        let t = throughput(&[bad], T0 + DAY, 7);
        assert_eq!(t.merges, 0);
        assert_eq!(t.unreadable_events, 1);
    }

    #[test]
    fn the_fold_is_deterministic() {
        let trail = [
            ev(T0, "became_actionable", Some(1)),
            ev(T0 + 3, "refreshed", Some(1)),
            ev(T0 + 9, "merge_succeeded", Some(1)),
        ];
        assert_eq!(
            throughput(&trail, T0 + DAY, 7),
            throughput(&trail, T0 + DAY, 7)
        );
    }
}

#[cfg(test)]
mod window_branches {
    //! Outcomes outside the window, outcomes without a pull request, and a window of no days.

    use super::*;
    use crate::integration::drain::{IntegrationAction, IntegrationEvent};

    fn at(action: IntegrationAction, pr: Option<u64>, when: &str) -> IntegrationEvent {
        IntegrationEvent {
            at: when.into(),
            pr,
            ..IntegrationEvent::of(action)
        }
    }

    #[test]
    fn a_cycle_closed_before_the_window_counts_nothing_and_no_days_divide_nothing() {
        let trail = vec![
            at(IntegrationAction::Selected, Some(1), "2026-01-01T00:00:00Z"),
            at(
                IntegrationAction::MergeFailed,
                Some(1),
                "2026-01-01T00:01:00Z",
            ),
            at(IntegrationAction::MergeFailed, None, "2026-01-01T00:02:00Z"),
        ];
        let now = parse_rfc3339("2026-10-01T00:00:00Z").unwrap();
        let t = throughput(&trail, now, 7);
        assert_eq!(t.merge_failures, 0, "outside the window");
        let t = throughput(&trail, now, 0);
        assert_eq!(t.merges_per_day, 0.0);
    }
}
