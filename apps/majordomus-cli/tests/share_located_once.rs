//! The distribution a process was pointed at is the one its handlers read.
//!
//! A process locates the share directory once, at start-up, honouring `--share`, then
//! `MAJORDOMUS_SHARE`, then the repository's own `share/`, then the one beside the
//! executable. Two handlers used to locate a second one for themselves, and a second
//! locator never sees `--share`: pointed at one distribution, a process answered from
//! another, and in a repository carrying no `share/` of its own it failed outright —
//! `every_cached_capability_answers_the_same_from_the_handler_the_cold_cache_and_the_warm_cache`
//! failed on exactly that in CI, where no `MAJORDOMUS_SHARE` is set, for as long as the
//! job before it was red for another reason.
//!
//! The fixture here carries a decoy: a `share/` of its own, holding the file the second
//! locator looks for and an obligation vocabulary that the real distribution does not have.
//! The app is pointed at the real one. Whatever the handlers answer names which of the two
//! they read.

mod common;

use common::{dist_share, Fixture};
use serde_json::json;

/// A share directory a second locator would find first: `kinds.yaml` is what
/// [`majordomus_cli::share::Share::locate`] looks for, and the vocabulary beside it names
/// a token the real distribution does not ship.
fn decoy_share(f: &Fixture) {
    f.write("share/kinds.yaml", "version: 1\nkinds: {}\n");
    f.write(
        "share/obligations.yaml",
        "version: 1
obligations:
  - id: decoy
    title: The decoy was read
    summary: This vocabulary is not the one the process was pointed at.
    discharged_by: none
    remote: false
",
    );
}

#[test]
fn the_vocabulary_comes_from_the_distribution_the_process_was_pointed_at() {
    let f = Fixture::new();
    decoy_share(&f);
    // `common::load_app` passes `--share <the crate's own distribution>`
    let app = common::load_app(&f);
    assert_eq!(
        app.context.index.share_dir.as_deref(),
        Some(dist_share().as_path()),
        "the index carries the share this process located"
    );

    let answer = app
        .context
        .execute("obligations.vocabulary", json!({}))
        .expect("the vocabulary is answered");
    let source = answer["source"].as_str().expect("where it was read from");
    assert!(
        source.starts_with(dist_share().to_str().unwrap()),
        "read from the distribution the process was pointed at, not {source}"
    );
    let ids: Vec<&str> = answer["obligations"]
        .as_array()
        .expect("the tokens")
        .iter()
        .map(|o| o["id"].as_str().unwrap())
        .collect();
    assert!(
        !ids.contains(&"decoy"),
        "the decoy beside the repository was read instead: {ids:?}"
    );
    assert!(
        ids.contains(&"tests"),
        "the distribution's own tokens are there: {ids:?}"
    );

    // and the closure resolves against the same vocabulary rather than reporting that it
    // could not be read
    let closure = app
        .context
        .execute("obligations.closure", json!({}))
        .expect("the closure is answered");
    let findings = closure["findings"].to_string();
    assert!(
        !findings.contains("vocabulary could not be read"),
        "the closure read the vocabulary: {findings}"
    );
}

#[test]
fn the_command_registry_comes_from_the_same_distribution() {
    let f = Fixture::new();
    // the decoy has no command registry at all, so a handler that read it would list
    // none of the shell tool's commands
    decoy_share(&f);
    let app = common::load_app(&f);
    let answer = app
        .context
        .execute("commands.list", json!({}))
        .expect("the commands are answered");
    let names: Vec<String> = answer["commands"]
        .as_array()
        .expect("the commands")
        .iter()
        .filter_map(|c| c["id"].as_str().map(str::to_string))
        .collect();
    assert!(
        names.iter().any(|n| n == "tool.start"),
        "the shell tool's registry was read from the distribution: {names:?}"
    );
}
