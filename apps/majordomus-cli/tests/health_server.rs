//! `health.report` says something true about the process serving this checkout: where its
//! shared server stands, at what address, from what version, and why when the answer is
//! not `ready` — decided by the same reading `server.status` answers from, never by a
//! second opinion written into the health module.
//!
//! What is proved here: a real server reports itself `ready` with its own address; a
//! checkout nobody serves is reported and does not make the report worse than it was; a
//! lease naming a server that answers for nobody is a warning carrying the reason; and the
//! word the check prints is the word `server.status` prints, read from the same lease.

mod common;

use common::{Fixture, Served};
use majordomus_cli::capability::builtin::server::{local_half, standing_at, ServerStanding};
use serde_json::{json, Value};

/// The check the report carries under `id`, or a failure naming what it did carry.
fn check<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap_or_else(|| panic!("the report carries checks: {report}"))
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no `{id}` check in {report}"))
}

/// Severity as `HealthStatus` orders it: the worst check decides the whole, and `unknown`
/// is worse than `warn` because an undecided dimension is not a healthy one.
fn rank(status: &Value) -> u8 {
    match status.as_str().expect("a status word") {
        "ok" => 0,
        "warn" => 1,
        "fail" => 2,
        "unknown" => 3,
        other => panic!("unknown health status {other}"),
    }
}

/// A running server reports itself, with the address it answers on and the version it
/// serves — the three facts the audit found `health.report` had never carried — and the
/// standing it prints is the one `server.status` prints from the same lease.
#[test]
fn a_served_checkout_is_named_ready_at_its_own_address() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, report) = s.get("/api/v1/health");
    assert_eq!(status, 200, "{report}");
    let server = check(&report, "server");

    assert_eq!(server["status"], "ok", "{server}");
    let detail = server["detail"].as_str().expect("a detail line");
    assert!(detail.starts_with("ready: "), "{detail}");
    assert!(
        detail.contains(&s.address),
        "the address it answers on is not in {detail}"
    );
    assert!(
        detail.contains(majordomus_cli::VERSION),
        "the version it serves is not in {detail}"
    );
    assert!(
        server.get("findings").is_none(),
        "a ready server explains nothing: {server}"
    );
    assert_eq!(server["title"], "The shared server");
    assert_eq!(
        server["evidence"][0], "majordomus serve status",
        "every check carries the command that reproduces it"
    );
    assert!(
        server["decided_by"]
            .as_str()
            .expect("an engine")
            .contains("server.status"),
        "{server}"
    );

    // the same engine, so the same word: not two readings of one lease
    let (status, standing) = s.get("/api/v1/server");
    assert_eq!(status, 200, "{standing}");
    assert_eq!(standing["standing"], "ready");
    assert!(
        detail.starts_with(&format!("{}: ", standing["standing"].as_str().unwrap())),
        "the check says {detail}, the status says {}",
        standing["standing"]
    );
    assert_eq!(
        standing["this_process"]["url"].as_str().unwrap(),
        format!("http://{}", s.address)
    );

    // and the report as a whole is no worse for having asked
    assert_eq!(
        rank(&report["status"]),
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| rank(&c["status"]))
            .max()
            .unwrap(),
        "the report's status is its worst check: {report}"
    );
}

/// A checkout nobody serves is reported as such and changes nothing about the verdict: no
/// server is not an unhealthy process, and a health report that failed over one would fail
/// every command-line invocation of this executable.
#[test]
fn a_checkout_with_no_server_is_reported_and_does_not_fail_the_report() {
    let f = Fixture::new();
    let app = common::load_app(&f);

    let report = app
        .context
        .execute("health.report", json!({}))
        .expect("health.report answers");
    let server = check(&report, "server");

    assert_eq!(server["status"], "ok", "{server}");
    let detail = server["detail"].as_str().expect("a detail line");
    assert!(detail.starts_with("absent: "), "{detail}");
    assert!(
        detail.contains("nothing serves this checkout"),
        "it says why, not only that: {detail}"
    );
    assert!(
        server.get("findings").is_none(),
        "there is nothing to explain: {server}"
    );

    // the verdict is exactly what it would have been without this check
    let worst_of_the_others = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["id"] != "server")
        .map(|c| rank(&c["status"]))
        .max()
        .expect("the report carries other checks");
    assert_eq!(
        rank(&report["status"]),
        worst_of_the_others,
        "asking about a server that is not there changed the report's verdict: {report}"
    );

    // and `server.status` agrees, from the same lease that is not there
    let standing = app
        .context
        .execute("server.status", json!({}))
        .expect("server.status answers");
    assert_eq!(standing["standing"], "absent");
}

/// A lease naming a server that does not answer is a warning carrying the reason: the next
/// election takes such a lease over, so it stops nothing, and a reader who is told the
/// address is dead does not go looking for a process that is not there.
#[test]
fn a_lease_naming_a_dead_address_is_a_warning_with_the_reason() {
    let f = Fixture::new();
    let lease = f.path(".ai/local/state/mcp/server.json");
    std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
    // port 1 on loopback: a connection there is refused at once, so this costs no timeout
    std::fs::write(
        &lease,
        format!(
            r#"{{"schema":"majordomus-mcp-lease/v1","pid":1,"token":"x","root":"{}","url":"http://127.0.0.1:1","started_at":"2026-09-10T00:00:00Z","version":"{}"}}"#,
            f.root().display(),
            majordomus_cli::VERSION
        ),
    )
    .unwrap();

    let app = common::load_app(&f);
    let report = app
        .context
        .execute("health.report", json!({}))
        .expect("health.report answers");
    let server = check(&report, "server");

    assert_eq!(server["status"], "warn", "{server}");
    let detail = server["detail"].as_str().expect("a detail line");
    assert!(detail.starts_with("stale: "), "{detail}");
    assert!(
        detail.contains("http://127.0.0.1:1"),
        "the address it read is not in {detail}"
    );
    let findings = server["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("a stale lease is a finding: {server}"));
    assert_eq!(findings.len(), 1, "{server}");
    assert!(
        findings[0].as_str().unwrap().contains("does not answer"),
        "{server}"
    );
    assert!(
        rank(&report["status"]) >= rank(&server["status"]),
        "the report is at least as bad as its worst check: {report}"
    );
}

/// The health check and the status read one lease through one function. If this ever
/// stops holding, the two surfaces have grown separate opinions of the same file, which is
/// the defect the check was added to close.
#[test]
fn the_check_and_the_status_read_the_lease_through_one_function() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let checkout = f.root().canonicalize().expect("a canonical checkout");

    let (standing, reason, lease) = standing_at(&checkout, &local_half(&checkout));
    assert_eq!(standing, ServerStanding::Ready, "{reason:?}");
    assert!(reason.is_none());
    assert_eq!(
        lease.document().expect("a lease document").url.as_deref(),
        Some(format!("http://{}", s.address).as_str())
    );

    let (_, report) = s.get("/api/v1/health");
    assert!(check(&report, "server")["detail"]
        .as_str()
        .unwrap()
        .starts_with(standing.as_str()));
}

#[test]
fn the_mesh_is_a_dimension_of_health_and_unknown_where_no_server_decided_it() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let report = app
        .context
        .execute("health.report", serde_json::json!({}))
        .expect("health.report answers");
    let mesh = report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["id"] == "mesh")
        .cloned()
        .expect("a mesh check (I2157)");
    // a command-line process activates no mesh: unknown, never a guessed ok
    assert_eq!(mesh["status"], serde_json::json!("unknown"), "{mesh}");
}

/// A committed declaration that enables the mesh, with every socket-opening transport off.
const MESH_ENABLED: &str = "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: tofu\n";

/// [`MESH_ENABLED`] as the value a server activates its mesh from.
fn declared() -> majordomus_cli::mesh::MeshConfig {
    serde_json::from_value(json!({
        "schema": "mesh/v1", "kind": "mesh-declaration", "id": "majordomus", "enabled": true,
        "multicast": { "enabled": false }, "trust": { "policy": "tofu" },
    }))
    .expect("a declaration")
}

/// A discovery provider whose start succeeds or fails as the test says, and whose status
/// says the same: a transport the mesh doctor can count as running or failed without a
/// socket being opened.
struct Transport {
    fails: bool,
}

impl majordomus_cli::mesh::MeshProvider for Transport {
    fn id(&self) -> &'static str {
        "stand_in"
    }

    fn start(
        &mut self,
        _: &majordomus_cli::mesh::ProviderContext,
    ) -> Result<(), majordomus_cli::mesh::MeshError> {
        if self.fails {
            Err(majordomus_cli::mesh::MeshError::Provider(
                "cannot bind: address in use".into(),
            ))
        } else {
            Ok(())
        }
    }

    fn status(&self) -> majordomus_cli::mesh::ProviderStatus {
        use majordomus_cli::mesh::provider::MeshProviderState;
        majordomus_cli::mesh::ProviderStatus {
            id: "stand_in".into(),
            state: if self.fails {
                MeshProviderState::Failed
            } else {
                MeshProviderState::Running
            },
            detail: self.fails.then(|| "cannot bind: address in use".into()),
            sent: 0,
            received: 0,
        }
    }
}

/// A fixture whose committed declaration enables the mesh, loaded, with its mesh activated
/// from that declaration the way a server activates it, and one transport attached.
fn activated(f: &Fixture, transport: Transport) -> majordomus_cli::app::App {
    f.write(".ai/repo/mesh/majordomus.yaml", MESH_ENABLED);
    f.commit("the mesh declaration");
    let app = common::load_app(f);
    app.context
        .mesh
        .activate_as(
            &declared(),
            std::sync::Arc::new(majordomus_cli::mesh::NodeIdentity::ephemeral().unwrap()),
            "0000000000000001",
            vec![],
            vec![],
            majordomus_cli::VERSION,
        )
        .expect("the mesh activates");
    let _ = app.context.mesh.attach(Box::new(transport));
    assert!(app.context.mesh.status().active);
    app
}

/// With the mesh enabled and running, the health report's mesh dimension is the mesh
/// doctor's verdict on it: ok, decided by the doctor's runtime check, and saying what runs.
#[test]
fn an_enabled_mesh_that_runs_is_a_healthy_dimension_as_the_doctor_says() {
    let f = Fixture::new();
    let app = activated(&f, Transport { fails: false });
    let status = app.context.mesh.status();
    let verdict =
        majordomus_cli::mesh::doctor::runtime_verdict(Some(Ok(declared())), Some(&status));
    assert!(verdict.ok, "the doctor passes a running mesh: {verdict:?}");

    let report = app
        .context
        .execute("health.report", json!({}))
        .expect("health.report answers");
    let mesh = check(&report, "mesh");
    assert_eq!(mesh["status"], "ok", "{mesh}");
    assert_eq!(
        mesh["detail"],
        json!(verdict.detail),
        "the dimension says what the doctor says"
    );
    let detail = mesh["detail"].as_str().unwrap();
    assert!(
        detail.starts_with("active as ") && detail.contains("1 of 1 provider(s) running"),
        "{detail}"
    );
    assert_eq!(
        mesh["decided_by"],
        "mesh doctor's runtime check, over this server's mesh.status"
    );
    assert!(
        mesh.get("findings").is_none(),
        "a healthy mesh explains nothing: {mesh}"
    );
    app.context.mesh.stop();
}

/// With the mesh enabled and active but every transport it runs failed, the doctor fails it,
/// and so does the health report's mesh dimension — naming the failed transport and carrying
/// the doctor's remedy — and the report as a whole is no better than that failure.
#[test]
fn an_enabled_mesh_whose_every_transport_failed_fails_the_dimension_as_the_doctor_does() {
    let f = Fixture::new();
    let app = activated(&f, Transport { fails: true });
    let status = app.context.mesh.status();
    let verdict =
        majordomus_cli::mesh::doctor::runtime_verdict(Some(Ok(declared())), Some(&status));
    assert!(
        !verdict.ok,
        "the doctor fails a mesh that hears nothing: {verdict:?}"
    );

    let report = app
        .context
        .execute("health.report", json!({}))
        .expect("health.report answers");
    let mesh = check(&report, "mesh");
    assert_eq!(mesh["status"], "fail", "{mesh}");
    assert_eq!(mesh["detail"], json!(verdict.detail), "{mesh}");
    let detail = mesh["detail"].as_str().unwrap();
    assert!(
        detail.contains("0 of 1 provider(s) running")
            && detail.contains("failed: stand_in (cannot bind: address in use)"),
        "the failed transport is named: {detail}"
    );
    let findings = mesh["findings"]
        .as_array()
        .unwrap_or_else(|| panic!("a failed mesh carries the remedy: {mesh}"));
    assert_eq!(
        findings,
        &verdict
            .remediation
            .iter()
            .map(|r| json!(r))
            .collect::<Vec<_>>(),
        "the remedy is the doctor's"
    );
    assert!(
        rank(&report["status"]) >= rank(&json!("fail")),
        "the report is at least as bad as its failed mesh: {report}"
    );
    app.context.mesh.stop();
}
