//! `majordomus web`: the repository's web surfaces, from one discovery.
//!
//! Every subcommand is a thin adapter over [`crate::web`]: `list` and `explain` describe the
//! resolved topology, `validate` reports its findings, `manifest` writes it down and
//! `compose` builds the publishable tree from it. Nothing here knows the name of any
//! surface — a surface that is discovered is listed, selectable, validated and composed
//! because it was discovered, which is the whole point of the architecture.

use std::io::Write;
use std::path::PathBuf;

use crate::cli::{OutputFormat, ReportCommand, WebArgs, WebCommand};
use crate::error::{Error, Result};
use crate::repository::Repository;
use crate::web::compose::{self, PUBLISH_ROOT};
use crate::web::discover::{self, Runtime};
use crate::web::manifest::Manifest;
use crate::web::model::Topology;
use crate::web::validate::{self, Artifacts, Severity};

/// The exit code when the topology does not validate.
pub const EXIT_INVALID_TOPOLOGY: u8 = 10;

/// Run `majordomus web`.
pub fn run(args: WebArgs) -> Result<u8> {
    // The topology is a fact about the repository tree, not about the index: discovery
    // reads the site's configuration, the generated root and the executable's own
    // constants, so `web` answers in a repository whose index would not build.
    let start = match &args.repo.repo {
        Some(path) => path.clone(),
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let repository = Repository::discover(&start)?;
    let root = repository.root().to_path_buf();
    let topology = discover::discover(&root, Runtime::full())?;
    let selected = topology.select(&args.only, &args.exclude);
    unknown_selectors(&topology, &args.only, &args.exclude)?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command.unwrap_or(WebCommand::List) {
        WebCommand::List => list(&mut out, &selected, args.format),
        WebCommand::Explain { id } => explain(&mut out, &selected, id.as_deref(), args.format),
        WebCommand::Validate { artifacts } => {
            let policy = if artifacts {
                Artifacts::Required
            } else {
                Artifacts::Ignore
            };
            check(&mut out, &selected, &root, policy, args.format)
        }
        WebCommand::Manifest => {
            let findings = validate::validate(&selected, &root, Artifacts::Ignore);
            let manifest = Manifest::new(selected.clone(), findings, env!("CARGO_PKG_VERSION"));
            let path = manifest.write(&root)?;
            writeln!(
                out,
                "web manifest: {} ({} surface(s))",
                path.display(),
                selected.surfaces.len()
            )
            .map_err(Error::Transport)?;
            Ok(0)
        }
        WebCommand::Report { report } => {
            let dir = match report {
                ReportCommand::Tests {
                    suite,
                    crate_output,
                } => {
                    let text = std::fs::read_to_string(&suite)
                        .map_err(|e| Error::io(suite.display().to_string(), e))?;
                    let mut run = crate::web::report::tests::parse_cases(&text)?;
                    if let Some(path) = crate_output {
                        let cargo = std::fs::read_to_string(&path)
                            .map_err(|e| Error::io(path.display().to_string(), e))?;
                        run.crate_tests =
                            Some(crate::web::report::tests::parse_crate_tests(&cargo));
                    }
                    crate::web::report::tests::render(&root, &run)?
                }
                ReportCommand::Benchmarks { from } => {
                    let document = crate::web::report::benchmarks::read(&from)?;
                    let source = from
                        .strip_prefix(&root)
                        .unwrap_or(&from)
                        .to_string_lossy()
                        .to_string();
                    crate::web::report::benchmarks::render(&root, &document, &source)?
                }
            };
            let rel = dir.strip_prefix(&root).unwrap_or(&dir);
            writeln!(out, "web report: {}", rel.display()).map_err(Error::Transport)?;
            Ok(0)
        }
        WebCommand::Compose { destination } => {
            let dest = destination
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(PUBLISH_ROOT));
            let absolute = if dest.is_absolute() {
                dest.clone()
            } else {
                root.join(&dest)
            };
            let findings = validate::validate(&selected, &root, Artifacts::Required);
            if validate::blocking(&findings) {
                report(&mut out, &findings, args.format)?;
                return Ok(EXIT_INVALID_TOPOLOGY);
            }
            let composition = compose::compose(&selected, &root, &absolute)?;
            match args.format {
                OutputFormat::Json => writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&composition).unwrap_or_default()
                ),
                OutputFormat::Text => {
                    writeln!(out, "composed into {}", dest.display()).and_then(|()| {
                        for (id, files) in &composition.files {
                            let mount = selected
                                .get(id)
                                .map(|s| s.mount.to_string())
                                .unwrap_or_else(|| "?".into());
                            writeln!(out, "  {mount:<16} {id:<16} {files} file(s)")?;
                        }
                        writeln!(
                            out,
                            "{} file(s) from {} surface(s)",
                            composition.total(),
                            composition.files.len()
                        )
                    })
                }
            }
            .map_err(Error::Transport)?;
            Ok(0)
        }
    }
}

/// A selector that names nothing is a mistake, not an empty selection: it silently serves or
/// publishes less than the caller asked for.
fn unknown_selectors(topology: &Topology, only: &[String], exclude: &[String]) -> Result<()> {
    let known: Vec<&str> = topology.ids();
    for id in only.iter().chain(exclude.iter()) {
        if !known.iter().any(|k| k == id) {
            return Err(Error::InvalidSurface {
                surface: id.clone(),
                reason: format!(
                    "no surface with this id was discovered; the ids are: {}",
                    known.join(", ")
                ),
            });
        }
    }
    Ok(())
}

fn list(out: &mut impl Write, topology: &Topology, format: OutputFormat) -> Result<u8> {
    match format {
        OutputFormat::Json => {
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(topology).unwrap_or_default()
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            writeln!(
                out,
                "{:<16} {:<9} {:<16} {}",
                "ID", "KIND", "MOUNT", "SOURCE"
            )
            .map_err(Error::Transport)?;
            for surface in &topology.surfaces {
                let source = surface
                    .artifact
                    .as_ref()
                    .map(|a| a.to_string_lossy().to_string())
                    .unwrap_or_else(|| surface.producer.clone());
                writeln!(
                    out,
                    "{:<16} {:<9} {:<16} {}",
                    surface.id,
                    surface.kind.to_string(),
                    surface.mount.to_string(),
                    source
                )
                .map_err(Error::Transport)?;
            }
        }
    }
    Ok(0)
}

fn explain(
    out: &mut impl Write,
    topology: &Topology,
    id: Option<&str>,
    format: OutputFormat,
) -> Result<u8> {
    let chosen: Vec<_> = match id {
        Some(want) => topology.surfaces.iter().filter(|s| s.id == want).collect(),
        None => topology.surfaces.iter().collect(),
    };
    if chosen.is_empty() {
        return Err(Error::InvalidSurface {
            surface: id.unwrap_or("?").into(),
            reason: format!(
                "no such surface; the ids are: {}",
                topology.ids().join(", ")
            ),
        });
    }
    match format {
        OutputFormat::Json => {
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&chosen).unwrap_or_default()
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            for surface in chosen {
                writeln!(out, "{}", surface.id).map_err(Error::Transport)?;
                writeln!(out, "  title      {}", surface.title).map_err(Error::Transport)?;
                writeln!(out, "  kind       {}", surface.kind).map_err(Error::Transport)?;
                writeln!(out, "  mount      {}", surface.mount).map_err(Error::Transport)?;
                writeln!(out, "  producer   {}", surface.producer).map_err(Error::Transport)?;
                if let Some(artifact) = &surface.artifact {
                    writeln!(out, "  artifact   {}", artifact.display())
                        .map_err(Error::Transport)?;
                }
                if let Some(index) = &surface.index {
                    writeln!(out, "  index      {index}").map_err(Error::Transport)?;
                }
                for (field, provenance) in &surface.provenance {
                    writeln!(out, "  {field:<10} came from {provenance}")
                        .map_err(Error::Transport)?;
                }
                writeln!(out).map_err(Error::Transport)?;
            }
        }
    }
    Ok(0)
}

fn check(
    out: &mut impl Write,
    topology: &Topology,
    root: &std::path::Path,
    artifacts: Artifacts,
    format: OutputFormat,
) -> Result<u8> {
    let findings = validate::validate(topology, root, artifacts);
    report(out, &findings, format)?;
    if validate::blocking(&findings) {
        return Ok(EXIT_INVALID_TOPOLOGY);
    }
    if format == OutputFormat::Text {
        writeln!(
            out,
            "web validate: {} surface(s), no conflict",
            topology.surfaces.len()
        )
        .map_err(Error::Transport)?;
    }
    Ok(0)
}

fn report(
    out: &mut impl Write,
    findings: &[validate::Finding],
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(findings).unwrap_or_default()
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            for finding in findings {
                let mark = match finding.severity {
                    Severity::Error => "FAIL",
                    Severity::Warning => "WARN",
                };
                writeln!(
                    out,
                    "{mark} {:<38} {}\n     {}\n     fix: {}",
                    finding.rule, finding.surface, finding.message, finding.remedy
                )
                .map_err(Error::Transport)?;
            }
        }
    }
    Ok(())
}
