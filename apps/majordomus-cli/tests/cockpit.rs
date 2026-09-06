//! The Cockpit over a real socket: the pages it serves, what they are derived from, and
//! the guarantees that make it a projection rather than a second implementation.
//!
//! The behavioural claim these tests exist for: a capability added to the registry appears
//! in the Cockpit — in its listing, in its navigation, in its search, on a page of its own,
//! with a form generated from its schema — without one line of the Cockpit being edited.
//! `a_capability_the_repository_adds_reaches_every_cockpit_surface` is that claim, run.

mod common;

use common::{Fixture, Served};

/// Every page of the Cockpit that needs no identifier, with the area each belongs to.
const PAGES: &[&str] = &[
    "/cockpit",
    "/cockpit/capabilities",
    "/cockpit/objects",
    "/cockpit/directories",
    "/cockpit/directories?path=.ai/repo/rules",
    "/cockpit/graphs",
    "/cockpit/graphs/topology",
    "/cockpit/continuity",
    "/cockpit/health",
    "/cockpit/artifacts",
    "/cockpit/api",
    "/cockpit/search",
    "/cockpit/activity",
];

fn html(s: &Served, target: &str) -> (u16, String) {
    let (status, headers, body) = s.request("GET", target, None);
    assert!(
        headers
            .iter()
            .any(|(k, v)| k == "content-type" && v.starts_with("text/html")),
        "{target} is not HTML: {headers:?}"
    );
    (status, body)
}

#[test]
fn every_page_renders_complete_html_with_the_shell_and_the_security_headers() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    for page in PAGES {
        let (status, headers, body) = s.request("GET", page, None);
        assert_eq!(status, 200, "{page}");
        assert!(body.starts_with("<!doctype html>"), "{page}");
        assert!(body.trim_end().ends_with("</html>"), "{page}");
        assert!(body.contains("Majordomus Cockpit"), "{page} has no title");
        assert!(body.contains("mj-sidebar"), "{page} has no navigation");
        assert!(body.contains("mj-skip"), "{page} has no skip link");

        let header = |name: &str| {
            headers
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let policy = header("content-security-policy");
        assert!(policy.contains("default-src 'none'"), "{page}: {policy}");
        assert!(!policy.contains("unsafe-eval"), "{page}: {policy}");
        // styles may be inline (a drawing library sets style attributes); scripts never
        let script_src = policy
            .split("script-src ")
            .nth(1)
            .and_then(|s| s.split(';').next())
            .unwrap_or_default();
        assert!(
            !script_src.contains("unsafe-inline"),
            "{page}: {script_src}"
        );
        assert_eq!(header("x-content-type-options"), "nosniff", "{page}");
        assert_eq!(header("referrer-policy"), "no-referrer", "{page}");
    }
}

#[test]
fn a_capability_the_repository_adds_reaches_every_cockpit_surface() {
    // the behavioural guarantee: the Cockpit is a projection. A declarative object of a
    // kind the layer already declares becomes a capability, and the Cockpit shows it in
    // its listing, on a page, in the navigation and in its search — with nothing about it
    // written anywhere in the Cockpit.
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/cockpit-probe.v1.md",
        &common::rule(
            "project.cockpit-probe",
            1,
            "A rule added after the Cockpit was written",
        ),
    );
    f.commit("add a rule");
    let s = Served::start(&f.root(), &[]);

    let id = "rule.project.cockpit-probe@1";
    let uri = "majordomus://rule/project.cockpit-probe@1";

    // 1. the capability listing
    let (status, listing) = html(&s, "/cockpit/capabilities?q=cockpit-probe");
    assert_eq!(status, 200);
    assert!(listing.contains(id), "the listing does not carry {id}");

    // 2. a page of its own, reached by its canonical id
    let (status, page) = html(&s, &format!("/cockpit/capabilities/{}", urlencode(id)));
    assert_eq!(status, 200);
    assert!(
        page.contains("A rule added after the Cockpit was written"),
        "{page}"
    );
    assert!(
        page.contains("Projections"),
        "the page shows its projections"
    );
    assert!(page.contains(uri), "the page shows its MCP resource URI");

    // 3. the object page, and the navigation entry for its kind
    let (status, object) = html(&s, &format!("/cockpit/object?uri={}", urlencode(uri)));
    assert_eq!(status, 200);
    assert!(object.contains("project.cockpit-probe@1"), "{object}");
    assert!(
        object.contains("/cockpit/objects?kind=rule"),
        "the navigation offers the kind"
    );

    // 4. the search
    let (status, found) = html(&s, "/cockpit/search?q=cockpit-probe");
    assert_eq!(status, 200);
    assert!(found.contains(id), "the search does not find it");

    // 5. and the JSON the palette reads is the registry's own listing, not a Cockpit list
    let (status, capabilities) = s.get("/api/v1/capabilities");
    assert_eq!(status, 200);
    assert!(
        capabilities["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == id),
        "the capability listing the palette reads does not carry it"
    );
}

#[test]
fn the_runner_form_is_generated_from_the_input_schema() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    // objects.search takes a required string, an optional integer with bounds and an
    // optional kind: three different controls, none of them written for this capability
    let (status, page) = html(&s, "/cockpit/capabilities/objects.search");
    assert_eq!(status, 200);
    assert!(page.contains("data-mj-runner=\"objects.search\""), "{page}");
    assert!(page.contains("data-mj-path=\"/api/v1/search\""), "{page}");
    assert!(page.contains("data-mj-method=\"GET\""), "{page}");
    assert!(page.contains("name=\"query\""), "the required string");
    assert!(
        page.contains("data-mj-type=\"integer\""),
        "the bounded integer"
    );
    assert!(
        page.contains("required"),
        "requiredness reaches the control"
    );

    // and the examples are the capability's own benchmark cases
    assert!(page.contains("Load into the runner"), "{page}");
    assert!(page.contains("benchmark cases"), "{page}");

    // a capability with no input renders the form and says so, rather than an empty box
    let (_, empty_input) = html(&s, "/cockpit/capabilities/repository.info");
    assert!(
        empty_input.contains("takes no input"),
        "a capability with an empty input says so"
    );
}

#[test]
fn a_graph_page_lists_every_node_and_edge_before_any_library_loads() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, list) = html(&s, "/cockpit/graphs");
    assert_eq!(status, 200);
    for id in ["registry", "layer", "rules", "adrs", "use-cases"] {
        assert!(
            list.contains(&format!("/cockpit/graphs/{id}")),
            "{id} is not listed"
        );
    }

    let (status, page) = html(&s, "/cockpit/graphs/registry");
    assert_eq!(status, 200);
    // the drawing is an enhancement: the nodes and edges are in the HTML
    assert!(
        page.contains("data-mj-graph=\"registry\""),
        "a frame for the drawing"
    );
    assert!(
        page.contains("capability:repository.info"),
        "a node, as text"
    );
    assert!(
        page.contains("projection:http"),
        "a projection node, as text"
    );
    assert!(page.contains("composes"), "an edge kind, with its meaning");
    assert!(
        page.contains("Nodes") && page.contains("Edges"),
        "both tables"
    );

    // the same graph as data, which is what the drawing reads
    let (status, data) = s.get("/api/v1/graph?id=registry");
    assert_eq!(status, 200);
    assert_eq!(data["id"], "registry");
    assert!(data["nodes"].as_array().unwrap().len() > 5);
    assert!(data["metadata"]["acyclic"].is_boolean());

    let (status, missing) = s.get("/api/v1/graph?id=nope");
    assert_eq!(status, 404);
    assert_eq!(missing["error"]["code"], "not_found");
}

/// The artifacts page is the reading half of the generator, and it holds no list of its
/// own: it shows what the committed manifest declares. In a fixture that has never run
/// `majordomus generate` it says so instead of failing, and after a generation every
/// document it names is there in every encoding it is committed in.
#[test]
fn the_artifacts_page_and_route_report_the_generated_tree_and_say_when_there_is_none() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    // nothing generated yet: a fact, not a failure
    let (status, page) = html(&s, "/cockpit/artifacts");
    assert_eq!(status, 200);
    assert!(
        page.contains("no generated tree yet"),
        "the page does not say the manifest is absent"
    );
    let (status, report) = s.get("/api/v1/artifacts");
    assert_eq!(status, 200);
    assert_eq!(report["present"], false);
    assert_eq!(report["tallies"]["artifacts"], 0);
    assert_eq!(report["manifest"], "docs/generated/artifacts.json");
    drop(s);

    // generate, and every encoding of every document is named, current, and typed
    let (code, _, err) = common::run_in(&f.root(), &["generate"], "");
    assert_eq!(code, 0, "{err}");
    let s = Served::start(&f.root(), &[]);
    let (status, report) = s.get("/api/v1/artifacts");
    assert_eq!(status, 200);
    assert_eq!(report["present"], true);
    assert_eq!(report["schema"], "majordomus/generated-artifacts/v1");
    assert_eq!(report["tallies"]["stale"], 0, "{report}");
    assert_eq!(report["tallies"]["missing"], 0, "{report}");
    assert!(report["tallies"]["current"].as_u64().unwrap() > 5);
    let documents = report["documents"].as_array().unwrap();
    let registry = documents
        .iter()
        .find(|d| d["id"] == "registry")
        .expect("the registry document");
    let formats: Vec<&str> = registry["formats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert!(
        formats.contains(&"json") && formats.contains(&"yaml"),
        "the registry is committed in both encodings: {formats:?}"
    );

    // the page shows the same answer, and one filter narrows it
    let (status, page) = html(&s, "/cockpit/artifacts");
    assert_eq!(status, 200);
    assert!(page.contains("docs/generated/registry.yaml"), "{page}");
    assert!(page.contains("majordomus/capability-registry/v1"));
    let (status, only_yaml) = s.get("/api/v1/artifacts?format=yaml");
    assert_eq!(status, 200);
    assert!(only_yaml["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["format"] == "yaml"));

    // an edited artifact is stale, by hash, without regenerating anything
    let path = f.path("docs/generated/registry.yaml");
    std::fs::write(
        &path,
        format!("{}\n", std::fs::read_to_string(&path).unwrap()),
    )
    .unwrap();
    drop(s);
    let s = Served::start(&f.root(), &[]);
    let (_, report) = s.get("/api/v1/artifacts?document=registry");
    assert_eq!(report["tallies"]["stale"], 1, "{report}");
    assert!(report["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["path"] == "docs/generated/registry.yaml" && a["state"] == "stale"));
}

/// What the reading half says when the manifest is not what it should be, and when a file
/// it names is gone. Both are facts about the tree, reported rather than guessed at.
#[test]
fn the_artifacts_capability_reports_a_missing_file_and_refuses_a_manifest_it_cannot_read() {
    let f = Fixture::new();
    let (code, _, err) = common::run_in(&f.root(), &["generate"], "");
    assert_eq!(code, 0, "{err}");

    // a file the manifest names and the tree no longer has
    std::fs::remove_file(f.path("docs/generated/benchmarks.yaml")).unwrap();
    let s = Served::start(&f.root(), &[]);
    let (status, report) = s.get("/api/v1/artifacts?document=benchmarks");
    assert_eq!(status, 200);
    assert_eq!(report["tallies"]["missing"], 1, "{report}");
    assert_eq!(
        report["tallies"]["documents"], 1,
        "one document was asked for"
    );
    assert!(report["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["path"] == "docs/generated/benchmarks.yaml" && a["state"] == "missing"));
    assert!(report["documents"]
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["id"] == "benchmarks"));
    drop(s);

    // a manifest carrying a schema this executable does not read
    let path = f.path("docs/generated/artifacts.json");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace(
            "majordomus/generated-artifacts/v1",
            "majordomus/generated-artifacts/v99",
        ),
    )
    .unwrap();
    let s = Served::start(&f.root(), &[]);
    let (status, body) = s.get("/api/v1/artifacts");
    assert_eq!(status, 500);
    assert!(
        body["error"]["message"].as_str().unwrap().contains("v99"),
        "{body}"
    );
    drop(s);

    // a manifest that is not the document at all
    std::fs::write(&path, "{\"schema\": 1}\n").unwrap();
    let s = Served::start(&f.root(), &[]);
    let (status, body) = s.get("/api/v1/artifacts");
    assert_eq!(status, 500);
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not the manifest this executable writes"),
        "{body}"
    );
    // and the page says so rather than showing a blank one
    let (status, page) = html(&s, "/cockpit/artifacts");
    assert_eq!(status, 500);
    assert!(page.contains("did not answer"), "{page}");
}

#[test]
fn the_health_page_shows_the_verdicts_the_engines_reach() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, page) = html(&s, "/cockpit/health");
    assert_eq!(status, 200);
    for check in [
        "The layer as it was read",
        "The capability registry",
        "The declared scope",
        "Benchmark coverage",
        "Committed projections",
    ] {
        assert!(page.contains(check), "the health page lacks '{check}'");
    }
    assert!(
        page.contains("majordomus generate --check"),
        "a check names the command that reproduces it"
    );

    // and the same thing as data, over the capability every other transport calls
    let (status, health) = s.get("/api/v1/health");
    assert_eq!(status, 200);
    assert!(health["checks"].as_array().unwrap().len() >= 6);
    assert!(health["status"].is_string());
}

#[test]
fn the_continuity_page_shows_what_the_lifecycle_is_holding_and_labels_what_not_to_trust() {
    // The empty page is covered by the sweep over PAGES. This is the other half: a checkout
    // that has been worked in, where every card has something to render and one of the
    // records must be marked as not safe to read as current knowledge.
    let f = Fixture::new();
    let root = f.root();
    let root_s = root.to_string_lossy().to_string();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();

    let record = |created: &str, task: &str, at: &str| {
        format!(
            "---\nschema_version: 1\ncreated_at: {created}\ntask_id: {task}\nprofile: implementation\n\
             owner: \"tester\"\nrepository_id: {root_s}/.git\nworktree: {root_s}\nbranch: {branch}\n\
             head: {at}\nworking_tree: clean\nchanged_files:\n---\n\n# Objective\n\nDo the thing.\n\n\
             # Current State\n\nHalf done.\n\n# Next Action\n\nFinish the thing.\n"
        )
    };
    // written on a commit this repository has never had: the label must say so
    f.write(
        ".ai/local/state/handovers/a.md",
        &record(
            "2026-01-01T00:00:00Z",
            "t-1",
            "0123456789abcdef0123456789abcdef01234567",
        ),
    );
    f.write(
        ".ai/local/state/checkpoints/c.md",
        &record("2026-01-02T00:00:00Z", "t-1", &head),
    );
    f.write(
        ".ai/local/state/session-current.yaml",
        &format!(
            "session_id: s-here\nstarted_at: 2026-01-01T00:00:00Z\nowner: \"tester\"\n\
             worker: claude\nprovider: claude-code\nrepository_id: {root_s}/.git\n\
             worktree: {root_s}\nbranch: {branch}\nstart_head: {head}\nstart_working_tree: clean\n"
        ),
    );
    f.write(
        ".ai/local/state/current.yaml",
        &format!(
            "id: t-1\ntask: \"Do the thing\"\nprofile: implementation\nowner: \"tester\"\n\
             scope:\n  - lib\n  - docs\nstarted_at: 2026-01-01T00:00:00Z\noutcome: active\n\
             repository_id: {root_s}/.git\nworktree: {root_s}\nbranch: {branch}\nhead: {head}\n\
             working_tree: clean\n"
        ),
    );
    f.write(
        ".ai/local/state/open-questions.md",
        "# Open questions\n\n- [unresolved] t-1 — Which budget applies? (2026-01-01)\n",
    );

    let s = Served::start(&root, &[]);
    let (status, page) = html(&s, "/cockpit/continuity");
    assert_eq!(status, 200);

    for text in [
        "s-here",                // the open episode
        "claude-code",           // the provider that opened it
        "Do the thing",          // the active task
        "Finish the thing.",     // the section a resuming worker acts on
        "Which budget applies?", // the blocker
        "diverged",              // the label on the handover
    ] {
        assert!(
            page.contains(text),
            "the continuity page lacks '{text}':\n{page}"
        );
    }
    assert!(
        page.contains("Trust git over anything it says"),
        "a record from a history that no longer exists must say so, not only be labelled"
    );
    assert!(
        page.contains("refuses"),
        "an open question must say what it refuses"
    );

    // and the same state as data, over the capability the page itself called
    let (status, c) = s.get("/api/v1/continuity");
    assert_eq!(status, 200);
    assert_eq!(c["session"]["session_id"], "s-here");
    assert_eq!(c["handover"]["divergence"], "diverged");
    assert_eq!(c["checkpoint"]["divergence"], "exact");
    assert_eq!(c["blockers"].as_array().unwrap().len(), 1);
    assert_eq!(c["task"]["scope"].as_array().unwrap().len(), 2);
}

#[test]
fn assets_are_served_from_the_distribution_with_immutable_urls_and_no_traversal() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (_, page) = html(&s, "/cockpit");
    let versioned = page
        .split("/cockpit/assets/cockpit.css?v=")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("the stylesheet link carries a digest");
    assert_eq!(versioned.len(), 16, "a short digest: {versioned}");

    let (status, headers, body) = s.request(
        "GET",
        &format!("/cockpit/assets/cockpit.css?v={versioned}"),
        None,
    );
    assert_eq!(status, 200);
    assert!(
        body.contains(".mj-card"),
        "the stylesheet defines the components"
    );
    assert!(
        headers
            .iter()
            .any(|(k, v)| k == "cache-control" && v.contains("immutable")),
        "a digest-addressed asset is immutable: {headers:?}"
    );

    // the same file without the digest revalidates instead
    let (status, headers, _) = s.request("GET", "/cockpit/assets/cockpit.css", None);
    assert_eq!(status, 200);
    assert!(headers
        .iter()
        .any(|(k, v)| k == "cache-control" && v == "no-cache"));

    // nothing escapes the asset directory, and nothing outside the served types is served
    for hostile in [
        "/cockpit/assets/../../kinds.yaml",
        "/cockpit/assets/../kinds.yaml",
        "/cockpit/assets/src/cockpit.css/../../../kinds.yaml",
    ] {
        let (status, _, body) = s.request("GET", hostile, None);
        assert_eq!(status, 404, "{hostile} was served: {body}");
        assert!(!body.contains("identity:"), "{hostile} leaked a kinds file");
    }
}

#[test]
fn a_state_changing_request_from_another_origin_is_refused_and_a_read_is_not() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    // a page on another origin posting to the one command this executable has
    let (status, _, body) = s.request_with(
        "POST",
        "/api/v1/peers/announce",
        Some("{\"intent\":\"from a hostile page\"}"),
        &[("Origin", "http://evil.example")],
    );
    assert_eq!(status, 403, "{body}");
    assert!(body.contains("its own origin"), "{body}");

    // the same request from this server's own origin is not refused by the origin check
    // (it is refused later, for having no MCP session, which is a different thing)
    let own = format!("http://{}", s.address);
    let (status, _, body) = s.request_with(
        "POST",
        "/api/v1/peers/announce",
        Some("{\"intent\":\"from the cockpit\"}"),
        &[("Origin", &own)],
    );
    assert_ne!(
        status, 403,
        "a same-origin call is not a cross-origin one: {body}"
    );

    // and a read from anywhere is untouched: a browser cannot see the answer anyway
    let (status, _, _) = s.request_with(
        "GET",
        "/api/v1/repository",
        None,
        &[("Origin", "http://evil.example")],
    );
    assert_eq!(status, 200);
}

#[test]
fn the_index_answers_the_topology_to_a_client_and_the_home_page_to_a_browser() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, index) = s.get("/");
    assert_eq!(status, 200);
    let surfaces = index["surfaces"]
        .as_array()
        .expect("the index lists the surfaces this process serves");
    let mount = |id: &str| {
        surfaces
            .iter()
            .find(|s| s["id"] == id)
            .map(|s| s["path"].as_str().unwrap_or_default().to_string())
    };
    assert_eq!(mount("cockpit").as_deref(), Some("/cockpit"));
    assert_eq!(mount("openapi").as_deref(), Some("/openapi.json"));
    assert_eq!(mount("swagger").as_deref(), Some("/swagger"));

    let (status, _, body) = s.request_with("GET", "/", None, &[("Accept", "text/html")]);
    assert_eq!(status, 200);
    assert!(body.contains("Majordomus"), "{body}");
    assert!(body.contains("/cockpit"), "{body}");
    assert!(body.contains("/swagger"), "{body}");
}

#[test]
fn a_page_the_cockpit_does_not_serve_says_so_without_pretending_to_be_one() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, body) = html(&s, "/cockpit/nothing-here");
    assert_eq!(status, 404);
    assert!(body.contains("serves no page"), "{body}");

    let (status, body) = html(&s, "/cockpit/capabilities/not.a.capability");
    assert_eq!(status, 404);
    assert!(body.contains("no capability"), "{body}");

    let (status, body) = html(&s, "/cockpit/object?uri=majordomus://rule/nope@1");
    assert_eq!(status, 404);
    assert!(body.contains("mj-alert"), "{body}");

    // a mutation is not a page
    let (status, _, _) = s.request("POST", "/cockpit", Some("{}"));
    assert_eq!(status, 405);
}

#[test]
fn repository_content_reaches_the_page_as_text_and_never_as_markup() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/hostile.v1.md",
        &common::rule(
            "project.hostile",
            1,
            "A rule whose title is </h1><script>alert(1)</script>",
        ),
    );
    f.commit("a rule with markup in its title");
    let s = Served::start(&f.root(), &[]);

    for page in [
        "/cockpit/capabilities?q=hostile".to_string(),
        "/cockpit/objects?kind=rule".to_string(),
        format!(
            "/cockpit/object?uri={}",
            urlencode("majordomus://rule/project.hostile@1")
        ),
        "/cockpit/search?q=hostile".to_string(),
    ] {
        let (status, body) = html(&s, &page);
        assert_eq!(status, 200, "{page}");
        assert!(
            !body.contains("<script>alert(1)</script>"),
            "{page} rendered repository content as markup"
        );
        assert!(
            body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "{page} did not show the content at all"
        );
    }
}

#[test]
fn a_page_costs_no_rebuild_of_anything_canonical() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let before = s.get("/api/v1/perf").1;
    for page in PAGES {
        let (status, _, _) = s.request("GET", page, None);
        assert_eq!(status, 200, "{page}");
    }
    let after = s.get("/api/v1/perf").1;

    for counter in [
        "repository_scans",
        "index_builds",
        "registry_builds",
        "schema_generations",
        "http_projection_builds",
    ] {
        assert_eq!(
            before[counter], after[counter],
            "serving the Cockpit moved {counter}: a page rebuilt canonical state"
        );
    }
}

#[test]
fn a_listing_is_read_a_page_at_a_time_and_entered_by_its_parts() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    // every listing of the layer says what window of it is being shown
    for page in ["/cockpit/capabilities", "/cockpit/objects"] {
        let (status, body) = html(&s, page);
        assert_eq!(status, 200, "{page}");
        assert!(
            body.contains("mj-pagination-summary"),
            "{page} does not say what part of the listing it is showing"
        );
        assert!(
            body.contains("mj-chips"),
            "{page} offers no way in but scrolling"
        );
    }

    // a filter chip carries the filters already in force and drops the page number: a
    // reader on page 4 of a search who picks a module lands on that module's first page
    let (_, body) = html(&s, "/cockpit/capabilities?q=repository&page=2");
    let chip = body
        .split(r#"mj-chip" href=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a module chip");
    assert!(chip.contains("q=repository"), "the chip dropped the filter: {chip}");
    assert!(!chip.contains("page="), "the chip kept a page number: {chip}");

    // a page number past the end is a page that exists, not an error and not a panic
    let (status, body) = html(&s, "/cockpit/objects?page=99999");
    assert_eq!(status, 200);
    assert!(body.contains("mj-pagination-summary"), "{body}");
}

fn urlencode(s: &str) -> String {
    majordomus_cli::http::router::percent_encode(s)
}
