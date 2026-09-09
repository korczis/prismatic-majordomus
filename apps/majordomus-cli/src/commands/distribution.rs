//! `majordomus distribution`: the distribution model through the registry's own
//! capabilities. Every subcommand here executes a capability and renders what it answered;
//! the facts are the model's, the phrasing is this file's, and nothing between them
//! restates a platform, a name or a URL.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{DistributionArgs, DistributionCommand, OutputFormat};
use crate::distribution::{render, Model, Releases};
use crate::error::{Error, Result};

/// The exit code when the model or a release record breaks an invariant.
pub const EXIT_INVALID: u8 = 10;

/// Run `majordomus distribution`.
pub fn run(args: DistributionArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command {
        None | Some(DistributionCommand::Show) => show(&app, args.format, &mut out),
        Some(DistributionCommand::Status) => status(&app, args.format, &mut out),
        Some(DistributionCommand::Targets) => targets(&app, args.format, &mut out),
        Some(DistributionCommand::Validate) => validate(&app, args.format, &mut out),
        Some(DistributionCommand::Matrix) => {
            let model = Model::load(&app.share)?;
            w(&mut out, render::matrix_json(&model).trim_end().to_string())?;
            Ok(0)
        }
        Some(DistributionCommand::Artifact { target, tag }) => {
            artifact(&app, &target, &tag, args.format, &mut out)
        }
        Some(DistributionCommand::Releases) => releases(&app, args.format, &mut out),
        Some(DistributionCommand::Metadata { record }) => metadata(&app, &record, &mut out),
        Some(DistributionCommand::Build) => build(&app, args.format, &mut out),
    }
}

type Out<'a> = std::io::StdoutLock<'a>;

fn w(out: &mut Out<'_>, s: String) -> Result<()> {
    writeln!(out, "{s}").map_err(Error::Transport)
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}

/// Execute the capability exposed at a command path: the command line is a projection of
/// the registry, not a second implementation beside it.
fn call(app: &App, path: &[&str], input: Value) -> Result<Value> {
    let ctx = &app.context;
    let words: Vec<String> = path.iter().map(|w| (*w).to_string()).collect();
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

/// `majordomus distribution status`. Exits 10 when the published installation does not
/// work, so that a script and a person get the same answer without reading prose.
fn status(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["distribution", "status"], json!({}))?;
    let installable = v["installable"].as_bool().unwrap_or(false);
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            let s = |k: &str| v[k].as_str().unwrap_or("?").to_string();
            for c in v["checks"].as_array().into_iter().flatten() {
                let state = match c["state"].as_str().unwrap_or("") {
                    "ok" => "OK  ",
                    "failed" => "FAIL",
                    _ => "?   ",
                };
                w(
                    out,
                    format!(
                        "{state} {:<10} {}",
                        c["id"].as_str().unwrap_or(""),
                        c["observed"].as_str().unwrap_or("")
                    ),
                )?;
            }
            w(out, String::new())?;
            w(out, format!("install      {}", s("install_command")))?;
            w(
                out,
                format!(
                    "stable       {}",
                    v["stable_tag"].as_str().unwrap_or("none")
                ),
            )?;
            w(
                out,
                format!("installable  {}", if installable { "yes" } else { "NO" }),
            )?;
            // A failure states its cause and the command that changes it, in that order,
            // because an operator reading this is deciding what to do next.
            if !installable {
                for c in v["checks"].as_array().into_iter().flatten() {
                    if c["state"].as_str() != Some("failed") {
                        continue;
                    }
                    w(out, String::new())?;
                    w(out, format!("CAUSE  {}", c["cause"].as_str().unwrap_or("")))?;
                    if let Some(next) = c["next"].as_str() {
                        w(out, format!("NEXT   {next}"))?;
                    }
                }
            }
        }
    }
    Ok(if installable { 0 } else { EXIT_INVALID })
}

fn show(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["distribution", "show"], json!({}))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            let s = |k: &str| v[k].as_str().unwrap_or("?").to_string();
            w(out, format!("binary       {}", s("binary")))?;
            w(out, format!("repository   {}", s("repository")))?;
            w(out, format!("installer    {}", s("installer_url")))?;
            w(out, format!("install      {}", s("install_command")))?;
            w(out, format!("then         {}", s("next_command")))?;
            w(out, format!("launchers    {}", s("install_dir")))?;
            w(out, format!("prefix       {}", s("prefix")))?;
            w(out, format!("checksum     {}", s("checksum")))?;
            w(out, format!("latest       {}", s("latest_url")))?;
            w(
                out,
                format!(
                    "targets      {} published of {} declared",
                    v["supported"],
                    v["targets"].as_array().map_or(0, Vec::len)
                ),
            )?;
        }
    }
    Ok(0)
}

fn targets(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["distribution", "show"], json!({}))?;
    let rows = v["targets"].as_array().cloned().unwrap_or_default();
    match format {
        OutputFormat::Json => w(out, pretty(&json!({ "targets": rows })))?,
        OutputFormat::Text => {
            w(
                out,
                format!(
                    "{:<20} {:<32} {:<13} {}",
                    "ID", "RUST TARGET", "STATUS", "TITLE"
                ),
            )?;
            for t in &rows {
                w(
                    out,
                    format!(
                        "{:<20} {:<32} {:<13} {}",
                        t["id"].as_str().unwrap_or("?"),
                        t["rust_target"].as_str().unwrap_or("?"),
                        t["status"].as_str().unwrap_or("?"),
                        t["title"].as_str().unwrap_or("?")
                    ),
                )?;
            }
        }
    }
    Ok(0)
}

fn validate(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let model = Model::load(&app.share)?;
    let records = Releases::load(app.repository.root())?;
    let mut findings = model.findings();
    findings.extend(records.findings(&model));
    match format {
        OutputFormat::Json => w(
            out,
            pretty(&json!({
                "valid": findings.is_empty(),
                "targets": model.targets.len(),
                "published": model.published().count(),
                "releases": records.releases.len(),
                "findings": findings,
            })),
        )?,
        OutputFormat::Text => {
            for f in &findings {
                w(out, format!("FAIL distribution  {f}"))?;
            }
            if findings.is_empty() {
                w(
                    out,
                    format!(
                        "OK   distribution  {} target(s), {} published, {} release(s) recorded",
                        model.targets.len(),
                        model.published().count(),
                        records.releases.len()
                    ),
                )?;
            }
        }
    }
    Ok(if findings.is_empty() { 0 } else { EXIT_INVALID })
}

fn artifact(
    app: &App,
    target: &str,
    tag: &str,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let v = call(
        app,
        &["distribution", "artifact"],
        json!({ "target": target, "tag": tag }),
    )?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        // two lines, name then root: what `scripts/release-package` reads
        OutputFormat::Text => {
            w(out, v["name"].as_str().unwrap_or_default().to_string())?;
            w(out, v["root"].as_str().unwrap_or_default().to_string())?;
        }
    }
    Ok(0)
}

fn releases(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["distribution", "releases"], json!({}))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            let latest = v["latest"]["tag"].as_str().unwrap_or("");
            let rows = v["releases"].as_array().cloned().unwrap_or_default();
            if rows.is_empty() {
                w(out, "no release is recorded yet".to_string())?;
                return Ok(0);
            }
            w(
                out,
                format!(
                    "{:<14} {:<22} {:<12} {}",
                    "TAG", "PUBLISHED", "CHANNEL", "ARTIFACTS"
                ),
            )?;
            for r in &rows {
                let tag = r["tag"].as_str().unwrap_or("?");
                w(
                    out,
                    format!(
                        "{:<14} {:<22} {:<12} {}{}",
                        tag,
                        r["published_at"].as_str().unwrap_or("?"),
                        r["channel"].as_str().unwrap_or("?"),
                        r["artifacts"],
                        if tag == latest { "   (latest)" } else { "" }
                    ),
                )?;
            }
        }
    }
    Ok(0)
}

/// The public metadata one record publishes, rendered from the record and the model alone.
/// The release pipeline uses this to see what it is about to commit; the test suite uses it
/// to serve a release that never existed. Neither renders the document itself.
fn metadata(app: &App, record: &std::path::Path, out: &mut Out<'_>) -> Result<u8> {
    let model = Model::load(&app.share)?;
    if !record.is_file() {
        return Err(Error::InvalidRelease {
            path: record.display().to_string(),
            reason: "no such release record; a record is written by the release pipeline under .ai/repo/releases/ and named by its tag".into(),
        });
    }
    let text = std::fs::read_to_string(record).map_err(|e| Error::io(record, e))?;
    let release =
        crate::distribution::Release::parse(&text).map_err(|reason| Error::InvalidRelease {
            path: record.display().to_string(),
            reason,
        })?;
    let findings = release.findings(&model);
    if let Some(first) = findings.first() {
        return Err(Error::InvalidRelease {
            path: record.display().to_string(),
            reason: first.clone(),
        });
    }
    w(out, release.public_json(&model).trim_end().to_string())?;
    Ok(0)
}

fn build(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["distribution", "build"], json!({}))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            let s = |k: &str| v[k].as_str().unwrap_or("?").to_string();
            w(out, format!("version      {}", s("version")))?;
            w(out, format!("target       {}", s("target")))?;
            w(out, format!("profile      {}", s("profile")))?;
            w(out, format!("commit       {}", s("commit")))?;
            w(
                out,
                format!(
                    "distributed  {}",
                    v["distribution_target"]
                        .as_str()
                        .unwrap_or("no target of the model matches this triple")
                ),
            )?;
        }
    }
    Ok(0)
}
