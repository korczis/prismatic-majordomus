//! `majordomus proceed`: what a session proceeds with, on the command line (ADR 0177).
//!
//! The capability decides; this renders. `proceed.plan` asks the server holding this
//! checkout's lease, so the claims a person reads here are that server's board and mesh and
//! not the empty ones of this process.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::Context;
use crate::cli::{OutputFormat, ProceedArgs, ProceedCommand};
use crate::error::{Error, Result};

/// The exit code when nothing can be selected: the code `dashboard overview` and `quality
/// report` use for an answer that is not satisfied.
pub const EXIT_NOTHING_SELECTED: u8 = 10;
/// The exit code when an issue is named but the decision is provisional: a source it needs
/// (the board, or a declared mesh) could not be read, so nothing should be taken on it.
pub const EXIT_PROVISIONAL: u8 = 11;

/// Run `majordomus proceed`.
pub fn run(args: ProceedArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    answer(&app.context, &args, &mut stdout.lock())
}

/// Ask the capability exposed as `majordomus proceed plan` and write its answer to `out`.
fn answer(ctx: &Context, args: &ProceedArgs, out: &mut impl Write) -> Result<u8> {
    let ProceedCommand::Plan { milestone, session } = &args.command;
    let id = ctx
        .registry
        .by_cli(&["proceed".to_string(), "plan".to_string()])
        .map(|c| c.id.as_str().to_string())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus proceed plan`".into(),
        })?;
    let mut input = json!({});
    if let Some(m) = milestone {
        input["milestone"] = json!(m);
    }
    if let Some(s) = session {
        input["session"] = json!(s);
    }
    let v = ctx.execute(&id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    let text = if args.format == OutputFormat::Json {
        format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
    } else {
        render(&v)
    };
    out.write_all(text.as_bytes()).map_err(Error::Transport)?;
    Ok(exit_for(&v))
}

/// The exit code the decision's own words imply.
fn exit_for(v: &Value) -> u8 {
    if !v["selected"].is_object() {
        EXIT_NOTHING_SELECTED
    } else if v["provisional"] == json!(true) {
        EXIT_PROVISIONAL
    } else {
        0
    }
}

/// The decision as a person reads it: the selection and why, then what was passed over,
/// what waits and what the decision was read from.
fn render(v: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let list = |v: &Value| -> Vec<Value> { v.as_array().cloned().unwrap_or_default() };
    let mut out = String::new();
    let sel = &v["selected"];
    if sel.is_object() {
        let t = &sel["terms"];
        out.push_str(&format!(
            "selected  {}  {} ({}, wave {}, {})\n",
            s(&sel["issue"]),
            s(&sel["title"]),
            s(&sel["milestone"]),
            t["wave"],
            s(&t["priority"])
        ));
    } else {
        out.push_str("selected  nothing\n");
    }
    out.push_str(&format!("why       {}\n", s(&v["reason"])));
    if sel.is_object() {
        let unblocks: Vec<String> = list(&sel["terms"]["unblocks"]).iter().map(s).collect();
        out.push_str(&format!(
            "unblocks  {}\n",
            if unblocks.is_empty() {
                "nothing".to_string()
            } else {
                unblocks.join(", ")
            }
        ));
        let f = &sel["finishes_with"];
        out.push_str(&format!(
            "finishes  {} (evidence {} of {})\n",
            s(&f["objective"]),
            f["have"],
            f["need"]
        ));
        for (n, step) in list(&v["next"]).iter().enumerate() {
            let label = if n == 0 { "next" } else { "" };
            out.push_str(&format!("{label:<9} {}. {}\n", n + 1, s(step)));
        }
    }
    let mut first = true;
    for c in list(&v["candidates"]) {
        for r in list(&c["refused"]) {
            let label = if first { "passed" } else { "" };
            first = false;
            out.push_str(&format!(
                "{label:<9} {}  {} by {}: {}\n",
                s(&c["issue"]),
                s(&r["constraint"]),
                s(&r["holder"]),
                s(&r["detail"])
            ));
        }
    }
    if let Some(w) = v["waiting"].as_object().filter(|w| !w.is_empty()) {
        let parts: Vec<String> = w.iter().map(|(k, n)| format!("{k} {n}")).collect();
        out.push_str(&format!("waiting   {}\n", parts.join(", ")));
    }
    for (n, src) in list(&v["sources"]).iter().enumerate() {
        let label = if n == 0 { "sources" } else { "" };
        out.push_str(&format!(
            "{label:<9} {} {}: {}\n",
            s(&src["source"]),
            s(&src["state"]),
            s(&src["summary"])
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn args(argv: &[&str]) -> ProceedArgs {
        let mut full = vec!["majordomus", "proceed"];
        full.extend_from_slice(argv);
        match crate::cli::Cli::try_parse_from(full)
            .expect("parses")
            .command
        {
            crate::cli::Command::Proceed(a) => a,
            _ => panic!("not the proceed command"),
        }
    }

    #[test]
    fn a_selection_renders_its_reason_steps_refusals_and_sources() {
        let v = json!({
            "selected": {
                "issue": "I0002", "title": "t", "milestone": "M",
                "terms": {"in_milestone": true, "wave": 1, "priority": "p1", "unblocks": ["I0003"]},
                "refused": [], "shared_with": [], "scope": ["src"],
                "finishes_with": {"objective": "o", "have": 0, "need": 1}
            },
            "reason": "I0002 is the first",
            "next": ["claim", "worktree"],
            "milestone": "M",
            "candidates": [{
                "issue": "I0001",
                "refused": [{"constraint": "claimed", "hold": "k", "holder": "p2", "detail": "names I0001"}]
            }],
            "waiting": {"BLOCKED": 2},
            "sources": [{"source": "board", "state": "read", "holds": 1, "summary": "1 board(s)"}]
        });
        let text = render(&v);
        for line in [
            "selected  I0002  t (M, wave 1, p1)",
            "why       I0002 is the first",
            "unblocks  I0003",
            "finishes  o (evidence 0 of 1)",
            "next      1. claim",
            "          2. worktree",
            "passed    I0001  claimed by p2: names I0001",
            "waiting   BLOCKED 2",
            "sources   board read: 1 board(s)",
        ] {
            assert!(text.contains(line), "missing {line:?} in\n{text}");
        }
        let none = render(&json!({"selected": null, "reason": "r", "sources": []}));
        assert_eq!(none, "selected  nothing\nwhy       r\n");
        let quiet = render(&json!({"selected": {"terms": {"unblocks": []}, "finishes_with": {}}}));
        assert!(quiet.contains("unblocks  nothing"), "{quiet}");
    }

    #[test]
    fn the_exit_code_is_the_decisions_own_word() {
        assert_eq!(exit_for(&json!({"selected": null})), EXIT_NOTHING_SELECTED);
        let sel = json!({"issue": "I1"});
        assert_eq!(
            exit_for(&json!({"selected": sel, "provisional": true})),
            EXIT_PROVISIONAL
        );
        assert_eq!(exit_for(&json!({"selected": sel, "provisional": false})), 0);
    }

    #[test]
    fn the_command_answers_with_the_capability_and_exits_10_on_no_selection() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let mut out = Vec::new();
        let code = answer(
            &ctx,
            &args(&[
                "plan",
                "--milestone",
                "M",
                "--session",
                "s",
                "--format",
                "json",
            ]),
            &mut out,
        )
        .unwrap();
        assert_eq!(code, EXIT_NOTHING_SELECTED);
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["milestone"], "M");
        assert!(v["selected"].is_null());
        let mut out = Vec::new();
        answer(&ctx, &args(&["plan"]), &mut out).unwrap();
        assert!(String::from_utf8(out)
            .unwrap()
            .starts_with("selected  nothing\n"));
    }
}
