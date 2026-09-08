//! Repository entry: the activation a shell evaluates, and the banner a person reads.
//!
//! These are two outputs of one snapshot, kept apart on purpose. The activation is a
//! payload — nothing but shell statements, safe to `eval`, with no prose in it. The
//! banner is a rendering — prose, colour and width, never evaluated by anything. Mixing
//! them is how a repository entry becomes a security surface, so `.envrc` asks for them
//! separately and this module never emits one inside the other.
//!
//! Neither builds an index, runs a build, spawns a subprocess or touches the network. The
//! branch is read from `.git/HEAD`, the running server's address from the lease it
//! already writes, and everything about commands from the graph. What cannot be answered
//! that cheaply is not claimed at all.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::facts::Facts;
use super::graph::CommandGraph;
use super::model::Program;

/// Where the banner remembers what it last showed, relative to the repository root: one
/// small file per terminal session, under the checkout-local state directory.
pub const STATE_DIR: &str = ".ai/local/state/command/entry";

/// The variable that chooses how much the banner says.
pub const BANNER_ENV: &str = "MAJORDOMUS_BANNER";

/// The variable the activation exports so that anything downstream knows which checkout
/// it is in without discovering it again.
pub const ROOT_ENV: &str = "MAJORDOMUS_REPOSITORY";

/// The variable naming the graph's structure, so a tool can tell whether what it cached
/// is still current without asking.
pub const STRUCTURE_ENV: &str = "MAJORDOMUS_COMMAND_STRUCTURE";

/// How much the banner says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerMode {
    /// Full on the first entry and after a material change, nothing when nothing changed.
    Auto,
    /// Always the whole thing.
    Full,
    /// Always one line and the entry commands.
    Compact,
    /// Nothing, ever.
    Off,
}

impl BannerMode {
    /// The mode a name selects.
    ///
    /// ```
    /// use majordomus_cli::command::activate::BannerMode;
    /// assert_eq!(BannerMode::parse("compact"), Some(BannerMode::Compact));
    /// assert_eq!(BannerMode::parse("nonsense"), None);
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "auto" => Some(BannerMode::Auto),
            "full" => Some(BannerMode::Full),
            "compact" => Some(BannerMode::Compact),
            "off" | "none" | "0" => Some(BannerMode::Off),
            _ => None,
        }
    }

    /// The mode this environment asks for. A machine — a pipe, a CI runner — is never
    /// shown a banner, whatever the variable says, because nothing there reads it.
    pub fn from_environment(interactive: bool) -> Self {
        if !interactive || std::env::var_os("CI").is_some() {
            return BannerMode::Off;
        }
        std::env::var(BANNER_ENV)
            .ok()
            .and_then(|v| BannerMode::parse(&v))
            .unwrap_or(BannerMode::Auto)
    }
}

/// The shells the activation payload can be written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    /// direnv's own evaluation environment, which is bash with direnv's library loaded.
    Direnv,
    /// Plain POSIX-ish shells: bash and zsh take the same statements.
    Posix,
}

impl Shell {
    /// The shell a name selects.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "direnv" => Some(Shell::Direnv),
            "bash" | "zsh" | "posix" | "sh" => Some(Shell::Posix),
            _ => None,
        }
    }
}

/// What repository entry found, cheaply. The banner renders this; nothing else discovers
/// anything of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The repository root.
    pub root: PathBuf,
    /// The name a person calls it: the root's own directory name.
    pub name: String,
    /// The branch, or the short commit when the head is detached; `None` outside git.
    pub head: Option<String>,
    /// The address of the shared server, when one has written its lease.
    pub server: Option<String>,
    /// How many commands are listed, and how many of them can run here.
    pub commands: (usize, usize),
    /// The graph's structure fingerprint.
    pub structure: String,
    /// What the banner suggests running, in the graph's own order.
    pub entry_commands: Vec<(String, String)>,
    /// What could not be answered: an unbuilt executable, a broken workflow object.
    pub notes: Vec<String>,
}

impl Entry {
    /// Read the entry state of a checkout from the graph and the facts.
    pub fn read(root: &Path, graph: &CommandGraph, facts: &Facts) -> Self {
        let listed = graph.listed();
        let available = listed.iter().filter(|c| c.availability.available).count();
        let entry_commands = listed
            .iter()
            .filter(|c| c.tags.iter().any(|t| t == "entry") && c.availability.available)
            .take(4)
            .filter_map(|c| {
                Some((
                    format!("just {}", c.projections.just.clone()?),
                    c.summary.clone(),
                ))
            })
            .collect();
        let mut notes = Vec::new();
        if facts.native.is_none() {
            notes.push("the Rust executable is not built".to_string());
        }
        for d in graph.diagnostics.iter().filter(|d| d.fatal) {
            notes.push(d.message.clone());
        }
        Entry {
            name: root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "repository".to_string()),
            head: head(root),
            server: server_url(root),
            commands: (available, listed.len()),
            structure: graph.structure.clone(),
            entry_commands,
            notes,
            root: root.to_path_buf(),
        }
    }

    /// What the banner is keyed on: everything a person would want to be told again.
    /// Deliberately not the working tree's cleanliness — reading that costs more than the
    /// whole of repository entry is allowed to.
    pub fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        for part in [
            self.root.to_string_lossy().as_ref(),
            self.head.as_deref().unwrap_or("-"),
            self.server.as_deref().unwrap_or("-"),
            &self.structure,
            &self.commands.0.to_string(),
            &self.notes.join("|"),
        ] {
            h.update(part.as_bytes());
            h.update(b"\0");
        }
        format!("{:x}", h.finalize())
    }
}

/// The branch of a checkout, read from `.git/HEAD` — a file, not a subprocess. In a
/// worktree `.git` is a file naming the real directory, and that is followed once.
fn head(root: &Path) -> Option<String> {
    let dot_git = root.join(".git");
    let git_dir = if dot_git.is_dir() {
        dot_git
    } else {
        let text = std::fs::read_to_string(&dot_git).ok()?;
        let rest = text.trim().strip_prefix("gitdir:")?.trim().to_string();
        let path = PathBuf::from(&rest);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    };
    let text = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let text = text.trim();
    Some(match text.strip_prefix("ref: refs/heads/") {
        Some(branch) => branch.to_string(),
        None => text.chars().take(8).collect(),
    })
}

/// The shared server's address, from the lease it writes when it starts. No probe, no
/// connection: a lease that is stale says so when something tries to use it, and a banner
/// is not something that should be finding out.
fn server_url(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(".ai/local").join(crate::lease::LEASE_PATH)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("url")
        .and_then(|v| v.as_str())
        .and_then(super::completion::sanitise)
}

/// The activation payload: shell statements and nothing else.
///
/// Every value written into it is a single-quoted literal with the quotes stripped from
/// the value first, so a repository whose path or branch carried a quote, a `$(...)` or a
/// newline cannot become a command. The payload is meant to be `eval`ed, which is exactly
/// why it may not contain one word that came from anywhere unchecked.
pub fn activation(entry: &Entry, facts: &Facts, shell: Shell) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {} — Majordomus repository activation", entry.name);
    let _ = writeln!(
        out,
        "export {ROOT_ENV}={}",
        sh_quote(&entry.root.to_string_lossy())
    );
    let _ = writeln!(out, "export {STRUCTURE_ENV}={}", sh_quote(&entry.structure));
    if let Some(native) = &facts.native {
        if let Some(dir) = native.parent() {
            let dir = sh_quote(&dir.to_string_lossy());
            match shell {
                Shell::Direnv => {
                    let _ = writeln!(out, "PATH_add {dir}");
                }
                Shell::Posix => {
                    let _ = writeln!(out, "PATH={dir}:$PATH; export PATH");
                }
            }
        }
    }
    if facts.shell.is_some() {
        let dir = sh_quote(&entry.root.join("bin").to_string_lossy());
        match shell {
            Shell::Direnv => {
                let _ = writeln!(out, "PATH_add {dir}");
            }
            Shell::Posix => {
                let _ = writeln!(out, "PATH={dir}:$PATH; export PATH");
            }
        }
    }
    if shell == Shell::Direnv {
        for path in watches(entry, facts) {
            let _ = writeln!(out, "watch_file {}", sh_quote(&path));
        }
    }
    out
}

/// The files direnv should watch: exactly the inputs the graph and the facts were read
/// from. One dependency declaration serves the cache, the watch list and the report; a
/// second list of paths in `.envrc` would be the same knowledge written twice.
pub fn watches(entry: &Entry, facts: &Facts) -> Vec<String> {
    let mut out = vec![
        "share/commands.yaml".to_string(),
        super::workflow::DIRECTORY.to_string(),
    ];
    if facts.git {
        out.push(".git/HEAD".to_string());
    }
    if let Some(native) = &facts.native {
        if let Ok(relative) = native.strip_prefix(&entry.root) {
            out.push(relative.to_string_lossy().into_owned());
        }
    }
    out.retain(|p| entry.root.join(p).exists());
    out
}

/// A shell word: a single-quoted literal with every quote and control character removed
/// from the value first. Nothing that reaches `eval` was ever able to end its own quoting.
///
/// ```
/// use majordomus_cli::command::activate::sh_quote;
/// assert_eq!(sh_quote("/tmp/x"), "'/tmp/x'");
/// assert_eq!(sh_quote("a'; rm -rf /; '"), "'a; rm -rf /; '");
/// assert!(!sh_quote("a\nb").contains('\n'));
/// ```
pub fn sh_quote(word: &str) -> String {
    let cleaned: String = word
        .chars()
        .filter(|c| *c != '\'' && !c.is_control())
        .collect();
    format!("'{cleaned}'")
}

/// The banner, or nothing. `auto` shows the whole thing when the entry state is one this
/// session has not seen, and stays silent when it is: re-entering a directory a hundred
/// times in one session prints once.
pub fn banner(entry: &Entry, mode: BannerMode, width: usize, colour: bool) -> Option<String> {
    let decided = match mode {
        BannerMode::Off => return None,
        BannerMode::Full => Rendering::Full,
        BannerMode::Compact => Rendering::Compact,
        BannerMode::Auto => match remember(entry) {
            Seen::First => Rendering::Full,
            Seen::Changed => Rendering::Compact,
            Seen::Same => return None,
        },
    };
    Some(match decided {
        Rendering::Full => full(entry, width, colour),
        Rendering::Compact => compact(entry, colour),
    })
}

enum Rendering {
    Full,
    Compact,
}

/// Whether this session has seen this entry state before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    /// Nothing was remembered for this session.
    First,
    /// Something was, and it was different.
    Changed,
    /// Something was, and it was the same.
    Same,
}

/// Compare the entry state with what this session last saw, and record the new one. The
/// record is one file per session under the checkout's own ignored state directory: no
/// global state, nothing shared between users, and nothing left behind when the checkout
/// is deleted.
pub fn remember(entry: &Entry) -> Seen {
    let key = session_key();
    let path = entry.root.join(STATE_DIR).join(format!("{key}.txt"));
    let fingerprint = entry.fingerprint();
    let previous = std::fs::read_to_string(&path).ok();
    let seen = match previous.as_deref().map(str::trim) {
        None => Seen::First,
        Some(p) if p == fingerprint => Seen::Same,
        Some(_) => Seen::Changed,
    };
    if seen != Seen::Same {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = super::atomic_write(&path, fingerprint.as_bytes());
    }
    seen
}

/// What identifies this terminal session, as far as the banner is concerned. A session
/// the environment names is used as it is; otherwise the terminal device; otherwise one
/// shared key, which only means a banner is shown once rather than once per window.
fn session_key() -> String {
    let raw = std::env::var("MAJORDOMUS_ENTRY_SESSION")
        .or_else(|_| std::env::var("TERM_SESSION_ID"))
        .or_else(|_| std::env::var("WINDOWID"))
        .unwrap_or_else(|_| "shared".to_string());
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "shared".to_string()
    } else {
        cleaned
    }
}

const DIM: &str = "\u{1b}[2m";
const BOLD: &str = "\u{1b}[1m";
const RESET: &str = "\u{1b}[0m";

fn paint(text: &str, code: &str, colour: bool) -> String {
    if colour {
        format!("{code}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// One line and the entry commands.
fn compact(entry: &Entry, colour: bool) -> String {
    let mut line = format!("◆ {}", paint(&entry.name, BOLD, colour));
    if let Some(head) = &entry.head {
        line.push_str(&format!(" · {head}"));
    }
    line.push_str(&format!(
        " · {}/{} commands",
        entry.commands.0, entry.commands.1
    ));
    if let Some(server) = &entry.server {
        line.push_str(&format!(" · {server}"));
    }
    let mut out = line;
    if !entry.entry_commands.is_empty() {
        let names: Vec<&str> = entry
            .entry_commands
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        out.push('\n');
        out.push_str(&paint(&format!("  {}", names.join(" · ")), DIM, colour));
    }
    for note in &entry.notes {
        out.push('\n');
        out.push_str(&format!("  ! {note}"));
    }
    out.push('\n');
    out
}

/// The whole thing, inside a box that fits the terminal it is printed in. Width is
/// counted in characters a terminal advances the cursor for, not in bytes, so a name with
/// an accent in it does not tear the frame.
fn full(entry: &Entry, width: usize, colour: bool) -> String {
    let inner = width.clamp(28, 100).saturating_sub(4);
    let mut rows: Vec<(String, String)> = Vec::new();
    if let Some(head) = &entry.head {
        rows.push(("head".into(), head.clone()));
    }
    rows.push((
        "commands".into(),
        format!("{} available of {}", entry.commands.0, entry.commands.1),
    ));
    if let Some(server) = &entry.server {
        rows.push(("server".into(), server.clone()));
    }
    for note in &entry.notes {
        rows.push(("note".into(), note.clone()));
    }

    let mut out = String::new();
    let _ = writeln!(out, "╭{}╮", "─".repeat(inner + 2));
    let title = format!(
        "{}  {}",
        paint("MAJORDOMUS", BOLD, colour),
        paint(&entry.name, DIM, colour)
    );
    let _ = writeln!(out, "│ {} │", pad(&title, inner, colour));
    let _ = writeln!(out, "├{}┤", "─".repeat(inner + 2));
    for (key, value) in rows {
        let text = format!("{:<10}{}", key, value);
        let _ = writeln!(out, "│ {} │", pad(&text, inner, colour));
    }
    if !entry.entry_commands.is_empty() {
        let _ = writeln!(out, "├{}┤", "─".repeat(inner + 2));
        for (name, summary) in &entry.entry_commands {
            let room = inner.saturating_sub(name.chars().count() + 2);
            let text = format!("{name}  {}", truncate(summary, room));
            let _ = writeln!(out, "│ {} │", pad(&text, inner, colour));
        }
    }
    let _ = writeln!(out, "╰{}╯", "─".repeat(inner + 2));
    out
}

/// Pad to a width counted in printable characters: the escape sequences colour adds take
/// no columns and must not be counted, and the text is truncated by characters.
fn pad(text: &str, width: usize, colour: bool) -> String {
    let visible = if colour { strip_ansi(text) } else { text.to_string() };
    let len = visible.chars().count();
    if len > width {
        let cut = truncate(&visible, width);
        return cut;
    }
    format!("{text}{}", " ".repeat(width - len))
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    let mut out: String = text.chars().take(width - 1).collect();
    out.push('…');
    out
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// What the bootstrap hint says when the Rust executable is not there. The command comes
/// from the graph — the workflow that builds it declares itself an entry command — and is
/// never a string written here.
pub fn bootstrap_hint(graph: &CommandGraph) -> Option<String> {
    graph
        .commands
        .iter()
        .find(|c| c.program == Program::Workflow && c.tags.iter().any(|t| t == "bootstrap"))
        .and_then(|c| c.projections.just.clone())
        .map(|name| format!("just {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(root: &Path) -> Entry {
        Entry {
            root: root.to_path_buf(),
            name: "repo".into(),
            head: Some("master".into()),
            server: None,
            commands: (10, 12),
            structure: "abc".into(),
            entry_commands: vec![("just check".into(), "Quality gates".into())],
            notes: Vec::new(),
        }
    }

    #[test]
    fn a_word_reaching_eval_cannot_end_its_own_quoting() {
        for hostile in [
            "a'; rm -rf /; echo '",
            "$(touch /tmp/pwned)",
            "`id`",
            "line\nbreak",
            "semi;colon",
        ] {
            let quoted = sh_quote(hostile);
            assert!(quoted.starts_with('\'') && quoted.ends_with('\''));
            assert_eq!(
                quoted.matches('\'').count(),
                2,
                "{hostile:?} produced {quoted:?}"
            );
            assert!(!quoted.contains('\n'));
        }
    }

    #[test]
    fn the_activation_is_shell_statements_and_no_prose() {
        let dir = tempfile::tempdir().unwrap();
        let facts = Facts::none(dir.path());
        let text = activation(&entry(dir.path()), &facts, Shell::Direnv);
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            assert!(
                line.starts_with("export ")
                    || line.starts_with("PATH_add ")
                    || line.starts_with("watch_file ")
                    || line.starts_with("PATH="),
                "{line}"
            );
        }
        assert!(text.contains(&format!("export {ROOT_ENV}=")));
        assert!(text.contains(&format!("export {STRUCTURE_ENV}='abc'")));
    }

    #[test]
    fn the_watch_list_is_the_inputs_and_only_the_ones_that_exist() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("share")).unwrap();
        std::fs::write(dir.path().join("share/commands.yaml"), "version: 1\n").unwrap();
        let facts = Facts::none(dir.path());
        let watches = watches(&entry(dir.path()), &facts);
        assert_eq!(watches, vec!["share/commands.yaml".to_string()]);
    }

    #[test]
    fn the_banner_is_shown_once_and_then_not_again() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("MAJORDOMUS_ENTRY_SESSION", "test-session-one");
        let e = entry(dir.path());
        assert_eq!(remember(&e), Seen::First);
        assert_eq!(remember(&e), Seen::Same);
        let mut changed = e.clone();
        changed.head = Some("other".into());
        assert_eq!(remember(&changed), Seen::Changed);
        assert_eq!(remember(&changed), Seen::Same);
        std::env::remove_var("MAJORDOMUS_ENTRY_SESSION");
    }

    #[test]
    fn auto_is_full_then_silent_then_compact_when_something_moved() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("MAJORDOMUS_ENTRY_SESSION", "test-session-two");
        let e = entry(dir.path());
        assert!(banner(&e, BannerMode::Auto, 80, false)
            .is_some_and(|b| b.contains("MAJORDOMUS")));
        assert!(banner(&e, BannerMode::Auto, 80, false).is_none());
        let mut changed = e.clone();
        changed.commands = (11, 12);
        let second = banner(&changed, BannerMode::Auto, 80, false).expect("a change is shown");
        assert!(second.starts_with('◆'), "{second}");
        assert!(banner(&changed, BannerMode::Off, 80, false).is_none());
        std::env::remove_var("MAJORDOMUS_ENTRY_SESSION");
    }

    #[test]
    fn the_frame_holds_at_every_width_and_with_or_without_colour() {
        let dir = tempfile::tempdir().unwrap();
        let mut e = entry(dir.path());
        e.name = "název-s-diakritikou-a-🙂".into();
        for width in [20usize, 40, 80, 200] {
            for colour in [false, true] {
                let text = full(&e, width, colour);
                let widths: Vec<usize> = text
                    .lines()
                    .map(|l| strip_ansi(l).chars().count())
                    .collect();
                assert!(
                    widths.windows(2).all(|w| w[0] == w[1]),
                    "width {width} colour {colour}: {widths:?}"
                );
            }
        }
    }

    #[test]
    fn nothing_a_terminal_would_act_on_survives_into_the_banner() {
        let dir = tempfile::tempdir().unwrap();
        let mut e = entry(dir.path());
        e.notes = vec![super::super::completion::sanitise("a\u{1b}[2Jclear")
            .unwrap_or_default()];
        let text = full(&e, 80, false);
        assert!(!text.contains('\u{1b}'));
    }

    #[test]
    fn a_head_is_read_from_the_file_and_never_from_a_subprocess() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join(".git");
        std::fs::create_dir_all(&git).unwrap();
        std::fs::write(git.join("HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        assert_eq!(head(dir.path()).as_deref(), Some("feature/x"));
        std::fs::write(git.join("HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(head(dir.path()).as_deref(), Some("01234567"));
    }

    #[test]
    fn a_worktree_reads_the_head_of_its_own_directory() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(real.join("HEAD"), "ref: refs/heads/wt\n").unwrap();
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(
            work.join(".git"),
            format!("gitdir: {}\n", real.display()),
        )
        .unwrap();
        assert_eq!(head(&work).as_deref(), Some("wt"));
    }

    #[test]
    fn a_machine_is_never_shown_a_banner() {
        assert_eq!(BannerMode::from_environment(false), BannerMode::Off);
    }
}
