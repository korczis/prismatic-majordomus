//! The repository is discovered by the model's owners and nobody else (I3017, the last slice
//! of I1105). A process finds its repository once (I3014), the entry points hand what they
//! found on (I3015) and a capability asks its context (I3016); this test keeps it so. It
//! reads the crate's sources and refuses a `Repository::discover(` or `Repository::open(`
//! outside the files named below, or more of them in a file than it is allowed, naming the
//! file and the line. A module's own tests, below its `#[cfg(test)] mod`, and comments are
//! not counted: a fixture opens the repository it built.
//!
//! A new owner is added here with the reason it may discover; that line is the review.

use std::path::{Path, PathBuf};

/// The files that may discover or open the repository, how many times, and why.
const OWNERS: &[(&str, usize, &str)] = &[
    ("src/app.rs", 1, "App::load: the one discovery of a command"),
    (
        "src/commands/serve.rs",
        1,
        "serve elects a server before the layer loads",
    ),
    (
        "src/commands/mcp.rs",
        1,
        "mcp elects a server before the layer loads",
    ),
    (
        "src/commands/env.rs",
        1,
        "the environment entry runs on every directory change",
    ),
    (
        "src/commands/web.rs",
        1,
        "web serves the site without loading the layer",
    ),
    (
        "src/commands/guard.rs",
        1,
        "the edit-guard hook finds the repository of the file edited",
    ),
    (
        "src/command_graph/load.rs",
        1,
        "completion reads the command graph without the layer",
    ),
    (
        "src/worktree/trunk.rs",
        1,
        "the trunk thresholds a checkout given by its path declares",
    ),
    (
        "src/worktree/uncommitted.rs",
        1,
        "the threshold a checkout given by its path declares",
    ),
    (
        "src/capability/handler.rs",
        1,
        "Context::repository, the accessor a capability asks",
    ),
    (
        "src/capability/builtin/server.rs",
        1,
        "local_half of a checkout given by its path",
    ),
    (
        "src/capability/builtin/brief.rs",
        1,
        "the brief interval of a checkout given by its path",
    ),
    (
        "src/guard.rs",
        1,
        "the guard's policy, read for a hook process",
    ),
    (
        "src/guard/remote_build.rs",
        1,
        "the Bash guard finds the checkout a hook payload's cwd lies in",
    ),
    (
        "src/integration/allocate.rs",
        1,
        "allocation for a checkout given by its path",
    ),
    (
        "src/intent_binding.rs",
        1,
        "start_refusal holds an index, not a context",
    ),
    (
        "src/synthetic.rs",
        1,
        "the synthetic repository a test builds",
    ),
];

/// Every `.rs` file under `dir`, in a stable order.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("a directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

/// The lines of `text` that discover or open the repository, before its test module and
/// outside comments, as 1-based line numbers.
fn discoveries(text: &str) -> Vec<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if line.trim() == "#[cfg(test)]"
            && lines
                .get(i + 1)
                .is_some_and(|next| next.trim_start().starts_with("mod "))
        {
            break;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        if line.contains("Repository::discover(") || line.contains("Repository::open(") {
            found.push(i + 1);
        }
    }
    found
}

#[test]
fn the_repository_is_discovered_by_its_owners_alone() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&crate_dir.join("src"), &mut files);
    let mut refused = Vec::new();
    for path in &files {
        let rel = path
            .strip_prefix(crate_dir)
            .expect("under the crate")
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "src/repository.rs" {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("a source file");
        let lines = discoveries(&text);
        let allowed = OWNERS
            .iter()
            .find(|(file, _, _)| *file == rel)
            .map_or(0, |(_, n, _)| *n);
        if lines.len() > allowed {
            refused.push(format!(
                "{rel}:{} discovers the repository {} time(s), {allowed} allowed: ask the context (Context::repository) or the caller for it, or name the file in OWNERS with its reason",
                lines[allowed],
                lines.len()
            ));
        }
    }
    assert!(refused.is_empty(), "\n{}", refused.join("\n"));
}

/// Every owner still discovers: a name left behind after its call went away would let the
/// next one back in unseen.
#[test]
fn every_owner_named_still_discovers() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (file, n, why) in OWNERS {
        let text = std::fs::read_to_string(crate_dir.join(file))
            .unwrap_or_else(|e| panic!("{file} ({why}): {e}"));
        assert_eq!(
            discoveries(&text).len(),
            *n,
            "{file} is allowed {n} discovery ({why}); lower or remove its entry"
        );
    }
}

/// The scanner itself: a call is counted, a comment and a test module are not.
#[test]
fn the_scan_counts_calls_not_comments_or_tests() {
    let text = "fn a() { Repository::open(p); }\n// Repository::discover(p)\n#[cfg(test)]\nmod tests {\n    fn b() { Repository::discover(p); }\n}\n";
    assert_eq!(discoveries(text), vec![1]);
}
