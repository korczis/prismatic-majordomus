//! A check judged against the commit the change is built on (I4730).
//!
//! A gate that fails tells a worker that *something* is wrong, not that *their change* is.
//! When the trunk itself breaks a check — a whole-tree scan that flags a file another branch
//! added, a test that went red on the trunk an hour ago, a simulation whose outcome varies
//! from run to run — every change built on it fails that check for a reason that is not its
//! own, and every worker diagnoses the same failure again as a defect of their work.
//!
//! The answer is a comparison, not a better check: run the check over the change and over
//! the commit it is built on (its merge base with the trunk, or the trunk's tip), the same
//! declared number of times on each side, and compare what each side shows. This module is
//! that comparison and nothing else. It never runs the check: the caller — `finish` (I4413),
//! an adopter's gate script, a CI job — runs it, extracts what failed, and hands the runs
//! here. One judgement, so that two callers can never classify the same runs two ways.
//!
//! # The decision rule
//!
//! Per failure id, with `c`, `b` and `k` the number of change, base and control runs that
//! show it:
//!
//! - **introduced** — `c > 0`, `b = 0`: the change shows it and the base never did;
//! - **pre-existing** — `c > 0`, `b > 0`: the base shows it too, so it is the trunk's;
//! - **fixed** — `c = 0`, `b > 0`: the base shows it and the change never did;
//! - **unknown** — the base cannot be compared with (it was not run, or not the declared
//!   number of times), or the id is synthetic on both sides: a failure nobody could name
//!   (`exit:<code>`, `not-finished`) matching another failure nobody could name proves only
//!   that both sides failed. Unknown is never shown as pre-existing.
//!
//! Under the default rule, `subset` — the owner's word: *no failure the base does not also
//! show, judged over repeated runs* — only an introduced id blocks. Under `rate`, an id
//! blocks when the change shows it in more runs than the base or the control does, by more
//! than the declared slack; the control is a neutral perturbation of the base (a different
//! seed, an empty commit) and is the noise floor. The control never erases an introduced id
//! under `subset`, and under `rate` it lowers the bar only by the rate it measured.
//!
//! **flaky** is a property, never a class: an id seen in some but not all runs of a side. It
//! is reported and never turns an introduced id into anything else.
//!
//! # What the verdict says, and about what
//!
//! `clean` means the change shows nothing the base does not; `clean_with_trunk_failures`
//! says the same while the base itself is red, and carries the trunk alarm, so a
//! pre-existing failure is reported as the trunk's rather than passed in silence;
//! `introduced` means something blocks; `undecided` means the runs do not decide it, with
//! the reason. The denominator — the runs declared and seen on each side, the ids examined
//! and the rule — is in every report, and so is the provenance the caller supplied: a
//! verdict over runs of an unnamed commit is a verdict about nothing in particular.
//!
//! The judgement explains a failure; it never turns a red verification into a green one.
//! What `clean_with_trunk_failures` means for a pull request is the caller's policy.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The synthetic id of a run that did not finish (its exit is `null`): a timeout, a kill.
pub(crate) const NOT_FINISHED: &str = "not-finished";

/// The least number of runs per side the `rate` rule compares: below it a rate is a
/// presence, and the `subset` rule already says that more plainly.
pub(crate) const RATE_MIN_RUNS: u32 = 3;

/// One run of the check, as the caller observed it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineRun {
    /// The exit status; `null` when the run did not finish (it was killed, or timed out).
    pub exit: Option<i32>,
    /// The tree the run was taken over (`git rev-parse HEAD^{tree}`, or any identity the
    /// caller uses), when known. The runs of one side must all name the same tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<String>,
    /// What the run reported as failing, as the caller extracted it from the output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<BaselineFinding>,
}

/// One failure a run reported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineFinding {
    /// The failure's identity: a test name, a rule id and the file it flagged, a scenario.
    /// Two runs show the same failure when they report the same id.
    pub id: String,
    /// The repository-relative file the finding is about, when it is about one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The line in that file, when the check names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// The runs of one side: the change, the base or the control.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BaselineSide {
    /// The commit the side was run at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Every run of the check on this side.
    #[serde(default)]
    pub runs: Vec<BaselineRun>,
    /// Why the side could not be run at all (the base could not be exported, prepared or
    /// resolved): the side then decides nothing, and says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

/// How the runs decide.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BaselineRule {
    /// No failure the base does not also show: only an introduced id blocks.
    #[default]
    Subset,
    /// An id blocks when the change shows it in more runs than the base and the control by
    /// more than `slack`; needs at least three runs per side.
    Rate {
        /// The runs by which the change may exceed the noise floor without blocking.
        slack: u32,
    },
}

/// What the class of a failure id is, relative to the base.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BaselineClass {
    /// The change shows it; the base never did.
    Introduced,
    /// The base shows it too: the trunk's failure, inherited.
    PreExisting,
    /// The base shows it; the change never did.
    Fixed,
    /// The runs cannot tell: the base was not compared, or the id names no failure.
    Unknown,
}

/// The one word of the comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BaselineVerdict {
    /// The change shows nothing the base does not, and the base is green.
    Clean,
    /// The change shows nothing the base does not, and the base itself fails: see the
    /// trunk alarm. Not green.
    CleanWithTrunkFailures,
    /// At least one failure id blocks.
    Introduced,
    /// The runs do not decide it; `reasons` says why.
    Undecided,
}

/// How one failure id was judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaselineJudgement {
    /// The failure's identity.
    pub id: String,
    /// Its class relative to the base.
    pub class: BaselineClass,
    /// Whether it blocks under the declared rule.
    pub blocks: bool,
    /// The change runs that showed it.
    pub change: u32,
    /// The base runs that showed it.
    pub base: u32,
    /// The control runs that showed it, when a control was run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<u32>,
    /// Seen in some but not all runs of a side.
    pub flaky: bool,
    /// The id is one this module made up for a failure that named nothing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub synthetic: bool,
    /// The file the finding is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The line in that file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Whether that file is in the change set: `false` is "this finding is outside your
    /// diff". Absent when the finding names no file or no change set was established.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_diff: Option<bool>,
}

/// The denominator: what the verdict is a verdict about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaselineDenominator {
    /// The runs the caller declared per side.
    pub runs_declared: u32,
    /// The change runs supplied.
    pub change_runs: u32,
    /// The base runs supplied.
    pub base_runs: u32,
    /// The control runs supplied, when a control was run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_runs: Option<u32>,
    /// The distinct failure ids examined.
    pub ids_examined: u32,
    /// The rule, in words.
    pub rule: String,
    /// What the rule does not correct for.
    pub correction: String,
}

/// The base's own failures: the trunk is red, whatever the change did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrunkAlarm {
    /// The commit the base was run at, when the caller named it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_commit: Option<String>,
    /// Each failing id of the base, as `<id> (<b>/<N>)`.
    pub failing: Vec<String>,
}

/// Everything the comparison says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaselineReport {
    /// The one word.
    pub verdict: BaselineVerdict,
    /// Why the verdict is what it is, one line each; never empty when undecided.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    /// What the verdict is about.
    pub denominator: BaselineDenominator,
    /// The command the caller ran, when named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The commit the change side was run at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_commit: Option<String>,
    /// The commit the base side was run at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_commit: Option<String>,
    /// Every failure id, in id order.
    pub findings: Vec<BaselineJudgement>,
    /// Present whenever the base shows a failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk_alarm: Option<TrunkAlarm>,
    /// Introduced ids whose file is outside the change set: an environment or an ordering
    /// effect as often as a defect, reported rather than exempted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub introduced_outside_diff: Vec<String>,
}

/// A path as the change set spells it: forward slashes, no leading `./`.
pub(crate) fn normalise_path(path: &str) -> String {
    let p = path.replace('\\', "/");
    let mut s = p.as_str();
    while let Some(rest) = s.strip_prefix("./") {
        s = rest;
    }
    s.to_string()
}

/// Whether a path is in a change set. git reports a new directory nobody has added as the
/// directory (`src/`), not as the files inside it, so an entry ending in `/` holds every
/// path beneath it.
fn in_change_set(path: &str, set: &BTreeSet<String>) -> bool {
    set.contains(path)
        || set
            .iter()
            .any(|entry| entry.ends_with('/') && path.starts_with(entry.as_str()))
}

/// The ids one run shows, each with the first finding that named it; a run that failed and
/// named nothing shows `exit:<code>`, and a run that did not finish also shows
/// [`NOT_FINISHED`], whatever partial findings it left.
fn run_ids(run: &BaselineRun) -> BTreeMap<String, (bool, Option<&BaselineFinding>)> {
    let mut ids: BTreeMap<String, (bool, Option<&BaselineFinding>)> = BTreeMap::new();
    for f in &run.findings {
        ids.entry(f.id.clone()).or_insert((false, Some(f)));
    }
    match run.exit {
        None => {
            ids.entry(NOT_FINISHED.to_string()).or_insert((true, None));
        }
        Some(code) if code != 0 && run.findings.is_empty() => {
            ids.insert(format!("exit:{code}"), (true, None));
        }
        Some(_) => {}
    }
    ids
}

/// Why a side cannot be compared, if it cannot.
fn side_problem(name: &str, side: &BaselineSide, declared: u32) -> Option<String> {
    if let Some(reason) = &side.unavailable {
        return Some(format!("the {name} could not be run: {reason}"));
    }
    let seen = side.runs.len() as u32;
    if seen != declared {
        return Some(format!(
            "the {name} has {seen} run(s) where {declared} were declared; a side is compared \
             only over exactly the declared number"
        ));
    }
    let trees: BTreeSet<&str> = side.runs.iter().filter_map(|r| r.tree.as_deref()).collect();
    if trees.len() > 1 {
        return Some(format!(
            "the {name} runs span {} trees ({}); one side is one tree",
            trees.len(),
            trees.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    None
}

/// Count, per id, the runs of one side that show it.
fn counts(side: &BaselineSide) -> BTreeMap<String, u32> {
    let mut out: BTreeMap<String, u32> = BTreeMap::new();
    for run in &side.runs {
        for id in run_ids(run).into_keys() {
            *out.entry(id).or_insert(0) += 1;
        }
    }
    out
}

/// Judge the runs. `changed` is the change set the `in_diff` answers are taken over, when
/// one was established.
///
/// An input the comparison cannot mean anything over — no declared run, or the `rate` rule
/// over fewer than [`RATE_MIN_RUNS`] — is refused, so a caller learns of its mistake rather
/// than reading an answer to a question it did not ask.
pub(crate) fn judge(
    runs: u32,
    rule: BaselineRule,
    change: &BaselineSide,
    base: &BaselineSide,
    control: Option<&BaselineSide>,
    changed: Option<&BTreeSet<String>>,
) -> Result<BaselineReport, String> {
    if runs == 0 {
        return Err("runs must be at least 1: a comparison over no run decides nothing".into());
    }
    if let BaselineRule::Rate { .. } = rule {
        if runs < RATE_MIN_RUNS {
            return Err(format!(
                "the rate rule needs at least {RATE_MIN_RUNS} runs per side, {runs} declared: \
                 below that a rate is a presence, which the subset rule says plainly"
            ));
        }
    }

    let mut reasons: Vec<String> = Vec::new();
    let change_problem = side_problem("change", change, runs);
    let base_problem = side_problem("base", base, runs);
    let control_problem = control.and_then(|c| side_problem("control", c, runs));
    reasons.extend(change_problem.iter().cloned());
    reasons.extend(base_problem.iter().cloned());
    reasons.extend(control_problem.iter().cloned());

    let c_counts = counts(change);
    let b_counts = counts(base);
    let k_counts = control.map(counts);

    // the first finding that named each id, change side first: where it points
    let mut located: BTreeMap<String, (bool, Option<BaselineFinding>)> = BTreeMap::new();
    for run in change.runs.iter().chain(base.runs.iter()) {
        for (id, (synthetic, f)) in run_ids(run) {
            located.entry(id).or_insert((synthetic, f.cloned()));
        }
    }

    let mut findings: Vec<BaselineJudgement> = Vec::new();
    for (id, (synthetic, first)) in &located {
        let c = c_counts.get(id).copied().unwrap_or(0);
        let b = b_counts.get(id).copied().unwrap_or(0);
        let k = k_counts.as_ref().map(|m| m.get(id).copied().unwrap_or(0));
        if base_problem.is_some() && c == 0 {
            // a base that decides nothing fixes nothing either
            continue;
        }
        let class = if base_problem.is_some() {
            BaselineClass::Unknown
        } else if c > 0 && b == 0 {
            BaselineClass::Introduced
        } else if c > 0 && *synthetic {
            BaselineClass::Unknown
        } else if c > 0 {
            BaselineClass::PreExisting
        } else {
            BaselineClass::Fixed
        };
        let path = first
            .as_ref()
            .and_then(|f| f.path.as_deref())
            .map(normalise_path);
        let in_diff = match (&path, changed) {
            (Some(p), Some(set)) => Some(in_change_set(p, set)),
            _ => None,
        };
        let blocks = match (class, rule) {
            (BaselineClass::Introduced, BaselineRule::Subset) => true,
            // a finding in the change's own files is never the environment's noise
            (BaselineClass::Introduced, BaselineRule::Rate { .. }) if in_diff == Some(true) => true,
            (
                BaselineClass::Introduced | BaselineClass::PreExisting,
                BaselineRule::Rate { slack },
            ) => {
                let floor = b.max(k.unwrap_or(0));
                c > floor + slack
            }
            _ => false,
        };
        let flaky = [Some(c), Some(b), k]
            .into_iter()
            .flatten()
            .any(|n| n > 0 && n < runs);
        findings.push(BaselineJudgement {
            id: id.clone(),
            class,
            blocks,
            change: c,
            base: b,
            control: k,
            flaky,
            synthetic: *synthetic,
            line: first.as_ref().and_then(|f| f.line),
            path,
            in_diff,
        });
    }

    let trunk_alarm = if base_problem.is_none() {
        let failing: Vec<String> = findings
            .iter()
            .filter(|f| f.base > 0)
            .map(|f| format!("{} ({}/{runs})", f.id, f.base))
            .collect();
        (!failing.is_empty()).then(|| TrunkAlarm {
            base_commit: base.commit.clone(),
            failing,
        })
    } else {
        None
    };

    let introduced_outside_diff: Vec<String> = findings
        .iter()
        .filter(|f| f.class == BaselineClass::Introduced && f.in_diff == Some(false))
        .map(|f| f.id.clone())
        .collect();

    let blocking = findings.iter().filter(|f| f.blocks).count();
    let unknown: Vec<&str> = findings
        .iter()
        .filter(|f| f.class == BaselineClass::Unknown)
        .map(|f| f.id.as_str())
        .collect();
    let verdict = if change_problem.is_some() || control_problem.is_some() {
        BaselineVerdict::Undecided
    } else if blocking > 0 && base_problem.is_none() {
        reasons.push(format!(
            "{blocking} failure id(s) block under the {} rule",
            rule_words(rule)
        ));
        BaselineVerdict::Introduced
    } else if base_problem.is_some() {
        BaselineVerdict::Undecided
    } else if !unknown.is_empty() {
        reasons.push(format!(
            "{} failure(s) name nothing on either side, so they cannot be matched: {}",
            unknown.len(),
            unknown.join(", ")
        ));
        BaselineVerdict::Undecided
    } else if trunk_alarm.is_some() {
        BaselineVerdict::CleanWithTrunkFailures
    } else {
        BaselineVerdict::Clean
    };

    Ok(BaselineReport {
        verdict,
        reasons,
        denominator: BaselineDenominator {
            runs_declared: runs,
            change_runs: change.runs.len() as u32,
            base_runs: base.runs.len() as u32,
            control_runs: control.map(|c| c.runs.len() as u32),
            ids_examined: findings.len() as u32,
            rule: rule_words(rule),
            correction: "none: each failure id is judged on its own, so many ids raise the \
                         chance that one of them differs by chance"
                .into(),
        },
        command: None,
        change_commit: change.commit.clone(),
        base_commit: base.commit.clone(),
        findings,
        trunk_alarm,
        introduced_outside_diff,
    })
}

/// The rule, in the words a report prints.
fn rule_words(rule: BaselineRule) -> String {
    match rule {
        BaselineRule::Subset => "subset (no failure the base does not also show)".into(),
        BaselineRule::Rate { slack } => {
            format!("rate (blocks when the change shows an id in more than {slack} run(s) beyond the base and the control)")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(exit: Option<i32>, ids: &[&str]) -> BaselineRun {
        BaselineRun {
            exit,
            tree: None,
            findings: ids
                .iter()
                .map(|id| BaselineFinding {
                    id: (*id).into(),
                    path: None,
                    line: None,
                })
                .collect(),
        }
    }

    fn side(commit: &str, runs: Vec<BaselineRun>) -> BaselineSide {
        BaselineSide {
            commit: Some(commit.into()),
            runs,
            unavailable: None,
        }
    }

    fn class_of<'a>(r: &'a BaselineReport, id: &str) -> &'a BaselineJudgement {
        r.findings
            .iter()
            .find(|f| f.id == id)
            .unwrap_or_else(|| panic!("no {id} in {r:?}"))
    }

    #[test]
    fn a_failure_the_base_shows_too_is_the_trunks_and_does_not_block() {
        let change = side("c1", vec![run(Some(1), &["perf"]), run(Some(1), &["perf"])]);
        let base = side("b1", vec![run(Some(1), &["perf"]), run(Some(1), &["perf"])]);
        let r = judge(2, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert_eq!(r.verdict, BaselineVerdict::CleanWithTrunkFailures);
        let f = class_of(&r, "perf");
        assert_eq!(
            (f.class, f.blocks, f.flaky),
            (BaselineClass::PreExisting, false, false)
        );
        let alarm = r.trunk_alarm.expect("a red base raises the alarm");
        assert_eq!(alarm.base_commit.as_deref(), Some("b1"));
        assert_eq!(alarm.failing, ["perf (2/2)"]);
        assert_eq!(
            (r.change_commit.as_deref(), r.base_commit.as_deref()),
            (Some("c1"), Some("b1"))
        );
    }

    #[test]
    fn a_failure_only_the_change_shows_is_introduced_and_blocks_even_when_flaky() {
        let change = side(
            "c",
            vec![run(Some(1), &["parse", "perf"]), run(Some(1), &["perf"])],
        );
        let base = side("b", vec![run(Some(1), &["perf"]), run(Some(0), &[])]);
        let r = judge(2, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert_eq!(r.verdict, BaselineVerdict::Introduced);
        let parse = class_of(&r, "parse");
        assert_eq!(
            (parse.class, parse.blocks, parse.flaky),
            (BaselineClass::Introduced, true, true)
        );
        let perf = class_of(&r, "perf");
        assert_eq!(
            (perf.class, perf.blocks, perf.flaky),
            (BaselineClass::PreExisting, false, true)
        );
        assert!(
            r.reasons
                .iter()
                .any(|l| l.contains("1 failure id(s) block")),
            "{:?}",
            r.reasons
        );
    }

    #[test]
    fn a_green_base_and_a_green_change_are_clean_and_a_fix_is_named() {
        let change = side("c", vec![run(Some(0), &[])]);
        let base = side("b", vec![run(Some(0), &[])]);
        let r = judge(1, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert_eq!(r.verdict, BaselineVerdict::Clean);
        assert!(r.trunk_alarm.is_none() && r.findings.is_empty());

        let base = side("b", vec![run(Some(1), &["old"])]);
        let r = judge(1, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert_eq!(class_of(&r, "old").class, BaselineClass::Fixed);
        // a base that is red is red even when the change fixed it: the trunk still fails
        assert_eq!(r.verdict, BaselineVerdict::CleanWithTrunkFailures);
    }

    #[test]
    fn a_failure_that_names_nothing_on_both_sides_is_unknown_never_pre_existing() {
        let change = side("c", vec![run(Some(1), &[])]);
        let base = side("b", vec![run(Some(1), &[])]);
        let r = judge(1, BaselineRule::Subset, &change, &base, None, None).unwrap();
        let f = class_of(&r, "exit:1");
        assert_eq!(
            (f.class, f.synthetic, f.blocks),
            (BaselineClass::Unknown, true, false)
        );
        assert_eq!(r.verdict, BaselineVerdict::Undecided);
        assert!(
            r.reasons.iter().any(|l| l.contains("name nothing")),
            "{:?}",
            r.reasons
        );

        // another exit code is another failure: an OOM kill does not match a test failure
        let change = side("c", vec![run(Some(137), &[])]);
        let r = judge(1, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert_eq!(class_of(&r, "exit:137").class, BaselineClass::Introduced);
        assert_eq!(r.verdict, BaselineVerdict::Introduced);
    }

    #[test]
    fn a_run_that_did_not_finish_is_a_failure_of_its_own_whatever_it_left() {
        let change = side("c", vec![run(None, &["perf"])]);
        let base = side("b", vec![run(Some(1), &["perf"])]);
        let r = judge(1, BaselineRule::Subset, &change, &base, None, None).unwrap();
        let f = class_of(&r, NOT_FINISHED);
        assert_eq!(
            (f.class, f.blocks, f.synthetic),
            (BaselineClass::Introduced, true, true)
        );
        assert_eq!(class_of(&r, "perf").class, BaselineClass::PreExisting);
        assert_eq!(r.verdict, BaselineVerdict::Introduced);
    }

    #[test]
    fn a_base_that_could_not_be_run_decides_nothing_and_says_why() {
        let change = side("c", vec![run(Some(1), &["perf"])]);
        let base = BaselineSide {
            commit: Some("b".into()),
            runs: vec![],
            unavailable: Some("shallow clone: the merge base is not in this repository".into()),
        };
        let r = judge(1, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert_eq!(r.verdict, BaselineVerdict::Undecided);
        assert_eq!(class_of(&r, "perf").class, BaselineClass::Unknown);
        assert!(r.trunk_alarm.is_none());
        assert!(r.reasons[0].contains("shallow clone"), "{:?}", r.reasons);
    }

    #[test]
    fn a_side_is_compared_only_over_the_declared_runs_of_one_tree() {
        let change = side("c", vec![run(Some(0), &[])]);
        let base = side("b", vec![run(Some(1), &["perf"])]);
        // fewer change runs than declared
        let r = judge(2, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert_eq!(r.verdict, BaselineVerdict::Undecided);
        assert!(r
            .reasons
            .iter()
            .any(|l| l.contains("the change has 1 run(s) where 2")));
        // a base of the wrong size decides nothing, and fixes nothing
        let two = side("c", vec![run(Some(0), &[]), run(Some(0), &[])]);
        let r = judge(2, BaselineRule::Subset, &two, &base, None, None).unwrap();
        assert_eq!(r.verdict, BaselineVerdict::Undecided);
        assert!(r.findings.is_empty(), "{:?}", r.findings);
        // runs over two trees are two sides, not one
        let mut a = run(Some(0), &[]);
        a.tree = Some("t1".into());
        let mut b = run(Some(0), &[]);
        b.tree = Some("t2".into());
        let mixed = side("c", vec![a, b]);
        let base2 = side("b", vec![run(Some(0), &[]), run(Some(0), &[])]);
        let r = judge(2, BaselineRule::Subset, &mixed, &base2, None, None).unwrap();
        assert_eq!(r.verdict, BaselineVerdict::Undecided);
        assert!(
            r.reasons
                .iter()
                .any(|l| l.contains("span 2 trees (t1, t2)")),
            "{:?}",
            r.reasons
        );
        // and a control of the wrong size undecides too
        let bad_control = side("b", vec![run(Some(0), &[])]);
        let r = judge(
            2,
            BaselineRule::Subset,
            &two,
            &base2,
            Some(&bad_control),
            None,
        )
        .unwrap();
        assert_eq!(r.verdict, BaselineVerdict::Undecided);
        assert_eq!(r.denominator.control_runs, Some(1));
    }

    #[test]
    fn a_finding_outside_the_diff_is_said_to_be_and_an_introduced_one_is_listed() {
        let finding = |id: &str, path: &str| BaselineFinding {
            id: id.into(),
            path: Some(path.into()),
            line: Some(7),
        };
        let change = side(
            "c",
            vec![BaselineRun {
                exit: Some(1),
                tree: None,
                findings: vec![
                    finding("refs:docs/a.md", "./docs/a.md"),
                    finding("label:src/x.cpp", "src\\x.cpp"),
                ],
            }],
        );
        let base = side(
            "b",
            vec![BaselineRun {
                exit: Some(1),
                tree: None,
                findings: vec![finding("refs:docs/a.md", "docs/a.md")],
            }],
        );
        let changed: BTreeSet<String> = ["src/y.cpp".to_string()].into_iter().collect();
        let r = judge(
            1,
            BaselineRule::Subset,
            &change,
            &base,
            None,
            Some(&changed),
        )
        .unwrap();
        let refs = class_of(&r, "refs:docs/a.md");
        assert_eq!(
            (refs.class, refs.in_diff, refs.path.as_deref(), refs.line),
            (
                BaselineClass::PreExisting,
                Some(false),
                Some("docs/a.md"),
                Some(7)
            )
        );
        let label = class_of(&r, "label:src/x.cpp");
        assert_eq!(
            (label.class, label.in_diff, label.path.as_deref()),
            (BaselineClass::Introduced, Some(false), Some("src/x.cpp"))
        );
        assert_eq!(r.introduced_outside_diff, ["label:src/x.cpp"]);
        // with no change set, nothing is claimed about the diff
        let r = judge(1, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert!(r.findings.iter().all(|f| f.in_diff.is_none()));
    }

    #[test]
    fn under_the_rate_rule_the_noise_floor_is_the_base_or_the_control() {
        let fails = |n: usize, of: usize, id: &str| -> Vec<BaselineRun> {
            (0..of)
                .map(|i| {
                    if i < n {
                        run(Some(1), &[id])
                    } else {
                        run(Some(0), &[])
                    }
                })
                .collect()
        };
        let rule = BaselineRule::Rate { slack: 1 };
        // the change shows it 2 of 4, the base 0, the control 1: within the slack of the floor
        let change = side("c", fails(2, 4, "sim"));
        let base = side("b", fails(0, 4, "sim"));
        let control = side("b+seed", fails(1, 4, "sim"));
        let r = judge(4, rule, &change, &base, Some(&control), None).unwrap();
        let f = class_of(&r, "sim");
        assert_eq!(
            (f.class, f.blocks, f.control, f.flaky),
            (BaselineClass::Introduced, false, Some(1), true)
        );
        assert_eq!(r.verdict, BaselineVerdict::Clean);
        // 4 of 4 against a floor of 1 is past the slack, and blocks
        let change = side("c", fails(4, 4, "sim"));
        let r = judge(4, rule, &change, &base, Some(&control), None).unwrap();
        assert!(class_of(&r, "sim").blocks);
        assert_eq!(r.verdict, BaselineVerdict::Introduced);
        // a pre-existing id that got much worse blocks too
        let base = side("b", fails(1, 4, "sim"));
        let r = judge(4, rule, &change, &base, None, None).unwrap();
        let f = class_of(&r, "sim");
        assert_eq!((f.class, f.blocks), (BaselineClass::PreExisting, true));
        // and the control never excuses a finding in the change's own files
        let in_own = BaselineRun {
            exit: Some(1),
            tree: None,
            findings: vec![BaselineFinding {
                id: "x".into(),
                path: Some("src/x.rs".into()),
                line: None,
            }],
        };
        let change = side("c", vec![in_own, run(Some(0), &[]), run(Some(0), &[])]);
        let base = side("b", fails(0, 3, "x"));
        let control = side("k", fails(3, 3, "x"));
        let changed: BTreeSet<String> = ["src/x.rs".to_string()].into_iter().collect();
        let r = judge(
            3,
            BaselineRule::Rate { slack: 0 },
            &change,
            &base,
            Some(&control),
            Some(&changed),
        )
        .unwrap();
        assert!(class_of(&r, "x").blocks);
        assert!(r.denominator.rule.starts_with("rate"));
    }

    #[test]
    fn a_comparison_over_nothing_is_refused_rather_than_answered() {
        let s = side("c", vec![]);
        assert!(judge(0, BaselineRule::Subset, &s, &s, None, None)
            .unwrap_err()
            .contains("at least 1"));
        let e = judge(2, BaselineRule::Rate { slack: 0 }, &s, &s, None, None).unwrap_err();
        assert!(e.contains("at least 3 runs"), "{e}");
    }

    #[test]
    fn the_denominator_and_the_vocabulary_are_what_a_reader_sees() {
        let change = side("c", vec![run(Some(1), &["a", "a"])]);
        let base = side("b", vec![run(Some(0), &[])]);
        let r = judge(1, BaselineRule::Subset, &change, &base, None, None).unwrap();
        assert_eq!(
            class_of(&r, "a").change,
            1,
            "an id twice in one run is one run that shows it"
        );
        assert_eq!(r.denominator.ids_examined, 1);
        assert!(r.denominator.correction.starts_with("none"));
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["verdict"], "introduced");
        assert_eq!(json["findings"][0]["class"], "introduced");
        let rule: BaselineRule =
            serde_json::from_value(serde_json::json!({"rate": {"slack": 2}})).unwrap();
        assert_eq!(rule, BaselineRule::Rate { slack: 2 });
        let rule: BaselineRule = serde_json::from_value(serde_json::json!("subset")).unwrap();
        assert_eq!(rule, BaselineRule::Subset);
        assert_eq!(normalise_path("././a\\b.rs"), "a/b.rs");
        let set: BTreeSet<String> = ["src/".to_string(), "docs/a.md".to_string()]
            .into_iter()
            .collect();
        assert!(in_change_set("src/x/y.rs", &set) && in_change_set("docs/a.md", &set));
        assert!(!in_change_set("docs/b.md", &set) && !in_change_set("srcx/y.rs", &set));
    }
}
