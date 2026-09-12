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
///
/// The derived `Ord` is load-bearing: the variants are declared in authority order, so
/// `<` means "spent before" and "harder to drop", and the budget's ordering is a plain
/// `sort` rather than a table mapping tiers to ranks. A variant added in the wrong place
/// would silently re-rank the whole answer, which is why the declaration order is the one
/// thing about this enum to be careful with.
///
/// ```
/// use majordomus_cli::devcontext::Tier;
///
/// // the declaration order *is* the order the budget spends in
/// let mut sorted = Tier::ORDER.to_vec();
/// sorted.sort();
/// assert_eq!(sorted, Tier::ORDER.to_vec());
///
/// // so the strongest authority is the smallest, and it is the one never dropped
/// assert_eq!(Tier::ORDER.iter().copied().min(), Some(Tier::Task));
/// assert!(Tier::Task < Tier::Governance && Tier::Governance < Tier::History);
///
/// // and the serialised word is the lowercase one the report is read under
/// assert_eq!(
///     serde_json::to_value(Tier::Governance).unwrap(),
///     serde_json::json!(Tier::Governance.as_str()),
/// );
/// ```
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
    ///
    /// This is carried into the answer — [`TierSpend::meaning`](super::TierSpend) is this
    /// string — rather than left for a consumer to look up, because the per-tier spend is
    /// read by people who are deciding what to raise the budget for and "knowledge: 4
    /// excluded" is not actionable without knowing what the tier holds.
    ///
    /// ```
    /// use majordomus_cli::devcontext::Tier;
    ///
    /// // it explains the tier rather than naming it, so it is never just the word again
    /// for t in Tier::ORDER {
    ///     assert!(t.meaning().len() > t.as_str().len(), "{} restates itself", t.as_str());
    /// }
    ///
    /// // and it says what is actually in there
    /// assert!(Tier::Governance.meaning().contains("policy"));
    /// assert!(Tier::Decision.meaning().contains("architectural decisions"));
    /// ```
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
///
/// The division that matters is [`Selector::declared`], and it is deliberately lopsided:
/// six of the seven selectors follow something the repository states, and exactly one
/// guesses. That ratio is the design. Inference is confined to a single selector so that
/// the inferred part of any answer can be found, weighed and ignored, rather than being
/// mixed through the rest at unmarked strength.
///
/// ```
/// use majordomus_cli::devcontext::Selector;
///
/// // exactly one selector infers, and the answer can always be split on it
/// let inferred: Vec<&str> = Selector::ALL
///     .iter()
///     .filter(|s| !s.declared())
///     .map(|s| s.as_str())
///     .collect();
/// assert_eq!(inferred, ["intent_match"]);
///
/// // every selector has its own word, so a reason is never ambiguous
/// let mut words: Vec<&str> = Selector::ALL.iter().map(|s| s.as_str()).collect();
/// let before = words.len();
/// words.sort();
/// words.dedup();
/// assert_eq!(words.len(), before);
///
/// // and the word is the serialised form
/// assert_eq!(
///     serde_json::to_value(Selector::ScopePath).unwrap(),
///     serde_json::json!("scope_path"),
/// );
/// ```
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
///
/// A `Discovery` is a *kind* of way in, not one traversal. Thirty-three issues of one
/// milestone all reach that milestone along `belongs_to`, and recording that thirty-three
/// times would say the answer had thirty-three reasons when it has one — so a path already
/// recorded for the same selector, edge and reason is kept rather than repeated.
///
/// `via` and `edge` are present exactly when they mean something: a seed was not reached
/// from anything and followed no edge, so both are absent and are skipped on the wire
/// rather than serialised as null. `confidence` is 1.0 for every declared selector and less
/// only for [`Selector::IntentMatch`]; `weight` is what this particular path contributes to
/// relevance, which is a different quantity — how much it counts, against how sure it is.
///
/// ```
/// use majordomus_cli::devcontext::{Discovery, Selector};
/// use serde_json::json;
///
/// // the request named it: nothing to come via, no edge followed, depth zero
/// let seed: Discovery = serde_json::from_value(json!({
///     "selector": "seed",
///     "reason": "named in the request",
///     "depth": 0,
///     "confidence": 1.0,
///     "weight": 1.0,
/// })).unwrap();
/// assert_eq!(seed.selector, Selector::Seed);
/// assert!(seed.via.is_none() && seed.edge.is_none());
/// assert!(seed.selector.declared() && seed.confidence == 1.0);
///
/// // the wire form omits them rather than carrying nulls a reader must skip
/// let wire = serde_json::to_value(&seed).unwrap();
/// assert!(wire.get("via").is_none() && wire.get("edge").is_none());
///
/// // a relation says both where it came from and which edge it followed
/// let walked: Discovery = serde_json::from_value(json!({
///     "selector": "relation",
///     "reason": "the decision in force over this scope",
///     "via": "majordomus://issue/I0301",
///     "edge": "governed_by",
///     "depth": 1,
///     "confidence": 1.0,
///     "weight": 0.9,
/// })).unwrap();
/// assert_eq!(walked.edge.as_deref(), Some("governed_by"));
/// assert_eq!(walked.via.as_deref(), Some("majordomus://issue/I0301"));
///
/// // and a guess says so, in the one field that distinguishes the two halves of an answer
/// let guessed: Discovery = serde_json::from_value(json!({
///     "selector": "intent_match",
///     "reason": "the intent's words occur in it",
///     "depth": 2,
///     "confidence": 0.4,
///     "weight": 0.4,
/// })).unwrap();
/// assert!(!guessed.selector.declared() && guessed.confidence < 1.0);
/// ```
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
///
/// It is a copy of the index's provenance and not a re-derivation of it, which is the
/// property worth relying on: no path here was constructed by this module, so a path in an
/// answer is a path the index discovered, and the two cannot disagree about where something
/// lives.
///
/// `bytes` is the measured half of an entry's cost — the estimate on top of it is
/// [`tokens_for`] — so this is also where the budget's arithmetic starts.
///
/// ```
/// use majordomus_cli::devcontext::{tokens_for, EntryProvenance};
/// use serde_json::json;
///
/// let p: EntryProvenance = serde_json::from_value(json!({
///     "path": ".ai/repo/adrs/0052-the-episode-is-the-boundary.md",
///     "directory": ".ai/repo/adrs",
///     "source_class": "adr",
///     "section": "adrs",
///     "bytes": 8_192,
/// })).unwrap();
///
/// // the size is what the cost is computed from, and the estimate says so of itself
/// assert_eq!(tokens_for(p.bytes), 2_048);
///
/// // a file under no manifest section — a root README, say — carries none rather than ""
/// let rootish: EntryProvenance = serde_json::from_value(json!({
///     "path": "README.md",
///     "directory": ".",
///     "source_class": "readme",
///     "bytes": 4_000,
/// })).unwrap();
/// assert!(rootish.section.is_none());
/// assert_eq!(rootish.directory, ".", "the root is `.`, not the empty string");
/// ```
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
///
/// Notably, it does **not** carry the prose. The identifier is here and the body is not,
/// because the body belongs to the object and `objects.get` serves it whole; copying it
/// would put the layer in the answer twice and flatten the structure into text before
/// anything had decided what to keep. An entry is a decision about an object, not a copy of
/// one.
///
/// Three of the scalars are derived from `discovered_by` rather than independent of it, and
/// a reader can check them: `confidence` is the highest of any path's, `depth` the shortest,
/// and a `discovered_by` of length above one is the evidence that deduplication happened.
/// `required` is the veto — a blocking rule, the policy, the scope, or a seed — and it is
/// the one field that overrides the budget.
///
/// ```
/// use majordomus_cli::devcontext::{tokens_for, ContextEntry, Selector, Tier};
/// use serde_json::json;
///
/// // one ADR, reached twice: once along a declared edge and once by matching the intent
/// let e: ContextEntry = serde_json::from_value(json!({
///     "uri": "majordomus://adr/0052",
///     "kind": "adr",
///     "title": "The episode is the boundary",
///     "provenance": {
///         "path": ".ai/repo/adrs/0052.md", "directory": ".ai/repo/adrs",
///         "source_class": "adr", "section": "adrs", "bytes": 8_192,
///     },
///     "tier": "decision",
///     "relevance": 0.82,
///     "confidence": 1.0,
///     "discovered_by": [
///         {"selector": "relation", "reason": "in force over this scope",
///          "via": "majordomus://issue/I0301", "edge": "governed_by",
///          "depth": 1, "confidence": 1.0, "weight": 0.9},
///         {"selector": "intent_match", "reason": "the intent's words occur in it",
///          "depth": 2, "confidence": 0.4, "weight": 0.4},
///     ],
///     "depth": 1,
///     "cost_tokens": 2_048,
///     "cost_bytes": 8_192,
///     "version": "2026-09-11",
///     "status": "accepted",
///     "required": false,
/// })).unwrap();
///
/// assert_eq!(e.tier, Tier::Decision);
///
/// // the cost is the estimate over the measured bytes, and the answer carries both halves
/// assert_eq!(e.cost_tokens, tokens_for(e.cost_bytes));
///
/// // the derived scalars agree with the paths they are derived from
/// let best = e.discovered_by.iter().map(|d| d.confidence).fold(0.0_f64, f64::max);
/// assert_eq!(e.confidence, best);
/// assert_eq!(e.depth, e.discovered_by.iter().map(|d| d.depth).min().unwrap());
///
/// // two paths for one entry: the record that four things pointing here is one entry
/// assert_eq!(e.discovered_by.len(), 2);
/// assert_eq!(e.discovered_by[0].selector, Selector::Relation, "best path first");
///
/// // and the prose is not here: the identifier is how a caller gets the body
/// assert!(e.uri.starts_with("majordomus://"));
/// ```
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
///
/// The six divide into three that a caller can act on by asking again — `Budget`,
/// `Relevance` and `Depth` are all consequences of the request's own limits, so raising the
/// limit brings the entry back — and three that are facts about the repository:
/// `Stale` and `Superseded` mean the layer itself says this is not the current thing, and
/// `Unresolved` means the layer names something it does not hold, which is a dangling
/// reference somebody should fix.
///
/// ```
/// use majordomus_cli::devcontext::ExclusionReason as Why;
///
/// // the ones a different request would change, against the ones it would not
/// let request_bound = [Why::Budget, Why::Relevance, Why::Depth];
/// let repository_bound = [Why::Stale, Why::Superseded, Why::Unresolved];
/// for r in repository_bound {
///     assert!(!request_bound.contains(&r), "{} is not the caller's to fix", r.as_str());
/// }
///
/// // and the word is the serialised form, snake_case for the two-word one
/// assert_eq!(Why::Superseded.as_str(), "superseded");
/// assert_eq!(
///     serde_json::to_value(Why::Unresolved).unwrap(),
///     serde_json::json!(Why::Unresolved.as_str()),
/// );
/// ```
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
///
/// `cost_tokens` is why this type exists in the shape it does. A budget that reported only
/// *that* it dropped things would leave a caller guessing at a new ceiling; one that reports
/// what each omission would have cost turns "raise the budget" into arithmetic. `detail`
/// carries the same in words, so the row is legible without the reader doing that
/// arithmetic themselves.
///
/// The ordering puts the tier first as an explicit rank, because the tier is the precedence
/// the budget spent in; inside a tier a reader is looking for a reason, and the identifier
/// ends the key so the list is total and the same tree always prints it the same way.
///
/// ```
/// use majordomus_cli::devcontext::{Excluded, ExclusionReason, Tier};
/// use majordomus_cli::order::canonical;
/// use serde_json::json;
///
/// let row = |uri: &str, tier: &str, reason: &str, cost: u64| -> Excluded {
///     serde_json::from_value(json!({
///         "uri": uri, "tier": tier, "reason": reason,
///         "detail": "as measured", "cost_tokens": cost, "relevance": 0.4,
///     })).unwrap()
/// };
///
/// let mut excluded = vec![
///     row("majordomus://knowledge/b", "history", "budget", 900),
///     row("majordomus://adr/0001", "decision", "superseded", 300),
///     row("majordomus://knowledge/a", "history", "budget", 1_200),
/// ];
/// canonical(&mut excluded);
///
/// // the tier ranks first, so the decision tier precedes history whatever the reason says
/// assert_eq!(excluded[0].tier, Tier::Decision);
/// assert_eq!(excluded[0].reason, ExclusionReason::Superseded);
/// // then the reason, then the identifier — which makes the order total
/// let rest: Vec<&str> = excluded[1..].iter().map(|e| e.uri.as_str()).collect();
/// assert_eq!(rest, ["majordomus://knowledge/a", "majordomus://knowledge/b"]);
///
/// // and the row a caller acts on says what it would have cost to keep
/// let biggest = excluded.iter().max_by_key(|e| e.cost_tokens).unwrap();
/// assert_eq!(biggest.cost_tokens, 1_200);
/// ```
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
///
/// One type covers two different collapses, and `dropped` is how a reader tells them apart.
/// **Structural** collapse is the same identifier reached several ways: nothing was
/// discarded, so `dropped` is empty and `paths` is the count that makes the row worth
/// printing — this is the record that four things pointing at one ADR produced one entry.
/// **Semantic** collapse is two different identifiers standing for one thing, one file
/// indexed under two kinds or one rule at two versions: there `dropped` names what lost,
/// because something really was discarded and a reader is entitled to know what.
///
/// ```
/// use majordomus_cli::devcontext::Deduplicated;
/// use serde_json::json;
///
/// // structural: one identifier, four ways in, nothing discarded
/// let structural: Deduplicated = serde_json::from_value(json!({
///     "kept": "majordomus://adr/0052",
///     "dropped": [],
///     "key": "uri",
///     "detail": "one object reached by 4 discovery path(s)",
///     "paths": 4,
/// })).unwrap();
/// assert!(structural.dropped.is_empty(), "nothing lost; this is a record, not a decision");
/// assert!(structural.paths > 1, "which is what makes the row worth printing");
/// assert_eq!(structural.key, "uri");
///
/// // semantic: two identifiers for one rule, and the older version lost
/// let semantic: Deduplicated = serde_json::from_value(json!({
///     "kept": "majordomus://rule/project.x@2",
///     "dropped": ["majordomus://rule/project.x@1"],
///     "key": "identity",
///     "detail": "one rule at two versions; the newer is in the answer",
///     "paths": 1,
/// })).unwrap();
/// assert_eq!(semantic.dropped, ["majordomus://rule/project.x@1"]);
/// assert_ne!(semantic.kept, semantic.dropped[0], "two names, and one of them lost");
/// ```
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
///
/// The compiler could have resolved every disagreement by keeping one side, and for
/// deduplication it does. A conflict is what is left when both sides are legitimately in
/// the answer and they still do not agree — so the resolution belongs to the session
/// reading it, and the compiler's job is to make sure the disagreement is visible rather
/// than implied by the order of two rows.
///
/// `current` is the side the compiler treats as current, which is a statement about
/// recency, not correctness: `supersedes` means the layer declared one to replace the other,
/// `version` means two versions of one thing are both here.
///
/// ```
/// use majordomus_cli::devcontext::Conflict;
/// use majordomus_cli::order::canonical;
/// use serde_json::json;
///
/// let row = |current: &str, against: &str, kind: &str| -> Conflict {
///     serde_json::from_value(json!({
///         "current": current, "against": against, "kind": kind,
///         "detail": "both are in the answer and they disagree",
///     })).unwrap()
/// };
///
/// let mut conflicts = vec![
///     row("majordomus://rule/project.x@2", "majordomus://rule/project.x@1", "version"),
///     row("majordomus://adr/0052", "majordomus://adr/0031", "supersedes"),
/// ];
/// canonical(&mut conflicts);
///
/// // filed under the kind of disagreement, so like reads with like
/// let kinds: Vec<&str> = conflicts.iter().map(|c| c.kind.as_str()).collect();
/// assert_eq!(kinds, ["supersedes", "version"]);
///
/// // and both sides are always named: the pair is the fact, not the winner alone
/// for c in &conflicts {
///     assert_ne!(c.current, c.against);
///     assert!(!c.detail.is_empty());
/// }
/// ```
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

/// What one tier cost, how much of it survived, and how much of that could not have been
/// dropped.
///
/// There is no per-tier ceiling, which is worth saying because the name suggests one: the
/// budget is a single pot spent in tier order, and this is the *report* of what each tier
/// took from it, not an allowance it was given. A tier with many exclusions is therefore
/// not a tier that hit its own limit — it is a tier that was reached after the pot had been
/// spent by the tiers above it.
///
/// `meaning` is carried so that a reader deciding what to raise the budget for does not
/// have to look up what `knowledge` holds; `required` is the part of `selected` the budget
/// had no say over.
///
/// ```
/// use majordomus_cli::devcontext::{Tier, TierSpend};
/// use serde_json::json;
///
/// let t: TierSpend = serde_json::from_value(json!({
///     "tier": "governance",
///     "meaning": Tier::Governance.meaning(),
///     "selected": 9,
///     "excluded": 2,
///     "tokens": 6_400,
///     "required": 3,
/// })).unwrap();
///
/// assert_eq!(t.tier, Tier::Governance);
/// // the undroppable part is a part of what survived, never a separate count
/// assert!(t.required <= t.selected);
/// // and the row explains its own tier, so "2 excluded" is actionable
/// assert_eq!(t.meaning, Tier::Governance.meaning());
/// assert!(t.meaning.contains("policy"));
/// ```
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
///
/// `over_budget` is the field to read and the one most likely to be misread. It does not
/// mean the compiler overspent by accident: every entry the budget has a say over is refused
/// the moment it would not fit, so the only way past the ceiling is through the entries that
/// may not be dropped — the seeds, the policy, the scope, a blocking rule. So
/// `over_budget` means *the things this context cannot do without already cost more than you
/// allowed*, and `required_tokens` is the explanation of it. The honest answer to that is to
/// say so, not to return a context missing the thing it was compiled about.
///
/// `remaining_tokens` is floored at zero rather than going negative, so it is safe to
/// display; the overspend is read from `used_tokens` against `limit_tokens`, which is why
/// both are carried.
///
/// ```
/// use majordomus_cli::devcontext::{Budget, Tier, BYTES_PER_TOKEN};
/// use serde_json::json;
///
/// let tier = |name: &str, tokens: u64, required: usize| json!({
///     "tier": name, "meaning": "as this tier is for", "selected": 1,
///     "excluded": 0, "tokens": tokens, "required": required,
/// });
///
/// // the undroppable entries alone cost more than the caller allowed
/// let b: Budget = serde_json::from_value(json!({
///     "limit_tokens": 1_000,
///     "used_tokens": 2_500,
///     "required_tokens": 2_500,
///     "remaining_tokens": 0,
///     "over_budget": true,
///     "bytes_per_token": BYTES_PER_TOKEN,
///     "tiers": [tier("task", 2_000, 1), tier("governance", 500, 1)],
/// })).unwrap();
///
/// assert!(b.over_budget);
/// assert!(b.used_tokens > b.limit_tokens, "the overspend is read from these two");
/// assert_eq!(b.required_tokens, b.used_tokens, "all of it was undroppable");
/// assert_eq!(b.remaining_tokens, 0, "floored, so it is safe to display");
///
/// // the divisor is carried so a caller measuring tokens differently can convert
/// // rather than guess what the estimate meant
/// assert_eq!(b.bytes_per_token, BYTES_PER_TOKEN);
///
/// // and the tiers are reported in the order the budget spends in
/// let order: Vec<Tier> = b.tiers.iter().map(|t| t.tier).collect();
/// assert_eq!(order, [Tier::Task, Tier::Governance]);
/// assert!(order.windows(2).all(|w| w[0] < w[1]));
/// ```
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
///
/// `branch` and `head` are optional because a checkout can legitimately have neither — a
/// detached head, or a tree that is not a git repository at all — and the absence is carried
/// rather than faked. `working_tree` is not optional, because there is always something to
/// say: when git cannot be reached it carries `unavailable: <reason>` instead of a status,
/// so a reader is never left unable to tell "clean" from "not asked".
///
/// `commits` is derived from the selected entries' own records and never from a log walk.
/// That is the difference between "the commits this work says it produced" and "what
/// happened to be committed recently", and only the first is context.
///
/// ```
/// use majordomus_cli::devcontext::GitContext;
/// use serde_json::json;
///
/// // a tree the compiler could read
/// let known: GitContext = serde_json::from_value(json!({
///     "branch": "master",
///     "head": "cd04012f1b1dc21fd493e2be14938ffdfaccc8fa",
///     "working_tree": "clean",
/// })).unwrap();
/// assert_eq!(known.branch.as_deref(), Some("master"));
/// assert!(known.commits.is_empty(), "no selected record named a commit");
///
/// // and one it could not: the status field still says something a reader can act on
/// let unknown: GitContext = serde_json::from_value(json!({
///     "working_tree": "unavailable: not a git repository",
/// })).unwrap();
/// assert!(unknown.head.is_none() && unknown.branch.is_none());
/// assert!(
///     unknown.working_tree.starts_with("unavailable: "),
///     "never silently indistinguishable from clean",
/// );
/// ```
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
///
/// The pair is the fact, not the commit alone. One commit named by three records is three
/// references, because each record is a separate claim about what that commit was for, and
/// folding them would lose the attribution — which is the only reason to carry a commit here
/// at all. So `named_by` ends the ordering key, and two references to one commit both
/// survive.
///
/// ```
/// use majordomus_cli::devcontext::CommitReference;
/// use majordomus_cli::order::canonical;
/// use serde_json::json;
///
/// let by = |commit: &str, record: &str| -> CommitReference {
///     serde_json::from_value(json!({"commit": commit, "named_by": record})).unwrap()
/// };
///
/// let mut commits = vec![
///     by("cd04012", "majordomus://session/b"),
///     by("cd04012", "majordomus://session/a"),
///     by("5e31dd4", "majordomus://issue/I0301"),
/// ];
/// canonical(&mut commits);
///
/// // filed under the commit, which is what a reader looks for
/// let order: Vec<&str> = commits.iter().map(|c| c.commit.as_str()).collect();
/// assert_eq!(order, ["5e31dd4", "cd04012", "cd04012"]);
///
/// // and the record ends the key, so one commit named twice stays two facts
/// assert_eq!(commits[1].named_by, "majordomus://session/a");
/// assert_eq!(commits[2].named_by, "majordomus://session/b");
///
/// // `covers` is what the record said the commit was for, when it said
/// let with_reason = by("cd04012", "majordomus://session/a");
/// assert!(with_reason.covers.is_none(), "absent rather than an empty sentence");
/// ```
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
///
/// A seed is the request echoed back after resolution, and it is in the answer so that a
/// caller can see what their short name turned into. `from` names the request field it came
/// from — `issue`, `milestone`, `uris`, `paths`, `intent` — which is how the same identifier
/// arriving two ways stays two seeds rather than one.
///
/// `resolved` is populated only when resolution actually changed something: name a
/// milestone by its slug and it carries the canonical identity, name it by that identity and
/// there was nothing to resolve, so the field is absent. A path and an intent are not
/// resolved against the index at all — their `uri` is the literal the request gave.
///
/// ```
/// use majordomus_cli::devcontext::Seed;
/// use serde_json::json;
///
/// // named by a slug: the answer says what it became
/// let by_slug: Seed = serde_json::from_value(json!({
///     "uri": "majordomus://milestone/M04",
///     "kind": "milestone",
///     "from": "milestone",
///     "resolved": "M04",
/// })).unwrap();
/// assert_eq!(by_slug.resolved.as_deref(), Some("M04"));
///
/// // named canonically: nothing was resolved, so nothing is reported
/// let canonical: Seed = serde_json::from_value(json!({
///     "uri": "majordomus://issue/I0301",
///     "kind": "issue",
///     "from": "issue",
/// })).unwrap();
/// assert!(canonical.resolved.is_none());
///
/// // and a path is carried verbatim: its `uri` is not a canonical identifier at all
/// let path: Seed = serde_json::from_value(json!({
///     "uri": "apps/majordomus-cli/src",
///     "kind": "path",
///     "from": "paths",
/// })).unwrap();
/// assert!(!path.uri.starts_with("majordomus://"));
/// assert_eq!(path.from, "paths", "which request field it came from");
/// ```
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
///
/// Two invariants hold over every answer, and they are what make it safe to cache and to
/// compare. `fingerprint` is over the index this was compiled from, so the same request
/// against the same fingerprint is the same answer and a different fingerprint is a
/// different tree — a caller can tell two trees apart without asking. And
/// `budget.used_tokens` is exactly the sum of `selected`'s costs, so the arithmetic in the
/// budget describes the list beside it rather than a walk that has since been filtered.
///
/// ```
/// use majordomus_cli::devcontext::{compile, CompileInput, CompiledContext};
/// use majordomus_cli::devcontext::{Tier, DEFAULT_BUDGET_TOKENS};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
///
/// let c: CompiledContext = compile(&ctx, CompileInput::default()).unwrap();
///
/// // the answer says which tree it is true of, and nothing else has to be asked
/// assert_eq!(c.schema, 1);
/// assert!(!c.fingerprint.is_empty(), "an answer that names no tree is not one");
///
/// // the budget describes the list beside it
/// let spent: u64 = c.selected.iter().map(|e| e.cost_tokens).sum();
/// assert_eq!(c.budget.used_tokens, spent);
/// assert_eq!(c.budget.limit_tokens, DEFAULT_BUDGET_TOKENS);
///
/// // every tier is reported, so an empty tier is visibly empty rather than missing
/// assert_eq!(c.budget.tiers.len(), Tier::ORDER.len());
///
/// // a request that named nothing has no seeds and is still an answer
/// assert!(c.seeds.is_empty());
///
/// // and recompiling the same request over the same tree gives the same answer
/// let again: CompiledContext = compile(&ctx, CompileInput::default()).unwrap();
/// assert_eq!(again.fingerprint, c.fingerprint);
/// assert_eq!(again.selected.len(), c.selected.len());
/// ```
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
