//! `majordomus bench`: time every target the projection derives, report coverage, keep
//! and check baselines. The list of what is timed is never written here: it is the
//! benchmark projection of the registry against this repository.

use std::io::Write;

use crate::app::App;
use crate::bench::baseline::{self, Check, Policy};
use crate::bench::results::{CacheMode, Provenance, ResultDocument};
use crate::bench::{BenchmarkProjection, Coverage, Profile, Runner, Transport};
use crate::cli::{BaselineCommand, BenchArgs, BenchCommand, OutputFormat, TransportArg};
use crate::error::{Error, Result};

/// Run `majordomus bench`.
pub fn run(args: BenchArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = app.context.clone();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let projection = BenchmarkProjection::from_context(&ctx);
    match args.command {
        Some(BenchCommand::Coverage { format, check }) => {
            let coverage = Coverage::compute(&ctx, &projection);
            match format {
                OutputFormat::Json => {
                    writeln!(out, "{}", pretty(&coverage)).map_err(Error::Transport)?
                }
                OutputFormat::Text => {
                    write!(out, "{}", coverage.render()).map_err(Error::Transport)?
                }
            }
            if check && !coverage.is_complete() {
                return Ok(10);
            }
            Ok(0)
        }
        Some(BenchCommand::Baseline {
            command:
                BaselineCommand::Update {
                    profile,
                    allow_dirty,
                },
        }) => {
            let profile = parse_profile(profile.name())?;
            let provenance = Provenance::of(&app.repository, ctx.registry.fingerprint());
            if provenance.dirty && !allow_dirty {
                return Err(Error::Protocol {
                    reason: "the work tree is dirty; a baseline is recorded from a clean commit (or pass --allow-dirty)".into(),
                });
            }
            let doc = measure(&app, &projection, profile, None, None)?;
            let path = baseline::write_baseline(&app.repository, &doc)?;
            writeln!(out, "{}", path.display()).map_err(Error::Transport)?;
            writeln!(
                out,
                "baseline {}: {} measurement(s) from profile {}, commit {}",
                doc.provenance.platform(),
                doc.results.len(),
                doc.profile,
                doc.provenance.commit.as_deref().unwrap_or("-")
            )
            .map_err(Error::Transport)?;
            Ok(0)
        }
        None => {
            let profile = parse_profile(args.profile.name())?;
            let transport = match args.transport {
                TransportArg::All => None,
                TransportArg::System => Some(Filter::System),
                TransportArg::Direct => Some(Filter::Transport(Transport::Direct)),
                TransportArg::Mcp => Some(Filter::Transport(Transport::Mcp)),
                TransportArg::Http => Some(Filter::Transport(Transport::Http)),
            };
            let doc = measure(&app, &projection, profile, args.id.as_deref(), transport)?;
            if doc.results.is_empty() {
                return Err(Error::CapabilityNotFound {
                    id: args
                        .id
                        .unwrap_or_else(|| format!("{:?}", args.transport).to_lowercase()),
                });
            }
            let written = if args.no_write {
                None
            } else {
                Some(doc.write_local(&app.repository)?)
            };
            match args.format {
                OutputFormat::Json => {
                    writeln!(out, "{}", pretty(&doc)).map_err(Error::Transport)?
                }
                OutputFormat::Text => {
                    write!(out, "{}", render(&doc)).map_err(Error::Transport)?;
                    if let Some(p) = &written {
                        writeln!(out, "written {}", p.display()).map_err(Error::Transport)?;
                    }
                }
            }
            if args.check {
                let policy = Policy::load(&app.repository)?;
                let baseline =
                    baseline::load_baseline(&app.repository, &doc.provenance.platform())?;
                let check = Check::compare(&doc, baseline.as_ref(), &policy);
                match args.format {
                    OutputFormat::Json => {
                        writeln!(out, "{}", pretty(&check)).map_err(Error::Transport)?
                    }
                    OutputFormat::Text => {
                        write!(out, "{}", check.render()).map_err(Error::Transport)?
                    }
                }
                if check.failed() {
                    return Ok(10);
                }
            }
            Ok(0)
        }
    }
}

enum Filter {
    Transport(Transport),
    System,
}

fn parse_profile(name: &str) -> Result<Profile> {
    Profile::parse(name).ok_or_else(|| Error::Protocol {
        reason: format!("--profile {name}: not quick, full or ci"),
    })
}

/// Time the selected targets and assemble the document.
fn measure(
    app: &App,
    projection: &BenchmarkProjection,
    profile: Profile,
    id: Option<&str>,
    filter: Option<Filter>,
) -> Result<ResultDocument> {
    let ctx = app.context.clone();
    let mut child_args = Vec::new();
    if args_discovery_is_filesystem(app) {
        child_args.push("--discovery".to_string());
        child_args.push("filesystem".to_string());
    }
    let mut runner = Runner::new(ctx.clone(), profile, app.repository.root())
        .with_share(app.share.dir().to_path_buf())
        .with_child_args(child_args);
    let mut results = Vec::new();
    for target in &projection.targets {
        if let Some(id) = id {
            let matches = target.capability_id() == Some(id) || target.key.starts_with(id);
            if !matches {
                continue;
            }
        }
        match &filter {
            Some(Filter::Transport(t))
                if target.transport() != *t || target.capability_id().is_none() =>
            {
                continue
            }
            Some(Filter::System) if target.capability_id().is_some() => continue,
            _ => {}
        }
        tracing::info!(target = %target.key, "benchmarking");
        results.extend(runner.run(target)?);
    }
    runner.finish();
    Ok(ResultDocument {
        schema: crate::bench::RESULT_SCHEMA.into(),
        finished_at: crate::peers::rfc3339(std::time::SystemTime::now()),
        profile: profile.name.into(),
        provenance: Provenance::of(&app.repository, ctx.registry.fingerprint()),
        results,
    })
}

/// The human report: slowest p95 first.
fn render(doc: &ResultDocument) -> String {
    let mut rows: Vec<&crate::bench::BenchmarkResult> = doc.results.iter().collect();
    rows.sort_by(|a, b| {
        b.stats
            .p95_us
            .partial_cmp(&a.stats.p95_us)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut s = format!(
        "benchmark {} — {} measurement(s), profile {}, {} {}, commit {}{}\n\n{:<52} {:<6} {:>4} {:>10} {:>10} {:>10} {:>10}\n",
        doc.provenance.platform(),
        doc.results.len(),
        doc.profile,
        doc.provenance.os,
        doc.provenance.arch,
        doc.provenance.commit.as_deref().map(|c| &c[..c.len().min(12)]).unwrap_or("-"),
        if doc.provenance.dirty { " (dirty)" } else { "" },
        "target",
        "cache",
        "n",
        "p50 ms",
        "p95 ms",
        "p99 ms",
        "max ms"
    );
    for r in rows {
        let cache = match r.cache_mode {
            CacheMode::Uncached => "-",
            CacheMode::Cold => "cold",
            CacheMode::Warm => "warm",
            CacheMode::NotApplicable => "",
        };
        s.push_str(&format!(
            "{:<52} {:<6} {:>4} {:>10.3} {:>10.3} {:>10.3} {:>10.3}\n",
            r.key,
            cache,
            r.stats.samples,
            r.stats.p50_us / 1000.0,
            r.stats.p95_us / 1000.0,
            r.stats.p99_us / 1000.0,
            r.stats.max_us / 1000.0
        ));
    }
    s
}

/// Did the parent read the repository through the filesystem? The child must too.
fn args_discovery_is_filesystem(app: &App) -> bool {
    app.context.index.repository.discovery == "filesystem"
}

fn pretty<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}
