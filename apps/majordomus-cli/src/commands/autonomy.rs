//! `majordomus autonomy`: how much a coding-agent session in this checkout may do unasked,
//! through the registry's `autonomy.status`, `autonomy.explain` and `autonomy.friction`.
//!
//! The command renders the capability's answer and, for `explain`, chooses the exit code
//! from the decision; the judgement is the capability's, so the terminal, the
//! `--format json` document, MCP and HTTP cannot disagree about it.

use std::io::Write;

use clap::ValueEnum;
use serde_json::{json, Value};

use crate::app::App;
use crate::autonomy::{AutonomyStatus, CommandDecision, Decision, Friction};
use crate::capability::Context;
use crate::cli::{AutonomyArgs, AutonomyCommand, OutputFormat};
use crate::error::{Error, Result};

/// The exit code when no static rule decides a command: the client's permission layer does.
pub const EXIT_DEFER: u8 = 10;
/// The exit code when a person approves a command under every profile.
pub const EXIT_APPROVAL: u8 = 11;

/// Run `majordomus autonomy`.
pub fn run(args: AutonomyArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    answer(&app.context, args.command, args.format, &mut stdout.lock())
}

fn ask(ctx: &Context, words: [&str; 2], input: Value) -> Result<Value> {
    let words: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                words.join(" ")
            ),
        })?;
    ctx.execute(id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })
}

fn read<T: serde::de::DeserializeOwned>(value: &Value, what: &str) -> Result<T> {
    serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
        reason: format!("{what} answered something this command cannot read: {e}"),
    })
}

/// Ask the capability `command` names, write its answer to `out`, and return the exit code.
fn answer<W: Write>(
    ctx: &Context,
    command: AutonomyCommand,
    format: OutputFormat,
    out: &mut W,
) -> Result<u8> {
    let w = |out: &mut W, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    let (value, code) = match &command {
        AutonomyCommand::Status => (ask(ctx, ["autonomy", "status"], json!({}))?, 0),
        AutonomyCommand::Explain { command, profile } => {
            let mut input = json!({ "command": command });
            if let Some(p) = profile.and_then(|p| p.to_possible_value()) {
                input["profile"] = json!(p.get_name());
            }
            let value = ask(ctx, ["autonomy", "explain"], input)?;
            let d: CommandDecision = read(&value, "autonomy.explain")?;
            let code = match d.decision {
                Decision::Allow => 0,
                Decision::Defer => EXIT_DEFER,
                Decision::RequireApproval => EXIT_APPROVAL,
            };
            (value, code)
        }
        AutonomyCommand::Friction { since_days } => (
            ask(
                ctx,
                ["autonomy", "friction"],
                json!({ "since_days": since_days }),
            )?,
            0,
        ),
    };
    if matches!(format, OutputFormat::Json) {
        w(out, format!("{value:#}"))?;
        return Ok(code);
    }
    match command {
        AutonomyCommand::Status => {
            let s: AutonomyStatus = read(&value, "autonomy.status")?;
            w(
                out,
                format!(
                    "profile {} -> --permission-mode {}",
                    json!(s.profile).as_str().unwrap_or("?"),
                    json!(s.recommended_mode).as_str().unwrap_or("?")
                ),
            )?;
            for r in &s.reasons {
                w(out, format!("  because {r}"))?;
            }
            for r in &s.refused {
                w(
                    out,
                    format!(
                        "  not {}: {}",
                        json!(r.profile).as_str().unwrap_or("?"),
                        r.because
                    ),
                )?;
            }
            for l in &s.layers {
                let state = match (&l.error, l.present) {
                    (Some(e), _) => format!("unreadable: {e}"),
                    (None, false) => "absent".into(),
                    (None, true) => format!(
                        "mode {} allow {} ask {} deny {} autoMode {}{}",
                        l.default_mode.as_deref().unwrap_or("-"),
                        l.allow,
                        l.ask,
                        l.deny,
                        l.auto_mode_allow,
                        if l.hooks.is_empty() {
                            String::new()
                        } else {
                            format!(" hooks {}", l.hooks.join(","))
                        }
                    ),
                };
                w(out, format!("  {:<8} {} {}", l.scope, l.path, state))?;
            }
        }
        AutonomyCommand::Explain { .. } => {
            let d: CommandDecision = read(&value, "autonomy.explain")?;
            w(
                out,
                format!(
                    "{} ({}, {:?} profile): {}",
                    json!(d.decision).as_str().unwrap_or("?"),
                    json!(d.risk).as_str().unwrap_or("?"),
                    d.profile,
                    d.reason
                ),
            )?;
        }
        AutonomyCommand::Friction { .. } => {
            let f: Friction = read(&value, "autonomy.friction")?;
            w(
                out,
                format!(
                    "{} transcript(s), {} tool call(s) since {}; {} refusal(s){}",
                    f.transcripts,
                    f.tool_calls,
                    f.since,
                    f.by_kind.values().sum::<usize>(),
                    f.per_thousand_calls
                        .map(|r| format!(", {r} per thousand calls"))
                        .unwrap_or_default()
                ),
            )?;
            for (k, n) in &f.by_kind {
                w(out, format!("{n:>6}  {k}"))?;
            }
            for (r, n) in &f.by_reason {
                w(out, format!("{n:>6}  classifier: {r}"))?;
            }
        }
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::AutonomyProfileArg;

    fn context() -> (crate::synthetic::SyntheticRepository, Context) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let index = repo.index().unwrap();
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(vec![crate::capability::builtin::autonomy::module()])
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        (repo, ctx)
    }

    fn run_text(ctx: &Context, command: AutonomyCommand, format: OutputFormat) -> (u8, String) {
        let mut buf = Vec::new();
        let code = answer(ctx, command, format, &mut buf).unwrap();
        (code, String::from_utf8(buf).unwrap())
    }

    #[test]
    fn explain_says_its_decision_with_its_exit_code() {
        let (_r, ctx) = context();
        for (cmd, code, word) in [
            ("cargo test", 0, "allow"),
            ("make", EXIT_DEFER, "defer"),
            ("git reset --hard", EXIT_APPROVAL, "require_approval"),
        ] {
            let c = AutonomyCommand::Explain {
                command: cmd.into(),
                profile: Some(AutonomyProfileArg::Trusted),
            };
            let (got, text) = run_text(&ctx, c, OutputFormat::Text);
            assert_eq!(got, code, "{cmd}");
            assert!(text.starts_with(word), "{cmd}: {text}");
        }
    }

    #[test]
    fn status_and_friction_render_in_both_shapes() {
        let (_r, ctx) = context();
        let (code, text) = run_text(&ctx, AutonomyCommand::Status, OutputFormat::Text);
        assert_eq!(code, 0);
        assert!(text.starts_with("profile "), "{text}");
        assert!(text.contains("project"), "{text}");
        // the mode is printed as the client spells it, so the line can be pasted as a flag
        let mode = text
            .lines()
            .next()
            .and_then(|l| l.rsplit(' ').next())
            .unwrap_or("");
        let modes = [
            "default",
            "acceptEdits",
            "auto",
            "dontAsk",
            "bypassPermissions",
        ];
        assert!(modes.contains(&mode), "{text}");
        let (_, json) = run_text(&ctx, AutonomyCommand::Status, OutputFormat::Json);
        assert!(json.contains("majordomus.autonomy.status/v1"));
        let f = AutonomyCommand::Friction { since_days: 1 };
        let (code, text) = run_text(&ctx, f, OutputFormat::Text);
        assert_eq!(code, 0);
        assert!(text.contains("tool call(s) since"), "{text}");
    }
}
