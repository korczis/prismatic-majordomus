//! The `postmortems` module: what turned the trunk's CI red, read back.
//!
//! Five operations over the records under `.ai/repo/post-mortems/`, none of which changes
//! the repository, and deliberately unequal in where they can be called from:
//!
//! - `postmortems.list`, `postmortems.check` and `postmortems.diagnose` read the index and
//!   nothing else, so they are reads on every surface. A page lists the records, a gate
//!   judges them, and an agent holding a failing log asks which record it looks like.
//! - `postmortems.owed` and `postmortems.draft` ask the forge, through the forge's own tool
//!   and with the caller's credential. They are the command line's and nothing else's: a
//!   page or a remote caller never makes this process reach the network.
//!
//! The rules themselves are in `crate::postmortem`; this file reads the index, asks the
//! forge when it is told to, and shapes the answers.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CliExposure, Exposure, McpExposure, McpResource, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::peers::parse_rfc3339;
use crate::postmortem::{
    self, forge, AnsweredRun, ForgeRun, OwedRun, PostMortem, PostMortemCause, PostMortemFinding,
    PostMortemMatch, PostMortemStatus,
};
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which the records are read as one resource.
pub(crate) const POST_MORTEMS_URI: &str = "majordomus://post-mortems";

/// The branch whose failed runs owe a record, unless the caller names another.
pub(crate) const DEFAULT_BRANCH: &str = "master";

/// How many of the branch's newest runs are asked for.
pub(crate) const DEFAULT_LIMIT: u64 = 60;

/// The most runs one question may ask the forge for.
pub(crate) const MAX_LIMIT: u64 = 500;

/// How long after a run finished its record may still be missing, in hours.
pub(crate) const DEFAULT_GRACE_HOURS: u64 = 24;

/// The day the rule came into force: a run created before it owes nothing.
pub(crate) const DEFAULT_SINCE: &str = "2026-10-13";

/// The exit status of a question whose answer is a measured no.
pub(crate) const EXIT_REFUSED: i32 = 10;

/// The exit status of a question nobody could answer.
pub(crate) const EXIT_UNANSWERED: i32 = 12;

/// One record as a list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemSummary {
    /// The record's identity.
    pub(crate) id: String,
    /// The record's URI in the index.
    pub(crate) uri: String,
    /// What went wrong, in one line.
    pub(crate) title: String,
    /// How far the record has come.
    pub(crate) status: PostMortemStatus,
    /// The day the failure was first seen.
    pub(crate) date: String,
    /// Why it failed.
    pub(crate) cause: PostMortemCause,
    /// The runs the record answers.
    pub(crate) runs: Vec<AnsweredRun>,
    /// Where the cause lives.
    pub(crate) components: Vec<String>,
    /// How many signals a failing log is searched for.
    pub(crate) signals: usize,
    /// What removed the cause, once it is removed.
    pub(crate) fixed_by: Vec<String>,
    /// What now stops the cause from returning, as references of the layer.
    pub(crate) prevention: Vec<String>,
    /// What else of the layer the failure concerns, as `<kind>:<identity>`.
    pub(crate) related: Vec<String>,
    /// The record's file, repository-relative.
    pub(crate) path: String,
}

/// A file indexed as a post-mortem whose front matter is not one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct UnreadablePostMortem {
    /// The file, repository-relative.
    pub(crate) path: String,
    /// Why it could not be read.
    pub(crate) reason: String,
}

/// Every record, newest first, with the counts a reader asks for first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemList {
    /// Where the records live, repository-relative.
    pub(crate) directory: String,
    /// How many records there are.
    pub(crate) count: usize,
    /// How many records carry each status.
    pub(crate) by_status: BTreeMap<String, u64>,
    /// How many records carry each cause class.
    pub(crate) by_cause: BTreeMap<String, u64>,
    /// The records, newest first.
    pub(crate) records: Vec<PostMortemSummary>,
    /// Files indexed as post-mortems that could not be read as one.
    pub(crate) unreadable: Vec<UnreadablePostMortem>,
}

/// What the cross-field rules found in the records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemCheck {
    /// Whether no rule was broken.
    pub(crate) ok: bool,
    /// How many records were judged.
    pub(crate) records: usize,
    /// Every broken rule, by record.
    pub(crate) findings: Vec<PostMortemFinding>,
}

/// The input of `postmortems.owed`: which runs are asked about and how late a record may
/// be. Everything is optional.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PostMortemOwedInput {
    /// The branch whose runs are asked about; `master` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) branch: Option<String>,
    /// How many of its newest runs the forge is asked for; 60 when absent, 500 at most.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) limit: Option<u64>,
    /// How many hours after a run finished its record may still be missing; 24 when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) grace_hours: Option<u64>,
    /// The day (`YYYY-MM-DD`) or moment (RFC 3339) before which no run owes anything;
    /// `2026-10-13` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) since: Option<String>,
    /// Only these workflows, by the name the forge reports; every workflow when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) workflows: Vec<String>,
    /// The moment ages are measured from, RFC 3339; the clock when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) now: Option<String>,
    /// The runs, in the shape `gh run list --json` prints them. When given, the forge is
    /// not asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runs: Option<Vec<Value>>,
}

/// Which of the trunk's failed runs no record answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemOwed {
    /// Whether the runs were observed. False when the forge could not be asked: nothing is
    /// owed by a forge nobody saw, and nothing is cleared by it either.
    pub(crate) observed: bool,
    /// Why not, in the words of whatever refused; absent when the runs were observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<String>,
    /// Where the runs came from: `forge` or `input`.
    pub(crate) source: String,
    /// The branch asked about.
    pub(crate) branch: String,
    /// The moment before which no run owes anything, as it was given.
    pub(crate) since: String,
    /// The grace, in hours.
    pub(crate) grace_hours: u64,
    /// How many runs of the branch were looked at.
    pub(crate) runs: usize,
    /// How many of them failed inside the window.
    pub(crate) failed: usize,
    /// How many of those a record names.
    pub(crate) answered: usize,
    /// The failed runs no record names, newest first.
    pub(crate) owed: Vec<OwedRun>,
    /// How many of them are past their grace.
    pub(crate) past_grace: usize,
    /// The exit status the command line reports: 0 nothing is owed past its grace, 10
    /// something is.
    pub(crate) exit_code: i32,
}

/// The input of `postmortems.diagnose`: the text of a failure.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PostMortemDiagnoseInput {
    /// The failing log, a job's summary, or any excerpt of either.
    pub(crate) text: String,
}

/// The records whose signals a failure shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemDiagnosis {
    /// How many records were searched.
    pub(crate) records: usize,
    /// How many signals those records carry.
    pub(crate) signals: usize,
    /// The records with at least one signal in the text: most signals first, then newest.
    pub(crate) matches: Vec<PostMortemMatch>,
}

/// The input of `postmortems.draft`: the run to draft a record for.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PostMortemDraftInput {
    /// The forge's identifier of the run: digits.
    pub(crate) run: String,
    /// Runs in the shape `gh run list --json` prints them. When given, the run is looked up
    /// among them and the forge is not asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runs: Option<Vec<Value>>,
    /// The names of the jobs that failed, for a run given in `runs`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) jobs: Vec<String>,
}

/// The text of a draft record for one failed run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemDraft {
    /// Whether the run was observed. False when the forge could not be asked.
    pub(crate) observed: bool,
    /// Why not; absent when the run was observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<String>,
    /// The identity the draft carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    /// Where a record of that identity lives, repository-relative. Nothing was written
    /// there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    /// The records that already name the run, by identity.
    pub(crate) answered_by: Vec<String>,
    /// The draft: front matter and the four sections. Empty when the run was not observed.
    pub(crate) text: String,
    /// The exit status the command line reports: 0 drafted, 12 the forge was not answered.
    pub(crate) exit_code: i32,
}

fn root_of(ctx: &Context) -> PathBuf {
    PathBuf::from(&ctx.index.repository.root)
}

/// Every record the index holds, newest first, and the files it holds as post-mortems that
/// do not read as one.
fn read(ctx: &Context) -> (Vec<PostMortem>, Vec<UnreadablePostMortem>) {
    let mut records = Vec::new();
    let mut unreadable = Vec::new();
    for o in ctx
        .index
        .objects
        .iter()
        .filter(|o| o.kind == postmortem::KIND)
    {
        match PostMortem::read(&o.metadata, &o.provenance.path) {
            Ok(record) => records.push(record),
            Err(reason) => unreadable.push(UnreadablePostMortem {
                path: o.provenance.path.clone(),
                reason,
            }),
        }
    }
    crate::order::canonical(&mut records);
    (records, unreadable)
}

fn summary(r: PostMortem) -> PostMortemSummary {
    PostMortemSummary {
        uri: crate::model::uri_for(postmortem::KIND, &r.id),
        id: r.id,
        title: r.title,
        status: r.status,
        date: r.date,
        cause: r.cause,
        runs: r.runs,
        components: r.components,
        signals: r.signals.len(),
        fixed_by: r.fixed_by,
        prevention: r.prevention,
        related: r.related,
        path: r.path,
    }
}

fn list(ctx: &Context, _: Empty) -> Result<PostMortemList, CapabilityError> {
    let (records, unreadable) = read(ctx);
    let mut by_status = BTreeMap::new();
    let mut by_cause = BTreeMap::new();
    for r in &records {
        *by_status.entry(r.status.as_str().to_string()).or_insert(0) += 1;
        *by_cause
            .entry(r.cause.class.as_str().to_string())
            .or_insert(0) += 1;
    }
    Ok(PostMortemList {
        directory: postmortem::DIRECTORY.to_string(),
        count: records.len(),
        by_status,
        by_cause,
        records: records.into_iter().map(summary).collect(),
        unreadable,
    })
}

/// Whether `path` is a file or directory of the tree under `root`. A path that climbs out
/// of the tree is not in it.
fn in_tree(root: &Path, path: &str) -> bool {
    let relative = Path::new(path);
    relative
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        && root.join(relative).exists()
}

fn check(ctx: &Context, _: Empty) -> Result<PostMortemCheck, CapabilityError> {
    let (records, unreadable) = read(ctx);
    let root = root_of(ctx);
    let mut findings: Vec<PostMortemFinding> = unreadable
        .into_iter()
        .map(|u| PostMortemFinding {
            record: u.path,
            code: "unreadable".to_string(),
            message: format!("the front matter is not a post-mortem: {}", u.reason),
        })
        .collect();
    findings.extend(postmortem::judge(&records, &|path| in_tree(&root, path)));
    // the body half of the contract: the sections the schema's `Body` names, read from the
    // list `generate` projects for the shell tool, so the two halves of the tool agree
    let rules = crate::share::Share::locate(ctx.index.share.as_deref(), &root)
        .ok()
        .and_then(|share| {
            std::fs::read_to_string(
                share
                    .sections_dir()
                    .join(format!("{}.txt", postmortem::KIND)),
            )
            .ok()
        })
        .map(|text| postmortem::section_rules(&text))
        .unwrap_or_default();
    for o in ctx
        .index
        .objects
        .iter()
        .filter(|o| o.kind == postmortem::KIND)
    {
        let label = o
            .metadata
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or(&o.provenance.path);
        findings.extend(postmortem::judge_body(label, &o.body, &rules));
    }
    Ok(PostMortemCheck {
        ok: findings.is_empty(),
        records: records.len(),
        findings,
    })
}

/// A day (`YYYY-MM-DD`) or a moment (RFC 3339) as Unix seconds.
fn moment(text: &str) -> Option<u64> {
    parse_rfc3339(text).or_else(|| parse_rfc3339(&format!("{text}T00:00:00Z")))
}

fn owed(ctx: &Context, input: PostMortemOwedInput) -> Result<PostMortemOwed, CapabilityError> {
    owed_with(forge::GH, ctx, input)
}

/// `postmortems.owed`, asking `program` in place of `gh`: the seam a test puts its own
/// program in.
fn owed_with(
    program: &str,
    ctx: &Context,
    input: PostMortemOwedInput,
) -> Result<PostMortemOwed, CapabilityError> {
    let branch = input.branch.unwrap_or_else(|| DEFAULT_BRANCH.to_string());
    let since_text = input.since.unwrap_or_else(|| DEFAULT_SINCE.to_string());
    let since = moment(&since_text).ok_or_else(|| {
        CapabilityError::InvalidInput(format!(
            "`{since_text}` is neither a day (YYYY-MM-DD) nor an RFC 3339 moment"
        ))
    })?;
    let now = match &input.now {
        Some(text) => parse_rfc3339(text).ok_or_else(|| {
            CapabilityError::InvalidInput(format!("`{text}` is not an RFC 3339 moment"))
        })?,
        None => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default(),
    };
    let grace_hours = input.grace_hours.unwrap_or(DEFAULT_GRACE_HOURS);
    let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let (source, observed) = match input.runs {
        Some(given) => ("input", Ok(ForgeRun::list_from_forge(&Value::Array(given)))),
        None => (
            "forge",
            forge::runs_with(program, &root_of(ctx), &branch, limit),
        ),
    };
    let unobserved = |reason: String| PostMortemOwed {
        observed: false,
        reason: Some(reason),
        source: source.to_string(),
        branch: branch.clone(),
        since: since_text.clone(),
        grace_hours,
        runs: 0,
        failed: 0,
        answered: 0,
        owed: Vec::new(),
        past_grace: 0,
        exit_code: 0,
    };
    let runs = match observed {
        Ok(runs) => postmortem::of_trunk(runs, &branch, &input.workflows),
        Err(reason) => return Ok(unobserved(reason)),
    };
    let (records, _) = read(ctx);
    let grace = grace_hours.saturating_mul(3600);
    let owing = postmortem::owed(&runs, &records, now, grace, since);
    // the same question with no record answers how many failed inside the window at all
    let failed = postmortem::owed(&runs, &[], now, grace, since).len();
    let past_grace = owing.iter().filter(|o| !o.within_grace).count();
    Ok(PostMortemOwed {
        observed: true,
        reason: None,
        source: source.to_string(),
        branch,
        since: since_text,
        grace_hours,
        runs: runs.len(),
        failed,
        answered: failed - owing.len(),
        exit_code: if past_grace > 0 { EXIT_REFUSED } else { 0 },
        past_grace,
        owed: owing,
    })
}

fn diagnose(
    ctx: &Context,
    input: PostMortemDiagnoseInput,
) -> Result<PostMortemDiagnosis, CapabilityError> {
    if input.text.trim().is_empty() {
        return Err(CapabilityError::Refused(
            "there is no text to search: pass the failing log, or an excerpt of it".into(),
        ));
    }
    let (records, _) = read(ctx);
    Ok(PostMortemDiagnosis {
        records: records.len(),
        signals: records.iter().map(|r| r.signals.len()).sum(),
        matches: postmortem::diagnose(&input.text, &records),
    })
}

fn draft(ctx: &Context, input: PostMortemDraftInput) -> Result<PostMortemDraft, CapabilityError> {
    draft_with(forge::GH, ctx, input)
}

/// `postmortems.draft`, asking `program` in place of `gh`.
fn draft_with(
    program: &str,
    ctx: &Context,
    input: PostMortemDraftInput,
) -> Result<PostMortemDraft, CapabilityError> {
    if !forge::is_run_id(&input.run) {
        return Err(CapabilityError::InvalidInput(format!(
            "`{}` is not a run identifier: digits only",
            input.run
        )));
    }
    let observed = match input.runs {
        Some(given) => ForgeRun::list_from_forge(&Value::Array(given))
            .into_iter()
            .find(|r| r.id == input.run)
            .map(|r| (r, input.jobs))
            .ok_or_else(|| {
                CapabilityError::NotFound(format!(
                    "run {} is not among the runs that were given",
                    input.run
                ))
            })?,
        None => match forge::run_with(program, &root_of(ctx), &input.run) {
            Ok(found) => found,
            Err(reason) => {
                return Ok(PostMortemDraft {
                    observed: false,
                    reason: Some(reason),
                    id: None,
                    path: None,
                    answered_by: Vec::new(),
                    text: String::new(),
                    exit_code: EXIT_UNANSWERED,
                })
            }
        },
    };
    let (run, jobs) = observed;
    let text = postmortem::draft(&run, &jobs).map_err(CapabilityError::Refused)?;
    let id = postmortem::draft_id(&run).map_err(CapabilityError::Refused)?;
    let (records, _) = read(ctx);
    Ok(PostMortemDraft {
        observed: true,
        reason: None,
        path: Some(format!("{}{id}.md", postmortem::DIRECTORY)),
        id: Some(id),
        answered_by: records
            .iter()
            .filter(|r| r.runs.iter().any(|answered| answered.id == run.id))
            .map(|r| r.id.clone())
            .collect(),
        text,
        exit_code: 0,
    })
}

impl BenchmarkCases for PostMortemOwedInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing: the capability is waived because it asks the forge; timing it in a loop
        // would measure the forge, not this executable
        Vec::new()
    }
}

impl BenchmarkCases for PostMortemDraftInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing, for the same reason
        Vec::new()
    }
}

impl BenchmarkCases for PostMortemDiagnoseInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // a log that shows a signal some record carries, when any record carries one, and
        // one that shows none
        let known = ctx
            .index
            .objects
            .iter()
            .filter(|o| o.kind == postmortem::KIND)
            .filter_map(|o| PostMortem::read(&o.metadata, &o.provenance.path).ok())
            .find_map(|r| r.signals.into_iter().next())
            .map(|s| s.text);
        let mut cases = vec![NamedCase::new(
            "no-known-signal",
            PostMortemDiagnoseInput {
                text: "error: process completed with exit code 1".into(),
            },
        )];
        if let Some(text) = known {
            cases.push(NamedCase::new(
                "a-recorded-signal",
                PostMortemDiagnoseInput {
                    text: format!("step 3\n{text}\nstep 4"),
                },
            ));
        }
        cases
    }
}

/// The module: three reads on every surface and two questions to the forge that only the
/// command line asks.
pub(crate) fn module() -> ModuleDescriptor {
    module! {
        id: "postmortems",
        title: "Post-mortems",
        description: "What turned the trunk's CI red and why: one record per cause under .ai/repo/post-mortems/, naming the runs it answers, the cause as a class and a sentence, the components it lives in, the signals a failing log shows, what fixed it and what now prevents it. The records are listed, judged against the rules their schema cannot say, compared with the trunk's failed runs to find the run nobody has answered, and searched for the signals of a new failure. Nothing here changes the repository; the two operations that ask the forge are the command line's alone.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "postmortems.list",
                title: "Every post-mortem, newest first",
                description: "Every record under .ai/repo/post-mortems/ with its status, day, cause class and summary, the runs it answers, its components, what fixed and what prevents it, what else of the layer it concerns, how many signals it carries and its path, with the counts by status and by cause class. A file indexed as a post-mortem whose front matter does not read as one is listed apart, with the reason. Reads the index and nothing else.",
                input: Empty,
                output: PostMortemList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_postmortems".into()),
                        resource: Some(McpResource { uri: POST_MORTEMS_URI.into(), name: "post-mortems".into() }),
                    }),
                    http: get("/api/v1/post-mortems"),
                    cli: Some(CliExposure { path: vec!["postmortem".into(), "list".into()] }),
                },
                tags: ["post-mortem", "ci", "evidence"],
                handler: list,
            },
            capability! {
                id: "postmortems.check",
                title: "Judge the post-mortems against the rules their schema cannot say",
                description: "The rules that relate two fields of a record, a record to the tree and one record to another: a draft carries the cause class `undetermined` and a later status does not; an analysed or resolved record names a component and a signal; `fixed_by` is present exactly when the record is resolved; a component or a prevention that names a path (`file:`, `test:`) names one in the tree (a resolved record may name a component its fix removed); no two records share an identity. Whether a reference to an object of the layer resolves is the graph's question, answered for every kind by the one check of unresolved relations, and is not asked again here. Each finding names the record, a stable code and one sentence. Reads the index and the tree; asks no network.",
                input: Empty,
                output: PostMortemCheck,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: super::mcp("majordomus_postmortems_check"),
                    http: get("/api/v1/post-mortems/check"),
                    cli: Some(CliExposure { path: vec!["postmortem".into(), "check".into()] }),
                },
                tags: ["post-mortem", "ci", "verification"],
                handler: check,
            },
            capability! {
                id: "postmortems.owed",
                title: "The trunk's failed runs no post-mortem answers",
                description: "Asks the forge, through its own command-line tool and with the caller's credential, for the newest runs of the trunk (branch `master`, 60 runs, unless named), and returns every run that ended in `failure` or `timed_out` since the rule came into force and that no record names in `runs[].id`, each with its page, its age and whether it is still within its grace (24 hours unless named). A cancelled, skipped or green run owes nothing. When the forge cannot be asked the answer is `observed: false` with the reason and owes nothing: an unobserved forge is said, never guessed. When `runs` is given the forge is not asked. Exit 0 when nothing is owed past its grace, 10 when something is.",
                input: PostMortemOwedInput,
                output: PostMortemOwed,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["postmortem".into(), "owed".into()] }),
                },
                tags: ["post-mortem", "ci", "forge"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: owed,
            },
            capability! {
                id: "postmortems.diagnose",
                title: "Which post-mortems a failure looks like",
                description: "Searches the text of a failure — a failing log, a job's summary, an excerpt — for the signals of every record, ignoring case and matching each signal as written. Every record with at least one signal in the text is returned with its status, cause, the signals found, its components, what fixed it, what now prevents it and what else it concerned, the record with the most signals first and among equals the newest. No weighting and no closeness: a record is named because its own words are in the log. Reads the index and nothing else.",
                input: PostMortemDiagnoseInput,
                output: PostMortemDiagnosis,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: super::mcp("majordomus_postmortems_diagnose"),
                    http: get("/api/v1/post-mortems/diagnose"),
                    cli: Some(CliExposure { path: vec!["postmortem".into(), "diagnose".into()] }),
                },
                tags: ["post-mortem", "ci", "diagnosis"],
                handler: diagnose,
            },
            capability! {
                id: "postmortems.draft",
                title: "Draft a post-mortem for one failed run",
                description: "Asks the forge, through its own command-line tool, for one run and its failed jobs, and returns the text of a `draft` record for it: the front matter the schema asks for, with the cause class `undetermined`, and the four required sections, each saying that nothing is established yet. The identity is `pm-`, the day of the run, the workflow and the run's identifier. The text is returned, with the path a record of that identity lives at and the records that already name the run; nothing is written. When `runs` is given the run is looked up there and the forge is not asked. Exit 0 when drafted, 12 when the forge could not be asked.",
                input: PostMortemDraftInput,
                output: PostMortemDraft,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["postmortem".into(), "draft".into()] }),
                },
                tags: ["post-mortem", "ci", "forge"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: draft,
            },
        ],
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::capability::model::{CapabilityKind, Effect, HttpMethod};
    use crate::synthetic::SyntheticRepository;
    use serde_json::json;

    /// The source class the records are discovered through, appended to a synthetic
    /// repository's own.
    const POST_MORTEM_SOURCES: &str = "
  - id: post-mortem
    kind: post-mortem
    discovery: vcs
    pathspec: ':(glob).ai/repo/post-mortems/*.md'
    required: false
";

    const BODY: &str = "## What failed\n\nIt failed.\n\n## Root cause\n\nA cause.\n\n## Fix\n\nA fix.\n\n## Prevention\n\nA test.\n";

    /// One record's file: `front` is everything between the identity and the body.
    pub(crate) fn record_file(id: &str, title: &str, front: &str) -> String {
        format!(
            "---\nschema: post-mortem/v1\nid: {id}\nkind: post-mortem\ntitle: \"{title}\"\n\
             description: \"What to take from {id}.\"\n{front}---\n\n{BODY}"
        )
    }

    /// A synthetic repository with two sound records: one resolved, naming run 100 and a
    /// document of the tree, and a draft naming run 200.
    pub(crate) fn recorded() -> SyntheticRepository {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).expect("the sources");
        text.push_str(POST_MORTEM_SOURCES);
        std::fs::write(&sources, text).expect("the sources");
        write(
            &repo,
            "pm-20261013-lease",
            &record_file(
                "pm-20261013-lease",
                "Two writers lost the lease",
                "status: resolved\ndate: 2026-10-13\nruns:\n  - workflow: validate\n    id: \"100\"\n\
                 \x20   commit: \"abcdef1\"\n    jobs: [\"rust (3)\"]\n    step: cargo test\n\
                 cause:\n  class: race\n  summary: \"Two writers, one file.\"\n\
                 components:\n  - file:docs/DOC_0.md\n  - rule:project.rule-0@1\n\
                 signals:\n  - id: lease-lost\n    text: \"the lease was lost\"\n\
                 \x20 - id: exit\n    text: \"exit code 101\"\n\
                 fixed_by:\n  - pr:1078\nprevention:\n  - test:docs/DOC_0.md\ntags: [lease]\n\
                 related: [\"rule:project.rule-1@1\"]\n",
            ),
        );
        write(
            &repo,
            "pm-20261014-validate-200",
            &record_file(
                "pm-20261014-validate-200",
                "validate run 200 ended in failure",
                "status: draft\ndate: 2026-10-14\nruns:\n  - workflow: validate\n    id: \"200\"\n\
                 \x20   commit: \"abcdef2\"\n    jobs: [\"structure\"]\n\
                 cause:\n  class: undetermined\n  summary: \"Not yet established.\"\n",
            ),
        );
        repo
    }

    pub(crate) fn write(repo: &SyntheticRepository, name: &str, text: &str) {
        let dir = repo.root().join(".ai/repo/post-mortems");
        std::fs::create_dir_all(&dir).expect("the directory");
        std::fs::write(dir.join(format!("{name}.md")), text).expect("a record");
    }

    fn forge_run(id: u64, conclusion: &str, at: &str, branch: &str) -> Value {
        json!({"databaseId": id, "workflowName": "validate", "conclusion": conclusion,
            "headSha": "0123456789abcdef0123456789abcdef01234567", "createdAt": at,
            "updatedAt": at, "headBranch": branch,
            "url": format!("https://forge.example/o/r/actions/runs/{id}")})
    }

    #[test]
    fn the_records_are_listed_newest_first_with_their_counts() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        assert!(
            ctx.index.diagnostics.iter().all(|d| !d
                .path
                .as_deref()
                .is_some_and(|p| p.contains("post-mortems"))),
            "the fixture's records are valid: {:?}",
            ctx.index.diagnostics
        );
        let listed = list(&ctx, Empty {}).unwrap();
        assert_eq!(listed.directory, ".ai/repo/post-mortems/");
        assert_eq!(listed.count, 2);
        assert!(listed.unreadable.is_empty());
        let ids: Vec<&str> = listed.records.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["pm-20261014-validate-200", "pm-20261013-lease"]);
        assert_eq!(
            listed.by_status,
            BTreeMap::from([("draft".into(), 1), ("resolved".into(), 1)])
        );
        assert_eq!(
            listed.by_cause,
            BTreeMap::from([("race".into(), 1), ("undetermined".into(), 1)])
        );
        let lease = &listed.records[1];
        assert_eq!(lease.uri, "majordomus://post-mortem/pm-20261013-lease");
        assert!(
            ctx.index.get(&lease.uri).is_some(),
            "the URI is the index's"
        );
        assert_eq!(lease.title, "Two writers lost the lease");
        assert_eq!(lease.status, PostMortemStatus::Resolved);
        assert_eq!(lease.cause.summary, "Two writers, one file.");
        assert_eq!(lease.runs[0].id, "100");
        assert_eq!(lease.runs[0].jobs, ["rust (3)"]);
        assert_eq!(lease.signals, 2);
        assert_eq!(lease.fixed_by, ["pr:1078"]);
        assert_eq!(
            lease.components,
            ["file:docs/DOC_0.md", "rule:project.rule-0@1"]
        );
        assert_eq!(lease.prevention, ["test:docs/DOC_0.md"]);
        assert_eq!(lease.related, ["rule:project.rule-1@1"]);
        assert_eq!(lease.path, ".ai/repo/post-mortems/pm-20261013-lease.md");

        // through the executor, as every surface calls it
        let v = ctx.execute("postmortems.list", json!({})).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["records"][0]["status"], "draft");

        // a repository with no record answers an empty list, not an error
        let bare = SyntheticRepository::small().unwrap();
        let none = list(&bare.context().unwrap(), Empty {}).unwrap();
        assert_eq!(none.count, 0);
        assert!(none.records.is_empty() && none.by_status.is_empty());
    }

    #[test]
    fn the_check_judges_the_records_against_the_tree() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let sound = check(&ctx, Empty {}).unwrap();
        assert!(sound.ok, "{:?}", sound.findings);
        assert_eq!(sound.records, 2);

        // an analysed record with no signal, naming a path that is not there
        write(
            &repo,
            "pm-20261015-gone",
            &record_file(
                "pm-20261015-gone",
                "A cause in a file that is gone",
                "status: analysed\ndate: 2026-10-15\nruns:\n  - workflow: validate\n    id: \"300\"\n\
                 \x20   commit: \"abcdef3\"\n    jobs: [\"rust (1)\"]\n\
                 cause:\n  class: defect\n  summary: \"A defect.\"\n\
                 components:\n  - file:docs/NO_SUCH.md\n  - file:../outside.md\n  - rule:no.such-rule@1\n\
                 prevention:\n  - test:test/no_such_test.sh\n",
            ),
        );
        let ctx = repo.context().expect("a context");
        let broken = check(&ctx, Empty {}).unwrap();
        assert!(!broken.ok);
        assert_eq!(broken.records, 3);
        let codes: Vec<(&str, &str)> = broken
            .findings
            .iter()
            .map(|f| (f.record.as_str(), f.code.as_str()))
            .collect();
        assert_eq!(
            codes,
            [
                ("pm-20261015-gone", "no-signal"),
                ("pm-20261015-gone", "component-missing"),
                ("pm-20261015-gone", "component-missing"),
                ("pm-20261015-gone", "prevention-missing"),
            ],
            "a rule that does not resolve is the graph's finding, not this check's"
        );
        // and the graph does find it, through the table every kind's references go through
        let unresolved = crate::graph::unresolved_relations(&ctx.registry, &ctx.index.objects);
        let dangling: Vec<(&str, &str)> = unresolved
            .iter()
            .filter(|u| u.declared_in.ends_with("pm-20261015-gone.md"))
            .map(|u| (u.key.as_str(), u.reference.as_str()))
            .collect();
        assert_eq!(dangling, [("components", "rule:no.such-rule@1")]);
        let v = ctx.execute("postmortems.check", json!({})).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["findings"].as_array().unwrap().len(), 4);
    }

    /// The record is an object of the graph: what it names is reached from it, and it is
    /// reached from what it names, through the relations every kind declares in one table.
    #[test]
    fn a_record_is_traversable_from_both_ends() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let objects = &ctx.index.objects;
        assert_eq!(
            crate::graph::unresolved_relations(&ctx.registry, objects),
            vec![],
            "every reference of the fixture's records resolves"
        );
        let uri = "majordomus://post-mortem/pm-20261013-lease";
        let record = ctx.index.get(uri).expect("the record");
        let out: Vec<(String, String, bool)> = crate::entity::edges(&ctx.registry, objects, record)
            .into_iter()
            .map(|e| (e.edge, e.label, e.external))
            .collect();
        for expected in [
            ("failed_in", "project.rule-0@1", false),
            ("concerns", "project.rule-1@1", false),
            ("fixed_by", "pr:1078", true),
        ] {
            assert!(
                out.iter().any(|(edge, label, external)| {
                    (edge.as_str(), *external) == (expected.0, expected.2)
                        && label.contains(expected.1)
                }),
                "{expected:?} is not among {out:?}"
            );
        }
        // from the other end: the rule's page names the record that concerns it
        let rule = ctx
            .index
            .get("majordomus://rule/project.rule-1@1")
            .expect("the rule");
        let back = crate::entity::edges(&ctx.registry, objects, rule);
        assert!(
            back.iter()
                .any(|e| e.edge == "concerns" && e.uri.as_deref() == Some(uri)),
            "the rule does not lead back to the record"
        );
        // and the composed graph draws each relation as an edge of a kind it declares
        let graph = crate::graph::composed_complete(&ctx.registry, &ctx.index);
        for kind in ["failed_in", "guarded_by", "concerns", "fixed_by"] {
            assert!(graph.edge_kinds.contains_key(kind), "{kind} is undeclared");
            assert!(
                graph
                    .edges
                    .iter()
                    .any(|e| e.source == uri && e.kind == kind),
                "the record has no `{kind}` edge"
            );
        }
        assert!(graph
            .nodes
            .iter()
            .any(|n| n.id == "fix:pr:1078" && n.kind == "fix"));
    }

    #[test]
    fn a_path_is_in_the_tree_only_below_its_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/c.md"), "").unwrap();
        assert!(in_tree(dir.path(), "a/b/c.md") && in_tree(dir.path(), "a/b"));
        assert!(in_tree(dir.path(), "./a/b/c.md"));
        assert!(!in_tree(dir.path(), "a/b/d.md"));
        assert!(!in_tree(dir.path(), "a/../a/b/c.md"), "it climbs");
        assert!(!in_tree(
            dir.path(),
            &dir.path().join("a").to_string_lossy()
        ));
    }

    #[test]
    fn the_given_runs_are_compared_with_the_records_and_the_forge_is_not_asked() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let input = |now: &str| PostMortemOwedInput {
            now: Some(now.into()),
            runs: Some(vec![
                forge_run(100, "failure", "2026-10-13T05:00:00Z", "master"),
                forge_run(200, "failure", "2026-10-14T05:00:00Z", "master"),
                forge_run(400, "failure", "2026-10-15T05:00:00Z", "master"),
                forge_run(401, "timed_out", "2026-10-15T06:00:00Z", ""),
                forge_run(402, "cancelled", "2026-10-15T07:00:00Z", "master"),
                forge_run(403, "failure", "2026-10-15T08:00:00Z", "fix/x"),
                forge_run(90, "failure", "2026-10-12T05:00:00Z", "master"),
            ]),
            ..PostMortemOwedInput::default()
        };
        // a program that does not exist: were the forge asked, the answer would say so
        let soon = owed_with(
            "majordomus-no-such-gh-binary",
            &ctx,
            input("2026-10-15T12:00:00Z"),
        )
        .unwrap();
        assert!(soon.observed && soon.reason.is_none());
        assert_eq!(soon.source, "input");
        assert_eq!(
            (soon.branch.as_str(), soon.since.as_str()),
            ("master", "2026-10-13")
        );
        assert_eq!(soon.grace_hours, 24);
        assert_eq!(soon.runs, 6, "the run of another branch is not the trunk's");
        assert_eq!((soon.failed, soon.answered), (4, 2));
        let ids: Vec<&str> = soon.owed.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, ["401", "400"]);
        assert!(soon.owed.iter().all(|o| o.within_grace));
        assert_eq!((soon.past_grace, soon.exit_code), (0, 0));

        let late = owed_with(
            "majordomus-no-such-gh-binary",
            &ctx,
            input("2026-10-16T05:30:00Z"),
        )
        .unwrap();
        assert_eq!(late.owed[0].id, "401");
        assert!(late.owed[0].within_grace && !late.owed[1].within_grace);
        assert_eq!(late.owed[1].age_seconds, Some(24 * 3600 + 1800));
        assert_eq!((late.past_grace, late.exit_code), (1, EXIT_REFUSED));

        // a narrower window, a shorter grace, one workflow
        let narrowed = owed_with(
            "majordomus-no-such-gh-binary",
            &ctx,
            PostMortemOwedInput {
                since: Some("2026-10-15T05:30:00Z".into()),
                grace_hours: Some(0),
                workflows: vec!["validate".into()],
                ..input("2026-10-15T12:00:00Z")
            },
        )
        .unwrap();
        assert_eq!(narrowed.owed.len(), 1);
        assert_eq!((narrowed.past_grace, narrowed.exit_code), (1, EXIT_REFUSED));
        let other = owed_with(
            "majordomus-no-such-gh-binary",
            &ctx,
            PostMortemOwedInput {
                workflows: vec!["pages".into()],
                ..input("2026-10-15T12:00:00Z")
            },
        )
        .unwrap();
        assert_eq!((other.runs, other.failed), (0, 0));

        for bad in [
            PostMortemOwedInput {
                since: Some("last week".into()),
                ..Default::default()
            },
            PostMortemOwedInput {
                now: Some("noon".into()),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                owed_with("majordomus-no-such-gh-binary", &ctx, bad),
                Err(CapabilityError::InvalidInput(_))
            ));
        }
    }

    #[test]
    #[cfg(unix)]
    fn the_forge_is_asked_for_the_trunk_s_runs_and_an_unobserved_forge_owes_nothing() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let dir = tempfile::tempdir().unwrap();
        let program = forge::tests::fake_gh(dir.path());
        let program = program.to_str().unwrap();

        // no credential: said, with the reason, and nothing is owed
        let unseen = owed_with(program, &ctx, PostMortemOwedInput::default()).unwrap();
        assert!(!unseen.observed);
        assert!(unseen.reason.as_deref().unwrap().contains("gh auth login"));
        assert_eq!(unseen.source, "forge");
        assert!(unseen.owed.is_empty());
        assert_eq!(unseen.exit_code, 0);
        // the capability itself asks the forge's own tool, and says which of the two it was
        let asked = owed(&ctx, PostMortemOwedInput::default()).unwrap();
        assert_eq!(asked.observed, asked.reason.is_none());
        assert_eq!(asked.source, "forge");

        std::fs::write(
            dir.path().join("list.json"),
            json!([
                forge_run(100, "failure", "2026-10-13T05:00:00Z", "master"),
                forge_run(500, "failure", "2026-10-13T06:00:00Z", "master"),
            ])
            .to_string(),
        )
        .unwrap();
        let seen = owed_with(
            program,
            &ctx,
            PostMortemOwedInput {
                branch: Some("master".into()),
                limit: Some(100_000),
                now: Some("2026-10-20T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(seen.observed);
        assert_eq!(seen.owed.len(), 1);
        assert_eq!(seen.owed[0].id, "500");
        assert_eq!(seen.exit_code, EXIT_REFUSED);
        let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
        assert!(
            calls
                .lines()
                .last()
                .unwrap()
                .starts_with("run list --branch master --limit 500 "),
            "the limit is bounded: {calls}"
        );
    }

    #[test]
    fn a_failure_is_searched_for_the_signals_of_every_record() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let found = diagnose(
            &ctx,
            PostMortemDiagnoseInput {
                text: "error: The Lease Was Lost\nExit code 101".into(),
            },
        )
        .unwrap();
        assert_eq!((found.records, found.signals), (2, 2));
        assert_eq!(found.matches.len(), 1);
        assert_eq!(found.matches[0].id, "pm-20261013-lease");
        assert_eq!(found.matches[0].signals, ["lease-lost", "exit"]);
        assert_eq!(found.matches[0].fixed_by, ["pr:1078"]);
        assert_eq!(found.matches[0].related, ["rule:project.rule-1@1"]);
        assert_eq!(found.matches[0].prevention, ["test:docs/DOC_0.md"]);

        let v = ctx
            .execute(
                "postmortems.diagnose",
                json!({"text": "nothing anyone recorded"}),
            )
            .unwrap();
        assert_eq!(v["matches"], json!([]));
        assert!(matches!(
            diagnose(&ctx, PostMortemDiagnoseInput { text: " \n".into() }),
            Err(CapabilityError::Refused(_))
        ));

        // the benchmark cases name a signal the index holds, and one it does not
        let cases = PostMortemDiagnoseInput::benchmark_cases(&CaseContext { index: &ctx.index });
        assert_eq!(cases.len(), 2);
        let bare = SyntheticRepository::small().unwrap();
        let bare = bare.context().unwrap();
        assert_eq!(
            PostMortemDiagnoseInput::benchmark_cases(&CaseContext { index: &bare.index }).len(),
            1
        );
        assert!(
            PostMortemOwedInput::benchmark_cases(&CaseContext { index: &ctx.index }).is_empty()
        );
        assert!(
            PostMortemDraftInput::benchmark_cases(&CaseContext { index: &ctx.index }).is_empty()
        );
    }

    #[test]
    fn a_draft_is_made_from_a_given_run_and_names_who_already_answers_it() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let given = |run: &str| PostMortemDraftInput {
            run: run.into(),
            runs: Some(vec![
                forge_run(100, "failure", "2026-10-13T05:00:00Z", "master"),
                forge_run(600, "failure", "2026-10-16T05:00:00Z", "master"),
            ]),
            jobs: vec!["rust (3)".into()],
        };
        let drafted = draft_with("majordomus-no-such-gh-binary", &ctx, given("600")).unwrap();
        assert!(drafted.observed);
        assert_eq!(drafted.id.as_deref(), Some("pm-20261016-validate-600"));
        assert_eq!(
            drafted.path.as_deref(),
            Some(".ai/repo/post-mortems/pm-20261016-validate-600.md")
        );
        assert!(drafted.answered_by.is_empty());
        assert!(drafted.text.contains("status: draft\n"));
        assert!(drafted.text.contains("      - \"rust (3)\"\n"));
        assert_eq!(drafted.exit_code, 0);
        assert!(
            !repo.root().join(drafted.path.unwrap()).exists(),
            "a draft is text, never a file"
        );

        // saved where it says, the draft is a record the index reads and the rules accept
        write(&repo, "pm-20261016-validate-600", &drafted.text);
        let ctx = repo.context().expect("a context");
        let listed = list(&ctx, Empty {}).unwrap();
        assert_eq!(listed.records[0].id, "pm-20261016-validate-600");
        assert!(listed.unreadable.is_empty());
        assert!(check(&ctx, Empty {}).unwrap().ok);
        assert!(
            ctx.index.diagnostics.iter().all(|d| !d
                .path
                .as_deref()
                .is_some_and(|p| p.contains("post-mortems"))),
            "{:?}",
            ctx.index.diagnostics
        );

        let again = draft_with("majordomus-no-such-gh-binary", &ctx, given("100")).unwrap();
        assert_eq!(again.answered_by, ["pm-20261013-lease"]);

        assert!(matches!(
            draft_with("majordomus-no-such-gh-binary", &ctx, given("999")),
            Err(CapabilityError::NotFound(_))
        ));
        assert!(matches!(
            draft_with("majordomus-no-such-gh-binary", &ctx, given("--web")),
            Err(CapabilityError::InvalidInput(_))
        ));
        let undated = PostMortemDraftInput {
            run: "7".into(),
            runs: Some(vec![json!({"databaseId": 7, "conclusion": "failure"})]),
            jobs: Vec::new(),
        };
        assert!(matches!(
            draft_with("majordomus-no-such-gh-binary", &ctx, undated),
            Err(CapabilityError::Refused(_))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn a_draft_asks_the_forge_for_the_run_and_says_when_it_could_not() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let dir = tempfile::tempdir().unwrap();
        let program = forge::tests::fake_gh(dir.path());
        let program = program.to_str().unwrap();
        let ask = || PostMortemDraftInput {
            run: "700".into(),
            ..Default::default()
        };

        let unseen = draft_with(program, &ctx, ask()).unwrap();
        assert!(!unseen.observed && unseen.text.is_empty());
        assert_eq!(unseen.exit_code, EXIT_UNANSWERED);
        assert!(unseen.reason.unwrap().contains("gh auth login"));
        // the capability itself, where the forge knows nothing: the same receipt
        let asked = draft(&ctx, ask()).unwrap();
        assert!(!asked.observed);
        assert_eq!(asked.exit_code, EXIT_UNANSWERED);

        let mut run = forge_run(700, "failure", "2026-10-17T05:00:00Z", "master");
        run["jobs"] = json!([{"name": "structure", "conclusion": "failure"},
                             {"name": "rust (1)", "conclusion": "success"}]);
        std::fs::write(dir.path().join("view.json"), run.to_string()).unwrap();
        let drafted = draft_with(program, &ctx, ask()).unwrap();
        assert_eq!(drafted.id.as_deref(), Some("pm-20261017-validate-700"));
        assert!(drafted.text.contains("      - \"structure\"\n"));
        assert!(!drafted.text.contains("rust (1)"));
    }

    /// The asymmetry is the security property: the operations that ask the forge are on no
    /// surface a browser or an MCP client can call, and none of the five writes.
    #[test]
    fn only_the_reads_of_the_index_are_reachable_from_http_and_mcp() {
        let m = module();
        assert_eq!(m.id.as_str(), "postmortems");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|c| c.capability.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "postmortems.list",
                "postmortems.check",
                "postmortems.owed",
                "postmortems.diagnose",
                "postmortems.draft"
            ]
        );
        for e in &m.capabilities {
            let c = &e.capability;
            assert_eq!(c.kind, CapabilityKind::Query, "{}", c.id);
            assert_ne!(c.execution.effect, Effect::RepositoryMutation, "{}", c.id);
            let word = c.id.as_str().strip_prefix("postmortems.").unwrap();
            assert_eq!(c.exposure.cli.as_ref().unwrap().path, ["postmortem", word]);
            let asks_the_forge = matches!(word, "owed" | "draft");
            assert_eq!(c.exposure.http.is_none(), asks_the_forge, "{}", c.id);
            assert_eq!(c.exposure.mcp.is_none(), asks_the_forge, "{}", c.id);
            if let Some(http) = &c.exposure.http {
                assert_eq!(http.method, HttpMethod::Get);
                assert!(http.path.starts_with("/api/v1/post-mortems"));
            }
        }
    }
}
