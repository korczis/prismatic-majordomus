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
use crate::guard::{
    edit_guard_mode, from_payload, judge, server_claims, ClaimSource, EditVerdict,
};
use crate::policy::EditGuardMode;

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
    let answered = answer(&args.provider, base, &payload, server_claims);
    if let Some(a) = answered.as_ref().filter(|a| a.reported()) {
        note_unclaimed(a);
    }
    Ok(report(
        answered.as_ref(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ))
}

/// One edit judged: the verdict, the checkout it belongs to, the writing session, and what
/// that checkout's policy does with an edit no claim covers.
struct Answer {
    verdict: EditVerdict,
    root: PathBuf,
    session: Option<String>,
    mode: EditGuardMode,
}

impl Answer {
    /// Whether the edit is unclaimed and the policy reports it rather than refusing it.
    fn reported(&self) -> bool {
        !self.verdict.allowed && self.mode == EditGuardMode::Advisory
    }
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
) -> Option<Answer> {
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
    let verdict = judge(
        &root,
        &path.to_string_lossy(),
        session.as_deref(),
        || claims(&root),
    );
    // the mode is read only for an edit nothing covers: every other edit, the common case
    // this hook runs before, costs what it did before the mode existed (I3322: under ~50 ms)
    let mode = if verdict.allowed {
        EditGuardMode::default()
    } else {
        edit_guard_mode(&root)
    };
    Some(Answer {
        verdict,
        root,
        session,
        mode,
    })
}

/// The exit code for `answered`. A refusal writes the reason and the remedy to `err`, which
/// the provider hands to the model, and exits 2. An unclaimed edit the policy only reports
/// (`session.edit_guard: advisory`, I3322) exits 0 with the same words as Claude Code's
/// `additionalContext` on `out`: the model reads them beside the tool's result, and with no
/// permission decision in the answer the person's permission flow is the one it would be
/// without the hook (code.claude.com/docs/en/hooks). Anything else says nothing.
fn report(answered: Option<&Answer>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let Some(a) = answered.filter(|a| !a.verdict.allowed) else {
        return 0;
    };
    let remedy = a.verdict.remedy.as_deref().unwrap_or_default();
    if a.mode == EditGuardMode::Advisory {
        let note = format!(
            "majordomus guard (advisory): {}\n  {remedy}\n  The edit goes ahead: \
             session.edit_guard is advisory in .ai/repo/policy.yaml; refuse would stop it (I3322).",
            a.verdict.detail
        );
        let answer = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "additionalContext": note,
            }
        });
        let _ = writeln!(out, "{answer}");
        return 0;
    }
    let _ = writeln!(
        err,
        "majordomus guard: {}\n  {remedy}\n  \
         (session.edits_require_claim in .ai/repo/policy.yaml turns this guard off; I2735)",
        a.verdict.detail
    );
    EXIT_REFUSED
}

/// Where an unclaimed edit the policy only reports is noted, one JSON line each, for the
/// claim-coverage report doctor prints: local state, never shared, never committed.
pub const UNCLAIMED_LOG: &str = ".ai/local/state/guard/unclaimed.jsonl";

/// Note an unclaimed, reported edit in its checkout's [`UNCLAIMED_LOG`]. A note that cannot
/// be written is dropped: the edit is let through either way, and a hook that failed on its
/// own bookkeeping would be a hook in the way.
fn note_unclaimed(a: &Answer) {
    let line = serde_json::json!({
        "at": crate::ledger::now(),
        "session": a.session.as_deref().unwrap_or("-"),
        "path": a.verdict.path,
    });
    let file = a.root.join(UNCLAIMED_LOG);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
    {
        let _ = writeln!(f, "{line}");
    }
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
        let answered = answer(
            "claude-code",
            Some(dir.path().to_path_buf()),
            &edit("docs/new.md"),
            |asked| {
                assert_eq!(asked, root.as_path(), "the checkout of the file is asked");
                ClaimSource::Claims(vec![])
            },
        )
        .expect("a file of the repository");
        assert!(!answered.verdict.allowed);
        assert_eq!(
            answered.verdict.path, "docs/new.md",
            "a file that does not exist yet is placed"
        );
        assert_eq!(answered.mode, EditGuardMode::Refuse, "refusal is the default");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(report(Some(&answered), &mut out, &mut err), EXIT_REFUSED);
        assert!(out.is_empty(), "a refusal answers on stderr alone");
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
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(report(None, &mut out, &mut err), 0);
        assert!(out.is_empty() && err.is_empty());
    }

    #[test]
    fn an_allowed_edit_says_nothing() {
        let dir = repository();
        let claims = |_: &Path| ClaimSource::Unavailable("no server".into());
        let answered = answer(
            "claude-code",
            Some(dir.path().into()),
            &edit("a.md"),
            claims,
        )
        .expect("a file of the repository");
        assert_eq!(answered.verdict.ground, "undecided");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(report(Some(&answered), &mut out, &mut err), 0);
        assert!(out.is_empty() && err.is_empty());
    }

    /// An unclaimed edit in a repository whose policy declares `session.edit_guard: advisory`
    /// is let through (exit 0) with the reason and the remedy as Claude Code's
    /// `additionalContext`, carrying no permission decision, and is noted for the
    /// claim-coverage report (I3322).
    #[test]
    fn an_unclaimed_edit_under_an_advisory_guard_is_reported_and_let_through() {
        let dir = repository();
        let policy = dir.path().join(".ai/repo/policy.yaml");
        let text = std::fs::read_to_string(&policy).unwrap().replace(
            "edits_require_claim: true",
            "edits_require_claim: true\n  edit_guard: advisory",
        );
        std::fs::write(&policy, text).unwrap();
        let answered = answer(
            "claude-code",
            Some(dir.path().to_path_buf()),
            &edit("docs/new.md"),
            |_| ClaimSource::Claims(vec![]),
        )
        .expect("a file of the repository");
        assert!(!answered.verdict.allowed, "the edit is still unclaimed");
        assert_eq!(answered.mode, EditGuardMode::Advisory);
        assert!(answered.reported());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(report(Some(&answered), &mut out, &mut err), 0, "let through");
        assert!(err.is_empty(), "nothing on stderr, which the model never sees on exit 0");
        let json: serde_json::Value = serde_json::from_slice(&out).expect("one JSON answer");
        let hook = &json["hookSpecificOutput"];
        assert_eq!(hook["hookEventName"], "PreToolUse");
        assert!(
            hook.get("permissionDecision").is_none(),
            "no decision: the permission flow is the one without the hook: {json}"
        );
        let note = hook["additionalContext"].as_str().expect("the note");
        assert!(note.contains("no claim covers docs/new.md"), "{note}");
        assert!(note.contains("--session claude-code-s1"), "{note}");
        note_unclaimed(&answered);
        let log = std::fs::read_to_string(answered.root.join(UNCLAIMED_LOG)).expect("noted");
        let line: serde_json::Value = serde_json::from_str(log.trim()).expect("one line");
        assert_eq!(line["session"], "claude-code-s1");
        assert_eq!(line["path"], "docs/new.md");
    }
}
