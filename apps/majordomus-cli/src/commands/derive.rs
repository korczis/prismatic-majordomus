//! `majordomus derive`: the command line's rendering of [`crate::derive`].
//!
//! Nothing is decided here. The work tree is found, the surroundings are read once, the
//! module answers, and this file prints: the plan on stdout, the lines a step hands
//! `scripts/derive` on stderr, and the step's exit code as the exit code.

use std::io::Write;
use std::path::PathBuf;

use crate::cli::{DeriveArgs, DeriveCommand, DeriveMode, OutputFormat};
use crate::derive::key::Ambient;
use crate::derive::{begin, forget, plan, record, Mode, Step};
use crate::error::{Error, Result};
use crate::git::read_only;

/// Exit code of a step that could not measure: the repository's could-not-reach code.
const EXIT_UNMEASURED: u8 = 12;

/// Run `majordomus derive`.
pub fn run(args: DeriveArgs) -> Result<u8> {
    let root = work_tree(args.repo.repo)?;
    let refused = |reason: String| Error::Refused {
        code: EXIT_UNMEASURED,
        reason: format!("derive: {reason}"),
    };
    let ambient = Ambient::capture();
    let step = match args.command {
        DeriveCommand::Plan => {
            let plan = plan(&root, &ambient).map_err(refused)?;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            let w = |e: std::io::Error| Error::io("<stdout>", e);
            match args.format {
                OutputFormat::Json => writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&plan).unwrap_or_default()
                )
                .map_err(w)?,
                OutputFormat::Text => {
                    let state = serde_json::to_value(plan.state).unwrap_or_default();
                    writeln!(
                        out,
                        "stage B: {}  (key {}, recorded {})",
                        state.as_str().unwrap_or("?"),
                        plan.key.as_deref().unwrap_or("none"),
                        plan.recorded_key.as_deref().unwrap_or("none")
                    )
                    .map_err(w)?;
                    writeln!(out, "  {}", plan.sentence()).map_err(w)?;
                    writeln!(
                        out,
                        "  {} input(s), {} output file(s), {} moved",
                        plan.inputs, plan.outputs, plan.moved
                    )
                    .map_err(w)?;
                }
            }
            return Ok(0);
        }
        DeriveCommand::Begin { mode } => {
            let mode = match mode {
                DeriveMode::Incremental => Mode::Incremental,
                DeriveMode::Shadow => Mode::Shadow,
            };
            begin(&root, &ambient, mode).map_err(refused)?
        }
        DeriveCommand::Record => record(&root, &ambient).map_err(refused)?,
        DeriveCommand::Forget => forget(&root).map_err(refused)?,
    };
    say(&step);
    Ok(step.code)
}

/// The step's lines, on stderr: they are progress of the derive, and stdout is its caller's.
fn say(step: &Step) {
    let stderr = std::io::stderr();
    let mut err = stderr.lock();
    for line in &step.lines {
        let _ = writeln!(err, "{line}");
    }
}

/// The top of the work tree that holds `start` (default: the current directory).
fn work_tree(start: Option<PathBuf>) -> Result<PathBuf> {
    let start = match start {
        Some(p) => p,
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let out = read_only(&start)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| Error::Git {
            reason: format!("cannot run git: {e}"),
        })?;
    let top = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || top.is_empty() {
        return Err(Error::Refused {
            code: EXIT_UNMEASURED,
            reason: format!(
                "derive: {} is not inside a git work tree, and a derive's state belongs to one",
                start.display()
            ),
        });
    }
    Ok(PathBuf::from(top))
}
