//! How long CI takes, measured from the recorded forge observation: every check run the forge
//! reported on an open pull request's head, with when it started and when it completed.
//!
//! Nothing here asks the forge or estimates: a duration is `completed_at - started_at` of a run
//! that has both, and a run without either is counted as unmeasured, never as zero. The
//! observation holds the latest run of each check on each open head, so this is what CI took
//! on the heads now open, not a history; the history of merges is `integration::metrics`.

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

/// How long CI took on one open pull request's head: from its first check's start to its last
/// check's completion, and the check that took longest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
}

/// CI timings over every open head the observation holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

fn slowest_first(seconds: u64) -> i64 {
    i64::try_from(seconds).map_or(i64::MIN, |s| -s)
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
        for c in pr.checks.iter().filter(|c| c.kind == CheckKind::CheckRun) {
            let measured = parse_rfc3339(&c.started_at).zip(parse_rfc3339(&c.completed_at));
            let Some((start, end)) = measured.filter(|(s, e)| e >= s) else {
                unmeasured += 1;
                continue;
            };
            let took = end - start;
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

    #[test]
    fn the_nearest_rank_of_one_value_is_that_value() {
        assert_eq!(rank(&[42], 50), 42);
        assert_eq!(rank(&[42], 90), 42);
        assert_eq!(rank(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 90), 9);
        assert_eq!(rank(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 50), 5);
    }
}
