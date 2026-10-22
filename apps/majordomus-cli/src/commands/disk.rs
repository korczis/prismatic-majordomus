//! `majordomus disk prune`: what this machine can give back, through the registry's
//! `disk.prune` (I3833).
//!
//! The command renders the capability's answer; the candidates, the locks, the verdicts
//! and the removal are the capability's, so the terminal, the `--format json` document,
//! MCP and HTTP cannot disagree about what was removed. A prune is a check unless `--apply`
//! is said.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::capability::builtin::disk::DiskPruneResult;
use crate::cli::{DiskArgs, DiskCommand, OutputFormat};
use crate::disk::containers::ContainerReport;
use crate::disk::Kept;
use crate::error::{Error, Result};

/// Run `majordomus disk`.
pub fn run(args: DiskArgs) -> Result<u8> {
    let DiskCommand::Prune {
        apply,
        idle_minutes,
        extra,
        containers,
    } = args.command;
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let words = ["disk".to_string(), "prune".to_string()];
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus disk prune`".into(),
        })?;
    let input = json!({
        "idle_minutes": idle_minutes,
        "check": !apply,
        "extra": extra,
        "containers": containers,
    });
    let value = ctx.execute(id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    let result: DiskPruneResult =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!("disk.prune answered with something this command cannot read: {e}"),
        })?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => render(&mut out, &result).map_err(Error::Transport)?,
    }
    Ok(0)
}

/// Bytes as GiB with one decimal, the unit a person reads a disk in.
fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / 1_073_741_824.0)
}

/// One line per candidate, then what was reclaimed and the free space before and after.
fn render(out: &mut impl Write, r: &DiskPruneResult) -> std::io::Result<()> {
    let b = &r.build_dirs;
    for c in &b.candidates {
        let what = match (c.removed, c.kept) {
            (true, _) => "removed".to_string(),
            (false, Some(Kept::Check)) => "would remove".to_string(),
            (false, Some(Kept::Recent)) => format!("kept: changed within {} min", b.idle_minutes),
            (false, Some(Kept::Locked)) => "kept: a build holds its lock".to_string(),
            (false, Some(Kept::Unreadable)) => "kept: could not be read".to_string(),
            (false, None) => "not removed".to_string(),
        };
        writeln!(out, "{:<14} {:>9}  {}", what, gib(c.bytes), c.path)?;
        if let Some(e) = &c.error {
            writeln!(out, "               {e}")?;
        }
    }
    if b.candidates.is_empty() {
        writeln!(out, "no build directory to consider")?;
    }
    let would: u64 = b
        .candidates
        .iter()
        .filter(|c| c.kept == Some(Kept::Check))
        .map(|c| c.bytes)
        .sum();
    if b.check {
        writeln!(
            out,
            "check: {} would be reclaimed; nothing was removed (--apply removes)",
            gib(would)
        )?;
    } else {
        writeln!(out, "reclaimed {}", gib(b.reclaimed_bytes))?;
    }
    if let Some(c) = &r.containers {
        render_containers(out, c)?;
    }
    if let (Some(before), Some(after)) = (r.free_before, r.free_after) {
        writeln!(out, "free: {} before, {} after", gib(before), gib(after))?;
    }
    Ok(())
}

/// The container runtime: what docker says it holds, each command run, the disk image.
fn render_containers(out: &mut impl Write, c: &ContainerReport) -> std::io::Result<()> {
    writeln!(
        out,
        "containers: {}{}",
        c.runtime.as_deref().unwrap_or("none"),
        if c.running { "" } else { " (not running)" }
    )?;
    for k in &c.reclaimable {
        writeln!(out, "  reclaimable {:<14} {}", k.kind, k.reclaimable)?;
    }
    for s in &c.steps {
        let mark = if s.ok { "ran" } else { "FAILED" };
        writeln!(out, "  {mark:<6} {}  {}", s.command, s.detail)?;
    }
    if let (Some(img), Some(before)) = (&c.disk_image, c.allocated_before) {
        let after = c.allocated_after.map(gib).unwrap_or_else(|| "?".into());
        writeln!(
            out,
            "  disk image {img}: {} before, {after} after",
            gib(before)
        )?;
    }
    if let Some(n) = &c.note {
        writeln!(out, "  {n}")?;
    }
    Ok(())
}
