//! The product model: the domain over the fixture's own feature, the contract its schema
//! enforces, the public projection the site reads, and the invariant the whole arrangement
//! exists for — one file added is answered everywhere, and one file removed is answered
//! nowhere.
//!
//! What is deliberately not here: any list of features. Every assertion below either
//! counts what the fixture declares or names the one record the test itself wrote.
//!
//! Proves claim product-surfaces-derived (docs/CLAIMS.yaml), whose `test:` names this case.
//! Proves claim product-references-resolve (docs/CLAIMS.yaml), whose `test:` names this case.
//! Proves claim product-projection-public-safe (docs/CLAIMS.yaml), whose `test:` names this case.

mod common;

use common::Fixture;
use majordomus_cli::model::Severity;
use majordomus_cli::product::{ProductModel, SURFACES};
use majordomus_cli::site::PUBLIC_FEATURE_FIELDS;
use serde_json::{json, Value};

fn model(f: &Fixture) -> (majordomus_cli::app::App, ProductModel) {
    let app = common::load_app(f);
    let ctx = &app.context;
    let m = ProductModel::build(&ctx.index, &ctx.registry, &ctx.why, &ctx.web);
    (app, m)
}

/// A second feature, valid, naming the same fixture objects the first one names.
fn second_feature(id: &str) -> String {
    common::FEATURE
        .replace("id: fixture-feature", &format!("id: {id}"))
        .replace("featured: true", "featured: false")
        .replace("weight: 10", "weight: 20")
        .replace(
            "short_title: Fixture feature",
            "short_title: Second feature",
        )
}

// ---------------------------------------------------------------- the records

#[test]
fn a_valid_model_reads_the_feature_and_derives_its_surfaces_from_what_it_names() {
    let f = Fixture::new();
    let (app, m) = model(&f);
    assert_eq!(m.all().len(), 1, "the fixture declares one feature");
    assert_eq!(m.errors(), 0, "findings: {:?}", m.findings());
    let r = m.feature("fixture-feature").expect("the feature is read");
    assert_eq!(r.feature.route, "/features/fixture-feature/");
    assert_eq!(r.feature.source, ".ai/repo/features/fixture-feature.md");
    assert!(r.feature.body.contains("## What it does"));
    assert!(r.feature.featured);

    // the module resolved to its capabilities, and the surfaces follow from their exposures
    let module = r
        .module_refs
        .iter()
        .find(|x| x.id == "repository")
        .expect("the repository module resolves");
    let expected: Vec<String> = app
        .context
        .registry
        .iter()
        .filter(|c| c.module.as_str() == "repository")
        .filter(|c| {
            matches!(
                c.provenance,
                majordomus_cli::capability::Provenance::Builtin { .. }
            )
        })
        .map(|c| c.id.to_string())
        .collect();
    let got: Vec<String> = module.capabilities.iter().map(|c| c.id.clone()).collect();
    assert_eq!(
        got, expected,
        "every capability of the module, from the registry"
    );
    let has_http = module.capabilities.iter().any(|c| c.route.is_some());
    let has_cli = module.capabilities.iter().any(|c| c.cli.is_some());
    assert_eq!(r.surfaces.api, has_http, "api follows the HTTP exposures");
    assert_eq!(
        r.surfaces.cli, has_cli,
        "cli follows the command-line exposures"
    );
    assert!(
        r.surfaces.mcp,
        "a kind named means every object is an MCP resource"
    );
    assert!(
        r.surfaces.cockpit,
        "a module named means every capability has a page"
    );
    assert!(r.surfaces.docs, "a document named");
    assert_eq!(r.counts.capabilities, expected.len());
    assert_eq!(
        r.counts.http_routes,
        module
            .capabilities
            .iter()
            .filter(|c| c.route.is_some())
            .count()
    );

    // the kind resolved to its count, the rule to its identity, the claim to its status
    assert_eq!(r.kind_refs.len(), 1);
    assert_eq!(r.kind_refs[0].name, "rule");
    assert_eq!(r.kind_refs[0].objects, app.context.index.kinds()["rule"]);
    assert_eq!(r.counts.objects, r.kind_refs[0].objects);
    assert_eq!(r.rule_refs[0].identity, "project.alpha@1");
    assert_eq!(r.rule_refs[0].class, "advisory");
    assert!(!r.rule_refs[0].enforced);
    assert_eq!(r.claim_refs[0].status, "guaranteed");
    assert_eq!(r.evidence.claims["guaranteed"], 1);
    assert_eq!(r.doc_refs[0].path, "docs/CLI.md");
    assert_eq!(r.cockpit_refs[0].route, "/cockpit");
    assert_eq!(r.web_refs[0].mount, "/swagger");

    // the moment that names the feature's claim is the moment the feature answers: derived
    // through the catalogue's own reverse index, never written in either file
    assert_eq!(
        r.moments.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["fixture-moment"]
    );
    assert_eq!(r.counts.moments, 1);
}

#[test]
fn the_providers_are_the_templates_the_distribution_ships_decorated_by_the_policy() {
    let f = Fixture::new();
    let (_app, m) = model(&f);
    let ids: Vec<&str> = m.providers().iter().map(|p| p.id.as_str()).collect();
    // the set is the share directory's, read at run time; nothing here is a vendor list
    let mut expected: Vec<String> = std::fs::read_dir(common::dist_share().join("providers"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter_map(|n| n.strip_suffix(".tmpl").map(str::to_string))
        .collect();
    expected.sort();
    assert_eq!(ids, expected);
    let agents = m.providers().iter().find(|p| p.id == "agents").unwrap();
    assert_eq!(
        agents.bootstraps.len(),
        1,
        "the fixture's policy projects AGENTS.md"
    );
    assert_eq!(agents.bootstraps[0].target, "AGENTS.md");
    assert!(agents.bootstraps[0].always_loaded);
    let claude = m
        .providers()
        .iter()
        .find(|p| p.id == "claude-code")
        .unwrap();
    assert!(
        claude.client_config.is_none(),
        "the fixture carries no .mcp.json"
    );
    assert!(
        claude.hooks.is_empty(),
        "the fixture's policy wires no hook"
    );
    assert_eq!(claude.title, "Claude Code");
}

#[test]
fn a_client_configuration_at_the_root_is_a_fact_of_the_tree() {
    let f = Fixture::new();
    f.write(".mcp.json", "{\"mcpServers\":{}}\n");
    f.commit("client config");
    let (_app, m) = model(&f);
    let claude = m
        .providers()
        .iter()
        .find(|p| p.id == "claude-code")
        .unwrap();
    assert_eq!(claude.client_config.as_deref(), Some(".mcp.json"));
}

#[test]
fn the_fingerprint_is_stable_for_a_tree_and_moves_with_it() {
    let f = Fixture::new();
    let (_a, first) = model(&f);
    let (_b, again) = model(&f);
    assert_eq!(first.fingerprint(), again.fingerprint());
    f.write(".ai/repo/features/second.md", &second_feature("second"));
    f.commit("another feature");
    let (_c, moved) = model(&f);
    assert_ne!(first.fingerprint(), moved.fingerprint());
}

#[test]
fn coverage_names_what_no_feature_presents() {
    let f = Fixture::new();
    let (_app, m) = model(&f);
    let repository = m
        .module_coverage()
        .iter()
        .find(|c| c.id == "repository")
        .unwrap();
    assert_eq!(repository.features, ["fixture-feature"]);
    let uncovered: Vec<&str> = m
        .module_coverage()
        .iter()
        .filter(|c| c.features.is_empty())
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        !uncovered.is_empty(),
        "the fixture names one module of many"
    );
    assert!(
        m.findings()
            .iter()
            .any(|x| x.code == "uncovered" && x.severity == Severity::Warning),
        "a gap is a warning, never silence"
    );
    assert_eq!(m.errors(), 0, "and never an error");
}

// ---------------------------------------------------------------- what the schema refuses

fn refused(body: &str, code: &str, fragment: &str) {
    let f = Fixture::new();
    f.write(".ai/repo/features/broken.md", body);
    f.commit("broken");
    let (code_seen, v, _err) = common::inspect(&f.root(), &[]);
    assert_eq!(code_seen, 10, "a refused record degrades the index");
    let diagnostics = v["repository"]["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let hit = diagnostics.iter().any(|d| {
        d["code"] == code
            && d["message"].as_str().is_some_and(|m| m.contains(fragment))
            && d["path"] == ".ai/repo/features/broken.md"
    });
    assert!(
        hit,
        "expected a {code} diagnostic naming {fragment:?}; got {diagnostics:#?}"
    );
}

#[test]
fn a_derived_key_written_into_a_source_is_refused() {
    refused(
        &second_feature("broken").replace("weight: 20", "weight: 20\nsurfaces: [cli, api]"),
        "unknown_key",
        "surfaces",
    );
}

#[test]
fn a_missing_required_field_is_refused() {
    refused(
        &second_feature("broken").replace(
            "headline: 'A feature that exists so every projection has something to project.'\n",
            "",
        ),
        "schema_violation",
        "headline",
    );
}

#[test]
fn an_unknown_schema_version_is_refused_rather_than_guessed() {
    refused(
        &second_feature("broken").replace("schema: feature/v1", "schema: feature/v2"),
        "unsupported_version",
        "feature/v2",
    );
}

// ---------------------------------------------------------------- what the model refuses

#[test]
fn an_unresolved_reference_is_an_error_with_the_nearest_candidate() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/features/typo.md",
        &second_feature("typo").replace("modules: [repository]", "modules: [repositry]"),
    );
    f.commit("typo");
    let (_app, m) = model(&f);
    let found = m
        .findings()
        .iter()
        .find(|x| x.code == "unknown_reference" && x.id.as_deref() == Some("typo"))
        .expect("the typo is a finding");
    assert_eq!(found.severity, Severity::Error);
    assert_eq!(found.field.as_deref(), Some("modules"));
    assert_eq!(found.did_you_mean.as_deref(), Some("repository"));
    assert_eq!(found.path, ".ai/repo/features/typo.md");
    assert!(m.errors() > 0);
}

#[test]
fn a_draft_may_not_be_featured_and_a_reserved_id_is_refused() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/features/matrix.md",
        &second_feature("matrix")
            .replace("status: stable", "status: draft")
            .replace("featured: false", "featured: true"),
    );
    f.commit("bad");
    let (_app, m) = model(&f);
    let codes: Vec<&str> = m
        .findings()
        .iter()
        .filter(|x| x.id.as_deref() == Some("matrix"))
        .map(|x| x.code.as_str())
        .collect();
    assert!(codes.contains(&"featured_draft"), "{codes:?}");
    assert!(codes.contains(&"reserved_identity"), "{codes:?}");
}

#[test]
fn a_stable_feature_under_its_floors_is_a_warning_not_an_error() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/features/thin.md",
        &second_feature("thin")
            .replace("docs: [docs/CLI.md]\n", "")
            .replace(
                "## What it does not do\n\nNothing the fixture does not say.\n",
                "",
            ),
    );
    f.commit("thin");
    let (_app, m) = model(&f);
    let floors: Vec<&str> = m
        .findings()
        .iter()
        .filter(|x| x.id.as_deref() == Some("thin") && x.code == "missing_content")
        .filter_map(|x| x.field.as_deref())
        .collect();
    assert!(floors.contains(&"docs"), "{floors:?}");
    assert!(floors.contains(&"body"), "{floors:?}");
    assert_eq!(m.errors(), 0);
}

// ---------------------------------------------------------------- the projections

#[test]
fn the_capabilities_answer_the_same_model_over_every_transport() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = &app.context;
    let list = ctx
        .execute("product.features", json!({}))
        .expect("the listing");
    assert_eq!(list["counts"]["features"], 1);
    assert_eq!(list["features"][0]["id"], "fixture-feature");
    assert_eq!(list["surfaces"].as_array().unwrap().len(), SURFACES.len());
    let one = ctx
        .execute("product.feature", json!({ "id": "fixture-feature" }))
        .expect("the feature");
    assert_eq!(one["route"], "/features/fixture-feature/");
    assert_eq!(one["moments"][0]["id"], "fixture-moment");
    let matrix = ctx
        .execute("product.matrix", json!({}))
        .expect("the matrix");
    assert_eq!(matrix["rows"][0]["id"], "fixture-feature");
    assert!(matrix["modules"].as_array().unwrap().len() > 1);
    let valid = ctx
        .execute("product.validate", json!({}))
        .expect("validation");
    assert_eq!(valid["valid"], true);
    // an unknown surface filter is an invalid input naming the vocabulary, not an empty answer
    let err = ctx
        .execute("product.features", json!({ "surface": "fax" }))
        .unwrap_err()
        .to_string();
    assert!(err.contains("cli") && err.contains("docs"), "{err}");

    // MCP: the tools and the resources the declaration projects
    let (_code, v, _err) = common::inspect(&f.root(), &[]);
    let tools: Vec<&str> = v["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for t in [
        "majordomus_features",
        "majordomus_feature",
        "majordomus_product_matrix",
        "majordomus_providers",
        "majordomus_product_validate",
    ] {
        assert!(tools.contains(&t), "tool {t} missing from {tools:?}");
    }
    let uris = common::resource_uris(&v);
    assert!(uris.iter().any(|u| u == "majordomus://product"));
    assert!(uris
        .iter()
        .any(|u| u == "majordomus://feature/fixture-feature"));

    // HTTP: the same document over a real socket
    let served = common::Served::start(&f.root(), &[]);
    let (status, _headers, body) = served.request("GET", "/api/v1/product/features", None);
    assert_eq!(status, 200);
    let over_http: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(over_http["features"][0]["id"], "fixture-feature");
    assert_eq!(over_http["fingerprint"], list["fingerprint"]);
    let (status, _h, body) = served.request("GET", "/api/v1/product/feature?id=nowhere", None);
    assert_eq!(status, 404, "{body}");
}

#[test]
fn the_site_dataset_is_an_allow_listed_public_projection_of_the_same_model() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let artifacts = majordomus_cli::site::product_artifacts(&app.context).expect("the dataset");
    let product = artifacts
        .iter()
        .find(|a| a.path == "site/data/registry/product.json")
        .expect("product.json is planned");
    assert_eq!(
        product.schema.as_deref(),
        Some(majordomus_cli::site::PRODUCT_SCHEMA)
    );
    let v: Value = serde_json::from_str(&product.content).unwrap();
    assert_eq!(v["schema"], majordomus_cli::site::PRODUCT_SCHEMA);
    assert_eq!(v["valid"], true);
    assert_eq!(v["features"][0]["id"], "fixture-feature");
    // every key of a feature was named by the allow-list, and the body stayed in its file
    for feature in v["features"].as_array().unwrap() {
        for key in feature.as_object().unwrap().keys() {
            assert!(
                PUBLIC_FEATURE_FIELDS.contains(&key.as_str()),
                "{key} reached the public dataset without being named"
            );
        }
        assert!(feature.get("body").is_none());
    }
    // nothing names the machine: the fixture's root is an absolute path and must not appear
    let root = f.root().display().to_string();
    assert!(
        !product.content.contains(&root),
        "the dataset names the repository root"
    );
    assert!(!product.content.contains("/tmp/") && !product.content.contains("/private/"));
    // the telemetry is counted from the registry and the index, never typed
    assert_eq!(
        v["telemetry"]["capabilities"],
        app.context.registry.summary().total
    );
    assert_eq!(v["telemetry"]["objects"], app.context.index.objects.len());
    assert!(v["telemetry"]["mcp_tools"].as_u64().unwrap() > 0);
    // the contract published for the document is satisfied
    let schemas = majordomus_cli::generate::GeneratedSchemas::load(
        &common::dist_share().join("schemas/generated"),
    )
    .unwrap();
    let violations = schemas.violations(majordomus_cli::site::PRODUCT_SCHEMA, &v);
    assert!(violations.is_empty(), "{violations:?}");
    // and the graph beside it holds the feature
    let graph = artifacts
        .iter()
        .find(|a| a.path == "site/data/registry/product-graph.json")
        .unwrap();
    let g: Value = serde_json::from_str(&graph.content).unwrap();
    assert!(g["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["id"] == "feature:fixture-feature"));
    assert!(g["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["source"] == "feature:fixture-feature" && e["target"] == "module:repository"));
    // deterministic: the same tree writes the same bytes
    let again = majordomus_cli::site::product_artifacts(&app.context).unwrap();
    assert_eq!(again[0].content, product.content);
}

// ---------------------------------------------------------------- the invariant

#[test]
fn one_file_added_is_answered_everywhere_and_one_removed_is_answered_nowhere() {
    let f = Fixture::new();
    f.write(".ai/repo/features/probe.md", &second_feature("probe"));
    f.commit("one file");
    let app = common::load_app(&f);
    let ctx = &app.context;
    let list = ctx.execute("product.features", json!({})).unwrap();
    let ids: Vec<&str> = list["features"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x["id"].as_str())
        .collect();
    assert!(ids.contains(&"probe"), "{ids:?}");
    let matrix = ctx.execute("product.matrix", json!({})).unwrap();
    assert!(matrix["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == "probe"));
    let repository = matrix["modules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "repository")
        .unwrap();
    assert!(repository["features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x == "probe"));
    let dataset = majordomus_cli::site::product_artifacts(ctx).unwrap();
    assert!(dataset[0].content.contains("\"/features/probe/\""));
    assert!(dataset[1].content.contains("feature:probe"));
    let (_code, v, _err) = common::inspect(&f.root(), &[]);
    assert!(common::resource_uris(&v)
        .iter()
        .any(|u| u == "majordomus://feature/probe"));

    f.remove(".ai/repo/features/probe.md");
    f.commit("one file removed");
    let app = common::load_app(&f);
    let ctx = &app.context;
    let list = ctx.execute("product.features", json!({})).unwrap();
    assert!(!list["features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == "probe"));
    let dataset = majordomus_cli::site::product_artifacts(ctx).unwrap();
    assert!(!dataset[0].content.contains("probe"));
    let err = ctx
        .execute("product.feature", json!({ "id": "probe" }))
        .unwrap_err()
        .to_string();
    assert!(err.contains("no feature 'probe'"), "{err}");
}
