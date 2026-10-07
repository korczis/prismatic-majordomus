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

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::app::App;
use crate::capability::handler::Context;
use crate::cli::{OutputFormat, PackArgs, PackCommand};
use crate::error::{Error, Result};
use crate::pack::{PackFinding, PackPlan, PackVerdict};

/// The exit code when a finding stands.
pub const EXIT_FINDINGS: u8 = 10;
/// The exit code when the tree or the pack could not be read: not a pass.
pub const EXIT_UNMEASURED: u8 = 12;

/// Run `majordomus pack`.
pub fn run(args: PackArgs) -> Result<u8> {
    let stdout = std::io::stdout();
    run_to(args, &mut stdout.lock())
}

/// Run `majordomus pack`, writing what it prints to `out`.
fn run_to<W: Write>(args: PackArgs, out: &mut W) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let format = args.format;
    match args.command {
        PackCommand::Plan { profile } => answer(
            ctx,
            "plan",
            json!({ "profile": profile }),
            format,
            out,
            render_plan,
            plan_code,
        ),
        PackCommand::Verify { dir } => answer(
            ctx,
            "verify",
            json!({ "dir": dir }),
            format,
            out,
            render_verdict,
            verdict_code,
        ),
        PackCommand::Build {
            profile,
            out: dest,
            force,
        } => build(ctx, profile.as_deref(), dest, force, format, out),
    }
}

/// Ask the registry's `pack.<word>` capability, read its answer as `T`, print it in the
/// format asked for, and exit by it. A capability that refuses the input, or answers with
/// something that is not a `T`, is the one error.
fn answer<T: DeserializeOwned, W: Write>(
    ctx: &Context,
    word: &str,
    input: Value,
    format: OutputFormat,
    out: &mut W,
    render: fn(&T) -> String,
    code: fn(&T) -> u8,
) -> Result<u8> {
    type Failure = Box<dyn std::error::Error>;
    let words = ["pack".to_string(), word.to_string()];
    // a word no capability is exposed under asks the registry for no capability, which it
    // refuses like any unknown id
    let id = ctx.registry.by_cli(&words).map_or("", |c| c.id.as_str());
    ctx.execute(id, input)
        .map_err(Failure::from)
        .and_then(|value| {
            serde_json::from_value::<T>(value.clone())
                .map(|answer| (value, answer))
                .map_err(Failure::from)
        })
        .map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })
        .and_then(|(value, answer)| {
            emit(out, format, &value, &render(&answer)).map(|()| code(&answer))
        })
}

/// `majordomus pack build`: plan with the same function the capability calls, write the pack
/// only when the plan passes, and print the plan with the verdict on what was written.
fn build<W: Write>(
    ctx: &Context,
    profile: Option<&str>,
    dest: Option<PathBuf>,
    force: bool,
    format: OutputFormat,
    out: &mut W,
) -> Result<u8> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let planned = crate::capability::builtin::pack::planned(ctx, profile);
    let plan = &planned.plan;
    if !plan.passes {
        return emit(out, format, &json!({ "plan": plan }), &render_plan(plan))
            .map(|()| plan_code(plan));
    }
    // a relative destination is the repository's; joining an absolute one keeps it whole
    let dest = dest.map_or_else(|| crate::pack::default_out(&root, plan), |d| root.join(d));
    if dest.join(crate::pack::MANIFEST).is_file() && !force {
        return Err(Error::Protocol {
            reason: format!(
                "{} already holds a pack; pass --force to replace it",
                dest.display()
            ),
        });
    }
    crate::pack::build(&planned, &root, &dest)
        .map_err(|reason| Error::Protocol { reason })
        .and_then(|verdict| {
            let value =
                json!({ "plan": plan, "out": dest.display().to_string(), "verdict": verdict });
            let text = render_plan(plan) + &render_verdict(&verdict);
            emit(out, format, &value, &text).map(|()| verdict_code(&verdict))
        })
}

/// Print an answer: the value, pretty, for `--format json`, and the rendering otherwise.
fn emit<W: Write>(out: &mut W, format: OutputFormat, value: &Value, text: &str) -> Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{value:#}"),
        OutputFormat::Text => out.write_all(text.as_bytes()),
    }
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

fn render_findings(findings: &[PackFinding]) -> String {
    findings
        .iter()
        .map(|f| {
            format!(
                "FAIL  {}  {}  {}\n      remedy: {}\n",
                f.code,
                f.path.as_deref().unwrap_or("-"),
                f.message,
                f.remedy
            )
        })
        .collect()
}

/// The terminal rendering of a plan: the findings, what was left out, the shards, the verdict.
fn render_plan(plan: &PackPlan) -> String {
    if !plan.measured {
        let why = plan.reason.as_deref().unwrap_or("no reason given");
        return format!("pack plan: not measured: {why}\n");
    }
    let mut s = render_findings(&plan.findings);
    let commit = plan.commit.as_deref().unwrap_or("no commit");
    s += &format!(
        "pack: profile {} at {} — {} of {} tracked file(s), {} bytes, {} tokens ({})\n",
        plan.profile,
        &commit[..commit.len().min(12)],
        plan.selected,
        plan.tracked,
        plan.bytes,
        plan.tokens,
        plan.tokenizer
    );
    if !plan.index_matches_head {
        s += "      the index holds staged changes HEAD does not; commit them first\n";
    }
    for (reason, n) in &plan.dropped {
        s += &format!("      left out: {n} ({reason})\n");
    }
    for d in &plan.dropped_files {
        s += &format!("        {} ({})\n", d.path, d.reason.as_str());
    }
    if let Some(l) = plan.limits {
        s += &format!(
            "      shards: {} + index, of at most {} file(s) of {} tokens\n",
            plan.shards.len(),
            l.max_count,
            l.max_tokens
        );
    }
    for sh in &plan.shards {
        s += &format!(
            "        {}  {} file(s), {} tokens\n",
            sh.file, sh.files, sh.tokens
        );
    }
    s + &if plan.passes {
        "pack plan: clean\n".to_string()
    } else {
        format!("pack plan: {} finding(s)\n", plan.findings.len())
    }
}

/// The terminal rendering of a verdict on a written pack.
fn render_verdict(v: &PackVerdict) -> String {
    if !v.measured {
        let why = v.reason.as_deref().unwrap_or("no reason given");
        return format!("pack verify: not measured: {why}\n");
    }
    let mut s = render_findings(&v.findings);
    s += &format!(
        "pack {}: {} file(s) to upload, {} source file(s), the largest {} tokens\n",
        v.dir, v.files, v.sources, v.largest_tokens
    );
    s + &if v.passes {
        "pack verify: clean\n".to_string()
    } else {
        format!("pack verify: {} finding(s)\n", v.findings.len())
    }
}
