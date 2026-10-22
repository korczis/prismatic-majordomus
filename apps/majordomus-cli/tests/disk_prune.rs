//! `disk.prune` answers on every surface (I3833): over MCP an idle build directory is
//! removed and a locked one kept, and on the command line a prune is a check unless
//! `--apply` is said.

mod common;

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use common::Fixture;
use serde_json::{json, Value};

/// A build directory under `root` with a file and both cargo locks, every entry aged two hours.
fn idle_build_dir(root: &Path, name: &str) -> PathBuf {
    let d = root.join(name);
    fs::create_dir_all(d.join("debug/deps")).unwrap();
    fs::write(d.join("debug/deps/libx.rlib"), vec![0u8; 4096]).unwrap();
    fs::write(d.join("debug/.cargo-lock"), b"").unwrap();
    fs::write(d.join("debug/.cargo-build-lock"), b"").unwrap();
    let then = SystemTime::now() - Duration::from_secs(2 * 3600);
    for p in [
        d.join("debug/deps/libx.rlib"),
        d.join("debug/.cargo-lock"),
        d.join("debug/.cargo-build-lock"),
        d.join("debug/deps"),
        d.join("debug"),
    ] {
        File::open(&p).and_then(|f| f.set_modified(then)).unwrap();
    }
    d
}

/// One `tools/call` of the MCP server in the fixture; the structured result.
fn call(f: &Fixture, tool: &str, arguments: Value) -> Value {
    let mut child = common::majordomus()
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": tool, "arguments": arguments } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    frames[1]["result"]["structuredContent"].clone()
}

/// The command line in `cwd` with extra environment; exit code, stdout, stderr.
fn run_with(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = common::majordomus();
    cmd.args(args)
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .current_dir(cwd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn over_mcp_an_idle_directory_is_removed_and_a_locked_one_kept() {
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("build-private");
    let idle = idle_build_dir(&root, "idle");
    let busy = idle_build_dir(&root, "busy");
    // a running cargo holds the build lock of `busy`
    let holder = File::options()
        .read(true)
        .write(true)
        .open(busy.join("debug/.cargo-build-lock"))
        .unwrap();
    // SAFETY: the test owns `holder` for the whole call
    assert_eq!(
        unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );

    let r = call(
        &f,
        "majordomus_disk_prune",
        json!({ "idle_minutes": 30, "roots": [root.display().to_string()] }),
    );
    let by = |p: &Path| {
        r["build_dirs"]["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["path"] == p.display().to_string())
            .cloned()
            .unwrap_or(Value::Null)
    };
    assert_eq!(by(&idle)["removed"], json!(true), "{r}");
    assert_eq!(by(&busy)["kept"], json!("locked"), "{r}");
    assert!(!idle.exists(), "the idle directory is still there");
    assert!(busy.exists(), "a directory in use was removed");
    assert!(r["build_dirs"]["reclaimed_bytes"].as_u64().unwrap() >= 4096);
    drop(holder);
}

#[test]
fn on_the_command_line_a_prune_is_a_check_unless_apply_is_said() {
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let idle = idle_build_dir(&home.path().join("build-private"), "idle");
    let cargo_home = home.path().display().to_string();

    let (code, out, err) = run_with(
        &f.root(),
        &["disk", "prune", "--format", "json"],
        &[("CARGO_HOME", cargo_home.as_str())],
    );
    assert_eq!(code, 0, "{err}");
    let r: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(r["build_dirs"]["check"], json!(true));
    assert_eq!(
        r["build_dirs"]["candidates"][0]["kept"],
        json!("check"),
        "{r}"
    );
    assert!(idle.exists(), "a check removed a directory");

    let (code, out, err) = run_with(
        &f.root(),
        &["disk", "prune", "--apply"],
        &[("CARGO_HOME", cargo_home.as_str())],
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("removed"), "{out}");
    assert!(!idle.exists(), "--apply did not remove the idle directory");
}

#[test]
fn a_container_check_reads_the_runtime_and_runs_nothing_else() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let calls = home.path().join("calls");
    let docker = bin.join("docker");
    fs::write(
        &docker,
        format!(
            "#!/bin/sh\necho \"docker $*\" >> '{}'\n\
             [ \"$1\" = system ] && echo '{{\"Type\":\"Build Cache\",\"Reclaimable\":\"3GB\"}}'\n\
             exit 0\n",
            calls.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&docker, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let h = home.path().display().to_string();

    let (code, out, err) = run_with(
        &f.root(),
        &["disk", "prune", "--containers", "--format", "json"],
        &[
            ("PATH", path.as_str()),
            ("CARGO_HOME", h.as_str()),
            ("COLIMA_HOME", h.as_str()),
        ],
    );
    assert_eq!(code, 0, "{err}");
    let r: Value = serde_json::from_str(&out).unwrap();
    let c = &r["containers"];
    assert_eq!(c["runtime"], json!("docker"), "{r}");
    assert_eq!(c["running"], json!(true));
    assert_eq!(c["check"], json!(true));
    assert_eq!(c["reclaimable"][0]["reclaimable"], json!("3GB"));
    assert_eq!(c["steps"], json!([]));
    let ran = fs::read_to_string(&calls).unwrap();
    assert_eq!(
        ran.lines().count(),
        1,
        "a check ran more than the read: {ran}"
    );
}
