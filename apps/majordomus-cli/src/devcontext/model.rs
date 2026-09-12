//! What a compiled development context is made of.
//!
//! Every type here is a value, not a rendering. A [`ContextEntry`] carries the canonical
//! identifier of the thing it selects, where that thing came from, which selector reached
//! it and why, how confident the compiler is, how much of the budget it costs — and never
//! the prose. The prose belongs to the object, which `objects.get` serves whole; copying it
//! here would put the layer in the answer twice and would flatten the structure into text
//! before anything has decided what to keep.
//!
//! ```
//! use majordomus_cli::devcontext::{Selector, Tier};
//!
//! // the tiers are a total order, and the order is the budget's order
//! assert!(Tier::Task < Tier::Governance);
//! assert!(Tier::Governance < Tier::History);
//!
//! // a selector says whether what it produced was declared or inferred
//! assert!(Selector::Relation.declared());
//! assert!(!Selector::IntentMatch.declared());
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where an entry sits in the order the budget spends in.
///
/// The order is this repository's own: `lib/context.sh` assembles its sections "in
/// authority order, highest first, because a worker that runs out of budget must lose the
/// least reliable evidence rather than the most: git, then the task and its policy, then
/// blockers, then authored records, then event history." These six tiers are that
/// sentence, applied to objects instead of to sections.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// What was asked for: the issue, its milestone, the paths named, the intent. Never
    /// dropped — a context that lost the thing it was compiled about is not a smaller
    /// answer, it is the wrong one.
    Task,
    /// What the work will be judged against: the policy, the scope, the directory
    /// contracts that reach the named paths, and the rules. A blocking rule is never
    /// dropped.
    Governance,
    /// The code and the cases the work touches.
    Source,
    /// The architectural decisions already in force over it.
    Decision,
    /// What a previous session left: handovers, session records, curated knowledge.
    Knowledge,
    /// Supporting evidence: use cases, claims, features, and the rest of the layer.
    History,
}

impl Tier {
    /// Every tier, in the order the budget spends in.
    ///
    /// ```
    /// use majordomus_cli::devcontext::Tier;
    /// assert_eq!(Tier::ORDER.len(), 6);
    /// assert_eq!(Tier::ORDER[0], Tier::Task);
    /// ```
    pub const ORDER: &'static [Tier] = &[
        Tier::Task,
        Tier::Governance,
        Tier::Source,
        Tier::Decision,
        Tier::Knowledge,
        Tier::History,
    ];

    /// The word this tier is reported under.
    ///
    /// ```
    /// use majordomus_cli::devcontext::Tier;
    /// assert_eq!(Tier::Governance.as_str(), "governance");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Task => "task",
            Tier::Governance => "governance",
            Tier::Source => "source",
            Tier::Decision => "decision",
            Tier::Knowledge => "knowledge",
            Tier::History => "history",
        }
    }

    /// What this tier is for, in one line, so that a reader of the answer never has to
    /// find this file.
    pub fn meaning(self) -> &'static str {
        match self {
            Tier::Task => "what was asked for: the issue, its milestone, the paths named, the intent",
            Tier::Governance => "what the work is judged against: the policy, the scope, the directory contracts, the rules",
            Tier::Source => "the code and the cases the work touches",
            Tier::Decision => "the architectural decisions already in force over it",
            Tier::Knowledge => "what a previous session left: handovers, session records, curated knowledge",
            Tier::History => "supporting evidence: use cases, claims, features, and the rest of the layer",
        }
    }
}

/// What reached an entry. This is the provenance of the *selection*, and it is a different
/// fact from the provenance of the file: `Relation` says the compiler followed an edge the
/// layer declares, `IntentMatch` says it guessed from words. An answer that did not keep
/// them apart could not say which half of itself to trust.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Selector {
    /// Named in the request.
    Seed,
    /// Reached along a typed edge of the composed graph.
    Relation,
    /// Its path lies under a path the request or a seed declares.
    ScopePath,
    /// A directory contract that reaches one of those paths.
    Contract,
    /// A rule or policy the layer applies to everything.
    Governance,
    /// A recorded session of this branch or of these paths, the handover the resolver
    /// chose, the newest checkpoint.
    SessionState,
    /// Inferred: the words of the intent occur in the object's own words.
    IntentMatch,
}

impl Selector {
    /// The word this selector is reported under.
    ///
    /// ```
    /// use majordomus_cli::devcontext::Selector;
    /// assert_eq!(Selector::ScopePath.as_str(), "scope_path");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Selector::Seed => "seed",
            Selector::Relation => "relation",
            Selector::ScopePath => "scope_path",
            Selector::Contract => "contract",
            Selector::Governance => "governance",
            Selector::SessionState => "session_state",
            Selector::IntentMatch => "intent_match",
        }
    }

    /// Did the repository state this, or did the compiler infer it? A declared selector
    /// carries confidence 1.0; an inferred one carries less, and says how much.
    ///
    /// ```
    /// use majordomus_cli::devcontext::Selector;
    /// assert!(Selector::Contract.declared());
    /// assert!(!Selector::IntentMatch.declared());
    /// ```
    pub fn declared(self) -> bool {
        !matches!(self, Selector::IntentMatch)
    }

    /// Every selector, in a stable order.
    pub const ALL: &'static [Selector] = &[
        Selector::Seed,
        Selector::Relation,
        Selector::ScopePath,
        Selector::Contract,
        Selector::Governance,
        Selector::SessionState,
        Selector::IntentMatch,
    ];
}

/// One way an entry was reached: which selector, why, how far from a seed, and how much
/// the compiler trusts it. An entry keeps every one of these, because "five discovery
/// paths found the same ADR" is the interesting fact, not a thing to hide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Discovery {
    /// What reached it.
    pub selector: Selector,
    /// Why it was included, in words a person reads without this file.
    pub reason: String,
    /// The entry this one was reached from, when it was reached from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    /// The edge kind followed, for `Relation`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<String>,
    /// Hops from a seed; a seed is 0.
    pub depth: usize,
    /// 1.0 when the repository declared this path, less when it was inferred.
    pub confidence: f64,
    /// What this path contributes to the entry's relevance.
    pub weight: f64,
}

/// Where the selected thing lives, as the index knows it. Nothing here is authored: every
/// field is the index's own [`crate::model::Provenance`], carried rather than restated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntryProvenance {
    /// Repository-relative path, forward slashes.
    pub path: String,
    /// The directory it sits in.
    pub directory: String,
    /// The `sources.yaml` class that discovered it.
    pub source_class: String,
    /// The manifest section it falls under, when it falls under one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Size of the file in bytes.
    pub bytes: u64,
}

/// One thing the compiler decided a session should be given: what it is, how it was
/// reached, what it costs, and how far the compiler trusts it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ContextEntry {
    /// The canonical identifier: `majordomus://<kind>/<identity>`, unique in the answer.
    pub uri: String,
    /// The kind the layer declared for it.
    pub kind: String,
    /// Its title, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Where it lives.
    pub provenance: EntryProvenance,
    /// Which tier's budget it is spent from.
    pub tier: Tier,
    /// 0.0 to 1.0, deterministic for a given tree and request. The order within a tier.
    pub relevance: f64,
    /// The highest confidence of any path that reached it.
    pub confidence: f64,
    /// Every path that reached it, best first. Length above one is the evidence that
    /// deduplication happened.
    pub discovered_by: Vec<Discovery>,
    /// The shortest number of hops from a seed.
    pub depth: usize,
    /// Estimated tokens: the file's bytes divided by [`BYTES_PER_TOKEN`], never fewer
    /// than one. An estimate, said to be one.
    pub cost_tokens: u64,
    /// The file's bytes, the measured half of the cost.
    pub cost_bytes: u64,
    /// The version or date the object states about itself, when it states one: a rule's
    /// version, an ADR's date, an issue's `updated_at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The status word the object declares, when it declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// True when a blocking rule, a policy or the scope: never dropped for budget.
    pub required: bool,
    /// Facts worth carrying about this entry that are not the object's prose — a rule's
    /// class, an issue's milestone, the commits an issue's evidence names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub facts: BTreeMap<String, String>,
}

/// Why something the compiler selected did not survive.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    /// The budget ran out before its tier was filled.
    Budget,
    /// Its relevance fell below the floor the request set.
    Relevance,
    /// It is beyond the depth the request allows.
    Depth,
    /// The object says it is deprecated or superseded.
    Stale,
    /// A newer version of the same thing is in the answer.
    Superseded,
    /// The layer names it and holds nothing under that name.
    Unresolved,
}

impl ExclusionReason {
    /// The word this reason is reported under.
    ///
    /// ```
    /// use majordomus_cli::devcontext::ExclusionReason;
    /// assert_eq!(ExclusionReason::Budget.as_str(), "budget");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ExclusionReason::Budget => "budget",
            ExclusionReason::Relevance => "relevance",
            ExclusionReason::Depth => "depth",
            ExclusionReason::Stale => "stale",
            ExclusionReason::Superseded => "superseded",
            ExclusionReason::Unresolved => "unresolved",
        }
    }
}

/// One thing the compiler reached and did not give. Excluded is not the same as absent:
/// an under-filled context is debugged from this list, not by guessing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Excluded {
    /// The canonical identifier, or the name the layer used when nothing resolves.
    pub uri: String,
    /// Its kind, when it resolved to something.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The tier it would have been spent from.
    pub tier: Tier,
    /// Why it is not in the answer.
    pub reason: ExclusionReason,
    /// The same, in words.
    pub detail: String,
    /// What it would have cost, so that a reader can decide whether to raise the budget.
    pub cost_tokens: u64,
    /// What its relevance was.
    pub relevance: f64,
}

/// The tier is the precedence the budget spends in, so it is the explicit rank; inside a
/// tier a reader looks for a reason, and the identifier ends the key so the list is total.
impl crate::order::Ordered for Excluded {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(self.reason.as_str(), &self.uri).ranked(self.tier as i64)
    }
}

/// Two names, one thing. The compiler keeps one entry and records the collapse here, so
/// that "why is this ADR listed once when four things point at it" has an answer that is
/// state rather than a log line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Deduplicated {
    /// The entry that survived.
    pub kept: String,
    /// What was folded into it, or dropped for it.
    pub dropped: Vec<String>,
    /// What made them the same thing: `uri`, `path` or `identity`.
    pub key: String,
    /// The same, in words.
    pub detail: String,
    /// How many discovery paths reached the surviving entry.
    pub paths: usize,
}

/// Filed under what made the two the same thing, then by the entry that survived — which
/// is unique, because one entry is kept once under one key.
impl crate::order::Ordered for Deduplicated {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::grouped(&self.key, &self.kept, &self.kept)
    }
}

/// Two things in the answer that do not agree, named rather than silently ordered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Conflict {
    /// The entry the compiler treats as current.
    pub current: String,
    /// The entry it stands against.
    pub against: String,
    /// What kind of disagreement: `supersedes` or `version`.
    pub kind: String,
    /// The same, in words.
    pub detail: String,
}

/// Filed under the kind of disagreement, then the current entry, and ended by the entry it
/// stands against — the pair is what makes one conflict distinct from another under a kind.
impl crate::order::Ordered for Conflict {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::grouped(&self.kind, &self.current, &self.against)
    }
}

/// What one tier cost and what it was allowed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TierSpend {
    /// The tier.
    pub tier: Tier,
    /// What it is for.
    pub meaning: String,
    /// How many entries survived in it.
    pub selected: usize,
    /// How many were dropped from it for budget.
    pub excluded: usize,
    /// Tokens the survivors cost.
    pub tokens: u64,
    /// Entries in it that cannot be dropped.
    pub required: usize,
}

/// The budget, and what became of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Budget {
    /// The ceiling this answer was compiled under, in estimated tokens.
    pub limit_tokens: u64,
    /// What the survivors cost.
    pub used_tokens: u64,
    /// What the entries that cannot be dropped cost. Above the limit, the answer is
    /// over-budget and says so rather than dropping what it must keep.
    pub required_tokens: u64,
    /// `limit - used`, floored at zero.
    pub remaining_tokens: u64,
    /// True when the required entries alone exceed the limit.
    pub over_budget: bool,
    /// Bytes per token, the divisor the estimate uses.
    pub bytes_per_token: u64,
    /// Per tier, in the order the budget spends in.
    pub tiers: Vec<TierSpend>,
}

/// The head this answer was compiled against. A compiled context is only true of one tree,
/// and these three fields plus the index fingerprint are what say which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitContext {
    /// The branch, when the checkout is on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The head commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// `clean`, `dirty`, or what git said.
    pub working_tree: String,
    /// Commits the selected entries name as their own evidence, newest record first.
    /// Derived from the layer's records, never from a log walk.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<CommitReference>,
}

/// One commit the layer itself names, and the record that named it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct CommitReference {
    /// The commit as the record wrote it.
    pub commit: String,
    /// The entry that named it.
    pub named_by: String,
    /// What the record says the commit was for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covers: Option<String>,
}

/// The commit is what a reader looks for; the entry that named it ends the key, because one
/// commit is named by several records and each of those is a separate fact.
impl crate::order::Ordered for CommitReference {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.commit, &self.named_by)
    }
}

/// What the request asked to compile a context about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Seed {
    /// The canonical identifier of the seed, or the literal the request gave for an
    /// intent or a path.
    pub uri: String,
    /// `issue`, `milestone`, `path`, `intent`, or the kind of the object named.
    pub kind: String,
    /// Where it came from: which field of the request.
    pub from: String,
    /// What the compiler resolved it to, when the request named something by a short name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
}

/// A compiled development context: what to give a session, what was left out, what
/// collapsed into what, and the budget it was all decided under.
///
/// This is the canonical form. Rendering it as a prompt is a downstream projection of this
/// value and is not performed here — the structure is what lets a caller re-budget, re-order
/// or explain the selection without recompiling it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CompiledContext {
    /// The format of this answer.
    pub schema: u64,
    /// The index this was compiled from, over every object's path and content. Two answers
    /// with the same fingerprint and the same request are the same answer; a different
    /// fingerprint is a different tree.
    pub fingerprint: String,
    /// The head the answer is true of.
    pub git: GitContext,
    /// What was asked for.
    pub seeds: Vec<Seed>,
    /// What to give the session, in the order the budget spent: tier, then relevance, then
    /// canonical identifier.
    pub selected: Vec<ContextEntry>,
    /// What was reached and not given, with the reason for each.
    pub excluded: Vec<Excluded>,
    /// What collapsed into what.
    pub deduplicated: Vec<Deduplicated>,
    /// What does not agree with what.
    pub conflicts: Vec<Conflict>,
    /// The budget and its per-tier spend.
    pub budget: Budget,
    /// What the compiler could not read: a knowledge source the layer names and does not
    /// hold, a session record that no longer describes this checkout. Never an error: an
    /// absent source is an answer.
    pub diagnostics: Vec<crate::model::Diagnostic>,
}

/// Bytes of a file per estimated token. Four is the working figure for English prose and
/// for the YAML and Markdown this layer is made of; the answer says which divisor it used
/// so that a caller measuring differently can convert rather than guess.
pub const BYTES_PER_TOKEN: u64 = 4;

/// The default ceiling, in estimated tokens.
///
/// It is a token budget, not the shell builder's line budget: an entry here is a whole
/// object, and objects differ in size by two orders of magnitude, so lines would ration
/// the wrong quantity. The figure is the same order as the shell builder's 300 lines once
/// bodies are counted, and the request overrides it.
pub const DEFAULT_BUDGET_TOKENS: u64 = 24_000;

/// The furthest the compiler walks from a seed unless the request says otherwise.
pub const DEFAULT_MAX_DEPTH: usize = 3;

/// Estimated tokens for a file of this many bytes: never fewer than one, so that an entry
/// always costs something.
///
/// ```
/// use majordomus_cli::devcontext::tokens_for;
/// assert_eq!(tokens_for(0), 1);
/// assert_eq!(tokens_for(4), 1);
/// assert_eq!(tokens_for(4001), 1001);
/// ```
pub fn tokens_for(bytes: u64) -> u64 {
    bytes.div_ceil(BYTES_PER_TOKEN).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tiers_are_the_order_the_budget_spends_in() {
        // every tier appears exactly once, sorted, and each says what it is for
        let mut sorted = Tier::ORDER.to_vec();
        sorted.sort();
        assert_eq!(sorted, Tier::ORDER.to_vec());
        for t in Tier::ORDER {
            assert!(!t.meaning().is_empty(), "{} says nothing", t.as_str());
            assert!(!t.as_str().is_empty());
        }
    }

    #[test]
    fn a_selector_says_whether_it_declared_or_guessed() {
        let inferred: Vec<&str> = Selector::ALL
            .iter()
            .filter(|s| !s.declared())
            .map(|s| s.as_str())
            .collect();
        assert_eq!(inferred, vec!["intent_match"]);
        // and every selector has a distinct word
        let mut words: Vec<&str> = Selector::ALL.iter().map(|s| s.as_str()).collect();
        words.sort();
        let before = words.len();
        words.dedup();
        assert_eq!(words.len(), before);
    }

    #[test]
    fn the_token_estimate_never_costs_nothing() {
        assert_eq!(tokens_for(0), 1);
        assert_eq!(tokens_for(1), 1);
        assert_eq!(tokens_for(8), 2);
        assert!(tokens_for(u64::MAX) > 0);
    }
}
