//! Trends: the series of CI runs the pages publication records, read as it lies in the
//! checkout.
//!
//! `scripts/pages trends` writes `site/data/trends.json` from the forge, oldest run first:
//! for every run its wall and queued time, its jobs and, when the run measured it, its
//! coverage; and, over all of them, the median of every job. The file is produced at publish
//! time only. It is never derived, never committed, and nothing committed depends on it, so
//! a checkout that was never published has none.
//!
//! This module only reads that one file and says what it holds, or why it holds nothing:
//!
//! - **Absent** is a state, never an error and never an empty series: a checkout the pages
//!   publication never wrote into has recorded no trends.
//! - **Another schema** than [`SCHEMA`] is refused with the schema it found, because a file
//!   that is not this series must not be drawn as one.
//! - **A figure the file does not give** stays `None`. A run without coverage has a gap in
//!   the coverage line, and a job without a median has no median; neither is a zero.
//!
//! Nothing is fetched and nothing is written: the reader of a screen gets this file's
//! content, and a published site's own copy is the site's page, not this process's reading.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::order::{OrderKey, Ordered};

/// The schema the series is written with.
pub(crate) const SCHEMA: &str = "trends/v1";

/// Where the series lies, relative to the repository's root.
pub(crate) const FILE: &str = "site/data/trends.json";

/// One covered-over-total figure of a run, with the percentage the two give.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TrendRatio {
    /// What the run covered: lines, functions or regions.
    pub covered: u64,
    /// What there was to cover.
    pub total: u64,
    /// `covered` over `total` in percent, or `None` when there was nothing to cover.
    pub percent: Option<f64>,
}

/// The coverage a run measured; each kind is `None` when the file does not give it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TrendCoverage {
    /// Source lines.
    pub lines: Option<TrendRatio>,
    /// Functions.
    pub functions: Option<TrendRatio>,
    /// Regions.
    pub regions: Option<TrendRatio>,
}

/// One job of a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TrendJob {
    /// The job's name.
    pub name: String,
    /// Seconds it waited for a runner, when the file gives them.
    pub queued_seconds: Option<f64>,
    /// Seconds it ran, when the file gives them.
    pub run_seconds: Option<f64>,
    /// How it ended, when the file says.
    pub conclusion: Option<String>,
}

/// One run of the series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TrendRun {
    /// The forge's id of the run, as text.
    pub run_id: String,
    /// The commit the run judged.
    pub head_sha: Option<String>,
    /// When the run started, as the file wrote it.
    pub started_at: Option<String>,
    /// How the run ended.
    pub conclusion: Option<String>,
    /// Seconds from the run's start to its end.
    pub wall_seconds: Option<f64>,
    /// Seconds its jobs waited for a runner, summed.
    pub queued_seconds_total: Option<f64>,
    /// Its jobs.
    pub jobs: Vec<TrendJob>,
    /// The coverage it measured, or `None` when it measured none.
    pub coverage: Option<TrendCoverage>,
}

/// The median of one job over the series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct JobMedian {
    /// The job's name.
    pub name: String,
    /// How many runs the median is over.
    pub runs: Option<u64>,
    /// The median of its run time, in seconds.
    pub median_run_seconds: Option<f64>,
    /// The median of its queue time, in seconds.
    pub median_queued_seconds: Option<f64>,
}

impl Ordered for JobMedian {
    /// The slowest median first; a job without one after every job that has one.
    fn order_key(&self) -> OrderKey<'_> {
        let key = OrderKey::plain(&self.name, &self.name);
        match self.median_run_seconds {
            // a median is a count of seconds: its whole seconds are the rank, largest first
            Some(s) => key.descending(s.round() as u64),
            None => key.ranked(i64::MAX),
        }
    }
}

/// The series, or why this checkout holds none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct TrendsReport {
    /// Whether the checkout holds a series of this schema.
    pub recorded: bool,
    /// Why it does not, when it does not.
    pub reason: Option<String>,
    /// The ids of the runs the series was written from, as the file gives them.
    pub generated_from: Vec<String>,
    /// The runs, oldest first.
    pub runs: Vec<TrendRun>,
    /// The medians of the jobs, the slowest first.
    pub job_medians: Vec<JobMedian>,
}

impl TrendsReport {
    fn none(reason: String) -> Self {
        TrendsReport {
            recorded: false,
            reason: Some(reason),
            generated_from: Vec::new(),
            runs: Vec::new(),
            job_medians: Vec::new(),
        }
    }
}

/// Read the series from the checkout at `root`.
pub(crate) fn read(root: &std::path::Path) -> TrendsReport {
    match std::fs::read_to_string(root.join(FILE)) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            TrendsReport::none(format!("{FILE} is not in this checkout"))
        }
        Err(e) => TrendsReport::none(format!("{FILE} could not be read: {e}")),
    }
}

/// The series a file's text holds, or why it holds none.
pub(crate) fn parse(text: &str) -> TrendsReport {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return TrendsReport::none(format!("{FILE} is not JSON: {e}")),
    };
    match value.get("schema").and_then(Value::as_str) {
        Some(SCHEMA) => {}
        Some(other) => {
            return TrendsReport::none(format!("{FILE} is schema {other}, not {SCHEMA}"));
        }
        None => return TrendsReport::none(format!("{FILE} names no schema, not {SCHEMA}")),
    }
    let mut job_medians: Vec<JobMedian> = array(&value, "job_medians")
        .iter()
        .map(|m| JobMedian {
            name: string(m, "name").unwrap_or_else(|| "unnamed".into()),
            runs: m.get("runs").and_then(Value::as_u64),
            median_run_seconds: seconds(m, "median_run_s"),
            median_queued_seconds: seconds(m, "median_queued_s"),
        })
        .collect();
    crate::order::canonical(&mut job_medians);
    TrendsReport {
        recorded: true,
        reason: None,
        generated_from: array(&value, "generated_from").iter().map(id).collect(),
        runs: array(&value, "runs").iter().map(run).collect(),
        job_medians,
    }
}

fn run(r: &Value) -> TrendRun {
    TrendRun {
        run_id: r.get("run_id").map_or_else(|| "unknown".into(), id),
        head_sha: string(r, "head_sha"),
        started_at: string(r, "started_at"),
        conclusion: string(r, "conclusion"),
        wall_seconds: seconds(r, "wall_seconds"),
        queued_seconds_total: seconds(r, "queued_seconds_total"),
        jobs: array(r, "jobs")
            .iter()
            .map(|j| TrendJob {
                name: string(j, "name").unwrap_or_else(|| "unnamed".into()),
                queued_seconds: seconds(j, "queued_s"),
                run_seconds: seconds(j, "run_s"),
                conclusion: string(j, "conclusion"),
            })
            .collect(),
        coverage: r
            .get("coverage")
            .filter(|c| c.is_object())
            .map(|c| TrendCoverage {
                lines: ratio(c, "lines"),
                functions: ratio(c, "functions"),
                regions: ratio(c, "regions"),
            }),
    }
}

fn ratio(coverage: &Value, kind: &str) -> Option<TrendRatio> {
    let figure = coverage.get(kind)?;
    let covered = figure.get("covered")?.as_u64()?;
    let total = figure.get("total")?.as_u64()?;
    Some(TrendRatio {
        covered,
        total,
        percent: (total > 0).then(|| covered as f64 * 100.0 / total as f64),
    })
}

fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// A number of seconds, when the file gives a finite one that is not negative.
fn seconds(v: &Value, key: &str) -> Option<f64> {
    v.get(key)
        .and_then(Value::as_f64)
        .filter(|s| s.is_finite() && *s >= 0.0)
}

/// An id the forge wrote as a number or as text.
fn id(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The series a fixture holds: four runs oldest first, the second without coverage.
    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test/fixtures/trends/trends.json"
    ));

    #[test]
    fn a_fixture_is_read_with_its_percentages_and_its_gap() {
        let report = parse(FIXTURE);
        assert!(report.recorded && report.reason.is_none());
        assert_eq!(report.generated_from, ["9001", "9002", "9003", "9004"]);
        assert_eq!(report.runs.len(), 4);
        let lines: Vec<Option<f64>> = report
            .runs
            .iter()
            .map(|r| {
                r.coverage
                    .as_ref()
                    .and_then(|c| c.lines.as_ref())
                    .and_then(|l| l.percent)
            })
            .collect();
        assert_eq!(lines, [Some(70.0), None, Some(75.0), Some(80.0)]);
        let regions = report.runs[3]
            .coverage
            .as_ref()
            .and_then(|c| c.regions.clone());
        assert_eq!(regions.and_then(|r| r.percent), Some(87.5));
        assert_eq!(report.runs[1].wall_seconds, Some(2100.0));
        assert_eq!(report.runs[0].jobs.len(), 2);
        // the slowest median first
        let names: Vec<&str> = report.job_medians.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["rust", "suite", "site", "lint"]);
    }

    #[test]
    fn a_figure_the_file_does_not_give_is_none_and_never_zero() {
        let report = parse(
            r#"{"schema":"trends/v1","runs":[{"run_id":7,"coverage":{"lines":{"covered":1,"total":0},"functions":{"covered":3}}}],
                "job_medians":[{"name":"x"}]}"#,
        );
        let run = &report.runs[0];
        assert_eq!(run.run_id, "7");
        assert_eq!(run.wall_seconds, None);
        let coverage = run.coverage.as_ref().expect("coverage is an object");
        assert_eq!(coverage.lines.as_ref().map(|l| l.percent), Some(None));
        assert_eq!(coverage.functions, None);
        assert_eq!(coverage.regions, None);
        assert_eq!(report.job_medians[0].median_run_seconds, None);
    }

    #[test]
    fn what_is_not_this_series_holds_none_and_says_why() {
        let wrong = parse(r#"{"schema":"trends/v2","runs":[]}"#);
        assert!(!wrong.recorded && wrong.runs.is_empty());
        assert!(wrong
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("trends/v2")));
        let none = parse(r#"{"runs":[]}"#);
        assert!(none
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("no schema")));
        let broken = parse("{");
        assert!(broken
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("not JSON")));
    }

    #[test]
    fn a_checkout_without_the_file_has_recorded_nothing() {
        let dir = tempfile::tempdir().expect("a directory");
        let report = read(dir.path());
        assert!(!report.recorded && report.runs.is_empty());
        assert!(report.reason.as_deref().is_some_and(|r| r.contains(FILE)));
        // a directory where the file should be is unreadable, and says so
        std::fs::create_dir_all(dir.path().join(FILE)).expect("a directory in its place");
        let unreadable = read(dir.path());
        assert!(unreadable
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("could not be read")));
    }

    #[test]
    fn a_job_median_without_a_name_is_unnamed_and_one_without_a_median_comes_last() {
        let report = parse(
            r#"{"schema":"trends/v1","job_medians":[
                {"name":"nomedian","runs":2},
                {"median_run_s":10.4},
                {"name":"slow","median_run_s":90}]}"#,
        );
        let names: Vec<&str> = report.job_medians.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["slow", "unnamed", "nomedian"]);
        assert_eq!(report.job_medians[2].median_run_seconds, None);
        assert_eq!(report.job_medians[2].runs, Some(2));
        assert_eq!(report.job_medians[1].median_run_seconds, Some(10.4));
    }

    #[test]
    fn a_run_and_a_job_without_an_id_or_a_name_are_unknown_and_unnamed() {
        let report = parse(
            r#"{"schema":"trends/v1","generated_from":["a-run",12],
                "runs":[{"jobs":[{"queued_s":-1,"run_s":3},{"name":"","conclusion":"failure"}]}]}"#,
        );
        // an id written as text stays as it is, one written as a number is its digits
        assert_eq!(report.generated_from, ["a-run", "12"]);
        let run = &report.runs[0];
        assert_eq!(run.run_id, "unknown");
        assert_eq!(run.jobs.len(), 2);
        assert_eq!(run.jobs[0].name, "unnamed");
        assert_eq!(
            run.jobs[0].queued_seconds, None,
            "a negative wait is no figure"
        );
        assert_eq!(run.jobs[0].run_seconds, Some(3.0));
        assert_eq!(run.jobs[1].name, "unnamed", "an empty name is no name");
        assert_eq!(run.jobs[1].conclusion.as_deref(), Some("failure"));
        assert_eq!(run.coverage, None);
        // a run whose id is text keeps it
        let text = parse(r#"{"schema":"trends/v1","runs":[{"run_id":"r-7"}]}"#);
        assert_eq!(text.runs[0].run_id, "r-7");
    }

    #[test]
    fn a_coverage_figure_missing_a_part_or_of_another_type_is_none() {
        let report = parse(
            r#"{"schema":"trends/v1","runs":[{"run_id":1,"coverage":{
                "lines":{"total":10},
                "functions":{"covered":"3","total":4},
                "regions":{"covered":3,"total":"4"}}},
                {"run_id":2,"coverage":{"lines":{"covered":1,"total":4}}},
                {"run_id":3,"coverage":[]}]}"#,
        );
        let none = report.runs[0].coverage.as_ref().expect("an object");
        assert_eq!(none.lines, None, "no covered count");
        assert_eq!(none.functions, None, "a covered count that is text");
        assert_eq!(none.regions, None, "a total that is text");
        let some = report.runs[1].coverage.as_ref().expect("an object");
        assert_eq!(some.lines.as_ref().and_then(|l| l.percent), Some(25.0));
        assert_eq!(some.functions, None, "a kind the run does not carry");
        assert_eq!(report.runs[2].coverage, None, "coverage that is no object");
    }

    #[test]
    fn a_series_that_holds_no_run_is_recorded_and_empty() {
        let report = parse(r#"{"schema":"trends/v1"}"#);
        assert!(report.recorded && report.reason.is_none());
        assert!(report.runs.is_empty() && report.job_medians.is_empty());
        assert!(report.generated_from.is_empty());
    }
}
