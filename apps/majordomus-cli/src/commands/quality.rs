//! `majordomus quality report`: the crate's own public surface, through the registry's
//! `quality.report`.
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
use crate::capability::builtin::quality::{baseline_key, QualityAnswer, BASELINE};
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, QualityArgs, QualityCommand, QualityReportArgs};
use crate::error::{Error, Result};

/// The exit code when a finding stands. The code every unmet contract in this executable
/// uses, so a caller need not learn a second vocabulary for this one.
pub const EXIT_VIOLATIONS: u8 = 10;

/// Run `majordomus quality`.
pub fn run(args: QualityArgs) -> Result<u8> {
    match args.command {
        QualityCommand::Report(args) => report(args),
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
        let mut report = QualityReport::default();
        report.target = "apps/majordomus-cli".into();
        report.public_api.items = 10;
        report.public_api.documented = 10;
        report.public_api.owe_example = 4;
        report.public_api.exampled = 3;
        report.public_api.exempt = vec![crate::quality::Exemption {
            reason: "a value is shown by an example of what reads it".into(),
            items: 6,
        }];
        report.modules.modules = 2;
        report.modules.documented = 2;
        report.modules.exampled = 1;
        report.modules.behaviourally_tested = 2;
        report.operations.canonical = 5;
        report.operations.cli = 2;
        report.operations.http = 5;
        report.operations.openapi = 5;
        report.operations.mcp = 4;
        report.operations.cli_commands = 7;
        report.operations.cli_from_capability = 2;
        report.operations.cli_local = 5;

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
        let mut report = QualityReport::default();
        let at = |symbol: &str, line| {
            Violation::new(
                ViolationCode::RustPublicMissingExample,
                symbol,
                "apps/majordomus-cli/src/a.rs",
                Some(line),
                "carries behaviour and no example of it",
            )
        };
        report.violations = vec![at("a::one", 3), at("a::two", 9)];

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
