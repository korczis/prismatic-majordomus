//! One completion engine, for every surface that has a command line.
//!
//! A shell adapter's whole job is to say what has been typed and where the cursor is. It
//! knows no command, no flag, no accepted value and no repository; it sends a request and
//! renders what comes back. Everything a candidate depends on is the canonical graph and
//! the registries this repository already keeps, which is why `just providers-use <TAB>`
//! and `majordomus providers use <TAB>` cannot drift: the first resolves its recipe to the
//! same node the second walks to, and one engine answers both.
//!
//! What it may not do is as much of the contract as what it does. No network, ever. No
//! build. No server. No value taken from an environment variable a command declares as a
//! credential. When the repository's index would be needed for an answer and no answer is
//! cheaply available, the reply is empty rather than slow: a completion that takes two
//! seconds is worse than one that offers nothing.
//!
//! ```
//! use majordomus_cli::control::completion::{self, Request};
//! use majordomus_cli::control::Surface;
//! let g = majordomus_cli::control::graph::of_this_executable();
//! // the words of a command line, and which one the cursor is in
//! let r = Request::new(Surface::Cli, vec!["worktree".into(), "".into()], 1);
//! let answer = completion::answer(&g, &r, &completion::Offline);
//! assert!(answer.candidates.iter().any(|c| c.value == "create"));
//! ```

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::control::graph::{Argument, CommandGraph, CommandNode, RegistryValues, ValueSource};
use crate::control::Surface;

/// What the shell adapter reports: the words of the command line without the program's own
/// name, and which of them the cursor is in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CompletionRequest")]
pub struct Request {
    /// Which command line this is.
    pub surface: Surface,
    /// The words, without the program name. The word under the cursor may be empty.
    pub words: Vec<String>,
    /// The index in `words` of the word being completed.
    pub cursor_word: usize,
    /// Where the shell is, for a path candidate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
}

impl Request {
    /// A request over words and a cursor.
    pub fn new(surface: Surface, words: Vec<String>, cursor_word: usize) -> Self {
        Request {
            surface,
            words,
            cursor_word,
            cwd: None,
        }
    }

    /// The word being completed; empty when the cursor is past the last word.
    pub fn prefix(&self) -> &str {
        self.words
            .get(self.cursor_word)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// The words before the cursor.
    pub fn before(&self) -> &[String] {
        let end = self.cursor_word.min(self.words.len());
        &self.words[..end]
    }
}

/// What kind of thing a candidate is, so a shell that groups its menu can.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CompletionCandidateKind")]
pub enum CandidateKind {
    /// A command or a subcommand.
    Command,
    /// A workflow discovered outside the executable.
    Workflow,
    /// A flag.
    Flag,
    /// A value of an argument.
    Value,
    /// A path; the shell completes it itself.
    Path,
}

/// One thing that may be typed next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CompletionCandidate")]
pub struct Candidate {
    /// What is inserted.
    pub value: String,
    /// One line beside it, when the shell shows descriptions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What it is.
    pub kind: CandidateKind,
    /// Whether a space follows the insertion.
    pub append_space: bool,
}

/// The answer, with the fingerprint of the graph it was computed from so that a cache can
/// tell whether it is still current.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CompletionAnswer")]
pub struct Answer {
    /// The candidates, in the order they should be offered.
    pub candidates: Vec<Candidate>,
    /// The graph this was computed from.
    pub fingerprint: String,
}

/// Where dynamic values come from. The engine never reaches for one itself: it asks, and a
/// resolver that cannot answer cheaply says so by answering with nothing.
pub trait Values {
    /// The identities this registry holds, for the prefix typed so far.
    fn resolve(&self, registry: RegistryValues, prefix: &str) -> Vec<Candidate>;
}

/// The resolver that answers only from what is already in this process: no subprocess, no
/// index build, no repository read.
pub struct Offline;

impl Values for Offline {
    fn resolve(&self, _registry: RegistryValues, _prefix: &str) -> Vec<Candidate> {
        Vec::new()
    }
}

/// Everything that may be offered for one request.
pub fn answer(graph: &CommandGraph, request: &Request, values: &dyn Values) -> Answer {
    let prefix = request.prefix();
    let mut candidates = match request.surface {
        Surface::Just => just(graph, request, values, prefix),
        _ => cli(graph, request, values, prefix),
    };
    candidates.retain(|c| c.value.starts_with(prefix));
    candidates.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.value.cmp(&b.value)));
    candidates.dedup_by(|a, b| a.value == b.value && a.kind == b.kind);
    Answer {
        candidates,
        fingerprint: graph.fingerprint.clone(),
    }
}

/// The executable's own command line: walk the words into the deepest command that matches,
/// then offer that command's subcommands, flags or values.
fn cli(
    graph: &CommandGraph,
    request: &Request,
    values: &dyn Values,
    prefix: &str,
) -> Vec<Candidate> {
    let before = request.before();
    let mut path: Vec<String> = Vec::new();
    let mut consumed = 0usize;
    for word in before {
        if word.starts_with('-') {
            break;
        }
        let mut candidate_path = path.clone();
        candidate_path.push(word.clone());
        if graph.commands.iter().any(|c| c.path == candidate_path) {
            path = candidate_path;
            consumed += 1;
        } else {
            break;
        }
    }
    let node = graph.commands.iter().find(|c| c.path == path);
    if prefix.starts_with('-') {
        return node.map(flags).unwrap_or_default();
    }
    if let Some(candidates) = value_of_preceding_flag(node, before, values) {
        return candidates;
    }
    // still naming the command: offer what is directly under the words matched so far
    if consumed == before.len() {
        let mut out = children(graph, &path);
        if let Some(node) = node {
            out.extend(positional_values(node, values));
        }
        return out;
    }
    node.map(|n| positional_values(n, values))
        .unwrap_or_default()
}

/// The `just` surface: the first word is a recipe, which resolves to the same node the
/// command line would have walked to, and everything after it is that node's own.
fn just(
    graph: &CommandGraph,
    request: &Request,
    values: &dyn Values,
    prefix: &str,
) -> Vec<Candidate> {
    let before = request.before();
    let Some(recipe) = before.first() else {
        return graph
            .commands
            .iter()
            .filter(|c| c.availability.is_available())
            .filter_map(|c| {
                c.projections.just.as_ref().map(|name| Candidate {
                    value: name.clone(),
                    description: Some(c.summary.clone()),
                    kind: match c.execution {
                        crate::control::graph::Execution::External { .. } => {
                            CandidateKind::Workflow
                        }
                        _ => CandidateKind::Command,
                    },
                    append_space: true,
                })
            })
            .collect();
    };
    let Some(node) = graph.resolve(Surface::Just, recipe) else {
        return Vec::new();
    };
    if prefix.starts_with('-') {
        return flags(node);
    }
    if let Some(candidates) = value_of_preceding_flag(Some(node), before, values) {
        return candidates;
    }
    positional_values(node, values)
}

/// The commands directly under a path.
fn children(graph: &CommandGraph, path: &[String]) -> Vec<Candidate> {
    graph
        .commands
        .iter()
        .filter(|c| c.path.len() == path.len() + 1 && c.path.starts_with(path))
        .filter(|c| c.availability.is_available())
        .filter(|c| !c.projections.cli.is_empty())
        .map(|c| Candidate {
            value: c.path.last().cloned().unwrap_or_default(),
            description: Some(c.summary.clone()),
            kind: CandidateKind::Command,
            append_space: true,
        })
        .collect()
}

/// The flags of one command.
fn flags(node: &CommandNode) -> Vec<Candidate> {
    node.arguments
        .iter()
        .filter(|a| !a.positional)
        .filter_map(|a| {
            a.long.as_ref().map(|long| Candidate {
                value: format!("--{long}"),
                description: Some(a.help.clone()).filter(|h| !h.is_empty()),
                kind: CandidateKind::Flag,
                append_space: !a.takes_value,
            })
        })
        .collect()
}

/// When the word before the cursor is a flag that takes a value, that argument's values.
fn value_of_preceding_flag(
    node: Option<&CommandNode>,
    before: &[String],
    values: &dyn Values,
) -> Option<Vec<Candidate>> {
    let node = node?;
    let last = before.last()?;
    let name = last.strip_prefix("--")?;
    let arg = node
        .arguments
        .iter()
        .find(|a| a.long.as_deref() == Some(name))?;
    if !arg.takes_value {
        return None;
    }
    Some(values_of(arg, values))
}

/// The values of every positional argument of a command that has not been given yet. The
/// engine does not track which have been consumed: offering the values of a command's
/// positionals is right whichever one is being typed, and being wrong about the count is
/// worse than offering one candidate too many.
fn positional_values(node: &CommandNode, values: &dyn Values) -> Vec<Candidate> {
    node.arguments
        .iter()
        .filter(|a| a.positional)
        .flat_map(|a| values_of(a, values))
        .collect()
}

/// One argument's values, from its declared source.
fn values_of(arg: &Argument, values: &dyn Values) -> Vec<Candidate> {
    match &arg.values {
        ValueSource::None => Vec::new(),
        ValueSource::Path => vec![Candidate {
            value: String::new(),
            description: Some("a path".into()),
            kind: CandidateKind::Path,
            append_space: false,
        }],
        ValueSource::Enum { values } => values
            .iter()
            .map(|v| Candidate {
                value: v.value.clone(),
                description: v.help.clone(),
                kind: CandidateKind::Value,
                append_space: true,
            })
            .collect(),
        ValueSource::Registry { registry } => values.resolve(*registry, ""),
    }
}

/// The resolver a shell gets: identities this repository already holds, answered from the
/// cheapest place that holds them.
///
/// Capability and command identities are in this process the moment the graph is: no file
/// is read for them. Branches are one local `git` call. The kinds of the layer and the Why
/// catalogue are behind the repository's index, which costs seconds to build, so they are
/// answered with nothing rather than with a pause — a completion that stalls is worse than
/// one that stays quiet.
pub struct Repository {
    root: PathBuf,
}

impl Repository {
    /// The resolver for the directory the shell is standing in.
    pub fn here() -> Self {
        Repository {
            root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// The resolver for one root.
    pub fn at(root: PathBuf) -> Self {
        Repository { root }
    }

    fn branches(&self) -> Vec<Candidate> {
        let out = std::process::Command::new("git")
            .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
            .current_dir(&self.root)
            .output();
        let Ok(out) = out else { return Vec::new() };
        if !out.status.success() {
            return Vec::new();
        }
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|name| Candidate {
                value: name.trim().to_string(),
                description: None,
                kind: CandidateKind::Value,
                append_space: true,
            })
            .filter(|c| !c.value.is_empty())
            .collect()
    }
}

impl Values for Repository {
    fn resolve(&self, registry: RegistryValues, _prefix: &str) -> Vec<Candidate> {
        match registry {
            RegistryValues::Branch => self.branches(),
            RegistryValues::Capability => {
                let built = crate::capability::CapabilityRegistry::builder()
                    .with_modules(crate::capability::builtin::modules())
                    .build();
                built
                    .map(|r| {
                        r.iter()
                            .map(|c| Candidate {
                                value: c.id.as_str().to_string(),
                                description: Some(c.title.clone()),
                                kind: CandidateKind::Value,
                                append_space: true,
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            }
            RegistryValues::Command => crate::control::graph::of_this_executable()
                .commands
                .iter()
                .filter(|c| !c.group)
                .map(|c| Candidate {
                    value: c.id.clone(),
                    description: Some(c.summary.clone()),
                    kind: CandidateKind::Value,
                    append_space: true,
                })
                .collect(),
            // behind the index, and the index is not something a keystroke may pay for
            RegistryValues::ObjectKind | RegistryValues::Moment => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::graph;

    fn request(surface: Surface, words: &[&str]) -> Request {
        let words: Vec<String> = words.iter().map(|w| w.to_string()).collect();
        let cursor = words.len().saturating_sub(1);
        Request::new(surface, words, cursor)
    }

    #[test]
    fn the_first_word_offers_the_top_level_commands() {
        let g = graph::of_this_executable();
        let a = answer(&g, &request(Surface::Cli, &[""]), &Offline);
        let values: Vec<&str> = a.candidates.iter().map(|c| c.value.as_str()).collect();
        assert!(values.contains(&"worktree"), "{values:?}");
        assert!(values.contains(&"capabilities"), "{values:?}");
    }

    #[test]
    fn a_prefix_narrows_and_the_description_is_the_commands_own() {
        let g = graph::of_this_executable();
        let a = answer(&g, &request(Surface::Cli, &["cap"]), &Offline);
        assert_eq!(a.candidates.len(), 1, "{:?}", a.candidates);
        assert_eq!(a.candidates[0].value, "capabilities");
        assert!(a.candidates[0].description.is_some());
    }

    #[test]
    fn a_subcommand_offers_its_own_children_and_then_its_flags() {
        let g = graph::of_this_executable();
        let children = answer(&g, &request(Surface::Cli, &["worktree", ""]), &Offline);
        let values: Vec<&str> = children
            .candidates
            .iter()
            .map(|c| c.value.as_str())
            .collect();
        assert!(values.contains(&"create"), "{values:?}");
        let flags = answer(
            &g,
            &request(Surface::Cli, &["worktree", "create", "--"]),
            &Offline,
        );
        assert!(
            flags.candidates.iter().all(|c| c.value.starts_with("--")),
            "{:?}",
            flags.candidates
        );
        assert!(!flags.candidates.is_empty());
    }

    #[test]
    fn an_enum_argument_offers_exactly_what_the_declaration_accepts() {
        let g = graph::of_this_executable();
        let a = answer(
            &g,
            &request(Surface::Cli, &["worktree", "status", "--format", ""]),
            &Offline,
        );
        let values: Vec<&str> = a.candidates.iter().map(|c| c.value.as_str()).collect();
        assert!(values.contains(&"json"), "{values:?}");
    }

    #[test]
    fn the_just_surface_resolves_to_the_same_node_as_the_command_line() {
        let g = graph::of_this_executable();
        let by_just = answer(
            &g,
            &request(Surface::Just, &["worktree-status", "--format", ""]),
            &Offline,
        );
        let by_cli = answer(
            &g,
            &request(Surface::Cli, &["worktree", "status", "--format", ""]),
            &Offline,
        );
        assert_eq!(by_just.candidates, by_cli.candidates);
        assert!(!by_just.candidates.is_empty());
    }

    #[test]
    fn an_incomplete_or_unknown_command_line_answers_rather_than_panicking() {
        let g = graph::of_this_executable();
        for words in [
            vec!["nosuchcommand", ""],
            vec!["worktree", "nosuch", "--x"],
            vec![""],
            vec!["--"],
        ] {
            let _ = answer(&g, &request(Surface::Cli, &words), &Offline);
        }
    }
}
