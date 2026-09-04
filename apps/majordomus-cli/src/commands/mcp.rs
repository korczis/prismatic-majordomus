//! `majordomus mcp`: discover the repository, build the index, and either serve it over
//! stdio or, with `--inspect`, print it and stop.

use std::io::Write;

use crate::cli::{DiscoveryMode, McpArgs, OutputFormat, Transport};
use crate::discovery::{DiscoverySource, FileSystem, Sources, VcsIndex};
use crate::error::{Error, Result};
use crate::git;
use crate::index::Index;
use crate::mcp::{stdio, Server, Surface};
use crate::metadata::KindSchema;
use crate::model::Severity;
use crate::repository::Repository;

pub fn run(args: McpArgs) -> Result<u8> {
    let start = match &args.repo {
        Some(p) => p.clone(),
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let repo = Repository::discover(&start)?;
    tracing::info!(repository_root = %repo.root().display(), "repository found");
    let sources = Sources::load(&repo)?;
    let schema = KindSchema::embedded()?;
    let git_state = git::inspect(repo.root());
    let source: Box<dyn DiscoverySource> = match args.discovery {
        DiscoveryMode::Vcs => {
            if let git::GitState::Unavailable { reason } = &git_state {
                return Err(Error::Git {
                    reason: format!("{reason}; discovery through the version-control index needs git (or pass --discovery filesystem)"),
                });
            }
            Box::new(VcsIndex)
        }
        DiscoveryMode::Filesystem => Box::new(FileSystem {
            excluded: vec![".git".into(), repo.local_path()],
        }),
    };
    let index = Index::build(&repo, &sources, &schema, source.as_ref(), git_state)?;
    for d in &index.diagnostics {
        match d.severity {
            Severity::Error => tracing::error!(
                code = d.code,
                source_path = d.path.as_deref().unwrap_or("-"),
                "{}",
                d.message
            ),
            Severity::Warning => tracing::warn!(
                code = d.code,
                source_path = d.path.as_deref().unwrap_or("-"),
                "{}",
                d.message
            ),
            Severity::Info => tracing::info!(
                code = d.code,
                source_path = d.path.as_deref().unwrap_or("-"),
                "{}",
                d.message
            ),
        }
    }
    let errors = index.errors();
    if args.strict && errors > 0 {
        return Err(Error::StrictDiagnostics { count: errors });
    }
    let surface = Surface::new(index);
    if args.inspect {
        return inspect(&surface, args.format);
    }
    let mut server = Server::new(surface, crate::VERSION);
    match args.transport {
        Transport::Stdio => {
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            stdio::serve(&mut server, stdin.lock(), stdout.lock())?;
        }
    }
    Ok(0)
}

fn inspect(surface: &Surface, format: OutputFormat) -> Result<u8> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let index = surface.index();
    match format {
        OutputFormat::Json => {
            let v = serde_json::json!({
                "repository": surface.repository_info(),
                "resources": surface.resources(),
                "tools": surface.tools(),
            });
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&v).map_err(|e| Error::Protocol {
                    reason: e.to_string()
                })?
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            writeln!(out, "repository  {}", index.repository.root).map_err(Error::Transport)?;
            writeln!(out, "discovery   {}", index.repository.discovery)
                .map_err(Error::Transport)?;
            writeln!(
                out,
                "state       {}",
                match index.state {
                    crate::index::State::Ok => "ok",
                    crate::index::State::Degraded => "degraded",
                }
            )
            .map_err(Error::Transport)?;
            for (kind, n) in index.kinds() {
                writeln!(out, "kind        {kind:<12} {n}").map_err(Error::Transport)?;
            }
            for r in surface.resources() {
                writeln!(out, "resource    {}", r.uri).map_err(Error::Transport)?;
            }
            for t in surface.tools() {
                writeln!(out, "tool        {}", t.name).map_err(Error::Transport)?;
            }
            for d in &index.diagnostics {
                let level = match d.severity {
                    Severity::Error => "FAIL",
                    Severity::Warning => "WARN",
                    Severity::Info => "INFO",
                };
                writeln!(
                    out,
                    "{level:<4} {:<22} {} — {}",
                    d.code,
                    d.path.as_deref().unwrap_or("-"),
                    d.message
                )
                .map_err(Error::Transport)?;
            }
        }
    }
    Ok(if index.errors() > 0 { 10 } else { 0 })
}
