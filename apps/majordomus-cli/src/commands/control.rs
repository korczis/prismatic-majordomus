//! `majordomus commands` and `majordomus completion`: the canonical command graph, read,
//! and the engine a shell adapter asks.
//!
//! Both are in the lightweight class. Neither builds the repository's index, and neither
//! composes a server: the graph is a walk of the clap declaration and the builtin registry,
//! which is what lets a keystroke pay for it. Discovering the workflows `just` holds is the
//! one thing that spawns a process, and it happens only when it is asked for.

use std::io::Write;

use crate::cli::{
    CommandsArgs, CommandsCommand, CompletionArgs, CompletionCommand, CompletionFormat,
    CompletionShell, CompletionSurface, OutputFormat, ProjectionSurface,
};
use crate::control::completion::{self, Candidate, Request};
use crate::control::graph::{self, CommandGraph, CommandNode, Execution};
use crate::control::{just, Surface};
use crate::error::{Error, Result};

/// The exit code when the graph itself is invalid: a collision, or a command that does not
/// say what running it changes.
pub const EXIT_INVALID_GRAPH: u8 = 10;

/// The exit code when a command id names nothing.
pub const EXIT_NO_SUCH_COMMAND: u8 = 4;

/// The adapters, shipped with the distribution and printed on request. They are static on
/// purpose: an adapter that carried a command would have to be regenerated, and this one
/// never does.
const ZSH: &str = include_str!("../../../../share/completion/majordomus.zsh");
const BASH: &str = include_str!("../../../../share/completion/majordomus.bash");

/// Run `majordomus commands`.
pub fn run(args: CommandsArgs) -> Result<u8> {
    let graph = build(args.workflows);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command.unwrap_or(CommandsCommand::List) {
        CommandsCommand::List => list(&mut out, &graph, args.format)?,
        CommandsCommand::Explain { id } => {
            let Some(node) = graph.find(&id) else {
                writeln!(out, "no command '{id}'").map_err(Error::Transport)?;
                return Ok(EXIT_NO_SUCH_COMMAND);
            };
            explain(&mut out, node, args.format)?;
        }
        CommandsCommand::Graph => {
            let text = serde_json::to_string_pretty(&graph)
                .map_err(|e| Error::Transport(std::io::Error::other(e)))?;
            writeln!(out, "{text}").map_err(Error::Transport)?;
        }
        CommandsCommand::Projection { surface } => projection(&mut out, &graph, surface)?,
        CommandsCommand::Materialise => {
            let root = repository_root();
            let state = just::materialise(&root, &graph).map_err(Error::Transport)?;
            let path = root.join(just::BRIDGE_PATH);
            writeln!(
                out,
                "{} {}",
                match state {
                    just::Materialised::Written => "wrote",
                    just::Materialised::Current => "current",
                },
                path.display()
            )
            .map_err(Error::Transport)?;
        }
    }
    for d in graph.diagnostics.iter().filter(|d| d.fatal) {
        writeln!(out, "FAIL {}  {}  {}", d.code, d.subject, d.detail).map_err(Error::Transport)?;
    }
    Ok(if graph.is_valid() {
        0
    } else {
        EXIT_INVALID_GRAPH
    })
}

/// The repository this call is standing in, or the directory itself when it is not one.
fn repository_root() -> std::path::PathBuf {
    let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    crate::repository::Repository::discover(&here)
        .map(|r| r.root().to_path_buf())
        .unwrap_or(here)
}

/// The graph, with the repository's own workflows when they were asked for.
fn build(workflows: bool) -> CommandGraph {
    if !workflows {
        return graph::of_this_executable();
    }
    let root = repository_root();
    let discovered = crate::control::workflow::discover(&root).unwrap_or_default();
    let registry = crate::capability::CapabilityRegistry::builder()
        .with_modules(crate::capability::builtin::modules())
        .build()
        .unwrap_or_default();
    graph::build(&crate::cli::tree(), &registry, &discovered)
}

fn list(out: &mut impl Write, graph: &CommandGraph, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(&graph.commands)
                .map_err(|e| Error::Transport(std::io::Error::other(e)))?;
            writeln!(out, "{text}").map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            for c in &graph.commands {
                if c.group {
                    continue;
                }
                let effect = c.semantics.map(|s| s.effect.label()).unwrap_or("?");
                let surfaces = surfaces(c).join(" ");
                writeln!(out, "{:<28} {:<12} {}", c.id, effect, surfaces)
                    .map_err(Error::Transport)?;
                writeln!(out, "{:<28} {}", "", c.summary).map_err(Error::Transport)?;
            }
        }
    }
    Ok(())
}

/// The surfaces one command actually appears on, in a fixed order.
fn surfaces(c: &CommandNode) -> Vec<String> {
    let mut out = Vec::new();
    if !c.projections.cli.is_empty() {
        out.push("cli".to_string());
    }
    if c.projections.just.is_some() {
        out.push("just".to_string());
    }
    if c.projections.mcp.is_some() {
        out.push("mcp".to_string());
    }
    if c.projections.http.is_some() {
        out.push("http".to_string());
    }
    if c.projections.cockpit.is_some() {
        out.push("cockpit".to_string());
    }
    out
}

fn explain(out: &mut impl Write, node: &CommandNode, format: OutputFormat) -> Result<()> {
    if let OutputFormat::Json = format {
        let text = serde_json::to_string_pretty(node)
            .map_err(|e| Error::Transport(std::io::Error::other(e)))?;
        writeln!(out, "{text}").map_err(Error::Transport)?;
        return Ok(());
    }
    fn w(out: &mut impl Write, label: &str, value: String) -> Result<()> {
        writeln!(out, "{label:<14}{value}").map_err(Error::Transport)
    }
    w(out, "command", node.id.clone())?;
    w(out, "summary", node.summary.clone())?;
    match &node.execution {
        Execution::Native { capability } => w(
            out,
            "runs",
            match capability {
                Some(id) => format!("this executable, through the capability {id}"),
                None => "this executable".to_string(),
            },
        )?,
        Execution::External { runner, source } => w(out, "runs", format!("{runner} ({source})"))?,
    }
    if let Some(s) = node.semantics {
        w(
            out,
            "effect",
            format!("{} — {}", s.effect.label(), s.effect.describe()),
        )?;
        w(out, "caller", s.interactivity.label().to_string())?;
        match s.refusal() {
            Some(reason) => w(out, "machines", format!("not offered: {reason}"))?,
            None => w(out, "machines", "offered".to_string())?,
        }
    }
    w(out, "declared in", node.provenance.clone())?;
    writeln!(out, "\nprojections").map_err(Error::Transport)?;
    if !node.projections.cli.is_empty() {
        w(
            out,
            "  cli",
            format!("majordomus {}", node.projections.cli.join(" ")),
        )?;
    }
    if let Some(name) = &node.projections.just {
        w(out, "  just", format!("just {name}"))?;
    }
    match &node.projections.mcp {
        Some(name) => w(out, "  mcp", name.clone())?,
        None => w(
            out,
            "  mcp",
            format!(
                "none — {}",
                node.semantics
                    .and_then(|s| s.refusal())
                    .unwrap_or_else(|| "it is addressed through its subcommands".into())
            ),
        )?,
    }
    if let Some(route) = &node.projections.http {
        w(out, "  http", route.clone())?;
    }
    if let Some(action) = &node.projections.cockpit {
        w(out, "  cockpit", action.clone())?;
    }
    w(out, "  docs", node.projections.docs.clone())?;
    Ok(())
}

fn projection(
    out: &mut impl Write,
    graph: &CommandGraph,
    surface: ProjectionSurface,
) -> Result<()> {
    match surface {
        ProjectionSurface::Just => {
            write!(out, "{}", just::render(graph)).map_err(Error::Transport)?;
        }
        ProjectionSurface::Cli => {
            for c in graph
                .commands
                .iter()
                .filter(|c| !c.projections.cli.is_empty())
            {
                writeln!(out, "majordomus {}", c.projections.cli.join(" "))
                    .map_err(Error::Transport)?;
            }
        }
        ProjectionSurface::Mcp => {
            for c in &graph.commands {
                if let Some(tool) = &c.projections.mcp {
                    writeln!(out, "{tool}\t{}", c.id).map_err(Error::Transport)?;
                }
            }
        }
    }
    Ok(())
}

/// Run `majordomus completion`.
pub fn completion(args: CompletionArgs) -> Result<u8> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command {
        CompletionCommand::Script { shell } => {
            let text = match shell {
                CompletionShell::Zsh => ZSH,
                CompletionShell::Bash => BASH,
            };
            write!(out, "{text}").map_err(Error::Transport)?;
        }
        CompletionCommand::Query {
            surface,
            cursor,
            format,
            words,
        } => {
            let surface = match surface {
                CompletionSurface::Cli => Surface::Cli,
                CompletionSurface::Just => Surface::Just,
            };
            // the `just` surface needs the recipes `just` holds; the command line does not
            let graph = build(matches!(surface, Surface::Just));
            let request = Request::new(surface, words, cursor);
            let values = completion::Repository::here();
            let answer = completion::answer(&graph, &request, &values);
            match format {
                CompletionFormat::Json => {
                    let text = serde_json::to_string_pretty(&answer)
                        .map_err(|e| Error::Transport(std::io::Error::other(e)))?;
                    writeln!(out, "{text}").map_err(Error::Transport)?;
                }
                CompletionFormat::Shell => {
                    for c in &answer.candidates {
                        writeln!(out, "{}", shell_line(c)).map_err(Error::Transport)?;
                    }
                }
            }
        }
    }
    Ok(0)
}

/// One candidate, as a shell adapter reads it: the value, a tab, and a description with
/// nothing in it that a terminal would act on.
fn shell_line(candidate: &Candidate) -> String {
    let value = match candidate.kind {
        completion::CandidateKind::Path => "<path>",
        _ => candidate.value.as_str(),
    };
    let description = candidate
        .description
        .as_deref()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>();
    format!("{value}\t{description}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_description_can_carry_no_control_sequence_to_the_terminal() {
        let candidate = Candidate {
            value: "worktree".into(),
            description: Some("a\u{1b}[31mred\u{7}\nsecond line".into()),
            kind: completion::CandidateKind::Command,
            append_space: true,
        };
        let line = shell_line(&candidate);
        assert!(!line.contains('\u{1b}'), "{line:?}");
        assert!(!line.contains('\u{7}'), "{line:?}");
        assert_eq!(line.lines().count(), 1, "{line:?}");
    }
}
