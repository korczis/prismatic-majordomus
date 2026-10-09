//! `majordomus fleet`: the fleet's plan, status and rollout, rendered for a terminal.
//!
//! Every subcommand runs a capability of the `fleet` module through the one executor; this
//! file renders and chooses the exit code, and `--format json` prints the value verbatim.
//!
//! Exit codes: 0 when the answer is what was asked (a plan; a status in which every machine
//! answered; a rollout in which every machine converged and the hubs see each other), 10
//! otherwise, with the lines saying which machine and why.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{FleetArgs, FleetCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus fleet`.
pub fn run(args: FleetArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let (path, input, text): (&[&str], Value, fn(&Value) -> (Vec<String>, u8)) =
        match &args.command {
            FleetCommand::Plan { machine, version } => (
                &["fleet", "plan"],
                json!({ "machines": machine, "version": version }),
                plan_text,
            ),
            FleetCommand::Status => (&["fleet", "status"], json!({}), status_text),
            FleetCommand::Rollout {
                machine,
                version,
                keep_servers,
            } => (
                &["fleet", "rollout"],
                json!({ "machines": machine, "version": version, "keep_servers": keep_servers }),
                rollout_text,
            ),
        };
    let input = match input {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(_, v)| {
                    !v.is_null()
                        && v.as_bool() != Some(false)
                        && v.as_array().is_none_or(|a| !a.is_empty())
                })
                .collect(),
        ),
        other => other,
    };
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    let asked = path.join(".");
    let id = app
        .context
        .registry
        .by_cli(&words)
        .map_or(asked.as_str(), |c| c.id.as_str())
        .to_string();
    let v = app.context.execute(&id, input).map_err(refusal)?;
    let (lines, code) = text(&v);
    let rendered = match args.format {
        OutputFormat::Json => format!("{v:#}"),
        OutputFormat::Text => lines.join("\n"),
    };
    let stdout = std::io::stdout();
    writeln!(stdout.lock(), "{rendered}").map_err(Error::Transport)?;
    Ok(code)
}

fn refusal(e: CapabilityError) -> Error {
    match e {
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        CapabilityError::Refused(reason) | CapabilityError::InvalidInput(reason) => {
            Error::Refused { code: 10, reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

fn or<'a>(v: &'a Value, key: &str, absent: &'a str) -> &'a str {
    v[key].as_str().unwrap_or(absent)
}

fn items(v: &Value, key: &str) -> Vec<Value> {
    v[key].as_array().cloned().unwrap_or_default()
}

fn plan_text(v: &Value) -> (Vec<String>, u8) {
    let mut out = vec![
        format!("fleet      {}", s(v, "declaration")),
        format!("version    {}", s(v, "version")),
        format!("installer  {}", s(v, "installer")),
        format!("this node  {}", or(v, "local_node", "(none)")),
    ];
    for m in items(v, "machines") {
        let reach = if m["local"].as_bool() == Some(true) {
            "local".to_string()
        } else {
            items(&m, "destinations")
                .iter()
                .filter_map(|d| d.as_str().map(str::to_string))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let hub = if m["hub"].is_object() {
            format!(
                "  hub ~/{} :{}",
                s(&m["hub"], "checkout"),
                m["hub"]["port"].as_u64().unwrap_or_default()
            )
        } else {
            String::new()
        };
        let steps: Vec<String> = items(&m, "steps")
            .iter()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect();
        out.push(format!(
            "machine    {:<22} {}  by {reach}{hub}",
            s(&m, "id"),
            &s(&m, "node")[..8.min(s(&m, "node").len())]
        ));
        out.push(format!("  steps    {}", steps.join(" → ")));
    }
    (out, 0)
}

fn status_text(v: &Value) -> (Vec<String>, u8) {
    let mut out = vec![
        format!("fleet      {}", s(v, "declaration")),
        format!("this       {}", s(v, "version")),
    ];
    let mut code = 0;
    for m in items(v, "machines") {
        let id = s(&m, "id");
        if let Some(e) = m["error"].as_str() {
            code = 10;
            out.push(format!("machine    {id:<22} unreachable — {e}"));
            continue;
        }
        out.push(format!(
            "machine    {id:<22} {}  {}  installed {}  by {}",
            or(&m, "platform", "?"),
            &s(&m, "node")[..8.min(s(&m, "node").len())],
            or(&m, "installed", "nothing"),
            s(&m, "reached_by")
        ));
        if m["hub"].is_object() {
            let h = &m["hub"];
            let checkout = if h["present"].as_bool() == Some(true) {
                format!(
                    "{} {}{}",
                    or(h, "branch", "?"),
                    &s(h, "head")[..10.min(s(h, "head").len())],
                    if h["dirty"].as_bool() == Some(true) {
                        " dirty"
                    } else {
                        ""
                    }
                )
            } else {
                "absent".into()
            };
            out.push(format!(
                "  hub      ~/{} ({checkout})  port {} answers {}",
                s(h, "checkout"),
                h["port"].as_u64().unwrap_or_default(),
                or(h, "answering", "nothing")
            ));
        }
        for server in items(&m, "servers") {
            out.push(format!("  server   {}", server.as_str().unwrap_or_default()));
        }
    }
    (out, code)
}

fn rollout_text(v: &Value) -> (Vec<String>, u8) {
    let mut out = vec![
        format!("fleet      {}", s(v, "declaration")),
        format!("version    {}", s(v, "version")),
    ];
    for m in items(v, "machines") {
        out.push(format!(
            "machine    {:<22} {}  {} → {}  by {}",
            s(&m, "id"),
            s(&m, "verdict"),
            or(&m, "before", "nothing"),
            or(&m, "after", "?"),
            or(&m, "reached_by", "-")
        ));
        for st in items(&m, "steps") {
            out.push(format!(
                "  {:<9}{:<9}{}",
                s(&st, "step"),
                s(&st, "status"),
                s(&st, "detail")
            ));
        }
    }
    for h in items(v, "mesh") {
        let by: Vec<String> = items(&h, "seen_by")
            .iter()
            .filter_map(|b| b.as_str().map(str::to_string))
            .collect();
        let versions: Vec<String> = items(&h, "versions")
            .iter()
            .filter_map(|b| b.as_str().map(str::to_string))
            .collect();
        out.push(format!(
            "mesh       {:<22} seen by {}{}",
            s(&h, "id"),
            if by.is_empty() {
                "no hub".to_string()
            } else {
                by.join(", ")
            },
            if versions.is_empty() {
                String::new()
            } else {
                format!(" at {}", versions.join(", "))
            }
        ));
    }
    let verdict = s(v, "verdict");
    out.push(format!("verdict    {verdict}"));
    (out, if verdict == "converged" { 0 } else { 10 })
}
