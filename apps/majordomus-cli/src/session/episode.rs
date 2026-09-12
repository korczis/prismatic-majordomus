//! The aggregate: what an episode is, and what it is not.
//!
//! An **episode** is a provider's conversation boundary. It begins when a client attaches
//! and ends when it detaches, whether or not anybody declared a task, and whether or not
//! the last task anybody declared was finished a week ago (ADR 0041).
//!
//! A **task** is a unit of intended work that a person opens and closes. It is *related* to
//! an episode and it is not part of one. The relation is `Option<TaskId>` and nothing more:
//! no outcome, no profile, no scope, no obligations. Those are the task aggregate's, and an
//! episode that carried a copy of them would be a second account of a record that already
//! exists — the shape of every defect this subsystem has produced.
//!
//! The separation is not stylistic. On 2026-09-05 task `t-20260905034523-a9f1` was marked
//! `handed_over` and never replaced, and for six days this repository wrote no checkpoint
//! and no handover while `doctor` and `watch` reported health. Three code paths each asked
//! a task whether an episode's artefact should be written, and each refusal was correct
//! against its own contract. In this model there is no such question to ask: nothing on
//! [`Episode`] can gate a transition on a task, because a transition is
//! [`super::EpisodeState::may_move_to`] and it takes no task.
//!
//! ```
//! use majordomus_cli::session::{Episode, EpisodeId, EpisodeState, TaskId};
//! use std::path::Path;
//!
//! let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
//! let episode = Episode::opening(id, Path::new("/r"))
//!     .opened_at("2026-09-10T20:55:42Z", "master", "9f3a2a0", 1_789_073_742)
//!     .with_task(TaskId::parse("t-20260905034523-a9f1").expect("the task from the audit"));
//!
//! // the task is `handed_over` in the incident, and the episode still closes
//! assert!(episode.may_move_to(EpisodeState::Closed));
//! assert!(Episode::opening(episode.id.clone(), Path::new("/r")).task.is_none());
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::identity::{CheckoutId, EpisodeId, ProviderSessionId, RepositoryId, TaskId, WorkerId};
use super::state::EpisodeState;

/// One execution episode: a provider's conversation boundary, with the identities that say
/// whose and where it is, and at most a relation to a task.
///
/// ```
/// use majordomus_cli::session::{Episode, EpisodeId, EpisodeState, ProviderSessionId, TaskId};
/// use std::path::Path;
///
/// let id = EpisodeId::parse("s-20260910205542-e2a6").unwrap();
/// let e = Episode::opening(id, Path::new("/r"))
///     .with_provider("claude-code", ProviderSessionId::new("01Bv2gJsf"));
///
/// assert_eq!(e.state, EpisodeState::Opening);
/// assert!(e.task.is_none(), "an episode without a task is ordinary");
///
/// // a task is a relation the episode carries, never a thing that gates it
/// let e = e.with_task(TaskId::parse("t-20260905034523-a9f1").unwrap());
/// assert!(e.task.is_some());
/// assert!(e.state.may_move_to(EpisodeState::Open), "whatever the task says");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Episode {
    /// The episode's own identity, and the only session identity there is.
    pub id: EpisodeId,
    /// Where it is in its life.
    pub state: EpisodeState,
    /// The repository, as this executable computes it. `None` where git cannot be asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<RepositoryId>,
    /// The checkout it was opened in. Two worktrees of one repository differ here, and
    /// that difference is the first tier of every record resolution in the tool.
    pub checkout: CheckoutId,
    /// The provider that opened it, when a provider did. Only something running inside a
    /// provider's own hook can name it, which is what makes it a fact rather than a claim.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// The provider's own name for its conversation: an **external correlation id**, used
    /// to find this episode again and never to identify it. `None` for an episode opened
    /// by hand, which is the one episode no provider session owns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_session: Option<ProviderSessionId>,
    /// The worker, when one was supplied. Never inferred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<WorkerId>,
    /// The task this episode relates to, when there is one. The whole of the relation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskId>,
    /// When it opened, RFC 3339 in UTC. Empty for an episode that is still `Opening`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub started_at: String,
    /// The branch it opened on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    /// The commit it opened at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start_head: String,
    /// The newest sign of life: the later of `started_at` and the newest ledger line this
    /// episode stamped, as Unix seconds. `None` when neither can be read as a timestamp,
    /// which is the one case [`Episode::stranded_after`] refuses to judge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sign_of_life: Option<i64>,
}

impl Episode {
    /// A newly minted episode, before anything durable has been written. It exists as a
    /// state so that a crash between minting an id and writing the open record is something
    /// the model can name rather than an episode that never happened.
    ///
    /// ```
    /// use majordomus_cli::session::{Episode, EpisodeId, EpisodeState};
    /// use std::path::Path;
    ///
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
    /// let e = Episode::opening(id, Path::new("/r"));
    /// assert_eq!(e.state, EpisodeState::Opening);
    /// assert!(e.started_at.is_empty(), "nothing durable has been written yet");
    /// ```
    pub fn opening(id: EpisodeId, root: &std::path::Path) -> Self {
        Episode {
            id,
            state: EpisodeState::Opening,
            repository: RepositoryId::of(root),
            checkout: CheckoutId::of(root),
            provider: String::new(),
            provider_session: None,
            worker: None,
            task: None,
            started_at: String::new(),
            branch: String::new(),
            start_head: String::new(),
            last_sign_of_life: None,
        }
    }

    /// Name the provider and its own session identity — the external correlation id a
    /// hook passes back to find this episode again, and never the episode's own name.
    ///
    /// ```
    /// use majordomus_cli::session::{Episode, EpisodeId, ProviderSessionId};
    /// use std::path::Path;
    ///
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
    /// let e = Episode::opening(id, Path::new("/r"))
    ///     .with_provider("claude-code", ProviderSessionId::new("01Bv2gJsf"));
    /// assert_eq!(e.provider, "claude-code");
    /// assert_ne!(e.provider_session.as_ref().map(|p| p.as_str()), Some(e.id.as_str()));
    /// ```
    pub fn with_provider(
        mut self,
        provider: impl Into<String>,
        session: ProviderSessionId,
    ) -> Self {
        self.provider = provider.into();
        self.provider_session = Some(session);
        self
    }

    /// Relate the episode to a task. The whole of the relation: an episode carries a task's
    /// identity and nothing else of it, so nothing here can gate a transition on one.
    ///
    /// ```
    /// use majordomus_cli::session::{Episode, EpisodeId, EpisodeState, TaskId};
    /// use std::path::Path;
    ///
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
    /// let e = Episode::opening(id, Path::new("/r"))
    ///     .with_task(TaskId::parse("t-20260905034523-a9f1").expect("a task id"));
    /// assert!(e.task.is_some());
    /// assert!(e.state.may_move_to(EpisodeState::Open), "whatever the task says");
    /// ```
    pub fn with_task(mut self, task: TaskId) -> Self {
        self.task = Some(task);
        self
    }

    /// Record the worker, when one was supplied. Absent stays absent: an inferred worker
    /// is indistinguishable from a recorded one the moment it is written down.
    ///
    /// ```
    /// use majordomus_cli::session::{Episode, EpisodeId, WorkerId};
    /// use std::path::Path;
    ///
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
    /// let e = Episode::opening(id, Path::new("/r")).with_worker(WorkerId::supplied(""));
    /// assert!(e.worker.is_none());
    /// ```
    pub fn with_worker(mut self, worker: Option<WorkerId>) -> Self {
        self.worker = worker;
        self
    }

    /// Record where and when it opened, and move it to [`EpisodeState::Open`].
    ///
    /// ```
    /// use majordomus_cli::session::{Episode, EpisodeId, EpisodeState};
    /// use std::path::Path;
    ///
    /// let e = Episode::opening(EpisodeId::parse("s-20260910205542-e2a6").unwrap(), Path::new("/r"))
    ///     .opened_at("2026-09-10T20:55:42Z", "master", "9f3a2a0", 1_789_073_742);
    /// assert_eq!(e.state, EpisodeState::Open);
    /// assert_eq!(e.last_sign_of_life, Some(1_789_073_742));
    /// ```
    pub fn opened_at(
        mut self,
        started_at: impl Into<String>,
        branch: impl Into<String>,
        head: impl Into<String>,
        at: i64,
    ) -> Self {
        debug_assert!(self.state.may_move_to(EpisodeState::Open));
        self.started_at = started_at.into();
        self.branch = branch.into();
        self.start_head = head.into();
        self.last_sign_of_life = Some(at);
        self.state = EpisodeState::Open;
        self
    }

    /// Is this episode's last sign of life older than `threshold_seconds` at `now`?
    ///
    /// The one predicate the state machine needs from the time dimension, and deliberately
    /// the only one. The vocabulary of age — `fresh | aging | stale | unknown | invalid`
    /// and `Thresholds::judge` — belongs to `continuity` (ADR 0041) and is not restated
    /// here; this domain consumes seconds and answers a boolean.
    ///
    /// `None` means *unjudgeable*, not *fine*. An episode whose evidence cannot be read as
    /// a timestamp is skipped and counted by a recovery sweep, never swept: on 2026-09-10 a
    /// first draft of the shell sweep would have deleted a staging directory belonging to a
    /// `scripts/derive` that was running, because "the run that made it did not finish" is
    /// a claim and not a measurement until something reads the clock.
    ///
    /// ```
    /// use majordomus_cli::session::{Episode, EpisodeId};
    /// use std::path::Path;
    ///
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").unwrap();
    /// let live = Episode::opening(id.clone(), Path::new("/r"))
    ///     .opened_at("2026-09-10T20:55:42Z", "master", "9f3a2a0", 1_000);
    /// assert_eq!(live.stranded_after(1_600, 3_600), Some(false), "ten minutes is not stranded");
    /// assert_eq!(live.stranded_after(5_000, 3_600), Some(true), "an hour later it is");
    ///
    /// let unjudgeable = Episode::opening(id, Path::new("/r"));
    /// assert_eq!(unjudgeable.stranded_after(5_000, 3_600), None, "no evidence is not `fine`");
    /// ```
    pub fn stranded_after(&self, now: i64, threshold_seconds: i64) -> Option<bool> {
        let last = self.last_sign_of_life?;
        Some(now.saturating_sub(last) > threshold_seconds)
    }

    /// The transition this episode may take to reach `target`, or `None` when the machine
    /// forbids the move. The store asks this before it writes; a caller that wants to know
    /// without writing asks the same question and gets the same answer.
    ///
    /// ```
    /// use majordomus_cli::session::{Episode, EpisodeId, EpisodeState};
    /// use std::path::Path;
    ///
    /// let mut e = Episode::opening(EpisodeId::parse("s-20260910205542-e2a6").unwrap(), Path::new("/r"))
    ///     .opened_at("2026-09-10T20:55:42Z", "master", "9f3a2a0", 1_000);
    /// assert!(e.may_move_to(EpisodeState::Closed));
    /// e.state = EpisodeState::Closed;
    /// assert!(!e.may_move_to(EpisodeState::Closed), "closing twice is not a move");
    /// ```
    pub fn may_move_to(&self, target: EpisodeState) -> bool {
        self.state.may_move_to(target)
    }
}

impl crate::order::Ordered for Episode {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey {
            group: Some(self.state.as_str()),
            rank: 0,
            label: self.id.as_str(),
            identity: self.id.as_str(),
        }
    }
}

/// How an episode ended, as the record states it.
///
/// ```
/// use majordomus_cli::session::Outcome;
/// // the one thing about an ended session that changes what somebody does next
/// assert_eq!(Outcome::Interrupted.as_str(), "interrupted");
/// assert_ne!(Outcome::Closed, Outcome::Interrupted);
/// ```
///
/// Two values, both self-reported and neither verified — the record says so. `closed` is a
/// worker ending an episode deliberately; `interrupted` tells the next reader that the
/// episode was cut short and its records may be incomplete, which is the one thing about an
/// ended session that changes what somebody does next. A recovery sweep always writes
/// `interrupted`, because an episode nobody ended is by definition one nobody finished.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// A worker ended the episode.
    Closed,
    /// The episode was cut short: a crash, a killed process, or a recovery sweep.
    Interrupted,
}

impl Outcome {
    /// The word as serialised, which is the word the record carries and the word a
    /// resuming worker reads to learn whether the episode was finished or cut short.
    ///
    /// ```
    /// use majordomus_cli::session::Outcome;
    /// assert_eq!(Outcome::Closed.as_str(), "closed");
    /// assert_eq!(Outcome::Interrupted.as_str(), "interrupted");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Closed => "closed",
            Outcome::Interrupted => "interrupted",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn episode() -> Episode {
        Episode::opening(
            EpisodeId::parse("s-20260910205542-e2a6").expect("an id"),
            Path::new("/r"),
        )
        .opened_at("2026-09-10T20:55:42Z", "master", "9f3a2a0", 1_000)
    }

    #[test]
    fn a_tasks_outcome_is_not_reachable_from_an_episodes_transition() {
        // The structural assertion of ADR 0041. `Episode` carries at most a TaskId: there
        // is no outcome on it, so no expression of the form `if outcome != active` can be
        // written against an episode without adding a field this test would fail on.
        let e = episode().with_task(TaskId::parse("t-20260905034523-a9f1").expect("a task"));
        assert!(e.may_move_to(EpisodeState::Closed));
        let json = serde_json::to_value(&e).expect("serialisable");
        let obj = json.as_object().expect("an object");
        assert_eq!(
            obj.get("task").and_then(|v| v.as_str()),
            Some("t-20260905034523-a9f1")
        );
        for forbidden in ["outcome", "profile", "scope", "requires", "task_outcome"] {
            assert!(
                !obj.contains_key(forbidden),
                "an episode carries `{forbidden}`; that is the task aggregate's"
            );
        }
    }

    #[test]
    fn an_episode_with_no_task_is_ordinary_and_still_closes() {
        let e = episode();
        assert!(e.task.is_none());
        assert!(e.may_move_to(EpisodeState::Closed));
    }

    #[test]
    fn an_unjudgeable_episode_is_never_stranded_and_never_fine() {
        let e = Episode::opening(
            EpisodeId::parse("s-20260910205542-e2a6").expect("an id"),
            Path::new("/r"),
        );
        assert_eq!(e.stranded_after(i64::MAX / 2, 1), None);
    }

    #[test]
    fn the_threshold_is_exclusive_at_its_own_boundary() {
        let e = episode();
        assert_eq!(e.stranded_after(1_000 + 3_600, 3_600), Some(false));
        assert_eq!(e.stranded_after(1_001 + 3_600, 3_600), Some(true));
    }
}
