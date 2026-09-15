//! `majordomus mesh`: the terminal rendering of the mesh capabilities.
//!
//! `identity` answers in-process — the identity file is a fact of this machine, so the
//! local registry's own handler is truthful. `status` and `nodes` are facts of the
//! *running server*'s memory, so the command finds this checkout's server (through the
//! same `server.status` capability `serve status` renders) and asks it over HTTP; a
//! missing server is an answer with its reason, never an error. `doctor` is both: its
//! `runtime` check is the server's verdict on the declaration, so it is asked of the
//! server when one serves this checkout and run here when none does — and a failed
//! verdict exits 10, because a mesh declared on and not running is a contract unmet.
//! The command owns rendering and nothing else.

use std::time::Duration;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{MeshArgs, MeshCommand, MeshQueryArgs, OutputFormat};
use crate::error::{Error, Result};

/// One bounded request to the running server.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Run `majordomus mesh`.
pub fn run(args: MeshArgs) -> Result<u8> {
    match args.command {
        MeshCommand::Status(args) => from_server(args, "/api/v1/mesh", render_status),
        MeshCommand::Nodes(args) => from_server(args, "/api/v1/mesh/nodes", render_nodes),
        MeshCommand::Identity(args) => in_process(args, &["mesh", "identity"], render_identity),
        MeshCommand::Doctor(args) => doctor(args),
    }
}

/// `mesh doctor`: the server's report when one serves this checkout, this process's
/// otherwise; exit 10 when a check failed, 0 when every check holds.
fn doctor(args: MeshQueryArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let value = match ready_server(ctx)? {
        Some(url) => fetch(&url, "/api/v1/mesh/doctor")?,
        None => ctx.execute("mesh.doctor", json!({})).map_err(map)?,
    };
    emit(&args, &value, render_doctor);
    Ok(if value["ok"] == json!(true) { 0 } else { 10 })
}

/// The address of this checkout's ready server, when there is one.
fn ready_server(ctx: &crate::capability::handler::Context) -> Result<Option<String>> {
    let status = ctx.execute("server.status", json!({})).map_err(map)?;
    Ok(status["servers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["this_checkout"] == json!(true) && s["standing"] == json!("ready"))
        .and_then(|s| s["lease"]["url"].as_str())
        .map(str::to_string))
}

/// One bounded GET of `route` at the running server, as JSON.
fn fetch(url: &str, route: &str) -> Result<Value> {
    let reply = crate::mcp::bridge::request(url, "GET", route, &[], None, REQUEST_TIMEOUT)
        .map_err(|e| Error::Protocol {
            reason: format!("{url}{route}: {e}"),
        })?;
    if reply.status != 200 {
        return Err(Error::Protocol {
            reason: format!("{url}{route}: status {}", reply.status),
        });
    }
    serde_json::from_str(&reply.body).map_err(|e| Error::Protocol {
        reason: format!("{url}{route}: not JSON: {e}"),
    })
}

/// Execute the capability the CLI path names, in this process, and render it.
fn in_process(args: MeshQueryArgs, path: &[&str], render: fn(&Value) -> String) -> Result<u8> {
    let app = App::load(&args.repo)?;
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
    let value = ctx.execute(id, json!({})).map_err(map)?;
    emit(&args, &value, render);
    Ok(0)
}

/// Ask this checkout's running server for `route`, and render the answer. No running
/// server is an answer: the mesh lives in the server's memory and nowhere else.
fn from_server(args: MeshQueryArgs, route: &str, render: fn(&Value) -> String) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let Some(url) = ready_server(ctx)? else {
        let answer = json!({
            "active": false,
            "reason": "no running server for this checkout: the mesh lives inside the shared server (majordomus serve ensure starts one)",
        });
        emit(&args, &answer, render);
        return Ok(0);
    };
    let value = fetch(&url, route)?;
    emit(&args, &value, render);
    Ok(0)
}

fn emit(args: &MeshQueryArgs, value: &Value, render: fn(&Value) -> String) {
    match args.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_default()
            )
        }
        OutputFormat::Text => println!("{}", render(value)),
    }
}

fn render_status(v: &Value) -> String {
    let mut out = String::new();
    let active = v["active"].as_bool().unwrap_or(false);
    out.push_str(&format!(
        "mesh       {}\n",
        if active { "active" } else { "inactive" }
    ));
    if let Some(reason) = v["reason"].as_str() {
        out.push_str(&format!("reason     {reason}\n"));
    }
    if let Some(id) = v["identity"]["node_id"].as_str() {
        out.push_str(&format!(
            "node       {id} ({})\n",
            v["identity"]["display_name"].as_str().unwrap_or("")
        ));
    }
    if let Some(policy) = v["trust_policy"].as_str() {
        out.push_str(&format!("trust      {policy}\n"));
    }
    for p in v["providers"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "provider   {} {} sent {} received {}{}\n",
            p["id"].as_str().unwrap_or("?"),
            p["state"].as_str().unwrap_or("?"),
            p["sent"],
            p["received"],
            p["detail"]
                .as_str()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default(),
        ));
    }
    let t = &v["tallies"];
    out.push_str(&format!(
        "nodes      {} ({} trusted, {} present); accepted {}, replayed {}, expired {}",
        t["nodes"], t["trusted"], t["present"], t["accepted"], t["replayed"], t["expired"]
    ));
    out
}

fn render_nodes(v: &Value) -> String {
    if v["active"] == json!(false) {
        return render_status(v);
    }
    let nodes = v["nodes"].as_array().cloned().unwrap_or_default();
    if nodes.is_empty() {
        return "mesh       0 node(s) observed".into();
    }
    let mut out = format!("mesh       {} node(s)\n", nodes.len());
    for n in &nodes {
        let sources: Vec<&str> = n["sources"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s["source"].as_str())
            .collect();
        out.push_str(&format!(
            "{}  {}  trust {}  {}  [{}]  last seen {}\n",
            n["node_id"].as_str().unwrap_or("?"),
            n["display_name"].as_str().unwrap_or(""),
            n["trust"]["state"].as_str().unwrap_or("?"),
            n["presence"].as_str().unwrap_or("?"),
            sources.join(","),
            n["last_seen"].as_str().unwrap_or("?"),
        ));
    }
    out.trim_end().to_string()
}

fn render_identity(v: &Value) -> String {
    let mut out = String::new();
    out.push_str(&format!("present    {}\n", v["present"]));
    if let Some(path) = v["path"].as_str() {
        out.push_str(&format!("path       {path}\n"));
    }
    if let Some(id) = v["identity"]["node_id"].as_str() {
        out.push_str(&format!("node       {id}\n"));
        out.push_str(&format!(
            "name       {}\n",
            v["identity"]["display_name"].as_str().unwrap_or("")
        ));
        out.push_str(&format!(
            "key        {}\n",
            v["identity"]["public_key"].as_str().unwrap_or("")
        ));
    }
    if let Some(error) = v["error"].as_str() {
        out.push_str(&format!("error      {error}\n"));
    }
    out.trim_end().to_string()
}

fn render_doctor(v: &Value) -> String {
    let mut out = String::new();
    for c in v["checks"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{}  {:<12} {}\n",
            if c["ok"] == json!(true) {
                "ok  "
            } else {
                "FAIL"
            },
            c["check"].as_str().unwrap_or("?"),
            c["detail"].as_str().unwrap_or(""),
        ));
    }
    out.push_str(&format!(
        "verdict    {}",
        if v["ok"] == json!(true) {
            "every check holds"
        } else {
            "a check failed (see above)"
        }
    ));
    out
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}
