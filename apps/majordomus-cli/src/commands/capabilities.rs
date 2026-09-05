//! `majordomus capabilities`: introspection of the registry through the registry's own
//! introspection capabilities. Nothing here lists anything of its own.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{CapabilitiesArgs, CapabilitiesCommand, OutputFormat, SchemaSide};
use crate::error::{Error, Result};

/// Run `majordomus capabilities`.
pub fn run(args: CapabilitiesArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let w = |out: &mut std::io::StdoutLock<'_>, s: String| {
        writeln!(out, "{s}").map_err(Error::Transport)
    };
    match args.command {
        CapabilitiesCommand::List {
            kind,
            exposure,
            format,
        } => {
            let input = json!({ "kind": kind, "exposure": exposure });
            let v = ctx
                .execute(cli_capability(ctx, &["capabilities", "list"])?, input)
                .map_err(map)?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    for c in v["capabilities"].as_array().into_iter().flatten() {
                        let mcp = c["exposure"]["mcp"].as_object().map(|m| {
                            let mut parts = Vec::new();
                            if let Some(t) = m.get("tool").and_then(Value::as_str) {
                                parts.push(format!("tool:{t}"));
                            }
                            if let Some(r) = m.get("resource").and_then(|r| r["uri"].as_str()) {
                                parts.push(format!("resource:{r}"));
                            }
                            parts.join(",")
                        });
                        let http = c["exposure"]["http"].as_object().map(|h| {
                            format!(
                                "{} {}",
                                h["method"].as_str().unwrap_or("?"),
                                h["path"].as_str().unwrap_or("?")
                            )
                        });
                        let cli = c["exposure"]["cli"]["path"].as_array().map(|p| {
                            p.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join(" ")
                        });
                        w(
                            &mut out,
                            format!(
                                "{:<48} {:<9} {:<22} mcp={} http={} cli={}",
                                c["id"].as_str().unwrap_or("?"),
                                c["kind"].as_str().unwrap_or("?"),
                                c["stability"].as_str().unwrap_or("?"),
                                mcp.unwrap_or_else(|| "-".into()),
                                http.unwrap_or_else(|| "-".into()),
                                cli.unwrap_or_else(|| "-".into()),
                            ),
                        )?;
                    }
                    let s = &v["summary"];
                    w(&mut out, format!("capabilities: {} ({} builtin, {} declarative); mcp tools {}, mcp resources {}, http routes {}, cli commands {}", s["total"], s["builtin"], s["declarative"], s["mcp_tools"], s["mcp_resources"], s["http_routes"], s["cli_commands"]))?;
                }
            }
        }
        CapabilitiesCommand::Describe { id, format } => {
            let v = ctx
                .execute(
                    cli_capability(ctx, &["capabilities", "describe"])?,
                    json!({ "id": id }),
                )
                .map_err(|e| match e {
                    CapabilityError::NotFound(_) => Error::CapabilityNotFound { id: id.clone() },
                    other => map(other),
                })?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    w(
                        &mut out,
                        format!("id           {}", v["id"].as_str().unwrap_or("?")),
                    )?;
                    w(
                        &mut out,
                        format!("kind         {}", v["kind"].as_str().unwrap_or("?")),
                    )?;
                    w(
                        &mut out,
                        format!("stability    {}", v["stability"].as_str().unwrap_or("?")),
                    )?;
                    w(
                        &mut out,
                        format!("title        {}", v["title"].as_str().unwrap_or("")),
                    )?;
                    w(
                        &mut out,
                        format!("description  {}", v["description"].as_str().unwrap_or("")),
                    )?;
                    w(
                        &mut out,
                        format!("provenance   {}", provenance_line(&v["provenance"])),
                    )?;
                    w(
                        &mut out,
                        format!(
                            "mcp          {}",
                            v["exposure"]
                                .get("mcp")
                                .map(|m| m.to_string())
                                .unwrap_or_else(|| "none".into())
                        ),
                    )?;
                    w(
                        &mut out,
                        format!(
                            "http         {}",
                            v["exposure"]
                                .get("http")
                                .map(|h| format!(
                                    "{} {}",
                                    h["method"].as_str().unwrap_or("?"),
                                    h["path"].as_str().unwrap_or("?")
                                ))
                                .unwrap_or_else(|| "none".into())
                        ),
                    )?;
                    w(
                        &mut out,
                        format!(
                            "cli          {}",
                            v["exposure"]
                                .get("cli")
                                .and_then(|c| c["path"].as_array())
                                .map(|p| format!(
                                    "majordomus {}",
                                    p.iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                ))
                                .unwrap_or_else(|| "none".into())
                        ),
                    )?;
                    w(
                        &mut out,
                        format!(
                            "input        {}",
                            v["input"]["name"].as_str().unwrap_or("(anonymous)")
                        ),
                    )?;
                    w(
                        &mut out,
                        format!(
                            "output       {}",
                            v["output"]["name"].as_str().unwrap_or("(anonymous)")
                        ),
                    )?;
                }
            }
        }
        CapabilitiesCommand::Schema { id, side } => {
            let c = ctx
                .registry
                .get(&id)
                .ok_or_else(|| Error::CapabilityNotFound { id: id.clone() })?;
            let schema = match side {
                SchemaSide::Input => &c.input.schema,
                SchemaSide::Output => &c.output.schema,
            };
            w(&mut out, pretty(schema))?;
        }
        CapabilitiesCommand::Validate => {
            // App::load already refused to build an invalid registry with every error named.
            let s = ctx.registry.summary();
            let queries = ctx
                .registry
                .iter()
                .filter(|c| c.kind.is_executable())
                .count();
            w(
                &mut out,
                format!(
                    "OK   share       {} ({}) — kinds from {}; {} schema(s)",
                    app.share.dir().display(),
                    app.share.origin,
                    app.schema.sources().join(" + "),
                    app.schema.schemas().count()
                ),
            )?;
            w(&mut out, format!("OK   registry    {} capabilities — every id, MCP name, MCP uri, HTTP route and CLI path unique; {} executable(s) carry a handler", s.total, queries))?;
            w(&mut out, format!("OK   modules     {} module(s) — every executable composed in the module its namespace names; {} required benchmark target(s), {} waived, {} cached", s.modules, s.benchmark_required, s.benchmark_waived, s.cached))?;
            w(&mut out, format!("OK   mcp         {} tool(s), {} resource(s) — every exposure names an executable capability", s.mcp_tools, s.mcp_resources))?;
            w(
                &mut out,
                format!(
                    "OK   http        {} route(s) — every path under {}",
                    s.http_routes,
                    crate::capability::HttpExposure::PREFIX
                ),
            )?;
            let cases = crate::capability::CaseContext { index: &ctx.index };
            let doc = crate::http::openapi::document(&ctx.registry, crate::VERSION, Some(&cases))
                .map_err(|reason| Error::Http { reason })?;
            let ops = doc["paths"]
                .as_object()
                .map(|p| {
                    p.values()
                        .map(|m| m.as_object().map(|o| o.len()).unwrap_or(0))
                        .sum::<usize>()
                })
                .unwrap_or(0);
            w(&mut out, format!("OK   openapi     {} operation(s), {} schema component(s) — generated from the registry without conflict", ops, doc["components"]["schemas"].as_object().map(|s| s.len()).unwrap_or(0)))?;
            let projection = crate::bench::BenchmarkProjection::from_context(ctx);
            let coverage = crate::bench::Coverage::compute(ctx, &projection);
            let total = coverage.tallies.get("total").cloned().unwrap_or_default();
            let mut failures = 0;
            if coverage.is_complete() {
                w(&mut out, format!("OK   benchmarks  {} target(s) cover {} requirement(s) — every executable timed directly and on every transport it is exposed on, plus the transports' own operations", projection.targets.len(), total.required))?;
            } else {
                failures += 1;
                w(&mut out, format!("FAIL benchmarks  {} of {} requirement(s) missing, {} waived — a case must exist for every exposed executable  [reproduce: majordomus bench coverage]", total.missing, total.required, total.waived))?;
                for line in coverage
                    .lines
                    .iter()
                    .filter(|l| l.state != crate::bench::CoverageState::Covered)
                {
                    w(
                        &mut out,
                        format!(
                            "     {:?} {} on {}",
                            line.state,
                            line.subject,
                            line.transport.name()
                        ),
                    )?;
                }
            }
            w(&mut out, format!("validate: {failures} failure(s)"))?;
            if failures > 0 {
                return Ok(10);
            }
        }
    }
    Ok(0)
}

/// The capability the registry binds to a CLI path; the command line dispatches on the
/// registry's CLI exposure rather than on a name of its own.
fn cli_capability<'a>(ctx: &'a crate::capability::Context, path: &[&str]) -> Result<&'a str> {
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    ctx.registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

fn provenance_line(p: &Value) -> String {
    match p["source"].as_str() {
        Some("builtin") => format!("builtin {}", p["module"].as_str().unwrap_or("?")),
        Some("declarative") => format!(
            "{} (class {}, section {})",
            p["path"].as_str().unwrap_or("?"),
            p["source_class"].as_str().unwrap_or("?"),
            p["section"].as_str().unwrap_or("-")
        ),
        _ => p.to_string(),
    }
}
