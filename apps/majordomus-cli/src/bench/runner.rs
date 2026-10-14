//! The runners: direct through the executor, HTTP over a real loopback socket served by
//! this process, MCP through a real `majordomus mcp --standalone` child on stdio. One
//! canonical case is serialised by the actual transport adapters (the query string of a
//! `GET`, the JSON body of a `POST`, a `tools/call` frame), so a benchmark exercises the
//! adapter it claims to. Cold and warm cache modes are measured for a cached capability;
//! a fresh process is spawned per sample for the process-cold system target.
//!
//! The socket serves what `serve` serves: the Cockpit and the UI beside the API. It is
//! served twice when a target asks for it, once over the context the runner holds
//! (pinned, nothing re-read) and once over a live view of the same repository, the one a
//! shared server answers from, so that a page timed over the live view pays the
//! per-request generation check — the stamp of the watched git control files, and a
//! reload when they moved — that a pinned context never does.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::capability::{CachePolicy, CapabilityError, Context};
use crate::cli::{DiscoveryMode, RepoArgs};
use crate::error::{Error, Result};
use crate::http::server;
use crate::http::Router;
use crate::live::Live;
use crate::mcp::bridge;

use super::projection::{BenchmarkTarget, TargetKind, Transport, View};
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
    /// Processes spawned for the process-cold target, and samples per cache mode of a
    /// deferred panel: the targets whose one sample costs what a whole quick run does.
    pub cold_spawns: usize,
    /// Are the UI's deferred panels measured? Their sources (the integration queue, the
    /// delivery report) are the reason the UI defers them, and one sample of one of them
    /// takes longer than every other target together; only `full` admits them.
    pub deferred: bool,
}

impl Profile {
    /// Fast developer feedback.
    pub const QUICK: Profile = Profile {
        name: "quick",
        warmup: 3,
        samples: 20,
        cold_spawns: 2,
        deferred: false,
    };
    /// Stable evidence.
    pub const FULL: Profile = Profile {
        name: "full",
        warmup: 10,
        samples: 200,
        cold_spawns: 10,
        deferred: true,
    };
    /// Structural gates plus a conservative measurement.
    pub const CI: Profile = Profile {
        name: "ci",
        warmup: 5,
        samples: 50,
        cold_spawns: 3,
        deferred: false,
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

    /// Does this profile measure the target? Every target but a deferred panel, which only
    /// a profile that admits deferred panels does.
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkTarget, Profile, TargetKind, View};
    /// let panel = BenchmarkTarget {
    ///     key: "system.http.ui.panels.drain".into(),
    ///     kind: TargetKind::Page {
    ///         route: "/ui/panels/drain".into(),
    ///         view: View::Pinned,
    ///         deferred: true,
    ///     },
    /// };
    /// assert!(!Profile::QUICK.admits(&panel) && !Profile::CI.admits(&panel));
    /// assert!(Profile::FULL.admits(&panel));
    /// ```
    pub fn admits(&self, target: &BenchmarkTarget) -> bool {
        self.deferred || !target.is_deferred()
    }
}

/// How long one page request may take before the run fails: a screen, and a deferred
/// panel, whose source is the slow one by definition (a cold integration queue has taken
/// 158 s on a loaded machine).
const PAGE_TIMEOUT: Duration = Duration::from_secs(120);
const DEFERRED_TIMEOUT: Duration = Duration::from_secs(900);

/// Times targets against one context.
pub struct Runner {
    ctx: Arc<Context>,
    profile: Profile,
    http: Option<server::Running>,
    /// The second socket, over the live view, started on the first target that needs it.
    live_http: Option<(Arc<Live>, server::Running)>,
    /// How the live view re-reads the repository when git moves it; derived from the
    /// context when not given.
    repo_args: Option<RepoArgs>,
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
            live_http: None,
            repo_args: None,
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

    /// How the live view reloads the repository: the arguments the process read it with,
    /// so that a reload discovers the same root, distribution and discovery mode.
    ///
    /// ```
    /// use majordomus_cli::bench::{BenchmarkProjection, Profile, Runner};
    /// use majordomus_cli::cli::RepoArgs;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let projection = BenchmarkProjection::from_context(&ctx);
    /// let live = projection
    ///     .targets
    ///     .iter()
    ///     .find(|t| t.key == "system.http.live.index")
    ///     .unwrap();
    /// let tiny = Profile { name: "doc", warmup: 0, samples: 2, cold_spawns: 1, deferred: false };
    /// let args = RepoArgs {
    ///     repo: Some(repo.root().to_path_buf()),
    ///     ..RepoArgs::default()
    /// };
    /// let mut runner = Runner::new(ctx, tiny, repo.root()).with_repo_args(args);
    /// // the live socket is served over a view started from these arguments
    /// assert_eq!(runner.run(live).unwrap()[0].stats.samples, 2);
    /// runner.finish();
    /// ```
    pub fn with_repo_args(mut self, args: RepoArgs) -> Self {
        self.repo_args = Some(args);
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
            TargetKind::Page {
                route,
                view,
                deferred,
            } => {
                let mut out = Vec::new();
                for mode in [CacheMode::Cold, CacheMode::Warm] {
                    out.push(BenchmarkResult {
                        key: target.key.clone(),
                        kind: target.kind.clone(),
                        cache_mode: mode,
                        stats: self.page(&target.key, route, *view, *deferred, mode)?,
                        handler_invocations: None,
                    });
                }
                Ok(out)
            }
        }
    }

    /// One page of the UI as a browser asks for it. Cold clears the capability cache of
    /// the context the page will be answered from before every sample, warm leaves it;
    /// most of a screen's sources are uncached, so the two differ by what is cached. A
    /// deferred panel takes `cold_spawns` samples after one warm-up, the budget of the
    /// other targets whose one sample is a whole run's worth.
    fn page(
        &mut self,
        key: &str,
        route: &str,
        view: View,
        deferred: bool,
        mode: CacheMode,
    ) -> Result<Statistics> {
        let (warmup, samples, timeout) = if deferred {
            (
                self.profile.warmup.min(1),
                self.profile.cold_spawns.max(1),
                DEFERRED_TIMEOUT,
            )
        } else {
            (self.profile.warmup, self.profile.samples, PAGE_TIMEOUT)
        };
        let headers = [("Accept", "text/html")];
        match view {
            View::Pinned => {
                let url = self.http_server()?;
                let ctx = Arc::clone(&self.ctx);
                let clear = move || {
                    if mode == CacheMode::Cold {
                        ctx.executor.clear();
                    }
                };
                time_requests(
                    &url, "GET", route, &headers, None, timeout, warmup, samples, clear,
                )
            }
            View::Live => {
                let url = self.live_server()?;
                let live = self.live_http.as_ref().map(|(l, _)| Arc::clone(l));
                let before = live.as_ref().map(|l| l.generation());
                let clear = {
                    let live = live.clone();
                    move || {
                        if mode == CacheMode::Cold {
                            if let Some(live) = &live {
                                // the context the next request will be answered from; taking
                                // it stamps the repository, outside the timed request
                                live.current().executor.clear();
                            }
                        }
                    }
                };
                let stats = time_requests(
                    &url, "GET", route, &headers, None, timeout, warmup, samples, clear,
                )?;
                if let (Some(live), Some(before)) = (live, before) {
                    let reloads = live.generation() - before;
                    if reloads > 0 {
                        // a reload inside the samples is part of what serving costs, and a
                        // reader of an outlier must be able to tell it from the code
                        tracing::warn!(
                            target = key,
                            reloads,
                            "the repository moved during the samples: the live view reloaded"
                        );
                    }
                }
                Ok(stats)
            }
        }
    }

    /// The socket over the live view, as `serve` builds it: the Cockpit and the UI over a
    /// [`Live`] that follows the repository the runner's context was read from.
    fn live_server(&mut self) -> Result<String> {
        if self.live_http.is_none() {
            let args = self.repo_args.clone().unwrap_or_else(|| RepoArgs {
                repo: Some(self.repo_root.clone()),
                discovery: if self.ctx.index.repository.discovery == "filesystem" {
                    DiscoveryMode::Filesystem
                } else {
                    DiscoveryMode::Vcs
                },
                strict: false,
                share: self.share.clone(),
            });
            let live = Arc::new(Live::watching(args, Arc::clone(&self.ctx)));
            let bound = server::bind("127.0.0.1", 0)?;
            let router = Router::new(Arc::clone(&live), crate::VERSION)
                .with_cockpit(self.share.as_deref())
                .with_ui(self.share.as_deref());
            self.live_http = Some((live, bound.start(router)));
        }
        Ok(self
            .live_http
            .as_ref()
            .map(|(_, r)| r.url())
            .unwrap_or_default())
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
        // A refusal is an answer, and answering is the work a benchmark exists to measure:
        // `objects.get` on a URI the layer does not hold, or `deploy.get` on a repository
        // with no deployment, does the lookup and then declines, and a case declared to
        // time that path is timing something real. Only `Internal` means the capability
        // failed, and only then is the sample meaningless.
        //
        // The other two transports already draw the line here. The HTTP runner treats
        // `status >= 500` as fatal, which is exactly `Internal` after the router's mapping
        // (404 not_found, 422 refused, 500 internal); the MCP runner fails on a JSON-RPC
        // `error`, and the surface answers a refusal as a *result* carrying `isError`,
        // reserving `error` for `Internal`. This path was the one that disagreed, so the
        // same capability timed three ways gave three verdicts on one event.
        let fatal = |id: &str, e: CapabilityError| -> Option<Error> {
            match e {
                CapabilityError::Internal(reason) => Some(Error::Protocol {
                    reason: format!("{id}: {reason}"),
                }),
                _ => None,
            }
        };
        let call = |ctx: &Context| -> Result<()> {
            if mode == CacheMode::Cold {
                ctx.executor.clear();
            }
            if let Err(e) = ctx.execute(id, input.clone()) {
                if let Some(fatal) = fatal(id, e) {
                    return Err(fatal);
                }
            }
            Ok(())
        };
        for _ in 0..self.profile.warmup {
            call(&ctx)?;
        }
        let before = ctx.executor.handler_invocations();
        let mut samples = Vec::with_capacity(self.profile.samples);
        for _ in 0..self.profile.samples {
            if mode == CacheMode::Cold {
                ctx.executor.clear();
            }
            let t = Instant::now();
            let answered = ctx.execute(id, input.clone());
            samples.push(t.elapsed());
            if let Err(e) = answered {
                if let Some(fatal) = fatal(id, e) {
                    return Err(fatal);
                }
            }
        }
        let after = ctx.executor.handler_invocations();
        Ok((Statistics::of(&samples), Some(after - before)))
    }

    fn http_server(&mut self) -> Result<String> {
        if self.http.is_none() {
            let bound = server::bind("127.0.0.1", 0)?;
            // with the Cockpit and the UI, so that the pages measured here are the pages
            // served: the share directory is the one the runner was given, and without one
            // the pages render unstyled markup, which is the same work
            let router = Router::new(Arc::clone(&self.ctx), crate::VERSION)
                .with_cockpit(self.share.as_deref())
                .with_ui(self.share.as_deref());
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
        self.http_with(method, path, input, mode, &[])
    }

    fn http_with(
        &mut self,
        method: &str,
        path: &str,
        input: &Value,
        mode: CacheMode,
        headers: &[(&str, &str)],
    ) -> Result<Statistics> {
        let url = self.http_server()?;
        let (target, body) = match method {
            "GET" => (format!("{path}{}", query_string(input)), None),
            _ => (path.to_string(), Some(input.to_string())),
        };
        let ctx = Arc::clone(&self.ctx);
        let clear = move || {
            if mode == CacheMode::Cold {
                ctx.executor.clear();
            }
        };
        time_requests(
            &url,
            method,
            &target,
            headers,
            body.as_deref(),
            Duration::from_secs(30),
            self.profile.warmup,
            self.profile.samples,
            clear,
        )
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
            SystemTarget::HttpIndex
            | SystemTarget::HttpHome
            | SystemTarget::HttpOpenApi
            | SystemTarget::HttpSwagger
            | SystemTarget::HttpCockpitOverview
            | SystemTarget::HttpCockpitCapabilities
            | SystemTarget::HttpCockpitGraph
            | SystemTarget::HttpIndexLive => {
                // the mounts come from the surfaces that declare them, so a route that
                // moves moves its benchmark with it
                let path = match target {
                    SystemTarget::HttpIndex
                    | SystemTarget::HttpHome
                    | SystemTarget::HttpIndexLive => "/",
                    SystemTarget::HttpOpenApi => crate::http::swagger::SPEC_PATH,
                    SystemTarget::HttpCockpitOverview => crate::cockpit::PREFIX,
                    SystemTarget::HttpCockpitCapabilities => "/cockpit/capabilities",
                    SystemTarget::HttpCockpitGraph => "/cockpit/graphs/registry",
                    _ => crate::http::swagger::SWAGGER_PATH,
                };
                // the home page is the same route as the index and a different answer:
                // only an explicit text/html asks for the rendered page
                let headers: &[(&str, &str)] = match target {
                    SystemTarget::HttpHome => &[("Accept", "text/html")],
                    _ => &[],
                };
                if target == SystemTarget::HttpIndexLive {
                    let url = self.live_server()?;
                    return time_requests(
                        &url,
                        "GET",
                        path,
                        headers,
                        None,
                        Duration::from_secs(30),
                        profile.warmup,
                        profile.samples,
                        || {},
                    );
                }
                self.http_with("GET", path, &json!({}), CacheMode::NotApplicable, headers)
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
        if let Some((_, http)) = self.live_http.take() {
            http.stop();
        }
    }
}

/// `warmup` requests, then `samples` timed ones, over a fresh connection each, with
/// `before` run ahead of every request and outside its time. A status of 500 or more
/// fails the run: a refusal is an answer and is timed, an internal failure is not.
#[allow(clippy::too_many_arguments)]
fn time_requests(
    url: &str,
    method: &str,
    target: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    timeout: Duration,
    warmup: usize,
    samples: usize,
    before: impl Fn(),
) -> Result<Statistics> {
    let once = || -> Result<Duration> {
        before();
        let t = Instant::now();
        let reply = bridge::request(url, method, target, headers, body, timeout).map_err(|e| {
            Error::Http {
                reason: format!("{method} {target}: {e}"),
            }
        })?;
        let elapsed = t.elapsed();
        if reply.status >= 500 {
            return Err(Error::Http {
                reason: format!("{method} {target}: status {}", reply.status),
            });
        }
        Ok(elapsed)
    };
    for _ in 0..warmup {
        once()?;
    }
    let mut taken = Vec::with_capacity(samples);
    for _ in 0..samples {
        taken.push(once()?);
    }
    Ok(Statistics::of(&taken))
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
