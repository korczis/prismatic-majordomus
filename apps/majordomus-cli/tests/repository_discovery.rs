//! Root selection: the nearest ancestor carrying .ai/manifest.yaml, and nothing else.

mod common;

use common::{inspect, run_in, Fixture, MANIFEST};
use majordomus_cli::{Error, Repository};

#[test]
fn finds_the_root_from_the_root_and_from_a_nested_directory() {
    let f = Fixture::new();
    let root = f.root();
    assert_eq!(Repository::discover(&root).unwrap().root(), root);
    let nested = f.path("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(Repository::discover(&nested).unwrap().root(), root);
    // and through the executable
    let (code, v, _) = inspect(&nested, &[]);
    assert_eq!(code, 0);
    assert_eq!(
        v["repository"]["repository"]["root"],
        root.to_str().unwrap()
    );
}

#[test]
fn an_ordinary_git_repository_is_not_a_majordomus_repository() {
    let f = Fixture::empty_git();
    f.write("README.md", "# Just a repo\n");
    f.commit("x");
    match Repository::discover(&f.root()) {
        Err(Error::RepositoryNotFound { .. }) => {}
        other => panic!("{other:?}"),
    }
    let (code, out, err) = run_in(&f.root(), &["mcp", "--inspect"], "");
    assert_eq!(code, 12, "{err}");
    assert!(out.is_empty());
    assert!(err.contains("no Majordomus repository found"), "{err}");
}

#[test]
fn a_directory_with_no_markers_is_not_found() {
    let f = Fixture::plain_dir();
    assert!(matches!(
        Repository::discover(&f.root()),
        Err(Error::RepositoryNotFound { .. })
    ));
}

#[test]
fn the_nearest_of_two_nested_candidates_wins() {
    let outer = Fixture::new();
    let inner = outer.path("sub/inner");
    std::fs::create_dir_all(inner.join(".ai")).unwrap();
    std::fs::write(inner.join(".ai/manifest.yaml"), MANIFEST).unwrap();
    let deep = inner.join("x/y");
    std::fs::create_dir_all(&deep).unwrap();
    assert_eq!(
        Repository::discover(&deep).unwrap().root(),
        inner.canonicalize().unwrap()
    );
    assert_eq!(
        Repository::discover(&outer.path("sub")).unwrap().root(),
        outer.root()
    );
}

#[test]
fn a_nearer_broken_manifest_is_an_error_not_a_skip() {
    let outer = Fixture::new();
    let inner = outer.path("sub/inner");
    std::fs::create_dir_all(inner.join(".ai")).unwrap();
    std::fs::write(
        inner.join(".ai/manifest.yaml"),
        "schema: ai-repository/v9\n",
    )
    .unwrap();
    match Repository::discover(&inner) {
        Err(Error::UnsupportedSchema { found, .. }) => assert_eq!(found, "ai-repository/v9"),
        other => panic!("{other:?}"),
    }
    std::fs::write(
        inner.join(".ai/manifest.yaml"),
        "schema: ai-repository/v1\nrepo:\n  path: repo\nbogus: 1\n",
    )
    .unwrap();
    match Repository::discover(&inner) {
        Err(Error::UnknownKeys { keys, .. }) => assert_eq!(keys, vec!["bogus"]),
        other => panic!("{other:?}"),
    }
    let (code, _, err) = run_in(&inner, &["mcp", "--inspect"], "");
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("unknown key(s): bogus"), "{err}");
}

#[test]
fn the_pre_ai_layout_is_refused_by_name() {
    let f = Fixture::empty_git();
    f.write(".majordomus/policy.yaml", "version: 1\n");
    match Repository::discover(&f.root()) {
        Err(Error::LegacyLayout { .. }) => {}
        other => panic!("{other:?}"),
    }
    let (code, _, err) = run_in(&f.root(), &["mcp", "--inspect"], "");
    assert_eq!(code, 12);
    assert!(err.contains("majordomus migrate"), "{err}");
}

#[test]
fn a_tool_installation_under_dot_majordomus_is_not_a_layout() {
    let f = Fixture::new();
    f.write(".majordomus/bin/majordomus", "#!/bin/sh\n");
    assert_eq!(Repository::discover(&f.root()).unwrap().root(), f.root());
}

#[test]
fn sections_resolve_from_the_manifest() {
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).unwrap();
    assert_eq!(
        repo.section_path("rules").as_deref(),
        Some(".ai/repo/rules")
    );
    assert_eq!(
        repo.section_of(".ai/repo/rules/project/alpha.v1.md"),
        Some("rules")
    );
    assert_eq!(repo.section_of(".ai/repo/policy.yaml"), Some("policy"));
    assert_eq!(repo.section_of("README.md"), None);
    assert_eq!(repo.local_path(), ".ai/local");
}
