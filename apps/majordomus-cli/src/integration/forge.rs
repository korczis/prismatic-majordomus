//! The forge adapter: the one place pull-request integration talks to the network.
//!
//! SECURITY.md declares it: `majordomus prs refresh` and `majordomus prs drain` — and
//! nothing else — run the GitHub CLI (`gh`) and `git fetch` against this repository's own
//! remote. The HTTP server, the MCP tools and the Cockpit never do: they render the last
//! [`ForgeObservation`] recorded under `.ai/local/state/integration/`, with its age, so a
//! page load can never reach the network. The observation is plain data, so every test
//! builds one by hand and no test needs the forge.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::{
    CheckKind, CheckObservation, CheckRunState, PullRequestObservation, RequiredCheck,
    ReviewObservation, ReviewPolicy,
};

/// The recorded observation's schema version.
pub const OBSERVATION_SCHEMA: u32 = 2;

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
    /// required-check verdict is then unknown, never passed).
    pub required_checks: Option<Vec<RequiredCheck>>,
    /// What the base requires of reviews, from its protection and rulesets together; `None`
    /// when either could not be read.
    pub review_policy: Option<ReviewPolicy>,
    /// The merge methods the repository allows, in the forge's words (`merge`, `squash`,
    /// `rebase`).
    pub merge_methods: Vec<String>,
    /// Every open pull request.
    pub pull_requests: Vec<PullRequestObservation>,
    /// Pull requests that are no longer open and that a supersession names, by number: a
    /// closed one whose body says it supersedes an open one, and every successor an open
    /// one's body names that is not open. What decides whether a declared successor landed.
    /// Empty in an observation recorded before it was read.
    #[serde(default)]
    pub resolved: BTreeMap<u64, ResolvedPullRequest>,
}

/// A pull request that is no longer open, as the forge reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedPullRequest {
    /// Whether the forge says it was merged, rather than closed unmerged. Evidence only: a
    /// successor landed when git finds its head in master, whatever the forge calls it.
    pub merged: bool,
    /// The commit its branch pointed at when it was merged or closed.
    pub head_sha: String,
    /// The body, for the supersessions it declares; never rendered.
    #[serde(default)]
    pub body: String,
}

/// How many closed pull requests declaring a supersession are read, newest first.
pub const RESOLVED_LIMIT: usize = 200;

/// One pull request that is no longer open, from `gh pr list --state closed --json` or
/// `gh pr view --json` output: `None` for an open one, or one without a head.
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
    let body = v
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some((
        number,
        ResolvedPullRequest {
            merged,
            head_sha,
            body,
        },
    ))
}

/// The pull requests no longer open that a supersession involving an open one names: closed
/// ones whose body says they supersede an open one (`closed`, the forge's answer to a search),
/// and the successors open bodies name that are not open, each read with `view`. A successor
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
        .flat_map(|p| declared_supersessions(&p.body).superseded_by)
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
            .map(|a| {
                a.iter()
                    .map(|c| {
                        let text = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or("");
                        let kind = if text("__typename") == "StatusContext" {
                            CheckKind::StatusContext
                        } else {
                            CheckKind::CheckRun
                        };
                        CheckObservation {
                            name: c
                                .get("name")
                                .or_else(|| c.get("context"))
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            state: check_state(c),
                            kind,
                            // the app, where the forge names it (a check run's suite's app)
                            app_id: c
                                .pointer("/app/databaseId")
                                .or_else(|| c.pointer("/checkSuite/app/databaseId"))
                                .and_then(Value::as_u64),
                            completed_at: match text("completedAt") {
                                // a check run's zero time means it has not completed
                                "" | "0001-01-01T00:00:00Z" => text("startedAt")
                                    .trim_start_matches("0001-01-01T00:00:00Z")
                                    .to_string(),
                                t => t.to_string(),
                            },
                        }
                    })
                    .collect()
            })
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
/// once, bound to an app when either source binds it.
fn union_checks(a: Vec<RequiredCheck>, b: Vec<RequiredCheck>) -> Vec<RequiredCheck> {
    let mut by_context: std::collections::BTreeMap<String, Option<u64>> =
        std::collections::BTreeMap::new();
    for c in a.into_iter().chain(b) {
        let slot = by_context.entry(c.context).or_insert(None);
        if slot.is_none() {
            *slot = c.app_id;
        }
    }
    by_context
        .into_iter()
        .map(|(context, app_id)| RequiredCheck { context, app_id })
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
        let protection: Option<(Vec<RequiredCheck>, ReviewPolicy)> = if ok {
            serde_json::from_str::<Value>(&out)
                .ok()
                .map(|v| protection_of(&v))
        } else if err.contains("Branch not protected") || out.contains("Branch not protected") {
            Some((Vec::new(), ReviewPolicy::default()))
        } else {
            None
        };
        // the rulesets that apply to the base add requirements the protection does not list;
        // a ruleset read that fails leaves the requirement unread, as an unread protection does
        let (ok, out, _) = gh_retrying(
            root,
            &["api", &format!("repos/{repository}/rules/branches/{base}")],
        )?;
        let rules: Option<(Vec<RequiredCheck>, ReviewPolicy)> = if ok {
            serde_json::from_str::<Value>(&out)
                .ok()
                .map(|v| rules_of(&v))
        } else {
            None
        };
        let (required_checks, review_policy) = match (protection, rules) {
            (Some((pc, pr)), Some((rc, rr))) => {
                (Some(union_checks(pc, rc)), Some(union_reviews(pr, rr)))
            }
            _ => (None, None),
        };
        let list = gh_json(
            root,
            &[
                "pr",
                "list",
                "--state",
                "open",
                "--limit",
                "500",
                "--json",
                "number,title,author,headRefName,headRefOid,baseRefName,isDraft,labels,createdAt,updatedAt,body,statusCheckRollup,reviewDecision,latestReviews,reviewRequests,autoMergeRequest,isCrossRepository",
            ],
        )?;
        // keyed by number, so the observation is in number order whatever the forge listed
        let pull_requests: Vec<PullRequestObservation> = list
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
        // a successor that landed is no longer listed among the open ones: closed pull requests
        // whose body declares a supersession are read with their heads, so the one they
        // replace is still known to be replaced once they are gone
        let closed = gh_json(
            root,
            &[
                "pr",
                "list",
                "--state",
                "closed",
                "--search",
                "supersedes in:body sort:updated-desc",
                "--limit",
                &RESOLVED_LIMIT.to_string(),
                "--json",
                "number,state,headRefOid,body",
            ],
        )?;
        let mut unreadable = None;
        let resolved = resolved_for(&pull_requests, &closed, |m| {
            match gh_retrying(
                root,
                &[
                    "pr",
                    "view",
                    &m.to_string(),
                    "--json",
                    "number,state,headRefOid,body",
                ],
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
            merge_methods,
            pull_requests,
            resolved,
        })
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

    #[test]
    fn a_check_entry_records_what_wrote_it_and_when_it_completed() {
        let p = pull_request_of(&json!({
            "number": 3,
            "statusCheckRollup": [
                {"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS",
                 "startedAt": "2026-09-01T00:00:00Z", "completedAt": "2026-09-01T00:05:00Z",
                 "app": {"databaseId": 15368}},
                {"__typename": "CheckRun", "name": "ci", "status": "IN_PROGRESS",
                 "startedAt": "2026-09-01T00:06:00Z", "completedAt": "0001-01-01T00:00:00Z"},
                {"__typename": "StatusContext", "context": "ci", "state": "SUCCESS",
                 "startedAt": "2026-09-01T00:07:00Z"}
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
}
