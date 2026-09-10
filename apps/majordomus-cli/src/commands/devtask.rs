//! `majordomus devtask`: one issue or one milestone as an executable development scope,
//! through the registry's own `devtask.*` capabilities.
//!
//! This file renders; it does not know anything. Both answers below are the output of the
//! capability the CLI exposure names, executed through the one executor, so the command
//! line, the HTTP routes and the MCP tools cannot answer the same question differently.
//! Nothing here decides a readiness, a blocker or a provenance.
//!
//! The one thing the rendering does decide is what a person sees first, and it is the
//! provenance: every authored value is printed with the marker of where it came from, so
//! that a field a machine worked out cannot be read off a terminal as something a person
//! wrote down. That is the whole point of the model, and a rendering that hid it would
//! undo it.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{DevtaskArgs, DevtaskCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus devtask`.
pub fn run(args: DevtaskArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let format = args.format;
    match &args.command {
        DevtaskCommand::Issue { id, no_git } => {
            let v = call(
                &app,
                &["devtask", "issue"],
                json!({ "issue": id, "git": !no_git }),
            )?;
            emit(format, &v, issue_text)
        }
        DevtaskCommand::Milestone { id } => {
            let v = call(&app, &["devtask", "milestone"], json!({ "milestone": id }))?;
            emit(format, &v, milestone_text)
        }
    }
}

fn call(app: &App, path: &[&str], input: Value) -> Result<Value> {
    let ctx = &app.context;
    let words: Vec<String> = path.iter().map(|w| (*w).to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.to_string())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(&id, input).map_err(map)
}

fn map(e: CapabilityError) -> Error {
    match e {
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        CapabilityError::InvalidInput(reason) => Error::Protocol { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn emit(format: OutputFormat, v: &Value, text: impl Fn(&Value) -> String) -> Result<u8> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let body = match format {
        OutputFormat::Json => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
        OutputFormat::Text => text(v),
    };
    writeln!(out, "{body}").map_err(Error::Transport)?;
    Ok(0)
}

/// One attested field on one line: the value, then where it came from. An unknown field is
/// printed with its reason rather than omitted, because a reader has to be able to tell a
/// field that says nothing from a field nobody asked about.
fn field(label: &str, f: &Value) -> String {
    let provenance = f["provenance"].as_str().unwrap_or("unknown");
    let reason = f["reason"].as_str().unwrap_or("");
    let value = match (&f["value"], &f["values"]) {
        (Value::String(s), _) => s.clone(),
        (Value::Number(n), _) => n.to_string(),
        (_, Value::Array(items)) if !items.is_empty() => items
            .iter()
            .map(|i| i.as_str().unwrap_or("").to_string())
            .collect::<Vec<String>>()
            .join(", "),
        (_, Value::Array(_)) => "(none)".to_string(),
        _ => format!("— {reason}"),
    };
    format!("  {label:<22}  {value}  [{provenance}]")
}

fn issue_text(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    let id = v["issue"].as_str().unwrap_or("");
    let declared = v["declared"].as_bool().unwrap_or(false);
    out.push(format!(
        "{id}  {}",
        if declared {
            v["record"].as_str().unwrap_or("declared")
        } else {
            "not declared by the canonical model"
        }
    ));
    let r = &v["readiness"];
    out.push(String::new());
    out.push(format!(
        "readiness  {}  (canonical {})",
        r["state"].as_str().unwrap_or(""),
        if r["canonical_status"].as_str().unwrap_or("").is_empty() {
            "—"
        } else {
            r["canonical_status"].as_str().unwrap_or("")
        }
    ));
    out.push(format!("  {}", r["reason"].as_str().unwrap_or("")));
    if let Some(blockers) = r["blockers"].as_array() {
        for b in blockers {
            out.push(format!(
                "  blocker  {:<10} {:<12} {}",
                b["kind"].as_str().unwrap_or(""),
                b["subject"].as_str().unwrap_or(""),
                b["detail"].as_str().unwrap_or("")
            ));
        }
    }

    let d = &v["declaration"];
    out.push(String::new());
    out.push("declaration — what a person authored".into());
    for (label, key) in [
        ("title", "title"),
        ("milestone", "milestone"),
        ("priority", "priority"),
        ("profile", "profile"),
        ("objective", "objective"),
        ("completion", "completion"),
        ("started_at", "started_at"),
        ("verified_at", "verified_at"),
        ("completed_at", "completed_at"),
        ("depends_on", "depends_on"),
        ("scope", "scope"),
        ("acceptance_criteria", "acceptance_criteria"),
        ("validation", "validation"),
        ("evidence_required", "evidence_required"),
        ("evidence_present", "evidence_present"),
        ("evidence_commits", "evidence_commits"),
    ] {
        out.push(field(label, &d[key]));
    }

    let p = &v["position"];
    out.push(String::new());
    out.push("position — what the plan graph derives".into());
    for (label, key) in [
        ("status", "status"),
        ("wave", "wave"),
        ("blocked_by", "blocked_by"),
        ("dependents", "dependents"),
        ("unblocks", "transitive_dependents"),
        ("milestone_status", "milestone_status"),
        ("evidence_have", "evidence_have"),
        ("evidence_need", "evidence_need"),
    ] {
        out.push(field(label, &p[key]));
    }

    let e = &v["execution"];
    out.push(String::new());
    out.push(format!(
        "execution — local state  (git consulted: {})",
        e["git_consulted"].as_bool().unwrap_or(false)
    ));
    for (label, key) in [
        ("branches", "branches"),
        ("merged_branches", "merged_branches"),
        ("commits", "commits"),
        ("trunk", "trunk"),
        ("sessions", "sessions"),
    ] {
        out.push(field(label, &e[key]));
    }

    let sy = &v["synchronisation"];
    out.push(String::new());
    out.push(format!(
        "synchronisation — {} (writable: {})",
        sy["adapter"].as_str().unwrap_or(""),
        sy["writable"].as_bool().unwrap_or(false)
    ));
    out.push(field("repository", &sy["repository"]));
    out.push(field("state", &sy["state"]));
    out.push(field("external_id", &sy["external_id"]));

    if let Some(diagnostics) = v["diagnostics"].as_array() {
        if !diagnostics.is_empty() {
            out.push(String::new());
            out.push("diagnostics".into());
            for x in diagnostics {
                out.push(format!(
                    "  {:<5} {:<22} {}",
                    x["level"].as_str().unwrap_or(""),
                    x["code"].as_str().unwrap_or(""),
                    x["message"].as_str().unwrap_or("")
                ));
            }
        }
    }

    let a = &v["attestation"];
    out.push(String::new());
    out.push(format!(
        "{} field(s): {} explicit, {} derived, {} inferred, {} unknown",
        a["explicit"].as_u64().unwrap_or(0)
            + a["derived"].as_u64().unwrap_or(0)
            + a["inferred"].as_u64().unwrap_or(0)
            + a["unknown"].as_u64().unwrap_or(0),
        a["explicit"],
        a["derived"],
        a["inferred"],
        a["unknown"]
    ));
    out.join("\n")
}

fn ids(v: &Value) -> String {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect::<Vec<String>>()
                .join(" ")
        })
        .unwrap_or_default()
}

fn milestone_text(v: &Value) -> String {
    let mut out: Vec<String> = Vec::new();
    let id = v["milestone"].as_str().unwrap_or("");
    out.push(format!(
        "{id}  {}  [{}]",
        v["title"]["value"].as_str().unwrap_or("(not declared)"),
        v["status"]["value"].as_str().unwrap_or("unknown")
    ));
    out.push(field("depends_on", &v["depends_on"]));
    out.push(field("blocked_by", &v["blocked_by"]));

    let nodes = v["nodes"].as_array().cloned().unwrap_or_default();
    out.push(String::new());
    let width = nodes
        .iter()
        .map(|n| n["issue"].as_str().unwrap_or("").len())
        .max()
        .unwrap_or(5)
        .max(5);
    out.push(format!(
        "{:<width$}  {:<18}  {:<5}  {:<8}  {}",
        "ISSUE",
        "READINESS",
        "WAVE",
        "UNBLOCKS",
        "TITLE",
        width = width
    ));
    for n in &nodes {
        out.push(format!(
            "{:<width$}  {:<18}  {:<5}  {:<8}  {}",
            n["issue"].as_str().unwrap_or(""),
            n["readiness"].as_str().unwrap_or(""),
            n["wave"].as_u64().map_or("—".to_string(), |w| w.to_string()),
            n["transitive_dependents"].as_u64().unwrap_or(0),
            n["title"].as_str().unwrap_or(""),
            width = width
        ));
    }

    out.push(String::new());
    for (label, key) in [
        ("ready", "ready"),
        ("active", "active"),
        ("review", "review"),
        ("completion_blocked", "completion_blocked"),
        ("blocked", "blocked"),
        ("waiting", "waiting"),
        ("complete", "complete"),
        ("cancelled", "cancelled"),
    ] {
        let set = ids(&v[key]);
        if !set.is_empty() {
            out.push(format!("  {label:<20} {set}"));
        }
    }

    if let Some(critical) = v["critical_blockers"].as_array() {
        if !critical.is_empty() {
            out.push(String::new());
            out.push("critical blockers — most unfinished work held back first".into());
            for c in critical {
                out.push(format!(
                    "  {:<10} {:<18} unblocks {:<4} {}",
                    c["issue"].as_str().unwrap_or(""),
                    c["readiness"].as_str().unwrap_or(""),
                    c["weight"].as_u64().unwrap_or(0),
                    ids(&c["blocks"])
                ));
            }
        }
    }

    if let Some(sets) = v["parallelizable"].as_array() {
        if !sets.is_empty() {
            out.push(String::new());
            out.push("parallelizable — subsets that may run at the same time".into());
            for (n, s) in sets.iter().enumerate() {
                out.push(format!(
                    "  set {} (wave {})  {}",
                    n + 1,
                    s["wave"].as_u64().unwrap_or(0),
                    ids(&s["issues"])
                ));
                for c in s["serialised"].as_array().unwrap_or(&Vec::new()) {
                    out.push(format!(
                        "      serialised: {} shares {} with {}",
                        c["excluded"].as_str().unwrap_or(""),
                        c["path"].as_str().unwrap_or(""),
                        c["held"].as_str().unwrap_or("")
                    ));
                }
            }
        }
    }

    if let Some(cycles) = v["cycles"].as_array() {
        for c in cycles {
            out.push(String::new());
            out.push(format!("cycle: {}", ids(c)));
        }
    }

    if let Some(diagnostics) = v["diagnostics"].as_array() {
        if !diagnostics.is_empty() {
            out.push(String::new());
            out.push("diagnostics".into());
            for x in diagnostics {
                out.push(format!(
                    "  {:<5} {:<26} {}",
                    x["level"].as_str().unwrap_or(""),
                    x["code"].as_str().unwrap_or(""),
                    x["message"].as_str().unwrap_or("")
                ));
            }
        }
    }

    let c = &v["counts"];
    out.push(String::new());
    out.push(format!(
        "{} issue(s), {} required",
        c["total"].as_u64().unwrap_or(0),
        c["required"].as_u64().unwrap_or(0)
    ));
    out.join("\n")
}
