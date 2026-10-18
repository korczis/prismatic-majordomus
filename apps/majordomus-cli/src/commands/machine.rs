//! `majordomus machine`: the build output this repository's builds leave on the machine.
//!
//! Both subcommands run a capability of the `machine` module through the one executor, and
//! render its answer: the sweep as one line per candidate and a summary, the schedule as the
//! scheduler, its files and each command it ran.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{MachineArgs, MachineCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus machine`.
pub fn run(args: MachineArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let (id, input) = request(&args.command);
    let v = ctx.execute(id, input).map_err(protocol)?;
    let text = match args.format {
        OutputFormat::Json => format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default()),
        OutputFormat::Text => match args.command {
            MachineCommand::Sweep { .. } => sweep_text(&v),
            MachineCommand::Schedule { .. } => schedule_text(&v),
        },
    };
    std::io::stdout()
        .lock()
        .write_all(text.as_bytes())
        .map_err(Error::Transport)?;
    Ok(0)
}

/// A capability's refusal as the command's error, its reason kept.
fn protocol(e: CapabilityError) -> Error {
    match e {
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Protocol { reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

/// The capability a subcommand runs, and its input.
fn request(command: &MachineCommand) -> (&'static str, Value) {
    match command {
        MachineCommand::Sweep {
            reclaim,
            idle_minutes,
            floor_gib,
            cargo_home,
        } => {
            let mut input = json!({ "check": !reclaim, "cargo_homes": cargo_home });
            if let Some(m) = idle_minutes {
                input["idle_minutes"] = json!(m);
            }
            if let Some(f) = floor_gib {
                input["floor_gib"] = json!(f);
            }
            ("machine.sweep", input)
        }
        MachineCommand::Schedule {
            install,
            remove,
            floor_gib,
            program,
            no_activate,
        } => {
            let action = if *install {
                "install"
            } else if *remove {
                "remove"
            } else {
                "status"
            };
            let mut input = json!({ "action": action, "activate": !no_activate });
            if let Some(f) = floor_gib {
                input["floor_gib"] = json!(f);
            }
            if let Some(p) = program {
                input["program"] = json!(p);
            }
            ("machine.schedule", input)
        }
    }
}

fn gib(v: &Value) -> String {
    v.as_u64().map_or_else(
        || "?".to_string(),
        |b| format!("{:.1} GiB", b as f64 / f64::from(1u32 << 30)),
    )
}

/// The sweep, one line per candidate, then what it amounted to.
fn sweep_text(v: &Value) -> String {
    let mut out = String::new();
    for i in v["items"].as_array().into_iter().flatten() {
        let idle = i["idle_minutes"]
            .as_u64()
            .map_or_else(|| "?".to_string(), |m| format!("{m}m"));
        let done = if i["removed"].as_bool() == Some(true) {
            " removed"
        } else {
            ""
        };
        out.push_str(&format!(
            "{:<12} {:<20} {:>7} {:>10} {}{done}\n",
            i["verdict"].as_str().unwrap_or("?"),
            i["kind"].as_str().unwrap_or("?"),
            idle,
            gib(&i["bytes"]),
            i["path"].as_str().unwrap_or("?"),
        ));
    }
    let n = v["items"].as_array().map_or(0, Vec::len);
    let mode = if v["check"].as_bool() == Some(false) {
        "reclaimed"
    } else {
        "would reclaim (check)"
    };
    let reclaimable: u64 = v["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|i| i["verdict"] == "reclaimable")
        .filter_map(|i| i["bytes"].as_u64())
        .sum();
    let amount = if v["check"].as_bool() == Some(false) {
        v["reclaimed_bytes"].clone()
    } else {
        json!(reclaimable)
    };
    out.push_str(&format!(
        "{n} candidate(s) under {} home(s); {mode} {}; free {} -> {}{}\n",
        v["homes"].as_array().map_or(0, Vec::len),
        gib(&amount),
        gib(&v["free_before_bytes"]),
        gib(&v["free_after_bytes"]),
        if v["below_floor"].as_bool() == Some(true) {
            " (below the floor: swept at 30 minutes)"
        } else {
            ""
        },
    ));
    out
}

/// The schedule: the scheduler, whether its entry is there, its files and what ran.
fn schedule_text(v: &Value) -> String {
    let mut out = format!(
        "scheduler    {}\ninstalled    {}\nprogram      {}\nevery        {} minutes\n",
        v["scheduler"].as_str().unwrap_or("?"),
        if v["installed"].as_bool() == Some(true) {
            "yes"
        } else {
            "no"
        },
        v["program"].as_str().unwrap_or("?"),
        v["every_minutes"],
    );
    for f in v["files"].as_array().into_iter().flatten() {
        out.push_str(&format!("file         {}\n", f.as_str().unwrap_or("?")));
    }
    for a in v["activation"].as_array().into_iter().flatten() {
        out.push_str(&format!("ran          {}\n", a.as_str().unwrap_or("?")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each subcommand asks its capability with the input its flags say.
    #[test]
    fn each_subcommand_asks_its_capability() {
        let (id, input) = request(&MachineCommand::Sweep {
            reclaim: true,
            idle_minutes: Some(60),
            floor_gib: Some(20),
            cargo_home: vec!["/h".into()],
        });
        assert_eq!(id, "machine.sweep");
        assert_eq!(
            input,
            json!({"check": false, "cargo_homes": ["/h"], "idle_minutes": 60, "floor_gib": 20})
        );
        let (_, input) = request(&MachineCommand::Sweep {
            reclaim: false,
            idle_minutes: None,
            floor_gib: None,
            cargo_home: Vec::new(),
        });
        assert_eq!(input, json!({"check": true, "cargo_homes": []}));
        for (install, remove, action) in [
            (true, false, "install"),
            (false, true, "remove"),
            (false, false, "status"),
        ] {
            let (id, input) = request(&MachineCommand::Schedule {
                install,
                remove,
                floor_gib: Some(25),
                program: Some("/m".into()),
                no_activate: true,
            });
            assert_eq!(id, "machine.schedule");
            assert_eq!(
                input,
                json!({"action": action, "activate": false, "floor_gib": 25, "program": "/m"})
            );
        }
        let (_, input) = request(&MachineCommand::Schedule {
            install: false,
            remove: false,
            floor_gib: None,
            program: None,
            no_activate: false,
        });
        assert_eq!(input, json!({"action": "status", "activate": true}));
    }

    /// The text of a sweep names each candidate and what it came to, checked or reclaimed,
    /// below the floor or not; the text of a schedule names its files and what ran.
    #[test]
    fn the_text_says_what_was_found_and_done() {
        let item = |verdict: &str, removed: bool| {
            json!({"path": "/h/build-private/a", "kind": "private_build_dir", "verdict": verdict,
                   "idle_minutes": 200, "bytes": 1u64 << 30, "removed": removed})
        };
        let checked = sweep_text(
            &json!({"check": true, "homes": ["/h"], "free_before_bytes": 10u64 << 30,
            "free_after_bytes": 10u64 << 30, "below_floor": false, "items": [item("reclaimable", false),
            {"path": "/h/x", "kind": "coverage_target", "verdict": "unmeasurable", "idle_minutes": null,
             "bytes": 0, "removed": false}], "reclaimed_bytes": 0}),
        );
        assert!(
            checked.contains("reclaimable  private_build_dir"),
            "{checked}"
        );
        assert!(
            checked.contains("would reclaim (check) 1.0 GiB"),
            "{checked}"
        );
        assert!(checked.contains("     ? "), "{checked}");
        let swept = sweep_text(
            &json!({"check": false, "homes": ["/h"], "free_before_bytes": null,
            "free_after_bytes": null, "below_floor": true, "items": [item("reclaimable", true)],
            "reclaimed_bytes": 1u64 << 30}),
        );
        assert!(
            swept.contains(" removed\n") && swept.contains("reclaimed 1.0 GiB"),
            "{swept}"
        );
        assert!(
            swept.contains("free ? -> ?") && swept.contains("below the floor"),
            "{swept}"
        );
        let schedule = schedule_text(
            &json!({"scheduler": "launchd", "installed": true, "program": "/m",
            "every_minutes": 15, "files": ["/p.plist"], "activation": ["launchctl load -w /p.plist: ok"]}),
        );
        assert!(
            schedule.contains("installed    yes") && schedule.contains("file         /p.plist")
        );
        assert!(schedule.contains("ran          launchctl load"));
        assert!(schedule_text(&json!({})).contains("installed    no"));
    }

    /// Run whole, in a disposable repository: the sweep of an empty home and the schedule's
    /// status answer in both shapes, and a refused install is the command's error.
    #[test]
    fn the_command_runs_whole() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let empty = tempfile::tempdir().unwrap();
        let repo_args = || crate::cli::RepoArgs {
            repo: Some(repo.root().to_path_buf()),
            discovery: crate::cli::DiscoveryMode::Filesystem,
            strict: false,
            share: Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
        };
        for format in [OutputFormat::Text, OutputFormat::Json] {
            let sweep = MachineCommand::Sweep {
                reclaim: false,
                idle_minutes: None,
                floor_gib: None,
                cargo_home: vec![empty.path().to_path_buf()],
            };
            assert_eq!(
                run(MachineArgs {
                    repo: repo_args(),
                    command: sweep,
                    format
                })
                .unwrap(),
                0
            );
            let status = MachineCommand::Schedule {
                install: false,
                remove: false,
                floor_gib: None,
                program: Some("/opt/majordomus".into()),
                no_activate: true,
            };
            assert_eq!(
                run(MachineArgs {
                    repo: repo_args(),
                    command: status,
                    format
                })
                .unwrap(),
                0
            );
        }
        for (e, said) in [
            (
                CapabilityError::Refused("no scheduler".into()),
                "no scheduler",
            ),
            (CapabilityError::InvalidInput("bad".into()), "bad"),
            (CapabilityError::NotFound("gone".into()), "gone"),
        ] {
            let Error::Protocol { reason } = protocol(e) else {
                panic!("a protocol error")
            };
            assert!(reason.contains(said), "{reason}");
        }
    }
}
