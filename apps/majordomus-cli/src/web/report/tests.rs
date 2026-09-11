//! The test report: the suite's own results, rendered.
//!
//! The evidence is what the runs already write — `test/run.sh`'s TSV report for the
//! behavioural cases and `cargo test`'s summary lines for the crate — and this module
//! parses, joins and renders it. It decides nothing about what passed: a case that failed
//! is a case the runner said failed, and a report that disagreed with the runner would be
//! worse than no report.
//!
//! The lifecycle is parse, join, render: the runner's TSV and cargo's summary lines become
//! one [`Run`], and the run becomes a directory that declares itself.
//!
//! ```
//! use majordomus_cli::web::report::tests;
//! let mut run =
//!     tests::parse_cases("69_context\tok\t12\tparallel\n96_quality\tok\t4\tserial\n").unwrap();
//! run.crate_tests = Some(tests::parse_crate_tests(
//!     "test result: ok. 93 passed; 0 failed; 0 ignored\n",
//! ));
//! assert!(run.green());
//!
//! let tmp = tempfile::tempdir().unwrap();
//! let dir = tests::render(tmp.path(), &run).unwrap();
//! // the rendering and the evidence it was made from, in one self-declaring directory
//! assert!(dir.join("index.html").is_file());
//! assert!(dir.join("results.json").is_file());
//! assert!(dir.join("surface.json").is_file());
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{html, Origin};
use crate::error::{Error, Result};

/// The identity of the surface this report owns.
///
/// Named here rather than at each call site because the UI conformance report is a section
/// of this same surface (`/tests/ui`) and has to name the surface it writes into. Two
/// spellings of one identity is how a directory ends up declared twice, differently.
pub const SURFACE_ID: &str = "tests";
/// The surface's title, in a listing.
pub const SURFACE_TITLE: &str = "Test results";
/// What produces it, for a reader who has to rebuild it.
pub const SURFACE_PRODUCER: &str =
    "bash test/run.sh + cargo test, rendered by majordomus web report tests";

/// One behavioural case, as the runner recorded it.
///
/// The fields are the runner's own columns and are kept as it wrote them — `result` is a
/// string and not a bool, because the runner's word is the evidence and normalising it here
/// would put this module in the position of deciding what passed. [`Case::passed`] reads
/// that word; it does not replace it.
///
/// ```
/// use majordomus_cli::web::report::tests::{self, Case};
/// let run = tests::parse_cases("96_quality\tFAIL\t7\texclusive\n").unwrap();
/// let case: &Case = &run.cases[0];
/// assert_eq!(case.name, "96_quality");
/// assert_eq!(case.result, "FAIL", "the runner's own word is kept, not a bool");
/// assert_eq!(case.phase, "exclusive");
/// assert!(!case.passed());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    /// The case's name, which is its file's stem.
    pub name: String,
    /// `ok` or `FAIL`, as the runner wrote it.
    pub result: String,
    /// How long it took, in whole seconds.
    pub seconds: u64,
    /// The phase it ran in: parallel, exclusive or serial.
    pub phase: String,
}

impl Case {
    /// Did it pass, by the runner's own word for it?
    ///
    /// Exactly `ok`, in any case, and nothing else: a case whose result the runner spelled
    /// some other way has not passed. Read that way round deliberately — anything this
    /// module failed to recognise counts as a failure, so a report can only ever
    /// understate a green run and never overstate one.
    ///
    /// ```
    /// use majordomus_cli::web::report::tests;
    /// let run = tests::parse_cases("a\tok\t1\tparallel\nb\tOK\t1\tparallel\n\
    ///                              c\tFAIL\t1\tparallel\nd\ttimeout\t1\tparallel\n")
    ///     .unwrap();
    /// let passed: Vec<bool> = run.cases.iter().map(|c| c.passed()).collect();
    /// assert_eq!(passed, vec![true, true, false, false]);
    /// ```
    pub fn passed(&self) -> bool {
        self.result.eq_ignore_ascii_case("ok")
    }
}

/// A whole run: the cases, and what the crate's own tests said.
///
/// Two kinds of evidence in one value, because a reader asking "is the suite green?" means
/// both. The crate's totals are optional: a run of the behavioural cases alone is a run,
/// and a `None` there says "not measured" rather than "nothing failed".
///
/// ```
/// use majordomus_cli::web::report::tests::{self, CrateTests, Run};
/// let mut run: Run = tests::parse_cases("a\tok\t1\tparallel\n").unwrap();
/// assert!(run.green(), "the cases passed and the crate was not measured");
/// // a crate failure makes the same set of cases a red run
/// run.crate_tests = Some(CrateTests { passed: 92, failed: 1, ignored: 0 });
/// assert!(!run.green());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The behavioural cases, in the order the report recorded them.
    pub cases: Vec<Case>,
    /// The crate's test totals, when a cargo run was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crate_tests: Option<CrateTests>,
}

/// What `cargo test` reported, summed over its binaries.
///
/// A sum and not a per-binary breakdown: the machine-readable format cargo would give one
/// from is still unstable, so this is read from the human summary lines every cargo prints
/// and only the totals survive that. `ignored` is carried rather than dropped, because a
/// test that is not run is not a test that passed.
///
/// ```
/// use majordomus_cli::web::report::tests;
/// let totals = tests::parse_crate_tests(
///     "test result: ok. 93 passed; 0 failed; 2 ignored; 0 measured\n\
///      test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured\n",
/// );
/// assert_eq!((totals.passed, totals.failed, totals.ignored), (98, 0, 2));
/// // a run nobody measured is not the same value as a run that measured nothing
/// assert_eq!(tests::parse_crate_tests("nothing here"), tests::CrateTests::default());
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateTests {
    /// Tests that passed.
    pub passed: usize,
    /// Tests that failed.
    pub failed: usize,
    /// Tests that were ignored.
    pub ignored: usize,
}

impl Run {
    /// How many of the behavioural cases the runner said passed.
    ///
    /// The behavioural cases only: the crate's own tests are counted by whatever ran them
    /// and are carried separately, so this number is never a total of the two.
    ///
    /// ```
    /// use majordomus_cli::web::report::tests;
    /// let run = tests::parse_cases("a\tok\t1\tparallel\nb\tFAIL\t2\texclusive\n").unwrap();
    /// assert_eq!(run.passed(), 1);
    /// assert_eq!(run.passed() + run.failed(), run.cases.len());
    /// ```
    pub fn passed(&self) -> usize {
        self.cases.iter().filter(|c| c.passed()).count()
    }

    /// How many cases did not pass, which is every case that was not recorded as `ok`.
    ///
    /// Derived from the total rather than counted on its own, so the two numbers cannot
    /// disagree about a case whose result nobody recognised: such a case is a failure, and
    /// it is a failure in both readings.
    ///
    /// ```
    /// use majordomus_cli::web::report::tests;
    /// let run = tests::parse_cases("a\tok\t1\tparallel\nb\tsomething-else\t2\tserial\n")
    ///     .unwrap();
    /// assert_eq!(run.failed(), 1, "a result this module does not recognise is not a pass");
    /// ```
    pub fn failed(&self) -> usize {
        self.cases.len() - self.passed()
    }

    /// The whole run's wall time, as the sum of the seconds the runner recorded.
    ///
    /// A sum of recorded evidence and not a measurement of anything: this module runs
    /// nothing and times nothing, and the figure is only as good as what the runner wrote.
    /// Cases that ran in parallel are added up too, so this is the work the run did rather
    /// than how long a person waited.
    ///
    /// ```
    /// use majordomus_cli::web::report::tests;
    /// let run = tests::parse_cases("a\tok\t12\tparallel\nb\tok\t3\tparallel\n").unwrap();
    /// assert_eq!(run.seconds(), 15, "the sum of what the runner recorded");
    /// ```
    pub fn seconds(&self) -> u64 {
        self.cases.iter().map(|c| c.seconds).sum()
    }

    /// Did everything the run measured pass?
    ///
    /// Both kinds of evidence, and unmeasured is not counted against the run: a `Run` with
    /// no crate totals is green when its cases are, because "nobody ran the crate's tests"
    /// is not the same claim as "they failed". A single failure of either kind is enough to
    /// make it false.
    ///
    /// ```
    /// use majordomus_cli::web::report::tests::{self, CrateTests};
    /// let mut run = tests::parse_cases("a\tok\t1\tparallel\n").unwrap();
    /// assert!(run.green());
    /// run.crate_tests = Some(CrateTests { passed: 0, failed: 1, ignored: 0 });
    /// assert!(!run.green(), "a crate failure is a failure of the run");
    ///
    /// let red = tests::parse_cases("a\tFAIL\t1\tparallel\n").unwrap();
    /// assert!(!red.green());
    /// ```
    pub fn green(&self) -> bool {
        self.failed() == 0 && self.crate_tests.map(|t| t.failed == 0).unwrap_or(true)
    }
}

/// Parse the runner's TSV: `name <TAB> result <TAB> seconds <TAB> phase`, one case per line.
///
/// A malformed line is refused rather than skipped: a report that silently dropped a case
/// would claim a run was smaller than it was.
///
/// ```
/// use majordomus_cli::web::report::tests::parse_cases;
/// let run = parse_cases("69_context\tok\t12\tparallel\n99_adr\tFAIL\t3\texclusive\n").unwrap();
/// assert_eq!(run.passed(), 1);
/// assert_eq!(run.failed(), 1);
/// assert_eq!(run.seconds(), 15);
/// ```
pub fn parse_cases(text: &str) -> Result<Run> {
    let mut cases = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            return Err(Error::InvalidSurface {
                surface: "tests".into(),
                reason: format!(
                    "line {} of the run report has {} field(s); it is name<TAB>result<TAB>seconds<TAB>phase",
                    n + 1,
                    fields.len()
                ),
            });
        }
        cases.push(Case {
            name: fields[0].to_string(),
            result: fields[1].to_string(),
            seconds: fields[2].parse().unwrap_or(0),
            phase: fields[3].to_string(),
        });
    }
    Ok(Run {
        cases,
        crate_tests: None,
    })
}

/// Read `cargo test`'s human output and sum its result lines.
///
/// The machine format is still unstable, so this reads what every cargo prints:
/// `test result: ok. 93 passed; 0 failed; 0 ignored; ...`, once per test binary.
///
/// ```
/// use majordomus_cli::web::report::tests::parse_crate_tests;
/// let t = parse_crate_tests("test result: ok. 93 passed; 0 failed; 2 ignored; 0 measured\n\
///                            test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured\n");
/// assert_eq!(t.passed, 98);
/// assert_eq!(t.failed, 1);
/// assert_eq!(t.ignored, 2);
/// ```
pub fn parse_crate_tests(text: &str) -> CrateTests {
    let mut totals = CrateTests::default();
    for line in text.lines().filter(|l| l.contains("test result:")) {
        // the shape is "<n> passed; <n> failed; <n> ignored": read each label with the
        // number that precedes it, rather than assuming where in the line either sits
        let words: Vec<&str> = line.split_whitespace().collect();
        for (i, word) in words.iter().enumerate().skip(1) {
            let label = word.trim_end_matches(|c: char| !c.is_ascii_alphabetic());
            let field = match label {
                "passed" => &mut totals.passed,
                "failed" => &mut totals.failed,
                "ignored" => &mut totals.ignored,
                _ => continue,
            };
            if let Ok(count) = words[i - 1].parse::<usize>() {
                *field += count;
            }
        }
    }
    totals
}

/// Render the run into its own directory under the generated web root, and declare it.
///
/// Returns the directory written.
///
/// Three files: the page, the results it was made from, and the declaration that makes the
/// directory a surface. The verdict on the page is the run's own — [`Run::green`] — and the
/// failures are listed first, because a report whose reader has to scroll to find out
/// whether it is red is a report that will be read as green.
///
/// The UI conformance section is linked only when it is actually there: a report that
/// linked a page answering 404 would be worse than one that says nothing about it.
///
/// ```
/// use majordomus_cli::web::report::tests;
/// let tmp = tempfile::tempdir().unwrap();
/// let run = tests::parse_cases("a\tok\t1\tparallel\nb\tFAIL\t2\texclusive\n").unwrap();
/// let dir = tests::render(tmp.path(), &run).unwrap();
///
/// let page = std::fs::read_to_string(dir.join("index.html")).unwrap();
/// assert!(page.contains("failures"), "a red run says so on the page");
/// assert!(!page.contains("UI conformance"), "an absent section is not linked");
/// // the evidence is written beside the page, so the page can be checked
/// assert!(dir.join("results.json").is_file());
/// ```
pub fn render(root: &Path, run: &Run) -> Result<std::path::PathBuf> {
    let dir = super::declare(root, SURFACE_ID, SURFACE_TITLE, SURFACE_PRODUCER)?;
    let origin = Origin::read(root);
    super::write_json(&dir, "results.json", run)?;

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut ordered = run.cases.clone();
    ordered.sort_by(|a, b| {
        b.passed()
            .cmp(&a.passed())
            .reverse()
            .then(a.name.cmp(&b.name))
    });
    for case in &ordered {
        let mark = if case.passed() {
            "<span class=\"pass\">ok</span>".to_string()
        } else {
            format!("<span class=\"fail\">{}</span>", html::escape(&case.result))
        };
        rows.push(vec![
            format!("<span class=\"mono\">{}</span>", html::escape(&case.name)),
            mark,
            format!("<span class=\"num\">{}</span>", case.seconds),
            html::escape(&case.phase),
        ]);
    }

    let mut summary_items = vec![
        ("cases", run.cases.len().to_string()),
        ("passed", run.passed().to_string()),
        ("failed", run.failed().to_string()),
        ("seconds", run.seconds().to_string()),
    ];
    if let Some(crate_tests) = run.crate_tests {
        summary_items.push(("crate tests", crate_tests.passed.to_string()));
        summary_items.push(("crate failures", crate_tests.failed.to_string()));
    }

    let verdict = if run.green() {
        "Every case the run measured passed."
    } else {
        "The run has failures; each is named below."
    };
    // the UI conformance audit writes its own section of this surface; link it when it is
    // there, and say nothing when it is not, rather than linking a page that answers 404
    let ui = if dir.join(super::ui::SECTION).join("index.html").is_file() {
        format!(
            "<h2>UI conformance</h2><p><a href=\"{}/\">What a browser found on every page of \
             the built site</a>.</p>",
            super::ui::SECTION
        )
    } else {
        String::new()
    };
    let body = format!(
        "{}<h2>Behavioural cases</h2>{}{}{}",
        html::summary(&summary_items),
        html::table(&["case", "result", "seconds", "phase"], &rows),
        ui,
        html::origin(&origin, &[("results.json", "results.json")])
    );
    let page = html::page("Test results", verdict, &body);
    super::write(&dir, "index.html", &page)?;
    Ok(dir)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn a_malformed_line_is_refused_rather_than_dropped() {
        let err = parse_cases("only-a-name\n").unwrap_err().to_string();
        assert!(err.contains("field"), "{err}");
    }

    #[test]
    fn a_rendered_report_declares_itself_and_carries_its_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let run = parse_cases("a\tok\t1\tparallel\nb\tFAIL\t2\texclusive\n").unwrap();
        let dir = render(tmp.path(), &run).unwrap();
        let page = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(page.contains("Test results"));
        assert!(page.contains("failures"), "a red run says so: {page}");
        assert!(dir.join("results.json").is_file());
        let surfaces = crate::web::discover::generated(tmp.path()).unwrap();
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].mount.as_str(), "/tests");
    }

    #[test]
    fn a_green_run_says_so() {
        let tmp = tempfile::tempdir().unwrap();
        let run = parse_cases("a\tok\t1\tparallel\n").unwrap();
        let dir = render(tmp.path(), &run).unwrap();
        let page = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(
            page.contains("Every case the run measured passed"),
            "{page}"
        );
    }
}
