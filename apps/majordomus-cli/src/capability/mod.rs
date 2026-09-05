//! The canonical capability model and its registry: the one place a capability is
//! defined. MCP, HTTP, OpenAPI, the CLI's introspection and the generated documentation
//! are projections of what lives here and define nothing of their own.
//!
//! Two sources feed the registry: typed executable capabilities written in Rust
//! ([`builtin`]) and declarative objects the repository's layer declares
//! ([`declarative`]). Both normalise into the same [`Capability`] descriptor; a consumer
//! cannot tell, and need not care, which source an entry came from except through its
//! provenance.

pub mod benchmark;
pub mod builtin;
pub mod declarative;
pub mod executor;
pub mod handler;
pub mod model;
pub mod module;
pub mod registry;
pub mod schema;

pub use benchmark::{BenchmarkCases, CaseContext, CaseProvider, NamedCase};
pub use executor::CapabilityExecutor;
pub use handler::{CapabilityError, Context, Executable, Handler};
pub use model::{
    BenchmarkPolicy, CachePolicy, Capability, CapabilityId, CapabilityKind, CliExposure, Exposure,
    HttpExposure, HttpMethod, McpExposure, McpResource, ModuleId, Provenance, Stability,
    WaiverReason,
};
pub use module::ModuleDescriptor;
pub use registry::{CapabilityRegistry, Entry, RegistryError};
pub use schema::CanonicalSchema;
