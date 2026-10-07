//! The forge adapter: the one place pull-request integration talks to the network.
//!
//! SECURITY.md declares it: `majordomus prs refresh` and `majordomus prs drain` — and
//! nothing else — run the GitHub CLI (`gh`), `git fetch` and `git ls-remote` against this
//! repository's own remote. The HTTP server, the MCP tools and the Cockpit never do: they render the last
//! [`ForgeObservation`] recorded under `.ai/local/state/integration/`, with its age, so a
//! page load can never reach the network. The observation is plain data, so every test
//! builds one by hand and no test needs the forge.
//!
//! A required check the base binds to an app is only that app's check run, and `gh pr list`
//! does not say which app wrote a check run. So when a context is bound, the refresh reads
//! every open pull request's checks once more, from the forge's GraphQL rollup
//! ([`WRITERS_QUERY`]), with the app of each ([`attributed_checks()`], [`attribute()`]). A
//! base that binds nothing asks nothing more.
//!
//! Who may declare that one pull request replaces another is the forge's word too, and
//! `gh pr list` does not carry it. So whenever a pull request is open, the refresh reads, with
//! every open one, its author's association and the pull requests that mention it
//! ([`DECLARATIONS_QUERY`], [`declarations()`], [`declare()`]): a declaration about an open pull
//! request is read from that pull request's own cross-references, never from a bounded search
//! of closed ones. One whose cross-references could not be read whole says so
//! ([`CrossReferenceRead`]), and the classifier holds it and no other. A read the forge would
//! not answer is not a hold: it fails the observation, and none is recorded from a part.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::{
    CheckKind, CheckObservation, CheckRunState, CrossReferenceRead, PullRequestObservation,
    RequiredCheck, ReviewObservation, ReviewPolicy,
};

/// The recorded observation's schema version. 4 since a declaration is read from the
/// cross-references of the pull request it is about, with who made it: a record of 3 carries
/// neither and is refused until `prs refresh`.
pub const OBSERVATION_SCHEMA: u32 = 4;

/// Where the pull-request heads are fetched to: a namespace of this tool's own, so no
/// fetch ever moves a ref a person or another tool owns.
pub const PR_REF_PREFIX: &str = "refs/majordomus/prs/";

/// Everything the forge said, at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForgeObservation {
    /// The format; this reads [`OBSERVATION_SCHEMA`].
    pub schema: u32,
    /// `owner/name`.
    pub repository: String,
    /// The integration base (the default branch).
    pub base: String,
    /// The base's commit as the forge reported it when observed.
    pub base_sha: String,
    /// When, RFC 3339.
    pub observed_at: String,
    /// The checks the base requires, from its branch protection and its rulesets together,
    /// each with the app bound to it; `None` when either could not be read (every
    /// required-check verdict is then unknown, never passed). When one of them is bound to
    /// an app, every pull request's checks were read with the app that wrote each
    /// ([`attributed_checks()`]).
    pub required_checks: Option<Vec<RequiredCheck>>,
    /// What the base requires of reviews, from its protection and rulesets together; `None`
    /// when either could not be read.
    pub review_policy: Option<ReviewPolicy>,
    /// Whether the base requires a branch to be up to date before it merges — the
    /// protection's `required_status_checks.strict`, or a ruleset's
    /// `strict_required_status_checks_policy`. The forge-side half of the guard against a
    /// merge onto a master nobody tested with the change; the executor's half is the parent
    /// check after every merge. `None` when either could not be read.
    #[serde(default)]
    pub up_to_date_required: Option<bool>,
    /// The merge methods the repository allows, in the forge's words (`merge`, `squash`,
    /// `rebase`).
    pub merge_methods: Vec<String>,
    /// Every open pull request.
    pub pull_requests: Vec<PullRequestObservation>,
    /// Pull requests that are no longer open and that a supersession names, by number: a
    /// closed or merged one that mentions an open one and whose body says it supersedes it,
    /// read from that open one's cross-references, and every successor or dependency an open
    /// one's body names that is not open. What decides whether a declared successor landed.
    /// Empty in an observation recorded before it was read.
    #[serde(default)]
    pub resolved: BTreeMap<u64, ResolvedPullRequest>,
    /// Whether the repository deletes a pull request's head branch when it merges (the
    /// forge's `delete_branch_on_merge`). The forge decides deletion, never the executor
    /// (owner decision D4); `None` when unread, and in an observation recorded before it was.
    #[serde(default)]
    pub delete_branch_on_merge: Option<bool>,
}

/// A branch of this repository that a merged pull request came from, still on origin at the
/// head that merged. A branch whose tip moved after its merge carries newer work and is not
/// one: only the exact merged head is left behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MergedBranch {
    /// The branch name on origin.
    pub branch: String,
    /// Its tip, which is the merged head.
    pub tip: String,
    /// The pull request that merged it.
    pub pr: u64,
    /// When it merged, as the forge reported it.
    pub merged_at: String,
}

/// How many merged pull requests are read, newest first. A branch whose pull request merged
/// before the newest this many is not reported; the report says how many were read.
pub const MERGED_LIMIT: usize = 1000;

/// Origin's branches from `git ls-remote --heads` output: name → tip. The module is private,
/// so the examples here are text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::remote_heads_of;
/// let heads = remote_heads_of("a1\trefs/heads/master\nb2\trefs/heads/feature/x\nc3\trefs/tags/v1\n");
/// assert_eq!(heads.get("feature/x").map(String::as_str), Some("b2"));
/// assert_eq!(heads.len(), 2, "a tag is not a branch");
/// ```
pub fn remote_heads_of(ls_remote: &str) -> BTreeMap<String, String> {
    ls_remote
        .lines()
        .filter_map(|l| {
            let (sha, name) = l.split_once('\t')?;
            let name = name.trim().strip_prefix("refs/heads/")?;
            (!sha.is_empty() && !name.is_empty()).then(|| (name.to_string(), sha.to_string()))
        })
        .collect()
}

/// The merged pull requests whose head branch origin still serves at the head that merged,
/// from `gh pr list --state merged --json number,state,headRefName,headRefOid,isCrossRepository,mergedAt`.
///
/// A pull request counts only when the forge says it `MERGED` and when: it came from this
/// repository (a fork's branch is not a branch here, whatever it is called), its branch is not
/// the base, and origin's tip of that branch is exactly its merged head. One branch, one entry:
/// when several merged pull requests left the same branch at the same head, the newest
/// number names it. In branch order.
///
/// ```text
/// use majordomus_cli::integration::forge::{merged_branches_of, remote_heads_of};
/// let heads = remote_heads_of("aa\trefs/heads/fix/a\nbb\trefs/heads/fix/b\ncc\trefs/heads/master\n");
/// let merged = serde_json::json!([
///   {"number": 1, "state": "MERGED", "headRefName": "fix/a", "headRefOid": "aa", "isCrossRepository": false, "mergedAt": "t1"},
///   {"number": 2, "state": "MERGED", "headRefName": "fix/b", "headRefOid": "b0", "isCrossRepository": false, "mergedAt": "t2"},
///   {"number": 3, "state": "OPEN",   "headRefName": "fix/a", "headRefOid": "aa", "isCrossRepository": false}
/// ]);
/// let left = merged_branches_of(&merged, &heads, "master");
/// assert_eq!(left.len(), 1, "fix/b moved after its merge, and an open one is not merged");
/// assert_eq!((left[0].branch.as_str(), left[0].pr), ("fix/a", 1));
/// ```
pub fn merged_branches_of(
    merged: &Value,
    heads: &BTreeMap<String, String>,
    base: &str,
) -> Vec<MergedBranch> {
    let mut by_branch: BTreeMap<String, MergedBranch> = BTreeMap::new();
    for p in merged.as_array().into_iter().flatten() {
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("");
        let (Some(pr), "MERGED") = (p.get("number").and_then(Value::as_u64), s("state")) else {
            continue;
        };
        let (branch, tip, merged_at) = (s("headRefName"), s("headRefOid"), s("mergedAt"));
        let fork = p
            .get("isCrossRepository")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if fork || branch.is_empty() || branch == base || tip.is_empty() || merged_at.is_empty() {
            continue;
        }
        if heads.get(branch).map(String::as_str) != Some(tip) {
            continue;
        }
        let entry = MergedBranch {
            branch: branch.to_string(),
            tip: tip.to_string(),
            pr,
            merged_at: merged_at.to_string(),
        };
        match by_branch.get(branch) {
            Some(seen) if seen.pr > pr => {}
            _ => {
                by_branch.insert(branch.to_string(), entry);
            }
        }
    }
    by_branch.into_values().collect()
}

/// A pull request that is no longer open, as the forge reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedPullRequest {
    /// Whether the forge says it was merged, rather than closed unmerged. Evidence, and the
    /// difference between a successor that released its hold (closed) and one merged somewhere
    /// master does not contain. Never what makes a successor landed: git does.
    pub merged: bool,
    /// The commit its branch pointed at when it was merged or closed.
    pub head_sha: String,
    /// The body, for the supersessions it declares; never rendered.
    #[serde(default)]
    pub body: String,
    /// Its merge commit, as the forge named it; empty when it has none or it was not read.
    #[serde(default)]
    pub merge_commit: String,
    /// Its author's login; empty when unread. Evidence only.
    #[serde(default)]
    pub author: String,
    /// The forge's word for what its author is to the repository; empty when unread.
    #[serde(default)]
    pub author_association: String,
    /// Whether its head lives in a fork. A record that does not say is read as a fork.
    #[serde(default = "unread_is_a_fork")]
    pub cross_repository: bool,
    /// The branch it asked to merge into; empty when unread. Evidence only.
    #[serde(default)]
    pub base_ref: String,
    /// How many files the forge says it changes; zero when unread. A veto, never a proof: a
    /// pull request that changes no file brought nothing to master, wherever its head now
    /// points, so it is never a successor that landed.
    #[serde(default)]
    pub changed_files: u64,
}

/// What a resolved pull request that does not say where its head lives is read as: a fork,
/// which declares nothing.
fn unread_is_a_fork() -> bool {
    true
}

/// How many open pull requests one observation lists. A forge with as many open as this may
/// have more, and the queue says so.
pub const OPEN_LIMIT: usize = 500;

/// How many open pull requests one page of the declarations read asks for.
pub const DECLARATIONS_PAGE: usize = 50;

/// How many pages of one pull request's cross-references are read, a hundred each: five
/// thousand mentions. One that more pull requests and issues mention is held, never taken to
/// have no declaration. Every mention counts, an issue's and another repository's too, so the
/// bound is what an outsider must exceed to hold a pull request, and the cost of reading up
/// to it is theirs to raise; it is far above any mention count this repository has seen.
pub const REFERENCE_PAGES: usize = 50;

/// What `gh pr view` is asked of a successor or dependency that is no longer open: what
/// [`resolved_of()`] reads, but for the author's association, which `gh pr view` does not
/// carry.
pub const RESOLVED_FIELDS: &str =
    "number,state,headRefOid,body,mergeCommit,baseRefName,author,isCrossRepository,changedFiles";

/// One pull request that is no longer open, from the source of a cross-reference
/// ([`DECLARATIONS_QUERY`]) or from `gh pr view --json` output: `None` for an open one, or one
/// without a head. What a reading does not say fails closed: no merge commit, no author, no
/// association, a head read as a fork's and no changed file, so it declares nothing and
/// never landed.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::resolved_of;
/// let viewed = serde_json::json!({"number": 7, "state": "MERGED", "headRefOid": "h7",
///     "mergeCommit": {"oid": "m7"}});
/// let (number, read) = resolved_of(&viewed).unwrap();
/// assert_eq!((number, read.merged, read.merge_commit.as_str()), (7, true, "m7"));
/// assert!(read.cross_repository && read.author_association.is_empty(), "unread declares nothing");
/// assert_eq!(read.changed_files, 0, "and changed nothing");
/// assert!(resolved_of(&serde_json::json!({"number": 7, "state": "OPEN", "headRefOid": "h7"})).is_none());
/// ```
pub fn resolved_of(v: &Value) -> Option<(u64, ResolvedPullRequest)> {
    let number = v.get("number")?.as_u64()?;
    let merged = match v.get("state")?.as_str()? {
        "MERGED" => true,
        "CLOSED" => false,
        _ => return None,
    };
    let head_sha = v.get("headRefOid")?.as_str()?.to_string();
    if head_sha.is_empty() {
        return None;
    }
    let text = |pointer: &str| {
        v.pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    Some((
        number,
        ResolvedPullRequest {
            merged,
            head_sha,
            body: text("/body"),
            merge_commit: text("/mergeCommit/oid"),
            author: text("/author/login"),
            author_association: text("/authorAssociation"),
            cross_repository: v
                .get("isCrossRepository")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            base_ref: text("/baseRefName"),
            changed_files: v.get("changedFiles").and_then(Value::as_u64).unwrap_or(0),
        },
    ))
}

/// The pull requests no longer open that a supersession or a dependency involving an open one
/// names: closed or merged ones whose body says they supersede an open one (`closed`: the
/// sources of the open ones' cross-references, as [`declare()`] returned them), and the
/// successors and dependencies open bodies name that are not open, each read with `view`. One
/// `view` cannot read is left out, and the classifier says it is unread.
pub fn resolved_for(
    open: &[PullRequestObservation],
    closed: &Value,
    mut view: impl FnMut(u64) -> Option<Value>,
) -> BTreeMap<u64, ResolvedPullRequest> {
    use super::classify::declared_supersessions;
    let numbers: BTreeSet<u64> = open.iter().map(|p| p.number).collect();
    let mut resolved: BTreeMap<u64, ResolvedPullRequest> = closed
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(resolved_of)
        .filter(|(n, r)| {
            !numbers.contains(n)
                && declared_supersessions(&r.body)
                    .supersedes
                    .iter()
                    .any(|t| t != n && numbers.contains(t))
        })
        .collect();
    let named: BTreeSet<u64> = open
        .iter()
        .flat_map(|p| {
            declared_supersessions(&p.body)
                .superseded_by
                .into_iter()
                .chain(super::classify::declared_dependencies(&p.body))
        })
        .filter(|m| !numbers.contains(m) && !resolved.contains_key(m))
        .collect();
    for m in named {
        if let Some((n, r)) = view(m).as_ref().and_then(resolved_of) {
            if n == m {
                resolved.insert(n, r);
            }
        }
    }
    resolved
}

/// Why the forge could not be observed.
#[derive(Debug)]
pub struct ForgeError(pub String);

impl std::fmt::Display for ForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A source of forge observations. The command line uses [`GhForge`]; tests use a value.
pub trait Forge {
    /// Observe the forge now.
    fn observe(&self) -> Result<ForgeObservation, ForgeError>;
}

/// The GitHub CLI, run in the repository.
pub struct GhForge<'a> {
    /// The repository root.
    pub root: &'a Path,
}

fn gh(root: &Path, args: &[&str]) -> Result<(bool, String, String), ForgeError> {
    let out = Command::new("gh")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| ForgeError(format!("the GitHub CLI (gh) could not run: {e}")))?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    ))
}

/// `gh`, asked again on a transient failure ([`super::retry`]): every call here is a read,
/// so asking twice is harmless. A refusal is returned as it came, for the caller to read.
fn gh_retrying(root: &Path, args: &[&str]) -> Result<(bool, String, String), ForgeError> {
    super::retry::forge(|| {
        let (ok, out, err) = gh(root, args).map_err(|e| e.0)?;
        if !ok && super::retry::transient(&err) {
            Err(format!("gh {} failed: {}", args.join(" "), err.trim()))
        } else {
            Ok((ok, out, err))
        }
    })
    .map_err(ForgeError)
}

fn gh_json(root: &Path, args: &[&str]) -> Result<Value, ForgeError> {
    let (ok, out, err) = gh_retrying(root, args)?;
    if !ok {
        return Err(ForgeError(format!(
            "gh {} failed: {}",
            args.join(" "),
            err.trim()
        )));
    }
    serde_json::from_str(&out).map_err(|e| ForgeError(format!("gh {}: {e}", args.join(" "))))
}

/// The state word of one rollup entry: a check run (`status`/`conclusion`) or a commit
/// status context (`state`).
pub fn check_state(entry: &Value) -> CheckRunState {
    let s = |k: &str| {
        entry
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_uppercase()
    };
    if entry.get("__typename").and_then(Value::as_str) == Some("StatusContext") {
        return match s("state").as_str() {
            "SUCCESS" => CheckRunState::Passed,
            "PENDING" | "EXPECTED" => CheckRunState::Pending,
            _ => CheckRunState::Failed,
        };
    }
    if s("status") != "COMPLETED" {
        return CheckRunState::Pending;
    }
    match s("conclusion").as_str() {
        "SUCCESS" => CheckRunState::Passed,
        "SKIPPED" | "NEUTRAL" => CheckRunState::Skipped,
        _ => CheckRunState::Failed,
    }
}

/// One check from a rollup entry: `gh pr list --json statusCheckRollup` output, or a node of
/// the GraphQL rollup [`WRITERS_QUERY`] reads. The two have one shape, but for the app: only
/// the GraphQL node carries `checkSuite.app.databaseId`, so a check run read from `gh pr list`
/// names no app, and neither does a status context, which no app writes.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::check_of;
/// let run = serde_json::json!({"__typename": "CheckRun", "name": "ci", "status": "COMPLETED",
///     "conclusion": "SUCCESS", "completedAt": "2026-09-01T00:05:00Z",
///     "checkSuite": {"app": {"databaseId": 15368}}});
/// assert_eq!(check_of(&run).app_id, Some(15368));
/// let listed = serde_json::json!({"__typename": "CheckRun", "name": "ci", "status": "COMPLETED"});
/// assert_eq!(check_of(&listed).app_id, None, "gh pr list does not say who wrote it");
/// ```
pub fn check_of(entry: &Value) -> CheckObservation {
    let text = |k: &str| entry.get(k).and_then(Value::as_str).unwrap_or("");
    let kind = if text("__typename") == "StatusContext" {
        CheckKind::StatusContext
    } else {
        CheckKind::CheckRun
    };
    CheckObservation {
        name: entry
            .get("name")
            .or_else(|| entry.get("context"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        state: check_state(entry),
        kind,
        // the app of the suite the run belongs to: the one writer a forge read names
        app_id: entry
            .pointer("/checkSuite/app/databaseId")
            .and_then(Value::as_u64),
        completed_at: match text("completedAt") {
            // a check run's zero time means it has not completed
            "" | "0001-01-01T00:00:00Z" => text("startedAt")
                .trim_start_matches("0001-01-01T00:00:00Z")
                .to_string(),
            t => t.to_string(),
        },
    }
}

/// One pull request from `gh pr list --json` output.
pub fn pull_request_of(v: &Value) -> Option<PullRequestObservation> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Some(PullRequestObservation {
        number: v.get("number")?.as_u64()?,
        title: s("title"),
        author: v
            .pointer("/author/login")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        head_ref: s("headRefName"),
        head_sha: s("headRefOid"),
        base_ref: s("baseRefName"),
        draft: v.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
        labels: v
            .get("labels")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|l| l.get("name").and_then(Value::as_str).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        created_at: s("createdAt"),
        updated_at: s("updatedAt"),
        body: s("body"),
        checks: v
            .get("statusCheckRollup")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(check_of).collect())
            .unwrap_or_default(),
        review_decision: s("reviewDecision"),
        auto_merge: v
            .get("autoMergeRequest")
            .map(|a| !a.is_null())
            .unwrap_or(false),
        cross_repository: v
            .get("isCrossRepository")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        // `gh pr list` reports neither: the declarations read does ([`declare()`])
        author_association: String::new(),
        cross_references: CrossReferenceRead::Unread,
        latest_reviews: v
            .get("latestReviews")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|r| ReviewObservation {
                        author: r
                            .pointer("/author/login")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        state: r
                            .get("state")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_ascii_uppercase(),
                        commit: r
                            .pointer("/commit/oid")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        review_requests: v
            .get("reviewRequests")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|r| {
                        r.get("login")
                            .or_else(|| r.get("slug"))
                            .or_else(|| r.get("name"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Union two readings of the required checks: a context required by either is required,
/// once for each app it is bound to. Two sources that bind one context to two apps require
/// both apps' check runs, as the forge does; an unbound entry of a context is the same
/// requirement as a bound one and is dropped beside it. In context order, then app order.
fn union_checks(a: Vec<RequiredCheck>, b: Vec<RequiredCheck>) -> Vec<RequiredCheck> {
    let all: BTreeSet<RequiredCheck> = a.into_iter().chain(b).collect();
    let bound: BTreeSet<String> = all
        .iter()
        .filter(|c| c.app_id.is_some())
        .map(|c| c.context.clone())
        .collect();
    all.into_iter()
        .filter(|c| c.app_id.is_some() || !bound.contains(&c.context))
        .collect()
}

/// Union two review requirements: the stricter of each.
fn union_reviews(a: ReviewPolicy, b: ReviewPolicy) -> ReviewPolicy {
    ReviewPolicy {
        approvals: a.approvals.max(b.approvals),
        code_owners: a.code_owners || b.code_owners,
        dismiss_stale: a.dismiss_stale || b.dismiss_stale,
    }
}

/// The required checks and review requirement the rulesets that apply to a branch add,
/// from `repos/{r}/rules/branches/{base}`: its `required_status_checks` rules (a context and
/// the `integration_id` bound to it) and its `pull_request` rules.
pub fn rules_of(v: &Value) -> (Vec<RequiredCheck>, ReviewPolicy) {
    let mut checks = Vec::new();
    let mut reviews = ReviewPolicy::default();
    for rule in v.as_array().into_iter().flatten() {
        let params = rule.get("parameters");
        match rule.get("type").and_then(Value::as_str) {
            Some("required_status_checks") => {
                for c in params
                    .and_then(|p| p.get("required_status_checks"))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(context) = c.get("context").and_then(Value::as_str) {
                        checks.push(RequiredCheck {
                            context: context.to_string(),
                            app_id: c.get("integration_id").and_then(Value::as_u64),
                        });
                    }
                }
            }
            Some("pull_request") => {
                let p = |k: &str| params.and_then(|p| p.get(k));
                reviews = union_reviews(
                    reviews,
                    ReviewPolicy {
                        approvals: p("required_approving_review_count")
                            .and_then(Value::as_u64)
                            .unwrap_or(0),
                        code_owners: p("require_code_owner_review")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        dismiss_stale: p("dismiss_stale_reviews_on_push")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    },
                );
            }
            _ => {}
        }
    }
    (union_checks(checks, Vec::new()), reviews)
}

/// Whether a branch protection requires a branch to be up to date before it merges.
pub fn protection_requires_up_to_date(v: &Value) -> bool {
    v.pointer("/required_status_checks/strict")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Whether any ruleset that applies requires a branch to be up to date before it merges.
pub fn rules_require_up_to_date(v: &Value) -> bool {
    v.as_array().into_iter().flatten().any(|rule| {
        rule.get("type").and_then(Value::as_str) == Some("required_status_checks")
            && rule
                .pointer("/parameters/strict_required_status_checks_policy")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    })
}

/// The required checks and review requirement from a branch-protection document. A
/// context the protection lists under `contexts` and again under `checks` is one check,
/// bound to the app `checks` names for it.
pub fn protection_of(v: &Value) -> (Vec<RequiredCheck>, ReviewPolicy) {
    let listed: Vec<RequiredCheck> = v
        .pointer("/required_status_checks/contexts")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| c.as_str().map(RequiredCheck::from))
                .collect()
        })
        .unwrap_or_default();
    let bound: Vec<RequiredCheck> = v
        .pointer("/required_status_checks/checks")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some(RequiredCheck {
                        context: c.get("context")?.as_str()?.to_string(),
                        app_id: c.get("app_id").and_then(Value::as_u64),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let reviews = match v.get("required_pull_request_reviews") {
        // a review section without a count still asks for one
        Some(r) => ReviewPolicy {
            approvals: r
                .get("required_approving_review_count")
                .and_then(Value::as_u64)
                .unwrap_or(1),
            code_owners: r
                .get("require_code_owner_reviews")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            dismiss_stale: r
                .get("dismiss_stale_reviews")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        None => ReviewPolicy::default(),
    };
    (union_checks(bound, listed), reviews)
}

/// How many open pull requests one page of the writers read asks for.
pub const WRITERS_PAGE: usize = 50;

/// How many pages of one head's contexts are read, a hundred contexts each. A head carrying
/// more is left unattributed, and a context bound to an app is then `unknown` on it.
pub const CONTEXT_PAGES: usize = 10;

/// The GraphQL read of who wrote each check: the open pull requests, newest first as
/// `gh pr list` lists them, each with its head and the first hundred contexts of its rollup.
/// A context node has the shape of a `gh pr list` rollup entry plus `checkSuite.app`, so
/// [`check_of()`] reads both; a status context's `createdAt` is asked for as `startedAt`, the
/// name the rollup entry gives the time it was set. Variables: `owner`, `name`, `n` (the page
/// size) and `after` (the cursor of the page before, absent for the first).
pub const WRITERS_QUERY: &str = "query($owner:String!,$name:String!,$n:Int!,$after:String){\
repository(owner:$owner,name:$name){\
pullRequests(states:OPEN,first:$n,after:$after,orderBy:{field:CREATED_AT,direction:DESC}){\
pageInfo{hasNextPage endCursor}\
nodes{number headRefOid statusCheckRollup{contexts(first:100){\
pageInfo{hasNextPage endCursor}\
nodes{__typename \
...on CheckRun{name status conclusion startedAt completedAt checkSuite{app{databaseId}}}\
...on StatusContext{context state startedAt:createdAt}}}}}}}}";

/// The GraphQL read of one pull request's contexts, a page at a time: what
/// [`WRITERS_QUERY`] asks of every open pull request, asked of the one whose contexts did
/// not fit a page. Variables: `owner`, `name`, `number` and `after` (the cursor of the page of
/// contexts before, absent for the first).
pub const WRITERS_OF_QUERY: &str =
    "query($owner:String!,$name:String!,$number:Int!,$after:String){\
repository(owner:$owner,name:$name){\
pullRequest(number:$number){number headRefOid statusCheckRollup{contexts(first:100,after:$after){\
pageInfo{hasNextPage endCursor}\
nodes{__typename \
...on CheckRun{name status conclusion startedAt completedAt checkSuite{app{databaseId}}}\
...on StatusContext{context state startedAt:createdAt}}}}}}}";

/// The checks read with their writers, by pull request number: the head they are of, and
/// the checks. What [`attributed_checks()`] answers and [`attribute()`] applies.
pub type AttributedChecks = BTreeMap<u64, (String, Vec<CheckObservation>)>;

/// One page of one pull request's rollup, as a writers answer gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollupPage {
    /// The head the rollup is of.
    pub head: String,
    /// The checks on this page, each with the app that wrote it where the forge named one.
    pub checks: Vec<CheckObservation>,
    /// Whether these are all of them: `false` when the forge says more contexts follow, and
    /// when it listed contexts without saying whether more do.
    pub complete: bool,
    /// Where the next page of contexts starts, when the forge named a cursor.
    pub cursor: Option<String>,
}

/// One pull request node of a writers answer, read as a [`RollupPage`]. A head without a
/// rollup (`statusCheckRollup: null`) has no check, and that is all of them.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::rollup_page_of;
/// let node = serde_json::json!({"number": 1, "headRefOid": "h1", "statusCheckRollup": {"contexts": {
///     "pageInfo": {"hasNextPage": true, "endCursor": "c1"},
///     "nodes": [{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED",
///                "conclusion": "SUCCESS", "checkSuite": {"app": {"databaseId": 15368}}}]}}});
/// let page = rollup_page_of(&node);
/// assert_eq!((page.head.as_str(), page.checks[0].app_id), ("h1", Some(15368)));
/// assert_eq!((page.complete, page.cursor.as_deref()), (false, Some("c1")));
/// assert!(rollup_page_of(&serde_json::json!({"headRefOid": "h1", "statusCheckRollup": null})).complete);
/// ```
pub fn rollup_page_of(node: &Value) -> RollupPage {
    let contexts = node.pointer("/statusCheckRollup/contexts");
    let info = |k: &str| contexts.and_then(|c| c.pointer(k));
    RollupPage {
        head: node
            .get("headRefOid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        checks: contexts
            .and_then(|c| c.get("nodes"))
            .and_then(Value::as_array)
            .map(|a| a.iter().map(check_of).collect())
            .unwrap_or_default(),
        complete: contexts.is_none()
            || info("/pageInfo/hasNextPage").and_then(Value::as_bool) == Some(false),
        cursor: info("/pageInfo/endCursor")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// Every numbered pull request node of one [`WRITERS_QUERY`] answer, with its rollup page.
fn rollups_of(page: &Value) -> Vec<(u64, RollupPage)> {
    page.pointer("/data/repository/pullRequests/nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| {
            node.get("number")
                .and_then(Value::as_u64)
                .map(|number| (number, rollup_page_of(node)))
        })
        .collect()
}

/// The pull requests one [`WRITERS_QUERY`] answer read whole: number → (head, checks). One
/// whose contexts did not fit the page is left out ([`truncated_of()`] names it): a rollup
/// read in part is not read, and a failure on the page that was not read must not go unseen.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::attributed_checks_of;
/// let page = serde_json::json!({"data": {"repository": {"pullRequests": {"nodes": [
///   {"number": 1, "headRefOid": "h1", "statusCheckRollup": {"contexts": {
///      "pageInfo": {"hasNextPage": false}, "nodes": [{"__typename": "CheckRun", "name": "ci",
///        "status": "COMPLETED", "conclusion": "SUCCESS", "checkSuite": {"app": {"databaseId": 15368}}}]}}},
///   {"number": 2, "headRefOid": "h2", "statusCheckRollup": {"contexts": {
///      "pageInfo": {"hasNextPage": true, "endCursor": "c"}, "nodes": []}}}
/// ]}}}});
/// let read = attributed_checks_of(&page);
/// assert_eq!(read[&1].0, "h1");
/// assert_eq!(read[&1].1[0].app_id, Some(15368));
/// assert!(!read.contains_key(&2), "a truncated rollup is unread");
/// ```
pub fn attributed_checks_of(page: &Value) -> AttributedChecks {
    rollups_of(page)
        .into_iter()
        .filter(|(_, rollup)| rollup.complete)
        .map(|(number, rollup)| (number, (rollup.head, rollup.checks)))
        .collect()
}

/// The pull requests of one [`WRITERS_QUERY`] answer whose contexts did not fit the page:
/// what [`attributed_checks_of()`] leaves out, and [`attributed_checks()`] reads again a page
/// of contexts at a time.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::truncated_of;
/// let page = serde_json::json!({"data": {"repository": {"pullRequests": {"nodes": [
///   {"number": 2, "headRefOid": "h2", "statusCheckRollup": {"contexts": {
///      "pageInfo": {"hasNextPage": true, "endCursor": "c"}, "nodes": []}}}
/// ]}}}});
/// assert_eq!(truncated_of(&page).into_iter().collect::<Vec<_>>(), [2]);
/// ```
pub fn truncated_of(page: &Value) -> BTreeSet<u64> {
    rollups_of(page)
        .into_iter()
        .filter(|(_, rollup)| !rollup.complete)
        .map(|(number, _)| number)
        .collect()
}

/// The cursor the next page of pull requests starts after, when one [`WRITERS_QUERY`]
/// answer says there is one and names it.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::next_cursor;
/// let more = serde_json::json!({"data": {"repository": {"pullRequests": {
///     "pageInfo": {"hasNextPage": true, "endCursor": "c1"}, "nodes": []}}}});
/// assert_eq!(next_cursor(&more).as_deref(), Some("c1"));
/// assert_eq!(next_cursor(&serde_json::json!({})), None);
/// ```
pub fn next_cursor(page: &Value) -> Option<String> {
    let info = page.pointer("/data/repository/pullRequests/pageInfo");
    let said = |k: &str| info.and_then(|i| i.get(k));
    let more = said("hasNextPage").and_then(Value::as_bool) == Some(true);
    said("endCursor")
        .and_then(Value::as_str)
        .filter(|_| more)
        .map(str::to_string)
}

/// One pull request's whole rollup, read a page of contexts at a time with
/// [`WRITERS_OF_QUERY`]: `ask` answers the page after a cursor (`None` first). `None` when it
/// cannot be read whole: the head moved between two pages, a page says more follow and names
/// no cursor, or [`CONTEXT_PAGES`] pages were not enough. Unread is never a shorter list.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::whole_rollup;
/// let page = |more: bool, name: &str| serde_json::json!({"data": {"repository": {"pullRequest": {
///     "number": 1, "headRefOid": "h1", "statusCheckRollup": {"contexts": {
///       "pageInfo": {"hasNextPage": more, "endCursor": "c1"},
///       "nodes": [{"__typename": "CheckRun", "name": name, "status": "COMPLETED"}]}}}}}});
/// let whole = whole_rollup(|after| Ok(page(after.is_none(), after.unwrap_or("ci")))).unwrap();
/// let (head, checks) = whole.expect("two pages");
/// assert_eq!((head.as_str(), checks.len()), ("h1", 2));
/// ```
pub fn whole_rollup(
    mut ask: impl FnMut(Option<&str>) -> Result<Value, ForgeError>,
) -> Result<Option<(String, Vec<CheckObservation>)>, ForgeError> {
    let mut read: Option<(String, Vec<CheckObservation>)> = None;
    let mut cursor: Option<String> = None;
    for _ in 0..CONTEXT_PAGES {
        let answer = ask(cursor.as_deref())?;
        let page = rollup_page_of(
            answer
                .pointer("/data/repository/pullRequest")
                .unwrap_or(&Value::Null),
        );
        let complete = page.complete;
        let (head, mut checks) = read
            .take()
            .unwrap_or_else(|| (page.head.clone(), Vec::new()));
        // a rollup is of one head: pages of two heads are not one rollup
        let same_head = head == page.head;
        checks.extend(page.checks);
        if complete || !same_head || page.cursor.is_none() {
            return Ok((complete && same_head).then_some((head, checks)));
        }
        read = Some((head, checks));
        cursor = page.cursor;
    }
    Ok(None)
}

/// The checks of the open pull requests `wanted` names, each read with the app that wrote
/// it. `page` answers one [`WRITERS_QUERY`] page after a cursor (`None` first); pages are
/// asked for until every wanted pull request was seen, the forge has no more, or `limit`
/// pull requests' worth of pages were asked for. A wanted one whose contexts did not fit its
/// page is read again whole with `rest` ([`whole_rollup()`]: its number, then the cursor). One
/// that stays unread is absent from the answer, and [`attribute()`] leaves it unattributed. A
/// read that fails fails the whole: a writer is read or the observation is not made.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::attributed_checks;
/// let page = serde_json::json!({"data": {"repository": {"pullRequests": {
///   "pageInfo": {"hasNextPage": false}, "nodes": [
///     {"number": 1, "headRefOid": "h1", "statusCheckRollup": null}]}}}});
/// let wanted = [1].into_iter().collect();
/// let read = attributed_checks(&wanted, 500, |_| Ok(page.clone()), |_, _| unreachable!()).unwrap();
/// assert_eq!(read[&1], ("h1".to_string(), vec![]));
/// ```
pub fn attributed_checks(
    wanted: &BTreeSet<u64>,
    limit: usize,
    mut page: impl FnMut(Option<&str>) -> Result<Value, ForgeError>,
    mut rest: impl FnMut(u64, Option<&str>) -> Result<Value, ForgeError>,
) -> Result<AttributedChecks, ForgeError> {
    let mut read = AttributedChecks::new();
    let mut truncated: BTreeSet<u64> = BTreeSet::new();
    let mut cursor: Option<String> = None;
    for _ in 0..limit.div_ceil(WRITERS_PAGE) {
        if wanted
            .iter()
            .all(|n| read.contains_key(n) || truncated.contains(n))
        {
            break;
        }
        let answer = page(cursor.as_deref())?;
        read.extend(attributed_checks_of(&answer));
        truncated.extend(truncated_of(&answer));
        cursor = next_cursor(&answer);
        if cursor.is_none() {
            break;
        }
    }
    for number in truncated.intersection(wanted) {
        let whole = whole_rollup(|after| rest(*number, after))?;
        read.extend(whole.map(|rollup| (*number, rollup)));
    }
    Ok(read)
}

/// Give each observed pull request the checks read with their writers. Only a read of the
/// head that was observed counts: a pull request the read does not name, or names at another
/// head (it moved between the two reads), keeps the checks `gh pr list` gave it, none of
/// which names an app, so a context bound to an app is `unknown` on it and never passed. A
/// head that was not observed is never matched, whatever the read says of it.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::{attribute, pull_request_of, AttributedChecks};
/// let mut prs = vec![pull_request_of(&serde_json::json!({"number": 1, "headRefOid": "h1"})).unwrap()];
/// let mut read = AttributedChecks::new();
/// read.insert(1, ("moved".to_string(), vec![Default::default()]));
/// attribute(&mut prs, &read);
/// assert!(prs[0].checks.is_empty(), "another head's checks are not this head's");
/// ```
pub fn attribute(prs: &mut [PullRequestObservation], read: &AttributedChecks) {
    for pr in prs.iter_mut() {
        let checks = read
            .get(&pr.number)
            .filter(|(head, _)| !head.is_empty() && *head == pr.head_sha)
            .map(|(_, checks)| checks.clone());
        if let Some(checks) = checks {
            pr.checks = checks;
        }
    }
}

/// The `gh api graphql` arguments of one writers read: `query`, the repository's `owner` and
/// `name`, one typed variable (`n=50` or `number=7`) and the cursor to read after. Everything
/// but the number goes with `-f`, as the string it is: `-F` would read an owner or a
/// repository called `2048`, `true` or `null` as a number, a boolean or nothing, and one
/// starting with `@` as a file.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::writers_args;
/// let args = writers_args("query{}", "2048", "true", "n=50", Some("c1"));
/// assert_eq!(args, ["api", "graphql", "-f", "query=query{}", "-f", "owner=2048", "-f",
///     "name=true", "-F", "n=50", "-f", "after=c1"]);
/// ```
pub fn writers_args(
    query: &str,
    owner: &str,
    name: &str,
    typed: &str,
    after: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "api".to_string(),
        "graphql".to_string(),
        "-f".to_string(),
        format!("query={query}"),
        "-f".to_string(),
        format!("owner={owner}"),
        "-f".to_string(),
        format!("name={name}"),
        "-F".to_string(),
        typed.to_string(),
    ];
    args.extend(
        after
            .into_iter()
            .flat_map(|cursor| ["-f".to_string(), format!("after={cursor}")]),
    );
    args
}

/// One GraphQL read through `gh`, with the arguments [`writers_args()`] built.
fn gh_graphql(root: &Path, args: &[String]) -> Result<Value, ForgeError> {
    gh_json(root, &args.iter().map(String::as_str).collect::<Vec<_>>())
}

/// The GraphQL read of who may declare and what was declared: the open pull requests, newest
/// first as `gh pr list` lists them, each with its author's association, whether its head
/// lives in a fork, and the first hundred cross-references to it. The source of a
/// cross-reference that is a pull request is read with everything [`resolved_of()`] reads: its
/// state, body, head, merge commit, whether its head lives in a fork, its author and that
/// author's association, its base, and how many files it changes. Two flags share a name: the
/// event's `isCrossRepository` says the mention came from another repository, and a pull
/// request's says its head lives in a fork. Variables: `owner`, `name`, `n` (the page size)
/// and `after` (the cursor of the page before, absent for the first).
pub const DECLARATIONS_QUERY: &str = "query($owner:String!,$name:String!,$n:Int!,$after:String){\
repository(owner:$owner,name:$name){\
pullRequests(states:OPEN,first:$n,after:$after,orderBy:{field:CREATED_AT,direction:DESC}){\
pageInfo{hasNextPage endCursor}\
nodes{number authorAssociation isCrossRepository \
timelineItems(first:100,itemTypes:[CROSS_REFERENCED_EVENT]){\
pageInfo{hasNextPage endCursor}\
nodes{...on CrossReferencedEvent{isCrossRepository source{__typename \
...on PullRequest{number state body headRefOid mergeCommit{oid} isCrossRepository \
authorAssociation author{login} baseRefName changedFiles}}}}}}}}}";

/// The GraphQL read of one pull request's cross-references, a page at a time: what
/// [`DECLARATIONS_QUERY`] asks of every open pull request, asked of the one whose
/// cross-references did not fit a page. Variables: `owner`, `name`, `number` and `after` (the
/// cursor of the page of cross-references before, absent for the first).
pub const DECLARATIONS_OF_QUERY: &str =
    "query($owner:String!,$name:String!,$number:Int!,$after:String){\
repository(owner:$owner,name:$name){\
pullRequest(number:$number){number authorAssociation isCrossRepository \
timelineItems(first:100,after:$after,itemTypes:[CROSS_REFERENCED_EVENT]){\
pageInfo{hasNextPage endCursor}\
nodes{...on CrossReferencedEvent{isCrossRepository source{__typename \
...on PullRequest{number state body headRefOid mergeCommit{oid} isCrossRepository \
authorAssociation author{login} baseRefName changedFiles}}}}}}}}";

/// One page of the cross-references to one pull request.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferencePage {
    /// The forge's word for what the pull request's author is to the repository; empty when
    /// the node does not say.
    pub association: String,
    /// Whether the pull request's head lives in a fork, as this read said it; `None` when the
    /// node does not say.
    pub cross_repository: Option<bool>,
    /// The pull requests of this repository that mention it, each as the forge described it.
    pub sources: Vec<Value>,
    /// Whether these are all of them: only when the forge says no more follow.
    pub complete: bool,
    /// Where the next page of cross-references starts, when the forge named a cursor.
    pub cursor: Option<String>,
}

/// One pull request node of a declarations answer, read as a [`ReferencePage`]. A source is
/// kept only when the event says the mention came from this repository and the source is a
/// numbered pull request: a `#N` written in another repository is not this repository's #N,
/// an issue declares nothing, and an event that does not say where it came from is not
/// trusted to be from here. Unlike a rollup, a node without `timelineItems` is not complete:
/// what was not listed was not read.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::reference_page_of;
/// let node = serde_json::json!({"number": 1, "authorAssociation": "OWNER",
///     "isCrossRepository": false, "timelineItems": {
///     "pageInfo": {"hasNextPage": true, "endCursor": "c1"},
///     "nodes": [{"isCrossRepository": false, "source": {"__typename": "PullRequest", "number": 2}},
///               {"isCrossRepository": true, "source": {"__typename": "PullRequest", "number": 3}},
///               {"isCrossRepository": false, "source": {"__typename": "Issue"}}]}});
/// let page = reference_page_of(&node);
/// assert_eq!((page.association.as_str(), page.sources.len()), ("OWNER", 1));
/// assert_eq!(page.cross_repository, Some(false));
/// assert_eq!((page.complete, page.cursor.as_deref()), (false, Some("c1")));
/// assert!(!reference_page_of(&serde_json::json!({"number": 1})).complete, "unlisted is unread");
/// ```
pub fn reference_page_of(node: &Value) -> ReferencePage {
    let items = node.get("timelineItems");
    let info = |k: &str| items.and_then(|t| t.pointer(k));
    ReferencePage {
        association: node
            .get("authorAssociation")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        cross_repository: node.get("isCrossRepository").and_then(Value::as_bool),
        sources: items
            .and_then(|t| t.get("nodes"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|event| event.get("isCrossRepository").and_then(Value::as_bool) == Some(false))
            .filter_map(|event| event.get("source"))
            .filter(|source| source.get("number").and_then(Value::as_u64).is_some())
            .cloned()
            .collect(),
        complete: info("/pageInfo/hasNextPage").and_then(Value::as_bool) == Some(false),
        cursor: info("/pageInfo/endCursor")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// What was read about one open pull request: who its author is to the repository, where its
/// head lives, and the pull requests that mention it.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclarationRead {
    /// The forge's word for what its author is to the repository; empty when unread.
    pub association: String,
    /// Whether its head lives in a fork, as this read said it; `None` when it did not say.
    /// Only `Some(false)` lets its author declare anything ([`declare()`]).
    pub cross_repository: Option<bool>,
    /// The pull requests of this repository that mention it; empty unless `read` is whole.
    pub sources: Vec<Value>,
    /// Whether every cross-reference was read.
    pub read: CrossReferenceRead,
}

impl DeclarationRead {
    /// One page's answer as a read: whole with its sources when the page is all of them,
    /// truncated — with no source, since a part of the references is not the references —
    /// when it is not.
    fn of(page: ReferencePage, sources: Vec<Value>) -> Self {
        let (sources, read) = if page.complete {
            (sources, CrossReferenceRead::Whole)
        } else {
            (Vec::new(), CrossReferenceRead::Truncated)
        };
        DeclarationRead {
            association: page.association,
            cross_repository: page.cross_repository,
            sources,
            read,
        }
    }
}

/// The declarations read, by open pull request number. What [`declarations()`] answers and
/// [`declare()`] applies.
pub type Declarations = BTreeMap<u64, DeclarationRead>;

/// Every numbered pull request node of one [`DECLARATIONS_QUERY`] answer, as it was read from
/// that page alone: whole when its cross-references fit, truncated — with no source — when
/// they did not.
fn declarations_of(page: &Value) -> Declarations {
    page.pointer("/data/repository/pullRequests/nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| {
            let number = node.get("number").and_then(Value::as_u64)?;
            let mut page = reference_page_of(node);
            let sources = std::mem::take(&mut page.sources);
            Some((number, DeclarationRead::of(page, sources)))
        })
        .collect()
}

/// Every cross-reference to one pull request, read a page at a time with
/// [`DECLARATIONS_OF_QUERY`]: `ask` answers the page after a cursor (`None` first). Whole when
/// the forge says no more follow; truncated, with no source, when a page says more follow and
/// names no cursor, or [`REFERENCE_PAGES`] pages were not enough. `None` when the forge no
/// longer shows the pull request: nothing was read of it, and unread is never a shorter list.
/// A page the forge would not answer is the error it gave: a failed read is not a hold.
fn whole_references(
    mut ask: impl FnMut(Option<&str>) -> Result<Value, ForgeError>,
) -> Result<Option<DeclarationRead>, ForgeError> {
    let mut sources: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut last: Option<ReferencePage> = None;
    for _ in 0..REFERENCE_PAGES {
        let answer = ask(cursor.as_deref())?;
        let Some(node) = answer
            .pointer("/data/repository/pullRequest")
            .filter(|node| node.is_object())
        else {
            return Ok(None);
        };
        let mut page = reference_page_of(node);
        sources.append(&mut page.sources);
        cursor = page.cursor.clone();
        let stop = page.complete || cursor.is_none();
        last = Some(page);
        if stop {
            break;
        }
    }
    // the last page read says whether more follow: one that does not is the whole
    Ok(last.map(|page| DeclarationRead::of(page, sources)))
}

/// What is declared about the open pull requests `wanted` names, and by whom. `page` answers
/// one [`DECLARATIONS_QUERY`] page after a cursor (`None` first); pages are asked for until
/// every wanted pull request was seen, the forge has no more, or `limit` pull requests' worth
/// of pages were asked for. A wanted one that was not read whole by then — its
/// cross-references did not fit its page, or no page listed it — is asked for alone with
/// `rest` (its number, then the cursor), [`REFERENCE_PAGES`] pages at most; one that still
/// does not fit is truncated, with no source at all, and held.
///
/// The two ways a read falls short are kept apart. A truncated read is that pull request's:
/// it is held and the others are decided. A read the forge would not answer — a page, or one
/// pull request's references — is an error, and the caller makes no observation from it:
/// what was not read is never an observation with a part missing. One the forge no longer
/// shows when asked for alone is unread — with the association a page gave it, or else absent
/// from the answer, which [`declare()`] leaves unread too.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::declarations;
/// use majordomus_cli::integration::CrossReferenceRead;
/// let page = serde_json::json!({"data": {"repository": {"pullRequests": {
///   "pageInfo": {"hasNextPage": false}, "nodes": [
///     {"number": 1, "authorAssociation": "OWNER", "isCrossRepository": false, "timelineItems": {
///        "pageInfo": {"hasNextPage": false}, "nodes": []}}]}}}});
/// let wanted = [1, 2].into_iter().collect();
/// let gone = |_: u64, _: Option<&str>| Ok(serde_json::json!({"data": {"repository": {"pullRequest": null}}}));
/// let read = declarations(&wanted, 500, |_| Ok(page.clone()), gone).unwrap();
/// assert_eq!((read[&1].association.as_str(), read[&1].read), ("OWNER", CrossReferenceRead::Whole));
/// assert!(!read.contains_key(&2), "one the forge no longer shows is unread, and #1 still is read");
/// let refused = |_: u64, _: Option<&str>| Err(ForgeError("HTTP 403".into()));
/// assert!(declarations(&wanted, 500, |_| Ok(page.clone()), refused).is_err(), "a failed read is an error");
/// ```
pub fn declarations(
    wanted: &BTreeSet<u64>,
    limit: usize,
    mut page: impl FnMut(Option<&str>) -> Result<Value, ForgeError>,
    mut rest: impl FnMut(u64, Option<&str>) -> Result<Value, ForgeError>,
) -> Result<Declarations, ForgeError> {
    let mut read = Declarations::new();
    let mut cursor: Option<String> = None;
    for _ in 0..limit.div_ceil(DECLARATIONS_PAGE) {
        if wanted.iter().all(|n| read.contains_key(n)) {
            break;
        }
        let answer = page(cursor.as_deref())?;
        read.extend(declarations_of(&answer));
        cursor = next_cursor(&answer);
        if cursor.is_none() {
            break;
        }
    }
    let whole = |read: &Declarations, n: &u64| {
        read.get(n)
            .is_some_and(|d| d.read == CrossReferenceRead::Whole)
    };
    let alone: Vec<u64> = wanted
        .iter()
        .copied()
        .filter(|n| !whole(&read, n))
        .collect();
    for number in alone {
        let entry = whole_references(|after| rest(number, after))?;
        // one the forge no longer shows is not a truncation: what a page said of its author
        // is kept, and its references are unread
        if let Some(kept) = read.get_mut(&number).filter(|_| entry.is_none()) {
            kept.read = CrossReferenceRead::Unread;
        }
        read.extend(entry.map(|entry| (number, entry)));
    }
    Ok(read)
}

/// Give each observed pull request what the declarations read said of it: its author's
/// association, and whether its cross-references were read whole. One the read does not name
/// keeps no association and `unread`, so it authorises nothing and is held. Returns the
/// sources of every named pull request's cross-references, in number order, as one array:
/// what [`resolved_for()`] reads the closed declarers from.
///
/// Where a head lives is half of who may declare, and `gh pr list` reads an absent answer as
/// "this repository". So an association is recorded only for a pull request this read placed:
/// one it says lives here (`Some(false)`), or one the list already calls a fork, which
/// declares nothing whatever its association. For one this read did not place, or placed in a
/// fork the list did not see, no association is recorded, and it authorises nothing. The
/// list's own flag is left as it was read.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::forge::{declare, pull_request_of, DeclarationRead, Declarations};
/// use majordomus_cli::integration::CrossReferenceRead;
/// let mut prs = vec![pull_request_of(&serde_json::json!({"number": 1})).unwrap(),
///                    pull_request_of(&serde_json::json!({"number": 2})).unwrap()];
/// let mut read = Declarations::new();
/// read.insert(1, DeclarationRead { association: "OWNER".into(), cross_repository: Some(false),
///     sources: vec![serde_json::json!({"number": 7})], read: CrossReferenceRead::Whole });
/// let sources = declare(&mut prs, &read);
/// assert_eq!(sources, serde_json::json!([{"number": 7}]));
/// assert_eq!(prs[0].cross_references, CrossReferenceRead::Whole);
/// assert_eq!(prs[1].cross_references, CrossReferenceRead::Unread, "not named: held");
/// ```
pub fn declare(prs: &mut [PullRequestObservation], read: &Declarations) -> Value {
    let mut sources: BTreeMap<u64, &[Value]> = BTreeMap::new();
    for pr in prs.iter_mut() {
        if let Some(said) = read.get(&pr.number) {
            let placed = said.cross_repository == Some(false) || pr.cross_repository;
            pr.author_association = Some(said.association.as_str())
                .filter(|_| placed)
                .unwrap_or("")
                .to_string();
            pr.cross_references = said.read;
            sources.insert(pr.number, &said.sources);
        }
    }
    Value::Array(sources.into_values().flatten().cloned().collect())
}

impl Forge for GhForge<'_> {
    fn observe(&self) -> Result<ForgeObservation, ForgeError> {
        let root = self.root;
        let repo = gh_json(
            root,
            &["repo", "view", "--json", "nameWithOwner,defaultBranchRef"],
        )?;
        let repository = repo
            .get("nameWithOwner")
            .and_then(Value::as_str)
            .ok_or_else(|| ForgeError("gh repo view named no repository".into()))?
            .to_string();
        let base = repo
            .pointer("/defaultBranchRef/name")
            .and_then(Value::as_str)
            .ok_or_else(|| ForgeError("gh repo view named no default branch".into()))?
            .to_string();
        let settings = gh_json(root, &["api", &format!("repos/{repository}")])?;
        let delete_branch_on_merge = settings
            .get("delete_branch_on_merge")
            .and_then(Value::as_bool);
        let mut merge_methods = Vec::new();
        for (key, word) in [
            ("allow_merge_commit", "merge"),
            ("allow_squash_merge", "squash"),
            ("allow_rebase_merge", "rebase"),
        ] {
            if settings.get(key).and_then(Value::as_bool).unwrap_or(false) {
                merge_methods.push(word.to_string());
            }
        }
        let base_sha = gh_json(
            root,
            &[
                "api",
                &format!("repos/{repository}/commits/{base}"),
                "--jq",
                "{sha: .sha}",
            ],
        )?
        .get("sha")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
        // an unprotected branch is a 404 whose message says so: nothing is required, which
        // is a reading, not a failure; any other refusal leaves the requirement unread
        let (ok, out, err) = gh_retrying(
            root,
            &[
                "api",
                &format!("repos/{repository}/branches/{base}/protection"),
            ],
        )?;
        let protection: Option<(Vec<RequiredCheck>, ReviewPolicy, bool)> = if ok {
            serde_json::from_str::<Value>(&out).ok().map(|v| {
                let (checks, reviews) = protection_of(&v);
                (checks, reviews, protection_requires_up_to_date(&v))
            })
        } else if err.contains("Branch not protected") || out.contains("Branch not protected") {
            Some((Vec::new(), ReviewPolicy::default(), false))
        } else {
            None
        };
        // the rulesets that apply to the base add requirements the protection does not list;
        // a ruleset read that fails leaves the requirement unread, as an unread protection does
        let (ok, out, _) = gh_retrying(
            root,
            &["api", &format!("repos/{repository}/rules/branches/{base}")],
        )?;
        let rules: Option<(Vec<RequiredCheck>, ReviewPolicy, bool)> = if ok {
            serde_json::from_str::<Value>(&out).ok().map(|v| {
                let (checks, reviews) = rules_of(&v);
                (checks, reviews, rules_require_up_to_date(&v))
            })
        } else {
            None
        };
        let (required_checks, review_policy, up_to_date_required) = match (protection, rules) {
            (Some((pc, pr, ps)), Some((rc, rr, rs))) => (
                Some(union_checks(pc, rc)),
                Some(union_reviews(pr, rr)),
                Some(ps || rs),
            ),
            _ => (None, None, None),
        };
        let list = gh_json(
            root,
            &[
                "pr",
                "list",
                "--state",
                "open",
                "--limit",
                &OPEN_LIMIT.to_string(),
                "--json",
                "number,title,author,headRefName,headRefOid,baseRefName,isDraft,labels,createdAt,updatedAt,body,statusCheckRollup,reviewDecision,latestReviews,reviewRequests,autoMergeRequest,isCrossRepository",
            ],
        )?;
        // keyed by number, so the observation is in number order whatever the forge listed
        let mut pull_requests: Vec<PullRequestObservation> = list
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(pull_request_of)
                    .map(|p| (p.number, p))
                    .collect::<std::collections::BTreeMap<_, _>>()
                    .into_values()
                    .collect()
            })
            .unwrap_or_default();
        // a context bound to an app is only that app's check run, and the list above does not
        // say who wrote one: the writers are read, or the observation is not made. A base that
        // binds nothing needs no writer and asks nothing more.
        let bound = required_checks
            .as_ref()
            .is_some_and(|r| r.iter().any(|c| c.app_id.is_some()));
        let (owner, name) = repository
            .split_once('/')
            .unwrap_or((repository.as_str(), ""));
        let wanted: BTreeSet<u64> = pull_requests.iter().map(|p| p.number).collect();
        let ask = |query: &str, typed: &str, after: Option<&str>| {
            gh_graphql(root, &writers_args(query, owner, name, typed, after))
        };
        if bound {
            let size = format!("n={WRITERS_PAGE}");
            let read = attributed_checks(
                &wanted,
                OPEN_LIMIT,
                |after| ask(WRITERS_QUERY, &size, after),
                |number, after| ask(WRITERS_OF_QUERY, &format!("number={number}"), after),
            )?;
            attribute(&mut pull_requests, &read);
        }
        // who may declare a replacement, and what was declared: every open pull request is
        // read with its author's association and the pull requests that mention it. A
        // successor that landed is no longer listed among the open ones, and it is found
        // among the cross-references of the one it replaces, whenever it was closed: no
        // bounded search of closed pull requests decides. Nothing open, nothing asked; a read
        // that is partial holds the pull request it was about, and no other; a read that
        // fails makes no observation at all.
        let size = format!("n={DECLARATIONS_PAGE}");
        let read = declarations(
            &wanted,
            OPEN_LIMIT,
            |after| ask(DECLARATIONS_QUERY, &size, after),
            |number, after| ask(DECLARATIONS_OF_QUERY, &format!("number={number}"), after),
        )?;
        let sources = declare(&mut pull_requests, &read);
        let mut unreadable = None;
        let resolved = resolved_for(&pull_requests, &sources, |m| {
            match gh_retrying(
                root,
                &["pr", "view", &m.to_string(), "--json", RESOLVED_FIELDS],
            ) {
                Ok((true, out, _)) => serde_json::from_str(&out).ok(),
                // not a pull request, or refused: the successor is unread, never landed
                Ok(_) => None,
                Err(e) => {
                    unreadable.get_or_insert(e);
                    None
                }
            }
        });
        // an outage that outlasted the retries is a failed observation, as for the open list
        if let Some(e) = unreadable {
            return Err(e);
        }
        Ok(ForgeObservation {
            schema: OBSERVATION_SCHEMA,
            repository,
            base,
            base_sha,
            observed_at: crate::peers::rfc3339(std::time::SystemTime::now()),
            required_checks,
            review_policy,
            up_to_date_required,
            merge_methods,
            pull_requests,
            resolved,
            delete_branch_on_merge,
        })
    }
}

/// The merged branches origin still serves ([`merged_branches_of`]), read now, or `None` when
/// origin's branches or the merged pull requests could not be read: unread is not "nothing
/// left behind". Asked only by `prs cleanup`, on demand — never by `prs refresh`, which the
/// executor runs before every decision: what the forge left behind decides no merge, so its
/// two reads stay off the hot path.
pub fn merged_branches(root: &Path, base: &str) -> Option<Vec<MergedBranch>> {
    let heads = origin_heads(root)?;
    merged_branches_given(&heads, base, || merged_list(root))
}

/// [`merged_branches`] once origin's branches are known: an origin serving only the base has
/// left nothing behind and the merged pull requests are never asked for; otherwise `list`
/// answers them, and `None` from it is unread.
///
/// ```text
/// use majordomus_cli::integration::forge::{merged_branches_given, remote_heads_of};
/// let only_base = remote_heads_of("cc\trefs/heads/master\n");
/// assert_eq!(merged_branches_given(&only_base, "master", || unreachable!()), Some(vec![]));
/// let heads = remote_heads_of("aa\trefs/heads/fix/a\ncc\trefs/heads/master\n");
/// assert_eq!(merged_branches_given(&heads, "master", || None), None, "unread");
/// ```
pub fn merged_branches_given(
    heads: &BTreeMap<String, String>,
    base: &str,
    list: impl FnOnce() -> Option<Value>,
) -> Option<Vec<MergedBranch>> {
    if heads.keys().all(|b| b == base) {
        return Some(Vec::new());
    }
    Some(merged_branches_of(&list()?, heads, base))
}

/// Origin's branches, from `git ls-remote --heads origin`; `None` when git cannot answer.
fn origin_heads(root: &Path) -> Option<BTreeMap<String, String>> {
    super::retry::forge(|| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["ls-remote", "--heads", "origin"])
            .output()
            .map_err(ls_remote_could_not_run)
            .and_then(heads_of_output)
    })
    .ok()
}

fn ls_remote_could_not_run(e: std::io::Error) -> String {
    format!("git ls-remote could not run: {e}")
}

/// The branches an `ls-remote --heads` answer names, or what it refused with.
fn heads_of_output(out: std::process::Output) -> Result<BTreeMap<String, String>, String> {
    if out.status.success() {
        Ok(remote_heads_of(&String::from_utf8_lossy(&out.stdout)))
    } else {
        Err(format!(
            "git ls-remote failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The merged pull requests, newest [`MERGED_LIMIT`], as the forge listed them.
fn merged_list(root: &Path) -> Option<Value> {
    gh_retrying(
        root,
        &[
            "pr",
            "list",
            "--state",
            "merged",
            "--limit",
            &MERGED_LIMIT.to_string(),
            "--json",
            "number,state,headRefName,headRefOid,isCrossRepository,mergedAt",
        ],
    )
    .ok()
    .and_then(listed)
}

/// A `gh` answer read as JSON, only when `gh` said it succeeded.
fn listed((ok, out, _): (bool, String, String)) -> Option<Value> {
    if ok {
        serde_json::from_str(&out).ok()
    } else {
        None
    }
}

/// Remove the mirrors of pull requests this observation no longer names: merged, closed or
/// gone. Each one held the head the pull request had when it was last open, and a mirror
/// that is never refreshed again is only a stale answer waiting to be read. Best effort: a
/// mirror that cannot be removed is left, and nothing reads it.
fn prune_mirrors(root: &Path, obs: &ForgeObservation) {
    let keep: std::collections::BTreeSet<u64> = obs
        .pull_requests
        .iter()
        .map(|p| p.number)
        .chain(obs.resolved.keys().copied())
        .collect();
    // git that cannot run lists nothing, and nothing is removed
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["for-each-ref", "--format=%(refname)", PR_REF_PREFIX])
        .output()
        .map(|o| o.stdout)
        .unwrap_or_default();
    for r in String::from_utf8_lossy(&out).lines() {
        let stale = r
            .strip_prefix(PR_REF_PREFIX)
            .and_then(|n| n.parse::<u64>().ok())
            .is_some_and(|n| !keep.contains(&n));
        if stale {
            let _ = Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["update-ref", "-d", r])
                .status();
        }
    }
}

/// Fetch the base and every observed head into this clone, the heads under
/// [`PR_REF_PREFIX`]. One `git fetch`; a head that cannot be fetched stays unknown to the
/// relation, which then says so.
pub fn fetch(root: &Path, obs: &ForgeObservation) -> Result<(), ForgeError> {
    let mut args: Vec<String> = vec![
        "-C".into(),
        root.display().to_string(),
        "fetch".into(),
        "--quiet".into(),
        "--no-tags".into(),
        "origin".into(),
        format!("+refs/heads/{0}:refs/remotes/origin/{0}", obs.base),
    ];
    for p in &obs.pull_requests {
        args.push(format!("+refs/pull/{0}/head:{PR_REF_PREFIX}{0}", p.number));
    }
    // a successor no longer open is fetched too, so git can say whether its head landed; the
    // forge keeps refs/pull/<n>/head for a closed pull request, and only one it read is here
    for n in obs.resolved.keys() {
        args.push(format!("+refs/pull/{n}/head:{PR_REF_PREFIX}{n}"));
    }
    prune_mirrors(root, obs);
    // a fetch is a read: a dropped connection is asked again, a refusal is not
    super::retry::forge(|| {
        let out = Command::new("git")
            .args(&args)
            .output()
            .map_err(|e| format!("git fetch could not run: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "git fetch failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    })
    .map_err(ForgeError)
}

/// Tests only: an observed pull request as a whole declarations read leaves it, its author
/// the repository's owner. What a fixture built from `gh pr list` output needs before a
/// queue is built from it: a pull request whose cross-references were not read is held.
#[cfg(test)]
pub(crate) fn read_whole(mut pr: PullRequestObservation) -> PullRequestObservation {
    pr.author_association = "OWNER".into();
    pr.cross_references = CrossReferenceRead::Whole;
    pr
}

#[cfg(test)]
mod tests {
    //! The forge's JSON, read into the observation: every state word a rollup entry can
    //! carry, a pull request with every field and one with none, and a branch protection in
    //! each of its shapes.

    use super::*;
    use serde_json::json;

    #[test]
    fn every_rollup_state_word_has_one_meaning() {
        let ctx = |s: &str| json!({"__typename": "StatusContext", "state": s});
        assert_eq!(check_state(&ctx("success")), CheckRunState::Passed);
        assert_eq!(check_state(&ctx("PENDING")), CheckRunState::Pending);
        assert_eq!(check_state(&ctx("EXPECTED")), CheckRunState::Pending);
        assert_eq!(check_state(&ctx("ERROR")), CheckRunState::Failed);
        assert_eq!(check_state(&ctx("FAILURE")), CheckRunState::Failed);
        let run = |status: &str, conclusion: &str| json!({"__typename": "CheckRun", "status": status, "conclusion": conclusion});
        assert_eq!(check_state(&run("IN_PROGRESS", "")), CheckRunState::Pending);
        assert_eq!(check_state(&run("QUEUED", "")), CheckRunState::Pending);
        assert_eq!(
            check_state(&run("COMPLETED", "SUCCESS")),
            CheckRunState::Passed
        );
        assert_eq!(
            check_state(&run("completed", "skipped")),
            CheckRunState::Skipped
        );
        assert_eq!(
            check_state(&run("COMPLETED", "NEUTRAL")),
            CheckRunState::Skipped
        );
        assert_eq!(
            check_state(&run("COMPLETED", "FAILURE")),
            CheckRunState::Failed
        );
        assert_eq!(
            check_state(&run("COMPLETED", "CANCELLED")),
            CheckRunState::Failed
        );
        assert_eq!(
            check_state(&run("COMPLETED", "TIMED_OUT")),
            CheckRunState::Failed
        );
        // an entry that says nothing has not completed, so it has not passed
        assert_eq!(check_state(&json!({})), CheckRunState::Pending);
    }

    #[test]
    fn a_pull_request_is_read_with_every_field_and_without_any() {
        let full = json!({
            "number": 42,
            "title": "a change",
            "author": {"login": "someone"},
            "headRefName": "feature/x",
            "headRefOid": "abc",
            "baseRefName": "master",
            "isDraft": true,
            "labels": [{"name": "hold"}, {"name": "docs"}],
            "createdAt": "2026-09-01T00:00:00Z",
            "updatedAt": "2026-09-02T00:00:00Z",
            "body": "Depends on #7.",
            "statusCheckRollup": [
                {"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS"},
                {"__typename": "StatusContext", "context": "legacy", "state": "PENDING"}
            ],
            "reviewDecision": "APPROVED",
            "autoMergeRequest": {"enabledAt": "2026-09-03T00:00:00Z"},
            "isCrossRepository": true
        });
        let p = pull_request_of(&full).expect("read");
        assert_eq!(p.number, 42);
        assert_eq!(p.author, "someone");
        assert!(p.draft && p.auto_merge && p.cross_repository);
        assert_eq!(p.labels, ["hold", "docs"]);
        assert_eq!(p.checks.len(), 2);
        assert_eq!(p.checks[0].name, "ci");
        assert_eq!(p.checks[0].state, CheckRunState::Passed);
        assert_eq!(
            p.checks[1].name, "legacy",
            "a status context is named by its context"
        );
        assert_eq!(p.checks[1].state, CheckRunState::Pending);
        assert_eq!(p.review_decision, "APPROVED");

        let bare = pull_request_of(&json!({"number": 1})).expect("a number is enough");
        assert!(!bare.draft && !bare.auto_merge && !bare.cross_repository);
        assert!(bare.labels.is_empty() && bare.checks.is_empty());
        assert_eq!(bare.title, "");
        let disarmed = pull_request_of(&json!({"number": 2, "autoMergeRequest": null})).unwrap();
        assert!(
            !disarmed.auto_merge,
            "a null auto-merge request is not armed"
        );
        assert!(pull_request_of(&json!({"title": "no number"})).is_none());
    }

    #[test]
    fn a_protection_names_its_checks_once_with_their_app_and_its_review_policy() {
        let both = json!({
            "required_status_checks": {
                "contexts": ["ci", "lint"],
                "checks": [{"context": "ci", "app_id": 15368}, {"context": "docs"}]
            },
            "required_pull_request_reviews": {
                "required_approving_review_count": 2,
                "require_code_owner_reviews": true,
                "dismiss_stale_reviews": true
            }
        });
        let (checks, reviews) = protection_of(&both);
        assert_eq!(
            checks,
            vec![
                RequiredCheck {
                    context: "ci".into(),
                    app_id: Some(15368)
                },
                RequiredCheck::from("docs"),
                RequiredCheck::from("lint"),
            ],
            "a context listed twice is one check, bound to the app `checks` names"
        );
        assert_eq!(
            reviews,
            ReviewPolicy {
                approvals: 2,
                code_owners: true,
                dismiss_stale: true
            }
        );

        let none_required = json!({
            "required_status_checks": {"contexts": []},
            "required_pull_request_reviews": {"required_approving_review_count": 0}
        });
        let (checks, reviews) = protection_of(&none_required);
        assert!(checks.is_empty());
        assert_eq!(reviews.approvals, 0);

        let reviews_without_count = json!({"required_pull_request_reviews": {}});
        assert_eq!(
            protection_of(&reviews_without_count).1.approvals,
            1,
            "a review section without a count still asks for one"
        );
        assert_eq!(protection_of(&json!({})).1, ReviewPolicy::default());
    }

    #[test]
    fn the_rulesets_add_required_checks_bound_to_an_app_and_a_review_policy() {
        let rules = json!([
            {"type": "deletion"},
            {"type": "required_status_checks", "parameters": {
                "strict_required_status_checks_policy": false,
                "required_status_checks": [
                    {"context": "ci", "integration_id": 15368},
                    {"context": "build"}
                ]
            }},
            {"type": "pull_request", "parameters": {
                "required_approving_review_count": 1,
                "require_code_owner_review": true,
                "dismiss_stale_reviews_on_push": false
            }}
        ]);
        let (checks, reviews) = rules_of(&rules);
        assert_eq!(
            checks,
            vec![
                RequiredCheck::from("build"),
                RequiredCheck {
                    context: "ci".into(),
                    app_id: Some(15368)
                },
            ]
        );
        assert_eq!(
            reviews,
            ReviewPolicy {
                approvals: 1,
                code_owners: true,
                dismiss_stale: false
            }
        );
        // a branch no ruleset applies to adds nothing
        assert_eq!(rules_of(&json!([])), (Vec::new(), ReviewPolicy::default()));
        // the two sources union: a context either requires, the stricter review rule of each
        let (pc, pr) = protection_of(&json!({
            "required_status_checks": {"contexts": ["ci", "lint"]},
            "required_pull_request_reviews": {"required_approving_review_count": 2}
        }));
        let (rc, rr) = rules_of(&rules);
        assert_eq!(
            union_checks(pc, rc),
            vec![
                RequiredCheck::from("build"),
                RequiredCheck {
                    context: "ci".into(),
                    app_id: Some(15368)
                },
                RequiredCheck::from("lint"),
            ]
        );
        assert_eq!(
            union_reviews(pr, rr),
            ReviewPolicy {
                approvals: 2,
                code_owners: true,
                dismiss_stale: false
            }
        );
    }

    /// The app is read where a forge read names it, and nowhere else: `gh pr list` names
    /// none, so a check run it lists has no writer, and neither has a status context.
    #[test]
    fn a_check_entry_records_what_wrote_it_and_when_it_completed() {
        let p = pull_request_of(&json!({
            "number": 3,
            "statusCheckRollup": [
                {"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS",
                 "startedAt": "2026-09-01T00:00:00Z", "completedAt": "2026-09-01T00:05:00Z",
                 "checkSuite": {"app": {"databaseId": 15368}}},
                {"__typename": "CheckRun", "name": "ci", "status": "IN_PROGRESS",
                 "startedAt": "2026-09-01T00:06:00Z", "completedAt": "0001-01-01T00:00:00Z"},
                {"__typename": "StatusContext", "context": "ci", "state": "SUCCESS",
                 "startedAt": "2026-09-01T00:07:00Z"},
                // exactly the keys gh's own projection of the rollup carries
                {"__typename": "CheckRun", "completedAt": "2026-09-01T00:05:00Z",
                 "conclusion": "SUCCESS", "detailsUrl": "https://example.com/run/1", "name": "ci",
                 "startedAt": "2026-09-01T00:00:00Z", "status": "COMPLETED",
                 "workflowName": "validate"},
                {"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS",
                 "checkSuite": {"app": null}},
                // a shape no forge read produces names no writer
                {"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS",
                 "app": {"databaseId": 15368}}
            ]
        }))
        .unwrap();
        assert_eq!(p.checks[0].kind, CheckKind::CheckRun);
        assert_eq!(p.checks[0].app_id, Some(15368));
        assert_eq!(p.checks[0].completed_at, "2026-09-01T00:05:00Z");
        assert_eq!(
            p.checks[1].completed_at, "2026-09-01T00:06:00Z",
            "a run that has not completed is placed by when it started"
        );
        assert_eq!(p.checks[2].kind, CheckKind::StatusContext);
        assert_eq!(p.checks[2].app_id, None);
        assert_eq!(p.checks[2].completed_at, "2026-09-01T00:07:00Z");
        assert_eq!(p.checks[3].state, CheckRunState::Passed);
        assert_eq!(
            p.checks[3].app_id, None,
            "gh pr list does not say who wrote a check run"
        );
        assert_eq!(p.checks[4].app_id, None, "a suite without an app");
        assert_eq!(
            p.checks[5].app_id, None,
            "the app is the suite's, or unread"
        );
        let alone = check_of(&json!({
            "__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS",
            "completedAt": "2026-09-01T00:05:00Z", "checkSuite": {"app": {"databaseId": 15368}}
        }));
        assert_eq!(p.checks[0], alone, "one parser reads both rollups");
    }

    /// Two sources binding one context to two apps require both apps' runs; an unbound entry
    /// beside a bound one is the same requirement, and stands alone only when nothing binds it.
    #[test]
    fn a_context_bound_to_two_apps_is_two_requirements() {
        let bound = |app: u64| RequiredCheck {
            context: "ci".into(),
            app_id: Some(app),
        };
        let (pc, _) = protection_of(&json!({
            "required_status_checks": {
                "contexts": ["ci", "lint"],
                "checks": [{"context": "ci", "app_id": 15368}, {"context": "lint", "app_id": -1}]
            }
        }));
        assert_eq!(pc, vec![bound(15368), RequiredCheck::from("lint")]);
        let (rc, _) = rules_of(&json!([
            {"type": "required_status_checks", "parameters": {"required_status_checks": [
                {"context": "ci", "integration_id": 99},
                {"context": "ci", "integration_id": 15368},
                {"context": "lint"}
            ]}}
        ]));
        assert_eq!(
            rc,
            vec![bound(99), bound(15368), RequiredCheck::from("lint")]
        );
        assert_eq!(
            union_checks(pc, rc),
            vec![bound(99), bound(15368), RequiredCheck::from("lint")],
            "each app once, and the unbound context once"
        );
        assert_eq!(
            union_checks(vec!["ci".into()], vec![bound(7)]),
            vec![bound(7)],
            "a source that names no app does not unbind the other's"
        );
    }

    fn run_node(name: &str, conclusion: &str, app: Value) -> Value {
        json!({"__typename": "CheckRun", "name": name, "status": "COMPLETED",
               "conclusion": conclusion, "startedAt": "2026-09-01T00:00:00Z",
               "completedAt": "2026-09-01T00:05:00Z", "checkSuite": {"app": app}})
    }

    fn pr_node(number: u64, head: &str, more: Value, cursor: Value, nodes: Vec<Value>) -> Value {
        json!({"number": number, "headRefOid": head, "statusCheckRollup": {"contexts": {
            "pageInfo": {"hasNextPage": more, "endCursor": cursor}, "nodes": nodes}}})
    }

    fn writers_page(more: bool, cursor: Value, nodes: Vec<Value>) -> Value {
        json!({"data": {"repository": {"pullRequests": {
            "pageInfo": {"hasNextPage": more, "endCursor": cursor}, "nodes": nodes}}}})
    }

    fn rest_page(head: &str, more: Value, cursor: Value, nodes: Vec<Value>) -> Value {
        json!({"data": {"repository": {"pullRequest": pr_node(1, head, more, cursor, nodes)}}})
    }

    #[test]
    fn the_writers_page_is_read_per_pull_request() {
        let page = writers_page(
            false,
            Value::Null,
            vec![
                pr_node(
                    1,
                    "h1",
                    json!(false),
                    Value::Null,
                    vec![
                        run_node("ci", "SUCCESS", json!({"databaseId": 15368})),
                        run_node("ci", "FAILURE", json!({"databaseId": 99})),
                        json!({"__typename": "StatusContext", "context": "legacy",
                               "state": "SUCCESS", "startedAt": "2026-09-01T00:07:00Z"}),
                    ],
                ),
                json!({"number": 2, "headRefOid": "h2", "statusCheckRollup": null}),
                pr_node(3, "h3", json!(true), json!("c3"), vec![]),
                json!({"headRefOid": "h4", "statusCheckRollup": null}),
                // contexts listed without a word on whether more follow are not all of them
                json!({"number": 5, "headRefOid": "h5", "statusCheckRollup": {"contexts": {"nodes": []}}}),
            ],
        );
        let read = attributed_checks_of(&page);
        assert_eq!(read.keys().copied().collect::<Vec<_>>(), [1, 2]);
        let (head, checks) = &read[&1];
        assert_eq!(head, "h1");
        assert_eq!(
            checks.iter().map(|c| c.app_id).collect::<Vec<_>>(),
            [Some(15368), Some(99), None],
            "the app is on the runs only"
        );
        assert_eq!(checks[1].state, CheckRunState::Failed);
        assert_eq!(
            (checks[2].name.as_str(), checks[2].kind),
            ("legacy", CheckKind::StatusContext)
        );
        assert_eq!(
            checks[2].completed_at, "2026-09-01T00:07:00Z",
            "a status context keeps the time it was set"
        );
        assert_eq!(
            read[&2],
            ("h2".to_string(), Vec::new()),
            "no rollup: no check"
        );
        assert_eq!(truncated_of(&page).into_iter().collect::<Vec<_>>(), [3, 5]);
        assert!(attributed_checks_of(&json!({})).is_empty());
        assert!(truncated_of(&json!({})).is_empty());

        let third = rollup_page_of(&pr_node(3, "h3", json!(true), json!("c3"), vec![]));
        assert_eq!(
            (third.complete, third.cursor.as_deref()),
            (false, Some("c3"))
        );
        assert!(rollup_page_of(&Value::Null).complete, "nothing listed");
        assert_eq!(rollup_page_of(&Value::Null).head, "");

        assert_eq!(
            next_cursor(&writers_page(true, json!("c1"), vec![])).as_deref(),
            Some("c1")
        );
        assert_eq!(next_cursor(&writers_page(false, json!("c1"), vec![])), None);
        assert_eq!(next_cursor(&writers_page(true, Value::Null, vec![])), None);
        assert_eq!(next_cursor(&json!({"data": {}})), None);
    }

    #[test]
    fn every_page_of_writers_is_asked_for_once() {
        let ci = || vec![run_node("ci", "SUCCESS", json!({"databaseId": 15368}))];
        let wanted: BTreeSet<u64> = [1, 2].into_iter().collect();
        let none = |_: u64, _: Option<&str>| -> Result<Value, ForgeError> {
            Err(ForgeError("no rollup was truncated".into()))
        };
        let pages = |after: Option<&str>| match after {
            None => writers_page(
                true,
                json!("c1"),
                vec![pr_node(2, "h2", json!(false), Value::Null, ci())],
            ),
            _ => writers_page(
                false,
                Value::Null,
                vec![pr_node(1, "h1", json!(false), Value::Null, ci())],
            ),
        };
        let mut asked: Vec<Option<String>> = Vec::new();
        let read = attributed_checks(
            &wanted,
            OPEN_LIMIT,
            |after| {
                asked.push(after.map(str::to_string));
                Ok(pages(after))
            },
            none,
        )
        .unwrap();
        assert_eq!(asked, [None, Some("c1".to_string())]);
        assert_eq!(read.keys().copied().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(read[&1].1[0].app_id, Some(15368));

        // the newest are listed first: once every observed number was seen, nothing more is asked
        let mut calls = 0;
        let newest: BTreeSet<u64> = [2].into_iter().collect();
        let read = attributed_checks(
            &newest,
            OPEN_LIMIT,
            |after| {
                calls += 1;
                Ok(pages(after))
            },
            none,
        )
        .unwrap();
        assert_eq!((calls, read.contains_key(&2)), (1, true));

        // nothing observed, nothing asked
        let read = attributed_checks(
            &BTreeSet::new(),
            OPEN_LIMIT,
            |_| Err(ForgeError("asked".into())),
            none,
        )
        .unwrap();
        assert!(read.is_empty());

        // a page the forge refuses fails the read, whatever the pages before it said
        let refused = attributed_checks(
            &wanted,
            OPEN_LIMIT,
            |after| match after {
                None => Ok(pages(None)),
                _ => Err(ForgeError("HTTP 403".into())),
            },
            none,
        )
        .unwrap_err();
        assert_eq!(refused.0, "HTTP 403");

        // a limit of one page stops after it, though the forge has more
        let mut calls = 0;
        let read = attributed_checks(
            &wanted,
            1,
            |after| {
                calls += 1;
                Ok(pages(after))
            },
            none,
        )
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(
            read.keys().copied().collect::<Vec<_>>(),
            [2],
            "#1 is unread"
        );
    }

    #[test]
    fn a_rollup_that_does_not_fit_a_page_is_read_whole_or_not_at_all() {
        let ci = |app: u64| run_node("ci", "SUCCESS", json!({"databaseId": app}));
        let wanted: BTreeSet<u64> = [1].into_iter().collect();
        // #1's contexts did not fit; #7 is truncated too, and nobody observed it
        let first = || {
            writers_page(
                false,
                Value::Null,
                vec![
                    pr_node(1, "h1", json!(true), json!("x1"), vec![ci(15368)]),
                    pr_node(7, "h7", json!(true), json!("x7"), vec![]),
                ],
            )
        };
        let mut asked: Vec<(u64, Option<String>)> = Vec::new();
        let read = attributed_checks(
            &wanted,
            OPEN_LIMIT,
            |_| Ok(first()),
            |number, after| {
                asked.push((number, after.map(str::to_string)));
                Ok(match after {
                    None => rest_page("h1", json!(true), json!("x1"), vec![ci(15368)]),
                    _ => rest_page("h1", json!(false), Value::Null, vec![ci(99)]),
                })
            },
        )
        .unwrap();
        assert_eq!(asked, [(1, None), (1, Some("x1".to_string()))]);
        let (head, checks) = &read[&1];
        assert_eq!(head, "h1");
        assert_eq!(
            checks.iter().map(|c| c.app_id).collect::<Vec<_>>(),
            [Some(15368), Some(99)],
            "every page of its contexts, in order"
        );

        // each way the whole cannot be read leaves it unread, never shorter
        let unread = |answer: &dyn Fn(Option<&str>) -> Value| {
            attributed_checks(
                &wanted,
                OPEN_LIMIT,
                |_| Ok(first()),
                |_, after| Ok(answer(after)),
            )
            .unwrap()
        };
        let moved = unread(&|after: Option<&str>| match after {
            None => rest_page("h1", json!(true), json!("x1"), vec![ci(15368)]),
            _ => rest_page("h1b", json!(false), Value::Null, vec![]),
        });
        assert!(moved.is_empty(), "the head moved between two pages");
        let no_cursor = unread(&|_| rest_page("h1", json!(true), Value::Null, vec![ci(15368)]));
        assert!(
            no_cursor.is_empty(),
            "more follow, and no cursor says where"
        );
        let endless = unread(&|_| rest_page("h1", json!(true), json!("x"), vec![ci(15368)]));
        assert!(endless.is_empty(), "more pages than are read");
        let gone = unread(&|_| json!({"data": {"repository": {"pullRequest": null}}}));
        assert_eq!(
            gone[&1],
            (String::new(), Vec::new()),
            "no head: nothing to match"
        );

        // a page of contexts the forge refuses fails the read
        let refused = attributed_checks(
            &wanted,
            OPEN_LIMIT,
            |_| Ok(first()),
            |_, _| Err(ForgeError("HTTP 502".into())),
        )
        .unwrap_err();
        assert_eq!(refused.0, "HTTP 502");

        let mut pages = 0;
        let whole = whole_rollup(|_| {
            pages += 1;
            Ok(rest_page("h1", json!(true), json!("x"), vec![]))
        })
        .unwrap();
        assert_eq!((whole, pages), (None, CONTEXT_PAGES));
    }

    #[test]
    fn only_the_observed_head_is_attributed() {
        let listed = |number: u64, head: &str| {
            pull_request_of(&json!({
                "number": number, "headRefOid": head,
                "statusCheckRollup": [{"__typename": "CheckRun", "name": "ci",
                    "status": "COMPLETED", "conclusion": "SUCCESS"}]
            }))
            .unwrap()
        };
        let mut prs = vec![
            listed(1, "h1"),
            listed(2, "h2"),
            listed(3, "h3"),
            listed(4, ""),
        ];
        let its = |app: u64| {
            vec![check_of(&run_node(
                "ci",
                "SUCCESS",
                json!({"databaseId": app}),
            ))]
        };
        let mut read = AttributedChecks::new();
        read.insert(1, ("h1".to_string(), its(15368)));
        // #2 moved between the two reads; #3 was not read; #4 was listed without a head
        read.insert(2, ("h2-moved".to_string(), its(15368)));
        read.insert(4, (String::new(), its(15368)));
        attribute(&mut prs, &read);
        assert_eq!(prs[0].checks[0].app_id, Some(15368));
        for unattributed in &prs[1..] {
            assert_eq!(unattributed.checks.len(), 1, "#{}", unattributed.number);
            assert_eq!(
                unattributed.checks[0].app_id, None,
                "#{} keeps the checks it was listed with",
                unattributed.number
            );
        }
    }

    /// The owner, the name and the cursor go as strings, whatever they look like: only the
    /// number is typed.
    #[test]
    fn the_writers_read_passes_names_as_strings() {
        let first = writers_args(WRITERS_QUERY, "2048", "true", "n=50", None);
        assert_eq!(first[..3], ["api", "graphql", "-f"]);
        assert_eq!(first[3], format!("query={WRITERS_QUERY}"));
        assert_eq!(
            first[4..],
            ["-f", "owner=2048", "-f", "name=true", "-F", "n=50"]
        );
        let next = writers_args(WRITERS_OF_QUERY, "o", "r", "number=7", Some("@c1"));
        assert_eq!(next[8..], ["-F", "number=7", "-f", "after=@c1"]);
        assert!(next[3].starts_with("query=query($owner:String!,$name:String!,$number:Int!"));
        // the list this read attributes is newest first, and so is the read
        assert!(WRITERS_QUERY.contains("orderBy:{field:CREATED_AT,direction:DESC}"));
        assert!(!WRITERS_QUERY.contains('\n') && !WRITERS_OF_QUERY.contains('\n'));
        for query in [WRITERS_QUERY, WRITERS_OF_QUERY] {
            assert!(query.contains("checkSuite{app{databaseId}}"), "{query}");
            assert!(query.contains("startedAt:createdAt"), "{query}");
            assert!(query.contains("__typename ...on CheckRun"), "{query}");
        }
    }

    #[test]
    fn the_latest_reviews_are_read_with_the_commit_each_was_given_on() {
        let p = pull_request_of(&json!({
            "number": 4,
            "latestReviews": [
                {"author": {"login": "ana"}, "state": "APPROVED", "commit": {"oid": "c1"}},
                {"author": {"login": "bo"}, "state": "changes_requested", "commit": {"oid": "c2"}},
                {"author": {"login": "cy"}, "state": "COMMENTED"}
            ],
            "reviewRequests": [{"login": "dee"}, {"slug": "core"}, {"__typename": "Bot"}]
        }))
        .unwrap();
        assert_eq!(p.latest_reviews.len(), 3);
        assert_eq!(p.latest_reviews[0].commit, "c1");
        assert_eq!(p.latest_reviews[1].state, "CHANGES_REQUESTED");
        assert_eq!(
            p.latest_reviews[2].commit, "",
            "a review without a commit says so"
        );
        assert_eq!(p.review_requests, ["dee", "core"]);
    }

    /// Each way a merged pull request is not a branch left behind, and the one way it is.
    #[test]
    fn only_a_same_repository_branch_at_its_merged_head_is_left_behind() {
        let heads = remote_heads_of(
            "aa\trefs/heads/fix/a\nbb\trefs/heads/fix/b\ncc\trefs/heads/master\nff\trefs/tags/v1\n\
             dd\trefs/heads/fix/fork\nee\trefs/heads/fix/dup\nbad line\n\trefs/heads/x\n",
        );
        assert_eq!(heads.len(), 5, "{heads:?}");
        let m = |n: u64, state: &str, branch: &str, head: &str, fork: bool, at: &str| {
            json!({"number": n, "state": state, "headRefName": branch, "headRefOid": head,
                   "isCrossRepository": fork, "mergedAt": at})
        };
        let merged = json!([
            m(1, "MERGED", "fix/a", "aa", false, "t"),
            m(2, "MERGED", "fix/b", "b0", false, "t"),
            m(3, "CLOSED", "fix/a", "aa", false, "t"),
            m(4, "MERGED", "fix/fork", "dd", true, "t"),
            m(5, "MERGED", "master", "cc", false, "t"),
            m(6, "MERGED", "fix/a", "aa", false, ""),
            m(7, "MERGED", "", "aa", false, "t"),
            m(8, "MERGED", "fix/dup", "ee", false, "t"),
            m(9, "MERGED", "fix/dup", "ee", false, "t"),
            m(10, "MERGED", "fix/dup", "ee", false, "t"),
            json!({"state": "MERGED", "headRefName": "fix/a", "headRefOid": "aa"}),
            json!({"number": 11, "state": "MERGED", "headRefName": "fix/a", "headRefOid": "aa",
                   "mergedAt": "t"}),
        ]);
        let left = merged_branches_of(&merged, &heads, "master");
        let got: Vec<(&str, u64)> = left.iter().map(|b| (b.branch.as_str(), b.pr)).collect();
        assert_eq!(got, [("fix/a", 1), ("fix/dup", 10)], "{left:?}");
        // the newest number names a branch whatever order the forge listed them in
        let reversed = json!([
            m(10, "MERGED", "fix/dup", "ee", false, "t"),
            m(8, "MERGED", "fix/dup", "ee", false, "t")
        ]);
        assert_eq!(merged_branches_of(&reversed, &heads, "master")[0].pr, 10);
        assert!(merged_branches_of(&json!({}), &heads, "master").is_empty());
    }

    /// The read's two answers that need no forge: origin unreadable is unread, and an origin
    /// serving only the base has left nothing behind, without asking for merged pull requests.
    #[test]
    fn origin_decides_before_the_forge_is_asked() {
        let git = |dir: &Path, args: &[&str]| {
            let out = Command::new("git")
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {out:?}");
        };
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        git(base, &["init", "-q", "work"]);
        let work = base.join("work");
        assert_eq!(merged_branches(&work, "master"), None, "no origin: unread");
        git(
            base,
            &["init", "-q", "--bare", "-b", "master", "origin.git"],
        );
        git(
            &work,
            &[
                "remote",
                "add",
                "origin",
                base.join("origin.git").to_str().unwrap(),
            ],
        );
        git(&work, &["commit", "-q", "--allow-empty", "-m", "base"]);
        git(&work, &["push", "-q", "origin", "HEAD:refs/heads/master"]);
        assert_eq!(merged_branches(&work, "master"), Some(Vec::new()));
        // a branch besides the base: the merged pull requests are asked for, and a forge that
        // cannot list them here (no GitHub remote, or no gh at all) leaves the report unread
        git(&work, &["push", "-q", "origin", "HEAD:refs/heads/fix/a"]);
        let asked = merged_branches(&work, "master");
        assert!(
            asked.as_ref().is_none_or(|v| v.is_empty()),
            "no merged pull request of this origin can name fix/a: {asked:?}"
        );
    }

    #[test]
    fn the_helpers_name_what_refused() {
        assert!(ls_remote_could_not_run(std::io::Error::other("gone")).contains("could not run"));
        assert_eq!(
            listed((true, "[1]".into(), String::new())),
            Some(json!([1]))
        );
        assert_eq!(listed((true, "not json".into(), String::new())), None);
        assert_eq!(listed((false, "[1]".into(), "refused".into())), None);
        let heads = remote_heads_of("aa\trefs/heads/fix/a\ncc\trefs/heads/master\n");
        let merged = json!([{"number": 1, "state": "MERGED", "headRefName": "fix/a",
            "headRefOid": "aa", "isCrossRepository": false, "mergedAt": "t"}]);
        assert_eq!(
            merged_branches_given(&heads, "master", || Some(merged)).map(|v| v.len()),
            Some(1)
        );
    }

    /// A pull request source of a cross-reference, with everything the read asks of it.
    fn source(number: u64, state: &str, body: &str, association: &str, fork: bool) -> Value {
        json!({"__typename": "PullRequest", "number": number, "state": state, "body": body,
               "headRefOid": format!("h{number}"), "mergeCommit": {"oid": format!("m{number}")},
               "isCrossRepository": fork, "authorAssociation": association,
               "author": {"login": "ana"}, "baseRefName": "master", "changedFiles": 3})
    }

    /// One cross-reference from this repository.
    fn mention(source: Value) -> Value {
        json!({"isCrossRepository": false, "source": source})
    }

    /// One open pull request node of a declarations answer.
    fn declared_node(number: u64, more: Value, cursor: Value, events: Vec<Value>) -> Value {
        json!({"number": number, "authorAssociation": "OWNER", "isCrossRepository": false,
            "timelineItems": {
                "pageInfo": {"hasNextPage": more, "endCursor": cursor}, "nodes": events}})
    }

    fn references_page(more: Value, cursor: Value, events: Vec<Value>) -> Value {
        json!({"data": {"repository": {"pullRequest": declared_node(1, more, cursor, events)}}})
    }

    #[test]
    fn a_resolved_pull_request_is_read_with_who_declared_and_where_it_merged() {
        let (number, read) =
            resolved_of(&source(7, "MERGED", "Supersedes #1", "COLLABORATOR", false)).unwrap();
        assert_eq!(number, 7);
        assert_eq!(
            read,
            ResolvedPullRequest {
                merged: true,
                head_sha: "h7".into(),
                body: "Supersedes #1".into(),
                merge_commit: "m7".into(),
                author: "ana".into(),
                author_association: "COLLABORATOR".into(),
                cross_repository: false,
                base_ref: "master".into(),
                changed_files: 3,
            }
        );
        // what a reading does not say fails closed
        let bare = ResolvedPullRequest {
            merged: true,
            head_sha: "h".into(),
            body: String::new(),
            merge_commit: String::new(),
            author: String::new(),
            author_association: String::new(),
            cross_repository: true,
            base_ref: String::new(),
            changed_files: 0,
        };
        let (_, read) =
            resolved_of(&json!({"number": 7, "state": "MERGED", "headRefOid": "h"})).unwrap();
        assert_eq!(read, bare);
        let (_, read) = resolved_of(&json!({"number": 7, "state": "MERGED", "headRefOid": "h",
            "mergeCommit": null, "author": null, "authorAssociation": null,
            "isCrossRepository": null, "baseRefName": null, "changedFiles": null}))
        .unwrap();
        assert_eq!(read, bare, "a null is not an answer");
        // and so does a record that predates the fields
        let recorded: ResolvedPullRequest =
            serde_json::from_str(r#"{"merged":true,"head_sha":"h"}"#).unwrap();
        assert_eq!(
            recorded, bare,
            "a record that does not say is read as a fork"
        );
        assert!(unread_is_a_fork());
    }

    #[test]
    fn a_pull_request_the_list_names_has_no_reference_read() {
        let listed = pull_request_of(&json!({"number": 1, "authorAssociation": "OWNER"})).unwrap();
        assert_eq!(
            listed.author_association, "",
            "gh pr list does not report it"
        );
        assert_eq!(listed.cross_references, CrossReferenceRead::Unread);
        // a record written before either was read says the same
        let mut record = serde_json::to_value(&listed).unwrap();
        let fields = record.as_object_mut().unwrap();
        assert_eq!(fields.remove("cross_references"), Some(json!("unread")));
        assert_eq!(fields.remove("author_association"), Some(json!("")));
        let older: PullRequestObservation = serde_json::from_value(record).unwrap();
        assert_eq!(older, listed);
        let words: Vec<Value> = [
            CrossReferenceRead::Unread,
            CrossReferenceRead::Truncated,
            CrossReferenceRead::Whole,
        ]
        .iter()
        .map(|w| serde_json::to_value(w).unwrap())
        .collect();
        assert_eq!(words, [json!("unread"), json!("truncated"), json!("whole")]);
        let whole = read_whole(listed);
        assert_eq!(
            (whole.author_association.as_str(), whole.cross_references),
            ("OWNER", CrossReferenceRead::Whole)
        );
    }

    #[test]
    fn a_reference_page_is_read_per_pull_request() {
        let kept = source(2, "MERGED", "Supersedes #1", "OWNER", false);
        let node = declared_node(
            1,
            json!(false),
            json!("c9"),
            vec![
                mention(kept.clone()),
                // an issue mentions it: no number, no declaration
                mention(json!({"__typename": "Issue"})),
                // written in another repository: its #1 is not this #1
                json!({"isCrossRepository": true,
                       "source": source(3, "MERGED", "Supersedes #1", "OWNER", false)}),
                // an event that does not say where it came from is not taken to be from here
                json!({"source": source(4, "MERGED", "Supersedes #1", "OWNER", false)}),
                json!({"isCrossRepository": false}),
                json!({}),
            ],
        );
        let page = reference_page_of(&node);
        assert_eq!(page.association, "OWNER");
        assert_eq!(page.cross_repository, Some(false));
        assert_eq!(page.sources, [kept]);
        assert_eq!((page.complete, page.cursor.as_deref()), (true, Some("c9")));
        let more = reference_page_of(&declared_node(1, json!(true), json!("c1"), vec![]));
        assert_eq!((more.complete, more.cursor.as_deref()), (false, Some("c1")));
        let unsaid = reference_page_of(&json!({"number": 1, "timelineItems": {"nodes": []}}));
        assert!(
            !unsaid.complete,
            "not saying whether more follow is not `no more`"
        );
        assert_eq!((unsaid.association.as_str(), unsaid.cursor), ("", None));
        assert_eq!(unsaid.cross_repository, None, "not said is not `here`");
        let null = reference_page_of(&json!({"number": 1, "isCrossRepository": null}));
        assert_eq!(null.cross_repository, None, "a null is not an answer");
        for unlisted in [
            json!({"number": 1}),
            json!({"number": 1, "timelineItems": null}),
            Value::Null,
        ] {
            let page = reference_page_of(&unlisted);
            assert!(
                !page.complete,
                "what was not listed was not read: {unlisted}"
            );
            assert!(page.sources.is_empty());
        }
    }

    #[test]
    fn every_page_of_declarations_is_asked_for_once() {
        let wanted: BTreeSet<u64> = [1, 2].into_iter().collect();
        let none = |number: u64, _: Option<&str>| -> Result<Value, ForgeError> {
            panic!("#{number} was asked for alone")
        };
        let declarer = || mention(source(7, "MERGED", "Supersedes #1", "OWNER", false));
        let pages = |after: Option<&str>| match after {
            None => writers_page(
                true,
                json!("c1"),
                vec![declared_node(2, json!(false), Value::Null, vec![])],
            ),
            _ => writers_page(
                false,
                Value::Null,
                vec![
                    declared_node(1, json!(false), Value::Null, vec![declarer()]),
                    json!({"authorAssociation": "OWNER"}),
                ],
            ),
        };
        let mut asked: Vec<Option<String>> = Vec::new();
        let read = declarations(
            &wanted,
            OPEN_LIMIT,
            |after| {
                asked.push(after.map(str::to_string));
                Ok(pages(after))
            },
            none,
        )
        .unwrap();
        assert_eq!(asked, [None, Some("c1".to_string())]);
        assert_eq!(read.keys().copied().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(read[&1].read, CrossReferenceRead::Whole);
        assert_eq!(read[&1].association, "OWNER");
        assert_eq!(read[&1].cross_repository, Some(false));
        assert_eq!(read[&1].sources.len(), 1);
        assert!(read[&2].sources.is_empty());

        // the newest are listed first: once every observed number was seen, nothing more is asked
        let mut calls = 0;
        let newest: BTreeSet<u64> = [2].into_iter().collect();
        let read = declarations(
            &newest,
            OPEN_LIMIT,
            |after| {
                calls += 1;
                Ok(pages(after))
            },
            none,
        )
        .unwrap();
        assert_eq!((calls, read.contains_key(&2)), (1, true));

        // nothing open, nothing asked
        let read = declarations(
            &BTreeSet::new(),
            OPEN_LIMIT,
            |_| panic!("a page was asked for"),
            none,
        )
        .unwrap();
        assert!(read.is_empty());
    }

    #[test]
    fn one_no_page_listed_is_asked_for_alone_and_a_refusal_fails_the_read() {
        let wanted: BTreeSet<u64> = [1, 2].into_iter().collect();
        let declarer = || mention(source(7, "MERGED", "Supersedes #1", "OWNER", false));
        let first = || {
            writers_page(
                true,
                json!("c1"),
                vec![declared_node(2, json!(false), Value::Null, vec![])],
            )
        };
        let alone = || references_page(json!(false), Value::Null, vec![declarer()]);

        // a limit of one page stops the paging, though the forge has more: #1 is asked for alone
        let (mut calls, mut asked) = (0, Vec::new());
        let read = declarations(
            &wanted,
            1,
            |_| {
                calls += 1;
                Ok(first())
            },
            |number, after| {
                asked.push((number, after.map(str::to_string)));
                Ok(alone())
            },
        )
        .unwrap();
        assert_eq!((calls, asked), (1, vec![(1, None)]));
        assert_eq!(read[&1].read, CrossReferenceRead::Whole);
        assert_eq!(read[&1].sources.len(), 1);

        // a page the forge refuses fails the read: nothing is asked for alone to stand in
        // for it, and what the pages before it said is no observation
        let mut asked = Vec::new();
        let failed = declarations(
            &wanted,
            OPEN_LIMIT,
            |after| match after {
                None => Ok(first()),
                _ => Err(ForgeError("HTTP 403".into())),
            },
            |number, _| {
                asked.push(number);
                Ok(alone())
            },
        );
        assert_eq!(failed.unwrap_err().0, "HTTP 403");
        assert!(asked.is_empty(), "a failed page is not papered over");

        // one asked for alone that the forge refuses fails the read too, whatever was read
        // of the others
        let failed = declarations(
            &wanted,
            1,
            |_| Ok(first()),
            |_, _| Err(ForgeError("HTTP 502".into())),
        );
        assert_eq!(failed.unwrap_err().0, "HTTP 502");
    }

    #[test]
    fn a_timeline_that_does_not_fit_a_page_is_read_whole_or_held() {
        let wanted: BTreeSet<u64> = [1].into_iter().collect();
        let declarer = |n: u64| mention(source(n, "MERGED", "Supersedes #1", "OWNER", false));
        // #1's cross-references did not fit, though the page shows one that would close it;
        // #7 is truncated too, and nobody observed it
        let first = || {
            writers_page(
                false,
                Value::Null,
                vec![
                    declared_node(1, json!(true), json!("x1"), vec![declarer(5)]),
                    declared_node(7, json!(true), json!("x7"), vec![]),
                ],
            )
        };
        let mut asked: Vec<(u64, Option<String>)> = Vec::new();
        let read = declarations(
            &wanted,
            OPEN_LIMIT,
            |_| Ok(first()),
            |number, after| {
                asked.push((number, after.map(str::to_string)));
                Ok(match after {
                    None => references_page(json!(true), json!("x1"), vec![declarer(5)]),
                    _ => references_page(json!(false), Value::Null, vec![declarer(6)]),
                })
            },
        )
        .unwrap();
        assert_eq!(
            asked,
            [(1, None), (1, Some("x1".to_string()))],
            "#7 was not observed, and is not asked again"
        );
        assert_eq!(read[&1].read, CrossReferenceRead::Whole);
        let numbers: Vec<u64> = read[&1]
            .sources
            .iter()
            .filter_map(|s| s.get("number").and_then(Value::as_u64))
            .collect();
        assert_eq!(numbers, [5, 6], "every page of its references, in order");
        assert_eq!(read[&7].read, CrossReferenceRead::Truncated);

        // each way the whole cannot be read holds it, with no source at all and with who its
        // author is, which a page did say
        let asked_alone = |answer: &dyn Fn(Option<&str>) -> Result<Value, ForgeError>| {
            let mut pages = 0;
            let read = declarations(
                &wanted,
                OPEN_LIMIT,
                |_| Ok(first()),
                |_, after| {
                    pages += 1;
                    answer(after)
                },
            );
            (pages, read)
        };
        let held = |answer: &dyn Fn(Option<&str>) -> Result<Value, ForgeError>| {
            let (pages, read) = asked_alone(answer);
            let read = read.unwrap();
            assert_eq!(
                (read[&1].association.as_str(), read[&1].cross_repository),
                ("OWNER", Some(false)),
                "who its author is was still read"
            );
            assert!(
                read[&1].sources.is_empty(),
                "a part of the references is not the references"
            );
            (pages, read[&1].read)
        };
        let more = |cursor: Value| Ok(references_page(json!(true), cursor, vec![declarer(5)]));
        assert_eq!(
            held(&|_| more(Value::Null)),
            (1, CrossReferenceRead::Truncated),
            "more follow, and no cursor says where"
        );
        assert_eq!(
            held(&|_| more(json!("x"))),
            (REFERENCE_PAGES, CrossReferenceRead::Truncated),
            "more pages than are read"
        );
        // what the forge no longer shows is unread, not truncated: a refresh may read it
        assert_eq!(
            held(&|_| Ok(json!({"data": {"repository": {"pullRequest": null}}}))),
            (1, CrossReferenceRead::Unread),
            "a pull request the forge no longer shows was not read"
        );
        // a page of references the forge refuses is neither: the read failed, on whichever
        // page, and nothing is held on a part of it
        let (pages, failed) = asked_alone(&|_| Err(ForgeError("HTTP 502".into())));
        assert_eq!((pages, failed.unwrap_err().0.as_str()), (1, "HTTP 502"));
        let (pages, failed) = asked_alone(&|after| match after {
            None => more(json!("x")),
            _ => Err(ForgeError("HTTP 503".into())),
        });
        assert_eq!((pages, failed.unwrap_err().0.as_str()), (2, "HTTP 503"));
    }

    #[test]
    fn only_a_pull_request_the_read_names_is_marked() {
        let listed = |number: u64| pull_request_of(&json!({"number": number})).unwrap();
        let mut prs = vec![listed(1), listed(2), listed(3)];
        let mut read = Declarations::new();
        // read out of order, and of one nobody observed
        read.insert(
            2,
            DeclarationRead {
                association: "CONTRIBUTOR".into(),
                cross_repository: Some(false),
                sources: vec![json!({"number": 8})],
                read: CrossReferenceRead::Whole,
            },
        );
        read.insert(
            1,
            DeclarationRead {
                association: "OWNER".into(),
                cross_repository: Some(false),
                sources: vec![json!({"number": 7}), json!({"number": 9})],
                read: CrossReferenceRead::Whole,
            },
        );
        read.insert(
            40,
            DeclarationRead {
                association: "OWNER".into(),
                cross_repository: Some(false),
                sources: vec![json!({"number": 41})],
                read: CrossReferenceRead::Whole,
            },
        );
        let sources = declare(&mut prs, &read);
        assert_eq!(
            sources,
            json!([{"number": 7}, {"number": 9}, {"number": 8}]),
            "every source of every observed one, in number order"
        );
        let said: Vec<(&str, CrossReferenceRead)> = prs
            .iter()
            .map(|p| (p.author_association.as_str(), p.cross_references))
            .collect();
        assert_eq!(
            said,
            [
                ("OWNER", CrossReferenceRead::Whole),
                ("CONTRIBUTOR", CrossReferenceRead::Whole),
                ("", CrossReferenceRead::Unread)
            ],
            "#3 was not named: nobody's, and held"
        );
        // a truncated read marks it truncated, and never whole
        let mut read = Declarations::new();
        read.insert(
            3,
            DeclarationRead {
                association: "MEMBER".into(),
                cross_repository: Some(false),
                sources: Vec::new(),
                read: CrossReferenceRead::Truncated,
            },
        );
        assert_eq!(declare(&mut prs, &read), json!([]));
        assert_eq!(
            (prs[2].author_association.as_str(), prs[2].cross_references),
            ("MEMBER", CrossReferenceRead::Truncated)
        );
    }

    #[test]
    fn an_association_is_recorded_only_for_one_the_read_placed() {
        // (what the list said, what the declarations read said) of where the head lives
        let marked = |listed: Value, said: Option<bool>| {
            let mut prs =
                vec![pull_request_of(&json!({"number": 1, "isCrossRepository": listed})).unwrap()];
            let mut read = Declarations::new();
            read.insert(
                1,
                DeclarationRead {
                    association: "OWNER".into(),
                    cross_repository: said,
                    sources: Vec::new(),
                    read: CrossReferenceRead::Whole,
                },
            );
            declare(&mut prs, &read);
            let pr = prs.remove(0);
            assert_eq!(pr.cross_references, CrossReferenceRead::Whole);
            (pr.author_association, pr.cross_repository)
        };
        let owner = |fork: bool| ("OWNER".to_string(), fork);
        let nobody = (String::new(), false);
        assert_eq!(marked(json!(false), Some(false)), owner(false));
        // the list reads an absent or null flag as `this repository`: only the read's own
        // word that it lives here lets its author declare
        assert_eq!(marked(Value::Null, Some(false)), owner(false));
        assert_eq!(marked(json!(false), None), nobody, "absent: unauthorised");
        assert_eq!(marked(Value::Null, None), nobody, "absent twice");
        assert_eq!(marked(json!(false), Some(true)), nobody, "the two disagree");
        // a fork declares nothing whatever its association, so the word is kept as evidence,
        // and the list's own flag is left as it was read
        assert_eq!(marked(json!(true), Some(true)), owner(true));
        assert_eq!(marked(json!(true), None), owner(true));
        assert_eq!(marked(json!(true), Some(false)), owner(true));
    }

    #[test]
    fn the_forge_is_asked_exactly_this() {
        assert_eq!(
            DECLARATIONS_QUERY,
            "query($owner:String!,$name:String!,$n:Int!,$after:String){repository(owner:$owner,name:$name){pullRequests(states:OPEN,first:$n,after:$after,orderBy:{field:CREATED_AT,direction:DESC}){pageInfo{hasNextPage endCursor}nodes{number authorAssociation isCrossRepository timelineItems(first:100,itemTypes:[CROSS_REFERENCED_EVENT]){pageInfo{hasNextPage endCursor}nodes{...on CrossReferencedEvent{isCrossRepository source{__typename ...on PullRequest{number state body headRefOid mergeCommit{oid} isCrossRepository authorAssociation author{login} baseRefName changedFiles}}}}}}}}}"
        );
        assert_eq!(
            DECLARATIONS_OF_QUERY,
            "query($owner:String!,$name:String!,$number:Int!,$after:String){repository(owner:$owner,name:$name){pullRequest(number:$number){number authorAssociation isCrossRepository timelineItems(first:100,after:$after,itemTypes:[CROSS_REFERENCED_EVENT]){pageInfo{hasNextPage endCursor}nodes{...on CrossReferencedEvent{isCrossRepository source{__typename ...on PullRequest{number state body headRefOid mergeCommit{oid} isCrossRepository authorAssociation author{login} baseRefName changedFiles}}}}}}}}"
        );
        for query in [DECLARATIONS_QUERY, DECLARATIONS_OF_QUERY] {
            let (opened, closed) = (query.matches('{').count(), query.matches('}').count());
            assert_eq!(opened, closed, "{query}");
            assert!(!query.contains('\n'), "{query}");
        }
        assert_eq!(
            writers_args(DECLARATIONS_QUERY, "o", "r", "n=50", None),
            [
                "api",
                "graphql",
                "-f",
                format!("query={DECLARATIONS_QUERY}").as_str(),
                "-f",
                "owner=o",
                "-f",
                "name=r",
                "-F",
                "n=50"
            ]
        );
        assert_eq!(format!("n={DECLARATIONS_PAGE}"), "n=50");
        assert_eq!(
            RESOLVED_FIELDS,
            "number,state,headRefOid,body,mergeCommit,baseRefName,author,isCrossRepository,changedFiles"
        );
        assert_eq!(REFERENCE_PAGES, 50);
        assert_eq!(OBSERVATION_SCHEMA, 4);
    }

    #[test]
    fn a_closed_declarer_is_resolved_from_the_cross_references() {
        let listed = |number: u64| pull_request_of(&json!({"number": number})).unwrap();
        let mut open = vec![listed(1), listed(2)];
        let mut read = Declarations::new();
        read.insert(
            1,
            DeclarationRead {
                association: "OWNER".into(),
                cross_repository: Some(false),
                sources: vec![
                    source(5, "CLOSED", "Supersedes #1", "CONTRIBUTOR", true),
                    // still open: it speaks for itself, among the open ones
                    source(2, "OPEN", "Supersedes #1", "OWNER", false),
                    // mentions #1 and supersedes only what is not open
                    source(6, "MERGED", "See #1.\nSupersedes #40", "OWNER", false),
                    // mentions #1 in prose
                    source(8, "MERGED", "this supersedes #1", "OWNER", false),
                ],
                read: CrossReferenceRead::Whole,
            },
        );
        let sources = declare(&mut open, &read);
        let resolved = resolved_for(&open, &sources, |n| panic!("#{n} was viewed"));
        assert_eq!(resolved.keys().copied().collect::<Vec<_>>(), [5]);
        let five = &resolved[&5];
        assert_eq!(
            (
                five.merged,
                five.author_association.as_str(),
                five.cross_repository,
                five.merge_commit.as_str()
            ),
            (false, "CONTRIBUTOR", true, "m5"),
            "who declared is kept, for the classifier to weigh"
        );
    }

    #[test]
    fn a_dependency_that_is_not_open_is_read_like_a_successor() {
        let open: Vec<PullRequestObservation> = [json!({
            "number": 1, "title": "t", "author": {"login": "a"}, "headRefName": "f",
            "headRefOid": "h1", "baseRefName": "master", "isDraft": false, "labels": [],
            "createdAt": "t", "updatedAt": "t", "body": "Depends on #4\nDepends on #5",
            "statusCheckRollup": [], "reviewDecision": "", "autoMergeRequest": null,
            "isCrossRepository": false
        })]
        .iter()
        .filter_map(pull_request_of)
        .collect();
        let mut viewed = Vec::new();
        let resolved = resolved_for(&open, &json!([]), |n| {
            viewed.push(n);
            (n == 4)
                .then(|| json!({"number": 4, "state": "CLOSED", "headRefOid": "h4", "body": ""}))
        });
        assert_eq!(viewed, [4, 5]);
        assert_eq!(
            resolved.keys().copied().collect::<Vec<_>>(),
            [4],
            "#5 unread"
        );
        assert!(!resolved[&4].merged);
    }
}
