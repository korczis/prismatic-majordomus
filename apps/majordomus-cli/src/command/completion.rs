//! One completion engine, for every surface.
//!
//! A shell adapter's whole job is to say where the cursor is and to render what comes
//! back. It parses no command, knows no argument, holds no list of values and never grows
//! when a command is added. Everything else happens here, over the canonical graph:
//! the surface's name is resolved back to a command, the command's own arguments say what
//! may follow, and the values come from the registries that own them.
//!
//! `just bench-coverage --format <TAB>` and `majordomus bench coverage --format <TAB>`
//! therefore produce the same candidates from the same code: the first resolves
//! `bench-coverage` to `native.bench.coverage` and then both are the same question.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::graph::{CommandGraph, Surface};
use super::model::{ArgumentSpec, CommandId, CommandNode, Program, ValueSource, Visibility};
use super::values::{may_offer, ValueIndex};

/// The schema of a completion answer.
pub const SCHEMA: &str = "majordomus/completion/v1";

/// The most candidates an answer carries. A shell that is handed ten thousand values is
/// slower than one that is handed none, and no menu of that size is read by anyone.
pub const LIMIT: usize = 500;

/// What a shell is asking. The words are the command line with the program's own name
/// already removed, which is the one thing every adapter can do correctly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompletionRequest {
    /// Where the line was typed.
    pub surface: Surface,
    /// The words after the program's own name, as the shell split them.
    pub words: Vec<String>,
    /// The index in `words` of the word the cursor is in. A cursor past the last word is
    /// `words.len()`, which means a new, empty word.
    pub cursor: usize,
}

impl CompletionRequest {
    /// The text before the cursor in the word being completed.
    pub fn prefix(&self) -> &str {
        self.words.get(self.cursor).map(String::as_str).unwrap_or("")
    }

    /// The words before the one being completed.
    pub fn head(&self) -> &[String] {
        &self.words[..self.cursor.min(self.words.len())]
    }
}

/// What kind of thing a candidate is, so that a shell can group and decorate it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum CompletionKind {
    /// A command or a recipe.
    Command,
    /// A name a command also answers to.
    Alias,
    /// An option, with its dashes.
    Flag,
    /// A value of an argument.
    Value,
    /// Not a value but an instruction: complete a filesystem path here. The shell already
    /// knows how to do that better than any engine could, quoting and `~` included.
    Path,
    /// The same, restricted to directories.
    Directory,
}

/// One thing a person could type next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompletionCandidate {
    /// The text to insert.
    pub value: String,
    /// What the menu shows; the value itself unless something clearer exists.
    pub display: String,
    /// The line beside it, sanitised of anything a terminal would act on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What it is.
    pub kind: CompletionKind,
    /// Whether a space belongs after it.
    pub append_space: bool,
    /// Lower sorts first. Set by the engine's one ordering rule, never by an adapter.
    pub priority: i32,
}

/// The answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CompletionResponse {
    /// [`SCHEMA`].
    pub schema: String,
    /// The command the words resolved to, when they resolved to one. A shell may ignore
    /// it; `--debug` prints it, and the tests assert on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<CommandId>,
    /// Whether values that would have needed the value index were left out because no
    /// current one was found. Never an error: the structural candidates still came back.
    pub values_unavailable: bool,
    /// The candidates, already ordered.
    pub candidates: Vec<CompletionCandidate>,
}

impl CompletionResponse {
    /// The compact encoding a shell reads: one candidate per line, the value and its
    /// description separated by a tab. Both halves are free of tabs and newlines by
    /// construction, so a shell needs no parser.
    pub fn to_shell(&self) -> String {
        let mut out = String::new();
        for c in &self.candidates {
            out.push_str(&c.value);
            out.push('\t');
            out.push_str(c.description.as_deref().unwrap_or(""));
            out.push('\n');
        }
        out
    }

    /// The directive kinds present, for an adapter that asks whether to fall back to the
    /// shell's own file completion.
    pub fn directive(&self) -> Option<CompletionKind> {
        self.candidates
            .iter()
            .map(|c| c.kind)
            .find(|k| matches!(k, CompletionKind::Path | CompletionKind::Directory))
    }
}

/// The engine: a graph, and the values of this checkout when there are any.
pub struct Engine<'a> {
    graph: &'a CommandGraph,
    values: &'a ValueIndex,
}

impl<'a> Engine<'a> {
    /// An engine over a graph and a value index. An empty index is legitimate: the
    /// structural candidates do not need it.
    pub fn new(graph: &'a CommandGraph, values: &'a ValueIndex) -> Self {
        Engine { graph, values }
    }

    /// Answer one request.
    pub fn complete(&self, request: &CompletionRequest) -> CompletionResponse {
        let mut out = Answer::default();
        match request.surface {
            Surface::Just | Surface::Cockpit => self.surface_name(request, &mut out),
            Surface::Cli => self.native(request, request.head(), &mut out),
        }
        out.finish(request.prefix())
    }

    /// A surface whose first word is one name for a whole command. Completing that word
    /// offers the names; completing anything after it is the command's own arguments, and
    /// is answered by exactly the code the native command line uses.
    fn surface_name(&self, request: &CompletionRequest, out: &mut Answer) {
        if request.cursor == 0 {
            for c in self.graph.listed() {
                let name = match request.surface {
                    Surface::Cockpit => c.projections.cockpit.clone(),
                    _ => c.projections.just.clone(),
                };
                if let Some(name) = name {
                    out.command(c, name, CompletionKind::Command);
                }
                if request.surface != Surface::Cockpit {
                    for alias in &c.projections.just_aliases {
                        out.command(c, alias.clone(), CompletionKind::Alias);
                    }
                }
            }
            return;
        }
        let Some(node) = self
            .graph
            .resolve(request.surface, &request.words[..1])
            .cloned()
        else {
            return;
        };
        out.resolved = Some(node.id.clone());
        // Everything after the recipe name is forwarded to the command as it stands, so
        // the words the command sees are the ones after the name.
        let head: Vec<String> = request.words[1..request.cursor.min(request.words.len())].to_vec();
        self.arguments(&node, &head, request.prefix(), out);
    }

    /// The native command line: the words are the command's own path followed by its
    /// arguments, and how many of each is decided by the graph, not by counting dashes.
    fn native(&self, request: &CompletionRequest, head: &[String], out: &mut Answer) {
        let resolved = self.graph.longest_cli_prefix(head);
        let (node, used) = match resolved {
            Some((node, used)) => (node.clone(), used),
            None => {
                for c in self.graph.listed() {
                    if c.program == Program::Native && c.path.len() == 1 {
                        out.command(c, c.path[0].clone(), CompletionKind::Command);
                    }
                }
                return;
            }
        };
        out.resolved = Some(node.id.clone());
        if used == head.len() {
            // The cursor sits where a subcommand could go: offer them, and the command's
            // own arguments beside them.
            for c in self.graph.commands.iter().filter(|c| {
                c.program == Program::Native
                    && c.path.len() == node.path.len() + 1
                    && c.path.starts_with(&node.path)
                    && c.visibility == Visibility::Public
            }) {
                out.command(c, c.path[c.path.len() - 1].clone(), CompletionKind::Command);
            }
        }
        self.arguments(&node, &head[used.min(head.len())..], request.prefix(), out);
    }

    /// What may follow a command that is already known: the value of the option just
    /// typed, the options themselves, or the next positional's values.
    fn arguments(&self, node: &CommandNode, rest: &[String], prefix: &str, out: &mut Answer) {
        // `--flag <TAB>`: the value of that flag and nothing else.
        if let Some(previous) = rest.last() {
            if previous.starts_with('-') && !previous.contains('=') {
                if let Some(arg) = node.argument_for_word(previous) {
                    if arg.takes_value {
                        self.values(arg, out);
                        return;
                    }
                }
            }
        }
        if prefix.starts_with('-') || prefix.is_empty() {
            for arg in node.arguments.iter().filter(|a| !a.positional) {
                if let Some(long) = &arg.long {
                    out.flag(format!("--{long}"), arg);
                }
            }
        }
        if prefix.starts_with('-') {
            return;
        }
        // The next positional that has not been given yet. Words that begin with a dash,
        // and the values that follow a flag, are not positionals.
        let given = count_positionals(node, rest);
        let positionals: Vec<&ArgumentSpec> =
            node.arguments.iter().filter(|a| a.positional).collect();
        let next = positionals
            .get(given)
            .or_else(|| positionals.last().filter(|a| a.variadic));
        if let Some(arg) = next {
            self.values(arg, out);
        }
    }

    /// The candidate values of one argument. A credential is never completed; an
    /// enumerated argument carries its own values; a path becomes a directive to the
    /// shell; an identifier comes from the registry that owns it.
    fn values(&self, arg: &ArgumentSpec, out: &mut Answer) {
        if !may_offer(arg.sensitivity) {
            return;
        }
        match &arg.values {
            ValueSource::None => {}
            ValueSource::Enumerated { values } => {
                for v in values {
                    out.value(v.value.clone(), v.description.clone());
                }
            }
            ValueSource::Path { directories_only } => out.path(*directories_only),
            ValueSource::Registry { registry } => {
                let candidates = self.values.candidates(*registry);
                if candidates.is_empty() {
                    out.values_unavailable = true;
                    return;
                }
                for c in candidates {
                    out.value(c.value.clone(), c.description.clone());
                }
            }
        }
    }
}

/// How many positional values a command has already been given. A word that begins with a
/// dash is an option; the word after an option that takes a value is that value;
/// everything else is a positional.
fn count_positionals(node: &CommandNode, rest: &[String]) -> usize {
    let mut count = 0;
    let mut skip_next = false;
    for word in rest {
        if skip_next {
            skip_next = false;
            continue;
        }
        if word.starts_with('-') {
            if !word.contains('=') {
                skip_next = node
                    .argument_for_word(word)
                    .is_some_and(|a| a.takes_value && !a.variadic);
            }
            continue;
        }
        count += 1;
    }
    count
}

/// Candidates being collected, with the one ordering rule and the one sanitiser.
#[derive(Default)]
struct Answer {
    candidates: Vec<CompletionCandidate>,
    resolved: Option<CommandId>,
    values_unavailable: bool,
}

impl Answer {
    fn command(&mut self, node: &CommandNode, name: String, kind: CompletionKind) {
        let mut description = node.summary.clone();
        if let Some(reason) = &node.availability.reason {
            description = format!("{description} — unavailable: {reason}");
        }
        if let Some(deprecation) = &node.deprecation {
            description = format!("{description} — deprecated: {}", deprecation.note);
        }
        let priority = match (node.availability.available, node.deprecation.is_some(), kind) {
            (true, false, CompletionKind::Command) => 0,
            (true, false, _) => 10,
            (true, true, _) => 20,
            (false, _, _) => 30,
        };
        self.candidates.push(CompletionCandidate {
            display: name.clone(),
            value: name,
            description: sanitise(&description),
            kind,
            append_space: true,
            priority,
        });
    }

    fn flag(&mut self, name: String, arg: &ArgumentSpec) {
        self.candidates.push(CompletionCandidate {
            display: name.clone(),
            value: name,
            description: sanitise(&arg.help),
            kind: CompletionKind::Flag,
            // An option that takes a value wants one next; a flag is finished.
            append_space: true,
            priority: 40,
        });
    }

    fn value(&mut self, value: String, description: Option<String>) {
        self.candidates.push(CompletionCandidate {
            display: value.clone(),
            value,
            description: description.as_deref().and_then(sanitise),
            kind: CompletionKind::Value,
            append_space: true,
            priority: 5,
        });
    }

    fn path(&mut self, directories_only: bool) {
        self.candidates.push(CompletionCandidate {
            value: String::new(),
            display: if directories_only {
                "<directory>".into()
            } else {
                "<path>".into()
            },
            description: Some("a path; the shell completes it".into()),
            kind: if directories_only {
                CompletionKind::Directory
            } else {
                CompletionKind::Path
            },
            append_space: false,
            priority: 50,
        });
    }

    fn finish(mut self, prefix: &str) -> CompletionResponse {
        self.candidates.retain(|c| {
            matches!(c.kind, CompletionKind::Path | CompletionKind::Directory)
                || c.value.starts_with(prefix)
        });
        self.candidates.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.value.cmp(&b.value))
        });
        self.candidates.dedup_by(|a, b| a.value == b.value && a.kind == b.kind);
        self.candidates.truncate(LIMIT);
        CompletionResponse {
            schema: SCHEMA.to_string(),
            resolved: self.resolved,
            values_unavailable: self.values_unavailable,
            candidates: self.candidates,
        }
    }
}

/// A description a terminal will print as text and nothing else: no escape sequence, no
/// newline, no tab, no other control character. Repository metadata reaches a terminal
/// through here, and a title that tried to move the cursor arrives as ordinary words.
///
/// ```
/// use majordomus_cli::command::completion::sanitise;
/// assert_eq!(sanitise("plain"), Some("plain".to_string()));
/// assert_eq!(sanitise("a\u{1b}[31mred\u{7}"), Some("a[31mred".to_string()));
/// assert_eq!(sanitise("two\nlines\tapart"), Some("two lines apart".to_string()));
/// assert_eq!(sanitise("   "), None);
/// ```
pub fn sanitise(text: &str) -> Option<String> {
    let cleaned: String = text
        .chars()
        .filter_map(|c| match c {
            '\n' | '\r' | '\t' => Some(' '),
            c if c.is_control() => None,
            // The Unicode bidirectional and interlinear controls reorder or hide what
            // follows them in a terminal as effectively as an escape sequence does.
            '\u{200e}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => None,
            c => Some(c),
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Facts;

    fn share() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")
    }

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn graph() -> CommandGraph {
        let root = root();
        CommandGraph::build(&root, &share(), &Facts::read(&root))
    }

    fn ask(graph: &CommandGraph, surface: Surface, line: &[&str], cursor: usize) -> CompletionResponse {
        let values = ValueIndex::default();
        let engine = Engine::new(graph, &values);
        engine.complete(&CompletionRequest {
            surface,
            words: line.iter().map(|s| (*s).to_string()).collect(),
            cursor,
        })
    }

    fn values_of(r: &CompletionResponse) -> Vec<&str> {
        r.candidates.iter().map(|c| c.value.as_str()).collect()
    }

    #[test]
    fn the_first_word_of_the_bridge_offers_every_recipe() {
        let g = graph();
        let r = ask(&g, Surface::Just, &[""], 0);
        let names = values_of(&r);
        assert!(names.contains(&"bench-coverage"), "{names:?}");
        assert!(names.contains(&"doctor"), "{names:?}");
        assert!(r.resolved.is_none());
    }

    #[test]
    fn a_prefix_narrows_and_nothing_else_comes_back() {
        let g = graph();
        let r = ask(&g, Surface::Just, &["bench-"], 0);
        assert!(!r.candidates.is_empty());
        assert!(r.candidates.iter().all(|c| c.value.starts_with("bench-")));
    }

    #[test]
    fn the_bridge_and_the_command_line_complete_an_argument_identically() {
        let g = graph();
        let by_just = ask(&g, Surface::Just, &["bench-coverage", "--format", ""], 2);
        let by_cli = ask(&g, Surface::Cli, &["bench", "coverage", "--format", ""], 3);
        assert_eq!(values_of(&by_just), values_of(&by_cli));
        assert!(values_of(&by_just).contains(&"json"), "{:?}", values_of(&by_just));
        assert_eq!(
            by_just.resolved.as_ref().map(|i| i.as_str()),
            Some("native.bench.coverage")
        );
        assert_eq!(by_just.resolved, by_cli.resolved);
    }

    #[test]
    fn a_subcommand_is_offered_where_one_may_go() {
        let g = graph();
        let r = ask(&g, Surface::Cli, &["capabilities", ""], 1);
        let names = values_of(&r);
        for expected in ["list", "describe", "validate"] {
            assert!(names.contains(&expected), "{expected} missing from {names:?}");
        }
    }

    #[test]
    fn options_are_offered_when_a_dash_is_typed_and_values_are_not() {
        let g = graph();
        let r = ask(&g, Surface::Cli, &["capabilities", "list", "--"], 2);
        assert!(r
            .candidates
            .iter()
            .all(|c| c.kind == CompletionKind::Flag), "{:?}", values_of(&r));
        assert!(values_of(&r).contains(&"--format"));
    }

    #[test]
    fn a_path_argument_is_handed_back_to_the_shell() {
        let g = graph();
        let r = ask(&g, Surface::Cli, &["distribution", "metadata", "--record", ""], 3);
        assert_eq!(r.directive(), Some(CompletionKind::Path));
    }

    #[test]
    fn an_identifier_argument_says_so_when_no_value_index_has_been_built() {
        let g = graph();
        let r = ask(&g, Surface::Cli, &["capabilities", "describe", ""], 2);
        assert!(
            r.values_unavailable,
            "an empty index is reported, never silently empty"
        );
    }

    #[test]
    fn an_identifier_argument_is_completed_from_the_registry_that_owns_it() {
        let g = graph();
        let mut values = ValueIndex::default();
        values.values.insert(
            super::super::model::ValueRegistry::Capability.key().into(),
            vec![
                super::super::values::Candidate::described("objects.get", "Read one object"),
                super::super::values::Candidate::bare("repository.info"),
            ],
        );
        let engine = Engine::new(&g, &values);
        let r = engine.complete(&CompletionRequest {
            surface: Surface::Cli,
            words: vec!["capabilities".into(), "describe".into(), "obj".into()],
            cursor: 2,
        });
        assert_eq!(values_of(&r), vec!["objects.get"]);
        assert!(!r.values_unavailable);
    }

    #[test]
    fn nonsense_is_answered_and_never_panics() {
        let g = graph();
        for (line, cursor) in [
            (vec![], 0usize),
            (vec![""], 5),
            (vec!["nope"], 0),
            (vec!["nope", "--unknown", ""], 2),
            (vec!["--", "--", "--"], 3),
            (vec!["bench", "coverage", "--format=json", ""], 3),
            (vec!["příkaz", "🙂"], 1),
        ] {
            let r = ask(&g, Surface::Cli, &line.iter().map(|s| *s).collect::<Vec<_>>(), cursor);
            assert!(r.candidates.len() <= LIMIT);
        }
    }

    #[test]
    fn an_unavailable_command_is_shown_last_and_says_why() {
        let root = root();
        let g = CommandGraph::build(&root, &share(), &Facts::none(&root));
        let r = ask(&g, Surface::Just, &[""], 0);
        let doctor = r
            .candidates
            .iter()
            .find(|c| c.value == "doctor")
            .expect("doctor is offered");
        assert!(doctor.priority >= 30);
        assert!(doctor
            .description
            .as_deref()
            .is_some_and(|d| d.contains("unavailable")));
    }

    #[test]
    fn descriptions_carry_nothing_a_terminal_would_act_on() {
        let g = graph();
        let r = ask(&g, Surface::Just, &[""], 0);
        for c in &r.candidates {
            let text = format!("{}{}", c.value, c.description.clone().unwrap_or_default());
            assert!(
                !text.chars().any(|ch| ch.is_control()),
                "{text:?} carries a control character"
            );
        }
        let shell = r.to_shell();
        assert_eq!(
            shell.lines().count(),
            r.candidates.len(),
            "one line per candidate, whatever the metadata said"
        );
    }
}
