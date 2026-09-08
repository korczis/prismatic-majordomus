//! The canonical capability model and its registry: the one place a capability is
//! defined. MCP, HTTP, OpenAPI, the CLI's introspection and the generated documentation
//! are projections of what lives here and define nothing of their own.
//!
//! Two sources feed the registry: typed executable capabilities written in Rust
//! ([`builtin`]) and declarative objects the repository's layer declares
//! ([`declarative`]). Both normalise into the same [`Capability`] descriptor; a consumer
//! cannot tell, and need not care, which source an entry came from except through its
//! provenance.
//!
//! ```
//! use majordomus_cli::capability::{builtin, CapabilityRegistry, HttpMethod};
//!
//! // the application composed from its modules, validated on the way in
//! let registry = CapabilityRegistry::builder()
//!     .with_modules(builtin::modules())
//!     .build()
//!     .expect("the composed application is valid");
//!
//! // one descriptor, and every projection is a way of asking for it
//! let by_id = registry.get("repository.info").expect("a canonical id");
//! let by_route = registry
//!     .by_http(HttpMethod::Get, "/api/v1/repository")
//!     .expect("the route it declares");
//! let by_tool = registry
//!     .by_mcp_tool("majordomus_repository")
//!     .expect("the MCP tool it declares");
//! assert_eq!(by_id.id, by_route.id);
//! assert_eq!(by_id.id, by_tool.id);
//!
//! // and nothing was told about it: the exposures are on the descriptor itself
//! assert_eq!(by_id.exposure.http.as_ref().unwrap().path, "/api/v1/repository");
//! ```

pub(crate) mod benchmark;
pub mod builtin;
pub(crate) mod declarative;
pub mod executor;
pub mod handler;
pub mod model;
pub(crate) mod module;
pub mod registry;
pub mod schema;

pub use benchmark::{BenchmarkCases, CaseContext, CaseProvider, NamedCase};
pub use executor::CapabilityExecutor;
pub use handler::{CapabilityError, Context, Executable, Handler};
pub use model::{
    Availability, BenchmarkPolicy, CachePolicy, Capability, CapabilityId, CapabilityKind,
    CliExposure, Exposure, HttpExposure, HttpMethod, McpExposure, McpResource, ModuleId,
    Provenance, Stability, Visibility, WaiverReason,
};
pub use module::ModuleDescriptor;
pub use registry::{CapabilityRegistry, Entry, RegistryError};
pub use schema::CanonicalSchema;
