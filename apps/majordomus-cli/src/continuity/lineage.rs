//! The lineage of published handovers: which record continues which, which line of work
//! each belongs to, and what two records — or two stores — are to each other.
//!
//! Order is causal, never chronological. A record names its parent, and a parent's id is a
//! digest of its content, so a record can only name records that existed before it: the
//! parent links form a forest, one tree per line, and "newer" means "descends from". Two
//! clocks on two machines are never compared to decide anything; `published_at` is shown
//! to a person and used for an age, nothing else.
//!
//! The distinction the whole design rests on is between a line that **diverged** — two
//! records continue the same record, so the same work was continued twice — and two lines
//! that are **independent**: separate work, started separately, on one machine or two.
//! The first is a conflict a person resolves; the second is ordinary.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::record::SignedRecord;
use crate::model::Diagnostic;

/// What one record is to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// The same record.
    Equal,
    /// The second descends from the first: the second is newer.
    Newer,
    /// The first descends from the second: the second is older.
    Older,
    /// Same line, neither descends from the other: the work was continued twice.
    Diverged,
    /// Different lines: separate work.
    Independent,
}

/// Whether a line has one current record or several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineState {
    /// One head: the line continues from it.
    Linear,
    /// More than one head: the same work was continued in more than one place.
    Diverged,
}

/// One line of work, as the store holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Line {
    /// The line's id: its first record's id.
    pub id: String,
    /// Linear or diverged.
    pub state: LineState,
    /// The records no other record continues, in id order.
    pub heads: Vec<String>,
    /// How many records the line holds.
    pub records: usize,
}

/// The admitted records of a store, with their parent links.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// Every admitted record, by id.
    pub records: BTreeMap<String, SignedRecord>,
    children: BTreeMap<String, BTreeSet<String>>,
}

/// The deepest parent chain followed; a longer one is reported, never followed further.
const MAX_DEPTH: usize = 100_000;

impl Graph {
    /// The graph of `records`.
    pub fn new(records: impl IntoIterator<Item = SignedRecord>) -> Self {
        let mut graph = Graph::default();
        for r in records {
            if let Some(parent) = &r.record.parent {
                graph
                    .children
                    .entry(parent.clone())
                    .or_default()
                    .insert(r.id.clone());
            }
            graph.records.insert(r.id.clone(), r);
        }
        graph
    }

    /// The line a record belongs to.
    pub fn line_of(&self, id: &str) -> Option<String> {
        self.records.get(id).map(|r| r.record.line_of(id))
    }

    /// Does `descendant` descend from `ancestor` (or equal it)?
    pub fn descends(&self, descendant: &str, ancestor: &str) -> bool {
        let mut at = descendant.to_string();
        for _ in 0..MAX_DEPTH {
            if at == ancestor {
                return true;
            }
            match self.records.get(&at).and_then(|r| r.record.parent.clone()) {
                Some(parent) => at = parent,
                None => return false,
            }
        }
        false
    }

    /// What `b` is to `a`.
    ///
    /// ```
    /// use majordomus_cli::continuity::lineage::{Graph, Relation};
    /// let g = Graph::new([]);
    /// assert_eq!(g.relation("x", "x"), Relation::Equal);
    /// ```
    pub fn relation(&self, a: &str, b: &str) -> Relation {
        if a == b {
            return Relation::Equal;
        }
        if self.line_of(a) != self.line_of(b) {
            return Relation::Independent;
        }
        if self.descends(b, a) {
            Relation::Newer
        } else if self.descends(a, b) {
            Relation::Older
        } else {
            Relation::Diverged
        }
    }

    /// The records no record continues.
    pub fn heads(&self) -> BTreeSet<String> {
        self.records
            .keys()
            .filter(|id| {
                !self
                    .children
                    .get(*id)
                    .is_some_and(|c| c.iter().any(|child| self.records.contains_key(child)))
            })
            .cloned()
            .collect()
    }

    /// Every line, by id.
    pub fn lines(&self) -> BTreeMap<String, Line> {
        let mut lines: BTreeMap<String, Line> = BTreeMap::new();
        for (id, r) in &self.records {
            let line = r.record.line_of(id);
            lines
                .entry(line.clone())
                .or_insert_with(|| Line {
                    id: line,
                    state: LineState::Linear,
                    heads: Vec::new(),
                    records: 0,
                })
                .records += 1;
        }
        for head in self.heads() {
            if let Some(line) = self.line_of(&head) {
                if let Some(l) = lines.get_mut(&line) {
                    l.heads.push(head);
                }
            }
        }
        for l in lines.values_mut() {
            if l.heads.len() > 1 {
                l.state = LineState::Diverged;
            }
        }
        lines
    }

    /// What is wrong with the lineage, as diagnostics: a parent the store does not hold, a
    /// record whose line is not its parent's, and a diverged line.
    pub fn findings(&self) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for (id, r) in &self.records {
            let path = Some(super::record::path_of(id));
            if let Some(parent) = &r.record.parent {
                match self.records.get(parent) {
                    None => out.push(Diagnostic::warning(
                        "continuity.dangling_parent",
                        path,
                        format!(
                            "continues {parent}, which this store does not hold; sync to fetch \
                             it, or the line was started in a store this one never merged"
                        ),
                    )),
                    Some(p) => {
                        if r.record.line.as_deref() != Some(p.record.line_of(parent).as_str()) {
                            out.push(Diagnostic::error(
                                "continuity.broken_lineage",
                                path,
                                format!(
                                    "declares line {} but its parent {parent} is on line {}",
                                    r.record.line.as_deref().unwrap_or("-"),
                                    p.record.line_of(parent)
                                ),
                            ));
                        }
                    }
                }
            }
        }
        for line in self.lines().values() {
            if line.state == LineState::Diverged {
                out.push(Diagnostic::warning(
                    "continuity.diverged",
                    None,
                    format!(
                        "line {} has {} heads ({}): the same work was continued in more than \
                         one place. Resume one with `continuity resume --record <id>`; the \
                         next publication from it continues that head",
                        line.id,
                        line.heads.len(),
                        line.heads.join(", ")
                    ),
                ));
            }
        }
        out
    }
}

/// How the local store and a remote's stand towards each other, for one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineSync {
    /// Both hold the same heads.
    Equal,
    /// The remote holds records that continue the local heads: fetching brings them.
    RemoteNewer,
    /// The local store holds records that continue the remote's: publishing sends them.
    LocalNewer,
    /// Each holds a continuation the other lacks: after merging, the line has two heads.
    Diverged,
    /// Only the local store holds the line.
    LocalOnly,
    /// Only the remote holds the line.
    RemoteOnly,
}

/// Compare one line across two graphs.
pub fn compare(local: &Graph, remote: &Graph, line: &str) -> LineSync {
    let heads = |g: &Graph| -> BTreeSet<String> {
        g.heads()
            .into_iter()
            .filter(|h| g.line_of(h).as_deref() == Some(line))
            .collect()
    };
    let (l, r) = (heads(local), heads(remote));
    match (l.is_empty(), r.is_empty()) {
        (true, true) | (false, true) => return LineSync::LocalOnly,
        (true, false) => return LineSync::RemoteOnly,
        _ => {}
    }
    if l == r {
        return LineSync::Equal;
    }
    // every head of one side is reachable from (contained in) the other's graph
    let contained = |inner: &BTreeSet<String>, outer: &Graph| {
        inner.iter().all(|h| {
            outer.records.contains_key(h) && outer.heads().iter().any(|oh| outer.descends(oh, h))
        })
    };
    match (contained(&l, remote), contained(&r, local)) {
        (true, _) => LineSync::RemoteNewer,
        (_, true) => LineSync::LocalNewer,
        _ => LineSync::Diverged,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::continuity::record::{tests_support::sample, SignedRecord};
    use crate::mesh::identity::NodeIdentity;

    /// A signed record continuing `parent` (on `parent`'s line), with `tag` in its body so
    /// that each is distinct.
    pub(crate) fn rec(
        identity: &NodeIdentity,
        parent: Option<&SignedRecord>,
        tag: &str,
    ) -> SignedRecord {
        let mut r = sample();
        r.device.node = identity.public.node_id.as_str().to_string();
        r.handover.body = format!("# Objective\n{tag}\n\n# Current State\nx\n\n# Next Action\ny\n");
        r.handover.id = crate::mesh::journal::HandoverBody::digest_of(&r.handover.body);
        if let Some(p) = parent {
            r.parent = Some(p.id.clone());
            r.line = Some(p.record.line_of(&p.id));
        }
        SignedRecord::sign(r, identity)
    }

    #[test]
    fn newer_older_equal_diverged_and_independent_are_told_apart() {
        let a_dev = NodeIdentity::ephemeral().unwrap();
        let b_dev = NodeIdentity::ephemeral().unwrap();
        let a = rec(&a_dev, None, "a");
        let b = rec(&a_dev, Some(&a), "b");
        let c = rec(&b_dev, Some(&a), "c");
        let other = rec(&b_dev, None, "other line");
        let g = Graph::new([a.clone(), b.clone(), c.clone(), other.clone()]);
        assert_eq!(g.relation(&a.id, &a.id), Relation::Equal);
        assert_eq!(g.relation(&a.id, &b.id), Relation::Newer);
        assert_eq!(g.relation(&b.id, &a.id), Relation::Older);
        assert_eq!(g.relation(&b.id, &c.id), Relation::Diverged);
        assert_eq!(g.relation(&b.id, &other.id), Relation::Independent);

        let lines = g.lines();
        assert_eq!(lines[&a.id].state, LineState::Diverged);
        assert_eq!(lines[&a.id].heads.len(), 2);
        assert_eq!(
            lines[&other.id].state,
            LineState::Linear,
            "separate work is not a conflict"
        );
        let codes: Vec<String> = g.findings().into_iter().map(|d| d.code).collect();
        assert_eq!(codes, vec!["continuity.diverged".to_string()]);
    }

    #[test]
    fn the_clock_decides_nothing() {
        let dev = NodeIdentity::ephemeral().unwrap();
        let a = rec(&dev, None, "a");
        let mut b_record = rec(&dev, Some(&a), "b").record;
        // the child claims to be published years before its parent
        b_record.published_at = "2001-01-01T00:00:00Z".into();
        let b = SignedRecord::sign(b_record, &dev);
        let g = Graph::new([a.clone(), b.clone()]);
        assert_eq!(g.relation(&a.id, &b.id), Relation::Newer);
    }

    #[test]
    fn stores_compare_per_line() {
        let dev = NodeIdentity::ephemeral().unwrap();
        let a = rec(&dev, None, "a");
        let b = rec(&dev, Some(&a), "b");
        let c = rec(&dev, Some(&a), "c");
        let line = a.id.clone();
        let only_a = Graph::new([a.clone()]);
        let ab = Graph::new([a.clone(), b.clone()]);
        let ac = Graph::new([a.clone(), c.clone()]);
        assert_eq!(compare(&only_a, &only_a, &line), LineSync::Equal);
        assert_eq!(compare(&only_a, &ab, &line), LineSync::RemoteNewer);
        assert_eq!(compare(&ab, &only_a, &line), LineSync::LocalNewer);
        assert_eq!(compare(&ab, &ac, &line), LineSync::Diverged);
        assert_eq!(compare(&ab, &Graph::default(), &line), LineSync::LocalOnly);
        assert_eq!(compare(&Graph::default(), &ab, &line), LineSync::RemoteOnly);
    }

    #[test]
    fn a_dangling_parent_and_a_broken_line_are_reported() {
        let dev = NodeIdentity::ephemeral().unwrap();
        let a = rec(&dev, None, "a");
        let b = rec(&dev, Some(&a), "b");
        let g = Graph::new([b.clone()]);
        assert_eq!(g.findings()[0].code, "continuity.dangling_parent");

        let mut bad = b.record.clone();
        bad.line = Some("e".repeat(32));
        let bad = SignedRecord::sign(bad, &dev);
        let g = Graph::new([a, bad]);
        assert!(g
            .findings()
            .iter()
            .any(|d| d.code == "continuity.broken_lineage"));
    }
}
