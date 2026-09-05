//! The MCP capability: a projection of the index into resources and tools (`surface`),
//! the JSON-RPC and MCP method handling over it (`protocol`), the one local transport
//! (`stdio`), and the bridge that forwards a stdio session to the shared server of
//! another process (`bridge`). Only `protocol` knows the wire shapes; only `stdio` knows
//! about file descriptors; `surface` knows neither.

pub mod bridge;
pub mod protocol;
pub mod stdio;
pub mod surface;

pub use protocol::Server;
pub use surface::Surface;
