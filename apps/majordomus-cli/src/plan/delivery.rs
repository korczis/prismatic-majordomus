//! How far each issue's work has travelled: inferred from git, the forge cache and the
//! published release pointer, and never written down (ADR 0166, decision 1).
//!
//! An issue's declared status is a stamp (`plan start|verify|done`); its delivery is a fact
//! about the objects that realised it. This module reads those objects and answers, for every
//! issue, the furthest of six stages, each with the object that proved it:
//!
//! | stage      | proved by                                                              |
//! |------------|------------------------------------------------------------------------|
//! | `none`     | nothing that was read names the issue                                  |
//! | `branch`   | a local or remote-tracking branch names it                             |
//! | `pr_open`  | an open pull request in the recorded forge observation names it        |
//! | `merged`   | a merge (title or branch) or a commit (subject) in the trunk's history, |
//! |            | or a merged pull request's forge title whose merge or head is in it     |
//! | `released` | the first release record whose `commit` contains that merge            |
//! | `deployed` | the published `latest.json` names that release or a later one          |
//!
//! # What names an issue
//!
//! The edges are the ones the repository already reads, not new ones:
//!
//! - a branch names an issue by the I1405 rule ([`issue_of`]: a path component equal to the
//!   id, or the id followed by `-`), or by a path component equal to the issue's declared
//!   `slug` — the name `majordomus worktree create` work is given in this repository;
//! - a pull request (open, or merged into the trunk) names an issue by its title or its head
//!   branch ([`issues_named`], the rule the PR Drain screen links issues by), or by the slug;
//! - a commit names an issue by its subject, the way this repository's commits are written
//!   (`type(scope): description (I2740)`), and never by its body, which mentions the issues
//!   around the work as readily as the one it realises; a pull request merged inside a
//!   combined top reaches the trunk through an octopus merge that names it only in prose,
//!   so its commits' subjects are the edge that survives;
//! - a merge commit carries its pull request's number and head branch in its subject
//!   (`Merge pull request #N from owner/branch`) and the title as the first line of its body,
//!   and a local merge (`Merge branch 'x'`, `Merge remote-tracking branch 'origin/x'`) its
//!   branch.
//!
//! When several merges name one issue, the newest one is its proof: the issue has travelled
//! only as far as its last merged change, so a follow-up merged after the last release reads
//! `merged`, not `released`.
//!
//! # Two ways to observe
//!
//! - **Live** (no `at`): the trunk as this clone knows it, every branch, the forge
//!   observation `majordomus prs refresh` recorded, the release records, and the
//!   `latest.json` of the commit the last successful Pages deployment published (the
//!   deployment observation `majordomus served forge` recorded).
//! - **At a commit** (`at`): the merges in that commit's history and the tree's release
//!   records, so only `merged` and `released` can be proved. Nothing live is read — no
//!   branch, no forge, no deployment, no clock — and every live source is reported unread, so
//!   the answer is a function of the commit alone. That is what lets
//!   `scripts/generate-site-data --check` reproduce a committed answer byte for byte on any
//!   day, whatever was tagged, pushed, opened or deployed since.
//!
//! Releases are read from the records in both modes, never from tags: a record names the
//! commit it released and is a file of the tree, a tag is a ref that can appear on any day.
//!
//! # Unread is not none
//!
//! A source that was not read is reported by name with the reason, and an issue whose stage
//! that source could have raised carries it in `unread` and is not `settled`. A checkout that
//! never observed the forge cannot say that no pull request is open; it says it did not look.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::deploy::observed::ObservedMergedPullRequest;
use crate::distribution::release::{Release, Releases, LATEST, PUBLIC_DIR};
use crate::integration::ForgeObservation;
use crate::plan::{Plan, PlanIssue};
use crate::ui::prs::issues_named;
use crate::worktree::state::issue_of;
use crate::worktree::Trunk;

/// The version of this answer's shape.
pub(crate) const DELIVERY_SCHEMA: u32 = 1;

/// The environment the published site is deployed to, in the forge's words.
pub(crate) const PAGES_ENVIRONMENT: &str = "github-pages";

/// The separator between the fields of one commit in a `git log` this module reads.
const FIELD: char = '\u{0}';
/// The separator between commits: a commit body may hold any line, never this byte.
const RECORD: char = '\u{1e}';

// ---------------------------------------------------------------- the answer

/// How far one issue's work has travelled, in order: each stage implies every one before it
/// has been passed through, so the furthest one proved is the issue's stage.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeliveryStage {
    /// Nothing that was read names the issue.
    None,
    /// A branch names it.
    Branch,
    /// An open pull request names it.
    PrOpen,
    /// A merge in the trunk's history names it.
    Merged,
    /// A `v*` release contains its newest merge.
    Released,
    /// The published `latest.json` names that release or a later one.
    Deployed,
}

impl DeliveryStage {
    /// Every stage, in order: the vocabulary that travels with every answer.
    pub(crate) const ALL: [DeliveryStage; 6] = [
        DeliveryStage::None,
        DeliveryStage::Branch,
        DeliveryStage::PrOpen,
        DeliveryStage::Merged,
        DeliveryStage::Released,
        DeliveryStage::Deployed,
    ];

    /// The word the answer spells it with.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            DeliveryStage::None => "none",
            DeliveryStage::Branch => "branch",
            DeliveryStage::PrOpen => "pr_open",
            DeliveryStage::Merged => "merged",
            DeliveryStage::Released => "released",
            DeliveryStage::Deployed => "deployed",
        }
    }
}

/// Whether one source of the answer was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeliveryReading {
    /// It was read; what it holds is in the answer.
    Read,
    /// It was not read, and `reason` says why. Nothing it could have proved is claimed or
    /// denied.
    Unread,
}

/// One source the answer is built from, and whether it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DeliverySource {
    /// `branches`, `forge`, `history`, `releases` or `publication`.
    pub name: String,
    /// Read or unread.
    pub state: DeliveryReading,
    /// What was read, or why nothing was: the ref, the file, the command that would record
    /// it.
    pub detail: String,
    /// When what was read was observed, RFC 3339, when it was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
}

/// A branch that names the issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DeliveryBranch {
    /// The ref, short: `feature/x`, or `origin/feature/x` for a remote-tracking one.
    pub name: String,
    /// The commit it points at.
    pub head: String,
    /// True for a remote-tracking ref.
    pub remote: bool,
    /// How the name names the issue: `id` or `slug`.
    pub via: String,
    /// When the refs were read.
    pub observed_at: String,
}

/// An open pull request that names the issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DeliveryPullRequest {
    /// The number.
    pub number: u64,
    /// The title.
    pub title: String,
    /// The branch it asks to merge.
    pub head_ref: String,
    /// True for a draft.
    pub draft: bool,
    /// How it names the issue: `title_or_branch` or `slug`.
    pub via: String,
    /// When the forge was observed.
    pub observed_at: String,
}

/// The newest commit in the trunk's history that names the issue: a merge, by its pull
/// request's title or its branch, or a commit whose subject names it the way this
/// repository's commits do (`type(scope): description (I2740)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DeliveryMerge {
    /// The commit: a merge commit, or a commit whose subject names the issue.
    pub commit: String,
    /// The pull request it merged, when its subject names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<u64>,
    /// The branch it merged, when its subject names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The pull request's title, when the merge carries one, or the commit's subject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// How it names the issue: `title_or_branch` or `slug` for a merge, `subject` for a
    /// commit, `forge_title` for a merged pull request's title as the forge recorded it.
    pub via: String,
    /// When it was committed, RFC 3339.
    pub merged_at: String,
    /// How many commits in the history name the issue; this one is the newest.
    pub merges: usize,
    /// When the history was read.
    pub observed_at: String,
}

/// The first release that contains the issue's newest merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DeliveryRelease {
    /// The tag: `v0.27.0`.
    pub tag: String,
    /// The commit the tag names.
    pub commit: String,
    /// When the releases were read.
    pub observed_at: String,
}

/// The published release pointer that covers the issue's release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DeliveryDeployment {
    /// The version `latest.json` names.
    pub version: String,
    /// Where that `latest.json` was read: `<commit>:site/static/releases/latest.json`, the
    /// commit the observed Pages deployment published.
    pub latest_json: String,
    /// When the publication was observed.
    pub observed_at: String,
}

/// One issue's delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct IssueDelivery {
    /// The issue id.
    pub issue: String,
    /// Its title.
    pub title: String,
    /// Its milestone.
    pub milestone: String,
    /// Its declared status, as the plan derives it from the stamps: the other half of the
    /// comparison this answer exists for.
    pub status: String,
    /// The furthest stage proved.
    pub stage: DeliveryStage,
    /// True when no unread source could have proved a further stage.
    pub settled: bool,
    /// The sources that were not read and could have proved a further stage.
    pub unread: Vec<String>,
    /// The branch that names it, local first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<DeliveryBranch>,
    /// The open pull request that names it, newest first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_open: Option<DeliveryPullRequest>,
    /// The newest merge that names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged: Option<DeliveryMerge>,
    /// The first release containing that merge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released: Option<DeliveryRelease>,
    /// The publication that covers that release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed: Option<DeliveryDeployment>,
}

/// Where and when the answer was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct DeliveryObservation {
    /// `live`, or `at` a named commit.
    pub mode: String,
    /// The history read: the trunk ref in live mode, the commit asked for otherwise.
    pub trunk: String,
    /// The commit that history ends at.
    pub commit: String,
    /// When: the clock in live mode, the commit's committer time at a commit.
    pub observed_at: String,
}

/// Every issue's delivery, with what it was read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct IssueDeliveryReport {
    /// The version of this shape.
    pub schema: u32,
    /// Where and when.
    pub observed: DeliveryObservation,
    /// The stage vocabulary, in order.
    pub stages: Vec<DeliveryStage>,
    /// Every source, read or not.
    pub sources: Vec<DeliverySource>,
    /// How many issues are at each stage, keyed by every stage of the vocabulary.
    pub tallies: BTreeMap<String, usize>,
    /// How many issues an unread source could have moved further.
    pub unsettled: usize,
    /// The issues, in id order.
    pub issues: Vec<IssueDelivery>,
}

// ---------------------------------------------------------------- the pure half

/// One merge commit, as its subject and first body line state it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MergeCommit {
    /// The commit.
    pub commit: String,
    /// Committer time, Unix seconds.
    pub time: u64,
    /// The pull request number, when the subject names one.
    pub pull_request: Option<u64>,
    /// The merged branch, without its owner or remote, when the subject names one.
    pub branch: Option<String>,
    /// The pull request's title, for a pull-request merge.
    pub title: Option<String>,
}

/// Every commit of the output of `git log --no-merges --format=%H%x00%ct%x00%s%x1e`, its
/// subject carried as its title: the commits whose subjects name an issue.
pub(crate) fn parse_subjects(text: &str) -> Vec<MergeCommit> {
    text.split(RECORD)
        .filter_map(|record| {
            let mut f = record.trim_start_matches(['\n', '\r']).splitn(3, FIELD);
            let commit = f.next()?.trim().to_string();
            let time = f.next()?.trim().parse::<u64>().ok()?;
            let subject = f.next()?.trim().to_string();
            (!commit.is_empty()).then_some(MergeCommit {
                commit,
                time,
                pull_request: None,
                branch: None,
                title: Some(subject),
            })
        })
        .collect()
}

/// Parse the output of `git log --merges --format=%H%x00%ct%x00%s%x00%b%x1e`.
pub(crate) fn parse_merges(text: &str) -> Vec<MergeCommit> {
    text.split(RECORD)
        .filter_map(|record| {
            let record = record.trim_start_matches(['\n', '\r']);
            let mut f = record.splitn(4, FIELD);
            let commit = f.next()?.trim().to_string();
            let time = f.next()?.trim().parse::<u64>().ok()?;
            let subject = f.next()?.to_string();
            let body = f.next().unwrap_or_default();
            if commit.is_empty() {
                return None;
            }
            let (pull_request, branch, is_pull) = merge_subject(&subject);
            let title = if is_pull {
                body.lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .map(str::to_string)
            } else {
                None
            };
            Some(MergeCommit {
                commit,
                time,
                pull_request,
                branch,
                title,
            })
        })
        .collect()
}

/// What a merge subject says: the pull request, the branch, and whether it is the forge's
/// pull-request merge (whose body's first line is the title).
fn merge_subject(subject: &str) -> (Option<u64>, Option<String>, bool) {
    if let Some(rest) = subject.strip_prefix("Merge pull request #") {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let number = digits.parse::<u64>().ok();
        let branch = rest[digits.len()..]
            .trim_start()
            .strip_prefix("from ")
            .map(str::trim)
            // `owner/branch`: the owner is not part of the branch's name
            .and_then(|b| b.split_once('/').map(|(_, name)| name.to_string()));
        return (number, branch, true);
    }
    for prefix in ["Merge remote-tracking branch '", "Merge branch '"] {
        if let Some(rest) = subject.strip_prefix(prefix) {
            let Some((name, _)) = rest.split_once('\'') else {
                return (None, None, false);
            };
            let name = if prefix.contains("remote-tracking") {
                // `origin/x`: the remote is not part of the branch's name
                name.split_once('/').map_or(name, |(_, b)| b)
            } else {
                name
            };
            return (None, Some(name.to_string()), false);
        }
    }
    (None, None, false)
}

/// The issues a name, a title or both name, among `declared`, each with how: the I1405
/// rule over the title and the branch, then a branch component equal to a declared slug.
fn named_by(
    title: Option<&str>,
    branch: Option<&str>,
    declared: &BTreeSet<String>,
    slugs: &BTreeMap<String, String>,
) -> Vec<(String, &'static str)> {
    let mut found: Vec<(String, &'static str)> = issues_named(&json!({
        "title": title.unwrap_or_default(),
        "head_ref": branch.unwrap_or_default(),
    }))
    .into_iter()
    .filter(|id| declared.contains(id))
    .map(|id| (id, "title_or_branch"))
    .collect();
    for component in branch.unwrap_or_default().split('/') {
        if let Some(id) = slugs.get(component) {
            if !found.iter().any(|(f, _)| f == id) {
                found.push((id.clone(), "slug"));
            }
        }
    }
    found
}

/// A branch's issue: the I1405 rule, or a component equal to a declared slug.
fn branch_names(
    name: &str,
    ids: &[String],
    slugs: &BTreeMap<String, String>,
) -> Option<(String, &'static str)> {
    if let Some(id) = issue_of(name, ids) {
        return Some((id, "id"));
    }
    name.split('/')
        .find_map(|c| slugs.get(c))
        .map(|id| (id.clone(), "slug"))
}

/// A `v*` tag's version as a sortable key: the numbers, then a release above any of its
/// pre-releases. `None` for a tag that is not a version.
fn version_key(tag: &str) -> Option<(Vec<u64>, bool, String)> {
    let v = tag.strip_prefix('v').unwrap_or(tag);
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (v, None),
    };
    let numbers: Option<Vec<u64>> = core.split('.').map(|n| n.parse().ok()).collect();
    let numbers = numbers.filter(|n| !n.is_empty())?;
    Some((numbers, pre.is_none(), pre.unwrap_or_default().to_string()))
}

/// The issues' stages from what was read. Pure: every input is already in memory.
pub(crate) struct Evidence {
    /// When the git half was observed.
    pub observed_at: String,
    /// Branches by issue, local first.
    pub branches: BTreeMap<String, DeliveryBranch>,
    /// The open pull requests by issue, newest first; `None` when the forge was not read.
    pub open: Option<BTreeMap<String, DeliveryPullRequest>>,
    /// Whether the branches were read.
    pub branches_read: bool,
    /// The merges naming each issue, newest first.
    pub merges: BTreeMap<String, Vec<(MergeCommit, &'static str)>>,
    /// The first release containing a commit, by commit.
    pub first_release: BTreeMap<String, (String, String)>,
    /// The published version and where it was read; `None` when the publication was not read.
    pub published: Option<(String, String, String)>,
}

/// Assemble one issue's delivery from the evidence.
pub(crate) fn assess(issue: &PlanIssue, e: &Evidence) -> IssueDelivery {
    let branch = e.branches.get(&issue.id).cloned();
    let pr_open = e.open.as_ref().and_then(|o| o.get(&issue.id).cloned());
    let merges = e.merges.get(&issue.id);
    let merged = merges
        .and_then(|m| m.first())
        .map(|(m, via)| DeliveryMerge {
            commit: m.commit.clone(),
            pull_request: m.pull_request,
            branch: m.branch.clone(),
            title: m.title.clone(),
            via: (*via).to_string(),
            merged_at: crate::peers::rfc3339(UNIX_EPOCH + Duration::from_secs(m.time)),
            merges: merges.map_or(0, Vec::len),
            observed_at: e.observed_at.clone(),
        });
    let released = merged.as_ref().and_then(|m| {
        e.first_release
            .get(&m.commit)
            .map(|(tag, commit)| DeliveryRelease {
                tag: tag.clone(),
                commit: commit.clone(),
                observed_at: e.observed_at.clone(),
            })
    });
    let deployed = match (&released, &e.published) {
        (Some(r), Some((version, latest_json, observed_at)))
            if version_key(&r.tag) <= version_key(version) =>
        {
            Some(DeliveryDeployment {
                version: version.clone(),
                latest_json: latest_json.clone(),
                observed_at: observed_at.clone(),
            })
        }
        _ => None,
    };
    let stage = if deployed.is_some() {
        DeliveryStage::Deployed
    } else if released.is_some() {
        DeliveryStage::Released
    } else if merged.is_some() {
        DeliveryStage::Merged
    } else if pr_open.is_some() {
        DeliveryStage::PrOpen
    } else if branch.is_some() {
        DeliveryStage::Branch
    } else {
        DeliveryStage::None
    };
    let mut unread = Vec::new();
    if !e.branches_read && stage < DeliveryStage::Branch {
        unread.push("branches".to_string());
    }
    if e.open.is_none() && stage < DeliveryStage::PrOpen {
        unread.push("forge".to_string());
    }
    if e.published.is_none() && stage == DeliveryStage::Released {
        unread.push("publication".to_string());
    }
    IssueDelivery {
        issue: issue.id.clone(),
        title: issue.title.clone(),
        milestone: issue.milestone.clone(),
        status: issue.status.clone(),
        stage,
        settled: unread.is_empty(),
        unread,
        branch,
        pr_open,
        merged,
        released,
        deployed,
    }
}

// ---------------------------------------------------------------- reading git and the records

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = crate::worktree::git::run(root, args).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_ok(root: &Path, args: &[&str]) -> Option<String> {
    let out = crate::worktree::git::try_run(root, args).ok()?;
    (out.status == Some(0)).then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The ref the live answer reads the trunk from: the remote-tracking trunk unless the local
/// one is ahead of it, because a primary checkout's local trunk can lag the forge by hundreds
/// of merges and every merge it lacks would read as unmerged.
fn trunk_ref(root: &Path, trunk: &str) -> Option<String> {
    let remote = format!("origin/{trunk}");
    let local_ok = git_ok(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{trunk}^{{commit}}"),
        ],
    );
    let remote_ok = git_ok(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{remote}^{{commit}}"),
        ],
    );
    match (local_ok, remote_ok) {
        (Some(l), Some(r)) if l.trim() != r.trim() => {
            if crate::git::is_ancestor(root, &remote, trunk) == Some(true) {
                Some(trunk.to_string())
            } else {
                Some(remote)
            }
        }
        (_, Some(_)) => Some(remote),
        (Some(_), None) => Some(trunk.to_string()),
        (None, None) => None,
    }
}

/// Every commit in `rev`'s history that is not a merge, with its subject.
fn subjects_of(root: &Path, rev: &str) -> Result<Vec<MergeCommit>, String> {
    let text = git(
        root,
        &[
            "log",
            "--no-merges",
            "--format=%H%x00%ct%x00%s%x1e",
            rev,
            "--",
        ],
    )?;
    Ok(parse_subjects(&text))
}

/// Every merge in `rev`'s history.
fn merges_of(root: &Path, rev: &str) -> Result<Vec<MergeCommit>, String> {
    let text = git(
        root,
        &[
            "log",
            "--merges",
            "--format=%H%x00%ct%x00%s%x00%b%x1e",
            rev,
            "--",
        ],
    )?;
    Ok(parse_merges(&text))
}

/// For each commit reachable from the releases, the first release (in version order) that
/// contains it: `rev-list <release> --not <every earlier release>` is exactly the set of
/// commits whose first containing release that is — `git tag --contains`, asked once per
/// release rather than once per commit.
fn first_releases(
    root: &Path,
    releases: &[(String, String)],
) -> Result<BTreeMap<String, (String, String)>, String> {
    let mut first: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut earlier: Vec<String> = Vec::new();
    for (tag, commit) in releases {
        let mut args: Vec<String> = vec!["rev-list".into(), commit.clone()];
        if !earlier.is_empty() {
            args.push("--not".into());
            args.extend(earlier.iter().cloned());
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        // a release whose commit this clone does not hold contains nothing it can show
        if let Some(text) = git_ok(root, &argv) {
            for c in text.lines().filter(|l| !l.is_empty()) {
                first
                    .entry(c.to_string())
                    .or_insert_with(|| (tag.clone(), commit.clone()));
            }
            earlier.push(commit.clone());
        }
    }
    Ok(first)
}

/// Releases in version order, lowest first, keeping only `v*` versions.
fn in_version_order(mut releases: Vec<(String, String)>) -> Vec<(String, String)> {
    releases.retain(|(tag, _)| tag.starts_with('v') && version_key(tag).is_some());
    releases.sort_by_key(|r| version_key(&r.0));
    releases
}

/// The version a `latest.json` names.
fn latest_version(text: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    v.get("version")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "it names no version".to_string())
}

fn latest_path() -> String {
    format!("{PUBLIC_DIR}/{LATEST}.json")
}

/// The published version in live mode: the `latest.json` of the commit the newest
/// successful Pages deployment published.
fn published_live(root: &Path) -> Result<(String, String, String), String> {
    let observation = crate::deploy::observed::load(root)?.ok_or_else(|| {
        "no deployment of this repository has been observed from this checkout; \
         `majordomus served forge` records one"
            .to_string()
    })?;
    let deployed = observation
        .deployments
        .iter()
        .filter(|d| d.environment == PAGES_ENVIRONMENT && d.state == "success")
        .max_by(|a, b| a.created_at.cmp(&b.created_at))
        .ok_or_else(|| {
            format!(
                "the deployment observation of {} holds no successful `{PAGES_ENVIRONMENT}` \
                 deployment",
                observation.observed_at
            )
        })?;
    let at = format!("{}:{}", deployed.sha, latest_path());
    let text = git_ok(root, &["show", &at]).ok_or_else(|| {
        format!("{at} cannot be read in this clone (the deployed commit is not fetched, or it publishes no release pointer)")
    })?;
    let version = latest_version(&text).map_err(|e| format!("{at}: {e}"))?;
    Ok((version, at, observation.observed_at))
}

/// The merged pull requests the deployment observation holds, and when it was made.
fn merged_titles(root: &Path) -> Result<(Vec<ObservedMergedPullRequest>, String), String> {
    let observation = crate::deploy::observed::load(root)?.ok_or_else(|| {
        "the forge's merged pull requests have not been observed from this checkout; \
         `majordomus served forge` records them"
            .to_string()
    })?;
    let merged = observation.merged_pull_requests.ok_or_else(|| {
        format!(
            "the deployment observation of {} holds no merged pull requests; \
             `majordomus served forge` records them",
            observation.observed_at
        )
    })?;
    Ok((merged, observation.observed_at))
}

/// Every branch that names an issue, local first: issue → its first naming ref.
fn branches(
    root: &Path,
    ids: &[String],
    slugs: &BTreeMap<String, String>,
    trunk: &str,
    observed_at: &str,
) -> Result<BTreeMap<String, DeliveryBranch>, String> {
    let text = git(
        root,
        &[
            "for-each-ref",
            "refs/heads",
            "refs/remotes",
            "--format=%(refname:short)%00%(objectname)%00%(refname)",
        ],
    )?;
    let mut found: BTreeMap<String, DeliveryBranch> = BTreeMap::new();
    let mut refs: Vec<(bool, String, String)> = text
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(FIELD).collect();
            let (name, head, full) = (*f.first()?, *f.get(1)?, *f.get(2)?);
            let remote = full.starts_with("refs/remotes/");
            let bare = if remote {
                name.split_once('/').map_or(name, |(_, b)| b)
            } else {
                name
            };
            (!name.is_empty() && !name.ends_with("/HEAD") && bare != trunk && name != "HEAD")
                .then(|| (remote, name.to_string(), head.to_string()))
        })
        .collect();
    refs.sort();
    for (remote, name, head) in refs {
        if let Some((id, via)) = branch_names(&name, ids, slugs) {
            found.entry(id).or_insert_with(|| DeliveryBranch {
                name,
                head,
                remote,
                via: via.to_string(),
                observed_at: observed_at.to_string(),
            });
        }
    }
    Ok(found)
}

/// The open pull requests naming each issue, newest first.
fn open_pulls(
    observation: &ForgeObservation,
    declared: &BTreeSet<String>,
    slugs: &BTreeMap<String, String>,
) -> BTreeMap<String, DeliveryPullRequest> {
    let mut pulls: Vec<_> = observation.pull_requests.iter().collect();
    pulls.sort_by_key(|p| std::cmp::Reverse(p.number));
    let mut found: BTreeMap<String, DeliveryPullRequest> = BTreeMap::new();
    for pr in pulls {
        for (id, via) in named_by(Some(&pr.title), Some(&pr.head_ref), declared, slugs) {
            found.entry(id).or_insert_with(|| DeliveryPullRequest {
                number: pr.number,
                title: pr.title.clone(),
                head_ref: pr.head_ref.clone(),
                draft: pr.draft,
                via: via.to_string(),
                observed_at: observation.observed_at.clone(),
            });
        }
    }
    found
}

fn source(name: &str, read: Result<String, String>, observed_at: Option<&str>) -> DeliverySource {
    match read {
        Ok(detail) => DeliverySource {
            name: name.into(),
            state: DeliveryReading::Read,
            detail,
            observed_at: observed_at.map(str::to_string),
        },
        Err(detail) => DeliverySource {
            name: name.into(),
            state: DeliveryReading::Unread,
            detail,
            observed_at: None,
        },
    }
}

/// Infer the delivery of every issue of `plan` (or of `only`), observed live or `at` a
/// commit, in the repository at `root`.
pub(crate) fn infer(
    root: &Path,
    plan: &Plan,
    trunk: &Trunk,
    at: Option<&str>,
    only: Option<&str>,
) -> Result<IssueDeliveryReport, String> {
    let ids: Vec<String> = plan.issues.iter().map(|i| i.id.clone()).collect();
    let declared: BTreeSet<String> = ids.iter().cloned().collect();
    let slugs: BTreeMap<String, String> = plan
        .issues
        .iter()
        .filter(|i| !i.slug.is_empty())
        .map(|i| (i.slug.clone(), i.id.clone()))
        .collect();
    let trunk_name = trunk.branch.clone().unwrap_or_else(|| "master".into());

    // where the history ends, and when that was
    let (mode, history, observed_at) = match at {
        Some(rev) => {
            let commit = git_ok(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "--end-of-options",
                    &format!("{rev}^{{commit}}"),
                ],
            )
            .map(|s| s.trim().to_string())
            .ok_or_else(|| format!("`{rev}` does not resolve to a commit in this repository"))?;
            let time = git(root, &["log", "-1", "--format=%ct", &commit])?
                .trim()
                .parse::<u64>()
                .map_err(|e| format!("the committer time of {commit}: {e}"))?;
            (
                "at",
                commit,
                crate::peers::rfc3339(UNIX_EPOCH + Duration::from_secs(time)),
            )
        }
        None => {
            let r = trunk_ref(root, &trunk_name).ok_or_else(|| {
                format!("the trunk `{trunk_name}` does not resolve in this clone, so there is no history to read merges from")
            })?;
            ("live", r, crate::peers::rfc3339(SystemTime::now()))
        }
    };
    let commit = git(root, &["rev-parse", &format!("{history}^{{commit}}")])?
        .trim()
        .to_string();

    // the merges, newest first, attributed
    let all_merges = merges_of(root, &commit)?;
    let mut merges: BTreeMap<String, Vec<(MergeCommit, &'static str)>> = BTreeMap::new();
    for m in &all_merges {
        for (id, via) in named_by(m.title.as_deref(), m.branch.as_deref(), &declared, &slugs) {
            merges.entry(id).or_default().push((m.clone(), via));
        }
    }
    // A pull request merged inside a combined top reaches the trunk through an octopus merge
    // that names it only in prose, and its title is the forge's, not git's; its own commits
    // carry the issue in their subjects. A subject is read, never a body: a body mentions
    // the issues around the work as readily as the one it realises.
    let all_subjects = subjects_of(root, &commit)?;
    for c in &all_subjects {
        for (id, _) in named_by(c.title.as_deref(), None, &declared, &BTreeMap::new()) {
            merges.entry(id).or_default().push((c.clone(), "subject"));
        }
    }
    // The forge's titles of merged pull requests, live only: a pull request merged inside a
    // combined top has its title nowhere in git, and its commits' subjects may not name the
    // issue (#1097 realised I2653 and named it only in a body). A title counts when the
    // pull request's merge commit or head is in the history read, at that commit's time.
    // They only add to what the history proves, so an unread list leaves the history's own
    // edges deciding, and the source says it was not read.
    let titles = if mode == "live" {
        merged_titles(root)
    } else {
        Err(
            "the titles of merged pull requests are the forge's, and an answer at a commit \
             reads that commit's history only; the live answer (no `at`) reads them"
                .to_string(),
        )
    };
    if let Ok((list, _)) = &titles {
        let times: BTreeMap<&str, u64> = all_merges
            .iter()
            .chain(&all_subjects)
            .map(|c| (c.commit.as_str(), c.time))
            .collect();
        for pr in list {
            let landed = [pr.merge_commit.as_str(), pr.head_sha.as_str()]
                .into_iter()
                .find_map(|c| times.get(c).map(|t| (c, *t)));
            let Some((commit, time)) = landed else {
                continue;
            };
            for (id, _) in named_by(Some(&pr.title), Some(&pr.head_ref), &declared, &slugs) {
                let named = merges.entry(id).or_default();
                if named.iter().any(|(m, _)| m.commit == commit) {
                    continue;
                }
                named.push((
                    MergeCommit {
                        commit: commit.to_string(),
                        time,
                        pull_request: Some(pr.number),
                        branch: Some(pr.head_ref.clone()),
                        title: Some(pr.title.clone()),
                    },
                    "forge_title",
                ));
            }
        }
    }
    for named in merges.values_mut() {
        // newest first; a merge before the commits it brought in at the same second
        named.sort_by_key(|n| std::cmp::Reverse(n.0.time));
    }

    let mut sources = vec![
        source(
            "history",
            Ok(format!(
                "{} merge commit(s) and {} other commit(s) in the history of {history}",
                all_merges.len(),
                all_subjects.len()
            )),
            Some(&observed_at),
        ),
        source(
            "merged_titles",
            titles
                .as_ref()
                .map(|(l, _)| format!("{} merged pull request title(s) of the forge", l.len()))
                .map_err(Clone::clone),
            titles.as_ref().ok().map(|(_, o)| o.as_str()),
        ),
    ];

    // releases: the tree's release records, in both modes. A record names the commit it
    // released, and it is a file of the tree, so the answer at a commit is a function of the
    // commit; a tag is a ref that can appear on any day and would move it.
    let releases = in_version_order(
        Releases::load(root)
            .map_err(|e| e.to_string())?
            .releases
            .into_iter()
            .map(|r: Release| (r.tag, r.commit))
            .collect(),
    );
    let first_release = first_releases(root, &releases)?;
    sources.push(source(
        "releases",
        Ok(format!(
            "{} release record(s) under {}",
            releases.len(),
            crate::distribution::release::DIR
        )),
        Some(&observed_at),
    ));

    // the publication: what the site serves is live state, read only by the live answer
    let published = if mode == "live" {
        published_live(root)
    } else {
        Err(
            "what the site publishes is live state, and an answer at a commit reads that \
             commit's history and this tree's release records only; the live answer (no `at`) \
             reads the deployment observation"
                .to_string(),
        )
    };
    sources.push(source(
        "publication",
        published
            .as_ref()
            .map(|(v, at, _)| format!("{at} names {v}"))
            .map_err(Clone::clone),
        published.as_ref().ok().map(|(_, _, o)| o.as_str()),
    ));

    // the branches and the forge: live only
    let (branch_map, branches_read, open) = if mode == "live" {
        let b = branches(root, &ids, &slugs, &trunk_name, &observed_at)?;
        sources.push(source(
            "branches",
            Ok(format!(
                "{} issue(s) named by a local or remote-tracking branch",
                b.len()
            )),
            Some(&observed_at),
        ));
        let open = match crate::integration::load_observation(root) {
            Ok(Some(o)) => {
                sources.push(source(
                    "forge",
                    Ok(format!(
                        "{} open pull request(s) of {}",
                        o.pull_requests.len(),
                        o.repository
                    )),
                    Some(&o.observed_at),
                ));
                Some(open_pulls(&o, &declared, &slugs))
            }
            Ok(None) => {
                sources.push(source(
                    "forge",
                    Err("the forge has never been observed from this checkout; `majordomus prs refresh` records it".into()),
                    None,
                ));
                None
            }
            Err(e) => {
                sources.push(source("forge", Err(e), None));
                None
            }
        };
        (b, true, open)
    } else {
        let why = "an answer at a commit reads that commit's history and this tree only; \
                   the live answer (no `at`) reads it"
            .to_string();
        sources.push(source("branches", Err(why.clone()), None));
        sources.push(source("forge", Err(why), None));
        (BTreeMap::new(), false, None)
    };

    let evidence = Evidence {
        observed_at: observed_at.clone(),
        branches: branch_map,
        open,
        branches_read,
        merges,
        first_release,
        published: published.ok(),
    };
    let issues: Vec<IssueDelivery> = plan
        .issues
        .iter()
        .filter(|i| only.is_none_or(|id| i.id == id))
        .map(|i| assess(i, &evidence))
        .collect();
    let mut tallies: BTreeMap<String, usize> = DeliveryStage::ALL
        .iter()
        .map(|s| (s.as_str().to_string(), 0))
        .collect();
    for i in &issues {
        *tallies.entry(i.stage.as_str().to_string()).or_default() += 1;
    }
    Ok(IssueDeliveryReport {
        schema: DELIVERY_SCHEMA,
        observed: DeliveryObservation {
            mode: mode.to_string(),
            trunk: history,
            commit,
            observed_at,
        },
        stages: DeliveryStage::ALL.to_vec(),
        sources,
        tallies,
        unsettled: issues.iter().filter(|i| !i.settled).count(),
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(id: &str, slug: &str) -> PlanIssue {
        PlanIssue {
            id: id.into(),
            milestone: "m".into(),
            status: "READY".into(),
            wave: 1,
            priority: "p1".into(),
            profile: "implementation".into(),
            parallel_safe: true,
            title: format!("{id} title"),
            slug: slug.into(),
            depends_on: vec![],
            blocked_by: vec![],
            dependents: vec![],
            scope: vec![],
            serves: vec![],
            objective: String::new(),
            evidence_have: 0,
            evidence_need: 0,
            started_at: String::new(),
            verified_at: String::new(),
            completed_at: String::new(),
        }
    }

    #[test]
    fn a_merge_subject_names_its_pull_request_and_branch() {
        let text = "aaa\x001700000000\x00Merge pull request #12 from me/feature/I0001-x\x00Do it (I0001)\n\nbody\x1e\n\
                    bbb\x001700000001\x00Merge remote-tracking branch 'origin/fix/a-slug' into HEAD\x00\x1e\n\
                    ccc\x001700000002\x00Merge branch 'feature/b' of github.com:x/y into z\x00\x1e\n\
                    ddd\x001700000003\x00merge: #9 into the top\x00I0002 mentioned\x1e\n\
                    eee\x001700000004\x00Merge branch 'broken\x00\x1e\n\
                    \x001\x00x\x00\x1e";
        let m = parse_merges(text);
        assert_eq!(m.len(), 5);
        assert_eq!(m[0].pull_request, Some(12));
        assert_eq!(m[0].branch.as_deref(), Some("feature/I0001-x"));
        assert_eq!(m[0].title.as_deref(), Some("Do it (I0001)"));
        assert_eq!(m[1].branch.as_deref(), Some("fix/a-slug"));
        assert_eq!(m[1].title, None);
        assert_eq!(m[2].branch.as_deref(), Some("feature/b"));
        assert_eq!(
            (m[3].pull_request, &m[3].branch, &m[3].title),
            (None, &None, &None)
        );
        assert_eq!(m[4].branch, None);
    }

    #[test]
    fn a_commit_subject_is_carried_as_its_title() {
        let c = parse_subjects("aaa\x001700000000\x00feat(x): a thing (I0001)\x1e\nbbb\x00nope\x00x\x1e\n\x001\x00y\x1e");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].title.as_deref(), Some("feat(x): a thing (I0001)"));
        assert_eq!((c[0].pull_request, &c[0].branch), (None, &None));
    }

    #[test]
    fn names_come_from_title_branch_and_slug_and_only_declared_ids() {
        let declared: BTreeSet<String> = ["I0001", "I0002"].map(String::from).into();
        let slugs: BTreeMap<String, String> = [("a-slug".to_string(), "I0002".to_string())].into();
        let n = named_by(
            Some("fix (I0001) and I9999"),
            Some("fix/a-slug"),
            &declared,
            &slugs,
        );
        assert_eq!(
            n,
            vec![
                ("I0001".into(), "title_or_branch"),
                ("I0002".into(), "slug")
            ]
        );
        assert!(named_by(None, None, &declared, &slugs).is_empty());
        let ids: Vec<String> = declared.iter().cloned().collect();
        assert_eq!(
            branch_names("feature/I0001-x", &ids, &slugs),
            Some(("I0001".into(), "id"))
        );
        assert_eq!(
            branch_names("origin/fix/a-slug", &ids, &slugs),
            Some(("I0002".into(), "slug"))
        );
        assert_eq!(branch_names("fix/other", &ids, &slugs), None);
    }

    #[test]
    fn versions_order_numerically_and_a_release_above_its_prerelease() {
        assert!(version_key("v0.10.0") > version_key("v0.9.1"));
        assert!(version_key("v1.0.0") > version_key("v1.0.0-rc.1"));
        assert_eq!(version_key("vnext"), None);
        assert_eq!(version_key("0.27.0"), version_key("v0.27.0"));
        let ordered = in_version_order(vec![
            ("v0.10.0".into(), "c".into()),
            ("v0.9.0".into(), "b".into()),
            ("other".into(), "x".into()),
        ]);
        assert_eq!(
            ordered.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
            ["v0.9.0", "v0.10.0"]
        );
        assert_eq!(
            latest_version("{\"version\":\"0.27.0\"}").unwrap(),
            "0.27.0"
        );
        assert!(latest_version("{}").is_err());
        assert!(latest_version("not json").is_err());
    }

    fn evidence() -> Evidence {
        Evidence {
            observed_at: "2026-10-16T00:00:00Z".into(),
            branches: BTreeMap::new(),
            open: None,
            branches_read: false,
            merges: BTreeMap::new(),
            first_release: BTreeMap::new(),
            published: None,
        }
    }

    #[test]
    fn an_unread_source_leaves_the_stage_unsettled_and_says_which() {
        let d = assess(&issue("I0001", ""), &evidence());
        assert_eq!(d.stage, DeliveryStage::None);
        assert!(!d.settled);
        assert_eq!(d.unread, ["branches", "forge"]);
    }

    #[test]
    fn each_stage_is_proved_by_its_object() {
        let mut e = evidence();
        e.branches_read = true;
        e.open = Some(BTreeMap::new());
        let merge = MergeCommit {
            commit: "m1".into(),
            time: 1_788_000_000,
            pull_request: Some(7),
            branch: Some("feature/x".into()),
            title: Some("x (I0001)".into()),
        };
        e.merges
            .insert("I0001".into(), vec![(merge, "title_or_branch")]);
        let d = assess(&issue("I0001", ""), &e);
        assert_eq!((d.stage, d.settled), (DeliveryStage::Merged, true));
        assert_eq!(d.merged.as_ref().unwrap().pull_request, Some(7));
        assert_eq!(d.merged.as_ref().unwrap().merged_at, "2026-08-29T10:40:00Z");

        e.first_release
            .insert("m1".into(), ("v0.2.0".into(), "t".into()));
        let d = assess(&issue("I0001", ""), &e);
        assert_eq!(d.stage, DeliveryStage::Released);
        assert_eq!(d.unread, ["publication"]);

        e.published = Some(("0.1.0".into(), "latest.json".into(), "now".into()));
        assert_eq!(
            assess(&issue("I0001", ""), &e).stage,
            DeliveryStage::Released
        );
        e.published = Some(("0.2.0".into(), "latest.json".into(), "now".into()));
        let d = assess(&issue("I0001", ""), &e);
        assert_eq!(d.stage, DeliveryStage::Deployed);
        assert_eq!(d.deployed.unwrap().version, "0.2.0");
    }

    #[test]
    fn the_vocabulary_spells_every_stage_as_it_serialises() {
        for s in DeliveryStage::ALL {
            assert_eq!(serde_json::to_value(s).unwrap(), json!(s.as_str()));
        }
    }
}
