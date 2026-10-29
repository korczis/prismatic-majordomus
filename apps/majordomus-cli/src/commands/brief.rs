//! `majordomus brief`: what surrounds this session, on the command line (ADR 0136).
//!
//! The capability composes; this renders. `brief.session` asks the server holding this
//! checkout's lease, so what a person reads here is the board and the mesh of that server
//! and not the empty board of this process.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::capability::Context;
use crate::cli::{BriefArgs, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus brief`.
pub fn run(args: BriefArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    answer(&app.context, &args, &mut stdout.lock()).map(|()| 0)
}

/// Ask the capability exposed as `majordomus brief` and write its answer to `out`.
fn answer(ctx: &Context, args: &BriefArgs, out: &mut impl Write) -> Result<()> {
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
    // the caller's provider session, as the shell lifecycle resolves it, when none is given:
    // a brief about the checkout's pointer is not necessarily about this session
    let session = args.session.clone().or_else(|| {
        crate::capability::builtin::brief::provider_session_from_env(|k| std::env::var(k).ok())
    });
    if let Some(s) = &session {
        input["session"] = json!(s);
    }
    if let Some(s) = &args.since {
        input["since"] = json!(s);
    }
    if let Some(b) = &args.background_json {
        let tasks: serde_json::Value = serde_json::from_str(b).map_err(|e| Error::Protocol {
            reason: format!("--background-json is not a JSON array of tasks: {e}"),
        })?;
        input["background"] = tasks;
    }
    let v = ctx.execute(&id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    render(out, &v, args).map_err(Error::Transport)
}

/// The answer in the shape asked for.
fn render(out: &mut impl Write, v: &serde_json::Value, args: &BriefArgs) -> std::io::Result<()> {
    let text = |p: &str| v.pointer(p).and_then(|x| x.as_str()).unwrap_or_default();
    if args.format == OutputFormat::Json {
        // a JSON value always serialises: the only failure left is the writer's
        serde_json::to_writer_pretty(&mut *out, v)?;
        return writeln!(out);
    }
    if args.keep_awake {
        writeln!(out, "{}", text("/keep_awake"))?;
        return writeln!(out, "Now: {}", text("/line"));
    }
    if args.report {
        return writeln!(out, "{}", text("/report/prompt"));
    }
    if args.line {
        return writeln!(out, "{}", text("/line"));
    }
    // the header: when, by whom, in how long, under which contract
    if !text("/at").is_empty() {
        let ms = v.pointer("/answered_by/elapsed_ms").and_then(|x| x.as_u64()).unwrap_or(0);
        writeln!(
            out,
            "majordomus brief  {}  {}{}  ({:.1} s)  report {} #{}",
            text("/at"),
            text("/answered_by/process"),
            match text("/answered_by/url") {
                "" => String::new(),
                u => format!(" {u}"),
            },
            ms as f64 / 1000.0,
            text("/report/contract"),
            text("/report/fingerprint"),
        )?;
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
    use crate::capability::CapabilityError;
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
            "report": { "prompt": "Majordomus report owed: x" },
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
        assert_eq!(rendered(&v, &["--report"]), "Majordomus report owed: x\n");
        let json_out = rendered(&v, &["--format", "json"]);
        let back: serde_json::Value = serde_json::from_str(&json_out).expect("one JSON document");
        assert_eq!(back, v);
        assert!(json_out.ends_with("}\n"), "one document, one line end");
    }

    /// A writer that refuses every byte: a closed pipe.
    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the reader went away",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Every shape reports a writer that refuses it, rather than printing half an answer.
    #[test]
    fn a_closed_output_is_an_error_in_every_shape() {
        let v = json!({
            "line": "I1 @main",
            "lines": ["session      a"],
            "keep_awake": "Keep yourself awake.",
            "report": { "prompt": "Majordomus report owed: x" },
        });
        for argv in [
            &[][..],
            &["--line"][..],
            &["--keep-awake"][..],
            &["--report"][..],
            &["--format", "json"][..],
        ] {
            let e = render(&mut Closed, &v, &args(argv)).expect_err("nothing can be written");
            assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{argv:?}");
        }
    }

    /// A writer that keeps its first `n` writes and refuses the rest.
    struct ClosesAfter(usize, Vec<u8>);

    impl Write for ClosesAfter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.0 == 0 {
                return Closed.write(buf);
            }
            self.0 -= 1;
            self.1.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_keep_awake_cut_after_its_text_still_fails() {
        let v = json!({ "line": "I1 @main", "keep_awake": "Keep yourself awake." });
        // the text and its line end are written; the `Now:` line is refused
        let mut out = ClosesAfter(2, Vec::new());
        let e = render(&mut out, &v, &args(&["--keep-awake"])).expect_err("the second line");
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe);
        assert_eq!(String::from_utf8_lossy(&out.1), "Keep yourself awake.\n");
    }

    #[test]
    fn a_directory_that_is_no_repository_is_refused_before_anything_is_asked() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let path = dir.path().to_string_lossy().to_string();
        assert!(run(args(&["--repo", &path])).is_err());
    }

    #[test]
    fn a_registry_without_the_brief_says_so() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let bare = Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(
                crate::capability::CapabilityRegistry::builder()
                    .build()
                    .expect("an empty registry"),
            ),
        );
        match answer(&bare, &args(&[]), &mut Vec::new()) {
            Err(Error::Protocol { reason }) => {
                assert_eq!(reason, "no capability is exposed as `majordomus brief`")
            }
            other => panic!("expected a protocol error, got {other:?}"),
        }
    }

    #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct Asked {}

    impl crate::capability::BenchmarkCases for Asked {
        fn benchmark_cases(
            _: &crate::capability::CaseContext<'_>,
        ) -> Vec<crate::capability::NamedCase<Self>> {
            vec![crate::capability::NamedCase::new("default", Asked {})]
        }
    }

    #[derive(serde::Serialize, schemars::JsonSchema)]
    struct Never {}

    fn refuses(_: &Context, _: Asked) -> std::result::Result<Never, CapabilityError> {
        Err(CapabilityError::Refused("no brief today".into()))
    }

    /// The capability's own refusal reaches the person, and nothing is printed.
    #[test]
    fn a_brief_that_is_refused_is_reported() {
        use crate::capability::model::{CliExposure, Exposure, Stability};
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_builtin(vec![crate::capability! {
                id: "test.brief",
                title: "A brief that refuses",
                description: "Refuses every call.",
                input: Asked,
                output: Never,
                stability: Stability::Experimental,
                exposure: Exposure {
                    cli: Some(CliExposure { path: vec!["brief".into()] }),
                    ..Exposure::default()
                },
                tags: [],
                handler: refuses,
            }])
            .build()
            .expect("a registry of one");
        let ctx = Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(registry),
        );
        let mut out = Vec::new();
        match answer(&ctx, &args(&["--session", "s-1"]), &mut out) {
            Err(Error::Protocol { reason }) => {
                assert!(reason.contains("no brief today"), "{reason}")
            }
            other => panic!("expected a protocol error, got {other:?}"),
        }
        assert!(out.is_empty(), "a refusal printed something");
    }

    /// Without `--this-checkout` the brief covers every board, and the answer is written
    /// whole to the writer it is given.
    #[test]
    fn the_brief_of_every_board_is_asked_and_written() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let mut out = Vec::new();
        answer(
            &ctx,
            &args(&["--format", "json", "--session", "no-such-session"]),
            &mut out,
        )
        .expect("the brief");
        let b: serde_json::Value = serde_json::from_slice(&out).expect("one JSON document");
        assert_eq!(b["answered_by"]["process"], "this_process", "{b}");
        assert!(b["peers"]["boards"].as_u64().is_some(), "{b}");
        let e = answer(&ctx, &args(&["--line"]), &mut Closed).expect_err("a closed output");
        assert!(matches!(e, Error::Transport(_)), "{e:?}");
    }
}
