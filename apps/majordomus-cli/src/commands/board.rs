//! `majordomus board`: the delivery board's documents on the command line. The subcommand
//! runs the capability its CLI path names in-process and owns the rendering alone; the
//! lanes, the composition and the digest live in the capability, and `--format json`
//! prints the answer the HTTP route and the MCP tool serve.

use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{BoardArgs, BoardCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus board`.
pub fn run(args: BoardArgs) -> Result<u8> {
    let BoardCommand::Snapshot { by } = args.command;
    // the repository and the answer fold into one result: a failure leaves by one exit
    let value = App::load(&args.repo)
        .and_then(|app| execute(&app.context, &["board", "snapshot"], json!({ "by": by })))?;
    print!(
        "{}",
        match args.format {
            OutputFormat::Json => format!("{value:#}\n"),
            OutputFormat::Text => text(&value),
        }
    );
    Ok(0)
}

/// Run the capability exposed as `majordomus <words>`, its refusal as this command's.
fn execute(ctx: &crate::capability::Context, words: &[&str], input: Value) -> Result<Value> {
    let words: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str().to_string())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                words.join(" ")
            ),
        })?;
    ctx.execute(&id, input).map_err(refusal)
}

/// The exit code of a refused snapshot: an open milestone in no lane is a verdict not
/// satisfied, 10, as `quality report` and `shell check` answer one; anything else is the
/// executable's own failure.
pub const EXIT_UNLANED: u8 = 10;

fn refusal(e: CapabilityError) -> Error {
    match e {
        CapabilityError::Refused(reason) => Error::Refused {
            code: EXIT_UNLANED,
            reason,
        },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

/// The snapshot as a person reads it: the digest and commit, then one line per document.
fn text(v: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "digest     {}",
        v["digest"].as_str().unwrap_or_default()
    );
    let _ = writeln!(
        out,
        "master     {} {}",
        v["master_sha"].as_str().unwrap_or_default(),
        v["master_committed_at"].as_str().unwrap_or_default()
    );
    for (id, doc) in v["documents"].as_object().into_iter().flatten() {
        let _ = writeln!(
            out,
            "{id:<20} {:>4} item(s)  {:>7} bytes",
            doc["items"].as_array().map_or(0, Vec::len),
            v["sizes"][id].as_u64().unwrap_or_default()
        );
    }
    for d in v["diagnostics"].as_array().into_iter().flatten() {
        let _ = writeln!(out, "note       {}", d.as_str().unwrap_or_default());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_keeps_its_words_and_exits_10() {
        let e = refusal(CapabilityError::Refused("milestone m is in no lane".into()));
        assert_eq!(e.exit_code(), EXIT_UNLANED);
        assert_eq!(e.to_string(), "milestone m is in no lane");
        let e = refusal(CapabilityError::Internal("broken".into()));
        assert_eq!(e.exit_code(), 13);
    }

    #[test]
    fn the_text_names_the_digest_and_every_document() {
        let v = json!({
            "digest": "sha256:ab", "master_sha": "abc", "master_committed_at": "2026-10-28",
            "sizes": {"meta": 10, "prs": 20},
            "documents": {"meta": {}, "prs": {"items": [1, 2]}},
            "diagnostics": ["the pull request queue is not observed"],
        });
        let t = text(&v);
        assert!(
            t.starts_with("digest     sha256:ab\nmaster     abc 2026-10-28\n"),
            "{t}"
        );
        assert!(
            t.contains("meta                    0 item(s)       10 bytes"),
            "{t}"
        );
        assert!(
            t.contains("prs                     2 item(s)       20 bytes"),
            "{t}"
        );
        assert!(
            t.ends_with("note       the pull request queue is not observed\n"),
            "{t}"
        );
    }

    /// The command answers through the capability its words name, and words no capability
    /// is exposed as, or a directory that holds no repository, are errors before anything runs.
    #[test]
    fn the_snapshot_runs_by_its_words_and_what_is_missing_is_an_error() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let v = execute(&ctx, &["board", "snapshot"], json!({ "by": "t" })).expect("a snapshot");
        assert_eq!(v["documents"]["meta"]["syncedBy"], "t");
        let err = execute(&ctx, &["board", "nothing"], json!({})).unwrap_err();
        assert!(
            matches!(&err, Error::Protocol { reason }
                if reason == "no capability is exposed as `majordomus board nothing`"),
            "{err:?}"
        );
        let empty = tempfile::tempdir().expect("a directory");
        let args = BoardArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(empty.path().to_path_buf()),
                discovery: crate::cli::DiscoveryMode::Filesystem,
                strict: false,
                share: Some(crate::synthetic::crate_share()),
            },
            command: BoardCommand::Snapshot { by: None },
            format: OutputFormat::Json,
        };
        assert!(run(args).is_err());
    }
}
