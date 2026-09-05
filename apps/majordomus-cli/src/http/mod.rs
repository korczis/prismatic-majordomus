//! The HTTP projection: routes derived from the registry's HTTP exposures, an OpenAPI
//! document derived from the same registry at request time, and a Swagger UI shell that
//! loads that document. `router` knows the registry and nothing about sockets; `server`
//! knows sockets and nothing about capabilities.

pub mod mcp;
pub mod openapi;
pub mod router;
pub mod server;
pub mod swagger;

pub use router::{Request, Response, Router};
