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
