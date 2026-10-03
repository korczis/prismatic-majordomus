//! `majordomus pack`: the tracked tree as token-bounded text shards, through the registry's
//! `pack.plan` and `pack.verify`.
//!
//! `plan` and `verify` render the capability's answer and take the exit code from it, so the
//! terminal, `--format json`, MCP and HTTP cannot disagree about a verdict. `build` is the
//! one thing this command does that no projection offers, because it writes: it plans with
//! the same function the capability calls, writes the pack only when the plan passes, and
//! then verifies what it wrote, so a pack that leaves this command has been read back.

use std::io::Write;
use std::path::PathBuf;

use serde_json::json;

use crate::app::App;
use crate::cli::{OutputFormat, PackArgs, PackCommand};
use crate::error::{Error, Result};
use crate::pack::{PackFinding, PackPlan, PackVerdict, Profiles};

/// The exit code when a finding stands.
pub const EXIT_FINDINGS: u8 = 10;
/// The exit code when the tree or the pack could not be read: not a pass.
pub const EXIT_UNMEASURED: u8 = 12;

/// Run `majordomus pack`.
pub fn run(args: PackArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let root = PathBuf::from(&ctx.index.repository.root);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command {
        PackCommand::Plan { profile } => {
            let value = execute(ctx, "plan", json!({ "profile": profile }))?;
            let plan: PackPlan = decode(&value)?;
            match args.format {
                OutputFormat::Json => print_json(&mut out, &value)?,
                OutputFormat::Text => render_plan(&mut out, &plan)?,
            }
            Ok(plan_code(&plan))
        }
        PackCommand::Verify { dir } => {
            let value = execute(ctx, "verify", json!({ "dir": dir }))?;
            let verdict: PackVerdict = decode(&value)?;
            match args.format {
                OutputFormat::Json => print_json(&mut out, &value)?,
                OutputFormat::Text => render_verdict(&mut out, &verdict)?,
            }
            Ok(verdict_code(&verdict))
        }
        PackCommand::Build {
            profile,
            out: dest,
            force,
        } => {
            let share = crate::share::Share::locate(ctx.index.share.as_deref(), &root)?
                .dir()
                .to_path_buf();
            let planned = crate::pack::plan(&root, &share, profile.as_deref());
            if !planned.plan.passes {
                match args.format {
                    OutputFormat::Json => print_json(&mut out, &json!({ "plan": planned.plan }))?,
                    OutputFormat::Text => render_plan(&mut out, &planned.plan)?,
                }
                return Ok(plan_code(&planned.plan));
            }
            let profiles =
                Profiles::load(&share, &root).map_err(|reason| Error::Protocol { reason })?;
            let dest = dest
                .map(|d| if d.is_absolute() { d } else { root.join(d) })
                .unwrap_or_else(|| crate::pack::default_out(&root, &planned.plan));
            if dest.join(crate::pack::MANIFEST).is_file() && !force {
                return Err(Error::Protocol {
                    reason: format!(
                        "{} already holds a pack; pass --force to replace it",
                        dest.display()
                    ),
                });
            }
            let verdict = crate::pack::build(&planned, &profiles, &root, &dest)
                .map_err(|reason| Error::Protocol { reason })?;
            match args.format {
                OutputFormat::Json => print_json(
                    &mut out,
                    &json!({ "plan": planned.plan, "out": dest.display().to_string(), "verdict": verdict }),
                )?,
                OutputFormat::Text => {
                    render_plan(&mut out, &planned.plan)?;
                    render_verdict(&mut out, &verdict)?;
                }
            }
            Ok(verdict_code(&verdict))
        }
    }
}

fn execute(
    ctx: &crate::capability::handler::Context,
    word: &str,
    input: serde_json::Value,
) -> Result<serde_json::Value> {
    let words = ["pack".to_string(), word.to_string()];
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!("no capability is exposed as `majordomus pack {word}`"),
        })?;
    ctx.execute(id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })
}

fn decode<T: serde::de::DeserializeOwned>(value: &serde_json::Value) -> Result<T> {
    serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
        reason: format!(
            "the pack capability answered with something this command cannot read: {e}"
        ),
    })
}

fn print_json<W: Write>(out: &mut W, value: &serde_json::Value) -> Result<()> {
    writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    )
    .map_err(Error::Transport)
}

fn plan_code(plan: &PackPlan) -> u8 {
    if !plan.measured {
        EXIT_UNMEASURED
    } else if plan.passes {
        0
    } else {
        EXIT_FINDINGS
    }
}

fn verdict_code(v: &PackVerdict) -> u8 {
    if !v.measured {
        EXIT_UNMEASURED
    } else if v.passes {
        0
    } else {
        EXIT_FINDINGS
    }
}

fn render_findings<W: Write>(out: &mut W, findings: &[PackFinding]) -> Result<()> {
    for f in findings {
        writeln!(
            out,
            "FAIL  {}  {}  {}\n      remedy: {}",
            f.code,
            f.path.as_deref().unwrap_or("-"),
            f.message,
            f.remedy
        )
        .map_err(Error::Transport)?;
    }
    Ok(())
}

/// The terminal rendering of a plan: the findings, what was left out, the shards, the verdict.
fn render_plan<W: Write>(out: &mut W, plan: &PackPlan) -> Result<()> {
    let w = |out: &mut W, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    if !plan.measured {
        return w(
            out,
            format!(
                "pack plan: not measured: {}",
                plan.reason.as_deref().unwrap_or("no reason given")
            ),
        );
    }
    render_findings(out, &plan.findings)?;
    w(
        out,
        format!(
            "pack: profile {} at {} — {} of {} tracked file(s), {} bytes, {} tokens ({})",
            plan.profile,
            plan.commit
                .as_deref()
                .map(|c| &c[..c.len().min(12)])
                .unwrap_or("no commit"),
            plan.selected,
            plan.tracked,
            plan.bytes,
            plan.tokens,
            plan.tokenizer
        ),
    )?;
    if !plan.index_matches_head {
        w(
            out,
            "      the index holds staged changes HEAD does not; commit them first".into(),
        )?;
    }
    for (reason, n) in &plan.dropped {
        w(out, format!("      left out: {n} ({reason})"))?;
    }
    for d in &plan.dropped_files {
        w(out, format!("        {} ({})", d.path, d.reason.as_str()))?;
    }
    if let Some(l) = plan.limits {
        w(
            out,
            format!(
                "      shards: {} + index, of at most {} file(s) of {} tokens",
                plan.shards.len(),
                l.max_count,
                l.max_tokens
            ),
        )?;
    }
    for s in &plan.shards {
        w(
            out,
            format!(
                "        {}  {} file(s), {} tokens",
                s.file, s.files, s.tokens
            ),
        )?;
    }
    if plan.passes {
        w(out, "pack plan: clean".into())
    } else {
        w(
            out,
            format!("pack plan: {} finding(s)", plan.findings.len()),
        )
    }
}

/// The terminal rendering of a verdict on a written pack.
fn render_verdict<W: Write>(out: &mut W, v: &PackVerdict) -> Result<()> {
    let w = |out: &mut W, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    if !v.measured {
        return w(
            out,
            format!(
                "pack verify: not measured: {}",
                v.reason.as_deref().unwrap_or("no reason given")
            ),
        );
    }
    render_findings(out, &v.findings)?;
    w(
        out,
        format!(
            "pack {}: {} file(s) to upload, {} source file(s), the largest {} tokens",
            v.dir, v.files, v.sources, v.largest_tokens
        ),
    )?;
    if v.passes {
        w(out, "pack verify: clean".into())
    } else {
        w(out, format!("pack verify: {} finding(s)", v.findings.len()))
    }
}
