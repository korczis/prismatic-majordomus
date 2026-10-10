//! Pull-request integration (ADR 0101): the disposition model, the planner and the
//! executor, proved over a scripted repository — no forge, no network.
//!
//! The scripted repository is a small world: a master generation counter, and pull
//! requests whose heads contain some generation of master, may depend on another, may fail
//! their required check, may become redundant or conflicting when another lands. It answers
//! observations the way the forge and git would, and it counts them, which is how the
//! property this subsystem exists for — *every merge is preceded by a fresh observation,
//! and no plan survives a merge* — is asserted rather than trusted.

use std::collections::{BTreeMap, BTreeSet};

use crate::integration::drain::{self, DrainStepOutcome, Integrator};
use crate::integration::{
    build_queue, CheckObservation, CheckRunState, ForgeObservation, IntegrationQueue,
    PullRequestAssessment, PullRequestDisposition, PullRequestObservation, PullRequestReview,
    RelationToMaster, RequiredCheckState, OBSERVATION_SCHEMA,
};
use proptest::prelude::*;

// ---------------------------------------------------------------- the scripted world

#[derive(Debug, Clone)]
struct Sim {
    number: u64,
    head: String,
    /// The master generation the head contains.
    contains: u32,
    depends_on: Option<u64>,
    failing: bool,
    redundant_after: Option<u64>,
    conflicts_after: Option<u64>,
    created: String,
    draft: bool,
    labels: Vec<String>,
    /// The forge's review decision, verbatim.
    review: &'static str,
    /// The branch it asks to merge into.
    base: &'static str,
    /// The branch it asks to merge.
    head_ref: String,
    /// Whether the head lives in a fork.
    cross_repository: bool,
    /// The authored paths its merge would change.
    paths: Vec<String>,
    /// Its required check is still running.
    ci_pending: bool,
    /// Its required check never reported on the head.
    ci_unreported: bool,
    /// The forge has auto-merge armed on it.
    auto_merge: bool,
    /// More lines of its body, after any dependency line: supersession declarations.
    body: String,
    /// What the forge says its author is to the repository.
    association: &'static str,
    /// Whether the pull requests that mention it were read whole.
    references: crate::integration::CrossReferenceRead,
    /// The pull requests whose heads this head already holds: the links below it in a train.
    carries: Vec<u64>,
}

/// How a pull request stopped being open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gone {
    /// Merged with a merge commit: master contains its head.
    Merged,
    /// Merged somewhere master does not contain: the forge says merged, and master holds
    /// neither its head nor a merge commit of it.
    Squashed,
    /// Closed unmerged.
    Closed,
    /// Closed, not merged, after a batch carried its head into master.
    Batched,
    /// Closed, not merged, after its branch was reset onto a commit of master: its head is on
    /// master, and the forge names no file it changes.
    Reset,
}

fn sim(number: u64) -> Sim {
    Sim {
        number,
        head: format!("h{number}.0"),
        contains: 0,
        depends_on: None,
        failing: false,
        redundant_after: None,
        conflicts_after: None,
        created: format!("2026-09-{:02}T00:00:00Z", number.min(28)),
        draft: false,
        labels: Vec::new(),
        review: "",
        base: "master",
        head_ref: format!("feature/{number}"),
        cross_repository: false,
        paths: vec![format!("docs/{number}.md")],
        ci_pending: false,
        ci_unreported: false,
        auto_merge: false,
        body: String::new(),
        // the repository's owner, read whole: what a single-owner repository's queue is
        association: "OWNER",
        references: crate::integration::CrossReferenceRead::Whole,
        carries: Vec::new(),
    }
}

/// Something that happens on the forge between two of the executor's observations.
#[derive(Debug, Clone)]
enum Meanwhile {
    /// The author pushes: the head moves, the master it contains does not.
    HeadMoves(u64),
    /// Somebody closes it.
    Closes(u64),
    /// Somebody puts a label on it.
    Labelled(u64, &'static str),
    /// The forge's review decision becomes this.
    Reviewed(u64, &'static str),
    /// Somebody arms auto-merge on it.
    AutoMergeArmed(u64),
    /// The forge cannot be read, with these words: this observation fails.
    Outage(&'static str),
}

#[derive(Debug)]
struct World {
    master: u32,
    open: Vec<Sim>,
    merged: Vec<u64>,
    observations: usize,
    /// On this observation (1-based), somebody else merges into master first.
    master_moves_on: Option<usize>,
    /// On each observation (1-based) named, what happens on the forge just before it.
    meanwhile: Vec<(usize, Meanwhile)>,
    /// Observation counts at which each merge happened.
    merged_after_observation: Vec<usize>,
    /// Whether the branch protection requires a review; `None` when it cannot be read.
    reviews_required: Option<bool>,
    /// The forge refuses to merge these.
    merge_refuses: BTreeSet<u64>,
    /// The forge does not show a merge it accepted.
    verify_fails: bool,
    /// On this observation (1-based), the audit trail at this path stops taking appends:
    /// a directory is put where the file was.
    sabotage_on: Option<(usize, std::path::PathBuf)>,
    /// How many times a merge reached the forge.
    merge_calls: usize,
    /// How many times a refresh reached the branch.
    refresh_calls: usize,
    /// The forge refuses to close these.
    close_refuses: BTreeSet<u64>,
    /// How many times a closure reached the forge.
    close_calls: usize,
    /// Bringing master into a branch fails.
    refresh_fails: bool,
    /// The merge methods the repository's settings allow, in the forge's words.
    merge_methods: Vec<&'static str>,
    /// Pull requests no longer open, and how they went: what the forge reports of those a
    /// supersession names.
    gone: Vec<(Sim, Gone)>,
    /// Where each merge landed: the master it was made on and the head it merged.
    merged_onto: BTreeMap<u64, (String, String)>,
    /// Somebody else's merge lands on master just before the forge merges these.
    foreign_merge_before: BTreeSet<u64>,
    /// The forge merges these, and its answer is lost on the way back.
    answer_lost_after_landing: BTreeSet<u64>,
    /// The forge's answer is lost and these never merged.
    answer_lost_before_landing: BTreeSet<u64>,
    /// How many times a verification was asked for.
    verify_calls: usize,
    /// The repository whose trail the executor's refusals are folded from, as `queue_of`
    /// does: set it to see the queue the command line would see.
    trail_root: Option<std::path::PathBuf>,
    /// The next observations fail with these words, once each.
    outages: Vec<String>,
    /// Every closure comment that reached the forge.
    close_comments: Vec<String>,
    /// On this observation (1-based) the queue's policy names no merge method, whatever
    /// its pull requests say: an observation that contradicts itself.
    no_method_on: Option<usize>,
    /// Which pull request's merge made each master generation. A generation absent here was
    /// somebody else's merge.
    landed: BTreeMap<u32, u64>,
}

impl Default for World {
    fn default() -> Self {
        World {
            master: 0,
            open: Vec::new(),
            merged: Vec::new(),
            observations: 0,
            master_moves_on: None,
            meanwhile: Vec::new(),
            merged_after_observation: Vec::new(),
            reviews_required: Some(false),
            merge_refuses: BTreeSet::new(),
            verify_fails: false,
            sabotage_on: None,
            merge_calls: 0,
            refresh_calls: 0,
            close_refuses: BTreeSet::new(),
            close_calls: 0,
            refresh_fails: false,
            merge_methods: vec!["merge"],
            gone: Vec::new(),
            merged_onto: BTreeMap::new(),
            foreign_merge_before: BTreeSet::new(),
            answer_lost_after_landing: BTreeSet::new(),
            answer_lost_before_landing: BTreeSet::new(),
            verify_calls: 0,
            trail_root: None,
            outages: Vec::new(),
            close_comments: Vec::new(),
            no_method_on: None,
            landed: BTreeMap::new(),
        }
    }
}

fn master_sha(g: u32) -> String {
    format!("m{g}")
}

impl World {
    fn observation(&self) -> ForgeObservation {
        ForgeObservation {
            schema: OBSERVATION_SCHEMA,
            repository: "owner/repo".into(),
            base: "master".into(),
            base_sha: master_sha(self.master),
            observed_at: format!("t{}", self.observations),
            required_checks: Some(vec!["ci".into()]),
            review_policy: self.reviews_required.map(|r| super::ReviewPolicy {
                approvals: u64::from(r),
                ..Default::default()
            }),
            up_to_date_required: Some(true),
            merge_methods: self.merge_methods.iter().map(|m| m.to_string()).collect(),
            pull_requests: self.open.iter().map(observe_pr).collect(),
            resolved: self.resolved(),
            delete_branch_on_merge: None,
        }
    }

    /// What the forge reports of the pull requests no longer open, through the adapter's own
    /// selection: the ones that mention an open one (its cross-references: here every one
    /// that is gone, as a source with its author's association), and each one viewed.
    fn resolved(&self) -> BTreeMap<u64, super::forge::ResolvedPullRequest> {
        let json = |(s, how): &(Sim, Gone)| {
            let merged = matches!(how, Gone::Merged | Gone::Squashed);
            serde_json::json!({
                "number": s.number,
                "state": if merged { "MERGED" } else { "CLOSED" },
                "headRefOid": s.head,
                "body": observe_pr(s).body,
                "authorAssociation": s.association,
                "isCrossRepository": s.cross_repository,
                "author": {"login": "someone"},
                "baseRefName": s.base,
                "changedFiles": u64::from(*how != Gone::Reset),
            })
        };
        let closed: Vec<serde_json::Value> = self.gone.iter().map(json).collect();
        let open: Vec<PullRequestObservation> = self.open.iter().map(observe_pr).collect();
        super::forge::resolved_for(&open, &serde_json::Value::Array(closed), |m| {
            self.gone.iter().find(|(s, _)| s.number == m).map(json)
        })
    }

    /// Whether merging `s` yields its own tree although its head lacks master: every master
    /// generation it does not contain was the merge of a pull request whose head it holds.
    fn carried(&self, s: &Sim) -> bool {
        s.contains < self.master
            && (s.contains + 1..=self.master)
                .all(|g| self.landed.get(&g).is_some_and(|pr| s.carries.contains(pr)))
    }

    fn relation(&self, n: u64) -> RelationToMaster {
        if let Some((_, how)) = self.gone.iter().find(|(s, _)| s.number == n) {
            // a pull request no longer open: its head is on master when it was merged with a
            // merge commit, and when a batch carried it in
            return if matches!(how, Gone::Merged | Gone::Batched | Gone::Reset) {
                RelationToMaster::Contained
            } else {
                RelationToMaster::Behind {
                    behind: 1,
                    authored: Vec::new(),
                }
            };
        }
        let s = self.open.iter().find(|s| s.number == n).expect("open");
        if s.redundant_after.is_some_and(|r| self.merged.contains(&r)) {
            return RelationToMaster::Superseded;
        }
        if s.conflicts_after.is_some_and(|r| self.merged.contains(&r)) {
            return RelationToMaster::Conflicting {
                paths: vec![format!("src/{n}.rs")],
            };
        }
        let authored = s.paths.clone();
        if s.contains == self.master {
            RelationToMaster::UpToDate { authored }
        } else if self.carried(s) {
            RelationToMaster::CarriesMaster {
                behind: u64::from(self.master - s.contains),
                authored,
            }
        } else {
            RelationToMaster::Behind {
                behind: u64::from(self.master - s.contains),
                authored,
            }
        }
    }

    fn queue(&self) -> IntegrationQueue {
        let obs = self.observation();
        build_queue(&obs, &master_sha(self.master), |p| self.relation(p.number))
    }

    /// `n` stops being open, as `how` says.
    fn gone_as(&mut self, n: u64, how: Gone) {
        if let Some(i) = self.open.iter().position(|s| s.number == n) {
            let s = self.open.remove(i);
            self.gone.push((s, how));
        }
    }

    fn happen(&mut self, what: &Meanwhile) {
        match what {
            Meanwhile::HeadMoves(n) => {
                let s = self.open.iter_mut().find(|s| s.number == *n).expect("open");
                s.head = format!("{}+", s.head);
            }
            Meanwhile::Closes(n) => self.gone_as(*n, Gone::Closed),
            Meanwhile::Labelled(n, label) => {
                let s = self.open.iter_mut().find(|s| s.number == *n).expect("open");
                s.labels.push((*label).to_string());
            }
            Meanwhile::Reviewed(n, decision) => {
                let s = self.open.iter_mut().find(|s| s.number == *n).expect("open");
                s.review = decision;
            }
            Meanwhile::AutoMergeArmed(n) => {
                let s = self.open.iter_mut().find(|s| s.number == *n).expect("open");
                s.auto_merge = true;
            }
            Meanwhile::Outage(words) => self.outages.push((*words).to_string()),
        }
    }
}

fn observe_pr(s: &Sim) -> PullRequestObservation {
    PullRequestObservation {
        number: s.number,
        title: format!("change {}", s.number),
        author: "someone".into(),
        head_ref: s.head_ref.clone(),
        head_sha: s.head.clone(),
        base_ref: s.base.into(),
        draft: s.draft,
        labels: s.labels.clone(),
        created_at: s.created.clone(),
        updated_at: s.created.clone(),
        body: s
            .depends_on
            .map(|d| format!("Depends on #{d}."))
            .into_iter()
            .chain((!s.body.is_empty()).then(|| s.body.clone()))
            .collect::<Vec<_>>()
            .join("\n"),
        checks: if s.ci_unreported {
            Vec::new()
        } else {
            vec![CheckObservation {
                name: "ci".into(),
                state: if s.failing {
                    CheckRunState::Failed
                } else if s.ci_pending {
                    CheckRunState::Pending
                } else {
                    CheckRunState::Passed
                },
                ..Default::default()
            }]
        },
        review_decision: s.review.into(),
        auto_merge: s.auto_merge,
        cross_repository: s.cross_repository,
        // an approval the forge reports is one given on this head
        latest_reviews: if s.review == "APPROVED" {
            vec![super::ReviewObservation {
                author: "reviewer".into(),
                state: "APPROVED".into(),
                commit: s.head.clone(),
            }]
        } else {
            Vec::new()
        },
        review_requests: Vec::new(),
        author_association: s.association.into(),
        cross_references: s.references,
    }
}

impl Integrator for World {
    fn observe(&mut self) -> Result<IntegrationQueue, String> {
        self.observations += 1;
        if self.master_moves_on == Some(self.observations) {
            // somebody else's merge lands between this executor's two observations
            self.master += 1;
        }
        let now: Vec<Meanwhile> = self
            .meanwhile
            .iter()
            .filter(|(at, _)| *at == self.observations)
            .map(|(_, what)| what.clone())
            .collect();
        for what in &now {
            self.happen(what);
        }
        if let Some((_, trail)) = self
            .sabotage_on
            .as_ref()
            .filter(|(at, _)| *at == self.observations)
        {
            unwritable(trail);
        }
        if !self.outages.is_empty() {
            return Err(self.outages.remove(0));
        }
        let mut q = self.queue();
        if let Some(root) = &self.trail_root {
            super::executor_feedback(&mut q, &drain::events(root));
        }
        if self.no_method_on == Some(self.observations) {
            q.policy.merge_method = None;
        }
        Ok(q)
    }

    fn merge(&mut self, pr: u64, head_sha: &str, method: &str) -> Result<(), String> {
        self.merge_calls += 1;
        assert_eq!(method, "merge", "the repository's merge method");
        if self.merge_refuses.contains(&pr) {
            return Err(
                "Pull request is not mergeable: the base branch policy prohibits the merge".into(),
            );
        }
        let i = self
            .open
            .iter()
            .position(|s| s.number == pr)
            .ok_or("not open")?;
        assert_eq!(
            self.open[i].head, head_sha,
            "merged a head that was not decided on"
        );
        assert!(
            self.open[i].contains == self.master || self.carried(&self.open[i]),
            "merged a head whose merge does not yield its own tree"
        );
        assert!(!self.open[i].failing, "merged a failing pull request");
        assert!(
            !self.open[i].ci_pending && !self.open[i].ci_unreported,
            "merged a pull request whose required check has not passed"
        );
        assert!(
            self.open[i].labels.is_empty(),
            "merged a pull request that carries a label"
        );
        if self.answer_lost_before_landing.contains(&pr) {
            return Err("Post https://api.github.com/graphql: timed out".into());
        }
        if self.foreign_merge_before.contains(&pr) {
            // another change lands first; the forge then merges this one on top of it
            self.master += 1;
        }
        let head = self.open[i].head.clone();
        self.merged_onto.insert(pr, (master_sha(self.master), head));
        self.gone_as(pr, Gone::Merged);
        self.merged.push(pr);
        self.master += 1;
        self.landed.insert(self.master, pr);
        self.merged_after_observation.push(self.observations);
        if self.answer_lost_after_landing.contains(&pr) {
            return Err("Post https://api.github.com/graphql: timed out".into());
        }
        Ok(())
    }

    fn verify(
        &mut self,
        pr: u64,
        at: &crate::integration::EvaluatedAgainst,
        _method: &str,
    ) -> Result<drain::Landed, drain::NotLanded> {
        self.verify_calls += 1;
        let Some((onto, head)) = self.merged_onto.get(&pr).cloned() else {
            return Err(drain::NotLanded::NotMerged(format!(
                "the forge still says OPEN for #{pr}"
            )));
        };
        if self.verify_fails {
            return Err(drain::NotLanded::Unproved(format!(
                "the forge does not show #{pr} merged"
            )));
        }
        // what a real merge commit's parents would say: the master it landed on and the head
        if onto != at.master_sha || head != at.head_sha {
            return Err(drain::NotLanded::Unproved(format!(
                "unexpected_master: #{pr} landed on {onto} as {head}, not on {} as {}",
                at.master_sha, at.head_sha
            )));
        }
        Ok(drain::Landed {
            master_after: master_sha(self.master),
            merge_commit: Some(format!("merge-{pr}")),
        })
    }

    fn refresh_branch(&mut self, a: &PullRequestAssessment, _base: &str) -> Result<String, String> {
        self.refresh_calls += 1;
        if self.refresh_fails {
            return Err("the merge of master conflicts after all".into());
        }
        let master = self.master;
        let s = self
            .open
            .iter_mut()
            .find(|s| s.number == a.number)
            .ok_or("not open")?;
        assert_eq!(s.head, a.evaluated_against.head_sha);
        assert!(
            s.labels.is_empty(),
            "refreshed a pull request that carries a label"
        );
        s.contains = master;
        s.head = format!("h{}.{master}", s.number);
        Ok(s.head.clone())
    }

    fn close(&mut self, pr: u64, head_sha: &str, comment: &str) -> Result<(), String> {
        self.close_calls += 1;
        self.close_comments.push(comment.to_string());
        if self.close_refuses.contains(&pr) {
            return Err("HTTP 403: Resource not accessible by integration".into());
        }
        let s = self
            .open
            .iter()
            .find(|s| s.number == pr)
            .ok_or("not open")?;
        // the forge side of the guard: never a head a person pushed to meanwhile
        if s.head != head_sha {
            return Err(format!("#{pr}'s head moved to {}", s.head));
        }
        self.gone_as(pr, Gone::Closed);
        Ok(())
    }
}

/// A fresh, empty temporary directory that no other test shares. The process id and the
/// clock alone collided: tests started in the same microsecond were handed one directory, and
/// the second `git init` found the first one's files. A per-process counter makes the name
/// unique, and `create_dir` (not `create_dir_all`) refuses a directory that already exists.
fn unique_temp(prefix: &str) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}-{n}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&d).unwrap_or_else(|e| panic!("{}: {e}", d.display()));
    d
}

/// A scratch repository: the audit trail is the repository's, under its common git
/// directory, so a scratch root is a git repository of its own.
fn scratch() -> std::path::PathBuf {
    let d = unique_temp("mj-integration");
    git(&d, &["init", "-q"]);
    d
}

/// Where the repository's audit trail is: one file under the common git directory.
fn trail_of(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".git/majordomus/integration/events.jsonl")
}

/// Make the audit trail refuse every append (and every read): a directory where the file is.
fn unwritable(trail: &std::path::Path) {
    let _ = std::fs::remove_file(trail);
    std::fs::create_dir_all(trail).unwrap();
}

fn disposition(q: &IntegrationQueue, n: u64) -> PullRequestDisposition {
    q.get(n)
        .unwrap_or_else(|| panic!("#{n} not in queue"))
        .disposition
}

// ---------------------------------------------------------------- classification

#[test]
fn each_disposition_is_reached_by_its_own_evidence() {
    let mut w = World::default();
    let mut ready = sim(1);
    ready.created = "2026-09-01T00:00:00Z".into();
    let mut draft = sim(2);
    draft.draft = true;
    let mut held = sim(3);
    held.labels = vec!["Do-Not-Merge".into()];
    let mut failing = sim(4);
    failing.failing = true;
    let mut dependent = sim(5);
    dependent.depends_on = Some(1);
    let mut redundant = sim(6);
    redundant.redundant_after = Some(99);
    let mut conflicting = sim(7);
    conflicting.conflicts_after = Some(98);
    w.open = vec![
        ready,
        draft,
        held,
        failing,
        dependent,
        redundant,
        conflicting,
        sim(8),
    ];
    w.merged = vec![99, 98];
    w.master = 0;
    // #8 is behind: its head contains an older master
    w.open[7].contains = 0;
    w.master = 1;
    for s in w.open.iter_mut().take(7) {
        s.contains = 1;
    }
    let q = w.queue();
    assert_eq!(disposition(&q, 1), PullRequestDisposition::Ready);
    assert_eq!(disposition(&q, 2), PullRequestDisposition::Draft);
    assert_eq!(disposition(&q, 3), PullRequestDisposition::Blocked);
    assert_eq!(disposition(&q, 4), PullRequestDisposition::NeedsRepair);
    assert_eq!(
        disposition(&q, 5),
        PullRequestDisposition::WaitingForDependency
    );
    assert_eq!(disposition(&q, 6), PullRequestDisposition::Redundant);
    assert_eq!(disposition(&q, 7), PullRequestDisposition::Conflicting);
    assert_eq!(disposition(&q, 8), PullRequestDisposition::NeedsRefresh);
    assert_eq!(q.next_merge, Some(1));
    assert_eq!(q.next_refresh, vec![8]);
    // every assessment names what it was decided against and why
    for a in &q.assessments {
        assert_eq!(a.evaluated_against.master_sha, "m1");
        assert!(!a.reasons.is_empty(), "#{} has no reason", a.number);
        assert!(!a.evidence.is_empty(), "#{} has no evidence", a.number);
    }
}

#[test]
fn a_required_check_is_passed_only_when_it_passed() {
    let w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    for (state, want) in [
        (
            Some(CheckRunState::Pending),
            PullRequestDisposition::WaitingForChecks,
        ),
        (
            Some(CheckRunState::Skipped),
            PullRequestDisposition::WaitingForChecks,
        ),
        (None, PullRequestDisposition::WaitingForChecks),
    ] {
        let mut obs = w.observation();
        obs.pull_requests[0].checks = state
            .map(|s| {
                vec![CheckObservation {
                    name: "ci".into(),
                    state: s,
                    ..Default::default()
                }]
            })
            .unwrap_or_default();
        // a green check that is not the required one proves nothing
        obs.pull_requests[0].checks.push(CheckObservation {
            name: "suite".into(),
            state: CheckRunState::Passed,
            ..Default::default()
        });
        let q = build_queue(&obs, "m0", |p| w.relation(p.number));
        assert_eq!(disposition(&q, 1), want, "{state:?}");
    }
    // unread branch protection: nothing can be ready
    let mut obs = w.observation();
    obs.required_checks = None;
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    assert_eq!(disposition(&q, 1), PullRequestDisposition::Unknown);
    assert_eq!(
        q.get(1).unwrap().required_checks,
        RequiredCheckState::Unknown
    );
    assert!(q.next_merge.is_none());
}

#[test]
fn only_derived_differences_are_never_called_redundant() {
    let w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let obs = w.observation();
    let q = build_queue(&obs, "m0", |_| RelationToMaster::DerivedOnly {
        paths: vec!["docs/generated/x.json".into()],
    });
    assert_eq!(
        disposition(&q, 1),
        PullRequestDisposition::PossiblyRedundant
    );
}

#[test]
fn a_stale_observation_is_said() {
    let w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let obs = w.observation();
    let q = build_queue(&obs, "m9", |p| w.relation(p.number));
    assert!(
        q.diagnostics
            .iter()
            .any(|d| d.starts_with("stale observation")),
        "{:?}",
        q.diagnostics
    );
}

// ---------------------------------------------------------------- the executor

#[test]
fn a_decision_that_went_stale_merges_nothing() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        // the second observation — the one taken just before acting — sees a new master
        master_moves_on: Some(2),
        ..Default::default()
    };
    let out = drain::step(&root, &mut w, false, false).unwrap();
    assert!(
        matches!(&out, DrainStepOutcome::StaleDecision { pr: 1, what } if what.contains("master moved")),
        "{out:?}"
    );
    assert!(w.merged.is_empty(), "merged on a stale decision");
    let events = drain::events(&root);
    assert!(events.iter().any(|e| e.action.as_str() == "stale_decision"));
    assert!(!events
        .iter()
        .any(|e| e.action.as_str() == "merge_attempted"));
}

/// One step against a world in which `what` happens just before the step's second
/// observation, the one the executor acts on.
fn step_with_second_observation(
    w: &mut World,
    what: Meanwhile,
    allow_refresh: bool,
) -> (DrainStepOutcome, Vec<String>) {
    let root = scratch();
    w.meanwhile.push((2, what));
    let out = drain::step(&root, w, false, allow_refresh).unwrap();
    let actions = drain::events(&root)
        .into_iter()
        .map(|e| e.action.as_str().to_string())
        .collect();
    (out, actions)
}

fn stale_with(out: &DrainStepOutcome, pr: u64, said: &str) -> bool {
    matches!(out, DrainStepOutcome::StaleDecision { pr: p, what } if *p == pr && what.contains(said))
}

#[test]
fn a_head_that_moved_merges_nothing() {
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let (out, actions) = step_with_second_observation(&mut w, Meanwhile::HeadMoves(1), false);
    assert!(stale_with(&out, 1, "its head moved"), "{out:?}");
    assert!(w.merged.is_empty(), "merged a head nobody decided on");
    assert!(actions.contains(&"stale_decision".to_string()));
    assert!(
        !actions.contains(&"merge_attempted".to_string()),
        "{actions:?}"
    );
}

#[test]
fn a_pull_request_closed_meanwhile_merges_nothing() {
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let (out, actions) = step_with_second_observation(&mut w, Meanwhile::Closes(1), false);
    assert!(stale_with(&out, 1, "no longer open"), "{out:?}");
    assert!(w.merged.is_empty());
    assert!(
        !actions.contains(&"merge_attempted".to_string()),
        "{actions:?}"
    );
}

#[test]
fn a_changed_disposition_merges_nothing() {
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let (out, actions) =
        step_with_second_observation(&mut w, Meanwhile::Labelled(1, "hold"), false);
    assert!(stale_with(&out, 1, "it is blocked now"), "{out:?}");
    assert!(w.merged.is_empty());
    assert!(
        !actions.contains(&"merge_attempted".to_string()),
        "{actions:?}"
    );
}

/// A world in which nothing is ready and #1 needs master brought in.
fn one_behind() -> World {
    World {
        open: vec![sim(1)],
        master: 1,
        ..Default::default()
    }
}

fn refreshed_nothing(w: &World, actions: &[String]) -> bool {
    w.open.iter().all(|s| s.contains == 0) && !actions.iter().any(|a| a == "refreshed")
}

#[test]
fn a_head_that_moved_is_not_refreshed() {
    let mut w = one_behind();
    let (out, actions) = step_with_second_observation(&mut w, Meanwhile::HeadMoves(1), true);
    assert!(stale_with(&out, 1, "its head moved"), "{out:?}");
    assert!(refreshed_nothing(&w, &actions), "{actions:?}");
    assert!(actions.contains(&"stale_decision".to_string()));
}

#[test]
fn a_pull_request_closed_meanwhile_is_not_refreshed() {
    let mut w = one_behind();
    let (out, actions) = step_with_second_observation(&mut w, Meanwhile::Closes(1), true);
    assert!(stale_with(&out, 1, "no longer open"), "{out:?}");
    assert!(!actions.iter().any(|a| a == "refreshed"), "{actions:?}");
}

#[test]
fn a_changed_disposition_is_not_refreshed() {
    let mut w = one_behind();
    let (out, actions) = step_with_second_observation(&mut w, Meanwhile::Labelled(1, "hold"), true);
    assert!(stale_with(&out, 1, "it is blocked now"), "{out:?}");
    assert!(refreshed_nothing(&w, &actions), "{actions:?}");
}

#[test]
fn a_refused_merge_is_recorded_and_the_next_step_plans_again() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        merge_refuses: [1].into_iter().collect(),
        ..Default::default()
    };
    let out = drain::step(&root, &mut w, false, false).unwrap();
    assert!(
        matches!(&out, DrainStepOutcome::MergeRefused { pr: 1, reason, .. } if reason.contains("not mergeable")),
        "{out:?}"
    );
    assert!(w.merged.is_empty());
    let actions: Vec<String> = drain::events(&root)
        .into_iter()
        .map(|e| e.action.as_str().to_string())
        .collect();
    assert!(
        actions.ends_with(&["merge_attempted".into(), "merge_failed".into()]),
        "{actions:?}"
    );
}

#[test]
fn a_merge_that_cannot_be_verified_stops_the_drain() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        verify_fails: true,
        ..Default::default()
    };
    let report = drain::drain(&root, &mut w, 5, false, false).unwrap();
    assert!(
        matches!(
            report.steps.last(),
            Some(DrainStepOutcome::VerificationFailed { pr: 1, .. })
        ),
        "{report:?}"
    );
    assert!(
        report.merged.is_empty(),
        "an unverified merge is not counted"
    );
    assert!(
        report.stopped.contains("could not be verified"),
        "{}",
        report.stopped
    );
    assert_eq!(
        w.merged,
        vec![1],
        "nothing after the unverified merge was merged"
    );
    let actions: Vec<String> = drain::events(&root)
        .into_iter()
        .map(|e| e.action.as_str().to_string())
        .collect();
    assert_eq!(
        actions.last().map(String::as_str),
        Some("verification_failed")
    );
}

#[test]
fn each_review_state_has_its_disposition() {
    use crate::integration::PullRequestReview as R;
    use PullRequestDisposition as D;
    for (decision, required, review, want) in [
        ("APPROVED", Some(true), R::Approved, D::Ready),
        ("APPROVED", Some(false), R::Approved, D::Ready),
        ("APPROVED", None, R::Approved, D::Ready),
        (
            "CHANGES_REQUESTED",
            Some(false),
            R::ChangesRequested,
            D::WaitingForReview,
        ),
        (
            "CHANGES_REQUESTED",
            None,
            R::ChangesRequested,
            D::WaitingForReview,
        ),
        // the forge says a review is required — a ruleset or code owners can require one
        // the branch protection does not — and that is never "not required"
        (
            "REVIEW_REQUIRED",
            Some(false),
            R::Pending,
            D::WaitingForReview,
        ),
        (
            "REVIEW_REQUIRED",
            Some(true),
            R::Pending,
            D::WaitingForReview,
        ),
        ("REVIEW_REQUIRED", None, R::Pending, D::WaitingForReview),
        ("", Some(false), R::NotRequired, D::Ready),
        ("", Some(true), R::Pending, D::WaitingForReview),
        ("", None, R::Unknown, D::Unknown),
    ] {
        let mut s = sim(1);
        s.review = decision;
        let w = World {
            open: vec![s],
            reviews_required: required,
            ..Default::default()
        };
        let q = w.queue();
        let a = q.get(1).unwrap();
        assert_eq!(
            (a.review, a.disposition),
            (review, want),
            "{decision:?} under {required:?}"
        );
        assert_eq!(q.next_merge.is_some(), want == D::Ready);
    }
}

#[test]
fn a_review_withdrawn_meanwhile_merges_nothing() {
    for decision in ["CHANGES_REQUESTED", "REVIEW_REQUIRED"] {
        let mut s = sim(1);
        s.review = "APPROVED";
        let mut w = World {
            open: vec![s],
            ..Default::default()
        };
        let (out, actions) =
            step_with_second_observation(&mut w, Meanwhile::Reviewed(1, decision), false);
        assert!(
            stale_with(&out, 1, "it is waiting_for_review now"),
            "{decision}: {out:?}"
        );
        assert!(w.merged.is_empty(), "{decision}: merged");
        assert!(
            !actions.contains(&"merge_attempted".to_string()),
            "{decision}: {actions:?}"
        );
    }
}

#[test]
fn a_fork_whose_branch_is_named_like_the_base_stacks_nothing() {
    let mut fork = sim(9);
    fork.cross_repository = true;
    fork.head_ref = "master".into();
    let mut stacked = sim(3);
    stacked.base = "feature/1";
    let w = World {
        open: vec![sim(1), stacked, fork],
        ..Default::default()
    };
    let q = w.queue();
    assert_eq!(
        disposition(&q, 1),
        PullRequestDisposition::Ready,
        "{:?}",
        q.get(1).unwrap().reasons
    );
    assert!(q.get(1).unwrap().dependencies.is_empty());
    // the fork contains master and passed its check: it is not stacked on itself either
    assert_eq!(
        disposition(&q, 9),
        PullRequestDisposition::Ready,
        "{:?}",
        q.get(9).unwrap().reasons
    );
    // a pull request stacked on a branch of this repository still waits for it
    assert_eq!(
        disposition(&q, 3),
        PullRequestDisposition::WaitingForDependency
    );
    assert_eq!(q.get(3).unwrap().reasons, vec!["stacked_on:#1".to_string()]);
    assert_eq!(q.next_merge, Some(1));
}

/// A fork's branch never answers for a branch of this repository, even when the two share
/// a name: #3 is stacked on this repository's `feature/1`, which is #1's, not the fork's.
#[test]
fn a_fork_sharing_a_branch_name_is_not_what_another_is_stacked_on() {
    let mut stacked = sim(3);
    stacked.base = "feature/1";
    let mut fork = sim(9);
    fork.cross_repository = true;
    fork.head_ref = "feature/1".into();
    let w = World {
        open: vec![sim(1), stacked, fork],
        ..Default::default()
    };
    let q = w.queue();
    assert_eq!(q.get(3).unwrap().reasons, vec!["stacked_on:#1".to_string()]);
    let deps: Vec<u64> = q
        .get(3)
        .unwrap()
        .dependencies
        .iter()
        .map(|d| d.number)
        .collect();
    assert_eq!(deps, vec![1]);
}

/// Only a pull request that targets another branch can be stacked: a back-merge from
/// `master` into a release branch does not make every pull request into master wait on it.
#[test]
fn a_back_merge_from_the_base_stacks_nothing_onto_it() {
    let mut back_merge = sim(5);
    back_merge.head_ref = "master".into();
    back_merge.base = "release/1";
    let w = World {
        open: vec![sim(1), back_merge],
        ..Default::default()
    };
    let q = w.queue();
    assert_eq!(
        disposition(&q, 1),
        PullRequestDisposition::Ready,
        "{:?}",
        q.get(1).unwrap().reasons
    );
    assert!(q.get(1).unwrap().dependencies.is_empty());
}

#[test]
fn a_dry_run_changes_nothing_and_records_nothing() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        ..Default::default()
    };
    let report = drain::drain(&root, &mut w, 5, true, true).unwrap();
    assert_eq!(report.steps, vec![DrainStepOutcome::WouldMerge { pr: 1 }]);
    assert!(w.merged.is_empty());
    assert!(drain::events(&root).is_empty());
}

/// The end-to-end scenario: five pull requests, a dependency, a failure, a redundancy, and
/// one merge at a time with a new plan after each.
#[test]
fn the_queue_is_rediscovered_after_every_merge() {
    let root = scratch();
    let mut dependent = sim(2);
    dependent.depends_on = Some(1);
    let mut failing = sim(4);
    failing.failing = true;
    let mut redundant = sim(5);
    redundant.redundant_after = Some(1);
    let mut w = World {
        open: vec![sim(1), dependent, sim(3), failing, redundant],
        ..Default::default()
    };

    // cycle 1: #1 and #3 are ready; #1 is older, so it goes first
    let q0 = w.queue();
    assert_eq!(
        disposition(&q0, 2),
        PullRequestDisposition::WaitingForDependency
    );
    assert_eq!(disposition(&q0, 4), PullRequestDisposition::NeedsRepair);
    assert_eq!(q0.next_merge, Some(1));
    let report = drain::drain(&root, &mut w, 3, false, false).unwrap();
    // exactly one merge: #1's merge moved master, so #3 no longer contains it
    assert_eq!(report.merged, vec![1]);
    assert!(
        matches!(report.steps.last(), Some(DrainStepOutcome::Idle { .. })),
        "{report:?}"
    );

    // re-planned from the new master: #5's patch is on master, #2 and #3 need master
    let q1 = w.queue();
    assert_eq!(q1.master_sha, "m1");
    assert_eq!(disposition(&q1, 5), PullRequestDisposition::Redundant);
    assert_eq!(disposition(&q1, 2), PullRequestDisposition::NeedsRefresh);
    assert_eq!(disposition(&q1, 3), PullRequestDisposition::NeedsRefresh);
    assert_eq!(disposition(&q1, 4), PullRequestDisposition::NeedsRepair);

    // cycle 2: nothing is ready; with refresh allowed, master is brought into exactly one
    let report = drain::drain(&root, &mut w, 3, false, true).unwrap();
    assert!(report.merged.is_empty());
    let refreshed = match report.steps.last() {
        Some(DrainStepOutcome::Refreshed { pr, .. }) => *pr,
        other => panic!("expected a refresh, got {other:?}"),
    };
    assert_eq!(refreshed, 2, "#2 is older than #3");
    // pipeline depth one: #3 was not touched
    assert_eq!(w.open.iter().find(|s| s.number == 3).unwrap().contains, 0);

    // cycle 3: #2 now contains master and its check passed: it merges; that puts #3 behind
    // again, and the step after the merge — re-planned from master m2 — brings m2 into #3
    let report = drain::drain(&root, &mut w, 3, false, true).unwrap();
    assert_eq!(report.merged, vec![2]);
    assert!(
        matches!(
            report.steps.last(),
            Some(DrainStepOutcome::Refreshed { pr: 3, .. })
        ),
        "{report:?}"
    );
    let q3 = w.queue();
    assert_eq!(q3.master_sha, "m2");
    assert_eq!(disposition(&q3, 3), PullRequestDisposition::Ready);
    assert_eq!(q3.get(3).unwrap().evaluated_against.head_sha, "h3.2");

    // every merge was preceded by two observations taken in its own step: the plan of a
    // previous step was never the one acted on
    let mut last = 0;
    for at in &w.merged_after_observation {
        assert!(
            at - last >= 2,
            "a merge reused an earlier observation: {:?}",
            w.merged_after_observation
        );
        last = *at;
    }
    let actions: Vec<String> = drain::events(&root)
        .into_iter()
        .map(|e| e.action.as_str().to_string())
        .collect();
    assert!(actions.contains(&"merge_succeeded".to_string()));
    assert!(actions.contains(&"refreshed".to_string()));
}

#[test]
fn a_conflict_after_a_merge_is_skipped_and_the_next_is_worked_on() {
    let root = scratch();
    let mut b = sim(2);
    b.conflicts_after = Some(1);
    let mut w = World {
        open: vec![sim(1), b, sim(3)],
        ..Default::default()
    };
    let report = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert_eq!(report.merged, vec![1]);
    let q = w.queue();
    assert_eq!(disposition(&q, 2), PullRequestDisposition::Conflicting);
    // the conflicting one does not stall the queue: the next refresh is #3
    assert_eq!(q.next_refresh, vec![3]);
    let report = drain::drain(&root, &mut w, 1, false, true).unwrap();
    assert!(
        matches!(
            report.steps.last(),
            Some(DrainStepOutcome::Refreshed { pr: 3, .. })
        ),
        "{report:?}"
    );
}

/// The executor's own refreshed head holds the pipeline while its required check runs —
/// whether the forge reports it as pending or has not created it yet, which is how an
/// aggregate check that needs every other job reads for most of a CI run.
#[test]
fn a_refreshed_pull_request_waiting_for_checks_holds_the_pipeline() {
    for (unreported, pending) in [(false, true), (true, false)] {
        let root = scratch();
        let mut w = World {
            open: vec![sim(1), sim(2)],
            master: 1,
            ..Default::default()
        };
        // the executor brings master into #1, the older of the two
        let out = drain::step(&root, &mut w, false, true).unwrap();
        assert!(
            matches!(out, DrainStepOutcome::Refreshed { pr: 1, .. }),
            "{out:?}"
        );
        // and the required check now runs on the head it pushed
        w.open[0].ci_pending = pending;
        w.open[0].ci_unreported = unreported;
        assert_eq!(
            disposition(&w.queue(), 1),
            PullRequestDisposition::WaitingForChecks
        );
        let report = drain::drain(&root, &mut w, 1, false, true).unwrap();
        assert_eq!(
            report.steps,
            vec![DrainStepOutcome::AwaitingChecks { pr: 1 }],
            "unreported {unreported}, pending {pending}"
        );
        assert_eq!(
            w.open[1].contains, 0,
            "#2 was refreshed while the executor waits for #1's checks"
        );
    }
}

/// The pipeline waits only for a check it started: a required check that never reports
/// would otherwise hold every refresh forever, and one running on a head the author pushed
/// is not the executor's run to wait for.
#[test]
fn a_check_the_executor_did_not_start_does_not_hold_the_refresh_pipeline() {
    for (unreported, pending) in [(true, false), (false, true)] {
        let root = scratch();
        let mut w = World {
            open: vec![sim(1), sim(2)],
            master: 1,
            ..Default::default()
        };
        // #1 contains master, by its author's own merge; #2 does not
        w.open[0].contains = 1;
        w.open[0].ci_unreported = unreported;
        w.open[0].ci_pending = pending;
        assert_eq!(
            disposition(&w.queue(), 1),
            PullRequestDisposition::WaitingForChecks
        );
        let out = drain::step(&root, &mut w, false, true).unwrap();
        assert!(
            matches!(out, DrainStepOutcome::Refreshed { pr: 2, .. }),
            "unreported {unreported}, pending {pending}: {out:?}"
        );
    }
}

/// A required check that has not reported on the executor's own head holds the pipeline only
/// for [`drain::REFRESHED_HEAD_REPORTS_WITHIN`] after the push: past that it is taken never to
/// report, and the next pull request is refreshed rather than every refresh freezing. A trail
/// line whose time cannot be read bounds a missing check to nothing, while a pending one still
/// holds; a `refreshed` line written before `head_after` existed names no head and holds
/// nothing, whatever the check.
#[test]
fn an_unreported_check_on_the_executors_own_head_holds_the_pipeline_only_for_a_bound() {
    let past = crate::peers::rfc3339(
        std::time::SystemTime::now()
            - drain::REFRESHED_HEAD_REPORTS_WITHIN
            - std::time::Duration::from_secs(60),
    );
    type Rewrite = fn(&mut drain::IntegrationEvent, &str);
    let aged: Rewrite = |e, past| e.at = past.to_string();
    let unreadable: Rewrite = |e, _| e.at = "not a time".into();
    let legacy: Rewrite = |e, _| e.head_after = None;
    // (rewrite of the refreshed line, check pending rather than unreported, #1 still holds)
    for (what, rewrite, pending, holds) in [
        ("aged past the bound", aged, false, false),
        (
            "unreadable time, unreported check",
            unreadable,
            false,
            false,
        ),
        ("unreadable time, pending check", unreadable, true, true),
        ("no head_after, pending check", legacy, true, false),
    ] {
        let root = scratch();
        let mut w = World {
            open: vec![sim(1), sim(2)],
            master: 1,
            ..Default::default()
        };
        let out = drain::step(&root, &mut w, false, true).unwrap();
        assert!(
            matches!(out, DrainStepOutcome::Refreshed { pr: 1, .. }),
            "{what}: {out:?}"
        );
        // the head the executor pushed never gets its required check reported
        w.open[0].ci_unreported = !pending;
        w.open[0].ci_pending = pending;
        let q = w.queue();
        assert_eq!(disposition(&q, 1), PullRequestDisposition::WaitingForChecks);
        assert_eq!(
            q.get(1).unwrap().required_checks,
            if pending {
                RequiredCheckState::Pending
            } else {
                RequiredCheckState::Missing
            },
            "{what}"
        );
        let trail: Vec<String> = drain::events(&root)
            .into_iter()
            .map(|mut e| {
                if e.action.as_str() == "refreshed" {
                    rewrite(&mut e, &past);
                }
                serde_json::to_string(&e).unwrap()
            })
            .collect();
        std::fs::write(trail_of(&root), trail.join("\n") + "\n").unwrap();
        let out = drain::step(&root, &mut w, false, true).unwrap();
        if holds {
            assert_eq!(out, DrainStepOutcome::AwaitingChecks { pr: 1 }, "{what}");
        } else {
            assert!(
                matches!(out, DrainStepOutcome::Refreshed { pr: 2, .. }),
                "{what}: {out:?}"
            );
        }
    }
}

#[test]
fn cleanup_lists_only_what_is_provably_on_master() {
    let root = scratch();
    // #3 is old and similar to #1: neither is evidence that its work landed
    let mut same_paths = sim(3);
    same_paths.created = "2020-01-01T00:00:00Z".into();
    let mut w = World {
        open: vec![sim(1), sim(2), same_paths],
        ..Default::default()
    };
    w.open[0].redundant_after = Some(99);
    w.merged = vec![99];
    struct DerivedTwo<'a>(&'a mut World);
    impl Integrator for DerivedTwo<'_> {
        fn observe(&mut self) -> Result<IntegrationQueue, String> {
            let obs = self.0.observation();
            let w = &*self.0;
            Ok(build_queue(&obs, "m0", |p| match p.number {
                2 => RelationToMaster::DerivedOnly {
                    paths: vec!["site/data/x.json".into()],
                },
                n => w.relation(n),
            }))
        }
        fn merge(&mut self, _: u64, _: &str, _: &str) -> Result<(), String> {
            unreachable!()
        }
        fn verify(
            &mut self,
            _: u64,
            _: &crate::integration::EvaluatedAgainst,
            _: &str,
        ) -> Result<drain::Landed, drain::NotLanded> {
            unreachable!()
        }
        fn refresh_branch(&mut self, _: &PullRequestAssessment, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn close(&mut self, _: u64, _: &str, _: &str) -> Result<(), String> {
            unreachable!()
        }
    }
    let items = drain::cleanup(&root, &mut DerivedTwo(&mut w), false).unwrap();
    let by: BTreeMap<u64, String> = items.into_iter().map(|i| (i.pr, i.action)).collect();
    assert_eq!(by.get(&1).map(String::as_str), Some("would_close"));
    assert_eq!(by.get(&2).map(String::as_str), Some("left_for_a_person"));
    // a similar title and an old date are not evidence of anything
    assert!(!by.contains_key(&3));
}

// ---------------------------------------------------------------- properties

/// Creation moments drawn from a few, so that two pull requests often share one and the
/// number has to break the tie.
const CREATED: [&str; 3] = [
    "2026-09-01T00:00:00Z",
    "2026-09-02T00:00:00Z",
    "2026-09-03T00:00:00Z",
];

/// Paths several pull requests may author, so that overlaps occur.
const SHARED: [&str; 3] = ["docs/shared.md", "apps/x/src/lib.rs", "README.md"];

/// Review decisions the forge reports.
const REVIEWS: [&str; 4] = ["", "APPROVED", "CHANGES_REQUESTED", "REVIEW_REQUIRED"];

fn arb_sim() -> impl Strategy<Value = Sim> {
    (
        (
            1u64..40,
            0u32..3,
            any::<bool>(),
            any::<bool>(),
            proptest::option::of(1u64..40),
            0usize..4,
        ),
        (
            0usize..CREATED.len(),
            proptest::sample::subsequence(SHARED.to_vec(), 0..=2),
            0usize..REVIEWS.len(),
            any::<bool>(),
            0usize..4,
        ),
    )
        .prop_map(
            |(
                (n, contains, failing, draft, depends_on, label),
                (created, shared, review, fork, base),
            )| {
                let mut s = sim(n);
                s.contains = contains;
                s.failing = failing;
                s.draft = draft;
                s.depends_on = depends_on;
                s.labels = match label {
                    0 => vec!["hold".into()],
                    _ => Vec::new(),
                };
                s.auto_merge = label == 1;
                s.created = CREATED[created].into();
                s.paths.extend(shared.into_iter().map(String::from));
                s.review = REVIEWS[review];
                s.cross_repository = fork;
                if base == 0 {
                    s.base = "release/1";
                }
                if fork && base == 1 {
                    // a fork's branch may carry any name, the base's own included
                    s.head_ref = "master".into();
                }
                s
            },
        )
}

fn arb_world() -> impl Strategy<Value = World> {
    (
        proptest::collection::vec(arb_sim(), 0..12),
        0u32..3,
        prop_oneof![Just(None), Just(Some(true)), Just(Some(false))],
    )
        .prop_map(|(mut open, master, reviews_required)| {
            open.sort_by_key(|s| s.number);
            open.dedup_by_key(|s| s.number);
            // a head cannot contain a master that does not exist yet
            for s in &mut open {
                s.contains = s.contains.min(master);
            }
            World {
                open,
                master,
                reviews_required,
                ..Default::default()
            }
        })
}

proptest! {
    #[test]
    fn ranking_does_not_depend_on_the_order_observed(w in arb_world(), seed in any::<u64>()) {
        let obs = w.observation();
        let a: Vec<u64> = build_queue(&obs, &master_sha(w.master), |p| w.relation(p.number))
            .assessments.iter().map(|a| a.number).collect();
        let mut shuffled = obs.clone();
        let len = shuffled.pull_requests.len();
        if len > 1 {
            shuffled.pull_requests.rotate_left((seed as usize) % len);
            shuffled.pull_requests.reverse();
        }
        let b: Vec<u64> = build_queue(&shuffled, &master_sha(w.master), |p| w.relation(p.number))
            .assessments.iter().map(|a| a.number).collect();
        prop_assert_eq!(a, b);
    }

    #[test]
    fn only_an_eligible_pull_request_is_ready(w in arb_world()) {
        let q = w.queue();
        for a in q.assessments.iter().filter(|a| a.disposition == PullRequestDisposition::Ready) {
            let s = w.open.iter().find(|s| s.number == a.number).unwrap();
            prop_assert!(!s.draft);
            prop_assert!(s.labels.is_empty());
            prop_assert!(!s.auto_merge);
            prop_assert!(!s.failing);
            prop_assert_eq!(s.contains, w.master);
            prop_assert_eq!(a.required_checks, RequiredCheckState::Passed);
            let reviewed = matches!(
                a.review,
                PullRequestReview::Approved | PullRequestReview::NotRequired
            );
            prop_assert!(reviewed, "#{} is ready with review {:?}", a.number, a.review);
            prop_assert!(
                s.review != "CHANGES_REQUESTED" && s.review != "REVIEW_REQUIRED",
                "#{} is ready while the forge says {}", a.number, s.review
            );
            let up_to_date = matches!(a.relation, RelationToMaster::UpToDate { .. });
            prop_assert!(up_to_date);
            if let Some(d) = s.depends_on {
                prop_assert!(d == s.number || !w.open.iter().any(|o| o.number == d));
            }
        }
        // the next merge, when any, is the first ready one in rank order
        let first_ready = q.assessments.iter().find(|a| a.disposition == PullRequestDisposition::Ready).map(|a| a.number);
        prop_assert_eq!(q.next_merge, first_ready);
    }

    #[test]
    fn a_fork_is_never_what_another_is_stacked_on(w in arb_world()) {
        let q = w.queue();
        let forks: BTreeSet<u64> = w
            .open
            .iter()
            .filter(|s| s.cross_repository)
            .map(|s| s.number)
            .collect();
        for a in &q.assessments {
            let s = w.open.iter().find(|s| s.number == a.number).unwrap();
            for d in a.dependencies.iter().filter(|d| forks.contains(&d.number)) {
                // only a declaration in the body makes a fork a dependency
                prop_assert_eq!(Some(d.number), s.depends_on, "#{} on fork #{}", a.number, d.number);
            }
        }
    }

    #[test]
    fn ties_are_broken_by_created_then_number(w in arb_world()) {
        // (the components before age now include declared dependents and the change's size)
        let q = w.queue();
        let contenders: BTreeSet<u64> = q
            .assessments
            .iter()
            .filter(|a| matches!(
                a.disposition,
                PullRequestDisposition::Ready | PullRequestDisposition::NeedsRefresh
            ))
            .map(|a| a.number)
            .collect();
        // every component before age, read independently of the planner: the factors the
        // planner recorded must agree with what is recomputed here
        let mut dependents: BTreeMap<u64, usize> = BTreeMap::new();
        for a in &q.assessments {
            for d in &a.dependencies {
                if d.certainty == crate::integration::DependencyCertainty::Confirmed && !d.satisfied {
                    *dependents.entry(d.number).or_default() += 1;
                }
            }
        }
        let before_age = |a: &PullRequestAssessment| {
            let contention = a.overlaps.iter().filter(|o| contenders.contains(&o.number)).count();
            let waiting_on_it = dependents.get(&a.number).copied().unwrap_or(0);
            (
                a.lane as u8,
                a.disposition as u8,
                a.risk as u8,
                contention,
                std::cmp::Reverse(waiting_on_it),
                a.authored_paths.len(),
            )
        };
        for pair in q.assessments.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            prop_assert!(before_age(a) <= before_age(b), "#{} before #{}", a.number, b.number);
            if before_age(a) == before_age(b) {
                // equal on everything before age: the older first, and of two opened in the
                // same moment the lower number
                prop_assert!(
                    (a.created_at.as_str(), a.number) < (b.created_at.as_str(), b.number),
                    "#{} ({}) before #{} ({})", a.number, a.created_at, b.number, b.created_at
                );
            }
        }
    }

    #[test]
    fn build_queue_is_idempotent(w in arb_world(), seed in any::<u64>()) {
        let obs = w.observation();
        let master = master_sha(w.master);
        let once = build_queue(&obs, &master, |p| w.relation(p.number));
        let twice = build_queue(&obs, &master, |p| w.relation(p.number));
        // the whole value: dispositions, the order of reasons and evidence, overlaps, tallies
        prop_assert_eq!(&once, &twice);
        let mut shuffled = obs.clone();
        let len = shuffled.pull_requests.len();
        if len > 1 {
            shuffled.pull_requests.rotate_left((seed as usize) % len);
            shuffled.pull_requests.reverse();
        }
        prop_assert_eq!(&once, &build_queue(&shuffled, &master, |p| w.relation(p.number)));
    }
}

// ---------------------------------------------------------------- the relation, by real git

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit_on(dir: &std::path::Path, branch: &str, from: &str, files: &[(&str, &str)]) -> String {
    git(dir, &["checkout", "-q", "-B", branch, from]);
    for (p, c) in files {
        std::fs::write(dir.join(p), c).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", branch]);
    git(dir, &["rev-parse", "HEAD"])
}

/// A repository in which every relation occurs: what each head is to `master` is named by
/// its key.
struct RelationFixture {
    dir: std::path::PathBuf,
    master: String,
    heads: BTreeMap<&'static str, String>,
}

fn relation_fixture() -> RelationFixture {
    let dir = scratch();
    git(&dir, &["init", "-q", "-b", "master"]);
    std::fs::write(dir.join(".gitattributes"), "gen.json merge=derived\n").unwrap();
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    std::fs::write(dir.join("gen.json"), "{}\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);

    let authored = commit_on(&dir, "authored", &base, &[("b.txt", "new\n")]);
    let derived_only = commit_on(&dir, "derived", &base, &[("gen.json", "{\"x\":1}\n")]);
    let conflicting = commit_on(&dir, "conflict", &base, &[("a.txt", "theirs\n")]);
    let derived_conflict = commit_on(
        &dir,
        "derived-conflict",
        &base,
        &[("gen.json", "{\"y\":2}\n"), ("c.txt", "c\n")],
    );
    // master moves on: a.txt and gen.json change there too
    let master = commit_on(
        &dir,
        "master",
        &base,
        &[("a.txt", "ours\n"), ("gen.json", "{\"m\":1}\n")],
    );
    let up_to_date = commit_on(&dir, "fresh", &master, &[("d.txt", "d\n")]);
    let same = commit_on(
        &dir,
        "same",
        &base,
        &[("a.txt", "ours\n"), ("gen.json", "{\"m\":1}\n")],
    );
    let heads = BTreeMap::from([
        ("base", base),
        ("authored", authored),
        ("derived_only", derived_only),
        ("conflicting", conflicting),
        ("derived_conflict", derived_conflict),
        ("up_to_date", up_to_date),
        ("same", same),
        (
            "absent",
            "0000000000000000000000000000000000000000".to_string(),
        ),
    ]);
    RelationFixture { dir, master, heads }
}

#[test]
fn the_relation_to_master_is_decided_by_git_with_the_derived_attribute() {
    use crate::integration::relation_to_master;
    let RelationFixture { dir, master, heads } = relation_fixture();
    let h = |k: &str| heads[k].clone();
    let (base, same, authored, up_to_date) = (h("base"), h("same"), h("authored"), h("up_to_date"));
    let (conflicting, derived_conflict, derived_only) =
        (h("conflicting"), h("derived_conflict"), h("derived_only"));

    assert_eq!(
        relation_to_master(&dir, &master, &base),
        RelationToMaster::Contained
    );
    assert_eq!(
        relation_to_master(&dir, &master, &same),
        RelationToMaster::Superseded
    );
    assert!(matches!(
        relation_to_master(&dir, &master, &authored),
        RelationToMaster::Behind { behind: 1, ref authored } if authored == &vec!["b.txt".to_string()]
    ));
    assert!(matches!(
        relation_to_master(&dir, &master, &up_to_date),
        RelationToMaster::UpToDate { ref authored } if authored == &vec!["d.txt".to_string()]
    ));
    assert_eq!(
        relation_to_master(&dir, &master, &conflicting),
        RelationToMaster::Conflicting {
            paths: vec!["a.txt".into()]
        }
    );
    // a conflict on a derived path is the regeneration's, not a person's: the authored
    // change is what decides
    assert!(
        matches!(
            relation_to_master(&dir, &master, &derived_conflict),
            RelationToMaster::Behind { ref authored, .. } if authored == &vec!["c.txt".to_string()]
        ),
        "{:?}",
        relation_to_master(&dir, &master, &derived_conflict)
    );
    // only derived output differs: never redundant, only possibly redundant
    assert!(
        matches!(
            relation_to_master(&dir, &master, &derived_only),
            RelationToMaster::DerivedOnly { .. }
        ),
        "{:?}",
        relation_to_master(&dir, &master, &derived_only)
    );
    assert!(matches!(
        relation_to_master(&dir, &master, "0000000000000000000000000000000000000000"),
        RelationToMaster::Unknown { .. }
    ));
}

/// A repository in which `above` was built on `below`'s head and `below` then landed the way
/// the forge lands it, as a merge commit on master that writes `below`'s tree.
struct StackedFixture {
    dir: std::path::PathBuf,
    landed: String,
    above: String,
}

fn stacked_fixture() -> StackedFixture {
    let dir = scratch();
    git(&dir, &["init", "-q", "-b", "master"]);
    std::fs::write(dir.join(".gitattributes"), "gen.json merge=derived\n").unwrap();
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    std::fs::write(dir.join("gen.json"), "{}\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);
    let below = commit_on(
        &dir,
        "below",
        &base,
        &[("b.txt", "b\n"), ("gen.json", "{\"b\":1}\n")],
    );
    let above = commit_on(
        &dir,
        "above",
        &below,
        &[("c.txt", "c\n"), ("gen.json", "{\"b\":1,\"c\":1}\n")],
    );
    let landed = git(
        &dir,
        &[
            "commit-tree",
            &format!("{below}^{{tree}}"),
            "-p",
            &base,
            "-p",
            &below,
            "-m",
            "merge below",
        ],
    );
    StackedFixture { dir, landed, above }
}

/// ADR 0129, decided by git. Master's merge commit is not in `above`, merging `above` writes
/// `above`'s own tree, and a merge commit is all `above` lacks: it carries master. One change
/// on master that `above` never saw, authored or derived, and the same head is behind. So it
/// is after a commit of master's own that changes no file: the tree is the same, and the
/// changelog lists a commit the head's never did.
#[test]
fn a_head_stacked_on_a_landed_head_carries_master() {
    use crate::integration::relation_to_master;
    let StackedFixture { dir, landed, above } = stacked_fixture();
    assert_eq!(
        relation_to_master(&dir, &landed, &above),
        RelationToMaster::CarriesMaster {
            behind: 1,
            authored: vec!["c.txt".into()],
        }
    );
    // the property itself, asked of git without this module in between
    assert_eq!(
        git(&dir, &["merge-tree", "--write-tree", &landed, &above]),
        git(&dir, &["rev-parse", &format!("{above}^{{tree}}")]),
    );
    let moved = commit_on(&dir, "moved", &landed, &[("d.txt", "d\n")]);
    assert!(
        matches!(
            relation_to_master(&dir, &moved, &above),
            RelationToMaster::Behind { behind: 2, ref authored } if authored == &vec!["c.txt".to_string()]
        ),
        "{:?}",
        relation_to_master(&dir, &moved, &above)
    );
    // master's own commit that changes nothing: the merge still writes the head's tree, and
    // history holds a commit no generator on the head ever listed
    git(&dir, &["checkout", "-q", "-B", "empty", &landed]);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "nothing"]);
    let empty = git(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(
        git(&dir, &["merge-tree", "--write-tree", &empty, &above]),
        git(&dir, &["rev-parse", &format!("{above}^{{tree}}")]),
    );
    assert!(
        matches!(
            relation_to_master(&dir, &empty, &above),
            RelationToMaster::Behind { behind: 2, .. }
        ),
        "{:?}",
        relation_to_master(&dir, &empty, &above)
    );
    // a derived file both sides rewrote is the regeneration's to settle: behind, never carried
    let rederived = commit_on(
        &dir,
        "rederived",
        &landed,
        &[("gen.json", "{\"b\":1,\"m\":2}\n")],
    );
    assert!(
        matches!(
            relation_to_master(&dir, &rederived, &above),
            RelationToMaster::Behind { behind: 2, .. }
        ),
        "{:?}",
        relation_to_master(&dir, &rederived, &above)
    );
}

/// A relation kept by an executable that could not tell `carries_master` from `behind` is not
/// a fact in this one's vocabulary: the cache drops it, git is asked again, and the file says
/// which vocabulary decided what it holds now.
#[test]
fn a_relation_of_an_older_vocabulary_is_asked_again() {
    let StackedFixture { dir, landed, above } = stacked_fixture();
    observed(&dir, &landed, vec![observed_pr(2, &above)]);
    let path = crate::integration::state_path(&dir, crate::integration::RELATIONS_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        format!(
            "{{\"entries\":{{\"{landed}..{above}\":{{\"kind\":\"behind\",\"behind\":1,\"authored\":[\"c.txt\"]}}}}}}"
        ),
    )
    .unwrap();
    let q = crate::integration::queue_and_record(&dir).unwrap();
    assert!(
        matches!(
            q.get(2).unwrap().relation,
            RelationToMaster::CarriesMaster { behind: 1, .. }
        ),
        "{:?}",
        q.get(2).unwrap().relation
    );
    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(kept["vocabulary"], 1);
    assert_eq!(
        kept["entries"][format!("{landed}..{above}")]["kind"],
        "carries_master"
    );
}

/// `prs repair` brings master into a head that is behind. A head that carries master has
/// nothing to bring in: its merge writes its own tree, and repair says so and would push
/// nothing.
#[test]
fn a_head_that_carries_master_has_nothing_to_repair() {
    let StackedFixture { dir, landed, above } = stacked_fixture();
    observed(&dir, &landed, vec![observed_pr(2, &above)]);
    let r = repair::plan(&dir, &RepairTarget::Number(2)).unwrap();
    assert_eq!(outcome(&r), "nothing", "{r:?}");
    assert!(
        matches!(
            &r.outcome,
            repair::RepairOutcome::NothingToRepair { why } if why.contains("yields its own tree")
        ),
        "{r:?}"
    );
    assert!(
        matches!(r.relation, Some(RelationToMaster::CarriesMaster { .. })),
        "{r:?}"
    );
}

/// ADR 0129 through the executor: #2 was built on #1's head. Once #1 has landed, #2 lacks
/// master's merge commit and its merge yields its own tree, so the verdict its head already
/// has is the verdict on what master becomes: it is ready, says `carries_master`, and merges
/// with no refresh in between.
#[test]
fn a_link_above_a_landed_one_merges_without_a_refresh() {
    let root = scratch();
    let mut above = sim(2);
    above.carries = vec![1];
    above.depends_on = Some(1);
    let mut w = World {
        open: vec![sim(1), above],
        ..Default::default()
    };
    assert_eq!(
        disposition(&w.queue(), 2),
        PullRequestDisposition::WaitingForDependency
    );
    let report = drain::drain(&root, &mut w, 1, false, true).unwrap();
    assert_eq!(report.merged, vec![1], "{report:?}");

    let q = w.queue();
    let a = q.get(2).unwrap();
    assert!(
        matches!(
            a.relation,
            RelationToMaster::CarriesMaster { behind: 1, .. }
        ),
        "{:?}",
        a.relation
    );
    assert_eq!(a.disposition, PullRequestDisposition::Ready);
    assert!(a.gates.iter().all(|g| g.passed), "{:?}", a.gates);
    assert_eq!(a.reasons, ["carries_master", "required_checks_passed"]);
    assert!(q.next_refresh.is_empty(), "{:?}", q.next_refresh);

    let report = drain::drain(&root, &mut w, 1, false, true).unwrap();
    assert_eq!(report.merged, vec![2], "{report:?}");
    assert_eq!(w.merge_calls, 2);
    assert_eq!(
        w.refresh_calls, 0,
        "a head that carries master is not refreshed"
    );
}

/// The link carries only what it was built on. A merge of somebody else's between, and master
/// holds a change its head never saw: behind, to be refreshed like any other.
#[test]
fn a_link_is_behind_once_master_holds_what_it_never_saw() {
    let root = scratch();
    let mut above = sim(2);
    above.carries = vec![1];
    let mut w = World {
        open: vec![sim(1), above],
        ..Default::default()
    };
    let report = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert_eq!(report.merged, vec![1], "{report:?}");
    w.master += 1;
    let q = w.queue();
    assert!(
        matches!(
            q.get(2).unwrap().relation,
            RelationToMaster::Behind { behind: 2, .. }
        ),
        "{:?}",
        q.get(2).unwrap().relation
    );
    assert_eq!(disposition(&q, 2), PullRequestDisposition::NeedsRefresh);
    assert_eq!(q.next_refresh, vec![2]);
}

/// `project.cache-is-invisible`: the relation cache answers exactly what git answers, on a
/// hit, on a miss, and after the round trip through the file `queue_of` keeps it in.
#[test]
fn relation_cache_equals_recomputation() {
    use crate::integration::{relation_cached, relation_to_master, RelationCache};
    let RelationFixture { dir, master, heads } = relation_fixture();
    let mut cache = RelationCache::default();
    for (name, head) in &heads {
        let fresh = relation_to_master(&dir, &master, head);
        let miss = relation_cached(&dir, &mut cache, &master, head);
        let hit = relation_cached(&dir, &mut cache, &master, head);
        assert_eq!(miss, fresh, "{name}: a miss differs from git");
        assert_eq!(hit, fresh, "{name}: a hit differs from git");
    }
    // an unknown is not a fact about the pair, so it is not kept
    assert_eq!(cache.entries.len(), heads.len() - 1);
    assert!(!cache.entries.keys().any(|k| k.ends_with(&heads["absent"])));
    let text = serde_json::to_string(&cache).unwrap();
    let mut reread: RelationCache = serde_json::from_str(&text).unwrap();
    for (name, head) in &heads {
        assert_eq!(
            relation_cached(&dir, &mut reread, &master, head),
            relation_to_master(&dir, &master, head),
            "{name}: the stored cache differs from git"
        );
    }
}

/// `project.cache-is-invisible` for the shape beside the relation (WP16): a hit and a miss
/// answer what git answers, an unanswered shape and a name that is not a commit id are not
/// kept, and the stored cache answers the same after its round trip.
#[test]
fn shape_cache_equals_recomputation() {
    use crate::integration::relation::change_shape;
    use crate::integration::{shape_cached, RelationCache};
    let RelationFixture { dir, master, heads } = relation_fixture();
    let mut cache = RelationCache::default();
    for (name, head) in &heads {
        let fresh = change_shape(&dir, &master, head);
        let miss = shape_cached(&dir, &mut cache, &master, head);
        let hit = shape_cached(&dir, &mut cache, &master, head);
        assert_eq!(miss, fresh, "{name}: a miss differs from git");
        assert_eq!(hit, fresh, "{name}: a hit differs from git");
    }
    assert_eq!(
        cache.shapes.len(),
        heads.len() - 1,
        "the absent head is not kept"
    );
    assert_eq!(shape_cached(&dir, &mut cache, &master, "HEAD"), None);
    assert_eq!(cache.shapes.len(), heads.len() - 1, "a ref is never a key");
    let text = serde_json::to_string(&cache).unwrap();
    let mut reread: RelationCache = serde_json::from_str(&text).unwrap();
    for (name, head) in &heads {
        assert_eq!(
            shape_cached(&dir, &mut reread, &master, head),
            change_shape(&dir, &master, head),
            "{name}: the stored cache differs from git"
        );
    }
}

/// The queue reads each head's shape through the cache it keeps beside the relations: a
/// conflicting head's authored paths are its whole change, the assessment carries the shape,
/// and a shape kept for another master is dropped with that master's relations.
#[test]
fn the_queue_reads_and_keeps_each_heads_shape() {
    let RelationFixture { dir, master, heads } = relation_fixture();
    observed(
        &dir,
        &master,
        vec![
            observed_pr(1, &heads["conflicting"]),
            observed_pr(2, &heads["derived_conflict"]),
        ],
    );
    let path = crate::integration::state_path(&dir, crate::integration::RELATIONS_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old = format!("{}..{}", "1".repeat(40), heads["authored"]);
    std::fs::write(
        &path,
        format!("{{\"entries\":{{}},\"shapes\":{{\"{old}\":{{\"authored\":[],\"derived\":[]}}}}}}"),
    )
    .unwrap();
    for _ in 0..2 {
        let q = crate::integration::queue_and_record(&dir).unwrap();
        let two = q.get(2).unwrap();
        assert_eq!(two.authored_paths, vec!["c.txt".to_string()]);
        let shape = two.change_shape.as_ref().expect("read");
        assert_eq!(shape.derived, vec!["gen.json".to_string()]);
        assert_eq!(q.get(1).unwrap().authored_paths, vec!["a.txt".to_string()]);
    }
    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let keys: Vec<&String> = kept["shapes"].as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert!(keys.iter().all(|k| k.starts_with(&master)), "{keys:?}");
}

/// What a merge changes by kind decides overlaps, risk and rank (WP16): two pull requests
/// that raise the version overlap as a version bump, two that change release records overlap
/// as a release whatever files they name, a shared path is contention, and of ready pull
/// requests of equal risk the one that overlaps nothing ranks first.
#[test]
fn the_shape_of_a_change_decides_its_overlaps_risk_and_rank() {
    use crate::integration::{
        build_queue_shaped, ChangeShape, IntegrationRisk, OverlapKind, PathOverlap,
    };
    use crate::release::version::MANIFEST;
    // #1 and #2 share a path, #3 shares none: equal risk, so #3 lands first though younger
    let mut one = sim(1);
    one.paths = vec!["docs/shared.md".into()];
    let mut two = sim(2);
    two.paths = vec!["docs/shared.md".into()];
    let w = World {
        open: vec![one, two, sim(3)],
        ..Default::default()
    };
    let q = build_queue_shaped(&w.observation(), "m0", |p| w.relation(p.number), |_| None);
    let order: Vec<u64> = q.assessments.iter().map(|a| a.number).collect();
    assert_eq!(order, vec![3, 1, 2]);
    assert!(q
        .assessments
        .iter()
        .all(|a| a.disposition == PullRequestDisposition::Ready));

    let shape = |authored: &[&str], bump: Option<&str>| ChangeShape {
        authored: authored.iter().map(|s| s.to_string()).collect(),
        derived: Vec::new(),
        version_bump: bump.map(str::to_string),
    };
    let release = |v: &str| format!(".ai/repo/releases/{v}.yaml");
    let shapes = BTreeMap::from([
        (1, shape(&[MANIFEST, "docs/1.md"], Some("0.2.0"))),
        (2, shape(&[MANIFEST], Some("0.3.0"))),
        (3, shape(&[&release("v0.2.0")], None)),
        (4, shape(&[&release("v0.3.0")], None)),
        (6, shape(&[MANIFEST], None)),
    ]);
    let w = World {
        open: (1..=6).map(sim).collect(),
        ..Default::default()
    };
    let q = build_queue_shaped(
        &w.observation(),
        "m0",
        |p| w.relation(p.number),
        |p| shapes.get(&p.number).cloned(),
    );
    let a = |n: u64| q.get(n).unwrap();
    assert_eq!(
        a(1).authored_paths,
        shapes[&1].authored,
        "the shape's paths are the change"
    );
    assert_eq!(a(1).change_shape.as_ref(), Some(&shapes[&1]));
    assert_eq!(
        a(1).overlaps,
        vec![
            PathOverlap {
                number: 2,
                paths: vec![MANIFEST.into()],
                kind: OverlapKind::VersionBump,
            },
            PathOverlap {
                number: 6,
                paths: vec![MANIFEST.into()],
                kind: OverlapKind::Authored,
            },
        ]
    );
    assert_eq!(a(1).risk, IntegrationRisk::High);
    for factor in ["version_bump:0.2.0", "overlapping_version_bump:#2"] {
        assert!(
            a(1).risk_factors.iter().any(|f| f == factor),
            "{:?}",
            a(1).risk_factors
        );
    }
    assert_eq!(
        a(3).overlaps,
        vec![PathOverlap {
            number: 4,
            paths: vec![release("v0.2.0"), release("v0.3.0")],
            kind: OverlapKind::Release,
        }]
    );
    assert_eq!(a(3).risk, IntegrationRisk::High);
    assert!(
        a(3).risk_factors
            .iter()
            .any(|f| f == "overlapping_release:#4"),
        "{:?}",
        a(3).risk_factors
    );
    assert!(a(3)
        .risk_factors
        .iter()
        .any(|f| f == "touches .ai/repo/releases/"));
    // #5 has no shape: its relation's paths, nothing in common, low risk, and first
    assert_eq!(a(5).change_shape, None);
    assert!(a(5).overlaps.is_empty());
    assert_eq!(a(5).risk, IntegrationRisk::Low);
    assert_eq!(q.assessments[0].number, 5);
}

/// Record an observation of `prs` in `dir`, with `master` as this clone's fetched base, the
/// way `prs refresh` leaves a checkout for `queue_of`.
fn observed(dir: &std::path::Path, master: &str, prs: Vec<PullRequestObservation>) {
    git(dir, &["update-ref", "refs/remotes/origin/master", master]);
    let obs = ForgeObservation {
        schema: OBSERVATION_SCHEMA,
        repository: "owner/repo".into(),
        base: "master".into(),
        base_sha: master.into(),
        observed_at: "t0".into(),
        required_checks: Some(vec!["ci".into()]),
        review_policy: Some(Default::default()),
        up_to_date_required: Some(true),
        merge_methods: vec!["merge".into()],
        pull_requests: prs,
        resolved: Default::default(),
        delete_branch_on_merge: None,
    };
    crate::integration::store_observation(dir, &obs).unwrap();
}

fn observed_pr(number: u64, head_sha: &str) -> PullRequestObservation {
    let mut p = observe_pr(&sim(number));
    p.head_sha = head_sha.into();
    p
}

/// The keys of the relation cache `queue_and_record` left in `dir`.
fn cached_keys(dir: &std::path::Path) -> Vec<String> {
    let path = crate::integration::state_path(dir, crate::integration::RELATIONS_FILE);
    // queue_and_record writes the cache on every build, an empty one included
    let text = std::fs::read_to_string(path).unwrap();
    let cache: serde_json::Value = serde_json::from_str(&text).unwrap();
    cache["entries"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

fn commit_ids(key: &str) -> bool {
    let id = |s: &str| s.len() == 40 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    key.split_once("..").is_some_and(|(m, h)| id(m) && id(h))
}

/// A relation is decided on exactly the head the forge reported. A head that moved during the
/// refresh is not in the clone: its relation is unknown, never the one of whatever a fetched
/// ref holds now, and a name that is not a commit id is never decided or kept.
#[test]
fn the_relation_is_decided_on_the_observed_head_alone() {
    let RelationFixture { dir, master, heads } = relation_fixture();
    let moved = "1111111111111111111111111111111111111111";
    // what the fetch brought in for #1 is another head than the one observed
    git(
        &dir,
        &["update-ref", "refs/majordomus/prs/1", &heads["authored"]],
    );
    observed(
        &dir,
        &master,
        vec![
            observed_pr(1, moved),
            observed_pr(2, "HEAD"),
            observed_pr(3, &heads["authored"]),
        ],
    );
    let q = crate::integration::queue_and_record(&dir).unwrap();
    let relation = |n: u64| q.get(n).unwrap().relation.clone();
    assert!(
        matches!(relation(1), RelationToMaster::Unknown { ref reason } if reason.contains(moved)),
        "{:?}",
        relation(1)
    );
    assert_eq!(disposition(&q, 1), PullRequestDisposition::Unknown);
    assert!(
        matches!(relation(2), RelationToMaster::Unknown { .. }),
        "{:?}",
        relation(2)
    );
    assert!(
        matches!(relation(3), RelationToMaster::Behind { .. }),
        "{:?}",
        relation(3)
    );
    let keys = cached_keys(&dir);
    assert_eq!(
        keys,
        vec![format!("{master}..{}", heads["authored"])],
        "only the observed, fetched commit id is kept"
    );
    assert!(keys.iter().all(|k| commit_ids(k) && !k.contains("refs/")));
}

/// A cache written before only commit ids were kept may hold `<master>..refs/...` keys. They
/// are dropped on the next read even while master stands still, so the file never carries a
/// ref past one queue.
#[test]
fn a_ref_keyed_relation_written_earlier_is_dropped_while_master_stands_still() {
    let RelationFixture { dir, master, heads } = relation_fixture();
    observed(&dir, &master, vec![observed_pr(3, &heads["authored"])]);
    let stale = format!("{master}..refs/majordomus/prs/3");
    let path = crate::integration::state_path(&dir, crate::integration::RELATIONS_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        format!("{{\"entries\":{{\"{stale}\":{{\"kind\":\"up_to_date\",\"authored\":[]}}}}}}"),
    )
    .unwrap();
    crate::integration::queue_and_record(&dir).unwrap();
    let keys = cached_keys(&dir);
    assert!(!keys.contains(&stale), "{keys:?}");
    assert!(keys.iter().all(|k| commit_ids(k) && !k.contains("refs/")));
}

/// Enough paths that git's answer fills its stdout pipe before all of them are written: the
/// attributes are still read, because the paths are written while the answer is read.
#[test]
fn derived_paths_of_many_paths_does_not_wait_on_itself() {
    let RelationFixture { dir, master, .. } = relation_fixture();
    let mut paths: Vec<String> = (0..20_000)
        .map(|i| format!("site/data/generated/a-rather-long-directory-name/file-{i:05}.json"))
        .collect();
    // the one derived path, last: its answer comes only after every other
    paths.push("gen.json".into());
    let derived = super::relation::derived_paths(&dir, &master, &paths).unwrap();
    assert_eq!(derived.into_iter().collect::<Vec<_>>(), vec!["gen.json"]);
}

/// A repository whose master's `.gitattributes` cannot be read, and a head that changes only
/// the derived file it names: git merges and diffs it, and cannot say which path is derived.
fn unreadable_attributes() -> (std::path::PathBuf, String, String) {
    let dir = scratch();
    git(&dir, &["init", "-q", "-b", "master"]);
    std::fs::write(dir.join(".gitattributes"), "gen.json merge=derived\n").unwrap();
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    std::fs::write(dir.join("gen.json"), "{}\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);
    let head = commit_on(&dir, "derived", &base, &[("gen.json", "{\"x\":1}\n")]);
    let master = commit_on(&dir, "master", &base, &[("a.txt", "two\n")]);
    // corrupt the attributes blob: every command but check-attr leaves it unread
    let blob = git(&dir, &["rev-parse", &format!("{master}:.gitattributes")]);
    let loose = dir.join(".git/objects").join(&blob[..2]).join(&blob[2..]);
    std::fs::remove_file(&loose).unwrap();
    std::fs::write(&loose, b"not an object").unwrap();
    (dir, master, head)
}

#[test]
fn an_attribute_read_that_fails_is_unknown_and_never_cached() {
    use crate::integration::relation_to_master;
    let (dir, master, head) = unreadable_attributes();
    let r = relation_to_master(&dir, &master, &head);
    // without the attribute, gen.json would be called authored and the head behind
    assert!(
        matches!(r, RelationToMaster::Unknown { ref reason } if reason.contains("check-attr")),
        "{r:?}"
    );
    observed(&dir, &master, vec![observed_pr(1, &head)]);
    let q = crate::integration::queue_and_record(&dir).unwrap();
    assert_eq!(disposition(&q, 1), PullRequestDisposition::Unknown);
    assert!(cached_keys(&dir).is_empty(), "{:?}", cached_keys(&dir));
}

#[test]
fn derived_paths_says_when_git_cannot_answer() {
    use crate::integration::relation::derived_paths;
    let none = derived_paths(std::path::Path::new("/nonexistent"), "m", &[]);
    assert_eq!(none, Ok(Default::default()), "no path asks nothing");
    let err = derived_paths(
        std::path::Path::new("/nonexistent/majordomus"),
        "m",
        &["a".to_string()],
    );
    assert!(
        matches!(&err, Err(e) if e.contains("could not run")),
        "{err:?}"
    );
}

#[test]
fn a_dependency_is_a_declaration_not_a_mention() {
    use crate::integration::declared_dependencies;
    assert_eq!(
        declared_dependencies("Stacked on #601.\nSee #12 for context."),
        vec![601]
    );
    assert_eq!(
        declared_dependencies("Depends on #644 and #645"),
        vec![644, 645]
    );
    assert!(declared_dependencies("fixes #12").is_empty());
}

/// A dependency marker opens its line, after at most a bullet, a quote or emphasis. A
/// word in a sentence is prose: "thereafter #5" and "introduced after #540" declare nothing.
#[test]
fn a_dependency_marker_starts_its_line() {
    use crate::integration::declared_dependencies as deps;
    let none: Vec<u64> = Vec::new();
    for (body, want) in [
        ("- Depends on #7", vec![7]),
        ("> **Depends on:** #7", vec![7]),
        ("* Stacked on #8 and #9.", vec![8, 9]),
        ("1. Requires #10", vec![10]),
        ("Land after #11", vec![11]),
        ("_Land after_ #12", vec![12]),
        ("Depends on #14, #15 and #16", vec![14, 15, 16]),
        ("thereafter #5", none.clone()),
        ("Fixes the regression introduced after #540.", none.clone()),
        ("After #3 landed, this became possible.", none.clone()),
        (
            "This change requires #9 to be reverted first.",
            none.clone(),
        ),
        (
            "See the discussion; it depends on #13 somewhat.",
            none.clone(),
        ),
        ("Dependson #17", none.clone()),
        ("Stacked onto #18", none.clone()),
        ("Requires2 #19", none.clone()),
        ("Depends on #x", none.clone()),
        ("Requires #20 & #21", vec![20, 21]),
        ("+ Requires #22", vec![22]),
        ("12 depends on #23", none.clone()),
    ] {
        assert_eq!(deps(body), want, "{body:?}");
    }
}

#[test]
fn every_disposition_has_one_word_and_one_lane() {
    let words: std::collections::BTreeSet<&str> = PullRequestDisposition::ALL
        .iter()
        .map(|d| d.as_str())
        .collect();
    assert_eq!(words.len(), PullRequestDisposition::ALL.len());
    assert_eq!(
        PullRequestDisposition::NeedsRefresh.as_str(),
        "needs_refresh"
    );
    assert_eq!(
        PullRequestDisposition::Ready.lane(),
        crate::integration::IntegrationLane::Ready
    );
    assert_eq!(PullRequestDisposition::Unsafe.as_str(), "unsafe");
    assert_eq!(
        PullRequestDisposition::Unsafe.lane(),
        crate::integration::IntegrationLane::Held
    );
}

// ---------------------------------------------------------------- waiting and starvation

use crate::integration::wait;

#[test]
fn the_executor_records_who_waited_and_who_was_passed_over() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(3)],
        ..Default::default()
    };
    let report = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert_eq!(report.merged, vec![1]);
    let trail = drain::events(&root);
    let became: Vec<u64> = trail
        .iter()
        .filter(|e| e.action == wait::BECAME_ACTIONABLE)
        .filter_map(|e| e.pr)
        .collect();
    assert_eq!(
        became,
        vec![1, 3],
        "both were actionable when first observed"
    );
    let selected = trail
        .iter()
        .find(|e| e.action.as_str() == "selected")
        .unwrap();
    assert_eq!(selected.pr, Some(1));
    assert_eq!(selected.passed_over, vec![3], "#3 was ready and not chosen");

    let waits = wait::waits(&trail);
    assert!(
        !waits.contains_key(&1),
        "a merged pull request waits for nothing"
    );
    let w3 = &waits[&3];
    assert_eq!(w3.passed_over, 1);
    assert_eq!(w3.last_passed_over.as_ref().unwrap().for_pr, 1);

    // the next step: #3 is behind now but still the executor's (refreshable), so its wait
    // goes on — it is not restarted by the change of disposition
    drain::drain(&root, &mut w, 1, false, false).unwrap();
    let trail = drain::events(&root);
    assert!(!trail
        .iter()
        .any(|e| e.action == wait::LEFT_ACTIONABLE && e.pr == Some(3)));
    let mut q = w.queue();
    wait::annotate(&mut q, &trail);
    assert_eq!(q.get(3).unwrap().wait.as_ref().unwrap().passed_over, 1);
}

#[test]
fn a_pull_request_that_stops_being_actionable_leaves_its_wait() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(5)],
        ..Default::default()
    };
    wait::record_transitions(&root, &w.queue()).unwrap();
    assert!(wait::waits(&drain::events(&root)).contains_key(&5));
    // its check fails on the next look: it needs repair, a person's work, not the executor's
    w.open[0].failing = true;
    assert_eq!(wait::record_transitions(&root, &w.queue()).unwrap(), 1);
    let trail = drain::events(&root);
    let left = trail.last().unwrap();
    assert_eq!(left.action, wait::LEFT_ACTIONABLE);
    assert_eq!(left.detail, "it is needs_repair now");
    assert!(wait::waits(&trail).is_empty());
    // and nothing is recorded when nothing changed
    assert_eq!(wait::record_transitions(&root, &w.queue()).unwrap(), 0);
}

/// The trail records what a failed `git` or `gh` said, and that text names the remote in
/// whatever form it was configured. The trail is served over HTTP and MCP, so a credential
/// in such a URL is removed where every event is written — by this tool's own rule, not by
/// trusting the subprocess to have anonymised its message.
#[test]
fn a_relayed_error_is_recorded_without_the_credential_in_its_url() {
    let root = scratch();
    let said = "git ls-remote failed: fatal: unable to access \
                'https://x-access-token:ghs_trailsecret0001@example.invalid/o/r.git/': \
                Could not resolve host: example.invalid";
    let written = drain::record(
        &root,
        drain::IntegrationEvent {
            pr: Some(1),
            detail: said.into(),
            ..drain::IntegrationEvent::of(drain::IntegrationAction::RefreshFailed)
        },
    )
    .unwrap();
    assert!(
        !written.detail.contains("ghs_trailsecret0001"),
        "{}",
        written.detail
    );
    assert!(
        written
            .detail
            .contains("'https://example.invalid/o/r.git/'"),
        "the URL itself is kept, so the event still says which remote: {}",
        written.detail
    );
    assert!(written.detail.contains("Could not resolve host"));
    // what was returned is what is on disk, and what a reader of the trail is served
    let trail = drain::events(&root);
    assert_eq!(trail.last().unwrap().detail, written.detail);
    let raw = std::fs::read_to_string(crate::integration::events_path(&root).unwrap()).unwrap();
    assert!(!raw.contains("ghs_trailsecret0001"));
    // and the failure is still classified from what was said
    assert!(written.class.is_some());
}

#[test]
fn starvation_is_visible_and_changes_no_rank() {
    let root = scratch();
    let w = World {
        open: vec![sim(1), sim(2)],
        ..Default::default()
    };
    let q = w.queue();
    wait::record_transitions(&root, &q).unwrap();
    for _ in 0..wait::STARVING_AFTER {
        drain::record(
            &root,
            drain::IntegrationEvent {
                pr: Some(1),
                passed_over: vec![2],
                ..drain::IntegrationEvent::of(drain::IntegrationAction::Selected)
            },
        )
        .unwrap();
    }
    let mut annotated = q.clone();
    wait::annotate(&mut annotated, &drain::events(&root));
    assert_eq!(annotated.starving, vec![2]);
    assert_eq!(
        annotated.get(2).unwrap().wait.as_ref().unwrap().passed_over,
        wait::STARVING_AFTER
    );
    let order = |q: &IntegrationQueue| q.assessments.iter().map(|a| a.number).collect::<Vec<_>>();
    assert_eq!(order(&annotated), order(&q), "a long wait reorders nothing");
    assert_eq!(annotated.next_merge, q.next_merge);
}

// ---------------------------------------------------------------- continuous

fn continuous_opts(cycles: Option<usize>) -> drain::ContinuousOptions {
    drain::ContinuousOptions {
        max_per_cycle: 1,
        allow_refresh: false,
        interval: std::time::Duration::from_secs(60),
        cycles,
    }
}

#[test]
fn a_continuous_drain_observes_afresh_in_every_cycle() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(3)],
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let mut slept = std::time::Duration::ZERO;
    let mut heard = Vec::new();
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(Some(3)),
        &stop,
        &mut |d| slept += d,
        &mut |n, r| heard.push((n, r.merged.clone())),
    );
    assert_eq!(report.cycles, 3);
    assert_eq!(
        report.merged,
        vec![1],
        "#3 is behind after #1 and refresh is not allowed"
    );
    assert!(report.failure.is_none());
    assert!(report.stopped.contains("3 cycle(s)"), "{}", report.stopped);
    assert_eq!(heard, vec![(1, vec![1]), (2, vec![]), (3, vec![])]);
    // waited between cycles and not after the last, in slices that add up to the interval
    assert_eq!(slept, std::time::Duration::from_secs(120));
    // the merge cycle observed twice, each idle cycle once: nothing was carried over
    assert_eq!(w.observations, 4);
}

#[test]
fn a_stop_is_honoured_at_the_next_safe_point() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(None),
        &stop,
        // a signal arrives during the first wait
        &mut |_| stop.store(true, std::sync::atomic::Ordering::SeqCst),
        &mut |_, _| {},
    );
    assert_eq!(report.cycles, 1);
    assert_eq!(report.merged, vec![1]);
    assert!(
        report.stopped.starts_with("asked to stop"),
        "{}",
        report.stopped
    );

    // a stop before anything ran runs nothing
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(true);
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(None),
        &stop,
        &mut |_| {},
        &mut |_, _| {},
    );
    assert_eq!((report.cycles, w.observations), (0, 0));
}

#[test]
fn a_repository_that_cannot_be_read_ends_a_continuous_drain() {
    struct Down;
    impl Integrator for Down {
        fn observe(&mut self) -> Result<IntegrationQueue, String> {
            Err("HTTP 401: Bad credentials".into())
        }
        fn merge(&mut self, _: u64, _: &str, _: &str) -> Result<(), String> {
            unreachable!()
        }
        fn verify(
            &mut self,
            _: u64,
            _: &crate::integration::EvaluatedAgainst,
            _: &str,
        ) -> Result<drain::Landed, drain::NotLanded> {
            unreachable!()
        }
        fn refresh_branch(&mut self, _: &PullRequestAssessment, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn close(&mut self, _: u64, _: &str, _: &str) -> Result<(), String> {
            unreachable!()
        }
    }
    let stop = std::sync::atomic::AtomicBool::new(false);
    let report = drain::continuous(
        &scratch(),
        &mut Down,
        continuous_opts(None),
        &stop,
        &mut |_| panic!("waited after a systemic failure"),
        &mut |_, _| {},
    );
    assert_eq!(report.cycles, 0);
    assert_eq!(report.failure.as_deref(), Some("HTTP 401: Bad credentials"));
}

// ---------------------------------------------------------------- the audit trail

/// The outcome's wire word: what the command line and a capability read.
fn outcome_word(out: &DrainStepOutcome) -> String {
    serde_json::to_value(out).unwrap()["outcome"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn trail_actions(root: &std::path::Path) -> Vec<String> {
    drain::events(root)
        .into_iter()
        .map(|e| e.action.as_str().to_string())
        .collect()
}

#[test]
fn a_trail_that_cannot_be_written_merges_nothing() {
    let root = scratch();
    unwritable(&trail_of(&root));
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let out = drain::step(&root, &mut w, false, false);
    assert!(out.is_err(), "a step went on without its trail: {out:?}");
    assert_eq!(w.merge_calls, 0, "a merge reached the forge unrecorded");
    assert!(w.merged.is_empty());
}

#[test]
fn a_merge_whose_attempt_cannot_be_recorded_is_refused() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        // the trail breaks between the selection and the merge it would precede
        sabotage_on: Some((2, trail_of(&root))),
        ..Default::default()
    };
    let out = drain::step(&root, &mut w, false, false).expect("a typed refusal, not an error");
    assert_eq!(outcome_word(&out), "trail_unwritable", "{out:?}");
    assert_eq!(w.merge_calls, 0, "a merge reached the forge unrecorded");
    assert!(w.merged.is_empty());
}

#[test]
fn a_refresh_whose_attempt_cannot_be_recorded_is_refused() {
    let root = scratch();
    let mut w = one_behind();
    w.sabotage_on = Some((2, trail_of(&root)));
    let out = drain::step(&root, &mut w, false, true).expect("a typed refusal, not an error");
    assert_eq!(outcome_word(&out), "trail_unwritable", "{out:?}");
    assert_eq!(w.refresh_calls, 0, "a refresh was pushed unrecorded");
    assert!(w.open.iter().all(|s| s.contains == 0));
}

#[test]
fn a_drain_stops_on_a_trail_that_cannot_be_written() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        sabotage_on: Some((2, trail_of(&root))),
        ..Default::default()
    };
    let report = drain::drain(&root, &mut w, 2, false, false).unwrap();
    assert_eq!(report.steps.len(), 1, "{report:?}");
    assert!(report.merged.is_empty());
    assert!(report.stopped.contains("trail"), "{}", report.stopped);
    assert_eq!(w.merge_calls, 0);
}

/// One trail line exactly as the executor wrote it before its action was typed.
fn old_line(action: &str) -> String {
    format!(
        r#"{{"at":"2026-10-01T00:00:00Z","actor":"t (pid 1 on h)","action":"{action}","pr":1,"master_before":"m0","head_sha":"h1","master_after":null,"reasons":["ready"],"detail":"d"}}"#
    )
}

#[test]
fn trail_lines_written_with_a_string_action_still_parse() {
    for word in [
        "selected",
        "refresh_selected",
        "stale_decision",
        "merge_attempted",
        "merge_succeeded",
        "merge_failed",
        "verification_failed",
        "refreshed",
        "refresh_failed",
        "closed_superseded",
        "closed_redundant",
        "idle",
        "became_actionable",
        "left_actionable",
    ] {
        let line = old_line(word);
        let e: drain::IntegrationEvent =
            serde_json::from_str(&line).unwrap_or_else(|err| panic!("{word}: {err}"));
        assert_eq!(e.action.as_str(), word);
        let again = serde_json::to_value(&e).unwrap();
        assert_eq!(again["action"], word, "the wire word is unchanged");
        for absent in ["evidence", "class", "merge_commit"] {
            assert!(
                again.get(absent).is_none(),
                "{absent} is written when empty"
            );
        }
    }
    // a word the executor never writes is not an action
    assert!(serde_json::from_str::<drain::IntegrationEvent>(&old_line("merge_suceeded")).is_err());
}

#[test]
fn a_checkouts_own_trail_is_moved_into_the_repositorys_once() {
    let root = scratch();
    let old = root.join(".ai/local/state/integration/events.jsonl");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(
        &old,
        format!(
            "{}\n{}\n",
            old_line("selected"),
            old_line("merge_succeeded")
        ),
    )
    .unwrap();
    let lines = |p: &std::path::Path| {
        std::fs::read_to_string(p)
            .map(|t| t.lines().count())
            .unwrap_or(0)
    };
    assert_eq!(trail_actions(&root), ["selected", "merge_succeeded"]);
    assert_eq!(
        lines(&trail_of(&root)),
        2,
        "the repository's trail holds them"
    );
    // asked again, nothing is appended twice
    assert_eq!(drain::events(&root).len(), 2);
    assert_eq!(lines(&trail_of(&root)), 2);
    // nor after the repository's trail is removed: the move happens once, ever
    std::fs::remove_file(trail_of(&root)).unwrap();
    assert!(drain::events(&root).is_empty());
    assert_eq!(lines(&trail_of(&root)), 0);
}

#[test]
fn the_lease_is_recorded_when_taken_and_when_given_back() {
    let root = scratch();
    {
        let _lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
        assert_eq!(trail_actions(&root), ["lease_acquired"]);
    }
    assert_eq!(trail_actions(&root), ["lease_acquired", "lease_released"]);
}

#[test]
fn a_continuous_drain_is_recorded_when_it_starts_and_stops() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(Some(1)),
        &stop,
        &mut |_| {},
        &mut |_, _| {},
    );
    assert_eq!(report.merged, vec![1]);
    let trail = drain::events(&root);
    assert_eq!(
        trail.first().map(|e| e.action.as_str()),
        Some("continuous_started")
    );
    let last = trail.last().unwrap();
    assert_eq!(last.action.as_str(), "continuous_stopped");
    assert_eq!(last.detail, report.stopped);
}

/// Every action, in the order an executor's run meets them.
const ALL_ACTIONS: [drain::IntegrationAction; 28] = [
    drain::IntegrationAction::LeaseAcquired,
    drain::IntegrationAction::LeaseReleased,
    drain::IntegrationAction::ContinuousStarted,
    drain::IntegrationAction::ContinuousStopped,
    drain::IntegrationAction::Observed,
    drain::IntegrationAction::BecameActionable,
    drain::IntegrationAction::LeftActionable,
    drain::IntegrationAction::Selected,
    drain::IntegrationAction::RefreshSelected,
    drain::IntegrationAction::StaleDecision,
    drain::IntegrationAction::MergeAttempted,
    drain::IntegrationAction::MergeSucceeded,
    drain::IntegrationAction::MergeFailed,
    drain::IntegrationAction::VerificationFailed,
    drain::IntegrationAction::RefreshAttempted,
    drain::IntegrationAction::Refreshed,
    drain::IntegrationAction::RefreshFailed,
    drain::IntegrationAction::CloseAttempted,
    drain::IntegrationAction::ClosedRedundant,
    drain::IntegrationAction::ClosedSuperseded,
    drain::IntegrationAction::CloseFailed,
    drain::IntegrationAction::RepairSelected,
    drain::IntegrationAction::RepairAttempted,
    drain::IntegrationAction::Repaired,
    drain::IntegrationAction::RepairRefused,
    drain::IntegrationAction::Idle,
    drain::IntegrationAction::FailureAcknowledged,
    drain::IntegrationAction::ObserveFailed,
];

/// A new action must be added to [`ALL_ACTIONS`] before this compiles: no wildcard.
#[allow(dead_code)]
fn every_action_is_listed(a: drain::IntegrationAction) {
    use drain::IntegrationAction as A;
    match a {
        A::LeaseAcquired
        | A::LeaseReleased
        | A::ContinuousStarted
        | A::ContinuousStopped
        | A::Observed
        | A::BecameActionable
        | A::LeftActionable
        | A::Selected
        | A::RefreshSelected
        | A::StaleDecision
        | A::MergeAttempted
        | A::MergeSucceeded
        | A::MergeFailed
        | A::VerificationFailed
        | A::RefreshAttempted
        | A::Refreshed
        | A::RefreshFailed
        | A::CloseAttempted
        | A::ClosedSuperseded
        | A::ClosedRedundant
        | A::CloseFailed
        | A::RepairSelected
        | A::RepairAttempted
        | A::Repaired
        | A::RepairRefused
        | A::Idle
        | A::FailureAcknowledged
        | A::ObserveFailed => (),
    }
}

#[test]
fn every_action_has_one_word_and_it_is_the_wire_word() {
    let words: BTreeSet<&str> = ALL_ACTIONS.iter().map(|a| a.as_str()).collect();
    assert_eq!(words.len(), ALL_ACTIONS.len());
    for a in ALL_ACTIONS {
        assert_eq!(serde_json::to_value(a).unwrap(), a.as_str());
        let back: drain::IntegrationAction =
            serde_json::from_value(serde_json::json!(a.as_str())).unwrap();
        assert_eq!(back, a);
        // padded like any word, for the command line's columns
        assert_eq!(format!("{a:<24}|").len(), 25, "{a:?}");
    }
}

#[test]
fn a_failure_class_round_trips_on_an_event() {
    use drain::FailureClass as C;
    for class in [
        C::Stale,
        C::Conflict,
        C::NewFailingCheck,
        C::ReviewRevoked,
        C::Transient,
        C::PolicyViolation,
        C::VerificationFailed,
        C::Unreadable,
    ] {
        let e = drain::IntegrationEvent {
            class: Some(class),
            merge_commit: Some("c".repeat(40)),
            ..drain::IntegrationEvent::of(drain::IntegrationAction::MergeFailed)
        };
        let line = serde_json::to_string(&e).unwrap();
        let back: drain::IntegrationEvent = serde_json::from_str(&line).unwrap();
        assert_eq!(back, e.clone());
        assert!(format!("{back:?}").contains(&format!("{class:?}")));
    }
}

#[test]
fn the_acts_carry_the_evidence_they_were_decided_on() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    drain::drain(&root, &mut w, 1, false, false).unwrap();
    let trail = drain::events(&root);
    let of = |word: &str| {
        trail
            .iter()
            .find(|e| e.action.as_str() == word)
            .unwrap_or_else(|| panic!("no {word}"))
    };
    for word in ["merge_attempted", "merge_succeeded"] {
        assert!(!of(word).evidence.is_empty(), "{word} carries no evidence");
    }
    assert!(of("selected").evidence.is_empty());

    let root = scratch();
    let mut w = one_behind();
    drain::step(&root, &mut w, false, true).unwrap();
    let trail = drain::events(&root);
    let actions: Vec<&str> = trail.iter().map(|e| e.action.as_str()).collect();
    assert!(
        actions.ends_with(&["refresh_selected", "refresh_attempted", "refreshed"]),
        "{actions:?}"
    );
    assert!(!trail[actions.len() - 3].evidence.is_empty());
}

#[test]
fn a_closure_is_recorded_before_and_after_it_reaches_the_forge() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        ..Default::default()
    };
    w.open[0].redundant_after = Some(99);
    w.open[1].redundant_after = Some(99);
    w.merged = vec![99];
    w.close_refuses.insert(2);
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    let by: BTreeMap<u64, String> = items.into_iter().map(|i| (i.pr, i.action)).collect();
    assert_eq!(by[&1], "closed");
    assert!(by[&2].starts_with("close_failed: HTTP 403"), "{}", by[&2]);
    let trail = drain::events(&root);
    let said: Vec<(&str, Option<u64>)> = trail.iter().map(|e| (e.action.as_str(), e.pr)).collect();
    assert_eq!(
        said,
        [
            ("close_attempted", Some(1)),
            ("closed_redundant", Some(1)),
            ("close_attempted", Some(2)),
            ("close_failed", Some(2)),
        ]
    );
    assert!(
        !trail[1].evidence.is_empty(),
        "the closure names its evidence"
    );
}

#[test]
fn a_closure_the_trail_cannot_record_is_not_made() {
    let root = scratch();
    unwritable(&trail_of(&root));
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    w.open[0].redundant_after = Some(99);
    w.merged = vec![99];
    let err = drain::cleanup(&root, &mut w, true).unwrap_err();
    assert!(err.contains("#1 was not closed"), "{err}");
    assert_eq!(w.close_calls, 0, "a closure reached the forge unrecorded");
}

#[test]
fn a_lease_the_trail_cannot_record_is_given_back() {
    let root = scratch();
    unwritable(&trail_of(&root));
    let err = match drain::IntegrationLease::acquire(&root, "master") {
        Ok(_) => panic!("a lease was held with nothing recorded"),
        Err(e) => e,
    };
    assert!(err.contains("given back"), "{err}");
    assert!(drain::IntegrationLease::read(&root, "master")
        .unwrap()
        .is_none());
}

#[test]
fn a_release_the_trail_cannot_record_still_releases() {
    let root = scratch();
    let lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
    unwritable(&trail_of(&root));
    drop(lease);
    assert!(drain::IntegrationLease::read(&root, "master")
        .unwrap()
        .is_none());
}

#[test]
fn a_continuous_drain_without_a_trail_attempts_nothing() {
    let root = scratch();
    unwritable(&trail_of(&root));
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(None),
        &stop,
        &mut |_| panic!("waited with no trail"),
        &mut |_, _| {},
    );
    assert_eq!((report.cycles, w.observations), (0, 0));
    assert!(report.failure.is_some());
}

#[test]
fn a_continuous_drain_stops_when_its_trail_breaks() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        sabotage_on: Some((2, trail_of(&root))),
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(None),
        &stop,
        &mut |_| panic!("waited after the trail broke"),
        &mut |_, _| {},
    );
    assert_eq!(report.cycles, 1);
    assert_eq!(w.merge_calls, 0);
    // the failure that stopped it, not the stop it could not record afterwards
    let failure = report.failure.unwrap();
    assert!(failure.contains("events.jsonl"), "{failure}");
    assert!(
        report.stopped.contains("merge_attempted"),
        "{}",
        report.stopped
    );
}

#[test]
fn outside_a_repository_there_is_no_trail_to_read_or_write() {
    let dir = unique_temp("mj-integration-plain");
    assert!(drain::events(&dir).is_empty());
    assert!(drain::record(
        &dir,
        drain::IntegrationEvent::of(drain::IntegrationAction::Idle)
    )
    .is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// One scripted run of `scenario` from a fresh repository; what reached the forge is on the
/// world returned. Its result is not looked at: a run whose trail write failed may end in an
/// error or a refusal, and what is held is only that no act went unrecorded.
fn run_scenario(root: &std::path::Path, scenario: &str) -> World {
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    match scenario {
        "merge" => drop(drain::drain(root, &mut w, 1, false, false)),
        "refused" => {
            w.merge_refuses.insert(1);
            drop(drain::step(root, &mut w, false, false));
        }
        "unverified" => {
            w.verify_fails = true;
            drop(drain::step(root, &mut w, false, false));
        }
        "stale" => {
            w.master_moves_on = Some(2);
            drop(drain::step(root, &mut w, false, false));
        }
        "idle" => {
            w.open.clear();
            drop(drain::step(root, &mut w, false, false));
        }
        "left" => {
            // #1 was actionable when last recorded, and its check fails now
            let k = drain::FAIL_WRITE.with(|n| n.replace(0));
            wait::record_transitions(root, &w.queue()).unwrap();
            drain::FAIL_WRITE.with(|n| n.set(k));
            w.open[0].failing = true;
            drop(drain::step(root, &mut w, false, false));
        }
        "refresh" | "refresh_fails" | "refresh_stale" => {
            w = one_behind();
            w.refresh_fails = scenario == "refresh_fails";
            if scenario == "refresh_stale" {
                w.meanwhile = vec![(2, Meanwhile::HeadMoves(1))];
            }
            drop(drain::step(root, &mut w, false, true));
        }
        "cleanup" => {
            w.open = vec![sim(1), sim(2)];
            w.open[0].redundant_after = Some(99);
            w.open[1].redundant_after = Some(99);
            w.merged = vec![99];
            w.close_refuses.insert(2);
            drop(drain::cleanup(root, &mut w, true));
        }
        "continuous" => {
            drain::continuous(
                root,
                &mut w,
                continuous_opts(Some(2)),
                &stop,
                &mut |_| {},
                &mut |_, _| {},
            );
        }
        "no_method" => {
            w.no_method_on = Some(2);
            drop(drain::step(root, &mut w, false, false));
        }
        "lost_landed" | "lost_unproved" => {
            w.answer_lost_after_landing.insert(1);
            w.verify_fails = scenario == "lost_unproved";
            drop(drain::step(root, &mut w, false, false));
        }
        "reconcile_landed" | "reconcile_never" | "reconcile_unproved" => {
            let k = drain::FAIL_WRITE.with(|n| n.replace(0));
            attempted(root, 1);
            drain::FAIL_WRITE.with(|n| n.set(k));
            w = asked_before(scenario);
            drop(drain::drain(root, &mut w, 1, false, false));
        }
        "cleanup_stale" => {
            w.open[0].redundant_after = Some(99);
            w.merged = vec![99];
            w.meanwhile.push((2, Meanwhile::Closes(1)));
            drop(drain::cleanup(root, &mut w, true));
        }
        // "lease": taken and given back
        _ => drop(drain::IntegrationLease::acquire(root, "master")),
    }
    w
}

/// The trail of an executor that asked the forge to merge `pr` onto master `m0` and stopped
/// before it learnt how that ended.
fn attempted(root: &std::path::Path, pr: u64) {
    drain::record(
        root,
        drain::IntegrationEvent {
            pr: Some(pr),
            master_before: Some(master_sha(0)),
            head_sha: Some(sim(pr).head),
            detail: "--merge".into(),
            ..drain::IntegrationEvent::of(drain::IntegrationAction::MergeAttempted)
        },
    )
    .unwrap();
}

/// The world after the merge of #1 [`attempted`] asked for: `reconcile_landed` — it landed
/// where it was decided; `reconcile_unproved` — it landed and nothing proves where;
/// `reconcile_never` — it never landed.
fn asked_before(how: &str) -> World {
    let mut w = World::default();
    if how != "reconcile_never" {
        w.merged_onto.insert(1, (master_sha(0), sim(1).head));
        w.master = 1;
    }
    w.verify_fails = how == "reconcile_unproved";
    w
}

#[test]
fn every_trail_write_can_fail_and_no_act_goes_unrecorded() {
    for scenario in [
        "merge",
        "refused",
        "unverified",
        "stale",
        "idle",
        "left",
        "refresh",
        "refresh_fails",
        "refresh_stale",
        "cleanup",
        "continuous",
        "lease",
        "no_method",
        "lost_landed",
        "lost_unproved",
        "reconcile_landed",
        "reconcile_never",
        "reconcile_unproved",
        "cleanup_stale",
    ] {
        // the k-th write fails, for every k until a run makes fewer than k writes
        for k in 1..64 {
            let root = scratch();
            drain::FAIL_WRITE.with(|n| n.set(k));
            let w = run_scenario(&root, scenario);
            let unspent = drain::FAIL_WRITE.with(|n| n.replace(0));
            let trail = drain::events(&root);
            let count = |a: &str| trail.iter().filter(|e| e.action.as_str() == a).count();
            let at = format!("{scenario}, write {k} failing");
            assert!(w.merge_calls <= count("merge_attempted"), "{at}: {trail:?}");
            assert!(w.refresh_calls <= count("refresh_attempted"), "{at}");
            assert!(w.close_calls <= count("close_attempted"), "{at}");
            if unspent > 0 {
                assert!(k > 1, "{scenario} wrote nothing at all");
                break;
            }
            assert!(k < 63, "{scenario} never ran out of writes");
        }
    }
}

#[test]
fn a_lease_taken_over_is_not_released_by_its_old_holder() {
    let root = scratch();
    let lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    std::fs::write(&path, "another holder").unwrap();
    drop(lease);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "another holder");
    assert_eq!(trail_actions(&root), ["lease_acquired"]);
}

#[test]
fn a_trail_that_cannot_be_moved_is_neither_read_nor_written() {
    let root = scratch();
    let old = root.join(".ai/local/state/integration/events.jsonl");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, format!("{}\n", old_line("selected"))).unwrap();
    // the repository's state directory is a file: nothing can be made under it
    std::fs::create_dir_all(root.join(".git/majordomus")).unwrap();
    std::fs::write(root.join(".git/majordomus/integration"), "").unwrap();
    assert!(drain::events(&root).is_empty());
    let err = drain::record(
        &root,
        drain::IntegrationEvent::of(drain::IntegrationAction::Idle),
    )
    .unwrap_err();
    assert!(err.contains("events.jsonl"), "{err}");
}

#[test]
fn outside_a_repository_there_is_no_summary() {
    let dir = unique_temp("mj-integration-nosummary");
    assert!(super::QueueSummary::load(&dir).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- executors racing (WP27)
//
// Two executors started at the same instant on one base: exactly one holds the lease at any
// moment, and no pull request is merged twice. Threads here; two processes, and two
// worktrees of one repository, in cases 855 and 856.

/// How many executors race in each round, and how many rounds: a race lost once in a
/// thousand starts must still lose inside one run of the suite.
const RACERS: usize = 8;
const ROUNDS: usize = 25;

/// Every executor of a round takes the lease at one instant and keeps whatever it got until
/// every other one has tried; the number of leases held at once is returned.
fn race_for_the_lease(root: &std::path::Path) -> usize {
    use std::sync::{Arc, Barrier};
    let start = Arc::new(Barrier::new(RACERS));
    let tried = Arc::new(Barrier::new(RACERS));
    let racers: Vec<_> = (0..RACERS)
        .map(|_| {
            let (root, start, tried) = (root.to_path_buf(), start.clone(), tried.clone());
            std::thread::spawn(move || {
                start.wait();
                let lease = drain::IntegrationLease::acquire(&root, "master");
                tried.wait();
                match lease {
                    Ok(_) => true,
                    Err(e) => {
                        // refused, by name: never a lease taken and lost unsaid
                        assert!(
                            e.contains("another integration executor holds")
                                || e.contains("could not be taken"),
                            "{e}"
                        );
                        false
                    }
                }
            })
        })
        .collect();
    racers
        .into_iter()
        .map(|r| r.join().expect("a racer panicked"))
        .filter(|won| *won)
        .count()
}

#[test]
fn executors_started_at_one_instant_take_one_lease() {
    for round in 0..ROUNDS {
        let root = scratch();
        assert_eq!(race_for_the_lease(&root), 1, "round {round}");
        // the winner took it once and gave it back once; the losers recorded nothing
        assert_eq!(
            trail_actions(&root),
            ["lease_acquired", "lease_released"],
            "round {round}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn executors_reclaiming_one_stale_lease_take_it_once() {
    for round in 0..ROUNDS {
        let root = scratch();
        // a holder that stopped without releasing: its lease is past LEASE_STALE_AFTER
        let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "a holder that is gone").unwrap();
        age_lease(&path, drain::LEASE_STALE_AFTER * 2);
        assert_eq!(race_for_the_lease(&root), 1, "round {round}");
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// Make the lease file's record look `age` old, as a holder that stopped renewing leaves it.
fn age_lease(path: &std::path::Path, age: std::time::Duration) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .and_then(|f| f.set_modified(std::time::SystemTime::now() - age))
        .unwrap();
}

#[test]
fn a_record_whose_holder_is_gone_is_taken_over_at_once() {
    let root = scratch();
    // a holder that crashed a moment ago: its record is fresh, but nobody holds the lock
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"pid":1,"host":"gone","base":"master","since":0}"#,
    )
    .unwrap();
    let lease = drain::IntegrationLease::acquire(&root, "master").expect("taken over");
    let held = drain::IntegrationLease::read(&root, "master")
        .unwrap()
        .unwrap();
    assert_eq!(
        held.holder.unwrap().pid,
        std::process::id(),
        "the record is the new holder's"
    );
    drop(lease);
    assert!(drain::IntegrationLease::read(&root, "master")
        .unwrap()
        .is_none());
}

#[test]
fn a_live_holder_is_never_taken_over_however_old_its_record() {
    let root = scratch();
    let lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    age_lease(&path, drain::LEASE_STALE_AFTER * 2);
    // an observer calls it stale; an executor still cannot take it
    assert!(
        drain::IntegrationLease::read(&root, "master")
            .unwrap()
            .unwrap()
            .stale
    );
    let err = match drain::IntegrationLease::acquire(&root, "master") {
        Ok(_) => panic!("a live holder's lease was taken over"),
        Err(e) => e,
    };
    assert!(err.contains("another integration executor holds"), "{err}");
    // renewing writes through the held file, and freshens it
    lease.renew().unwrap();
    assert!(
        !drain::IntegrationLease::read(&root, "master")
            .unwrap()
            .unwrap()
            .stale
    );
    drop(lease);
    assert_eq!(trail_actions(&root), ["lease_acquired", "lease_released"]);
}

/// One scripted forge that several executors observe and merge through.
struct Shared(std::sync::Arc<std::sync::Mutex<World>>);

impl Integrator for Shared {
    fn observe(&mut self) -> Result<IntegrationQueue, String> {
        self.0.lock().unwrap().observe()
    }
    fn merge(&mut self, pr: u64, head_sha: &str, method: &str) -> Result<(), String> {
        self.0.lock().unwrap().merge(pr, head_sha, method)
    }
    fn verify(
        &mut self,
        pr: u64,
        at: &crate::integration::EvaluatedAgainst,
        method: &str,
    ) -> Result<drain::Landed, drain::NotLanded> {
        self.0.lock().unwrap().verify(pr, at, method)
    }
    fn refresh_branch(&mut self, a: &PullRequestAssessment, base: &str) -> Result<String, String> {
        self.0.lock().unwrap().refresh_branch(a, base)
    }
    fn close(&mut self, pr: u64, head_sha: &str, comment: &str) -> Result<(), String> {
        self.0.lock().unwrap().close(pr, head_sha, comment)
    }
}

#[test]
fn racing_executors_merge_each_pull_request_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    for round in 0..ROUNDS / 5 {
        let root = scratch();
        // three ready changes: each merge puts the others behind, so the queue is only
        // emptied by several drains, merging and refreshing in turn
        let world = Arc::new(Mutex::new(World {
            open: vec![sim(1), sim(2), sim(3)],
            ..Default::default()
        }));
        let holding = Arc::new(AtomicBool::new(false));
        let start = Arc::new(Barrier::new(RACERS));
        let racers: Vec<_> = (0..RACERS)
            .map(|_| {
                let (root, world, holding, start) =
                    (root.clone(), world.clone(), holding.clone(), start.clone());
                std::thread::spawn(move || {
                    let mut merged = Vec::new();
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
                    start.wait();
                    while !world.lock().unwrap().open.is_empty() {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "the queue never emptied"
                        );
                        let Ok(lease) = drain::IntegrationLease::acquire(&root, "master") else {
                            std::thread::yield_now();
                            continue;
                        };
                        assert!(
                            !holding.swap(true, Ordering::SeqCst),
                            "two executors held the lease at once"
                        );
                        let report =
                            drain::drain(&root, &mut Shared(world.clone()), 3, false, true)
                                .unwrap();
                        merged.extend(report.merged);
                        holding.store(false, Ordering::SeqCst);
                        drop(lease);
                    }
                    merged
                })
            })
            .collect();
        let mut merged: Vec<u64> = racers
            .into_iter()
            .flat_map(|r| r.join().expect("a racer panicked"))
            .collect();
        merged.sort_unstable();
        assert_eq!(
            merged,
            [1, 2, 3],
            "round {round}: what the executors merged"
        );
        let w = world.lock().unwrap();
        assert_eq!(
            w.merge_calls, 3,
            "round {round}: a merge reached the forge twice"
        );
        // each change merged after its own refresh, once: #1 needed none
        assert_eq!(
            w.refresh_calls, 2,
            "round {round}: a branch was refreshed twice"
        );
        let attempted = trail_actions(&root)
            .iter()
            .filter(|a| *a == "merge_attempted")
            .count();
        assert_eq!(attempted, 3, "round {round}: the trail");
        drop(w);
        let _ = std::fs::remove_dir_all(&root);
    }
}

// ---------------------------------------------------------------- required-check authority

fn run(name: &str, state: CheckRunState, completed_at: &str) -> CheckObservation {
    CheckObservation {
        name: name.into(),
        state,
        completed_at: completed_at.into(),
        ..Default::default()
    }
}

/// The newest report of a context is its verdict: a failure that a re-run fixed has passed,
/// and a pass that a re-run broke has failed.
#[test]
fn a_failed_run_followed_by_a_passing_rerun_has_passed() {
    use crate::integration::classify::required_checks;
    let req: Vec<super::RequiredCheck> = vec!["ci".into()];
    let fixed = vec![
        run("ci", CheckRunState::Failed, "2026-09-01T00:01:00Z"),
        run("ci", CheckRunState::Passed, "2026-09-01T00:09:00Z"),
    ];
    assert_eq!(
        required_checks(&fixed, Some(&req), &[]),
        RequiredCheckState::Passed
    );
    let broken = vec![
        run("ci", CheckRunState::Passed, "2026-09-01T00:01:00Z"),
        run("ci", CheckRunState::Failed, "2026-09-01T00:09:00Z"),
    ];
    assert_eq!(
        required_checks(&broken, Some(&req), &[]),
        RequiredCheckState::Failed
    );
    // a re-run still running makes the check pending, whatever finished before it
    let rerunning = vec![
        run("ci", CheckRunState::Passed, "2026-09-01T00:01:00Z"),
        run("ci", CheckRunState::Pending, ""),
    ];
    assert_eq!(
        required_checks(&rerunning, Some(&req), &[]),
        RequiredCheckState::Pending
    );
}

/// A check the base binds to an app is that app's check run: a status context of the same
/// name, or another app's run, is not it, so the check has not reported.
#[test]
fn a_status_context_from_the_wrong_writer_is_missing() {
    use crate::integration::classify::required_checks;
    use crate::integration::CheckKind;
    let bound = vec![super::RequiredCheck {
        context: "ci".into(),
        app_id: Some(15368),
    }];
    let status = vec![CheckObservation {
        name: "ci".into(),
        state: CheckRunState::Passed,
        kind: CheckKind::StatusContext,
        ..Default::default()
    }];
    assert_eq!(
        required_checks(&status, Some(&bound), &[]),
        RequiredCheckState::Missing
    );
    let other_app = vec![CheckObservation {
        name: "ci".into(),
        state: CheckRunState::Passed,
        app_id: Some(1),
        ..Default::default()
    }];
    assert_eq!(
        required_checks(&other_app, Some(&bound), &[]),
        RequiredCheckState::Missing
    );
    let its_app = vec![CheckObservation {
        name: "ci".into(),
        state: CheckRunState::Passed,
        app_id: Some(15368),
        ..Default::default()
    }];
    assert_eq!(
        required_checks(&its_app, Some(&bound), &[]),
        RequiredCheckState::Passed
    );
    // what `gh pr list` alone reports: a check run of the name that names no app. Nobody
    // read who wrote it, so it is not the bound app's run, and it has not passed.
    let unread = vec![CheckObservation {
        name: "ci".into(),
        state: CheckRunState::Passed,
        ..Default::default()
    }];
    assert_eq!(
        required_checks(&unread, Some(&bound), &[]),
        RequiredCheckState::Unknown
    );
    // unbound, a status context of the name is the check
    let unbound: Vec<super::RequiredCheck> = vec!["ci".into()];
    assert_eq!(
        required_checks(&status, Some(&unbound), &[]),
        RequiredCheckState::Passed
    );
}

/// A check run of `ci` written by `app` (`None`: the writer was not read), in `state`,
/// completed at `at`.
fn run_by(app: Option<u64>, state: CheckRunState, at: &str) -> CheckObservation {
    CheckObservation {
        app_id: app,
        ..run("ci", state, at)
    }
}

fn bound_to(context: &str, app: u64) -> super::RequiredCheck {
    super::RequiredCheck {
        context: context.into(),
        app_id: Some(app),
    }
}

/// Another app's check run of a bound context neither passes it, nor fails it, nor holds it
/// pending, whenever it reported: only the bound app's runs are read.
#[test]
fn a_foreign_run_never_stands_for_a_bound_check() {
    use crate::integration::classify::required_checks;
    let bound = vec![bound_to("ci", 15368)];
    let (t1, t2) = ("2026-09-01T00:01:00Z", "2026-09-01T00:09:00Z");
    let verdict = |checks: &[CheckObservation]| required_checks(checks, Some(&bound), &[]);
    // the override: the bound app failed, and a newer run of another app passed
    assert_eq!(
        verdict(&[
            run_by(Some(15368), CheckRunState::Failed, t1),
            run_by(Some(1), CheckRunState::Passed, t2),
        ]),
        RequiredCheckState::Failed
    );
    assert_eq!(
        verdict(&[
            run_by(Some(15368), CheckRunState::Passed, t1),
            run_by(Some(1), CheckRunState::Pending, ""),
        ]),
        RequiredCheckState::Passed,
        "a foreign run cannot hold it pending"
    );
    assert_eq!(
        verdict(&[
            run_by(Some(15368), CheckRunState::Passed, t1),
            run_by(Some(1), CheckRunState::Failed, t2),
        ]),
        RequiredCheckState::Passed,
        "a foreign run cannot fail it"
    );
    assert_eq!(
        verdict(&[run_by(Some(1), CheckRunState::Passed, t1)]),
        RequiredCheckState::Missing,
        "the bound app has not reported"
    );
}

/// A check run of a bound context whose writer was not read is never the bound app's run and
/// never nobody's: the check is unknown, unless the bound app's own verdict is a failure.
#[test]
fn an_unattributed_run_makes_a_bound_check_unknown() {
    use crate::integration::classify::{required_check_states, required_checks};
    let bound = vec![bound_to("ci", 15368)];
    let (t1, t2) = ("2026-09-01T00:01:00Z", "2026-09-01T00:09:00Z");
    let verdict = |checks: &[CheckObservation]| required_checks(checks, Some(&bound), &[]);
    assert_eq!(
        verdict(&[
            run_by(Some(15368), CheckRunState::Passed, t1),
            run_by(None, CheckRunState::Passed, t2),
        ]),
        RequiredCheckState::Unknown
    );
    assert_eq!(
        verdict(&[
            run_by(Some(15368), CheckRunState::Failed, t1),
            run_by(None, CheckRunState::Passed, t2),
        ]),
        RequiredCheckState::Failed,
        "the bound app's failure stands whatever else is unread"
    );
    assert_eq!(
        verdict(&[run_by(None, CheckRunState::Pending, "")]),
        RequiredCheckState::Unknown,
        "an unread writer's run does not make the check wait"
    );
    // a status context of the name is not a check run: it is not it, and not an unread writer
    let status = CheckObservation {
        kind: crate::integration::CheckKind::StatusContext,
        ..run_by(None, CheckRunState::Passed, t2)
    };
    assert_eq!(
        verdict(&[run_by(Some(15368), CheckRunState::Passed, t1), status]),
        RequiredCheckState::Passed
    );
    // unbound, any report of the name is the check, as before
    let unbound: Vec<super::RequiredCheck> = vec!["ci".into()];
    assert_eq!(
        required_checks(
            &[run_by(None, CheckRunState::Passed, t1)],
            Some(&unbound),
            &[]
        ),
        RequiredCheckState::Passed
    );
    // each requirement is answered on its own
    let mixed = vec![bound_to("ci", 15368), "lint".into()];
    let checks = vec![
        run_by(None, CheckRunState::Passed, t1),
        run("lint", CheckRunState::Passed, t1),
    ];
    assert_eq!(
        required_check_states(&checks, &mixed, &[]),
        vec![
            ("ci".to_string(), RequiredCheckState::Unknown),
            ("lint".to_string(), RequiredCheckState::Passed),
        ]
    );
}

/// A context two sources bind to two apps is required of both: one app's pass does not
/// stand for the other's failure.
#[test]
fn a_context_bound_to_two_apps_needs_both() {
    use crate::integration::classify::{required_check_states, required_checks};
    let both = vec![bound_to("ci", 7), bound_to("ci", 15368)];
    let at = "2026-09-01T00:01:00Z";
    let split = vec![
        run_by(Some(7), CheckRunState::Passed, at),
        run_by(Some(15368), CheckRunState::Failed, at),
    ];
    assert_eq!(
        required_check_states(&split, &both, &[]),
        vec![
            ("ci".to_string(), RequiredCheckState::Passed),
            ("ci".to_string(), RequiredCheckState::Failed),
        ]
    );
    assert_eq!(
        required_checks(&split, Some(&both), &[]),
        RequiredCheckState::Failed
    );
    let one = vec![run_by(Some(7), CheckRunState::Passed, at)];
    assert_eq!(
        required_checks(&one, Some(&both), &[]),
        RequiredCheckState::Missing
    );
    let passed = vec![
        run_by(Some(7), CheckRunState::Passed, at),
        run_by(Some(15368), CheckRunState::Passed, at),
    ];
    assert_eq!(
        required_checks(&passed, Some(&both), &[]),
        RequiredCheckState::Passed
    );
}

/// The whole verdict on a pull request whose bound check was reported by a writer nobody
/// read: unknown, with its own reason, never waiting and never next; the queue says so and
/// why. The two older meanings of an unknown requirement still answer as they did.
#[test]
fn a_bound_check_with_an_unread_writer_is_unknown_not_waiting() {
    use crate::integration::classify::{unread_writers, UNREAD_WRITER_REMEDY};
    use crate::integration::{EvidenceKind, IntegrationGate};
    let w = World {
        open: vec![sim(1), sim(2)],
        ..Default::default()
    };
    let mut obs = w.observation();
    obs.required_checks = Some(vec![bound_to("ci", 15368), "lint".into()]);
    // #1 as `gh pr list` alone reports it; #2 read with its writers
    obs.pull_requests[0].checks = vec![
        run_by(None, CheckRunState::Passed, ""),
        run("lint", CheckRunState::Passed, ""),
    ];
    obs.pull_requests[1].checks = vec![
        run_by(Some(15368), CheckRunState::Passed, ""),
        run("lint", CheckRunState::Passed, ""),
    ];
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Unknown);
    assert_eq!(a.required_checks, RequiredCheckState::Unknown);
    assert_eq!(
        a.reasons.iter().map(|r| r.to_string()).collect::<Vec<_>>(),
        ["required_checks:unknown"]
    );
    assert_eq!(a.next_action.as_deref(), Some(UNREAD_WRITER_REMEDY));
    assert!(
        a.gates.iter().map(|g| g.gate).eq(IntegrationGate::ALL),
        "every gate answered, in policy order"
    );
    let failed: Vec<IntegrationGate> = a
        .gates
        .iter()
        .filter(|g| !g.passed)
        .map(|g| g.gate)
        .collect();
    assert_eq!(failed, [IntegrationGate::RequiredChecks]);
    let lines: Vec<(&str, &str)> = a
        .evidence
        .iter()
        .filter(|e| e.kind == EvidenceKind::RequiredCheck)
        .map(|e| (e.detail.as_str(), e.status.as_str()))
        .collect();
    assert_eq!(lines, [("ci (app 15368)", "unknown"), ("lint", "passed")]);
    // the one read with its writers is ready, and it alone is next
    assert_eq!(disposition(&q, 2), PullRequestDisposition::Ready);
    assert_eq!(q.next_merge, Some(2));
    assert_eq!(q.diagnostics.len(), 1, "{:?}", q.diagnostics);
    let said = &q.diagnostics[0];
    assert!(
        said.starts_with("1 pull request(s) carry a check run of ci (app 15368)"),
        "{said}"
    );
    assert!(
        said.contains("(#1)") && said.contains("majordomus prs refresh"),
        "{said}"
    );
    // the remedy names the bound the forge adapter reads to
    let bound = (super::forge::CONTEXT_PAGES * 100).to_string();
    assert!(
        UNREAD_WRITER_REMEDY.contains(&bound),
        "{UNREAD_WRITER_REMEDY}"
    );

    // alone, it leaves nothing next
    obs.pull_requests.truncate(1);
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    assert_eq!(q.next_merge, None);
    assert_eq!(q.diagnostics.len(), 1, "{:?}", q.diagnostics);

    // every writer read, or nothing bound: the queue has nothing to say about writers
    let unbound: Vec<super::RequiredCheck> = vec!["ci".into(), "lint".into()];
    assert!(unread_writers(&obs.pull_requests, &unbound).is_empty());
    obs.pull_requests[0].checks[0].app_id = Some(99);
    assert!(unread_writers(&obs.pull_requests, &[bound_to("ci", 15368)]).is_empty());
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    assert_eq!(disposition(&q, 1), PullRequestDisposition::WaitingForChecks);
    assert_eq!(
        q.get(1).unwrap().required_checks,
        RequiredCheckState::Missing
    );
    assert!(q.diagnostics.is_empty(), "{:?}", q.diagnostics);

    // the two older meanings of unknown keep their own reasons
    obs.required_checks = Some(Vec::new());
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    let reasons = |q: &IntegrationQueue| -> Vec<String> {
        let a = q.get(1).unwrap();
        assert_eq!(a.disposition, PullRequestDisposition::Unknown);
        a.reasons.iter().map(|r| r.to_string()).collect()
    };
    assert_eq!(reasons(&q), ["no_required_checks"]);
    obs.required_checks = None;
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    assert_eq!(reasons(&q), ["required_checks_unread"]);
}

/// A skipped required check has not passed, unless the policy permits that context's skip.
#[test]
fn a_skip_is_missing_unless_permitted() {
    use crate::integration::classify::required_checks;
    let req: Vec<super::RequiredCheck> = vec!["ci".into(), "lint".into()];
    let checks = vec![
        run("ci", CheckRunState::Passed, "2026-09-01T00:01:00Z"),
        run("lint", CheckRunState::Skipped, "2026-09-01T00:02:00Z"),
    ];
    assert_eq!(
        required_checks(&checks, Some(&req), &[]),
        RequiredCheckState::Missing
    );
    assert_eq!(
        required_checks(&checks, Some(&req), &["ci".into()]),
        RequiredCheckState::Missing,
        "permitting another context's skip permits nothing here"
    );
    assert_eq!(
        required_checks(&checks, Some(&req), &["lint".into()]),
        RequiredCheckState::Skipped
    );
}

/// Owner decision D5: a base that requires no check proves nothing about a head, so nothing
/// is ready onto it. The queue says why, the evidence says none were required, and the
/// diagnostics say how to fix it.
#[test]
fn an_empty_required_set_is_unknown_and_says_so() {
    let w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let mut obs = w.observation();
    obs.required_checks = Some(Vec::new());
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Unknown);
    assert_eq!(a.reasons, ["no_required_checks"]);
    assert!(a
        .evidence
        .iter()
        .any(|e| e.kind == "required_checks" && e.status == "none_required"));
    assert!(q.next_merge.is_none());
    assert!(
        q.diagnostics
            .iter()
            .any(|d| d.contains("requires no check")),
        "{:?}",
        q.diagnostics
    );
}

// ---------------------------------------------------------------- review authority

fn reviewed(
    decision: &str,
    reviews: &[(&str, &str)],
    policy: Option<super::ReviewPolicy>,
) -> PullRequestReview {
    use crate::integration::classify::review_state;
    let mut pr = observe_pr(&sim(1));
    pr.review_decision = decision.into();
    pr.latest_reviews = reviews
        .iter()
        .map(|(state, commit)| super::ReviewObservation {
            author: "r".into(),
            state: (*state).into(),
            commit: (*commit).into(),
        })
        .collect();
    review_state(&pr, policy.as_ref())
}

/// An approval is of the commit it was given on. With the head moved since, the approval is
/// stale, not approved, even when the forge still says APPROVED because dismissal is off.
#[test]
fn a_stale_approval_is_not_approved() {
    let one = Some(super::ReviewPolicy {
        approvals: 1,
        ..Default::default()
    });
    let head = sim(1).head;
    assert_eq!(
        reviewed("APPROVED", &[("APPROVED", &head)], one),
        PullRequestReview::Approved
    );
    assert_eq!(
        reviewed("APPROVED", &[("APPROVED", "old")], one),
        PullRequestReview::Stale
    );
    assert_eq!(
        reviewed("APPROVED", &[], one),
        PullRequestReview::Pending,
        "an approval nobody gave"
    );
    assert_eq!(
        reviewed("", &[("COMMENTED", &head)], one),
        PullRequestReview::Pending
    );
    let two = Some(super::ReviewPolicy {
        approvals: 2,
        ..Default::default()
    });
    assert_eq!(
        reviewed("APPROVED", &[("APPROVED", &head), ("APPROVED", "old")], two),
        PullRequestReview::Stale,
        "one of the two approvals is of another commit"
    );
}

/// The forge's REVIEW_REQUIRED with the approvals counted on the head is a code owner's
/// review still owed, when the base requires one.
#[test]
fn approvals_without_a_code_owners_are_code_owners_pending() {
    let owners = Some(super::ReviewPolicy {
        approvals: 1,
        code_owners: true,
        ..Default::default()
    });
    let head = sim(1).head;
    assert_eq!(
        reviewed("REVIEW_REQUIRED", &[("APPROVED", &head)], owners),
        PullRequestReview::CodeOwnersPending
    );
    assert_eq!(
        reviewed("APPROVED", &[("APPROVED", &head)], owners),
        PullRequestReview::Approved
    );
}

/// Unread policy is never a pass: the forge's APPROVED is believed only where an approval
/// on the head stands behind it, and a request for changes holds whatever the policy.
#[test]
fn an_unread_review_policy_passes_nothing_unsupported() {
    let head = sim(1).head;
    assert_eq!(
        reviewed("APPROVED", &[("APPROVED", &head)], None),
        PullRequestReview::Approved
    );
    assert_eq!(
        reviewed("APPROVED", &[("APPROVED", "old")], None),
        PullRequestReview::Stale
    );
    assert_eq!(reviewed("APPROVED", &[], None), PullRequestReview::Unknown);
    assert_eq!(reviewed("", &[], None), PullRequestReview::Unknown);
    assert_eq!(
        reviewed("CHANGES_REQUESTED", &[("APPROVED", &head)], None),
        PullRequestReview::ChangesRequested
    );
    // and each of the new states keeps the pull request waiting for review
    let w = World {
        open: vec![sim(1)],
        reviews_required: Some(true),
        ..Default::default()
    };
    let mut obs = w.observation();
    obs.pull_requests[0].review_decision = "APPROVED".into();
    obs.pull_requests[0].latest_reviews = vec![super::ReviewObservation {
        author: "r".into(),
        state: "APPROVED".into(),
        commit: "old".into(),
    }];
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    let a = q.get(1).unwrap();
    assert_eq!(
        (a.review, a.disposition),
        (
            PullRequestReview::Stale,
            PullRequestDisposition::WaitingForReview
        )
    );
}

// ---------------------------------------------------------------- a lease renewed or lost (WP5)

#[test]
fn a_lease_taken_over_cannot_be_renewed_and_its_successor_survives() {
    let root = scratch();
    let first = drain::IntegrationLease::acquire(&root, "master").unwrap();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    // something removed the file: the first executor's lock now guards a file nobody finds
    std::fs::remove_file(&path).unwrap();
    let second = drain::IntegrationLease::acquire(&root, "master").expect("a new file, free");
    let err = first.renew().unwrap_err();
    assert!(err.contains("lease was lost"), "{err}");
    second.renew().expect("the successor renews");
    // giving the lost lease back leaves the successor's record where it is
    drop(first);
    let held = drain::IntegrationLease::read(&root, "master")
        .unwrap()
        .unwrap();
    assert_eq!(held.holder.unwrap().pid, std::process::id());
    assert!(path.is_file());
    drop(second);
    assert!(!path.exists());
}

#[test]
fn a_record_naming_another_holder_cannot_be_renewed() {
    let root = scratch();
    let lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    std::fs::write(&path, "another holder").unwrap();
    let err = lease.renew().unwrap_err();
    assert!(err.contains("names another holder"), "{err}");
    // and the record is not overwritten by the refused renewal
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "another holder");
}

#[test]
fn a_kept_alive_lease_stays_fresh_and_stops_when_dropped() {
    let root = scratch();
    let lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    age_lease(&path, drain::LEASE_STALE_AFTER * 2);
    assert!(
        drain::IntegrationLease::read(&root, "master")
            .unwrap()
            .unwrap()
            .stale
    );
    {
        let _alive = lease.keep_alive_every(std::time::Duration::from_millis(20));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while drain::IntegrationLease::read(&root, "master")
            .unwrap()
            .unwrap()
            .stale
        {
            assert!(
                std::time::Instant::now() < deadline,
                "the keep-alive never renewed"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    // stopped: the record ages again and nothing writes it
    age_lease(&path, drain::LEASE_STALE_AFTER * 2);
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        drain::IntegrationLease::read(&root, "master")
            .unwrap()
            .unwrap()
            .stale
    );
}

#[test]
fn a_keep_alive_never_writes_a_lease_that_was_lost() {
    let root = scratch();
    let first = drain::IntegrationLease::acquire(&root, "master").unwrap();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    let _alive = first.keep_alive_every(std::time::Duration::from_millis(10));
    std::fs::remove_file(&path).unwrap();
    let second = drain::IntegrationLease::acquire(&root, "master").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let held = std::fs::read_to_string(&path).unwrap();
    assert!(
        held.contains(&format!("\"pid\":{}", std::process::id())),
        "{held}"
    );
    second.renew().expect("the successor's record is intact");
}

// ---------------------------------------------------------------- WP11: typed reasons, evidence and gates
//
// Every gate is answered and every failing one is reported; the first decides. The reasons
// are typed, and their wire form is the `code:payload` string they always had.

/// One of every reason the vocabulary names, with payloads that test the parser: a `:` inside
/// a free-text payload, every review state, every required-check state.
fn every_reason() -> Vec<crate::integration::ReasonCode> {
    use crate::integration::ReasonCode as R;
    // a new variant must be added below before this compiles: no wildcard
    let _covered = |r: &R| match r {
        R::StackedOn { .. }
        | R::BaseIs { .. }
        | R::Draft
        | R::Label { .. }
        | R::AutoMergeArmed
        | R::MergeCommitNotAllowed
        | R::SupersededBy { .. }
        | R::SuccessorOpen { .. }
        | R::SuccessorNotLanded { .. }
        | R::SuccessorUnread { .. }
        | R::HeadReachableFromMaster
        | R::MergeChangesNothing
        | R::PatchIdsUpstream
        | R::OnlyDerivedArtifactsDiffer
        | R::RelationUnknown { .. }
        | R::ConflictsOn { .. }
        | R::DependsOn { .. }
        | R::Review { .. }
        | R::ReviewPolicyUnread
        | R::RequiredCheckFailed
        | R::BehindMaster { .. }
        | R::ForkHead
        | R::RequiredChecks { .. }
        | R::NoRequiredChecks
        | R::RequiredChecksUnread
        | R::ContainsMaster
        | R::CarriesMaster
        | R::RequiredChecksPassed
        | R::RequiredChecksSkipped
        | R::ExecutorMergeRefused { .. }
        | R::ExecutorRefreshFailed { .. }
        | R::LabelObsolete { .. }
        | R::DependencyCycle { .. }
        | R::DependencyClosedUnmerged { .. }
        | R::DependencyUnread { .. }
        | R::DeclarationsUnread
        | R::Unrecognised(_) => (),
    };
    let mut all = vec![
        R::StackedOn { number: 601 },
        R::BaseIs {
            base: "release/1".into(),
        },
        R::BaseIs { base: "".into() },
        R::Draft,
        R::Label {
            name: "Do-Not-Merge".into(),
        },
        R::Label {
            name: "scope:ci".into(),
        },
        R::AutoMergeArmed,
        R::MergeCommitNotAllowed,
        R::SupersededBy { number: 12 },
        R::SuccessorOpen { number: 13 },
        R::SuccessorNotLanded { number: 14 },
        R::SuccessorUnread { number: 15 },
        R::HeadReachableFromMaster,
        R::MergeChangesNothing,
        R::PatchIdsUpstream,
        R::OnlyDerivedArtifactsDiffer,
        R::RelationUnknown {
            reason: "git merge-tree: exit 128: bad object".into(),
        },
        R::ConflictsOn { count: 3 },
        R::DependsOn { number: 7 },
        R::ReviewPolicyUnread,
        R::RequiredCheckFailed,
        R::BehindMaster { commits: 12 },
        R::ForkHead,
        R::NoRequiredChecks,
        R::RequiredChecksUnread,
        R::ContainsMaster,
        R::CarriesMaster,
        R::RequiredChecksPassed,
        R::RequiredChecksSkipped,
        R::ExecutorMergeRefused {
            head: "a".repeat(40),
        },
        R::ExecutorRefreshFailed {
            master: "b".repeat(40),
        },
        R::LabelObsolete {
            name: "Obsolete: superseded by the redesign".into(),
        },
        R::DependencyCycle { number: 611 },
        R::DependencyClosedUnmerged { number: 612 },
        R::DependencyUnread { number: 613 },
        R::DeclarationsUnread,
    ];
    for state in [
        PullRequestReview::NotRequired,
        PullRequestReview::Approved,
        PullRequestReview::ChangesRequested,
        PullRequestReview::Pending,
        PullRequestReview::Stale,
        PullRequestReview::CodeOwnersPending,
        PullRequestReview::Unknown,
    ] {
        all.push(R::Review { state });
    }
    for state in [
        RequiredCheckState::Passed,
        RequiredCheckState::Skipped,
        RequiredCheckState::Pending,
        RequiredCheckState::Failed,
        RequiredCheckState::Missing,
        RequiredCheckState::Unknown,
    ] {
        all.push(R::RequiredChecks { state });
    }
    all
}

#[test]
fn a_draft_that_conflicts_and_fails_its_check_says_all_three_and_stays_a_draft() {
    use crate::integration::{IntegrationGate as G, ReasonCode as R};
    let mut s = sim(1);
    s.draft = true;
    s.failing = true;
    s.conflicts_after = Some(99);
    let w = World {
        open: vec![s],
        merged: vec![99],
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Draft);
    assert_eq!(
        a.reasons,
        vec![
            R::Draft,
            R::ConflictsOn { count: 1 },
            R::RequiredCheckFailed
        ]
    );
    // the wire form is the strings the reasons always were
    assert_eq!(
        serde_json::to_value(&a.reasons).unwrap(),
        serde_json::json!(["draft", "conflicts_on:1", "required_check_failed"])
    );
    // every gate answered, in policy order
    let order: Vec<G> = a.gates.iter().map(|g| g.gate).collect();
    assert_eq!(order, G::ALL);
    let failed: Vec<G> = a
        .gates
        .iter()
        .filter(|g| !g.passed)
        .map(|g| g.gate)
        .collect();
    assert_eq!(
        failed,
        [
            G::Draft,
            G::RelationToMaster,
            G::NoFailingCheck,
            G::Freshness,
            G::RequiredChecks
        ]
    );
    // draft and base evidence are always there, and the conflict is git's
    let kinds: Vec<&str> = a.evidence.iter().map(|e| e.kind.as_str()).collect();
    for kind in [
        "base",
        "draft",
        "required_checks",
        "required_check",
        "relation_to_master",
    ] {
        assert!(kinds.contains(&kind), "{kind} missing from {kinds:?}");
    }
    let relation = a
        .evidence
        .iter()
        .find(|e| e.kind == "relation_to_master")
        .unwrap();
    assert_eq!(
        relation.source,
        Some(crate::integration::EvidenceSource::Git {
            master_sha: "m0".into(),
            head_sha: a.evaluated_against.head_sha.clone(),
        })
    );
}

#[test]
fn a_ready_pull_request_passed_every_gate_and_says_why_as_it_always_did() {
    let w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Ready);
    assert!(a.gates.iter().all(|g| g.passed), "{:?}", a.gates);
    assert_eq!(a.reasons, ["contains_master", "required_checks_passed"]);
    // the observation's moment is named, and a draft/base answer is evidence too
    assert_eq!(a.evaluated_against.observed_at, "t0");
    assert!(a
        .evidence
        .iter()
        .any(|e| e.kind == "draft" && e.status == "ready_for_review"));
    assert!(a
        .evidence
        .iter()
        .any(|e| e.kind == "base" && e.status == "integration_base"));
    for e in &a.evidence {
        assert!(e.source.is_some(), "{e:?} names no source");
    }
}

#[test]
fn each_required_context_is_its_own_evidence_and_a_blocking_label_always_is() {
    let mut s = sim(1);
    s.labels = vec!["wip".into()];
    let w = World {
        open: vec![s],
        ..Default::default()
    };
    let mut obs = w.observation();
    obs.required_checks = Some(vec!["ci".into(), "lint".into()]);
    let q = build_queue(&obs, "m0", |p| w.relation(p.number));
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Blocked);
    // `ci` passed, `lint` never reported: the label decides, the missing check is said too
    assert_eq!(
        a.reasons,
        ["label:wip", "required_checks:missing"],
        "{:?}",
        a.reasons
    );
    let per_context: Vec<(String, String)> = a
        .evidence
        .iter()
        .filter(|e| e.kind == "required_check")
        .map(|e| (e.detail.clone(), e.status.clone()))
        .collect();
    assert_eq!(
        per_context,
        [
            ("ci".to_string(), "passed".to_string()),
            ("lint".to_string(), "missing".to_string())
        ]
    );
    assert!(a
        .evidence
        .iter()
        .any(|e| e.kind == "label" && e.detail == "wip"));
    assert_eq!(
        a.evidence
            .iter()
            .find(|e| e.kind == "label")
            .and_then(|e| e.source.clone()),
        Some(crate::integration::EvidenceSource::Forge {
            observed_at: "t0".into()
        })
    );
}

#[test]
fn every_reason_code_survives_its_wire_string() {
    use crate::integration::ReasonCode;
    for r in every_reason() {
        let wire = r.to_string();
        assert_eq!(wire.parse::<ReasonCode>().as_ref(), Ok(&r), "{wire}");
        assert!(
            wire.starts_with(r.code()),
            "{wire} does not open with {}",
            r.code()
        );
        // and through serde, as a plain JSON string
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json, serde_json::Value::String(wire.clone()));
        assert_eq!(serde_json::from_value::<ReasonCode>(json).unwrap(), r);
    }
    // a code outside the vocabulary is refused by the parser and carried verbatim by serde
    assert!("ready".parse::<ReasonCode>().is_err());
    assert!("draft:yes".parse::<ReasonCode>().is_err());
    assert!(
        "depends_on:7".parse::<ReasonCode>().is_err(),
        "the number is #N"
    );
    assert!("review:sleepy".parse::<ReasonCode>().is_err());
    let odd: ReasonCode = serde_json::from_value(serde_json::json!("ready")).unwrap();
    assert_eq!(odd, ReasonCode::Unrecognised("ready".into()));
    assert_eq!(
        serde_json::to_value(&odd).unwrap(),
        serde_json::json!("ready")
    );
}

/// A trail line exactly as the executor wrote it while reasons and evidence kinds were
/// strings and evidence named no source.
const OLD_TRAIL_LINE: &str = r#"{"at":"2026-10-02T09:00:00Z","actor":"t (pid 1 on h)","action":"merge_attempted","pr":4,"master_before":"m1","head_sha":"h4","master_after":null,"reasons":["contains_master","required_checks_passed","behind_master:3","fork_head","review:changes_requested","ready"],"detail":"d","evidence":[{"kind":"required_checks","status":"passed","detail":"ci: passed"},{"kind":"relation_to_master","status":"up_to_date","detail":"contains master; 1 authored path(s)"},{"kind":"label","status":"blocking","detail":"wip"}]}"#;

#[test]
fn old_trail_lines_still_parse_and_write_back_unchanged() {
    use crate::integration::{EvidenceKind, ReasonCode as R};
    let e: drain::IntegrationEvent = serde_json::from_str(OLD_TRAIL_LINE).unwrap();
    assert_eq!(
        e.reasons,
        vec![
            R::ContainsMaster,
            R::RequiredChecksPassed,
            R::BehindMaster { commits: 3 },
            R::ForkHead,
            R::Review {
                state: PullRequestReview::ChangesRequested
            },
            R::Unrecognised("ready".into()),
        ]
    );
    let kinds: Vec<EvidenceKind> = e.evidence.iter().map(|x| x.kind).collect();
    assert_eq!(
        kinds,
        [
            EvidenceKind::RequiredChecks,
            EvidenceKind::RelationToMaster,
            EvidenceKind::Label
        ]
    );
    assert!(e.evidence.iter().all(|x| x.source.is_none()));
    // written back, the line says exactly what it said
    let original: serde_json::Value = serde_json::from_str(OLD_TRAIL_LINE).unwrap();
    assert_eq!(serde_json::to_value(&e).unwrap(), original);
    // and the line the older suite keeps for typed actions reads too
    let older: drain::IntegrationEvent = serde_json::from_str(&old_line("selected")).unwrap();
    assert_eq!(older.reasons, ["ready"]);
}

#[test]
fn the_same_revisions_observed_later_are_the_same_decision() {
    let at = |observed_at: &str| crate::integration::EvaluatedAgainst {
        master_sha: "m1".into(),
        head_sha: "h1".into(),
        observed_at: observed_at.into(),
    };
    assert_eq!(at("t1"), at("t2"));
    let moved = crate::integration::EvaluatedAgainst {
        head_sha: "h2".into(),
        ..at("t1")
    };
    assert_ne!(at("t1"), moved);
}

proptest! {
    /// Over any world: every gate is answered in policy order, the disposition is `ready`
    /// exactly when every gate passed, every failing gate but a derivative one contributes a
    /// reason, the first reason is the decisive gate's, and nothing the classifier says falls
    /// outside the vocabulary.
    #[test]
    fn the_first_failing_gate_decides_and_every_reason_is_in_the_vocabulary(w in arb_world()) {
        use crate::integration::{IntegrationGate as G, ReasonCode};
        let q = w.queue();
        for a in &q.assessments {
            let order: Vec<G> = a.gates.iter().map(|g| g.gate).collect();
            prop_assert_eq!(&order[..], &G::ALL[..]);
            let ready = a.disposition == PullRequestDisposition::Ready;
            prop_assert_eq!(ready, a.gates.iter().all(|g| g.passed), "#{}", a.number);
            prop_assert!(!a.reasons.is_empty());
            for r in &a.reasons {
                prop_assert!(!matches!(r, ReasonCode::Unrecognised(_)), "{}", r);
                let parsed = r.to_string().parse::<ReasonCode>();
                prop_assert_eq!(parsed.as_ref(), Ok(r));
            }
            if let Some(first) = a.gates.iter().find(|g| !g.passed) {
                // the first reason belongs to the first failing gate
                let code = a.reasons[0].code();
                let of_gate: &[&str] = match first.gate {
                    G::Base => &["stacked_on", "base_is"],
                    G::Draft => &["draft"],
                    G::Label => &["label"],
                    G::AutoMerge => &["auto_merge_armed"],
                    G::Supersession => &[
                        "superseded_by",
                        "successor_open",
                        "successor_not_landed",
                        "successor_unread",
                    ],
                    G::MergeMethod => &["merge_commit_not_allowed"],
                    G::RelationToMaster => &[
                        "head_reachable_from_master",
                        "merge_changes_nothing",
                        "patch_ids_upstream",
                        "only_derived_artifacts_differ",
                        "relation_unknown",
                        "conflicts_on",
                    ],
                    G::Dependency => &[
                        "depends_on",
                        "dependency_cycle",
                        "dependency_closed_unmerged",
                        "dependency_unread",
                    ],
                    G::Review => &["review", "review_policy_unread"],
                    G::NoFailingCheck => &["required_check_failed"],
                    G::Freshness => &["behind_master"],
                    G::RequiredChecks => &[
                        "required_checks",
                        "no_required_checks",
                        "required_checks_unread",
                    ],
                };
                prop_assert!(of_gate.contains(&code), "#{}: {} after {:?}", a.number, code, first.gate);
            }
        }
    }
}

// ---------------------------------------------------------------- WP12: Unsafe, merge method, label policy
//
// Auto-merge armed is `unsafe`: the forge would merge it on its own, outside the executor. A
// repository that allows no merge commit holds every pull request, and nothing is ever merged
// otherwise. The labels that hold are one table.

#[test]
fn auto_merge_armed_is_unsafe_and_every_later_failing_gate_is_still_said() {
    use crate::integration::{IntegrationGate as G, IntegrationLane, ReasonCode as R};
    // #1: armed, its check failed, behind master; #2: armed and only behind, which would
    // otherwise be refreshed; #3: armed and held by a label, which comes first
    let mut failing = sim(1);
    failing.auto_merge = true;
    failing.failing = true;
    let mut behind = sim(2);
    behind.auto_merge = true;
    let mut labelled = sim(3);
    labelled.auto_merge = true;
    labelled.labels = vec!["wip".into()];
    labelled.contains = 1;
    let w = World {
        open: vec![failing, behind, labelled],
        master: 1,
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Unsafe);
    assert_eq!(a.lane, IntegrationLane::Held);
    assert_eq!(
        a.reasons,
        vec![
            R::AutoMergeArmed,
            R::RequiredCheckFailed,
            R::BehindMaster { commits: 1 }
        ]
    );
    assert_eq!(
        serde_json::to_value(&a.reasons).unwrap(),
        serde_json::json!([
            "auto_merge_armed",
            "required_check_failed",
            "behind_master:1"
        ])
    );
    let failed: Vec<G> = a
        .gates
        .iter()
        .filter(|g| !g.passed)
        .map(|g| g.gate)
        .collect();
    assert_eq!(
        failed,
        [
            G::AutoMerge,
            G::NoFailingCheck,
            G::Freshness,
            G::RequiredChecks
        ]
    );
    let armed = a
        .evidence
        .iter()
        .find(|e| e.kind == crate::integration::EvidenceKind::AutoMerge)
        .expect("auto-merge evidence");
    assert_eq!(armed.status, "armed");
    assert_eq!(armed.kind.as_str(), "auto_merge");
    assert_eq!(
        armed.source,
        Some(crate::integration::EvidenceSource::Forge {
            observed_at: "t0".into()
        })
    );
    assert!(a
        .next_action
        .as_deref()
        .is_some_and(|n| n.contains("--disable-auto")));

    // an armed one is never refreshed: a refreshed head would be merged by the forge
    assert_eq!(disposition(&q, 2), PullRequestDisposition::Unsafe);
    assert!(q.next_refresh.is_empty(), "{:?}", q.next_refresh);
    assert_eq!(q.next_merge, None);

    // the label gate comes first, and the armed auto-merge is said after it
    let held = q.get(3).unwrap();
    assert_eq!(held.disposition, PullRequestDisposition::Blocked);
    assert_eq!(held.reasons, ["label:wip", "auto_merge_armed"]);

    // the queue names every armed pull request
    let diagnostic = q
        .diagnostics
        .iter()
        .find(|d| d.contains("auto-merge is armed"))
        .expect("an auto-merge diagnostic");
    assert!(diagnostic.contains("#1 #2 #3"), "{diagnostic}");
    // a pull request without it carries no auto-merge evidence
    let quiet = World {
        open: vec![sim(4)],
        ..Default::default()
    }
    .queue();
    assert!(quiet
        .get(4)
        .unwrap()
        .evidence
        .iter()
        .all(|e| e.kind != "auto_merge"));
    assert!(quiet.diagnostics.iter().all(|d| !d.contains("auto-merge")));
}

#[test]
fn auto_merge_armed_between_the_decision_and_the_act_merges_nothing() {
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let (out, actions) = step_with_second_observation(&mut w, Meanwhile::AutoMergeArmed(1), false);
    assert!(stale_with(&out, 1, "it is unsafe now"), "{out:?}");
    assert!(w.merged.is_empty());
    assert_eq!(w.merge_calls, 0);
    assert!(
        !actions.iter().any(|a| a == "merge_attempted"),
        "{actions:?}"
    );
}

#[test]
fn a_repository_without_merge_commits_holds_every_candidate_and_merges_nothing() {
    use crate::integration::ReasonCode as R;
    for allowed in [
        vec!["squash"],
        vec!["squash", "rebase"],
        vec!["rebase"],
        vec![],
    ] {
        let mut behind = sim(2);
        behind.contains = 0;
        let mut ready = sim(1);
        ready.contains = 1;
        let mut w = World {
            open: vec![ready, behind],
            master: 1,
            merge_methods: allowed.clone(),
            ..Default::default()
        };
        let q = w.queue();
        assert_eq!(q.policy.merge_method, None, "{allowed:?}");
        for a in &q.assessments {
            assert_eq!(
                a.disposition,
                PullRequestDisposition::Blocked,
                "#{} with {allowed:?}",
                a.number
            );
            assert_eq!(a.reasons[0], R::MergeCommitNotAllowed, "{:?}", a.reasons);
            let settings = a
                .evidence
                .iter()
                .find(|e| e.kind == crate::integration::EvidenceKind::RepositorySettings)
                .expect("settings evidence");
            assert_eq!(settings.kind.as_str(), "repository_settings");
            assert_eq!(settings.status, "merge_commit_not_allowed");
            let gate = a
                .gates
                .iter()
                .find(|g| g.gate == crate::integration::IntegrationGate::MergeMethod)
                .unwrap();
            assert!(!gate.passed);
        }
        // #2 is still said to be behind, after the decisive reason
        assert_eq!(
            q.get(2).unwrap().reasons,
            vec![R::MergeCommitNotAllowed, R::BehindMaster { commits: 1 }]
        );
        assert_eq!(q.next_merge, None);
        assert!(q.next_refresh.is_empty());
        assert!(
            q.diagnostics
                .iter()
                .any(|d| d.contains("allows no merge commit")),
            "{:?}",
            q.diagnostics
        );
        // and the executor neither merges nor refreshes anything
        let root = scratch();
        let out = drain::step(&root, &mut w, false, true).unwrap();
        assert!(matches!(out, DrainStepOutcome::Idle { .. }), "{out:?}");
        assert_eq!((w.merge_calls, w.refresh_calls), (0, 0), "{allowed:?}");
    }
}

#[test]
fn the_merge_method_is_a_merge_commit_and_never_another() {
    for allowed in [
        vec!["merge"],
        vec!["squash", "merge"],
        vec!["rebase", "squash", "merge"],
    ] {
        let w = World {
            open: vec![sim(1)],
            merge_methods: allowed.clone(),
            ..Default::default()
        };
        let q = w.queue();
        assert_eq!(
            q.policy.merge_method.as_deref(),
            Some("merge"),
            "{allowed:?}"
        );
        assert_eq!(disposition(&q, 1), PullRequestDisposition::Ready);
        assert!(q
            .get(1)
            .unwrap()
            .evidence
            .iter()
            .all(|e| e.kind != "repository_settings"));
    }
    // the executor asks the forge for exactly that
    let mut w = World {
        open: vec![sim(1)],
        merge_methods: vec!["squash", "merge", "rebase"],
        ..Default::default()
    };
    let root = scratch();
    let out = drain::step(&root, &mut w, false, false).unwrap();
    assert!(
        matches!(out, DrainStepOutcome::Merged { pr: 1, .. }),
        "{out:?}"
    );
    let attempted = drain::events(&root)
        .into_iter()
        .find(|e| e.action.as_str() == "merge_attempted")
        .expect("merge_attempted");
    assert_eq!(attempted.detail, "--merge");
}

#[test]
fn every_disposition_is_listed_at_runtime_unsafe_among_them() {
    use crate::integration::IntegrationLane;
    // `ALL` is a runtime value, not a test fixture: this function would compile without
    // `cfg(test)` too
    fn held() -> Vec<&'static str> {
        PullRequestDisposition::ALL
            .iter()
            .filter(|d| d.lane() == IntegrationLane::Held)
            .map(|d| d.as_str())
            .collect()
    }
    assert!(PullRequestDisposition::ALL.contains(&PullRequestDisposition::Unsafe));
    assert_eq!(
        held(),
        ["draft", "blocked", "unsafe", "other_base", "unknown"]
    );
    // every word reads back as its disposition
    for d in PullRequestDisposition::ALL {
        let back: PullRequestDisposition =
            serde_json::from_value(serde_json::Value::String(d.as_str().into())).unwrap();
        assert_eq!(back, d);
    }
}

#[test]
fn the_label_policy_is_one_table_and_every_effect_holds() {
    use crate::integration::{LabelEffect, LABEL_POLICY};
    let w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let q = w.queue();
    // the policy carries the table, as it is
    assert_eq!(q.policy.labels, LABEL_POLICY.to_vec());
    // a label holds a pull request (D11: none opts out of the executor's refresh), or marks
    // it obsolete (D3), and nothing else; exactly one label means obsolete
    assert!(LABEL_POLICY
        .iter()
        .all(|l| matches!(l.effect, LabelEffect::Hold | LabelEffect::Obsolete)));
    assert_eq!(
        LABEL_POLICY
            .iter()
            .filter(|l| l.effect == LabelEffect::Obsolete)
            .map(|l| l.name.as_ref())
            .collect::<Vec<_>>(),
        ["obsolete"]
    );
    let names: Vec<&str> = LABEL_POLICY.iter().map(|l| l.name.as_ref()).collect();
    assert_eq!(
        names,
        [
            "do-not-merge",
            "do not merge",
            "blocked",
            "hold",
            "on-hold",
            "wip",
            "manual-merge",
            "obsolete"
        ]
    );
    // every hold label of the table holds, whatever its case; a label outside it does not
    let holds: Vec<&str> = LABEL_POLICY
        .iter()
        .filter(|l| l.effect == LabelEffect::Hold)
        .map(|l| l.name.as_ref())
        .collect();
    for label in holds.iter().map(|n| n.to_ascii_uppercase()) {
        let mut s = sim(1);
        s.labels = vec![label.clone()];
        let q = World {
            open: vec![s],
            ..Default::default()
        }
        .queue();
        let a = q.get(1).unwrap();
        assert_eq!(a.disposition, PullRequestDisposition::Blocked, "{label}");
        assert_eq!(a.reasons[0].to_string(), format!("label:{label}"));
    }
    let mut s = sim(1);
    s.labels = vec!["no-refresh".into(), "documentation".into()];
    let q = World {
        open: vec![s],
        ..Default::default()
    }
    .queue();
    assert_eq!(disposition(&q, 1), PullRequestDisposition::Ready);
    // on the wire the table is a list of names with their effect
    assert_eq!(
        serde_json::to_value(&LABEL_POLICY[0]).unwrap(),
        serde_json::json!({"name": "do-not-merge", "effect": "hold"})
    );
}

#[test]
fn work_already_on_master_stays_redundant_without_merge_commits_and_is_still_closed() {
    use crate::integration::{IntegrationGate as G, ReasonCode as R};
    // #1's head is contained in master; #2's merge changes nothing
    let mut redundant = sim(2);
    redundant.redundant_after = Some(99);
    let mut w = World {
        open: vec![sim(1), redundant],
        merged: vec![99],
        merge_methods: vec!["squash"],
        ..Default::default()
    };
    let obs = w.observation();
    let q = build_queue(&obs, "m0", |p| match p.number {
        1 => RelationToMaster::Contained,
        _ => w.relation(p.number),
    });
    assert_eq!(q.policy.merge_method, None);
    for (n, why) in [(1, R::HeadReachableFromMaster), (2, R::MergeChangesNothing)] {
        let a = q.get(n).unwrap();
        assert_eq!(a.disposition, PullRequestDisposition::Redundant, "#{n}");
        // the relation decides; the setting is still said, after it
        assert_eq!(a.reasons, vec![why, R::MergeCommitNotAllowed], "#{n}");
        let failed: Vec<G> = a
            .gates
            .iter()
            .filter(|g| !g.passed)
            .map(|g| g.gate)
            .collect();
        assert_eq!(failed[..2], [G::RelationToMaster, G::MergeMethod], "#{n}");
        assert!(a.evidence.iter().any(|e| e.kind == "repository_settings"));
    }
    // and cleanup still closes redundant work: closing does not depend on the merge setting
    let root = scratch();
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    let closed: Vec<(u64, &str)> = items.iter().map(|i| (i.pr, i.action.as_str())).collect();
    assert_eq!(closed, [(2, "closed")]);
    assert_eq!((w.close_calls, w.merge_calls), (1, 0));
}

// ---------------------------------------------------------------- WP13: Redundant and Superseded{by}
//
// `redundant` is git's strong evidence that the work is on master (before 0.13 it was the word
// `superseded`). `superseded` is a declared successor that landed, named in `superseded_by`. A
// successor still open makes it wait; one that did not land leaves it to a person; weak
// evidence is never closed.

fn superseded_by_body(n: u64) -> String {
    format!("Superseded by #{n}.")
}

#[test]
fn a_successor_that_landed_supersedes_it_and_cleanup_closes_it_naming_the_successor() {
    use crate::integration::{EvidenceKind, EvidenceSource, IntegrationGate as G, ReasonCode as R};
    // #1 says #2 replaces it; #2 was merged with a merge commit, and #1 now conflicts with what
    // #2 brought — superseded, not conflicting
    let mut replaced = sim(1);
    replaced.body = superseded_by_body(2);
    replaced.conflicts_after = Some(2);
    let mut w = World {
        open: vec![replaced],
        merged: vec![2],
        gone: vec![(sim(2), Gone::Merged)],
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Superseded);
    assert_eq!(a.superseded_by, Some(2));
    assert_eq!(a.lane, crate::integration::IntegrationLane::Cleanup);
    assert_eq!(a.reasons[0], R::SupersededBy { number: 2 });
    assert_eq!(a.reasons[0].to_string(), "superseded_by:#2");
    assert!(
        a.reasons.contains(&R::ConflictsOn { count: 1 }),
        "{:?}",
        a.reasons
    );
    let first_failed = a.gates.iter().find(|g| !g.passed).unwrap().gate;
    assert_eq!(first_failed, G::Supersession);
    let s = a
        .evidence
        .iter()
        .find(|e| e.kind == EvidenceKind::Supersession)
        .expect("supersession evidence");
    assert_eq!(s.status, "landed");
    assert!(s.detail.contains("superseded by #2"), "{}", s.detail);
    // that it landed is git's: master and the successor's head
    assert_eq!(
        s.source,
        Some(EvidenceSource::Git {
            master_sha: "m0".into(),
            head_sha: "h2.0".into()
        })
    );
    // on the wire the disposition is one word, and its successor a field beside it
    let wire = serde_json::to_value(a).unwrap();
    assert_eq!(wire["disposition"], "superseded");
    assert_eq!(wire["superseded_by"], 2);

    // cleanup closes it, records the closure as `closed_superseded`, and names the successor
    let root = scratch();
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        (items[0].pr, items[0].disposition, items[0].action.as_str()),
        (1, PullRequestDisposition::Superseded, "closed")
    );
    assert_eq!((w.close_calls, w.merge_calls), (1, 0));
    let trail = drain::events(&root);
    let said: Vec<&str> = trail.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(said, ["close_attempted", "closed_superseded"]);
    for e in &trail {
        assert!(
            e.detail.starts_with("Superseded by #2, which landed."),
            "{}",
            e.detail
        );
    }
    assert!(trail[1]
        .evidence
        .iter()
        .any(|e| e.kind == EvidenceKind::Supersession));
}

#[test]
fn an_open_successor_holds_it_until_the_successor_lands_and_then_it_is_closed() {
    use crate::integration::ReasonCode as R;
    let root = scratch();
    // #1 would be ready on its own; #2, its declared successor, is open and ready too
    let mut replaced = sim(1);
    replaced.body = superseded_by_body(2);
    let mut w = World {
        open: vec![replaced, sim(2)],
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::WaitingForDependency);
    assert_eq!(a.reasons[0], R::SuccessorOpen { number: 2 });
    assert_eq!(a.superseded_by, None);
    assert!(serde_json::to_value(a)
        .unwrap()
        .get("superseded_by")
        .is_none());
    // the executor merges the successor, never the one it replaces
    assert_eq!(q.next_merge, Some(2));
    // and nothing closes it while its successor is open
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    assert!(items.iter().all(|i| i.pr != 1), "{items:?}");
    assert_eq!(w.close_calls, 0);

    let report = drain::drain(&root, &mut w, 3, false, false).unwrap();
    assert_eq!(report.merged, vec![2]);
    assert!(w.open.iter().any(|s| s.number == 1), "#1 was not merged");
    // #2 landed: #1 is superseded by it now, and cleanup closes it
    assert_eq!(w.queue().get(1).unwrap().superseded_by, Some(2));
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    let closed: Vec<(u64, &str)> = items.iter().map(|i| (i.pr, i.action.as_str())).collect();
    assert_eq!(closed, [(1, "closed")]);
    assert_eq!(w.merge_calls, 1);
}

#[test]
fn supersedes_in_the_successors_body_works_from_the_other_side() {
    use crate::integration::ReasonCode as R;
    let root = scratch();
    // only #2's body says anything: it supersedes #1
    let mut successor = sim(2);
    successor.body = "- **Supersedes:** #1".into();
    let mut w = World {
        open: vec![sim(1), successor],
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::WaitingForDependency);
    assert_eq!(a.reasons[0], R::SuccessorOpen { number: 2 });
    let s = a
        .evidence
        .iter()
        .find(|e| e.kind == "supersession")
        .unwrap();
    assert!(
        s.detail.contains("#2 says it supersedes #1"),
        "{}",
        s.detail
    );
    // the successor itself is not held by what it supersedes
    assert_eq!(disposition(&q, 2), PullRequestDisposition::Ready);

    // once #2 merged it is no longer open, and #1's cross-references still name it. It counts
    // because the scripted author is the repository's owner (`sim`'s default association)
    let report = drain::drain(&root, &mut w, 3, false, false).unwrap();
    assert_eq!(report.merged, vec![2]);
    let obs = w.observation();
    assert!(obs.resolved.contains_key(&2), "{:?}", obs.resolved);
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Superseded);
    assert_eq!(a.superseded_by, Some(2));
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    assert_eq!(items[0].action, "closed");
    assert_eq!(w.close_calls, 1);
}

#[test]
fn a_successor_closed_unmerged_releases_it_and_one_merged_elsewhere_is_a_persons() {
    use crate::integration::ReasonCode as R;
    let root = scratch();
    // #1's successor #2 was closed unmerged; #3's successor #4 was squashed (merged, but
    // neither its head nor a merge commit of it is on master); #5's successor #9 is not open
    // and the forge does not know it
    let mut one = sim(1);
    one.body = superseded_by_body(2);
    let mut three = sim(3);
    three.body = superseded_by_body(4);
    let mut five = sim(5);
    five.body = superseded_by_body(9);
    let mut w = World {
        open: vec![one, three, five],
        gone: vec![(sim(2), Gone::Closed), (sim(4), Gone::Squashed)],
        ..Default::default()
    };
    let q = w.queue();
    let supersession = |n: u64| {
        q.get(n)
            .unwrap()
            .evidence
            .iter()
            .find(|e| e.kind == "supersession")
            .unwrap()
            .clone()
    };
    // a successor closed unmerged replaces nothing: the hold is released, and #1 is its own
    let a = q.get(1).unwrap();
    assert_eq!(
        a.disposition,
        PullRequestDisposition::Ready,
        "{:?}",
        a.reasons
    );
    assert!(
        !a.reasons.contains(&R::SuccessorNotLanded { number: 2 }),
        "{:?}",
        a.reasons
    );
    assert_eq!(a.superseded_by, None);
    let s = supersession(1);
    assert_eq!(s.status, "not_landed");
    assert!(s.detail.contains("closed unmerged"), "{}", s.detail);
    // one the forge calls merged that master does not contain is a person's to decide
    let a = q.get(3).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::PossiblyRedundant);
    assert_eq!(a.reasons[0], R::SuccessorNotLanded { number: 4 });
    assert_eq!(a.superseded_by, None);
    let s = supersession(3);
    assert_eq!(s.status, "not_landed");
    assert!(s.detail.contains("is merged, but master"), "{}", s.detail);
    let a = q.get(5).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Unknown);
    assert_eq!(a.reasons[0].to_string(), "successor_unread:#9");
    // weak evidence and an unread successor: nothing is closed, the weak one is listed
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    let by: BTreeMap<u64, &str> = items.iter().map(|i| (i.pr, i.action.as_str())).collect();
    assert_eq!(by, BTreeMap::from([(3, "left_for_a_person")]));
    assert_eq!(w.close_calls, 0);
    assert!(drain::events(&root).is_empty());
}

/// The possible supersessions an assessment carries, as their details.
fn possible_supersessions(a: &PullRequestAssessment) -> Vec<&str> {
    a.evidence
        .iter()
        .filter(|e| e.kind == "supersession" && e.status == "possible_supersession")
        .map(|e| e.detail.as_str())
        .collect()
}

/// What a declaration by somebody the repository does not let declare one does to the pull
/// request it names, while the declarer is open and once it merged: nothing, but for evidence.
fn a_declaration_that_does_not_count(declarer: Sim, names: &str) {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), declarer],
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(
        a.disposition,
        PullRequestDisposition::Ready,
        "{:?}",
        a.reasons
    );
    assert!(
        !a.reasons.iter().any(|r| r.code().starts_with("successor_")),
        "it is not held: {:?}",
        a.reasons
    );
    let said = possible_supersessions(a);
    assert_eq!(said.len(), 1, "{:?}", a.evidence);
    assert!(said[0].contains(names), "{}", said[0]);
    assert!(
        q.next_merge.is_some(),
        "the queue is not held either: {:?}",
        q.diagnostics
    );
    assert_eq!(disposition(&q, 2), PullRequestDisposition::Ready);
    // the declarer merges: master contains its head, and #1 is still decided on its own
    w.gone_as(2, Gone::Merged);
    w.merged.push(2);
    let obs = w.observation();
    assert!(
        obs.resolved.contains_key(&2),
        "its cross-references still name the declarer: {:?}",
        obs.resolved
    );
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(
        a.disposition,
        PullRequestDisposition::Ready,
        "{:?}",
        a.reasons
    );
    assert_eq!(a.superseded_by, None);
    assert_eq!(possible_supersessions(a).len(), 1);
    assert!(drain::cleanup_plan(&q).is_empty());
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    assert!(items.is_empty(), "{items:?}");
    assert_eq!(w.close_calls, 0);
    assert!(w.open.iter().any(|s| s.number == 1), "#1 is still open");
}

#[test]
fn an_unauthorised_supersedes_neither_holds_nor_closes() {
    let mut outsider = sim(2);
    outsider.association = "CONTRIBUTOR";
    outsider.body = "Supersedes #1".into();
    a_declaration_that_does_not_count(
        outsider,
        "#2 by someone (CONTRIBUTOR) says it supersedes #1",
    );
    // an association the forge did not report is nobody's
    let mut unread = sim(2);
    unread.association = "";
    unread.body = "Supersedes #1".into();
    a_declaration_that_does_not_count(unread, "#2 by someone (association unread)");
}

#[test]
fn a_forks_supersedes_decides_nothing() {
    let mut fork = sim(2);
    fork.cross_repository = true;
    fork.body = "Supersedes #1".into();
    assert_eq!(fork.association, "OWNER", "whoever its author is");
    a_declaration_that_does_not_count(fork, "#2 by someone (OWNER, from a fork)");
}

#[test]
fn only_what_a_bodys_author_states_is_a_supersession() {
    use crate::integration::classify::{declared_supersessions, stated_lines};
    // someone else's words, an example and a template's hint are not the author's statement
    for body in [
        "> Supersedes #1\nno, it does not",
        "  > Superseded by #1",
        "```\nSupersedes #1\n```",
        "~~~text\nSuperseded by #1\n~~~",
        "```\n~~~\nSupersedes #1\n~~~\n```",
        "<!--\nSupersedes #1\n-->",
        "<!-- say what this replaces:\nSupersedes #1 -->",
        "an unclosed fence takes the rest\n```\nSupersedes #1",
        "an unclosed comment takes the rest <!--\nSupersedes #1",
    ] {
        assert_eq!(declared_supersessions(body), Default::default(), "{body:?}");
    }
    // what stands outside them is stated, before and after
    let s = declared_supersessions(
        "Supersedes #1\n```\nSupersedes #2\n```\nSupersedes #3 <!-- note -->\n<!--\nSupersedes #4\n--> Supersedes #5\nSuperseded by #6\n> Supersedes #7",
    );
    assert_eq!((s.supersedes, s.superseded_by), (vec![1, 3], vec![6]));
    assert_eq!(
        stated_lines("a\n> quoted\n```\nfenced\n```\nb <!--\nhint\n-->\nc"),
        "a\nb <!--\nc"
    );
    // a dependency keeps the lenient reading: it can only make a pull request wait
    assert_eq!(
        crate::integration::declared_dependencies("> Depends on #4"),
        vec![4]
    );
}

#[test]
fn a_successor_whose_head_proves_nothing_closes_nothing() {
    // An owner's #1 says a contributor's fork #2 replaces it, and #3 says the same of #4, a
    // branch of this repository. Each successor was closed unmerged with its head on master:
    // #2's fork could point there at will, and #4 was reset onto master and changes no file.
    let root = scratch();
    let mut one = sim(1);
    one.body = superseded_by_body(2);
    let mut three = sim(3);
    three.body = superseded_by_body(4);
    let mut fork = sim(2);
    fork.cross_repository = true;
    fork.association = "CONTRIBUTOR";
    let mut w = World {
        open: vec![one, three],
        gone: vec![(fork, Gone::Batched), (sim(4), Gone::Reset)],
        ..Default::default()
    };
    let obs = w.observation();
    assert!(obs.resolved[&2].cross_repository && !obs.resolved[&2].merged);
    assert_eq!(obs.resolved[&4].changed_files, 0);
    let q = w.queue();
    for (n, successor, said) in [
        (1, 2, "its head lives in a fork"),
        (3, 4, "the forge names no file it changes"),
    ] {
        let a = q.get(n).unwrap();
        assert_eq!(a.disposition, PullRequestDisposition::Ready, "#{n}");
        assert_eq!(a.superseded_by, None, "#{n}");
        let s = a
            .evidence
            .iter()
            .find(|e| e.kind == "supersession")
            .unwrap();
        assert_eq!(s.status, "not_landed", "#{n}");
        assert!(s.detail.contains(said), "{}", s.detail);
        assert!(
            s.detail.contains(&format!("#{successor} ("))
                && s.detail.contains("decided on its own"),
            "{}",
            s.detail
        );
    }
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    assert!(items.iter().all(|i| i.action != "closed"), "{items:?}");
    assert_eq!(w.close_calls, 0);
}

#[test]
fn a_successor_that_landed_inside_a_batch_supersedes() {
    let root = scratch();
    // #2 was closed, not merged: a batch carried its head into master
    let mut replaced = sim(1);
    replaced.body = superseded_by_body(2);
    let mut w = World {
        open: vec![replaced],
        gone: vec![(sim(2), Gone::Batched)],
        ..Default::default()
    };
    let obs = w.observation();
    assert!(!obs.resolved[&2].merged, "the forge calls it closed");
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Superseded);
    assert_eq!(a.superseded_by, Some(2));
    let s = a
        .evidence
        .iter()
        .find(|e| e.kind == "supersession")
        .unwrap();
    assert_eq!(s.status, "landed");
    assert!(
        s.detail.contains("is closed, and master contains its head"),
        "{}",
        s.detail
    );
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    assert_eq!(items[0].action, "closed");
    assert_eq!(w.close_calls, 1);
}

#[test]
fn a_truncated_reference_read_holds_and_nothing_closes_it() {
    use crate::integration::{CrossReferenceRead, ReasonCode as R};
    let root = scratch();
    // #1's successor #3 landed, and would close it — but the pull requests that mention #1
    // were read in part, so nothing about #1 is decided
    let mut held = sim(1);
    held.body = superseded_by_body(3);
    held.references = CrossReferenceRead::Truncated;
    let mut w = World {
        open: vec![held, sim(2)],
        merged: vec![3],
        gone: vec![(sim(3), Gone::Merged)],
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Unknown);
    assert_eq!(a.reasons[0], R::DeclarationsUnread);
    assert_eq!(a.superseded_by, None);
    assert!(
        q.diagnostics
            .iter()
            .any(|d| d
                .contains("the pull requests that mention #1 were not all read (truncated: #1)")),
        "{:?}",
        q.diagnostics
    );
    let report = drain::drain(&root, &mut w, 3, false, false).unwrap();
    assert_eq!(report.merged, vec![2], "never #1");
    assert!(w.open.iter().any(|s| s.number == 1));
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    assert!(items.is_empty(), "{items:?}");
    assert_eq!(w.close_calls, 0);
    // read whole, the same world closes it
    w.open[0].references = CrossReferenceRead::Whole;
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    assert_eq!(items[0].action, "closed");
}

#[test]
fn the_closing_comment_of_a_superseded_one_does_not_claim_its_work_is_on_master() {
    let root = scratch();
    let mut replaced = sim(1);
    replaced.body = superseded_by_body(2);
    let mut w = World {
        open: vec![replaced],
        merged: vec![2],
        gone: vec![(sim(2), Gone::Merged)],
        ..Default::default()
    };
    drain::cleanup(&root, &mut w, true).unwrap();
    let c = &w.close_comments[0];
    assert!(c.starts_with("Superseded by #2, which landed.\n\n"), "{c}");
    assert!(!c.contains("its work is already on"), "{c}");
    assert!(
        c.contains("#2 was declared its replacement by someone this repository lets declare one, and `master` contains it."),
        "{c}"
    );
    assert!(
        c.contains("Evidence: superseded_by:#2 (master m0, head h1.0)."),
        "{c}"
    );
    // one closed on git's own proof is told its work is there
    let mut w = superseded_two();
    drain::cleanup(&scratch(), &mut w, true).unwrap();
    let c = &w.close_comments[0];
    assert!(
        c.starts_with("Closed by `majordomus prs cleanup`: its work is already on `master`."),
        "{c}"
    );
}

#[test]
fn a_successor_that_landed_outranks_one_still_open_and_a_self_reference_is_nothing() {
    use crate::integration::ReasonCode as R;
    let mut replaced = sim(1);
    replaced.body = "Superseded by #2\nSuperseded by #3".into();
    let mut itself = sim(4);
    itself.body = superseded_by_body(4);
    let w = World {
        open: vec![replaced, sim(2), itself],
        merged: vec![3],
        gone: vec![(sim(3), Gone::Merged)],
        ..Default::default()
    };
    let q = w.queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Superseded);
    assert_eq!(a.superseded_by, Some(3));
    assert_eq!(
        a.reasons[..2],
        [
            R::SupersededBy { number: 3 },
            R::SuccessorOpen { number: 2 }
        ]
    );
    assert_eq!(
        a.evidence
            .iter()
            .filter(|e| e.kind == "supersession")
            .count(),
        2
    );
    assert_eq!(disposition(&q, 4), PullRequestDisposition::Ready);
    // a draft whose successor landed is a draft: the earlier gate decides, and `superseded_by`
    // comes only with the disposition it belongs to
    let mut draft = sim(1);
    draft.body = superseded_by_body(3);
    draft.draft = true;
    let q = World {
        open: vec![draft],
        merged: vec![3],
        gone: vec![(sim(3), Gone::Merged)],
        ..Default::default()
    }
    .queue();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Draft);
    assert_eq!(a.superseded_by, None);
    assert!(a.reasons.contains(&R::SupersededBy { number: 3 }));
}

#[test]
fn a_supersession_is_a_line_anchored_declaration() {
    use crate::integration::classify::declared_supersessions;
    let s = declared_supersessions("Supersedes #12 and #14.\n  superseded by #20");
    assert_eq!(s.supersedes, vec![12, 14]);
    assert_eq!(s.superseded_by, vec![20]);
    for prose in [
        "this supersedes #3",
        "it was superseded by #5 in spirit, not in fact",
        "Supersedes #x",
        "Supersedesx #3",
        "Superseded #3",
    ] {
        assert_eq!(
            declared_supersessions(prose),
            Default::default(),
            "{prose:?}"
        );
    }
    let s = declared_supersessions("1. **Superseded by:** #7, #8 & #9");
    assert_eq!(s.superseded_by, vec![7, 8, 9]);
    // and a dependency marker is not a supersession, nor the other way round
    assert_eq!(declared_supersessions("Depends on #4"), Default::default());
    assert!(crate::integration::declared_dependencies("Supersedes #4").is_empty());
}

#[test]
fn the_forge_reads_only_what_a_supersession_of_an_open_pull_request_names() {
    use super::forge::resolved_for;
    use serde_json::json;
    let mut one = observe_pr(&sim(1));
    one.body = "Superseded by #7".into();
    let open = vec![one, observe_pr(&sim(2))];
    let closed = json!([
        // supersedes an open one: read
        {"number": 5, "state": "MERGED", "headRefOid": "h5", "body": "Supersedes #2",
         "authorAssociation": "MEMBER", "isCrossRepository": false},
        // supersedes only what is not open: not needed
        {"number": 6, "state": "MERGED", "headRefOid": "h6", "body": "Supersedes #40"},
        // mentions the word in prose: no declaration
        {"number": 8, "state": "CLOSED", "headRefOid": "h8", "body": "this supersedes #1"},
        // what an answer without a state says: not a closed pull request
        {"number": 1, "headRefOid": "h1", "body": "Supersedes #2"}
    ]);
    let mut viewed = Vec::new();
    let resolved = resolved_for(&open, &closed, |m| {
        viewed.push(m);
        Some(json!({"number": m, "state": "CLOSED", "headRefOid": format!("h{m}"), "body": ""}))
    });
    assert_eq!(
        viewed,
        [7],
        "only the named successor that is not open is viewed"
    );
    assert_eq!(resolved.keys().copied().collect::<Vec<_>>(), [5, 7]);
    assert!(resolved[&5].merged);
    assert!(!resolved[&7].merged);
    assert_eq!(resolved[&5].head_sha, "h5");
    // a source of a cross-reference says who declared; a view does not, and declares nothing
    assert_eq!(
        (
            resolved[&5].author_association.as_str(),
            resolved[&5].cross_repository
        ),
        ("MEMBER", false)
    );
    assert_eq!(
        (
            resolved[&7].author_association.as_str(),
            resolved[&7].cross_repository
        ),
        ("", true)
    );
    // a successor the forge cannot show is left out: unread, never landed
    let none = resolved_for(&open, &json!([]), |_| None);
    assert!(none.is_empty());
}

/// A scratch repository whose first commit holds `files`, with `master` on it.
fn repository_with(files: &[(&str, &str)]) -> (std::path::PathBuf, String) {
    let dir = scratch();
    git(&dir, &["init", "-q", "-b", "master"]);
    for (p, c) in files {
        std::fs::write(dir.join(p), c).unwrap();
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "-q", "-B", "master", &base]);
    (dir, base)
}

/// A repository for patch identity: `base` commits `f.txt`, `master` cherry-picks a branch's
/// change of it and then changes it again. Returns the directory, master and the heads.
fn patch_fixture() -> (std::path::PathBuf, String, BTreeMap<&'static str, String>) {
    let (dir, base) = repository_with(&[("f.txt", "a\n"), ("g.txt", "g\n")]);
    let mut heads = BTreeMap::new();
    // every commit cherry-picked onto master, which then moved past it: the merge conflicts
    let picked = commit_on(&dir, "picked", &base, &[("f.txt", "b\n")]);
    heads.insert("picked", picked.clone());
    // the same change, and one more that master never got
    commit_on(&dir, "partial", &base, &[("f.txt", "b\n")]);
    let partial = commit_on(&dir, "partial", "partial", &[("h.txt", "h\n")]);
    heads.insert("partial", partial);
    // the same change, and a merge of a side branch: a merge has no patch to compare
    commit_on(&dir, "side", &base, &[("s.txt", "s\n")]);
    commit_on(&dir, "merged", &base, &[("f.txt", "b\n")]);
    git(
        &dir,
        &["merge", "-q", "--no-ff", "-m", "merge side", "side"],
    );
    heads.insert("merged", git(&dir, &["rev-parse", "HEAD"]));
    git(&dir, &["checkout", "-q", "master"]);
    // -x: a commit of its own, never the same object when the clock has not moved
    git(&dir, &["cherry-pick", "-x", &picked]);
    let master = commit_on(&dir, "master", "master", &[("f.txt", "c\n")]);
    (dir, master, heads)
}

#[test]
fn every_patch_on_master_is_redundant_and_a_partial_match_never_is() {
    use crate::integration::{relation_to_master, ReasonCode as R};
    let (dir, master, heads) = patch_fixture();
    assert_eq!(
        relation_to_master(&dir, &master, &heads["picked"]),
        RelationToMaster::PatchIdsUpstream { commits: 1 }
    );
    // one commit master lacks is enough to refuse it, and so is a merge commit
    for partial in ["partial", "merged"] {
        let r = relation_to_master(&dir, &master, &heads[partial]);
        assert!(
            matches!(r, RelationToMaster::Conflicting { .. }),
            "{partial}: {r:?}"
        );
    }
    // classified: strong evidence, redundant, closable
    let obs = World {
        open: vec![sim(1)],
        ..Default::default()
    }
    .observation();
    let q = build_queue(&obs, "m0", |_| RelationToMaster::PatchIdsUpstream {
        commits: 1,
    });
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::Redundant);
    assert_eq!(a.reasons[0], R::PatchIdsUpstream);
    assert_eq!(a.reasons[0].to_string(), "patch_ids_upstream");
    let rel = a
        .evidence
        .iter()
        .find(|e| e.kind == "relation_to_master")
        .unwrap();
    assert_eq!(rel.status, "patch_ids_upstream");
    assert_eq!(
        serde_json::to_value(&a.relation).unwrap(),
        serde_json::json!({"kind": "patch_ids_upstream", "commits": 1})
    );
}

#[test]
fn equal_patches_never_override_a_clean_merge_that_changes_authored_paths() {
    use crate::integration::relation_to_master;
    // master took the change and reverted it since: equal patches, and master lacks the change
    let (dir, base) = repository_with(&[("f.txt", "a\n")]);
    let head = commit_on(&dir, "change", &base, &[("f.txt", "b\n")]);
    git(&dir, &["checkout", "-q", "master"]);
    git(&dir, &["cherry-pick", "-x", &head]);
    let master = commit_on(&dir, "master", "master", &[("f.txt", "a\n")]);
    assert_eq!(
        crate::integration::relation::patches_upstream(&dir, &master, &head),
        Some(1),
        "git cherry calls every patch upstream"
    );
    let r = relation_to_master(&dir, &master, &head);
    assert!(matches!(r, RelationToMaster::Behind { .. }), "{r:?}");
}

#[test]
fn the_words_written_before_the_rename_still_read() {
    use crate::integration::{EvidenceKind, ReasonCode as R};
    // a closure an executor recorded before 0.13: `closed_superseded`, for what is now
    // `closed_redundant`, with the strong relation's reason
    let line = r#"{"at":"2026-10-02T09:00:00Z","actor":"t (pid 1 on h)","action":"closed_superseded","pr":2,"master_before":"m1","head_sha":"h2","master_after":null,"reasons":["head_reachable_from_master"],"detail":"Closed by `majordomus prs cleanup`: its work is already on `master`.","evidence":[{"kind":"relation_to_master","status":"contained","detail":"the head is an ancestor of master"}]}"#;
    let e: drain::IntegrationEvent = serde_json::from_str(line).unwrap();
    assert_eq!(e.action, drain::IntegrationAction::ClosedSuperseded);
    assert_eq!(e.reasons, vec![R::HeadReachableFromMaster]);
    assert_eq!(e.evidence[0].kind, EvidenceKind::RelationToMaster);
    let original: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(serde_json::to_value(&e).unwrap(), original);
    // and the trail folds both closures the same way: a closed pull request waits no more
    for action in ["closed_superseded", "closed_redundant"] {
        let became = old_line("became_actionable");
        let closed = old_line(action);
        let trail: Vec<drain::IntegrationEvent> = [became, closed]
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert!(super::wait::waits(&trail).is_empty(), "{action}");
    }
    // the disposition words read, the old one among them, and the relation's word is unchanged
    for word in ["superseded", "redundant", "possibly_redundant"] {
        let d: PullRequestDisposition = serde_json::from_value(serde_json::json!(word)).unwrap();
        assert_eq!(d.as_str(), word);
    }
    assert_eq!(
        serde_json::from_value::<RelationToMaster>(serde_json::json!({"kind": "superseded"}))
            .unwrap(),
        RelationToMaster::Superseded
    );
    // a cleanup item and an observation written before: no successor, nothing resolved
    let item: drain::CleanupItem = serde_json::from_value(serde_json::json!({
        "pr": 2, "disposition": "superseded", "reasons": ["merge_changes_nothing"],
        "action": "closed"
    }))
    .unwrap();
    assert_eq!(item.disposition, PullRequestDisposition::Superseded);
    let mut obs = serde_json::to_value(World::default().observation()).unwrap();
    obs.as_object_mut().unwrap().remove("resolved");
    let obs: ForgeObservation = serde_json::from_value(obs).unwrap();
    assert!(obs.resolved.is_empty());
}

#[test]
fn redundant_and_superseded_are_two_words_of_the_cleanup_lane() {
    use crate::integration::{IntegrationGate as G, IntegrationLane as L};
    assert_eq!(PullRequestDisposition::Redundant.as_str(), "redundant");
    assert_eq!(PullRequestDisposition::Superseded.as_str(), "superseded");
    let cleanup: Vec<&str> = PullRequestDisposition::ALL
        .iter()
        .filter(|d| d.lane() == L::Cleanup)
        .map(|d| d.as_str())
        .collect();
    assert_eq!(
        cleanup,
        ["redundant", "superseded", "possibly_redundant", "obsolete"]
    );
    // the supersession gate is asked before the relation to master
    let at = |g: G| G::ALL.iter().position(|x| *x == g).unwrap();
    assert_eq!(at(G::Supersession) + 1, at(G::RelationToMaster));
    assert_eq!(G::Supersession.as_str(), "supersession");
}

// ---------------------------------------------------------------- a merge proved where it landed (WP6)

fn at_of(master: &str, head: &str) -> crate::integration::EvaluatedAgainst {
    crate::integration::EvaluatedAgainst {
        master_sha: master.into(),
        head_sha: head.into(),
        ..Default::default()
    }
}

/// A merge the trail asked for, as an executor that stopped after asking leaves it.
fn asked(root: &std::path::Path, pr: u64, master: &str, head: &str) {
    drain::record(
        root,
        drain::IntegrationEvent {
            pr: Some(pr),
            master_before: Some(master.into()),
            head_sha: Some(head.into()),
            detail: "--merge".into(),
            ..drain::IntegrationEvent::of(drain::IntegrationAction::MergeAttempted)
        },
    )
    .unwrap();
}

#[test]
fn a_foreign_merge_between_decision_and_merge_fails_verification() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        foreign_merge_before: [1].into(),
        ..Default::default()
    };
    let report = drain::drain(&root, &mut w, 2, false, false).unwrap();
    assert!(report.merged.is_empty(), "{report:?}");
    match report.steps.last() {
        Some(DrainStepOutcome::VerificationFailed { pr: 1, reason }) => {
            assert!(reason.contains("unexpected_master"), "{reason}")
        }
        other => panic!("expected a failed verification, got {other:?}"),
    }
    // the drain stopped there: #2 was not merged onto a master nobody verified
    assert_eq!(w.merge_calls, 1);
    assert_eq!(trail_actions(&root).last().unwrap(), "verification_failed");
}

#[test]
fn a_timed_out_merge_that_landed_is_recorded_as_merged() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        answer_lost_after_landing: [1].into(),
        ..Default::default()
    };
    let outcome = drain::step(&root, &mut w, false, false).unwrap();
    assert!(
        matches!(outcome, DrainStepOutcome::Merged { pr: 1, .. }),
        "{outcome:?}"
    );
    assert_eq!(
        w.merge_calls, 1,
        "a merge whose answer was lost is never asked for again"
    );
    let trail = drain::events(&root);
    let last = trail.last().unwrap();
    assert_eq!(last.action.as_str(), "merge_succeeded");
    assert!(last.detail.contains("answer was lost"), "{}", last.detail);
    assert_eq!(last.merge_commit.as_deref(), Some("merge-1"));
}

#[test]
fn a_timed_out_merge_that_never_landed_is_refused_and_not_asked_again() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        answer_lost_before_landing: [1].into(),
        ..Default::default()
    };
    let outcome = drain::step(&root, &mut w, false, false).unwrap();
    assert!(
        matches!(outcome, DrainStepOutcome::MergeRefused { pr: 1, .. }),
        "{outcome:?}"
    );
    assert_eq!((w.merge_calls, w.verify_calls), (1, 1));
    assert_eq!(trail_actions(&root).last().unwrap(), "merge_failed");
}

#[test]
fn a_verified_merge_names_its_merge_commit() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    drain::drain(&root, &mut w, 1, false, false).unwrap();
    let merged = drain::events(&root)
        .into_iter()
        .find(|e| e.action.as_str() == "merge_succeeded")
        .unwrap();
    assert_eq!(merged.merge_commit.as_deref(), Some("merge-1"));
}

#[test]
fn reconcile_closes_an_interrupted_merge_that_landed() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        ..Default::default()
    };
    // the last executor asked, the forge merged, and the executor stopped before verifying
    Integrator::merge(&mut w, 1, "h1.0", "merge").unwrap();
    asked(&root, 1, "m0", "h1.0");
    let report = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert!(
        matches!(
            report.steps.first(),
            Some(DrainStepOutcome::Merged { pr: 1, .. })
        ),
        "{report:?}"
    );
    assert_eq!(report.merged, vec![1]);
    assert_eq!(
        w.merge_calls, 1,
        "the interrupted merge is verified, not repeated"
    );
    let ended = drain::events(&root)
        .into_iter()
        .find(|e| e.action.as_str() == "merge_succeeded")
        .unwrap();
    assert!(ended.detail.starts_with("reconciled"), "{}", ended.detail);
    // and reconciling again finds nothing: the attempt has its end now
    assert_eq!(drain::reconcile(&root, &mut w).unwrap(), None);
}

#[test]
fn reconcile_closes_an_interrupted_merge_that_never_landed() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    asked(&root, 1, "m0", "h1.0");
    let report = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert!(
        matches!(
            report.steps.first(),
            Some(DrainStepOutcome::MergeRefused { pr: 1, .. })
        ),
        "{report:?}"
    );
    // ended as refused, the change is decided afresh and merges in this same drain
    assert_eq!(report.merged, vec![1]);
    assert_eq!(w.merge_calls, 1);
}

#[test]
fn verification_failure_blocks_the_next_drain_until_acknowledged() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        verify_fails: true,
        ..Default::default()
    };
    let first = drain::drain(&root, &mut w, 2, false, false).unwrap();
    assert!(matches!(
        first.steps.last(),
        Some(DrainStepOutcome::VerificationFailed { pr: 1, .. })
    ));
    w.verify_fails = false;
    let calls = w.merge_calls;
    // the next drain merges nothing, and says why and how to go on
    let halted = drain::drain(&root, &mut w, 2, false, false).unwrap();
    assert!(
        matches!(
            halted.steps.as_slice(),
            [DrainStepOutcome::Halted { pr: Some(1), .. }]
        ),
        "{halted:?}"
    );
    assert!(
        halted.stopped.contains("--resume-after-failure"),
        "{}",
        halted.stopped
    );
    assert_eq!(w.merge_calls, calls);
    // a dry run is not a merge: it still plans
    let dry = drain::drain(&root, &mut w, 1, true, false).unwrap();
    assert!(!matches!(
        dry.steps.first(),
        Some(DrainStepOutcome::Halted { .. })
    ));
    // a person looked
    drain::acknowledge_failure(&root, "a person").unwrap();
    assert_eq!(drain::halted_by(&root), None);
    // #1's merge moved master; #2 is brought up to date the way a refresh would
    let master = w.master;
    w.open.iter_mut().for_each(|s| s.contains = master);
    let resumed = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert_eq!(resumed.merged, vec![2]);
}

#[test]
fn a_continuous_drain_stops_on_an_unacknowledged_failure() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        verify_fails: true,
        ..Default::default()
    };
    drain::drain(&root, &mut w, 1, false, false).unwrap();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(None),
        &stop,
        &mut |_| panic!("waited while halted"),
        &mut |_, _| {},
    );
    assert_eq!(report.cycles, 1);
    assert!(report.merged.is_empty());
    assert!(
        report.stopped.contains("--resume-after-failure"),
        "{}",
        report.stopped
    );
    assert!(
        report.failure.is_none(),
        "a halt is a finding, not an outage"
    );
}

#[test]
fn a_base_that_does_not_require_up_to_date_branches_is_said() {
    let w = World::default();
    let mut obs = w.observation();
    obs.up_to_date_required = Some(false);
    let q = build_queue(&obs, &master_sha(0), |_| unreachable!());
    // said in the policy, for the owner to change (D8); not a defect of the queue's answer
    assert_eq!(q.policy.up_to_date_required, Some(false));
    assert!(q.diagnostics.is_empty(), "{:?}", q.diagnostics);
    obs.up_to_date_required = Some(true);
    let q = build_queue(&obs, &master_sha(0), |_| unreachable!());
    assert_eq!(q.policy.up_to_date_required, Some(true));
}

#[test]
fn the_up_to_date_requirement_is_read_from_protection_and_rulesets() {
    use super::forge::{protection_requires_up_to_date, rules_require_up_to_date};
    use serde_json::json;
    assert!(protection_requires_up_to_date(
        &json!({"required_status_checks": {"strict": true, "contexts": ["ci"]}})
    ));
    assert!(!protection_requires_up_to_date(
        &json!({"required_status_checks": {"strict": false}})
    ));
    assert!(!protection_requires_up_to_date(&json!({})));
    assert!(rules_require_up_to_date(&json!([
        {"type": "required_status_checks",
         "parameters": {"strict_required_status_checks_policy": true, "required_status_checks": []}}
    ])));
    assert!(!rules_require_up_to_date(
        &json!([{"type": "pull_request", "parameters": {}}])
    ));
}

/// The landing proof over real commits: a merge of the decided head onto the decided master
/// is proved; the same merge after another one landed first is refused by name.
#[test]
fn a_landing_is_proved_from_the_merge_commit_parents() {
    let d = unique_temp("mj-landing");
    git(&d, &["init", "-q", "-b", "master"]);
    git(&d, &["commit", "-q", "--allow-empty", "-m", "base"]);
    let m0 = git(&d, &["rev-parse", "HEAD"]);
    let branch = |name: &str| {
        git(&d, &["checkout", "-q", "-b", name, &m0]);
        git(&d, &["commit", "-q", "--allow-empty", "-m", name]);
        let h = git(&d, &["rev-parse", "HEAD"]);
        git(&d, &["checkout", "-q", "master"]);
        h
    };
    let ours = branch("ours");
    let theirs = branch("theirs");
    // another change lands first, then ours on top of it
    git(&d, &["merge", "-q", "--no-ff", "-m", "theirs", &theirs]);
    git(&d, &["merge", "-q", "--no-ff", "-m", "ours", &ours]);
    let master = git(&d, &["rev-parse", "HEAD"]);
    let err = drain::landing(&d, &at_of(&m0, &ours), "merge", &master).unwrap_err();
    assert!(err.contains("unexpected_master"), "{err}");
    // decided against the master it actually landed on, it is proved, and named
    let m1 = git(&d, &["rev-parse", "HEAD^1"]);
    assert_eq!(
        drain::landing(&d, &at_of(&m1, &ours), "merge", &master).unwrap(),
        Some(master.clone())
    );
    // nothing landed on a master that has not moved
    assert!(drain::landing(&d, &at_of(&master, &ours), "merge", &master).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

// ---------------------------------------------------------------- a refused candidate does not block the queue (WP7)

#[test]
fn a_refused_merge_lets_the_next_pull_request_merge() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        merge_refuses: [1].into(),
        trail_root: Some(root.clone()),
        ..Default::default()
    };
    let report = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert_eq!(report.merged, vec![2], "{report:?}");
    match report.steps.first() {
        Some(DrainStepOutcome::MergeRefused { pr: 1, class, .. }) => {
            assert_eq!(*class, drain::FailureClass::PolicyViolation);
            assert!(class.recoverable());
        }
        other => panic!("expected #1 refused, got {other:?}"),
    }
    // #1 was asked once: the queue held it back, it was not tried again
    assert_eq!(w.merge_calls, 2);
    let failed = drain::events(&root)
        .into_iter()
        .find(|e| e.action.as_str() == "merge_failed")
        .unwrap();
    assert_eq!(failed.class, Some(drain::FailureClass::PolicyViolation));
}

#[test]
fn a_refused_candidate_is_held_until_its_head_or_master_moves() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        merge_refuses: [1].into(),
        trail_root: Some(root.clone()),
        ..Default::default()
    };
    drain::drain(&root, &mut w, 1, false, false).unwrap();
    let q = w.observe().unwrap();
    let a = q.get(1).unwrap();
    assert_eq!(a.disposition, PullRequestDisposition::NeedsRepair);
    assert!(
        a.reasons
            .iter()
            .any(|r| matches!(r, crate::integration::ReasonCode::ExecutorMergeRefused { head } if head == "h1.0")),
        "{:?}",
        a.reasons
    );
    assert_eq!(q.next_merge, None);
    // the author pushes: a new head clears the hold, and the forge now accepts it
    w.merge_refuses.clear();
    w.open[0].head = "h1.1".into();
    let report = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert_eq!(report.merged, vec![1]);
}

#[test]
fn a_transient_refusal_holds_nothing_back() {
    let root = scratch();
    let mut q_world = World {
        open: vec![sim(1)],
        trail_root: Some(root.clone()),
        ..Default::default()
    };
    // what a timed-out merge that never landed leaves
    drain::record(
        &root,
        drain::IntegrationEvent {
            pr: Some(1),
            head_sha: Some("h1.0".into()),
            master_before: Some("m0".into()),
            detail: "Post https://api.github.com/graphql: timed out".into(),
            ..drain::IntegrationEvent::of(drain::IntegrationAction::MergeFailed)
        },
    )
    .unwrap();
    let q = q_world.observe().unwrap();
    assert_eq!(q.get(1).unwrap().disposition, PullRequestDisposition::Ready);
    assert_eq!(
        drain::events(&root)[0].class,
        Some(drain::FailureClass::Transient)
    );
}

#[test]
fn a_failing_refresh_does_not_block_the_next_refresh() {
    let root = scratch();
    let mut w = World {
        // both behind master: each needs it brought in
        master: 1,
        open: vec![sim(1), sim(2)],
        refresh_fails: true,
        trail_root: Some(root.clone()),
        ..Default::default()
    };
    let first = drain::drain(&root, &mut w, 1, false, true).unwrap();
    assert!(
        matches!(
            first.steps.first(),
            Some(DrainStepOutcome::RefreshFailed { pr: 1, .. })
        ),
        "{first:?}"
    );
    // the failure is #1's: the drain went on and tried #2 rather than stopping
    assert!(
        first
            .steps
            .iter()
            .any(|s| matches!(s, DrainStepOutcome::RefreshFailed { pr: 2, .. })),
        "{first:?}"
    );
    w.refresh_fails = false;
    let q = w.observe().unwrap();
    assert_eq!(
        q.get(1).unwrap().disposition,
        PullRequestDisposition::NeedsRepair
    );
    assert_eq!(
        q.get(2).unwrap().disposition,
        PullRequestDisposition::NeedsRepair
    );
    // master moves on: both may be tried again
    w.master = 2;
    let q = w.observe().unwrap();
    assert_eq!(q.next_refresh, vec![1, 2]);
}

#[test]
fn stop_is_honoured_inside_a_multi_merge_drain() {
    let root = scratch();
    let stop = std::sync::atomic::AtomicBool::new(true);
    let mut w = World {
        open: vec![sim(1), sim(2)],
        ..Default::default()
    };
    let report = drain::drain_until(&root, &mut w, 3, false, false, Some(&stop)).unwrap();
    assert!(report.merged.is_empty());
    assert_eq!(w.merge_calls, 0);
    assert!(
        report.stopped.starts_with("asked to stop"),
        "{}",
        report.stopped
    );
}

#[test]
fn a_short_outage_does_not_end_continuous_mode() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        outages: vec!["HTTP 502: Bad Gateway".into(); drain::CONTINUOUS_TRANSIENT_LIMIT],
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(Some(1)),
        &stop,
        &mut |_| {},
        &mut |_, _| {},
    );
    assert!(report.failure.is_none(), "{report:?}");
    assert_eq!(report.merged, vec![1]);
    let outages = trail_actions(&root)
        .iter()
        .filter(|a| *a == "observe_failed")
        .count();
    assert_eq!(outages, drain::CONTINUOUS_TRANSIENT_LIMIT);
}

#[test]
fn an_outage_longer_than_the_limit_ends_continuous_mode() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        outages: vec!["HTTP 502: Bad Gateway".into(); drain::CONTINUOUS_TRANSIENT_LIMIT + 1],
        ..Default::default()
    };
    let stop = std::sync::atomic::AtomicBool::new(false);
    let report = drain::continuous(
        &root,
        &mut w,
        continuous_opts(None),
        &stop,
        &mut |_| {},
        &mut |_, _| {},
    );
    assert_eq!(report.failure.as_deref(), Some("HTTP 502: Bad Gateway"));
    assert!(report.merged.is_empty());
}

#[test]
fn failure_classes_say_whether_the_next_candidate_may_be_tried() {
    use drain::FailureClass as C;
    assert_eq!(
        C::of_merge_refusal(
            "Pull request is not mergeable: the base branch policy prohibits the merge"
        ),
        C::PolicyViolation
    );
    assert_eq!(
        C::of_merge_refusal("merge conflict between base and head"),
        C::Conflict
    );
    assert_eq!(
        C::of_merge_refusal("Head branch was modified. Review and try the merge again."),
        C::Stale
    );
    assert_eq!(
        C::of_merge_refusal("HTTP 503: Service Unavailable"),
        C::Transient
    );
    assert_eq!(
        C::of_refresh_failure("the merge of master conflicts after all"),
        C::Conflict
    );
    assert_eq!(
        C::of_refresh_failure("! [rejected] (fetch first)"),
        C::Stale
    );
    for c in [
        C::Stale,
        C::Conflict,
        C::NewFailingCheck,
        C::ReviewRevoked,
        C::Transient,
        C::PolicyViolation,
    ] {
        assert!(c.recoverable(), "{c}");
    }
    assert!(!C::VerificationFailed.recoverable());
    assert!(!C::Unreadable.recoverable());
}

// ---------------------------------------------------------------- cleanup closes only what it decided (WP8)

/// Two superseded pull requests and one ready: what every cleanup test starts from.
fn superseded_two() -> World {
    let mut w = World {
        open: vec![sim(1), sim(2), sim(3)],
        ..Default::default()
    };
    w.open[0].redundant_after = Some(99);
    w.open[1].redundant_after = Some(99);
    w.merged = vec![99];
    w
}

#[test]
fn cleanup_apply_closes_only_superseded_with_its_deciding_reason() {
    let root = scratch();
    let mut w = superseded_two();
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    let closed: Vec<u64> = items
        .iter()
        .filter(|i| i.action == "closed")
        .map(|i| i.pr)
        .collect();
    assert_eq!(closed, [1, 2], "{items:?}");
    assert!(
        w.open.iter().any(|s| s.number == 3),
        "the ready one is never closed"
    );
    // the comment names the reason that decided it, the base, the master and the head
    let c = &w.close_comments[0];
    assert!(
        c.contains("on `master`") && c.contains("master m0") && c.contains("head h1.0"),
        "{c}"
    );
    let cited = c
        .split("Evidence: ")
        .nth(1)
        .and_then(|e| e.split(" (master").next())
        .unwrap();
    assert_eq!(
        cited, "merge_changes_nothing",
        "only the deciding reason is cited: {c}"
    );
}

#[test]
fn a_moved_head_is_not_closed() {
    let root = scratch();
    let mut w = superseded_two();
    // between the first observation and the second, a person pushes to #1
    w.meanwhile.push((2, Meanwhile::HeadMoves(1)));
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    let by: BTreeMap<u64, String> = items.into_iter().map(|i| (i.pr, i.action)).collect();
    assert!(by[&1].starts_with("stale_decision"), "{}", by[&1]);
    assert_eq!(by[&2], "closed");
    assert!(w.open.iter().any(|s| s.number == 1), "#1 is still open");
    assert_eq!(w.close_calls, 1, "#1's closure never reached the forge");
    assert!(trail_actions(&root).contains(&"stale_decision".to_string()));
}

#[test]
fn the_forge_refuses_to_close_a_head_that_moved_after_the_decision() {
    let mut w = superseded_two();
    w.open[0].head = "h1.1".into();
    let err = Integrator::close(&mut w, 1, "h1.0", "closing").unwrap_err();
    assert!(err.contains("head moved"), "{err}");
    assert!(w.open.iter().any(|s| s.number == 1));
}

#[test]
fn a_failed_close_is_recorded_and_the_next_is_still_closed() {
    let root = scratch();
    let mut w = superseded_two();
    w.close_refuses.insert(1);
    let items = drain::cleanup(&root, &mut w, true).unwrap();
    let by: BTreeMap<u64, String> = items.into_iter().map(|i| (i.pr, i.action)).collect();
    assert!(by[&1].starts_with("close_failed"), "{}", by[&1]);
    assert_eq!(by[&2], "closed");
    let failed = drain::events(&root)
        .into_iter()
        .find(|e| e.action.as_str() == "close_failed")
        .unwrap();
    assert_eq!(failed.pr, Some(1));
}

#[test]
fn a_cleanup_listing_closes_nothing_and_observes_once() {
    let root = scratch();
    let mut w = superseded_two();
    let items = drain::cleanup(&root, &mut w, false).unwrap();
    assert!(items.iter().all(|i| i.action == "would_close"), "{items:?}");
    assert_eq!((w.close_calls, w.observations), (0, 1));
    assert!(trail_actions(&root).is_empty());
}

// ---------------------------------------------------------------- reads write nothing (WP17)

/// A read of the queue — status, plan, explain, the capability, the Cockpit — leaves the
/// checkout as it was: neither the relation cache nor the summary is written. Only the path
/// that just observed the forge keeps what it learnt.
#[test]
fn a_read_of_the_queue_writes_nothing() {
    let dir = unique_temp("mj-integration-pure-read");
    git(&dir, &["init", "-q", "-b", "master"]);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "base"]);
    let master = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["update-ref", "refs/remotes/origin/master", &master]);
    observed(&dir, &master, vec![]);
    let cache = dir.join(".ai/local/state/integration/relations.json");
    let summary = dir.join(".git/majordomus/integration/summary.json");
    let _ = std::fs::remove_file(&cache);
    crate::integration::queue_of(&dir).unwrap();
    assert!(!cache.exists() && !summary.exists(), "a read wrote state");
    crate::integration::queue_and_record(&dir).unwrap();
    assert!(
        cache.exists() && summary.exists(),
        "the recording path keeps nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- repair: one named pull request

use crate::integration::repair::{self, RepairOutcome, RepairRefusal, RepairTarget};

/// #1 behind master and clean but for derived files; #2 conflicting on an authored path;
/// #3 up to date; #4 behind, from a fork; #5 behind, auto-merge armed; #6 behind, another base.
fn repair_world() -> World {
    let mut conflicting = sim(2);
    conflicting.conflicts_after = Some(90);
    let mut up_to_date = sim(3);
    up_to_date.contains = 1;
    let mut fork = sim(4);
    fork.cross_repository = true;
    let mut armed = sim(5);
    armed.auto_merge = true;
    let mut elsewhere = sim(6);
    elsewhere.base = "release";
    World {
        open: vec![sim(1), conflicting, up_to_date, fork, armed, elsewhere],
        merged: vec![90],
        master: 1,
        ..Default::default()
    }
}

fn outcome(r: &repair::RepairReport) -> String {
    match &r.outcome {
        RepairOutcome::NothingToRepair { .. } => "nothing".into(),
        RepairOutcome::WouldRepair => "would_repair".into(),
        RepairOutcome::Repaired { .. } => "repaired".into(),
        RepairOutcome::Refused { refusal } => format!(
            "refused:{}",
            serde_json::to_value(refusal).unwrap()["kind"]
                .as_str()
                .unwrap()
        ),
    }
}

#[test]
fn a_repair_is_eligible_only_when_behind_with_no_authored_conflict() {
    let w = repair_world();
    let q = w.queue();
    let say = |t: &str| outcome(&repair::dry_run(&q, &RepairTarget::parse(t)));
    assert_eq!(say("1"), "would_repair");
    // by its head branch, as `scripts/unblock` took it
    assert_eq!(say("feature/1"), "would_repair");
    assert_eq!(say("2"), "refused:authored_conflict");
    assert_eq!(say("3"), "nothing");
    assert_eq!(say("4"), "refused:fork_head");
    assert_eq!(say("5"), "refused:auto_merge_armed");
    assert_eq!(say("6"), "refused:other_base");
    assert_eq!(say("99"), "refused:not_open");
    assert_eq!(say("feature/nowhere"), "refused:not_open");
    // the authored conflict names its files, which the classification found
    let RepairOutcome::Refused {
        refusal: RepairRefusal::AuthoredConflict { paths },
    } = repair::dry_run(&q, &RepairTarget::Number(2)).outcome
    else {
        panic!("#2 is not refused as an authored conflict");
    };
    assert_eq!(paths, ["src/2.rs"]);
}

#[test]
fn a_repair_leaves_what_is_on_master_already_to_cleanup() {
    let mut w = repair_world();
    w.open[0].redundant_after = Some(90);
    let q = w.queue();
    let r = repair::dry_run(&q, &RepairTarget::Number(1));
    assert_eq!(outcome(&r), "nothing", "{r:?}");
    let RepairOutcome::NothingToRepair { why } = r.outcome else {
        unreachable!()
    };
    assert!(why.contains("cleanup"), "{why}");
}

#[test]
fn an_applied_repair_records_its_selection_and_attempt_before_the_push_and_its_outcome_after() {
    // the integrator, wrapped: it reads the trail at the moment the push is asked for
    struct Watching<'r> {
        world: World,
        root: &'r std::path::Path,
        trail_at_push: Option<Vec<String>>,
    }
    impl Integrator for Watching<'_> {
        fn observe(&mut self) -> Result<IntegrationQueue, String> {
            self.world.observe()
        }
        fn merge(&mut self, pr: u64, head: &str, method: &str) -> Result<(), String> {
            self.world.merge(pr, head, method)
        }
        fn verify(
            &mut self,
            pr: u64,
            at: &crate::integration::EvaluatedAgainst,
            method: &str,
        ) -> Result<drain::Landed, drain::NotLanded> {
            self.world.verify(pr, at, method)
        }
        fn refresh_branch(
            &mut self,
            a: &PullRequestAssessment,
            base: &str,
        ) -> Result<String, String> {
            self.trail_at_push = Some(trail_actions(self.root));
            self.world.refresh_branch(a, base)
        }
        fn close(&mut self, pr: u64, head: &str, comment: &str) -> Result<(), String> {
            self.world.close(pr, head, comment)
        }
    }
    let root = scratch();
    let mut w = Watching {
        world: repair_world(),
        root: &root,
        trail_at_push: None,
    };
    let r = repair::apply(&root, &mut w, &RepairTarget::Number(1)).unwrap();
    assert_eq!(outcome(&r), "repaired", "{r:?}");
    assert!(!r.dry_run);
    assert_eq!(
        w.trail_at_push.as_deref(),
        Some(
            &[
                "repair_selected".to_string(),
                "repair_attempted".to_string()
            ][..]
        ),
        "the push was asked for before the trail named it"
    );
    assert_eq!(
        trail_actions(&root),
        ["repair_selected", "repair_attempted", "repaired"]
    );
    let repaired = drain::events(&root).pop().unwrap();
    assert_eq!(repaired.head_after.as_deref(), Some("h1.1"));
    assert_eq!(repaired.master_after.as_deref(), Some("m1"));
    // never a merge into master: the forge's merge was not asked for, and master stands
    assert_eq!((w.world.merge_calls, w.world.master), (0, 1));
    assert_eq!(w.world.refresh_calls, 1);
    assert_eq!(
        w.world.observations, 1,
        "the act is decided on a fresh observation"
    );
}

#[test]
fn an_applied_repair_whose_attempt_cannot_be_recorded_pushes_nothing() {
    let root = scratch();
    let mut w = repair_world();
    // the second write — repair_attempted — is refused
    drain::FAIL_WRITE.with(|n| n.set(2));
    let r = repair::apply(&root, &mut w, &RepairTarget::Number(1)).unwrap();
    drain::FAIL_WRITE.with(|n| n.set(0));
    assert_eq!(outcome(&r), "refused:trail_unwritable", "{r:?}");
    assert_eq!(w.refresh_calls, 0, "a push reached the remote unrecorded");
    assert_eq!(trail_actions(&root), ["repair_selected"]);
}

#[test]
fn an_applied_repair_that_is_refused_says_so_on_the_trail_and_pushes_nothing() {
    let root = scratch();
    let mut w = repair_world();
    let r = repair::apply(&root, &mut w, &RepairTarget::Number(2)).unwrap();
    assert_eq!(outcome(&r), "refused:authored_conflict");
    assert_eq!(w.refresh_calls, 0);
    let refused = drain::events(&root);
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert_eq!(refused[0].action.as_str(), "repair_refused");
    assert_eq!(refused[0].class, Some(drain::FailureClass::Conflict));
    assert!(
        refused[0].detail.contains("src/2.rs"),
        "{}",
        refused[0].detail
    );
    // a pull request with nothing to bring in records nothing and pushes nothing
    let root = scratch();
    let r = repair::apply(&root, &mut w, &RepairTarget::Number(3)).unwrap();
    assert_eq!(outcome(&r), "nothing");
    assert!(trail_actions(&root).is_empty());
    assert_eq!(w.refresh_calls, 0);
}

#[test]
fn an_applied_repair_whose_push_is_refused_records_the_class_and_reports_it() {
    struct Moved(World);
    impl Integrator for Moved {
        fn observe(&mut self) -> Result<IntegrationQueue, String> {
            self.0.observe()
        }
        fn merge(&mut self, pr: u64, head: &str, method: &str) -> Result<(), String> {
            self.0.merge(pr, head, method)
        }
        fn verify(
            &mut self,
            pr: u64,
            at: &crate::integration::EvaluatedAgainst,
            method: &str,
        ) -> Result<drain::Landed, drain::NotLanded> {
            self.0.verify(pr, at, method)
        }
        fn refresh_branch(&mut self, _: &PullRequestAssessment, _: &str) -> Result<String, String> {
            Err("the push was refused: ! [rejected] feature/1 (stale info)".into())
        }
        fn close(&mut self, pr: u64, head: &str, comment: &str) -> Result<(), String> {
            self.0.close(pr, head, comment)
        }
    }
    let root = scratch();
    let mut w = Moved(repair_world());
    let r = repair::apply(&root, &mut w, &RepairTarget::Number(1)).unwrap();
    let RepairOutcome::Refused {
        refusal: RepairRefusal::ActFailed { class, .. },
    } = r.outcome
    else {
        panic!("{r:?}");
    };
    assert_eq!(class, drain::FailureClass::Stale);
    assert_eq!(
        trail_actions(&root),
        ["repair_selected", "repair_attempted", "repair_refused"]
    );
    assert_eq!(
        drain::events(&root)[2].class,
        Some(drain::FailureClass::Stale)
    );
}

#[test]
fn a_repair_dry_run_reads_the_recorded_observation_and_writes_nothing() {
    let dir = unique_temp("mj-integration-repair-plan");
    // nothing observed: no classification to decide from, said as status says it
    git(&dir, &["init", "-q", "-b", "master"]);
    let err = repair::plan(&dir, &RepairTarget::Number(1)).unwrap_err();
    assert!(err.contains("prs refresh"), "{err}");
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "-q", "-b", "feature/1"]);
    std::fs::write(dir.join("one.txt"), "one\n").unwrap();
    git(&dir, &["add", "one.txt"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    let head = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "-q", "master"]);
    git(
        &dir,
        &["commit", "-q", "--allow-empty", "-m", "master moves"],
    );
    let master = git(&dir, &["rev-parse", "HEAD"]);
    assert_ne!(master, base);
    observed(&dir, &master, vec![observed_pr(1, &head)]);
    let r = repair::plan(&dir, &RepairTarget::parse("feature/1")).unwrap();
    assert_eq!(outcome(&r), "would_repair", "{r:?}");
    assert_eq!(
        (
            r.dry_run,
            r.pr,
            r.master_sha.as_str(),
            r.observed_at.as_str()
        ),
        (true, Some(1), master.as_str(), "t0")
    );
    // a read: no trail line, no cache, no summary
    assert!(trail_actions(&dir).is_empty());
    assert!(!dir
        .join(".git/majordomus/integration/summary.json")
        .exists());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- a rank names its factors (WP15)

#[test]
fn a_pull_request_others_wait_for_ranks_first() {
    // #1 and #2 are equal in everything but #3, which declares it waits for #2
    let mut dependent = sim(3);
    dependent.depends_on = Some(2);
    let w = World {
        open: vec![sim(1), sim(2), dependent],
        ..Default::default()
    };
    let q = w.queue();
    assert_eq!(
        q.next_merge,
        Some(2),
        "{:?}",
        q.assessments.iter().map(|a| a.number).collect::<Vec<_>>()
    );
    let f = q.get(2).unwrap().rank_factors.as_ref().unwrap();
    assert_eq!(f.dependents, 1);
    assert_eq!(
        q.get(1).unwrap().rank_factors.as_ref().unwrap().dependents,
        0
    );
}

#[test]
fn a_smaller_change_ranks_before_a_larger_one_of_the_same_age() {
    let mut big = sim(1);
    big.paths = vec!["a.rs".into(), "b.rs".into(), "c.rs".into()];
    big.created = "2026-09-01T00:00:00Z".into();
    let mut small = sim(2);
    small.paths = vec!["d.rs".into()];
    small.created = "2026-09-01T00:00:00Z".into();
    let w = World {
        open: vec![big, small],
        ..Default::default()
    };
    let q = w.queue();
    assert_eq!(q.next_merge, Some(2));
    assert_eq!(
        q.get(1)
            .unwrap()
            .rank_factors
            .as_ref()
            .unwrap()
            .authored_paths,
        3
    );
}

#[test]
fn every_ranked_assessment_carries_the_factors_it_was_ranked_by() {
    let w = World {
        open: vec![sim(1), sim(2), sim(3)],
        ..Default::default()
    };
    let q = w.queue();
    let factors: Vec<&crate::integration::RankFactors> = q
        .assessments
        .iter()
        .map(|a| a.rank_factors.as_ref().expect("ranked"))
        .collect();
    // the factors are what ordered them: each pair is in the order its factors say
    for pair in factors.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(
            (
                a.lane as u8,
                a.disposition as u8,
                a.risk as u8,
                a.contention
            ) <= (
                b.lane as u8,
                b.disposition as u8,
                b.risk as u8,
                b.contention
            ),
            "{a:?} before {b:?}"
        );
    }
    assert_eq!(factors[0].number, q.assessments[0].number);
}

#[test]
fn only_a_closed_or_merged_pull_request_with_a_head_is_resolved() {
    use super::forge::resolved_of;
    use serde_json::json;
    for v in [
        json!({"state": "MERGED", "headRefOid": "h"}),
        json!({"number": "7", "state": "MERGED", "headRefOid": "h"}),
        json!({"number": 7, "headRefOid": "h"}),
        json!({"number": 7, "state": 1, "headRefOid": "h"}),
        json!({"number": 7, "state": "OPEN", "headRefOid": "h"}),
        json!({"number": 7, "state": "MERGED"}),
        json!({"number": 7, "state": "MERGED", "headRefOid": 1}),
        json!({"number": 7, "state": "CLOSED", "headRefOid": ""}),
    ] {
        assert!(resolved_of(&v).is_none(), "{v}");
    }
    // a view that answers with another pull request is no answer about the one named
    let mut one = observe_pr(&sim(1));
    one.body = "Superseded by #7".into();
    let resolved = super::forge::resolved_for(&[one], &json!([]), |_| {
        Some(json!({"number": 9, "state": "MERGED", "headRefOid": "h9"}))
    });
    assert!(resolved.is_empty(), "{resolved:?}");
    let (n, r) = resolved_of(&json!({"number": 7, "state": "CLOSED", "headRefOid": "h"})).unwrap();
    assert_eq!(
        (n, r.merged, r.head_sha.as_str(), r.body.as_str()),
        (7, false, "h", "")
    );
    // what a reading does not say fails closed: nobody's, from a fork, merged nowhere
    assert_eq!(
        (
            r.merge_commit.as_str(),
            r.author.as_str(),
            r.author_association.as_str(),
            r.cross_repository,
            r.base_ref.as_str()
        ),
        ("", "", "", true, "")
    );
}

// ---------------------------------------------------------------- what every path says

#[test]
fn a_ready_pull_request_on_an_observation_that_allows_no_merge_commit_is_not_merged() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        no_method_on: Some(2),
        ..Default::default()
    };
    let out = drain::step(&root, &mut w, false, false).unwrap();
    assert!(stale_with(&out, 1, "allows no merge commit"), "{out:?}");
    assert_eq!(w.merge_calls, 0);
}

#[test]
fn a_lost_answer_of_a_merge_that_landed_unproved_stops_the_drain() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        answer_lost_after_landing: BTreeSet::from([1]),
        verify_fails: true,
        ..Default::default()
    };
    let out = drain::step(&root, &mut w, false, false).unwrap();
    assert!(
        matches!(&out, DrainStepOutcome::VerificationFailed { pr: 1, .. }),
        "{out:?}"
    );
    assert_eq!(w.merge_calls, 1, "a lost answer is never asked again");
}

#[test]
fn a_merge_asked_for_and_never_ended_is_ended_first_by_what_landed() {
    for (how, ended) in [
        ("reconcile_landed", "merge_succeeded"),
        ("reconcile_never", "merge_failed"),
        ("reconcile_unproved", "verification_failed"),
    ] {
        let root = scratch();
        attempted(&root, 1);
        // what an observer recorded of #1 meanwhile ends nothing
        drain::record(
            &root,
            drain::IntegrationEvent {
                pr: Some(1),
                ..drain::IntegrationEvent::of(drain::IntegrationAction::LeftActionable)
            },
        )
        .unwrap();
        let mut w = asked_before(how);
        let out = drain::reconcile(&root, &mut w).unwrap();
        match how {
            "reconcile_landed" => assert!(
                matches!(&out, Some(DrainStepOutcome::Merged { pr: 1, .. })),
                "{out:?}"
            ),
            "reconcile_never" => assert!(
                matches!(
                    &out,
                    Some(DrainStepOutcome::MergeRefused {
                        pr: 1,
                        class: drain::FailureClass::Transient,
                        ..
                    })
                ),
                "{out:?}"
            ),
            _ => assert!(
                matches!(
                    &out,
                    Some(DrainStepOutcome::VerificationFailed { pr: 1, .. })
                ),
                "{out:?}"
            ),
        }
        assert_eq!(trail_actions(&root).last().map(String::as_str), Some(ended));
        // ended: nothing is left to reconcile, and the forge is not asked again
        let asked = w.verify_calls;
        assert!(drain::reconcile(&root, &mut w).unwrap().is_none());
        assert_eq!(w.verify_calls, asked);

        // an end the trail cannot record is an error, never an act taken silently
        let root = scratch();
        attempted(&root, 1);
        let mut w = asked_before(how);
        drain::FAIL_WRITE.with(|n| n.set(1));
        let err = drain::drain(&root, &mut w, 1, false, false);
        drain::FAIL_WRITE.with(|n| n.set(0));
        assert!(err.is_err(), "{how}: {err:?}");
    }
}

#[test]
fn an_attempt_the_trail_cannot_key_is_not_reconciled() {
    let root = scratch();
    drain::record(
        &root,
        drain::IntegrationEvent::of(drain::IntegrationAction::MergeAttempted),
    )
    .unwrap();
    let mut w = asked_before("reconcile_landed");
    assert!(drain::reconcile(&root, &mut w).unwrap().is_none());
    assert_eq!(w.verify_calls, 0);
}

#[test]
fn a_drain_after_a_merge_reconciles_nothing_and_merges_the_next() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        ..Default::default()
    };
    let first = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert_eq!(first.merged, [1]);
    // #2 contains the master before #1 landed; bringing it up to date is not allowed here
    w.open[0].contains = w.master;
    let second = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert_eq!(second.merged, [2], "{second:?}");
    assert_eq!(
        w.verify_calls, 2,
        "the first merge was verified once, not again"
    );
}

#[test]
fn a_stale_decision_is_not_the_end_of_a_drain_and_an_outage_on_a_merge_is() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        master_moves_on: Some(2),
        ..Default::default()
    };
    let r = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert!(
        matches!(r.steps[0], DrainStepOutcome::StaleDecision { pr: 1, .. }),
        "{r:?}"
    );
    assert!(
        r.steps.len() > 1,
        "the drain stopped on a stale decision: {r:?}"
    );

    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        answer_lost_before_landing: BTreeSet::from([1]),
        ..Default::default()
    };
    let r = drain::drain(&root, &mut w, 2, false, false).unwrap();
    assert!(r.stopped.contains("nothing further is attempted"), "{r:?}");
    assert_eq!(
        w.merge_calls, 1,
        "the forge was asked again during an outage"
    );
}

#[test]
fn a_verification_failure_that_names_no_pull_request_still_halts_every_drain() {
    let root = scratch();
    drain::record(
        &root,
        drain::IntegrationEvent {
            detail: "what landed is not known".into(),
            ..drain::IntegrationEvent::of(drain::IntegrationAction::VerificationFailed)
        },
    )
    .unwrap();
    let mut w = World {
        open: vec![sim(1)],
        ..Default::default()
    };
    let r = drain::drain(&root, &mut w, 1, false, false).unwrap();
    assert!(
        r.stopped.starts_with("a merge could not be verified"),
        "{r:?}"
    );
    assert!(matches!(
        r.steps[..],
        [DrainStepOutcome::Halted { pr: None, .. }]
    ));
    assert_eq!(w.merge_calls, 0);
}

#[test]
fn a_closure_decided_on_a_queue_that_changed_meanwhile_is_not_made() {
    for (what, said) in [
        (Meanwhile::Closes(1), "#1 is no longer open"),
        (Meanwhile::Labelled(1, "wip"), "now, not redundant"),
    ] {
        let root = scratch();
        let mut w = World {
            open: vec![sim(1)],
            merged: vec![99],
            ..Default::default()
        };
        w.open[0].redundant_after = Some(99);
        w.meanwhile.push((2, what));
        let items = drain::cleanup(&root, &mut w, true).unwrap();
        assert!(items[0].action.contains(said), "{items:?}");
        assert_eq!(w.close_calls, 0);
    }
    // the second observation fails: nothing is closed, and the cleanup says why
    let root = scratch();
    let mut w = World {
        open: vec![sim(1)],
        merged: vec![99],
        ..Default::default()
    };
    w.open[0].redundant_after = Some(99);
    w.meanwhile
        .push((2, Meanwhile::Outage("HTTP 401: Bad credentials")));
    let err = drain::cleanup(&root, &mut w, true).unwrap_err();
    assert!(err.contains("Bad credentials"), "{err}");
    assert_eq!(w.close_calls, 0);
}

#[test]
fn every_failure_is_classed_from_the_forges_own_words() {
    use drain::FailureClass as C;
    for (said, class) in [
        (
            "Post https://api.github.com/graphql: timed out",
            C::Transient,
        ),
        ("Pull request is not mergeable: merge conflict", C::Conflict),
        (
            "Head branch was modified. Review and try the merge again.",
            C::Stale,
        ),
        ("At least 1 approving review is required", C::ReviewRevoked),
        (
            "Required status check \"ci\" is failing",
            C::NewFailingCheck,
        ),
        (
            "2 of 3 required status checks are expected",
            C::NewFailingCheck,
        ),
        (
            "the base branch policy prohibits the merge",
            C::PolicyViolation,
        ),
    ] {
        assert_eq!(C::of_merge_refusal(said), class, "{said}");
    }
    for (said, class) in [
        (
            "fatal: unable to access: Could not resolve host: github.com",
            C::Transient,
        ),
        ("the merge of master conflicts after all", C::Conflict),
        ("! [rejected] HEAD -> feature/x (fetch first)", C::Stale),
        (
            "Updates were rejected because the tip is behind (non-fast-forward)",
            C::Stale,
        ),
        ("hint: updates were rejected", C::Stale),
        ("scripts/derive failed: exit 1", C::PolicyViolation),
        // the repair push's listing of the branch: an outage is transient, a branch that moved
        // is stale
        (
            "the branch could not be listed: git ls-remote origin refs/heads/f: fatal: unable to access: Could not resolve host: github.com",
            C::Transient,
        ),
        (
            "the branch moved since it was observed: nothing is not abc; nothing was pushed",
            C::Stale,
        ),
    ] {
        assert_eq!(C::of_refresh_failure(said), class, "{said}");
    }
    assert_eq!(C::Unreadable.to_string(), "unreadable");
    assert!(!C::Unreadable.recoverable());
}

#[test]
fn a_timestamp_with_any_field_that_is_not_a_number_is_not_read() {
    use crate::peers::parse_rfc3339;
    assert!(parse_rfc3339("2026-10-05T00:00:00Z").is_some());
    for bad in [
        "x026-10-05T00:00:00Z",
        "2026-x0-05T00:00:00Z",
        "2026-10-x5T00:00:00Z",
        "2026-10-05Tx0:00:00Z",
        "2026-10-05T00:x0:00Z",
        "2026-10-05T00:00:x0Z",
        // twenty bytes, but the year's end splits a character
        "202\u{e9}10-05T00:00:00Z",
    ] {
        assert_eq!(bad.len(), 20, "{bad}");
        assert_eq!(parse_rfc3339(bad), None, "{bad}");
    }
}

// ---------------------------------------------------------------- the lease's own writes

#[test]
fn a_lease_whose_path_cannot_be_opened_is_not_taken() {
    let root = scratch();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    std::fs::create_dir_all(&path).unwrap();
    let Err(err) = drain::IntegrationLease::acquire(&root, "master") else {
        panic!("a lease was taken on a path that is a directory");
    };
    assert!(err.contains(&path.display().to_string()), "{err}");
    assert!(drain::events(&root).is_empty());
}

#[test]
fn a_lease_whose_record_cannot_be_written_is_given_back_and_its_renewal_refused() {
    let root = scratch();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    drain::FAIL_WRITE.with(|n| n.set(1));
    let Err(err) = drain::IntegrationLease::acquire(&root, "master") else {
        panic!("a lease was held with no holder record");
    };
    assert!(err.contains("holder record could not be written"), "{err}");
    assert!(!path.exists(), "the lease was not given back");
    assert!(drain::events(&root).is_empty());

    let lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
    drain::FAIL_WRITE.with(|n| n.set(1));
    let err = lease.renew().unwrap_err();
    assert!(err.contains(&path.display().to_string()), "{err}");
    lease.renew().expect("the next renewal writes");
}

#[test]
fn a_kept_alive_lease_renews_on_its_interval_until_dropped() {
    let root = scratch();
    let lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
    // the default interval outlasts this test: taken and dropped, it renews nothing
    drop(lease.keep_alive());
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    let modified = || std::fs::metadata(&path).unwrap().modified().unwrap();
    let before = modified();
    let alive = lease.keep_alive_every(std::time::Duration::from_millis(400));
    std::thread::sleep(std::time::Duration::from_millis(1100));
    drop(alive);
    assert!(
        modified() > before,
        "an interval longer than the poll renewed nothing"
    );
}

#[test]
fn an_executor_whose_lease_was_lost_observes_nothing() {
    let root = scratch();
    let lease = drain::IntegrationLease::acquire(&root, "master").unwrap();
    let path = drain::IntegrationLease::path_for(&root.join(".git"), "master");
    std::fs::write(&path, "another holder").unwrap();
    let mut forge = drain::ForgeIntegrator {
        root: &root,
        lease: Some(&lease),
    };
    let Err(err) = forge.observe() else {
        panic!("an executor without its lease observed");
    };
    assert!(err.contains("lease was lost"), "{err}");
}

// ---------------------------------------------------------------- what landed, from git

#[test]
fn what_landed_is_proved_from_masters_first_parent_line() {
    let (dir, base) = repository_with(&[("a.txt", "a\n")]);
    let decided = commit_on(&dir, "decided", &base, &[("d.txt", "d\n")]);
    let other = commit_on(&dir, "other", &base, &[("o.txt", "o\n")]);
    let at = |master: &str| crate::integration::EvaluatedAgainst {
        master_sha: master.into(),
        head_sha: "h".into(),
        observed_at: "t".into(),
    };
    let err = drain::landing(&dir, &at(&decided), "merge", &other).unwrap_err();
    assert!(err.contains("does not contain"), "{err}");
    // a rebase leaves no single commit to name
    assert_eq!(drain::landing(&dir, &at(&base), "rebase", &other), Ok(None));
    // the decision's master is on master only through a second parent
    git(&dir, &["checkout", "-q", &other]);
    git(
        &dir,
        &["merge", "-q", "--no-ff", "-m", "merge decided", &decided],
    );
    let merged = git(&dir, &["rev-parse", "HEAD"]);
    let err = drain::landing(&dir, &at(&decided), "merge", &merged).unwrap_err();
    assert!(err.contains("not on master's first-parent line"), "{err}");
    // a squash names its one commit, whose only parent is the decision's master
    let squashed = commit_on(&dir, "squashed", &decided, &[("s.txt", "s\n")]);
    assert_eq!(
        drain::landing(&dir, &at(&decided), "squash", &squashed),
        Ok(Some(squashed.clone()))
    );
}

#[test]
fn a_drain_ends_on_every_outcome_but_progress_and_a_candidates_own_refusal() {
    use drain::{ends_drain, FailureClass as C, IntegrationAction};
    use DrainStepOutcome as O;
    let refused = |class| O::MergeRefused {
        pr: 1,
        reason: "said".into(),
        class,
    };
    let refresh_failed = |class| O::RefreshFailed {
        pr: 1,
        reason: "said".into(),
        class,
    };
    for goes_on in [
        O::Merged {
            pr: 1,
            master_before: "a".into(),
            master_after: "b".into(),
        },
        O::StaleDecision {
            pr: 1,
            what: "master moved".into(),
        },
        refused(C::PolicyViolation),
        refresh_failed(C::Conflict),
    ] {
        assert_eq!(ends_drain(&goes_on), None, "{goes_on:?}");
    }
    for ends in [
        O::Idle { why: "none".into() },
        O::WouldMerge { pr: 1 },
        refused(C::Transient),
        refused(C::Unreadable),
        O::WouldRefresh { pr: 1 },
        O::Refreshed {
            pr: 1,
            head_before: "a".into(),
            head_after: "b".into(),
        },
        O::AwaitingChecks { pr: 1 },
        refresh_failed(C::Unreadable),
        O::VerificationFailed {
            pr: 1,
            reason: "unproved".into(),
        },
        O::TrailUnwritable {
            pr: 1,
            unrecorded: IntegrationAction::MergeAttempted,
            reason: "read-only".into(),
        },
        O::Halted {
            pr: None,
            reason: "a person looks first".into(),
        },
    ] {
        assert!(ends_drain(&ends).is_some(), "{ends:?}");
    }
}

#[test]
fn a_required_check_without_a_context_requires_nothing() {
    use super::forge::{protection_of, rules_of};
    use serde_json::json;
    let (checks, _) = rules_of(&json!([{
        "type": "required_status_checks",
        "parameters": {"required_status_checks": [{"integration_id": 1}, {"context": "ci"}]}
    }]));
    assert_eq!(checks, vec![super::RequiredCheck::from("ci")]);
    let (checks, _) = protection_of(&json!({
        "required_status_checks": {
            "checks": [{"app_id": 1}, {"context": 5}, {"context": "ci", "app_id": 2}]
        }
    }));
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].context, "ci");
    assert_eq!(checks[0].app_id, Some(2));
}

/// A head `prs repair` pushed holds the refresh pipeline exactly as one a drain refreshed:
/// master was brought in and its required check now runs, so refreshing another pull request
/// now would put this one behind again the moment either merged and waste the run.
#[test]
fn a_repaired_pull_request_waiting_for_checks_holds_the_pipeline() {
    let root = scratch();
    let mut w = World {
        open: vec![sim(1), sim(2)],
        master: 1,
        ..Default::default()
    };
    // #1 is brought up to date by `prs repair`, not by a drain's refresh
    let r = repair::apply(&root, &mut w, &RepairTarget::Number(1)).unwrap();
    assert!(matches!(r.outcome, RepairOutcome::Repaired { .. }), "{r:?}");
    w.open[0].ci_pending = true;
    assert_eq!(
        disposition(&w.queue(), 1),
        PullRequestDisposition::WaitingForChecks
    );
    let report = drain::drain(&root, &mut w, 1, false, true).unwrap();
    assert_eq!(
        report.steps,
        vec![DrainStepOutcome::AwaitingChecks { pr: 1 }]
    );
    assert_eq!(
        w.open[1].contains, 0,
        "#2 was refreshed while #1's repaired head waits for its checks"
    );
}

/// Every way a repair is refused has the class the trail records it with and a sentence that
/// says what a person does about it.
#[test]
fn every_repair_refusal_has_its_class_and_its_sentence() {
    use drain::FailureClass as C;
    let cases = [
        (
            RepairRefusal::NotOpen {
                target: "#9".into(),
            },
            C::PolicyViolation,
            "#9 is not an open pull request",
        ),
        (
            RepairRefusal::OtherBase {
                base: "release".into(),
            },
            C::PolicyViolation,
            "it targets release, not the integration base",
        ),
        (
            RepairRefusal::ForkHead,
            C::PolicyViolation,
            "lives in a fork",
        ),
        (
            RepairRefusal::AutoMergeArmed,
            C::PolicyViolation,
            "a person disarms it first",
        ),
        (
            RepairRefusal::AuthoredConflict {
                paths: vec!["src/a.rs".into()],
            },
            C::Conflict,
            "conflicts on 1 authored file(s), which are the owner's to settle: src/a.rs",
        ),
        (
            RepairRefusal::Undecidable {
                reason: "git merge-tree failed".into(),
            },
            C::Unreadable,
            "could not decide its relation to master: git merge-tree failed",
        ),
        (
            RepairRefusal::ActFailed {
                reason: "scripts/derive failed".into(),
                class: C::PolicyViolation,
            },
            C::PolicyViolation,
            "nothing reached the branch: scripts/derive failed",
        ),
        (
            RepairRefusal::TrailUnwritable {
                reason: "disk full".into(),
            },
            C::Unreadable,
            "the act was not taken: the trail could not record it first: disk full",
        ),
    ];
    for (refusal, class, said) in cases {
        assert_eq!(refusal.class(), class, "{refusal:?}");
        assert!(refusal.to_string().contains(said), "{refusal}");
    }
}

/// The decision reads the relation git gave and the disposition the classifier gave, and
/// nothing else: each relation a head can have to master maps onto one answer.
#[test]
fn a_repair_decides_on_each_relation_and_disposition_the_queue_holds() {
    let q = repair_world().queue();
    let say = |relation: RelationToMaster,
               disposition: Option<(PullRequestDisposition, Option<u64>)>| {
        let mut q = q.clone();
        let a = q
            .assessments
            .iter_mut()
            .find(|a| a.number == 1)
            .expect("#1 is open");
        a.relation = relation;
        if let Some((d, by)) = disposition {
            a.disposition = d;
            a.superseded_by = by;
        }
        let r = repair::dry_run(&q, &RepairTarget::Number(1));
        let why = match &r.outcome {
            RepairOutcome::NothingToRepair { why } => why.clone(),
            _ => String::new(),
        };
        (outcome(&r), why)
    };
    let behind = || RelationToMaster::Behind {
        behind: 1,
        authored: vec![],
    };
    let (o, why) = say(
        behind(),
        Some((PullRequestDisposition::Superseded, Some(9))),
    );
    assert_eq!(o, "nothing");
    assert!(why.contains("a successor, #9, landed"), "{why}");
    let (o, why) = say(behind(), Some((PullRequestDisposition::Superseded, None)));
    assert_eq!(o, "nothing");
    assert!(why.contains("a successor landed"), "{why}");
    let (o, _) = say(
        RelationToMaster::Unknown {
            reason: "git merge-tree failed".into(),
        },
        None,
    );
    assert_eq!(o, "refused:undecidable");
    for on_master in [
        RelationToMaster::Contained,
        RelationToMaster::Superseded,
        RelationToMaster::PatchIdsUpstream { commits: 2 },
    ] {
        let (o, why) = say(on_master.clone(), None);
        assert_eq!(o, "nothing", "{on_master:?}");
        assert!(why.contains("cleanup is the lane"), "{on_master:?}: {why}");
    }
    let (o, why) = say(
        RelationToMaster::DerivedOnly {
            paths: vec!["docs/generated/x.json".into()],
        },
        None,
    );
    assert_eq!(o, "nothing");
    assert!(why.contains("possibly_redundant"), "{why}");
}

/// Every write the act makes can fail, and the forge's observation can: each is said, and a
/// push the trail did not announce first is never made.
#[test]
fn an_applied_repair_says_each_write_it_could_not_make() {
    let apply = |w: &mut dyn Integrator, n: u64, failing_write: usize| {
        let root = scratch();
        drain::FAIL_WRITE.with(|k| k.set(failing_write));
        let r = repair::apply(&root, w, &RepairTarget::Number(n));
        drain::FAIL_WRITE.with(|k| k.set(0));
        r
    };
    // the forge could not be observed: nothing is decided, nothing is written
    let mut w = repair_world();
    w.outages = vec!["HTTP 502: Bad Gateway".into()];
    assert_eq!(apply(&mut w, 1, 0).unwrap_err(), "HTTP 502: Bad Gateway");
    // a refusal the trail cannot record is an error, not a silent refusal
    let mut w = repair_world();
    assert!(apply(&mut w, 2, 1).unwrap_err().contains("trail refused"));
    // the selection cannot be recorded: the act is not taken
    let mut w = repair_world();
    let r = apply(&mut w, 1, 1).unwrap();
    assert_eq!(outcome(&r), "refused:trail_unwritable", "{r:?}");
    assert_eq!(w.refresh_calls, 0, "a push reached the remote unrecorded");
    // the push was made and its outcome cannot be recorded: said as an error, after the act
    let mut w = repair_world();
    assert!(apply(&mut w, 1, 3).unwrap_err().contains("trail refused"));
    assert_eq!(w.refresh_calls, 1);
    // the act failed and its refusal cannot be recorded
    let mut w = repair_world();
    w.refresh_fails = true;
    assert!(apply(&mut w, 1, 3).unwrap_err().contains("trail refused"));
}

/// Write `record` where `prs refresh` records the observation of `root`.
fn recorded_observation(root: &std::path::Path, record: &serde_json::Value) {
    let path = crate::integration::state_path(root, crate::integration::OBSERVATION_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_string_pretty(record).unwrap()).unwrap();
}

#[test]
fn an_observation_of_another_schema_is_refused_older_or_newer() {
    let root = scratch();
    let w = World::default();
    let current = serde_json::to_value(w.observation()).unwrap();
    assert_eq!(current["schema"], OBSERVATION_SCHEMA);
    recorded_observation(&root, &current);
    assert_eq!(
        crate::integration::load_observation(&root).unwrap(),
        Some(w.observation())
    );
    // 3 is what 0.14.0 recorded; 5 stands for a newer executable's record met by this one,
    // as this one's record is met by 0.14.0. Neither is read as empty, and neither as current
    for other in [OBSERVATION_SCHEMA - 1, OBSERVATION_SCHEMA + 1] {
        let mut record = current.clone();
        record["schema"] = serde_json::json!(other);
        // a newer record may hold what this one cannot parse: its number is read first
        record["pull_requests"] = serde_json::json!("whatever schema it is, not this one's");
        recorded_observation(&root, &record);
        let refused = crate::integration::load_observation(&root).unwrap_err();
        assert!(
            refused.contains(&format!("schema {other}, this reads {OBSERVATION_SCHEMA}")),
            "{refused}"
        );
        assert!(refused.contains("majordomus prs refresh"), "{refused}");
        let queue = crate::integration::queue_of(&root).unwrap_err();
        assert_eq!(queue, refused, "no queue is built from it");
    }
    // a record that names no schema, or is not JSON, is no observation either
    let mut unnamed = current.clone();
    unnamed.as_object_mut().unwrap().remove("schema");
    recorded_observation(&root, &unnamed);
    let refused = crate::integration::load_observation(&root).unwrap_err();
    assert!(refused.contains("missing field `schema`"), "{refused}");
    recorded_observation(&root, &serde_json::json!("not an observation"));
    assert!(crate::integration::load_observation(&root).is_err());
    let path = crate::integration::state_path(&root, crate::integration::OBSERVATION_FILE);
    std::fs::write(&path, "{").unwrap();
    assert!(crate::integration::load_observation(&root).is_err());
}

/// The trail lines 0.14.0 wrote, one per shape it had: every field it could write, and the
/// fields it left out when empty.
const TRAIL_OF_0_14_0: &str = r##"{"at":"2026-10-06T10:00:00Z","actor":"t <t@example.com> (pid 41 on h)","action":"selected","pr":1,"master_before":"m0","head_sha":"h1","master_after":null,"reasons":["contains_master"],"detail":"#1 is the next merge","passed_over":[2,3]}
{"at":"2026-10-06T10:00:01Z","actor":"t <t@example.com> (pid 41 on h)","action":"merge_attempted","pr":1,"master_before":"m0","head_sha":"h1","master_after":null,"reasons":["contains_master"],"detail":"gh pr merge 1","evidence":[{"kind":"relation_to_master","status":"up_to_date","detail":"contains master m0","source":{"kind":"git","master_sha":"m0","head_sha":"h1"}},{"kind":"required_checks","status":"passed","detail":"ci","source":{"kind":"forge","observed_at":"2026-10-06T09:59:00Z"}}]}
{"at":"2026-10-06T10:00:09Z","actor":"t <t@example.com> (pid 41 on h)","action":"merge_succeeded","pr":1,"master_before":"m0","head_sha":"h1","master_after":"m1","reasons":["contains_master"],"detail":"master is m1","evidence":[{"kind":"supersession","status":"none","detail":"no successor is declared"}],"merge_commit":"m1"}
{"at":"2026-10-06T10:01:00Z","actor":"t <t@example.com> (pid 41 on h)","action":"refreshed","pr":2,"master_before":"m1","head_sha":"h2","master_after":null,"reasons":["behind_master:1"],"detail":"pushed","head_after":"h2b"}
{"at":"2026-10-06T10:02:00Z","actor":"t <t@example.com> (pid 41 on h)","action":"merge_failed","pr":3,"master_before":"m1","head_sha":"h3","master_after":null,"reasons":["contains_master"],"detail":"head moved","class":"stale"}
{"at":"2026-10-06T10:03:00Z","actor":"t <t@example.com> (pid 41 on h)","action":"closed_superseded","pr":4,"master_before":"m1","head_sha":"h4","master_after":null,"reasons":["superseded_by:#1","successor_not_landed:#9"],"detail":"closed"}
{"at":"2026-10-06T10:04:00Z","actor":"t <t@example.com> (pid 41 on h)","action":"idle","pr":null,"master_before":"m1","head_sha":null,"master_after":null,"reasons":[],"detail":"nothing to do"}
"##;

#[test]
fn the_trail_0_14_0_wrote_is_still_read_and_no_event_needs_a_new_field() {
    use crate::integration::ReasonCode;
    let root = scratch();
    let trail = trail_of(&root);
    std::fs::create_dir_all(trail.parent().unwrap()).unwrap();
    std::fs::write(&trail, TRAIL_OF_0_14_0).unwrap();
    let events = drain::events(&root);
    assert_eq!(
        trail_actions(&root),
        [
            "selected",
            "merge_attempted",
            "merge_succeeded",
            "refreshed",
            "merge_failed",
            "closed_superseded",
            "idle"
        ],
        "every line 0.14.0 wrote is an event"
    );
    // the verified merge a continuous drain counts is read with what it carried
    let merged = &events[2];
    assert_eq!(
        (merged.pr, merged.merge_commit.as_deref()),
        (Some(1), Some("m1"))
    );
    assert_eq!(merged.reasons, [ReasonCode::ContainsMaster]);
    assert_eq!(events[0].passed_over, [2, 3]);
    assert_eq!(events[1].evidence.len(), 2);
    assert_eq!(events[3].head_after.as_deref(), Some("h2b"));
    assert_eq!(
        events[5].reasons[0].to_string(),
        "superseded_by:#1",
        "a reason it wrote reads back verbatim"
    );
    // read and written again, each line is the line it was: nothing is added to an old event
    for (line, event) in TRAIL_OF_0_14_0.lines().zip(&events) {
        let was: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(serde_json::to_value(event).unwrap(), was, "{line}");
    }

    // an event that carries the reason this version added has the fields 0.14.0's had, and
    // the reason is a string: 0.14.0 reads a code it does not know as an unrecognised one,
    // verbatim, as this reader reads one it does not know
    let known: BTreeSet<&str> = [
        "at",
        "actor",
        "action",
        "pr",
        "master_before",
        "head_sha",
        "master_after",
        "reasons",
        "detail",
        "passed_over",
        "head_after",
        "evidence",
        "class",
        "merge_commit",
    ]
    .into_iter()
    .collect();
    let mut newer = events[1].clone();
    newer.reasons = vec![ReasonCode::DeclarationsUnread];
    newer.evidence[0].kind = crate::integration::EvidenceKind::Supersession;
    newer.evidence[0].status = "possible_supersession".into();
    let wire = serde_json::to_value(&newer).unwrap();
    let fields: BTreeSet<&str> = wire
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert!(fields.is_subset(&known), "{fields:?}");
    assert_eq!(wire["reasons"], serde_json::json!(["declarations_unread"]));
    let evidence: BTreeSet<&str> = wire["evidence"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        evidence.into_iter().collect::<Vec<_>>(),
        ["detail", "kind", "source", "status"]
    );
    let unknown: ReasonCode = serde_json::from_value(serde_json::json!("a_later_code:#7")).unwrap();
    assert_eq!(unknown, ReasonCode::Unrecognised("a_later_code:#7".into()));
    assert_eq!(serde_json::to_value(&unknown).unwrap(), "a_later_code:#7");
}
