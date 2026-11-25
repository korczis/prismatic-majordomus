//! Whether an issue this head authors or executes is another's: `plan.collisions` (ADR 0238,
//! I4775).
//!
//! `plan.allocate` (I3040) keeps two callers from taking one id, but only callers that use
//! it, and only once their claims have synced. Nothing told a head that the id it authored
//! had been authored on another line too, or that the issue it was about to execute already
//! had an open pull request: in one adopter repository on 2026-11-25 peers took colliding ids
//! despite board reservations, and two sessions executed one issue because the second
//! session's picker ran before the first one's pull request existed. This module answers, for
//! one head, every such collision with its holder:
//!
//! | class               | what                                                                 | verdict |
//! |---------------------|----------------------------------------------------------------------|---------|
//! | `id_reused`         | a record the head adds since its merge-base that another line also added under another title | refused |
//! | `retitled`          | a record the base already holds whose title the head changes         | warned  |
//! | `claimed_elsewhere` | an issue the head executes that another open pull request or another session's live mesh claim names | refused |
//! | `claimed_elsewhere` | the same, named only by another unmerged branch                       | warned  |
//!
//! The head executes an issue only when its branch names it (the I1405 rule, or the slug) or
//! the caller names it: changing a record's stamps is recording, not execution, and a head
//! whose change is planning-only (I2749) executes nothing of its own branch's. An open pull
//! request names an issue by its head branch, or by a title naming at most
//! [`TITLE_LIMIT`] ids; a title naming more is a combined top, which claims none.
//!
//! The cost of `id_reused` scales with the records the head adds, not with the refs: one
//! `git log --all --diff-filter=A` over those paths finds every commit that added them on any
//! ref, and a record is compared by its title, not by its blob, because stamps change a blob
//! without changing whose record it is.
//!
//! The forge is an observation (`majordomus prs refresh`): one older than
//! `collisions.max_age` (`.ai/repo/ci/plan.yaml`) is stale, and a stale or unread forge never
//! proves that no other pull request names an issue.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub(crate) use crate::capability::builtin::plan::CollisionGate;
use crate::integration::{ForgeObservation, PullRequestObservation};
use crate::plan::delivery::{DeliveryReading, DeliverySource};
use crate::plan::Plan;
use crate::ui::prs::issues_named;
use crate::worktree::state::issue_of;
use crate::worktree::Trunk;

/// The version of the report's shape.
pub(crate) const COLLISIONS_SCHEMA: u32 = 1;

/// Where the plan's issue records live.
pub(crate) const ISSUES_DIR: &str = ".ai/repo/project/issues/";

/// A pull request title naming more ids than this is a combined top, and claims none.
pub(crate) const TITLE_LIMIT: usize = 3;

/// The seconds a forge observation is current for when `.ai/repo/ci/plan.yaml` declares none.
pub(crate) const DEFAULT_MAX_AGE: u64 = 3600;

/// Exit code of a gate that refuses a collision.
pub(crate) const EXIT_REFUSED: u8 = 10;

/// Exit code of a pull-request gate that could not read the forge.
pub(crate) const EXIT_UNJUDGED: u8 = 12;

/// A live mesh claim that names an issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveClaim {
    /// The issue.
    pub issue: String,
    /// `<stream>/<claim>`.
    pub key: String,
    /// The holding session.
    pub session: String,
}

/// What the caller asked.
#[derive(Debug, Clone, Default)]
pub(crate) struct Asked<'a> {
    /// The head: a revision.
    pub head: &'a str,
    /// The head's branch, when the caller knows it better than git (a CI checkout of a
    /// pull request is detached).
    pub branch: Option<&'a str>,
    /// An issue the caller is about to execute.
    pub issue: Option<&'a str>,
    /// The gate, when one is asked.
    pub gate: Option<CollisionGate>,
    /// The caller's own mesh session.
    pub session: Option<&'a str>,
}

/// One kind of collision.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CollisionClass {
    /// An id another line of history authored too, under another title.
    IdReused,
    /// A record the base holds whose title the head changes.
    Retitled,
    /// An issue the head executes that another pull request, session or branch names.
    ClaimedElsewhere,
}

/// What the check made of one collision.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CollisionVerdict {
    /// The head may not proceed while it stands.
    Refused,
    /// Reported; it does not refuse.
    Warned,
}

/// Who holds what the head collides with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct CollisionHolder {
    /// `trunk`, `branch`, `pull_request` or `claim`.
    pub kind: String,
    /// The ref, the pull request's head branch, or the claim's key.
    pub name: String,
    /// The pull request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<u64>,
    /// The holding mesh session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The commit that authored the record, for `id_reused`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The title the holder gives the record, or its pull request's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// One collision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Collision {
    /// The class.
    pub class: CollisionClass,
    /// The issue id.
    pub issue: String,
    /// Refused or warned.
    pub verdict: CollisionVerdict,
    /// Who holds it.
    pub holder: CollisionHolder,
    /// What collides, in one sentence.
    pub detail: String,
    /// What settles it.
    pub remedy: String,
}

/// Collisions are listed refused first, then by class, then by issue in its natural order.
impl crate::order::Ordered for Collision {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let rank = match self.verdict {
            CollisionVerdict::Refused => 0,
            CollisionVerdict::Warned => 3,
        } + match self.class {
            CollisionClass::IdReused => 0,
            CollisionClass::Retitled => 1,
            CollisionClass::ClaimedElsewhere => 2,
        };
        crate::order::OrderKey::plain(&self.issue, &self.holder.name).ranked(rank)
    }
}

/// Every collision of one head, and what a gate makes of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct CollisionReport {
    /// The version of this shape.
    pub schema: u32,
    /// The head judged, as a commit.
    pub head: String,
    /// Its branch, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The trunk ref it was judged against.
    pub trunk: String,
    /// Their merge-base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_base: Option<String>,
    /// True when the head's change is planning-only.
    pub planning_only: bool,
    /// The issues the head executes.
    pub executes: Vec<String>,
    /// The issue records the head adds.
    pub authors: Vec<String>,
    /// The gate asked, when one was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<CollisionGate>,
    /// Every source, read or not.
    pub sources: Vec<DeliverySource>,
    /// The collisions, refused first.
    pub collisions: Vec<Collision>,
    /// How many are refused.
    pub refused: usize,
    /// How many are warned.
    pub warned: usize,
    /// `clean`, `warned`, `refused`, or `unjudged` (a pull-request gate with no forge).
    pub verdict: String,
    /// The exit code a gate ends with: 0, [`EXIT_REFUSED`] or [`EXIT_UNJUDGED`].
    pub exit_code: u8,
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    crate::worktree::git::run(root, args)
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .map_err(|e| e.to_string())
}

/// The `title:` of an issue record's text, unquoted.
pub(crate) fn title_of(text: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix("title:"))
        .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
        .unwrap_or_default()
}

/// The issue id a record path names: `.ai/repo/project/issues/I0001.yaml` is `I0001`.
pub(crate) fn id_of_path(path: &str) -> Option<String> {
    path.strip_prefix(ISSUES_DIR)?
        .strip_suffix(".yaml")
        .filter(|id| !id.contains('/'))
        .map(str::to_string)
}

/// The issues a branch name names among the plan's: the I1405 rule, then the slug.
pub(crate) fn branch_issues(
    branch: &str,
    ids: &[String],
    slugs: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut found: Vec<String> = issue_of(branch, ids).into_iter().collect();
    for component in branch.split('/') {
        if let Some(id) = slugs.get(component) {
            if !found.contains(id) {
                found.push(id.clone());
            }
        }
    }
    found
}

/// The issues an open pull request claims: those its head branch names, and those its title
/// names when it names at most [`TITLE_LIMIT`] of the declared ones.
pub(crate) fn pull_request_claims(
    pr: &PullRequestObservation,
    ids: &[String],
    slugs: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut claimed = branch_issues(&pr.head_ref, ids, slugs);
    // the limit counts every id the title names, declared or not: a train's title names its
    // members whatever this plan holds
    let titled = issues_named(&json!({ "title": pr.title, "head_ref": "" }));
    if titled.len() <= TITLE_LIMIT {
        for id in titled.into_iter().filter(|id| ids.contains(id)) {
            if !claimed.contains(&id) {
                claimed.push(id);
            }
        }
    }
    claimed
}

/// `collisions.max_age` of the plan model's text, when it declares a positive one.
pub(crate) fn max_age_of(text: &str) -> Option<u64> {
    crate::metadata::yaml::parse_mapping(text)
        .ok()?
        .get("collisions")?
        .get("max_age")?
        .as_u64()
        .filter(|s| *s > 0)
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// What the forge says about the other open pull requests: the observation when it is of
/// this repository and current, else why it cannot be read as one.
pub(crate) fn forge_reading(
    observation: Result<Option<ForgeObservation>, String>,
    pinned: Option<&str>,
    max_age: u64,
    now: u64,
) -> (Option<ForgeObservation>, DeliverySource) {
    let unread = |detail: String, at: Option<String>| DeliverySource {
        name: "forge".into(),
        state: DeliveryReading::Unread,
        detail,
        observed_at: at,
    };
    match observation {
        Err(e) => (None, unread(e, None)),
        Ok(None) => (
            None,
            unread(
                "the forge has never been observed from this checkout; `majordomus prs refresh` records it".into(),
                None,
            ),
        ),
        Ok(Some(o)) if !crate::plan::delivery::observation_is_of(&o.repository, pinned) => {
            let why = crate::plan::delivery::foreign_observation(&o.repository, pinned);
            (None, unread(why, Some(o.observed_at)))
        }
        Ok(Some(o)) => {
            let age = crate::peers::parse_rfc3339(&o.observed_at).map(|t| now.saturating_sub(t));
            match age {
                Some(age) if age <= max_age => {
                    let detail = format!(
                        "{} open pull request(s) of {}, observed {age}s ago",
                        o.pull_requests.len(),
                        o.repository
                    );
                    let at = o.observed_at.clone();
                    (
                        Some(o),
                        DeliverySource {
                            name: "forge".into(),
                            state: DeliveryReading::Read,
                            detail,
                            observed_at: Some(at),
                        },
                    )
                }
                _ => {
                    let why = format!(
                        "the forge observation of {} is stale (observed {}, older than collisions.max_age {max_age}s); `majordomus prs refresh` records a current one",
                        o.repository, o.observed_at
                    );
                    (None, unread(why, Some(o.observed_at)))
                }
            }
        }
    }
}

/// Every collision of the head `asked` names, read from the repository at `root`.
#[allow(clippy::too_many_lines)]
pub(crate) fn judge_repository(
    root: &Path,
    plan: &Plan,
    trunk: &Trunk,
    asked: &Asked<'_>,
    claims: Option<&[LiveClaim]>,
    claims_unread: Option<String>,
) -> Result<CollisionReport, String> {
    let ids: Vec<String> = plan.issues.iter().map(|i| i.id.clone()).collect();
    let slugs: BTreeMap<String, String> = plan
        .issues
        .iter()
        .filter(|i| !i.slug.is_empty())
        .map(|i| (i.slug.clone(), i.id.clone()))
        .collect();
    let trunk_name = trunk.branch.as_deref().unwrap_or("master").to_string();
    let trunk_ref = crate::plan::delivery::trunk_ref(root, &trunk_name)
        .ok_or_else(|| format!("the trunk `{trunk_name}` does not resolve in this clone"))?;
    let head = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{}^{{commit}}", asked.head),
        ],
    )
    .map(|h| h.trim().to_string())
    .map_err(|_| format!("the head `{}` does not resolve in this clone", asked.head))?;
    let branch = match asked.branch {
        Some(b) => Some(b.to_string()),
        None => git(root, &["rev-parse", "--abbrev-ref", asked.head])
            .ok()
            .map(|b| b.trim().to_string())
            .filter(|b| !b.is_empty() && b != "HEAD"),
    };
    let merge_base = git(root, &["merge-base", &head, &trunk_ref])
        .ok()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty());
    let planning_only = match merge_base.as_deref() {
        Some(mb) if mb != head => {
            crate::plan::delivery::planning_only(root, &[(head.as_str(), mb)])?.contains(&head)
        }
        _ => false,
    };

    let mut executes: Vec<String> = Vec::new();
    if !planning_only {
        if let Some(b) = branch.as_deref() {
            executes.extend(branch_issues(b, &ids, &slugs));
        }
    }
    if let Some(i) = asked.issue {
        if !executes.iter().any(|e| e == i) {
            executes.push(i.to_string());
        }
    }

    let mut collisions: Vec<Collision> = Vec::new();
    let mut authors: Vec<String> = Vec::new();
    let mut sources: Vec<DeliverySource> = Vec::new();

    // --- the records the head authors, and the ones it retitles
    if let Some(mb) = merge_base.as_deref() {
        let diff = git(
            root,
            &[
                "diff",
                "--name-status",
                "--no-renames",
                mb,
                head.as_str(),
                "--",
                ISSUES_DIR,
            ],
        )?;
        for line in diff.lines() {
            let Some((status, path)) = line.split_once('\t') else {
                continue;
            };
            let Some(id) = id_of_path(path) else {
                continue;
            };
            let mine = git(root, &["show", &format!("{head}:{path}")])
                .map(|t| title_of(&t))
                .unwrap_or_default();
            match status {
                "A" => {
                    authors.push(id.clone());
                    collisions.extend(authored_elsewhere(
                        root, &head, &trunk_ref, path, &id, &mine,
                    )?);
                }
                "M" => {
                    let before = git(root, &["show", &format!("{mb}:{path}")])
                        .map(|t| title_of(&t))
                        .unwrap_or_default();
                    if !before.is_empty() && before != mine {
                        collisions.push(Collision {
                            class: CollisionClass::Retitled,
                            issue: id.clone(),
                            verdict: CollisionVerdict::Warned,
                            holder: CollisionHolder {
                                kind: "trunk".into(),
                                name: trunk_ref.clone(),
                                title: Some(before.clone()),
                                ..CollisionHolder::default()
                            },
                            detail: format!(
                                "{id} is \"{before}\" on the trunk and this head calls it \"{mine}\": \
                                 a retitle, or another issue written over it after a rebase"
                            ),
                            remedy: format!(
                                "if this head's {id} is a new issue, `majordomus mesh allocate issue` and renumber it"
                            ),
                        });
                    }
                }
                _ => {}
            }
        }
        sources.push(DeliverySource {
            name: "history".into(),
            state: DeliveryReading::Read,
            detail: format!(
                "{} issue record(s) added since the merge-base {}",
                authors.len(),
                mb.get(..11).unwrap_or(mb)
            ),
            observed_at: None,
        });
    } else {
        sources.push(DeliverySource {
            name: "history".into(),
            state: DeliveryReading::Unread,
            detail: format!("the head and `{trunk_ref}` share no history"),
            observed_at: None,
        });
    }

    // --- who else executes what the head executes
    let max_age = std::fs::read_to_string(root.join(super::drift::MODEL))
        .ok()
        .and_then(|t| max_age_of(&t))
        .unwrap_or(DEFAULT_MAX_AGE);
    let pinned = crate::integration::forge::pinned_repository(root);
    let (forge, forge_source) = forge_reading(
        crate::integration::load_observation(root),
        pinned.as_deref(),
        max_age,
        now_seconds(),
    );
    let forge_read = forge.is_some();
    sources.push(forge_source);
    let pulls: &[PullRequestObservation] = forge.as_ref().map_or(&[], |o| &o.pull_requests);
    let own: Option<&PullRequestObservation> = branch
        .as_deref()
        .and_then(|b| pulls.iter().find(|p| p.head_ref == b));
    let own_supersedes: BTreeSet<u64> = own
        .map(|p| crate::integration::classify::declared_supersessions(&p.body).supersedes)
        .unwrap_or_default()
        .into_iter()
        .collect();
    for id in &executes {
        for pr in pulls {
            if Some(pr.head_ref.as_str()) == branch.as_deref() || pr.head_sha == head {
                continue;
            }
            if !pull_request_claims(pr, &ids, &slugs).contains(id) {
                continue;
            }
            // this head replaces theirs: its own body says "Supersedes #N", or theirs says
            // "Superseded by" this head's pull request. A pull request that replaces this
            // head's is no reason to proceed, so the other way round still refuses.
            let theirs = crate::integration::classify::declared_supersessions(&pr.body);
            let superseded = own_supersedes.contains(&pr.number)
                || own.is_some_and(|o| theirs.superseded_by.contains(&o.number));
            if superseded {
                continue;
            }
            collisions.push(Collision {
                class: CollisionClass::ClaimedElsewhere,
                issue: id.clone(),
                verdict: CollisionVerdict::Refused,
                holder: CollisionHolder {
                    kind: "pull_request".into(),
                    name: pr.head_ref.clone(),
                    pull_request: Some(pr.number),
                    title: Some(pr.title.clone()),
                    ..CollisionHolder::default()
                },
                detail: format!(
                    "open pull request #{} ({}, by {}) already executes {id}",
                    pr.number, pr.head_ref, pr.author
                ),
                remedy: format!(
                    "join #{n} or settle with its author; if this head replaces it, say \"Supersedes #{n}\" in this pull request's body; \
                     to start anyway, `majordomus start --issue {id} --alongside \"<reason>\"`",
                    n = pr.number
                ),
            });
        }
        for c in claims.unwrap_or_default() {
            if &c.issue != id || asked.session.is_some_and(|s| is_session(&c.session, s)) {
                continue;
            }
            collisions.push(Collision {
                class: CollisionClass::ClaimedElsewhere,
                issue: id.clone(),
                verdict: CollisionVerdict::Refused,
                holder: CollisionHolder {
                    kind: "claim".into(),
                    name: c.key.clone(),
                    session: Some(c.session.clone()),
                    ..CollisionHolder::default()
                },
                detail: format!("session {} holds a live mesh claim for {id}", c.session),
                remedy: format!(
                    "ask that session (`majordomus mesh who`), or to start anyway `majordomus start --issue {id} --alongside \"<reason>\"`"
                ),
            });
        }
    }
    sources.push(match (&claims, claims_unread) {
        (Some(c), _) => DeliverySource {
            name: "claims".into(),
            state: DeliveryReading::Read,
            detail: format!("{} live mesh claim(s) naming an issue", c.len()),
            observed_at: None,
        },
        (None, why) => DeliverySource {
            name: "claims".into(),
            state: DeliveryReading::Unread,
            detail: why.unwrap_or_else(|| "the mesh was not asked".into()),
            observed_at: None,
        },
    });

    // --- branches that name it, unmerged, with no pull request of their own
    if !executes.is_empty() {
        let listed = git(
            root,
            &[
                "for-each-ref",
                "--no-merged",
                &trunk_ref,
                "--format=%(refname:short)%00%(objectname)",
                "refs/heads",
                "refs/remotes",
            ],
        );
        match listed {
            Ok(text) => {
                let mine = branch.as_deref().unwrap_or("");
                let remotes: Vec<String> = git(root, &["remote"])
                    .map(|r| r.lines().map(|l| l.trim().to_string()).collect())
                    .unwrap_or_default();
                for line in text.lines() {
                    let Some((name, sha)) = line.split_once('\0') else {
                        continue;
                    };
                    // `origin/feature/x` is the branch `feature/x`, a local one is itself
                    let bare = name
                        .split_once('/')
                        .filter(|(remote, _)| remotes.iter().any(|r| r == remote))
                        .map_or(name, |(_, rest)| rest);
                    if bare == mine || sha == head || bare == "HEAD" {
                        continue;
                    }
                    if pulls.iter().any(|p| p.head_ref == bare) {
                        continue;
                    }
                    for id in branch_issues(bare, &ids, &slugs) {
                        if !executes.contains(&id) {
                            continue;
                        }
                        collisions.push(Collision {
                            class: CollisionClass::ClaimedElsewhere,
                            issue: id.clone(),
                            verdict: CollisionVerdict::Warned,
                            holder: CollisionHolder {
                                kind: "branch".into(),
                                name: name.to_string(),
                                commit: Some(sha.to_string()),
                                ..CollisionHolder::default()
                            },
                            detail: format!("branch {name} names {id} and is not merged"),
                            remedy: format!(
                                "see whose it is: `majordomus worktree` and `git log -1 {name}`"
                            ),
                        });
                    }
                }
                sources.push(DeliverySource {
                    name: "branches".into(),
                    state: DeliveryReading::Read,
                    detail: "unmerged local and remote-tracking branches".into(),
                    observed_at: None,
                });
            }
            Err(e) => sources.push(DeliverySource {
                name: "branches".into(),
                state: DeliveryReading::Unread,
                detail: e,
                observed_at: None,
            }),
        }
    }

    Ok(conclude(
        CollisionReport {
            schema: COLLISIONS_SCHEMA,
            head,
            branch,
            trunk: trunk_ref,
            merge_base,
            planning_only,
            executes,
            authors,
            gate: asked.gate,
            sources,
            collisions,
            refused: 0,
            warned: 0,
            verdict: String::new(),
            exit_code: 0,
        },
        forge_read,
    ))
}

/// Whether a claim's holding session key (`<stream>/<session>`, or the bare session) is the
/// session `name`.
pub(crate) fn is_session(key: &str, name: &str) -> bool {
    key == name || key.rsplit_once('/').is_some_and(|(_, s)| s == name)
}

/// The commits on other lines of history that added `path` under another title than `mine`.
fn authored_elsewhere(
    root: &Path,
    head: &str,
    trunk_ref: &str,
    path: &str,
    id: &str,
    mine: &str,
) -> Result<Vec<Collision>, String> {
    let adds = git(
        root,
        &["log", "--all", "--diff-filter=A", "--format=%H", "--", path],
    )?;
    let mut found = Vec::new();
    for commit in adds.lines().map(str::trim).filter(|c| !c.is_empty()) {
        if crate::git::is_ancestor(root, commit, head) == Some(true) {
            continue;
        }
        let theirs = git(root, &["show", &format!("{commit}:{path}")])
            .map(|t| title_of(&t))
            .unwrap_or_default();
        if theirs == mine {
            continue;
        }
        let on_trunk = crate::git::is_ancestor(root, commit, trunk_ref) == Some(true);
        let holder_ref = if on_trunk {
            trunk_ref.to_string()
        } else {
            git(
                root,
                &[
                    "for-each-ref",
                    "--contains",
                    commit,
                    "--format=%(refname:short)",
                    "refs/heads",
                    "refs/remotes",
                ],
            )
            .ok()
            .and_then(|r| {
                r.lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| commit.to_string())
        };
        found.push(Collision {
            class: CollisionClass::IdReused,
            issue: id.to_string(),
            verdict: CollisionVerdict::Refused,
            holder: CollisionHolder {
                kind: if on_trunk { "trunk" } else { "branch" }.into(),
                name: holder_ref.clone(),
                commit: Some(commit.to_string()),
                title: Some(theirs.clone()),
                ..CollisionHolder::default()
            },
            detail: format!(
                "{id} is authored on {holder_ref} as \"{theirs}\" and on this head as \"{mine}\""
            ),
            remedy: format!(
                "`majordomus mesh allocate issue`, then renumber this head's {id} (its file, its id and every reference to it)"
            ),
        });
    }
    Ok(found)
}

/// The verdict and exit code of a report whose collisions are known. Pure.
pub(crate) fn conclude(mut r: CollisionReport, forge_read: bool) -> CollisionReport {
    crate::order::canonical(&mut r.collisions);
    r.refused = r
        .collisions
        .iter()
        .filter(|c| c.verdict == CollisionVerdict::Refused)
        .count();
    r.warned = r.collisions.len() - r.refused;
    let unjudged = r.gate == Some(CollisionGate::Pr) && !forge_read && !r.executes.is_empty();
    let (verdict, code) = if r.refused > 0 {
        ("refused", EXIT_REFUSED)
    } else if unjudged {
        ("unjudged", EXIT_UNJUDGED)
    } else if r.warned > 0 {
        ("warned", 0)
    } else {
        ("clean", 0)
    };
    r.verdict = verdict.into();
    r.exit_code = if r.gate.is_some() { code } else { 0 };
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr(number: u64, head_ref: &str, title: &str, body: &str) -> PullRequestObservation {
        serde_json::from_value(json!({
            "number": number, "title": title, "author": "someone", "head_ref": head_ref,
            "head_sha": format!("sha{number}"), "base_ref": "master", "draft": false,
            "labels": [], "created_at": "", "updated_at": "", "body": body, "checks": [],
            "review_decision": "", "auto_merge": false, "cross_repository": false,
            "latest_reviews": [], "review_requests": [], "author_association": "OWNER",
        }))
        .expect("a pull request observation")
    }

    fn ids() -> Vec<String> {
        ["I0001", "I0002", "I0003", "I0004", "I0005"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn a_pull_request_claims_what_its_branch_and_a_short_title_name() {
        let mut slugs = BTreeMap::new();
        slugs.insert("the-slug".to_string(), "I0005".to_string());
        assert_eq!(
            pull_request_claims(
                &pr(1, "feature/I0001-x", "feat: x (I0002)", ""),
                &ids(),
                &slugs
            ),
            ["I0001", "I0002"]
        );
        assert_eq!(
            pull_request_claims(&pr(2, "fix/the-slug", "fix: y", ""), &ids(), &slugs),
            ["I0005"]
        );
        let train = pr(
            3,
            "integration/train-1",
            "Train: I0001, I0002, I0003, I0004",
            "",
        );
        assert!(
            pull_request_claims(&train, &ids(), &slugs).is_empty(),
            "a combined top claims none of its members"
        );
        assert!(
            pull_request_claims(&pr(4, "chore/x", "I9999 undeclared", ""), &ids(), &slugs)
                .is_empty()
        );
    }

    #[test]
    fn a_claims_session_key_is_its_stream_and_its_session() {
        assert!(is_session("abc-def/s1", "s1"));
        assert!(is_session("s1", "s1"));
        assert!(!is_session("abc-def/s10", "s1"));
        assert!(!is_session("s1/x", "s1"));
    }

    #[test]
    fn a_record_path_title_and_max_age_are_read_as_written() {
        assert_eq!(
            id_of_path(".ai/repo/project/issues/I0001.yaml").as_deref(),
            Some("I0001")
        );
        assert_eq!(id_of_path(".ai/repo/project/issues/x/I0001.yaml"), None);
        assert_eq!(id_of_path("docs/I0001.yaml"), None);
        assert_eq!(title_of("id: I1\ntitle: \"The work\"\n"), "The work");
        assert_eq!(title_of("id: I1\n"), "");
        assert_eq!(max_age_of("collisions:\n  max_age: 900\n"), Some(900));
        assert_eq!(max_age_of("collisions:\n  max_age: 0\n"), None);
        assert_eq!(max_age_of("drift:\n  window: 1\n"), None);
    }

    fn observation(repository: &str, observed_at: &str) -> ForgeObservation {
        serde_json::from_value(json!({
            "schema": crate::integration::forge::OBSERVATION_SCHEMA,
            "repository": repository, "base": "master", "base_sha": "abc",
            "observed_at": observed_at, "pull_requests": [],
        }))
        .expect("an observation")
    }

    #[test]
    fn the_forge_is_read_only_when_it_is_this_repositorys_and_current() {
        let now = crate::peers::parse_rfc3339("2026-11-25T10:00:00Z").unwrap();
        let fresh = || Ok(Some(observation("o/fork", "2026-11-25T09:50:00Z")));
        let (o, s) = forge_reading(fresh(), Some("o/fork"), 900, now);
        assert!(o.is_some());
        assert_eq!(s.state, DeliveryReading::Read);
        assert!(s.detail.contains("observed 600s ago"), "{}", s.detail);

        let (o, s) = forge_reading(fresh(), Some("o/fork"), 300, now);
        assert!(o.is_none() && s.detail.contains("stale"), "{}", s.detail);

        let (o, s) = forge_reading(fresh(), Some("parent/repo"), 900, now);
        assert!(
            o.is_none() && s.detail.contains("is of o/fork"),
            "{}",
            s.detail
        );

        let (o, s) = forge_reading(Ok(None), None, 900, now);
        assert!(o.is_none() && s.detail.contains("never been observed"));
        let (o, s) = forge_reading(Err("unreadable".into()), None, 900, now);
        assert!(o.is_none() && s.detail == "unreadable");
    }

    fn bare(
        gate: Option<CollisionGate>,
        executes: &[&str],
        collisions: Vec<Collision>,
    ) -> CollisionReport {
        CollisionReport {
            schema: COLLISIONS_SCHEMA,
            head: "h".into(),
            branch: None,
            trunk: "origin/master".into(),
            merge_base: None,
            planning_only: false,
            executes: executes.iter().map(|e| (*e).to_string()).collect(),
            authors: vec![],
            gate,
            sources: vec![],
            collisions,
            refused: 0,
            warned: 0,
            verdict: String::new(),
            exit_code: 0,
        }
    }

    fn collision(verdict: CollisionVerdict) -> Collision {
        Collision {
            class: CollisionClass::ClaimedElsewhere,
            issue: "I0001".into(),
            verdict,
            holder: CollisionHolder::default(),
            detail: String::new(),
            remedy: String::new(),
        }
    }

    #[test]
    fn a_gate_refuses_a_refusal_and_a_pull_request_gate_without_a_forge_cannot_judge() {
        let r = conclude(
            bare(
                Some(CollisionGate::Start),
                &["I0001"],
                vec![
                    collision(CollisionVerdict::Warned),
                    collision(CollisionVerdict::Refused),
                ],
            ),
            true,
        );
        assert_eq!(
            (r.verdict.as_str(), r.exit_code, r.refused, r.warned),
            ("refused", EXIT_REFUSED, 1, 1)
        );
        assert_eq!(
            r.collisions[0].verdict,
            CollisionVerdict::Refused,
            "refused first"
        );
        let report = conclude(
            bare(None, &["I0001"], vec![collision(CollisionVerdict::Refused)]),
            true,
        );
        assert_eq!(
            (report.verdict.as_str(), report.exit_code),
            ("refused", 0),
            "a report exits 0"
        );
        let pr = conclude(bare(Some(CollisionGate::Pr), &["I0001"], vec![]), false);
        assert_eq!(
            (pr.verdict.as_str(), pr.exit_code),
            ("unjudged", EXIT_UNJUDGED)
        );
        let push = conclude(bare(Some(CollisionGate::Push), &["I0001"], vec![]), false);
        assert_eq!(
            (push.verdict.as_str(), push.exit_code),
            ("clean", 0),
            "a push is not refused for a forge it could not read"
        );
        let nothing = conclude(bare(Some(CollisionGate::Pr), &[], vec![]), false);
        assert_eq!(
            nothing.verdict, "clean",
            "a head that executes nothing needs no forge"
        );
        let warned = conclude(
            bare(
                Some(CollisionGate::Pr),
                &[],
                vec![collision(CollisionVerdict::Warned)],
            ),
            true,
        );
        assert_eq!((warned.verdict.as_str(), warned.exit_code), ("warned", 0));
    }
}
