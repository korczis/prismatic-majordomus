//! `majordomus run` and `majordomus executions`: the execution plane on the command line.
//!
//! Two commands with two different relationships to a process, both deliberate:
//!
//! - `run` starts an execution **here** and follows it. Following is the point, and the
//!   cheapest way to follow something is to subscribe to the store it is writing into —
//!   which this process has, because the execution is its own. Nothing is asked
//!   repeatedly.
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
use crate::execution::{Delivery, ExecutionState, Filter};

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

    let deadline = std::time::Instant::now() + PATIENCE;
    loop {
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
fn ask_server(
    ctx: &crate::capability::Context,
    url: &str,
    id: &str,
    input: &Value,
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
        std::time::Duration::from_secs(30),
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
    let w = |out: &mut std::io::StdoutLock<'_>, s: String| {
        writeln!(out, "{s}").map_err(Error::Transport)
    };
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

fn cli_capability<'a>(ctx: &'a crate::capability::Context, path: &[&str]) -> Result<&'a str> {
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

fn map(e: CapabilityError) -> Error {
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
