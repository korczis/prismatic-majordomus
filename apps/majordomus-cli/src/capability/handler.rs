//! Execution: a typed handler behind a JSON boundary, the context it runs in, and the
//! transport-neutral error it may return. A handler never learns whether MCP, HTTP or the
//! command line called it.

use std::marker::PhantomData;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::index::Index;
use crate::peers::{PeerBoard, PeerId};

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
    /// The peer this call came from, when it came through an MCP session; `None` for the
    /// command line and for a plain HTTP request.
    pub caller: Option<PeerId>,
}

impl Context {
    /// A context over an index and a registry, with an empty board and no caller.
    pub fn new(index: Arc<Index>, registry: Arc<CapabilityRegistry>) -> Self {
        Context {
            index,
            registry,
            peers: Arc::new(PeerBoard::new()),
            caller: None,
        }
    }

    /// The same context, seen from one peer: what a session hands its handlers.
    pub fn for_caller(&self, caller: PeerId) -> Self {
        Context {
            index: Arc::clone(&self.index),
            registry: Arc::clone(&self.registry),
            peers: Arc::clone(&self.peers),
            caller: Some(caller),
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

/// A descriptor with its behaviour: what the builtin source contributes.
pub struct Executable {
    /// The descriptor.
    pub capability: Capability,
    /// The behaviour, behind the JSON boundary.
    pub handler: Arc<dyn Handler>,
}

/// Build an executable capability from its parts: a query by default, or the kind given
/// (`kind: CapabilityKind::Command` for one that changes this process's memory). `$I` and `$O` must derive
/// `schemars::JsonSchema` (and serde); their schemas become the canonical schemas, and
/// their doc comments the descriptions a client reads. The provenance is the module the
/// macro is expanded in. Nothing here registers anything: the result is composed
/// explicitly into [`super::builtin::all`].
#[macro_export]
macro_rules! capability {
    (
        id: $id:expr,
        title: $title:expr,
        description: $description:expr,
        input: $input:ty,
        output: $output:ty,
        stability: $stability:expr,
        exposure: $exposure:expr,
        tags: [$($tag:expr),* $(,)?],
        handler: $handler:expr $(,)?
    ) => {
        $crate::capability! {
            id: $id, kind: $crate::capability::CapabilityKind::Query, title: $title,
            description: $description, input: $input, output: $output, stability: $stability,
            exposure: $exposure, tags: [$($tag),*], handler: $handler,
        }
    };
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
        handler: $handler:expr $(,)?
    ) => {
        $crate::capability::Executable {
            capability: $crate::capability::Capability {
                id: $crate::capability::CapabilityId::unchecked($id),
                kind: $kind,
                title: String::from($title),
                description: String::from($description),
                input: $crate::capability::CanonicalSchema::of::<$input>(),
                output: $crate::capability::CanonicalSchema::of::<$output>(),
                provenance: $crate::capability::Provenance::Builtin { module: String::from(module_path!()) },
                exposure: $exposure,
                stability: $stability,
                tags: vec![$(String::from($tag)),*],
            },
            handler: $crate::capability::handler::handler::<$input, $output, _>($handler),
        }
    };
}
