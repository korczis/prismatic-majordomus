//! The executable development task: one issue or one milestone read as work to be done,
//! with the origin of every value on the value.
//!
//! # What this is, and what it is not
//!
//! It is **not** a second project model. The canonical records under
//! `.ai/repo/project/` are the only declaration, [`crate::plan`] is the only derivation of
//! status, waves, dependents and findings out of them, `worktree::trace` is the
//! only derivation of the branches and commits that realised an issue, and
//! `scripts/github-sync` is the only adapter that projects either onto GitHub. This module
//! composes those three and adds nothing to them: every status it reports is the status
//! `plan.issues` reports, and `task::tests` holds that equality as an assertion.
//!
//! What it adds is the thing none of them could add on its own:
//!
//! 1. **One answer.** A worker asking "what is this issue, and can I start it?" had to call
//!    `plan.issues`, `trace.issue` and a shell adapter, and join three shapes by hand. The
//!    join is a derivation like any other, so it belongs in the executable.
//! 2. **Provenance on every field.** The plan flattens a missing key to the empty string,
//!    because that is what the awk it is held to byte-equality with does. That is right for
//!    the plan and wrong for a reader deciding whether to trust a value: `objective: ""` and
//!    a record with no `objective` at all are the same string and different facts. Every
//!    field here carries a [`FieldProvenance`] — [`Explicit`], [`Derived`], [`Inferred`] or
//!    [`Unknown`] — and the constructors are the only way to build one, so a field cannot
//!    claim to be authored unless the record's own metadata carried the key.
//! 3. **Readiness as a derived vocabulary.** [`readiness::TaskReadiness`] is a projection of the
//!    canonical status, never a replacement for it; it splits exactly the three distinctions
//!    the canonical vocabulary conflates and invents nothing else. See that module.
//!
//! [`Explicit`]: FieldProvenance::Explicit
//! [`Derived`]: FieldProvenance::Derived
//! [`Inferred`]: FieldProvenance::Inferred
//! [`Unknown`]: FieldProvenance::Unknown
//!
//! # The four kinds of fact, kept apart by the type
//!
//! A development task mixes facts that answer to different owners, and conflating them is
//! how a projection ends up overwriting what a person wrote. So they are four groups of one
//! structure rather than one flat bag of keys:
//!
//! | group | owner | may this executable write it |
//! |---|---|---|
//! | [`task::DevTaskDeclaration`] | the canonical record, authored by a person | no; `majordomus plan record` writes |
//! | [`task::DevTaskPosition`] | derived from the plan graph, stored nowhere | nothing to write |
//! | [`task::DevTaskExecution`] | local execution state: git refs, commits, sessions | nothing to write |
//! | [`task::DevTaskSynchronisation`] | the external projection on GitHub | **never**; the adapter owns it |
//!
//! Nothing here writes anything at all, which is what makes the last row safe: a
//! user-authored GitHub value cannot be silently overwritten by a reader.

pub mod graph;
pub mod readiness;
pub mod task;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use graph::{
    CriticalBlocker, MilestoneCounts, MilestoneEdge, MilestoneGraph, MilestoneNode, ParallelSet,
    ScopeConflict,
};
pub use readiness::{Blocker, BlockerKind, TaskReadiness};
pub use task::{
    DevTask, DevTaskDeclaration, DevTaskDiagnostic, DevTaskExecution, DevTaskPosition,
    DevTaskSynchronisation, ReadinessVerdict,
};

/// Where a value came from, and therefore how much weight a reader may put on it.
///
/// The four words are not degrees of confidence, they are four different relations to the
/// repository, and the distinction that matters most is the first from the last two: a
/// person wrote it, or a machine worked it out.
///
/// ```
/// use majordomus_cli::devtask::{AttestedText, FieldProvenance};
///
/// // a value the record itself carried
/// let title = AttestedText::explicit("Ship it", ".ai/repo/project/issues/I0001.yaml#title");
/// assert_eq!(title.provenance, FieldProvenance::Explicit);
///
/// // a key the record does not have is unknown, and unknown carries no value
/// let missing = AttestedText::unknown(
///     ".ai/repo/project/issues/I0001.yaml#objective",
///     "the record declares no `objective`",
/// );
/// assert_eq!(missing.provenance, FieldProvenance::Unknown);
/// assert!(missing.value.is_none());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FieldProvenance {
    /// A person authored it, in the canonical record, under this key. The strongest thing
    /// that can be said about a value, and the only one that survives a rewrite of every
    /// derivation in this repository.
    Explicit,
    /// A machine worked it out from records or from git, by a rule that cannot be wrong
    /// about a repository it can read: an issue's dependents are the issues that name it.
    /// Re-derived on every call and stored nowhere.
    Derived,
    /// A machine worked it out by a rule that *can* be wrong, because the relation it reads
    /// was never declared: a session belongs to an issue because its branch name contains
    /// the issue's id. Useful, and never to be mistaken for a declaration.
    Inferred,
    /// Not available. Either the record does not carry the key, or the fact lives outside
    /// what this process may read — a live GitHub state, for one — and the field says which
    /// in its `reason`. An unknown field never carries a value.
    Unknown,
}

impl FieldProvenance {
    /// The word a projection prints.
    ///
    /// ```
    /// use majordomus_cli::devtask::FieldProvenance;
    /// assert_eq!(FieldProvenance::Explicit.as_str(), "explicit");
    /// assert_eq!(FieldProvenance::Unknown.as_str(), "unknown");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            FieldProvenance::Explicit => "explicit",
            FieldProvenance::Derived => "derived",
            FieldProvenance::Inferred => "inferred",
            FieldProvenance::Unknown => "unknown",
        }
    }

    /// Did a person write this, in the canonical record?
    ///
    /// ```
    /// use majordomus_cli::devtask::FieldProvenance;
    /// assert!(FieldProvenance::Explicit.is_authored());
    /// assert!(!FieldProvenance::Derived.is_authored());
    /// ```
    pub fn is_authored(self) -> bool {
        matches!(self, FieldProvenance::Explicit)
    }
}

/// One scalar with its provenance: the unit every field of a development task is made of.
///
/// The invariant the constructors enforce, and [`AttestedText::is_consistent`] states, is
/// that a value is present exactly when the provenance is not [`FieldProvenance::Unknown`].
/// Without it the shape can express "unknown, and here is the value", which is the defect
/// this whole module exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestedText {
    /// The value, absent exactly when the provenance is `unknown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Where it came from.
    pub provenance: FieldProvenance,
    /// The one place a reader goes to check it: a repository-relative path with the key it
    /// was read from, a capability id, or a command. Never a machine path.
    pub source: String,
    /// Why the provenance is what it is, when that is worth a sentence: always for
    /// `unknown` and for `inferred`, and omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AttestedText {
    /// A value the canonical record carried under this key.
    pub fn explicit(value: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            value: Some(value.into()),
            provenance: FieldProvenance::Explicit,
            source: source.into(),
            reason: None,
        }
    }

    /// A value worked out by a rule that cannot be wrong about a readable repository.
    pub fn derived(value: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            value: Some(value.into()),
            provenance: FieldProvenance::Derived,
            source: source.into(),
            reason: None,
        }
    }

    /// A value worked out by a rule that can be wrong, with the rule stated.
    pub fn inferred(
        value: impl Into<String>,
        source: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            value: Some(value.into()),
            provenance: FieldProvenance::Inferred,
            source: source.into(),
            reason: Some(reason.into()),
        }
    }

    /// No value, and why there is none.
    pub fn unknown(source: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            value: None,
            provenance: FieldProvenance::Unknown,
            source: source.into(),
            reason: Some(reason.into()),
        }
    }

    /// The value, or the empty string. For a renderer; never for a decision.
    pub fn text(&self) -> &str {
        self.value.as_deref().unwrap_or_default()
    }

    /// Is the invariant held: a value present exactly when the provenance is not unknown?
    ///
    /// ```
    /// use majordomus_cli::devtask::AttestedText;
    /// assert!(AttestedText::explicit("x", "s").is_consistent());
    /// assert!(AttestedText::unknown("s", "why").is_consistent());
    /// ```
    pub fn is_consistent(&self) -> bool {
        self.value.is_some() != (self.provenance == FieldProvenance::Unknown)
    }
}

/// A list with its provenance. The distinction a bare `Vec` cannot make is the one that
/// matters here: `depends_on: []` in the record is an authored empty list, and a record with
/// no `depends_on` key is unknown. Both are empty; only one is a statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestedList {
    /// The items, in the order the record or the derivation produced them. Empty for an
    /// authored empty list and for an unknown field alike; `provenance` tells them apart.
    pub values: Vec<String>,
    /// Where they came from.
    pub provenance: FieldProvenance,
    /// The one place a reader goes to check them.
    pub source: String,
    /// Why the provenance is what it is, when that is worth a sentence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AttestedList {
    /// A list the canonical record carried under this key, including an empty one.
    pub fn explicit(values: Vec<String>, source: impl Into<String>) -> Self {
        Self {
            values,
            provenance: FieldProvenance::Explicit,
            source: source.into(),
            reason: None,
        }
    }

    /// A list worked out by a rule that cannot be wrong about a readable repository.
    pub fn derived(values: Vec<String>, source: impl Into<String>) -> Self {
        Self {
            values,
            provenance: FieldProvenance::Derived,
            source: source.into(),
            reason: None,
        }
    }

    /// A list worked out by a rule that can be wrong, with the rule stated.
    pub fn inferred(
        values: Vec<String>,
        source: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            values,
            provenance: FieldProvenance::Inferred,
            source: source.into(),
            reason: Some(reason.into()),
        }
    }

    /// No list at all, and why there is none.
    pub fn unknown(source: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            values: Vec::new(),
            provenance: FieldProvenance::Unknown,
            source: source.into(),
            reason: Some(reason.into()),
        }
    }

    /// How many items, which is zero for an unknown list too.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// No items — for an authored empty list as well as for an unknown one.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// A count with its provenance. Separate from [`AttestedText`] because a wave, a commit
/// tally and an evidence count are numbers a caller does arithmetic on, and a number
/// rendered as a string is a number every client has to parse back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestedCount {
    /// The number, absent exactly when the provenance is `unknown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<u32>,
    /// Where it came from.
    pub provenance: FieldProvenance,
    /// The one place a reader goes to check it.
    pub source: String,
    /// Why the provenance is what it is, when that is worth a sentence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AttestedCount {
    /// A number the canonical record carried under this key.
    pub fn explicit(value: u32, source: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            provenance: FieldProvenance::Explicit,
            source: source.into(),
            reason: None,
        }
    }

    /// A number worked out from records or from git.
    pub fn derived(value: u32, source: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            provenance: FieldProvenance::Derived,
            source: source.into(),
            reason: None,
        }
    }

    /// No number, and why there is none.
    pub fn unknown(source: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            value: None,
            provenance: FieldProvenance::Unknown,
            source: source.into(),
            reason: Some(reason.into()),
        }
    }
}

/// How much of one answer was authored, and how much a machine worked out.
///
/// Not decoration: it is the measurement that makes the provenance discipline checkable
/// from outside. A task whose declaration is mostly `unknown` is a placeholder however
/// complete its derived half looks, and this is the number that says so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttestationTally {
    /// Fields a person authored in the canonical record.
    pub explicit: u32,
    /// Fields worked out by a rule that cannot be wrong about a readable repository.
    pub derived: u32,
    /// Fields worked out by a rule that can be wrong.
    pub inferred: u32,
    /// Fields not available at all.
    pub unknown: u32,
}

impl AttestationTally {
    /// Count one field.
    pub fn count(&mut self, p: FieldProvenance) {
        match p {
            FieldProvenance::Explicit => self.explicit += 1,
            FieldProvenance::Derived => self.derived += 1,
            FieldProvenance::Inferred => self.inferred += 1,
            FieldProvenance::Unknown => self.unknown += 1,
        }
    }

    /// Every field counted.
    ///
    /// ```
    /// use majordomus_cli::devtask::{AttestationTally, FieldProvenance};
    /// let mut t = AttestationTally::default();
    /// t.count(FieldProvenance::Explicit);
    /// t.count(FieldProvenance::Unknown);
    /// assert_eq!(t.total(), 2);
    /// ```
    pub fn total(&self) -> u32 {
        self.explicit + self.derived + self.inferred + self.unknown
    }
}
