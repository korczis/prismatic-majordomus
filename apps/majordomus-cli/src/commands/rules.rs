//! `majordomus rules`: the rule corpus against the proof there is for it.
//!
//! Every subcommand runs a capability of the `rules` module through the one executor, so
//! the answer a person reads here and the answer a client reads over MCP or HTTP are the
//! same answer rendered twice, not two derivations that happen to agree today.
//!
//! The text rendering is blunt about the distinctions the subsystem exists for. `proven`
//! and `inputs unchanged` print differently, and so do `gated` and `proven`: a gate refuses
//! violations, which is a mechanism, and a recorded pass is a verdict, and a column that
//! showed both as a tick would be the badge whose derivation cannot be inspected. `reviewed`
//! prints its reason, because the reason is the whole declaration.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, RulesArgs, RulesCommand};
use crate::error::{Error, Result};

/// The exit code when `--check` finds a rule whose class the proof does not support.
pub const EXIT_UNSUPPORTED: u8 = 10;

/// Run `majordomus rules`.
pub fn run(args: RulesArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match args.command {
        RulesCommand::Report {
            state,
            class,
            namespace,
            findings,
            check,
        } => {
            let mut input = json!({ "findings_only": findings });
            for (key, value) in [
                ("state", &state),
                ("class", &class),
                ("namespace", &namespace),
            ] {
                if let Some(v) = value {
                    input[key] = json!(v);
                }
            }
            let v = execute(ctx, &["rules", "report"], input)?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => report_text(&mut out, &v)?,
            }
            if check && !v["findings"].as_array().map(Vec::is_empty).unwrap_or(true) {
                return Ok(EXIT_UNSUPPORTED);
            }
            Ok(0)
        }

        RulesCommand::Show { id } => {
            let v = execute(ctx, &["rules", "show"], json!({ "rule": id }))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => show_text(&mut out, &v)?,
            }
            Ok(0)
        }

        RulesCommand::Proves { id } => {
            let v = execute(ctx, &["rules", "proves"], json!({ "test": id }))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => proves_text(&mut out, &v)?,
            }
            Ok(0)
        }
    }
}

// ---------------------------------------------------------------- text

fn w(out: &mut std::io::StdoutLock<'_>, s: String) -> Result<()> {
    writeln!(out, "{s}").map_err(Error::Transport)
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("?")
}

fn report_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    w(
        out,
        format!(
            "head         {} ({})",
            short(v["head"].as_str().unwrap_or("unknown")),
            s(v, "working_tree")
        ),
    )?;
    let c = &v["coverage"];
    w(
        out,
        format!(
            "rules        {} — {} blocking, {} advisory",
            c["rules"], c["blocking"], c["advisory"]
        ),
    )?;
    // Every number below is counted apart on purpose. A single "governance is N% complete"
    // would be the vanity metric this subsystem exists instead of.
    for (label, key) in [
        ("names a proof", "named_proof"),
        ("proof in the tree", "artifacts_present"),
        ("a runner drives", "runnable"),
        ("a run recorded", "recorded"),
        ("a run that passed", "passing"),
        ("a gate runs", "gated"),
        ("gate, no verdict", "mechanism_only"),
        ("review-enforced", "review_only"),
    ] {
        w(out, format!("{label:<18} {} / {}", c[key], c["rules"]))?;
    }
    w(out, String::new())?;

    if let Some(t) = v["states"].as_object() {
        for (state, count) in t {
            w(out, format!("{state:<18} {count}"))?;
        }
        w(out, String::new())?;
    }

    for p in v["rules"].as_array().into_iter().flatten() {
        let r = &p["rule"];
        w(
            out,
            format!(
                "{:<18} {:<9} {:<10} {}",
                s(p, "state"),
                s(r, "class"),
                r["enforcement"]["mode"].as_str().unwrap_or("?"),
                s(r, "id")
            ),
        )?;
        for t in p["tests"].as_array().into_iter().flatten() {
            w(
                out,
                format!(
                    "                   {} {}{}",
                    t["kind"].as_str().unwrap_or("?"),
                    s(t, "path"),
                    if t["present"].as_bool().unwrap_or(false) {
                        String::new()
                    } else {
                        "  (not in this checkout)".to_string()
                    }
                ),
            )?;
        }
        if let Some(why) = r["enforcement"]["reviewed_because"].as_str() {
            w(out, format!("                   because {why}"))?;
        }
    }

    let findings = v["findings"].as_array().cloned().unwrap_or_default();
    if !findings.is_empty() {
        w(out, String::new())?;
        w(
            out,
            format!(
                "{} rule(s) declare a class the proof does not support:",
                findings.len()
            ),
        )?;
        for f in &findings {
            w(out, format!("  {:<44} {}", s(f, "rule"), s(f, "reason")))?;
            w(
                out,
                format!("  {:<44} reproduce: {}", "", s(f, "reproduce")),
            )?;
        }
    }
    Ok(())
}

fn show_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    let p = &v["proof"];
    let r = &p["rule"];
    w(out, format!("rule         {}", s(r, "id")))?;
    w(out, format!("             {}", s(r, "title")))?;
    if let Some(st) = r["statement"].as_str() {
        w(out, format!("             {st}"))?;
    }
    w(out, format!("class        {}", s(r, "class")))?;
    w(out, format!("status       {}", s(r, "status")))?;
    w(out, format!("source       {}", s(r, "path")))?;
    w(
        out,
        format!(
            "mode         {}",
            r["enforcement"]["mode"].as_str().unwrap_or("?")
        ),
    )?;
    w(out, format!("state        {}", s(p, "state")))?;
    w(out, format!("             {}", s(p, "meaning")))?;

    if let Some(val) = p["validator"].as_object() {
        w(
            out,
            format!(
                "validator    {}{}",
                val["function"].as_str().unwrap_or("?"),
                match val["defined_in"].as_str() {
                    Some(f) => format!("  in {f}"),
                    None => "  (nothing in lib/ defines it)".to_string(),
                }
            ),
        )?;
    }
    if let Some(why) = r["enforcement"]["reviewed_because"].as_str() {
        w(out, format!("because      {why}"))?;
    }

    let tests = p["tests"].as_array().cloned().unwrap_or_default();
    if !tests.is_empty() {
        w(out, String::new())?;
        w(out, format!("proved by {} artifact(s):", tests.len()))?;
        for t in &tests {
            w(
                out,
                format!(
                    "  {:<8} {:<44} {}",
                    t["kind"].as_str().unwrap_or("?"),
                    s(t, "path"),
                    if t["present"].as_bool().unwrap_or(false) {
                        s(t, "state")
                    } else {
                        "not in this checkout"
                    }
                ),
            )?;
            if let Some(e) = t["execution"].as_object() {
                w(
                    out,
                    format!(
                        "  {:<8} {:<44} {} at {} · {}s · {}",
                        "",
                        "",
                        e["outcome"].as_str().unwrap_or("?"),
                        short(e["commit"].as_str().unwrap_or("?")),
                        e["seconds"],
                        e["at"].as_str().unwrap_or("?")
                    ),
                )?;
            }
            if let Some(rp) = t["reproduce"].as_str() {
                w(out, format!("  {:<8} {:<44} reproduce: {rp}", "", ""))?;
            }
        }
    }

    let gates: Vec<&str> = p["gates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !gates.is_empty() {
        w(out, String::new())?;
        w(out, format!("gates        {}", gates.join(", ")))?;
    }

    for (label, key) in [("depends on", "depends_on"), ("required by", "required_by")] {
        let list = v[key].as_array().cloned().unwrap_or_default();
        if list.is_empty() {
            continue;
        }
        w(out, String::new())?;
        w(out, format!("{label} {} rule(s):", list.len()))?;
        for d in &list {
            w(out, format!("  {:<18} {}", s(d, "state"), s(d, "id")))?;
        }
    }

    if let Some(m) = v["missing"].as_str() {
        w(out, String::new())?;
        w(out, format!("missing      {m}"))?;
    }
    Ok(())
}

fn proves_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    w(
        out,
        format!(
            "test         {}",
            v["test"].as_str().unwrap_or("(no runner)")
        ),
    )?;
    w(
        out,
        format!(
            "source       {}{}",
            s(v, "path"),
            if v["present"].as_bool().unwrap_or(false) {
                ""
            } else {
                "  (not in this checkout)"
            }
        ),
    )?;
    if let Some(r) = v["reproduce"].as_str() {
        w(out, format!("reproduce    {r}"))?;
    }

    let proves = v["proves"].as_array().cloned().unwrap_or_default();
    w(out, String::new())?;
    w(out, format!("proves {} rule(s):", proves.len()))?;
    for r in &proves {
        w(
            out,
            format!(
                "  {:<18} {:<9} {}",
                s(r, "state"),
                s(r, "class"),
                s(r, "id")
            ),
        )?;
    }

    let sole: Vec<&str> = v["sole_proof_of"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !sole.is_empty() {
        w(out, String::new())?;
        w(
            out,
            format!(
                "deleting it would leave {} rule(s) with no proof at all:",
                sole.len()
            ),
        )?;
        for r in sole {
            w(out, format!("  {r}"))?;
        }
    }
    Ok(())
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
        // A rule the repository does not declare is a missing thing, not a malformed
        // request, and the exit code has to say which: a typo that read as "this rule has no
        // proof" is the one answer this command must never give.
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Protocol { reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}
