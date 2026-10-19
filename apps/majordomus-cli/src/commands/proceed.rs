//! `majordomus proceed`: what a session proceeds with, on the command line (ADR 0177).
//!
//! `proceed plan`: the capability decides; this renders. `proceed.plan` asks the server
//! holding this checkout's lease, so the claims a person reads here are that server's board
//! and mesh and not the empty ones of this process.
//!
//! `proceed take` acts on the decision once (I2974, ADR 0177 decision 3), composing what
//! exists and nothing else: `proceed.plan` and `mesh.claim` asked of the *primary* checkout's
//! server, `worktree create`'s ensure of the issue's canonical branch, and `start --issue` run
//! by the tool in that worktree. Every mesh call goes to one runtime whichever checkout asks,
//! because a mesh session's key ends with its server run: the claim a worktree's own server
//! would hold is another session's.
//!
//! What a session has taken is read first from the take record written into the worktree
//! (`.ai/local/state/proceed.json`), never from a live claim alone, so the same session taking
//! again — from the primary checkout or the worktree, after the server restarted, after
//! `finish` — gets its work back and creates nothing: an expired claim is taken again, a
//! finished task ends the take and releases its claim, a conflicted claim is refused with the
//! claim that beat it. A claim this run made is released when its worktree cannot be
//! ensured; a start that is refused keeps the claim and the record, so a rerun continues at
//! the start.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::app::App;
use crate::capability::Context;
use crate::cli::{OutputFormat, ProceedArgs, ProceedCommand};
use crate::error::{Error, Result};
use crate::integration::exclusive::{server_of, Mesh};

/// The exit code when nothing can be selected: the code `dashboard overview` and `quality
/// report` use for an answer that is not satisfied.
pub const EXIT_NOTHING_SELECTED: u8 = 10;
/// The exit code when an issue is named but the decision is provisional: a source it needs
/// (the board, or a declared mesh) could not be read, so nothing should be taken on it.
pub const EXIT_PROVISIONAL: u8 = 11;
/// The exit code when a take is refused: its claim lost a race twice, its claim lost the
/// fold, or the worktree it would use holds another active task.
pub const EXIT_REFUSED: u8 = 15;

/// Run `majordomus proceed`.
pub fn run(args: ProceedArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    match &args.command {
        ProceedCommand::Plan { .. } => answer(&app.context, &args, &mut stdout.lock()),
        ProceedCommand::Take { session, milestone } => {
            let report = take(&app, session, milestone.as_deref())?;
            let text = if args.format == OutputFormat::Json {
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                )
            } else {
                report.render()
            };
            stdout
                .lock()
                .write_all(text.as_bytes())
                .map_err(Error::Transport)?;
            Ok(0)
        }
    }
}

/// Ask the capability exposed as `majordomus proceed plan` and write its answer to `out`.
fn answer(ctx: &Context, args: &ProceedArgs, out: &mut impl Write) -> Result<u8> {
    let ProceedCommand::Plan { milestone, session } = &args.command else {
        unreachable!("answer renders proceed plan only")
    };
    let id = ctx
        .registry
        .by_cli(&["proceed".to_string(), "plan".to_string()])
        .map(|c| c.id.as_str().to_string())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus proceed plan`".into(),
        })?;
    let mut input = json!({});
    if let Some(m) = milestone {
        input["milestone"] = json!(m);
    }
    if let Some(s) = session {
        input["session"] = json!(s);
    }
    let v = ctx.execute(&id, input).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    let text = if args.format == OutputFormat::Json {
        format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default())
    } else {
        render(&v)
    };
    out.write_all(text.as_bytes()).map_err(Error::Transport)?;
    Ok(exit_for(&v))
}

/// The exit code the decision's own words imply.
fn exit_for(v: &Value) -> u8 {
    if !v["selected"].is_object() {
        EXIT_NOTHING_SELECTED
    } else if v["provisional"] == json!(true) {
        EXIT_PROVISIONAL
    } else {
        0
    }
}

/// The decision as a person reads it: the selection and why, then what was passed over,
/// what waits and what the decision was read from.
fn render(v: &Value) -> String {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    let list = |v: &Value| -> Vec<Value> { v.as_array().cloned().unwrap_or_default() };
    let mut out = String::new();
    let sel = &v["selected"];
    if sel.is_object() {
        let t = &sel["terms"];
        out.push_str(&format!(
            "selected  {}  {} ({}, wave {}, {})\n",
            s(&sel["issue"]),
            s(&sel["title"]),
            s(&sel["milestone"]),
            t["wave"],
            s(&t["priority"])
        ));
    } else {
        out.push_str("selected  nothing\n");
    }
    out.push_str(&format!("why       {}\n", s(&v["reason"])));
    if sel.is_object() {
        let unblocks: Vec<String> = list(&sel["terms"]["unblocks"]).iter().map(s).collect();
        out.push_str(&format!(
            "unblocks  {}\n",
            if unblocks.is_empty() {
                "nothing".to_string()
            } else {
                unblocks.join(", ")
            }
        ));
        let f = &sel["finishes_with"];
        out.push_str(&format!(
            "finishes  {} (evidence {} of {})\n",
            s(&f["objective"]),
            f["have"],
            f["need"]
        ));
        for (n, step) in list(&v["next"]).iter().enumerate() {
            let label = if n == 0 { "next" } else { "" };
            out.push_str(&format!("{label:<9} {}. {}\n", n + 1, s(step)));
        }
    }
    let mut first = true;
    for c in list(&v["candidates"]) {
        for r in list(&c["refused"]) {
            let label = if first { "passed" } else { "" };
            first = false;
            out.push_str(&format!(
                "{label:<9} {}  {} by {}: {}\n",
                s(&c["issue"]),
                s(&r["constraint"]),
                s(&r["holder"]),
                s(&r["detail"])
            ));
        }
    }
    if let Some(w) = v["waiting"].as_object().filter(|w| !w.is_empty()) {
        let parts: Vec<String> = w.iter().map(|(k, n)| format!("{k} {n}")).collect();
        out.push_str(&format!("waiting   {}\n", parts.join(", ")));
    }
    for (n, src) in list(&v["sources"]).iter().enumerate() {
        let label = if n == 0 { "sources" } else { "" };
        out.push_str(&format!(
            "{label:<9} {} {}: {}\n",
            s(&src["source"]),
            s(&src["state"]),
            s(&src["summary"])
        ));
    }
    out
}

// ---------------------------------------------------------------- proceed take

/// Where a take is recorded, in the worktree it took.
pub(crate) const TAKE_RECORD: &str = ".ai/local/state/proceed.json";

/// What a take wrote into its worktree: whose work it is and how to find its claim again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TakeRecord {
    /// The mesh session the work is held under, on the primary checkout's server.
    pub session: String,
    /// The issue taken.
    pub issue: String,
    /// Its title, for the task's description.
    pub title: String,
    /// The branch of the worktree.
    pub branch: String,
    /// The scope the claim and the task cover.
    pub scope: Vec<String>,
    /// The key of the exclusive claim, as the primary checkout's server answered it.
    pub claim: String,
    /// When the take was made.
    pub at: String,
}

/// What each step of a take did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Step {
    /// This run made it.
    Created,
    /// It was there already and was used as it is.
    Found,
    /// The claim had ended (its server restarted) and was taken again.
    Reclaimed,
    /// The task had finished, so the take ended and its claim was given back.
    Released,
}

/// The answer of `proceed take`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TakeReport {
    /// The session the work is held under.
    pub session: String,
    /// The issue taken.
    pub issue: String,
    /// Its title.
    pub title: String,
    /// The primary checkout whose server holds the claim.
    pub primary: String,
    /// The claim's key.
    pub claim: String,
    /// The worktree.
    pub worktree: String,
    /// Its branch.
    pub branch: String,
    /// The task bound to the issue in it, once started.
    pub task: Option<String>,
    /// What each step did: `decision`, `claim`, `worktree`, `task`.
    pub steps: Vec<(String, Step)>,
    /// Why, in one sentence.
    pub reason: String,
}

impl TakeReport {
    fn render(&self) -> String {
        let mut out = format!(
            "took      {}  {}\nwhy       {}\nworktree  {} ({})\nclaim     {} on {}\n",
            self.issue,
            self.title,
            self.reason,
            self.worktree,
            self.branch,
            self.claim,
            self.primary
        );
        if let Some(t) = &self.task {
            out.push_str(&format!("task      {t}\n"));
        }
        let steps: Vec<String> = self
            .steps
            .iter()
            .map(|(name, step)| {
                format!(
                    "{name} {}",
                    serde_json::to_value(step)
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or_default()
                )
            })
            .collect();
        out.push_str(&format!("steps     {}\n", steps.join(", ")));
        out
    }
}

fn refused(code: u8, reason: impl Into<String>) -> Error {
    Error::Refused {
        code,
        reason: reason.into(),
    }
}

/// The primary checkout of the repository at `root`: the first worktree git lists, or `root`
/// when git lists none.
fn primary_of(root: &Path) -> (PathBuf, Vec<crate::worktree::WorktreeRecord>) {
    let records = crate::worktree::topology::read(root).unwrap_or_default();
    let primary = records
        .iter()
        .find(|r| !r.bare)
        .map(|r| r.path.clone())
        .unwrap_or_else(|| root.to_path_buf());
    (primary, records)
}

/// The take recorded in `worktree`, when there is one.
pub(crate) fn read_take(worktree: &Path) -> Option<TakeRecord> {
    let text = std::fs::read_to_string(worktree.join(TAKE_RECORD)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_take(worktree: &Path, record: &TakeRecord) -> Result<()> {
    let path = worktree.join(TAKE_RECORD);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let text = serde_json::to_string_pretty(record).unwrap_or_default();
    std::fs::write(&path, format!("{text}\n")).map_err(|e| Error::io(&path, e))
}

/// Where the task of `worktree` stands, as far as a take is concerned.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TaskStanding {
    /// No task of this checkout, or a record of another checkout's.
    None,
    /// A task bound to the issue, not finished (active, or handed over).
    Open(String),
    /// A task bound to the issue that finished.
    Finished(String),
    /// An active task bound to something else.
    Other(String),
}

/// The task recorded in `worktree`, judged against `issue`. A record whose `worktree` names
/// another checkout is that checkout's (`mj_task_is_foreign`), and counts as none here.
pub(crate) fn task_standing(worktree: &Path, issue: &str) -> TaskStanding {
    let path = worktree.join(".ai/local/state/current.yaml");
    let Some(task) = crate::capability::builtin::continuity::read_task(&path) else {
        return TaskStanding::None;
    };
    let foreign = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| crate::metadata::yaml::parse_mapping(&t).ok())
        .and_then(|m| m.get("worktree").and_then(crate::metadata::yaml::scalar_string))
        .filter(|w| !w.is_empty())
        .is_some_and(|w| {
            let w = PathBuf::from(w);
            w.canonicalize().unwrap_or(w)
                != worktree.canonicalize().unwrap_or(worktree.to_path_buf())
        });
    if foreign {
        return TaskStanding::None;
    }
    let open = matches!(task.outcome.as_str(), "active" | "handed_over");
    match (task.issue == issue, open) {
        (true, true) => TaskStanding::Open(task.id),
        (true, false) => TaskStanding::Finished(task.id),
        (false, true) if task.outcome == "active" => TaskStanding::Other(task.id),
        _ => TaskStanding::None,
    }
}

/// `method target` on the primary checkout's server: the answer when it is 200, else the
/// refusal it gave.
fn ask(home: &dyn Mesh, method: &str, target: &str, body: Option<Value>) -> Result<Value> {
    match home.ask(method, target, body) {
        Ok((200, v)) => Ok(v),
        Ok((status, v)) => Err(refused(
            if status == 409 || v.to_string().contains("claim_conflict") {
                EXIT_REFUSED
            } else {
                crate::worktree::EXIT_INTERNAL
            },
            format!(
                "{method} {target} answered {status}: {}",
                v["error"]["message"].as_str().unwrap_or(&v.to_string())
            ),
        )),
        Err(e) => Err(refused(EXIT_PROVISIONAL, e)),
    }
}

/// Claim `scope` exclusively for `session` on the home server; the claim's key.
fn claim(
    home: &dyn Mesh,
    session: &str,
    issue: &str,
    title: &str,
    scope: &[String],
) -> Result<String> {
    let mut scope = scope.to_vec();
    let record = format!(".ai/repo/project/issues/{issue}.yaml");
    if !scope.contains(&record) {
        scope.push(record);
    }
    let v = ask(
        home,
        "POST",
        "/api/v1/mesh/claims",
        Some(json!({
            "session": session,
            "scope": scope,
            "intent": format!("proceed: {issue}: {title}"),
            "mode": "exclusive",
            "issue": issue,
        })),
    )?;
    v["key"].as_str().map(str::to_string).ok_or_else(|| {
        refused(
            crate::worktree::EXIT_INTERNAL,
            format!("the claim answered no key: {v}"),
        )
    })
}

fn release(home: &dyn Mesh, key: &str) -> Result<()> {
    ask(
        home,
        "POST",
        "/api/v1/mesh/claims/release",
        Some(json!({ "claim": key })),
    )
    .map(|_| ())
}

/// The standing of claim `key` among the live claims of `session` on the home server: held,
/// conflicted (with the winner's key), or gone.
fn claim_standing(home: &dyn Mesh, session: &str, key: &str) -> Result<Option<(String, String)>> {
    let target = format!(
        "/api/v1/mesh/state?view=full&claims=live&session={}",
        crate::http::router::percent_encode(session)
    );
    let v = ask(home, "GET", &target, None)?;
    Ok(v["state"]["claims"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["key"] == key)
        .map(|c| {
            let state = &c["state"];
            (
                state["state"].as_str().unwrap_or_default().to_string(),
                state["detail"].as_str().unwrap_or_default().to_string(),
            )
        }))
}

/// Take what `proceed.plan` selects for `session`, or give back what the session took.
fn take(app: &App, session: &str, milestone: Option<&str>) -> Result<TakeReport> {
    let root = PathBuf::from(&app.index().repository.root);
    let (primary, records) = primary_of(&root);
    let home = server_of(&primary).ok_or_else(|| {
        refused(
            EXIT_PROVISIONAL,
            format!(
                "no server answers for the primary checkout {}, which holds every take; nothing was claimed or created. Start it with: majordomus --repo {} serve ensure",
                primary.display(),
                primary.display()
            ),
        )
    })?;
    let mut steps = Vec::new();

    // what the session took already, wherever it is
    let held = records
        .iter()
        .filter(|r| !r.bare && r.path.exists())
        .find_map(|r| {
            read_take(&r.path)
                .filter(|t| t.session == session)
                .map(|t| (r.path.clone(), t))
        });
    let (worktree, record) = match held {
        Some((worktree, mut record)) => {
            steps.push(("decision".to_string(), Step::Found));
            match task_standing(&worktree, &record.issue) {
                TaskStanding::Finished(id) => {
                    release(&home, &record.claim)
                        .or_else(|e| match claim_standing(&home, session, &record.claim) {
                            Ok(None) => Ok(()),
                            _ => Err(e),
                        })?;
                    std::fs::remove_file(worktree.join(TAKE_RECORD))
                        .map_err(|e| Error::io(worktree.join(TAKE_RECORD), e))?;
                    steps.push(("claim".to_string(), Step::Released));
                    steps.push(("task".to_string(), Step::Found));
                    return Ok(TakeReport {
                        session: session.into(),
                        issue: record.issue.clone(),
                        title: record.title.clone(),
                        primary: primary.display().to_string(),
                        claim: record.claim.clone(),
                        worktree: worktree.display().to_string(),
                        branch: record.branch.clone(),
                        task: Some(id.clone()),
                        steps,
                        reason: format!(
                            "{} was taken by {session} and its task {id} has finished, so the take ends here and its claim is released; hand the head to the landing seat, then take again",
                            record.issue
                        ),
                    });
                }
                TaskStanding::Other(id) => {
                    return Err(refused(
                        EXIT_REFUSED,
                        format!(
                            "{} holds task {id}, which is not bound to {}; finish or hand it over before the take continues",
                            worktree.display(),
                            record.issue
                        ),
                    ))
                }
                _ => {}
            }
            match claim_standing(&home, session, &record.claim)? {
                Some((state, _)) if state == "held" => steps.push(("claim".to_string(), Step::Found)),
                Some((state, winner)) if state == "conflicted" => {
                    return Err(refused(
                        EXIT_REFUSED,
                        format!(
                            "the claim {} on {} lost to {winner}: another runtime took the same work before the runtimes met; release it (majordomus --repo {} mesh release {}) and take again",
                            record.claim,
                            record.issue,
                            primary.display(),
                            record.claim
                        ),
                    ))
                }
                _ => {
                    let mut r = record.clone();
                    r.claim = claim(&home, session, &r.issue, &r.title, &r.scope)?;
                    write_take(&worktree, &r)?;
                    record = r;
                    steps.push(("claim".to_string(), Step::Reclaimed));
                }
            }
            steps.push(("worktree".to_string(), Step::Found));
            (worktree, record)
        }
        None => {
            let (decision, key) = decide_and_claim(&home, session, milestone)?;
            steps.push(("decision".to_string(), Step::Created));
            steps.push(("claim".to_string(), Step::Created));
            let issue = decision["issue"].as_str().unwrap_or_default().to_string();
            let title = decision["title"].as_str().unwrap_or_default().to_string();
            let scope: Vec<String> = decision["scope"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect();
            let ensured =
                super::worktree::branch_of_issue(app.index(), &issue).and_then(|branch| {
                    let svc = crate::worktree::WorktreeService::open(&primary)
                        .map_err(|e| refused(e.exit_code(), e.to_string()))?;
                    svc.ensure(&crate::worktree::CreateRequest {
                        branch: branch.clone(),
                        base: None,
                    })
                    .map_err(|e| refused(e.exit_code(), e.to_string()))
                    .map(|report| (branch, report))
                });
            let (branch, report) = match ensured {
                Ok(x) => x,
                Err(e) => {
                    // nothing holds the claim's work: give it back before saying why
                    let _ = release(&home, &key);
                    return Err(e);
                }
            };
            steps.push((
                "worktree".to_string(),
                if report.existed {
                    Step::Found
                } else {
                    Step::Created
                },
            ));
            let record = TakeRecord {
                session: session.into(),
                issue,
                title,
                branch,
                scope,
                claim: key,
                at: crate::evidence::record::now_rfc3339(),
            };
            let worktree = PathBuf::from(&report.path);
            write_take(&worktree, &record)?;
            (worktree, record)
        }
    };

    // the task, bound to the issue, in the worktree
    let task = match task_standing(&worktree, &record.issue) {
        TaskStanding::Open(id) => {
            steps.push(("task".to_string(), Step::Found));
            id
        }
        TaskStanding::Other(id) => {
            return Err(refused(
                EXIT_REFUSED,
                format!(
                    "{} holds task {id}, which is not bound to {}; finish or hand it over before the take continues",
                    worktree.display(),
                    record.issue
                ),
            ))
        }
        _ => {
            start(app, &worktree, &record)?;
            steps.push(("task".to_string(), Step::Created));
            match task_standing(&worktree, &record.issue) {
                TaskStanding::Open(id) => id,
                other => {
                    return Err(refused(
                        crate::worktree::EXIT_INTERNAL,
                        format!("start exited 0 and the worktree holds no task bound to {}: {other:?}", record.issue),
                    ))
                }
            }
        }
    };
    let created = steps.iter().any(|(_, s)| *s != Step::Found);
    Ok(TakeReport {
        session: session.into(),
        issue: record.issue.clone(),
        title: record.title.clone(),
        primary: primary.display().to_string(),
        claim: record.claim.clone(),
        worktree: worktree.display().to_string(),
        branch: record.branch.clone(),
        task: Some(task),
        steps,
        reason: if created {
            format!("{} is {session}'s, held on the primary checkout's server, and its task runs in its worktree", record.issue)
        } else {
            format!("{session} took {} already: its claim, worktree and task are as they were, and nothing was created", record.issue)
        },
    })
}

/// Ask `proceed.plan` for `session` and claim its selection; once more when the claim lost a
/// race between the decision and the claim. The selection and the claim's key.
fn decide_and_claim(
    home: &dyn Mesh,
    session: &str,
    milestone: Option<&str>,
) -> Result<(Value, String)> {
    let mut target = format!(
        "/api/v1/proceed/plan?local=true&session={}",
        crate::http::router::percent_encode(session)
    );
    if let Some(m) = milestone {
        target.push_str(&format!(
            "&milestone={}",
            crate::http::router::percent_encode(m)
        ));
    }
    let mut lost = Vec::new();
    for _ in 0..2 {
        let d = ask(home, "GET", &target, None)?;
        if !d["selected"].is_object() {
            return Err(refused(
                EXIT_NOTHING_SELECTED,
                format!(
                    "nothing to take: {}",
                    d["reason"].as_str().unwrap_or_default()
                ),
            ));
        }
        if d["provisional"] == json!(true) {
            return Err(refused(
                EXIT_PROVISIONAL,
                format!(
                    "the decision is provisional, so nothing is taken: {}",
                    d["reason"].as_str().unwrap_or_default()
                ),
            ));
        }
        let sel = d["selected"].clone();
        let scope: Vec<String> = sel["scope"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect();
        let issue = sel["issue"].as_str().unwrap_or_default();
        let title = sel["title"].as_str().unwrap_or_default();
        match claim(home, session, issue, title, &scope) {
            Ok(key) => return Ok((sel, key)),
            Err(Error::Refused { code, reason }) if code == EXIT_REFUSED => lost.push(reason),
            Err(e) => return Err(e),
        }
    }
    Err(refused(
        EXIT_REFUSED,
        format!(
            "two selections were claimed by others before this session could: {}",
            lost.join("; ")
        ),
    ))
}

/// `start --issue` in the worktree, run by this tool's own front end with the share it
/// resolved, so that a repository without a launcher of its own starts the same way.
fn start(app: &App, worktree: &Path, record: &TakeRecord) -> Result<()> {
    let front = app
        .share
        .dir()
        .parent()
        .map(|d| d.join("bin/majordomus"))
        .filter(|p| p.is_file())
        .ok_or_else(|| {
            refused(
                crate::worktree::EXIT_INTERNAL,
                format!(
                    "no front end beside the share at {}",
                    app.share.dir().display()
                ),
            )
        })?;
    let out = std::process::Command::new(&front)
        .arg("--repo")
        .arg(worktree)
        .arg("start")
        .arg(format!("{}: {}", record.issue, record.title))
        .arg("--issue")
        .arg(&record.issue)
        .arg("--scope")
        .arg(record.scope.join(","))
        .current_dir(worktree)
        .env("MAJORDOMUS_SHARE", app.share.dir())
        .output()
        .map_err(|e| Error::io(&front, e))?;
    let mut err = std::io::stderr();
    let _ = err.write_all(&out.stdout);
    let _ = err.write_all(&out.stderr);
    match out.status.code() {
        Some(0) => Ok(()),
        code => Err(refused(
            code.and_then(|c| u8::try_from(c).ok()).unwrap_or(crate::worktree::EXIT_INTERNAL),
            format!(
                "start refused {} in {}; the claim and the take are kept, so taking again continues here: {}",
                record.issue,
                worktree.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn args(argv: &[&str]) -> ProceedArgs {
        let mut full = vec!["majordomus", "proceed"];
        full.extend_from_slice(argv);
        match crate::cli::Cli::try_parse_from(full)
            .expect("parses")
            .command
        {
            crate::cli::Command::Proceed(a) => a,
            _ => panic!("not the proceed command"),
        }
    }

    #[test]
    fn a_selection_renders_its_reason_steps_refusals_and_sources() {
        let v = json!({
            "selected": {
                "issue": "I0002", "title": "t", "milestone": "M",
                "terms": {"in_milestone": true, "wave": 1, "priority": "p1", "unblocks": ["I0003"]},
                "refused": [], "shared_with": [], "scope": ["src"],
                "finishes_with": {"objective": "o", "have": 0, "need": 1}
            },
            "reason": "I0002 is the first",
            "next": ["claim", "worktree"],
            "milestone": "M",
            "candidates": [{
                "issue": "I0001",
                "refused": [{"constraint": "claimed", "hold": "k", "holder": "p2", "detail": "names I0001"}]
            }],
            "waiting": {"BLOCKED": 2},
            "sources": [{"source": "board", "state": "read", "holds": 1, "summary": "1 board(s)"}]
        });
        let text = render(&v);
        for line in [
            "selected  I0002  t (M, wave 1, p1)",
            "why       I0002 is the first",
            "unblocks  I0003",
            "finishes  o (evidence 0 of 1)",
            "next      1. claim",
            "          2. worktree",
            "passed    I0001  claimed by p2: names I0001",
            "waiting   BLOCKED 2",
            "sources   board read: 1 board(s)",
        ] {
            assert!(text.contains(line), "missing {line:?} in\n{text}");
        }
        let none = render(&json!({"selected": null, "reason": "r", "sources": []}));
        assert_eq!(none, "selected  nothing\nwhy       r\n");
        let quiet = render(&json!({"selected": {"terms": {"unblocks": []}, "finishes_with": {}}}));
        assert!(quiet.contains("unblocks  nothing"), "{quiet}");
    }

    #[test]
    fn the_exit_code_is_the_decisions_own_word() {
        assert_eq!(exit_for(&json!({"selected": null})), EXIT_NOTHING_SELECTED);
        let sel = json!({"issue": "I1"});
        assert_eq!(
            exit_for(&json!({"selected": sel, "provisional": true})),
            EXIT_PROVISIONAL
        );
        assert_eq!(exit_for(&json!({"selected": sel, "provisional": false})), 0);
    }

    #[test]
    fn the_command_answers_with_the_capability_and_exits_10_on_no_selection() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let mut out = Vec::new();
        let code = answer(
            &ctx,
            &args(&[
                "plan",
                "--milestone",
                "M",
                "--session",
                "s",
                "--format",
                "json",
            ]),
            &mut out,
        )
        .unwrap();
        assert_eq!(code, EXIT_NOTHING_SELECTED);
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["milestone"], "M");
        assert!(v["selected"].is_null());
        let mut out = Vec::new();
        answer(&ctx, &args(&["plan"]), &mut out).unwrap();
        assert!(String::from_utf8(out)
            .unwrap()
            .starts_with("selected  nothing\n"));
    }

    #[test]
    fn a_task_is_judged_against_the_issue_the_take_holds() {
        let dir = tempfile::tempdir().unwrap();
        let wt = dir.path().canonicalize().unwrap();
        assert_eq!(task_standing(&wt, "I1"), TaskStanding::None, "no record");
        let state = wt.join(".ai/local/state");
        std::fs::create_dir_all(&state).unwrap();
        let write = |outcome: &str, issue: &str, worktree: &Path| {
            std::fs::write(
                state.join("current.yaml"),
                format!(
                    "id: t-1\noutcome: {outcome}\nissue: {issue}\nworktree: {}\nscope:\n  - src\n",
                    worktree.display()
                ),
            )
            .unwrap()
        };
        write("active", "I1", &wt);
        assert_eq!(task_standing(&wt, "I1"), TaskStanding::Open("t-1".into()));
        write("handed_over", "I1", &wt);
        assert_eq!(
            task_standing(&wt, "I1"),
            TaskStanding::Open("t-1".into()),
            "a handed-over task is resumed, not replaced"
        );
        write("partial", "I1", &wt);
        assert_eq!(
            task_standing(&wt, "I1"),
            TaskStanding::Finished("t-1".into())
        );
        write("active", "I2", &wt);
        assert_eq!(task_standing(&wt, "I1"), TaskStanding::Other("t-1".into()));
        write("completed", "I2", &wt);
        assert_eq!(
            task_standing(&wt, "I1"),
            TaskStanding::None,
            "another issue's finished task is no obstacle"
        );
        write("active", "I1", Path::new("/elsewhere"));
        assert_eq!(
            task_standing(&wt, "I1"),
            TaskStanding::None,
            "another checkout's record is not this one's task"
        );
    }

    #[test]
    fn a_take_record_reads_back_what_was_written() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_take(dir.path()), None);
        let record = TakeRecord {
            session: "s1".into(),
            issue: "I1".into(),
            title: "t".into(),
            branch: "feature/I1-t".into(),
            scope: vec!["src/a".into()],
            claim: "stream/c-1".into(),
            at: "2026-10-18T00:00:00Z".into(),
        };
        write_take(dir.path(), &record).unwrap();
        assert_eq!(read_take(dir.path()), Some(record));
    }

    #[test]
    fn a_take_without_a_server_for_the_primary_checkout_creates_nothing() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let app = App::load(&crate::cli::RepoArgs {
            repo: Some(repo.root().to_path_buf()),
            discovery: crate::cli::DiscoveryMode::Filesystem,
            share: Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
            ..Default::default()
        })
        .unwrap();
        let Err(Error::Refused { code, reason }) = take(&app, "s1", None) else {
            panic!("a take with no server must be refused");
        };
        assert_eq!(code, EXIT_PROVISIONAL);
        assert!(reason.contains("serve ensure"), "{reason}");
        assert!(!repo.root().join(TAKE_RECORD).exists());
    }
}
