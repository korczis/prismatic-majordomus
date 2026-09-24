//! `majordomus delivery`: every product feature against the delivery invariant, through the
//! registry's own `delivery.*` capabilities.
//!
//! This file renders; it decides nothing. The verdicts, the stage and `exists` are the
//! capability's output, so the command line cannot call a feature delivered that the HTTP
//! route and the MCP tool would not. The rendering keeps the three verdicts three: a pass
//! is `pass`, a fail is `FAIL` and an unknown is `unknown`, never a blank that reads as fine.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::{CapabilityError, Context};
use crate::cli::{DeliveryArgs, DeliveryCommand, OutputFormat};
use crate::error::{Error, Result};

/// The exit code of `--check` when a feature does not exist.
pub const EXIT_NOT_DELIVERED: u8 = 10;

/// Run `majordomus delivery`.
pub fn run(args: DeliveryArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let (v, check, text): (Value, bool, fn(&Value) -> String) = match &args.command {
        DeliveryCommand::Report { check } => (
            execute(&app.context, &["delivery", "report"], json!({}))?,
            *check,
            report_text,
        ),
        DeliveryCommand::Show { id, check } => (
            execute(&app.context, &["delivery", "show"], json!({ "id": id }))?,
            *check,
            feature_text,
        ),
    };
    let body = match args.format {
        OutputFormat::Json => serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
        OutputFormat::Text => text(&v),
    };
    writeln!(std::io::stdout().lock(), "{body}").map_err(Error::Transport)?;
    Ok(verdict(&v, check))
}

/// The exit code: 0, or 10 under `--check` when anything examined does not exist.
fn verdict(v: &Value, check: bool) -> u8 {
    let all_exist = match v.get("features").and_then(Value::as_array) {
        Some(features) => features.iter().all(|f| f["exists"] == json!(true)),
        None => v["exists"] == json!(true),
    };
    if check && !all_exist {
        EXIT_NOT_DELIVERED
    } else {
        0
    }
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

fn word(verdict: &Value) -> &'static str {
    match verdict.as_str() {
        Some("pass") => "pass",
        Some("fail") => "FAIL",
        _ => "unknown",
    }
}

fn state(f: &Value) -> String {
    match f["delivery"]["state"].as_str() {
        Some("delivered") => "DELIVERED".into(),
        _ => format!(
            "NOT DELIVERED ({})",
            f["delivery"]["stage"].as_str().unwrap_or("unknown")
        ),
    }
}

fn identity_line(v: &Value) -> String {
    let p = &v["publication"];
    let identity = &p["identity"];
    let what = match identity["state"].as_str() {
        Some("served") => format!(
            "serves {}",
            identity["commit"]
                .as_str()
                .unwrap_or("")
                .get(..12)
                .unwrap_or("")
        ),
        _ => format!(
            "{}: {}",
            identity["state"].as_str().unwrap_or("unknown"),
            identity["reason"].as_str().unwrap_or("")
        ),
    };
    format!(
        "site {}  {what}; verification {}",
        p["url"].as_str().unwrap_or("(none)"),
        p["verification"]["state"].as_str().unwrap_or("unknown")
    )
}

fn report_text(v: &Value) -> String {
    let empty = Vec::new();
    let features = v["features"].as_array().unwrap_or(&empty);
    let width = features
        .iter()
        .map(|f| f["id"].as_str().unwrap_or("").len())
        .max()
        .unwrap_or(7)
        .max(7);
    let mut out = vec![format!(
        "{:<width$}  {:<9} {:<9} {:<9} {:<9} {:<9} {:<9} STATE",
        "FEATURE", "ON_MASTER", "DEPLOYED", "VERIFIED", "TESTS", "EVIDENCE", "UI"
    )];
    for f in features {
        let mut row = format!("{:<width$} ", f["id"].as_str().unwrap_or(""));
        for d in f["dimensions"].as_array().unwrap_or(&empty) {
            row.push_str(&format!(" {:<9}", word(&d["verdict"])));
        }
        row.push_str(&format!(" {}", state(f)));
        out.push(row);
    }
    let t = &v["tallies"];
    out.push(String::new());
    out.push(identity_line(v));
    out.push(format!(
        "{} feature(s): {} on master, {} deployed, {} publicly verified, {} exist  (trunk {})",
        t["features"],
        t["on_master"],
        t["deployed"],
        t["publicly_verified"],
        t["exists"],
        v["trunk"].as_str().unwrap_or("unknown")
    ));
    out.join("\n")
}

fn feature_text(f: &Value) -> String {
    let empty = Vec::new();
    let mut out = vec![
        format!(
            "{}  {}",
            f["id"].as_str().unwrap_or(""),
            f["title"].as_str().unwrap_or("")
        ),
        format!("  {}", state(f)),
        String::new(),
    ];
    for d in f["dimensions"].as_array().unwrap_or(&empty) {
        out.push(format!(
            "  {:<24} {:<8} {}",
            d["dimension"].as_str().unwrap_or(""),
            word(&d["verdict"]),
            d["reason"].as_str().unwrap_or("")
        ));
        if let Some(r) = d["remediation"].as_str() {
            out.push(format!("  {:<24} {:<8} -> {r}", "", ""));
        }
    }
    let paths: Vec<&str> = f["implementation"]["paths"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(Value::as_str)
        .collect();
    out.push(String::new());
    out.push(format!(
        "  implemented by: {}",
        if paths.is_empty() {
            "(nothing it names)".to_string()
        } else {
            paths.join(", ")
        }
    ));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(exists: bool) -> Value {
        json!({
            "id": "evidence",
            "title": "Evidence",
            "exists": exists,
            "delivery": if exists { json!({"state": "delivered"}) }
                        else { json!({"state": "not_delivered", "stage": "on_master"}) },
            "dimensions": [
                {"dimension": "on_master", "verdict": "pass", "reason": "on the trunk"},
                {"dimension": "deployed", "verdict": "fail", "reason": "older", "remediation": "publish"},
                {"dimension": "publicly_verified", "verdict": "unknown", "reason": "offline", "remediation": "retry"}
            ],
            "implementation": {"paths": ["lib/evidence.sh"]}
        })
    }

    #[test]
    fn the_three_verdicts_are_rendered_three_ways() {
        let text = feature_text(&feature(false));
        assert!(text.contains("NOT DELIVERED (on_master)"), "{text}");
        assert!(text.contains("pass"));
        assert!(text.contains("FAIL"));
        assert!(text.contains("unknown"));
        assert!(text.contains("-> publish"));
        assert!(text.contains("lib/evidence.sh"));
        let mut none = feature(true);
        none["implementation"]["paths"] = json!([]);
        let text = feature_text(&none);
        assert!(text.contains("DELIVERED") && !text.contains("NOT DELIVERED"));
        assert!(text.contains("(nothing it names)"));
    }

    #[test]
    fn the_report_carries_the_tallies_and_the_publication() {
        let v = json!({
            "trunk": "origin/master",
            "publication": {
                "url": "https://example.invalid",
                "identity": {"state": "served", "commit": "0123456789abcdef0123"},
                "verification": {"state": "unmeasured", "reason": "no gh"}
            },
            "tallies": {"features": 1, "on_master": 1, "deployed": 0, "publicly_verified": 0, "exists": 0},
            "features": [feature(false)]
        });
        let text = report_text(&v);
        assert!(text.contains("FEATURE"), "{text}");
        assert!(text.contains("evidence"));
        assert!(text.contains("serves 0123456789ab"));
        assert!(
            text.contains("1 feature(s): 1 on master, 0 deployed, 0 publicly verified, 0 exist")
        );
        let offline = json!({
            "publication": {"identity": {"state": "unreachable", "reason": "offline"}, "verification": {}},
            "tallies": {}, "features": []
        });
        let text = report_text(&offline);
        assert!(text.contains("unreachable: offline"), "{text}");
        assert!(text.contains("trunk unknown"));
    }

    #[test]
    fn check_exits_ten_unless_everything_examined_exists() {
        assert_eq!(verdict(&feature(false), false), 0);
        assert_eq!(verdict(&feature(false), true), EXIT_NOT_DELIVERED);
        assert_eq!(verdict(&feature(true), true), 0);
        let report = |a, b| json!({ "features": [feature(a), feature(b)] });
        assert_eq!(verdict(&report(true, true), true), 0);
        assert_eq!(verdict(&report(true, false), true), EXIT_NOT_DELIVERED);
    }
}
