//! Two things a running server decides that no other test reached: an unbounded `serve`
//! whose lease is taken over ends by itself once nobody is attached, and the health report
//! of a process that decided the mesh reads the mesh doctor's runtime verdict.

mod common;

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use common::{dist_share, Fixture};
use majordomus_cli::lease::LeaseFile;
use serde_json::{json, Value};

fn lease_path(f: &Fixture) -> std::path::PathBuf {
    f.path(".ai/local/state/mcp/server.json")
}

fn wait_until(what: &str, timeout: Duration, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{what} did not happen within {timeout:?}");
}

/// Write `text` over the lease the way another elector would: whole, by a rename.
fn replace_lease(path: &Path, text: &str) {
    let tmp = path.with_extension("successor.tmp");
    std::fs::write(&tmp, text).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

/// A server started with no pipe and no idle bound runs until it is stopped, unless its lease
/// is taken over: then, with no session left to hand on, it has nothing to serve and ends by
/// itself, leaving the successor's lease where it is (I2127).
#[test]
fn an_unbounded_server_whose_lease_is_taken_over_ends_by_itself() {
    let f = Fixture::new();
    let log = f.path("serve.log");
    let mut child = common::majordomus()
        .args(["serve", "--port", "0", "--idle", "0"])
        .current_dir(f.root())
        .env("MAJORDOMUS_SHARE", dist_share())
        .env("MAJORDOMUS_LOG", "info")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&log).unwrap())
        .spawn()
        .expect("spawn serve");
    let lease = lease_path(&f);
    wait_until(
        "the server publishes its address",
        Duration::from_secs(60),
        || matches!(LeaseFile::read(&lease), LeaseFile::Document(d) if d.url.is_some()),
    );
    let successor = json!({
        "schema": "majordomus-mcp-lease/v1",
        "pid": std::process::id(),
        "token": "the-successor",
        "root": f.root(),
        "url": "http://127.0.0.1:9",
        "started_at": "2026-10-12T00:00:00Z",
    })
    .to_string();
    replace_lease(&lease, &successor);

    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait for serve") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "the superseded server did not end:\n{}",
                std::fs::read_to_string(&log).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(status.success(), "{status:?}\n{text}");
    assert!(
        text.contains("this server lost the lease and its last session was handed on; stopping"),
        "it ended because it was superseded and alone:\n{text}"
    );
    assert_eq!(
        LeaseFile::read(&lease).document().map(|d| d.token.clone()),
        Some("the-successor".to_string()),
        "the successor's lease is left where it is"
    );
}

/// The `mesh` check of a health report run in this process.
fn mesh_check(app: &majordomus_cli::app::App) -> Value {
    let report = app
        .context
        .execute("health.report", json!({}))
        .expect("health.report answers");
    report["checks"]
        .as_array()
        .expect("the report carries checks")
        .iter()
        .find(|c| c["id"] == "mesh")
        .expect("the report carries the mesh check")
        .clone()
}

/// Once a server has decided the mesh, the health report reads the mesh doctor's verdict on
/// that decision: an undeclared mesh that is off is as declared, and its reason is said.
#[test]
fn a_decided_mesh_that_nothing_declares_is_reported_off_as_declared() {
    let f = Fixture::new();
    // the executor caches a report, so the undecided answer is asked of an app of its own
    let before = mesh_check(&common::load_app(&f));
    assert_eq!(before["status"], "unknown", "nothing decided yet: {before}");

    let app = common::load_app(&f);
    app.context.mesh.decline("the node identity did not load");
    let check = mesh_check(&app);
    assert_eq!(check["status"], "ok", "{check}");
    let detail = check["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("declares no mesh") && detail.contains("the node identity did not load"),
        "{check}"
    );
    assert_eq!(
        check["decided_by"],
        "mesh doctor's runtime check, over this server's mesh.status"
    );
}

/// A committed declaration that enables the mesh, and a server that could not activate it:
/// the health report fails the mesh check and says what to do.
#[test]
fn a_decided_mesh_that_is_declared_but_not_active_fails_the_check() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/mesh/majordomus.yaml",
        "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: tofu\n",
    );
    f.commit("the mesh declaration");
    let app = common::load_app(&f);
    app.context.mesh.decline("the node identity did not load");
    let check = mesh_check(&app);
    assert_eq!(check["status"], "fail", "{check}");
    let detail = check["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("the declaration is enabled and this server's mesh is not active")
            && detail.contains("the node identity did not load"),
        "{check}"
    );
    assert!(
        check["findings"].as_array().is_some_and(|f| f
            .iter()
            .any(|x| x.as_str().is_some_and(|x| x.contains("restart the server")))),
        "the remedy is a finding: {check}"
    );
}
