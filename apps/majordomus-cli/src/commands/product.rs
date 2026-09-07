//! `majordomus product`: the product model, through the registry's own `product.*`
//! capabilities.
//!
//! This file renders; it does not know anything. Every answer below is the output of the
//! capability the CLI exposure names, executed through the one executor, so the command
//! line, the HTTP routes and the MCP tools cannot answer the same question differently.
//! Nothing here names a feature, a module, a command, a kind or a provider.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, ProductArgs, ProductCommand};
use crate::error::{Error, Result};

/// The exit code when `validate` finds an error in the model.
pub const EXIT_INVALID: u8 = 10;

/// Run `majordomus product`.
pub fn run(args: ProductArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let format = args.format;
    match args.command.as_ref().unwrap_or(&ProductCommand::List) {
        ProductCommand::List => {
            let v = call(&app, &["product", "list"], query_of(&args))?;
            emit(format, &v, list_text)
        }
        ProductCommand::Show { id } => {
            let v = call(&app, &["product", "show"], json!({ "id": id }))?;
            emit(format, &v, feature_text)
        }
        ProductCommand::Matrix => {
            let v = call(&app, &["product", "matrix"], json!({}))?;
            emit(format, &v, matrix_text)
        }
        ProductCommand::Providers => {
            let v = call(&app, &["product", "providers"], json!({}))?;
            emit(format, &v, providers_text)
        }
        ProductCommand::Validate => {
            let v = call(&app, &["product", "validate"], json!({}))?;
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
fn query_of(f: &ProductArgs) -> Value {
    let mut o = serde_json::Map::new();
    let mut put = |k: &str, v: &Option<String>| {
        if let Some(v) = v {
            o.insert(k.into(), Value::String(v.clone()));
        }
    };
    put("area", &f.area);
    put("module", &f.module);
    put("command", &f.names_command);
    put("surface", &f.surface);
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

/// The surfaces of one feature as five marks, in the vocabulary's order, read from the
/// answer's own `surfaces` object rather than from a list here.
fn marks(surfaces: &Value) -> String {
    crate::product::SURFACES
        .iter()
        .map(|(id, _, _)| {
            if surfaces[id].as_bool().unwrap_or(false) {
                (*id).to_string()
            } else {
                "-".repeat(id.len())
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn list_text(v: &Value) -> String {
    let features = v["features"].as_array().cloned().unwrap_or_default();
    let width = features
        .iter()
        .map(|f| s(f, "id").len())
        .max()
        .unwrap_or(4)
        .max(4);
    let mut out = vec![format!(
        "{:<width$}  {:<26}  {}",
        "SLUG",
        "SURFACES",
        "TITLE",
        width = width
    )];
    for f in &features {
        out.push(format!(
            "{:<width$}  {:<26}  {}{}",
            s(f, "id"),
            marks(&f["surfaces"]),
            s(f, "title"),
            if f["featured"].as_bool().unwrap_or(false) {
                "  [featured]"
            } else {
                ""
            },
            width = width
        ));
    }
    let c = &v["counts"];
    out.push(String::new());
    out.push(format!(
        "{} feature(s) listed; {} stable, {} featured, {} provider(s); modules {}/{}, commands {}/{}, kinds {}/{} named by a feature  [{}]",
        features.len(),
        c["features"],
        c["featured"],
        c["providers"],
        c["modules_covered"],
        c["modules"],
        c["commands_covered"],
        c["commands"],
        c["kinds_covered"],
        c["kinds"],
        &v["fingerprint"].as_str().unwrap_or("")[..12.min(s(v, "fingerprint").len())],
    ));
    out.join("\n")
}

fn feature_text(v: &Value) -> String {
    let mut out = vec![
        format!("{}  {}", s(v, "id"), s(v, "title")),
        String::new(),
        format!("  {}", s(v, "headline")),
        format!("  {}", s(v, "summary")),
        String::new(),
        format!(
            "  status {}   weight {}   featured {}   route {}",
            s(v, "status"),
            v["weight"],
            v["featured"],
            s(v, "route")
        ),
        format!("  source {}", s(v, "source")),
        format!("  surfaces {}   (derived)", marks(&v["surfaces"])),
    ];
    let row = |label: &str, values: Vec<String>| -> Option<String> {
        (!values.is_empty()).then(|| format!("  {label:<18}{}", values.join(", ")))
    };
    out.push(String::new());
    for (label, key) in [
        ("areas", "areas"),
        ("audiences", "audiences"),
        ("modules", "modules"),
        ("commands", "commands"),
        ("kinds", "kinds"),
        ("rules", "rules"),
        ("docs", "docs"),
        ("adrs", "adrs"),
        ("claims", "claims"),
        ("use cases", "use_cases"),
        ("cockpit", "cockpit"),
        ("web", "web"),
        ("related", "related"),
        ("tags", "tags"),
    ] {
        out.extend(row(label, arr(v, key)));
    }
    out.push(String::new());
    out.push("  derived".into());
    let c = &v["counts"];
    out.push(format!(
        "    capabilities {}  mcp tools {}  mcp resources {}  http routes {}  cli paths {}  commands {}  objects {}  rules {} ({} enforced)  docs {}  adrs {}  claims {}  use cases {}  moments {}",
        c["capabilities"], c["mcp_tools"], c["mcp_resources"], c["http_routes"], c["cli_paths"], c["commands"], c["objects"], c["rules"], c["enforced_rules"], c["docs"], c["adrs"], c["claims"], c["use_cases"], c["moments"]
    ));
    if let Some(mods) = v["module_refs"].as_array() {
        for m in mods {
            out.push(format!("    module {} — {}", s(m, "id"), s(m, "title")));
            for c in m["capabilities"].as_array().into_iter().flatten() {
                out.push(format!(
                    "      {:<32} {:<9} mcp={} http={} cli={}",
                    s(c, "id"),
                    s(c, "kind"),
                    c["tool"].as_str().unwrap_or("-"),
                    c["route"].as_str().unwrap_or("-"),
                    c["cli"].as_str().unwrap_or("-"),
                ));
            }
        }
    }
    for (label, key, name, extra) in [
        ("commands", "command_refs", "id", "summary"),
        ("kinds", "kind_refs", "name", "objects"),
        ("rules", "rule_refs", "id", "class"),
        ("claims", "claim_refs", "id", "status"),
        ("moments", "moments", "id", "hook"),
    ] {
        if let Some(items) = v[key].as_array() {
            if !items.is_empty() {
                out.push(format!("    {label}"));
                for i in items {
                    let e = match &i[extra] {
                        Value::String(x) => x.clone(),
                        other => other.to_string(),
                    };
                    out.push(format!("      {:<40} {e}", s(i, name)));
                }
            }
        }
    }
    let ev = &v["evidence"]["claims"];
    if let Some(o) = ev.as_object() {
        if !o.is_empty() {
            out.push(format!(
                "    evidence: {}",
                o.iter()
                    .map(|(k, n)| format!("{n} {k}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    let back = arr(v, "backlinks");
    if !back.is_empty() {
        out.push(format!("    named by: {}", back.join(", ")));
    }
    out.join("\n")
}

fn matrix_text(v: &Value) -> String {
    let rows = v["rows"].as_array().cloned().unwrap_or_default();
    let width = rows
        .iter()
        .map(|r| s(r, "id").len())
        .max()
        .unwrap_or(7)
        .max(7);
    let header: String = crate::product::SURFACES
        .iter()
        .map(|(id, _, _)| *id)
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = vec![format!(
        "{:<width$}  {}  STATUS",
        "FEATURE",
        header,
        width = width
    )];
    for r in &rows {
        out.push(format!(
            "{:<width$}  {}  {}",
            s(r, "id"),
            marks(&r["surfaces"]),
            s(r, "status"),
            width = width
        ));
    }
    for (label, key) in [
        ("MODULE", "modules"),
        ("COMMAND", "commands"),
        ("KIND", "kinds"),
    ] {
        let items = v[key].as_array().cloned().unwrap_or_default();
        if items.is_empty() {
            continue;
        }
        let w = items
            .iter()
            .map(|c| s(c, "id").len())
            .max()
            .unwrap_or(4)
            .max(label.len());
        out.push(String::new());
        out.push(format!("{:<w$}  FEATURES", label, w = w));
        for c in &items {
            let fs = arr(c, "features");
            out.push(format!(
                "{:<w$}  {}",
                s(c, "id"),
                if fs.is_empty() {
                    "(none — a gap)".to_string()
                } else {
                    fs.join(", ")
                },
                w = w
            ));
        }
    }
    out.join("\n")
}

fn providers_text(v: &Value) -> String {
    let items = v["providers"].as_array().cloned().unwrap_or_default();
    let width = items
        .iter()
        .map(|p| s(p, "id").len())
        .max()
        .unwrap_or(8)
        .max(8);
    let mut out = vec![format!(
        "{:<width$}  {:<36}  BOOTSTRAPS  CLIENT CONFIG  HOOKS",
        "PROVIDER",
        "TITLE",
        width = width
    )];
    for p in &items {
        let boots: Vec<String> = p["bootstraps"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|b| s(b, "target"))
            .collect();
        out.push(format!(
            "{:<width$}  {:<36}  {:<10}  {:<13}  {}",
            s(p, "id"),
            s(p, "title"),
            if boots.is_empty() {
                "-".to_string()
            } else {
                boots.join(",")
            },
            p["client_config"].as_str().unwrap_or("-"),
            {
                let h = arr(p, "hooks");
                if h.is_empty() {
                    "-".to_string()
                } else {
                    h.join(",")
                }
            },
            width = width
        ));
    }
    out.push(String::new());
    out.push(format!("{} provider(s)", items.len()));
    out.join("\n")
}

fn validation_text(v: &Value) -> String {
    let mut out = Vec::new();
    for f in v["findings"].as_array().into_iter().flatten() {
        let mut line = format!("{:<8} {}", s(f, "severity").to_uppercase(), s(f, "path"));
        if let Some(id) = f["id"].as_str() {
            line.push_str(&format!(" [{id}]"));
        }
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
        "{} feature(s) ({} of every status), {} featured, {} provider(s); modules {}/{}, commands {}/{}, kinds {}/{} named by a feature",
        c["features"], c["features_all"], c["featured"], c["providers"], c["modules_covered"], c["modules"], c["commands_covered"], c["commands"], c["kinds_covered"], c["kinds"]
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
