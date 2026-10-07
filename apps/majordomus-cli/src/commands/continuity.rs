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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuity::tests_support::{body, commit, git, handover, machine, World};
    use crate::continuity::{self as domain, PublishRequest};

    /// Render `v` with `text`, answering the lines and the exit code.
    fn render(text: Text<Vec<u8>>, v: &Value) -> (String, u8) {
        let mut out = Vec::new();
        let code = text(&mut out, v).unwrap();
        (String::from_utf8(out).unwrap(), code)
    }

    fn value<T: serde::Serialize>(t: &T) -> Value {
        serde_json::to_value(t).unwrap()
    }

    /// Every rendering of every answer, from the values the operations really return on two
    /// clones of one remote — never from a hand-built value of the shape they might have.
    #[test]
    fn every_answer_renders_what_a_person_acts_on_and_exits_as_the_json_does() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        handover(
            &a_root,
            "20261003T120000Z",
            &body("Ship x", "parser done", "write\nthe test"),
            None,
        );
        let published = value(
            &domain::publish(
                &a,
                &PublishRequest {
                    issue: Some("#184".into()),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let (text, code) = render(publish_text, &published);
        assert_eq!(code, 0);
        assert!(text.starts_with("Published   "), "{text}");
        assert!(
            text.contains("device      macbook-pro (this device)"),
            "{text}"
        );
        assert!(text.contains("issue       #184"), "{text}");
        assert!(text.contains("source      feature/x @ "), "{text}");
        assert!(text.contains("objective   Ship x"), "{text}");
        assert!(text.contains(" (new)"), "{text}");
        assert!(
            text.contains("next        majordomus-cli continuity sync"),
            "{text}"
        );
        let unchanged = value(&domain::publish(&a, &PublishRequest::default()).unwrap());
        assert!(render(publish_text, &unchanged)
            .0
            .starts_with("Unchanged   "));

        let (text, _) = render(status_text, &value(&domain::status(&a).unwrap()));
        assert!(text.contains("continues   "), "{text}");
        assert!(text.contains("never_synced"), "{text}");
        assert!(text.contains("resumable   none"), "{text}");
        assert!(
            text.contains("next        majordomus-cli continuity sync"),
            "{text}"
        );

        let synced = value(&domain::sync(&a, None).unwrap());
        let (text, code) = render(sync_text, &synced);
        assert_eq!(code, 0);
        assert!(
            text.starts_with("Sync        published with origin"),
            "{text}"
        );
        assert!(
            text.contains("moved       0 fetched, 1 published"),
            "{text}"
        );
        assert!(text.contains("local_only"), "{text}");
        let sync_cmd = ContinuityCommand::Sync { remote: None };
        assert_eq!(exit_of(&sync_cmd, &synced), 0);

        // B: a clone, before and after it asks
        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        let (text, _) = render(status_text, &value(&domain::status(&b).unwrap()));
        assert!(
            text.contains("continues   nothing yet on this branch"),
            "{text}"
        );
        let nothing = value(&domain::plan(&b, None).unwrap());
        let (text, code) = render(plan_text, &nothing);
        assert_eq!(code, 0, "nothing to resume is not a failure");
        assert!(
            text.contains("run         majordomus-cli continuity sync"),
            "{text}"
        );

        let synced = value(&domain::sync(&b, None).unwrap());
        let (text, _) = render(sync_text, &synced);
        assert!(text.contains("Resumable from another device (1)"), "{text}");
        assert!(text.contains("macbook-pro [undeclared]"), "{text}");
        let (text, _) = render(status_text, &value(&domain::status(&b).unwrap()));
        assert!(text.contains("last sync   ok with origin at "), "{text}");
        assert!(text.contains("Resumable from another device (1)"), "{text}");
        assert!(
            text.contains("next        majordomus-cli continuity plan"),
            "{text}"
        );

        let planned = value(&domain::plan(&b, None).unwrap());
        let (text, code) = render(plan_text, &planned);
        assert_eq!(code, 0);
        assert!(
            text.starts_with("Resume      ready_with_warnings"),
            "{text}"
        );
        assert!(text.contains("  expected    feature/x @ "), "{text}");
        assert!(text.contains("  relation    exact"), "{text}");
        assert!(text.contains("Next action\n  write\n  the test"), "{text}");
        assert!(text.contains("Restores    handover, intent"), "{text}");
        assert!(text.contains("Recomputes  worktree"), "{text}");
        assert!(text.contains("WARN    origin_undeclared: "), "{text}");
        assert!(
            text.contains("run         majordomus-cli continuity resume --record "),
            "{text}"
        );
        let plan_cmd = ContinuityCommand::Plan { record: None };
        assert_eq!(exit_of(&plan_cmd, &planned), 0);

        let resumed = value(&domain::resume(&b, None).unwrap());
        let (text, code) = render(resume_text, &resumed);
        assert_eq!(code, 0);
        assert!(
            text.starts_with("Session resumed\nhandover    .ai/local/state/handovers/"),
            "{text}"
        );
        assert!(text.contains("decisions   0 carried"), "{text}");
        assert!(
            text.contains("from        macbook-pro -> this device"),
            "{text}"
        );
        assert_eq!(
            exit_of(&ContinuityCommand::Resume { record: None }, &resumed),
            0
        );

        // a source this clone does not have blocks the resume, and both renderings exit 10
        commit(&a_root, "unpushed");
        handover(&a_root, "20261003T130000Z", &body("o", "s", "n"), None);
        domain::publish(&a, &PublishRequest::default()).unwrap();
        domain::sync(&a, None).unwrap();
        domain::sync(&b, None).unwrap();
        std::fs::write(b_root.join("lib/a"), "dirty\n").unwrap();
        let blocked = value(&domain::resume(&b, None).unwrap());
        let (text, code) = render(resume_text, &blocked);
        assert_eq!(code, 10);
        assert!(
            text.starts_with("Resume blocked  requires_source_update"),
            "{text}"
        );
        assert!(text.contains("BLOCKER head_missing: "), "{text}");
        assert!(text.contains("  local       feature/x @ "), "{text}");
        assert!(text.contains("  dirty"), "{text}");
        assert_eq!(
            exit_of(&ContinuityCommand::Resume { record: None }, &blocked),
            10
        );
        assert_eq!(exit_of(&plan_cmd, &blocked["plan"]), 10);
        assert_eq!(render(plan_text, &blocked["plan"]).1, 10);

        let all = value(&domain::records(&b).unwrap());
        let (text, code) = render(records_text, &all);
        assert_eq!(code, 0);
        assert!(text.starts_with("2 record(s)\n"), "{text}");
        assert!(
            text.contains("  parent -  macbook-pro  feature/x  "),
            "{text}"
        );
    }

    /// The records a person must choose between are listed, a dirty origin is named, and
    /// the diagnostics and an unreachable remote reach the terminal.
    #[test]
    fn a_choice_a_dirty_origin_and_an_unreachable_remote_are_rendered() {
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);
        std::fs::write(a_root.join("lib/a"), "a\nuncommitted\n").unwrap();
        handover(&a_root, "20261003T120000Z", &body("one", "x", "y"), None);
        domain::publish(&a, &PublishRequest::default()).unwrap();
        domain::sync(&a, None).unwrap();
        let c_root = w.clone_as("c");
        let c = machine(&c_root, w.identity("c", "mac-studio"), None);
        handover(&c_root, "20261003T120500Z", &body("two", "x", "y"), None);
        domain::publish(&c, &PublishRequest::default()).unwrap();
        domain::sync(&c, None).unwrap();

        let b_root = w.clone_as("b");
        let b = machine(&b_root, w.identity("b", "mac-mini"), None);
        domain::sync(&b, None).unwrap();
        let choice = value(&domain::plan(&b, None).unwrap());
        let (text, code) = render(plan_text, &choice);
        assert_eq!(code, 10);
        assert!(text.starts_with("Resume      choose_record"), "{text}");
        assert!(
            text.contains("device      mac-studio [undeclared]"),
            "{text}"
        );
        assert!(text.contains("BLOCKER several_resumable"), "{text}");

        let dirty = choice["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["device"]["label"] == "macbook-pro")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let (text, _) = render(plan_text, &value(&domain::plan(&b, Some(&dirty)).unwrap()));
        assert!(text.contains(" (1 uncommitted)"), "{text}");
        assert!(
            text.contains("  origin      dirty: 1 uncommitted file(s) not in its commit"),
            "{text}"
        );
        assert!(text.contains("WARN    source_incomplete: "), "{text}");

        // a stray file in the store reaches every listing as a diagnostic
        crate::continuity::store::add(&b_root, &[("stray".into(), b"{}".to_vec())], "x\n").unwrap();
        let (text, _) = render(records_text, &value(&domain::records(&b).unwrap()));
        assert!(
            text.contains("WARNING continuity.stray_file records/stray.json: "),
            "{text}"
        );

        let gone = w.dir.path().join("gone.git");
        git(&b_root, &["remote", "add", "gone", gone.to_str().unwrap()]);
        let unreachable = value(&domain::sync(&b, Some("gone")).unwrap());
        let (text, code) = render(sync_text, &unreachable);
        assert_eq!(code, 10);
        assert!(
            text.starts_with("Sync        unreachable with gone"),
            "{text}"
        );
        assert!(
            text.contains("WARNING continuity.remote_unreachable"),
            "{text}"
        );
        assert_eq!(
            exit_of(&ContinuityCommand::Sync { remote: None }, &unreachable),
            10
        );
    }

    #[test]
    fn an_age_reads_in_the_unit_a_person_would_use() {
        let at = |m: Option<i64>| {
            let mut v = json!({ "published_at": "2026-10-03T12:00:00Z" });
            if let Some(m) = m {
                v["age_minutes"] = json!(m);
            }
            age(&v)
        };
        assert_eq!(at(Some(5)), "5 min ago");
        assert_eq!(at(Some(180)), "3 h ago");
        assert_eq!(at(Some(60 * 24 * 3)), "3 d ago");
        assert_eq!(at(None), "2026-10-03T12:00:00Z");
    }

    #[test]
    fn a_device_renders_its_label_node_and_key() {
        let v = json!({ "device": { "node": "ab", "label": "mac-mini" }, "public_key": "cd" });
        let (text, code) = render(device_text, &v);
        assert_eq!(code, 0);
        assert_eq!(
            text,
            "device      mac-mini\nnode        ab\npublic key  cd\n"
        );
        assert_eq!(exit_of(&ContinuityCommand::Device { label: None }, &v), 0);
    }

    #[test]
    fn only_the_absent_options_are_left_out_of_the_input() {
        let v = strip_nulls(json!({ "remote": null, "record": "ab" }));
        assert_eq!(v, json!({ "record": "ab" }));
        assert_eq!(strip_nulls(json!(3)), json!(3));
    }
}
