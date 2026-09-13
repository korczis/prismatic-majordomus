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

/// How much to measure: the warm-up, the samples per target and cache mode, and the
/// processes spawned for the one target that needs a fresh one.
///
/// A profile is not a speed setting. The sample count decides which metrics the regression
/// policy will judge at all: `p95` asks for fifty samples and `p99` for two hundred, so a
/// `quick` run of twenty gates on the median and reports both tails as `SHORT`. Choosing a
/// profile is choosing how much of the policy is in force.
///
/// ```
/// use majordomus_cli::bench::baseline::Policy;
/// use majordomus_cli::bench::Profile;
///
/// let policy = Policy::default();
/// // Twenty samples: the median is judged, the tails are reported.
/// assert!(Profile::QUICK.samples < policy.regression["p95"].minimum_samples);
/// // Fifty: enough for p95, not for p99.
/// assert!(Profile::CI.samples >= policy.regression["p95"].minimum_samples);
/// assert!(Profile::CI.samples < policy.regression["p99"].minimum_samples);
/// // Two hundred: every metric in the policy gates.
/// assert!(Profile::FULL.samples >= policy.regression["p99"].minimum_samples);
///
/// // Every profile warms up first, so no measurement contains the first-call cost.
/// assert!([Profile::QUICK, Profile::CI, Profile::FULL].iter().all(|p| p.warmup > 0));
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

    /// The profile of a name, or `None` for anything else.
    ///
    /// There are exactly three and they are not composable: a run is `quick`, `ci` or
    /// `full`, and the name goes into the result document beside the numbers, so a reader
    /// of a baseline knows how many samples produced it. An unrecognised name is refused
    /// rather than defaulted, because defaulting would file a twenty-sample run under a
    /// profile that promises two hundred.
    ///
    /// ```
    /// use majordomus_cli::bench::Profile;
    /// assert_eq!(Profile::parse("full"), Some(Profile::FULL));
    /// assert_eq!(Profile::parse("quick").unwrap().samples, 20);
    /// assert_eq!(Profile::parse("thorough"), None);
    ///
    /// // The name round-trips, which is what makes it usable as a file name and a record.
    /// for p in [Profile::QUICK, Profile::CI, Profile::FULL] {
    ///     assert_eq!(Profile::parse(p.name), Some(p));
    /// }
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

/// Times targets against one context, over whichever transport each target names.
///
/// One runner holds all three transports because two of them cost something to start: the
/// HTTP socket and the MCP child are created on first use and kept for the rest of the run,
/// so a hundred targets pay for one server and one child rather than a hundred. That is
/// also why [`finish`](Runner::finish) exists and takes `self` — the socket and the child
/// outlive any single measurement and have to be stopped once, at the end.
///
/// ```no_run
/// use majordomus_cli::app::App;
/// use majordomus_cli::bench::{BenchmarkProjection, Profile, Runner, Transport};
/// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
///
/// let app = App::load(&RepoArgs {
///     repo: Some("<repository>".into()),
///     discovery: DiscoveryMode::Vcs,
///     strict: false,
///     share: None,
/// })
/// .unwrap();
/// let projection = BenchmarkProjection::from_context(&app.context);
/// let mut runner = Runner::new(app.context.clone(), Profile::QUICK, app.repository.root());
///
/// // The direct targets need neither the socket nor the child, so neither is started.
/// let mut measured = 0;
/// for target in projection.by_transport(Transport::Direct) {
///     measured += runner.run(target).unwrap().len();
/// }
/// assert!(measured >= projection.by_transport(Transport::Direct).count());
/// runner.finish();
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
    /// Nothing is started here, which is what makes a runner cheap to create and a
    /// direct-only run free of a server and a child process. The executable to spawn
    /// defaults to this process's own, so the child measured is the build being measured;
    /// [`with_executable`](Runner::with_executable) is for the tests that need to say
    /// otherwise.
    ///
    /// ```no_run
    /// use majordomus_cli::app::App;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
    ///
    /// let app = App::load(&RepoArgs {
    ///     repo: Some("<repository>".into()),
    ///     discovery: DiscoveryMode::Vcs,
    ///     strict: false,
    ///     share: None,
    /// })
    /// .unwrap();
    ///
    /// // Creating one binds no socket and spawns nothing; dropping it costs nothing either.
    /// let runner = Runner::new(app.context.clone(), Profile::CI, app.repository.root());
    /// runner.finish();
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
    /// The default — this process's own binary — is what makes an MCP measurement a
    /// measurement of the build under test. The override exists for the integration tests,
    /// which run inside a test harness whose executable is not a `majordomus` at all and
    /// must name the built binary explicitly.
    ///
    /// ```no_run
    /// use majordomus_cli::app::App;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
    ///
    /// let app = App::load(&RepoArgs {
    ///     repo: Some("<repository>".into()),
    ///     discovery: DiscoveryMode::Vcs,
    ///     strict: false,
    ///     share: None,
    /// })
    /// .unwrap();
    ///
    /// // A test that knows where the binary it built is.
    /// let built = app.repository.root().join("apps/majordomus-cli/target/release/majordomus");
    /// let runner = Runner::new(app.context.clone(), Profile::QUICK, app.repository.root())
    ///     .with_executable(built.clone());
    /// assert!(built.ends_with("majordomus"), "the child is a majordomus, not the test harness");
    /// runner.finish();
    /// ```
    pub fn with_executable(mut self, path: std::path::PathBuf) -> Self {
        self.executable = path;
        self
    }

    /// The share directory the MCP child reads (`MAJORDOMUS_SHARE`); the parent's.
    ///
    /// The kinds and schemas decide what the layer contains, so a child reading a different
    /// distribution builds a different index and its `tools/list` is a different length. The
    /// MCP numbers would then be of another repository than the direct ones they are printed
    /// beside — the same comparison, two subjects.
    ///
    /// ```no_run
    /// use majordomus_cli::app::App;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
    ///
    /// let app = App::load(&RepoArgs {
    ///     repo: Some("<repository>".into()),
    ///     discovery: DiscoveryMode::Vcs,
    ///     strict: false,
    ///     share: None,
    /// })
    /// .unwrap();
    ///
    /// // Hand the child what the parent resolved, rather than letting it resolve again.
    /// let runner = Runner::new(app.context.clone(), Profile::QUICK, app.repository.root())
    ///     .with_share(app.share.dir().to_path_buf());
    /// runner.finish();
    /// ```
    pub fn with_share(mut self, share: std::path::PathBuf) -> Self {
        self.share = Some(share);
        self
    }

    /// Arguments the MCP child gets after `mcp --standalone`, so that it reads the
    /// repository the way the parent did (`--discovery filesystem`, `--strict`).
    ///
    /// Discovery is the one that bites. A parent that enumerated the layer with
    /// `--discovery filesystem` sees a fixture's untracked files; a child left on the
    /// default asks git and sees none of them, so it indexes fewer objects and answers
    /// faster — a difference in the measurement produced entirely by the benchmark's own
    /// setup.
    ///
    /// ```no_run
    /// use majordomus_cli::app::App;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
    ///
    /// let args = RepoArgs {
    ///     repo: Some("<repository>".into()),
    ///     discovery: DiscoveryMode::Filesystem,
    ///     strict: true,
    ///     share: None,
    /// };
    /// let app = App::load(&args).unwrap();
    ///
    /// // Pass the parent's own reading of the repository down to the child.
    /// let mut child_args = vec!["--discovery".to_string(), "filesystem".to_string()];
    /// if args.strict {
    ///     child_args.push("--strict".to_string());
    /// }
    /// let runner = Runner::new(app.context.clone(), Profile::QUICK, app.repository.root())
    ///     .with_child_args(child_args.clone());
    /// assert_eq!(child_args.len(), 3, "discovery, its value, and strict");
    /// runner.finish();
    /// ```
    pub fn with_child_args(mut self, args: Vec<String>) -> Self {
        self.child_args = args;
        self
    }

    /// Time one target: one result per cache mode it has.
    ///
    /// The vector's length is decided by the target's cache policy, not by the caller: a
    /// capability that declares a process cache is measured twice — cold, with the cache
    /// cleared before every sample, and warm, with the same input repeated — and one that
    /// declares none is measured once. Two numbers for one target is the point; the
    /// difference between them is what the cache is worth, and on the direct transport the
    /// handler-invocation counter says whether the warm run actually hit it.
    ///
    /// ```no_run
    /// use majordomus_cli::app::App;
    /// use majordomus_cli::bench::results::CacheMode;
    /// use majordomus_cli::bench::{BenchmarkProjection, Profile, Runner, TargetKind};
    /// use majordomus_cli::capability::CachePolicy;
    /// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
    ///
    /// let app = App::load(&RepoArgs {
    ///     repo: Some("<repository>".into()),
    ///     discovery: DiscoveryMode::Vcs,
    ///     strict: false,
    ///     share: None,
    /// })
    /// .unwrap();
    /// let projection = BenchmarkProjection::from_context(&app.context);
    /// let mut runner = Runner::new(app.context.clone(), Profile::QUICK, app.repository.root());
    ///
    /// for target in &projection.targets {
    ///     let results = runner.run(target).unwrap();
    ///     let cached = matches!(
    ///         &target.kind,
    ///         TargetKind::Capability { cache, .. } if cache.is_enabled()
    ///     );
    ///     assert_eq!(results.len(), if cached { 2 } else { 1 });
    ///     if cached {
    ///         assert!(results.iter().any(|r| r.cache_mode == CacheMode::Cold));
    ///         assert!(results.iter().any(|r| r.cache_mode == CacheMode::Warm));
    ///     }
    ///     assert!(results.iter().all(|r| r.key == target.key));
    /// }
    /// runner.finish();
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
    /// It takes `self` because there is nothing to run afterwards, and because forgetting
    /// it would leave a bound port and a live `majordomus mcp` process behind for every run
    /// — on a machine where several sessions benchmark the same repository, that is how a
    /// port comes to be held by a process nobody remembers starting. Calling it on a runner
    /// that never started either is well defined and does nothing.
    ///
    /// ```no_run
    /// use majordomus_cli::app::App;
    /// use majordomus_cli::bench::{Profile, Runner};
    /// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
    ///
    /// let app = App::load(&RepoArgs {
    ///     repo: Some("<repository>".into()),
    ///     discovery: DiscoveryMode::Vcs,
    ///     strict: false,
    ///     share: None,
    /// })
    /// .unwrap();
    ///
    /// // Nothing was measured, so no socket and no child were ever started.
    /// let runner = Runner::new(app.context.clone(), Profile::QUICK, app.repository.root());
    /// runner.finish();
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
