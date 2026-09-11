//! Extractors: the one boundary through which knowledge enters the model.
//!
//! An extractor reads one class of evidence — git, the layer's index, a Cargo manifest,
//! the documentation, the capability registry, the CI workflows — and emits typed nodes,
//! claims, evidence and relations in the vocabulary it declares beside them. It never
//! decides freshness, conflicts or coverage; those are passes over what every extractor
//! emitted, in [`super::service`].
//!
//! Composition mirrors the capability modules: one Rust module per extractor, each with
//! its `extractor()` constructor, and [`builtin`] composing them with `compose_extractors!`.
//! Adding an extractor is one module and one name in that list; its kinds, relations and
//! predicates appear in `knowledge extractors`, `knowledge kinds`, the site and the Cockpit
//! from the declaration, and nothing else is edited.

pub mod cargo;
pub mod docs;
pub mod git;
pub mod layer;
pub mod registry;
pub mod semantic;
pub mod workflows;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::capability::CapabilityRegistry;
use crate::git::GitState;
use crate::index::Index;
use crate::model::Diagnostic;
use crate::scope::{Scope, Verdict};

use super::model::{
    Claim, ClaimState, Confidence, Evidence, EvidenceKind, ExtractorInfo, Fingerprint,
    Freshness, Gap, GapCategory, Granularity, Locator, Node, Ownership, Provenance, Relation,
    Verification, Visibility,
};

/// What every extractor may read. Immutable for the run; nothing here writes.
pub struct ExtractionContext<'a> {
    /// The repository root, absolute. Used to read files; never written into the model.
    pub root: &'a Path,
    /// The layer's index: every declared object, validated.
    pub index: &'a Index,
    /// The capability registry.
    pub registry: &'a CapabilityRegistry,
    /// Every tracked path, sorted, as git lists them.
    pub tracked: &'a [String],
    /// What git said about the repository.
    pub git: &'a GitState,
    /// The repository scope: a path out of it is never read.
    pub scope: &'a Scope,
    /// The local half of the layer, repository-relative: never a source, never evidence.
    pub local: &'a str,
}

impl ExtractionContext<'_> {
    /// Read a tracked file as text, respecting the scope: a path the scope refuses, a
    /// symlink, a file over the index limit or one that is not UTF-8 answers `None`.
    pub fn read(&self, rel: &str) -> Option<String> {
        if self.scope.classify(self.root, rel).verdict == Verdict::Out {
            return None;
        }
        let abs = self.root.join(rel);
        let meta = std::fs::symlink_metadata(&abs).ok()?;
        if meta.file_type().is_symlink()
            || !meta.is_file()
            || meta.len() > crate::index::MAX_FILE_BYTES
        {
            return None;
        }
        String::from_utf8(std::fs::read(&abs).ok()?).ok()
    }

    /// Read a generated artifact the scope keeps out of a worker's context: the manifest
    /// and what it declares are Majordomus-owned and carry nothing sensitive, and the
    /// canonicality audit cannot see an orphan projection it may not read. Tracked files
    /// only; the symlink, size and UTF-8 rules of [`Self::read`] still apply.
    pub fn read_generated(&self, rel: &str) -> Option<String> {
        if !self.is_tracked(rel) {
            return None;
        }
        let abs = self.root.join(rel);
        let meta = std::fs::symlink_metadata(&abs).ok()?;
        if meta.file_type().is_symlink()
            || !meta.is_file()
            || meta.len() > crate::index::MAX_FILE_BYTES
        {
            return None;
        }
        String::from_utf8(std::fs::read(&abs).ok()?).ok()
    }

    /// Is a path a directory git tracks files under?
    pub fn is_directory(&self, rel: &str) -> bool {
        let rel = rel.trim_end_matches('/');
        if rel.is_empty() {
            return false;
        }
        let prefix = format!("{rel}/");
        let at = self.tracked.partition_point(|p| p.as_str() < prefix.as_str());
        self.tracked.get(at).is_some_and(|p| p.starts_with(&prefix))
    }

    /// The paths the committed generation manifest declares, when there is one: what a
    /// link to a generated file resolves to (an artifact, not a document).
    pub fn generated_paths(&self) -> BTreeSet<String> {
        let manifest = format!("{}/{}.json", crate::generate::OUT_DIR, crate::generate::MANIFEST_ID);
        let Some(text) = self.read_generated(&manifest) else {
            return BTreeSet::new();
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            return BTreeSet::new();
        };
        v.get("artifacts")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|a| a.get("path").and_then(|p| p.as_str()).map(str::to_string))
            .collect()
    }

    /// Is a path tracked?
    pub fn is_tracked(&self, rel: &str) -> bool {
        self.tracked.binary_search_by(|p| p.as_str().cmp(rel)).is_ok()
    }

    /// Is a path under the local half of the layer?
    pub fn is_local(&self, rel: &str) -> bool {
        rel == self.local || rel.starts_with(&format!("{}/", self.local))
    }

    /// The absolute path of a repository-relative one.
    pub fn abs(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

/// What one extractor emitted.
#[derive(Debug, Default, Clone)]
pub struct Extraction {
    /// Evidence, by id; a duplicate id with the same fingerprint is one piece.
    pub evidence: Vec<Evidence>,
    /// Nodes, with their claims.
    pub nodes: Vec<Node>,
    /// Relations between nodes this or another extractor emitted.
    pub relations: Vec<Relation>,
    /// Gaps the extractor could see on its own.
    pub gaps: Vec<Gap>,
    /// What could not be read or did not validate.
    pub diagnostics: Vec<Diagnostic>,
}

impl Extraction {
    /// Add a piece of file evidence and answer its id.
    pub fn file_evidence(
        &mut self,
        extractor: &str,
        path: &str,
        content: &[u8],
        visibility: Visibility,
    ) -> String {
        let id = format!("file:{path}");
        if !self.evidence.iter().any(|e| e.id == id) {
            self.evidence.push(Evidence {
                id: id.clone(),
                kind: EvidenceKind::File,
                locator: Locator {
                    path: Some(path.to_string()),
                    ..Default::default()
                },
                fingerprint: Fingerprint::sha256(content, Granularity::File),
                extractor: extractor.to_string(),
                visibility,
                remote_processing: visibility == Visibility::Public,
            });
        }
        id
    }

    /// Add a piece of entry evidence (one member of a structured file) and answer its id.
    pub fn entry_evidence(
        &mut self,
        extractor: &str,
        path: &str,
        member: &str,
        content: &[u8],
        visibility: Visibility,
    ) -> String {
        let id = format!("entry:{path}#{member}");
        if !self.evidence.iter().any(|e| e.id == id) {
            self.evidence.push(Evidence {
                id: id.clone(),
                kind: EvidenceKind::ManifestEntry,
                locator: Locator {
                    path: Some(path.to_string()),
                    member: Some(member.to_string()),
                    ..Default::default()
                },
                fingerprint: Fingerprint::sha256(content, Granularity::Entry),
                extractor: extractor.to_string(),
                visibility,
                remote_processing: visibility == Visibility::Public,
            });
        }
        id
    }

    /// Add a node, or merge into one already emitted with the same id.
    pub fn node(&mut self, node: Node) -> &mut Node {
        let at = match self.nodes.iter().position(|n| n.id == node.id) {
            Some(i) => {
                let existing = &mut self.nodes[i];
                for ev in node.evidence {
                    if !existing.evidence.contains(&ev) {
                        existing.evidence.push(ev);
                    }
                }
                existing.claims.extend(node.claims);
                if existing.summary.is_none() {
                    existing.summary = node.summary;
                }
                i
            }
            None => {
                self.nodes.push(node);
                self.nodes.len() - 1
            }
        };
        &mut self.nodes[at]
    }

    /// Add a relation, once.
    pub fn relation(&mut self, relation: Relation) {
        if !self.relations.contains(&relation) {
            self.relations.push(relation);
        }
    }

    /// Add a gap, once by id.
    pub fn gap(&mut self, category: GapCategory, subject: &str, reason: &str, remedy: &str) {
        let id = format!("{}:{subject}", gap_word(category));
        if !self.gaps.iter().any(|g| g.id == id) {
            self.gaps.push(Gap {
                id,
                category,
                subject: subject.to_string(),
                reason: reason.to_string(),
                remedy: remedy.to_string(),
            });
        }
    }
}

/// The node of a directory git tracks files under, for a reference that names one.
pub fn directory_node(path: &str, extractor: &'static str) -> Node {
    let path = path.trim_end_matches('/').to_string();
    NodeSpec {
        id: format!("directory:{path}"),
        title: format!("{path}/"),
        summary: None,
        provenance: Provenance::Observed,
        ownership: Ownership::External,
        visibility: Visibility::Public,
        evidence: vec![],
        source: Some(path),
        extractor,
    }
    .build()
}

/// The node a repository path resolves to, for an extractor that found a reference to
/// it: the layer's object when the index holds it, the generated artifact when the
/// manifest declares it, a directory when git tracks files under it, a plain file when
/// git tracks it, and nothing otherwise. The node for a file or a directory is added to
/// the extraction; the other two are named and left to their own extractors.
pub fn resolve_path(
    out: &mut Extraction,
    ctx: &ExtractionContext<'_>,
    by_path: &std::collections::BTreeMap<&str, String>,
    generated: &BTreeSet<String>,
    target: &str,
    extractor: &'static str,
) -> Option<String> {
    let target = target.trim_end_matches('/');
    if let Some(layer) = by_path.get(target) {
        return Some(layer.clone());
    }
    if generated.contains(target) {
        return Some(format!("artifact:{target}"));
    }
    if ctx.is_directory(target) {
        let n = directory_node(target, extractor);
        let id = n.id.clone();
        out.node(n);
        return Some(id);
    }
    if ctx.is_tracked(target) {
        let id = format!("file:{target}");
        out.node(
            NodeSpec {
                id: id.clone(),
                title: target.to_string(),
                summary: None,
                provenance: Provenance::Observed,
                ownership: Ownership::External,
                visibility: Visibility::Public,
                evidence: vec![],
                source: Some(target.to_string()),
                extractor,
            }
            .build(),
        );
        return Some(id);
    }
    None
}

/// The word a gap category is keyed under.
pub fn gap_word(category: GapCategory) -> &'static str {
    match category {
        GapCategory::UndocumentedComponent => "undocumented",
        GapCategory::UnresolvedReference => "unresolved",
        GapCategory::UnverifiedKnowledge => "unverified",
        GapCategory::UnexercisedCapability => "unexercised",
        GapCategory::Canonicality => "canonicality",
    }
}

/// A node under construction: the fields every node has, filled in one place.
pub struct NodeSpec<'a> {
    /// `<kind>:<local>`.
    pub id: String,
    /// The title.
    pub title: String,
    /// One line, when there is one.
    pub summary: Option<String>,
    /// How it came to be known.
    pub provenance: Provenance,
    /// Who owns its source.
    pub ownership: Ownership,
    /// Who may see it.
    pub visibility: Visibility,
    /// The evidence ids it rests on.
    pub evidence: Vec<String>,
    /// The repository-relative source, when one file owns it.
    pub source: Option<String>,
    /// The extractor emitting it.
    pub extractor: &'a str,
}

impl NodeSpec<'_> {
    /// The node, with the confidence its provenance implies and no freshness decided yet.
    pub fn build(self) -> Node {
        let confidence = match self.provenance {
            Provenance::Observed => Confidence::observed(),
            Provenance::Declared => Confidence::declared(),
            Provenance::Curated => Confidence::curated(),
            Provenance::Derived => Confidence::inferred(),
        };
        Node {
            kind: super::model::kind_of(&self.id).to_string(),
            id: self.id,
            title: self.title,
            summary: self.summary,
            provenance: self.provenance,
            ownership: self.ownership,
            visibility: self.visibility,
            confidence,
            evidence: self.evidence,
            claims: Vec::new(),
            source: self.source,
            extractor: self.extractor.to_string(),
            freshness: Freshness::Unverified,
            freshness_reason: None,
            route: None,
        }
    }
}

/// A claim about a node, with the confidence its provenance implies.
pub fn claim(
    subject: &str,
    predicate: &str,
    value: Value,
    provenance: Provenance,
    evidence: Vec<String>,
    verification: Verification,
) -> Claim {
    let confidence = match provenance {
        Provenance::Observed => Confidence::observed(),
        Provenance::Declared => Confidence::declared(),
        Provenance::Curated => Confidence::curated(),
        Provenance::Derived => Confidence::inferred(),
    };
    Claim {
        id: format!("{subject}#{predicate}"),
        subject: subject.to_string(),
        predicate: predicate.to_string(),
        value,
        provenance,
        evidence,
        confidence,
        verification,
        state: ClaimState::Asserted,
        freshness: Freshness::Unverified,
        reason: None,
    }
}

/// A claim whose id carries a discriminator, for several claims of one predicate about
/// one subject (one per dependency, one per reference).
pub fn claim_with(
    subject: &str,
    predicate: &str,
    discriminator: &str,
    value: Value,
    provenance: Provenance,
    evidence: Vec<String>,
    verification: Verification,
) -> Claim {
    let mut c = claim(subject, predicate, value, provenance, evidence, verification);
    c.id = format!("{subject}#{predicate}[{discriminator}]");
    c
}

/// One extractor: its declaration and its behaviour.
pub trait Extractor: Send + Sync {
    /// What it is and what it emits. May look at the context for a vocabulary that is
    /// read off the repository (the layer's kinds).
    fn info(&self, ctx: &ExtractionContext<'_>) -> ExtractorInfo;
    /// Run it.
    fn extract(&self, ctx: &ExtractionContext<'_>) -> Extraction;
    /// The entries of one structured file this extractor reads, with their fingerprints:
    /// what impact analysis compares between two versions of the file. Empty for an
    /// extractor whose evidence is whole files.
    fn entries(&self, _path: &str, _content: &str) -> Vec<(String, Fingerprint)> {
        Vec::new()
    }
}

/// Compose extractors: each name is a module with an `extractor()` constructor. The one
/// list there is; a name added here is an extractor everywhere.
#[macro_export]
macro_rules! compose_extractors {
    ( $( $($segment:ident)::+ ),* $(,)? ) => {
        vec![$( Box::new($($segment)::+::extractor()) as Box<dyn $crate::knowledge::extract::Extractor> ),*]
    };
}

/// The built-in extractors, composed. Deterministic ones only: the semantic extractor
/// reads what a provider cached and is composed here too, because a cached derivation is
/// a fact of the checkout, not a call to a provider.
pub mod builtin {
    use super::Extractor;

    /// Every extractor this executable ships, in the order they run.
    pub fn extractors() -> Vec<Box<dyn Extractor>> {
        crate::compose_extractors![
            super::git,
            super::layer,
            super::cargo,
            super::docs,
            super::registry,
            super::workflows,
            super::semantic
        ]
    }
}

/// Every tracked path matching one of the given predicates, in git order.
pub fn tracked_where<'a>(
    ctx: &'a ExtractionContext<'_>,
    keep: impl Fn(&str) -> bool + 'a,
) -> impl Iterator<Item = &'a String> + 'a {
    ctx.tracked
        .iter()
        .filter(move |p| !ctx.is_local(p) && keep(p))
}

/// The set of node ids an extraction holds, for a caller that wants to link into it.
pub fn node_ids(extraction: &Extraction) -> BTreeSet<&str> {
    extraction.nodes.iter().map(|n| n.id.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_added_twice_is_merged_and_its_evidence_deduplicated() {
        let mut x = Extraction::default();
        let mut a = NodeSpec {
            id: "component:a".into(),
            title: "a".into(),
            summary: None,
            provenance: Provenance::Observed,
            ownership: Ownership::External,
            visibility: Visibility::Public,
            evidence: vec!["file:Cargo.toml".into()],
            source: Some("Cargo.toml".into()),
            extractor: "t",
        }
        .build();
        a.claims.push(claim(
            "component:a",
            "version",
            Value::String("1".into()),
            Provenance::Observed,
            vec![],
            Verification::Content,
        ));
        x.node(a.clone());
        let mut b = a.clone();
        b.summary = Some("later".into());
        b.claims[0].id = "component:a#version[2]".into();
        x.node(b);
        assert_eq!(x.nodes.len(), 1);
        assert_eq!(x.nodes[0].evidence.len(), 1);
        assert_eq!(x.nodes[0].claims.len(), 2);
        assert_eq!(x.nodes[0].summary.as_deref(), Some("later"));
    }

    #[test]
    fn evidence_and_gaps_are_added_once() {
        let mut x = Extraction::default();
        let a = x.file_evidence("t", "README.md", b"hello", Visibility::Public);
        let b = x.file_evidence("t", "README.md", b"hello", Visibility::Public);
        assert_eq!(a, b);
        assert_eq!(x.evidence.len(), 1);
        assert_eq!(x.evidence[0].fingerprint.granularity, Granularity::File);
        x.gap(GapCategory::UnresolvedReference, "rule:x", "why", "how");
        x.gap(GapCategory::UnresolvedReference, "rule:x", "why", "how");
        assert_eq!(x.gaps.len(), 1);
        assert_eq!(x.gaps[0].id, "unresolved:rule:x");
    }
}
