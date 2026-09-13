//! The Why catalogue: the domain over the fixture's own catalogue, the contract its schemas
//! enforce, and the invariant the whole arrangement exists for — one file added is answered
//! everywhere, and one file removed is answered nowhere.
//!
//! What is deliberately not here: any list of moments. Every assertion below either counts
//! what the fixture declares or names the one record the test itself wrote.
//!
//! Proves claim why-references-resolve (docs/CLAIMS.yaml), whose `test:` names this case.
//! Proves claim why-diagnosis-explainable (docs/CLAIMS.yaml), whose `test:` names this case.

mod common;

use common::Fixture;
use majordomus_cli::model::Severity;
use majordomus_cli::why::{Catalogue, Query};
use serde_json::json;

fn catalogue(f: &Fixture) -> (majordomus_cli::app::App, Catalogue) {
    let app = common::load_app(f);
    let c = Catalogue::build(&app.context.index, &app.context.registry);
    (app, c)
}

// ---------------------------------------------------------------- the records

#[test]
fn a_valid_catalogue_is_read_into_typed_records_and_validates() {
    let f = Fixture::new();
    let (_app, c) = catalogue(&f);
    assert_eq!(c.all().len(), 1, "the fixture declares one moment");
    assert_eq!(c.audiences().len(), 1);
    assert_eq!(c.areas().len(), 1);
    let m = c.moment("fixture-moment").expect("the moment is read");
    assert_eq!(m.title, "The moment the fixture recognises");
    assert_eq!(m.severity, "medium");
    assert_eq!(m.examples.len(), 3);
    assert_eq!(m.signals.len(), 1);
    // the two facts the file cannot hold, derived
    assert_eq!(m.route, "/why/fixture-moment/");
    assert_eq!(m.source, ".ai/repo/why/moments/fixture-moment.md");
    assert!(m.body.contains("## The moment"));
    assert_eq!(c.errors(), 0, "findings: {:?}", c.findings());
}

#[test]
fn the_fingerprint_is_stable_for_a_tree_and_moves_with_it() {
    let f = Fixture::new();
    let (_app, first) = catalogue(&f);
    let (_app2, again) = catalogue(&f);
    assert_eq!(first.fingerprint(), again.fingerprint());
    assert!(!first.fingerprint().is_empty());

    f.write(
        ".ai/repo/why/areas/second-area.md",
        &common::AREA.replace("fixture-area", "second-area"),
    );
    f.commit("another area");
    let (_app3, moved) = catalogue(&f);
    assert_ne!(first.fingerprint(), moved.fingerprint());
}

// ---------------------------------------------------------------- what the schema refuses

/// The schema is the contract, and it runs before the domain sees anything. Each of these
/// writes one broken record and asserts the index refused it, with the reason named.
fn refused(body: &str, code: &str, fragment: &str) {
    let f = Fixture::new();
    f.write(".ai/repo/why/moments/broken.md", body);
    f.commit("broken");
    // a degraded index is the expected outcome here: `--inspect` exits 10 for it
    let (code_seen, v, _err) = common::inspect(&f.root(), &[]);
    assert_eq!(code_seen, 10, "a refused record degrades the index");
    let diagnostics = v["repository"]["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let hit = diagnostics.iter().any(|d| {
        d["code"] == code
            && d["message"].as_str().is_some_and(|m| m.contains(fragment))
            && d["path"] == ".ai/repo/why/moments/broken.md"
    });
    assert!(
        hit,
        "expected a {code} diagnostic naming {fragment:?}; got {diagnostics:#?}"
    );
}

#[test]
fn an_unknown_schema_version_is_refused_rather_than_guessed() {
    refused(
        &common::MOMENT
            .replace("schema: moment/v1", "schema: moment/v2")
            .replace("id: fixture-moment", "id: broken"),
        "unsupported_version",
        "moment/v2",
    );
}

#[test]
fn a_missing_required_field_is_refused() {
    refused(
        &common::MOMENT
            .replace("id: fixture-moment", "id: broken")
            .replace("severity: medium\n", ""),
        "schema_violation",
        "severity",
    );
}

#[test]
fn a_value_outside_the_enum_is_refused() {
    refused(
        &common::MOMENT
            .replace("id: fixture-moment", "id: broken")
            .replace("frequency: common", "frequency: hourly"),
        "schema_violation",
        "frequency",
    );
}

#[test]
fn a_derived_key_written_into_a_source_is_refused() {
    refused(
        &common::MOMENT
            .replace("id: fixture-moment", "id: broken")
            .replace("weight: 10", "weight: 10\nroute: /why/broken/"),
        "unknown_key",
        "route",
    );
}

#[test]
fn front_matter_that_does_not_parse_is_refused() {
    refused(
        "---\nschema: moment/v1\n\tid: broken\n---\n\n## The moment\n",
        "malformed_front_matter",
        "",
    );
}

#[test]
fn two_files_claiming_one_identity_are_both_excluded_and_the_catalogue_says_so() {
    let f = Fixture::new();
    f.write(".ai/repo/why/moments/a-copy.md", common::MOMENT);
    f.commit("duplicate");
    let (_app, c) = catalogue(&f);
    assert!(c.moment("fixture-moment").is_none(), "both are excluded");
    assert!(
        c.findings()
            .iter()
            .any(|d| d.code == "duplicate_identity" && d.severity == Severity::Error),
        "the catalogue reports the exclusion rather than reporting itself valid: {:?}",
        c.findings()
    );
}

// ---------------------------------------------------------------- referential integrity

#[test]
fn an_unresolved_reference_is_an_error_with_the_nearest_candidate() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/why/moments/typo.md",
        &common::MOMENT
            .replace("id: fixture-moment", "id: typo")
            .replace("audiences: [fixture-team]", "audiences: [fixture-tea]"),
    );
    f.commit("typo");
    let (_app, c) = catalogue(&f);
    let found = c
        .findings()
        .iter()
        .find(|d| d.code == "unknown_reference" && d.id.as_deref() == Some("typo"))
        .expect("the unresolved audience is a finding");
    assert_eq!(found.field.as_deref(), Some("audiences"));
    assert!(found.message.contains("fixture-tea"));
    assert_eq!(found.did_you_mean.as_deref(), Some("fixture-team"));
    assert!(c.errors() > 0);
}

#[test]
fn a_related_moment_that_does_not_exist_is_an_error() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/why/moments/dangling.md",
        &common::MOMENT
            .replace("id: fixture-moment", "id: dangling")
            .replace(
                "tags: [fixture]",
                "tags: [fixture]\nrelated: [no-such-moment]",
            ),
    );
    f.commit("dangling");
    let (_app, c) = catalogue(&f);
    assert!(c.findings().iter().any(|d| d.code == "unknown_reference"
        && d.field.as_deref() == Some("related")
        && d.message.contains("no-such-moment")));
}

#[test]
fn a_file_whose_name_disagrees_with_its_id_is_an_error_naming_the_name_it_should_have() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/why/moments/not-the-id.md",
        &common::MOMENT.replace("id: fixture-moment", "id: some-other-id"),
    );
    f.commit("misnamed");
    let (_app, c) = catalogue(&f);
    let found = c
        .findings()
        .iter()
        .find(|d| d.code == "filename_mismatch")
        .expect("the mismatch is a finding");
    assert_eq!(found.did_you_mean.as_deref(), Some("some-other-id.md"));
}

#[test]
fn a_public_record_below_its_floor_is_a_warning_and_not_an_error() {
    let f = Fixture::new();
    let one_example = common::MOMENT
        .replace("id: fixture-moment", "id: thin")
        .split("  - id: two")
        .next()
        .unwrap()
        .to_string()
        + "claims: [policy-parse]\n---\n\n## The moment\n\nx\n\n## Why it happens\n\nx\n\n## What it does not do\n\nx\n";
    f.write(".ai/repo/why/moments/thin.md", &one_example);
    f.commit("thin");
    let (_app, c) = catalogue(&f);
    assert!(c.moment("thin").is_some(), "it is still read");
    let w = c
        .findings()
        .iter()
        .find(|d| d.id.as_deref() == Some("thin") && d.code == "missing_content")
        .expect("the floor is a finding");
    assert_eq!(w.severity, Severity::Warning);
    assert!(w.message.contains("three concrete examples"));
}

#[test]
fn a_draft_is_exempt_from_the_content_floors() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/why/moments/a-draft.md",
        &common::MOMENT
            .replace("id: fixture-moment", "id: a-draft")
            .replace("status: stable", "status: draft"),
    );
    f.commit("draft");
    let (_app, c) = catalogue(&f);
    assert!(c.moment("a-draft").is_some());
    assert!(!c
        .findings()
        .iter()
        .any(|d| d.id.as_deref() == Some("a-draft") && d.code == "missing_content"));
}

// ---------------------------------------------------------------- queries and derivation

#[test]
fn membership_and_every_reverse_relation_are_derived_from_the_moments() {
    let f = Fixture::new();
    let (_app, c) = catalogue(&f);
    // an audience holds what names it, and its own file holds no list
    let members = c.naming("audience", "fixture-team");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].id, "fixture-moment");
    let audience = c.audience("fixture-team").expect("the audience is read");
    let raw = std::fs::read_to_string(f.root().join(&audience.source)).unwrap();
    assert!(
        !raw.contains("fixture-moment"),
        "the audience's own file must not list its members"
    );
    // the responsibility comes through the claim, which declares it
    assert_eq!(c.responsibilities("fixture-moment"), ["policy"]);
    // a claim's reverse index is the same relation read the other way
    assert_eq!(c.naming("claim", "policy-parse").len(), 1);
}

#[test]
fn a_backlink_is_derived_and_never_authored() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/why/moments/names-it.md",
        &common::MOMENT
            .replace("id: fixture-moment", "id: names-it")
            .replace(
                "tags: [fixture]",
                "tags: [fixture]\nrelated: [fixture-moment]",
            ),
    );
    f.commit("related");
    let (_app, c) = catalogue(&f);
    assert_eq!(c.backlinks("fixture-moment"), ["names-it"]);
    assert!(
        c.backlinks("names-it").is_empty(),
        "the reverse edge points one way only"
    );
}

#[test]
fn every_filter_value_the_facets_offer_selects_something() {
    let f = Fixture::new();
    let (_app, c) = catalogue(&f);
    let facets = c.facets();
    assert!(!facets.audiences.is_empty() && !facets.areas.is_empty());
    for v in facets
        .audiences
        .iter()
        .chain(facets.areas.iter())
        .chain(facets.tags.iter())
        .chain(facets.severities.iter())
    {
        assert!(v.count > 0, "a facet value with no moment is not offered");
    }
    let by_audience = c.select(&Query {
        audience: Some("fixture-team".into()),
        ..Query::default()
    });
    assert_eq!(by_audience.len(), 1);
    let by_nothing = c.select(&Query {
        tag: Some("no-such-tag".into()),
        ..Query::default()
    });
    assert!(by_nothing.is_empty());
}

#[test]
fn search_reads_the_body_the_signals_and_the_examples() {
    let f = Fixture::new();
    let (_app, c) = catalogue(&f);
    for needle in [
        "fixture says so",
        "recognised its own moment",
        "the catalogue does",
    ] {
        let hits = c.select(&Query {
            q: Some(needle.into()),
            ..Query::default()
        });
        assert_eq!(hits.len(), 1, "searching for {needle:?}");
    }
}

#[test]
fn a_diagnosis_counts_and_says_which_moment_produced_each_row() {
    let f = Fixture::new();
    let (_app, c) = catalogue(&f);
    let d = c.diagnose(&["fixture-signal".into(), "not-a-signal".into()]);
    assert_eq!(d.moments, ["fixture-moment"]);
    assert_eq!(d.unresolved, ["not-a-signal"]);
    let area = d.areas.first().expect("an area is implied");
    assert_eq!(area.id, "fixture-area");
    assert_eq!(area.count, 1);
    assert_eq!(area.matched_because, ["fixture-moment"]);
    // a moment id selects the same moment a signal of it does
    let by_moment = c.diagnose(&["fixture-moment".into()]);
    assert_eq!(by_moment.moments, d.moments);
    // and nothing selected is an empty diagnosis, not an error
    assert!(c.diagnose(&[]).moments.is_empty());
}

// ---------------------------------------------------------------- the projections

#[test]
fn one_file_added_is_answered_by_every_projection_and_removing_it_removes_it_from_all() {
    let f = Fixture::new();
    let probe = common::MOMENT
        .replace("id: fixture-moment", "id: probe")
        .replace(
            "hook: 'recognised the moment the fixture declares'",
            "hook: 'probed the catalogue with one added file'",
        )
        .replace("featured: true", "featured: false");
    f.write(".ai/repo/why/moments/probe.md", &probe);
    f.commit("one file");

    // the domain
    let (app, c) = catalogue(&f);
    assert!(c.moment("probe").is_some());
    // the capability every other surface projects
    let listed = app
        .context
        .execute("why.list", json!({}))
        .expect("why.list answers");
    assert!(listed["moments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == "probe"));
    assert_eq!(listed["counts"]["moments"], 2);
    let one = app
        .context
        .execute("why.moment", json!({ "id": "probe" }))
        .expect("why.moment answers");
    assert_eq!(one["route"], "/why/probe/");
    // MCP, as a resource of the layer, with no tool added for it
    let (_code, v, _err) = common::inspect(&f.root(), &[]);
    assert!(common::resource_uris(&v).contains(&"majordomus://moment/probe".to_string()));
    // the site projection
    let artifacts =
        majordomus_cli::site::why_artifacts(&app.context).expect("the site dataset is written");
    let dataset = artifacts
        .iter()
        .find(|a| a.path.ends_with("why.json"))
        .expect("why.json is an artifact");
    assert!(dataset.content.contains("\"probe\""));
    assert!(dataset.content.contains("\"/why/probe/\""));
    // the derived graph
    let g = majordomus_cli::graph::derive("why", &app.context.registry, &app.context.index)
        .expect("the why graph is derived");
    assert!(g.nodes.iter().any(|n| n.id == "moment:probe"));

    // ---- and now it is removed, with nothing else changed
    std::fs::remove_file(f.root().join(".ai/repo/why/moments/probe.md")).unwrap();
    f.commit("one file removed");
    let (app, c) = catalogue(&f);
    assert!(c.moment("probe").is_none());
    let listed = app.context.execute("why.list", json!({})).unwrap();
    assert!(!listed["moments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == "probe"));
    assert!(app
        .context
        .execute("why.moment", json!({ "id": "probe" }))
        .is_err());
    let (_code, v, _err) = common::inspect(&f.root(), &[]);
    assert!(!common::resource_uris(&v).contains(&"majordomus://moment/probe".to_string()));
    let artifacts = majordomus_cli::site::why_artifacts(&app.context).unwrap();
    assert!(!artifacts
        .iter()
        .find(|a| a.path.ends_with("why.json"))
        .unwrap()
        .content
        .contains("\"probe\""));
    let g =
        majordomus_cli::graph::derive("why", &app.context.registry, &app.context.index).unwrap();
    assert!(!g.nodes.iter().any(|n| n.id == "moment:probe"));
}

#[test]
fn the_site_dataset_is_deterministic_and_carries_no_prose() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let first = majordomus_cli::site::why_artifacts(&app.context).unwrap();
    let again = majordomus_cli::site::why_artifacts(&app.context).unwrap();
    assert_eq!(first, again, "the same tree produces the same bytes");
    let dataset = first.iter().find(|a| a.path.ends_with("why.json")).unwrap();
    assert!(
        !dataset.content.contains("Because the fixture says so"),
        "the bodies stay in the files the records name"
    );
    assert!(dataset.content.contains("\"fingerprint\""));
    assert!(dataset.content.contains("\"signals\""));
}

#[test]
fn an_unknown_facet_value_is_an_invalid_input_naming_the_ones_that_exist() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let e = app
        .context
        .execute("why.list", json!({ "audience": "no-such-audience" }))
        .expect_err("an audience the catalogue does not have is refused");
    assert!(e.to_string().contains("fixture-team"), "got {e}");
}

#[test]
fn a_repository_with_no_catalogue_answers_an_empty_one_rather_than_failing() {
    let f = Fixture::new();
    for p in [
        ".ai/repo/why/moments/fixture-moment.md",
        ".ai/repo/why/audiences/fixture-team.md",
        ".ai/repo/why/areas/fixture-area.md",
    ] {
        std::fs::remove_file(f.root().join(p)).unwrap();
    }
    f.commit("no catalogue");
    let app = common::load_app(&f);
    let listed = app.context.execute("why.list", json!({})).unwrap();
    assert_eq!(listed["counts"]["moments"], 0);
    let report = app.context.execute("why.validate", json!({})).unwrap();
    assert_eq!(report["valid"], true, "an empty catalogue is a valid one");
}

#[test]
fn a_moment_may_not_claim_a_route_the_section_owns() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/why/moments/areas.md",
        &common::MOMENT.replace("id: fixture-moment", "id: areas"),
    );
    f.commit("reserved");
    let (_app, c) = catalogue(&f);
    let found = c
        .findings()
        .iter()
        .find(|d| d.code == "reserved_identity")
        .expect("a reserved identity is a finding");
    assert!(found.message.contains("route this section owns"));
    assert_eq!(c.errors(), 1);
}
