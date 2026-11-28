//! `majordomus commit`: the plan over the working tree, the scope vocabulary, and the
//! verdict on one message.
//!
//! Every answer comes from the registry's own capabilities — `commit.plan`,
//! `commit.scopes`, `commit.validate` — so that what a person reads here, what an MCP
//! client is handed, what the HTTP route answers and what the `commit-msg` hook refuses
//! with are one computation rendered four ways. This module renders; it decides nothing.

use std::io::{Read, Write};

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{CommitArgs, CommitCommand, OutputFormat};
use crate::error::{Error, Result};

/// The exit code when `validate` finds an error, and when a plan cannot be made.
///
/// 10 is this executable's "contract unmet", which is what a message that does not satisfy
/// the commit policy is. The `commit-msg` hook passes it straight through to git, which
/// aborts the commit.
pub const EXIT_UNMET: u8 = 10;

/// Run `majordomus commit`.
pub fn run(args: CommitArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.command.unwrap_or(CommitCommand::Plan) {
        CommitCommand::Plan => {
            let v = execute(ctx, &["commit", "plan"], json!({}))?;
            render_plan(&mut out, &v, args.format)?;
            Ok(0)
        }
        CommitCommand::Scopes => {
            let v = execute(ctx, &["commit", "scopes"], json!({}))?;
            render_scopes(&mut out, &v, args.format)?;
            Ok(0)
        }
        CommitCommand::History { range } => {
            let v = execute(
                ctx,
                &["commit", "history"],
                json!({ "range": range.unwrap_or_else(|| "HEAD".into()) }),
            )?;
            render_history(&mut out, &v, args.format)?;
            Ok(if v["failing"].as_u64().unwrap_or(0) == 0 {
                0
            } else {
                EXIT_UNMET
            })
        }
        CommitCommand::Validate { file, paths, rev } => {
            let message = read_message(&app, file.as_deref(), rev.as_deref())?;
            let mut input = json!({ "message": message });
            if !paths.is_empty() {
                input["paths"] = json!(paths);
            }
            let v = execute(ctx, &["commit", "validate"], input)?;
            render_verdict(&mut out, &v, args.format)?;
            Ok(if v["passed"].as_bool() == Some(true) {
                0
            } else {
                EXIT_UNMET
            })
        }
        CommitCommand::Reword {
            range,
            messages,
            trunk,
            supersedes,
            url,
            body,
            apply,
            branch,
            body_out,
        } => {
            let mut map = serde_json::Map::new();
            for m in &messages {
                let (rev, file) = m.split_once('=').ok_or_else(|| Error::Protocol {
                    reason: format!("--message takes <rev>=<file>, and '{m}' has no ="),
                })?;
                let path = std::path::Path::new(file);
                let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
                map.insert(rev.to_string(), Value::String(text));
            }
            let body = match body {
                Some(file) => {
                    let path = std::path::Path::new(&file);
                    Some(std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?)
                }
                None => None,
            };
            let input = json!({
                "range": range, "messages": map, "trunk": trunk,
                "supersedes": supersedes, "url": url, "body": body,
            });
            let v = execute(ctx, &["commit", "reword"], input)?;
            if let (Some(file), Some(text)) = (&body_out, v["successor_body"].as_str()) {
                let path = std::path::Path::new(file);
                std::fs::write(path, text).map_err(|e| Error::io(path, e))?;
            }
            if args.format == OutputFormat::Json && !apply {
                writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?;
            } else {
                render_reword(&mut out, &v)?;
            }
            if v["passed"].as_bool() != Some(true) {
                return Ok(EXIT_UNMET);
            }
            if !apply {
                return Ok(0);
            }
            let branch = branch.unwrap_or_default();
            let plan: crate::commit::reword::RewordPlan =
                serde_json::from_value(v).map_err(|e| Error::Protocol {
                    reason: format!("the reword plan does not read back: {e}"),
                })?;
            let root = std::path::PathBuf::from(app.context.index.repository.root.as_str());
            let done = crate::commit::reword::apply(&root, &plan, &branch)
                .map_err(|reason| Error::Protocol { reason })?;
            render_applied(&mut out, &done, supersedes, body_out.as_deref())?;
            Ok(0)
        }
    }
}

/// What the plan says: one line per commit, what refuses it, and the successor's body.
fn render_reword(out: &mut impl Write, v: &Value) -> Result<()> {
    let short = |s: &str| s.chars().take(9).collect::<String>();
    writeln!(
        out,
        "range    {}..{}",
        short(v["base"].as_str().unwrap_or("")),
        short(v["head"].as_str().unwrap_or(""))
    )
    .map_err(Error::Transport)?;
    for c in v["commits"].as_array().into_iter().flatten() {
        let mark = match (c["reworded"].as_bool(), c["passed"].as_bool()) {
            (_, Some(false)) => "FAIL",
            (Some(true), _) => "new ",
            _ => "kept",
        };
        writeln!(
            out,
            "{mark}     {}  {}",
            short(c["commit"].as_str().unwrap_or("")),
            c["message"]
                .as_str()
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
        )
        .map_err(Error::Transport)?;
        for d in c["findings"].as_array().into_iter().flatten() {
            writeln!(
                out,
                "           {:<8} {:<34} {}",
                d["severity"].as_str().unwrap_or("?"),
                d["code"].as_str().unwrap_or("?"),
                d["message"].as_str().unwrap_or("")
            )
            .map_err(Error::Transport)?;
        }
    }
    for c in v["unrewordable"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "behind   {}  {}  (carried by a kept parent)",
            short(c["commit"].as_str().unwrap_or("")),
            c["subject"].as_str().unwrap_or("")
        )
        .map_err(Error::Transport)?;
    }
    for p in v["problems"].as_array().into_iter().flatten() {
        writeln!(out, "refused  {}", p.as_str().unwrap_or("")).map_err(Error::Transport)?;
    }
    if v["passed"].as_bool() == Some(true) {
        writeln!(
            out,
            "ok       the plan passes; --apply --branch <new> replays it"
        )
        .map_err(Error::Transport)?;
    }
    Ok(())
}

/// What the replay made, and the remote half, which stays with the person.
fn render_applied(
    out: &mut impl Write,
    done: &crate::commit::reword::Applied,
    supersedes: Option<u64>,
    body_out: Option<&str>,
) -> Result<()> {
    let w = |out: &mut dyn Write, line: String| writeln!(out, "{line}").map_err(Error::Transport);
    w(
        &mut *out,
        format!(
            "created  {} at {}",
            done.branch,
            &done.head[..12.min(done.head.len())]
        ),
    )?;
    for (old, new) in &done.replayed {
        w(
            &mut *out,
            format!(
                "         {} -> {}",
                &old[..9.min(old.len())],
                &new[..9.min(new.len())]
            ),
        )?;
    }
    for d in &done.dropped {
        w(
            &mut *out,
            format!("dropped  {d}: git commit-tree does not carry it over"),
        )?;
    }
    w(
        &mut *out,
        "next     the old branch and its pull request are untouched; then:".into(),
    )?;
    w(
        &mut *out,
        format!("         git push -u origin {}", done.branch),
    )?;
    let body = body_out
        .map(|f| format!(" --body-file {f}"))
        .unwrap_or_default();
    w(
        &mut *out,
        format!("         gh pr create --head {}{body}", done.branch),
    )?;
    if let Some(n) = supersedes {
        w(
            &mut *out,
            format!(
                "         gh pr merge {n} --disable-auto; gh pr ready {n} --undo   # the old #{n}"
            ),
        )?;
        w(&mut *out, format!(
            "         #{n} is held while the successor is open and `majordomus prs cleanup --apply` \
             closes it once it lands; the marker counts only from an owner, member or \
             collaborator on a branch of this repository"
        ))?;
    }
    Ok(())
}

/// The message to judge: a file, a revision, or standard input.
///
/// Standard input is the default because that is the shape the `commit-msg` hook has when
/// it pipes, and because `majordomus commit validate <<'EOF'` is how a person tries a
/// message before writing it.
fn read_message(app: &App, file: Option<&str>, rev: Option<&str>) -> Result<String> {
    if let Some(rev) = rev {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(app.context.index.repository.root.as_str())
            .args(["log", "-1", "--format=%B", rev])
            .output()
            .map_err(|e| Error::Protocol {
                reason: format!("git could not be run: {e}"),
            })?;
        if !out.status.success() {
            return Err(Error::Protocol {
                reason: format!(
                    "git does not know the revision '{rev}': {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            });
        }
        return Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string());
    }
    if let Some(file) = file {
        let path = std::path::Path::new(file);
        return std::fs::read_to_string(path).map_err(|e| Error::io(path, e));
    }
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(Error::Transport)?;
    Ok(text)
}

fn render_plan(out: &mut impl Write, v: &Value, format: OutputFormat) -> Result<()> {
    if format == OutputFormat::Json {
        return writeln!(out, "{}", pretty(v)).map_err(Error::Transport);
    }
    let tree = &v["tree"];
    let branch = tree["branch"].as_str().unwrap_or("(detached)");
    let upstream = tree["upstream"]
        .as_str()
        .map(|u| {
            let ahead = tree["ahead"].as_u64().unwrap_or(0);
            let behind = tree["behind"].as_u64().unwrap_or(0);
            format!(" · {u} +{ahead}/-{behind}")
        })
        .unwrap_or_else(|| " · no upstream".into());
    writeln!(out, "branch       {branch}{upstream}").map_err(Error::Transport)?;
    if let Some(what) = tree["in_progress"].as_str() {
        writeln!(out, "in progress  {what}").map_err(Error::Transport)?;
    }
    let changes = tree["changes"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let count = |stage: &str| {
        changes
            .iter()
            .filter(|c| c["stage"].as_str() == Some(stage))
            .count()
    };
    writeln!(
        out,
        "tree         {} staged, {} unstaged, {} untracked",
        count("staged"),
        count("unstaged"),
        count("untracked")
    )
    .map_err(Error::Transport)?;
    writeln!(
        out,
        "fingerprint  {} @ {}",
        v["fingerprint"]["changes"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(12)
            .collect::<String>(),
        v["fingerprint"]["head"]
            .as_str()
            .unwrap_or("?")
            .chars()
            .take(9)
            .collect::<String>()
    )
    .map_err(Error::Transport)?;
    let groups = v["groups"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    if groups.is_empty() {
        writeln!(out, "\nnothing staged; there is no commit to plan").map_err(Error::Transport)?;
    }
    for (i, g) in groups.iter().enumerate() {
        let header = &g["message"]["header"];
        let word = header["word"].as_str().unwrap_or("");
        let scope = header["scope"]
            .as_str()
            .map(|s| format!("({s})"))
            .unwrap_or_default();
        let head = if word.is_empty() && scope.is_empty() {
            "<type>(<scope>): <subject>".to_string()
        } else {
            format!(
                "{}{scope}: <subject>",
                if word.is_empty() { "<type>" } else { word }
            )
        };
        writeln!(out, "\ncommit {}    {head}", i + 1).map_err(Error::Transport)?;
        writeln!(
            out,
            "  why        {}",
            g["rationale"].as_str().unwrap_or("")
        )
        .map_err(Error::Transport)?;
        for p in g["paths"].as_array().into_iter().flatten() {
            writeln!(out, "  {}", p.as_str().unwrap_or("")).map_err(Error::Transport)?;
        }
    }
    for d in v["diagnostics"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "\n{:<8} {} {}",
            d["severity"].as_str().unwrap_or("?"),
            d["code"].as_str().unwrap_or("?"),
            d["message"].as_str().unwrap_or("")
        )
        .map_err(Error::Transport)?;
    }
    Ok(())
}

fn render_scopes(out: &mut impl Write, v: &Value, format: OutputFormat) -> Result<()> {
    if format == OutputFormat::Json {
        return writeln!(out, "{}", pretty(v)).map_err(Error::Transport);
    }
    let vocabulary = &v["vocabulary"];
    if let Some(why) = vocabulary["unavailable"].as_str() {
        return writeln!(out, "no vocabulary: {why}").map_err(Error::Transport);
    }
    writeln!(
        out,
        "learned from {} commit(s), at most {}",
        vocabulary["sampled"].as_u64().unwrap_or(0),
        v["sample"].as_u64().unwrap_or(0)
    )
    .map_err(Error::Transport)?;
    for s in vocabulary["scopes"].as_array().into_iter().flatten() {
        let dirs: Vec<&str> = s["directories"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .take(3)
            .collect();
        writeln!(
            out,
            "  {:<20} {:>4}  {}",
            s["scope"].as_str().unwrap_or("?"),
            s["commits"].as_u64().unwrap_or(0),
            dirs.join(", ")
        )
        .map_err(Error::Transport)?;
    }
    Ok(())
}

fn render_history(out: &mut impl Write, v: &Value, format: OutputFormat) -> Result<()> {
    if format == OutputFormat::Json {
        return writeln!(out, "{}", pretty(v)).map_err(Error::Transport);
    }
    for c in v["commits"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "{}  {}",
            c["commit"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(9)
                .collect::<String>(),
            c["subject"].as_str().unwrap_or("")
        )
        .map_err(Error::Transport)?;
        for d in c["findings"].as_array().into_iter().flatten() {
            writeln!(
                out,
                "           {:<8} {:<34} {}",
                d["severity"].as_str().unwrap_or("?"),
                d["code"].as_str().unwrap_or("?"),
                d["message"].as_str().unwrap_or("")
            )
            .map_err(Error::Transport)?;
        }
    }
    writeln!(
        out,
        "\n{} range      {} commit(s): {} exempt, {} failing",
        v["range"].as_str().unwrap_or("?"),
        v["total"].as_u64().unwrap_or(0),
        v["exempt"].as_u64().unwrap_or(0),
        v["failing"].as_u64().unwrap_or(0)
    )
    .map_err(Error::Transport)
}

fn render_verdict(out: &mut impl Write, v: &Value, format: OutputFormat) -> Result<()> {
    if format == OutputFormat::Json {
        return writeln!(out, "{}", pretty(v)).map_err(Error::Transport);
    }
    let header = &v["message"]["header"];
    writeln!(
        out,
        "{}",
        header["subject"].as_str().unwrap_or("(no subject)")
    )
    .map_err(Error::Transport)?;
    if let Some(exempt) = v["exempt"].as_str() {
        return writeln!(
            out,
            "exempt   {exempt}: git composed this subject, not a person"
        )
        .map_err(Error::Transport);
    }
    for d in v["findings"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "{:<8} {:<34} {}",
            d["severity"].as_str().unwrap_or("?"),
            d["code"].as_str().unwrap_or("?"),
            d["message"].as_str().unwrap_or("")
        )
        .map_err(Error::Transport)?;
    }
    if v["passed"].as_bool() == Some(true) && v["findings"].as_array().is_none_or(|f| f.is_empty())
    {
        writeln!(out, "ok       nothing to report").map_err(Error::Transport)?;
    }
    Ok(())
}

/// Run one capability by the command line it is exposed as, so that the command line and
/// the registry cannot disagree about which capability answers.
fn execute(ctx: &crate::capability::Context, path: &[&str], input: Value) -> Result<Value> {
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(id, input).map_err(map)
}

fn map(e: CapabilityError) -> Error {
    match e {
        CapabilityError::InvalidInput(reason) => Error::Protocol { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reword_plan_renders_each_commit_what_refuses_it_and_whether_it_passes() {
        let v = json!({
            "base": "aaaaaaaaaaaa", "head": "bbbbbbbbbbbb",
            "commits": [
                {"commit": "c1c1c1c1c1c1", "reworded": true, "passed": true,
                 "message": "Codechange: [B] A test\n", "findings": []},
                {"commit": "c2c2c2c2c2c2", "reworded": false, "passed": false,
                 "message": "Test: [X] y\n",
                 "findings": [{"severity": "error", "code": "commit.unknown_word", "message": "'Test' is not a word"}]},
                {"commit": "c3c3c3c3c3c3", "reworded": false, "passed": true,
                 "message": "Fix: [C] z\n", "findings": []}
            ],
            "unrewordable": [{"commit": "d4d4d4d4d4d4", "subject": "Test: [S] side", "findings": []}],
            "problems": ["1 commit(s) reach the branch through a merge's other parent"],
            "passed": false
        });
        let mut buf: Vec<u8> = Vec::new();
        render_reword(&mut buf, &v).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("range    aaaaaaaaa..bbbbbbbbb"), "{text}");
        assert!(
            text.contains("new      c1c1c1c1c  Codechange: [B] A test"),
            "{text}"
        );
        assert!(text.contains("FAIL     c2c2c2c2c  Test: [X] y"), "{text}");
        assert!(text.contains("commit.unknown_word"), "{text}");
        assert!(text.contains("kept     c3c3c3c3c  Fix: [C] z"), "{text}");
        assert!(
            text.contains("behind   d4d4d4d4d  Test: [S] side"),
            "{text}"
        );
        assert!(text.contains("refused  1 commit(s)"), "{text}");
        assert!(!text.contains("the plan passes"), "{text}");
        let mut ok: Vec<u8> = Vec::new();
        render_reword(&mut ok, &json!({"base": "a", "head": "b", "passed": true})).unwrap();
        assert!(String::from_utf8(ok).unwrap().contains("the plan passes"));
    }

    #[test]
    fn an_applied_replay_names_the_branch_what_it_dropped_and_the_remote_half() {
        let done = crate::commit::reword::Applied {
            branch: "fix/reworded".into(),
            head: "0123456789abcdef".into(),
            replayed: vec![("aaaaaaaaaaaa".into(), "bbbbbbbbbbbb".into())],
            dropped: vec!["signature of aaaaaaaaaaaa".into()],
        };
        let mut buf: Vec<u8> = Vec::new();
        render_applied(&mut buf, &done, Some(7), Some("succ.md")).unwrap();
        let text = String::from_utf8(buf).unwrap();
        for want in [
            "created  fix/reworded at 0123456789ab",
            "aaaaaaaaa -> bbbbbbbbb",
            "dropped  signature of aaaaaaaaaaaa",
            "git push -u origin fix/reworded",
            "gh pr create --head fix/reworded --body-file succ.md",
            "gh pr merge 7 --disable-auto; gh pr ready 7 --undo",
            "prs cleanup --apply",
        ] {
            assert!(text.contains(want), "{want}\n{text}");
        }
        let mut bare: Vec<u8> = Vec::new();
        render_applied(&mut bare, &done, None, None).unwrap();
        let bare = String::from_utf8(bare).unwrap();
        assert!(
            bare.contains("gh pr create --head fix/reworded\n") && !bare.contains("gh pr merge")
        );
    }

    fn render(v: Value) -> String {
        let mut buf: Vec<u8> = Vec::new();
        render_verdict(&mut buf, &v, OutputFormat::Text).expect("it renders");
        String::from_utf8(buf).expect("utf-8")
    }

    #[test]
    fn a_verdict_with_nothing_to_say_says_so_rather_than_printing_nothing() {
        // Silence and success are indistinguishable at a terminal, and a hook that printed
        // nothing on a good message would look broken every time it worked.
        let text = render(json!({
            "message": {"header": {"subject": "a thing"}},
            "passed": true,
            "findings": []
        }));
        assert!(text.contains("ok"), "{text}");
        assert!(text.contains("nothing to report"), "{text}");
    }

    #[test]
    fn an_exempt_subject_is_reported_as_exempt_and_not_as_a_pass() {
        let text = render(json!({
            "message": {"header": {"subject": "Merge pull request #1 from a/b"}},
            "passed": true,
            "exempt": "git_authored",
            "findings": []
        }));
        assert!(text.contains("exempt"), "{text}");
        assert!(text.contains("git composed"), "{text}");
    }

    #[test]
    fn every_finding_is_rendered_with_its_code_so_that_it_can_be_looked_up() {
        let text = render(json!({
            "message": {"header": {"subject": "a thing"}},
            "passed": false,
            "findings": [
                {"severity": "error", "code": "commit.subject_too_long", "message": "82 characters"},
                {"severity": "warning", "code": "commit.unknown_scope", "message": "'x' is not a scope"}
            ]
        }));
        assert!(text.contains("commit.subject_too_long"), "{text}");
        assert!(text.contains("commit.unknown_scope"), "{text}");
        assert!(text.contains("82 characters"), "{text}");
    }

    #[test]
    fn a_plan_over_a_clean_tree_says_there_is_nothing_to_commit() {
        let mut buf: Vec<u8> = Vec::new();
        render_plan(
            &mut buf,
            &json!({
                "tree": {"branch": "master", "changes": []},
                "fingerprint": {"head": "a1b2c3d4e5f6", "changes": "deadbeef"},
                "groups": [],
                "diagnostics": []
            }),
            OutputFormat::Text,
        )
        .expect("it renders");
        let text = String::from_utf8(buf).expect("utf-8");
        assert!(text.contains("nothing staged"), "{text}");
        assert!(text.contains("branch       master"), "{text}");
    }
}
