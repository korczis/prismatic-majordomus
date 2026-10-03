use super::*;
use std::process::Command;

const PROFILES_TEXT: &str = r#"default: chatgpt
binary_extensions: ["png", "zip"]
artifact_paths: ["target/**", "**/target/**", "**/*-wt/**", "**/*.min.js"]
profiles:
  - id: chatgpt
    title: A test pack
    derived: drop
    binary: drop
    artifacts: drop
    exclude: ["docs/**"]
    include: ["docs/keep.md"]
    orientation:
      - "Read README.md first."
    shards:
      max_count: 20
      max_tokens: 900
  - id: unlimited
    title: No limits
    derived: keep
    binary: drop
    artifacts: drop
"#;

fn git(root: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
}

fn put(root: &Path, path: &str, bytes: &[u8]) {
    let p = root.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// A repository holding one of everything a pack must leave out, and some source.
fn fixture() -> (tempfile::TempDir, Profiles) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    put(root, "README.md", b"# Fixture\n");
    put(root, "src/a.rs", b"fn a() {}\n");
    // enough source that a small budget cuts it into several shards
    for m in 1..=4 {
        let body: String = (0..40).map(|i| format!("fn f{m}_{i}() {{}}\n")).collect();
        put(root, &format!("src/m{m}.rs"), body.as_bytes());
    }
    put(root, "src/no_newline.rs", b"fn b() {}");
    put(root, "src/fence.md", b"```` four ticks\n");
    put(root, "logo.png", b"PNG");
    put(root, "blob.dat", b"text\0with a nul");
    put(root, "latin1.txt", &[0x63, 0x61, 0x66, 0xe9]);
    put(root, "target/debug/out.rs", b"fn built() {}\n");
    put(root, "apps/x/target/release/y.rs", b"fn built() {}\n");
    put(root, "repo-wt/feature/a/file.rs", b"fn other() {}\n");
    put(root, "vendor/lib.min.js", b"var a=1;\n");
    put(root, "docs/drop.md", b"dropped\n");
    put(root, "docs/keep.md", b"kept\n");
    put(root, "gen/out.json", b"{}\n");
    put(root, ".gitattributes", b"gen/** merge=derived\n");
    #[cfg(unix)]
    std::os::unix::fs::symlink("/elsewhere", root.join("link")).unwrap();
    git(root, &["add", "-A"]);
    // a gitlink, as `git add` of a nested repository records it
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000,1111111111111111111111111111111111111111,nested-wt",
        ],
    );
    git(root, &["commit", "-qm", "fixture"]);
    (dir, Profiles::parse(PROFILES_TEXT).unwrap())
}

fn reasons(plan: &PackPlan) -> BTreeMap<String, String> {
    plan.dropped_files
        .iter()
        .map(|d| (d.path.clone(), d.reason.as_str().to_string()))
        .collect()
}

#[test]
fn binaries_worktrees_links_and_build_output_never_reach_a_pack() {
    let (dir, profiles) = fixture();
    let profile = profiles.profile(None).unwrap().clone();
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let p = &planned.plan;
    assert!(p.measured && p.passes, "{:?}", p.findings);
    let r = reasons(p);
    assert_eq!(r["logo.png"], "binary", "by extension");
    assert_eq!(r["blob.dat"], "binary", "by a NUL byte");
    assert_eq!(r["latin1.txt"], "binary", "content that is not UTF-8");
    assert_eq!(r["target/debug/out.rs"], "artifact");
    assert_eq!(r["apps/x/target/release/y.rs"], "artifact");
    assert_eq!(
        r["repo-wt/feature/a/file.rs"], "artifact",
        "a worktree container"
    );
    assert_eq!(r["vendor/lib.min.js"], "artifact");
    assert_eq!(r["nested-wt"], "worktree", "a gitlink");
    #[cfg(unix)]
    assert_eq!(r["link"], "link");
    assert_eq!(r["docs/drop.md"], "excluded");
    assert_eq!(
        p.dropped["derived"], 1,
        "derived files are counted, not listed"
    );
    assert!(!r.contains_key("gen/out.json"));
    let carried: Vec<&str> = planned
        .files
        .iter()
        .map(|(_, s)| s.entry.path.as_str())
        .collect();
    assert!(
        carried.contains(&"docs/keep.md"),
        "include re-admits an excluded file"
    );
    assert!(carried.contains(&"src/a.rs") && carried.contains(&"README.md"));
    for forbidden in [
        "logo.png",
        "target/debug/out.rs",
        "nested-wt",
        "link",
        "gen/out.json",
    ] {
        assert!(!carried.contains(&forbidden), "{forbidden} travelled");
    }
}

#[test]
fn an_include_never_readmits_a_binary_or_an_artifact() {
    let (dir, profiles) = fixture();
    let mut profile = profiles.profile(None).unwrap().clone();
    profile.include = vec!["**".into()];
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let r = reasons(&planned.plan);
    assert_eq!(r["logo.png"], "binary");
    assert_eq!(r["target/debug/out.rs"], "artifact");
    assert_eq!(r["nested-wt"], "worktree");
    assert!(
        !r.contains_key("docs/drop.md"),
        "but it does re-admit an exclusion"
    );
}

#[test]
fn a_built_pack_verifies_and_every_tampering_is_named() {
    let (dir, profiles) = fixture();
    let profile = profiles.profile(None).unwrap().clone();
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let out = dir.path().join("tmp/packs/p");
    let verdict = build(&planned, &profiles, dir.path(), &out).unwrap();
    assert!(verdict.passes, "{:?}", verdict.findings);
    assert_eq!(verdict.sources, planned.plan.selected);
    assert!(
        planned.plan.shards.len() > 1,
        "a 900-token budget cuts several shards"
    );

    // the index orients and names what is not here
    let index = std::fs::read_to_string(out.join(INDEX)).unwrap();
    assert!(index.contains("Read README.md first."), "{index}");
    assert!(index.contains("`nested-wt` (worktree)"), "{index}");

    // a stray binary dropped into the directory
    std::fs::write(out.join("logo.png"), b"PNG").unwrap();
    let v = verify(&out, &profiles, dir.path());
    assert!(
        v.findings.iter().any(|f| f.code == "pack.stray"),
        "{:?}",
        v.findings
    );
    std::fs::remove_file(out.join("logo.png")).unwrap();

    // a shard whose content was changed
    let first = &planned.plan.shards[0].file;
    let text = std::fs::read_to_string(out.join(first)).unwrap();
    std::fs::write(out.join(first), text.replacen("Fixture", "Fixturf", 1)).unwrap();
    let v = verify(&out, &profiles, dir.path());
    assert!(!v.passes);
    assert!(
        v.findings.iter().any(|f| f.code == "pack.tampered"),
        "{:?}",
        v.findings
    );

    // rebuilding over a pack replaces it; over something else it refuses
    assert!(build(&planned, &profiles, dir.path(), &out).unwrap().passes);
    let other = dir.path().join("not-a-pack");
    put(dir.path(), "not-a-pack/keep.txt", b"mine\n");
    assert!(build(&planned, &profiles, dir.path(), &other).is_err());
    assert!(
        other.join("keep.txt").is_file(),
        "nothing of someone else's is removed"
    );
}

#[test]
fn every_block_restores_its_content_exactly() {
    let (dir, profiles) = fixture();
    let profile = profiles.profile(None).unwrap().clone();
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let out = dir.path().join("tmp/packs/q");
    build(&planned, &profiles, dir.path(), &out).unwrap();
    let all: String = planned
        .plan
        .shards
        .iter()
        .map(|s| std::fs::read_to_string(out.join(&s.file)).unwrap())
        .collect();
    // the four-backtick content is held by a five-backtick fence
    assert!(
        all.contains("`````markdown\n```` four ticks\n`````\n"),
        "{all}"
    );
    // a file without a final newline is marked so and restored without one
    assert!(all.contains("\"eol\":false"));
    let mut f = Vec::new();
    let blocks = shard::tests_parse(&all, &mut f);
    assert!(f.is_empty(), "{f:?}");
    assert_eq!(blocks, planned.plan.selected);
}

#[test]
fn a_file_over_the_budget_and_too_many_shards_refuse_the_pack() {
    let (dir, profiles) = fixture();
    let mut profile = profiles.profile(None).unwrap().clone();
    profile.shards = Some(ShardLimits {
        max_count: 2,
        max_tokens: 60,
    });
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let codes: BTreeSet<&str> = planned
        .plan
        .findings
        .iter()
        .map(|f| f.code.as_str())
        .collect();
    assert!(codes.contains("pack.file_too_large"), "{codes:?}");
    assert!(!planned.plan.passes);
    assert!(build(&planned, &profiles, dir.path(), &dir.path().join("x")).is_err());

    let mut few = profile.clone();
    few.shards = Some(ShardLimits {
        max_count: 2,
        max_tokens: 900,
    });
    let planned = plan_with(dir.path(), &profiles, &few).unwrap();
    assert!(planned
        .plan
        .findings
        .iter()
        .any(|f| f.code == "pack.too_many_shards"));

    let none = profiles.profile(Some("unlimited")).unwrap().clone();
    let planned = plan_with(dir.path(), &profiles, &none).unwrap();
    assert!(planned
        .plan
        .findings
        .iter()
        .any(|f| f.code == "pack.no_limits"));
}

#[test]
fn a_path_of_this_machine_refuses_the_pack_and_content_travels_unchanged() {
    let (dir, profiles) = fixture();
    let here = format!("{}/src", dir.path().display());
    put(
        dir.path(),
        "src/leak.rs",
        format!("// built in {here}\n").as_bytes(),
    );
    // prose the published-text redaction table would rewrite: a pack never rewrites it
    let prose = "the task-in-progress-for-three-weeks moment\n";
    put(dir.path(), "src/prose.md", prose.as_bytes());
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-qm", "leak"]);
    let profile = profiles.profile(None).unwrap().clone();
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let leak = planned
        .plan
        .findings
        .iter()
        .find(|f| f.code == "pack.leak")
        .unwrap();
    assert_eq!(leak.path.as_deref(), Some("src/leak.rs"));
    assert!(leak.message.starts_with("line 1 "), "{}", leak.message);
    let carried = planned
        .files
        .iter()
        .find(|(_, s)| s.entry.path == "src/prose.md")
        .unwrap();
    assert_eq!(carried.1.text, prose);
}

#[test]
fn shard_names_follow_the_directory_they_open_with() {
    assert_eq!(
        shard::shard_name(1, "apps/majordomus-cli/src/a.rs"),
        "01-apps-majordomus-cli.md"
    );
    assert_eq!(
        shard::shard_name(12, ".ai/repo/rules/x.md"),
        "12-ai-repo.md"
    );
    assert_eq!(shard::shard_name(3, "README.md"), "03-root.md");
    assert_eq!(shard::shard_name(4, "docs/A.md"), "04-docs.md");
}

#[test]
fn a_hosted_runner_home_is_not_a_machine_path() {
    assert!(is_runner_home(Path::new(&format!(
        "{}{}",
        "/home/", "runner"
    ))));
    assert!(!is_runner_home(Path::new(&format!(
        "{}{}",
        "/home/", "someone"
    ))));
}

#[test]
fn a_repository_profile_replaces_the_shipped_one_whole() {
    let mut p = Profiles::parse(PROFILES_TEXT).unwrap();
    let local = Profiles::parse("default: x\nprofiles:\n  - id: chatgpt\n    title: Mine\n")
        .unwrap()
        .profiles;
    p.overlay(local);
    let mine = p.profile(Some("chatgpt")).unwrap();
    assert_eq!(mine.title, "Mine");
    assert!(
        mine.shards.is_none(),
        "nothing of the shipped profile is merged in"
    );
    assert_eq!(p.profiles.iter().filter(|q| q.id == "chatgpt").count(), 1);
    assert_eq!(p.default, "chatgpt", "the default stays the distribution's");
}
