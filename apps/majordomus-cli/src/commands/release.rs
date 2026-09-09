//! `majordomus release`: the changelog, the version, and the one writer that raises it.
//!
//! The read half delegates to the capabilities, so the command line renders exactly what
//! HTTP and MCP answer with. `bump` does not: it writes tracked files, which no capability
//! may do, and the exposure policy keeps it off every machine surface for that reason.

use std::io::Write;

use crate::app::App;
use crate::cli::{OutputFormat, ReleaseArgs, ReleaseCommand};
use crate::error::{Error, Result};
use crate::release::{self, changelog, version};

/// Exit code when the two writers of the version disagree, matching
/// `scripts/release-version --check`.
pub const EXIT_DISAGREE: u8 = 10;

/// Run `majordomus release`.
pub fn run(args: ReleaseArgs) -> Result<u8> {
    match args.command {
        None => render_changelog(&args, None),
        Some(ReleaseCommand::Changelog { ref version }) => {
            let v = version.clone();
            render_changelog(&args, v)
        }
        Some(ReleaseCommand::Version) => render_version(&args),
        Some(ReleaseCommand::Bump {
            ref level,
            ref exact,
            dry_run,
        }) => bump(&args, level.as_deref(), exact.as_deref(), dry_run),
    }
}

/// The changelog, through the capability so that every surface renders one value.
fn render_changelog(args: &ReleaseArgs, only: Option<String>) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let mut input = serde_json::Map::new();
    if let Some(v) = only {
        input.insert("version".into(), serde_json::Value::String(v));
    }
    let value = app
        .context
        .execute("release.changelog", serde_json::Value::Object(input))
        .map_err(|e| Error::Refused {
            code: 12,
            reason: e.to_string(),
        })?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            let log: release::Changelog =
                serde_json::from_value(value).map_err(|e| Error::Protocol {
                    reason: e.to_string(),
                })?;
            write!(out, "{}", changelog::render(&log)).map_err(Error::Transport)?;
        }
    }
    Ok(0)
}

/// The version report. Exits 10 when the two writers disagree, which is the same verdict
/// `scripts/release-version --check` gives and the same code.
fn render_version(args: &ReleaseArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let value = app
        .context
        .execute("release.version", serde_json::json!({}))
        .map_err(|e| Error::Refused {
            code: 12,
            reason: e.to_string(),
        })?;
    let report: release::model::VersionReport =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            writeln!(out, "declared     {}", report.declared).map_err(Error::Transport)?;
            writeln!(out, "tool         {}", report.tool).map_err(Error::Transport)?;
            writeln!(
                out,
                "agree        {}",
                if report.agree { "yes" } else { "NO" }
            )
            .map_err(Error::Transport)?;
            writeln!(
                out,
                "last release {}",
                report.last_release.as_deref().unwrap_or("—")
            )
            .map_err(Error::Transport)?;
            writeln!(out, "commits      {} since it", report.changes.len())
                .map_err(Error::Transport)?;
            writeln!(out, "bump         {}", report.bump).map_err(Error::Transport)?;
            writeln!(
                out,
                "next         {}",
                report.next.as_deref().unwrap_or("—")
            )
            .map_err(Error::Transport)?;
        }
    }
    Ok(if report.agree { 0 } else { EXIT_DISAGREE })
}

/// Raise the version in both places.
fn bump(args: &ReleaseArgs, level: Option<&str>, exact: Option<&str>, dry_run: bool) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let root = std::path::Path::new(&app.index().repository.root).to_path_buf();
    let report = version::report(&root, &app.index().objects);

    let to = match exact {
        Some(v) => {
            version::Version::parse(v).ok_or_else(|| Error::Refused {
                code: 2,
                reason: format!("'{v}' is not a version; it must be three numbers, `1.2.3`"),
            })?;
            v.to_string()
        }
        None => {
            let bump = match level {
                Some(word) => version::Bump::parse(word).ok_or_else(|| Error::Refused {
                    code: 2,
                    reason: format!(
                        "'{word}' is not a level; it is one of major, minor, patch, none"
                    ),
                })?,
                None => version::Bump::parse(&report.bump).unwrap_or(version::Bump::None),
            };
            let current =
                version::Version::parse(&report.declared).ok_or_else(|| Error::Refused {
                    code: 12,
                    reason: format!(
                        "the declared version '{}' is not three numbers, so it cannot be raised",
                        report.declared
                    ),
                })?;
            current.raised(bump).to_string()
        }
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if to == report.declared {
        writeln!(
            out,
            "release: the version is already {to}; nothing written ({} commit(s) since {})",
            report.changes.len(),
            report.last_release.as_deref().unwrap_or("the beginning")
        )
        .map_err(Error::Transport)?;
        return Ok(0);
    }
    if dry_run {
        writeln!(
            out,
            "release: would raise {} -> {to} ({}, from {} commit(s))",
            report.declared,
            report.bump,
            report.changes.len()
        )
        .map_err(Error::Transport)?;
        writeln!(out, "         {}", version::MANIFEST).map_err(Error::Transport)?;
        writeln!(out, "         {}", version::ENTRY).map_err(Error::Transport)?;
        return Ok(0);
    }

    let written = version::write(&root, &to).map_err(|e| Error::io(root.clone(), e))?;
    writeln!(out, "release: {} -> {to}", report.declared).map_err(Error::Transport)?;
    for f in &written {
        writeln!(out, "         {f} written").map_err(Error::Transport)?;
    }
    // The check the release workflow runs, run here, so a bump that half-applied is caught
    // by the thing built to catch it rather than by the release.
    let after = version::report(&root, &app.index().objects);
    if !after.agree {
        writeln!(
            out,
            "release: the two writers disagree after the bump ({} vs {})",
            after.declared, after.tool
        )
        .map_err(Error::Transport)?;
        return Ok(EXIT_DISAGREE);
    }
    writeln!(
        out,
        "         both writers agree; `majordomus generate changelog` renders the new section"
    )
    .map_err(Error::Transport)?;
    Ok(0)
}
