//! `plan.collisions` through the built executable (I4775, ADR 0238): an issue id authored on
//! two branches under two titles is `id_reused` naming the other branch, the same record on
//! two refs is none, a retitled record is a warning, an open pull request that executes the
//! head's issue refuses it unless a supersession is declared, a train's title claims none of
//! its members, a planning-only head executes nothing, and a pull-request gate with no forge
//! reading cannot judge.

mod common;

use common::{run_in, Fixture, Served, ISSUE};
use serde_json::{json, Value};

fn record(id: &str, title: &str) -> String {
    ISSUE
        .replace("id: I0001", &format!("id: {id}"))
        .replace("slug: issue-I0001", &format!("slug: issue-{id}"))
        .replace(
            "title: The bounded piece of work",
            &format!("title: {title}"),
        )
}

fn trunk(f: &Fixture) -> String {
    f.git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string()
}

fn collisions(f: &Fixture, args: &[&str]) -> (i32, Value, String) {
    let mut argv = vec!["plan", "collisions"];
    argv.extend_from_slice(args);
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}\n{err}"));
    (code, v, err)
}

fn classes(v: &Value) -> Vec<(String, String, String)> {
    v["collisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["class"].as_str().unwrap().to_string(),
                c["verdict"].as_str().unwrap().to_string(),
                c["holder"]["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn observe(f: &Fixture, pulls: Value) {
    f.write(
        ".ai/repo/ci/plan.yaml",
        "version: 1\ndrift:\n  window: 60\n  enforcement: advisory\ncollisions:\n  max_age: 3000000000\n",
    );
    f.write(
        ".ai/local/state/integration/observation.json",
        &json!({
            "schema": 4, "repository": "someone/fixture", "base": "master",
            "base_sha": "0000000000000000000000000000000000000000",
            "observed_at": "2026-11-25T08:00:00Z", "required_checks": null,
            "review_policy": null, "merge_methods": ["merge"], "pull_requests": pulls
        })
        .to_string(),
    );
}

fn pull(number: u64, head_ref: &str, title: &str, body: &str) -> Value {
    json!({
        "number": number, "title": title, "author": "someone", "head_ref": head_ref,
        "head_sha": format!("{number:040}"), "base_ref": "master", "draft": false,
        "created_at": "2026-11-25T07:00:00Z", "updated_at": "2026-11-25T07:30:00Z", "body": body
    })
}

#[test]
fn an_id_authored_on_two_branches_under_two_titles_is_refused_naming_the_other() {
    let f = Fixture::new();
    let t = trunk(&f);
    f.git(&["checkout", "-q", "-b", "other/the-rival"]);
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &record("I0002", "The rival's work"),
    );
    f.commit("docs(plan): the rival's I0002");
    f.git(&["checkout", "-q", &t]);
    f.git(&["checkout", "-q", "-b", "feature/I0002-mine"]);
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &record("I0002", "My work"),
    );
    f.write("lib/mine.sh", "echo mine\n");
    f.commit("feat: my work (I0002)");

    let (code, v, err) = collisions(&f, &["--gate", "push"]);
    assert_eq!(code, 10, "{v}\n{err}");
    assert_eq!(v["verdict"], "refused");
    assert_eq!(v["authors"], json!(["I0002"]));
    assert_eq!(v["executes"], json!(["I0002"]));
    let found = classes(&v);
    assert!(
        found.contains(&(
            "id_reused".into(),
            "refused".into(),
            "other/the-rival".into()
        )),
        "{found:?}"
    );
    let reused = &v["collisions"][0];
    assert_eq!(reused["holder"]["title"], "The rival's work");
    assert!(reused["remedy"]
        .as_str()
        .unwrap()
        .contains("mesh allocate issue"));
    let source = |name: &str| {
        v["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == name)
            .cloned()
            .unwrap()
    };
    assert_eq!(source("forge")["state"], "unread");
    assert_eq!(source("history")["state"], "read");

    // a report is never a gate
    let (code, v, _) = collisions(&f, &[]);
    assert_eq!((code, v["verdict"].as_str()), (0, Some("refused")));

    // the same record carried on another branch is no collision; the text says what it read
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &record("I0002", "The rival's work"),
    );
    f.commit("docs(plan): adopt the rival's title");
    let (code, v, _) = collisions(&f, &["--gate", "push"]);
    assert_eq!(code, 0, "{v}");
    assert!(!classes(&v).iter().any(|c| c.0 == "id_reused"), "{v}");
    let (_, out, _) = run_in(&f.root(), &["plan", "collisions"], "");
    assert!(out.contains("executes: I0002 · authors: I0002"), "{out}");
    assert!(out.contains("plan collisions: "), "{out}");
}

#[test]
fn an_open_pull_request_executing_the_issue_refuses_unless_one_supersedes_the_other() {
    let f = Fixture::new();
    let t = trunk(&f);
    f.git(&["checkout", "-q", "-b", "fix/I0001-elsewhere"]);
    f.write("lib/elsewhere.sh", "echo elsewhere\n");
    f.commit("wip elsewhere");
    f.git(&["checkout", "-q", &t]);
    f.git(&["checkout", "-q", "-b", "feature/issue-I0001"]);
    f.write("lib/mine.sh", "echo mine\n");
    f.commit("feat: mine");

    // no forge reading: a pull request gate cannot judge, a push gate warns
    let (code, v, _) = collisions(&f, &["--gate", "pr"]);
    assert_eq!((code, v["verdict"].as_str()), (12, Some("unjudged")), "{v}");
    let (code, v, _) = collisions(&f, &["--gate", "push"]);
    assert_eq!(code, 0, "{v}");
    assert!(
        classes(&v).contains(&(
            "claimed_elsewhere".into(),
            "warned".into(),
            "fix/I0001-elsewhere".into()
        )),
        "an unmerged branch alone only warns: {v}"
    );

    observe(
        &f,
        json!([
            pull(9, "fix/I0001-elsewhere", "fix: elsewhere", ""),
            pull(
                11,
                "integration/train",
                "Train: I0001, I0002, I0003, I0004",
                ""
            )
        ]),
    );
    let (code, v, err) = collisions(&f, &["--gate", "start", "--issue", "I0001"]);
    assert_eq!(code, 10, "{v}\n{err}");
    let refused = &v["collisions"][0];
    assert_eq!(refused["class"], "claimed_elsewhere");
    assert_eq!(refused["holder"]["pull_request"], 9);
    assert!(refused["remedy"].as_str().unwrap().contains("--alongside"));
    assert_eq!(
        v["collisions"].as_array().unwrap().len(),
        1,
        "the train claims none of its members, and a branch with a pull request is counted once: {v}"
    );

    // this head's own pull request declares that it supersedes #9
    observe(
        &f,
        json!([
            pull(9, "fix/I0001-elsewhere", "fix: elsewhere", ""),
            pull(12, "feature/issue-I0001", "feat: mine", "Supersedes #9")
        ]),
    );
    let (code, v, _) = collisions(&f, &["--gate", "pr"]);
    assert_eq!((code, v["verdict"].as_str()), (0, Some("clean")), "{v}");
    let (_, out, _) = run_in(&f.root(), &["plan", "collisions", "--issue", "I0001"], "");
    assert!(out.contains("plan collisions: clean"), "{out}");

    // the other pull request says this head's replaces it; one that replaces this head's
    // is no reason to proceed
    observe(
        &f,
        json!([
            pull(
                9,
                "fix/I0001-elsewhere",
                "fix: elsewhere",
                "Superseded by #12"
            ),
            pull(12, "feature/issue-I0001", "feat: mine", "")
        ]),
    );
    let (code, v, _) = collisions(&f, &["--gate", "pr"]);
    assert_eq!((code, v["verdict"].as_str()), (0, Some("clean")), "{v}");
    observe(
        &f,
        json!([
            pull(9, "fix/I0001-elsewhere", "fix: elsewhere", "Supersedes #12"),
            pull(12, "feature/issue-I0001", "feat: mine", "")
        ]),
    );
    let (code, _, _) = collisions(&f, &["--gate", "pr"]);
    assert_eq!(code, 10);
}

#[test]
fn another_sessions_live_claim_on_the_issue_refuses_and_the_callers_own_does_not() {
    let f = Fixture::new();
    let state = f.parent().join("xdg-state");
    let key = majordomus_cli::mesh::identity::NodeIdentity::load_or_create(
        &state.join("majordomus").join("node.json"),
    )
    .expect("a node identity")
    .public
    .public_key;
    f.write(
        ".ai/repo/mesh/majordomus.yaml",
        &format!(
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n\
             multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n    - {key}\n\
             cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: fixture\n"
        ),
    );
    let mut served = Served::start_with_env(
        &f.root(),
        &["--discovery", "filesystem"],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    );
    let post = |path: &str, body: Value| -> (u16, Value) {
        let (status, _, text) = served.request("POST", path, Some(&body.to_string()));
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    };
    let (status, opened) = post(
        "/api/v1/mesh/sessions",
        json!({"session": "s1", "client": "test", "intent": "the work", "issue": "I0001"}),
    );
    assert_eq!(status, 200, "{opened}");
    let (status, claimed) = post(
        "/api/v1/mesh/claims",
        json!({"session": "s1", "scope": ["lib"], "issue": "I0001", "intent": "the work"}),
    );
    assert_eq!(status, 200, "{claimed}");
    let (status, theirs) = served.get("/api/v1/plan/collisions?issue=I0001&gate=start");
    let (_, own) = served.get("/api/v1/plan/collisions?issue=I0001&gate=start&session=s1");
    // a command line asks the checkout's server for the claims
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "plan",
            "collisions",
            "--issue",
            "I0001",
            "--gate",
            "start",
            "--format",
            "json",
        ],
        "",
    );
    served.stop();
    assert_eq!(status, 200, "{theirs}");
    assert_eq!(theirs["verdict"], "refused", "{theirs}");
    let held = &theirs["collisions"][0];
    assert_eq!(held["holder"]["kind"], "claim");
    assert!(
        held["holder"]["session"]
            .as_str()
            .is_some_and(|s| s.ends_with("/s1")),
        "{held}"
    );
    assert_eq!(
        own["verdict"], "clean",
        "the caller's own claim is its own: {own}"
    );
    let cli: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}{err}"));
    let claims = cli["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "claims")
        .cloned()
        .unwrap();
    if claims["state"] == "read" {
        assert_eq!(
            (code, cli["verdict"].as_str()),
            (10, Some("refused")),
            "{cli}"
        );
    } else {
        // the command line found no server record for this checkout; say so rather than pass
        assert_eq!(code, 0, "{cli}");
        assert!(
            claims["detail"].as_str().is_some_and(|d| !d.is_empty()),
            "{cli}"
        );
    }
}

#[test]
fn a_planning_only_head_executes_nothing_and_a_retitle_is_a_warning() {
    let f = Fixture::new();
    f.git(&["checkout", "-q", "-b", "plan/I0001-records"]);
    let text = std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).unwrap();
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &text.replace("The bounded piece of work", "Another issue written over it"),
    );
    f.commit("docs(plan): records");
    let (code, v, _) = collisions(&f, &["--gate", "pr"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["planning_only"], true);
    assert_eq!(v["executes"], json!([]), "recording is not executing");
    assert_eq!(v["verdict"], "warned");
    let retitled = &v["collisions"][0];
    assert_eq!(retitled["class"], "retitled");
    assert_eq!(retitled["holder"]["title"], "The bounded piece of work");

    let (code, _, err) = run_in(&f.root(), &["plan", "collisions", "--issue", "I9999"], "");
    assert_ne!(code, 0);
    assert!(err.contains("no issue 'I9999'"), "{err}");
    let (code, _, err) = run_in(
        &f.root(),
        &["plan", "collisions", "--head", "no-such-rev"],
        "",
    );
    assert_ne!(code, 0);
    assert!(err.contains("does not resolve"), "{err}");
}
