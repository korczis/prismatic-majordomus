//! What was actually deployed, as the forge recorded it: every deployment to every
//! environment (GitHub Pages among them) and every release, read with the forge's own tool
//! and kept as one observation.
//!
//! A deployment *declared* in the layer says where the product is meant to run
//! ([`super::Deployment`]). This is the other half: where something was in fact put, when,
//! from which commit, and how that ended. Nothing here is declared, listed or configured —
//! an environment exists because the forge has a deployment in it, so a new target appears
//! the first time something is deployed there and no file names it.
//!
//! The reading is one GraphQL question through `gh`, asked by `majordomus served forge` and
//! recorded beside the pull-request observation, which `majordomus prs refresh` takes. The
//! same command also reads the newest merged pull requests, a page at a time, because their
//! titles are the forge's and `plan.delivery` reads the issues they name from here; the
//! executor's `prs refresh` never asks for them, so they stay off its hot path.
//! Every reader — `deploy.observed`, the Deployments screen, the published snapshot — reads
//! the record and never the network, and a checkout that has never observed says so rather
//! than showing an empty table.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The observation's file, beside the pull-request observation.
pub const FILE: &str = "deployments.json";

/// The format of the recorded observation.
pub const SCHEMA: u32 = 1;

/// How many deployments and releases one observation asks for: the newest of each. Enough to
/// show what is live everywhere and how it got there; the forge keeps the rest.
pub const WINDOW: usize = 50;

/// The one question asked of the forge.
const QUERY: &str = "query($owner:String!,$name:String!,$n:Int!){repository(owner:$owner,name:$name){url \
deployments(last:$n){nodes{databaseId environment createdAt task commitOid ref{name} creator{login} \
latestStatus{state environmentUrl logUrl}}} \
releases(first:$n,orderBy:{field:CREATED_AT,direction:DESC}){nodes{tagName name publishedAt url isPrerelease isDraft}}}}";

/// How many merged pull requests one observation reads, newest first: the title a merge on
/// the trunk does not carry in git (a pull request merged inside a combined top reaches the
/// trunk through an octopus merge that names it only in prose) is the forge's, and
/// `plan.delivery` reads it from here (ADR 0166). Read a page at a time, [`MERGED_PAGE`] each.
pub const MERGED_WINDOW: usize = 1000;

/// One page of merged pull requests: the forge's largest.
pub const MERGED_PAGE: usize = 100;

/// The question asked for each page of merged pull requests, walking back from the newest.
const MERGED_QUERY: &str = "query($owner:String!,$name:String!,$n:Int!,$before:String){repository(owner:$owner,name:$name){\
pullRequests(states:MERGED,last:$n,before:$before){pageInfo{hasPreviousPage startCursor} \
nodes{number title headRefName headRefOid mergedAt mergeCommit{oid}}}}}";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One merged pull request, as the forge recorded it: what names the issues it realised.
pub struct ObservedMergedPullRequest {
    /// The number.
    pub number: u64,
    /// The title: the words that name an issue (`... (I2741)`).
    pub title: String,
    /// The branch it merged.
    pub head_ref: String,
    /// The head commit that merged.
    pub head_sha: String,
    /// The merge commit the forge names; empty when it names none.
    #[serde(default)]
    pub merge_commit: String,
    /// When it merged, RFC 3339.
    pub merged_at: String,
}

/// Newest first: a pull request's number only grows, so the largest is the latest opened.
impl crate::order::Ordered for ObservedMergedPullRequest {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.title, &self.head_sha).descending(self.number)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One deployment the forge recorded.
pub struct ObservedDeployment {
    /// The forge's identity of the deployment.
    pub id: String,
    /// The environment it went to, in the forge's words (`github-pages`, `production`, ...).
    pub environment: String,
    /// The commit that was deployed.
    pub sha: String,
    /// The ref it was deployed from, when the forge still names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    /// When it was created, RFC 3339.
    pub created_at: String,
    /// Who or what created it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    /// How it ended, lower case as the forge states it (`success`, `failure`, `in_progress`,
    /// ...); `unknown` when the forge reported no status at all.
    pub state: String,
    /// Where the environment is served, when the deployment says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_url: Option<String>,
    /// The run that performed it, when the deployment says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_url: Option<String>,
}

// Newest first: what is live now leads, whatever order the forge answered in.
impl crate::order::Ordered for ObservedDeployment {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.created_at, &self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One release the forge published.
pub struct ObservedRelease {
    /// The tag.
    pub tag: String,
    /// The release's title, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// When it was published; absent for a draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// Its page on the forge.
    pub url: String,
    /// Whether the forge marks it a pre-release.
    pub prerelease: bool,
    /// Whether it is still a draft.
    pub draft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// Everything the forge said about deployments and releases, at one moment.
pub struct DeploymentObservation {
    /// [`SCHEMA`].
    pub schema: u32,
    /// `owner/name`.
    pub repository: String,
    /// The repository's address on the forge: what every commit, environment and release link
    /// here is built from, so no host is written anywhere else.
    pub repository_url: String,
    /// When, RFC 3339.
    pub observed_at: String,
    /// The newest deployments, newest first.
    pub deployments: Vec<ObservedDeployment>,
    /// The newest releases, newest first.
    pub releases: Vec<ObservedRelease>,
    /// The newest merged pull requests (up to [`MERGED_WINDOW`]), newest first; `None` when
    /// they were not read — an observation recorded before they were, or a read the forge
    /// refused — which is unread, never "nothing merged".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_pull_requests: Option<Vec<ObservedMergedPullRequest>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One environment, as its deployments describe it.
pub struct EnvironmentSummary {
    /// The environment.
    pub name: String,
    /// How many of the observed deployments went to it.
    pub deployments: usize,
    /// The state of its newest deployment.
    pub state: String,
    /// The commit its newest deployment put there.
    pub sha: String,
    /// When that was.
    pub deployed_at: String,
    /// Where it is served, when its newest deployment says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The environment's activity on the forge.
    pub activity_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// `deploy.observed`: what was deployed where, or that nothing has been observed here.
pub struct ObservedDeployments {
    /// Whether this checkout holds an observation at all.
    pub observed: bool,
    /// Why not, and what to run, when it holds none or cannot read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `owner/name`, when observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// The repository's address on the forge, when observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
    /// When the forge was asked, when observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    /// Every environment something was deployed to, by name.
    pub environments: Vec<EnvironmentSummary>,
    /// The observed deployments, newest first.
    pub deployments: Vec<ObservedDeployment>,
    /// The observed releases, newest first.
    pub releases: Vec<ObservedRelease>,
}

fn string(v: &Value, pointer: &str) -> Option<String> {
    v.pointer(pointer)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn nodes<'a>(repository: &'a Value, connection: &str) -> &'a [Value] {
    repository
        .pointer(&format!("/{connection}/nodes"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// The observation a GraphQL answer holds. A node that names no commit or no tag is not a
/// deployment or a release anybody could follow, and is left out.
pub fn parse(repository: &str, observed_at: &str, answer: &Value) -> DeploymentObservation {
    let repo = answer.pointer("/data/repository").unwrap_or(&Value::Null);
    let mut deployments: Vec<ObservedDeployment> = nodes(repo, "deployments")
        .iter()
        .filter_map(|d| {
            Some(ObservedDeployment {
                sha: string(d, "/commitOid")?,
                id: d
                    .get("databaseId")
                    .and_then(Value::as_u64)
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                environment: string(d, "/environment").unwrap_or_else(|| "unnamed".into()),
                git_ref: string(d, "/ref/name"),
                created_at: string(d, "/createdAt").unwrap_or_default(),
                creator: string(d, "/creator/login"),
                state: string(d, "/latestStatus/state")
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_else(|| "unknown".into()),
                environment_url: string(d, "/latestStatus/environmentUrl"),
                log_url: string(d, "/latestStatus/logUrl"),
            })
        })
        .collect();
    crate::order::canonical(&mut deployments);
    deployments.reverse();
    let releases = nodes(repo, "releases")
        .iter()
        .filter_map(|r| {
            Some(ObservedRelease {
                tag: string(r, "/tagName")?,
                url: string(r, "/url")?,
                name: string(r, "/name"),
                published_at: string(r, "/publishedAt"),
                prerelease: r.get("isPrerelease").and_then(Value::as_bool) == Some(true),
                draft: r.get("isDraft").and_then(Value::as_bool) == Some(true),
            })
        })
        .collect();
    DeploymentObservation {
        schema: SCHEMA,
        repository: repository.to_string(),
        repository_url: string(repo, "/url").unwrap_or_default(),
        observed_at: observed_at.to_string(),
        deployments,
        releases,
        merged_pull_requests: None,
    }
}

/// The merged pull requests one page of [`MERGED_QUERY`] holds, newest first, and the cursor
/// of the page before it when there is one. A node that names no number, no head or no
/// merge time is not one anybody could follow, and is left out.
pub fn parse_merged(answer: &Value) -> (Vec<ObservedMergedPullRequest>, Option<String>) {
    let repo = answer.pointer("/data/repository").unwrap_or(&Value::Null);
    let mut merged: Vec<ObservedMergedPullRequest> = nodes(repo, "pullRequests")
        .iter()
        .filter_map(|p| {
            Some(ObservedMergedPullRequest {
                number: p.get("number").and_then(Value::as_u64)?,
                title: string(p, "/title").unwrap_or_default(),
                head_ref: string(p, "/headRefName").unwrap_or_default(),
                head_sha: string(p, "/headRefOid")?,
                merge_commit: string(p, "/mergeCommit/oid").unwrap_or_default(),
                merged_at: string(p, "/mergedAt")?,
            })
        })
        .collect();
    crate::order::canonical(&mut merged);
    let more = repo.pointer("/pullRequests/pageInfo/hasPreviousPage") == Some(&Value::Bool(true));
    let earlier = more
        .then(|| string(repo, "/pullRequests/pageInfo/startCursor"))
        .flatten();
    (merged, earlier)
}

/// Every page of merged pull requests, newest first, up to [`MERGED_WINDOW`]; `ask` answers
/// one page given the cursor before which it lies (`None` for the newest). A page the forge
/// refuses fails the whole read: a partial list would read as "nothing older merged".
pub fn merged_pages(
    mut ask: impl FnMut(Option<&str>) -> Result<Value, String>,
) -> Result<Vec<ObservedMergedPullRequest>, String> {
    let mut all: Vec<ObservedMergedPullRequest> = Vec::new();
    let mut before: Option<String> = None;
    loop {
        let (page, earlier) = parse_merged(&ask(before.as_deref())?);
        let empty = page.is_empty();
        all.extend(page);
        match earlier {
            Some(cursor)
                if !empty && all.len() < MERGED_WINDOW && before.as_deref() != Some(&cursor) =>
            {
                before = Some(cursor);
            }
            _ => break,
        }
    }
    all.truncate(MERGED_WINDOW);
    Ok(all)
}

fn run(program: &str, root: &Path, args: &[&str]) -> Result<Value, String> {
    let out = crate::process::retrying_busy(|| {
        Command::new(program).current_dir(root).args(args).output()
    })
    .map_err(|e| format!("the GitHub CLI ({program}) could not run: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    match out.status.success() {
        true => serde_json::from_str(&text).map_err(|e| format!("{program} {}: {e}", args[0])),
        false => Err(format!(
            "{program} {} failed: {}",
            args[0],
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// Ask the forge through `program` (the GitHub CLI) from the repository at `root`.
pub fn observe_with(program: &str, root: &Path) -> Result<DeploymentObservation, String> {
    let named = run(program, root, &["repo", "view", "--json", "nameWithOwner"])?;
    let repository = string(&named, "/nameWithOwner")
        .ok_or_else(|| format!("{program} repo view named no repository"))?;
    let (owner, name) = repository
        .split_once('/')
        .ok_or_else(|| format!("{repository} is not owner/name"))?;
    let answer = run(
        program,
        root,
        &[
            "api",
            "graphql",
            "-f",
            &format!("query={QUERY}"),
            "-F",
            &format!("owner={owner}"),
            "-F",
            &format!("name={name}"),
            "-F",
            &format!("n={WINDOW}"),
        ],
    )?;
    let mut observation = parse(
        &repository,
        &crate::evidence::record::now_rfc3339(),
        &answer,
    );
    // the merged pull requests are a second question, a page at a time; a refusal leaves
    // them unread and keeps what the deployments said
    observation.merged_pull_requests = merged_pages(|before| {
        let mut args: Vec<String> = vec![
            "api".into(),
            "graphql".into(),
            "-f".into(),
            format!("query={MERGED_QUERY}"),
            "-F".into(),
            format!("owner={owner}"),
            "-F".into(),
            format!("name={name}"),
            "-F".into(),
            format!("n={MERGED_PAGE}"),
        ];
        if let Some(cursor) = before {
            args.extend(["-f".into(), format!("before={cursor}")]);
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        run(program, root, &argv)
    })
    .ok();
    Ok(observation)
}

/// The forge's own tool, as it is found on the path.
pub const GH: &str = "gh";

/// Observe the forge through `program` and record what it said. The network step, taken by
/// `served forge` with [`GH`].
pub fn refresh_with(program: &str, root: &Path) -> Result<DeploymentObservation, String> {
    observe_with(program, root).and_then(|observation| {
        store(root, &observation)?;
        Ok(observation)
    })
}

/// Record an observation, through a temporary sibling and a rename. A directory that could
/// not be made is found out by the write into it.
pub fn store(root: &Path, observation: &DeploymentObservation) -> Result<(), String> {
    let path = crate::integration::state_path(root, FILE);
    let staging = path.with_extension("json.tmp");
    let _ = std::fs::create_dir_all(path.with_file_name(""));
    std::fs::write(&staging, format!("{:#}\n", serde_json::json!(observation)))
        .and_then(|()| std::fs::rename(&staging, &path))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The recorded observation, if there is one. A record of another schema is refused by its
/// number, never read as empty.
pub fn load(root: &Path) -> Result<Option<DeploymentObservation>, String> {
    let path = crate::integration::state_path(root, FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let record: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    match record.get("schema").and_then(Value::as_u64) {
        Some(schema) if schema == u64::from(SCHEMA) => serde_json::from_value(record)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
        other => Err(format!(
            "{}: schema {}, this reads {SCHEMA}; majordomus served forge records one this reads",
            path.display(),
            other.map_or("none".to_string(), |s| s.to_string())
        )),
    }
}

/// Every environment the deployments name, each described by its newest deployment.
pub fn environments(observation: &DeploymentObservation) -> Vec<EnvironmentSummary> {
    let mut by_name: BTreeMap<&str, EnvironmentSummary> = BTreeMap::new();
    // newest first, so the first deployment met for an environment is the one that describes it
    for d in &observation.deployments {
        by_name
            .entry(d.environment.as_str())
            .and_modify(|e| e.deployments += 1)
            .or_insert_with(|| EnvironmentSummary {
                name: d.environment.clone(),
                deployments: 1,
                state: d.state.clone(),
                sha: d.sha.clone(),
                deployed_at: d.created_at.clone(),
                url: d.environment_url.clone(),
                activity_url: format!(
                    "{}/deployments/{}",
                    observation.repository_url, d.environment
                ),
            });
    }
    by_name.into_values().collect()
}

/// What `deploy.observed` answers for the repository at `root`.
pub fn report(root: &Path) -> ObservedDeployments {
    let unobserved = |reason: String| ObservedDeployments {
        observed: false,
        reason: Some(reason),
        repository: None,
        repository_url: None,
        observed_at: None,
        environments: Vec::new(),
        deployments: Vec::new(),
        releases: Vec::new(),
    };
    match load(root) {
        Ok(Some(o)) => ObservedDeployments {
            observed: true,
            reason: None,
            environments: environments(&o),
            repository: Some(o.repository),
            repository_url: Some(o.repository_url),
            observed_at: Some(o.observed_at),
            deployments: o.deployments,
            releases: o.releases,
        },
        Ok(None) => unobserved(
            "the forge has not been observed from this checkout; majordomus served forge asks it"
                .into(),
        ),
        Err(reason) => unobserved(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn answer() -> Value {
        json!({"data": {"repository": {
            "url": "https://forge.example/o/r",
            "deployments": {"nodes": [
                {"databaseId": 1, "environment": "github-pages", "createdAt": "2026-10-12T10:00:00Z",
                 "task": "deploy", "commitOid": "aaa", "ref": {"name": "gh-pages"},
                 "creator": {"login": "bot"},
                 "latestStatus": {"state": "SUCCESS", "environmentUrl": "https://site.example/", "logUrl": "https://forge.example/run/1"}},
                {"databaseId": 3, "environment": "github-pages", "createdAt": "2026-10-12T12:00:00Z",
                 "commitOid": "ccc", "ref": null, "creator": null, "latestStatus": null},
                {"databaseId": 2, "environment": "production", "createdAt": "2026-10-12T11:00:00Z",
                 "commitOid": "bbb", "latestStatus": {"state": "FAILURE"}},
                {"databaseId": 4, "environment": "ghost", "createdAt": "2026-10-12T13:00:00Z"}
            ]},
            "releases": {"nodes": [
                {"tagName": "v2", "name": "Two", "publishedAt": "2026-10-12T09:00:00Z",
                 "url": "https://forge.example/o/r/releases/tag/v2", "isPrerelease": false, "isDraft": false},
                {"tagName": "v3", "url": "https://forge.example/o/r/releases/tag/v3", "isPrerelease": true, "isDraft": true},
                {"name": "no tag", "url": "https://forge.example/x"}
            ]}
        }}})
    }

    #[test]
    fn an_answer_becomes_deployments_newest_first_and_releases_as_the_forge_ordered_them() {
        let o = parse("o/r", "2026-10-12T14:00:00Z", &answer());
        let ids: Vec<&str> = o.deployments.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            ["3", "2", "1"],
            "a deployment that names no commit is left out"
        );
        assert_eq!(
            o.deployments[0].state, "unknown",
            "no status is not success"
        );
        assert_eq!(o.deployments[1].state, "failure");
        assert_eq!(o.deployments[2].git_ref.as_deref(), Some("gh-pages"));
        assert_eq!(o.deployments[2].creator.as_deref(), Some("bot"));
        let tags: Vec<&str> = o.releases.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["v2", "v3"], "a release without a tag is left out");
        assert!(o.releases[1].draft && o.releases[1].prerelease);
        assert_eq!(o.repository_url, "https://forge.example/o/r");
    }

    #[test]
    fn an_environment_is_described_by_its_newest_deployment() {
        let o = parse("o/r", "t", &answer());
        let envs = environments(&o);
        let names: Vec<&str> = envs.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["github-pages", "production"]);
        assert_eq!(envs[0].deployments, 2);
        assert_eq!(
            envs[0].sha, "ccc",
            "the newest, not the first the forge listed"
        );
        assert_eq!(envs[0].state, "unknown");
        assert_eq!(envs[1].state, "failure");
        assert_eq!(
            envs[0].activity_url,
            "https://forge.example/o/r/deployments/github-pages"
        );
    }

    fn merged_page(numbers: &[u64], earlier: Option<&str>) -> Value {
        let nodes: Vec<Value> = numbers
            .iter()
            .map(|n| {
                json!({"number": n, "title": format!("fix: thing (I{n:04})"), "headRefName": format!("fix/{n}"),
                       "headRefOid": format!("h{n}"), "mergedAt": "2026-10-16T10:00:00Z",
                       "mergeCommit": {"oid": format!("m{n}")}})
            })
            .chain([
                json!({"number": 99, "title": "no head, not followable"}),
                json!({"title": "no number", "headRefOid": "h", "mergedAt": "2026-10-16T10:00:00Z"}),
                json!({"number": 98, "title": "never merged", "headRefOid": "h98"}),
            ])
            .collect();
        json!({"data": {"repository": {"pullRequests": {
            "pageInfo": {"hasPreviousPage": earlier.is_some(), "startCursor": earlier},
            "nodes": nodes
        }}}})
    }

    #[test]
    fn merged_pull_requests_are_read_page_by_page_newest_first_and_a_refusal_fails_the_read() {
        let (page, earlier) = parse_merged(&merged_page(&[3, 4], Some("c1")));
        assert_eq!(
            page.iter().map(|p| p.number).collect::<Vec<_>>(),
            [4, 3],
            "newest first; a node with no head, number or merge time is left out"
        );
        assert_eq!(page[0].merge_commit, "m4");
        assert_eq!(earlier.as_deref(), Some("c1"));

        let mut asked: Vec<Option<String>> = Vec::new();
        let all = merged_pages(|before| {
            asked.push(before.map(str::to_string));
            Ok(match before {
                None => merged_page(&[3, 4], Some("c1")),
                Some(_) => merged_page(&[1, 2], None),
            })
        })
        .unwrap();
        assert_eq!(asked, [None, Some("c1".to_string())]);
        assert_eq!(
            all.iter().map(|p| p.number).collect::<Vec<_>>(),
            [4, 3, 2, 1]
        );

        // a forge that hands back the same cursor is not walked forever
        let mut calls = 0;
        merged_pages(|_| {
            calls += 1;
            Ok(merged_page(&[1], Some("same")))
        })
        .unwrap();
        assert_eq!(calls, 2);

        let mut first = true;
        let refused = merged_pages(|_| {
            if std::mem::take(&mut first) {
                Ok(merged_page(&[2], Some("c")))
            } else {
                Err("rate limited".into())
            }
        });
        assert_eq!(
            refused,
            Err("rate limited".to_string()),
            "partial is not whole"
        );
    }

    #[test]
    fn an_answer_with_no_repository_is_an_empty_observation_not_a_panic() {
        let o = parse("o/r", "t", &json!({"data": {}}));
        assert!(o.deployments.is_empty() && o.releases.is_empty());
        assert_eq!(o.repository_url, "");
    }

    #[test]
    fn a_record_is_stored_read_back_and_refused_when_it_is_another_schema() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()).unwrap(), None);
        let unobserved = report(dir.path());
        assert!(!unobserved.observed);
        assert!(unobserved.reason.unwrap().contains("served forge"));

        let o = parse("o/r", "2026-10-12T14:00:00Z", &answer());
        store(dir.path(), &o).unwrap();
        assert_eq!(load(dir.path()).unwrap().as_ref(), Some(&o));
        let observed = report(dir.path());
        assert!(observed.observed && observed.reason.is_none());
        assert_eq!(observed.environments.len(), 2);
        assert_eq!(observed.repository.as_deref(), Some("o/r"));

        let path = crate::integration::state_path(dir.path(), FILE);
        std::fs::write(&path, r#"{"schema": 99}"#).unwrap();
        let refused = load(dir.path()).unwrap_err();
        assert!(refused.contains("schema 99, this reads 1"), "{refused}");
        assert!(
            !report(dir.path()).observed,
            "a record it cannot read is not observed"
        );
        std::fs::write(&path, "{}").unwrap();
        assert!(load(dir.path()).unwrap_err().contains("schema none"));
        std::fs::write(&path, "not json").unwrap();
        assert!(load(dir.path()).is_err());
        std::fs::write(&path, r#"{"schema": 1}"#).unwrap();
        assert!(
            load(dir.path()).is_err(),
            "the right schema and the wrong shape"
        );
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir_all(&path).unwrap();
        assert!(
            load(dir.path()).is_err(),
            "a directory where the record should be"
        );
    }

    /// A program standing in for `gh`: `repo view` names the repository, `api` answers the
    /// question, and the file `fail` beside it makes every call fail.
    #[cfg(unix)]
    fn fake_gh(dir: &Path, repository: &str, body: &str) -> String {
        use std::os::unix::fs::PermissionsExt;
        let program = dir.join("fake-gh");
        std::fs::write(dir.join("answer.json"), body).unwrap();
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nd=\"$(dirname \"$0\")\"\n[ -e \"$d/fail\" ] && {{ echo 'no network' >&2; exit 1; }}\n\
                 case \"$1\" in\n  repo) echo '{{\"nameWithOwner\": \"{repository}\"}}' ;;\n  api) cat \"$d/answer.json\" ;;\nesac\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        program.to_string_lossy().into_owned()
    }

    #[cfg(unix)]
    #[test]
    fn the_forge_is_asked_through_its_own_tool_and_every_refusal_is_said() {
        let dir = tempfile::tempdir().unwrap();
        let gh = fake_gh(dir.path(), "o/r", &answer().to_string());
        let o = observe_with(&gh, dir.path()).unwrap();
        assert_eq!(o.repository, "o/r");
        assert_eq!(o.deployments.len(), 3);
        assert_eq!(
            o.merged_pull_requests,
            Some(vec![]),
            "an answer holding no merged pull request is read as none"
        );

        std::fs::write(dir.path().join("answer.json"), "not json").unwrap();
        assert!(observe_with(&gh, dir.path()).unwrap_err().contains("api"));

        std::fs::write(dir.path().join("fail"), "").unwrap();
        let refused = observe_with(&gh, dir.path()).unwrap_err();
        assert!(refused.contains("failed: no network"), "{refused}");
        std::fs::remove_file(dir.path().join("fail")).unwrap();

        let elsewhere = tempfile::tempdir().unwrap();
        let unnamed = fake_gh(elsewhere.path(), "", "{}");
        assert!(observe_with(&unnamed, dir.path())
            .unwrap_err()
            .contains("named no repository"));
        let flat = fake_gh(elsewhere.path(), "flat", "{}");
        assert!(observe_with(&flat, dir.path())
            .unwrap_err()
            .contains("not owner/name"));
        assert!(observe_with("/no/such/program", dir.path())
            .unwrap_err()
            .contains("could not run"));

        // the merged pull requests are asked a page at a time, the next one before the
        // cursor the last one named; this forge names the same cursor again, which ends it
        std::fs::write(
            dir.path().join("answer.json"),
            merged_page(&[5, 6], Some("c1")).to_string(),
        )
        .unwrap();
        let paged = observe_with(&gh, dir.path()).unwrap();
        let numbers: Vec<u64> = paged
            .merged_pull_requests
            .unwrap()
            .iter()
            .map(|p| p.number)
            .collect();
        assert_eq!(
            numbers,
            [6, 5, 6, 5],
            "two pages asked, the second before c1"
        );

        // refreshed: asked and recorded, and a record that cannot be written is said
        std::fs::write(dir.path().join("answer.json"), answer().to_string()).unwrap();
        let o = refresh_with(&gh, dir.path()).unwrap();
        assert_eq!(load(dir.path()).unwrap(), Some(o));
        assert!(refresh_with("/no/such/program", dir.path()).is_err());
        let blocked = tempfile::tempdir().unwrap();
        std::fs::write(
            blocked.path().join(".ai"),
            "a file where the layer would be",
        )
        .unwrap();
        assert!(refresh_with(&gh, blocked.path()).is_err());
    }
}
