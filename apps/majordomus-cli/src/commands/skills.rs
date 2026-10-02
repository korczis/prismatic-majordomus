//! `majordomus skills`: every skill as a proven capability, through the registry's own
//! `skills.*` capabilities.
//!
//! This file renders; it decides nothing. Every answer is the output of the capability the
//! command line exposure names, run through the one executor, so the command line, the HTTP
//! routes and the MCP tools cannot answer the same question differently. The one verdict,
//! `verify`, is read out of that output and turned into exit 10.
//!
//! The text rendering keeps the four facts apart. `proven` and `inputs_unchanged` print
//! differently, and an orphan prints which half it lacks, because a column that showed a tick
//! for "a test names it" would be the badge whose derivation cannot be inspected.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::{CapabilityError, Context};
use crate::cli::{OutputFormat, SkillsArgs, SkillsCommand};
use crate::error::{Error, Result};

/// The exit code when `verify` finds a failure.
pub const EXIT_INVALID: u8 = 10;

/// Run `majordomus skills`.
pub fn run(args: SkillsArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    answer(&app.context, &args.command, args.format, &mut stdout.lock())
}

/// Answer one `skills` subcommand from `ctx`, writing it to `out` in `format`.
fn answer<W: Write>(
    ctx: &Context,
    command: &SkillsCommand,
    format: OutputFormat,
    out: &mut W,
) -> Result<u8> {
    match command {
        SkillsCommand::Status => {
            let v = call(ctx, &["skills", "status"], json!({}))?;
            emit(out, format, &v, status_text)?;
            Ok(0)
        }
        SkillsCommand::Explain { id } => {
            let v = call(ctx, &["skills", "explain"], json!({ "id": id }))?;
            emit(out, format, &v, explain_text)?;
            Ok(0)
        }
        SkillsCommand::Verify => {
            let v = call(ctx, &["skills", "verify"], json!({}))?;
            emit(out, format, &v, verify_text)?;
            Ok(if v["valid"].as_bool() == Some(true) {
                0
            } else {
                EXIT_INVALID
            })
        }
    }
}

fn call(ctx: &Context, path: &[&str], input: Value) -> Result<Value> {
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.to_string())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(&id, input).map_err(|e| match e {
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })
}

fn emit<W: Write>(
    out: &mut W,
    format: OutputFormat,
    v: &Value,
    text: impl Fn(&Value) -> String,
) -> Result<()> {
    let body = match format {
        // `{:#}` is serde_json's own pretty writer, the one `to_string_pretty` runs, so the
        // bytes are the same; serialising a `Value` cannot fail, so there is no fallback
        OutputFormat::Json => format!("{v:#}"),
        OutputFormat::Text => text(v),
    };
    writeln!(out, "{body}").map_err(Error::Transport)
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}

fn yes(b: &Value) -> &'static str {
    if b.as_bool() == Some(true) {
        "yes"
    } else {
        "no"
    }
}

fn status_text(v: &Value) -> String {
    let skills = v["skills"].as_array().cloned().unwrap_or_default();
    let width = skills
        .iter()
        .map(|i| s(i, "id").len())
        .max()
        .unwrap_or(2)
        .max(2);
    let mut out = vec![format!(
        "{:<width$}  {:<10}  {:<12}  {:<16}  {:<10}  {:<8}  USED",
        "ID", "STATUS", "STANDING", "TESTED", "DOCUMENTED", "ENFORCED"
    )];
    for i in &skills {
        out.push(format!(
            "{:<width$}  {:<10}  {:<12}  {:<16}  {:<10}  {:<8}  {}",
            s(i, "id"),
            s(i, "status"),
            s(i, "standing"),
            i["tested"]["state"].as_str().unwrap_or(""),
            yes(&i["documented"]["documented"]),
            yes(&i["enforced"]["enforced"]),
            yes(&i["used"]["used"]),
        ));
    }
    out.push(String::new());
    out.push(format!("{} skill(s)", v["count"]));
    out.join("\n")
}

fn explain_text(v: &Value) -> String {
    let mut out = vec![
        format!("{}  {}", s(v, "id"), s(v, "title")),
        String::new(),
        format!("  {}", s(v, "description")),
        String::new(),
        format!(
            "  status {}   version {}   standing {}   source {}",
            s(v, "status"),
            v["version"],
            s(v, "standing"),
            s(v, "path")
        ),
    ];
    if let Some(p) = v["provenance"].as_object() {
        out.push(format!(
            "  provenance  {} {} {}",
            p.get("origin").and_then(Value::as_str).unwrap_or(""),
            p.get("ledger").and_then(Value::as_str).unwrap_or(""),
            p.get("decision").and_then(Value::as_str).unwrap_or("")
        ));
    }
    out.push(format!(
        "  tested      {}",
        v["tested"]["state"].as_str().unwrap_or("")
    ));
    for t in v["tested"]["tests"].as_array().into_iter().flatten() {
        out.push(format!(
            "    test      {}  {}{}",
            s(t, "path"),
            s(t, "state"),
            t["reproduce"]
                .as_str()
                .map(|r| format!("  [reproduce: {r}]"))
                .unwrap_or_default()
        ));
    }
    out.push(format!(
        "  documented  {}  {}",
        yes(&v["documented"]["documented"]),
        s(&v["documented"], "page")
    ));
    let gates: Vec<&str> = v["enforced"]["gates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    out.push(format!(
        "  enforced    {}  doctrine {}  gates {}",
        yes(&v["enforced"]["enforced"]),
        v["enforced"]["doctrine"].as_str().unwrap_or("-"),
        if gates.is_empty() {
            "-".to_string()
        } else {
            gates.join(", ")
        }
    ));
    out.push(format!("  used        {}", yes(&v["used"]["used"])));
    for i in v["used"]["invocations"].as_array().into_iter().flatten() {
        out.push(format!(
            "    invoked   {}:{}  {}",
            s(i, "path"),
            i["line"],
            s(i, "reference")
        ));
    }
    findings_text(&mut out, &v["findings"]);
    out.join("\n")
}

fn findings_text(out: &mut Vec<String>, findings: &Value) {
    for f in findings.as_array().into_iter().flatten() {
        out.push(format!(
            "{} {}  {}: {}  [reproduce: {}]",
            s(f, "level").to_uppercase(),
            s(f, "code"),
            s(f, "subject"),
            s(f, "message"),
            s(f, "reproduce"),
        ));
    }
}

fn verify_text(v: &Value) -> String {
    let mut out = Vec::new();
    findings_text(&mut out, &v["findings"]);
    out.push(format!(
        "{} skill(s), {} failure(s), {} warning(s): {}",
        v["skills"],
        v["failures"],
        v["warnings"],
        if v["valid"].as_bool() == Some(true) {
            "valid"
        } else {
            "invalid"
        }
    ));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::capability::{builtin, CapabilityRegistry};
    use crate::cli::{DiscoveryMode, RepoArgs};
    use crate::synthetic::SyntheticRepository;

    /// A sink that refuses every write, the way a closed pipe does.
    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "closed",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A synthetic repository holding one active skill, `alpha`, that nothing tests or uses.
    fn with_skill() -> SyntheticRepository {
        let repo = SyntheticRepository::small().unwrap();
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let text = std::fs::read_to_string(&sources).unwrap();
        std::fs::write(
            &sources,
            format!("{text}  - id: skill\n    kind: skill\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/skills/*/SKILL.md'\n    required: false\n"),
        )
        .unwrap();
        let skill = repo.root().join(".ai/repo/skills/alpha/SKILL.md");
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        std::fs::write(
            &skill,
            "---\nschema: skill/v1\nid: alpha\nversion: 1\ntitle: Skill alpha\ndescription: The alpha procedure.\nstatus: active\n---\n# Purpose\n\nWhy.\n\n# Procedure\n\n1. Do it.\n\n# Output\n\nA report.\n",
        )
        .unwrap();
        repo
    }

    #[test]
    fn the_status_table_is_one_row_per_skill_under_one_header() {
        let v = json!({ "count": 2, "skills": [
            { "id": "alpha", "status": "active", "standing": "proven",
              "tested": { "state": "proven" }, "documented": { "documented": true },
              "enforced": { "enforced": true }, "used": { "used": true } },
            { "id": "a-longer-skill", "status": "draft", "standing": "not_required",
              "tested": { "state": "untested" }, "documented": { "documented": false },
              "enforced": { "enforced": false }, "used": {} }
        ]});
        assert_eq!(
            status_text(&v),
            [
                "ID              STATUS      STANDING      TESTED            DOCUMENTED  ENFORCED  USED",
                "alpha           active      proven        proven            yes         yes       yes",
                "a-longer-skill  draft       not_required  untested          no          no        no",
                "",
                "2 skill(s)",
            ]
            .join("\n")
        );
        assert_eq!(
            status_text(&json!({ "count": 0, "skills": [] })),
            "ID  STATUS      STANDING      TESTED            DOCUMENTED  ENFORCED  USED\n\n0 skill(s)"
        );
    }

    #[test]
    fn the_explanation_keeps_the_four_facts_apart_and_names_each_finding() {
        let v = json!({
            "id": "alpha", "title": "Skill alpha", "description": "The alpha procedure.",
            "status": "active", "version": 2, "standing": "partial",
            "path": ".ai/repo/skills/alpha/SKILL.md",
            "provenance": { "origin": "extracted", "ledger": "l-1", "decision": "d-1" },
            "tested": { "state": "stale", "tests": [
                { "path": "test/cases/01_a.sh", "state": "stale", "reproduce": "test/run.sh 01_a" },
                { "path": "docs/a.md", "state": "unrunnable" }
            ]},
            "documented": { "documented": true, "page": "site/content/skills/alpha.md" },
            "enforced": { "enforced": true, "doctrine": "project.skills", "gates": ["g1", "g2"] },
            "used": { "used": true, "invocations": [
                { "path": ".ai/repo/workflows/w.md", "line": 3, "reference": "majordomus://skill/alpha" }
            ]},
            "findings": [
                { "level": "warn", "code": "unevidenced", "subject": "alpha", "message": "run it", "reproduce": "test/run.sh 01_a" }
            ]
        });
        assert_eq!(
            explain_text(&v),
            [
                "alpha  Skill alpha",
                "",
                "  The alpha procedure.",
                "",
                "  status active   version 2   standing partial   source .ai/repo/skills/alpha/SKILL.md",
                "  provenance  extracted l-1 d-1",
                "  tested      stale",
                "    test      test/cases/01_a.sh  stale  [reproduce: test/run.sh 01_a]",
                "    test      docs/a.md  unrunnable",
                "  documented  yes  site/content/skills/alpha.md",
                "  enforced    yes  doctrine project.skills  gates g1, g2",
                "  used        yes",
                "    invoked   .ai/repo/workflows/w.md:3  majordomus://skill/alpha",
                "WARN unevidenced  alpha: run it  [reproduce: test/run.sh 01_a]",
            ]
            .join("\n")
        );
        // no provenance, no doctrine and no gate: nothing invented in their place
        let bare =
            json!({ "id": "beta", "tested": {}, "documented": {}, "enforced": {}, "used": {} });
        let text = explain_text(&bare);
        assert!(!text.contains("provenance"), "{text}");
        assert!(
            text.contains("  enforced    no  doctrine -  gates -"),
            "{text}"
        );
    }

    #[test]
    fn the_verdict_line_counts_and_says_valid_or_invalid() {
        let invalid = json!({ "skills": 1, "failures": 1, "warnings": 0, "valid": false,
            "findings": [{ "level": "fail", "code": "unused", "subject": "a", "message": "m", "reproduce": "r" }] });
        assert_eq!(
            verify_text(&invalid),
            "FAIL unused  a: m  [reproduce: r]\n1 skill(s), 1 failure(s), 0 warning(s): invalid"
        );
        let valid = json!({ "skills": 0, "failures": 0, "warnings": 0, "valid": true });
        assert_eq!(
            verify_text(&valid),
            "0 skill(s), 0 failure(s), 0 warning(s): valid"
        );
    }

    /// Each verb reaches its capability and prints what it answered; the verdict is the
    /// exit code; a missing skill is not found; a sink that refuses the answer is an error.
    #[test]
    fn each_verb_answers_through_its_capability_and_a_closed_sink_is_an_error() {
        let repo = with_skill();
        let ctx = repo.context().unwrap();
        let status = SkillsCommand::Status;
        let explain = SkillsCommand::Explain { id: "alpha".into() };

        let mut out = Vec::new();
        assert_eq!(
            answer(&ctx, &status, OutputFormat::Json, &mut out).unwrap(),
            0
        );
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["count"], 1);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("{}\n", serde_json::to_string_pretty(&v).unwrap())
        );

        let mut out = Vec::new();
        assert_eq!(
            answer(&ctx, &explain, OutputFormat::Text, &mut out).unwrap(),
            0
        );
        assert!(String::from_utf8(out)
            .unwrap()
            .starts_with("alpha  Skill alpha\n"));

        // nothing tests or invokes alpha, so it is an orphan and the verdict refuses
        let mut out = Vec::new();
        assert_eq!(
            answer(&ctx, &SkillsCommand::Verify, OutputFormat::Text, &mut out).unwrap(),
            EXIT_INVALID
        );
        assert!(String::from_utf8(out).unwrap().ends_with(": invalid\n"));

        match answer(
            &ctx,
            &SkillsCommand::Explain {
                id: "absent".into(),
            },
            OutputFormat::Text,
            &mut Vec::new(),
        ) {
            Err(e @ Error::NotFound { .. }) => assert_eq!(e.exit_code(), 12),
            other => panic!("an absent skill answered {other:?}"),
        }

        for command in [&status, &explain, &SkillsCommand::Verify] {
            assert!(
                matches!(
                    answer(&ctx, command, OutputFormat::Text, &mut Closed),
                    Err(Error::Transport(_))
                ),
                "{command:?}"
            );
        }
    }

    /// A capability that refuses is a protocol error carrying its message, for every verb; a
    /// registry exposing no `skills` command is named as such; no repository fails first.
    #[test]
    fn a_refusal_an_unexposed_verb_and_no_repository_are_each_an_error() {
        let repo = with_skill();
        let ledger = repo.root().join(crate::evidence::LEDGER_PATH);
        std::fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        std::fs::write(&ledger, "not a ledger").unwrap();
        let ctx = repo.context().unwrap();
        for command in [SkillsCommand::Status, SkillsCommand::Verify] {
            match answer(&ctx, &command, OutputFormat::Json, &mut Vec::new()) {
                Err(Error::Protocol { reason }) => {
                    assert!(
                        reason.contains("not a ledger this version can read"),
                        "{reason}"
                    )
                }
                other => panic!("{command:?} answered over an unreadable ledger: {other:?}"),
            }
        }

        let index = repo.index().unwrap();
        let registry = CapabilityRegistry::builder()
            .with_modules(
                builtin::modules()
                    .into_iter()
                    .filter(|m| m.id.as_str() != "skills")
                    .collect(),
            )
            .build()
            .unwrap();
        let bare = Context::new(Arc::new(index), Arc::new(registry));
        match answer(
            &bare,
            &SkillsCommand::Status,
            OutputFormat::Json,
            &mut Vec::new(),
        ) {
            Err(Error::Protocol { reason }) => {
                assert_eq!(
                    reason,
                    "no capability is exposed as `majordomus skills status`"
                )
            }
            other => panic!("a registry with no skills answered {other:?}"),
        }

        let nowhere = tempfile::tempdir().unwrap();
        let args = SkillsArgs {
            repo: RepoArgs {
                repo: Some(nowhere.path().to_path_buf()),
                discovery: DiscoveryMode::Vcs,
                strict: false,
                share: None,
            },
            command: SkillsCommand::Status,
            format: OutputFormat::Json,
        };
        assert!(run(args).is_err());
    }
}
