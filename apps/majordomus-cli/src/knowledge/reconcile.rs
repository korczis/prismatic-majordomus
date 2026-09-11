//! Reconciliation: what to do about every conflict, stale claim, unresolved reference
//! and gap, as proposals with an owner. Majordomus rewrites nothing it does not own: a
//! proposal about an external source is a sentence for a person, and the one action the
//! tool takes itself — on `--accept` — is recording that a person verified the curated
//! claims against their present evidence, which is a baseline change with the diff in
//! the commit.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::baseline::{Baseline, VerifiedClaim};
use super::model::{Freshness, GapCategory, KnowledgeModel, Ownership, Provenance, Resolution};

/// One proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeProposal")]
pub struct Proposal {
    /// Stable across runs: `<class>:<subject>`.
    pub id: String,
    /// `resolve_conflict`, `reverify`, `fix_reference`, `document`, `exercise`,
    /// `derive` or `review`.
    pub class: String,
    /// The node or claim it is about.
    pub subject: String,
    /// What to do, one sentence.
    pub action: String,
    /// Who owns the source: `external` means a person edits it.
    pub owner: Ownership,
    /// `accept` applies it on `--accept`; `edit` needs a person.
    pub applies_by: String,
}

/// What reconciliation found and, on accept, did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeReconciliation")]
pub struct Reconciliation {
    /// Every proposal, by class then subject.
    pub proposals: Vec<Proposal>,
    /// How many are applied by `--accept`.
    pub acceptable: usize,
    /// The claims accepted, when `--accept` ran.
    pub accepted: Vec<String>,
    /// Whether the baseline was written.
    pub baseline_written: bool,
}

/// Propose.
pub fn propose(model: &KnowledgeModel) -> Vec<Proposal> {
    let mut out = Vec::new();
    for c in &model.conflicts {
        if c.resolution == Resolution::Accepted {
            continue;
        }
        out.push(Proposal {
            id: format!("resolve_conflict:{}", c.id),
            class: "resolve_conflict".into(),
            subject: c.subject.clone(),
            action: format!("{}: {}", c.basis, c.remedy),
            owner: Ownership::External,
            applies_by: "edit".into(),
        });
    }
    for (n, c) in model.claims() {
        match c.freshness {
            Freshness::Stale => out.push(Proposal {
                id: format!("reverify:{}", c.id),
                class: "reverify".into(),
                subject: n.id.clone(),
                action: format!(
                    "{}: read {} again, correct the record if it no longer holds, then accept",
                    c.reason.as_deref().unwrap_or("stale"),
                    c.evidence.join(", ")
                ),
                owner: n.ownership,
                applies_by: "accept".into(),
            }),
            Freshness::Unverified if c.provenance == Provenance::Curated && c.state != super::model::ClaimState::Unverified => {
                out.push(Proposal {
                    id: format!("reverify:{}", c.id),
                    class: "reverify".into(),
                    subject: n.id.clone(),
                    action: format!("verify `{}` against {} and accept", c.predicate, c.evidence.join(", ")),
                    owner: n.ownership,
                    applies_by: "accept".into(),
                })
            }
            Freshness::PossiblyStale if c.provenance == Provenance::Derived => out.push(Proposal {
                id: format!("review:{}", c.id),
                class: "review".into(),
                subject: n.id.clone(),
                action: format!("a provider derived `{}`; confirm it and accept, or drop it from the cache", c.predicate),
                owner: Ownership::External,
                applies_by: "accept".into(),
            }),
            _ => {}
        }
    }
    for g in &model.gaps {
        let (class, applies_by) = match g.category {
            GapCategory::UnresolvedReference => ("fix_reference", "edit"),
            GapCategory::UndocumentedComponent => ("document", "edit"),
            GapCategory::UnexercisedCapability => ("exercise", "edit"),
            GapCategory::UnverifiedKnowledge => continue,
            GapCategory::Canonicality => ("derive", "edit"),
        };
        out.push(Proposal {
            id: format!("{class}:{}", g.subject),
            class: class.into(),
            subject: g.subject.clone(),
            action: format!("{}: {}", g.reason, g.remedy),
            owner: Ownership::External,
            applies_by: applies_by.into(),
        });
    }
    out.sort_by(|a, b| a.class.cmp(&b.class).then(a.subject.cmp(&b.subject)).then(a.id.cmp(&b.id)));
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// Reconcile: propose, and on `accept` verify every curated and derived content claim
/// against its present evidence in the baseline. Answers the reconciliation and the
/// baseline to write, when accepting changed it.
pub fn reconcile(model: &KnowledgeModel, baseline: &Baseline, accept: bool) -> (Reconciliation, Option<Baseline>) {
    let proposals = propose(model);
    let acceptable = proposals.iter().filter(|p| p.applies_by == "accept").count();
    if !accept {
        return (
            Reconciliation {
                proposals,
                acceptable,
                accepted: vec![],
                baseline_written: false,
            },
            None,
        );
    }
    let mut next = baseline.clone();
    let mut accepted = Vec::new();
    for (_, c) in model.claims() {
        if !super::freshness::needs_verification(c) || c.state == super::model::ClaimState::Conflicted {
            continue;
        }
        for ev in &c.evidence {
            let Some(e) = model.evidence(ev) else { continue };
            let entry = VerifiedClaim {
                claim: c.id.clone(),
                evidence: ev.clone(),
                fingerprint: e.fingerprint.value.clone(),
            };
            match next.verified.iter_mut().find(|v| v.claim == entry.claim && v.evidence == entry.evidence) {
                Some(v) if v.fingerprint == entry.fingerprint => {}
                Some(v) => {
                    v.fingerprint = entry.fingerprint;
                    accepted.push(c.id.clone());
                }
                None => {
                    next.verified.push(entry);
                    accepted.push(c.id.clone());
                }
            }
        }
    }
    accepted.sort();
    accepted.dedup();
    let changed = next != *baseline;
    (
        Reconciliation {
            proposals,
            acceptable,
            accepted,
            baseline_written: changed,
        },
        changed.then_some(next),
    )
}
