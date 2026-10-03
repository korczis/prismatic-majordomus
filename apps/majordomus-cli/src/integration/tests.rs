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
            reviews_required: self.reviews_required,
            merge_methods: vec!["merge".into()],
            pull_requests: self.open.iter().map(observe_pr).collect(),
        }
    }

    fn relation(&self, n: u64) -> RelationToMaster {
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

    fn happen(&mut self, what: &Meanwhile) {
        match what {
            Meanwhile::HeadMoves(n) => {
                let s = self.open.iter_mut().find(|s| s.number == *n).expect("open");
                s.head = format!("{}+", s.head);
            }
            Meanwhile::Closes(n) => self.open.retain(|s| s.number != *n),
            Meanwhile::Labelled(n, label) => {
                let s = self.open.iter_mut().find(|s| s.number == *n).expect("open");
                s.labels.push((*label).to_string());
            }
            Meanwhile::Reviewed(n, decision) => {
                let s = self.open.iter_mut().find(|s| s.number == *n).expect("open");
                s.review = decision;
            }
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
            .unwrap_or_default(),
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
            }]
        },
        review_decision: s.review.into(),
        auto_merge: false,
        cross_repository: s.cross_repository,
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
        Ok(self.queue())
    }

    fn merge(&mut self, pr: u64, head_sha: &str, method: &str) -> Result<(), String> {
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
        assert_eq!(
            self.open[i].contains, self.master,
            "merged a head that does not contain the current master"
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
        self.open.remove(i);
        self.merged.push(pr);
        self.master += 1;
        self.merged_after_observation.push(self.observations);
        Ok(())
    }

    fn verify(&mut self, pr: u64, _head_sha: &str) -> Result<String, String> {
        assert!(self.merged.contains(&pr));
        if self.verify_fails {
            return Err(format!("the forge does not show #{pr} merged"));
        }
        Ok(master_sha(self.master))
    }

    fn refresh_branch(&mut self, a: &PullRequestAssessment, _base: &str) -> Result<String, String> {
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
}

fn scratch() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "mj-integration-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
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
    assert_eq!(disposition(&q, 6), PullRequestDisposition::Superseded);
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
                }]
            })
            .unwrap_or_default();
        // a green check that is not the required one proves nothing
        obs.pull_requests[0].checks.push(CheckObservation {
            name: "suite".into(),
            state: CheckRunState::Passed,
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
fn only_derived_differences_are_never_called_superseded() {
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
    assert!(events.iter().any(|e| e.action == "stale_decision"));
    assert!(!events.iter().any(|e| e.action == "merge_attempted"));
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
    let actions = drain::events(&root).into_iter().map(|e| e.action).collect();
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
        matches!(&out, DrainStepOutcome::MergeRefused { pr: 1, reason } if reason.contains("not mergeable")),
        "{out:?}"
    );
    assert!(w.merged.is_empty());
    let actions: Vec<String> = drain::events(&root).into_iter().map(|e| e.action).collect();
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
    let actions: Vec<String> = drain::events(&root).into_iter().map(|e| e.action).collect();
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
    assert_eq!(disposition(&q1, 5), PullRequestDisposition::Superseded);
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
    let actions: Vec<String> = drain::events(&root).into_iter().map(|e| e.action).collect();
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

#[test]
fn a_refreshed_pull_request_waiting_for_checks_holds_the_pipeline() {
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
        "#2 was refreshed while the executor waits for #1's checks"
    );
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
        fn verify(&mut self, _: u64, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn refresh_branch(&mut self, _: &PullRequestAssessment, _: &str) -> Result<String, String> {
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
        let before_age = |a: &PullRequestAssessment| {
            let contention = a.overlaps.iter().filter(|o| contenders.contains(&o.number)).count();
            (a.lane as u8, a.disposition as u8, a.risk as u8, contention)
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
    // only derived output differs: never superseded, only possibly redundant
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
        reviews_required: Some(false),
        merge_methods: vec!["merge".into()],
        pull_requests: prs,
    };
    crate::integration::store_observation(dir, &obs).unwrap();
}

fn observed_pr(number: u64, head_sha: &str) -> PullRequestObservation {
    let mut p = observe_pr(&sim(number));
    p.head_sha = head_sha.into();
    p
}

/// The keys of the relation cache `queue_of` left in `dir`.
fn cached_keys(dir: &std::path::Path) -> Vec<String> {
    let path = crate::integration::state_path(dir, crate::integration::RELATIONS_FILE);
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| "{}".into());
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
    let q = crate::integration::queue_of(&dir).unwrap();
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
    let q = crate::integration::queue_of(&dir).unwrap();
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
    let selected = trail.iter().find(|e| e.action == "selected").unwrap();
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
    wait::record_transitions(&root, &w.queue());
    assert!(wait::waits(&drain::events(&root)).contains_key(&5));
    // its check fails on the next look: it needs repair, a person's work, not the executor's
    w.open[0].failing = true;
    assert_eq!(wait::record_transitions(&root, &w.queue()), 1);
    let trail = drain::events(&root);
    let left = trail.last().unwrap();
    assert_eq!(left.action, wait::LEFT_ACTIONABLE);
    assert_eq!(left.detail, "it is needs_repair now");
    assert!(wait::waits(&trail).is_empty());
    // and nothing is recorded when nothing changed
    assert_eq!(wait::record_transitions(&root, &w.queue()), 0);
}

#[test]
fn starvation_is_visible_and_changes_no_rank() {
    let root = scratch();
    let w = World {
        open: vec![sim(1), sim(2)],
        ..Default::default()
    };
    let q = w.queue();
    wait::record_transitions(&root, &q);
    for _ in 0..wait::STARVING_AFTER {
        drain::record(
            &root,
            drain::IntegrationEvent {
                at: String::new(),
                actor: String::new(),
                action: "selected".into(),
                pr: Some(1),
                master_before: None,
                head_sha: None,
                master_after: None,
                reasons: Vec::new(),
                detail: String::new(),
                passed_over: vec![2],
                head_after: None,
            },
        );
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
        fn verify(&mut self, _: u64, _: &str) -> Result<String, String> {
            unreachable!()
        }
        fn refresh_branch(&mut self, _: &PullRequestAssessment, _: &str) -> Result<String, String> {
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
