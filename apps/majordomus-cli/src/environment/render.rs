//! The banner: a snapshot rendered for a person at a terminal.
//!
//! This is a view and nothing else. It discovers nothing, reads no file, runs no command
//! and holds no fact of its own — every number, name, URL and command in it comes out of
//! the [`super::RepositoryEnvironment`] it was handed. That is what keeps the thing a
//! person sees on `cd` and the thing `GET /api/v1/environment` returns from ever
//! disagreeing.
//!
//! # What it refuses to do
//!
//! - Print anything it was not given. A count that was not resolved is shown as `—`, never
//!   as `0`.
//! - Print anything unsanitised. Every value goes through [`super::text::sanitise`]; a
//!   branch name is arbitrary text chosen by whoever made the branch.
//! - Measure in bytes. Every column is [`super::text::width`], so a box does not fall
//!   apart on a name that is not ASCII.
//! - Colour when it was not asked to. `NO_COLOR`, a pipe, and `CI` each turn it off.
//! - Fail. A render error would take the shell down with it, so there is no path here
//!   that can produce one.

use std::fmt::Write as _;

use crate::model::Severity;

use super::text::{pad_with, sanitise, truncate_with, width};
use super::{
    ProjectionState, RepositoryEnvironment, ServiceAvailability, TierState, ToolchainAvailability,
    VcsState,
};

/// How much of a snapshot to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerMode {
    /// Decide from the terminal and from whether this snapshot says anything new.
    #[default]
    Auto,
    /// The whole box.
    Full,
    /// Two lines.
    Compact,
    /// Nothing.
    Off,
}

impl BannerMode {
    /// Parse a mode by name; `None` for a word that is not one.
    ///
    /// ```
    /// use majordomus_cli::environment::render::BannerMode;
    /// assert_eq!(BannerMode::parse("compact"), Some(BannerMode::Compact));
    /// assert_eq!(BannerMode::parse("COMPACT"), Some(BannerMode::Compact));
    /// assert_eq!(BannerMode::parse("loud"), None);
    /// ```
    pub fn parse(word: &str) -> Option<Self> {
        match word.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(BannerMode::Auto),
            "full" => Some(BannerMode::Full),
            "compact" => Some(BannerMode::Compact),
            "off" | "none" | "silent" => Some(BannerMode::Off),
            _ => None,
        }
    }

    /// The word as written.
    pub fn as_str(self) -> &'static str {
        match self {
            BannerMode::Auto => "auto",
            BannerMode::Full => "full",
            BannerMode::Compact => "compact",
            BannerMode::Off => "off",
        }
    }
}

/// What the terminal on the other end can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presentation {
    /// Whether escape sequences may be used.
    pub colour: bool,
    /// Whether characters outside ASCII may be used.
    pub unicode: bool,
    /// How many columns there are.
    pub columns: usize,
    /// Whether the output is going to a terminal at all.
    pub interactive: bool,
}

/// The widest a banner is drawn, however wide the terminal is. A box that stretches across
/// a 200-column window is harder to read than one that does not.
pub const MAX_WIDTH: usize = 78;

/// The narrowest terminal the box is drawn in; below it, the plain two-line form is used
/// whatever the mode, because a box that does not fit is worse than no box.
pub const MIN_BOX_WIDTH: usize = 46;

impl Default for Presentation {
    fn default() -> Self {
        Presentation {
            colour: false,
            unicode: true,
            columns: MAX_WIDTH,
            interactive: false,
        }
    }
}

impl Presentation {
    /// What this process's standard error can take.
    ///
    /// The banner goes to standard error, not standard output: standard output belongs to
    /// whatever the caller is capturing, and a banner in the middle of `majordomus env
    /// status --json | jq` would be a bug in every consumer at once.
    ///
    /// `NO_COLOR` — set to anything at all, including the empty string, per the
    /// convention — turns colour off. So does `CI`, and so does a standard error that is
    /// not a terminal.
    pub fn detect() -> Self {
        let interactive = is_terminal(libc::STDERR_FILENO);
        let ci = std::env::var_os("CI").is_some();
        Presentation {
            colour: interactive && !ci && std::env::var_os("NO_COLOR").is_none(),
            unicode: unicode_locale(),
            columns: terminal_columns().unwrap_or(MAX_WIDTH),
            interactive: interactive && !ci,
        }
    }
}

/// Is this file descriptor a terminal?
fn is_terminal(fd: i32) -> bool {
    // SAFETY: isatty takes a file descriptor and reads no memory; any integer is a
    // defined argument, and an invalid one returns 0.
    unsafe { libc::isatty(fd) == 1 }
}

/// The terminal's width, from `COLUMNS` when a shell exported it, else from the terminal
/// itself. `None` when neither answers, which is every non-interactive case.
fn terminal_columns() -> Option<usize> {
    if let Some(columns) = std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.trim().parse::<usize>().ok())
    {
        if columns > 0 {
            return Some(columns);
        }
    }
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: TIOCGWINSZ writes a winsize into the pointer given; the struct is owned
    // here, correctly sized, and the call is a no-op returning -1 on a non-terminal.
    let ok = unsafe { libc::ioctl(libc::STDERR_FILENO, libc::TIOCGWINSZ, &mut size) } == 0;
    (ok && size.ws_col > 0).then_some(size.ws_col as usize)
}

/// Does the locale say this terminal can render characters outside ASCII?
fn unicode_locale() -> bool {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .map(|v| {
            v.to_ascii_uppercase().contains("UTF-8") || v.to_ascii_uppercase().contains("UTF8")
        })
        // No locale set at all: a terminal that says nothing is assumed to be modern.
        // Being wrong here costs a few mojibake characters, and being wrong the other way
        // costs every non-ASCII repository a legible banner.
        .unwrap_or(true)
}

/// The characters the box is drawn with.
struct Glyphs {
    top_left: &'static str,
    top_right: &'static str,
    bottom_left: &'static str,
    bottom_right: &'static str,
    horizontal: &'static str,
    vertical: &'static str,
    tee_left: &'static str,
    tee_right: &'static str,
    dirty: &'static str,
    ahead: &'static str,
    behind: &'static str,
    yes: &'static str,
    no: &'static str,
    unknown: &'static str,
    bullet: &'static str,
    mark: &'static str,
    /// What a truncated value ends with.
    ellipsis: &'static str,
    /// What stands in for a value nothing resolved. Never a zero.
    unresolved: &'static str,
}

const UNICODE: Glyphs = Glyphs {
    top_left: "╭",
    top_right: "╮",
    bottom_left: "╰",
    bottom_right: "╯",
    horizontal: "─",
    vertical: "│",
    tee_left: "├",
    tee_right: "┤",
    dirty: "✱",
    ahead: "↑",
    behind: "↓",
    yes: "✓",
    no: "✗",
    unknown: "○",
    bullet: "·",
    mark: "◆",
    ellipsis: "…",
    unresolved: "—",
};

const ASCII: Glyphs = Glyphs {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    horizontal: "-",
    vertical: "|",
    tee_left: "+",
    tee_right: "+",
    dirty: "*",
    ahead: "^",
    behind: "v",
    yes: "ok",
    no: "!",
    unknown: "?",
    bullet: "-",
    mark: ">",
    ellipsis: "...",
    unresolved: "n/a",
};

/// Render a banner, or nothing when the mode says nothing.
///
/// `seen_before` is the digest of the last snapshot shown to this person, which decides
/// what `auto` does: a repository that has not changed since they last looked gets the
/// two-line form, and a first look or a changed repository gets the box.
pub fn banner(
    environment: &RepositoryEnvironment,
    mode: BannerMode,
    presentation: &Presentation,
    seen_before: Option<&str>,
) -> Option<String> {
    let resolved = match mode {
        BannerMode::Off => return None,
        BannerMode::Auto => {
            if !presentation.interactive {
                return None;
            }
            match seen_before {
                Some(digest) if digest == environment.digest() => BannerMode::Compact,
                _ => BannerMode::Full,
            }
        }
        other => other,
    };
    Some(match resolved {
        BannerMode::Compact => compact(environment, presentation),
        _ if presentation.columns < MIN_BOX_WIDTH => compact(environment, presentation),
        _ => full(environment, presentation),
    })
}

/// How wide the label column of an ordinary row is.
const LABEL_WIDTH: usize = 11;

/// How wide the label column of a command row is: a command is the label there, and
/// `just cockpit-assets-check` truncated to eleven columns is not a command.
const COMMAND_WIDTH: usize = 26;

/// One line of the box.
enum Row {
    /// A label and a value.
    Field(String, String),
    /// A command and what it does; the command gets the wider column.
    Command(String, String),
    /// Nothing, for spacing.
    Blank,
    /// A horizontal divider.
    Divider,
}

/// The full banner.
fn full(environment: &RepositoryEnvironment, presentation: &Presentation) -> String {
    let g = glyphs(presentation);
    let inner = presentation.columns.clamp(MIN_BOX_WIDTH, MAX_WIDTH) - 2;
    let mut rows: Vec<Row> = Vec::new();

    // ---- what this is
    let name = sanitise(&environment.project.name).to_uppercase();
    let version = format!("v{}", sanitise(&environment.project.version));
    let title_gap = inner.saturating_sub(2 + width(&name) + width(&version) + 2);
    rows.push(Row::Field(
        format!("{name}{}{version}", " ".repeat(title_gap.max(1))),
        String::new(),
    ));
    rows.push(Row::Field(
        truncate_with(
            &sanitise(&environment.project.summary),
            inner - 4,
            g.ellipsis,
        ),
        String::new(),
    ));
    rows.push(Row::Divider);

    // ---- the checkout
    let branch = match &environment.vcs {
        VcsState::Git(tree) => tree
            .branch
            .clone()
            .map(|b| sanitise(&b))
            .unwrap_or_else(|| match &tree.head {
                Some(head) => format!("detached at {}", short(head)),
                None => "unborn".into(),
            }),
        VcsState::Unavailable { .. } => g.unresolved.into(),
    };
    let repository_name = sanitise(&environment.repository.name);
    rows.push(Row::Field(
        "repo".into(),
        two_columns(
            &repository_name,
            "branch",
            &branch,
            inner - LABEL_WIDTH - 2,
            g,
        ),
    ));
    rows.push(Row::Field("git".into(), git_line(environment, g)));
    let runtime = toolchain_line(environment, g);
    if !runtime.is_empty() {
        rows.push(Row::Field("runtime".into(), runtime));
    }

    // ---- what the layer holds
    rows.push(Row::Blank);
    rows.push(Row::Field(
        "layer".into(),
        layer_line(environment, g, inner - LABEL_WIDTH - 2),
    ));
    let providers = provider_line(environment, g);
    if !providers.is_empty() {
        rows.push(Row::Field("agents".into(), providers));
    }

    // ---- where things are
    let running: Vec<&super::ServiceState> = environment
        .services
        .iter()
        .filter(|s| s.url.is_some() && s.id != "index" && s.id != "openapi")
        .collect();
    if !running.is_empty() {
        rows.push(Row::Blank);
        for service in running {
            let url = service.url.as_deref().unwrap_or_default();
            let mark = match service.availability {
                ServiceAvailability::Available => g.yes,
                ServiceAvailability::NotRunning => g.no,
                ServiceAvailability::Unknown => g.unknown,
            };
            rows.push(Row::Field(
                sanitise(&service.id),
                format!("{} {mark}", sanitise(url)),
            ));
        }
    }

    // ---- what to run
    let entrypoints = &environment.workflows.entrypoints;
    if !entrypoints.is_empty() {
        rows.push(Row::Divider);
        for entry in entrypoints.iter().take(4) {
            rows.push(Row::Command(
                sanitise(&entry.command),
                truncate_with(
                    &sanitise(entry.description.as_deref().unwrap_or_default()),
                    inner.saturating_sub(COMMAND_WIDTH + 2),
                    g.ellipsis,
                ),
            ));
        }
    }

    // ---- anything wrong
    if let Some(worst) = environment.headline_diagnostic() {
        rows.push(Row::Divider);
        rows.push(Row::Field(
            match worst.severity {
                Severity::Error => g.no.into(),
                _ => g.unknown.into(),
            },
            truncate_with(
                &sanitise(&worst.message),
                inner.saturating_sub(6),
                g.ellipsis,
            ),
        ));
    }

    draw(&rows, inner, g, presentation)
}

/// A label column and a value column, both padded, for a row that carries two pairs.
fn two_columns(first: &str, label: &str, second: &str, budget: usize, g: &Glyphs) -> String {
    let left = budget.saturating_sub(width(label) + 2).min(budget) / 2 + 4;
    format!(
        "{}{label}  {}",
        pad_with(first, left.min(budget), g.ellipsis),
        truncate_with(
            second,
            budget.saturating_sub(left + width(label) + 2),
            g.ellipsis
        )
    )
}

/// `✱ 3 modified · 2 staged · ↑2 ↓1`, or `clean`.
fn git_line(environment: &RepositoryEnvironment, g: &Glyphs) -> String {
    let Some(tree) = environment.vcs.tree() else {
        return match &environment.vcs {
            VcsState::Unavailable { reason } => truncate_with(&sanitise(reason), 50, g.ellipsis),
            VcsState::Git(_) => unreachable!("the tree was just taken"),
        };
    };
    let mut parts: Vec<String> = Vec::new();
    if tree.clean {
        parts.push("clean".into());
    } else {
        let mut dirt: Vec<String> = Vec::new();
        if tree.staged > 0 {
            dirt.push(format!("{} staged", tree.staged));
        }
        if tree.modified > 0 {
            dirt.push(format!("{} modified", tree.modified));
        }
        if tree.untracked > 0 {
            dirt.push(format!("{} untracked", tree.untracked));
        }
        if tree.conflicted > 0 {
            dirt.push(format!("{} conflicted", tree.conflicted));
        }
        parts.push(format!("{} {}", g.dirty, dirt.join(", ")));
    }
    match (tree.ahead, tree.behind) {
        (Some(0), Some(0)) => parts.push("in step".into()),
        (Some(a), Some(b)) => parts.push(format!("{}{a} {}{b}", g.ahead, g.behind)),
        // No upstream is not "in step with nothing"; it is a branch nobody else has.
        _ => parts.push("no upstream".into()),
    }
    parts.join(&format!(" {} ", g.bullet))
}

/// `Rust 1.90.0 · Node 22.20.0`, showing only what the repository declares.
fn toolchain_line(environment: &RepositoryEnvironment, g: &Glyphs) -> String {
    environment
        .toolchains
        .iter()
        .map(|t| {
            let version = match (&t.installed, t.availability) {
                (Some(v), _) => sanitise(v),
                (None, ToolchainAvailability::Missing) => format!("{} missing", g.no),
                (None, _) => t
                    .declared
                    .as_deref()
                    .map(|d| format!("{} declared", sanitise(d)))
                    .unwrap_or_else(|| g.unresolved.into()),
            };
            format!("{} {version}", sanitise(&t.title))
        })
        .collect::<Vec<_>>()
        .join(&format!(" {} ", g.bullet))
}

/// `rule 87 · use-case 38 · adr 19`, largest first, or a note that nothing counted them.
fn layer_line(environment: &RepositoryEnvironment, g: &Glyphs, budget: usize) -> String {
    if environment.layer.state == TierState::Unavailable {
        return format!("{}  (run `majordomus env status` to count)", g.unresolved);
    }
    let mut kinds = environment.layer.kinds.clone();
    // Largest first: what a repository is mostly made of is what a person wants to see
    // when the line has to be cut. Ties by name, so the order is total and stable.
    kinds.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.kind.cmp(&b.kind)));
    let mut line = String::new();
    for kind in &kinds {
        let part = format!("{} {}", sanitise(&kind.kind), kind.count);
        let separator = if line.is_empty() {
            String::new()
        } else {
            format!(" {} ", g.bullet)
        };
        if width(&line) + width(&separator) + width(&part) > budget {
            break;
        }
        line.push_str(&separator);
        line.push_str(&part);
    }
    if line.is_empty() {
        return g.unresolved.into();
    }
    line
}

/// `AGENTS.md ✓ · CLAUDE.md ✗`.
fn provider_line(environment: &RepositoryEnvironment, g: &Glyphs) -> String {
    environment
        .providers
        .iter()
        .map(|p| {
            let mark = match p.state {
                ProjectionState::Current => g.yes,
                ProjectionState::Stale | ProjectionState::Absent => g.no,
                ProjectionState::Unknown => g.unknown,
            };
            format!("{} {mark}", sanitise(&p.target))
        })
        .collect::<Vec<_>>()
        .join(&format!(" {} ", g.bullet))
}

/// Draw the rows inside a box.
fn draw(rows: &[Row], inner: usize, g: &Glyphs, presentation: &Presentation) -> String {
    let dim = |s: &str| -> String {
        if presentation.colour {
            format!("\u{1b}[2m{s}\u{1b}[0m")
        } else {
            s.to_string()
        }
    };
    let bold = |s: &str| -> String {
        if presentation.colour {
            format!("\u{1b}[1m{s}\u{1b}[0m")
        } else {
            s.to_string()
        }
    };
    let mut out = String::new();
    let bar = g.horizontal.repeat(inner);
    let _ = writeln!(out, "{}{bar}{}", g.top_left, g.top_right);
    for (i, row) in rows.iter().enumerate() {
        match row {
            Row::Divider => {
                let _ = writeln!(out, "{}{bar}{}", g.tee_left, g.tee_right);
            }
            Row::Blank => {
                let _ = writeln!(out, "{}{}{}", g.vertical, " ".repeat(inner), g.vertical);
            }
            Row::Field(label, value) if value.is_empty() => {
                // A heading row: the label is the whole line.
                let text = pad_with(&format!("  {label}"), inner, g.ellipsis);
                let painted = if i == 0 { bold(&text) } else { dim(&text) };
                let _ = writeln!(out, "{}{painted}{}", g.vertical, g.vertical);
            }
            Row::Field(label, value) | Row::Command(label, value) => {
                let column = match row {
                    Row::Command(_, _) => COMMAND_WIDTH,
                    _ => LABEL_WIDTH,
                };
                let padded_label = pad_with(&format!("  {label}"), column, g.ellipsis);
                let padded_value = pad_with(value, inner.saturating_sub(column), g.ellipsis);
                let _ = writeln!(
                    out,
                    "{}{}{padded_value}{}",
                    g.vertical,
                    dim(&padded_label),
                    g.vertical
                );
            }
        }
    }
    let _ = writeln!(out, "{}{bar}{}", g.bottom_left, g.bottom_right);
    out
}

/// The two-line form.
fn compact(environment: &RepositoryEnvironment, presentation: &Presentation) -> String {
    let g = glyphs(presentation);
    let budget = presentation.columns.clamp(20, MAX_WIDTH + 20);
    let mut parts: Vec<String> = vec![sanitise(&environment.repository.name)];
    if let Some(tree) = environment.vcs.tree() {
        let mut state = tree
            .branch
            .clone()
            .map(|b| sanitise(&b))
            .unwrap_or_else(|| "detached".into());
        if !tree.clean {
            let _ = write!(state, " {}{}", g.dirty, tree.dirty_files() + tree.untracked);
        }
        if let (Some(a), Some(b)) = (tree.ahead, tree.behind) {
            if a > 0 {
                let _ = write!(state, " {}{a}", g.ahead);
            }
            if b > 0 {
                let _ = write!(state, " {}{b}", g.behind);
            }
        }
        parts.push(state);
    }
    if let Some(rust) = environment
        .toolchains
        .iter()
        .find_map(|t| t.installed.as_ref().map(|v| format!("{} {}", t.title, v)))
    {
        parts.push(sanitise(&rust));
    }
    if let Some(objects) = environment.layer.objects {
        parts.push(format!("{objects} objects"));
    }
    if let Some(cockpit) = environment.service("cockpit") {
        if cockpit.availability == ServiceAvailability::Available {
            parts.push(format!("cockpit {}", g.yes));
        }
    }
    let separator = format!(" {} ", g.bullet);
    let head = format!("{} {}", g.mark, parts.join(&separator));

    let commands: Vec<String> = environment
        .workflows
        .entrypoints
        .iter()
        .take(3)
        .map(|e| sanitise(&e.command))
        .collect();
    let mut out = truncate_with(&head, budget, g.ellipsis);
    if !commands.is_empty() {
        let _ = write!(
            out,
            "\n{}",
            truncate_with(
                &format!("  {}", commands.join(&separator)),
                budget,
                g.ellipsis
            )
        );
    }
    if let Some(worst) = environment.headline_diagnostic() {
        let _ = write!(
            out,
            "\n{}",
            truncate_with(
                &format!("  {} {}", g.unknown, sanitise(&worst.message)),
                budget,
                g.ellipsis
            )
        );
    }
    out.push('\n');
    out
}

fn glyphs(presentation: &Presentation) -> &'static Glyphs {
    if presentation.unicode {
        &UNICODE
    } else {
        &ASCII
    }
}

fn short(commit: &str) -> String {
    commit.chars().take(12).collect()
}

/// A snapshot with something in every section, for the tests of this module and of the
/// ones that render or export the same value. Written once: three modules asserting
/// against three hand-built snapshots would be asserting against three different products.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;
    use crate::environment::{
        GitWorkingTree, KindCount, LayerSummary, ProjectIdentity, RepositoryIdentity, Resolution,
        ServiceState, ToolchainState, WorkflowCatalogue, WorkflowEntrypoint,
    };

    /// A fully populated snapshot of a repository that does not exist.
    pub(crate) fn environment() -> RepositoryEnvironment {
        RepositoryEnvironment {
            schema: RepositoryEnvironment::schema_id(),
            generated_at: "2026-09-06T12:00:00Z".into(),
            resolution: Resolution::Full,
            project: ProjectIdentity::of_this_build(),
            repository: RepositoryIdentity {
                name: "prismatic-majordomus".into(),
                root: "/somewhere/prismatic-majordomus".into(),
                layer_schema: "ai-repository/v1".into(),
                sections: Default::default(),
                local_path: ".ai/local".into(),
                linked_worktree: false,
            },
            vcs: VcsState::Git(GitWorkingTree {
                head: Some("c027ef6eac5d953ca539547bfcecbef727c3d369".into()),
                branch: Some("master".into()),
                detached: false,
                upstream: Some("origin/master".into()),
                ahead: Some(2),
                behind: Some(0),
                staged: 1,
                modified: 3,
                untracked: 1,
                conflicted: 0,
                clean: false,
                changed_paths: vec![],
            }),
            toolchains: vec![ToolchainState {
                id: "rust".into(),
                title: "Rust".into(),
                declared: Some("1.85".into()),
                declared_by: "apps/majordomus-cli/Cargo.toml".into(),
                installed: Some("1.90.0".into()),
                availability: ToolchainAvailability::Installed,
            }],
            layer: LayerSummary {
                state: TierState::Resolved,
                kinds: vec![
                    KindCount {
                        kind: "rule".into(),
                        count: 87,
                    },
                    KindCount {
                        kind: "adr".into(),
                        count: 19,
                    },
                ],
                objects: Some(902),
                capabilities: Some(934),
                invalid: Some(0),
                degraded: Some(false),
            },
            workflows: WorkflowCatalogue {
                state: TierState::Resolved,
                source: Some("just".into()),
                workflows: vec![],
                entrypoints: vec![WorkflowEntrypoint {
                    group: "check".into(),
                    workflow: "check".into(),
                    command: "just check".into(),
                    description: Some("Every gate.".into()),
                }],
            },
            providers: vec![],
            services: vec![
                ServiceState {
                    id: "cockpit".into(),
                    title: "Cockpit".into(),
                    path: "/cockpit".into(),
                    url: Some("http://127.0.0.1:8741/cockpit".into()),
                    availability: ServiceAvailability::Available,
                },
                ServiceState {
                    id: "index".into(),
                    title: "Home page".into(),
                    path: "/".into(),
                    url: Some("http://127.0.0.1:8741/".into()),
                    availability: ServiceAvailability::Available,
                },
            ],
            diagnostics: vec![],
            provenance: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::environment;
    use super::*;
    use crate::environment::LayerSummary;

    fn plain(columns: usize) -> Presentation {
        Presentation {
            colour: false,
            unicode: true,
            columns,
            interactive: true,
        }
    }

    /// The box has to be a box: every line exactly as wide as every other, whatever is in
    /// it. Measured in columns, which is the whole reason `text::width` exists.
    #[test]
    fn every_line_of_the_box_is_the_same_width() {
        for columns in [MIN_BOX_WIDTH, 60, 78, 200] {
            let drawn =
                banner(&environment(), BannerMode::Full, &plain(columns), None).expect("a banner");
            let widths: Vec<usize> = drawn.lines().map(width).collect();
            let first = widths[0];
            assert!(
                widths.iter().all(|w| *w == first),
                "at {columns} columns the lines are {widths:?}"
            );
            assert!(
                first <= columns.max(MIN_BOX_WIDTH),
                "wider than the terminal"
            );
        }
    }

    #[test]
    fn a_narrow_terminal_gets_the_plain_form_rather_than_a_broken_box() {
        let drawn = banner(&environment(), BannerMode::Full, &plain(30), None).expect("a banner");
        assert!(!drawn.contains('╭'), "a box was drawn into 30 columns");
        for line in drawn.lines() {
            assert!(width(line) <= 30, "{line:?} overflows 30 columns");
        }
    }

    /// The rule the whole model exists to keep: a count that was not counted is never
    /// rendered as a zero.
    #[test]
    fn an_unresolved_count_is_shown_as_unknown_and_never_as_zero() {
        let mut env = environment();
        env.layer = LayerSummary::unavailable();
        let drawn = banner(&env, BannerMode::Full, &plain(78), None).expect("a banner");
        assert!(drawn.contains("—"));
        assert!(
            !drawn.contains("rule 0") && !drawn.contains("0 objects"),
            "{drawn}"
        );
    }

    #[test]
    fn no_colour_means_no_escape_sequence_anywhere() {
        let drawn = banner(&environment(), BannerMode::Full, &plain(78), None).expect("a banner");
        assert!(!drawn.contains('\u{1b}'));
        let coloured = banner(
            &environment(),
            BannerMode::Full,
            &Presentation {
                colour: true,
                ..plain(78)
            },
            None,
        )
        .expect("a banner");
        assert!(coloured.contains('\u{1b}'), "colour was asked for");
    }

    #[test]
    fn an_ascii_terminal_gets_no_character_outside_ascii() {
        let drawn = banner(
            &environment(),
            BannerMode::Full,
            &Presentation {
                unicode: false,
                ..plain(78)
            },
            None,
        )
        .expect("a banner");
        assert!(drawn.is_ascii(), "{drawn}");
    }

    /// The banner prints repository metadata, and repository metadata is not trusted.
    #[test]
    fn a_hostile_branch_name_cannot_reach_the_terminal_through_the_banner() {
        let mut env = environment();
        if let VcsState::Git(tree) = &mut env.vcs {
            tree.branch = Some("\u{1b}]0;pwned\u{7}evil".into());
        }
        env.repository.name = "\u{1b}[2Jwiped".into();
        let drawn = banner(&env, BannerMode::Full, &plain(78), None).expect("a banner");
        assert!(!drawn.contains('\u{1b}'), "an escape reached the output");
        assert!(drawn.contains("evil"), "the readable part is still shown");
    }

    #[test]
    fn off_renders_nothing_at_all() {
        assert_eq!(
            banner(&environment(), BannerMode::Off, &plain(78), None),
            None
        );
    }

    #[test]
    fn auto_is_silent_when_nothing_is_watching() {
        let piped = Presentation {
            interactive: false,
            ..plain(78)
        };
        assert_eq!(banner(&environment(), BannerMode::Auto, &piped, None), None);
    }

    #[test]
    fn auto_shows_the_box_once_and_the_short_form_on_a_return() {
        let env = environment();
        let first = banner(&env, BannerMode::Auto, &plain(78), None).expect("a banner");
        assert!(first.lines().count() > 5, "a first look gets the box");
        let again =
            banner(&env, BannerMode::Auto, &plain(78), Some(&env.digest())).expect("a banner");
        assert!(again.lines().count() <= 3, "a return gets the short form");
        let changed = banner(
            &env,
            BannerMode::Auto,
            &plain(78),
            Some("a different digest"),
        )
        .expect("a banner");
        assert!(
            changed.lines().count() > 5,
            "a changed repository is news again"
        );
    }

    #[test]
    fn the_compact_form_fits_the_terminal_it_was_given() {
        for columns in [20, 40, 80, 200] {
            let drawn = banner(&environment(), BannerMode::Compact, &plain(columns), None)
                .expect("a banner");
            for line in drawn.lines() {
                assert!(width(line) <= columns, "{line:?} overflows {columns}");
            }
        }
    }

    #[test]
    fn the_commands_shown_are_the_ones_the_snapshot_carries() {
        let mut env = environment();
        let drawn = banner(&env, BannerMode::Full, &plain(78), None).expect("a banner");
        assert!(drawn.contains("just check"));
        env.workflows.entrypoints.clear();
        let bare = banner(&env, BannerMode::Full, &plain(78), None).expect("a banner");
        assert!(
            !bare.contains("just "),
            "a snapshot with no workflows must not suggest one"
        );
    }

    #[test]
    fn a_repository_without_git_still_renders() {
        let mut env = environment();
        env.vcs = VcsState::Unavailable {
            reason: "not a work tree".into(),
        };
        let drawn = banner(&env, BannerMode::Full, &plain(78), None).expect("a banner");
        assert!(drawn.contains("not a work tree"));
        let short = banner(&env, BannerMode::Compact, &plain(78), None).expect("a banner");
        assert!(!short.is_empty());
    }
}
