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
/// ```
/// use majordomus_cli::commit::CommitRule;
/// // every rule has a stable code, and the order is the order a person would fix them in
/// assert_eq!(CommitRule::NotConventional.code(), "commit.not_conventional");
/// assert_eq!(CommitRule::ALL.first(), Some(&CommitRule::ControlCharacter));
/// assert!(CommitRule::ALL.iter().all(|r| r.code().starts_with("commit.")));
/// ```
pub enum CommitRule {
    /// The message carries a control character: C0 other than a line feed or a tab, DEL, or
    /// C1. Judged before the grammar, because a colour code in front of the type is a
    /// control character and not a word that fails to be a type (I4605).
    ControlCharacter,
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
            CommitRule::ControlCharacter => "commit.control_character",
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
        CommitRule::ControlCharacter,
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
/// ```
/// use majordomus_cli::commit::{judge, CommitExemption, CommitPolicy, CommitSubject};
/// let v = judge(&CommitSubject::of("Merge pull request #1 from a/b"), &CommitPolicy::default());
/// assert_eq!(v.exempt, Some(CommitExemption::GitAuthored));
/// ```
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
/// ```
/// use majordomus_cli::commit::{judge, CommitPolicy, CommitSubject};
/// // the two optional halves are the two a caller may not have: a hook knows the staged
/// // files, a gate reading history does not
/// let hook = CommitSubject {
///     text: "fix(a): a thing".into(),
///     paths: Some(vec!["src/a.rs".into()]),
///     ..CommitSubject::default()
/// };
/// assert_eq!(judge(&hook, &CommitPolicy::default()).findings.len(), 1);
/// let gate = CommitSubject::of("fix(a): a thing");
/// assert!(judge(&gate, &CommitPolicy::default()).findings.is_empty());
/// ```
pub struct CommitSubject {
    /// The message as it will be stored.
    pub text: String,
    /// The scope vocabulary this repository yields, when it has been derived.
    pub scopes: Option<Vec<String>>,
    /// The paths this commit will contain, when they are known.
    pub paths: Option<Vec<String>>,
    /// The record ids the layer holds, for resolving what the message names.
    pub records: Vec<String>,
    /// What of `text` git stores, which is what is judged as text: the hook's input under
    /// the repository's cleanup, or every line of a commit already made.
    pub stored: super::message::Stored,
}

impl CommitSubject {
    /// A subject that is only a message: the judgements that need no other evidence.
    /// ```
    /// use majordomus_cli::commit::CommitSubject;
    /// let s = CommitSubject::of("feat(a): a thing");
    /// assert!(s.paths.is_none(), "a judgement needing paths is not made rather than guessed");
    /// assert!(s.scopes.is_none());
    /// ```
    pub fn of(text: &str) -> CommitSubject {
        CommitSubject {
            text: text.to_string(),
            ..CommitSubject::default()
        }
    }
}

/// What the judge concluded: the message as it was parsed, whether anything of error
/// severity was found, the exemption when git composed the subject rather than a person, and
/// every finding in [`CommitRule::ALL`] order — which is the order the evidence for them
/// appears in the message, so a person reads them in the order they would fix them.
///
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// ```
/// use majordomus_cli::commit::{judge, CommitPolicy, CommitSubject, CommitVerdict};
/// let v: CommitVerdict = judge(&CommitSubject::of("update stuff"), &CommitPolicy::default());
/// assert!(!v.passed);
/// assert_eq!(v.findings[0].code, "commit.not_conventional");
/// assert_eq!(v.message.header.subject, "update stuff", "what was judged, not what was typed");
/// ```
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
/// Judge one commit against `policy`.
///
/// ```
/// use majordomus_cli::commit::{judge, CommitPolicy, CommitSubject};
/// let p = CommitPolicy::default();
/// // a well-formed subject has nothing said about it
/// assert!(judge(&CommitSubject::of("feat(commit): a subject"), &p).findings.is_empty());
/// // an id the layer does not hold is an error, because nothing may be invented
/// let invented = CommitSubject { text: "fix(a): under I9999".into(), ..CommitSubject::default() };
/// assert!(!judge(&invented, &p).passed);
/// ```
pub fn judge(subject: &CommitSubject, policy: &CommitPolicy) -> CommitVerdict {
    let message = CommitMessage::parse_stored(&subject.text, &subject.stored);
    let head = &message.header;

    // What bytes a message may hold is judged first and for every message, git's own
    // subjects included: their exemption is from the grammar, not from being text.
    let mut found: Vec<(CommitRule, Severity, String)> = Vec::new();
    let control = control_characters(&subject.text, &subject.stored);
    if let (Some(control), Some(severity)) = (&control, policy.control_character.severity()) {
        found.push((
            CommitRule::ControlCharacter,
            severity,
            control.detail.clone(),
        ));
    }
    // Only a control code in the header corrupts what the grammar reads; one in the body
    // leaves every other judgement sound, and those findings are still owed.
    let header_refused = control.as_ref().is_some_and(|c| c.in_header)
        && found.iter().any(|(_, s, _)| *s == Severity::Error);

    if git_authored(&head.subject) || git_authored(subject.text.lines().next().unwrap_or_default())
    {
        if found.is_empty() {
            return CommitVerdict {
                message,
                passed: true,
                exempt: Some(CommitExemption::GitAuthored),
                findings: Vec::new(),
            };
        }
        let mut verdict = finish(message, found);
        verdict.exempt = Some(CommitExemption::GitAuthored);
        return verdict;
    }
    if header_refused {
        // The grammar below would read the control code as part of a word and report the
        // consequence (`'\x1b[33mfix' is not a commit type`) instead of the cause.
        return finish(message, found);
    }

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
    if message.breaking()
        && message.body.trim().is_empty()
        && message.trailer("BREAKING CHANGE").is_none()
    {
        add(
            CommitRule::BreakingUnexplained,
            policy.breaking_unexplained,
            "the commit is marked breaking and explains nothing; write a body or a BREAKING CHANGE trailer".into(),
        );
    }
    for candidate in record_candidates(&without_code_spans(&subject.text)) {
        if !subject.records.contains(&candidate) {
            add(
                CommitRule::UnresolvedReference,
                policy.reference_unresolved,
                format!(
                    "the message names {candidate}, which this repository's layer does not hold"
                ),
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

/// How many positions a control-character finding names before it only counts the rest.
const CONTROL_POSITIONS_SHOWN: usize = 5;

/// Whether a character is a control character a commit message may not hold: C0 other than
/// a tab (a line feed never reaches here, it separates the lines), DEL, or C1.
fn is_control(ch: char) -> bool {
    ch != '\t' && (ch < ' ' || ('\u{7f}'..='\u{9f}').contains(&ch))
}

/// The code point and its name, as a person looks it up: `U+001B ESCAPE`.
fn control_name(ch: char) -> String {
    let name = match ch {
        '\u{0}' => "NULL",
        '\u{7}' => "BELL",
        '\u{8}' => "BACKSPACE",
        '\u{b}' => "LINE TABULATION",
        '\u{c}' => "FORM FEED",
        '\r' => "CARRIAGE RETURN",
        '\u{1b}' => "ESCAPE",
        '\u{7f}' => "DELETE",
        '\u{9b}' => "CONTROL SEQUENCE INTRODUCER",
        c if c < ' ' => "C0 CONTROL",
        _ => "C1 CONTROL",
    };
    format!("U+{:04X} {name}", ch as u32)
}

/// The control characters a message carries: one detail naming each one's line, column and
/// code point, and whether the header line carries one.
struct ControlCharacters {
    detail: String,
    in_header: bool,
}

/// Every control character in the lines git stores (under `strip` cleanup comments are not,
/// and a `git commit -v` diff never is), or `None` when the message is text. Lines are
/// numbered as in the text given and columns count code points, both from 1, so a finding
/// points at what the author sees in the editor.
///
/// A carriage return that ends a line is a line ending, not content: an editor that writes
/// CRLF has written plain text.
fn control_characters(text: &str, stored: &super::message::Stored) -> Option<ControlCharacters> {
    let mut shown: Vec<String> = Vec::new();
    let mut count = 0usize;
    let mut in_header = false;
    for (i, (n, line)) in super::message::stored_lines(text, stored)
        .into_iter()
        .enumerate()
    {
        for (column, ch) in line.chars().enumerate() {
            if is_control(ch) {
                count += 1;
                in_header |= i == 0;
                if shown.len() < CONTROL_POSITIONS_SHOWN {
                    shown.push(format!(
                        "line {n}, column {}: {}",
                        column + 1,
                        control_name(ch)
                    ));
                }
            }
        }
    }
    if count == 0 {
        return None;
    }
    let more = match count - shown.len() {
        0 => String::new(),
        rest => format!("; and {rest} more"),
    };
    let plural = if count == 1 { "" } else { "s" };
    Some(ControlCharacters {
        detail: format!(
            "the message carries {count} control character{plural}, which every later reader \
             shows raw (a pager's colour codes, typically); write plain text: {}{more}",
            shown.join("; ")
        ),
        in_header,
    })
}

/// A text to print at a terminal with every control character written as an escape
/// (`\u{1b}`), so that a message under judgement cannot move the cursor or clear the screen
/// of the person reading the verdict.
///
/// ```
/// use majordomus_cli::commit::verdict::printable;
/// assert_eq!(printable("a \u{1b}[2Kb"), "a \\u{1b}[2Kb");
/// assert_eq!(printable("tab\tok"), "tab\tok");
/// ```
pub fn printable(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if is_control(ch) && ch != '\n' {
                format!("\\u{{{:x}}}", ch as u32)
            } else {
                ch.to_string()
            }
        })
        .collect()
}

/// Order the findings and decide the verdict.
fn finish(message: CommitMessage, found: Vec<(CommitRule, Severity, String)>) -> CommitVerdict {
    let mut ranked: Vec<RankedFinding> = found
        .into_iter()
        .map(|(rule, severity, message)| RankedFinding {
            rule,
            severity,
            message,
        })
        .collect();
    crate::order::canonical(&mut ranked);
    let found: Vec<(CommitRule, Severity, String)> = ranked
        .into_iter()
        .map(|f| (f.rule, f.severity, f.message))
        .collect();
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

/// A finding in the order a person reads them: by the rule's place in `CommitRule::ALL`,
/// then by what it says, so two findings under one rule never trade places between runs.
struct RankedFinding {
    rule: CommitRule,
    severity: Severity,
    message: String,
}

impl crate::order::Ordered for RankedFinding {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let position = CommitRule::ALL
            .iter()
            .position(|r| *r == self.rule)
            .map_or(i64::MAX, |p| p as i64);
        crate::order::OrderKey::plain(&self.message, &self.message).ranked(position)
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

    /// The message an adopter's GitLab titles carried: a pager's colour codes (I4605).
    #[test]
    fn an_escape_inside_the_subject_is_refused_and_named_by_its_position() {
        let v = judge(
            &CommitSubject::of("fix(commit): x \u{1b}[1mbold\u{1b}[m"),
            &CommitPolicy::default(),
        );
        assert!(!v.passed);
        assert_eq!(codes(&v), vec!["commit.control_character"]);
        let detail = &v.findings[0].message;
        assert!(detail.contains("2 control characters"), "{detail}");
        assert!(
            detail.contains("line 1, column 16: U+001B ESCAPE"),
            "{detail}"
        );
        assert!(
            detail.contains("line 1, column 24: U+001B ESCAPE"),
            "{detail}"
        );
    }

    #[test]
    fn a_control_code_before_the_type_is_itself_and_not_a_wrong_type() {
        // Before I4605 this was commit.not_conventional, "'\x1b[33mfix' is not a commit
        // type": the consequence reported, the cause never named.
        let v = judge(
            &CommitSubject::of("\u{1b}[33mfix(commit): x\u{1b}[m"),
            &CommitPolicy::default(),
        );
        assert!(!v.passed);
        assert_eq!(codes(&v), vec!["commit.control_character"]);
        assert!(v.findings[0]
            .message
            .contains("line 1, column 1: U+001B ESCAPE"));
    }

    #[test]
    fn c1_and_c0_codes_are_refused_anywhere_and_tabs_line_endings_are_text() {
        for text in [
            "fix(commit): x \u{9b}31m y",
            "fix(commit): x\n\nbody \u{1} here",
            "fix(commit): x\n\nbody \u{7f} here",
        ] {
            let v = judge(&CommitSubject::of(text), &CommitPolicy::default());
            assert_eq!(codes(&v), vec!["commit.control_character"], "{text:?}");
        }
        let body = judge(
            &CommitSubject::of("fix(commit): x\r\n\r\nbody\twith a tab\r\n\n\nand blank lines\n"),
            &CommitPolicy::default(),
        );
        assert!(body.findings.is_empty(), "{:?}", body.findings);
        let c1 = judge(
            &CommitSubject::of("fix(commit): x \u{9b}y"),
            &CommitPolicy::default(),
        );
        assert!(c1.findings[0]
            .message
            .contains("U+009B CONTROL SEQUENCE INTRODUCER"));
        // one carriage return before a line feed is the ending; a second is content
        let crcr = judge(
            &CommitSubject::of("fix(commit): x\r\r\nbody"),
            &CommitPolicy::default(),
        );
        assert!(
            crcr.findings[0]
                .message
                .contains("line 1, column 15: U+000D"),
            "{:?}",
            crcr.findings
        );
        // a carriage return inside a line is not a line ending
        let cr = judge(
            &CommitSubject::of("fix(commit): x\ry"),
            &CommitPolicy::default(),
        );
        assert!(cr.findings[0].message.contains("U+000D CARRIAGE RETURN"));
    }

    #[test]
    fn a_subject_git_composed_is_exempt_from_the_grammar_but_not_from_being_text() {
        let coloured = judge(
            &CommitSubject::of("Merge branch '\u{1b}[32mfeature\u{1b}[m'"),
            &CommitPolicy::default(),
        );
        assert!(!coloured.passed);
        assert_eq!(coloured.exempt, Some(CommitExemption::GitAuthored));
        assert_eq!(codes(&coloured), vec!["commit.control_character"]);
        let plain = judge(
            &CommitSubject::of("Merge branch 'feature'"),
            &CommitPolicy::default(),
        );
        assert!(plain.passed && plain.findings.is_empty());
        assert_eq!(plain.exempt, Some(CommitExemption::GitAuthored));
    }

    #[test]
    fn the_level_is_the_policy_s_like_every_other_commit_rule() {
        let text = "\u{1b}[33mfix(commit): x";
        let off = CommitPolicy {
            control_character: FindingLevel::Off,
            ..CommitPolicy::default()
        };
        // off: not judged, and the grammar reads the escape as part of the word, as before
        assert_eq!(
            codes(&judge(&CommitSubject::of(text), &off)),
            vec!["commit.not_conventional"]
        );
        let warning = CommitPolicy {
            control_character: FindingLevel::Warning,
            ..CommitPolicy::default()
        };
        let v = judge(&CommitSubject::of("fix(commit): x \u{1b}[1my"), &warning);
        assert!(v.passed);
        assert_eq!(codes(&v), vec!["commit.control_character"]);
        assert_eq!(v.findings[0].severity, Severity::Warning);
    }

    /// git strips comment lines and everything from the `git commit -v` scissors on, so
    /// neither is judged; the line numbers stay the editor's (I4605 review).
    #[test]
    fn what_git_does_not_store_is_not_judged() {
        let commented = "fix(commit): x\n\nbody\n# note: \u{1b}[33mwarning\u{1b}[m";
        let v = judge(&CommitSubject::of(commented), &CommitPolicy::default());
        assert!(v.findings.is_empty(), "{:?}", v.findings);
        let verbose = format!(
            "fix(commit): x\n\n# Please enter the commit message\n# {}\n# Do not modify\ndiff --git a/f b/f\n+\u{c}form feed\n",
            crate::commit::message::SCISSORS
        );
        let v = judge(&CommitSubject::of(&verbose), &CommitPolicy::default());
        assert!(v.findings.is_empty(), "{:?}", v.findings);
        let numbered = "fix(commit): x\n# a comment\n\nbody \u{1b} here";
        let v = judge(&CommitSubject::of(numbered), &CommitPolicy::default());
        assert!(
            v.findings[0].message.contains("line 4, column 6"),
            "{:?}",
            v.findings
        );
    }

    /// What is judged follows what git stores: comment lines are stored under `verbatim`,
    /// `whitespace` and `scissors` cleanup, and a commit already made is judged whole, even
    /// below a scissors line its author typed (I4605 review).
    #[test]
    fn what_is_judged_follows_the_cleanup_git_will_apply() {
        use crate::commit::message::Stored;
        let commented = "fix(commit): x\n\n# note: \u{1b}[33mwarning\u{1b}[m";
        let judged = |stored: Stored| {
            let subject = CommitSubject {
                text: commented.into(),
                stored,
                ..CommitSubject::default()
            };
            codes(&judge(&subject, &CommitPolicy::default()))
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        assert!(judged(Stored::StripComments("#".into())).is_empty());
        assert_eq!(
            judged(Stored::KeepComments("#".into())),
            vec!["commit.control_character"]
        );
        assert_eq!(judged(Stored::Everything), vec!["commit.control_character"]);
        // a comment prefix other than `#` is the repository's, and `#` is then text
        assert_eq!(
            judged(Stored::StripComments(";".into())),
            vec!["commit.control_character"]
        );
        let verbose = format!(
            "fix(commit): x\n# {}\n+\u{c}form feed",
            crate::commit::message::SCISSORS
        );
        let kept = CommitSubject {
            text: verbose.clone(),
            stored: Stored::KeepComments("#".into()),
            ..CommitSubject::default()
        };
        assert!(judge(&kept, &CommitPolicy::default()).findings.is_empty());
        let made = CommitSubject {
            text: verbose,
            stored: Stored::Everything,
            ..CommitSubject::default()
        };
        assert_eq!(
            codes(&judge(&made, &CommitPolicy::default())),
            vec!["commit.control_character"]
        );
    }

    /// A control code only in the body leaves the grammar sound, so its findings are still
    /// reported beside it (I4605 review).
    #[test]
    fn a_control_code_in_the_body_does_not_hide_the_grammar() {
        let v = judge(
            &CommitSubject::of("update stuff\n\nbody \u{1b} here"),
            &CommitPolicy::default(),
        );
        assert_eq!(
            codes(&v),
            vec!["commit.control_character", "commit.not_conventional"]
        );
    }

    #[test]
    fn a_subject_printed_at_a_terminal_cannot_drive_it() {
        assert_eq!(printable("a \u{1b}[2K\rb"), "a \\u{1b}[2K\\u{d}b");
        assert_eq!(printable("plain\ttext"), "plain\ttext");
    }

    #[test]
    fn many_control_characters_name_five_positions_and_count_the_rest() {
        let text = format!("fix(commit): {}", "\u{1b}[m".repeat(8));
        let v = judge(&CommitSubject::of(&text), &CommitPolicy::default());
        let detail = &v.findings[0].message;
        assert!(detail.contains("8 control characters"), "{detail}");
        assert_eq!(detail.matches("U+001B").count(), 5, "{detail}");
        assert!(detail.ends_with("; and 3 more"), "{detail}");
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
        let v = judge(
            &CommitSubject::of(&format!("feat(a): {long}")),
            &CommitPolicy::default(),
        );
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
        assert!(judge(&subject, &CommitPolicy::default())
            .findings
            .is_empty());
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
            paths: Some(vec![
                "src/commit/verdict.rs".into(),
                "test/cases/1.sh".into(),
            ]),
            ..CommitSubject::default()
        };
        assert!(judge(&with_test, &CommitPolicy::default())
            .findings
            .is_empty());

        let without = CommitSubject {
            text: "fix(commit): a thing".into(),
            paths: Some(vec!["src/commit/verdict.rs".into()]),
            ..CommitSubject::default()
        };
        let v = judge(&without, &CommitPolicy::default());
        assert_eq!(codes(&v), vec!["commit.fix_without_test"]);
        assert!(
            v.passed,
            "a warning by default, so that a gate over history does not rewrite it"
        );

        // a gate reading history knows no paths, and must not invent the finding
        let unknown = CommitSubject::of("fix(commit): a thing");
        assert!(judge(&unknown, &CommitPolicy::default())
            .findings
            .is_empty());
    }

    #[test]
    fn a_fix_that_only_touches_tests_is_covered_by_them() {
        let only_tests = CommitSubject {
            text: "fix(commit): a thing".into(),
            paths: Some(vec!["apps/majordomus-cli/src/commit/tests.rs".into()]),
            ..CommitSubject::default()
        };
        assert!(judge(&only_tests, &CommitPolicy::default())
            .findings
            .is_empty());
    }

    #[test]
    fn a_breaking_change_must_say_why() {
        let bare = judge(
            &CommitSubject::of("feat(api)!: the route moved"),
            &CommitPolicy::default(),
        );
        assert_eq!(codes(&bare), vec!["commit.breaking_unexplained"]);
        assert!(!bare.passed);

        let explained = judge(
            &CommitSubject::of("feat(api)!: the route moved\n\nIt is now under /api/v2."),
            &CommitPolicy::default(),
        );
        assert!(explained.findings.is_empty());

        let trailer = judge(
            &CommitSubject::of(
                "feat(api): the route moved\n\nBREAKING CHANGE: it is under /api/v2",
            ),
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
/// ```
/// use majordomus_cli::commit::verdict::JudgedCommit;
/// // what a history report lists: only the commits that have something to say
/// let row = JudgedCommit {
///     commit: "a1b2c3d4e".into(),
///     subject: "update stuff".into(),
///     passed: false,
///     exempt: None,
///     findings: Vec::new(),
/// };
/// assert!(!row.passed);
/// ```
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
/// A range of history, judged in one pass.
///
/// ```
/// use majordomus_cli::commit::verdict::HistoryReport;
/// let r = HistoryReport {
///     range: "HEAD".into(),
///     total: 1755,
///     exempt: 561,
///     failing: 54,
///     commits: Vec::new(),
/// };
/// // a commit with nothing to say is counted and not listed: a report of a thousand
/// // passing commits is a report nobody reads
/// assert!(r.commits.is_empty());
/// assert_eq!(r.total - r.exempt, 1194);
/// ```
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

/// A record id inside a backticked code span is being *shown*, not cited: a message that
/// writes ``I9999 — Gone from the model`` is illustrating the shape of a reference, and the
/// layer is not expected to hold it. Blanking the spans before the scan keeps a real
/// citation in prose readable while an example stops being one.
///
/// ```
/// use majordomus_cli::commit::verdict::without_code_spans;
/// assert_eq!(without_code_spans("names `I9999` here"), "names         here");
/// assert_eq!(without_code_spans("names I9999 here"), "names I9999 here");
/// ```
pub fn without_code_spans(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside = false;
    for ch in text.chars() {
        if ch == '`' {
            inside = !inside;
            out.push(' ');
        } else if inside && ch != '\n' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}
