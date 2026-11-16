//! `majordomus proceed`: what a session proceeds with, on the command line (ADR 0177).
//!
//! `proceed plan`: the capability decides; this renders. `proceed.plan` asks the server
//! holding this checkout's lease, so the claims a person reads here are that server's board
//! and mesh and not the empty ones of this process.
//!
//! `proceed step`: one step as one idempotent work order (I4005). The order's `do` names the
//! capability that acts, `devtask.implement`, whose execution takes the issue's mesh claim
//! and releases it when the run ends (I4003, I4004); proceed itself claims, creates and
//! starts nothing. `proceed take` (I2974), a second composition of claim, worktree and start
//! that held a caller-side claim and a record of its own, is retired by the owner's decision
//! of 2026-11-02 (ADR 0177, decision 3 amended): one path acts, and it is the execution's.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::Context;
use crate::cli::{OutputFormat, ProceedArgs, ProceedCommand};
use crate::error::{Error, Result};

/// The exit code when nothing can be selected: the code `dashboard overview` and `quality
/// report` use for an answer that is not satisfied.
pub const EXIT_NOTHING_SELECTED: u8 = 10;
/// The exit code when an issue is named but the decision is provisional: a source it needs
/// (the board, or a declared mesh) could not be read, so nothing should be taken on it.
pub const EXIT_PROVISIONAL: u8 = 11;

/// Run `majordomus proceed`.
pub fn run(args: ProceedArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    let out = &mut stdout.lock();
    match &args.command {
        ProceedCommand::Plan { milestone, session } => answer(
            &app.context,
            milestone.as_ref(),
            session.as_ref(),
            args.format,
            out,
        ),
        ProceedCommand::Step {
            intent,
            milestone,
            session,
            at,
        } => step(
            &app.context,
            [
                ("intent", intent.as_ref()),
                ("milestone", milestone.as_ref()),
                ("session", session.as_ref()),
                ("at", at.as_ref()),
            ],
            args.format,
            out,
        ),
    }
}

/// Ask the capability exposed as `majordomus proceed plan` for `milestone` and `session`, and
/// write its answer to `out` in `format`.
fn answer(
    ctx: &Context,
    milestone: Option<&String>,
    session: Option<&String>,
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<u8> {
    let id = ctx
        .registry
        .by_cli(&["proceed".to_string(), "plan".to_string()])
        .map(|c| c.id.as_str().to_string())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus proceed plan`".into(),
        })?;
    let mut input = json!({});
    if let Some(m) = milestone {
        input["milestone"] = json!(m);
    }
    if let Some(s) = session {
        input["session"] = json!(s);
    }
    let v = ctx.execute(&id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    let text = if format == OutputFormat::Json {
        format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
    } else {
        render(&v)
    };
    out.write_all(text.as_bytes()).map_err(Error::Transport)?;
    Ok(exit_for(&v))
}

/// Ask the capability exposed as `majordomus proceed step` with the fields `asked` names, each
/// sent when given, and write its work order to `out` in `format`.
fn step(
    ctx: &Context,
    asked: [(&str, Option<&String>); 4],
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<u8> {
    let id = ctx
        .registry
        .by_cli(&["proceed".to_string(), "step".to_string()])
        .map(|c| c.id.as_str().to_string())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus proceed step`".into(),
        })?;
    let mut input = json!({});
    for (k, v) in asked {
        if let Some(v) = v {
            input[k] = json!(v);
        }
    }
    let v = ctx.execute(&id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    let text = if format == OutputFormat::Json {
        format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
    } else {
        render_order(&v)
    };
    out.write_all(text.as_bytes()).map_err(Error::Transport)?;
    Ok(order_exit(&v))
}

/// A step that waits is an answer that is not satisfied; work ordered, or nothing left, is.
fn order_exit(v: &Value) -> u8 {
    if v["outcome"] == json!("wait") {
        EXIT_NOTHING_SELECTED
    } else {
        0
    }
}

/// The work order as a person reads it: what to do, beside what, what to say and when next.
fn render_order(v: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let item = |w: &Value| {
        let mut line = format!(
            "{} {}  {}: {}",
            s(&w["capability"]),
            serde_json::to_string(&w["input"]).unwrap_or_default(),
            s(&w["issue"]),
            s(&w["title"])
        );
        let mut on = Vec::new();
        if let Some(m) = w["model"].as_str() {
            on.push(format!("model {m}"));
        }
        if let Some(e) = w["effort"].as_str() {
            on.push(format!("effort {e}"));
        }
        if !on.is_empty() {
            line.push_str(&format!(" ({})", on.join(", ")));
        }
        line
    };
    let mut out = format!("order     {}  {}\n", s(&v["key"]), s(&v["outcome"]));
    out.push_str(&format!("why       {}\n", s(&v["reason"])));
    if v["do"].is_object() {
        out.push_str(&format!("do        {}\n", item(&v["do"])));
    }
    for (n, w) in v["fan_out"].as_array().into_iter().flatten().enumerate() {
        let label = if n == 0 { "fan out" } else { "" };
        out.push_str(&format!("{label:<9} {}\n", item(w)));
    }
    out.push_str(&format!(
        "report    [{}] {}\n",
        s(&v["report"]["claim"]),
        s(&v["report"]["intent"])
    ));
    out.push_str(&format!(
        "next      {}\n",
        v["next_due_at"]
            .as_str()
            .map_or_else(|| "nothing scheduled".to_string(), |t| format!("at {t}"))
    ));
    let i = &v["intent"];
    if i.is_object() {
        let words =
            |v: &Value| -> Vec<String> { v.as_array().into_iter().flatten().map(s).collect() };
        out.push_str(&format!(
            "intent    {}  {}, {} of {} criteria met; unmet: {}; serving: {}\n",
            s(&i["intent"]),
            s(&i["stage"]),
            i["met"],
            i["criteria"],
            or_none(&words(&i["unmet"])),
            or_none(&words(&i["serving"]))
        ));
    }
    for (n, src) in v["sources"].as_array().into_iter().flatten().enumerate() {
        let label = if n == 0 { "sources" } else { "" };
        out.push_str(&format!(
            "{label:<9} {} {}: {}\n",
            s(&src["source"]),
            s(&src["state"]),
            s(&src["summary"])
        ));
    }
    out
}

fn or_none(words: &[String]) -> String {
    if words.is_empty() {
        "none".into()
    } else {
        words.join(", ")
    }
}

/// The exit code the decision's own words imply.
fn exit_for(v: &Value) -> u8 {
    if !v["selected"].is_object() {
        EXIT_NOTHING_SELECTED
    } else if v["provisional"] == json!(true) {
        EXIT_PROVISIONAL
    } else {
        0
    }
}

/// The decision as a person reads it: the selection and why, then what was passed over,
/// what waits and what the decision was read from.
fn render(v: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let list = |v: &Value| -> Vec<Value> { v.as_array().cloned().unwrap_or_default() };
    let mut out = String::new();
    let sel = &v["selected"];
    if sel.is_object() {
        let t = &sel["terms"];
        out.push_str(&format!(
            "selected  {}  {} ({}, wave {}, {})\n",
            s(&sel["issue"]),
            s(&sel["title"]),
            s(&sel["milestone"]),
            t["wave"],
            s(&t["priority"])
        ));
    } else {
        out.push_str("selected  nothing\n");
    }
    out.push_str(&format!("why       {}\n", s(&v["reason"])));
    if sel.is_object() {
        let unblocks: Vec<String> = list(&sel["terms"]["unblocks"]).iter().map(s).collect();
        out.push_str(&format!(
            "unblocks  {}\n",
            if unblocks.is_empty() {
                "nothing".to_string()
            } else {
                unblocks.join(", ")
            }
        ));
        let f = &sel["finishes_with"];
        out.push_str(&format!(
            "finishes  {} (evidence {} of {})\n",
            s(&f["objective"]),
            f["have"],
            f["need"]
        ));
        for (n, step) in list(&v["next"]).iter().enumerate() {
            let label = if n == 0 { "next" } else { "" };
            out.push_str(&format!("{label:<9} {}. {}\n", n + 1, s(step)));
        }
    }
    let mut first = true;
    for c in list(&v["candidates"]) {
        for r in list(&c["refused"]) {
            let label = if first { "passed" } else { "" };
            first = false;
            out.push_str(&format!(
                "{label:<9} {}  {} by {}: {}\n",
                s(&c["issue"]),
                s(&r["constraint"]),
                s(&r["holder"]),
                s(&r["detail"])
            ));
        }
    }
    if let Some(w) = v["waiting"].as_object().filter(|w| !w.is_empty()) {
        let parts: Vec<String> = w.iter().map(|(k, n)| format!("{k} {n}")).collect();
        out.push_str(&format!("waiting   {}\n", parts.join(", ")));
    }
    for (n, src) in list(&v["sources"]).iter().enumerate() {
        let label = if n == 0 { "sources" } else { "" };
        out.push_str(&format!(
            "{label:<9} {} {}: {}\n",
            s(&src["source"]),
            s(&src["state"]),
            s(&src["summary"])
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// `proceed plan` as the command line parsed it, asked of `ctx`.
    fn ask_plan(ctx: &Context, a: &ProceedArgs, out: &mut impl Write) -> Result<u8> {
        let ProceedCommand::Plan { milestone, session } = &a.command else {
            panic!("not proceed plan")
        };
        answer(ctx, milestone.as_ref(), session.as_ref(), a.format, out)
    }

    /// `proceed step` as the command line parsed it, asked of `ctx`.
    fn ask_step(ctx: &Context, a: &ProceedArgs, out: &mut impl Write) -> Result<u8> {
        let ProceedCommand::Step {
            intent,
            milestone,
            session,
            at,
        } = &a.command
        else {
            panic!("not proceed step")
        };
        step(
            ctx,
            [
                ("intent", intent.as_ref()),
                ("milestone", milestone.as_ref()),
                ("session", session.as_ref()),
                ("at", at.as_ref()),
            ],
            a.format,
            out,
        )
    }

    fn args(argv: &[&str]) -> ProceedArgs {
        let mut full = vec!["majordomus", "proceed"];
        full.extend_from_slice(argv);
        match crate::cli::Cli::try_parse_from(full)
            .expect("parses")
            .command
        {
            crate::cli::Command::Proceed(a) => a,
            _ => panic!("not the proceed command"),
        }
    }

    #[test]
    fn a_selection_renders_its_reason_steps_refusals_and_sources() {
        let v = json!({
            "selected": {
                "issue": "I0002", "title": "t", "milestone": "M",
                "terms": {"in_milestone": true, "wave": 1, "priority": "p1", "unblocks": ["I0003"]},
                "refused": [], "shared_with": [], "scope": ["src"],
                "finishes_with": {"objective": "o", "have": 0, "need": 1}
            },
            "reason": "I0002 is the first",
            "next": ["claim", "worktree"],
            "milestone": "M",
            "candidates": [{
                "issue": "I0001",
                "refused": [{"constraint": "claimed", "hold": "k", "holder": "p2", "detail": "names I0001"}]
            }],
            "waiting": {"BLOCKED": 2},
            "sources": [{"source": "board", "state": "read", "holds": 1, "summary": "1 board(s)"}]
        });
        let text = render(&v);
        for line in [
            "selected  I0002  t (M, wave 1, p1)",
            "why       I0002 is the first",
            "unblocks  I0003",
            "finishes  o (evidence 0 of 1)",
            "next      1. claim",
            "          2. worktree",
            "passed    I0001  claimed by p2: names I0001",
            "waiting   BLOCKED 2",
            "sources   board read: 1 board(s)",
        ] {
            assert!(text.contains(line), "missing {line:?} in\n{text}");
        }
        let none = render(&json!({"selected": null, "reason": "r", "sources": []}));
        assert_eq!(none, "selected  nothing\nwhy       r\n");
        let quiet = render(&json!({"selected": {"terms": {"unblocks": []}, "finishes_with": {}}}));
        assert!(quiet.contains("unblocks  nothing"), "{quiet}");
    }

    #[test]
    fn a_work_order_renders_what_to_do_beside_what_what_to_say_and_when() {
        let w = |issue: &str| {
            json!({"capability": "devtask.implement",
                   "input": {"issue": issue, "idempotency_key": format!("implement-{issue}")},
                   "issue": issue, "title": "t", "scope": ["s"], "profile": "implementation",
                   "effort": "medium", "model": "m-1"})
        };
        let v = json!({
            "key": "step-0123456789abcdef", "outcome": "implement", "done": false,
            "reason": "I1 is first", "do": w("I1"), "fan_out": [w("I2"), w("I3")],
            "report": {"claim": "proceed", "intent": "I1: t", "scope": ["s"]},
            "next_due_at": "2026-10-23T10:05:00Z",
            "intent": {"intent": "probe", "stage": "executing", "criteria": 2, "met": 1,
                       "unmet": ["b"], "serving": ["I1"]},
            "at": "2026-10-23T10:00:00Z",
            "sources": [{"source": "board", "state": "read", "holds": 0, "summary": "1 board(s)"}]
        });
        let text = render_order(&v);
        for line in [
            "order     step-0123456789abcdef  implement",
            "why       I1 is first",
            "do        devtask.implement {\"issue\":\"I1\",\"idempotency_key\":\"implement-I1\"}  I1: t (model m-1, effort medium)",
            "fan out   devtask.implement",
            "          devtask.implement {\"issue\":\"I3\"",
            "report    [proceed] I1: t",
            "next      at 2026-10-23T10:05:00Z",
            "intent    probe  executing, 1 of 2 criteria met; unmet: b; serving: I1",
            "sources   board read: 1 board(s)",
        ] {
            assert!(text.contains(line), "missing {line:?} in\n{text}");
        }
        assert_eq!(order_exit(&v), 0);
        let done = json!({"key": "step-1", "outcome": "done", "done": true, "reason": "met",
                          "do": null, "fan_out": [], "report": {"claim": "proceed", "intent": "met"},
                          "next_due_at": null, "intent": {"intent": "probe", "stage": "verifying",
                          "criteria": 1, "met": 1, "unmet": [], "serving": []}, "sources": []});
        let text = render_order(&done);
        assert!(text.contains("next      nothing scheduled"), "{text}");
        assert!(text.contains("unmet: none; serving: none"), "{text}");
        assert!(!text.contains("do        "), "{text}");
        assert_eq!(order_exit(&done), 0);
        assert_eq!(
            order_exit(&json!({"outcome": "wait"})),
            EXIT_NOTHING_SELECTED
        );
        // an item that names no model or effort says nothing about them
        let bare = render_order(
            &json!({"do": {"capability": "c", "input": {}, "issue": "I", "title": "t"}}),
        );
        assert!(bare.contains("do        c {}  I: t\n"), "{bare}");
    }

    #[test]
    fn the_step_command_answers_with_the_capability_twice_the_same() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let run = || {
            let mut out = Vec::new();
            let code = ask_step(
                &ctx,
                &args(&["step", "--at", "2026-10-23T10:00:00Z", "--format", "json"]),
                &mut out,
            )
            .unwrap();
            (code, String::from_utf8(out).unwrap())
        };
        let (code, first) = run();
        assert_eq!(
            run(),
            (code, first.clone()),
            "the same state, the same order"
        );
        let v: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(v["at"], "2026-10-23T10:00:00Z");
        assert_eq!(code, order_exit(&v));
        let mut out = Vec::new();
        let err = ask_step(&ctx, &args(&["step", "--at", "noon"]), &mut out).unwrap_err();
        assert!(err.to_string().contains("RFC 3339"), "{err}");
        let mut out = Vec::new();
        ask_step(
            &ctx,
            &args(&[
                "step",
                "--intent",
                "x",
                "--milestone",
                "M",
                "--session",
                "s",
            ]),
            &mut out,
        )
        .unwrap_err();
    }

    #[test]
    fn the_exit_code_is_the_decisions_own_word() {
        assert_eq!(exit_for(&json!({"selected": null})), EXIT_NOTHING_SELECTED);
        let sel = json!({"issue": "I1"});
        assert_eq!(
            exit_for(&json!({"selected": sel, "provisional": true})),
            EXIT_PROVISIONAL
        );
        assert_eq!(exit_for(&json!({"selected": sel, "provisional": false})), 0);
    }

    #[test]
    fn the_command_answers_with_the_capability_and_exits_10_on_no_selection() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let mut out = Vec::new();
        let code = ask_plan(
            &ctx,
            &args(&[
                "plan",
                "--milestone",
                "M",
                "--session",
                "s",
                "--format",
                "json",
            ]),
            &mut out,
        )
        .unwrap();
        assert_eq!(code, EXIT_NOTHING_SELECTED);
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["milestone"], "M");
        assert!(v["selected"].is_null());
        let mut out = Vec::new();
        ask_plan(&ctx, &args(&["plan"]), &mut out).unwrap();
        assert!(String::from_utf8(out)
            .unwrap()
            .starts_with("selected  nothing\n"));
    }

    /// A terminal that cannot be written to: the answer was composed and could not be said.
    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("the terminal is gone"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_step_renders_as_text_and_an_answer_that_cannot_be_written_is_an_error() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let mut out = Vec::new();
        ask_step(
            &ctx,
            &args(&["step", "--at", "2026-10-23T10:00:00Z"]),
            &mut out,
        )
        .unwrap();
        assert!(
            String::from_utf8(out)
                .unwrap()
                .starts_with("order     step-"),
            "the text rendering"
        );
        assert!(ask_step(
            &ctx,
            &args(&["step", "--at", "2026-10-23T10:00:00Z"]),
            &mut Closed
        )
        .is_err());
        assert!(ask_plan(&ctx, &args(&["plan"]), &mut Closed).is_err());
    }

    #[test]
    fn a_registry_without_the_command_or_whose_capability_refuses_is_an_error() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let bare = Context::new(
            std::sync::Arc::new(repo.index().unwrap()),
            std::sync::Arc::new(
                crate::capability::CapabilityRegistry::builder()
                    .build()
                    .expect("an empty registry"),
            ),
        );
        let mut out = Vec::new();
        let err = ask_plan(&bare, &args(&["plan"]), &mut out).unwrap_err();
        assert!(
            err.to_string()
                .contains("no capability is exposed as `majordomus proceed plan`"),
            "{err}"
        );
        let err = ask_step(&bare, &args(&["step"]), &mut out).unwrap_err();
        assert!(
            err.to_string()
                .contains("no capability is exposed as `majordomus proceed step`"),
            "{err}"
        );

        let refusing = Context::new(
            std::sync::Arc::new(repo.index().unwrap()),
            std::sync::Arc::new(
                crate::capability::CapabilityRegistry::builder()
                    .with_builtin(vec![crate::capability! {
                        id: "proceed.plan", title: "Plan", description: "Refuses.",
                        input: Anything, output: Value,
                        stability: crate::capability::model::Stability::Experimental,
                        exposure: crate::capability::model::Exposure {
                            cli: Some(crate::capability::model::CliExposure {
                                path: vec!["proceed".into(), "plan".into()],
                            }),
                            ..Default::default()
                        },
                        tags: [],
                        handler: refuse,
                    }])
                    .build()
                    .expect("a registry"),
            ),
        );
        let err = ask_plan(&refusing, &args(&["plan"]), &mut out).unwrap_err();
        assert!(err.to_string().contains("not today"), "{err}");
        assert!(out.is_empty(), "nothing was written");
    }

    #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct Anything {}

    impl crate::capability::benchmark::BenchmarkCases for Anything {
        fn benchmark_cases(
            _: &crate::capability::benchmark::CaseContext<'_>,
        ) -> Vec<crate::capability::benchmark::NamedCase<Self>> {
            vec![crate::capability::benchmark::NamedCase::new(
                "default",
                Anything {},
            )]
        }
    }

    fn refuse(
        _: &Context,
        _: Anything,
    ) -> std::result::Result<Value, crate::capability::CapabilityError> {
        Err(crate::capability::CapabilityError::Refused(
            "not today".into(),
        ))
    }

    #[test]
    fn a_command_outside_a_repository_is_an_error() {
        assert!(run(args(&["plan", "--repo", "/nonexistent/checkout"])).is_err());
    }

    #[test]
    fn every_source_and_every_refusal_after_the_first_is_a_line_of_its_own() {
        let v = json!({
            "selected": null,
            "reason": "r",
            "candidates": [{
                "issue": "I0001",
                "refused": [
                    {"constraint": "claimed", "holder": "p2", "detail": "names I0001"},
                    {"constraint": "scope_held", "holder": "p3", "detail": "holds src"}
                ]
            }],
            "sources": [
                {"source": "board", "state": "read", "summary": "1 board(s)"},
                {"source": "mesh", "state": "undeclared", "summary": "no mesh"}
            ]
        });
        let text = render(&v);
        assert!(
            text.contains("          I0001  scope_held by p3: holds src\n"),
            "{text}"
        );
        assert!(
            text.contains("          mesh undeclared: no mesh\n"),
            "{text}"
        );
        let order = render_order(&json!({
            "sources": [
                {"source": "board", "state": "read", "summary": "1 board(s)"},
                {"source": "mesh", "state": "undeclared", "summary": "no mesh"}
            ]
        }));
        assert!(
            order.contains("          mesh undeclared: no mesh\n"),
            "{order}"
        );
    }
}
