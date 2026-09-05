//! A disposable repository shaped like the one `majordomus init` writes: the manifest,
//! the source classes, one policy, one profile, one prompt, one rule, one document.
//! Every test builds its own and never reads the developer's checkout.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

pub const BIN: &str = env!("CARGO_BIN_EXE_majordomus");

pub const MANIFEST: &str = "schema: ai-repository/v1

repo:
  path: repo

local:
  path: local
  tracked: false
  implicit_context: false

sections:
  policy: repo/policy.yaml
  profiles: repo/profiles
  rules: repo/rules
  prompts: repo/prompts
  skills: repo/skills
  workflows: repo/workflows
  knowledge: repo/knowledge
  adrs: repo/adrs
  project: repo/project
";

pub const SOURCES: &str = "version: 1

sources:
  - id: policy
    kind: policy
    discovery: vcs
    pathspec: ':(glob).ai/repo/policy.yaml'
    required: true

  - id: profile
    kind: profile
    discovery: vcs
    pathspec: ':(glob).ai/repo/profiles/*.yaml'
    required: true

  - id: rule
    kind: rule
    discovery: vcs
    pathspec: ':(glob).ai/repo/rules/**/*.md'
    required: true

  - id: prompt
    kind: prompt
    discovery: vcs
    pathspec: ':(glob).ai/repo/prompts/*.md'
    required: false

  - id: workflow
    kind: document
    discovery: vcs
    pathspec: ':(glob).ai/repo/workflows/*.md'
    required: false

  - id: milestone
    kind: milestone
    discovery: vcs
    pathspec: ':(glob).ai/repo/project/milestones/*.yaml'
    required: false

  - id: issue
    kind: issue
    discovery: vcs
    pathspec: ':(glob).ai/repo/project/issues/*.yaml'
    required: false

  - id: document
    kind: document
    discovery: vcs
    pathspec: ':(glob)docs/*.md'
    required: false

  - id: readme
    kind: document
    discovery: vcs
    pathspec: ':(glob)*.md'
    required: false
";

pub const POLICY: &str = "version: 1

context:
  always_loaded_budget_lines: 150
  strategy: minimum-sufficient

profiles:
  default: implementation

projections:
  - provider: agents
    target: AGENTS.md
    always_loaded: true
";

pub const PROFILE: &str = "name: implementation
description: build a described feature
capability: standard
effort: medium
verbosity: concise
presentation: engineering
checkpoint_interval: 15m
";

pub const PROMPT: &str = "---
name: continue
description: resume a task from durable state
---

# Resume

{{CONTEXT}}
";

/// A rule file with the given id, version and title.
pub fn rule(id: &str, version: u32, title: &str) -> String {
    format!(
        "---
id: {id}
version: {version}
kind: rule
title: {title}
description: {title}, in one sentence.
statement: {title}: the normative sentence.
status: active
class: advisory
depends_on: []
tags: [fixture]
---

# Rationale

Because the fixture says so.
"
    )
}

pub struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    /// A complete, valid, committed repository.
    pub fn new() -> Self {
        let f = Self::empty_git();
        f.write(".ai/manifest.yaml", MANIFEST);
        f.write(".ai/README.md", "# Repository AI context\n");
        f.write(".ai/repo/policy.yaml", POLICY);
        f.write(".ai/repo/profiles/implementation.yaml", PROFILE);
        f.write(".ai/repo/prompts/continue.md", PROMPT);
        f.write(
            ".ai/repo/rules/README.md",
            "# Repository rules\n\nHow to read one.\n",
        );
        f.write(
            ".ai/repo/rules/project/alpha.v1.md",
            &rule("project.alpha", 1, "Alpha"),
        );
        f.write(".ai/repo/knowledge/sources.yaml", SOURCES);
        f.write(
            ".ai/repo/workflows/task-lifecycle.md",
            "# The task lifecycle\n\nstart, check, finish.\n",
        );
        f.write(".ai/local/state/current.yaml", "id: t-1\n");
        f.write(".gitignore", ".ai/local/\n");
        f.write("README.md", "# Fixture\n\nRead AGENTS.md.\n");
        f.write("docs/CLI.md", "# CLI specification\n\nEvery command.\n");
        f.commit("install");
        f
    }

    /// A git repository with nothing in it.
    pub fn empty_git() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let f = Fixture { dir };
        f.git(&["init", "-q", "."]);
        f.git(&["config", "user.email", "t@example.com"]);
        f.git(&["config", "user.name", "t"]);
        f.git(&["commit", "-q", "--allow-empty", "-m", "init"]);
        f
    }

    /// A directory that is not a git repository at all.
    pub fn plain_dir() -> Self {
        Fixture {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    pub fn root(&self) -> PathBuf {
        self.dir.path().canonicalize().expect("canonical root")
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    pub fn write(&self, rel: &str, content: &str) {
        let p = self.path(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    pub fn write_bytes(&self, rel: &str, content: &[u8]) {
        let p = self.path(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    pub fn remove(&self, rel: &str) {
        std::fs::remove_file(self.path(rel)).unwrap();
    }

    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(self.dir.path())
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    pub fn commit(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    /// `git status --porcelain` plus a hash of every tracked file: the non-invasiveness witness.
    pub fn snapshot(&self) -> String {
        let status = self.git(&["status", "--porcelain"]);
        let files = self.git(&["ls-files", "-s"]);
        format!("{status}\n{files}")
    }
}

/// Run the binary with `args` in `cwd`, feeding `stdin`, and return (status, stdout, stderr).
pub fn run_in(cwd: &Path, args: &[&str], stdin: &str) -> (i32, String, String) {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(cwd)
        .env("MAJORDOMUS_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn majordomus");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).expect("stdout is UTF-8"),
        String::from_utf8(out.stderr).expect("stderr is UTF-8"),
    )
}

/// Run `mcp --inspect --format json` and parse it.
pub fn inspect(cwd: &Path, extra: &[&str]) -> (i32, Value, String) {
    let mut args = vec!["mcp", "--inspect", "--format", "json"];
    args.extend_from_slice(extra);
    let (code, out, err) = run_in(cwd, &args, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("inspect output is not JSON: {e}\n{out}\n{err}"));
    (code, v, err)
}

pub fn resource_uris(inspect: &Value) -> Vec<String> {
    inspect["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["uri"].as_str().unwrap().to_string())
        .collect()
}

pub fn diagnostics(inspect: &Value) -> Vec<Value> {
    inspect["repository"]["diagnostics"]
        .as_array()
        .unwrap()
        .clone()
}

// ---------------------------------------------------------------- HTTP helpers

use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;
use std::process::Child;

/// A running `majordomus serve --port 0` with the address it reported. Dropping it closes
/// the child's stdin, which is how the server is told to stop, and asserts a clean exit.
pub struct Served {
    pub child: Child,
    pub address: String,
}

impl Served {
    /// Spawn the server in `cwd` and wait for the "listening on" line on stderr.
    pub fn start(cwd: &Path, extra: &[&str]) -> Self {
        use std::process::Stdio;
        let mut args = vec!["serve", "--port", "0"];
        args.extend_from_slice(extra);
        let mut child = Command::new(BIN)
            .args(&args)
            .current_dir(cwd)
            .env("MAJORDOMUS_LOG", "info")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn majordomus serve");
        let stderr = child.stderr.take().unwrap();
        let mut lines = BufReader::new(stderr).lines();
        let mut address = None;
        for line in lines.by_ref() {
            let line = line.unwrap();
            if let Some(rest) = line.split("listening on http://").nth(1) {
                address = Some(rest.split_whitespace().next().unwrap().to_string());
                break;
            }
            if line.contains("majordomus:") {
                panic!("serve failed: {line}");
            }
        }
        // keep draining stderr so the child never blocks on a full pipe
        std::thread::spawn(move || for _ in lines {});
        Served {
            child,
            address: address.expect("serve reported its address"),
        }
    }

    /// One HTTP/1.1 request; returns (status, headers, body).
    pub fn request(
        &self,
        method: &str,
        target: &str,
        body: Option<&str>,
    ) -> (u16, Vec<(String, String)>, String) {
        let mut stream = TcpStream::connect(&self.address).expect("connect");
        let body = body.unwrap_or("");
        let req = format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            self.address,
            body.len()
        );
        use std::io::Write;
        stream.write_all(req.as_bytes()).unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        let text = String::from_utf8(raw).expect("response is UTF-8");
        let (head, body) = text.split_once("\r\n\r\n").expect("a header/body split");
        let mut lines = head.lines();
        let status: u16 = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let headers = lines
            .filter_map(|l| {
                l.split_once(": ")
                    .map(|(k, v)| (k.to_lowercase(), v.to_string()))
            })
            .collect();
        (status, headers, body.to_string())
    }

    pub fn get(&self, target: &str) -> (u16, Value) {
        let (status, _, body) = self.request("GET", target, None);
        let v = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("GET {target}: body is not JSON ({e}): {body}"));
        (status, v)
    }
}

impl Served {
    /// Close stdin and wait: the documented way to stop the server.
    pub fn stop(&mut self) -> i32 {
        drop(self.child.stdin.take());
        let status = self.child.wait().expect("wait for serve");
        status.code().unwrap_or(-1)
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        if self.child.stdin.is_some() {
            let code = self.stop();
            assert_eq!(code, 0, "serve did not end cleanly when stdin closed");
        }
    }
}

/// Everything a `$ref` in an OpenAPI document points at must exist under components.
pub fn openapi_refs_resolve(doc: &Value) -> Vec<String> {
    let mut missing = Vec::new();
    fn walk(v: &Value, doc: &Value, missing: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                if let Some(Value::String(r)) = m.get("$ref") {
                    let name = r.strip_prefix("#/components/schemas/").unwrap_or("");
                    if name.is_empty() || doc["components"]["schemas"].get(name).is_none() {
                        missing.push(r.clone());
                    }
                }
                m.values().for_each(|c| walk(c, doc, missing));
            }
            Value::Array(a) => a.iter().for_each(|c| walk(c, doc, missing)),
            _ => {}
        }
    }
    walk(doc, doc, &mut missing);
    missing
}

/// A fixture as a library-level index and registry, for tests of the projections that
/// need no process boundary.
pub fn load_app(f: &Fixture) -> majordomus_cli::app::App {
    let args = majordomus_cli::cli::RepoArgs {
        repo: Some(f.root()),
        ..Default::default()
    };
    majordomus_cli::app::App::load(&args).expect("app loads")
}
