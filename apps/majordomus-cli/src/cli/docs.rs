//! The command line as documentation data.
//!
//! clap is the one declaration of the command line: every command, argument, default and
//! value set. This module walks the built [`clap::Command`] into [`CommandDoc`], attaches
//! the typed examples declared beside the declaration in [`super::EXAMPLES`], and derives
//! the route each command has on the website. Every projection of the native command line
//! — `--help`, `docs/generated/cli.md`, `docs/generated/cli.json`, the site dataset and
//! `/docs/cli/**` — is a rendering of this one tree, never of `--help` prose and never of
//! a list kept beside the declaration.

use clap::CommandFactory;

use super::{Cli, ExampleDoc};

/// The file the whole command line is declared in; the provenance every projection shows.
pub const DECLARATION: &str = "apps/majordomus-cli/src/cli.rs";

/// The prefix of every command's route on the website. The native command line's reference
/// owns this section; the *shell* tool's specification (`docs/CLI.md`) is a different
/// program and renders elsewhere.
pub const ROUTE_PREFIX: &str = "/docs/cli";

/// The schema of `docs/generated/cli.json`, the machine-readable projection of this tree.
pub const SCHEMA: &str = "majordomus/cli/v1";

/// One command of the executable's command line, as clap declares it, with the commands
/// under it. The root is `majordomus`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommandDoc {
    /// The full path, `["majordomus", "bench", "coverage"]`.
    pub path: Vec<String>,
    /// The route of this command's page, `/docs/cli/bench/coverage/`. Derived here once so
    /// that no consumer downstream repeats the rule.
    pub route: String,
    /// Whether the command can be run on its own: a leaf, or a parent that does something
    /// when no subcommand follows it. A command that only groups others cannot be run and
    /// carries no example.
    pub executable: bool,
    /// The usage line, `majordomus bench coverage [OPTIONS]`, derived here so that no
    /// projection assembles one of its own.
    pub usage: String,
    /// The one-line description.
    pub about: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The long description, when the command has one.
    pub long_about: Option<String>,
    /// The arguments in declaration order: positionals and options, `--help` and
    /// `--version` excluded.
    pub args: Vec<ArgDoc>,
    /// The documented examples of this command, in declaration order. Every one of them is
    /// parsed by this crate's own parser and run against the built executable by the
    /// example tests; nothing is shown that is not run.
    pub examples: Vec<ExampleView>,
    /// The subcommands in declaration order.
    pub subcommands: Vec<CommandDoc>,
}

/// One argument of a command.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ArgDoc {
    /// The argument's id, `transport`.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `--transport`, without the dashes.
    pub long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `-t`, without the dash.
    pub short: Option<char>,
    /// Given by position rather than by flag.
    pub positional: bool,
    /// The help text.
    pub help: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The placeholder for the value, `PATH`.
    pub value_name: Option<String>,
    /// Whether the argument takes a value at all (a flag does not).
    pub takes_value: bool,
    /// Must be given.
    pub required: bool,
    /// Accepted by every command under the one that declares it.
    pub global: bool,
    /// The values a value-enum argument accepts, with the help of each; empty for any
    /// other argument. Always present, so a template can ask for its length.
    pub possible_values: Vec<PossibleValueDoc>,
    /// The default value(s), as clap renders them; empty when there is none.
    pub defaults: Vec<String>,
}

/// One value of a value-enum argument.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PossibleValueDoc {
    /// The value as typed.
    pub name: String,
    /// Its help, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

/// One example, as every projection shows it. The command lines are rendered from the
/// declaration's `argv`, never written out a second time, so what a reader copies is what
/// the example test executed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExampleView {
    /// The example's id, unique across the whole command line.
    pub id: String,
    /// One line: what this example shows.
    pub title: String,
    /// What it does and what comes back.
    pub description: String,
    /// The argument vector, without the executable's own name.
    pub argv: Vec<String>,
    /// The command line as a person types it, rendered from `argv`.
    pub command: String,
    /// The commands that run before it in the same session; empty for an example that
    /// needs nothing prepared.
    pub setup: Vec<SetupView>,
    /// What the example test asserts, in one phrase; the evidence the page may cite.
    pub expectation: String,
}

impl ExampleView {
    fn of(e: &ExampleDoc) -> Self {
        ExampleView {
            id: e.id.to_string(),
            title: e.title.to_string(),
            description: e.description.to_string(),
            argv: e.argv.iter().map(|a| a.to_string()).collect(),
            command: render(e.argv),
            setup: e
                .setup
                .iter()
                .map(|s| SetupView {
                    argv: s.iter().map(|a| a.to_string()).collect(),
                    command: render(s),
                })
                .collect(),
            expectation: e.expect.describe(),
        }
    }
}

/// One command run before an example, in the same session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SetupView {
    /// The argument vector, without the executable's own name.
    pub argv: Vec<String>,
    /// The command line as a person types it, rendered from `argv`.
    pub command: String,
}

/// `["bench", "coverage", "--format", "json"]` as `majordomus bench coverage --format json`.
/// A word that would not survive a shell unquoted is quoted; nothing else is decorated.
pub fn render(argv: &[&str]) -> String {
    let mut s = String::from("majordomus");
    for word in argv {
        s.push(' ');
        if word.is_empty()
            || word.contains(|c: char| c.is_whitespace() || "'\"\\$`*?|&;<>()[]{}#~!".contains(c))
        {
            s.push('\'');
            s.push_str(&word.replace('\'', "'\\''"));
            s.push('\'');
        } else {
            s.push_str(word);
        }
    }
    s
}

/// The route of a command path: `["majordomus", "bench", "coverage"]` becomes
/// `/docs/cli/bench/coverage/`, and the root becomes `/docs/cli/`. The one rule; every
/// other consumer reads [`CommandDoc::route`] rather than repeating it.
pub fn route(path: &[String]) -> String {
    let mut s = String::from(ROUTE_PREFIX);
    for word in path.iter().skip(1) {
        s.push('/');
        s.push_str(word);
    }
    s.push('/');
    s
}

/// The whole command line, from the built clap declaration and the examples declared with
/// it. Deterministic and pure: clap keeps declaration order, and nothing here reads the
/// environment, the repository or the network.
pub fn tree() -> CommandDoc {
    let mut root = Cli::command();
    root.build();
    walk(&root, Vec::new())
}

fn walk(cmd: &clap::Command, mut path: Vec<String>) -> CommandDoc {
    path.push(cmd.get_name().to_string());
    let args: Vec<ArgDoc> = cmd
        .get_arguments()
        .filter(|a| !matches!(a.get_id().as_str(), "help" | "version"))
        .map(|a| ArgDoc {
            name: a.get_id().to_string(),
            long: a.get_long().map(str::to_string),
            short: a.get_short(),
            positional: a.is_positional(),
            help: a.get_help().map(|h| h.to_string()).unwrap_or_default(),
            value_name: a
                .get_value_names()
                .and_then(|names| names.first())
                .map(|n| n.to_string()),
            takes_value: a.get_action().takes_values(),
            required: a.is_required_set(),
            global: a.is_global_set(),
            possible_values: a
                .get_possible_values()
                .into_iter()
                .filter(|v| !v.is_hide_set())
                .map(|v| PossibleValueDoc {
                    name: v.get_name().to_string(),
                    help: v.get_help().map(|h| h.to_string()),
                })
                .collect(),
            defaults: a
                .get_default_values()
                .iter()
                .map(|v| v.to_string_lossy().into_owned())
                .collect(),
        })
        .collect();
    // clap adds a `help` subcommand to every command that has subcommands; it documents
    // nothing of its own and is left out, as `--help` is among the arguments
    let subcommands: Vec<CommandDoc> = cmd
        .get_subcommands()
        .filter(|s| !s.is_hide_set() && s.get_name() != "help")
        .map(|s| walk(s, path.clone()))
        .collect();
    // A command a person can run: one with nothing under it, or one whose subcommand is
    // optional and which therefore does something on its own. Read from clap, never from a
    // list: a command that grows a required subcommand stops needing an example of its own
    // on the same edit that makes it unrunnable.
    let executable = subcommands.is_empty() || !cmd.is_subcommand_required_set();
    let examples = super::EXAMPLES
        .iter()
        .filter(|e| e.command == path[1..].join(" "))
        .flat_map(|e| e.examples.iter())
        .map(ExampleView::of)
        .collect();
    CommandDoc {
        route: route(&path),
        usage: usage(&path, &args, &subcommands, executable),
        path,
        executable,
        about: cmd.get_about().map(|a| a.to_string()).unwrap_or_default(),
        long_about: cmd.get_long_about().map(|a| a.to_string()),
        args,
        examples,
        subcommands,
    }
}

impl CommandDoc {
    /// Every command, this one first, then the subcommands depth first.
    pub fn flatten(&self) -> Vec<&CommandDoc> {
        let mut out = vec![self];
        for s in &self.subcommands {
            out.extend(s.flatten());
        }
        out
    }

    /// The command as a person types it: `majordomus bench coverage`.
    pub fn command(&self) -> String {
        self.path.join(" ")
    }

    /// The route of the command this one is under, or `None` for the root.
    pub fn parent_route(&self) -> Option<String> {
        (self.path.len() > 1).then(|| route(&self.path[..self.path.len() - 1]))
    }
}

/// The usage line of a command: what it is called, that it takes options, whether a
/// subcommand follows it and whether one must, and its positionals in order.
fn usage(path: &[String], args: &[ArgDoc], subcommands: &[CommandDoc], executable: bool) -> String {
    let mut s = path.join(" ");
    if args.iter().any(|a| !a.positional) {
        s.push_str(" [OPTIONS]");
    }
    if !subcommands.is_empty() {
        s.push_str(if executable {
            " [COMMAND]"
        } else {
            " <COMMAND>"
        });
    }
    for a in args.iter().filter(|a| a.positional) {
        let name = a
            .value_name
            .clone()
            .unwrap_or_else(|| a.name.to_uppercase());
        s.push_str(&if a.required {
            format!(" <{name}>")
        } else {
            format!(" [{name}]")
        });
    }
    s
}

/// The machine-readable projection of the whole tree: `docs/generated/cli.json`, the file
/// the website's generator reads. One document, deterministic, with no provenance of its
/// own beyond the crate version.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CliDocument {
    /// [`SCHEMA`].
    pub schema: &'static str,
    /// The crate version that wrote it.
    pub version: String,
    /// [`DECLARATION`]: the file every command in this document is declared in.
    pub source: &'static str,
    /// The route prefix the command routes are under.
    pub route_prefix: &'static str,
    /// The command tree, root first.
    pub cli: CommandDoc,
}

/// The whole tree as the committed JSON projection.
pub fn document(tree: &CommandDoc, version: &str) -> CliDocument {
    CliDocument {
        schema: SCHEMA,
        version: version.to_string(),
        source: DECLARATION,
        route_prefix: ROUTE_PREFIX,
        cli: tree.clone(),
    }
}
