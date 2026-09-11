//! The process boundary's own contract: which stream a command writes to, what it exits
//! with, and what it may not put in front of a program that is reading it.
//!
//! `tests/cli.rs` asserts this for the root and for one command — help on stdout and
//! nothing on stderr, a usage error as exit 2 with stdout left empty. Those are contracts
//! of *every* command, and until this file existed they were checked on a sample of two.
//! The interesting failure is not a command that gets it wrong from the start; it is a
//! command that starts writing a diagnostic to stdout, or exiting 1 instead of 2 on a
//! typo, long after somebody wrote a script that pipes it.
//!
//! Nothing here enumerates the commands. The subject is `majordomus_cli::cli::tree()`
//! flattened, so a command added tomorrow is held to the same contract without this file
//! being edited — the property `tests/cli_docs.rs` and `tests/cli_examples.rs` already have
//! and the reason a new command cannot quietly escape.
//!
//! Both checks below are side-effect free by construction: clap answers `--help` and refuses
//! an unknown option before the command's own code runs, so no server starts, no repository
//! is written, and one fixture serves every case.

mod common;

use common::{run_in, Fixture};
use majordomus_cli::cli;

/// The path of every command in the tree, root included, as argument vectors.
fn every_command() -> Vec<Vec<String>> {
    cli::tree()
        .flatten()
        .into_iter()
        .map(|c| c.path.iter().skip(1).cloned().collect())
        .collect()
}

fn argv(path: &[String]) -> Vec<&str> {
    path.iter().map(String::as_str).collect()
}

#[test]
fn every_command_writes_its_help_to_stdout_and_nothing_to_stderr() {
    let f = Fixture::new();
    let all = every_command();
    assert!(all.len() > 1, "the tree flattened to nothing");
    for path in &all {
        let mut args = argv(path);
        args.push("--help");
        let (code, out, err) = run_in(&f.root(), &args, "");
        let name = if path.is_empty() {
            "majordomus".to_string()
        } else {
            format!("majordomus {}", path.join(" "))
        };
        assert_eq!(code, 0, "`{name} --help` exited {code}:\n{err}");
        assert!(
            err.is_empty(),
            "`{name} --help` wrote to stderr, so a reader piping stdout sees a diagnostic \
             out of band:\n{err}"
        );
        // clap prints the usage line, and it names the command being helped. A group whose
        // help arrived under the wrong path is the failure this catches.
        assert!(
            out.contains(&format!("Usage: {name}")),
            "`{name} --help` does not print its own usage line:\n{out}"
        );
    }
}

#[test]
fn an_unknown_option_is_exit_two_on_stderr_and_stdout_stays_empty() {
    let f = Fixture::new();
    // A name no command can plausibly declare, so this stays a usage error as the tree
    // grows. A short flag would risk colliding with one a command really has.
    const TYPO: &str = "--mj-no-such-option";
    for path in &every_command() {
        let mut args = argv(path);
        args.push(TYPO);
        let (code, out, err) = run_in(&f.root(), &args, "");
        let name = if path.is_empty() {
            "majordomus".to_string()
        } else {
            format!("majordomus {}", path.join(" "))
        };
        assert_eq!(
            code, 2,
            "`{name} {TYPO}` exited {code}, not 2. A usage error is exit 2 for every \
             command, which is what a caller distinguishes from a command that ran and \
             refused (10) or could not run (12).\nstdout: {out}\nstderr: {err}"
        );
        assert!(
            out.is_empty(),
            "`{name} {TYPO}` wrote to stdout while failing, so a caller parsing stdout is \
             handed an error message where a document should be:\n{out}"
        );
        assert!(
            err.contains(TYPO),
            "`{name} {TYPO}` does not name the argument it refused:\n{err}"
        );
    }
}

#[test]
fn no_command_puts_a_terminal_escape_sequence_in_a_pipe() {
    // The output of these runs is not a terminal — a test's pipe never is — so a command
    // that styles its output unconditionally is a command whose output cannot be parsed,
    // diffed or logged. `--help` is the one invocation every command answers, which makes
    // it the one place this can be asserted across the whole tree.
    //
    // ESC is the whole test: colour, cursor movement and the hyperlink sequence all begin
    // with it, and none of them belongs in a pipe.
    let f = Fixture::new();
    for path in &every_command() {
        let mut args = argv(path);
        args.push("--help");
        let (_code, out, _err) = run_in(&f.root(), &args, "");
        let name = format!("majordomus {}", path.join(" "));
        if let Some(at) = out.find('\u{1b}') {
            let from = at.saturating_sub(40);
            panic!(
                "`{name} --help` wrote an escape sequence to a pipe at byte {at}:\n{:?}",
                &out[from..(at + 40).min(out.len())]
            );
        }
    }
}
