//! Freshness: whether a piece of knowledge still holds, decided from evidence
//! fingerprints and the baseline, never from a date.
//!
//! - An observed claim is current: it was read off the tree this scan.
//! - A declared claim is current when what it declares exists, unverified when the
//!   reference it makes resolves to nothing, and possibly stale when something it
//!   points at changed since the baseline was recorded.
//! - A curated claim about content is current while the evidence it was verified against
//!   still carries the fingerprint the baseline recorded, stale when that fingerprint
//!   moved, and unverified when nobody verified it yet.
//! - A derived claim is possibly stale until a person verifies it.
//! - A claim party to an open conflict is conflicted, whatever else it is.
//!
//! A node is as fresh as its least fresh claim, made worse by a propagating relation to
//! a node whose evidence changed since the baseline: a document describing a component
//! whose manifest moved is possibly stale even when its own text did not change.

use super::baseline::Baseline;
use super::model::{Claim, ClaimState, Freshness, KnowledgeModel, Node, Provenance, Verification};
use super::Adjacency;

/// Does this claim need a person to verify it against its evidence before it is current?
pub fn needs_verification(c: &Claim) -> bool {
    matches!(c.provenance, Provenance::Curated | Provenance::Derived)
        && c.verification == Verification::Content
        && !c.evidence.is_empty()
}

/// The freshness of one claim against the baseline, with the reason when it is not
/// current.
pub fn of_claim(c: &Claim, model: &KnowledgeModel, baseline: &Baseline) -> (Freshness, Option<String>) {
    if c.state == ClaimState::Conflicted {
        return (Freshness::Conflicted, c.reason.clone());
    }
    if c.state == ClaimState::Unverified {
        return (
            Freshness::Unverified,
            c.reason.clone().or_else(|| Some("the reference resolves to nothing".into())),
        );
    }
    match c.provenance {
        Provenance::Observed => (Freshness::Current, None),
        Provenance::Declared => (Freshness::Current, None),
        Provenance::Curated | Provenance::Derived => {
            if c.verification == Verification::Existence || c.evidence.is_empty() {
                return (Freshness::Current, None);
            }
            let mut worst = Freshness::Current;
            let mut reason = None;
            for ev in &c.evidence {
                let live = model.evidence(ev).map(|e| e.fingerprint.value.as_str());
                match (baseline.verified_fingerprint(&c.id, ev), live) {
                    (Some(recorded), Some(now)) if recorded == now => {}
                    (Some(_), Some(_)) => {
                        worst = worst.worse(Freshness::Stale);
                        reason = Some(format!("{ev} changed since this was verified"));
                    }
                    (Some(_), None) => {
                        worst = worst.worse(Freshness::Stale);
                        reason = Some(format!("{ev} is gone"));
                    }
                    (None, _) => {
                        let f = if c.provenance == Provenance::Derived {
                            Freshness::PossiblyStale
                        } else {
                            Freshness::Unverified
                        };
                        if worst.worse(f) == f && worst != f {
                            worst = f;
                            reason = Some(match c.provenance {
                                Provenance::Derived => "derived by a provider and not yet verified by a person".into(),
                                _ => format!("not verified against {ev}; run `majordomus knowledge reconcile --accept` after checking it"),
                            });
                        }
                    }
                }
            }
            (worst, reason)
        }
    }
}

/// The freshness of a node from its claims alone, before propagation: what recording the
/// baseline would leave, since recording verifies every content claim it can.
pub fn after_verification(n: &Node) -> Freshness {
    let mut f = Freshness::Current;
    for c in &n.claims {
        let cf = match c.state {
            ClaimState::Conflicted => Freshness::Conflicted,
            ClaimState::Unverified => Freshness::Unverified,
            ClaimState::Asserted => Freshness::Current,
        };
        f = f.worse(cf);
    }
    f
}

/// Compute the freshness of every claim and node against the baseline.
pub fn compute(model: &mut KnowledgeModel, baseline: &Baseline) {
    // the claims
    let snapshot = model.clone();
    for n in &mut model.nodes {
        let mut worst = Freshness::Current;
        let mut reason: Option<String> = None;
        for c in &mut n.claims {
            let (f, r) = of_claim(c, &snapshot, baseline);
            c.freshness = f;
            if f.worse(worst) == f && f != worst {
                worst = f;
                reason = r.map(|r| format!("{}: {r}", c.predicate));
            }
        }
        n.freshness = worst;
        n.freshness_reason = reason;
    }
    // what changed since the baseline: every node whose fingerprint moved. A node whose
    // own evidence moved is a fact of the tree — observed nodes are re-read, so they are
    // current — and what propagates is the change to their dependants
    if baseline.nodes.is_empty() {
        return;
    }
    let changed: std::collections::BTreeSet<&str> = snapshot
        .nodes
        .iter()
        .filter(|n| {
            baseline
                .node_fingerprint(&n.id)
                .is_some_and(|recorded| recorded != super::model::node_fingerprint(n, &snapshot))
        })
        .map(|n| n.id.as_str())
        .collect();
    if changed.is_empty() {
        return;
    }
    let propagating: std::collections::BTreeSet<&str> = snapshot
        .relation_kinds()
        .iter()
        .filter(|(_, info)| info.propagates)
        .map(|(k, _)| *k)
        .collect();
    let adjacency = Adjacency::of(&snapshot.relations);
    for n in &mut model.nodes {
        if n.provenance == Provenance::Observed || changed.contains(n.id.as_str()) {
            continue;
        }
        let Some(out) = adjacency.outgoing.get(n.id.as_str()) else {
            continue;
        };
        for r in out {
            if propagating.contains(r.kind.as_str()) && changed.contains(r.target.as_str()) {
                if n.freshness.worse(Freshness::PossiblyStale) == Freshness::PossiblyStale
                    && n.freshness != Freshness::PossiblyStale
                {
                    n.freshness = Freshness::PossiblyStale;
                    n.freshness_reason = Some(format!(
                        "{} changed since the baseline was recorded, and this {} it",
                        r.target,
                        r.kind.replace('_', " ")
                    ));
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::extract::{claim, claim_with};
    use crate::knowledge::model::{Evidence, EvidenceKind, Fingerprint, Granularity, Locator, Visibility};
    use serde_json::Value;

    fn model_with(evidence_fp: &str) -> KnowledgeModel {
        let mut m = KnowledgeModel {
            schema: super::super::model::SCHEMA.into(),
            repository: super::super::model::RepositoryIdentity {
                name: "r".into(),
                id: "x".into(),
                branch: None,
                head: None,
                working_tree: "clean".into(),
            },
            reference: super::super::model::ReferenceInfo {
                kind: "baseline".into(),
                revision: None,
                mode: "protect".into(),
                path: None,
            },
            extractors: vec![],
            evidence: vec![Evidence {
                id: "file:docs/X.md".into(),
                kind: EvidenceKind::File,
                locator: Locator { path: Some("docs/X.md".into()), ..Default::default() },
                fingerprint: Fingerprint { algorithm: "sha256".into(), value: evidence_fp.into(), granularity: Granularity::File },
                extractor: "t".into(),
                visibility: Visibility::Public,
                remote_processing: true,
            }],
            nodes: vec![],
            relations: vec![],
            conflicts: vec![],
            gaps: vec![],
            coverage: Default::default(),
            diagnostics: vec![],
            fingerprint: String::new(),
        };
        let mut n = crate::knowledge::extract::NodeSpec {
            id: "knowledge:k".into(),
            title: "k".into(),
            summary: None,
            provenance: Provenance::Curated,
            ownership: crate::knowledge::model::Ownership::External,
            visibility: Visibility::Public,
            evidence: vec!["file:docs/X.md".into()],
            source: None,
            extractor: "t",
        }
        .build();
        n.claims.push(claim_with(
            "knowledge:k",
            "reference",
            "verified_against=file:docs/X.md",
            Value::String("v".into()),
            Provenance::Curated,
            vec!["file:docs/X.md".into()],
            Verification::Content,
        ));
        n.claims.push(claim(
            "knowledge:k",
            "status",
            Value::String("verified".into()),
            Provenance::Curated,
            vec![],
            Verification::Content,
        ));
        m.nodes.push(n);
        m
    }

    #[test]
    fn a_curated_claim_is_unverified_then_current_then_stale() {
        let mut m = model_with("aaa");
        compute(&mut m, &Baseline::default());
        assert_eq!(m.nodes[0].freshness, Freshness::Unverified);
        let recorded = Baseline::record(&m, &Baseline::default());
        assert_eq!(recorded.verified.len(), 1);
        compute(&mut m, &recorded);
        assert_eq!(m.nodes[0].freshness, Freshness::Current, "{:?}", m.nodes[0].freshness_reason);
        let mut moved = model_with("bbb");
        compute(&mut moved, &recorded);
        assert_eq!(moved.nodes[0].freshness, Freshness::Stale);
        assert!(moved.nodes[0].freshness_reason.as_deref().unwrap().contains("changed since"));
    }
}

/// Helpers the sibling passes' tests share.
#[cfg(test)]
pub(crate) mod tests_support {
    use crate::knowledge::model::*;

    /// A model with nothing in it.
    pub fn empty_model() -> KnowledgeModel {
        KnowledgeModel {
            schema: SCHEMA.into(),
            repository: RepositoryIdentity {
                name: "r".into(),
                id: "x".into(),
                branch: None,
                head: None,
                working_tree: "clean".into(),
            },
            reference: ReferenceInfo {
                kind: "baseline".into(),
                revision: None,
                mode: "protect".into(),
                path: None,
            },
            extractors: vec![],
            evidence: vec![],
            nodes: vec![],
            relations: vec![],
            conflicts: vec![],
            gaps: vec![],
            coverage: Coverage::default(),
            diagnostics: vec![],
            fingerprint: String::new(),
        }
    }
}
