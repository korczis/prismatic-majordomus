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
        ServedCommand::Forge => {
            let v = execute(ctx, &["served", "forge"], json!({}))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)),
                OutputFormat::Text => writeln!(out, "{}", forge_line(&v)),
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

/// The receipt of asking the forge, as one line: what it held, or why it was not asked.
fn forge_line(v: &Value) -> String {
    if v["observed"].as_bool() == Some(true) {
        format!(
            "observed {} deployment(s) to {} environment(s) and {} release(s) of {} at {}",
            v["deployments"],
            v["environments"],
            v["releases"],
            v["repository"].as_str().unwrap_or("?"),
            v["observed_at"].as_str().unwrap_or("?"),
        )
    } else {
        format!(
            "the forge's deployments were not observed: {}",
            v["reason"].as_str().unwrap_or("no reason was given")
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_receipt_of_asking_the_forge_is_one_line_either_way() {
        let held = json!({"observed": true, "deployments": 50, "environments": 2, "releases": 21,
            "repository": "o/r", "observed_at": "2026-10-12T21:00:00Z", "exit_code": 0});
        assert_eq!(
            forge_line(&held),
            "observed 50 deployment(s) to 2 environment(s) and 21 release(s) of o/r at 2026-10-12T21:00:00Z"
        );
        let refused = json!({"observed": false, "reason": "gh could not run", "exit_code": 12});
        assert_eq!(
            forge_line(&refused),
            "the forge's deployments were not observed: gh could not run"
        );
        assert!(forge_line(&json!({})).ends_with("no reason was given"));
    }

    /// `served forge`, run whole, in a repository the forge does not know: exit 12, said.
    #[test]
    fn asking_a_forge_that_knows_nothing_ends_on_not_answered() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        for format in [OutputFormat::Text, OutputFormat::Json] {
            let code = run(ServedArgs {
                repo: crate::cli::RepoArgs {
                    repo: Some(repo.root().to_path_buf()),
                    discovery: crate::cli::DiscoveryMode::Filesystem,
                    strict: false,
                    share: Some(
                        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share"),
                    ),
                },
                command: ServedCommand::Forge,
                format,
            })
            .expect("a receipt, not an error");
            assert_eq!(code, 12);
        }
    }
}
