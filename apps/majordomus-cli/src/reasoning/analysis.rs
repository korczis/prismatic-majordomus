//! `reasoning.analysis`: the structural check of an evidence-tagged multi-pass analysis.
//!
//! An analysis answers a hard question in passes. The skill `evidence-tagged-analysis`
//! holds the protocol; this module holds the check its documents are held to. The
//! documents of one analysis are its **bundle**: one directory whose Markdown files each
//! declare their role in front matter, or one file checked as a bundle of one.
//!
//! * `question`: the questions and the rules, written before anything is read.
//! * `lens`: pass 1, one independent lens. Its claims are rows of a findings table
//!   (`ID | Claim | Tag | Evidence`), each tagged OBSERVED (with a locator), DERIVED (with a
//!   formula) or INFERRED (with what would confirm it).
//! * `cross-check`: pass 2, a refutation of other lenses. One verdict row per finding
//!   (`ID | Verdict | Tag | Evidence`): CONFIRMED, REFUTED or UNVERIFIABLE, itself tagged.
//! * `synthesis`: pass 3. Root causes (`RC | Cause | Findings`) and remedies
//!   (`ID | Remedy | Addresses | Owner`) cite only CONFIRMED findings; a refuted register
//!   (`ID | Verdict | Correction`) names what must not be reused; the gaps ledger names
//!   every UNVERIFIABLE finding.
//!
//! What is proven is **structure**, and the answer says so in [`PROMISE`]: that a locator
//! supports its claim, or that a formula compares like with like, is the cross-check's work.
//! A check whose inputs are absent is `not_run` with the reason, never passed, and a bundle
//! with any check not run is `incomplete`, not `ok`.
//!
//! ```
//! use majordomus_cli::reasoning::analysis::{check, AnalysisConformance, Document};
//!
//! let lens = Document::new("lens-cache.md", "---\nanalysis:\n  id: misses\n  pass: lens\n  lens: cache\n---\n\
//! | ID | Claim | Tag | Evidence |\n|---|---|---|---|\n\
//! | cache-F1 | The cache misses on every restart | OBSERVED | `src/cache.rs:40` |\n\n\
//! ## Gaps ledger\n\n- none\n");
//! let report = check(&[lens]);
//! assert_eq!(report.state, AnalysisConformance::Incomplete, "no cross-check, no synthesis: incomplete");
//! assert!(!report.ok);
//! assert!(report.findings.is_empty(), "what was checked conforms");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::metadata::frontmatter;

/// What a conforming answer proves, stated in every answer so that nobody reads more into it.
pub const PROMISE: &str = "structural conformance: every claim row carries a typed, sourced evidence tag, every finding has one cross-check verdict, the synthesis cites only confirmed findings and the gaps are ledgered; whether a locator supports its claim is the cross-check's work, not this check's";

/// The checks, in the order they run and report.
pub const CHECKS: [&str; 7] = [
    "bundle",
    "question",
    "findings",
    "verdicts",
    "synthesis",
    "citations",
    "gaps",
];

/// One Markdown document of a bundle: its repository-relative path and its text.
///
/// ```
/// use majordomus_cli::reasoning::analysis::Document;
/// let d = Document::new("a/question.md", "# Q\n");
/// assert_eq!((d.path.as_str(), d.text.as_str()), ("a/question.md", "# Q\n"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Where it was read from, as reported.
    pub path: String,
    /// Its whole text, front matter included.
    pub text: String,
}

impl Document {
    /// A document from its path and text.
    ///
    /// ```
    /// use majordomus_cli::reasoning::analysis::Document;
    /// assert_eq!(Document::new("x.md", "t").path, "x.md");
    /// ```
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> Self {
        Document {
            path: path.into(),
            text: text.into(),
        }
    }
}

/// How one check ended: it ran and found nothing, it ran and found something, or it could
/// not run because what it compares is absent.
///
/// ```
/// use majordomus_cli::reasoning::analysis::AnalysisCheckState;
/// assert_eq!(serde_json::to_value(AnalysisCheckState::NotRun).unwrap(), "not_run");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisCheckState {
    /// It ran and found nothing.
    Passed,
    /// It ran and found at least one defect.
    Failed,
    /// Its inputs are absent; the reason says which.
    NotRun,
}

/// The verdict over a whole bundle.
///
/// ```
/// use majordomus_cli::reasoning::analysis::AnalysisConformance;
/// assert_eq!(serde_json::to_value(AnalysisConformance::Incomplete).unwrap(), "incomplete");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisConformance {
    /// Every check ran and passed.
    Conforms,
    /// At least one check failed.
    Fails,
    /// Nothing failed, and at least one check did not run.
    Incomplete,
}

/// One check and how it ended.
///
/// ```
/// use majordomus_cli::reasoning::analysis::{check, AnalysisCheck, AnalysisCheckState, Document};
/// let r = check(&[Document::new("s.md", "# not an analysis\n")]);
/// let bundle: &AnalysisCheck = r.checks.iter().find(|c| c.check == "bundle").unwrap();
/// assert_eq!(bundle.state, AnalysisCheckState::Failed);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisCheck {
    /// One of [`CHECKS`].
    pub check: String,
    /// How it ended.
    pub state: AnalysisCheckState,
    /// Why it did not run, when it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One defect: the check that found it, where, the id it concerns and what is wrong.
///
/// ```
/// use majordomus_cli::reasoning::analysis::{check, AnalysisFinding, Document};
/// let r = check(&[Document::new("s.md", "# not an analysis\n")]);
/// let f: &AnalysisFinding = &r.findings[0];
/// assert_eq!((f.check.as_str(), f.file.as_str()), ("bundle", "s.md"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisFinding {
    /// The check that found it.
    pub check: String,
    /// The document, as given.
    pub file: String,
    /// The 1-based line, when the defect has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The finding, root cause or remedy id it concerns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// One sentence saying what the grammar refuses.
    pub message: String,
}

/// One document and the role it declares.
///
/// ```
/// use majordomus_cli::reasoning::analysis::{check, AnalysisDocument, Document};
/// let r = check(&[Document::new("q.md", "---\nanalysis:\n  id: a\n  pass: question\n---\n# Q1 why?\n")]);
/// let d: &AnalysisDocument = &r.documents[0];
/// assert_eq!(d.pass.as_deref(), Some("question"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisDocument {
    /// The document, as given.
    pub file: String,
    /// `question`, `lens`, `cross-check` or `synthesis`; absent when it declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass: Option<String>,
    /// The lens it reads through, for a lens or a cross-check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lens: Option<String>,
}

/// The answer over a bundle.
///
/// `ok` is true exactly when `state` is `conforms`: an incomplete bundle is not ok.
///
/// ```
/// use majordomus_cli::reasoning::analysis::{check, AnalysisReport, AnalysisConformance, PROMISE};
/// let r: AnalysisReport = check(&[]);
/// assert_eq!(r.state, AnalysisConformance::Fails, "no document is a failed bundle");
/// assert_eq!(r.promise, PROMISE);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisReport {
    /// Every check ran and passed.
    pub ok: bool,
    /// conforms, fails or incomplete.
    pub state: AnalysisConformance,
    /// What a conforming answer proves, and what it does not.
    pub promise: String,
    /// The analysis id the documents declare, when they agree on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<String>,
    /// Each document and its role.
    pub documents: Vec<AnalysisDocument>,
    /// Paths beside the documents that are not part of the bundle: subdirectories and files
    /// that are not Markdown. Reported, never read.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<String>,
    /// Every check in [`CHECKS`] order.
    pub checks: Vec<AnalysisCheck>,
    /// Every defect.
    pub findings: Vec<AnalysisFinding>,
    /// Counts: findings defined, and verdicts by kind.
    pub counts: BTreeMap<String, usize>,
}

// ---------------------------------------------------------------- the grammar

fn finding_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\b[A-Za-z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)*-F[0-9]+\b").expect("finding id")
    })
}

fn locator_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(concat!(
            r"[\w./-]*[./][\w./-]*:\d+", // path:line
            r"|https?://\S+",            // a URL
            r"|`[^`]{2,}`",              // a command, a query, a log line
            r"|\b(?:[0-9a-f]*[0-9][0-9a-f]*[a-f][0-9a-f]*|[0-9a-f]*[a-f][0-9a-f]*[0-9][0-9a-f]*)\b"
        ))
        .expect("locator")
    })
}

fn formula_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"[=+*/×÷−%<>]|\d\s*-\s*\d|\b(?:ln|log|sum|mean|median|max|min|count)\b")
            .expect("formula")
    })
}

/// A commit hash is 7 to 40 hex digits; the locator pattern finds hex runs, this bounds them.
fn is_locator(evidence: &str) -> bool {
    locator_re().find_iter(evidence).any(|m| {
        let s = m.as_str();
        let hex = s.chars().all(|c| c.is_ascii_hexdigit());
        !hex || (7..=40).contains(&s.len())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Question,
    Lens,
    CrossCheck,
    Synthesis,
}

impl Pass {
    fn parse(word: &str) -> Option<Pass> {
        match word {
            "question" => Some(Pass::Question),
            "lens" => Some(Pass::Lens),
            "cross-check" => Some(Pass::CrossCheck),
            "synthesis" => Some(Pass::Synthesis),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    Confirmed,
    Refuted,
    Unverifiable,
}

impl Verdict {
    fn parse(cell: &str) -> Option<Verdict> {
        let word = cell.split(|c: char| !c.is_ascii_alphabetic()).next()?;
        match word.to_ascii_uppercase().as_str() {
            "CONFIRMED" => Some(Verdict::Confirmed),
            "REFUTED" => Some(Verdict::Refuted),
            "UNVERIFIABLE" => Some(Verdict::Unverifiable),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Verdict::Confirmed => "CONFIRMED",
            Verdict::Refuted => "REFUTED",
            Verdict::Unverifiable => "UNVERIFIABLE",
        }
    }
}

/// A table row: its line and its cells by lower-cased column name.
#[derive(Debug)]
struct Row {
    line: usize,
    cells: BTreeMap<String, String>,
}

impl Row {
    fn get(&self, column: &str) -> &str {
        self.cells.get(column).map(String::as_str).unwrap_or("")
    }
}

#[derive(Debug, Default)]
struct Table {
    columns: BTreeSet<String>,
    rows: Vec<Row>,
}

impl Table {
    fn has(&self, columns: &[&str]) -> bool {
        columns.iter().all(|c| self.columns.contains(*c))
    }
}

/// A parsed document: its role, its tables, its lines and its gaps ledger.
#[derive(Debug)]
struct Parsed {
    file: String,
    analysis: Option<String>,
    pass: Option<Pass>,
    lens: Option<String>,
    /// (1-based line, text) of the body.
    lines: Vec<(usize, String)>,
    tables: Vec<Table>,
    /// The lines of the gaps ledger section, when there is one, and whether it has an item.
    gaps: Option<(BTreeSet<usize>, bool)>,
}

/// Cells of a `|`-delimited row: a `|` inside a backtick span or escaped is text.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim();
    let inner = inner.strip_prefix('|').unwrap_or(inner);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut code = false;
    let mut escaped = false;
    for c in inner.chars() {
        if escaped {
            cur.push(c);
            escaped = false;
        } else if c == '\\' {
            cur.push(c);
            escaped = true;
        } else if c == '`' {
            cur.push(c);
            code = !code;
        } else if c == '|' && !code {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// A cell without the emphasis around it: `**Q1-F3**` reads `Q1-F3`.
fn plain(cell: &str) -> String {
    cell.trim()
        .trim_matches(|c| c == '*' || c == '_' || c == '`')
        .trim()
        .to_string()
}

fn is_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let t = line.trim_start();
    let level = t.chars().take_while(|c| *c == '#').count();
    (level > 0 && t[level..].starts_with(' ')).then(|| (level, t[level..].trim()))
}

fn is_item(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("- ")
        || t.starts_with("* ")
        || t.split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

fn parse(doc: &Document, out: &mut Vec<AnalysisFinding>) -> Parsed {
    let mut parsed = Parsed {
        file: doc.path.clone(),
        analysis: None,
        pass: None,
        lens: None,
        lines: Vec::new(),
        tables: Vec::new(),
        gaps: None,
    };
    let mut bundle = |message: String| {
        out.push(AnalysisFinding {
            check: "bundle".into(),
            file: doc.path.clone(),
            line: Some(1),
            id: None,
            message,
        })
    };
    let (front, body) = match frontmatter::split(&doc.text) {
        Ok(s) => (s.front, s.body),
        Err(e) => {
            bundle(format!("its front matter is unreadable: {e}"));
            (None, doc.text.as_str())
        }
    };
    let offset = front.map_or(0, |f| f.lines().count() + 2);
    match front.map(frontmatter::parse) {
        None => bundle(
            "it declares no `analysis:` front matter, so it is not part of the bundle".into(),
        ),
        Some(Err(e)) => bundle(format!("its front matter is not YAML the layer reads: {e}")),
        Some(Ok(map)) => match map.get("analysis") {
            Some(Value::Object(a)) => {
                let text = |k: &str| a.get(k).and_then(crate::metadata::yaml::scalar_string);
                parsed.analysis = text("id").filter(|s| !s.is_empty());
                if parsed.analysis.is_none() {
                    bundle("`analysis.id` is missing".into());
                }
                parsed.lens = text("lens").filter(|s| !s.is_empty());
                match text("pass") {
                    None => bundle("`analysis.pass` is missing".into()),
                    Some(p) => match Pass::parse(&p) {
                        None => bundle(format!(
                            "`analysis.pass` is '{p}', not one of question, lens, cross-check, synthesis"
                        )),
                        Some(pass) => {
                            if matches!(pass, Pass::Lens | Pass::CrossCheck) && parsed.lens.is_none() {
                                bundle(format!("a {p} document names its `analysis.lens`"));
                            }
                            parsed.pass = Some(pass);
                        }
                    },
                }
            }
            _ => bundle("it declares no `analysis:` mapping in its front matter".into()),
        },
    }
    // the body: lines, tables, the gaps ledger
    let mut table: Option<(Vec<String>, Table)> = None;
    let mut gaps: Option<(usize, BTreeSet<usize>, bool)> = None;
    let mut ledgers: Vec<(BTreeSet<usize>, bool)> = Vec::new();
    for (i, text) in body.lines().enumerate() {
        let line = offset + i + 1;
        parsed.lines.push((line, text.to_string()));
        if let Some((level, title)) = heading(text) {
            if let Some((l, ls, item)) = gaps.take() {
                if level > l {
                    gaps = Some((l, ls, item));
                } else {
                    ledgers.push((ls, item));
                }
            }
            if gaps.is_none() && title.to_ascii_lowercase().contains("gaps ledger") {
                gaps = Some((level, BTreeSet::new(), false));
            }
        }
        if let Some((_, ls, item)) = gaps.as_mut() {
            ls.insert(line);
            *item |= is_item(text) || (text.trim_start().starts_with('|') && !is_separator(text));
        }
        if text.trim_start().starts_with('|') {
            if let Some((header, t)) = table.as_mut() {
                if !is_separator(text) {
                    let row = header.iter().cloned().zip(cells(text)).collect();
                    t.rows.push(Row { line, cells: row });
                }
            } else {
                let header: Vec<String> = cells(text)
                    .iter()
                    .map(|c| plain(c).to_ascii_lowercase())
                    .collect();
                let columns = header.iter().cloned().collect();
                table = Some((
                    header,
                    Table {
                        columns,
                        rows: Vec::new(),
                    },
                ));
            }
        } else if let Some((_, t)) = table.take() {
            parsed.tables.push(t);
        }
    }
    if let Some((_, t)) = table {
        parsed.tables.push(t);
    }
    if let Some((_, ls, item)) = gaps {
        ledgers.push((ls, item));
    }
    // a gaps ledger with an item wins over an empty one
    parsed.gaps = ledgers.into_iter().max_by_key(|(_, item)| *item);
    parsed
}

// ---------------------------------------------------------------- the checks

/// A finding defined in pass 1: where, and its tag.
#[derive(Debug)]
struct Defined {
    file: String,
    line: usize,
}

struct Run {
    findings: Vec<AnalysisFinding>,
}

impl Run {
    /// Record a defect; an empty `id` is a defect about no particular row.
    fn fail(&mut self, check: &str, file: &str, line: Option<usize>, id: &str, message: String) {
        self.findings.push(AnalysisFinding {
            check: check.into(),
            file: file.into(),
            line,
            id: (!id.is_empty()).then(|| id.to_string()),
            message,
        });
    }
}

/// The tag of a row and whether its evidence fits it; the defect when it does not.
fn tag_defect(tag: &str, evidence: &str) -> Option<String> {
    let evidence = evidence.trim();
    match tag.to_ascii_uppercase().as_str() {
        "OBSERVED" if !is_locator(evidence) => Some(
            "is OBSERVED and its evidence holds no locator (a path:line, a URL, a commit hash or a backtick span)".into(),
        ),
        "DERIVED" if !formula_re().is_match(evidence) && !finding_id_re().is_match(evidence) => Some(
            "is DERIVED and its evidence holds no formula (an operator, or the findings it is computed from)".into(),
        ),
        "INFERRED" if evidence.is_empty() => {
            Some("is INFERRED and its evidence does not say what would confirm it".into())
        }
        "OBSERVED" | "DERIVED" | "INFERRED" => None,
        "" => Some("carries no tag: OBSERVED, DERIVED or INFERRED".into()),
        other => Some(format!("carries the tag '{other}', not OBSERVED, DERIVED or INFERRED")),
    }
}

/// The documents of one role.
fn of(parsed: &[Parsed], pass: Pass) -> impl Iterator<Item = &Parsed> {
    parsed.iter().filter(move |p| p.pass == Some(pass))
}

/// Check a bundle: every document of one analysis. Pure over the texts given; see the
/// module documentation for the grammar and [`CHECKS`] for what is checked.
///
/// ```
/// use majordomus_cli::reasoning::analysis::{check, AnalysisConformance, Document};
///
/// let lens = Document::new("l.md", "---\nanalysis:\n  id: a\n  pass: lens\n  lens: x\n---\n\
/// | ID | Claim | Tag | Evidence |\n|---|---|---|---|\n| x-F1 | It is slow | DERIVED | 9 s / 3 calls = 3 s |\n\n## Gaps ledger\n- none\n");
/// let refuting = Document::new("c.md", "---\nanalysis:\n  id: a\n  pass: cross-check\n  lens: y\n---\n\
/// | ID | Verdict | Tag | Evidence |\n|---|---|---|---|\n| x-F1 | REFUTED | OBSERVED | `time run` says 1 s |\n\n## Gaps ledger\n- none\n");
/// let synthesis = Document::new("s.md", "---\nanalysis:\n  id: a\n  pass: synthesis\n---\n\
/// It is slow because of x-F1.\n");
/// let r = check(&[lens, refuting, synthesis]);
/// assert_eq!(r.state, AnalysisConformance::Fails);
/// assert!(r.findings.iter().any(|f| f.check == "citations" && f.id.as_deref() == Some("x-F1")));
/// ```
pub fn check(documents: &[Document]) -> AnalysisReport {
    let mut run = Run {
        findings: Vec::new(),
    };
    let parsed: Vec<Parsed> = documents
        .iter()
        .map(|d| parse(d, &mut run.findings))
        .collect();
    let mut states: BTreeMap<&str, (AnalysisCheckState, Option<String>)> = BTreeMap::new();
    let not_run = |why: &str| (AnalysisCheckState::NotRun, Some(why.to_string()));

    // bundle: roles, one id, at most one question and one synthesis
    if documents.is_empty() {
        run.fail(
            "bundle",
            "",
            None,
            "",
            "the bundle holds no Markdown document".into(),
        );
    }
    let ids: BTreeSet<&str> = parsed
        .iter()
        .filter_map(|p| p.analysis.as_deref())
        .collect();
    let analysis = (ids.len() == 1).then(|| ids.iter().next().expect("one").to_string());
    if ids.len() > 1 {
        for p in &parsed {
            run.fail(
                "bundle",
                &p.file,
                Some(1),
                "",
                format!(
                    "the bundle declares {} analysis ids ({}); one bundle is one analysis",
                    ids.len(),
                    ids.iter().cloned().collect::<Vec<_>>().join(", ")
                ),
            );
        }
    }
    for (pass, word) in [(Pass::Question, "question"), (Pass::Synthesis, "synthesis")] {
        if of(&parsed, pass).count() > 1 {
            for p in of(&parsed, pass) {
                run.fail(
                    "bundle",
                    &p.file,
                    Some(1),
                    "",
                    format!("the bundle holds more than one {word} document"),
                );
            }
        }
    }

    // question: written first, and not empty
    match of(&parsed, Pass::Question).next() {
        None => {
            states.insert("question", not_run("no question document: the questions and the rules are written before any lens reads"));
        }
        Some(q) => {
            if q.lines.iter().all(|(_, t)| t.trim().is_empty()) {
                run.fail(
                    "question",
                    &q.file,
                    None,
                    "",
                    "the question document is empty".into(),
                );
            }
        }
    }

    // findings: every lens row is typed and sourced
    let mut defined: BTreeMap<String, Defined> = BTreeMap::new();
    if of(&parsed, Pass::Lens).next().is_none() {
        states.insert("findings", not_run("no lens document"));
    }
    for p in of(&parsed, Pass::Lens) {
        let rows: Vec<&Row> = p
            .tables
            .iter()
            .filter(|t| t.has(&["id", "claim", "tag", "evidence"]))
            .flat_map(|t| &t.rows)
            .collect();
        if rows.is_empty() {
            run.fail(
                "findings",
                &p.file,
                None,
                "",
                "a lens document holds no findings table (ID | Claim | Tag | Evidence)".into(),
            );
        }
        for row in rows {
            let id = plain(row.get("id"));
            let whole = finding_id_re().find(&id).is_some_and(|m| m.as_str() == id);
            if !whole {
                run.fail(
                    "findings",
                    &p.file,
                    Some(row.line),
                    &id,
                    format!("'{id}' is not a finding id of the form <lens>-F<n>"),
                );
                continue;
            }
            if let Some(first) = defined.get(&id) {
                run.fail(
                    "findings",
                    &p.file,
                    Some(row.line),
                    &id,
                    format!(
                        "{id} is defined again; it was defined at {}:{}",
                        first.file, first.line
                    ),
                );
                continue;
            }
            if plain(row.get("claim")).is_empty() {
                run.fail(
                    "findings",
                    &p.file,
                    Some(row.line),
                    &id,
                    format!("{id} states no claim"),
                );
            }
            if let Some(defect) = tag_defect(&plain(row.get("tag")), row.get("evidence")) {
                run.fail(
                    "findings",
                    &p.file,
                    Some(row.line),
                    &id,
                    format!("{id} {defect}"),
                );
            }
            defined.insert(
                id,
                Defined {
                    file: p.file.clone(),
                    line: row.line,
                },
            );
        }
    }

    // verdicts: one per finding, each tagged
    let mut verdicts: BTreeMap<String, Vec<Verdict>> = BTreeMap::new();
    let have_verdicts = of(&parsed, Pass::CrossCheck).next().is_some();
    if !have_verdicts {
        states.insert(
            "verdicts",
            not_run("no cross-check document: no finding was put to refutation"),
        );
    } else if defined.is_empty() {
        states.insert("verdicts", not_run("no lens finding to give a verdict on"));
    }
    for p in of(&parsed, Pass::CrossCheck) {
        let rows: Vec<&Row> = p
            .tables
            .iter()
            .filter(|t| t.has(&["id", "verdict", "evidence"]))
            .flat_map(|t| &t.rows)
            .collect();
        if rows.is_empty() {
            run.fail(
                "verdicts",
                &p.file,
                None,
                "",
                "a cross-check document holds no verdict table (ID | Verdict | Tag | Evidence)"
                    .into(),
            );
        }
        for row in rows {
            let id = plain(row.get("id"));
            let Some(verdict) = Verdict::parse(&plain(row.get("verdict"))) else {
                run.fail(
                    "verdicts",
                    &p.file,
                    Some(row.line),
                    &id,
                    format!(
                        "the verdict on {id} is '{}', not CONFIRMED, REFUTED or UNVERIFIABLE",
                        plain(row.get("verdict"))
                    ),
                );
                continue;
            };
            if !defined.is_empty() && !defined.contains_key(&id) {
                run.fail(
                    "verdicts",
                    &p.file,
                    Some(row.line),
                    &id,
                    format!("{id} has a verdict and no lens defines it"),
                );
            }
            if let Some(defect) = tag_defect(&plain(row.get("tag")), row.get("evidence")) {
                run.fail(
                    "verdicts",
                    &p.file,
                    Some(row.line),
                    &id,
                    format!("the {} verdict on {id} {defect}", verdict.word()),
                );
            }
            verdicts.entry(id).or_default().push(verdict);
        }
    }
    if have_verdicts {
        for (id, d) in &defined {
            match verdicts.get(id).map(Vec::len).unwrap_or(0) {
                0 => run.fail("verdicts", &d.file, Some(d.line), id,
                    format!("{id} was never put to refutation: no cross-check gives it a verdict")),
                1 => {}
                n => run.fail("verdicts", &d.file, Some(d.line), id, format!(
                    "{id} has {n} verdict rows; one must stand, and a disagreement is settled by an experiment, never a count"
                )),
            }
        }
    }
    let standing: BTreeMap<&str, Verdict> = verdicts
        .iter()
        .filter(|(_, v)| v.len() == 1)
        .map(|(id, v)| (id.as_str(), v[0]))
        .collect();

    // synthesis: root causes from findings, remedies with owners
    let synthesis = of(&parsed, Pass::Synthesis).next();
    match synthesis {
        None => {
            states.insert("synthesis", not_run("no synthesis document"));
        }
        Some(s) => {
            let causes: Vec<&Row> = s
                .tables
                .iter()
                .filter(|t| t.has(&["rc", "cause", "findings"]))
                .flat_map(|t| &t.rows)
                .collect();
            let remedies: Vec<&Row> = s
                .tables
                .iter()
                .filter(|t| t.has(&["id", "remedy", "addresses", "owner"]))
                .flat_map(|t| &t.rows)
                .collect();
            if causes.is_empty() {
                run.fail(
                    "synthesis",
                    &s.file,
                    None,
                    "",
                    "the synthesis holds no root-cause table (RC | Cause | Findings)".into(),
                );
            }
            if remedies.is_empty() {
                run.fail(
                    "synthesis",
                    &s.file,
                    None,
                    "",
                    "the synthesis holds no remedy table (ID | Remedy | Addresses | Owner)".into(),
                );
            }
            let rcs: BTreeSet<String> = causes.iter().map(|r| plain(r.get("rc"))).collect();
            for row in &causes {
                let rc = plain(row.get("rc"));
                if !finding_id_re().is_match(row.get("findings")) {
                    run.fail(
                        "synthesis",
                        &s.file,
                        Some(row.line),
                        &rc,
                        format!(
                            "root cause {rc} cites no finding; a root cause is a chain of findings"
                        ),
                    );
                }
            }
            for row in &remedies {
                let id = plain(row.get("id"));
                let addresses = row.get("addresses");
                let named = finding_id_re().is_match(addresses)
                    || rcs
                        .iter()
                        .any(|rc| !rc.is_empty() && addresses.contains(rc.as_str()));
                if !named {
                    run.fail(
                        "synthesis",
                        &s.file,
                        Some(row.line),
                        &id,
                        format!("remedy {id} addresses no root cause and no finding"),
                    );
                }
                if plain(row.get("owner")).is_empty() {
                    run.fail(
                        "synthesis",
                        &s.file,
                        Some(row.line),
                        &id,
                        format!("remedy {id} names no owner (write `unowned` for a hole)"),
                    );
                }
            }
        }
    }

    // citations: the synthesis cites only what survived
    match synthesis {
        None => {
            states.insert("citations", not_run("no synthesis document"));
        }
        Some(_) if !have_verdicts => {
            states.insert(
                "citations",
                not_run("no cross-check document: what survived refutation is unknown"),
            );
        }
        Some(s) => {
            let mut register: BTreeSet<usize> = BTreeSet::new();
            for row in s
                .tables
                .iter()
                .filter(|t| t.has(&["id", "verdict", "correction"]))
                .flat_map(|t| &t.rows)
            {
                register.insert(row.line);
                let id = plain(row.get("id"));
                let stated = Verdict::parse(&plain(row.get("verdict")));
                match (stated, standing.get(id.as_str())) {
                    (_, None) => {} // reported below as an unknown or unsettled citation
                    (Some(Verdict::Confirmed), _) => run.fail("citations", &s.file, Some(row.line), &id,
                        format!("{id} is in the refuted register as CONFIRMED; the register holds what did not survive")),
                    (Some(st), Some(rec)) if st == *rec => {}
                    (_, Some(rec)) => run.fail("citations", &s.file, Some(row.line), &id, format!(
                        "the register states {} for {id}, and its cross-check verdict is {}",
                        stated.map_or("no verdict", Verdict::word), rec.word()
                    )),
                }
            }
            let gaps = s
                .gaps
                .as_ref()
                .map(|(ls, _)| ls.clone())
                .unwrap_or_default();
            let mut in_gaps: BTreeSet<String> = BTreeSet::new();
            for (line, text) in &s.lines {
                for m in finding_id_re().find_iter(text) {
                    let id = m.as_str();
                    if gaps.contains(line) {
                        in_gaps.insert(id.to_string());
                    }
                    let fail = |run: &mut Run, message: String| {
                        run.fail("citations", &s.file, Some(*line), id, message)
                    };
                    match standing.get(id) {
                        None if verdicts.contains_key(id) => fail(&mut run, format!("{id} is cited and its verdict is not settled")),
                        None => fail(&mut run, format!("{id} is cited and no cross-check gives it a verdict")),
                        Some(Verdict::Confirmed) => {}
                        Some(_) if register.contains(line) => {}
                        Some(Verdict::Unverifiable) if gaps.contains(line) => {}
                        Some(v) => fail(&mut run, format!(
                            "{id} is {} and is cited outside the refuted register{}; the synthesis builds only on CONFIRMED findings",
                            v.word(),
                            if *v == Verdict::Unverifiable { " and the gaps ledger" } else { "" }
                        )),
                    }
                }
            }
            for (id, v) in &standing {
                if *v == Verdict::Unverifiable && !in_gaps.contains(*id) {
                    run.fail(
                        "citations",
                        &s.file,
                        None,
                        id,
                        format!(
                            "{id} is UNVERIFIABLE and the synthesis's gaps ledger does not name it"
                        ),
                    );
                }
            }
        }
    }

    // gaps: every lens, cross-check and synthesis ledgers what it could not verify
    let ledgered: Vec<&Parsed> = parsed
        .iter()
        .filter(|p| {
            matches!(
                p.pass,
                Some(Pass::Lens | Pass::CrossCheck | Pass::Synthesis)
            )
        })
        .collect();
    if ledgered.is_empty() {
        states.insert(
            "gaps",
            not_run("no lens, cross-check or synthesis document"),
        );
    }
    for p in ledgered {
        match &p.gaps {
            None => run.fail(
                "gaps",
                &p.file,
                None,
                "",
                "it has no gaps ledger (a heading naming the Gaps ledger)".into(),
            ),
            Some((_, false)) => run.fail(
                "gaps",
                &p.file,
                None,
                "",
                "its gaps ledger holds no item; `- none` is the empty ledger".into(),
            ),
            Some(_) => {}
        }
    }

    // assemble
    let checks: Vec<AnalysisCheck> = CHECKS
        .iter()
        .map(|c| {
            let (state, reason) = match states.remove(c) {
                Some(s) => s,
                None if run.findings.iter().any(|f| f.check == *c) => {
                    (AnalysisCheckState::Failed, None)
                }
                None => (AnalysisCheckState::Passed, None),
            };
            // a check that could not run yet found a defect has failed: the defect is real
            let state = if state == AnalysisCheckState::NotRun
                && run.findings.iter().any(|f| f.check == *c)
            {
                AnalysisCheckState::Failed
            } else {
                state
            };
            AnalysisCheck {
                check: c.to_string(),
                state,
                reason,
            }
        })
        .collect();
    let state = if checks.iter().any(|c| c.state == AnalysisCheckState::Failed) {
        AnalysisConformance::Fails
    } else if checks.iter().any(|c| c.state == AnalysisCheckState::NotRun) {
        AnalysisConformance::Incomplete
    } else {
        AnalysisConformance::Conforms
    };
    let mut counts = BTreeMap::new();
    counts.insert("findings".to_string(), defined.len());
    for v in [Verdict::Confirmed, Verdict::Refuted, Verdict::Unverifiable] {
        counts.insert(
            v.word().to_ascii_lowercase(),
            standing.values().filter(|s| **s == v).count(),
        );
    }
    AnalysisReport {
        ok: state == AnalysisConformance::Conforms,
        state,
        promise: PROMISE.to_string(),
        analysis,
        documents: parsed
            .iter()
            .map(|p| AnalysisDocument {
                file: p.file.clone(),
                pass: p.pass.map(|x| {
                    match x {
                        Pass::Question => "question",
                        Pass::Lens => "lens",
                        Pass::CrossCheck => "cross-check",
                        Pass::Synthesis => "synthesis",
                    }
                    .to_string()
                }),
                lens: p.lens.clone(),
            })
            .collect(),
        ignored: Vec::new(),
        checks,
        findings: run.findings,
        counts,
    }
}

/// Read the bundle at `path` under `root`: a Markdown file, or every Markdown file directly
/// in a directory, in name order. What else the directory holds is returned as ignored. A
/// path outside the repository, or one that does not exist, is refused with the reason.
///
/// ```
/// use majordomus_cli::reasoning::analysis::read_bundle;
/// let root = tempfile::tempdir().unwrap();
/// std::fs::create_dir_all(root.path().join("a/notes")).unwrap();
/// std::fs::write(root.path().join("a/q.md"), "# Q\n").unwrap();
/// let (docs, ignored) = read_bundle(root.path(), "a").unwrap();
/// assert_eq!(docs[0].path, "a/q.md");
/// assert_eq!(ignored, ["a/notes"]);
/// assert!(read_bundle(root.path(), "../elsewhere").is_err());
/// ```
pub fn read_bundle(root: &Path, path: &str) -> Result<(Vec<Document>, Vec<String>), String> {
    let base = root
        .canonicalize()
        .map_err(|e| format!("the repository root is unreadable: {e}"))?;
    let target = base
        .join(path)
        .canonicalize()
        .map_err(|e| format!("{path}: {e}"))?;
    let rel = |p: &Path| {
        p.strip_prefix(&base)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };
    if !target.starts_with(&base) {
        return Err(format!(
            "{path} is outside the repository; an analysis is checked where it is tracked"
        ));
    }
    let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", rel(p)));
    if target.is_file() {
        return Ok((
            vec![Document::new(rel(&target), read(&target)?)],
            Vec::new(),
        ));
    }
    let mut names: Vec<String> = std::fs::read_dir(&target)
        .map_err(|e| format!("{path}: {e}"))?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    crate::order::canonical_strings(&mut names);
    let (mut docs, mut ignored) = (Vec::new(), Vec::new());
    for p in names.iter().map(|n| target.join(n)) {
        if p.is_file() && p.extension().is_some_and(|x| x == "md") {
            docs.push(Document::new(rel(&p), read(&p)?));
        } else {
            ignored.push(rel(&p));
        }
    }
    Ok((docs, ignored))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn front(pass: &str, lens: Option<&str>) -> String {
        let lens = lens.map(|l| format!("  lens: {l}\n")).unwrap_or_default();
        format!("---\nanalysis:\n  id: misses\n  pass: {pass}\n{lens}---\n")
    }

    fn question() -> Document {
        Document::new(
            "question.md",
            format!(
                "{}# Questions\n\n- Q1 why does the cache miss?\n\n# Rules\n\n- tag every claim\n",
                front("question", None)
            ),
        )
    }

    fn lens() -> Document {
        Document::new("lens-cache.md", format!(
            "{}# Lens: cache\n\n## Summary\n\nThe cache is cold.\n\n\
             | ID | Claim | Tag | Evidence |\n|---|---|---|---|\n\
             | **cache-F1** | Every restart empties it | OBSERVED | `src/cache.rs:40` |\n\
             | cache-F2 | Half of the reads miss | DERIVED | 50 misses / 100 reads |\n\
             | cache-F3 | The eviction is too eager | INFERRED | a run with eviction off would hit |\n\n\
             ## Gaps ledger\n\n- the production trace was not read\n",
            front("lens", Some("cache"))
        ))
    }

    fn cross(f3: &str) -> Document {
        Document::new("cross-check-load.md", format!(
            "{}# Cross-check\n\n| ID | Verdict | Tag | Evidence |\n|---|---|---|---|\n\
             | cache-F1 | CONFIRMED | OBSERVED | re-read src/cache.rs:40 |\n\
             | cache-F2 | REFUTED | DERIVED | 20 misses / 100 reads: the log counted retries twice |\n\
             | cache-F3 | {f3} |\n\n## Gaps ledger\n\n| What | Why |\n|---|---|\n| the trace | not exported |\n",
            front("cross-check", Some("load"))
        ))
    }

    fn synthesis(body: &str) -> Document {
        Document::new(
            "synthesis.md",
            format!("{}{body}", front("synthesis", None)),
        )
    }

    const GOOD: &str = "# Synthesis\n\n| RC | Cause | Findings |\n|---|---|---|\n| RC-a | restarts empty the cache | cache-F1 |\n\n\
        | ID | Remedy | Addresses | Owner | Verified by |\n|---|---|---|---|---|\n| R1 | warm on start | RC-a | the cache team | hit rate after restart |\n| R2 | measure eviction | cache-F1 | unowned | - |\n\n\
        ## Corrected claims\n\n| ID | Verdict | Correction |\n|---|---|---|\n| cache-F2 | REFUTED | 20%, not 50% |\n\n\
        ## Gaps ledger\n\n- cache-F3: eviction was not tested\n";

    fn bundle(f3: &str, body: &str) -> Vec<Document> {
        vec![question(), lens(), cross(f3), synthesis(body)]
    }

    fn defects(r: &AnalysisReport, check: &str) -> Vec<String> {
        r.findings
            .iter()
            .filter(|f| f.check == check)
            .map(|f| f.message.clone())
            .collect()
    }

    #[test]
    fn a_whole_bundle_conforms() {
        let r = check(&bundle("UNVERIFIABLE | INFERRED | needs the trace", GOOD));
        assert_eq!(r.findings, Vec::<AnalysisFinding>::new());
        assert_eq!(r.state, AnalysisConformance::Conforms);
        assert!(r.ok);
        assert_eq!(r.analysis.as_deref(), Some("misses"));
        assert_eq!(r.counts["findings"], 3);
        assert_eq!(
            (
                r.counts["confirmed"],
                r.counts["refuted"],
                r.counts["unverifiable"]
            ),
            (1, 1, 1)
        );
        assert!(r
            .checks
            .iter()
            .all(|c| c.state == AnalysisCheckState::Passed));
        assert_eq!(r.documents[2].lens.as_deref(), Some("load"));
        assert_eq!(r.documents[3].pass.as_deref(), Some("synthesis"));
    }

    #[test]
    fn an_unverifiable_finding_in_the_gaps_ledger_is_not_a_citation() {
        // the round-2 falsifier: the ledger must be able to name what could not be verified
        let r = check(&bundle("UNVERIFIABLE | INFERRED | needs the trace", GOOD));
        assert!(defects(&r, "citations").is_empty());
        let without = GOOD.replace("- cache-F3: eviction was not tested", "- the trace");
        let r = check(&bundle(
            "UNVERIFIABLE | INFERRED | needs the trace",
            &without,
        ));
        assert_eq!(
            defects(&r, "citations"),
            ["cache-F3 is UNVERIFIABLE and the synthesis's gaps ledger does not name it"]
        );
    }

    #[test]
    fn a_synthesis_built_on_an_unconfirmed_finding_fails() {
        let body = GOOD.replace(
            "| restarts empty the cache | cache-F1 |",
            "| restarts empty the cache | cache-F1, cache-F2, cache-F3 |",
        );
        let r = check(&bundle("UNVERIFIABLE | INFERRED | needs the trace", &body));
        let d = defects(&r, "citations");
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d[0].contains("cache-F2 is REFUTED and is cited outside the refuted register;"));
        assert!(d[1].contains("cache-F3 is UNVERIFIABLE and is cited outside the refuted register and the gaps ledger"));
        assert_eq!(r.state, AnalysisConformance::Fails);
        assert!(!r.ok);
    }

    #[test]
    fn the_register_states_the_recorded_verdict() {
        let confirmed = GOOD.replace(
            "| cache-F2 | REFUTED | 20%, not 50% |",
            "| cache-F1 | CONFIRMED | nothing |\n| cache-F2 | UNVERIFIABLE | x |",
        );
        let r = check(&bundle(
            "UNVERIFIABLE | INFERRED | needs the trace",
            &confirmed,
        ));
        let d = defects(&r, "citations");
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d[0].contains("cache-F1 is in the refuted register as CONFIRMED"));
        assert_eq!(
            d[1],
            "the register states UNVERIFIABLE for cache-F2, and its cross-check verdict is REFUTED"
        );
        let blank = GOOD.replace("| cache-F2 | REFUTED |", "| cache-F2 | maybe |");
        let r = check(&bundle("UNVERIFIABLE | INFERRED | needs the trace", &blank));
        assert_eq!(defects(&r, "citations"), ["the register states no verdict for cache-F2, and its cross-check verdict is REFUTED"]);
    }

    #[test]
    fn a_citation_without_a_standing_verdict_fails() {
        let body = GOOD.replace(
            "| cache-F2 | REFUTED | 20%, not 50% |",
            "| cache-F2 | REFUTED | 20%, not 50% |\n| other-F9 | REFUTED | x |",
        );
        let r = check(&bundle("UNVERIFIABLE | INFERRED | needs the trace", &body));
        assert_eq!(
            defects(&r, "citations"),
            ["other-F9 is cited and no cross-check gives it a verdict"]
        );
        // two verdicts on one finding: not settled, so citing it fails and so does the verdict check
        let mut docs = bundle(
            "CONFIRMED | OBSERVED | `grep evict` finds it",
            &GOOD.replace("- cache-F3: eviction was not tested", "- none"),
        );
        docs.push(Document::new("cross-check-2.md", format!(
            "{}| ID | Verdict | Tag | Evidence |\n|---|---|---|---|\n| cache-F1 | REFUTED | INFERRED | a cold start test |\n\n## Gaps ledger\n- none\n",
            front("cross-check", Some("second"))
        )));
        let r = check(&docs);
        assert!(defects(&r, "verdicts")[0].contains("cache-F1 has 2 verdict rows; one must stand"));
        assert!(defects(&r, "citations")
            .iter()
            .all(|m| m == "cache-F1 is cited and its verdict is not settled"));
        assert_eq!(defects(&r, "citations").len(), 2);
    }

    #[test]
    fn findings_are_typed_and_sourced() {
        let text = format!(
            "{}| ID | Claim | Tag | Evidence |\n|---|---|---|---|\n\
             | cache-F1 | a | OBSERVED | the code says so |\n\
             | cache-F2 | b | DERIVED | from the logs |\n\
             | cache-F3 | c | INFERRED |  |\n\
             | cache-F4 | d |  | x |\n\
             | cache-F5 | e | GUESSED | x |\n\
             | cache-F6 |  | OBSERVED | commit 1a2b3c4d |\n\
             | F7 | f | OBSERVED | https://example.invalid/x |\n\
             | cache-F1 | g | OBSERVED | a.rs:1 |\n\
             | cache-F8 | h | OBSERVED | abcdef1 |\n\
             | cache-F9 | i | OBSERVED | cafe1234cafe1234cafe1234cafe1234cafe1234cafe |\n\
             | cache-F10 | j | DERIVED | from cache-F8 |\n\n## Gaps ledger\n- none\n",
            front("lens", Some("cache"))
        );
        let r = check(&[Document::new("l.md", text)]);
        let d = defects(&r, "findings");
        assert_eq!(d, [
            "cache-F1 is OBSERVED and its evidence holds no locator (a path:line, a URL, a commit hash or a backtick span)",
            "cache-F2 is DERIVED and its evidence holds no formula (an operator, or the findings it is computed from)",
            "cache-F3 is INFERRED and its evidence does not say what would confirm it",
            "cache-F4 carries no tag: OBSERVED, DERIVED or INFERRED",
            "cache-F5 carries the tag 'GUESSED', not OBSERVED, DERIVED or INFERRED",
            "cache-F6 states no claim",
            "'F7' is not a finding id of the form <lens>-F<n>",
            "cache-F1 is defined again; it was defined at l.md:9",
            "cache-F9 is OBSERVED and its evidence holds no locator (a path:line, a URL, a commit hash or a backtick span)",
        ], "a 7-hex commit is a locator, a 44-hex run is not; a derivation may cite a finding");
        assert_eq!(r.findings[0].line, Some(9));
        assert_eq!(r.state, AnalysisConformance::Fails);
    }

    #[test]
    fn a_lens_without_a_findings_table_and_a_cross_check_without_verdicts_fail() {
        let l = Document::new(
            "l.md",
            format!(
                "{}Prose only.\n\n## Gaps ledger\n- none\n",
                front("lens", Some("x"))
            ),
        );
        let c = Document::new("c.md", format!("{}| ID | Verdict | Tag | Evidence |\n|---|---|---|---|\n| x-F1 | PLAUSIBLE | OBSERVED | a.rs:1 |\n| x-F2 | CONFIRMED | | |\n\n## Gaps ledger\n- none\n", front("cross-check", Some("y"))));
        let empty = Document::new(
            "e.md",
            format!(
                "{}## Gaps ledger\n- none\n",
                front("cross-check", Some("z"))
            ),
        );
        let r = check(&[l, c, empty]);
        assert_eq!(
            defects(&r, "findings"),
            ["a lens document holds no findings table (ID | Claim | Tag | Evidence)"]
        );
        assert_eq!(defects(&r, "verdicts"), [
            "the verdict on x-F1 is 'PLAUSIBLE', not CONFIRMED, REFUTED or UNVERIFIABLE",
            "the CONFIRMED verdict on x-F2 carries no tag: OBSERVED, DERIVED or INFERRED",
            "a cross-check document holds no verdict table (ID | Verdict | Tag | Evidence)",
        ], "with no lens finding, an unknown id is not reported and verdicts is not_run yet failed");
        let v = r.checks.iter().find(|c| c.check == "verdicts").unwrap();
        assert_eq!(
            v.state,
            AnalysisCheckState::Failed,
            "a defect found where the check could not fully run is a failure"
        );
    }

    #[test]
    fn every_finding_gets_one_verdict_on_a_known_id() {
        let c = Document::new("c.md", format!(
            "{}| ID | Verdict | Tag | Evidence |\n|---|---|---|---|\n| cache-F1 | CONFIRMED (scoped) | OBSERVED | `wc -l` |\n| cache-F2 | REFUTED | DERIVED | 2 = 1 + 1 |\n| other-F1 | CONFIRMED | OBSERVED | a.rs:2 |\n\n## Gaps ledger\n- none\n",
            front("cross-check", Some("load"))
        ));
        let r = check(&[question(), lens(), c]);
        assert_eq!(
            defects(&r, "verdicts"),
            [
                "other-F1 has a verdict and no lens defines it",
                "cache-F3 was never put to refutation: no cross-check gives it a verdict",
            ]
        );
        assert_eq!(
            r.counts["confirmed"], 2,
            "a qualifier is allowed after the verdict word"
        );
    }

    #[test]
    fn a_synthesis_names_causes_remedies_and_owners() {
        let body = "| RC | Cause | Findings |\n|---|---|---|\n| RC-a | x | nothing cited |\n\n\
            | ID | Remedy | Addresses | Owner |\n|---|---|---|---|\n| R1 | y | the weather |  |\n| R2 | z | RC-a | ops |\n\n## Gaps ledger\n- cache-F3\n";
        let r = check(&bundle("UNVERIFIABLE | INFERRED | needs the trace", body));
        assert_eq!(
            defects(&r, "synthesis"),
            [
                "root cause RC-a cites no finding; a root cause is a chain of findings",
                "remedy R1 addresses no root cause and no finding",
                "remedy R1 names no owner (write `unowned` for a hole)",
            ]
        );
        let r = check(&bundle(
            "UNVERIFIABLE | INFERRED | needs the trace",
            "Prose.\n\n## Gaps ledger\n- cache-F3\n",
        ));
        assert_eq!(
            defects(&r, "synthesis"),
            [
                "the synthesis holds no root-cause table (RC | Cause | Findings)",
                "the synthesis holds no remedy table (ID | Remedy | Addresses | Owner)",
            ]
        );
    }

    #[test]
    fn every_pass_document_ledgers_its_gaps() {
        let body = GOOD.replace(
            "## Gaps ledger\n\n- cache-F3: eviction was not tested\n",
            "## Gaps ledger\n\nNothing here.\n\n## After\n\n- cache-F3 is noted outside\n",
        );
        let mut docs = bundle("CONFIRMED | OBSERVED | `x`", &body);
        docs[1] = Document::new(
            "lens-cache.md",
            docs[1].text.replace("## Gaps ledger", "## Open points"),
        );
        let r = check(&docs);
        assert_eq!(
            defects(&r, "gaps"),
            [
                "it has no gaps ledger (a heading naming the Gaps ledger)",
                "its gaps ledger holds no item; `- none` is the empty ledger",
            ]
        );
        assert_eq!(
            r.findings
                .iter()
                .filter(|f| f.check == "gaps")
                .map(|f| f.file.as_str())
                .collect::<Vec<_>>(),
            ["lens-cache.md", "synthesis.md"]
        );
    }

    #[test]
    fn a_nested_heading_stays_in_the_gaps_ledger_and_a_sibling_ends_it() {
        let text = format!(
            "{}## Gaps ledger\n\n### Data\n\n1. the trace\n\n## Next\n\ntext\n",
            front("synthesis", None)
        );
        let p = parse(&Document::new("s.md", text), &mut Vec::new());
        let (lines, item) = p.gaps.expect("a ledger");
        assert!(item);
        assert_eq!(
            lines.iter().copied().collect::<Vec<_>>(),
            (6..=11).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_bundle_declares_one_analysis_and_its_roles() {
        let docs = vec![
            Document::new("plain.md", "# no front matter\n"),
            Document::new("unclosed.md", "---\nanalysis:\n"),
            Document::new("tab.md", "---\nanalysis:\n\tid: x\n---\n"),
            Document::new("scalar.md", "---\nanalysis: yes\n---\n"),
            Document::new("noid.md", "---\nanalysis:\n  pass: lens\n  lens: a\n---\n"),
            Document::new("nopass.md", "---\nanalysis:\n  id: one\n---\n"),
            Document::new(
                "badpass.md",
                "---\nanalysis:\n  id: one\n  pass: draft\n---\n",
            ),
            Document::new(
                "nolens.md",
                "---\nanalysis:\n  id: two\n  pass: cross-check\n---\n",
            ),
            Document::new(
                "q1.md",
                "---\nanalysis:\n  id: one\n  pass: question\n---\n",
            ),
            Document::new(
                "q2.md",
                "---\nanalysis:\n  id: one\n  pass: question\n---\n",
            ),
            Document::new(
                "s1.md",
                "---\nanalysis:\n  id: one\n  pass: synthesis\n---\n",
            ),
            Document::new(
                "s2.md",
                "---\nanalysis:\n  id: one\n  pass: synthesis\n---\n",
            ),
        ];
        let r = check(&docs);
        let bundle: Vec<(String, String)> = r
            .findings
            .iter()
            .filter(|f| f.check == "bundle")
            .map(|f| (f.file.clone(), f.message.clone()))
            .collect();
        let has =
            |file: &str, part: &str| bundle.iter().any(|(f, m)| f == file && m.contains(part));
        assert!(has("plain.md", "declares no `analysis:` front matter"));
        assert!(has("unclosed.md", "front matter is unreadable"));
        assert!(has("tab.md", "not YAML the layer reads"));
        assert!(has("scalar.md", "no `analysis:` mapping"));
        assert!(has("noid.md", "`analysis.id` is missing"));
        assert!(has("nopass.md", "`analysis.pass` is missing"));
        assert!(has("badpass.md", "'draft', not one of"));
        assert!(has("nolens.md", "names its `analysis.lens`"));
        assert!(has("q1.md", "declares 2 analysis ids (one, two)"));
        assert!(has("q2.md", "more than one question document"));
        assert!(has("s2.md", "more than one synthesis document"));
        assert!(r.analysis.is_none());
        assert!(defects(&r, "question").contains(&"the question document is empty".to_string()));
        assert_eq!(
            check(&[]).findings[0].message,
            "the bundle holds no Markdown document"
        );
    }

    #[test]
    fn a_synthesis_alone_is_incomplete_and_says_what_did_not_run() {
        let r = check(&[synthesis(&GOOD.replace("cache-F", "x-F"))]);
        assert_eq!(r.state, AnalysisConformance::Incomplete);
        assert!(!r.ok);
        let reasons: BTreeMap<&str, &str> = r
            .checks
            .iter()
            .filter_map(|c| c.reason.as_deref().map(|why| (c.check.as_str(), why)))
            .collect();
        assert!(reasons["question"].starts_with("no question document"));
        assert_eq!(reasons["findings"], "no lens document");
        assert!(reasons["verdicts"].starts_with("no cross-check document"));
        assert!(reasons["citations"].starts_with("no cross-check document"));
        let r = check(&[lens(), cross("CONFIRMED | OBSERVED | `x`")]);
        let reasons: Vec<&str> = r
            .checks
            .iter()
            .filter_map(|c| c.reason.as_deref())
            .collect();
        assert!(reasons.contains(&"no synthesis document"));
        let r = check(&[question()]);
        assert!(r.checks.iter().any(|c| c.check == "gaps"
            && c.reason.as_deref() == Some("no lens, cross-check or synthesis document")));
        let r = check(&[cross("CONFIRMED | OBSERVED | `x`")]);
        assert!(r.checks.iter().any(|c| c.check == "verdicts"
            && c.reason.as_deref() == Some("no lens finding to give a verdict on")));
    }

    #[test]
    fn cells_keep_pipes_inside_code_and_escapes() {
        assert_eq!(
            cells("| a | `x | y` | b \\| c |"),
            ["a", "`x | y`", "b \\| c"]
        );
        assert_eq!(cells("no | outer"), ["no", "outer"]);
        assert!(is_separator("|---|:--:|"));
        assert!(!is_separator("| a |"));
        assert_eq!(heading("### Title "), Some((3, "Title")));
        assert_eq!(heading("#hashtag"), None);
        assert!(
            is_item("12. twelve") && is_item("* star") && !is_item("1.5 not") && !is_item(". x")
        );
        assert_eq!(plain(" **`cache-F1`** "), "cache-F1");
        assert_eq!(Verdict::parse(""), None);
    }

    #[test]
    fn a_bundle_is_read_from_the_repository_only() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("analysis");
        std::fs::create_dir_all(dir.join("work")).unwrap();
        for d in bundle("UNVERIFIABLE | INFERRED | needs the trace", GOOD) {
            std::fs::write(dir.join(&d.path), &d.text).unwrap();
        }
        std::fs::write(dir.join("data.csv"), "a,b\n").unwrap();
        let (docs, ignored) = read_bundle(root.path(), "analysis").unwrap();
        assert_eq!(docs.len(), 4);
        assert_eq!(ignored, ["analysis/data.csv", "analysis/work"]);
        assert!(check(&docs).ok);
        let (one, none) = read_bundle(root.path(), "analysis/synthesis.md").unwrap();
        assert_eq!((one.len(), none.len()), (1, 0));
        assert!(read_bundle(root.path(), "absent")
            .unwrap_err()
            .starts_with("absent: "));
        let outside = tempfile::tempdir().unwrap();
        let err = read_bundle(root.path(), outside.path().to_str().unwrap()).unwrap_err();
        assert!(err.contains("outside the repository"), "{err}");
        assert!(read_bundle(&root.path().join("missing"), ".")
            .unwrap_err()
            .starts_with("the repository root is unreadable"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let locked = dir.join("locked.md");
            std::fs::write(&locked, "x").unwrap();
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
            if std::fs::read_to_string(&locked).is_err() {
                assert!(read_bundle(root.path(), "analysis")
                    .unwrap_err()
                    .starts_with("analysis/locked.md: "));
                assert!(read_bundle(root.path(), "analysis/locked.md").is_err());
            }
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
            let shut = root.path().join("shut");
            std::fs::create_dir(&shut).unwrap();
            std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();
            if std::fs::read_dir(&shut).is_err() {
                assert!(read_bundle(root.path(), "shut")
                    .unwrap_err()
                    .starts_with("shut: "));
            }
            std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
}
