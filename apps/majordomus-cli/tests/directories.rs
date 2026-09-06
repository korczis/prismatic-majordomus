//! The layer's directory contracts, projected: what a directory owes, which contract
//! decided, and the chain that applies once inheritance is resolved.
//!
//! The behavioural claim these tests exist for: the hierarchy is derived from the index
//! and nothing names a directory. A directory added to the layer appears in the answer —
//! and so in the API, the MCP resource and the Cockpit — with no line of any of them
//! edited. `a_directory_the_repository_adds_reaches_the_answer_with_no_edit` is that
//! claim, run.

mod common;

use common::{context_doc, rule, Fixture};
use serde_json::json;

/// The fixture's sources, with the class that discovers the layer's own contracts. A
/// repository that declares no such class reads exactly as before; one that does gets its
/// contracts as themselves rather than through whichever pathspec happens to reach them.
fn sources_with_context() -> String {
    common::SOURCES.replace(
        "sources:\n",
        "sources:\n  - id: context\n    kind: context\n    discovery: vcs\n    pathspec: ':(glob).ai/**/README.md'\n    required: true\n\n",
    )
}

/// A fixture whose layer declares the context class and carries a root contract.
fn layered() -> Fixture {
    let f = Fixture::new();
    f.write(".ai/repo/knowledge/sources.yaml", &sources_with_context());
    f.write(".ai/README.md", &context_doc("ai.layer", "The layer"));
    f.write(
        ".ai/repo/README.md",
        &context_doc("ai.repo", "The tracked half"),
    );
    f.commit("context class");
    f
}

fn report(f: &Fixture, input: serde_json::Value) -> serde_json::Value {
    let app = common::load_app(f);
    app.context
        .execute("directories.list", input)
        .unwrap_or_else(|e| panic!("directories.list refused: {e:?}"))
}

fn node<'a>(report: &'a serde_json::Value, path: &str) -> &'a serde_json::Value {
    report["directories"]
        .as_array()
        .expect("directories")
        .iter()
        .find(|d| d["path"] == path)
        .unwrap_or_else(|| panic!("no directory {path} in {report:#}"))
}

#[test]
fn a_directory_the_repository_adds_reaches_the_answer_with_no_edit() {
    let f = layered();

    let before = report(&f, json!({}));
    let owed_before = before["tallies"]["owed"].as_u64().unwrap();
    assert!(
        before["directories"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["path"] != ".ai/repo/rules/zones"),
        "a directory that does not exist is in the answer"
    );

    // a directory with content the index reads, and no contract: it owes one
    f.write(
        ".ai/repo/rules/zones/zone-a.v1.md",
        &rule("project.zone-a", 1, "Zone A"),
    );
    f.commit("a zone");
    let owed = report(&f, json!({}));
    let z = node(&owed, ".ai/repo/rules/zones");
    assert_eq!(z["state"], "owed", "{z:#}");
    assert_eq!(z["requires_contract"], true);
    assert_eq!(
        owed["tallies"]["owed"].as_u64().unwrap(),
        owed_before + 1,
        "the new directory did not move the tally"
    );

    // the same directory, once it says what it is for
    f.write(
        ".ai/repo/rules/zones/README.md",
        &context_doc("ai.repo.rules.zones", "Zones"),
    );
    f.commit("the zone says what it is for");
    let documented = report(&f, json!({}));
    let z = node(&documented, ".ai/repo/rules/zones");
    assert_eq!(z["state"], "documented");
    assert_eq!(z["contract"]["id"], "ai.repo.rules.zones");
    assert_eq!(z["contract"]["path"], ".ai/repo/rules/zones/README.md");
    assert_eq!(
        documented["tallies"]["owed"].as_u64().unwrap(),
        owed_before,
        "documenting the directory did not settle the tally"
    );
    assert_eq!(z["parent"], ".ai/repo/rules");

    // and the parent knows its child, because the hierarchy is the index's
    let parent = node(&documented, ".ai/repo/rules");
    assert!(
        parent["children"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == ".ai/repo/rules/zones"),
        "the parent does not name the child: {parent:#}"
    );
}

#[test]
fn an_exemption_releases_the_subtree_it_names_and_no_other() {
    let f = layered();
    f.write(
        ".ai/repo/rules/zones/carried/pkg/installed.v1.md",
        &rule("vendor.installed", 1, "Installed"),
    );
    f.write(
        ".ai/repo/rules/zones/own/written.v1.md",
        &rule("project.written-here", 1, "Written here"),
    );
    f.commit("a carried subtree beside one of our own");

    // with nothing declared, both owe a contract
    let plain = report(&f, json!({}));
    assert_eq!(
        node(&plain, ".ai/repo/rules/zones/carried")["state"],
        "owed"
    );
    assert_eq!(node(&plain, ".ai/repo/rules/zones/own")["state"], "owed");

    // the contract that governs the subtree releases one of them by name
    let doc = context_doc("ai.repo.rules.zones", "Zones").replace(
        "order: 100\n",
        "order: 100\nchildren:\n  require_contract: true\n  exempt: [.ai/repo/rules/zones/carried]\n",
    );
    f.write(".ai/repo/rules/zones/README.md", &doc);
    f.commit("release the carried subtree");

    let r = report(&f, json!({}));
    let carried = node(&r, ".ai/repo/rules/zones/carried");
    assert_eq!(carried["state"], "exempt", "{carried:#}");
    assert_eq!(carried["exempted_by"], ".ai/repo/rules/zones/README.md");
    // the exemption reaches the whole subtree, not only the directory named
    assert_eq!(
        node(&r, ".ai/repo/rules/zones/carried/pkg")["state"],
        "exempt"
    );
    // a directory the repository does write still owes one, vendored sibling or not
    assert_eq!(node(&r, ".ai/repo/rules/zones/own")["state"], "owed");
}

#[test]
fn a_subtree_contract_that_says_its_children_owe_nothing_releases_them_all() {
    let f = layered();
    f.write(
        ".ai/repo/rules/zones/one/one.v1.md",
        &rule("project.one", 1, "One"),
    );
    f.write(
        ".ai/repo/rules/zones/two/two.v1.md",
        &rule("project.two", 1, "Two"),
    );
    let doc = context_doc("ai.repo.rules.zones", "Zones").replace(
        "order: 100\n",
        "order: 100\nchildren:\n  require_contract: false\n",
    );
    f.write(".ai/repo/rules/zones/README.md", &doc);
    f.commit("instances of a kind, not sections of the layer");

    let r = report(&f, json!({}));
    for d in [".ai/repo/rules/zones/one", ".ai/repo/rules/zones/two"] {
        let n = node(&r, d);
        assert_eq!(n["state"], "exempt", "{n:#}");
        assert_eq!(n["requires_contract"], false);
        assert_eq!(n["governed_by"], ".ai/repo/rules/zones/README.md");
    }
    assert!(
        r["directories"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["path"]
                .as_str()
                .unwrap()
                .starts_with(".ai/repo/rules/zones/"))
            .all(|d| d["state"] == "exempt"),
        "a directory below the released subtree still owes a contract"
    );
}

#[test]
fn the_effective_chain_is_root_first_and_says_which_document_is_local() {
    let f = layered();
    f.write(
        ".ai/repo/rules/zones/README.md",
        &context_doc("ai.repo.rules.zones", "Zones"),
    );
    f.write(
        ".ai/repo/rules/zones/zone-a.v1.md",
        &rule("project.zone-a", 1, "Zone A"),
    );
    f.commit("a zone");

    let r = report(&f, json!({ "path": ".ai/repo/rules/zones" }));
    assert_eq!(r["directories"].as_array().unwrap().len(), 1);
    let chain = node(&r, ".ai/repo/rules/zones")["effective"]
        .as_array()
        .unwrap();
    let ids: Vec<&str> = chain.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        vec![
            "ai.layer",
            "ai.repo",
            "ai.repo.rules",
            "ai.repo.rules.zones"
        ]
    );

    // depth ascends: the least specific document applies first
    let depths: Vec<u64> = chain.iter().map(|e| e["depth"].as_u64().unwrap()).collect();
    assert!(depths.windows(2).all(|w| w[0] <= w[1]), "{depths:?}");

    // exactly one document is the directory's own
    let local: Vec<&str> = chain
        .iter()
        .filter(|e| e["local"] == true)
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(local, vec!["ai.repo.rules.zones"]);
    assert!(chain[0]["reason"].as_str().unwrap().contains("ancestor"));
}

#[test]
fn the_whole_tree_answers_without_chains_until_they_are_asked_for() {
    let f = layered();
    let lean = report(&f, json!({}));
    assert!(
        lean["directories"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d.get("effective").is_none()),
        "the tree carried chains nobody asked for"
    );
    let full = report(&f, json!({ "effective": true }));
    assert!(
        full["directories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d.get("effective").is_some()),
        "asking for the chains produced none"
    );
}

#[test]
fn the_state_filter_narrows_the_answer_and_never_the_tallies() {
    let f = layered();
    f.write(
        ".ai/repo/rules/zones/zone-a.v1.md",
        &rule("project.zone-a", 1, "Zone A"),
    );
    f.commit("a zone with no contract");

    let all = report(&f, json!({}));
    let owed = report(&f, json!({ "state": "owed" }));
    assert_eq!(
        owed["tallies"], all["tallies"],
        "a filter changed what the layer is, not what was listed"
    );
    let listed = owed["directories"].as_array().unwrap();
    assert!(listed.iter().all(|d| d["state"] == "owed"));
    assert!(listed.len() < all["directories"].as_array().unwrap().len());
}

#[test]
fn a_path_outside_the_layer_is_refused_and_one_the_index_lacks_is_not_found() {
    let f = layered();
    let app = common::load_app(&f);

    let outside = app
        .context
        .execute("directories.list", json!({ "path": "docs" }))
        .expect_err("a path outside the layer was answered");
    assert!(
        format!("{outside:?}").contains(".ai"),
        "the refusal does not say where the contracts live: {outside:?}"
    );

    let absent = app
        .context
        .execute("directories.list", json!({ "path": ".ai/repo/absent" }))
        .expect_err("a directory the index lacks was answered");
    assert!(
        format!("{absent:?}")
            .to_lowercase()
            .contains("no directory"),
        "{absent:?}"
    );

    let escaping = app
        .context
        .execute("directories.list", json!({ "path": "../etc" }))
        .expect_err("a path leaving the repository was answered");
    assert!(format!("{escaping:?}").contains(".."), "{escaping:?}");
}

#[test]
fn a_document_scoped_to_declared_paths_reaches_them_at_the_depth_it_names() {
    let f = layered();
    f.write(
        ".ai/repo/rules/zones/README.md",
        &context_doc("ai.repo.rules.zones", "Zones"),
    );
    f.write(
        ".ai/repo/rules/zones/zone-a.v1.md",
        &rule("project.zone-a", 1, "Zone A"),
    );
    // a document that sits at the root of the layer but speaks only for one subtree
    let explicit = context_doc("ai.aside", "An aside")
        .replace("scope: subtree", "scope: explicit")
        .replace("order: 100", "order: 50\npaths: [.ai/repo/rules/zones]");
    f.write(".ai/repo/aside/README.md", &explicit);
    f.write(".ai/repo/aside/note.md", "# Note\n");
    f.commit("a document scoped to a path it names");

    let r = report(&f, json!({ "path": ".ai/repo/rules/zones" }));
    let chain = node(&r, ".ai/repo/rules/zones")["effective"]
        .as_array()
        .unwrap();
    let aside = chain
        .iter()
        .find(|e| e["id"] == "ai.aside")
        .unwrap_or_else(|| {
            panic!("the explicit document does not reach the path it names: {chain:#?}")
        });
    assert!(aside["reason"].as_str().unwrap().contains("explicit"));
    // it applies as specifically as the path it names, not as its own directory
    assert_eq!(aside["depth"], 3);

    // and it reaches nothing else
    let elsewhere = report(&f, json!({ "path": ".ai/repo/rules" }));
    assert!(
        node(&elsewhere, ".ai/repo/rules")["effective"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["id"] != "ai.aside"),
        "the explicit document reached a directory it does not name"
    );
}

#[test]
fn a_deprecated_document_is_listed_and_never_applied() {
    let f = layered();
    let deprecated =
        context_doc("ai.repo.rules.zones", "Zones").replace("status: active", "status: deprecated");
    f.write(".ai/repo/rules/zones/README.md", &deprecated);
    f.write(
        ".ai/repo/rules/zones/zone-a.v1.md",
        &rule("project.zone-a", 1, "Zone A"),
    );
    f.commit("a document on its way out");

    let r = report(&f, json!({ "path": ".ai/repo/rules/zones" }));
    let d = node(&r, ".ai/repo/rules/zones");
    // the contract is still declared here, and the directory still counts as documented
    assert_eq!(d["contract"]["status"], "deprecated");
    assert_eq!(d["state"], "documented");
    // but nothing resolves against it
    assert!(
        d["effective"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["id"] != "ai.repo.rules.zones"),
        "a deprecated document was applied: {d:#}"
    );
}

#[test]
fn a_replacing_document_stands_in_for_what_it_supersedes() {
    let f = layered();
    let replacing = context_doc("ai.repo.rules.zones", "Zones")
        .replace("composition: extend", "composition: replace")
        .replace("order: 100", "order: 100\nsupersedes: [ai.repo.rules]");
    f.write(".ai/repo/rules/zones/README.md", &replacing);
    f.write(
        ".ai/repo/rules/zones/zone-a.v1.md",
        &rule("project.zone-a", 1, "Zone A"),
    );
    f.commit("a document that stands in for the one above it");

    let r = report(&f, json!({ "path": ".ai/repo/rules/zones" }));
    let chain = node(&r, ".ai/repo/rules/zones")["effective"]
        .as_array()
        .unwrap();
    let ids: Vec<&str> = chain.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert!(
        ids.contains(&"ai.repo.rules.zones"),
        "the replacing document left the chain: {ids:?}"
    );
    assert!(
        !ids.contains(&"ai.repo.rules"),
        "the superseded document is still applied: {ids:?}"
    );
    // what it does not supersede is untouched
    assert!(
        ids.contains(&"ai.layer") && ids.contains(&"ai.repo"),
        "{ids:?}"
    );
}
