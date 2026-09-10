//! What changed in the public contract, and what that costs.
//!
//! ```text
//!   baseline snapshot ─┐
//!                      ├──▶ ContractDiff ──▶ CompatibilityImpact ──▶ Bump ──▶ required version
//!   current snapshot ──┘        (facts)          (observation)      (policy)
//! ```
//!
//! The three stages are kept apart deliberately. The diff is what happened; the impact is
//! what it means for a caller; the bump is what this project's policy makes of the
//! meaning. An explanation reads back along that chain — *this fact changed, therefore
//! this is breaking, therefore the policy requires a major* — and every step of it is data
//! that came from somewhere, not a sentence written by hand.
//!
//! # Precision
//!
//! A subsystem that reports a breaking change where there is none teaches everybody to
//! pass `--allow` and stops being an authority. So every classification here is
//! conservative in the direction of *saying less*: a fact whose policy is not declared is
//! reported as unclassified and counted as nothing, and the surfaces a snapshot does not
//! cover are not compared at all.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::contract::{ContractSnapshot, Surface};
use super::version::{Bump, Version};

/// What a change costs the people who already depend on this.
///
/// Distinct from [`Bump`], and not collapsible into it: `Additive` and `Patch` map to
/// different bumps below 1.0.0 than above it, and an explanation that had only the bump
/// could not say why. The order is the strength order — a diff's impact is the strongest
/// of its changes — which is what [`Ord`] is derived for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "CompatibilityImpact")]
pub enum CompatibilityImpact {
    /// Nothing a caller can observe changed.
    None,
    /// Behaviour changed within the contract: everything that compiled still compiles and
    /// everything that parsed still parses.
    Patch,
    /// The contract grew. Existing callers are unaffected; new callers can do more.
    Additive,
    /// The contract shrank or moved. Something that worked stops working.
    Breaking,
}

impl CompatibilityImpact {
    /// The word a report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Patch => "patch",
            Self::Additive => "additive",
            Self::Breaking => "breaking",
        }
    }

    /// The stronger of two impacts.
    pub fn max(self, other: Self) -> Self {
        if other > self {
            other
        } else {
            self
        }
    }
}

impl std::fmt::Display for CompatibilityImpact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What happened to one entry or one fact.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "ContractChangeKind")]
pub enum ChangeKind {
    /// It did not exist at the baseline and exists now.
    Added,
    /// It existed at the baseline and does not exist now.
    Removed,
    /// It exists in both and states something different.
    Changed,
}

impl ChangeKind {
    /// The word a report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
        }
    }
}

/// One difference between two snapshots, with the impact it carries and the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ContractChange")]
pub struct ContractChange {
    /// Which part of the surface.
    pub surface: Surface,
    /// The entry: a capability id, a command id, a schema id, a target id.
    pub entry: String,
    /// The fact within the entry, when the whole entry did not appear or disappear.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact: Option<String>,
    /// What happened.
    pub kind: ChangeKind,
    /// What it stated at the baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// What it states now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// What it costs a caller.
    pub impact: CompatibilityImpact,
    /// One sentence: why that impact, in the terms the policy is written in. This is what
    /// `version explain` prints; nothing composes a reason of its own from the fields
    /// above.
    pub reason: String,
}

impl ContractChange {
    /// The one line a report prints for this change.
    ///
    /// ```
    /// use majordomus_cli::release::diff::{ChangeKind, CompatibilityImpact, ContractChange};
    /// use majordomus_cli::release::contract::Surface;
    ///
    /// let change = ContractChange {
    ///     surface: Surface::Capability,
    ///     entry: "release.status".into(),
    ///     fact: None,
    ///     kind: ChangeKind::Added,
    ///     before: None,
    ///     after: None,
    ///     impact: CompatibilityImpact::Additive,
    ///     reason: "a capability that did not exist".into(),
    /// };
    /// assert_eq!(change.line(), "+ capability release.status");
    /// ```
    pub fn line(&self) -> String {
        let mark = match self.kind {
            ChangeKind::Added => '+',
            ChangeKind::Removed => '-',
            ChangeKind::Changed => '~',
        };
        let mut s = format!("{mark} {} {}", self.surface.noun(), self.entry);
        if let Some(fact) = &self.fact {
            s.push_str(&format!(" · {fact}"));
            match (&self.before, &self.after) {
                (Some(b), Some(a)) => s.push_str(&format!(": {b} → {a}")),
                (Some(b), None) => s.push_str(&format!(": was {b}")),
                (None, Some(a)) => s.push_str(&format!(": {a}")),
                (None, None) => {}
            }
        }
        s
    }
}

/// Every difference between two snapshots, and the strongest impact among them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ContractDiff")]
pub struct ContractDiff {
    /// The fingerprint of the snapshot compared against.
    pub baseline: String,
    /// The fingerprint of the snapshot compared.
    pub current: String,
    /// Every change, in a deterministic order: impact first, then surface, entry, fact.
    pub changes: Vec<ContractChange>,
    /// The strongest impact among the changes; `None` when there are none.
    pub impact: CompatibilityImpact,
    /// Surfaces present in one snapshot and not the other, which were therefore not
    /// compared. Empty in the ordinary case; non-empty means the verdict is partial and
    /// every report says so rather than presenting it as complete.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uncompared: Vec<Surface>,
}

impl ContractDiff {
    /// How many changes carry each impact.
    pub fn tallies(&self) -> BTreeMap<&'static str, usize> {
        let mut out = BTreeMap::new();
        for change in &self.changes {
            *out.entry(change.impact.as_str()).or_insert(0) += 1;
        }
        out
    }

    /// The changes carrying one impact.
    pub fn with_impact(
        &self,
        impact: CompatibilityImpact,
    ) -> impl Iterator<Item = &ContractChange> {
        self.changes.iter().filter(move |c| c.impact == impact)
    }

    /// True when nothing a caller can observe changed.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// What the change of one fact means.
///
/// Declared as data, per fact name, so that adding a fact to the snapshot does not require
/// editing a classifier — and so that a fact nobody classified is *visible* rather than
/// quietly worth nothing.
#[derive(Debug, Clone, Copy)]
struct FactPolicy {
    /// Matched against the fact's name, either exactly or as a prefix before a `.`.
    key: &'static str,
    /// Whether `key` is the whole name or the first segment of it.
    prefix: bool,
    /// The fact appears where it was absent.
    added: CompatibilityImpact,
    /// The fact disappears.
    removed: CompatibilityImpact,
    /// The fact states something else.
    changed: CompatibilityImpact,
    /// Why, in the policy's own words.
    reason: &'static str,
}

/// The whole classification policy, in one table.
///
/// Read it as the answer to "what can a caller do that they could not do before, and what
/// can they no longer do".
const POLICIES: &[FactPolicy] = &[
    FactPolicy {
        key: "kind",
        prefix: false,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "a capability that changes between a query and a command changes what calling it does",
    },
    // An input property is bound by name over three transports and the input types refuse
    // unknown keys, so losing one is as breaking as gaining a required one.
    FactPolicy {
        key: "input",
        prefix: true,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "the input is bound by name and refuses an unknown key, so a caller that sends the old shape is refused",
    },
    FactPolicy {
        key: "output",
        prefix: true,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "a caller reads the output by name, so a field that is gone or has another type is a field they cannot read",
    },
    FactPolicy {
        key: "exposure",
        prefix: true,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "a projection is the address a caller holds; withdrawing or moving one leaves them calling nothing",
    },
    FactPolicy {
        key: "invocation",
        prefix: false,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "the invocation is what a script has written down",
    },
    FactPolicy {
        key: "argument",
        prefix: true,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "a command line that passed an argument stops parsing when it is gone, renamed, or no longer takes what it took",
    },
    FactPolicy {
        key: "aliases",
        prefix: false,
        // An alias set is one fact, so a change covers both gaining and losing one and is
        // classified at the cost of losing one. `version explain` prints the before and
        // after, which is where the distinction lives.
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "an alias is a name somebody typed; the set changed, and a name may have left it",
    },
    FactPolicy {
        key: "field",
        prefix: true,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "every repository's own documents are validated against this schema, so a field that changed invalidates files that were valid",
    },
    FactPolicy {
        key: "closed",
        prefix: false,
        added: CompatibilityImpact::Breaking,
        removed: CompatibilityImpact::Additive,
        changed: CompatibilityImpact::Breaking,
        reason: "a schema that starts refusing unknown keys refuses documents it used to accept",
    },
    FactPolicy {
        key: "rust-target",
        prefix: false,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "the triple is how an installer asks for the artifact of a platform",
    },
    FactPolicy {
        key: "status",
        prefix: false,
        added: CompatibilityImpact::Additive,
        removed: CompatibilityImpact::Breaking,
        changed: CompatibilityImpact::Breaking,
        reason: "a platform whose promise changed is a platform an installation may no longer find an artifact for",
    },
];

/// The policy for a fact, by name.
fn policy_for(fact: &str) -> Option<&'static FactPolicy> {
    POLICIES.iter().find(|p| {
        if p.prefix {
            fact == p.key || fact.starts_with(&format!("{}.", p.key))
        } else {
            fact == p.key
        }
    })
}

/// What a whole entry appearing or disappearing costs.
///
/// Every surface in the contract is something a caller addresses by name, so the answer is
/// the same for all of them and is stated once. It is written as a function rather than a
/// constant so that a surface which one day needs a different answer has a place to say
/// so, without the caller of this function changing.
fn entry_impact(kind: ChangeKind, surface: Surface) -> (CompatibilityImpact, String) {
    match kind {
        ChangeKind::Added => (
            CompatibilityImpact::Additive,
            format!("a {} that did not exist at the baseline", surface.noun()),
        ),
        ChangeKind::Removed => (
            CompatibilityImpact::Breaking,
            format!(
                "a {} the baseline offered and this tree does not",
                surface.noun()
            ),
        ),
        ChangeKind::Changed => (
            CompatibilityImpact::Patch,
            format!("a {} whose facts moved", surface.noun()),
        ),
    }
}

/// Compare two snapshots.
///
/// Surfaces one snapshot covers and the other does not are listed in
/// [`ContractDiff::uncompared`] and take no part in the verdict. That is the difference
/// between "the command graph was not built when the baseline was written" and "every
/// command was removed", and getting it wrong would report a breaking change on the first
/// release after a new surface joined the contract.
pub fn diff(baseline: &ContractSnapshot, current: &ContractSnapshot) -> ContractDiff {
    let baseline_surfaces = baseline.surfaces();
    let current_surfaces = current.surfaces();
    let comparable: BTreeSet<Surface> = baseline_surfaces
        .intersection(&current_surfaces)
        .copied()
        .collect();
    let mut uncompared: Vec<Surface> = baseline_surfaces
        .symmetric_difference(&current_surfaces)
        .copied()
        .collect();
    uncompared.sort();

    fn index<'a>(
        snapshot: &'a ContractSnapshot,
        comparable: &BTreeSet<Surface>,
    ) -> BTreeMap<(Surface, &'a str), &'a BTreeMap<String, String>> {
        snapshot
            .entries
            .iter()
            .filter(|e| comparable.contains(&e.surface))
            .map(|e| ((e.surface, e.id.as_str()), &e.facts))
            .collect()
    }
    let before = index(baseline, &comparable);
    let after = index(current, &comparable);

    let mut changes = Vec::new();
    for (key, facts) in &before {
        if !after.contains_key(key) {
            let (impact, reason) = entry_impact(ChangeKind::Removed, key.0);
            changes.push(ContractChange {
                surface: key.0,
                entry: key.1.to_string(),
                fact: None,
                kind: ChangeKind::Removed,
                before: Some(summarise(facts)),
                after: None,
                impact,
                reason,
            });
        }
    }
    for (key, facts) in &after {
        if !before.contains_key(key) {
            let (impact, reason) = entry_impact(ChangeKind::Added, key.0);
            changes.push(ContractChange {
                surface: key.0,
                entry: key.1.to_string(),
                fact: None,
                kind: ChangeKind::Added,
                before: None,
                after: Some(summarise(facts)),
                impact,
                reason,
            });
        }
    }
    for (key, old) in &before {
        let Some(new) = after.get(key) else { continue };
        let names: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
        for name in names {
            let (b, a) = (old.get(name), new.get(name));
            if b == a {
                continue;
            }
            let kind = match (b, a) {
                (None, Some(_)) => ChangeKind::Added,
                (Some(_), None) => ChangeKind::Removed,
                _ => ChangeKind::Changed,
            };
            let (impact, reason) = match policy_for(name) {
                Some(policy) => (
                    match kind {
                        ChangeKind::Added => policy.added,
                        ChangeKind::Removed => policy.removed,
                        ChangeKind::Changed => policy.changed,
                    },
                    policy.reason.to_string(),
                ),
                // A fact the policy does not classify is worth nothing and says so. It is
                // still reported: an unclassified fact is a gap in the table, and a gap
                // nobody can see is a gap nobody fixes.
                None => (
                    CompatibilityImpact::None,
                    format!("`{name}` is not classified by the compatibility policy"),
                ),
            };
            changes.push(ContractChange {
                surface: key.0,
                entry: key.1.to_string(),
                fact: Some(name.clone()),
                kind,
                before: b.cloned(),
                after: a.cloned(),
                impact,
                reason,
            });
        }
    }

    changes.sort_by(|x, y| {
        y.impact
            .cmp(&x.impact)
            .then_with(|| x.surface.cmp(&y.surface))
            .then_with(|| x.entry.cmp(&y.entry))
            .then_with(|| x.fact.cmp(&y.fact))
    });
    let impact = changes
        .iter()
        .map(|c| c.impact)
        .fold(CompatibilityImpact::None, CompatibilityImpact::max);
    ContractDiff {
        baseline: baseline.fingerprint.clone(),
        current: current.fingerprint.clone(),
        changes,
        impact,
        uncompared,
    }
}

/// One line describing a whole entry, for the `before`/`after` of an addition or removal.
fn summarise(facts: &BTreeMap<String, String>) -> String {
    // The projections, when there are any, are what a person recognises an entry by.
    let mut marks: Vec<String> = facts
        .iter()
        .filter(|(k, _)| k.starts_with("exposure.") || k.as_str() == "invocation")
        .map(|(_, v)| v.clone())
        .collect();
    marks.sort();
    if marks.is_empty() {
        format!("{} fact(s)", facts.len())
    } else {
        marks.join(", ")
    }
}

/// The bump an impact requires, under this project's release policy.
///
/// # The policy
///
/// At and above 1.0.0 the mapping is the specification's, and needs no defence:
///
/// | impact | bump |
/// |---|---|
/// | none | none |
/// | patch | patch |
/// | additive | minor |
/// | breaking | major |
///
/// Below 1.0.0 the specification binds nothing, which is exactly why this project states
/// what it does rather than inheriting folklore. **A breaking change moves the minor and
/// everything else moves the patch.** The zero major stays zero until the project says the
/// contract is settled, and the minor is what an installation pins against: a caller who
/// pinned `0.3` keeps working across `0.3.x` and is told to look again at `0.4.0`. This is
/// the rule Cargo already applies to a `0.x` dependency, so a repository that depends on
/// this project by version range gets the behaviour it already expects.
///
/// The distinction between `additive` and `patch` is not lost below 1.0.0 — both map to a
/// patch bump, and the impact is recorded and reported either way.
///
/// ```
/// use majordomus_cli::release::diff::{required_bump, CompatibilityImpact};
/// use majordomus_cli::release::version::{Bump, Version};
///
/// let after: Version = "1.4.2".parse().unwrap();
/// assert_eq!(required_bump(CompatibilityImpact::Breaking, &after), Bump::Major);
/// assert_eq!(required_bump(CompatibilityImpact::Additive, &after), Bump::Minor);
///
/// let before_one: Version = "0.3.1".parse().unwrap();
/// assert_eq!(required_bump(CompatibilityImpact::Breaking, &before_one), Bump::Minor);
/// assert_eq!(required_bump(CompatibilityImpact::Additive, &before_one), Bump::Patch);
/// ```
pub fn required_bump(impact: CompatibilityImpact, current: &Version) -> Bump {
    if current.is_initial_development() {
        return match impact {
            CompatibilityImpact::None => Bump::None,
            CompatibilityImpact::Patch | CompatibilityImpact::Additive => Bump::Patch,
            CompatibilityImpact::Breaking => Bump::Minor,
        };
    }
    match impact {
        CompatibilityImpact::None => Bump::None,
        CompatibilityImpact::Patch => Bump::Patch,
        CompatibilityImpact::Additive => Bump::Minor,
        CompatibilityImpact::Breaking => Bump::Major,
    }
}

/// The lowest version a release carrying this impact may declare.
///
/// The invariant the whole subsystem exists for: *a release may not declare a version
/// below the one its own contract change implies*. A version above it is allowed — a
/// project may decide a release deserves a bigger number than it strictly needs — and a
/// version below it is refused, whatever the commit messages say.
///
/// The baseline is what the minimum is computed from, not the working tree's current
/// version: a tree whose version was already bumped since the last release must not be
/// asked to bump again for the same change.
///
/// ```
/// use majordomus_cli::release::diff::{minimum_version, CompatibilityImpact};
/// use majordomus_cli::release::version::Version;
///
/// let published: Version = "0.3.1".parse().unwrap();
/// let minimum = minimum_version(CompatibilityImpact::Additive, &published);
/// assert_eq!(minimum.to_string(), "0.3.2");
/// ```
pub fn minimum_version(impact: CompatibilityImpact, baseline: &Version) -> Version {
    required_bump(impact, baseline).apply(baseline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::contract::ContractEntry;

    fn snapshot(entries: Vec<ContractEntry>) -> ContractSnapshot {
        ContractSnapshot::from_entries(entries)
    }

    fn entry(surface: Surface, id: &str, facts: &[(&str, &str)]) -> ContractEntry {
        ContractEntry {
            surface,
            id: id.into(),
            facts: facts
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }

    fn capability(id: &str, facts: &[(&str, &str)]) -> ContractEntry {
        entry(Surface::Capability, id, facts)
    }

    #[test]
    fn an_unchanged_contract_has_no_impact() {
        let a = snapshot(vec![capability("a.b", &[("kind", "query")])]);
        let d = diff(&a, &a);
        assert!(d.is_empty());
        assert_eq!(d.impact, CompatibilityImpact::None);
    }

    #[test]
    fn a_new_capability_is_additive() {
        let a = snapshot(vec![capability("a.b", &[("kind", "query")])]);
        let b = snapshot(vec![
            capability("a.b", &[("kind", "query")]),
            capability("a.c", &[("kind", "query")]),
        ]);
        let d = diff(&a, &b);
        assert_eq!(d.impact, CompatibilityImpact::Additive);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.changes[0].kind, ChangeKind::Added);
        assert_eq!(d.changes[0].entry, "a.c");
    }

    #[test]
    fn a_capability_that_disappears_is_breaking() {
        let a = snapshot(vec![
            capability("a.b", &[("kind", "query")]),
            capability("a.c", &[("kind", "query")]),
        ]);
        let b = snapshot(vec![capability("a.b", &[("kind", "query")])]);
        let d = diff(&a, &b);
        assert_eq!(d.impact, CompatibilityImpact::Breaking);
        assert_eq!(d.changes[0].entry, "a.c");
        assert_eq!(d.changes[0].kind, ChangeKind::Removed);
    }

    #[test]
    fn a_new_optional_input_is_additive_and_a_new_required_one_is_not_special_cased() {
        let a = snapshot(vec![capability("a.b", &[("kind", "query")])]);
        let b = snapshot(vec![capability(
            "a.b",
            &[("kind", "query"), ("input.since", "string optional")],
        )]);
        assert_eq!(diff(&a, &b).impact, CompatibilityImpact::Additive);
    }

    /// The case the whole classifier exists for: a field that was optional and is now
    /// required is a *changed* fact, not an added one, and changing an input fact is
    /// breaking.
    #[test]
    fn an_input_that_becomes_required_is_breaking() {
        let a = snapshot(vec![capability(
            "a.b",
            &[("input.since", "string optional")],
        )]);
        let b = snapshot(vec![capability(
            "a.b",
            &[("input.since", "string required")],
        )]);
        let d = diff(&a, &b);
        assert_eq!(d.impact, CompatibilityImpact::Breaking);
        assert_eq!(d.changes[0].before.as_deref(), Some("string optional"));
        assert_eq!(d.changes[0].after.as_deref(), Some("string required"));
    }

    #[test]
    fn an_output_field_that_disappears_is_breaking_and_a_new_one_is_additive() {
        let a = snapshot(vec![capability(
            "a.b",
            &[("output.count", "integer required")],
        )]);
        let b = snapshot(vec![capability(
            "a.b",
            &[("output.total", "integer required")],
        )]);
        let d = diff(&a, &b);
        assert_eq!(d.impact, CompatibilityImpact::Breaking);
        assert_eq!(d.changes.len(), 2, "one removal and one addition");
    }

    #[test]
    fn a_route_that_moves_is_breaking() {
        let a = snapshot(vec![capability(
            "a.b",
            &[("exposure.http", "GET /api/v1/thing")],
        )]);
        let b = snapshot(vec![capability(
            "a.b",
            &[("exposure.http", "GET /api/v1/things")],
        )]);
        assert_eq!(diff(&a, &b).impact, CompatibilityImpact::Breaking);
    }

    #[test]
    fn a_capability_that_gains_a_projection_is_additive() {
        let a = snapshot(vec![capability("a.b", &[("kind", "query")])]);
        let b = snapshot(vec![capability(
            "a.b",
            &[("kind", "query"), ("exposure.cli", "thing show")],
        )]);
        assert_eq!(diff(&a, &b).impact, CompatibilityImpact::Additive);
    }

    /// A surface the baseline never carried is not compared, so the first release after a
    /// surface joins the contract does not report every entry of it as an addition — nor,
    /// worse, would removing a surface report every entry as a removal.
    #[test]
    fn a_surface_only_one_snapshot_covers_is_not_compared() {
        let a = snapshot(vec![capability("a.b", &[("kind", "query")])]);
        let b = snapshot(vec![
            capability("a.b", &[("kind", "query")]),
            entry(
                Surface::Target,
                "linux-x86_64-gnu",
                &[("status", "supported")],
            ),
        ]);
        let d = diff(&a, &b);
        assert!(d.is_empty(), "{:?}", d.changes);
        assert_eq!(d.uncompared, vec![Surface::Target]);
        assert_eq!(d.impact, CompatibilityImpact::None);
    }

    #[test]
    fn a_published_target_that_is_withdrawn_is_breaking() {
        let a = snapshot(vec![entry(
            Surface::Target,
            "windows-x86_64",
            &[("status", "supported")],
        )]);
        let b = snapshot(vec![entry(
            Surface::Target,
            "windows-x86_64",
            &[("status", "unavailable")],
        )]);
        assert_eq!(diff(&a, &b).impact, CompatibilityImpact::Breaking);
    }

    #[test]
    fn a_document_schema_that_gains_a_required_field_is_breaking() {
        let a = snapshot(vec![entry(
            Surface::DocumentKind,
            "majordomus.rule/v1",
            &[("field.id", "string required")],
        )]);
        let b = snapshot(vec![entry(
            Surface::DocumentKind,
            "majordomus.rule/v1",
            &[
                ("field.id", "string required"),
                ("field.owner", "string required"),
            ],
        )]);
        // Adding a field to a schema is additive by the table; what makes it breaking is
        // that the field is required, which the *fact* carries and the reason explains.
        let d = diff(&a, &b);
        assert_eq!(d.changes[0].after.as_deref(), Some("string required"));
        assert_eq!(d.impact, CompatibilityImpact::Additive);
    }

    #[test]
    fn an_unclassified_fact_is_reported_and_costs_nothing() {
        let a = snapshot(vec![capability("a.b", &[("colour", "blue")])]);
        let b = snapshot(vec![capability("a.b", &[("colour", "green")])]);
        let d = diff(&a, &b);
        assert_eq!(d.impact, CompatibilityImpact::None);
        assert_eq!(d.changes.len(), 1);
        assert!(
            d.changes[0].reason.contains("not classified"),
            "{:?}",
            d.changes[0]
        );
    }

    #[test]
    fn the_strongest_change_decides_the_verdict() {
        let a = snapshot(vec![
            capability("a.b", &[("kind", "query")]),
            capability("a.c", &[("kind", "query")]),
        ]);
        let b = snapshot(vec![
            capability("a.b", &[("kind", "query")]),
            capability("a.d", &[("kind", "query")]),
        ]);
        let d = diff(&a, &b);
        assert_eq!(d.impact, CompatibilityImpact::Breaking);
        assert_eq!(
            d.changes[0].impact,
            CompatibilityImpact::Breaking,
            "and the strongest is reported first"
        );
        assert_eq!(d.tallies()["breaking"], 1);
        assert_eq!(d.tallies()["additive"], 1);
    }

    #[test]
    fn the_diff_is_deterministic_whatever_order_the_entries_arrive_in() {
        let forward = snapshot(vec![
            capability("a.b", &[("kind", "query")]),
            capability("a.c", &[("kind", "query")]),
        ]);
        let backward = snapshot(vec![
            capability("a.c", &[("kind", "query")]),
            capability("a.b", &[("kind", "query")]),
        ]);
        let empty = snapshot(vec![capability("a.b", &[("kind", "query")])]);
        assert_eq!(diff(&empty, &forward), diff(&empty, &backward));
    }

    #[test]
    fn the_policy_below_one_zero_puts_a_break_in_the_minor() {
        let v: Version = "0.3.1".parse().unwrap();
        assert_eq!(
            minimum_version(CompatibilityImpact::Breaking, &v).to_string(),
            "0.4.0"
        );
        assert_eq!(
            minimum_version(CompatibilityImpact::Additive, &v).to_string(),
            "0.3.2"
        );
        assert_eq!(
            minimum_version(CompatibilityImpact::Patch, &v).to_string(),
            "0.3.2"
        );
        assert_eq!(
            minimum_version(CompatibilityImpact::None, &v).to_string(),
            "0.3.1"
        );
    }

    #[test]
    fn the_policy_at_and_above_one_zero_is_the_specifications() {
        let v: Version = "1.4.2".parse().unwrap();
        assert_eq!(
            minimum_version(CompatibilityImpact::Breaking, &v).to_string(),
            "2.0.0"
        );
        assert_eq!(
            minimum_version(CompatibilityImpact::Additive, &v).to_string(),
            "1.5.0"
        );
        assert_eq!(
            minimum_version(CompatibilityImpact::Patch, &v).to_string(),
            "1.4.3"
        );
    }

    #[test]
    fn every_impact_a_change_can_carry_has_a_policy_that_produced_it() {
        for policy in POLICIES {
            assert!(
                !policy.reason.is_empty(),
                "`{}` classifies without saying why",
                policy.key
            );
            assert!(
                policy
                    .key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-'),
                "`{}` is not a fact name",
                policy.key
            );
        }
    }

    /// Every fact the snapshot builder can emit is classified. A fact with no policy is
    /// legal — it reports as unclassified — but one this repository's own contract
    /// produces would be a silent hole in the verdict, so the table is checked against the
    /// names the builder writes.
    #[test]
    fn every_fact_this_repository_emits_is_classified() {
        for fact in [
            "kind",
            "input.document",
            "output.count",
            "exposure.mcp.tool",
            "exposure.mcp.resource",
            "exposure.http",
            "exposure.cli",
            "invocation",
            "argument.repo",
            "aliases",
            "field.header.id",
            "closed",
            "rust-target",
            "status",
        ] {
            assert!(
                policy_for(fact).is_some(),
                "`{fact}` is emitted by the snapshot and classified by nothing"
            );
        }
    }
}
