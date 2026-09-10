//! Command dispatch: the one place a parsed command line becomes a call.
//!
//! Each command is a function from its own argument type to an exit code, and [`run`] is
//! the only mapping between the two. That is the whole of what lives here — no argument is
//! interpreted, no repository is opened, nothing is rendered. A command that does more than
//! render is a command that has taken business logic out of the capability it should be
//! calling: the modules below reach the registry through
//! [`Context::execute`](crate::capability::Context::execute), the same call MCP and the
//! HTTP routes make, so a command cannot answer differently from the API.
//!
//! # The exit-code contract
//!
//! `Ok(0)` is success. A command that decides a verdict answers with the code for it —
//! `10` for a contract unmet, which `scope --check` and `quality report` both use. An `Err`
//! carries its own code through [`crate::Error::exit_code`], so a failure has one code
//! whether the command chose it or an error did.
//!
//! # Which commands are here, and which are not
//!
//! A command is the projection of a capability, or [`crate::cli::local`] says why it is
//! not. Both are checked by [`crate::quality::parity`]. Nothing in this module decides it.
//!
//! ```
//! use majordomus_cli::cli::{Cli, Command};
//! use clap::Parser;
//!
//! // dispatch is total over the command line: every variant clap can parse has an arm
//! let cli = Cli::try_parse_from(["majordomus", "quality", "report", "--summary"]).unwrap();
//! assert!(matches!(cli.command, Command::Quality(_)));
//! ```

pub(crate) mod bench;
pub(crate) mod capabilities;
pub(crate) mod command_graph;
pub(crate) mod completion;
pub(crate) mod devcontext;
pub(crate) mod distribution;
pub(crate) mod env;
pub(crate) mod executions;
pub(crate) mod generate;
pub(crate) mod mcp;
pub(crate) mod product;
pub(crate) mod quality;
pub(crate) mod release;
pub(crate) mod scope;
pub(crate) mod serve;
pub(crate) mod web;
pub(crate) mod why;
pub(crate) mod worktree;

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
        Command::Release(args) => release::run(args),
        Command::Quality(args) => quality::run(args),
        Command::Run(args) => executions::run(args),
        Command::Executions(args) => executions::executions(args),
        Command::Devcontext(args) => devcontext::run(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn every_command_the_command_line_parses_reaches_a_dispatch_arm() {
        // the match in `run` is exhaustive by construction — a new variant of `Command` is a
        // compile error here — and this is the other half: that the words a person types
        // reach the variant this module then dispatches
        /// A command line, and the predicate that says which variant it must reach.
        type Case = (&'static [&'static str], fn(&Command) -> bool);
        let cases: &[Case] = &[
            (&["majordomus", "mcp"], |c| matches!(c, Command::Mcp(_))),
            (&["majordomus", "serve"], |c| matches!(c, Command::Serve(_))),
            (&["majordomus", "serve", "status"], |c| {
                matches!(c, Command::Serve(_))
            }),
            (&["majordomus", "capabilities", "list"], |c| {
                matches!(c, Command::Capabilities(_))
            }),
            (&["majordomus", "generate"], |c| {
                matches!(c, Command::Generate(_))
            }),
            (&["majordomus", "bench", "coverage"], |c| {
                matches!(c, Command::Bench(_))
            }),
            (&["majordomus", "scope"], |c| matches!(c, Command::Scope(_))),
            (&["majordomus", "web", "list"], |c| {
                matches!(c, Command::Web(_))
            }),
            (&["majordomus", "why", "list"], |c| {
                matches!(c, Command::Why(_))
            }),
            (&["majordomus", "distribution", "show"], |c| {
                matches!(c, Command::Distribution(_))
            }),
            (&["majordomus", "worktree", "status"], |c| {
                matches!(c, Command::Worktree(_))
            }),
            (&["majordomus", "quality", "report"], |c| {
                matches!(c, Command::Quality(_))
            }),
            (&["majordomus", "devcontext", "compile", "--issue", "I1"], |c| {
                matches!(c, Command::Devcontext(_))
            }),
        ];
        for (argv, is_expected) in cases {
            let cli =
                Cli::try_parse_from(*argv).unwrap_or_else(|e| panic!("{}: {e}", argv.join(" ")));
            assert!(
                is_expected(&cli.command),
                "{} dispatched elsewhere",
                argv.join(" ")
            );
        }
    }

    #[test]
    fn a_command_line_nothing_declares_is_refused_rather_than_dispatched() {
        assert!(Cli::try_parse_from(["majordomus", "nonesuch"]).is_err());
        assert!(
            Cli::try_parse_from(["majordomus"]).is_err(),
            "a command is required"
        );
    }
}
