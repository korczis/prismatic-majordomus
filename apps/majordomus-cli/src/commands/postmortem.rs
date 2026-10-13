//! `majordomus postmortem`: what turned the trunk's CI red, listed, judged, compared with
//! the trunk's failed runs, matched against a failing log, and drafted.
//!
//! Every subcommand runs a capability of the `postmortems` module through the one executor;
//! what is the command's own is where the input comes from (a file, standard input, the
//! forge's log of a run) and how the answer is set for a person. The exit status is the
//! answer's: `check` exits 10 on any finding, `owed` exits 10 when a failed run is past its
//! grace with no record, and a question the forge did not answer where only the forge
//! could (`draft`, `diagnose --run`) exits 12.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::{CapabilityError, Context};
use crate::cli::{OutputFormat, PostmortemArgs, PostmortemCommand};
use crate::error::{Error, Result};
use crate::postmortem::{forge, DIRECTORY};

/// The exit status of a finding or an owed record: the contract is unmet.
const EXIT_REFUSED: u8 = 10;

/// The exit status of a question the forge did not answer.
const EXIT_UNANSWERED: u8 = 12;

/// Run `majordomus postmortem`.
pub fn run(args: PostmortemArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let (text, code) = answer(&app.context, args.command, args.format, forge::GH)?;
    std::io::stdout()
        .lock()
        .write_all(text.as_bytes())
        .map_err(Error::Transport)?;
    Ok(code)
}

/// What one subcommand prints and exits with. `gh` is the program asked when the command
/// itself fetches a run's log.
fn answer(
    ctx: &Context,
    command: PostmortemCommand,
    format: OutputFormat,
    gh: &str,
) -> Result<(String, u8)> {
    let json = format == OutputFormat::Json;
    match command {
        PostmortemCommand::List => {
            let v = execute(ctx, "list", json!({}))?;
            Ok((if json { pretty(&v) } else { list_text(&v) }, 0))
        }
        PostmortemCommand::Check => {
            let v = execute(ctx, "check", json!({}))?;
            let code = if v["ok"].as_bool() == Some(true) {
                0
            } else {
                EXIT_REFUSED
            };
            Ok((if json { pretty(&v) } else { check_text(&v) }, code))
        }
        PostmortemCommand::Owed {
            branch,
            limit,
            grace_hours,
            since,
            workflows,
            now,
            runs_json,
        } => {
            let runs = runs_json.as_deref().map(read_runs).transpose()?;
            let input = strip_nulls(json!({
                "branch": branch, "limit": limit, "grace_hours": grace_hours, "since": since,
                "workflows": workflows, "now": now, "runs": runs,
            }));
            let v = execute(ctx, "owed", input)?;
            let code = exit_code(&v, 0);
            Ok((if json { pretty(&v) } else { owed_text(&v) }, code))
        }
        PostmortemCommand::Diagnose { text, file, run } => {
            let text = match (text, file, run) {
                (Some(text), _, _) => text,
                (None, Some(file), _) => read_text(&file)?,
                (None, None, Some(run)) => {
                    let root = PathBuf::from(&ctx.index.repository.root);
                    forge::failed_log_with(gh, &root, &run).map_err(|reason| Error::Refused {
                        code: EXIT_UNANSWERED,
                        reason: format!("the failed log of run {run} was not read: {reason}"),
                    })?
                }
                (None, None, None) => read_text(Path::new("-"))?,
            };
            let v = execute(ctx, "diagnose", json!({ "text": text }))?;
            Ok((if json { pretty(&v) } else { diagnose_text(&v) }, 0))
        }
        PostmortemCommand::Draft {
            run,
            runs_json,
            jobs,
        } => {
            let runs = runs_json.as_deref().map(read_runs).transpose()?;
            let input = strip_nulls(json!({ "run": run, "runs": runs, "jobs": jobs }));
            let v = execute(ctx, "draft", input)?;
            let code = exit_code(&v, EXIT_UNANSWERED);
            Ok((if json { pretty(&v) } else { draft_text(&v) }, code))
        }
    }
}

/// The exit status an answer carries, or `otherwise` when it carries none.
fn exit_code(v: &Value, otherwise: u8) -> u8 {
    v["exit_code"]
        .as_u64()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(otherwise)
}

/// The text of `path`, or of standard input when the path is `-`.
fn read_text(path: &Path) -> Result<String> {
    if path == Path::new("-") {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(Error::Transport)?;
        return Ok(text);
    }
    let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The runs a file holds, in the shape `gh run list --json` prints: a list.
fn read_runs(path: &Path) -> Result<Value> {
    let text = read_text(path)?;
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Array(runs)) => Ok(Value::Array(runs)),
        Ok(_) => Err(Error::Refused {
            code: 2,
            reason: format!(
                "{}: not a list of runs: expected what `gh run list --json {}` prints",
                path.display(),
                forge::RUN_FIELDS
            ),
        }),
        Err(e) => Err(Error::Refused {
            code: 2,
            reason: format!("{}: not JSON: {e}", path.display()),
        }),
    }
}

fn text_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("?")
}

fn strings(v: &Value) -> Vec<&str> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// `key n, key n` for a map of counts, or `none`.
fn counts(v: &Value) -> String {
    let parts: Vec<String> = v
        .as_object()
        .into_iter()
        .flatten()
        .map(|(word, n)| format!("{word} {n}"))
        .collect();
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

fn list_text(v: &Value) -> String {
    let records = v["records"].as_array().cloned().unwrap_or_default();
    let mut out = String::new();
    if records.is_empty() {
        out.push_str(&format!("no post-mortem is recorded under {DIRECTORY}\n"));
    } else {
        out.push_str(&format!(
            "{} post-mortem(s)   status: {}   cause: {}\n",
            records.len(),
            counts(&v["by_status"]),
            counts(&v["by_cause"])
        ));
    }
    for r in &records {
        let runs: Vec<String> = r["runs"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|run| format!("{} {}", text_of(run, "workflow"), text_of(run, "id")))
            .collect();
        out.push_str(&format!(
            "{:<9} {:<12} {}  {}  {}\n",
            text_of(r, "status"),
            text_of(&r["cause"], "class"),
            text_of(r, "date"),
            text_of(r, "id"),
            text_of(r, "title"),
        ));
        out.push_str(&format!("  runs       {}\n", runs.join(", ")));
        for (label, key) in [("components", "components"), ("related   ", "related")] {
            let values = strings(&r[key]);
            if !values.is_empty() {
                out.push_str(&format!("  {label} {}\n", values.join(", ")));
            }
        }
    }
    for u in v["unreadable"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "unreadable  {}: {}\n",
            text_of(u, "path"),
            text_of(u, "reason")
        ));
    }
    out
}

fn check_text(v: &Value) -> String {
    let findings = v["findings"].as_array().cloned().unwrap_or_default();
    let mut out = String::new();
    for f in &findings {
        out.push_str(&format!(
            "FAIL  {}  {}  {}\n",
            text_of(f, "record"),
            text_of(f, "code"),
            text_of(f, "message")
        ));
    }
    out.push_str(&format!(
        "{} post-mortem(s) judged, {} finding(s)\n",
        v["records"].as_u64().unwrap_or(0),
        findings.len()
    ));
    out
}

/// An age for a person: minutes, hours or days, whichever is the largest that fits.
fn age(seconds: Option<u64>) -> String {
    match seconds {
        None => "age unknown".to_string(),
        Some(s) if s < 3600 => format!("{}m ago", s / 60),
        Some(s) if s < 48 * 3600 => format!("{}h ago", s / 3600),
        Some(s) => format!("{}d ago", s / 86_400),
    }
}

fn owed_text(v: &Value) -> String {
    if v["observed"].as_bool() != Some(true) {
        return format!(
            "the runs of {} were not observed, so nothing is known to be owed: {}\n",
            text_of(v, "branch"),
            v["reason"].as_str().unwrap_or("no reason was given")
        );
    }
    let mut out = String::new();
    let owed = v["owed"].as_array().cloned().unwrap_or_default();
    for o in &owed {
        let standing = if o["within_grace"].as_bool() == Some(true) {
            "owed"
        } else {
            "LATE"
        };
        out.push_str(&format!(
            "{standing:<5} {} {}  {}  {}  {}\n",
            text_of(o, "workflow"),
            text_of(o, "id"),
            text_of(o, "conclusion"),
            age(o["age_seconds"].as_u64()),
            text_of(o, "url"),
        ));
    }
    out.push_str(&format!(
        "{} failed run(s) of {} since {}: {} answered, {} owed, {} past the {}h grace\n",
        v["failed"].as_u64().unwrap_or(0),
        text_of(v, "branch"),
        text_of(v, "since"),
        v["answered"].as_u64().unwrap_or(0),
        owed.len(),
        v["past_grace"].as_u64().unwrap_or(0),
        v["grace_hours"].as_u64().unwrap_or(0),
    ));
    if !owed.is_empty() {
        out.push_str("draft a record: majordomus postmortem draft --run <id>\n");
    }
    out
}

fn diagnose_text(v: &Value) -> String {
    let matches = v["matches"].as_array().cloned().unwrap_or_default();
    if matches.is_empty() {
        return format!(
            "no recorded failure shows its signals in this text ({} post-mortem(s), {} signal(s) searched)\n",
            v["records"].as_u64().unwrap_or(0),
            v["signals"].as_u64().unwrap_or(0)
        );
    }
    let mut out = String::new();
    for m in &matches {
        out.push_str(&format!(
            "{}  {}  {}\n",
            text_of(m, "id"),
            text_of(m, "status"),
            text_of(m, "title")
        ));
        out.push_str(&format!(
            "  cause      {}: {}\n",
            text_of(&m["cause"], "class"),
            text_of(&m["cause"], "summary")
        ));
        out.push_str(&format!(
            "  signals    {}\n",
            strings(&m["signals"]).join(", ")
        ));
        for (label, key) in [
            ("components", "components"),
            ("fixed by  ", "fixed_by"),
            ("prevention", "prevention"),
            ("related   ", "related"),
        ] {
            let values = strings(&m[key]);
            if !values.is_empty() {
                out.push_str(&format!("  {label} {}\n", values.join(", ")));
            }
        }
        out.push_str(&format!("  record     {}\n", text_of(m, "path")));
    }
    out
}

fn draft_text(v: &Value) -> String {
    if v["observed"].as_bool() != Some(true) {
        return format!(
            "the run was not observed, so nothing was drafted: {}\n",
            v["reason"].as_str().unwrap_or("no reason was given")
        );
    }
    v["text"].as_str().unwrap_or_default().to_string()
}

fn strip_nulls(mut v: Value) -> Value {
    if let Some(m) = v.as_object_mut() {
        m.retain(|_, x| !x.is_null());
    }
    v
}

fn pretty(v: &Value) -> String {
    let mut text = serde_json::to_string_pretty(v).unwrap_or_default();
    text.push('\n');
    text
}

/// Run the capability exposed as `majordomus postmortem <word>`. What the caller got wrong
/// is a usage error, what is not there is a missing artifact, and anything else is this
/// executable's own failure.
fn execute(ctx: &Context, word: &str, input: Value) -> Result<Value> {
    let words = ["postmortem".to_string(), word.to_string()];
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!("no capability is exposed as `majordomus postmortem {word}`"),
        })?;
    ctx.execute(id, input).map_err(|e| match e {
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Refused { code: 2, reason }
        }
        CapabilityError::NotFound(reason) => Error::Refused {
            code: EXIT_UNANSWERED,
            reason,
        },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::builtin::postmortems::tests::{record_file, recorded, write};

    const NO_GH: &str = "majordomus-no-such-gh-binary";

    fn runs_file(dir: &Path) -> PathBuf {
        let path = dir.join("runs.json");
        let run = |id: u64, conclusion: &str, at: &str| {
            json!({"databaseId": id, "workflowName": "validate", "conclusion": conclusion,
                "headSha": "0123456789abcdef0123456789abcdef01234567", "createdAt": at,
                "updatedAt": at, "headBranch": "master",
                "url": format!("https://forge.example/o/r/actions/runs/{id}")})
        };
        std::fs::write(
            &path,
            json!([
                run(100, "failure", "2026-10-13T05:00:00Z"),
                run(400, "failure", "2026-10-15T05:00:00Z"),
                run(401, "timed_out", "2026-10-15T11:30:00Z"),
                run(402, "cancelled", "2026-10-15T07:00:00Z"),
            ])
            .to_string(),
        )
        .unwrap();
        path
    }

    fn owed_at(now: &str, runs: &Path) -> PostmortemCommand {
        PostmortemCommand::Owed {
            branch: None,
            limit: None,
            grace_hours: None,
            since: None,
            workflows: Vec::new(),
            now: Some(now.into()),
            runs_json: Some(runs.to_path_buf()),
        }
    }

    #[test]
    fn the_records_are_listed_and_judged_for_a_person_and_for_a_script() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let (text, code) =
            answer(&ctx, PostmortemCommand::List, OutputFormat::Text, NO_GH).unwrap();
        assert_eq!(code, 0);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "2 post-mortem(s)   status: draft 1, resolved 1   cause: race 1, undetermined 1"
        );
        assert_eq!(
            lines[1],
            "draft     undetermined 2026-10-14  pm-20261014-validate-200  validate run 200 ended in failure"
        );
        assert_eq!(lines[2], "  runs       validate 200");
        assert!(lines[3].starts_with("resolved  race         2026-10-13  pm-20261013-lease"));
        assert_eq!(
            lines[5],
            "  components file:docs/DOC_0.md, rule:project.rule-0@1"
        );
        assert_eq!(lines[6], "  related    rule:project.rule-1@1");
        let (text, _) = answer(&ctx, PostmortemCommand::List, OutputFormat::Json, NO_GH).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["count"], 2);

        let (text, code) =
            answer(&ctx, PostmortemCommand::Check, OutputFormat::Text, NO_GH).unwrap();
        assert_eq!(
            (text.as_str(), code),
            ("2 post-mortem(s) judged, 0 finding(s)\n", 0)
        );

        // a resolved record that names no fix: said, with its code, and exit 10
        write(
            &repo,
            "pm-20261015-unfixed",
            &record_file(
                "pm-20261015-unfixed",
                "Resolved, and nothing says by what",
                "status: resolved\ndate: 2026-10-15\nruns:\n  - workflow: validate\n    id: \"300\"\n\
                 \x20   commit: \"abcdef3\"\n    jobs: [\"rust (1)\"]\n\
                 cause:\n  class: defect\n  summary: \"A defect.\"\n\
                 components:\n  - file:docs/DOC_1.md\n\
                 signals:\n  - id: lint\n    text: \"shell-lint: finding\"\n",
            ),
        );
        let ctx = repo.context().expect("a context");
        let (text, code) =
            answer(&ctx, PostmortemCommand::Check, OutputFormat::Text, NO_GH).unwrap();
        assert_eq!(code, EXIT_REFUSED);
        assert_eq!(
            text,
            "FAIL  pm-20261015-unfixed  resolved-without-fix  a resolved record names what removed the cause in `fixed_by`\n\
             3 post-mortem(s) judged, 1 finding(s)\n"
        );
        let (text, code) =
            answer(&ctx, PostmortemCommand::Check, OutputFormat::Json, NO_GH).unwrap();
        assert_eq!(code, EXIT_REFUSED);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["findings"][0]["code"], "resolved-without-fix");

        // nothing recorded is said in words, and a file that is no record is listed apart
        assert_eq!(
            list_text(&json!({"records": [], "unreadable": [{"path": "p.md", "reason": "r"}]})),
            "no post-mortem is recorded under .ai/repo/post-mortems/\nunreadable  p.md: r\n"
        );
        assert_eq!(counts(&json!({})), "none");
    }

    #[test]
    fn the_owed_runs_are_read_from_a_file_and_the_exit_says_whether_one_is_late() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let dir = tempfile::tempdir().unwrap();
        let runs = runs_file(dir.path());

        let (text, code) = answer(
            &ctx,
            owed_at("2026-10-15T12:00:00Z", &runs),
            OutputFormat::Text,
            NO_GH,
        )
        .unwrap();
        assert_eq!(code, 0, "{text}");
        assert_eq!(
            text,
            "owed  validate 401  timed_out  30m ago  https://forge.example/o/r/actions/runs/401\n\
             owed  validate 400  failure  7h ago  https://forge.example/o/r/actions/runs/400\n\
             3 failed run(s) of master since 2026-10-13: 1 answered, 2 owed, 0 past the 24h grace\n\
             draft a record: majordomus postmortem draft --run <id>\n"
        );

        let (text, code) = answer(
            &ctx,
            owed_at("2026-10-18T12:00:00Z", &runs),
            OutputFormat::Text,
            NO_GH,
        )
        .unwrap();
        assert_eq!(code, EXIT_REFUSED);
        assert!(
            text.starts_with("LATE  validate 401  timed_out  3d ago  "),
            "{text}"
        );
        assert!(text.contains("2 owed, 2 past the 24h grace"), "{text}");

        let (text, code) = answer(
            &ctx,
            owed_at("2026-10-18T12:00:00Z", &runs),
            OutputFormat::Json,
            NO_GH,
        )
        .unwrap();
        assert_eq!(code, EXIT_REFUSED);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["source"], "input");
        assert_eq!(v["owed"][1]["id"], "400");

        // every option reaches the capability
        let narrowed = PostmortemCommand::Owed {
            branch: Some("master".into()),
            limit: Some(5),
            grace_hours: Some(1),
            since: Some("2026-10-15T06:00:00Z".into()),
            workflows: vec!["validate".into()],
            now: Some("2026-10-15T12:00:00Z".into()),
            runs_json: Some(runs.clone()),
        };
        let (text, code) = answer(&ctx, narrowed, OutputFormat::Json, NO_GH).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            (v["failed"].as_u64(), v["grace_hours"].as_u64()),
            (Some(1), Some(1))
        );
        assert_eq!(v["since"], "2026-10-15T06:00:00Z");
        assert_eq!(code, 0, "thirty minutes is inside an hour's grace");

        // nothing owed is one line, and an unobserved forge is said and owes nothing
        let answered = json!({"observed": true, "branch": "master", "since": "2026-10-13",
            "failed": 1, "answered": 1, "owed": [], "past_grace": 0, "grace_hours": 24});
        assert_eq!(
            owed_text(&answered),
            "1 failed run(s) of master since 2026-10-13: 1 answered, 0 owed, 0 past the 24h grace\n"
        );
        let unseen = json!({"observed": false, "branch": "master", "reason": "gh could not run"});
        assert_eq!(
            owed_text(&unseen),
            "the runs of master were not observed, so nothing is known to be owed: gh could not run\n"
        );
        assert!(owed_text(&json!({})).ends_with("no reason was given\n"));
        assert_eq!(exit_code(&json!({}), 7), 7);
        assert_eq!(age(None), "age unknown");
        assert_eq!(age(Some(59)), "0m ago");
        assert_eq!(age(Some(47 * 3600 + 3599)), "47h ago");
        assert_eq!(age(Some(48 * 3600)), "2d ago");

        // a file that is not a list of runs is a usage error, with the file named
        for (content, why) in [("{}", "not a list of runs"), ("runs", "not JSON")] {
            std::fs::write(&runs, content).unwrap();
            let refused = answer(
                &ctx,
                owed_at("2026-10-15T12:00:00Z", &runs),
                OutputFormat::Text,
                NO_GH,
            )
            .unwrap_err();
            assert_eq!(refused.exit_code(), 2);
            assert!(refused.to_string().contains(why), "{refused}");
            assert!(refused.to_string().contains("runs.json"), "{refused}");
        }
        let missing = read_runs(&dir.path().join("no-such.json")).unwrap_err();
        assert_eq!(missing.exit_code(), 13);
    }

    #[test]
    fn a_failure_is_diagnosed_from_text_a_file_or_a_run_s_log() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let dir = tempfile::tempdir().unwrap();
        let diagnose = |text: Option<&str>, file: Option<PathBuf>, run: Option<&str>| {
            PostmortemCommand::Diagnose {
                text: text.map(str::to_string),
                file,
                run: run.map(str::to_string),
            }
        };

        let (text, code) = answer(
            &ctx,
            diagnose(Some("error: The Lease Was Lost"), None, None),
            OutputFormat::Text,
            NO_GH,
        )
        .unwrap();
        assert_eq!(code, 0);
        assert_eq!(
            text,
            "pm-20261013-lease  resolved  Two writers lost the lease\n\
             \x20 cause      race: Two writers, one file.\n\
             \x20 signals    lease-lost\n\
             \x20 components file:docs/DOC_0.md, rule:project.rule-0@1\n\
             \x20 fixed by   pr:1078\n\
             \x20 prevention test:docs/DOC_0.md\n\
             \x20 related    rule:project.rule-1@1\n\
             \x20 record     .ai/repo/post-mortems/pm-20261013-lease.md\n"
        );

        let log = dir.path().join("failed.log");
        std::fs::write(&log, b"step\nprocess ended with exit code 101\n\xff\n").unwrap();
        let (text, _) = answer(
            &ctx,
            diagnose(None, Some(log), None),
            OutputFormat::Json,
            NO_GH,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["matches"][0]["signals"], json!(["exit"]));

        let (text, code) = answer(
            &ctx,
            diagnose(Some("nothing anyone recorded"), None, None),
            OutputFormat::Text,
            NO_GH,
        )
        .unwrap();
        assert_eq!(code, 0, "no match is an answer");
        assert_eq!(
            text,
            "no recorded failure shows its signals in this text (2 post-mortem(s), 2 signal(s) searched)\n"
        );

        // a run's log that the forge did not give is exit 12, with the reason
        let unread = answer(
            &ctx,
            diagnose(None, None, Some("11")),
            OutputFormat::Text,
            NO_GH,
        )
        .unwrap_err();
        assert_eq!(unread.exit_code(), EXIT_UNANSWERED);
        assert!(unread
            .to_string()
            .starts_with("the failed log of run 11 was not read: "));
        // and blank text is a usage error, not an empty answer
        let blank = answer(
            &ctx,
            diagnose(Some(" "), None, None),
            OutputFormat::Text,
            NO_GH,
        )
        .unwrap_err();
        assert_eq!(blank.exit_code(), 2);
        let absent = answer(
            &ctx,
            diagnose(None, Some(dir.path().join("no-such.log")), None),
            OutputFormat::Text,
            NO_GH,
        )
        .unwrap_err();
        assert_eq!(absent.exit_code(), 13);
    }

    #[test]
    #[cfg(unix)]
    fn a_run_s_failed_log_is_fetched_and_diagnosed() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let dir = tempfile::tempdir().unwrap();
        let gh = forge::tests::fake_gh(dir.path());
        std::fs::write(
            dir.path().join("failed.log"),
            "rust (3)\tcargo test\terror: the lease was lost\n",
        )
        .unwrap();
        let (text, code) = answer(
            &ctx,
            PostmortemCommand::Diagnose {
                text: None,
                file: None,
                run: Some("11".into()),
            },
            OutputFormat::Text,
            gh.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(code, 0);
        assert!(text.starts_with("pm-20261013-lease  resolved  "), "{text}");
        let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
        assert_eq!(calls.trim(), "run view 11 --log-failed");
    }

    #[test]
    fn a_draft_is_printed_from_a_run_in_a_file_and_never_written() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let dir = tempfile::tempdir().unwrap();
        let runs = runs_file(dir.path());
        let draft = |run: &str| PostmortemCommand::Draft {
            run: run.into(),
            runs_json: Some(runs.clone()),
            jobs: vec!["rust (3)".into()],
        };
        let (text, code) = answer(&ctx, draft("400"), OutputFormat::Text, NO_GH).unwrap();
        assert_eq!(code, 0);
        assert!(text.starts_with("---\nschema: post-mortem/v1\nid: pm-20261015-validate-400\n"));
        assert!(
            text.ends_with("## Prevention\n\nNot yet established.\n"),
            "{text}"
        );
        assert!(!repo
            .root()
            .join(".ai/repo/post-mortems/pm-20261015-validate-400.md")
            .exists());

        let (text, code) = answer(&ctx, draft("100"), OutputFormat::Json, NO_GH).unwrap();
        assert_eq!(code, 0);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["id"], "pm-20261013-validate-100");
        assert_eq!(v["answered_by"], json!(["pm-20261013-lease"]));

        // a run the file does not hold is a missing artifact; a run that is no run is usage
        assert_eq!(
            answer(&ctx, draft("999"), OutputFormat::Text, NO_GH)
                .unwrap_err()
                .exit_code(),
            EXIT_UNANSWERED
        );
        assert_eq!(
            answer(&ctx, draft("--web"), OutputFormat::Text, NO_GH)
                .unwrap_err()
                .exit_code(),
            2
        );
        // and without a file the forge is asked, which here cannot answer: said, exit 12
        let unseen = json!({"observed": false, "reason": "gh could not run", "exit_code": 12});
        assert_eq!(
            draft_text(&unseen),
            "the run was not observed, so nothing was drafted: gh could not run\n"
        );
        assert_eq!(exit_code(&unseen, 0), EXIT_UNANSWERED);
        assert!(draft_text(&json!({})).ends_with("no reason was given\n"));
    }

    /// `postmortem`, run whole against a repository on disk: the records are read through
    /// the application as every command reads them.
    #[test]
    fn the_command_runs_whole_against_a_repository() {
        let repo = recorded();
        let args = |command: PostmortemCommand, format: OutputFormat| PostmortemArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(repo.root().to_path_buf()),
                discovery: crate::cli::DiscoveryMode::Filesystem,
                strict: false,
                share: Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
            },
            command,
            format,
        };
        assert_eq!(
            run(args(PostmortemCommand::List, OutputFormat::Text)).unwrap(),
            0
        );
        assert_eq!(
            run(args(PostmortemCommand::Check, OutputFormat::Json)).unwrap(),
            0
        );
        let unknown = execute(&repo.context().unwrap(), "no-such-word", json!({})).unwrap_err();
        assert!(unknown
            .to_string()
            .contains("majordomus postmortem no-such-word"));
    }
}
