//! How long CI takes, measured from the recorded forge observation: every check run the forge
//! reported on an open pull request's head, with when it started and when it completed.
//!
//! Nothing here asks the forge or estimates: a duration is `completed_at - started_at` of a run
//! that has both, and a run without either is counted as unmeasured, never as zero. The
//! observation holds the latest run of each check on each open head, so this is what CI took
//! on the heads now open, not a history; the history of merges is `integration::metrics`.
//!
//! Each head also names its critical path, the chain of checks that bounded its wall time,
//! and how unevenly each matrix family (`suite-shard (1)` .. `suite-shard (4)`) was dealt.
//! The observation holds no job graph, so the chain is inferred from the times alone: it ends
//! at the check that completed last, and each check's predecessor is the one that completed
//! last before it started. A check that happened to complete in that gap is read as the
//! predecessor; the check of the chain that took longest is named whatever the order.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::forge::ForgeObservation;
use super::model::{CheckKind, CheckRunState};
use crate::order::{canonical, numeric, OrderKey, Ordered};
use crate::peers::parse_rfc3339;

/// How one check takes, across the open heads that ran it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckTiming {
    /// The check's name, as the forge reports it.
    pub name: String,
    /// Runs that started and completed, so were measured.
    pub runs: u64,
    /// Of those, how many passed.
    pub passed: u64,
    /// Of those, how many failed.
    pub failed: u64,
    /// The median duration, in seconds.
    pub median_seconds: u64,
    /// The 90th percentile duration (nearest rank), in seconds.
    pub p90_seconds: u64,
    /// The longest run, in seconds.
    pub slowest_seconds: u64,
}

/// The chain of checks that bounded one head's wall time, and the one of them that took
/// longest: the job a shorter run has to shorten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CriticalPath {
    /// The checks of the chain, first to last, each started after the one before completed.
    pub chain: Vec<String>,
    /// The check of the chain that took longest.
    pub check: String,
    /// How long it took, in seconds.
    pub seconds: u64,
    /// `seconds / wall_seconds` of the head, to two decimals; 0 for a wall time of 0 s.
    pub share: f64,
}

/// How unevenly one matrix family was dealt on a head: the checks named `<family> (<values>)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MatrixSpread {
    /// The family: the name before its matrix values, `suite-shard` of `suite-shard (2)`.
    pub family: String,
    /// How many of its checks were measured.
    pub shards: u64,
    /// The shard that took longest.
    pub slowest_shard: String,
    /// How long it took, in seconds.
    pub slowest_seconds: u64,
    /// How long the fastest shard took, in seconds.
    pub fastest_seconds: u64,
    /// `slowest_seconds / fastest_seconds`, to two decimals: 1 for an even deal; `None` when
    /// the fastest shard took 0 s, so there is nothing to divide by.
    pub spread: Option<f64>,
}

/// How long CI took on one open pull request's head: from its first check's start to its last
/// check's completion, and the check that took longest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HeadTiming {
    /// The pull request.
    pub number: u64,
    /// The head the checks ran on.
    pub head_sha: String,
    /// When its first measured check started, RFC 3339.
    pub started_at: String,
    /// When its last measured check completed, RFC 3339.
    pub completed_at: String,
    /// The wall time between them, in seconds: what a reader waited for CI on this head.
    pub wall_seconds: u64,
    /// The measured check that took longest.
    pub slowest_check: String,
    /// How long it took, in seconds.
    pub slowest_seconds: u64,
    /// Whether any measured check on the head failed.
    pub failed: bool,
    /// The chain of checks that bounded the wall time; `None` when every measured check on
    /// the head was skipped, so none ran.
    pub critical_path: Option<CriticalPath>,
    /// Every matrix family on the head, the most uneven deal first.
    pub matrices: Vec<MatrixSpread>,
}

/// CI timings over every open head the observation holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CiTimings {
    /// When the forge was observed, RFC 3339: the moment these timings describe.
    pub observed_at: String,
    /// Check runs measured, across every check and head.
    pub measured_runs: u64,
    /// Check runs the observation holds without both a start and a completion: running,
    /// queued, or recorded before starts were read. Counted, never measured as zero.
    pub unmeasured_runs: u64,
    /// Every check, the slowest median first.
    pub checks: Vec<CheckTiming>,
    /// Every head with a measured check, the longest wall time first.
    pub heads: Vec<HeadTiming>,
}

/// Checks are read slowest median first, then by name: the median is the rank, negated so
/// that the longest wait leads.
impl Ordered for CheckTiming {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.name, &self.name).ranked(slowest_first(self.median_seconds))
    }
}

/// Heads are read longest wall time first, then by head.
impl Ordered for HeadTiming {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.head_sha, &self.head_sha).ranked(slowest_first(self.wall_seconds))
    }
}

/// Matrix families are read most uneven first, then by family; a spread that could not be
/// divided reads last.
impl Ordered for MatrixSpread {
    fn order_key(&self) -> OrderKey<'_> {
        let hundredths = self.spread.map_or(0, |s| (s * 100.0).round() as u64);
        OrderKey::plain(&self.family, &self.family).ranked(slowest_first(hundredths))
    }
}

fn slowest_first(seconds: u64) -> i64 {
    i64::try_from(seconds).map_or(i64::MIN, |s| -s)
}

/// `n / d` to two decimals.
fn ratio(n: u64, d: u64) -> f64 {
    (n as f64 / d as f64 * 100.0).round() / 100.0
}

/// A check that ran on a head: its name, start and completion, in seconds since the epoch.
type Job<'a> = (&'a str, u64, u64);

/// The critical path of `jobs` over a wall time of `wall` seconds, inferred from the times:
/// from the check that completed last, back through the check that completed last before each
/// one started. Equal completions go to the longer check, then the later name, so the chain
/// is one and equal instants read in name order; the longest of equals is the earlier name.
fn critical_path(jobs: &[Job<'_>], wall: u64) -> Option<CriticalPath> {
    // the latest completion at or before `bound`, among the checks not yet on the chain
    let latest = |bound: u64, taken: &[bool]| {
        (0..jobs.len())
            .filter(|&i| !taken[i] && jobs[i].2 <= bound)
            .max_by(|&a, &b| {
                let (na, sa, ea) = jobs[a];
                let (nb, sb, eb) = jobs[b];
                (ea, ea - sa).cmp(&(eb, eb - sb)).then(na.cmp(nb))
            })
    };
    let mut taken = vec![false; jobs.len()];
    let mut chain = Vec::new();
    let mut at = latest(u64::MAX, &taken);
    while let Some(i) = at {
        taken[i] = true;
        chain.push(i);
        at = latest(jobs[i].1, &taken);
    }
    chain.reverse();
    let longest = chain.iter().copied().max_by(|&a, &b| {
        let (na, sa, ea) = jobs[a];
        let (nb, sb, eb) = jobs[b];
        (ea - sa).cmp(&(eb - sb)).then(nb.cmp(na))
    })?;
    let (name, start, end) = jobs[longest];
    let seconds = end - start;
    Some(CriticalPath {
        chain: chain.iter().map(|&i| jobs[i].0.to_string()).collect(),
        check: name.to_string(),
        seconds,
        share: if wall == 0 { 0.0 } else { ratio(seconds, wall) },
    })
}

/// The family of a matrix check, `suite-shard` of `suite-shard (2)`; `None` for a check that
/// carries no matrix values.
fn family(name: &str) -> Option<&str> {
    let open = name.strip_suffix(')')?.rfind(" (")?;
    Some(&name[..open]).filter(|f| !f.is_empty())
}

/// How unevenly each matrix family among `jobs` was dealt, the most uneven first.
fn matrices(jobs: &[Job<'_>]) -> Vec<MatrixSpread> {
    let mut by_family: BTreeMap<&str, Vec<(u64, &str)>> = BTreeMap::new();
    for &(name, start, end) in jobs {
        if let Some(f) = family(name) {
            by_family.entry(f).or_default().push((end - start, name));
        }
    }
    let mut spreads: Vec<MatrixSpread> = by_family
        .into_iter()
        .map(|(family, mut shards)| {
            // longest first, and of equal ones the earlier name
            shards.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
            let (slowest, name) = shards[0];
            let fastest = shards[shards.len() - 1].0;
            MatrixSpread {
                family: family.to_string(),
                shards: shards.len() as u64,
                slowest_shard: name.to_string(),
                slowest_seconds: slowest,
                fastest_seconds: fastest,
                spread: (fastest > 0).then(|| ratio(slowest, fastest)),
            }
        })
        .collect();
    canonical(&mut spreads);
    spreads
}

/// The value at the nearest rank `p` (0..=100) of `sorted`, which is ascending and not empty.
fn rank(sorted: &[u64], p: u64) -> u64 {
    let n = sorted.len() as u64;
    let i = (p * n).div_ceil(100).saturating_sub(1).min(n - 1);
    sorted[i as usize]
}

/// CI timings measured from `obs`.
///
/// ```text
/// // a run with a start and a completion is measured; one still running is counted only
/// let t = timings(&observation_with(&[("ci", "00:00:00", "00:10:00"), ("suite", "00:00:00", "")]));
/// assert_eq!((t.measured_runs, t.unmeasured_runs), (1, 1));
/// assert_eq!(t.checks[0].median_seconds, 600);
/// ```
pub fn timings(obs: &ForgeObservation) -> CiTimings {
    let mut by_check: BTreeMap<String, (Vec<u64>, u64, u64)> = BTreeMap::new();
    let mut heads = Vec::new();
    let mut unmeasured = 0u64;
    for pr in &obs.pull_requests {
        let mut first: Option<(u64, &str)> = None;
        let mut last: Option<(u64, &str)> = None;
        let mut slowest: Option<(u64, &str)> = None;
        let mut failed = false;
        // the checks that ran, for the critical path and the matrices: a skip ran nothing
        let mut jobs: Vec<Job<'_>> = Vec::new();
        for c in pr.checks.iter().filter(|c| c.kind == CheckKind::CheckRun) {
            let measured = parse_rfc3339(&c.started_at).zip(parse_rfc3339(&c.completed_at));
            let Some((start, end)) = measured.filter(|(s, e)| e >= s) else {
                unmeasured += 1;
                continue;
            };
            let took = end - start;
            if c.state != CheckRunState::Skipped {
                jobs.push((c.name.as_str(), start, end));
            }
            let entry = by_check.entry(c.name.clone()).or_default();
            entry.0.push(took);
            match c.state {
                CheckRunState::Passed => entry.1 += 1,
                CheckRunState::Failed => {
                    entry.2 += 1;
                    failed = true;
                }
                _ => {}
            }
            if first.is_none_or(|(s, _)| start < s) {
                first = Some((start, c.started_at.as_str()));
            }
            if last.is_none_or(|(e, _)| end > e) {
                last = Some((end, c.completed_at.as_str()));
            }
            if slowest.is_none_or(|(t, _)| took > t) {
                slowest = Some((took, c.name.as_str()));
            }
        }
        if let (Some((s, started)), Some((e, completed)), Some((took, name))) =
            (first, last, slowest)
        {
            heads.push(HeadTiming {
                number: pr.number,
                head_sha: pr.head_sha.clone(),
                started_at: started.to_string(),
                completed_at: completed.to_string(),
                wall_seconds: e - s,
                slowest_check: name.to_string(),
                slowest_seconds: took,
                failed,
                critical_path: critical_path(&jobs, e - s),
                matrices: matrices(&jobs),
            });
        }
    }
    let mut checks: Vec<CheckTiming> = by_check
        .into_iter()
        .map(|(name, (took, passed, failed))| {
            // a sample has one order, `order::numeric`'s; whole seconds are exact in an f64
            let mut sample: Vec<f64> = took.iter().map(|&t| t as f64).collect();
            numeric(&mut sample);
            let took: Vec<u64> = sample.into_iter().map(|t| t as u64).collect();
            CheckTiming {
                runs: took.len() as u64,
                passed,
                failed,
                median_seconds: rank(&took, 50),
                p90_seconds: rank(&took, 90),
                slowest_seconds: *took.last().unwrap_or(&0),
                name,
            }
        })
        .collect();
    canonical(&mut checks);
    canonical(&mut heads);
    CiTimings {
        observed_at: obs.observed_at.clone(),
        measured_runs: checks.iter().map(|c| c.runs).sum(),
        unmeasured_runs: unmeasured,
        checks,
        heads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::model::{CheckObservation, PullRequestObservation};

    fn check(name: &str, state: CheckRunState, started: &str, completed: &str) -> CheckObservation {
        CheckObservation {
            name: name.into(),
            state,
            kind: CheckKind::CheckRun,
            started_at: started.into(),
            completed_at: completed.into(),
            ..CheckObservation::default()
        }
    }

    fn pr(number: u64, checks: Vec<CheckObservation>) -> PullRequestObservation {
        let mut p: PullRequestObservation = serde_json::from_value(serde_json::json!({
            "number": number, "title": "t", "author": "a", "head_ref": "h", "head_sha": format!("{number:040}"),
            "base_ref": "master", "draft": false, "created_at": "2026-10-01T00:00:00Z",
            "updated_at": "2026-10-01T00:00:00Z"
        }))
        .expect("a pull request");
        p.checks = checks;
        p
    }

    fn observation(prs: Vec<PullRequestObservation>) -> ForgeObservation {
        let mut o: ForgeObservation = serde_json::from_value(serde_json::json!({
            "schema": crate::integration::OBSERVATION_SCHEMA, "repository": "o/r", "base": "master",
            "base_sha": "0".repeat(40), "observed_at": "2026-10-11T12:00:00Z",
            "required_checks": null, "review_policy": null, "merge_methods": ["merge"], "pull_requests": []
        }))
        .expect("an observation");
        o.pull_requests = prs;
        o
    }

    const T: &str = "2026-10-11T10:00:00Z";
    fn at(min: u64) -> String {
        let s = parse_rfc3339(T).unwrap() + min * 60;
        crate::peers::rfc3339(std::time::UNIX_EPOCH + std::time::Duration::from_secs(s))
    }

    #[test]
    fn a_check_is_measured_from_its_runs_and_the_slowest_median_comes_first() {
        let o = observation(vec![
            pr(
                1,
                vec![
                    check("rust", CheckRunState::Passed, &at(0), &at(10)),
                    check("site", CheckRunState::Passed, &at(0), &at(40)),
                ],
            ),
            pr(
                2,
                vec![
                    check("rust", CheckRunState::Failed, &at(5), &at(25)),
                    check("site", CheckRunState::Passed, &at(5), &at(35)),
                ],
            ),
            pr(
                3,
                vec![check("rust", CheckRunState::Passed, &at(0), &at(30))],
            ),
        ]);
        let t = timings(&o);
        assert_eq!(t.measured_runs, 5);
        assert_eq!(t.checks[0].name, "site", "the slowest median first");
        let rust = t.checks.iter().find(|c| c.name == "rust").unwrap();
        assert_eq!((rust.runs, rust.passed, rust.failed), (3, 2, 1));
        assert_eq!(rust.median_seconds, 20 * 60);
        assert_eq!(rust.p90_seconds, 30 * 60);
        assert_eq!(rust.slowest_seconds, 30 * 60);
    }

    #[test]
    fn a_head_waits_from_its_first_start_to_its_last_completion() {
        let o = observation(vec![pr(
            7,
            vec![
                check("rust", CheckRunState::Passed, &at(2), &at(12)),
                check("site", CheckRunState::Failed, &at(0), &at(41)),
            ],
        )]);
        let h = &timings(&o).heads[0];
        assert_eq!((h.number, h.wall_seconds), (7, 41 * 60));
        assert_eq!(
            (h.slowest_check.as_str(), h.slowest_seconds),
            ("site", 41 * 60)
        );
        assert!(h.failed);
        assert_eq!(
            (h.started_at.as_str(), h.completed_at.as_str()),
            (at(0).as_str(), at(41).as_str())
        );
    }

    #[test]
    fn a_run_without_a_start_or_an_end_is_counted_and_never_measured_as_zero() {
        let o = observation(vec![pr(
            9,
            vec![
                check("running", CheckRunState::Pending, &at(0), ""),
                check("old record", CheckRunState::Passed, "", &at(5)),
                check("backwards", CheckRunState::Passed, &at(9), &at(1)),
                CheckObservation {
                    name: "status".into(),
                    kind: CheckKind::StatusContext,
                    completed_at: at(3),
                    ..CheckObservation::default()
                },
            ],
        )]);
        let t = timings(&o);
        assert_eq!(
            (t.measured_runs, t.unmeasured_runs),
            (0, 3),
            "a status context is no run"
        );
        assert!(t.checks.is_empty() && t.heads.is_empty());
    }

    #[test]
    fn heads_lead_with_the_longest_wait_and_equal_medians_read_naturally() {
        let o = observation(vec![
            pr(
                1,
                vec![check("check-10", CheckRunState::Passed, &at(0), &at(5))],
            ),
            pr(
                2,
                vec![check("check-2", CheckRunState::Passed, &at(0), &at(5))],
            ),
            pr(
                3,
                vec![check("check-2", CheckRunState::Passed, &at(0), &at(20))],
            ),
        ]);
        let t = timings(&o);
        let heads: Vec<u64> = t.heads.iter().map(|h| h.number).collect();
        assert_eq!(heads[0], 3, "the longest wall time first: {heads:?}");
        let names: Vec<&str> = t.checks.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["check-2", "check-10"],
            "check-2's median is 5 min too"
        );
        assert!(crate::order::is_canonical(&t.checks) && crate::order::is_canonical(&t.heads));
    }

    /// A skipped run that started and completed is measured, and counted neither as passed
    /// nor as failed.
    #[test]
    fn a_skipped_run_is_measured_and_neither_passed_nor_failed() {
        let o = observation(vec![pr(
            11,
            vec![check("lint", CheckRunState::Skipped, &at(0), &at(1))],
        )]);
        let t = timings(&o);
        let lint = &t.checks[0];
        assert_eq!((lint.runs, lint.passed, lint.failed), (1, 0, 0));
        assert!(!t.heads[0].failed);
    }

    #[test]
    fn the_nearest_rank_of_one_value_is_that_value() {
        assert_eq!(rank(&[42], 50), 42);
        assert_eq!(rank(&[42], 90), 42);
        assert_eq!(rank(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 90), 9);
        assert_eq!(rank(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 50), 5);
    }

    /// A run that ends before and takes less than one already seen moves neither the head's
    /// last completion nor its slowest check.
    #[test]
    fn a_run_inside_another_moves_neither_the_end_nor_the_slowest() {
        let o = observation(vec![pr(
            8,
            vec![
                check("site", CheckRunState::Passed, &at(0), &at(40)),
                check("rust", CheckRunState::Passed, &at(5), &at(10)),
            ],
        )]);
        let h = &timings(&o).heads[0];
        assert_eq!(h.wall_seconds, 40 * 60);
        assert_eq!(h.completed_at, at(40));
        assert_eq!(
            (h.slowest_check.as_str(), h.slowest_seconds),
            ("site", 40 * 60)
        );
    }

    /// `T` plus `s` seconds.
    fn sec(s: u64) -> String {
        let s = parse_rfc3339(T).unwrap() + s;
        crate::peers::rfc3339(std::time::UNIX_EPOCH + std::time::Duration::from_secs(s))
    }

    /// The fourteen jobs of validate run 41932965548, attempt 1 (2026-11-05), each as long as
    /// it took there; the starts and the waits between them are laid out to its 156 min.
    fn run_41932965548() -> Vec<CheckObservation> {
        let ran = |name: &str, start: u64, took: u64| {
            check(name, CheckRunState::Passed, &sec(start), &sec(start + took))
        };
        vec![
            ran("plan", 0, 40),
            ran("suite-shard (1)", 100, 4004),
            ran("suite-shard (2)", 100, 7726),
            ran("suite-shard (3)", 100, 4800),
            ran("suite-shard (4)", 100, 5523),
            ran("suite", 8400, 30),
            ran("evidence", 8800, 560),
            ran("rust (1)", 100, 2100),
            ran("rust (2)", 100, 2400),
            ran("rust (3)", 100, 2733),
            ran("coverage", 100, 1975),
            ran("structure", 100, 600),
            ran("site", 100, 900),
            ran("ui", 100, 1600),
        ]
    }

    /// The hand diagnosis of 2026-11-05, read from the timings: suite-shard (2) bounded the
    /// run at 7726 s of its 9360, and the suite was dealt 1.93 times as long to its slowest
    /// shard as to its fastest.
    #[test]
    fn run_41932965548_is_bounded_by_its_second_suite_shard() {
        let t = timings(&observation(vec![pr(2900, run_41932965548())]));
        let h = &t.heads[0];
        assert_eq!(h.wall_seconds, 9360, "156 min");
        let path = h.critical_path.as_ref().expect("a critical path");
        assert_eq!(
            path.chain,
            ["plan", "suite-shard (2)", "suite", "evidence"],
            "back from the last completion through the check each waited for"
        );
        assert_eq!(
            (path.check.as_str(), path.seconds),
            ("suite-shard (2)", 7726)
        );
        assert_eq!(path.share, 0.83);
        let families: Vec<&str> = h.matrices.iter().map(|m| m.family.as_str()).collect();
        assert_eq!(
            families,
            ["suite-shard", "rust"],
            "the most uneven deal first"
        );
        let suite = &h.matrices[0];
        assert_eq!(
            (
                suite.shards,
                suite.slowest_shard.as_str(),
                suite.slowest_seconds
            ),
            (4, "suite-shard (2)", 7726)
        );
        assert_eq!((suite.fastest_seconds, suite.spread), (4004, Some(1.93)));
        assert_eq!(h.matrices[1].spread, Some(1.3));
        assert!(crate::order::is_canonical(&h.matrices));
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json["heads"][0]["critical_path"]["share"], 0.83);
        assert_eq!(json["heads"][0]["matrices"][0]["spread"], 1.93);
    }

    /// A head with no check run has no wait to bound, and nothing of it is reported.
    #[test]
    fn an_empty_run_has_no_head_and_no_critical_path() {
        let t = timings(&observation(vec![pr(1, vec![])]));
        assert!(t.heads.is_empty() && t.checks.is_empty());
        assert_eq!(critical_path(&[], 0), None);
        assert!(matrices(&[]).is_empty());
    }

    /// One job is the whole of its run, and a name without matrix values is no family.
    #[test]
    fn a_single_job_is_its_own_critical_path() {
        let t = timings(&observation(vec![pr(
            2,
            vec![check("build", CheckRunState::Failed, &at(0), &at(10))],
        )]));
        let h = &t.heads[0];
        let path = h.critical_path.as_ref().unwrap();
        assert_eq!(path.chain, ["build"]);
        assert_eq!(
            (path.check.as_str(), path.seconds, path.share),
            ("build", 600, 1.0)
        );
        assert!(h.matrices.is_empty());
    }

    /// A matrix of one shard is dealt evenly: its spread is 1.
    #[test]
    fn a_matrix_of_one_is_an_even_deal() {
        let t = timings(&observation(vec![pr(
            3,
            vec![check(
                "suite-shard (1)",
                CheckRunState::Passed,
                &at(0),
                &at(5),
            )],
        )]));
        let m = &t.heads[0].matrices;
        assert_eq!(m.len(), 1);
        assert_eq!(
            (m[0].family.as_str(), m[0].shards, m[0].spread),
            ("suite-shard", 1, Some(1.0))
        );
        assert_eq!((m[0].slowest_seconds, m[0].fastest_seconds), (300, 300));
    }

    /// A shard of 0 s cannot be divided by, so its family's spread is unknown and reads last;
    /// a head of 0 s gives its check no share; and a head whose checks were all skipped ran
    /// nothing, so it has no critical path.
    #[test]
    fn a_zero_is_never_divided_by_and_a_skip_is_no_critical_path() {
        let jobs: Vec<Job<'_>> = vec![
            ("lint (a)", 5, 5),
            ("lint (b)", 5, 5),
            ("rust (1)", 0, 4),
            ("rust (2)", 0, 2),
        ];
        let m = matrices(&jobs);
        let read: Vec<(&str, Option<f64>)> =
            m.iter().map(|m| (m.family.as_str(), m.spread)).collect();
        assert_eq!(read, [("rust", Some(2.0)), ("lint", None)]);
        let path = critical_path(&jobs[..2], 0).unwrap();
        assert_eq!(path.share, 0.0);
        assert_eq!(
            path.chain,
            ["lint (a)", "lint (b)"],
            "two instants, one after the other"
        );
        let skipped = timings(&observation(vec![pr(
            4,
            vec![check("lint", CheckRunState::Skipped, &at(0), &at(1))],
        )]));
        assert_eq!(skipped.heads[0].critical_path, None);
    }

    #[test]
    fn a_family_is_the_name_before_its_matrix_values() {
        assert_eq!(family("suite-shard (2)"), Some("suite-shard"));
        assert_eq!(family("rust (3, ubuntu)"), Some("rust"));
        assert_eq!(family("rust"), None);
        assert_eq!(family(" (1)"), None);
        assert_eq!(family("odd)"), None);
    }
}
