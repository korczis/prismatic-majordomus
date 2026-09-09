//! The semantics of the native command line that its declaration cannot carry.
//!
//! clap declares everything about *shape*: the commands, their arguments, defaults, value
//! sets, help. It cannot declare what running one does to the repository, whether it holds
//! a terminal, or where the values of `<BRANCH>` come from. Those are decisions, and this
//! is where they are made — once, beside the declaration, in the same crate, never again
//! per surface.
//!
//! # Why this is not a second registry
//!
//! Two properties keep it from becoming one:
//!
//! - **The default is the answer for most commands.** This executable is read-only and
//!   non-interactive; [`Semantics::DEFAULT`] says so, and a command that is nothing more
//!   than that appears nowhere below. Only a command that *departs* from the default is
//!   named — which is exactly the information nothing else holds.
//! - **Completeness is enforced in both directions.** `validate` fails when an entry
//!   names a command that does not exist (a rename that left this file behind) and when a
//!   command's effect could not be decided. The table cannot silently go stale, which is
//!   the property that distinguishes a declaration from a mirror.
//!
//! This is the same arrangement the examples beside the command line already use, and it
//! is checked by the same kind of test.

use super::model::{Effect, Interactivity, Requirement, Secrecy, ValueSource};

/// What a command does, beyond what its declaration says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Semantics {
    /// The command path this describes, the executable's own name excluded:
    /// `&["worktree", "remove"]`.
    pub path: &'static [&'static str],
    /// What running it changes.
    pub effect: Effect,
    /// How it behaves towards a terminal.
    pub interactivity: Interactivity,
    /// What must be true for it to mean anything.
    pub requires: &'static [Requirement],
}

impl Semantics {
    /// What a command is when it says nothing: this executable reads and answers.
    pub const DEFAULT: Semantics = Semantics {
        path: &[],
        effect: Effect::ReadOnly,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository, Requirement::Layer],
    };
}

/// Every command that departs from [`Semantics::DEFAULT`].
///
/// Ordered by command path. A command absent from this table is read-only and
/// non-interactive, and needs a repository with a layer in it.
pub const SEMANTICS: &[Semantics] = &[
    // The two servers. Both read; neither returns.
    Semantics {
        path: &["mcp"],
        effect: Effect::ReadOnly,
        interactivity: Interactivity::LongRunning,
        requires: &[Requirement::Repository, Requirement::Layer],
    },
    Semantics {
        path: &["serve"],
        effect: Effect::ReadOnly,
        interactivity: Interactivity::LongRunning,
        requires: &[Requirement::Repository, Requirement::Layer],
    },
    // Generation writes the committed projections.
    Semantics {
        path: &["generate"],
        effect: Effect::RepositoryMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository, Requirement::Layer],
    },
    // The benchmark writes its results and its baseline under the layer's repository half.
    Semantics {
        path: &["bench"],
        effect: Effect::RepositoryMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository, Requirement::Layer],
    },
    Semantics {
        path: &["bench", "coverage"],
        effect: Effect::ReadOnly,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository, Requirement::Layer],
    },
    Semantics {
        path: &["bench", "baseline", "update"],
        effect: Effect::RepositoryMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository, Requirement::Layer],
    },
    // The one command here that writes outside the repository: a person's shell startup
    // file. It needs no repository and no layer — a shell is installed once and serves every
    // checkout — and nothing declares a capability for it, so no machine surface carries it.
    Semantics {
        path: &["completion", "install"],
        effect: Effect::LocalMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[],
    },
    // Raising the version writes two tracked files, which is a repository mutation and is
    // why no capability declares it: the policy keeps repository mutations off every machine
    // surface, so the bump is a command a person runs and nothing reachable over MCP or HTTP.
    Semantics {
        path: &["release", "bump"],
        effect: Effect::RepositoryMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository],
    },
    // Packaging writes into the build directory.
    Semantics {
        path: &["distribution", "build"],
        effect: Effect::LocalMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository, Requirement::RustToolchain],
    },
    // The environment is a property of the checkout; it needs no layer.
    Semantics {
        path: &["env"],
        effect: Effect::ReadOnly,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository],
    },
    // The topology reads git and writes worktrees. It never needs the layer.
    Semantics {
        path: &["worktree"],
        effect: Effect::ReadOnly,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository],
    },
    Semantics {
        path: &["worktree", "create"],
        effect: Effect::RepositoryMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository],
    },
    Semantics {
        path: &["worktree", "ensure"],
        effect: Effect::RepositoryMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository],
    },
    Semantics {
        path: &["worktree", "migrate"],
        effect: Effect::RepositoryMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository],
    },
    Semantics {
        path: &["worktree", "repair"],
        effect: Effect::RepositoryMutation,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository],
    },
    Semantics {
        path: &["worktree", "remove"],
        effect: Effect::Destructive,
        interactivity: Interactivity::NonInteractive,
        requires: &[Requirement::Repository],
    },
];

/// Where the values of an argument come from, when the type the declaration erased is one
/// this repository has a registry for.
///
/// Keyed by the command path and the argument's id. Inference comes first — a value-enum
/// argument carries its own values and a `PATH` placeholder is a path — so only an
/// identifier whose set lives in a registry appears here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueBinding {
    /// The command path the argument belongs to.
    pub path: &'static [&'static str],
    /// The argument's id, as clap knows it.
    pub argument: &'static str,
    /// Where its values come from.
    pub source: ValueSource,
}

/// Every argument whose values a registry here knows.
pub const VALUE_BINDINGS: &[ValueBinding] = &[
    ValueBinding {
        path: &["capabilities", "describe"],
        argument: "id",
        source: ValueSource::Capability,
    },
    ValueBinding {
        path: &["capabilities", "schema"],
        argument: "id",
        source: ValueSource::Capability,
    },
    ValueBinding {
        path: &["why", "show"],
        argument: "id",
        source: ValueSource::Moment,
    },
    ValueBinding {
        path: &["worktree", "path"],
        argument: "branch",
        source: ValueSource::Branch,
    },
    ValueBinding {
        path: &["worktree", "inspect"],
        argument: "branch",
        source: ValueSource::Branch,
    },
    ValueBinding {
        path: &["worktree", "create"],
        argument: "branch",
        source: ValueSource::Branch,
    },
    ValueBinding {
        path: &["worktree", "ensure"],
        argument: "branch",
        source: ValueSource::Branch,
    },
    ValueBinding {
        path: &["worktree", "remove"],
        argument: "branch",
        source: ValueSource::Branch,
    },
    ValueBinding {
        path: &["commands", "show"],
        argument: "id",
        source: ValueSource::Command,
    },
    ValueBinding {
        path: &["commands", "explain"],
        argument: "id",
        source: ValueSource::Command,
    },
    ValueBinding {
        path: &["web", "explain"],
        argument: "id",
        source: ValueSource::Graph,
    },
];

/// A name the workflow runner answered to before the bridge was derived, kept so that no
/// recipe a person or a script used disappears.
///
/// An alias carries no description, no group and no semantics of its own: it names a
/// command of the graph and, where the older spelling meant a command *with* an argument,
/// the arguments it always passed. Everything a projection shows about it comes from the
/// command it names. [`unused_aliases`] fails the build when an alias names a command that
/// no longer exists, so a rename cannot leave a recipe pointing at nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowAlias {
    /// The recipe name, as it was.
    pub alias: &'static str,
    /// The command it resolves to, by canonical identity.
    pub command: &'static str,
    /// The arguments the older spelling always passed, before the caller's own.
    pub arguments: &'static [&'static str],
    /// Why it exists, in one line, for the generated file and the reference to print.
    pub reason: &'static str,
}

/// Every workflow name that predates the generated bridge.
///
/// This is the compatibility inventory of the migration, and it is the only place it
/// exists: the bridge renders these, the reference prints them, and nothing else keeps a
/// table of old names.
pub const WORKFLOW_ALIASES: &[WorkflowAlias] = &[
    WorkflowAlias {
        alias: "capabilities",
        command: "executable.capabilities.list",
        arguments: &[],
        reason: "the recipe before the bridge derived `capabilities-list` from the command",
    },
    WorkflowAlias {
        alias: "describe",
        command: "executable.capabilities.describe",
        arguments: &[],
        reason: "the recipe before the bridge derived `capabilities-describe`",
    },
    WorkflowAlias {
        alias: "validate",
        command: "executable.capabilities.validate",
        arguments: &[],
        reason: "the recipe before the bridge derived `capabilities-validate`",
    },
    WorkflowAlias {
        alias: "generate-check",
        command: "executable.generate",
        arguments: &["--check"],
        reason: "the recipe that spelled `generate --check`",
    },
    WorkflowAlias {
        alias: "inspect",
        command: "executable.mcp",
        arguments: &["--inspect"],
        reason: "the recipe that spelled `mcp --inspect`",
    },
    WorkflowAlias {
        alias: "bench-run",
        command: "executable.bench",
        arguments: &[],
        reason: "the recipe before `bench` was qualified by the program that owns it",
    },
    WorkflowAlias {
        alias: "bench-check",
        command: "executable.bench",
        arguments: &["--profile", "ci", "--check", "--no-write"],
        reason: "the recipe that spelled the continuous-integration benchmark run",
    },
    WorkflowAlias {
        alias: "bench-baseline",
        command: "executable.bench.baseline.update",
        arguments: &[],
        reason: "the recipe before the bridge derived `bench-baseline-update`",
    },
    WorkflowAlias {
        alias: "wt",
        command: "executable.worktree",
        arguments: &[],
        reason: "the command line's own alias for `worktree`, kept for the runner too",
    },
    WorkflowAlias {
        alias: "wt-create",
        command: "executable.worktree.ensure",
        arguments: &[],
        reason: "the recipe that created a worktree if it was absent and answered it if not",
    },
    WorkflowAlias {
        alias: "wt-migrate",
        command: "executable.worktree.migrate",
        arguments: &[],
        reason: "the recipe before the bridge derived `worktree-migrate`",
    },
    WorkflowAlias {
        alias: "wt-doctor",
        command: "executable.worktree.doctor",
        arguments: &[],
        reason: "the recipe before the bridge derived `worktree-doctor`",
    },
    WorkflowAlias {
        alias: "usecase-list",
        command: "tool.usecase",
        arguments: &["list"],
        reason: "the recipe that spelled one subcommand of the shell tool's `usecase`",
    },
    WorkflowAlias {
        alias: "usecase-run",
        command: "tool.usecase",
        arguments: &["run"],
        reason: "the recipe that spelled one subcommand of the shell tool's `usecase`",
    },
    WorkflowAlias {
        alias: "usecase-coverage",
        command: "tool.usecase",
        arguments: &["coverage", "--check"],
        reason: "the recipe that spelled the use-case coverage gate",
    },
    WorkflowAlias {
        alias: "usecase-impact",
        command: "tool.usecase",
        arguments: &["impact"],
        reason: "the recipe that spelled one subcommand of the shell tool's `usecase`",
    },
];

/// Every alias that names a command the graph does not carry.
pub fn unused_aliases(existing: &[String]) -> Vec<&'static WorkflowAlias> {
    WORKFLOW_ALIASES
        .iter()
        .filter(|a| !existing.iter().any(|id| id == a.command))
        .collect()
}

/// The semantics of one command: the most specific entry that names it, or the most
/// specific entry that names a prefix of it, or the default.
///
/// Inheritance down the tree is what keeps the table short: `worktree` says the whole
/// topology needs no layer once, and every command under it is answered by that.
pub fn of(path: &[String]) -> Semantics {
    let mut best: Option<&Semantics> = None;
    for entry in SEMANTICS {
        if entry.path.len() <= path.len()
            && entry
                .path
                .iter()
                .zip(path.iter())
                .all(|(a, b)| *a == b.as_str())
            && best.is_none_or(|b| b.path.len() < entry.path.len())
        {
            best = Some(entry);
        }
    }
    best.copied().unwrap_or(Semantics::DEFAULT)
}

/// Where an argument's values come from, when a binding names it.
pub fn binding(path: &[String], argument: &str) -> Option<ValueSource> {
    VALUE_BINDINGS
        .iter()
        .find(|b| {
            b.argument == argument
                && b.path.len() == path.len()
                && b.path
                    .iter()
                    .zip(path.iter())
                    .all(|(a, c)| *a == c.as_str())
        })
        .map(|b| b.source)
}

/// How openly an argument's value may be handled, inferred from what it is called.
///
/// Nothing in this executable takes a credential today. The classification exists so that
/// the day one does, no completion, log or cache has to be taught to leave it alone.
pub fn secrecy(argument: &str, value_name: Option<&str>) -> Secrecy {
    let hay = format!(
        "{argument} {}",
        value_name.unwrap_or_default().to_lowercase()
    );
    const SECRET: &[&str] = &["token", "secret", "password", "passwd", "api-key", "apikey"];
    const SENSITIVE: &[&str] = &["credential", "auth", "key"];
    if SECRET.iter().any(|w| hay.contains(w)) {
        Secrecy::Secret
    } else if SENSITIVE.iter().any(|w| hay.contains(w)) {
        Secrecy::Sensitive
    } else {
        Secrecy::Public
    }
}

/// Every entry names a command that exists.
///
/// The failure this catches is the one that makes a table like this dangerous: a command
/// is renamed, the entry beside it is not, and the command silently falls back to the
/// default — which for `worktree remove` would mean a destructive command advertised to
/// every machine surface. Returns the offending paths.
pub fn unused(existing: &[Vec<String>]) -> Vec<String> {
    let mut out = Vec::new();
    for entry in SEMANTICS {
        if !existing
            .iter()
            .any(|p| p.len() == entry.path.len() && p.iter().zip(entry.path).all(|(a, b)| a == b))
        {
            out.push(entry.path.join(" "));
        }
    }
    for binding in VALUE_BINDINGS {
        if !existing.iter().any(|p| {
            p.len() == binding.path.len() && p.iter().zip(binding.path).all(|(a, b)| a == b)
        }) {
            out.push(format!("{} <{}>", binding.path.join(" "), binding.argument));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn the_default_answers_an_unnamed_command() {
        let s = of(&p(&["scope"]));
        assert_eq!(s.effect, Effect::ReadOnly);
        assert_eq!(s.interactivity, Interactivity::NonInteractive);
    }

    #[test]
    fn the_most_specific_entry_wins() {
        assert_eq!(of(&p(&["bench"])).effect, Effect::RepositoryMutation);
        assert_eq!(of(&p(&["bench", "coverage"])).effect, Effect::ReadOnly);
        assert_eq!(
            of(&p(&["bench", "baseline", "update"])).effect,
            Effect::RepositoryMutation
        );
    }

    #[test]
    fn semantics_are_inherited_down_the_tree() {
        // `worktree` declares that the topology needs no layer; `worktree list` inherits it.
        let s = of(&p(&["worktree", "list"]));
        assert_eq!(s.requires, &[Requirement::Repository]);
        assert_eq!(s.effect, Effect::ReadOnly);
        assert_eq!(of(&p(&["worktree", "remove"])).effect, Effect::Destructive);
    }

    #[test]
    fn a_binding_is_exact_not_inherited() {
        assert_eq!(
            binding(&p(&["worktree", "path"]), "branch"),
            Some(ValueSource::Branch)
        );
        assert_eq!(binding(&p(&["worktree"]), "branch"), None);
    }

    #[test]
    fn secrets_are_recognised_by_name() {
        assert_eq!(secrecy("token", None), Secrecy::Secret);
        assert_eq!(secrecy("api_key", Some("API-KEY")), Secrecy::Secret);
        assert_eq!(secrecy("branch", Some("BRANCH")), Secrecy::Public);
    }

    #[test]
    fn an_entry_naming_nothing_is_reported() {
        let existing = vec![p(&["mcp"])];
        let stale = unused(&existing);
        assert!(stale.contains(&"serve".to_string()));
    }
}
