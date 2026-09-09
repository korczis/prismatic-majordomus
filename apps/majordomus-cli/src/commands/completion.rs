//! `majordomus completion`: the query a shell adapter makes, and the adapter itself.
//!
//! The query is the hot path — it runs on every TAB — so it takes the fast load: the clap
//! tree, the shell tool's registry, and the workflows from the cache the last
//! materialisation wrote. No index, no subprocess, no network, no server. When the cache is
//! absent the answer is smaller and still correct, which is the right failure for a
//! completion.

use std::io::Write;

use crate::cli::{
    CompletionArgs, CompletionCommand, CompletionInitArgs, CompletionInstallArgs,
    CompletionQueryArgs, CompletionShell, CompletionSurface, OutputFormat,
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
        Some(CompletionCommand::Install(ref install_args)) => install(install_args),
    }
}

/// The markers the managed block is written between.
///
/// Everything outside them belongs to the person whose file this is and is never read,
/// rewritten or reordered. Everything between them belongs to this command and is replaced
/// wholesale, which is what makes installing twice the same as installing once.
const BEGIN: &str = "# >>> MAJORDOMUS >>>";
const END: &str = "# <<< MAJORDOMUS <<<";

/// `completion install`.
///
/// The only thing in this executable that writes outside the repository, and it is its own
/// command for that reason: putting a line into somebody's shell startup file is a decision
/// a person makes, never a side effect of initialising a repository.
fn install(args: &CompletionInstallArgs) -> Result<u8> {
    let path = match &args.rc {
        Some(p) => p.clone(),
        None => default_rc(args.shell)?,
    };

    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let wanted = if args.remove {
        None
    } else {
        Some(format!(
            "{BEGIN}\n\
             # Written by `majordomus completion install`. Everything between these markers is\n\
             # replaced when it runs again, and removed by `--remove`; nothing outside them is read.\n\
             # The integration carries no command of its own — it asks the executable for every\n\
             # candidate — so it serves every repository and never goes stale.\n\
             eval \"$({} completion init --shell {})\"\n\
             {END}",
            program(),
            shell_name(args.shell),
        ))
    };

    let updated = splice(&existing, wanted.as_deref());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if updated == existing {
        writeln!(
            out,
            "completion: {} is already as asked; nothing written",
            path.display()
        )
        .map_err(Error::Transport)?;
        return Ok(0);
    }
    if args.dry_run {
        writeln!(
            out,
            "completion: would {} the managed block in {}",
            if args.remove { "remove" } else { "write" },
            path.display()
        )
        .map_err(Error::Transport)?;
        return Ok(0);
    }

    // A backup only when there was something to lose, and only the first time: a person who
    // runs this twice does not want two copies of their shell configuration lying about.
    if !existing.is_empty() {
        let backup = path.with_extension("majordomus.bak");
        if !backup.exists() {
            std::fs::write(&backup, &existing).map_err(|e| Error::io(backup.clone(), e))?;
            writeln!(out, "completion: kept {}", backup.display()).map_err(Error::Transport)?;
        }
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, &updated).map_err(|e| Error::io(path.clone(), e))?;
    writeln!(
        out,
        "completion: {} the managed block in {}\ncompletion: open a new shell, or `source {}`",
        if args.remove { "removed" } else { "wrote" },
        path.display(),
        path.display()
    )
    .map_err(Error::Transport)?;
    Ok(0)
}

/// The program name the installed line calls.
///
/// `majordomus` rather than this executable's path: the integration is generic and outlives
/// any one build directory, and inside a repository the environment names the executable
/// that answers through `MAJORDOMUS_COMPLETION_BIN`.
fn program() -> &'static str {
    "majordomus"
}

/// The shell's own name, as its `--shell` value spells it.
fn shell_name(shell: CompletionShell) -> &'static str {
    match shell {
        CompletionShell::Zsh => "zsh",
        CompletionShell::Bash => "bash",
        CompletionShell::Fish => "fish",
    }
}

/// Where a shell reads its startup from, when the caller does not say.
fn default_rc(shell: CompletionShell) -> Result<std::path::PathBuf> {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| Error::Refused {
            code: 12,
            reason:
                "HOME is not set, so there is no startup file to install into; name one with --rc"
                    .into(),
        })?;
    Ok(match shell {
        CompletionShell::Zsh => home.join(".zshrc"),
        CompletionShell::Bash => home.join(".bashrc"),
        CompletionShell::Fish => home.join(".config/fish/config.fish"),
    })
}

/// Replace the managed block with `block`, or remove it when `block` is `None`.
///
/// Pure, so the whole of the decision is testable without a filesystem: the caller compares
/// the answer with what the file already held and writes only on a difference, which is why
/// running this twice is not two writes.
fn splice(existing: &str, block: Option<&str>) -> String {
    let (before, after) = match (existing.find(BEGIN), existing.find(END)) {
        (Some(b), Some(e)) if e > b => {
            let end = e + END.len();
            let after = existing[end..]
                .strip_prefix('\n')
                .unwrap_or(&existing[end..]);
            (&existing[..b], after)
        }
        // No block, or one whose markers are damaged: leave every byte alone and append.
        _ => (existing, ""),
    };
    let mut out = String::new();
    out.push_str(before);
    if let Some(block) = block {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(block);
        out.push('\n');
    }
    out.push_str(after);
    out
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
        graph: &loaded.graph,
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
///
/// What *is* answered here is answered from something already in hand. A command id comes
/// from the graph this very query resolved against; a capability id comes from composing
/// the builtin modules, which reads no file and builds no index. Neither costs a syscall,
/// so neither is a reason to make TAB wait.
struct Repository<'a> {
    root: std::path::PathBuf,
    graph: &'a crate::command_graph::CommandGraph,
}

impl ValueResolver for Repository<'_> {
    fn values(&self, source: ValueSource) -> Vec<String> {
        match source {
            // git keeps the local branches as files under refs/heads and as lines in
            // packed-refs. Reading them is two file reads; asking git is a process.
            ValueSource::Branch => branches(&self.root),
            ValueSource::Shell => shell::ALL.iter().map(|(name, _)| (*name).into()).collect(),
            // The graph is the answer to `what commands are there`, and it is already
            // loaded — this query resolved a command line against it a moment ago.
            ValueSource::Command => self
                .graph
                .commands
                .iter()
                .map(|c| c.id.to_string())
                .collect(),
            // Composing the builtin modules is compiled-in arithmetic: no index, no walk.
            // The declarative capabilities of the layer are absent for that reason, and
            // they are the ones no command names as an argument.
            ValueSource::Capability => load::registry()
                .map(|r| r.iter().map(|c| c.id.to_string()).collect())
                .unwrap_or_default(),
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

#[cfg(test)]
mod install_tests {
    use super::*;

    const BLOCK: &str = "# >>> MAJORDOMUS >>>\nline\n# <<< MAJORDOMUS <<<";

    #[test]
    fn an_empty_file_gains_the_block() {
        assert_eq!(splice("", Some(BLOCK)), format!("{BLOCK}\n"));
    }

    #[test]
    fn installing_twice_writes_the_same_bytes() {
        let once = splice("export FOO=1\n", Some(BLOCK));
        let twice = splice(&once, Some(BLOCK));
        assert_eq!(once, twice, "a second install changed the file");
    }

    #[test]
    fn nothing_outside_the_markers_is_touched() {
        let before = "export FOO=1\nalias x=y\n";
        let after = "unset BAR\n";
        let installed = splice(&format!("{before}{BLOCK}\n{after}"), Some(BLOCK));
        assert!(
            installed.starts_with(before),
            "the head was rewritten: {installed:?}"
        );
        assert!(
            installed.ends_with(after),
            "the tail was rewritten: {installed:?}"
        );
    }

    #[test]
    fn removing_leaves_the_file_as_it_was() {
        let original = "export FOO=1\nunset BAR\n";
        let installed = splice(original, Some(BLOCK));
        assert_eq!(splice(&installed, None), original);
    }

    #[test]
    fn a_file_with_damaged_markers_keeps_every_byte() {
        // Only the opening marker: the block cannot be located, so nothing is replaced and
        // the person's own text survives. Appending is the safe answer, never a rewrite.
        let odd = "# >>> MAJORDOMUS >>>\nsomething a person edited\n";
        let out = splice(odd, Some(BLOCK));
        assert!(
            out.starts_with(odd),
            "a damaged block ate the file: {out:?}"
        );
    }
}
