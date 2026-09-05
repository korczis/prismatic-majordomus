//! The completeness contract of the native command line's documentation.
//!
//! A command of the Rust executable is not finished when it parses: it is finished when it
//! says what it does, every argument says what it is for, and every command a person can
//! run carries at least one example that the example tests execute. This module is the one
//! place that contract is expressed. [`validate`] reports every violation of one run, so a
//! missing piece is fixed in one edit rather than one violation at a time.

use std::collections::BTreeMap;

use super::docs::{CommandDoc, DECLARATION};
use super::{Cli, EXAMPLES};

/// One thing missing from the command line's documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The stable code, `CLI_DOC_MISSING_EXAMPLE`; what a reader greps for.
    pub code: &'static str,
    /// The command it is about, as a person types it.
    pub command: String,
    /// What is missing, in one line.
    pub detail: String,
    /// The rule that was broken, in one line.
    pub rule: &'static str,
}

impl std::fmt::Display for Violation {
    /// The actionable form: the code, the command, the file to edit, what is wrong and the
    /// rule that says so.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:\n  command: {}\n  source: {}\n  detail: {}\n  rule: {}",
            self.code, self.command, DECLARATION, self.detail, self.rule
        )
    }
}

fn v(code: &'static str, command: &str, detail: String, rule: &'static str) -> Violation {
    Violation {
        code,
        command: command.to_string(),
        detail,
        rule,
    }
}

/// Every violation in the tree, in command order, then in example order. An empty result
/// is the contract met.
///
/// ```
/// // the command line this crate declares documents itself completely
/// assert!(majordomus_cli::cli::validate(&majordomus_cli::cli::tree()).is_empty());
/// ```
pub fn validate(tree: &CommandDoc) -> Vec<Violation> {
    let mut out = Vec::new();
    let commands: BTreeMap<String, &CommandDoc> = tree
        .flatten()
        .into_iter()
        .map(|c| (c.path[1..].join(" "), c))
        .collect();

    // every example set names a command of the tree, and none names it twice: an example
    // attached to a command that was renamed or removed is exactly the drift this catches
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for set in EXAMPLES {
        *seen.entry(set.command).or_default() += 1;
        if !commands.contains_key(set.command) {
            out.push(v(
                "CLI_DOC_EXAMPLE_WRONG_COMMAND",
                format!("majordomus {}", set.command).trim_end(),
                format!(
                    "{} example(s) are declared for a command the clap declaration does not have",
                    set.examples.len()
                ),
                "an example set names a command of the command line",
            ));
        }
    }
    for (command, n) in seen.iter().filter(|(_, n)| **n > 1) {
        out.push(v(
            "CLI_DOC_DUPLICATE_EXAMPLE_SET",
            format!("majordomus {command}").trim_end(),
            format!("{n} example sets are declared for it; one command has one set"),
            "each command declares its examples once",
        ));
    }

    let mut ids: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for set in EXAMPLES {
        for e in set.examples {
            ids.entry(e.id).or_default().push(set.command.to_string());
        }
    }
    for (id, owners) in ids.iter().filter(|(_, o)| o.len() > 1) {
        out.push(v(
            "CLI_DOC_DUPLICATE_EXAMPLE_ID",
            "majordomus",
            format!(
                "example id `{id}` is declared {} times ({})",
                owners.len(),
                owners.join(", ")
            ),
            "an example id is unique across the whole command line",
        ));
    }

    for c in tree.flatten() {
        let name = c.command();
        if c.about.trim().is_empty() {
            out.push(v(
                "CLI_DOC_MISSING_ABOUT",
                &name,
                "the command has no one-line description".to_string(),
                "every command says in one line what it does",
            ));
        }
        // A parent that only groups commands is described by the commands under it; a
        // command a person runs answers "what does running this do" in more than a line.
        if !c.subcommands.is_empty()
            && c.long_about
                .as_deref()
                .map(|l| l.trim().is_empty())
                .unwrap_or(true)
            && c.path.len() == 1
        {
            out.push(v(
                "CLI_DOC_MISSING_LONG_ABOUT",
                &name,
                "the root of the command line has no long description".to_string(),
                "the root command carries the long description of the executable",
            ));
        }
        for a in &c.args {
            if a.help.trim().is_empty() {
                out.push(v(
                    "CLI_DOC_EMPTY_ARG_HELP",
                    &name,
                    format!("argument `{}` has no help text", a.name),
                    "every argument says what it is for",
                ));
            }
            if a.takes_value && !a.possible_values.is_empty() {
                for pv in a.possible_values.iter().filter(|p| {
                    p.help
                        .as_deref()
                        .map(|h| h.trim().is_empty())
                        .unwrap_or(true)
                }) {
                    out.push(v(
                        "CLI_DOC_EMPTY_VALUE_HELP",
                        &name,
                        format!(
                            "argument `{}` accepts `{}` and does not say what it means",
                            a.name, pv.name
                        ),
                        "every accepted value of an enumerated argument says what it means",
                    ));
                }
            }
        }
        if c.executable && c.examples.is_empty() {
            out.push(v(
                "CLI_DOC_MISSING_EXAMPLE",
                &name,
                "a command that can be run and has no example".to_string(),
                "every command a person can run carries at least one executable example",
            ));
        }
        if !c.executable && !c.examples.is_empty() {
            out.push(v(
                "CLI_DOC_EXAMPLE_ON_GROUP",
                &name,
                "the command only groups other commands and cannot be run, yet carries an example"
                    .to_string(),
                "an example belongs to a command that can be run",
            ));
        }
        for e in &c.examples {
            if e.title.trim().is_empty() || e.description.trim().is_empty() {
                out.push(v(
                    "CLI_DOC_EMPTY_EXAMPLE",
                    &name,
                    format!("example `{}` has no title or no description", e.id),
                    "an example says what it shows and what comes back",
                ));
            }
            // Level A: the argv shown to a reader is the argv this crate's own parser
            // accepts. A renamed flag, a removed subcommand or an invalid enumerated value
            // fails here, at the speed of a unit test.
            for argv in std::iter::once(&e.argv).chain(e.setup.iter().map(|s| &s.argv)) {
                if let Err(err) = parse(argv) {
                    out.push(v(
                        "CLI_DOC_EXAMPLE_DOES_NOT_PARSE",
                        &name,
                        format!(
                            "example `{}`: `majordomus {}` is not accepted by the parser: {}",
                            e.id,
                            argv.join(" "),
                            err.kind()
                        ),
                        "an example parses through the command line it documents",
                    ));
                }
            }
            if !e.argv.starts_with(&c.path[1..]) {
                out.push(v(
                    "CLI_DOC_EXAMPLE_WRONG_COMMAND",
                    &name,
                    format!(
                        "example `{}` runs `majordomus {}`, which is not this command",
                        e.id,
                        e.argv.join(" ")
                    ),
                    "an example of a command runs that command",
                ));
            }
        }
    }
    out
}

/// The example's argv through the canonical parser. Public so that a test can hold one
/// argument vector to the declaration without going through the whole tree.
pub fn parse(argv: &[String]) -> Result<Cli, clap::Error> {
    use clap::Parser;
    let mut full = vec!["majordomus".to_string()];
    full.extend(argv.iter().cloned());
    Cli::try_parse_from(full)
}
