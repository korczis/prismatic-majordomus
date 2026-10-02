//! `majordomus skills`: every skill as a proven capability, through the registry's own
//! `skills.*` capabilities.
//!
//! This file renders; it decides nothing. Every answer is the output of the capability the
//! command line exposure names, run through the one executor, so the command line, the HTTP
//! routes and the MCP tools cannot answer the same question differently. The one verdict,
//! `verify`, is read out of that output and turned into exit 10.
//!
//! The text rendering keeps the four facts apart. `proven` and `inputs_unchanged` print
//! differently, and an orphan prints which half it lacks, because a column that showed a tick
//! for "a test names it" would be the badge whose derivation cannot be inspected.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, SkillsArgs, SkillsCommand};
use crate::error::{Error, Result};

/// The exit code when `verify` finds a failure.
pub const EXIT_INVALID: u8 = 10;

/// Run `majordomus skills`.
pub fn run(args: SkillsArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let format = args.format;
    match &args.command {
        SkillsCommand::Status => {
            let v = call(&app, &["skills", "status"], json!({}))?;
            emit(format, &v, status_text)?;
            Ok(0)
        }
        SkillsCommand::Explain { id } => {
            let v = call(&app, &["skills", "explain"], json!({ "id": id }))?;
            emit(format, &v, explain_text)?;
            Ok(0)
        }
        SkillsCommand::Verify => {
            let v = call(&app, &["skills", "verify"], json!({}))?;
            emit(format, &v, verify_text)?;
            Ok(if v["valid"].as_bool() == Some(true) {
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

fn yes(b: &Value) -> &'static str {
    if b.as_bool() == Some(true) {
        "yes"
    } else {
        "no"
    }
}

fn status_text(v: &Value) -> String {
    let skills = v["skills"].as_array().cloned().unwrap_or_default();
    let width = skills
        .iter()
        .map(|i| s(i, "id").len())
        .max()
        .unwrap_or(2)
        .max(2);
    let mut out = vec![format!(
        "{:<width$}  {:<10}  {:<12}  {:<16}  {:<10}  {:<8}  USED",
        "ID", "STATUS", "STANDING", "TESTED", "DOCUMENTED", "ENFORCED"
    )];
    for i in &skills {
        out.push(format!(
            "{:<width$}  {:<10}  {:<12}  {:<16}  {:<10}  {:<8}  {}",
            s(i, "id"),
            s(i, "status"),
            s(i, "standing"),
            i["tested"]["state"].as_str().unwrap_or(""),
            yes(&i["documented"]["documented"]),
            yes(&i["enforced"]["enforced"]),
            yes(&i["used"]["used"]),
        ));
    }
    out.push(String::new());
    out.push(format!("{} skill(s)", v["count"]));
    out.join("\n")
}

fn explain_text(v: &Value) -> String {
    let mut out = vec![
        format!("{}  {}", s(v, "id"), s(v, "title")),
        String::new(),
        format!("  {}", s(v, "description")),
        String::new(),
        format!(
            "  status {}   version {}   standing {}   source {}",
            s(v, "status"),
            v["version"],
            s(v, "standing"),
            s(v, "path")
        ),
    ];
    if let Some(p) = v["provenance"].as_object() {
        out.push(format!(
            "  provenance  {} {} {}",
            p.get("origin").and_then(Value::as_str).unwrap_or(""),
            p.get("ledger").and_then(Value::as_str).unwrap_or(""),
            p.get("decision").and_then(Value::as_str).unwrap_or("")
        ));
    }
    out.push(format!(
        "  tested      {}",
        v["tested"]["state"].as_str().unwrap_or("")
    ));
    for t in v["tested"]["tests"].as_array().into_iter().flatten() {
        out.push(format!(
            "    test      {}  {}{}",
            s(t, "path"),
            s(t, "state"),
            t["reproduce"]
                .as_str()
                .map(|r| format!("  [reproduce: {r}]"))
                .unwrap_or_default()
        ));
    }
    out.push(format!(
        "  documented  {}  {}",
        yes(&v["documented"]["documented"]),
        s(&v["documented"], "page")
    ));
    let gates: Vec<&str> = v["enforced"]["gates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    out.push(format!(
        "  enforced    {}  doctrine {}  gates {}",
        yes(&v["enforced"]["enforced"]),
        v["enforced"]["doctrine"].as_str().unwrap_or("-"),
        if gates.is_empty() {
            "-".to_string()
        } else {
            gates.join(", ")
        }
    ));
    out.push(format!("  used        {}", yes(&v["used"]["used"])));
    for i in v["used"]["invocations"].as_array().into_iter().flatten() {
        out.push(format!(
            "    invoked   {}:{}  {}",
            s(i, "path"),
            i["line"],
            s(i, "reference")
        ));
    }
    findings_text(&mut out, &v["findings"]);
    out.join("\n")
}

fn findings_text(out: &mut Vec<String>, findings: &Value) {
    for f in findings.as_array().into_iter().flatten() {
        out.push(format!(
            "{} {}  {}: {}  [reproduce: {}]",
            s(f, "level").to_uppercase(),
            s(f, "code"),
            s(f, "subject"),
            s(f, "message"),
            s(f, "reproduce"),
        ));
    }
}

fn verify_text(v: &Value) -> String {
    let mut out = Vec::new();
    findings_text(&mut out, &v["findings"]);
    out.push(format!(
        "{} skill(s), {} failure(s), {} warning(s): {}",
        v["skills"],
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
