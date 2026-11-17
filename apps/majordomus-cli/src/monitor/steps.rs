//! `majordomus monitor steps`: wait until every step of a range of one job of a workflow
//! run is success (I4624).
//!
//! A landing gated on "the hosted job's steps 4-12 are success" was watched by hand-written
//! `gh api` loops, and they broke three separate ways. This module is the reading those loops
//! got wrong, written once:
//!
//! - **Pending is everything not yet concluded.** A running step reports conclusion `""`
//!   through `gh run view --json jobs` and `null` through the REST jobs API; both, and a
//!   status other than `completed`, are pending. A fallback that reads a present value as
//!   done declared a run green after four of its nine steps.
//! - **Red is any conclusion but success, at once.** `failure`, `cancelled` (a run cancelled
//!   by concurrency, or cut at its timeout) and `timed_out` end the wait on the first step that
//!   carries one, even while other steps of the range are still running. `skipped` is red
//!   too unless the caller allows it: a step that did not run did not succeed.
//! - **A finished run settles what is missing.** While the run goes on, a job not yet listed
//!   and a step past the job's last are pending; once the run (or the job) has finished, they
//!   are red, named with what the job does have.
//!
//! Reading is gentle: no interval below [`FLOOR`] seconds, a run read and a jobs read per
//! poll, a rate-limited answer waited out until the limit's reset, and a transient failure
//! (a 5xx, a dropped connection) retried with a doubling wait. Everything the network
//! answers goes through [`Api`] and every wait through [`Clock`], so the tests feed recorded
//! `gh api` answers and a clock that only counts.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

use super::github;
use crate::cli::StepRange;

/// The shortest interval between two reads, in seconds: several watchers reading every
/// minute ran an account's core API limit out in one session.
pub(crate) const FLOOR: u64 = 120;

/// The interval between two reads when the caller does not say, in seconds.
pub(crate) const DEFAULT_INTERVAL: u64 = 180;

/// The longest wait a run of transient failures doubles up to, in seconds.
const BACKOFF_CAP: u64 = 1800;

/// How long one `gh api` call may take.
const CALL_BUDGET: Duration = Duration::from_secs(30);

/// At or below this many calls left in the hour's limit, the next read waits for the reset:
/// one poll spends two.
const RESERVE: u64 = 2;

/// Which run is watched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Subject {
    /// One run, by its id.
    Run(u64),
    /// The newest run for a head commit of a branch, of one workflow when it is named.
    Head {
        /// The branch the run is for.
        branch: String,
        /// The head commit the run is for.
        sha: String,
        /// A workflow-name pattern; `*` matches any run of characters.
        workflow: Option<String>,
    },
}

/// What is watched, and how strictly.
#[derive(Debug, Clone)]
pub(crate) struct Watch {
    /// The run.
    pub subject: Subject,
    /// A job-name pattern; `*` matches any run of characters, and every matching job is judged.
    pub job: String,
    /// The steps of each matching job that must be success.
    pub steps: StepRange,
    /// Whether a step that concluded `skipped` passes.
    pub allow_skipped: bool,
}

/// The verdict of a step, a job, a reading and a watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Verdict {
    /// Concluded success (or skipped, when allowed).
    Green,
    /// Not concluded yet: running, queued, or not listed while the run goes on.
    Pending,
    /// Concluded anything but success, or missing from a run that finished.
    Red,
}

impl Verdict {
    fn as_str(self) -> &'static str {
        match self {
            Verdict::Green => "green",
            Verdict::Pending => "pending",
            Verdict::Red => "red",
        }
    }
}

/// One step of the range as the jobs API reported it, with its verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct StepReading {
    /// The step's number.
    pub number: u64,
    /// Its name; absent when the job lists no such step.
    pub name: Option<String>,
    /// Its status as reported (`completed`, `in_progress`, `pending`, `queued`).
    pub status: Option<String>,
    /// Its conclusion as reported; `""` and `null` both read as none.
    pub conclusion: Option<String>,
    /// What it means for the watch.
    pub verdict: Verdict,
    /// Whether the job lists no step with this number.
    pub missing: bool,
}

/// One matching job, the steps of the range, and its verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct JobReading {
    /// The job's id.
    pub id: Option<u64>,
    /// The job's name.
    pub name: String,
    /// Its status.
    pub status: String,
    /// Its conclusion, when it has one.
    pub conclusion: Option<String>,
    /// Its page.
    pub url: Option<String>,
    /// The numbers of every step the job lists, for the reader of a range that missed.
    pub listed: Vec<u64>,
    /// The steps of the range.
    pub steps: Vec<StepReading>,
    /// The worst verdict of its steps.
    pub verdict: Verdict,
}

/// The run as it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RunReading {
    /// The run's id.
    pub id: u64,
    /// The workflow's name.
    pub name: String,
    /// Its status.
    pub status: String,
    /// Its conclusion, when it has one.
    pub conclusion: Option<String>,
    /// The branch it ran for.
    pub branch: Option<String>,
    /// The commit it ran for.
    pub sha: Option<String>,
    /// Which attempt this is.
    pub attempt: Option<u64>,
    /// Its page.
    pub url: Option<String>,
}

/// One reading of the run and its jobs, judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Reading {
    /// The run; absent while no run for the head exists yet.
    pub run: Option<RunReading>,
    /// Every job whose name matches, in the order the API listed them.
    pub jobs: Vec<JobReading>,
    /// The verdict over every matching job's range.
    pub verdict: Verdict,
    /// Why, in one line: the first red step, what is still pending, or that all is green.
    pub reason: String,
}

/// A reported string, with `""` and `null` both read as none.
fn reported(v: &Value, key: &str) -> Option<String> {
    v[key]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Whether `name` matches `pattern`, where `*` matches any run of characters and everything
/// else matches itself, over the whole name.
fn matches(pattern: &str, name: &str) -> bool {
    let body = pattern
        .split('*')
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join(".*");
    Regex::new(&format!("^{body}$")).is_ok_and(|re| re.is_match(name))
}

/// The verdict of one step as reported, before what is missing is considered.
fn step_verdict(status: Option<&str>, conclusion: Option<&str>, allow_skipped: bool) -> Verdict {
    match (status, conclusion) {
        (Some("completed"), Some("success")) => Verdict::Green,
        (Some("completed"), Some("skipped")) if allow_skipped => Verdict::Green,
        // a conclusion stated is final, whatever the status says beside it
        (_, Some(_)) => Verdict::Red,
        // no conclusion: running, queued, or completed with nothing said — never green
        (_, None) => Verdict::Pending,
    }
}

fn run_reading(run: &Value) -> Option<RunReading> {
    Some(RunReading {
        id: run["id"].as_u64()?,
        name: reported(run, "name").unwrap_or_default(),
        status: reported(run, "status").unwrap_or_default(),
        conclusion: reported(run, "conclusion"),
        branch: reported(run, "head_branch"),
        sha: reported(run, "head_sha"),
        attempt: run["run_attempt"].as_u64(),
        url: reported(run, "html_url"),
    })
}

/// Judge one reading: `run` as `GET …/actions/runs/{id}` answers it, `jobs` as `GET
/// …/actions/runs/{id}/jobs` does (or `gh run view --json jobs`, whose fields are the same
/// for this purpose).
pub(crate) fn judge(run: &Value, jobs: &Value, watch: &Watch) -> Reading {
    let run = run_reading(run);
    let run_finished = run.as_ref().is_some_and(|r| r.status == "completed");
    let mut readings = Vec::new();
    for job in jobs["jobs"].as_array().into_iter().flatten() {
        let name = reported(job, "name").unwrap_or_default();
        if !matches(&watch.job, &name) {
            continue;
        }
        let status = reported(job, "status").unwrap_or_default();
        // a job that finished, or that belongs to a run that did, will list nothing more
        let settled = run_finished || status == "completed";
        let listed: Vec<&Value> = job["steps"].as_array().into_iter().flatten().collect();
        let steps: Vec<StepReading> = (watch.steps.first..=watch.steps.last)
            .map(
                |n| match listed.iter().find(|s| s["number"].as_u64() == Some(n)) {
                    Some(step) => {
                        let status = reported(step, "status");
                        let conclusion = reported(step, "conclusion");
                        let mut verdict = step_verdict(
                            status.as_deref(),
                            conclusion.as_deref(),
                            watch.allow_skipped,
                        );
                        if verdict == Verdict::Pending && settled {
                            verdict = Verdict::Red;
                        }
                        StepReading {
                            number: n,
                            name: reported(step, "name"),
                            status,
                            conclusion,
                            verdict,
                            missing: false,
                        }
                    }
                    None => StepReading {
                        number: n,
                        name: None,
                        status: None,
                        conclusion: None,
                        verdict: if settled {
                            Verdict::Red
                        } else {
                            Verdict::Pending
                        },
                        missing: true,
                    },
                },
            )
            .collect();
        let verdict = steps
            .iter()
            .map(|s| s.verdict)
            .max()
            .unwrap_or(Verdict::Green);
        readings.push(JobReading {
            id: job["id"].as_u64().or_else(|| job["databaseId"].as_u64()),
            name,
            status,
            conclusion: reported(job, "conclusion"),
            url: reported(job, "html_url").or_else(|| reported(job, "url")),
            listed: listed.iter().filter_map(|s| s["number"].as_u64()).collect(),
            steps,
            verdict,
        });
    }
    let (verdict, reason) = verdict_of(run.as_ref(), &readings, watch);
    Reading {
        run,
        jobs: readings,
        verdict,
        reason,
    }
}

/// The verdict over every matching job and the line that says why.
fn verdict_of(run: Option<&RunReading>, jobs: &[JobReading], watch: &Watch) -> (Verdict, String) {
    let Some(run) = run else {
        return (Verdict::Pending, "no run for this head yet".into());
    };
    if jobs.is_empty() {
        return if run.status == "completed" {
            (
                Verdict::Red,
                format!(
                    "run {} finished ({}) and no job matches {:?}",
                    run.id,
                    run.conclusion.as_deref().unwrap_or("no conclusion"),
                    watch.job
                ),
            )
        } else {
            (
                Verdict::Pending,
                format!("no job matching {:?} is listed yet", watch.job),
            )
        };
    }
    let red = jobs.iter().find_map(|j| {
        j.steps
            .iter()
            .find(|s| s.verdict == Verdict::Red)
            .map(|s| (j, s))
    });
    if let Some((job, step)) = red {
        let what = if step.missing {
            format!(
                "step {} of {:?} does not exist and the job finished; it lists steps {}",
                step.number,
                job.name,
                spans(&job.listed)
            )
        } else {
            format!(
                "step {} {:?} of {:?} {}",
                step.number,
                step.name.as_deref().unwrap_or_default(),
                job.name,
                match step.conclusion.as_deref() {
                    Some(c) => format!("concluded {c}"),
                    None => format!(
                        "never concluded ({}) and the {} finished",
                        step.status.as_deref().unwrap_or("no status"),
                        if run.status == "completed" {
                            "run"
                        } else {
                            "job"
                        }
                    ),
                }
            )
        };
        return (Verdict::Red, what);
    }
    let pending: Vec<String> = jobs
        .iter()
        .flat_map(|j| {
            j.steps
                .iter()
                .filter(|s| s.verdict == Verdict::Pending)
                .map(move |s| (j, s))
        })
        .map(|(j, s)| match (&s.status, s.missing) {
            (_, true) => format!("{} step {} (not listed)", j.name, s.number),
            (Some(status), _) => format!("{} step {} ({status})", j.name, s.number),
            (None, _) => format!("{} step {}", j.name, s.number),
        })
        .collect();
    let total: usize = jobs.iter().map(|j| j.steps.len()).sum();
    if pending.is_empty() {
        return (
            Verdict::Green,
            format!(
                "every step {} of {} job(s) matching {:?} is success",
                watch.steps,
                jobs.len(),
                watch.job
            ),
        );
    }
    let shown: Vec<&str> = pending.iter().take(3).map(String::as_str).collect();
    let more = pending.len().saturating_sub(shown.len());
    (
        Verdict::Pending,
        format!(
            "{} of {total} step(s) success; waiting on {}{}",
            total - pending.len(),
            shown.join(", "),
            if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            }
        ),
    )
}

/// Step numbers as spans: `1-4, 8, 25-28`; `none` when there are none.
fn spans(numbers: &[u64]) -> String {
    let mut sorted = numbers.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < sorted.len() {
        let start = sorted[i];
        let mut end = start;
        while i + 1 < sorted.len() && sorted[i + 1] == end + 1 {
            i += 1;
            end = sorted[i];
        }
        out.push(if start == end {
            start.to_string()
        } else {
            format!("{start}-{end}")
        });
        i += 1;
    }
    if out.is_empty() {
        "none".into()
    } else {
        out.join(", ")
    }
}

/// What one `GET` came to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Answer {
    /// A 2xx answer and its body, with the limit's reset (Unix seconds) when the calls left in
    /// it are at or below [`RESERVE`].
    Json(Value, Option<u64>),
    /// The rate limit refused the call: wait until `reset` (Unix seconds) or `retry_after`
    /// seconds, whichever is said.
    RateLimited {
        /// When the limit resets, in Unix seconds.
        reset: Option<u64>,
        /// How long GitHub asked to wait, in seconds.
        retry_after: Option<u64>,
    },
    /// Worth asking again later: a 5xx, a dropped connection, a call past its budget.
    Transient(String),
    /// Asking again will not help: no `gh`, no repository, no such run, no permission.
    Unreadable(String),
}

/// The network, as the watcher needs it: one `GET` of a REST path with query parameters.
pub(crate) trait Api {
    /// `GET path?query`.
    fn get(&mut self, path: &str, query: &[(&str, String)]) -> Answer;
}

/// Time, as the watcher needs it.
pub(crate) trait Clock {
    /// Now, in Unix seconds.
    fn now(&self) -> u64;
    /// Wait this many seconds.
    fn sleep(&mut self, seconds: u64);
}

/// The wall clock.
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn sleep(&mut self, seconds: u64) {
        std::thread::sleep(Duration::from_secs(seconds));
    }
}

/// `gh api`, in the checkout at `root`, for the repository `gh` resolves there or for
/// `github` (`OWNER/NAME`) when given.
pub(crate) struct GhApi {
    /// Where `gh` runs, so that it resolves `{owner}/{repo}` from this checkout's remotes.
    pub root: PathBuf,
    /// The repository to read instead, as `OWNER/NAME`.
    pub github: Option<String>,
    /// The program to run; `gh` in earnest.
    pub program: String,
}

impl Api for GhApi {
    fn get(&mut self, path: &str, query: &[(&str, String)]) -> Answer {
        let fields: Vec<String> = query.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let mut args: Vec<&str> = vec!["api", "--include", "--method", "GET", path];
        for field in &fields {
            args.push("--raw-field");
            args.push(field);
        }
        let envs: Vec<(&str, &str)> = self
            .github
            .as_deref()
            .map(|repo| vec![("GH_REPO", repo)])
            .unwrap_or_default();
        match github::call(&self.program, &self.root, &args, &envs, CALL_BUDGET) {
            Ok(called) => parse_answer(&called.stdout, &called.stderr),
            Err((super::MonitorSourceHealth::Absent, reason)) => Answer::Unreadable(reason),
            Err((_, reason)) => Answer::Transient(reason),
        }
    }
}

/// What `gh api --include` printed, as an [`Answer`]: the status line and headers first when
/// GitHub answered, only an error on stderr when it was never reached.
pub(crate) fn parse_answer(stdout: &str, stderr: &str) -> Answer {
    let text = stdout.replace("\r\n", "\n");
    let Some(rest) = text.strip_prefix("HTTP/") else {
        let reason = stderr
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("gh api failed and said nothing")
            .to_string();
        let lower = reason.to_lowercase();
        let transient = [
            "error connecting",
            "timeout",
            "timed out",
            "connection",
            "eof",
            "tls",
            "no such host",
            "temporarily",
        ]
        .iter()
        .any(|w| lower.contains(w));
        return if transient {
            Answer::Transient(reason)
        } else {
            Answer::Unreadable(reason)
        };
    };
    let (head, body) = rest.split_once("\n\n").unwrap_or((rest, ""));
    let mut lines = head.lines();
    let code: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let header = |name: &str| {
        head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    };
    let number = |name: &str| header(name).and_then(|v| v.parse::<u64>().ok());
    let remaining = number("x-ratelimit-remaining");
    let reset = number("x-ratelimit-reset");
    let retry_after = number("retry-after");
    let body: Value = serde_json::from_str(body.trim()).unwrap_or(Value::Null);
    let message = body["message"].as_str().unwrap_or_default().to_string();
    match code {
        200..=299 => {
            let low = remaining.is_some_and(|r| r <= RESERVE);
            Answer::Json(body, reset.filter(|_| low))
        }
        403 | 429
            if remaining == Some(0)
                || retry_after.is_some()
                || message.to_lowercase().contains("rate limit") =>
        {
            Answer::RateLimited { reset, retry_after }
        }
        500..=599 => Answer::Transient(format!("GitHub answered {code}: {message}")),
        _ => Answer::Unreadable(format!(
            "GitHub answered {code}: {}",
            if message.is_empty() {
                "no message"
            } else {
                &message
            }
        )),
    }
}

/// Why a read did not produce a reading.
#[derive(Debug, Clone, PartialEq)]
enum Halt {
    /// Wait until this Unix second before asking again.
    Until(u64, String),
    /// Ask again after a doubled wait.
    Transient(String),
    /// Stop: asking again will not help.
    Unreadable(String),
}

/// Ask `api` once for the run and its jobs and judge them; the second value is the Unix
/// second before which nothing more should be asked, when the limit is nearly spent.
fn read(api: &mut dyn Api, watch: &Watch, now: u64) -> Result<(Reading, Option<u64>), Halt> {
    let mut hold: Option<u64> = None;
    let run = match &watch.subject {
        Subject::Run(id) => ask(
            api,
            now,
            &mut hold,
            &format!("repos/{{owner}}/{{repo}}/actions/runs/{id}"),
            &[],
        )?,
        Subject::Head {
            branch,
            sha,
            workflow,
        } => {
            let runs = ask(
                api,
                now,
                &mut hold,
                "repos/{owner}/{repo}/actions/runs",
                &[
                    ("branch", branch.clone()),
                    ("head_sha", sha.clone()),
                    ("per_page", "50".into()),
                ],
            )?;
            match pick(&runs, workflow.as_deref())? {
                Some(run) => run,
                None => {
                    let reading = judge(&Value::Null, &json!({"jobs": []}), watch);
                    return Ok((reading, hold));
                }
            }
        }
    };
    let Some(id) = run["id"].as_u64() else {
        return Err(Halt::Unreadable("GitHub answered a run with no id".into()));
    };
    let jobs = ask(
        api,
        now,
        &mut hold,
        &format!("repos/{{owner}}/{{repo}}/actions/runs/{id}/jobs"),
        &[("per_page", "100".into())],
    )?;
    Ok((judge(&run, &jobs, watch), hold))
}

/// One `GET` through `api`, as a value or the halt it comes to; a nearly spent limit's reset
/// is kept in `hold`.
fn ask(
    api: &mut dyn Api,
    now: u64,
    hold: &mut Option<u64>,
    path: &str,
    query: &[(&str, String)],
) -> Result<Value, Halt> {
    match api.get(path, query) {
        Answer::Json(v, reset) => {
            *hold = (*hold).max(reset);
            Ok(v)
        }
        Answer::RateLimited { reset, retry_after } => {
            let until = retry_after
                .map(|s| now + s)
                .max(reset)
                .unwrap_or(now + FLOOR);
            Err(Halt::Until(
                until,
                "GitHub's rate limit refused the read".into(),
            ))
        }
        Answer::Transient(reason) => Err(Halt::Transient(reason)),
        Answer::Unreadable(reason) => Err(Halt::Unreadable(reason)),
    }
}

/// The newest run of a runs listing, of the workflow named when one is; several workflows
/// and none named is refused, because which one carries the job is the caller's to say.
fn pick(runs: &Value, workflow: Option<&str>) -> Result<Option<Value>, Halt> {
    let candidates: Vec<&Value> = runs["workflow_runs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| workflow.is_none_or(|w| matches(w, r["name"].as_str().unwrap_or_default())))
        .collect();
    let mut names: Vec<&str> = candidates
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    if names.len() > 1 {
        return Err(Halt::Unreadable(format!(
            "several workflows ran for this head ({}): name one with --workflow",
            names.join(", ")
        )));
    }
    // the API lists the newest first
    Ok(candidates.first().map(|r| (*r).clone()))
}

/// How a watch ended.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Outcome {
    /// `green`, `red`, `timeout` or `unreadable`.
    pub verdict: &'static str,
    /// Why, in one line.
    pub reason: String,
    /// The last reading, when there was one.
    pub reading: Option<Reading>,
    /// How many polls were made.
    pub polls: u64,
}

impl Outcome {
    /// The exit code: 0 green, 10 red, 11 timed out while pending, 12 unreadable.
    pub(crate) fn exit_code(&self) -> u8 {
        match self.verdict {
            "green" => 0,
            "red" => 10,
            "timeout" => 11,
            _ => 12,
        }
    }
}

/// Watch until the verdict is green or red, the deadline passes, or GitHub cannot be read;
/// `progress` hears every poll that decided nothing, with the seconds until the next.
pub(crate) fn watch(
    api: &mut dyn Api,
    clock: &mut dyn Clock,
    w: &Watch,
    interval: u64,
    timeout: u64,
    progress: &mut dyn FnMut(&str),
) -> Outcome {
    let interval = interval.max(FLOOR);
    let deadline = clock.now().saturating_add(timeout);
    let mut backoff = interval;
    let mut last: Option<Reading> = None;
    let mut polls = 0;
    loop {
        polls += 1;
        let now = clock.now();
        let (wait, said) = match read(api, w, now) {
            Ok((reading, hold)) => {
                if reading.verdict != Verdict::Pending {
                    return Outcome {
                        verdict: reading.verdict.as_str(),
                        reason: reading.reason.clone(),
                        reading: Some(reading),
                        polls,
                    };
                }
                backoff = interval;
                let wait = hold.map_or(interval, |h| h.saturating_sub(now).max(interval));
                let said = format!("pending: {}", reading.reason);
                last = Some(reading);
                (wait, said)
            }
            Err(Halt::Until(until, why)) => (until.saturating_sub(now).max(interval), why),
            Err(Halt::Transient(why)) => {
                let wait = backoff;
                backoff = backoff.saturating_mul(2).min(BACKOFF_CAP.max(interval));
                (wait, format!("transient: {why}"))
            }
            Err(Halt::Unreadable(why)) => {
                return Outcome {
                    verdict: "unreadable",
                    reason: why,
                    reading: last,
                    polls,
                };
            }
        };
        let now = clock.now();
        if now >= deadline {
            return Outcome {
                verdict: "timeout",
                reason: format!(
                    "the deadline passed while pending; last: {}",
                    last.as_ref().map_or(said.as_str(), |r| r.reason.as_str())
                ),
                reading: last,
                polls,
            };
        }
        let wait = wait.min(deadline - now);
        progress(&format!("{said}; next read in {wait}s"));
        clock.sleep(wait);
    }
}

/// The reading as lines for a person: the run, each job, each step of the range, the verdict.
pub(crate) fn render(outcome: &Outcome) -> String {
    let mut out = String::new();
    if let Some(reading) = &outcome.reading {
        if let Some(run) = &reading.run {
            out.push_str(&format!(
                "run {}  {}  {}{}  {} {}\n",
                run.id,
                run.name,
                run.status,
                run.conclusion
                    .as_deref()
                    .map(|c| format!("/{c}"))
                    .unwrap_or_default(),
                run.branch.as_deref().unwrap_or("-"),
                run.sha.as_deref().map_or("-", |s| &s[..s.len().min(12)]),
            ));
            if let Some(url) = &run.url {
                out.push_str(&format!("    {url}\n"));
            }
        }
        for job in &reading.jobs {
            out.push_str(&format!(
                "job {}  {}{}  {}\n",
                job.name,
                job.status,
                job.conclusion
                    .as_deref()
                    .map(|c| format!("/{c}"))
                    .unwrap_or_default(),
                job.verdict.as_str()
            ));
            for step in &job.steps {
                out.push_str(&format!(
                    "  {:>3}  {:<7}  {:<12}  {}\n",
                    step.number,
                    step.verdict.as_str(),
                    if step.missing {
                        "not listed".to_string()
                    } else {
                        match (&step.status, &step.conclusion) {
                            (_, Some(c)) => c.clone(),
                            (Some(s), None) => s.clone(),
                            (None, None) => "-".into(),
                        }
                    },
                    step.name.as_deref().unwrap_or_default()
                ));
            }
        }
    }
    out.push_str(&format!("{}: {}\n", outcome.verdict, outcome.reason));
    out
}

/// The checkout `gh` runs in: the repository root a command was pointed at.
pub(crate) fn gh_api(root: &Path, github: Option<String>) -> GhApi {
    GhApi {
        root: root.to_path_buf(),
        github,
        program: "gh".into(),
    }
}

#[cfg(test)]
mod tests {
    //! The fixtures under `tests/fixtures/gh-api-steps/` are `gh api` answers recorded from
    //! this repository's own workflow runs on 2026-11-17 and trimmed to the fields read (the
    //! jobs to three each); `jobs-in-progress-run-view.json` is `gh run view --json jobs` of
    //! the same run, which prints `""` where the REST API prints `null`. `rate-limited.http`
    //! is the one constructed answer, in the shape GitHub documents for a spent limit.

    use super::*;
    use std::collections::VecDeque;

    macro_rules! fixture {
        ($name:literal) => {
            serde_json::from_str::<Value>(include_str!(concat!(
                "../../tests/fixtures/gh-api-steps/",
                $name
            )))
            .expect(concat!($name, " is JSON"))
        };
    }

    fn watching(job: &str, steps: &str) -> Watch {
        Watch {
            subject: Subject::Run(1),
            job: job.into(),
            steps: crate::cli::parse_step_range(steps).unwrap(),
            allow_skipped: false,
        }
    }

    #[test]
    fn a_running_step_whose_conclusion_is_empty_is_pending() {
        // `gh run view --json jobs` prints "" for a running step's conclusion: the reading
        // that took a present value for done declared green after four of nine steps
        let reading = judge(
            &fixture!("run-in-progress.json"),
            &fixture!("jobs-in-progress-run-view.json"),
            &watching("suite-shard (4)", "4-12"),
        );
        assert_eq!(reading.verdict, Verdict::Pending, "{}", reading.reason);
        let step8 = &reading.jobs[0].steps[4];
        assert_eq!(step8.number, 8);
        assert_eq!(step8.status.as_deref(), Some("in_progress"));
        assert_eq!(step8.conclusion, None, "\"\" is no conclusion");
        assert_eq!(step8.verdict, Verdict::Pending);
        assert!(reading.jobs[0].steps[..4]
            .iter()
            .all(|s| s.verdict == Verdict::Green));
        assert!(
            reading.reason.contains("4 of 9 step(s) success"),
            "{}",
            reading.reason
        );
    }

    #[test]
    fn a_running_step_whose_conclusion_is_null_is_pending() {
        let reading = judge(
            &fixture!("run-in-progress.json"),
            &fixture!("jobs-in-progress.json"),
            &watching("suite-shard (4)", "4-12"),
        );
        assert_eq!(reading.verdict, Verdict::Pending, "{}", reading.reason);
        // steps 9-12 are listed as pending; 13 and 14 follow, then the post steps from 25
        assert_eq!(reading.jobs[0].listed.len(), 18);
        assert!(reading.jobs[0].steps[5..]
            .iter()
            .all(|s| s.verdict == Verdict::Pending));
    }

    #[test]
    fn a_step_that_says_completed_with_no_conclusion_is_still_pending() {
        let run = json!({"id": 1, "status": "in_progress"});
        let jobs = json!({"jobs": [{"name": "test", "status": "in_progress", "steps": [
            {"number": 1, "status": "completed", "conclusion": ""}
        ]}]});
        let reading = judge(&run, &jobs, &watching("test", "1"));
        assert_eq!(reading.verdict, Verdict::Pending);
    }

    #[test]
    fn a_cancelled_step_is_red_at_once() {
        let reading = judge(
            &fixture!("run-cancelled-with-jobs.json"),
            &fixture!("jobs-cancelled.json"),
            &watching("ui-shard (4)", "4-10"),
        );
        assert_eq!(reading.verdict, Verdict::Red);
        assert!(
            reading.reason.contains("step 7") && reading.reason.contains("concluded cancelled"),
            "{}",
            reading.reason
        );
    }

    #[test]
    fn a_red_step_ends_the_wait_while_others_still_run() {
        let run = json!({"id": 1, "status": "in_progress"});
        let jobs = json!({"jobs": [{"name": "test", "status": "in_progress", "steps": [
            {"number": 4, "status": "completed", "conclusion": "timed_out"},
            {"number": 5, "status": "in_progress", "conclusion": null}
        ]}]});
        let reading = judge(&run, &jobs, &watching("test", "4-5"));
        assert_eq!(reading.verdict, Verdict::Red);
        assert!(
            reading.reason.contains("concluded timed_out"),
            "{}",
            reading.reason
        );
    }

    #[test]
    fn a_run_that_finished_with_no_matching_job_is_red() {
        // cancelled by concurrency before any job started: the jobs API lists none
        let reading = judge(
            &fixture!("run-cancelled-before-jobs.json"),
            &fixture!("jobs-none.json"),
            &watching("structure", "4-12"),
        );
        assert_eq!(reading.verdict, Verdict::Red);
        assert!(
            reading.reason.contains("finished (cancelled)")
                && reading.reason.contains("no job matches"),
            "{}",
            reading.reason
        );
    }

    #[test]
    fn a_missing_job_is_pending_while_the_run_goes_on() {
        let reading = judge(
            &fixture!("run-in-progress.json"),
            &fixture!("jobs-in-progress.json"),
            &watching("pages", "1-3"),
        );
        assert_eq!(reading.verdict, Verdict::Pending);
        assert!(
            reading.reason.contains("no job matching"),
            "{}",
            reading.reason
        );
    }

    #[test]
    fn a_range_past_the_steps_of_a_finished_job_is_red_and_names_its_steps() {
        // `build` finished with steps 1-7 and 13-15 (numbers have gaps); a range to 16 can never be met
        let reading = judge(
            &fixture!("run-success.json"),
            &fixture!("jobs-success.json"),
            &Watch {
                allow_skipped: true,
                ..watching("build", "13-16")
            },
        );
        assert_eq!(reading.verdict, Verdict::Red);
        assert!(
            reading.reason.contains("step 16")
                && reading.reason.contains("does not exist")
                && reading.reason.contains("lists steps 1-7, 13-15"),
            "{}",
            reading.reason
        );
    }

    #[test]
    fn a_range_past_the_listed_steps_of_a_running_job_is_pending() {
        let run = json!({"id": 1, "status": "in_progress"});
        let jobs = json!({"jobs": [{"name": "test", "status": "in_progress", "steps": [
            {"number": 1, "status": "completed", "conclusion": "success"}
        ]}]});
        assert_eq!(
            judge(&run, &jobs, &watching("test", "1-3")).verdict,
            Verdict::Pending
        );
    }

    #[test]
    fn a_finished_range_of_success_is_green() {
        let reading = judge(
            &fixture!("run-success.json"),
            &fixture!("jobs-success.json"),
            &watching("structure", "1-12"),
        );
        assert_eq!(reading.verdict, Verdict::Green, "{}", reading.reason);
        assert_eq!(reading.jobs[0].steps.len(), 12);
    }

    #[test]
    fn a_skipped_step_is_red_unless_allowed() {
        // `build` skips step 7 when nothing asks for it
        let strict = judge(
            &fixture!("run-success.json"),
            &fixture!("jobs-success.json"),
            &watching("build", "4-7"),
        );
        assert_eq!(strict.verdict, Verdict::Red);
        assert!(
            strict.reason.contains("concluded skipped"),
            "{}",
            strict.reason
        );
        let lenient = judge(
            &fixture!("run-success.json"),
            &fixture!("jobs-success.json"),
            &Watch {
                allow_skipped: true,
                ..watching("build", "4-7")
            },
        );
        assert_eq!(lenient.verdict, Verdict::Green, "{}", lenient.reason);
    }

    #[test]
    fn a_job_pattern_matches_the_whole_name_and_every_match_is_judged() {
        assert!(matches("suite-shard*", "suite-shard (4)"));
        assert!(matches("rust (*)", "rust (2)"));
        assert!(!matches("suite", "suite-shard (4)"));
        assert!(
            matches("a.b", "a.b") && !matches("a.b", "axb"),
            "a dot is a dot"
        );
        let reading = judge(
            &fixture!("run-cancelled-with-jobs.json"),
            &fixture!("jobs-cancelled.json"),
            &watching("*", "1"),
        );
        assert_eq!(reading.jobs.len(), 3);
    }

    #[test]
    fn spans_compress_step_numbers() {
        assert_eq!(spans(&[1, 2, 3, 4, 8, 25, 26]), "1-4, 8, 25-26");
        assert_eq!(spans(&[]), "none");
    }

    #[test]
    fn an_answer_is_parsed_with_its_limit() {
        let ok = include_str!("../../tests/fixtures/gh-api-steps/run-in-progress.http");
        let Answer::Json(body, hold) = parse_answer(ok, "") else {
            panic!("a 200 is JSON")
        };
        assert_eq!(body["status"], "in_progress");
        assert_eq!(hold, None, "4593 calls left holds nothing back");
        let spent = ok.replace("X-Ratelimit-Remaining: 4593", "X-Ratelimit-Remaining: 1");
        assert_eq!(
            parse_answer(&spent, ""),
            Answer::Json(body, Some(1794877950)),
            "a nearly spent limit names its reset"
        );
        let limited = include_str!("../../tests/fixtures/gh-api-steps/rate-limited.http");
        assert_eq!(
            parse_answer(limited, "gh: API rate limit exceeded (HTTP 403)"),
            Answer::RateLimited {
                reset: Some(1794877950),
                retry_after: None
            }
        );
        let secondary = "HTTP/2.0 429 Too Many Requests\nRetry-After: 60\n\n{}";
        assert_eq!(
            parse_answer(secondary, ""),
            Answer::RateLimited {
                reset: None,
                retry_after: Some(60)
            }
        );
        let missing = "HTTP/2.0 404 Not Found\n\n{\"message\":\"Not Found\"}";
        assert_eq!(
            parse_answer(missing, ""),
            Answer::Unreadable("GitHub answered 404: Not Found".into())
        );
        assert!(matches!(
            parse_answer("HTTP/2.0 502 Bad Gateway\n\n", ""),
            Answer::Transient(_)
        ));
        assert!(matches!(
            parse_answer("", "error connecting to api.github.com"),
            Answer::Transient(_)
        ));
        assert!(matches!(
            parse_answer("", "none of the git remotes configured for this repository point to a known GitHub host"),
            Answer::Unreadable(_)
        ));
    }

    /// Recorded answers handed out in order, each path asked recorded.
    struct Replay {
        answers: VecDeque<Answer>,
        asked: Vec<String>,
    }

    impl Replay {
        fn new(answers: Vec<Answer>) -> Replay {
            Replay {
                answers: answers.into(),
                asked: Vec::new(),
            }
        }
    }

    impl Api for Replay {
        fn get(&mut self, path: &str, query: &[(&str, String)]) -> Answer {
            let q: Vec<String> = query.iter().map(|(k, v)| format!("{k}={v}")).collect();
            self.asked.push(format!("{path}?{}", q.join("&")));
            self.answers
                .pop_front()
                .unwrap_or_else(|| Answer::Unreadable("the replay ran out".into()))
        }
    }

    /// A clock that only counts.
    struct Counted {
        now: u64,
        slept: Vec<u64>,
    }

    impl Clock for Counted {
        fn now(&self) -> u64 {
            self.now
        }
        fn sleep(&mut self, seconds: u64) {
            self.slept.push(seconds);
            self.now += seconds;
        }
    }

    fn json(v: Value) -> Answer {
        Answer::Json(v, None)
    }

    #[test]
    fn a_watch_polls_gently_until_green_and_exits_0() {
        let mut api = Replay::new(vec![
            json(fixture!("run-in-progress.json")),
            json(fixture!("jobs-in-progress.json")),
            json(fixture!("run-success.json")),
            json(fixture!("jobs-success.json")),
        ]);
        let mut clock = Counted {
            now: 1000,
            slept: vec![],
        };
        let mut said = Vec::new();
        let outcome = watch(
            &mut api,
            &mut clock,
            &watching("structure", "1-12"),
            30,
            3600,
            &mut |line| said.push(line.to_string()),
        );
        assert_eq!(outcome.verdict, "green", "{}", outcome.reason);
        assert_eq!(outcome.exit_code(), 0);
        assert_eq!(outcome.polls, 2);
        assert_eq!(
            clock.slept,
            vec![FLOOR],
            "an interval below the floor is raised to it"
        );
        assert_eq!(api.asked[0], "repos/{owner}/{repo}/actions/runs/1?");
        assert_eq!(
            api.asked[1],
            "repos/{owner}/{repo}/actions/runs/43976207701/jobs?per_page=100"
        );
        assert!(said[0].starts_with("pending: "), "{said:?}");
        assert!(
            render(&outcome).contains("green: every step 1-12"),
            "{}",
            render(&outcome)
        );
    }

    #[test]
    fn a_watch_exits_10_on_red_and_11_at_the_deadline() {
        let mut api = Replay::new(vec![
            json(fixture!("run-cancelled-with-jobs.json")),
            json(fixture!("jobs-cancelled.json")),
        ]);
        let mut clock = Counted {
            now: 0,
            slept: vec![],
        };
        let red = watch(
            &mut api,
            &mut clock,
            &watching("ui-shard (4)", "4-10"),
            180,
            600,
            &mut |_| {},
        );
        assert_eq!((red.verdict, red.exit_code()), ("red", 10));
        assert!(render(&red).contains("cancelled"), "{}", render(&red));

        let pending = || {
            vec![
                json(fixture!("run-in-progress.json")),
                json(fixture!("jobs-in-progress.json")),
            ]
        };
        let mut api = Replay::new([pending(), pending(), pending()].concat());
        let mut clock = Counted {
            now: 0,
            slept: vec![],
        };
        let late = watch(
            &mut api,
            &mut clock,
            &watching("suite-shard (4)", "4-12"),
            180,
            300,
            &mut |_| {},
        );
        assert_eq!((late.verdict, late.exit_code()), ("timeout", 11));
        assert_eq!(
            clock.slept,
            vec![180, 120],
            "the last wait is cut to the deadline"
        );
        assert_eq!(late.polls, 3, "one read at the deadline itself");
    }

    #[test]
    fn a_rate_limited_read_waits_for_the_reset_and_a_transient_one_doubles() {
        let mut api = Replay::new(vec![
            Answer::RateLimited {
                reset: Some(2000),
                retry_after: None,
            },
            Answer::Transient("GitHub answered 502".into()),
            Answer::Transient("GitHub answered 502".into()),
            json(fixture!("run-success.json")),
            json(fixture!("jobs-success.json")),
        ]);
        let mut clock = Counted {
            now: 1000,
            slept: vec![],
        };
        let outcome = watch(
            &mut api,
            &mut clock,
            &watching("ci", "1-8"),
            180,
            7200,
            &mut |_| {},
        );
        assert_eq!(outcome.verdict, "green", "{}", outcome.reason);
        assert_eq!(clock.slept, vec![1000, 180, 360]);
    }

    #[test]
    fn a_nearly_spent_limit_holds_the_next_read_until_its_reset() {
        let mut api = Replay::new(vec![
            json(fixture!("run-in-progress.json")),
            Answer::Json(fixture!("jobs-in-progress.json"), Some(5000)),
            json(fixture!("run-success.json")),
            json(fixture!("jobs-success.json")),
        ]);
        let mut clock = Counted {
            now: 1000,
            slept: vec![],
        };
        let outcome = watch(
            &mut api,
            &mut clock,
            &watching("ci", "1-8"),
            180,
            7200,
            &mut |_| {},
        );
        assert_eq!(outcome.verdict, "green");
        assert_eq!(clock.slept, vec![4000]);
    }

    #[test]
    fn an_unreadable_answer_exits_12_at_once() {
        let mut api = Replay::new(vec![Answer::Unreadable(
            "GitHub answered 404: Not Found".into(),
        )]);
        let mut clock = Counted {
            now: 0,
            slept: vec![],
        };
        let outcome = watch(
            &mut api,
            &mut clock,
            &watching("ci", "1"),
            180,
            7200,
            &mut |_| {},
        );
        assert_eq!((outcome.verdict, outcome.exit_code()), ("unreadable", 12));
        assert!(clock.slept.is_empty());
    }

    #[test]
    fn a_head_is_read_through_its_newest_run_of_the_named_workflow() {
        let watch_head = |workflow: Option<&str>| Watch {
            subject: Subject::Head {
                branch: "plan/i4605-commit-control-character-20261116".into(),
                sha: "abc".into(),
                workflow: workflow.map(str::to_string),
            },
            ..watching("structure", "1-12")
        };
        let mut api = Replay::new(vec![
            json(fixture!("runs-branch.json")),
            json(fixture!("jobs-success.json")),
        ]);
        let (reading, _) = read(&mut api, &watch_head(Some("validate")), 0).unwrap();
        assert_eq!(reading.verdict, Verdict::Green, "{}", reading.reason);
        assert_eq!(
            api.asked[0],
            "repos/{owner}/{repo}/actions/runs?branch=plan/i4605-commit-control-character-20261116&head_sha=abc&per_page=50"
        );
        assert_eq!(
            api.asked[1],
            "repos/{owner}/{repo}/actions/runs/43965643615/jobs?per_page=100"
        );

        let mut api = Replay::new(vec![json(fixture!("runs-branch.json"))]);
        let refused = read(&mut api, &watch_head(None), 0).unwrap_err();
        assert!(
            matches!(&refused, Halt::Unreadable(m) if m.contains("pr-meta, validate")),
            "{refused:?}"
        );

        let mut api = Replay::new(vec![json(json!({"total_count": 0, "workflow_runs": []}))]);
        let (none, _) = read(&mut api, &watch_head(Some("validate")), 0).unwrap();
        assert_eq!(none.verdict, Verdict::Pending);
        assert_eq!(none.reason, "no run for this head yet");
    }
}
