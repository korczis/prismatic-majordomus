//! `majordomus brief`: what surrounds this session, on the command line (ADR 0136).
//!
//! The capability composes; this renders. `brief.session` asks the server holding this
//! checkout's lease, so what a person reads here is the board and the mesh of that server
//! and not the empty board of this process.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::cli::{BriefArgs, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus brief`.
pub fn run(args: BriefArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = ctx
        .registry
        .by_cli(&["brief".to_string()])
        .map(|c| c.id.as_str().to_string())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus brief`".into(),
        })?;
    let mut input = json!({
        "checkouts": if args.this_checkout { "this" } else { "repository" },
    });
    if let Some(s) = &args.session {
        input["session"] = json!(s);
    }
    let v = ctx.execute(&id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    render(&mut out, &v, &args).map_err(Error::Transport)?;
    Ok(0)
}

/// The answer in the shape asked for.
fn render(out: &mut impl Write, v: &serde_json::Value, args: &BriefArgs) -> std::io::Result<()> {
    let text = |p: &str| v.pointer(p).and_then(|x| x.as_str()).unwrap_or_default();
    if args.format == OutputFormat::Json {
        return writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
        );
    }
    if args.keep_awake {
        writeln!(out, "{}", text("/keep_awake"))?;
        return writeln!(out, "Now: {}", text("/line"));
    }
    if args.line {
        return writeln!(out, "{}", text("/line"));
    }
    let empty = Vec::new();
    for l in v["lines"].as_array().unwrap_or(&empty) {
        writeln!(out, "{}", l.as_str().unwrap_or_default())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn args(argv: &[&str]) -> BriefArgs {
        let mut full = vec!["majordomus", "brief"];
        full.extend_from_slice(argv);
        match crate::cli::Cli::try_parse_from(full)
            .expect("parses")
            .command
        {
            crate::cli::Command::Brief(a) => a,
            _ => panic!("not the brief command"),
        }
    }

    fn rendered(v: &serde_json::Value, argv: &[&str]) -> String {
        let mut out = Vec::new();
        render(&mut out, v, &args(argv)).expect("renders");
        String::from_utf8(out).expect("utf-8")
    }

    #[test]
    fn each_shape_prints_what_it_names() {
        let v = json!({
            "line": "I1 @main | peers 1/1b | mesh off | prs ?",
            "lines": ["session      a", "peers        b"],
            "keep_awake": "Keep yourself awake.",
        });
        assert_eq!(rendered(&v, &[]), "session      a\npeers        b\n");
        assert_eq!(
            rendered(&v, &["--line"]),
            "I1 @main | peers 1/1b | mesh off | prs ?\n"
        );
        assert_eq!(
            rendered(&v, &["--keep-awake"]),
            "Keep yourself awake.\nNow: I1 @main | peers 1/1b | mesh off | prs ?\n"
        );
        let json_out = rendered(&v, &["--format", "json"]);
        let back: serde_json::Value = serde_json::from_str(&json_out).expect("one JSON document");
        assert_eq!(back, v);
    }
}
