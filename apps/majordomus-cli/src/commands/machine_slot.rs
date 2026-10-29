//! `majordomus machine slot`: the machine's counting slot for heavy work, through the
//! registry's `machine.slot.*` (I4006).
//!
//! The command renders the capability's answer and chooses the exit code from it; the
//! admission, the reclaim and the refusal are the capability's. Two things are this command's
//! own. A pid left out is the command's parent, the shell that runs it, because that is the
//! process doing the work and the one whose end frees the slot. And while an acquisition
//! waits, a line on stderr says so, with the holders, when it starts and whenever what it
//! waits on changes, at least every thirty seconds: a wait that says nothing reads as a hang,
//! which is how eleven builds sat behind one lock on 2026-10-22.
//!
//! The first word of an acquisition's text answer is its verdict — `admitted`,
//! `already_held` or `refused` — for a shell caller to read; `--format json` is the whole
//! answer.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::{CapabilityError, Context};
use crate::cli::{MachineArgs, MachineCommand, OutputFormat, SlotCommand};
use crate::error::{Error, Result};
use crate::machine_slot::{SlotAnswer, SlotHolder, SlotRelease, SlotStatus, SlotVerdict};

/// The exit code of a refused acquisition: the bound ran out, or the disk is under the floor.
/// The same code every bounded wait of this repository ends with (`lib/machine_lock.sh`,
/// `mj_cargo_bounded`).
pub const EXIT_REFUSED: u8 = 75;
/// The exit code of an input the capability refuses: a pid that does not run, no purpose.
pub const EXIT_INVALID: u8 = 2;
/// A slot directory that could not be found, read or written: not a refusal of the caller,
/// and not a pass either.
pub const EXIT_REFUSED_INTERNAL: u8 = 12;

/// How often a waiting acquisition says so when nothing has changed.
const SAY_EVERY: Duration = Duration::from_secs(30);
/// How long an acquisition may wait before it first says so.
const FIRST_SAY: Duration = Duration::from_secs(2);

/// Run `majordomus machine`.
pub fn run(args: MachineArgs) -> Result<u8> {
    let MachineCommand::Slot(slot) = args.command;
    let app = App::load(&args.repo)?;
    let parent = std::os::unix::process::parent_id();
    answer(
        &app.context,
        slot.command,
        args.format,
        parent,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr(),
    )
}

/// One verb against a context: the answer on `out`, a waiting acquisition's progress on
/// `err`, and the exit code. `parent` is the pid a verb that names none is about.
fn answer(
    ctx: &Context,
    command: SlotCommand,
    format: OutputFormat,
    parent: u32,
    out: &mut impl Write,
    err: &mut (dyn Write + Send),
) -> Result<u8> {
    match command {
        SlotCommand::Status { volume } => {
            let value = execute(ctx, "status", with_volume(json!({}), volume.as_deref()))?;
            let status: SlotStatus = read(&value)?;
            match format {
                OutputFormat::Json => print_json(out, &value)?,
                OutputFormat::Text => print_lines(out, &status_lines(&status))?,
            }
            Ok(0)
        }
        SlotCommand::Acquire {
            purpose,
            pid,
            wait,
            volume,
        } => {
            let pid = pid.unwrap_or(parent);
            let mut input = json!({ "pid": pid, "purpose": purpose });
            if let Some(w) = wait {
                input["wait_seconds"] = json!(w);
            }
            let input = with_volume(input, volume.as_deref());
            let done = AtomicBool::new(false);
            let value = std::thread::scope(|s| {
                s.spawn(|| say_while_waiting(ctx, pid, volume.as_deref(), &done, err));
                let answer = execute(ctx, "acquire", input);
                done.store(true, Ordering::SeqCst);
                answer
            })?;
            let answer: SlotAnswer = read(&value)?;
            match format {
                OutputFormat::Json => print_json(out, &value)?,
                OutputFormat::Text => print_lines(out, &answer_lines(pid, &answer))?,
            }
            Ok(match answer.verdict {
                SlotVerdict::Admitted | SlotVerdict::AlreadyHeld => 0,
                SlotVerdict::Refused => EXIT_REFUSED,
            })
        }
        SlotCommand::Release { pid } => {
            let pid = pid.unwrap_or(parent);
            let value = execute(ctx, "release", json!({ "pid": pid }))?;
            let answer: SlotRelease = read(&value)?;
            let line = if answer.released {
                format!("released: pid {pid} holds no machine slot now")
            } else {
                format!("not held: pid {pid} held no machine slot")
            };
            match format {
                OutputFormat::Json => print_json(out, &value)?,
                OutputFormat::Text => print_lines(out, &[line])?,
            }
            Ok(0)
        }
    }
}

/// The input with the volume the work writes to, when one is named.
fn with_volume(mut input: Value, volume: Option<&Path>) -> Value {
    if let Some(v) = volume {
        input["volume"] = json!(v.display().to_string());
    }
    input
}

/// Run `machine slot <verb>` through the capability the registry exposes under those words.
fn execute(ctx: &Context, verb: &str, input: Value) -> Result<Value> {
    let words: Vec<String> = ["machine", "slot", verb]
        .iter()
        .map(|w| w.to_string())
        .collect();
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
    ctx.execute(&id, input).map_err(|e| match e {
        CapabilityError::InvalidInput(reason) => Error::Refused {
            code: EXIT_INVALID,
            reason: format!("majordomus machine slot {verb}: {reason}"),
        },
        other => Error::Refused {
            code: EXIT_REFUSED_INTERNAL,
            reason: format!("majordomus machine slot {verb}: {other}"),
        },
    })
}

fn read<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T> {
    serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
        reason: format!("machine.slot answered with something this command cannot read: {e}"),
    })
}

fn print_json(out: &mut impl Write, value: &Value) -> Result<()> {
    writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    )
    .map_err(Error::Transport)
}

fn print_lines(out: &mut impl Write, lines: &[String]) -> Result<()> {
    lines
        .iter()
        .try_for_each(|line| writeln!(out, "{line}"))
        .map_err(Error::Transport)
}

fn age(seconds: u64) -> String {
    format!("{}m{:02}s", seconds / 60, seconds % 60)
}

fn holders_clause(holders: &[SlotHolder]) -> String {
    holders
        .iter()
        .map(|h| format!("pid {} ({}, {})", h.pid, h.purpose, age(h.age_seconds)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// What the acquisition of `pid` waits on, with the holders, and how long it has waited;
/// `None` when the status cannot be read or `pid` is not among the waiters.
fn waiting_now(ctx: &Context, pid: u32, volume: Option<&Path>) -> Option<(String, u64)> {
    let input = with_volume(json!({}), volume);
    let value = ctx.execute("machine.slot.status", input).ok()?;
    let status = serde_json::from_value::<SlotStatus>(value).ok()?;
    let me = status.waiters.iter().find(|w| w.pid == pid)?;
    let what = format!(
        "{}; holders: {}",
        me.waiting_on,
        holders_clause(&status.holders)
    );
    Some((what, me.waiting_seconds))
}

/// While the acquisition runs: one line on `err` once it has waited [`FIRST_SAY`], again
/// whenever the holders change, and at least every [`SAY_EVERY`].
fn say_while_waiting(
    ctx: &Context,
    pid: u32,
    volume: Option<&Path>,
    done: &AtomicBool,
    err: &mut (dyn Write + Send),
) {
    let start = Instant::now();
    let mut said: Option<(String, Instant)> = None;
    while !done.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
        let looking = start.elapsed() >= FIRST_SAY && !done.load(Ordering::SeqCst);
        let due = looking
            .then(|| waiting_now(ctx, pid, volume))
            .flatten()
            .filter(|(what, _)| match &said {
                None => true,
                Some((last, at)) => last != what || at.elapsed() >= SAY_EVERY,
            });
        if let Some((what, waited)) = due.filter(|_| !done.load(Ordering::SeqCst)) {
            let _ = writeln!(
                err,
                "majordomus: waiting for a machine slot ({waited}s): {what}"
            );
            said = Some((what, Instant::now()));
        }
    }
}

fn status_lines(s: &SlotStatus) -> Vec<String> {
    let mut lines = vec![
        format!("machine slots: {}", s.summary),
        format!(
            "  limits: {} slot(s), floor {} — rule {}, declared at {}",
            s.limits.slots.map_or("no limit".into(), |n| n.to_string()),
            s.limits
                .min_free_gib
                .map_or("none".into(), |g| format!("{g} GiB")),
            s.limits.rule,
            s.limits.declared_in
        ),
        format!("  directory: {}", s.directory),
    ];
    lines.extend(s.holders.iter().map(|h| {
        format!(
            "  holder  pid {:<7} {:>9}  {:<10} {}",
            h.pid,
            age(h.age_seconds),
            format!("{:?}", h.liveness).to_lowercase(),
            h.purpose
        )
    }));
    lines.extend(s.waiters.iter().map(|q| {
        format!(
            "  waiter  pid {:<7} {:>9}  {}",
            q.pid,
            age(q.waiting_seconds),
            q.purpose
        )
    }));
    lines
}

fn answer_lines(pid: u32, a: &SlotAnswer) -> Vec<String> {
    let mut lines = match a.verdict {
        SlotVerdict::Admitted => vec![format!(
            "admitted: pid {pid} holds a machine slot after {}s ({})",
            a.waited_seconds, a.status.summary
        )],
        SlotVerdict::AlreadyHeld => vec![format!(
            "already_held: pid {pid} holds a machine slot already ({})",
            a.status.summary
        )],
        SlotVerdict::Refused => a
            .refusal
            .iter()
            .flat_map(|r| {
                let reason = serde_json::to_value(r.reason)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                [
                    format!("refused ({reason}): {}", r.detail),
                    format!("  instead: {}", r.remedy),
                ]
            })
            .collect(),
    };
    lines.extend(a.reclaimed.iter().map(|g| {
        format!(
            "  reclaimed: pid {} ({}) held a slot for {} and its process is gone",
            g.pid,
            g.purpose,
            age(g.age_seconds)
        )
    }));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::builtin::machine_slot::module;
    use crate::capability::CapabilityRegistry;
    use crate::synthetic::SyntheticRepository;
    use std::path::PathBuf;
    use std::process::{Child, Command};
    use std::sync::Arc;

    /// A context over a synthetic repository whose policy is `policy`, and the slot directory
    /// its handlers resolve: the repository's git directory is kept in a directory of the
    /// repository's own, so the slot directory beside it is this test's alone. `None` when the
    /// environment points every test at one directory, where nothing here can be proven.
    fn fixture(policy: &str) -> Option<(SyntheticRepository, Context, PathBuf)> {
        let named = std::env::var_os(crate::machine_slot::DIRECTORY_ENV);
        let None = named else { return None };
        let repo = SyntheticRepository::small().unwrap();
        std::fs::write(repo.root().join(".ai/repo/policy.yaml"), policy).unwrap();
        let git_dir = repo.root().join(".home/primary/.git");
        std::fs::create_dir_all(git_dir.parent().unwrap()).unwrap();
        assert!(Command::new("git")
            .arg("init")
            .arg("-q")
            .arg("--separate-git-dir")
            .arg(&git_dir)
            .arg(repo.root())
            .status()
            .expect("git")
            .success());
        let dir = crate::machine_slot::directory(repo.root()).expect("a git repository");
        let ctx = context(&repo, vec![module()]);
        Some((repo, ctx, dir))
    }

    fn context(
        repo: &SyntheticRepository,
        modules: Vec<crate::capability::ModuleDescriptor>,
    ) -> Context {
        let index = repo.index().unwrap();
        let registry = CapabilityRegistry::builder()
            .with_modules(modules)
            .with_index(&index)
            .build()
            .expect("the registry builds");
        Context::new(Arc::new(index), Arc::new(registry))
    }

    /// A process of this machine that runs until it is killed.
    fn sleeper() -> Child {
        Command::new("sleep")
            .arg("300")
            .spawn()
            .expect("sleep runs")
    }

    fn stop(mut child: Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    /// The command's answer as text, its stderr as text, and its exit code.
    fn ask(
        ctx: &Context,
        command: SlotCommand,
        format: OutputFormat,
        parent: u32,
    ) -> (String, String, u8) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = answer(ctx, command, format, parent, &mut out, &mut err).unwrap();
        let text = |b: Vec<u8>| String::from_utf8(b).unwrap();
        (text(out), text(err), code)
    }

    fn acquire(purpose: &str, pid: Option<u32>, wait: Option<u64>) -> SlotCommand {
        SlotCommand::Acquire {
            purpose: purpose.into(),
            pid,
            wait,
            volume: None,
        }
    }

    const ONE_SLOT: &str = "version: 1\nmachine:\n  heavy_work:\n    slots: 1\n";

    #[test]
    fn the_text_answers_say_admitted_already_held_refused_and_released_with_their_codes() {
        let Some((_repo, ctx, dir)) = fixture(ONE_SLOT) else {
            return;
        };
        let (a, b) = (sleeper(), sleeper());
        // a pid left out is the parent's
        let (out, err, code) = ask(
            &ctx,
            acquire("build a", None, None),
            OutputFormat::Text,
            a.id(),
        );
        assert_eq!(code, 0);
        assert!(
            out.starts_with(&format!(
                "admitted: pid {} holds a machine slot after 0s (",
                a.id()
            )),
            "{out}"
        );
        assert_eq!(err, "", "an admission at once says nothing while waiting");
        let (out, _, code) = ask(
            &ctx,
            acquire("build a", Some(a.id()), None),
            OutputFormat::Text,
            1,
        );
        assert_eq!(code, 0, "a holder asking again is told it holds one");
        assert!(
            out.starts_with(&format!("already_held: pid {} holds", a.id())),
            "{out}"
        );

        // the one slot is held: a second process waits its bound of none and is refused
        let (out, _, code) = ask(
            &ctx,
            acquire("build b", Some(b.id()), Some(0)),
            OutputFormat::Text,
            1,
        );
        assert_eq!(code, EXIT_REFUSED);
        let lines: Vec<&str> = out.lines().collect();
        assert!(
            lines[0].starts_with("refused (slots_full): 1 of 1 machine slots"),
            "{out}"
        );
        assert!(
            lines[0].contains(&format!("pid {} (build a", a.id())),
            "{out}"
        );
        assert!(
            lines[1].starts_with("  instead: wait for a holder"),
            "{out}"
        );

        // the status names the limits, where they are declared, the directory and the holder
        let status = SlotCommand::Status {
            volume: Some(dir.clone()),
        };
        let (out, _, code) = ask(&ctx, status, OutputFormat::Text, 1);
        assert_eq!(code, 0);
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("machine slots: "), "{out}");
        assert!(
            lines[1].starts_with("  limits: 1 slot(s), floor none — rule "),
            "{out}"
        );
        assert!(lines[1].contains("declared at "), "{out}");
        assert_eq!(lines[2], format!("  directory: {}", dir.display()));
        assert!(
            lines[3].starts_with(&format!("  holder  pid {:<7}", a.id())),
            "{out}"
        );
        assert!(
            lines[3].contains(" alive ") && lines[3].ends_with(" build a"),
            "{out}"
        );

        // a release names what it gave back, and a second one that there was nothing
        let (out, _, code) = ask(
            &ctx,
            SlotCommand::Release { pid: None },
            OutputFormat::Text,
            a.id(),
        );
        assert_eq!(code, 0);
        assert_eq!(
            out,
            format!("released: pid {} holds no machine slot now\n", a.id())
        );
        let (out, _, code) = ask(
            &ctx,
            SlotCommand::Release { pid: Some(a.id()) },
            OutputFormat::Text,
            1,
        );
        assert_eq!(code, 0, "releasing nothing is not a failure");
        assert_eq!(
            out,
            format!("not held: pid {} held no machine slot\n", a.id())
        );
        stop(a);
        stop(b);
    }

    #[test]
    fn a_holder_whose_process_is_gone_is_named_as_reclaimed_by_the_next_admission() {
        let Some((_repo, ctx, _dir)) = fixture(ONE_SLOT) else {
            return;
        };
        let (a, b) = (sleeper(), sleeper());
        let gone = a.id();
        ask(
            &ctx,
            acquire("gone soon", Some(gone), None),
            OutputFormat::Text,
            1,
        );
        stop(a);
        let (out, _, code) = ask(
            &ctx,
            acquire("next", Some(b.id()), None),
            OutputFormat::Text,
            1,
        );
        assert_eq!(code, 0);
        let lines: Vec<&str> = out.lines().collect();
        assert!(
            lines[0].starts_with(&format!("admitted: pid {}", b.id())),
            "{out}"
        );
        assert!(
            lines[1].starts_with(&format!(
                "  reclaimed: pid {gone} (gone soon) held a slot for 0m"
            )),
            "{out}"
        );
        assert!(lines[1].ends_with("s and its process is gone"), "{out}");
        stop(b);
    }

    #[test]
    fn the_json_answers_are_the_capabilitys_whole_answer() {
        let Some((_repo, ctx, dir)) = fixture(ONE_SLOT) else {
            return;
        };
        let a = sleeper();
        let json = |out: &str| serde_json::from_str::<Value>(out).expect("one JSON document");
        let take = SlotCommand::Acquire {
            purpose: "a build".into(),
            pid: Some(a.id()),
            wait: Some(0),
            volume: Some(dir.clone()),
        };
        let (out, _, code) = ask(&ctx, take, OutputFormat::Json, 1);
        assert_eq!(code, 0);
        assert_eq!(json(&out)["verdict"], "admitted");
        let (out, _, _) = ask(
            &ctx,
            SlotCommand::Status { volume: None },
            OutputFormat::Json,
            1,
        );
        assert_eq!(json(&out)["held"], 1);
        assert_eq!(json(&out)["holders"][0]["pid"], a.id());
        let (out, _, _) = ask(
            &ctx,
            SlotCommand::Release { pid: Some(a.id()) },
            OutputFormat::Json,
            1,
        );
        assert_eq!(json(&out)["released"], true);
        stop(a);
    }

    #[test]
    fn a_waiting_acquisition_says_what_it_waits_on_and_is_listed_until_it_is_admitted() {
        let Some((_repo, ctx, dir)) = fixture(ONE_SLOT) else {
            return;
        };
        let (a, b) = (sleeper(), sleeper());
        ask(
            &ctx,
            acquire("first", Some(a.id()), None),
            OutputFormat::Text,
            1,
        );
        let (out, err, code, listed) = std::thread::scope(|s| {
            let waiting = s.spawn(|| {
                let (mut out, mut err) = (Vec::new(), Vec::new());
                let wait = acquire("second", Some(b.id()), Some(30));
                let code = answer(&ctx, wait, OutputFormat::Text, 1, &mut out, &mut err).unwrap();
                (
                    String::from_utf8(out).unwrap(),
                    String::from_utf8(err).unwrap(),
                    code,
                )
            });
            // past the first report, and one look after it, the waiter is in the status
            std::thread::sleep(Duration::from_millis(3000));
            let (listed, _, _) = ask(
                &ctx,
                SlotCommand::Status { volume: None },
                OutputFormat::Text,
                1,
            );
            crate::machine_slot::release(&dir, a.id()).unwrap();
            let (out, err, code) = waiting.join().unwrap();
            (out, err, code, listed)
        });
        assert!(
            listed.contains(&format!("  waiter  pid {:<7}", b.id())) && listed.contains("second"),
            "{listed}"
        );
        assert_eq!(code, 0);
        assert!(
            out.starts_with(&format!("admitted: pid {} holds", b.id())),
            "{out}"
        );
        // the holders' ages are part of what it waits on, so a report may repeat as they grow
        let said: Vec<&str> = err.lines().collect();
        assert!(!said.is_empty(), "a wait past the first report says so");
        assert!(
            said[0].starts_with("majordomus: waiting for a machine slot ("),
            "{err}"
        );
        assert!(
            said[0].contains(&format!(
                "machine.slot: 1 of 1 held; holders: pid {} (first, ",
                a.id()
            )),
            "{err}"
        );
        stop(a);
        stop(b);
    }

    #[test]
    fn a_refused_input_an_unusable_directory_and_a_missing_capability_are_errors_with_codes() {
        let Some((repo, ctx, _dir)) = fixture(ONE_SLOT) else {
            return;
        };
        let empty = acquire("  ", Some(std::process::id()), None);
        let e = answer(
            &ctx,
            empty,
            OutputFormat::Text,
            1,
            &mut Vec::<u8>::new(),
            &mut Vec::<u8>::new(),
        );
        assert!(
            matches!(
                e,
                Err(Error::Refused {
                    code: EXIT_INVALID,
                    ..
                })
            ),
            "{e:?}"
        );

        // a repository outside git has no slot directory: refused, but not as the caller's fault
        let outside = SyntheticRepository::small().unwrap();
        let ctx = context(&outside, vec![module()]);
        let e = answer(
            &ctx,
            SlotCommand::Status { volume: None },
            OutputFormat::Text,
            1,
            &mut Vec::<u8>::new(),
            &mut Vec::<u8>::new(),
        );
        match e {
            Err(Error::Refused { code, reason }) => {
                assert_eq!(code, EXIT_REFUSED_INTERNAL);
                assert!(
                    reason.starts_with("majordomus machine slot status: "),
                    "{reason}"
                );
            }
            other => panic!("{other:?}"),
        }

        // a registry without the module exposes nothing under the command's words
        let ctx = context(&repo, Vec::new());
        match execute(&ctx, "status", json!({})) {
            Err(Error::Protocol { reason }) => assert_eq!(
                reason,
                "no capability is exposed as `majordomus machine slot status`"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_answer_of_another_shape_is_a_protocol_error_naming_it() {
        match read::<SlotStatus>(&json!({ "held": "many" })) {
            Err(Error::Protocol { reason }) => assert!(
                reason.starts_with("machine.slot answered with something this command cannot read"),
                "{reason}"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_age_is_minutes_and_two_digit_seconds() {
        assert_eq!(age(0), "0m00s");
        assert_eq!(age(61), "1m01s");
        assert_eq!(age(3600), "60m00s");
    }
}
