//! Attaching a client to the shared server, projected.
//!
//! The behavioural claims these tests exist for: what a client needs is read from the
//! provider declarations and from this checkout — never from a table of vendors written
//! into a surface — and asking the question does not answer it wrongly by taking the lease.

mod common;

use common::Fixture;
use majordomus_cli::capability::builtin::connect::{self, SERVER_NAME};
use serde_json::json;

fn report(f: &Fixture, input: serde_json::Value) -> serde_json::Value {
    let app = common::load_app(f);
    app.context
        .execute("connect.list", input)
        .unwrap_or_else(|e| panic!("connect.list refused: {e:?}"))
}

fn client<'a>(report: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    report["clients"]
        .as_array()
        .expect("clients")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no client {id} in {report:#}"))
}

#[test]
fn the_module_declares_one_capability_and_every_projection_of_it() {
    let m = connect::module();
    assert_eq!(m.id.as_str(), "connect");
    let c = &m.capabilities[0].capability;
    assert_eq!(c.id.to_string(), "connect.list");
    assert!(
        c.exposure.mcp.is_some() && c.exposure.http.is_some() && c.exposure.cli.is_some(),
        "one answer, four surfaces: {:?}",
        c.exposure
    );
}

#[test]
fn a_client_that_keeps_its_configuration_in_the_application_gets_the_procedure_filled_in() {
    let f = Fixture::new();
    let r = report(&f, json!({ "client": "chatgpt" }));

    let chatgpt = client(&r, "chatgpt");
    assert_eq!(chatgpt["configured_in"], "application");
    assert!(
        chatgpt["config_path"].is_null(),
        "the app reads no file in the tree: {chatgpt:#}"
    );
    let setup = chatgpt["setup"].as_str().expect("a setup");
    assert!(
        !setup.contains('{'),
        "every token is filled for this checkout: {setup}"
    );
    assert!(
        setup.contains(r["stdio"]["command"].as_str().unwrap()),
        "the stdio form names the absolute launcher: {setup}"
    );
    assert!(
        setup.contains("MCP servers"),
        "the vendor's own procedure is what is printed: {setup}"
    );
    assert_eq!(
        r["name"], SERVER_NAME,
        "one repository is one server under one name on every board"
    );
}

#[test]
fn a_client_that_reads_a_file_is_answered_with_what_the_checkout_holds() {
    let f = Fixture::new();
    let absent = report(&f, json!({ "client": "claude-code" }));
    let before = client(&absent, "claude-code");
    assert_eq!(before["config_path"], ".mcp.json");
    assert_eq!(before["configured"], false);
    assert!(
        before["note"]
            .as_str()
            .is_some_and(|n| n.contains(".mcp.json")),
        "the absence is named: {before:#}"
    );

    f.write(".mcp.json", "{\"mcpServers\":{\"majordomus\":{}}}\n");
    f.commit("client config");
    let present = report(&f, json!({ "client": "claude-code" }));
    let after = client(&present, "claude-code");
    assert_eq!(after["configured"], true);
    assert_eq!(
        after["setup"].as_str().unwrap(),
        "{\"mcpServers\":{\"majordomus\":{}}}\n",
        "the file's own bytes are the configuration, not a rendering of it"
    );
}

#[test]
fn asking_how_to_connect_does_not_take_the_lease() {
    let f = Fixture::new();
    let r = report(&f, json!({}));

    assert_eq!(r["server"]["running"], false);
    assert!(
        r["server"]["reason"].as_str().is_some(),
        "why there is no endpoint is said: {r:#}"
    );
    let lease = f.path(r["server"]["lease"].as_str().expect("the lease path"));
    assert!(
        !lease.exists(),
        "reading the lease must not create one: {}",
        lease.display()
    );
}

#[test]
fn every_provider_of_the_distribution_is_a_client_and_an_unknown_one_is_refused() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let declared: Vec<String> = app
        .context
        .index
        .providers
        .providers
        .iter()
        .map(|p| p.id.clone())
        .collect();

    let r = report(&f, json!({}));
    let answered: Vec<String> = r["clients"]
        .as_array()
        .expect("clients")
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        answered, declared,
        "the clients are the providers, in their order"
    );

    let err = app
        .context
        .execute("connect.list", json!({ "client": "nonesuch" }))
        .expect_err("an unknown client is refused");
    assert!(
        format!("{err:?}").contains("nonesuch"),
        "the refusal names it: {err:?}"
    );
}
