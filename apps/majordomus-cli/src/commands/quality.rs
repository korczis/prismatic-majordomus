//! `majordomus quality report`: the crate's own public surface, through the registry's
//! `quality.report`; and `majordomus quality rustdoc`: the tree rustdoc renders from it,
//! through `quality.rustdoc`.
//!
//! The command owns the rendering and nothing else. The measurement, the filtering, the
//! ratchet and the verdict all happen inside the capability, so the terminal report, the
//! `--json` document and the HTTP response are three renderings of one execution and
//! cannot disagree about whether the gate passed — which is the failure mode a quality gate
//! must not have.
//!
//! `--write-baseline` is the one thing this command does that the capability does not: it
//! writes the accepted findings into the repository, and writing is not something any
//! projection of the registry offers. It is a deliberate act of a person, which is why it
//! is a flag on a command and not a field of an input.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::capability::builtin::quality::{baseline_key, judge_rustdoc, QualityAnswer, BASELINE};
use crate::capability::CapabilityError;
use crate::cli::{
    OutputFormat, QualityArgs, QualityCommand, QualityReportArgs, QualityRustdocArgs,
};
use crate::error::{Error, Result};
use crate::quality::rustdoc::{RustdocFindingKind, RustdocReport, RustdocVerdict, Tree};

/// The exit code when a finding stands. The code every unmet contract in this executable
/// uses, so a caller need not learn a second vocabulary for this one.
pub const EXIT_VIOLATIONS: u8 = 10;

/// Run `majordomus quality`.
pub fn run(args: QualityArgs) -> Result<u8> {
    match args.command {
        QualityCommand::Report(args) => report(args),
        QualityCommand::Rustdoc(args) => rustdoc(args),
    }
}

fn report(args: QualityReportArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = ctx
        .registry
        .by_cli(&["quality".to_string(), "report".to_string()])
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus quality report`".into(),
        })?;

    // the baseline is written from the unfiltered measurement: a baseline recorded through
    // a filter would accept only what the filter happened to show
    let input = if args.write_baseline {
        // everything, the accepted debt included: the file records the whole of what stands
        json!({ "include_baselined": true })
    } else {
        json!({
            "code": args.code,
            "path": args.path,
            "summary_only": args.summary,
            "include_baselined": args.include_baselined,
        })
    };
    let value = ctx.execute(id, input).map_err(map)?;
    // the typed result the capability returned, read back through the same serialisation
    // every other transport reads it through: the terminal rendering below is a reading of
    // this value and never a second computation of it
    let answer: QualityAnswer =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!("quality.report answered with something this command cannot read: {e}"),
        })?;

    if args.write_baseline {
        return write_baseline(&app, &answer);
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => {
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => render(&mut out, &answer)?,
    }
    Ok(if answer.passes { 0 } else { EXIT_VIOLATIONS })
}

/// The terminal rendering of one answer.
///
/// Generic over the sink so the shape a person reads can be asserted against a buffer
/// rather than only against a terminal: a renderer only a terminal can see is a renderer
/// nothing tests, and this one carries the verdict a build acts on.
fn render<W: Write>(out: &mut W, answer: &QualityAnswer) -> Result<()> {
    let w = |out: &mut W, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    if !answer.measured {
        w(
            out,
            format!(
                "quality     not measured: {}",
                answer.reason.as_deref().unwrap_or("")
            ),
        )?;
        return Ok(());
    }
    let r = &answer.report;
    w(out, format!("target      {}", r.target))?;
    w(
        out,
        format!(
            "items       {} exported, {} documented",
            r.public_api.items, r.public_api.documented
        ),
    )?;
    w(
        out,
        format!(
            "examples    {} of {} items that owe one",
            r.public_api.exampled, r.public_api.owe_example
        ),
    )?;
    for e in &r.public_api.exempt {
        w(out, format!("  exempt    {:5}  {}", e.items, e.reason))?;
    }
    w(
        out,
        format!(
            "modules     {} exported, {} documented, {} exampled, {} behaviourally tested",
            r.modules.modules,
            r.modules.documented,
            r.modules.exampled,
            r.modules.behaviourally_tested
        ),
    )?;
    w(
        out,
        format!(
            "operations  {} canonical: {} cli, {} http, {} openapi, {} mcp",
            r.operations.canonical,
            r.operations.cli,
            r.operations.http,
            r.operations.openapi,
            r.operations.mcp
        ),
    )?;
    w(
        out,
        format!(
            "commands    {} runnable: {} from a capability, {} classified local",
            r.operations.cli_commands, r.operations.cli_from_capability, r.operations.cli_local
        ),
    )?;
    if answer.baselined > 0 {
        w(
            out,
            format!(
                "baseline    {} finding(s) accepted from {BASELINE}",
                answer.baselined
            ),
        )?;
    }

    if r.violations.is_empty() {
        w(out, String::new())?;
        w(
            out,
            if answer.passes {
                "quality: no findings".to_string()
            } else {
                "quality: findings were not listed (--summary)".to_string()
            },
        )?;
        return Ok(());
    }
    w(out, String::new())?;
    // grouped by code, because that is how somebody fixes them: one kind at a time, and
    // the reason and the remedy are properties of the code rather than of each finding
    let mut by_code: std::collections::BTreeMap<&str, Vec<&crate::quality::Violation>> =
        Default::default();
    for v in &r.violations {
        by_code.entry(v.code.as_str()).or_default().push(v);
    }
    for (code, group) in &by_code {
        let first = group[0];
        w(out, format!("{code}  ({} finding(s))", group.len()))?;
        w(out, format!("  rule        {}", first.rule))?;
        w(out, format!("  why         {}", first.why))?;
        w(out, format!("  remedy      {}", first.remediation))?;
        for v in group {
            w(out, format!("  {}", v.line_summary()))?;
        }
        w(out, String::new())?;
    }
    w(out, format!("quality: {} finding(s)", r.violations.len()))?;
    Ok(())
}

/// `majordomus quality rustdoc`.
///
/// Without `--tree` the command is the capability's projection: `quality.rustdoc` is
/// executed and its typed answer rendered, so the terminal, the JSON document and the HTTP
/// response are one execution. `--tree` names another copy of the tree, which only a person
/// at this terminal may do; that path runs the same judgement function the capability runs,
/// so a copy can be judged but never judged differently.
fn rustdoc(args: QualityRustdocArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let root = std::path::Path::new(&ctx.index.repository.root);
    let value = match &args.tree {
        None => {
            let id = ctx
                .registry
                .by_cli(&["quality".to_string(), "rustdoc".to_string()])
                .map(|c| c.id.as_str())
                .ok_or_else(|| Error::Protocol {
                    reason: "no capability is exposed as `majordomus quality rustdoc`".into(),
                })?;
            ctx.execute(
                id,
                json!({ "kind": args.kind, "summary_only": args.summary }),
            )
            .map_err(map)?
        }
        Some(dir) => {
            let kind = match &args.kind {
                None => None,
                Some(k) => Some(
                    serde_json::from_value::<RustdocFindingKind>(json!(k)).map_err(|_| {
                        Error::Refused {
                            code: crate::cli::EXIT_USAGE,
                            reason: format!(
                                "'{k}' is not a kind of rustdoc finding; the kinds are: {}",
                                RustdocFindingKind::all()
                                    .iter()
                                    .map(|k| k.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        }
                    })?,
                ),
            };
            let absolute = if dir.is_absolute() {
                dir.clone()
            } else {
                std::env::current_dir()
                    .map_err(|e| Error::io(dir, e))?
                    .join(dir)
            };
            let shown = absolute
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| dir.to_string_lossy().into_owned());
            let tree = Tree::At {
                dir: absolute,
                shown,
                mount: None,
            };
            let report = judge_rustdoc(root, tree)?.filtered(kind, args.summary);
            serde_json::to_value(&report).map_err(|e| Error::Protocol {
                reason: format!("the rustdoc report does not serialise: {e}"),
            })?
        }
    };
    let report: RustdocReport =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!(
                "quality.rustdoc answered with something this command cannot read: {e}"
            ),
        })?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => render_rustdoc(&mut out, &report)?,
    }
    Ok(report.exit_code())
}

/// The terminal rendering of one rustdoc judgement: what was judged, what was counted, and
/// every finding grouped by kind with its remedy once.
fn render_rustdoc<W: Write>(out: &mut W, r: &RustdocReport) -> Result<()> {
    let w = |out: &mut W, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    let mounted = r
        .mount
        .as_deref()
        .map(|m| format!(", mounted at {m}"))
        .unwrap_or_default();
    w(out, format!("tree        {}{mounted}", r.tree))?;
    if !r.krate.is_empty() {
        w(out, format!("crate       {}", r.krate))?;
    }
    if r.verdict == RustdocVerdict::NoTree {
        w(
            out,
            format!(
                "rustdoc: no tree to judge: {}",
                r.reason.as_deref().unwrap_or("")
            ),
        )?;
        return Ok(());
    }
    let c = &r.counts;
    w(
        out,
        format!(
            "built from  {}",
            r.built_from.as_deref().unwrap_or("(not declared)")
        ),
    )?;
    w(
        out,
        format!("head        {}", r.head.as_deref().unwrap_or("(unknown)")),
    )?;
    w(
        out,
        format!(
            "items       {} exported; {} own a page, deriving {} page(s)",
            c.exported_items, c.page_owning_items, c.derived_pages
        ),
    )?;
    w(
        out,
        format!(
            "files       {} item page(s), {} redirect stub(s), {} system file(s); {} html of {} file(s)",
            c.item_pages, c.redirect_stubs, c.system_files, c.html_files, c.files
        ),
    )?;
    w(out, format!("links       {} relative", c.relative_links))?;
    for a in &c.accepted_links {
        w(
            out,
            format!(
                "  accepted  {:5}  {} on {} page(s)",
                a.links,
                a.class.as_str(),
                a.pages
            ),
        )?;
    }
    let external: usize = c.external_links.iter().map(|h| h.links).sum();
    w(
        out,
        format!(
            "external    {external} link(s) to {} host(s), counted and never fetched",
            c.external_links.len()
        ),
    )?;
    w(
        out,
        format!(
            "modules     {} exported, each with its page (--format json lists them)",
            r.modules.len()
        ),
    )?;
    w(out, String::new())?;
    if r.findings.is_empty() {
        w(
            out,
            match r.verdict {
                RustdocVerdict::Clean => "rustdoc: clean".to_string(),
                _ => "rustdoc: findings were not listed (--summary or --kind)".to_string(),
            },
        )?;
        return Ok(());
    }
    let mut by_kind: std::collections::BTreeMap<
        &str,
        Vec<&crate::quality::rustdoc::RustdocFinding>,
    > = Default::default();
    for f in &r.findings {
        by_kind.entry(f.kind.as_str()).or_default().push(f);
    }
    for (kind, group) in &by_kind {
        w(out, format!("{kind}  ({} finding(s))", group.len()))?;
        w(out, format!("  remedy      {}", group[0].remedy))?;
        for f in group {
            w(out, format!("  {}", f.line_summary()))?;
        }
        w(out, String::new())?;
    }
    w(out, format!("rustdoc: {} finding(s)", r.findings.len()))?;
    Ok(())
}

/// Record today's findings as the accepted baseline. A deliberate act, which is why it has
/// its own flag and prints what it wrote.
fn write_baseline(app: &App, answer: &QualityAnswer) -> Result<u8> {
    let root = std::path::Path::new(&app.context.index.repository.root);
    let path = root.join(BASELINE);
    let mut lines: Vec<String> = answer.report.violations.iter().map(baseline_key).collect();
    // the report is already sorted; sorting the keys as well makes the file's order a
    // property of its content rather than of the walk that produced it
    lines.sort();
    lines.dedup();
    let header = format!(
        "# The findings of {} that project.rust-public-api-quality accepts today.\n\
         #\n\
         # One per line: CODE<TAB>path<TAB>symbol. The line number is deliberately not part\n\
         # of the key, so that a finding moving down its file is the same finding and an\n\
         # unrelated edit above it does not read as new debt.\n\
         #\n\
         # The gate fails for a finding that is not here, so the debt can shrink and cannot\n\
         # grow. Written only by `majordomus quality report --write-baseline`. The rule is\n\
         # not finished until this file is empty and removed.\n",
        crate::capability::model::CRATE_DIR
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(&path, format!("{header}{}\n", lines.join("\n")))
        .map_err(|e| Error::io(&path, e))?;
    println!("{BASELINE}: {} finding(s) accepted", lines.len());
    Ok(0)
}

/// A capability error as this command's exit code.
///
/// An input the capability refused is the caller's, so it exits `2` — the usage code of
/// the contract — rather than `13`, which says the executable itself failed. A person who
/// mistyped a violation code must not be told the program is broken.
fn map(e: CapabilityError) -> Error {
    match e {
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Refused {
                code: crate::cli::EXIT_USAGE,
                reason,
            }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality::{QualityReport, Violation, ViolationCode};

    fn rendered(answer: &QualityAnswer) -> String {
        let mut buf: Vec<u8> = Vec::new();
        render(&mut buf, answer).expect("the renderer writes");
        String::from_utf8(buf).expect("utf-8")
    }

    fn measured(report: QualityReport, passes: bool, baselined: usize) -> QualityAnswer {
        QualityAnswer {
            measured: true,
            reason: None,
            report,
            passes,
            baselined,
        }
    }

    #[test]
    fn a_repository_with_no_crate_is_rendered_as_an_answer_and_not_as_a_failure() {
        let out = rendered(&QualityAnswer {
            measured: false,
            reason: Some("no Rust crate at apps/majordomus-cli".into()),
            report: QualityReport::default(),
            passes: true,
            baselined: 0,
        });
        assert!(out.contains("not measured"), "{out}");
        assert!(
            out.contains("no Rust crate at apps/majordomus-cli"),
            "{out}"
        );
        // and nothing that would read as a measurement of zero
        assert!(!out.contains("items "), "{out}");
    }

    #[test]
    fn the_counts_a_reader_acts_on_are_all_present() {
        let report = QualityReport {
            target: "apps/majordomus-cli".into(),
            public_api: crate::quality::PublicApiQuality {
                items: 10,
                documented: 10,
                owe_example: 4,
                exampled: 3,
                exempt: vec![crate::quality::Exemption {
                    reason: "a value is shown by an example of what reads it".into(),
                    items: 6,
                }],
            },
            modules: crate::quality::ModuleQuality {
                modules: 2,
                documented: 2,
                exampled: 1,
                behaviourally_tested: 2,
            },
            operations: crate::quality::OperationParity {
                canonical: 5,
                cli: 2,
                http: 5,
                openapi: 5,
                mcp: 4,
                cli_commands: 7,
                cli_from_capability: 2,
                cli_local: 5,
            },
            ..QualityReport::default()
        };

        let out = rendered(&measured(report, true, 0));
        for fragment in [
            "target      apps/majordomus-cli",
            "10 exported, 10 documented",
            "3 of 4 items that owe one",
            "a value is shown by an example of what reads it",
            "2 exported, 2 documented, 1 exampled, 2 behaviourally tested",
            "5 canonical: 2 cli, 5 http, 5 openapi, 4 mcp",
            "7 runnable: 2 from a capability, 5 classified local",
            "quality: no findings",
        ] {
            assert!(out.contains(fragment), "missing {fragment:?} in:\n{out}");
        }
    }

    #[test]
    fn a_finding_is_rendered_with_the_rule_the_reason_and_the_remedy_once_per_code() {
        let at = |symbol: &str, line| {
            Violation::new(
                ViolationCode::RustPublicMissingExample,
                symbol,
                "apps/majordomus-cli/src/a.rs",
                Some(line),
                "carries behaviour and no example of it",
            )
        };
        let report = QualityReport {
            violations: vec![at("a::one", 3), at("a::two", 9)],
            ..QualityReport::default()
        };

        let out = rendered(&measured(report, false, 0));
        assert!(
            out.contains("RUST_PUBLIC_MISSING_EXAMPLE  (2 finding(s))"),
            "{out}"
        );
        // the reason and the remedy belong to the code, so they appear once, not per finding
        assert_eq!(out.matches("  rule        ").count(), 1, "{out}");
        assert_eq!(out.matches("  why         ").count(), 1, "{out}");
        assert_eq!(out.matches("  remedy      ").count(), 1, "{out}");
        // and every occurrence is located
        assert!(
            out.contains("src/a.rs:3") && out.contains("src/a.rs:9"),
            "{out}"
        );
        assert!(out.contains("quality: 2 finding(s)"), "{out}");
    }

    #[test]
    fn what_the_baseline_accepts_is_said_rather_than_hidden() {
        let out = rendered(&measured(QualityReport::default(), true, 7));
        assert!(out.contains("baseline    7 finding(s) accepted"), "{out}");
        assert!(out.contains(BASELINE), "it names the file: {out}");

        // and when it accepts nothing, the line is absent rather than reading "0"
        let out = rendered(&measured(QualityReport::default(), true, 0));
        assert!(!out.contains("baseline    "), "{out}");
    }

    #[test]
    fn a_summary_that_hid_the_findings_does_not_read_as_a_clean_report() {
        // --summary clears the list but not the verdict; saying "no findings" here would be
        // the one lie this command must not tell
        let out = rendered(&measured(QualityReport::default(), false, 0));
        assert!(out.contains("were not listed"), "{out}");
        assert!(!out.contains("quality: no findings"), "{out}");
    }

    fn rustdoc_rendered(report: &RustdocReport) -> String {
        let mut buf: Vec<u8> = Vec::new();
        render_rustdoc(&mut buf, report).expect("the renderer writes");
        String::from_utf8(buf).expect("utf-8")
    }

    fn rustdoc_report(verdict: RustdocVerdict) -> RustdocReport {
        serde_json::from_value(serde_json::json!({
            "schema": crate::quality::rustdoc::SCHEMA,
            "tree": "target/web/rustdoc",
            "mount": "/rustdoc",
            "crate": "majordomus_cli",
            "built_from": "abc",
            "head": "abc",
            "verdict": verdict,
            "counts": {
                "exported_items": 10, "page_owning_items": 4, "derived_pages": 4,
                "item_pages": 3, "redirect_stubs": 1, "system_files": 7, "html_files": 5,
                "files": 12, "relative_links": 40,
                "accepted_links": [
                    {"class": "inherited-documentation", "links": 2, "pages": 1, "written": ["x"]},
                    {"class": "unwritten-implementors", "links": 0, "pages": 0, "written": []}
                ],
                "external_links": [{"host": "doc.rust-lang.org", "links": 9}]
            },
            "modules": [{"path": "majordomus_cli", "route": "majordomus_cli/index.html"}],
            "findings": []
        }))
        .unwrap()
    }

    #[test]
    fn a_rustdoc_judgement_is_rendered_with_its_counts_and_its_verdict() {
        let out = rustdoc_rendered(&rustdoc_report(RustdocVerdict::Clean));
        for fragment in [
            "tree        target/web/rustdoc, mounted at /rustdoc",
            "built from  abc",
            "10 exported; 4 own a page, deriving 4 page(s)",
            "3 item page(s), 1 redirect stub(s), 7 system file(s)",
            "inherited-documentation on 1 page(s)",
            "9 link(s) to 1 host(s)",
            "rustdoc: clean",
        ] {
            assert!(out.contains(fragment), "missing {fragment:?} in:\n{out}");
        }
        // a verdict of findings whose list was filtered away never reads as clean
        let out = rustdoc_rendered(&rustdoc_report(RustdocVerdict::Findings));
        assert!(!out.contains("rustdoc: clean"), "{out}");
        assert!(out.contains("were not listed"), "{out}");
    }

    #[test]
    fn a_rustdoc_finding_is_rendered_under_its_kind_with_the_remedy_once() {
        let mut report = rustdoc_report(RustdocVerdict::Findings);
        report.findings = serde_json::from_value(serde_json::json!([
            {"kind": "link", "file": "a.html", "subject": "x", "message": "m", "remedy": "fix it"},
            {"kind": "link", "file": "b.html", "subject": "y", "message": "m", "remedy": "fix it"}
        ]))
        .unwrap();
        let out = rustdoc_rendered(&report);
        assert!(out.contains("link  (2 finding(s))"), "{out}");
        assert_eq!(out.matches("  remedy      ").count(), 1, "{out}");
        assert!(
            out.contains("a.html: x - m") && out.contains("b.html: y - m"),
            "{out}"
        );
        assert!(out.contains("rustdoc: 2 finding(s)"), "{out}");
    }

    #[test]
    fn nothing_to_judge_is_said_with_the_reason_and_no_counts() {
        let mut report = rustdoc_report(RustdocVerdict::NoTree);
        report.reason = Some("the producer has not run".into());
        let out = rustdoc_rendered(&report);
        assert!(
            out.contains("no tree to judge: the producer has not run"),
            "{out}"
        );
        assert!(
            !out.contains("items "),
            "nothing that reads as a measurement: {out}"
        );
    }

    #[test]
    fn an_input_the_capability_refuses_is_the_callers_mistake_and_not_an_internal_error() {
        let usage = map(CapabilityError::InvalidInput("no such code".into()));
        assert_eq!(usage.exit_code(), crate::cli::EXIT_USAGE);
        let refused = map(CapabilityError::Refused("blank".into()));
        assert_eq!(refused.exit_code(), crate::cli::EXIT_USAGE);
        // and a handler that broke is still the executable's fault
        let internal = map(CapabilityError::Internal("boom".into()));
        assert_eq!(internal.exit_code(), 13);
    }
}
