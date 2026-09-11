//! The canonical knowledge model: typed nodes, claims, evidence and relations, with the
//! four vocabularies every projection reads — provenance, confidence, freshness and
//! ownership — carried as data. Nothing here reads a file, runs git or knows a transport;
//! the extractors produce these values and the service, the capabilities, the command
//! line, the Cockpit and the site consume them.
//!
//! ```text
//! Evidence  ──supports──▶  Claim  ──about──▶  Node  ──relation──▶  Node
//!    │                       │                  │
//!    fingerprint             provenance          freshness (against the baseline)
//!    granularity             confidence          ownership, visibility
//! ```
//!
//! The rules the model holds itself to are in [`validate`], and they are refusals rather
//! than features: a claim without evidence is `unverified` and never `current`; an
//! identifier carries no whitespace; a relation names two nodes the model holds; a kind or
//! a predicate is one the extractor that emitted it declared.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Diagnostic;

/// The schema every persisted or served knowledge document carries. A document of another
/// version is refused by name, never guessed at (see [`super::migrate`]).
pub const SCHEMA: &str = "majordomus/knowledge/v1";

// ---------------------------------------------------------------- vocabularies

/// How a statement came to be known. The four classes are the whole vocabulary; a fifth
/// would be a class nobody can decide from evidence.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeProvenance")]
pub enum Provenance {
    /// Read off the repository or git by a deterministic extractor: a file exists, a
    /// manifest names a package, HEAD is a commit. The evidence is the source itself.
    Observed,
    /// Stated by a structured declaration a person wrote about the repository: front
    /// matter, a claims matrix, a deployment object. True because somebody said so.
    Declared,
    /// Concluded from other knowledge by a rule this executable applies, or by a semantic
    /// provider. Never mistaken for an observation.
    Derived,
    /// Written deliberately by a person as knowledge, under the curated kind of the layer.
    Curated,
}

impl Provenance {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::knowledge::model::Provenance;
    /// assert_eq!(Provenance::Declared.as_str(), "declared");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Provenance::Observed => "observed",
            Provenance::Declared => "declared",
            Provenance::Derived => "derived",
            Provenance::Curated => "curated",
        }
    }

    /// Every class, in the order a report lists them.
    pub const ALL: [Provenance; 4] = [
        Provenance::Observed,
        Provenance::Declared,
        Provenance::Derived,
        Provenance::Curated,
    ];
}

/// Whether knowledge may still be trusted, decided against its evidence and never by its
/// age. An old decision can be current; yesterday's note can already be contradicted.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeFreshness")]
pub enum Freshness {
    /// Every piece of evidence the knowledge rests on is what it was verified against.
    Current,
    /// Something the knowledge depends on moved; the knowledge itself was not touched.
    PossiblyStale,
    /// Evidence the knowledge rests on directly changed since it was verified.
    Stale,
    /// Other evidence contradicts it, and nobody has resolved which side holds.
    Conflicted,
    /// Nothing verifies it: no evidence, evidence that does not resolve, or no baseline.
    Unverified,
}

impl Freshness {
    /// The word as serialised.
    pub fn as_str(self) -> &'static str {
        match self {
            Freshness::Current => "current",
            Freshness::PossiblyStale => "possibly_stale",
            Freshness::Stale => "stale",
            Freshness::Conflicted => "conflicted",
            Freshness::Unverified => "unverified",
        }
    }

    /// Every state, in the order a report lists them.
    pub const ALL: [Freshness; 5] = [
        Freshness::Current,
        Freshness::PossiblyStale,
        Freshness::Stale,
        Freshness::Conflicted,
        Freshness::Unverified,
    ];

    /// The worse of two states, `conflicted` being the worst, then `stale`, `unverified`,
    /// `possibly_stale`, `current`.
    ///
    /// ```
    /// use majordomus_cli::knowledge::model::Freshness::*;
    /// assert_eq!(Current.worse(PossiblyStale), PossiblyStale);
    /// assert_eq!(Stale.worse(Conflicted), Conflicted);
    /// assert_eq!(Unverified.worse(PossiblyStale), Unverified);
    /// ```
    pub fn worse(self, other: Freshness) -> Freshness {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }

    fn rank(self) -> u8 {
        match self {
            Freshness::Current => 0,
            Freshness::PossiblyStale => 1,
            Freshness::Unverified => 2,
            Freshness::Stale => 3,
            Freshness::Conflicted => 4,
        }
    }

    /// Is this state one a health check counts as debt?
    pub fn is_debt(self) -> bool {
        !matches!(self, Freshness::Current)
    }
}

/// Who may change the source a piece of knowledge came from, which decides what
/// reconciliation may do with it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeOwnership")]
pub enum Ownership {
    /// A person or another tool owns the source; Majordomus reports and proposes, never
    /// rewrites.
    External,
    /// Majordomus generates the source and may regenerate or re-accept it.
    Majordomus,
    /// A file with a machine-owned part and a human-owned part; only the machine-owned
    /// part is ever updated automatically.
    Hybrid,
}

impl Ownership {
    /// The word as serialised.
    pub fn as_str(self) -> &'static str {
        match self {
            Ownership::External => "external",
            Ownership::Majordomus => "majordomus",
            Ownership::Hybrid => "hybrid",
        }
    }
}

/// Who may see a piece of knowledge, and therefore where it may be projected.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeVisibility")]
pub enum Visibility {
    /// May be published on a public surface such as GitHub Pages.
    Public,
    /// Served to whoever can reach the loopback server; never published.
    Internal,
    /// Never leaves this machine: not published, not sent to a remote provider.
    Restricted,
}

impl Visibility {
    /// The word as serialised.
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Internal => "internal",
            Visibility::Restricted => "restricted",
        }
    }

    /// The more restrictive of two.
    pub fn narrower(self, other: Visibility) -> Visibility {
        if other > self {
            other
        } else {
            self
        }
    }
}

/// How strongly the evidence supports a statement. Never a number a model produced: the
/// level follows from the basis, and the basis is a fact about the evidence.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeConfidenceLevel")]
pub enum ConfidenceLevel {
    /// Deterministically extracted, or corroborated by independent sources.
    High,
    /// One structured declaration or one curated statement, resolvable, uncontradicted.
    Medium,
    /// An inference, or a statement whose evidence does not resolve.
    Low,
}

/// Why a confidence level is what it is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeConfidenceBasis")]
pub enum ConfidenceBasis {
    /// A deterministic extractor read it from the source itself.
    DeterministicExtraction,
    /// A structured declaration states it and the declaration validates against its schema.
    StructuredDeclaration,
    /// A person wrote it as curated knowledge.
    HumanCuration,
    /// Two independent sources agree.
    Corroborated,
    /// One source only.
    SingleSource,
    /// The evidence it names does not resolve to anything this repository holds.
    ReferenceUnresolved,
    /// A semantic provider inferred it.
    SemanticInference,
}

/// The confidence of a claim or a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeConfidence")]
pub struct Confidence {
    /// The level, which follows from the basis.
    pub level: ConfidenceLevel,
    /// Why, in the order the reasons were established.
    pub basis: Vec<ConfidenceBasis>,
}

impl Confidence {
    /// The confidence of an observed statement.
    pub fn observed() -> Self {
        Confidence {
            level: ConfidenceLevel::High,
            basis: vec![ConfidenceBasis::DeterministicExtraction],
        }
    }

    /// The confidence of a structured declaration.
    pub fn declared() -> Self {
        Confidence {
            level: ConfidenceLevel::Medium,
            basis: vec![
                ConfidenceBasis::StructuredDeclaration,
                ConfidenceBasis::SingleSource,
            ],
        }
    }

    /// The confidence of a curated statement.
    pub fn curated() -> Self {
        Confidence {
            level: ConfidenceLevel::Medium,
            basis: vec![ConfidenceBasis::HumanCuration, ConfidenceBasis::SingleSource],
        }
    }

    /// The confidence of a semantic inference.
    pub fn inferred() -> Self {
        Confidence {
            level: ConfidenceLevel::Low,
            basis: vec![ConfidenceBasis::SemanticInference],
        }
    }

    /// Lower the confidence because the evidence does not resolve.
    pub fn unresolved(mut self) -> Self {
        self.level = ConfidenceLevel::Low;
        self.basis.retain(|b| *b != ConfidenceBasis::SingleSource);
        self.basis.push(ConfidenceBasis::ReferenceUnresolved);
        self
    }

    /// Raise the confidence because an independent source agrees.
    pub fn corroborated(mut self) -> Self {
        self.level = ConfidenceLevel::High;
        self.basis.retain(|b| *b != ConfidenceBasis::SingleSource);
        if !self.basis.contains(&ConfidenceBasis::Corroborated) {
            self.basis.push(ConfidenceBasis::Corroborated);
        }
        self
    }
}

// ---------------------------------------------------------------- evidence

/// What kind of thing a piece of evidence is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeEvidenceKind")]
pub enum EvidenceKind {
    /// A tracked file, whole.
    File,
    /// One object of the layer's index, with its parsed metadata.
    Object,
    /// One entry of a manifest or a configuration file: a dependency, a member, a section.
    ManifestEntry,
    /// A git revision.
    Commit,
    /// A capability of this executable's registry.
    Capability,
    /// A generated artifact of the generation plan.
    Artifact,
}

/// How much of a source a fingerprint covers. Stated rather than implied: file-level
/// evidence cannot pretend to entry-level precision.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeGranularity")]
pub enum Granularity {
    /// The whole revision.
    Revision,
    /// The whole file.
    File,
    /// One entry of a structured file.
    Entry,
    /// One object's parsed metadata, independent of its prose.
    Object,
}

/// A content fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeFingerprint")]
pub struct Fingerprint {
    /// The algorithm; `sha256` today.
    pub algorithm: String,
    /// The hex digest.
    pub value: String,
    /// How much of the source it covers.
    pub granularity: Granularity,
}

impl Fingerprint {
    /// A sha256 fingerprint of `content` at the given granularity.
    pub fn sha256(content: &[u8], granularity: Granularity) -> Self {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(content);
        Fingerprint {
            algorithm: "sha256".into(),
            value: format!("{:x}", h.finalize()),
            granularity,
        }
    }
}

/// Where a piece of evidence is found. Repository-relative, never a machine path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeLocator")]
pub struct Locator {
    /// The repository-relative path, when the evidence is in a file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The `majordomus://` URI, when the evidence is an object of the layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// The member inside the file, when the evidence is one entry of it
    /// (`dependencies.serde`, `claims.3`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<String>,
    /// The canonical id, when the evidence is a capability or an artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// One piece of evidence: something that was read, fingerprinted, and can be read again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeEvidence")]
pub struct Evidence {
    /// Stable identity: `file:<path>`, `entry:<path>#<member>`, `object:<uri>`,
    /// `commit:<sha>`, `capability:<id>`, `artifact:<path>`.
    pub id: String,
    /// What kind of thing it is.
    pub kind: EvidenceKind,
    /// Where it is.
    pub locator: Locator,
    /// Its fingerprint at the moment it was read.
    pub fingerprint: Fingerprint,
    /// The extractor that read it.
    pub extractor: String,
    /// Who may see it.
    pub visibility: Visibility,
    /// Whether its content may be sent to a remote provider at all.
    pub remote_processing: bool,
}

// ---------------------------------------------------------------- claims

/// How a claim is verified again later.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeVerification")]
pub enum Verification {
    /// The claim holds while the thing it names exists; re-checked live on every read.
    Existence,
    /// The claim was made against the content of its evidence; it holds while that
    /// content's fingerprint is the one it was verified against.
    Content,
}

/// Where a claim stands.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeClaimState")]
pub enum ClaimState {
    /// Asserted, and nothing contradicts it.
    Asserted,
    /// Another claim about the same subject and predicate carries a different value.
    Conflicted,
    /// Its evidence does not resolve, or it has none.
    Unverified,
}

/// One proposition: a subject, a predicate and a value, with what supports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeClaim")]
pub struct Claim {
    /// Stable identity: `<subject>#<predicate>` with a discriminator when several claims
    /// share both.
    pub id: String,
    /// The node the claim is about.
    pub subject: String,
    /// The predicate, from the vocabulary an extractor declared.
    pub predicate: String,
    /// The value: a string, a number, a boolean, a list.
    pub value: Value,
    /// How the claim came to be known.
    pub provenance: Provenance,
    /// The evidence ids that support it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// How strongly.
    pub confidence: Confidence,
    /// How it is verified again.
    pub verification: Verification,
    /// Where it stands.
    pub state: ClaimState,
    /// Where it stands against the baseline; filled by the freshness pass.
    pub freshness: Freshness,
    /// Why it stands there, when the reason is worth a line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ---------------------------------------------------------------- nodes and relations

/// One node of the knowledge graph: a thing the repository holds, has, decides or does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeNode")]
pub struct Node {
    /// Stable identity, `<kind>:<local>`: `component:majordomus-cli`, `document:docs/CLI.md`,
    /// `decision:adr-0021`, `capability:objects.get`. Survives a retitle.
    pub id: String,
    /// The kind, from the vocabulary an extractor declared.
    pub kind: String,
    /// The short name a listing shows.
    pub title: String,
    /// One line about it, when the source holds one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// How the node came to be known.
    pub provenance: Provenance,
    /// Who owns its source.
    pub ownership: Ownership,
    /// Who may see it.
    pub visibility: Visibility,
    /// How strongly the evidence supports it.
    pub confidence: Confidence,
    /// The evidence ids the node itself rests on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// The claims made about it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<Claim>,
    /// The repository-relative source, when one file owns the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The extractor that emitted it.
    pub extractor: String,
    /// Where it stands against the baseline; filled by the freshness pass.
    pub freshness: Freshness,
    /// Why, when the reason is worth a line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_reason: Option<String>,
    /// The Cockpit route of the underlying object, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}

/// One typed, directed relation between two nodes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeRelation")]
pub struct Relation {
    /// The node the relation leaves.
    pub source: String,
    /// The node it enters.
    pub target: String,
    /// The kind, from the vocabulary an extractor declared.
    pub kind: String,
    /// How the relation came to be known.
    pub provenance: Provenance,
    /// The evidence ids that state it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

// ---------------------------------------------------------------- vocabularies declared

/// One kind an extractor emits, with what it means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeKindInfo")]
pub struct KindInfo {
    /// The kind.
    pub kind: String,
    /// What a node of this kind is.
    pub meaning: String,
}

/// One relation kind an extractor emits, with what it asserts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeRelationInfo")]
pub struct RelationInfo {
    /// The relation kind.
    pub kind: String,
    /// What an edge of this kind asserts.
    pub meaning: String,
    /// True when a change at the target may make the source possibly stale.
    pub propagates: bool,
}

/// One predicate an extractor emits, with what it means and how claims of it behave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgePredicateInfo")]
pub struct PredicateInfo {
    /// The predicate.
    pub name: String,
    /// What a claim of it says.
    pub meaning: String,
    /// True when one subject has exactly one value: two different values are a conflict.
    pub functional: bool,
}

/// What an extractor is, declared once beside its behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeExtractorInfo")]
pub struct ExtractorInfo {
    /// The stable identity, `[a-z][a-z0-9_-]*`.
    pub id: String,
    /// The version; a change to what it emits is a new version.
    pub version: u32,
    /// The short name.
    pub title: String,
    /// One paragraph.
    pub description: String,
    /// True when the same tree always yields the same result; false for a semantic one.
    pub deterministic: bool,
    /// True when it reads content that may hold secrets.
    pub reads_sensitive: bool,
    /// The kinds it emits.
    pub kinds: Vec<KindInfo>,
    /// The relation kinds it emits.
    pub relations: Vec<RelationInfo>,
    /// The predicates it emits.
    pub predicates: Vec<PredicateInfo>,
}

// ---------------------------------------------------------------- conflicts, gaps, coverage

/// How serious a conflict is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeConflictSeverity")]
pub enum ConflictSeverity {
    /// An observed fact contradicts a declaration: the declaration is wrong or the code moved.
    High,
    /// Two declarations disagree.
    Medium,
    /// A derived statement disagrees with something stronger.
    Low,
}

/// One side of a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeConflictSide")]
pub struct ConflictSide {
    /// The claim id.
    pub claim: String,
    /// Its value.
    pub value: Value,
    /// Its provenance.
    pub provenance: Provenance,
    /// The evidence ids behind it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

/// Where a conflict stands.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeResolution")]
pub enum Resolution {
    /// Nobody has decided which side holds.
    Open,
    /// The baseline accepts it as known debt.
    Accepted,
}

/// Two claims about one subject and one functional predicate carrying different values.
/// Neither side is chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeConflict")]
pub struct Conflict {
    /// Stable identity: `<subject>#<predicate>`.
    pub id: String,
    /// The node both claims are about.
    pub subject: String,
    /// The predicate they disagree on.
    pub predicate: String,
    /// The sides, strongest provenance first.
    pub sides: Vec<ConflictSide>,
    /// How serious.
    pub severity: ConflictSeverity,
    /// Why the severity is what it is.
    pub basis: String,
    /// Where it stands.
    pub resolution: Resolution,
    /// What a person does about it.
    pub remedy: String,
}

/// What kind of gap a gap is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeGapCategory")]
pub enum GapCategory {
    /// A component the repository has and no document describes.
    UndocumentedComponent,
    /// A declaration names a file, a test or an object that does not exist.
    UnresolvedReference,
    /// Knowledge nothing verifies.
    UnverifiedKnowledge,
    /// A capability no use case, claim or decision names.
    UnexercisedCapability,
    /// A generated artifact whose source cannot be found, or a mirror of a canonical source.
    Canonicality,
}

/// Something the repository should know and does not, with what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeGap")]
pub struct Gap {
    /// Stable identity: `<category>:<subject>`.
    pub id: String,
    /// The category.
    pub category: GapCategory,
    /// The node or evidence it concerns.
    pub subject: String,
    /// Why it is a gap.
    pub reason: String,
    /// What closes it.
    pub remedy: String,
}

/// One coverage row: a denominator this executable can define deterministically, and how
/// much of it is covered. No percentage is computed; the numbers are the answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeCoverageRow")]
pub struct CoverageRow {
    /// Stable identity.
    pub id: String,
    /// What is counted.
    pub title: String,
    /// What defines the denominator.
    pub denominator: String,
    /// How many were discovered.
    pub discovered: usize,
    /// How many are covered.
    pub covered: usize,
    /// The node ids that are not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
}

/// Every coverage row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeCoverage")]
pub struct Coverage {
    /// The rows, in a stable order.
    pub rows: Vec<CoverageRow>,
}

// ---------------------------------------------------------------- the model

/// The repository the model is about, as git identifies it. No machine path: the model is
/// served and cached locally, but a projection of it may be published.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeRepositoryIdentity")]
pub struct RepositoryIdentity {
    /// The repository's name: the last component of its root.
    pub name: String,
    /// A digest of the root, so two processes can tell they mean one repository.
    pub id: String,
    /// The branch, when git could say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The commit, when git could say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// `clean`, `dirty` or `unknown`.
    pub working_tree: String,
}

/// Which reference freshness was decided against.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeReferenceInfo")]
pub struct ReferenceInfo {
    /// `baseline` when a tracked baseline exists, `none` otherwise.
    pub kind: String,
    /// The revision the baseline was taken at, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// The enforcement mode in force.
    pub mode: String,
    /// Where the baseline is, repository-relative, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// The whole model: what the repository knows, with the evidence behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeModel {
    /// [`SCHEMA`].
    pub schema: String,
    /// The repository it is about.
    pub repository: RepositoryIdentity,
    /// The reference freshness was decided against.
    pub reference: ReferenceInfo,
    /// Every extractor that ran, with its vocabulary.
    pub extractors: Vec<ExtractorInfo>,
    /// Every piece of evidence, by id.
    pub evidence: Vec<Evidence>,
    /// Every node, by id, claims inside.
    pub nodes: Vec<Node>,
    /// Every relation, sorted.
    pub relations: Vec<Relation>,
    /// Every conflict, by id.
    pub conflicts: Vec<Conflict>,
    /// Every gap, by id.
    pub gaps: Vec<Gap>,
    /// Coverage where a denominator is defined.
    pub coverage: Coverage,
    /// What could not be read or did not validate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    /// A hash of every node, claim, evidence fingerprint and relation, freshness excluded:
    /// the same tree yields the same fingerprint.
    pub fingerprint: String,
}

impl KnowledgeModel {
    /// A node by id.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes
            .binary_search_by(|n| n.id.as_str().cmp(id))
            .ok()
            .map(|i| &self.nodes[i])
    }

    /// A piece of evidence by id.
    pub fn evidence(&self, id: &str) -> Option<&Evidence> {
        self.evidence
            .binary_search_by(|e| e.id.as_str().cmp(id))
            .ok()
            .map(|i| &self.evidence[i])
    }

    /// The relations leaving a node.
    pub fn relations_from<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Relation> {
        self.relations.iter().filter(move |r| r.source == id)
    }

    /// The relations entering a node.
    pub fn relations_to<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Relation> {
        self.relations.iter().filter(move |r| r.target == id)
    }

    /// Every claim of every node, with the node it belongs to.
    pub fn claims(&self) -> impl Iterator<Item = (&Node, &Claim)> {
        self.nodes.iter().flat_map(|n| n.claims.iter().map(move |c| (n, c)))
    }

    /// The node kinds present, counted.
    pub fn kinds(&self) -> BTreeMap<&str, usize> {
        let mut m = BTreeMap::new();
        for n in &self.nodes {
            *m.entry(n.kind.as_str()).or_insert(0) += 1;
        }
        m
    }

    /// Nodes counted by freshness.
    pub fn freshness_tallies(&self) -> BTreeMap<String, usize> {
        let mut m: BTreeMap<String, usize> = Freshness::ALL
            .iter()
            .map(|f| (f.as_str().to_string(), 0))
            .collect();
        for n in &self.nodes {
            *m.entry(n.freshness.as_str().to_string()).or_insert(0) += 1;
        }
        m
    }

    /// Nodes counted by provenance.
    pub fn provenance_tallies(&self) -> BTreeMap<String, usize> {
        let mut m: BTreeMap<String, usize> = Provenance::ALL
            .iter()
            .map(|p| (p.as_str().to_string(), 0))
            .collect();
        for n in &self.nodes {
            *m.entry(n.provenance.as_str().to_string()).or_insert(0) += 1;
        }
        m
    }

    /// The predicate vocabulary every extractor declared, by name.
    pub fn predicates(&self) -> BTreeMap<&str, &PredicateInfo> {
        self.extractors
            .iter()
            .flat_map(|e| e.predicates.iter())
            .map(|p| (p.name.as_str(), p))
            .collect()
    }

    /// The relation vocabulary every extractor declared, by kind.
    pub fn relation_kinds(&self) -> BTreeMap<&str, &RelationInfo> {
        self.extractors
            .iter()
            .flat_map(|e| e.relations.iter())
            .map(|r| (r.kind.as_str(), r))
            .collect()
    }

    /// The node kinds every extractor declared, by kind, with the extractor that declared it.
    pub fn kind_vocabulary(&self) -> BTreeMap<&str, (&str, &KindInfo)> {
        let mut m = BTreeMap::new();
        for e in &self.extractors {
            for k in &e.kinds {
                m.entry(k.kind.as_str()).or_insert((e.id.as_str(), k));
            }
        }
        m
    }
}

/// Is `id` a well-formed knowledge identifier: `<kind>:<local>`, the kind
/// `[a-z][a-z0-9_-]*`, the local part non-empty and free of whitespace and control
/// characters?
///
/// ```
/// use majordomus_cli::knowledge::model::valid_id;
/// assert!(valid_id("component:majordomus-cli"));
/// assert!(valid_id("document:docs/Příručka.md"));
/// assert!(!valid_id("component"));
/// assert!(!valid_id("Component:x"));
/// assert!(!valid_id("component:a b"));
/// ```
pub fn valid_id(id: &str) -> bool {
    let Some((kind, local)) = id.split_once(':') else {
        return false;
    };
    let mut chars = kind.chars();
    let ok_kind = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    ok_kind
        && !local.is_empty()
        && !local
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
}

/// The kind half of a node id.
pub fn kind_of(id: &str) -> &str {
    id.split_once(':').map(|(k, _)| k).unwrap_or(id)
}

/// Every way the model contradicts itself. Empty is the healthy answer; anything else is
/// a defect in an extractor, never in the repository.
pub fn validate(model: &KnowledgeModel) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let err = |code: &'static str, subject: &str, message: String| {
        Diagnostic::error(code, Some(subject.to_string()), message)
    };
    if model.schema != SCHEMA {
        out.push(err(
            "knowledge_schema",
            "model",
            format!("schema '{}' is not {SCHEMA}", model.schema),
        ));
    }
    let node_ids: BTreeSet<&str> = model.nodes.iter().map(|n| n.id.as_str()).collect();
    let evidence_ids: BTreeSet<&str> = model.evidence.iter().map(|e| e.id.as_str()).collect();
    let kinds = model.kind_vocabulary();
    let predicates = model.predicates();
    let relation_kinds = model.relation_kinds();

    if node_ids.len() != model.nodes.len() {
        out.push(err(
            "knowledge_duplicate_node",
            "model",
            "two nodes carry one id".into(),
        ));
    }
    if evidence_ids.len() != model.evidence.len() {
        out.push(err(
            "knowledge_duplicate_evidence",
            "model",
            "two pieces of evidence carry one id".into(),
        ));
    }
    for e in &model.evidence {
        if e.fingerprint.value.is_empty() {
            out.push(err(
                "knowledge_empty_fingerprint",
                &e.id,
                "deterministic evidence carries no fingerprint".into(),
            ));
        }
    }
    let mut claim_ids: BTreeSet<&str> = BTreeSet::new();
    for n in &model.nodes {
        if !valid_id(&n.id) {
            out.push(err(
                "knowledge_invalid_id",
                &n.id,
                "a node id is <kind>:<local>, the local part free of whitespace".into(),
            ));
        }
        if kind_of(&n.id) != n.kind {
            out.push(err(
                "knowledge_kind_mismatch",
                &n.id,
                format!("the id says kind '{}' and the node says '{}'", kind_of(&n.id), n.kind),
            ));
        }
        if !kinds.contains_key(n.kind.as_str()) {
            out.push(err(
                "knowledge_undeclared_kind",
                &n.id,
                format!("kind '{}' is declared by no extractor", n.kind),
            ));
        }
        if !model.extractors.iter().any(|e| e.id == n.extractor) {
            out.push(err(
                "knowledge_unknown_extractor",
                &n.id,
                format!("extractor '{}' did not run", n.extractor),
            ));
        }
        for ev in &n.evidence {
            if !evidence_ids.contains(ev.as_str()) {
                out.push(err(
                    "knowledge_dangling_evidence",
                    &n.id,
                    format!("names evidence '{ev}' the model does not hold"),
                ));
            }
        }
        if n.provenance != Provenance::Observed
            && n.evidence.is_empty()
            && n.freshness == Freshness::Current
        {
            out.push(err(
                "knowledge_current_without_evidence",
                &n.id,
                "a node that is not observed cannot be current with no evidence".into(),
            ));
        }
        for c in &n.claims {
            if !claim_ids.insert(c.id.as_str()) {
                out.push(err(
                    "knowledge_duplicate_claim",
                    &c.id,
                    "two claims carry one id".into(),
                ));
            }
            if c.subject != n.id {
                out.push(err(
                    "knowledge_claim_subject",
                    &c.id,
                    format!("the claim is about '{}' and sits under '{}'", c.subject, n.id),
                ));
            }
            if !predicates.contains_key(c.predicate.as_str()) {
                out.push(err(
                    "knowledge_undeclared_predicate",
                    &c.id,
                    format!("predicate '{}' is declared by no extractor", c.predicate),
                ));
            }
            for ev in &c.evidence {
                if !evidence_ids.contains(ev.as_str()) {
                    out.push(err(
                        "knowledge_dangling_evidence",
                        &c.id,
                        format!("names evidence '{ev}' the model does not hold"),
                    ));
                }
            }
            if c.provenance != Provenance::Observed
                && c.evidence.is_empty()
                && c.freshness == Freshness::Current
            {
                out.push(err(
                    "knowledge_current_without_evidence",
                    &c.id,
                    "a claim that is not observed cannot be current with no evidence".into(),
                ));
            }
        }
    }
    for r in &model.relations {
        if !node_ids.contains(r.source.as_str()) || !node_ids.contains(r.target.as_str()) {
            out.push(err(
                "knowledge_dangling_relation",
                &format!("{} -{}-> {}", r.source, r.kind, r.target),
                "a relation names a node the model does not hold".into(),
            ));
        }
        if !relation_kinds.contains_key(r.kind.as_str()) {
            out.push(err(
                "knowledge_undeclared_relation",
                &format!("{} -{}-> {}", r.source, r.kind, r.target),
                format!("relation kind '{}' is declared by no extractor", r.kind),
            ));
        }
    }
    out
}

/// One node's fingerprint: its evidence fingerprints and its claims, in order, so that a
/// change to what it rests on or to what it says moves it and nothing else does. Sixteen
/// hex characters: enough to tell a change, short enough for a thousand of them to sit in
/// a tracked file.
pub fn node_fingerprint(node: &Node, model: &KnowledgeModel) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for ev in &node.evidence {
        h.update(ev.as_bytes());
        h.update(b"\0");
        if let Some(e) = model.evidence(ev) {
            h.update(e.fingerprint.value.as_bytes());
        }
        h.update(b"\n");
    }
    for c in &node.claims {
        h.update(c.id.as_bytes());
        h.update(b"\0");
        h.update(c.value.to_string().as_bytes());
        h.update(b"\n");
    }
    format!("{:x}", h.finalize())[..16].to_string()
}

/// A hash over everything that is a fact of the tree: node ids, kinds, titles, claims
/// (predicate and value), evidence fingerprints and relations, in sorted order, freshness
/// and diagnostics excluded.
pub fn fingerprint(
    nodes: &[Node],
    evidence: &[Evidence],
    relations: &[Relation],
) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for n in nodes {
        h.update(n.id.as_bytes());
        h.update(b"\0");
        h.update(n.kind.as_bytes());
        h.update(b"\0");
        h.update(n.title.as_bytes());
        h.update(b"\0");
        h.update(n.provenance.as_str().as_bytes());
        for c in &n.claims {
            h.update(b"\x01");
            h.update(c.id.as_bytes());
            h.update(b"\0");
            h.update(c.predicate.as_bytes());
            h.update(b"\0");
            h.update(c.value.to_string().as_bytes());
        }
        h.update(b"\n");
    }
    for e in evidence {
        h.update(e.id.as_bytes());
        h.update(b"\0");
        h.update(e.fingerprint.value.as_bytes());
        h.update(b"\n");
    }
    for r in relations {
        h.update(r.source.as_bytes());
        h.update(b"\0");
        h.update(r.kind.as_bytes());
        h.update(b"\0");
        h.update(r.target.as_bytes());
        h.update(b"\n");
    }
    format!("{:x}", h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extractor(id: &str, kind: &str, predicate: &str, relation: &str) -> ExtractorInfo {
        ExtractorInfo {
            id: id.into(),
            version: 1,
            title: id.into(),
            description: "a test extractor".into(),
            deterministic: true,
            reads_sensitive: false,
            kinds: vec![KindInfo {
                kind: kind.into(),
                meaning: "a thing".into(),
            }],
            relations: vec![RelationInfo {
                kind: relation.into(),
                meaning: "an edge".into(),
                propagates: true,
            }],
            predicates: vec![PredicateInfo {
                name: predicate.into(),
                meaning: "a predicate".into(),
                functional: true,
            }],
        }
    }

    fn evidence(id: &str) -> Evidence {
        Evidence {
            id: id.into(),
            kind: EvidenceKind::File,
            locator: Locator {
                path: Some("Cargo.toml".into()),
                ..Default::default()
            },
            fingerprint: Fingerprint::sha256(b"x", Granularity::File),
            extractor: "t".into(),
            visibility: Visibility::Public,
            remote_processing: true,
        }
    }

    fn node(id: &str, provenance: Provenance, evidence: &[&str]) -> Node {
        Node {
            id: id.into(),
            kind: kind_of(id).into(),
            title: id.into(),
            summary: None,
            provenance,
            ownership: Ownership::External,
            visibility: Visibility::Public,
            confidence: Confidence::observed(),
            evidence: evidence.iter().map(|e| e.to_string()).collect(),
            claims: vec![],
            source: None,
            extractor: "t".into(),
            freshness: Freshness::Current,
            freshness_reason: None,
            route: None,
        }
    }

    fn model(nodes: Vec<Node>, evidence: Vec<Evidence>, relations: Vec<Relation>) -> KnowledgeModel {
        let fingerprint = fingerprint(&nodes, &evidence, &relations);
        KnowledgeModel {
            schema: SCHEMA.into(),
            repository: RepositoryIdentity::default(),
            reference: ReferenceInfo::default(),
            extractors: vec![extractor("t", "component", "version", "depends_on")],
            evidence,
            nodes,
            relations,
            conflicts: vec![],
            gaps: vec![],
            coverage: Coverage::default(),
            diagnostics: vec![],
            fingerprint,
        }
    }

    #[test]
    fn a_well_formed_model_validates() {
        let m = model(
            vec![
                node("component:a", Provenance::Observed, &["file:Cargo.toml"]),
                node("component:b", Provenance::Declared, &["file:Cargo.toml"]),
            ],
            vec![evidence("file:Cargo.toml")],
            vec![Relation {
                source: "component:a".into(),
                target: "component:b".into(),
                kind: "depends_on".into(),
                provenance: Provenance::Observed,
                evidence: vec!["file:Cargo.toml".into()],
            }],
        );
        assert_eq!(validate(&m), vec![]);
    }

    #[test]
    fn a_declared_node_cannot_be_current_with_no_evidence() {
        let m = model(vec![node("component:b", Provenance::Declared, &[])], vec![], vec![]);
        let found = validate(&m);
        let codes: Vec<&str> = found.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.contains(&"knowledge_current_without_evidence"), "{codes:?}");
    }

    #[test]
    fn dangling_references_and_undeclared_vocabulary_are_named() {
        let mut n = node("gizmo:x", Provenance::Observed, &["file:missing"]);
        n.claims.push(Claim {
            id: "gizmo:x#colour".into(),
            subject: "gizmo:x".into(),
            predicate: "colour".into(),
            value: Value::String("red".into()),
            provenance: Provenance::Observed,
            evidence: vec![],
            confidence: Confidence::observed(),
            verification: Verification::Content,
            state: ClaimState::Asserted,
            freshness: Freshness::Current,
            reason: None,
        });
        let m = model(
            vec![n],
            vec![],
            vec![Relation {
                source: "gizmo:x".into(),
                target: "gizmo:y".into(),
                kind: "likes".into(),
                provenance: Provenance::Observed,
                evidence: vec![],
            }],
        );
        let found = validate(&m);
        let codes: BTreeSet<&str> = found.iter().map(|d| d.code.as_str()).collect();
        for want in [
            "knowledge_undeclared_kind",
            "knowledge_dangling_evidence",
            "knowledge_undeclared_predicate",
            "knowledge_dangling_relation",
            "knowledge_undeclared_relation",
        ] {
            assert!(codes.contains(want), "missing {want}: {codes:?}");
        }
    }

    #[test]
    fn the_fingerprint_ignores_freshness_and_follows_content() {
        let a = model(vec![node("component:a", Provenance::Observed, &[])], vec![], vec![]);
        let mut b = a.clone();
        b.nodes[0].freshness = Freshness::Stale;
        assert_eq!(
            fingerprint(&a.nodes, &a.evidence, &a.relations),
            fingerprint(&b.nodes, &b.evidence, &b.relations)
        );
        let mut c = a.clone();
        c.nodes[0].title = "renamed".into();
        assert_ne!(
            fingerprint(&a.nodes, &a.evidence, &a.relations),
            fingerprint(&c.nodes, &c.evidence, &c.relations)
        );
    }

    #[test]
    fn the_vocabularies_serialise_as_the_words_the_documentation_uses() {
        assert_eq!(serde_json::to_string(&Freshness::PossiblyStale).unwrap(), "\"possibly_stale\"");
        assert_eq!(serde_json::to_string(&Ownership::Majordomus).unwrap(), "\"majordomus\"");
        assert_eq!(serde_json::to_string(&Provenance::Curated).unwrap(), "\"curated\"");
        assert_eq!(serde_json::to_string(&Visibility::Restricted).unwrap(), "\"restricted\"");
        for f in Freshness::ALL {
            let back: Freshness = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
            assert_eq!(back, f);
        }
    }

    #[test]
    fn confidence_moves_with_its_basis_and_never_with_a_number() {
        let c = Confidence::declared().corroborated();
        assert_eq!(c.level, ConfidenceLevel::High);
        assert!(c.basis.contains(&ConfidenceBasis::Corroborated));
        assert!(!c.basis.contains(&ConfidenceBasis::SingleSource));
        let u = Confidence::curated().unresolved();
        assert_eq!(u.level, ConfidenceLevel::Low);
        assert!(u.basis.contains(&ConfidenceBasis::ReferenceUnresolved));
    }
}
