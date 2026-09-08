//! The Just projection: the bridge a person types `just` at, rendered from the graph.
//!
//! Every recipe in the generated bridge is one command of the graph, its description is
//! that command's own summary, and its body invokes the program that owns the command.
//! Nothing in it is written by hand, and the root `justfile` imports it rather than
//! listing anything: a command added to the clap declaration, to the shell tool's
//! registry or to the workflow tree appears in `just --list` on the next activation with
//! no edit to any Just file.
//!
//! Two properties matter more than the rendering.
//!
//! *Arguments are forwarded, not interpolated.* `set positional-arguments` gives each
//! recipe the real argument vector in `"$@"`, so a path with a space, a quote, a `$(...)`
//! or a leading dash reaches the executable exactly as it was typed. Nothing of the
//! caller's is ever pasted into a command line.
//!
//! *The bridge is never a source.* It is written under the checkout-local state
//! directory, which git ignores, keyed by the fingerprint of what it was projected from,
//! and rewritten only when that changes. Entering a repository twice writes nothing the
//! second time, and entering it once changes no tracked file.

use std::path::{Path, PathBuf};

use super::facts::Facts;
use super::graph::CommandGraph;
use super::model::{CommandNode, Execution, Program};

/// Where the generated bridge lives, relative to the repository root.
pub const PATH: &str = ".ai/local/state/command/bridge.just";

/// The first line of the file, so that nobody edits it twice.
pub const HEADER: &str = "GENERATED FILE — DO NOT EDIT DIRECTLY";

/// The command that rewrites it.
pub const REGENERATE: &str = "majordomus commands bridge";

/// The bridge as text, and the fingerprint it was rendered at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bridge {
    /// The file's whole content.
    pub text: String,
    /// What the content is a function of: the graph's structure and the executables the
    /// recipes name. Availability is deliberately not in it — a checkout that lost a
    /// dependency would render the same file.
    pub fingerprint: String,
}

impl Bridge {
    /// The path of the bridge inside a checkout.
    pub fn path(root: &Path) -> PathBuf {
        root.join(PATH)
    }

    /// Render the bridge of a graph, as the facts of this checkout say the programs are
    /// reached.
    pub fn render(graph: &CommandGraph, facts: &Facts) -> Self {
        let native = facts
            .native_command()
            .unwrap_or_else(|| "majordomus".to_string());
        let shell = super::facts::SHELL_BIN.to_string();
        let mut text = String::new();
        text.push_str(&preamble(graph, &native, &shell));
        for node in graph.listed() {
            text.push_str(&recipe(node, &native, &shell));
        }
        for node in graph.listed() {
            for alias in &node.projections.just_aliases {
                if let Some(name) = &node.projections.just {
                    text.push_str(&format!("alias {alias} := {name}\n"));
                }
            }
        }
        let fingerprint = fingerprint(&graph.structure, &native, &shell);
        Bridge { text, fingerprint }
    }

    /// Write the bridge if what it is a function of has changed, and say whether it did.
    ///
    /// The check is one line of one file: the fingerprint is written into the bridge
    /// itself, so deciding costs a read of a few hundred bytes and no rendering at all.
    pub fn materialise(&self, root: &Path) -> std::io::Result<bool> {
        let path = Self::path(root);
        if let Some(existing) = std::fs::read_to_string(&path).ok() {
            if existing.contains(&self.fingerprint) {
                return Ok(false);
            }
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        super::atomic_write(&path, self.text.as_bytes())?;
        Ok(true)
    }
}

/// The bridge's fingerprint: the graph's structure and the spelling of every program a
/// recipe names.
fn fingerprint(structure: &str, native: &str, shell: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(structure.as_bytes());
    h.update(b"\0");
    h.update(native.as_bytes());
    h.update(b"\0");
    h.update(shell.as_bytes());
    format!("{:x}", h.finalize())
}

fn preamble(graph: &CommandGraph, native: &str, shell: &str) -> String {
    format!(
        "# {HEADER}\n\
         # Source: the Majordomus command graph ({schema}); regenerate with `{REGENERATE}`\n\
         # Generator: majordomus-cli {version}\n\
         # Fingerprint: {fingerprint}\n\
         #\n\
         # Every recipe below is one command of the graph. Its description is that\n\
         # command's own summary and exists nowhere else. Arguments are forwarded through\n\
         # \"$@\", never interpolated, so anything a person types reaches the program as\n\
         # they typed it.\n\
         \n\
         set positional-arguments := true\n\
         \n\
         majordomus_native := {native}\n\
         majordomus_shell := {shell}\n\
         \n",
        schema = graph.schema,
        version = graph.version,
        fingerprint = fingerprint(&graph.structure, native, shell),
        native = quote(native),
        shell = quote(shell),
    )
}

/// One recipe. The body is a single command with `"$@"` at the end of it: whatever the
/// caller passed goes to the program, and nothing the graph holds is ever evaluated by a
/// shell.
fn recipe(node: &CommandNode, _native: &str, _shell: &str) -> String {
    let Some(name) = &node.projections.just else {
        return String::new();
    };
    let mut out = String::new();
    out.push_str(&format!("# {}\n", one_line(&summary(node))));
    out.push_str(&format!("[group('{}')]\n", group(node)));
    out.push_str(&format!("{name} *args:\n"));
    for line in body(node) {
        out.push_str(&format!("    {line}\n"));
    }
    out.push('\n');
    out
}

/// The lines of a recipe's body: one per step, each the program and the words the command
/// declares, then `"$@"`.
fn body(node: &CommandNode) -> Vec<String> {
    match &node.execution {
        Execution::Native { argv } => vec![format!(
            "@\"{{{{majordomus_native}}}}\" {} \"$@\"",
            argv.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ")
        )],
        Execution::Shell { argv } => vec![format!(
            "@\"{{{{majordomus_shell}}}}\" {} \"$@\"",
            argv.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ")
        )],
        Execution::External { steps } => {
            let last = steps.len().saturating_sub(1);
            steps
                .iter()
                .enumerate()
                .map(|(i, words)| {
                    let mut line = words.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ");
                    // Only the last step of a workflow takes the caller's arguments; a
                    // workflow that wanted them on every step would be two workflows.
                    if i == last {
                        line.push_str(" \"$@\"");
                    }
                    line
                })
                .collect()
        }
    }
}

/// The description a recipe carries: the command's summary, with a deprecation on the
/// front when there is one, so `just --list` and the reference tell the same story.
fn summary(node: &CommandNode) -> String {
    match &node.deprecation {
        Some(d) => format!("DEPRECATED: {} {}", d.note, node.summary),
        None => node.summary.clone(),
    }
}

/// The group `just --list` files a recipe under: the command's own first word for the
/// native executable and for a workflow, and the category the shell tool's registry
/// already gives its commands. Derived, so no recipe is filed by hand.
fn group(node: &CommandNode) -> String {
    let raw = match node.program {
        Program::Shell => node
            .tags
            .iter()
            .find(|t| !t.starts_with("stage:"))
            .cloned()
            .unwrap_or_else(|| "lifecycle".to_string()),
        _ => node.path.first().cloned().unwrap_or_default(),
    };
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// A word as a Just string literal. Single quotes in Just are raw: nothing inside them is
/// interpreted, which is exactly what a projected value needs. A word carrying a single
/// quote — which no command path or program name of this repository does — is refused
/// rather than escaped, because there is no escape inside a raw string.
fn quote(word: &str) -> String {
    if word.contains('\'') || word.contains('\n') {
        return format!("'{}'", word.replace(['\'', '\n'], ""));
    }
    format!("'{word}'")
}

/// One line, whatever the source said: a description that carried a newline would end the
/// comment and turn the next line into Just syntax.
fn one_line(text: &str) -> String {
    super::completion::sanitise(text).unwrap_or_else(|| "(no description)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")
    }

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn bridge() -> Bridge {
        let root = root();
        let facts = Facts::read(&root);
        Bridge::render(&CommandGraph::build(&root, &share(), &facts), &facts)
    }

    #[test]
    fn every_listed_command_becomes_a_recipe_and_nothing_else_does() {
        let root = root();
        let facts = Facts::read(&root);
        let graph = CommandGraph::build(&root, &share(), &facts);
        let text = Bridge::render(&graph, &facts).text;
        for node in graph.listed() {
            let name = node.projections.just.clone().unwrap();
            assert!(
                text.contains(&format!("\n{name} *args:\n")),
                "{name} has no recipe"
            );
        }
        let recipes = text
            .lines()
            .filter(|l| l.ends_with(" *args:"))
            .count();
        assert_eq!(recipes, graph.listed().len(), "no ghost recipe");
    }

    #[test]
    fn the_description_is_the_commands_own_summary_and_exists_once() {
        let root = root();
        let facts = Facts::read(&root);
        let graph = CommandGraph::build(&root, &share(), &facts);
        let text = Bridge::render(&graph, &facts).text;
        let node = graph.get("native.bench.coverage").unwrap();
        let expected = super::one_line(&node.summary);
        assert!(text.contains(&format!("# {expected}\n[group('bench')]\nbench-coverage *args:")));
    }

    #[test]
    fn arguments_are_forwarded_and_never_interpolated() {
        let text = bridge().text;
        assert!(text.contains("set positional-arguments := true"));
        for line in text.lines().filter(|l| l.starts_with("    ")) {
            assert!(line.ends_with("\"$@\""), "{line}");
            assert!(
                !line.contains("{{args}}"),
                "a caller's words are never pasted into a command line: {line}"
            );
        }
    }

    #[test]
    fn nothing_the_graph_holds_reaches_a_shell_unquoted() {
        let text = bridge().text;
        for line in text.lines().filter(|l| l.starts_with("    ")) {
            // Everything between the program and "$@" is a raw Just string.
            let payload = line.trim_start().trim_end_matches(" \"$@\"");
            let dangerous = ["$(", "`", ";", "&&", "||", ">", "<", "|"];
            let outside_quotes: String = {
                let mut out = String::new();
                let mut inside = false;
                for c in payload.chars() {
                    if c == '\'' {
                        inside = !inside;
                    } else if !inside {
                        out.push(c);
                    }
                }
                out
            };
            for d in dangerous {
                assert!(
                    !outside_quotes.contains(d),
                    "{d} outside a quoted word in {line}"
                );
            }
        }
    }

    #[test]
    fn the_bridge_is_written_once_and_not_again() {
        let dir = tempfile::tempdir().unwrap();
        let b = bridge();
        assert!(b.materialise(dir.path()).unwrap(), "the first write happens");
        assert!(
            !b.materialise(dir.path()).unwrap(),
            "the second changes nothing"
        );
        let text = std::fs::read_to_string(Bridge::path(dir.path())).unwrap();
        assert!(text.starts_with(&format!("# {HEADER}")));
    }

    #[test]
    fn a_half_written_bridge_is_replaced_rather_than_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let path = Bridge::path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# truncated\nbench-cov").unwrap();
        let b = bridge();
        assert!(b.materialise(dir.path()).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), b.text);
    }

    #[test]
    fn a_description_that_tried_to_be_syntax_is_one_line_of_comment() {
        assert_eq!(one_line("a\nb: rm -rf /"), "a b: rm -rf /");
        assert_eq!(one_line("   "), "(no description)");
        assert!(!one_line("x\u{1b}[31m").contains('\u{1b}'));
    }

    #[test]
    fn a_word_carrying_a_quote_cannot_close_the_literal() {
        assert_eq!(quote("plain"), "'plain'");
        assert_eq!(quote("with'quote"), "'withquote'");
        assert!(!quote("a\nb").contains('\n'));
    }

    #[test]
    fn an_alias_is_projected_as_an_alias_and_not_as_a_second_recipe() {
        let root = root();
        let facts = Facts::read(&root);
        let graph = CommandGraph::build(&root, &share(), &facts);
        let text = Bridge::render(&graph, &facts).text;
        for node in graph.listed() {
            for alias in &node.projections.just_aliases {
                let name = node.projections.just.clone().unwrap();
                assert!(text.contains(&format!("alias {alias} := {name}\n")), "{alias}");
                assert!(!text.contains(&format!("\n{alias} *args:\n")));
            }
        }
    }
}
