//! The skeleton a new repository receives agrees with the layer it seeds (I2872, I1102).
//!
//! `share/skeleton` is what `init` lays down; `.ai/` is this repository's own layer, kept
//! honest by every gate. Where the two say the same thing — the local half's declaration, the
//! sections a layer has, the layout a contract declares — they must say it alike, or the next
//! repository starts from a description the tool no longer holds. A section only this
//! repository has is named here with the reason, so it reads as a decision and not as drift.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use majordomus_cli::metadata::yaml::parse_mapping;
use majordomus_cli::repository::Manifest;
use serde_json::Value;

/// Sections this repository's layer has and an adopted layer does not, with why.
const REPOSITORY_ONLY: &[(&str, &str)] = &[(
    "releases",
    "Majordomus's own release history: release/version.rs pins apps/majordomus-cli and the records come from scripts/release-record (the release seat, 2026-10-21)",
)];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest(path: &Path) -> Manifest {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Manifest::parse(path, &text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A Markdown contract's front matter, as JSON.
fn front_matter(path: &Path) -> serde_json::Map<String, Value> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let body = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(fm, _)| fm)
        .unwrap_or_else(|| panic!("{} has no front matter", path.display()));
    parse_mapping(body).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_skeleton_declares_the_local_half_as_the_layer_does() {
    let skel = manifest(&root().join("share/skeleton/ai/manifest.yaml"));
    let own = manifest(&root().join(".ai/manifest.yaml"));
    assert_eq!(
        skel.local, own.local,
        "the skeleton's local: block and this layer's disagree"
    );
    assert_eq!(skel.repo, own.repo, "the tracked half's path");
}

#[test]
fn the_skeleton_has_every_section_but_the_repository_only_ones() {
    let skel: BTreeSet<String> = manifest(&root().join("share/skeleton/ai/manifest.yaml"))
        .sections
        .keys()
        .cloned()
        .collect();
    let own: BTreeSet<String> = manifest(&root().join(".ai/manifest.yaml"))
        .sections
        .keys()
        .cloned()
        .collect();
    let only: BTreeSet<String> = REPOSITORY_ONLY.iter().map(|(s, _)| s.to_string()).collect();
    let missing: Vec<&String> = own
        .difference(&skel)
        .filter(|s| !only.contains(*s))
        .collect();
    assert!(
        missing.is_empty(),
        "sections this layer has that the skeleton does not seed and no reason names: {missing:?}"
    );
    let extra: Vec<&String> = skel.difference(&own).collect();
    assert!(
        extra.is_empty(),
        "the skeleton seeds sections this layer lacks: {extra:?}"
    );
    for (s, why) in REPOSITORY_ONLY {
        assert!(
            own.contains(*s) && !skel.contains(*s),
            "{s} is named repository-only ({why}) but is not: remove or correct the entry"
        );
    }
}

#[test]
fn a_contract_the_skeleton_shares_declares_the_same_layout() {
    let skel = root().join("share/skeleton/ai");
    let own = root().join(".ai");
    let mut compared = 0;
    let mut stack = vec![skel.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .expect("a skeleton directory")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.file_name().is_none_or(|n| n != "README.md") {
                continue;
            }
            let rel = path.strip_prefix(&skel).expect("under the skeleton");
            let mine = own.join(rel);
            if !mine.is_file() {
                continue;
            }
            let (a, b) = (front_matter(&path), front_matter(&mine));
            if a.get("id") != b.get("id") {
                continue;
            }
            assert_eq!(
                a.get("layout"),
                b.get("layout"),
                "{} and {} declare different layouts",
                path.display(),
                mine.display()
            );
            compared += 1;
        }
    }
    assert!(compared > 0, "no contract of the skeleton was compared");
}
