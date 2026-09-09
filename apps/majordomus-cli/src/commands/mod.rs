//! Command dispatch: each command is a function from its arguments to an exit code, and
//! this module is the only place that maps one to the other.

pub mod bench;
pub mod capabilities;
pub mod command_graph;
pub mod completion;
pub mod distribution;
pub mod env;
pub mod generate;
pub mod mcp;
pub mod product;
pub mod scope;
pub mod serve;
pub mod web;
pub mod why;
pub mod worktree;

use crate::cli::{Cli, Command};
use crate::error::Result;

/// Run the selected command. `Ok(0)` is success; an `Err` carries its own exit code.
pub fn run(cli: Cli) -> Result<u8> {
    match cli.command {
        Command::Mcp(args) => mcp::run(args),
        Command::Serve(args) => serve::run(args),
        Command::Capabilities(args) => capabilities::run(args),
        Command::Generate(args) => generate::run(args),
        Command::Bench(args) => bench::run(args),
        Command::Scope(args) => scope::run(args),
        Command::Web(args) => web::run(args),
        Command::Why(args) => why::run(args),
        Command::Distribution(args) => distribution::run(args),
        Command::Env(args) => env::run(args),
        Command::Commands(args) => command_graph::run(args),
        Command::Completion(args) => completion::run(args),
        Command::Worktree(args) => worktree::run(args),
        Command::Product(args) => product::run(args),
    }
}
