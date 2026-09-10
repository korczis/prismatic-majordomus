//! A disposable repository shaped like the one `majordomus init` writes: the manifest,
//! the source classes, one policy, one profile, one prompt, one rule, one document.
//! Every test builds its own and never reads the developer's checkout.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

pub const BIN: &str = env!("CARGO_BIN_EXE_majordomus");

/// The tool distribution beside this crate: `../../share`, with kinds.yaml and schemas/.
pub fn dist_share() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../share")
        .canonicalize()
        .expect("share beside the crate")
}

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
  why: repo/why
  deployments: repo/deployments

context:
  documents: [README.md]
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

  - id: curated
    kind: knowledge
    discovery: vcs
    pathspec: ':(glob).ai/repo/knowledge/curated/*.md'
    required: false

  - id: knowledge_baseline
    kind: knowledge-baseline
    discovery: vcs
    pathspec: ':(glob).ai/repo/knowledge/baseline.yaml'
    required: false

  - id: knowledge_exceptions
    kind: knowledge-exceptions
    discovery: vcs
    pathspec: ':(glob).ai/repo/knowledge/exceptions.yaml'
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

  - id: claims
    kind: claim
    discovery: vcs
    pathspec: ':(glob)docs/CLAIMS.yaml'
    required: false

  - id: moment
    kind: moment
    discovery: vcs
    pathspec: ':(glob).ai/repo/why/moments/*.md'
    required: false

  - id: audience
    kind: audience
    discovery: vcs
    pathspec: ':(glob).ai/repo/why/audiences/*.md'
    required: false

  - id: area
    kind: area
    discovery: vcs
    pathspec: ':(glob).ai/repo/why/areas/*.md'
    required: false

  - id: deployment
    kind: deployment
    discovery: vcs
    pathspec: ':(glob).ai/repo/deployments/*.yaml'
    required: false

  - id: claim_page
    kind: document
    discovery: vcs
    pathspec: ':(glob)docs/claims/*.md'
    required: false

  - id: library
    kind: implementation
    discovery: vcs
    pathspec: ':(glob)lib/*.sh'
    required: false

  - id: case
    kind: test
    discovery: vcs
    pathspec: ':(glob)test/cases/*.sh'
    required: false
";

/// A context document as the layer's READMEs carry it (`schema: context/v1`).
pub fn context_doc(id: &str, title: &str) -> String {
    format!(
        "---
schema: context/v1
id: {id}
kind: context
title: {title}
description: What this directory is for.
status: active
scope: subtree
providers: [\"*\"]
audience: [human, agent]
composition: extend
order: 100
---

# {title}

Read this before the files beside it.
"
    )
}

pub const CLAIMS: &str = "version: 1

statuses:
  - id: guaranteed
    meaning: Deterministic and blocking.
  - id: planned
    meaning: Specified and not implemented.

claims:
  - id: policy-parse
    claim: The policy is parsed and an unknown key is refused
    source: docs/SCHEMAS.md
    implementation: lib/a.sh
    test: test/cases/00_x.sh
    status: guaranteed
    responsibility: policy
    note: A restricted YAML subset.

  - id: routing
    claim: Routing recommendations will follow measurement
    source: docs/DESIGN.md
    implementation: '-'
    test: '-'
    status: planned
    responsibility: none
";

pub const POLICY: &str = "version: 1

context:
  always_loaded_budget_lines: 150
  strategy: minimum-sufficient

profiles:
  default: implementation
  checkpoint_interval_default: 15m

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

/// The fixture's own catalogue: one audience, one area and one moment that names both.
/// Small on purpose — the point is that the mechanism works on one file each, not that the
/// fixture has a rich catalogue.
pub const AUDIENCE: &str = "---
schema: audience/v1
id: fixture-team
kind: audience
title: The fixture's team
short_title: Fixture team
summary: 'A team that exists so the catalogue has somebody to belong to.'
status: stable
weight: 10
---

# The fixture's team

Because the fixture says so.
";

pub const AREA: &str = "---
schema: area/v1
id: fixture-area
kind: area
title: The fixture's area
summary: 'An operational area that exists so a moment has somewhere to fall.'
status: stable
weight: 10
---

# The fixture's area

Because the fixture says so.
";

/// A deployment the fixture declares, so that the capabilities reading one have an object
/// to read. Every required field and nothing else: this is the smallest thing the contract
/// calls a deployment, not a copy of the repository's own.
pub const DEPLOYMENT: &str = "# The fixture's deployment.
schema: deployment/v1
kind: deployment
id: fixture-deployment
title: The fixture's deployment
description: A deployment that exists so the capabilities reading one have an object to read.
status: declared

application: fixture

build:
  package: fixture-cli
  binary: fixture
  profile: release
  inputs:
    - apps/fixture-cli

listen:
  port: 8080
  interface: all

health:
  liveness: /api/v1/live
  readiness: /api/v1/ready
  grace_seconds: 2
  interval_seconds: 15
  timeout_seconds: 2

resources:
  cpu_kind: shared
  cpus: 1
  memory_mb: 256

machines:
  count: 1
  min_running: 0
  autostart: true
  autostop: true

region: fra

provider:
  name: fly
";

pub const MOMENT: &str = "---
schema: moment/v1
id: fixture-moment
kind: moment
title: 'The moment the fixture recognises'
hook: 'recognised the moment the fixture declares'
summary: 'A moment that exists so every projection has something to project.'
status: stable
severity: medium
frequency: common
weight: 10
featured: true
audiences: [fixture-team]
areas: [fixture-area]
tags: [fixture]
signals:
  - id: fixture-signal
    text: 'The fixture recognised its own moment.'
examples:
  - id: one
    audience: fixture-team
    title: 'The first situation'
    before: 'Nothing records it.'
    after: 'The catalogue does.'
  - id: two
    audience: fixture-team
    title: 'The second situation'
    before: 'Nothing records it either.'
    after: 'The catalogue does.'
  - id: three
    audience: fixture-team
    title: 'The third situation'
    before: 'Still nothing.'
    after: 'Still the catalogue.'
claims: [policy-parse]
---

## The moment

Because the fixture says so.

## Why it happens

Because the fixture says so.

## What it does not do

Nothing the fixture does not say.
";

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
            &context_doc("ai.repo.rules", "Repository rules"),
        );
        f.write(
            ".ai/repo/workflows/README.md",
            &context_doc("ai.repo.workflows", "Workflows"),
        );
        f.write("docs/CLAIMS.yaml", CLAIMS);
        f.write(".ai/repo/why/audiences/fixture-team.md", AUDIENCE);
        f.write(".ai/repo/why/areas/fixture-area.md", AREA);
        f.write(".ai/repo/why/moments/fixture-moment.md", MOMENT);
        f.write(".ai/repo/deployments/fixture-deployment.yaml", DEPLOYMENT);
        f.write(
            "docs/claims/policy-parse.md",
            "# The policy is parsed

## What it means

Parsed.
",
        );
        f.write(
            "lib/a.sh",
            "#!/usr/bin/env bash
# the one library file
echo a
",
        );
        f.write(
            "test/cases/00_x.sh",
            "# majordomus-covers: none
. \"$ROOT/test/lib.sh\"
true
",
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

    /// A brownfield repository: the complete fixture plus a component the knowledge
    /// system reads off a manifest, documentation that links to it and to nowhere, a
    /// curated record that contradicts the manifest, and one that must never leave the
    /// machine. Deterministic; what `scripts/knowledge-demo` builds for a person.
    pub fn brownfield() -> Self {
        let f = Self::new();
        f.write(
            "apps/alpha/Cargo.toml",
            "[package]\nname = \"alpha\"\nversion = \"1.2.3\"\nedition = \"2021\"\nlicense = \"MIT\"\n\n[dependencies]\nserde = \"1\"\n",
        );
        f.write(
            "docs/ALPHA.md",
            "# The alpha component\n\nThe crate lives under [apps/alpha](../apps/alpha/) and ships as one binary.\nIts operating notes are in [the runbook](runbooks/alpha.md), which nobody wrote.\n",
        );
        f.write(
            ".ai/repo/knowledge/curated/alpha-version.md",
            "---\nschema: knowledge/v1\nid: alpha-version\nkind: knowledge\nclass: fact\ntitle: alpha is at version 2.0.0\ndescription: The version the release notes were written against.\nstatus: verified\nepistemics: observed\ndate: 2026-01-01\nprovenance:\n  origin: authored\n  derived_from:\n    - file:apps/alpha/Cargo.toml\nasserts:\n  - subject: component:alpha\n    predicate: version\n    value: '2.0.0'\n---\n\n# alpha is at 2.0.0\n\nRead off the manifest when the notes were written.\n",
        );
        f.write(
            ".ai/repo/knowledge/curated/alpha-secret.md",
            "---\nschema: knowledge/v1\nid: alpha-secret\nkind: knowledge\nclass: constraint\ntitle: alpha talks to a partner system nobody outside may know about\ndescription: A constraint that stays with the checkout.\nstatus: candidate\nepistemics: decided\ndate: 2026-01-01\nvisibility: restricted\nprovenance:\n  origin: authored\n---\n\n# Restricted\n\nPARTNER-ZETA is the counterparty; the contract forbids naming it in public.\n",
        );
        f.commit("brownfield");
        f
    }

    /// A git repository with nothing in it.
    pub fn empty_git() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let f = Fixture { dir };
        std::fs::create_dir_all(f.repo()).expect("repository directory");
        f.git(&["init", "-q", "."]);
        f.git(&["config", "user.email", "t@example.com"]);
        f.git(&["config", "user.name", "t"]);
        f.git(&["commit", "-q", "--allow-empty", "-m", "init"]);
        f
    }

    /// A directory that is not a git repository at all.
    pub fn plain_dir() -> Self {
        let f = Fixture {
            dir: tempfile::tempdir().expect("tempdir"),
        };
        std::fs::create_dir_all(f.repo()).expect("repository directory");
        f
    }

    /// The repository root. It is a *subdirectory* of the temporary directory, not the
    /// temporary directory itself, so that anything a command creates beside the repository
    /// — the worktree container, which is `<root>-wt` by the topology — is still inside the
    /// temporary directory and is removed with it. A fixture rooted at the temporary
    /// directory would leak one container per test into the system temp space.
    pub fn root(&self) -> PathBuf {
        self.repo().canonicalize().expect("canonical root")
    }

    /// The repository root before canonicalisation, which is what has to be created.
    fn repo(&self) -> PathBuf {
        self.dir.path().join("repo")
    }

    /// The directory the repository sits in: where its worktree container goes.
    pub fn parent(&self) -> PathBuf {
        self.dir.path().canonicalize().expect("canonical parent")
    }

    /// The worktree container of this fixture, by the topology's derivation.
    pub fn container(&self) -> PathBuf {
        self.parent().join("repo-wt")
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.repo().join(rel)
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
            .arg(self.repo())
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
        .env("MAJORDOMUS_SHARE", dist_share())
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
            .env("MAJORDOMUS_SHARE", dist_share())
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
        self.request_with(method, target, body, &[])
    }

    /// The same, with further request headers: what a browser sends (`Origin`, `Accept`)
    /// and a plain client does not.
    pub fn request_with(
        &self,
        method: &str,
        target: &str,
        body: Option<&str>,
        extra: &[(&str, &str)],
    ) -> (u16, Vec<(String, String)>, String) {
        let mut stream = TcpStream::connect(&self.address).expect("connect");
        let body = body.unwrap_or("");
        let extra: String = extra.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
        let req = format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\n\r\n{body}",
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
        share: Some(dist_share()),
        ..Default::default()
    };
    majordomus_cli::app::App::load(&args).expect("app loads")
}

/// The kind schema of the distribution, loaded for a repository.
pub fn dist_schema(repo: &majordomus_cli::Repository) -> majordomus_cli::metadata::KindSchema {
    let share =
        majordomus_cli::share::Share::locate(Some(&dist_share()), repo.root()).expect("share");
    majordomus_cli::metadata::KindSchema::load(&share, repo).expect("kind schema")
}

/// The scope the repository declares, or the distribution's default beside this crate.
pub fn dist_scope(repo: &majordomus_cli::Repository) -> majordomus_cli::scope::Scope {
    let share =
        majordomus_cli::share::Share::locate(Some(&dist_share()), repo.root()).expect("share");
    majordomus_cli::scope::Scope::load(&share, repo).expect("scope")
}
