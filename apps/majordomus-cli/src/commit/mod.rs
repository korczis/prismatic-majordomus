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
