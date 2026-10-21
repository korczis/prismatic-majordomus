//! `majordomus deploy`: the command line of `deploy.slug`, through the one executor.
//!
//! The text rendering says what was assembled, from which commit and where, then the
//! top-level entries and the provider the slug is for; `--format json` prints the capability's
//! answer whole, which is what a workflow reads the provider's application and URL from. A
//! refusal exits 15, something the request names that does not exist exits 12.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{DeployArgs, DeployCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus deploy`.
pub fn run(args: DeployArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let DeployCommand::Slug {
        deployment,
        commit,
        executable,
        out,
    } = args.command;
    let mut input = json!({ "executable": executable, "out": out });
    if let Some(d) = deployment {
        input["deployment"] = json!(d);
    }
    if let Some(c) = commit {
        input["commit"] = json!(c);
    }
    let v = execute(ctx, input)?;
    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(
            w,
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_default()
        ),
        OutputFormat::Text => writeln!(w, "{}", text(&v)),
    }
    .map_err(Error::Transport)?;
    Ok(0)
}

/// The slug as a person reads it.
fn text(v: &Value) -> String {
    let s = |p: &str| {
        v.pointer(p)
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string()
    };
    let entries = v["entries"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    format!(
        "assembled    {} at {} into {}\n\
         entries      {entries}\n\
         files        {} file(s), {} byte(s)\n\
         executable   {} sha256 {}\n\
         provider     {} {} {}{}",
        s("/deployment"),
        s("/commit"),
        s("/out"),
        v["files"],
        v["bytes"],
        s("/executable"),
        s("/executable_sha256"),
        s("/provider/name"),
        s("/provider/app"),
        s("/provider/url"),
        s("/liveness"),
    )
}

fn execute(ctx: &crate::capability::Context, input: Value) -> Result<Value> {
    let words = ["deploy".to_string(), "slug".to_string()];
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus deploy slug`".into(),
        })?;
    ctx.execute(id, input).map_err(|e| match e {
        CapabilityError::Refused(reason) => Error::Refused { code: 15, reason },
        CapabilityError::NotFound(reason) => Error::Refused { code: 12, reason },
        CapabilityError::InvalidInput(reason) => Error::Refused { code: 2, reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slug_reads_as_what_where_and_for_whom() {
        let v = json!({"deployment": "majordomus", "commit": "a".repeat(40), "out": "/tmp/slug",
            "entries": [".ai", "Procfile"], "files": 2, "bytes": 10,
            "executable": ".majordomus/bin/majordomus", "executable_sha256": "00",
            "buildpack": "b", "liveness": "/api/v1/live",
            "provider": {"name": "gigalixir", "app": "a-b", "stack": "s",
                         "url": "https://a-b.gigalixirapp.com"}});
        let t = text(&v);
        assert!(t.starts_with(&format!(
            "assembled    majordomus at {} into /tmp/slug",
            "a".repeat(40)
        )));
        assert!(t.contains("entries      .ai Procfile\n"));
        assert!(t.ends_with("provider     gigalixir a-b https://a-b.gigalixirapp.com/api/v1/live"));
    }
}
