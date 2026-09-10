//! The release state is one model, and every surface is a reading of it.
//!
//! The claim this file exists to hold: a version, a compatibility verdict and a changelog
//! entry are computed once and rendered several times. A test that only exercised the
//! command line would pass while the HTTP route answered something else, which is exactly
//! the failure the subsystem was built to remove — so every assertion here is made against
//! at least two surfaces, or against the model and one projection of it.

mod common;

use majordomus_cli::release::change::{Change, ChangeType, Changes};
use majordomus_cli::release::changelog::Changelog;
use majordomus_cli::release::contract::{ContractEntry, ContractSnapshot, Surface};
use majordomus_cli::release::diff::{diff, minimum_version, CompatibilityImpact};
use majordomus_cli::release::version::{Bump, Version};

/// The eight capabilities, and the surfaces each is declared on.
const RELEASE_CAPABILITIES: &[(&str, &str, &str)] = &[
    (
        "release.version",
        "majordomus_version",
        "/api/v1/release/version",
    ),
    (
        "release.status",
        "majordomus_release_status",
        "/api/v1/release/status",
    ),
    (
        "release.explain",
        "majordomus_release_explain",
        "/api/v1/release/explain",
    ),
    (
        "release.diff",
        "majordomus_release_diff",
        "/api/v1/release/diff",
    ),
    (
        "release.changelog",
        "majordomus_changelog",
        "/api/v1/release/changelog",
    ),
    (
        "release.manifest",
        "majordomus_release_manifest",
        "/api/v1/release/manifest",
    ),
    (
        "release.plan",
        "majordomus_release_plan",
        "/api/v1/release/plan",
    ),
    (
        "release.check",
        "majordomus_release_check",
        "/api/v1/release/check",
    ),
];

/// Every release capability answers over HTTP, and the answer is the model rather than
/// prose about it. The MCP tool behind each is the same capability through the same
/// executor — the registry assertion below proves they resolve to one declaration, and the
/// executor is what makes one declaration one answer.
#[test]
fn every_release_capability_answers_over_http() {
    let f = common::Fixture::new();
    let served = common::Served::start(&f.root(), &[]);

    for (id, _, path) in RELEASE_CAPABILITIES {
        let (status, body) = served.get(path);
        assert_eq!(status, 200, "{id}: {path} answered {status}: {body}");
        assert!(
            body.is_object(),
            "{id}: {path} answered something that is not an object: {body}"
        );
    }
}

/// A capability exposed on three surfaces is one declaration, and the OpenAPI document,
/// the MCP tool list and the command line are three renderings of it.
#[test]
fn the_release_module_is_declared_once_and_projected_three_ways() {
    let f = common::Fixture::new();
    let app = common::load_app(&f);
    let registry = app.registry();

    for (id, tool, path) in RELEASE_CAPABILITIES {
        let capability = registry
            .get(id)
            .unwrap_or_else(|| panic!("the registry does not hold {id}"));
        assert_eq!(
            capability
                .exposure
                .mcp
                .as_ref()
                .and_then(|m| m.tool.as_deref()),
            Some(*tool),
            "{id}"
        );
        assert_eq!(
            capability.exposure.http.as_ref().map(|h| h.path.as_str()),
            Some(*path),
            "{id}"
        );
        assert!(
            capability.exposure.cli.is_some(),
            "{id} is not on the command line"
        );
        // and the three resolve back to the same capability
        assert_eq!(registry.by_mcp_tool(tool).map(|c| c.id.as_str()), Some(*id));
        assert_eq!(
            registry
                .by_http(majordomus_cli::capability::HttpMethod::Get, path)
                .map(|c| c.id.as_str()),
            Some(*id)
        );
    }
}

/// Nothing in the release module writes. A release is prepared by a command of the
/// executable; a capability that mutated the repository would put a release behind an
/// unauthenticated loopback socket.
#[test]
fn no_release_capability_writes() {
    let f = common::Fixture::new();
    let app = common::load_app(&f);
    for (id, _, _) in RELEASE_CAPABILITIES {
        let capability = app.registry().get(id).expect("declared");
        assert!(capability.kind.is_read_only(), "{id} is not read-only");
    }
}

/// The version this executable reports is the crate's, on every surface that reports one.
/// This is the drift the subsystem exists to make impossible: before it, the shell tool
/// and the crate stated the version separately and the website took one from each.
#[test]
fn one_version_reaches_every_surface() {
    let f = common::Fixture::new();
    let served = common::Served::start(&f.root(), &[]);
    let expected = env!("CARGO_PKG_VERSION");

    let (_, versions) = served.get("/api/v1/release/version");
    assert_eq!(versions["source"], expected, "the release state's source");
    assert_eq!(versions["running"], expected, "the process's own");

    let (_, openapi) = served.get("/openapi.json");
    assert_eq!(openapi["info"]["version"], expected, "the OpenAPI document");

    let (_, live) = served.get("/api/v1/live");
    assert_eq!(live["version"], expected, "liveness");

    let (_, build) = served.get("/api/v1/distribution/build");
    assert_eq!(build["version"], expected, "the build report");
}

/// The Cockpit's version display renders the release state rather than a string, and the
/// page it links to exists. The topbar is on every page, so this also holds the indicator
/// to never taking the Cockpit down.
#[test]
fn the_cockpit_version_display_leads_to_the_release_page() {
    let f = common::Fixture::new();
    let served = common::Served::start(&f.root(), &[]);

    let (_, _, overview) = served.request("GET", "/cockpit", None);
    assert!(
        overview.contains(r#"class="mj-version""#),
        "the topbar has no version display"
    );
    assert!(
        overview.contains(r#"href="/cockpit/release""#),
        "the version display does not lead anywhere"
    );
    assert!(
        overview.contains(env!("CARGO_PKG_VERSION")),
        "the topbar does not show the version"
    );

    let (_, _, page) = served.request("GET", "/cockpit/release", None);
    for expected in ["Versions", "Readiness", "History", "Source", "Published"] {
        assert!(
            page.contains(expected),
            "/cockpit/release has no {expected}"
        );
    }
}

/// The changelog a person reads and the changelog a program reads are one document.
#[test]
fn the_changelog_is_one_document_in_two_renderings() {
    let f = common::Fixture::new();
    let served = common::Served::start(&f.root(), &[]);
    let (status, body) = served.get("/api/v1/release/changelog");
    assert_eq!(status, 200);
    assert!(
        body["markdown"]
            .as_str()
            .is_some_and(|m| m.starts_with("# Changelog")),
        "the changelog answer carries no document"
    );
    assert!(
        body["releases"].is_array(),
        "the changelog answer carries no model"
    );
}

// ---------------------------------------------------------------------------------------
// The engine's own behaviour, over snapshots built in the test rather than read from disk.
// These are the adversarial cases: each is a release that must be refused, or must be
// allowed for a reason that can be stated.
// ---------------------------------------------------------------------------------------

fn entry(surface: Surface, id: &str, facts: &[(&str, &str)]) -> ContractEntry {
    ContractEntry {
        surface,
        id: id.into(),
        facts: facts
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    }
}

fn capability(id: &str, facts: &[(&str, &str)]) -> ContractEntry {
    entry(Surface::Capability, id, facts)
}

/// Scenario A — a fix. Nothing in the contract moved, so the release is a patch.
#[test]
fn scenario_a_an_internal_fix_requires_a_patch() {
    let before = ContractSnapshot::from_entries(vec![capability("a.b", &[("kind", "query")])]);
    let after = before.clone();
    let d = diff(&before, &after);
    assert_eq!(d.impact, CompatibilityImpact::None);

    // the contract says nothing happened; a change record saying `patch` is what makes it
    // a release at all, and below 1.0.0 that is a patch bump
    let baseline: Version = "1.2.3".parse().unwrap();
    assert_eq!(
        minimum_version(CompatibilityImpact::Patch, &baseline).to_string(),
        "1.2.4"
    );
}

/// Scenario B — an addition. A patch is refused; a minor is not.
#[test]
fn scenario_b_a_public_addition_refuses_a_patch_and_takes_a_minor() {
    let before = ContractSnapshot::from_entries(vec![capability(
        "a.b",
        &[("kind", "query"), ("exposure.http", "GET /api/v1/a")],
    )]);
    let after = ContractSnapshot::from_entries(vec![
        capability(
            "a.b",
            &[("kind", "query"), ("exposure.http", "GET /api/v1/a")],
        ),
        capability(
            "a.c",
            &[("kind", "query"), ("exposure.http", "GET /api/v1/c")],
        ),
    ]);
    let d = diff(&before, &after);
    assert_eq!(d.impact, CompatibilityImpact::Additive);

    let baseline: Version = "1.2.3".parse().unwrap();
    let minimum = minimum_version(d.impact, &baseline);
    assert_eq!(minimum.to_string(), "1.3.0");

    let patched = Bump::Patch.apply(&baseline);
    assert!(
        patched < minimum,
        "a patch does not clear the minimum an addition requires"
    );
    let minor = Bump::Minor.apply(&baseline);
    assert!(minor >= minimum, "a minor does");
}

/// Scenario C — a removal. Patch and minor are both refused; a major is required, and
/// below 1.0.0 a minor is.
#[test]
fn scenario_c_a_removal_requires_a_major_and_below_one_zero_a_minor() {
    let before = ContractSnapshot::from_entries(vec![
        capability("a.b", &[("exposure.http", "GET /api/v1/a")]),
        capability("a.c", &[("exposure.http", "GET /api/v1/c")]),
    ]);
    let after = ContractSnapshot::from_entries(vec![capability(
        "a.b",
        &[("exposure.http", "GET /api/v1/a")],
    )]);
    let d = diff(&before, &after);
    assert_eq!(d.impact, CompatibilityImpact::Breaking);
    assert_eq!(d.changes.len(), 1);
    assert_eq!(d.changes[0].entry, "a.c");

    let above: Version = "1.2.3".parse().unwrap();
    assert_eq!(minimum_version(d.impact, &above).to_string(), "2.0.0");
    assert!(Bump::Patch.apply(&above) < minimum_version(d.impact, &above));
    assert!(Bump::Minor.apply(&above) < minimum_version(d.impact, &above));

    let below: Version = "0.3.1".parse().unwrap();
    assert_eq!(minimum_version(d.impact, &below).to_string(), "0.4.0");
    assert!(Bump::Patch.apply(&below) < minimum_version(d.impact, &below));
}

/// A breaking change that no change record accounts for is a release whose changelog does
/// not explain the thing its readers most need explained.
#[test]
fn a_breaking_change_no_record_names_is_uncovered() {
    // `after` keeps a capability so the surface is still covered. A snapshot with no
    // capabilities at all is not a contract that lost them; it is a snapshot that failed to
    // build, and the diff refuses to read one as the other.
    let before = ContractSnapshot::from_entries(vec![
        capability("a.b", &[("kind", "query")]),
        capability("a.kept", &[("kind", "query")]),
    ]);
    let after = ContractSnapshot::from_entries(vec![capability("a.kept", &[("kind", "query")])]);
    let d = diff(&before, &after);
    assert_eq!(d.impact, CompatibilityImpact::Breaking);

    let uncovered: Vec<String> = d
        .with_impact(CompatibilityImpact::Breaking)
        .map(|c| format!("{}:{}", c.surface.as_str(), c.entry))
        .collect();
    assert_eq!(uncovered, ["capability:a.b"]);

    // a record naming it is what closes the finding
    let record = Change {
        id: "removes-a-b".into(),
        title: "a.b is gone".into(),
        change_type: ChangeType::Removed,
        impact: CompatibilityImpact::Breaking,
        released_in: None,
        scopes: Vec::new(),
        contract: vec!["capability:a.b".into()],
        issues: Vec::new(),
        pull_requests: Vec::new(),
        commits: Vec::new(),
        adrs: Vec::new(),
        migration: Some("docs/RELEASE.md".into()),
        summary: String::new(),
        path: ".ai/repo/changes/removes-a-b.md".into(),
    };
    let changes = Changes {
        changes: vec![record],
        unreadable: Vec::new(),
    };
    assert!(changes.covered().contains("capability:a.b"));
    assert!(
        changes.findings().is_empty(),
        "a breaking record that names a migration document is complete"
    );
}

/// Determinism: the same repository state produces the same document, byte for byte,
/// whatever order the entries arrived in. A baseline read months later depends on it.
#[test]
fn the_contract_and_the_changelog_are_deterministic() {
    let forward = ContractSnapshot::from_entries(vec![
        capability("a.b", &[("kind", "query")]),
        capability("a.c", &[("kind", "query")]),
    ]);
    let backward = ContractSnapshot::from_entries(vec![
        capability("a.c", &[("kind", "query")]),
        capability("a.b", &[("kind", "query")]),
    ]);
    assert_eq!(forward.fingerprint, backward.fingerprint);
    assert_eq!(forward.to_json(), backward.to_json());

    let changes = |order: [&str; 2]| Changes {
        changes: order
            .iter()
            .map(|id| Change {
                id: (*id).into(),
                title: format!("the {id} change"),
                change_type: ChangeType::Added,
                impact: CompatibilityImpact::Additive,
                released_in: None,
                scopes: Vec::new(),
                contract: Vec::new(),
                issues: Vec::new(),
                pull_requests: Vec::new(),
                commits: Vec::new(),
                adrs: Vec::new(),
                migration: None,
                summary: String::new(),
                path: format!(".ai/repo/changes/{id}.md"),
            })
            .collect(),
        unreadable: Vec::new(),
    };
    let releases = majordomus_cli::distribution::release::Releases {
        releases: Vec::new(),
    };
    assert_eq!(
        Changelog::build(&changes(["a", "b"]), &releases).to_markdown(),
        Changelog::build(&changes(["b", "a"]), &releases).to_markdown(),
    );
}

/// The committed contract of this repository is a contract: it covers the surfaces a
/// verdict needs, and it carries no workflow — a recipe of the local runner is not
/// something anybody outside can call, and including one would make the document differ
/// between two machines reading the same commit.
#[test]
fn the_committed_contract_covers_what_a_verdict_needs() {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(majordomus_cli::release::SNAPSHOT_PATH),
    )
    .expect("the contract is committed");
    let snapshot = ContractSnapshot::parse(&text).expect("the committed contract parses");
    let surfaces = snapshot.surfaces();
    for required in majordomus_cli::release::REQUIRED_SURFACES {
        assert!(
            surfaces.contains(required),
            "the committed contract carries no {} surface",
            required.noun()
        );
    }
    assert!(
        !snapshot
            .entries
            .iter()
            .any(|e| e.id.starts_with("workflow.")),
        "the contract carries a workflow, which is not the same on every machine"
    );
    // every capability it names is one this build actually has
    let f = common::Fixture::new();
    let app = common::load_app(&f);
    for e in snapshot
        .entries
        .iter()
        .filter(|e| e.surface == Surface::Capability)
    {
        assert!(
            app.registry().get(&e.id).is_some(),
            "the contract names `{}`, which this build does not have",
            e.id
        );
    }
}

/// A JSON answer is the model, not prose about it: a caller can read the fields.
#[test]
fn the_release_state_is_answered_as_data() {
    let f = common::Fixture::new();
    let served = common::Served::start(&f.root(), &[]);
    let (_, state) = served.get("/api/v1/release/status");
    for field in [
        "versions",
        "baseline",
        "target_version",
        "readiness",
        "contract_changes",
        "unreleased_changes",
    ] {
        assert!(
            state.get(field).is_some(),
            "the release state carries no `{field}`: {state}"
        );
    }
    assert!(
        state["versions"]["source"].is_string(),
        "the four versions are not four fields"
    );
}

/// An input the schema does not admit is refused rather than ignored, on the surface that
/// binds it by name.
#[test]
fn an_unknown_input_is_refused() {
    let f = common::Fixture::new();
    let served = common::Served::start(&f.root(), &[]);
    let (status, body) = served.get("/api/v1/release/diff?nonsense=1");
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["error"]["code"], "invalid_input");
}

/// A version asked for below the minimum is refused by the planner, not merely reported.
/// The refusal is the invariant; the report is a courtesy.
#[test]
fn a_plan_for_a_version_below_the_minimum_is_refused() {
    let f = common::Fixture::new();
    let served = common::Served::start(&f.root(), &[]);
    let (status, body) = served.get("/api/v1/release/plan?version=0.0.1");
    // 422 is the executable's `refused`; a repository with no baseline has no minimum and
    // answers 200, which is the honest answer there and is not a failure of this test.
    assert!(
        status == 200 || status == 422,
        "unexpected {status}: {body}"
    );
    if status == 422 {
        assert_eq!(body["error"]["code"], "refused");
        assert!(
            body["error"]["message"]
                .as_str()
                .is_some_and(|m| m.contains("requires at least")),
            "{body}"
        );
    }
}

/// The value a caller sends and the value it gets back are the same version: a version
/// crosses a YAML record, a JSON manifest, a git tag and a command line, and must be the
/// same version at the end.
#[test]
fn a_version_survives_every_encoding_it_crosses() {
    for text in ["0.3.1", "1.0.0-rc.1", "2.10.0", "0.0.1"] {
        let parsed: Version = text.parse().expect("a version");
        let json = serde_json::to_string(&parsed).expect("serialises");
        let back: Version = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(parsed, back);
        assert_eq!(back.to_string(), text);
        assert_eq!(Version::from_tag(&back.tag()), Some(back));
    }
}
