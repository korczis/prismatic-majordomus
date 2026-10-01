//! `majordomus reasoning`: the terminal rendering of the reasoning capabilities. Every
//! subcommand executes the capability its CLI path names, in-process, and owns the
//! rendering and nothing else; `--format json` prints the answer the HTTP route and the
//! MCP tool serve.

use std::io::Read;

use serde_json::{json, Value};

use crate::app::App;
use crate::cli::{OutputFormat, ReasoningArgs, ReasoningCommand};
use crate::error::{Error, Result};

/// Run `majordomus reasoning`.
pub fn run(args: ReasoningArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let execute = |words: &[&str], input: Value| -> Result<Value> {
        let path: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        let id = ctx
            .registry
            .by_cli(&path)
            .map(|c| c.id.as_str())
            .ok_or_else(|| Error::Protocol {
                reason: format!(
                    "no capability is exposed as `majordomus {}`",
                    words.join(" ")
                ),
            })?;
        ctx.execute(id, input).map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })
    };
    let json_out = matches!(args.format, OutputFormat::Json);
    let print = |value: &Value, text: String| {
        if json_out {
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_default()
            );
        } else {
            print!("{text}");
        }
    };
    match args.command {
        ReasoningCommand::Advisors => {
            let v = execute(&["reasoning", "advisors"], json!({}))?;
            print(&v, advisors_text(&v));
        }
        ReasoningCommand::Plan {
            materiality,
            confidence,
            capabilities,
        } => {
            let v = execute(
                &["reasoning", "plan"],
                json!({ "materiality": materiality, "confidence": confidence, "capabilities": capabilities }),
            )?;
            print(&v, plan_text(&v));
        }
        ReasoningCommand::Record { file } => {
            let text = match file {
                Some(path) => std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?,
                None => {
                    let mut s = String::new();
                    std::io::stdin()
                        .read_to_string(&mut s)
                        .map_err(|e| Error::Protocol {
                            reason: format!("standard input: {e}"),
                        })?;
                    s
                }
            };
            let record: Value = serde_json::from_str(&text).map_err(|e| Error::Protocol {
                reason: format!("the record is not JSON: {e}"),
            })?;
            let v = execute(&["reasoning", "record"], json!({ "record": record }))?;
            print(&v, step_text(&v));
        }
        ReasoningCommand::Status { task, report } => {
            let v = execute(&["reasoning", "status"], json!({ "task": task }))?;
            if report && !json_out {
                print!("{}", v["report"].as_str().unwrap_or_default());
            } else {
                print(&v, status_text(&v));
            }
        }
        ReasoningCommand::Explain { id } => {
            let v = execute(&["reasoning", "explain"], json!({ "id": id }))?;
            print(&v, explain_text(&v));
        }
        ReasoningCommand::Check => {
            let v = execute(&["reasoning", "check"], json!({}))?;
            print(&v, check_text(&v));
            if v["ok"] != Value::Bool(true) {
                return Ok(10);
            }
        }
    }
    Ok(0)
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

fn list(v: &Value) -> Vec<&str> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn advisors_text(v: &Value) -> String {
    let mut out = format!(
        "reasoning   operational · mode {} ({})\nadvisors    {} available · {} optional unavailable\n",
        s(&v["mode"]["mode"]),
        s(&v["mode"]["source"]),
        v["available"],
        v["unavailable"]
    );
    for a in v["advisors"].as_array().into_iter().flatten() {
        let mut line = format!(
            "  {:<14} {:<18} {:<13} {}",
            s(&a["id"]),
            s(&a["status"]),
            s(&a["transport"]),
            s(&a["reason"])
        );
        if a["recovering"] == Value::Bool(true) {
            line.push_str(" (recovering)");
        }
        if let Some(until) = a["circuit"]["until"].as_str() {
            line.push_str(&format!(" until {until}"));
        }
        out.push_str(&format!("{line}\n"));
    }
    out.push_str("capacity\n");
    for c in v["capacity"].as_array().into_iter().flatten() {
        let who = list(&c["available"]);
        out.push_str(&format!(
            "  {:<22} {}\n",
            s(&c["capability"]),
            if who.is_empty() {
                "local review".to_string()
            } else {
                who.join(", ")
            }
        ));
    }
    for d in list(&v["diagnostics"]) {
        out.push_str(&format!("diagnostic  {d}\n"));
    }
    out
}

fn plan_text(v: &Value) -> String {
    let mut out = format!(
        "outcome     {}\nreason      {}\nmode        {} ({}) · budget {} · available {}\n",
        s(&v["outcome"]),
        s(&v["reason"]),
        s(&v["mode"]),
        s(&v["mode_source"]),
        v["budget"],
        v["available"]
    );
    for a in v["selected"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "consult     {} — covers {} — {}\n",
            s(&a["advisor"]),
            list(&a["covers"]).join(", "),
            s(&a["why"])
        ));
    }
    for e in v["excluded"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "excluded    {} — {}\n",
            s(&e["advisor"]),
            s(&e["reason"])
        ));
    }
    for step in list(&v["local_review"]) {
        out.push_str(&format!("local       {step}\n"));
    }
    out
}

fn step_text(v: &Value) -> String {
    format!(
        "[reasoning] {} {}\n  {}\n  recorded {}\n",
        s(&v["step"]["phase"]),
        s(&v["step"]["id"]),
        s(&v["step"]["summary"]),
        s(&v["path"])
    )
}

fn status_text(v: &Value) -> String {
    let st = &v["state"];
    let mut out = format!(
        "task        {}\nmode        {} ({}) · advisors {} available, {} unavailable\nrecords     {} · consultations {} completed, {} failed · independent reviewers {}\n",
        s(&st["task"]),
        s(&v["mode"]["mode"]),
        s(&v["mode"]["source"]),
        v["advisors_available"],
        v["advisors_unavailable"],
        st["totals"]["records"],
        st["totals"]["consultations_completed"],
        st["totals"]["consultations_failed"],
        st["totals"]["independent_reviewers"]
    );
    for e in st["timeline"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "  {}  {:<16} {}\n",
            s(&e["at"]),
            s(&e["phase"]),
            s(&e["summary"])
        ));
    }
    out
}

fn explain_text(v: &Value) -> String {
    let mut out = format!(
        "record      {} ({})\nassessment  {}\n",
        s(&v["record"]["id"]),
        s(&v["record"]["body"]["kind"]),
        v["assessment"].as_str().unwrap_or("none")
    );
    for e in v["chain"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "  {:<16} {}  {}\n",
            s(&e["phase"]),
            s(&e["id"]),
            s(&e["summary"])
        ));
    }
    out
}

fn check_text(v: &Value) -> String {
    let mut out = format!("checks      {}\n", list(&v["checks"]).join(", "));
    let findings = v["findings"].as_array().cloned().unwrap_or_default();
    if findings.is_empty() {
        out.push_str("ok          no finding\n");
    }
    for f in findings {
        out.push_str(&format!(
            "FAIL {:<9} {}: {}\n",
            s(&f["check"]),
            s(&f["subject"]),
            s(&f["message"])
        ));
    }
    out
}
