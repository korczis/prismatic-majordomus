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
    let verdict = build(&planned, dir.path(), &out).unwrap();
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
    assert!(build(&planned, dir.path(), &out).unwrap().passes);
    let other = dir.path().join("not-a-pack");
    put(dir.path(), "not-a-pack/keep.txt", b"mine\n");
    assert!(build(&planned, dir.path(), &other).is_err());
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
    build(&planned, dir.path(), &out).unwrap();
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
    assert!(build(&planned, dir.path(), &dir.path().join("x")).is_err());

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

/// A pack of the fixture, built into `<fixture>/tmp/packs/v`, with the profiles it verifies
/// against and the file name of its first shard.
fn built() -> (tempfile::TempDir, Profiles, PathBuf, String) {
    let (dir, profiles) = fixture();
    let profile = profiles.profile(None).unwrap().clone();
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let out = dir.path().join("tmp/packs/v");
    assert!(build(&planned, dir.path(), &out).unwrap().passes);
    let first = planned.plan.shards[0].file.clone();
    (dir, profiles, out, first)
}

/// The codes of a verdict's findings.
fn codes(v: &PackVerdict) -> BTreeSet<String> {
    v.findings.iter().map(|f| f.code.clone()).collect()
}

/// Rewrite the first file marker of `file` in the pack at `out` with `change`.
fn remark(out: &Path, file: &str, change: impl Fn(&mut serde_json::Value)) {
    let text = std::fs::read_to_string(out.join(file)).unwrap();
    let mut done = false;
    let lines: Vec<String> = text
        .split('\n')
        .map(|l| match l.strip_prefix("<!-- majordomus:file ") {
            Some(rest) if !done => {
                done = true;
                let mut m: serde_json::Value =
                    serde_json::from_str(rest.strip_suffix(" -->").unwrap()).unwrap();
                change(&mut m);
                format!("<!-- majordomus:file {m} -->")
            }
            _ => l.to_string(),
        })
        .collect();
    assert!(done, "{file} holds no marker");
    std::fs::write(out.join(file), lines.join("\n")).unwrap();
}

/// Rewrite the pack's manifest with `change`.
fn remanifest(out: &Path, change: impl Fn(&mut serde_json::Value)) {
    let mut m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join(MANIFEST)).unwrap()).unwrap();
    change(&mut m);
    std::fs::write(out.join(MANIFEST), m.to_string()).unwrap();
}

#[test]
fn a_manifest_that_cannot_be_read_is_unmeasured_never_a_pass() {
    let (dir, profiles, out, _) = built();
    let absent = verify(&dir.path().join("nowhere"), &profiles, dir.path());
    assert!(!absent.measured && !absent.passes);
    assert!(absent.reason.unwrap().starts_with("cannot read pack.json"));

    std::fs::write(out.join(MANIFEST), "{ not json").unwrap();
    let broken = verify(&out, &profiles, dir.path());
    assert!(!broken.measured && !broken.passes);
    assert!(broken.reason.unwrap().starts_with("pack.json: "));
}

#[test]
fn a_manifest_of_another_schema_or_an_unknown_profile_is_unmeasured() {
    let (dir, profiles, out, _) = built();
    remanifest(&out, |m| m["schema"] = "majordomus.pack/v0".into());
    let v = verify(&out, &profiles, dir.path());
    assert!(!v.measured);
    assert!(v.reason.unwrap().contains("not majordomus.pack/v1"));

    remanifest(&out, |m| {
        m["schema"] = MANIFEST_SCHEMA.into();
        m["profile"] = "gone".into();
    });
    let v = verify(&out, &profiles, dir.path());
    assert!(!v.measured);
    assert!(v.reason.unwrap().contains("no profile 'gone'"));
}

#[test]
fn a_pack_over_the_profiles_limits_is_refused_by_the_verifier() {
    let (dir, mut profiles, out, _) = built();
    let p = profiles
        .profiles
        .iter_mut()
        .find(|p| p.id == "chatgpt")
        .unwrap();
    p.shards = Some(ShardLimits {
        max_count: 1,
        max_tokens: 10,
    });
    let v = verify(&out, &profiles, dir.path());
    assert!(v.measured && !v.passes);
    let c = codes(&v);
    assert!(c.contains("pack.too_many_shards"), "{c:?}");
    assert!(c.contains("pack.over_budget"), "{c:?}");

    // a profile that has lost its limits holds a pack to none
    let p = profiles
        .profiles
        .iter_mut()
        .find(|p| p.id == "chatgpt")
        .unwrap();
    p.shards = None;
    let v = verify(&out, &profiles, dir.path());
    assert!(v.passes, "{:?}", v.findings);
}

#[test]
fn a_shard_that_is_missing_not_text_or_names_this_machine_is_refused() {
    let (dir, profiles, out, first) = built();
    std::fs::remove_file(out.join(&first)).unwrap();
    let v = verify(&out, &profiles, dir.path());
    assert!(codes(&v).contains("pack.missing"), "{:?}", v.findings);

    std::fs::write(out.join(&first), [0xff, 0xfe, 0x41]).unwrap();
    let v = verify(&out, &profiles, dir.path());
    assert!(
        v.findings
            .iter()
            .any(|f| f.code == "pack.not_text" && f.message.contains("UTF-8")),
        "{:?}",
        v.findings
    );

    std::fs::write(out.join(&first), "text\0with a nul\n").unwrap();
    let v = verify(&out, &profiles, dir.path());
    assert!(
        v.findings
            .iter()
            .any(|f| f.code == "pack.not_text" && f.message.contains("NUL")),
        "{:?}",
        v.findings
    );

    let here = format!("built in {}/src\n", dir.path().display());
    std::fs::write(out.join(&first), here).unwrap();
    let v = verify(&out, &profiles, dir.path());
    assert!(codes(&v).contains("pack.leak"), "{:?}", v.findings);
}

#[test]
fn a_shard_that_does_not_carry_what_the_manifest_names_is_refused() {
    let (dir, profiles, out, first) = built();
    remanifest(&out, |m| {
        let paths = m["shards"][0]["paths"].as_array_mut().unwrap();
        paths.pop();
    });
    let v = verify(&out, &profiles, dir.path());
    assert!(
        v.findings
            .iter()
            .any(|f| f.code == "pack.manifest" && f.path.as_deref() == Some(first.as_str())),
        "{:?}",
        v.findings
    );
}

#[test]
fn a_block_whose_marker_names_what_no_pack_carries_is_refused() {
    for (field, value, what) in [
        ("mode", "160000", "a gitlink"),
        ("mode", "120000", "a symbolic link"),
        ("path", "target/debug/x.rs", "an artifact path"),
        ("path", "logo.png", "a binary extension"),
    ] {
        let (dir, profiles, out, first) = built();
        remark(&out, &first, |m| m[field] = value.into());
        let v = verify(&out, &profiles, dir.path());
        assert!(
            v.findings
                .iter()
                .any(|f| f.code == "pack.forbidden_path" && f.message.contains(what)),
            "{field}={value}: {:?}",
            v.findings
        );
    }
}

#[test]
fn a_marker_that_does_not_parse_or_a_block_left_open_is_refused() {
    let mut f = Vec::new();
    let n = shard::tests_parse("<!-- majordomus:file {not json -->\n", &mut f);
    assert_eq!(n, 0);
    assert!(
        f[0].message.contains("a file marker that does not parse"),
        "{f:?}"
    );

    let mut f = Vec::new();
    let open = "<!-- majordomus:file {\"path\":\"a.rs\",\"mode\":\"100644\",\"bytes\":3,\
                \"sha256\":\"x\",\"eol\":true} -->\n## a.rs\n\n```rust\nfn\n";
    assert_eq!(shard::tests_parse(open, &mut f), 0);
    assert!(
        f[0].message.contains("the block of a.rs is not closed"),
        "{f:?}"
    );
}

#[test]
fn every_language_the_index_knows_opens_its_fence() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    let files = [
        ("a.toml", "toml"),
        ("a.js", "javascript"),
        ("a.ts", "typescript"),
        ("a.css", "css"),
        ("a.html", "html"),
        ("a.rhai", "rust"),
        ("a.py", "python"),
    ];
    for (path, _) in files {
        put(root, path, b"x\n");
    }
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "languages"]);
    let profiles = Profiles::parse(PROFILES_TEXT).unwrap();
    let profile = profiles.profile(None).unwrap().clone();
    let planned = plan_with(root, &profiles, &profile).unwrap();
    let out = root.join("tmp/packs/l");
    build(&planned, root, &out).unwrap();
    let all = std::fs::read_to_string(out.join(&planned.plan.shards[0].file)).unwrap();
    for (path, language) in files {
        assert!(
            all.contains(&format!("## {path}\n\n```{language}\n")),
            "{path}: {all}"
        );
    }
    // a shard opening with a directory that has no letter in its name is named for the root
    assert_eq!(shard::shard_name(5, "_/a.md"), "05-root.md");
}

#[test]
fn the_machine_is_the_checkout_and_a_persons_home_in_every_spelling() {
    let root = Path::new("/private/tmp/r");
    let none = machine_paths_for(root, None);
    assert_eq!(
        none,
        ["/private/tmp/r/", "/tmp/r/"],
        "both spellings of the root"
    );
    assert_eq!(
        machine_paths_for(root, Some("/".into())),
        none,
        "/ is nobody's home"
    );
    let runner = format!("{}{}", "/home/", "runner");
    assert_eq!(
        machine_paths_for(root, Some(runner.into())),
        none,
        "a hosted runner's home names no person"
    );
    // a home that does not exist is still listed, as written; a /private prefix that is
    // not a directory of its own is not a second spelling
    let home = machine_paths_for(root, Some("/privatehome/someone".into()));
    assert!(
        home.contains(&"/privatehome/someone/".to_string()),
        "{home:?}"
    );
    assert!(!home.contains(&"home/someone/".to_string()), "{home:?}");
}

#[test]
fn profiles_that_cannot_be_read_are_named_by_the_file_that_failed() {
    let dir = tempfile::tempdir().unwrap();
    let (share, root) = (dir.path().join("share"), dir.path().join("repo"));
    std::fs::create_dir_all(&share).unwrap();
    std::fs::create_dir_all(root.join(".ai/repo")).unwrap();
    let e = Profiles::load(&share, &root).unwrap_err();
    assert!(e.starts_with("cannot read "), "{e}");

    std::fs::write(share.join(PROFILES), "profiles: []\n").unwrap();
    let e = Profiles::load(&share, &root).unwrap_err();
    assert!(
        e.contains("archive.yaml: "),
        "a profiles file without a default: {e}"
    );

    std::fs::write(share.join(PROFILES), PROFILES_TEXT).unwrap();
    std::fs::write(root.join(OVERLAY), "profiles: 7\n").unwrap();
    let e = Profiles::load(&share, &root).unwrap_err();
    assert!(e.starts_with(OVERLAY), "{e}");
}

#[test]
fn a_tree_that_cannot_be_read_is_an_unmeasured_plan() {
    let (dir, _) = fixture();
    let share = dir.path().join("share");
    std::fs::create_dir_all(&share).unwrap();
    // no profiles file
    let p = plan(dir.path(), &share, None).plan;
    assert!(!p.measured && !p.passes);
    assert!(p.reason.unwrap().starts_with("cannot read "));

    std::fs::write(share.join(PROFILES), PROFILES_TEXT).unwrap();
    // a directory git does not know
    let plain = tempfile::tempdir().unwrap();
    let p = plan(plain.path(), &share, None).plan;
    assert!(!p.measured);
    assert!(p.reason.unwrap().starts_with("git ls-files: "));
    // a directory that does not exist, where git cannot even be started
    let p = plan(&plain.path().join("gone"), &share, None).plan;
    assert!(!p.measured);
    assert!(p.reason.unwrap().starts_with("cannot run git ls-files: "));
}

#[test]
fn an_unresolved_conflict_or_a_lost_object_refuses_the_plan() {
    let (dir, profiles) = fixture();
    let profile = profiles.profile(None).unwrap().clone();

    // a blob staged and then lost from the object store
    put(dir.path(), "lost.txt", b"a blob nobody kept\n");
    git(dir.path(), &["add", "lost.txt"]);
    let oid = Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args(["rev-parse", ":lost.txt"])
        .output()
        .unwrap();
    let oid = String::from_utf8(oid.stdout).unwrap().trim().to_string();
    std::fs::remove_file(
        dir.path()
            .join(".git/objects")
            .join(&oid[..2])
            .join(&oid[2..]),
    )
    .unwrap();
    let e = plan_with(dir.path(), &profiles, &profile).err().unwrap();
    assert!(e.starts_with("lost.txt: git cat-file answered"), "{e}");
    git(dir.path(), &["rm", "-q", "--cached", "lost.txt"]);

    // the three stages a conflicted merge leaves
    let blob = Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args(["rev-parse", "HEAD:README.md"])
        .output()
        .unwrap();
    let blob = String::from_utf8(blob.stdout).unwrap().trim().to_string();
    let mut info = Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write as _;
    let stages: String = (1..=3)
        .map(|s| format!("100644 {blob} {s}\tconflict.txt\n"))
        .collect();
    info.stdin
        .take()
        .unwrap()
        .write_all(stages.as_bytes())
        .unwrap();
    assert!(info.wait().unwrap().success());
    let e = plan_with(dir.path(), &profiles, &profile).err().unwrap();
    assert!(e.starts_with("conflict.txt is unmerged"), "{e}");
}

#[test]
fn an_answer_cut_short_names_the_entry_it_was_for() {
    let entry = |path: &str, oid: &str| IndexEntry {
        mode: "100644".into(),
        oid: oid.into(),
        path: path.into(),
    };
    let whole = b"aa blob 3\nabc\nbb blob 1\nx\n";
    let blobs = split_blobs(whole, &[entry("a", "aa"), entry("b", "bb")]).unwrap();
    assert_eq!(blobs, [b"abc".to_vec(), b"x".to_vec()]);
    let e = split_blobs(b"aa blob 30\nabc\n", &[entry("a", "aa")]).unwrap_err();
    assert_eq!(e, "a: git cat-file answered 'aa blob 30'");
    let e = split_blobs(
        whole,
        &[entry("a", "aa"), entry("b", "bb"), entry("c", "cc")],
    )
    .unwrap_err();
    assert_eq!(e, "c: git cat-file answered ''");
}

#[test]
fn profiles_that_keep_artifacts_and_binaries_still_carry_only_text_and_blobs() {
    let (dir, profiles) = fixture();
    let mut profile = profiles.profile(None).unwrap().clone();
    profile.artifacts = "keep".into();
    profile.binary = "keep".into();
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let r = reasons(&planned.plan);
    assert_eq!(r["nested-wt"], "worktree", "a gitlink has no blob to carry");
    assert_eq!(
        r["latin1.txt"], "binary",
        "content that is not UTF-8 is not text"
    );
    assert!(!r.contains_key("target/debug/out.rs"), "{r:?}");
}

#[test]
fn an_index_longer_than_one_file_refuses_the_pack() {
    let (dir, profiles) = fixture();
    let mut profile = profiles.profile(None).unwrap().clone();
    profile.orientation = vec!["Read every word of this orientation. ".repeat(200)];
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    assert!(!planned.plan.passes);
    let f = &planned.plan.findings;
    assert!(f.iter().any(|f| f.code == "pack.index_too_large"), "{f:?}");
}

#[test]
fn a_destination_that_cannot_hold_a_pack_is_refused_and_left_alone() {
    let (dir, profiles) = fixture();
    let profile = profiles.profile(None).unwrap().clone();
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();
    let at = |name: &str| dir.path().join("dest").join(name);
    std::fs::create_dir_all(dir.path().join("dest")).unwrap();

    // a file, not a directory
    std::fs::write(at("file"), "x").unwrap();
    let e = build(&planned, dir.path(), &at("file")).unwrap_err();
    assert!(e.starts_with("cannot read "), "{e}");
    // a directory under a file
    let e = build(&planned, dir.path(), &at("file").join("under")).unwrap_err();
    assert!(e.starts_with("cannot create "), "{e}");

    // a pack holding something no pack writes
    let pack = at("pack");
    assert!(build(&planned, dir.path(), &pack).unwrap().passes);
    std::fs::write(pack.join("notes.txt"), "mine\n").unwrap();
    let e = build(&planned, dir.path(), &pack).unwrap_err();
    assert!(e.contains("notes.txt, which no pack writes"), "{e}");
    assert!(pack.join(MANIFEST).is_file(), "a refusal removed the pack");
    std::fs::remove_file(pack.join("notes.txt")).unwrap();
    // a directory where a shard would be replaced
    std::fs::create_dir_all(pack.join("zz.md")).unwrap();
    let e = build(&planned, dir.path(), &pack).unwrap_err();
    assert!(e.starts_with("cannot replace zz.md"), "{e}");

    // an empty directory is a destination like an absent one
    let empty = at("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(build(&planned, dir.path(), &empty).unwrap().passes);
}

#[cfg(unix)]
#[test]
fn a_pack_that_cannot_be_written_names_the_file_that_failed() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, profiles) = fixture();
    let profile = profiles.profile(None).unwrap().clone();
    let planned = plan_with(dir.path(), &profiles, &profile).unwrap();

    let locked = dir.path().join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let e = build(&planned, dir.path(), &locked);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    let e = e.unwrap_err();
    assert!(e.starts_with("cannot write 01-"), "{e}");

    for blocked in [INDEX, MANIFEST] {
        let out = dir.path().join(format!("blocked-{blocked}"));
        std::fs::create_dir_all(out.join(blocked)).unwrap();
        let e = shard::write(&planned, &out).unwrap_err();
        assert!(e.starts_with(&format!("cannot write {blocked}")), "{e}");
    }
}
