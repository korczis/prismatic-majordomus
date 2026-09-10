//! `majordomus generate`: write the committed projections, or with `--check` say whether
//! the committed ones still match the registry. Check mode writes nothing.

use std::io::Write;

use crate::app::App;
use crate::cli::{GenerateArgs, GenerateTarget};
use crate::error::{Error, Result};
use crate::generate::{self, Target};

/// Run `majordomus generate`.
pub fn run(args: GenerateArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    // Before anything is planned: a stale executable rewrites every artifact's provenance
    // header with its own, older version, and nothing else about the bytes changes, so the
    // damage reads as an ordinary regeneration. It happened — a leftover 0.2.0 build in
    // target/release restamped forty generated files in a 0.3.1 checkout and no check
    // objected. Where the repository being generated is the one that declares this
    // executable, the two versions must agree.
    refuse_stale_executable(&app)?;
    let targets: &[Target] = match args.target {
        GenerateTarget::All => Target::ALL,
        GenerateTarget::Openapi => &[Target::OpenApi],
        GenerateTarget::Docs => &[Target::Docs],
        GenerateTarget::Benchmarks => &[Target::Benchmarks],
        GenerateTarget::Registry => &[Target::Registry],
        GenerateTarget::Allow => &[Target::Allow],
        GenerateTarget::Providers => &[Target::Providers],
        GenerateTarget::Site => &[Target::Site],
        GenerateTarget::Distribution => &[Target::Distribution],
        GenerateTarget::Manifest => &[Target::Manifest],
        GenerateTarget::Web => &[Target::Web],
        GenerateTarget::Changelog => &[Target::Changelog],
        GenerateTarget::Deployment => &[Target::Deployment],
        GenerateTarget::Design => &[Target::Design],
    };
    let artifacts = generate::plan(&app, targets)?;
    // the contract before the bytes: an artifact that carries no provenance, declares an
    // encoding its suffix contradicts, or breaks the schema it names is never written and
    // never reported in sync
    let schemas = generate::GeneratedSchemas::load(&app.share.generated_schemas_dir())?;
    generate::verify(&artifacts, &schemas)?;
    let root = args
        .out
        .clone()
        .unwrap_or_else(|| app.repository.root().to_path_buf());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if args.check {
        generate::check(&root, &artifacts)?;
        for a in &artifacts {
            writeln!(
                out,
                "OK   generated   {} — {}, matches the registry",
                a.path,
                a.schema.as_deref().unwrap_or(a.format.suffix())
            )
            .map_err(Error::Transport)?;
        }
        writeln!(out, "generate --check: in sync").map_err(Error::Transport)?;
    } else {
        for path in generate::write(&root, &artifacts)? {
            writeln!(
                out,
                "{}",
                path.strip_prefix(&root).unwrap_or(&path).display()
            )
            .map_err(Error::Transport)?;
        }
    }
    Ok(0)
}

/// Refuse when this executable was built from a different revision of the crate the
/// repository declares. The question is only asked where the repository carries the
/// manifest: a foreign repository has no opinion about this executable's version, and
/// generating there is exactly what a released binary is for.
fn refuse_stale_executable(app: &App) -> Result<()> {
    let manifest = app.repository.root().join("apps/majordomus-cli/Cargo.toml");
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return Ok(());
    };
    let Some(declared) = package_version(&text) else {
        return Ok(());
    };
    if declared == crate::VERSION {
        return Ok(());
    }
    Err(Error::Refused {
        code: 10,
        reason: format!(
            "this executable is {running} and {path} declares {declared}: generating would \
             stamp every artifact with a version this tree does not have. Rebuild it — \
             `cargo build --release --manifest-path apps/majordomus-cli/Cargo.toml` — or run \
             `scripts/derive`, which builds it before it generates.",
            running = crate::VERSION,
            path = "apps/majordomus-cli/Cargo.toml",
        ),
    })
}

/// The `version` of the manifest's `[package]` table. Read without a TOML parser, because
/// the one field wanted here is a quoted scalar in a table whose header is unambiguous, and
/// a dependency to read it would be a dependency this crate argues against carrying.
fn package_version(text: &str) -> Option<String> {
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some(rest) = line.strip_prefix("version") else {
            continue;
        };
        let rest = rest.trim_start().strip_prefix('=')?.trim();
        let value = rest.split('#').next().unwrap_or(rest).trim();
        return Some(value.trim_matches('"').to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::package_version;

    #[test]
    fn reads_the_package_version_and_not_a_dependency_version() {
        let manifest = "[package]\nname = \"majordomus-cli\"\nversion = \"0.3.1\"\n\n[dependencies]\nclap = { version = \"4.5\" }\n";
        assert_eq!(package_version(manifest).as_deref(), Some("0.3.1"));
    }

    #[test]
    fn a_version_outside_the_package_table_is_not_the_package_version() {
        let manifest = "[dependencies]\nserde = { version = \"1\" }\n";
        assert_eq!(package_version(manifest), None);
    }

    #[test]
    fn a_trailing_comment_is_not_part_of_the_version() {
        let manifest = "[package]\nversion = \"1.2.3\"   # bumped by `release version`\n";
        assert_eq!(package_version(manifest).as_deref(), Some("1.2.3"));
    }
}
