//! Command dispatch: each command is a function from its arguments to an exit code, and
//! this module is the only place that maps one to the other.

pub mod bench;
pub mod capabilities;
pub mod distribution;
pub mod generate;
pub mod knowledge;
pub mod mcp;
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
        Command::Worktree(args) => worktree::run(args),
        Command::Knowledge(args) => knowledge::run(args),
        Command::Canonicality(args) => knowledge::run_canonicality(args),
        Command::Explain(args) => knowledge::run_explain(args),
        Command::Change(args) => knowledge::run_change(args),
    }
}
