//! Why a command of the command line is not itself a capability.
//!
//! # The question this answers
//!
//! Every capability of this executable is projected to HTTP and to the OpenAPI document,
//! and most of them to MCP. The command line is the one projection that also carries
//! commands of its own: starting a server, writing a generated file, rendering a value for
//! a person. Left alone, that is how an operation goes missing from the API — not by
//! decision, but because nobody noticed it had.
//!
//! So the rule is not *every command is a capability*. It is: **a command is the
//! projection of a capability, or it says here why it is not, and the reason is checked.**
//! [`LOCAL`] is that statement, and [`crate::quality::parity`] is the check. A command in
//! neither place is a finding; an entry here naming a command that no longer exists is a
//! finding; an entry here for a command that *is* bound to a capability is a finding. None
//! of the three can be reached by forgetting.
//!
//! # Where it lives
//!
//! Beside the clap declaration it is about, for the same reason
//! [`super::EXAMPLES`] does: the person adding a command is looking at this file, and a
//! statement kept anywhere else is a statement that goes stale in a refactor nobody
//! connects to it.
//!
//! ```
//! use majordomus_cli::cli::local::{LocalReason, LOCAL};
//!
//! // starting a server is not an operation that can answer over a socket
//! let serve = LOCAL.iter().find(|l| l.command == "serve").unwrap();
//! assert!(matches!(serve.reason, LocalReason::ProcessLifecycle));
//!
//! // and a command that renders a capability names the capability it renders
//! let list = LOCAL.iter().find(|l| l.command == "web list").unwrap();
//! assert_eq!(list.reason.renders(), Some("web.surfaces"));
//! ```

/// Why a command belongs to the command line alone.
///
/// The variants are the four structural reasons this executable has, and there is no
/// fifth for "not projected yet": a read that belongs in the API and is missing from it
/// is a defect, and a vocabulary that can express it politely is a vocabulary that will.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalReason {
    /// The command runs a default subcommand when none is given: it is that command's
    /// other name, not an operation of its own. The target is the full command, as a
    /// person types it after `majordomus`.
    Alias(&'static str),
    /// What the command produces is a running process, not a value. There is nothing for a
    /// request/response projection to return, and the caller of such a projection would be
    /// asking the server to become a different server.
    ProcessLifecycle,
    /// The command writes into the repository. Every projection of the registry is
    /// read-only — that is the executable's first promise — so a command that writes is
    /// offered to the person who runs the executable and to nobody over a network.
    WritesRepository,
    /// The command answers about the working copy and the shell that invoked it. A caller
    /// over HTTP or MCP does not share either, so the same question asked over a socket
    /// would be a different question with the same name, which is worse than not asking it.
    SessionLocal,
    /// The command renders, for a person at a terminal, the value the named capability
    /// answers. The operation is in the API; this is its terminal rendering. The domain
    /// logic is shared — the command and the capability read the same resolution — and
    /// only the shaping of the output is the command's own.
    RendersCapability(&'static str),
}

impl LocalReason {
    /// The capability this command renders, when it renders one.
    ///
    /// ```
    /// use majordomus_cli::cli::local::LocalReason;
    /// assert_eq!(LocalReason::RendersCapability("web.surfaces").renders(), Some("web.surfaces"));
    /// assert_eq!(LocalReason::ProcessLifecycle.renders(), None);
    /// ```
    pub fn renders(self) -> Option<&'static str> {
        match self {
            LocalReason::RendersCapability(id) => Some(id),
            _ => None,
        }
    }

    /// The command this one is another name for, when it is.
    ///
    /// ```
    /// use majordomus_cli::cli::local::LocalReason;
    /// assert_eq!(LocalReason::Alias("why list").aliases(), Some("why list"));
    /// assert_eq!(LocalReason::SessionLocal.aliases(), None);
    /// ```
    pub fn aliases(self) -> Option<&'static str> {
        match self {
            LocalReason::Alias(target) => Some(target),
            _ => None,
        }
    }

    /// The short word a report and the generated reference show.
    ///
    /// ```
    /// use majordomus_cli::cli::local::LocalReason;
    /// assert_eq!(LocalReason::WritesRepository.as_str(), "writes_repository");
    /// assert_eq!(LocalReason::Alias("why list").as_str(), "alias");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            LocalReason::Alias(_) => "alias",
            LocalReason::ProcessLifecycle => "process_lifecycle",
            LocalReason::WritesRepository => "writes_repository",
            LocalReason::SessionLocal => "session_local",
            LocalReason::RendersCapability(_) => "renders_capability",
        }
    }
}

/// One command of the command line that is not itself a capability, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalCommand {
    /// The command as a person types it after `majordomus`: `bench baseline update`.
    pub command: &'static str,
    /// Why it is not a capability.
    pub reason: LocalReason,
    /// One sentence about this command in particular. The reason says which of four
    /// structural cases it is; this says what a reader of *this* command needs to know.
    pub note: &'static str,
}

/// Every command of the command line that is not the projection of a capability.
///
/// Checked in both directions by [`crate::quality::parity`]: a runnable command that is
/// neither bound to a capability nor listed here is a finding, and an entry here that
/// names no command, or names one that is bound, is a finding too.
///
/// ```
/// use majordomus_cli::cli::local::LOCAL;
/// // every entry says something specific, not just which case it is
/// assert!(LOCAL.iter().all(|l| l.note.split_whitespace().count() >= 6));
/// ```
pub const LOCAL: &[LocalCommand] = &[
    // ---------------------------------------------------------------- servers
    LocalCommand {
        command: "mcp",
        reason: LocalReason::ProcessLifecycle,
        note: "becomes the stdio MCP server for the client that spawned it; the capabilities it then serves are the projection.",
    },
    LocalCommand {
        command: "serve",
        reason: LocalReason::ProcessLifecycle,
        note: "becomes the HTTP server that serves every capability over the API this command exists to start.",
    },
    LocalCommand {
        command: "serve ensure",
        reason: LocalReason::ProcessLifecycle,
        note: "starts the HTTP server for this checkout as a process of its own when none answers, and waits until it is ready.",
    },
    LocalCommand {
        command: "serve stop",
        reason: LocalReason::ProcessLifecycle,
        note: "ends the server this checkout's lease names by signalling the process, which nothing served by that process could do to itself.",
    },
    // ---------------------------------------------------------------- writers
    LocalCommand {
        command: "generate",
        reason: LocalReason::WritesRepository,
        note: "writes the committed projections of the registry into the working tree, which a read-only API never offers.",
    },
    LocalCommand {
        command: "bench",
        reason: LocalReason::WritesRepository,
        note: "times every capability through every transport and writes the results; a measurement served over a transport would be measuring itself.",
    },
    LocalCommand {
        command: "bench baseline update",
        reason: LocalReason::WritesRepository,
        note: "accepts a measurement as the baseline of this platform by writing it into the layer, which is a deliberate act of a person.",
    },
    LocalCommand {
        command: "web compose",
        reason: LocalReason::WritesRepository,
        note: "assembles the publication into a destination directory, writing every surface's artifact into it.",
    },
    LocalCommand {
        command: "web report tests",
        reason: LocalReason::WritesRepository,
        note: "writes the test report surface from a run's output, which is a producer of the web root rather than a reading of it.",
    },
    LocalCommand {
        command: "web report benchmarks",
        reason: LocalReason::WritesRepository,
        note: "writes the benchmark report surface from a results file, which is a producer of the web root rather than a reading of it.",
    },
    LocalCommand {
        command: "web report ui",
        reason: LocalReason::WritesRepository,
        note: "writes the UI conformance report surface from a probe's output, which is a producer of the web root rather than a reading of it.",
    },
    LocalCommand {
        command: "worktree create",
        reason: LocalReason::WritesRepository,
        note: "creates a branch and a linked worktree on disk at the path the topology derives for it.",
    },
    LocalCommand {
        command: "worktree ensure",
        reason: LocalReason::WritesRepository,
        note: "creates the canonical worktree of a branch when it is absent, which is the same write made idempotent.",
    },
    LocalCommand {
        command: "worktree repair",
        reason: LocalReason::WritesRepository,
        note: "rewrites the git administrative files of a worktree whose links no longer resolve.",
    },
    LocalCommand {
        command: "worktree remove",
        reason: LocalReason::WritesRepository,
        note: "removes a linked worktree from disk and from the repository's administrative data.",
    },
    LocalCommand {
        command: "worktree cleanup",
        reason: LocalReason::WritesRepository,
        note: "prunes the worktrees git still records and whose directories are gone.",
    },
    // ---------------------------------------------------------------- the caller's own checkout
    LocalCommand {
        command: "worktree root",
        reason: LocalReason::SessionLocal,
        note: "answers where the repository containing the current directory begins, which a networked caller does not share.",
    },
    LocalCommand {
        command: "worktree guard",
        reason: LocalReason::SessionLocal,
        note: "decides whether the commit being made in this checkout may proceed, and is invoked by this checkout's pre-commit hook.",
    },
    // ---------------------------------------------------------------- other names
    LocalCommand {
        command: "why",
        reason: LocalReason::Alias("why list"),
        note: "runs the catalogue listing when no subcommand follows it, so the two are one operation typed two ways.",
    },
    LocalCommand {
        command: "web",
        reason: LocalReason::Alias("web list"),
        note: "runs the surface listing when no subcommand follows it, so the two are one operation typed two ways.",
    },
    LocalCommand {
        command: "distribution",
        reason: LocalReason::Alias("distribution show"),
        note: "runs the model listing when no subcommand follows it, so the two are one operation typed two ways.",
    },
    LocalCommand {
        command: "worktree",
        reason: LocalReason::Alias("worktree status"),
        note: "runs the status of this checkout when no subcommand follows it, so the two are one operation typed two ways.",
    },
    // ---------------------------------------------------------------- renderings of a capability
    LocalCommand {
        command: "capabilities schema",
        reason: LocalReason::RendersCapability("capabilities.describe"),
        note: "prints the canonical input or output schema that capabilities.describe already carries on the descriptor it returns.",
    },
    LocalCommand {
        command: "mesh status",
        reason: LocalReason::RendersCapability("mesh.status"),
        note: "asks this checkout's running server for mesh.status and renders it: the mesh lives in the server's memory, so an in-process answer would truthfully say only that this process runs no mesh.",
    },
    LocalCommand {
        command: "mesh nodes",
        reason: LocalReason::RendersCapability("mesh.nodes"),
        note: "asks this checkout's running server for mesh.nodes and renders it, for the same reason as mesh status: the registry is the server's, not this process's.",
    },
    LocalCommand {
        command: "capabilities validate",
        reason: LocalReason::RendersCapability("repository.info"),
        note: "reports the registry's own validation, which repository.info answers as the diagnostics of the process that built it.",
    },
    LocalCommand {
        command: "bench coverage",
        reason: LocalReason::RendersCapability("capabilities.list"),
        note: "reports which capabilities are benchmark targets and which are waived, from the benchmark policy capabilities.list carries on every descriptor.",
    },
    LocalCommand {
        command: "web list",
        reason: LocalReason::RendersCapability("web.surfaces"),
        note: "prints the resolved topology web.surfaces answers, in the same route-precedence order and from the same resolution.",
    },
    LocalCommand {
        command: "web explain",
        reason: LocalReason::RendersCapability("web.surfaces"),
        note: "prints one surface of that same resolved topology, with the provenance of every value on it.",
    },
    LocalCommand {
        command: "web validate",
        reason: LocalReason::RendersCapability("web.surfaces"),
        note: "prints the findings web.surfaces already carries, and can additionally require each artifact to be present on this disk.",
    },
    LocalCommand {
        command: "web manifest",
        reason: LocalReason::RendersCapability("web.surfaces"),
        note: "prints the routing manifest derived from the same resolved topology, for a static host to consume.",
    },
    LocalCommand {
        command: "distribution validate",
        reason: LocalReason::RendersCapability("distribution.model"),
        note: "prints the findings of the distribution model that distribution.model returns with it.",
    },
    LocalCommand {
        command: "distribution targets",
        reason: LocalReason::RendersCapability("distribution.model"),
        note: "prints the platforms of the model distribution.model returns, one line each.",
    },
    LocalCommand {
        command: "distribution matrix",
        reason: LocalReason::RendersCapability("distribution.model"),
        note: "prints those same platforms in the shape a CI job's matrix consumes.",
    },
    LocalCommand {
        command: "distribution metadata",
        reason: LocalReason::RendersCapability("distribution.model"),
        note: "prints the packaging metadata of the model, which is the same value seen field by field.",
    },
    LocalCommand {
        command: "worktree list",
        reason: LocalReason::RendersCapability("worktree.topology"),
        note: "prints the registered worktrees the topology holds, one line each.",
    },
    LocalCommand {
        command: "worktree path",
        reason: LocalReason::RendersCapability("worktree.topology"),
        note: "prints the canonical path the topology derives for a branch, for a shell to interpolate.",
    },
    LocalCommand {
        command: "worktree validate",
        reason: LocalReason::RendersCapability("worktree.topology"),
        note: "prints the topology's findings and exits non-zero on one, which is the same value read as a verdict.",
    },
    LocalCommand {
        command: "worktree doctor",
        reason: LocalReason::RendersCapability("worktree.topology"),
        note: "prints those findings with the command that repairs each one beside it.",
    },
    LocalCommand {
        command: "worktree branches",
        reason: LocalReason::RendersCapability("worktree.topology"),
        note: "prints the branches of the repository with the worktree each belongs in, derived from the same topology.",
    },
    // ---------------------------------------------------------------- the command graph
    // Three landings — the command graph (#117), the environment and executions (#142) and
    // the release module — each added runnable commands without saying here why they were
    // not capabilities, and the parity check found all twenty-one at once the day a CI run
    // finished. Every entry below was read off the command's own dispatch; none is a guess.
    LocalCommand {
        command: "commands",
        reason: LocalReason::Alias("commands list"),
        note: "with nothing after it, lists the graph.",
    },
    LocalCommand {
        command: "commands list",
        reason: LocalReason::RendersCapability("commands.list"),
        note: "prints the command graph as a table, one row per command with where it is projected.",
    },
    LocalCommand {
        command: "commands show",
        reason: LocalReason::RendersCapability("commands.get"),
        note: "prints one command's node: its arguments, its origin, its projections.",
    },
    LocalCommand {
        command: "commands explain",
        reason: LocalReason::RendersCapability("commands.get"),
        note: "prints the same node with the provenance of every fact beside it.",
    },
    LocalCommand {
        command: "commands graph",
        reason: LocalReason::RendersCapability("commands.graph"),
        note: "prints the whole graph document, the value the capability answers.",
    },
    LocalCommand {
        command: "commands bridge",
        reason: LocalReason::WritesRepository,
        note: "writes the workflow bridge the graph derives into .ai/local/cache/, which a read-only API never offers.",
    },
    // ---------------------------------------------------------------- completion
    LocalCommand {
        command: "completion",
        reason: LocalReason::Alias("completion init"),
        note: "with nothing after it, prints the integration for the current shell.",
    },
    LocalCommand {
        command: "completion init",
        reason: LocalReason::SessionLocal,
        note: "prints the shell snippet that turns TAB into a query of this executable; which shell is a fact of the terminal that asked.",
    },
    LocalCommand {
        command: "completion query",
        reason: LocalReason::SessionLocal,
        note: "answers the candidates for a partial command line the shell is holding, which no caller over a socket has.",
    },
    LocalCommand {
        command: "completion install",
        reason: LocalReason::WritesRepository,
        note: "writes the integration into the person's shell rc file, backing up what was there; a deliberate act at a terminal.",
    },
    // ---------------------------------------------------------------- environment
    LocalCommand {
        command: "env",
        reason: LocalReason::Alias("env status"),
        note: "with nothing after it, reports the checkout.",
    },
    LocalCommand {
        command: "env status",
        reason: LocalReason::RendersCapability("environment.status"),
        note: "prints what this checkout is, as a table.",
    },
    LocalCommand {
        command: "env explain",
        reason: LocalReason::RendersCapability("environment.explain"),
        note: "prints the same facts with where each one was read from.",
    },
    LocalCommand {
        command: "env banner",
        reason: LocalReason::SessionLocal,
        note: "prints the one-line banner direnv shows on entering this worktree; it is about the shell that just arrived.",
    },
    LocalCommand {
        command: "env export",
        reason: LocalReason::SessionLocal,
        note: "prints the environment variables for the shell to evaluate, which is the shell's own state and nobody else's.",
    },
    // ---------------------------------------------------------------- executions
    LocalCommand {
        command: "executions",
        reason: LocalReason::Alias("executions list"),
        note: "with nothing after it, lists the executions.",
    },
    // ---------------------------------------------------------------- product
    LocalCommand {
        command: "product",
        reason: LocalReason::Alias("product list"),
        note: "with nothing after it, lists the features.",
    },
    // ---------------------------------------------------------------- release
    LocalCommand {
        command: "release",
        reason: LocalReason::Alias("release changelog"),
        note: "with nothing after it, prints the changelog.",
    },
    LocalCommand {
        command: "release changelog",
        reason: LocalReason::RendersCapability("release.changelog"),
        note: "prints the changelog as Markdown, the same document the capability answers as data.",
    },
    LocalCommand {
        command: "release version",
        reason: LocalReason::RendersCapability("release.version"),
        note: "prints the declared and the tool's version and whether they agree.",
    },
    LocalCommand {
        command: "release bump",
        reason: LocalReason::WritesRepository,
        note: "raises the version in both places it is written; the one writer, and a deliberate act.",
    },
];

/// The entry for a command, by the words a person types after `majordomus`.
///
/// ```
/// use majordomus_cli::cli::local::find;
/// assert!(find("serve").is_some());
/// assert!(find("capabilities list").is_none(), "that one is a capability");
/// ```
pub fn find(command: &str) -> Option<&'static LocalCommand> {
    LOCAL.iter().find(|l| l.command == command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn no_command_is_classified_twice() {
        let mut seen = BTreeSet::new();
        for l in LOCAL {
            assert!(seen.insert(l.command), "{} is listed twice", l.command);
        }
    }

    #[test]
    fn every_alias_names_a_command_that_is_itself_accounted_for() {
        // an alias of an unclassified command would move the question rather than answer it
        for l in LOCAL {
            let Some(target) = l.reason.aliases() else {
                continue;
            };
            assert_ne!(target, l.command, "{} aliases itself", l.command);
            // the target is either a capability's command or classified in its own right;
            // parity proves the first, and this proves the entry is not circular
            assert!(
                find(target).is_none_or(|t| t.reason.aliases().is_none()),
                "{} aliases {target}, which is itself an alias",
                l.command
            );
        }
    }

    #[test]
    fn a_rendering_names_a_capability_and_never_a_command() {
        for l in LOCAL {
            let Some(id) = l.reason.renders() else {
                continue;
            };
            assert!(
                id.contains('.') && !id.contains(' '),
                "{} renders '{id}', which is not a capability id",
                l.command
            );
        }
    }
}
