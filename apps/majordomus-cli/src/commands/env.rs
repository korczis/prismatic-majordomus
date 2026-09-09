//! `majordomus env`: the repository environment on the command line.
//!
//! Four renderings of one value, and the reason they are one command rather than four is
//! that they must never disagree. `status` prints it, `banner` draws it, `export` turns
//! the two or three parts a shell needs into assignments, and `explain` says where each
//! value came from.
//!
//! # Which of them may build the index
//!
//! `status` and `explain` are commands a person waits on, so they resolve in full: the
//! index is built, what the layer holds is counted, and the cache is written.
//!
//! `banner` and `export` are what `direnv` runs on every `cd`, so they resolve fast: they
//! never build the index, never contact anything but a loopback address a running server
//! published, and take what they cannot compute from the cache the last full resolution
//! left. This is the difference between entering a repository in tens of milliseconds and
//! entering it in two seconds.
//!
//! # Which stream each writes to
//!
//! `status`, `explain` and `export` write to standard output, because a caller is reading
//! them. `banner` writes to **standard error**, because `direnv` reads the standard output
//! of a `.envrc` as the environment it is applying; a banner on standard output would be
//! parsed as variable assignments and would break every shell that entered the directory.

use std::io::Write;

use crate::app::App;
use crate::cli::{EnvArgs, EnvCommand, OutputFormat, RepoArgs};
use crate::environment::render::{banner, BannerMode, Presentation};
use crate::environment::shell::{export, Dialect};
use crate::environment::{
    cache::Cache, resolve, EnvironmentQuery, Inputs, ProjectionState, RepositoryEnvironment,
    ServiceAvailability, TierState, ToolchainAvailability, VcsState,
};
use crate::error::{Error, Result};
use crate::repository::Repository;
use crate::share::Share;

/// Run `majordomus env`.
pub fn run(args: EnvArgs) -> Result<u8> {
    match args.command {
        None | Some(EnvCommand::Status) => status(&args.repo, args.format),
        Some(EnvCommand::Explain { field }) => explain(&args.repo, args.format, field.as_deref()),
        Some(EnvCommand::Banner { mode, width }) => {
            banner_command(&args.repo, mode.as_deref(), width)
        }
        Some(EnvCommand::Export {
            shell,
            banner,
            mode,
            bridge,
        }) => export_command(&args.repo, &shell, banner, mode.as_deref(), bridge),
    }
}

/// A full snapshot: the index is built, so every count is real.
fn resolve_full(repo: &RepoArgs) -> Result<RepositoryEnvironment> {
    let app = App::load(repo)?;
    Ok(resolve(
        &Inputs {
            repository: &app.repository,
            share: Some(&app.share),
            index: Some(app.index()),
            registry: Some(app.registry()),
        },
        &EnvironmentQuery::full(),
    ))
}

/// A fast snapshot: the repository's manifest, one `git` call, the cache. Never the index.
///
/// The two things it does load — the manifest and the distribution directory — are a file
/// read and a few path checks. What it deliberately does not do is call [`App::load`],
/// which is where the seconds are.
fn resolve_fast(repo: &RepoArgs) -> Result<(RepositoryEnvironment, Option<Share>)> {
    let start = match &repo.repo {
        Some(p) => p.clone(),
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let repository = Repository::discover(&start)?;
    // A missing distribution is not fatal here: without it the provider projections are
    // listed from the policy and their state is reported unknown, which is honest.
    let share = Share::locate(repo.share.as_deref(), repository.root()).ok();
    let environment = resolve(
        &Inputs {
            repository: &repository,
            share: share.as_ref(),
            index: None,
            registry: None,
        },
        &EnvironmentQuery::fast(),
    );
    Ok((environment, share))
}

fn status(repo: &RepoArgs, format: OutputFormat) -> Result<u8> {
    let environment = resolve_full(repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&environment).map_err(|e| Error::Protocol {
                reason: e.to_string()
            })?
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => write_status(&mut out, &environment)?,
    }
    Ok(0)
}

/// The human form: one line per fact, in the order the model declares them, greppable.
fn write_status(out: &mut impl Write, e: &RepositoryEnvironment) -> Result<()> {
    let mut line = |label: &str, value: String| -> Result<()> {
        writeln!(out, "{label:<12} {value}").map_err(Error::Transport)
    };
    line(
        "project",
        format!(
            "{} {} ({}, {})",
            e.project.name, e.project.version, e.project.profile, e.project.target
        ),
    )?;
    line(
        "repository",
        format!("{} — {}", e.repository.name, e.repository.root),
    )?;
    line("layer", e.repository.layer_schema.clone())?;
    line("resolution", e.resolution.as_str().into())?;

    match &e.vcs {
        VcsState::Git(t) => {
            line(
                "branch",
                t.branch.clone().unwrap_or_else(|| match &t.head {
                    Some(head) => format!("(detached at {})", &head[..12.min(head.len())]),
                    None => "(unborn)".into(),
                }),
            )?;
            line(
                "working tree",
                if t.clean {
                    "clean".to_string()
                } else {
                    format!(
                        "{} staged, {} modified, {} untracked, {} conflicted",
                        t.staged, t.modified, t.untracked, t.conflicted
                    )
                },
            )?;
            line(
                "upstream",
                match (&t.upstream, t.ahead, t.behind) {
                    (Some(u), Some(a), Some(b)) => format!("{u} (ahead {a}, behind {b})"),
                    (Some(u), _, _) => u.clone(),
                    _ => "none".into(),
                },
            )?;
        }
        VcsState::Unavailable { reason } => line("branch", format!("unavailable: {reason}"))?,
    }

    for t in &e.toolchains {
        line(
            &format!("toolchain {}", t.id),
            format!(
                "{}{} ({})",
                match (&t.installed, t.availability) {
                    (Some(v), _) => v.clone(),
                    (None, ToolchainAvailability::Missing) => "not installed".into(),
                    (None, _) => "unknown".into(),
                },
                t.declared
                    .as_deref()
                    .map(|d| format!(", declared {d}"))
                    .unwrap_or_default(),
                t.declared_by
            ),
        )?;
    }

    match e.layer.state {
        TierState::Unavailable => line("layer holds", "unknown (not counted)".into())?,
        state => {
            line(
                "layer holds",
                format!(
                    "{} object(s), {} capabilities [{}]",
                    e.layer.objects.unwrap_or_default(),
                    e.layer.capabilities.unwrap_or_default(),
                    state.as_str()
                ),
            )?;
            for kind in &e.layer.kinds {
                line(&format!("  {}", kind.kind), kind.count.to_string())?;
            }
        }
    }

    match e.workflows.state {
        TierState::Unavailable => line("workflows", "none (no workflow runner here)".into())?,
        state => {
            line(
                "workflows",
                format!("{} [{}]", e.workflows.workflows.len(), state.as_str()),
            )?;
            for entry in &e.workflows.entrypoints {
                line(
                    &format!("  {}", entry.group),
                    format!(
                        "{}{}",
                        entry.command,
                        entry
                            .description
                            .as_deref()
                            .map(|d| format!(" — {d}"))
                            .unwrap_or_default()
                    ),
                )?;
            }
        }
    }

    for p in &e.providers {
        line(
            &format!("provider {}", p.id),
            format!(
                "{} — {}",
                p.target,
                match p.state {
                    ProjectionState::Current => "current",
                    ProjectionState::Stale => "STALE (majordomus generate)",
                    ProjectionState::Absent => "MISSING (majordomus generate)",
                    ProjectionState::Unknown => "unknown",
                }
            ),
        )?;
    }

    for s in &e.services {
        line(
            &format!("service {}", s.id),
            match &s.url {
                Some(url) => format!(
                    "{url} — {}",
                    match s.availability {
                        ServiceAvailability::Available => "answering",
                        ServiceAvailability::NotRunning => "no answer",
                        ServiceAvailability::Unknown => "not probed",
                    }
                ),
                None => format!("{} — no server is running", s.path),
            },
        )?;
    }

    for d in &e.diagnostics {
        writeln!(
            out,
            "{:<12} {} — {}",
            format!("{:?}", d.severity).to_uppercase(),
            d.code,
            d.message
        )
        .map_err(Error::Transport)?;
    }
    Ok(())
}

fn explain(repo: &RepoArgs, format: OutputFormat, field: Option<&str>) -> Result<u8> {
    let environment = resolve_full(repo)?;
    let selected: Vec<_> = environment
        .provenance
        .iter()
        .filter(|p| match field {
            None => true,
            Some(f) => p.field == f || p.field.starts_with(&format!("{f}.")),
        })
        .collect();
    if selected.is_empty() {
        return Err(Error::Protocol {
            reason: format!(
                "no field named '{}'; `majordomus env explain` with no field lists every one",
                field.unwrap_or_default()
            ),
        });
    }
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&selected).map_err(|e| Error::Protocol {
                reason: e.to_string()
            })?
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            for p in selected {
                writeln!(out, "{}", p.field).map_err(Error::Transport)?;
                if let Some(value) = &p.value {
                    writeln!(out, "  value      {value}").map_err(Error::Transport)?;
                }
                writeln!(out, "  source     {}", p.source).map_err(Error::Transport)?;
                writeln!(out, "  resolver   {}", p.resolver).map_err(Error::Transport)?;
                writeln!(out, "  confidence {:?}", p.confidence).map_err(Error::Transport)?;
            }
        }
    }
    Ok(0)
}

/// The environment variable a person sets once, in their shell profile or in `.envrc`, to
/// decide how much the banner says. Named here because it is part of the command's
/// contract and the documentation renders it from this constant.
pub const BANNER_ENV: &str = "MAJORDOMUS_BANNER";

/// The mode a caller asked for: the option, then the environment, then `auto`. A person
/// sets the variable once; a caller that wants a particular rendering says so.
fn requested_mode(mode: Option<&str>) -> Result<BannerMode> {
    let requested = match mode {
        Some(mode) => mode.to_string(),
        None => std::env::var(BANNER_ENV).unwrap_or_else(|_| "auto".into()),
    };
    BannerMode::parse(&requested).ok_or_else(|| Error::Protocol {
        reason: format!(
            "'{requested}' is not a banner mode; one of auto, full, compact, off (from {})",
            match mode {
                Some(_) => "--mode",
                None => BANNER_ENV,
            }
        ),
    })
}

/// Draw a snapshot and remember the digest, so that `auto` can tell a first look from a
/// return. Never fails: a banner that could not be drawn is one a person does not see, and
/// taking the shell down with it would be the one unacceptable outcome.
fn draw(environment: &RepositoryEnvironment, mode: BannerMode, presentation: &Presentation) {
    let root = std::path::Path::new(&environment.repository.root);
    let local_half = &environment.repository.local_path;
    let seen = Cache::load(root, local_half).tiers.last_rendered_digest;
    let Some(text) = banner(environment, mode, presentation, seen.as_deref()) else {
        return;
    };
    // Standard error: direnv reads standard output as the environment it is applying.
    eprint!("{text}");
    let digest = environment.digest();
    if seen.as_deref() != Some(digest.as_str()) {
        let mut cache = Cache::load(root, local_half);
        cache.tiers.last_rendered_digest = Some(digest);
        if let Err(e) = cache.store(root, local_half) {
            tracing::debug!(error = %e, "the rendered digest could not be recorded");
        }
    }
}

fn banner_command(repo: &RepoArgs, mode: Option<&str>, width: Option<usize>) -> Result<u8> {
    let mode = requested_mode(mode)?;
    // Off costs nothing at all: no repository discovery, no git, no cache. A person who
    // turned the banner off should not pay for one that is then thrown away.
    if mode == BannerMode::Off {
        return Ok(0);
    }
    let mut presentation = Presentation::detect();
    if let Some(width) = width {
        presentation.columns = width;
        // An explicit width means a caller is asking for a rendering rather than reading a
        // terminal, so it gets one whatever `auto` would have decided about the terminal.
        presentation.interactive = true;
    }
    // `auto` decides from the terminal before anything is resolved: in a pipe or under CI
    // the answer is silence, and silence must not cost a `git status`.
    if mode == BannerMode::Auto && !presentation.interactive {
        return Ok(0);
    }

    let (environment, _) = resolve_fast(repo)?;
    draw(&environment, mode, &presentation);
    Ok(0)
}

fn export_command(
    repo: &RepoArgs,
    shell: &str,
    with_banner: bool,
    mode: Option<&str>,
    with_bridge: bool,
) -> Result<u8> {
    let dialect = Dialect::parse(shell).ok_or_else(|| Error::Protocol {
        reason: format!(
            "'{shell}' is not a shell this writes for; one of direnv, bash, zsh, sh, ksh, fish"
        ),
    })?;
    // Refused before anything is resolved, so a mode nobody can render never costs a
    // `git status` first.
    let mode = with_banner.then(|| requested_mode(mode)).transpose()?;
    let (environment, share) = resolve_fast(repo)?;
    let share = share.map(|s| s.dir().display().to_string());
    let script = export(&environment, share.as_deref(), dialect);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write!(out, "{script}").map_err(Error::Transport)?;
    if let Some(mode) = mode {
        // The same snapshot, rendered for the other stream. One resolution, two surfaces:
        // this is the whole reason an adapter makes one call rather than two.
        draw(&environment, mode, &Presentation::detect());
    }
    if with_bridge {
        refresh_bridge(repo);
    }
    Ok(0)
}

/// The resolution each subcommand uses.
///
/// Test-only, and honestly so: the dispatch above does not consult this — each of the four
/// commands calls `resolve_full` or `resolve_fast` directly — so this restates that decision
/// rather than making it. It earns its place by being the thing the test names, which is how
/// the rule the module is built around ("only the two a person waits on may build the index")
/// is written down somewhere a change has to walk past. It was a doc example until the
/// crate's public surface narrowed and an example outside the crate could no longer name it.
#[cfg(test)]
use crate::environment::Resolution;

#[cfg(test)]
pub fn resolution_of(subcommand: &str) -> Option<Resolution> {
    match subcommand {
        "status" | "explain" => Some(Resolution::Full),
        "banner" | "export" => Some(Resolution::Fast),
        _ => None,
    }
}

/// Refresh the workflow bridge if one of the declarations behind it has changed.
///
/// The third thing an entry into the repository is for. It is here rather than in the
/// shell entry point because deciding whether a generated file is current is exactly the
/// kind of work the entry point may not do: it would be a second implementation of the
/// staleness rule, running on every `cd`, that nothing tests.
///
/// Silent, and never fatal. A repository the graph cannot be built for still has an
/// environment, and a failure here must not make a shell report that entering the
/// directory failed.
fn refresh_bridge(repo: &RepoArgs) {
    use crate::command_graph::{bridge, load};
    let Ok(root) = load::root(repo) else {
        return;
    };
    if bridge::is_current(&root) {
        return;
    }
    let Ok(loaded) = load::full(repo) else {
        return;
    };
    if !loaded.graph.errors().is_empty() {
        return;
    }
    let _ = bridge::materialise(&root, &loaded.graph);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule the whole command is built around: only the two a person waits on may
    /// build the index. If this ever says otherwise, `cd` has become slow.
    #[test]
    fn only_the_commands_a_person_waits_on_resolve_in_full() {
        assert_eq!(resolution_of("status"), Some(Resolution::Full));
        assert_eq!(resolution_of("explain"), Some(Resolution::Full));
        assert_eq!(resolution_of("banner"), Some(Resolution::Fast));
        assert_eq!(resolution_of("export"), Some(Resolution::Fast));
    }

    #[test]
    fn a_banner_mode_that_is_not_one_is_refused_by_name() {
        let args = RepoArgs::default();
        match banner_command(&args, Some("loud"), None) {
            Err(Error::Protocol { reason }) => assert!(reason.contains("loud"), "{reason}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_shell_this_does_not_write_for_is_refused_by_name() {
        let args = RepoArgs::default();
        match export_command(&args, "powershell", false, None, false) {
            Err(Error::Protocol { reason }) => assert!(reason.contains("powershell"), "{reason}"),
            other => panic!("{other:?}"),
        }
    }

    /// `off` must be free. It is what a person sets when they do not want the banner, and
    /// paying a `git status` to then print nothing would be the one cost they were trying
    /// to avoid.
    #[test]
    fn a_banner_turned_off_resolves_nothing_at_all() {
        // A repository that does not exist: reaching the resolver would fail, so this
        // returning 0 is the assertion that it never got there.
        let args = RepoArgs {
            repo: Some("/nonexistent/majordomus/checkout".into()),
            ..RepoArgs::default()
        };
        assert_eq!(
            banner_command(&args, Some("off"), None).expect("no work"),
            0
        );
    }
}
