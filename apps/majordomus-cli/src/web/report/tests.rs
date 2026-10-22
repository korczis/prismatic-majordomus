//! The test report: the suite's own results, rendered.
//!
//! The evidence is what the runs already write — `test/run.sh`'s TSV report for the
//! behavioural cases and `cargo test`'s summary lines for the crate — and this module
//! renders it. It reads both through the evidence adapters
//! ([`crate::evidence::adapters`]), the same readers the recorder uses, so the page and the
//! recorded evidence cannot read one run two ways. It decides nothing about what passed: a
//! case that failed is a case the runner said failed, and a report that disagreed with the
//! runner would be worse than no report.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{html, Origin};
use crate::error::{Error, Result};
use crate::evidence::adapters::{read_libtest, read_suite};

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
    /// Did it pass?
    pub fn passed(&self) -> bool {
        self.result.eq_ignore_ascii_case("ok")
    }
}

/// A whole run: the cases, and what the crate's own tests said.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The behavioural cases, in the order the report recorded them.
    pub cases: Vec<Case>,
    /// The crate's test totals, when a cargo run was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crate_tests: Option<CrateTests>,
}

/// What `cargo test` reported, summed over its binaries.
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
    /// How many cases passed.
    pub fn passed(&self) -> usize {
        self.cases.iter().filter(|c| c.passed()).count()
    }

    /// How many cases failed.
    pub fn failed(&self) -> usize {
        self.cases.len() - self.passed()
    }

    /// The whole run's wall time, as the sum of its cases.
    pub fn seconds(&self) -> u64 {
        self.cases.iter().map(|c| c.seconds).sum()
    }

    /// Did everything the run measured pass?
    pub fn green(&self) -> bool {
        self.failed() == 0 && self.crate_tests.map(|t| t.failed == 0).unwrap_or(true)
    }
}

/// Parse the runner's TSV: `name <TAB> result <TAB> seconds <TAB> phase`, one case per line.
///
/// A malformed line is refused rather than skipped: a report that silently dropped a case
/// would claim a run was smaller than it was. The rows are read by the suite adapter
/// ([`crate::evidence::adapters::read_suite`]), the one the recorder reads them with.
///
/// ```
/// use majordomus_cli::web::report::tests::parse_cases;
/// let run = parse_cases("69_context\tok\t12\tparallel\n99_adr\tFAIL\t3\texclusive\n").unwrap();
/// assert_eq!(run.passed(), 1);
/// assert_eq!(run.failed(), 1);
/// assert_eq!(run.seconds(), 15);
/// ```
pub fn parse_cases(text: &str) -> Result<Run> {
    let read = read_suite(text, None).map_err(|reason| Error::InvalidSurface {
        surface: "tests".into(),
        reason,
    })?;
    let cases = read
        .cases
        .into_iter()
        .map(|c| Case {
            name: c.name,
            result: c.word,
            seconds: c.seconds,
            phase: c.phase,
        })
        .collect();
    Ok(Run {
        cases,
        crate_tests: None,
    })
}

/// Read `cargo test`'s human output and sum its result lines.
///
/// The machine format is still unstable, so this reads what every cargo prints:
/// `test result: ok. 93 passed; 0 failed; 0 ignored; ...`, once per test binary, through
/// the libtest adapter ([`crate::evidence::adapters::read_libtest`]) the recorder reads it
/// with, so a coloured run sums as the plain one does.
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
    let totals = read_libtest(text).totals();
    let count = |n: u64| usize::try_from(n).unwrap_or(usize::MAX);
    CrateTests {
        passed: count(totals.passed),
        failed: count(totals.failed),
        ignored: count(totals.ignored),
    }
}

/// Render the run into its own directory under the generated web root, and declare it.
///
/// Returns the directory written.
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

    /// The page and the recorder read one run the same way: the suite's rows and the
    /// crate's totals are what the adapters read, a coloured cargo line included.
    #[test]
    fn the_report_reads_what_the_adapters_read() {
        let tsv = "01_a\tok\t1\tparallel\n02_b\tTIMEOUT\t9\texclusive\n";
        let run = parse_cases(tsv).unwrap();
        let read = crate::evidence::adapters::read_suite(tsv, None).unwrap();
        let ours: Vec<(&str, &str, u64, &str)> = run
            .cases
            .iter()
            .map(|c| {
                (
                    c.name.as_str(),
                    c.result.as_str(),
                    c.seconds,
                    c.phase.as_str(),
                )
            })
            .collect();
        let theirs: Vec<(&str, &str, u64, &str)> = read
            .cases
            .iter()
            .map(|c| {
                (
                    c.name.as_str(),
                    c.word.as_str(),
                    c.seconds,
                    c.phase.as_str(),
                )
            })
            .collect();
        assert_eq!(ours, theirs);
        let e = '\u{1b}';
        let coloured = format!(
            "test result: {e}[32mok{e}(B{e}[m. 3 passed; 0 failed; 1 ignored; 0 measured\n"
        );
        let t = parse_crate_tests(&coloured);
        assert_eq!((t.passed, t.failed, t.ignored), (3, 0, 1));
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
