//! `prs batch-check`: a composed branch that is not a batch is refused (ADR 0114 D5).
//!
//! # What it asks
//!
//! Whether the branch under test merges the heads of two or more *other* open pull requests
//! of this repository, and, when it does, whether it is the batch its manifest says it is.
//! A hand-built batch — several `git merge --no-ff` of other pull requests' heads, a fix
//! written on the batch branch, no record of who rode — is what ADR 0114 replaced, and
//! nothing in CI could tell one from a branch of ordinary work. This is the gate that can.
//!
//! # How it decides
//!
//! From git alone, once the open pull requests' heads are known. It walks the first-parent
//! line from `merge-base(base, head)` to the head. A commit there with a second parent is
//!
//! - a **member merge** when it has exactly two parents and the second is the *current head*
//!   of another open pull request of this repository. "Current head", not an ancestor of
//!   one: ADR 0114 D5 speaks of a head that "contains the heads of two or more other open
//!   pull requests", and D3 records for each member "its head". A member whose owner pushed
//!   since is no longer what the batch carries, and D4's answer to that is to compose again;
//! - a **merge of the base** when every other parent is an ancestor of the base, which is
//!   what a refresh leaves and is nobody's member;
//! - anything else otherwise, and then it is judged like any commit that is not a merge.
//!
//! Two things make a branch a batch to be judged, and either is enough: it merges two or more
//! distinct pull requests that way, or it adds or changes a manifest under
//! [`super::compose::BATCH_DIR`] relative to the base. Neither: not a batch — one merge and
//! no manifest is a stack. The manifest decides on its own because a batch whose members
//! moved or landed since it was composed still says what it carries: counted by its merges
//! alone it would be "not a batch", and a stale record would go unjudged.
//!
//! A batch to be judged must add or change exactly one manifest; that manifest's members
//! must be the member merges' pull requests in first-parent order, at least two of them, each
//! `head` the merge's second parent and each `merge_commit` the merge; and every other commit
//! on the line — neither a member merge nor a merge of the base — may change only the
//! manifest, the two files `release bump` writes ([`crate::release::version::MANIFEST`],
//! [`crate::release::version::LOCK`]) and paths the head's own `.gitattributes` marks
//! `merge=derived`.
//!
//! # What it never does
//!
//! Report clean because it could not look. The open pull requests are the forge's to list,
//! and they are asked for only when git alone cannot already answer: a branch that touches
//! no manifest and has fewer than two merges of anything but the base on its first-parent
//! line is not a batch whatever the forge would say. When they are needed and cannot be
//! read, the answer is an error —
//! exit 12 on the command line, "cannot run" — never a verdict.
//!
//! The forge tests a pull request on a merge commit of its own making (`refs/pull/<n>/merge`:
//! first parent the base, second parent the pull request's head). A head of that shape is
//! looked through — the second parent is judged — and the report says so.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::compose::{short, BatchManifest, BATCH_DIR};

/// One open pull request of this repository, as the forge listed it: the only two facts the
/// gate takes from outside git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenHead {
    /// The pull request.
    pub number: u64,
    /// Its current head commit.
    pub head: String,
}

/// A merge commit on the first-parent line whose second parent is another open pull
/// request's current head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MemberMerge {
    /// The pull request whose head was merged.
    pub number: u64,
    /// That head: the merge commit's second parent.
    pub head: String,
    /// The merge commit.
    pub merge_commit: String,
}

/// One way a branch that merges several pull requests is not the batch its manifest says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BatchFinding {
    /// The branch merges these pull requests' heads and adds no manifest.
    ManifestMissing {
        /// The member merges that were found.
        members: Vec<MemberMerge>,
    },
    /// The branch adds or changes more than one manifest: a batch has one identity.
    ManifestAmbiguous {
        /// The manifests it touches.
        paths: Vec<String>,
    },
    /// The manifest is not an `integration-batch/v1` document.
    ManifestUnreadable {
        /// The manifest.
        path: String,
        /// Why it could not be read.
        reason: String,
    },
    /// A member merge the manifest does not name.
    MemberNotInManifest {
        /// The pull request that was merged.
        number: u64,
        /// The merge commit that carries it.
        merge_commit: String,
        /// The manifest that is silent about it.
        manifest: String,
    },
    /// A member the manifest names that no merge on the first-parent line carries.
    MemberNotMerged {
        /// The pull request the manifest names.
        number: u64,
        /// The manifest.
        manifest: String,
        /// The line of the manifest that names it.
        line: usize,
    },
    /// The manifest lists a member at another position than its merge has on the line.
    MemberOutOfOrder {
        /// The 1-based position.
        position: usize,
        /// The pull request the manifest has there.
        listed: u64,
        /// The pull request whose merge is there.
        merged: u64,
        /// The manifest.
        manifest: String,
        /// The line of the manifest that names `listed`.
        line: usize,
    },
    /// The manifest's `head` for a member is not the second parent of its merge.
    WrongHead {
        /// The member.
        number: u64,
        /// What the manifest says.
        listed: String,
        /// The merge's second parent.
        merged: String,
        /// The manifest.
        manifest: String,
        /// The line of the manifest that names the member.
        line: usize,
    },
    /// The manifest's `merge_commit` for a member is not the commit that merges it.
    WrongMergeCommit {
        /// The member.
        number: u64,
        /// What the manifest says.
        listed: String,
        /// The merge commit on the line.
        merged: String,
        /// The manifest.
        manifest: String,
        /// The line of the manifest that names the member.
        line: usize,
    },
    /// The manifest names exactly the member merges, and there are fewer than two: one pull
    /// request is `prs repair`, and none is no composition at all.
    TooFewMembers {
        /// How many member merges the first-parent line holds.
        found: usize,
        /// The manifest.
        manifest: String,
    },
    /// A commit of the batch itself changes a path no composition may: not the manifest, not
    /// a version file `release bump` writes, not a `merge=derived` path.
    ForeignChange {
        /// The commit on the first-parent line.
        commit: String,
        /// Its subject.
        subject: String,
        /// The path it changes.
        path: String,
    },
}

impl std::fmt::Display for BatchFinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchFinding::ManifestMissing { members } => write!(
                f,
                "no manifest under {BATCH_DIR}/ is added, and the branch merges {}: a batch is \
                 composed by `majordomus prs compose --apply`, which writes it",
                members
                    .iter()
                    .map(|m| format!(
                        "#{} at {} (merge {})",
                        m.number,
                        short(&m.head),
                        short(&m.merge_commit)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            BatchFinding::ManifestAmbiguous { paths } => write!(
                f,
                "{} manifests are added or changed ({}): a batch has exactly one",
                paths.len(),
                paths.join(", ")
            ),
            BatchFinding::ManifestUnreadable { path, reason } => {
                write!(f, "{path} is not a batch manifest: {reason}")
            }
            BatchFinding::MemberNotInManifest {
                number,
                merge_commit,
                manifest,
            } => write!(
                f,
                "commit {} merges #{number}, and {manifest} names no member #{number}",
                short(merge_commit)
            ),
            BatchFinding::MemberNotMerged {
                number,
                manifest,
                line,
            } => write!(
                f,
                "{manifest}:{line} names member #{number}, and no merge on the first-parent \
                 line has #{number}'s current head as its second parent"
            ),
            BatchFinding::MemberOutOfOrder {
                position,
                listed,
                merged,
                manifest,
                line,
            } => write!(
                f,
                "{manifest}:{line} lists #{listed} as member {position}, and the merge at that \
                 position on the first-parent line is #{merged}'s"
            ),
            BatchFinding::WrongHead {
                number,
                listed,
                merged,
                manifest,
                line,
            } => write!(
                f,
                "{manifest}:{line} gives #{number} the head {}, and its merge's second parent \
                 is {}",
                short(listed),
                short(merged)
            ),
            BatchFinding::WrongMergeCommit {
                number,
                listed,
                merged,
                manifest,
                line,
            } => write!(
                f,
                "{manifest}:{line} gives #{number} the merge commit {}, and the commit that \
                 merges it is {}",
                short(listed),
                short(merged)
            ),
            BatchFinding::TooFewMembers { found, manifest } => write!(
                f,
                "{manifest} is a batch of {found} member(s), and a batch has at least {}: one \
                 pull request is `majordomus prs repair`",
                super::compose::MIN_MEMBERS
            ),
            BatchFinding::ForeignChange {
                commit,
                subject,
                path,
            } => write!(
                f,
                "commit {} ({subject}) changes {path} on the batch branch: a composition \
                 changes only its manifest, the version files and derived paths; a fix is \
                 written on the member's branch and the batch is composed again",
                short(commit)
            ),
        }
    }
}

/// What the gate decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BatchVerdict {
    /// The branch merges fewer than two other open pull requests and touches no manifest:
    /// not this gate's subject.
    NotABatch,
    /// It merges two or more, or touches a manifest, and it is the batch its manifest says.
    Batch,
    /// It merges two or more, or touches a manifest, and it is not.
    Refused,
}

/// The answer of one `prs batch-check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BatchCheck {
    /// The base, as it was named.
    pub base: String,
    /// `merge-base(base, head)`: where the first-parent line that was walked starts.
    pub merge_base: String,
    /// The commit that was judged.
    pub head: String,
    /// The forge's test merge that was looked through to reach `head`, when the head that
    /// was named is one.
    pub looked_through: Option<String>,
    /// Whether the forge's list of open pull requests was read. `false` when git alone
    /// decided: the branch touches no manifest and the line holds fewer than two merges of
    /// anything but the base.
    pub forge_read: bool,
    /// The member merges, in first-parent order.
    pub members: Vec<MemberMerge>,
    /// The one manifest the branch adds or changes, when it adds exactly one.
    pub manifest: Option<String>,
    /// The verdict.
    pub verdict: BatchVerdict,
    /// Why it is refused; empty otherwise.
    pub findings: Vec<BatchFinding>,
}

impl BatchCheck {
    /// The verdict in one line, as the command line says it.
    pub fn summary(&self) -> String {
        let at = format!("{} from {}", short(&self.head), self.base);
        match self.verdict {
            BatchVerdict::NotABatch => format!(
                "batch-check: not a batch: {at} merges {} other open pull request(s) on its \
                 first-parent line and touches no batch manifest, and a batch has at least two{}",
                self.members.len(),
                if self.forge_read {
                    ""
                } else {
                    " (decided by git alone)"
                }
            ),
            BatchVerdict::Batch => format!(
                "batch-check: ok: {at} is the batch {} says: {}",
                self.manifest.as_deref().unwrap_or("its manifest"),
                numbered(&self.members)
            ),
            BatchVerdict::Refused => match &self.manifest {
                // judged for its manifest alone: what it merges would not have made it a batch
                Some(manifest) if self.members.len() < super::compose::MIN_MEMBERS => format!(
                    "batch-check: REFUSED: {at} adds or changes {manifest}, merges {} and is \
                     not the batch that manifest says ({} finding(s))",
                    numbered(&self.members),
                    self.findings.len()
                ),
                _ => format!(
                    "batch-check: REFUSED: {at} merges {} and is not a batch ({} finding(s))",
                    numbered(&self.members),
                    self.findings.len()
                ),
            },
        }
    }
}

fn numbered(members: &[MemberMerge]) -> String {
    if members.is_empty() {
        return "no other open pull request".to_string();
    }
    members
        .iter()
        .map(|m| format!("#{}", m.number))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One git command in `root`: what it printed, or what it was and why it failed.
fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("--no-pager")
        .args(args)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn is_ancestor(root: &Path, a: &str, b: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", "--is-ancestor", a, b])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The NUL-separated paths a `-z` listing printed.
fn paths_of(listing: &str) -> Vec<String> {
    listing
        .split('\0')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// The 1-based line of `text` that opens the member `number`, or 1 when none does.
fn member_line(text: &str, number: u64) -> usize {
    let wanted = format!("- number: {number}");
    text.lines()
        .position(|l| l.trim() == wanted)
        .map_or(1, |i| i + 1)
}

/// The base a check is made against when none is named: the base the forge was last
/// observed to name, as this clone's `origin/<base>`; then what `origin/HEAD` points at;
/// then the first of `origin/master`, `origin/main`, `master`, `main` this clone has — the
/// order `scripts/ci-plan` looks in. `Err` when this clone has none of them.
pub fn default_base(root: &Path) -> Result<String, String> {
    let has = |r: &str| git(root, &["rev-parse", "--verify", "--quiet", r]).is_ok();
    if let Ok(Some(obs)) = super::load_observation(root) {
        let named = format!("origin/{}", obs.base);
        if has(&named) {
            return Ok(named);
        }
    }
    if let Ok(head) = git(
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        if has(&head) {
            return Ok(head);
        }
    }
    ["origin/master", "origin/main", "master", "main"]
        .into_iter()
        .find(|r| has(r))
        .map(str::to_string)
        .ok_or_else(|| {
            "no base to check against: this clone has no origin/master, origin/main, master \
             or main; name one with --base"
                .to_string()
        })
}

/// The open pull requests of this repository, read through the subsystem's own forge adapter
/// ([`super::forge::GhForge::open_heads`]): their numbers and current heads and nothing
/// else, so the gate needs no more of a CI token than the right to list pull requests. A
/// pull request whose head lives in a fork is not "of this repository" and cannot be a
/// member (ADR 0114 D2). A read: nothing is fetched, stored or recorded.
pub fn open_heads(root: &Path) -> Result<Vec<OpenHead>, String> {
    Ok(super::forge::GhForge { root }
        .open_heads()
        .map_err(|e| e.0)?
        .into_iter()
        .map(|(number, head)| OpenHead { number, head })
        .collect())
}

/// One commit of the first-parent line, with its parents.
struct Step {
    commit: String,
    parents: Vec<String>,
}

/// Judge `head` against `base`. `open` lists the other open pull requests' heads and is asked
/// only when git alone cannot decide. `Err` is "cannot run": git could not answer, or the
/// open pull requests were needed and could not be listed — never a verdict.
///
/// ```text
/// use crate::integration::batch::{check, BatchVerdict};
/// // a branch of ordinary commits: git decides, and the forge is never asked
/// let report = check(root, "origin/master", "HEAD", || Err("no forge".into())).unwrap();
/// assert_eq!(report.verdict, BatchVerdict::NotABatch);
/// assert!(!report.forge_read);
/// ```
pub fn check(
    root: &Path,
    base: &str,
    head: &str,
    open: impl FnOnce() -> Result<Vec<OpenHead>, String>,
) -> Result<BatchCheck, String> {
    let commit = |r: &str| git(root, &["rev-parse", "--verify", &format!("{r}^{{commit}}")]);
    let base_sha = commit(base).map_err(|e| format!("the base {base} is not a commit: {e}"))?;
    let named = commit(head).map_err(|e| format!("the head {head} is not a commit: {e}"))?;
    // the forge's test merge: first parent on the base, second parent the pull request
    let parents: Vec<String> = git(root, &["rev-list", "--parents", "-1", &named])?
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect();
    let (judged, looked_through) = match parents.as_slice() {
        [first, second] if is_ancestor(root, first, &base_sha) => {
            (second.clone(), Some(named.clone()))
        }
        _ => (named, None),
    };
    let merge_base = git(root, &["merge-base", &base_sha, &judged])
        .map_err(|e| format!("{base} and {head} share no history: {e}"))?;
    let line: Vec<Step> = git(
        root,
        &[
            "rev-list",
            "--first-parent",
            "--reverse",
            "--parents",
            &format!("{merge_base}..{judged}"),
        ],
    )?
    .lines()
    .filter_map(|l| {
        let mut ids = l.split_whitespace().map(str::to_string);
        ids.next().map(|commit| Step {
            commit,
            parents: ids.collect(),
        })
    })
    .collect();
    // a merge of the base is nobody's member, and what is left is all that could be one
    let of_base = |s: &Step| {
        s.parents.len() > 1
            && s.parents[1..]
                .iter()
                .all(|p| is_ancestor(root, p, &base_sha))
    };
    let candidates = line
        .iter()
        .filter(|s| s.parents.len() > 1 && !of_base(s))
        .count();
    let mut report = BatchCheck {
        base: base.to_string(),
        merge_base: merge_base.clone(),
        head: judged.clone(),
        looked_through,
        forge_read: false,
        members: Vec::new(),
        manifest: None,
        verdict: BatchVerdict::NotABatch,
        findings: Vec::new(),
    };
    // a manifest the branch adds or changes makes it a batch to be judged, whatever it
    // merges: renames are not followed, so a manifest moved to another name is one added
    let manifests: Vec<String> = paths_of(&git(
        root,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            "--diff-filter=AM",
            &merge_base,
            &judged,
            "--",
            BATCH_DIR,
        ],
    )?)
    .into_iter()
    .filter(|p| p.ends_with(".yaml"))
    .collect();
    if manifests.is_empty() && candidates < super::compose::MIN_MEMBERS {
        return Ok(report);
    }
    let others: Vec<OpenHead> = open()
        .map_err(|e| {
            format!(
                "the open pull requests could not be listed, and {} merges {candidates} \
                 commit(s) that are not the base's on its first-parent line and adds or \
                 changes {} batch manifest(s): {e}",
                short(&judged),
                manifests.len()
            )
        })?
        .into_iter()
        // the pull request under test is not another one
        .filter(|o| o.head != judged)
        .collect();
    report.forge_read = true;
    let pr_of = |sha: &str| others.iter().find(|o| o.head == sha).map(|o| o.number);
    // every pull request whose head a commit of the line merges, however many parents it has
    let merged: BTreeSet<u64> = line
        .iter()
        .flat_map(|s| s.parents.iter().skip(1))
        .filter_map(|p| pr_of(p))
        .collect();
    let member_of = |s: &Step| match s.parents.as_slice() {
        [_, second] => pr_of(second).map(|number| MemberMerge {
            number,
            head: second.clone(),
            merge_commit: s.commit.clone(),
        }),
        _ => None,
    };
    report.members = line.iter().filter_map(member_of).collect();
    if manifests.is_empty() && merged.len() < super::compose::MIN_MEMBERS {
        return Ok(report);
    }
    let members = report.members.clone();
    let mut findings: Vec<BatchFinding> = Vec::new();
    match manifests.as_slice() {
        [] => findings.push(BatchFinding::ManifestMissing {
            members: members.clone(),
        }),
        [path] => {
            report.manifest = Some(path.clone());
            let text = git(root, &["show", &format!("{judged}:{path}")])?;
            match BatchManifest::parse(&text) {
                Err(reason) => findings.push(BatchFinding::ManifestUnreadable {
                    path: path.clone(),
                    reason,
                }),
                Ok(m) => {
                    let differences = compare(&members, &m, path, &text);
                    // a manifest that names exactly the one merge there is, or none
                    if differences.is_empty() && members.len() < super::compose::MIN_MEMBERS {
                        findings.push(BatchFinding::TooFewMembers {
                            found: members.len(),
                            manifest: path.clone(),
                        });
                    }
                    findings.extend(differences);
                }
            }
        }
        paths => findings.push(BatchFinding::ManifestAmbiguous {
            paths: paths.to_vec(),
        }),
    }
    // every commit of the batch itself: neither a member's merge nor the base's
    for s in line
        .iter()
        .filter(|s| !of_base(s) && member_of(s).is_none())
    {
        let changed = paths_of(&git(
            root,
            &[
                "diff",
                "--name-only",
                "-z",
                &format!("{}^1", s.commit),
                &s.commit,
            ],
        )?);
        let derived = super::relation::derived_paths(root, &judged, &changed)?;
        let subject = git(root, &["log", "-1", "--format=%s", &s.commit]).unwrap_or_default();
        findings.extend(
            changed
                .into_iter()
                .filter(|p| {
                    report.manifest.as_deref() != Some(p.as_str())
                        && p != crate::release::version::MANIFEST
                        && p != crate::release::version::LOCK
                        && !derived.contains(p)
                })
                .map(|path| BatchFinding::ForeignChange {
                    commit: s.commit.clone(),
                    subject: subject.clone(),
                    path,
                }),
        );
    }
    report.verdict = if findings.is_empty() {
        BatchVerdict::Batch
    } else {
        BatchVerdict::Refused
    };
    report.findings = findings;
    Ok(report)
}

/// How the manifest `m` at `path` (its `text`) disagrees with the member merges git found:
/// a merge it does not name, a member it names that was not merged, then — when it names
/// exactly the merged — a member at another position, and for every member both know a head
/// or a merge commit that is not the merge's. Pure.
pub fn compare(
    merges: &[MemberMerge],
    m: &BatchManifest,
    path: &str,
    text: &str,
) -> Vec<BatchFinding> {
    let mut findings = Vec::new();
    let listed = m.numbers();
    let manifest = || path.to_string();
    for g in merges.iter().filter(|g| !listed.contains(&g.number)) {
        findings.push(BatchFinding::MemberNotInManifest {
            number: g.number,
            merge_commit: g.merge_commit.clone(),
            manifest: manifest(),
        });
    }
    for n in listed
        .iter()
        .filter(|n| !merges.iter().any(|g| g.number == **n))
    {
        findings.push(BatchFinding::MemberNotMerged {
            number: *n,
            manifest: manifest(),
            line: member_line(text, *n),
        });
    }
    if findings.is_empty() {
        for (i, (l, g)) in listed.iter().zip(merges).enumerate() {
            if *l != g.number {
                findings.push(BatchFinding::MemberOutOfOrder {
                    position: i + 1,
                    listed: *l,
                    merged: g.number,
                    manifest: manifest(),
                    line: member_line(text, *l),
                });
            }
        }
    }
    for c in &m.members {
        let Some(g) = merges.iter().find(|g| g.number == c.number) else {
            continue;
        };
        let line = member_line(text, c.number);
        if c.head != g.head {
            findings.push(BatchFinding::WrongHead {
                number: c.number,
                listed: c.head.clone(),
                merged: g.head.clone(),
                manifest: manifest(),
                line,
            });
        }
        if c.merge_commit != g.merge_commit {
            findings.push(BatchFinding::WrongMergeCommit {
                number: c.number,
                listed: c.merge_commit.clone(),
                merged: g.merge_commit.clone(),
                manifest: manifest(),
                line,
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::super::compose::ComposedMember;
    use super::*;

    fn merge(n: u64) -> MemberMerge {
        MemberMerge {
            number: n,
            head: format!("h{n}"),
            merge_commit: format!("c{n}"),
        }
    }

    fn manifest(members: &[(u64, &str, &str)]) -> (BatchManifest, String) {
        let m = BatchManifest::of(
            "master",
            "aaaaaaaaaaaa",
            "2026-10-08T10:00:00Z",
            members
                .iter()
                .map(|(n, head, commit)| ComposedMember {
                    number: *n,
                    head: head.to_string(),
                    title: format!("change {n}"),
                    merge_commit: commit.to_string(),
                })
                .collect(),
        );
        let text = m.to_yaml();
        (m, text)
    }

    #[test]
    fn a_manifest_that_names_the_merges_in_order_has_no_finding() {
        let (m, text) = manifest(&[(1, "h1", "c1"), (2, "h2", "c2")]);
        assert_eq!(compare(&[merge(1), merge(2)], &m, "b.yaml", &text), []);
    }

    #[test]
    fn a_missing_member_a_wrong_order_and_a_wrong_head_are_each_named_with_the_line() {
        // #3 was merged and is not named
        let (m, text) = manifest(&[(1, "h1", "c1"), (2, "h2", "c2")]);
        let found = compare(&[merge(1), merge(2), merge(3)], &m, "b.yaml", &text);
        assert_eq!(
            found,
            [BatchFinding::MemberNotInManifest {
                number: 3,
                merge_commit: "c3".into(),
                manifest: "b.yaml".into()
            }]
        );
        assert!(found[0].to_string().contains("names no member #3"));
        // #2 is named and was not merged: the manifest's line says where
        let found = compare(&[merge(1)], &m, "b.yaml", &text);
        let BatchFinding::MemberNotMerged { number, line, .. } = &found[0] else {
            panic!("{found:?}");
        };
        assert_eq!(*number, 2);
        assert_eq!(text.lines().nth(*line - 1), Some("  - number: 2"));
        // the same members, the other way round
        let (m, text) = manifest(&[(2, "h2", "c2"), (1, "h1", "c1")]);
        let found = compare(&[merge(1), merge(2)], &m, "b.yaml", &text);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(matches!(
            found[0],
            BatchFinding::MemberOutOfOrder {
                position: 1,
                listed: 2,
                merged: 1,
                ..
            }
        ));
        // a head and a merge commit that are not the merge's
        let (m, text) = manifest(&[(1, "h1", "c1"), (2, "other", "elsewhere")]);
        let found = compare(&[merge(1), merge(2)], &m, "b.yaml", &text);
        assert!(matches!(
            &found[0],
            BatchFinding::WrongHead { number: 2, listed, merged, .. } if listed == "other" && merged == "h2"
        ));
        assert!(matches!(
            &found[1],
            BatchFinding::WrongMergeCommit { number: 2, .. }
        ));
        assert!(found[0].to_string().starts_with("b.yaml:"), "{}", found[0]);
    }

    #[test]
    fn the_summary_says_which_of_the_three_it_is() {
        let mut r = BatchCheck {
            base: "origin/master".into(),
            merge_base: "m".into(),
            head: "hhhhhhhhhhhhhh".into(),
            looked_through: None,
            forge_read: false,
            members: Vec::new(),
            manifest: None,
            verdict: BatchVerdict::NotABatch,
            findings: Vec::new(),
        };
        assert!(r.summary().contains("not a batch") && r.summary().contains("git alone"));
        r.forge_read = true;
        r.members = vec![merge(1), merge(2)];
        r.manifest = Some("b.yaml".into());
        r.verdict = BatchVerdict::Batch;
        assert!(r
            .summary()
            .contains("ok: hhhhhhhhhh from origin/master is the batch b.yaml"));
        r.verdict = BatchVerdict::Refused;
        r.findings = vec![BatchFinding::ManifestMissing {
            members: r.members.clone(),
        }];
        assert!(r.summary().contains("REFUSED") && r.summary().contains("#1, #2"));
        // judged for its manifest alone: the summary says that is why
        r.members = vec![merge(1)];
        assert!(
            r.summary()
                .contains("adds or changes b.yaml, merges #1 and is not the batch"),
            "{}",
            r.summary()
        );
        r.members.clear();
        assert!(r.summary().contains("merges no other open pull request"));
        let few = BatchFinding::TooFewMembers {
            found: 1,
            manifest: "b.yaml".into(),
        };
        assert!(few.to_string().contains("a batch of 1 member(s)"), "{few}");
        assert!(r.findings[0].to_string().contains("prs compose --apply"));
    }
}
