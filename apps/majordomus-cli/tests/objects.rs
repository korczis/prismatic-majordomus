//! One resolution of a `majordomus://` URI: `objects.get`, the MCP resource read and the
//! HTTP object route answer a declarative object as the file read, and a URI a query
//! projects (`majordomus://repository`) as that query's answer, tagged so a client can
//! tell which it got. Every branch of the resolution is exercised here at the library
//! level: the object, the answer, the unknown URI, the registry naming an object the index
//! lacks, a query that refuses, and a query that fails. The tail of the file covers what
//! is left of the module and the surface: a search stopping at its limit, and a tool whose
//! handler fails internally.

mod common;

use std::sync::Arc;

use common::Fixture;
use majordomus_cli::capability;
use majordomus_cli::capability::builtin::{
    self, resolve, Empty, Resolved, ResourceView, REPOSITORY_URI,
};
use majordomus_cli::capability::{
    CapabilityError, CapabilityKind, CapabilityRegistry, CaseContext, Context, Exposure,
    McpExposure, McpResource, Stability,
};
use majordomus_cli::mcp::surface::SurfaceError;
use majordomus_cli::mcp::Surface;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Value};

#[test]
fn the_repository_uri_answers_repository_info_as_a_tagged_document() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = &app.context;
    let report = ctx.execute("repository.info", json!({})).unwrap();
    let doc = ctx
        .execute("objects.get", json!({ "uri": REPOSITORY_URI }))
        .unwrap();
    assert_eq!(doc["source"], "builtin");
    assert_eq!(doc["uri"], REPOSITORY_URI);
    assert_eq!(doc["id"], "repository.info");
    assert_eq!(doc["kind"], "query");
    assert_eq!(doc["identity"], "repository");
    assert_eq!(doc["title"], "Repository and index state");
    assert!(doc["description"].as_str().unwrap().contains("diagnostic"));
    assert_eq!(doc["provenance"]["source"], "builtin");
    assert_eq!(doc["media_type"], "application/json");
    assert_eq!(doc["answer"], report, "the answer is repository.info's");
    let parsed: Value = serde_json::from_str(doc["content"].as_str().unwrap()).unwrap();
    assert_eq!(parsed, report, "the text is the same report");
    // the typed view round-trips through its own schema
    let view: ResourceView = serde_json::from_value(doc.clone()).unwrap();
    assert!(matches!(view, ResourceView::Builtin(ref a) if a.kind == CapabilityKind::Query));
    // the MCP resource read is the same resolution: same text, same media type
    let surface = Surface::new(Arc::clone(ctx));
    let read = surface.read(REPOSITORY_URI).unwrap();
    assert_eq!(read.media_type, "application/json");
    assert_eq!(read.text, doc["content"]);
}

#[test]
fn a_declarative_object_is_the_file_read_tagged_declarative() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = &app.context;
    let uri = "majordomus://rule/project.alpha@1";
    let doc = ctx.execute("objects.get", json!({ "uri": uri })).unwrap();
    assert_eq!(doc["source"], "declarative");
    assert_eq!(doc["id"], "rule.project.alpha@1");
    assert_eq!(
        doc["provenance"]["path"],
        ".ai/repo/rules/project/alpha.v1.md"
    );
    assert_eq!(doc["content"], common::rule("project.alpha", 1, "Alpha"));
    let view: ResourceView = serde_json::from_value(doc.clone()).unwrap();
    assert!(matches!(view, ResourceView::Declarative(ref o) if o.identity == "project.alpha@1"));
    let resolved = resolve(ctx, uri).unwrap();
    assert!(matches!(resolved, Resolved::Object(o) if o.uri == uri));
    assert_eq!(resolved.media_type(), "text/markdown");
    let read = Surface::new(Arc::clone(ctx)).read(uri).unwrap();
    assert_eq!(
        (read.media_type.as_str(), read.text),
        ("text/markdown", resolved.text())
    );
}

#[test]
fn an_unknown_uri_is_not_found_on_every_projection() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = &app.context;
    let err = ctx
        .execute("objects.get", json!({ "uri": "majordomus://rule/none@1" }))
        .unwrap_err();
    assert!(
        matches!(&err, CapabilityError::NotFound(m) if m.contains("majordomus://rule/none@1")),
        "{err}"
    );
    let err = Surface::new(Arc::clone(ctx))
        .read("majordomus://rule/none@1")
        .unwrap_err();
    assert!(
        matches!(&err, SurfaceError::UnknownResource(u) if u == "majordomus://rule/none@1"),
        "{err}"
    );
}

#[test]
fn a_registry_naming_an_object_the_index_lacks_is_an_internal_error() {
    // registry from a repository holding alpha, index from one holding beta: a state the
    // application never composes, and the one that must not be answered as "not found"
    let a = Fixture::new();
    let with_alpha = common::load_app(&a);
    let b = Fixture::new();
    b.remove(".ai/repo/rules/project/alpha.v1.md");
    b.write(
        ".ai/repo/rules/project/beta.v1.md",
        &common::rule("project.beta", 1, "Beta"),
    );
    b.commit("beta");
    let with_beta = common::load_app(&b);
    let ctx = Context::new(
        Arc::clone(&with_beta.context.index),
        Arc::clone(&with_alpha.context.registry),
    );
    let err = ctx
        .execute(
            "objects.get",
            json!({ "uri": "majordomus://rule/project.alpha@1" }),
        )
        .unwrap_err();
    assert!(
        matches!(&err, CapabilityError::Internal(m) if m.contains("rule.project.alpha@1") && m.contains("index")),
        "{err}"
    );
    let err = Surface::new(Arc::new(ctx))
        .read("majordomus://rule/project.alpha@1")
        .unwrap_err();
    assert!(matches!(err, SurfaceError::Internal(_)), "{err}");
}

/// Nothing.
#[derive(Serialize, JsonSchema)]
struct Nothing {}

fn refusing(_: &Context, _: Empty) -> Result<Nothing, CapabilityError> {
    Err(CapabilityError::Refused("not today".into()))
}

fn failing(_: &Context, _: Empty) -> Result<Nothing, CapabilityError> {
    Err(CapabilityError::NotFound("its own thing".into()))
}

fn broken(_: &Context, _: Empty) -> Result<Nothing, CapabilityError> {
    Err(CapabilityError::Internal("a bug".into()))
}

fn as_resource(uri: &str) -> Exposure {
    Exposure {
        mcp: Some(McpExposure {
            tool: None,
            resource: Some(McpResource {
                uri: uri.into(),
                name: "demo".into(),
            }),
        }),
        http: None,
        cli: None,
    }
}

/// The builtins plus two queries read as resources (one refuses, one fails) and one tool
/// whose handler fails internally.
fn context_with_demo_resources(f: &Fixture) -> Arc<Context> {
    let app = common::load_app(f);
    let mut executables = builtin::all();
    executables.push(capability! {
        id: "demo.refusing", title: "Refusing", description: "Refuses.",
        input: Empty, output: Nothing, stability: Stability::Experimental,
        exposure: as_resource("majordomus://demo/refusing"), tags: ["demo"],
        handler: refusing,
    });
    executables.push(capability! {
        id: "demo.failing", title: "Failing", description: "Fails.",
        input: Empty, output: Nothing, stability: Stability::Experimental,
        exposure: as_resource("majordomus://demo/failing"), tags: ["demo"],
        handler: failing,
    });
    executables.push(capability! {
        id: "demo.broken", title: "Broken", description: "Fails internally, as a tool.",
        input: Empty, output: Nothing, stability: Stability::Experimental,
        exposure: Exposure { mcp: Some(McpExposure { tool: Some("demo_broken".into()), resource: None }), http: None, cli: None },
        tags: ["demo"],
        handler: broken,
    });
    let registry = CapabilityRegistry::builder()
        .with_builtin(executables)
        .with_index(app.index())
        .build()
        .unwrap();
    Arc::new(Context::new(
        Arc::clone(&app.context.index),
        Arc::new(registry),
    ))
}

#[test]
fn a_query_read_as_a_resource_passes_its_refusal_through_and_its_failure_is_internal() {
    let f = Fixture::new();
    let ctx = context_with_demo_resources(&f);
    let err = ctx
        .execute(
            "objects.get",
            json!({ "uri": "majordomus://demo/refusing" }),
        )
        .unwrap_err();
    assert_eq!(err, CapabilityError::Refused("not today".into()));
    let err = ctx
        .execute("objects.get", json!({ "uri": "majordomus://demo/failing" }))
        .unwrap_err();
    assert!(
        matches!(&err, CapabilityError::Internal(m)
            if m.contains("majordomus://demo/failing") && m.contains("demo.failing") && m.contains("its own thing")),
        "{err}"
    );
    // through the resource read both are internal: the URI exists, the read could not answer
    let surface = Surface::new(Arc::clone(&ctx));
    for uri in ["majordomus://demo/refusing", "majordomus://demo/failing"] {
        let err = surface.read(uri).unwrap_err();
        assert!(matches!(err, SurfaceError::Internal(_)), "{uri}: {err}");
    }
}

#[test]
fn every_benchmark_case_of_objects_get_answers_and_one_is_the_repository() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = &app.context;
    let cases = ctx.registry.cases("objects.get").unwrap()(&CaseContext { index: &ctx.index });
    let names: Vec<&str> = cases.iter().map(|c| c.name).collect();
    assert_eq!(names, ["first-object", "repository"]);
    for c in &cases {
        let doc = ctx
            .execute("objects.get", c.input.clone())
            .unwrap_or_else(|e| panic!("{}: {e}", c.name));
        let expected = if c.name == "repository" {
            "builtin"
        } else {
            "declarative"
        };
        assert_eq!(doc["source"], expected, "{}", c.name);
    }
}

#[test]
fn a_tool_whose_handler_fails_internally_is_a_surface_error_not_a_refusal() {
    let f = Fixture::new();
    let ctx = context_with_demo_resources(&f);
    let surface = Surface::new(ctx);
    let err = surface.call("demo_broken", &json!({})).unwrap_err();
    assert!(
        matches!(&err, SurfaceError::Internal(m) if m == "a bug"),
        "{err}"
    );
}

#[test]
fn a_search_stops_at_its_limit() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    // more than one object carries "the"; a limit of one ends the walk after the first hit
    let all = app
        .context
        .execute("objects.search", json!({ "query": "the" }))
        .unwrap();
    assert!(all["count"].as_u64().unwrap() > 1, "{all}");
    let one = app
        .context
        .execute("objects.search", json!({ "query": "the", "limit": 1 }))
        .unwrap();
    assert_eq!(
        (one["count"].as_u64(), one["limit"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(
        one["hits"][0], all["hits"][0],
        "the first hit, in URI order"
    );
}
