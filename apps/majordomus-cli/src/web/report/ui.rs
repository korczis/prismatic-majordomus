//! The UI conformance report: what a browser found on every page of the built site.
//!
//! The evidence is the results document `scripts/ui audit` writes — the pages it visited,
//! the widths it visited them at, and every finding, each already naming the rule it broke
//! and the elements that broke it. This module parses and renders it. It decides nothing:
//! a page is conformant here exactly when the audit found nothing on it, and a report that
//! disagreed with the audit would be worse than no report.
//!
//! The rendering is a *section of the test surface* rather than a surface of its own. A UI
//! conformance run is a test run, `/tests/ui` is where a reader looks for it, and the
//! topology refuses a surface mounted inside another's subtree (`surface.nested-mount`), so
//! the architecture has already answered where this belongs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{html, tests, Origin};
use crate::error::{Error, Result};

/// The contract of the results document, written by `scripts/lib/ui-run.mjs`.
pub const RESULTS_SCHEMA: &str = "ui-audit/v1";

/// The directory of this report inside the test surface, and the path it is read at.
pub const SECTION: &str = "ui";

/// One element a finding names, as the browser saw it.
///
/// The selector is the only field every rule produces; a rule that measured more — how far
/// past the viewport an element reached, what its position was — carries it here rather
/// than in prose, so the report can render it without knowing which rule it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    /// A selector a person can paste into the browser's console.
    pub selector: String,
    /// Whatever else the rule measured about it.
    #[serde(flatten)]
    pub measured: BTreeMap<String, Value>,
}

/// One thing wrong on one page at one width.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// The page.
    pub route: String,
    /// The width it was visited at, in CSS pixels.
    pub width: u32,
    /// How thoroughly the page was visited: `sweep` or `critical`.
    #[serde(default)]
    pub tier: String,
    /// The rule it broke, stable enough to grep for.
    pub rule: String,
    /// What is wrong, in one line.
    pub detail: String,
    /// The elements that broke it, when the rule could name them.
    #[serde(default)]
    pub elements: Vec<Element>,
}

/// One surface the audit measured, and what it contributed.
///
/// A built surface is a directory, and every page of it is visited. A surface the executable
/// renders has no directory: its routes are crawled out of its own anchors and a sample of
/// each family is visited, so the number of routes it *has* is reported beside the number
/// that was *sampled*. A sample rendered as a total would be a report overstating its own
/// coverage, and the Cockpit's thousands of routes are why that distinction belongs on the
/// page rather than in somebody's head.
///
/// ```
/// use majordomus_cli::web::report::ui::{parse, Surface};
/// let run = parse(r#"{"schema":"ui-audit/v1","pages":108,"visits":478,"findings":[],
///     "surfaces":[{"id":"cockpit","mount":"/cockpit","kind":"native-route",
///                  "routes":2660,"families":14,"sampled":108}]}"#).unwrap();
/// let surface: &Surface = &run.surfaces[0];
/// assert_eq!((surface.routes, surface.sampled), (2660, 108));
/// assert!(!surface.truncated, "a crawl that hit its budget reports a floor, not a total");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    /// The surface's id in the topology.
    pub id: String,
    /// Where the executable serves it.
    pub mount: String,
    /// `static-directory` or `native-route`.
    #[serde(default)]
    pub kind: String,
    /// How many routes the surface has, when they were crawled rather than read from disk.
    #[serde(default)]
    pub routes: usize,
    /// How many families those routes fell into.
    #[serde(default)]
    pub families: usize,
    /// How many of them the audit visited.
    #[serde(default)]
    pub sampled: usize,
    /// Whether the crawl hit its budget, so the route count is a floor rather than a total.
    #[serde(default)]
    pub truncated: bool,
}

/// A subtree of a page the engine was told to skip, and what declared it.
///
/// A page may carry a component tree that is not this repository's to fix — pinned at a
/// version, its markup and its stylesheet arriving together. The page declares it with
/// `data-mj-foreign`; the engine skips it, and the report says so. "Not measured" and
/// "measured and clean" are different claims, and a report that conflated them would be
/// worth nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Foreign {
    /// A page it was seen on.
    #[serde(default)]
    pub route: String,
    /// The element that carried the declaration.
    #[serde(default)]
    pub selector: String,
    /// What the page said it is: the third party and its pinned version.
    #[serde(default)]
    pub declares: String,
}

/// A whole audit run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    /// The contract of this document.
    pub schema: String,
    /// The origin the audit drove.
    #[serde(default)]
    pub origin: String,
    /// Whether every discovered page was visited, or the run was narrowed for iteration.
    #[serde(default)]
    pub complete: bool,
    /// Where the pages and the widths came from.
    #[serde(default)]
    pub source: BTreeMap<String, String>,
    /// The breakpoints the compiled stylesheet declared.
    #[serde(default)]
    pub breakpoints: Vec<u32>,
    /// The widths the sweep visits.
    #[serde(default)]
    pub viewports: Vec<u32>,
    /// Every surface the run measured, and what each contributed.
    #[serde(default)]
    pub surfaces: Vec<Surface>,
    /// Every subtree the engine was told to skip, once per declaration.
    #[serde(default)]
    pub foreign: Vec<Foreign>,
    /// How many pages were visited.
    pub pages: usize,
    /// How many page-and-width visits that came to.
    pub visits: usize,
    /// How long the run took, in whole seconds.
    #[serde(default)]
    pub seconds: u64,
    /// Every finding, in the order the run produced them.
    pub findings: Vec<Finding>,
}

impl Run {
    /// Did every page pass?
    pub fn green(&self) -> bool {
        self.findings.is_empty()
    }

    /// The findings per rule, most frequent first, then by rule name.
    ///
    /// Derived rather than read from the document: a count and a list that can disagree is
    /// a report that will one day lie about its own evidence.
    ///
    /// ```
    /// use majordomus_cli::web::report::ui::parse;
    /// let run = parse(r#"{"schema":"ui-audit/v1","pages":1,"visits":1,"findings":[
    ///     {"route":"/","width":320,"rule":"a","detail":"x"},
    ///     {"route":"/","width":320,"rule":"a","detail":"y"},
    ///     {"route":"/","width":320,"rule":"b","detail":"z"}]}"#).unwrap();
    /// assert_eq!(run.by_rule(), vec![("a".to_string(), 2), ("b".to_string(), 1)]);
    /// ```
    pub fn by_rule(&self) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for finding in &self.findings {
            *counts.entry(finding.rule.as_str()).or_default() += 1;
        }
        let mut ordered: Vec<(String, usize)> = counts
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        ordered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ordered
    }

    /// The pages a rule was found on, deduplicated and ordered.
    pub fn routes_of(&self, rule: &str) -> Vec<&str> {
        let mut routes: Vec<&str> = self
            .findings
            .iter()
            .filter(|f| f.rule == rule)
            .map(|f| f.route.as_str())
            .collect();
        routes.sort_unstable();
        routes.dedup();
        routes
    }
}

/// Read a results document, refusing a contract this executable does not know.
pub fn parse(text: &str) -> Result<Run> {
    let run: Run = serde_json::from_str(text).map_err(|e| Error::InvalidSurface {
        surface: "ui".into(),
        reason: format!(
            "the results document does not parse: {e}; the contract is {RESULTS_SCHEMA}"
        ),
    })?;
    if run.schema != RESULTS_SCHEMA {
        return Err(Error::InvalidSurface {
            surface: "ui".into(),
            reason: format!(
                "the results document declares schema '{}', and this executable reads {RESULTS_SCHEMA}",
                run.schema
            ),
        });
    }
    Ok(run)
}

/// Read a results document from disk.
pub fn read(path: &Path) -> Result<Run> {
    let text =
        std::fs::read_to_string(path).map_err(|e| Error::io(path.display().to_string(), e))?;
    parse(&text)
}

/// Render the run into `/tests/ui`, inside the test surface.
///
/// Returns the directory written.
pub fn render(root: &Path, run: &Run) -> Result<PathBuf> {
    // the enclosing surface must exist for this section to be reachable, and belongs to the
    // test report: declared here only when nobody has declared it, never overwritten
    let surface = super::declare_if_absent(
        root,
        tests::SURFACE_ID,
        tests::SURFACE_TITLE,
        tests::SURFACE_PRODUCER,
    )?;
    let dir = surface.join(SECTION);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(dir.display().to_string(), e))?;
    let origin = Origin::read(root);
    super::write_json(&dir, "results.json", run)?;

    let mut summary = vec![
        ("pages", run.pages.to_string()),
        ("visits", run.visits.to_string()),
        ("findings", run.findings.len().to_string()),
        ("rules broken", run.by_rule().len().to_string()),
        ("seconds", run.seconds.to_string()),
    ];
    if !run.viewports.is_empty() {
        summary.push(("widths", run.viewports.len().to_string()));
    }

    let mut rule_rows: Vec<Vec<String>> = Vec::new();
    for (rule, count) in run.by_rule() {
        let routes = run.routes_of(&rule);
        let example = routes.first().copied().unwrap_or("");
        rule_rows.push(vec![
            format!("<span class=\"mono\">{}</span>", html::escape(&rule)),
            format!("<span class=\"num\">{count}</span>"),
            format!("<span class=\"num\">{}</span>", routes.len()),
            format!("<span class=\"mono\">{}</span>", html::escape(example)),
        ]);
    }

    let mut finding_rows: Vec<Vec<String>> = Vec::new();
    for finding in &run.findings {
        let elements = finding
            .elements
            .iter()
            .take(3)
            .map(|e| html::escape(&e.selector))
            .collect::<Vec<_>>()
            .join("<br>");
        finding_rows.push(vec![
            format!(
                "<span class=\"mono\">{}</span>",
                html::escape(&finding.route)
            ),
            format!("<span class=\"num\">{}</span>", finding.width),
            format!(
                "<span class=\"mono\">{}</span>",
                html::escape(&finding.rule)
            ),
            html::escape(&finding.detail),
            format!("<span class=\"mono\">{elements}</span>"),
        ]);
    }

    let verdict = if run.green() {
        "Every page the audit visited satisfies every invariant it checks."
    } else {
        "The audit found conformance failures; every one is named below, with the page, the \
         width and the element."
    };
    let scope = if run.complete {
        String::new()
    } else {
        "<p class=\"lede\"><strong>This run was narrowed</strong> — it visited part of the \
         site, not all of it, so it cannot stand for a clean audit.</p>"
            .to_string()
    };
    let provenance = format!(
        "<h2>Where the target set came from</h2><ul><li>pages: {}</li><li>widths: {}</li>\
         <li>breakpoints: <span class=\"mono\">{}</span></li>\
         <li>widths visited: <span class=\"mono\">{}</span></li></ul>\
         <p class=\"lede\">Nothing in this run names a page or a width. Add a page to the \
         site and it is audited; change a breakpoint in the theme and the widths follow.</p>",
        html::escape(
            run.source
                .get("pages")
                .map(String::as_str)
                .unwrap_or("the built site")
        ),
        html::escape(
            run.source
                .get("viewports")
                .map(String::as_str)
                .unwrap_or("the compiled stylesheet")
        ),
        run.breakpoints
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        run.viewports
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    );

    // Which surfaces were measured at all. This table is the answer to the question ADR 0036
    // left open — whether the audit reaches the surfaces the executable renders as well as
    // the ones it serves from a directory — so it is on the page whether the run is green or
    // not, and it says what a crawled surface was sampled *from*.
    let surfaces_section = if run.surfaces.is_empty() {
        String::new()
    } else {
        let rows: Vec<Vec<String>> = run
            .surfaces
            .iter()
            .map(|surface| {
                let native = surface.kind == "native-route";
                let found = if !native {
                    "its directory and its sitemap".to_string()
                } else if surface.routes == 0 {
                    "no pages: the mount is not a document".to_string()
                } else {
                    format!(
                        "{} route(s) in {} family(ies), crawled from its own anchors{}",
                        surface.routes,
                        surface.families,
                        if surface.truncated {
                            ", and the crawl hit its budget, so that is a floor"
                        } else {
                            ""
                        }
                    )
                };
                vec![
                    format!("<span class=\"mono\">{}</span>", html::escape(&surface.id)),
                    format!("<span class=\"mono\">{}</span>", html::escape(&surface.mount)),
                    format!("<span class=\"mono\">{}</span>", html::escape(&surface.kind)),
                    if native {
                        format!("<span class=\"num\">{}</span>", surface.sampled)
                    } else {
                        "every page".to_string()
                    },
                    html::escape(&found),
                ]
            })
            .collect();
        format!(
            "<h2>The surfaces this run measured</h2>{}",
            html::table(&["surface", "mount", "kind", "visited", "found by"], &rows)
        )
    };

    // What was not measured, and what declared it. On the page whether the run is green or
    // not: a reader who cannot see this cannot tell a clean audit from a narrowed one.
    let foreign_section = if run.foreign.is_empty() {
        String::new()
    } else {
        let rows: Vec<Vec<String>> = run
            .foreign
            .iter()
            .map(|subtree| {
                vec![
                    format!("<span class=\"mono\">{}</span>", html::escape(&subtree.declares)),
                    format!("<span class=\"mono\">{}</span>", html::escape(&subtree.selector)),
                    format!("<span class=\"mono\">{}</span>", html::escape(&subtree.route)),
                ]
            })
            .collect();
        format!(
            "<h2>Not measured, and why</h2><p class=\"lede\">A page may declare a subtree \
             that is a third party's to answer for. The accessibility engine skipped these; \
             everything around them was measured as usual.</p>{}",
            html::table(&["declared as", "element", "first seen on"], &rows)
        )
    };

    let findings_section = if run.green() {
        String::new()
    } else {
        format!(
            "<h2>By rule</h2>{}<h2>Every finding</h2>{}",
            html::table(&["rule", "findings", "pages", "first page"], &rule_rows),
            html::table(
                &["page", "width", "rule", "detail", "elements"],
                &finding_rows
            ),
        )
    };

    let body = format!(
        "{}{}{}{}{}{}<p><a href=\"../\">The test run this belongs to</a></p>{}",
        scope,
        html::summary(&summary),
        surfaces_section,
        foreign_section,
        findings_section,
        provenance,
        html::origin(&origin, &[("results.json", "results.json")])
    );
    let page = html::page("UI conformance", verdict, &body);
    super::write(&dir, "index.html", &page)?;
    Ok(dir)
}

#[cfg(test)]
mod unit {
    use super::*;

    const RUN: &str = r#"{
        "schema": "ui-audit/v1", "origin": "http://127.0.0.1:1", "complete": true,
        "source": {"pages": "the built site", "viewports": "the stylesheet"},
        "breakpoints": [640, 768], "viewports": [320, 639, 640, 1440],
        "pages": 2, "visits": 4, "seconds": 7,
        "findings": [
            {"route": "/", "width": 320, "tier": "sweep",
             "rule": "responsive.horizontal-overflow", "detail": "the document is 420px wide",
             "elements": [{"selector": "pre.code", "right": 420, "width": 400}]}
        ]
    }"#;

    #[test]
    fn a_document_from_another_contract_is_refused_rather_than_guessed() {
        let err = parse(r#"{"schema":"ui-audit/v2","pages":0,"visits":0,"findings":[]}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("ui-audit/v1"), "{err}");
    }

    #[test]
    fn what_a_rule_measured_survives_the_round_trip() {
        let run = parse(RUN).unwrap();
        let element = &run.findings[0].elements[0];
        assert_eq!(element.selector, "pre.code");
        assert_eq!(element.measured.get("right").unwrap().as_i64(), Some(420));
    }

    #[test]
    fn the_report_is_a_section_of_the_test_surface_and_declares_no_surface_of_its_own() {
        let tmp = tempfile::tempdir().unwrap();
        let run = parse(RUN).unwrap();
        let dir = render(tmp.path(), &run).unwrap();
        assert!(dir.ends_with("tests/ui"));
        assert!(dir.join("index.html").is_file());
        assert!(dir.join("results.json").is_file());
        assert!(
            !dir.join(crate::web::discover::DECLARATION_FILE).is_file(),
            "a section of another surface declares nothing of its own"
        );
        let surfaces = crate::web::discover::generated(tmp.path()).unwrap();
        assert_eq!(surfaces.len(), 1, "one surface, not two: {surfaces:?}");
        assert_eq!(surfaces[0].mount.as_str(), "/tests");
    }

    #[test]
    fn rendering_the_section_never_overwrites_the_enclosing_report() {
        let tmp = tempfile::tempdir().unwrap();
        let suite = tests::parse_cases("a\tok\t1\tparallel\n").unwrap();
        tests::render(tmp.path(), &suite).unwrap();
        let before = std::fs::read_to_string(
            super::super::directory(tmp.path(), "tests").join("surface.json"),
        )
        .unwrap();
        render(tmp.path(), &parse(RUN).unwrap()).unwrap();
        let after = std::fs::read_to_string(
            super::super::directory(tmp.path(), "tests").join("surface.json"),
        )
        .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn a_narrowed_run_says_so_on_the_page() {
        let tmp = tempfile::tempdir().unwrap();
        let mut run = parse(RUN).unwrap();
        run.complete = false;
        let dir = render(tmp.path(), &run).unwrap();
        let page = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(page.contains("narrowed"), "{page}");
    }

    #[test]
    fn a_clean_run_says_so_and_lists_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let mut run = parse(RUN).unwrap();
        run.findings.clear();
        let dir = render(tmp.path(), &run).unwrap();
        let page = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(page.contains("satisfies every invariant"), "{page}");
        assert!(!page.contains("Every finding"), "{page}");
    }
}
