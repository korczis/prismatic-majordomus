//! `/ui/compute` (ADR 0170, I2788) over a real socket, and the committed module it serves.
//!
//! The page computes the canonical digest in a same-origin Worker with the committed wasm32
//! build of the portable core, and asks `compute.verify` to recompute it. These tests hold
//! what that promise rests on:
//! - the page keeps the Cockpit's policy, and only the Worker's response may compile
//!   WebAssembly;
//! - the module the page names is served as `application/wasm`, byte for byte;
//! - the committed module names the digest of the core sources it was built from, that digest
//!   is the sources' current one, and the module carries no path of the machine that built it.
//!
//! The module's answers are judged by the shared fixture, executed: the `portable-core` gate
//! runs `apps/majordomus-core/tests/wasm-parity.mjs` against the committed module.
//!
//! The module is rebuilt, with its provenance, by the one ignored test below:
//! `cargo test --test ui_compute -- --ignored --exact the_core_module_is_rebuilt_from_its_sources`.
//! Two hosts build different bytes from the same sources (the code is laid out differently),
//! so what is held is the sources a module names and the answers it gives, not its bytes.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Fixture, Served};
use serde_json::Value;
use sha2::{Digest, Sha256};

const MODULE: &str = "share/ui/compute/majordomus_core.wasm";
const PROVENANCE: &str = "share/ui/compute/majordomus_core.json";
const REBUILD: &str =
    "cargo test --test ui_compute -- --ignored --exact the_core_module_is_rebuilt_from_its_sources";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The files a module is built from: the toolchain the repository pins, and the core's
/// manifest, lock and sources, by repository-relative path.
fn sources(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .expect("read a source directory")
            .map(|e| e.expect("an entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let relative = path.strip_prefix(root).expect("under the root");
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut files = vec![
        "rust-toolchain.toml".to_string(),
        "apps/majordomus-core/Cargo.toml".to_string(),
        "apps/majordomus-core/Cargo.lock".to_string(),
    ];
    walk(&root.join("apps/majordomus-core/src"), root, &mut files);
    files.sort();
    files
}

/// One digest over every source: each file's path and the digest of its bytes, in path order.
fn sources_digest(root: &Path) -> String {
    let mut listing = String::new();
    for file in sources(root) {
        let bytes = std::fs::read(root.join(&file)).expect("read a source");
        listing.push_str(&format!("{file}\n{}\n", sha256(&bytes)));
    }
    sha256(listing.as_bytes())
}

fn provenance(root: &Path) -> Value {
    let text = std::fs::read_to_string(root.join(PROVENANCE)).expect("the module's provenance");
    serde_json::from_str(&text).expect("the provenance is JSON")
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// A GET whose body is bytes: the shared helper reads a body as text.
fn get_bytes(address: &str, target: &str) -> (u16, String, Vec<u8>) {
    let mut stream = TcpStream::connect(address).expect("connect");
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .expect("write the request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read the response");
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a header/body split");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("a status");
    (status, head, raw[split + 4..].to_vec())
}

/// The value of `name="..."` the first time it appears in `html`, unescaped.
fn attribute(html: &str, name: &str) -> String {
    let start = html
        .find(&format!("{name}=\""))
        .unwrap_or_else(|| panic!("the page names {name}"))
        + name.len()
        + 2;
    let end = start + html[start..].find('"').expect("the attribute closes");
    html[start..end].replace("&amp;", "&")
}

#[test]
fn the_committed_module_is_built_from_the_current_core_sources() {
    let root = repo();
    let record = provenance(&root);
    let module = std::fs::read(root.join(MODULE)).expect("the committed module");
    assert_eq!(
        record["module_sha256"].as_str(),
        Some(sha256(&module).as_str()),
        "{PROVENANCE} names another module than {MODULE}; rebuild both with: {REBUILD}"
    );
    assert_eq!(
        record["sources_sha256"].as_str(),
        Some(sources_digest(&root).as_str()),
        "the core's sources moved since {MODULE} was built from them; rebuild it with: {REBUILD}"
    );
    assert_eq!(record["target"], "wasm32-unknown-unknown");
}

#[test]
fn the_committed_module_names_no_path_of_the_machine_that_built_it() {
    let module = std::fs::read(repo().join(MODULE)).expect("the committed module");
    assert_eq!(&module[..4], b"\0asm", "a WebAssembly module");
    for export in [&b"mj_alloc"[..], b"mj_canonical_digest", b"mj_free"] {
        assert!(
            module.windows(export.len()).any(|w| w == export),
            "the module exports {}",
            String::from_utf8_lossy(export)
        );
    }
    for host in [
        &b"/Users/"[..],
        b"/home/",
        b"/private/",
        b"/tmp/",
        b"/root/",
        b":\\",
    ] {
        assert!(
            !module.windows(host.len()).any(|w| w == host),
            "the module carries a path of the machine that built it ({}); rebuild it with: {REBUILD}",
            String::from_utf8_lossy(host)
        );
    }
}

#[test]
fn the_page_keeps_the_cockpits_policy_and_only_its_worker_may_compile_webassembly() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, headers, page) = s.request("GET", "/ui/compute", None);
    assert_eq!(status, 200, "{page}");
    let (_, cockpit, _) = s.request("GET", "/cockpit", None);
    let policy = header(&headers, "content-security-policy").expect("the page's policy");
    assert_eq!(
        Some(policy),
        header(&cockpit, "content-security-policy"),
        "the page carries the Cockpit's policy"
    );
    assert!(!policy.contains("wasm-unsafe-eval"), "{policy}");
    assert!(
        page.contains("data-keeps-its-place"),
        "nothing reads the page again under the reader"
    );

    // the Worker: served with the one policy that allows compiling the module
    let worker = attribute(&page, "data-compute-worker");
    assert!(
        worker.starts_with("/ui/assets/compute/worker.js?v="),
        "{worker}"
    );
    let (status, headers, script) = s.request("GET", &worker, None);
    assert_eq!(status, 200, "{script}");
    let worker_policy = header(&headers, "content-security-policy").expect("the Worker's policy");
    assert!(
        worker_policy.contains("script-src 'self' 'wasm-unsafe-eval'"),
        "{worker_policy}"
    );
    assert!(
        worker_policy.contains("connect-src 'self'"),
        "{worker_policy}"
    );
    assert!(
        script.contains("mj_canonical_digest"),
        "the Worker calls the module's ABI"
    );

    // the page's script, under the page's policy
    let start = page
        .find("/ui/assets/compute/page.js?v=")
        .expect("the page loads its script");
    let page_script = &page[start..start + page[start..].find('"').expect("the URL closes")];
    let (status, headers, _) = s.request("GET", page_script, None);
    assert_eq!(status, 200, "{page_script}");
    assert_eq!(
        header(&headers, "content-security-policy"),
        None,
        "only the Worker gets one"
    );

    // the verifier the page asks is compute.verify's own route
    assert_eq!(
        attribute(&page, "data-compute-verify"),
        "/api/v1/compute/verify"
    );
}

#[test]
fn the_module_the_page_names_is_the_committed_one_byte_for_byte() {
    let root = repo();
    let committed = std::fs::read(root.join(MODULE)).expect("the committed module");
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (_, _, page) = s.request("GET", "/ui/compute", None);

    let module = attribute(&page, "data-compute-module");
    assert!(
        module.ends_with(&format!("?v={}", &sha256(&committed)[..16])),
        "the URL names the bytes: {module}"
    );
    let (status, head, body) = get_bytes(&s.address, &module);
    assert_eq!(status, 200, "{head}");
    assert!(
        head.to_ascii_lowercase()
            .contains("content-type: application/wasm"),
        "{head}"
    );
    assert_eq!(body.len(), committed.len());
    assert!(body == committed, "served byte for byte");

    // the page says which sources that module was built from
    let record = provenance(&root);
    for key in ["module_sha256", "sources_sha256"] {
        let value = record[key].as_str().expect("a digest");
        assert!(page.contains(value), "the page shows the module's {key}");
    }
}

/// The producer of the committed module and its provenance. It builds the core for wasm32
/// with the repository's toolchain, remapping the registry and the standard library to
/// fixed prefixes so that no path of this machine is written into the module, and records
/// the digest of the sources it built from. Ignored: it writes into `share/`.
#[test]
#[ignore = "writes the committed module; run it after changing the core"]
fn the_core_module_is_rebuilt_from_its_sources() {
    let root = repo();
    let rustc = |args: &[&str]| {
        let out = Command::new("rustc")
            .args(args)
            .current_dir(&root)
            .output()
            .expect("run rustc");
        assert!(out.status.success(), "rustc {args:?} failed");
        String::from_utf8(out.stdout).expect("rustc answers UTF-8")
    };
    let verbose = rustc(&["-vV"]);
    let field = |name: &str| {
        verbose
            .lines()
            .find_map(|l| l.strip_prefix(&format!("{name}: ")))
            .unwrap_or_else(|| panic!("rustc -vV names {name}"))
            .to_string()
    };
    let (commit, host) = (field("commit-hash"), field("host"));
    let toolchain = verbose.lines().next().expect("rustc's version").to_string();
    let sysroot = rustc(&["--print", "sysroot"]).trim().to_string();

    let manifest = root.join("apps/majordomus-core/Cargo.toml");
    let metadata = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(&manifest)
        .current_dir(&root)
        .output()
        .expect("run cargo metadata");
    assert!(metadata.status.success(), "cargo metadata failed");
    let metadata: Value = serde_json::from_slice(&metadata.stdout).expect("metadata is JSON");
    let registry = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter_map(|p| p["manifest_path"].as_str())
        .find_map(|path| {
            path.find("/registry/src/")
                .map(|i| path[..i + "/registry/src".len()].to_string())
        })
        .expect("a dependency from the registry");

    let target_dir = std::env::temp_dir().join("majordomus-core-module");
    let flags = format!(
        "--remap-path-prefix={registry}=/registry \
         --remap-path-prefix={sysroot}/lib/rustlib/src/rust=/rustc/{commit}"
    );
    let built = Command::new("cargo")
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        .current_dir(&root)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_RUSTFLAGS")
        .env("RUSTFLAGS", flags)
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_INCREMENTAL", "0")
        .status()
        .expect("run cargo build");
    assert!(built.success(), "the wasm32 build failed");

    let module =
        std::fs::read(target_dir.join("wasm32-unknown-unknown/release/majordomus_core.wasm"))
            .expect("the built module");
    std::fs::write(root.join(MODULE), &module).expect("write the module");
    let record = serde_json::json!({
        "module": "majordomus_core.wasm",
        "module_sha256": sha256(&module),
        "sources_sha256": sources_digest(&root),
        "sources": sources(&root),
        "target": "wasm32-unknown-unknown",
        "toolchain": toolchain,
        "host": host,
        "rebuild": REBUILD,
    });
    let text = serde_json::to_string_pretty(&record).expect("the record serialises") + "\n";
    std::fs::write(root.join(PROVENANCE), text).expect("write the provenance");
}

/// Every attribute value of `name` on the page, in order, unescaped.
fn attributes(html: &str, name: &str) -> Vec<String> {
    let needle = format!(" {name}=\"");
    html.match_indices(&needle)
        .map(|(at, _)| {
            let start = at + needle.len();
            let end = start + html[start..].find('"').expect("the attribute closes");
            html[start..end]
                .replace("&quot;", "\"")
                .replace("&#39;", "'")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
        })
        .collect()
}

#[test]
fn the_page_carries_every_row_of_the_shared_fixture_as_the_fixture_holds_it() {
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            repo().join("apps/majordomus-core/fixtures/canonical-digest.v1.json"),
        )
        .expect("the shared fixture"),
    )
    .expect("the fixture is JSON");
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (status, _, page) = s.request("GET", "/ui/compute", None);
    assert_eq!(status, 200, "{page}");
    assert!(
        page.contains("data-compute-fixture"),
        "the page offers no fixture run"
    );
    let button = page
        .split("<button")
        .find(|tag| tag.contains("data-fixture-run"))
        .and_then(|tag| tag.split('>').next())
        .expect("the run button");
    assert!(
        button.contains("disabled"),
        "nothing runs without the page's script: {button}"
    );

    // each row's own tag: an empty attribute is written bare, so it is read as empty
    let got: Vec<(String, String, String, String)> = page
        .split("<tr")
        .filter(|tag| tag.contains("data-fixture-row="))
        .map(|tag| {
            let tag = format!(" {}", tag.split('>').next().expect("the tag closes"));
            let one = |name: &str| {
                attributes(&tag, name)
                    .into_iter()
                    .next()
                    .unwrap_or_default()
            };
            (
                one("data-fixture-row"),
                one("data-fixture-input"),
                one("data-fixture-digest"),
                one("data-fixture-kind"),
            )
        })
        .collect();
    let mut want = Vec::new();
    for (key, kind) in [("cases", "case"), ("invalid", "invalid")] {
        for row in fixture[key].as_array().expect("rows") {
            want.push((
                row["id"].as_str().expect("an id").to_string(),
                row["input"].as_str().expect("an input").to_string(),
                row["digest"].as_str().unwrap_or("").to_string(),
                kind.to_string(),
            ));
        }
    }
    assert_eq!(
        got, want,
        "the page's rows are not the fixture's, byte for byte"
    );
}
