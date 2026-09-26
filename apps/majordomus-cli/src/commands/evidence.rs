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
        } => {
            let mut input = json!({});
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
                    for u in v["unknown"].as_array().into_iter().flatten() {
                        writeln!(out, "unknown      {} (no such test here)", u)
                            .map_err(Error::Transport)?;
                    }
                }
            }
            Ok(0)
        }
    }
}

// ---------------------------------------------------------------- text

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

    /// A claim whose state needs a reason prints it on the line under the claim, and one
    /// whose state says it all prints nothing more.
    #[test]
    fn a_claim_prints_its_detail_when_it_has_one() {
        assert_eq!(detail_line(&json!({ "state": "not_run" })), "");
        let d = detail_line(&json!({ "state": "not_run", "detail": "the test declined to run" }));
        assert_eq!(d, "\n                   the test declined to run");
    }
}
