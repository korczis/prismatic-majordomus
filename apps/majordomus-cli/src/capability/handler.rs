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
#[derive(Clone)]
pub struct Context {
    /// The index of the repository's objects.
    pub index: Arc<Index>,
    /// The registry the handler belongs to, for introspection capabilities.
    pub registry: Arc<CapabilityRegistry>,
    /// The peers attached to this process.
    pub peers: Arc<PeerBoard>,
    /// The execution episodes this process holds: one per client that asked for one, keyed
    /// by the client's own durable identity and driven by the connection (ADR 0043).
    ///
    /// Beside the peer board and not inside it, because a peer and an episode are not the
    /// same thing: a connection holds zero or one episode, and an episode outlives the
    /// connection that opened it so that a client which reconnects comes back to its own.
    pub episodes: Arc<crate::episodes::EpisodeBoard>,
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
    pub fn new(index: Arc<Index>, registry: Arc<CapabilityRegistry>) -> Self {
        let index_root = std::path::PathBuf::from(&index.repository.root);
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
            // The board drives the repository the index was read from: the episode a client
            // opens here is an episode of *this* checkout, written by the same command a
            // provider hook runs. A checkout whose tool cannot be found says so in every
            // episode rather than recording nothing quietly.
            episodes: Arc::new(crate::episodes::EpisodeBoard::for_repository(
                index_root.clone(),
            )),
            executor: Arc::new(CapabilityExecutor::new()),
            executions: Arc::new(crate::execution::ExecutionEngine::default()),
            progress: crate::execution::Progress::silent(),
            caller: None,
            web,
        }
    }

    /// The same context, seen from one peer: what a session hands its handlers.
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
            episodes: Arc::clone(&self.episodes),
            executor: Arc::clone(&self.executor),
            executions: Arc::clone(&self.executions),
            progress: self.progress.clone(),
            caller: self.caller.clone(),
            web: Arc::clone(&self.web),
        }
    }

    /// Execute a capability by id: the one way anything calls a handler.
    pub fn execute(&self, id: &str, input: Value) -> Result<Value, CapabilityError> {
        self.executor.execute(self, id, input)
    }

    /// The same, with the cache stepped over: what an execution runs.
    ///
    /// See [`CapabilityExecutor::execute_uncached`] for why watching something happen and
    /// being handed a remembered answer are not the same request.
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
pub trait Handler: Send + Sync {
    /// Run the handler on a JSON input and answer with a JSON output.
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
