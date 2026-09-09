//! The typed value every release surface answers with.
//!
//! Published through the capability registry, so these types share the one OpenAPI component
//! namespace with every other module's: each carries a `Release`-prefixed schema name, the
//! way `distribution` prefixes its own for exactly the same reason.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The schema of the changelog document.
pub const CHANGELOG_SCHEMA: &str = "majordomus/changelog/v1";

/// What a conventional commit says it did.
///
/// The set is the one the repository's own commit convention uses; a commit whose subject
/// does not parse is [`ChangeKind::Other`] and still appears, because a changelog that
/// silently drops what it cannot classify is a changelog that lies by omission.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseChangeKind")]
pub enum ChangeKind {
    /// A capability a person did not have before.
    Feat,
    /// Behaviour that was wrong and is not any more.
    Fix,
    /// Performance, with the behaviour unchanged.
    Perf,
    /// Structure, with the behaviour unchanged.
    Refactor,
    /// Documentation.
    Docs,
    /// Tests.
    Test,
    /// The build, the pipeline, the tooling.
    Chore,
    /// Continuous integration.
    Ci,
    /// A commit whose subject does not parse as a conventional commit.
    Other,
}

impl ChangeKind {
    /// The kind one conventional-commit type word names.
    pub fn parse(word: &str) -> ChangeKind {
        match word {
            "feat" => ChangeKind::Feat,
            "fix" => ChangeKind::Fix,
            "perf" => ChangeKind::Perf,
            "refactor" => ChangeKind::Refactor,
            "docs" => ChangeKind::Docs,
            "test" => ChangeKind::Test,
            "chore" => ChangeKind::Chore,
            "ci" => ChangeKind::Ci,
            _ => ChangeKind::Other,
        }
    }

    /// The heading this kind is rendered under, and the order the headings appear in.
    ///
    /// Ordered by what a reader of a changelog came for: what is new, what is fixed, what is
    /// faster, then the rest. Nothing is hidden — a section with no entries is simply absent.
    pub fn heading(self) -> &'static str {
        match self {
            ChangeKind::Feat => "Added",
            ChangeKind::Fix => "Fixed",
            ChangeKind::Perf => "Performance",
            ChangeKind::Refactor => "Changed",
            ChangeKind::Docs => "Documentation",
            ChangeKind::Test => "Tests",
            ChangeKind::Ci => "Pipeline",
            ChangeKind::Chore => "Housekeeping",
            ChangeKind::Other => "Other",
        }
    }

    /// Where the heading sorts.
    pub fn rank(self) -> u8 {
        match self {
            ChangeKind::Feat => 0,
            ChangeKind::Fix => 1,
            ChangeKind::Perf => 2,
            ChangeKind::Refactor => 3,
            ChangeKind::Docs => 4,
            ChangeKind::Test => 5,
            ChangeKind::Ci => 6,
            ChangeKind::Chore => 7,
            ChangeKind::Other => 8,
        }
    }
}

/// One change, from one commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseChange")]
pub struct Change {
    /// What it did.
    pub kind: ChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The area it did it in, when the subject named one: `commands`, `ci`, `site`.
    pub scope: Option<String>,
    /// The subject, without the type and scope that prefixed it.
    pub subject: String,
    /// Whether the commit marked itself breaking, with `!` or a `BREAKING CHANGE:` trailer.
    pub breaking: bool,
    /// The abbreviated commit.
    pub commit: String,
}

/// One decision, as the layer's own ADR object states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseDecision")]
pub struct Decision {
    /// `adr-0027`.
    pub id: String,
    /// The decision, in its own words.
    pub title: String,
    /// `proposed`, `accepted`, `superseded`.
    pub status: String,
    /// The date the record carries.
    pub date: String,
    /// The page it is published at.
    pub route: String,
}

/// One published artifact, from the release record's own evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseChangelogArtifact")]
pub struct Artifact {
    /// The platform triple the record names.
    pub target: String,
    /// The file name.
    pub name: String,
    /// Its SHA-256, as the record read it off the file.
    pub sha256: String,
}

/// One version's worth of changelog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseSection")]
pub struct ReleaseSection {
    /// The version, or `unreleased`.
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The tag, when one was published.
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// When it was published, from the record.
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The commit it was published from.
    pub commit: Option<String>,
    /// Whether this section is the work that has not been released.
    pub unreleased: bool,
    /// The decisions dated inside this release's window.
    pub decisions: Vec<Decision>,
    /// The changes, from the commits in this release's range.
    pub changes: Vec<Change>,
    /// What was published, when this section is a release.
    pub artifacts: Vec<Artifact>,
}

/// The whole changelog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseChangelog")]
pub struct Changelog {
    /// The schema this document satisfies.
    pub schema: String,
    /// The version the tree currently declares.
    pub current: String,
    /// Newest first, the unreleased section leading when there is one.
    pub sections: Vec<ReleaseSection>,
    /// What could not be read, said rather than hidden: a repository with no git history,
    /// a release record that names no commit, a tag that is not in this clone.
    pub diagnostics: Vec<String>,
}

/// What the version is, and what the commits since the last release imply it should become.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseVersionReport")]
pub struct VersionReport {
    /// The version the crate manifest declares — the authority.
    pub declared: String,
    /// The version `bin/majordomus` prints.
    pub tool: String,
    /// Whether the two agree. `scripts/release-version --check` is the gate; this is the
    /// same question asked by the executable, so every surface can show the answer.
    pub agree: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The last release the layer records.
    pub last_release: Option<String>,
    /// What the commits since it imply: `major`, `minor`, `patch`, or `none`.
    pub bump: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The version that bump would produce.
    pub next: Option<String>,
    /// How many commits since the last release, and of what kind — the evidence for the
    /// bump, so that a surprising answer can be checked rather than believed.
    pub changes: Vec<Change>,
}
