//! `majordomus offload`: the command line's rendering of [`crate::offload`].
//!
//! Nothing is decided here. The declaration is read (from `--declaration`, or from the
//! repository's index), the module answers, and this file prints the answer as lines for a
//! person or as one JSON document.

use std::io::Write;

use serde_json::json;

use crate::cli::{OffloadArgs, OffloadCommand, OutputFormat};
use crate::error::{Error, Result};
use crate::offload::declaration::OffloadDeclaration;
use crate::offload::host::{probe, Standing};
use crate::offload::run::{offload, OffloadRequest, EXIT_UNAVAILABLE};

/// Run `majordomus offload`.
pub fn run(args: OffloadArgs) -> Result<u8> {
    let declaration = load(&args)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let w = |e: std::io::Error| Error::io("<stdout>", e);
    match args.command {
        OffloadCommand::Hosts => {
            hosts(declaration.as_ref(), args.format, &mut out).map_err(w)?;
            Ok(0)
        }
        OffloadCommand::Probe => {
            let d = required(declaration)?;
            let reports = probe(&d);
            let chosen = reports.iter().find(|r| r.chosen).map(|r| r.id.clone());
            match args.format {
                OutputFormat::Json => {
                    let doc = json!({ "chosen": chosen, "hosts": reports });
                    writeln!(
                        out,
                        "{}",
                        serde_json::to_string_pretty(&doc).unwrap_or_default()
                    )
                    .map_err(w)?;
                }
                OutputFormat::Text => {
                    for r in &reports {
                        let free = r.free_gib.map_or("?".into(), |f| f.to_string());
                        let load = r.load.map_or("?".into(), |l| l.to_string());
                        let mark = if r.chosen { "*" } else { " " };
                        let why = r
                            .reason
                            .as_deref()
                            .map(|s| format!("  ({s})"))
                            .unwrap_or_default();
                        writeln!(
                            out,
                            "{mark} {:<12} {:<28} free {free:>4} GiB (min {:>3})  load {load:>2}  {}{why}",
                            r.id,
                            r.address,
                            r.min_free_gib,
                            standing(r.standing)
                        )
                        .map_err(w)?;
                    }
                    match &chosen {
                        Some(id) => writeln!(out, "a derive would go to {id}").map_err(w)?,
                        None => writeln!(out, "no declared host can take a derive").map_err(w)?,
                    }
                }
            }
            Ok(if chosen.is_some() {
                0
            } else {
                EXIT_UNAVAILABLE
            })
        }
        OffloadCommand::Derive {
            worktree,
            host,
            keep,
        } => {
            let d = required(declaration)?;
            let request = OffloadRequest {
                worktree,
                host,
                keep,
            };
            let stderr = std::io::stderr();
            let report = offload(&d, &request, &mut stderr.lock()).map_err(|r| Error::Refused {
                code: r.code,
                reason: format!("offload derive: {}", r.reason),
            })?;
            match args.format {
                OutputFormat::Json => {
                    writeln!(
                        out,
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    )
                    .map_err(w)?;
                }
                OutputFormat::Text => {
                    writeln!(
                        out,
                        "derived {} on {} in {} s ({} s in the queue): {} file(s) staged, nothing committed",
                        report.tag,
                        report.host,
                        report.seconds,
                        report.waited_seconds,
                        report.staged.len()
                    )
                    .map_err(w)?;
                    for f in &report.staged {
                        writeln!(out, "  {f}").map_err(w)?;
                    }
                }
            }
            Ok(0)
        }
    }
}

fn standing(s: Standing) -> &'static str {
    match s {
        Standing::Eligible => "eligible",
        Standing::Unreachable => "unreachable",
        Standing::UnderMinimum => "under its free-space minimum",
        Standing::UnknownSpace => "free space unknown",
    }
}

fn load(args: &OffloadArgs) -> Result<Option<OffloadDeclaration>> {
    let refused = |reason: String| Error::Refused {
        code: EXIT_UNAVAILABLE,
        reason: format!("offload: {reason}"),
    };
    if let Some(path) = &args.declaration {
        return OffloadDeclaration::read(path).map(Some).map_err(refused);
    }
    let app = crate::app::App::load(&args.repo)?;
    crate::offload::from_objects(&app.index().objects).map_err(refused)
}

fn required(d: Option<OffloadDeclaration>) -> Result<OffloadDeclaration> {
    d.ok_or_else(|| Error::Refused {
        code: EXIT_UNAVAILABLE,
        reason: "offload: this repository declares no derive-offload host (.ai/repo/offload/*.yaml, kind derive-offload); see docs/OFFLOAD.md".into(),
    })
}

fn hosts(
    d: Option<&OffloadDeclaration>,
    format: OutputFormat,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    if format == OutputFormat::Json {
        let doc = json!({ "declared": d.is_some(), "declaration": d });
        return writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&doc).unwrap_or_default()
        );
    }
    let Some(d) = d else {
        return writeln!(out, "no derive-offload declaration in this repository: nothing to offload to (.ai/repo/offload/*.yaml)");
    };
    writeln!(
        out,
        "declaration {} — base {}, minimum {} GiB free",
        d.id, d.base, d.min_free_gib
    )?;
    writeln!(
        out,
        "queue {} (lock {}), a look every {} s, at most {} s",
        d.queue.directory, d.queue.lock, d.queue.poll_seconds, d.queue.wait_max_seconds
    )?;
    writeln!(
        out,
        "on a host: checkout ~/{}, worktrees ~/{}, shared build ~/{}, runs {}",
        d.remote.checkout, d.remote.worktrees, d.remote.target, d.remote.derive
    )?;
    for h in &d.hosts {
        let id = h
            .identity_file
            .as_deref()
            .map(|f| format!("  -i {f}"))
            .unwrap_or_default();
        writeln!(
            out,
            "  {:<12} {:<28} min {:>3} GiB{id}",
            h.id,
            h.address,
            h.min_free_gib(d)
        )?;
    }
    Ok(())
}
