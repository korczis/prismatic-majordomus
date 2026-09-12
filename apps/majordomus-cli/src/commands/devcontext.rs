//! `majordomus devcontext`: the context compiler, through the registry's own
//! `devcontext.*` capabilities.
//!
//! This file renders; it selects nothing. Every answer below is the output of the capability
//! the CLI exposure names, executed through the one executor, so the command line, the HTTP
//! routes and the MCP tools cannot answer the same request differently — which is the
//! property a context compiler most needs, because two surfaces that disagreed about what a
//! session should be given would be two compilers.

use std::io::Write;

use serde_json::Value;

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{DevcontextArgs, DevcontextCommand, DevcontextRequest, OutputFormat};
use crate::error::{Error, Result};

/// The exit code when the entries that may not be dropped already exceed the budget: the
/// same code, for the same condition, as the shell tool's `majordomus context`.
pub const EXIT_OVER_BUDGET: u8 = 10;

/// Run `majordomus devcontext`.
pub fn run(args: DevcontextArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let format = args.format;
    match &args.command {
        DevcontextCommand::Compile(req) => {
            let v = call(&app, &["devcontext", "compile"], request_of(req))?;
            emit(format, &v, compiled_text)?;
            Ok(if v["budget"]["over_budget"].as_bool().unwrap_or(false) {
                EXIT_OVER_BUDGET
            } else {
                0
            })
        }
        DevcontextCommand::Explain { uri, request } => {
            let mut input = request_of(request);
            input["uri"] = Value::String(uri.clone());
            let v = call(&app, &["devcontext", "explain"], input)?;
            emit(format, &v, explanation_text)
        }
        DevcontextCommand::Policy => {
            let v = call(&app, &["devcontext", "policy"], serde_json::json!({}))?;
            emit(format, &v, policy_text)
        }
    }
}

/// The request flags as the capability's own input. The domain type is serialised rather
/// than a JSON object assembled by hand, so a field added to the request is a flag here and
/// a key of the input at once.
fn request_of(r: &DevcontextRequest) -> Value {
    let input = crate::devcontext::CompileInput {
        issue: r.issue.clone(),
        milestone: r.milestone.clone(),
        intent: r.intent.clone(),
        paths: r.paths.clone(),
        uris: r.uris.clone(),
        budget_tokens: r.budget_tokens,
        max_depth: r.max_depth,
        floor: r.floor,
        all_blocking_rules: r.all_blocking_rules.then_some(true),
    };
    serde_json::to_value(input).unwrap_or(Value::Null)
}

fn call(app: &App, path: &[&str], input: Value) -> Result<Value> {
    let ctx = &app.context;
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
    ctx.execute(&id, input).map_err(map)
}

fn map(e: CapabilityError) -> Error {
    match e {
        // an issue that does not exist is a missing artifact, not an internal failure
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        CapabilityError::InvalidInput(reason) => Error::Protocol { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn emit(format: OutputFormat, v: &Value, text: impl Fn(&Value) -> String) -> Result<u8> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let body = match format {
        OutputFormat::Json => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
        OutputFormat::Text => text(v),
    };
    writeln!(out, "{body}").map_err(Error::Transport)?;
    Ok(0)
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn short(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

fn compiled_text(v: &Value) -> String {
    let mut o = String::new();
    let git = &v["git"];
    o.push_str(&format!(
        "devcontext   index {}  head {} ({}, {})\n",
        short(s(&v["fingerprint"])),
        short(s(&git["head"])),
        git["branch"].as_str().unwrap_or("detached"),
        s(&git["working_tree"])
    ));
    for seed in v["seeds"].as_array().into_iter().flatten() {
        o.push_str(&format!(
            "seed         {:<10} {}\n",
            s(&seed["kind"]),
            s(&seed["uri"])
        ));
    }
    let b = &v["budget"];
    o.push_str(&format!(
        "budget       {} of {} token(s) used, {} required, {} bytes per token{}\n",
        b["used_tokens"],
        b["limit_tokens"],
        b["required_tokens"],
        b["bytes_per_token"],
        if b["over_budget"].as_bool().unwrap_or(false) {
            "  OVER BUDGET: what may not be dropped already exceeds the limit"
        } else {
            ""
        }
    ));
    for t in b["tiers"].as_array().into_iter().flatten() {
        o.push_str(&format!(
            "  {:<11} {:>3} selected {:>3} dropped {:>7} tokens\n",
            s(&t["tier"]),
            t["selected"],
            t["excluded"],
            t["tokens"]
        ));
    }

    let selected = v["selected"].as_array().cloned().unwrap_or_default();
    o.push_str(&format!("\nSELECTED ({})\n", selected.len()));
    o.push_str("TIER        REL   CONF  TOKENS  URI\n");
    for e in &selected {
        o.push_str(&format!(
            "{:<11} {:<5.3} {:<5.3} {:>6}  {}{}\n",
            s(&e["tier"]),
            e["relevance"].as_f64().unwrap_or(0.0),
            e["confidence"].as_f64().unwrap_or(0.0),
            e["cost_tokens"],
            s(&e["uri"]),
            if e["required"].as_bool().unwrap_or(false) {
                "  (required)"
            } else {
                ""
            }
        ));
        for d in e["discovered_by"].as_array().into_iter().flatten() {
            let edge = d["edge"]
                .as_str()
                .map(|x| format!("({x})"))
                .unwrap_or_default();
            o.push_str(&format!(
                "            <- {}{} {}\n",
                s(&d["selector"]),
                edge,
                s(&d["reason"])
            ));
        }
    }

    let excluded = v["excluded"].as_array().cloned().unwrap_or_default();
    o.push_str(&format!("\nEXCLUDED ({})\n", excluded.len()));
    for e in &excluded {
        o.push_str(&format!(
            "{:<11} {:<10} {:>6}  {}  — {}\n",
            s(&e["tier"]),
            s(&e["reason"]),
            e["cost_tokens"],
            s(&e["uri"]),
            s(&e["detail"])
        ));
    }

    let dedup = v["deduplicated"].as_array().cloned().unwrap_or_default();
    o.push_str(&format!("\nDEDUPLICATED ({})\n", dedup.len()));
    for d in &dedup {
        o.push_str(&format!(
            "{:<9} {}  — {}\n",
            s(&d["key"]),
            s(&d["kept"]),
            s(&d["detail"])
        ));
    }

    let conflicts = v["conflicts"].as_array().cloned().unwrap_or_default();
    if !conflicts.is_empty() {
        o.push_str(&format!("\nCONFLICTS ({})\n", conflicts.len()));
        for c in &conflicts {
            o.push_str(&format!(
                "{:<11} {} over {}  — {}\n",
                s(&c["kind"]),
                s(&c["current"]),
                s(&c["against"]),
                s(&c["detail"])
            ));
        }
    }

    let commits = git["commits"].as_array().cloned().unwrap_or_default();
    if !commits.is_empty() {
        o.push_str(&format!("\nCOMMITS THE LAYER NAMES ({})\n", commits.len()));
        for c in &commits {
            o.push_str(&format!(
                "{}  {}{}\n",
                short(s(&c["commit"])),
                s(&c["named_by"]),
                c["covers"]
                    .as_str()
                    .map(|x| format!("  ({x})"))
                    .unwrap_or_default()
            ));
        }
    }

    let diags = v["diagnostics"].as_array().cloned().unwrap_or_default();
    if !diags.is_empty() {
        o.push_str(&format!("\nDIAGNOSTICS ({})\n", diags.len()));
        for d in &diags {
            o.push_str(&format!(
                "{:<8} {:<22} {}\n",
                s(&d["severity"]),
                s(&d["code"]),
                s(&d["message"])
            ));
        }
    }
    o.trim_end().to_string()
}

fn explanation_text(v: &Value) -> String {
    format!(
        "{}  {}\n{}\nbudget {} of {} token(s) used",
        s(&v["standing"]),
        s(&v["uri"]),
        s(&v["detail"]),
        v["budget"]["used_tokens"],
        v["budget"]["limit_tokens"]
    )
}

fn policy_text(v: &Value) -> String {
    let mut o = String::new();
    o.push_str(&format!(
        "graph        {}\ndefaults     budget {} tokens, depth {}, floor {}, {} bytes per token\n\nTIER  POSITION  KINDS\n",
        s(&v["graph"]),
        v["default_budget_tokens"],
        v["default_max_depth"],
        v["default_floor"],
        v["bytes_per_token"]
    ));
    for t in v["tiers"].as_array().into_iter().flatten() {
        let kinds: Vec<&str> = t["kinds"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        o.push_str(&format!(
            "{:<11} {}  {}\n            {}\n",
            s(&t["tier"]),
            t["position"],
            kinds.join(", "),
            s(&t["meaning"])
        ));
    }
    o.push_str("\nEDGE             FORWARD  REVERSE  REASON\n");
    for e in v["edges"].as_array().into_iter().flatten() {
        let w = |k: &str| {
            e[k].as_f64()
                .map(|x| format!("{x}"))
                .unwrap_or_else(|| "-".into())
        };
        o.push_str(&format!(
            "{:<16} {:<8} {:<8} {}{}\n",
            s(&e["edge"]),
            w("forward"),
            w("reverse"),
            if e["refused"].as_bool().unwrap_or(false) {
                "REFUSED: "
            } else {
                ""
            },
            s(&e["reason"])
        ));
    }
    o.push_str("\nSELECTOR       DECLARED\n");
    for sel in v["selectors"].as_array().into_iter().flatten() {
        o.push_str(&format!(
            "{:<14} {}\n",
            s(&sel["selector"]),
            if sel["declared"].as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no — inferred, confidence below one"
            }
        ));
    }
    o.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_text_says_what_was_selected_excluded_and_why() {
        let v = json!({
            "fingerprint": "abcdef0123456789",
            "git": {"head": "0123456789abcdef", "branch": "master", "working_tree": "clean",
                    "commits": [{"commit": "feedfacefeedface", "named_by": "majordomus://issue/I1", "covers": "proof"}]},
            "seeds": [{"uri": "majordomus://issue/I1", "kind": "issue", "from": "issue"}],
            "budget": {"used_tokens": 10, "limit_tokens": 20, "required_tokens": 5,
                       "bytes_per_token": 4, "over_budget": false,
                       "tiers": [{"tier": "task", "selected": 1, "excluded": 0, "tokens": 10}]},
            "selected": [{"tier": "task", "relevance": 1.0, "confidence": 1.0, "cost_tokens": 10,
                          "uri": "majordomus://issue/I1", "required": true,
                          "discovered_by": [{"selector": "seed", "reason": "named in the request"}]}],
            "excluded": [{"tier": "history", "reason": "budget", "cost_tokens": 99,
                          "uri": "majordomus://claim/x", "detail": "would not fit"}],
            "deduplicated": [{"key": "uri", "kept": "majordomus://adr/a", "detail": "two paths"}],
            "conflicts": [],
            "diagnostics": []
        });
        let t = compiled_text(&v);
        for want in [
            "SELECTED (1)",
            "majordomus://issue/I1  (required)",
            "<- seed named in the request",
            "EXCLUDED (1)",
            "budget",
            "would not fit",
            "DEDUPLICATED (1)",
            "COMMITS THE LAYER NAMES (1)",
            "10 of 20 token(s) used",
        ] {
            assert!(t.contains(want), "missing {want:?} in\n{t}");
        }
        assert!(!t.contains("OVER BUDGET"));
    }

    #[test]
    fn the_request_flags_become_the_capability_input() {
        let r = DevcontextRequest {
            issue: Some("I0001".into()),
            milestone: None,
            intent: None,
            paths: vec!["lib".into()],
            uris: vec![],
            budget_tokens: Some(100),
            max_depth: None,
            floor: None,
            all_blocking_rules: false,
        };
        let v = request_of(&r);
        assert_eq!(v["issue"], "I0001");
        assert_eq!(v["paths"][0], "lib");
        assert_eq!(v["budget_tokens"], 100);
        // an unset flag is absent, not null: the input type denies unknown fields and the
        // executor's cache key is the canonical form
        assert!(v.get("milestone").is_none());
        assert!(v.get("all_blocking_rules").is_none());
    }
}
