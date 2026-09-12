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
    /// Formatting, with the behaviour unchanged.
    Style,
    /// The build system or its dependencies.
    Build,
    /// A change that undoes another.
    Revert,
    /// An integration: one line of work brought into another.
    ///
    /// Not in the conventional-commit specification, and in this repository's history 38
    /// times and counting, because integration here is a deliberate act with a subject
    /// somebody writes — `merge: bring origin/master into feature/x` — and not the sentence
    /// `git merge` composes. Declaring it is what lets the validator hold those subjects to
    /// the same width and the same references as every other commit, instead of either
    /// refusing a convention this project practises or exempting it from being checked at
    /// all. The changelog never renders it: `commits::in_range` reads the log with
    /// `--no-merges`, so a merge commit is not a release entry however its subject is
    /// spelled.
    Merge,
    /// A commit whose subject does not parse as a conventional commit.
    Other,
}

impl ChangeKind {
    /// Every type word the convention defines, with the kind it names.
    ///
    /// One list, read in both directions: [`ChangeKind::parse`] maps a word to a kind, and
    /// [`ChangeKind::WORDS`] is what a validator quotes back to somebody who typed a word
    /// that is not one. A second copy of these eleven words in the validator is exactly the
    /// duplication that lets a type added here go unmentioned there.
    const TABLE: &'static [(&'static str, ChangeKind)] = &[
        ("feat", ChangeKind::Feat),
        ("fix", ChangeKind::Fix),
        ("perf", ChangeKind::Perf),
        ("refactor", ChangeKind::Refactor),
        ("docs", ChangeKind::Docs),
        ("test", ChangeKind::Test),
        ("chore", ChangeKind::Chore),
        ("ci", ChangeKind::Ci),
        ("style", ChangeKind::Style),
        ("build", ChangeKind::Build),
        ("revert", ChangeKind::Revert),
        ("merge", ChangeKind::Merge),
    ];

    /// The type words, in the order [`Self::TABLE`] declares them.
    ///
    /// ```
    /// use majordomus_cli::release::ChangeKind;
    /// assert_eq!(ChangeKind::WORDS.first(), Some(&"feat"));
    /// assert!(ChangeKind::WORDS.iter().all(|w| ChangeKind::parse(w) != ChangeKind::Other));
    /// ```
    pub const WORDS: &'static [&'static str] = &[
        "feat", "fix", "perf", "refactor", "docs", "test", "chore", "ci", "style", "build",
        "revert", "merge",
    ];

    /// The kind one conventional-commit type word names.
    pub fn parse(word: &str) -> ChangeKind {
        ChangeKind::TABLE
            .iter()
            .find(|(w, _)| *w == word)
            .map(|(_, k)| *k)
            .unwrap_or(ChangeKind::Other)
    }

    /// The word this kind is written as, or `None` for [`ChangeKind::Other`], which is not
    /// a type anybody writes.
    ///
    /// The inverse of [`ChangeKind::parse`] over the one table, so the two cannot disagree
    /// about which words exist:
    ///
    /// ```
    /// use majordomus_cli::release::model::ChangeKind;
    ///
    /// assert_eq!(ChangeKind::parse("feat").word(), Some("feat"));
    /// assert_eq!(ChangeKind::Perf.word(), Some("perf"));
    ///
    /// // `Other` is what an unknown word parses to, and it is not itself a word: a
    /// // validator quoting the vocabulary back must not offer it as something to type.
    /// assert_eq!(ChangeKind::parse("nonsense"), ChangeKind::Other);
    /// assert_eq!(ChangeKind::Other.word(), None);
    ///
    /// // every word the vocabulary advertises round-trips through both directions
    /// for word in ChangeKind::WORDS {
    ///     assert_eq!(ChangeKind::parse(word).word(), Some(*word), "{word} round-trips");
    /// }
    /// ```
    pub fn word(self) -> Option<&'static str> {
        ChangeKind::TABLE
            .iter()
            .find(|(_, k)| *k == self)
            .map(|(w, _)| *w)
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
            ChangeKind::Style => "Formatting",
            ChangeKind::Build => "Build",
            ChangeKind::Revert => "Reverted",
            ChangeKind::Merge => "Integration",
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
            // A revert is what a reader most needs to see and least expects, so it sits with
            // the fixes rather than at the bottom with the housekeeping.
            ChangeKind::Revert => 2,
            ChangeKind::Build => 6,
            ChangeKind::Style => 7,
            ChangeKind::Merge => 8,
            ChangeKind::Other => 8,
        }
    }
}

/// A record of the layer that a commit names in its own text.
///
/// Inferred, never declared beside the commit: an issue id or a milestone id appearing in a
/// subject or a body is a reference, and the layer already holds the object it refers to. A
/// reference to something the layer does not have is not carried — a link to a record that
/// does not exist is worse than no link, because the reader cannot tell until they follow it.
///
/// ```
/// use majordomus_cli::release::model::Reference;
/// let r = Reference {
///     kind: "issue".into(),
///     id: "I1305".into(),
///     title: "An observed contract has a fingerprint".into(),
///     route: Some("/plan/i1305/".into()),
/// };
/// let json = serde_json::to_value(&r).unwrap();
/// assert_eq!(json["id"], "I1305");
/// assert_eq!(json["route"], "/plan/i1305/");
/// // a reference the site has no page for carries no route rather than an invented one
/// let bare = Reference { route: None, ..r };
/// assert!(serde_json::to_value(&bare).unwrap().get("route").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseReference")]
pub struct Reference {
    /// `issue` or `milestone`.
    pub kind: String,
    /// `I1305`, `M000`.
    pub id: String,
    /// What it is, from the record itself.
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Where it is published on this site, when it has a page.
    pub route: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Where that commit can be read, when the repository's own URL is known. Derived from
    /// `about::REPOSITORY`, never written beside each entry.
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The records of the layer this commit names — issues, milestones — resolved against
    /// what the layer actually holds.
    pub references: Vec<Reference>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Where the decision itself can be read: the file in the repository. The site has no
    /// per-ADR page, so this is the honest destination rather than an invented route.
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The commit that added the file, which is what decides the release it belongs to. The
    /// front-matter date says when the decision was made; those are different questions.
    pub added: Option<String>,
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

/// The changes of one kind, with the heading and the rank that decide where they are shown.
///
/// The grouping is in the document rather than in each renderer. It was in the Markdown
/// renderer alone, and the site — which cannot see a Rust function — grouped alphabetically
/// instead, so the same changelog read in two orders depending on which surface showed it.
/// A presentation order stated once and carried is the only kind that survives a projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseChangeGroup")]
pub struct ChangeGroup {
    /// What the changes in it did.
    pub kind: ChangeKind,
    /// The heading it is shown under.
    pub heading: String,
    /// Where it sorts among the other groups; ascending.
    pub rank: u8,
    /// The changes, in the order the commits were read.
    pub changes: Vec<Change>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The release notes the record names — the published release itself.
    pub notes_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Every commit between the previous release and this one, as the forge renders it.
    pub compare_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The tree at this release.
    pub tree_url: Option<String>,
    /// The decisions dated inside this release's window.
    pub decisions: Vec<Decision>,
    /// The changes, from the commits in this release's range, grouped by what they did and
    /// ordered by the rank each group carries. The flat list is the concatenation of the
    /// groups' own; nothing holds it twice.
    pub groups: Vec<ChangeGroup>,
    /// What was published, when this section is a release.
    pub artifacts: Vec<Artifact>,
}

/// Which capability answered with this document, and where that capability is projected.
///
/// The document carries its own provenance so that a reader — a person on the site, a client
/// over MCP — can find the same value elsewhere without anything having to enumerate the
/// routes. A page that listed them would be declaring them a second time, which is the
/// failure `site-check`'s `registry` and `cli` assertions exist to catch; it caught this one.
///
/// ```
/// use majordomus_cli::release::model::ProducedBy;
/// let by = ProducedBy {
///     capability: "release.changelog".into(),
///     cli: Some("majordomus release changelog".into()),
///     http: Some("/api/v1/changelog".into()),
///     mcp_tool: None,
///     mcp_resource: Some("majordomus://changelog".into()),
/// };
/// let json = serde_json::to_value(&by).unwrap();
/// assert_eq!(json["capability"], "release.changelog");
/// // a surface the capability is not projected on is absent, not null
/// assert!(json.get("mcp_tool").is_none());
/// assert_eq!(ProducedBy::default().capability, "");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseProducedBy")]
pub struct ProducedBy {
    /// The capability id.
    pub capability: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The command line that renders it.
    pub cli: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The HTTP route that answers with it.
    pub http: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The MCP tool.
    pub mcp_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The MCP resource URI.
    pub mcp_resource: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Which capability answered, and where else the same value can be had.
    pub produced_by: Option<ProducedBy>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_type_vocabulary_reads_the_same_in_both_directions() {
        // `WORDS` is what a validator quotes and `TABLE` is what the parser reads. They are
        // two spellings of one list, and this is the assertion that keeps them one: a type
        // added to the table and forgotten in the words, or the reverse, fails here rather
        // than in a validator that quietly stops mentioning it.
        let from_table: Vec<&str> = ChangeKind::TABLE.iter().map(|(w, _)| *w).collect();
        assert_eq!(from_table, ChangeKind::WORDS);
        for word in ChangeKind::WORDS {
            let kind = ChangeKind::parse(word);
            assert_ne!(kind, ChangeKind::Other, "{word} parses to nothing");
            assert_eq!(kind.word(), Some(*word), "{word} does not render back");
        }
        // every kind but `Other` is reachable from a word, so no variant is orphaned
        assert_eq!(ChangeKind::TABLE.len(), ChangeKind::WORDS.len());
        assert_eq!(ChangeKind::Other.word(), None);
        assert_eq!(ChangeKind::parse("wip"), ChangeKind::Other);
    }
}
