//! `majordomus models`: the terminal rendering of the model catalogue capabilities.
//! Every subcommand executes the capability the CLI path names in-process — the
//! catalogue is a file of the share directory, so the local answer is the truth — and
//! own the rendering and nothing else: the filtering, the routing and every reason
//! live in the capability, and `--format json` prints the same answer the HTTP route
//! and the MCP tool serve.

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{
    ModelsArgs, ModelsCommand, ModelsListArgs, ModelsObserveArgs, ModelsObservedArgs,
    ModelsRouteArgs, OutputFormat,
};
use crate::error::{Error, Result};

/// Run `majordomus models`.
pub fn run(args: ModelsArgs) -> Result<u8> {
    match args.command {
        ModelsCommand::List(args) => list(args),
        ModelsCommand::Route(args) => route(args),
        ModelsCommand::Observe(args) => observe(args),
        ModelsCommand::Observed(args) => observed(args),
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
            "observed": args.observed,
            "min_evidence": args.min_evidence.map(|e| e.as_str()),
            "max_age": args.max_age,
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
            if args.observed && value["observed"].is_null() {
                out.push_str("observed   none recorded; run scripts/inference-observe\n");
            }
            if let Some(o) = value["observed"].as_object() {
                out.push_str(&format!(
                    "observed   {}  {}\n",
                    o["recorded_at"].as_str().unwrap_or("?"),
                    if o["stale"] == Value::Bool(true) {
                        "stale"
                    } else {
                        "fresh"
                    },
                ));
            }
            println!("{}", out.trim_end());
        }
    }
    Ok(0)
}

fn observe(args: ModelsObserveArgs) -> Result<u8> {
    use std::io::Read;
    let text = match &args.file {
        Some(path) => std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?,
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| Error::Protocol {
                    reason: format!("standard input: {e}"),
                })?;
            s
        }
    };
    let report: Value = serde_json::from_str(&text).map_err(|e| Error::Protocol {
        reason: format!("the observation is not JSON: {e}"),
    })?;
    let value = execute(&args.repo, &["models", "observe"], report)?;
    match args.format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ),
        OutputFormat::Text => println!("{}", observation_text(&value).trim_end()),
    }
    Ok(0)
}

fn observed(args: ModelsObservedArgs) -> Result<u8> {
    let value = execute(
        &args.repo,
        &["models", "observed"],
        json!({ "max_age": args.max_age }),
    )?;
    match args.format {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ),
        OutputFormat::Text => {
            let mut out = String::new();
            match value["observation"].as_object() {
                None => out.push_str(&format!(
                    "observation  none recorded in {}; run {}\n",
                    value["path"].as_str().unwrap_or("?"),
                    value["observe_with"]
                        .as_str()
                        .unwrap_or("scripts/inference-observe"),
                )),
                Some(_) => {
                    out.push_str(&format!(
                        "observation  {}  {} s old, {}  ({} model(s))\n",
                        value["observation"]["recorded_at"].as_str().unwrap_or("?"),
                        value["age_seconds"]
                            .as_u64()
                            .map(|a| a.to_string())
                            .unwrap_or_else(|| "?".into()),
                        if value["stale"] == Value::Bool(true) {
                            "stale"
                        } else {
                            "fresh"
                        },
                        value["models"],
                    ));
                    out.push_str(&observation_text(&value["observation"]));
                }
            }
            println!("{}", out.trim_end());
        }
    }
    Ok(0)
}

/// One line per runtime, one per model with each capability word and its evidence.
fn observation_text(o: &Value) -> String {
    let mut out = String::new();
    for r in o["runtimes"].as_array().into_iter().flatten() {
        let detail = match r["state"].as_str() {
            Some("identified") => {
                let models = r["models"].as_array().map_or(0, Vec::len);
                let loaded = r["models"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|m| m["loaded"] == Value::Bool(true))
                    .count();
                format!(
                    "{}{models} model(s), {loaded} loaded",
                    r["version"]
                        .as_str()
                        .map(|v| format!("{v}  "))
                        .unwrap_or_default()
                )
            }
            _ => r["diagnostic"].as_str().unwrap_or("").to_string(),
        };
        out.push_str(&format!(
            "runtime    {}  {}  {}  {}  {detail}\n",
            r["vendor"].as_str().unwrap_or("?"),
            r["endpoint"].as_str().unwrap_or("?"),
            r["locality"].as_str().unwrap_or("?"),
            r["state"].as_str().unwrap_or("?"),
        ));
        for m in r["models"].as_array().into_iter().flatten() {
            let words: Vec<String> = m["standing"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| {
                    let word = c["word"].as_str().unwrap_or("?");
                    if c["refuted"] == Value::Bool(true) {
                        format!("{word}:refuted")
                    } else {
                        format!("{word}:{}", c["evidence"].as_str().unwrap_or("?"))
                    }
                })
                .collect();
            let ctx = &m["context_window"];
            out.push_str(&format!(
                "  model    {}  {}  ctx {}  [{}]\n",
                m["id"].as_str().unwrap_or("?"),
                match m["loaded"] {
                    Value::Bool(true) => "loaded",
                    Value::Bool(false) => "not loaded",
                    _ => "load unknown",
                },
                ctx["configured"]
                    .as_u64()
                    .or_else(|| ctx["advertised"].as_u64())
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".into()),
                words.join(" "),
            ));
        }
    }
    for x in o["refused"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "refused    {}  {}  — {}\n",
            x["vendor"].as_str().unwrap_or("?"),
            x["endpoint"].as_str().unwrap_or("?"),
            x["reason"].as_str().unwrap_or(""),
        ));
    }
    out
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}
