//! `majordomus evidence`: the claims matrix against the runs actually recorded for it.
//!
//! Every subcommand runs a capability of the `evidence` module through the one executor, so
//! the answer a person reads here and the answer a client reads over MCP or HTTP are the
//! same answer rendered twice, not two derivations that happen to agree today.
//!
//! The text rendering is deliberately blunt about the distinction the whole subsystem
//! exists for: `proven` and `inputs unchanged` are printed differently, and the second one
//! says what it means, because a column that showed both as a tick would be the badge whose
//! derivation cannot be inspected.

use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{EvidenceArgs, EvidenceCommand, OutputFormat};
use crate::error::{Error, Result};

/// The exit code when `--check` finds a claim the evidence does not support.
pub const EXIT_UNSUPPORTED: u8 = 10;

/// Run `majordomus evidence`.
pub fn run(args: EvidenceArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match args.command {
        EvidenceCommand::Show {
            state,
            status,
            findings,
            check,
            presented,
            presented_tree,
        } => {
            // a presented commit is judged from the ledger it holds, and the runs the working
            // ledger holds beside it can only withhold `proven`: the capability reads both,
            // and an absent revision or tree is the null an optional input reads as absent
            let mut input = json!({
                "findings_only": findings,
                "presented": presented,
                "presented_tree": presented_tree,
            });
            if let Some(s) = &state {
                input["state"] = json!(s);
            }
            if let Some(s) = &status {
                input["status"] = json!(s);
            }
            let v = execute(ctx, &["evidence", "show"], input)?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => show_text(&mut out, &v)?,
            }
            if check {
                let unsupported = !v["findings"].as_array().map(Vec::is_empty).unwrap_or(true);
                // An empty finding list over an index that lost entries is not a pass; it
                // is the absence of an answer. `--check` must not spell the two the same.
                let partial = !v["subject"]["complete"].as_bool().unwrap_or(false);
                if unsupported || partial {
                    return Ok(EXIT_UNSUPPORTED);
                }
            }
            Ok(0)
        }

        EvidenceCommand::Claim { id } => {
            let v = execute(ctx, &["evidence", "claim"], json!({ "claim": id }))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => claim_text(&mut out, &v)?,
            }
            Ok(0)
        }

        EvidenceCommand::Proves { id } => {
            let v = execute(ctx, &["evidence", "proves"], json!({ "test": id }))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => test_text(&mut out, &v)?,
            }
            Ok(0)
        }

        EvidenceCommand::Record {
            suite,
            crate_output,
            origin,
            provenance,
            coverage,
            ledger,
            run_record,
        } => {
            let mut input = json!({});
            if !provenance.is_empty() {
                input["provenance"] = json!(provenance);
            }
            if let Some(p) = &coverage {
                input["coverage"] = json!(p.to_string_lossy());
            }
            if let Some(l) = &ledger {
                input["ledger"] = json!(l);
            }
            if let Some(p) = &run_record {
                input["run_record"] = json!(p.to_string_lossy());
            }
            if let Some(p) = &suite {
                input["suite"] = json!(p.to_string_lossy());
            }
            if let Some(p) = &crate_output {
                input["crate_output"] = json!(p.to_string_lossy());
            }
            if let Some(o) = &origin {
                input["origin"] = json!(o);
            }
            let v = execute(ctx, &["evidence", "record"], input)?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => {
                    return record_text(&mut out, &v, run_record.as_deref()).map(|()| 0)
                }
            }
            Ok(0)
        }
        EvidenceCommand::Stamp {
            producer,
            report,
            exclude,
            out: file,
        } => {
            let mut input = json!({ "exclude": exclude });
            if let Some(p) = &producer {
                input["producer"] = json!(p);
            }
            if let Some(p) = &report {
                input["report"] = json!(p.to_string_lossy());
            }
            let v = execute(ctx, &["evidence", "stamp"], input)?;
            if let Some(f) = &file {
                if let Some(dir) = f.parent().filter(|d| !d.as_os_str().is_empty()) {
                    std::fs::create_dir_all(dir).map_err(Error::Transport)?;
                }
                std::fs::write(f, format!("{}\n", pretty(&v))).map_err(Error::Transport)?;
            }
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => stamp_text(&mut out, &v, file.as_deref())?,
            }
            Ok(0)
        }
    }
}

// ---------------------------------------------------------------- text

/// A recording, one line per fact: what reached the ledger, against what, where, what the
/// reports held that no claim can name yet, and what they named that this repository lacks.
fn record_text(out: &mut impl Write, v: &Value, run_record: Option<&Path>) -> Result<()> {
    writeln!(
        out,
        "recorded     {} execution(s), {} passing",
        v["recorded"], v["passed"]
    )
    .map_err(Error::Transport)?;
    writeln!(
        out,
        "against      {} ({})",
        short(v["commit"].as_str().unwrap_or("?")),
        v["working_tree"].as_str().unwrap_or("?")
    )
    .map_err(Error::Transport)?;
    writeln!(out, "ledger       {}", v["ledger"].as_str().unwrap_or("?"))
        .map_err(Error::Transport)?;
    let r = &v["run_record"];
    writeln!(out, "run          {}", r["id"].as_str().unwrap_or("?")).map_err(Error::Transport)?;
    let measured: Vec<String> = r["provenance"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let producer = p["producer"].as_str().unwrap_or("?");
            format!("{producer} ({})", p["working_tree"].as_str().unwrap_or("?"))
        })
        .collect();
    if measured.is_empty() {
        writeln!(out, "measured     nothing: the tree is recorded as unknown")
    } else {
        writeln!(out, "measured     {}", measured.join(", "))
    }
    .map_err(Error::Transport)?;
    for a in r["absent"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "absent       {} ({})",
            a["producer"].as_str().unwrap_or("?"),
            a["reason"].as_str().unwrap_or("?")
        )
        .map_err(Error::Transport)?;
    }
    // what the reports held that no claim can name yet, listed, never hidden
    for d in v["dropped"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "dropped      {} ({})",
            d["what"].as_str().unwrap_or("?"),
            d["reason"].as_str().unwrap_or("?")
        )
        .map_err(Error::Transport)?;
    }
    for u in v["unknown"].as_array().into_iter().flatten() {
        writeln!(out, "unknown      {} (no such test here)", u).map_err(Error::Transport)?;
    }
    if let Some(p) = run_record {
        writeln!(out, "run record   {}", p.display()).map_err(Error::Transport)?;
    }
    Ok(())
}

/// A measurement, one line per fact: what was stamped at which commit and tree, what was
/// excluded, the report it names, and where it was written.
fn stamp_text(out: &mut impl Write, v: &Value, written: Option<&Path>) -> Result<()> {
    let commit = v["commit"].as_str().unwrap_or("?");
    writeln!(
        out,
        "stamped      {} at {} ({})",
        v["producer"].as_str().unwrap_or("a run"),
        commit.get(..8).unwrap_or(commit),
        v["working_tree"].as_str().unwrap_or("?")
    )
    .map_err(Error::Transport)?;
    let excluded: Vec<&str> = v["excluded"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !excluded.is_empty() {
        writeln!(out, "excluded     {}", excluded.join(", ")).map_err(Error::Transport)?;
    }
    if let Some(r) = v.get("report") {
        let digest = r["digest"].as_str().unwrap_or("?");
        writeln!(
            out,
            "report       {} {}",
            r["path"].as_str().unwrap_or("?"),
            digest.get(..19).unwrap_or(digest)
        )
        .map_err(Error::Transport)?;
    }
    if let Some(p) = written {
        writeln!(out, "written      {}", p.display()).map_err(Error::Transport)?;
    }
    Ok(())
}

fn show_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    let w =
        |o: &mut std::io::StdoutLock<'_>, s: String| writeln!(o, "{s}").map_err(Error::Transport);

    w(
        out,
        format!(
            "head         {} ({})\n{}",
            short(v["head"].as_str().unwrap_or("unknown")),
            v["working_tree"].as_str().unwrap_or("?"),
            judged_at(v).join("\n")
        ),
    )?;
    let l = &v["ledger"];
    w(
        out,
        format!(
            "ledger       {} — {} execution(s){}",
            l["path"].as_str().unwrap_or("?"),
            l["executions"],
            l["newest"]
                .as_str()
                .map(|n| format!(", newest {n}"))
                .unwrap_or_default()
        ),
    )?;
    if !l["present"].as_bool().unwrap_or(false) {
        w(
            out,
            "             nothing has been recorded; every claim reads `not run`, which is true"
                .to_string(),
        )?;
    }
    w(out, String::new())?;

    let sub = &v["subject"];
    if !sub["complete"].as_bool().unwrap_or(true) {
        w(
            out,
            format!(
                "SUBJECT      INCOMPLETE — the index excluded {} file(s), so every count below \
                 is over a smaller matrix than this repository has:",
                sub["excluded"].as_array().map_or(0, Vec::len)
            ),
        )?;
        for e in sub["excluded"].as_array().into_iter().flatten() {
            w(out, format!("             {}", e.as_str().unwrap_or("?")))?;
        }
        w(out, String::new())?;
    } else {
        w(
            out,
            format!("subject      {} claim(s), whole", sub["examined"]),
        )?;
        w(out, String::new())?;
    }

    if let Some(t) = v["totals"].as_object() {
        for (state, count) in t {
            w(out, format!("{state:<18} {count}"))?;
        }
        w(out, String::new())?;
    }

    for c in v["claims"].as_array().into_iter().flatten() {
        let state = c["state"].as_str().unwrap_or("?");
        w(
            out,
            format!(
                "{:<18} {:<12} {}{}",
                state,
                c["status"].as_str().unwrap_or("?"),
                c["id"].as_str().unwrap_or("?"),
                detail_line(c)
            ),
        )?;
        if let Some(e) = c["execution"].as_object() {
            w(
                out,
                format!(
                    "                   {} at {} · {}s · {} · {}",
                    e["outcome"].as_str().unwrap_or("?"),
                    short(e["commit"].as_str().unwrap_or("?")),
                    e["seconds"],
                    e["origin"].as_str().unwrap_or("?"),
                    e["at"].as_str().unwrap_or("?")
                ),
            )?;
        }
        for p in c["changed"].as_array().into_iter().flatten() {
            w(
                out,
                format!(
                    "                   changed since: {}",
                    p.as_str().unwrap_or("?")
                ),
            )?;
        }
    }

    let findings = v["findings"].as_array().cloned().unwrap_or_default();
    if !findings.is_empty() {
        w(out, String::new())?;
        w(
            out,
            format!(
                "{} claim(s) declare a guarantee the evidence does not support:",
                findings.len()
            ),
        )?;
        for f in &findings {
            w(
                out,
                format!(
                    "  {:<40} {}",
                    f["claim"].as_str().unwrap_or("?"),
                    f["reason"].as_str().unwrap_or("?")
                ),
            )?;
            if let Some(r) = f["reproduce"].as_str() {
                w(out, format!("  {:<40} reproduce: {r}", ""))?;
            }
        }
    }
    Ok(())
}

fn claim_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    let w =
        |o: &mut std::io::StdoutLock<'_>, s: String| writeln!(o, "{s}").map_err(Error::Transport);
    w(
        out,
        format!("claim        {}", v["id"].as_str().unwrap_or("?")),
    )?;
    w(
        out,
        format!("             {}", v["claim"].as_str().unwrap_or("?")),
    )?;
    w(
        out,
        format!("status       {}", v["status"].as_str().unwrap_or("?")),
    )?;
    w(
        out,
        format!("state        {}", v["state"].as_str().unwrap_or("?")),
    )?;
    w(
        out,
        format!("             {}", v["meaning"].as_str().unwrap_or("")),
    )?;
    for (label, key) in [
        ("source", "source"),
        ("implementation", "implementation"),
        ("test", "test_path"),
    ] {
        if let Some(s) = v[key].as_str() {
            w(out, format!("{label:<12} {s}"))?;
        }
    }
    if let Some(e) = v["execution"].as_object() {
        w(out, String::new())?;
        w(
            out,
            format!("execution    {}", e["outcome"].as_str().unwrap_or("?")),
        )?;
        w(
            out,
            format!("commit       {}", e["commit"].as_str().unwrap_or("?")),
        )?;
        w(
            out,
            format!("tree         {}", e["working_tree"].as_str().unwrap_or("?")),
        )?;
        w(out, format!("duration     {}s", e["seconds"]))?;
        w(
            out,
            format!(
                "recorded     {} ({})",
                e["at"].as_str().unwrap_or("?"),
                e["origin"].as_str().unwrap_or("?")
            ),
        )?;
        w(
            out,
            format!("digest       {}", e["digest"].as_str().unwrap_or("?")),
        )?;
    }
    for p in v["changed"].as_array().into_iter().flatten() {
        w(out, format!("changed      {}", p.as_str().unwrap_or("?")))?;
    }
    let also: Vec<&str> = v["also_proves"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !also.is_empty() {
        w(out, String::new())?;
        w(
            out,
            format!("the same test also proves {} claim(s):", also.len()),
        )?;
        for a in also {
            w(out, format!("  {a}"))?;
        }
    }
    if let Some(r) = v["reproduce"].as_str() {
        w(out, String::new())?;
        w(out, format!("reproduce    {r}"))?;
    }
    Ok(())
}

fn test_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    let w =
        |o: &mut std::io::StdoutLock<'_>, s: String| writeln!(o, "{s}").map_err(Error::Transport);
    w(
        out,
        format!("test         {}", v["test"].as_str().unwrap_or("?")),
    )?;
    w(
        out,
        format!("runner       {}", v["runner"].as_str().unwrap_or("?")),
    )?;
    w(
        out,
        format!(
            "source       {}{}",
            v["source"].as_str().unwrap_or("?"),
            if v["present"].as_bool().unwrap_or(false) {
                ""
            } else {
                "  (not in this checkout)"
            }
        ),
    )?;
    if let Some(e) = v["execution"].as_object() {
        w(
            out,
            format!(
                "latest       {} at {} · {}s · {}",
                e["outcome"].as_str().unwrap_or("?"),
                short(e["commit"].as_str().unwrap_or("?")),
                e["seconds"],
                e["at"].as_str().unwrap_or("?")
            ),
        )?;
        match v["digest_matches"].as_bool() {
            Some(true) => w(out, "digest       matches the source as recorded".into())?,
            Some(false) => w(
                out,
                "digest       DOES NOT match: the test has changed since this run".into(),
            )?,
            None => {}
        }
    } else {
        w(
            out,
            "latest       no run of this test has ever been recorded".into(),
        )?;
    }
    w(
        out,
        format!("reproduce    {}", v["reproduce"].as_str().unwrap_or("?")),
    )?;

    let proves = v["proves"].as_array().cloned().unwrap_or_default();
    w(out, String::new())?;
    w(out, format!("proves {} claim(s):", proves.len()))?;
    for c in &proves {
        w(
            out,
            format!(
                "  {:<18} {:<12} {}",
                c["state"].as_str().unwrap_or("?"),
                c["status"].as_str().unwrap_or("?"),
                c["id"].as_str().unwrap_or("?")
            ),
        )?;
    }
    Ok(())
}

/// Why a claim's state is what it is, as the line under the claim that says so: empty when
/// the state alone says it, and otherwise the detail on a line of its own.
fn detail_line(c: &Value) -> String {
    c["detail"]
        .as_str()
        .map(|d| format!("\n                   {d}"))
        .unwrap_or_default()
}

/// The line naming what the verdicts below were judged at, and — for a presented commit whose
/// checkout holds runs its own ledger does not — the line saying what those runs are for.
fn judged_at(v: &Value) -> Vec<String> {
    let p = &v["presented"];
    let tree = p["tree"].as_str().unwrap_or("unknown");
    let mut lines = vec![match p["revision"].as_str() {
        None | Some("working_tree") => format!(
            "judged at the working tree (HEAD {}, {tree})",
            twelve(v["head"].as_str().unwrap_or("unknown"))
        ),
        Some(commit) => format!("judged at {} as committed ({tree})", twelve(commit)),
    }];
    let held: Vec<&str> = p["uncommitted"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !held.is_empty() {
        lines.push(format!(
            "             the working ledger holds executions this commit does not ({}), read \
             only to withhold `proven`",
            held.join(", ")
        ));
    }
    lines
}

/// The first twelve characters of a commit: the spelling the judged line and every detail
/// sentence use, so a reader can match one against the other.
fn twelve(commit: &str) -> String {
    commit.chars().take(12).collect()
}

// ---------------------------------------------------------------- plumbing

/// The first eight characters of a commit, for a line a person reads. Never used as an
/// identity: the full object name is what the payload carries.
fn short(commit: &str) -> String {
    commit.chars().take(8).collect()
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
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

    /// The first line says what was judged, and a presented commit whose checkout holds runs
    /// its own ledger does not says what those runs are read for.
    #[test]
    fn the_report_says_what_it_was_judged_at() {
        let here = json!({
            "head": "abcdef0123456789abcdef0123456789abcdef01",
            "presented": { "revision": "working_tree", "tree": "dirty" }
        });
        assert_eq!(
            judged_at(&here),
            ["judged at the working tree (HEAD abcdef012345, dirty)"]
        );

        let commit = json!({
            "presented": {
                "revision": "0123456789abcdef0123456789abcdef01234567",
                "tree": "clean",
                "uncommitted": ["suite:01_alpha", "suite:02_beta"]
            }
        });
        let lines = judged_at(&commit);
        assert_eq!(lines[0], "judged at 0123456789ab as committed (clean)");
        assert!(
            lines[1].contains("(suite:01_alpha, suite:02_beta)"),
            "{}",
            lines[1]
        );
        assert!(
            lines[1].contains("only to withhold `proven`"),
            "{}",
            lines[1]
        );
    }

    /// A recording prints one `dropped` line per entry, with its reason, after the ledger and
    /// before what the reports named that this repository lacks; one with nothing dropped
    /// prints no such line.
    #[test]
    fn a_recording_lists_what_it_dropped_after_the_ledger() {
        let render = |v: Value| {
            let mut buf = Vec::new();
            record_text(&mut buf, &v, None).unwrap();
            String::from_utf8(buf).unwrap()
        };
        let text = render(json!({
            "recorded": 1,
            "passed": 1,
            "commit": "0123456789abcdef0123456789abcdef01234567",
            "working_tree": "clean",
            "ledger": ".ai/repo/evidence/ledger.json",
            "unknown": ["crate:ghost"],
            "dropped": [
                { "producer": "crate", "what": "unittests src/lib.rs", "reason": "unit" },
                { "producer": "crate", "what": "doc-tests majordomus_cli", "reason": "doc" }
            ],
            "run_record": {
                "id": "local:0123456789ab:20260926T120000Z",
                "provenance": [{ "producer": "crate", "working_tree": "clean" }],
                "absent": [{ "producer": "suite", "reason": "no suite report was given" }]
            }
        }));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                "recorded     1 execution(s), 1 passing",
                "against      01234567 (clean)",
                "ledger       .ai/repo/evidence/ledger.json",
                "run          local:0123456789ab:20260926T120000Z",
                "measured     crate (clean)",
                "absent       suite (no suite report was given)",
                "dropped      unittests src/lib.rs (unit)",
                "dropped      doc-tests majordomus_cli (doc)",
                "unknown      \"crate:ghost\" (no such test here)",
            ]
        );

        let none = render(json!({ "recorded": 0, "passed": 0, "commit": "c", "ledger": "l" }));
        assert!(!none.contains("dropped"), "{none}");
    }

    /// A writer that takes whole lines until it holds `lines` of them, then refuses every
    /// write, and counts the writes it was asked for after its first refusal.
    struct ClosesAfter {
        lines: usize,
        held: Vec<u8>,
        refused: usize,
    }

    impl Write for ClosesAfter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.held.iter().filter(|&&b| b == b'\n').count() >= self.lines {
                self.refused += 1;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "the reader went away",
                ));
            }
            self.held.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A line that cannot be written ends the recording's output with a transport error at
    /// that line: nothing after it is attempted, and nothing before it is lost.
    #[test]
    fn a_recording_whose_output_cannot_be_written_says_so_at_the_first_line() {
        let v = json!({
            "recorded": 1,
            "passed": 1,
            "commit": "0123456789abcdef0123456789abcdef01234567",
            "working_tree": "clean",
            "ledger": ".ai/repo/evidence/ledger.json",
            "unknown": ["crate:ghost"],
            "dropped": [{ "producer": "crate", "what": "doc-tests x", "reason": "doc" }]
        });
        let mut all = ClosesAfter {
            lines: usize::MAX,
            held: Vec::new(),
            refused: 0,
        };
        record_text(&mut all, &v, Some(Path::new("run.json"))).unwrap();
        let total = String::from_utf8(all.held).unwrap().lines().count();
        assert_eq!(
            total, 8,
            "recorded, against, ledger, run, measured, one dropped, one unknown, run record"
        );

        for lines in 0..total {
            let mut out = ClosesAfter {
                lines,
                held: Vec::new(),
                refused: 0,
            };
            let got = record_text(&mut out, &v, Some(Path::new("run.json")));
            assert!(
                matches!(got, Err(Error::Transport(_))),
                "the write of line {} was not reported: {got:?}",
                lines + 1
            );
            assert_eq!(
                out.refused,
                1,
                "a write was attempted after line {} failed",
                lines + 1
            );
            let held = String::from_utf8(out.held).unwrap();
            assert_eq!(held.lines().count(), lines, "{held}");
        }
    }

    /// `evidence record` in its text format runs end to end through the command: the crate
    /// run's binary is recorded, and the command answers with success.
    #[test]
    fn a_text_recording_runs_through_the_command() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(repo.root())
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
        };
        std::fs::create_dir_all(repo.root().join("apps/majordomus-cli/tests")).unwrap();
        std::fs::write(
            repo.root().join("apps/majordomus-cli/tests/why.rs"),
            "// why\n",
        )
        .unwrap();
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "fixture"]);

        let reports = tempfile::tempdir().unwrap();
        let log = reports.path().join("crate.log");
        std::fs::write(
            &log,
            "     Running tests/why.rs (target/debug/deps/why-1)\n\
             test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n\
                Doc-tests majordomus_cli\n\
             test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
        )
        .unwrap();
        let args = EvidenceArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(repo.root().to_path_buf()),
                share: Some(crate::synthetic::crate_share()),
                ..Default::default()
            },
            command: EvidenceCommand::Record {
                suite: None,
                crate_output: Some(log),
                origin: None,
                provenance: vec![],
                coverage: None,
                ledger: None,
                run_record: None,
            },
            format: OutputFormat::Text,
        };
        assert_eq!(run(args).unwrap(), 0);
        let ledger = crate::evidence::Ledger::load(repo.root()).unwrap();
        let tests: Vec<&str> = ledger.executions.iter().map(|e| e.test.as_str()).collect();
        assert_eq!(
            tests,
            ["crate:why"],
            "the binary, and nothing the output dropped"
        );
    }

    /// A claim whose state needs a reason prints it on the line under the claim, and one
    /// whose state says it all prints nothing more.
    #[test]
    fn a_claim_prints_its_detail_when_it_has_one() {
        assert_eq!(detail_line(&json!({ "state": "not_run" })), "");
        let d = detail_line(&json!({ "state": "not_run", "detail": "the test declined to run" }));
        assert_eq!(d, "\n                   the test declined to run");
    }

    /// A measurement prints what was stamped at which commit and tree, what was excluded,
    /// the report and where it was written; a bare measurement prints only the first line.
    #[test]
    fn a_stamp_prints_one_line_per_fact() {
        let render = |v: Value, written: Option<&Path>| {
            let mut buf = Vec::new();
            stamp_text(&mut buf, &v, written).unwrap();
            String::from_utf8(buf).unwrap()
        };
        let text = render(
            json!({
                "producer": "suite",
                "commit": "0123456789abcdef0123456789abcdef01234567",
                "working_tree": "clean",
                "excluded": ["dist", "suite.tsv"],
                "report": { "path": "suite.tsv", "digest": "sha256:abcdef0123456789abcdef", "bytes": 3 }
            }),
            Some(Path::new("p.json")),
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                "stamped      suite at 01234567 (clean)",
                "excluded     dist, suite.tsv",
                "report       suite.tsv sha256:abcdef012345",
                "written      p.json",
            ]
        );
        let bare = render(json!({ "commit": "0123", "working_tree": "dirty" }), None);
        assert_eq!(bare, "stamped      a run at 0123 (dirty)\n");
    }
}
