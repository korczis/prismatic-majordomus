//! Every Majordomus runtime of this user on this host, from any directory (I3184).
//!
//! A real `majordomus serve` in a fixture of its own, and `majordomus runtimes --host` asked
//! from a directory inside no repository: once with the state directory the server wrote its
//! index entry into, and once with an empty one, where only the process table can find it.
//! The running server answers the same reading over its own API.

mod common;

use std::path::Path;

use common::{Fixture, Served};
use serde_json::Value;

/// `majordomus runtimes --host --format json`, run in `cwd` with `state` as the state
/// directory.
fn runtimes(cwd: &Path, state: &Path) -> Value {
    let out = common::majordomus()
        .args(["runtimes", "--host", "--format", "json"])
        .current_dir(cwd)
        .env("XDG_STATE_HOME", state)
        .output()
        .expect("run majordomus runtimes");
    assert!(
        out.status.success(),
        "runtimes --host exited {}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("runtimes --host prints one JSON document")
}

/// The row of `checkout` in an inventory.
fn row<'a>(inventory: &'a Value, checkout: &Path) -> Option<&'a Value> {
    inventory["runtimes"]
        .as_array()?
        .iter()
        .find(|r| r["checkout"].as_str() == checkout.to_str())
}

#[test]
fn a_host_lists_its_runtimes_from_outside_every_repository() {
    let f = Fixture::new();
    let root = f.root().canonicalize().unwrap();
    let written = tempfile::tempdir().unwrap();
    let empty = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let state = written.path().to_str().unwrap().to_string();
    let server = Served::start_with_env(&root, &[], &[("XDG_STATE_HOME", &state)]);
    let port: u64 = server.address.rsplit(':').next().unwrap().parse().unwrap();

    // the server added its root to the index beside node.json, and nothing else
    let index = written.path().join("majordomus").join("checkout-roots");
    let text = std::fs::read_to_string(&index).expect("the server wrote its index entry");
    assert!(text.lines().any(|l| l == root.to_str().unwrap()), "{text}");

    // asked from outside every repository, with that index: found by both, live
    let inventory = runtimes(outside.path(), written.path());
    let r = row(&inventory, &root).unwrap_or_else(|| panic!("not listed: {inventory:#}"));
    assert_eq!(r["found_by"], serde_json::json!(["index", "process_table"]), "{r:#}");
    assert_eq!(r["live"], true, "{r:#}");
    assert_eq!(r["port"], port);
    assert_eq!(r["version"], majordomus_cli::VERSION);
    assert_eq!(r["version_source"], "lease");
    assert!(r["processes"].as_array().unwrap().iter().any(|p| p["holds_lease"] == true), "{r:#}");
    assert_eq!(inventory["index"]["entries"], 1);

    // with an index that never heard of it: the process table finds it all the same
    let inventory = runtimes(outside.path(), empty.path());
    let r = row(&inventory, &root).unwrap_or_else(|| panic!("not found by the process table: {inventory:#}"));
    assert_eq!(r["found_by"], serde_json::json!(["process_table"]), "{r:#}");
    assert_eq!(r["live"], true, "{r:#}");
    assert!(!empty.path().join("majordomus").exists(), "the reader wrote nothing");

    // and the running server answers the same reading over its own API
    let (status, v) = server.get("/api/v1/runtimes");
    assert_eq!(status, 200, "{v}");
    assert!(row(&v, &root).is_some_and(|r| r["live"] == true), "{v:#}");

    // the text a person reads carries the same row
    let out = common::majordomus()
        .args(["runtimes", "--host"])
        .current_dir(outside.path())
        .env("XDG_STATE_HOME", written.path())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.lines().any(|l| l.ends_with(root.to_str().unwrap()) && l.starts_with("ready")), "{text}");
    drop(server);
}
