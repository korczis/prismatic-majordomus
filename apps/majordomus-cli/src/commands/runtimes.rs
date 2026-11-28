//! `majordomus runtimes --host`: every Majordomus runtime of this user on this host (I3184).
//!
//! The command-line projection of `server.runtimes`, answered in this process: the question
//! is asked from outside every checkout as readily as from inside one, so no layer is loaded
//! and no server is asked or started. The reading is the capability's own function
//! ([`crate::capability::builtin::server::host_runtimes`]), so the command line, the MCP tool
//! and the HTTP route of any running server cannot drift into two answers. Exit 0 whatever
//! it finds: an empty host is an answer.

use std::io::Write;

use crate::capability::builtin::server::{host_runtimes, FoundBy, HostRuntimes};
use crate::cli::{OutputFormat, RuntimesArgs};
use crate::error::{Error, Result};

/// Run `majordomus runtimes`.
pub fn run(args: RuntimesArgs) -> Result<u8> {
    let stdout = std::io::stdout();
    emit(&mut stdout.lock(), &host_runtimes(), args.format)
}

/// Write the inventory to `out`: exit 0, or the transport error that stopped it.
fn emit(out: &mut impl Write, inventory: &HostRuntimes, format: OutputFormat) -> Result<u8> {
    write(out, inventory, format).map_err(Error::Transport)?;
    Ok(0)
}

/// Render the inventory in `format`.
fn write(
    out: &mut impl Write,
    inventory: &HostRuntimes,
    format: OutputFormat,
) -> std::io::Result<()> {
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(inventory).unwrap_or_default()
        ),
        OutputFormat::Text => write_text(out, inventory),
    }
}

/// The lines a person reads: where the reading came from, then one line a checkout.
fn write_text(out: &mut impl Write, inventory: &HostRuntimes) -> std::io::Result<()> {
    writeln!(
        out,
        "node       {}",
        inventory
            .node
            .as_deref()
            .unwrap_or("none (no node.json read)")
    )?;
    let index = &inventory.index;
    match (&index.path, &index.problem) {
        (_, Some(problem)) => writeln!(out, "index      unreadable: {problem}")?,
        (Some(path), None) => writeln!(
            out,
            "index      {} ({} root(s))",
            path.display(),
            index.entries
        )?,
        (None, None) => writeln!(out, "index      none (no state directory)")?,
    }
    let table = &inventory.process_table;
    match &table.problem {
        Some(problem) => writeln!(out, "processes  unread: {problem}")?,
        None => writeln!(out, "processes  {} serve/mcp process(es)", table.processes)?,
    }
    writeln!(out, "reader     {}", inventory.version)?;
    writeln!(out)?;
    if inventory.runtimes.is_empty() {
        writeln!(out, "no Majordomus runtime of this user on this host")?;
    } else {
        writeln!(
            out,
            "{:<9} {:<5} {:<10} {:<6} {:<8} {:<20} CHECKOUT",
            "STANDING", "LIVE", "VERSION", "PORT", "PID", "FOUND"
        )?;
    }
    for r in &inventory.runtimes {
        let found: Vec<&str> = r
            .found_by
            .iter()
            .map(|f| match f {
                FoundBy::Index => "index",
                FoundBy::ProcessTable => "process_table",
            })
            .collect();
        writeln!(
            out,
            "{:<9} {:<5} {:<10} {:<6} {:<8} {:<20} {}",
            r.standing.as_str(),
            if r.live { "yes" } else { "no" },
            r.version.as_deref().unwrap_or("-"),
            r.port.map_or("-".to_string(), |p| p.to_string()),
            r.lease
                .as_ref()
                .map_or("-".to_string(), |l| l.pid.to_string()),
            found.join(","),
            r.checkout.display()
        )?;
        if let Some(reason) = &r.reason {
            writeln!(out, "          {reason}")?;
        }
    }
    if inventory.not_running > 0 {
        writeln!(
            out,
            "{} indexed checkout(s) have no lease and no process: not running, not listed",
            inventory.not_running
        )?;
    }
    for p in &inventory.unplaced {
        writeln!(
            out,
            "unplaced   pid {} ({:?}, {}): its checkout could not be found",
            p.pid,
            p.subcommand,
            p.executable.display()
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::builtin::server::{
        HostIndexView, HostProcessView, HostRuntime, ProcessTableView, ServerStanding,
        VersionSource,
    };
    use crate::lease::host::Subcommand;

    fn render(inventory: &HostRuntimes, format: OutputFormat) -> String {
        let mut out = Vec::new();
        write(&mut out, inventory, format).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn empty() -> HostRuntimes {
        HostRuntimes {
            version: "1.0.0".into(),
            node: None,
            index: HostIndexView {
                path: None,
                entries: 0,
                problem: None,
            },
            process_table: ProcessTableView {
                read: false,
                processes: 0,
                problem: Some("no ps".into()),
            },
            runtimes: Vec::new(),
            not_running: 0,
            unplaced: Vec::new(),
        }
    }

    /// An empty host says so, and says where it looked and what it could not read.
    #[test]
    fn an_empty_host_is_an_answer() {
        let text = render(&empty(), OutputFormat::Text);
        assert!(text.contains("no Majordomus runtime"), "{text}");
        assert!(text.contains("index      none"), "{text}");
        assert!(text.contains("processes  unread: no ps"), "{text}");
        assert!(text.contains("node       none"), "{text}");
        let mut unreadable = empty();
        unreadable.index.problem = Some("cannot be read".into());
        assert!(render(&unreadable, OutputFormat::Text).contains("index      unreadable"));
        let v: serde_json::Value =
            serde_json::from_str(&render(&empty(), OutputFormat::Json)).unwrap();
        assert_eq!(v["runtimes"], serde_json::json!([]));
    }

    /// Every runtime is one line, its reason beneath it, and what is not listed is counted.
    #[test]
    fn every_runtime_is_one_line() {
        let inventory = full();
        let text = render(&inventory, OutputFormat::Text);
        assert!(
            text.contains("index      /s/checkout-roots (3 root(s))"),
            "{text}"
        );
        assert!(
            text.contains("processes  1 serve/mcp process(es)"),
            "{text}"
        );
        assert!(text.contains("STANDING"), "{text}");
        let ready = text.lines().find(|l| l.ends_with(" /a")).unwrap();
        assert!(
            ready.starts_with("ready")
                && ready.contains("8741")
                && ready.contains("index,process_table"),
            "{ready}"
        );
        let stale = text.lines().find(|l| l.ends_with(" /b")).unwrap();
        assert!(
            stale.starts_with("stale") && stale.contains(" no "),
            "{stale}"
        );
        assert!(text.contains("the server does not answer"), "{text}");
        assert!(text.contains("1 indexed checkout(s)"), "{text}");
        assert!(text.contains("unplaced   pid 9"), "{text}");
    }

    /// A writer that accepts `left` writes and refuses every one after.
    struct Refusing {
        left: usize,
    }

    impl Write for Refusing {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.left == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "closed",
                ));
            }
            self.left -= 1;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// An output that closes at any point of the text is the transport error that stopped
    /// it, never a partial answer called complete; an output that stays open is exit 0.
    #[test]
    fn an_output_that_closes_is_a_transport_error() {
        let mut unreadable = full();
        unreadable.index.problem = Some("cannot be read".into());
        unreadable.process_table.problem = Some("no ps".into());
        for inventory in [empty(), full(), unreadable] {
            for format in [OutputFormat::Text, OutputFormat::Json] {
                let mut open = Vec::new();
                assert_eq!(emit(&mut open, &inventory, format).unwrap(), 0);
                let mut refusals = 0;
                for left in 0.. {
                    match emit(&mut Refusing { left }, &inventory, format) {
                        Err(Error::Transport(e)) => {
                            assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe);
                            refusals += 1;
                        }
                        Err(other) => panic!("not a transport error: {other}"),
                        Ok(code) => {
                            assert_eq!(code, 0);
                            break;
                        }
                    }
                }
                assert!(refusals > 0, "the output was never refused");
            }
        }
    }

    /// An inventory with a line of every kind.
    fn full() -> HostRuntimes {
        let mut inventory = empty();
        inventory.node = Some("n1".into());
        inventory.index = HostIndexView {
            path: Some("/s/checkout-roots".into()),
            entries: 3,
            problem: None,
        };
        inventory.process_table = ProcessTableView {
            read: true,
            processes: 1,
            problem: None,
        };
        inventory.not_running = 1;
        inventory.unplaced = vec![HostProcessView {
            pid: 9,
            subcommand: Subcommand::Mcp,
            executable: "/bin/majordomus".into(),
            holds_lease: false,
        }];
        inventory.runtimes = vec![
            HostRuntime {
                checkout: "/a".into(),
                checkout_id: "a".into(),
                git: None,
                found_by: vec![FoundBy::Index, FoundBy::ProcessTable],
                standing: ServerStanding::Ready,
                live: true,
                reason: None,
                lease: Some(crate::capability::builtin::server::LeaseView {
                    pid: 41,
                    url: Some("http://127.0.0.1:8741".into()),
                    started_at: "t".into(),
                    executable: None,
                    version: Some("1.0.0".into()),
                }),
                port: Some(8741),
                version: Some("1.0.0".into()),
                version_source: Some(VersionSource::Lease),
                processes: Vec::new(),
            },
            HostRuntime {
                checkout: "/b".into(),
                checkout_id: "b".into(),
                git: None,
                found_by: vec![FoundBy::Index],
                standing: ServerStanding::Stale,
                live: false,
                reason: Some("the server does not answer".into()),
                lease: None,
                port: None,
                version: None,
                version_source: None,
                processes: Vec::new(),
            },
        ];
        inventory
    }
}
