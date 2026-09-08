//! The `just` bridge: a projection of the graph, not a file anyone writes.
//!
//! One recipe per canonical command, its description the command's own summary, its group
//! the command's own namespace, and its body a call to the executable with the arguments
//! forwarded unchanged. A command that asks for confirmation on the command line asks for
//! it here too, because that is derived from the same classification.
//!
//! The bridge never calls another presentation surface: a recipe runs the executable, and
//! the executable never runs `just`. That is the whole of the cycle rule, and there is a
//! test for it.
//!
//! The file is written under an ignored runtime directory, atomically, and only when the
//! projection's fingerprint has changed — entering a repository must not dirty it, and
//! must not pay for a rewrite that would produce identical bytes.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::control::graph::{CommandGraph, CommandNode, Execution};

/// Where the materialised bridge lives, repository-relative. Ignored, never tracked: it is
/// a function of the executable and the declaration, and both are already in the tree.
pub const BRIDGE_PATH: &str = ".majordomus/runtime/just/bridge.just";

/// The header every generated file of this projection carries.
fn header(graph: &CommandGraph) -> String {
    format!(
        "# GENERATED FILE — DO NOT EDIT.\n\
         # Every recipe below is a projection of the canonical command graph of\n\
         # majordomus-cli {version}. Change the command, not this file.\n\
         # Regenerate: majordomus commands projection just\n\
         # Graph fingerprint: {fingerprint}\n",
        version = graph.version,
        fingerprint = graph.fingerprint,
    )
}

/// The bridge for one graph.
///
/// ```
/// use majordomus_cli::control::{graph, just};
/// let text = just::render(&graph::of_this_executable());
/// assert!(text.contains("capabilities-list *args:"));
/// // the description is the command's own, not a second text
/// assert!(text.contains("# Every capability"));
/// // and nothing in it calls just
/// assert!(!text.lines().any(|l| l.trim_start().starts_with("just ")));
/// ```
pub fn render(graph: &CommandGraph) -> String {
    let mut out = header(graph);
    out.push_str(
        "\n# Arguments reach the executable exactly as they were typed — spaces, quotes,\n\
         # `$`, everything — because `just` passes them as positional arguments and the\n\
         # bodies below forward \"$@\" rather than interpolating a string.\n\
         set positional-arguments\n\
         \n\
         # The executable the bridge calls. `bin/majordomus-cli` resolves it — an installed\n\
         # one, this workspace's build — and is the only place that resolution is written.\n\
         majordomus_exe := env(\"MAJORDOMUS_EXE\", justfile_directory() / \"bin/majordomus-cli\")\n",
    );
    for node in graph.commands.iter().filter(|c| bridged(c)) {
        let Some(recipe) = node.projections.just.as_deref() else {
            continue;
        };
        out.push('\n');
        out.push_str(&format!("# {}\n", one_line(&node.summary)));
        if let Some(group) = node.path.first() {
            out.push_str(&format!("[group('{}')]\n", escape_single(group)));
        }
        if node
            .semantics
            .is_some_and(|s| s.effect.needs_confirmation())
        {
            out.push_str(&format!(
                "[confirm(\"{} — {}. Continue? [y/N]\")]\n",
                escape_double(&node.path.join(" ")),
                escape_double(node.semantics.map(|s| s.effect.describe()).unwrap_or("")),
            ));
        }
        out.push_str(&format!("{recipe} *args:\n"));
        out.push_str(&format!(
            "    \"{{{{majordomus_exe}}}}\" {} \"$@\"\n",
            node.path.join(" ")
        ));
        for alias in &node.aliases {
            out.push_str(&format!("alias {alias} := {recipe}\n"));
        }
    }
    out
}

/// Is this a command the bridge carries? Everything the executable itself runs; never a
/// workflow, which `just` already holds and which the bridge would only be re-declaring.
fn bridged(node: &CommandNode) -> bool {
    !node.group
        && node.availability.is_available()
        && matches!(node.execution, Execution::Native { .. })
        && !node.projections.cli.is_empty()
}

/// The first line of a summary, with anything a comment cannot carry removed.
fn one_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}

fn escape_single(text: &str) -> String {
    text.replace('\'', "")
}

fn escape_double(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .map(|c| if c == '"' { '\'' } else { c })
        .collect()
}

/// What a materialisation did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Materialised {
    /// The file on disk already was this projection; nothing was written.
    Current,
    /// It was written.
    Written,
}

/// Write the bridge under `root`, atomically, and only when it would differ.
///
/// The fingerprint is in the file's own header, so staleness is decided by reading the
/// file rather than by keeping a second record of what was written.
pub fn materialise(root: &Path, graph: &CommandGraph) -> std::io::Result<Materialised> {
    let path = root.join(BRIDGE_PATH);
    let text = render(graph);
    if std::fs::read_to_string(&path).is_ok_and(|existing| existing == text) {
        return Ok(Materialised::Current);
    }
    let dir = path.parent().unwrap_or(root);
    std::fs::create_dir_all(dir)?;
    let temporary = temporary_path(dir);
    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, &path)?;
    Ok(Materialised::Written)
}

/// A name no other process of this machine is writing at the same moment.
fn temporary_path(dir: &Path) -> PathBuf {
    dir.join(format!(".bridge.just.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::graph;

    #[test]
    fn the_bridge_never_calls_just() {
        let text = render(&graph::of_this_executable());
        for line in text.lines() {
            let body = line.trim_start();
            assert!(
                !body.starts_with("just ") && !body.contains("\"just\""),
                "the bridge would recurse: {line}"
            );
        }
    }

    #[test]
    fn a_destructive_command_asks_before_it_runs() {
        let text = render(&graph::of_this_executable());
        let removal = text
            .split("\n\n")
            .find(|block| block.contains("\nworktree-remove *args:"))
            .expect("the removal recipe");
        assert!(removal.contains("[confirm("), "{removal}");
    }

    #[test]
    fn a_read_only_command_does_not() {
        let text = render(&graph::of_this_executable());
        let list = text
            .split("\n\n")
            .find(|block| block.contains("\nworktree-list *args:"))
            .expect("the list recipe");
        assert!(!list.contains("[confirm("), "{list}");
    }

    #[test]
    fn materialising_twice_writes_once() {
        let dir = tempfile::tempdir().expect("a directory");
        let g = graph::of_this_executable();
        assert_eq!(
            materialise(dir.path(), &g).expect("first"),
            Materialised::Written
        );
        assert_eq!(
            materialise(dir.path(), &g).expect("second"),
            Materialised::Current
        );
        assert!(dir.path().join(BRIDGE_PATH).exists());
    }

    #[test]
    fn every_recipe_name_is_one_just_can_parse() {
        let text = render(&graph::of_this_executable());
        for line in text.lines().filter(|l| l.ends_with(" *args:")) {
            let name = line.trim_end_matches(" *args:");
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "{name}"
            );
            assert!(!name.starts_with(|c: char| c.is_ascii_digit()), "{name}");
        }
    }
}
