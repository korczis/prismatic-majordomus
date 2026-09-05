//! `majordomus serve` as a client sees it: a child process on a loopback port, real
//! requests, the OpenAPI document, the Swagger shell, capability operations, errors, and
//! the same answer from HTTP and MCP for the same capability.

mod common;

use std::collections::BTreeSet;
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

use common::{rule, Fixture, Served, BIN};
use serde_json::{json, Value};

#[test]
fn openapi_docs_and_one_operation_over_a_real_socket() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    assert!(
        s.address.starts_with("127.0.0.1:"),
        "loopback by default: {}",
        s.address
    );

    let (status, index) = s.get("/");
    assert_eq!(status, 200);
    assert_eq!(index["openapi"], "/openapi.json");

    let (status, doc) = s.get("/openapi.json");
    assert_eq!(status, 200);
    assert_eq!(doc["openapi"], "3.1.0");
    let ops: Vec<&Value> = doc["paths"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|m| m.as_object().unwrap().values())
        .collect();
    let ids: BTreeSet<&str> = ops
        .iter()
        .map(|o| o["operationId"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), ops.len());
    assert!(ids.contains("objects.get") && ids.contains("capabilities.list"));
    assert!(ops
        .iter()
        .all(|o| o["responses"]["200"].is_object() && o["x-majordomus-id"].is_string()));
    assert!(common::openapi_refs_resolve(&doc).is_empty());
    assert!(
        !doc.to_string().contains(f.root().to_str().unwrap()),
        "the document carries the checkout path"
    );

    let (status, headers, html) = s.request("GET", "/docs", None);
    assert_eq!(status, 200);
    assert!(headers
        .iter()
        .any(|(k, v)| k == "content-type" && v.starts_with("text/html")));
    assert!(html.contains("url: \"/openapi.json\""), "{html}");
    assert!(html.contains("swagger-ui-dist@"));
    assert!(
        !html.contains("\"paths\""),
        "the shell embeds no specification"
    );

    let (status, list) = s.get("/api/v1/objects?kind=rule");
    assert_eq!(status, 200);
    assert_eq!(list["count"], 1);
    assert_eq!(list["objects"][0]["id"], "rule.project.alpha@1");

    let (status, got) = s.get("/api/v1/object?uri=majordomus://rule/project.alpha@1");
    assert_eq!(status, 200);
    assert_eq!(got["content"], rule("project.alpha", 1, "Alpha"));
    assert_eq!(
        got["provenance"]["path"],
        ".ai/repo/rules/project/alpha.v1.md"
    );

    let (status, caps) = s.get("/api/v1/capabilities?kind=query");
    assert_eq!(status, 200);
    assert!(caps["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == "objects.get"));
    let (status, one) = s.get("/api/v1/capability?id=rule.project.alpha@1");
    assert_eq!(status, 200);
    assert_eq!(one["kind"], "resource");
    assert_eq!(
        one["exposure"]["mcp"]["resource"]["uri"],
        "majordomus://rule/project.alpha@1"
    );
    assert!(one["exposure"].get("http").is_none());

    let (status, repo) = s.get("/api/v1/repository");
    assert_eq!(status, 200);
    assert_eq!(repo["state"], "ok");
    assert_eq!(repo["capabilities"]["http_routes"], 9);

    // the object route resolves the repository URI to repository.info, as MCP does
    let (status, doc) = s.get("/api/v1/object?uri=majordomus://repository");
    assert_eq!(status, 200, "{doc}");
    assert_eq!(doc["source"], "builtin");
    assert_eq!(doc["id"], "repository.info");
    assert_eq!(doc["identity"], "repository");
    assert_eq!(doc["media_type"], "application/json");
    assert_eq!(
        doc["answer"], repo,
        "the same report the repository route answers"
    );
    let parsed: Value = serde_json::from_str(doc["content"].as_str().unwrap()).unwrap();
    assert_eq!(parsed, repo, "and its text parses back to it");
    assert_eq!(got["source"], "declarative");
}

#[test]
fn errors_are_typed_and_the_server_survives_them() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (status, v) = s.get("/api/v1/object");
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (400, Some("invalid_input"))
    );
    let (status, v) = s.get("/api/v1/object?uri=majordomus://rule/none@1");
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (404, Some("not_found"))
    );
    let (status, v) = s.get("/api/v1/objects?bogus=1");
    assert_eq!(status, 400);
    assert!(v["error"]["message"].as_str().unwrap().contains("bogus"));
    let (status, v) = s.get("/api/v1/search?query=x&limit=abc");
    assert_eq!(status, 400);
    assert!(v["error"]["message"].as_str().unwrap().contains("limit"));
    let (status, v) = s.get("/api/v1/search?query=%20");
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (422, Some("refused"))
    );
    let (status, v) = s.get("/api/v1/capability?id=nope.x");
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (404, Some("not_found"))
    );
    let (status, _) = s.get("/nope");
    assert_eq!(status, 404);
    let (status, _, _) = s.request("POST", "/api/v1/objects", Some("{}"));
    assert_eq!(status, 405);
    let (status, _, _) = s.request("DELETE", "/api/v1/objects", None);
    assert_eq!(status, 405);
    // still alive
    let (status, _) = s.get("/api/v1/repository");
    assert_eq!(status, 200);
}

/// One MCP tools/call over stdio, returning structuredContent.
fn mcp_call(cwd: &std::path::Path, tool: &str, args: Value) -> Value {
    let mut child = Command::new(BIN)
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": tool, "arguments": args } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    frames[1]["result"]["structuredContent"].clone()
}

#[test]
fn mcp_and_http_answer_the_same_capability_with_the_same_result() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let via_http = s.get("/api/v1/object?uri=majordomus://prompt/continue").1;
    let via_mcp = mcp_call(
        &f.root(),
        "majordomus_get",
        json!({ "uri": "majordomus://prompt/continue" }),
    );
    assert_eq!(via_http, via_mcp);
    let via_http = s.get("/api/v1/object?uri=majordomus://repository").1;
    let via_mcp = mcp_call(
        &f.root(),
        "majordomus_get",
        json!({ "uri": "majordomus://repository" }),
    );
    assert_eq!(
        via_http, via_mcp,
        "the repository URI resolves alike on both"
    );
    let via_http = s.get("/api/v1/search?query=resume&limit=5").1;
    let via_mcp = mcp_call(
        &f.root(),
        "majordomus_search",
        json!({ "query": "resume", "limit": 5 }),
    );
    assert_eq!(via_http, via_mcp);
    let via_http = s.get("/api/v1/capability?id=objects.search").1;
    let via_mcp = mcp_call(
        &f.root(),
        "majordomus_capability",
        json!({ "id": "objects.search" }),
    );
    assert_eq!(via_http, via_mcp);
}

#[test]
fn a_declarative_object_added_to_the_repository_reaches_http_and_introspection_untouched() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/beta.v1.md",
        &rule("project.beta", 1, "Beta"),
    );
    f.commit("beta");
    let s = Served::start(&f.root(), &[]);
    let (_, list) = s.get("/api/v1/objects?kind=rule");
    assert_eq!(list["count"], 2);
    let (_, cap) = s.get("/api/v1/capability?id=rule.project.beta@1");
    assert_eq!(
        cap["provenance"]["path"],
        ".ai/repo/rules/project/beta.v1.md"
    );
    assert_eq!(
        cap["exposure"]["mcp"]["resource"]["uri"],
        "majordomus://rule/project.beta@1"
    );
    let (_, doc) = s.get("/openapi.json");
    assert!(
        !doc.to_string().contains("project.beta"),
        "a resource declares no HTTP route and gets none"
    );
    let (code, out, _) = common::run_in(
        &f.root(),
        &[
            "capabilities",
            "list",
            "--kind",
            "resource",
            "--format",
            "json",
        ],
        "",
    );
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert!(v["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == "rule.project.beta@1"));
}

#[test]
fn serving_from_a_nested_directory_finds_the_same_root_and_writes_nothing() {
    let f = Fixture::new();
    let nested = f.path("docs");
    let before = f.snapshot();
    let s = Served::start(&nested, &[]);
    let (_, repo) = s.get("/api/v1/repository");
    assert_eq!(repo["repository"]["root"], f.root().to_str().unwrap());
    let _ = s.get("/openapi.json");
    let (status, _, _) = s.request("GET", "/docs", None);
    assert_eq!(status, 200);
    let mut s = s;
    assert_eq!(s.stop(), 0, "closing stdin ends the server with 0");
    assert_eq!(f.snapshot(), before);
}

#[test]
fn outside_a_repository_serve_refuses_with_exit_12() {
    let f = Fixture::plain_dir();
    let (code, out, err) = common::run_in(&f.root(), &["serve", "--port", "0"], "");
    assert_eq!(code, 12, "{err}");
    assert!(out.is_empty());
}

#[test]
fn head_is_answered_and_a_bad_kind_filter_is_an_invalid_input() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (status, headers, body) = s.request("HEAD", "/docs", None);
    assert_eq!(status, 200);
    assert!(headers
        .iter()
        .any(|(k, v)| k == "content-type" && v.starts_with("text/html")));
    assert!(body.is_empty(), "a HEAD carries no body");
    let (status, v) = s.get("/api/v1/objects?kind=clam");
    assert_eq!(
        (status, v["error"]["code"].as_str()),
        (400, Some("invalid_input"))
    );
    assert!(
        v["error"]["message"].as_str().unwrap().contains("rule"),
        "names the kinds present: {v}"
    );
    // the document is rendered once per process and served identically afterwards
    let (_, a) = s.get("/openapi.json");
    let (_, b) = s.get("/openapi.json");
    assert_eq!(a, b);
    assert_eq!(
        a["jsonSchemaDialect"],
        "https://spec.openapis.org/oas/3.1/dialect/base"
    );
}

#[test]
fn with_stdin_at_dev_null_the_server_keeps_running() {
    let f = Fixture::new();
    let mut child = Command::new(BIN)
        .args(["serve", "--port", "0"])
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "info")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let mut lines = std::io::BufReader::new(stderr).lines();
    let mut address = None;
    for line in lines.by_ref() {
        let line = line.unwrap();
        if let Some(rest) = line.split("listening on http://").nth(1) {
            address = Some(rest.split_whitespace().next().unwrap().to_string());
            break;
        }
    }
    let address = address.expect("bound");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(
        child.try_wait().unwrap().is_none(),
        "the server exited on /dev/null stdin"
    );
    let mut stream = std::net::TcpStream::connect(&address).unwrap();
    stream
        .write_all(
            format!("GET / HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .unwrap();
    let mut raw = String::new();
    std::io::Read::read_to_string(&mut stream, &mut raw).unwrap();
    assert!(raw.starts_with("HTTP/1.1 200"), "{raw}");
    child.kill().unwrap();
    let _ = child.wait();
}

/// Every route the registry puts on the wire, replayed over a real socket with the
/// capability's own benchmark cases, bound the way a client would send them; and the
/// document's examples are exactly those cases. Driven by the registry: a route added
/// tomorrow is replayed tomorrow, and a case the document does not show is a failure here.
#[test]
fn every_route_answers_its_benchmark_cases_and_the_document_shows_them() {
    use majordomus_cli::capability::{CapabilityKind, CaseContext, HttpMethod};
    use majordomus_cli::http::Request;

    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let s = Served::start(&f.root(), &[]);
    let (_, doc) = s.get("/openapi.json");

    let cases = CaseContext { index: &ctx.index };
    let mut replayed = 0;
    for c in ctx.registry.iter() {
        let Some(http) = &c.exposure.http else {
            continue;
        };
        let op = &doc["paths"][&http.path][http.method.as_str().to_lowercase()];
        assert!(
            op.is_object(),
            "{}: {} {} is in the document",
            c.id,
            http.method.as_str(),
            http.path
        );
        let provider = ctx
            .registry
            .cases(c.id.as_str())
            .unwrap_or_else(|| panic!("{}: an executable on the wire has a case provider", c.id));
        let inputs = provider(&cases);
        assert!(
            !inputs.is_empty(),
            "{}: at least one case in this repository",
            c.id
        );
        for case in &inputs {
            let request = Request::bind(http.method, &http.path, &case.input);
            let body = (http.method == HttpMethod::Post)
                .then(|| String::from_utf8(request.body.clone()).unwrap());
            let (status, _, text) = s.request(&request.method, &request.target(), body.as_deref());
            let answer: Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("{}/{}: not JSON ({e}): {text}", c.id, case.name));
            match status {
                200 => assert!(answer.is_object(), "{}/{}: {answer}", c.id, case.name),
                // a command may turn a caller down for a reason that is not the input: over
                // plain HTTP there is no MCP session to announce for
                422 if c.kind == CapabilityKind::Command => assert_eq!(
                    answer["error"]["code"], "refused",
                    "{}/{}: {answer}",
                    c.id, case.name
                ),
                other => panic!("{}/{} answered {other}: {answer}", c.id, case.name),
            }
            replayed += 1;

            // the document shows this case, under its name, where the binding puts it
            match http.method {
                HttpMethod::Get => {
                    for (name, value) in case.input.as_object().unwrap() {
                        if value.is_null() {
                            continue;
                        }
                        let param = op["parameters"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|p| p["name"] == *name)
                            .unwrap_or_else(|| {
                                panic!("{}: parameter {name} is in the document", c.id)
                            });
                        assert_eq!(
                            param["examples"][case.name]["value"], *value,
                            "{}/{}: the example of {name}",
                            c.id, case.name
                        );
                        assert!(
                            param["schema"]["type"] != json!(["string", "null"])
                                && param["schema"].get("default") != Some(&Value::Null),
                            "{}: a query parameter is never null: {}",
                            c.id,
                            param["schema"]
                        );
                    }
                }
                HttpMethod::Post => {
                    assert_eq!(
                        op["requestBody"]["content"]["application/json"]["examples"][case.name]
                            ["value"],
                        case.input,
                        "{}/{}: the request body example",
                        c.id,
                        case.name
                    );
                }
            }
        }
        // the responses are the statuses the router maps this kind's errors to
        let responses = op["responses"].as_object().unwrap();
        for status in ["200", "400", "404", "500", "default"] {
            assert!(
                responses.contains_key(status),
                "{}: response {status}",
                c.id
            );
        }
        assert_eq!(
            responses.contains_key("422"),
            c.kind == CapabilityKind::Command,
            "{}: 422 is a command's alone",
            c.id
        );
        for (status, r) in responses {
            assert!(
                r["description"].as_str().is_some_and(|d| !d.is_empty()),
                "{}: {status} is described",
                c.id
            );
            assert!(
                r["content"]["application/json"]["schema"].is_object(),
                "{}: {status} has a schema",
                c.id
            );
        }
        assert_eq!(op["x-majordomus-benchmark"], json!(c.benchmark), "{}", c.id);
        assert_eq!(op["x-majordomus-cache"], json!(c.cache), "{}", c.id);
    }
    assert!(
        replayed >= 9,
        "the builtin routes were replayed: {replayed}"
    );

    // the document's prose is the one source: tags are the modules, info is about.rs
    let tags: BTreeSet<&str> = doc["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    let used: BTreeSet<&str> = doc["paths"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|m| m.as_object().unwrap().values())
        .flat_map(|o| {
            o["tags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap())
        })
        .collect();
    assert_eq!(
        tags, used,
        "every tag an operation uses is declared with a description, and no other"
    );
    for t in doc["tags"].as_array().unwrap() {
        let module = ctx
            .registry
            .modules()
            .find(|m| m.id.as_str() == t["name"].as_str().unwrap())
            .expect("a tag is a module");
        assert_eq!(t["description"], json!(module.description));
    }
    assert_eq!(
        doc["info"]["description"],
        json!(majordomus_cli::about::description())
    );
    assert_eq!(
        doc["info"]["summary"],
        json!(majordomus_cli::about::SUMMARY)
    );
    assert_eq!(
        doc["externalDocs"]["url"],
        json!(majordomus_cli::about::REFERENCE_URL)
    );
    assert_eq!(
        doc["info"]["license"]["identifier"],
        json!(majordomus_cli::about::LICENSE)
    );
    let (_, index) = s.get("/");
    assert_eq!(index["description"], json!(majordomus_cli::about::SUMMARY));
    assert_eq!(
        index["reference"],
        json!(majordomus_cli::about::REFERENCE_URL)
    );
}
