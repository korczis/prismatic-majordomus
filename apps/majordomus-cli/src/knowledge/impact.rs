//! Impact: what a change set touches in the knowledge model. A change set is a list of
//! paths — the working tree against a base, two revisions, or paths named outright —
//! and impact walks from the evidence those paths are to the nodes that rest on it, then
//! along every propagating relation to the nodes that rest on those.
//!
//! Entry-level precision where an extractor offers it: a manifest with forty entries
//! changed in one is one changed entry, and only the nodes whose evidence is that entry
//! are hit directly. The comparison is between the base revision's content and the
//! working tree's, through the extractor's own `entries`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::git::ChangedPath;

use super::extract::Extractor;
use super::model::{Granularity, KnowledgeModel};
use super::Adjacency;

/// How far a change propagates along relations.
pub const MAX_DEPTH: usize = 4;

/// One entry of a structured file that changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeChangedEntry")]
pub struct ChangedEntry {
    /// The file.
    pub path: String,
    /// The entry within it.
    pub member: String,
    /// `added`, `modified` or `deleted`.
    pub status: String,
}

/// A node the change set reaches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeAffected")]
pub struct Affected {
    /// The node.
    pub id: String,
    /// Its kind.
    pub kind: String,
    /// Its title.
    pub title: String,
    /// `0` for a node whose own evidence changed; the hop count otherwise.
    pub depth: usize,
    /// The evidence id or the relation that carried the impact.
    pub via: String,
    /// One line: why.
    pub reason: String,
}

/// The impact of a change set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeImpactReport")]
pub struct ImpactReport {
    /// What the change set is: `working-tree`, `revisions` or `paths`.
    pub change_set: String,
    /// The base the working tree or the revision was compared with, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// The paths that changed.
    pub changed: Vec<ChangedPath>,
    /// The entries that changed, where an extractor could tell.
    pub entries: Vec<ChangedEntry>,
    /// Nodes whose own evidence changed.
    pub direct: Vec<Affected>,
    /// Nodes reached along propagating relations, nearest first.
    pub transitive: Vec<Affected>,
    /// The claims resting on changed evidence, by id.
    pub claims: Vec<String>,
    /// Extractors whose evidence the change set touches: what a scan re-reads.
    pub extractors: Vec<String>,
    /// Paths no evidence rests on: a change nothing in the model knows about.
    pub unmodelled: Vec<String>,
}

/// Analyse the impact of `changed` on the model. `base_content` answers the content of a
/// path at the base revision, for entry-level comparison; `live_content` the working
/// tree's.
pub fn analyse(
    model: &KnowledgeModel,
    extractors: &[Box<dyn Extractor>],
    change_set: &str,
    base: Option<String>,
    changed: Vec<ChangedPath>,
    base_content: &dyn Fn(&str) -> Option<String>,
    live_content: &dyn Fn(&str) -> Option<String>,
) -> ImpactReport {
    let changed_paths: BTreeSet<&str> = changed.iter().map(|c| c.path.as_str()).collect();
    // entry-level: which members of each modified file differ
    let mut entries = Vec::new();
    let mut changed_members: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut entry_aware: BTreeSet<&str> = BTreeSet::new();
    for c in changed.iter().filter(|c| c.status == "modified") {
        let (Some(before), Some(after)) = (base_content(&c.path), live_content(&c.path)) else {
            continue;
        };
        for e in extractors {
            let old: BTreeMap<String, String> = e
                .entries(&c.path, &before)
                .into_iter()
                .map(|(m, fp)| (m, fp.value))
                .collect();
            let new: BTreeMap<String, String> = e
                .entries(&c.path, &after)
                .into_iter()
                .map(|(m, fp)| (m, fp.value))
                .collect();
            if old.is_empty() && new.is_empty() {
                continue;
            }
            entry_aware.insert(c.path.as_str());
            let members: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
            for m in members {
                let status = match (old.get(m), new.get(m)) {
                    (Some(a), Some(b)) if a == b => continue,
                    (Some(_), Some(_)) => "modified",
                    (None, Some(_)) => "added",
                    (Some(_), None) => "deleted",
                    (None, None) => continue,
                };
                changed_members
                    .entry(c.path.as_str())
                    .or_default()
                    .insert(m.clone());
                entries.push(ChangedEntry {
                    path: c.path.clone(),
                    member: m.clone(),
                    status: status.into(),
                });
            }
        }
    }
    // the evidence that changed
    let mut hit_evidence: BTreeMap<&str, String> = BTreeMap::new();
    let mut extractors_hit = BTreeSet::new();
    let mut modelled_paths: BTreeSet<&str> = BTreeSet::new();
    for e in &model.evidence {
        let Some(path) = e.locator.path.as_deref() else {
            continue;
        };
        if !changed_paths.contains(path) {
            continue;
        }
        modelled_paths.insert(path);
        let reason = match e.fingerprint.granularity {
            Granularity::Entry => {
                let member = e.locator.member.as_deref().unwrap_or("");
                let file_changed_as_whole = !entry_aware.contains(path);
                let member_changed = changed_members
                    .get(path)
                    .is_some_and(|m| m.contains(member));
                if !(file_changed_as_whole || member_changed) {
                    continue;
                }
                format!("entry `{member}` of {path} changed")
            }
            Granularity::File => {
                // a file an extractor reads entry by entry, changed only in entries that
                // are not this node's, still changes the file evidence: the node that
                // rests on the whole file is hit, which is the truth of it
                format!("{path} changed")
            }
            Granularity::Object => format!("the object at {path} changed"),
            Granularity::Revision => format!("the revision changed"),
        };
        hit_evidence.insert(e.id.as_str(), reason);
        extractors_hit.insert(e.extractor.clone());
    }
    // direct: nodes and claims on that evidence
    let mut direct = Vec::new();
    let mut claims = Vec::new();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for n in &model.nodes {
        let mut via = None;
        for ev in &n.evidence {
            if let Some(reason) = hit_evidence.get(ev.as_str()) {
                via = Some((ev.clone(), reason.clone()));
                break;
            }
        }
        for c in &n.claims {
            if c.evidence.iter().any(|ev| hit_evidence.contains_key(ev.as_str())) {
                claims.push(c.id.clone());
                if via.is_none() {
                    let ev = c.evidence.iter().find(|ev| hit_evidence.contains_key(ev.as_str())).cloned().unwrap_or_default();
                    let reason = hit_evidence.get(ev.as_str()).cloned().unwrap_or_default();
                    via = Some((ev, reason));
                }
            }
        }
        if let Some((via, reason)) = via {
            seen.insert(n.id.as_str(), 0);
            direct.push(Affected {
                id: n.id.clone(),
                kind: n.kind.clone(),
                title: n.title.clone(),
                depth: 0,
                via,
                reason,
            });
        }
    }
    // transitive: along propagating relations, against their direction (the dependant
    // rests on the changed thing)
    let propagating: BTreeSet<&str> = model
        .relation_kinds()
        .iter()
        .filter(|(_, i)| i.propagates)
        .map(|(k, _)| *k)
        .collect();
    let adjacency = Adjacency::of(&model.relations);
    let mut transitive = Vec::new();
    let mut queue: VecDeque<(&str, usize)> = direct.iter().map(|a| (a.id.as_str(), 0)).collect();
    while let Some((id, depth)) = queue.pop_front() {
        if depth >= MAX_DEPTH {
            continue;
        }
        for r in adjacency.incoming.get(id).into_iter().flatten() {
            if !propagating.contains(r.kind.as_str()) {
                continue;
            }
            let next = r.source.as_str();
            if seen.contains_key(next) {
                continue;
            }
            seen.insert(next, depth + 1);
            if let Some(n) = model.node(next) {
                transitive.push(Affected {
                    id: n.id.clone(),
                    kind: n.kind.clone(),
                    title: n.title.clone(),
                    depth: depth + 1,
                    via: format!("{} {} {}", r.source, r.kind, r.target),
                    reason: format!("{} {} {}, which changed", n.id, r.kind.replace('_', " "), r.target),
                });
            }
            queue.push_back((next, depth + 1));
        }
    }
    transitive.sort_by(|a, b| a.depth.cmp(&b.depth).then(a.id.cmp(&b.id)));
    claims.sort();
    claims.dedup();
    let unmodelled = changed
        .iter()
        .filter(|c| !modelled_paths.contains(c.path.as_str()))
        .map(|c| c.path.clone())
        .collect();
    ImpactReport {
        change_set: change_set.into(),
        base,
        changed,
        entries,
        direct,
        transitive,
        claims,
        extractors: extractors_hit.into_iter().collect(),
        unmodelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::extract::NodeSpec;
    use crate::knowledge::model::*;

    #[test]
    fn a_changed_file_hits_its_node_and_propagates_to_what_rests_on_it() {
        let mut m = crate::knowledge::freshness::tests_support::empty_model();
        let mut x = crate::knowledge::extract::Extraction::default();
        let ev = x.file_evidence("t", "Cargo.toml", b"[package]", Visibility::Public);
        let a = NodeSpec { id: "component:a".into(), title: "a".into(), summary: None, provenance: Provenance::Observed, ownership: Ownership::External, visibility: Visibility::Public, evidence: vec![ev.clone()], source: Some("Cargo.toml".into()), extractor: "t" }.build();
        let d = NodeSpec { id: "document:README.md".into(), title: "r".into(), summary: None, provenance: Provenance::Observed, ownership: Ownership::External, visibility: Visibility::Public, evidence: vec![], source: None, extractor: "t" }.build();
        let far = NodeSpec { id: "module:m".into(), title: "m".into(), summary: None, provenance: Provenance::Observed, ownership: Ownership::External, visibility: Visibility::Public, evidence: vec![], source: None, extractor: "t" }.build();
        m.evidence = x.evidence;
        m.nodes = vec![a, d, far];
        m.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        m.relations = vec![
            Relation { source: "document:README.md".into(), target: "component:a".into(), kind: "references".into(), provenance: Provenance::Observed, evidence: vec![] },
            Relation { source: "module:m".into(), target: "component:a".into(), kind: "composes".into(), provenance: Provenance::Observed, evidence: vec![] },
        ];
        m.extractors.push(ExtractorInfo {
            id: "t".into(), version: 1, title: "t".into(), description: String::new(), deterministic: true, reads_sensitive: false, kinds: vec![],
            relations: vec![
                RelationInfo { kind: "references".into(), meaning: String::new(), propagates: true },
                RelationInfo { kind: "composes".into(), meaning: String::new(), propagates: false },
            ],
            predicates: vec![],
        });
        let changed = vec![ChangedPath { path: "Cargo.toml".into(), status: "modified".into() }, ChangedPath { path: "other.txt".into(), status: "added".into() }];
        let report = analyse(&m, &[], "paths", None, changed, &|_| None, &|_| None);
        assert_eq!(report.direct.len(), 1);
        assert_eq!(report.direct[0].id, "component:a");
        assert_eq!(report.transitive.len(), 1);
        assert_eq!(report.transitive[0].id, "document:README.md");
        assert_eq!(report.unmodelled, vec!["other.txt".to_string()]);
        assert_eq!(report.extractors, vec!["t".to_string()]);
    }
}
