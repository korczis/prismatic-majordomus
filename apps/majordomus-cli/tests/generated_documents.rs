//! Every generated artifact is typed: it declares the document it projects, the encoding
//! it is written in, the contract its content satisfies and the source it came from; it
//! carries a provenance header in the form its encoding allows; a structured document is
//! committed in every encoding this repository commits it in, from one value; and the
//! manifest is the plan, generated.
//!
//! These are the properties of the enforcement rule
//! `project.generated-artifacts-are-typed@1`. They are checked here over a real plan of a
//! real repository, and the negative half — that each property, broken, is *refused* — is
//! checked by mutating a plan and asking the verifier, so that none of these assertions
//! can pass vacuously.

mod common;

use majordomus_cli::generate::{
    self, opens_with_banner, Artifact, ArtifactFormat, Document, GeneratedSchemas, HeaderStyle,
    Target, HEADER, MANIFEST_ID, MANIFEST_SCHEMA,
};
use serde_json::{json, Value};

fn schemas() -> GeneratedSchemas {
    GeneratedSchemas::load(&common::dist_share().join("schemas/generated"))
        .expect("the contracts of the generated documents load")
}

fn plan(f: &common::Fixture) -> Vec<Artifact> {
    // every target but the allow-lists: the fixture's share is the distribution's, beside
    // this crate, so an allow-list path resolves outside the fixture root
    let targets: Vec<Target> = Target::ALL
        .iter()
        .copied()
        .filter(|t| *t != Target::Allow)
        .collect();
    generate::plan(&common::load_app(f), &targets).expect("a plan")
}

// ---------------------------------------------------------------- the properties

#[test]
fn every_artifact_declares_its_encoding_its_source_and_carries_a_header() {
    let f = common::Fixture::new();
    let artifacts = plan(&f);
    assert!(
        artifacts.len() > 10,
        "a plan of {} artifacts",
        artifacts.len()
    );
    for a in &artifacts {
        assert_eq!(
            a.format,
            ArtifactFormat::of_path(&a.path),
            "{} declares an encoding its suffix contradicts",
            a.path
        );
        assert!(!a.source.trim().is_empty(), "{} names no source", a.path);
        assert!(!a.document.trim().is_empty(), "{} projects nothing", a.path);
        // the provider bootstraps stamp themselves with the policy hash instead
        if a.document.starts_with("providers/") {
            continue;
        }
        // `opens_with_banner`, not `starts_with`: an executable script opens with its `#!`
        // line and a projected page with its own title, and both carry the banner on the
        // line after. The rule is stated once, in the generator, and read here.
        match a.format {
            ArtifactFormat::Markdown => assert!(
                opens_with_banner(&a.content, &format!("<!-- {HEADER}")),
                "{} carries no banner",
                a.path
            ),
            ArtifactFormat::Yaml | ArtifactFormat::Text => assert!(
                opens_with_banner(&a.content, &format!("# {HEADER}")),
                "{} carries no banner",
                a.path
            ),
            ArtifactFormat::Json => {
                let v: Value = serde_json::from_str(&a.content)
                    .unwrap_or_else(|e| panic!("{} is not JSON: {e}", a.path));
                let banner = v
                    .get("generated")
                    .or_else(|| v.get("x-majordomus-generated"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                assert!(
                    banner.starts_with(HEADER),
                    "{} says nothing about being generated",
                    a.path
                );
                assert!(
                    banner.contains("majordomus generate"),
                    "{} does not name the command that rewrites it",
                    a.path
                );
            }
        }
    }
    // and the whole plan is accepted by the verifier the command runs
    assert_eq!(generate::violations(&artifacts, &schemas()), Vec::new());
}

#[test]
fn a_document_with_a_json_encoding_has_a_yaml_one_and_they_are_the_same_document() {
    let f = common::Fixture::new();
    let artifacts = plan(&f);
    let json: Vec<&Artifact> = artifacts
        .iter()
        .filter(|a| a.format == ArtifactFormat::Json && !a.document.starts_with("providers/"))
        // Documents whose only reader is a program are committed as JSON alone: the three
        // the website loads, the matrix the release workflow reads, and the public metadata
        // of each release. A YAML twin of any of them would be a file nobody opens.
        .filter(|a| {
            !matches!(
                a.document.as_str(),
                "site-registry"
                    | "site-why"
                    | "site-why-graph"
                    | "site-distribution"
                    | "distribution-matrix"
            ) && !a.document.starts_with("release/")
        })
        // a projected JSON Schema is JSON by its own contract: `.schema.json` is what a
        // validator looks for, and a YAML sibling would be a second encoding of a file
        // whose format is named in its extension and read by nothing that wants YAML
        .filter(|a| !a.document.starts_with("schemas/"))
        .collect();
    assert!(json.len() >= 4, "{} JSON documents", json.len());
    for a in json {
        let yaml = artifacts
            .iter()
            .find(|b| b.document == a.document && b.format == ArtifactFormat::Yaml)
            .unwrap_or_else(|| panic!("{} has no YAML encoding", a.document));
        assert_eq!(
            yaml.path,
            a.path.replace(".json", ".yaml"),
            "the encodings of {} are not siblings",
            a.document
        );
        assert_eq!(
            yaml.schema, a.schema,
            "{} declares two contracts",
            a.document
        );
        assert_eq!(yaml.source, a.source, "{} names two sources", a.document);
        // the same value: what the YAML says, keyed, appears in the JSON
        let value: Value = serde_json::from_str(&a.content).unwrap();
        for key in value.as_object().unwrap().keys() {
            assert!(
                yaml.content.contains(&format!("\n{key}:"))
                    || yaml.content.contains(&format!("\n\"{key}\":")),
                "{} is in {} and not in its YAML encoding",
                key,
                a.path
            );
        }
    }
}

#[test]
fn every_document_that_names_a_contract_satisfies_the_published_one() {
    let f = common::Fixture::new();
    let schemas = schemas();
    let published: Vec<String> = schemas.ids().map(str::to_string).collect();
    assert!(
        published.contains(&MANIFEST_SCHEMA.to_string())
            && published.contains(&generate::REGISTRY_SCHEMA.to_string())
            && published.contains(&generate::BENCHMARKS_SCHEMA.to_string()),
        "{published:?}"
    );
    let mut checked = 0;
    for a in plan(&f) {
        let (Some(schema), ArtifactFormat::Json) = (&a.schema, a.format) else {
            continue;
        };
        if !published.contains(schema) {
            continue;
        }
        let value: Value = serde_json::from_str(&a.content).unwrap();
        assert_eq!(
            schemas.violations(schema, &value),
            Vec::<String>::new(),
            "{} does not satisfy {schema}",
            a.path
        );
        checked += 1;
    }
    assert!(checked >= 4, "only {checked} documents carry a contract");
}

#[test]
fn the_manifest_is_the_plan_and_names_itself_without_hashing_itself() {
    let f = common::Fixture::new();
    let artifacts = plan(&f);
    let manifest = artifacts
        .iter()
        .find(|a| a.document == MANIFEST_ID && a.format == ArtifactFormat::Json)
        .expect("the manifest");
    let doc: Value = serde_json::from_str(&manifest.content).unwrap();
    let listed = doc["artifacts"].as_array().unwrap();
    assert_eq!(listed.len(), artifacts.len());
    for a in &artifacts {
        let entry = listed
            .iter()
            .find(|e| e["path"] == a.path)
            .unwrap_or_else(|| panic!("{} is planned and not listed", a.path));
        assert_eq!(entry["document"], a.document);
        assert_eq!(
            entry["format"].as_str().unwrap(),
            a.format
                .suffix()
                .replace("md", "markdown")
                .replace("txt", "text")
        );
        if a.document == MANIFEST_ID {
            assert_eq!(entry["describes_itself"], true, "{}", a.path);
            assert!(entry.get("sha256").is_none(), "{} hashes itself", a.path);
        } else {
            assert_eq!(entry["bytes"].as_u64().unwrap() as usize, a.content.len());
            assert!(entry["sha256"].as_str().unwrap().len() == 64);
        }
    }
    // the documents view is a fold of the same list, and the manifest's own encodings are in it
    let documents = doc["documents"].as_array().unwrap();
    let mine = documents
        .iter()
        .find(|d| d["id"] == MANIFEST_ID)
        .expect("the manifest describes itself as a document");
    let formats: Vec<&str> = mine["formats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(formats, ["json", "markdown", "yaml"]);
    for d in documents {
        let id = d["id"].as_str().unwrap();
        assert!(
            artifacts.iter().any(|a| a.document == id),
            "{id} is a document of nothing"
        );
    }
}

// ---------------------------------------------------------------- the refusals

/// Each property, broken, is refused. Without this the assertions above could all hold of
/// a verifier that returns nothing.
#[test]
fn the_verifier_refuses_a_missing_header_a_wrong_encoding_a_broken_contract_and_a_stale_manifest() {
    let f = common::Fixture::new();
    let schemas = schemas();
    let good = plan(&f);
    assert_eq!(generate::violations(&good, &schemas), Vec::new());

    let broken = |mutate: &dyn Fn(&mut Vec<Artifact>)| -> Vec<String> {
        let mut a = good.clone();
        mutate(&mut a);
        generate::violations(&a, &schemas)
            .into_iter()
            .map(|v| v.to_string())
            .collect()
    };

    // a banner stripped from a Markdown artifact
    let v = broken(&|a| {
        let x = a
            .iter_mut()
            .find(|x| x.path == "docs/generated/capabilities.md")
            .unwrap();
        x.content = "# Capability reference\n".into();
    });
    assert!(
        v.iter()
            .any(|r| r.contains("capabilities.md") && r.contains("no generated-file banner")),
        "{v:?}"
    );

    // a banner stripped from a YAML artifact
    let v = broken(&|a| {
        let x = a
            .iter_mut()
            .find(|x| x.path == "docs/generated/registry.yaml")
            .unwrap();
        x.content = "modules: []\n".into();
    });
    assert!(v.iter().any(|r| r.contains("registry.yaml")), "{v:?}");

    // an encoding its suffix contradicts
    let v = broken(&|a| {
        let x = a
            .iter_mut()
            .find(|x| x.path == "docs/generated/registry.json")
            .unwrap();
        x.format = ArtifactFormat::Yaml;
    });
    assert!(
        v.iter().any(|r| r.contains("declares format yaml")),
        "{v:?}"
    );

    // a document that no longer satisfies the contract it names
    let v = broken(&|a| {
        let x = a
            .iter_mut()
            .find(|x| x.path == "docs/generated/benchmarks.json")
            .unwrap();
        let mut value: Value = serde_json::from_str(&x.content).unwrap();
        value.as_object_mut().unwrap().remove("coverage");
        x.content = serde_json::to_string_pretty(&value).unwrap() + "\n";
    });
    assert!(
        v.iter()
            .any(|r| r.contains("benchmarks.json") && r.contains("majordomus/benchmark-matrix/v1")),
        "{v:?}"
    );

    // a manifest that no longer describes the plan
    let v = broken(&|a| {
        let x = a
            .iter_mut()
            .find(|x| x.path == "docs/generated/capabilities.md")
            .unwrap();
        x.content.push('\n');
    });
    assert!(
        v.iter()
            .any(|r| r.contains("stale hash for docs/generated/capabilities.md")),
        "{v:?}"
    );

    // an artifact the manifest never heard of
    let v = broken(&|a| {
        a.push(Artifact::markdown(
            "docs/generated/invented.md",
            "invented",
            "nowhere",
            "test",
            "# Invented\n",
        ));
    });
    assert!(
        v.iter()
            .any(|r| r.contains("does not list docs/generated/invented.md")),
        "{v:?}"
    );
}

/// A schema file that pins no document validates nothing, so loading refuses it rather
/// than accepting a contract that can never fail.
#[test]
fn a_contract_that_pins_no_document_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("nothing.schema.json"),
        r#"{"type":"object"}"#,
    )
    .unwrap();
    let err = GeneratedSchemas::load(dir.path()).unwrap_err().to_string();
    assert!(err.contains("pins no document"), "{err}");
}

// ---------------------------------------------------------------- the encodings

/// One value, two encodings, and the YAML reads back through the layer's own parser when
/// the value is inside the layer's subset. This is the guarantee that keeps the `.yaml`
/// beside every `.json` honest rather than decorative.
#[test]
fn a_document_renders_to_json_and_to_yaml_from_one_value() {
    let doc = Document::new(
        "fixture",
        "majordomus/fixture/v1",
        "a test",
        json!({ "items": [{ "id": "a", "n": 1 }], "flag": true }),
    );
    let arts = doc.artifacts("test");
    assert_eq!(arts.len(), 2);
    assert_eq!(arts[0].path, "docs/generated/fixture.json");
    assert_eq!(arts[1].path, "docs/generated/fixture.yaml");
    assert_eq!(arts[0].schema.as_deref(), Some("majordomus/fixture/v1"));

    let from_json: Value = serde_json::from_str(&arts[0].content).unwrap();
    let from_yaml = Value::Object(
        majordomus_cli::metadata::yaml::parse_mapping(&arts[1].content)
            .expect("the layer's parser reads what the layer's writer wrote"),
    );
    assert_eq!(from_json, from_yaml);
    assert_eq!(from_json["schema"], "majordomus/fixture/v1");
    assert_eq!(from_json["items"][0]["id"], "a");

    // the OpenAPI document cannot carry members its own specification does not define, so
    // its provenance is an extension and its `schema` stays the OpenAPI version
    let openapi = Document {
        id: "fixture".into(),
        dir: "docs/generated".into(),
        schema: None,
        source: "a test".into(),
        style: HeaderStyle::Extension,
        value: json!({ "openapi": "3.1.0" }),
    };
    let v: Value = serde_json::from_str(&openapi.artifacts("test")[0].content).unwrap();
    assert_eq!(v["openapi"], "3.1.0");
    assert!(v.get("generated").is_none());
    assert!(v["x-majordomus-generated"]
        .as_str()
        .unwrap()
        .starts_with(HEADER));
}

/// `generate --check` refuses a hand-edited encoding, and `generate <document>` writes
/// every encoding of it back. The one command a person runs, end to end.
#[test]
fn a_hand_edited_encoding_is_refused_and_regenerating_the_document_restores_every_encoding() {
    let f = common::Fixture::new();
    let (code, out, err) = common::run_in(&f.root(), &["generate"], "");
    assert_eq!(code, 0, "{err}");
    for expected in [
        "docs/generated/registry.json",
        "docs/generated/registry.yaml",
        "docs/generated/artifacts.json",
        "docs/generated/artifacts.yaml",
        "docs/generated/artifacts.md",
    ] {
        assert!(out.contains(expected), "{expected} was not written: {out}");
    }
    let (code, out, _) = common::run_in(&f.root(), &["generate", "--check"], "");
    assert_eq!(code, 0);
    assert!(out.contains("majordomus/capability-registry/v1"), "{out}");

    let path = f.path("docs/generated/registry.yaml");
    std::fs::write(&path, "# hand edited\n").unwrap();
    let (code, _, err) = common::run_in(&f.root(), &["generate", "--check"], "");
    assert_eq!(code, 10);
    assert!(
        err.contains("docs/generated/registry.yaml (differs)"),
        "{err}"
    );

    let (code, out, err) = common::run_in(&f.root(), &["generate", "registry"], "");
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        [
            "docs/generated/registry.json",
            "docs/generated/registry.yaml"
        ]
    );
    let (code, _, err) = common::run_in(&f.root(), &["generate", "--check"], "");
    assert_eq!(code, 0, "{err}");

    // and the manifest alone is a manifest of everything, not of itself
    let (code, out, err) = common::run_in(&f.root(), &["generate", "manifest"], "");
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        [
            "docs/generated/artifacts.json",
            "docs/generated/artifacts.yaml",
            "docs/generated/artifacts.md"
        ]
    );
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(f.path("docs/generated/artifacts.json")).unwrap(),
    )
    .unwrap();
    assert!(doc["artifacts"].as_array().unwrap().len() > 10);
}
