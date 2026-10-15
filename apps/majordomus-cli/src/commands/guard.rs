//! `majordomus guard`: the edit guard on the command line (I2735).
//!
//! `guard hook` is what a provider's pre-tool hook runs. It reads the provider's payload from
//! stdin, answers `guard.edit` for the path it names without building the index — it runs
//! before every edit — and says the answer in the one code the provider reads as a refusal:
//! 2, with the reason and the remedy on stderr, which the provider hands to the model. Every
//! other outcome is 0, an undecidable one included: a guard that refused because it could
//! not read its own inputs would stop every edit of a checkout nobody has served.

use std::io::Read;
use std::path::PathBuf;

use crate::cli::{GuardArgs, GuardCommand, GuardHookArgs};
use crate::error::Result;
use crate::guard::{from_payload, judge, server_claims};

/// The code a provider's pre-tool hook reads as "do not run this tool".
const EXIT_REFUSED: u8 = 2;

/// Run `majordomus guard`.
pub fn run(args: GuardArgs) -> Result<u8> {
    match args.command {
        GuardCommand::Hook(a) => hook(a),
    }
}

fn hook(args: GuardHookArgs) -> Result<u8> {
    let mut payload = String::new();
    if std::io::stdin().read_to_string(&mut payload).is_err() {
        return Ok(0);
    }
    let Some((path, session)) = from_payload(&payload) else {
        return Ok(0);
    };
    // The checkout is the one the written file belongs to, not the one the hook was installed
    // in: a session started in the primary checkout edits files of linked worktrees, and each
    // of those has its own task, its own server and its own claims. A relative path is the
    // hook's working directory's, which is the provider's project.
    let base = args
        .repo
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let target = base.join(&path);
    let start = target
        .ancestors()
        .find(|a| a.is_dir())
        .map(PathBuf::from)
        .unwrap_or(base);
    let Ok(repository) = crate::repository::Repository::discover(&start) else {
        return Ok(0);
    };
    let root = repository.root().to_path_buf();
    // the root is canonical, so the path is spelled the same way: its existing ancestor
    // canonicalised, and whatever of it does not exist yet appended
    let path = match (start.canonicalize(), target.strip_prefix(&start)) {
        (Ok(real), Ok(rest)) => real.join(rest).to_string_lossy().to_string(),
        _ => target.to_string_lossy().to_string(),
    };
    let session = session.map(|s| format!("{}-{s}", args.provider));
    let verdict = judge(&root, &path, session.as_deref(), || server_claims(&root));
    if verdict.allowed {
        return Ok(0);
    }
    eprintln!("majordomus guard: {}", verdict.detail);
    if let Some(remedy) = &verdict.remedy {
        eprintln!("  {remedy}");
    }
    eprintln!(
        "  (session.edits_require_claim in .ai/repo/policy.yaml turns this guard off; I2735)"
    );
    Ok(EXIT_REFUSED)
}
