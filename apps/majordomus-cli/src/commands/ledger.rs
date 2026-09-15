//! `majordomus ledger append`: the shell tool's writer of the ledger.
//!
//! The one command here that does not reach its capability through the executor, and the
//! reason is measured rather than preferred: the shell calls it on every event it records,
//! and composing a [`crate::capability::Context`] builds the repository's index — a walk of
//! the whole layer — where an append needs one file read, two git questions and one locked
//! write. Both paths run the same body, [`crate::capability::builtin::ledger`]'s `append`,
//! so the line a person's command writes and the line the capability writes cannot differ.

use std::io::{Read, Write};
use std::path::PathBuf;

use crate::capability::builtin::ledger;
use crate::cli::{LedgerArgs, LedgerCommand, OutputFormat};
use crate::error::{Error, Result};
use crate::share::Share;

/// The exit code of a refused append: the shell's `MJ_EX_INTERNAL`, because an event the
/// vocabulary does not accept is a defect of the caller, never a fact about the repository.
const EXIT_REFUSED: u8 = 13;

/// Run `majordomus ledger`.
pub fn run(args: LedgerArgs) -> Result<u8> {
    let LedgerCommand::Append {
        event,
        payload,
        root,
        share,
        format,
    } = args.command;
    let root = match root {
        Some(r) => r,
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let share: PathBuf = Share::locate(share.as_deref(), &root)?.dir().to_path_buf();
    let payload = match payload {
        Some(p) => p,
        None => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| Error::io("<stdin>", e))?;
            text
        }
    };
    let report = ledger::append(&root, &share, &event, &payload).map_err(|e| Error::Refused {
        code: EXIT_REFUSED,
        reason: e.to_string(),
    })?;
    if format == OutputFormat::Json {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let doc = serde_json::to_string_pretty(&report).map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })?;
        writeln!(out, "{doc}").map_err(Error::Transport)?;
    }
    Ok(0)
}
