//! One completion engine, for every surface.
//!
//! A shell adapter parses the line it is completing and renders what comes back. It knows
//! no commands, no flags, no identifiers and no repository: it forwards a
//! [`Request`] and prints a [`Response`]. Everything else happens here, over the command
//! graph, which is why `just <TAB>`, `majordomus <TAB>` and `./bin/majordomus <TAB>` cannot
//! disagree about what exists — they are three spellings resolved against one graph by one
//! function.
//!
//! ```text
//!   zsh / bash / fish
//!         │  words, cursor
//!         ▼
//!   generic adapter ──► majordomus completion query ──► CompletionEngine
//!                                                            │
//!                                              CommandGraph  +  value sources
//! ```
//!
//! # What it will not do
//!
//! - **No network.** A value that can only be learned remotely is not offered.
//! - **No secret.** An argument classified as a secret is never enumerated, and its value
//!   never enters a candidate, a cache or a log.
//! - **No dependence on a server.** A running server may make a value source faster; a
//!   stopped one changes nothing. Nothing here starts one.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::model::{ArgumentSpec, CommandGraph, CommandNode, Origin, Surface, ValueSource};
use super::policy;

/// What a shell asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Request {
    /// Which spelling the words are in.
    pub surface: Surface,
    /// The words of the command line, the program's own name included at index 0.
    pub words: Vec<String>,
    /// The index of the word the cursor is in. A cursor past the last word is a new,
    /// empty word.
    pub cursor: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where the shell is. Used only to make a path candidate relative; never scanned.
    pub cwd: Option<PathBuf>,
}

impl Request {
    /// The text of the word being completed, empty when the cursor opens a new one.
    pub fn prefix(&self) -> &str {
        self.words
            .get(self.cursor)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// The words before the cursor, the program's own name excluded.
    pub fn preceding(&self) -> &[String] {
        let end = self.cursor.min(self.words.len());
        let start = 1.min(end);
        &self.words[start..end]
    }
}

/// What kind of thing a candidate is, so a shell that groups its menu can.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    /// A command or a subcommand.
    Command,
    /// A recipe of the workflow runner.
    Workflow,
    /// An option, with its dashes.
    Flag,
    /// A value of an argument.
    Value,
    /// A path; the shell completes the rest itself.
    Path,
}

/// One thing the caller may type next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// What to insert.
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// One line about it, for a shell that shows descriptions.
    pub description: Option<String>,
    /// What kind of thing it is.
    pub kind: CandidateKind,
    /// Whether a space belongs after it.
    pub append_space: bool,
    /// Lower sorts first. Derived from what the candidate is, never from a preference.
    pub priority: i32,
}

/// What the engine answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Response {
    /// The candidates, ordered.
    pub candidates: Vec<Candidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The command the words resolved to, for a debugging adapter.
    pub resolved: Option<String>,
    /// Whether the shell should fall back to its own file completion as well.
    pub files: bool,
}

/// Where a value that is not in the declaration comes from.
///
/// The graph knows that `<BRANCH>` is a branch; it does not know which branches exist. A
/// resolver answers that, and the same resolver answers it for a shell, for a generated
/// form and for a machine surface — so a select in the Cockpit and a TAB in a terminal
/// cannot offer different sets.
pub trait ValueResolver {
    /// The values of one source, or none when this resolver cannot answer it here.
    fn values(&self, source: ValueSource) -> Vec<String>;
}

/// A resolver that answers nothing.
///
/// The engine is complete without one: command and flag completion, and every value the
/// declaration carries, are answered from the graph alone. This is what the fast path uses
/// and what a checkout with no repository gets.
pub struct NoValues;

impl ValueResolver for NoValues {
    fn values(&self, _source: ValueSource) -> Vec<String> {
        Vec::new()
    }
}

/// Complete one line.
pub fn complete(graph: &CommandGraph, request: &Request, values: &dyn ValueResolver) -> Response {
    let prefix = request.prefix().to_string();
    let words = request.preceding();

    match request.surface {
        Surface::Workflow => complete_workflow(graph, &prefix, words, values),
        _ => complete_cli(graph, &prefix, words, values),
    }
}

/// The command line of either program: `majordomus <TAB>`.
fn complete_cli(
    graph: &CommandGraph,
    prefix: &str,
    words: &[String],
    values: &dyn ValueResolver,
) -> Response {
    // The longest command path the words spell. Both programs answer to `majordomus`, so
    // both are searched and the deeper match wins; a word that names neither ends the walk
    // and everything after it is an argument.
    let mut path: Vec<String> = Vec::new();
    let mut resolved: Option<&CommandNode> = None;
    let mut consumed = 0usize;
    for (i, word) in words.iter().enumerate() {
        if word.starts_with('-') {
            break;
        }
        let mut candidate_path = path.clone();
        candidate_path.push(word.clone());
        match graph.commands.iter().find(|c| {
            matches!(c.origin, Origin::Executable | Origin::Tool) && c.path == candidate_path
        }) {
            Some(node) => {
                path = candidate_path;
                resolved = Some(node);
                consumed = i + 1;
            }
            None => break,
        }
    }
    let rest = &words[consumed.min(words.len())..];
    candidates_for(graph, resolved, &path, rest, prefix, values, false)
}

/// The workflow runner: `just <TAB>`.
fn complete_workflow(
    graph: &CommandGraph,
    prefix: &str,
    words: &[String],
    values: &dyn ValueResolver,
) -> Response {
    // The runner takes one recipe name and then arguments. Resolving that name back to the
    // canonical node is the whole of the surface alias handling: what follows is completed
    // by the same code that completes the command line, from the same argument metadata.
    let Some(name) = words.first() else {
        let mut candidates: Vec<Candidate> = graph
            .commands
            .iter()
            .filter(|c| policy::offered_on(c, Surface::Workflow))
            .filter_map(|c| {
                let name = c.projections.workflow.clone()?;
                Some(Candidate {
                    description: Some(summary(c)),
                    kind: CandidateKind::Workflow,
                    append_space: true,
                    priority: priority_of(c),
                    value: name,
                })
            })
            .collect();
        return finish(&mut candidates, prefix, None, false);
    };
    let Some(node) = graph.resolve(Surface::Workflow, name) else {
        return Response {
            candidates: Vec::new(),
            resolved: None,
            files: true,
        };
    };
    let path = node.path.clone();
    candidates_for(graph, Some(node), &path, &words[1..], prefix, values, true)
}

/// The candidates at one point of one command.
#[allow(clippy::too_many_arguments)]
fn candidates_for<'a>(
    graph: &'a CommandGraph,
    node: Option<&'a CommandNode>,
    path: &[String],
    rest: &[String],
    prefix: &str,
    values: &dyn ValueResolver,
    in_workflow: bool,
) -> Response {
    let mut out: Vec<Candidate> = Vec::new();
    let mut files = false;

    // A flag that takes a value, immediately before the cursor, decides everything.
    if let Some(previous) = rest.last() {
        if previous.starts_with('-') {
            if let Some(arg) = node.and_then(|n| flag(n, previous)) {
                if arg.takes_value {
                    let (mut vals, wants_files) = value_candidates(arg, values);
                    return finish(&mut vals, prefix, node, wants_files);
                }
            }
        }
    }

    if prefix.starts_with('-') {
        if let Some(node) = node {
            for arg in &node.arguments {
                if arg.positional {
                    continue;
                }
                if let Some(long) = &arg.long {
                    out.push(Candidate {
                        value: format!("--{long}"),
                        description: Some(arg.help.clone()),
                        kind: CandidateKind::Flag,
                        append_space: !arg.takes_value,
                        priority: if arg.required { 10 } else { 20 },
                    });
                }
            }
        }
        return finish(&mut out, prefix, node, false);
    }

    // The subcommands under the resolved point, unless the surface has no notion of one:
    // the workflow runner takes a recipe and then arguments, never a subcommand.
    if !in_workflow {
        for child in graph.commands.iter().filter(|c| {
            matches!(c.origin, Origin::Executable | Origin::Tool)
                && c.path.len() == path.len() + 1
                && c.path.starts_with(path)
        }) {
            out.push(Candidate {
                value: child.path.last().cloned().unwrap_or_default(),
                description: Some(summary(child)),
                kind: CandidateKind::Command,
                append_space: true,
                priority: priority_of(child),
            });
        }
        if let Some(node) = node {
            for alias in &node.aliases {
                out.push(Candidate {
                    value: alias.clone(),
                    description: Some(format!("alias of {}", node.invocation)),
                    kind: CandidateKind::Command,
                    append_space: true,
                    priority: 60,
                });
            }
        }
    }

    // The next positional argument's values, when one is expected.
    if let Some(node) = node {
        let given = rest.iter().filter(|w| !w.starts_with('-')).count();
        if let Some(arg) = node.arguments.iter().filter(|a| a.positional).nth(given) {
            let (mut vals, wants_files) = value_candidates(arg, values);
            files |= wants_files;
            out.append(&mut vals);
        }
    }

    finish(&mut out, prefix, node, files)
}

/// The candidates for one argument's value, and whether the shell should also offer files.
fn value_candidates(arg: &ArgumentSpec, values: &dyn ValueResolver) -> (Vec<Candidate>, bool) {
    // A secret is never enumerated. This is checked here as well as in the graph's own
    // validation, because a completion is the one place where offering a value is the same
    // as printing it.
    if !arg.source.suggestible() {
        return (Vec::new(), matches!(arg.source, ValueSource::Free));
    }
    match arg.source {
        ValueSource::Enumerated => (
            arg.values
                .iter()
                .map(|v| Candidate {
                    value: v.value.clone(),
                    description: v.description.clone(),
                    kind: CandidateKind::Value,
                    append_space: true,
                    priority: 30,
                })
                .collect(),
            false,
        ),
        ValueSource::Path | ValueSource::RepositoryPath => (Vec::new(), true),
        other => (
            values
                .values(other)
                .into_iter()
                .map(|v| Candidate {
                    value: v,
                    description: None,
                    kind: CandidateKind::Value,
                    append_space: true,
                    priority: 30,
                })
                .collect(),
            false,
        ),
    }
}

/// The flag one word names, long or short.
fn flag<'a>(node: &'a CommandNode, word: &str) -> Option<&'a ArgumentSpec> {
    let bare = word.trim_start_matches('-');
    node.arguments.iter().find(|a| {
        a.long.as_deref() == Some(bare) || a.short.map(|c| c.to_string()).as_deref() == Some(bare)
    })
}

/// One line about a command, with the reason it is unavailable when it is.
fn summary(node: &CommandNode) -> String {
    match (&node.availability.available, &node.availability.reason) {
        (false, Some(reason)) => format!("{} (unavailable: {reason})", node.summary),
        _ => node.summary.clone(),
    }
}

/// Where a command sorts among its siblings.
///
/// Deterministic and derived: an entry point first, then by what running it changes, then
/// alphabetically by the value itself in [`finish`]. Nothing reorders by use, because a
/// completion whose order depends on history is a completion nobody can predict.
fn priority_of(node: &CommandNode) -> i32 {
    let mut p = match node.origin {
        Origin::Workflow => 0,
        Origin::Tool => 10,
        Origin::Executable => 10,
    };
    if node.entrypoint == Some(true) {
        p -= 5;
    }
    if !node.availability.available {
        p += 100;
    }
    p + (node.effect as i32)
}

/// Filter by the prefix, order deterministically, answer.
fn finish(
    candidates: &mut Vec<Candidate>,
    prefix: &str,
    node: Option<&CommandNode>,
    files: bool,
) -> Response {
    candidates.retain(|c| c.value.starts_with(prefix));
    candidates.sort_by(|a, b| (a.priority, &a.value).cmp(&(b.priority, &b.value)));
    candidates.dedup_by(|a, b| a.value == b.value);
    Response {
        candidates: std::mem::take(candidates),
        resolved: node.map(|n| n.id.to_string()),
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_graph::{build, Inputs};

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }

    fn request(surface: Surface, line: &str, trailing_space: bool) -> Request {
        let words = words(line);
        let cursor = if trailing_space {
            words.len()
        } else {
            words.len().saturating_sub(1)
        };
        Request {
            surface,
            words,
            cursor,
            cwd: None,
        }
    }

    #[test]
    fn the_root_offers_the_top_level_commands() {
        let graph = build(&Inputs::default());
        let r = complete(
            &graph,
            &request(Surface::Cli, "majordomus", true),
            &NoValues,
        );
        let names: Vec<&str> = r.candidates.iter().map(|c| c.value.as_str()).collect();
        assert!(names.contains(&"worktree"), "{names:?}");
        assert!(names.contains(&"serve"), "{names:?}");
    }

    #[test]
    fn a_prefix_narrows_it() {
        let graph = build(&Inputs::default());
        let r = complete(
            &graph,
            &request(Surface::Cli, "majordomus work", false),
            &NoValues,
        );
        assert!(r.candidates.iter().all(|c| c.value.starts_with("work")));
        assert!(r.candidates.iter().any(|c| c.value == "worktree"));
    }

    #[test]
    fn a_nested_command_offers_its_subcommands() {
        let graph = build(&Inputs::default());
        let r = complete(
            &graph,
            &request(Surface::Cli, "majordomus worktree", true),
            &NoValues,
        );
        let names: Vec<&str> = r.candidates.iter().map(|c| c.value.as_str()).collect();
        assert!(names.contains(&"status"), "{names:?}");
        assert!(names.contains(&"migrate"), "{names:?}");
        assert_eq!(r.resolved.as_deref(), Some("executable.worktree"));
    }

    #[test]
    fn a_dash_offers_the_flags_of_the_resolved_command() {
        let graph = build(&Inputs::default());
        let r = complete(
            &graph,
            &request(Surface::Cli, "majordomus worktree status -", false),
            &NoValues,
        );
        assert!(r.candidates.iter().all(|c| c.kind == CandidateKind::Flag));
        assert!(r.candidates.iter().any(|c| c.value == "--repo"));
    }

    #[test]
    fn a_value_enum_completes_from_the_declaration() {
        let graph = build(&Inputs::default());
        // `--format` is a value enum on several commands; whichever carries it, the values
        // come from clap and not from a list here.
        let node = graph
            .commands
            .iter()
            .find(|n| n.arguments.iter().any(|a| !a.values.is_empty()))
            .expect("some command has a value enum");
        let arg = node
            .arguments
            .iter()
            .find(|a| !a.values.is_empty())
            .unwrap();
        let long = arg.long.clone().expect("a value enum is an option here");
        let line = format!("majordomus {} --{long}", node.path.join(" "));
        let r = complete(&graph, &request(Surface::Cli, &line, true), &NoValues);
        assert!(!r.candidates.is_empty(), "no values for --{long}");
        assert!(r.candidates.iter().all(|c| c.kind == CandidateKind::Value));
    }

    #[test]
    fn the_workflow_surface_offers_recipes_and_then_the_same_arguments() {
        let graph = build(&Inputs::default());
        let r = complete(&graph, &request(Surface::Workflow, "just", true), &NoValues);
        assert!(r
            .candidates
            .iter()
            .any(|c| c.value == "worktree-status" && c.kind == CandidateKind::Workflow));

        let r = complete(
            &graph,
            &request(Surface::Workflow, "just worktree-status -", false),
            &NoValues,
        );
        // The same flags the command line offers, from the same node.
        assert!(r.candidates.iter().any(|c| c.value == "--repo"));
        assert_eq!(r.resolved.as_deref(), Some("executable.worktree.status"));
    }

    #[test]
    fn a_dynamic_source_comes_from_the_resolver_and_never_from_the_engine() {
        struct Branches;
        impl ValueResolver for Branches {
            fn values(&self, source: ValueSource) -> Vec<String> {
                match source {
                    ValueSource::Branch => vec!["master".into(), "feature/x".into()],
                    _ => Vec::new(),
                }
            }
        }
        let graph = build(&Inputs::default());
        let r = complete(
            &graph,
            &request(Surface::Cli, "majordomus worktree path", true),
            &Branches,
        );
        let names: Vec<&str> = r.candidates.iter().map(|c| c.value.as_str()).collect();
        assert!(names.contains(&"master"), "{names:?}");
        // and with no resolver, nothing is invented
        let r = complete(
            &graph,
            &request(Surface::Cli, "majordomus worktree path", true),
            &NoValues,
        );
        assert!(r.candidates.iter().all(|c| c.kind != CandidateKind::Value));
    }

    #[test]
    fn an_unknown_command_does_not_panic() {
        let graph = build(&Inputs::default());
        for line in [
            "majordomus nonesuch --unknown",
            "majordomus worktree nonesuch",
            "just nonesuch --x",
            "majordomus",
        ] {
            let _ = complete(&graph, &request(Surface::Cli, line, true), &NoValues);
            let _ = complete(&graph, &request(Surface::Workflow, line, false), &NoValues);
        }
    }
}
