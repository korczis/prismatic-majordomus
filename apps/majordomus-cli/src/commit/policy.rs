//! What this repository holds a commit to, as data.
//!
//! The conventional type vocabulary is deliberately *not* here: [`crate::release::ChangeKind`]
//! already is the list of words a conventional commit's type may be, the changelog already
//! renders from it, and a second copy under a `types:` key would be exactly the duplication
//! this subsystem exists to remove. What is here is what a repository can reasonably decide
//! differently: the grammar its history is written in ([`CommitGrammar`], conventional unless
//! declared, whose keyword style carries its own words because no other table holds them), a
//! subject width, and how loudly to report the things that are findings in one project and
//! noise in another.
//!
//! The block is read from `.ai/repo/policy.yaml`, the canonical policy this repository
//! already has, and every field has the default this repository's own history satisfies, so
//! a policy that says nothing about commits still yields the same verdict.
//!
//! ```
//! use majordomus_cli::commit::CommitPolicy;
//! let p = CommitPolicy::default();
//! assert_eq!(p.max_chars(), Some(72));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::grammar::CommitGrammar;
use crate::model::Severity;

/// How loudly one judgement is reported, including not at all.
///
/// `Off` is a value and not an absence: a repository that has decided a check does not
/// apply to it has made a decision, and a decision is worth being able to read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
/// ```
/// use majordomus_cli::commit::FindingLevel;
/// // a repository that has turned a check off has made a decision, and it reads back
/// let off: FindingLevel = serde_json::from_str("\"off\"").expect("a level");
/// assert_eq!(off, FindingLevel::Off);
/// assert_eq!(FindingLevel::default(), FindingLevel::Warning);
/// ```
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
    /// ```
    /// use majordomus_cli::commit::FindingLevel;
    /// use majordomus_cli::Severity;
    /// assert_eq!(FindingLevel::Off.severity(), None);
    /// assert_eq!(FindingLevel::Error.severity(), Some(Severity::Error));
    /// ```
    pub fn severity(self) -> Option<Severity> {
        match self {
            FindingLevel::Off => None,
            FindingLevel::Info => Some(Severity::Info),
            FindingLevel::Warning => Some(Severity::Warning),
            FindingLevel::Error => Some(Severity::Error),
        }
    }
}

/// `commit:` in the repository policy: the grammar, the subject width, and how loudly to
/// report each judgement a repository can reasonably decide differently about. The
/// conventional type vocabulary is deliberately absent — it is [`crate::release::ChangeKind`],
/// which the changelog already renders from, and a second copy of those words here is the
/// duplication this subsystem exists to remove.
///
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// ```
/// use majordomus_cli::commit::{CommitPolicy, FindingLevel};
/// // a policy that says nothing is the default this repository's history already satisfies
/// let quiet: CommitPolicy = serde_json::from_str("{}").expect("an empty block");
/// assert_eq!(quiet, CommitPolicy::default());
/// assert_eq!(quiet.max_chars(), Some(72));
/// // and one that says something keeps the defaults it did not mention
/// let narrow: CommitPolicy =
///     serde_json::from_str(r#"{"subject_max_chars": 50}"#).expect("a block");
/// assert_eq!(narrow.max_chars(), Some(50));
/// assert_eq!(narrow.reference_unresolved, FindingLevel::Error);
/// // 0 is no width at all
/// let wide: CommitPolicy = serde_json::from_str(r#"{"subject_max_chars": 0}"#).unwrap();
/// assert_eq!(wide.max_chars(), None);
/// ```
pub struct CommitPolicy {
    /// The grammar a header is written in. Conventional when absent.
    #[serde(default)]
    pub grammar: CommitGrammar,
    /// The widest a subject line may be; `0` is no limit. Absent, it is the grammar's
    /// default: 72 for the conventional grammar, which is what a terminal and every git viewer
    /// show without wrapping, and no limit for the keyword style, whose upstream checker sets
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_max_chars: Option<usize>,
    /// A subject that ends in a full stop. Absent, an error under the conventional grammar (a
    /// subject is a title) and off under the keyword style, whose own examples end in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_trailing_period: Option<FindingLevel>,
    /// A character outside printable ASCII (0x20 to 0x7F) on any line, a tab included and
    /// DEL allowed, as the keyword style's upstream checker counts them. Absent, off under
    /// the conventional grammar and an error under the keyword style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_ascii: Option<FindingLevel>,
    /// A line that ends in whitespace, a carriage return included. Absent, off under the
    /// conventional grammar and an error under the keyword style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trailing_whitespace: Option<FindingLevel>,
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
    /// A message that names no record id the layer holds (under the keyword style, a
    /// header's `#N` counts). Off by default: this repository lets a commit stand on its own,
    /// while an adopter whose plan binds every change to an issue sets it to `error` and has
    /// the hook ask for the id before the commit exists.
    #[serde(default = "off")]
    pub reference_required: FindingLevel,
    /// A closing keyword — `Closes #12`, `fixes #3` — anywhere in the message. Off by
    /// default; a repository that projects its plan onto a forge sets it to `error`, because
    /// a forge closes the issue the moment such a commit lands, and the layer, not a merged
    /// commit, decides when an issue is done.
    #[serde(default = "off")]
    pub closing_keyword: FindingLevel,
}

fn error() -> FindingLevel {
    FindingLevel::Error
}

fn off() -> FindingLevel {
    FindingLevel::Off
}

impl Default for CommitPolicy {
    fn default() -> Self {
        CommitPolicy {
            grammar: CommitGrammar::Conventional,
            subject_max_chars: None,
            subject_trailing_period: None,
            non_ascii: None,
            trailing_whitespace: None,
            scope_unknown: FindingLevel::Warning,
            fix_requires_test: FindingLevel::Warning,
            reference_unresolved: FindingLevel::Error,
            breaking_unexplained: FindingLevel::Error,
            reference_required: FindingLevel::Off,
            closing_keyword: FindingLevel::Off,
        }
    }
}

impl CommitPolicy {
    /// The subject width this policy holds a header to, or `None` for no limit.
    /// ```
    /// use majordomus_cli::commit::CommitPolicy;
    /// let k: CommitPolicy = serde_json::from_str(
    ///     r#"{"grammar": {"style": "keyword", "words": [{"word": "Fix", "kind": "fix"}]}}"#).unwrap();
    /// assert_eq!(k.max_chars(), None, "the keyword style sets no width unless asked");
    /// ```
    pub fn max_chars(&self) -> Option<usize> {
        match self.subject_max_chars {
            Some(0) => None,
            Some(n) => Some(n),
            None if self.grammar.is_keyword() => None,
            None => Some(72),
        }
    }

    /// How loudly a trailing full stop is reported.
    pub fn trailing_period(&self) -> FindingLevel {
        self.subject_trailing_period
            .unwrap_or(if self.grammar.is_keyword() {
                FindingLevel::Off
            } else {
                FindingLevel::Error
            })
    }

    /// How loudly a character outside printable ASCII is reported.
    pub fn non_ascii_level(&self) -> FindingLevel {
        self.non_ascii.unwrap_or(if self.grammar.is_keyword() {
            FindingLevel::Error
        } else {
            FindingLevel::Off
        })
    }

    /// How loudly a line that ends in whitespace is reported.
    pub fn trailing_whitespace_level(&self) -> FindingLevel {
        self.trailing_whitespace
            .unwrap_or(if self.grammar.is_keyword() {
                FindingLevel::Error
            } else {
                FindingLevel::Off
            })
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
        assert_eq!(p.max_chars(), Some(50));
        assert_eq!(p.fix_requires_test, FindingLevel::Error);
        // and what it did not say keeps the default
        assert_eq!(p.reference_unresolved, FindingLevel::Error);
    }

    #[test]
    fn the_new_levels_take_the_grammars_default_until_the_policy_says_otherwise() {
        let c = CommitPolicy::default();
        assert_eq!(
            (
                c.max_chars(),
                c.trailing_period(),
                c.non_ascii_level(),
                c.trailing_whitespace_level()
            ),
            (
                Some(72),
                FindingLevel::Error,
                FindingLevel::Off,
                FindingLevel::Off
            ),
            "the conventional defaults are what they were"
        );
        let k: CommitPolicy = serde_json::from_str(
            r#"{"grammar": {"style": "keyword", "words": [{"word": "Fix", "kind": "fix"}]}}"#,
        )
        .unwrap();
        assert_eq!(
            (
                k.max_chars(),
                k.trailing_period(),
                k.non_ascii_level(),
                k.trailing_whitespace_level()
            ),
            (
                None,
                FindingLevel::Off,
                FindingLevel::Error,
                FindingLevel::Error
            ),
            "the keyword style's defaults are its upstream checker's"
        );
        let said: CommitPolicy = serde_json::from_str(
            r#"{"grammar": {"style": "keyword", "words": [{"word": "Fix", "kind": "fix"}]},
                "subject_max_chars": 80, "subject_trailing_period": "warning",
                "non_ascii": "off", "trailing_whitespace": "info"}"#,
        )
        .unwrap();
        assert_eq!(
            (
                said.max_chars(),
                said.trailing_period(),
                said.non_ascii_level(),
                said.trailing_whitespace_level()
            ),
            (
                Some(80),
                FindingLevel::Warning,
                FindingLevel::Off,
                FindingLevel::Info
            ),
            "what a policy says wins over every default"
        );
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
