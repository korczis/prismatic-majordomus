//! `majordomus models`: the terminal rendering of the model catalogue capabilities.
//! Both subcommands execute the capability the CLI path names in-process — the
//! catalogue is a file of the share directory, so the local answer is the truth — and
//! own the rendering and nothing else: the filtering, the routing and every reason
//! live in the capability, and `--format json` prints the same answer the HTTP route
//! and the MCP tool serve.

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{ModelsArgs, ModelsCommand, ModelsListArgs, ModelsRouteArgs, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus models`.
pub fn run(args: ModelsArgs) -> Result<u8> {
    match args.command {
        ModelsCommand::List(args) => list(args),
        ModelsCommand::Route(args) => route(args),
    }
}

fn execute(repo: &crate::cli::RepoArgs, path: &[&str], input: Value) -> Result<Value> {
    let app = App::load(repo)?;
    let ctx = &app.context;
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

fn list(args: ModelsListArgs) -> Result<u8> {
    let value = execute(
        &args.repo,
        &["models", "list"],
        json!({ "vendor": args.vendor, "capability": args.capability, "id": args.id }),
    )?;
    match args.format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ),
        OutputFormat::Text => {
            let mut out = String::new();
            for v in value["vendors"].as_array().into_iter().flatten() {
                let credential = match v["credential_configured"] {
                    Value::Bool(true) => "credential configured",
                    Value::Bool(false) => "credential not configured",
                    _ => "no credential declared",
                };
                out.push_str(&format!(
                    "vendor     {}  {}  {}  {credential}\n",
                    v["id"].as_str().unwrap_or("?"),
                    v["title"].as_str().unwrap_or(""),
                    v["inference"].as_str().unwrap_or("remote"),
                ));
            }
            out.push_str(&format!("models     {} model(s)\n", value["count"]));
            for m in value["models"].as_array().into_iter().flatten() {
                let caps: Vec<&str> = m["capabilities"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                out.push_str(&format!(
                    "{}  {}  {}  ctx {}  [{}]{}\n",
                    m["id"].as_str().unwrap_or("?"),
                    m["vendor"].as_str().unwrap_or("?"),
                    m["status"].as_str().unwrap_or("available"),
                    m["context_window"]
                        .as_u64()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".into()),
                    caps.join(","),
                    m["note"]
                        .as_str()
                        .map(|n| format!("  — {n}"))
                        .unwrap_or_default(),
                ));
            }
            for d in value["diagnostics"].as_array().into_iter().flatten() {
                out.push_str(&format!("finding    {}\n", d.as_str().unwrap_or("")));
            }
            println!("{}", out.trim_end());
        }
    }
    Ok(0)
}

fn route(args: ModelsRouteArgs) -> Result<u8> {
    let value = execute(
        &args.repo,
        &["models", "route"],
        json!({
            "require": args.require,
            "min_context": args.min_context,
            "vendor": args.vendor,
            "local_only": args.local_only,
            "model": args.model,
        }),
    )?;
    match args.format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ),
        OutputFormat::Text => {
            let mut out = String::new();
            match value["selected"].as_str() {
                Some(id) => {
                    out.push_str(&format!("selected   {id}\n"));
                    if let Some(reason) = value["reason"].as_str() {
                        out.push_str(&format!("reason     {reason}\n"));
                    }
                }
                None => out.push_str("selected   nothing qualifies\n"),
            }
            let fallbacks: Vec<&str> = value["fallbacks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            if !fallbacks.is_empty() {
                out.push_str(&format!("fallbacks  {}\n", fallbacks.join(" → ")));
            }
            for e in value["excluded"].as_array().into_iter().flatten() {
                out.push_str(&format!(
                    "excluded   {}  — {}\n",
                    e["model"].as_str().unwrap_or("?"),
                    e["reason"].as_str().unwrap_or(""),
                ));
            }
            println!("{}", out.trim_end());
        }
    }
    Ok(0)
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}
