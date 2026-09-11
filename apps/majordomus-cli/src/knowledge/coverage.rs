//! Coverage: how much of what the repository has is known about, over denominators this
//! executable can define deterministically. No percentage is computed; the numbers are
//! the answer, and every missing item is named so that the report is a worklist.
//!
//! The rows, and the gap each missing item becomes:
//!
//! | row | denominator | covered when | gap |
//! |---|---|---|---|
//! | components documented | every component the manifests declare | a document or a curated record references, describes or documents it, or its manifest | undocumented component |
//! | capabilities exercised | every query and command of the registry | it has at least one benchmark case | unexercised capability |
//! | references resolved | every typed reference the layer declares | it resolves | (the reference is already an unresolved gap) |
//! | curated knowledge verified | every curated record | its freshness is current | unverified knowledge |
//! | artifacts derived | every generated artifact the manifest declares | it names what it was derived from | (a canonicality gap) |
//! | layer objects reached | every object of the layer | something references it, or it references something | — (reported, not a gap) |

use std::collections::{BTreeMap, BTreeSet};

use super::model::{ClaimState, Coverage, CoverageRow, Freshness, GapCategory, KnowledgeModel, Provenance};
use super::{Adjacency, Inputs};

/// Compute the coverage rows and the gaps that follow from them.
pub fn compute(model: &mut KnowledgeModel, inputs: &Inputs<'_>) {
    let adjacency = Adjacency::of(&model.relations);
    let mut rows = Vec::new();
    let mut gaps = Vec::new();

    // components documented
    {
        let documenting: BTreeSet<&str> = ["references", "describes", "documents", "derived_from"].into();
        let mut missing = Vec::new();
        let mut total = 0;
        for n in model.nodes.iter().filter(|n| n.kind == "component") {
            total += 1;
            let manifest = n.source.as_deref().map(|p| format!("file:{p}"));
            // the component's directory: a README that links to `apps/x/` documents x
            let directory = n
                .source
                .as_deref()
                .and_then(|p| p.rsplit_once('/'))
                .map(|(d, _)| format!("directory:{d}"));
            let covered = adjacency
                .incoming
                .get(n.id.as_str())
                .into_iter()
                .flatten()
                .chain(manifest.as_deref().and_then(|m| adjacency.incoming.get(m)).into_iter().flatten())
                .chain(directory.as_deref().and_then(|d| adjacency.incoming.get(d)).into_iter().flatten())
                .any(|r| {
                    documenting.contains(r.kind.as_str())
                        && model
                            .node(&r.source)
                            .is_some_and(|s| matches!(s.kind.as_str(), "document" | "knowledge" | "adr" | "application" | "use-case"))
                });
            if !covered {
                missing.push(n.id.clone());
                gaps.push((
                    GapCategory::UndocumentedComponent,
                    n.id.clone(),
                    format!("no document, decision or curated record references {}", n.id),
                    "link the component from a README or a document, or write a curated record about it".to_string(),
                ));
            }
        }
        rows.push(CoverageRow {
            id: "components-documented".into(),
            title: "Components documented".into(),
            denominator: "every component the manifests declare".into(),
            discovered: total,
            covered: total - missing.len(),
            missing,
        });
    }

    // capabilities exercised
    {
        let ctx = crate::capability::benchmark::CaseContext { index: inputs.index };
        let mut missing = Vec::new();
        let mut total = 0;
        for n in model.nodes.iter().filter(|n| n.kind == "capability") {
            total += 1;
            let id = n.id.trim_start_matches("capability:");
            let cases = inputs
                .registry
                .cases(id)
                .map(|f| f(&ctx).len())
                .unwrap_or(0);
            if cases == 0 {
                missing.push(n.id.clone());
                gaps.push((
                    GapCategory::UnexercisedCapability,
                    n.id.clone(),
                    format!("{id} has no benchmark case, so nothing exercises it"),
                    "give its input type a `BenchmarkCases` implementation with one case".to_string(),
                ));
            }
        }
        rows.push(CoverageRow {
            id: "capabilities-exercised".into(),
            title: "Capabilities exercised".into(),
            denominator: "every query and command of the registry".into(),
            discovered: total,
            covered: total - missing.len(),
            missing,
        });
    }

    // references resolved
    {
        let mut missing = Vec::new();
        let mut total = 0;
        for (n, c) in model.claims().filter(|(_, c)| c.predicate == "reference") {
            total += 1;
            if c.state == ClaimState::Unverified {
                missing.push(format!("{}: {}", n.id, c.value.as_str().unwrap_or("")));
            }
        }
        rows.push(CoverageRow {
            id: "references-resolved".into(),
            title: "References resolved".into(),
            denominator: "every typed reference the layer and the documents make".into(),
            discovered: total,
            covered: total - missing.len(),
            missing,
        });
    }

    // curated knowledge verified
    {
        let mut missing = Vec::new();
        let mut total = 0;
        for n in model.nodes.iter().filter(|n| n.provenance == Provenance::Curated && n.kind == "knowledge") {
            total += 1;
            if n.freshness != Freshness::Current {
                missing.push(n.id.clone());
                gaps.push((
                    GapCategory::UnverifiedKnowledge,
                    n.id.clone(),
                    format!(
                        "{} is {}{}",
                        n.id,
                        n.freshness.as_str(),
                        n.freshness_reason.as_deref().map(|r| format!(": {r}")).unwrap_or_default()
                    ),
                    "check the record against its evidence, then `majordomus knowledge reconcile --accept`".to_string(),
                ));
            }
        }
        rows.push(CoverageRow {
            id: "curated-verified".into(),
            title: "Curated knowledge verified".into(),
            denominator: "every curated record of the layer".into(),
            discovered: total,
            covered: total - missing.len(),
            missing,
        });
    }

    // artifacts derived
    {
        let mut missing = Vec::new();
        let mut total = 0;
        for n in model.nodes.iter().filter(|n| n.kind == "artifact") {
            total += 1;
            let derived = adjacency
                .outgoing
                .get(n.id.as_str())
                .into_iter()
                .flatten()
                .any(|r| r.kind == "derived_from");
            if !derived {
                missing.push(n.id.clone());
            }
        }
        rows.push(CoverageRow {
            id: "artifacts-derived".into(),
            title: "Generated artifacts with a declared source".into(),
            denominator: "every artifact the generation manifest declares".into(),
            discovered: total,
            covered: total - missing.len(),
            missing,
        });
    }

    // layer objects reached
    {
        let mut missing = Vec::new();
        let mut total = 0;
        let layer_kinds: BTreeSet<&str> = inputs.index.kinds().into_keys().collect();
        for n in model.nodes.iter().filter(|n| layer_kinds.contains(n.kind.as_str()) && n.kind != "context" && n.kind != "document") {
            total += 1;
            let reached = adjacency.incoming.get(n.id.as_str()).is_some_and(|v| !v.is_empty())
                || adjacency.outgoing.get(n.id.as_str()).is_some_and(|v| !v.is_empty());
            if !reached {
                missing.push(n.id.clone());
            }
        }
        rows.push(CoverageRow {
            id: "layer-objects-reached".into(),
            title: "Layer objects related to something".into(),
            denominator: "every typed object of the layer, contracts and plain documents aside".into(),
            discovered: total,
            covered: total - missing.len(),
            missing,
        });
    }

    model.coverage = Coverage { rows };
    let mut seen: BTreeMap<String, ()> = model.gaps.iter().map(|g| (g.id.clone(), ())).collect();
    for (category, subject, reason, remedy) in gaps {
        let id = format!("{}:{subject}", super::extract::gap_word(category));
        if seen.insert(id.clone(), ()).is_none() {
            model.gaps.push(super::model::Gap {
                id,
                category,
                subject,
                reason,
                remedy,
            });
        }
    }
    model.gaps.sort_by(|a, b| a.id.cmp(&b.id));
}
