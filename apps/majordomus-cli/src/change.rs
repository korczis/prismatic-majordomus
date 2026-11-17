//! That something a follower of the blackboard reads has changed in this process (ADR 0176).
//!
//! The board and the mesh journal hold the facts and say nothing when they change. A page
//! that wants to show them live would otherwise ask again on a timer. Each of them now
//! advances one generation when it changes. The live channel waits on that generation and
//! reads the blackboard again only when it moves. The generation is a number, not a
//! message: what changed is read from the records, as it always was, so nothing can be told
//! twice or told wrong.
//!
//! One generation serves the whole process, because a follower reads one projection of all
//! of it. A change is counted, never queued, so a burst of a thousand ingested events wakes
//! a waiter once.

use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

/// A generation that only moves forward, and that a thread can wait on.
pub(crate) struct Generation {
    n: Mutex<u64>,
    moved: Condvar,
}

impl Generation {
    /// A generation at zero.
    pub(crate) const fn new() -> Self {
        Self {
            n: Mutex::new(0),
            moved: Condvar::new(),
        }
    }

    /// Something changed: move on and wake every waiter.
    pub(crate) fn advance(&self) {
        let mut n = self.n.lock().unwrap_or_else(PoisonError::into_inner);
        *n += 1;
        self.moved.notify_all();
    }

    /// Where it stands now.
    pub(crate) fn current(&self) -> u64 {
        *self.n.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wait until it is past `seen`, or `timeout` passes, and say where it stands. A
    /// generation already past `seen` answers at once.
    pub(crate) fn wait_past(&self, seen: u64, timeout: Duration) -> u64 {
        let n = self.n.lock().unwrap_or_else(PoisonError::into_inner);
        let (n, _) = self
            .moved
            .wait_timeout_while(n, timeout, |n| *n <= seen)
            .unwrap_or_else(PoisonError::into_inner);
        *n
    }
}

impl Default for Generation {
    fn default() -> Self {
        Self::new()
    }
}

/// The blackboard's generation: advanced by every change of a board of this process and of
/// its mesh journal.
pub(crate) static BLACKBOARD: Generation = Generation::new();

/// The generation a capability's answer follows beyond the repository, when it has one: the
/// blackboard's for `blackboard.snapshot`, none for any other. A reading of such a source
/// taken before the generation moved is not current (`capability::readings`), so a screen
/// read after a change frame never shows the blackboard from before the change as if it were
/// now.
pub(crate) fn generation_of(capability: &str) -> Option<u64> {
    (capability == "blackboard.snapshot").then(|| BLACKBOARD.current())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    /// A waiter on another thread is woken by the change, not by its timeout.
    #[test]
    fn a_change_wakes_a_waiter_at_once() {
        let g = Arc::new(Generation::new());
        let waiter = {
            let g = Arc::clone(&g);
            std::thread::spawn(move || {
                let started = Instant::now();
                (g.wait_past(0, Duration::from_secs(30)), started.elapsed())
            })
        };
        std::thread::sleep(Duration::from_millis(50));
        g.advance();
        let (n, waited) = waiter.join().unwrap();
        assert_eq!(n, 1);
        assert!(
            waited < Duration::from_secs(5),
            "woken by the change: {waited:?}"
        );
    }

    /// A generation already past what the waiter saw answers at once, two changes wake it
    /// once, and a generation that did not move answers when the timeout passes.
    #[test]
    fn a_waiter_is_answered_by_the_generation_or_by_its_timeout() {
        let g = Generation::new();
        assert_eq!(
            g.wait_past(0, Duration::from_millis(10)),
            0,
            "nothing moved: the timeout"
        );
        g.advance();
        g.advance();
        assert_eq!(
            g.wait_past(0, Duration::from_secs(5)),
            2,
            "two changes, one wake"
        );
    }

    /// A default generation is a new one, and only the blackboard's capability follows it.
    #[test]
    fn a_default_generation_starts_at_zero_and_one_capability_follows_the_blackboard() {
        let g = Generation::default();
        assert_eq!(g.current(), 0);
        g.advance();
        assert_eq!(g.current(), 1);
        assert!(generation_of("blackboard.snapshot").is_some());
        assert_eq!(generation_of("peers.list"), None);
    }
}
