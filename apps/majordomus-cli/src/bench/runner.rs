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

use crate::capability::{CachePolicy, CapabilityError, Context};
use crate::error::{Error, Result};
use crate::http::server;
use crate::http::Router;
use crate::mcp::bridge;
use crate::perf::COUNTERS;

use super::projection::{BenchmarkTarget, TargetKind, Transport};
use super::results::{BenchmarkResult, CacheMode};
use super::stats::Statistics;
use super::system::SystemTarget;

/// How much to measure: the three questions a run can be asked.
///
/// Sample count is not a knob to be turned per invocation, because the regression policy
/// gates on percentiles and a percentile is only meaningful over enough samples — a p99 of
/// twenty samples is the slowest sample. So the counts are named profiles: `quick` is
/// developer feedback and is not evidence, `full` is evidence, and `ci` is the compromise a
/// gate can afford. The policy's own `minimum_samples` is what refuses to fail a run whose
/// profile was too small for a metric.
///
/// ```
/// use majordomus_cli::bench::Profile;
/// // the profile a person reaches for first is not the one that produces evidence
/// assert!(Profile::QUICK.samples < Profile::CI.samples);
/// assert!(Profile::CI.samples < Profile::FULL.samples);
/// // every profile warms up before it measures, so no sample is the first call
/// for profile in [Profile::QUICK, Profile::CI, Profile::FULL] {
///     assert!(profile.warmup > 0, "{}", profile.name);
///     assert!(profile.cold_spawns > 0, "{}", profile.name);
///     assert_eq!(Profile::parse(profile.name), Some(profile));
/// }
/// ```
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

    /// The profile of a name, or nothing.
    ///
    /// Where `--profile` becomes a value. There is deliberately no default here and no
    /// nearest match: a run whose sample count was decided by a typo would produce numbers
    /// that look like evidence and are not, and the caller is better placed to say what to
    /// do about an unknown name.
    ///
    /// ```
    /// use majordomus_cli::bench::Profile;
    /// assert_eq!(Profile::parse("quick"), Some(Profile::QUICK));
    /// assert_eq!(Profile::parse("full"), Some(Profile::FULL));
    /// assert_eq!(Profile::parse("ci"), Some(Profile::CI));
    /// // the name a profile carries is the name it parses from
    /// assert_eq!(Profile::parse(Profile::FULL.name), Some(Profile::FULL));
    /// assert_eq!(Profile::parse("thorough"), None, "no nearest match, no default");
    /// ```
    pub fn parse(name: &str) -> Option<Profile> {
        match name {
            "quick" => Some(Profile::QUICK),
            "full" => Some(Profile::FULL),
            "ci" => Some(Profile::CI),
            _ => None,
        }
    }
}

/// Times targets against one context, through the transport each target names.
///
/// It measures the real thing on every transport: the executor in process, a real loopback
/// socket for HTTP, and a real `majordomus mcp` child process over stdio. Nothing is
/// simulated, which is why a number from this can be compared with what a client
/// experiences — and why the socket and the child are started lazily and kept, rather than
/// per sample. A run that never touches a transport never pays for it.
///
/// It is not reusable across repositories: the context, the repository root and the child's
/// arguments are fixed when it is built, so every sample of a run measures the same thing.
///
/// ```no_run
/// use std::sync::Arc;
/// use majordomus_cli::bench::{BenchmarkProjection, Profile, Runner, Transport};
/// use majordomus_cli::capability::Context;
/// // compiled and not run: it starts a socket and a child process
/// fn measure(ctx: Arc<Context>, root: &std::path::Path) {
///     let projection = BenchmarkProjection::from_context(&ctx);
///     let mut runner = Runner::new(ctx, Profile::QUICK, root);
///     for target in projection.by_transport(Transport::Direct) {
///         let results = runner.run(target).expect("the direct transport is in process");
///         assert!(results.iter().all(|r| r.key == target.key));
///     }
///     runner.finish();
/// }
/// ```
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
    ///
    /// Lazily, and that is the contract: constructing a runner binds no port and spawns
    /// nothing, so a run narrowed to the direct transport costs neither. The executable the
    /// MCP child will be spawned from defaults to this one, which is what makes a
    /// benchmark measure the build that is running rather than whatever is on the `PATH`.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    ///
    /// // that this example runs at all is the laziness contract: constructing a runner
    /// // over a real context binds no port and spawns no child, so a doctest can do it
    /// let runner = Runner::new(Arc::clone(&ctx), Profile::CI, repo.root());
    /// assert_eq!(Arc::strong_count(&ctx), 2, "the runner holds the context, it does not copy it");
    ///
    /// runner.finish();
    /// assert_eq!(Arc::strong_count(&ctx), 1, "and finish released it; nothing was left running");
    /// ```
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
    ///
    /// The default is the running executable, so a benchmark measures the build in hand.
    /// This exists for the case where it cannot: a test binary is not a `majordomus`, so a
    /// test that benchmarks the MCP transport has to say which executable to spawn.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    ///
    /// // naming the executable spawns nothing: the child starts on first use, and this
    /// // example never reaches one
    /// let runner = Runner::new(Arc::clone(&ctx), Profile::QUICK, repo.root())
    ///     .with_executable(repo.root().join("target/release/majordomus"));
    /// assert_eq!(Arc::strong_count(&ctx), 2, "the builder returns the runner, not a new one");
    ///
    /// runner.finish();
    /// assert_eq!(Arc::strong_count(&ctx), 1, "no child was ever spawned to wait for");
    /// ```
    pub fn with_executable(mut self, path: std::path::PathBuf) -> Self {
        self.executable = path;
        self
    }

    /// The share directory the MCP child reads (`MAJORDOMUS_SHARE`); the parent's.
    ///
    /// The child has to read the same kinds and schemas the parent did, or it is a
    /// different program and its numbers answer a different question. Passing the parent's
    /// share explicitly is what stops the child inheriting whatever a stale environment
    /// variable happened to point at.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    ///
    /// let runner = Runner::new(Arc::clone(&ctx), Profile::QUICK, repo.root())
    ///     .with_share(repo.root().join("share"));
    /// assert_eq!(Arc::strong_count(&ctx), 2, "the builder returns the runner, not a new one");
    ///
    /// runner.finish();
    /// assert_eq!(Arc::strong_count(&ctx), 1, "the share is read by a child that never started");
    /// ```
    pub fn with_share(mut self, share: std::path::PathBuf) -> Self {
        self.share = Some(share);
        self
    }

    /// Arguments the MCP child gets after `mcp --standalone`, so that it reads the
    /// repository the way the parent did (`--discovery filesystem`, `--strict`).
    ///
    /// The same reason as the share directory: a child that discovered the repository
    /// differently is measuring a different index, and the difference would show up as a
    /// transport cost. These are appended to the child's command line, so they are the
    /// caller's to keep in step with the parent's own invocation.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    ///
    /// // the parent discovered the repository this way, so the child is told to as well
    /// let runner = Runner::new(Arc::clone(&ctx), Profile::QUICK, repo.root())
    ///     .with_child_args(vec!["--discovery".into(), "filesystem".into(), "--strict".into()]);
    /// assert_eq!(Arc::strong_count(&ctx), 2, "the builder returns the runner, not a new one");
    ///
    /// runner.finish();
    /// assert_eq!(Arc::strong_count(&ctx), 1, "the arguments configured a child that never started");
    /// ```
    pub fn with_child_args(mut self, args: Vec<String>) -> Self {
        self.child_args = args;
        self
    }

    /// Time one target: one result per cache mode it has.
    ///
    /// One target in, one result *per cache mode* out — a capability with a cache is
    /// measured cold and warm, because those are two different questions and comparing one
    /// against the other's baseline would report a regression that is not one. A target
    /// without a cache yields a single result.
    ///
    /// Every sample is preceded by the profile's warm-up, so no measurement is of a first
    /// call; the transport's own resources are started here on first use and kept until
    /// [`Runner::finish`].
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use majordomus_cli::bench::{BenchmarkProjection, Profile, Runner};
    /// use majordomus_cli::capability::Context;
    /// // compiled and not run: it makes real calls over real transports
    /// fn measure(ctx: Arc<Context>, root: &std::path::Path) {
    ///     let projection = BenchmarkProjection::from_context(&ctx);
    ///     let target = &projection.targets[0];
    ///     let mut runner = Runner::new(ctx, Profile::QUICK, root);
    ///     let results = runner.run(target).expect("the target was measured");
    ///     // every result is about the target that was asked for, one per cache mode
    ///     assert!(!results.is_empty());
    ///     assert!(results.iter().all(|r| r.key == target.key));
    ///     runner.finish();
    /// }
    /// ```
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
        let before = COUNTERS
            .handler_invocations
            .load(std::sync::atomic::Ordering::Relaxed);
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
        let after = COUNTERS
            .handler_invocations
            .load(std::sync::atomic::Ordering::Relaxed);
        Ok((Statistics::of(&samples), Some(after - before)))
    }

    fn http_server(&mut self) -> Result<String> {
        if self.http.is_none() {
            let bound = server::bind("127.0.0.1", 0)?;
            // with the Cockpit, so that the pages measured here are the pages served: the
            // share directory is the one the runner was given, and without one the Cockpit
            // renders unstyled markup, which is the same work
            let router = Router::new(Arc::clone(&self.ctx), crate::VERSION)
                .with_cockpit(self.share.as_deref());
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
        let once = || -> Result<Duration> {
            if mode == CacheMode::Cold {
                ctx.executor.clear();
            }
            let t = Instant::now();
            let reply = bridge::request(
                &url,
                method,
                &target,
                headers,
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
            SystemTarget::HttpIndex
            | SystemTarget::HttpHome
            | SystemTarget::HttpOpenApi
            | SystemTarget::HttpSwagger
            | SystemTarget::HttpCockpitOverview
            | SystemTarget::HttpCockpitCapabilities
            | SystemTarget::HttpCockpitGraph => {
                // the mounts come from the surfaces that declare them, so a route that
                // moves moves its benchmark with it
                let path = match target {
                    SystemTarget::HttpIndex | SystemTarget::HttpHome => "/",
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
                self.http_with("GET", path, &json!({}), CacheMode::NotApplicable, headers)
            }
        }
    }

    /// Stop the socket and the child.
    ///
    /// It takes `self`, so a runner cannot be used after its transports are gone. It is
    /// safe on a runner that started neither — the lazy resources are simply absent — which
    /// is why every path out of a run can call it unconditionally rather than remembering
    /// what it touched.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    ///
    /// // nothing was measured, so neither transport was ever started — and finishing is
    /// // still correct, which is the claim that lets every path out of a run call it
    /// let runner = Runner::new(Arc::clone(&ctx), Profile::QUICK, repo.root());
    /// runner.finish();
    ///
    /// // it took `self`, so the runner is gone and its handle on the context with it;
    /// // a finish that leaked the runner would leave this at 2
    /// assert_eq!(Arc::strong_count(&ctx), 1);
    /// ```
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
