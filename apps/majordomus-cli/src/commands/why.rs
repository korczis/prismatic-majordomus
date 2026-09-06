//! `majordomus why`: the operational moments, through the registry's own `why.*`
//! capabilities.
//!
//! This file renders; it does not know anything. Every answer below is the output of the
//! capability the CLI exposure names, executed through the one executor, so the command
//! line, the HTTP routes and the MCP tools cannot answer the same question differently.
//! Nothing here names a moment, an audience or an area.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, WhyArgs, WhyCommand};
use crate::error::{Error, Result};

/// The exit code when `validate` finds an error in the catalogue.
pub const EXIT_INVALID: u8 = 10;

/// Run `majordomus why`.
pub fn run(args: WhyArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let format = args.format;
    // `why` with nothing after it lists, because listing is what a person wants when they
    // ask what this section holds.
    match args.command.as_ref().unwrap_or(&WhyCommand::List) {
        WhyCommand::List => {
            let v = call(&app, &["why", "list"], query_of(&args))?;
            emit(format, &v, list_text)
        }
        WhyCommand::Show { id } => {
            let v = call(&app, &["why", "show"], json!({ "id": id }))?;
            emit(format, &v, moment_text)
        }
        WhyCommand::Audiences => {
            let v = call(&app, &["why", "audiences"], json!({}))?;
            emit(format, &v, |v| members_text(v, "audiences"))
        }
        WhyCommand::Areas => {
            let v = call(&app, &["why", "areas"], json!({}))?;
            emit(format, &v, |v| members_text(v, "areas"))
        }
        WhyCommand::Diagnose { signals } if signals.is_empty() => {
            // Without a selection there is nothing to diagnose, so the questionnaire is
            // printed instead of an empty answer. It is built from the signals of the
            // catalogue and from nothing else.
            let all = call(&app, &["why", "list"], json!({}))?;
            let full = questionnaire(&app, &all)?;
            emit(format, &full, questionnaire_text)
        }
        WhyCommand::Diagnose { signals } => {
            let v = call(
                &app,
                &["why", "diagnose"],
                json!({ "signals": signals.join(",") }),
            )?;
            emit(format, &v, diagnosis_text)
        }
        WhyCommand::Validate => {
            let v = call(&app, &["why", "validate"], json!({}))?;
            let code = if v["valid"].as_bool().unwrap_or(false) {
                0
            } else {
                EXIT_INVALID
            };
            emit(format, &v, validation_text)?;
            Ok(code)
        }
    }
}

/// Every facet the command line offers, as the capability's own input. A facet added to
/// the query type is a flag here and nowhere else.
fn query_of(f: &WhyArgs) -> Value {
    let mut o = serde_json::Map::new();
    let mut put = |k: &str, v: &Option<String>| {
        if let Some(v) = v {
            o.insert(k.into(), Value::String(v.clone()));
        }
    };
    put("audience", &f.audience);
    put("area", &f.area);
    put("tag", &f.tag);
    put("severity", &f.severity);
    put("frequency", &f.frequency);
    put("lifecycle", &f.lifecycle);
    put("capability", &f.capability);
    put("command", &f.names_command);
    put("q", &f.query);
    if f.featured {
        o.insert("featured".into(), Value::Bool(true));
    }
    if f.all {
        o.insert("status".into(), Value::String("any".into()));
    }
    Value::Object(o)
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
    ctx.execute(&id, input).map_err(map)
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: match e {
            CapabilityError::InvalidInput(reason) | CapabilityError::NotFound(reason) => reason,
            other => other.to_string(),
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

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}

fn arr(v: &Value, k: &str) -> Vec<String> {
    v[k].as_array()
        .map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn list_text(v: &Value) -> String {
    let moments = v["moments"].as_array().cloned().unwrap_or_default();
    let width = moments
        .iter()
        .map(|m| s(m, "id").len())
        .max()
        .unwrap_or(4)
        .max(4);
    let mut out = vec![format!("{:<width$}  {}", "SLUG", "TITLE", width = width)];
    for m in &moments {
        out.push(format!(
            "{:<width$}  {}",
            s(m, "id"),
            s(m, "title"),
            width = width
        ));
    }
    let c = &v["counts"];
    out.push(String::new());
    out.push(format!(
        "{} of {} moment(s), {} audience(s), {} area(s), {} signal(s)  [{}]",
        moments.len(),
        c["moments"],
        c["audiences"],
        c["areas"],
        c["signals"],
        &v["fingerprint"].as_str().unwrap_or("")[..12.min(s(v, "fingerprint").len())],
    ));
    out.join("\n")
}

fn moment_text(v: &Value) -> String {
    let mut out = vec![
        format!("{}  {}", s(v, "id"), s(v, "title")),
        String::new(),
        format!("  {}", s(v, "summary")),
        String::new(),
        format!(
            "  status {}   severity {}   frequency {}   route {}",
            s(v, "status"),
            s(v, "severity"),
            s(v, "frequency"),
            s(v, "route")
        ),
        format!("  source {}", s(v, "source")),
    ];
    let row = |label: &str, values: Vec<String>| -> Option<String> {
        (!values.is_empty()).then(|| format!("  {label:<18}{}", values.join(", ")))
    };
    out.push(String::new());
    for (label, key) in [
        ("audiences", "audiences"),
        ("areas", "areas"),
        ("lifecycle", "lifecycle"),
        ("tags", "tags"),
        ("commands", "commands"),
        ("capabilities", "capabilities"),
        ("claims", "claims"),
        ("doctrines", "doctrines"),
        ("use cases", "use_cases"),
    ] {
        out.extend(row(label, arr(v, key)));
    }
    out.push(String::new());
    out.push("  derived".into());
    for (label, key) in [
        ("responsibilities", "responsibilities"),
        ("named by", "backlinks"),
        ("related", "related"),
        ("similar", "similar"),
    ] {
        out.extend(row(label, arr(v, key)).map(|l| format!("  {l}")));
    }
    if let Some(signals) = v["signals"].as_array() {
        out.push(String::new());
        out.push("  signals".into());
        for sig in signals {
            out.push(format!("    {:<28}{}", s(sig, "id"), s(sig, "text")));
        }
    }
    if let Some(examples) = v["examples"].as_array() {
        out.push(String::new());
        out.push("  examples".into());
        for e in examples {
            out.push(format!(
                "    {} ({})  {}",
                s(e, "id"),
                s(e, "audience"),
                s(e, "title")
            ));
            out.push(format!("      before  {}", s(e, "before")));
            out.push(format!("      after   {}", s(e, "after")));
        }
    }
    out.join("\n")
}

fn members_text(v: &Value, key: &str) -> String {
    let items = v[key].as_array().cloned().unwrap_or_default();
    let width = items
        .iter()
        .map(|a| s(a, "id").len())
        .max()
        .unwrap_or(4)
        .max(4);
    let mut out = vec![format!(
        "{:<width$}  {:>5}  {}",
        "SLUG",
        "N",
        "TITLE",
        width = width
    )];
    for a in &items {
        out.push(format!(
            "{:<width$}  {:>5}  {}",
            s(a, "id"),
            a["count"].as_u64().unwrap_or(0),
            s(a, "title"),
            width = width
        ));
    }
    out.join("\n")
}

/// The questionnaire, assembled from the catalogue's own signals. Nothing here is a list
/// of questions: a moment that declares a signal adds a line, and one that declares none
/// adds nothing.
fn questionnaire(app: &App, all: &Value) -> Result<Value> {
    let mut rows = Vec::new();
    for m in all["moments"].as_array().into_iter().flatten() {
        let detail = call(app, &["why", "show"], json!({ "id": s(m, "id") }))?;
        for sig in detail["signals"].as_array().into_iter().flatten() {
            rows.push(json!({
                "signal": s(sig, "id"),
                "text": s(sig, "text"),
                "moment": s(m, "id"),
                "areas": m["areas"].clone(),
            }));
        }
    }
    Ok(json!({ "questions": rows, "counts": all["counts"].clone() }))
}

fn questionnaire_text(v: &Value) -> String {
    let rows = v["questions"].as_array().cloned().unwrap_or_default();
    let mut out = vec![
        "Which of these happened to you this week? Pass each one you recognise:".into(),
        "  majordomus why diagnose --signal <id> --signal <id> ...".into(),
        String::new(),
    ];
    for r in &rows {
        out.push(format!("  [ ] {:<32}{}", s(r, "signal"), s(r, "text")));
    }
    out.push(String::new());
    out.push(format!("{} question(s)", rows.len()));
    out.join("\n")
}

fn diagnosis_text(v: &Value) -> String {
    let mut out = Vec::new();
    let moments = arr(v, "moments");
    out.push(format!(
        "{} moment(s) matched: {}",
        moments.len(),
        moments.join(", ")
    ));
    for u in arr(v, "unresolved") {
        out.push(format!(
            "  unresolved: {u} — no signal or moment of that name"
        ));
    }
    let block = |label: &str, key: &str| -> Vec<String> {
        let items = v[key].as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            return Vec::new();
        }
        let mut lines = vec![String::new(), format!("{label}")];
        for r in items {
            lines.push(format!(
                "  {:<34}{:>3}   {}",
                s(&r, "id"),
                r["count"].as_u64().unwrap_or(0),
                arr(&r, "matched_because").join(", ")
            ));
        }
        lines
    };
    out.extend(block("your operational load, by area", "areas"));
    out.extend(block("the audiences this looks like", "audiences"));
    out.extend(block("capabilities that answer it", "capabilities"));
    out.extend(block("commands that answer it", "commands"));
    out.extend(block("what is guaranteed here", "claims"));
    out.extend(block("rules that govern it", "doctrines"));
    out.extend(block("use cases that show the way out", "use_cases"));
    let also = arr(v, "also_worth_reading");
    if !also.is_empty() {
        out.push(String::new());
        out.push(format!("also worth reading: {}", also.join(", ")));
    }
    out.join("\n")
}

fn validation_text(v: &Value) -> String {
    let mut out = Vec::new();
    for f in v["findings"].as_array().into_iter().flatten() {
        let mut line = format!("{:<8} {}", s(f, "severity").to_uppercase(), s(f, "path"));
        if let Some(field) = f["field"].as_str() {
            line.push_str(&format!(":{field}"));
        }
        out.push(line);
        out.push(format!("         {}", s(f, "message")));
        if let Some(did) = f["did_you_mean"].as_str() {
            out.push(format!("         did you mean: {did}"));
        }
    }
    if !out.is_empty() {
        out.push(String::new());
    }
    let c = &v["counts"];
    out.push(format!(
        "{} moment(s), {} audience(s), {} area(s), {} signal(s), {} example(s)",
        c["moments"], c["audiences"], c["areas"], c["signals"], c["examples"]
    ));
    out.push(format!(
        "{}: {} error(s), {} warning(s)",
        if v["valid"].as_bool().unwrap_or(false) {
            "valid"
        } else {
            "INVALID"
        },
        v["errors"],
        v["warnings"]
    ));
    out.join("\n")
}
