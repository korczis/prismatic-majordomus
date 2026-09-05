//! The runners: direct through the executor, HTTP over a real loopback socket served by
//! this process, MCP through a real `majordomus mcp --standalone` child on stdio. One
//! canonical case is serialised by the actual transport adapters (the query string of a
//! `GET`, the JSON body of a `POST`, a `tools/call` frame), so a benchmark exercises the
//! adapter it claims to. Cold and warm cache modes are measured for a cached capability;
//! a fresh process is spawned per sample for the process-cold system target.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::capability::{CachePolicy, Context};
use crate::error::{Error, Result};
use crate::http::server;
use crate::http::Router;
use crate::mcp::bridge;
use crate::perf::COUNTERS;

use super::projection::{BenchmarkTarget, TargetKind, Transport};
use super::results::{BenchmarkResult, CacheMode};
use super::stats::Statistics;
use super::system::SystemTarget;

/// How much to measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    /// The name (`quick`, `full`, `ci`).
    pub name: &'static str,
    /// Calls before the samples.
    pub warmup: usize,
    /// Samples per target and cache mode.
    pub samples: usize,
    /// Processes spawned for the process-cold target.
    pub cold_spawns: usize,
}

impl Profile {
    /// Fast developer feedback.
    pub const QUICK: Profile = Profile {
        name: "quick",
        warmup: 3,
        samples: 20,
        cold_spawns: 2,
    };
    /// Stable evidence.
    pub const FULL: Profile = Profile {
        name: "full",
        warmup: 10,
        samples: 200,
        cold_spawns: 10,
    };
    /// Structural gates plus a conservative measurement.
    pub const CI: Profile = Profile {
        name: "ci",
        warmup: 5,
        samples: 50,
        cold_spawns: 3,
    };

    /// By name.
    pub fn parse(name: &str) -> Option<Profile> {
        match name {
            "quick" => Some(Profile::QUICK),
            "full" => Some(Profile::FULL),
            "ci" => Some(Profile::CI),
            _ => None,
        }
    }
}

/// Times targets against one context.
pub struct Runner {
    ctx: Arc<Context>,
    profile: Profile,
    http: Option<server::Running>,
    mcp: Option<McpChild>,
    /// The executable to spawn for MCP: this one, unless a test says otherwise.
    executable: std::path::PathBuf,
    repo_root: std::path::PathBuf,
    /// The share directory the child reads kinds and schemas from.
    share: Option<std::path::PathBuf>,
    /// Extra arguments for the child (`--discovery filesystem`, `--strict`).
    child_args: Vec<String>,
}

impl Runner {
    /// A runner over a context; the HTTP socket and the MCP child are started on first use.
    pub fn new(ctx: Arc<Context>, profile: Profile, repo_root: &std::path::Path) -> Self {
        Runner {
            ctx,
            profile,
            http: None,
            mcp: None,
            executable: std::env::current_exe().unwrap_or_else(|_| "majordomus".into()),
            repo_root: repo_root.to_path_buf(),
            share: None,
            child_args: Vec::new(),
        }
    }

    /// Spawn this executable for the MCP transport instead of the running one.
    pub fn with_executable(mut self, path: std::path::PathBuf) -> Self {
        self.executable = path;
        self
    }

    /// The share directory the MCP child reads (`MAJORDOMUS_SHARE`); the parent's.
    pub fn with_share(mut self, share: std::path::PathBuf) -> Self {
        self.share = Some(share);
        self
    }

    /// Arguments the MCP child gets after `mcp --standalone`, so that it reads the
    /// repository the way the parent did (`--discovery filesystem`, `--strict`).
    pub fn with_child_args(mut self, args: Vec<String>) -> Self {
        self.child_args = args;
        self
    }

    /// Time one target: one result per cache mode it has.
    pub fn run(&mut self, target: &BenchmarkTarget) -> Result<Vec<BenchmarkResult>> {
        match &target.kind {
            TargetKind::Capability {
                id,
                transport,
                input,
                cache,
                tool,
                route,
                ..
            } => {
                let modes: Vec<CacheMode> = if cache.is_enabled() {
                    vec![CacheMode::Cold, CacheMode::Warm]
                } else {
                    vec![CacheMode::Uncached]
                };
                let mut out = Vec::new();
                for mode in modes {
                    let (stats, invocations) = match transport {
                        Transport::Direct => self.direct(id, input, mode, *cache)?,
                        Transport::Http => {
                            let (method, path) = route.clone().ok_or_else(|| Error::Protocol {
                                reason: format!("{id}: HTTP target without a route"),
                            })?;
                            (self.http(&method, &path, input, mode)?, None)
                        }
                        Transport::Mcp => {
                            let tool = tool.clone().ok_or_else(|| Error::Protocol {
                                reason: format!("{id}: MCP target without a tool"),
                            })?;
                            (self.mcp_tool(&tool, input, mode)?, None)
                        }
                    };
                    out.push(BenchmarkResult {
                        key: target.key.clone(),
                        kind: target.kind.clone(),
                        cache_mode: mode,
                        stats,
                        handler_invocations: invocations,
                    });
                }
                Ok(out)
            }
            TargetKind::System { target: system } => Ok(vec![BenchmarkResult {
                key: target.key.clone(),
                kind: target.kind.clone(),
                cache_mode: CacheMode::NotApplicable,
                stats: self.system(*system)?,
                handler_invocations: None,
            }]),
        }
    }

    /// The executor, in process. Cold clears the cache before every sample.
    fn direct(
        &mut self,
        id: &str,
        input: &Value,
        mode: CacheMode,
        _cache: CachePolicy,
    ) -> Result<(Statistics, Option<u64>)> {
        let ctx = Arc::clone(&self.ctx);
        // a command needs a caller: the runner is a peer of its own board
        let ctx = match ctx.registry.get(id).map(|c| c.kind) {
            Some(crate::capability::CapabilityKind::Command) => {
                Arc::new(ctx.for_caller(ctx.peers.attach(crate::peers::Transport::Stdio)))
            }
            _ => ctx,
        };
        let call = |ctx: &Context| -> Result<()> {
            if mode == CacheMode::Cold {
                ctx.executor.clear();
            }
            ctx.execute(id, input.clone())
                .map_err(|e| Error::Protocol {
                    reason: format!("{id}: {e}"),
                })?;
            Ok(())
        };
        for _ in 0..self.profile.warmup {
            call(&ctx)?;
        }
        let before = COUNTERS
            .handler_invocations
            .load(std::sync::atomic::Ordering::Relaxed);
        let mut samples = Vec::with_capacity(self.profile.samples);
        for _ in 0..self.profile.samples {
            if mode == CacheMode::Cold {
                ctx.executor.clear();
            }
            let t = Instant::now();
            ctx.execute(id, input.clone())
                .map_err(|e| Error::Protocol {
                    reason: format!("{id}: {e}"),
                })?;
            samples.push(t.elapsed());
        }
        let after = COUNTERS
            .handler_invocations
            .load(std::sync::atomic::Ordering::Relaxed);
        Ok((Statistics::of(&samples), Some(after - before)))
    }

    fn http_server(&mut self) -> Result<String> {
        if self.http.is_none() {
            let bound = server::bind("127.0.0.1", 0)?;
            let router = Router::new(Arc::clone(&self.ctx), crate::VERSION);
            self.http = Some(bound.start(router));
        }
        Ok(self.http.as_ref().map(|r| r.url()).unwrap_or_default())
    }

    /// A real request over the loopback socket, the input bound as the route binds it.
    fn http(
        &mut self,
        method: &str,
        path: &str,
        input: &Value,
        mode: CacheMode,
    ) -> Result<Statistics> {
        let url = self.http_server()?;
        let (target, body) = match method {
            "GET" => (format!("{path}{}", query_string(input)), None),
            _ => (path.to_string(), Some(input.to_string())),
        };
        let ctx = Arc::clone(&self.ctx);
        let once = || -> Result<Duration> {
            if mode == CacheMode::Cold {
                ctx.executor.clear();
            }
            let t = Instant::now();
            let reply = bridge::request(
                &url,
                method,
                &target,
                &[],
                body.as_deref(),
                Duration::from_secs(30),
            )
            .map_err(|e| Error::Http {
                reason: format!("{method} {target}: {e}"),
            })?;
            let elapsed = t.elapsed();
            if reply.status >= 500 {
                return Err(Error::Http {
                    reason: format!("{method} {target}: status {}", reply.status),
                });
            }
            Ok(elapsed)
        };
        for _ in 0..self.profile.warmup {
            once()?;
        }
        let mut samples = Vec::with_capacity(self.profile.samples);
        for _ in 0..self.profile.samples {
            samples.push(once()?);
        }
        Ok(Statistics::of(&samples))
    }

    fn mcp_child(&mut self) -> Result<&mut McpChild> {
        if self.mcp.is_none() {
            let mut child = McpChild::spawn(
                &self.executable,
                &self.repo_root,
                self.share.as_deref(),
                &self.child_args,
            )?;
            child.initialize()?;
            self.mcp = Some(child);
        }
        Ok(self.mcp.as_mut().expect("spawned"))
    }

    /// A `tools/call` frame to a real child process. Cold mode is not observable from
    /// outside the process, so both modes measure a warm process; the difference is
    /// reported by the direct transport.
    fn mcp_tool(&mut self, tool: &str, input: &Value, _mode: CacheMode) -> Result<Statistics> {
        let profile = self.profile;
        let child = self.mcp_child()?;
        let frame = |id: u64| json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": input } });
        for _ in 0..profile.warmup {
            let id = child.next_id();
            child.round_trip(&frame(id))?;
        }
        let mut samples = Vec::with_capacity(profile.samples);
        for _ in 0..profile.samples {
            let f = frame(child.next_id());
            let t = Instant::now();
            let answer = child.round_trip(&f)?;
            samples.push(t.elapsed());
            if answer.get("error").is_some() {
                return Err(Error::Protocol {
                    reason: format!("{tool}: {}", answer["error"]),
                });
            }
        }
        Ok(Statistics::of(&samples))
    }

    fn system(&mut self, target: SystemTarget) -> Result<Statistics> {
        let profile = self.profile;
        match target {
            SystemTarget::McpProcessCold => {
                let mut samples = Vec::new();
                for _ in 0..profile.cold_spawns {
                    let t = Instant::now();
                    let mut child = McpChild::spawn(
                        &self.executable,
                        &self.repo_root,
                        self.share.as_deref(),
                        &self.child_args,
                    )?;
                    child.initialize()?;
                    let id = child.next_id();
                    child.round_trip(
                        &json!({ "jsonrpc": "2.0", "id": id, "method": "tools/list" }),
                    )?;
                    samples.push(t.elapsed());
                    child.close();
                }
                Ok(Statistics::of(&samples))
            }
            SystemTarget::McpInitialize
            | SystemTarget::McpPing
            | SystemTarget::McpToolsList
            | SystemTarget::McpResourcesList
            | SystemTarget::McpResourcesRead => {
                let first_resource = self
                    .ctx
                    .index
                    .objects
                    .first()
                    .map(|o| o.uri.clone())
                    .unwrap_or_else(|| "majordomus://repository".into());
                let child = self.mcp_child()?;
                let frame = |id: u64| match target {
                    SystemTarget::McpInitialize => {
                        json!({ "jsonrpc": "2.0", "id": id, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "majordomus-bench", "version": crate::VERSION } } })
                    }
                    SystemTarget::McpPing => {
                        json!({ "jsonrpc": "2.0", "id": id, "method": "ping" })
                    }
                    SystemTarget::McpToolsList => {
                        json!({ "jsonrpc": "2.0", "id": id, "method": "tools/list" })
                    }
                    SystemTarget::McpResourcesList => {
                        json!({ "jsonrpc": "2.0", "id": id, "method": "resources/list" })
                    }
                    _ => {
                        json!({ "jsonrpc": "2.0", "id": id, "method": "resources/read", "params": { "uri": first_resource } })
                    }
                };
                for _ in 0..profile.warmup {
                    let id = child.next_id();
                    child.round_trip(&frame(id))?;
                }
                let mut samples = Vec::with_capacity(profile.samples);
                for _ in 0..profile.samples {
                    let f = frame(child.next_id());
                    let t = Instant::now();
                    child.round_trip(&f)?;
                    samples.push(t.elapsed());
                }
                Ok(Statistics::of(&samples))
            }
            SystemTarget::HttpIndex | SystemTarget::HttpOpenApi | SystemTarget::HttpDocs => {
                let path = match target {
                    SystemTarget::HttpIndex => "/",
                    SystemTarget::HttpOpenApi => "/openapi.json",
                    _ => "/docs",
                };
                self.http("GET", path, &json!({}), CacheMode::NotApplicable)
            }
        }
    }

    /// Stop the socket and the child.
    pub fn finish(mut self) {
        if let Some(child) = self.mcp.take() {
            child.close();
        }
        if let Some(http) = self.http.take() {
            http.stop();
        }
    }
}

/// The query string a `GET` route binds: every top-level property, scalars as text.
fn query_string(input: &Value) -> String {
    let Some(map) = input.as_object() else {
        return String::new();
    };
    let pairs: Vec<String> = map
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| {
            let text = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("{}={}", percent_encode(k), percent_encode(&text))
        })
        .collect();
    if pairs.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs.join("&"))
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'/'
            | b':'
            | b'@' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A `majordomus mcp --standalone` child on real pipes.
struct McpChild {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next: u64,
}

impl McpChild {
    fn spawn(
        executable: &std::path::Path,
        repo_root: &std::path::Path,
        share: Option<&std::path::Path>,
        extra: &[String],
    ) -> Result<Self> {
        let mut command = Command::new(executable);
        command
            .args(["mcp", "--standalone"])
            .args(extra)
            .current_dir(repo_root)
            .env("MAJORDOMUS_LOG", "warn");
        if let Some(share) = share {
            command.env("MAJORDOMUS_SHARE", share);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                Error::Transport(std::io::Error::other(format!(
                    "spawn {}: {e}",
                    executable.display()
                )))
            })?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = BufReader::new(child.stdout.take().expect("piped"));
        Ok(McpChild {
            child,
            stdin,
            stdout,
            next: 0,
        })
    }

    fn next_id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn initialize(&mut self) -> Result<()> {
        let id = self.next_id();
        let answer = self.round_trip(&json!({ "jsonrpc": "2.0", "id": id, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "majordomus-bench", "version": crate::VERSION } } }))?;
        if answer.get("result").is_none() {
            return Err(Error::Protocol {
                reason: format!("initialize failed: {answer}"),
            });
        }
        writeln!(
            self.stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
        )
        .map_err(Error::Transport)?;
        Ok(())
    }

    fn round_trip(&mut self, frame: &Value) -> Result<Value> {
        writeln!(self.stdin, "{frame}").map_err(Error::Transport)?;
        self.stdin.flush().map_err(Error::Transport)?;
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line).map_err(Error::Transport)?;
        if n == 0 {
            return Err(Error::Protocol {
                reason: "the mcp child closed its stdout".into(),
            });
        }
        serde_json::from_str(&line).map_err(|e| Error::Protocol {
            reason: format!("not a frame: {e}: {line}"),
        })
    }

    fn close(mut self) {
        drop(self.stdin);
        let _ = self.child.wait();
    }
}
