//! The MCP capability: a projection of the index into resources and tools (`surface`),
//! the JSON-RPC and MCP method handling over it (`protocol`), and the one transport
//! shipped (`stdio`). Only `protocol` knows the wire shapes; only `stdio` knows about
//! file descriptors; `surface` knows neither.

pub mod protocol;
pub mod stdio;
pub mod surface;

pub use protocol::Server;
pub use surface::Surface;
