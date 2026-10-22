//! No answer, log line or mesh frame carries a credential the redactor knows (I2161).
//!
//! An error echoes what it was asked — a path, a tool name, an argument — and a handover is
//! replicated in clear to every linked runtime. Each of those now passes the one redaction rule
//! set (`redaction::redact_secrets`, the port of the capture table) before it leaves the process.

mod common;

use std::sync::Arc;

use common::Fixture;
use majordomus_cli::http::mcp::McpEndpoint;
use majordomus_cli::http::{Request, Router};
use serde_json::{json, Value};

/// A credential's shape, assembled at run time so that no committed file carries one.
fn token() -> String {
    format!("{}{}", "ghp_", "a".repeat(36))
}

#[test]
fn an_http_error_does_not_echo_a_credential() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let router = Router::new(app.context.clone(), "test");
    let path = format!("/api/v1/{}", token());
    let response = router.handle(&Request::parse_target("GET", &path, vec![]));
    assert_eq!(response.status, 404);
    let text = response.body.text();
    assert!(!text.contains(&token()), "{text}");
    assert!(text.contains("[redacted:github-token]"), "{text}");
}

#[test]
fn an_mcp_error_does_not_echo_a_credential() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let endpoint = Arc::new(McpEndpoint::new(
        app.context.clone(),
        "test",
        "http://127.0.0.1:0".into(),
    ));
    let post = |session: Option<&str>, message: Value| {
        let mut req = Request::parse_target("POST", "/mcp", message.to_string().into_bytes());
        if let Some(id) = session {
            req = req.with_headers(vec![("Mcp-Session-Id".into(), id.into())]);
        }
        endpoint.handle(&req)
    };
    let opened = post(
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "unit", "version": "0" } } }),
    );
    let session = opened
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
        .map(|(_, v)| v.clone())
        .expect("a session");
    let reply = post(
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": token(), "arguments": {} } }),
    );
    let text = reply.body.text();
    assert!(!text.contains(&token()), "{text}");
}

#[test]
fn a_handover_body_is_redacted_before_it_is_signed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("20261009T120000Z--x.md");
    std::fs::write(
        &path,
        format!(
            "---\ntask_id: t-1\n---\n\n# Objective\nship it\n# Current State\npushed with {}\n# Next Action\nnone\n",
            token()
        ),
    )
    .unwrap();
    let body = majordomus_cli::mesh::handover::to_body(&path, None, None).unwrap();
    assert!(!body.body.contains(&token()), "{}", body.body);
    assert!(body.body.contains("[redacted:github-token]"));
    assert_eq!(
        body.id,
        majordomus_cli::mesh::journal::HandoverBody::digest_of(&body.body),
        "the id names the body that travels"
    );
}

/// Run the executable in `cwd` with the distribution's share and debug logging, so that every
/// request the server it starts answers is logged.
fn mj(cwd: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let out = common::majordomus()
        .args(args)
        .current_dir(cwd)
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env("MAJORDOMUS_LOG", "debug")
        .output()
        .expect("run majordomus");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A credential-shaped value carried by a request — in its path, its query and its `Origin`
/// — reaches neither the answer nor `server.log`, the file a server `serve ensure` starts
/// writes its log to. The server logs at debug here, so every request it answers is logged,
/// and the log must carry the redaction marker where the value was: the fields were logged,
/// and logged redacted, rather than never logged at all.
#[test]
fn credential_never_echoed_test() {
    let f = Fixture::new();
    let (code, out, err) = mj(
        &f.root(),
        &[
            "serve", "ensure", "--format", "json", "--wait", "40", "--idle", "120",
        ],
    );
    assert_eq!(code, 0, "{out}\n{err}");
    let ensured: Value = serde_json::from_str(&out).expect("ensure prints JSON");
    assert_eq!(ensured["standing"], "ready", "{ensured}");
    let url = ensured["url"].as_str().expect("an address").to_string();
    let log = f.root().join(".ai/local/state/mcp/server.log");
    assert_eq!(
        ensured["log"].as_str().map(std::path::PathBuf::from),
        Some(log.clone()),
        "the server logs to server.log beside its lease"
    );

    let secret = token();
    let ask = |method: &str, target: &str, headers: &[(&str, &str)]| {
        majordomus_cli::mcp::bridge::request(
            &url,
            method,
            target,
            headers,
            (method == "POST").then_some("{}"),
            std::time::Duration::from_secs(30),
        )
        .expect("the server answers")
    };
    // the path and the query of an unknown route
    let missing = ask(
        "GET",
        &format!("/api/v1/{secret}?access_token={secret}"),
        &[],
    );
    assert_eq!(missing.status, 404, "{}", missing.body);
    assert!(!missing.body.contains(&secret), "{}", missing.body);
    // a state-changing request from a foreign origin, refused and logged with that origin
    let origin = format!("http://{secret}.example");
    let foreign = ask(
        "POST",
        &format!("/api/v1/{secret}"),
        &[("Origin", origin.as_str())],
    );
    assert_eq!(foreign.status, 403, "{}", foreign.body);
    assert!(!foreign.body.contains(&secret), "{}", foreign.body);

    let (code, out, err) = mj(&f.root(), &["serve", "stop"]);
    assert_eq!(code, 0, "{out}{err}");
    let text = std::fs::read_to_string(&log).expect("the server wrote its log");
    assert!(
        !text.contains(&secret),
        "server.log carries the credential:\n{text}"
    );
    assert!(
        text.contains("a state-changing request from another origin was refused")
            && text.contains("[redacted:github-token]"),
        "the requests were logged, with the credential redacted:\n{text}"
    );
}

/// Every backticked Rust path in `{src}/{rel}`'s module documentation resolves to a module of
/// the crate and, below it, to a function or type defined there; each file path names a
/// file of the repository; each `MJ_`/`mj_` name is defined in `lib/capture.sh`.
fn named_in_module_doc(src: &std::path::Path, rel: &str) -> Vec<(String, std::path::PathBuf)> {
    let repo = src.join("../../..");
    // the module documentation's prose: its example, fenced, is code and names nothing
    let mut fenced = false;
    let doc: String = std::fs::read_to_string(src.join(rel))
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("//!"))
        .filter(|l| {
            if l.trim_start().starts_with("```") {
                fenced = !fenced;
                return false;
            }
            !fenced
        })
        .collect::<Vec<_>>()
        .join(" ");
    let capture = std::fs::read_to_string(repo.join("lib/capture.sh")).unwrap();
    let mut paths = Vec::new();
    for span in doc.split('`').skip(1).step_by(2) {
        if span.contains("::") {
            let segments: Vec<&str> = span.split("::").collect();
            let resolved = (1..=segments.len()).rev().find_map(|k| {
                let module = segments[..k].join("/");
                [format!("{module}.rs"), format!("{module}/mod.rs")]
                    .into_iter()
                    .map(|f| src.join(f))
                    .find(|f| f.is_file())
                    .map(|f| (f, &segments[k..]))
            });
            let (file, items) =
                resolved.unwrap_or_else(|| panic!("`{span}` names no module of the crate"));
            let text = std::fs::read_to_string(&file).unwrap();
            let defines = |item: &str| {
                [
                    format!("fn {item}("),
                    format!("struct {item} "),
                    format!("enum {item} "),
                ]
                .iter()
                .any(|d| text.contains(d.as_str()))
            };
            match items {
                [] => {}
                [item] => assert!(defines(item), "`{span}`: no {item} in {}", file.display()),
                [owner, method] => assert!(
                    text.contains(&format!("impl {owner} ")) && defines(method),
                    "`{span}`: no {owner}::{method} in {}",
                    file.display()
                ),
                _ => panic!("`{span}` is deeper than a module, a type and a method"),
            }
            if let Some(item) = items.last() {
                paths.push((item.to_string(), file));
            }
        } else if span.contains('/') && !span.contains(' ') {
            assert!(
                repo.join(span).exists(),
                "`{span}` is not a file of the repository"
            );
        } else if span.starts_with("MJ_") || span.starts_with("mj_") {
            assert!(
                capture.contains(&format!("{span}=")) || capture.contains(&format!("{span}()")),
                "`{span}` is not defined in lib/capture.sh"
            );
        }
    }
    paths
}

/// The functions the line at `index` of `lines` is inside, innermost first: each `fn` above it
/// that is indented less than the one before, so a helper nested in a function names both.
fn enclosing_fns(lines: &[&str], index: usize) -> Vec<String> {
    let mut found = Vec::new();
    let mut indent = usize::MAX;
    for l in lines[..=index].iter().rev() {
        let depth = l.len() - l.trim_start().len();
        if depth >= indent {
            continue;
        }
        let name = ["fn ", "pub fn ", "pub(crate) fn ", "pub(super) fn "]
            .iter()
            .find_map(|p| l.trim_start().strip_prefix(p))
            .and_then(|rest| {
                rest.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .next()
            });
        if let Some(name) = name {
            found.push(name.to_string());
            indent = depth;
            if depth == 0 {
                break;
            }
        }
    }
    found
}

/// The redaction module's documentation names only callers that exist, and every caller
/// that exists: each item it names resolves in the crate, and each function of the crate
/// that calls `redact_secrets` or `public_text` is one it names (I2161).
#[test]
fn the_redaction_module_names_only_and_all_of_its_callers() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let named = named_in_module_doc(&src, "redaction.rs");
    assert!(
        named.iter().any(|(item, _)| item == "error_response"),
        "{named:?}"
    );

    let mut callers = Vec::new();
    let mut publishers = Vec::new();
    let mut pending = vec![src.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") || path == src.join("redaction.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                if code.contains("redaction::redact_secrets(") {
                    let within = enclosing_fns(&lines, i);
                    assert!(
                        !within.is_empty(),
                        "a call outside a function at {}",
                        path.display()
                    );
                    callers.push((within, path.clone()));
                }
                if code.contains("redaction::public_text(") {
                    publishers.push(format!("{}:{}", path.display(), i + 1));
                }
            }
        }
    }
    assert!(!callers.is_empty(), "no caller found: the scan is broken");
    for (within, file) in &callers {
        assert!(
            named
                .iter()
                .any(|(item, f)| f == file && within.contains(item)),
            "{} in {} calls the redactor and the module documentation of redaction.rs does not name it",
            within.join(" in "),
            file.display()
        );
    }
    // and the module says nothing calls `public_text` yet (I1955 wires it)
    assert!(
        publishers.is_empty(),
        "`public_text` is called at {publishers:?}: the module documentation says nothing does"
    );
}
