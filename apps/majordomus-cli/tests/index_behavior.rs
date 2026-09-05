//! The index as a whole: determinism, stable identities, provenance, classification.

mod common;

use common::{inspect, rule, Fixture};
use majordomus_cli::discovery::{FileSystem, Sources, VcsIndex};
use majordomus_cli::git::GitState;
use majordomus_cli::{Index, Repository};

fn build(f: &Fixture, filesystem: bool) -> Index {
    let repo = Repository::discover(&f.root()).unwrap();
    let sources = Sources::load(&repo).unwrap();
    let schema = common::dist_schema(&repo);
    let git = GitState::Unavailable {
        reason: "not asked".into(),
    };
    if filesystem {
        let fs = FileSystem {
            excluded: vec![".git".into(), repo.local_path()],
        };
        Index::build(&repo, &sources, &schema, &fs, git).unwrap()
    } else {
        Index::build(&repo, &sources, &schema, &VcsIndex, git).unwrap()
    }
}

fn shape(index: &Index) -> Vec<(String, String, String, String)> {
    index
        .objects
        .iter()
        .map(|o| {
            (
                o.uri.clone(),
                o.kind.clone(),
                o.provenance.path.clone(),
                o.provenance.source_class.clone(),
            )
        })
        .collect()
}

#[test]
fn same_content_same_index_whatever_enumerates_it() {
    let f = Fixture::new();
    for (i, name) in ["zeta", "mid", "alpha2"].iter().enumerate() {
        f.write(
            &format!(".ai/repo/rules/project/{name}.v1.md"),
            &rule(&format!("project.{name}"), 1, &format!("Rule {i}")),
        );
    }
    f.commit("more");
    let vcs = build(&f, false);
    let fs = build(&f, true);
    let again = build(&f, false);
    assert_eq!(shape(&vcs), shape(&again));
    assert_eq!(shape(&vcs), shape(&fs), "the walk and the index disagree");
    assert_eq!(vcs.diagnostics, fs.diagnostics);
    let uris: Vec<&str> = vcs.objects.iter().map(|o| o.uri.as_str()).collect();
    let mut sorted = uris.clone();
    sorted.sort();
    assert_eq!(uris, sorted, "objects are sorted by URI");
    assert!(
        vcs.objects.windows(2).all(|w| w[0].uri != w[1].uri),
        "URIs are unique"
    );
}

#[test]
fn the_index_only_sees_tracked_files_and_the_walk_sees_untracked_too() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/untracked.v1.md",
        &rule("project.untracked", 1, "Untracked"),
    );
    let vcs = build(&f, false);
    let fs = build(&f, true);
    assert!(vcs.get("majordomus://rule/project.untracked@1").is_none());
    assert!(fs.get("majordomus://rule/project.untracked@1").is_some());
    // and neither sees the local half, tracked or not
    f.git(&["add", "-f", ".ai/local/state/current.yaml"]);
    let vcs = build(&f, false);
    assert!(vcs
        .objects
        .iter()
        .all(|o| !o.provenance.path.starts_with(".ai/local")));
}

#[test]
fn provenance_and_classification_are_correct() {
    let f = Fixture::new();
    let index = build(&f, false);
    let rule = index
        .get("majordomus://rule/project.alpha@1")
        .expect("alpha");
    assert_eq!(rule.kind, "rule");
    assert_eq!(rule.identity, "project.alpha@1");
    assert_eq!(rule.title.as_deref(), Some("Alpha"));
    assert_eq!(rule.description.as_deref(), Some("Alpha, in one sentence."));
    assert_eq!(rule.provenance.path, ".ai/repo/rules/project/alpha.v1.md");
    assert_eq!(rule.provenance.directory, ".ai/repo/rules/project");
    assert_eq!(rule.provenance.source_class, "rule");
    assert_eq!(rule.provenance.section.as_deref(), Some("rules"));
    assert_eq!(rule.media_type, "text/markdown");
    assert_eq!(rule.metadata["class"], "advisory");
    assert_eq!(rule.metadata["version"], 1);
    assert_eq!(rule.tags(), vec!["fixture"]);
    assert!(rule.body.starts_with("\n# Rationale"));
    assert!(rule.content.starts_with("---\nid: project.alpha"));

    let readme = index
        .get("majordomus://document/README.md")
        .expect("readme");
    assert_eq!(readme.kind, "document");
    assert_eq!(readme.title.as_deref(), Some("Fixture"));
    assert_eq!(readme.provenance.directory, ".");
    assert_eq!(readme.provenance.source_class, "readme");
    assert_eq!(readme.provenance.section, None);

    let rules_readme = index
        .get("majordomus://context/ai.repo.rules")
        .expect("rules readme");
    assert_eq!(
        rules_readme.provenance.source_class, "rule",
        "discovered by the rule class"
    );
    assert_eq!(rules_readme.kind, "context", "read as the kind it declares");
    assert_eq!(rules_readme.metadata["schema"], "context/v1");
    let a_sh = index
        .get("majordomus://implementation/lib/a.sh")
        .expect("library file");
    assert_eq!(a_sh.media_type, "text/plain");
    assert!(a_sh.content.starts_with("#!/usr/bin/env bash"));
    assert!(a_sh.metadata.as_object().unwrap().is_empty());

    let policy = index
        .get("majordomus://policy/.ai/repo/policy.yaml")
        .expect("policy");
    assert_eq!(policy.media_type, "application/yaml");
    assert_eq!(
        policy.metadata["context"]["always_loaded_budget_lines"],
        150
    );
    assert_eq!(policy.provenance.section.as_deref(), Some("policy"));

    let profile = index
        .get("majordomus://profile/implementation")
        .expect("profile");
    assert_eq!(profile.title.as_deref(), Some("implementation"));
    assert_eq!(
        profile.description.as_deref(),
        Some("build a described feature")
    );
    assert_eq!(index.kinds()["rule"], 1);
}

#[test]
fn git_state_is_carried_and_reported_when_available() {
    let f = Fixture::new();
    let (_, v, _) = inspect(&f.root(), &[]);
    let git = &v["repository"]["repository"]["git"];
    assert_eq!(git["state"], "available");
    assert_eq!(git["branch"].as_str().map(|b| b.is_empty()), Some(false));
    assert_eq!(git["working_tree"], "clean");
    assert_eq!(git["head"].as_str().map(str::len), Some(40));
}

#[test]
fn without_git_vcs_discovery_is_refused_and_the_walk_still_works() {
    let f = Fixture::plain_dir();
    f.write(".ai/manifest.yaml", common::MANIFEST);
    f.write(".ai/repo/policy.yaml", common::POLICY);
    f.write(".ai/repo/profiles/implementation.yaml", common::PROFILE);
    f.write(
        ".ai/repo/rules/project/alpha.v1.md",
        &rule("project.alpha", 1, "Alpha"),
    );
    f.write(".ai/repo/knowledge/sources.yaml", common::SOURCES);
    let (code, _, err) = common::run_in(&f.root(), &["mcp", "--inspect"], "");
    assert_eq!(code, 13, "{err}");
    assert!(err.contains("--discovery filesystem"), "{err}");
    let (code, v, err) = inspect(&f.root(), &["--discovery", "filesystem"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(v["repository"]["repository"]["git"]["state"], "unavailable");
    assert_eq!(v["repository"]["repository"]["discovery"], "filesystem");
    assert!(common::resource_uris(&v).contains(&"majordomus://rule/project.alpha@1".to_string()));
}
