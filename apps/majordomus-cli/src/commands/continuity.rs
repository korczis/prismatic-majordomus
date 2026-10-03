//! `majordomus continuity`: the cross-machine half of continuity, rendered for a terminal.
//!
//! Every subcommand runs a capability of the `continuity` module through the one executor,
//! so the answer read here and the answer an agent reads over MCP or HTTP are one value
//! rendered twice. This file renders and chooses the exit code; whether a record is
//! admitted, what a plan decides and what a sync moved are decided in
//! [`crate::continuity`], and `--format json` prints the value verbatim.
//!
//! Exit codes: 0 when the operation did what was asked (a plan that is ready, or has
//! nothing to resume; a resume that resumed; a sync that reached its remote), 10 when it
//! was declined or blocked and the output says why (a plan with blockers, a resume that
//! did not proceed, an unreachable remote, a publication refused for a secret), 13 when the
//! store itself could not be read.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{ContinuityArgs, ContinuityCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus continuity`.
pub fn run(args: ContinuityArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    answer(&app.context, &args.command, args.format, &mut stdout.lock())
}

type Text<W> = fn(&mut W, &Value) -> Result<u8>;

fn answer<W: Write>(
    ctx: &crate::capability::Context,
    command: &ContinuityCommand,
    format: OutputFormat,
    out: &mut W,
) -> Result<u8> {
    let (path, input, text): (&[&str], Value, Text<W>) = match command {
        ContinuityCommand::Status => (&["continuity", "status"], json!({}), status_text),
        ContinuityCommand::Records => (&["continuity", "records"], json!({}), records_text),
        ContinuityCommand::Device { label } => (
            &["continuity", "device"],
            json!({ "label": label }),
            device_text,
        ),
        ContinuityCommand::Plan { record } => (
            &["continuity", "plan"],
            json!({ "record": record }),
            plan_text,
        ),
        ContinuityCommand::Publish {
            handover,
            issue,
            milestone,
        } => (
            &["continuity", "publish"],
            json!({ "handover": handover, "issue": issue, "milestone": milestone }),
            publish_text,
        ),
        ContinuityCommand::Sync { remote } => (
            &["continuity", "sync"],
            json!({ "remote": remote }),
            sync_text,
        ),
        ContinuityCommand::Resume { record } => (
            &["continuity", "resume"],
            json!({ "record": record }),
            resume_text,
        ),
    };
    let input = strip_nulls(input);
    let v = execute(ctx, path, input)?;
    match format {
        OutputFormat::Json => {
            writeln!(out, "{v:#}").map_err(Error::Transport)?;
            Ok(exit_of(command, &v))
        }
        OutputFormat::Text => text(out, &v),
    }
}

/// The exit code a value carries, the same whichever way it is rendered.
fn exit_of(command: &ContinuityCommand, v: &Value) -> u8 {
    match command {
        ContinuityCommand::Plan { .. } => plan_exit(v),
        ContinuityCommand::Resume { .. } if v["resumed"].as_bool() != Some(true) => 10,
        ContinuityCommand::Sync { .. } if s(v, "action") == "unreachable" => 10,
        _ => 0,
    }
}

fn plan_exit(v: &Value) -> u8 {
    match s(v, "status") {
        "ready" | "ready_with_warnings" | "nothing_to_resume" => 0,
        _ => 10,
    }
}

fn strip_nulls(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).collect())
        }
        other => other,
    }
}

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
    ctx.execute(id, input).map_err(|e| match e {
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        // a refusal is the operation's own answer, with what to do instead
        CapabilityError::Refused(reason) | CapabilityError::InvalidInput(reason) => {
            Error::Refused { code: 10, reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })
}

// ---------------------------------------------------------------- text

fn w<W: Write>(out: &mut W, line: String) -> Result<()> {
    writeln!(out, "{line}").map_err(Error::Transport)
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

fn short(id: &str) -> &str {
    &id[..12.min(id.len())]
}

fn age(v: &Value) -> String {
    match v["age_minutes"].as_i64() {
        Some(m) if m < 60 => format!("{m} min ago"),
        Some(m) if m < 60 * 48 => format!("{} h ago", m / 60),
        Some(m) => format!("{} d ago", m / (60 * 24)),
        None => s(v, "published_at").to_string(),
    }
}

fn record_lines<W: Write>(out: &mut W, r: &Value, indent: &str) -> Result<()> {
    let device = &r["device"];
    let who = if r["this_device"].as_bool() == Some(true) {
        format!("{} (this device)", s(device, "label"))
    } else {
        format!("{} [{}]", s(device, "label"), s(r, "trust"))
    };
    w(out, format!("{indent}record      {}", short(s(r, "id"))))?;
    w(out, format!("{indent}device      {who}"))?;
    w(out, format!("{indent}published   {}", age(r)))?;
    if !s(r, "task").is_empty() {
        w(out, format!("{indent}intent      {}", s(r, "task")))?;
    }
    if !s(r, "issue").is_empty() {
        w(out, format!("{indent}issue       {}", s(r, "issue")))?;
    }
    w(
        out,
        format!(
            "{indent}source      {} @ {}  {}{}",
            r["branch"].as_str().unwrap_or("DETACHED"),
            short(s(r, "head")),
            s(r, "working_tree"),
            match r["changed_total"].as_u64() {
                Some(n) if n > 0 => format!(" ({n} uncommitted)"),
                _ => String::new(),
            }
        ),
    )?;
    if !s(r, "objective").is_empty() {
        w(out, format!("{indent}objective   {}", s(r, "objective")))?;
    }
    Ok(())
}

fn diagnostics<W: Write>(out: &mut W, v: &Value) -> Result<()> {
    for d in v["diagnostics"].as_array().into_iter().flatten() {
        let path = d["path"]
            .as_str()
            .map(|p| format!(" {p}:"))
            .unwrap_or_default();
        w(
            out,
            format!(
                "{:<7} {}{path} {}",
                s(d, "severity").to_uppercase(),
                s(d, "code"),
                s(d, "message")
            ),
        )?;
    }
    Ok(())
}

fn status_text<W: Write>(out: &mut W, v: &Value) -> Result<u8> {
    let device = &v["device"];
    w(out, format!("repository  {}", short(s(v, "repository"))))?;
    w(
        out,
        format!(
            "device      {} ({})",
            s(device, "label"),
            short(s(device, "node"))
        ),
    )?;
    w(
        out,
        format!(
            "branch      {} @ {}  {}",
            s(v, "branch"),
            short(s(v, "head")),
            s(v, "working_tree")
        ),
    )?;
    match v["position"].as_object() {
        Some(p) => w(
            out,
            format!(
                "continues   {} ({} here)",
                short(p["record"].as_str().unwrap_or("")),
                p["via"].as_str().unwrap_or("")
            ),
        )?,
        None => w(out, "continues   nothing yet on this branch".into())?,
    }
    let store = &v["store"];
    w(
        out,
        format!(
            "store       {} record(s), {} refused; {} {}",
            store["records"],
            store["refused"],
            store["remote"].as_str().unwrap_or("no remote"),
            s(store, "sync")
        ),
    )?;
    if let Some(sync) = v["last_sync"].as_object() {
        w(
            out,
            format!(
                "last sync   {} with {} at {}",
                sync["outcome"].as_str().unwrap_or(""),
                sync["remote"].as_str().unwrap_or(""),
                sync["at"].as_str().unwrap_or("")
            ),
        )?;
    }
    for line in v["lines"].as_array().into_iter().flatten() {
        if s(line, "state") == "diverged" {
            w(
                out,
                format!(
                    "line        {} DIVERGED: {} heads",
                    short(s(line, "id")),
                    line["heads"].as_array().map(Vec::len).unwrap_or(0)
                ),
            )?;
        }
    }
    let resumable = v["resumable"].as_array().cloned().unwrap_or_default();
    if resumable.is_empty() {
        w(out, "resumable   none".into())?;
    } else {
        w(out, String::new())?;
        w(
            out,
            format!("Resumable from another device ({})", resumable.len()),
        )?;
        for r in &resumable {
            record_lines(out, r, "  ")?;
            w(out, String::new())?;
        }
    }
    diagnostics(out, v)?;
    for n in v["next"].as_array().into_iter().flatten() {
        w(out, format!("next        {}", n.as_str().unwrap_or("")))?;
    }
    Ok(0)
}

fn records_text<W: Write>(out: &mut W, v: &Value) -> Result<u8> {
    let records = v["records"].as_array().cloned().unwrap_or_default();
    w(out, format!("{} record(s)", records.len()))?;
    for r in &records {
        w(
            out,
            format!(
                "{}  line {}  parent {}  {}  {}  {}",
                short(s(r, "id")),
                short(s(r, "line")),
                r["parent"].as_str().map(short).unwrap_or("-"),
                s(&r["device"], "label"),
                r["branch"].as_str().unwrap_or("DETACHED"),
                s(r, "published_at")
            ),
        )?;
    }
    diagnostics(out, v)?;
    Ok(0)
}

fn plan_body<W: Write>(out: &mut W, v: &Value) -> Result<()> {
    if let Some(r) = v.get("record").filter(|r| r.is_object()) {
        record_lines(out, r, "  ")?;
    }
    for c in v["candidates"].as_array().into_iter().flatten() {
        w(out, String::new())?;
        record_lines(out, c, "  ")?;
    }
    if let Some(src) = v.get("source").filter(|s| s.is_object()) {
        w(out, String::new())?;
        w(out, "Source".into())?;
        w(
            out,
            format!(
                "  expected    {} @ {}",
                src["expected_branch"].as_str().unwrap_or("DETACHED"),
                short(src["expected_head"].as_str().unwrap_or(""))
            ),
        )?;
        w(
            out,
            format!(
                "  local       {} @ {}{}",
                src["local_branch"].as_str().unwrap_or("DETACHED"),
                short(src["local_head"].as_str().unwrap_or("")),
                if src["local_dirty"].as_bool() == Some(true) {
                    "  dirty"
                } else {
                    ""
                }
            ),
        )?;
        w(out, format!("  relation    {}", s(src, "relation")))?;
        if src["origin_dirty"].as_bool() == Some(true) {
            w(
                out,
                format!(
                    "  origin      dirty: {} uncommitted file(s) not in its commit",
                    src["origin_changed_total"]
                ),
            )?;
        }
    }
    if !s(v, "next_action").is_empty() {
        w(out, String::new())?;
        w(out, "Next action".into())?;
        for line in s(v, "next_action").lines() {
            w(out, format!("  {line}"))?;
        }
    }
    let list = |key: &str| -> Vec<String> {
        v[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect()
    };
    let restores = list("restores");
    if !restores.is_empty() {
        w(out, String::new())?;
        w(out, format!("Restores    {}", restores.join(", ")))?;
        w(
            out,
            format!("Recomputes  {}", list("recomputes").join(", ")),
        )?;
    }
    for (label, key) in [("BLOCKER", "blockers"), ("WARN", "warnings")] {
        for i in v[key].as_array().into_iter().flatten() {
            w(
                out,
                format!("{label:<7} {}: {}", s(i, "code"), s(i, "message")),
            )?;
        }
    }
    for a in list("actions") {
        w(out, format!("run         {a}"))?;
    }
    Ok(())
}

fn plan_text<W: Write>(out: &mut W, v: &Value) -> Result<u8> {
    w(out, format!("Resume      {}", s(v, "status")))?;
    plan_body(out, v)?;
    Ok(plan_exit(v))
}

fn publish_text<W: Write>(out: &mut W, v: &Value) -> Result<u8> {
    let r = &v["record"];
    if v["written"].as_bool() == Some(true) {
        w(out, format!("Published   {}", short(s(r, "id"))))?;
    } else {
        w(
            out,
            format!(
                "Unchanged   {} already holds this handover",
                short(s(r, "id"))
            ),
        )?;
    }
    w(out, format!("handover    {}", s(v, "handover")))?;
    record_lines(out, r, "")?;
    match r["parent"].as_str() {
        Some(p) => w(out, format!("continues   {}", short(p)))?,
        None => w(out, format!("line        {} (new)", short(s(r, "line"))))?,
    }
    for n in v["next"].as_array().into_iter().flatten() {
        w(out, format!("next        {}", n.as_str().unwrap_or("")))?;
    }
    Ok(0)
}

fn sync_text<W: Write>(out: &mut W, v: &Value) -> Result<u8> {
    w(
        out,
        format!(
            "Sync        {} with {}",
            s(v, "action"),
            v["remote"].as_str().unwrap_or("no remote")
        ),
    )?;
    w(
        out,
        format!(
            "moved       {} fetched, {} published",
            v["fetched"], v["published"]
        ),
    )?;
    w(out, format!("store       {}", s(&v["store"], "sync")))?;
    for l in v["lines"].as_array().into_iter().flatten() {
        w(
            out,
            format!("line        {}  {}", short(s(l, "line")), s(l, "relation")),
        )?;
    }
    let resumable = v["resumable"].as_array().cloned().unwrap_or_default();
    if !resumable.is_empty() {
        w(out, String::new())?;
        w(
            out,
            format!("Resumable from another device ({})", resumable.len()),
        )?;
        for r in &resumable {
            record_lines(out, r, "  ")?;
        }
        w(out, String::new())?;
        w(out, "next        majordomus-cli continuity plan".into())?;
    }
    diagnostics(out, v)?;
    Ok(if s(v, "action") == "unreachable" {
        10
    } else {
        0
    })
}

fn resume_text<W: Write>(out: &mut W, v: &Value) -> Result<u8> {
    let plan = &v["plan"];
    if v["resumed"].as_bool() == Some(true) {
        w(out, "Session resumed".into())?;
        w(out, format!("handover    {}", s(v, "handover")))?;
        w(
            out,
            format!("decisions   {} carried", v["decisions_carried"]),
        )?;
        if let Some(r) = plan.get("record").filter(|r| r.is_object()) {
            w(
                out,
                format!("from        {} -> this device", s(&r["device"], "label")),
            )?;
        }
    } else {
        w(out, format!("Resume blocked  {}", s(plan, "status")))?;
    }
    plan_body(out, plan)?;
    Ok(if v["resumed"].as_bool() == Some(true) {
        0
    } else {
        10
    })
}

fn device_text<W: Write>(out: &mut W, v: &Value) -> Result<u8> {
    let d = &v["device"];
    w(out, format!("device      {}", s(d, "label")))?;
    w(out, format!("node        {}", s(d, "node")))?;
    w(out, format!("public key  {}", s(v, "public_key")))?;
    Ok(0)
}
