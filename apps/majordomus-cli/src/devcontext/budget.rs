//! Deduplication, conflict, deterministic order, and what the budget could afford.
//!
//! The three happen in that order and none of them re-reads the repository. What arrives is
//! the [`Selection`](super::select::Selection) — one candidate per canonical identifier,
//! already carrying every path that reached it — and what leaves is the answer, with an
//! entry for everything kept and a reason for everything not.
//!
//! # Why deduplication is two rules and not one
//!
//! Structural duplication is the same identifier reached several ways, and it is already
//! folded by the time this module runs: the interesting output is the *record* that it
//! happened, so that "why is this ADR listed once when four things point at it" is
//! answerable. Semantic duplication is two different identifiers standing for one thing —
//! one file indexed under two kinds, or one rule at two versions — and that has to be
//! decided, because keeping both would spend the budget twice on one fact and would leave
//! the session holding two answers.
//!
//! # First fit, not truncation
//!
//! Entries are spent in one total order — tier, then relevance descending, then canonical
//! identifier — and an entry that does not fit is excluded while the walk continues. A
//! budget that stopped at the first thing too large would let one big file hide a hundred
//! small relevant ones; first fit spends the ceiling and names everything it skipped, with
//! what it would have cost, so raising the budget is an informed decision.

use std::collections::{BTreeMap, BTreeSet};

use crate::index::Index;

use super::model::{
    tokens_for, Budget, Conflict, ContextEntry, Deduplicated, EntryProvenance, Excluded,
    ExclusionReason, Tier, TierSpend, BYTES_PER_TOKEN,
};
use super::select::{version_of, Candidate, Selection};

/// What the budget produced.
pub struct Spend {
    /// What to give the session, in the order it was spent.
    pub selected: Vec<ContextEntry>,
    /// What was reached and not given.
    pub excluded: Vec<Excluded>,
    /// What collapsed into what.
    pub deduplicated: Vec<Deduplicated>,
    /// What does not agree with what.
    pub conflicts: Vec<Conflict>,
    /// The ceiling and the per-tier spend.
    pub budget: Budget,
}

/// The status words that mean a thing is no longer current.
const STALE: &[&str] = &["deprecated", "superseded", "rejected"];

/// Is this entry one the budget may not drop? The task's own seeds, and the two objects
/// that decide what every session is held to.
fn required_of(c: &Candidate, seeds: &BTreeSet<String>) -> bool {
    seeds.contains(&c.uri) || matches!(c.kind.as_str(), "policy" | "scope")
}

/// The bytes an entry costs. An object carries its own size; anything else is measured
/// once, here, from the path the layer named — never by walking for it.
fn bytes_of(c: &Candidate, index: &Index) -> u64 {
    if let Some(i) = c.object {
        return index.objects[i].provenance.bytes;
    }
    if let Some(b) = c.facts.get("bytes").and_then(|b| b.parse::<u64>().ok()) {
        return b;
    }
    match &c.path {
        Some(p) => std::fs::metadata(std::path::Path::new(&index.repository.root).join(p))
            .map(|m| m.len())
            .unwrap_or(0),
        None => 0,
    }
}

/// The kind that wins when one file is indexed under two: the specific one. `document` and
/// `file` are what the layer falls back to when nothing more precise claimed a path, so
/// they lose to anything that did.
fn specificity(kind: &str) -> u8 {
    match kind {
        "file" => 0,
        "document" => 1,
        _ => 2,
    }
}

/// The identity of a versioned object without its version: `project.x@2` is `project.x`.
fn stem(uri: &str) -> (&str, Option<u32>) {
    match uri.rsplit_once('@') {
        Some((head, v)) => (head, v.parse().ok()),
        None => (uri, None),
    }
}

/// Compile the selection into the answer: fold, decide, order, spend.
pub fn spend(
    index: &Index,
    sel: Selection,
    seeds: &BTreeSet<String>,
    limit: u64,
    supersedes: &BTreeMap<String, Vec<String>>,
) -> (Spend, Vec<crate::model::Diagnostic>) {
    let mut diagnostics = sel.diagnostics;
    let mut candidates = sel.candidates;
    let mut deduplicated = Vec::new();
    let mut conflicts = Vec::new();
    let mut excluded: Vec<Excluded> = Vec::new();

    // ---- structural: the record of what was already folded by identifier
    for c in candidates.values() {
        if c.discovered_by.len() < 2 {
            continue;
        }
        let mut ways: Vec<String> = c
            .discovered_by
            .iter()
            .map(|d| match &d.edge {
                Some(e) => format!("{}({e})", d.selector.as_str()),
                None => d.selector.as_str().to_string(),
            })
            .collect();
        ways.sort();
        ways.dedup();
        deduplicated.push(Deduplicated {
            kept: c.uri.clone(),
            dropped: Vec::new(),
            key: "uri".into(),
            detail: format!(
                "one object reached by {} discovery path(s): {}",
                c.discovered_by.len(),
                ways.join(", ")
            ),
            paths: c.discovered_by.len(),
        });
    }

    // ---- semantic: one file indexed under two kinds
    let mut by_path: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in candidates.values() {
        if let Some(p) = &c.path {
            by_path.entry(p.clone()).or_default().push(c.uri.clone());
        }
    }
    let mut drop: BTreeSet<String> = BTreeSet::new();
    for (path, uris) in &by_path {
        if uris.len() < 2 {
            continue;
        }
        // the most specific kind wins; a tie goes to the higher relevance, then to the
        // identifier, so the choice is the same on every run
        let mut ranked: Vec<&Candidate> = uris.iter().filter_map(|u| candidates.get(u)).collect();
        ranked.sort_by(|a, b| {
            specificity(&b.kind)
                .cmp(&specificity(&a.kind))
                .then(
                    b.relevance
                        .partial_cmp(&a.relevance)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(a.uri.cmp(&b.uri))
        });
        let kept = ranked[0].uri.clone();
        let lost: Vec<String> = ranked[1..].iter().map(|c| c.uri.clone()).collect();
        for u in &lost {
            drop.insert(u.clone());
        }
        deduplicated.push(Deduplicated {
            kept: kept.clone(),
            dropped: lost.clone(),
            key: "path".into(),
            detail: format!(
                "`{path}` is indexed under {} identifiers; the most specific kind is kept",
                uris.len()
            ),
            paths: 1,
        });
    }

    // ---- semantic: one thing at two versions
    let mut by_stem: BTreeMap<&str, Vec<(String, Option<u32>)>> = BTreeMap::new();
    for uri in candidates.keys() {
        let (s, v) = stem(uri);
        if v.is_some() {
            by_stem.entry(s).or_default().push((uri.clone(), v));
        }
    }
    for (s, mut versions) in by_stem {
        if versions.len() < 2 {
            continue;
        }
        versions.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let kept = versions[0].0.clone();
        for (uri, _) in &versions[1..] {
            drop.insert(uri.clone());
            conflicts.push(Conflict {
                current: kept.clone(),
                against: uri.clone(),
                kind: "version".into(),
                detail: format!(
                    "`{s}` is in the answer at two versions; the higher one is what applies"
                ),
            });
        }
        deduplicated.push(Deduplicated {
            kept,
            dropped: versions[1..].iter().map(|(u, _)| u.clone()).collect(),
            key: "identity".into(),
            detail: format!("`{s}` at {} versions", versions.len()),
            paths: 1,
        });
    }

    // ---- what stands in for what: a decision that supersedes another leaves the other out
    for (later, earlier) in supersedes {
        if !candidates.contains_key(later) {
            continue;
        }
        for e in earlier {
            if !candidates.contains_key(e) || seeds.contains(e) {
                continue;
            }
            drop.insert(e.clone());
            conflicts.push(Conflict {
                current: later.clone(),
                against: e.clone(),
                kind: "supersedes".into(),
                detail: format!("`{later}` stands in for `{e}`; the later decision is what holds"),
            });
        }
    }

    // the dropped ones leave the answer, and say so
    for uri in &drop {
        if let Some(c) = candidates.remove(uri) {
            let bytes = bytes_of(&c, index);
            let superseded = conflicts.iter().any(|k| &k.against == uri);
            excluded.push(Excluded {
                uri: c.uri.clone(),
                kind: Some(c.kind.clone()),
                tier: c.tier,
                reason: if superseded {
                    ExclusionReason::Superseded
                } else {
                    ExclusionReason::Unresolved
                },
                detail: match conflicts.iter().find(|k| &k.against == uri) {
                    Some(k) => k.detail.clone(),
                    None => "another identifier in the answer stands for the same file".into(),
                },
                cost_tokens: tokens_for(bytes),
                relevance: c.relevance,
            });
        }
    }

    // ---- stale: what the object says about itself. A seed is kept however it is marked —
    // the request asked for it — and the marking becomes a diagnostic instead.
    let stale: Vec<String> = candidates
        .values()
        .filter(|c| c.status.as_deref().is_some_and(|s| STALE.contains(&s)))
        .map(|c| c.uri.clone())
        .collect();
    for uri in stale {
        let Some(c) = candidates.get(&uri) else {
            continue;
        };
        let status = c.status.clone().unwrap_or_default();
        if seeds.contains(&uri) {
            diagnostics.push(crate::model::Diagnostic::warning(
                "stale_seed",
                c.path.clone(),
                format!("`{uri}` is `{status}` and was named in the request; it is kept anyway"),
            ));
            continue;
        }
        let c = candidates.remove(&uri).expect("just read");
        let bytes = bytes_of(&c, index);
        excluded.push(Excluded {
            uri,
            kind: Some(c.kind),
            tier: c.tier,
            reason: ExclusionReason::Stale,
            detail: format!("the object declares `status: {status}`"),
            cost_tokens: tokens_for(bytes),
            relevance: c.relevance,
        });
    }

    // ---- the one total order, and the spend
    let mut ordered: Vec<Candidate> = candidates.into_values().collect();
    ordered.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then(
                b.relevance
                    .partial_cmp(&a.relevance)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.uri.cmp(&b.uri))
    });

    let mut selected: Vec<ContextEntry> = Vec::new();
    let mut used = 0u64;
    let mut required_tokens = 0u64;
    for c in ordered {
        let bytes = bytes_of(&c, index);
        let cost = tokens_for(bytes);
        let required = required_of(&c, seeds);
        if required {
            required_tokens += cost;
        }
        if !required && used + cost > limit {
            excluded.push(Excluded {
                uri: c.uri.clone(),
                kind: Some(c.kind.clone()),
                tier: c.tier,
                reason: ExclusionReason::Budget,
                detail: format!(
                    "{cost} token(s) would not fit: {used} of {limit} already spent, {} left",
                    limit.saturating_sub(used)
                ),
                cost_tokens: cost,
                relevance: c.relevance,
            });
            continue;
        }
        used += cost;
        let mut discovered_by = c.discovered_by;
        discovered_by.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.depth.cmp(&b.depth))
                .then(a.selector.cmp(&b.selector))
                .then(a.reason.cmp(&b.reason))
        });
        let confidence = discovered_by
            .iter()
            .map(|d| d.confidence)
            .fold(0.0_f64, f64::max);
        let (provenance, version) = match c.object {
            Some(i) => {
                let o = &index.objects[i];
                (
                    EntryProvenance {
                        path: o.provenance.path.clone(),
                        directory: o.provenance.directory.clone(),
                        source_class: o.provenance.source_class.clone(),
                        section: o.provenance.section.clone(),
                        bytes,
                    },
                    version_of(o),
                )
            }
            None => {
                let path = c.path.clone().unwrap_or_else(|| c.uri.clone());
                let directory = path
                    .rsplit_once('/')
                    .map(|(d, _)| d.to_string())
                    .unwrap_or_else(|| ".".into());
                (
                    EntryProvenance {
                        path,
                        directory,
                        // not discovered by a source class: named by the layer, or read
                        // from this checkout's own local state
                        source_class: if c.uri.starts_with("local:") {
                            "local-state".into()
                        } else {
                            "named".into()
                        },
                        section: None,
                        bytes,
                    },
                    c.facts.get("created_at").cloned(),
                )
            }
        };
        selected.push(ContextEntry {
            uri: c.uri,
            kind: c.kind,
            title: c.title,
            provenance,
            tier: c.tier,
            relevance: round(c.relevance),
            confidence: round(confidence),
            discovered_by,
            depth: c.depth,
            cost_tokens: cost,
            cost_bytes: bytes,
            version,
            status: c.status,
            required,
            facts: c.facts,
        });
    }

    excluded.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then(a.reason.cmp(&b.reason))
            .then(a.uri.cmp(&b.uri))
    });
    deduplicated.sort_by(|a, b| a.key.cmp(&b.key).then(a.kept.cmp(&b.kept)));
    conflicts.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.current.cmp(&b.current))
            .then(a.against.cmp(&b.against))
    });

    let tiers = Tier::ORDER
        .iter()
        .map(|t| TierSpend {
            tier: *t,
            meaning: t.meaning().to_string(),
            selected: selected.iter().filter(|e| e.tier == *t).count(),
            excluded: excluded
                .iter()
                .filter(|e| e.tier == *t && e.reason == ExclusionReason::Budget)
                .count(),
            tokens: selected
                .iter()
                .filter(|e| e.tier == *t)
                .map(|e| e.cost_tokens)
                .sum(),
            required: selected
                .iter()
                .filter(|e| e.tier == *t && e.required)
                .count(),
        })
        .collect();

    for (reference, detail) in sel.unresolved {
        diagnostics.push(crate::model::Diagnostic::warning(
            "unresolved_reference",
            None,
            format!("`{reference}` is named and not held: {detail}"),
        ));
    }
    diagnostics.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .reverse()
            .then(a.code.cmp(&b.code))
            .then(a.message.cmp(&b.message))
    });
    diagnostics.dedup();

    (
        Spend {
            selected,
            excluded,
            deduplicated,
            conflicts,
            budget: Budget {
                limit_tokens: limit,
                used_tokens: used,
                required_tokens,
                remaining_tokens: limit.saturating_sub(used),
                // The answer is over budget when it *is* over budget. That happens only
                // through the entries it may not drop — the seeds and the two objects every
                // session is held to — because every other entry is refused the moment it
                // would not fit. `required_tokens` is the explanation; this is the fact.
                over_budget: used > limit,
                bytes_per_token: BYTES_PER_TOKEN,
                tiers,
            },
        },
        diagnostics,
    )
}

/// Three decimals: enough to order by, few enough that the same tree gives the same digits
/// on every platform.
fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_split_off_the_identity() {
        assert_eq!(
            stem("majordomus://rule/project.x@2"),
            ("majordomus://rule/project.x", Some(2))
        );
        assert_eq!(
            stem("majordomus://adr/adr-0001"),
            ("majordomus://adr/adr-0001", None)
        );
        // an @ with something unparseable after it is not a version
        assert_eq!(stem("a@b").1, None);
    }

    #[test]
    fn the_specific_kind_beats_the_fallback_kinds() {
        assert!(specificity("rule") > specificity("document"));
        assert!(specificity("document") > specificity("file"));
        assert_eq!(specificity("adr"), specificity("issue"));
    }

    #[test]
    fn rounding_is_stable_and_orderable() {
        assert_eq!(round(0.8 * 0.85), 0.68);
        assert_eq!(round(1.0), 1.0);
        assert!(round(0.6666666) < round(0.6669999) + 0.001);
    }
}
