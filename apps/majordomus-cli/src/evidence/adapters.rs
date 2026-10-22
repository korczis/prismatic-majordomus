//! The runners' own lines, read once: one adapter per runner the recorder handles.
//!
//! A run is recorded from what its runner printed. Two runners print what this repository
//! records: `test/run.sh`, which writes a TSV row per case (`MJ_TEST_REPORT`) and prints a
//! verdict line per case with the log of a case before its verdict, and `cargo test`, whose
//! libtest harness prints a `Running` line per binary, a `test <name> ... <word>` line per
//! test, the captured output of each failure and a `test result:` line per binary. Each has
//! one adapter here, and every reader of those lines reads them through it: the recorder
//! ([`super::record`]) and the web report's test page (`web::report::tests`) alike, so the
//! two cannot read the same run differently.
//!
//! # Every line is read, or listed
//!
//! An adapter decides, for every line it is given, one of two things: the line is read into
//! a case result (a case's own line, its verdict, its captured output, or the line that
//! opens or closes the run it belongs to), or it is listed as an [`EvidenceDroppedLine`]
//! with its number, its text and why no case holds it. Nothing is skipped in silence: a
//! recording that threw a line away would read as a run that printed less than it did, and
//! the line thrown away is, often enough, the reason a case failed. [`EvidenceLines`] counts
//! both sides, so `read + dropped == lines` is something a reader can check.
//!
//! ```
//! use majordomus_cli::evidence::adapters::read_libtest;
//! use majordomus_cli::evidence::Outcome;
//!
//! let read = read_libtest(
//!     "     Running tests/why.rs (target/debug/deps/why-1)\n\
//!      \n\
//!      running 2 tests\n\
//!      test holds ... ok\n\
//!      test breaks ... FAILED\n\
//!      \n\
//!      failures:\n\
//!      \n\
//!      ---- breaks stdout ----\n\
//!      assertion failed: it broke\n\
//!      \n\
//!      failures:\n\
//!      \x20   breaks\n\
//!      \n\
//!      test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; \
//!      finished in 0.10s\n",
//! );
//! let why = &read.units[0];
//! assert_eq!((why.name.as_str(), why.outcome), ("why", Outcome::Fail));
//! assert_eq!(why.cases[1].name, "breaks");
//! assert_eq!(why.cases[1].detail.as_deref(), Some("assertion failed: it broke"));
//! // every line is accounted for: read into the binary's cases, or listed with why
//! assert_eq!(read.lines.read + read.dropped.len(), read.lines.lines);
//! assert!(read.dropped.iter().all(|d| d.text.is_empty()), "only blank lines are dropped");
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::run::EvidenceProducer;
use super::Outcome;

/// Why the crate's own unit-test binary is listed rather than recorded.
pub(crate) const UNIT_TESTS: &str =
    "the crate's own unit tests, which no claim can name; each test is listed among its cases";
/// Why the crate's doctests are listed rather than recorded.
pub(crate) const DOC_TESTS: &str =
    "doctests, which no claim can name; each doctest is listed among its cases";
/// Why a binary that is not under `tests/` is listed rather than recorded.
pub(crate) const NOT_AN_INTEGRATION_BINARY: &str = "not an integration test binary under tests/";
/// Why a result line with no binary before it is listed rather than recorded.
pub(crate) const UNATTRIBUTED: &str = "no Running line named the binary it belongs to";

/// A blank line between the parts of a run.
const BLANK: &str = "a blank line between the parts of the run";
/// A line printed outside any test binary's run.
const OUTSIDE: &str = "printed outside any test binary's run: cargo's own build or summary output";
/// A line inside a binary's run that is none of the harness's own lines.
const INSIDE: &str = "printed inside a test binary's run, and not a line of its harness";
/// The captured output of a test the binary's run named no result for.
const UNNAMED_OUTPUT: &str = "the captured output of a test no result line of its binary names";
/// A note that a test is slow, about a test no result line names.
const UNNAMED_NOTE: &str = "a note about a test no result line of its binary names";
/// The runner's own progress line.
const RUNNER: &str = "the runner's own line about the run, not about one case";
/// The runner's totals.
const RUNNER_TOTALS: &str = "the runner's totals, which the report's rows restate";
/// The runner's list of the cases that failed.
const RUNNER_FAILED: &str = "the runner's list of failed cases, which the report's rows restate";
/// A verdict for a case the report holds no row for.
const UNREPORTED_VERDICT: &str = "a verdict for a case the report holds no row for";
/// Output followed by the verdict of a case the report holds no row for.
const UNREPORTED_OUTPUT: &str = "the output of a case the report holds no row for";
/// Output no verdict line followed.
const NO_VERDICT: &str = "output that no case's verdict line followed";

/// Which of a runner's two streams a line was read from: the report it wrote for a reader
/// (the suite's TSV), or the output it printed (the suite's console, `cargo test`'s output).
///
/// ```
/// use majordomus_cli::evidence::adapters::EvidenceStream;
/// assert_eq!(serde_json::to_value(EvidenceStream::Output).unwrap(), "output");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStream {
    /// The report a runner writes for a reader: `test/run.sh`'s TSV.
    Report,
    /// What a runner prints: `test/run.sh`'s console, `cargo test`'s output.
    Output,
}

/// One test, as its runner reported it on its own line: a suite case's TSV row, or a
/// libtest `test <name> ... <word>` line.
///
/// `detail` is what the runner printed about that one test and nothing else: a libtest
/// test's captured output, the reason it was ignored, or the note that it ran long. A suite
/// case's log belongs to its execution's detail, because the case is the execution.
///
/// ```
/// use majordomus_cli::evidence::adapters::EvidenceCase;
/// use majordomus_cli::evidence::Outcome;
///
/// let c: EvidenceCase =
///     serde_json::from_value(serde_json::json!({ "name": "a::b", "outcome": "fail" })).unwrap();
/// assert_eq!(c.outcome, Outcome::Fail);
/// assert!(c.seconds.is_none() && c.phase.is_none() && c.detail.is_none());
/// // what a runner did not say is not written back as null
/// assert_eq!(serde_json::to_value(&c).unwrap().as_object().unwrap().len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceCase")]
pub struct EvidenceCase {
    /// The test's name, as the runner printed it.
    pub name: String,
    /// What its line said.
    pub outcome: Outcome,
    /// How long it took, in whole seconds, when the runner said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u64>,
    /// The suite phase it ran in (`parallel`, `exclusive`, `serial`), for a suite case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// What the runner printed about this one test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A line a runner printed that no case result holds, listed with why.
///
/// `line` counts from 1 in the stream it names, and `text` is the line with its terminal
/// escape sequences removed.
///
/// ```
/// use majordomus_cli::evidence::adapters::{EvidenceDroppedLine, EvidenceStream};
/// use majordomus_cli::evidence::EvidenceProducer;
///
/// let d: EvidenceDroppedLine = serde_json::from_value(serde_json::json!({
///     "producer": "crate", "stream": "output", "line": 1,
///     "text": "   Compiling fixture v0.1.0", "reason": "cargo's own build output"
/// }))
/// .unwrap();
/// assert_eq!((d.producer, d.stream, d.line), (EvidenceProducer::Crate, EvidenceStream::Output, 1));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceDroppedLine {
    /// The runner whose line it is.
    pub producer: EvidenceProducer,
    /// The stream it was read from.
    pub stream: EvidenceStream,
    /// Its number in that stream, from 1.
    pub line: usize,
    /// The line, without terminal escape sequences.
    pub text: String,
    /// Why no case result holds it.
    pub reason: String,
}

/// How many lines one stream of a runner held, and how many of them a case result holds:
/// the rest are listed one by one as [`EvidenceDroppedLine`]s.
///
/// ```
/// use majordomus_cli::evidence::adapters::{EvidenceLines, EvidenceStream};
/// use majordomus_cli::evidence::EvidenceProducer;
///
/// let l = EvidenceLines {
///     producer: EvidenceProducer::Suite, stream: EvidenceStream::Report, lines: 3, read: 2,
/// };
/// assert_eq!(l.dropped(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceLines {
    /// The runner.
    pub producer: EvidenceProducer,
    /// The stream.
    pub stream: EvidenceStream,
    /// How many lines the stream held.
    pub lines: usize,
    /// How many of them a case result holds.
    pub read: usize,
}

impl EvidenceLines {
    /// How many lines of the stream are listed as dropped.
    ///
    /// ```
    /// use majordomus_cli::evidence::adapters::read_libtest;
    /// let read = read_libtest("   Compiling x\n");
    /// assert_eq!((read.lines.lines, read.lines.dropped()), (1, 1));
    /// ```
    pub fn dropped(&self) -> usize {
        self.lines.saturating_sub(self.read)
    }
}

/// What became of one line, while an adapter reads its stream.
#[derive(Clone, Copy)]
enum Use {
    /// Not decided yet; a line left so is listed, never lost.
    Open,
    /// A case result holds it.
    Read,
    /// Listed, with why.
    Dropped(&'static str),
}

/// One stream's lines, each with what became of it.
struct Tally {
    texts: Vec<String>,
    uses: Vec<Use>,
}

impl Tally {
    fn of(text: &str) -> Tally {
        let texts: Vec<String> = text.lines().map(strip_ansi).collect();
        let uses = vec![Use::Open; texts.len()];
        Tally { texts, uses }
    }

    fn read(&mut self, i: usize) {
        self.uses[i] = Use::Read;
    }

    fn drop(&mut self, i: usize, reason: &'static str) {
        self.uses[i] = Use::Dropped(reason);
    }

    /// The count, and every line not read, in the order the stream held them.
    fn finish(
        self,
        producer: EvidenceProducer,
        stream: EvidenceStream,
    ) -> (EvidenceLines, Vec<EvidenceDroppedLine>) {
        let lines = EvidenceLines {
            producer,
            stream,
            lines: self.texts.len(),
            read: self.uses.iter().filter(|u| matches!(u, Use::Read)).count(),
        };
        let dropped = self
            .texts
            .into_iter()
            .zip(self.uses)
            .enumerate()
            .filter_map(|(i, (text, u))| {
                let reason = match u {
                    Use::Read => return None,
                    Use::Dropped(r) => r,
                    Use::Open => NO_VERDICT,
                };
                Some(EvidenceDroppedLine {
                    producer,
                    stream,
                    line: i + 1,
                    text,
                    reason: reason.to_string(),
                })
            })
            .collect();
        (lines, dropped)
    }
}

// ---------------------------------------------------------------- the suite

/// One suite case: its TSV row, with the word the runner wrote, and the lines its log
/// printed in the runner's output.
///
/// ```
/// use majordomus_cli::evidence::adapters::read_suite;
/// use majordomus_cli::evidence::Outcome;
///
/// let read = read_suite("07_scope\tSKIP\t2\tserial\n", Some("    skip: no zola\nskip 07_scope\n"))
///     .unwrap();
/// let case = &read.cases[0];
/// assert_eq!((case.word.as_str(), case.outcome), ("SKIP", Outcome::Skip));
/// assert_eq!(case.detail().as_deref(), Some("    skip: no zola"));
/// assert_eq!(case.case().phase.as_deref(), Some("serial"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteCase {
    /// The case's name, which is its file's stem.
    pub name: String,
    /// The result word as the runner wrote it: `ok`, `FAIL`, `TIMEOUT` or `SKIP`.
    pub word: String,
    /// What the word means.
    pub outcome: Outcome,
    /// How long it took, in whole seconds.
    pub seconds: u64,
    /// The phase it ran in: parallel, exclusive or serial.
    pub phase: String,
    /// The lines the runner printed for it, in order: its log, and what the runner said of
    /// it beyond its verdict.
    pub log: Vec<String>,
}

impl SuiteCase {
    /// The case as a per-case result: its name, outcome, seconds and phase.
    ///
    /// ```
    /// use majordomus_cli::evidence::adapters::read_suite;
    /// let read = read_suite("01_a\tok\t3\tparallel\n", None).unwrap();
    /// assert_eq!(read.cases[0].case().seconds, Some(3));
    /// ```
    pub fn case(&self) -> EvidenceCase {
        EvidenceCase {
            name: self.name.clone(),
            outcome: self.outcome,
            seconds: Some(self.seconds),
            phase: Some(self.phase.clone()),
            detail: None,
        }
    }

    /// Its log as one text, or nothing when the runner printed nothing for it.
    ///
    /// ```
    /// use majordomus_cli::evidence::adapters::read_suite;
    /// let read = read_suite("01_a\tok\t3\tparallel\n", None).unwrap();
    /// assert!(read.cases[0].detail().is_none());
    /// ```
    pub fn detail(&self) -> Option<String> {
        (!self.log.is_empty()).then(|| self.log.join("\n"))
    }
}

/// A suite run, read: its cases, and every line of the report and of the output that no
/// case holds, listed with why.
///
/// ```
/// use majordomus_cli::evidence::adapters::{read_suite, SuiteRead};
/// let read: SuiteRead = read_suite("01_a\tok\t1\tparallel\n\n", None).unwrap();
/// assert_eq!(read.cases.len(), 1);
/// assert_eq!(read.dropped.len(), 1, "the blank line, listed");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SuiteRead {
    /// The cases, in the order the report recorded them.
    pub cases: Vec<SuiteCase>,
    /// The count of each stream read: the report, and the output when one was given.
    pub lines: Vec<EvidenceLines>,
    /// Every line no case holds.
    pub dropped: Vec<EvidenceDroppedLine>,
}

/// Read a suite run: `test/run.sh`'s TSV report (`name <TAB> result <TAB> seconds <TAB>
/// phase`, one case per line) and, when given, what it printed.
///
/// A malformed row is refused rather than skipped, with the reason as the error: a reading
/// that silently dropped a case would leave that case's claim reporting `not run` after a run
/// that did run it. A blank row is listed as dropped.
///
/// The output is joined to the report by case name. A case's verdict line (`ok   <name>`,
/// `FAIL <name>`, `TIMEOUT <name>`, `skip <name>`, a parallel worker's `ok      <name>  3s`)
/// closes every line printed since the verdict before it, which is that case's log: the
/// runner streams a case's output before its verdict when it runs serially, and prints a
/// failing case's whole log right before its verdict when it ran in parallel. The runner's
/// own lines (`run.sh: ...`, the totals, the list of failures) and anything no verdict
/// follows are listed as dropped.
///
/// ```
/// use majordomus_cli::evidence::adapters::read_suite;
///
/// let refused = read_suite("07_scope\tok\t12\n", None).unwrap_err();
/// assert!(refused.contains("field(s)"), "{refused}");
///
/// let read = read_suite(
///     "01_a\tok\t1\tserial\n02_b\tFAIL\t2\tserial\n",
///     Some("ok   01_a\n    expected /x/ in output:\nnothing\nFAIL 02_b\ntests: 1 passed, 1 failed, 0 skipped\n"),
/// )
/// .unwrap();
/// assert_eq!(read.cases[1].log, ["    expected /x/ in output:", "nothing"]);
/// assert_eq!(read.dropped.len(), 1, "the totals");
/// ```
pub fn read_suite(report: &str, output: Option<&str>) -> Result<SuiteRead, String> {
    let mut tally = Tally::of(report);
    let mut cases: Vec<SuiteCase> = Vec::new();
    for i in 0..tally.texts.len() {
        let line = &tally.texts[i];
        if line.trim().is_empty() {
            tally.drop(i, BLANK);
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            return Err(format!(
                "line {} of the run report has {} field(s); it is \
                 name<TAB>result<TAB>seconds<TAB>phase",
                i + 1,
                f.len()
            ));
        }
        cases.push(SuiteCase {
            name: f[0].to_string(),
            word: f[1].to_string(),
            outcome: Outcome::parse(f[1]),
            seconds: f[2].parse().unwrap_or(0),
            phase: f[3].to_string(),
            log: Vec::new(),
        });
        tally.read(i);
    }
    let (count, mut dropped) = tally.finish(EvidenceProducer::Suite, EvidenceStream::Report);
    let mut lines = vec![count];
    if let Some(output) = output {
        let (count, more) = read_suite_output(output, &mut cases);
        lines.push(count);
        dropped.extend(more);
    }
    Ok(SuiteRead {
        cases,
        lines,
        dropped,
    })
}

/// The runner's output, joined to the report's cases by name; see [`read_suite`].
fn read_suite_output(
    output: &str,
    cases: &mut [SuiteCase],
) -> (EvidenceLines, Vec<EvidenceDroppedLine>) {
    let mut by_name: BTreeMap<String, usize> = BTreeMap::new();
    for (k, c) in cases.iter().enumerate() {
        by_name.entry(c.name.clone()).or_insert(k);
    }
    let mut tally = Tally::of(output);
    let mut pending: Vec<usize> = Vec::new();
    for i in 0..tally.texts.len() {
        let line = tally.texts[i].clone();
        if let Some((name, said)) = suite_verdict(&line) {
            match by_name.get(name) {
                Some(&k) => {
                    for p in pending.drain(..) {
                        cases[k].log.push(tally.texts[p].clone());
                        tally.read(p);
                    }
                    if said {
                        cases[k].log.push(line.clone());
                    }
                    tally.read(i);
                }
                None => {
                    for p in pending.drain(..) {
                        tally.drop(p, UNREPORTED_OUTPUT);
                    }
                    tally.drop(i, UNREPORTED_VERDICT);
                }
            }
            continue;
        }
        if let Some(reason) = suite_runner_line(&line) {
            tally.drop(i, reason);
            continue;
        }
        pending.push(i);
    }
    // what is still pending stays open, and is listed as output no verdict followed
    tally.finish(EvidenceProducer::Suite, EvidenceStream::Output)
}

/// A line of the runner's own, which no case holds, with why.
fn suite_runner_line(line: &str) -> Option<&'static str> {
    if line.starts_with("run.sh: ") || line.starts_with("FAIL run.sh: ") {
        Some(RUNNER)
    } else if line.starts_with("tests: ") && line.contains(" passed, ") {
        Some(RUNNER_TOTALS)
    } else if line.starts_with("failed:") {
        Some(RUNNER_FAILED)
    } else {
        None
    }
}

/// The case a verdict line names, and whether the line says more than the verdict (the
/// `--no-skips` lines do, and are kept in the case's log).
///
/// The shapes are the runner's own: `<word> <name>` with one or more spaces, `FAIL <name>
/// (setup)`, a worker's `<word> <name>  <n>s`, `skip <name> is excused here: ...` and `FAIL
/// <name> declined...`, where the word is `ok`, `FAIL`, `TIMEOUT` or `skip` and the line
/// starts with it.
fn suite_verdict(line: &str) -> Option<(&str, bool)> {
    let t = line.trim_end();
    let (word, rest) = t.split_once(' ')?;
    if !matches!(word, "ok" | "FAIL" | "TIMEOUT" | "skip") {
        return None;
    }
    let rest = rest.trim_start();
    let end = rest.find(' ').unwrap_or(rest.len());
    let (name, after) = rest.split_at(end);
    let is_name = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !is_name {
        return None;
    }
    let seconds = after
        .strip_prefix("  ")
        .and_then(|s| s.strip_suffix('s'))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    if after.is_empty() || seconds || (word == "FAIL" && after == " (setup)") {
        Some((name, false))
    } else if (word == "skip" && after.starts_with(" is excused here"))
        || (word == "FAIL" && after.starts_with(" declined"))
    {
        Some((name, true))
    } else {
        None
    }
}

// ---------------------------------------------------------------- libtest

/// What a libtest run is: an integration binary under `tests/`, the crate's own unit-test
/// binary, its doctests, another binary (a bench, an example), or a result line no `Running`
/// line named.
///
/// ```
/// use majordomus_cli::evidence::adapters::LibtestUnitKind;
/// assert!(LibtestUnitKind::Integration.reason().is_none());
/// assert!(LibtestUnitKind::DocTests.reason().unwrap().contains("no claim can name"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LibtestUnitKind {
    /// `Running tests/<name>.rs`: a binary a claim can name.
    Integration,
    /// `Running unittests <path>`: the crate's own unit tests.
    UnitTests,
    /// `Doc-tests <crate>`.
    DocTests,
    /// `Running <path>` for a path not under `tests/`.
    Other,
    /// A `test result:` line no `Running` line named.
    Unattributed,
}

impl LibtestUnitKind {
    /// Why a run of this kind is listed rather than recorded; none for an integration
    /// binary, which is recorded.
    ///
    /// ```
    /// use majordomus_cli::evidence::adapters::LibtestUnitKind;
    /// assert!(LibtestUnitKind::Other.reason().unwrap().contains("tests/"));
    /// ```
    pub fn reason(self) -> Option<&'static str> {
        match self {
            LibtestUnitKind::Integration => None,
            LibtestUnitKind::UnitTests => Some(UNIT_TESTS),
            LibtestUnitKind::DocTests => Some(DOC_TESTS),
            LibtestUnitKind::Other => Some(NOT_AN_INTEGRATION_BINARY),
            LibtestUnitKind::Unattributed => Some(UNATTRIBUTED),
        }
    }
}

/// One libtest run within `cargo test`'s output: a binary, or the doctests, with its result
/// line judged by its counts and every test line it printed as a case.
///
/// `name` is a binary's source under `tests/` without the extension (`why`), never the
/// hashed file cargo built; `unittests <path>`, `doc-tests <crate>` or the path, for the
/// kinds a claim cannot name; `a result line` for a result no `Running` line named.
///
/// ```
/// use majordomus_cli::evidence::adapters::{read_libtest, LibtestUnitKind};
/// use majordomus_cli::evidence::Outcome;
///
/// let read = read_libtest(
///     "   Doc-tests fixture\n\
///      test src/lib.rs - add_one (line 5) ... ok\n\
///      test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
///      finished in 0.84s\n",
/// );
/// let doc = &read.units[0];
/// assert_eq!((doc.kind, doc.name.as_str()), (LibtestUnitKind::DocTests, "doc-tests fixture"));
/// assert_eq!(doc.cases[0].name, "src/lib.rs - add_one (line 5)");
/// assert_eq!(doc.outcome, Outcome::Pass);
/// assert!(doc.detail().is_none(), "a pass needs no reason");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibtestUnit {
    /// What the run is.
    pub kind: LibtestUnitKind,
    /// Its name, as described above.
    pub name: String,
    /// What its result line, read by its counts, says happened; an error when it printed
    /// none.
    pub outcome: Outcome,
    /// `finished in` rounded to whole seconds; 0 when the line carried none.
    pub seconds: u64,
    /// Tests that ran and passed.
    pub passed: u64,
    /// Tests that ran and failed.
    pub failed: u64,
    /// Tests that were compiled in and not run.
    pub ignored: u64,
    /// Tests a name filter left out of this run.
    pub filtered_out: u64,
    /// The result line, as printed, without escape sequences.
    pub result: Option<String>,
    /// Every test line it printed, in order, each with what it printed about that test.
    pub cases: Vec<EvidenceCase>,
}

impl LibtestUnit {
    /// Why the run is not a pass, in the runner's words: its result line, or that it
    /// printed none. A pass has no reason to give.
    ///
    /// ```
    /// use majordomus_cli::evidence::adapters::read_libtest;
    /// let read = read_libtest("     Running tests/cut.rs (target/debug/deps/cut-1)\n");
    /// assert!(read.units[0].detail().unwrap().contains("no `test result:` line"));
    /// ```
    pub fn detail(&self) -> Option<String> {
        if self.outcome.proves() {
            return None;
        }
        Some(match &self.result {
            Some(line) => line.clone(),
            None => "no `test result:` line followed it, so the harness could not say what ran"
                .to_string(),
        })
    }

    fn opened(kind: LibtestUnitKind, name: String) -> LibtestUnit {
        LibtestUnit {
            kind,
            name,
            outcome: Outcome::Error,
            seconds: 0,
            passed: 0,
            failed: 0,
            ignored: 0,
            filtered_out: 0,
            result: None,
            cases: Vec::new(),
        }
    }
}

/// Sums over every result line of a libtest reading, whatever run it belongs to.
///
/// ```
/// use majordomus_cli::evidence::adapters::read_libtest;
/// let t = read_libtest("test result: ok. 93 passed; 0 failed; 2 ignored; 0 measured\n").totals();
/// assert_eq!((t.passed, t.failed, t.ignored), (93, 0, 2));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LibtestTotals {
    /// Tests that passed.
    pub passed: u64,
    /// Tests that failed.
    pub failed: u64,
    /// Tests that were ignored.
    pub ignored: u64,
}

/// `cargo test`'s output, read: every libtest run in it, and every line no case holds.
///
/// ```
/// use majordomus_cli::evidence::adapters::{read_libtest, LibtestRead};
/// let read: LibtestRead = read_libtest("");
/// assert!(read.units.is_empty() && read.dropped.is_empty());
/// assert_eq!(read.lines.lines, 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibtestRead {
    /// Each run, in the order the output held it.
    pub units: Vec<LibtestUnit>,
    /// The count of the output's lines.
    pub lines: EvidenceLines,
    /// Every line no case holds.
    pub dropped: Vec<EvidenceDroppedLine>,
}

impl LibtestRead {
    /// The counts of every result line, summed.
    ///
    /// ```
    /// use majordomus_cli::evidence::adapters::read_libtest;
    /// assert_eq!(read_libtest("nothing").totals().passed, 0);
    /// ```
    pub fn totals(&self) -> LibtestTotals {
        self.units
            .iter()
            .fold(LibtestTotals::default(), |t, u| LibtestTotals {
                passed: t.passed + u.passed,
                failed: t.failed + u.failed,
                ignored: t.ignored + u.ignored,
            })
    }
}

/// The run being read, with what is still open in it.
struct Open {
    unit: LibtestUnit,
    /// The captured output of each case, by its index, as lines.
    output: Vec<Vec<String>>,
    /// The case whose captured output the lines now belong to; `Some(None)` for a test no
    /// case line named, whose lines are listed.
    block: Option<Option<usize>>,
    /// Inside a `failures:` or `successes:` section, where the names are restated.
    section: bool,
    /// Notes that a test ran long: the line, and the test it names.
    notes: Vec<(usize, String)>,
}

impl Open {
    fn new(kind: LibtestUnitKind, name: String) -> Open {
        Open {
            unit: LibtestUnit::opened(kind, name),
            output: Vec::new(),
            block: None,
            section: false,
            notes: Vec::new(),
        }
    }

    fn case_index(&self, name: &str) -> Option<usize> {
        self.unit.cases.iter().position(|c| c.name == name)
    }

    /// The run, with each case's captured output and notes joined to it.
    fn close(mut self, tally: &mut Tally) -> LibtestUnit {
        for (i, name) in std::mem::take(&mut self.notes) {
            match self.case_index(&name) {
                Some(k) => {
                    let note = tally.texts[i].trim().to_string();
                    let case = &mut self.unit.cases[k];
                    case.detail = Some(match case.detail.take() {
                        Some(d) => format!("{note}\n{d}"),
                        None => note,
                    });
                }
                None => tally.drop(i, UNNAMED_NOTE),
            }
        }
        for (k, lines) in self.output.into_iter().enumerate() {
            let start = lines.iter().position(|l| !l.trim().is_empty());
            let end = lines.iter().rposition(|l| !l.trim().is_empty());
            let (Some(start), Some(end)) = (start, end) else {
                continue;
            };
            let text = lines[start..=end].join("\n");
            let case = &mut self.unit.cases[k];
            case.detail = Some(match case.detail.take() {
                Some(d) => format!("{d}\n{text}"),
                None => text,
            });
        }
        self.unit
    }
}

/// Read `cargo test`'s output: each `Running` or `Doc-tests` line opens a run, each `test
/// <name> ... <word>` line inside it is a case, the captured output under `---- <name>
/// stdout ----` is that case's detail, and the `test result:` line closes the run, judged by
/// its counts. Escape sequences are stripped from every line first, so a coloured run reads
/// as a plain one.
///
/// A result line is judged by the first of these that holds:
///
/// | The result line | Outcome |
/// |---|---|
/// | a failed count above zero | fail |
/// | the word `FAILED` | fail |
/// | the word `ok` and no `passed` count | error |
/// | the word `ok` and nothing passed (every test ignored, say) | skip |
/// | the word `ok` and a filtered-out count above zero (a subset) | skip |
/// | the word `ok` | pass |
/// | any other word | error |
///
/// A run whose opening line is followed by no result line is an error. A case line's word
/// reads `ok` as a pass, `FAILED` as a failure, `ignored` (with the reason after its comma
/// as the case's detail) as a skip, and anything else as an error with the word as its
/// detail. The lines the harness prints that restate what the case lines said (`running 3
/// tests`, the `failures:` headings and the names under them) are read; blank lines between
/// the parts, cargo's own build and summary lines, and anything else are listed as dropped.
///
/// ```
/// use majordomus_cli::evidence::adapters::read_libtest;
/// use majordomus_cli::evidence::Outcome;
///
/// let read = read_libtest(
///     "   Compiling fixture v0.1.0\n\
///      \x20    Running tests/empty.rs (target/debug/deps/empty-1)\n\
///      test only_ignored ... ignored, needs a network\n\
///      test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; \
///      finished in 0.00s\n",
/// );
/// let empty = &read.units[0];
/// assert_eq!(empty.outcome, Outcome::Skip, "a binary that ran nothing is not a pass");
/// assert_eq!(empty.cases[0].outcome, Outcome::Skip);
/// assert_eq!(empty.cases[0].detail.as_deref(), Some("needs a network"));
/// assert_eq!(read.dropped[0].line, 1, "cargo's own build line, listed");
/// ```
pub fn read_libtest(text: &str) -> LibtestRead {
    let mut tally = Tally::of(text);
    let mut units: Vec<LibtestUnit> = Vec::new();
    let mut open: Option<Open> = None;
    for i in 0..tally.texts.len() {
        let plain = tally.texts[i].clone();
        let t = plain.trim();
        if let Some(rest) = t.strip_prefix("Running ") {
            if let Some(o) = open.take() {
                units.push(o.close(&mut tally));
            }
            // `tests/why.rs (target/debug/deps/why-1a2b)`: the source path is the name,
            // never the hashed binary, which changes between builds
            let mut words = rest.split_whitespace();
            let first = words.next().unwrap_or("");
            open = Some(if first == "unittests" {
                let path = words.next().unwrap_or("");
                Open::new(LibtestUnitKind::UnitTests, format!("unittests {path}"))
            } else if let Some(name) = first
                .strip_prefix("tests/")
                .and_then(|p| p.strip_suffix(".rs"))
            {
                Open::new(LibtestUnitKind::Integration, name.to_string())
            } else {
                Open::new(LibtestUnitKind::Other, first.to_string())
            });
            tally.read(i);
            continue;
        }
        if let Some(rest) = t.strip_prefix("Doc-tests ") {
            if let Some(o) = open.take() {
                units.push(o.close(&mut tally));
            }
            let krate = rest.split_whitespace().next().unwrap_or("");
            open = Some(Open::new(
                LibtestUnitKind::DocTests,
                format!("doc-tests {krate}"),
            ));
            tally.read(i);
            continue;
        }
        if let Some(rest) = t.strip_prefix("test result: ") {
            let o = open.take().unwrap_or_else(|| {
                Open::new(LibtestUnitKind::Unattributed, "a result line".to_string())
            });
            let mut unit = o.close(&mut tally);
            judge(&mut unit, rest);
            unit.result = Some(t.to_string());
            units.push(unit);
            tally.read(i);
            continue;
        }
        let Some(o) = open.as_mut() else {
            tally.drop(i, if t.is_empty() { BLANK } else { OUTSIDE });
            continue;
        };
        if let Some(name) = t
            .strip_prefix("---- ")
            .and_then(|r| r.strip_suffix(" stdout ----"))
        {
            let k = o.case_index(name);
            o.block = Some(k);
            if k.is_some() {
                tally.read(i);
            } else {
                tally.drop(i, UNNAMED_OUTPUT);
            }
            continue;
        }
        if t == "failures:" || t == "successes:" {
            o.block = None;
            o.section = true;
            tally.read(i);
            continue;
        }
        if let Some(block) = o.block {
            match block {
                Some(k) => {
                    if o.output.len() <= k {
                        o.output.resize(k + 1, Vec::new());
                    }
                    o.output[k].push(plain.clone());
                    tally.read(i);
                }
                None => tally.drop(i, UNNAMED_OUTPUT),
            }
            continue;
        }
        if t.starts_with("running ") && (t.ends_with(" tests") || t.ends_with(" test")) {
            tally.read(i);
            continue;
        }
        if let Some(rest) = t.strip_prefix("test ") {
            if let Some((name, word)) = rest.split_once(" ... ") {
                o.unit.cases.push(libtest_case(name, word));
                tally.read(i);
                continue;
            }
            if let Some(name) = rest.strip_suffix(" has been running for over 60 seconds") {
                o.notes.push((i, name.to_string()));
                tally.read(i);
                continue;
            }
        }
        if o.section && plain.starts_with("    ") && o.case_index(t).is_some() {
            tally.read(i);
            continue;
        }
        tally.drop(i, if t.is_empty() { BLANK } else { INSIDE });
    }
    if let Some(o) = open.take() {
        units.push(o.close(&mut tally));
    }
    let (lines, dropped) = tally.finish(EvidenceProducer::Crate, EvidenceStream::Output);
    LibtestRead {
        units,
        lines,
        dropped,
    }
}

/// One `test <name> ... <word>` line as a case.
fn libtest_case(name: &str, word: &str) -> EvidenceCase {
    let word = word.trim();
    let (outcome, detail) = match word {
        "ok" => (Outcome::Pass, None),
        "FAILED" => (Outcome::Fail, None),
        "ignored" => (Outcome::Skip, None),
        w if w.starts_with("ignored, ") => (
            Outcome::Skip,
            Some(w.trim_start_matches("ignored, ").to_string()),
        ),
        w if w.starts_with("bench:") => (Outcome::Pass, Some(w.to_string())),
        w => (Outcome::Error, Some(w.to_string())),
    };
    EvidenceCase {
        name: name.to_string(),
        outcome,
        seconds: None,
        phase: None,
        detail,
    }
}

/// A run's result line, `<word>. <counts>; finished in <S>s`, judged by the table of
/// [`read_libtest`] into `unit`.
fn judge(unit: &mut LibtestUnit, rest: &str) {
    let (word, counts) = rest.split_once('.').unwrap_or((rest, ""));
    let word = word.trim();
    let (mut passed, mut failed, mut ignored, mut filtered_out) = (None, 0, 0, 0);
    let mut seconds = 0;
    for part in counts.split(';').map(str::trim) {
        if let Some(s) = part.strip_prefix("finished in ") {
            seconds = s
                .trim_end_matches('s')
                .trim()
                .parse::<f64>()
                .map(|f| f.round() as u64)
                .unwrap_or(0);
            continue;
        }
        let Some((n, label)) = part.split_once(' ') else {
            continue;
        };
        let Ok(n) = n.parse::<u64>() else { continue };
        match label.trim() {
            "passed" => passed = Some(n),
            "failed" => failed = n,
            "ignored" => ignored = n,
            "filtered out" => filtered_out = n,
            _ => {}
        }
    }
    unit.outcome = if failed > 0 || word == "FAILED" {
        Outcome::Fail
    } else if word == "ok" {
        match passed {
            // the line said `ok` and not what ran: nothing here can say it passed
            None => Outcome::Error,
            // it ran nothing, or only a subset a filter chose: a skip, not the proof
            Some(0) => Outcome::Skip,
            Some(_) if filtered_out > 0 => Outcome::Skip,
            Some(_) => Outcome::Pass,
        }
    } else {
        Outcome::Error
    };
    unit.seconds = seconds;
    unit.passed = passed.unwrap_or(0);
    unit.failed = failed;
    unit.ignored = ignored;
    unit.filtered_out = filtered_out;
}

/// A line with its terminal escape sequences removed: `ESC [` up to its final byte (`@`
/// to `~`), `ESC ]` up to BEL or `ESC \`, `ESC` with intermediate bytes (space to `/`) up to
/// the final byte after them, and any other `ESC` with the character after it.
/// `cargo test` colours its output when it thinks a person is watching, and a coloured
/// `Running` or `ok` is the same line as a plain one. The test harness resets its colour
/// through the terminal's own `sgr0`, which on an xterm is `ESC ( B ESC [ m`: the `( B`
/// selects the character set, and reading it as a two-character pair would leave its `B`
/// behind and turn `ok` into `okB`.
pub(crate) fn strip_ansi(line: &str) -> String {
    const ESC: char = '\u{1b}';
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != ESC {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                let mut after_esc = false;
                for c in chars.by_ref() {
                    if c == '\u{7}' || (after_esc && c == '\\') {
                        break;
                    }
                    after_esc = c == ESC;
                }
            }
            // intermediate bytes, then the one final byte that ends the sequence
            Some(' '..='/') => {
                while chars.next_if(|c| (' '..='/').contains(c)).is_some() {}
                chars.next();
            }
            Some(_) | None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test/fixtures/evidence-adapters")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// Every line is read or listed, once: the count says so, and the listed lines are
    /// exactly the lines the count does not hold.
    fn accounted(lines: &EvidenceLines, dropped: &[EvidenceDroppedLine], text: &str) {
        assert_eq!(lines.lines, text.lines().count());
        let mine: Vec<&EvidenceDroppedLine> = dropped
            .iter()
            .filter(|d| d.producer == lines.producer && d.stream == lines.stream)
            .collect();
        assert_eq!(lines.read + mine.len(), lines.lines, "{mine:?}");
        let mut numbers: Vec<usize> = mine.iter().map(|d| d.line).collect();
        numbers.dedup();
        assert_eq!(numbers.len(), mine.len(), "a line listed twice");
        let texts: Vec<String> = text.lines().map(strip_ansi).collect();
        for d in mine {
            assert_eq!(d.text, texts[d.line - 1], "line {}", d.line);
        }
    }

    #[test]
    fn a_real_cargo_run_is_read_line_by_line() {
        let text = fixture("cargo-test.txt");
        let read = read_libtest(&text);
        accounted(&read.lines, &read.dropped, &text);
        let units: Vec<(LibtestUnitKind, &str, Outcome)> = read
            .units
            .iter()
            .map(|u| (u.kind, u.name.as_str(), u.outcome))
            .collect();
        assert_eq!(
            units,
            [
                (
                    LibtestUnitKind::UnitTests,
                    "unittests src/lib.rs",
                    Outcome::Pass
                ),
                (LibtestUnitKind::Integration, "empty", Outcome::Skip),
                (LibtestUnitKind::Integration, "green", Outcome::Pass),
                (LibtestUnitKind::Integration, "red", Outcome::Fail),
                (
                    LibtestUnitKind::DocTests,
                    "doc-tests fixture",
                    Outcome::Pass
                ),
            ]
        );
        let red = &read.units[3];
        let cases: Vec<(&str, Outcome)> = red
            .cases
            .iter()
            .map(|c| (c.name.as_str(), c.outcome))
            .collect();
        assert_eq!(
            cases,
            [
                ("ignored_plain", Outcome::Skip),
                ("passes", Outcome::Pass),
                ("fails_too", Outcome::Fail),
                ("fails_with_output", Outcome::Fail),
            ]
        );
        let output = red.cases[3].detail.as_deref().unwrap();
        assert!(
            output.starts_with("a line the test printed")
                && output.contains("arithmetic is broken")
                && output.ends_with("right: 3"),
            "{output}"
        );
        assert!(red.cases[2]
            .detail
            .as_deref()
            .unwrap()
            .contains("second failure"));
        assert!(red.detail().unwrap().starts_with("test result: FAILED."));
        let unit_tests = &read.units[0];
        assert_eq!(
            unit_tests.cases[0].detail.as_deref(),
            Some("needs a network")
        );
        // the dropped lines are cargo's own and the blank separators, and no case line
        for d in &read.dropped {
            assert!(!d.text.trim_start().starts_with("test "), "{d:?}");
        }
        assert!(read
            .dropped
            .iter()
            .any(|d| d.text.contains("to rerun pass `--test red`") && d.reason == OUTSIDE));
        assert_eq!(read.totals().failed, 2);
    }

    #[test]
    fn a_coloured_run_reads_as_the_plain_one() {
        let text = fixture("cargo-test-color.txt");
        assert!(text.contains('\u{1b}'), "the fixture is coloured");
        let read = read_libtest(&text);
        accounted(&read.lines, &read.dropped, &text);
        assert_eq!(read.units.len(), 1);
        assert_eq!(read.units[0].outcome, Outcome::Pass);
        assert!(read.units[0]
            .cases
            .iter()
            .all(|c| c.outcome == Outcome::Pass));
        assert_eq!(read.units[0].cases[2].name, "panics - should panic");
    }

    #[test]
    fn a_filtered_run_is_a_skip_with_its_reason() {
        let text = fixture("cargo-test-filtered.txt");
        let read = read_libtest(&text);
        accounted(&read.lines, &read.dropped, &text);
        assert_eq!(read.units[0].outcome, Outcome::Skip);
        assert!(read.units[0].detail().unwrap().contains("2 filtered out"));
    }

    #[test]
    fn real_suite_runs_are_read_line_by_line() {
        for mode in ["serial", "parallel", "no-skips"] {
            let report = fixture(&format!("suite-{mode}.tsv"));
            let output = fixture(&format!("suite-{mode}.txt"));
            let read = read_suite(&report, Some(&output)).unwrap();
            assert_eq!(read.lines.len(), 2, "{mode}");
            accounted(&read.lines[0], &read.dropped, &report);
            accounted(&read.lines[1], &read.dropped, &output);
            let fails = read.cases.iter().find(|c| c.name == "02_fails").unwrap();
            assert_eq!(fails.outcome, Outcome::Fail, "{mode}");
            assert_eq!(
                fails.log,
                ["    expected /proven/ in output:", "not run"],
                "{mode}"
            );
            for d in &read.dropped {
                assert_eq!(d.stream, EvidenceStream::Output, "{mode}: {d:?}");
                assert!(
                    [RUNNER, RUNNER_TOTALS, RUNNER_FAILED].contains(&d.reason.as_str()),
                    "{mode}: {d:?}"
                );
            }
        }
        let serial = read_suite(
            &fixture("suite-serial.tsv"),
            Some(&fixture("suite-serial.txt")),
        )
        .unwrap();
        let skipped = serial
            .cases
            .iter()
            .find(|c| c.name == "03_declines")
            .unwrap();
        assert_eq!(
            skipped.detail().as_deref(),
            Some("    skip: no zola on this machine")
        );
        let no_skips = read_suite(
            &fixture("suite-no-skips.tsv"),
            Some(&fixture("suite-no-skips.txt")),
        )
        .unwrap();
        let hangs = no_skips
            .cases
            .iter()
            .find(|c| c.name == "05_hangs")
            .unwrap();
        assert_eq!(hangs.outcome, Outcome::Timeout);
        assert!(hangs.detail().unwrap().contains("did not finish within"));
        let declined = no_skips
            .cases
            .iter()
            .find(|c| c.name == "03_declines")
            .unwrap();
        assert!(
            declined
                .log
                .last()
                .unwrap()
                .starts_with("FAIL 03_declines declined"),
            "{declined:?}"
        );
    }

    #[test]
    fn output_no_verdict_follows_and_unreported_cases_are_listed() {
        let read = read_suite(
            "01_a\tok\t1\tserial\n",
            Some("    left over\nFAIL 99_ghost\nok   01_a\n    trailing\n"),
        )
        .unwrap();
        let reasons: Vec<(usize, &str)> = read
            .dropped
            .iter()
            .map(|d| (d.line, d.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                (1, UNREPORTED_OUTPUT),
                (2, UNREPORTED_VERDICT),
                (4, NO_VERDICT)
            ]
        );
        assert!(read.cases[0].log.is_empty());
    }

    #[test]
    fn verdict_shapes_are_the_runners_own() {
        assert_eq!(suite_verdict("ok   01_a"), Some(("01_a", false)));
        assert_eq!(suite_verdict("FAIL 01_a (setup)"), Some(("01_a", false)));
        assert_eq!(suite_verdict("ok      01_a  12s"), Some(("01_a", false)));
        assert_eq!(
            suite_verdict("skip 01_a is excused here: its subject is read in the ui job"),
            Some(("01_a", true))
        );
        assert_eq!(suite_verdict("    ok   01_a"), None, "indented: a log line");
        assert_eq!(suite_verdict("ok 01_a and more"), None);
        assert_eq!(suite_verdict("FAIL run.sh: the checkout changed"), None);
        assert_eq!(suite_verdict("okay 01_a"), None);
    }

    #[test]
    fn a_libtest_case_word_reads_as_its_outcome() {
        let o = |w: &str| libtest_case("x", w);
        assert_eq!(o("ok").outcome, Outcome::Pass);
        assert_eq!(o("FAILED").outcome, Outcome::Fail);
        assert_eq!(o("ignored").outcome, Outcome::Skip);
        assert_eq!(o("ignored, slow").detail.as_deref(), Some("slow"));
        assert_eq!(o("bench: 10 ns/iter").outcome, Outcome::Pass);
        let odd = o("interrupted");
        assert_eq!(
            (odd.outcome, odd.detail.as_deref()),
            (Outcome::Error, Some("interrupted"))
        );
    }

    #[test]
    fn a_slow_note_joins_its_case_or_is_listed() {
        let read = read_libtest(
            "     Running tests/slow.rs (target/debug/deps/slow-1)\n\
             test slow has been running for over 60 seconds\n\
             test ghost has been running for over 60 seconds\n\
             test slow ... ok\n\
             test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
        );
        assert_eq!(
            read.units[0].cases[0].detail.as_deref(),
            Some("test slow has been running for over 60 seconds")
        );
        assert_eq!(read.dropped.len(), 1);
        assert_eq!(
            (read.dropped[0].line, read.dropped[0].reason.as_str()),
            (3, UNNAMED_NOTE)
        );
    }
}
