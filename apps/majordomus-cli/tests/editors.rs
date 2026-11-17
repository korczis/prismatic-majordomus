//! `majordomus generate editors` over a real repository: a skill and a justfile become the
//! files Zed and Antigravity read, a file somebody wrote at a projected path is refused by
//! name, a projection whose skill is retired is reported by `--check` and removed by
//! `generate`, and a second run changes nothing (ADR 0140).

mod common;

use common::{run_in, Fixture, SOURCES};

const SKILL_SOURCE: &str = "
  - id: skill
    kind: skill
    discovery: vcs
    pathspec: ':(glob).ai/repo/skills/*/SKILL.md'
    required: false
";

fn skill(status: &str) -> String {
    format!(
        "---\nschema: skill/v1\nid: triage\nversion: 1\ntitle: Triage\n\
         description: Sort a new report into the plan before anyone works on it.\n\
         status: {status}\ntags: [plan]\n---\n# Purpose\n\nSort it.\n\n# Procedure\n\n\
         1. Read it.\n\n# Output\n\nOne plan entry.\n"
    )
}

const JUSTFILE: &str = "\
# Build it.
[group('build')]
build:
    @echo build

# Remove what was built.
[group('build')]
[confirm]
clean:
    @echo clean

# Every gate.
[group('test')]
test:
    @echo test
";

fn has_just() -> bool {
    std::process::Command::new("just")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A repository with one active skill and a justfile, committed.
fn repository() -> Fixture {
    let f = Fixture::new();
    f.write(
        ".ai/repo/knowledge/sources.yaml",
        &format!("{SOURCES}{SKILL_SOURCE}"),
    );
    f.write(".ai/repo/skills/triage/SKILL.md", &skill("active"));
    f.write("justfile", JUSTFILE);
    f.commit("a skill and a justfile");
    f
}

#[test]
fn a_skill_and_a_justfile_become_what_zed_and_antigravity_read() {
    let f = repository();
    let (code, out, err) = run_in(&f.root(), &["generate", "editors"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains(".agents/skills/triage/SKILL.md"), "{out}");

    let projected = std::fs::read_to_string(f.path(".agents/skills/triage/SKILL.md")).unwrap();
    assert!(projected.starts_with("---\nname: triage\ndescription: \"Sort a new report"));
    assert!(projected.contains("GENERATED FILE"));
    assert!(projected.contains("](../../../.ai/repo/skills/triage/SKILL.md)"));
    assert!(
        !projected.contains("Read it."),
        "the procedure is linked, never copied"
    );

    let tasks = f.path(".zed/tasks.json");
    if has_just() {
        let text = std::fs::read_to_string(&tasks).unwrap();
        assert!(text.starts_with("// GENERATED FILE"), "{text}");
        let body: String = text
            .lines()
            .filter(|l| !l.starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        let labels: Vec<&str> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["label"].as_str().unwrap())
            .collect();
        assert_eq!(
            labels,
            ["just build", "just test"],
            "declared group order, and a [confirm] recipe is never one keystroke away"
        );
    } else {
        assert!(!tasks.exists(), "no runner, no task list");
    }

    // the check holds the tree, and a second run changes nothing
    let (code, out, err) = run_in(&f.root(), &["generate", "editors", "--check"], "");
    assert_eq!(code, 0, "{out}{err}");
    let before = std::fs::read(f.path(".agents/skills/triage/SKILL.md")).unwrap();
    let (code, _, _) = run_in(&f.root(), &["generate", "editors"], "");
    assert_eq!(code, 0);
    assert_eq!(
        std::fs::read(f.path(".agents/skills/triage/SKILL.md")).unwrap(),
        before
    );
}

#[test]
fn a_file_somebody_wrote_at_a_projected_path_is_refused_by_name() {
    let f = repository();
    let mine = "---\nname: triage\ndescription: mine\n---\nmy own notes\n";
    f.write(".agents/skills/triage/SKILL.md", mine);
    let (code, out, err) = run_in(&f.root(), &["generate", "editors"], "");
    assert_eq!(code, 15, "{out}{err}");
    let said = format!("{out}{err}");
    assert!(said.contains("did not write"), "{said}");
    assert!(said.contains(".agents/skills/triage/SKILL.md"), "{said}");
    assert_eq!(
        std::fs::read_to_string(f.path(".agents/skills/triage/SKILL.md")).unwrap(),
        mine,
        "the person's file is untouched"
    );
}

#[test]
fn a_retired_skill_leaves_an_orphan_the_check_names_and_generate_removes() {
    let f = repository();
    let (code, _, _) = run_in(&f.root(), &["generate", "editors"], "");
    assert_eq!(code, 0);
    f.write(".ai/repo/skills/triage/SKILL.md", &skill("deprecated"));
    f.commit("the skill is retired");

    let (code, out, err) = run_in(&f.root(), &["generate", "editors", "--check"], "");
    assert_eq!(code, 10, "{out}{err}");
    assert!(
        format!("{out}{err}").contains(".agents/skills/triage/SKILL.md (orphaned)"),
        "{out}{err}"
    );

    let (code, out, err) = run_in(&f.root(), &["generate", "editors"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("orphaned editor projection"), "{err}");
    assert!(!f.path(".agents/skills/triage").exists());
}

/// Where `just` cannot be asked, nothing is written: a task list present on a machine that
/// has `just` and absent on one that has not would make the committed file depend on the
/// machine rather than the tree, so `generate` refuses with exit 12 and says what to install.
#[cfg(unix)]
#[test]
fn without_just_the_projection_is_refused_rather_than_half_written() {
    let f = repository();
    // a PATH that has git, which the index needs, and nothing else
    let bin = tempfile::tempdir().unwrap();
    let git = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let git = String::from_utf8(git.stdout).unwrap().trim().to_string();
    std::os::unix::fs::symlink(&git, bin.path().join("git")).unwrap();

    let out = common::majordomus()
        .args(["generate", "editors"])
        .current_dir(f.root())
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env("PATH", bin.path())
        .output()
        .unwrap();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(12), "{said}");
    assert!(said.contains("install `just`"), "{said}");
    assert!(
        !f.path(".agents/skills/triage/SKILL.md").exists(),
        "nothing is half-written"
    );
    assert!(!f.path(".zed/tasks.json").exists());
}
