//! The commit as a value: one grammar, one policy, one verdict, one plan.
//!
//! # What was here before
//!
//! Two halves of this subject already existed and did not know about each other. The
//! changelog parsed conventional commits out of the history to render a release; the rule
//! `project.conventional-commits` stated what a commit must look like and was marked
//! *advisory*, with the failure behaviour "no command decides this rule; a reviewer does".
//! So the repository had a parser and a rule, and no verdict — the exact shape of governance
//! the rest of this repository exists to refuse.
//!
//! The other half was outside the repository altogether. The procedure for writing a commit
//! — how to derive a scope, how to link work, when a fix needs a test, which subjects are
//! exempt — lived in prose that an AI client read and executed by hand, one file per client,
//! with a path-to-scope table maintained by a person. That is a workflow whose semantics
//! live in a prompt, which makes it unversioned, unenforceable, untestable and true for
//! exactly one tool.
//!
//! # What this is
//!
//! One typed subject with four parts, each the only place its question is answered:
//!
//! - [`CommitHeader`] and [`CommitMessage`] — the grammar. The changelog and the validator read this
//!   parse; there is no second regular expression anywhere.
//! - [`CommitPolicy`] — what this repository holds a commit to, as data in `policy.yaml`.
//! - [`judge`] — the verdict, as typed findings over the canonical [`crate::model::Diagnostic`].
//! - [`plan`] and [`scopes`] — what is in the working tree, which scope the *history* gives
//!   it, and how it divides into commits, fingerprinted against the tree it came from.
//!
//! Every surface is a projection of these: the capabilities in
//! [`crate::capability::builtin::commit`] carry them to MCP and HTTP, the command line
//! renders them, the `commit-msg` hook asks [`judge`] the same question CI asks of the
//! history, and the documentation is generated from the declarations. Nothing restates them.
//!
//! ```
//! use majordomus_cli::commit::{judge, CommitPolicy, CommitHeader, CommitMessage, CommitSubject};
//!
//! // one grammar
//! assert!(CommitHeader::parse("feat(commit): the subject is a value").is_conventional());
//! // one verdict, from the same parse
//! assert!(judge(&CommitSubject::of("feat(commit): the subject is a value"), &CommitPolicy::default()).passed);
//! // and a message is a value that renders, not a string that is parsed back
//! assert_eq!(CommitMessage::parse("docs: one line").render(), "docs: one line\n");
//! ```

pub mod header;
pub mod message;
pub mod plan;
pub mod policy;
pub mod scopes;
pub mod verdict;

pub use header::CommitHeader;
pub use message::{CommitMessage, CommitTrailer};
pub use plan::{ChangeStage, CommitGroup, CommitPlan, PlanFingerprint, WorkingTreeState};
pub use policy::{CommitPolicy, FindingLevel};
pub use scopes::{ScopeSuggestion, ScopeVocabulary};
pub use verdict::{judge, CommitExemption, CommitRule, CommitSubject, CommitVerdict};

#[cfg(test)]
mod tests {
    use super::*;

    /// The four parts compose into one answer, which is the only thing this module is
    /// responsible for: everything else is asserted beside the code that does it.
    ///
    /// The property under test is that a message *authored* as a value, rendered, parsed
    /// back and judged is the same commit throughout — because that is what lets the planner
    /// propose a message, the hook judge the rendered text, and the changelog read it back
    /// months later without any of the three holding its own idea of what a commit is.
    #[test]
    fn a_message_authored_as_a_value_survives_being_rendered_parsed_and_judged() {
        let authored = CommitMessage {
            header: CommitHeader {
                word: "feat".into(),
                kind: crate::release::ChangeKind::Feat,
                scope: Some("commit".into()),
                breaking: false,
                subject: "the value is the same at every step".into(),
            },
            body: "Why it was written this way.".into(),
            trailers: vec![CommitTrailer {
                key: "Co-Authored-By".into(),
                value: "Someone <s@example.org>".into(),
            }],
        };

        let rendered = authored.render();
        let parsed = CommitMessage::parse(&rendered);
        assert_eq!(parsed, authored, "the round trip is exact");

        let verdict = judge(&CommitSubject::of(&rendered), &CommitPolicy::default());
        assert!(verdict.passed, "{:?}", verdict.findings);
        assert_eq!(
            verdict.message.header.subject, authored.header.subject,
            "the judge read the same commit the author wrote"
        );

        // and the changelog's reader, which is a different caller of the same grammar,
        // agrees about every field it carries
        let change = crate::release::commits::parse("abc123456", &authored.header.render(), "");
        assert_eq!(change.kind, authored.header.kind);
        assert_eq!(change.scope, authored.header.scope);
        assert_eq!(change.subject, authored.header.subject);
        assert_eq!(change.breaking, authored.header.breaking);
    }

    /// A subject nobody spelled conventionally reaches both readers, and they disagree only
    /// about what to do with it. That disagreement is the design; a test that did not state
    /// it would leave the next person free to "fix" one of the two.
    #[test]
    fn the_changelog_carries_what_the_judge_refuses() {
        let subject = "update stuff";
        let change = crate::release::commits::parse("abc123456", subject, "");
        assert_eq!(
            change.subject, subject,
            "the changelog keeps it whole: dropping it would lie about what the release holds"
        );
        let verdict = judge(&CommitSubject::of(subject), &CommitPolicy::default());
        assert!(!verdict.passed, "the judge reports it");
        assert_eq!(verdict.findings.len(), 1, "and reports it once");
    }
}
