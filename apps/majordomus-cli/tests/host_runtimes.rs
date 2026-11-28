//! Every Majordomus runtime of this user on this host, from any directory (I3184).
//!
//! A real `majordomus serve` in a fixture of its own, and `majordomus runtimes --host` asked
//! from a directory inside no repository: once with the state directory the server wrote its
//! index entry into, and once with an empty one, where only the process table can find it.
//! The running server answers the same reading over its own API.

mod common;

use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use common::{Fixture, Served};
use serde_json::{json, Value};

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

/// One `tools/call` of `majordomus_runtimes` over stdio from `cwd`, with `state` as the state
/// directory: the tool's structured answer, as a client receives it.
fn mcp_runtimes(cwd: &Path, state: &Path) -> Value {
    let mut child = common::majordomus()
        .arg("mcp")
        .current_dir(cwd)
        .env("XDG_STATE_HOME", state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("mcp starts");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        let initialize = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "host_runtimes", "version": "0" }
            }
        });
        let call = json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "majordomus_runtimes", "arguments": {} }
        });
        writeln!(stdin, "{initialize}").expect("initialize");
        writeln!(stdin, "{call}").expect("tools/call");
    }
    let out = child.wait_with_output().expect("mcp exits");
    let frames: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect("a JSON-RPC frame"))
        .collect();
    let answer = frames
        .iter()
        .find(|f| f["id"] == 2)
        .unwrap_or_else(|| panic!("no answer to the tools/call: {frames:#?}"));
    assert_eq!(answer["result"]["isError"], false, "{answer:#}");
    answer["result"]["structuredContent"].clone()
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
    assert_eq!(
        r["found_by"],
        serde_json::json!(["index", "process_table"]),
        "{r:#}"
    );
    assert_eq!(r["live"], true, "{r:#}");
    assert_eq!(r["port"], port);
    assert_eq!(r["version"], majordomus_cli::VERSION);
    assert_eq!(r["version_source"], "lease");
    assert!(
        r["processes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["holds_lease"] == true),
        "{r:#}"
    );
    assert_eq!(inventory["index"]["entries"], 1);

    // with an index that never heard of it: the process table finds it all the same
    let inventory = runtimes(outside.path(), empty.path());
    let r = row(&inventory, &root)
        .unwrap_or_else(|| panic!("not found by the process table: {inventory:#}"));
    assert_eq!(r["found_by"], serde_json::json!(["process_table"]), "{r:#}");
    assert_eq!(r["live"], true, "{r:#}");
    assert!(
        !empty.path().join("majordomus").exists(),
        "the reader wrote nothing"
    );

    // and the running server answers the same reading over its own API
    let (status, v) = server.get("/api/v1/runtimes");
    assert_eq!(status, 200, "{v}");
    assert!(row(&v, &root).is_some_and(|r| r["live"] == true), "{v:#}");

    // and an MCP client calling `majordomus_runtimes` is given the same row
    let v = mcp_runtimes(&root, written.path());
    assert!(row(&v, &root).is_some_and(|r| r["live"] == true), "{v:#}");

    // the text a person reads carries the same row
    let out = common::majordomus()
        .args(["runtimes", "--host"])
        .current_dir(outside.path())
        .env("XDG_STATE_HOME", written.path())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.lines()
            .any(|l| l.ends_with(root.to_str().unwrap()) && l.starts_with("ready")),
        "{text}"
    );
    drop(server);
}

/// The host index (`majordomus_cli::lease::host`) is a cache of roots and nothing else: a
/// root is added once however often its server publishes, a root whose checkout is gone is
/// dropped at the next write, a line that is no absolute path is not a root, and a reader of
/// an absent index is given an empty cache rather than an error.
#[test]
fn the_host_index_holds_each_root_once_and_an_absent_one_is_empty() {
    use majordomus_cli::lease::host::{read_index, remember_at, INDEX_FILE};
    let dir = tempfile::tempdir().unwrap();
    let index = dir.path().join("majordomus").join(INDEX_FILE);
    assert_eq!(
        read_index(&index).unwrap(),
        Vec::<std::path::PathBuf>::new()
    );

    let (one, two, gone) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let gone_path = gone.path().to_path_buf();
    assert!(
        remember_at(&index, one.path()).unwrap(),
        "a new root is written"
    );
    assert!(
        !remember_at(&index, one.path()).unwrap(),
        "a known root is not"
    );
    assert!(remember_at(&index, &gone_path).unwrap());
    drop(gone);
    assert!(remember_at(&index, two.path()).unwrap());
    let mut text = std::fs::read_to_string(&index).unwrap();
    text.push_str("relative/path\n\n");
    std::fs::write(&index, text).unwrap();
    assert_eq!(read_index(&index).unwrap(), vec![one.path(), two.path()]);
}
