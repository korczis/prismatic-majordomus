//! Whether each structure job still fits in half its bound (I4381, criterion 9).
//!
//! On 2026-10-25 validate run 40813274272 was killed at the structure job's 55-minute bound with
//! all 59 of its gates green: shell-lint had grown from 416 s to a p90 of 1388 s in two weeks,
//! and nothing said so until the kill. The headroom is the warning that comes first. A job's
//! wall time, as CI measured it over the committed window of recent master runs, is read at its
//! 90th percentile (nearest rank) and compared with the job's `timeout-minutes`; past
//! `HEADROOM_PERCENT` of it the judgement fails and names the job and the gates that cost it
//! most, so the fix is a split or a cheaper gate and never a larger bound.
//!
//! # Nothing here is a second declaration
//!
//! - **Which jobs** are the structure jobs the CI model assigns gates to: every `job:` of
//!   `.ai/repo/ci/gates.yaml` named `structure` or `structure-<x>` (`structure_jobs`).
//! - **The bound** is the job's own `timeout-minutes` in `.github/workflows/validate.yml`.
//! - **The window** is `.ai/repo/ci/structure-job-durations.tsv`, written by
//!   `scripts/ci-baseline --structure-durations` from successful master runs that ran every
//!   structure job (one cohort: a run from before a split measured another job).
//! - **The cost of each gate** is `.ai/repo/ci/structure-durations.tsv`, written by the same
//!   command.
//!
//! A window that is missing, empty for a job or unreadable is never a pass: the job is
//! `HeadroomVerdict::Unmeasured`, and the judgement fails.
//!
//! The judgement is a pure function of those four texts (`judge`), so every case of it is a
//! fixture: the unit tests below hold 49% fitting, 51% exceeding, exactly half fitting, the
//! nearest-rank p90, the three costliest gates named, and every missing input failing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::model::{GateModel, MODEL_PATH};
use crate::order::{canonical, numeric, OrderKey, Ordered};

/// The window of measured job wall times, relative to the repository root.
pub(crate) const WINDOW_PATH: &str = ".ai/repo/ci/structure-job-durations.tsv";
/// The longest seconds each structure gate took, relative to the repository root.
pub(crate) const GATE_COSTS_PATH: &str = ".ai/repo/ci/structure-durations.tsv";
/// The workflow whose jobs carry the bounds, relative to the repository root.
pub(crate) const WORKFLOW_PATH: &str = ".github/workflows/validate.yml";
/// The share of a job's bound its p90 may take (I4381): past it, the job is one slow day from
/// being killed with every gate green.
pub(crate) const HEADROOM_PERCENT: u64 = 50;
/// How many of a job's gates the judgement names.
pub(crate) const COSTLIEST: usize = 3;

/// How one structure job stands against its bound.
///
/// ```
/// use majordomus_cli::gates::HeadroomVerdict;
/// // only a job that fits passes; a job nobody measured is not a pass
/// assert_eq!(serde_json::to_string(&HeadroomVerdict::Unmeasured).unwrap(), "\"unmeasured\"");
/// assert_ne!(HeadroomVerdict::Unmeasured, HeadroomVerdict::Fits);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeadroomVerdict {
    /// The p90 is at most `HEADROOM_PERCENT` of the bound.
    Fits,
    /// The p90 is past it.
    Exceeds,
    /// No window, no measured run of the job or no bound: not a pass.
    Unmeasured,
}

/// One gate and the longest seconds it took in the window its costs were read from: what a
/// job past its share names, longest first, so the remedy starts at the gate that costs most.
///
/// ```
/// use majordomus_cli::gates::GateCost;
/// let c: GateCost = serde_json::from_value(serde_json::json!({
///     "gate": "shell-lint", "seconds": 1388
/// })).unwrap();
/// assert_eq!((c.gate.as_str(), c.seconds), ("shell-lint", 1388));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GateCost {
    /// The gate's id in the CI model.
    pub gate: String,
    /// Its longest recorded seconds.
    pub seconds: u64,
}

/// Gates are read costliest first, then by name: the seconds are the rank, negated so that
/// the gate that costs most leads.
impl Ordered for GateCost {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.gate, &self.gate).descending(self.seconds)
    }
}

/// One structure job's headroom: its bound, the share of it the p90 may take, how many measured
/// runs the window holds, the p90 itself and the share it takes, the verdict with its reason in
/// one sentence, and the gates that cost the job most.
///
/// ```
/// use majordomus_cli::gates::{HeadroomVerdict, JobHeadroom};
/// let j: JobHeadroom = serde_json::from_value(serde_json::json!({
///     "job": "structure", "bound_seconds": 3300, "limit_seconds": 1650, "runs": 2,
///     "p90_seconds": 1455, "used_percent": 44, "verdict": "fits",
///     "reason": "job structure: p90 1455 s over 2 run(s) is 44% of its 3300 s bound",
///     "costliest_gates": []
/// })).unwrap();
/// // the p90 is within the limit, so the job fits
/// assert!(j.p90_seconds <= j.limit_seconds);
/// assert_eq!(j.verdict, HeadroomVerdict::Fits);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JobHeadroom {
    /// The job, as the CI model and the workflow name it.
    pub job: String,
    /// Its `timeout-minutes`, in seconds; absent when the workflow states none.
    pub bound_seconds: Option<u64>,
    /// `HEADROOM_PERCENT` of the bound, in seconds: the most the p90 may be.
    pub limit_seconds: Option<u64>,
    /// How many measured runs of the job the window holds.
    pub runs: u64,
    /// The 90th percentile (nearest rank) of those runs, in seconds.
    pub p90_seconds: Option<u64>,
    /// The p90 as a whole percent of the bound, rounded down.
    pub used_percent: Option<u64>,
    /// How the job stands.
    pub verdict: HeadroomVerdict,
    /// The verdict in one sentence.
    pub reason: String,
    /// The job's most expensive gates, longest first; at most `COSTLIEST`.
    pub costliest_gates: Vec<GateCost>,
}

/// Every structure job's headroom, and whether all of them fit.
///
/// ```
/// use majordomus_cli::gates::HeadroomReport;
/// let r: HeadroomReport = serde_json::from_value(serde_json::json!({
///     "ok": false, "percent": 50, "window": ".ai/repo/ci/structure-job-durations.tsv",
///     "jobs": [], "problems": ["the window is missing"]
/// })).unwrap();
/// // a report with a problem is never ok
/// assert!(!r.ok && !r.problems.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HeadroomReport {
    /// True only when every structure job fits and every input was read.
    pub ok: bool,
    /// The share of a bound a p90 may take.
    pub percent: u64,
    /// Where the window was read from.
    pub window: String,
    /// One entry per structure job, in name order.
    pub jobs: Vec<JobHeadroom>,
    /// What could not be read, one line each; any line makes `ok` false.
    pub problems: Vec<String>,
}

/// What the judgement reads, already loaded, so that every case is a fixture.
pub(crate) struct Inputs<'a> {
    /// The structure jobs, in name order.
    pub(crate) jobs: &'a [String],
    /// Each job's bound in seconds.
    pub(crate) bounds: &'a BTreeMap<String, u64>,
    /// Each gate's job.
    pub(crate) gate_jobs: &'a BTreeMap<String, String>,
    /// The window's text: `<run id>\t<job>\t<seconds>` lines, `#` comments; `None` when absent.
    pub window: Option<&'a str>,
    /// The gate costs' text: `<gate>\t<seconds>` lines, `#` comments; `None` when absent.
    pub costs: Option<&'a str>,
}

/// The structure jobs of a model: every gate's job named `structure` or `structure-<x>`.
pub(crate) fn structure_jobs(gate_jobs: &BTreeMap<String, String>) -> Vec<String> {
    let set: BTreeSet<&String> = gate_jobs
        .values()
        .filter(|j| j.as_str() == "structure" || j.starts_with("structure-"))
        .collect();
    set.into_iter().cloned().collect()
}

/// The 90th percentile of a non-empty list by nearest rank: the value at rank ⌈0.9·n⌉.
fn p90(sorted: &[u64]) -> u64 {
    let n = sorted.len() as u64;
    let rank = (n * 9).div_ceil(10).max(1);
    sorted[(rank - 1) as usize]
}

/// Each job's measured seconds from the window, or the lines that do not read.
fn parse_window(text: &str) -> (BTreeMap<String, Vec<u64>>, Vec<String>) {
    let mut runs: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    let mut problems = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            [run, job, secs] if run.parse::<u64>().is_ok() => match secs.parse::<u64>() {
                Ok(s) => runs.entry((*job).to_string()).or_default().push(s),
                Err(_) => problems.push(format!(
                    "{WINDOW_PATH}:{}: seconds is not a whole number: {secs}",
                    i + 1
                )),
            },
            _ => problems.push(format!(
                "{WINDOW_PATH}:{}: not <run id>\\t<job>\\t<seconds>: {line}",
                i + 1
            )),
        }
    }
    (runs, problems)
}

/// Each gate's recorded seconds from the costs file; unreadable lines are skipped, because the
/// costs only name gates and never decide the verdict.
fn parse_costs(text: &str) -> BTreeMap<String, u64> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let (g, s) = l.split_once('\t')?;
            Some((g.to_string(), s.trim().parse().ok()?))
        })
        .collect()
}

/// Judge every structure job against its bound.
pub(crate) fn judge(inputs: &Inputs<'_>) -> HeadroomReport {
    let mut problems = Vec::new();
    let (window, window_problems) = match inputs.window {
        Some(text) => parse_window(text),
        None => {
            problems.push(format!(
                "{WINDOW_PATH} is missing; run scripts/ci-baseline --structure-durations and commit it"
            ));
            (BTreeMap::new(), Vec::new())
        }
    };
    problems.extend(window_problems);
    let costs = inputs.costs.map(parse_costs).unwrap_or_default();

    let jobs = inputs
        .jobs
        .iter()
        .map(|job| {
            let bound = inputs.bounds.get(job).copied();
            let limit = bound.map(|b| b * HEADROOM_PERCENT / 100);
            // a sample has one order, `order::numeric`'s; whole seconds are exact in an f64
            let mut sample: Vec<f64> = window
                .get(job)
                .map(|v| v.iter().map(|&t| t as f64).collect())
                .unwrap_or_default();
            numeric(&mut sample);
            let measured: Vec<u64> = sample.into_iter().map(|t| t as u64).collect();
            let p = (!measured.is_empty()).then(|| p90(&measured));
            let mut costliest: Vec<GateCost> = inputs
                .gate_jobs
                .iter()
                .filter(|(_, j)| *j == job)
                .filter_map(|(g, _)| {
                    costs.get(g).map(|s| GateCost { gate: g.clone(), seconds: *s })
                })
                .collect();
            canonical(&mut costliest);
            costliest.truncate(COSTLIEST);
            let named = costliest
                .iter()
                .map(|c| format!("{} {} s", c.gate, c.seconds))
                .collect::<Vec<_>>()
                .join(", ");
            let (verdict, used, reason) = match (bound, p) {
                (None, _) => (
                    HeadroomVerdict::Unmeasured,
                    None,
                    format!("{WORKFLOW_PATH} states no timeout-minutes for job {job}"),
                ),
                (Some(_), None) => (
                    HeadroomVerdict::Unmeasured,
                    None,
                    format!("{WINDOW_PATH} holds no measured run of job {job}"),
                ),
                (Some(b), Some(p)) => {
                    let used = p * 100 / b.max(1);
                    // past the share, strictly: exactly half still fits
                    if p * 100 > b * HEADROOM_PERCENT {
                        (
                            HeadroomVerdict::Exceeds,
                            Some(used),
                            format!(
                                "job {job}: p90 {p} s over {} run(s) is {used}% of its {b} s bound, past {HEADROOM_PERCENT}%; split it or make its costliest gates cheaper ({}), never raise the bound",
                                measured.len(),
                                if named.is_empty() { "no gate costs recorded".to_string() } else { named.clone() }
                            ),
                        )
                    } else {
                        (
                            HeadroomVerdict::Fits,
                            Some(used),
                            format!(
                                "job {job}: p90 {p} s over {} run(s) is {used}% of its {b} s bound",
                                measured.len()
                            ),
                        )
                    }
                }
            };
            JobHeadroom {
                job: job.clone(),
                bound_seconds: bound,
                limit_seconds: limit,
                runs: measured.len() as u64,
                p90_seconds: p,
                used_percent: used,
                verdict,
                reason,
                costliest_gates: costliest,
            }
        })
        .collect::<Vec<_>>();
    if inputs.jobs.is_empty() {
        problems.push(format!("{MODEL_PATH} assigns no gate to a structure job"));
    }
    let ok = problems.is_empty() && jobs.iter().all(|j| j.verdict == HeadroomVerdict::Fits);
    HeadroomReport {
        ok,
        percent: HEADROOM_PERCENT,
        window: WINDOW_PATH.to_string(),
        jobs,
        problems,
    }
}

/// Each job's `timeout-minutes` in a workflow, in seconds: the first `timeout-minutes:` at the
/// job's own indentation (four spaces) under a `  <job>:` line, before the next job.
pub(crate) fn workflow_bounds(text: &str) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let job_line = line.strip_prefix("  ").filter(|rest| {
            !rest.starts_with(' ') && !rest.starts_with('#') && rest.trim_end().ends_with(':')
        });
        if let Some(rest) = job_line {
            current = Some(rest.trim_end().trim_end_matches(':').to_string());
            continue;
        }
        if !line.starts_with(' ') && !line.trim().is_empty() {
            current = None;
            continue;
        }
        if let (Some(job), Some(v)) = (&current, line.strip_prefix("    timeout-minutes:")) {
            if let Ok(min) = v.trim().parse::<u64>() {
                out.entry(job.clone()).or_insert(min * 60);
            }
        }
    }
    out
}

/// Read the model, the workflow, the window and the costs from a repository root, and judge.
pub(crate) fn read(root: &Path) -> HeadroomReport {
    let gate_jobs: BTreeMap<String, String> = match GateModel::load(root) {
        Ok(m) => m
            .gates
            .iter()
            .map(|g| (g.id.clone(), g.job.clone()))
            .collect(),
        Err(reason) => {
            return HeadroomReport {
                ok: false,
                percent: HEADROOM_PERCENT,
                window: WINDOW_PATH.to_string(),
                jobs: Vec::new(),
                problems: vec![reason],
            }
        }
    };
    let jobs = structure_jobs(&gate_jobs);
    let workflow = std::fs::read_to_string(root.join(WORKFLOW_PATH)).unwrap_or_default();
    let bounds = workflow_bounds(&workflow);
    let window = std::fs::read_to_string(root.join(WINDOW_PATH)).ok();
    let costs = std::fs::read_to_string(root.join(GATE_COSTS_PATH)).ok();
    judge(&Inputs {
        jobs: &jobs,
        bounds: &bounds,
        gate_jobs: &gate_jobs,
        window: window.as_deref(),
        costs: costs.as_deref(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(window: Option<&str>) -> HeadroomReport {
        let bounds = BTreeMap::from([
            ("structure".to_string(), 3300),
            ("structure-b".to_string(), 2400),
        ]);
        let gate_jobs = BTreeMap::from([
            ("slow-gate".to_string(), "structure".to_string()),
            ("mid-gate".to_string(), "structure".to_string()),
            ("small-gate".to_string(), "structure".to_string()),
            ("free-gate".to_string(), "structure".to_string()),
            ("net-gate".to_string(), "structure-b".to_string()),
            ("rust-gate".to_string(), "rust".to_string()),
        ]);
        let jobs = structure_jobs(&gate_jobs);
        judge(&Inputs {
            jobs: &jobs,
            bounds: &bounds,
            gate_jobs: &gate_jobs,
            window,
            costs: Some("# c\nsmall-gate\t10\nfree-gate\t0\nslow-gate\t1388\nmid-gate\t400\nnet-gate\t589\n"),
        })
    }

    #[test]
    fn only_structure_jobs_are_judged_in_name_order() {
        let r = fixture(Some("1\tstructure\t100\n1\tstructure-b\t100\n"));
        let jobs: Vec<&str> = r.jobs.iter().map(|j| j.job.as_str()).collect();
        assert_eq!(jobs, ["structure", "structure-b"]);
        assert!(r.ok);
    }

    #[test]
    fn forty_nine_percent_fits_and_fifty_one_exceeds() {
        // 3300 s bound: 49% is 1617 s, 51% is 1683 s; structure-b 2400 s: 49% is 1176 s
        let fits = fixture(Some("1\tstructure\t1617\n1\tstructure-b\t1176\n"));
        assert!(fits.ok, "{fits:?}");
        assert_eq!(fits.jobs[0].used_percent, Some(49));
        let over = fixture(Some("1\tstructure\t1683\n1\tstructure-b\t1176\n"));
        assert!(!over.ok);
        assert_eq!(over.jobs[0].verdict, HeadroomVerdict::Exceeds);
        assert_eq!(over.jobs[0].used_percent, Some(51));
        assert_eq!(over.jobs[1].verdict, HeadroomVerdict::Fits);
    }

    #[test]
    fn exactly_half_still_fits() {
        let r = fixture(Some("1\tstructure\t1650\n1\tstructure-b\t1200\n"));
        assert!(r.ok, "{r:?}");
    }

    #[test]
    fn an_exceeding_job_names_its_three_costliest_gates_longest_first() {
        let r = fixture(Some("1\tstructure\t3000\n1\tstructure-b\t100\n"));
        let names: Vec<&str> = r.jobs[0]
            .costliest_gates
            .iter()
            .map(|c| c.gate.as_str())
            .collect();
        assert_eq!(names, ["slow-gate", "mid-gate", "small-gate"]);
        assert!(
            r.jobs[0].reason.contains("slow-gate 1388 s"),
            "{}",
            r.jobs[0].reason
        );
        assert!(r.jobs[0].reason.contains("never raise the bound"));
    }

    #[test]
    fn the_p90_is_the_nearest_rank_not_the_mean_or_the_maximum() {
        // ten runs: rank 9 is the ninth smallest; the one slow outlier is the tenth
        let w: String = (1..=10)
            .map(|i| format!("{i}\tstructure\t{}\n", if i == 10 { 9999 } else { i * 100 }))
            .collect::<String>()
            + "1\tstructure-b\t1\n";
        let r = fixture(Some(&w));
        assert_eq!(r.jobs[0].p90_seconds, Some(900));
        assert_eq!(r.jobs[0].runs, 10);
        assert!(r.ok);
        assert_eq!(p90(&[7]), 7);
    }

    #[test]
    fn a_missing_window_or_a_job_without_runs_is_never_a_pass() {
        let none = fixture(None);
        assert!(!none.ok);
        assert!(none
            .jobs
            .iter()
            .all(|j| j.verdict == HeadroomVerdict::Unmeasured));
        assert!(none.problems[0].contains("is missing"));
        let partial = fixture(Some("1\tstructure\t100\n"));
        assert!(!partial.ok);
        assert_eq!(partial.jobs[1].verdict, HeadroomVerdict::Unmeasured);
        let empty = fixture(Some("# only a comment\n"));
        assert!(!empty.ok);
    }

    #[test]
    fn an_unreadable_window_line_fails_and_is_named() {
        let r = fixture(Some("1\tstructure\t100\n1\tstructure-b\tlots\nnonsense\n"));
        assert!(!r.ok);
        assert_eq!(r.problems.len(), 2);
        assert!(r.problems[0].contains(":2:"), "{:?}", r.problems);
    }

    #[test]
    fn a_job_without_a_bound_is_unmeasured() {
        let gate_jobs = BTreeMap::from([("g".to_string(), "structure-c".to_string())]);
        let jobs = structure_jobs(&gate_jobs);
        let r = judge(&Inputs {
            jobs: &jobs,
            bounds: &BTreeMap::new(),
            gate_jobs: &gate_jobs,
            window: Some("1\tstructure-c\t1\n"),
            costs: None,
        });
        assert_eq!(r.jobs[0].verdict, HeadroomVerdict::Unmeasured);
        assert!(r.jobs[0].reason.contains("timeout-minutes"));
        assert!(!r.ok);
    }

    #[test]
    fn a_model_with_no_structure_job_is_a_problem() {
        let r = judge(&Inputs {
            jobs: &[],
            bounds: &BTreeMap::new(),
            gate_jobs: &BTreeMap::new(),
            window: Some(""),
            costs: None,
        });
        assert!(!r.ok);
    }

    #[test]
    fn a_repository_without_a_model_answers_with_the_reason_and_is_not_ok() {
        let dir = tempfile::tempdir().unwrap();
        let r = read(dir.path());
        assert!(!r.ok);
        assert!(r.jobs.is_empty());
        assert!(r.problems[0].contains("gates.yaml"), "{:?}", r.problems);
    }

    #[test]
    fn the_workflow_bound_is_read_at_the_jobs_own_indentation() {
        let wf = "name: v\njobs:\n  plan:\n    timeout-minutes: 10\n    steps:\n      - run: x\n        timeout-minutes: 3\n  structure:\n    needs: plan\n    # a comment\n    timeout-minutes: 55\n  structure-b:\n    timeout-minutes: 40\n  ci:\n    steps: []\n";
        let b = workflow_bounds(wf);
        assert_eq!(b.get("plan"), Some(&600));
        assert_eq!(b.get("structure"), Some(&3300));
        assert_eq!(b.get("structure-b"), Some(&2400));
        assert_eq!(b.get("ci"), None);
    }
}
