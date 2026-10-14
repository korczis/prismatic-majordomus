//! The cluster page of the Dashboard Suite: who works where, how far each milestone is, and
//! what the integration and derive queues say, as one reading and one rendering (ADR 0159).
//!
//! **It has no data of its own**, like every page of the suite (ADR 0088). Three owners are
//! asked once each and nothing is counted a second time: `mesh.peers` for the machines, their
//! runtimes, sessions and claims; `plan.status` for the progress of every milestone;
//! `brief.session` for the one-line brief, the mesh's links, the integration queue and the
//! derive lock. A source that does not answer is named in `notes` and its section is empty,
//! never invented.
//!
//! **The text is generated once.** `lines` is what the command line prints, what the `/ui`
//! panel draws in a `<pre>` and what a provider mod hands a session on its interval: the
//! three never disagree because none of them renders it. The width is the caller's, and a
//! line is cut to it rather than wrapped, so a table stays a table in a narrow terminal.
//!
//! **Who answers.** The board and the mesh live in the memory of the server that holds the
//! checkout's lease. A command-line process has an empty board, so this capability asks
//! `brief.session` (which already finds the leaseholder) and, when that was answered by a
//! leaseholder other than this process, asks the same route there with `local: true`.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};

/// Console width a caller that names none gets.
pub(crate) const DEFAULT_WIDTH: u16 = 78;
const MIN_WIDTH: u16 = 40;
const MAX_WIDTH: u16 = 200;
/// How many open milestones the page draws before it says how many it left out.
const MILESTONES_SHOWN: usize = 8;
/// How many session lines a machine shows.
const ACTIVITY_SHOWN: usize = 3;
/// How many characters of an announced intent a line keeps at the default width.
const INTENT_CHARS: usize = 60;
/// The widest the milestone id column grows, in characters.
const ID_CELLS_MAX: usize = 24;
/// Width of a progress bar, in cells.
const BAR_CELLS: usize = 20;
/// How long the command line waits for the leaseholder's page.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(20);

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What to ask of `dashboard.cluster`.
pub struct ClusterInput {
    /// Compose in the process that answers and never ask the checkout's leaseholder: what a forwarded request says, so that it is never forwarded again.
    #[serde(default)]
    pub local: bool,
    /// The console width `lines` is cut to, in characters (40 to 200; 78 when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u16>,
}

impl BenchmarkCases for ClusterInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // narrow: a benchmark asks no other server and forwards nothing
        vec![NamedCase::new(
            "default",
            ClusterInput {
                local: true,
                width: Some(DEFAULT_WIDTH),
            },
        )]
    }
}

// ---------------------------------------------------------------- output

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One machine of the mesh and what runs on it.
pub struct ClusterMachine {
    /// The name a person reads: the card's, else the first eight characters of the node id.
    pub name: String,
    /// Whether it is the machine that composed the page.
    pub local: bool,
    /// `own` (this machine), `live` (a runtime beats) or `expired` (none does).
    pub liveness: String,
    /// How many of its runtimes beat.
    pub runtimes_live: usize,
    /// How many runtimes it has, expired ones included.
    pub runtimes: usize,
    /// Active sessions on it.
    pub sessions: usize,
    /// Claims those sessions hold.
    pub claims: usize,
    /// What its sessions say they are doing, at most three, cut.
    pub activity: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One milestone and how far it is.
pub struct ClusterMilestone {
    /// The identity (`M003`, or the slug a milestone is named by).
    pub id: String,
    /// One line naming the outcome.
    pub title: String,
    /// The derived status word the plan reports.
    pub status: String,
    /// Issues done.
    pub done: u32,
    /// Issues that count (the plan's own denominator: total less cancelled).
    pub required: u32,
    /// `done` over `required`, whole percent; `0` where nothing counts.
    pub percent: u32,
    /// Issues ready to take.
    pub ready: u32,
    /// Issues waiting on a dependency.
    pub blocked: u32,
    /// Issues being worked.
    pub active: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The answer of `dashboard.cluster`.
pub struct DashboardCluster {
    /// When it was composed, RFC 3339.
    pub at: String,
    /// Which process composed it: `leaseholder`, `this_process`, or `forwarded` with the address in `notes`.
    pub origin: String,
    /// The brief's one line, as a status line shows it.
    pub line: String,
    /// The machines, this one first.
    pub machines: Vec<ClusterMachine>,
    /// Open milestones, newest id last, at most eight.
    pub milestones: Vec<ClusterMilestone>,
    /// Milestones the plan reports done.
    pub milestones_done: u32,
    /// Every milestone the plan has.
    pub milestones_total: u32,
    /// How many open milestones the page left out.
    pub milestones_left_out: u32,
    /// The integration queue, as `majordomus prs brief` says it.
    pub integration: String,
    /// The derive lock, as the brief says it.
    pub derive: String,
    /// The mesh's links and what its journal addresses to this runtime.
    pub mesh: String,
    /// What could not be read and why, in words.
    pub notes: Vec<String>,
    /// The page as console text, cut to the width asked.
    pub lines: Vec<String>,
}

// ---------------------------------------------------------------- the handler

pub(crate) fn cluster(
    ctx: &Context,
    input: ClusterInput,
) -> Result<DashboardCluster, CapabilityError> {
    let width = usize::from(
        input
            .width
            .unwrap_or(DEFAULT_WIDTH)
            .clamp(MIN_WIDTH, MAX_WIDTH),
    );
    let brief = ctx
        .execute(
            "brief.session",
            json!({ "local": input.local, "checkouts": "repository" }),
        )
        .map_err(|e| e.to_string());

    // The board is the leaseholder's memory: a process without one asks it.
    let mut forwarded_from = None;
    if !input.local && ctx.mesh.cooperation().is_none() {
        let url = brief.as_ref().ok().and_then(|b| {
            (b.pointer("/answered_by/process").and_then(Value::as_str) == Some("leaseholder"))
                .then(|| b.pointer("/answered_by/url").and_then(Value::as_str))
                .flatten()
                .map(str::to_string)
        });
        if let Some(url) = url {
            match forward(ctx, &url, width) {
                Ok(page) => return Ok(page),
                Err(why) => forwarded_from = Some(why),
            }
        }
    }

    let peers = ctx
        .execute("mesh.peers", json!({}))
        .map_err(|e| e.to_string());
    let plan = ctx
        .execute("plan.status", json!({}))
        .map_err(|e| e.to_string());
    let mut page = compose(
        crate::ledger::now(),
        brief.as_ref().ok(),
        peers.as_ref().ok(),
        plan.as_ref().ok(),
        width,
    );
    for (what, r) in [
        ("brief.session", &brief),
        ("mesh.peers", &peers),
        ("plan.status", &plan),
    ] {
        if let Err(why) = r {
            page.notes.push(format!("{what} did not answer: {why}"));
        }
    }
    if let Some(why) = forwarded_from {
        page.notes.push(why);
    }
    page.lines = render(&page, width);
    Ok(page)
}

/// Ask the leaseholder for its page, over the route the registry declares.
fn forward(ctx: &Context, url: &str, width: usize) -> Result<DashboardCluster, String> {
    let path = ctx
        .registry
        .get("dashboard.cluster")
        .and_then(|c| c.exposure.http.as_ref())
        .map(|h| h.path.clone())
        .ok_or_else(|| "dashboard.cluster declares no HTTP route to ask over".to_string())?;
    let query = format!("{path}?local=true&width={width}");
    let reply = crate::mcp::bridge::request(url, "GET", &query, &[], None, FORWARD_TIMEOUT)
        .map_err(|e| {
            format!("the leaseholder at {url} did not answer ({e}); composed by this process")
        })?;
    if reply.status != 200 {
        return Err(format!(
            "the leaseholder at {url} answered {} (a server older than this executable has no cluster page); composed by this process",
            reply.status
        ));
    }
    let mut page: DashboardCluster = serde_json::from_str(&reply.body).map_err(|e| {
        format!("the leaseholder at {url} answered a page this executable cannot read ({e}); composed by this process")
    })?;
    page.origin = "forwarded".into();
    page.notes
        .push(format!("composed by the leaseholder at {url}"));
    Ok(page)
}

// ---------------------------------------------------------------- composition

/// Compose the page from what three owners answered. Pure: the same answers and width give
/// the same page, which is what the tests hold.
pub(crate) fn compose(
    at: String,
    brief: Option<&Value>,
    peers: Option<&Value>,
    plan: Option<&Value>,
    width: usize,
) -> DashboardCluster {
    let text = |v: Option<&Value>, p: &str| -> String {
        v.and_then(|v| v.pointer(p))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let machines = peers.map(machines_of).unwrap_or_default();
    let (milestones, done, total, left_out) = plan.map(milestones_of).unwrap_or_default();
    let origin = match text(brief, "/answered_by/process").as_str() {
        "" => "this_process".to_string(),
        p => p.to_string(),
    };
    let mut page = DashboardCluster {
        at,
        origin,
        line: text(brief, "/line"),
        machines,
        milestones,
        milestones_done: done,
        milestones_total: total,
        milestones_left_out: left_out,
        integration: text(brief, "/integration/summary"),
        derive: text(brief, "/derive/summary"),
        mesh: text(brief, "/mesh/summary"),
        notes: Vec::new(),
        lines: Vec::new(),
    };
    page.lines = render(&page, width);
    page
}

fn machines_of(peers: &Value) -> Vec<ClusterMachine> {
    let empty = Vec::new();
    let mut out = Vec::new();
    for m in peers["machines"].as_array().unwrap_or(&empty) {
        let runtimes = m["runtimes"].as_array().unwrap_or(&empty);
        let live = |r: &Value| r["liveness"].as_str() != Some("expired");
        let runtimes_live = runtimes.iter().filter(|r| live(r)).count();
        let own = runtimes
            .iter()
            .any(|r| r["liveness"].as_str() == Some("own"));
        let (mut sessions, mut claims, mut activity) = (0usize, 0usize, Vec::new());
        for r in runtimes {
            for s in r["sessions"].as_array().unwrap_or(&empty) {
                if !s["state"]
                    .as_str()
                    .is_some_and(|w| w.eq_ignore_ascii_case("active"))
                {
                    continue;
                }
                sessions += 1;
                claims += s["claims_held"].as_u64().unwrap_or(0) as usize;
                if activity.len() < ACTIVITY_SHOWN {
                    let who = s
                        .pointer("/info/worker")
                        .and_then(Value::as_str)
                        .or_else(|| s.pointer("/info/client").and_then(Value::as_str))
                        .unwrap_or("session");
                    let what = s
                        .pointer("/info/intent")
                        .and_then(Value::as_str)
                        .unwrap_or("(no intent announced)");
                    activity.push(format!("{who}: {}", clip(what, INTENT_CHARS)));
                }
            }
        }
        out.push(ClusterMachine {
            name: m["name"].as_str().unwrap_or("?").to_string(),
            local: m["local"].as_bool().unwrap_or(false),
            liveness: if own {
                "own"
            } else if runtimes_live > 0 {
                "live"
            } else {
                "expired"
            }
            .to_string(),
            runtimes_live,
            runtimes: runtimes.len(),
            sessions,
            claims,
            activity,
        });
    }
    out
}

fn milestones_of(plan: &Value) -> (Vec<ClusterMilestone>, u32, u32, u32) {
    let empty = Vec::new();
    let all = plan["milestones"].as_array().unwrap_or(&empty);
    let (mut open, mut done) = (Vec::new(), 0u32);
    for m in all {
        let status = m["status"].as_str().unwrap_or_default().to_string();
        if status.eq_ignore_ascii_case("DONE") {
            done += 1;
            continue;
        }
        if status.eq_ignore_ascii_case("CANCELLED") {
            continue;
        }
        let n = |k: &str| {
            m.pointer(&format!("/counts/by_status/{k}"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32
        };
        let required = m
            .pointer("/counts/required")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let issues_done = n("DONE");
        open.push(ClusterMilestone {
            id: m["id"].as_str().unwrap_or("?").to_string(),
            title: m["title"].as_str().unwrap_or_default().to_string(),
            status,
            done: issues_done,
            required,
            percent: (issues_done * 100).checked_div(required).unwrap_or(0),
            ready: n("READY"),
            blocked: n("BLOCKED"),
            active: n("ACTIVE"),
        });
    }
    let total = all.len() as u32;
    let left_out = open.len().saturating_sub(MILESTONES_SHOWN) as u32;
    // the milestones being worked first, then the ones with the most left to do
    open.sort_by(|a, b| {
        (b.active > 0)
            .cmp(&(a.active > 0))
            .then(b.percent.cmp(&a.percent))
            .then(a.id.cmp(&b.id))
    });
    open.truncate(MILESTONES_SHOWN);
    (open, done, total, left_out)
}

// ---------------------------------------------------------------- rendering

/// A bar of `cells` cells, `done` of `total` filled.
pub(crate) fn bar(done: u32, total: u32, cells: usize) -> String {
    let filled = if total == 0 {
        0
    } else {
        ((u64::from(done.min(total)) * cells as u64 + u64::from(total) / 2) / u64::from(total))
            as usize
    };
    format!("{}{}", "█".repeat(filled), "░".repeat(cells - filled))
}

/// `s` on one line, cut to `n` characters with an ellipsis.
pub(crate) fn clip(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        s
    } else {
        let cut: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{}…", cut.trim_end())
    }
}

/// `s` cut to `n` characters with an ellipsis, spacing kept: a rendered line's columns are
/// its meaning, which `clip` (free text) would collapse.
fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{}…", head.trim_end())
    }
}

fn pad(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len >= n {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(n - len))
    }
}

/// The page as console text, every line cut to `width`.
pub(crate) fn render(p: &DashboardCluster, width: usize) -> Vec<String> {
    let live = p
        .machines
        .iter()
        .filter(|m| m.liveness != "expired")
        .count();
    let sessions: usize = p.machines.iter().map(|m| m.sessions).sum();
    let claims: usize = p.machines.iter().map(|m| m.claims).sum();
    let inner = width.saturating_sub(4);
    let mut out: Vec<String> = Vec::new();

    let title = format!(" CLUSTER  {} ", p.at);
    let dashes = width.saturating_sub(title.chars().count() + 2);
    out.push(format!(
        "╭─{title}{}╮",
        "─".repeat(dashes.saturating_sub(1))
    ));
    let head = format!(
        "{sessions} session(s) · {claims} claim(s) · {live}/{} machine(s) live · milestones {}/{} done",
        p.machines.len(),
        p.milestones_done,
        p.milestones_total
    );
    out.push(format!("│ {} │", pad(&clip(&head, inner), inner)));
    if !p.line.is_empty() {
        out.push(format!("│ {} │", pad(&clip(&p.line, inner), inner)));
    }
    out.push(format!("╰{}╯", "─".repeat(width.saturating_sub(2))));

    out.push(String::new());
    out.push("MACHINES".into());
    if p.machines.is_empty() {
        out.push("  none read (the mesh is not running here, or no server answered)".into());
    }
    let widest = p
        .machines
        .iter()
        .map(|m| m.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(20);
    let busiest = p
        .machines
        .iter()
        .map(|m| m.sessions)
        .max()
        .unwrap_or(0)
        .max(1);
    for m in &p.machines {
        let mark = match m.liveness.as_str() {
            "own" => "●",
            "live" => "●",
            _ => "○",
        };
        let name = if m.local {
            format!("{} (this)", m.name)
        } else {
            m.name.clone()
        };
        let load = bar(m.sessions as u32, busiest as u32, 8);
        out.push(cut(
            &format!(
                "  {mark} {}  {load}  {} session(s) {} claim(s)  {}/{} runtime(s) live{}",
                pad(&name, widest + 7),
                m.sessions,
                m.claims,
                m.runtimes_live,
                m.runtimes,
                if m.liveness == "expired" {
                    "  expired"
                } else {
                    ""
                }
            ),
            width,
        ));
        for a in &m.activity {
            out.push(cut(&format!("      {a}"), width));
        }
    }

    out.push(String::new());
    out.push("PLAN".into());
    if p.milestones.is_empty() && p.milestones_total == 0 {
        out.push("  no plan read".into());
    } else if p.milestones.is_empty() {
        out.push("  every milestone is done".into());
    }
    // one id column for the whole table, as wide as the longest id up to a cap: a slug of
    // twenty characters must not push its bar out of line with the others
    let id_cells = p
        .milestones
        .iter()
        .map(|m| m.id.chars().count())
        .max()
        .unwrap_or(0)
        .clamp(4, ID_CELLS_MAX);
    for m in &p.milestones {
        out.push(cut(
            &format!(
                "  {} ▕{}▏ {:>3}%  {}/{}  {}",
                pad(&cut(&m.id, id_cells), id_cells),
                bar(m.done, m.required, BAR_CELLS),
                m.percent,
                m.done,
                m.required,
                m.title
            ),
            width,
        ));
    }
    if p.milestones_left_out > 0 {
        out.push(format!(
            "  … and {} more open milestone(s)",
            p.milestones_left_out
        ));
    }

    for (label, text) in [
        ("INTEGRATION", &p.integration),
        ("DERIVE", &p.derive),
        ("MESH", &p.mesh),
    ] {
        out.push(String::new());
        out.push(cut(
            &format!(
                "{}  {}",
                pad(label, 11),
                if text.is_empty() { "not read" } else { text }
            ),
            width,
        ));
    }
    if !p.notes.is_empty() {
        out.push(String::new());
        out.push("NOT READ".into());
        for n in &p.notes {
            out.push(cut(&format!("  {n}"), width));
        }
    }
    // the width is a promise about every line, the fixed ones included
    out.into_iter().map(|l| cut(&l, width)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peers() -> Value {
        json!({ "machines": [
            { "node": "aaa", "name": "mac", "local": true, "runtimes": [
                { "runtime": "aaa-1", "liveness": "own", "sessions": [
                    { "state": "active", "claims_held": 2,
                      "info": { "worker": "claude@mac", "client": "claude-code", "intent": "building the cluster page" } },
                    { "state": "closed", "claims_held": 9, "info": { "client": "cli" } } ] } ] },
            { "node": "bbb", "name": "linux", "local": false, "runtimes": [
                { "runtime": "bbb-1", "liveness": "live", "sessions": [
                    { "state": "active", "claims_held": 0, "info": { "client": "codex" } } ] },
                { "runtime": "bbb-2", "liveness": "expired", "sessions": [] } ] },
            { "node": "ccc", "name": "gone", "local": false, "runtimes": [
                { "runtime": "ccc-1", "liveness": "expired", "sessions": [] } ] } ] })
    }

    fn plan() -> Value {
        let counts = |done: u64, ready: u64, blocked: u64, active: u64, required: u64| {
            json!({ "total": required, "required": required, "by_status":
                { "DONE": done, "READY": ready, "BLOCKED": blocked, "ACTIVE": active, "VERIFY": 0, "CANCELLED": 0 } })
        };
        json!({ "milestones": [
            { "id": "M000", "status": "DONE", "title": "finished", "counts": counts(5, 0, 0, 0, 5) },
            { "id": "M001", "status": "ACTIVE", "title": "in flight", "counts": counts(3, 1, 0, 1, 10) },
            { "id": "M002", "status": "READY", "title": "not started", "counts": counts(0, 4, 0, 0, 4) },
            { "id": "M003", "status": "CANCELLED", "title": "dropped", "counts": counts(0, 0, 0, 0, 0) } ] })
    }

    #[test]
    fn machines_count_active_sessions_and_their_claims_only() {
        let m = machines_of(&peers());
        assert_eq!(m.len(), 3);
        assert_eq!(
            (m[0].sessions, m[0].claims, m[0].liveness.as_str()),
            (1, 2, "own")
        );
        assert_eq!(m[0].activity, vec!["claude@mac: building the cluster page"]);
        // a closed session's claims are not this machine's work
        assert_eq!(m[1].liveness, "live");
        assert_eq!((m[1].runtimes_live, m[1].runtimes), (1, 2));
        assert_eq!(m[1].activity, vec!["codex: (no intent announced)"]);
        assert_eq!(m[2].liveness, "expired");
    }

    #[test]
    fn milestones_are_the_plans_numbers_and_done_and_cancelled_are_not_drawn() {
        let (open, done, total, left_out) = milestones_of(&plan());
        assert_eq!((done, total, left_out), (1, 4, 0));
        assert_eq!(open.len(), 2);
        // the one being worked comes first
        assert_eq!(open[0].id, "M001");
        assert_eq!(
            (open[0].done, open[0].required, open[0].percent),
            (3, 10, 30)
        );
        assert_eq!((open[0].ready, open[0].active), (1, 1));
        assert_eq!(open[1].percent, 0);
    }

    #[test]
    fn a_bar_is_proportional_and_never_overflows() {
        assert_eq!(bar(0, 10, 10), "░░░░░░░░░░");
        assert_eq!(bar(5, 10, 10), "█████░░░░░");
        assert_eq!(bar(10, 10, 10), "██████████");
        assert_eq!(
            bar(99, 10, 10),
            "██████████",
            "more done than required is full, not wider"
        );
        assert_eq!(
            bar(0, 0, 4),
            "░░░░",
            "nothing required draws empty, not a division"
        );
    }

    #[test]
    fn the_page_is_deterministic_and_every_line_fits_the_width() {
        let brief = json!({
            "line": "I1 @main | peers 3/5b | mesh 3/3 | prs 52 open",
            "answered_by": { "process": "leaseholder", "url": "http://127.0.0.1:1" },
            "integration": { "summary": "master at abc observed 3 min ago: 52 open" },
            "derive": { "summary": "free" },
            "mesh": { "summary": "3 of 3 link(s) connected" },
        });
        let a = compose("t".into(), Some(&brief), Some(&peers()), Some(&plan()), 60);
        let b = compose("t".into(), Some(&brief), Some(&peers()), Some(&plan()), 60);
        assert_eq!(a, b, "the same answers give the same page");
        assert_eq!(a.origin, "leaseholder");
        for l in &a.lines {
            assert!(
                l.chars().count() <= 60,
                "{l:?} is wider than the width asked"
            );
        }
        // wide enough that nothing is cut, the page says everything the sources said
        let wide = compose("t".into(), Some(&brief), Some(&peers()), Some(&plan()), 100);
        let text = wide.lines.join("\n");
        assert!(
            text.contains("2 session(s) · 2 claim(s) · 2/3 machine(s) live · milestones 1/4 done"),
            "{text}"
        );
        assert!(text.contains("● mac (this)"), "{text}");
        assert!(
            text.contains("○ gone"),
            "an expired machine is drawn open: {text}"
        );
        assert!(text.contains("M001") && text.contains(" 30%"), "{text}");
        assert!(text.contains("INTEGRATION  master at abc"), "{text}");
    }

    #[test]
    fn the_narrowest_page_fits_even_where_nothing_was_read() {
        let p = compose(
            "2026-10-13T22:35:32Z".into(),
            None,
            None,
            None,
            usize::from(MIN_WIDTH),
        );
        for l in &p.lines {
            assert!(
                l.chars().count() <= usize::from(MIN_WIDTH),
                "{l:?} is wider than the floor"
            );
        }
    }

    #[test]
    fn an_unanswered_source_is_said_and_never_invented() {
        let p = compose("t".into(), None, None, None, 78);
        assert!(p.machines.is_empty() && p.milestones.is_empty());
        assert_eq!(p.origin, "this_process");
        let text = p.lines.join("\n");
        assert!(text.contains("none read"), "{text}");
        assert!(text.contains("no plan read"), "{text}");
        assert!(text.contains("INTEGRATION  not read"), "{text}");
    }
}
