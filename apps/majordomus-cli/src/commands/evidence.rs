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
use std::path::{Path, PathBuf};

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
            run,
            cases,
        } => {
            let mut input = json!({});
            if let Some(r) = &run {
                input["run"] = json!(r);
            }
            if !cases.is_empty() {
                input["cases"] = json!(cases);
            }
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
            write_stamp(&mut out, &v, file.as_deref(), args.format).map(|()| 0)
        }

        EvidenceCommand::Subject {
            key,
            presented,
            presented_tree,
        } => {
            // an absent revision or tree is the null an optional input reads as absent
            let input = json!({
                "subject": key,
                "presented": presented,
                "presented_tree": presented_tree,
            });
            let v = execute(ctx, &["evidence", "subject"], input)?;
            answer(&mut out, &v, args.format, subject_text).map(|()| 0)
        }

        EvidenceCommand::Coverage {
            from,
            presented_tree,
        } => {
            // a path a person types is read from where they typed it; the capability reads
            // a relative one against the repository, so it is handed over absolute
            let from = from.map(|p| std::path::absolute(&p).unwrap_or(p));
            let input = json!({
                "from": from.map(|p| p.to_string_lossy().into_owned()),
                "presented_tree": presented_tree,
            });
            let v = execute(ctx, &["evidence", "coverage"], input)?;
            answer(&mut out, &v, args.format, coverage_text).map(|()| 0)
        }

        EvidenceCommand::Publish {
            presented,
            presented_tree,
            from,
            run,
            out: dir,
        } => {
            // paths a person types are read from where they typed them; the capability reads
            // a relative one against the repository, so each is handed over absolute
            let absolute = |p: PathBuf| std::path::absolute(&p).unwrap_or(p);
            let input = json!({
                "presented": presented,
                "presented_tree": presented_tree,
                "from": from.map(absolute).map(|p| p.to_string_lossy().into_owned()),
                "run": run,
                "out": absolute(dir).to_string_lossy(),
            });
            let v = execute(ctx, &["evidence", "publish"], input)?;
            answer(&mut out, &v, args.format, publish_text).map(|()| 0)
        }
    }
}

/// An answer in the format asked: the document itself, or the text rendering of it.
fn answer(
    out: &mut impl Write,
    v: &Value,
    format: OutputFormat,
    text: fn(&mut dyn Write, &Value) -> std::io::Result<()>,
) -> Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{}", pretty(v)),
        OutputFormat::Text => text(out, v),
    }
    .map_err(Error::Transport)
}

/// One subject, one line per fact: what it is, what it was judged at, its verdict and the
/// totals beside it, then each route, member, mechanism and finding. Rendered into one
/// string and written once.
fn subject_text(out: &mut dyn Write, v: &Value) -> std::io::Result<()> {
    let s = |v: &Value| v.as_str().unwrap_or("?").to_string();
    let mut lines = vec![format!(
        "subject      {} ({})",
        s(&v["subject"]),
        s(&v["kind"])
    )];
    let p = &v["presented"];
    lines.push(match p["revision"].as_str() {
        Some("working_tree") => format!("judged at the working tree ({})", s(&p["tree"])),
        _ => format!(
            "judged at {} as committed ({})",
            twelve(p["revision"].as_str().unwrap_or("?")),
            s(&p["tree"])
        ),
    });
    lines.push(format!("verdict      {}", s(&v["verdict"])));
    lines.push(format!("             {}", s(&v["meaning"])));
    let totals: Vec<String> = v["totals"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(state, n)| format!("{state} {n}"))
        .collect();
    lines.push(format!("totals       {}", totals.join(", ")));
    for r in v["routes"].as_array().into_iter().flatten() {
        let what = r["test"].as_str().or(r["path"].as_str()).unwrap_or("-");
        lines.push(format!(
            "route        {:<18} {:<10} {what}",
            s(&r["state"]),
            s(&r["via"])
        ));
        if let Some(d) = r["detail"].as_str() {
            lines.push(format!("                                           {d}"));
        }
    }
    for m in v["members"].as_array().into_iter().flatten() {
        lines.push(format!(
            "member       {:<18} {}",
            s(&m["state"]),
            s(&m["subject"])
        ));
    }
    for m in v["mechanisms"].as_array().into_iter().flatten() {
        lines.push(format!("mechanism    {}", s(m)));
    }
    for f in v["subject_findings"].as_array().into_iter().flatten() {
        lines.push(format!(
            "finding      {}: {}",
            s(&f["code"]),
            s(&f["message"])
        ));
    }
    writeln!(out, "{}", lines.join("\n"))
}

/// The coverage records, one line each with its state and reason under it, after the line
/// naming the checkout they were judged against and the source they were read from. Rendered
/// into one string and written once.
fn coverage_text(out: &mut dyn Write, v: &Value) -> std::io::Result<()> {
    let s = |v: &Value| v.as_str().unwrap_or("?").to_string();
    let p = &v["presented"];
    let mut lines = vec![match p["commit"].as_str() {
        Some(c) => format!("judged at {} ({})", twelve(c), s(&p["tree"])),
        None => format!("judged at no commit: {}", s(&p["reason"])),
    }];
    lines.push(format!(
        "source       {}{}",
        s(&v["source"]),
        if v["present"].as_bool().unwrap_or(false) {
            ""
        } else {
            "  (not there: nothing has written a run record to it)"
        }
    ));
    lines.push(format!(
        "runs         {} read, {} with coverage",
        v["runs"],
        v["records"].as_array().map_or(0, Vec::len)
    ));
    for r in v["records"].as_array().into_iter().flatten() {
        let c = &r["coverage"];
        lines.push(format!(
            "{:<12} {} at {} ({}) · lines {}% · {}",
            s(&r["state"]),
            s(&r["run"]),
            twelve(c["commit"].as_str().unwrap_or("?")),
            s(&c["working_tree"]),
            c["crate"]["lines"]["percent"],
            s(&r["file"])
        ));
        lines.push(format!("             {}", s(&r["reason"])));
    }
    for u in v["unread"].as_array().into_iter().flatten() {
        lines.push(format!(
            "unread       {} ({})",
            s(&u["file"]),
            s(&u["reason"])
        ));
    }
    writeln!(out, "{}", lines.join("\n"))
}

/// A publication written: its state at the commit it presents, why, each record read with
/// where it stands and what it capped, the claims' totals, and the file. Rendered into one
/// string and written once.
fn publish_text(out: &mut dyn Write, v: &Value) -> std::io::Result<()> {
    let s = |v: &Value| v.as_str().unwrap_or("?").to_string();
    let p = &v["publication"];
    let mut lines = vec![format!(
        "published    {} at {} ({})",
        s(&p["state"]),
        twelve(p["presented"]["commit"].as_str().unwrap_or("?")),
        s(&p["presented"]["tree"])
    )];
    for r in p["reasons"].as_array().into_iter().flatten() {
        lines.push(format!("             {}", s(r)));
    }
    for r in p["records"].as_array().into_iter().flatten() {
        let behind = match r["commits_between"].as_u64() {
            Some(n) => format!(
                ", {n} commit(s) and {} changed file(s) behind",
                r["changed"].as_array().map_or(0, Vec::len)
            ),
            None => String::new(),
        };
        lines.push(format!(
            "record       {:<10} {} at {} ({}){behind}",
            s(&r["relation"]),
            s(&r["id"]),
            twelve(r["commit"].as_str().unwrap_or("?")),
            s(&r["working_tree"])
        ));
        for c in r["capped"].as_array().into_iter().flatten() {
            lines.push(format!("             caps {}", s(c)));
        }
    }
    let totals: Vec<String> = p["totals"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(state, n)| format!("{state} {n}"))
        .collect();
    lines.push(format!(
        "claims       {}: {}",
        p["claims"].as_array().map_or(0, Vec::len),
        totals.join(", ")
    ));
    for u in p["unread"].as_array().into_iter().flatten() {
        lines.push(format!(
            "unread       {} ({})",
            s(&u["file"]),
            s(&u["reason"])
        ));
    }
    if p["withheld"].as_u64().unwrap_or(0) > 0 {
        lines.push(format!(
            "withheld     {} text(s) the redactor could not make safe",
            p["withheld"]
        ));
    }
    lines.push(format!("written      {}", s(&v["file"])));
    writeln!(out, "{}", lines.join("\n"))
}

/// A measurement written to `file` when one is named, then answered on `out` in `format`;
/// a file or a line that cannot be written is a transport error.
fn write_stamp(
    out: &mut impl Write,
    v: &Value,
    file: Option<&Path>,
    format: OutputFormat,
) -> Result<()> {
    if let Some(f) = file {
        // a bare file name's parent is the empty path, which create_dir_all accepts as is
        f.parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .map_err(Error::Transport)?;
        std::fs::write(f, format!("{}\n", pretty(v))).map_err(Error::Transport)?;
    }
    match format {
        OutputFormat::Json => writeln!(out, "{}", pretty(v)).map_err(Error::Transport),
        OutputFormat::Text => stamp_text(out, v, file),
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
    let lines: Vec<String> = r["lines"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|l| {
            format!(
                "{} {} ({} of {} read)",
                l["producer"].as_str().unwrap_or("?"),
                l["stream"].as_str().unwrap_or("?"),
                l["read"],
                l["lines"]
            )
        })
        .collect();
    if !lines.is_empty() {
        writeln!(out, "lines        {}", lines.join(", ")).map_err(Error::Transport)?;
    }
    // every line no case result holds is in the run record; here, how many and why
    let mut why: Vec<String> = r["dropped_lines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| d["reason"].as_str().map(str::to_string))
        .collect();
    crate::order::canonical_strings(&mut why);
    let mut counted: Vec<(String, usize)> = Vec::new();
    for reason in why {
        match counted.last_mut() {
            Some((r, n)) if *r == reason => *n += 1,
            _ => counted.push((reason, 1)),
        }
    }
    for (reason, n) in counted {
        writeln!(out, "not read     {n} line(s): {reason}").map_err(Error::Transport)?;
    }
    if let Some(p) = run_record {
        writeln!(out, "run record   {}", p.display()).map_err(Error::Transport)?;
    }
    if let Some(p) = v["local_run"].as_str() {
        writeln!(out, "local run    {p}").map_err(Error::Transport)?;
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
                run: None,
                cases: vec![],
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

    /// A subject prints what it is, what it was judged at, its verdict and totals, then one
    /// line per route (with its detail under it), member, mechanism and finding.
    #[test]
    fn a_subject_prints_one_line_per_part() {
        let render = |v: Value| {
            let mut buf = Vec::new();
            subject_text(&mut buf, &v).unwrap();
            String::from_utf8(buf).unwrap()
        };
        let text = render(json!({
            "subject": "rule:project.alpha", "kind": "rule",
            "presented": { "revision": "0123456789abcdef0123456789abcdef01234567", "tree": "dirty" },
            "verdict": "not_run", "meaning": "nothing ran",
            "totals": { "not run": 1, "no test": 1 },
            "routes": [
                { "via": "rule", "test": "suite:01_a", "state": "not_run", "detail": "never ran" },
                { "via": "scenario", "state": "not_run" }
            ],
            "members": [{ "subject": "claim:a", "state": "proven" }],
            "mechanisms": ["scripts/gate"],
            "subject_findings": [{ "code": "dangling_member", "message": "gone" }]
        }));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "subject      rule:project.alpha (rule)");
        assert_eq!(lines[1], "judged at 0123456789ab as committed (dirty)");
        assert_eq!(lines[2], "verdict      not_run");
        assert_eq!(lines[4], "totals       not run 1, no test 1");
        assert!(lines[5].starts_with("route        not_run") && lines[5].ends_with("suite:01_a"));
        assert!(lines[6].ends_with("never ran"), "{text}");
        assert!(
            lines[7].ends_with(" -"),
            "a route with no test or path: {text}"
        );
        assert_eq!(lines[8], "member       proven             claim:a");
        assert_eq!(lines[9], "mechanism    scripts/gate");
        assert_eq!(lines[10], "finding      dangling_member: gone");

        let here = render(json!({ "presented": { "revision": "working_tree", "tree": "clean" } }));
        assert!(
            here.contains("judged at the working tree (clean)"),
            "{here}"
        );
    }

    /// The coverage lines: the checkout judged against, the source and whether it is there,
    /// the counts, each record with its reason under it, and what was not read.
    #[test]
    fn coverage_prints_each_record_with_its_reason() {
        let render = |v: Value| {
            let mut buf = Vec::new();
            coverage_text(&mut buf, &v).unwrap();
            String::from_utf8(buf).unwrap()
        };
        let text = render(json!({
            "presented": { "commit": "0123456789abcdef0123456789abcdef01234567", "tree": "clean" },
            "source": ".ai/local/evidence/runs", "present": true, "runs": 2,
            "records": [{
                "run": "r-a", "state": "stale", "reason": "measured elsewhere", "file": "a.json",
                "coverage": { "commit": "ffffffffffffffffffffffffffffffffffffffff",
                              "working_tree": "clean", "crate": { "lines": { "percent": 80.0 } } }
            }],
            "unread": [{ "file": "m.json", "reason": "not a run record" }]
        }));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                "judged at 0123456789ab (clean)",
                "source       .ai/local/evidence/runs",
                "runs         2 read, 1 with coverage",
                "stale        r-a at ffffffffffff (clean) · lines 80.0% · a.json",
                "             measured elsewhere",
                "unread       m.json (not a run record)",
            ]
        );
        let none = render(json!({
            "presented": { "commit": null, "tree": "unknown", "reason": "no git" },
            "source": "x", "present": false, "runs": 0, "records": [], "unread": []
        }));
        assert!(none.starts_with("judged at no commit: no git\n"), "{none}");
        assert!(none.contains("(not there: nothing has written"), "{none}");
    }

    /// `evidence subject` and `evidence coverage` run end to end through the command in
    /// either format, and an unknown subject is an error of the command.
    #[test]
    fn subject_and_coverage_run_through_the_command() {
        let repo = committed();
        for format in [OutputFormat::Text, OutputFormat::Json] {
            let cmd = EvidenceCommand::Subject {
                key: "capability:evidence.subject".into(),
                presented: None,
                presented_tree: None,
            };
            assert_eq!(run(evidence(&repo, cmd, format)).unwrap(), 0);
            let cmd = EvidenceCommand::Coverage {
                from: None,
                presented_tree: Some("dirty".into()),
            };
            assert_eq!(run(evidence(&repo, cmd, format)).unwrap(), 0);
        }
        let cmd = EvidenceCommand::Subject {
            key: "feature:no-such".into(),
            presented: Some("HEAD".into()),
            presented_tree: None,
        };
        let err = run(evidence(&repo, cmd, OutputFormat::Text)).unwrap_err();
        assert_eq!(err.exit_code(), 13, "{err}");
        assert!(err.to_string().contains("evidence-subjects.json"), "{err}");
        // a path a person types is resolved where they typed it, then held to the checkout
        let cmd = EvidenceCommand::Coverage {
            from: Some(repo.root().join("apps")),
            presented_tree: None,
        };
        assert_eq!(run(evidence(&repo, cmd, OutputFormat::Json)).unwrap(), 0);
        let cmd = EvidenceCommand::Coverage {
            from: Some(std::path::PathBuf::from("/")),
            presented_tree: None,
        };
        let err = run(evidence(&repo, cmd, OutputFormat::Json)).unwrap_err();
        assert!(err.to_string().contains("outside the repository"), "{err}");
    }

    /// The publication lines: the state at the commit presented, each reason, each record
    /// with where it stands and what it capped, the totals, what was not read or withheld,
    /// and the file written.
    #[test]
    fn a_publication_prints_its_state_its_records_and_its_file() {
        let render = |v: Value| {
            let mut buf = Vec::new();
            publish_text(&mut buf, &v).unwrap();
            String::from_utf8(buf).unwrap()
        };
        let text = render(json!({
            "file": "/x/evidence-publication.json",
            "publication": {
                "state": "stale",
                "presented": { "commit": "0123456789abcdef0123456789abcdef01234567", "tree": "clean" },
                "reasons": ["capped by r-a"],
                "records": [
                    { "relation": "ancestor", "id": "r-a", "commit": "ffffffffffffffffffffffffffffffffffffffff",
                      "working_tree": "clean", "commits_between": 2, "changed": ["a", "b"], "capped": ["c1"] },
                    { "relation": "unrelated", "id": "r-b", "commit": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                      "working_tree": "dirty", "changed": [], "capped": [] }
                ],
                "claims": [{ "id": "c1" }],
                "totals": { "stale": 1 },
                "unread": [{ "file": "manifest.json", "reason": "not a run record" }],
                "withheld": 1
            }
        }));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                "published    stale at 0123456789ab (clean)",
                "             capped by r-a",
                "record       ancestor   r-a at ffffffffffff (clean), 2 commit(s) and 2 changed file(s) behind",
                "             caps c1",
                "record       unrelated  r-b at eeeeeeeeeeee (dirty)",
                "claims       1: stale 1",
                "unread       manifest.json (not a run record)",
                "withheld     1 text(s) the redactor could not make safe",
                "written      /x/evidence-publication.json",
            ]
        );
    }

    /// `evidence publish` runs end to end through the command in either format, a relative
    /// destination is read from where it was typed, and one inside the checkout is an error.
    #[test]
    fn a_publication_runs_through_the_command() {
        let repo = committed();
        let away = tempfile::tempdir().unwrap();
        for format in [OutputFormat::Text, OutputFormat::Json] {
            let cmd = EvidenceCommand::Publish {
                presented: "HEAD".into(),
                presented_tree: Some("dirty".into()),
                from: Some(away.path().to_path_buf()),
                run: Some("pending".into()),
                out: away.path().join("out"),
            };
            assert_eq!(run(evidence(&repo, cmd, format)).unwrap(), 0);
        }
        let written = std::fs::read_to_string(
            away.path()
                .join("out")
                .join(crate::evidence::publication::FILE),
        )
        .unwrap();
        assert!(written.contains("\"state\": \"dirty\""), "{written}");
        let cmd = EvidenceCommand::Publish {
            presented: "HEAD".into(),
            presented_tree: None,
            from: None,
            run: None,
            out: repo.root().join("inside"),
        };
        let err = run(evidence(&repo, cmd, OutputFormat::Text)).unwrap_err();
        assert_eq!(err.exit_code(), 13, "{err}");
        assert!(err.to_string().contains("inside the checkout"), "{err}");
    }

    /// An answer that cannot be written, in either format, is a transport error.
    #[test]
    fn an_unwritable_answer_is_a_transport_error() {
        let v = json!({ "subject": "claim:a" });
        for format in [OutputFormat::Json, OutputFormat::Text] {
            let mut closed = ClosesAfter {
                lines: 0,
                held: Vec::new(),
                refused: 0,
            };
            let got = answer(&mut closed, &v, format, subject_text);
            assert!(matches!(got, Err(Error::Transport(_))), "{got:?}");
        }
    }

    /// A small repository holding one crate integration test, committed.
    fn committed() -> crate::synthetic::SyntheticRepository {
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
        repo
    }

    fn evidence(
        repo: &crate::synthetic::SyntheticRepository,
        command: EvidenceCommand,
        format: OutputFormat,
    ) -> EvidenceArgs {
        EvidenceArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(repo.root().to_path_buf()),
                share: Some(crate::synthetic::crate_share()),
                ..Default::default()
            },
            command,
            format,
        }
    }

    fn stamping(
        producer: Option<&str>,
        report: Option<&Path>,
        out: Option<&Path>,
    ) -> EvidenceCommand {
        EvidenceCommand::Stamp {
            producer: producer.map(str::to_string),
            report: report.map(Path::to_path_buf),
            exclude: vec![],
            out: out.map(Path::to_path_buf),
        }
    }

    fn recording(crate_output: Option<&Path>, provenance: Vec<String>) -> EvidenceCommand {
        EvidenceCommand::Record {
            suite: None,
            crate_output: crate_output.map(Path::to_path_buf),
            origin: None,
            provenance,
            coverage: None,
            ledger: None,
            run_record: None,
            run: None,
            cases: vec![],
        }
    }

    /// A stamp written to a file the command creates is carried by `record --provenance`
    /// into a local ledger and a run record, each written where the command was told.
    #[test]
    fn a_stamp_written_by_the_command_is_carried_into_a_recording() {
        let repo = committed();
        let reports = tempfile::tempdir().unwrap();
        let log = reports.path().join("crate.log");
        std::fs::write(
            &log,
            "     Running tests/why.rs (target/debug/deps/why-1)\n\
             test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
        )
        .unwrap();
        let measurement = reports.path().join("deep/p.json");

        let cmd = stamping(None, Some(&log), Some(&measurement));
        assert_eq!(run(evidence(&repo, cmd, OutputFormat::Text)).unwrap(), 0);
        let stamped: Value =
            serde_json::from_str(&std::fs::read_to_string(&measurement).unwrap()).unwrap();
        assert_eq!(stamped["report"]["path"], "crate.log", "{stamped}");
        let cmd = stamping(Some("crate"), None, None);
        assert_eq!(run(evidence(&repo, cmd, OutputFormat::Json)).unwrap(), 0);

        let run_record = reports.path().join("run/run.json");
        let cmd = EvidenceCommand::Record {
            suite: None,
            crate_output: Some(log),
            origin: None,
            provenance: vec![format!("crate={}", measurement.display())],
            coverage: None,
            ledger: Some("local".into()),
            run_record: Some(run_record.clone()),
            run: None,
            cases: vec![],
        };
        assert_eq!(run(evidence(&repo, cmd, OutputFormat::Json)).unwrap(), 0);
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(&run_record).unwrap()).unwrap();
        assert_eq!(v["totals"]["executions"], 1, "{v}");
        assert_eq!(v["provenance"][0]["producer"], "crate", "{v}");
        assert_eq!(v["ledger"], "local", "{v}");
        assert!(repo
            .root()
            .join(crate::evidence::ledger::LOCAL_LEDGER_PATH)
            .is_file());
    }

    /// What the recorder refuses reaches the command as an error: a measurement that cannot
    /// be read, and a recording of nothing.
    #[test]
    fn a_refused_recording_is_an_error_of_the_command() {
        let repo = committed();
        let reports = tempfile::tempdir().unwrap();
        let cmd = EvidenceCommand::Record {
            suite: None,
            crate_output: None,
            origin: None,
            provenance: vec![reports.path().join("absent.json").display().to_string()],
            coverage: Some(reports.path().join("coverage.json")),
            ledger: None,
            run_record: None,
            run: None,
            cases: vec![],
        };
        let err = run(evidence(&repo, cmd, OutputFormat::Text)).unwrap_err();
        assert!(err.to_string().contains("absent.json"), "{err}");

        let cmd = recording(None, vec![]);
        let err = run(evidence(&repo, cmd, OutputFormat::Text)).unwrap_err();
        assert!(err.to_string().contains("nothing to record"), "{err}");
    }

    /// A stamp the capability refuses, or whose measurement cannot be written where it was
    /// told, is an error of the command.
    #[test]
    fn a_stamp_that_cannot_be_taken_or_written_is_an_error() {
        let repo = committed();
        let reports = tempfile::tempdir().unwrap();
        let pattern = EvidenceCommand::Stamp {
            producer: None,
            report: None,
            exclude: vec!["*".into()],
            out: None,
        };
        assert!(run(evidence(&repo, pattern, OutputFormat::Text)).is_err());

        let blocker = reports.path().join("file");
        std::fs::write(&blocker, "x").unwrap();
        for out in [blocker.join("p.json"), reports.path().to_path_buf()] {
            let cmd = stamping(None, None, Some(&out));
            let got = run(evidence(&repo, cmd, OutputFormat::Text));
            assert!(matches!(got, Err(Error::Transport(_))), "{out:?}: {got:?}");
        }
    }

    /// A measurement is written to its file, then answered in the
    /// format asked; an answer that cannot be written, in either format, is a transport error.
    #[test]
    fn a_stamp_is_written_then_answered_and_an_unwritable_answer_is_an_error() {
        let v = json!({ "producer": "suite", "commit": "0123456789ab", "working_tree": "clean" });
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("p.json");
        let mut text = Vec::new();
        write_stamp(&mut text, &v, Some(&file), OutputFormat::Text).unwrap();
        let text = String::from_utf8(text).unwrap();
        assert!(
            text.starts_with("stamped      suite at 01234567 (clean)\n"),
            "{text}"
        );
        assert!(text.contains("written      "), "{text}");
        let back: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(back, v);
        let mut json = Vec::new();
        write_stamp(&mut json, &v, None, OutputFormat::Json).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&json).unwrap(), v);

        for format in [OutputFormat::Json, OutputFormat::Text] {
            let mut closed = ClosesAfter {
                lines: 0,
                held: Vec::new(),
                refused: 0,
            };
            let got = write_stamp(&mut closed, &v, None, format);
            assert!(matches!(got, Err(Error::Transport(_))), "{got:?}");
        }
    }

    /// Every line of a measurement and of a recording's absences is written or reported:
    /// a line that cannot be written ends the output with a transport error at that line.
    #[test]
    fn a_stamp_or_an_absence_that_cannot_be_written_says_so_at_its_line() {
        let stamp = json!({
            "producer": "suite",
            "commit": "0123456789abcdef0123456789abcdef01234567",
            "working_tree": "clean",
            "excluded": ["dist"],
            "report": { "path": "suite.tsv", "digest": "sha256:abcdef0123456789abcdef" }
        });
        for lines in 0..4 {
            let mut out = ClosesAfter {
                lines,
                held: Vec::new(),
                refused: 0,
            };
            let got = stamp_text(&mut out, &stamp, Some(Path::new("p.json")));
            assert!(
                matches!(got, Err(Error::Transport(_))),
                "line {}",
                lines + 1
            );
        }
        let recording = json!({
            "recorded": 0, "passed": 0, "commit": "c", "ledger": "l",
            "run_record": { "id": "r", "absent": [{ "producer": "suite", "reason": "none" }] }
        });
        let mut out = ClosesAfter {
            lines: 5,
            held: Vec::new(),
            refused: 0,
        };
        let got = record_text(&mut out, &recording, None);
        assert!(matches!(got, Err(Error::Transport(_))), "{got:?}");
        let held = String::from_utf8(out.held).unwrap();
        assert!(
            held.ends_with("measured     nothing: the tree is recorded as unknown\n"),
            "{held}"
        );
    }
}
