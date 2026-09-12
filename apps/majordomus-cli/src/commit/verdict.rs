//! The judge: one commit message, against the policy, with every finding it earns.
//!
//! # Why this exists at all
//!
//! `project.conventional-commits` was a rule whose failure behaviour read *"No command
//! decides this rule; a reviewer does"*. That is a rule this repository states and cannot
//! check, which is the failure mode the whole governance layer exists to prevent — and the
//! parser that would have decided it was already here, reading the history for the
//! changelog. What was missing was not a grammar. It was a verdict.
//!
//! # What it judges, and what it refuses to
//!
//! The findings below are the ones a machine can decide from evidence that is present at
//! the moment a commit is written: the grammar, the width, whether a scope is one this
//! repository uses, whether a record the message names exists, whether a fix carries a test,
//! whether a breaking change says why. It does not judge whether the subject is a good
//! sentence, whether the change is atomic, or whether the work was worth doing; a reviewer
//! decides those, and a validator that pretended to would be a validator nobody trusts.
//!
//! ```
//! use majordomus_cli::commit::{judge, CommitPolicy, CommitSubject};
//! use majordomus_cli::model::Severity;
//!
//! let v = judge(&CommitSubject::of("update stuff"), &CommitPolicy::default());
//! assert!(!v.passed);
//! assert_eq!(v.findings[0].code, "commit.not_conventional");
//!
//! // and git's own subjects are exempt rather than wrong
//! let m = judge(&CommitSubject::of("Merge pull request #249 from korczis/fix"), &CommitPolicy::default());
//! assert!(m.passed);
//! assert!(m.exempt.is_some());
//! let _ = Severity::Error;
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::policy::{git_authored, CommitPolicy, FindingLevel};
use super::CommitMessage;
use crate::model::{Diagnostic, Severity};
use crate::release::commits::record_candidates;
use crate::release::ChangeKind;

/// Every code this module can produce, as one closed set.
///
/// A judgement is a value here and not a string at the call site. The codes reach a person
/// through a diagnostic, a machine through JSON and a hook through an exit code, and all
/// three read the same list — which is what makes a finding something a rule document can
/// name and a test can assert on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommitRule {
    /// The header is not `type(scope): subject`.
    NotConventional,
    /// The subject is wider than the policy allows.
    SubjectTooLong,
    /// The header parsed but said nothing after the colon.
    SubjectEmpty,
    /// The subject ends in a full stop; a subject is a title, not a sentence.
    SubjectTrailingPeriod,
    /// The scope is not one this repository's history or layout yields.
    UnknownScope,
    /// A `fix` whose staged files include no test.
    FixWithoutTest,
    /// The message names a record id the layer does not hold.
    UnresolvedReference,
    /// The commit declares itself breaking and explains nothing.
    BreakingUnexplained,
}

impl CommitRule {
    /// The stable code a diagnostic carries.
    ///
    /// ```
    /// use majordomus_cli::commit::CommitRule;
    /// assert_eq!(CommitRule::SubjectTooLong.code(), "commit.subject_too_long");
    /// ```
    pub fn code(self) -> &'static str {
        match self {
            CommitRule::NotConventional => "commit.not_conventional",
            CommitRule::SubjectTooLong => "commit.subject_too_long",
            CommitRule::SubjectEmpty => "commit.subject_empty",
            CommitRule::SubjectTrailingPeriod => "commit.subject_trailing_period",
            CommitRule::UnknownScope => "commit.unknown_scope",
            CommitRule::FixWithoutTest => "commit.fix_without_test",
            CommitRule::UnresolvedReference => "commit.unresolved_reference",
            CommitRule::BreakingUnexplained => "commit.breaking_unexplained",
        }
    }

    /// Every rule, in the order a verdict reports them. Ordered by how early in the message
    /// the evidence for them appears, so a person reads findings in the order they would
    /// fix them.
    pub const ALL: &'static [CommitRule] = &[
        CommitRule::NotConventional,
        CommitRule::SubjectEmpty,
        CommitRule::SubjectTooLong,
        CommitRule::SubjectTrailingPeriod,
        CommitRule::UnknownScope,
        CommitRule::BreakingUnexplained,
        CommitRule::UnresolvedReference,
        CommitRule::FixWithoutTest,
    ];
}

/// Why a commit was not judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommitExemption {
    /// git composed the subject: a merge, a revert, a fixup.
    GitAuthored,
}

/// Everything the judge is allowed to look at.
///
/// A struct rather than five arguments, because the two halves that are optional are
/// exactly the two a caller may not have: a hook running on `commit-msg` knows the staged
/// files, a gate reading history does not, and a judge that demanded both would have to be
/// two judges.
#[derive(Debug, Clone, Default)]
pub struct CommitSubject {
    /// The message as it will be stored.
    pub text: String,
    /// The scope vocabulary this repository yields, when it has been derived.
    pub scopes: Option<Vec<String>>,
    /// The paths this commit will contain, when they are known.
    pub paths: Option<Vec<String>>,
    /// The record ids the layer holds, for resolving what the message names.
    pub records: Vec<String>,
}

impl CommitSubject {
    /// A subject that is only a message: the judgements that need no other evidence.
    pub fn of(text: &str) -> CommitSubject {
        CommitSubject {
            text: text.to_string(),
            ..CommitSubject::default()
        }
    }
}

/// What the judge concluded.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommitVerdict {
    /// The message as parsed; what was judged, not what was typed.
    pub message: CommitMessage,
    /// Whether nothing of error severity was found.
    pub passed: bool,
    /// Why the commit was not judged, when it was not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exempt: Option<CommitExemption>,
    /// Every finding, in [`CommitRule::ALL`] order.
    pub findings: Vec<Diagnostic>,
}

/// Whether a path is a test of this repository.
///
/// Derived from the shapes this repository actually uses rather than from a list a person
/// maintains: a path under a `test`/`tests` directory, a Rust `#[cfg(test)]` file named
/// `tests.rs`, or a name that says so.
fn is_test_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.split('/')
        .any(|seg| seg == "test" || seg == "tests" || seg == "spec")
        || p.contains("_test.")
        || p.contains(".test.")
        || p.contains("_spec.")
        || p.ends_with("/tests.rs")
}

/// Judge one commit.
pub fn judge(subject: &CommitSubject, policy: &CommitPolicy) -> CommitVerdict {
    let message = CommitMessage::parse(&subject.text);
    let head = &message.header;

    if git_authored(&head.subject) || git_authored(subject.text.lines().next().unwrap_or_default())
    {
        return CommitVerdict {
            message,
            passed: true,
            exempt: Some(CommitExemption::GitAuthored),
            findings: Vec::new(),
        };
    }

    let mut found: Vec<(CommitRule, Severity, String)> = Vec::new();
    let mut add = |rule: CommitRule, level: FindingLevel, message: String| {
        if let Some(severity) = level.severity() {
            found.push((rule, severity, message));
        }
    };

    if !head.is_conventional() {
        let detail = if head.word.is_empty() {
            format!(
                "'{}' is not `type(scope): subject`; the types are {}",
                head.subject.chars().take(60).collect::<String>(),
                ChangeKind::WORDS.join(", ")
            )
        } else {
            format!(
                "'{}' is not a commit type; the types are {}",
                head.word,
                ChangeKind::WORDS.join(", ")
            )
        };
        add(CommitRule::NotConventional, FindingLevel::Error, detail);
        // Everything below reads fields this parse did not produce; reporting them as well
        // would bury the one finding that matters under consequences of it.
        return finish(message, found);
    }

    if head.subject.trim().is_empty() {
        add(
            CommitRule::SubjectEmpty,
            FindingLevel::Error,
            "the header says nothing after the colon".into(),
        );
    }
    let width = head.render().chars().count();
    if width > policy.subject_max_chars {
        add(
            CommitRule::SubjectTooLong,
            FindingLevel::Error,
            format!(
                "the subject is {width} characters; the policy allows {}",
                policy.subject_max_chars
            ),
        );
    }
    if head.subject.ends_with('.') {
        add(
            CommitRule::SubjectTrailingPeriod,
            FindingLevel::Error,
            "the subject ends in a full stop; a subject is a title, not a sentence".into(),
        );
    }
    if let (Some(scope), Some(known)) = (head.scope.as_deref(), subject.scopes.as_deref()) {
        if !known.iter().any(|k| k == scope) {
            add(
                CommitRule::UnknownScope,
                policy.scope_unknown,
                format!(
                    "'{scope}' is not a scope this repository uses; `majordomus commit scopes` lists them"
                ),
            );
        }
    }
    if message.breaking() && message.body.trim().is_empty() && message.trailer("BREAKING CHANGE").is_none()
    {
        add(
            CommitRule::BreakingUnexplained,
            policy.breaking_unexplained,
            "the commit is marked breaking and explains nothing; write a body or a BREAKING CHANGE trailer".into(),
        );
    }
    for candidate in record_candidates(&subject.text) {
        if !subject.records.iter().any(|r| *r == candidate) {
            add(
                CommitRule::UnresolvedReference,
                policy.reference_unresolved,
                format!("the message names {candidate}, which this repository's layer does not hold"),
            );
        }
    }
    if head.kind == ChangeKind::Fix {
        if let Some(paths) = subject.paths.as_deref() {
            if !paths.is_empty() && !paths.iter().any(|p| is_test_path(p)) {
                add(
                    CommitRule::FixWithoutTest,
                    policy.fix_requires_test,
                    "a fix with no test among its files; a bug that can come back is a bug with no regression test".into(),
                );
            }
        }
    }

    finish(message, found)
}

/// Order the findings and decide the verdict.
fn finish(message: CommitMessage, mut found: Vec<(CommitRule, Severity, String)>) -> CommitVerdict {
    found.sort_by_key(|(rule, _, _)| {
        CommitRule::ALL
            .iter()
            .position(|r| r == rule)
            .unwrap_or(usize::MAX)
    });
    let passed = !found.iter().any(|(_, s, _)| *s == Severity::Error);
    CommitVerdict {
        message,
        passed,
        exempt: None,
        findings: found
            .into_iter()
            .map(|(rule, severity, msg)| Diagnostic {
                severity,
                code: rule.code().to_string(),
                path: None,
                message: msg,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(v: &CommitVerdict) -> Vec<&str> {
        v.findings.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn a_well_formed_commit_passes_with_nothing_to_say() {
        let v = judge(
            &CommitSubject::of("feat(commit): the rule is decided by a command"),
            &CommitPolicy::default(),
        );
        assert!(v.passed, "{:?}", v.findings);
        assert!(v.findings.is_empty());
        assert!(v.exempt.is_none());
    }

    #[test]
    fn a_subject_that_is_not_conventional_is_one_finding_and_not_five() {
        // The consequences of an unparsed header are not findings of their own: a person
        // fixing `update stuff` does not need to be told its scope is unknown as well.
        let v = judge(&CommitSubject::of("update stuff"), &CommitPolicy::default());
        assert!(!v.passed);
        assert_eq!(codes(&v), vec!["commit.not_conventional"]);
    }

    #[test]
    fn git_composes_subjects_that_no_convention_applies_to() {
        for s in [
            "Merge pull request #249 from korczis/fix/pipelines",
            "Revert \"feat(x): a thing\"",
            "fixup! feat(x): a thing",
        ] {
            let v = judge(&CommitSubject::of(s), &CommitPolicy::default());
            assert!(v.passed, "{s}");
            assert_eq!(v.exempt, Some(CommitExemption::GitAuthored), "{s}");
            assert!(v.findings.is_empty(), "{s}");
        }
    }

    #[test]
    fn a_wide_subject_is_measured_whole_and_not_only_after_the_colon() {
        let long = "x".repeat(80);
        let v = judge(&CommitSubject::of(&format!("feat(a): {long}")), &CommitPolicy::default());
        assert_eq!(codes(&v), vec!["commit.subject_too_long"]);
        assert!(!v.passed);
        // the width reported is the whole line, which is what a viewer wraps
        assert!(v.findings[0].message.contains("89 characters"));
    }

    #[test]
    fn a_scope_outside_the_vocabulary_is_a_warning_and_still_passes() {
        let subject = CommitSubject {
            text: "feat(unheard-of): a new subsystem".into(),
            scopes: Some(vec!["commit".into(), "http".into()]),
            ..CommitSubject::default()
        };
        let v = judge(&subject, &CommitPolicy::default());
        assert_eq!(codes(&v), vec!["commit.unknown_scope"]);
        // inference must not refuse the first commit of a subsystem it has never seen
        assert!(v.passed);
        assert_eq!(v.findings[0].severity, Severity::Warning);
    }

    #[test]
    fn a_scope_in_the_vocabulary_says_nothing() {
        let subject = CommitSubject {
            text: "feat(commit): a thing".into(),
            scopes: Some(vec!["commit".into()]),
            ..CommitSubject::default()
        };
        assert!(judge(&subject, &CommitPolicy::default()).findings.is_empty());
    }

    #[test]
    fn a_reference_the_layer_does_not_hold_fails_and_one_it_holds_does_not() {
        let held = CommitSubject {
            text: "fix(plan): a thing under I1305".into(),
            records: vec!["I1305".into()],
            ..CommitSubject::default()
        };
        assert!(judge(&held, &CommitPolicy::default()).findings.is_empty());

        let invented = CommitSubject {
            text: "fix(plan): a thing under I9999".into(),
            records: vec!["I1305".into()],
            ..CommitSubject::default()
        };
        let v = judge(&invented, &CommitPolicy::default());
        assert_eq!(codes(&v), vec!["commit.unresolved_reference"]);
        assert!(!v.passed);
    }

    #[test]
    fn a_fix_is_asked_for_a_test_only_when_its_files_are_known() {
        let with_test = CommitSubject {
            text: "fix(commit): a thing".into(),
            paths: Some(vec!["src/commit/verdict.rs".into(), "test/cases/1.sh".into()]),
            ..CommitSubject::default()
        };
        assert!(judge(&with_test, &CommitPolicy::default()).findings.is_empty());

        let without = CommitSubject {
            text: "fix(commit): a thing".into(),
            paths: Some(vec!["src/commit/verdict.rs".into()]),
            ..CommitSubject::default()
        };
        let v = judge(&without, &CommitPolicy::default());
        assert_eq!(codes(&v), vec!["commit.fix_without_test"]);
        assert!(v.passed, "a warning by default, so that a gate over history does not rewrite it");

        // a gate reading history knows no paths, and must not invent the finding
        let unknown = CommitSubject::of("fix(commit): a thing");
        assert!(judge(&unknown, &CommitPolicy::default()).findings.is_empty());
    }

    #[test]
    fn a_fix_that_only_touches_tests_is_covered_by_them() {
        let only_tests = CommitSubject {
            text: "fix(commit): a thing".into(),
            paths: Some(vec!["apps/majordomus-cli/src/commit/tests.rs".into()]),
            ..CommitSubject::default()
        };
        assert!(judge(&only_tests, &CommitPolicy::default()).findings.is_empty());
    }

    #[test]
    fn a_breaking_change_must_say_why() {
        let bare = judge(&CommitSubject::of("feat(api)!: the route moved"), &CommitPolicy::default());
        assert_eq!(codes(&bare), vec!["commit.breaking_unexplained"]);
        assert!(!bare.passed);

        let explained = judge(
            &CommitSubject::of("feat(api)!: the route moved\n\nIt is now under /api/v2."),
            &CommitPolicy::default(),
        );
        assert!(explained.findings.is_empty());

        let trailer = judge(
            &CommitSubject::of("feat(api): the route moved\n\nBREAKING CHANGE: it is under /api/v2"),
            &CommitPolicy::default(),
        );
        assert!(trailer.findings.is_empty());
    }

    #[test]
    fn a_level_of_off_reports_nothing_at_all() {
        let policy = CommitPolicy {
            reference_unresolved: FindingLevel::Off,
            ..CommitPolicy::default()
        };
        let invented = CommitSubject {
            text: "fix(plan): a thing under I9999".into(),
            ..CommitSubject::default()
        };
        let v = judge(&invented, &policy);
        assert!(v.findings.is_empty());
        assert!(v.passed);
    }

    #[test]
    fn findings_are_ordered_by_where_their_evidence_is() {
        let subject = CommitSubject {
            text: format!("fix(nope)!: {}.", "x".repeat(80)),
            scopes: Some(vec!["commit".into()]),
            ..CommitSubject::default()
        };
        let v = judge(&subject, &CommitPolicy::default());
        assert_eq!(
            codes(&v),
            vec![
                "commit.subject_too_long",
                "commit.subject_trailing_period",
                "commit.unknown_scope",
                "commit.breaking_unexplained",
            ]
        );
    }

    #[test]
    fn a_test_path_is_recognised_by_the_shapes_this_repository_uses() {
        for p in [
            "test/cases/01_init.sh",
            "tests/integration.rs",
            "apps/majordomus-cli/src/commit/tests.rs",
            "lib/foo_test.exs",
            "site/a.test.js",
        ] {
            assert!(is_test_path(p), "{p}");
        }
        for p in ["src/commit/verdict.rs", "docs/COMMIT.md", "latest.rs"] {
            assert!(!is_test_path(p), "{p}");
        }
    }
}

/// One commit of the history, judged.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JudgedCommit {
    /// The commit name, as git gave it.
    pub commit: String,
    /// Its subject line.
    pub subject: String,
    /// Whether it passed.
    pub passed: bool,
    /// Why it was not judged, when it was not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exempt: Option<CommitExemption>,
    /// Every finding.
    pub findings: Vec<Diagnostic>,
}

/// A range of history, judged.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HistoryReport {
    /// The range as asked for.
    pub range: String,
    /// How many commits were read.
    pub total: usize,
    /// How many were exempt because git composed their subject.
    pub exempt: usize,
    /// How many carry a finding of error severity.
    pub failing: usize,
    /// Every commit that carries a finding, newest first. A commit with nothing to say is
    /// counted and not listed: a report that printed a thousand passing commits would be a
    /// report nobody reads.
    pub commits: Vec<JudgedCommit>,
}
