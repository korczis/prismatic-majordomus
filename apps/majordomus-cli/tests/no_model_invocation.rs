//! Majordomus never invokes a model, and this is the check that refuses a change that would.
//!
//! The product's claim is that it supervises the work a provider does and never becomes a
//! provider: no model client, no HTTP call to a model host, no spawned provider CLI. The claim
//! was verified by reading (docs/RUNTIME_CONTINUITY.md) and held by nothing. A boundary rule
//! with no registered, blocking check is the reference implementation's own counter-example —
//! its enforcement module existed and was never wired — so this one is a test of the crate
//! and runs wherever the crate's tests run, with no new job and no new shell (I1700).
//!
//! Three measurements, each over the whole of what it is about:
//!
//! * `Cargo.lock` resolves no HTTP client crate. The lock and not `Cargo.toml`, because a
//!   client arriving as a transitive dependency is still linked into the executable.
//! * No source file of the crate spawns a provider's CLI by name.
//! * No source file of the crate names a model host. A catalogue of models is declared data
//!   and lives in `share/models.yaml`, outside the tree this reads; a host named in Rust is a
//!   host the executable can reach.
//!
//! Each scanner is a function over text, so the failure paths are proven here on planted
//! input as well as on the tree.

use std::path::{Path, PathBuf};

/// Crates whose purpose is to be an HTTP client. `hyper-util` is here because its `client`
/// module is the legacy client: if it is ever needed for a server, prove its `client` feature
/// is off and narrow this entry, rather than delete it.
const HTTP_CLIENT_CRATES: &[&str] = &[
    "reqwest",
    "ureq",
    "hyper-util",
    "isahc",
    "attohttpc",
    "surf",
    "curl",
    "curl-sys",
    "awc",
];

/// The command-line programs of the providers Majordomus supervises.
const PROVIDER_CLIS: &[&str] = &["claude", "codex", "gemini", "ollama"];

/// Where models are served. A local runtime's default port counts: a model on this machine is
/// still a model invoked.
const MODEL_HOSTS: &[&str] = &[
    "api.anthropic.com",
    "api.openai.com",
    "generativelanguage.googleapis.com",
    "bedrock-runtime",
    "openrouter.ai",
    "localhost:11434",
    "127.0.0.1:11434",
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every package name the lock resolves that is an HTTP client.
fn http_clients_in_lock(lock: &str) -> Vec<String> {
    lock.lines()
        .filter_map(|l| l.trim().strip_prefix("name = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .filter(|name| HTTP_CLIENT_CRATES.contains(name))
        .map(str::to_string)
        .collect()
}

/// Every spawn of a provider CLI in one source text, as `<cli>` per occurrence. A spawn is a
/// `Command::new` whose program is the provider's name as a string literal, with or without a
/// path in front of it (`"/usr/local/bin/claude"`).
fn provider_spawns(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find("Command::new(") {
        let after = rest[at + "Command::new(".len()..].trim_start();
        if let Some(literal) = after.strip_prefix('"') {
            if let Some(end) = literal.find('"') {
                let program = &literal[..end];
                let base = program.rsplit('/').next().unwrap_or(program);
                if PROVIDER_CLIS.contains(&base) {
                    found.push(base.to_string());
                }
            }
        }
        rest = &rest[at + 1..];
    }
    found
}

/// Every model host one source text names.
fn model_hosts(source: &str) -> Vec<String> {
    MODEL_HOSTS
        .iter()
        .filter(|h| source.contains(*h))
        .map(|h| h.to_string())
        .collect()
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_lock_resolves_no_http_client() {
    let lock = std::fs::read_to_string(crate_dir().join("Cargo.lock")).expect("Cargo.lock");
    // A lock this reader cannot parse would report no client, which is not a pass.
    assert!(
        lock.lines().filter(|l| l.starts_with("name = \"")).count() > 10,
        "Cargo.lock holds no package this reader recognises; the scan measured nothing"
    );
    let found = http_clients_in_lock(&lock);
    assert!(
        found.is_empty(),
        "Cargo.lock resolves HTTP client crate(s) {found:?}. Majordomus never invokes a model, \
         and a client in the executable is the means to; remove the dependency that brings it \
         (cargo tree -i <crate>)"
    );
}

#[test]
fn no_source_spawns_a_provider_cli_or_names_a_model_host() {
    let mut files = Vec::new();
    rust_sources(&crate_dir().join("src"), &mut files);
    assert!(
        files.len() > 50,
        "the crate's src/ held {} Rust files; the scan measured nothing",
        files.len()
    );
    let mut findings = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).expect("a source file");
        let rel = f
            .strip_prefix(crate_dir())
            .unwrap_or(f)
            .display()
            .to_string();
        for cli in provider_spawns(&text) {
            findings.push(format!("{rel}: spawns the provider CLI `{cli}`"));
        }
        for host in model_hosts(&text) {
            findings.push(format!("{rel}: names the model host `{host}`"));
        }
    }
    assert!(
        findings.is_empty(),
        "Majordomus never invokes a model; these would:\n  {}\nA model catalogue is declared \
         data (share/models.yaml); a host or a provider spawn in Rust is a call.",
        findings.join("\n  ")
    );
}

#[test]
fn each_scanner_finds_what_it_is_for() {
    let lock = "[[package]]\nname = \"serde\"\n\n[[package]]\nname = \"reqwest\"\nversion = \"0.12\"\n\n[[package]]\nname = \"hyper-util\"\n";
    assert_eq!(http_clients_in_lock(lock), ["reqwest", "hyper-util"]);
    // a name that merely contains a client's name is not that client
    assert!(http_clients_in_lock("name = \"curl-parser-lite\"\nname = \"surfboard\"\n").is_empty());

    assert_eq!(
        provider_spawns(r#"let c = std::process::Command::new( "claude" ).arg("-p");"#),
        ["claude"]
    );
    assert_eq!(
        provider_spawns(r#"Command::new("/opt/homebrew/bin/ollama")"#),
        ["ollama"]
    );
    // git is not a provider, and a provider's name that is not a program is not a spawn
    assert!(provider_spawns(r#"Command::new("git"); let p = "claude";"#).is_empty());

    assert_eq!(
        model_hosts(r#"const URL: &str = "https://api.openai.com/v1/chat";"#),
        ["api.openai.com"]
    );
    assert_eq!(
        model_hosts("http://localhost:11434/api/generate"),
        ["localhost:11434"]
    );
    assert!(model_hosts("https://majordomus.dev").is_empty());
}
