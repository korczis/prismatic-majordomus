//! `majordomus intent`: the intents, through the registry's own `intent.*` capabilities.
//!
//! This file renders; it decides nothing. Every answer is the output of the capability the
//! CLI exposure names, executed through the one executor, so the command line, the HTTP
//! routes and the MCP tools cannot answer the same question differently. The two verdicts
//! — `validate` and `preflight` — are read out of that output and turned into exit 10.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::{CapabilityError, Context};
use crate::cli::{IntentArgs, IntentCommand, OutputFormat};
use crate::error::{Error, Result};

/// The exit code when `validate` finds a failure or `preflight` refuses.
pub const EXIT_INVALID: u8 = 10;

/// Run `majordomus intent`.
pub fn run(args: IntentArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let format = args.format;
    match &args.command {
        IntentCommand::List => {
            let v = call(&app.context, &["intent", "list"], json!({}))?;
            emit(format, &v, list_text)?;
            Ok(0)
        }
        IntentCommand::Show { id } => {
            let v = call(&app.context, &["intent", "show"], json!({ "id": id }))?;
            emit(format, &v, show_text)?;
            Ok(0)
        }
        IntentCommand::Validate => {
            let v = call(&app.context, &["intent", "validate"], json!({}))?;
            emit(format, &v, validate_text)?;
            Ok(if v["valid"].as_bool() == Some(true) {
                0
            } else {
                EXIT_INVALID
            })
        }
        // the coverage is a projection of records the load already read: it has no refusal
        // of its own, so the only failure is the write, which is returned as it is
        IntentCommand::Coverage => call(&app.context, &["intent", "coverage"], json!({}))
            .and_then(|v| emit(format, &v, coverage_text))
            .map(|()| 0),
        IntentCommand::Preflight { issue, paths } => {
            let mut input = json!({ "paths": paths.join(",") });
            if let Some(issue) = issue {
                input["issue"] = json!(issue);
            }
            let v = call(&app.context, &["intent", "preflight"], input)?;
            emit(format, &v, preflight_text)?;
            Ok(if v["verdict"] == "refused" {
                EXIT_INVALID
            } else {
                0
            })
        }
        IntentCommand::Realization { intent } => {
            let mut input = json!({});
            if let Some(intent) = intent {
                input["intent"] = json!(intent);
            }
            let v = call(&app.context, &["intent", "realization"], input)?;
            emit(format, &v, realization_text)?;
            let regressed = v["findings"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|f| f["code"] == "closed_work_contradicted");
            Ok(if regressed { EXIT_INVALID } else { 0 })
        }
        IntentCommand::Explain { id } => {
            let v = call(&app.context, &["intent", "explain"], json!({ "id": id }))?;
            emit(format, &v, explain_text)?;
            Ok(0)
        }
    }
}

/// Each intent with how far reality is from it and who realises it, then every unit of work
/// with its strongest link or the reason it has none.
fn realization_text(v: &Value) -> String {
    let mut out = Vec::new();
    for i in v["intents"].as_array().into_iter().flatten() {
        out.push(format!(
            "{}  {}  {}/{} met  {}",
            s(i, "intent"),
            s(i, "stage"),
            i["met"],
            i["criteria"],
            s(i, "title")
        ));
        for c in i["unmet"].as_array().into_iter().flatten() {
            let issues: Vec<&str> = c["issues"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            out.push(format!(
                "  unmet     {}  {}{}",
                s(c, "id"),
                s(c, "state"),
                if issues.is_empty() {
                    String::new()
                } else {
                    format!("  served by {}", issues.join(" "))
                }
            ));
        }
        for w in i["work"].as_array().into_iter().flatten() {
            let providers: Vec<&str> = w["providers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            out.push(format!(
                "  work      {} {}  {}  {}  handovers {}{}",
                s(w, "kind"),
                s(w, "id"),
                s(w, "outcome"),
                s(w, "provenance"),
                w["handovers"],
                if providers.is_empty() {
                    String::new()
                } else {
                    format!("  by {}", providers.join(", "))
                }
            ));
        }
    }
    let work = v["work"].as_array().cloned().unwrap_or_default();
    if !work.is_empty() {
        out.push(String::new());
    }
    for w in &work {
        let unit = &w["work"];
        let link = w["links"].as_array().and_then(|l| l.first()).map(|l| {
            format!(
                "{} via {} {} ({})",
                s(l, "intent"),
                s(l, "issue"),
                s(l, "via"),
                s(l, "provenance")
            )
        });
        out.push(format!(
            "{} {}  {}  {}",
            s(unit, "kind"),
            s(unit, "id"),
            s(unit, "outcome"),
            link.unwrap_or_else(|| format!("unlinked: {}", s(w, "unlinked")))
        ));
    }
    findings_text(&mut out, &v["findings"]);
    out.push(format!(
        "{} intent(s), {} unit(s) of work, {} serving no intent",
        v["intents"].as_array().map_or(0, Vec::len),
        work.len(),
        v["orphans"]
    ));
    out.join("\n")
}

fn explain_text(v: &Value) -> String {
    let i = &v["intent"];
    let mut out = vec![
        format!("{}  {}", s(i, "id"), s(i, "title")),
        String::new(),
        format!("  {}", s(i, "statement")),
        String::new(),
    ];
    for b in v["because"].as_array().into_iter().flatten() {
        out.push(format!("  - {}", b.as_str().unwrap_or("")));
    }
    out.join("\n")
}

fn call(ctx: &Context, path: &[&str], input: Value) -> Result<Value> {
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

fn emit(format: OutputFormat, v: &Value, text: impl Fn(&Value) -> String) -> Result<()> {
    let body = match format {
        // `{:#}` is serde_json's own pretty writer, the one `to_string_pretty` runs, so the
        // bytes are the same; serialising a `Value` cannot fail, so there is no fallback
        OutputFormat::Json => format!("{v:#}"),
        OutputFormat::Text => text(v),
    };
    writeln!(std::io::stdout().lock(), "{body}").map_err(Error::Transport)
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}

fn list_text(v: &Value) -> String {
    let intents = v["intents"].as_array().cloned().unwrap_or_default();
    let width = intents
        .iter()
        .map(|i| s(i, "id").len())
        .max()
        .unwrap_or(2)
        .max(2);
    let mut out = vec![format!(
        "{:<width$}  {:<10}  {:<11}  {:<7}  TITLE",
        "ID", "STAGE", "VERDICT", "MET"
    )];
    for i in &intents {
        let total = i["satisfaction"].as_array().map_or(0, Vec::len);
        out.push(format!(
            "{:<width$}  {:<10}  {:<11}  {:<7}  {}",
            s(i, "id"),
            s(i, "stage"),
            s(&i["verdict"], "state"),
            format!("{}/{total}", i["met"]),
            s(i, "title"),
        ));
    }
    out.push(String::new());
    out.push(format!("{} intent(s)", v["count"]));
    out.join("\n")
}

/// Every criterion with the work that carries it, then every issue with the reason it exists.
fn coverage_text(v: &Value) -> String {
    let criteria = v["criteria"].as_array().cloned().unwrap_or_default();
    let mut out = vec![format!("{:<28}  {:<9}  ISSUES", "CRITERION", "STRENGTH")];
    for c in &criteria {
        let issues: Vec<&str> = c["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|i| s(i, "id"))
            .collect();
        out.push(format!(
            "{:<28}  {:<9}  {}",
            format!("{}#{}", s(c, "intent"), s(c, "criterion")),
            s(c, "strength"),
            if issues.is_empty() {
                "—".into()
            } else {
                issues.join(" ")
            },
        ));
    }
    let issues = v["issues"].as_array().cloned().unwrap_or_default();
    let mut origins: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for i in &issues {
        *origins.entry(s(i, "origin")).or_default() += 1;
    }
    out.push(String::new());
    out.push(format!(
        "{} criterion(s); {} issue(s): {}",
        criteria.len(),
        issues.len(),
        origins
            .iter()
            .map(|(o, n)| format!("{n} {o}"))
            .collect::<Vec<_>>()
            .join(", "),
    ));
    out.join("\n")
}

fn show_text(v: &Value) -> String {
    let mut out = vec![
        format!("{}  {}", s(v, "id"), s(v, "title")),
        String::new(),
        format!("  {}", s(v, "statement")),
        String::new(),
        format!(
            "  stage {}   verdict {}   source {}",
            s(v, "stage"),
            s(&v["verdict"], "state"),
            s(v, "source")
        ),
    ];
    for inv in v["invariants"].as_array().into_iter().flatten() {
        out.push(format!("  invariant   {}", inv.as_str().unwrap_or("")));
    }
    for m in v["milestones"].as_array().into_iter().flatten() {
        out.push(format!(
            "  milestone   {}  {}",
            s(m, "id"),
            m["status"].as_str().unwrap_or("unresolved")
        ));
    }
    for c in v["satisfaction"].as_array().into_iter().flatten() {
        out.push(format!(
            "  criterion   {}  {}{}  {} {}{}",
            s(c, "id"),
            s(c, "state"),
            // a met criterion names the verdict it rests on: `proven` and `inputs_unchanged`
            // are two answers, never one tick
            match (c["met"].as_bool(), c["proof"].as_str()) {
                (Some(true), Some(p)) => format!(" ({p})"),
                _ => String::new(),
            },
            s(c, "evidence"),
            s(c, "ref"),
            c["reproduce"]
                .as_str()
                .map(|r| format!("  [reproduce: {r}]"))
                .unwrap_or_default()
        ));
    }
    for r in v["verdict"]["reasons"].as_array().into_iter().flatten() {
        out.push(format!(
            "  held back   {}  {} {}",
            s(r, "criterion"),
            s(r, "evidence"),
            s(r, "state"),
        ));
    }
    for g in v["governance"].as_array().into_iter().flatten() {
        out.push(format!("  governance  {}", g.as_str().unwrap_or("")));
    }
    out.join("\n")
}

fn findings_text(out: &mut Vec<String>, findings: &Value) {
    for f in findings.as_array().into_iter().flatten() {
        out.push(format!(
            "{} {}  {}: {}  [reproduce: {}]",
            s(f, "level"),
            s(f, "code"),
            s(f, "subject"),
            s(f, "message"),
            s(f, "reproduce"),
        ));
    }
}

fn validate_text(v: &Value) -> String {
    let mut out = Vec::new();
    findings_text(&mut out, &v["findings"]);
    out.push(format!(
        "{} intent(s), {} failure(s), {} warning(s): {}",
        v["intents"],
        v["failures"],
        v["warnings"],
        if v["valid"].as_bool() == Some(true) {
            "valid"
        } else {
            "invalid"
        }
    ));
    out.join("\n")
}

fn words(v: &Value) -> Vec<&str> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// The verdict, each issue with its own, each intent the work is held to with what it asks
/// of the worker, then every refusal with its cause.
fn preflight_text(v: &Value) -> String {
    let mut out = vec![format!("verdict     {}", s(v, "verdict"))];
    for i in v["issues"].as_array().into_iter().flatten() {
        let serves = words(&i["serves"]);
        out.push(format!(
            "issue       {}  {}  milestone {}{}",
            s(i, "issue"),
            s(i, "verdict"),
            s(i, "milestone"),
            if serves.is_empty() {
                String::new()
            } else {
                format!("  serves {}", serves.join(" "))
            }
        ));
    }
    for i in v["intents"].as_array().into_iter().flatten() {
        out.push(format!(
            "intent      {}  {}  {}",
            s(i, "id"),
            s(i, "stage"),
            s(i, "title")
        ));
        out.push(format!("  statement   {}", s(i, "statement")));
        for c in i["criteria"].as_array().into_iter().flatten() {
            out.push(format!("  criterion   {}  {}", s(c, "id"), s(c, "state")));
        }
        for inv in words(&i["invariants"]) {
            out.push(format!("  invariant   {inv}"));
        }
        for n in words(&i["non_goals"]) {
            out.push(format!("  non-goal    {n}"));
        }
        let critique = &i["critique"];
        if critique.is_object() {
            let open: Vec<&str> = critique["open_blocking"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|f| s(f, "id"))
                .collect();
            out.push(format!(
                "  critique    reviewed at {} by {}; open blocking: {}",
                s(critique, "reviewed_at"),
                s(critique, "reviewed_by"),
                if open.is_empty() {
                    "none".to_string()
                } else {
                    open.join(", ")
                }
            ));
        } else {
            out.push("  critique    none recorded".into());
        }
        let gap = &i["gap"];
        if gap.is_object() {
            let conditions: Vec<String> = gap["conditions"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| format!("{} {}", s(c, "criterion"), s(c, "state_text")))
                .collect();
            out.push(format!(
                "  gap         observed at {}: {}",
                s(gap, "observed_at"),
                conditions.join(", ")
            ));
        }
    }
    for g in words(&v["governance"]) {
        out.push(format!("governance  {g}"));
    }
    for r in v["refusals"].as_array().into_iter().flatten() {
        out.push(format!(
            "refusal     {}  {}",
            s(r, "cause"),
            s(r, "message")
        ));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    #[test]
    fn each_verb_reaches_its_capability_and_an_unexposed_one_is_named() {
        let repo = SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();

        // a repository with no intent answers the list, and validates
        let list = call(&ctx, &["intent", "list"], json!({})).unwrap();
        assert_eq!(list["count"], 0);
        // the JSON the command prints is byte for byte serde_json's pretty rendering
        assert_eq!(
            format!("{list:#}"),
            serde_json::to_string_pretty(&list).unwrap()
        );
        let valid = call(&ctx, &["intent", "validate"], json!({})).unwrap();
        assert_eq!(valid["valid"], true);

        match call(&ctx, &["intent", "absent"], json!({})) {
            Err(Error::Protocol { reason }) => assert_eq!(
                reason,
                "no capability is exposed as `majordomus intent absent`"
            ),
            other => panic!("an unexposed verb answered: {other:?}"),
        }
        // the capability's own not-found is the command line's not-found, exit 12
        match call(&ctx, &["intent", "show"], json!({ "id": "absent" })) {
            Err(e @ Error::NotFound { .. }) => {
                assert_eq!(e.exit_code(), 12);
                assert!(e.to_string().contains("no intent 'absent'"), "{e}");
            }
            other => panic!("an absent intent answered: {other:?}"),
        }
        // every other refusal is a protocol error carrying the capability's message
        match call(&ctx, &["intent", "preflight"], json!({ "paths": "" })) {
            Err(Error::Protocol { reason }) => {
                assert!(reason.contains("name the issue"), "{reason}")
            }
            other => panic!("a preflight of nothing answered: {other:?}"),
        }
    }

    #[test]
    fn list_and_show_print_the_verdict_beside_the_stage() {
        let intent = json!({
            "id": "probe", "title": "Probe", "statement": "True.", "stage": "executing",
            "source": "s.yaml", "met": 1,
            "satisfaction": [{"id": "a"}, {"id": "b"}],
            "verdict": {"state": "unknown",
                        "reasons": [{"criterion": "b", "evidence": "command",
                                     "state": "not_derivable"}]},
        });
        let list = list_text(&json!({ "count": 1, "intents": [intent] }));
        assert_eq!(
            list.lines().take(2).collect::<Vec<_>>(),
            [
                "ID     STAGE       VERDICT      MET      TITLE",
                "probe  executing   unknown      1/2      Probe",
            ]
        );
        let show = show_text(&intent);
        assert!(
            show.contains("  stage executing   verdict unknown   source s.yaml"),
            "{show}"
        );
        assert!(
            show.contains("  held back   b  command not_derivable"),
            "{show}"
        );
    }

    #[test]
    fn realization_text_separates_work_from_intents_only_when_there_is_work() {
        let intent = json!({ "intent": "I1", "stage": "proposed", "met": 0, "criteria": 1,
                             "title": "Ship it" });
        let idle = json!({ "intents": [intent], "work": [], "findings": [], "orphans": 0 });
        assert_eq!(
            realization_text(&idle),
            "I1  proposed  0/1 met  Ship it\n\
             1 intent(s), 0 unit(s) of work, 0 serving no intent"
        );

        let busy = json!({
            "intents": [intent],
            "work": [{
                "work": { "kind": "task", "id": "t-1", "outcome": "active" },
                "links": [],
                "unlinked": "names no issue",
            }],
            "findings": [],
            "orphans": 1,
        });
        assert_eq!(
            realization_text(&busy),
            "I1  proposed  0/1 met  Ship it\n\
             \n\
             task t-1  active  unlinked: names no issue\n\
             1 intent(s), 1 unit(s) of work, 1 serving no intent"
        );
    }
}
