//! `majordomus autonomy`: how much a coding-agent session in this checkout may do unasked,
//! through the registry's `autonomy.status`, `autonomy.explain` and `autonomy.friction`.
//!
//! The command renders the capability's answer and, for `explain`, chooses the exit code
//! from the decision; the judgement is the capability's, so the terminal, the
//! `--format json` document, MCP and HTTP cannot disagree about it. The text is rendered
//! from the answer as served, so a field the capability adds never breaks the terminal.

use std::io::Write;

use clap::ValueEnum;
use serde_json::{json, Value};

use crate::app::App;
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
    let (code, text) = answer(&app.context, &args.command, args.format)?;
    emit(&mut std::io::stdout().lock(), &text).map(|()| code)
}

fn emit<W: Write>(out: &mut W, text: &str) -> Result<()> {
    out.write_all(text.as_bytes()).map_err(Error::Transport)
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

/// Ask the capability `command` names; the exit code and the text to write.
fn answer(ctx: &Context, command: &AutonomyCommand, format: OutputFormat) -> Result<(u8, String)> {
    let value = match command {
        AutonomyCommand::Status => ask(ctx, ["autonomy", "status"], json!({}))?,
        AutonomyCommand::Explain { command, profile } => {
            let mut input = json!({ "command": command });
            if let Some(p) = profile.and_then(|p| p.to_possible_value()) {
                input["profile"] = json!(p.get_name());
            }
            ask(ctx, ["autonomy", "explain"], input)?
        }
        AutonomyCommand::Friction { since_days } => ask(
            ctx,
            ["autonomy", "friction"],
            json!({ "since_days": since_days }),
        )?,
    };
    let text = match format {
        OutputFormat::Json => format!("{value:#}\n"),
        _ => text(command, &value),
    };
    Ok((exit_code(command, &value), text))
}

/// `explain` says its decision with its exit code; the other two answer and exit 0.
fn exit_code(command: &AutonomyCommand, value: &Value) -> u8 {
    match command {
        AutonomyCommand::Explain { .. } => match value["decision"].as_str() {
            Some("allow") => 0,
            Some("require_approval") => EXIT_APPROVAL,
            _ => EXIT_DEFER,
        },
        _ => 0,
    }
}

fn list<'a>(v: &'a Value, pointer: &str) -> &'a [Value] {
    v.pointer(pointer)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn word<'a>(v: &'a Value, pointer: &str) -> &'a str {
    v.pointer(pointer).and_then(Value::as_str).unwrap_or("?")
}

/// One settings layer, as `status` prints it: presence, mode, rule counts, hook events.
fn layer_state(l: &Value) -> String {
    if let Some(e) = l["error"].as_str() {
        return format!("unreadable: {e}");
    }
    if l["present"] != true {
        return "absent".into();
    }
    let hooks: Vec<&str> = list(l, "/hooks").iter().filter_map(Value::as_str).collect();
    let hooks = if hooks.is_empty() {
        String::new()
    } else {
        format!(" hooks {}", hooks.join(","))
    };
    format!(
        "mode {} allow {} ask {} deny {} autoMode {}{hooks}",
        l["default_mode"].as_str().unwrap_or("-"),
        l["allow"],
        l["ask"],
        l["deny"],
        l["auto_mode_allow"],
    )
}

/// The answer as a person reads it.
fn text(command: &AutonomyCommand, v: &Value) -> String {
    let mut lines = Vec::new();
    match command {
        AutonomyCommand::Status => {
            lines.push(format!(
                "profile {} -> --permission-mode {}",
                word(v, "/profile"),
                word(v, "/recommended_mode")
            ));
            for r in list(v, "/reasons") {
                lines.push(format!("  because {}", r.as_str().unwrap_or("?")));
            }
            for r in list(v, "/refused") {
                lines.push(format!(
                    "  not {}: {}",
                    word(r, "/profile"),
                    word(r, "/because")
                ));
            }
            for l in list(v, "/layers") {
                lines.push(format!(
                    "  {:<8} {} {}",
                    word(l, "/scope"),
                    word(l, "/path"),
                    layer_state(l)
                ));
            }
        }
        AutonomyCommand::Explain { .. } => lines.push(format!(
            "{} ({}, {} profile): {}",
            word(v, "/decision"),
            word(v, "/risk"),
            word(v, "/profile"),
            word(v, "/reason")
        )),
        AutonomyCommand::Friction { .. } => {
            let kinds: Vec<(&String, &Value)> =
                v["by_kind"].as_object().into_iter().flatten().collect();
            let refusals: u64 = kinds.iter().filter_map(|(_, n)| n.as_u64()).sum();
            let rate = v["per_thousand_calls"]
                .as_f64()
                .map(|r| format!(", {r} per thousand calls"))
                .unwrap_or_default();
            lines.push(format!(
                "{} transcript(s), {} tool call(s) since {}; {refusals} refusal(s){rate}",
                v["transcripts"],
                v["tool_calls"],
                word(v, "/since"),
            ));
            // a Value ignores a width, so the counts are printed as the numbers they are
            for (k, n) in kinds {
                lines.push(format!("{:>6}  {k}", n.as_u64().unwrap_or(0)));
            }
            for (r, n) in v["by_reason"].as_object().into_iter().flatten() {
                lines.push(format!("{:>6}  classifier: {r}", n.as_u64().unwrap_or(0)));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{AutonomyProfileArg, RepoArgs};

    fn context_of(
        modules: Vec<crate::capability::module::ModuleDescriptor>,
    ) -> (crate::synthetic::SyntheticRepository, Context) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let index = repo.index().unwrap();
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(modules)
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        (repo, ctx)
    }

    fn context() -> (crate::synthetic::SyntheticRepository, Context) {
        context_of(vec![crate::capability::builtin::autonomy::module()])
    }

    fn explain(command: &str) -> AutonomyCommand {
        AutonomyCommand::Explain {
            command: command.into(),
            profile: Some(AutonomyProfileArg::Trusted),
        }
    }

    #[test]
    fn explain_says_its_decision_with_its_exit_code() {
        let (_r, ctx) = context();
        for (cmd, code, word) in [
            ("cargo test", 0, "allow"),
            ("make", EXIT_DEFER, "defer"),
            ("git reset --hard", EXIT_APPROVAL, "require_approval"),
        ] {
            let (got, text) = answer(&ctx, &explain(cmd), OutputFormat::Text).unwrap();
            assert_eq!(got, code, "{cmd}");
            assert!(text.starts_with(word), "{cmd}: {text}");
            assert!(text.contains("trusted profile"), "{cmd}: {text}");
        }
        // with no profile named, the checkout's own judges it
        let own = AutonomyCommand::Explain {
            command: "ls".into(),
            profile: None,
        };
        assert_eq!(answer(&ctx, &own, OutputFormat::Text).unwrap().0, 0);
    }

    #[test]
    fn status_and_friction_render_in_both_shapes() {
        let (_r, ctx) = context();
        let (code, text) = answer(&ctx, &AutonomyCommand::Status, OutputFormat::Text).unwrap();
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
        let (_, json) = answer(&ctx, &AutonomyCommand::Status, OutputFormat::Json).unwrap();
        assert!(json.contains("majordomus.autonomy.status/v1"));
        let f = AutonomyCommand::Friction { since_days: 1 };
        let (code, text) = answer(&ctx, &f, OutputFormat::Text).unwrap();
        assert_eq!(code, 0);
        assert!(text.contains("tool call(s) since"), "{text}");
    }

    #[test]
    fn a_registry_without_the_capability_and_a_refused_input_are_errors() {
        let (_r, empty) = context_of(Vec::new());
        for c in [
            AutonomyCommand::Status,
            explain("ls"),
            AutonomyCommand::Friction { since_days: 1 },
        ] {
            let e = answer(&empty, &c, OutputFormat::Text).unwrap_err();
            assert!(e.to_string().contains("no capability is exposed"), "{e}");
        }
        let (_r, ctx) = context();
        let e = answer(
            &ctx,
            &AutonomyCommand::Friction { since_days: 0 },
            OutputFormat::Json,
        )
        .unwrap_err();
        assert!(e.to_string().contains("since_days"), "{e}");
    }

    #[test]
    fn every_layer_state_and_refusal_is_printed() {
        let v = json!({
            "profile": "conservative",
            "recommended_mode": "default",
            "reasons": ["a reason"],
            "refused": [{ "profile": "trusted", "because": "not accepted" }],
            "layers": [
                { "scope": "local", "path": "a", "present": true, "error": "bad json" },
                { "scope": "managed", "path": "b", "present": false },
                { "scope": "project", "path": "c", "present": true, "default_mode": "auto",
                  "allow": 2, "ask": 0, "deny": 1, "auto_mode_allow": 0, "hooks": [] },
                { "scope": "user", "path": "d", "present": true, "allow": 1, "ask": 0,
                  "deny": 0, "auto_mode_allow": 3, "hooks": ["PreToolUse", "Stop"] },
            ],
        });
        let t = text(&AutonomyCommand::Status, &v);
        assert!(
            t.starts_with("profile conservative -> --permission-mode default\n"),
            "{t}"
        );
        assert!(t.contains("  because a reason\n"), "{t}");
        assert!(t.contains("  not trusted: not accepted\n"), "{t}");
        assert!(t.contains("a unreadable: bad json"), "{t}");
        assert!(t.contains("b absent"), "{t}");
        assert!(
            t.contains("c mode auto allow 2 ask 0 deny 1 autoMode 0\n"),
            "{t}"
        );
        assert!(
            t.contains("d mode - allow 1 ask 0 deny 0 autoMode 3 hooks PreToolUse,Stop"),
            "{t}"
        );
    }

    #[test]
    fn friction_prints_its_rate_only_when_a_tool_was_called() {
        let some = json!({
            "transcripts": 2, "tool_calls": 1000, "since": "t",
            "by_kind": { "classifier_denial": 3, "hook_block": 1 },
            "by_reason": { "CI Bypass": 3 },
            "per_thousand_calls": 4.0,
        });
        let t = text(&AutonomyCommand::Friction { since_days: 1 }, &some);
        assert!(
            t.starts_with("2 transcript(s), 1000 tool call(s) since t; 4 refusal(s), 4 per"),
            "{t}"
        );
        assert!(t.contains("     3  classifier_denial\n"), "{t}");
        assert!(t.contains("     3  classifier: CI Bypass\n"), "{t}");
        let none = json!({ "transcripts": 0, "tool_calls": 0, "since": "t", "by_kind": {} });
        let t = text(&AutonomyCommand::Friction { since_days: 1 }, &none);
        assert_eq!(t, "0 transcript(s), 0 tool call(s) since t; 0 refusal(s)\n");
    }

    #[test]
    fn a_writer_that_refuses_is_a_transport_error() {
        struct Refusing;
        impl Write for Refusing {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("closed"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(Refusing.flush().is_ok());
        assert!(matches!(emit(&mut Refusing, "x"), Err(Error::Transport(_))));
        let mut buf = Vec::new();
        emit(&mut buf, "ok\n").unwrap();
        assert_eq!(buf, b"ok\n");
    }

    #[test]
    fn run_answers_in_a_repository_and_refuses_outside_one() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let args = |root: &std::path::Path, command| AutonomyArgs {
            // the synthetic repository is no git repository: its files are walked
            repo: RepoArgs {
                repo: Some(root.to_path_buf()),
                share: Some(crate::synthetic::crate_share()),
                discovery: crate::cli::DiscoveryMode::Filesystem,
                ..Default::default()
            },
            command,
            format: OutputFormat::Json,
        };
        assert_eq!(
            run(args(repo.root(), explain("git push"))).unwrap(),
            EXIT_APPROVAL
        );
        let nowhere = tempfile::tempdir().unwrap();
        assert!(run(args(
            &nowhere.path().join("absent"),
            AutonomyCommand::Status
        ))
        .is_err());
    }
}
