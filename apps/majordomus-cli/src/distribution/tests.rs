//! The distribution model's own invariants: what it parses, what it refuses, and what the
//! naming function derives. The repository's real model is checked here too, so that a
//! change to `share/distribution.yaml` that breaks an invariant fails the crate's suite
//! and not only the shell gate.

use super::release::{Channel, Release, Releases};
use super::*;

/// The repository's real model, located from this crate's own path.
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the crate sits two directories below the repository root")
}

fn real_model() -> Model {
    let path = repo_root().join("share").join(FILE);
    let text = std::fs::read_to_string(&path).expect("the model is committed");
    Model::parse(&text).expect("the committed model parses and holds its invariants")
}

#[test]
fn the_committed_model_holds_every_invariant() {
    let model = real_model();
    assert_eq!(model.findings(), Vec::<String>::new());
    assert_eq!(model.schema, SCHEMA);
    assert!(
        model.published().count() >= 1,
        "a model that publishes nothing offers nothing"
    );
}

#[test]
fn every_published_target_derives_a_unique_artifact_name() {
    let model = real_model();
    let mut names: Vec<String> = model
        .published()
        .map(|t| t.artifact_name(&model.project, &model.archive, "v1.2.3"))
        .collect();
    let total = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), total, "two targets derive one artifact name");
}

#[test]
fn the_naming_function_is_binary_tag_target_extension() {
    let model = real_model();
    let target = model.published().next().expect("one published target");
    assert_eq!(
        target.artifact_name(&model.project, &model.archive, "v0.2.0"),
        format!(
            "{}-v0.2.0-{}.{}",
            model.project.binary, target.rust_target, model.archive.extension
        )
    );
    assert_eq!(
        target.archive_root(&model.project, "v0.2.0"),
        format!("{}-v0.2.0-{}", model.project.binary, target.rust_target)
    );
}

#[test]
fn the_install_command_is_composed_from_its_parts() {
    let model = real_model();
    assert_eq!(
        model.install_command(),
        format!(
            "{} {}/{} | {}",
            model.installer.download_command,
            model.installer.base_url,
            model.installer.script,
            model.installer.shell
        )
    );
    assert!(model.install_and_init_command().ends_with("-s -- --init"));
}

#[test]
fn every_linux_target_declares_a_libc_and_no_other_does() {
    let model = real_model();
    for t in &model.targets {
        assert_eq!(
            t.os == Os::Linux,
            t.libc.is_some(),
            "target {} disagrees with the libc rule",
            t.id
        );
    }
}

#[test]
fn every_published_target_names_a_runner_and_every_other_names_a_reason() {
    let model = real_model();
    for t in &model.targets {
        if t.status.is_published() {
            assert!(t.build.is_some(), "{} publishes without a build", t.id);
        } else {
            assert!(t.reason.is_some(), "{} is unbuilt without a reason", t.id);
        }
    }
}

fn minimal() -> String {
    let model = real_model();
    let path = repo_root().join("share").join(FILE);
    let _ = model;
    std::fs::read_to_string(path).expect("the model is committed")
}

#[test]
fn a_duplicate_target_id_is_a_finding() {
    let text = minimal();
    let mut model = Model::parse(&text).expect("parses");
    let first = model.targets[0].clone();
    model.targets.push(Target {
        rust_target: "sparc64-unknown-linux-gnu".into(),
        ..first
    });
    assert!(
        model.findings().iter().any(|f| f.contains("claim the id")),
        "a repeated id must be reported: {:?}",
        model.findings()
    );
}

#[test]
fn a_duplicate_rust_target_is_a_finding() {
    let text = minimal();
    let mut model = Model::parse(&text).expect("parses");
    let first = model.targets[0].clone();
    model.targets.push(Target {
        id: "another".into(),
        ..first
    });
    assert!(model
        .findings()
        .iter()
        .any(|f| f.contains("claim the Rust target")));
}

#[test]
fn an_insecure_base_url_is_a_finding() {
    let text = minimal();
    let mut model = Model::parse(&text).expect("parses");
    model.installer.base_url = "http://example.invalid".into();
    assert!(model.findings().iter().any(|f| f.contains("not https")));
}

#[test]
fn an_unknown_key_is_refused_rather_than_ignored() {
    let text = format!("{}\nsurprise: true\n", minimal());
    assert!(Model::parse(&text).is_err());
}

#[test]
fn a_model_of_another_schema_version_is_refused() {
    let text = minimal().replace(SCHEMA, "majordomus-distribution/v99");
    let err = Model::parse(&text).expect_err("a newer contract is refused, never guessed");
    assert!(err.contains("majordomus-distribution/v99"), "{err}");
}

fn sample_release(model: &Model, tag: &str, channel: Channel) -> Release {
    Release {
        schema: release::SCHEMA.into(),
        version: tag.trim_start_matches('v').to_string(),
        tag: tag.to_string(),
        channel,
        commit: "0".repeat(40),
        published_at: "2026-01-01T00:00:00Z".into(),
        notes_url: None,
        yanked: false,
        artifacts: model
            .published()
            .map(|t| {
                let name = t.artifact_name(&model.project, &model.archive, tag);
                release::ReleaseArtifact {
                    target: t.id.clone(),
                    url: format!("{}{tag}/{name}", model.project.download_prefix()),
                    name,
                    sha256: "a".repeat(64),
                    size: 1024,
                }
            })
            .collect(),
    }
}

#[test]
fn a_complete_release_agrees_with_the_model() {
    let model = real_model();
    let release = sample_release(&model, "v0.2.0", Channel::Stable);
    assert_eq!(release.findings(&model), Vec::<String>::new());
}

#[test]
fn a_target_added_after_a_release_does_not_invalidate_it() {
    // The trap this replaces: `findings` used to require an artifact for every target the
    // model publishes *now*, so adding a platform made every published release incomplete
    // and `generate --target distribution` refused the tree — a platform could not be added
    // at all. Completeness is decided by scripts/release-record when the record is written.
    let model = real_model();
    let mut release = sample_release(&model, "v0.2.0", Channel::Stable);
    release.artifacts.pop();
    assert_eq!(
        release.findings(&model),
        Vec::<String>::new(),
        "a release that predates a target cannot carry it, and is not in breach for that"
    );
}

#[test]
fn a_release_artifact_for_a_target_the_model_does_not_declare_is_refused() {
    let model = real_model();
    let mut release = sample_release(&model, "v0.2.0", Channel::Stable);
    release.artifacts[0].target = "sparc64-unknown-linux-gnu".into();
    assert!(release
        .findings(&model)
        .iter()
        .any(|f| f.contains("the model does not declare")));
}

#[test]
fn a_release_artifact_named_by_hand_is_refused() {
    let model = real_model();
    let mut release = sample_release(&model, "v0.2.0", Channel::Stable);
    release.artifacts[0].name = "majordomus.tar.gz".into();
    assert!(release
        .findings(&model)
        .iter()
        .any(|f| f.contains("the naming function derives")));
}

#[test]
fn a_release_artifact_from_another_host_is_refused() {
    let model = real_model();
    let mut release = sample_release(&model, "v0.2.0", Channel::Stable);
    release.artifacts[0].url = "https://example.invalid/majordomus.tar.gz".into();
    assert!(release
        .findings(&model)
        .iter()
        .any(|f| f.contains("served from")));
}

#[test]
fn the_stable_pointer_is_the_highest_unyanked_stable_release() {
    let model = real_model();
    let mut yanked = sample_release(&model, "v0.3.0", Channel::Stable);
    yanked.yanked = true;
    let releases = Releases::ordered(vec![
        sample_release(&model, "v0.1.0", Channel::Stable),
        sample_release(&model, "v0.4.0", Channel::Prerelease),
        yanked,
        sample_release(&model, "v0.2.0", Channel::Stable),
    ]);
    assert_eq!(
        releases.latest_stable().map(|r| r.tag.as_str()),
        Some("v0.2.0"),
        "a prerelease is never resolved and a withdrawn release is skipped"
    );
    assert_eq!(releases.by_tag("v0.3.0").map(|r| r.yanked), Some(true));
}

#[test]
fn a_prerelease_sorts_below_its_own_final_release() {
    let model = real_model();
    let releases = Releases::ordered(vec![
        sample_release(&model, "v1.0.0-rc.1", Channel::Stable),
        sample_release(&model, "v1.0.0", Channel::Stable),
    ]);
    assert_eq!(
        releases.releases.first().map(|r| r.tag.as_str()),
        Some("v1.0.0")
    );
}

#[test]
fn the_public_metadata_holds_one_line_per_artifact() {
    let model = real_model();
    let release = sample_release(&model, "v0.2.0", Channel::Stable);
    let json = release.public_json(&model);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["tag"], "v0.2.0");
    for t in model.published() {
        let line = format!("    \"{}\": {{", t.rust_target);
        assert!(
            json.lines().filter(|l| l.starts_with(&line)).count() == 1,
            "the installer parses one line per target; {} has none or many",
            t.rust_target
        );
        assert_eq!(parsed["artifacts"][&t.rust_target]["size"], 1024);
    }
}

#[test]
fn the_public_metadata_says_it_is_generated() {
    // The typed-artifact check refuses a generated JSON document that does not declare
    // itself one, and these two files are written only when a release exists — so nothing
    // but this test notices before a publication does.
    let model = real_model();
    let release = sample_release(&model, "v0.2.0", Channel::Stable);
    let parsed: serde_json::Value =
        serde_json::from_str(&release.public_json(&model)).expect("valid JSON");
    let banner = parsed["generated"].as_str().unwrap_or_default();
    assert!(
        banner.starts_with(crate::generate::HEADER),
        "the public release metadata carries no generated banner: {banner:?}"
    );
}

#[test]
fn the_matrix_carries_the_tag_placeholder_and_never_a_tag() {
    let model = real_model();
    let json = render::matrix_json(&model);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let include = parsed["include"].as_array().expect("an include list");
    assert_eq!(include.len(), model.published().count());
    for entry in include {
        let artifact = entry["artifact"].as_str().expect("an artifact name");
        assert!(
            artifact.contains(render::TAG_PLACEHOLDER),
            "the workflow substitutes a tag rather than composing a name: {artifact}"
        );
        assert!(!entry["runner"].as_str().unwrap_or_default().is_empty());
    }
}

#[test]
fn the_installer_region_quotes_every_value_it_emits() {
    let model = real_model();
    let region = render::installer_region(&model);
    assert!(region.starts_with(render::REGION_BEGIN));
    assert!(region.trim_end().ends_with(render::REGION_END));
    for t in model.published() {
        assert!(
            region.contains(&t.rust_target),
            "the installer's table must carry {}",
            t.rust_target
        );
    }
    assert!(region.contains("MJ_BINARY='majordomus'"));
}

#[test]
fn splicing_the_installer_replaces_only_the_region() {
    let model = real_model();
    let template = format!(
        "#!/bin/sh\nbefore\n{}\nstale\n{}\nafter\n",
        render::REGION_BEGIN,
        render::REGION_END
    );
    let out = render::installer(&model, &template).expect("the template carries a region");
    assert!(out.starts_with("#!/bin/sh\nbefore\n"));
    assert!(out.ends_with("after\n"));
    assert!(!out.contains("stale"));
    assert!(out.contains("MJ_TARGETS='"));
}

#[test]
fn a_template_without_a_region_is_refused() {
    let model = real_model();
    assert!(render::installer(&model, "#!/bin/sh\n").is_err());
}

#[test]
fn the_guide_refuses_an_unsubstituted_placeholder() {
    let model = real_model();
    let releases = Releases::default();
    let err = render::install_doc(&model, &releases, "0.1.0", "{{nothing_declares_this}}")
        .expect_err("a placeholder nothing fills is a mistake in the template");
    assert!(err.contains("unsubstituted"));
}

#[test]
fn the_platform_table_lists_every_declared_target() {
    let model = real_model();
    let table = render::platform_table(&model);
    for t in &model.targets {
        assert!(
            table.contains(&t.rust_target),
            "the table must list {}",
            t.rust_target
        );
    }
}
