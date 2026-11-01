//! `majordomus guard`: the edit guard on the command line (I2735).
//!
//! `guard hook` is what a provider's pre-tool hook runs. It reads the provider's payload from
//! stdin, answers `guard.edit` for the path it names without building the index — it runs
//! before every edit — and says the answer in the one code the provider reads as a refusal:
//! 2, with the reason and the remedy on stderr, which the provider hands to the model. Every
//! other outcome is 0, an undecidable one included: a guard that refused because it could
//! not read its own inputs would stop every edit of a checkout nobody has served.
//!
//! Under `session.edit_guard_mode: advisory` (I3322) an edit no claim covers is one of those
//! other outcomes, and it is not let through in silence: the reason and the remedy are said on
//! stderr, and one line — when, which session, which path — is appended to
//! [`UNCLAIMED_LOG`] in the checkout the file belongs to. `majordomus doctor` reads those
//! lines across the repository's checkouts and names the sessions that edited unclaimed
//! recently; that list is what says when refusal can be turned on (I3323).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::cli::{GuardArgs, GuardCommand, GuardHookArgs};
use crate::error::Result;
use crate::guard::{from_payload, judge, server_claims, ClaimSource, EditVerdict};

/// The code a provider's pre-tool hook reads as "do not run this tool".
const EXIT_REFUSED: u8 = 2;

/// Where an advisory guard records an unclaimed edit, relative to the checkout's root: one
/// tab-separated line per edit — the time (UTC, RFC 3339), the writing session
/// (`<provider>-<session id>`, `-` when the payload named none) and the repository-relative
/// path. Beside the shim's own `skipped.log`, under the checkout's local state, because it is
/// a fact about this checkout's sessions and nobody else's to commit.
const UNCLAIMED_LOG: &str = ".ai/local/state/guard/unclaimed.log";

/// One judged edit: the checkout it belongs to, the session that asked, and the verdict.
struct Judged {
    root: PathBuf,
    session: Option<String>,
    verdict: EditVerdict,
}

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
    let judged = answer(&args.provider, base, &payload, server_claims);
    let mut err = std::io::stderr();
    if let Some(j) = judged.as_ref().filter(|j| advised(&j.verdict)) {
        record(
            j,
            &crate::peers::rfc3339(std::time::SystemTime::now()),
            &mut err,
        );
    }
    Ok(report(judged.as_ref().map(|j| &j.verdict), &mut err))
}

/// Whether `verdict` is an unclaimed edit an advisory guard let through.
fn advised(verdict: &EditVerdict) -> bool {
    verdict.allowed && verdict.ground == "unclaimed"
}

/// Append the advisory line for `j` to its checkout's [`UNCLAIMED_LOG`]. A line that cannot
/// be written is said on `err` and costs the edit nothing: the record is the report's input,
/// and an edit is never stopped for the sake of a report about it.
fn record(j: &Judged, at: &str, err: &mut dyn Write) {
    let log = j.root.join(UNCLAIMED_LOG);
    // a field is one line and holds no tab, whatever a payload put in it
    let field = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    let line = format!(
        "{at}\t{}\t{}\n",
        field(j.session.as_deref().unwrap_or("-")),
        field(&j.verdict.path)
    );
    let written = log
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log)
        })
        .and_then(|mut f| f.write_all(line.as_bytes()));
    if let Err(e) = written {
        let _ = writeln!(
            err,
            "majordomus guard: the unclaimed edit was not recorded in {}: {e}",
            log.display()
        );
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
) -> Option<Judged> {
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
    let verdict = judge(&root, &path.to_string_lossy(), session.as_deref(), || {
        claims(&root)
    });
    Some(Judged {
        root,
        session,
        verdict,
    })
}

/// The exit code for `verdict`, with the reason and the remedy written to `err` for a
/// refusal — the provider hands that text to the model — and for an unclaimed edit an
/// advisory guard let through, which exits 0 and is said all the same.
fn report(verdict: Option<&EditVerdict>, err: &mut dyn Write) -> u8 {
    let Some(unclaimed) = verdict.filter(|v| !v.allowed || advised(v)) else {
        return 0;
    };
    let remedy = unclaimed.remedy.as_deref().unwrap_or_default();
    if unclaimed.allowed {
        let _ = writeln!(
            err,
            "majordomus guard (advisory): {}\n  {remedy}\n  \
             (recorded in {UNCLAIMED_LOG}; session.edit_guard_mode: refuse in \
             .ai/repo/policy.yaml would have refused it; I3322)",
            unclaimed.detail
        );
        return 0;
    }
    let _ = writeln!(
        err,
        "majordomus guard: {}\n  {remedy}\n  \
         (session.edits_require_claim in .ai/repo/policy.yaml turns this guard off; I2735)",
        unclaimed.detail
    );
    EXIT_REFUSED
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository on the mesh whose policy asks every edit for a claim, with the guard in
    /// `mode`.
    fn repository_in(mode: &str) -> tempfile::TempDir {
        let skeleton = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share/skeleton");
        let dir = tempfile::tempdir().unwrap();
        let ai = dir.path().join(".ai");
        std::fs::create_dir_all(ai.join("repo/mesh")).unwrap();
        std::fs::copy(skeleton.join("ai/manifest.yaml"), ai.join("manifest.yaml")).unwrap();
        let policy = std::fs::read_to_string(skeleton.join("policy.yaml"))
            .unwrap()
            .replace("edits_require_claim: false", "edits_require_claim: true")
            .replace(
                "edit_guard_mode: advisory",
                &format!("edit_guard_mode: {mode}"),
            );
        std::fs::write(ai.join("repo/policy.yaml"), policy).unwrap();
        std::fs::write(
            ai.join("repo/mesh/mesh.yaml"),
            "enabled: true\ntrust:\n  policy: tofu\n",
        )
        .unwrap();
        dir
    }

    /// The same, refusing.
    fn repository() -> tempfile::TempDir {
        repository_in("refuse")
    }

    fn edit(path: &str) -> String {
        serde_json::json!({ "session_id": "s1", "tool_input": { "file_path": path } }).to_string()
    }

    #[test]
    fn an_unclaimed_edit_is_refused_with_the_reason_and_the_remedy() {
        let dir = repository();
        let root = dir.path().canonicalize().unwrap();
        let judged = answer(
            "claude-code",
            Some(dir.path().to_path_buf()),
            &edit("docs/new.md"),
            |asked| {
                assert_eq!(asked, root.as_path(), "the checkout of the file is asked");
                ClaimSource::Claims(vec![])
            },
        )
        .expect("a file of the repository");
        let verdict = &judged.verdict;
        assert!(!verdict.allowed);
        assert!(
            !advised(verdict),
            "a refusal is not an advisory let-through"
        );
        assert_eq!(
            verdict.path, "docs/new.md",
            "a file that does not exist yet is placed"
        );
        let mut err = Vec::new();
        assert_eq!(report(Some(verdict), &mut err), EXIT_REFUSED);
        let text = String::from_utf8(err).unwrap();
        assert!(text.contains("no claim covers docs/new.md"), "{text}");
        assert!(text.contains("--session claude-code-s1"), "{text}");
    }

    #[test]
    fn an_advisory_guard_lets_the_unclaimed_edit_through_says_so_and_records_it() {
        let dir = repository_in("advisory");
        let judged = answer(
            "claude-code",
            Some(dir.path().to_path_buf()),
            &edit("docs/new.md"),
            |_| ClaimSource::Claims(vec![]),
        )
        .expect("a file of the repository");
        assert!(advised(&judged.verdict), "{:?}", judged.verdict);
        assert_eq!(judged.session.as_deref(), Some("claude-code-s1"));
        let mut err = Vec::new();
        assert_eq!(report(Some(&judged.verdict), &mut err), 0);
        let text = String::from_utf8(err).unwrap();
        assert!(text.contains("majordomus guard (advisory)"), "{text}");
        assert!(text.contains("no claim covers docs/new.md"), "{text}");
        assert!(
            text.contains("majordomus start"),
            "the remedy is said: {text}"
        );
        let mut err = Vec::new();
        record(&judged, "2026-11-01T10:00:00Z", &mut err);
        record(&judged, "2026-11-01T10:05:00Z", &mut err);
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
        let log = std::fs::read_to_string(judged.root.join(UNCLAIMED_LOG)).unwrap();
        assert_eq!(
            log,
            "2026-11-01T10:00:00Z\tclaude-code-s1\tdocs/new.md\n\
             2026-11-01T10:05:00Z\tclaude-code-s1\tdocs/new.md\n",
            "one line per edit, appended"
        );
    }

    #[test]
    fn a_record_that_cannot_be_written_is_said_and_costs_the_edit_nothing() {
        let dir = repository_in("advisory");
        let judged = answer(
            "claude-code",
            Some(dir.path().to_path_buf()),
            &edit("docs/new.md"),
            |_| ClaimSource::Claims(vec![]),
        )
        .expect("a file of the repository");
        // the directory the log belongs in is a file
        let state = judged.root.join(".ai/local/state");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("guard"), "not a directory").unwrap();
        let mut err = Vec::new();
        record(&judged, "2026-11-01T10:00:00Z", &mut err);
        let text = String::from_utf8(err).unwrap();
        assert!(text.contains("was not recorded"), "{text}");
        assert_eq!(report(Some(&judged.verdict), &mut Vec::<u8>::new()), 0);
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
        let judged = answer(
            "claude-code",
            Some(dir.path().into()),
            &edit("a.md"),
            claims,
        )
        .expect("a file of the repository");
        assert_eq!(judged.verdict.ground, "undecided");
        assert!(!advised(&judged.verdict));
        let mut err = Vec::new();
        assert_eq!(report(Some(&judged.verdict), &mut err), 0);
        assert!(err.is_empty());
    }
}
