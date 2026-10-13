//! The board: every worker of this repository, every claim each one holds, the paths those
//! claims name, where two workers meet, and what the mesh holds exclusively. Read from
//! `peers.board`, which derives all of it from the boards of every checkout on every read.
//!
//! The screen draws the whole answer and counts nothing itself: every number on it is a
//! field of the source, every list is the source's in the source's order, and a list cut to
//! the screen's bound says how many it left out. It announces nothing and claims nothing: a
//! screen reads and never executes (ADR 0142).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;

use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{
    anchor, graph, hero, kpi, list, more, nothing, panel, table, text, unanswered, wide, GraphNode,
    Layout,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/board";

const BOARD: Source = Source::of("peers.board");

/// How many rows any one table of the screen draws. A board has no bound of its own: a
/// repository with sixty worktrees has sixty checkouts, and a busy day has hundreds of
/// claims. What is past the bound is counted and said.
pub const SHOWN: usize = 100;

/// The Board screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[BOARD]);
    let board = &answers[&BOARD];
    let workers = labels(board);

    let top = hero(
        "users",
        "violet",
        "Board",
        "Who is working here, on what, and where two of them meet.",
        "Every worker attached to a server of this repository, every claim each one holds, the paths those claims name, and every pair of claims whose scope meets. Derived on every read from the boards of every checkout; nothing here is stored.",
        vec![
            kpi("Workers", &board.read("/counts/workers")),
            kpi("Attached", &board.read("/counts/attached")),
            kpi("Claims", &board.read("/counts/claims")),
            kpi("Contested paths", &board.read("/counts/contested")),
            kpi("Collisions", &board.read("/counts/collisions")),
            kpi("Exclusive", &board.read("/counts/exclusive")),
            kpi("Unread boards", &board.read("/counts/unread")),
        ],
    );

    let mut grid = el("div").class("mj-ui-grid");
    if let Some(warning) = partial(board) {
        grid = grid.child(wide(warning));
    }
    let grid = grid
        .child(wide(panel(
            "Who holds what",
            "Checkouts, the workers attached to each, the claims they hold and the paths those claims name. A path two workers hold is drawn in the colour of a collision.",
            unanswered(board).unwrap_or_else(|| picture(board, &workers)),
            &[board],
        )))
        .child(wide(panel(
            "Collisions",
            "Every pair of claims of two different workers whose scope meets, both sides named, each pair once.",
            unanswered(board).unwrap_or_else(|| collisions(board, &workers)),
            &[board],
        )))
        .child(wide(panel(
            "Claims",
            "Every claim of every worker, newest first: not only what each worker said last.",
            unanswered(board).unwrap_or_else(|| claims(board, &workers)),
            &[board],
        )))
        .child(wide(panel(
            "Workers",
            "Every worker, attached first, with where it works and when its server last heard from it.",
            unanswered(board).unwrap_or_else(|| roster(board)),
            &[board],
        )))
        .child(wide(panel(
            "Paths",
            "Every path somebody named, contested first, with who named it.",
            unanswered(board).unwrap_or_else(|| paths(board, &workers)),
            &[board],
        )))
        .child(wide(panel(
            "Held exclusively",
            "The exclusive claims the mesh holds now. A board claim tells; one of these refuses a second claimant.",
            unanswered(board).unwrap_or_else(|| exclusive(board)),
            &[board],
        )))
        .child(wide(panel(
            "Checkouts",
            "Every checkout the answer covers, with where its server stands; one whose board could not be read comes first.",
            unanswered(board).unwrap_or_else(|| checkouts(board)),
            &[board],
        )));

    View::new(
        "Board",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// What a worker's key reads as on the screen: its id on its own board, its client, and the
/// branch its checkout has, so two boards' `p1` are told apart by where they work.
fn labels(board: &Answer) -> BTreeMap<String, String> {
    list(board, "/workers")
        .iter()
        .map(|worker| (text(worker, "key"), label(worker)))
        .collect()
}

fn label(worker: &Value) -> String {
    let branch = text(worker, "branch");
    let place = if branch.is_empty() {
        "a detached head".to_string()
    } else {
        branch
    };
    format!(
        "{} · {} · {place}",
        text(worker, "peer"),
        text(worker, "client")
    )
}

/// A claim's key, read through the worker it belongs to: `<worker>#<name>`.
fn claim_label(key: &str, workers: &BTreeMap<String, String>) -> String {
    let (worker, name) = key.rsplit_once('#').unwrap_or((key, ""));
    let who = workers.get(worker).map_or(worker, String::as_str);
    if name.is_empty() {
        who.to_string()
    } else {
        format!("{who} · {name}")
    }
}

/// The address of a claim's row on this screen.
fn claim_href(key: &str) -> String {
    format!("{ROUTE}#claim-{}", anchor(key))
}

/// A cell that lists strings, one to a line, or says there are none.
fn lines(items: Vec<String>, none: &str) -> El {
    if items.is_empty() {
        return el("span").class("mj-ui-dim").text(none);
    }
    el("ul")
        .class("mj-ui-prose-list")
        .children(items.into_iter().map(|item| el("li").text(item)))
}

fn strings(entry: &Value, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn flag(entry: &Value, key: &str) -> bool {
    entry.get(key).and_then(Value::as_bool) == Some(true)
}

/// A table cut to the screen's bound, with the line that says what it left out.
fn bounded(headers: &[&str], rows: Vec<El>, total: usize, empty: &str) -> El {
    let shown = rows.len();
    let body = el("div")
        .class("mj-ui-stack")
        .child(table(headers, rows, empty));
    match more(shown, total, None) {
        Some(line) => body.child(line),
        None => body,
    }
}

/// The warning a board that is not whole owes its reader: which checkouts could not be
/// asked, and why. `None` when every board was read.
fn partial(board: &Answer) -> Option<El> {
    let answer = board.ok()?;
    if answer.get("complete").and_then(Value::as_bool) != Some(false) {
        return None;
    }
    let unread: Vec<String> = list(board, "/checkouts")
        .iter()
        .filter(|checkout| !text(checkout, "reason").is_empty())
        .map(|checkout| {
            format!(
                "{}: {}",
                text(checkout, "worktree"),
                text(checkout, "reason")
            )
        })
        .collect();
    Some(panel(
        "This board is not whole",
        "A checkout's server could not be asked, so the lists below are a part of the board and a short list is not an empty repository.",
        lines(unread, "The source says the board is not whole and names no checkout."),
        &[board],
    ))
}

/// The board as a graph: checkout, worker, claim, path. The fallback is the claims with the
/// paths each names, which is the same relation as a list.
fn picture(board: &Answer, workers: &BTreeMap<String, String>) -> El {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for worker in list(board, "/workers").iter().take(SHOWN) {
        let key = text(worker, "key");
        let worktree = text(worker, "worktree");
        let checkout_id = format!("checkout:{worktree}");
        if seen.insert(checkout_id.clone()) {
            let branch = text(worker, "branch");
            nodes.push(GraphNode {
                id: checkout_id.clone(),
                label: if branch.is_empty() { worktree } else { branch },
                tone: "cyan",
                href: format!("{ROUTE}#checkouts"),
                kind: "node",
            });
        }
        let worker_id = format!("worker:{key}");
        nodes.push(GraphNode {
            id: worker_id.clone(),
            label: format!("{} {}", text(worker, "peer"), text(worker, "client")),
            tone: if flag(worker, "attached") {
                "violet"
            } else {
                "blue"
            },
            href: format!("{ROUTE}#worker-{}", anchor(&key)),
            kind: "node",
        });
        edges.push((checkout_id, worker_id));
    }
    let contested: std::collections::BTreeSet<String> = list(board, "/paths")
        .iter()
        .filter(|path| flag(path, "contested"))
        .map(|path| text(path, "path"))
        .collect();
    let all = list(board, "/claims");
    for claim in all.iter().take(SHOWN) {
        let key = text(claim, "key");
        let claim_id = format!("claim:{key}");
        let name = text(claim, "name");
        nodes.push(GraphNode {
            id: claim_id.clone(),
            label: if name.is_empty() {
                "claim".to_string()
            } else {
                name
            },
            tone: if strings(claim, "collides").is_empty() {
                "orange"
            } else {
                "rose"
            },
            href: claim_href(&key),
            kind: "node",
        });
        edges.push((
            format!("worker:{}", text(claim, "worker")),
            claim_id.clone(),
        ));
        for path in strings(claim, "scope") {
            let path_id = format!("path:{path}");
            if seen.insert(path_id.clone()) {
                nodes.push(GraphNode {
                    id: path_id.clone(),
                    tone: if contested.contains(&path) {
                        "rose"
                    } else {
                        "green"
                    },
                    label: path,
                    href: format!("{ROUTE}#paths"),
                    kind: "leaf",
                });
            }
            edges.push((claim_id.clone(), path_id));
        }
    }
    let rows = all
        .iter()
        .take(SHOWN)
        .map(|claim| {
            el("tr")
                .child(el("td").text(claim_label(&text(claim, "key"), workers)))
                .child(el("td").child(lines(strings(claim, "scope"), "no path named")))
        })
        .collect::<Vec<_>>();
    graph(
        "The board: checkouts, the workers attached to each, the claims they hold and the paths those claims name",
        Layout::Force,
        &nodes,
        &edges,
        bounded(&["Claim", "Paths"], rows, all.len(), "Nobody has announced anything."),
    )
}

fn collisions(board: &Answer, workers: &BTreeMap<String, String>) -> El {
    let all = list(board, "/collisions");
    let rows = all
        .iter()
        .take(SHOWN)
        .map(|pair| {
            let side = |key: &str| {
                let claim = text(pair, key);
                el("td").child(
                    el("a")
                        .class("mj-ui-link")
                        .attr("href", claim_href(&claim))
                        .text(claim_label(&claim, workers)),
                )
            };
            let meetings: Vec<String> = pair
                .get("paths")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|meeting| {
                    let (first, second) = (text(meeting, "first"), text(meeting, "second"));
                    if first == second {
                        first
                    } else {
                        format!("{first} meets {second}")
                    }
                })
                .collect();
            el("tr")
                .child(side("first"))
                .child(side("second"))
                .child(el("td").child(lines(meetings, "no path")))
                .child(el("td").child(word_badge(if flag(pair, "live") {
                    "live"
                } else {
                    "departed"
                })))
        })
        .collect::<Vec<_>>();
    bounded(
        &[
            "Announced first",
            "Announced later",
            "Where they meet",
            "Both here",
        ],
        rows,
        all.len(),
        "No two workers' claims meet.",
    )
}

fn claims(board: &Answer, workers: &BTreeMap<String, String>) -> El {
    let all = list(board, "/claims");
    let rows = all
        .iter()
        .take(SHOWN)
        .map(|claim| {
            let key = text(claim, "key");
            let meets = strings(claim, "collides");
            el("tr")
                .attr("id", format!("claim-{}", anchor(&key)))
                .child(
                    el("td")
                        .class("mj-ui-mono mj-ui-dim")
                        .text(text(claim, "at")),
                )
                .child(
                    el("td")
                        .child(el("span").text(claim_label(&key, workers)))
                        .child(el("span").text(" "))
                        .child(word_badge(if flag(claim, "attached") {
                            "attached"
                        } else {
                            "departed"
                        })),
                )
                .child(el("td").class("mj-ui-prose").text(text(claim, "intent")))
                .child(el("td").child(lines(strings(claim, "scope"), "no path named")))
                .child(
                    el("td").child(lines(
                        meets
                            .iter()
                            .map(|other| claim_label(other, workers))
                            .collect(),
                        "nobody",
                    )),
                )
        })
        .collect::<Vec<_>>();
    bounded(
        &["Announced", "Who", "What for", "Paths", "Meets"],
        rows,
        all.len(),
        "Nobody has announced anything.",
    )
}

fn roster(board: &Answer) -> El {
    let all = list(board, "/workers");
    let rows = all
        .iter()
        .take(SHOWN)
        .map(|worker| {
            let seen = worker
                .get("last_seen_seconds_ago")
                .and_then(Value::as_u64)
                .map_or_else(|| "not said".to_string(), ago);
            el("tr")
                .attr("id", format!("worker-{}", anchor(&text(worker, "key"))))
                .child(
                    el("td")
                        .child(el("span").class("mj-ui-mono").text(text(worker, "peer")))
                        .child(el("span").class("mj-ui-cell-title").text(format!(
                            "{} {}",
                            text(worker, "client"),
                            text(worker, "version")
                        ))),
                )
                .child(
                    el("td")
                        .child(el("span").text(match text(worker, "branch") {
                            branch if branch.is_empty() => "a detached head".to_string(),
                            branch => branch,
                        }))
                        .child(
                            el("span")
                                .class("mj-ui-cell-title mj-ui-dim")
                                .text(text(worker, "worktree")),
                        ),
                )
                .child(el("td").child(word_badge(if flag(worker, "attached") {
                    "attached"
                } else {
                    "departed"
                })))
                .child(el("td").class("mj-ui-dim").text(seen))
                .child(
                    el("td").text(
                        worker
                            .get("claims")
                            .and_then(Value::as_u64)
                            .map_or_else(|| "not said".to_string(), |n| n.to_string()),
                    ),
                )
        })
        .collect::<Vec<_>>();
    bounded(
        &["Worker", "Where", "Standing", "Last heard", "Claims"],
        rows,
        all.len(),
        "No worker is attached to any server of this repository.",
    )
}

fn paths(board: &Answer, workers: &BTreeMap<String, String>) -> El {
    let all = list(board, "/paths");
    let rows = all
        .iter()
        .take(SHOWN)
        .map(|path| {
            let held_by: Vec<String> = strings(path, "workers")
                .iter()
                .map(|key| workers.get(key).cloned().unwrap_or_else(|| key.clone()))
                .collect();
            el("tr")
                .child(el("td").class("mj-ui-mono").text(text(path, "path")))
                .child(el("td").child(word_badge(if flag(path, "contested") {
                    "contested"
                } else {
                    "one worker"
                })))
                .child(el("td").child(lines(held_by, "nobody")))
                .child(el("td").text(strings(path, "claims").len().to_string()))
        })
        .collect::<Vec<_>>();
    bounded(
        &["Path", "Held by", "Workers", "Claims"],
        rows,
        all.len(),
        "No claim names a path.",
    )
}

fn exclusive(board: &Answer) -> El {
    let Some(answer) = board.ok() else {
        return nothing("The board did not answer.");
    };
    if answer.get("mesh").and_then(Value::as_bool) != Some(true) {
        let why = text(answer, "mesh_reason");
        return nothing(&if why.is_empty() {
            "The mesh is not active in this checkout, so nothing can hold an exclusive claim."
                .to_string()
        } else {
            format!("The mesh is not active in this checkout: {why}")
        });
    }
    let all = list(board, "/exclusive");
    let rows = all
        .iter()
        .take(SHOWN)
        .map(|claim| {
            let what = [text(claim, "intent"), text(claim, "issue")]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            el("tr")
                .child(el("td").class("mj-ui-mono").text(text(claim, "session")))
                .child(el("td").child(lines(strings(claim, "scope"), "no path")))
                .child(el("td").class("mj-ui-prose").text(if what.is_empty() {
                    "not said".to_string()
                } else {
                    what
                }))
                .child(el("td").text(strings(claim, "meets").len().to_string()))
        })
        .collect::<Vec<_>>();
    bounded(
        &[
            "Session",
            "Excludes others from",
            "What for",
            "Board claims under it",
        ],
        rows,
        all.len(),
        "The mesh holds no exclusive claim now.",
    )
}

fn checkouts(board: &Answer) -> El {
    let mut all = list(board, "/checkouts");
    // unread first, then the ones with somebody attached; the source's order otherwise
    all.sort_by_key(|checkout| {
        (
            text(checkout, "reason").is_empty(),
            checkout.get("attached").and_then(Value::as_u64) == Some(0),
        )
    });
    let rows = all
        .iter()
        .take(SHOWN)
        .map(|checkout| {
            let reason = text(checkout, "reason");
            el("tr")
                .child(
                    el("td")
                        .child(el("span").text(match text(checkout, "branch") {
                            branch if branch.is_empty() => "a detached head".to_string(),
                            branch => branch,
                        }))
                        .child(
                            el("span")
                                .class("mj-ui-cell-title mj-ui-dim")
                                .text(text(checkout, "worktree")),
                        ),
                )
                .child(el("td").child(word_badge(&text(checkout, "standing"))))
                .child(
                    el("td").text(
                        checkout
                            .get("attached")
                            .and_then(Value::as_u64)
                            .map_or_else(|| "not said".to_string(), |n| n.to_string()),
                    ),
                )
                .child(el("td").class("mj-ui-dim").text(if reason.is_empty() {
                    "read".to_string()
                } else {
                    reason
                }))
        })
        .collect::<Vec<_>>();
    el("div").attr("id", "checkouts").child(bounded(
        &["Checkout", "Its server", "Attached", "Its board"],
        rows,
        all.len(),
        "The answer covers no checkout.",
    ))
}

/// Seconds as the largest whole unit: how long ago a server last heard from a worker.
fn ago(seconds: u64) -> String {
    match seconds {
        s if s >= 2 * 24 * 3600 => format!("{} d ago", s / (24 * 3600)),
        s if s >= 2 * 3600 => format!("{} h ago", s / 3600),
        s if s >= 120 => format!("{} min ago", s / 60),
        s => format!("{s} s ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;
    use crate::ui::source::Standing;
    use serde_json::json;

    fn answer(value: Value) -> Answer {
        Answer {
            source: BOARD,
            route: None,
            exposure: Default::default(),
            value: Ok(value),
            took: std::time::Duration::ZERO,
            standing: Standing::NOW,
        }
    }

    fn board() -> Answer {
        answer(json!({
            "complete": false,
            "counts": { "workers": 2, "attached": 1, "claims": 2, "contested": 2, "collisions": 1, "exclusive": 1, "unread": 1 },
            "workers": [
                { "key": "aaa/p1", "peer": "p1", "client": "claude-code", "version": "2", "worktree": "/r", "branch": "master", "attached": true, "last_seen_seconds_ago": 7, "claims": 1 },
                { "key": "bbb/p1", "peer": "p1", "client": "codex", "version": "1", "worktree": "/r-wt/x", "attached": false, "last_seen_seconds_ago": 7300, "claims": 1 },
            ],
            "claims": [
                { "key": "aaa/p1#ui", "worker": "aaa/p1", "name": "ui", "intent": "the screen", "scope": ["src/ui"], "at": "2026-10-13T10:05:00Z", "attached": true, "collides": ["bbb/p1#"] },
                { "key": "bbb/p1#", "worker": "bbb/p1", "intent": "one file", "scope": ["src/ui/mesh.rs"], "at": "2026-10-13T10:01:00Z", "attached": false, "collides": ["aaa/p1#ui"] },
            ],
            "paths": [
                { "path": "src/ui", "claims": ["aaa/p1#ui"], "workers": ["aaa/p1"], "contested": true },
                { "path": "src/ui/mesh.rs", "claims": ["bbb/p1#"], "workers": ["bbb/p1"], "contested": true },
            ],
            "collisions": [
                { "first": "bbb/p1#", "second": "aaa/p1#ui", "live": false, "paths": [{ "first": "src/ui/mesh.rs", "second": "src/ui" }] },
            ],
            "mesh": true,
            "exclusive": [
                { "key": "s/c-1", "runtime": "r", "session": "landing", "scope": ["campaign/landing"], "intent": "the landing", "meets": [] },
            ],
            "checkouts": [
                { "id": "aaa", "worktree": "/r", "branch": "master", "this_checkout": true, "standing": "ready", "attached": 1 },
                { "id": "ccc", "worktree": "/r-wt/y", "branch": "fix/y", "this_checkout": false, "standing": "starting", "attached": 0, "reason": "its server is starting" },
            ],
        }))
    }

    /// The screen reads the one source, draws every panel, and offers no control.
    #[test]
    fn the_screen_reads_the_board_and_runs_nothing() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let view = screen(&ctx);
        assert_eq!(view.title, "Board");
        let html = view.main.render();
        for want in [
            "Who holds what",
            "Collisions",
            "Claims",
            "Workers",
            "Paths",
            "Held exclusively",
            "Checkouts",
            "peers.board",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        for control in ["<form", "<button", "method=\"post\""] {
            assert!(
                !html.contains(control),
                "the screen carries {control}: {html}"
            );
        }
    }

    /// Two boards' `p1` are told apart by where they work, a collision names both sides and
    /// leads to both claims, and a claim that left is said to have left.
    #[test]
    fn a_collision_names_both_sides_and_leads_to_both_claims() {
        let board = board();
        let workers = labels(&board);
        assert_eq!(workers["aaa/p1"], "p1 · claude-code · master");
        assert_eq!(workers["bbb/p1"], "p1 · codex · a detached head");
        let html = collisions(&board, &workers).render();
        assert!(html.contains("p1 · codex · a detached head"), "{html}");
        assert!(html.contains("p1 · claude-code · master · ui"), "{html}");
        assert!(html.contains("src/ui/mesh.rs meets src/ui"), "{html}");
        assert!(html.contains("departed"), "{html}");
        for claim in ["aaa/p1#ui", "bbb/p1#"] {
            assert!(
                html.contains(&claim_href(claim)),
                "{claim} leads nowhere: {html}"
            );
        }
        let rows = claims(&board, &workers).render();
        for claim in ["aaa/p1#ui", "bbb/p1#"] {
            assert!(
                rows.contains(&format!("id=\"claim-{}\"", anchor(claim))),
                "{claim} has no row to lead to: {rows}"
            );
        }
    }

    /// A board that is not whole says so first and names the checkout and the reason; a whole
    /// one carries no warning.
    #[test]
    fn a_board_that_is_not_whole_says_which_checkout_and_why() {
        let warning = partial(&board()).expect("the board is not whole").render();
        assert!(warning.contains("This board is not whole"), "{warning}");
        assert!(
            warning.contains("/r-wt/y: its server is starting"),
            "{warning}"
        );
        assert!(partial(&answer(json!({ "complete": true }))).is_none());
        let listed = checkouts(&board()).render();
        let unread = listed
            .find("its server is starting")
            .expect("the unread checkout");
        let read = listed.find(">read<").expect("the read checkout");
        assert!(unread < read, "the unread checkout comes first: {listed}");
    }

    /// The picture relates checkout, worker, claim and path, colours a contested path as a
    /// collision, and its fallback is the same relation as a table.
    #[test]
    fn the_picture_is_the_claims_and_their_paths_twice() {
        let board = board();
        let html = picture(&board, &labels(&board)).render();
        let data = html
            .split("data-graph=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the graph carries its data")
            .replace("&quot;", "\"");
        let graph: Value = serde_json::from_str(&data).expect("the data parses");
        let ids: Vec<&str> = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["id"].as_str())
            .collect();
        for want in [
            "checkout:/r",
            "worker:aaa/p1",
            "claim:aaa/p1#ui",
            "path:src/ui",
            "path:src/ui/mesh.rs",
        ] {
            assert!(ids.contains(&want), "no node {want}: {ids:?}");
        }
        let contested = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "path:src/ui")
            .unwrap();
        assert_eq!(contested["tone"], "rose");
        assert_eq!(graph["edges"].as_array().unwrap().len(), 6);
        assert!(html.contains("mj-ui-graph-fallback"), "{html}");
    }

    /// An inactive mesh is said with its reason, an empty board in words, and a time as its
    /// largest whole unit.
    #[test]
    fn an_inactive_mesh_and_an_empty_board_are_said_in_words() {
        let off = answer(json!({ "mesh": false, "mesh_reason": "no identity is enrolled" }));
        assert!(exclusive(&off)
            .render()
            .contains("The mesh is not active in this checkout: no identity is enrolled"));
        let empty = answer(json!({ "mesh": true }));
        assert!(exclusive(&empty)
            .render()
            .contains("The mesh holds no exclusive claim now."));
        let workers = BTreeMap::new();
        assert!(claims(&empty, &workers)
            .render()
            .contains("Nobody has announced anything."));
        assert!(
            collisions(&empty, &workers)
                .render()
                .contains("No two workers&#39; claims meet.")
                || collisions(&empty, &workers)
                    .render()
                    .contains("No two workers' claims meet.")
        );
        assert_eq!(ago(45), "45 s ago");
        assert_eq!(ago(600), "10 min ago");
        assert_eq!(ago(3 * 3600), "3 h ago");
        assert_eq!(ago(3 * 24 * 3600 + 5), "3 d ago");
    }
}
