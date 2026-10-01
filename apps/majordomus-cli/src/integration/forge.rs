//! The forge adapter: the one place pull-request integration talks to the network.
//!
//! SECURITY.md declares it: `majordomus prs refresh` and `majordomus prs drain` — and
//! nothing else — run the GitHub CLI (`gh`) and `git fetch` against this repository's own
//! remote. The HTTP server, the MCP tools and the Cockpit never do: they render the last
//! [`ForgeObservation`] recorded under `.ai/local/state/integration/`, with its age, so a
//! page load can never reach the network. The observation is plain data, so every test
//! builds one by hand and no test needs the forge.

use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::{CheckObservation, CheckRunState, PullRequestObservation};

/// The recorded observation's schema version.
pub const OBSERVATION_SCHEMA: u32 = 1;

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
    /// The status contexts the base's protection requires; `None` when it could not be
    /// read (every required-check verdict is then unknown).
    pub required_checks: Option<Vec<String>>,
    /// Whether the protection requires an approving review; `None` when unread.
    pub reviews_required: Option<bool>,
    /// The merge methods the repository allows, in the forge's words (`merge`, `squash`,
    /// `rebase`).
    pub merge_methods: Vec<String>,
    /// Every open pull request.
    pub pull_requests: Vec<PullRequestObservation>,
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
                    .map(|c| CheckObservation {
                        name: c
                            .get("name")
                            .or_else(|| c.get("context"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        state: check_state(c),
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
    })
}

/// The required contexts and review requirement from a branch-protection document.
pub fn protection_of(v: &Value) -> (Option<Vec<String>>, Option<bool>) {
    // a set: the protection lists a context under `contexts` and again under `checks`, and
    // the order is the set's, so no sort sits beside what renders it
    let mut contexts: std::collections::BTreeSet<String> = v
        .pointer("/required_status_checks/contexts")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| c.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if let Some(checks) = v
        .pointer("/required_status_checks/checks")
        .and_then(Value::as_array)
    {
        contexts.extend(
            checks
                .iter()
                .filter_map(|c| c.get("context").and_then(Value::as_str).map(str::to_string)),
        );
    }
    let reviews = v
        .pointer("/required_pull_request_reviews/required_approving_review_count")
        .and_then(Value::as_u64)
        .map(|n| n > 0)
        .or(Some(v.get("required_pull_request_reviews").is_some()));
    (Some(contexts.into_iter().collect()), reviews)
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
        let (required_checks, reviews_required) = if ok {
            serde_json::from_str::<Value>(&out)
                .map(|v| protection_of(&v))
                .unwrap_or((None, None))
        } else if err.contains("Branch not protected") || out.contains("Branch not protected") {
            (Some(Vec::new()), Some(false))
        } else {
            (None, None)
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
                "number,title,author,headRefName,headRefOid,baseRefName,isDraft,labels,createdAt,updatedAt,body,statusCheckRollup,reviewDecision,autoMergeRequest,isCrossRepository",
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
        Ok(ForgeObservation {
            schema: OBSERVATION_SCHEMA,
            repository,
            base,
            base_sha,
            observed_at: crate::peers::rfc3339(std::time::SystemTime::now()),
            required_checks,
            reviews_required,
            merge_methods,
            pull_requests,
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
    fn a_protection_names_its_contexts_once_and_its_review_requirement() {
        let both = json!({
            "required_status_checks": {
                "contexts": ["ci", "lint"],
                "checks": [{"context": "ci"}, {"context": "docs"}]
            },
            "required_pull_request_reviews": {"required_approving_review_count": 1}
        });
        let (contexts, reviews) = protection_of(&both);
        assert_eq!(contexts.unwrap(), ["ci", "docs", "lint"]);
        assert_eq!(reviews, Some(true));

        let none_required = json!({
            "required_status_checks": {"contexts": []},
            "required_pull_request_reviews": {"required_approving_review_count": 0}
        });
        assert_eq!(protection_of(&none_required).1, Some(false));

        let reviews_without_count = json!({"required_pull_request_reviews": {}});
        let (contexts, reviews) = protection_of(&reviews_without_count);
        assert_eq!(contexts.unwrap(), Vec::<String>::new());
        assert_eq!(
            reviews,
            Some(true),
            "a review section without a count still asks for one"
        );

        assert_eq!(protection_of(&json!({})).1, Some(false));
    }
}
