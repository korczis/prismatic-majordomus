//! The HTTP projection: the surfaces this repository exposes, resolved once and served.
//!
//! `surfaces` narrows the resolved web topology to what this process can answer and binds a
//! handler to each; `router` asks it who owns a path and dispatches, knowing the registry
//! and nothing about sockets; `server` knows sockets and nothing about capabilities. The
//! OpenAPI document is derived from the same registry at request time, and the Swagger UI
//! shell loads it.

pub mod mcp;
pub mod openapi;
pub mod router;
pub mod server;
pub(crate) mod surfaces;
pub mod swagger;

pub use router::{Body, Request, Response, Router};
pub use surfaces::Served;
