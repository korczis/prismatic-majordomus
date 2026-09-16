//! Where the work of this repository is being held, and whether any of it is held
//! somewhere it can be lost.
//!
//! # The problem this exists for
//!
//! `project.a-worker-that-stops-leaves-its-work-behind` is written down as advisory, and
//! says why: *"nothing can see the uncommitted half except somebody looking, which is
//! exactly why the responsibility is written down rather than gated."* That premise was
//! true when the rule was written and is not true now. [`crate::worktree`] already counts
//! the uncommitted work of every registered worktree, and git answers in one call which
//! commits of this repository have never reached a remote. What was missing was not a
//! measurement but a **verdict** over the repository: one word that says whether every
//! unit of work is somewhere another worker could find it.
//!
//! # What a holding is
//!
//! A holding is anything that can hold work: a worktree (its uncommitted files), a branch
//! (its commits), a stash (its diff). Each is given a [`Disposition`] from the evidence,
//! and a holding whose disposition is [`Disposition::LocalOnly`] or
//! [`Disposition::Uncommitted`] is **at risk**: it exists on one disk, it is invisible to
//! every other worker, and a worker that stops is not a rollback.
//!
//! # What this is not
//!
//! It is not a second reading of the topology — [`crate::worktree::WorktreeService`] is
//! the only one, and this composes its answer. It is not a pull-request inventory: a pull
//! request lives on a forge, reaching one is a network call, and the completion invariant
//! already asks whether the trunk reaches a commit (`integrated`). This answers the half
//! that is decidable offline, from this repository alone, in one pass.
//!
//! ```
//! use majordomus_cli::convergence::Disposition;
//!
//! // the two dispositions that mean "this exists on one disk only"
//! assert!(Disposition::LocalOnly.at_risk() && Disposition::Uncommitted.at_risk());
//! // and the two that mean somebody else could pick it up
//! assert!(!Disposition::Integrated.at_risk() && !Disposition::Published.at_risk());
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::worktree::{git, Detail, Result, WorktreeService};

/// The schema this module answers under.
pub const SCHEMA: &str = "convergence/v1";

// ---------------------------------------------------------------- vocabulary

/// What kind of thing is holding the work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HoldingKind {
    /// A registered work tree, holding files that are not committed.
    Worktree,
    /// A local branch, holding commits.
    Branch,
    /// An entry of `git stash`.
    Stash,
}

impl HoldingKind {
    /// The word this kind is reported under.
    pub fn as_str(&self) -> &'static str {
        match self {
            HoldingKind::Worktree => "worktree",
            HoldingKind::Branch => "branch",
            HoldingKind::Stash => "stash",
        }
    }
}

/// Where the work a holding carries can be reached from.
///
/// The order is the order of safety: everything above [`Disposition::LocalOnly`] is
/// reachable by somebody who is not the worker that made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// The trunk reaches it: it is in the repository's own history.
    Integrated,
    /// A remote reaches it: another checkout can fetch it, and it survives this disk.
    Published,
    /// Committed here and nowhere else. One disk away from being lost.
    LocalOnly,
    /// Not committed at all: files in a work tree, or a stash entry.
    Uncommitted,
}

impl Disposition {
    /// The word this disposition is reported under.
    pub fn as_str(&self) -> &'static str {
        match self {
            Disposition::Integrated => "integrated",
            Disposition::Published => "published",
            Disposition::LocalOnly => "local_only",
            Disposition::Uncommitted => "uncommitted",
        }
    }

    /// This holding's work exists on one disk only.
    pub fn at_risk(&self) -> bool {
        matches!(self, Disposition::LocalOnly | Disposition::Uncommitted)
    }
}

// ---------------------------------------------------------------- the documents

/// One thing holding work, and what is known about where that work can be reached from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Holding {
    /// What kind of thing holds it.
    pub kind: HoldingKind,
    /// How it is named: a branch name, an absolute worktree path, a stash ref.
    pub identity: String,
    /// Where the work can be reached from.
    pub disposition: Disposition,
    /// What was actually read — never a restatement of the disposition.
    pub evidence: String,
    /// The command that would move it out of danger. Empty when it is not at risk.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub remedy: String,
    /// This holding's work exists on one disk only.
    pub at_risk: bool,
}

/// The repository's convergence: every holding, and the one verdict over them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConvergenceReport {
    /// [`SCHEMA`].
    pub schema: String,
    /// The primary checkout this was measured from.
    pub repository: String,
    /// The trunk the containment was decided against, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
    /// Every holding, at-risk ones first, then by kind and identity.
    pub holdings: Vec<Holding>,
    /// How many holdings carry each disposition, by its word.
    pub tallies: BTreeMap<String, usize>,
    /// The holdings that exist on one disk only.
    pub at_risk: usize,
    /// Nothing is held where it can be lost.
    pub converged: bool,
}

impl ConvergenceReport {
    /// The one line a person or a gate reads first.
    pub fn summary(&self) -> String {
        if self.converged {
            return format!(
                "converged: {} holding(s), every one reachable by somebody else",
                self.holdings.len()
            );
        }
        format!(
            "not converged: {} of {} holding(s) exist on this disk only",
            self.at_risk,
            self.holdings.len()
        )
    }
}

// ---------------------------------------------------------------- the measurement

/// Measure the repository holding `root`.
///
/// One topology read (the same service the hooks and the Cockpit ask), one `git rev-list`
/// for the commits no remote reaches, one `git stash list`. Nothing here writes, and
/// nothing here reaches the network: a remote-tracking ref is what this checkout last
/// fetched, which is exactly the question — whether the work left this disk, not whether
/// the remote still has it.
pub fn report(root: &Path) -> Result<ConvergenceReport> {
    let service = WorktreeService::open(root)?;
    let topology = service.topology(Detail::Full)?;
    let primary = topology.repository.primary_worktree.clone();
    let unreachable = commits_no_remote_reaches(Path::new(&primary))?;
    let stashes = stash_entries(Path::new(&primary))?;

    let mut holdings = Vec::new();

    for worktree in &topology.worktrees {
        let Some(dirty) = worktree.dirty.as_ref() else {
            continue;
        };
        if dirty.clean {
            continue;
        }
        holdings.push(Holding {
            kind: HoldingKind::Worktree,
            identity: worktree.path.clone(),
            disposition: Disposition::Uncommitted,
            evidence: format!(
                "{} — {}",
                worktree.label,
                dirty.summary()
            ),
            remedy: format!(
                "git -C {} add -A && git -C {} commit -m 'checkpoint: unreviewed'",
                worktree.path, worktree.path
            ),
            at_risk: true,
        });
    }

    for branch in &topology.branches {
        let (disposition, evidence) = if branch.merged_into_trunk == Some(true) {
            (
                Disposition::Integrated,
                format!("the trunk reaches {}", short(&branch.head)),
            )
        } else if unreachable.contains(&branch.head) {
            (
                Disposition::LocalOnly,
                format!(
                    "{} is on no remote-tracking ref of this checkout",
                    short(&branch.head)
                ),
            )
        } else {
            (
                Disposition::Published,
                format!("a remote-tracking ref reaches {}", short(&branch.head)),
            )
        };
        let at_risk = disposition.at_risk();
        holdings.push(Holding {
            kind: HoldingKind::Branch,
            identity: branch.name.clone(),
            disposition,
            evidence,
            remedy: if at_risk {
                format!("git push -u origin {}", branch.name)
            } else {
                String::new()
            },
            at_risk,
        });
    }

    for (reference, subject) in stashes {
        holdings.push(Holding {
            kind: HoldingKind::Stash,
            identity: reference.clone(),
            disposition: Disposition::Uncommitted,
            evidence: subject,
            remedy: format!("git stash show -p {reference}"),
            at_risk: true,
        });
    }

    holdings.sort_by(|a, b| {
        b.at_risk
            .cmp(&a.at_risk)
            .then(a.kind.cmp(&b.kind))
            .then(a.identity.cmp(&b.identity))
    });

    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    for holding in &holdings {
        *tallies
            .entry(holding.disposition.as_str().to_string())
            .or_default() += 1;
    }
    let at_risk = holdings.iter().filter(|h| h.at_risk).count();

    Ok(ConvergenceReport {
        schema: SCHEMA.to_string(),
        repository: primary,
        trunk: topology.trunk.branch.clone(),
        holdings,
        tallies,
        at_risk,
        converged: at_risk == 0,
    })
}

/// The commits that are on a local branch and on no remote-tracking ref.
///
/// One call: `git rev-list --branches --not --remotes`. Asking per branch would be one
/// subprocess per branch, and this repository has three hundred of them. `--no-walk` is
/// deliberately *not* used — it drops the exclusion, and the probe then reports that
/// nothing is unpublished no matter what is unpublished.
fn commits_no_remote_reaches(primary: &Path) -> Result<BTreeSet<String>> {
    let out = git::run(primary, &["rev-list", "--branches", "--not", "--remotes"])?;
    Ok(out
        .text()?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// Every stash entry: its ref and the subject git records for it.
fn stash_entries(primary: &Path) -> Result<Vec<(String, String)>> {
    let out = git::run(primary, &["stash", "list", "--format=%gd%x09%gs"])?;
    Ok(out
        .text()?
        .lines()
        .filter_map(|line| {
            let (reference, subject) = line.split_once('\t')?;
            if reference.trim().is_empty() {
                return None;
            }
            Some((reference.trim().to_string(), subject.trim().to_string()))
        })
        .collect())
}

/// A commit, abbreviated the way git abbreviates it in prose.
fn short(commit: &str) -> &str {
    let end = commit.len().min(9);
    &commit[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vocabulary is the contract: a reader that persists these words must see the
    /// same ones tomorrow.
    #[test]
    fn the_words_are_stable() {
        assert_eq!(Disposition::Integrated.as_str(), "integrated");
        assert_eq!(Disposition::Published.as_str(), "published");
        assert_eq!(Disposition::LocalOnly.as_str(), "local_only");
        assert_eq!(Disposition::Uncommitted.as_str(), "uncommitted");
        assert_eq!(HoldingKind::Worktree.as_str(), "worktree");
        assert_eq!(HoldingKind::Branch.as_str(), "branch");
        assert_eq!(HoldingKind::Stash.as_str(), "stash");
    }

    /// Exactly the two dispositions that mean "one disk" are at risk. This is the
    /// judgement the gate rests on, so it is asserted rather than assumed.
    #[test]
    fn at_risk_is_exactly_the_unreachable_two() {
        let all = [
            Disposition::Integrated,
            Disposition::Published,
            Disposition::LocalOnly,
            Disposition::Uncommitted,
        ];
        let at_risk: Vec<&str> = all
            .iter()
            .filter(|d| d.at_risk())
            .map(|d| d.as_str())
            .collect();
        assert_eq!(at_risk, vec!["local_only", "uncommitted"]);
    }

    /// A summary states the count it refuses over, so a person reading the one line knows
    /// how much is in danger without reading the list.
    #[test]
    fn the_summary_states_the_count_it_refuses_over() {
        let mut report = ConvergenceReport {
            schema: SCHEMA.into(),
            repository: "/tmp/r".into(),
            trunk: Some("master".into()),
            holdings: vec![Holding {
                kind: HoldingKind::Branch,
                identity: "feature/x".into(),
                disposition: Disposition::LocalOnly,
                evidence: "abc on no remote".into(),
                remedy: "git push -u origin feature/x".into(),
                at_risk: true,
            }],
            tallies: BTreeMap::new(),
            at_risk: 1,
            converged: false,
        };
        assert!(report.summary().contains("1 of 1"));
        report.at_risk = 0;
        report.converged = true;
        assert!(report.summary().starts_with("converged"));
    }

    /// An abbreviation never panics on a short or empty commit: the topology can answer
    /// with an empty head for a branch git could not resolve.
    #[test]
    fn abbreviating_a_short_commit_is_not_a_panic() {
        assert_eq!(short(""), "");
        assert_eq!(short("abc"), "abc");
        assert_eq!(short("0123456789abcdef"), "012345678");
    }
}
