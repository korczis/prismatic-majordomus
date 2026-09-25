//! `majordomus served`: what each deployment serves, as observed and as it stands now.
//!
//! Both subcommands run a capability of the `served` module through the one executor. The
//! text rendering prints the verdict first and the reason beside it, and `observe` exits
//! with the verdict's code, so a script reads the same three-way answer a person does:
//! 0 served, 10 measurably not, 12 not answered.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, ServedArgs, ServedCommand};
use crate::error::{Error, Result};

/// Run `majordomus served`.
pub fn run(args: ServedArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command {
        ServedCommand::Observe {
            commit,
            url,
            identity,
            deployment,
            timeout,
            dry_run,
        } => {
            let input = json!({
                "commit": commit, "url": url, "identity": identity,
                "deployment": deployment, "timeout_seconds": timeout, "dry_run": dry_run,
            });
            let v = execute(ctx, &["served", "observe"], strip_nulls(input))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)),
                OutputFormat::Text => {
                    let o = &v["observation"];
                    write_line(&mut out, o, o["deployment"].as_str().unwrap_or("?"))?;
                    let recorded = v["recorded"].as_str().unwrap_or("not recorded (dry run)");
                    writeln!(out, "recorded     {recorded}")
                }
            }
            .map_err(Error::Transport)?;
            Ok(u8::try_from(v["exit_code"].as_i64().unwrap_or(12)).unwrap_or(12))
        }
        ServedCommand::Show { commit, deployment } => {
            let input = strip_nulls(json!({ "commit": commit, "deployment": deployment }));
            let v = execute(ctx, &["served", "show"], input)?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => {
                    let expected = v["expected"].as_str().unwrap_or("?");
                    writeln!(out, "against      {}", &expected[..expected.len().min(12)])
                        .map_err(Error::Transport)?;
                    let rows = v["deployments"].as_array().cloned().unwrap_or_default();
                    if rows.is_empty() {
                        writeln!(out, "nothing observed yet: majordomus served observe")
                            .map_err(Error::Transport)?;
                    }
                    for d in &rows {
                        let now =
                            json!({ "verdict": d["now"]["verdict"], "reason": d["now"]["reason"] });
                        write_line(&mut out, &now, d["deployment"].as_str().unwrap_or("?"))?;
                        writeln!(
                            out,
                            "  seen       {}",
                            d["observation"]["at"].as_str().unwrap_or("?")
                        )
                        .map_err(Error::Transport)?;
                    }
                    if let Some(n) = v["unreadable"].as_u64().filter(|n| *n > 0) {
                        writeln!(out, "unreadable   {n} line(s) of {}", v["observations"])
                            .map_err(Error::Transport)?;
                    }
                }
            }
            Ok(0)
        }
    }
}

fn write_line(out: &mut std::io::StdoutLock<'_>, judged: &Value, name: &str) -> Result<()> {
    writeln!(
        out,
        "{name:<12} {:<11} {}",
        judged["verdict"].as_str().unwrap_or("?"),
        judged["reason"].as_str().unwrap_or("")
    )
    .map_err(Error::Transport)
}

fn strip_nulls(mut v: Value) -> Value {
    if let Some(m) = v.as_object_mut() {
        m.retain(|_, x| !x.is_null());
    }
    v
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

fn execute(ctx: &crate::capability::Context, path: &[&str], input: Value) -> Result<Value> {
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
    ctx.execute(id, input).map_err(|e| match e {
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Protocol { reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })
}
