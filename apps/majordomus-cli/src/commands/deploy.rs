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
    run_to(args, &mut std::io::stdout().lock())
}

/// Run `majordomus deploy`, printing to `w`.
fn run_to(args: DeployArgs, w: &mut dyn Write) -> Result<u8> {
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

    use crate::cli::{Cli, Command, DiscoveryMode, RepoArgs};
    use crate::deploy::slug::testing::hosted_layer;

    fn args(
        root: &std::path::Path,
        deployment: Option<&str>,
        executable: &std::path::Path,
        out: &std::path::Path,
        format: OutputFormat,
    ) -> DeployArgs {
        DeployArgs {
            repo: RepoArgs {
                repo: Some(root.to_path_buf()),
                discovery: DiscoveryMode::Filesystem,
                share: Some(crate::synthetic::crate_share()),
                ..Default::default()
            },
            command: DeployCommand::Slug {
                deployment: deployment.map(String::from),
                commit: deployment.map(|_| "HEAD".to_string()),
                executable: executable.display().to_string(),
                out: out.display().to_string(),
            },
            format,
        }
    }

    /// The command prints what was assembled as a person reads it, or the capability's
    /// answer whole as JSON.
    #[test]
    fn the_deploy_command_prints_the_slug_as_text_or_json() {
        let repo = hosted_layer(&["hosted"]);
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = elsewhere.path().join("exe");
        std::fs::write(&exe, b"an executable").unwrap();

        let mut w = Vec::new();
        let a = args(
            repo.root(),
            Some("hosted"),
            &exe,
            &elsewhere.path().join("text"),
            OutputFormat::Text,
        );
        assert_eq!(run_to(a, &mut w).unwrap(), 0);
        let t = String::from_utf8(w).unwrap();
        assert!(t.starts_with("assembled    hosted at "), "{t}");
        assert!(t.contains("provider     gigalixir hosted-app"), "{t}");

        let mut w = Vec::new();
        let a = args(
            repo.root(),
            None,
            &exe,
            &elsewhere.path().join("json"),
            OutputFormat::Json,
        );
        assert_eq!(run_to(a, &mut w).unwrap(), 0);
        let v: Value = serde_json::from_slice(&w).unwrap();
        assert_eq!(v["provider"]["app"], "hosted-app");
        assert_eq!(v["deployment"], "hosted");
    }

    /// The command is dispatched from the command line, and a refusal exits 15 with
    /// nothing printed.
    #[test]
    fn the_deploy_command_is_dispatched_and_a_refusal_exits_15() {
        let repo = hosted_layer(&["hosted"]);
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = elsewhere.path().join("exe");
        std::fs::write(&exe, b"an executable").unwrap();
        let cli = Cli {
            command: Command::Deploy(args(
                repo.root(),
                None,
                &exe,
                elsewhere.path(),
                OutputFormat::Text,
            )),
        };
        let e = crate::commands::run(cli).unwrap_err();
        assert!(
            matches!(&e, Error::Refused { code: 15, reason } if reason.contains("not empty")),
            "{e}"
        );
    }

    /// Each capability error has its exit: not found 12, invalid input 2, and anything else
    /// a protocol error — as is a registry that exposes no `deploy slug` at all.
    #[test]
    fn every_deploy_slug_error_has_its_exit() {
        let repo = hosted_layer(&["hosted"]);
        let ctx = repo.context().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = elsewhere.path().join("exe");
        std::fs::write(&exe, b"an executable").unwrap();
        let out = elsewhere.path().join("slug").display().to_string();
        let code = |input: Value| match execute(&ctx, input) {
            Err(Error::Refused { code, .. }) => Some(code),
            Err(Error::Protocol { .. }) => None,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            code(json!({"deployment": "absent", "executable": exe, "out": out})),
            Some(12)
        );
        assert_eq!(code(json!({"out": out})), Some(2));
        assert_eq!(
            code(json!({"executable": exe, "out": exe.join("slug")})),
            None,
            "a failure under a sound request is not a refusal"
        );

        let index = repo.index().unwrap();
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_index(&index)
            .build()
            .unwrap();
        let bare = crate::capability::Context::new(
            std::sync::Arc::new(index),
            std::sync::Arc::new(registry),
        );
        assert!(matches!(
            execute(&bare, json!({})),
            Err(Error::Protocol { reason }) if reason.contains("majordomus deploy slug")
        ));
    }

    /// A slug assembled and not printed is a transport failure, not a success.
    #[test]
    fn a_deploy_slug_that_cannot_be_printed_is_a_transport_failure() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let repo = hosted_layer(&["hosted"]);
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = elsewhere.path().join("exe");
        std::fs::write(&exe, b"an executable").unwrap();
        let a = args(
            repo.root(),
            None,
            &exe,
            &elsewhere.path().join("slug"),
            OutputFormat::Text,
        );
        assert!(matches!(
            run_to(a, &mut Closed),
            Err(Error::Transport(e)) if e.kind() == std::io::ErrorKind::BrokenPipe
        ));
    }
}
