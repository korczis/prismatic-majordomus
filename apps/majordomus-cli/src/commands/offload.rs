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
use crate::offload::host::{probe, HostReport, Standing};
use crate::offload::run::{offload, OffloadReport, OffloadRequest, EXIT_UNAVAILABLE};

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
            let chosen = probe_report(&reports, args.format, &mut out).map_err(w)?;
            Ok(if chosen { 0 } else { EXIT_UNAVAILABLE })
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
            derive_report(&report, args.format, &mut out).map_err(w)?;
            Ok(0)
        }
    }
}

/// Print every host's measurement; answers whether one was chosen.
fn probe_report(
    reports: &[HostReport],
    format: OutputFormat,
    out: &mut dyn Write,
) -> std::io::Result<bool> {
    let chosen = reports.iter().find(|r| r.chosen).map(|r| r.id.clone());
    match format {
        OutputFormat::Json => {
            let doc = json!({ "chosen": chosen, "hosts": reports });
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&doc).unwrap_or_default()
            )?;
        }
        OutputFormat::Text => {
            for r in reports {
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
                )?;
            }
            match &chosen {
                Some(id) => writeln!(out, "a derive would go to {id}")?,
                None => writeln!(out, "no declared host can take a derive")?,
            }
        }
    }
    Ok(chosen.is_some())
}

/// Print what an offloaded derive did.
fn derive_report(
    report: &OffloadReport,
    format: OutputFormat,
    out: &mut dyn Write,
) -> std::io::Result<()> {
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(report).unwrap_or_default()
        ),
        OutputFormat::Text => {
            writeln!(
                out,
                "derived {} on {} in {} s ({} s in the queue): {} file(s) staged, nothing committed",
                report.tag,
                report.host,
                report.seconds,
                report.waited_seconds,
                report.staged.len()
            )?;
            for f in &report.staged {
                writeln!(out, "  {f}")?;
            }
            Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer that refuses the first write carrying `marker`, as a closed pipe would at
    /// that line, and keeps everything written before it.
    struct FailOn {
        marker: &'static str,
        written: Vec<u8>,
    }

    impl FailOn {
        fn new(marker: &'static str) -> FailOn {
            FailOn {
                marker,
                written: Vec::new(),
            }
        }

        fn text(&self) -> String {
            String::from_utf8(self.written.clone()).unwrap()
        }
    }

    impl Write for FailOn {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if String::from_utf8_lossy(buf).contains(self.marker) {
                return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
            }
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn declaration() -> OffloadDeclaration {
        serde_json::from_value(json!({
            "schema": "derive-offload/v1", "kind": "derive-offload", "id": "derive",
            "hosts": [
                { "id": "lundra", "address": "u@lundra" },
                { "id": "macbook", "address": "u@macbook", "identity_file": "~/.ssh/k",
                  "min_free_gib": 20 }
            ]
        }))
        .unwrap()
    }

    fn report(id: &str, standing: Standing, chosen: bool) -> HostReport {
        HostReport {
            id: id.into(),
            address: format!("u@{id}"),
            min_free_gib: 10,
            free_gib: (standing != Standing::Unreachable).then_some(50),
            load: (standing != Standing::Unreachable).then_some(1),
            standing,
            reason: (standing != Standing::Eligible).then(|| format!("{id} is out")),
            chosen,
        }
    }

    #[test]
    fn the_hosts_are_listed_as_declared_or_said_to_be_absent() {
        let mut out = FailOn::new("\0");
        hosts(None, OutputFormat::Text, &mut out).unwrap();
        assert_eq!(
            out.text(),
            "no derive-offload declaration in this repository: nothing to offload to \
             (.ai/repo/offload/*.yaml)\n"
        );

        let mut out = FailOn::new("\0");
        hosts(None, OutputFormat::Json, &mut out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out.text()).unwrap();
        assert_eq!(v["declared"], false);

        let d = declaration();
        let mut out = FailOn::new("\0");
        hosts(Some(&d), OutputFormat::Json, &mut out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out.text()).unwrap();
        assert_eq!(v["declaration"]["hosts"][1]["identity_file"], "~/.ssh/k");

        let mut out = FailOn::new("\0");
        hosts(Some(&d), OutputFormat::Text, &mut out).unwrap();
        let text = out.text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            [
                "declaration derive — base origin/master, minimum 10 GiB free",
                "queue /tmp/offload-queue (lock /tmp/offload-derive.lock), a look every 5 s, \
                 at most 14400 s",
                "on a host: checkout ~/dev/prismatic-majordomus, worktrees \
                 ~/dev/prismatic-majordomus-wt, shared build ~/.cache/majordomus-offload-target, \
                 runs scripts/derive",
                "  lundra       u@lundra                     min  10 GiB",
                "  macbook      u@macbook                    min  20 GiB  -i ~/.ssh/k",
            ]
        );
    }

    /// Output that cannot be written is an error at whichever line it fails, never a listing
    /// cut short in silence.
    #[test]
    fn a_hosts_listing_that_cannot_be_written_is_an_error() {
        let d = declaration();
        for (marker, lines_before) in [
            ("declaration", 0),
            ("queue ", 1),
            ("on a host", 2),
            ("lundra", 3),
        ] {
            let mut out = FailOn::new(marker);
            let e = hosts(Some(&d), OutputFormat::Text, &mut out).unwrap_err();
            assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe);
            assert_eq!(out.text().matches('\n').count(), lines_before, "{marker}");
        }
    }

    #[test]
    fn every_standing_has_its_words() {
        assert_eq!(standing(Standing::Eligible), "eligible");
        assert_eq!(standing(Standing::Unreachable), "unreachable");
        assert_eq!(
            standing(Standing::UnderMinimum),
            "under its free-space minimum"
        );
        assert_eq!(standing(Standing::UnknownSpace), "free space unknown");
    }

    #[test]
    fn a_probe_names_the_host_a_derive_would_take() {
        let reports = [
            report("down", Standing::Unreachable, false),
            report("blind", Standing::UnknownSpace, false),
            report("roomy", Standing::Eligible, true),
        ];
        let mut out = FailOn::new("\0");
        assert!(probe_report(&reports, OutputFormat::Text, &mut out).unwrap());
        let text = out.text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4, "{text}");
        assert!(
            lines[0].starts_with("  down ")
                && lines[0].contains("free    ? GiB")
                && lines[0].ends_with("unreachable  (down is out)"),
            "{}",
            lines[0]
        );
        assert!(lines[1].ends_with("free space unknown  (blind is out)"));
        assert!(lines[2].starts_with("* roomy ") && lines[2].ends_with("load  1  eligible"));
        assert_eq!(lines[3], "a derive would go to roomy");

        let mut out = FailOn::new("\0");
        assert!(probe_report(&reports, OutputFormat::Json, &mut out).unwrap());
        let v: serde_json::Value = serde_json::from_str(&out.text()).unwrap();
        assert_eq!(v["chosen"], "roomy");
        assert_eq!(v["hosts"][1]["standing"], "unknown_space");

        let none = [report("down", Standing::Unreachable, false)];
        let mut out = FailOn::new("\0");
        assert!(!probe_report(&none, OutputFormat::Text, &mut out).unwrap());
        assert!(out.text().ends_with("no declared host can take a derive\n"));
    }

    #[test]
    fn a_probe_that_cannot_be_written_is_an_error() {
        let reports = [report("roomy", Standing::Eligible, true)];
        let none = [report("down", Standing::Unreachable, false)];
        for (rs, format, marker) in [
            (&reports, OutputFormat::Text, "roomy"),
            (&reports, OutputFormat::Text, "a derive would go to"),
            (&none, OutputFormat::Text, "no declared host"),
            (&reports, OutputFormat::Json, "\"chosen\""),
        ] {
            let mut out = FailOn::new(marker);
            let e = probe_report(rs, format, &mut out).unwrap_err();
            assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{marker}");
        }
    }

    fn derived() -> OffloadReport {
        OffloadReport {
            host: "lundra".into(),
            tag: "0123456789".into(),
            input: "0123456789abcdef".into(),
            derived: Some("fedcba9876543210".into()),
            staged: vec!["generated.txt".into(), "parents.txt".into()],
            waited_seconds: 3,
            seconds: 42,
            probe: Vec::new(),
        }
    }

    #[test]
    fn a_derive_report_says_what_was_staged() {
        let mut out = FailOn::new("\0");
        derive_report(&derived(), OutputFormat::Text, &mut out).unwrap();
        assert_eq!(
            out.text(),
            "derived 0123456789 on lundra in 42 s (3 s in the queue): 2 file(s) staged, \
             nothing committed\n  generated.txt\n  parents.txt\n"
        );
        let mut out = FailOn::new("\0");
        derive_report(&derived(), OutputFormat::Json, &mut out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out.text()).unwrap();
        assert_eq!(v["derived"], "fedcba9876543210");
        assert!(v.get("probe").is_none(), "an empty probe is not printed");

        for (format, marker) in [
            (OutputFormat::Text, "derived "),
            (OutputFormat::Text, "parents.txt"),
            (OutputFormat::Json, "\"staged\""),
        ] {
            let mut out = FailOn::new(marker);
            let e = derive_report(&derived(), format, &mut out).unwrap_err();
            assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "{marker}");
        }
    }
}
