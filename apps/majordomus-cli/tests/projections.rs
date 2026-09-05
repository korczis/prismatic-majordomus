//! Every projection is the registry, seen from somewhere: the same canonical id shows up
//! in MCP, HTTP, OpenAPI, introspection and the generated reference; nothing appears in a
//! projection that the registry does not hold; and a change to one canonical type
//! changes every projection without a projection being edited.

mod common;

use std::collections::BTreeSet;

use majordomus_cli::capability::handler::handler;
use majordomus_cli::capability::{
    BenchmarkPolicy, CachePolicy, CanonicalSchema, Capability, CapabilityId, CapabilityKind,
    CapabilityRegistry, Executable, Exposure, HttpExposure, HttpMethod, McpExposure, ModuleId,
    Provenance, Stability,
};
use majordomus_cli::generate::{artifacts, Target};
use majordomus_cli::http::openapi;
use majordomus_cli::mcp::Surface;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[test]
fn every_declared_projection_exists_and_no_projection_is_an_orphan() {
    let f = common::Fixture::new();
    let app = common::load_app(&f);
    let registry = app.registry();
    let surface = Surface::new(app.context.clone());
    let doc = openapi::document(registry, "test").unwrap();
    let reference: String = artifacts(registry, "test", &[Target::Docs])
        .unwrap()
        .into_iter()
        .map(|a| a.content)
        .collect();

    let tools = surface.tools();
    let resources = surface.resources();
    let mut ops: Vec<(String, String, Value)> = Vec::new();
    for (path, methods) in doc["paths"].as_object().unwrap() {
        for (method, op) in methods.as_object().unwrap() {
            ops.push((method.to_uppercase(), path.clone(), op.clone()));
        }
    }

    // declared -> present, with the same id
    for c in registry.iter() {
        if let Some(mcp) = &c.exposure.mcp {
            if let Some(name) = &mcp.tool {
                let t = tools
                    .iter()
                    .find(|t| &t.name == name)
                    .unwrap_or_else(|| panic!("{}: MCP tool {name} missing", c.id));
                assert_eq!(t.id, c.id.to_string());
                assert_eq!(t.input_schema, c.input.for_mcp());
                assert_eq!(t.description, c.description);
            }
            if let Some(res) = &mcp.resource {
                let r = resources
                    .iter()
                    .find(|r| r.uri == res.uri)
                    .unwrap_or_else(|| panic!("{}: MCP resource missing", c.id));
                assert_eq!(r.meta["id"], c.id.as_str());
            }
        }
        if let Some(http) = &c.exposure.http {
            let (_, _, op) = ops
                .iter()
                .find(|(m, p, _)| m == http.method.as_str() && p == &http.path)
                .unwrap_or_else(|| panic!("{}: OpenAPI operation missing", c.id));
            assert_eq!(op["operationId"], c.id.as_str());
            assert_eq!(op["x-majordomus-id"], c.id.as_str());
            assert_eq!(op["description"], c.description);
            assert_eq!(
                op["x-majordomus-kind"],
                serde_json::to_value(c.kind).unwrap()
            );
            assert!(registry.by_http(http.method, &http.path).is_some());
        }
        if let Some(cli) = &c.exposure.cli {
            assert_eq!(registry.by_cli(&cli.path).unwrap().id, c.id);
        }
        if matches!(c.provenance, Provenance::Builtin { .. }) {
            assert!(
                reference.contains(&format!("`{}`", c.id)),
                "{} missing from the generated reference",
                c.id
            );
        }
    }
    // present -> declared
    for t in tools.iter() {
        assert_eq!(registry.by_mcp_tool(&t.name).unwrap().id.to_string(), t.id);
    }
    for r in resources.iter() {
        assert!(
            registry.by_mcp_uri(&r.uri).is_some(),
            "orphan MCP resource {}",
            r.uri
        );
    }
    for (m, p, op) in &ops {
        let c = registry
            .by_http(HttpMethod::parse(m).unwrap(), p)
            .unwrap_or_else(|| panic!("orphan operation {m} {p}"));
        assert_eq!(op["operationId"], c.id.as_str());
    }
    let ids: BTreeSet<&str> = ops
        .iter()
        .map(|(_, _, op)| op["operationId"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), ops.len(), "operationIds are unique");
    assert!(
        common::openapi_refs_resolve(&doc).is_empty(),
        "{:?}",
        common::openapi_refs_resolve(&doc)
    );
    assert_eq!(doc["openapi"], "3.1.0");
    // the declarative object is a capability, an MCP resource, and absent from HTTP paths
    let alpha = registry.get("rule.project.alpha@1").unwrap();
    assert_eq!(alpha.kind, CapabilityKind::Resource);
    assert!(resources
        .iter()
        .any(|r| r.uri == "majordomus://rule/project.alpha@1"));
    assert!(alpha.exposure.http.is_none());
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct EchoV1 {
    /// The thing.
    foo: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct EchoV2 {
    /// The thing.
    foo: String,
    /// The other thing.
    bar: Option<u32>,
}
#[derive(Serialize, JsonSchema)]
struct Echoed {
    foo: String,
}

fn echo<I: serde::de::DeserializeOwned + 'static>(
    description: &str,
    schema: CanonicalSchema,
    http: bool,
) -> Executable {
    Executable {
        capability: Capability {
            id: CapabilityId::parse("fixture.echo").unwrap(),
            module: ModuleId::unchecked("fixture"),
            kind: CapabilityKind::Query,
            title: "Echo".into(),
            description: description.into(),
            input: schema,
            output: CanonicalSchema::of::<Echoed>(),
            provenance: Provenance::Builtin {
                module: "fixture".into(),
            },
            exposure: Exposure {
                mcp: Some(McpExposure {
                    tool: Some("fixture_echo".into()),
                    resource: None,
                }),
                http: http.then(|| HttpExposure {
                    method: HttpMethod::Get,
                    path: "/api/v1/echo".into(),
                }),
                cli: None,
            },
            stability: Stability::Experimental,
            tags: vec![],
            benchmark: BenchmarkPolicy::Required,
            cache: CachePolicy::Disabled,
        },
        handler: handler::<I, Echoed, _>(|_, _| Ok(Echoed { foo: "x".into() })),
        cases: |_| vec![],
    }
}

/// Projections of a one-capability registry: (MCP tool schema, OpenAPI operation, reference).
fn project(exec: Executable) -> (Value, Value, String) {
    let registry = CapabilityRegistry::builder()
        .with_builtin(vec![exec])
        .build()
        .unwrap();
    let doc = openapi::document(&registry, "test").unwrap();
    let op = doc["paths"]["/api/v1/echo"]["get"].clone();
    let reference: String = artifacts(&registry, "test", &[Target::Docs])
        .unwrap()
        .into_iter()
        .map(|a| a.content)
        .collect();
    let tool_schema = registry.get("fixture.echo").unwrap().input.for_mcp();
    (tool_schema, op, reference)
}

#[test]
fn a_change_to_the_canonical_type_or_description_reaches_every_projection() {
    let (mcp1, op1, ref1) = project(echo::<EchoV1>(
        "Echo, version one.",
        CanonicalSchema::of::<EchoV1>(),
        true,
    ));
    assert!(mcp1["properties"]["foo"].is_object() && mcp1["properties"].get("bar").is_none());
    let params1: Vec<&str> = op1["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(params1, ["foo"]);
    assert_eq!(op1["description"], "Echo, version one.");
    assert!(
        ref1.contains("Echo, version one.")
            && ref1.contains("| `foo` |")
            && !ref1.contains("| `bar` |")
    );

    // the only change: the input type and the description of the one descriptor
    let (mcp2, op2, ref2) = project(echo::<EchoV2>(
        "Echo, version two.",
        CanonicalSchema::of::<EchoV2>(),
        true,
    ));
    assert!(mcp2["properties"]["bar"].is_object());
    let params2: Vec<&str> = op2["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(params2, ["foo", "bar"]);
    let bar = op2["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "bar")
        .unwrap();
    assert_eq!(bar["required"], false);
    assert_eq!(bar["description"], "The other thing.");
    assert_eq!(op2["description"], "Echo, version two.");
    assert!(ref2.contains("Echo, version two.") && ref2.contains("| `bar` |"));

    // MCP and OpenAPI take the same constraints from the same schema
    assert_eq!(mcp2["required"], serde_json::json!(["foo"]));
    let openapi_foo = op2["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "foo")
        .unwrap();
    assert_eq!(openapi_foo["required"], true);
    assert_eq!(
        openapi_foo["schema"]["type"],
        mcp2["properties"]["foo"]["type"]
    );

    // exposure off: the route and the operation vanish, the tool stays
    let registry = CapabilityRegistry::builder()
        .with_builtin(vec![echo::<EchoV2>(
            "Echo.",
            CanonicalSchema::of::<EchoV2>(),
            false,
        )])
        .build()
        .unwrap();
    let doc = openapi::document(&registry, "test").unwrap();
    assert!(doc["paths"].as_object().unwrap().is_empty());
    assert!(registry.by_mcp_tool("fixture_echo").is_some());
    assert!(registry
        .get("fixture.echo")
        .unwrap()
        .exposure
        .http
        .is_none());
}

#[test]
fn generated_artifacts_are_byte_identical_twice_and_carry_no_absolute_path() {
    let f = common::Fixture::new();
    let a = artifacts(common::load_app(&f).registry(), "test", Target::ALL).unwrap();
    let b = artifacts(common::load_app(&f).registry(), "test", Target::ALL).unwrap();
    assert_eq!(a, b);
    for art in &a {
        assert!(
            !art.content.contains(f.root().to_str().unwrap()),
            "{} carries the checkout path",
            art.path
        );
    }
    let doc: Value = serde_json::from_str(&a[0].content).unwrap();
    assert!(doc["paths"].as_object().unwrap().keys().is_sorted());
    assert!(doc["components"]["schemas"]
        .as_object()
        .unwrap()
        .keys()
        .is_sorted());
    assert!(a[1]
        .content
        .starts_with("<!-- GENERATED FILE — DO NOT EDIT DIRECTLY"));
}

#[test]
fn every_generated_artifact_is_derived_deterministic_and_traces_to_the_registry() {
    use majordomus_cli::bench::{BenchmarkProjection, SystemTarget};
    use majordomus_cli::capability::registry::ModuleSource;
    use majordomus_cli::generate::{context_artifacts, REGISTRY_SCHEMA};
    let f = common::Fixture::new();
    let app = common::load_app(&f);
    let ctx = app.context.clone();
    let a = context_artifacts(&ctx, "test", Target::ALL).unwrap();
    let b = context_artifacts(&common::load_app(&f).context, "test", Target::ALL).unwrap();
    assert_eq!(a, b, "the same registry generates the same bytes");
    let by_path = |p: &str| -> String {
        a.iter()
            .find(|x| x.path == p)
            .unwrap_or_else(|| panic!("artifact {p}"))
            .content
            .clone()
    };
    // one file per builtin module, linked from the index, each carrying its capabilities
    let index = by_path("docs/generated/capabilities.md");
    assert!(index.starts_with("<!-- GENERATED FILE"));
    for m in ctx
        .registry
        .modules()
        .filter(|m| m.source == ModuleSource::Builtin)
    {
        let page = by_path(&format!("docs/generated/modules/{}.md", m.id));
        assert!(index.contains(&format!("modules/{}.md", m.id)));
        for c in ctx.registry.iter().filter(|c| c.module == m.id) {
            assert!(
                page.contains(&format!("## `{}` — {}", c.id, c.title)),
                "{} in {}",
                c.id,
                m.id
            );
            assert!(index.contains(&format!("| `{}` | `{}` |", c.id, m.id)));
        }
    }
    assert!(
        !a.iter().any(|x| x.path.contains("modules/rule.md")),
        "declarative kinds get no module page: they are the repository's"
    );
    // the benchmark matrix names every capability target and every system target
    let matrix = by_path("docs/generated/benchmarks.md");
    let projection = BenchmarkProjection::from_context(&ctx);
    for t in &projection.targets {
        match t.capability_id() {
            Some(id) => assert!(
                matrix.contains(&format!("| `{id}` |")),
                "{id} in the matrix"
            ),
            None => assert!(
                matrix.contains(&format!("| `{}` |", t.key)),
                "{} in the matrix",
                t.key
            ),
        }
    }
    for s in SystemTarget::ALL {
        assert!(matrix.contains(s.key()));
    }
    assert!(matrix.contains("| total | 36 | 36 | 0 | 0 |") || matrix.contains("| total |"));
    // the manifest is data: the builtin registry, no declarative object, no fingerprint
    let manifest: Value = serde_json::from_str(&by_path("docs/generated/registry.json")).unwrap();
    assert_eq!(manifest["schema"], REGISTRY_SCHEMA);
    let ids: Vec<&str> = manifest["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    for c in ctx.registry.iter() {
        let builtin = matches!(c.provenance, Provenance::Builtin { .. });
        assert_eq!(ids.contains(&c.id.as_str()), builtin, "{}", c.id);
    }
    assert!(manifest.get("fingerprint").is_none());
    assert!(manifest["declarative_kinds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k == "rule"));
    assert_eq!(
        manifest["modules"].as_array().unwrap().len(),
        ctx.registry
            .modules()
            .filter(|m| m.source == ModuleSource::Builtin)
            .count()
    );
    // written, then in sync; one byte changed, then stale by name
    let (code, out, err) = common::run_in(&f.root(), &["generate"], "");
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("docs/generated/benchmarks.md")
            && out.contains("docs/generated/modules/objects.md")
            && out.contains("docs/generated/registry.json"),
        "{out}"
    );
    let (code, _, _) = common::run_in(&f.root(), &["generate", "--check"], "");
    assert_eq!(code, 0);
    let path = f.path("docs/generated/benchmarks.md");
    std::fs::write(
        &path,
        format!("{}\n", std::fs::read_to_string(&path).unwrap()),
    )
    .unwrap();
    let (code, _, err) = common::run_in(&f.root(), &["generate", "--check"], "");
    assert_eq!(code, 10);
    assert!(
        err.contains("docs/generated/benchmarks.md (differs)"),
        "{err}"
    );
    let (code, out, _) = common::run_in(&f.root(), &["generate", "benchmarks"], "");
    assert_eq!(code, 0);
    assert_eq!(out.trim(), "docs/generated/benchmarks.md");
}
