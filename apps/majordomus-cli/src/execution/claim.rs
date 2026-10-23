//! The mesh claim an execution started for an issue holds while it runs (I4004).
//!
//! Two workers that start the same issue at once must not both run it. The mesh already
//! decides that question for sessions — an exclusive claim that meets a live exclusive claim
//! of another session is refused, on this runtime and on every runtime it has heard — so an
//! execution started for an issue asks it the same way a person's session does: through
//! [`Cooperation::claim`], the function `mesh.claim` calls, as the mesh session
//! `execution-<id>`, for the issue's declared scope.
//!
//! ```text
//!   queued ──► take ──► held ──────► claimed ──► running ──► final ──► released
//!                  │                                     └─► unknown_outcome ──► kept
//!                  ├─► conflict ──► refused  (the holder named; nothing runs)
//!                  └─► no mesh ───► unclaimed ──► claimed ──► running ...   (said, never a failure)
//! ```
//!
//! The claim is given back when the execution reaches a final state, by the worker that ran
//! it, and — through [`HeldClaim`]'s `Drop` — when that worker unwinds instead. An outcome
//! that is not known keeps the claim: whether the work happened is still to be read, and the
//! scope is not free until it has been. A kept claim, or one whose process died, ends when
//! its runtime stops beating, as every mesh claim does. Fencing a holder that comes back
//! after its claim expired is not done here (I3174).
//!
//! ```
//! use majordomus_cli::execution::claim::{disposition, scope_of, session_of};
//! use majordomus_cli::execution::{ClaimStanding, ExecutionId, ExecutionState};
//! let repo = majordomus_cli::synthetic::SyntheticRepository::small().unwrap();
//! let ctx = repo.context().unwrap();
//! let id = ExecutionId::fresh();
//! assert_eq!(session_of(&id), format!("execution-{id}"));
//! assert_eq!(scope_of(&ctx, "I4004"), ["issue/I4004"]);
//! assert_eq!(disposition(ExecutionState::Cancelled), ClaimStanding::Released);
//! ```

use std::sync::Arc;

use crate::capability::Context;
use crate::mesh::cooperation::{Cooperation, CooperationError};
use crate::mesh::journal::{ClaimMode, SessionInfo};

use super::model::{ClaimStanding, ExecutionId, ExecutionState, IssueClaim};

/// The client name an execution's mesh session is opened with.
pub const CLIENT: &str = "majordomus-execution";

/// The mesh session an execution claims as: `execution-<id>`, so that a refusal that names
/// the session names the execution that holds the scope.
///
/// ```
/// use majordomus_cli::execution::{claim::session_of, ExecutionId};
/// let id = ExecutionId::parse("x-20261023T010203Z-0a1b2c3d").unwrap();
/// assert_eq!(session_of(&id), "execution-x-20261023T010203Z-0a1b2c3d");
/// ```
pub fn session_of(id: &ExecutionId) -> String {
    format!("execution-{id}")
}

/// The scope an issue's claim covers: the paths its record declares under `scope`, or the
/// issue's pseudo-path `issue/<id>` when the record declares none or is not in this checkout.
///
/// ```
/// use majordomus_cli::execution::claim::scope_of;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// assert_eq!(scope_of(&ctx, "I9999"), ["issue/I9999"], "no such record here");
/// ```
pub fn scope_of(ctx: &Context, issue: &str) -> Vec<String> {
    let declared: Vec<String> = crate::plan::Plan::build(&ctx.index)
        .issue(issue)
        .map(|i| i.scope.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect();
    if declared.is_empty() {
        vec![format!("issue/{issue}")]
    } else {
        declared
    }
}

/// What the claim does when the execution reaches `state`: kept for an outcome that is not
/// known, released for anything else — a final state, or a worker that is leaving.
///
/// ```
/// use majordomus_cli::execution::claim::disposition;
/// use majordomus_cli::execution::{ClaimStanding, ExecutionState};
/// assert_eq!(disposition(ExecutionState::Succeeded), ClaimStanding::Released);
/// assert_eq!(disposition(ExecutionState::Refused), ClaimStanding::Released);
/// assert_eq!(disposition(ExecutionState::UnknownOutcome), ClaimStanding::Kept);
/// ```
pub fn disposition(state: ExecutionState) -> ClaimStanding {
    if state == ExecutionState::UnknownOutcome {
        ClaimStanding::Kept
    } else {
        ClaimStanding::Released
    }
}

/// How the attempt to take an issue's claim came out: held, unclaimed because no mesh runs,
/// or refused because another session holds the scope.
///
/// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
/// # use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
/// # use majordomus_cli::mesh::identity::NodeIdentity;
/// # use majordomus_cli::mesh::link::HttpTransport;
/// # use majordomus_cli::mesh::registry::MeshRegistry;
/// # use majordomus_cli::mesh::repository::of_root_commits;
/// # let cooperation = Cooperation::new(CooperationSetup {
/// #     identity: Arc::new(NodeIdentity::ephemeral().unwrap()), runtime: "0000000000000001".into(),
/// #     repository: of_root_commits(&["root".into()]), endpoints: vec!["127.0.0.1:9".into()],
/// #     version: "doc".into(), config: CooperationConfig::default(), trust: TrustConfig::default(),
/// #     journal_path: None, registry: Arc::new(MeshRegistry::new()),
/// #     transport: Arc::new(HttpTransport), board: None, checkout: CheckoutFacts::default(),
/// # }).unwrap();
/// # let repo = majordomus_cli::synthetic::SyntheticRepository::small().unwrap();
/// # let ctx = repo.context().unwrap();
/// // a context whose mesh runs
/// ctx.mesh.attach_cooperation(Arc::clone(&cooperation));
/// use majordomus_cli::execution::claim::{take, Taken};
/// use majordomus_cli::execution::ExecutionId;
/// let first = take(&ctx, &ExecutionId::fresh(), "one", "I4004");
/// assert!(matches!(first, Taken::Held(_)));
/// match take(&ctx, &ExecutionId::fresh(), "two", "I4004") {
///     Taken::Refused { conflict, message, .. } => {
///         assert!(conflict && message.starts_with("claim_conflict: I4004"), "{message}");
///     }
///     other => panic!("{other:?}"),
/// }
/// ```
#[derive(Debug)]
pub enum Taken {
    /// The exclusive claim is held; it goes back when the guard is settled or dropped.
    Held(HeldClaim),
    /// No mesh cooperation runs in this process: the execution runs without a claim, and
    /// the claim says why.
    Unclaimed(IssueClaim),
    /// The claim was refused: nothing may run.
    Refused {
        /// The claim, standing `refused`, with the holder or the reason in its detail.
        claim: IssueClaim,
        /// `true` when another session holds the scope (`claim_conflict`); `false` when the
        /// mesh refused the claim for another reason.
        conflict: bool,
        /// The sentence for the execution's error.
        message: String,
    },
}

/// Take the exclusive mesh claim of `issue` for the execution `id`, before it runs.
///
/// ```
/// use majordomus_cli::execution::claim::{take, Taken};
/// use majordomus_cli::execution::{ClaimStanding, ExecutionId};
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// // no mesh cooperation runs in this context: it runs unclaimed, and says so
/// match take(&ctx, &ExecutionId::fresh(), "Demonstrate", "I4004") {
///     Taken::Unclaimed(claim) => {
///         assert_eq!(claim.standing, ClaimStanding::Unclaimed);
///         assert!(claim.detail.is_some());
///     }
///     other => panic!("{other:?}"),
/// }
/// ```
pub fn take(ctx: &Context, id: &ExecutionId, intent: &str, issue: &str) -> Taken {
    let scope = scope_of(ctx, issue);
    let Some(cooperation) = ctx.mesh.cooperation() else {
        let reason = ctx
            .mesh
            .cooperation_status()
            .reason
            .unwrap_or_else(|| "mesh cooperation is not running in this process".into());
        return Taken::Unclaimed(IssueClaim {
            standing: ClaimStanding::Unclaimed,
            scope,
            detail: Some(format!(
                "ran without a mesh claim: {reason}; nothing on the mesh kept a second start of {issue} out"
            )),
            ..IssueClaim::wanted(issue)
        });
    };
    take_with(cooperation, id, intent, issue, scope)
}

fn take_with(
    cooperation: Arc<Cooperation>,
    id: &ExecutionId,
    intent: &str,
    issue: &str,
    scope: Vec<String>,
) -> Taken {
    let session = session_of(id);
    let info = SessionInfo {
        session: session.clone(),
        client: CLIENT.into(),
        issue: Some(issue.to_string()),
        intent: Some(intent.to_string()),
        ..SessionInfo::default()
    };
    match cooperation.claim(
        &info,
        scope.clone(),
        Some(intent.to_string()),
        ClaimMode::Exclusive,
        Some(issue.to_string()),
    ) {
        Ok(written) => Taken::Held(HeldClaim {
            cooperation,
            claim: IssueClaim {
                standing: ClaimStanding::Held,
                scope,
                key: Some(written.key),
                session: Some(session),
                ..IssueClaim::wanted(issue)
            },
            armed: true,
        }),
        Err(CooperationError::Conflict(holders)) => {
            let named = holders
                .iter()
                .map(|h| {
                    format!(
                        "session {} holds {} on {}{}",
                        h.session,
                        h.key,
                        h.scope.join(", "),
                        h.issue
                            .as_deref()
                            .map(|i| format!(" for {i}"))
                            .unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            let message =
                format!("claim_conflict: {issue} is claimed on the mesh: {named}; nothing ran");
            Taken::Refused {
                claim: IssueClaim {
                    standing: ClaimStanding::Refused,
                    scope,
                    detail: Some(named),
                    ..IssueClaim::wanted(issue)
                },
                conflict: true,
                message,
            }
        }
        Err(other) => {
            let message =
                format!("the mesh claim of {issue} could not be taken: {other}; nothing ran");
            Taken::Refused {
                claim: IssueClaim {
                    standing: ClaimStanding::Refused,
                    scope,
                    detail: Some(other.to_string()),
                    ..IssueClaim::wanted(issue)
                },
                conflict: false,
                message,
            }
        }
    }
}

/// An exclusive mesh claim an execution holds. Settled when the execution ends; released by
/// `Drop` when the worker unwinds without settling it, so a panic does not leave the scope
/// claimed for as long as the runtime beats.
///
/// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
/// # use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
/// # use majordomus_cli::mesh::identity::NodeIdentity;
/// # use majordomus_cli::mesh::link::HttpTransport;
/// # use majordomus_cli::mesh::registry::MeshRegistry;
/// # use majordomus_cli::mesh::repository::of_root_commits;
/// # let cooperation = Cooperation::new(CooperationSetup {
/// #     identity: Arc::new(NodeIdentity::ephemeral().unwrap()), runtime: "0000000000000001".into(),
/// #     repository: of_root_commits(&["root".into()]), endpoints: vec!["127.0.0.1:9".into()],
/// #     version: "doc".into(), config: CooperationConfig::default(), trust: TrustConfig::default(),
/// #     journal_path: None, registry: Arc::new(MeshRegistry::new()),
/// #     transport: Arc::new(HttpTransport), board: None, checkout: CheckoutFacts::default(),
/// # }).unwrap();
/// # let repo = majordomus_cli::synthetic::SyntheticRepository::small().unwrap();
/// # let ctx = repo.context().unwrap();
/// // a context whose mesh runs
/// ctx.mesh.attach_cooperation(Arc::clone(&cooperation));
/// use majordomus_cli::execution::claim::{take, HeldClaim, Taken};
/// use majordomus_cli::execution::{ClaimStanding, ExecutionId, ExecutionState};
/// let Taken::Held(held) = take(&ctx, &ExecutionId::fresh(), "x", "I4004") else { panic!() };
/// let held: HeldClaim = held;
/// assert_eq!(held.claim().standing, ClaimStanding::Held);
/// assert!(held.claim().key.is_some());
/// let released = held.settle(Some(ExecutionState::Succeeded));
/// assert_eq!(released.standing, ClaimStanding::Released);
/// // the scope is free again
/// assert!(matches!(take(&ctx, &ExecutionId::fresh(), "y", "I4004"), Taken::Held(_)));
/// ```
pub struct HeldClaim {
    cooperation: Arc<Cooperation>,
    claim: IssueClaim,
    armed: bool,
}

impl std::fmt::Debug for HeldClaim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeldClaim")
            .field("claim", &self.claim)
            .field("armed", &self.armed)
            .finish()
    }
}

impl HeldClaim {
    /// The claim as held: its key, its session and its scope, which the execution's
    /// `execution.issue_claim` event and the `claimed` holder are made from.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
    /// # use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
    /// # use majordomus_cli::mesh::identity::NodeIdentity;
    /// # use majordomus_cli::mesh::link::HttpTransport;
    /// # use majordomus_cli::mesh::registry::MeshRegistry;
    /// # use majordomus_cli::mesh::repository::of_root_commits;
    /// # let cooperation = Cooperation::new(CooperationSetup {
    /// #     identity: Arc::new(NodeIdentity::ephemeral().unwrap()), runtime: "0000000000000001".into(),
    /// #     repository: of_root_commits(&["root".into()]), endpoints: vec!["127.0.0.1:9".into()],
    /// #     version: "doc".into(), config: CooperationConfig::default(), trust: TrustConfig::default(),
    /// #     journal_path: None, registry: Arc::new(MeshRegistry::new()),
    /// #     transport: Arc::new(HttpTransport), board: None, checkout: CheckoutFacts::default(),
    /// # }).unwrap();
    /// # let repo = majordomus_cli::synthetic::SyntheticRepository::small().unwrap();
    /// # let ctx = repo.context().unwrap();
    /// // a context whose mesh runs
    /// ctx.mesh.attach_cooperation(Arc::clone(&cooperation));
    /// use majordomus_cli::execution::claim::{take, Taken};
    /// use majordomus_cli::execution::{ClaimStanding, ExecutionId, ExecutionState};
    /// let Taken::Held(held) = take(&ctx, &ExecutionId::fresh(), "x", "I4004") else { panic!() };
    /// assert_eq!(held.claim().standing, ClaimStanding::Held);
    /// assert!(held.claim().key.is_some());
    /// let released = held.settle(Some(ExecutionState::Succeeded));
    /// assert_eq!(released.standing, ClaimStanding::Released);
    /// // the scope is free again
    /// assert!(matches!(take(&ctx, &ExecutionId::fresh(), "y", "I4004"), Taken::Held(_)));
    /// ```
    pub fn claim(&self) -> &IssueClaim {
        &self.claim
    }

    /// Settle the claim for an execution that reached `state` (`None`: the record is gone):
    /// released, with its session closed, or kept for an outcome that is not known. The
    /// answer is the claim as it now stands, for the execution's event.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
    /// # use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
    /// # use majordomus_cli::mesh::identity::NodeIdentity;
    /// # use majordomus_cli::mesh::link::HttpTransport;
    /// # use majordomus_cli::mesh::registry::MeshRegistry;
    /// # use majordomus_cli::mesh::repository::of_root_commits;
    /// # let cooperation = Cooperation::new(CooperationSetup {
    /// #     identity: Arc::new(NodeIdentity::ephemeral().unwrap()), runtime: "0000000000000001".into(),
    /// #     repository: of_root_commits(&["root".into()]), endpoints: vec!["127.0.0.1:9".into()],
    /// #     version: "doc".into(), config: CooperationConfig::default(), trust: TrustConfig::default(),
    /// #     journal_path: None, registry: Arc::new(MeshRegistry::new()),
    /// #     transport: Arc::new(HttpTransport), board: None, checkout: CheckoutFacts::default(),
    /// # }).unwrap();
    /// # let repo = majordomus_cli::synthetic::SyntheticRepository::small().unwrap();
    /// # let ctx = repo.context().unwrap();
    /// // a context whose mesh runs
    /// ctx.mesh.attach_cooperation(Arc::clone(&cooperation));
    /// use majordomus_cli::execution::claim::{take, Taken};
    /// use majordomus_cli::execution::{ClaimStanding, ExecutionId, ExecutionState};
    /// let Taken::Held(held) = take(&ctx, &ExecutionId::fresh(), "x", "I1") else { panic!() };
    /// let kept = held.settle(Some(ExecutionState::UnknownOutcome));
    /// assert_eq!(kept.standing, ClaimStanding::Kept, "the outcome is not known");
    /// // so a second start of the issue is still refused
    /// assert!(matches!(take(&ctx, &ExecutionId::fresh(), "y", "I1"), Taken::Refused { .. }));
    /// ```
    pub fn settle(mut self, state: Option<ExecutionState>) -> IssueClaim {
        match state.map(disposition) {
            Some(ClaimStanding::Kept) => {
                self.armed = false;
                IssueClaim {
                    standing: ClaimStanding::Kept,
                    detail: Some(
                        "the outcome is not known, so the claim is kept until a reconciliation reads what happened; it ends when this runtime stops beating"
                            .into(),
                    ),
                    ..self.claim.clone()
                }
            }
            _ => self.release_now(),
        }
    }

    fn release_now(&mut self) -> IssueClaim {
        self.armed = false;
        let mut problems = Vec::new();
        if let Some(key) = &self.claim.key {
            if let Err(e) = self.cooperation.release(key) {
                problems.push(format!("the release was refused: {e}"));
            }
        }
        if let Some(session) = &self.claim.session {
            if let Err(e) = self.cooperation.close_session(session) {
                problems.push(format!("the session did not close: {e}"));
            }
        }
        if !problems.is_empty() {
            tracing::warn!(claim = ?self.claim.key, "an execution's issue claim was not given back cleanly: {}", problems.join("; "));
        }
        IssueClaim {
            standing: ClaimStanding::Released,
            detail: (!problems.is_empty()).then(|| problems.join("; ")),
            ..self.claim.clone()
        }
    }
}

impl Drop for HeldClaim {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.release_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::state::ClaimState;

    fn live(c: &Cooperation, key: &str) -> bool {
        c.state()
            .claims
            .iter()
            .any(|claim| claim.key == key && claim.state == ClaimState::Held)
    }

    #[test]
    fn a_claim_is_held_exclusively_and_a_second_start_of_the_issue_is_refused_naming_it() {
        let c = crate::mesh::testing::cooperation();
        let first = ExecutionId::fresh();
        let scope = vec!["issue/I4004".to_string()];
        let Taken::Held(held) = take_with(Arc::clone(&c), &first, "one", "I4004", scope.clone())
        else {
            panic!("the first start is not held");
        };
        let key = held.claim().key.clone().unwrap();
        assert!(live(&c, &key));
        assert_eq!(held.claim().standing, ClaimStanding::Held);
        assert_eq!(
            held.claim().session.as_deref(),
            Some(session_of(&first).as_str())
        );
        let second = ExecutionId::fresh();
        match take_with(Arc::clone(&c), &second, "two", "I4004", scope.clone()) {
            Taken::Refused {
                claim,
                conflict,
                message,
            } => {
                assert!(conflict);
                assert_eq!(claim.standing, ClaimStanding::Refused);
                assert!(message.starts_with("claim_conflict: I4004"), "{message}");
                assert!(
                    message.contains(&key),
                    "the holder's claim is named: {message}"
                );
                assert!(
                    message.contains(&session_of(&first)),
                    "the holding execution is named: {message}"
                );
            }
            other => panic!("the second start was not refused: {other:?}"),
        }
        // the refused start opened no session of its own
        assert!(!c
            .state()
            .sessions
            .iter()
            .any(|s| s.info.session == session_of(&second)));
        drop(held.settle(Some(ExecutionState::Succeeded)));
        assert!(!live(&c, &key), "released at the final state");
        assert!(
            matches!(take_with(c, &second, "two", "I4004", scope), Taken::Held(_)),
            "free again once released"
        );
    }

    #[test]
    fn every_final_state_releases_the_claim_and_an_unknown_outcome_keeps_it() {
        for state in ExecutionState::ALL {
            let c = crate::mesh::testing::cooperation();
            let id = ExecutionId::fresh();
            let Taken::Held(held) =
                take_with(Arc::clone(&c), &id, "x", "I1", vec!["issue/I1".into()])
            else {
                panic!("not held");
            };
            let key = held.claim().key.clone().unwrap();
            let settled = held.settle(Some(state));
            if state == ExecutionState::UnknownOutcome {
                assert_eq!(settled.standing, ClaimStanding::Kept, "{state:?}");
                assert!(settled.detail.as_deref().unwrap().contains("not known"));
                assert!(live(&c, &key), "{state:?}: kept");
            } else {
                assert_eq!(settled.standing, ClaimStanding::Released, "{state:?}");
                assert_eq!(settled.detail, None, "{state:?}: released cleanly");
                assert!(!live(&c, &key), "{state:?}: released");
            }
        }
    }

    #[test]
    fn a_claim_dropped_unsettled_is_released_and_a_gone_record_releases_it() {
        let c = crate::mesh::testing::cooperation();
        let Taken::Held(held) = take_with(
            Arc::clone(&c),
            &ExecutionId::fresh(),
            "x",
            "I2",
            vec!["issue/I2".into()],
        ) else {
            panic!("not held");
        };
        let key = held.claim().key.clone().unwrap();
        // a worker that panics unwinds through the guard
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _held = held;
            panic!("the worker broke");
        }));
        assert!(unwound.is_err());
        assert!(!live(&c, &key), "released on the unwind");

        let Taken::Held(held) = take_with(
            Arc::clone(&c),
            &ExecutionId::fresh(),
            "x",
            "I2",
            vec!["issue/I2".into()],
        ) else {
            panic!("not held");
        };
        let key = held.claim().key.clone().unwrap();
        assert_eq!(held.settle(None).standing, ClaimStanding::Released);
        assert!(!live(&c, &key));
    }

    #[test]
    fn without_a_mesh_it_runs_unclaimed_and_the_scope_is_the_issue_pseudo_path() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        match take(&ctx, &ExecutionId::fresh(), "x", "I4004") {
            Taken::Unclaimed(claim) => {
                assert_eq!(claim.standing, ClaimStanding::Unclaimed);
                assert_eq!(claim.scope, ["issue/I4004"]);
                assert!(claim.key.is_none() && claim.session.is_none());
                assert!(claim
                    .detail
                    .unwrap()
                    .starts_with("ran without a mesh claim"));
            }
            other => panic!("{other:?}"),
        }
    }
}
