//! `majordomus campaigns staged`: the prompt packs staged before they are kept, through the
//! registry's `campaigns.staged` (ADR 0236, I4715).
//!
//! The command renders the capability's answer; the listing is the capability's, so the
//! terminal, `--format json`, MCP and HTTP cannot disagree about a pack.

use std::fmt::Write as _;
use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::campaigns::StagedReport;
use crate::cli::{CampaignsArgs, CampaignsCommand, OutputFormat};
use crate::error::{Error, Result};

/// The exit code when the primary checkout could not be derived: nothing was listed.
pub const EXIT_UNMEASURED: u8 = 12;

/// The capability the command renders.
const CAPABILITY: &str = "campaigns.staged";

/// Run `majordomus campaigns`.
pub fn run(args: CampaignsArgs) -> Result<u8> {
    run_to(args, &mut std::io::stdout().lock())
}

/// Run `majordomus campaigns`, printing to `out`.
fn run_to(args: CampaignsArgs, out: &mut dyn Write) -> Result<u8> {
    let CampaignsCommand::Staged = args.command;
    let app = App::load(&args.repo)?;
    let (value, report) = app
        .context
        .execute(CAPABILITY, json!({}))
        .map_err(|e| e.to_string())
        .and_then(read)
        .map_err(|reason| Error::Protocol { reason })?;
    let text = match args.format {
        OutputFormat::Json => format!("{value:#}\n"),
        OutputFormat::Text => render(&report),
    };
    out.write_all(text.as_bytes()).map_err(Error::Transport)?;
    Ok(if report.reason.is_some() { EXIT_UNMEASURED } else { 0 })
}

/// The capability's answer as the report it renders, kept beside the answer itself.
fn read(value: serde_json::Value) -> std::result::Result<(serde_json::Value, StagedReport), String> {
    serde_json::from_value(value.clone())
        .map(|report| (value, report))
        .map_err(|e| format!("{CAPABILITY} answered with something this command cannot read: {e}"))
}

/// One line per staging place and per pack, then the tallies.
fn render(report: &StagedReport) -> String {
    let mut s = String::new();
    if let Some(reason) = &report.reason {
        let _ = writeln!(s, "campaigns staged: not read: {reason}");
        return s;
    }
    let primary = report.primary.as_deref().unwrap_or("-");
    let _ = writeln!(s, "primary checkout {primary}");
    for place in &report.places {
        let legacy = if place.legacy { " (legacy)" } else { "" };
        match &place.reason {
            Some(reason) => {
                let _ = writeln!(s, "place   {}{legacy}: absent ({reason})", place.path);
            }
            None => {
                let _ = writeln!(
                    s,
                    "place   {}{legacy}: {} holding dir(s) that are not packs",
                    place.path,
                    place.holding.len()
                );
            }
        }
    }
    for p in &report.packs {
        let age = p.age_days.map(|d| format!("{d}d")).unwrap_or_else(|| "-".into());
        let stale = if p.stale { " STALE" } else { "" };
        let heading = p.first_heading.as_deref().unwrap_or("-");
        let _ = writeln!(
            s,
            "{:<9} {}/{}  {} file(s), {age}{stale}  {heading}",
            p.state.as_str(),
            p.place,
            p.name,
            p.files
        );
        for record in &p.cited_by {
            let _ = writeln!(s, "          cited by {record}");
        }
    }
    let tallies: Vec<String> = report.tallies.iter().map(|(k, n)| format!("{k} {n}")).collect();
    let bound = report
        .stale_after_days
        .map(|d| format!("{d} day(s)"))
        .unwrap_or_else(|| "no bound declared".into());
    let _ = writeln!(
        s,
        "campaigns staged: {} pack(s) [{}], {} stale past {bound}",
        report.packs.len(),
        tallies.join(", "),
        report.stale
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaigns::{staged, StagedReport};
    use crate::cli::{DiscoveryMode, RepoArgs};
    use crate::synthetic::{crate_share, SyntheticRepository};
    use std::time::{Duration, UNIX_EPOCH};

    fn report(root: &std::path::Path) -> StagedReport {
        let at = UNIX_EPOCH + Duration::from_secs(4_000_000_000);
        staged(root, Ok(root.to_path_buf()), at, Some(7))
    }

    /// Every place and pack is a line, a stale pack says so, and the tallies close it.
    #[test]
    fn the_listing_names_each_place_and_pack() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("tmp/owner-packs/alpha");
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(pack.join("README.md"), "# Alpha\n").unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/repo/adrs")).unwrap();
        std::fs::write(dir.path().join(".ai/repo/adrs/0001.md"), "tmp/owner-packs/alpha\n").unwrap();

        let text = render(&report(dir.path()));
        assert!(text.contains("place   tmp/owner-packs: 0 holding dir(s)"), "{text}");
        assert!(text.contains("place   tmp/prompt-packs (legacy): absent ("), "{text}");
        assert!(text.contains("staged    tmp/owner-packs/alpha  1 file(s),"), "{text}");
        assert!(text.contains("STALE  Alpha"), "{text}");
        assert!(text.contains("cited by .ai/repo/adrs/0001.md"), "{text}");
        assert!(text.contains("1 pack(s) [staged 1], 1 stale past 7 day(s)"), "{text}");
    }

    /// An empty pack has no age or heading, and no bound is said as such.
    #[test]
    fn an_empty_pack_and_no_bound_are_said_plainly() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("tmp/owner-packs/empty")).unwrap();
        let mut r = report(dir.path());
        r.stale_after_days = None;
        let text = render(&r);
        assert!(text.contains("tmp/owner-packs/empty  0 file(s), -  -"), "{text}");
        assert!(text.contains("stale past no bound declared"), "{text}");
    }

    /// A primary checkout that could not be derived is one line with the reason.
    #[test]
    fn an_unread_report_is_one_line_with_its_reason() {
        let r = staged(std::path::Path::new("/"), Err("no git".into()), UNIX_EPOCH, None);
        assert_eq!(render(&r), "campaigns staged: not read: no git\n");
    }

    fn args(root: &std::path::Path, format: OutputFormat) -> CampaignsArgs {
        CampaignsArgs {
            repo: RepoArgs {
                repo: Some(root.to_path_buf()),
                discovery: DiscoveryMode::Filesystem,
                strict: false,
                share: Some(crate_share()),
            },
            command: CampaignsCommand::Staged,
            format,
        }
    }

    /// The command runs the capability: a synthetic layer is no git work tree, so both
    /// shapes say the primary could not be derived and exit 12.
    #[test]
    fn the_command_renders_the_capability_and_exits_on_its_reason() {
        let repo = SyntheticRepository::small().unwrap();
        let mut out = Vec::new();
        let code = run_to(args(repo.root(), OutputFormat::Text), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(code, EXIT_UNMEASURED, "{text}");
        assert!(text.starts_with("campaigns staged: not read: "), "{text}");

        let mut out = Vec::new();
        let code = run_to(args(repo.root(), OutputFormat::Json), &mut out).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(code, EXIT_UNMEASURED);
        assert!(value["reason"].is_string(), "{value}");
    }

    /// In a git work tree the primary is derived and the command exits 0.
    #[test]
    fn in_a_work_tree_the_command_exits_zero() {
        let repo = SyntheticRepository::small().unwrap();
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.root())
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let mut out = Vec::new();
        let code = run_to(args(repo.root(), OutputFormat::Text), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("campaigns staged: 0 pack(s) []"), "{text}");
    }

    /// A writer that refuses every write, as a closed pipe does.
    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// An output that cannot be written is a transport error, not a listing silently lost.
    #[test]
    fn an_output_that_cannot_be_written_is_a_transport_error() {
        let repo = SyntheticRepository::small().unwrap();
        let err = run_to(args(repo.root(), OutputFormat::Text), &mut Closed).unwrap_err();
        assert!(matches!(err, Error::Transport(_)), "{err:?}");
        assert!(Closed.flush().is_ok());
    }

    /// An answer that is not a report is a protocol error naming the capability.
    #[test]
    fn an_unreadable_answer_names_the_capability() {
        let err = read(json!({"packs": 3})).unwrap_err();
        assert!(err.contains("campaigns.staged answered"), "{err}");
    }
}
