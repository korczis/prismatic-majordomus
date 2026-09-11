//! Where an episode is in its life, and the transitions that move it there.
//!
//! # Why this is a type and not a convention
//!
//! The lifecycle used to be expressed as the presence and absence of files: an episode is
//! open because `state/sessions-open/<key>.yaml` exists, and closed because it does not and
//! a record does. That representation has no state it refuses to be in. It cannot say that
//! a closed episode may not be closed again — which is how one episode in this repository
//! came to have four immutable records — and it cannot say that a transition is legal
//! without also performing it, which is how every check of the form "is this allowed" ended
//! up written separately at each call site, in shell, three times, with the task guard ADR
//! 0041 removed embedded in one of them.
//!
//! So the transitions are the contract and [`EpisodeState::may_move_to`] is the one place
//! they are written down: the store asks before every change, and a caller that reads a
//! terminal state never sees it move again. The same shape as
//! [`crate::execution::ExecutionState`], for the same reason and deliberately in the same
//! style — a reader who has met one has met the other.
//!
//! ```text
//!   Opening ──► Open ──┬──► Detached ──┬──► Open        (resume)
//!                      │               └──► Closed      (close, or recover)
//!                      ├──► Open                        (checkpoint, repeated start: no move)
//!                      └──► Closed                      (close, or recover)
//! ```
//!
//! # What is not in the signature
//!
//! There is no task. [`EpisodeState::may_move_to`] takes a target state and nothing else;
//! [`Transition`] carries no task field. This is the whole of ADR 0041 expressed as a type:
//! the defect it removed was `outcome != active -> skip` on the path that writes an
//! episode's artefacts, and in this model there is nowhere to put it. A task's outcome may
//! change what a continuation record *says* — that belongs to the record's composition, not
//! to the state machine — and it may never decide whether one is written.
//!
//! ```
//! use majordomus_cli::session::{EpisodeState, Transition};
//!
//! // the whole machine, asked rather than drawn
//! assert!(EpisodeState::Open.may_move_to(EpisodeState::Closed));
//! assert!(!EpisodeState::Closed.may_move_to(EpisodeState::Open));
//! assert_eq!(Transition::Recover.from_to(), (EpisodeState::Detached, EpisodeState::Closed));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where an episode is: minted, open, detached from the connection that held it, or closed
/// into the immutable record.
///
/// ```
/// use majordomus_cli::session::EpisodeState;
///
/// assert!(EpisodeState::Open.may_move_to(EpisodeState::Closed));
/// assert!(!EpisodeState::Closed.may_move_to(EpisodeState::Open), "a record is immutable");
/// assert!(EpisodeState::Open.may_move_to(EpisodeState::Open), "a resume is not a move");
/// assert!(EpisodeState::Closed.is_final());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeState {
    /// An identity has been minted and nothing durable has been written yet. It exists so
    /// that a crash between minting an id and writing the open record is a state the model
    /// can name rather than an episode that never happened.
    Opening,
    /// The open record stands. The worker is here, or was recently.
    Open,
    /// The connection that held the episode went away, and the episode did not. A dropped
    /// socket is the most ordinary thing that happens to a session; an episode that closed
    /// itself on every disconnect would lose the work of every worker whose network
    /// hiccuped. It is still resumable, and it is still recoverable if nobody resumes it.
    Detached,
    /// The immutable record has been published. Terminal, and terminal is the point: the
    /// four records of `s-20260909152316-024f` all exist because nothing in the previous
    /// representation could say this.
    Closed,
}

impl EpisodeState {
    /// May an episode in this state move to `target`?
    ///
    /// The whole transition table, and the only copy of it. Note what the signature does
    /// not take: there is no task, no outcome and no policy. A transition of an episode is
    /// a fact about the episode.
    ///
    /// ```
    /// use majordomus_cli::session::EpisodeState::*;
    ///
    /// assert!(Opening.may_move_to(Open));
    /// assert!(Open.may_move_to(Detached) && Detached.may_move_to(Open));
    /// assert!(Open.may_move_to(Closed) && Detached.may_move_to(Closed));
    ///
    /// // an episode that never opened cannot be closed into a record of what it did
    /// assert!(!Opening.may_move_to(Closed));
    /// // and nothing leaves the terminal state
    /// for target in [Opening, Open, Detached, Closed] {
    ///     assert!(!Closed.may_move_to(target));
    /// }
    /// ```
    pub fn may_move_to(self, target: EpisodeState) -> bool {
        use EpisodeState::*;
        match (self, target) {
            (Closed, _) => false,
            (Opening, Open) => true,
            (Opening, _) => false,
            (Open, Open) | (Open, Detached) | (Open, Closed) => true,
            (Open, Opening) => false,
            (Detached, Open) | (Detached, Closed) => true,
            (Detached, Opening) | (Detached, Detached) => false,
        }
    }

    /// Is this a state an episode never leaves? True of `closed` alone, which is what a
    /// store asks before it writes and a reader asks before it waits for a change.
    ///
    /// ```
    /// use majordomus_cli::session::EpisodeState;
    /// assert!(EpisodeState::Closed.is_final());
    /// assert!(!EpisodeState::Detached.is_final(), "a detached episode can still be resumed");
    /// ```
    pub fn is_final(self) -> bool {
        matches!(self, EpisodeState::Closed)
    }

    /// The word as serialised, which is the word every projection of the machine carries
    /// and the word a person reads in `session status`.
    ///
    /// ```
    /// use majordomus_cli::session::EpisodeState;
    /// assert_eq!(EpisodeState::Detached.as_str(), "detached");
    /// assert_eq!(EpisodeState::Closed.as_str(), "closed");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            EpisodeState::Opening => "opening",
            EpisodeState::Open => "open",
            EpisodeState::Detached => "detached",
            EpisodeState::Closed => "closed",
        }
    }

    /// Every state, in the order a reader meets them. The projection of the machine is
    /// built from this rather than from a list written a second time somewhere else.
    pub const ALL: [EpisodeState; 4] = [
        EpisodeState::Opening,
        EpisodeState::Open,
        EpisodeState::Detached,
        EpisodeState::Closed,
    ];
}

/// What moved an episode. The vocabulary the ADR 0041 audit needs: `open`, `resume`,
/// `checkpoint`, `detach`, `close`, `recover`.
///
/// One of them does not change the state, and it is a transition anyway. A checkpoint
/// leaves an open episode open — but it is one of the two events whose *absence* is what
/// the audit was looking for, and an event that is modelled as "nothing happened" cannot
/// be counted. The six days of silence were six days in which `session.started` and
/// `session.closed` kept arriving and the two events that produce the records a worker
/// resumes from did not. A vocabulary in which those two are not events is a vocabulary in
/// which that outage is invisible.
///
/// ```
/// use majordomus_cli::session::{EpisodeState, Transition};
///
/// // a checkpoint is a transition that does not move the state, and says so
/// let c = Transition::Checkpoint;
/// assert_eq!(c.from_to(), (EpisodeState::Open, EpisodeState::Open));
/// assert!(!c.moves_state());
///
/// // a resume is canonically the return to a detached episode; a provider's start event
/// // arriving at one that is already open is the same transition seen from `Open`, which
/// // the machine allows without a move
/// assert_eq!(Transition::Resume.from_to(), (EpisodeState::Detached, EpisodeState::Open));
/// assert!(EpisodeState::Open.may_move_to(EpisodeState::Open));
///
/// // and every transition's own move is one the machine allows
/// for t in Transition::ALL {
///     let (from, to) = t.from_to();
///     assert!(from.may_move_to(to), "{} is not a legal move", t.as_str());
/// }
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    /// The episode begins: an id is minted and the open record is written.
    Open,
    /// The worker came back to a detached episode. A provider's start event also fires
    /// again on a resume and on a compaction against an episode that is still open; that
    /// is the same transition observed from `Open`, and the honest answer to "your episode
    /// is already open" is to keep it rather than to open a second one.
    Resume,
    /// A progress note was recorded against the episode. An artefact of the episode and
    /// not of a task (ADR 0041): a checkpoint outside a task records `task: none` and is a
    /// normal record, not a refusal.
    Checkpoint,
    /// The connection went away without an end event.
    Detach,
    /// The worker ended the episode and the immutable record was published.
    Close,
    /// Nobody ended it and its last sign of life is older than the stranded threshold, so
    /// maintenance closed it. The record says so: an episode closed this way is
    /// `interrupted`, because a reader resuming from it needs to know it may be incomplete.
    Recover,
}

impl Transition {
    /// The states this transition runs between: where an episode must be for it to apply,
    /// and where it is afterwards.
    ///
    /// ```
    /// use majordomus_cli::session::{EpisodeState, Transition};
    /// assert_eq!(Transition::Close.from_to(), (EpisodeState::Open, EpisodeState::Closed));
    /// assert_eq!(Transition::Detach.from_to(), (EpisodeState::Open, EpisodeState::Detached));
    /// ```
    pub fn from_to(self) -> (EpisodeState, EpisodeState) {
        use EpisodeState as S;
        match self {
            Transition::Open => (S::Opening, S::Open),
            Transition::Resume => (S::Detached, S::Open),
            Transition::Checkpoint => (S::Open, S::Open),
            Transition::Detach => (S::Open, S::Detached),
            Transition::Close => (S::Open, S::Closed),
            Transition::Recover => (S::Detached, S::Closed),
        }
    }

    /// Does this transition change the state, or is it an event inside one? A checkpoint
    /// is the second kind, and it is a transition anyway: an event modelled as "nothing
    /// happened" cannot be counted, and counting it is how a stopped writer becomes visible.
    ///
    /// ```
    /// use majordomus_cli::session::Transition;
    /// assert!(!Transition::Checkpoint.moves_state());
    /// assert!(Transition::Close.moves_state());
    /// ```
    pub fn moves_state(self) -> bool {
        let (from, to) = self.from_to();
        from != to
    }

    /// The word as serialised, which is what a projection of the machine carries and what
    /// a ledger event's name is read against.
    ///
    /// ```
    /// use majordomus_cli::session::Transition;
    /// assert_eq!(Transition::Recover.as_str(), "recover");
    /// assert_eq!(Transition::Checkpoint.as_str(), "checkpoint");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Transition::Open => "open",
            Transition::Resume => "resume",
            Transition::Checkpoint => "checkpoint",
            Transition::Detach => "detach",
            Transition::Close => "close",
            Transition::Recover => "recover",
        }
    }

    /// Every transition, in lifecycle order.
    pub const ALL: [Transition; 6] = [
        Transition::Open,
        Transition::Resume,
        Transition::Checkpoint,
        Transition::Detach,
        Transition::Close,
        Transition::Recover,
    ];
}

/// The state machine as data, so that every surface projects the same machine instead of
/// drawing its own.
///
/// ```
/// use majordomus_cli::session::Machine;
///
/// let m = Machine::describe();
/// assert!(m.states.iter().filter(|s| s.terminal).count() == 1);
/// assert!(m.independent_of.iter().any(|note| note.contains("task")));
/// ```
///
/// This is what `session.machine` answers. It exists because the same diagram is currently
/// redrawn in `docs/CONTINUITY.md`, in `lib/session.sh`'s comments and in the Cockpit, and
/// three drawings of one machine drift — the `lib/session.sh` one still had the task guard
/// in it after ADR 0041 removed it from the code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Machine {
    /// Every state, with what it means and whether it is terminal.
    pub states: Vec<StateView>,
    /// Every transition, with the states it runs between.
    pub transitions: Vec<TransitionView>,
    /// What the machine deliberately does not depend on, stated so that a reader does not
    /// have to infer an absence. One entry today: task state.
    pub independent_of: Vec<String>,
}

/// One state of the machine: its word, whether an episode ever leaves it, and where it may
/// go from there.
///
/// ```
/// use majordomus_cli::session::{EpisodeState, Machine, StateView};
///
/// let m = Machine::describe();
/// let closed: &StateView =
///     m.states.iter().find(|s| s.state == EpisodeState::Closed).expect("a state");
/// assert!(closed.terminal && closed.may_move_to.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StateView {
    /// The state's word.
    pub state: EpisodeState,
    /// Whether an episode ever leaves it.
    pub terminal: bool,
    /// The states an episode in this one may move to, in canonical order.
    pub may_move_to: Vec<EpisodeState>,
}

/// One transition of the machine: its word, the states it runs between, and whether it
/// moves the state at all.
///
/// ```
/// use majordomus_cli::session::{Machine, Transition, TransitionView};
///
/// let m = Machine::describe();
/// let close: &TransitionView = m
///     .transitions
///     .iter()
///     .find(|t| t.transition == Transition::Close)
///     .expect("a transition");
/// assert!(close.moves_state);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TransitionView {
    /// The transition's word.
    pub transition: Transition,
    /// Where it starts.
    pub from: EpisodeState,
    /// Where it ends.
    pub to: EpisodeState,
    /// False for `resume` and `checkpoint`, which are events inside a state.
    pub moves_state: bool,
}

impl Machine {
    /// The machine, derived from the two enums above and from nothing else.
    ///
    /// ```
    /// use majordomus_cli::session::{EpisodeState, Machine, Transition};
    ///
    /// let m = Machine::describe();
    /// assert_eq!(m.states.len(), EpisodeState::ALL.len());
    /// assert_eq!(m.transitions.len(), Transition::ALL.len());
    /// assert!(m.states.iter().filter(|s| s.terminal).count() == 1, "one terminal state");
    /// assert!(m.independent_of.iter().any(|s| s.contains("task")));
    /// ```
    pub fn describe() -> Self {
        let states = EpisodeState::ALL
            .iter()
            .map(|state| StateView {
                state: *state,
                terminal: state.is_final(),
                may_move_to: EpisodeState::ALL
                    .iter()
                    .copied()
                    .filter(|t| state.may_move_to(*t))
                    .collect(),
            })
            .collect();
        let transitions = Transition::ALL
            .iter()
            .map(|t| {
                let (from, to) = t.from_to();
                TransitionView {
                    transition: *t,
                    from,
                    to,
                    moves_state: t.moves_state(),
                }
            })
            .collect();
        Machine {
            states,
            transitions,
            independent_of: vec![
                "task state: an episode's lifecycle does not depend on a task's outcome, \
                 and may_move_to takes no task (ADR 0041)"
                    .to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_terminal_state_is_terminal_from_everywhere() {
        for target in EpisodeState::ALL {
            assert!(
                !EpisodeState::Closed.may_move_to(target),
                "closed -> {} is reachable",
                target.as_str()
            );
        }
        assert_eq!(
            EpisodeState::ALL
                .iter()
                .filter(|s| s.is_final())
                .collect::<Vec<_>>()
                .len(),
            1
        );
    }

    #[test]
    fn every_state_except_the_terminal_one_reaches_closed() {
        // The property that matters for recovery: an episode nobody ended must always be
        // closable. `Opening` reaches it through `Open`.
        assert!(EpisodeState::Open.may_move_to(EpisodeState::Closed));
        assert!(EpisodeState::Detached.may_move_to(EpisodeState::Closed));
        assert!(EpisodeState::Opening.may_move_to(EpisodeState::Open));
    }

    #[test]
    fn the_machine_is_derived_and_carries_no_second_list() {
        let m = Machine::describe();
        let words: Vec<&str> = m.states.iter().map(|s| s.state.as_str()).collect();
        assert_eq!(words, ["opening", "open", "detached", "closed"]);
        for view in &m.states {
            assert_eq!(view.terminal, view.may_move_to.is_empty());
        }
    }

    #[test]
    fn a_resume_and_a_checkpoint_are_events_and_not_moves() {
        assert!(!Transition::Checkpoint.moves_state());
        assert!(
            Transition::Resume.moves_state(),
            "from detached it is a move"
        );
        assert!(Transition::Close.moves_state());
        assert!(Transition::Recover.moves_state());
    }
}
