//! Command dispatch: each command is a function from its arguments to an exit code, and
//! this module is the only place that maps one to the other.

pub mod mcp;

use crate::cli::{Cli, Command};
use crate::error::Result;

/// Run the selected command. `Ok(0)` is success; an `Err` carries its own exit code.
pub fn run(cli: Cli) -> Result<u8> {
    match cli.command {
        Command::Mcp(args) => mcp::run(args),
    }
}
