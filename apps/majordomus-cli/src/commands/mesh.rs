//! `majordomus mesh`: the terminal rendering of the mesh capabilities.
//!
//! `identity` and `doctor` answer in-process — the identity file and the machine's
//! sockets are facts of this machine, so the local registry's own handlers are
//! truthful. Everything else is a fact of the *running server*'s memory — the registry,
//! the links, the journal — so the command finds this checkout's server (through the same
//! `server.status` capability `serve status` renders) and asks it over HTTP; a missing
//! server is an answer with its reason, never an error. The command owns rendering and
//! nothing else: every value it prints is a capability's answer, and `--format json`
//! prints that answer unchanged for scripts.
//!
//! Exit codes: 0 answered; 10 answered with a refusal or a failed verdict (a claim
//! conflict, an unknown runtime, a failed verification) — the answer is printed either way.

use std::time::Duration;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{
    MeshArgs, MeshCommand, MeshHandoverCommand, MeshQueryArgs, MeshReviewCommand,
    MeshSessionCommand, OutputFormat,
};
use crate::error::{Error, Result};

/// One bounded request to the running server. A verification runs a round with every
/// dialed peer, each bounded by the link timeout, so it gets longer.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const VERIFY_TIMEOUT: Duration = Duration::from_secs(60);

/// The exit code of an answered refusal or failed verdict.
const EXIT_REFUSED: u8 = 10;

/// Run `majordomus mesh`.
pub fn run(args: MeshArgs) -> Result<u8> {
    match args.command {
        MeshCommand::Status(q) => ask(&q, "GET", "/api/v1/mesh", None, render_status, never),
        MeshCommand::Nodes(q) => ask(&q, "GET", "/api/v1/mesh/nodes", None, render_nodes, never),
        MeshCommand::Identity(q) => in_process(q, &["mesh", "identity"], render_identity),
        MeshCommand::Doctor(q) => in_process(q, &["mesh", "doctor"], render_doctor),
        MeshCommand::Peers(q) => ask(&q, "GET", "/api/v1/mesh/peers", None, render_peers, never),
        MeshCommand::Peer(a) => {
            let target = format!("/api/v1/mesh/peer?runtime={}", encode(&a.runtime));
            ask(&a.query, "GET", &target, None, render_peer, |v| {
                v["found"] != json!(true)
            })
        }
        MeshCommand::State(q) => ask(&q, "GET", "/api/v1/mesh/state", None, render_state, never),
        MeshCommand::Events(a) => {
            let mut target = "/api/v1/mesh/events".to_string();
            let mut pairs = Vec::new();
            if let Some(after) = a.after {
                pairs.push(format!("after={after}"));
            }
            if let Some(limit) = a.limit {
                pairs.push(format!("limit={limit}"));
            }
            if !pairs.is_empty() {
                target = format!("{target}?{}", pairs.join("&"));
            }
            ask(&a.query, "GET", &target, None, render_events, never)
        }
        MeshCommand::Verify(q) => ask(
            &q,
            "POST",
            "/api/v1/mesh/verify",
            Some(json!({})),
            render_verify,
            |v| v["ok"] != json!(true),
        ),
        MeshCommand::Claim(a) => ask(
            &a.query,
            "POST",
            "/api/v1/mesh/claims",
            Some(json!({
                "session": a.session,
                "scope": a.scope,
                "intent": a.intent,
                "mode": if a.advisory { "advisory" } else { "exclusive" },
                "issue": a.issue,
                "task": a.task,
            })),
            render_written,
            never,
        ),
        MeshCommand::Release(a) => ask(
            &a.query,
            "POST",
            "/api/v1/mesh/claims/release",
            Some(json!({ "claim": a.claim })),
            render_written,
            never,
        ),
        MeshCommand::Session(s) => match s.command {
            MeshSessionCommand::Open(a) => ask(
                &a.query,
                "POST",
                "/api/v1/mesh/sessions",
                Some(json!({
                    "session": a.session,
                    "client": a.client,
                    "worker": a.worker,
                    "intent": a.intent,
                    "task": a.task,
                    "issue": a.issue,
                    "milestone": a.milestone,
                    "branch": a.branch,
                    "head": a.head,
                })),
                render_written,
                never,
            ),
            MeshSessionCommand::Close(a) => ask(
                &a.query,
                "POST",
                "/api/v1/mesh/sessions/close",
                Some(json!({ "session": a.session })),
                render_written,
                never,
            ),
        },
        MeshCommand::Handover(h) => match h.command {
            MeshHandoverCommand::Publish(a) => ask(
                &a.query,
                "POST",
                "/api/v1/mesh/handovers",
                Some(json!({ "path": a.path, "issue": a.issue, "milestone": a.milestone })),
                render_written,
                never,
            ),
            MeshHandoverCommand::Consume(a) => ask(
                &a.query,
                "POST",
                "/api/v1/mesh/handovers/consume",
                Some(json!({
                    "handover": a.handover,
                    "session": a.session,
                    "materialize": !a.no_materialize,
                })),
                render_consumed,
                never,
            ),
        },
        MeshCommand::Review(r) => match r.command {
            MeshReviewCommand::Request(a) => ask(
                &a.query,
                "POST",
                "/api/v1/mesh/reviews",
                Some(json!({
                    "session": a.session,
                    "subject": a.subject,
                    "scope": a.scope,
                    "issue": a.issue,
                    "reviewer": a.reviewer,
                })),
                render_written,
                never,
            ),
            MeshReviewCommand::Answer(a) => ask(
                &a.query,
                "POST",
                "/api/v1/mesh/reviews/answer",
                Some(json!({
                    "request": a.request,
                    "session": a.session,
                    "verdict": a.verdict,
                    "note": a.note,
                })),
                render_written,
                never,
            ),
        },
    }
}

fn never(_: &Value) -> bool {
    false
}

/// Percent-encode a query value: everything but unreserved characters.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Execute the capability the CLI path names, in this process, and render it.
fn in_process(args: MeshQueryArgs, path: &[&str], render: fn(&Value) -> String) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    let value = ctx.execute(id, json!({})).map_err(map)?;
    emit(&args, &value, render);
    Ok(0)
}

/// This checkout's ready server, when there is one.
fn server_url(args: &MeshQueryArgs) -> Result<Option<String>> {
    let app = App::load(&args.repo)?;
    let status = app
        .context
        .execute("server.status", json!({}))
        .map_err(map)?;
    Ok(status["servers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["this_checkout"] == json!(true) && s["standing"] == json!("ready"))
        .and_then(|s| s["lease"]["url"].as_str())
        .map(str::to_string))
}

/// Ask this checkout's running server, and render the answer. No running server is an
/// answer for a question and a refusal for a command: the mesh lives in the server.
fn ask(
    args: &MeshQueryArgs,
    method: &str,
    target: &str,
    body: Option<Value>,
    render: fn(&Value) -> String,
    failed: fn(&Value) -> bool,
) -> Result<u8> {
    let Some(url) = server_url(args)? else {
        let answer = json!({
            "active": false,
            "reason": "no running server for this checkout: the mesh lives inside the shared server (majordomus serve ensure starts one)",
        });
        emit(args, &answer, render);
        return Ok(if method == "GET" { 0 } else { EXIT_REFUSED });
    };
    let timeout = if target.ends_with("/verify") {
        VERIFY_TIMEOUT
    } else {
        REQUEST_TIMEOUT
    };
    let payload = body.map(|b| strip_nulls(b).to_string());
    let reply = crate::mcp::bridge::request(
        &url,
        method,
        target,
        &[("Content-Type", "application/json")],
        payload.as_deref(),
        timeout,
    )
    .map_err(|e| Error::Protocol {
        reason: format!("{url}{target}: {e}"),
    })?;
    let value: Value = serde_json::from_str(&reply.body).map_err(|e| Error::Protocol {
        reason: format!("{url}{target}: status {}, not JSON: {e}", reply.status),
    })?;
    if reply.status != 200 {
        // A typed refusal from the capability: printed, and exit 10.
        if let Some(error) = value.get("error") {
            match args.format {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                ),
                OutputFormat::Text => eprintln!(
                    "mesh: {}: {}",
                    error["code"].as_str().unwrap_or("error"),
                    error["message"].as_str().unwrap_or("")
                ),
            }
            return Ok(EXIT_REFUSED);
        }
        return Err(Error::Protocol {
            reason: format!("{url}{target}: status {}", reply.status),
        });
    }
    emit(args, &value, render);
    Ok(if failed(&value) { EXIT_REFUSED } else { 0 })
}

/// Drop `null` members, so an omitted option is absent rather than null in the input.
fn strip_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k, strip_nulls(v)))
                .collect(),
        ),
        other => other,
    }
}

fn emit(args: &MeshQueryArgs, value: &Value, render: fn(&Value) -> String) {
    match args.format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).unwrap_or_default()
            )
        }
        OutputFormat::Text => println!("{}", render(value)),
    }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

fn inactive(v: &Value) -> Option<String> {
    (v["active"] == json!(false)).then(|| {
        format!(
            "cooperation inactive\nreason     {}",
            v["reason"].as_str().unwrap_or("unknown")
        )
    })
}

fn render_status(v: &Value) -> String {
    let mut out = String::new();
    let active = v["active"].as_bool().unwrap_or(false);
    out.push_str(&format!(
        "mesh       {}\n",
        if active { "active" } else { "inactive" }
    ));
    if let Some(reason) = v["reason"].as_str() {
        out.push_str(&format!("reason     {reason}\n"));
    }
    if let Some(id) = v["identity"]["node_id"].as_str() {
        out.push_str(&format!(
            "node       {id} ({})\n",
            v["identity"]["display_name"].as_str().unwrap_or("")
        ));
    }
    if let Some(policy) = v["trust_policy"].as_str() {
        out.push_str(&format!("trust      {policy}\n"));
    }
    for p in v["providers"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "provider   {} {} sent {} received {}{}\n",
            p["id"].as_str().unwrap_or("?"),
            p["state"].as_str().unwrap_or("?"),
            p["sent"],
            p["received"],
            p["detail"]
                .as_str()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default(),
        ));
    }
    let t = &v["tallies"];
    out.push_str(&format!(
        "nodes      {} ({} trusted, {} present); accepted {}, replayed {}, expired {}",
        t["nodes"], t["trusted"], t["present"], t["accepted"], t["replayed"], t["expired"]
    ));
    out
}

fn render_nodes(v: &Value) -> String {
    if v["active"] == json!(false) {
        return render_status(v);
    }
    let nodes = v["nodes"].as_array().cloned().unwrap_or_default();
    if nodes.is_empty() {
        return "mesh       0 node(s) observed".into();
    }
    let mut out = format!("mesh       {} node(s)\n", nodes.len());
    for n in &nodes {
        let sources: Vec<&str> = n["sources"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s["source"].as_str())
            .collect();
        out.push_str(&format!(
            "{}-{}  {}  trust {}  {}  [{}]  last seen {}\n",
            n["node_id"].as_str().unwrap_or("?"),
            n["runtime"].as_str().unwrap_or(""),
            n["display_name"].as_str().unwrap_or(""),
            n["trust"]["state"].as_str().unwrap_or("?"),
            n["presence"].as_str().unwrap_or("?"),
            sources.join(","),
            n["last_seen"].as_str().unwrap_or("?"),
        ));
    }
    out.trim_end().to_string()
}

fn render_identity(v: &Value) -> String {
    let mut out = String::new();
    out.push_str(&format!("present    {}\n", v["present"]));
    if let Some(path) = v["path"].as_str() {
        out.push_str(&format!("path       {path}\n"));
    }
    if let Some(id) = v["identity"]["node_id"].as_str() {
        out.push_str(&format!("node       {id}\n"));
        out.push_str(&format!(
            "name       {}\n",
            v["identity"]["display_name"].as_str().unwrap_or("")
        ));
        out.push_str(&format!(
            "key        {}\n",
            v["identity"]["public_key"].as_str().unwrap_or("")
        ));
    }
    if let Some(error) = v["error"].as_str() {
        out.push_str(&format!("error      {error}\n"));
    }
    out.trim_end().to_string()
}

fn render_checks(v: &Value, out: &mut String) {
    for c in v["checks"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{}  {:<12} {}\n",
            if c["ok"] == json!(true) {
                "ok  "
            } else {
                "FAIL"
            },
            c["check"].as_str().unwrap_or("?"),
            c["detail"].as_str().unwrap_or(""),
        ));
        if let Some(impact) = c["impact"].as_str() {
            out.push_str(&format!("      impact      {impact}\n"));
        }
        if let Some(remedy) = c["remediation"].as_str() {
            out.push_str(&format!("      remedy      {remedy}\n"));
        }
    }
}

fn render_doctor(v: &Value) -> String {
    let mut out = String::new();
    render_checks(v, &mut out);
    out.push_str(&format!(
        "verdict    {}",
        if v["ok"] == json!(true) {
            "every check holds"
        } else {
            "a check failed (see above)"
        }
    ));
    out
}

fn short(key: &str) -> String {
    match key.split_once('-') {
        Some((node, runtime)) => format!("{}…-{runtime}", &node[..node.len().min(8)]),
        None => key.to_string(),
    }
}

fn render_session(sess: &Value, indent: &str, out: &mut String) {
    let info = &sess["info"];
    out.push_str(&format!(
        "{indent}session  {} {} [{}]{}{}\n",
        s(info, "client"),
        s(sess, "state"),
        s(info, "session"),
        info["intent"]
            .as_str()
            .map(|i| format!(" — {i}"))
            .unwrap_or_default(),
        info["issue"]
            .as_str()
            .map(|i| format!(" ({i})"))
            .unwrap_or_default(),
    ));
    for c in sess["claims"].as_array().into_iter().flatten() {
        let scope: Vec<&str> = c["scope"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        out.push_str(&format!(
            "{indent}  claim  {} {} {}{}\n",
            scope.join(","),
            s(c, "mode"),
            c["state"]["state"].as_str().unwrap_or("?"),
            c["state"]["detail"]
                .as_str()
                .map(|d| format!(" ({d})"))
                .unwrap_or_default(),
        ));
    }
}

fn render_peers(v: &Value) -> String {
    if let Some(text) = inactive(v) {
        return text;
    }
    let mut out = String::new();
    out.push_str(&format!("runtime    {}\n", s(v, "runtime")));
    out.push_str(&format!("repository {}\n", s(v, "repository")));
    for m in v["machines"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "machine    {} {} ({})\n",
            s(m, "name"),
            s(m, "node"),
            if m["local"] == json!(true) {
                "local"
            } else {
                "remote"
            }
        ));
        for r in m["runtimes"].as_array().into_iter().flatten() {
            let link = &r["link"];
            out.push_str(&format!(
                "  runtime  {} {}{}{}{}\n",
                short(s(r, "runtime")),
                s(r, "liveness"),
                if link.is_object() {
                    format!(" link {}", s(link, "state"))
                } else {
                    String::new()
                },
                r["last_beat_ms"]
                    .as_u64()
                    .map(|ms| format!(" beat {:.1}s ago", ms as f64 / 1000.0))
                    .unwrap_or_default(),
                link["rtt_ms"]
                    .as_u64()
                    .map(|ms| format!(" rtt {ms}ms"))
                    .unwrap_or_default(),
            ));
            for sess in r["sessions"].as_array().into_iter().flatten() {
                render_session(sess, "    ", &mut out);
            }
        }
    }
    for r in v["refused"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "refused    {} {} {}: {}\n",
            s(r, "endpoint"),
            s(r, "direction"),
            s(&r["refusal"], "code"),
            s(&r["refusal"], "detail")
        ));
    }
    out.push_str(&format!("digest     {}", s(v, "digest")));
    out
}

fn render_peer(v: &Value) -> String {
    if let Some(text) = inactive(v) {
        return text;
    }
    if v["found"] != json!(true) {
        return format!("not found  {}", v["reason"].as_str().unwrap_or(""));
    }
    let r = &v["runtime"];
    let mut out = format!(
        "machine    {}\nruntime    {}\nliveness   {}\n",
        s(v, "machine"),
        s(r, "runtime"),
        s(r, "liveness")
    );
    let link = &r["link"];
    if link.is_object() {
        out.push_str(&format!(
            "link       {} (out {}, in {}) protocol {} handshakes {} reconnects {} restarts {} failures {}\n",
            s(link, "state"),
            link["outbound"],
            link["inbound"],
            link["protocol"],
            link["handshakes"],
            link["reconnects"],
            link["restarts"],
            link["failures"]
        ));
    }
    for sess in r["sessions"].as_array().into_iter().flatten() {
        render_session(sess, "", &mut out);
    }
    out.trim_end().to_string()
}

fn render_state(v: &Value) -> String {
    if let Some(text) = inactive(v) {
        return text;
    }
    let st = &v["state"];
    let count = |k: &str| st[k].as_array().map(Vec::len).unwrap_or(0);
    let mut out = format!(
        "sessions   {}\nclaims     {}\noverlaps   {}\nhandovers  {}\nreviews    {}\n",
        count("sessions"),
        count("claims"),
        count("overlaps"),
        count("handovers"),
        count("reviews")
    );
    for c in st["claims"].as_array().into_iter().flatten() {
        let scope: Vec<&str> = c["scope"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        out.push_str(&format!(
            "claim      {} {} {} {}{}\n",
            s(c, "key"),
            scope.join(","),
            s(c, "mode"),
            c["state"]["state"].as_str().unwrap_or("?"),
            c["state"]["detail"]
                .as_str()
                .map(|d| format!(" ({d})"))
                .unwrap_or_default(),
        ));
    }
    for h in st["handovers"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "handover   {} from {} branch {} consumed by {}\n",
            s(h, "id"),
            short(s(h, "runtime")),
            h["handover"]["branch"].as_str().unwrap_or("-"),
            h["consumed_by"].as_array().map(Vec::len).unwrap_or(0)
        ));
    }
    for r in st["reviews"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "review     {} {} {} ({} answer(s))\n",
            s(r, "key"),
            s(r, "subject"),
            s(r, "state"),
            r["answers"].as_array().map(Vec::len).unwrap_or(0)
        ));
    }
    out.push_str(&format!("digest     {}", s(st, "digest")));
    out
}

fn render_events(v: &Value) -> String {
    if let Some(text) = inactive(v) {
        return text;
    }
    let mut out = String::new();
    for e in v["events"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{:>6}  {}/{}  {}\n",
            e["lamport"],
            short(&s(e, "stream").chars().take(49).collect::<String>()),
            e["seq"],
            e["body"]["kind"].as_str().unwrap_or("?")
        ));
    }
    out.push_str(&format!(
        "events     {} (lamport {})",
        v["count"], v["lamport"]
    ));
    out
}

fn render_verify(v: &Value) -> String {
    let mut out = String::new();
    if let Some(runtime) = v["runtime"].as_str() {
        out.push_str(&format!("runtime    {runtime}\n"));
    }
    render_checks(v, &mut out);
    for p in v["peers"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{}  peer {} {} {}{}{} — {}\n",
            if p["round_trip"] == json!(false) || p["state"] == json!("expired") {
                "FAIL"
            } else {
                "ok  "
            },
            short(s(p, "runtime")),
            s(p, "state"),
            if p["dialed"] == json!(true) {
                "dialed"
            } else {
                "dials us"
            },
            p["rtt_ms"]
                .as_u64()
                .map(|ms| format!(" rtt {ms}ms"))
                .unwrap_or_default(),
            p["converged"]
                .as_bool()
                .map(|c| if c { " converged" } else { " NOT converged" })
                .unwrap_or_default(),
            s(p, "detail"),
        ));
    }
    out.push_str(&format!(
        "verdict    {}",
        if v["ok"] == json!(true) {
            "cooperation verified"
        } else {
            "a check or a round failed (see above)"
        }
    ));
    out
}

fn render_written(v: &Value) -> String {
    if let Some(text) = inactive(v) {
        return text;
    }
    format!(
        "written    {}\nevent      {} (lamport {})",
        s(v, "key"),
        s(v, "event"),
        v["lamport"]
    )
}

fn render_consumed(v: &Value) -> String {
    if let Some(text) = inactive(v) {
        return text;
    }
    let mut out = format!(
        "handover   {}\nfrom       {}\nevent      {}\n",
        s(&v["handover"], "id"),
        s(&v["handover"], "runtime"),
        s(&v["written"], "event")
    );
    if let Some(path) = v["path"].as_str() {
        out.push_str(&format!("record     {path}\n"));
    }
    out.push_str(&format!(
        "branch     {}",
        v["handover"]["handover"]["branch"].as_str().unwrap_or("-")
    ));
    out
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 32-hex node key, as `<node>-<runtime>` carries it.
    const NODE_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const NODE_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn runtime_key(node: &str, runtime: &str) -> String {
        format!("{node}-{runtime}")
    }

    /// A renderer, as the command holds one.
    type Renderer = fn(&Value) -> String;

    /// Every renderer that carries the `inactive` guard, by the name a reader of the
    /// failure would look for.
    fn guarded() -> Vec<(&'static str, Renderer)> {
        vec![
            ("peers", render_peers as Renderer),
            ("peer", render_peer),
            ("state", render_state),
            ("events", render_events),
            ("written", render_written),
            ("consumed", render_consumed),
        ]
    }

    #[test]
    fn a_runtime_without_cooperation_answers_every_question_with_the_reason_it_is_off() {
        let answer = json!({
            "active": false,
            "reason": "no running server for this checkout: the mesh lives inside the shared server",
        });
        for (name, render) in guarded() {
            let text = render(&answer);
            assert!(
                text.starts_with("cooperation inactive"),
                "`mesh {name}` must say the mesh is off before anything else, got: {text}"
            );
            assert!(
                text.contains("the mesh lives inside the shared server"),
                "`mesh {name}` must carry the reason, not only the verdict, got: {text}"
            );
            assert!(
                !text.contains("digest"),
                "`mesh {name}` must render nothing of a state it does not have, got: {text}"
            );
        }
    }

    #[test]
    fn an_inactive_answer_that_gives_no_reason_says_so_rather_than_printing_nothing() {
        let text = render_state(&json!({ "active": false }));
        assert_eq!(text, "cooperation inactive\nreason     unknown");
    }

    #[test]
    fn an_active_answer_is_never_taken_for_an_inactive_one() {
        assert_eq!(inactive(&json!({ "active": true, "reason": "x" })), None);
        assert_eq!(
            inactive(&json!({})),
            None,
            "an answer that says nothing about activity is not a refusal"
        );
    }

    /// A peer tree with two machines. The local one comes first because the capability
    /// orders it first; the renderer must not reorder it, and must say which is which.
    fn two_machines() -> Value {
        json!({
            "active": true,
            "runtime": runtime_key(NODE_A, "0000000000000001"),
            "repository": "repo-1234",
            "machines": [
                {
                    "node": NODE_A,
                    "name": "laptop",
                    "local": true,
                    "runtimes": [{
                        "runtime": runtime_key(NODE_A, "0000000000000001"),
                        "this_runtime": true,
                        "liveness": "own",
                        "last_beat_ms": 2500,
                        "sessions": [{
                            "key": "s-1",
                            "info": {
                                "session": "s-1",
                                "client": "claude-code",
                                "intent": "raise coverage",
                                "issue": "#184",
                            },
                            "state": "open",
                            "claims": [{
                                "scope": ["apps/majordomus-cli", "docs"],
                                "mode": "exclusive",
                                "state": { "state": "held" },
                            }],
                            "reviews": [],
                        }],
                    }],
                },
                {
                    "node": NODE_B,
                    "name": "builder",
                    "local": false,
                    "runtimes": [{
                        "runtime": runtime_key(NODE_B, "0000000000000002"),
                        "this_runtime": false,
                        "liveness": "live",
                        "last_beat_ms": 800,
                        "link": { "state": "linked", "rtt_ms": 12 },
                        "sessions": [],
                    }],
                },
            ],
            "refused": [{
                "endpoint": "127.0.0.1:9",
                "direction": "inbound",
                "refusal": { "code": "untrusted", "detail": "no key allows this node" },
            }],
            "digest": "0123456789abcdef0123456789abcdef",
        })
    }

    #[test]
    fn the_peer_tree_prints_this_machine_before_the_others_and_says_which_is_which() {
        let text = render_peers(&two_machines());
        let local = text
            .find("machine    laptop")
            .expect("this machine is listed");
        let remote = text
            .find("machine    builder")
            .expect("the other machine is listed");
        assert!(
            local < remote,
            "the machine a person is on comes first:\n{text}"
        );
        assert!(text.contains("machine    laptop aaaaaaaa"), "{text}");
        assert!(text.contains("(local)"), "{text}");
        assert!(text.contains("(remote)"), "{text}");
        assert!(
            text.ends_with("digest     0123456789abcdef0123456789abcdef"),
            "{text}"
        );
    }

    #[test]
    fn a_runtime_in_the_tree_carries_its_liveness_its_beat_and_its_link() {
        let text = render_peers(&two_machines());
        assert!(
            text.contains("runtime  aaaaaaaa…-0000000000000001 own beat 2.5s ago"),
            "the local runtime beats and has no link to itself:\n{text}"
        );
        assert!(
            text.contains(
                "runtime  bbbbbbbb…-0000000000000002 live link linked beat 0.8s ago rtt 12ms"
            ),
            "a linked runtime shows the link state and the round trip:\n{text}"
        );
    }

    #[test]
    fn a_session_in_the_tree_brings_its_intent_its_issue_and_its_claims() {
        let text = render_peers(&two_machines());
        assert!(
            text.contains("session  claude-code open [s-1] — raise coverage (#184)"),
            "{text}"
        );
        assert!(
            text.contains("claim  apps/majordomus-cli,docs exclusive held"),
            "every claimed path is printed, not the first:\n{text}"
        );
    }

    #[test]
    fn a_candidate_that_was_refused_is_shown_with_the_rule_that_refused_it() {
        let text = render_peers(&two_machines());
        assert!(
            text.contains("refused    127.0.0.1:9 inbound untrusted: no key allows this node"),
            "a link that never formed is the commonest failure and must be visible:\n{text}"
        );
    }

    #[test]
    fn a_tree_of_no_machines_still_answers_with_the_runtime_and_the_digest() {
        let text = render_peers(&json!({
            "active": true,
            "runtime": "r-1",
            "repository": "repo-1234",
            "machines": [],
            "refused": [],
            "digest": "d0",
        }));
        assert_eq!(
            text, "runtime    r-1\nrepository repo-1234\ndigest     d0",
            "nothing between the header and the digest, and no blank claim of peers"
        );
    }

    #[test]
    fn a_runtime_nobody_here_has_heard_of_is_a_named_absence_not_an_empty_report() {
        let text = render_peer(&json!({
            "found": false,
            "reason": "no runtime cccc is linked or heard here",
            "refused": [],
        }));
        assert_eq!(text, "not found  no runtime cccc is linked or heard here");
    }

    #[test]
    fn a_peer_detail_carries_the_link_counters_a_person_diagnoses_a_flapping_link_by() {
        let text = render_peer(&json!({
            "found": true,
            "machine": "builder",
            "runtime": {
                "runtime": runtime_key(NODE_B, "0000000000000002"),
                "liveness": "live",
                "link": {
                    "state": "linked", "outbound": true, "inbound": false, "protocol": 1,
                    "handshakes": 3, "reconnects": 2, "restarts": 1, "failures": 0,
                },
                "sessions": [{
                    "key": "s-9",
                    "info": { "session": "s-9", "client": "codex" },
                    "state": "closed",
                    "claims": [],
                }],
            },
            "refused": [],
        }));
        assert!(text.starts_with("machine    builder\n"), "{text}");
        assert!(
            text.contains(
                "link       linked (out true, in false) protocol 1 handshakes 3 reconnects 2 restarts 1 failures 0"
            ),
            "a reconnect and a restart are different facts and both are printed:\n{text}"
        );
        assert!(
            text.ends_with("session  codex closed [s-9]"),
            "a session with no intent prints no dash:\n{text}"
        );
    }

    #[test]
    fn a_peer_detail_without_a_link_prints_the_runtime_and_no_link_line() {
        let text = render_peer(&json!({
            "found": true,
            "machine": "laptop",
            "runtime": { "runtime": "r-1", "liveness": "own", "sessions": [] },
            "refused": [],
        }));
        assert_eq!(text, "machine    laptop\nruntime    r-1\nliveness   own");
    }

    fn folded_state() -> Value {
        json!({
            "active": true,
            "state": {
                "sessions": [{ "key": "s/s1" }, { "key": "s/s2" }],
                "claims": [
                    {
                        "key": "s/c1", "scope": ["apps"], "mode": "exclusive",
                        "state": { "state": "held" },
                    },
                    {
                        "key": "s/c2", "scope": ["apps"], "mode": "exclusive",
                        "state": { "state": "conflicted", "detail": "s/c1" },
                    },
                ],
                "overlaps": [{ "a": "s/c1", "b": "s/c2" }],
                "handovers": [{
                    "id": "h1",
                    "runtime": runtime_key(NODE_A, "0000000000000001"),
                    "handover": { "branch": "feature/mesh-cooperation" },
                    "consumed_by": ["s/s2"],
                }],
                "reviews": [{
                    "key": "s/r1", "subject": "feature/mesh-cooperation", "state": "answered",
                    "answers": [{ "verdict": "approved" }, { "verdict": "changes" }],
                }],
                "digest": "cafebabecafebabecafebabecafebabe",
            },
            "streams": [],
        })
    }

    #[test]
    fn a_claim_that_lost_its_scope_names_the_claim_that_beat_it() {
        let text = render_state(&folded_state());
        assert!(
            text.contains("claim      s/c2 apps exclusive conflicted (s/c1)"),
            "the loser must be told which key won, on every runtime:\n{text}"
        );
        assert!(
            text.contains("claim      s/c1 apps exclusive held\n"),
            "the winner carries no detail:\n{text}"
        );
    }

    #[test]
    fn the_state_counts_every_collection_before_it_lists_any_of_them() {
        let text = render_state(&folded_state());
        assert!(
            text.starts_with(
                "sessions   2\nclaims     2\noverlaps   1\nhandovers  1\nreviews    1\n"
            ),
            "{text}"
        );
        assert!(
            text.ends_with("digest     cafebabecafebabecafebabecafebabe"),
            "{text}"
        );
    }

    #[test]
    fn a_handover_is_listed_by_its_branch_and_by_how_many_have_taken_it_up() {
        let text = render_state(&folded_state());
        assert!(
            text.contains(
                "handover   h1 from aaaaaaaa…-0000000000000001 branch feature/mesh-cooperation consumed by 1"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_review_is_listed_with_its_subject_and_the_number_of_answers_it_drew() {
        let text = render_state(&folded_state());
        assert!(
            text.contains("review     s/r1 feature/mesh-cooperation answered (2 answer(s))"),
            "two reviewers who disagree are two answers, not one verdict:\n{text}"
        );
    }

    #[test]
    fn an_empty_state_counts_zero_rather_than_omitting_the_collections() {
        let text = render_state(&json!({ "active": true, "state": { "digest": "d0" } }));
        assert_eq!(
            text,
            "sessions   0\nclaims     0\noverlaps   0\nhandovers  0\nreviews    0\ndigest     d0"
        );
    }

    #[test]
    fn the_event_list_renders_every_kind_and_ends_with_the_watermark_to_page_from() {
        let stream = format!("{NODE_A}-0000000000000001");
        let text = render_events(&json!({
            "active": true,
            "count": 3,
            "lamport": 12,
            "events": [
                { "lamport": 4, "stream": stream, "seq": 1,
                  "body": { "kind": "session_opened" } },
                { "lamport": 7, "stream": stream, "seq": 2,
                  "body": { "kind": "claim_acquired" } },
                { "lamport": 12, "stream": stream, "seq": 3,
                  "body": { "kind": "handover_published" } },
            ],
        }));
        for kind in ["session_opened", "claim_acquired", "handover_published"] {
            assert!(
                text.contains(kind),
                "the kind is what an event is read for:\n{text}"
            );
        }
        assert!(
            text.starts_with("4  aaaaaaaa…-0000000000000001/1  session_opened\n"),
            "the stamp, the stream and the sequence, then the kind:\n{text}"
        );
        assert!(
            text.ends_with("events     3 (lamport 12)"),
            "the watermark is the `after` of the next page:\n{text}"
        );
    }

    #[test]
    fn an_event_of_a_kind_this_executable_does_not_know_is_shown_rather_than_dropped() {
        let text = render_events(&json!({
            "active": true, "count": 1, "lamport": 1,
            "events": [{ "lamport": 1, "stream": "s", "seq": 1, "body": {} }],
        }));
        assert!(
            text.contains("s/1  ?"),
            "an opaque event still takes a line:\n{text}"
        );
    }

    #[test]
    fn a_verification_that_fails_names_the_check_and_the_peer_that_did_not_answer() {
        let text = render_verify(&json!({
            "ok": false,
            "runtime": "r-1",
            "checks": [
                { "check": "identity", "ok": true, "detail": "signed by this node" },
                { "check": "journal", "ok": false, "detail": "a stream is behind",
                  "impact": "peers do not see this runtime's claims",
                  "remediation": "restart the server" },
            ],
            "peers": [
                { "runtime": runtime_key(NODE_B, "0000000000000002"), "state": "linked",
                  "dialed": true, "round_trip": false, "detail": "no answer within the timeout" },
                { "runtime": runtime_key(NODE_A, "0000000000000003"), "state": "linked",
                  "dialed": false, "round_trip": true, "rtt_ms": 9, "converged": true,
                  "detail": "round complete" },
            ],
        }));
        assert!(text.starts_with("runtime    r-1\n"), "{text}");
        assert!(
            text.contains("ok    identity     signed by this node"),
            "{text}"
        );
        assert!(
            text.contains("FAIL  journal      a stream is behind"),
            "{text}"
        );
        assert!(
            text.contains("impact      peers do not see this runtime's claims"),
            "{text}"
        );
        assert!(text.contains("remedy      restart the server"), "{text}");
        assert!(
            text.contains("FAIL  peer bbbbbbbb…-0000000000000002 linked dialed — no answer within the timeout"),
            "a peer that did not come back is a failure of the round:\n{text}"
        );
        assert!(
            text.contains(
                "ok    peer aaaaaaaa…-0000000000000003 linked dials us rtt 9ms converged"
            ),
            "{text}"
        );
        assert!(
            text.ends_with("verdict    a check or a round failed (see above)"),
            "{text}"
        );
    }

    #[test]
    fn a_peer_whose_stream_expired_fails_the_round_even_when_it_answered() {
        let text = render_verify(&json!({
            "ok": false,
            "checks": [],
            "peers": [{ "runtime": "r-2", "state": "expired", "dialed": true,
                        "round_trip": true, "converged": false, "detail": "stale" }],
        }));
        assert!(text.starts_with("FAIL  peer r…-2 expired"), "{text}");
        assert!(text.contains("NOT converged"), "{text}");
    }

    #[test]
    fn a_verification_that_holds_says_cooperation_is_verified() {
        let text = render_verify(&json!({
            "ok": true, "runtime": "r-1",
            "checks": [{ "check": "identity", "ok": true, "detail": "signed" }],
            "peers": [],
        }));
        assert_eq!(text.lines().last(), Some("verdict    cooperation verified"));
    }

    #[test]
    fn a_verification_has_no_guard_because_a_mesh_that_is_off_is_a_failed_verification() {
        let text = render_verify(&json!({
            "ok": false,
            "checks": [{ "check": "cooperation", "ok": false,
                         "detail": "cooperation is disabled in the declaration" }],
            "peers": [],
        }));
        assert!(
            !text.contains("cooperation inactive"),
            "`mesh verify` answers no, with the check, rather than declining to answer:\n{text}"
        );
        assert!(
            text.contains("FAIL  cooperation  cooperation is disabled"),
            "{text}"
        );
    }

    #[test]
    fn what_was_written_is_answered_by_its_key_its_event_and_its_stamp() {
        let text = render_written(&json!({
            "key": "s/c1", "event": "s/4", "lamport": 17,
        }));
        assert_eq!(text, "written    s/c1\nevent      s/4 (lamport 17)");
    }

    #[test]
    fn a_consumed_handover_names_the_record_it_wrote_into_the_checkout() {
        let text = render_consumed(&json!({
            "handover": {
                "id": "h1",
                "runtime": "r-1",
                "handover": { "branch": "feature/mesh-cooperation" },
            },
            "written": { "key": "s/x", "event": "s/5", "lamport": 20 },
            "path": ".ai/local/handovers/h1.md",
        }));
        assert_eq!(
            text,
            "handover   h1\nfrom       r-1\nevent      s/5\n\
             record     .ai/local/handovers/h1.md\nbranch     feature/mesh-cooperation"
        );
    }

    #[test]
    fn a_handover_consumed_without_materializing_writes_no_record_line() {
        let text = render_consumed(&json!({
            "handover": { "id": "h1", "runtime": "r-1", "handover": {} },
            "written": { "event": "s/5" },
        }));
        assert!(!text.contains("record"), "{text}");
        assert!(
            text.ends_with("branch     -"),
            "a handover that names no branch prints a dash, not an empty field:\n{text}"
        );
    }

    #[test]
    fn the_status_line_says_active_or_inactive_before_anything_else() {
        let off =
            render_status(&json!({ "active": false, "reason": "disabled in the declaration" }));
        assert!(
            off.starts_with("mesh       inactive\nreason     disabled in the declaration"),
            "{off}"
        );

        let on = render_status(&json!({
            "active": true,
            "identity": { "node_id": "n1", "display_name": "laptop" },
            "trust_policy": "deny_unknown",
            "providers": [
                { "id": "lan", "state": "running", "sent": 4, "received": 7 },
                { "id": "relay", "state": "failed", "sent": 0, "received": 0,
                  "detail": "no route to host" },
            ],
            "tallies": { "nodes": 3, "trusted": 2, "present": 1,
                         "accepted": 10, "replayed": 1, "expired": 0 },
        }));
        assert!(on.starts_with("mesh       active\n"), "{on}");
        assert!(on.contains("node       n1 (laptop)"), "{on}");
        assert!(on.contains("trust      deny_unknown"), "{on}");
        assert!(
            on.contains("provider   lan running sent 4 received 7\n"),
            "{on}"
        );
        assert!(
            on.contains("provider   relay failed sent 0 received 0 — no route to host"),
            "a failed provider says why, or it cannot be fixed:\n{on}"
        );
        assert!(
            on.ends_with("nodes      3 (2 trusted, 1 present); accepted 10, replayed 1, expired 0"),
            "{on}"
        );
    }

    #[test]
    fn the_node_list_falls_back_to_the_status_when_the_mesh_is_off() {
        let off = json!({ "active": false, "reason": "disabled", "tallies": {} });
        assert_eq!(render_nodes(&off), render_status(&off));
    }

    #[test]
    fn a_mesh_that_has_observed_nobody_says_zero_rather_than_printing_a_header() {
        assert_eq!(
            render_nodes(&json!({ "active": true, "nodes": [] })),
            "mesh       0 node(s) observed"
        );
    }

    #[test]
    fn every_observed_node_is_listed_with_the_sources_that_saw_it() {
        let text = render_nodes(&json!({
            "active": true,
            "nodes": [{
                "node_id": "n1", "runtime": "0000000000000001", "display_name": "laptop",
                "trust": { "state": "trusted" }, "presence": "present",
                "sources": [{ "source": "lan" }, { "source": "seed" }],
                "last_seen": "2026-09-17T10:00:00Z",
            }],
        }));
        assert!(text.starts_with("mesh       1 node(s)\n"), "{text}");
        assert!(
            text.contains("n1-0000000000000001  laptop  trust trusted  present  [lan,seed]  last seen 2026-09-17T10:00:00Z"),
            "two sources that saw one node are both named:\n{text}"
        );
    }

    #[test]
    fn the_identity_report_prints_the_key_a_peer_has_to_be_told_to_allow() {
        let text = render_identity(&json!({
            "present": true,
            "path": ".ai/local/mesh/identity.json",
            "identity": { "node_id": "n1", "display_name": "laptop", "public_key": "ed25519:AAAA" },
        }));
        assert!(text.contains("present    true"), "{text}");
        assert!(
            text.contains("path       .ai/local/mesh/identity.json"),
            "{text}"
        );
        assert!(text.contains("node       n1"), "{text}");
        assert!(text.contains("name       laptop"), "{text}");
        assert!(
            text.ends_with("key        ed25519:AAAA"),
            "the public key is what the other machine's trust.allow needs:\n{text}"
        );
    }

    #[test]
    fn an_unreadable_identity_reports_the_error_instead_of_an_empty_node() {
        let text = render_identity(&json!({ "present": false, "error": "not valid JSON" }));
        assert_eq!(text, "present    false\nerror      not valid JSON");
    }

    #[test]
    fn the_doctor_ends_with_one_verdict_over_every_check_it_ran() {
        let good = render_doctor(&json!({
            "ok": true,
            "checks": [{ "check": "identity", "ok": true, "detail": "readable" }],
        }));
        assert_eq!(
            good,
            "ok    identity     readable\nverdict    every check holds"
        );

        let bad = render_doctor(&json!({
            "ok": false,
            "checks": [{ "check": "sockets", "ok": false, "detail": "port 8742 is taken",
                         "impact": "no peer can reach this runtime",
                         "remediation": "free the port" }],
        }));
        assert!(
            bad.contains("FAIL  sockets      port 8742 is taken"),
            "{bad}"
        );
        assert!(
            bad.contains("      impact      no peer can reach this runtime"),
            "{bad}"
        );
        assert!(bad.contains("      remedy      free the port"), "{bad}");
        assert!(
            bad.ends_with("verdict    a check failed (see above)"),
            "{bad}"
        );
    }

    #[test]
    fn a_runtime_key_is_shortened_to_the_node_a_person_can_recognize_and_the_whole_runtime() {
        assert_eq!(
            short(&runtime_key(NODE_A, "0000000000000001")),
            "aaaaaaaa…-0000000000000001",
            "the node is elided, the runtime never is: two runtimes on one machine must differ"
        );
        assert_eq!(
            short("short-rt"),
            "short…-rt",
            "a node shorter than eight is not padded"
        );
        assert_eq!(
            short("nodash"),
            "nodash",
            "what is not a runtime key is printed as it is"
        );
    }

    #[test]
    fn a_query_value_is_percent_encoded_outside_the_unreserved_set() {
        assert_eq!(encode("abcXYZ019-_.~"), "abcXYZ019-_.~");
        assert_eq!(encode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(
            encode("é"),
            "%C3%A9",
            "a non-ASCII value is encoded byte by byte, as a URL carries it"
        );
    }

    #[test]
    fn an_option_nobody_gave_is_absent_from_the_request_rather_than_null() {
        let body = strip_nulls(json!({
            "session": "s1",
            "issue": Value::Null,
            "nested": { "task": Value::Null, "scope": ["apps"] },
        }));
        assert_eq!(
            body,
            json!({ "session": "s1", "nested": { "scope": ["apps"] } })
        );
        assert_eq!(
            strip_nulls(json!([1, null])),
            json!([1, null]),
            "only members are dropped; an array the caller built is passed through"
        );
    }

    #[test]
    fn a_question_that_was_answered_is_never_a_failure_by_itself() {
        assert!(!never(&json!({ "ok": false })));
        assert_eq!(s(&json!({ "a": "b" }), "a"), "b");
        assert_eq!(
            s(&json!({ "a": 1 }), "a"),
            "",
            "a value that is not a string reads as empty"
        );
    }
}
