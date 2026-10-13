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
        CapabilitiesCommand::Invoke {
            id,
            input_json,
            input_file,
        } => {
            let input = invoke_input(input_json.as_deref(), input_file.as_deref())?;
            // the capability's own answer, and its own refusal: this command adds neither
            let v = ctx.execute(&id, input).map_err(|e| match e {
                CapabilityError::NotFound(reason) if ctx.registry.get(&id).is_none() => {
                    let _ = reason;
                    Error::CapabilityNotFound { id: id.clone() }
                }
                other => map(other),
            })?;
            w(&mut out, pretty(&v))?;
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
        CapabilitiesCommand::Projections {
            module,
            unmet,
            format,
        } => {
            let v = ctx
                .execute(
                    cli_capability(ctx, &["capabilities", "projections"])?,
                    json!({ "module": module, "unmet": unmet }),
                )
                .map_err(map)?;
            match format {
                OutputFormat::Json => w(&mut out, pretty(&v))?,
                OutputFormat::Text => {
                    let rows = v["rows"].as_array().cloned().unwrap_or_default();
                    for r in &rows {
                        let cell = |k: &str| r[k].as_str().unwrap_or("-").to_string();
                        w(
                            &mut out,
                            format!(
                                "{:<4} {:<34} {:<38} {:<34} {}",
                                if r["closed"].as_bool() == Some(true) {
                                    "ok"
                                } else {
                                    "FAIL"
                                },
                                r["id"].as_str().unwrap_or("?"),
                                cell("cli"),
                                cell("http"),
                                cell("mcp_tool"),
                            ),
                        )?;
                    }
                    let unbacked = v["unbacked"].as_array().cloned().unwrap_or_default();
                    w(&mut out, format!("rows: {}", rows.len()))?;
                    if !unbacked.is_empty() {
                        w(
                            &mut out,
                            format!(
                                "commands no capability claims: {} ({})",
                                unbacked.len(),
                                unbacked
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        )?;
                    }
                }
            }
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
            let mut failures = 0;
            // The native command line's own documentation contract: every command says what
            // it does, every argument says what it is for, and every command a person can
            // run carries at least one example that the example tests execute. Pure: the
            // clap declaration and the examples beside it, nothing read from disk.
            let cli = crate::cli::tree();
            let commands = cli.flatten();
            let violations = crate::cli::validate(&cli);
            if violations.is_empty() {
                w(&mut out, format!("OK   cli         {} command(s), {} of them runnable, {} example(s) — every command documented, every example parsed by the declaration it documents", commands.len(), commands.iter().filter(|c| c.executable).count(), commands.iter().map(|c| c.examples.len()).sum::<usize>()))?;
            } else {
                failures += violations.len();
                w(&mut out, format!("FAIL cli         {} violation(s) of the command-line documentation contract  [reproduce: cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test cli_docs]", violations.len()))?;
                for v in &violations {
                    for line in v.to_string().lines() {
                        w(&mut out, format!("     {line}"))?;
                    }
                }
            }
            // The command line is the one projection that is declared twice — once in
            // clap and once as the `cli` exposure of a descriptor — so it is the one that
            // can drift. Every other projection is built by walking the registry and
            // cannot carry an entry the registry does not have.
            let unmet = crate::capability::closure::findings(&ctx.registry, &cli);
            let debt = crate::capability::closure::unbacked(&ctx.registry, &cli);
            let declared = ctx
                .registry
                .iter()
                .filter(|c| c.exposure.cli.is_some())
                .count();
            if unmet.is_empty() {
                w(&mut out, format!("OK   projection  {declared} declared CLI exposure(s) answered by a runnable command; {} command(s) claimed by no capability", debt.len()))?;
            } else {
                failures += unmet.len();
                w(&mut out, format!("FAIL projection  {} capability(ies) claim a command line the clap declaration does not answer  [reproduce: majordomus capabilities projections --unmet]", unmet.len()))?;
                for f in &unmet {
                    for line in f.to_string().lines() {
                        w(&mut out, format!("     {line}"))?;
                    }
                }
            }

            let projection = crate::bench::BenchmarkProjection::from_context(ctx);
            let coverage = crate::bench::Coverage::compute(ctx, &projection);
            let total = coverage.tallies.get("total").cloned().unwrap_or_default();
            // a waiver is typed, reported and never counted — the rule
            // `project.rust-benchmark-coverage` says so, and what decides the verdict is
            // therefore whether anything is *missing*, not whether anything is waived
            if coverage.has_no_missing() {
                w(&mut out, format!("OK   benchmarks  {} target(s) cover {} requirement(s), {} waived, {} inapplicable — every executable timed directly and on every transport it is exposed on where its declared precondition holds, plus the transports' own operations", projection.targets.len(), total.required, total.waived, total.inapplicable))?;
                for line in coverage.lines.iter().filter(|l| {
                    matches!(
                        l.state,
                        crate::bench::CoverageState::Waived
                            | crate::bench::CoverageState::Inapplicable
                    )
                }) {
                    // an inapplicable line is shown, never folded into the count it did
                    // not earn: the repository lacks what its precondition names
                    w(
                        &mut out,
                        format!(
                            "     {:<8} {} on {} — {}",
                            if line.state == crate::bench::CoverageState::Waived {
                                "waived"
                            } else {
                                "inapplicable"
                            },
                            line.subject,
                            line.transport.name(),
                            line.reason.as_deref().unwrap_or("no reason")
                        ),
                    )?;
                }
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

/// The input of `capabilities invoke`: the JSON given on the command line, the file named
/// (`-` for standard input), or an empty object. It must be one JSON object, because that
/// is what every capability's input schema describes.
fn invoke_input(json: Option<&str>, file: Option<&std::path::Path>) -> Result<Value> {
    let text = match (json, file) {
        (Some(text), _) => text.to_string(),
        (None, Some(path)) if path == std::path::Path::new("-") => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                .map_err(Error::Transport)?;
            text
        }
        (None, Some(path)) => std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?,
        (None, None) => return Ok(json!({})),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(v) if v.is_object() => Ok(v),
        Ok(_) => Err(Error::Protocol {
            reason: "the input is not a JSON object; a capability takes its input as one object"
                .into(),
        }),
        Err(e) => Err(Error::Protocol {
            reason: format!("the input is not JSON: {e}"),
        }),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    fn run_in(repo: &SyntheticRepository, extra: &[&str]) -> Result<u8> {
        let root = repo.root().to_string_lossy().into_owned();
        let share = crate::synthetic::crate_share()
            .to_string_lossy()
            .into_owned();
        let mut argv = vec![
            "majordomus",
            "capabilities",
            "invoke",
            "--repo",
            &root,
            "--discovery",
            "filesystem",
            "--share",
            &share,
        ];
        argv.extend_from_slice(extra);
        crate::commands::run(<crate::cli::Cli as clap::Parser>::parse_from(argv))
    }

    /// The input is one JSON object from the command line or a file, and `{}` when neither
    /// is given; anything else is refused before a capability is asked.
    #[test]
    fn the_input_is_one_json_object_or_nothing() {
        assert_eq!(invoke_input(None, None).unwrap(), json!({}));
        assert_eq!(
            invoke_input(Some(r#"{"id": "x"}"#), None).unwrap(),
            json!({"id": "x"})
        );
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input.json");
        std::fs::write(&file, r#"{"statement": "s"}"#).unwrap();
        assert_eq!(
            invoke_input(None, Some(&file)).unwrap(),
            json!({"statement": "s"})
        );
        for bad in ["[1]", "\"text\"", "{"] {
            match invoke_input(Some(bad), None) {
                Err(Error::Protocol { reason }) => {
                    assert!(reason.starts_with("the input is not"), "{reason}")
                }
                other => panic!("{bad} answered {other:?}"),
            }
        }
        match invoke_input(None, Some(&dir.path().join("absent.json"))) {
            Err(e) => assert!(e.to_string().contains("absent.json"), "{e}"),
            Ok(v) => panic!("an absent file answered {v}"),
        }
    }

    /// A read and a write through the one generic command: the read answers, the write
    /// writes its record once and is the capability's own refusal the second time, and an id
    /// the registry does not hold is said as that.
    #[test]
    fn invoke_runs_the_named_capability_and_adds_nothing_of_its_own() {
        let repo = SyntheticRepository::small().unwrap();
        assert_eq!(run_in(&repo, &["repository.info"]).unwrap(), 0);
        let propose = [
            "intents.propose",
            "--input-json",
            r#"{"statement": "Invoked by its id", "id": "invoked"}"#,
        ];
        assert_eq!(run_in(&repo, &propose).unwrap(), 0);
        let record = repo.root().join(".ai/repo/project/intents/invoked.yaml");
        let text = std::fs::read_to_string(&record).unwrap();
        assert!(text.contains("statement: \"Invoked by its id\""), "{text}");
        match run_in(&repo, &propose) {
            Err(Error::Protocol { reason }) => {
                assert!(reason.contains("never overwrites"), "{reason}")
            }
            other => panic!("a second proposal answered {other:?}"),
        }
        assert_eq!(std::fs::read_to_string(&record).unwrap(), text);
        match run_in(&repo, &["no.such"]) {
            Err(e @ Error::CapabilityNotFound { .. }) => assert_eq!(e.exit_code(), 12),
            other => panic!("an unknown capability answered {other:?}"),
        }
        // a field the capability's input does not have is the capability's refusal
        match run_in(
            &repo,
            &["intents.propose", "--input-json", r#"{"stage": "x"}"#],
        ) {
            Err(Error::Protocol { reason }) => {
                assert!(reason.contains("invalid input"), "{reason}")
            }
            other => panic!("an unknown field answered {other:?}"),
        }
    }
}
