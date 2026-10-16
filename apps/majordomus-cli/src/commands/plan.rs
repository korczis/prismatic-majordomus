//! `majordomus plan delivery` and `majordomus plan drift`: every issue's furthest delivery
//! stage, and every contradiction between it and the declared status, through the registry's
//! own `plan.delivery` and `plan.drift` capabilities.
//!
//! This file renders; it infers nothing. The stage, its proof and what was left unread are
//! the capability's output, so the command line cannot call an issue merged that the HTTP
//! route and the MCP tool would not. An unsettled stage is printed with the sources that
//! could have moved it, never as a bare `none`.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::{CapabilityError, Context};
use crate::cli::{DriftGateArg, OutputFormat, PlanArgs, PlanCommand};
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
        PlanCommand::Drift { issue, at, gate } => {
            let mut input = json!({});
            if let Some(i) = issue {
                input["issue"] = json!(i);
            }
            if let Some(a) = at {
                input["at"] = json!(a);
            }
            if let Some(g) = gate {
                input["gate"] = json!(match g {
                    DriftGateArg::Head => "head",
                    DriftGateArg::Trunk => "trunk",
                });
            }
            execute(&app.context, &["plan", "drift"], input)?
        }
    };
    let drift = matches!(args.command, PlanCommand::Drift { .. });
    let body = match args.format {
        OutputFormat::Json => serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
        OutputFormat::Text if drift => drift_text(&v),
        OutputFormat::Text => delivery_text(&v),
    };
    writeln!(std::io::stdout().lock(), "{body}").map_err(Error::Transport)?;
    // a gate's verdict is its exit code; a report exits 0
    Ok(if drift {
        v["exit_code"]
            .as_u64()
            .and_then(|c| u8::try_from(c).ok())
            .unwrap_or(0)
    } else {
        0
    })
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

/// One finding's proof, in one phrase.
fn drift_proof(f: &Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(n) = f.pointer("/proof/pull_request").and_then(Value::as_u64) {
        parts.push(format!("#{n}"));
    }
    if !s(f, "/proof/merge_commit").is_empty() {
        parts.push(short(s(f, "/proof/merge_commit")).to_string());
    }
    if !s(f, "/proof/merged_at").is_empty() {
        parts.push(format!("merged {}", s(f, "/proof/merged_at")));
    }
    if !s(f, "/proof/release").is_empty() {
        parts.push(format!("in {}", s(f, "/proof/release")));
    }
    if !s(f, "/proof/started_at").is_empty() {
        parts.push(format!("started {}", s(f, "/proof/started_at")));
    }
    if let Some(age) = f.pointer("/age_seconds").and_then(Value::as_u64) {
        parts.push(format!("age {}h", age / 3600));
    }
    if parts.is_empty() {
        parts.push("no merge names it".into());
    }
    parts.join(" ")
}

fn drift_text(v: &Value) -> String {
    let mut out = format!(
        "plan drift — {} at {} {} ({})\n",
        s(v, "/observed/mode"),
        s(v, "/observed/trunk"),
        short(s(v, "/observed/commit")),
        s(v, "/observed/observed_at")
    );
    out.push_str(&format!(
        "  window       {:<7} {}: {}\n",
        s(v, "/window/state"),
        s(v, "/window/model"),
        s(v, "/window/detail")
    ));
    for src in v["sources"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "  {:<12} {:<7} {}\n",
            s(src, "/name"),
            s(src, "/state"),
            s(src, "/detail")
        ));
    }
    let tallies: Vec<String> = v["classes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|c| format!("{c} {}", v["tallies"][c].as_u64().unwrap_or(0)))
        .collect();
    out.push_str(&format!("classes: {}\n\n", tallies.join(" · ")));
    for f in v["findings"].as_array().into_iter().flatten() {
        let tag = match s(f, "/verdict") {
            "refused" => "REFUSED",
            "pending" => "PENDING",
            "reported" => "NOTE",
            "advisory" => "ADVISORY",
            _ => "FINDING",
        };
        let unread: Vec<&str> = f["unread"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let unread = if unread.is_empty() {
            String::new()
        } else {
            format!(" (unread: {})", unread.join(", "))
        };
        out.push_str(&format!(
            "{:<8} {:<6} {:<8} {:<19} {}{} — {}\n",
            tag,
            s(f, "/issue"),
            s(f, "/status"),
            s(f, "/class"),
            drift_proof(f),
            unread,
            s(f, "/remedy")
        ));
    }
    let gate = s(v, "/gate");
    if gate.is_empty() {
        out.push_str(&format!(
            "plan drift: {} finding(s), reported; no gate was asked",
            v["findings"].as_array().map_or(0, Vec::len)
        ));
    } else {
        out.push_str(&format!(
            "plan drift ({gate}): {} — {} refused, {} pending{}",
            s(v, "/verdict"),
            v["refused"].as_u64().unwrap_or(0),
            v["pending"].as_u64().unwrap_or(0),
            match s(v, "/verdict") {
                "advisory" => " (drift.enforcement is advisory: not enforced)",
                "unjudged" => " (no window: nothing can be called young)",
                _ => "",
            }
        ));
    }
    out
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

    #[test]
    fn a_drift_finding_names_its_proof_its_verdict_and_its_remedy() {
        let v = json!({
            "observed": {"mode": "live", "trunk": "origin/master", "commit": "0123456789abcdef", "observed_at": "t"},
            "window": {"model": ".ai/repo/ci/plan.yaml", "state": "read", "detail": "drift.window 60s, enforcement advisory"},
            "gate": "trunk",
            "sources": [{"name": "claims", "state": "unread", "detail": "no mesh"}],
            "classes": ["merged_not_recorded", "done_without_trace"],
            "tallies": {"merged_not_recorded": 1, "done_without_trace": 1},
            "findings": [
                {"issue": "I0001", "status": "READY", "class": "merged_not_recorded", "verdict": "refused",
                 "proof": {"pull_request": 5, "merge_commit": "fedcba9876543210", "merged_at": "2026-10-01T00:00:00Z"},
                 "age_seconds": 7200, "unread": ["forge"], "remedy": "majordomus plan verify|done I0001"},
                {"issue": "I0002", "status": "DONE", "class": "done_without_trace", "verdict": "advisory",
                 "proof": {}, "unread": [], "remedy": "nothing"}
            ],
            "refused": 1, "pending": 0, "verdict": "advisory", "exit_code": 0
        });
        let t = drift_text(&v);
        assert!(
            t.contains("window       read    .ai/repo/ci/plan.yaml"),
            "{t}"
        );
        assert!(t.contains("REFUSED  I0001  READY    merged_not_recorded #5 fedcba98765 merged 2026-10-01T00:00:00Z age 2h (unread: forge) — majordomus plan verify|done I0001"), "{t}");
        assert!(
            t.contains("ADVISORY I0002  DONE     done_without_trace  no merge names it — nothing"),
            "{t}"
        );
        assert!(t.contains("plan drift (trunk): advisory — 1 refused, 0 pending (drift.enforcement is advisory"), "{t}");
    }
}
