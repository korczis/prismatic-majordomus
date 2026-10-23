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
    let ctx = &app.context;
    let parent = std::os::unix::process::parent_id();
    let mut out = std::io::stdout().lock();
    match slot.command {
        SlotCommand::Status { volume } => {
            let mut input = json!({});
            if let Some(v) = volume {
                input["volume"] = json!(v.display().to_string());
            }
            let value = execute(ctx, "status", input)?;
            let status: SlotStatus = read(&value)?;
            match args.format {
                OutputFormat::Json => print_json(&mut out, &value)?,
                OutputFormat::Text => render_status(&mut out, &status)?,
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
            if let Some(v) = &volume {
                input["volume"] = json!(v.display().to_string());
            }
            let done = AtomicBool::new(false);
            let value = std::thread::scope(|s| {
                s.spawn(|| say_while_waiting(ctx, pid, volume.as_deref(), &done));
                let answer = execute(ctx, "acquire", input);
                done.store(true, Ordering::SeqCst);
                answer
            })?;
            let answer: SlotAnswer = read(&value)?;
            match args.format {
                OutputFormat::Json => print_json(&mut out, &value)?,
                OutputFormat::Text => render_answer(&mut out, pid, &answer)?,
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
            match args.format {
                OutputFormat::Json => print_json(&mut out, &value)?,
                OutputFormat::Text if answer.released => {
                    writeln!(out, "released: pid {pid} holds no machine slot now")
                        .map_err(Error::Transport)?
                }
                OutputFormat::Text => writeln!(out, "not held: pid {pid} held no machine slot")
                    .map_err(Error::Transport)?,
            }
            Ok(0)
        }
    }
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

/// While the acquisition runs: one line on stderr once it has waited [`FIRST_SAY`], again
/// whenever the holders change, and at least every [`SAY_EVERY`].
fn say_while_waiting(ctx: &Context, pid: u32, volume: Option<&std::path::Path>, done: &AtomicBool) {
    let start = Instant::now();
    let mut said: Option<(String, Instant)> = None;
    while !done.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
        if start.elapsed() < FIRST_SAY || done.load(Ordering::SeqCst) {
            continue;
        }
        let mut input = json!({});
        if let Some(v) = volume {
            input["volume"] = json!(v.display().to_string());
        }
        let Ok(value) = ctx.execute("machine.slot.status", input) else {
            continue;
        };
        let Ok(status) = serde_json::from_value::<SlotStatus>(value) else {
            continue;
        };
        let Some(me) = status.waiters.iter().find(|w| w.pid == pid) else {
            continue;
        };
        let what = format!(
            "{}; holders: {}",
            me.waiting_on,
            holders_clause(&status.holders)
        );
        let due = match &said {
            None => true,
            Some((last, at)) => *last != what || at.elapsed() >= SAY_EVERY,
        };
        if due && !done.load(Ordering::SeqCst) {
            eprintln!(
                "majordomus: waiting for a machine slot ({}s): {what}",
                me.waiting_seconds
            );
            said = Some((what, Instant::now()));
        }
    }
}

fn render_status(out: &mut impl Write, s: &SlotStatus) -> Result<()> {
    let w = |out: &mut dyn Write, line: String| writeln!(out, "{line}").map_err(Error::Transport);
    w(out, format!("machine slots: {}", s.summary))?;
    w(
        out,
        format!(
            "  limits: {} slot(s), floor {} — rule {}, declared at {}",
            s.limits.slots.map_or("no limit".into(), |n| n.to_string()),
            s.limits
                .min_free_gib
                .map_or("none".into(), |g| format!("{g} GiB")),
            s.limits.rule,
            s.limits.declared_in
        ),
    )?;
    w(out, format!("  directory: {}", s.directory))?;
    for h in &s.holders {
        w(
            out,
            format!(
                "  holder  pid {:<7} {:>9}  {:<10} {}",
                h.pid,
                age(h.age_seconds),
                format!("{:?}", h.liveness).to_lowercase(),
                h.purpose
            ),
        )?;
    }
    for q in &s.waiters {
        w(
            out,
            format!(
                "  waiter  pid {:<7} {:>9}  {}",
                q.pid,
                age(q.waiting_seconds),
                q.purpose
            ),
        )?;
    }
    Ok(())
}

fn render_answer(out: &mut impl Write, pid: u32, a: &SlotAnswer) -> Result<()> {
    let w = |out: &mut dyn Write, line: String| writeln!(out, "{line}").map_err(Error::Transport);
    match a.verdict {
        SlotVerdict::Admitted => w(
            out,
            format!(
                "admitted: pid {pid} holds a machine slot after {}s ({})",
                a.waited_seconds, a.status.summary
            ),
        )?,
        SlotVerdict::AlreadyHeld => w(
            out,
            format!(
                "already_held: pid {pid} holds a machine slot already ({})",
                a.status.summary
            ),
        )?,
        SlotVerdict::Refused => {
            if let Some(r) = &a.refusal {
                let reason = serde_json::to_value(r.reason)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                w(out, format!("refused ({reason}): {}", r.detail))?;
                w(out, format!("  instead: {}", r.remedy))?;
            }
        }
    }
    for g in &a.reclaimed {
        w(
            out,
            format!(
                "  reclaimed: pid {} ({}) held a slot for {} and its process is gone",
                g.pid,
                g.purpose,
                age(g.age_seconds)
            ),
        )?;
    }
    Ok(())
}
