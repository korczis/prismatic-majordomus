//! `majordomus knowledge`: what the knowledge deriver left for review, and whether it is
//! still writing, rendered for a terminal.
//!
//! Every subcommand runs a capability of the `knowledge_base` module through the one
//! executor, so the answer a person reads here and the answer a client reads over MCP or
//! HTTP are the same answer rendered twice, never two derivations that happen to agree.
//! This file renders and nothing else: which records are candidates, how a reference
//! resolves and whether the writer has stopped are decided in the capability, and
//! `--format json` prints its value verbatim.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{KnowledgeArgs, KnowledgeCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus knowledge`.
pub fn run(args: KnowledgeArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    answer(&app.context, &args.command, args.format, &mut stdout.lock())
}

/// Answer one `knowledge` subcommand from `ctx`, writing it to `out` in `format`.
fn answer<W: Write>(
    ctx: &crate::capability::Context,
    command: &KnowledgeCommand,
    format: OutputFormat,
    out: &mut W,
) -> Result<u8> {
    type Text<W> = fn(&mut W, &Value) -> Result<()>;
    let (path, input, text): (&[&str], Value, Text<W>) = match command {
        KnowledgeCommand::Candidates => (&["knowledge", "candidates"], json!({}), candidates_text),
        KnowledgeCommand::Record { id } => {
            (&["knowledge", "record"], json!({ "id": id }), record_text)
        }
        KnowledgeCommand::Status => (&["knowledge", "status"], json!({}), status_text),
    };
    let v = execute(ctx, path, input)?;
    match format {
        // `{:#}` is serde_json's own pretty writer, the one `to_string_pretty` runs, so the
        // bytes are the same; serialising a `Value` cannot fail, so there is no fallback
        OutputFormat::Json => writeln!(out, "{v:#}").map_err(Error::Transport)?,
        OutputFormat::Text => text(out, &v)?,
    }
    Ok(0)
}

// ---------------------------------------------------------------- text

fn w<W: Write>(out: &mut W, s: String) -> Result<()> {
    writeln!(out, "{s}").map_err(Error::Transport)
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

fn findings<W: Write>(out: &mut W, v: &Value) -> Result<()> {
    for f in v["findings"].as_array().into_iter().flatten() {
        w(out, format!("finding      {}", f.as_str().unwrap_or("")))?;
    }
    Ok(())
}

fn candidates_text<W: Write>(out: &mut W, v: &Value) -> Result<()> {
    let cap = match v["cap"].as_u64() {
        Some(c) => format!(", cap {c}"),
        None => String::new(),
    };
    w(
        out,
        format!(
            "candidates   {} awaiting review — {} on {}, {} whose episode is not known here{cap}",
            v["total"],
            v["on_this_branch"],
            s(v, "branch"),
            v["unattributed"]
        ),
    )?;
    for c in v["candidates"].as_array().into_iter().flatten() {
        w(
            out,
            format!(
                "{}  {}  {}  {}  {} ({})  {}",
                s(c, "id"),
                s(c, "class"),
                s(c, "date"),
                c["branch"].as_str().unwrap_or("-"),
                s(c, "freshness"),
                s(c, "freshness_reason"),
                s(c, "title"),
            ),
        )?;
    }
    findings(out, v)
}

fn resolution(r: &Value) -> String {
    let mut line = format!("{}  {}", s(r, "reference"), s(r, "resolution"));
    if let Some(t) = r["target"].as_str() {
        line.push_str(&format!("  -> {t}"));
    }
    if !s(r, "reason").is_empty() {
        line.push_str(&format!("  ({})", s(r, "reason")));
    }
    line
}

fn record_text<W: Write>(out: &mut W, v: &Value) -> Result<()> {
    w(out, format!("id           {}", s(v, "id")))?;
    w(out, format!("path         {}", s(v, "path")))?;
    w(
        out,
        format!(
            "record       {} {} {} {}  {}",
            s(v, "class"),
            s(v, "status"),
            s(v, "epistemics"),
            s(v, "origin"),
            s(v, "date")
        ),
    )?;
    w(out, format!("title        {}", s(v, "title")))?;
    if let Some(by) = v["superseded_by"].as_str() {
        w(out, format!("superseded   by {by}"))?;
    }
    for r in v["derived_from"].as_array().into_iter().flatten() {
        w(out, format!("derived_from {}", resolution(r)))?;
    }
    for r in v["relations"].as_array().into_iter().flatten() {
        w(
            out,
            format!("relation     {}  {}", s(r, "relation_type"), resolution(r)),
        )?;
    }
    w(
        out,
        format!(
            "resolved     {}",
            if v["resolved"].as_bool().unwrap_or(false) {
                "every reference answers"
            } else {
                "a reference dangles; `majordomus knowledge check` names it"
            }
        ),
    )
}

fn mark(v: &Value) -> String {
    match v.as_object() {
        Some(m) => format!(
            "{} at {}",
            m.get("episode").and_then(Value::as_str).unwrap_or("-"),
            m.get("ts").and_then(Value::as_str).unwrap_or("-")
        ),
        None => "none".to_string(),
    }
}

fn status_text<W: Write>(out: &mut W, v: &Value) -> Result<()> {
    w(
        out,
        format!("present      {}  ({})", v["present"], s(v, "branch")),
    )?;
    w(out, format!("last derived {}", mark(&v["last_derived"])))?;
    w(out, format!("last closed  {}", mark(&v["last_closed"])))?;
    w(
        out,
        format!(
            "underived    {} closed episode(s) no derivation names",
            v["closed_without_derivation"]
        ),
    )?;
    w(
        out,
        format!(
            "candidates   {} awaiting review, {} on this branch",
            v["candidates"], v["on_this_branch"]
        ),
    )?;
    w(
        out,
        format!(
            "freshness    {} — {}",
            s(v, "freshness"),
            s(v, "freshness_reason")
        ),
    )?;
    w(
        out,
        format!(
            "writer       {}",
            match (
                v["judged"].as_bool().unwrap_or(false),
                v["stopped_writer"].as_bool().unwrap_or(false)
            ) {
                (false, _) => "not judged",
                (true, true) => "STOPPED",
                (true, false) => "writing",
            }
        ),
    )?;
    w(
        out,
        format!(
            "switches     knowledge_on_end {}  knowledge_on_compact {}",
            v["knowledge_on_end"], v["knowledge_on_compact"]
        ),
    )?;
    findings(out, v)
}

/// Run one of this module's capabilities by the command line it declares, so that the
/// command line cannot reach a capability it does not claim.
fn execute(ctx: &crate::capability::Context, path: &[&str], input: Value) -> Result<Value> {
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(id, input).map_err(map)
}

fn map(e: CapabilityError) -> Error {
    match e {
        // A record the repository does not hold is a missing thing, not a malformed
        // request, and the exit code has to say which.
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Protocol { reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_renders_its_episode_and_time_and_absence_renders_as_none() {
        assert_eq!(
            mark(&json!({ "ts": "2026-09-12T10:00:00Z", "episode": "e1" })),
            "e1 at 2026-09-12T10:00:00Z"
        );
        assert_eq!(mark(&Value::Null), "none");
    }

    #[test]
    fn a_resolution_line_carries_the_target_or_the_reason() {
        assert_eq!(
            resolution(
                &json!({ "reference": "session:e1", "resolution": "object", "target": "majordomus://session/e1" })
            ),
            "session:e1  object  -> majordomus://session/e1"
        );
        assert_eq!(
            resolution(
                &json!({ "reference": "task:none", "resolution": "missing", "reason": "not a task" })
            ),
            "task:none  missing  (not a task)"
        );
    }

    /// A sink that accepts `lines` complete lines and refuses every write after them.
    struct FailAfter {
        lines: usize,
    }

    impl Write for FailAfter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.lines == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "closed",
                ));
            }
            self.lines -= buf.iter().filter(|b| **b == b'\n').count().min(self.lines);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    type Render = fn(&mut Vec<u8>, &Value) -> Result<()>;

    fn rendered(render: Render, v: &Value) -> String {
        let mut out = Vec::new();
        render(&mut out, v).unwrap();
        String::from_utf8(out).unwrap()
    }

    /// Every line a renderer writes is a write that can fail; a failure at any of them is a
    /// transport error, and none is swallowed into a shorter answer that looks complete.
    fn fails_at_every_line(
        render: fn(&mut FailAfter, &Value) -> Result<()>,
        v: &Value,
        lines: usize,
    ) {
        for n in 0..lines {
            match render(&mut FailAfter { lines: n }, v) {
                Err(Error::Transport(_)) => {}
                other => panic!("a sink closed after {n} line(s) answered {other:?}"),
            }
        }
        render(&mut FailAfter { lines }, v).expect("every line fits");
    }

    fn candidates() -> Value {
        json!({
            "total": 2, "on_this_branch": 1, "branch": "master", "unattributed": 1, "cap": 40,
            "candidates": [
                { "id": "e1-a", "class": "convention", "date": "2026-01-01", "branch": "master",
                  "freshness": "fresh", "freshness_reason": "young", "title": "A" },
                { "id": "e2-b", "class": "fact", "date": "2026-01-02",
                  "freshness": "stale", "freshness_reason": "old", "title": "B" }
            ],
            "findings": ["one finding"]
        })
    }

    #[test]
    fn the_queue_names_its_cap_and_every_candidate_and_a_missing_branch_is_a_dash() {
        assert_eq!(
            rendered(candidates_text, &candidates()),
            "candidates   2 awaiting review — 1 on master, 1 whose episode is not known here, cap 40\n\
             e1-a  convention  2026-01-01  master  fresh (young)  A\n\
             e2-b  fact  2026-01-02  -  stale (old)  B\n\
             finding      one finding\n"
        );
        let uncapped = json!({ "total": 0, "on_this_branch": 0, "branch": "x", "unattributed": 0 });
        assert_eq!(
            rendered(candidates_text, &uncapped),
            "candidates   0 awaiting review — 0 on x, 0 whose episode is not known here\n"
        );
        fails_at_every_line(candidates_text, &candidates(), 4);
    }

    fn record() -> Value {
        json!({
            "id": "e1-a", "path": ".ai/repo/knowledge/candidates/e1-a.md", "class": "convention",
            "status": "candidate", "epistemics": "decided", "origin": "extracted",
            "date": "2026-01-01", "title": "A", "superseded_by": "e3-c",
            "derived_from": [{ "reference": "session:e1", "resolution": "object", "target": "u" }],
            "relations": [{ "relation_type": "relates_to", "reference": "rule:x", "resolution": "missing", "reason": "gone" }],
            "resolved": false
        })
    }

    #[test]
    fn a_record_shows_every_reference_and_whether_all_of_them_answer() {
        assert_eq!(
            rendered(record_text, &record()),
            "id           e1-a\n\
             path         .ai/repo/knowledge/candidates/e1-a.md\n\
             record       convention candidate decided extracted  2026-01-01\n\
             title        A\n\
             superseded   by e3-c\n\
             derived_from session:e1  object  -> u\n\
             relation     relates_to  rule:x  missing  (gone)\n\
             resolved     a reference dangles; `majordomus knowledge check` names it\n"
        );
        let mut whole = record();
        whole["resolved"] = json!(true);
        whole.as_object_mut().unwrap().remove("superseded_by");
        let text = rendered(record_text, &whole);
        assert!(
            text.ends_with("resolved     every reference answers\n"),
            "{text}"
        );
        assert!(!text.contains("superseded"), "{text}");
        fails_at_every_line(record_text, &record(), 8);
    }

    fn status(judged: bool, stopped: bool) -> Value {
        json!({
            "present": true, "branch": "master",
            "last_derived": { "episode": "e1", "ts": "2026-01-01T00:00:00Z" },
            "closed_without_derivation": 1, "candidates": 2, "on_this_branch": 1,
            "freshness": "stale", "freshness_reason": "old", "judged": judged,
            "stopped_writer": stopped, "knowledge_on_end": true, "knowledge_on_compact": false,
            "findings": ["the writer has stopped"]
        })
    }

    #[test]
    fn the_status_says_whether_the_writer_is_writing_stopped_or_not_judged() {
        assert_eq!(
            rendered(status_text, &status(true, true)),
            "present      true  (master)\n\
             last derived e1 at 2026-01-01T00:00:00Z\n\
             last closed  none\n\
             underived    1 closed episode(s) no derivation names\n\
             candidates   2 awaiting review, 1 on this branch\n\
             freshness    stale — old\n\
             writer       STOPPED\n\
             switches     knowledge_on_end true  knowledge_on_compact false\n\
             finding      the writer has stopped\n"
        );
        assert!(rendered(status_text, &status(true, false)).contains("writer       writing\n"));
        assert!(rendered(status_text, &status(false, true)).contains("writer       not judged\n"));
        fails_at_every_line(status_text, &status(true, true), 9);
    }

    /// The command line keeps the capability's kind of failure: a missing record is not
    /// found (exit 12), a refusal or invalid input carries its own message, anything else
    /// its description.
    #[test]
    fn a_capability_error_keeps_its_kind_on_the_command_line() {
        assert!(matches!(
            map(CapabilityError::NotFound("no record".into())),
            Error::NotFound { reason } if reason == "no record"
        ));
        for e in [
            CapabilityError::InvalidInput("bad id".into()),
            CapabilityError::Refused("bad id".into()),
        ] {
            assert!(matches!(map(e), Error::Protocol { reason } if reason == "bad id"));
        }
        let internal = CapabilityError::Internal("broke".into());
        let said = internal.to_string();
        assert!(matches!(map(internal), Error::Protocol { reason } if reason == said));
    }

    /// Each subcommand reaches its capability over a repository with no knowledge at all and
    /// prints its answer in either format; a closed sink is an error in either; a record the
    /// repository does not hold is not found; a registry that exposes no `knowledge` command
    /// is named; no repository fails before anything runs.
    #[test]
    fn each_subcommand_answers_and_every_way_it_cannot_is_an_error() {
        use crate::synthetic::SyntheticRepository;
        let repo = SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        for command in [KnowledgeCommand::Candidates, KnowledgeCommand::Status] {
            let mut out = Vec::new();
            assert_eq!(
                answer(&ctx, &command, OutputFormat::Json, &mut out).unwrap(),
                0
            );
            let v: Value = serde_json::from_slice(&out).unwrap();
            assert_eq!(
                String::from_utf8(out).unwrap(),
                format!("{}\n", serde_json::to_string_pretty(&v).unwrap())
            );
            let mut out = Vec::new();
            answer(&ctx, &command, OutputFormat::Text, &mut out).unwrap();
            assert!(!out.is_empty());
            for format in [OutputFormat::Json, OutputFormat::Text] {
                assert!(matches!(
                    answer(&ctx, &command, format, &mut FailAfter { lines: 0 }),
                    Err(Error::Transport(_))
                ));
            }
        }
        match answer(
            &ctx,
            &KnowledgeCommand::Record {
                id: "absent".into(),
            },
            OutputFormat::Text,
            &mut Vec::new(),
        ) {
            Err(e @ Error::NotFound { .. }) => assert_eq!(e.exit_code(), 12),
            other => panic!("an absent record answered {other:?}"),
        }

        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(
                crate::capability::builtin::modules()
                    .into_iter()
                    .filter(|m| m.id.as_str() != "knowledge_base")
                    .collect(),
            )
            .build()
            .unwrap();
        let bare = crate::capability::Context::new(
            std::sync::Arc::new(repo.index().unwrap()),
            std::sync::Arc::new(registry),
        );
        match answer(
            &bare,
            &KnowledgeCommand::Status,
            OutputFormat::Json,
            &mut Vec::new(),
        ) {
            Err(Error::Protocol { reason }) => {
                assert_eq!(
                    reason,
                    "no capability is exposed as `majordomus knowledge status`"
                )
            }
            other => panic!("a registry with no knowledge answered {other:?}"),
        }

        let nowhere = tempfile::tempdir().unwrap();
        let args = KnowledgeArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(nowhere.path().to_path_buf()),
                discovery: crate::cli::DiscoveryMode::Vcs,
                strict: false,
                share: None,
            },
            command: KnowledgeCommand::Status,
            format: OutputFormat::Text,
        };
        assert!(run(args).is_err());
    }
}
