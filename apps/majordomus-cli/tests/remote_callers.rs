//! A caller on another host reads, and changes nothing it cannot authenticate (ADR 0126).
//!
//! On 2026-10-09 three servers on the owner's MacBook listened on every interface: the
//! mesh hub on 8791 and two servers of the primary checkout, bound there by
//! `MAJORDOMUS_HTTP_HOST=0.0.0.0`. The only guard on a state-changing request was the
//! `Origin` check, which stops a browser and nothing else. Any host on the LAN or the
//! tailnet could transition a plan record, run any capability through
//! `executions.start`, or post a mesh claim that this node then signed with its own trusted
//! key and replicated to every runtime of the repository.
//!
//! The rule these tests hold: a request from an address that is not loopback runs a
//! capability only when the capability reads, or when its input is a signed message the
//! handler verifies. It is decided once, from the capability's declared execution policy,
//! and every transport that knows the caller's address asks the same question.

mod common;

use std::net::{IpAddr, UdpSocket};
use std::sync::Arc;

use common::Fixture;
use majordomus_cli::capability::{CapabilityKind, Effect};
use majordomus_cli::http::mcp::McpEndpoint;
use majordomus_cli::http::{server, Request, Router};
use serde_json::{json, Value};

const LAN: &str = "192.168.100.30";
const TAILNET: &str = "100.92.246.32";

fn router(f: &Fixture) -> (majordomus_cli::app::App, Router, Arc<McpEndpoint>) {
    let app = common::load_app(f);
    let endpoint = Arc::new(McpEndpoint::new(
        app.context.clone(),
        "test",
        "http://127.0.0.1:0".into(),
    ));
    let router = Router::new(app.context.clone(), "test").with_mcp(Arc::clone(&endpoint));
    (app, router, endpoint)
}

fn from(address: &str, req: Request) -> Request {
    req.with_remote(Some(address.parse::<IpAddr>().unwrap()))
}

fn body(response: &majordomus_cli::http::router::Response) -> Value {
    serde_json::from_str(&response.body.text()).unwrap_or(Value::Null)
}

#[test]
fn every_route_that_changes_something_is_refused_to_another_host() {
    let f = Fixture::new();
    let (app, router, _) = router(&f);
    let mut refused = 0;
    for c in app.registry().iter() {
        let Some(http) = &c.exposure.http else {
            continue;
        };
        if c.execution.admits_remote() {
            continue;
        }
        for address in [LAN, TAILNET, "::ffff:10.0.0.2"] {
            let req = from(address, Request::bind(http.method, &http.path, &json!({})));
            let response = router.handle(&req);
            assert_eq!(
                response.status,
                403,
                "{} {} from {address} must be refused before it runs: {}",
                c.id,
                http.path,
                response.body.text()
            );
            assert_eq!(
                body(&response)["error"]["code"],
                json!("forbidden"),
                "{}",
                c.id
            );
        }
        refused += 1;
    }
    // the four the incident named, so that a registry change which silently made them
    // remote-admissible fails here by name and not only by count
    for path in [
        "/api/v1/plan/transition",
        "/api/v1/executions/start",
        "/api/v1/mesh/claims",
        "/api/v1/mesh/handovers/consume",
    ] {
        let c = app
            .registry()
            .iter()
            .find(|c| c.exposure.http.as_ref().is_some_and(|h| h.path == path))
            .unwrap_or_else(|| panic!("{path} is a route"));
        assert!(
            !c.execution.admits_remote(),
            "{path} must not admit a remote caller"
        );
    }
    assert!(
        refused >= 20,
        "only {refused} routes were refused; the registry has more writers than that"
    );
}

#[test]
fn the_entry_that_can_start_a_writer_is_announced_as_one() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let start = app
        .registry()
        .iter()
        .find(|c| c.id.as_str() == "executions.start")
        .expect("executions.start is a capability");
    // a local client asks before a tool that writes; it must ask before this one too
    assert_eq!(start.execution.effect, Effect::RepositoryMutation);
    assert!(start.execution.hints().destructive);
}

#[test]
fn a_remote_caller_still_reads() {
    let f = Fixture::new();
    let (app, router, _) = router(&f);
    let c = app
        .registry()
        .iter()
        .find(|c| c.id.as_str() == "repository.info")
        .expect("repository.info is a capability");
    assert_eq!(c.execution.effect, Effect::Read);
    let http = c
        .exposure
        .http
        .as_ref()
        .expect("repository.info has a route");
    let response = router.handle(&from(
        LAN,
        Request::bind(http.method, &http.path, &json!({})),
    ));
    assert_eq!(response.status, 200, "{}", response.body.text());
}

#[test]
fn the_same_write_from_loopback_is_not_refused() {
    let f = Fixture::new();
    let (_, router, _) = router(&f);
    let announce = |address: &str| {
        router.handle(&from(
            address,
            Request::bind(
                majordomus_cli::capability::HttpMethod::Post,
                "/api/v1/peers/announce",
                &json!({ "intent": "x" }),
            ),
        ))
    };
    // no MCP session behind a plain HTTP call, so the handler refuses on its own terms;
    // what matters is that it was reached, and that the remote one was not
    let local = announce("127.0.0.1");
    assert_ne!(local.status, 403, "{}", local.body.text());
    let v6 = announce("::1");
    assert_ne!(v6.status, 403, "{}", v6.body.text());
    assert_eq!(announce(LAN).status, 403);
}

#[test]
fn a_signed_link_message_from_another_host_reaches_its_handler() {
    let f = Fixture::new();
    let (app, router, _) = router(&f);
    let signed: Vec<_> = app
        .registry()
        .iter()
        .filter(|c| c.execution.signed_input)
        .collect();
    let ids: Vec<&str> = signed.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        ["mesh.link.hello", "mesh.link.sync", "mesh.register"],
        "a signed input is declared by exactly the handlers that verify one"
    );
    for c in signed {
        assert_eq!(c.kind, CapabilityKind::Command, "{}", c.id);
        assert_eq!(
            c.execution.effect,
            Effect::ProcessState,
            "{}: a signature never admits a write to the repository",
            c.id
        );
        let http = c
            .exposure
            .http
            .as_ref()
            .expect("a link handler has a route");
        let response = router.handle(&from(
            LAN,
            Request::bind(http.method, &http.path, &json!({ "body": {}, "sig": "" })),
        ));
        assert_ne!(
            response.status,
            403,
            "{} is admitted by its signature, not refused by the caller's address: {}",
            c.id,
            response.body.text()
        );
    }
}

fn mcp(
    endpoint: &McpEndpoint,
    address: &str,
    session: Option<&str>,
    message: Value,
) -> (u16, Value, Option<String>) {
    let mut req = Request::parse_target("POST", "/mcp", message.to_string().into_bytes());
    if let Some(id) = session {
        req = req.with_headers(vec![("Mcp-Session-Id".into(), id.into())]);
    }
    let response = endpoint.handle(&from(address, req));
    let id = response
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
        .map(|(_, v)| v.clone());
    (response.status, body(&response), id)
}

fn call(id: u64, tool: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": arguments } })
}

#[test]
fn an_mcp_session_is_judged_by_where_each_message_comes_from() {
    let f = Fixture::new();
    let (_, _, endpoint) = router(&f);
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "unit", "version": "0" } }
    });
    // a remote client may open a session and read
    let (status, _, session) = mcp(&endpoint, LAN, None, init);
    assert_eq!(status, 200);
    let session = session.expect("a session id");
    let (_, read, _) = mcp(
        &endpoint,
        LAN,
        Some(&session),
        call(2, "majordomus_repository", json!({})),
    );
    assert_eq!(read["result"]["isError"], json!(false), "{read}");

    // and is refused a tool that changes something, with the word HTTP answers
    let (_, write, _) = mcp(
        &endpoint,
        LAN,
        Some(&session),
        call(3, "majordomus_announce", json!({ "intent": "x" })),
    );
    assert_eq!(write["result"]["isError"], json!(true), "{write}");
    assert_eq!(
        write["result"]["_meta"]["majordomus"]["error"]["code"],
        json!("forbidden"),
        "{write}"
    );

    // the session id is a name, not a proof: the same session from loopback is this machine
    let (_, local, _) = mcp(
        &endpoint,
        "127.0.0.1",
        Some(&session),
        call(4, "majordomus_announce", json!({ "intent": "x" })),
    );
    assert_eq!(local["result"]["isError"], json!(false), "{local}");

    // and a remote message after it is refused again: nothing is remembered from the last one
    let (_, again, _) = mcp(
        &endpoint,
        TAILNET,
        Some(&session),
        call(5, "majordomus_announce", json!({ "intent": "y" })),
    );
    assert_eq!(
        again["result"]["_meta"]["majordomus"]["error"]["code"],
        json!("forbidden"),
        "{again}"
    );
}

/// An address of this machine that is not loopback: the source address the kernel would use
/// to reach a documentation-only address. Connecting a UDP socket sends nothing.
fn own_non_loopback_address() -> IpAddr {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("a UDP socket");
    socket
        .connect("192.0.2.1:9")
        .expect("this machine has a route off loopback; the socket test needs one");
    let ip = socket.local_addr().expect("a local address").ip();
    assert!(
        !ip.is_loopback() && !ip.is_unspecified(),
        "{ip} is not an address off loopback"
    );
    ip
}

#[test]
fn the_socket_says_where_a_request_came_from() {
    let f = Fixture::new();
    let (_, router, _) = router(&f);
    let bound = server::bind("0.0.0.0", 0).expect("a free port on every interface");
    let port = bound
        .address()
        .rsplit(':')
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .expect("the bound port");
    let running = bound.start(router);
    let post = |host: String| {
        majordomus_cli::mcp::bridge::request(
            &format!("http://{host}:{port}"),
            "POST",
            "/api/v1/peers/announce",
            &[("content-type", "application/json")],
            Some(r#"{"intent":"x"}"#),
            std::time::Duration::from_secs(10),
        )
        .expect("the server answers")
    };
    let remote = post(own_non_loopback_address().to_string());
    let local = post("127.0.0.1".into());
    running.stop();
    assert_eq!(remote.status, 403, "{}", remote.body);
    assert_ne!(local.status, 403, "{}", local.body);
}

#[test]
fn a_loopback_request_under_a_foreign_host_name_reads_nothing() {
    let f = Fixture::new();
    let (_, router, _) = router(&f);
    let get = |from: &str, host: Option<&str>| {
        let mut req = Request::bind(
            majordomus_cli::capability::HttpMethod::Get,
            "/api/v1/repository",
            &json!({}),
        );
        if let Some(h) = host {
            req = req.with_headers(vec![("Host".into(), h.into())]);
        }
        router.handle(&self::from(from, req))
    };
    // a rebinding page: this machine's browser, the page's own domain
    let rebound = get("127.0.0.1", Some("attacker.example:8741"));
    assert_eq!(rebound.status, 403, "{}", rebound.body.text());
    assert_eq!(body(&rebound)["error"]["code"], json!("forbidden"));
    // the same machine under its own names and addresses, and a program that names no host
    for host in [
        Some("127.0.0.1:8741"),
        Some("localhost:8741"),
        Some("[::1]:8741"),
        None,
    ] {
        assert_eq!(get("127.0.0.1", host).status, 200, "{host:?}");
    }
    // another host may name this machine as it knows it; ADR 0126 already lets it only read
    assert_eq!(get(LAN, Some("macbook.local:57547")).status, 200);
}

#[test]
fn the_api_page_runs_only_the_scripts_it_ships() {
    let f = Fixture::new();
    let (_, router, _) = router(&f);
    let page = router.handle(&Request::bind(
        majordomus_cli::capability::HttpMethod::Get,
        "/swagger",
        &json!({}),
    ));
    assert_eq!(page.status, 200);
    let policy = page
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-security-policy"))
        .map(|(_, v)| v.clone())
        .expect("a Content-Security-Policy");
    assert!(policy.contains("frame-ancestors 'none'"), "{policy}");
    assert!(!policy.contains("unsafe-eval"), "{policy}");
    let html = page.body.text();
    assert_eq!(
        html.matches("integrity=\"sha384-").count(),
        2,
        "both CDN files are pinned by hash"
    );
}
