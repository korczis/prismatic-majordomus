//! The ageing thresholds, declared once.
//!
//! Every "too long" in this subsystem is a key of `.ai/repo/policy.yaml` under `lifecycle:`
//! — not a constant in Rust, not a number in a shell script, and not a sentence in a
//! document. The policy file is the declaration, the JSON Schema beside it is the contract,
//! and [`AgingPolicy::resolve`] is the only reader. A threshold written down twice is a
//! threshold that will disagree with itself, and this repository already forbids that shape
//! for capabilities and commands; ageing is no different.
//!
//! The module is crate-private, so its examples are the unit tests at the foot of the file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `lifecycle:` of the policy, as written: durations as text, in the grammar the shell's
/// own `mj_duration_secs` reads — digits with an optional `s`, `m`, `h`, or `d` suffix,
/// bare digits meaning seconds.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct LifecyclePolicy {
    /// Newer than this and a worktree is being worked in, not neglected.
    #[serde(default)]
    pub active_within: Option<String>,
    /// A worktree with no commit newer than this, and nothing unique or uncommitted, is
    /// stale.
    #[serde(default)]
    pub worktree_inactive_after: Option<String>,
    /// A branch merged into the trunk and untouched for longer than this is a finding.
    #[serde(default)]
    pub branch_merged_after: Option<String>,
    /// A pull request that has been mergeable for longer than this is a finding: nothing
    /// technical is stopping it, so it is waiting on a person who has not noticed.
    #[serde(default)]
    pub pull_request_mergeable_after: Option<String>,
    /// A pull request whose checks have been failing for longer than this is a finding.
    #[serde(default)]
    pub pull_request_failing_after: Option<String>,
    /// Commits reachable from no ref on origin, older than this, are a finding whatever
    /// else is true of them.
    #[serde(default)]
    pub stranded_after: Option<String>,
}

/// The thresholds, resolved to seconds, with the text they were read from. Carried in every
/// report so that a verdict says what it judged against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgingPolicy {
    /// `lifecycle.active_within`, in seconds.
    pub active_within_seconds: i64,
    /// `lifecycle.worktree_inactive_after`, in seconds.
    pub worktree_inactive_after_seconds: i64,
    /// `lifecycle.branch_merged_after`, in seconds.
    pub branch_merged_after_seconds: i64,
    /// `lifecycle.pull_request_mergeable_after`, in seconds.
    pub pull_request_mergeable_after_seconds: i64,
    /// `lifecycle.pull_request_failing_after`, in seconds.
    pub pull_request_failing_after_seconds: i64,
    /// `lifecycle.stranded_after`, in seconds.
    pub stranded_after_seconds: i64,
    /// True when the repository declared none of these and the built-in defaults applied.
    /// A report that judged against a default rather than against the repository's own
    /// policy says so rather than pretending the repository chose these numbers.
    pub defaulted: bool,
}

/// The defaults, in the same grammar the policy uses. They exist so that a repository
/// without a `lifecycle:` section still gets an answer; they are not the policy.
const DEFAULT_ACTIVE_WITHIN: &str = "30m";
const DEFAULT_WORKTREE_INACTIVE_AFTER: &str = "7d";
const DEFAULT_BRANCH_MERGED_AFTER: &str = "7d";
const DEFAULT_PR_MERGEABLE_AFTER: &str = "2d";
const DEFAULT_PR_FAILING_AFTER: &str = "1d";
const DEFAULT_STRANDED_AFTER: &str = "1d";

impl Default for AgingPolicy {
    fn default() -> Self {
        AgingPolicy {
            active_within_seconds: parse_duration(DEFAULT_ACTIVE_WITHIN).unwrap_or(1_800),
            worktree_inactive_after_seconds: parse_duration(DEFAULT_WORKTREE_INACTIVE_AFTER)
                .unwrap_or(604_800),
            branch_merged_after_seconds: parse_duration(DEFAULT_BRANCH_MERGED_AFTER)
                .unwrap_or(604_800),
            pull_request_mergeable_after_seconds: parse_duration(DEFAULT_PR_MERGEABLE_AFTER)
                .unwrap_or(172_800),
            pull_request_failing_after_seconds: parse_duration(DEFAULT_PR_FAILING_AFTER)
                .unwrap_or(86_400),
            stranded_after_seconds: parse_duration(DEFAULT_STRANDED_AFTER).unwrap_or(86_400),
            defaulted: true,
        }
    }
}

impl AgingPolicy {
    /// Resolve the declared policy, falling back to the defaults key by key. A key that is
    /// present but unreadable falls back to its default rather than failing the whole
    /// report: an unparseable threshold must not make the inventory unavailable, and the
    /// schema refuses it at the gate where refusing is the right answer.
    pub fn resolve(declared: &LifecyclePolicy) -> Self {
        let mut resolved = AgingPolicy::default();
        let mut any = false;
        let mut take = |value: &Option<String>, slot: &mut i64| {
            if let Some(text) = value {
                if let Some(seconds) = parse_duration(text) {
                    *slot = seconds;
                    any = true;
                }
            }
        };
        take(&declared.active_within, &mut resolved.active_within_seconds);
        take(
            &declared.worktree_inactive_after,
            &mut resolved.worktree_inactive_after_seconds,
        );
        take(
            &declared.branch_merged_after,
            &mut resolved.branch_merged_after_seconds,
        );
        take(
            &declared.pull_request_mergeable_after,
            &mut resolved.pull_request_mergeable_after_seconds,
        );
        take(
            &declared.pull_request_failing_after,
            &mut resolved.pull_request_failing_after_seconds,
        );
        take(&declared.stranded_after, &mut resolved.stranded_after_seconds);
        resolved.defaulted = !any;
        resolved
    }
}

/// Read a duration in the grammar the shell tool's `mj_duration_secs` reads: digits with an
/// optional `s`, `m`, `h` or `d` suffix, bare digits meaning seconds. Anything else is
/// `None`; there is no partial parse and no silent zero.
///
/// `the_grammar_is_the_shells_grammar` below is the worked example.
pub fn parse_duration(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (digits, multiplier) = match text.as_bytes()[text.len() - 1] {
        b's' => (&text[..text.len() - 1], 1),
        b'm' => (&text[..text.len() - 1], 60),
        b'h' => (&text[..text.len() - 1], 3_600),
        b'd' => (&text[..text.len() - 1], 86_400),
        b'0'..=b'9' => (text, 1),
        _ => return None,
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<i64>().ok()?.checked_mul(multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declared_threshold_wins_and_an_absent_one_defaults() {
        let declared = LifecyclePolicy {
            worktree_inactive_after: Some("3d".into()),
            ..Default::default()
        };
        let resolved = AgingPolicy::resolve(&declared);
        assert_eq!(resolved.worktree_inactive_after_seconds, 3 * 86_400);
        assert_eq!(
            resolved.active_within_seconds,
            AgingPolicy::default().active_within_seconds
        );
        assert!(
            !resolved.defaulted,
            "the repository declared something, so the report must not claim it defaulted"
        );
    }

    #[test]
    fn an_unreadable_threshold_falls_back_rather_than_failing_the_inventory() {
        let declared = LifecyclePolicy {
            active_within: Some("whenever".into()),
            ..Default::default()
        };
        let resolved = AgingPolicy::resolve(&declared);
        assert_eq!(
            resolved.active_within_seconds,
            AgingPolicy::default().active_within_seconds
        );
        assert!(resolved.defaulted);
    }

    #[test]
    fn the_defaults_are_ordered_so_that_active_never_swallows_inactive() {
        let p = AgingPolicy::default();
        assert!(p.worktree_inactive_after_seconds > p.active_within_seconds);
        assert!(p.defaulted, "nothing was declared, and the report must say so");
    }

    #[test]
    fn the_grammar_is_the_shells_grammar() {
        // every form lib/common.sh's mj_duration_secs accepts, with the same meaning
        assert_eq!(parse_duration("0"), Some(0));
        assert_eq!(parse_duration("45s"), Some(45));
        assert_eq!(parse_duration("15m"), Some(900));
        assert_eq!(parse_duration("1h"), Some(3_600));
        // and the one extension, for thresholds a person states in days
        assert_eq!(parse_duration("14d"), Some(1_209_600));
        // refusals
        for bad in ["", " ", "m", "-1", "1.5h", "1w", "15 m", "fifteen"] {
            assert_eq!(parse_duration(bad), None, "{bad:?} must not parse");
        }
    }
}
