//! Execution: a typed handler behind a JSON boundary, the context it runs in, and the
//! transport-neutral error it may return. A handler never learns whether MCP, HTTP or the
//! command line called it.

use std::marker::PhantomData;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::index::Index;

use super::model::Capability;
use super::registry::CapabilityRegistry;

/// Why a call did not produce an output. Transport adapters map these to their own
/// vocabularies; nothing here names a status code or a JSON-RPC code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("refused: {0}")]
    Refused(String),
    #[error("internal: {0}")]
    Internal(String),
}

/// What a handler may read: the index of the repository and the registry it belongs to.
/// Both are immutable for the life of a process.
pub struct Context {
    pub index: Arc<Index>,
    pub registry: Arc<CapabilityRegistry>,
}

/// The JSON boundary of a handler.
pub trait Handler: Send + Sync {
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
    pub capability: Capability,
    pub handler: Arc<dyn Handler>,
}

/// Build an executable capability from its parts. `$I` and `$O` must derive
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
        $crate::capability::Executable {
            capability: $crate::capability::Capability {
                id: $crate::capability::CapabilityId::unchecked($id),
                kind: $crate::capability::CapabilityKind::Query,
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
