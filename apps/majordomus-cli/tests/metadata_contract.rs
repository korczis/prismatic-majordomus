//! The metadata contract, through the executable's own index: what a file must carry to
//! become an object, and the diagnostic each failure produces.

mod common;

use common::{diagnostics, inspect, resource_uris, rule, Fixture};

fn diagnostic_codes(v: &serde_json::Value) -> Vec<(String, Option<String>)> {
    diagnostics(v)
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_string(),
                d["path"].as_str().map(str::to_string),
            )
        })
        .collect()
}

#[test]
fn a_valid_fixture_has_no_diagnostics_and_every_kind_present() {
    let f = Fixture::new();
    let (code, v, err) = inspect(&f.root(), &[]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(v["repository"]["state"], "ok");
    assert!(diagnostics(&v).is_empty(), "{:?}", diagnostics(&v));
    let uris = resource_uris(&v);
    for expected in [
        "majordomus://repository",
        "majordomus://rule/project.alpha@1",
        "majordomus://prompt/continue",
        "majordomus://profile/implementation",
        "majordomus://policy/.ai/repo/policy.yaml",
        "majordomus://document/README.md",
        "majordomus://document/docs/CLI.md",
        "majordomus://context/ai.repo.rules",
        "majordomus://context/ai.repo.workflows",
        "majordomus://document/.ai/repo/workflows/task-lifecycle.md",
        "majordomus://claim/policy-parse",
        "majordomus://claim/routing",
        "majordomus://document/docs/claims/policy-parse.md",
        "majordomus://implementation/lib/a.sh",
        "majordomus://test/test/cases/00_x.sh",
    ] {
        assert!(
            uris.contains(&expected.to_string()),
            "missing {expected} in {uris:?}"
        );
    }
    assert!(
        !uris.iter().any(|u| u.contains(".ai/local")),
        "local state leaked: {uris:?}"
    );
}

#[test]
fn a_rule_without_front_matter_is_missing_front_matter_unless_it_is_a_document() {
    let f = Fixture::new();
    // rules/README.md carries no front matter and is read as a document, by the schema's
    // without_front_matter rule; a rule-looking file that is not a README gets the same
    // treatment, because the rule is about front matter, not file names.
    f.write(
        ".ai/repo/rules/project/notes.md",
        "# Notes\n\nNot a rule.\n",
    );
    f.commit("notes");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 0);
    assert!(resource_uris(&v)
        .contains(&"majordomus://document/.ai/repo/rules/project/notes.md".to_string()));
}

#[test]
fn malformed_front_matter_is_named_and_excluded() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/broken.v1.md",
        "---\nid: project.broken\nversion: 1\n# never closed\n",
    );
    f.write(
        ".ai/repo/rules/project/tabbed.v1.md",
        "---\nid:\tproject.tabbed\n---\n",
    );
    f.commit("broken");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 10, "an error diagnostic makes inspect exit 10");
    assert_eq!(v["repository"]["state"], "degraded");
    let codes = diagnostic_codes(&v);
    assert!(
        codes.contains(&(
            "malformed_front_matter".into(),
            Some(".ai/repo/rules/project/broken.v1.md".into())
        )),
        "{codes:?}"
    );
    assert!(
        codes.contains(&(
            "malformed_front_matter".into(),
            Some(".ai/repo/rules/project/tabbed.v1.md".into())
        )),
        "{codes:?}"
    );
    let uris = resource_uris(&v);
    assert!(!uris.iter().any(|u| u.contains("broken")), "{uris:?}");
    assert!(
        uris.contains(&"majordomus://rule/project.alpha@1".to_string()),
        "the valid rule still serves"
    );
}

#[test]
fn unknown_keys_are_named_per_file() {
    let f = Fixture::new();
    let text = rule("project.beta", 1, "Beta").replace(
        "tags: [fixture]",
        "tags: [fixture]\nowner: me\nx-majordomus:\n  colour: red",
    );
    f.write(".ai/repo/rules/project/beta.v1.md", &text);
    f.commit("beta");
    let (_, v, _) = inspect(&f.root(), &[]);
    let d = diagnostics(&v);
    let hit = d
        .iter()
        .find(|d| d["code"] == "unknown_key")
        .expect("unknown_key diagnostic");
    assert_eq!(hit["path"], ".ai/repo/rules/project/beta.v1.md");
    let msg = hit["message"].as_str().unwrap();
    assert!(
        msg.contains("owner")
            && msg.contains("x-majordomus.colour")
            && msg.contains("schema 'rule'"),
        "{msg}"
    );
}

#[test]
fn missing_identity_field_and_bad_identity_values() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/nover.v1.md",
        &rule("project.nover", 1, "No version").replace("version: 1\n", ""),
    );
    f.write(
        ".ai/repo/rules/project/slash.v1.md",
        &rule("project/slash", 1, "Slash"),
    );
    f.commit("ids");
    let (_, v, _) = inspect(&f.root(), &[]);
    let codes = diagnostic_codes(&v);
    assert!(
        codes.contains(&(
            "schema_violation".into(),
            Some(".ai/repo/rules/project/nover.v1.md".into())
        )),
        "{codes:?}"
    );
    assert!(
        codes.contains(&(
            "missing_field".into(),
            Some(".ai/repo/rules/project/slash.v1.md".into())
        )),
        "{codes:?}"
    );
}

#[test]
fn duplicate_identity_excludes_every_claimant() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/alpha-copy.v1.md",
        &rule("project.alpha", 1, "Alpha again"),
    );
    f.commit("dup");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 10);
    let d = diagnostics(&v);
    let dups: Vec<&str> = d
        .iter()
        .filter(|d| d["code"] == "duplicate_identity")
        .map(|d| d["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        dups,
        vec![
            ".ai/repo/rules/project/alpha-copy.v1.md",
            ".ai/repo/rules/project/alpha.v1.md"
        ]
    );
    assert!(
        !resource_uris(&v).contains(&"majordomus://rule/project.alpha@1".to_string()),
        "neither claimant is served"
    );
}

#[test]
fn a_different_version_is_a_different_identity() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/alpha.v2.md",
        &rule("project.alpha", 2, "Alpha v2"),
    );
    f.commit("v2");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 0);
    let uris = resource_uris(&v);
    assert!(uris.contains(&"majordomus://rule/project.alpha@1".to_string()));
    assert!(uris.contains(&"majordomus://rule/project.alpha@2".to_string()));
}

#[test]
fn kind_mismatch_is_refused() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/prompts/odd.md",
        "---\nname: odd\ndescription: says it is a rule\nkind: rule\n---\n",
    );
    f.commit("odd");
    let (_, v, _) = inspect(&f.root(), &[]);
    let codes = diagnostic_codes(&v);
    // `kind` is not a prompt key, so the allow-list refuses it first; that is the contract.
    assert!(
        codes
            .iter()
            .any(|(c, p)| c == "unknown_key" && p.as_deref() == Some(".ai/repo/prompts/odd.md")),
        "{codes:?}"
    );
    // a rule declaring another kind under the rule class is a mismatch
    f.write(
        ".ai/repo/rules/project/gamma.v1.md",
        &rule("project.gamma", 1, "Gamma").replace("kind: rule", "kind: prompt"),
    );
    f.commit("gamma");
    let (_, v, _) = inspect(&f.root(), &[]);
    let codes = diagnostic_codes(&v);
    // the rule schema pins kind to "rule", so the mismatch is a schema violation naming kind
    assert!(!codes.is_empty());
    let hit = diagnostics(&v)
        .into_iter()
        .find(|d| d["path"] == ".ai/repo/rules/project/gamma.v1.md")
        .expect("gamma named");
    assert_eq!(hit["code"], "schema_violation");
    assert!(hit["message"].as_str().unwrap().contains("kind"), "{hit}");
}

#[test]
fn unsupported_policy_version_is_named() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/policy.yaml",
        &common::POLICY.replace("version: 1", "version: 2"),
    );
    f.commit("v2");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 10);
    let codes = diagnostic_codes(&v);
    assert!(
        codes.contains(&(
            "unsupported_version".into(),
            Some(".ai/repo/policy.yaml".into())
        )),
        "{codes:?}"
    );
}

#[test]
fn malformed_yaml_in_a_yaml_kind_is_named() {
    let f = Fixture::new();
    f.write(".ai/repo/profiles/odd.yaml", "name: odd\n   effort: high\n");
    f.commit("odd");
    let (_, v, _) = inspect(&f.root(), &[]);
    let d = diagnostics(&v);
    let hit = d
        .iter()
        .find(|d| d["code"] == "malformed_yaml")
        .expect("malformed_yaml");
    assert!(
        hit["message"]
            .as_str()
            .unwrap()
            .contains("odd indentation on line 2"),
        "{hit}"
    );
}

#[test]
fn invalid_utf8_and_symlinks_are_refused() {
    let f = Fixture::new();
    f.write_bytes(
        ".ai/repo/rules/project/latin1.v1.md",
        b"---\nid: project.latin1\nversion: 1\ntitle: caf\xe9\n---\n",
    );
    f.commit("latin1");
    let (_, v, _) = inspect(&f.root(), &[]);
    let codes = diagnostic_codes(&v);
    assert!(
        codes.contains(&(
            "invalid_utf8".into(),
            Some(".ai/repo/rules/project/latin1.v1.md".into())
        )),
        "{codes:?}"
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(f.path("README.md"), f.path("docs/LINK.md")).unwrap();
        f.commit("link");
        let (_, v, _) = inspect(&f.root(), &[]);
        let codes = diagnostic_codes(&v);
        assert!(
            codes.contains(&("symlink".into(), Some("docs/LINK.md".into()))),
            "{codes:?}"
        );
    }
}

#[test]
fn a_required_class_that_discovers_nothing_is_an_error() {
    let f = Fixture::new();
    f.remove(".ai/repo/profiles/implementation.yaml");
    f.commit("no profiles");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 10);
    let d = diagnostics(&v);
    assert!(
        d.iter().any(|d| d["code"] == "required_source_empty"
            && d["message"].as_str().unwrap().contains("'profile'")),
        "{d:?}"
    );
}

#[test]
fn a_kind_the_executable_does_not_read_is_reported_not_guessed() {
    let f = Fixture::new();
    let sources = common::SOURCES.replace(
        "  - id: readme\n    kind: document",
        "  - id: skill\n    kind: skill\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/skills/*.md'\n    required: false\n\n  - id: readme\n    kind: document",
    );
    f.write(".ai/repo/knowledge/sources.yaml", &sources);
    f.write(".ai/repo/skills/one.md", "# One\n");
    f.commit("skill");
    let (_, v, _) = inspect(&f.root(), &[]);
    let codes = diagnostic_codes(&v);
    assert!(
        codes.contains(&("unknown_kind".into(), Some(".ai/repo/skills/one.md".into()))),
        "{codes:?}"
    );
}

#[test]
fn broken_sources_file_stops_startup() {
    let f = Fixture::new();
    f.write(".ai/repo/knowledge/sources.yaml", "version: 1\nsources:\n  - id: x\n    kind: rule\n    discovery: vcs\n    pathspec: 'no-glob-prefix'\n    required: true\n");
    f.commit("bad sources");
    let (code, _, err) = common::run_in(&f.root(), &["mcp", "--inspect"], "");
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("pathspec must start with :(glob)"), "{err}");
}

#[test]
fn a_context_document_is_read_as_context_whatever_class_found_it() {
    let f = Fixture::new();
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 0);
    let rules_readme = v["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["uri"] == "majordomus://context/ai.repo.rules")
        .expect("context resource");
    assert_eq!(rules_readme["meta"]["kind"], "context");
    assert_eq!(
        rules_readme["meta"]["provenance"]["source_class"], "rule",
        "discovered by the rule class"
    );
    assert_eq!(
        rules_readme["meta"]["provenance"]["path"],
        ".ai/repo/rules/README.md"
    );
    // an unsupported context schema is named, and a declared kind nothing lets a file declare is a mismatch
    f.write(
        ".ai/repo/rules/README.md",
        &common::context_doc("ai.repo.rules", "Rules").replace("context/v1", "context/v0"),
    );
    f.write(
        ".ai/repo/prompts/odd.md",
        "---\nname: odd\ndescription: d\n---\n",
    );
    f.write(
        ".ai/repo/workflows/plan.md",
        "---\nschema: context/v1\nid: x\nkind: profile\n---\n",
    );
    f.commit("odd");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 10);
    let codes = diagnostic_codes(&v);
    assert!(
        codes.contains(&(
            "unsupported_version".into(),
            Some(".ai/repo/rules/README.md".into())
        )),
        "{codes:?}"
    );
    assert!(
        codes
            .iter()
            .any(|(c, p)| p.as_deref() == Some(".ai/repo/workflows/plan.md")
                && (c == "unknown_key" || c == "kind_mismatch")),
        "{codes:?}"
    );
}

#[test]
fn a_collection_file_yields_one_object_per_member_with_member_provenance() {
    let f = Fixture::new();
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 0);
    let res = v["resources"].as_array().unwrap();
    let claim = res
        .iter()
        .find(|r| r["uri"] == "majordomus://claim/policy-parse")
        .expect("claim resource");
    assert_eq!(claim["meta"]["provenance"]["path"], "docs/CLAIMS.yaml");
    assert_eq!(claim["meta"]["provenance"]["member"], "claims.0");
    assert_eq!(claim["meta"]["provenance"]["source_class"], "claims");
    assert_eq!(claim["media_type"], "application/json");
    assert_eq!(
        claim["title"],
        "The policy is parsed and an unknown key is refused"
    );
    assert_eq!(claim["description"], "A restricted YAML subset.");
    // reading it returns the member as JSON, not the whole file
    let (_, out, _) = common::run_in(
        &f.root(),
        &[
            "capabilities",
            "describe",
            "claim.routing",
            "--format",
            "json",
        ],
        "",
    );
    let c: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(c["provenance"]["path"], "docs/CLAIMS.yaml");
    assert_eq!(c["provenance"]["member"], "claims.1");
    // a member without its identity is named by member, the others still serve; a wrong file version stops the file
    f.write(
        "docs/CLAIMS.yaml",
        &common::CLAIMS.replace("  - id: routing\n    claim:", "  - claim:"),
    );
    f.commit("no id");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 10);
    let d = diagnostics(&v);
    let hit = d
        .iter()
        .find(|d| d["code"] == "missing_field" && d["path"] == "docs/CLAIMS.yaml")
        .expect("member diagnostic");
    assert!(
        hit["message"].as_str().unwrap().contains("claims.1"),
        "{hit}"
    );
    assert!(resource_uris(&v).contains(&"majordomus://claim/policy-parse".to_string()));
    f.write(
        "docs/CLAIMS.yaml",
        &common::CLAIMS.replace("version: 1", "version: 2"),
    );
    f.commit("v2");
    let (_, v, _) = inspect(&f.root(), &[]);
    let codes = diagnostic_codes(&v);
    assert!(
        codes.contains(&(
            "unsupported_version".into(),
            Some("docs/CLAIMS.yaml".into())
        )),
        "{codes:?}"
    );
    assert!(!resource_uris(&v)
        .iter()
        .any(|u| u.starts_with("majordomus://claim/")));
}

#[test]
fn text_kinds_carry_content_and_no_metadata() {
    let f = Fixture::new();
    let (_, out, _) = common::run_in(
        &f.root(),
        &[
            "capabilities",
            "describe",
            "implementation.lib/a.sh",
            "--format",
            "json",
        ],
        "",
    );
    let c: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(c["kind"], "resource");
    assert_eq!(c["provenance"]["media_type"], "text/plain");
    assert_eq!(
        c["exposure"]["mcp"]["resource"]["uri"],
        "majordomus://implementation/lib/a.sh"
    );
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 0);
    let t = v["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["uri"] == "majordomus://test/test/cases/00_x.sh")
        .unwrap();
    assert_eq!(t["meta"]["kind"], "test");
    assert!(t["title"].is_null() || t["title"] == "test/cases/00_x.sh");
}

#[test]
fn a_schema_violation_names_the_field_and_the_constraint() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/typed.v1.md",
        &rule("project.typed", 1, "Typed")
            .replace("class: advisory", "class: fatal")
            .replace("version: 1", "version: one"),
    );
    f.commit("typed");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 10);
    let d = diagnostics(&v);
    let hit = d
        .iter()
        .find(|d| d["code"] == "schema_violation")
        .expect("schema_violation");
    let msg = hit["message"].as_str().unwrap();
    assert!(
        msg.contains("schema 'rule'") && msg.contains("class") && msg.contains("version"),
        "{msg}"
    );
    assert!(!resource_uris(&v)
        .iter()
        .any(|u| u.contains("project.typed")));
}

#[test]
fn without_a_share_directory_nothing_starts_and_the_search_is_named() {
    let f = Fixture::new();
    let child = std::process::Command::new(common::BIN)
        .args(["mcp", "--inspect"])
        .current_dir(f.root())
        .env_remove("MAJORDOMUS_SHARE")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.wait_with_output().unwrap();
    // the test binary sits under target/debug, beside no share/; the fixture has none either
    assert_eq!(
        out.status.code(),
        Some(12),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("no share directory holds kinds.yaml") && err.contains("--share"),
        "{err}"
    );
    let (code, _, _) = common::run_in(
        &f.root(),
        &[
            "mcp",
            "--inspect",
            "--share",
            common::dist_share().to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(code, 0);
}

#[test]
fn a_non_ascii_identity_is_served_not_fatal() {
    let f = Fixture::new();
    f.write("docs/Příručka.md", "# Příručka\n\nČeský text.\n");
    f.write(
        ".ai/repo/rules/project/priklad.v1.md",
        &rule("project.příklad", 1, "Příklad"),
    );
    f.commit("unicode");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 0);
    let uris = resource_uris(&v);
    assert!(
        uris.contains(&"majordomus://document/docs/Příručka.md".to_string()),
        "{uris:?}"
    );
    assert!(
        uris.contains(&"majordomus://rule/project.příklad@1".to_string()),
        "{uris:?}"
    );
}

#[test]
fn a_layer_readme_without_the_context_contract_is_named_by_the_manifest_convention() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/README.md",
        "# Repository rules\n\nNo front matter.\n",
    );
    f.commit("plain readme");
    let (code, v, _) = inspect(&f.root(), &[]);
    assert_eq!(code, 10);
    let codes = diagnostic_codes(&v);
    assert!(
        codes.contains(&(
            "missing_context_contract".into(),
            Some(".ai/repo/rules/README.md".into())
        )),
        "{codes:?}"
    );
    assert!(!resource_uris(&v)
        .iter()
        .any(|u| u.contains("rules/README.md")));
    // a README outside the layer is under no such convention
    f.write("docs/README.md", "# Docs\n");
    f.commit("docs readme");
    let (_, v, _) = inspect(&f.root(), &[]);
    assert!(resource_uris(&v).contains(&"majordomus://document/docs/README.md".to_string()));
}
