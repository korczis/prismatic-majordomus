//! What the MCP projection says about itself, held against what the server does.
//!
//! The registry declares each capability once; the MCP surface, the HTTP route, the
//! `mcp.projection` capability and the Cockpit's MCP page are all readings of that one
//! declaration. A reading can drift from the thing it reads, so every test here compares
//! two of them that were produced separately:
//!
//! - the methods the dispatcher names against the methods that answer, and the protocol
//!   versions it names against the ones it negotiates;
//! - the effect each listed tool states against the registry's, and the hints a client is
//!   given against the rule that derives them from the effect;
//! - the class a refusal names over MCP against the class the same capability names over
//!   HTTP;
//! - `mcp.projection` — as a tool, as a resource and as a route — against `initialize`,
//!   `tools/list` and the registry;
//! - the Cockpit's MCP page against `tools/list`: every tool the server lists is on the
//!   page, and narrowing by effect leaves the tools of that effect.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use common::{Fixture, Served, BIN};
use majordomus_cli::capability::{CapabilityKind, CaseContext, Effect, HttpMethod};
use majordomus_cli::mcp::{METHODS, PROTOCOL_VERSIONS, SERVER_NAME, SERVER_TITLE};
use serde_json::{json, Value};

/// The three classes a refusal may name. A fourth would be a new contract.
const REFUSAL_CODES: &[&str] = &["invalid_input", "not_found", "refused"];

fn state_home(f: &Fixture) -> String {
    f.parent().join("state").to_str().unwrap().to_string()
}

/// One standalone stdio session: the frames that answer a request, by id.
fn session(f: &Fixture, requests: &[Value]) -> BTreeMap<u64, Value> {
    let root = f.root();
    let mut child = Command::new(BIN)
        .args(["mcp", "--standalone", "--repo", root.to_str().unwrap()])
        .current_dir(&root)
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("MAJORDOMUS_ROOT")
        .env("XDG_STATE_HOME", state_home(f))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn majordomus mcp");
    let mut stdin = child.stdin.take().unwrap();
    let mut input = String::new();
    for r in requests {
        input.push_str(&format!("{r}\n"));
    }
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let mut err = child.stderr.take().unwrap();
    let errors = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err.read_to_end(&mut s);
        String::from_utf8_lossy(&s).into_owned()
    });
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .expect("stdout is UTF-8");
    let status = child.wait().expect("wait");
    writer.join().unwrap();
    let stderr = errors.join().unwrap();
    assert_eq!(status.code(), Some(0), "{stderr}");
    stdout
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .filter_map(|v| v["id"].as_u64().map(|id| (id, v)))
        .collect()
}

fn serve(f: &Fixture) -> Served {
    Served::start_with_env(&f.root(), &[], &[("XDG_STATE_HOME", &state_home(f))])
}

fn initialize(id: u64, version: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "initialize", "params": {
        "protocolVersion": version, "capabilities": {},
        "clientInfo": { "name": "projection", "version": "0" } } })
}

fn opening() -> Vec<Value> {
    vec![
        initialize(1, PROTOCOL_VERSIONS[0]),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ]
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn call(id: u64, tool: &str, arguments: Value) -> Value {
    request(
        id,
        "tools/call",
        json!({ "name": tool, "arguments": arguments }),
    )
}

/// The tools a session lists.
fn listed_tools(f: &Fixture) -> Vec<Value> {
    let mut requests = opening();
    requests.push(request(2, "tools/list", json!({})));
    session(f, &requests)[&2]["result"]["tools"]
        .as_array()
        .expect("tools")
        .clone()
}

/// The registry's word for an effect, as every projection serialises it.
fn effect_word(effect: Effect) -> String {
    serde_json::to_value(effect)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

// ------------------------------------------------------------------------- methods, versions

/// The dispatcher's list is the server's whole vocabulary: every method on it is answered
/// as a method the server has, and every other method of the protocol — the ones this
/// server does not implement, a misspelling, the empty string — is "method not found".
/// The versions it names are the versions it negotiates, exactly and in its own order.
#[test]
fn the_methods_and_versions_the_server_names_are_the_ones_it_answers() {
    let f = Fixture::new();
    // methods the specification defines for a server and this one may not have, plus noise
    let others = [
        "prompts/list",
        "prompts/get",
        "completion/complete",
        "logging/setLevel",
        "resources/subscribe",
        "resources/unsubscribe",
        "roots/list",
        "sampling/createMessage",
        "elicitation/create",
        "tools/List",
        "tools",
        "",
    ];
    let mut requests = Vec::new();
    for (i, v) in PROTOCOL_VERSIONS.iter().enumerate() {
        requests.push(initialize(i as u64 + 1, v));
    }
    requests.push(initialize(50, "1999-01-01"));
    for (i, m) in METHODS.iter().enumerate() {
        requests.push(request(100 + i as u64, m, json!({})));
    }
    for (i, m) in others.iter().enumerate() {
        requests.push(request(200 + i as u64, m, json!({})));
    }
    let r = session(&f, &requests);

    for (i, v) in PROTOCOL_VERSIONS.iter().enumerate() {
        let result = &r[&(i as u64 + 1)]["result"];
        assert_eq!(result["protocolVersion"], *v, "asked for {v}");
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
        assert_eq!(result["serverInfo"]["title"], SERVER_TITLE);
        assert_eq!(result["serverInfo"]["version"], majordomus_cli::VERSION);
    }
    assert_eq!(
        r[&50]["result"]["protocolVersion"], PROTOCOL_VERSIONS[0],
        "a version the server does not name is answered with its first"
    );
    for (i, m) in METHODS.iter().enumerate() {
        let answer = &r[&(100 + i as u64)];
        assert_ne!(
            answer["error"]["code"], -32601,
            "{m} is on the dispatcher's list and is answered as not found: {answer}"
        );
    }
    for (i, m) in others.iter().enumerate() {
        if METHODS.contains(m) {
            continue;
        }
        let answer = &r[&(200 + i as u64)];
        assert_eq!(
            answer["error"]["code"], -32601,
            "`{m}` is not on the dispatcher's list and is answered: {answer}"
        );
    }
    let mut sorted = METHODS.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        METHODS.len(),
        "the dispatcher names a method twice"
    );
}

// ------------------------------------------------------------------------- effects, hints

/// Every listed tool states the effect its capability declares, and the hints a client
/// acts on are that effect and nothing else: a reader is read-only and safe to repeat, a
/// tool that changes this process is neither, and a tool that writes the repository is
/// also destructive. No tool reaches outside this machine's repository, so none is
/// open-world. The rule is asserted over every tool; how many there are of each is the
/// registry's business.
#[test]
fn every_tool_states_its_effect_and_its_hints_follow_from_it() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let declared: BTreeMap<String, Effect> = app
        .context
        .registry
        .iter()
        .filter_map(|c| Some((c.exposure.mcp.as_ref()?.tool.clone()?, c.execution.effect)))
        .collect();
    let tools = listed_tools(&f);
    assert_eq!(
        tools.len(),
        declared.len(),
        "tools/list and the registry differ in size"
    );
    let mut seen = BTreeSet::new();
    let mut wrong = Vec::new();
    for t in &tools {
        let name = t["name"].as_str().unwrap();
        let Some(effect) = declared.get(name) else {
            wrong.push(format!("{name}: listed and not in the registry"));
            continue;
        };
        let word = effect_word(*effect);
        seen.insert(word.clone());
        if t["_meta"]["majordomus"]["effect"] != word.as_str() {
            wrong.push(format!(
                "{name}: states effect {} and the registry declares {word}",
                t["_meta"]["majordomus"]["effect"]
            ));
        }
        let (read_only, idempotent, destructive) = match effect {
            Effect::Read => (true, true, false),
            Effect::ProcessState => (false, false, false),
            Effect::RepositoryMutation => (false, false, true),
        };
        let a = &t["annotations"];
        for (hint, want) in [
            ("readOnlyHint", read_only),
            ("idempotentHint", idempotent),
            ("destructiveHint", destructive),
            ("openWorldHint", false),
        ] {
            if a[hint] != want {
                wrong.push(format!(
                    "{name} ({word}): {hint} is {} and should be {want}",
                    a[hint]
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} hint(s) do not follow:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    assert!(
        seen.len() >= 2,
        "the fixture's registry holds tools of one effect only ({seen:?}); the rule was not exercised"
    );
}

// ------------------------------------------------------------------------- refusals

/// A refusal says which kind it is, in one of three words, and carries no data: a client
/// that branches on the word must never find an answer beside it. The same capability
/// asked over HTTP names the same word — one classification, read twice.
///
/// The refusals are provoked from the registry: every tool with a required property is
/// called without it, every read tool is asked its own benchmark cases (some of which are
/// honest refusals), and an object that does not exist is asked for.
#[test]
fn a_refusal_names_its_class_and_the_same_one_over_http() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = &app.context;
    let case_ctx = CaseContext::of(ctx);

    struct Asked {
        what: String,
        route: Option<String>,
        expect: Option<&'static str>,
    }
    let mut requests = opening();
    let mut asked: BTreeMap<u64, Asked> = BTreeMap::new();
    let mut id = 10;
    for c in ctx.registry.iter() {
        let Some(tool) = c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()) else {
            continue;
        };
        let get = c
            .exposure
            .http
            .as_ref()
            .filter(|h| h.method == HttpMethod::Get)
            .map(|h| h.path.clone());
        let (_, required) = c.input.properties();
        if !required.is_empty() {
            // input is refused before any handler runs, so every effect can be asked
            requests.push(call(id, &tool, json!({})));
            asked.insert(
                id,
                Asked {
                    what: format!("{tool} with nothing"),
                    route: get.clone(),
                    expect: Some("invalid_input"),
                },
            );
            id += 1;
        }
        if c.execution.effect == Effect::Read && c.kind == CapabilityKind::Query {
            let cases = ctx
                .registry
                .cases(c.id.as_str())
                .map(|p| p(&case_ctx))
                .unwrap_or_default();
            for case in cases {
                let scalar = case
                    .input
                    .as_object()
                    .is_some_and(|m| m.values().all(|v| !v.is_object() && !v.is_array()));
                let route = get
                    .as_ref()
                    .filter(|_| scalar)
                    .map(|p| target_of(p, &case.input));
                requests.push(call(id, &tool, case.input));
                asked.insert(
                    id,
                    Asked {
                        what: format!("{tool} case {}", case.name),
                        route,
                        expect: None,
                    },
                );
                id += 1;
            }
        }
    }
    requests.push(call(
        id,
        "majordomus_get",
        json!({ "uri": "majordomus://rule/no.such@1" }),
    ));
    asked.insert(
        id,
        Asked {
            what: "majordomus_get of an object that does not exist".into(),
            route: None,
            expect: Some("not_found"),
        },
    );
    let r = session(&f, &requests);
    let served = serve(&f);

    let mut wrong = Vec::new();
    let mut classes = BTreeSet::new();
    let mut compared = 0;
    for (id, a) in &asked {
        let result = &r[id]["result"];
        if result["isError"] != true {
            if a.expect.is_some() {
                wrong.push(format!(
                    "{}: not refused: {}",
                    a.what,
                    trim(&r[id].to_string())
                ));
            }
            continue;
        }
        let code = result["_meta"]["majordomus"]["error"]["code"].as_str();
        let Some(code) = code.filter(|c| REFUSAL_CODES.contains(c)) else {
            wrong.push(format!(
                "{}: a refusal with no class: {}",
                a.what,
                trim(&result.to_string())
            ));
            continue;
        };
        classes.insert(code.to_string());
        if result.get("structuredContent").is_some() {
            wrong.push(format!("{}: a refusal that carries data", a.what));
        }
        if let Some(want) = a.expect {
            if code != want {
                wrong.push(format!("{}: refused as {code}, expected {want}", a.what));
            }
        }
        // the same question over HTTP, when the capability has a GET route for it
        let target = match (&a.route, a.expect) {
            (Some(route), Some("invalid_input")) => Some(route.clone()),
            (Some(route), None) => Some(route.clone()),
            _ => None,
        };
        if let Some(target) = target {
            let (status, _, body) = served.request("GET", &target, None);
            let over_http = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v["error"]["code"].as_str().map(String::from));
            compared += 1;
            if !(400..500).contains(&status) || over_http.as_deref() != Some(code) {
                wrong.push(format!(
                    "{}: MCP refuses as {code}; GET {target} answers {status} {over_http:?}",
                    a.what
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} refusal(s) are wrong:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    assert!(
        classes.contains("invalid_input") && classes.contains("not_found"),
        "the sweep provoked only {classes:?}"
    );
    assert!(
        compared >= 10,
        "only {compared} refusal(s) were compared with HTTP"
    );
}

// ------------------------------------------------------------------------- mcp.projection

/// `mcp.projection` is the server describing its own MCP surface, and a description is
/// only worth serving if it is the surface: its versions are the negotiated ones, its
/// methods the dispatcher's, its tools exactly `tools/list` with the same effects and
/// hints, its writers the tools that write, its tallies the sums of what it lists, and
/// "no prompts" true of a server that advertises none. The tool, the resource and the
/// route are one answer.
#[test]
fn the_projection_describes_the_server_that_answers_it() {
    let f = Fixture::new();
    let mut requests = opening();
    requests.push(request(2, "tools/list", json!({})));
    requests.push(call(3, "majordomus_mcp", json!({})));
    requests.push(request(
        4,
        "resources/read",
        json!({ "uri": "majordomus://mcp" }),
    ));
    requests.push(request(5, "resources/list", json!({})));
    for (i, effect) in ["read", "process_state", "repository_mutation"]
        .iter()
        .enumerate()
    {
        requests.push(call(
            10 + i as u64,
            "majordomus_mcp",
            json!({ "effect": effect }),
        ));
    }
    requests.push(call(
        20,
        "majordomus_mcp",
        json!({ "effect": "no_such_effect" }),
    ));
    let r = session(&f, &requests);
    let init = &r[&1]["result"];
    let listed = r[&2]["result"]["tools"].as_array().expect("tools");
    assert_eq!(r[&3]["result"]["isError"], false, "{}", r[&3]);
    let p = &r[&3]["result"]["structuredContent"];

    assert_eq!(p["server"]["name"], init["serverInfo"]["name"]);
    assert_eq!(p["server"]["title"], init["serverInfo"]["title"]);
    assert_eq!(p["server"]["version"], majordomus_cli::VERSION);
    assert_eq!(p["protocol_versions"], json!(PROTOCOL_VERSIONS));
    let methods: BTreeSet<&str> = p["serving"]["methods"]
        .as_array()
        .expect("methods")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(methods, METHODS.iter().copied().collect::<BTreeSet<_>>());
    for (family, key) in [
        ("tools", "tools"),
        ("resources", "resources"),
        ("prompts", "prompts"),
    ] {
        assert_eq!(
            p["serving"][key].as_bool(),
            Some(
                init["capabilities"]
                    .get(family)
                    .is_some_and(Value::is_object)
            ),
            "serving.{key} disagrees with what initialize advertises"
        );
    }

    // the tools, one for one, in the listing's order, with the listing's effects and hints
    let described = p["tools"].as_array().expect("tools");
    assert_eq!(
        p["tool_count"],
        listed.len(),
        "tool_count is not the listing's length"
    );
    let names = |ts: &[Value]| {
        ts.iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(described),
        names(listed),
        "the projection lists other tools, or another order"
    );
    for (d, l) in described.iter().zip(listed) {
        let name = &l["name"];
        assert_eq!(d["effect"], l["_meta"]["majordomus"]["effect"], "{name}");
        assert_eq!(d["capability"], l["_meta"]["majordomus"]["id"], "{name}");
        assert_eq!(d["title"], l["title"], "{name}");
        for (ours, theirs) in [
            ("read_only", "readOnlyHint"),
            ("destructive", "destructiveHint"),
            ("idempotent", "idempotentHint"),
            ("open_world", "openWorldHint"),
        ] {
            assert_eq!(d["hints"][ours], l["annotations"][theirs], "{name}: {ours}");
        }
    }
    let with_effect = |effect: &str| {
        listed
            .iter()
            .filter(|t| t["_meta"]["majordomus"]["effect"] == effect)
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    let writers: Vec<String> = p["writers"]
        .as_array()
        .expect("writers")
        .iter()
        .map(|w| {
            w.as_str()
                .or_else(|| w["name"].as_str())
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(
        writers,
        with_effect("repository_mutation"),
        "writers are the tools that write"
    );
    let mut tallied = 0;
    for e in p["effects"].as_array().expect("effects") {
        let effect = e["effect"].as_str().unwrap();
        assert_eq!(
            e["tools"],
            with_effect(effect).len(),
            "the tally of {effect}"
        );
        tallied += e["tools"].as_u64().unwrap();
    }
    assert_eq!(
        tallied,
        listed.len() as u64,
        "the tallies do not sum to the tools"
    );

    // the resource is the tool's answer; the listing counts are the resource listing's
    let text = r[&4]["result"]["contents"][0]["text"]
        .as_str()
        .expect("text");
    assert_eq!(
        &serde_json::from_str::<Value>(text).unwrap(),
        p,
        "the resource and the tool differ"
    );
    let resources = r[&5]["result"]["resources"].as_array().expect("resources");
    // `template` is the URI shape, not a count: what is listed is the builtin resources
    // and the layer's objects, and nothing else
    let counted = p["resources"]["builtin"].as_u64().expect("builtin")
        + p["resources"]["objects"].as_u64().expect("objects");
    assert_eq!(
        counted,
        resources.len() as u64,
        "resources.builtin + resources.objects is not the length of resources/list"
    );
    assert!(
        p["resources"]["template"]
            .as_str()
            .is_some_and(|t| t.starts_with("majordomus://")),
        "resources.template is the URI shape: {}",
        p["resources"]["template"]
    );

    // narrowing by effect leaves exactly the tools of that effect
    for (i, effect) in ["read", "process_state", "repository_mutation"]
        .iter()
        .enumerate()
    {
        let narrowed = &r[&(10 + i as u64)]["result"]["structuredContent"];
        assert_eq!(
            names(narrowed["tools"].as_array().expect("tools")),
            with_effect(effect),
            "effect={effect} does not narrow to the tools of that effect"
        );
    }
    assert_eq!(
        r[&20]["result"]["isError"], true,
        "an effect that does not exist is refused, not answered with everything"
    );

    // the route is the same description; who is attached is each process's own
    let served = serve(&f);
    let (status, over_http) = served.get("/api/v1/mcp");
    assert_eq!(status, 200, "{over_http}");
    for key in [
        "server",
        "protocol_versions",
        "tool_count",
        "tools",
        "effects",
        "writers",
    ] {
        assert_eq!(
            over_http[key], p[key],
            "GET /api/v1/mcp and the tool differ in `{key}`"
        );
    }

    // every finding says what is wrong and what to do about it
    for finding in p["findings"].as_array().expect("findings") {
        for key in ["code", "message", "remedy"] {
            assert!(
                finding[key].as_str().is_some_and(|s| !s.trim().is_empty()),
                "a finding without a {key}: {finding}"
            );
        }
    }
}

// ------------------------------------------------------------------------- the Cockpit

/// The Cockpit's MCP page shows the server's tools, not a copy of them: every tool
/// `tools/list` names is on the page, and `?effect=` leaves in the Tools table exactly the
/// tools of that effect — no more, no fewer. A page built from a list written into a
/// template would pass the first assertion the day it was written and fail it the day a
/// tool was added, which is the point of asking the server and the page separately.
///
/// Only the Tools table is narrowed: the page's last card lists every capability with its
/// tool whatever the filter says, so the table is found by its header row and read alone.
#[test]
fn the_cockpit_page_shows_the_tools_the_server_lists() {
    let f = Fixture::new();
    let listed = listed_tools(&f);
    let names: Vec<&str> = listed.iter().filter_map(|t| t["name"].as_str()).collect();
    let served = serve(&f);
    let get = |target: &str| {
        let (status, _, page) =
            served.request_with("GET", target, None, &[("Accept", "text/html")]);
        assert_eq!(status, 200, "GET {target}: {}", trim(&page));
        page
    };
    let in_table = |page: &str| -> BTreeSet<String> {
        let table = tools_table(page).expect("a table headed Tool | Capability | Title | Effect");
        names
            .iter()
            .filter(|n| whole_word(table, n))
            .map(|n| n.to_string())
            .collect()
    };
    let all: BTreeSet<String> = names.iter().map(|n| n.to_string()).collect();

    let page = get("/cockpit/mcp");
    assert_eq!(
        in_table(&page),
        all,
        "the Tools table is not the tools the server lists"
    );
    assert!(
        page.contains(majordomus_cli::VERSION),
        "the page does not state the server's version"
    );

    let mut narrowed_any = false;
    for effect in ["read", "process_state", "repository_mutation"] {
        let expected: BTreeSet<String> = listed
            .iter()
            .filter(|t| t["_meta"]["majordomus"]["effect"] == effect)
            .filter_map(|t| t["name"].as_str().map(String::from))
            .collect();
        let narrowed = get(&format!("/cockpit/mcp?effect={effect}"));
        assert_eq!(
            in_table(&narrowed),
            expected,
            "?effect={effect} does not leave exactly the tools of that effect in the Tools table"
        );
        if expected.len() < all.len() {
            narrowed_any = true;
            assert!(
                narrowed.len() < page.len(),
                "?effect={effect} shows fewer tools on a page that is no smaller"
            );
        }
    }
    assert!(
        narrowed_any,
        "every tool has one effect, so no filter narrowed anything"
    );
}

/// A page asked for an effect that is none of the three is not an empty page and not the
/// whole one: the capability refuses the input, and the page says the capability did not
/// answer, with the status of a failure.
#[test]
fn the_cockpit_page_says_so_when_the_capability_refuses_what_it_was_asked() {
    let f = Fixture::new();
    let served = serve(&f);
    let (status, _, page) = served.request_with(
        "GET",
        "/cockpit/mcp?effect=nonsense",
        None,
        &[("Accept", "text/html")],
    );
    assert_eq!(status, 500, "{}", trim(&page));
    assert!(
        page.contains("The capability behind this page did not answer"),
        "{}",
        trim(&page)
    );
    assert!(tools_table(&page).is_none(), "a refused page lists no tool");
}

/// The table whose header row reads Tool, Capability, Title, Effect, in that order: its
/// text from `<table` to `</table>`.
fn tools_table(page: &str) -> Option<&str> {
    let mut rest = page;
    while let Some(at) = rest.find("<table") {
        let from = &rest[at..];
        let end = from
            .find("</table>")
            .map(|e| e + "</table>".len())
            .unwrap_or(from.len());
        let table = &from[..end];
        let head = &table[..table.find("</tr>").unwrap_or(table.len())];
        let mut cursor = 0;
        let ordered = ["Tool", "Capability", "Title", "Effect"].iter().all(|h| {
            match head[cursor..].find(h) {
                Some(i) => {
                    cursor += i + h.len();
                    true
                }
                None => false,
            }
        });
        if ordered {
            return Some(table);
        }
        rest = &from[end..];
    }
    None
}

/// Does `text` hold `word` with no identifier character on either side?
fn whole_word(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
}

/// The GET target of a capability's route for one input: scalars as a query string.
fn target_of(path: &str, input: &Value) -> String {
    let query: Vec<String> = input
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| {
                    let text = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    format!("{}={}", encode(k), encode(&text))
                })
                .collect()
        })
        .unwrap_or_default();
    if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", query.join("&"))
    }
}

fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn trim(s: &str) -> String {
    s.chars().take(400).collect()
}
