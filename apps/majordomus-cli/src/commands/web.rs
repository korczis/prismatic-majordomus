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
use crate::web::model::{Availability, Topology};
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
            // master fixed the same clippy finding by inlining SOURCE; this keeps that and
            // adds the two columns the two worlds need
            writeln!(
                out,
                "{:<16} {:<9} {:<16} {:<14} {:<14} SOURCE",
                "ID", "KIND", "MOUNT", "CATEGORY", "WHERE"
            )
            .map_err(Error::Transport)?;
            for surface in &topology.surfaces {
                let source = surface
                    .artifact
                    .as_ref()
                    .map(|a| a.to_string_lossy().to_string())
                    .unwrap_or_else(|| surface.producer.clone());
                // two surfaces may share a mount when they live in different worlds — the
                // site as it is deployed owns `/` of a publication, the home page owns `/`
                // of a process — so a listing that hid the world would look like a conflict
                let world = match surface.availability {
                    Availability::Both => "served+published",
                    Availability::ServedOnly => "served",
                    Availability::PublishedOnly => "published",
                };
                writeln!(
                    out,
                    "{:<16} {:<9} {:<16} {:<14} {:<14} {}",
                    surface.id,
                    surface.kind.to_string(),
                    surface.mount.to_string(),
                    surface.category.to_string(),
                    world,
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
                writeln!(out, "  category   {}", surface.category).map_err(Error::Transport)?;
                writeln!(out, "  visibility {}", surface.visibility).map_err(Error::Transport)?;
                writeln!(out, "  where      {:?}", surface.availability)
                    .map_err(Error::Transport)?;
                if let Some(feature) = surface.feature {
                    writeln!(out, "  needs      the process to serve {feature}")
                        .map_err(Error::Transport)?;
                }
                if let Some(built) = &surface.built_from {
                    writeln!(out, "  built from {built}").map_err(Error::Transport)?;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::discover::{self, Runtime};
    use crate::web::validate::Finding;

    fn topology() -> Topology {
        Topology::new(discover::native_all())
    }

    fn rendered(f: impl FnOnce(&mut Vec<u8>) -> Result<u8>) -> String {
        let mut out = Vec::new();
        let code = f(&mut out).expect("the renderer writes");
        assert_eq!(code, 0);
        String::from_utf8(out).expect("the renderer writes text")
    }

    #[test]
    fn a_listing_names_every_surface_with_the_world_it_lives_in() {
        let text = rendered(|out| list(out, &topology(), OutputFormat::Text));
        assert!(text.starts_with("ID "), "{text}");
        for id in topology().ids() {
            assert!(
                text.contains(id),
                "{id} is missing from the listing:\n{text}"
            );
        }
        // the world is on the line, because two surfaces may share a mount across worlds
        assert!(text.contains("served"), "{text}");
        assert!(text.contains("documentation"), "{text}");
    }

    #[test]
    fn a_listing_as_json_is_the_topology_itself() {
        let text = rendered(|out| list(out, &topology(), OutputFormat::Json));
        let parsed: Topology = serde_json::from_str(&text).expect("the JSON is a topology");
        assert_eq!(parsed.ids(), topology().ids());
    }

    #[test]
    fn explaining_a_surface_says_where_each_of_its_values_came_from() {
        let text = rendered(|out| explain(out, &topology(), Some("swagger"), OutputFormat::Text));
        assert!(text.contains("swagger"), "{text}");
        assert!(text.contains("category   documentation"), "{text}");
        assert!(text.contains("visibility public"), "{text}");
        assert!(text.contains("came from"), "{text}");

        // no id explains every surface
        let all = rendered(|out| explain(out, &topology(), None, OutputFormat::Text));
        for id in topology().ids() {
            assert!(all.contains(id), "{id}:\n{all}");
        }
        let json = rendered(|out| explain(out, &topology(), Some("swagger"), OutputFormat::Json));
        assert!(serde_json::from_str::<Vec<crate::web::Surface>>(&json).is_ok());
    }

    #[test]
    fn explaining_a_surface_that_does_not_exist_lists_the_ones_that_do() {
        let mut out = Vec::new();
        let err = explain(&mut out, &topology(), Some("invented"), OutputFormat::Text)
            .expect_err("a surface nobody declared cannot be explained")
            .to_string();
        assert!(err.contains("invented"), "{err}");
        assert!(err.contains("swagger"), "the ids are named: {err}");
    }

    #[test]
    fn a_selector_that_names_nothing_is_a_mistake_rather_than_an_empty_selection() {
        let topology = topology();
        assert!(unknown_selectors(&topology, &[], &[]).is_ok());
        assert!(unknown_selectors(&topology, &["swagger".into()], &[]).is_ok());
        let err = unknown_selectors(&topology, &["invented".into()], &[])
            .expect_err("a selector that selects nothing is refused")
            .to_string();
        assert!(err.contains("invented"), "{err}");
        let err = unknown_selectors(&topology, &[], &["invented".into()])
            .expect_err("an exclusion that excludes nothing is refused too")
            .to_string();
        assert!(err.contains("invented"), "{err}");
    }

    #[test]
    fn a_clean_topology_reports_no_conflict_and_a_broken_one_exits_ten() {
        let text = rendered(|out| {
            check(
                out,
                &topology(),
                std::path::Path::new("/nonexistent"),
                Artifacts::Ignore,
                OutputFormat::Text,
            )
        });
        assert!(text.contains("no conflict"), "{text}");

        let mut surfaces = discover::native_all();
        let mut clone = surfaces
            .iter()
            .find(|s| s.id == "cockpit")
            .expect("the cockpit is declared")
            .clone();
        clone.id = "second".into();
        surfaces.push(clone);
        let mut out = Vec::new();
        let code = check(
            &mut out,
            &Topology::new(surfaces),
            std::path::Path::new("/nonexistent"),
            Artifacts::Ignore,
            OutputFormat::Text,
        )
        .expect("a broken topology reports rather than fails");
        assert_eq!(code, EXIT_INVALID_TOPOLOGY);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("FAIL"), "{text}");
        assert!(
            text.contains("fix:"),
            "every finding carries its remedy: {text}"
        );
    }

    #[test]
    fn findings_render_as_json_when_a_program_is_reading() {
        let findings = vec![Finding {
            severity: Severity::Warning,
            rule: "surface.example".into(),
            surface: "example".into(),
            message: "something".into(),
            remedy: "do something".into(),
        }];
        let mut out = Vec::new();
        report(&mut out, &findings, OutputFormat::Json).unwrap();
        let parsed: Vec<Finding> = serde_json::from_slice(&out).unwrap();
        assert_eq!(parsed, findings);
    }

    #[test]
    fn a_process_that_offers_nothing_lists_only_what_it_can_answer() {
        let bare = Topology::new(discover::native(Runtime::default()));
        let text = rendered(|out| list(out, &bare, OutputFormat::Text));
        assert!(!text.contains("cockpit"), "{text}");
        assert!(text.contains("swagger"), "{text}");
    }
}
