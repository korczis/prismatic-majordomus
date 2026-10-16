//! `majordomus plan delivery`: every issue's furthest delivery stage, through the registry's
//! own `plan.delivery` capability.
//!
//! This file renders; it infers nothing. The stage, its proof and what was left unread are
//! the capability's output, so the command line cannot call an issue merged that the HTTP
//! route and the MCP tool would not. An unsettled stage is printed with the sources that
//! could have moved it, never as a bare `none`.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::{CapabilityError, Context};
use crate::cli::{OutputFormat, PlanArgs, PlanCommand};
use crate::error::{Error, Result};

/// Run `majordomus plan <subcommand>`.
pub fn run(args: PlanArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let v = match &args.command {
        PlanCommand::Delivery { issue, at } => {
            let mut input = json!({});
            if let Some(i) = issue {
                input["issue"] = json!(i);
            }
            if let Some(a) = at {
                input["at"] = json!(a);
            }
            execute(&app.context, &["plan", "delivery"], input)?
        }
    };
    let body = match args.format {
        OutputFormat::Json => serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
        OutputFormat::Text => delivery_text(&v),
    };
    writeln!(std::io::stdout().lock(), "{body}").map_err(Error::Transport)?;
    Ok(0)
}

fn execute(ctx: &Context, path: &[&str], input: Value) -> Result<Value> {
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.to_string())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(&id, input).map_err(|e| match e {
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })
}

fn s<'a>(v: &'a Value, pointer: &str) -> &'a str {
    v.pointer(pointer).and_then(Value::as_str).unwrap_or("")
}

fn short(sha: &str) -> &str {
    sha.get(..11).unwrap_or(sha)
}

/// What proved one issue's stage, in one phrase.
fn proof(i: &Value) -> String {
    match s(i, "/stage") {
        "deployed" => format!(
            "{} published as {} ({})",
            s(i, "/released/tag"),
            s(i, "/deployed/version"),
            s(i, "/deployed/latest_json")
        ),
        "released" => format!(
            "{} contains {}",
            s(i, "/released/tag"),
            short(s(i, "/merged/commit"))
        ),
        "merged" => {
            let pr = i
                .pointer("/merged/pull_request")
                .and_then(Value::as_u64)
                .map(|n| format!("#{n} "))
                .unwrap_or_default();
            format!("{pr}{}", short(s(i, "/merged/commit")))
        }
        "pr_open" => format!(
            "#{} {}",
            i.pointer("/pr_open/number")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            s(i, "/pr_open/head_ref")
        ),
        "branch" => s(i, "/branch/name").to_string(),
        _ => String::new(),
    }
}

fn delivery_text(v: &Value) -> String {
    let mut out = format!(
        "plan delivery — {} at {} {} ({})\n",
        s(v, "/observed/mode"),
        s(v, "/observed/trunk"),
        short(s(v, "/observed/commit")),
        s(v, "/observed/observed_at")
    );
    for src in v["sources"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "  {:<12} {:<7} {}\n",
            s(src, "/name"),
            s(src, "/state"),
            s(src, "/detail")
        ));
    }
    let tallies: Vec<String> = v["stages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|st| format!("{st} {}", v["tallies"][st].as_u64().unwrap_or(0)))
        .collect();
    out.push_str(&format!(
        "stages: {} · unsettled {}\n\n",
        tallies.join(" · "),
        v["unsettled"].as_u64().unwrap_or(0)
    ));
    for i in v["issues"].as_array().into_iter().flatten() {
        let unread: Vec<&str> = i["unread"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let tail = if unread.is_empty() {
            proof(i)
        } else {
            format!("{} (unread: {})", proof(i), unread.join(", "))
        };
        out.push_str(
            format!(
                "{:<6} {:<8} {:<9} {}\n",
                s(i, "/issue"),
                s(i, "/status"),
                s(i, "/stage"),
                tail.trim()
            )
            .as_str(),
        );
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stage_names_its_proof_and_unread_sources_are_said() {
        let v = json!({
            "observed": {"mode": "at", "trunk": "HEAD", "commit": "0123456789abcdef", "observed_at": "t"},
            "sources": [{"name": "forge", "state": "unread", "detail": "never observed"}],
            "stages": ["none", "merged"],
            "tallies": {"none": 1, "merged": 1},
            "unsettled": 1,
            "issues": [
                {"issue": "I0001", "status": "READY", "stage": "none", "unread": ["forge"]},
                {"issue": "I0002", "status": "DONE", "stage": "merged", "unread": [],
                 "merged": {"commit": "fedcba9876543210", "pull_request": 7}},
                {"issue": "I0003", "status": "DONE", "stage": "released", "unread": [],
                 "merged": {"commit": "fedcba9876543210"}, "released": {"tag": "v0.1.0"}},
                {"issue": "I0004", "status": "DONE", "stage": "deployed", "unread": [],
                 "released": {"tag": "v0.1.0"}, "deployed": {"version": "0.2.0", "latest_json": "x"}},
                {"issue": "I0005", "status": "ACTIVE", "stage": "pr_open", "unread": [],
                 "pr_open": {"number": 9, "head_ref": "fix/y"}},
                {"issue": "I0006", "status": "ACTIVE", "stage": "branch", "unread": ["forge"],
                 "branch": {"name": "feature/z"}}
            ]
        });
        let t = delivery_text(&v);
        assert!(
            t.contains("plan delivery — at at HEAD 0123456789a (t)"),
            "{t}"
        );
        assert!(t.contains("forge        unread  never observed"), "{t}");
        assert!(t.contains("stages: none 1 · merged 1 · unsettled 1"), "{t}");
        assert!(
            t.contains("I0001  READY    none      (unread: forge)"),
            "{t}"
        );
        assert!(t.contains("#7 fedcba98765"), "{t}");
        assert!(t.contains("v0.1.0 contains fedcba98765"), "{t}");
        assert!(t.contains("v0.1.0 published as 0.2.0 (x)"), "{t}");
        assert!(t.contains("#9 fix/y"), "{t}");
        assert!(t.contains("feature/z (unread: forge)"), "{t}");
    }
}
