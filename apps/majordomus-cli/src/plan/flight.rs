//! Where the work is now: the milestones with work in flight, the one the graph allows next,
//! and the latest release as the releases themselves say it (ADR 0166, decision 4).
//!
//! `plan.roadmap` names a "now" from the stamps and the milestone order alone: the first
//! open milestone, whether or not anybody is working on it. On 2026-10-16 it said "Now M002",
//! a milestone with no work at all, while 20 issues of another milestone had open pull
//! requests. This module answers from what is happening instead:
//!
//! - **Now** is every open milestone holding an issue with work in flight: an open pull
//!   request that names it (`plan.delivery`'s `pr_open`), a live mesh claim made for it, or
//!   a declared `ACTIVE` status. Each one carries the issues that put it there, with why and
//!   the objects that prove it.
//! - **Next** is the first open milestone, in the plan's order, that is not now and whose
//!   unfinished prerequisites are all now: what the graph allows once the work in flight
//!   lands.
//! - **The latest release** is the highest stable version among the release records, the
//!   `v*` tags and the `latest.json` the last successful Pages deployment published, each
//!   reported as read or unread. A record merged after its tag no longer makes a screen show
//!   an older release than the executable that drew it.
//!
//! It is live state, read on every call and stored nowhere; a source that was not read is
//! named with its reason, never treated as empty.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::delivery::{
    self, DeliveryObservation, DeliveryReading, DeliverySource, DeliveryStage, IssueDelivery,
    IssueDeliveryReport,
};
use crate::distribution::release::Releases;
use crate::plan::Plan;
use crate::worktree::Trunk;

/// The version of this answer's shape.
pub(crate) const FLIGHT_SCHEMA: u32 = 1;

/// The statuses that close a milestone or an issue.
const CLOSED: [&str; 3] = ["DONE", "CANCELLED", "SUPERSEDED"];

/// Why an issue is in flight.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InFlight {
    /// An open pull request names it.
    PrOpen,
    /// A live mesh claim was made for it.
    Claim,
    /// Its declared status is ACTIVE.
    Active,
}

/// One object that proves where an issue's work stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Proof {
    /// `branch`, `pull_request`, `merge`, `release`, `publication` or `claim`.
    pub kind: String,
    /// What a reader is shown: `#1298`, a short commit, a tag, a branch.
    pub label: String,
    /// Where the object is, on the forge; absent for an object only this clone holds (a
    /// local branch, a mesh claim), so a link is never drawn to nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

/// One issue: its declared status beside its inferred stage, and what proves the stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct FlightIssue {
    /// The issue id.
    pub issue: String,
    /// Its title.
    pub title: String,
    /// Its declared status, from the stamps.
    pub status: String,
    /// Its inferred delivery stage, from `plan.delivery`.
    pub stage: DeliveryStage,
    /// True when no unread source could have proved a further stage.
    pub settled: bool,
    /// The sources that were not read and could have moved it further.
    pub unread: Vec<String>,
    /// Why it is in flight; empty for an issue that is not.
    pub in_flight: Vec<InFlight>,
    /// The objects that prove the stage (and the claims that put it in flight), in stage
    /// order.
    pub proofs: Vec<Proof>,
}

/// One milestone with the issues an answer shows of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct FlightMilestone {
    /// The milestone id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its declared status.
    pub status: String,
    /// Its issues: the ones in flight for a milestone of `now`, every one for the inspected
    /// milestone.
    pub issues: Vec<FlightIssue>,
}

/// One source's answer to "what is the latest release".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ReleaseReading {
    /// `records`, `tags` or `publication`.
    pub source: String,
    /// Read or unread.
    pub state: DeliveryReading,
    /// The version it names, when it was read and names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// What was read (`.ai/repo/releases/v0.27.0.yaml`, `refs/tags/v0.27.0`,
    /// `<commit>:site/static/releases/latest.json`), or why nothing was.
    pub detail: String,
    /// Where it can be seen on the forge, when it can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

/// The latest release, and every source that was asked for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct FlightRelease {
    /// The highest stable version any read source names; absent when none names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    /// The source that named it (the first in `records`, `tags`, `publication` order when
    /// several agree).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Where it can be seen on the forge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// The version of the executable that answered.
    pub executable: String,
    /// True when the executable's own version is tagged `v<version>`; the latest release is
    /// then never older than it.
    pub executable_tagged: bool,
    /// Every source, read or not.
    pub readings: Vec<ReleaseReading>,
}

/// Where the work is: the answer of `plan.flight`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PlanFlight {
    /// The version of this shape.
    pub schema: u32,
    /// Where and when the delivery half was observed.
    pub observed: DeliveryObservation,
    /// The milestones with work in flight, in the plan's order.
    pub now: Vec<String>,
    /// The first open milestone the graph allows once the work in flight lands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// The milestones of `now`, each with its issues in flight.
    pub milestones: Vec<FlightMilestone>,
    /// The milestone asked for with `milestone`, with every one of its issues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspected: Option<FlightMilestone>,
    /// The latest release.
    pub release: FlightRelease,
    /// Every source, read or not: `plan.delivery`'s, then `mesh`.
    pub sources: Vec<DeliverySource>,
}

/// One live mesh claim made for an issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IssueClaim {
    /// The issue it names.
    pub issue: String,
    /// The claim's key.
    pub key: String,
}

// ---------------------------------------------------------------- the pure half

/// The forge's web address of `repository` (`owner/name`), or `None` for a plan that names
/// none, so no link is built to nowhere.
pub(crate) fn forge_url(repository: &str) -> Option<String> {
    let r = repository.trim().trim_matches('/');
    (r.split('/').count() == 2 && !r.contains(' ') && !r.is_empty())
        .then(|| format!("https://github.com/{r}"))
}

/// The forge repository (`owner/name`) a GitHub remote URL names: `git@github.com:o/r.git`,
/// `https://github.com/o/r`, `ssh://git@github.com/o/r.git`. `None` for any other host.
pub(crate) fn github_repository(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url
        .split_once("github.com:")
        .or_else(|| url.split_once("github.com/"))
        .map(|(_, r)| r)?;
    let r = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = r.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    (parts.next().is_none() && !owner.is_empty() && !name.is_empty())
        .then(|| format!("{owner}/{name}"))
}

/// Which forge repository the proofs link into: the plan's own `repository`, else the one
/// the recorded forge observation names, else the one the `origin` remote names.
pub(crate) fn forge_of(root: &Path, plan: &Plan) -> Option<String> {
    forge_url(&plan.project.repository)
        .or_else(|| {
            crate::integration::load_observation(root)
                .ok()
                .flatten()
                .and_then(|o| forge_url(&o.repository))
        })
        .or_else(|| {
            let out = crate::worktree::git::try_run(root, &["remote", "get-url", "origin"]).ok()?;
            (out.status == Some(0))
                .then(|| github_repository(&String::from_utf8_lossy(&out.stdout)))
                .flatten()
                .and_then(|r| forge_url(&r))
        })
}

/// The objects that prove `d`'s stage, in stage order, each linked where the forge has it.
pub(crate) fn proofs(d: &IssueDelivery, forge: Option<&str>) -> Vec<Proof> {
    let at = |path: String| forge.map(|f| format!("{f}/{path}"));
    let mut out = Vec::new();
    if let Some(b) = &d.branch {
        // a remote-tracking branch is on the forge without its remote's name; a local one is
        // only in this clone
        let href = b
            .remote
            .then(|| b.name.split_once('/').map(|(_, n)| n.to_string()))
            .flatten()
            .and_then(|n| at(format!("tree/{n}")));
        out.push(Proof {
            kind: "branch".into(),
            label: b.name.clone(),
            href,
        });
    }
    if let Some(p) = &d.pr_open {
        out.push(Proof {
            kind: "pull_request".into(),
            label: format!("#{}", p.number),
            href: at(format!("pull/{}", p.number)),
        });
    }
    if let Some(m) = &d.merged {
        if let Some(n) = m.pull_request {
            out.push(Proof {
                kind: "pull_request".into(),
                label: format!("#{n}"),
                href: at(format!("pull/{n}")),
            });
        }
        out.push(Proof {
            kind: "merge".into(),
            label: m.commit.chars().take(11).collect(),
            href: at(format!("commit/{}", m.commit)),
        });
    }
    if let Some(r) = &d.released {
        out.push(Proof {
            kind: "release".into(),
            label: r.tag.clone(),
            href: at(format!("releases/tag/{}", r.tag)),
        });
    }
    if let Some(p) = &d.deployed {
        let href = p
            .latest_json
            .split_once(':')
            .and_then(|(commit, path)| at(format!("blob/{commit}/{path}")));
        out.push(Proof {
            kind: "publication".into(),
            label: format!("latest.json names {}", p.version),
            href,
        });
    }
    out
}

/// Why `d` is in flight, given the issues live claims name.
fn in_flight(d: &IssueDelivery, claimed: &BTreeSet<&str>) -> Vec<InFlight> {
    let mut why = Vec::new();
    if d.pr_open.is_some() {
        why.push(InFlight::PrOpen);
    }
    if claimed.contains(d.issue.as_str()) {
        why.push(InFlight::Claim);
    }
    if d.status == "ACTIVE" {
        why.push(InFlight::Active);
    }
    why
}

fn flight_issue(
    d: &IssueDelivery,
    claims: &[IssueClaim],
    claimed: &BTreeSet<&str>,
    forge: Option<&str>,
) -> FlightIssue {
    let mut proofs = proofs(d, forge);
    proofs.extend(claims.iter().filter(|c| c.issue == d.issue).map(|c| Proof {
        kind: "claim".into(),
        label: c.key.clone(),
        href: None,
    }));
    FlightIssue {
        issue: d.issue.clone(),
        title: d.title.clone(),
        status: d.status.clone(),
        stage: d.stage,
        settled: d.settled,
        unread: d.unread.clone(),
        in_flight: in_flight(d, claimed),
        proofs,
    }
}

/// Now, next, the milestones of now with their issues in flight, and the inspected
/// milestone with every issue. Pure: the plan, the delivery and the claims are in memory.
///
/// `claims` is `None` when the mesh was not read: its claims then put nothing in flight,
/// and the answer's `mesh` source says why.
pub(crate) fn derive(
    plan: &Plan,
    report: &IssueDeliveryReport,
    claims: Option<&[IssueClaim]>,
    inspect: Option<&str>,
    forge: Option<&str>,
) -> (
    Vec<String>,
    Option<String>,
    Vec<FlightMilestone>,
    Option<FlightMilestone>,
) {
    let claims = claims.unwrap_or_default();
    let claimed: BTreeSet<&str> = claims.iter().map(|c| c.issue.as_str()).collect();
    let by_milestone: BTreeMap<&str, Vec<&IssueDelivery>> =
        report.issues.iter().fold(BTreeMap::new(), |mut m, d| {
            m.entry(d.milestone.as_str())
                .or_insert_with(Vec::new)
                .push(d);
            m
        });
    let open = |status: &str| !CLOSED.contains(&status);

    let mut now = Vec::new();
    let mut milestones = Vec::new();
    for m in plan.roadmap() {
        if !open(&m.status) {
            continue;
        }
        let work: Vec<FlightIssue> = by_milestone
            .get(m.id.as_str())
            .into_iter()
            .flatten()
            .filter(|d| open(&d.status))
            .map(|d| flight_issue(d, claims, &claimed, forge))
            .filter(|i| !i.in_flight.is_empty())
            .collect();
        if !work.is_empty() {
            now.push(m.id.clone());
            milestones.push(FlightMilestone {
                id: m.id.clone(),
                title: m.title.clone(),
                status: m.status.clone(),
                issues: work,
            });
        }
    }
    let next = plan
        .roadmap()
        .into_iter()
        .filter(|m| open(&m.status) && !now.contains(&m.id))
        .find(|m| m.blocked_by.iter().all(|b| now.contains(b)))
        .map(|m| m.id.clone());
    let inspected = inspect
        .and_then(|id| plan.milestone(id))
        .map(|m| FlightMilestone {
            id: m.id.clone(),
            title: m.title.clone(),
            status: m.status.clone(),
            issues: by_milestone
                .get(m.id.as_str())
                .into_iter()
                .flatten()
                .map(|d| flight_issue(d, claims, &claimed, forge))
                .collect(),
        });
    (now, next, milestones, inspected)
}

/// A version as [`delivery::version_key`] orders it.
type VersionKey = (Vec<u64>, bool, String);

/// The highest stable version among the read readings, the first source in their order
/// winning a tie.
pub(crate) fn latest_of(readings: &[ReleaseReading]) -> Option<&ReleaseReading> {
    let mut best: Option<(&ReleaseReading, VersionKey)> = None;
    for r in readings.iter().filter(|r| r.state == DeliveryReading::Read) {
        let Some(key) = r.version.as_deref().and_then(delivery::version_key) else {
            continue;
        };
        if !key.1 {
            // a pre-release is not what an unpinned installation resolves to
            continue;
        }
        if best.as_ref().is_none_or(|(_, b)| key > *b) {
            best = Some((r, key));
        }
    }
    best.map(|(r, _)| r)
}

/// The highest stable `v*` tag among `tags` (one per line), when there is one.
pub(crate) fn highest_tag(tags: &str) -> Option<String> {
    tags.lines()
        .map(str::trim)
        .filter(|t| t.starts_with('v'))
        .filter_map(|t| delivery::version_key(t).map(|k| (k, t)))
        .filter(|(k, _)| k.1)
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, t)| t.to_string())
}

// ---------------------------------------------------------------- the live half

fn read(
    source: &str,
    version: Option<String>,
    detail: String,
    href: Option<String>,
) -> ReleaseReading {
    ReleaseReading {
        source: source.into(),
        state: DeliveryReading::Read,
        version,
        detail,
        href,
    }
}

fn unread(source: &str, detail: String) -> ReleaseReading {
    ReleaseReading {
        source: source.into(),
        state: DeliveryReading::Unread,
        version: None,
        detail,
        href: None,
    }
}

/// Ask the release records, the tags and the publication what the latest release is.
pub(crate) fn release(root: &Path, forge: Option<&str>, executable: &str) -> FlightRelease {
    let at = |path: String| forge.map(|f| format!("{f}/{path}"));
    let records = match Releases::load(root) {
        Ok(r) => match r.latest_stable() {
            Some(l) => read(
                "records",
                Some(l.version.clone()),
                format!("{}/{}.yaml", crate::distribution::release::DIR, l.tag),
                at(format!("releases/tag/{}", l.tag)),
            ),
            None => read(
                "records",
                None,
                format!(
                    "no stable release record under {}",
                    crate::distribution::release::DIR
                ),
                None,
            ),
        },
        Err(e) => unread("records", e.to_string()),
    };
    let tags_out = crate::worktree::git::try_run(root, &["tag", "--list", "v*"]);
    let (tags, own_tagged) = match tags_out {
        Ok(out) if out.status == Some(0) => {
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            let own = format!("v{executable}");
            let tagged = text.lines().any(|t| t.trim() == own);
            let reading = match highest_tag(&text) {
                Some(t) => read(
                    "tags",
                    Some(t.trim_start_matches('v').to_string()),
                    format!("refs/tags/{t}"),
                    at(format!("releases/tag/{t}")),
                ),
                None => read("tags", None, "no stable v* tag in this clone".into(), None),
            };
            (reading, tagged)
        }
        Ok(out) => (
            unread(
                "tags",
                format!("`git tag --list` failed: {}", out.stderr.trim()),
            ),
            false,
        ),
        Err(e) => (unread("tags", e.to_string()), false),
    };
    let publication = match delivery::published_live(root) {
        Ok((version, latest_json, _)) => {
            let href = latest_json
                .split_once(':')
                .and_then(|(c, p)| at(format!("blob/{c}/{p}")));
            read("publication", Some(version), latest_json, href)
        }
        Err(e) => unread("publication", e),
    };
    let readings = vec![records, tags, publication];
    let latest = latest_of(&readings).cloned();
    FlightRelease {
        latest: latest.as_ref().and_then(|r| r.version.clone()),
        source: latest.as_ref().map(|r| r.source.clone()),
        href: latest.and_then(|r| r.href),
        executable: executable.to_string(),
        executable_tagged: own_tagged,
        readings,
    }
}

/// Where the work is, read live from the repository at `root`, its mesh claims (`None` with
/// the reason when the mesh is not running here) and the releases.
pub(crate) fn observe(
    root: &Path,
    plan: &Plan,
    trunk: &Trunk,
    claims: Result<Vec<IssueClaim>, String>,
    inspect: Option<&str>,
) -> Result<PlanFlight, String> {
    let report = delivery::infer(root, plan, trunk, None, None)?;
    let mut sources = report.sources.clone();
    sources.push(match &claims {
        Ok(c) => DeliverySource {
            name: "mesh".into(),
            state: DeliveryReading::Read,
            detail: format!("{} live claim(s) naming an issue", c.len()),
            observed_at: Some(report.observed.observed_at.clone()),
        },
        Err(why) => DeliverySource {
            name: "mesh".into(),
            state: DeliveryReading::Unread,
            detail: why.clone(),
            observed_at: None,
        },
    });
    let forge = forge_of(root, plan);
    let (now, next, milestones, inspected) = derive(
        plan,
        &report,
        claims.as_deref().ok(),
        inspect,
        forge.as_deref(),
    );
    Ok(PlanFlight {
        schema: FLIGHT_SCHEMA,
        observed: report.observed.clone(),
        now,
        next,
        milestones,
        inspected,
        release: release(root, forge.as_deref(), env!("CARGO_PKG_VERSION")),
        sources,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(source: &str, version: Option<&str>) -> ReleaseReading {
        read(source, version.map(str::to_string), String::new(), None)
    }

    /// The highest stable version wins, whichever source names it; a tie goes to the first
    /// source; an unread source and a pre-release name nothing.
    #[test]
    fn the_latest_release_is_the_highest_any_source_names() {
        let lagging = [
            reading("records", Some("0.26.0")),
            reading("tags", Some("0.27.0")),
            unread("publication", "never observed".into()),
        ];
        let l = latest_of(&lagging).expect("a latest release");
        assert_eq!(
            (l.source.as_str(), l.version.as_deref()),
            ("tags", Some("0.27.0")),
            "a record merged after its tag does not hold the release back"
        );
        let tie = [
            reading("records", Some("0.27.0")),
            reading("tags", Some("0.27.0")),
        ];
        assert_eq!(latest_of(&tie).map(|r| r.source.as_str()), Some("records"));
        let numeric = [
            reading("records", Some("0.9.0")),
            reading("tags", Some("0.10.0")),
        ];
        assert_eq!(
            latest_of(&numeric).and_then(|r| r.version.as_deref()),
            Some("0.10.0"),
            "versions compare as numbers, not text"
        );
        let pre = [
            reading("records", Some("0.27.0")),
            reading("tags", Some("0.28.0-rc1")),
        ];
        assert_eq!(
            latest_of(&pre).and_then(|r| r.version.as_deref()),
            Some("0.27.0")
        );
        assert!(latest_of(&[unread("records", "x".into())]).is_none());
    }

    #[test]
    fn the_highest_tag_is_stable_and_numeric() {
        assert_eq!(
            highest_tag("v0.9.0\nv0.10.0\nv0.11.0-rc1\nnot-a-version\n").as_deref(),
            Some("v0.10.0")
        );
        assert_eq!(highest_tag(""), None);
    }

    #[test]
    fn the_forge_is_linked_only_when_the_plan_names_a_repository() {
        assert_eq!(
            forge_url("korczis/prismatic-majordomus").as_deref(),
            Some("https://github.com/korczis/prismatic-majordomus")
        );
        assert_eq!(forge_url(""), None);
        assert_eq!(forge_url("just-a-name"), None);
        for url in [
            "git@github.com:korczis/prismatic-majordomus.git",
            "https://github.com/korczis/prismatic-majordomus",
            "ssh://git@github.com/korczis/prismatic-majordomus.git\n",
        ] {
            assert_eq!(
                github_repository(url).as_deref(),
                Some("korczis/prismatic-majordomus"),
                "{url}"
            );
        }
        assert_eq!(github_repository("https://gitlab.com/o/r.git"), None);
    }
}
