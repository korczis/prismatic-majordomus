//! `majordomus completion`: the query a shell adapter makes, and the adapter itself.
//!
//! The query is the hot path — it runs on every TAB — so it takes the fast load: the clap
//! tree, the shell tool's registry, and the workflows from the cache the last
//! materialisation wrote. No index, no subprocess, no network, no server. When the cache is
//! absent the answer is smaller and still correct, which is the right failure for a
//! completion.

use std::io::Write;

use crate::cli::{
    CompletionArgs, CompletionCommand, CompletionInitArgs, CompletionQueryArgs, CompletionShell,
    CompletionSurface, OutputFormat,
};
use crate::command_graph::complete::{self, NoValues, Request, ValueResolver};
use crate::command_graph::{load, shell, Surface, ValueSource};
use crate::error::{Error, Result};

/// Run `majordomus completion`.
pub fn run(args: CompletionArgs) -> Result<u8> {
    match args.command {
        None => init(&CompletionInitArgs {
            shell: CompletionShell::Zsh,
        }),
        Some(CompletionCommand::Init(ref init_args)) => init(init_args),
        Some(CompletionCommand::Query(ref query_args)) => query(&args, query_args),
    }
}

/// `completion init`.
fn init(args: &CompletionInitArgs) -> Result<u8> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let script = match args.shell {
        CompletionShell::Zsh => shell::ZSH,
        CompletionShell::Bash => shell::BASH,
        CompletionShell::Fish => shell::FISH,
    };
    write!(out, "{script}").map_err(Error::Transport)?;
    Ok(0)
}

/// `completion query`.
fn query(args: &CompletionArgs, query_args: &CompletionQueryArgs) -> Result<u8> {
    // A completion never fails a shell. A repository that cannot be found, a cache that
    // cannot be read and a command line that makes no sense all produce no candidates,
    // and the shell falls back to its own file completion.
    let Ok(loaded) = load::fast(&args.repo) else {
        return Ok(0);
    };

    let surface = match query_args.surface {
        CompletionSurface::Cli => Surface::Cli,
        CompletionSurface::Workflow => Surface::Workflow,
    };
    let cursor = query_args
        .cursor
        .unwrap_or_else(|| query_args.words.len().saturating_sub(1));
    let request = Request {
        surface,
        words: query_args.words.clone(),
        cursor,
        cwd: std::env::current_dir().ok(),
    };

    let resolver = Repository {
        root: loaded.root.clone(),
    };
    let response = complete::complete(&loaded.graph, &request, &resolver);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match query_args.format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".into())
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            for candidate in &response.candidates {
                match &candidate.description {
                    Some(d) if !d.is_empty() => {
                        writeln!(out, "{}\t{}", candidate.value, one_line(d))
                    }
                    _ => writeln!(out, "{}", candidate.value),
                }
                .map_err(Error::Transport)?;
            }
        }
    }
    Ok(0)
}

/// One line, with nothing that would break a shell's parsing of the protocol.
fn one_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}

/// The value sources this repository can answer without a subprocess or a server.
///
/// Deliberately small. A source that would cost a process, a network call or an index build
/// answers nothing here rather than making TAB wait: the graph still says what the argument
/// *is*, so the Cockpit's form and a machine surface can resolve it where they can afford
/// to.
struct Repository {
    root: std::path::PathBuf,
}

impl ValueResolver for Repository {
    fn values(&self, source: ValueSource) -> Vec<String> {
        match source {
            // git keeps the local branches as files under refs/heads and as lines in
            // packed-refs. Reading them is two file reads; asking git is a process.
            ValueSource::Branch => branches(&self.root),
            ValueSource::Shell => shell::ALL.iter().map(|(name, _)| (*name).into()).collect(),
            _ => Vec::new(),
        }
    }
}

/// The local branches, read from git's own files.
fn branches(root: &std::path::Path) -> Vec<String> {
    let git = root.join(".git");
    // A linked worktree's `.git` is a file naming the real directory; the branches live
    // with the common directory, not with the worktree.
    let common = if git.is_file() {
        std::fs::read_to_string(&git)
            .ok()
            .and_then(|s| {
                s.strip_prefix("gitdir:")
                    .map(|p| std::path::PathBuf::from(p.trim()))
            })
            .and_then(|p| p.parent().and_then(|p| p.parent()).map(Path_to_owned))
            .unwrap_or(git)
    } else {
        git
    };

    let mut out = Vec::new();
    let heads = common.join("refs/heads");
    walk(&heads, &heads, &mut out);
    if let Ok(packed) = std::fs::read_to_string(common.join("packed-refs")) {
        for line in packed.lines() {
            if let Some(name) = line.split_once(" refs/heads/").map(|(_, n)| n) {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `PathBuf::to_path_buf` as a function, for the option chain above.
#[allow(non_snake_case)]
fn Path_to_owned(p: &std::path::Path) -> std::path::PathBuf {
    p.to_path_buf()
}

/// Every ref file under `dir`, named relative to `base`.
fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(base, &path, out);
        } else if let Ok(name) = path.strip_prefix(base) {
            out.push(name.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// A resolver that answers nothing, for a caller that wants only the declaration's values.
pub fn no_values() -> NoValues {
    NoValues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches_are_read_from_git_and_never_from_a_process() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let heads = dir.path().join(".git/refs/heads/feature");
        std::fs::create_dir_all(&heads).unwrap();
        std::fs::write(heads.join("x"), "0000\n").unwrap();
        std::fs::write(dir.path().join(".git/refs/heads/master"), "0000\n").unwrap();
        std::fs::write(
            dir.path().join(".git/packed-refs"),
            "# pack-refs with: peeled\n0000 refs/heads/packed\n",
        )
        .unwrap();
        let found = branches(dir.path());
        assert!(found.contains(&"master".to_string()), "{found:?}");
        assert!(found.contains(&"feature/x".to_string()), "{found:?}");
        assert!(found.contains(&"packed".to_string()), "{found:?}");
    }

    #[test]
    fn a_description_is_one_line_without_control_characters() {
        assert_eq!(one_line("a\nb"), "a");
        assert_eq!(one_line("a\tb"), "ab");
    }
}
