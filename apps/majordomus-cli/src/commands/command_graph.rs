//! `majordomus commands`: the command graph, read from the command line.
//!
//! Every subcommand here is a rendering of [`crate::command_graph`] and decides nothing of
//! its own: what a command is, where it is projected and why it is withheld are answered by
//! the graph and the policy beside it, so the terminal, the Cockpit and a machine client
//! give the same answers to the same questions.

use std::io::Write;

use serde_json::{json, Value};

use crate::cli::{
    CommandEffect, CommandOrigin, CommandsArgs, CommandsBridgeArgs, CommandsCommand,
    CommandsGraphArgs, CommandsListArgs, CommandsShowArgs, OutputFormat,
};
use crate::command_graph::{bridge, load, CommandGraph, CommandNode, Effect, Origin, Severity};
use crate::error::{Error, Result};

/// The exit code when a graph carries an error, or the materialised bridge is stale.
pub const EXIT_DRIFT: u8 = 10;

/// The exit code when a command is asked for by an identity nothing carries.
pub const EXIT_MISSING: u8 = 12;

/// Run `majordomus commands`.
pub fn run(args: CommandsArgs) -> Result<u8> {
    let format = args.format;
    match args.command {
        None => list(&args, &CommandsListArgs::none(), format),
        Some(CommandsCommand::List(ref list_args)) => list(&args, list_args, format),
        Some(CommandsCommand::Show(ref show)) => show_one(&args, show, format, false),
        Some(CommandsCommand::Explain(ref show)) => show_one(&args, show, format, true),
        Some(CommandsCommand::Graph(ref graph_args)) => whole(&args, graph_args, format),
        Some(CommandsCommand::Bridge(ref bridge_args)) => project(&args, bridge_args, format),
    }
}

impl CommandsListArgs {
    /// The filters of a bare `majordomus commands`: none.
    fn none() -> Self {
        CommandsListArgs {
            origin: None,
            effect: None,
            search: None,
        }
    }
}

/// `commands list`.
fn list(args: &CommandsArgs, filters: &CommandsListArgs, format: OutputFormat) -> Result<u8> {
    let loaded = load::full(&args.repo)?;
    let matched: Vec<&CommandNode> = loaded
        .graph
        .commands
        .iter()
        .filter(|c| filters.origin.is_none_or(|o| c.origin == origin_of(o)))
        .filter(|c| filters.effect.is_none_or(|e| c.effect <= effect_of(e)))
        .filter(|c| {
            filters
                .search
                .as_ref()
                .is_none_or(|t| c.haystack().contains(&t.to_lowercase()))
        })
        .collect();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Json => {
            let doc = json!({
                "schema": crate::command_graph::SCHEMA,
                "fingerprint": loaded.graph.fingerprint,
                "commands": matched,
            });
            writeln!(out, "{}", pretty(&doc)).map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            writeln!(
                out,
                "commands     {} of {} (graph {})",
                matched.len(),
                loaded.graph.commands.len(),
                loaded.graph.fingerprint
            )
            .map_err(Error::Transport)?;
            for node in &matched {
                writeln!(
                    out,
                    "{:<10} {:<20} {}",
                    origin_word(node.origin),
                    effect_word(node.effect),
                    node.invocation
                )
                .map_err(Error::Transport)?;
            }
            report(&mut out, &loaded.graph)?;
        }
    }
    Ok(0)
}

/// `commands show` and `commands explain`.
fn show_one(
    args: &CommandsArgs,
    show: &CommandsShowArgs,
    format: OutputFormat,
    explain: bool,
) -> Result<u8> {
    let loaded = load::full(&args.repo)?;
    let Some(node) = loaded
        .graph
        .commands
        .iter()
        .find(|c| c.id.as_str() == show.id)
    else {
        return Err(Error::Refused {
            code: EXIT_MISSING,
            reason: format!("no command carries the identity `{}`", show.id),
        });
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Json => {
            writeln!(
                out,
                "{}",
                pretty(&serde_json::to_value(node).unwrap_or(Value::Null))
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            writeln!(out, "command      {}", node.id).map_err(Error::Transport)?;
            writeln!(out, "run          {}", node.invocation).map_err(Error::Transport)?;
            writeln!(out, "summary      {}", node.summary).map_err(Error::Transport)?;
            writeln!(out, "effect       {}", effect_word(node.effect)).map_err(Error::Transport)?;
            writeln!(out, "runs in      {:?}", node.interactivity).map_err(Error::Transport)?;
            if !node.arguments.is_empty() {
                writeln!(out, "arguments").map_err(Error::Transport)?;
                for arg in &node.arguments {
                    let spelling = match &arg.long {
                        Some(long) => format!("--{long}"),
                        None => format!("<{}>", arg.name.to_uppercase()),
                    };
                    writeln!(
                        out,
                        "  {:<22} {:<18} {}",
                        spelling,
                        source_word(arg.source),
                        arg.help
                    )
                    .map_err(Error::Transport)?;
                }
            }
            writeln!(out, "projections").map_err(Error::Transport)?;
            let p = &node.projections;
            for (surface, value) in [
                ("command line", p.cli.clone()),
                ("workflow", p.workflow.clone().map(|w| format!("just {w}"))),
                ("mcp", p.mcp.clone()),
                ("http", p.http.clone()),
                ("cockpit", p.cockpit.clone()),
                ("docs", Some(p.docs.clone())),
            ] {
                match value {
                    Some(v) => writeln!(out, "  {surface:<14} {v}").map_err(Error::Transport)?,
                    None => writeln!(out, "  {surface:<14} —").map_err(Error::Transport)?,
                }
            }
            if let Some(why) = &p.withheld {
                writeln!(out, "  withheld       {why}").map_err(Error::Transport)?;
            }
            if explain {
                writeln!(out, "declared in  {}", node.provenance.declared_in)
                    .map_err(Error::Transport)?;
                if let Some(read_by) = &node.provenance.read_by {
                    writeln!(out, "read by      {read_by}").map_err(Error::Transport)?;
                }
                if let Some(capability) = &node.provenance.capability {
                    writeln!(out, "capability   {capability}").map_err(Error::Transport)?;
                }
                writeln!(
                    out,
                    "requires     {}",
                    node.availability
                        .requires
                        .iter()
                        .map(|r| format!("{r:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .map_err(Error::Transport)?;
                writeln!(
                    out,
                    "policy       apps/majordomus-cli/src/command_graph/policy.rs"
                )
                .map_err(Error::Transport)?;
            }
        }
    }
    Ok(0)
}

/// `commands graph`.
fn whole(args: &CommandsArgs, graph_args: &CommandsGraphArgs, format: OutputFormat) -> Result<u8> {
    let loaded = load::full(&args.repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            pretty(&serde_json::to_value(&loaded.graph).unwrap_or(Value::Null))
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            writeln!(out, "schema       {}", loaded.graph.schema).map_err(Error::Transport)?;
            writeln!(out, "fingerprint  {}", loaded.graph.fingerprint).map_err(Error::Transport)?;
            writeln!(out, "commands     {}", loaded.graph.commands.len())
                .map_err(Error::Transport)?;
            for origin in [Origin::Executable, Origin::Tool, Origin::Workflow] {
                writeln!(
                    out,
                    "  {:<10} {}",
                    origin_word(origin),
                    loaded.graph.of_origin(origin).count()
                )
                .map_err(Error::Transport)?;
            }
            report(&mut out, &loaded.graph)?;
        }
    }
    if graph_args.check && !loaded.graph.errors().is_empty() {
        return Ok(EXIT_DRIFT);
    }
    Ok(0)
}

/// `commands bridge`.
fn project(
    args: &CommandsArgs,
    bridge_args: &CommandsBridgeArgs,
    format: OutputFormat,
) -> Result<u8> {
    // The cheap question first, and it is the one asked on every entry into the
    // repository: have any of the declarations the bridge is derived from changed since it
    // was written? A few `stat` calls answer it, and when the answer is no there is
    // nothing to build, nothing to ask the workflow runner, and nothing to write.
    if !bridge_args.check {
        if let Ok(root) = load::root(&args.repo) {
            if bridge::is_current(&root) {
                if matches!(format, OutputFormat::Text) {
                    let stdout = std::io::stdout();
                    let mut out = stdout.lock();
                    writeln!(out, "bridge       already current").map_err(Error::Transport)?;
                }
                return Ok(0);
            }
        }
    }
    let loaded = load::full(&args.repo)?;
    if !loaded.graph.errors().is_empty() {
        // A projection of a graph with an error would write the error into a file every
        // shell then loads. The refusal is the point.
        let stderr = std::io::stderr();
        let mut err = stderr.lock();
        for d in loaded.graph.errors() {
            writeln!(err, "commands: {}", d.message).map_err(Error::Transport)?;
        }
        return Ok(EXIT_DRIFT);
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if bridge_args.check {
        let current = bridge::manifest(&loaded.root);
        let fresh = current
            .as_ref()
            .is_some_and(|m| m.fingerprint == loaded.graph.fingerprint);
        match format {
            OutputFormat::Json => writeln!(
                out,
                "{}",
                pretty(&json!({
                    "fresh": fresh,
                    "fingerprint": loaded.graph.fingerprint,
                    "materialised": current.map(|m| m.fingerprint),
                }))
            )
            .map_err(Error::Transport)?,
            OutputFormat::Text => writeln!(
                out,
                "bridge       {}",
                if fresh {
                    "current".to_string()
                } else {
                    format!(
                        "stale; run `majordomus commands bridge` (graph {})",
                        loaded.graph.fingerprint
                    )
                }
            )
            .map_err(Error::Transport)?,
        }
        return Ok(if fresh { 0 } else { EXIT_DRIFT });
    }

    let outcome = bridge::materialise(&loaded.root, &loaded.graph)
        .map_err(|e| Error::io(loaded.root.join(bridge::DIRECTORY), e))?;
    let _ = load::write_cache(&loaded.root, &loaded.graph);
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            pretty(&json!({
                "path": outcome.path.to_string_lossy(),
                "written": outcome.written,
                "recipes": outcome.recipes,
                "fingerprint": outcome.fingerprint,
            }))
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => writeln!(
            out,
            "bridge       {} recipe(s) {} ({})",
            outcome.recipes,
            if outcome.written {
                "written"
            } else {
                "already current"
            },
            outcome.path.display()
        )
        .map_err(Error::Transport)?,
    }
    Ok(0)
}

/// The diagnostics, when there are any worth printing.
fn report(out: &mut std::io::StdoutLock<'_>, graph: &CommandGraph) -> Result<()> {
    for d in graph
        .diagnostics
        .iter()
        .filter(|d| d.severity != Severity::Info)
    {
        let word = match d.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN ",
            Severity::Info => "INFO ",
        };
        writeln!(out, "{word}        {} [{}]", d.message, d.code).map_err(Error::Transport)?;
    }
    Ok(())
}

/// The word a person reads for an origin.
fn origin_word(origin: Origin) -> &'static str {
    match origin {
        Origin::Executable => "executable",
        Origin::Tool => "tool",
        Origin::Workflow => "workflow",
    }
}

/// The word a person reads for an effect.
fn effect_word(effect: Effect) -> &'static str {
    match effect {
        Effect::ReadOnly => "read-only",
        Effect::LocalMutation => "local-mutation",
        Effect::RepositoryMutation => "repository-mutation",
        Effect::NetworkMutation => "network-mutation",
        Effect::Destructive => "destructive",
    }
}

/// The word a person reads for a value source.
fn source_word(source: crate::command_graph::ValueSource) -> String {
    format!("{source:?}").to_lowercase()
}

/// The origin a filter names.
fn origin_of(o: CommandOrigin) -> Origin {
    match o {
        CommandOrigin::Executable => Origin::Executable,
        CommandOrigin::Tool => Origin::Tool,
        CommandOrigin::Workflow => Origin::Workflow,
    }
}

/// The effect a filter names.
fn effect_of(e: CommandEffect) -> Effect {
    match e {
        CommandEffect::ReadOnly => Effect::ReadOnly,
        CommandEffect::LocalMutation => Effect::LocalMutation,
        CommandEffect::RepositoryMutation => Effect::RepositoryMutation,
        CommandEffect::NetworkMutation => Effect::NetworkMutation,
        CommandEffect::Destructive => Effect::Destructive,
    }
}

/// One JSON document, deterministic.
fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".into())
}
