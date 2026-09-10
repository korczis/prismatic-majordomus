//! The server's state on a surface: the lease typed and read once, and every server of a
//! git repository listed from any checkout of it — the primary and its linked worktrees
//! each with a lease of their own, the same repository named on each, and a server that
//! stops turning `absent` on the next reading.
//!
//! And the narrower question beside it: `checkouts=this` answers for the checkout the
//! reading is made in and enumerates no other, while agreeing with the wide list about
//! that one, because a caller must never have to choose between the cheap answer and the
//! true one.

mod common;

use std::time::Duration;

use common::{Fixture, Served};
use majordomus_cli::capability::builtin::server::{standing_of, ServerStanding};
use majordomus_cli::lease::{LeaseDocument, LeaseFile, SCHEMA};

#[test]
fn the_standing_is_decided_from_the_lease_the_probe_and_the_version() {
    let lease = |url: Option<&str>, version: Option<&str>| LeaseDocument {
        schema: SCHEMA.into(),
        pid: 1,
        token: "t".into(),
        root: "/r".into(),
        url: url.map(str::to_string),
        started_at: "2026-09-10T00:00:00Z".into(),
        executable: None,
        version: version.map(str::to_string),
    };
    let young = Duration::ZERO;
    let old = Duration::from_secs(3600);
    let never = |_: &str| false;
    let always = |_: &str| true;
    let addressed = lease(Some("http://127.0.0.1:1"), Some(majordomus_cli::VERSION));

    // nothing, something binding, something that gave up binding
    assert_eq!(
        standing_of(&LeaseFile::Absent, young, never, majordomus_cli::VERSION),
        (ServerStanding::Absent, None)
    );
    assert_eq!(
        standing_of(&LeaseFile::Empty, young, never, majordomus_cli::VERSION).0,
        ServerStanding::Starting
    );
    assert_eq!(
        standing_of(&LeaseFile::Empty, old, never, majordomus_cli::VERSION).0,
        ServerStanding::Stale
    );
    let (standing, reason) = standing_of(
        &LeaseFile::Document(lease(None, None)),
        old,
        never,
        majordomus_cli::VERSION,
    );
    assert_eq!(standing, ServerStanding::Stale);
    assert!(reason.unwrap().contains("abandoned"));

    // a file that is not a lease says what it is
    let (standing, reason) = standing_of(
        &LeaseFile::Corrupt("not JSON".into()),
        young,
        always,
        majordomus_cli::VERSION,
    );
    assert_eq!(standing, ServerStanding::Stale);
    assert_eq!(reason.as_deref(), Some("corrupt lease: not JSON"));

    // an address nobody answers at, then one answered at this version
    let (standing, reason) = standing_of(
        &LeaseFile::Document(addressed.clone()),
        young,
        never,
        majordomus_cli::VERSION,
    );
    assert_eq!(standing, ServerStanding::Stale);
    assert!(reason.unwrap().contains("does not answer"));
    assert_eq!(
        standing_of(
            &LeaseFile::Document(addressed),
            young,
            always,
            majordomus_cli::VERSION
        ),
        (ServerStanding::Ready, None)
    );

    // answered from another version, and from a server that wrote none down
    let (standing, reason) = standing_of(
        &LeaseFile::Document(lease(Some("http://127.0.0.1:1"), Some("0.0.1"))),
        young,
        always,
        majordomus_cli::VERSION,
    );
    assert_eq!(standing, ServerStanding::Outdated);
    assert!(reason.unwrap().contains("0.0.1"));
    let (standing, reason) = standing_of(
        &LeaseFile::Document(lease(Some("http://127.0.0.1:1"), None)),
        young,
        always,
        majordomus_cli::VERSION,
    );
    assert_eq!(standing, ServerStanding::Outdated);
    assert!(reason.unwrap().contains("no version"));
}

#[test]
fn every_checkout_of_a_repository_lists_every_server_of_it() {
    let f = Fixture::new();
    // a linked worktree at the topology's canonical path for its branch
    let wt = f.container().join("feature").join("x");
    std::fs::create_dir_all(f.container()).unwrap();
    f.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "feature/x",
        wt.to_str().unwrap(),
    ]);
    let wt = wt.canonicalize().unwrap();

    let primary = Served::start(&f.root(), &[]);
    let mut linked = Served::start(&wt, &[]);

    // the index route names the checkout and the repository it belongs to
    let (status, a) = primary.get("/");
    assert_eq!(status, 200);
    let (_, b) = linked.get("/");
    assert_ne!(
        a["repository_id"], b["repository_id"],
        "two checkouts, two identities"
    );
    assert_eq!(
        a["git_repository_id"], b["git_repository_id"],
        "one git repository behind both"
    );
    assert!(a["git_repository_id"].is_string());
    assert_eq!(a["linked_worktree"], false);
    assert_eq!(b["linked_worktree"], true);

    // the status, from the primary's server: itself ready, the other one too
    let (status, s) = primary.get("/api/v1/server");
    assert_eq!(status, 200, "{s}");
    assert_eq!(s["standing"], "ready");
    assert_eq!(s["checkout_id"], a["repository_id"]);
    assert_eq!(s["git"]["id"], a["git_repository_id"]);
    assert_eq!(s["git"]["linked"], false);
    assert_eq!(s["desired"]["version"], majordomus_cli::VERSION);
    assert_eq!(
        s["this_process"]["url"].as_str().unwrap(),
        format!("http://{}", primary.address),
        "the lease this process holds is the one it published"
    );
    assert_eq!(s["this_process"]["version"], majordomus_cli::VERSION);
    assert!(
        s["this_process"].get("token").is_none(),
        "the token stays in the file"
    );
    let servers = s["servers"].as_array().unwrap();
    assert_eq!(servers.len(), 2, "{s}");
    assert_eq!(servers[0]["primary"], true);
    assert_eq!(servers[0]["this_checkout"], true);
    assert_eq!(servers[0]["standing"], "ready");
    assert_eq!(servers[0]["checkout_id"], a["repository_id"]);
    assert_eq!(servers[1]["primary"], false);
    assert_eq!(servers[1]["this_checkout"], false);
    assert_eq!(servers[1]["branch"], "feature/x");
    assert_eq!(servers[1]["standing"], "ready", "{}", servers[1]);
    assert_eq!(servers[1]["checkout_id"], b["repository_id"]);
    assert_eq!(
        servers[1]["lease"]["url"].as_str().unwrap(),
        format!("http://{}", linked.address)
    );
    assert_eq!(
        servers[1]["peers"], 0,
        "a `serve` with no MCP session holds no peer"
    );

    // and from the linked worktree's server, the same two, seen from the other side
    let (_, t) = linked.get("/api/v1/server");
    assert_eq!(t["standing"], "ready");
    assert_eq!(t["git"]["id"], a["git_repository_id"]);
    assert_eq!(t["git"]["linked"], true);
    let seen = t["servers"].as_array().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[0]["this_checkout"], false,
        "the primary is listed first, and it is not this one"
    );
    assert_eq!(seen[1]["this_checkout"], true);

    // the narrow question, from the primary: this checkout alone, and the same answer
    // about it that the wide list gave
    let (status, narrow) = primary.get("/api/v1/server?checkouts=this");
    assert_eq!(status, 200, "{narrow}");
    let only = narrow["servers"].as_array().unwrap();
    assert_eq!(only.len(), 1, "this checkout alone: {narrow}");
    assert_eq!(only[0]["this_checkout"], true);
    for field in [
        "worktree",
        "branch",
        "checkout_id",
        "primary",
        "standing",
        "peers",
    ] {
        assert_eq!(
            only[0][field], servers[0][field],
            "the two questions disagree about {field}: {narrow}"
        );
    }
    assert_eq!(narrow["standing"], s["standing"]);
    assert_eq!(narrow["checkout_id"], s["checkout_id"]);
    assert_eq!(
        narrow["git"]["id"], s["git"]["id"],
        "the repository is named either way"
    );
    assert_eq!(
        narrow["this_process"]["url"], s["this_process"]["url"],
        "the lease this process holds does not depend on how wide the question was"
    );

    // and from the linked worktree, where `primary` cannot be inferred from being first
    let (_, narrow_linked) = linked.get("/api/v1/server?checkouts=this");
    let only = narrow_linked["servers"].as_array().unwrap();
    assert_eq!(only.len(), 1, "{narrow_linked}");
    assert_eq!(
        only[0]["primary"], false,
        "a linked work tree is not the primary: {narrow_linked}"
    );
    assert_eq!(only[0]["branch"], "feature/x");
    assert_eq!(only[0]["this_checkout"], true);
    for field in ["worktree", "checkout_id", "primary", "standing", "branch"] {
        assert_eq!(
            only[0][field], seen[1][field],
            "the two questions disagree about {field}: {narrow_linked}"
        );
    }

    // a word that is not one of the two is refused by name rather than guessed at
    let (status, refused) = primary.get("/api/v1/server?checkouts=everything");
    assert_eq!(status, 400, "{refused}");

    // the linked worktree's server stops: its lease is released and the next reading says so
    assert_eq!(linked.stop(), 0);
    let (_, s) = primary.get("/api/v1/server");
    let servers = s["servers"].as_array().unwrap();
    assert_eq!(servers[1]["standing"], "absent", "{}", servers[1]);
    assert!(servers[1].get("lease").is_none());
    assert_eq!(servers[0]["standing"], "ready");
}

#[test]
fn a_lease_naming_a_server_of_another_checkout_is_stale_and_says_so() {
    let f = Fixture::new();
    let other = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    // the other repository's lease points at this server: it answers, but not for that root
    let lease = other.path(".ai/local/state/mcp/server.json");
    std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
    std::fs::write(
        &lease,
        format!(
            r#"{{"schema":"majordomus-mcp-lease/v1","pid":1,"token":"x","root":"{}","url":"http://{}","started_at":"2026-09-10T00:00:00Z","version":"{}"}}"#,
            other.root().display(),
            served.address,
            majordomus_cli::VERSION
        ),
    )
    .unwrap();
    let served_other = Served::start(&other.root(), &[]);
    // the other repository's own `serve` found the lease stale, took it over and now
    // answers for its root; its status says the primary of *that* repository is ready
    let (_, s) = served_other.get("/api/v1/server");
    assert_eq!(s["standing"], "ready", "{s}");
    assert_ne!(s["git"]["id"], {
        let (_, mine) = served.get("/");
        mine["git_repository_id"].clone()
    });
}

#[test]
fn a_process_that_serves_nothing_holds_no_lease() {
    // read in process: the capability answers from a context that never published
    let f = Fixture::new();
    let app = common::load_app(&f);
    let value = app
        .context
        .execute("server.status", serde_json::json!({}))
        .expect("server.status answers");
    assert_eq!(value["standing"], "absent");
    assert!(value.get("this_process").is_none());
    assert_eq!(value["servers"].as_array().unwrap().len(), 1);
    assert_eq!(value["servers"][0]["this_checkout"], true);
    assert_eq!(value["servers"][0]["primary"], true);

    // asking nothing is asking for the repository, and in a checkout that is the only one
    // of its repository the narrow answer is the same answer
    let narrow = app
        .context
        .execute("server.status", serde_json::json!({ "checkouts": "this" }))
        .expect("server.status answers the narrow question");
    assert_eq!(narrow, value);
    let wide = app
        .context
        .execute(
            "server.status",
            serde_json::json!({ "checkouts": "repository" }),
        )
        .expect("server.status answers the wide question");
    assert_eq!(wide, value, "the default is the wide question");
    app.context
        .execute(
            "server.status",
            serde_json::json!({ "checkouts": "everything" }),
        )
        .expect_err("a word that is not one of the two is refused");
}
