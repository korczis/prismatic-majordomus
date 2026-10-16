//! `majordomus guard`: the edit guard on the command line (I2735).
//!
//! `guard hook` is what a provider's pre-tool hook runs. It reads the provider's payload from
//! stdin, answers `guard.edit` for the path it names without building the index — it runs
//! before every edit — and says the answer in the one code the provider reads as a refusal:
//! 2, with the reason and the remedy on stderr, which the provider hands to the model. Every
//! other outcome is 0, an undecidable one included: a guard that refused because it could
//! not read its own inputs would stop every edit of a checkout nobody has served.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::cli::{GuardArgs, GuardCommand, GuardHookArgs};
use crate::error::Result;
use crate::guard::{from_payload, judge, server_claims, ClaimSource, EditVerdict};

/// The code a provider's pre-tool hook reads as "do not run this tool".
const EXIT_REFUSED: u8 = 2;

/// Run `majordomus guard`.
pub fn run(args: GuardArgs) -> Result<u8> {
    match args.command {
        GuardCommand::Hook(a) => hook(a),
    }
}

fn hook(args: GuardHookArgs) -> Result<u8> {
    // a payload that cannot be read whole names no path, and an edit nobody can name is let
    // through like every other undecidable one
    let mut payload = String::new();
    let _ = std::io::stdin().read_to_string(&mut payload);
    let base = args.repo.clone().or_else(|| std::env::current_dir().ok());
    let verdict = answer(&args.provider, base, &payload, server_claims);
    Ok(report(verdict.as_ref(), &mut std::io::stderr()))
}

/// The verdict on the edit `payload` names, or `None` when it names no file of a repository.
///
/// The checkout is the one the written file belongs to, not the one the hook was installed
/// in: a session started in the primary checkout edits files of linked worktrees, and each of
/// those has its own task, its own server and its own claims. A relative path is `base`'s,
/// the hook's working directory, which is the provider's project.
fn answer(
    provider: &str,
    base: Option<PathBuf>,
    payload: &str,
    claims: impl FnOnce(&Path) -> ClaimSource,
) -> Option<EditVerdict> {
    let (path, session) = from_payload(payload)?;
    let target = base.unwrap_or_default().join(path);
    let start = target.ancestors().find(|a| a.is_dir())?.to_path_buf();
    let repository = crate::repository::Repository::discover(&start).ok()?;
    let root = repository.root().to_path_buf();
    // the root is canonical, so the path is spelled the same way: its existing ancestor
    // canonicalised (discover has just done so without failing), and whatever of it does not
    // exist yet appended (`start` is one of its ancestors, so it is a prefix)
    let rest = target.strip_prefix(&start).unwrap_or(target.as_path());
    let path = start.canonicalize().unwrap_or_default().join(rest);
    let session = session.map(|s| format!("{provider}-{s}"));
    Some(judge(
        &root,
        &path.to_string_lossy(),
        session.as_deref(),
        || claims(&root),
    ))
}

/// The exit code for `verdict`, with the reason and the remedy written to `err` for a
/// refusal: the provider hands that text to the model.
fn report(verdict: Option<&EditVerdict>, err: &mut dyn Write) -> u8 {
    let Some(refused) = verdict.filter(|v| !v.allowed) else {
        return 0;
    };
    let remedy = refused.remedy.as_deref().unwrap_or_default();
    let _ = writeln!(
        err,
        "majordomus guard: {}\n  {remedy}\n  \
         (session.edits_require_claim in .ai/repo/policy.yaml turns this guard off; I2735)",
        refused.detail
    );
    EXIT_REFUSED
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository on the mesh whose policy asks every edit for a claim.
    fn repository() -> tempfile::TempDir {
        let skeleton = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share/skeleton");
        let dir = tempfile::tempdir().unwrap();
        let ai = dir.path().join(".ai");
        std::fs::create_dir_all(ai.join("repo/mesh")).unwrap();
        std::fs::copy(skeleton.join("ai/manifest.yaml"), ai.join("manifest.yaml")).unwrap();
        let policy = std::fs::read_to_string(skeleton.join("policy.yaml"))
            .unwrap()
            .replace("edits_require_claim: false", "edits_require_claim: true");
        std::fs::write(ai.join("repo/policy.yaml"), policy).unwrap();
        std::fs::write(ai.join("repo/mesh/mesh.yaml"), "enabled: true\n").unwrap();
        dir
    }

    fn edit(path: &str) -> String {
        serde_json::json!({ "session_id": "s1", "tool_input": { "file_path": path } }).to_string()
    }

    #[test]
    fn an_unclaimed_edit_is_refused_with_the_reason_and_the_remedy() {
        let dir = repository();
        let root = dir.path().canonicalize().unwrap();
        let verdict = answer(
            "claude-code",
            Some(dir.path().to_path_buf()),
            &edit("docs/new.md"),
            |asked| {
                assert_eq!(asked, root.as_path(), "the checkout of the file is asked");
                ClaimSource::Claims(vec![])
            },
        )
        .expect("a file of the repository");
        assert!(!verdict.allowed);
        assert_eq!(
            verdict.path, "docs/new.md",
            "a file that does not exist yet is placed"
        );
        let mut err = Vec::new();
        assert_eq!(report(Some(&verdict), &mut err), EXIT_REFUSED);
        let text = String::from_utf8(err).unwrap();
        assert!(text.contains("no claim covers docs/new.md"), "{text}");
        assert!(text.contains("--session claude-code-s1"), "{text}");
    }

    #[test]
    fn an_edit_naming_no_file_of_a_repository_is_let_through() {
        let dir = repository();
        let none = |_: &Path| -> ClaimSource { unreachable!("nothing to judge") };
        let base = Some(dir.path().to_path_buf());
        assert!(answer("claude-code", base.clone(), "not json", none).is_none());
        let ls = r#"{"tool_input":{"command":"ls"}}"#;
        assert!(answer("claude-code", base, ls, none).is_none());
        let bare = tempfile::tempdir().unwrap();
        let outside = edit(&bare.path().join("a.md").to_string_lossy());
        assert!(
            answer("claude-code", None, &outside, none).is_none(),
            "no manifest is above it"
        );
        let nowhere = edit("no-such-directory/y.md");
        assert!(answer("claude-code", None, &nowhere, none).is_none());
        let mut err = Vec::new();
        assert_eq!(report(None, &mut err), 0);
        assert!(err.is_empty());
    }

    #[test]
    fn an_allowed_edit_says_nothing() {
        let dir = repository();
        let claims = |_: &Path| ClaimSource::Unavailable("no server".into());
        let verdict = answer(
            "claude-code",
            Some(dir.path().into()),
            &edit("a.md"),
            claims,
        )
        .expect("a file of the repository");
        assert_eq!(verdict.ground, "undecided");
        let mut err = Vec::new();
        assert_eq!(report(Some(&verdict), &mut err), 0);
        assert!(err.is_empty());
    }
}
