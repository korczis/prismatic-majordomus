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
    let started = ctx
        .execute(
            cli_capability(ctx, &["run"])?,
            json!({ "capability": args.capability, "input": input }),
        )
        .map_err(map)?;
    let id = started["id"].as_str().unwrap_or_default().to_string();
    let mut relay = match &server {
        Some(url) => Some(Relay::new(ctx, url, event_id(&id)?, show)),
        None => {
            if !args.quiet {
                eprintln!("{NOT_VISIBLE}");
            }
            None
        }
    };

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
}
