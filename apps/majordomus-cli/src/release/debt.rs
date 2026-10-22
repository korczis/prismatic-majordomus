//! The debt a release carries, counted, and held against the release before it.
//!
//! # What is counted
//!
//! A gate that could not be made to pass on the day it was written keeps a baseline: a file
//! of violations it accepts, and refuses anything beyond. Each of those gates is a ratchet
//! inside one tree — the list may not grow in a change. None of them said anything about
//! the list ever getting shorter, and nothing added them up, so the repository shipped
//! several releases a day over a debt that only had to stand still.
//!
//! The owner's rule (2026-10-08) is that it does not stand still: before every release the
//! repository's own violations converge to zero, and every release reduces them by at least
//! a minimum. This module is the measurement that rule is held by.
//!
//! # One declaration
//!
//! [`REGISTRY`] names every baseline that is debt, the gate that reads it and the form its
//! entries take. Two forms exist in this repository and both are declared, because a line
//! count is right for one and wrong for the other:
//!
//! - [`DebtForm::Entries`]: one violation per line (`file<TAB>command`, an id, a tag). The debt
//!   is the number of lines that are neither blank nor a comment.
//! - [`DebtForm::Counts`]: the file states numbers (`54`, `crate_sorts=90`, `missing 192`,
//!   `path 6`). The debt is their sum. Counted as lines, such a file could go from 54 to 500
//!   unseen, and paying it down from 192 to 1 would count as nothing.
//!
//! A file under the layer whose name says `baseline` and that the registry neither lists nor
//! excludes by name is itself a finding: a new baseline cannot arrive unseen.
//!
//! # The comparison
//!
//! The previous release's record carries what it measured ([`RecordedDebt`]). Against it:
//!
//! - no baseline may be larger than it was — in any change, not only at a release;
//! - at a release, the total must be lower by at least the declared minimum;
//! - a previous release that recorded nothing is said to be that: the counts are recorded
//!   and compared with nothing, which is a standing of its own and never a pass by silence;
//! - at zero nothing is owed.
//!
//! # Debt that cannot be paid
//!
//! One baseline counts what no change can fix: commits already published that break the
//! commit policy, in a history that is append-only. Held as owed, it would make "lower than
//! the last release" unreachable the day everything else was paid. So a declaration may say
//! `payable: false`, and must say why. Such a baseline is still counted, still recorded and
//! still refused if it grows; it is left out only of what a release must reduce and of
//! "nothing is owed". The reason is required, so that a baseline cannot be declared
//! unpayable to escape the rule.
//!
//! # From a declaration to a verdict
//!
//! ```
//! use majordomus_cli::release::debt::{measure, DebtStanding};
//!
//! let dir = tempfile::tempdir().unwrap();
//! std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
//! // one baseline: a list of two accepted violations
//! std::fs::write(
//!     dir.path().join(".ai/repo/list-baseline.txt"),
//!     "# accepted\ndocs/A.md\tmajordomus x\ndocs/B.md\tmajordomus y\n",
//! )
//! .unwrap();
//! std::fs::write(
//!     dir.path().join(".ai/repo/ci/debt.yaml"),
//!     "version: 1\nminimum_reduction: 1\nbaselines:\n  - id: list\n    path: .ai/repo/list-baseline.txt\n    gate: list-check\n    form: entries\n",
//! )
//! .unwrap();
//!
//! // no release is on record: the count is recorded and compared with nothing
//! let report = measure(dir.path(), &[]).unwrap();
//! assert_eq!((report.total, report.standing), (2, DebtStanding::Unrecorded));
//! assert!(report.change_allowed && report.release_allowed);
//! // what the release's record will carry, for the next release to be held against
//! assert_eq!(report.recorded().baselines["list"], 2);
//!
//! // a baseline nobody declared is a refusal, in any change
//! std::fs::write(dir.path().join(".ai/repo/new-baseline.txt"), "x\n").unwrap();
//! let report = measure(dir.path(), &[]).unwrap();
//! assert_eq!(report.standing, DebtStanding::Refused);
//! assert!(!report.change_allowed);
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::metadata::yaml;
use crate::model::Object;

/// Where the debt baselines are declared, relative to the repository root.
pub const REGISTRY: &str = ".ai/repo/ci/debt.yaml";

/// The directory searched for a baseline the registry does not know.
const LAYER: &str = ".ai/repo";

/// How a baseline states its debt.
///
/// ```
/// use majordomus_cli::release::debt::{count, DebtForm};
/// // the same two lines are two violations as a list and five as numbers
/// let text = "missing 2\nadopt 3\n";
/// assert_eq!(count(DebtForm::Entries, text), Ok(2));
/// assert_eq!(count(DebtForm::Counts, text), Ok(5));
/// assert_eq!(serde_json::to_string(&DebtForm::Counts).unwrap(), "\"counts\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DebtForm {
    /// One violation per line; blank lines and `#` comment lines are not entries.
    Entries,
    /// Numbers the file states, summed: the last integer of every line that is not a comment.
    Counts,
}

/// One declared baseline.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Declared {
    id: String,
    path: String,
    gate: String,
    form: DebtForm,
    /// False for debt no change can pay; `because` then says why.
    #[serde(default = "payable")]
    payable: bool,
    #[serde(default)]
    because: String,
    /// How its gate says the baseline is above the truth, when the gate can.
    #[serde(default)]
    slack: Option<Slack>,
    /// A line beginning with this is not an entry: the evidence baseline marks a guarantee
    /// that IS supported with `+`, beside the bare lines that are the debt.
    #[serde(default)]
    not_entries: Option<String>,
}

/// How a baseline's own gate says the baseline holds more than the tree owes.
///
/// A baseline is a number somebody wrote. When what it accepted is fixed and nobody writes
/// it again, it stays above the truth: the gate passes, and a release records a debt the
/// tree does not have. Only the gate knows the measured value, so the declaration names the
/// command to ask and the words it prints.
///
/// ```
/// use majordomus_cli::release::debt::Slack;
/// let slack: Slack = serde_json::from_str(
///     r#"{"command": "scripts/ci/pipefail-check", "says": "tighten the baseline"}"#,
/// )
/// .unwrap();
/// assert_eq!(slack.says, "tighten the baseline");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Slack {
    /// The command that measures the baseline, run from the repository root.
    pub command: String,
    /// The words it prints when the baseline can be tightened.
    pub says: String,
}

/// What a declaration that does not say is: debt a change can pay.
fn payable() -> bool {
    true
}

/// A file named like a baseline that is not debt, and why.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Excluded {
    path: String,
    because: String,
}

/// The registry file, as written.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    version: u32,
    minimum_reduction: u64,
    baselines: Vec<Declared>,
    #[serde(default)]
    not_debt: Vec<Excluded>,
    /// The tracked evidence ledger. Declared, a release may not be cut over evidence whose
    /// newest CI execution was recorded before the previous release was published.
    #[serde(default)]
    evidence_ledger: Option<String>,
}

/// How old the tracked evidence is, held against the release before.
///
/// A baseline says what is accepted; the evidence ledger says what was run. A ledger nobody
/// recorded since the last release supports that release's claims with a run of some earlier
/// tree, and the check that reads it passes while supporting none of them.
///
/// ```
/// use majordomus_cli::release::debt::EvidenceAge;
/// let age: EvidenceAge = serde_json::from_str(
///     r#"{"ledger": ".ai/repo/evidence/ledger.json", "newest": "2026-09-17T01:14:59Z", "released": "2026-10-08T11:00:00Z", "current": false}"#,
/// )
/// .unwrap();
/// assert!(!age.current, "recorded before the release it would stand behind");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceAge {
    /// The ledger the declaration names.
    pub ledger: String,
    /// When its newest execution recorded by CI was made; `None` when it holds none, or
    /// could not be read.
    pub newest: Option<String>,
    /// When the previous release was published; `None` when no release is recorded.
    pub released: Option<String>,
    /// Whether that execution is at least as new as that release.
    pub current: bool,
}

/// What a release measured, as its record carries it.
///
/// ```
/// use majordomus_cli::release::debt::RecordedDebt;
/// let recorded: RecordedDebt = serde_json::from_str(r#"{"total": 3, "baselines": {"order": 3}}"#).unwrap();
/// assert_eq!(recorded.total, 3);
/// assert_eq!(recorded.baselines["order"], 3);
/// assert_eq!(recorded.payable, None, "a record written before the split says nothing of it");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordedDebt {
    /// The sum over every baseline.
    pub total: u64,
    /// The part of it a change can pay: what the next release must carry less of. RecordedDebt
    /// for a reader; the comparison is made baseline by baseline, from the counts below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payable: Option<u64>,
    /// The ids of the baselines declared unpayable at that release, so that the release in
    /// which one was first declared so can be told from every later one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unpayable: Vec<String>,
    /// Each baseline's debt, by the id the registry gives it.
    pub baselines: BTreeMap<String, u64>,
}

/// One baseline as it stands in this tree: what the declaration says of it, what it holds
/// now, and what the previous release recorded for it. The pair of counts is what a reader
/// compares; whether the baseline grew is judged in [`measure`], not here.
///
/// ```
/// use majordomus_cli::release::debt::{measure, BaselineDebt};
/// # let dir = tempfile::tempdir().unwrap();
/// # std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/list-baseline.txt"), "# accepted\ndocs/A.md\tmajordomus x\ndocs/B.md\tmajordomus y\n").unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/order-baseline.txt"), "crate_sorts=3\n").unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/ci/debt.yaml"), "version: 1\nminimum_reduction: 1\nbaselines:\n  - id: list\n    path: .ai/repo/list-baseline.txt\n    gate: list-check\n    form: entries\n  - id: order\n    path: .ai/repo/order-baseline.txt\n    gate: order-check\n    form: counts\n    payable: false\n    because: sort sites a later release retires\n").unwrap();
/// let report = measure(dir.path(), &[]).unwrap();
/// let list: &BaselineDebt = &report.baselines[0];
/// assert_eq!((list.id.as_str(), list.gate.as_str(), list.count, list.previous), ("list", "list-check", 2, None));
/// assert!(list.payable && list.because.is_empty());
/// let order = &report.baselines[1];
/// assert_eq!((order.count, order.payable), (3, false));
/// assert_eq!(order.because, "sort sites a later release retires");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaselineDebt {
    /// The registry's id for it.
    pub id: String,
    /// The file, relative to the repository root.
    pub path: String,
    /// The gate that reads it.
    pub gate: String,
    /// How it states its debt.
    pub form: DebtForm,
    /// What it holds now.
    pub count: u64,
    /// What the previous release recorded for it; absent when that release recorded nothing
    /// for this id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<u64>,
    /// Whether a change can pay it. False for debt that is counted and may not grow, and
    /// that no release is asked to reduce.
    pub payable: bool,
    /// Why it cannot be paid; empty for one that can.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub because: String,
    /// How its gate states slack; absent for a gate that cannot, which a release names as
    /// not measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slack: Option<Slack>,
}

/// Where the tree stands against the previous release.
///
/// ```
/// use majordomus_cli::release::debt::{measure, DebtStanding};
/// # let dir = tempfile::tempdir().unwrap();
/// # std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/list-baseline.txt"), "# accepted\ndocs/A.md\tmajordomus x\ndocs/B.md\tmajordomus y\n").unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/order-baseline.txt"), "crate_sorts=3\n").unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/ci/debt.yaml"), "version: 1\nminimum_reduction: 1\nbaselines:\n  - id: list\n    path: .ai/repo/list-baseline.txt\n    gate: list-check\n    form: entries\n  - id: order\n    path: .ai/repo/order-baseline.txt\n    gate: order-check\n    form: counts\n    payable: false\n    because: sort sites a later release retires\n").unwrap();
/// // nothing on record to compare with: said, and not refused
/// assert_eq!(measure(dir.path(), &[]).unwrap().standing, DebtStanding::Unrecorded);
/// // every payable baseline empty: nothing is owed, though the unpayable one still counts 3
/// std::fs::write(dir.path().join(".ai/repo/list-baseline.txt"), "# accepted\n").unwrap();
/// let report = measure(dir.path(), &[]).unwrap();
/// assert_eq!((report.standing, report.total, report.payable), (DebtStanding::Paid, 3, 0));
/// assert_eq!(serde_json::to_string(&DebtStanding::Owed).unwrap(), "\"owed\"");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DebtStanding {
    /// Every baseline is empty: nothing is owed.
    Paid,
    /// The previous release recorded no counts: these are recorded and compared with nothing.
    Unrecorded,
    /// No baseline grew and the total is lower by at least the minimum.
    Reduced,
    /// No baseline grew, and the total is not yet lower by the minimum: a release owes it.
    Owed,
    /// A baseline grew, could not be read, or is not declared: refused in any change.
    Refused,
}

/// The measurement, and the verdict over it.
///
/// ```
/// use majordomus_cli::release::debt::{measure, DebtReport};
/// # let dir = tempfile::tempdir().unwrap();
/// # std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/list-baseline.txt"), "# accepted\ndocs/A.md\tmajordomus x\ndocs/B.md\tmajordomus y\n").unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/order-baseline.txt"), "crate_sorts=3\n").unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/ci/debt.yaml"), "version: 1\nminimum_reduction: 1\nbaselines:\n  - id: list\n    path: .ai/repo/list-baseline.txt\n    gate: list-check\n    form: entries\n  - id: order\n    path: .ai/repo/order-baseline.txt\n    gate: order-check\n    form: counts\n    payable: false\n    because: sort sites a later release retires\n").unwrap();
/// let report: DebtReport = measure(dir.path(), &[]).unwrap();
/// // the total is everything counted; the payable part is what a release must reduce
/// assert_eq!((report.total, report.payable, report.minimum_reduction), (5, 2, 1));
/// assert_eq!((report.previous_release.clone(), report.previous_total), (None, None));
/// assert!(report.findings[0].contains("2 is recorded now and compared with nothing"));
/// assert!(report.findings[0].contains("3 more cannot be paid and is counted, not owed"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DebtReport {
    /// Every declared baseline, in the registry's order.
    pub baselines: Vec<BaselineDebt>,
    /// The sum of what they hold now.
    pub total: u64,
    /// The part of the total a change can pay: what a release must carry less of.
    pub payable: u64,
    /// The release the counts are compared with, as a reader recognises it (`v0.17.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_release: Option<String>,
    /// What that release recorded in total; absent when it recorded nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_total: Option<u64>,
    /// What that release recorded for the baselines a change can pay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_payable: Option<u64>,
    /// How much lower than the previous release's total a release must be.
    pub minimum_reduction: u64,
    /// Where the tree stands.
    pub standing: DebtStanding,
    /// Whether a change may carry this tree: nothing grew, nothing is undeclared or unread.
    pub change_allowed: bool,
    /// Whether a release may be cut from this tree.
    pub release_allowed: bool,
    /// Every reason it is not, or the one sentence that says why it is.
    pub findings: Vec<String>,
    /// The findings that refuse any change: a baseline that grew or could not be counted,
    /// a file the declaration does not account for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusing: Vec<String>,
    /// The findings that refuse a release and not a change: a debt that did not fall,
    /// evidence older than the release before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub release_refusing: Vec<String>,
    /// The age of the tracked evidence, when the declaration names a ledger. Stale evidence
    /// refuses a release and never a change: a change cannot be asked to re-record it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<EvidenceAge>,
}

/// The debt one baseline's text states, in its declared form.
///
/// ```
/// use majordomus_cli::release::debt::{count, DebtForm};
/// let list = "# accepted on the day the gate landed\ndocs/A.md\tmajordomus x\n\nv0.1.0  # predates the records\n";
/// assert_eq!(count(DebtForm::Entries, list), Ok(2));
/// let numbers = "# counters\ncrate_sorts=90\nshell_sorts=0\nmissing 192\nsite/templates/a.html 6\n54\n";
/// assert_eq!(count(DebtForm::Counts, numbers), Ok(90 + 192 + 6 + 54));
/// // a line of a counts file that states no number is unreadable, never zero
/// assert!(count(DebtForm::Counts, "crate_sorts=many\n").is_err());
/// assert_eq!(count(DebtForm::Counts, "# nothing yet\n"), Ok(0));
/// ```
pub fn count(form: DebtForm, text: &str) -> Result<u64, String> {
    let mut total: u64 = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match form {
            DebtForm::Entries => total += 1,
            DebtForm::Counts => {
                // the statement, without a trailing comment
                let stated = line.split(" #").next().unwrap_or(line).trim_end();
                let digits: String = stated
                    .chars()
                    .rev()
                    .take_while(char::is_ascii_digit)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                let n: u64 = digits
                    .parse()
                    .map_err(|_| format!("`{line}` states no count"))?;
                total = total
                    .checked_add(n)
                    .ok_or_else(|| format!("`{line}` overflows the count"))?;
            }
        }
    }
    Ok(total)
}

/// `text` without the lines a declaration says are not entries.
fn entries_only(text: &str, not_entries: Option<&str>) -> String {
    match not_entries.filter(|mark| !mark.is_empty()) {
        None => text.to_string(),
        Some(mark) => text
            .lines()
            .filter(|line| !line.trim_start().starts_with(mark))
            .map(|line| format!("{line}\n"))
            .collect(),
    }
}

/// Every file under the layer whose name says `baseline`, relative to the root, sorted.
fn named_baselines(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, found: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, found);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains("baseline"))
            {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                found.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut found = Vec::new();
    walk(&root.join(LAYER), root, &mut found);
    // a set, so that the answer does not depend on the order the directory was listed in
    found
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The newest release on record: its name, the debt block it carries (an error when the
/// block is there and cannot be read), and when it was published.
type Previous = (String, Result<Option<RecordedDebt>, String>, Option<String>);

/// What the newest release record among `objects` recorded, with the release's name.
///
/// `None` when the layer holds no release record; `Some((release, None))` when the newest
/// one carries no counts.
fn previous(objects: &[Object]) -> Option<Previous> {
    type Record = (
        super::version::Version,
        String,
        Result<Option<RecordedDebt>, String>,
        Option<String>,
    );
    let records: Vec<Record> = objects
        .iter()
        .filter(|o| o.kind == super::changelog::RELEASE_KIND)
        .filter_map(|o| {
            let raw = o.metadata.get("version")?.as_str()?;
            let version = super::version::Version::parse(raw)?;
            let name = o
                .metadata
                .get("tag")
                .and_then(|t| t.as_str())
                .map_or_else(|| format!("v{raw}"), str::to_string);
            // a block that is there and cannot be read is not a release that recorded none
            let recorded = o
                .metadata
                .get("debt")
                .map(|d| {
                    serde_json::from_value::<RecordedDebt>(d.clone()).map_err(|e| e.to_string())
                })
                .transpose();
            let published = o
                .metadata
                .get("published_at")
                .and_then(|t| t.as_str())
                .map(str::to_string);
            Some((version, name, recorded, published))
        })
        .collect();
    // the newest by version; a maximum, not an order anybody is shown
    records
        .into_iter()
        .max_by_key(|(v, _, _, _)| (v.major, v.minor, v.patch))
        .map(|(_, name, recorded, published)| (name, recorded, published))
}

/// When the newest execution CI recorded in `ledger` was made.
fn newest_ci(ledger: &serde_json::Value) -> Option<String> {
    ledger
        .get("executions")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .filter(|e| e.get("origin").and_then(|o| o.as_str()) == Some("ci"))
        .filter_map(|e| e.get("at").and_then(|a| a.as_str()))
        .max()
        .map(str::to_string)
}

/// The age of the evidence in `ledger`, and the reason a release may not be cut over it.
///
/// Times are compared as the text both files write, `2026-10-08T18:49:19Z`; one written
/// another way is refused rather than compared wrongly.
fn evidence_age(
    root: &Path,
    ledger: &str,
    released: Option<&str>,
) -> (EvidenceAge, Option<String>) {
    let read = std::fs::read_to_string(root.join(ledger))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let newest = read.as_ref().and_then(newest_ci);
    // `2026-10-08T18:49:19Z` and nothing else: only that shape orders as text
    let comparable = |t: &str| {
        t.len() == 20
            && t.bytes()
                .zip("dddd-dd-ddTdd:dd:ddZ".bytes())
                .all(|(have, want)| match want {
                    b'd' => have.is_ascii_digit(),
                    other => have == other,
                })
    };
    let why = match (&read, newest.as_deref(), released) {
        (None, _, _) => Some(format!(
            "{ledger} cannot be read as an evidence ledger: nothing says the claims were run"
        )),
        (Some(_), None, _) => Some(format!(
            "{ledger} holds no execution recorded by CI: no claim is supported by a run"
        )),
        (Some(_), Some(at), Some(release)) if !comparable(at) || !comparable(release) => Some(format!(
            "{ledger}: `{at}` and `{release}` are not times this can compare"
        )),
        (Some(_), Some(at), Some(release)) if at < release => Some(format!(
            "the newest CI execution in {ledger} is {at}, before the previous release was published ({release}): record the evidence of this tree before the tag"
        )),
        _ => None,
    };
    (
        EvidenceAge {
            ledger: ledger.to_string(),
            newest,
            released: released.map(str::to_string),
            current: why.is_none(),
        },
        why,
    )
}

/// Measure the tree at `root` against the newest release record among `objects`.
///
/// An error is a registry that cannot be read at all: without it nothing is known to be
/// debt, and that is not a tree with none.
///
/// ```
/// use majordomus_cli::release::debt::measure;
/// let empty = tempfile::tempdir().unwrap();
/// assert!(measure(empty.path(), &[]).unwrap_err().contains("nothing is known to be debt"));
/// # let dir = tempfile::tempdir().unwrap();
/// # std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/list-baseline.txt"), "# accepted\ndocs/A.md\tmajordomus x\ndocs/B.md\tmajordomus y\n").unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/order-baseline.txt"), "crate_sorts=3\n").unwrap();
/// # std::fs::write(dir.path().join(".ai/repo/ci/debt.yaml"), "version: 1\nminimum_reduction: 1\nbaselines:\n  - id: list\n    path: .ai/repo/list-baseline.txt\n    gate: list-check\n    form: entries\n  - id: order\n    path: .ai/repo/order-baseline.txt\n    gate: order-check\n    form: counts\n    payable: false\n    because: sort sites a later release retires\n").unwrap();
/// let report = measure(dir.path(), &[]).unwrap();
/// assert_eq!(report.baselines.len(), 2);
/// assert!(report.release_allowed, "a first measurement is not refused");
/// ```
pub fn measure(root: &Path, objects: &[Object]) -> Result<DebtReport, String> {
    let path: PathBuf = root.join(REGISTRY);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("{REGISTRY} cannot be read ({e}); nothing is known to be debt"))?;
    let registry: Registry = yaml::parse_into(&text).map_err(|e| format!("{REGISTRY}: {e}"))?;
    if registry.version != 1 {
        return Err(format!(
            "{REGISTRY}: version {}, this reads 1",
            registry.version
        ));
    }
    if registry.minimum_reduction < 1 {
        return Err(format!(
            "{REGISTRY}: minimum_reduction is {}, and a release reduces the debt by at least 1",
            registry.minimum_reduction
        ));
    }
    // A release on record that states no time is held as a time that cannot be compared,
    // not as no release at all.
    let (previous, published, garbled) = match previous(objects) {
        Some((name, Ok(recorded), published)) => {
            (Some((name, recorded)), Some(published.unwrap_or_default()), None)
        }
        Some((name, Err(why), published)) => (
            Some((name.clone(), None)),
            Some(published.unwrap_or_default()),
            Some(format!(
                "{name} records a debt block this cannot read ({why}): what it carried is not known, which is not a release that carried none"
            )),
        ),
        None => (None, None, None),
    };
    let mut report = judge(
        root,
        &registry,
        previous,
        published.as_deref(),
        &named_baselines(root),
    );
    if let Some(why) = garbled {
        report.refusing.push(why.clone());
        report.findings.insert(0, why);
        report.standing = DebtStanding::Refused;
        report.change_allowed = false;
        report.release_allowed = false;
    }
    Ok(report)
}

/// The verdict over a registry, the files it names and what the previous release recorded.
fn judge(
    root: &Path,
    registry: &Registry,
    previous: Option<(String, Option<RecordedDebt>)>,
    published: Option<&str>,
    named: &[String],
) -> DebtReport {
    let (previous_release, recorded) = match previous {
        Some((name, recorded)) => (Some(name), recorded),
        None => (None, None),
    };
    let mut findings = Vec::new();
    let mut refused = false;
    let mut baselines = Vec::new();
    let mut total: u64 = 0;
    // what a change can pay, now and as the previous release recorded it
    let mut owed: u64 = 0;
    let mut owed_before: u64 = 0;

    for declared in &registry.baselines {
        if !declared.payable && declared.because.trim().is_empty() {
            refused = true;
            findings.push(format!(
                "{} is declared `payable: false` without a reason: a baseline is not left out of what a release owes unsaid",
                declared.path
            ));
        }
        let count = match std::fs::read_to_string(root.join(&declared.path)) {
            Ok(text) => match count(
                declared.form,
                &entries_only(&text, declared.not_entries.as_deref()),
            ) {
                Ok(n) => n,
                Err(why) => {
                    refused = true;
                    findings.push(format!("{}: {why}", declared.path));
                    0
                }
            },
            Err(e) => {
                refused = true;
                findings.push(format!(
                    "{}: declared in {REGISTRY} and cannot be read ({e})",
                    declared.path
                ));
                0
            }
        };
        let before = recorded
            .as_ref()
            .and_then(|r| r.baselines.get(&declared.id).copied());
        if let Some(before) = before.filter(|before| count > *before) {
            refused = true;
            findings.push(format!(
                "{} grew from {before} to {count} since {}: a baseline may only shrink (gate {})",
                declared.path,
                previous_release
                    .as_deref()
                    .unwrap_or("the previous release"),
                declared.gate
            ));
        }
        total = total.saturating_add(count);
        if declared.payable {
            owed = owed.saturating_add(count);
            owed_before = owed_before.saturating_add(before.unwrap_or(0));
        }
        baselines.push(BaselineDebt {
            id: declared.id.clone(),
            path: declared.path.clone(),
            gate: declared.gate.clone(),
            form: declared.form,
            count,
            previous: before,
            payable: declared.payable,
            because: if declared.payable {
                String::new()
            } else {
                declared.because.trim().to_string()
            },
            slack: declared.slack.clone(),
        });
    }

    for file in named {
        let known = registry.baselines.iter().any(|d| &d.path == file)
            || registry.not_debt.iter().any(|x| &x.path == file);
        if !known {
            refused = true;
            findings.push(format!(
                "{file} is named like a baseline and {REGISTRY} neither counts it nor says why it is not debt"
            ));
        }
    }
    for excluded in &registry.not_debt {
        if excluded.because.trim().is_empty() {
            refused = true;
            findings.push(format!(
                "{} is excluded from the debt without a reason",
                excluded.path
            ));
        }
    }

    // A baseline the previous release counted and the declaration no longer names has not
    // been paid: it has been dropped from the sum.
    for (id, before) in recorded.iter().flat_map(|r| r.baselines.iter()) {
        if *before > 0 && !registry.baselines.iter().any(|d| &d.id == id) {
            refused = true;
            findings.push(format!(
                "{} recorded {before} for `{id}` and {REGISTRY} no longer counts it: a baseline leaves the debt by reaching zero, not by leaving the declaration",
                previous_release.as_deref().unwrap_or("the previous release")
            ));
        }
    }
    // every finding so far is a reason no change may land
    let refusing = findings.clone();

    let previous_total = recorded.as_ref().map(|r| r.total);
    let previous_payable = recorded.as_ref().map(|_| owed_before);
    let minimum = registry.minimum_reduction;
    // what is counted and cannot be paid, said beside every verdict it is left out of
    let frozen = total - owed;
    let aside = if frozen == 0 {
        String::new()
    } else {
        format!("; {frozen} more cannot be paid and is counted, not owed")
    };
    let standing = if refused {
        DebtStanding::Refused
    } else if owed == 0 {
        findings.push(format!(
            "every baseline a change can pay is empty: nothing is owed{aside}"
        ));
        DebtStanding::Paid
    } else {
        match previous_payable {
            None => {
                findings.push(match &previous_release {
                    Some(release) => format!(
                        "{release} recorded no debt: {owed} is recorded now and compared with nothing{aside}"
                    ),
                    None => format!(
                        "no release is recorded: {owed} is recorded now and compared with nothing{aside}"
                    ),
                });
                DebtStanding::Unrecorded
            }
            Some(before) if owed.saturating_add(minimum) <= before => {
                findings.push(format!(
                    "the debt fell from {before} to {owed} since {}, by at least the minimum of {minimum}{aside}",
                    previous_release.as_deref().unwrap_or("the previous release")
                ));
                DebtStanding::Reduced
            }
            Some(before) => {
                findings.push(format!(
                    "the debt is {owed} and was {before} at {}: a release must carry at most {}, lower by the minimum of {minimum}{aside}",
                    previous_release.as_deref().unwrap_or("the previous release"),
                    before.saturating_sub(minimum)
                ));
                DebtStanding::Owed
            }
        }
    };

    // the one finding that refuses a release and not a change is the last one said
    let mut release_refusing: Vec<String> = findings
        .last()
        .filter(|_| standing == DebtStanding::Owed)
        .cloned()
        .into_iter()
        .collect();

    // Debt left out of what a release owes is never left out of what a reader is told: each
    // unpayable baseline is named with its reason in every report, and the release in which
    // it was first declared so is marked.
    for b in baselines.iter().filter(|b| !b.payable) {
        let new = recorded
            .as_ref()
            .is_some_and(|r| !r.unpayable.contains(&b.id));
        findings.push(format!(
            "{} is declared unpayable and holds {}{}: {}",
            b.path,
            b.count,
            if new {
                format!(
                    ", newly declared so since {}",
                    previous_release
                        .as_deref()
                        .unwrap_or("the previous release")
                )
            } else {
                String::new()
            },
            b.because
        ));
    }

    // Stale evidence is the release's to answer for and never a change's.
    let evidence = registry
        .evidence_ledger
        .as_deref()
        .map(|ledger| evidence_age(root, ledger, published));
    let evidence = evidence.map(|(age, why)| {
        release_refusing.extend(why.clone());
        findings.extend(why);
        age
    });
    let evidence_holds = evidence.as_ref().is_none_or(|age| age.current);

    DebtReport {
        baselines,
        total,
        payable: owed,
        previous_release,
        previous_total,
        previous_payable,
        minimum_reduction: minimum,
        standing,
        change_allowed: standing != DebtStanding::Refused,
        release_allowed: evidence_holds
            && matches!(
                standing,
                DebtStanding::Paid | DebtStanding::Unrecorded | DebtStanding::Reduced
            ),
        findings,
        refusing,
        release_refusing,
        evidence,
    }
}

impl DebtReport {
    /// What this tree's release record carries, for the next release to be compared with.
    ///
    /// ```
    /// use majordomus_cli::release::debt::{measure, DebtStanding};
    /// let dir = tempfile::tempdir().unwrap();
    /// std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
    /// std::fs::write(dir.path().join(".ai/repo/order-baseline.txt"), "crate_sorts=2\n").unwrap();
    /// std::fs::write(
    ///     dir.path().join(".ai/repo/ci/debt.yaml"),
    ///     "version: 1\nminimum_reduction: 1\nbaselines:\n  - id: order\n    path: .ai/repo/order-baseline.txt\n    gate: order-check\n    form: counts\n",
    /// )
    /// .unwrap();
    /// let report = measure(dir.path(), &[]).unwrap();
    /// assert_eq!(report.standing, DebtStanding::Unrecorded, "no release to compare with, and said so");
    /// let recorded = report.recorded();
    /// assert_eq!((recorded.total, recorded.baselines["order"]), (2, 2));
    /// ```
    pub fn recorded(&self) -> RecordedDebt {
        RecordedDebt {
            total: self.total,
            payable: Some(self.payable),
            unpayable: self
                .baselines
                .iter()
                .filter(|b| !b.payable)
                .map(|b| b.id.clone())
                .collect(),
            baselines: self
                .baselines
                .iter()
                .map(|b| (b.id.clone(), b.count))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(minimum: u64) -> Registry {
        Registry {
            version: 1,
            minimum_reduction: minimum,
            baselines: vec![
                Declared {
                    id: "list".into(),
                    path: ".ai/repo/list-baseline.txt".into(),
                    gate: "list-check".into(),
                    form: DebtForm::Entries,
                    payable: true,
                    because: String::new(),
                    slack: None,
                    not_entries: None,
                },
                Declared {
                    id: "numbers".into(),
                    path: ".ai/repo/numbers-baseline.txt".into(),
                    gate: "numbers-check".into(),
                    form: DebtForm::Counts,
                    payable: true,
                    because: String::new(),
                    slack: None,
                    not_entries: None,
                },
            ],
            not_debt: vec![Excluded {
                path: ".ai/repo/ci/baseline.json".into(),
                because: "recorded timings, a measurement and not a violation".into(),
            }],
            evidence_ledger: None,
        }
    }

    /// A tree with a list of `entries` lines and a counter of `stated`.
    fn tree(entries: usize, stated: u64) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
        let list: String = (0..entries).map(|i| format!("entry-{i}\n")).collect();
        std::fs::write(
            dir.path().join(".ai/repo/list-baseline.txt"),
            format!("# accepted\n{list}"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".ai/repo/numbers-baseline.txt"),
            format!("violations={stated}\n"),
        )
        .unwrap();
        std::fs::write(dir.path().join(".ai/repo/ci/baseline.json"), "{}").unwrap();
        dir
    }

    fn was(list: u64, numbers: u64) -> Option<(String, Option<RecordedDebt>)> {
        Some((
            "v1.0.0".into(),
            Some(RecordedDebt {
                total: list + numbers,
                payable: Some(list + numbers),
                unpayable: Vec::new(),
                baselines: [("list".to_string(), list), ("numbers".to_string(), numbers)]
                    .into_iter()
                    .collect(),
            }),
        ))
    }

    fn judged(
        dir: &tempfile::TempDir,
        minimum: u64,
        previous: Option<(String, Option<RecordedDebt>)>,
    ) -> DebtReport {
        judge(
            dir.path(),
            &registry(minimum),
            previous,
            None,
            &named_baselines(dir.path()),
        )
    }

    #[test]
    fn a_release_must_carry_less_than_the_one_before_it() {
        let dir = tree(3, 10);
        // lower by the minimum: releasable
        let report = judged(&dir, 1, was(4, 10));
        assert_eq!(
            report.standing,
            DebtStanding::Reduced,
            "{:?}",
            report.findings
        );
        assert!(report.change_allowed && report.release_allowed);
        assert_eq!((report.total, report.previous_total), (13, Some(14)));
        // standing still is allowed in a change and owed at a release
        let report = judged(&dir, 1, was(3, 10));
        assert_eq!(report.standing, DebtStanding::Owed, "{:?}", report.findings);
        assert!(report.change_allowed && !report.release_allowed);
        assert!(
            report.findings[0].contains("must carry at most 12"),
            "{:?}",
            report.findings
        );
        // lower, but by less than a larger declared minimum
        let report = judged(&dir, 5, was(4, 10));
        assert_eq!(report.standing, DebtStanding::Owed, "{:?}", report.findings);
    }

    #[test]
    fn no_baseline_may_grow_whatever_the_total_does() {
        // the list grew by one while the counter fell by five: the total is lower, and the
        // growth is refused all the same, in a change as at a release
        let dir = tree(4, 5);
        let report = judged(&dir, 1, was(3, 10));
        assert_eq!(
            report.standing,
            DebtStanding::Refused,
            "{:?}",
            report.findings
        );
        assert!(!report.change_allowed && !report.release_allowed);
        assert!(
            report.findings.iter().any(|f| f
                .contains(".ai/repo/list-baseline.txt grew from 3 to 4 since v1.0.0")
                && f.contains("list-check")),
            "{:?}",
            report.findings
        );
        // a counter that grew is seen as growth, which a line count would miss
        let dir = tree(3, 500);
        let report = judged(&dir, 1, was(3, 54));
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.contains("numbers-baseline.txt grew from 54 to 500")),
            "{:?}",
            report.findings
        );
    }

    #[test]
    fn a_first_measurement_is_recorded_and_said_to_be_compared_with_nothing() {
        let dir = tree(3, 10);
        for previous in [None, Some(("v1.0.0".to_string(), None))] {
            let report = judged(&dir, 1, previous.clone());
            assert_eq!(report.standing, DebtStanding::Unrecorded);
            assert!(report.release_allowed, "the first release is not refused");
            assert_eq!(report.previous_total, None);
            assert!(
                report.findings[0].contains("compared with nothing"),
                "{previous:?}: {:?}",
                report.findings
            );
            assert!(report.baselines.iter().all(|b| b.previous.is_none()));
        }
        assert_eq!(judged(&dir, 1, None).recorded().total, 13);
    }

    #[test]
    fn nothing_owed_at_zero_and_nothing_hidden_or_unread() {
        let dir = tree(0, 0);
        let report = judged(&dir, 1, was(0, 0));
        assert_eq!(report.standing, DebtStanding::Paid, "{:?}", report.findings);
        assert!(report.release_allowed);

        // a baseline nobody declared
        std::fs::write(dir.path().join(".ai/repo/new-baseline.txt"), "x\n").unwrap();
        let report = judged(&dir, 1, was(0, 0));
        assert_eq!(report.standing, DebtStanding::Refused);
        assert!(
            report.findings[0].contains(".ai/repo/new-baseline.txt is named like a baseline"),
            "{:?}",
            report.findings
        );
        std::fs::remove_file(dir.path().join(".ai/repo/new-baseline.txt")).unwrap();

        // a counter that states no number, and a declared file that is gone
        std::fs::write(
            dir.path().join(".ai/repo/numbers-baseline.txt"),
            "violations=several\n",
        )
        .unwrap();
        std::fs::remove_file(dir.path().join(".ai/repo/list-baseline.txt")).unwrap();
        let report = judged(&dir, 1, was(0, 0));
        assert_eq!(report.standing, DebtStanding::Refused);
        assert!(report
            .findings
            .iter()
            .any(|f| f.contains("states no count")));
        assert!(report
            .findings
            .iter()
            .any(|f| f.contains("list-baseline.txt: declared in") && f.contains("cannot be read")));

        // an exclusion owes its reason
        let mut bare = registry(1);
        bare.not_debt[0].because = " ".into();
        let dir = tree(0, 0);
        let report = judge(dir.path(), &bare, None, None, &named_baselines(dir.path()));
        assert!(report
            .findings
            .iter()
            .any(|f| f.contains("excluded from the debt without a reason")));
    }

    /// Debt no change can pay is counted, recorded and refused when it grows, and is left
    /// out of what a release must reduce — when the declaration says why.
    #[test]
    fn debt_that_cannot_be_paid_is_counted_and_not_owed() {
        let mut frozen = registry(1);
        frozen.baselines[1].payable = false;
        frozen.baselines[1].because =
            "published commits an append-only history cannot rewrite".into();
        let judged = |dir: &tempfile::TempDir, previous| {
            judge(
                dir.path(),
                &frozen,
                previous,
                None,
                &named_baselines(dir.path()),
            )
        };
        // the list fell from 3 to 2; the counter stands at 10 and is not what is owed
        let dir = tree(2, 10);
        let report = judged(&dir, was(3, 10));
        assert_eq!(
            report.standing,
            DebtStanding::Reduced,
            "{:?}",
            report.findings
        );
        assert_eq!((report.total, report.payable), (12, 2));
        assert_eq!(
            (report.previous_total, report.previous_payable),
            (Some(13), Some(3))
        );
        assert!(
            report.findings[0].contains("the debt fell from 3 to 2")
                && report.findings[0].contains("10 more cannot be paid and is counted, not owed"),
            "{:?}",
            report.findings
        );
        assert_eq!(report.recorded().payable, Some(2));
        assert_eq!(
            report.recorded().baselines["numbers"],
            10,
            "it stays in the record"
        );
        assert_eq!(report.recorded().unpayable, ["numbers"]);
        // named in every report with its reason, and marked in the release that opened it
        assert!(
            report.findings.iter().any(|f| f
                == ".ai/repo/numbers-baseline.txt is declared unpayable and holds 10, newly declared so since v1.0.0: published commits an append-only history cannot rewrite"),
            "{:?}",
            report.findings
        );
        let mut later = was(3, 10);
        if let Some((_, Some(recorded))) = later.as_mut() {
            recorded.unpayable = vec!["numbers".into()];
        }
        let report = judged(&dir, later);
        assert!(
            report.findings.iter().any(|f| f
                == ".ai/repo/numbers-baseline.txt is declared unpayable and holds 10: published commits an append-only history cannot rewrite"),
            "{:?}",
            report.findings
        );
        // standing still on what can be paid is owed, whatever the frozen part is
        let report = judged(&dir, was(2, 10));
        assert_eq!(report.standing, DebtStanding::Owed, "{:?}", report.findings);
        // it may not grow
        let dir = tree(2, 11);
        let report = judged(&dir, was(3, 10));
        assert_eq!(
            report.standing,
            DebtStanding::Refused,
            "{:?}",
            report.findings
        );
        // with everything payable paid, nothing is owed though the frozen part remains
        let dir = tree(0, 10);
        let report = judged(&dir, was(1, 10));
        assert_eq!(report.standing, DebtStanding::Paid, "{:?}", report.findings);
        assert!(
            report.findings[0].contains("10 more cannot be paid"),
            "{:?}",
            report.findings
        );
        // and unpayable without a reason is refused
        frozen.baselines[1].because = "  ".into();
        let report = judge(
            dir.path(),
            &frozen,
            was(1, 10),
            None,
            &named_baselines(dir.path()),
        );
        assert_eq!(report.standing, DebtStanding::Refused);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.contains("declared `payable: false` without a reason")),
            "{:?}",
            report.findings
        );
    }

    #[test]
    fn the_registry_is_read_or_nothing_is_known() {
        let dir = tempfile::tempdir().unwrap();
        assert!(measure(dir.path(), &[])
            .unwrap_err()
            .contains("nothing is known to be debt"));
        std::fs::create_dir_all(dir.path().join(".ai/repo/ci")).unwrap();
        std::fs::write(
            dir.path().join(REGISTRY),
            "version: 2\nminimum_reduction: 1\nbaselines: []\n",
        )
        .unwrap();
        assert!(measure(dir.path(), &[])
            .unwrap_err()
            .contains("version 2, this reads 1"));
        std::fs::write(dir.path().join(REGISTRY), "version: 1\nminimum: 1\n").unwrap();
        assert!(
            measure(dir.path(), &[]).is_err(),
            "an unknown key is refused"
        );
    }

    /// A release record as the index holds it, with the debt block a record may carry.
    fn release(version: &str, tag: Option<&str>, debt: Option<serde_json::Value>) -> Object {
        let mut metadata = serde_json::Map::new();
        metadata.insert("version".into(), version.into());
        if let Some(tag) = tag {
            metadata.insert("tag".into(), tag.into());
        }
        if let Some(debt) = debt {
            metadata.insert("debt".into(), debt);
        }
        Object {
            kind: super::super::changelog::RELEASE_KIND.into(),
            identity: format!("v{version}"),
            uri: format!("majordomus://release-record/v{version}"),
            title: None,
            description: None,
            metadata: serde_json::Value::Object(metadata),
            body: String::new(),
            content: String::new(),
            media_type: "application/yaml",
            provenance: crate::model::Provenance {
                path: format!(".ai/repo/releases/v{version}.yaml"),
                directory: ".ai/repo/releases".into(),
                source_class: "release".into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    /// The fixture tree with the declaration written where `measure` reads it. Neither
    /// baseline says whether it can be paid, which is the declaration most of them make.
    fn declared_tree(entries: usize, stated: u64) -> tempfile::TempDir {
        let dir = tree(entries, stated);
        std::fs::write(
            dir.path().join(REGISTRY),
            "version: 1\nminimum_reduction: 1\nbaselines:\n  - id: list\n    path: .ai/repo/list-baseline.txt\n    gate: list-check\n    form: entries\n  - id: numbers\n    path: .ai/repo/numbers-baseline.txt\n    gate: numbers-check\n    form: counts\nnot_debt:\n  - path: .ai/repo/ci/baseline.json\n    because: recorded timings, a measurement and not a violation\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn the_tree_is_held_against_the_newest_release_record_and_what_it_counted() {
        let dir = declared_tree(3, 10);

        // no release on record: nothing to compare with, and a baseline is payable unless
        // its declaration says otherwise
        let report = measure(dir.path(), &[]).unwrap();
        assert_eq!(report.previous_release, None);
        assert_eq!(report.total, 13);
        assert!(report.baselines.iter().all(|b| b.payable));
        assert!(report.release_allowed, "{:?}", report.findings);

        // the newest record is the one compared with, by version and not by spelling:
        // 1.10.0 is newer than 1.9.0. A record with no tag is named by its version, one
        // whose version cannot be read is no release, and another kind is not asked.
        let counted = serde_json::json!({"total": 14, "baselines": {"list": 4, "numbers": 10}});
        let mut other = release("9.0.0", Some("v9.0.0"), Some(counted.clone()));
        other.kind = "rule".into();
        let mut unversioned = release("9.1.0", None, Some(counted.clone()));
        unversioned.metadata = serde_json::json!({ "debt": counted });
        let mut numbered = release("9.2.0", None, Some(counted.clone()));
        numbered.metadata = serde_json::json!({ "version": 9, "debt": counted });
        let mut objects = vec![
            release("1.9.0", Some("v1.9.0"), Some(counted.clone())),
            release("1.10.0", None, None),
            release("not-a-version", Some("vX"), Some(counted.clone())),
            unversioned,
            numbered,
            other,
        ];
        let report = measure(dir.path(), &objects).unwrap();
        assert_eq!(report.previous_release.as_deref(), Some("v1.10.0"));
        assert_eq!(
            report.previous_total, None,
            "the newest record counted nothing"
        );
        assert_eq!(report.standing, DebtStanding::Unrecorded);

        // a newer record that counted: the tree carries one less, which is the minimum
        objects.push(release("1.11.0", Some("v1.11.0"), Some(counted)));
        let report = measure(dir.path(), &objects).unwrap();
        assert_eq!(report.previous_release.as_deref(), Some("v1.11.0"));
        assert_eq!(report.previous_total, Some(14));
        assert_eq!(
            report.standing,
            DebtStanding::Reduced,
            "{:?}",
            report.findings
        );

        // and the same tree against a record that carried what it carries is owed
        objects.push(release(
            "1.12.0",
            Some("v1.12.0"),
            Some(serde_json::json!({"total": 13, "baselines": {"list": 3, "numbers": 10}})),
        ));
        let report = measure(dir.path(), &objects).unwrap();
        assert_eq!(report.standing, DebtStanding::Owed);
        assert!(report.change_allowed && !report.release_allowed);
    }

    #[test]
    fn a_counter_too_large_to_add_is_refused_and_never_wraps() {
        let err = count(DebtForm::Counts, "a=18446744073709551615\nb=1\n").unwrap_err();
        assert!(err.contains("`b=1` overflows the count"), "{err}");
    }

    const LEDGER: &str = ".ai/repo/evidence/ledger.json";

    /// The verdict over a paid-down tree whose declaration names the ledger, written as
    /// `ledger` (or not at all), against a release published at `released`.
    fn aged(ledger: Option<&str>, released: Option<&str>) -> DebtReport {
        let dir = tree(3, 10);
        if let Some(text) = ledger {
            std::fs::create_dir_all(dir.path().join(".ai/repo/evidence")).unwrap();
            std::fs::write(dir.path().join(LEDGER), text).unwrap();
        }
        let mut registry = registry(1);
        registry.evidence_ledger = Some(LEDGER.into());
        judge(
            dir.path(),
            &registry,
            was(4, 10),
            released,
            &named_baselines(dir.path()),
        )
    }

    #[test]
    fn evidence_older_than_the_previous_release_refuses_a_release_and_never_a_change() {
        let released = Some("2026-10-08T11:00:00Z");
        let run = |origin: &str, at: &str| {
            format!(
                r#"{{"version": 1, "executions": [{{"origin": "{origin}", "at": "{at}"}}, {{"origin": "ci", "at": "2026-09-01T00:00:00Z"}}]}}"#
            )
        };

        // recorded since the release: the debt verdict stands alone
        let report = aged(Some(&run("ci", "2026-10-08T18:49:19Z")), released);
        let age = report.evidence.clone().unwrap();
        assert!(
            age.current && report.release_allowed,
            "{:?}",
            report.findings
        );
        assert_eq!(age.newest.as_deref(), Some("2026-10-08T18:49:19Z"));
        assert_eq!(age.released.as_deref(), released);

        // the newest run is a person's and CI's is from before the release: stale, and the
        // debt it fell by does not make up for it
        let report = aged(Some(&run("local", "2026-10-08T18:49:19Z")), released);
        assert_eq!(report.standing, DebtStanding::Reduced);
        assert!(report.change_allowed && !report.release_allowed);
        assert!(
            report.findings.iter().any(|f| f.contains(
                "the newest CI execution in .ai/repo/evidence/ledger.json is 2026-09-01T00:00:00Z, before the previous release was published (2026-10-08T11:00:00Z)"
            )),
            "{:?}",
            report.findings
        );

        // no release to be older than: any CI execution is current
        assert!(aged(Some(&run("local", "2026-10-08T18:49:19Z")), None).release_allowed);
    }

    #[test]
    fn a_ledger_that_says_nothing_is_not_a_ledger_that_is_current() {
        let released = Some("2026-10-08T11:00:00Z");
        for (ledger, says) in [
            (None, "cannot be read as an evidence ledger"),
            (Some("not json"), "cannot be read as an evidence ledger"),
            (
                Some(r#"{"version": 1}"#),
                "holds no execution recorded by CI",
            ),
            (
                Some(r#"{"executions": {}}"#),
                "holds no execution recorded by CI",
            ),
            (
                Some(
                    r#"{"executions": [{"origin": "local", "at": "2026-10-08T18:49:19Z"}, {"origin": "ci"}]}"#,
                ),
                "holds no execution recorded by CI",
            ),
            (
                Some(r#"{"executions": [{"origin": "ci", "at": "2026-10-08 18:49"}]}"#),
                "are not times this can compare",
            ),
            // twenty characters ending in Z, and still not the shape that orders as text
            (
                Some(r#"{"executions": [{"origin": "ci", "at": "2026-10-08 18:49:19Z"}]}"#),
                "are not times this can compare",
            ),
        ] {
            let report = aged(ledger, released);
            assert!(
                !report.release_allowed && report.change_allowed,
                "{ledger:?}"
            );
            assert!(!report.evidence.as_ref().unwrap().current);
            assert!(
                report.findings.iter().any(|f| f.contains(says)),
                "{ledger:?}: {:?}",
                report.findings
            );
        }
        // a release that states its time another way is not compared either
        let report = aged(
            Some(r#"{"executions": [{"origin": "ci", "at": "2026-10-08T18:49:19Z"}]}"#),
            Some("yesterday"),
        );
        assert!(!report.release_allowed);

        // and a declaration that names no ledger asks nothing of it
        let dir = tree(3, 10);
        assert_eq!(judged(&dir, 1, was(4, 10)).evidence, None);
    }

    #[test]
    fn the_release_the_evidence_is_held_against_is_the_newest_record_s_own_time() {
        let dir = declared_tree(3, 10);
        let declaration = std::fs::read_to_string(dir.path().join(REGISTRY)).unwrap();
        std::fs::write(
            dir.path().join(REGISTRY),
            format!("{declaration}evidence_ledger: {LEDGER}\n"),
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/repo/evidence")).unwrap();
        std::fs::write(
            dir.path().join(LEDGER),
            r#"{"executions": [{"origin": "ci", "at": "2026-09-17T01:14:59Z"}]}"#,
        )
        .unwrap();
        let mut record = release("1.0.0", Some("v1.0.0"), None);
        record.metadata["published_at"] = "2026-10-08T11:00:00Z".into();
        let report = measure(dir.path(), &[record]).unwrap();
        let age = report.evidence.clone().unwrap();
        assert_eq!(age.released.as_deref(), Some("2026-10-08T11:00:00Z"));
        assert!(!age.current && !report.release_allowed);
        assert_eq!(report.release_refusing.len(), 1, "{:?}", report.findings);

        // a release on record that states no time cannot be compared with, which is not
        // the same as there being no release
        let report = measure(dir.path(), &[release("1.0.0", Some("v1.0.0"), None)]).unwrap();
        let age = report.evidence.unwrap();
        assert_eq!(age.released.as_deref(), Some(""));
        assert!(!age.current && !report.release_allowed);
        assert!(report
            .findings
            .iter()
            .any(|f| f.contains("are not times this can compare")));
    }

    #[test]
    fn a_declared_slack_reaches_the_report_and_an_undeclared_one_is_absent() {
        let dir = declared_tree(3, 10);
        let declaration = std::fs::read_to_string(dir.path().join(REGISTRY)).unwrap();
        std::fs::write(
            dir.path().join(REGISTRY),
            declaration.replace(
                "    gate: list-check\n    form: entries\n",
                "    gate: list-check\n    form: entries\n    slack:\n      command: scripts/ci/list-check\n      says: tighten the baseline\n",
            ),
        )
        .unwrap();
        let report = measure(dir.path(), &[]).unwrap();
        let slack = report.baselines[0].slack.clone().unwrap();
        assert_eq!(
            (slack.command.as_str(), slack.says.as_str()),
            ("scripts/ci/list-check", "tighten the baseline")
        );
        assert_eq!(report.baselines[1].slack, None);
    }

    #[test]
    fn a_line_the_declaration_marks_is_not_an_entry() {
        // the evidence baseline's form: a bare line is debt, a `+` line is a proof kept
        let text = "# header\nclaim-a\n+claim-b\n  +claim-c\nclaim-d\n";
        assert_eq!(
            count(DebtForm::Entries, &entries_only(text, Some("+"))),
            Ok(2)
        );
        assert_eq!(count(DebtForm::Entries, &entries_only(text, None)), Ok(4));
        assert_eq!(
            count(DebtForm::Entries, &entries_only(text, Some(""))),
            Ok(4)
        );

        let dir = declared_tree(3, 10);
        std::fs::write(
            dir.path().join(".ai/repo/list-baseline.txt"),
            "entry-0\n+kept\n+kept-too\n",
        )
        .unwrap();
        let declaration = std::fs::read_to_string(dir.path().join(REGISTRY)).unwrap();
        std::fs::write(
            dir.path().join(REGISTRY),
            declaration.replace(
                "    gate: list-check\n    form: entries\n",
                "    gate: list-check\n    form: entries\n    not_entries: \"+\"\n",
            ),
        )
        .unwrap();
        assert_eq!(measure(dir.path(), &[]).unwrap().baselines[0].count, 1);
    }

    #[test]
    fn a_previous_release_that_cannot_be_read_is_not_one_that_recorded_nothing() {
        let dir = declared_tree(3, 10);
        // a debt block of another shape: refused, in any change
        let garbled = release(
            "1.0.0",
            Some("v1.0.0"),
            Some(serde_json::json!({"total": "many"})),
        );
        let report = measure(dir.path(), &[garbled]).unwrap();
        assert_eq!(report.standing, DebtStanding::Refused);
        assert!(!report.change_allowed && !report.release_allowed);
        assert!(
            report.findings[0].starts_with("v1.0.0 records a debt block this cannot read ("),
            "{:?}",
            report.findings
        );
        assert_eq!(report.refusing, vec![report.findings[0].clone()]);

        // a baseline the previous release counted and the declaration dropped
        let counted = serde_json::json!({"total": 20, "baselines": {"list": 4, "numbers": 10, "retired": 6, "emptied": 0}});
        let report = measure(
            dir.path(),
            &[release("1.0.0", Some("v1.0.0"), Some(counted))],
        )
        .unwrap();
        assert_eq!(report.standing, DebtStanding::Refused);
        assert!(
            report.refusing.iter().any(|f| f.contains(
                "v1.0.0 recorded 6 for `retired` and .ai/repo/ci/debt.yaml no longer counts it"
            )),
            "{:?}",
            report.findings
        );
        assert!(
            !report.findings.iter().any(|f| f.contains("`emptied`")),
            "a baseline that reached zero may leave"
        );

        // a minimum below one is not a minimum
        let declaration = std::fs::read_to_string(dir.path().join(REGISTRY)).unwrap();
        std::fs::write(
            dir.path().join(REGISTRY),
            declaration.replace("minimum_reduction: 1", "minimum_reduction: 0"),
        )
        .unwrap();
        assert!(measure(dir.path(), &[])
            .unwrap_err()
            .contains("minimum_reduction is 0"));
    }

    #[test]
    fn the_findings_say_which_of_them_refuse_and_what() {
        let dir = tree(3, 10);
        // owed: one finding, and it refuses a release only
        let report = judged(&dir, 1, was(3, 10));
        assert_eq!(report.standing, DebtStanding::Owed);
        assert!(report.refusing.is_empty());
        assert_eq!(report.release_refusing.len(), 1);
        assert!(report.release_refusing[0].starts_with("the debt is 13 and was 13"));
        // reduced: nothing refuses anything
        let report = judged(&dir, 1, was(4, 10));
        assert!(report.refusing.is_empty() && report.release_refusing.is_empty());
        // grew: refuses any change
        let report = judged(&dir, 1, was(2, 10));
        assert_eq!(report.refusing.len(), 1);
        assert!(report.release_refusing.is_empty());
    }

    #[test]
    fn a_layer_that_is_not_there_names_no_baseline() {
        let dir = tempfile::tempdir().unwrap();
        assert!(named_baselines(dir.path()).is_empty());
    }
}
