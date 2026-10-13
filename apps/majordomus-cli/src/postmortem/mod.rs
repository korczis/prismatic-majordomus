//! Post-mortems: what turned the trunk's CI red, as records a later failure is matched
//! against.
//!
//! A record under `.ai/repo/post-mortems/` names the runs it answers, the cause as a class
//! and one sentence, the components the cause lives in, and the signals a failing log
//! shows. Its schema says what one record may hold; this module says what the schema cannot:
//!
//! - [`judge`] holds the rules that relate two fields of a record, a record to the tree, and
//!   one record to another. Whether a reference to an object of the layer resolves is not
//!   among them: `components`, `prevention` and `related` are relations of the graph
//!   (`crate::graph::RELATIONS`), and the check that refuses a dangling reference for every
//!   other kind refuses it here. That check reads a `file:` or `test:` reference as an edge
//!   out of the layer and asks nothing of the tree, so that one question is asked here;
//! - [`owed`] compares the trunk's failed runs with the runs the records name, and says
//!   which run nobody has answered and whether its grace has passed;
//! - [`diagnose`] searches a failing log for every record's signals;
//! - [`draft`] writes the text of a `draft` record for one failed run.
//!
//! Everything here is a function of what it is handed: the records as the index read them,
//! the runs as the forge printed them, the tree as a question, the clock as a number.
//! Reaching the forge is [`forge`], and nothing in this file calls it.

pub(crate) mod forge;

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::order::{OrderKey, Ordered};
use crate::peers::parse_rfc3339;

/// The kind a post-mortem is indexed as.
pub(crate) const KIND: &str = "post-mortem";

/// Where the records live, repository-relative.
pub(crate) const DIRECTORY: &str = ".ai/repo/post-mortems/";

/// The format version a draft is written in.
pub(crate) const SCHEMA: &str = "post-mortem/v1";

/// The prefixes of a reference that names a path of the tree rather than an object of the
/// layer. Every other prefix is a kind, and the graph resolves it.
pub(crate) const PATH_PREFIXES: &[&str] = &["file", "test"];

/// The conclusions of a run that owe a record. A cancelled or skipped run was stopped, not
/// failed, and a run that succeeded owes nothing.
pub(crate) const OWING_CONCLUSIONS: &[&str] = &["failure", "timed_out"];

/// The sentence a draft's section holds until someone establishes what belongs there.
pub(crate) const PLACEHOLDER: &str = "Not yet established.";

/// How far a record has come: recorded, explained, fixed.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PostMortemStatus {
    /// The failure is recorded and its cause is not established.
    Draft,
    /// The cause is established and named, with the components it lives in.
    Analysed,
    /// The fix is on the trunk and `fixed_by` names it.
    Resolved,
}

impl PostMortemStatus {
    /// The word the record carries.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PostMortemStatus::Draft => "draft",
            PostMortemStatus::Analysed => "analysed",
            PostMortemStatus::Resolved => "resolved",
        }
    }
}

/// Why a run failed, as a class.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CauseClass {
    /// The change under test was wrong.
    Defect,
    /// The test asserted something that was no longer, or never, true.
    Test,
    /// The outcome depended on timing.
    Race,
    /// The gate judged something other than what it names.
    Gate,
    /// The runner, a tool on it or the forge.
    Environment,
    /// Something outside the repository changed.
    Dependency,
    /// Not established: the only class a draft may carry.
    Undetermined,
}

impl CauseClass {
    /// The word the record carries.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            CauseClass::Defect => "defect",
            CauseClass::Test => "test",
            CauseClass::Race => "race",
            CauseClass::Gate => "gate",
            CauseClass::Environment => "environment",
            CauseClass::Dependency => "dependency",
            CauseClass::Undetermined => "undetermined",
        }
    }
}

/// The cause of a record: its class and one sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemCause {
    /// The class of the cause.
    pub(crate) class: CauseClass,
    /// The cause in one sentence.
    #[serde(default)]
    pub(crate) summary: String,
}

/// One run a record answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct AnsweredRun {
    /// The workflow's name as the forge reports it.
    #[serde(default)]
    pub(crate) workflow: String,
    /// The forge's identifier of the run: an identity, never a quantity.
    #[serde(deserialize_with = "identifier")]
    pub(crate) id: String,
    /// The commit the run judged.
    #[serde(default, deserialize_with = "identifier")]
    pub(crate) commit: String,
    /// The jobs that failed in it, by name.
    #[serde(default)]
    pub(crate) jobs: Vec<String>,
    /// The step that failed, where one job failed in one step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) step: Option<String>,
}

/// One recognisable trace of a failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemSignal {
    /// The signal's name within its record.
    pub(crate) id: String,
    /// The text a failing log shows, matched as written, ignoring case.
    #[serde(deserialize_with = "identifier")]
    pub(crate) text: String,
}

/// One post-mortem as the index read it: the front matter, typed, and where the file is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortem {
    /// The identity: `pm-`, the day the failure was first seen, and a slug.
    pub(crate) id: String,
    /// What went wrong, in one line.
    #[serde(default)]
    pub(crate) title: String,
    /// What a reader who meets the same failure should take from the record.
    #[serde(default)]
    pub(crate) description: String,
    /// How far the record has come.
    pub(crate) status: PostMortemStatus,
    /// The day the failure was first seen, `YYYY-MM-DD`.
    #[serde(default)]
    pub(crate) date: String,
    /// Every run the record answers.
    #[serde(default)]
    pub(crate) runs: Vec<AnsweredRun>,
    /// Why it failed.
    pub(crate) cause: PostMortemCause,
    /// Where the cause lives, as references of the layer: `file:<path>`, `rule:<identity>`.
    #[serde(default)]
    pub(crate) components: Vec<String>,
    /// What the failure looks like in a log.
    #[serde(default)]
    pub(crate) signals: Vec<PostMortemSignal>,
    /// What removed the cause: `commit:<sha>`, `pr:<number>`.
    #[serde(default)]
    pub(crate) fixed_by: Vec<String>,
    /// What now stops the cause from returning, as references of the layer.
    #[serde(default)]
    pub(crate) prevention: Vec<String>,
    /// What else of the layer the failure concerns: an issue, a milestone, a decision.
    #[serde(default)]
    pub(crate) related: Vec<String>,
    /// Free tags.
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    /// The record's file, repository-relative. Not part of the front matter: the reader
    /// fills it in.
    #[serde(default)]
    pub(crate) path: String,
}

impl PostMortem {
    /// Read one record from the front matter the index parsed, at `path`. A front matter
    /// that is not a post-mortem is refused with the reason, never read as a partial one.
    pub(crate) fn read(metadata: &Value, path: &str) -> Result<PostMortem, String> {
        let mut record: PostMortem =
            serde_json::from_value(metadata.clone()).map_err(|e| e.to_string())?;
        record.path = path.to_string();
        Ok(record)
    }

    /// The day of the record as one number, `YYYYMMDD`, and zero when it carries no
    /// readable day: what "newest first" is ordered by.
    fn day_number(&self) -> i64 {
        let digits: String = self.date.chars().filter(char::is_ascii_digit).collect();
        if digits.len() == 8 {
            digits.parse().unwrap_or(0)
        } else {
            0
        }
    }
}

/// Newest first, then by identity.
impl Ordered for PostMortem {
    fn order_key(&self) -> OrderKey<'_> {
        OrderKey::plain(&self.id, &self.id).ranked(-self.day_number())
    }
}

/// A scalar that is an identity or text and may have been written without quotes: the
/// layer's YAML reads an unquoted run of digits as a number, and a run identifier or a
/// commit made of digits alone is still a name.
fn identifier<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Ok(String::new()),
        other => Err(serde::de::Error::custom(format!(
            "expected a string, found {other}"
        ))),
    }
}

// ------------------------------------------------------------------------------- judge

/// One thing a record gets wrong that its schema cannot say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemFinding {
    /// The record's identity.
    pub(crate) record: String,
    /// A stable code naming the rule.
    pub(crate) code: String,
    /// What is wrong, in one sentence.
    pub(crate) message: String,
}

fn finding(record: &PostMortem, code: &str, message: String) -> PostMortemFinding {
    PostMortemFinding {
        record: record.id.clone(),
        code: code.to_string(),
        message,
    }
}

/// One line of the kind's section list, `share/sections/post-mortem.txt`: a heading the
/// body carries, at which level, and whether it must be there and must hold text. The list
/// is the projection `majordomus generate` writes from the schema's `Body`, so the sections
/// are named in the schema and nowhere in this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SectionRule {
    /// The number of leading hashes.
    pub(crate) level: usize,
    /// The heading's text.
    pub(crate) heading: String,
    /// The body must carry it.
    pub(crate) required: bool,
    /// It must hold something other than white space.
    pub(crate) non_empty: bool,
}

/// The section list as written: `<level>\t<heading>\t<required|optional>\t<non-empty|any>`,
/// one section per line, `#` comments skipped. A line that is not of that shape is skipped
/// too: the list is generated, and a reader that guessed at a malformed line would be
/// inventing a rule.
pub(crate) fn section_rules(text: &str) -> Vec<SectionRule> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut f = l.split('\t');
            let level = f.next()?.trim().parse().ok()?;
            let heading = f.next()?.trim().to_string();
            let required = f.next()?.trim() == "required";
            let non_empty = f.next()?.trim() == "non-empty";
            (!heading.is_empty()).then_some(SectionRule {
                level,
                heading,
                required,
                non_empty,
            })
        })
        .collect()
}

/// The body of one record against the kind's section list: a required section that is not
/// there, and a section that must hold text and holds none. A heading inside a fenced block
/// is text, not a heading.
pub(crate) fn judge_body(
    record: &str,
    body: &str,
    rules: &[SectionRule],
) -> Vec<PostMortemFinding> {
    // every heading of the body with whether anything but white space follows it
    let mut sections: Vec<(usize, String, bool)> = Vec::new();
    let mut fenced = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        let heading = (!fenced && hashes > 0 && line[hashes..].starts_with(' '))
            .then(|| line[hashes..].trim().to_string());
        match heading {
            Some(text) => sections.push((hashes, text, false)),
            None => {
                if let Some(last) = sections.last_mut() {
                    last.2 |= !line.trim().is_empty();
                }
            }
        }
    }
    let mut findings = Vec::new();
    for rule in rules {
        let found = sections
            .iter()
            .find(|(level, heading, _)| *level == rule.level && *heading == rule.heading);
        match found {
            None if rule.required => findings.push(PostMortemFinding {
                record: record.to_string(),
                code: "section-missing".to_string(),
                message: format!(
                    "the body carries no `{} {}` section, which the schema requires",
                    "#".repeat(rule.level),
                    rule.heading
                ),
            }),
            Some((_, _, false)) if rule.non_empty => findings.push(PostMortemFinding {
                record: record.to_string(),
                code: "section-empty".to_string(),
                message: format!(
                    "the section `{} {}` holds nothing: a section says what is known, or that nothing is established",
                    "#".repeat(rule.level),
                    rule.heading
                ),
            }),
            _ => {}
        }
    }
    findings
}

/// The path a reference names, when it names one: `file:<path>` and `test:<path>`. A
/// reference to an object of the layer names no path.
pub(crate) fn path_of(reference: &str) -> Option<&str> {
    reference
        .split_once(':')
        .filter(|(prefix, _)| PATH_PREFIXES.contains(prefix))
        .map(|(_, path)| path)
}

/// The rules that hold between the fields of a record, between a record and the tree, and
/// between records. `exists` answers whether a repository-relative path is in the tree.
///
/// The findings come in the order of the records handed in, and within a record in the
/// order the rules are written here.
pub(crate) fn judge(
    records: &[PostMortem],
    exists: &dyn Fn(&str) -> bool,
) -> Vec<PostMortemFinding> {
    let mut findings = Vec::new();
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for r in records {
        let undetermined = r.cause.class == CauseClass::Undetermined;
        let status = r.status.as_str();
        match r.status {
            PostMortemStatus::Draft if !undetermined => findings.push(finding(
                r,
                "draft-names-a-cause",
                format!(
                    "a draft carries the cause class `undetermined`, and this one says `{}`: a cause that is established makes the record `analysed`",
                    r.cause.class.as_str()
                ),
            )),
            PostMortemStatus::Analysed | PostMortemStatus::Resolved if undetermined => {
                findings.push(finding(
                    r,
                    "cause-undetermined",
                    format!("a record that is `{status}` names its cause, and this one's class is `undetermined`"),
                ))
            }
            _ => {}
        }
        if r.status != PostMortemStatus::Draft {
            if r.components.is_empty() {
                findings.push(finding(
                    r,
                    "no-component",
                    format!("a record that is `{status}` names at least one component the cause lives in"),
                ));
            }
            if r.signals.is_empty() {
                findings.push(finding(
                    r,
                    "no-signal",
                    format!("a record that is `{status}` carries at least one signal a failing log is searched for"),
                ));
            }
        }
        match (r.status, r.fixed_by.is_empty()) {
            (PostMortemStatus::Resolved, true) => findings.push(finding(
                r,
                "resolved-without-fix",
                "a resolved record names what removed the cause in `fixed_by`".to_string(),
            )),
            (PostMortemStatus::Draft | PostMortemStatus::Analysed, false) => {
                findings.push(finding(
                    r,
                    "fix-before-resolved",
                    format!("`fixed_by` is present exactly when the status is `resolved`, and this record is `{status}`"),
                ))
            }
            _ => {}
        }
        if r.status != PostMortemStatus::Resolved {
            for component in &r.components {
                if path_of(component).is_some_and(|path| !exists(path)) {
                    findings.push(finding(
                        r,
                        "component-missing",
                        format!("the component `{component}` names a path that is not in the tree"),
                    ));
                }
            }
        }
        for prevention in &r.prevention {
            if path_of(prevention).is_some_and(|path| !exists(path)) {
                findings.push(finding(
                    r,
                    "prevention-missing",
                    format!("the prevention `{prevention}` names a path that is not in the tree"),
                ));
            }
        }
        if let Some(first) = seen.get(r.id.as_str()) {
            findings.push(finding(
                r,
                "duplicate-id",
                format!(
                    "the identity `{}` is carried by `{first}` and by `{}`",
                    r.id, r.path
                ),
            ));
        } else {
            seen.insert(&r.id, &r.path);
        }
    }
    findings
}

// -------------------------------------------------------------------------------- owed

/// One run of a workflow as the forge reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ForgeRun {
    /// The workflow's name.
    pub(crate) workflow: String,
    /// The forge's identifier of the run, as a string.
    pub(crate) id: String,
    /// The commit the run judged.
    pub(crate) commit: String,
    /// The branch the run was for; empty when the forge did not say.
    #[serde(default)]
    pub(crate) branch: String,
    /// How the run ended (`failure`, `timed_out`, `cancelled`, `success`, ...); empty while
    /// it has not.
    pub(crate) conclusion: String,
    /// When the run was created, RFC 3339.
    pub(crate) created_at: String,
    /// When the run last changed, which for a finished run is when it finished, RFC 3339.
    pub(crate) updated_at: String,
    /// The run's page on the forge.
    #[serde(default)]
    pub(crate) url: String,
}

impl ForgeRun {
    /// One run from the shape `gh run list --json
    /// databaseId,workflowName,conclusion,headSha,createdAt,updatedAt,url,headBranch`
    /// prints. An entry without an identifier is no run.
    pub(crate) fn from_forge(v: &Value) -> Option<ForgeRun> {
        let text = |key: &str| v[key].as_str().unwrap_or_default().to_string();
        let id = match &v["databaseId"] {
            Value::Number(n) => n.to_string(),
            Value::String(s) if !s.is_empty() => s.clone(),
            _ => return None,
        };
        Some(ForgeRun {
            workflow: text("workflowName"),
            id,
            commit: text("headSha"),
            branch: text("headBranch"),
            conclusion: text("conclusion"),
            created_at: text("createdAt"),
            updated_at: text("updatedAt"),
            url: text("url"),
        })
    }

    /// Every run of a list the forge printed, in the order it printed them.
    pub(crate) fn list_from_forge(v: &Value) -> Vec<ForgeRun> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(ForgeRun::from_forge)
            .collect()
    }

    /// When the run finished, in Unix seconds: its last change, or its creation when the
    /// forge gave no later moment.
    fn finished(&self) -> Option<u64> {
        parse_rfc3339(&self.updated_at).or_else(|| parse_rfc3339(&self.created_at))
    }

    /// When the run was created, or failing that when it finished.
    fn created(&self) -> Option<u64> {
        parse_rfc3339(&self.created_at).or_else(|| parse_rfc3339(&self.updated_at))
    }
}

/// The runs of `branch`, and of `workflows` when any is named. A run the forge reported no
/// branch for is kept: the question that fetched it already named the branch.
pub(crate) fn of_trunk(runs: Vec<ForgeRun>, branch: &str, workflows: &[String]) -> Vec<ForgeRun> {
    runs.into_iter()
        .filter(|r| r.branch.is_empty() || r.branch == branch)
        .filter(|r| workflows.is_empty() || workflows.iter().any(|w| w == &r.workflow))
        .collect()
}

/// A failed run of the trunk that no record names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct OwedRun {
    /// The workflow's name.
    pub(crate) workflow: String,
    /// The forge's identifier of the run.
    pub(crate) id: String,
    /// The commit the run judged.
    pub(crate) commit: String,
    /// How the run ended.
    pub(crate) conclusion: String,
    /// The run's page on the forge.
    pub(crate) url: String,
    /// When the run finished, as the forge wrote it.
    pub(crate) finished_at: String,
    /// How long ago that was, in seconds; absent when the forge gave no readable time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) age_seconds: Option<u64>,
    /// Whether the run finished less than the grace ago: owed, and not yet late.
    pub(crate) within_grace: bool,
}

/// Newest first, then by identifier.
impl Ordered for OwedRun {
    fn order_key(&self) -> OrderKey<'_> {
        let age = self
            .age_seconds
            .map_or(i64::MAX, |a| i64::try_from(a).unwrap_or(i64::MAX));
        OrderKey::plain(&self.id, &self.id).ranked(age)
    }
}

/// Whether a conclusion owes a record.
pub(crate) fn owes(conclusion: &str) -> bool {
    OWING_CONCLUSIONS.contains(&conclusion)
}

/// The failed runs no record names, newest first.
///
/// `now`, `grace` and `since` are Unix seconds and a length in seconds. A run created before
/// `since` owes nothing: the rule does not reach back before it existed. A run that
/// finished less than `grace` ago is owed and within its grace. A run whose times the
/// forge did not give is owed and past its grace, because an age nobody can read is not an
/// age that excuses.
pub(crate) fn owed(
    runs: &[ForgeRun],
    records: &[PostMortem],
    now: u64,
    grace: u64,
    since: u64,
) -> Vec<OwedRun> {
    let mut out: Vec<OwedRun> = runs
        .iter()
        .filter(|run| owes(&run.conclusion))
        .filter(|run| run.created().is_none_or(|at| at >= since))
        .filter(|run| !answers(records, &run.id))
        .map(|run| {
            let age = run.finished().map(|at| now.saturating_sub(at));
            OwedRun {
                workflow: run.workflow.clone(),
                id: run.id.clone(),
                commit: run.commit.clone(),
                conclusion: run.conclusion.clone(),
                url: run.url.clone(),
                finished_at: run.updated_at.clone(),
                age_seconds: age,
                within_grace: age.is_some_and(|a| a < grace),
            }
        })
        .collect();
    crate::order::canonical(&mut out);
    out
}

/// Whether any record names the run `id`.
pub(crate) fn answers(records: &[PostMortem], id: &str) -> bool {
    records
        .iter()
        .any(|r| r.runs.iter().any(|run| run.id == id))
}

// ---------------------------------------------------------------------------- diagnose

/// A record whose signals a failure shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PostMortemMatch {
    /// The record's identity.
    pub(crate) id: String,
    /// The record's title.
    pub(crate) title: String,
    /// How far the record has come.
    pub(crate) status: PostMortemStatus,
    /// The day the failure was first seen.
    pub(crate) date: String,
    /// Why it failed then.
    pub(crate) cause: PostMortemCause,
    /// The signals of the record found in the text, by their ids, in the record's order.
    pub(crate) signals: Vec<String>,
    /// Where the cause lived.
    pub(crate) components: Vec<String>,
    /// What removed it, when it was removed.
    pub(crate) fixed_by: Vec<String>,
    /// What now stops it from returning.
    pub(crate) prevention: Vec<String>,
    /// What else of the layer it concerned.
    pub(crate) related: Vec<String>,
    /// The record's file.
    pub(crate) path: String,
    /// The record's day as one number: the second part of the rank.
    #[serde(skip)]
    day: i64,
}

/// By the number of signals found, most first; then newest; then by identity.
impl Ordered for PostMortemMatch {
    fn order_key(&self) -> OrderKey<'_> {
        // a day is below 10^8, so the count of signals always decides first
        let found = i64::try_from(self.signals.len().min(1_000_000)).unwrap_or(1_000_000);
        OrderKey::plain(&self.id, &self.id).ranked(-(found * 100_000_000 + self.day))
    }
}

/// Every record with at least one signal in `text`, the one with the most signals first and
/// among equals the newest. A signal is found when its text occurs in `text`, ignoring
/// case: no weighting, no closeness, no guess.
pub(crate) fn diagnose(text: &str, records: &[PostMortem]) -> Vec<PostMortemMatch> {
    let haystack = text.to_lowercase();
    let mut matches: Vec<PostMortemMatch> = records
        .iter()
        .filter_map(|r| {
            let found: Vec<String> = r
                .signals
                .iter()
                .filter(|s| !s.text.trim().is_empty())
                .filter(|s| haystack.contains(&s.text.to_lowercase()))
                .map(|s| s.id.clone())
                .collect();
            (!found.is_empty()).then(|| PostMortemMatch {
                id: r.id.clone(),
                title: r.title.clone(),
                status: r.status,
                date: r.date.clone(),
                cause: r.cause.clone(),
                signals: found,
                components: r.components.clone(),
                fixed_by: r.fixed_by.clone(),
                prevention: r.prevention.clone(),
                related: r.related.clone(),
                path: r.path.clone(),
                day: r.day_number(),
            })
        })
        .collect();
    crate::order::canonical(&mut matches);
    matches
}

// ------------------------------------------------------------------------------- draft

/// The identity a draft for `run` carries: `pm-`, the day of the run, the workflow and the
/// run's identifier. Refused when the forge gave the run no readable day.
pub(crate) fn draft_id(run: &ForgeRun) -> Result<String, String> {
    let day = day_of(run)?;
    Ok(format!(
        "pm-{}-{}-{}",
        day.replace('-', ""),
        slug(&run.workflow),
        run.id
    ))
}

/// The day of a run, `YYYY-MM-DD`, as the forge wrote it: no clock, no zone arithmetic.
fn day_of(run: &ForgeRun) -> Result<String, String> {
    [&run.created_at, &run.updated_at]
        .into_iter()
        .filter_map(|at| at.get(..10))
        .find(|day| {
            day.bytes().enumerate().all(|(i, b)| match i {
                4 | 7 => b == b'-',
                _ => b.is_ascii_digit(),
            })
        })
        .map(str::to_string)
        .ok_or_else(|| format!("the forge gave run {} no day it was created on", run.id))
}

/// A workflow's name as a slug: lower-case letters and digits, everything else one hyphen.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "run".to_string()
    } else {
        out.to_string()
    }
}

/// A string as a double-quoted scalar of the layer's YAML, on one line.
fn quoted(text: &str) -> String {
    let one_line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    format!("\"{}\"", one_line.replace('\\', "/").replace('"', "'"))
}

/// The text of a `draft` record for one failed run: the front matter the schema asks for
/// and the four sections it requires, each saying that nothing is established yet. It names
/// no component, no prevention and nothing related: those are findings of the analysis,
/// and a draft has made none. It is text to be reviewed and saved by whoever asked;
/// nothing here writes it anywhere.
///
/// Refused when the run carries no day or no commit, because a record without either is
/// not one the schema accepts and inventing them would be a guess.
pub(crate) fn draft(run: &ForgeRun, failed_jobs: &[String]) -> Result<String, String> {
    let id = draft_id(run)?;
    let day = day_of(run)?;
    let commit = run.commit.to_ascii_lowercase();
    if !(7..=40).contains(&commit.len()) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("the forge gave run {} no commit it judged", run.id));
    }
    let workflow = if run.workflow.trim().is_empty() {
        "unnamed workflow"
    } else {
        run.workflow.trim()
    };
    let conclusion = if run.conclusion.is_empty() {
        "failure"
    } else {
        &run.conclusion
    };
    let jobs: Vec<&str> = failed_jobs
        .iter()
        .map(|j| j.trim())
        .filter(|j| !j.is_empty())
        .collect();
    let mut text = String::new();
    text.push_str("---\n");
    text.push_str(&format!("schema: {SCHEMA}\n"));
    text.push_str(&format!("id: {id}\n"));
    text.push_str(&format!("kind: {KIND}\n"));
    text.push_str(&format!(
        "title: {}\n",
        quoted(&format!(
            "{workflow} run {} ended in {conclusion} on {}",
            run.id,
            &commit[..commit.len().min(12)]
        ))
    ));
    text.push_str(&format!(
        "description: {}\n",
        quoted("A draft: the run failed and its cause is not yet established.")
    ));
    text.push_str("status: draft\n");
    text.push_str(&format!("date: {day}\n"));
    text.push_str("runs:\n");
    text.push_str(&format!("  - workflow: {}\n", quoted(workflow)));
    text.push_str(&format!("    id: \"{}\"\n", run.id));
    text.push_str(&format!("    commit: \"{commit}\"\n"));
    text.push_str("    jobs:\n");
    if jobs.is_empty() {
        text.push_str("      - \"not reported by the forge\"\n");
    }
    for job in &jobs {
        text.push_str(&format!("      - {}\n", quoted(job)));
    }
    text.push_str("cause:\n");
    text.push_str("  class: undetermined\n");
    text.push_str(&format!("  summary: {}\n", quoted(PLACEHOLDER)));
    text.push_str("---\n\n");
    text.push_str("## What failed\n\n");
    let failed = if jobs.is_empty() {
        "the forge reported no failed job".to_string()
    } else {
        format!("failed: {}", jobs.join("; "))
    };
    text.push_str(&format!(
        "The `{workflow}` run {} on commit `{commit}` ended in `{conclusion}`; {failed}.",
        run.id
    ));
    if !run.url.is_empty() {
        text.push_str(&format!(" The run is at <{}>.", run.url));
    }
    text.push_str("\n\n");
    for heading in ["Root cause", "Fix", "Prevention"] {
        text.push_str(&format!("## {heading}\n\n{PLACEHOLDER}\n\n"));
    }
    text.truncate(text.trim_end().len());
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SECTIONS: &str = "# GENERATED\n2\tWhat failed\trequired\tnon-empty\n2\tRoot cause\trequired\tnon-empty\n2\tTimeline\toptional\tany\nnot a rule\n";

    #[test]
    fn the_section_list_is_read_as_it_is_generated() {
        let rules = section_rules(SECTIONS);
        assert_eq!(
            rules.len(),
            3,
            "the comment and the malformed line are skipped"
        );
        assert_eq!(rules[0].heading, "What failed");
        assert!(rules[0].required && rules[0].non_empty && rules[0].level == 2);
        assert!(!rules[2].required && !rules[2].non_empty);
    }

    #[test]
    fn a_body_with_every_required_section_is_accepted() {
        let body = "# Title\n\n## What failed\n\nIt fell.\n\n## Root cause\n\nGravity.\n";
        assert!(judge_body("pm-x", body, &section_rules(SECTIONS)).is_empty());
    }

    #[test]
    fn a_missing_required_section_is_a_finding() {
        let body = "# Title\n\n## What failed\n\nIt fell.\n";
        let found = judge_body("pm-x", body, &section_rules(SECTIONS));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "section-missing");
        assert!(found[0].message.contains("Root cause"));
        assert_eq!(found[0].record, "pm-x");
    }

    #[test]
    fn a_required_section_that_holds_nothing_is_a_finding() {
        let body = "## What failed\n\n   \n## Root cause\n\nGravity.\n";
        let found = judge_body("pm-x", body, &section_rules(SECTIONS));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "section-empty");
        assert!(found[0].message.contains("What failed"));
    }

    #[test]
    fn a_heading_at_another_level_or_inside_a_fence_is_not_the_section() {
        let body =
            "## What failed\n\nIt fell.\n\n```\n## Root cause\n```\n\n### Root cause\n\nNo.\n";
        let found = judge_body("pm-x", body, &section_rules(SECTIONS));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "section-missing");
    }

    #[test]
    fn an_optional_section_may_be_absent_or_empty() {
        let body = "## What failed\n\nIt fell.\n\n## Root cause\n\nGravity.\n\n## Timeline\n";
        assert!(judge_body("pm-x", body, &section_rules(SECTIONS)).is_empty());
    }

    const DAY: u64 = 86_400;

    /// A resolved record that every rule accepts, for a test to break one field of.
    fn record(id: &str) -> PostMortem {
        PostMortem::read(
            &json!({
                "schema": "post-mortem/v1", "id": id, "kind": "post-mortem",
                "title": format!("The cause of {id}"), "description": "d",
                "status": "resolved", "date": "2026-10-13",
                "runs": [{"workflow": "validate", "id": "100", "commit": "abcdef1",
                          "jobs": ["rust (3)"], "step": "cargo test"}],
                "cause": {"class": "race", "summary": "Two writers, one file."},
                "components": ["file:apps/majordomus-cli/src/lease.rs", "rule:project.alpha@1"],
                "signals": [{"id": "lease-lost", "text": "the lease was lost"}],
                "fixed_by": ["pr:1078"],
                "prevention": ["test:apps/majordomus-cli/tests/lease_lost.rs", "rule:project.alpha@1"],
                "related": ["issue:I2647"],
                "tags": ["lease"],
            }),
            &format!(".ai/repo/post-mortems/{id}.md"),
        )
        .expect("a record")
    }

    fn with(id: &str, change: impl FnOnce(&mut PostMortem)) -> PostMortem {
        let mut r = record(id);
        change(&mut r);
        r
    }

    fn draft_record(id: &str) -> PostMortem {
        with(id, |r| {
            r.status = PostMortemStatus::Draft;
            r.cause.class = CauseClass::Undetermined;
            r.fixed_by.clear();
        })
    }

    fn present(_: &str) -> bool {
        true
    }

    fn codes(records: &[PostMortem], exists: &dyn Fn(&str) -> bool) -> Vec<String> {
        judge(records, exists).into_iter().map(|f| f.code).collect()
    }

    #[test]
    fn a_record_is_read_from_its_front_matter_with_its_path() {
        let r = record("pm-20261013-lease");
        assert_eq!(r.path, ".ai/repo/post-mortems/pm-20261013-lease.md");
        assert_eq!(r.status, PostMortemStatus::Resolved);
        assert_eq!(r.cause.class, CauseClass::Race);
        assert_eq!(r.runs[0].step.as_deref(), Some("cargo test"));
        assert_eq!(r.signals[0].id, "lease-lost");
        // an identifier written without quotes arrives as a number and is still a name
        let unquoted = PostMortem::read(
            &json!({"id": "pm-20261013-x", "status": "draft",
                    "cause": {"class": "undetermined", "summary": "s"},
                    "runs": [{"id": 38359869013u64, "commit": 1234567}]}),
            "p.md",
        )
        .unwrap();
        assert_eq!(unquoted.runs[0].id, "38359869013");
        assert_eq!(unquoted.runs[0].commit, "1234567");
        assert!(unquoted.components.is_empty() && unquoted.title.is_empty());
        // what is not a post-mortem is refused, with the reason
        let refused = PostMortem::read(&json!({"id": "x", "status": "open"}), "p.md");
        assert!(refused.unwrap_err().contains("open"));
        assert!(PostMortem::read(
            &json!({"id": "x", "status": "draft",
            "cause": {"class": "undetermined"}, "runs": [{"id": ["a"]}]}),
            "p.md"
        )
        .unwrap_err()
        .contains("expected a string"));
    }

    #[test]
    fn a_sound_record_of_each_status_has_no_finding() {
        let records = [
            record("pm-20261013-a"),
            with("pm-20261013-b", |r| {
                r.status = PostMortemStatus::Analysed;
                r.fixed_by.clear();
            }),
            with("pm-20261013-c", |r| {
                r.status = PostMortemStatus::Draft;
                r.cause.class = CauseClass::Undetermined;
                r.fixed_by.clear();
                r.components.clear();
                r.signals.clear();
            }),
        ];
        assert_eq!(judge(&records, &present), vec![]);
    }

    #[test]
    fn a_draft_carries_no_cause_and_a_later_status_carries_one() {
        let named = with("pm-20261013-a", |r| {
            r.status = PostMortemStatus::Draft;
            r.fixed_by.clear();
        });
        let found = judge(std::slice::from_ref(&named), &present);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "draft-names-a-cause");
        assert_eq!(found[0].record, "pm-20261013-a");
        assert!(found[0].message.contains("`race`"), "{}", found[0].message);

        for status in [PostMortemStatus::Analysed, PostMortemStatus::Resolved] {
            let unnamed = with("pm-20261013-b", |r| {
                r.status = status;
                r.cause.class = CauseClass::Undetermined;
                if status == PostMortemStatus::Analysed {
                    r.fixed_by.clear();
                }
            });
            assert_eq!(codes(&[unnamed], &present), ["cause-undetermined"]);
        }
    }

    #[test]
    fn an_explained_record_names_a_component_and_a_signal() {
        for status in [PostMortemStatus::Analysed, PostMortemStatus::Resolved] {
            let bare = with("pm-20261013-a", |r| {
                r.status = status;
                r.components.clear();
                r.signals.clear();
                if status == PostMortemStatus::Analysed {
                    r.fixed_by.clear();
                }
            });
            assert_eq!(codes(&[bare], &present), ["no-component", "no-signal"]);
        }
        let only_signal = with("pm-20261013-b", |r| r.components.clear());
        assert_eq!(codes(&[only_signal], &present), ["no-component"]);
        let only_component = with("pm-20261013-c", |r| r.signals.clear());
        assert_eq!(codes(&[only_component], &present), ["no-signal"]);
    }

    #[test]
    fn a_fix_is_named_exactly_when_the_record_is_resolved() {
        let unfixed = with("pm-20261013-a", |r| r.fixed_by.clear());
        assert_eq!(codes(&[unfixed], &present), ["resolved-without-fix"]);
        let early = with("pm-20261013-b", |r| r.status = PostMortemStatus::Analysed);
        let found = judge(&[early], &present);
        assert_eq!(found[0].code, "fix-before-resolved");
        assert!(found[0].message.contains("`analysed`"));
        let draft = with("pm-20261013-c", |r| {
            r.status = PostMortemStatus::Draft;
            r.cause.class = CauseClass::Undetermined;
        });
        assert_eq!(codes(&[draft], &present), ["fix-before-resolved"]);
    }

    #[test]
    fn a_path_component_is_in_the_tree_until_the_record_is_resolved() {
        let absent = |path: &str| !path.ends_with("lease.rs");
        let analysed = with("pm-20261013-a", |r| {
            r.status = PostMortemStatus::Analysed;
            r.fixed_by.clear();
        });
        let found = judge(std::slice::from_ref(&analysed), &absent);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "component-missing");
        assert!(found[0]
            .message
            .contains("`file:apps/majordomus-cli/src/lease.rs`"));
        // a draft is held to it too
        let mut draft = draft_record("pm-20261013-b");
        draft.signals.clear();
        assert_eq!(codes(&[draft], &absent), ["component-missing"]);
        // a resolved record may name a file its fix removed
        assert_eq!(codes(&[record("pm-20261013-c")], &absent), [] as [&str; 0]);
        // and a reference to an object of the layer is the graph's to resolve, never the tree's
        let nothing = |_: &str| false;
        let objects = with("pm-20261013-d", |r| {
            r.status = PostMortemStatus::Analysed;
            r.fixed_by.clear();
            r.prevention.clear();
            r.components = vec!["rule:project.alpha@1".into(), "adr:0154".into()];
            r.related = vec!["issue:I0000".into(), "milestone:m".into()];
        });
        assert_eq!(codes(&[objects], &nothing), [] as [&str; 0]);
        // a test named as a component is a path too
        let by_test = with("pm-20261013-e", |r| {
            r.status = PostMortemStatus::Analysed;
            r.fixed_by.clear();
            r.prevention.clear();
            r.components = vec!["test:test/cases/1_x.sh".into()];
        });
        assert_eq!(codes(&[by_test], &nothing), ["component-missing"]);
        assert_eq!(path_of("file:lib/a.sh"), Some("lib/a.sh"));
        assert_eq!(path_of("test:test/cases/1:x.sh"), Some("test/cases/1:x.sh"));
        assert_eq!(path_of("rule:project.alpha@1"), None);
        assert_eq!(path_of("lib/a.sh"), None);
    }

    #[test]
    fn a_prevention_that_names_a_path_names_one_in_the_tree() {
        let absent = |path: &str| !path.ends_with("lease_lost.rs");
        let found = judge(&[record("pm-20261013-a")], &absent);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "prevention-missing");
        assert!(found[0]
            .message
            .contains("test:apps/majordomus-cli/tests/lease_lost.rs"));
        // a file named there is held to the same, resolved or not
        let nothing = |_: &str| false;
        let by_file = with("pm-20261013-b", |r| {
            r.prevention = vec!["file:scripts/x".into()]
        });
        assert_eq!(codes(&[by_file], &nothing), ["prevention-missing"]);
        // a rule or an issue named there is not a path and is not asked of the tree
        let other = with("pm-20261013-c", |r| {
            r.prevention = vec!["rule:x@1".into(), "issue:I1".into()];
        });
        assert_eq!(codes(&[other], &nothing), [] as [&str; 0]);
    }

    #[test]
    fn two_records_share_no_identity_and_may_share_a_run() {
        let first = record("pm-20261013-a");
        let second = with("pm-20261013-a", |r| r.path = "elsewhere.md".into());
        let found = judge(&[first.clone(), second], &present);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "duplicate-id");
        assert!(found[0].message.contains("elsewhere.md"));
        assert!(found[0].message.contains("pm-20261013-a.md"));
        // one run can fail for two causes: both records name run 100
        let other = record("pm-20261013-b");
        assert_eq!(first.runs[0].id, other.runs[0].id);
        assert_eq!(judge(&[first, other], &present), vec![]);
    }

    fn run(id: &str, conclusion: &str, finished: &str) -> ForgeRun {
        ForgeRun::from_forge(&json!({
            "databaseId": id.parse::<u64>().unwrap(), "workflowName": "validate",
            "conclusion": conclusion, "headSha": "0123456789abcdef0123456789abcdef01234567",
            "createdAt": finished, "updatedAt": finished, "headBranch": "master",
            "url": format!("https://forge.example/o/r/actions/runs/{id}"),
        }))
        .expect("a run")
    }

    #[test]
    fn a_failed_run_is_owed_until_a_record_names_it() {
        let now = parse_rfc3339("2026-10-15T12:00:00Z").unwrap();
        let since = parse_rfc3339("2026-10-13T00:00:00Z").unwrap();
        let runs = [
            run("100", "failure", "2026-10-14T00:00:00Z"), // answered by the record
            run("101", "failure", "2026-10-15T06:00:00Z"), // six hours ago
            run("102", "timed_out", "2026-10-13T06:00:00Z"), // two days ago
            run("103", "cancelled", "2026-10-14T00:00:00Z"),
            run("104", "skipped", "2026-10-14T00:00:00Z"),
            run("105", "success", "2026-10-14T00:00:00Z"),
            run("106", "failure", "2026-10-12T23:59:59Z"), // before the rule existed
            run("107", "", "2026-10-15T11:00:00Z"),        // still running
        ];
        let records = [record("pm-20261013-a")];
        let owing = owed(&runs, &records, now, DAY, since);
        let rows: Vec<(&str, bool, Option<u64>)> = owing
            .iter()
            .map(|o| (o.id.as_str(), o.within_grace, o.age_seconds))
            .collect();
        assert_eq!(
            rows,
            [
                ("101", true, Some(6 * 3600)),
                ("102", false, Some(2 * DAY + 6 * 3600))
            ],
            "newest first; the answered, the stopped, the green and the old owe nothing"
        );
        assert_eq!(owing[1].conclusion, "timed_out");
        assert_eq!(owing[0].url, "https://forge.example/o/r/actions/runs/101");
        assert_eq!(owing[0].finished_at, "2026-10-15T06:00:00Z");

        // with no record, the answered run is owed too
        let ids: Vec<String> = owed(&runs, &[], now, DAY, since)
            .into_iter()
            .map(|o| o.id)
            .collect();
        assert_eq!(ids, ["101", "100", "102"]);
        // the grace is a length: exactly at it, the run is late
        let at_grace = owed(&runs[1..2], &[], now, 6 * 3600, since);
        assert!(!at_grace[0].within_grace);
        // and a later `since` reaches less far back
        assert!(owed(&runs, &[], now, DAY, now).is_empty());
    }

    #[test]
    fn a_run_without_a_readable_time_is_owed_and_late() {
        let mut undated = run("200", "failure", "2026-10-14T00:00:00Z");
        undated.created_at.clear();
        undated.updated_at = "yesterday".into();
        let owing = owed(&[undated], &[], 0, DAY, u64::MAX);
        assert_eq!(owing.len(), 1);
        assert_eq!(owing[0].age_seconds, None);
        assert!(!owing[0].within_grace);
    }

    #[test]
    fn the_forge_s_list_is_read_as_runs_of_the_trunk() {
        let printed = json!([
            {"databaseId": 1, "workflowName": "validate", "conclusion": "failure",
             "headSha": "abc", "createdAt": "c", "updatedAt": "u", "headBranch": "master",
             "url": "https://x/1"},
            {"databaseId": "2", "workflowName": "pages", "conclusion": "success",
             "headBranch": "fix/x"},
            {"databaseId": 3, "workflowName": "pages"},
            {"workflowName": "no identifier"},
            {"databaseId": ""}
        ]);
        let runs = ForgeRun::list_from_forge(&printed);
        let ids: Vec<&str> = runs.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["1", "2", "3"]);
        assert_eq!(runs[0].commit, "abc");
        assert_eq!(runs[0].url, "https://x/1");
        assert!(ForgeRun::list_from_forge(&json!({"not": "a list"})).is_empty());

        let trunk = of_trunk(runs.clone(), "master", &[]);
        let ids: Vec<&str> = trunk.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["1", "3"], "a run with no branch is kept");
        let pages = of_trunk(runs, "master", &["pages".to_string()]);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, "3");
        assert!(owes("failure") && owes("timed_out"));
        assert!(!owes("cancelled") && !owes("") && !owes("success"));
    }

    #[test]
    fn a_failure_is_matched_by_signals_most_first_then_newest() {
        let one = with("pm-20261001-old", |r| {
            r.date = "2026-10-01".into();
            r.signals = vec![PostMortemSignal {
                id: "lease".into(),
                text: "The Lease Was Lost".into(),
            }];
        });
        let newer = with("pm-20261013-new", |r| {
            r.signals = vec![
                PostMortemSignal {
                    id: "lease".into(),
                    text: "the lease was lost".into(),
                },
                PostMortemSignal {
                    id: "absent".into(),
                    text: "never printed".into(),
                },
            ];
        });
        let two = with("pm-20260901-two", |r| {
            r.date = "2026-09-01".into();
            r.status = PostMortemStatus::Analysed;
            r.fixed_by.clear();
            r.signals = vec![
                PostMortemSignal {
                    id: "lease".into(),
                    text: "lease was lost".into(),
                },
                PostMortemSignal {
                    id: "exit".into(),
                    text: "exit code 101".into(),
                },
                PostMortemSignal {
                    id: "blank".into(),
                    text: "   ".into(),
                },
            ];
        });
        let silent = with("pm-20261013-silent", |r| {
            r.signals = vec![PostMortemSignal {
                id: "other".into(),
                text: "something else entirely".into(),
            }];
        });
        let log = "step 4\nerror: THE LEASE WAS LOST at 12:00\nprocess ended with Exit Code 101\n";
        let records = [one, newer, silent, two];
        let matches = diagnose(log, &records);
        let rows: Vec<(&str, Vec<&str>)> = matches
            .iter()
            .map(|m| {
                (
                    m.id.as_str(),
                    m.signals.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("pm-20260901-two", vec!["lease", "exit"]),
                ("pm-20261013-new", vec!["lease"]),
                ("pm-20261001-old", vec!["lease"]),
            ],
            "two signals outrank one however old; among equals the newest is first"
        );
        let top = &matches[0];
        assert_eq!(top.status, PostMortemStatus::Analysed);
        assert_eq!(top.cause.class, CauseClass::Race);
        assert_eq!(top.cause.summary, "Two writers, one file.");
        assert_eq!(top.title, "The cause of pm-20260901-two");
        assert!(top.fixed_by.is_empty());
        assert_eq!(matches[1].fixed_by, ["pr:1078"]);
        assert_eq!(top.components[0], "file:apps/majordomus-cli/src/lease.rs");
        assert_eq!(top.related, ["issue:I2647"]);
        assert_eq!(top.prevention[1], "rule:project.alpha@1");
        assert_eq!(top.path, ".ai/repo/post-mortems/pm-20260901-two.md");

        assert!(diagnose("a log that shows nothing known", &records).is_empty());
        assert!(diagnose("", &records).is_empty());
        assert!(diagnose(log, &[]).is_empty());
    }

    /// The front matter of a rendered record, parsed the way the index parses it.
    fn front_matter(text: &str) -> Value {
        let rest = text.strip_prefix("---\n").expect("front matter opens");
        let (front, _) = rest.split_once("\n---\n").expect("front matter closes");
        Value::Object(crate::metadata::yaml::parse_mapping(front).expect("the layer's YAML"))
    }

    #[test]
    fn a_draft_is_a_record_the_rules_accept() {
        let mut failed = run("38359869013", "failure", "2026-10-13T05:58:00Z");
        failed.workflow = "Deploy Pages".into();
        let jobs = [
            "rust (3)".to_string(),
            "site, links".to_string(),
            " ".to_string(),
        ];
        let text = draft(&failed, &jobs).unwrap();
        assert!(text.starts_with("---\nschema: post-mortem/v1\n"), "{text}");
        assert!(
            text.ends_with(&format!("## Prevention\n\n{PLACEHOLDER}\n")),
            "{text}"
        );

        let metadata = front_matter(&text);
        assert_eq!(metadata["kind"], "post-mortem");
        assert_eq!(
            metadata["runs"][0]["id"], "38359869013",
            "an identity, quoted"
        );
        let r = PostMortem::read(&metadata, "draft.md").expect("the draft reads back");
        assert_eq!(r.id, "pm-20261013-deploy-pages-38359869013");
        assert_eq!(r.id, draft_id(&failed).unwrap());
        assert_eq!(r.status, PostMortemStatus::Draft);
        assert_eq!(r.cause.class, CauseClass::Undetermined);
        assert_eq!(r.cause.summary, PLACEHOLDER);
        assert_eq!(r.date, "2026-10-13");
        assert_eq!(r.runs[0].workflow, "Deploy Pages");
        assert_eq!(r.runs[0].commit, failed.commit);
        assert_eq!(r.runs[0].jobs, ["rust (3)", "site, links"]);
        assert!(r.fixed_by.is_empty() && r.components.is_empty());
        assert!(r.prevention.is_empty() && r.related.is_empty());
        for field in ["components", "prevention", "related", "fixed_by", "signals"] {
            assert!(metadata.get(field).is_none(), "a draft names no {field}");
        }
        assert_eq!(judge(std::slice::from_ref(&r), &|_| false), vec![]);
        // and the run it was drafted for is answered by it
        assert!(owed(std::slice::from_ref(&failed), &[r], u64::MAX, 0, 0).is_empty());

        // the four sections the schema requires, each with a sentence
        let body = text.split("\n---\n").nth(1).unwrap();
        let headings: Vec<&str> = body.lines().filter(|l| l.starts_with("## ")).collect();
        assert_eq!(
            headings,
            ["## What failed", "## Root cause", "## Fix", "## Prevention"]
        );
        assert!(body.contains("ended in `failure`; failed: rust (3); site, links."));
        assert!(body.contains("<https://forge.example/o/r/actions/runs/38359869013>"));
        assert_eq!(body.matches(PLACEHOLDER).count(), 3);
    }

    #[test]
    fn a_draft_says_what_the_forge_did_not_and_invents_nothing() {
        let mut bare = run("7", "timed_out", "2026-10-13T05:58:00Z");
        bare.workflow = " ".into();
        bare.url.clear();
        bare.created_at.clear();
        let text = draft(&bare, &[]).unwrap();
        let r = PostMortem::read(&front_matter(&text), "d.md").unwrap();
        assert_eq!(r.id, "pm-20261013-run-7", "the day of its last change");
        assert_eq!(r.runs[0].jobs, ["not reported by the forge"]);
        assert_eq!(r.runs[0].workflow, "unnamed workflow");
        assert!(text.contains("ended in `timed_out`; the forge reported no failed job.\n"));
        assert!(!text.contains("The run is at"));

        let mut quoted_title = run("8", "failure", "2026-10-13T05:58:00Z");
        quoted_title.workflow = "say \"hi\"\\now\n".into();
        let r = PostMortem::read(&front_matter(&draft(&quoted_title, &[]).unwrap()), "d.md");
        assert_eq!(r.unwrap().id, "pm-20261013-say-hi-now-8");

        let mut undated = run("9", "failure", "soon");
        undated.updated_at = "2026/10/13".into();
        assert!(draft(&undated, &[]).unwrap_err().contains("no day"));
        assert!(draft_id(&undated).is_err());
        let mut uncommitted = run("10", "failure", "2026-10-13T05:58:00Z");
        uncommitted.commit = "HEAD".into();
        assert!(draft(&uncommitted, &[]).unwrap_err().contains("no commit"));
        assert_eq!(slug("--"), "run");
        assert_eq!(slug("CI / Rust (3)"), "ci-rust-3");
    }

    #[test]
    fn the_words_of_a_status_and_a_class_are_the_record_s() {
        for (status, word) in [
            (PostMortemStatus::Draft, "draft"),
            (PostMortemStatus::Analysed, "analysed"),
            (PostMortemStatus::Resolved, "resolved"),
        ] {
            assert_eq!(status.as_str(), word);
            assert_eq!(serde_json::to_value(status).unwrap(), word);
        }
        for class in [
            CauseClass::Defect,
            CauseClass::Test,
            CauseClass::Race,
            CauseClass::Gate,
            CauseClass::Environment,
            CauseClass::Dependency,
            CauseClass::Undetermined,
        ] {
            assert_eq!(serde_json::to_value(class).unwrap(), class.as_str());
        }
    }
}
