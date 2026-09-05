//! The repository scope, end to end: a repository declaring none is read under the
//! distribution's default; one declaring its own is read under that; discovery drops a
//! source outside it and says why; a tracked secret is reported; `repository.scope_classify` judges
//! by name, size and content through the executor, the command line, HTTP and MCP; a
//! malformed declaration refuses to start, naming the file and the key.

mod common;

use std::sync::Arc;

use common::{run_in, Fixture, Served};
use majordomus_cli::capability::builtin::SCOPE_URI;
use majordomus_cli::mcp::Surface;
use serde_json::{json, Value};

const OWN_SCOPE: &str = "version: 1
in:
  - .ai/manifest.yaml
  - .ai/README.md
  - .ai/repo/**
  - docs/**
  - lib/**
  - test/**
  - README.md
out:
  paths:
    - .git/
    - .ai/local/
    - '**/target/'
  binary: true
  max_bytes: 2048
  image:
    names: ['*.png']
  secret:
    names: ['.env', '*.pem']
  fixtures:
    paths: ['**/fixtures/']
    max_bytes: 16
";

fn with_own_scope(f: &Fixture) {
    let manifest = common::MANIFEST.replace(
        "  policy: repo/policy.yaml\n",
        "  policy: repo/policy.yaml\n  scope: repo/scope.yaml\n",
    );
    f.write(".ai/manifest.yaml", &manifest);
    f.write(".ai/repo/scope.yaml", OWN_SCOPE);
    let sources = common::SOURCES.replace(
        "  - id: profile\n",
        "  - id: scope\n    kind: scope\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/scope.yaml'\n    required: false\n\n  - id: profile\n",
    );
    f.write(".ai/repo/knowledge/sources.yaml", &sources);
}

#[test]
fn a_repository_declaring_no_scope_is_read_under_the_distribution_default() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let info = app.context.execute("repository.scope", json!({})).unwrap();
    assert_eq!(info["origin"], "distribution");
    assert!(info["path"]
        .as_str()
        .unwrap()
        .ends_with("skeleton/ai/repo/scope.yaml"));
    assert_eq!(info["declaration"]["version"], 1);
    assert_eq!(
        info["tracked"]["files"],
        f.git(&["ls-files"]).lines().count()
    );
    // the fixture is made of files the default admits, except its projections
    let out: Vec<&str> = info["tracked"]["out_files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["path"].as_str().unwrap())
        .collect();
    assert!(out.contains(&".gitignore"), "{out:?}");
    assert_eq!(
        app.index().repository.scope_origin,
        majordomus_cli::scope::Origin::Distribution
    );
    // the report as the index carries it is what every projection answers
    let report = app.context.execute("repository.info", json!({})).unwrap();
    assert_eq!(report["repository"]["scope_origin"], "distribution");
}

#[test]
fn the_repository_scope_governs_discovery_and_names_what_it_drops() {
    let f = Fixture::new();
    with_own_scope(&f);
    // a document outside `in`, a fixture over its limit, a secret, an image and a binary
    // all tracked and all claimed by a source class or the tally
    f.write("docs/big.md", &format!("# Big\n\n{}", "x".repeat(4096)));
    f.write(
        "test/fixtures/large.json",
        "{\"padding\": \"0123456789\"}\n",
    );
    f.write("test/fixtures/small.json", "{}\n");
    f.write("docs/.env", "TOKEN=1\n");
    f.write_bytes("docs/logo.png", b"\x89PNG\r\n\x1a\n\x00");
    f.write_bytes("docs/blob.md", b"# blob\n\x00\x01\x02");
    f.write("CONTRIBUTING.md", "# Contributing\n");
    f.commit("scope");
    let app = common::load_app(&f);
    let index = app.index();
    assert_eq!(
        index.repository.scope_origin,
        majordomus_cli::scope::Origin::Repository
    );
    assert_eq!(index.repository.scope_path, ".ai/repo/scope.yaml");
    let dropped: Vec<(String, String)> = index
        .diagnostics
        .iter()
        .filter(|d| d.code == "out_of_scope")
        .map(|d| (d.path.clone().unwrap(), d.message.clone()))
        .collect();
    let dropped_paths: Vec<&str> = dropped.iter().map(|(p, _)| p.as_str()).collect();
    assert!(dropped_paths.contains(&"docs/big.md"), "{dropped:?}");
    assert!(dropped_paths.contains(&"docs/blob.md"), "{dropped:?}");
    assert!(dropped_paths.contains(&"CONTRIBUTING.md"), "{dropped:?}");
    let big = dropped.iter().find(|(p, _)| p == "docs/big.md").unwrap();
    assert!(
        big.1.contains("over_limit") && big.1.contains("max_bytes 2048"),
        "{}",
        big.1
    );
    let blob = dropped.iter().find(|(p, _)| p == "docs/blob.md").unwrap();
    assert!(blob.1.contains("binary"), "{}", blob.1);
    let undeclared = dropped
        .iter()
        .find(|(p, _)| p == "CONTRIBUTING.md")
        .unwrap();
    assert!(undeclared.1.contains("undeclared"), "{}", undeclared.1);
    // nothing dropped became an object; what is in did
    assert!(index.get("majordomus://document/docs/big.md").is_none());
    assert!(index.get("majordomus://document/docs/CLI.md").is_some());
    // the declaration is itself an object of the layer, read as a resource
    let scope_object = index.get("majordomus://scope/.ai/repo/scope.yaml").unwrap();
    assert_eq!(scope_object.kind, "scope");
    // the tracked secret is a warning naming the rule; the index is not degraded by it
    let secret = index
        .diagnostics
        .iter()
        .find(|d| d.code == "tracked_secret")
        .expect("the tracked secret is reported");
    assert_eq!(secret.path.as_deref(), Some("docs/.env"));
    assert!(secret.message.contains(".env"));
    assert_eq!(index.state, majordomus_cli::index::State::Ok);
    // the tally counts every tracked file, by name and size
    let info = app.context.execute("repository.scope", json!({})).unwrap();
    let by = &info["tracked"]["by_reason"];
    assert_eq!(by["secret"], 1);
    assert_eq!(by["image"], 1);
    assert_eq!(by["over_limit"], 1);
    assert_eq!(by["fixture_over_limit"], 1);
    assert!(by["undeclared"].as_u64().unwrap() >= 1);
    assert_eq!(
        info["tracked"]["in"].as_u64().unwrap() + info["tracked"]["out"].as_u64().unwrap(),
        info["tracked"]["files"].as_u64().unwrap()
    );
}

#[test]
fn classify_judges_by_name_then_size_then_content_through_every_projection() {
    let f = Fixture::new();
    with_own_scope(&f);
    f.write(
        "test/fixtures/large.json",
        "{\"padding\": \"0123456789\"}\n",
    );
    f.write_bytes("docs/blob.md", b"# blob\n\x00\x01\x02");
    f.commit("scope");
    let app = common::load_app(&f);
    let ctx = &app.context;
    let judge = |path: &str| {
        ctx.execute("repository.scope_classify", json!({ "path": path }))
            .unwrap()
    };

    let v = judge("docs/CLI.md");
    assert_eq!(v["verdict"], "in");
    assert_eq!(v["rule"], "docs/**");
    assert_eq!(v["exists"], true);
    assert!(v["bytes"].as_u64().unwrap() > 0);
    assert!(v.get("reason").is_none());

    let v = judge(".ai/local/state/current.yaml");
    assert_eq!(
        (v["verdict"].as_str(), v["reason"].as_str()),
        (Some("out"), Some("path"))
    );
    assert_eq!(v["rule"], ".ai/local/");

    let v = judge("docs/secret.pem");
    assert_eq!(v["reason"], "secret");
    assert_eq!(
        v["exists"], false,
        "judged by name, whether or not it exists"
    );

    assert!(ctx
        .execute(
            "repository.scope_classify",
            json!({ "path": "docs/../docs/blob.md" })
        )
        .is_err());
    let v = judge("./docs/blob.md");
    assert_eq!(v["path"], "docs/blob.md");
    assert_eq!(v["reason"], "binary");

    let v = judge("test/fixtures/large.json");
    assert_eq!(v["reason"], "fixture_over_limit");
    assert!(v["rule"]
        .as_str()
        .unwrap()
        .contains("fixtures.max_bytes 16"));

    let v = judge("docs");
    assert_eq!(v["verdict"], "in");
    assert_eq!(v["directory"], true);

    let v = judge("Cargo.toml");
    assert_eq!(v["reason"], "undeclared");
    assert!(v.get("rule").is_none());

    for bad in ["/etc/passwd", "../x", "", "C:/x"] {
        let e = ctx
            .execute("repository.scope_classify", json!({ "path": bad }))
            .unwrap_err();
        assert!(
            matches!(
                e,
                majordomus_cli::capability::CapabilityError::InvalidInput(_)
            ),
            "{bad}: {e}"
        );
    }

    // the MCP tool and resource
    let surface = Surface::new(Arc::clone(ctx));
    let read = surface.read(SCOPE_URI).unwrap();
    let doc: Value = serde_json::from_str(&read.text).unwrap();
    assert_eq!(doc["origin"], "repository");
    let tools = surface.tools();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names.contains(&"majordomus_scope") && names.contains(&"majordomus_scope_classify"),
        "{names:?}"
    );

    // the HTTP routes
    let mut s = Served::start(&f.root(), &[]);
    let (status, v) = s.get("/api/v1/scope/classify?path=docs/secret.pem");
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["reason"], "secret");
    let (status, v) = s.get("/api/v1/scope");
    assert_eq!(status, 200);
    assert_eq!(v["origin"], "repository");
    let (status, v) = s.get("/api/v1/scope/classify?path=../x");
    assert_eq!(status, 400, "{v}");
    s.stop();

    // the command line: the report, one path, several with --check
    let (code, out, _) = run_in(&f.root(), &["scope"], "");
    assert_eq!(code, 0);
    assert!(out.contains(".ai/repo/scope.yaml (repository)"), "{out}");
    assert!(out.contains("tracked"), "{out}");
    let (code, out, _) = run_in(&f.root(), &["scope", "docs/CLI.md", "docs/secret.pem"], "");
    assert_eq!(code, 0);
    assert!(out.contains("in ") && out.contains("out  secret"), "{out}");
    let (code, out, _) = run_in(
        &f.root(),
        &[
            "scope",
            "--check",
            "--format",
            "json",
            "docs/CLI.md",
            "docs/secret.pem",
        ],
        "",
    );
    assert_eq!(code, 10, "{out}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
    let (code, _, _) = run_in(&f.root(), &["scope", "--check", "docs/CLI.md"], "");
    assert_eq!(code, 0);
    let (code, _, err) = run_in(&f.root(), &["scope", "/etc/passwd"], "");
    assert_ne!(code, 0);
    assert!(err.contains("absolute"), "{err}");
}

#[test]
fn a_malformed_declaration_refuses_to_start_naming_the_file() {
    let f = Fixture::new();
    with_own_scope(&f);
    f.write(
        ".ai/repo/scope.yaml",
        &OWN_SCOPE.replace("  binary: true\n", "  binary: true\n  colour: red\n"),
    );
    f.commit("bad key");
    let (code, _, err) = run_in(&f.root(), &["scope"], "");
    assert_eq!(code, 10, "{err}");
    assert!(
        err.contains(".ai/repo/scope.yaml") && err.contains("colour"),
        "{err}"
    );

    f.write(
        ".ai/repo/scope.yaml",
        &OWN_SCOPE.replace("  - docs/**\n", "  - /docs/**\n"),
    );
    f.commit("absolute pattern");
    let (code, _, err) = run_in(&f.root(), &["scope"], "");
    assert_eq!(code, 10, "{err}");
    assert!(
        err.contains("/docs/**") && err.contains("relative"),
        "{err}"
    );

    f.write(
        ".ai/repo/scope.yaml",
        &OWN_SCOPE.replace("version: 1", "version: 2"),
    );
    f.commit("version");
    let (code, _, err) = run_in(&f.root(), &["scope"], "");
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("version 2 is not 1"), "{err}");

    // a manifest naming the section without the file: the file is named, not defaulted
    f.remove(".ai/repo/scope.yaml");
    f.commit("absent");
    let (code, _, err) = run_in(&f.root(), &["scope"], "");
    assert_ne!(code, 0);
    assert!(err.contains("scope.yaml"), "{err}");
}
