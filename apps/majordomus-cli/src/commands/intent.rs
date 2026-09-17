//! `majordomus intent`: the intents, through the registry's own `intent.*` capabilities.
//!
//! This file renders; it decides nothing. Every answer is the output of the capability the
//! CLI exposure names, executed through the one executor, so the command line, the HTTP
//! routes and the MCP tools cannot answer the same question differently. The two verdicts
//! — `validate` and `preflight` — are read out of that output and turned into exit 10.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{IntentArgs, IntentCommand, OutputFormat};
use crate::error::{Error, Result};

/// The exit code when `validate` finds a failure or `preflight` refuses.
pub const EXIT_INVALID: u8 = 10;

/// Run `majordomus intent`.
pub fn run(args: IntentArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let format = args.format;
    match &args.command {
        IntentCommand::List => {
            let v = call(&app, &["intent", "list"], json!({}))?;
            emit(format, &v, list_text)?;
            Ok(0)
        }
        IntentCommand::Show { id } => {
            let v = call(&app, &["intent", "show"], json!({ "id": id }))?;
            emit(format, &v, show_text)?;
            Ok(0)
        }
        IntentCommand::Validate => {
            let v = call(&app, &["intent", "validate"], json!({}))?;
            emit(format, &v, validate_text)?;
            Ok(if v["valid"].as_bool() == Some(true) {
                0
            } else {
                EXIT_INVALID
            })
        }
        IntentCommand::Coverage => {
            let v = call(&app, &["intent", "coverage"], json!({}))?;
            emit(format, &v, coverage_text)?;
            Ok(0)
        }
        IntentCommand::Preflight { issue, paths } => {
            let mut input = json!({ "paths": paths.join(",") });
            if let Some(issue) = issue {
                input["issue"] = json!(issue);
            }
            let v = call(&app, &["intent", "preflight"], input)?;
            emit(format, &v, preflight_text)?;
            Ok(if v["verdict"] == "serves" {
                0
            } else {
                EXIT_INVALID
            })
        }
    }
}

fn call(app: &App, path: &[&str], input: Value) -> Result<Value> {
    let ctx = &app.context;
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
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
    ctx.execute(&id, input).map_err(|e| match e {
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })
}

fn emit(format: OutputFormat, v: &Value, text: impl Fn(&Value) -> String) -> Result<()> {
    let body = match format {
        OutputFormat::Json => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
        OutputFormat::Text => text(v),
    };
    writeln!(std::io::stdout().lock(), "{body}").map_err(Error::Transport)
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}

fn list_text(v: &Value) -> String {
    let intents = v["intents"].as_array().cloned().unwrap_or_default();
    let width = intents
        .iter()
        .map(|i| s(i, "id").len())
        .max()
        .unwrap_or(2)
        .max(2);
    let mut out = vec![format!(
        "{:<width$}  {:<10}  {:<7}  TITLE",
        "ID", "STAGE", "MET"
    )];
    for i in &intents {
        let total = i["satisfaction"].as_array().map_or(0, Vec::len);
        out.push(format!(
            "{:<width$}  {:<10}  {:<7}  {}",
            s(i, "id"),
            s(i, "stage"),
            format!("{}/{total}", i["met"]),
            s(i, "title"),
        ));
    }
    out.push(String::new());
    out.push(format!("{} intent(s)", v["count"]));
    out.join("\n")
}

/// Every criterion with the work that carries it, then every issue with the reason it exists.
fn coverage_text(v: &Value) -> String {
    let criteria = v["criteria"].as_array().cloned().unwrap_or_default();
    let mut out = vec![format!("{:<28}  {:<9}  ISSUES", "CRITERION", "STRENGTH")];
    for c in &criteria {
        let issues: Vec<&str> = c["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|i| s(i, "id"))
            .collect();
        out.push(format!(
            "{:<28}  {:<9}  {}",
            format!("{}#{}", s(c, "intent"), s(c, "criterion")),
            s(c, "strength"),
            if issues.is_empty() {
                "—".into()
            } else {
                issues.join(" ")
            },
        ));
    }
    let issues = v["issues"].as_array().cloned().unwrap_or_default();
    let mut origins: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for i in &issues {
        *origins.entry(s(i, "origin")).or_default() += 1;
    }
    out.push(String::new());
    out.push(format!(
        "{} criterion(s); {} issue(s): {}",
        criteria.len(),
        issues.len(),
        origins
            .iter()
            .map(|(o, n)| format!("{n} {o}"))
            .collect::<Vec<_>>()
            .join(", "),
    ));
    out.join("\n")
}

fn show_text(v: &Value) -> String {
    let mut out = vec![
        format!("{}  {}", s(v, "id"), s(v, "title")),
        String::new(),
        format!("  {}", s(v, "statement")),
        String::new(),
        format!("  stage {}   source {}", s(v, "stage"), s(v, "source")),
    ];
    for inv in v["invariants"].as_array().into_iter().flatten() {
        out.push(format!("  invariant   {}", inv.as_str().unwrap_or("")));
    }
    for m in v["milestones"].as_array().into_iter().flatten() {
        out.push(format!(
            "  milestone   {}  {}",
            s(m, "id"),
            m["status"].as_str().unwrap_or("unresolved")
        ));
    }
    for c in v["satisfaction"].as_array().into_iter().flatten() {
        out.push(format!(
            "  criterion   {}  {}  {} {}{}",
            s(c, "id"),
            s(c, "state"),
            s(c, "evidence"),
            s(c, "ref"),
            c["reproduce"]
                .as_str()
                .map(|r| format!("  [reproduce: {r}]"))
                .unwrap_or_default()
        ));
    }
    for g in v["governance"].as_array().into_iter().flatten() {
        out.push(format!("  governance  {}", g.as_str().unwrap_or("")));
    }
    out.join("\n")
}

fn findings_text(out: &mut Vec<String>, findings: &Value) {
    for f in findings.as_array().into_iter().flatten() {
        out.push(format!(
            "{} {}  {}: {}  [reproduce: {}]",
            s(f, "level"),
            s(f, "code"),
            s(f, "subject"),
            s(f, "message"),
            s(f, "reproduce"),
        ));
    }
}

fn validate_text(v: &Value) -> String {
    let mut out = Vec::new();
    findings_text(&mut out, &v["findings"]);
    out.push(format!(
        "{} intent(s), {} failure(s), {} warning(s): {}",
        v["intents"],
        v["failures"],
        v["warnings"],
        if v["valid"].as_bool() == Some(true) {
            "valid"
        } else {
            "invalid"
        }
    ));
    out.join("\n")
}

fn preflight_text(v: &Value) -> String {
    let mut out = vec![format!("verdict     {}", s(v, "verdict"))];
    let issues: Vec<&str> = v["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !issues.is_empty() {
        out.push(format!("issues      {}", issues.join(", ")));
    }
    for m in v["matches"].as_array().into_iter().flatten() {
        out.push(format!(
            "intent      {}  {}  via milestone {} and issue {}",
            s(m, "intent"),
            s(m, "stage"),
            s(m, "milestone"),
            s(m, "issue"),
        ));
    }
    for g in v["governance"].as_array().into_iter().flatten() {
        out.push(format!("governance  {}", g.as_str().unwrap_or("")));
    }
    if let Some(r) = v["refusal"].as_str() {
        out.push(format!("refusal     {r}"));
    }
    out.join("\n")
}
