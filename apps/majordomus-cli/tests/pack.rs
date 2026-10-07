//! The source pack through every surface that reaches it: `majordomus_pack_plan` and
//! `majordomus_pack_verify` as an MCP client sends them a `tools/call`, `majordomus pack`
//! on the command line in both output shapes, and `majordomus_cli::pack` in process — all
//! against one committed fixture that holds source, a binary and a profile too tight to
//! pack it, so that every verdict a surface can give is given by a real tree.

mod common;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

/// A repository profile beside the shipped ones: the same selection as `chatgpt`, with a
/// budget no file fits in, so that a plan has a finding to refuse the build with.
const TIGHT: &str = "profiles:
  - id: tight
    title: A budget nothing fits in
    derived: drop
    binary: drop
    artifacts: drop
    shards:
      max_count: 20
      max_tokens: 10
";

/// The supervised fixture with one source file, one binary, the tight profile and an
/// ignored `tmp/`, committed.
fn fixture() -> common::Fixture {
    let f = common::Fixture::new();
    f.write("src/main.rs", "fn main() {}\n");
    f.write_bytes("src/logo.png", b"\x89PNG\0\x01\x02");
    f.write(".ai/repo/archive.yaml", TIGHT);
    f.write(".gitignore", ".ai/local/\n/tmp/\n");
    f.commit("sources");
    f
}

/// Run `majordomus pack <args>` in the fixture and return (status, stdout, stderr).
fn pack(f: &common::Fixture, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(common::BIN)
        .arg("pack")
        .args(args)
        .current_dir(f.root())
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("majordomus runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// One MCP session against the fixture: `initialize`, then one `tools/call` per entry of
/// `calls`, answered in order. Each answer is the call's `result`, or its `error` when the
/// server refused the frame.
fn mcp(f: &common::Fixture, calls: &[(&str, Value)]) -> Vec<Value> {
    let mut child = Command::new(common::BIN)
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("majordomus mcp starts");
    {
        let mut stdin = child.stdin.take().unwrap();
        let init = json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "pack-test", "version": "0" }
            }
        });
        writeln!(stdin, "{init}").unwrap();
        for (n, (tool, arguments)) in calls.iter().enumerate() {
            let call = json!({
                "jsonrpc": "2.0", "id": n + 1, "method": "tools/call",
                "params": { "name": tool, "arguments": arguments }
            });
            writeln!(stdin, "{call}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("a JSON-RPC frame"))
        .collect();
    (1..=calls.len())
        .map(|id| {
            let frame = frames
                .iter()
                .find(|f| f["id"] == json!(id))
                .unwrap_or_else(|| panic!("no answer to call {id}: {frames:?}"));
            if frame.get("error").is_some() {
                frame["error"].clone()
            } else {
                frame["result"].clone()
            }
        })
        .collect()
}

#[test]
fn the_plan_tool_answers_what_the_tree_holds_and_what_it_leaves_out() {
    let f = fixture();
    let answers = mcp(
        &f,
        &[
            ("majordomus_pack_plan", json!({ "profile": "chatgpt" })),
            ("majordomus_pack_plan", json!({ "profile": "tight" })),
            (
                "majordomus_pack_plan",
                json!({ "profile": "no-such-profile" }),
            ),
        ],
    );
    let clean = &answers[0]["structuredContent"];
    assert_eq!(clean["measured"], true, "{clean}");
    assert_eq!(clean["passes"], true, "{clean}");
    assert_eq!(clean["profile"], "chatgpt");
    assert!(clean["selected"].as_u64().unwrap() > 0, "{clean}");
    let logo: Vec<&Value> = clean["dropped_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["path"] == "src/logo.png")
        .collect();
    assert_eq!(logo.len(), 1, "{clean}");
    assert_eq!(logo[0]["reason"], "binary");

    let tight = &answers[1]["structuredContent"];
    assert_eq!(tight["measured"], true, "{tight}");
    assert_eq!(tight["passes"], false, "{tight}");
    assert!(
        tight["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "pack.file_too_large"),
        "{tight}"
    );

    let unknown = &answers[2]["structuredContent"];
    assert_eq!(unknown["measured"], false, "{unknown}");
    assert_eq!(unknown["passes"], false, "not measured is never a pass");
    assert!(
        unknown["reason"]
            .as_str()
            .unwrap()
            .contains("no profile 'no-such-profile'"),
        "{unknown}"
    );
}

#[test]
fn the_verify_tool_reads_a_built_pack_and_refuses_one_that_was_changed() {
    let f = fixture();
    let (code, out, err) = pack(&f, &["build", "chatgpt", "--out", "tmp/packs/t"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("pack verify: clean"), "{out}");

    let shard = std::fs::read_dir(f.path("tmp/packs/t"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("01-"))
        .expect("a first shard");
    let clean = mcp(
        &f,
        &[("majordomus_pack_verify", json!({ "dir": "tmp/packs/t" }))],
    );
    let v = &clean[0]["structuredContent"];
    assert_eq!(v["passes"], true, "{v}");
    assert_eq!(v["profile"], "chatgpt");
    assert_eq!(
        v["dir"], "tmp/packs/t",
        "the answer names the directory as asked"
    );

    let mut text = std::fs::read_to_string(&shard).unwrap();
    text.push_str("tampered\n");
    std::fs::write(&shard, text).unwrap();
    let answers = mcp(
        &f,
        &[
            ("majordomus_pack_verify", json!({ "dir": "tmp/packs/t" })),
            (
                "majordomus_pack_verify",
                json!({ "dir": "tmp/packs/absent" }),
            ),
            ("majordomus_pack_verify", json!({ "dir": "../elsewhere" })),
        ],
    );
    let tampered = &answers[0]["structuredContent"];
    assert_eq!(tampered["measured"], true, "{tampered}");
    assert_eq!(tampered["passes"], false, "{tampered}");
    let absent = &answers[1]["structuredContent"];
    assert_eq!(absent["measured"], false, "{absent}");
    assert_eq!(absent["passes"], false, "{absent}");
    // a directory outside the repository is refused, not read
    let outside = answers[2].to_string();
    assert!(outside.contains("leaves the repository"), "{outside}");
    assert_ne!(answers[2]["structuredContent"]["passes"], true, "{outside}");
}

#[test]
fn the_command_line_renders_every_verdict_and_exits_by_it() {
    let f = fixture();

    let (code, out, err) = pack(&f, &["plan", "chatgpt"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("left out: 1 (binary)"), "{out}");
    assert!(out.contains("src/logo.png (binary)"), "{out}");
    assert!(out.contains("shards: 1 + index"), "{out}");
    assert!(out.ends_with("pack plan: clean\n"), "{out}");

    let (code, out, _) = pack(&f, &["plan", "chatgpt", "--format", "json"]);
    assert_eq!(code, 0);
    let plan: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(plan["passes"], true);

    let (code, out, _) = pack(&f, &["plan", "no-such-profile"]);
    assert_eq!(code, 12, "not measured is exit 12: {out}");
    assert!(
        out.starts_with("pack plan: not measured: no profile"),
        "{out}"
    );

    // the default profile declares no limits: measured, and refused for that
    let (code, out, _) = pack(&f, &["plan"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("FAIL  pack.no_limits  -  "), "{out}");
    // the remedy names `shards:`; what is absent is the line that counts shards
    assert!(!out.contains("      shards: "), "{out}");

    let (code, out, _) = pack(&f, &["plan", "tight"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("FAIL  pack.file_too_large  "), "{out}");
    assert!(out.contains("      remedy: "), "{out}");
    assert!(out.contains("finding(s)"), "{out}");

    // a plan with a finding is never built, in either shape
    let (code, out, _) = pack(&f, &["build", "tight", "--out", "tmp/packs/x"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("pack.file_too_large"), "{out}");
    let (code, out, _) = pack(
        &f,
        &["build", "tight", "--out", "tmp/packs/x", "--format", "json"],
    );
    assert_eq!(code, 10, "{out}");
    let refused: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(refused["plan"]["passes"], false, "{refused}");
    assert!(
        !f.path("tmp/packs/x").exists(),
        "a refused plan wrote a pack"
    );

    // the default destination is named for the repository, the profile and the commit
    let (code, out, err) = pack(&f, &["build", "chatgpt", "--format", "json"]);
    assert_eq!(code, 0, "{out}{err}");
    let built: Value = serde_json::from_str(&out).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    let want = format!("tmp/packs/repo-chatgpt-{}", &head.trim()[..12]);
    assert!(
        built["out"].as_str().unwrap().ends_with(&want),
        "{} does not end with {want}",
        built["out"]
    );
    assert_eq!(built["verdict"]["passes"], true, "{built}");
    let dir = built["out"].as_str().unwrap().to_string();

    let (code, out, err) = pack(&f, &["verify", &dir]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.ends_with("pack verify: clean\n"), "{out}");
    let (code, out, _) = pack(&f, &["verify", &dir, "--format", "json"]);
    assert_eq!(code, 0);
    assert_eq!(serde_json::from_str::<Value>(&out).unwrap()["passes"], true);

    // a second build refuses to overwrite without --force and replaces with it
    let (code, _, err) = pack(&f, &["build", "chatgpt"]);
    assert_ne!(code, 0);
    assert!(err.contains("already holds a pack"), "{err}");
    let (code, out, err) = pack(&f, &["build", "chatgpt", "--force"]);
    assert_eq!(code, 0, "{out}{err}");

    // a file dropped into the pack is a finding; a directory without a manifest is unmeasured
    std::fs::write(Path::new(&dir).join("99-stray.md"), "stray\n").unwrap();
    let (code, out, _) = pack(&f, &["verify", &dir]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("FAIL  pack.stray"), "{out}");
    assert!(out.contains("pack verify: "), "{out}");
    let (code, out, _) = pack(&f, &["verify", "tmp/packs/absent"]);
    assert_eq!(code, 12, "{out}");
    assert!(out.starts_with("pack verify: not measured: "), "{out}");

    // staged changes travel under a commit that does not hold them, and the plan says so
    f.write("src/main.rs", "fn main() { staged() }\n");
    f.git(&["add", "src/main.rs"]);
    let (code, out, _) = pack(&f, &["plan", "chatgpt"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("the index holds staged changes"), "{out}");
}

#[test]
fn the_library_plans_the_same_tree_the_tools_answer_about() {
    let f = fixture();
    let planned = majordomus_cli::pack::plan(&f.root(), &common::dist_share(), Some("chatgpt"));
    assert!(planned.plan.passes, "{:?}", planned.plan.findings);
    assert!(planned.plan.index_matches_head);
    let dropped: Vec<&str> = planned
        .plan
        .dropped_files
        .iter()
        .map(|d| d.path.as_str())
        .collect();
    let mut ordered = dropped.clone();
    majordomus_cli::order::canonical_strings(&mut ordered);
    assert_eq!(dropped, ordered, "the dropped files are in canonical order");
    assert!(dropped.contains(&"src/logo.png"), "{dropped:?}");
}

#[test]
fn the_command_line_refuses_what_it_cannot_answer() {
    let f = fixture();
    // a directory outside the repository is refused by the capability, not read
    let (code, out, err) = pack(&f, &["verify", "../elsewhere"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(err.contains("leaves the repository"), "{err}");

    // a destination that is not a pack is refused, and nothing in it is removed
    f.write("tmp/mine/keep.txt", "mine\n");
    let (code, _, err) = pack(&f, &["build", "chatgpt", "--out", "tmp/mine"]);
    assert_ne!(code, 0);
    assert!(err.contains("it is not a pack"), "{err}");
    assert!(f.path("tmp/mine/keep.txt").is_file());

    // profiles that cannot be read: the plan is unmeasured and the verifier refuses
    let (code, _, _) = pack(&f, &["build", "chatgpt", "--out", "tmp/packs/p"]);
    assert_eq!(code, 0);
    f.write(".ai/repo/archive.yaml", "profiles: 7\n");
    let (code, out, _) = pack(&f, &["plan", "chatgpt"]);
    assert_eq!(code, 12, "{out}");
    assert!(out.contains(".ai/repo/archive.yaml"), "{out}");
    let (code, out, err) = pack(&f, &["verify", "tmp/packs/p"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(err.contains(".ai/repo/archive.yaml"), "{err}");
}

#[test]
fn outside_a_repository_there_is_nothing_to_pack() {
    let plain = tempfile::tempdir().unwrap();
    let out = Command::new(common::BIN)
        .args(["pack", "plan"])
        .current_dir(plain.path())
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        out.stdout.is_empty(),
        "nothing is planned outside a repository"
    );
}
