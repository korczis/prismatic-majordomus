//! `majordomus connect`: what a client needs in front of it to reach this repository's one
//! shared MCP server, through the registry's own `connect.list`.
//!
//! It renders and nothing else. Which clients exist, where each keeps its configuration and
//! what this checkout holds for it are the capability's answer, so the command line, the
//! `majordomus_connect` tool and `GET /api/v1/connect` cannot disagree.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{ConnectArgs, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus connect`.
pub fn run(args: ConnectArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = ctx
        .registry
        .by_cli(&["connect".to_string()])
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus connect`".into(),
        })?;
    let input = match &args.client {
        Some(client) => json!({ "client": client }),
        None => json!({}),
    };
    let value = ctx.execute(id, input).map_err(map)?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let text = match args.format {
        OutputFormat::Json => pretty(&value),
        OutputFormat::Text => text(&value, args.client.is_some()),
    };
    writeln!(out, "{text}").map_err(Error::Transport)?;
    Ok(0)
}

fn text(v: &Value, one: bool) -> String {
    let s = |value: &Value, key: &str| value[key].as_str().unwrap_or("-").to_string();
    let mut lines = Vec::new();
    lines.push(format!("repository   {}", s(v, "repository")));
    lines.push(format!("server name  {}", s(v, "name")));
    let stdio = &v["stdio"];
    lines.push(format!(
        "launcher     {}{}",
        s(stdio, "command"),
        if stdio["present"].as_bool() == Some(true) {
            ""
        } else {
            "  (not in this checkout)"
        }
    ));
    let server = &v["server"];
    match server["mcp_url"].as_str() {
        Some(url) => lines.push(format!("endpoint     {url}  (a server is running)")),
        None => lines.push(format!(
            "endpoint     none  ({})",
            server["reason"].as_str().unwrap_or("no server is running")
        )),
    }
    lines.push(String::new());

    let clients: Vec<&Value> = v["clients"].as_array().into_iter().flatten().collect();
    let width = clients
        .iter()
        .map(|c| s(c, "id").chars().count())
        .max()
        .unwrap_or(6)
        .max(6);
    let titles = clients
        .iter()
        .map(|c| s(c, "title").chars().count())
        .max()
        .unwrap_or(5)
        .max(5);
    lines.push(format!(
        "{:<width$}  {:<titles$}  {:<12}  {}",
        "CLIENT",
        "TITLE",
        "KEPT IN",
        "CONFIGURATION",
        width = width,
        titles = titles
    ));
    for c in &clients {
        let where_ = match c["config_path"].as_str() {
            Some(path) if c["configured"].as_bool() == Some(true) => path.to_string(),
            Some(path) => format!("{path} (absent)"),
            None if c["setup"].is_string() => "its own settings".to_string(),
            None => "-".to_string(),
        };
        lines.push(format!(
            "{:<width$}  {:<titles$}  {:<12}  {}",
            s(c, "id"),
            s(c, "title"),
            s(c, "configured_in"),
            where_,
            width = width,
            titles = titles
        ));
    }

    // The configuration itself: for one named client always, for the whole set only where
    // there is a procedure a person must follow — a file at the root is already in front of
    // the client that reads it.
    for c in &clients {
        let show = one || (c["config_path"].is_null() && c["setup"].is_string());
        if !show {
            continue;
        }
        lines.push(String::new());
        lines.push(format!("--- {} ({})", s(c, "id"), s(c, "title")));
        if let Some(setup) = c["setup"].as_str() {
            lines.push(setup.trim_end().to_string());
        }
        if let Some(note) = c["note"].as_str() {
            lines.push(note.to_string());
        }
    }
    if !one {
        lines.push(String::new());
        lines.push(format!(
            "{} client(s); `majordomus connect <CLIENT>` prints one in full",
            clients.len()
        ));
    }
    lines.join("\n")
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}
