//! `majordomus run` and `majordomus executions`: the execution plane on the command line.
//!
//! Two commands with two different relationships to a process, both deliberate:
//!
//! - `run` starts an execution **here** and follows it. Following is the point, and the
//!   cheapest way to follow something is to subscribe to the store it is writing into —
//!   which this process has, because the execution is its own. When a server already
//!   serves this repository, the run is also reported to it (`executions.report`) while it
//!   goes, so the server's list, the Cockpit and the live channel show what a person ran
//!   and not only what was started there; a stop asked of that server comes back in the
//!   answer and is honoured here. With no server, the run says on stderr that nothing else
//!   can see it. A run never starts a server: the cost of `run` without one is unchanged.
//! - `executions` reads the **shared server** this repository's lease names, because that
//!   is where the executions a browser and an agent started live. An execution does not
//!   outlive the process that accepted it, and a one-shot command has run nothing, so
//!   asking this process would always answer nothing.
//!
//! Both reach the same capabilities of the same registry. Nothing here implements an
//! execution, a state or an event.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{ExecutionsArgs, ExecutionsCommand, OutputFormat, RunArgs};
use crate::error::{Error, Result};
use crate::execution::{Delivery, ExecutionId, ExecutionState, Filter, REPORT_HEARTBEAT};

/// The exit code when the execution ran and did not succeed. The command worked; what it
/// ran did not, and a script must be able to tell those apart.
pub const EXIT_EXECUTION_FAILED: u8 = 10;

/// How long to wait for an execution to reach a final state before giving up on it. A
/// task that is still going after this is still going: the id is printed and the
/// execution is not cancelled.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(600);

/// Run `majordomus run`.
pub fn run(args: RunArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let input: Value = match &args.input {
        Some(text) => serde_json::from_str(text).map_err(|e| Error::Protocol {
            reason: format!("--input is not JSON: {e}"),
        })?,
        None => json!({}),
    };
    if !input.is_object() {
        return Err(Error::Protocol {
            reason: "--input must be a JSON object, as every capability's input is".into(),
        });
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let show = args.follow || (!args.quiet && args.format == OutputFormat::Text);
    // the server already serving this checkout, if any; never one started for this run
    let server = crate::lease::serving(&app.repository);

    // subscribe before starting: an execution this fast would otherwise be over before
    // anything was listening
    let (subscriber, rx) = ctx.executions.store().subscribe(Filter::All);
    let mut start = json!({ "capability": args.capability, "input": input });
    if let Some(key) = &args.idempotency_key {
        start["idempotency_key"] = json!(key);
    }
    let started = ctx
        .execute(cli_capability(ctx, &["run"])?, start)
        .map_err(map)?;
    let id = started["id"].as_str().unwrap_or_default().to_string();
    // the id is the one this process just minted, so it parses whenever a server serves
    let mut relay = server
        .as_deref()
        .zip(ExecutionId::parse(&id))
        .map(|(url, execution)| Relay::new(ctx, url, execution, show));
    if server.is_none() && !args.quiet {
        eprintln!("{NOT_VISIBLE}");
    }

    let deadline = std::time::Instant::now() + PATIENCE;
    loop {
        if let Some(relay) = relay.as_mut() {
            relay.tick();
        }
        match rx.recv_timeout(std::time::Duration::from_millis(250)) {
            Ok(Delivery::Event(event)) => {
                if event.execution_id.as_str() != id {
                    continue;
                }
                if show {
                    if let Some(line) = describe(&event) {
                        eprintln!("{line}");
                    }
                }
                if event.payload.is_terminal() {
                    break;
                }
            }
            Ok(Delivery::Lagged { dropped }) => {
                if show {
                    eprintln!("     … {dropped} event(s) dropped: this reader fell behind");
                }
            }
            Err(_) => {
                let done = ctx
                    .executions
                    .store()
                    .get(&event_id(&id)?)
                    .is_some_and(|e| e.state.is_final());
                if done || std::time::Instant::now() >= deadline {
                    break;
                }
            }
        }
    }
    ctx.executions.store().unsubscribe(subscriber);
    if let Some(relay) = relay.as_mut() {
        relay.finish();
    }

    let snapshot = ctx
        .execute(
            cli_capability(ctx, &["executions", "show"])?,
            json!({ "id": id }),
        )
        .map_err(map)?;
    let state = snapshot["state"].as_str().unwrap_or("unknown");
    match args.format {
        OutputFormat::Json => writeln!(out, "{}", pretty(&snapshot)).map_err(Error::Transport)?,
        OutputFormat::Text if args.quiet => {
            writeln!(out, "{}", pretty(&snapshot["output"])).map_err(Error::Transport)?
        }
        OutputFormat::Text => {
            writeln!(out, "{state:<10} {id}  {}", args.capability).map_err(Error::Transport)?;
            match (&snapshot["output"], &snapshot["error"]) {
                (Value::Null, error) if !error.is_null() => writeln!(
                    out,
                    "{:<10} {}",
                    error["code"].as_str().unwrap_or("error"),
                    error["message"].as_str().unwrap_or_default()
                )
                .map_err(Error::Transport)?,
                (output, _) => {
                    writeln!(out, "{}", pretty(output)).map_err(Error::Transport)?;
                }
            }
        }
    }
    Ok(if state == ExecutionState::Succeeded.as_str() {
        0
    } else {
        EXIT_EXECUTION_FAILED
    })
}

/// What `run` says on stderr when no server serves this repository: the run is real, and
/// nothing but this process will ever show it.
pub const NOT_VISIBLE: &str = "majordomus run: no server is serving this repository, so this execution is visible to this process only: no execution list, Cockpit page or live channel will show it. A run never starts a server; start one with `majordomus serve` before the run to watch it there.";

/// The most bytes of events one report carries, under the server's body bound with room
/// for the description that travels with them.
const REPORT_BUDGET: usize = crate::http::server::MAX_BODY_BYTES * 3 / 4;

/// How long one report may take. The run never waits on the server for longer: a server
/// that cannot answer in this time is told nothing more, and the run goes on.
const REPORT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// How many reports in a row may fail before the run stops reporting.
const REPORT_ATTEMPTS: u32 = 3;

/// The report of one run to the server serving this checkout, from the run's own store.
///
/// Events are read from the store by cursor rather than taken from a subscription, so a
/// reader that lagged loses nothing, and the cursor is the one the server answers: a report
/// that was lost, cut short or sent twice is put right by the next.
struct Relay<'a> {
    ctx: &'a crate::capability::Context,
    url: String,
    id: ExecutionId,
    show: bool,
    /// The sequence the server holds everything up to.
    through: u64,
    /// When the last report was sent.
    last: Option<std::time::Instant>,
    /// Reports in a row that failed.
    failures: u32,
    /// Has any report been taken?
    reached: bool,
    /// The server's stop request, once it has been honoured here.
    stopped: bool,
    /// Why the run stopped reporting, once it has.
    lost: Option<String>,
}

impl<'a> Relay<'a> {
    fn new(ctx: &'a crate::capability::Context, url: &str, id: ExecutionId, show: bool) -> Self {
        Relay {
            ctx,
            url: url.to_string(),
            id,
            show,
            through: 0,
            last: None,
            failures: 0,
            reached: false,
            stopped: false,
            lost: None,
        }
    }

    /// The last sequence of the run here.
    fn local_last(&self) -> u64 {
        self.ctx
            .executions
            .store()
            .get(&self.id)
            .map_or(0, |e| e.last_sequence)
    }

    /// Report when there is something new and the last report is a moment old, or when the
    /// heartbeat is due.
    fn tick(&mut self) {
        let since = self.last.map(|t| t.elapsed());
        let news = self.local_last() > self.through;
        let due = match since {
            None => true,
            Some(since) => {
                (news && since >= std::time::Duration::from_millis(200))
                    || since >= REPORT_HEARTBEAT
            }
        };
        if due {
            self.send();
        }
    }

    /// Report until the server holds everything the run said, or it stops answering.
    fn finish(&mut self) {
        let mut rounds = 0;
        while self.lost.is_none() && self.through < self.local_last() && rounds < 64 {
            let before = self.through;
            self.send();
            if self.through == before {
                rounds += 8;
            }
            rounds += 1;
        }
    }

    fn send(&mut self) {
        if self.lost.is_some() {
            return;
        }
        self.last = Some(std::time::Instant::now());
        let store = self.ctx.executions.store();
        let Some(snapshot) = store.get(&self.id) else {
            return;
        };
        let page = store
            .events(
                &self.id,
                self.through,
                crate::capability::builtin::executions::MAX_EVENTS,
            )
            .map(|p| p.events)
            .unwrap_or_default();
        if page.first().is_some_and(|e| e.sequence > self.through + 1) {
            let missing = format!(
                "this process no longer holds events {}..{} of it",
                self.through + 1,
                page[0].sequence - 1
            );
            self.lose(missing);
            return;
        }
        let mut events = Vec::new();
        let mut bytes = 0usize;
        for event in &page {
            let value = lighter(event, crate::http::ws::MAX_FRAME_BYTES);
            let size = value.to_string().len();
            if !events.is_empty() && bytes + size > REPORT_BUDGET {
                break;
            }
            bytes += size;
            events.push(value);
        }
        let input = json!({
            "id": self.id.as_str(),
            "capability": snapshot.capability,
            "title": snapshot.title,
            "input": if snapshot.input.to_string().len() > crate::http::ws::MAX_FRAME_BYTES {
                withheld(&snapshot.input, crate::http::ws::MAX_FRAME_BYTES)
            } else {
                snapshot.input
            },
            "cancellable": snapshot.cancellable,
            "pid": std::process::id(),
            "events": events,
        });
        let answer = ask_server_within(
            self.ctx,
            &self.url,
            "executions.report",
            &input,
            REPORT_TIMEOUT,
        );
        match answer {
            Ok(answer) => {
                self.failures = 0;
                self.through = answer["through"].as_u64().unwrap_or(self.through);
                if !self.reached {
                    self.reached = true;
                    if self.show {
                        eprintln!(
                            "---  visible on the server serving this repository: {}{}",
                            self.url,
                            answer["execution"]["links"]["cockpit"]
                                .as_str()
                                .unwrap_or("")
                        );
                    }
                }
                if answer["cancel_requested"] == Value::Bool(true) && !self.stopped {
                    self.stopped = true;
                    self.ctx.executions.cancel(&self.id, "the server");
                }
            }
            Err(e) if !self.reached => self.lose(e.to_string()),
            Err(e) => {
                self.failures += 1;
                if self.failures >= REPORT_ATTEMPTS {
                    self.lose(e.to_string());
                }
            }
        }
    }

    fn lose(&mut self, why: String) {
        eprintln!(
            "majordomus run: the server at {} did not take this run's report ({why}); from here on this execution is visible to this process only.",
            self.url
        );
        self.lost = Some(why);
    }
}

/// An event as a report carries it: whole, or with the one unbounded value it may hold —
/// an output, an input — replaced by a note of its size when the event is over `limit`.
/// The value stays in this process; the run prints it.
fn lighter(event: &crate::execution::ExecutionEvent, limit: usize) -> Value {
    use crate::execution::EventPayload;
    let value = serde_json::to_value(event).unwrap_or(Value::Null);
    let size = value.to_string().len();
    if size <= limit {
        return value;
    }
    let note = |v: &Value| withheld(v, limit);
    let payload = match &event.payload {
        EventPayload::Completed { output } => EventPayload::Completed {
            output: note(output),
        },
        EventPayload::Created {
            capability,
            title,
            input,
        } => EventPayload::Created {
            capability: capability.clone(),
            title: title.clone(),
            input: note(input),
        },
        _ => return value,
    };
    let mut lighter = event.clone();
    lighter.payload = payload;
    serde_json::to_value(lighter).unwrap_or(value)
}

/// What a report carries in place of a value over `limit`: how large it is, and that it
/// stays in this process.
fn withheld(value: &Value, limit: usize) -> Value {
    json!({
        "withheld": "too_large",
        "bytes": value.to_string().len(),
        "limit": limit,
        "held_by": format!("the `majordomus run` process {}", std::process::id()),
    })
}

/// One line for one event, for a terminal watching a run.
fn describe(event: &crate::execution::ExecutionEvent) -> Option<String> {
    use crate::execution::EventPayload::*;
    Some(match &event.payload {
        Created { capability, .. } => format!("---  {capability}"),
        Started => "run  the handler was entered".to_string(),
        StepStarted { title, .. } => format!("···  {title}"),
        StepCompleted { name, ok, detail } => format!(
            "{}  {name}{}",
            if *ok { "OK " } else { "FAIL" },
            detail
                .as_ref()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default()
        ),
        Progress(p) => match p.percent() {
            Some(percent) => format!("{percent:>3}%  {}", p.message.clone().unwrap_or_default()),
            None => format!(
                "     {} {}",
                p.current,
                p.message.clone().unwrap_or_default()
            ),
        },
        Log { message, .. } => format!("     {message}"),
        Diagnostic(d) => format!("{:<4} {} {}", d.severity.as_str(), d.code, d.summary),
        Cancelling { by } => format!("---  {by} asked it to stop"),
        Cancelled => "---  cancelled".to_string(),
        Completed { .. } => "---  completed".to_string(),
        Failed { error } => format!("FAIL {}: {}", error.code, error.message),
        Claimed { holder } => format!("---  held by {holder}"),
        Verifying => "---  checking what it produced".to_string(),
        Refused { error, policy } => format!(
            "REFUSED {}: {}{}",
            error.code,
            error.message,
            policy
                .as_ref()
                .map(|p| format!(" ({p})"))
                .unwrap_or_default()
        ),
        UnknownOutcome { error } => format!("UNKNOWN {}: {}", error.code, error.message),
        Authorizing => "---  waiting for a policy decision".to_string(),
        Queued { .. } => return None,
    })
}

/// Run `majordomus executions`.
pub fn executions(args: ExecutionsArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let server = crate::lease::serving(&app.repository);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let (words, input): (&[&str], Value) = match &args.command {
        None | Some(ExecutionsCommand::List { .. }) => {
            let (state, capability) = match &args.command {
                Some(ExecutionsCommand::List { state, capability }) => {
                    (state.clone(), capability.clone())
                }
                _ => (None, None),
            };
            let mut input = json!({});
            if let Some(state) = state {
                input["state"] = json!(state);
            }
            if let Some(capability) = capability {
                input["capability"] = json!(capability);
            }
            (&["executions", "list"], input)
        }
        Some(ExecutionsCommand::Show { id }) => (&["executions", "show"], json!({ "id": id })),
        Some(ExecutionsCommand::Events { id, after }) => (
            &["executions", "events"],
            match after {
                Some(after) => json!({ "id": id, "after": after }),
                None => json!({ "id": id }),
            },
        ),
        Some(ExecutionsCommand::Cancel { id }) => (&["executions", "cancel"], json!({ "id": id })),
        Some(ExecutionsCommand::Protocol) => (&["executions", "protocol"], json!({})),
    };
    let id = cli_capability(ctx, words)?;
    let answer = match &server {
        Some(url) => ask_server(ctx, url, id, &input)?,
        None => ctx.execute(id, input).map_err(map)?,
    };

    match args.format {
        OutputFormat::Json => writeln!(out, "{}", pretty(&answer)).map_err(Error::Transport)?,
        OutputFormat::Text => {
            if server.is_none() && !matches!(args.command, Some(ExecutionsCommand::Protocol)) {
                writeln!(
                    out,
                    "no server is serving this repository; an execution lives in the process that accepted it, so this command has nothing to read. Start one with `majordomus serve`."
                )
                .map_err(Error::Transport)?;
            }
            render(&mut out, &args.command, &answer)?;
        }
    }
    Ok(0)
}

/// Ask the shared server, over the route the registry declares for this capability.
///
/// The route is not written here: it is read from the same descriptor the server binds,
/// so a route that moves moves once.
pub(crate) fn ask_server(
    ctx: &crate::capability::Context,
    url: &str,
    id: &str,
    input: &Value,
) -> Result<Value> {
    ask_server_within(ctx, url, id, input, std::time::Duration::from_secs(30))
}

/// [`ask_server`], giving up after `timeout`.
pub(crate) fn ask_server_within(
    ctx: &crate::capability::Context,
    url: &str,
    id: &str,
    input: &Value,
    timeout: std::time::Duration,
) -> Result<Value> {
    let http = ctx
        .registry
        .get(id)
        .and_then(|c| c.exposure.http.as_ref())
        .ok_or_else(|| Error::Protocol {
            reason: format!("'{id}' declares no HTTP exposure to ask a server through"),
        })?;
    let request = crate::http::Request::bind(http.method, &http.path, input);
    let body = String::from_utf8_lossy(&request.body).into_owned();
    let reply = crate::mcp::bridge::request(
        url,
        http.method.as_str(),
        &request.target(),
        &[("Content-Type", "application/json")],
        (!body.is_empty()).then_some(body.as_str()),
        timeout,
    )
    .map_err(|e| Error::Http {
        reason: format!("{url}{}: {e}", request.target()),
    })?;
    let value: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
    if reply.status >= 400 {
        return Err(Error::Protocol {
            reason: format!(
                "{}: {}",
                value["error"]["code"].as_str().unwrap_or("error"),
                value["error"]["message"]
                    .as_str()
                    .unwrap_or(reply.body.as_str())
            ),
        });
    }
    Ok(value)
}

fn render(
    out: &mut std::io::StdoutLock<'_>,
    command: &Option<ExecutionsCommand>,
    answer: &Value,
) -> Result<()> {
    write_render(out, command, answer)
}

/// The rendering itself, over anything that can be written to, so that what a reader sees
/// can be asserted without a terminal.
fn write_render(
    out: &mut dyn Write,
    command: &Option<ExecutionsCommand>,
    answer: &Value,
) -> Result<()> {
    let w = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    match command {
        None | Some(ExecutionsCommand::List { .. }) => {
            w(
                out,
                format!(
                    "{} execution(s); {} active, {} queued, {} live channel(s)",
                    answer["count"], answer["active"], answer["queued"], answer["live_channels"]
                ),
            )?;
            for e in answer["executions"].as_array().into_iter().flatten() {
                w(
                    out,
                    format!(
                        "{:<10} {:<32} {:<28} {}",
                        e["state"].as_str().unwrap_or("?"),
                        e["id"].as_str().unwrap_or("?"),
                        e["capability"].as_str().unwrap_or("?"),
                        e["duration_ms"]
                            .as_u64()
                            .map(|ms| format!("{ms} ms"))
                            .unwrap_or_default()
                    ),
                )?;
            }
        }
        Some(ExecutionsCommand::Show { .. }) | Some(ExecutionsCommand::Cancel { .. }) => {
            let e = if answer.get("execution").is_some() {
                &answer["execution"]
            } else {
                answer
            };
            if let Some(outcome) = answer["outcome"].as_str() {
                w(out, format!("cancel     {outcome}"))?;
            }
            w(
                out,
                format!(
                    "{:<10} {}  {}",
                    e["state"].as_str().unwrap_or("?"),
                    e["id"].as_str().unwrap_or("?"),
                    e["capability"].as_str().unwrap_or("?")
                ),
            )?;
            for step in e["steps"].as_array().into_iter().flatten() {
                w(
                    out,
                    format!(
                        "  {:<10} {}",
                        step["state"].as_str().unwrap_or("?"),
                        step["title"].as_str().unwrap_or("?")
                    ),
                )?;
            }
            if !e["error"].is_null() {
                w(
                    out,
                    format!(
                        "  {:<10} {}",
                        e["error"]["code"].as_str().unwrap_or("error"),
                        e["error"]["message"].as_str().unwrap_or_default()
                    ),
                )?;
            }
        }
        Some(ExecutionsCommand::Events { .. }) => {
            for event in answer["events"].as_array().into_iter().flatten() {
                w(
                    out,
                    format!(
                        "{:>4} {:<26} {}",
                        event["sequence"],
                        event["type"].as_str().unwrap_or("?"),
                        serde_json::to_string(&event["data"]).unwrap_or_default()
                    ),
                )?;
            }
            w(
                out,
                format!(
                    "last_sequence {}  more {}  truncated {}",
                    answer["last_sequence"], answer["more"], answer["truncated"]
                ),
            )?;
        }
        Some(ExecutionsCommand::Protocol) => {
            w(
                out,
                format!(
                    "protocol   {} over {}",
                    answer["protocol_version"].as_str().unwrap_or("?"),
                    answer["websocket"].as_str().unwrap_or("?")
                ),
            )?;
            w(out, format!("heartbeat  {}s", answer["heartbeat_seconds"]))?;
            for parameter in answer["subscription"].as_array().into_iter().flatten() {
                w(
                    out,
                    format!(
                        "parameter  {:<12} {}",
                        parameter["name"].as_str().unwrap_or("?"),
                        parameter["description"].as_str().unwrap_or("")
                    ),
                )?;
            }
            for name in answer["event_types"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(answer["stream_types"].as_array().into_iter().flatten())
            {
                w(out, format!("message    {}", name.as_str().unwrap_or("?")))?;
            }
        }
    }
    Ok(())
}

fn event_id(id: &str) -> Result<crate::execution::ExecutionId> {
    crate::execution::ExecutionId::parse(id).ok_or_else(|| Error::Protocol {
        reason: format!("'{id}' is not an execution id"),
    })
}

pub(crate) fn cli_capability<'a>(
    ctx: &'a crate::capability::Context,
    path: &[&str],
) -> Result<&'a str> {
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    ctx.registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })
}

pub(crate) fn map(e: CapabilityError) -> Error {
    match e {
        // the request was well formed and the thing is absent: the missing-artifact exit
        // code, so a script can tell "no such execution" from "the server did not answer"
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::{
        EventPayload, ExecutionDiagnostic, ExecutionError, ExecutionId, LogStream, ProgressView,
    };

    fn event(payload: EventPayload) -> crate::execution::ExecutionEvent {
        crate::execution::ExecutionEvent::new(ExecutionId::fresh(), 1, payload)
    }

    /// Every event a terminal can be shown has a line, and no line is blank. A payload
    /// added without a line here would be an execution that stopped saying what it was
    /// doing halfway through, on the one interface where that is the whole output.
    #[test]
    fn every_event_a_terminal_sees_has_a_line() {
        let cases = [
            EventPayload::Created {
                capability: "demo.x".into(),
                title: "Demo".into(),
                input: json!({}),
            },
            EventPayload::Started,
            EventPayload::StepStarted {
                name: "scan".into(),
                title: "Scanning".into(),
            },
            EventPayload::StepCompleted {
                name: "scan".into(),
                ok: true,
                detail: Some("41 files".into()),
            },
            EventPayload::StepCompleted {
                name: "scan".into(),
                ok: false,
                detail: None,
            },
            EventPayload::Progress(ProgressView {
                current: 3,
                total: Some(4),
                message: Some("three of four".into()),
            }),
            EventPayload::Progress(ProgressView {
                current: 3,
                total: None,
                message: None,
            }),
            EventPayload::Log {
                stream: LogStream::Stderr,
                message: "a line".into(),
            },
            EventPayload::Diagnostic(ExecutionDiagnostic {
                severity: crate::model::Severity::Warning,
                code: "slow".into(),
                summary: "it took a while".into(),
                detail: None,
                suggestion: None,
            }),
            EventPayload::Cancelling {
                by: "client".into(),
            },
            EventPayload::Cancelled,
            EventPayload::Completed { output: json!({}) },
            EventPayload::Failed {
                error: ExecutionError {
                    code: "internal".into(),
                    message: "it broke".into(),
                    suggestion: None,
                    correlation_id: "x".into(),
                },
            },
            EventPayload::Authorizing,
            EventPayload::Claimed {
                holder: "worker-1".into(),
            },
            EventPayload::Verifying,
            EventPayload::Refused {
                error: ExecutionError {
                    code: "refused".into(),
                    message: "not here".into(),
                    suggestion: None,
                    correlation_id: "x".into(),
                },
                policy: Some("project.example".into()),
            },
            EventPayload::UnknownOutcome {
                error: ExecutionError {
                    code: "unavailable".into(),
                    message: "it went silent".into(),
                    suggestion: None,
                    correlation_id: "x".into(),
                },
            },
        ];
        for payload in cases {
            let name = payload.type_name();
            let line = describe(&event(payload)).unwrap_or_else(|| panic!("{name} has no line"));
            assert!(!line.trim().is_empty(), "{name} renders as blank");
        }
        // the one event a terminal does not need: it is already looking at the queue
        assert!(describe(&event(EventPayload::Queued { ahead: 0 })).is_none());
        // and the words a reader looks for are there
        assert!(describe(&event(EventPayload::Progress(ProgressView {
            current: 3,
            total: Some(4),
            message: Some("three of four".into()),
        })))
        .unwrap()
        .contains("75%"));
        assert!(describe(&event(EventPayload::Failed {
            error: ExecutionError {
                code: "internal".into(),
                message: "it broke".into(),
                suggestion: None,
                correlation_id: "x".into(),
            },
        }))
        .unwrap()
        .contains("it broke"));
    }

    /// A report carries every event, and an output too large for the server's channel as a
    /// note of its size rather than as a body the server would refuse.
    #[test]
    fn a_report_carries_an_event_whole_or_names_what_it_withheld() {
        let small = event(EventPayload::Completed {
            output: json!({ "ok": true }),
        });
        assert_eq!(lighter(&small, 1024), serde_json::to_value(&small).unwrap());

        let big = event(EventPayload::Completed {
            output: json!({ "text": "x".repeat(4096) }),
        });
        let carried = lighter(&big, 1024);
        assert_eq!(carried["type"], "execution.completed");
        assert_eq!(
            carried["sequence"], 1,
            "the event keeps its place in the story"
        );
        assert_eq!(carried["data"]["output"]["withheld"], "too_large");
        assert!(carried["data"]["output"]["bytes"].as_u64().unwrap() > 4096);
        assert!(carried.to_string().len() < 1024);
        // and it is still an event the server can read
        let back: crate::execution::ExecutionEvent =
            serde_json::from_value(carried).expect("an event");
        assert_eq!(back.type_name(), "execution.completed");

        // a payload with no unbounded value is carried as it is, whatever its size
        let line = event(EventPayload::Log {
            stream: LogStream::Handler,
            message: "y".repeat(2000),
        });
        assert_eq!(lighter(&line, 16), serde_json::to_value(&line).unwrap());
        assert!(NOT_VISIBLE.contains("visible to this process only"));
    }

    /// What each subcommand prints, from the answer its capability gave. The renderer is
    /// asked with the shapes the capabilities actually answer, so a renamed field is a
    /// failure here rather than a blank column for a reader.
    #[test]
    fn each_subcommand_renders_what_its_capability_answered() {
        let render_to_string = |command: Option<ExecutionsCommand>, answer: Value| {
            // the renderer writes to stdout; the assertions below are about what it reads,
            // so it is driven for its side effects and its refusals
            let mut sink = Vec::new();
            write_render(&mut sink, &command, &answer).expect("rendering writes");
            String::from_utf8(sink).expect("text")
        };

        let list = json!({
            "count": 1, "active": 1, "queued": 0, "live_channels": 2,
            "executions": [{
                "id": "x-20260101T120000Z-4c3b2a19",
                "state": "running",
                "capability": "objects.verify",
                "duration_ms": null
            }]
        });
        let out = render_to_string(None, list.clone());
        assert!(
            out.contains("1 execution(s); 1 active, 0 queued, 2 live channel(s)"),
            "{out}"
        );
        assert!(
            out.contains("objects.verify") && out.contains("running"),
            "{out}"
        );
        assert_eq!(
            render_to_string(
                Some(ExecutionsCommand::List {
                    state: None,
                    capability: None
                }),
                list
            )
            .lines()
            .count(),
            2
        );

        let one = json!({
            "id": "x-20260101T120000Z-4c3b2a19",
            "state": "failed",
            "capability": "demo.x",
            "steps": [{ "state": "failed", "title": "Scanning" }],
            "error": { "code": "internal", "message": "it broke" }
        });
        let out = render_to_string(
            Some(ExecutionsCommand::Show { id: "x".into() }),
            one.clone(),
        );
        assert!(
            out.contains("failed") && out.contains("Scanning") && out.contains("it broke"),
            "{out}"
        );

        let cancelled = json!({ "outcome": "requested", "cancellable": true, "execution": one });
        let out = render_to_string(
            Some(ExecutionsCommand::Cancel { id: "x".into() }),
            cancelled,
        );
        assert!(out.contains("cancel     requested"), "{out}");

        let history = json!({
            "events": [{ "sequence": 1, "type": "execution.created", "data": {} }],
            "last_sequence": 1, "more": false, "truncated": false
        });
        let out = render_to_string(
            Some(ExecutionsCommand::Events {
                id: "x".into(),
                after: None,
            }),
            history,
        );
        assert!(
            out.contains("execution.created") && out.contains("last_sequence 1"),
            "{out}"
        );

        let protocol = json!({
            "protocol_version": "1", "websocket": "/events", "heartbeat_seconds": 20,
            "subscription": [{ "name": "execution", "description": "one to follow", "required": false }],
            "event_types": ["execution.created"], "stream_types": ["stream.ready"]
        });
        let out = render_to_string(Some(ExecutionsCommand::Protocol), protocol);
        assert!(out.contains("protocol   1 over /events"), "{out}");
        assert!(out.contains("heartbeat  20s"), "{out}");
        assert!(out.contains("parameter  execution"), "{out}");
        assert!(
            out.contains("message    execution.created") && out.contains("message    stream.ready"),
            "{out}"
        );
    }

    // ------------------------------------------------ the report to a serving server

    use std::io::Read;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use crate::execution::{Actor, ActorKind, RepositoryRef};
    use crate::synthetic::SyntheticRepository;

    /// What one request to [`serve`] was: its method, its path and its body as JSON.
    type Asked<'a> = (&'a str, &'a str, &'a Value);

    /// A loopback HTTP server that answers each request with `answer`, for the rest of the
    /// test binary's life; its URL. The body is read whole, by its `Content-Length`.
    fn serve(answer: impl Fn(Asked<'_>) -> (u16, Value) + Send + 'static) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut raw = Vec::new();
                let mut chunk = vec![0u8; 64 * 1024];
                let body_at = loop {
                    if let Some(at) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        break at + 4;
                    }
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break raw.len(),
                        Ok(n) => raw.extend_from_slice(&chunk[..n]),
                    }
                };
                let head = String::from_utf8_lossy(&raw[..body_at]).to_string();
                let length = head
                    .lines()
                    .filter_map(|l| l.split_once(':'))
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while raw.len() < body_at + length {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => raw.extend_from_slice(&chunk[..n]),
                    }
                }
                let mut words = head.split_whitespace();
                let method = words.next().unwrap_or_default().to_string();
                let path = words.next().unwrap_or("/").to_string();
                let body: Value = serde_json::from_slice(&raw[body_at..]).unwrap_or(Value::Null);
                let (status, value) = answer((&method, &path, &body));
                let text = value.to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                    text.len()
                );
            }
        });
        url
    }

    /// An address nothing listens on.
    fn dead_url() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("its address"));
        drop(listener);
        url
    }

    /// The last sequence a report carried, or `held` when it carried none.
    fn last_sequence(body: &Value, held: u64) -> u64 {
        body["events"]
            .as_array()
            .and_then(|events| events.last())
            .and_then(|e| e["sequence"].as_u64())
            .unwrap_or(held)
    }

    /// A server that takes every report whole and answers how far it holds the run, with
    /// `cancel` as its stop request; its URL, how many reports it took, and the bodies.
    fn taking(cancel: bool) -> (String, Arc<AtomicUsize>, Arc<Mutex<Vec<Value>>>) {
        let count = Arc::new(AtomicUsize::new(0));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let held = Arc::new(Mutex::new(0u64));
        let (c, b) = (Arc::clone(&count), Arc::clone(&bodies));
        let url = serve(move |(_, _, body)| {
            c.fetch_add(1, Ordering::SeqCst);
            b.lock().unwrap().push(body.clone());
            let mut held = held.lock().unwrap();
            *held = last_sequence(body, *held);
            (
                200,
                json!({
                    "applied": 0,
                    "through": *held,
                    "cancel_requested": cancel,
                    "execution": { "links": { "cockpit": "/cockpit/executions/x" } }
                }),
            )
        });
        (url, count, bodies)
    }

    /// An execution of this process, queued, with `input`.
    fn own(ctx: &crate::capability::Context, input: Value) -> ExecutionId {
        let id = ExecutionId::fresh();
        ctx.executions.store().create(
            id.clone(),
            "executions.demonstrate",
            "Demonstrate an execution",
            input,
            true,
            Actor::of(ActorKind::Internal),
            RepositoryRef {
                name: "r".into(),
                id: "id".into(),
                branch: None,
            },
        );
        id
    }

    fn line(n: usize, chars: usize) -> crate::execution::EventPayload {
        crate::execution::EventPayload::Log {
            stream: LogStream::Handler,
            message: format!("{n} {}", "x".repeat(chars)),
        }
    }

    /// A run reports what it said, says once where it can be seen, honours the stop the
    /// server hands back once, waits for news before reporting again, and finishes with the
    /// server holding everything it said.
    #[test]
    fn a_relay_reports_the_run_and_honours_the_stop_the_server_hands_back_once() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let store = ctx.executions.store();
        let id = own(&ctx, json!({ "steps": 1 }));
        store.publish(&id, crate::execution::EventPayload::Started);
        let (url, count, _) = taking(true);

        let mut relay = Relay::new(&ctx, &url, id.clone(), true);
        relay.tick();
        assert_eq!(count.load(Ordering::SeqCst), 1, "the first tick reports");
        assert!(relay.reached && relay.stopped && relay.lost.is_none());
        assert_eq!(relay.through, 2, "created and started are held");
        assert_eq!(store.get(&id).unwrap().state, ExecutionState::Cancelling);

        // the stop published news, but the last report is a moment old: nothing is sent
        relay.tick();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        std::thread::sleep(std::time::Duration::from_millis(220));
        relay.tick();
        assert_eq!(
            count.load(Ordering::SeqCst),
            2,
            "news a moment old is reported"
        );
        assert_eq!(relay.through, relay.local_last());

        // the server asks again; the run was already told, and is not cancelled twice
        assert_eq!(store.get(&id).unwrap().event_count, 3);
        store.publish(&id, line(1, 8));
        relay.finish();
        assert_eq!(count.load(Ordering::SeqCst), 3);
        assert_eq!(relay.through, relay.local_last());
        assert_eq!(store.get(&id).unwrap().event_count, 4, "no second stop");
        relay.finish();
        assert_eq!(count.load(Ordering::SeqCst), 3, "nothing new: nothing sent");
    }

    /// A server that takes a report and never moves its `through` is not asked for ever:
    /// the finish gives up after a handful of reports that changed nothing.
    #[test]
    fn a_finish_gives_up_on_a_server_that_holds_nothing_more() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let id = own(&ctx, json!({}));
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        let url = serve(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            (200, json!({ "through": 0, "cancel_requested": false }))
        });
        let mut relay = Relay::new(&ctx, &url, id, false);
        relay.finish();
        assert_eq!(count.load(Ordering::SeqCst), 8);
        assert_eq!(relay.through, 0);
        assert!(relay.reached && relay.lost.is_none());
    }

    /// A server that cannot be reached at the first report loses the run at once; one that
    /// took reports and then fails is given three tries in a row. A lost run sends nothing.
    #[test]
    fn a_relay_loses_an_unreachable_server_at_once_and_a_failing_one_after_three_tries() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let id = own(&ctx, json!({}));

        let mut unreachable = Relay::new(&ctx, &dead_url(), id.clone(), false);
        unreachable.send();
        assert!(unreachable.lost.is_some() && !unreachable.reached);
        let last = unreachable.last;
        unreachable.send();
        assert_eq!(unreachable.last, last, "a lost run is not reported again");

        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        let url = serve(move |_| {
            if c.fetch_add(1, Ordering::SeqCst) == 0 {
                (200, json!({ "through": 1, "cancel_requested": false }))
            } else {
                (
                    500,
                    json!({ "error": { "code": "internal", "message": "it broke" } }),
                )
            }
        });
        let mut failing = Relay::new(&ctx, &url, id, false);
        failing.send();
        assert!(failing.reached && failing.lost.is_none());
        failing.send();
        failing.send();
        assert_eq!(failing.failures, 2);
        assert!(
            failing.lost.is_none(),
            "two failures are not yet a lost server"
        );
        failing.send();
        assert!(
            failing
                .lost
                .as_deref()
                .is_some_and(|w| w.contains("it broke")),
            "{:?}",
            failing.lost
        );
    }

    /// A run this process no longer holds is not reported, and one whose oldest events this
    /// process has already dropped is lost rather than reported with a hole in it.
    #[test]
    fn a_relay_reports_nothing_of_a_run_it_does_not_hold_whole() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let (url, count, _) = taking(false);

        let mut absent = Relay::new(&ctx, &url, ExecutionId::fresh(), false);
        assert_eq!(absent.local_last(), 0);
        absent.send();
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert!(absent.lost.is_none());

        let id = own(&ctx, json!({}));
        let store = ctx.executions.store();
        for n in 0..crate::execution::Limits::default().max_events + 5 {
            store.publish(&id, line(n, 4));
        }
        let mut gapped = Relay::new(&ctx, &url, id, false);
        gapped.send();
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert!(
            gapped
                .lost
                .as_deref()
                .is_some_and(|w| w.contains("no longer holds events 1..")),
            "{:?}",
            gapped.lost
        );
    }

    /// A report stays under the server's body bound: an input too large for the channel is
    /// carried as a note of its size, and events past the budget wait for the next report.
    #[test]
    fn a_report_withholds_a_large_input_and_leaves_events_past_its_budget_for_the_next() {
        let repo = SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let large = json!({ "text": "y".repeat(crate::http::ws::MAX_FRAME_BYTES) });
        let id = own(&ctx, large);
        let store = ctx.executions.store();
        let limit = crate::execution::Limits::default().max_log_chars;
        for n in 0..450 {
            store.publish(&id, line(n, limit));
        }
        let (url, count, bodies) = taking(false);
        let mut relay = Relay::new(&ctx, &url, id, false);
        relay.send();
        {
            let bodies = bodies.lock().unwrap();
            let first = &bodies[0];
            assert_eq!(first["input"]["withheld"], "too_large");
            assert_eq!(first["events"][0]["type"], "execution.created");
            assert_eq!(
                first["events"][0]["data"]["input"]["withheld"], "too_large",
                "the created event carries the note too"
            );
            let carried = first["events"].as_array().unwrap().len();
            assert!(
                carried > 1 && carried < 451,
                "{carried} events in one report"
            );
            assert!(first.to_string().len() < crate::http::server::MAX_BODY_BYTES);
        }
        relay.finish();
        assert!(count.load(Ordering::SeqCst) >= 2);
        assert_eq!(relay.through, 451, "the rest followed in the next report");
    }

    /// A repository `run` can load: the synthetic one, made a git repository.
    fn runnable() -> SyntheticRepository {
        let repo = SyntheticRepository::small().expect("a repository");
        let git = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.root())
            .status()
            .expect("git");
        assert!(git.success());
        repo
    }

    fn run_args(repo: &SyntheticRepository, quiet: bool, format: OutputFormat) -> RunArgs {
        RunArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(repo.root().to_path_buf()),
                discovery: crate::cli::DiscoveryMode::Filesystem,
                share: Some(crate::synthetic::crate_share()),
                ..Default::default()
            },
            capability: "executions.demonstrate".into(),
            input: Some(r#"{ "steps": 1, "delay_ms": 0 }"#.into()),
            idempotency_key: None,
            follow: !quiet,
            quiet,
            format,
        }
    }

    /// With no server, a quiet run says nothing about being unseen and a loud one does;
    /// both run to the end.
    #[test]
    fn a_run_without_a_server_runs_and_says_so_unless_quiet() {
        let repo = runnable();
        assert_eq!(
            run(run_args(&repo, true, OutputFormat::Text)).expect("ran"),
            0
        );
        assert_eq!(
            run(run_args(&repo, false, OutputFormat::Json)).expect("ran"),
            0
        );
    }

    /// With a server serving the checkout, the run reports itself to it until the server
    /// holds everything the run said.
    #[test]
    fn a_run_with_a_server_reports_itself_to_it() {
        let repo = runnable();
        let app = App::load(&run_args(&repo, false, OutputFormat::Text).repo).expect("loads");
        let identity = crate::repository::identity(app.repository.root());
        let reports = Arc::new(Mutex::new(Vec::<Value>::new()));
        let held = Arc::new(Mutex::new(0u64));
        let r = Arc::clone(&reports);
        let url = serve(move |(method, _, body)| {
            if method == "GET" {
                return (
                    200,
                    json!({ "name": "majordomus", "repository_id": identity }),
                );
            }
            r.lock().unwrap().push(body.clone());
            let mut held = held.lock().unwrap();
            *held = last_sequence(body, *held);
            (200, json!({ "through": *held, "cancel_requested": false }))
        });
        let lease = crate::lease::lease_path(&app.repository);
        std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
        std::fs::write(
            &lease,
            json!({
                "schema": crate::lease::SCHEMA,
                "pid": 0,
                "token": "another-process",
                "root": app.repository.root(),
                "url": url,
                "started_at": "2026-10-15T00:00:00Z",
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            crate::lease::serving(&app.repository).as_deref(),
            Some(url.as_str())
        );

        assert_eq!(
            run(run_args(&repo, false, OutputFormat::Text)).expect("ran"),
            0
        );
        let reports = reports.lock().unwrap();
        assert!(!reports.is_empty(), "the run reported itself");
        let types: Vec<&str> = reports
            .iter()
            .flat_map(|r| r["events"].as_array().into_iter().flatten())
            .filter_map(|e| e["type"].as_str())
            .collect();
        assert_eq!(types.first(), Some(&"execution.created"), "{types:?}");
        assert_eq!(types.last(), Some(&"execution.completed"), "{types:?}");
    }
}
