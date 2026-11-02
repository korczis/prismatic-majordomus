//! The Blackboard: the work as a board of its delivery stages, with who carries each piece
//! (I4236). Nothing here derives a stage, a presence or a join of its own: the columns are
//! `plan.delivery`'s stages as `plan.issues` and `blackboard.snapshot` name them, and a card
//! is joined to a worker only through an issue id the worker's own record carries.
//!
//! The first paint asks the blackboard alone, which answers from memory: the workers, what
//! they claim, a graph of who carries which issue, and the stream. The board itself reads
//! `plan.delivery`, which walks the trunk's history and takes about a second here, so it is
//! a panel of its own at [`PANEL`]: the script pulls it in once the screen has painted, and a
//! reader without JavaScript follows its link.
//!
//! The screen follows `/events?follow=blackboard&frames=snapshot` while JavaScript runs, as
//! `/ui/peers` does, and is read again when the blackboard changes. It names no worker's
//! intent, worktree, machine or claimed path: those stay on `/ui/peers`, which a publication
//! withholds (I2984), so this screen can be published as it is.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask, ask_all, Answer, Source};
use super::view::{
    graph, hero, kpi, kpi_count, list, nothing, panel, text, timeline, unanswered, wide, GraphNode,
    Layout,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/board";

/// The route of the board panel, which the screen pulls in once it has painted.
pub const PANEL: &str = "/ui/panels/board";

/// The module that follows the live channel for the screen, beside `ui.js`.
pub const LIB: &str = "live.js";

/// The channel the screen follows: the blackboard's snapshot frames, which say it changed.
pub const CHANNEL: &str = "/events?follow=blackboard&frames=snapshot";

const BLACKBOARD: Source = Source::with("blackboard.snapshot", r#"{"signals":24}"#);
const DELIVERY: Source = Source::of("plan.delivery");
const ISSUES: Source = Source::of("plan.issues");

/// How many cards a column draws before it says how many more there are.
const SHOWN: usize = 14;

/// The columns, in the order work moves through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lane {
    /// Ready to start, and nothing of it is in flight.
    Ready,
    /// A branch names it, a live claim names it, or it is declared active.
    InFlight,
    /// An open pull request names it.
    InReview,
    /// Merged into the trunk, released or deployed.
    Landed,
}

impl Lane {
    const ALL: [Lane; 4] = [Lane::Ready, Lane::InFlight, Lane::InReview, Lane::Landed];

    fn title(self) -> &'static str {
        match self {
            Lane::Ready => "Ready",
            Lane::InFlight => "In flight",
            Lane::InReview => "In review",
            Lane::Landed => "Landed",
        }
    }

    fn tone(self) -> &'static str {
        match self {
            Lane::Ready => "cyan",
            Lane::InFlight => "blue",
            Lane::InReview => "orange",
            Lane::Landed => "green",
        }
    }

    fn says(self) -> &'static str {
        match self {
            Lane::Ready => {
                "Ready by the plan and not yet started, highest priority and earliest wave first."
            }
            Lane::InFlight => {
                "A branch, a live claim or an active stamp names it; no pull request yet."
            }
            Lane::InReview => "An open pull request names it.",
            Lane::Landed => "Its newest merge is on the trunk, newest first.",
        }
    }
}

/// The column an issue's delivery puts it in, or `None` when it belongs on no board column
/// (a blocked issue nobody has started). `claimed` says a worker's record names the issue;
/// `in_review` says `plan.delivery` found an open pull request naming it. An open pull
/// request is where the issue is now even when an earlier merge took its stage further, so
/// it wins; otherwise the stage is `plan.delivery`'s, only read.
pub fn lane(stage: &str, status: &str, claimed: bool, in_review: bool) -> Option<Lane> {
    if in_review {
        return Some(Lane::InReview);
    }
    match stage {
        "merged" | "released" | "deployed" => Some(Lane::Landed),
        "pr_open" => Some(Lane::InReview),
        "branch" => Some(Lane::InFlight),
        _ if claimed || matches!(status, "ACTIVE" | "VERIFY") => Some(Lane::InFlight),
        _ if status == "READY" => Some(Lane::Ready),
        _ => None,
    }
}

/// Where an issue is read in full.
fn issue_href(id: &str) -> String {
    format!("{}?issue={id}#issue", super::prs::ROUTE)
}

/// Where a pull request is read in full.
fn pr_href(number: u64) -> String {
    format!("{}?pr={number}#pr", super::prs::ROUTE)
}

/// Where a worker is read in full: the Peers screen. Never its row's anchor, which is the
/// worker's key: a publication names no session key (`published-safely`).
fn worker_href() -> &'static str {
    super::peers::ROUTE
}

/// A worker as an avatar: the short name people use for it, never its intent or machine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Avatar {
    /// The worker's key on the blackboard: what joins it, never drawn.
    pub key: String,
    /// The label drawn: its board position, else its client.
    pub label: String,
    /// `attached`, `active` or `departed`, as the blackboard said.
    pub presence: String,
}

fn avatar_of(w: &Value) -> Avatar {
    // the board position when the worker has one; else the client's initial and the tail of
    // the worker's key, which tells two sessions of one client apart without naming either
    let key = text(w, "key");
    let label = match text(w, "board_peer") {
        p if !p.is_empty() => p,
        _ => {
            let client = text(w, "client");
            let initial = client
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase().to_string())
                .unwrap_or_else(|| "?".into());
            let tail: String = key
                .chars()
                .rev()
                .filter(char::is_ascii_alphanumeric)
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("{initial}{tail}")
        }
    };
    Avatar {
        key,
        label,
        presence: text(w, "presence"),
    }
}

/// Every issue a worker's record names, its own task's issue and each claim's, joined to the
/// worker: an explicit identifier, never a match of words.
pub fn carriers(board: &Answer) -> BTreeMap<String, BTreeSet<Avatar>> {
    let mut by_issue: BTreeMap<String, BTreeSet<Avatar>> = BTreeMap::new();
    for w in list(board, "/workers") {
        let avatar = avatar_of(&w);
        let mut issues: BTreeSet<String> = BTreeSet::new();
        let own = text(&w, "issue");
        if !own.is_empty() {
            issues.insert(own);
        }
        for claim in w
            .get("claims")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = text(claim, "issue");
            if !id.is_empty() {
                issues.insert(id);
            }
        }
        for id in issues {
            by_issue.entry(id).or_default().insert(avatar.clone());
        }
    }
    by_issue
}

fn avatar(a: &Avatar) -> El {
    el("a")
        .class(format!(
            "mj-ui-board-avatar mj-ui-board-avatar--{}",
            if a.presence == "departed" {
                "away"
            } else {
                "here"
            }
        ))
        .attr("href", worker_href())
        .attr("title", format!("{} · {}", a.label, a.presence))
        .text(a.label.chars().take(3).collect::<String>())
}

// ---------------------------------------------------------------- the screen

/// The Blackboard; `lib` is the URL of [`LIB`] the page loads to follow the channel.
pub fn screen(ctx: &Context, lib: &str) -> View {
    let board = ask(ctx, BLACKBOARD);
    let carried = carriers(&board);

    let top = hero(
        "grid",
        "violet",
        "Blackboard",
        "The work by delivery stage, and who carries it.",
        "Every issue at the stage the plan derives from git, the forge and the releases, with the workers whose own records name it. Each card leads to its issue, each avatar to its worker, each pull request to its queue entry.",
        vec![
            kpi_count("Workers", &board.read("/workers")),
            kpi("Attached now", &board.read("/present")),
            kpi("Claims that meet", &board.read("/conflicts_total")),
            kpi_count("Reviews open", &board.read("/reviews")),
            kpi_count("Handovers waiting", &board.read("/handovers")),
            kpi("Every source read", &board.read("/complete")),
        ],
    );

    let following = el("p")
        .class("mj-ui-dim")
        .attr("data-follow-state", "")
        .text("Read when this page was served. While JavaScript runs it follows the live channel and is read again when the blackboard changes.");

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(placeholder()))
        .child(panel(
            "Who carries what",
            "Each worker and the issues its own record names: its task's issue and each claim's.",
            carrying(&board, &carried),
            &[&board],
        ))
        .child(panel(
            "The stream",
            "What happened, newest first, as the monitor folded it.",
            stream(&board),
            &[&board],
        ));

    View::new(
        "Blackboard",
        el("div")
            .class("mj-ui-stack")
            .attr("data-follow", CHANNEL)
            .attr("data-follow-lib", lib)
            .child(top)
            .child(following)
            .child(grid),
    )
}

/// Where the board will be. Without JavaScript it is a link to the panel's own page; the
/// script replaces it with the board once the screen has painted.
fn placeholder() -> El {
    el("section")
        .class("mj-ui-panel")
        .attr("aria-label", "The board")
        .attr("data-panel-src", PANEL)
        .attr("aria-busy", "false")
        .child(
            el("header")
                .class("mj-ui-panel-head")
                .child(el("h2").class("mj-ui-panel-title").text("The board"))
                .child(el("p").class("mj-ui-panel-sub").text(
                    "Asked on its own: plan.delivery walks the trunk's history, which would hold this screen back.",
                )),
        )
        .child(
            el("p").class("mj-ui-panel-body").child(
                el("a")
                    .class("mj-ui-link")
                    .attr("href", PANEL)
                    .text("Open the board"),
            ),
        )
}

/// The workers and the issues they carry, as a graph with its table beneath for a reader
/// without the picture.
fn carrying(board: &Answer, carried: &BTreeMap<String, BTreeSet<Avatar>>) -> El {
    unanswered(board).unwrap_or_else(|| {
        if carried.is_empty() {
            return nothing("No worker's record names an issue right now.");
        }
        let mut nodes: BTreeMap<String, GraphNode> = BTreeMap::new();
        let mut edges: Vec<(String, String)> = Vec::new();
        let mut rows: Vec<El> = Vec::new();
        // a worker's node is its position among the workers drawn, never its key: the page
        // carries its graph as data, and a publication names no session key
        let mut position: BTreeMap<&str, usize> = BTreeMap::new();
        for a in carried.values().flatten() {
            let next = position.len();
            position.entry(a.key.as_str()).or_insert(next);
        }
        for (issue, avatars) in carried {
            let issue_node = format!("issue:{issue}");
            nodes
                .entry(issue_node.clone())
                .or_insert_with(|| GraphNode {
                    id: issue_node.clone(),
                    label: issue.clone(),
                    tone: "orange",
                    href: issue_href(issue),
                    kind: "node",
                });
            for a in avatars {
                let worker_node = format!("worker:{}", position[a.key.as_str()]);
                nodes
                    .entry(worker_node.clone())
                    .or_insert_with(|| GraphNode {
                        id: worker_node.clone(),
                        label: a.label.clone(),
                        tone: if a.presence == "departed" {
                            "rose"
                        } else {
                            "blue"
                        },
                        href: worker_href().to_string(),
                        kind: "node",
                    });
                edges.push((worker_node.clone(), issue_node.clone()));
                rows.push(
                    el("tr")
                        .child(el("td").child(avatar(a)))
                        .child(
                            el("td").child(
                                el("a")
                                    .class("mj-ui-link mj-ui-mono")
                                    .attr("href", issue_href(issue))
                                    .text(issue.clone()),
                            ),
                        )
                        .child(el("td").text(a.presence.clone())),
                );
            }
        }
        let fallback = super::view::table(
            &["Worker", "Issue", "Here"],
            rows,
            "No worker's record names an issue right now.",
        );
        let nodes: Vec<GraphNode> = nodes.into_values().collect();
        graph(
            "The workers and the issues their records name",
            Layout::Force,
            &nodes,
            &edges,
            fallback,
        )
    })
}

/// How many signals the stream draws.
const STREAM: usize = 12;

/// The words of a signal a publication may carry: a merge, a run and a release are the
/// forge's and public already; a mesh signal's summary quotes intents and claimed paths, so
/// it is drawn as who and what kind, with its pull request and issue linked.
fn public_summary(s: &Value) -> String {
    match text(s, "source").as_str() {
        "github" | "executor" => text(s, "summary"),
        _ => text(s, "kind").replace('_', " "),
    }
}

/// The monitor's newest signals the blackboard carries, each linked where it says it can be
/// read, and none quoting an intent or a claimed path.
fn stream(board: &Answer) -> El {
    unanswered(board).unwrap_or_else(|| {
        let signals = list(board, "/signals");
        if signals.is_empty() {
            return nothing("Nothing happened that the monitor kept.");
        }
        timeline(
            signals
                .iter()
                .take(STREAM)
                .map(|s| {
                    let at = s
                        .get("at")
                        .and_then(Value::as_u64)
                        .map(|secs| {
                            crate::peers::rfc3339(
                                std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs),
                            )
                        })
                        .unwrap_or_default();
                    let mut detail = el("span").class("mj-ui-dim mj-ui-mono");
                    let who = [text(s, "actor"), text(s, "kind")]
                        .into_iter()
                        .filter(|x| !x.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · ");
                    detail = detail.text(who);
                    let mut body = el("span").child(detail);
                    if let Some(n) = s.get("pr").and_then(Value::as_u64) {
                        body = body.child(
                            el("a")
                                .class("mj-ui-link mj-ui-mono")
                                .attr("href", pr_href(n))
                                .text(format!(" #{n}")),
                        );
                    }
                    let issue = text(s, "issue");
                    if !issue.is_empty() {
                        body = body.child(
                            el("a")
                                .class("mj-ui-link mj-ui-mono")
                                .attr("href", issue_href(&issue))
                                .text(format!(" {issue}")),
                        );
                    }
                    (at, public_summary(s), body)
                })
                .collect(),
        )
    })
}

// ---------------------------------------------------------------- the board panel

/// The board, as a panel: what the fragment request and the page without JavaScript both
/// show.
pub fn panel_view(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[DELIVERY, ISSUES, BLACKBOARD]);
    let (delivery, issues, board) = (
        &answers[&DELIVERY],
        &answers[&ISSUES],
        &answers[&BLACKBOARD],
    );
    View::new("The board", columns(delivery, issues, board))
}

/// One card's facts, read from the three answers and joined by the issue id.
#[derive(Debug, Clone)]
struct Card {
    lane: Lane,
    id: String,
    title: String,
    milestone: String,
    priority: String,
    wave: u64,
    status: String,
    delivery: Value,
    carriers: Vec<Avatar>,
}

/// A column's order, stated once: an open column puts the carried cards first, then the plan's
/// priority, wave and id; the landed one puts the newest merge first.
impl crate::order::Ordered for Card {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let key = crate::order::OrderKey::plain(&self.id, &self.id);
        match self.lane {
            // an ISO 8601 time read as its digits grows with the time it names
            Lane::Landed => key.descending(
                merged_at(&self.delivery)
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    // ui-integrity: a sort key, never drawn: a merge with no time sorts after every timed one
                    .unwrap_or(0),
            ),
            _ => key.ranked(
                i64::from(u8::from(self.carriers.is_empty())) * 10_000_000_000
                    + i64::from(priority_rank(&self.priority)) * 1_000_000_000
                    + i64::try_from(self.wave.min(999_999_999)).unwrap_or(999_999_999),
            ),
        }
    }
}

fn priority_rank(p: &str) -> u8 {
    match p {
        "p0" => 0,
        "p1" => 1,
        "p2" => 2,
        "p3" => 3,
        _ => 9,
    }
}

/// The board's columns from the delivery, the plan and the blackboard.
fn columns(delivery: &Answer, issues: &Answer, board: &Answer) -> El {
    let body = unanswered(delivery).unwrap_or_else(|| {
        let plan: BTreeMap<String, Value> = list(issues, "/issues")
            .into_iter()
            .map(|i| (text(&i, "id"), i))
            .collect();
        let carried = carriers(board);
        let mut lanes: BTreeMap<Lane, Vec<Card>> = BTreeMap::new();
        for d in list(delivery, "/issues") {
            let id = text(&d, "issue");
            let p = plan.get(&id);
            let field = |k: &str| p.map(|v| text(v, k)).unwrap_or_default();
            let carriers: Vec<Avatar> = carried
                .get(&id)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            let status = text(&d, "status");
            let in_review = d.get("pr_open").is_some_and(|p| !p.is_null());
            let Some(lane) = lane(&text(&d, "stage"), &status, !carriers.is_empty(), in_review)
            else {
                continue;
            };
            lanes.entry(lane).or_default().push(Card {
                lane,
                title: text(&d, "title"),
                milestone: text(&d, "milestone"),
                priority: field("priority"),
                wave: p
                    .and_then(|v| v.get("wave"))
                    .and_then(Value::as_u64)
                    .unwrap_or(u64::MAX),
                status,
                carriers,
                delivery: d,
                id,
            });
        }
        for cards in lanes.values_mut() {
            crate::order::canonical(cards);
        }
        // a column a source could have filled, when that source was not read, says so rather
        // than reading as empty: an unobserved forge is not "nothing in review"
        let unread = unread_sources(delivery);
        el("div")
            .class("mj-ui-board")
            // it scrolls sideways on a narrow screen, so a keyboard must be able to reach it
            .attr("tabindex", "0")
            .attr("aria-label", "The board, by delivery stage")
            .children(Lane::ALL.iter().map(|l| {
                let why = match l {
                    Lane::InReview => unread.get("forge"),
                    Lane::InFlight => unread.get("branches"),
                    _ => None,
                };
                column(*l, lanes.get(l).map(Vec::as_slice).unwrap_or(&[]), why)
            }))
    });
    panel(
        "The board",
        "Every issue at the stage plan.delivery derives from git, the forge observation and the releases; a card carries the workers whose own records name it.",
        body,
        &[delivery, issues, board],
    )
    .attr("id", "board")
}

/// The sources `plan.delivery` could not read, each with the reason it gave.
fn unread_sources(delivery: &Answer) -> BTreeMap<String, String> {
    list(delivery, "/sources")
        .iter()
        .filter(|s| text(s, "state") == "unread")
        .map(|s| (text(s, "name"), text(s, "detail")))
        .collect()
}

fn merged_at(d: &Value) -> String {
    d.pointer("/merged/merged_at")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn column(lane: Lane, cards: &[Card], unread: Option<&String>) -> El {
    let shown = cards.iter().take(SHOWN).map(card);
    let mut col = el("section")
        .class(format!("mj-ui-board-lane mj-tone--{}", lane.tone()))
        .attr("aria-label", lane.title())
        .child(
            el("header")
                .class("mj-ui-board-lane-head")
                .child(
                    el("span")
                        .class("mj-ui-board-lane-dot")
                        .attr("aria-hidden", "true"),
                )
                .child(el("h3").class("mj-ui-board-lane-title").text(lane.title()))
                .child(
                    el("span")
                        .class("mj-ui-board-lane-count")
                        .text(cards.len().to_string()),
                ),
        )
        .child(el("p").class("mj-ui-board-lane-says").text(lane.says()))
        .children(shown);
    if let Some(why) = unread {
        col = col.child(
            el("p")
                .class("mj-ui-empty")
                .text(format!("Not read, so this column may be short: {why}")),
        );
    }
    if cards.is_empty() && unread.is_none() {
        col = col.child(el("p").class("mj-ui-empty").text("Nothing at this stage."));
    } else if cards.len() > SHOWN {
        col = col.child(
            el("p")
                .class("mj-ui-board-more")
                .text(format!("{} more at this stage", cards.len() - SHOWN)),
        );
    }
    col
}

fn card(c: &Card) -> El {
    let mut head = el("div").class("mj-ui-board-card-head").child(
        el("a")
            .class("mj-ui-board-id mj-ui-mono")
            .attr("href", issue_href(&c.id))
            .text(c.id.clone()),
    );
    if !c.priority.is_empty() {
        head = head.child(
            el("span")
                .class(format!("mj-ui-board-prio mj-ui-board-prio--{}", c.priority))
                .text(c.priority.clone()),
        );
    }
    if !c.status.is_empty() {
        head = head.child(
            el("span")
                .class("mj-ui-board-status")
                .text(c.status.to_lowercase()),
        );
    }
    let mut chips = el("div").class("mj-ui-board-chips");
    if let Some(pr) = c.delivery.get("pr_open") {
        if let Some(n) = pr.get("number").and_then(Value::as_u64) {
            let draft = pr.get("draft").and_then(Value::as_bool) == Some(true);
            chips = chips.child(
                el("a")
                    .class("mj-ui-board-chip mj-ui-board-chip--pr")
                    .attr("href", pr_href(n))
                    .text(if draft {
                        format!("#{n} draft")
                    } else {
                        format!("#{n}")
                    }),
            );
        }
    }
    if let Some(b) = c.delivery.pointer("/branch/name").and_then(Value::as_str) {
        if c.delivery.get("pr_open").is_none() {
            chips = chips.child(
                el("span")
                    .class("mj-ui-board-chip mj-ui-mono")
                    .text(b.to_string()),
            );
        }
    }
    if let Some(m) = c.delivery.get("merged") {
        let commit = m.get("commit").and_then(Value::as_str).unwrap_or_default();
        let merged = match m.get("pull_request").and_then(Value::as_u64) {
            Some(n) => el("a")
                .class("mj-ui-board-chip mj-ui-board-chip--merged")
                .attr("href", pr_href(n))
                .text(format!("#{n}")),
            None => el("span")
                .class("mj-ui-board-chip mj-ui-board-chip--merged mj-ui-mono")
                .text(commit.chars().take(11).collect::<String>()),
        };
        chips = chips.child(merged);
        if let Some(t) = c.delivery.pointer("/released/tag").and_then(Value::as_str) {
            chips = chips.child(el("span").class("mj-ui-board-chip").text(t.to_string()));
        }
    }
    let mut foot = el("div").class("mj-ui-board-card-foot").child(chips);
    if !c.carriers.is_empty() {
        foot = foot.child(
            el("div")
                .class("mj-ui-board-avatars")
                .children(c.carriers.iter().map(avatar)),
        );
    }
    el("article")
        .class("mj-ui-board-card")
        .attr("id", format!("card-{}", c.id))
        .child(head)
        .child(
            el("a")
                .class("mj-ui-board-card-title")
                .attr("href", issue_href(&c.id))
                .text(c.title.clone()),
        )
        .child(
            el("span")
                .class("mj-ui-board-card-meta")
                .text(if c.wave == u64::MAX {
                    c.milestone.clone()
                } else {
                    format!("{} · wave {}", c.milestone, c.wave)
                }),
        )
        .child(foot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::source::{Exposure, Standing};
    use std::sync::Arc;
    use std::time::Duration;

    fn answer(source: Source, value: Value) -> Answer {
        Answer {
            source,
            route: None,
            exposure: Exposure::default(),
            value: Ok(Arc::new(value)),
            took: Duration::from_millis(1),
            standing: Standing::NOW,
        }
    }

    /// The column is the delivery stage, read and never recomputed: a merge lands it, an open
    /// pull request reviews it, a branch or a claim or an active stamp puts it in flight, and
    /// a ready issue nobody touched is ready. A blocked one nobody holds is on no column.
    #[test]
    fn the_column_is_the_stage_the_delivery_derived() {
        assert_eq!(lane("released", "READY", false, false), Some(Lane::Landed));
        assert_eq!(lane("merged", "DONE", true, false), Some(Lane::Landed));
        assert_eq!(lane("pr_open", "READY", false, true), Some(Lane::InReview));
        assert_eq!(
            lane("released", "READY", false, true),
            Some(Lane::InReview),
            "an open pull request is where the issue is now, whatever an earlier merge reached"
        );
        assert_eq!(lane("branch", "READY", false, false), Some(Lane::InFlight));
        assert_eq!(lane("none", "READY", true, false), Some(Lane::InFlight));
        assert_eq!(lane("none", "ACTIVE", false, false), Some(Lane::InFlight));
        assert_eq!(lane("none", "READY", false, false), Some(Lane::Ready));
        assert_eq!(lane("none", "BLOCKED", false, false), None);
    }

    /// A worker is joined to an issue only through an id its own record carries, its task's
    /// or a claim's, and a worker with no such id carries nothing.
    #[test]
    fn a_worker_carries_the_issues_its_record_names_and_nothing_else() {
        let board = answer(
            BLACKBOARD,
            serde_json::json!({ "workers": [
                { "key": "mesh:a", "board_peer": "p3", "presence": "attached", "issue": "I0703",
                  "intent": "I2749 is mentioned here only in prose",
                  "claims": [{ "issue": "I4236", "scope": ["secret/path"] }] },
                { "key": "board:b", "client": "codex", "presence": "departed", "claims": [] }
            ]}),
        );
        let carried = carriers(&board);
        assert_eq!(
            carried.keys().cloned().collect::<Vec<_>>(),
            ["I0703", "I4236"],
            "an issue named only in prose is no join"
        );
        assert_eq!(carried["I4236"].iter().next().unwrap().label, "p3");
    }

    /// The board draws every column, links each card to its issue, its pull request to the
    /// queue entry and its worker to the Peers screen, and never prints a claimed path or an
    /// intent: the screen can be published as it is.
    #[test]
    fn every_card_links_to_its_objects_and_names_no_private_path() {
        let delivery = answer(
            DELIVERY,
            serde_json::json!({ "issues": [
                { "issue": "I1", "title": "ready one", "milestone": "m", "status": "READY", "stage": "none" },
                { "issue": "I2", "title": "claimed one", "milestone": "m", "status": "READY", "stage": "none" },
                { "issue": "I3", "title": "reviewed one", "milestone": "m", "status": "ACTIVE", "stage": "pr_open",
                  "pr_open": { "number": 42, "draft": true } },
                { "issue": "I4", "title": "landed one", "milestone": "m", "status": "DONE", "stage": "released",
                  "merged": { "commit": "abcdef0123456789", "pull_request": 7, "merged_at": "2026-10-25T10:00:00Z" },
                  "released": { "tag": "v1.0.0" } },
                { "issue": "I5", "title": "blocked one", "milestone": "m", "status": "BLOCKED", "stage": "none" }
            ]}),
        );
        let issues = answer(
            ISSUES,
            serde_json::json!({ "issues": [ { "id": "I1", "priority": "p0", "wave": 2 } ] }),
        );
        let board = answer(
            BLACKBOARD,
            serde_json::json!({ "workers": [
                { "key": "mesh:a", "board_peer": "p5", "presence": "attached",
                  "intent": "do the secret thing", "claims": [{ "issue": "I2", "scope": ["secret/path"] }] }
            ]}),
        );
        let html = columns(&delivery, &issues, &board).render();
        for said in [
            ">Ready<",
            ">In flight<",
            ">In review<",
            ">Landed<",
            "/ui/prs?issue=I1#issue",
            "/ui/prs?pr=42#pr",
            "#42 draft",
            "/ui/prs?pr=7#pr",
            "v1.0.0",
            "href=\"/ui/peers\"",
            ">p5<",
            "wave 2",
        ] {
            assert!(html.contains(said), "{said:?} is not in {html}");
        }
        for never in [
            "secret/path",
            "do the secret thing",
            "blocked one",
            "mesh:a",
        ] {
            assert!(!html.contains(never), "{never:?} leaked into {html}");
        }
    }

    /// The stream links what a signal names and quotes no mesh signal's words, which carry
    /// intents and claimed paths; a merge's words are the forge's and are kept. An avatar
    /// without a board position is the client's initial and the key's tail.
    #[test]
    fn the_stream_quotes_no_intent_or_path_and_an_avatar_names_no_one() {
        let board = answer(
            BLACKBOARD,
            serde_json::json!({
                "workers": [{ "key": "mesh:s/task-t-1-ab12", "client": "claude-code", "issue": "I9" }],
                "signals": [
                    { "at": 1793000000u64, "source": "mesh", "kind": "claim_acquired", "actor": "board-p3",
                      "summary": "claims advisory secret/path: do the secret thing", "issue": "I4236" },
                    { "at": 1793000001u64, "source": "github", "kind": "merged", "pr": 2571,
                      "summary": "#2571 merged into master" }
                ]
            }),
        );
        let html = stream(&board).render();
        for said in [
            "claim acquired",
            "board-p3",
            "/ui/prs?issue=I4236#issue",
            "#2571 merged into master",
            "/ui/prs?pr=2571#pr",
        ] {
            assert!(html.contains(said), "{said:?} is not in {html}");
        }
        for never in ["secret/path", "do the secret thing"] {
            assert!(!html.contains(never), "{never:?} leaked into {html}");
        }
        assert_eq!(carriers(&board)["I9"].iter().next().unwrap().label, "C12");
    }

    /// An unobserved forge leaves the review column saying it was not read and why, never
    /// "nothing at this stage": an unread source is not an empty one.
    #[test]
    fn an_unread_forge_is_said_in_the_review_column_not_read_as_empty() {
        let delivery = answer(
            DELIVERY,
            serde_json::json!({
                "issues": [],
                "sources": [{ "name": "forge", "state": "unread", "detail": "run majordomus prs refresh" }]
            }),
        );
        let none = answer(ISSUES, serde_json::json!({ "issues": [] }));
        let board = answer(BLACKBOARD, serde_json::json!({ "workers": [] }));
        let html = columns(&delivery, &none, &board).render();
        assert!(
            html.contains("Not read, so this column may be short: run majordomus prs refresh"),
            "{html}"
        );
        assert_eq!(
            html.matches("Nothing at this stage.").count(),
            3,
            "the three columns that were read say they are empty, the unread one does not: {html}"
        );
    }

    /// The screen follows the blackboard's channel and leaves the board to its panel.
    #[test]
    fn the_screen_follows_the_blackboard_and_defers_the_board() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let html = screen(&ctx, "/ui/assets/live.js?v=1").main.render();
        assert!(
            html.contains("data-follow=\"/events?follow=blackboard&amp;frames=snapshot\""),
            "{html}"
        );
        assert!(
            html.contains("data-panel-src=\"/ui/panels/board\""),
            "{html}"
        );
        assert!(html.contains("Who carries what"), "{html}");
    }

    /// Every way a card, a column and an avatar can be drawn: a column longer than it shows
    /// says how many more; the open columns order by carried, priority, wave and id and the
    /// landed one by when it merged; a pull request that is not a draft, a branch without one,
    /// a merge without a pull request and a release each have their chip; a departed worker
    /// is drawn away, a worker with no board position and no client is `?` and its key's tail;
    /// and the graph of who carries what draws each carrier and each issue it names.
    #[test]
    fn every_card_column_and_avatar_is_drawn() {
        let mut issues_in = vec![
            serde_json::json!({ "issue": "I10", "title": "branch only", "milestone": "m",
              "status": "READY", "stage": "branch", "branch": { "name": "feature/x" } }),
            serde_json::json!({ "issue": "I11", "title": "open review", "milestone": "m",
              "status": "ACTIVE", "stage": "pr_open" }),
            serde_json::json!({ "issue": "I12", "title": "reviewed", "milestone": "m",
              "status": "ACTIVE", "stage": "pr_open", "pr_open": { "number": 99, "draft": false } }),
            serde_json::json!({ "issue": "I13", "title": "merged by commit", "milestone": "m",
              "status": "DONE", "stage": "merged",
              "merged": { "commit": "0123456789abcdef", "merged_at": "2026-10-20T10:00:00Z" } }),
            serde_json::json!({ "issue": "I14", "title": "merged later", "milestone": "m",
              "status": "DONE", "stage": "deployed",
              "merged": { "commit": "fedcba9876543210", "pull_request": 8, "merged_at": "2026-10-24T10:00:00Z" },
              "released": { "tag": "v2.0.0" } }),
        ];
        for n in 0..(SHOWN + 3) {
            issues_in.push(
                serde_json::json!({ "issue": format!("R{n:02}"), "title": format!("ready {n}"),
              "milestone": "m", "status": "READY", "stage": "none" }),
            );
        }
        let delivery = answer(DELIVERY, serde_json::json!({ "issues": issues_in }));
        let issues = answer(
            ISSUES,
            serde_json::json!({ "issues": [
                { "id": "R00", "priority": "p3", "wave": 1 },
                { "id": "R01", "priority": "p2", "wave": 1 },
                { "id": "R02", "priority": "p1", "wave": 3 },
                { "id": "R03", "priority": "p1", "wave": 2 },
                { "id": "R04", "priority": "p0", "wave": 1 },
                { "id": "R05", "priority": "later", "wave": 1 }
            ]}),
        );
        let board = answer(
            BLACKBOARD,
            serde_json::json!({ "workers": [
                { "key": "mesh:aa-b7", "presence": "departed", "issue": "R05" },
                { "key": "mesh:cc-d9", "client": "codex", "presence": "attached",
                  "claims": [{ "issue": "I10" }, { "issue": "" }] }
            ]}),
        );
        let html = columns(&delivery, &issues, &board).render();
        for said in [
            "2 more at this stage",
            "/ui/prs?pr=99#pr",
            ">#99<",
            "feature/x",
            ">0123456789a<",
            "v2.0.0",
            "mj-ui-board-avatar--away",
            ">?b7<",
            ">Cd9<",
        ] {
            assert!(html.contains(said), "{said:?} is not in {html}");
        }
        // the ready column: p0, p1 by wave, p2, p3, then one the plan ranks nothing for
        let order: Vec<usize> = [
            "card-R04", "card-R03", "card-R02", "card-R01", "card-R00", "card-R06",
        ]
        .iter()
        .map(|id| {
            html.find(id)
                .unwrap_or_else(|| panic!("{id} is not drawn in {html}"))
        })
        .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?} in {html}");
        // the landed column: the later merge first
        assert!(
            html.find("card-I14").unwrap() < html.find("card-I13").unwrap(),
            "{html}"
        );
        assert_eq!(
            lane("pr_open", "ACTIVE", false, false),
            Some(Lane::InReview)
        );

        let graph = carrying(&board, &carriers(&board)).render();
        for said in ["R05", "I10", "?b7", "Cd9"] {
            assert!(graph.contains(said), "{said:?} is not in the graph {graph}");
        }
        assert!(
            !graph.contains("mesh:"),
            "a worker's key leaked into {graph}"
        );
    }

    /// What a card does not know it does not draw: no status, an open pull request the
    /// delivery gives no number for, and a branch whose pull request is open each leave
    /// their chip out, and the card is still drawn.
    #[test]
    fn a_card_draws_nothing_it_does_not_know() {
        let c = Card {
            lane: Lane::InReview,
            id: "I20".into(),
            title: "partial".into(),
            milestone: "m".into(),
            priority: String::new(),
            wave: u64::MAX,
            status: String::new(),
            delivery: serde_json::json!({
                "pr_open": { "draft": true },
                "branch": { "name": "feature/y" }
            }),
            carriers: Vec::new(),
        };
        let html = card(&c).render();
        assert!(html.contains("card-I20"), "{html}");
        assert!(!html.contains("mj-ui-board-status"), "{html}");
        assert!(!html.contains("mj-ui-board-chip--pr"), "{html}");
        assert!(!html.contains("feature/y"), "{html}");
    }
}
