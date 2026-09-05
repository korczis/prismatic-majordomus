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
    let targets: &[Target] = match args.target {
        GenerateTarget::All => Target::ALL,
        GenerateTarget::Openapi => &[Target::OpenApi],
        GenerateTarget::Docs => &[Target::Docs],
        GenerateTarget::Benchmarks => &[Target::Benchmarks],
        GenerateTarget::Registry => &[Target::Registry],
        GenerateTarget::Allow => &[Target::Allow],
        GenerateTarget::Providers => &[Target::Providers],
        GenerateTarget::Site => &[Target::Site],
    };
    let artifacts = generate::plan(&app, targets)?;
    let root = args
        .out
        .clone()
        .unwrap_or_else(|| app.repository.root().to_path_buf());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if args.check {
        generate::check(&root, &artifacts)?;
        for a in &artifacts {
            writeln!(out, "OK   generated   {} — matches the registry", a.path)
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
