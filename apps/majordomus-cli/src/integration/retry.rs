//! Bounded retries for the forge's transient failures, and only for those.
//!
//! A timeout, a 5xx, a rate limit or a dropped connection says nothing about the pull
//! request; asking again a little later is the right answer, a bounded number of times, with
//! the wait doubling between attempts. Anything else — a refusal by the branch protection, a
//! head that moved, an authentication failure, a pull request that does not exist — is the
//! answer, and is returned at once: retrying a semantic failure only hammers the API with a
//! question it already answered.
//!
//! What is retried is a read or an idempotent write. The merge itself is not
//! ([`super::drain::ForgeIntegrator`]): a merge that timed out may have landed, and the
//! verification after it — which *is* retried — is what finds out.

use std::time::Duration;

/// How many times, and how long to wait between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    /// Attempts in all, the first included. One means no retry.
    pub attempts: u32,
    /// The wait before the second attempt; doubled before each later one.
    pub initial: Duration,
    /// The longest single wait.
    pub max: Duration,
}

/// The forge's policy: four attempts, waiting 2 s, 4 s and 8 s — under fifteen seconds in
/// all, short enough that a drain step is not held for minutes by a forge that is down.
pub const FORGE: Backoff = Backoff {
    attempts: 4,
    initial: Duration::from_secs(2),
    max: Duration::from_secs(30),
};

impl Backoff {
    /// The wait before attempt `n` (1-based, so `n >= 2`).
    ///
    /// ```text
    /// FORGE.delay(2) == 2 s, FORGE.delay(3) == 4 s, FORGE.delay(4) == 8 s
    /// ```text
    pub fn delay(&self, n: u32) -> Duration {
        let doublings = n.saturating_sub(2).min(16);
        self.initial.saturating_mul(1u32 << doublings).min(self.max)
    }
}

/// Whether a failure message describes a transient condition of the network or the forge,
/// as `gh` and `git` word them. Conservative: a message that matches nothing is semantic.
pub fn transient(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    const WORDS: &[&str] = &[
        "timed out",
        "timeout",
        "rate limit",
        "secondary rate",
        "abuse detection",
        "http 500",
        "http 502",
        "http 503",
        "http 504",
        "500 internal server error",
        "502 bad gateway",
        "503 service unavailable",
        "504 gateway time",
        "connection reset",
        "connection refused",
        "could not resolve host",
        "temporary failure in name resolution",
        "tls handshake",
        "unexpected eof",
        "the remote end hung up unexpectedly",
        "early eof",
    ];
    WORDS.iter().any(|w| m.contains(w))
}

/// Run `op` until it succeeds, fails semantically, or `policy.attempts` are spent. `sleep`
/// is how a wait is taken: the forge passes `std::thread::sleep`, a test records the waits.
/// A failure after retries says how many attempts were made.
pub fn with_backoff<T>(
    policy: &Backoff,
    sleep: &mut dyn FnMut(Duration),
    mut op: impl FnMut() -> Result<T, String>,
) -> Result<T, String> {
    let mut attempt = 1;
    loop {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if transient(&e) && attempt < policy.attempts => {
                attempt += 1;
                sleep(policy.delay(attempt));
            }
            Err(e) if attempt > 1 => {
                return Err(format!("{e} (after {attempt} attempts)"));
            }
            Err(e) => return Err(e),
        }
    }
}

/// [`with_backoff`] with the forge's policy and a real sleep.
pub fn forge<T>(op: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    with_backoff(&FORGE, &mut std::thread::sleep, op)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transient_failure_is_retried_with_doubling_waits_and_then_succeeds() {
        let mut waits = Vec::new();
        let mut calls = 0;
        let r = with_backoff(&FORGE, &mut |d| waits.push(d), || {
            calls += 1;
            if calls < 3 {
                Err("HTTP 502: Bad Gateway (https://api.github.com/graphql)".to_string())
            } else {
                Ok(calls)
            }
        });
        assert_eq!(r, Ok(3));
        assert_eq!(waits, [Duration::from_secs(2), Duration::from_secs(4)]);
    }

    #[test]
    fn a_semantic_failure_is_returned_at_once() {
        let mut calls = 0;
        let r: Result<(), String> = with_backoff(&FORGE, &mut |_| {}, || {
            calls += 1;
            Err(
                "Pull request #7 is not mergeable: the base branch policy prohibits the merge"
                    .into(),
            )
        });
        assert_eq!(calls, 1, "a refusal is the answer, not an outage");
        assert!(r.unwrap_err().starts_with("Pull request #7"));
    }

    #[test]
    fn retries_are_bounded_and_the_failure_says_how_many() {
        let mut waits = Vec::new();
        let mut calls = 0;
        let r: Result<(), String> = with_backoff(&FORGE, &mut |d| waits.push(d), || {
            calls += 1;
            Err("API rate limit exceeded for user".into())
        });
        assert_eq!(calls, FORGE.attempts as usize);
        assert_eq!(waits.len(), FORGE.attempts as usize - 1);
        let e = r.unwrap_err();
        assert!(e.ends_with("(after 4 attempts)"), "{e}");
    }

    #[test]
    fn the_wait_doubles_up_to_its_ceiling() {
        let b = Backoff {
            attempts: 10,
            initial: Duration::from_secs(2),
            max: Duration::from_secs(30),
        };
        let waits: Vec<u64> = (2..=7).map(|n| b.delay(n).as_secs()).collect();
        assert_eq!(waits, [2, 4, 8, 16, 30, 30]);
    }

    #[test]
    fn only_outage_words_are_transient() {
        for t in [
            "dial tcp: i/o timeout",
            "error connecting to api.github.com: connection reset by peer",
            "You have exceeded a secondary rate limit",
            "fatal: the remote end hung up unexpectedly",
            "HTTP 503",
        ] {
            assert!(transient(t), "{t}");
        }
        for s in [
            "HTTP 401: Bad credentials",
            "HTTP 404: Not Found",
            "Head branch was modified. Review and try the merge again.",
            "could not find pull request",
            "",
        ] {
            assert!(!transient(s), "{s}");
        }
    }
}
