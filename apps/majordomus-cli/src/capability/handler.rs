//! What a capability's behaviour is, and what it is allowed to know.
//!
//! A handler is a typed function — `fn(&Context, Input) -> Result<Output, CapabilityError>`
//! — placed behind a JSON boundary by [`handler`]. The boundary is what lets one registry
//! hold capabilities with different input and output types; the typing is what keeps the
//! handler from doing its own deserialisation.
//!
//! # What a handler may know
//!
//! [`Context`] is the whole of it: the index of the repository, the registry it belongs to,
//! the board of peers attached to this process, the Why catalogue, the executor, the
//! resolved web topology, and — only when the call arrived through an MCP session — which
//! peer made it. A handler never learns whether MCP, HTTP or the command line called it,
//! and that is deliberate: an answer that depended on the transport would be a second
//! implementation hiding inside the first.
//!
//! Index, registry and topology are immutable for the life of a process and are shared by
//! `Arc`, so a request never rebuilds canonical state. The peer board is the one thing that
//! changes, and it lives in memory only.
//!
//! # The error model
//!
//! [`CapabilityError`] has four cases and names no status code and no JSON-RPC code. Each
//! transport maps them into its own vocabulary, so adding a transport cannot change what a
//! handler is able to say.
//!
//! ```
//! use majordomus_cli::capability::{CapabilityError, Context, handler::handler};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct In { n: u32 }
//! #[derive(Serialize)]
//! struct Out { doubled: u32 }
//!
//! fn double(_: &Context, input: In) -> Result<Out, CapabilityError> {
//!     if input.n > 100 {
//!         return Err(CapabilityError::Refused("n is above the limit".into()));
//!     }
//!     Ok(Out { doubled: input.n * 2 })
//! }
//!
//! // the JSON boundary: the same handler, called the way every transport calls it
//! let boxed = handler::<In, Out, _>(double);
//! assert_eq!(
//!     CapabilityError::Refused("n is above the limit".into()).to_string(),
//!     "refused: n is above the limit"
//! );
//! // an input the type cannot read is the caller's fault, and says so
//! assert!(matches!(
//!     serde_json::from_value::<In>(serde_json::json!({ "n": "twelve" })),
//!     Err(_)
//! ));
//! let _ = boxed;
//! ```

use std::marker::PhantomData;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::index::Index;
use crate::peers::{PeerBoard, PeerId};
use crate::web::Topology;

use super::executor::CapabilityExecutor;

use super::model::Capability;
use super::registry::CapabilityRegistry;

/// Why a call did not produce an output. Transport adapters map these to their own
/// vocabularies; nothing here names a status code or a JSON-RPC code.
///
/// Four cases, because four things can go wrong and a caller does different things about
/// each: fix the input, ask for something that exists, accept a refusal, or report a
/// defect. A fifth would have to be a defect of the tool with a different name, and a
/// transport would then have to guess where to map it.
///
/// ```
/// use majordomus_cli::capability::CapabilityError;
/// // the prefix a reader sees is the case, so an error read out of a log or a JSON body
/// // still says which of the four it was
/// assert_eq!(
///     CapabilityError::Refused("the query is blank".into()).to_string(),
///     "refused: the query is blank"
/// );
/// assert!(CapabilityError::NotFound("rule x".into()).to_string().starts_with("not found:"));
/// // and the whose-fault-is-it distinction survives comparison, which is what a test
/// // asserting a refusal rather than a crash depends on
/// assert_ne!(
///     CapabilityError::Internal("io".into()),
///     CapabilityError::InvalidInput("io".into())
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityError {
    #[error("invalid input: {0}")]
    /// The input did not deserialize into the handler's type, or a filter named something the repository does not have.
    InvalidInput(String),
    #[error("not found: {0}")]
    /// The thing asked for does not exist: an object, a capability.
    NotFound(String),
    #[error("refused: {0}")]
    /// The input is well-formed and the call is declined, with the reason (a blank query).
    Refused(String),
    #[error("internal: {0}")]
    /// The handler itself failed; never the caller's fault.
    Internal(String),
}

/// What a handler may read: the index of the repository, the registry it belongs to, the
/// board of peers attached to this process, and, when the call came through an MCP
/// session, which peer made it. Index and registry are immutable for the life of a
/// process; the board is the one thing that changes, and it lives in memory only.
///
/// It is also the whole of what a handler may know: there is no ambient state to reach
/// for, no global registry and no way to ask which transport called. A capability that
/// wants a fact about the repository reads it from the index it was handed, which is why
/// two transports asking one question at one moment cannot get two answers.
///
/// Cloning is cheap and shares everything — every field is an `Arc` or a small value — so
/// the narrowing methods ([`Context::for_caller`], [`Context::reporting`],
/// [`Context::with_web`]) hand out a view rather than a copy.
///
/// ```
/// use majordomus_cli::capability::Context;
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
///
/// // the index and the registry are one process's canonical state, shared by pointer
/// let same: Context = ctx.as_ref().clone();
/// assert!(std::sync::Arc::ptr_eq(&ctx.index, &same.index));
/// assert!(std::sync::Arc::ptr_eq(&ctx.registry, &same.registry));
///
/// // a context built this way has no caller: nothing came through an MCP session
/// assert!(ctx.caller.is_none());
/// ```
#[derive(Clone)]
pub struct Context {
    /// The index of the repository's objects.
    pub index: Arc<Index>,
    /// The registry the handler belongs to, for introspection capabilities.
    pub registry: Arc<CapabilityRegistry>,
    /// The peers attached to this process.
    pub peers: Arc<PeerBoard>,
    /// The Why catalogue, derived from the index once when this context is composed and
    /// shared by every projection that reads it. A request never rebuilds it.
    pub why: Arc<crate::why::Catalogue>,
    /// The product model: the features of the layer with every fact a surface may say
    /// about them derived from the registry, the index, the catalogue and the topology.
    /// Built once, after the three it reads, and shared the same way.
    pub product: Arc<crate::product::ProductModel>,
    /// The one execution path: counters, cache, handler dispatch. Shared by every
    /// transport and every session of this process.
    pub executor: Arc<CapabilityExecutor>,
    /// The peer this call came from, when it came through an MCP session; `None` for the
    /// command line and for a plain HTTP request.
    pub caller: Option<PeerId>,
    /// The executions this process is running, and the engine that starts them.
    ///
    /// One per process, so that an execution started from a browser is the same execution
    /// an MCP client asks about and the command line lists. A handler reads it only when
    /// it is itself the execution plane's own capability; every other handler reports
    /// through [`Context::progress`] and never learns whether anything was listening.
    pub executions: Arc<crate::execution::ExecutionEngine>,
    /// What this call reports its steps and progress to, and how it learns it should stop.
    ///
    /// Silent unless the call came through the execution engine, which is why a handler
    /// may report unconditionally and a benchmark sample costs nothing.
    pub progress: crate::execution::Progress,
    /// The repository's web surfaces, resolved once for this process.
    ///
    /// Resolved here and never again: a projection that serves a subset of it narrows this
    /// value rather than discovering its own, so what the HTTP router serves, what the home
    /// page lists and what `web.surfaces` answers are three readings of one resolution.
    pub web: Arc<Topology>,
}

impl Context {
    /// A context over an index and a registry, with an empty board, a fresh executor and
    /// no caller.
    ///
    /// Composed once per process, and it is where the derived state of the process is
    /// built: the Why catalogue and the product model are computed here, from the index and
    /// the registry, and then shared. A request never rebuilds them, which is the reason
    /// this constructor is expensive and the narrowing methods are not.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry, Context};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    /// use std::sync::Arc;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let index = Arc::new(repo.index().unwrap());
    /// let registry = Arc::new(
    ///     CapabilityRegistry::builder()
    ///         .with_modules(builtin::modules())
    ///         .with_index(&index)
    ///         .build()
    ///         .unwrap(),
    /// );
    /// let ctx = Context::new(Arc::clone(&index), registry);
    /// assert!(Arc::ptr_eq(&ctx.index, &index), "the index is shared, not copied");
    /// assert_eq!(ctx.peers.len(), 0, "a fresh board");
    /// assert_eq!(ctx.executor.cached_entries(), 0, "and a fresh cache");
    /// ```
    pub fn new(index: Arc<Index>, registry: Arc<CapabilityRegistry>) -> Self {
        let web = Arc::new(resolve_web(&index));
        let why = Arc::new(crate::why::Catalogue::build(&index, &registry));
        let product = Arc::new(crate::product::ProductModel::build(
            &index, &registry, &why, &web,
        ));
        Context {
            index,
            registry,
            why,
            product,
            peers: Arc::new(PeerBoard::new()),
            executor: Arc::new(CapabilityExecutor::new()),
            executions: Arc::new(crate::execution::ExecutionEngine::default()),
            progress: crate::execution::Progress::silent(),
            caller: None,
            web,
        }
    }

    /// The same context, seen from one peer: what a session hands its handlers.
    ///
    /// The only thing that changes is who is asking, and the only capabilities that read
    /// it are the ones about the board itself — a peer announcing what it is working on
    /// has to be told apart from the other peers. Every other handler behaves identically
    /// whether or not a caller is set, because an answer that depended on who asked would
    /// be a second implementation hiding inside the first.
    ///
    /// ```
    /// use majordomus_cli::peers::Transport;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    /// use std::sync::Arc;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let peer = ctx.peers.attach(Transport::Http);
    /// let seen = ctx.for_caller(peer.clone());
    ///
    /// assert_eq!(seen.caller.as_ref(), Some(&peer));
    /// assert!(ctx.caller.is_none(), "the original is untouched");
    /// // and everything else is the same objects, not copies of them
    /// assert!(Arc::ptr_eq(&seen.index, &ctx.index));
    /// assert!(Arc::ptr_eq(&seen.peers, &ctx.peers), "one board, seen from one peer");
    /// ```
    pub fn for_caller(&self, caller: PeerId) -> Self {
        Context {
            caller: Some(caller),
            ..self.same()
        }
    }

    /// The same context, seen by one execution: what the engine hands the handler it runs.
    ///
    /// Everything else is shared, the engine included, so a capability that reads the
    /// executions of this process sees its own while it runs.
    ///
    /// ```
    /// use majordomus_cli::execution::Progress;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    /// use std::sync::Arc;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// // a handler outside an execution reports into silence and never learns that it did
    /// assert!(!ctx.progress.cancelled());
    ///
    /// let watched = ctx.reporting(Progress::silent());
    /// // the engine is shared, so a capability run this way can still see the executions
    /// // of its own process, its own among them
    /// assert!(Arc::ptr_eq(&watched.executions, &ctx.executions));
    /// assert!(Arc::ptr_eq(&watched.registry, &ctx.registry));
    /// ```
    pub fn reporting(&self, progress: crate::execution::Progress) -> Self {
        Context {
            progress,
            ..self.same()
        }
    }

    /// The same context with the web topology narrowed to what one projection serves.
    ///
    /// The narrowing is a filter over the value this context already holds, never a second
    /// discovery: a projection cannot serve a surface the process did not resolve.
    ///
    /// ```
    /// use majordomus_cli::synthetic::SyntheticRepository;
    /// use majordomus_cli::web::Topology;
    /// use std::sync::Arc;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    ///
    /// // one projection serving nothing at all is still a narrowing of what was resolved
    /// let narrowed = ctx.with_web(Arc::new(Topology::new(Vec::new())));
    /// assert!(narrowed.web.surfaces.is_empty());
    /// assert!(!Arc::ptr_eq(&narrowed.web, &ctx.web), "the topology is the one thing replaced");
    /// assert!(Arc::ptr_eq(&narrowed.index, &ctx.index), "and it is the only one");
    /// ```
    pub fn with_web(&self, web: Arc<Topology>) -> Self {
        Context { web, ..self.same() }
    }

    fn same(&self) -> Self {
        Context {
            index: Arc::clone(&self.index),
            registry: Arc::clone(&self.registry),
            why: Arc::clone(&self.why),
            product: Arc::clone(&self.product),
            peers: Arc::clone(&self.peers),
            executor: Arc::clone(&self.executor),
            executions: Arc::clone(&self.executions),
            progress: self.progress.clone(),
            caller: self.caller.clone(),
            web: Arc::clone(&self.web),
        }
    }

    /// Execute a capability by id: the one way anything calls a handler.
    ///
    /// Including a handler calling another. A capability that needs what a second one
    /// answers asks for it here rather than calling the Rust function, so the composed
    /// call is counted, cached and validated exactly as an external one is, and a
    /// capability cannot be reached by a path the executor does not know about.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityError;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    ///
    /// let out = ctx.execute("repository.info", serde_json::json!({})).unwrap();
    /// assert_eq!(out["objects"], ctx.index.objects.len());
    ///
    /// // an input the handler's type cannot read is the caller's fault, and says so
    /// let bad = ctx.execute("capabilities.describe", serde_json::json!({ "id": 7 }));
    /// assert!(matches!(bad, Err(CapabilityError::InvalidInput(_))));
    /// ```
    pub fn execute(&self, id: &str, input: Value) -> Result<Value, CapabilityError> {
        self.executor.execute(self, id, input)
    }

    /// The same, with the cache stepped over: what an execution runs.
    ///
    /// See [`CapabilityExecutor::execute_uncached`] for why watching something happen and
    /// being handed a remembered answer are not the same request.
    ///
    /// ```
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    ///
    /// // a capability that declares a process cache, run the way an execution runs it:
    /// // the same answer, and nothing kept
    /// let observed = ctx.execute_observed("capabilities.list", serde_json::json!({})).unwrap();
    /// assert_eq!(ctx.executor.cached_entries(), 0);
    ///
    /// // asked as a plain call, the same input is remembered — and answers identically
    /// let called = ctx.execute("capabilities.list", serde_json::json!({})).unwrap();
    /// assert_eq!(observed, called);
    /// assert_eq!(ctx.executor.cached_entries(), 1);
    /// ```
    pub fn execute_observed(&self, id: &str, input: Value) -> Result<Value, CapabilityError> {
        self.executor.execute_uncached(self, id, input)
    }
}

/// The topology of the repository the index was read from.
///
/// Discovery reads the site's configuration, the generated web root and the executable's
/// own constants; a malformed producer declaration is reported and leaves the process with
/// the surfaces it answers itself, which is what lets a broken report be diagnosed over
/// the very surfaces that would otherwise be unreachable.
fn resolve_web(index: &Index) -> Topology {
    let root = std::path::Path::new(&index.repository.root);
    match crate::web::discover::discover(root, crate::web::discover::Runtime::full()) {
        Ok(topology) => topology,
        Err(e) => {
            tracing::warn!(
                "a web surface could not be resolved: {e}; this process serves its own routes only"
            );
            Topology::new(crate::web::discover::native(
                crate::web::discover::Runtime::full(),
            ))
        }
    }
}

/// The JSON boundary of a handler.
///
/// One registry holds capabilities whose inputs and outputs are all different types, and
/// this is how: behind this trait every handler has the same signature, and the typing is
/// recovered on the way in and lost again on the way out by [`handler`]. Implemented once,
/// for the wrapper that function returns; nothing else should implement it, because an
/// implementation that did its own deserialisation would be a payload no canonical schema
/// describes.
///
/// ```
/// use majordomus_cli::capability::{handler::handler, CapabilityError, Context, Handler};
/// use majordomus_cli::synthetic::SyntheticRepository;
/// use std::sync::Arc;
///
/// #[derive(serde::Deserialize)]
/// struct In { n: u32 }
/// #[derive(serde::Serialize)]
/// struct Out { doubled: u32 }
/// fn double(_: &Context, input: In) -> Result<Out, CapabilityError> {
///     Ok(Out { doubled: input.n * 2 })
/// }
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let boxed: Arc<dyn Handler> = handler::<In, Out, _>(double);
///
/// let out = boxed.call(&ctx, serde_json::json!({ "n": 21 })).unwrap();
/// assert_eq!(out, serde_json::json!({ "doubled": 42 }));
/// ```
pub trait Handler: Send + Sync {
    /// Run the handler on a JSON input and answer with a JSON output.
    ///
    /// The typed function behind it decides what an acceptable input is, so a body the
    /// type cannot read never reaches it and comes back as `InvalidInput` naming the
    /// field. A transport therefore validates nothing of its own.
    ///
    /// ```
    /// use majordomus_cli::capability::{handler::handler, CapabilityError, Context, Handler};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// #[derive(serde::Deserialize)]
    /// struct In { n: u32 }
    /// #[derive(serde::Serialize)]
    /// struct Out { n: u32 }
    /// fn echo(_: &Context, input: In) -> Result<Out, CapabilityError> { Ok(Out { n: input.n }) }
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let boxed = handler::<In, Out, _>(echo);
    ///
    /// assert_eq!(boxed.call(&ctx, serde_json::json!({ "n": 1 })).unwrap()["n"], 1);
    /// let refused = boxed.call(&ctx, serde_json::json!({ "n": "one" }));
    /// assert!(matches!(refused, Err(CapabilityError::InvalidInput(_))));
    /// ```
    fn call(&self, ctx: &Context, input: Value) -> Result<Value, CapabilityError>;
}

struct Typed<I, O, F> {
    f: F,
    _io: PhantomData<fn(I) -> O>,
}

impl<I, O, F> Handler for Typed<I, O, F>
where
    I: DeserializeOwned,
    O: Serialize,
    F: Fn(&Context, I) -> Result<O, CapabilityError> + Send + Sync,
{
    fn call(&self, ctx: &Context, input: Value) -> Result<Value, CapabilityError> {
        let input: I = serde_json::from_value(input)
            .map_err(|e| CapabilityError::InvalidInput(e.to_string()))?;
        let output = (self.f)(ctx, input)?;
        serde_json::to_value(output).map_err(|e| CapabilityError::Internal(e.to_string()))
    }
}

/// Wrap a typed function as a handler.
///
/// The one place the JSON boundary is crossed. Inside `f` the input is a Rust value that
/// deserialised successfully and the output is a Rust value that will serialise; outside
/// it, both are `Value`. A failure on the way in is the caller's fault and a failure on
/// the way out is the tool's, and the two errors say so — which is the whole reason this
/// is a function rather than something each capability writes for itself.
///
/// `capability!` calls it, so a declaration never mentions it.
///
/// ```
/// use majordomus_cli::capability::{handler::handler, CapabilityError, Context};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// #[derive(serde::Deserialize)]
/// struct In { n: u32 }
/// #[derive(serde::Serialize)]
/// struct Out { doubled: u32 }
/// fn double(_: &Context, input: In) -> Result<Out, CapabilityError> {
///     if input.n > 100 {
///         return Err(CapabilityError::Refused("n is above the limit".into()));
///     }
///     Ok(Out { doubled: input.n * 2 })
/// }
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let boxed = handler::<In, Out, _>(double);
///
/// // the handler's own refusal travels out unchanged
/// let refused = boxed.call(&ctx, serde_json::json!({ "n": 1000 }));
/// assert_eq!(refused, Err(CapabilityError::Refused("n is above the limit".into())));
///
/// // and an input the type cannot read never reached the handler at all
/// let invalid = boxed.call(&ctx, serde_json::json!({}));
/// assert!(matches!(invalid, Err(CapabilityError::InvalidInput(_))));
/// ```
pub fn handler<I, O, F>(f: F) -> Arc<dyn Handler>
where
    I: DeserializeOwned + 'static,
    O: Serialize + 'static,
    F: Fn(&Context, I) -> Result<O, CapabilityError> + Send + Sync + 'static,
{
    Arc::new(Typed {
        f,
        _io: PhantomData,
    })
}

/// A descriptor with its behaviour and its benchmark cases: what the builtin source
/// contributes.
///
/// Three things that must travel together and do, because `capability!` produces all
/// three from one declaration: the descriptor every projection reads, the handler behind
/// the JSON boundary, and the representative inputs taken from the input type. A
/// capability cannot therefore be composed with a handler and no cases, or with cases that
/// belong to a different input type — those are compile errors in the macro rather than
/// coverage findings later.
///
/// ```
/// use majordomus_cli::capability::builtin;
/// use majordomus_cli::capability::handler::Executable;
/// // the composition of the application, and every executable in it carries all three
/// let module = builtin::repository::module();
/// let first: &Executable = module.capabilities.first().unwrap();
/// assert_eq!(first.capability.id.as_str(), "repository.info");
/// assert!(first.capability.kind.is_executable(), "a descriptor with a handler");
/// ```
pub struct Executable {
    /// The descriptor.
    pub capability: Capability,
    /// The behaviour, behind the JSON boundary.
    pub handler: Arc<dyn Handler>,
    /// The representative inputs, from the input type's `BenchmarkCases`.
    pub cases: super::benchmark::CaseProvider,
}

impl Executable {
    /// Declare that this handler looks at its cancellation flag and stops.
    ///
    /// The one thing about a capability that its kind cannot decide, because it is a fact
    /// about the handler's own code rather than about what the call means. Everything a
    /// client does with it — whether the Cockpit offers a Cancel button, what
    /// `executions.cancel` reports — is read from here, so a handler that ignores its flag
    /// never produces a control that lies.
    ///
    /// ```
    /// use majordomus_cli::capability;
    /// use majordomus_cli::capability::{BenchmarkCases, CaseContext, Context, CapabilityError, Exposure, NamedCase, Stability};
    /// #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    /// struct In {}
    /// impl BenchmarkCases for In {
    ///     fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> { vec![NamedCase::new("default", In {})] }
    /// }
    /// #[derive(serde::Serialize, schemars::JsonSchema)]
    /// struct Out { ok: bool }
    /// fn walk(ctx: &Context, _: In) -> Result<Out, CapabilityError> {
    ///     if ctx.progress.cancelled() { return Err(ctx.progress.cancellation()); }
    ///     Ok(Out { ok: true })
    /// }
    /// let e = capability! {
    ///     id: "demo.walk", title: "Walk", description: "Walks.", input: In, output: Out,
    ///     stability: Stability::Experimental, exposure: Exposure::default(), tags: [],
    ///     handler: walk,
    /// }.cancellable();
    /// assert!(e.capability.execution.cancellable);
    /// ```
    pub fn cancellable(mut self) -> Self {
        self.capability.execution = self.capability.execution.stoppable();
        self
    }
}

/// The one canonical declaration of an executable capability. From it every projection is
/// derived: the MCP tool and its schemas, the HTTP route and its OpenAPI operation, the
/// Swagger UI entry, the command line's introspection, the benchmark targets for every
/// transport it is exposed on, the cache policy the executor applies, and the generated
/// reference. Nothing is declared a second time anywhere.
///
/// Fields, in this order: `id`, an optional `kind` (a query by default;
/// `CapabilityKind::Command` for one that changes this process's memory), `title`,
/// `description`, `input`, `output`, `stability`, `exposure`, `tags`, an optional `cache`
/// (`CachePolicy::Disabled` by default), an optional `benchmark`
/// (`BenchmarkPolicy::Required` by default), and `handler`. `$input` and `$output` derive
/// `schemars::JsonSchema` (and serde), and `$input` implements
/// [`super::benchmark::BenchmarkCases`]: their schemas become the canonical schemas,
/// their doc comments the descriptions a client reads, and the input's cases the
/// benchmark inputs of every transport. The provenance is the Rust module the macro is
/// expanded in; the capability's module is stamped by [`crate::module!`]. Nothing here
/// registers anything: the result is composed explicitly into a module.
///
/// ```
/// use majordomus_cli::capability;
/// use majordomus_cli::capability::{BenchmarkCases, CachePolicy, CaseContext, Context, CapabilityError, Exposure, NamedCase, Stability};
/// #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
/// struct In {}
/// impl BenchmarkCases for In {
///     fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> { vec![NamedCase::new("default", In {})] }
/// }
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Out { ok: bool }
/// fn ping(_: &Context, _: In) -> Result<Out, CapabilityError> { Ok(Out { ok: true }) }
/// let e = capability! {
///     id: "demo.ping", title: "Ping", description: "Answers.", input: In, output: Out,
///     stability: Stability::Experimental, exposure: Exposure::default(), tags: ["demo"],
///     cache: CachePolicy::Process { max_entries: 8, ttl_seconds: None },
///     handler: ping,
/// };
/// assert_eq!(e.capability.id.as_str(), "demo.ping");
/// assert!(e.capability.cache.is_enabled());
/// ```
#[macro_export]
macro_rules! capability {
    // ---- the full form: every field present
    (
        id: $id:expr,
        kind: $kind:expr,
        title: $title:expr,
        description: $description:expr,
        input: $input:ty,
        output: $output:ty,
        stability: $stability:expr,
        exposure: $exposure:expr,
        tags: [$($tag:expr),* $(,)?],
        cache: $cache:expr,
        benchmark: $benchmark:expr,
        handler: $handler:expr $(,)?
    ) => {{
        // classified here, from the kind and the exposure this declaration already
        // carries, so that no declaration restates what it has just said and no
        // projection decides it later
        let kind = $kind;
        let exposure = $exposure;
        $crate::capability::Executable {
            capability: $crate::capability::Capability {
                id: $crate::capability::CapabilityId::unchecked($id),
                module: $crate::capability::ModuleId::unchecked(""),
                kind,
                title: String::from($title),
                description: String::from($description),
                input: $crate::capability::CanonicalSchema::of::<$input>(),
                output: $crate::capability::CanonicalSchema::of::<$output>(),
                provenance: $crate::capability::Provenance::Builtin { module: String::from(module_path!()) },
                availability: $crate::capability::Availability::classify(kind, &exposure),
                visibility: $crate::capability::Visibility::classify(&exposure),
                exposure,
                stability: $stability,
                tags: vec![$(String::from($tag)),*],
                benchmark: $benchmark,
                cache: $cache,
                execution: $crate::capability::ExecutionPolicy::classify(kind),
            },
            handler: $crate::capability::handler::handler::<$input, $output, _>($handler),
            cases: <$input as $crate::capability::BenchmarkCases>::benchmark_cases_json,
        }
    }};
    // ---- defaults: kind Query, cache Disabled, benchmark Required, in every combination
    ( id: $id:expr, title: $title:expr, description: $d:expr, input: $i:ty, output: $o:ty, stability: $s:expr, exposure: $e:expr, tags: [$($t:expr),* $(,)?], handler: $h:expr $(,)? ) => {
        $crate::capability! { id: $id, kind: $crate::capability::CapabilityKind::Query, title: $title, description: $d, input: $i, output: $o, stability: $s, exposure: $e, tags: [$($t),*], cache: $crate::capability::CachePolicy::Disabled, benchmark: $crate::capability::BenchmarkPolicy::Required, handler: $h }
    };
    ( id: $id:expr, kind: $k:expr, title: $title:expr, description: $d:expr, input: $i:ty, output: $o:ty, stability: $s:expr, exposure: $e:expr, tags: [$($t:expr),* $(,)?], handler: $h:expr $(,)? ) => {
        $crate::capability! { id: $id, kind: $k, title: $title, description: $d, input: $i, output: $o, stability: $s, exposure: $e, tags: [$($t),*], cache: $crate::capability::CachePolicy::Disabled, benchmark: $crate::capability::BenchmarkPolicy::Required, handler: $h }
    };
    ( id: $id:expr, title: $title:expr, description: $d:expr, input: $i:ty, output: $o:ty, stability: $s:expr, exposure: $e:expr, tags: [$($t:expr),* $(,)?], cache: $c:expr, handler: $h:expr $(,)? ) => {
        $crate::capability! { id: $id, kind: $crate::capability::CapabilityKind::Query, title: $title, description: $d, input: $i, output: $o, stability: $s, exposure: $e, tags: [$($t),*], cache: $c, benchmark: $crate::capability::BenchmarkPolicy::Required, handler: $h }
    };
    ( id: $id:expr, kind: $k:expr, title: $title:expr, description: $d:expr, input: $i:ty, output: $o:ty, stability: $s:expr, exposure: $e:expr, tags: [$($t:expr),* $(,)?], cache: $c:expr, handler: $h:expr $(,)? ) => {
        $crate::capability! { id: $id, kind: $k, title: $title, description: $d, input: $i, output: $o, stability: $s, exposure: $e, tags: [$($t),*], cache: $c, benchmark: $crate::capability::BenchmarkPolicy::Required, handler: $h }
    };
    ( id: $id:expr, title: $title:expr, description: $d:expr, input: $i:ty, output: $o:ty, stability: $s:expr, exposure: $e:expr, tags: [$($t:expr),* $(,)?], benchmark: $b:expr, handler: $h:expr $(,)? ) => {
        $crate::capability! { id: $id, kind: $crate::capability::CapabilityKind::Query, title: $title, description: $d, input: $i, output: $o, stability: $s, exposure: $e, tags: [$($t),*], cache: $crate::capability::CachePolicy::Disabled, benchmark: $b, handler: $h }
    };
    ( id: $id:expr, kind: $k:expr, title: $title:expr, description: $d:expr, input: $i:ty, output: $o:ty, stability: $s:expr, exposure: $e:expr, tags: [$($t:expr),* $(,)?], benchmark: $b:expr, handler: $h:expr $(,)? ) => {
        $crate::capability! { id: $id, kind: $k, title: $title, description: $d, input: $i, output: $o, stability: $s, exposure: $e, tags: [$($t),*], cache: $crate::capability::CachePolicy::Disabled, benchmark: $b, handler: $h }
    };
    ( id: $id:expr, title: $title:expr, description: $d:expr, input: $i:ty, output: $o:ty, stability: $s:expr, exposure: $e:expr, tags: [$($t:expr),* $(,)?], cache: $c:expr, benchmark: $b:expr, handler: $h:expr $(,)? ) => {
        $crate::capability! { id: $id, kind: $crate::capability::CapabilityKind::Query, title: $title, description: $d, input: $i, output: $o, stability: $s, exposure: $e, tags: [$($t),*], cache: $c, benchmark: $b, handler: $h }
    };
}
