//! What this repository holds a commit to, as data.
//!
//! The type vocabulary is deliberately *not* here: [`crate::release::ChangeKind`] already
//! is the list of words a commit's type may be, the changelog already renders from it, and
//! a second copy under a `types:` key would be exactly the duplication this subsystem
//! exists to remove. What is here is only what a repository can reasonably decide
//! differently — a subject width, and how loudly to report three things that are findings
//! in one project and noise in another.
//!
//! The block is read from `.ai/repo/policy.yaml`, the canonical policy this repository
//! already has, and every field has the default this repository's own history satisfies, so
//! a policy that says nothing about commits still yields the same verdict.
//!
//! ```
//! use majordomus_cli::commit::CommitPolicy;
//! let p = CommitPolicy::default();
//! assert_eq!(p.subject_max_chars, 72);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Severity;

/// How loudly one judgement is reported, including not at all.
///
/// `Off` is a value and not an absence: a repository that has decided a check does not
/// apply to it has made a decision, and a decision is worth being able to read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum FindingLevel {
    /// Never reported.
    Off,
    /// Reported; changes no verdict.
    Info,
    /// Reported; the verdict is still a pass.
    #[default]
    Warning,
    /// Reported; the verdict fails.
    Error,
}

impl FindingLevel {
    /// The severity a finding at this level carries, or `None` when it is not reported.
    pub fn severity(self) -> Option<Severity> {
        match self {
            FindingLevel::Off => None,
            FindingLevel::Info => Some(Severity::Info),
            FindingLevel::Warning => Some(Severity::Warning),
            FindingLevel::Error => Some(Severity::Error),
        }
    }
}

/// `commit:` in the repository policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommitPolicy {
    /// The widest a subject line may be. 72 is what a terminal and every git viewer show
    /// without wrapping, and what this repository's history already keeps to.
    #[serde(default = "subject_max_chars")]
    pub subject_max_chars: usize,
    /// A scope outside the vocabulary the repository's own history and layout yield.
    /// A warning by default: the vocabulary is inferred, and inference must not refuse the
    /// first commit of a new subsystem.
    #[serde(default)]
    pub scope_unknown: FindingLevel,
    /// A `fix` whose staged files include no test. The strongest available evidence that a
    /// bug fix carries a regression test, and the cheapest place to ask for it.
    #[serde(default)]
    pub fix_requires_test: FindingLevel,
    /// A record id in the message — `I1305`, `M000` — that the layer does not hold. An
    /// error by default: a reference to a record that does not exist cannot be told from a
    /// good one until somebody follows it, which is usually months later.
    #[serde(default = "error")]
    pub reference_unresolved: FindingLevel,
    /// A commit that declares itself breaking and says nothing else. An error by default:
    /// the `!` is the only mark a release reads, and a release note derived from a breaking
    /// change with no explanation is a release note that says a thing broke.
    #[serde(default = "error")]
    pub breaking_unexplained: FindingLevel,
}

fn subject_max_chars() -> usize {
    72
}

fn error() -> FindingLevel {
    FindingLevel::Error
}

impl Default for CommitPolicy {
    fn default() -> Self {
        CommitPolicy {
            subject_max_chars: subject_max_chars(),
            scope_unknown: FindingLevel::Warning,
            fix_requires_test: FindingLevel::Warning,
            reference_unresolved: FindingLevel::Error,
            breaking_unexplained: FindingLevel::Error,
        }
    }
}

/// CommitSubject prefixes git itself writes, which no commit convention applies to.
///
/// These are not a repository's choice and are not in the policy: `git merge`, `git revert`
/// and `git commit --fixup` compose these subjects, a person did not, and a validator that
/// refused them would be refusing git. The list is short and closed because git's is.
pub const GIT_AUTHORED_PREFIXES: &[&str] = &[
    "Merge ",
    "Revert \"",
    "Revert: ",
    "fixup! ",
    "squash! ",
    "amend! ",
];

/// Whether git composed this subject rather than a person.
///
/// ```
/// use majordomus_cli::commit::policy::git_authored;
/// assert!(git_authored("Merge pull request #249 from korczis/fix"));
/// assert!(git_authored("Revert \"feat(x): a thing\""));
/// assert!(!git_authored("feat(x): a thing"));
/// ```
pub fn git_authored(subject: &str) -> bool {
    GIT_AUTHORED_PREFIXES.iter().any(|p| subject.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_policy_that_says_nothing_is_the_default() {
        let p: CommitPolicy = serde_json::from_str("{}").expect("an empty block");
        assert_eq!(p, CommitPolicy::default());
    }

    #[test]
    fn a_policy_that_says_something_is_read() {
        let p: CommitPolicy =
            serde_json::from_str(r#"{"subject_max_chars": 50, "fix_requires_test": "error"}"#)
                .expect("a block");
        assert_eq!(p.subject_max_chars, 50);
        assert_eq!(p.fix_requires_test, FindingLevel::Error);
        // and what it did not say keeps the default
        assert_eq!(p.reference_unresolved, FindingLevel::Error);
    }

    #[test]
    fn off_reports_nothing_and_every_other_level_has_a_severity() {
        assert_eq!(FindingLevel::Off.severity(), None);
        assert_eq!(FindingLevel::Info.severity(), Some(Severity::Info));
        assert_eq!(FindingLevel::Warning.severity(), Some(Severity::Warning));
        assert_eq!(FindingLevel::Error.severity(), Some(Severity::Error));
    }

    #[test]
    fn the_prefixes_git_writes_are_recognised_and_nothing_else_is() {
        for s in [
            "Merge branch 'master' into feature/x",
            "Merge pull request #1 from a/b",
            "Revert \"feat(x): a thing\"",
            "fixup! feat(x): a thing",
            "squash! feat(x): a thing",
        ] {
            assert!(git_authored(s), "{s}");
        }
        for s in [
            "feat(x): a thing",
            "Merged the two branches by hand",
            "revert(x): undo the thing",
        ] {
            assert!(!git_authored(s), "{s}");
        }
    }
}
