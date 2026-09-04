//! The executable: parse, log to stderr, run, exit with the contract's code.

use std::process::ExitCode;

use clap::Parser;

use majordomus_cli::cli::{Cli, EXIT_USAGE};

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // clap prints help and version to stdout with status 0, and usage errors to
            // stderr with status 2, which is the contract's usage code.
            let _ = e.print();
            return if e.use_stderr() {
                ExitCode::from(EXIT_USAGE)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    majordomus_cli::logging::init();
    match majordomus_cli::commands::run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            tracing::error!("{err}");
            eprintln!("majordomus: {err}");
            ExitCode::from(err.exit_code())
        }
    }
}
