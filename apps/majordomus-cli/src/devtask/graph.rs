//! The milestone as a typed dependency graph, with the partitions a work surface would
//! otherwise compute for itself.
//!
//! # What is derived here and what is not
//!
//! The graph itself is not derived here. [`crate::plan`] already walks the canonical records
//! and produces the edges, the waves, the statuses, the dependents and the findings; this
//! module reads that answer and computes the four things a plan surface needs and the plan
//! does not carry:
//!
//! * **the partitions by readiness** — ready, blocked, waiting, in progress, review,
//!   completion-blocked, complete, cancelled. Grouping is a derivation, and a derivation
//!   done in a template is a derivation done differently in every template.
//! * **the critical blockers** — which unfinished issues hold back the most downstream work,
//!   measured as the transitive closure of the dependents relation. What to unblock first
//!   is the question a milestone exists to answer.
//! * **the parallelizable subsets** — of the startable work, which issues may genuinely run
//!   at the same time: same wave, each `parallel_safe`, and no two sharing a scope path. The
//!   plan warns about a scope conflict; this partitions around it.
//! * **the cycles** — the plan reports that a cycle exists and names every issue stuck
//!   behind one. That is the right diagnostic and the wrong shape for a reader who wants to
//!   fix it: the strongly connected components are what a person edits.
//!
//! # Determinism
//!
//! Every list here is ordered by [`crate::order::natural_cmp`] over the canonical issue id,
//! which is the repository's canonical order and is total — identities are unique, so no
//! comparison ever ends in a tie. Nothing is ordered by a hash-map walk, by discovery order
//! or by insertion order, so two runs over the same records, on two machines, produce the
//! same bytes. `test/cases/129_executable_issues.sh` asserts that byte-for-byte.
//!
//! The graph algorithms below are deterministic for the same reason: the depth-first walk of
//! [`cycles`] visits roots in canonical order, and the greedy partition of
//! [`parallel_sets`] considers candidates in canonical order.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::order::natural_cmp;
use crate::plan::{overlap, Plan, PlanIssue};

use super::readiness::TaskReadiness;
use super::task::{DevTaskDiagnostic, RecordRef};
use super::{AttestedCount, AttestedList, AttestedText, FieldProvenance};

/// One issue of a milestone, as a node of its graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MilestoneNode {
    /// The issue id.
    pub issue: String,
    /// One line naming the outcome.
    pub title: String,
    /// Where it stands as work.
    pub readiness: TaskReadiness,
    /// The canonical status it refines.
    pub canonical_status: String,
    /// The execution wave, absent when a cycle prevents it from having one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave: Option<u32>,
    /// `p0` … `p3`.
    pub priority: String,
    /// Whether it may run beside another issue of its wave.
    pub parallel_safe: bool,
    /// What is not `DONE` yet, plus the milestone gate when it applies.
    pub blocked_by: Vec<String>,
    /// The issues that wait on it directly.
    pub dependents: Vec<String>,
    /// How many issues wait on it directly or through another.
    pub transitive_dependents: u32,
    /// The paths it touches.
    pub scope: Vec<String>,
}

/// One dependency edge of the milestone's graph, `from` before `to`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct MilestoneEdge {
    /// The issue that must be real first.
    pub from: String,
    /// The issue that waits for it.
    pub to: String,
    /// True when one end is outside this milestone. Such an edge is why a milestone can be
    /// entirely blocked with nothing wrong inside it, so it is marked rather than dropped.
    pub external: bool,
}

/// One unfinished issue that holds back downstream work, and how much.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CriticalBlocker {
    /// The issue.
    pub issue: String,
    /// Where it stands, which is what says whether unblocking it is work or a decision.
    pub readiness: TaskReadiness,
    /// The unfinished issues that wait on it, directly or through another, in canonical
    /// order.
    pub blocks: Vec<String>,
    /// How many that is. The number the list is ordered by.
    pub weight: u32,
}

/// Two issues of one wave that cannot run together, and the path that stops them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScopeConflict {
    /// The issue already in the set.
    pub held: String,
    /// The issue that could not join it.
    pub excluded: String,
    /// The scope path they share, as one of them declares it.
    pub path: String,
}

/// One subset of the startable work that may genuinely run at the same time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParallelSet {
    /// The wave every issue in it belongs to.
    pub wave: u32,
    /// The issues, in canonical order.
    pub issues: Vec<String>,
    /// Why the set is not larger: the candidates of the same wave that had to be serialised
    /// against something already in it.
    pub serialised: Vec<ScopeConflict>,
}

/// One milestone's issues counted by readiness, plus the two denominators.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct MilestoneCounts {
    /// Every issue naming this milestone.
    pub total: u32,
    /// Total less cancelled: the denominator a progress figure uses.
    pub required: u32,
    /// One entry per readiness state that has at least one issue, keyed by its word.
    pub by_readiness: BTreeMap<String, u32>,
}

/// One milestone as an executable dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MilestoneGraph {
    /// The milestone id, as asked for.
    pub milestone: String,
    /// Whether the canonical model declares it. False is answered, not refused.
    pub declared: bool,
    /// Repository-relative path of the canonical record, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// One line naming the outcome, as authored.
    pub title: AttestedText,
    /// What is true once it is real, as authored.
    pub outcome: AttestedText,
    /// The derived milestone status.
    pub status: AttestedText,
    /// Its layer in the milestone graph: what orders the roadmap.
    pub rank: AttestedCount,
    /// The milestones it requires, as authored.
    pub depends_on: AttestedList,
    /// Those of them that are not `DONE`. Non-empty means every issue here is held back
    /// whatever its own dependencies say.
    pub blocked_by: AttestedList,
    /// The milestones that require it.
    pub dependents: AttestedList,
    /// Every issue of the milestone, in canonical order.
    pub nodes: Vec<MilestoneNode>,
    /// Every dependency edge with at least one end in this milestone, sorted.
    pub edges: Vec<MilestoneEdge>,
    /// The issues a worker may pick up now.
    pub ready: Vec<String>,
    /// The issues waiting on a peer.
    pub blocked: Vec<String>,
    /// The issues waiting only on the milestone gate.
    pub waiting: Vec<String>,
    /// The issues somebody has started.
    pub active: Vec<String>,
    /// The issues awaiting confirmation.
    pub review: Vec<String>,
    /// The issues declared complete whose evidence does not back that up.
    pub completion_blocked: Vec<String>,
    /// The issues finished on their own terms.
    pub complete: Vec<String>,
    /// The issues withdrawn.
    pub cancelled: Vec<String>,
    /// What to unblock first, most downstream work first.
    pub critical_blockers: Vec<CriticalBlocker>,
    /// The startable work partitioned into subsets that may run at the same time.
    pub parallelizable: Vec<ParallelSet>,
    /// Every dependency cycle with an issue of this milestone in it, each as its strongly
    /// connected component in canonical order.
    pub cycles: Vec<Vec<String>>,
    /// Everything wrong with the records of this milestone or of its issues.
    pub diagnostics: Vec<DevTaskDiagnostic>,
    /// The issues counted by readiness.
    pub counts: MilestoneCounts,
}

/// Every issue that waits on `id`, directly or through another, in canonical order.
///
/// A breadth-first walk of the plan's own `dependents` relation. `id` itself is never in the
/// answer, even when a cycle leads back to it.
///
/// ```
/// use majordomus_cli::devtask::graph::transitive_dependents;
/// # use majordomus_cli::plan::PlanIssue;
/// # fn node(id: &str, dependents: &[&str]) -> PlanIssue {
/// #     PlanIssue { id: id.into(), milestone: "m".into(), status: "READY".into(), wave: 0,
/// #         priority: "p1".into(), profile: String::new(), parallel_safe: true,
/// #         title: String::new(), slug: String::new(), depends_on: vec![],
/// #         blocked_by: vec![], dependents: dependents.iter().map(|s| (*s).into()).collect(),
/// #         scope: vec![], objective: String::new(), evidence_have: 0, evidence_need: 0,
/// #         started_at: String::new(), verified_at: String::new(), completed_at: String::new() }
/// # }
/// let issues = vec![node("A", &["B"]), node("B", &["C"]), node("C", &[])];
/// assert_eq!(transitive_dependents(&issues, "A"), ["B", "C"]);
/// assert!(transitive_dependents(&issues, "C").is_empty());
/// ```
pub fn transitive_dependents(issues: &[PlanIssue], id: &str) -> Vec<String> {
    let by_id: BTreeMap<&str, &PlanIssue> = issues.iter().map(|i| (i.id.as_str(), i)).collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut queue: Vec<&str> = vec![id];
    while let Some(next) = queue.pop() {
        let Some(node) = by_id.get(next) else {
            continue;
        };
        for d in &node.dependents {
            if d != id && seen.insert(d.clone()) {
                queue.push(d.as_str());
            }
        }
    }
    let mut out: Vec<String> = seen.into_iter().collect();
    crate::order::canonical_strings(&mut out);
    out
}

/// Every dependency cycle of the issue graph, each as its strongly connected component in
/// canonical order, the components themselves in canonical order.
///
/// Tarjan's algorithm, with the roots visited in canonical order so the answer is the same
/// on every run and on every machine. A component of one node is a cycle only when the issue
/// depends on itself; the plan reports that separately as `self_dependency` and it is
/// included here too, because a reader fixing cycles wants both.
///
/// ```
/// use majordomus_cli::devtask::graph::cycles;
/// # use majordomus_cli::plan::PlanIssue;
/// # fn node(id: &str, depends_on: &[&str]) -> PlanIssue {
/// #     PlanIssue { id: id.into(), milestone: "m".into(), status: "READY".into(), wave: 0,
/// #         priority: "p1".into(), profile: String::new(), parallel_safe: true,
/// #         title: String::new(), slug: String::new(),
/// #         depends_on: depends_on.iter().map(|s| (*s).into()).collect(),
/// #         blocked_by: vec![], dependents: vec![], scope: vec![], objective: String::new(),
/// #         evidence_have: 0, evidence_need: 0, started_at: String::new(),
/// #         verified_at: String::new(), completed_at: String::new() }
/// # }
/// let issues = vec![node("A", &["B"]), node("B", &["A"]), node("C", &[])];
/// assert_eq!(cycles(&issues), vec![vec!["A".to_string(), "B".to_string()]]);
/// ```
pub fn cycles(issues: &[PlanIssue]) -> Vec<Vec<String>> {
    // canonical order of the nodes, so the walk is reproducible
    let mut order: Vec<&str> = issues.iter().map(|i| i.id.as_str()).collect();
    crate::order::canonical_strings(&mut order);
    let position: BTreeMap<&str, usize> =
        order.iter().enumerate().map(|(n, id)| (*id, n)).collect();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); order.len()];
    for i in issues {
        let Some(&from) = position.get(i.id.as_str()) else {
            continue;
        };
        let mut targets: Vec<usize> = i
            .depends_on
            .iter()
            .filter_map(|d| position.get(d.as_str()).copied())
            .collect();
        targets.sort_unstable();
        targets.dedup();
        adjacency[from] = targets;
    }

    let n = order.len();
    let unvisited = usize::MAX;
    let mut index = vec![unvisited; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut next: usize = 0;
    let mut components: Vec<Vec<String>> = Vec::new();

    // an explicit work stack: a deep dependency chain must not overflow the call stack
    for root in 0..n {
        if index[root] != unvisited {
            continue;
        }
        let mut work: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some((v, edge)) = work.pop() {
            if edge < adjacency[v].len() {
                work.push((v, edge + 1));
                let w = adjacency[v][edge];
                if index[w] == unvisited {
                    index[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    work.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            // v is finished: fold its low-link into its parent, then close a root
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == index[v] {
                let mut component: Vec<String> = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack[w] = false;
                    component.push(order[w].to_string());
                    if w == v {
                        break;
                    }
                }
                let cyclic = component.len() > 1 || adjacency[v].contains(&v);
                if cyclic {
                    crate::order::canonical_strings(&mut component);
                    components.push(component);
                }
            }
        }
    }
    components.sort_by(|a, b| natural_cmp(&a[0], &b[0]));
    components
}

/// Partition startable work into subsets that may genuinely run at the same time.
///
/// The rule is the repository's own: two issues may run together when they are in the same
/// wave, both declare `parallel_safe`, and no scope path of one contains a scope path of the
/// other — [`crate::plan::overlap`] is the same containment test the plan's own
/// `scope_conflict` warning uses, so the two can never disagree. An issue that declares
/// itself not parallel-safe gets a set of its own.
///
/// Greedy, over candidates in canonical order, so the partition is deterministic. It is not
/// the smallest possible partition — that is graph colouring — and it does not need to be: a
/// worker asks what can be fanned out now, and a conservative answer is a correct one.
pub fn parallel_sets(nodes: &[MilestoneNode]) -> Vec<ParallelSet> {
    let mut by_wave: BTreeMap<u32, Vec<&MilestoneNode>> = BTreeMap::new();
    for node in nodes.iter().filter(|n| n.readiness.is_startable()) {
        let Some(wave) = node.wave else { continue };
        by_wave.entry(wave).or_default().push(node);
    }
    let mut out: Vec<ParallelSet> = Vec::new();
    for (wave, mut candidates) in by_wave {
        candidates.sort_by(|a, b| natural_cmp(&a.issue, &b.issue));
        let mut sets: Vec<(Vec<&MilestoneNode>, Vec<ScopeConflict>)> = Vec::new();
        for candidate in candidates {
            if !candidate.parallel_safe {
                sets.push((vec![candidate], Vec::new()));
                continue;
            }
            let mut refused: Vec<ScopeConflict> = Vec::new();
            let mut placed = false;
            for (members, conflicts) in &mut sets {
                if members.iter().any(|m| !m.parallel_safe) {
                    continue;
                }
                match first_conflict(members, candidate) {
                    Some(conflict) => refused.push(conflict),
                    None => {
                        members.push(candidate);
                        conflicts.append(&mut refused);
                        placed = true;
                        break;
                    }
                }
            }
            if !placed {
                sets.push((vec![candidate], refused));
            }
        }
        for (members, serialised) in sets {
            let mut issues: Vec<String> = members.iter().map(|m| m.issue.clone()).collect();
            crate::order::canonical_strings(&mut issues);
            out.push(ParallelSet {
                wave,
                issues,
                serialised,
            });
        }
    }
    out
}

/// The first member of the set the candidate shares a scope path with.
fn first_conflict(members: &[&MilestoneNode], candidate: &MilestoneNode) -> Option<ScopeConflict> {
    for member in members {
        for held in &member.scope {
            for wanted in &candidate.scope {
                if overlap(held, wanted) {
                    return Some(ScopeConflict {
                        held: member.issue.clone(),
                        excluded: candidate.issue.clone(),
                        path: held.clone(),
                    });
                }
            }
        }
    }
    None
}

impl MilestoneGraph {
    /// The capability every derived field points a reader at.
    const PLAN: &'static str = "plan.model";

    /// Build one milestone's graph out of the derived plan.
    ///
    /// Pure: a walk of values the caller already has. Nothing is read, and no derivation the
    /// plan owns is repeated — the statuses, waves, edges and findings below are the plan's
    /// own, and only the partitions are computed here.
    #[allow(clippy::too_many_lines)] // one composition of a graph, section by section
    pub fn build(plan: &Plan, milestone: &str, record: Option<RecordRef<'_>>) -> MilestoneGraph {
        let derived = plan.milestone(milestone);
        let absent = "the canonical model declares no milestone with this id";
        let authored = |key: &str| -> AttestedText {
            let source = record.map_or_else(
                || format!(".ai/repo/project/milestones#{key}"),
                |r| format!("{}#{key}", r.path),
            );
            match record.map(|r| r.metadata).and_then(|m| m.get(key)) {
                Some(v) if !v.is_null() => {
                    AttestedText::explicit(crate::plan::scalar(Some(v)), source)
                }
                _ if record.is_none() => AttestedText::unknown(source, absent),
                _ => AttestedText::unknown(source, format!("the record declares no `{key}`")),
            }
        };
        let authored_list = |key: &str| -> AttestedList {
            let source = record.map_or_else(
                || format!(".ai/repo/project/milestones#{key}"),
                |r| format!("{}#{key}", r.path),
            );
            match record.map(|r| r.metadata).and_then(|m| m.get(key)) {
                Some(serde_json::Value::Array(items)) => AttestedList::explicit(
                    items.iter().map(|v| crate::plan::scalar(Some(v))).collect(),
                    source,
                ),
                Some(v) if !v.is_null() => {
                    AttestedList::explicit(vec![crate::plan::scalar(Some(v))], source)
                }
                _ if record.is_none() => AttestedList::unknown(source, absent),
                _ => AttestedList::unknown(source, format!("the record declares no `{key}`")),
            }
        };

        let mut nodes: Vec<MilestoneNode> = plan
            .issues
            .iter()
            .filter(|i| i.milestone == milestone)
            .map(|i| Self::node(plan, i))
            .collect();
        nodes.sort_by(|a, b| natural_cmp(&a.issue, &b.issue));

        let inside: BTreeSet<&str> = nodes.iter().map(|n| n.issue.as_str()).collect();
        let mut edges: Vec<MilestoneEdge> = plan
            .edges
            .iter()
            .filter(|e| inside.contains(e.from.as_str()) || inside.contains(e.to.as_str()))
            .map(|e| MilestoneEdge {
                from: e.from.clone(),
                to: e.to.clone(),
                external: !inside.contains(e.from.as_str()) || !inside.contains(e.to.as_str()),
            })
            .collect();
        edges.sort();

        let of = |state: TaskReadiness| -> Vec<String> {
            nodes
                .iter()
                .filter(|n| n.readiness == state)
                .map(|n| n.issue.clone())
                .collect()
        };
        let mut counts = MilestoneCounts {
            total: u32::try_from(nodes.len()).unwrap_or(u32::MAX),
            required: 0,
            by_readiness: BTreeMap::new(),
        };
        for n in &nodes {
            *counts
                .by_readiness
                .entry(n.readiness.as_str().to_string())
                .or_insert(0) += 1;
        }
        counts.required = counts.total
            - counts
                .by_readiness
                .get(TaskReadiness::Cancelled.as_str())
                .copied()
                .unwrap_or(0);

        let mut critical: Vec<CriticalBlocker> = nodes
            .iter()
            .filter(|n| !n.readiness.is_terminal())
            .filter_map(|n| {
                let blocks: Vec<String> = transitive_dependents(&plan.issues, &n.issue)
                    .into_iter()
                    .filter(|d| {
                        plan.issue(d)
                            .is_some_and(|i| !matches!(i.status.as_str(), "DONE" | "CANCELLED"))
                    })
                    .collect();
                if blocks.is_empty() {
                    return None;
                }
                Some(CriticalBlocker {
                    issue: n.issue.clone(),
                    readiness: n.readiness,
                    weight: u32::try_from(blocks.len()).unwrap_or(u32::MAX),
                    blocks,
                })
            })
            .collect();
        // most downstream work first; canonical order inside one weight, so the sequence is
        // total and no two runs disagree
        critical.sort_by(|a, b| {
            b.weight
                .cmp(&a.weight)
                .then_with(|| natural_cmp(&a.issue, &b.issue))
        });

        let all_cycles = cycles(&plan.issues);
        let here: Vec<Vec<String>> = all_cycles
            .into_iter()
            .filter(|c| c.iter().any(|id| inside.contains(id.as_str())))
            .collect();

        let diagnostics: Vec<DevTaskDiagnostic> = plan
            .findings
            .iter()
            .filter(|f| {
                f.subject == milestone
                    || inside.contains(f.subject.as_str())
                    || f.message
                        .split_whitespace()
                        .any(|w| inside.contains(w.trim_end_matches(',')))
            })
            .map(|f| DevTaskDiagnostic {
                level: f.level.clone(),
                code: f.code.clone(),
                message: format!("{}: {}", f.subject, f.message),
                reproduce: "majordomus plan validate".into(),
            })
            .collect();

        MilestoneGraph {
            milestone: milestone.to_string(),
            declared: derived.is_some(),
            record: record.map(|r| r.path.to_string()),
            title: authored("title"),
            outcome: authored("outcome"),
            status: derived.map_or_else(
                || AttestedText::unknown(Self::PLAN, absent),
                |m| AttestedText::derived(&m.status, Self::PLAN),
            ),
            rank: derived.map_or_else(
                || AttestedCount::unknown(Self::PLAN, absent),
                |m| AttestedCount::derived(m.rank, Self::PLAN),
            ),
            depends_on: authored_list("depends_on"),
            blocked_by: derived.map_or_else(
                || AttestedList::unknown(Self::PLAN, absent),
                |m| AttestedList::derived(m.blocked_by.clone(), Self::PLAN),
            ),
            dependents: derived.map_or_else(
                || AttestedList::unknown(Self::PLAN, absent),
                |m| AttestedList::derived(m.dependents.clone(), Self::PLAN),
            ),
            ready: of(TaskReadiness::Ready),
            blocked: of(TaskReadiness::Blocked),
            waiting: of(TaskReadiness::Waiting),
            active: of(TaskReadiness::InProgress),
            review: of(TaskReadiness::Review),
            completion_blocked: of(TaskReadiness::CompletionBlocked),
            complete: of(TaskReadiness::Complete),
            cancelled: of(TaskReadiness::Cancelled),
            critical_blockers: critical,
            parallelizable: parallel_sets(&nodes),
            cycles: here,
            diagnostics,
            counts,
            nodes,
            edges,
        }
    }

    fn node(plan: &Plan, i: &PlanIssue) -> MilestoneNode {
        let cyclic = plan
            .findings
            .iter()
            .any(|f| f.code == "cycle" && f.message.split_whitespace().any(|w| w == i.id));
        let covered = i.evidence_have >= i.evidence_need;
        MilestoneNode {
            issue: i.id.clone(),
            title: i.title.clone(),
            readiness: TaskReadiness::derive(
                &i.status,
                &i.blocked_by,
                !i.completed_at.is_empty(),
                covered,
            ),
            canonical_status: i.status.clone(),
            wave: if cyclic { None } else { Some(i.wave) },
            priority: i.priority.clone(),
            parallel_safe: i.parallel_safe,
            blocked_by: i.blocked_by.clone(),
            dependents: i.dependents.clone(),
            transitive_dependents: u32::try_from(transitive_dependents(&plan.issues, &i.id).len())
                .unwrap_or(u32::MAX),
            scope: i.scope.clone(),
        }
    }

    /// Is every partition consistent with the nodes: each issue in exactly one readiness
    /// bucket, and every bucket naming an issue of this milestone?
    ///
    /// The check a surface would otherwise have to trust. A node counted twice or in no
    /// bucket at all is the failure mode of grouping by hand, which is exactly what this
    /// type exists to remove from every template.
    pub fn is_partitioned(&self) -> bool {
        let mut counted = 0usize;
        for bucket in [
            &self.ready,
            &self.blocked,
            &self.waiting,
            &self.active,
            &self.review,
            &self.completion_blocked,
            &self.complete,
            &self.cancelled,
        ] {
            counted += bucket.len();
            if !bucket
                .iter()
                .all(|id| self.nodes.iter().any(|n| &n.issue == id))
            {
                return false;
            }
        }
        counted == self.nodes.len()
    }

    /// Whether anything about this milestone's records is a failure rather than a warning.
    pub fn has_failures(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == "FAIL")
    }

    /// Is every attested field's invariant held?
    pub fn is_consistent(&self) -> bool {
        [&self.title, &self.outcome, &self.status].iter().all(|f| {
            f.is_consistent() && (f.provenance != FieldProvenance::Unknown || f.reason.is_some())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{
        PlanCounts, PlanEdge, PlanFinding, PlanMilestone, PlanProject, PlanVocabulary,
    };

    fn node(id: &str, status: &str, depends_on: &[&str], scope: &[&str]) -> PlanIssue {
        PlanIssue {
            id: id.into(),
            milestone: "m1".into(),
            status: status.into(),
            wave: 0,
            priority: "p1".into(),
            profile: "implementation".into(),
            parallel_safe: true,
            title: format!("{id} title"),
            slug: String::new(),
            depends_on: depends_on.iter().map(|s| (*s).into()).collect(),
            blocked_by: Vec::new(),
            dependents: Vec::new(),
            scope: scope.iter().map(|s| (*s).into()).collect(),
            objective: String::new(),
            evidence_have: 0,
            evidence_need: 0,
            started_at: String::new(),
            verified_at: String::new(),
            completed_at: String::new(),
        }
    }

    /// Fill in the `dependents` and `blocked_by` the plan would have derived, so a fixture
    /// graph behaves the way a real one does without restating the derivation.
    fn close(mut issues: Vec<PlanIssue>) -> Vec<PlanIssue> {
        let done: BTreeSet<String> = issues
            .iter()
            .filter(|i| i.status == "DONE")
            .map(|i| i.id.clone())
            .collect();
        let edges: Vec<(String, String)> = issues
            .iter()
            .flat_map(|i| i.depends_on.iter().map(|d| (d.clone(), i.id.clone())))
            .collect();
        for i in &mut issues {
            i.dependents = edges
                .iter()
                .filter(|(from, _)| from == &i.id)
                .map(|(_, to)| to.clone())
                .collect();
            i.blocked_by = i
                .depends_on
                .iter()
                .filter(|d| !done.contains(*d))
                .cloned()
                .collect();
            if !i.blocked_by.is_empty() && i.status == "READY" {
                i.status = "BLOCKED".into();
            }
        }
        issues
    }

    fn plan_of(issues: Vec<PlanIssue>, findings: Vec<PlanFinding>) -> Plan {
        let issues = close(issues);
        let edges: Vec<PlanEdge> = {
            let mut e: Vec<PlanEdge> = issues
                .iter()
                .flat_map(|i| {
                    i.depends_on.iter().map(|d| PlanEdge {
                        from: d.clone(),
                        to: i.id.clone(),
                    })
                })
                .collect();
            e.sort();
            e
        };
        Plan {
            project: PlanProject {
                name: "fixture".into(),
                repository: "owner/fixture".into(),
                default_branch: "master".into(),
                active_milestone: "m1".into(),
            },
            statuses: PlanVocabulary {
                issue: crate::plan::ISSUE_STATUSES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                milestone: crate::plan::MILESTONE_STATUSES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            },
            milestones: vec![PlanMilestone {
                id: "m1".into(),
                status: "ACTIVE".into(),
                order: 1,
                priority: "p1".into(),
                title: "The milestone".into(),
                slug: "m1".into(),
                version: String::new(),
                rank: 0,
                depends_on: Vec::new(),
                blocked_by: Vec::new(),
                dependents: Vec::new(),
                claims: Vec::new(),
                counts: PlanCounts {
                    total: u32::try_from(issues.len()).unwrap_or(0),
                    required: u32::try_from(issues.len()).unwrap_or(0),
                    by_status: BTreeMap::new(),
                },
                issues: issues.iter().map(|i| i.id.clone()).collect(),
                outcome: String::new(),
            }],
            issues,
            waves: Vec::new(),
            edges,
            milestone_edges: Vec::new(),
            findings,
        }
    }

    /// A chain: only the head is ready, and it is the critical blocker for everything behind
    /// it, weighted by the whole tail.
    #[test]
    fn a_dependency_chain_has_one_ready_head_and_one_critical_blocker() {
        let plan = plan_of(
            vec![
                node("I0001", "READY", &[], &[]),
                node("I0002", "READY", &["I0001"], &[]),
                node("I0003", "READY", &["I0002"], &[]),
            ],
            Vec::new(),
        );
        let g = MilestoneGraph::build(&plan, "m1", None);
        assert_eq!(g.ready, ["I0001"]);
        assert_eq!(g.blocked, ["I0002", "I0003"]);
        assert_eq!(g.critical_blockers[0].issue, "I0001");
        assert_eq!(g.critical_blockers[0].weight, 2);
        assert_eq!(g.critical_blockers[0].blocks, ["I0002", "I0003"]);
        assert!(g.is_partitioned());
    }

    /// Fan-out: one head, three independent followers. The followers are one parallel set
    /// once the head is done; before that they are all blocked and the head's weight is 3.
    #[test]
    fn fan_out_weights_the_head_by_everything_downstream() {
        let plan = plan_of(
            vec![
                node("I0001", "DONE", &[], &[]),
                node("I0002", "READY", &["I0001"], &["a"]),
                node("I0003", "READY", &["I0001"], &["b"]),
                node("I0004", "READY", &["I0001"], &["c"]),
            ],
            Vec::new(),
        );
        let g = MilestoneGraph::build(&plan, "m1", None);
        assert_eq!(g.ready, ["I0002", "I0003", "I0004"]);
        assert_eq!(g.complete, ["I0001"]);
        // disjoint scopes, one wave, all parallel-safe: one set
        assert_eq!(g.parallelizable.len(), 1);
        assert_eq!(g.parallelizable[0].issues, ["I0002", "I0003", "I0004"]);
    }

    /// Fan-in: three heads and one issue behind all of them. Each head blocks the same one
    /// thing, so the weights tie and the order falls to the canonical id.
    #[test]
    fn fan_in_ties_the_weights_and_orders_by_identity() {
        let plan = plan_of(
            vec![
                node("I0001", "READY", &[], &[]),
                node("I0002", "READY", &[], &[]),
                node("I0003", "READY", &[], &[]),
                node("I0004", "READY", &["I0001", "I0002", "I0003"], &[]),
            ],
            Vec::new(),
        );
        let g = MilestoneGraph::build(&plan, "m1", None);
        assert_eq!(g.blocked, ["I0004"]);
        let names: Vec<&str> = g
            .critical_blockers
            .iter()
            .map(|c| c.issue.as_str())
            .collect();
        assert_eq!(names, ["I0001", "I0002", "I0003"]);
        assert!(g.critical_blockers.iter().all(|c| c.weight == 1));
    }

    /// A closed dependency does not block: the plan drops it from `blocked_by`, so what was
    /// behind it is ready.
    #[test]
    fn a_closed_dependency_releases_what_waited_on_it() {
        let plan = plan_of(
            vec![
                node("I0001", "DONE", &[], &[]),
                node("I0002", "READY", &["I0001"], &[]),
            ],
            Vec::new(),
        );
        let g = MilestoneGraph::build(&plan, "m1", None);
        assert_eq!(g.ready, ["I0002"]);
        assert!(g.blocked.is_empty());
    }

    /// Two ready issues of one wave that share a scope path are serialised into two sets,
    /// and the conflict names the path — the same containment rule the plan warns with.
    #[test]
    fn a_shared_scope_path_serialises_two_otherwise_parallel_issues() {
        let plan = plan_of(
            vec![
                node("I0001", "READY", &[], &["apps/cli/src"]),
                node("I0002", "READY", &[], &["apps/cli/src/main.rs"]),
            ],
            Vec::new(),
        );
        let g = MilestoneGraph::build(&plan, "m1", None);
        assert_eq!(g.parallelizable.len(), 2);
        assert_eq!(g.parallelizable[0].issues, ["I0001"]);
        assert_eq!(g.parallelizable[1].issues, ["I0002"]);
        let conflict = &g.parallelizable[1].serialised[0];
        assert_eq!(conflict.held, "I0001");
        assert_eq!(conflict.excluded, "I0002");
        assert_eq!(conflict.path, "apps/cli/src");
    }

    /// An issue that declares itself not parallel-safe never joins a set.
    #[test]
    fn an_issue_that_is_not_parallel_safe_runs_alone() {
        let mut alone = node("I0002", "READY", &[], &[]);
        alone.parallel_safe = false;
        let plan = plan_of(vec![node("I0001", "READY", &[], &[]), alone], Vec::new());
        let g = MilestoneGraph::build(&plan, "m1", None);
        let sets: Vec<Vec<String>> = g.parallelizable.iter().map(|s| s.issues.clone()).collect();
        assert_eq!(
            sets,
            vec![vec!["I0001".to_string()], vec!["I0002".to_string()]]
        );
    }

    /// A cycle is reported as its component, and every issue in it loses its wave rather
    /// than being given a plausible one.
    #[test]
    fn a_cycle_is_reported_as_a_component_and_costs_the_wave() {
        let plan = plan_of(
            vec![
                node("I0001", "READY", &["I0002"], &[]),
                node("I0002", "READY", &["I0001"], &[]),
                node("I0003", "READY", &[], &[]),
            ],
            vec![PlanFinding {
                level: "FAIL".into(),
                code: "cycle".into(),
                subject: "graph".into(),
                message:
                    "a dependency cycle prevents these issues from ever becoming ready: I0001 I0002"
                        .into(),
            }],
        );
        let g = MilestoneGraph::build(&plan, "m1", None);
        assert_eq!(
            g.cycles,
            vec![vec!["I0001".to_string(), "I0002".to_string()]]
        );
        for id in ["I0001", "I0002"] {
            let n = g.nodes.iter().find(|n| n.issue == id).unwrap();
            assert!(n.wave.is_none(), "{id} was given a wave despite the cycle");
        }
        assert!(g
            .nodes
            .iter()
            .find(|n| n.issue == "I0003")
            .unwrap()
            .wave
            .is_some());
        assert!(g.has_failures());
    }

    /// A dependency the model does not declare is a diagnostic, and it does not silently
    /// vanish from the graph: the plan's finding reaches the milestone.
    #[test]
    fn a_missing_dependency_is_a_diagnostic_rather_than_a_silent_omission() {
        let plan = plan_of(
            vec![node("I0001", "READY", &["I9999"], &[])],
            vec![PlanFinding {
                level: "FAIL".into(),
                code: "unknown_dependency".into(),
                subject: "I0001".into(),
                message: "depends on I9999, which is not an issue".into(),
            }],
        );
        let g = MilestoneGraph::build(&plan, "m1", None);
        assert!(g
            .diagnostics
            .iter()
            .any(|d| d.code == "unknown_dependency" && d.level == "FAIL"));
        assert!(g.has_failures());
    }

    /// A milestone the model does not declare is answered with every field unknown, not
    /// refused and not empty.
    #[test]
    fn an_undeclared_milestone_is_answered_with_nothing_claiming_to_be_authored() {
        let plan = plan_of(vec![node("I0001", "READY", &[], &[])], Vec::new());
        let g = MilestoneGraph::build(&plan, "nonesuch", None);
        assert!(!g.declared);
        assert!(g.nodes.is_empty());
        assert_eq!(g.status.provenance, FieldProvenance::Unknown);
        assert_eq!(g.title.provenance, FieldProvenance::Unknown);
        assert!(g.is_consistent());
        assert!(g.is_partitioned(), "an empty graph is partitioned");
    }

    /// The order does not depend on the order the records were read in: the same set of
    /// issues, shuffled, produces the same graph.
    #[test]
    fn the_graph_is_the_same_whatever_order_the_records_arrive_in() {
        let forward = plan_of(
            vec![
                node("I0001", "READY", &[], &["a"]),
                node("I0002", "READY", &["I0001"], &["b"]),
                node("I0010", "READY", &[], &["c"]),
            ],
            Vec::new(),
        );
        let backward = plan_of(
            vec![
                node("I0010", "READY", &[], &["c"]),
                node("I0002", "READY", &["I0001"], &["b"]),
                node("I0001", "READY", &[], &["a"]),
            ],
            Vec::new(),
        );
        let a = MilestoneGraph::build(&forward, "m1", None);
        let b = MilestoneGraph::build(&backward, "m1", None);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
        // and the ids are in canonical order, so I0002 precedes I0010 by digit value
        let ids: Vec<&str> = a.nodes.iter().map(|n| n.issue.as_str()).collect();
        assert_eq!(ids, ["I0001", "I0002", "I0010"]);
    }
}
