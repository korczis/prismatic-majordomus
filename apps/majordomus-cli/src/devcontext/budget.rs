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
//!
//! That last paragraph is the one worth seeing run, because the difference between first
//! fit and truncation is invisible in a report and total in effect. Below, the most
//! relevant candidate costs twice the whole ceiling: a truncating budget would have
//! stopped there and answered with nothing, and this one skips it, says what it would have
//! cost, and carries on to spend the ceiling on what does fit.
//!
//! ```
//! use std::collections::{BTreeMap, BTreeSet};
//!
//! use majordomus_cli::devcontext::budget::spend;
//! use majordomus_cli::devcontext::select::{Candidate, Selection};
//! use majordomus_cli::devcontext::{Discovery, ExclusionReason, Selector, Tier};
//! use majordomus_cli::synthetic::SyntheticRepository;
//!
//! let repo = SyntheticRepository::small().unwrap();
//! let index = repo.index().unwrap();
//!
//! // `bytes` as a fact rather than a file: the cost is the one thing this example pins
//! let candidate = |uri: &str, relevance: f64, bytes: u64| Candidate {
//!     uri: uri.into(),
//!     kind: "knowledge".into(),
//!     title: None,
//!     tier: Tier::Knowledge,
//!     relevance,
//!     depth: 1,
//!     discovered_by: vec![Discovery {
//!         selector: Selector::Relation,
//!         reason: "reached along a declared edge".into(),
//!         via: None,
//!         edge: Some("depends_on".into()),
//!         depth: 1,
//!         confidence: 1.0,
//!         weight: 0.9,
//!     }],
//!     object: None,
//!     path: None,
//!     status: None,
//!     facts: BTreeMap::from([("bytes".to_string(), bytes.to_string())]),
//! };
//!
//! let mut candidates = BTreeMap::new();
//! // 8000 bytes is 2000 tokens: twice the ceiling, and the more relevant of the two
//! candidates.insert("big".to_string(), candidate("majordomus://knowledge/big", 0.9, 8_000));
//! candidates.insert("small".to_string(), candidate("majordomus://knowledge/small", 0.5, 400));
//! let selection = Selection { candidates, unresolved: Vec::new(), diagnostics: Vec::new() };
//!
//! let (s, _) = spend(&index, selection, &BTreeSet::new(), 1_000, &BTreeMap::new());
//!
//! // the walk did not stop at the thing that would not fit
//! let kept: Vec<&str> = s.selected.iter().map(|e| e.uri.as_str()).collect();
//! assert_eq!(kept, ["majordomus://knowledge/small"]);
//!
//! // and what it skipped is named, with what it would have cost
//! assert_eq!(s.excluded.len(), 1);
//! assert_eq!(s.excluded[0].uri, "majordomus://knowledge/big");
//! assert_eq!(s.excluded[0].reason, ExclusionReason::Budget);
//! assert_eq!(s.excluded[0].cost_tokens, 2_000, "so raising the ceiling is informed");
//!
//! // the ceiling was respected, and `used` is the sum of what is in the answer
//! assert_eq!(s.budget.used_tokens, 100);
//! assert_eq!(s.budget.remaining_tokens, 900);
//! assert!(!s.budget.over_budget);
//! ```

use std::collections::{BTreeMap, BTreeSet};

use crate::index::Index;

use super::model::{
    tokens_for, Budget, Conflict, ContextEntry, Deduplicated, EntryProvenance, Excluded,
    ExclusionReason, Tier, TierSpend, BYTES_PER_TOKEN,
};
use super::select::{version_of, Candidate, Selection};

/// Everything the budget decided, as four lists and the arithmetic behind them: what the
/// session is given, what it is not and why, what collapsed into what, and what disagrees
/// with what.
///
/// The four lists are disjoint views of one walk rather than four separate answers, and the
/// one invariant that joins them to the budget is that `budget.used_tokens` is exactly the
/// sum of `selected`'s costs — nothing is spent that is not in the answer, and nothing is in
/// the answer that was not spent. An under-filled context is debugged from `excluded`, not
/// by guessing, which is why it carries a reason and a cost per row.
///
/// Every field is populated on every call, including with nothing reached: the per-tier
/// spend always has all six tiers, so a reader never has to distinguish "this tier was
/// empty" from "this tier is missing from the answer".
///
/// ```
/// use std::collections::{BTreeMap, BTreeSet};
///
/// use majordomus_cli::devcontext::budget::{spend, Spend};
/// use majordomus_cli::devcontext::select::Selection;
/// use majordomus_cli::devcontext::{Tier, DEFAULT_BUDGET_TOKENS};
/// use majordomus_cli::model::Diagnostic;
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let index = repo.index().unwrap();
/// let nothing = Selection {
///     candidates: BTreeMap::new(),
///     unresolved: Vec::new(),
///     diagnostics: Vec::new(),
/// };
///
/// // the answer and what could not be read, side by side and neither an error
/// let (s, diagnostics): (Spend, Vec<Diagnostic>) = spend(
///     &index, nothing, &BTreeSet::new(), DEFAULT_BUDGET_TOKENS, &BTreeMap::new(),
/// );
///
/// // reaching nothing is an answer, not an error, and it is a complete one
/// assert!(s.selected.is_empty() && s.excluded.is_empty());
/// assert!(s.deduplicated.is_empty() && s.conflicts.is_empty());
/// assert!(diagnostics.is_empty());
///
/// // the per-tier spend is always total, so no tier is merely absent
/// let tiers: Vec<Tier> = s.budget.tiers.iter().map(|t| t.tier).collect();
/// assert_eq!(tiers, Tier::ORDER.to_vec());
///
/// // and the invariant that ties the lists to the arithmetic
/// let spent: u64 = s.selected.iter().map(|e| e.cost_tokens).sum();
/// assert_eq!(s.budget.used_tokens, spent);
/// assert_eq!(s.budget.remaining_tokens, DEFAULT_BUDGET_TOKENS);
/// ```
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
///
/// The four stages run in that order and none of them re-reads the repository. `seeds` is
/// what the request asked about, and it is the one input that can push the answer over its
/// ceiling: a seed is never dropped for budget, and neither is the policy or the scope, so
/// `limit` bounds what the budget *chooses* and not what it must keep. When the entries it
/// may not drop cost more than the limit, the answer says `over_budget` rather than
/// returning a context missing the thing it was compiled about.
///
/// `supersedes` maps an identifier to the identifiers it declares it supersedes, and is
/// what turns two versions of one thing in the answer into one entry and a recorded
/// conflict. The diagnostics that come back alongside are what could not be read — never an
/// error, because an absent source is an answer.
///
/// ```
/// use std::collections::{BTreeMap, BTreeSet};
///
/// use majordomus_cli::devcontext::budget::spend;
/// use majordomus_cli::devcontext::select::{Candidate, Selection};
/// use majordomus_cli::devcontext::{Discovery, Selector, Tier};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let index = repo.index().unwrap();
///
/// let seed_uri = "majordomus://issue/I0301";
/// let candidate = Candidate {
///     uri: seed_uri.into(),
///     kind: "issue".into(),
///     title: Some("the thing being worked on".into()),
///     tier: Tier::Task,
///     relevance: 1.0,
///     depth: 0,
///     discovered_by: vec![Discovery {
///         selector: Selector::Seed,
///         reason: "named in the request".into(),
///         via: None,
///         edge: None,
///         depth: 0,
///         confidence: 1.0,
///         weight: 1.0,
///     }],
///     object: None,
///     path: None,
///     status: None,
///     // 40_000 bytes is 10_000 tokens, against a ceiling of 10
///     facts: BTreeMap::from([("bytes".to_string(), "40000".to_string())]),
/// };
/// let selection = Selection {
///     candidates: BTreeMap::from([(seed_uri.to_string(), candidate)]),
///     unresolved: Vec::new(),
///     diagnostics: Vec::new(),
/// };
/// let seeds = BTreeSet::from([seed_uri.to_string()]);
///
/// let (s, _) = spend(&index, selection, &seeds, 10, &BTreeMap::new());
///
/// // the seed survives a ceiling it does not fit in: a context that lost the thing it was
/// // compiled about is not a smaller answer, it is the wrong one
/// assert_eq!(s.selected.len(), 1);
/// assert!(s.selected[0].required);
/// assert!(s.excluded.is_empty());
///
/// // and the answer says so rather than pretending it stayed inside the budget
/// assert!(s.budget.over_budget);
/// assert_eq!(s.budget.used_tokens, 10_000);
/// assert_eq!(s.budget.required_tokens, 10_000, "all of it is undroppable");
/// assert_eq!(s.budget.remaining_tokens, 0, "floored, never negative");
/// ```
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
        crate::order::canonical_strings(&mut ways);
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

    crate::order::canonical(&mut excluded);
    crate::order::canonical(&mut deduplicated);
    crate::order::canonical(&mut conflicts);

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
