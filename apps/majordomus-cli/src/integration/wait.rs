//! How long a pull request has waited for the executor, and how often another was chosen
//! instead: starvation, made visible.
//!
//! Everything here is a fold over the audit trail ([`super::drain::events`]); nothing is
//! stored beside it. The executor records two facts the fold needs: when a pull request
//! *becomes* actionable (ready, or refreshable — the two dispositions the executor acts on)
//! and when it stops being so, both as transitions between one executor step's observation
//! and the trail's last word; and, on every selection, the other actionable pull requests it
//! passed over. A dry run records neither, so a dry run still leaves no trace.
//!
//! The rank is not changed by any of it. Its age tie-break already prefers the older of two
//! otherwise equal candidates; a long wait is a fact for a person ([`STARVING_AFTER`]), and
//! never a reason to merge something less safe sooner.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::drain::{events, record, IntegrationAction, IntegrationEvent};
use super::{ExecutorWait, IntegrationQueue, PassedOver, PullRequestDisposition};

/// A pull request passed over this many times while actionable is called starving.
pub const STARVING_AFTER: u32 = 3;

/// The trail's word for a pull request that became actionable.
pub const BECAME_ACTIONABLE: IntegrationAction = IntegrationAction::BecameActionable;
/// The trail's word for one that stopped being actionable.
pub const LEFT_ACTIONABLE: IntegrationAction = IntegrationAction::LeftActionable;

/// Whether the executor acts on a disposition: it merges a ready one and brings master into
/// a refreshable one.
pub fn actionable(d: PullRequestDisposition) -> bool {
    matches!(
        d,
        PullRequestDisposition::Ready | PullRequestDisposition::NeedsRefresh
    )
}

/// Every pull request the trail says is actionable now, with how long and how often passed
/// over. A fold, oldest event first; deterministic for a given trail.
pub fn waits(trail: &[IntegrationEvent]) -> BTreeMap<u64, ExecutorWait> {
    let mut out: BTreeMap<u64, ExecutorWait> = BTreeMap::new();
    for e in trail {
        match (e.action, e.pr) {
            (BECAME_ACTIONABLE, Some(n)) => {
                out.insert(
                    n,
                    ExecutorWait {
                        actionable_since: e.at.clone(),
                        passed_over: 0,
                        last_passed_over: None,
                    },
                );
            }
            (
                LEFT_ACTIONABLE
                | IntegrationAction::MergeSucceeded
                | IntegrationAction::ClosedSuperseded,
                Some(n),
            ) => {
                out.remove(&n);
            }
            (IntegrationAction::Selected | IntegrationAction::RefreshSelected, Some(chosen)) => {
                for n in &e.passed_over {
                    if let Some(w) = out.get_mut(n) {
                        w.passed_over += 1;
                        w.last_passed_over = Some(PassedOver {
                            at: e.at.clone(),
                            for_pr: chosen,
                            action: e.action,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Record the transitions between what the trail says is actionable and what `queue` says
/// is: one `became_actionable` per pull request that newly is, one `left_actionable` per
/// pull request that no longer is, with the disposition it has now (or that it is no longer
/// open). Nothing is recorded when nothing changed. Returns how many were recorded, or why
/// the trail could not take one.
pub fn record_transitions(root: &Path, queue: &IntegrationQueue) -> Result<usize, String> {
    let before: BTreeSet<u64> = waits(&events(root)).into_keys().collect();
    let now: BTreeSet<u64> = queue
        .assessments
        .iter()
        .filter(|a| actionable(a.disposition))
        .map(|a| a.number)
        .collect();
    let mut n = 0;
    for pr in now.difference(&before) {
        let a = queue.get(*pr);
        record(
            root,
            IntegrationEvent {
                pr: Some(*pr),
                master_before: a.map(|a| a.evaluated_against.master_sha.clone()),
                head_sha: a.map(|a| a.evaluated_against.head_sha.clone()),
                reasons: a.map(|a| a.reasons.clone()).unwrap_or_default(),
                detail: a
                    .map(|a| a.disposition.as_str().to_string())
                    .unwrap_or_default(),
                ..IntegrationEvent::of(BECAME_ACTIONABLE)
            },
        )?;
        n += 1;
    }
    for pr in before.difference(&now) {
        let detail = match queue.get(*pr) {
            Some(a) => format!("it is {} now", a.disposition.as_str()),
            None => "it is no longer open".to_string(),
        };
        record(
            root,
            IntegrationEvent {
                pr: Some(*pr),
                master_before: Some(queue.master_sha.clone()),
                head_sha: queue.get(*pr).map(|a| a.evaluated_against.head_sha.clone()),
                detail,
                ..IntegrationEvent::of(LEFT_ACTIONABLE)
            },
        )?;
        n += 1;
    }
    Ok(n)
}

/// Put the trail's waits on the queue: each actionable assessment gets its wait, and the
/// queue names the starving ones, in rank order. A pull request the trail calls actionable
/// but this queue does not — master moved since the executor last looked — gets none.
pub fn annotate(queue: &mut IntegrationQueue, trail: &[IntegrationEvent]) {
    let waits = waits(trail);
    for a in &mut queue.assessments {
        a.wait = if actionable(a.disposition) {
            waits.get(&a.number).cloned()
        } else {
            None
        };
    }
    queue.starving = queue
        .assessments
        .iter()
        .filter(|a| {
            a.wait
                .as_ref()
                .is_some_and(|w| w.passed_over >= STARVING_AFTER)
        })
        .map(|a| a.number)
        .collect();
}
