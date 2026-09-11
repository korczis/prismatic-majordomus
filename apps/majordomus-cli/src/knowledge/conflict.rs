//! Conflicts: two claims about one subject and one functional predicate with different
//! values. Deterministic by construction — a functional predicate has one value, so two
//! values from two sources are a contradiction the tree carries, not an opinion — and
//! never resolved silently: a conflict is recorded with both sides and their evidence,
//! marked on the claims, and stays open until a person accepts it by name.

use std::collections::BTreeMap;

use super::baseline::Baseline;
use super::model::{
    ClaimState, Conflict, ConflictSeverity, ConflictSide, Freshness, KnowledgeModel, Provenance,
    Resolution,
};

/// The severity of a contradiction between two provenances: a person contradicting the
/// tree is the one most worth a look, a provider contradicting anything the least.
fn severity(sides: &[ConflictSide]) -> ConflictSeverity {
    let has = |p: Provenance| sides.iter().any(|s| s.provenance == p);
    if has(Provenance::Derived) && sides.len() == 2 {
        return ConflictSeverity::Low;
    }
    if has(Provenance::Curated) && (has(Provenance::Observed) || has(Provenance::Declared)) {
        return ConflictSeverity::High;
    }
    if has(Provenance::Observed) && has(Provenance::Declared) {
        return ConflictSeverity::Medium;
    }
    if has(Provenance::Observed) && sides.iter().filter(|s| s.provenance == Provenance::Observed).count() > 1 {
        return ConflictSeverity::High;
    }
    ConflictSeverity::Medium
}

/// Find every conflict, mark the claims party to one, and set resolutions from the
/// baseline.
pub fn detect(model: &mut KnowledgeModel, baseline: &Baseline) {
    let functional: BTreeMap<String, bool> = model
        .predicates()
        .iter()
        .map(|(k, p)| (k.to_string(), p.functional))
        .collect();
    let mut conflicts = Vec::new();
    for n in &mut model.nodes {
        // group by predicate; a predicate no extractor declared is functional by default,
        // because a curated assertion of `owner` is one value or a contradiction
        let mut by_predicate: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (i, c) in n.claims.iter().enumerate() {
            if !functional.get(c.predicate.as_str()).copied().unwrap_or(true) {
                continue;
            }
            by_predicate.entry(c.predicate.as_str()).or_default().push(i);
        }
        let mut marks = Vec::new();
        for (predicate, indexes) in by_predicate {
            if indexes.len() < 2 {
                continue;
            }
            let mut sides: Vec<ConflictSide> = Vec::new();
            for &i in &indexes {
                let c = &n.claims[i];
                if sides.iter().any(|s| s.value == c.value) {
                    continue;
                }
                sides.push(ConflictSide {
                    claim: c.id.clone(),
                    value: c.value.clone(),
                    provenance: c.provenance,
                    evidence: c.evidence.clone(),
                });
            }
            if sides.len() < 2 {
                continue;
            }
            let id = format!("{}#{predicate}", n.id);
            let sev = severity(&sides);
            let basis = format!(
                "`{predicate}` is functional and {} sources give {} different values: {}",
                indexes.len(),
                sides.len(),
                sides
                    .iter()
                    .map(|s| format!("{} says {}", s.provenance.as_str(), s.value))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
            let resolution = if baseline.accepts(&id) {
                Resolution::Accepted
            } else {
                Resolution::Open
            };
            let remedy = match sides.iter().map(|s| s.provenance).max_by_key(|p| match p {
                Provenance::Observed => 3,
                Provenance::Declared => 2,
                Provenance::Curated => 1,
                Provenance::Derived => 0,
            }) {
                Some(Provenance::Observed) => "the tree is authoritative for an observed value: correct the declaration or the curated record, then record the baseline; or accept the conflict by id with a reason".to_string(),
                _ => "decide which source is right, correct the other, then record the baseline; or accept the conflict by id with a reason".to_string(),
            };
            for &i in &indexes {
                marks.push((i, id.clone(), sides.len()));
            }
            conflicts.push(Conflict {
                id,
                subject: n.id.clone(),
                predicate: predicate.to_string(),
                sides,
                severity: sev,
                basis,
                resolution,
                remedy,
            });
        }
        for (i, id, count) in marks {
            let c = &mut n.claims[i];
            c.state = ClaimState::Conflicted;
            c.freshness = Freshness::Conflicted;
            c.reason = Some(format!("party to conflict {id} ({count} values)"));
        }
    }
    conflicts.sort_by(|a, b| a.id.cmp(&b.id));
    model.conflicts = conflicts;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::extract::{claim, claim_with, NodeSpec};
    use crate::knowledge::model::{Ownership, Verification, Visibility};
    use serde_json::Value;

    #[test]
    fn two_values_of_a_functional_predicate_are_one_open_conflict_until_accepted() {
        let mut m = crate::knowledge::freshness::tests_support::empty_model();
        let mut n = NodeSpec {
            id: "component:a".into(),
            title: "a".into(),
            summary: None,
            provenance: Provenance::Observed,
            ownership: Ownership::External,
            visibility: Visibility::Public,
            evidence: vec![],
            source: None,
            extractor: "t",
        }
        .build();
        n.claims.push(claim("component:a", "version", Value::String("1.0".into()), Provenance::Observed, vec![], Verification::Content));
        n.claims.push(claim_with("component:a", "version", "k#0", Value::String("2.0".into()), Provenance::Curated, vec![], Verification::Content));
        n.claims.push(claim_with("component:a", "dependency", "x", Value::String("x".into()), Provenance::Observed, vec![], Verification::Content));
        n.claims.push(claim_with("component:a", "dependency", "y", Value::String("y".into()), Provenance::Observed, vec![], Verification::Content));
        m.nodes.push(n);
        m.extractors.push(crate::knowledge::model::ExtractorInfo {
            id: "t".into(),
            version: 1,
            title: "t".into(),
            description: String::new(),
            deterministic: true,
            reads_sensitive: false,
            kinds: vec![],
            relations: vec![],
            predicates: vec![
                crate::knowledge::model::PredicateInfo { name: "version".into(), meaning: String::new(), functional: true },
                crate::knowledge::model::PredicateInfo { name: "dependency".into(), meaning: String::new(), functional: false },
            ],
        });
        detect(&mut m, &Baseline::default());
        assert_eq!(m.conflicts.len(), 1);
        let c = &m.conflicts[0];
        assert_eq!(c.id, "component:a#version");
        assert_eq!(c.severity, ConflictSeverity::High);
        assert_eq!(c.resolution, Resolution::Open);
        assert_eq!(m.nodes[0].claims.iter().filter(|c| c.state == ClaimState::Conflicted).count(), 2);
        let accepted = Baseline {
            accepted: vec![crate::knowledge::baseline::AcceptedConflict { conflict: c.id.clone(), reason: None }],
            ..Default::default()
        };
        detect(&mut m, &accepted);
        assert_eq!(m.conflicts[0].resolution, Resolution::Accepted);
    }
}
