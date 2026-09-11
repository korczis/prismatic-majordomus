//! The one clock this subsystem reads.
//!
//! Ageing is the only part of the lifecycle that is not a pure function of git, and a
//! threshold nobody can control is a threshold nobody can test. Every age in this module is
//! `now - <a timestamp git printed>`, and `now` comes from here: [`Clock::system`] in the
//! executable, [`Clock::fixed`] in a test. No test in this subsystem reads the wall clock,
//! so none of them changes its verdict because it ran at midnight or a year later.
//!
//! The module is crate-private — a model the crate reads for itself is not public surface
//! — so its examples are the unit tests at the foot of this file rather than doctests.

use std::time::{SystemTime, UNIX_EPOCH};

/// Where "now" comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// The host's wall clock. What the executable uses.
    System,
    /// A fixed instant, in seconds since the Unix epoch. What every test uses.
    Fixed(i64),
}

impl Default for Clock {
    fn default() -> Self {
        Clock::System
    }
}

impl Clock {
    /// The host's wall clock.
    pub fn system() -> Self {
        Clock::System
    }

    /// A fixed instant, in seconds since the Unix epoch.
    pub fn fixed(unix_seconds: i64) -> Self {
        Clock::Fixed(unix_seconds)
    }

    /// Now, in seconds since the Unix epoch.
    pub fn unix(self) -> i64 {
        match self {
            Clock::Fixed(t) => t,
            Clock::System => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        }
    }

    /// Now, as RFC 3339, through the executable's one formatter.
    pub fn rfc3339(self) -> String {
        let secs = self.unix().max(0) as u64;
        crate::peers::rfc3339(UNIX_EPOCH + std::time::Duration::from_secs(secs))
    }

    /// How long ago `then` was, in seconds, clamped at zero. A timestamp in the future is
    /// a clock skew between two machines, not an age, and reporting a negative age would
    /// make every threshold comparison below read as "young" by accident.
    pub fn age_since(self, then: i64) -> i64 {
        (self.unix() - then).max(0)
    }

    /// How long ago `then` was, in whole days.
    pub fn days_since(self, then: i64) -> i64 {
        self.age_since(then) / 86_400
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_clock_is_the_instant_it_was_given() {
        let clock = Clock::fixed(1_788_000_000);
        assert_eq!(clock.unix(), 1_788_000_000);
        assert_eq!(clock.rfc3339(), "2026-08-29T10:40:00Z");
    }

    #[test]
    fn an_age_is_never_negative() {
        let clock = Clock::fixed(1_788_000_000);
        // a commit dated in the future is clock skew between two machines, not an age
        assert_eq!(clock.age_since(1_788_000_000 + 10), 0);
        assert_eq!(clock.age_since(1_788_000_000 - 86_400), 86_400);
        assert_eq!(clock.days_since(1_788_000_000 - 3 * 86_400), 3);
    }
}
