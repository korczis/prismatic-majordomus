//! Intent workspace: one intent and everything that serves it — its criteria, its
//! milestones, their issues with what blocks each, the waves those issues occupy and the
//! dependency graph between them — with every ready issue one link from the move that
//! starts it, and beside it the plan's own waves with what is still open in each. Read from
//! `intents.list`, `plan.roadmap`, `plan.issues`, `plan.waves` and `plan.next`.
//!
//! The screen decides nothing. Which issues belong to an intent is the plan's own relation:
//! the issues of the milestones the intent names, and any issue whose `serves` names it.
//! Status, wave, blockers and the next issue are the capabilities' words, drawn as given; a
//! new intent, milestone or issue appears here with no edit but its own record. The one
//! choice the screen makes is which intent to open when the link names none, and that rule
//! ([`PREFERRED_STAGES`]) is printed on the screen.
//!
//! The screen at `/ui/idd?intent=<id>` reads five capabilities — `intents.list`,
//! `plan.roadmap`, `plan.issues`, `plan.waves` and `plan.next` — and links to the milestone,
//! issue and criterion inspectors; the move itself (`plan.transition`) is offered on the
//! issue page.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::dashboard::{milestone_row, ROWS};
use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{
    graph, hero, hero_under, kpi, kpi_count, list, more, nothing, panel, panel_to, state,
    status_tone, table, text, unanswered, value, wide, GraphNode, Layout,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/idd";

const INTENTS: Source = Source::of("intents.list");
const ROADMAP: Source = Source::of("plan.roadmap");
const ISSUES: Source = Source::of("plan.issues");
const WAVES: Source = Source::of("plan.waves");
const NEXT: Source = Source::of("plan.next");
/// One intent in full, asked by id for the headline numbers (I2907).
const RECORD: &str = "intents.record";
/// Every move the plan's issues were given on this server: `plan.transition` as executions.
const MOVES: Source = Source::with("executions.list", r#"{"capability":"plan.transition"}"#);

/// The route that answers why the open intent stands where it stands: its own route, because
/// `intent_realization.explain` takes seconds and the screen's first paint must not wait for
/// it (I2789).
pub const STANDING_PANEL: &str = "/ui/panels/standing";

/// The stages, in the order the screen prefers them when the link names no intent: the
/// first intent `intents.list` reports at the earliest of these is opened. A presentation
/// rule, printed on the screen, never a status.
pub const PREFERRED_STAGES: &[&str] = &["executing", "verifying", "planned", "declared"];

/// The question the standing panel answers, its heading and its accessible name.
pub const STANDING_TITLE: &str = "Why does it stand where it stands?";

/// Where the workspace of intent `id` is.
pub fn href(id: &str) -> String {
    format!("{ROUTE}?intent={id}")
}

fn issue_href(id: &str) -> String {
    format!("{}?issue={id}#issue", super::prs::ROUTE)
}

/// Where the move of issue `id` is offered: this screen, with the intent kept and the issue
/// picked, at its move panel (I2781).
pub fn move_href(intent: &str, id: &str) -> String {
    format!("{ROUTE}?intent={intent}&issue={id}#move")
}

/// The issue a query picks, when it names one: the plan's ids are letters, digits, `-` and
/// `_`, and nothing a link could carry anything else in.
pub fn picked_issue(query: &[(String, String)]) -> Option<String> {
    query
        .iter()
        .find(|(k, _)| k == "issue")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| {
            !v.is_empty()
                && v.len() <= 64
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

fn strings(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The intent to open: the one the query names, else the first of `intents` at the earliest
/// of [`PREFERRED_STAGES`], else the first. `None` when there is no intent, and the named
/// id as given when the query names one, so that an unknown id is refused by name rather
/// than silently replaced.
pub fn chosen(query: &[(String, String)], intents: &[Value]) -> Option<String> {
    if let Some(id) = super::intent::picked(query) {
        return Some(id);
    }
    PREFERRED_STAGES
        .iter()
        .find_map(|stage| intents.iter().find(|i| text(i, "stage") == *stage))
        .or_else(|| intents.first())
        .map(|i| text(i, "id"))
}

/// The issues that serve intent `id`: those of the milestones it names and those whose
/// `serves` names it, in the plan's own order of `plan.issues`.
pub fn serving(intent: &Value, issues: &[Value]) -> Vec<Value> {
    let id = text(intent, "id");
    let milestones: BTreeSet<String> = intent
        .get("milestones")
        .and_then(Value::as_array)
        .map(|ms| ms.iter().map(|m| text(m, "id")).collect())
        .unwrap_or_default();
    issues
        .iter()
        .filter(|x| {
            milestones.contains(&text(x, "milestone"))
                || strings(x, "serves")
                    .iter()
                    .any(|s| super::intent::intent_of(s) == id)
        })
        .cloned()
        .collect()
}

fn chip(href: String, label: String) -> El {
    el("a")
        .class("mj-ui-chip mj-ui-chip--link")
        .attr("href", href)
        .child(el("code").class("mj-ui-chip-value").text(label))
}

/// The Intent workspace screen; `query` may name the intent (`?intent=intent-bound-work`).
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let answers = ask_all(ctx, &[INTENTS, ROADMAP, ISSUES, WAVES, NEXT, MOVES]);
    // the chosen intent in full, asked by its id, so the headline numbers are its own (I2907)
    let record = chosen(query, &list(&answers[&INTENTS], "/intents"))
        .map(|id| super::source::ask_input(ctx, RECORD, serde_json::json!({ "id": id })));
    render(
        ctx,
        record.as_ref(),
        &answers[&INTENTS],
        &answers[&ROADMAP],
        &answers[&ISSUES],
        &answers[&WAVES],
        &answers[&NEXT],
        &answers[&MOVES],
        query,
    )
}

/// The headline numbers: the chosen intent's own, read from `intents.record` (its stage, its
/// verdict, how many of its criteria are met of how many, how many milestones it names), so
/// nothing at the top of the screen counts outside the intent on it (I2907); the plan's next
/// issue leads to it. With no intent chosen there is only how many intents there are to choose.
fn headline(record: Option<&Answer>, intents: &Answer, next: &Answer) -> Vec<El> {
    let next_tile = match next.read("/issue/id").text().filter(|n| !n.is_empty()) {
        Some(n) => super::view::kpi_to(
            kpi("Next issue to start", &next.read("/issue/id")),
            &issue_href(&n),
        ),
        None => kpi("Next issue to start", &next.read("/issue/id")),
    };
    match record {
        Some(r) => vec![
            super::view::kpi_state("Stage", &r.read("/stage")),
            super::view::kpi_state("Verdict", &r.read("/verdict/state")),
            kpi("Criteria met", &r.read("/met")),
            kpi_count("Criteria", &r.read("/satisfaction")),
            kpi_count("Its milestones", &r.read("/milestones")),
            next_tile,
        ],
        None => vec![kpi_count("Intents", &intents.read("/intents")), next_tile],
    }
}

/// The workspace drawn from the six answers, whichever of them could not answer.
#[allow(clippy::too_many_arguments)]
fn render(
    ctx: &Context,
    record: Option<&Answer>,
    intents: &Answer,
    roadmap: &Answer,
    issues: &Answer,
    waves: &Answer,
    next: &Answer,
    moves: &Answer,
    query: &[(String, String)],
) -> View {
    let all = list(intents, "/intents");
    let id = chosen(query, &all);
    let intent = id
        .as_deref()
        .and_then(|id| all.iter().find(|i| text(i, "id") == id));

    let tagline = "From what the work is for to the move that starts it.";
    let lead = "One intent and everything that serves it: its milestones, their issues and what blocks them, the waves the dependencies allow and the graph of what waits for what.";
    let kpis = headline(record, intents, next);
    // the chosen intent's title is the page's heading, the screen's name its eyebrow (I3001)
    let top = match intent.map(|i| text(i, "title")).filter(|t| !t.is_empty()) {
        Some(title) => hero_under(
            "Intent workspace",
            "target",
            "violet",
            &title,
            tagline,
            lead,
            kpis,
        ),
        None => hero("target", "violet", "Intent workspace", tagline, lead, kpis),
    };

    let mut grid = el("div")
        .class("mj-ui-grid")
        .child(wide(choice(intents, id.as_deref())));
    match (id.as_deref(), intent) {
        (None, _) => {}
        (Some(id), None) => {
            if unanswered(intents).is_none() {
                grid = grid.child(wide(panel(
                    "Which intent?",
                    "The intent the link names.",
                    nothing(&format!(
                        "{id} is not an intent of this repository: pick one above."
                    )),
                    &[intents],
                )));
            }
        }
        (Some(id), Some(intent)) => {
            let mine = if unanswered(issues).is_none() {
                serving(intent, &list(issues, "/issues"))
            } else {
                Vec::new()
            };
            // the order a person asks in (I2916): what the intent is, the one move it asks
            // for now (I2917), what it still misses, the move picked, the work that serves
            // it, and last what was done here
            grid = grid
                .child(wide(super::intent::inspector_closing_to(
                    intents, issues, id, ROUTE,
                )))
                .child(wide(primary_action(issues, next, &mine, id)))
                .child(wide(standing_placeholder(id)));
            if let Some(issue) = picked_issue(query) {
                grid = grid.child(wide(move_panel(ctx, issues, &mine, &issue)));
            }
            grid = grid
                .child(wide(milestones(roadmap, intent)))
                .child(wide(issue_table(issues, waves, next, &mine, id)))
                .child(wide(wave_panel(waves, &mine, &known(issues, roadmap))))
                .child(wide(dependency_graph(issues, &mine)))
                .child(wide(moves_panel(moves, &mine)));
        }
    }
    // the whole plan's waves, whichever intent is open or none: what may run at the same
    // time and what is still open in each, the answer #1184's separate screen drew
    grid = grid.child(wide(plan_waves(waves, &known(issues, roadmap))));

    // the screen follows the executions this server runs and reads itself again when one
    // ends, so a move made anywhere on this server shows here without waiting for the
    // minute's refresh (I2783); what it shows is still only what its sources answer
    View::new(
        "Intent workspace",
        el("div")
            .class("mj-ui-stack")
            .attr("data-follows-executions", "")
            .child(top)
            .child(grid),
    )
}

/// Every intent as a link to its workspace, the open one marked, with the rule that chose
/// it when the link named none.
fn choice(intents: &Answer, open: Option<&str>) -> El {
    let body = unanswered(intents).unwrap_or_else(|| {
        let rows = list(intents, "/intents")
            .iter()
            .map(|i| {
                let id = text(i, "id");
                let current = open == Some(id.as_str());
                el("tr")
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-row-link")
                                .attr("href", href(&id))
                                .attr_if("aria-current", current.then_some("page"))
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()))
                                .child(el("span").class("mj-ui-cell-title").text(text(i, "title"))),
                        ),
                    )
                    .child(el("td").child(state(i, "stage")))
                    .child(el("td").child(super::intent::verdict(i)))
            })
            .collect::<Vec<_>>();
        if rows.is_empty() {
            return nothing(
                "No intent is recorded. Propose one with `majordomus intent propose`, or the intents.propose capability.",
            );
        }
        el("div")
            .class("mj-ui-stack")
            .child(table(&["Intent", "Stage", "Verdict"], rows, ""))
            .child(el("p").class("mj-ui-dim").text(format!(
                "A link that names no intent opens the first intent whose stage is {}, in that order.",
                PREFERRED_STAGES.join(", then ")
            )))
    });
    panel(
        "Which intent?",
        "What the work is for: pick one to see what serves it.",
        body,
        &[intents],
    )
}

/// The milestones the intent names, in the roadmap's order, and any it names that the plan
/// does not hold.
fn milestones(roadmap: &Answer, intent: &Value) -> El {
    let named: Vec<String> = intent
        .get("milestones")
        .and_then(Value::as_array)
        .map(|ms| ms.iter().map(|m| text(m, "id")).collect())
        .unwrap_or_default();
    let body = unanswered(roadmap).unwrap_or_else(|| {
        let plan = list(roadmap, "/milestones");
        let mut rows: Vec<El> = plan
            .iter()
            .filter(|m| named.contains(&text(m, "id")))
            .map(|m| {
                let waits: Vec<String> = strings(m, "blocked_by");
                let row = milestone_row(m);
                row.child(el("td").child(if waits.is_empty() {
                    el("span").class("mj-ui-dim").text("nothing")
                } else {
                    el("p").class("mj-ui-badges").children(
                        waits
                            .into_iter()
                            .map(|w| chip(super::milestone::href(&w), w)),
                    )
                }))
            })
            .collect();
        let held: BTreeSet<String> = plan.iter().map(|m| text(m, "id")).collect();
        rows.extend(named.iter().filter(|m| !held.contains(*m)).map(|m| {
            el("tr")
                .child(el("td").class("mj-ui-mono").text(m.clone()))
                .child(el("td").child(state(&Value::Null, "status")))
                .child(el("td").class("mj-ui-dim").text("not in the plan"))
                .child(el("td"))
        }));
        table(
            &["Milestone", "State", "Issues done", "Waits for"],
            rows,
            "The intent names no milestone.",
        )
    });
    panel_to(
        "Which milestones carry it?",
        "The milestones this intent names, in the roadmap's order; issues done is a count, not the milestone's acceptance.",
        Some(super::roadmap::ROUTE),
        body,
        &[roadmap],
    )
}

/// The issues in the order `plan.waves` lays them out — wave by wave, each wave in the
/// capability's own order — and any it does not lay out after them in `plan.issues`'
/// order. The screen sorts nothing: the order is the plan's.
pub fn in_wave_order<'a>(waves: &Answer, mine: &'a [Value]) -> Vec<&'a Value> {
    let by_id: BTreeMap<String, &Value> = mine.iter().map(|x| (text(x, "id"), x)).collect();
    let mut seen = BTreeSet::new();
    let mut ordered: Vec<&Value> = super::view::items(waves, "/waves")
        .iter()
        .flat_map(|w| {
            w.get("issues")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
        })
        .filter_map(|x| by_id.get(&text(x, "id")).copied())
        .filter(|x| seen.insert(text(x, "id")))
        .collect();
    ordered.extend(mine.iter().filter(|x| !seen.contains(&text(x, "id"))));
    ordered
}

/// The issues that serve the intent, in the plan's wave order, each with what blocks it and
/// the way to its move.
fn issue_table(issues: &Answer, waves: &Answer, next: &Answer, mine: &[Value], intent: &str) -> El {
    let next_id = next
        .ok()
        .and_then(|v| v.pointer("/issue/id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let body = unanswered(issues).unwrap_or_else(|| {
        let rows = in_wave_order(waves, mine)
            .into_iter()
            .map(|x| {
                let id = text(x, "id");
                let status = text(x, "status").to_ascii_uppercase();
                let blocked_by = strings(x, "blocked_by");
                let mut title = el("a")
                    .class("mj-ui-row-link")
                    .attr("href", issue_href(&id))
                    .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()))
                    .child(el("span").class("mj-ui-cell-title").text(text(x, "title")));
                if next_id.as_deref() == Some(id.as_str()) {
                    title = title.child(el("span").class("mj-ui-dim").text(" — the plan's next"));
                }
                let mv = move_of(&status);
                el("tr")
                    .attr("id", format!("issue-{id}"))
                    .child(el("td").child(title))
                    .child(el("td").child(chip(
                        super::milestone::href(&text(x, "milestone")),
                        text(x, "milestone"),
                    )))
                    .child(el("td").child(state(x, "status")))
                    .child(
                        el("td")
                            .class("mj-ui-mono")
                            .text(x.get("wave").map(Value::to_string).unwrap_or_default()),
                    )
                    .child(el("td").class("mj-ui-mono").text(text(x, "priority")))
                    .child(el("td").child(if blocked_by.is_empty() {
                        el("span").class("mj-ui-dim").text("nothing")
                    } else {
                        el("p")
                            .class("mj-ui-badges")
                            .children(blocked_by.into_iter().map(|b| chip(issue_href(&b), b)))
                    }))
                    .child(
                        el("td").child(match mv {
                            Some(word) => el("a")
                                .class("mj-ui-more")
                                .attr("href", move_href(intent, &id))
                                .attr(
                                    "title",
                                    format!("the move plan.transition makes: {word} {id}"),
                                )
                                .text(word),
                            None => el("span").class("mj-ui-dim").text("none"),
                        }),
                    )
            })
            .collect::<Vec<_>>();
        table(
            &[
                "Issue",
                "Milestone",
                "Status",
                "Wave",
                "Priority",
                "Blocked by",
                "Move",
            ],
            rows,
            "No issue serves this intent yet.",
        )
    });
    panel_to(
        "What work serves it?",
        "Every issue of its milestones or naming it, by wave; a move opens its panel on this screen, where the plan makes it, or refuses it and says why.",
        Some(super::issues::ROUTE),
        body,
        &[issues, waves, next],
    )
}

/// The move a status admits, in `plan.transition`'s words: a READY issue is started, an
/// ACTIVE one verified, one awaiting its verification done; any other status admits none.
fn move_of(status: &str) -> Option<&'static str> {
    match status.to_ascii_uppercase().as_str() {
        "READY" => Some("start"),
        "ACTIVE" => Some("verify"),
        "VERIFY" => Some("done"),
        _ => None,
    }
}

/// The one move the intent asks for now (I2917): `plan.next` restricted to the intent's
/// issues. The plan's own next issue when it serves the intent; otherwise the plan's ranking
/// (lowest wave, then priority, then id) over the intent's READY issues, and when none is
/// ready, the same ranking over the issues already under way. With nothing to move, none.
pub fn primary_move<'a>(next: &Answer, mine: &'a [Value]) -> Option<(&'static str, &'a Value)> {
    let movable = || {
        mine.iter()
            .filter_map(|x| move_of(&text(x, "status")).map(|m| (m, x)))
    };
    let rank = |(_, x): &(&'static str, &Value)| {
        (
            x.get("wave").and_then(Value::as_u64).unwrap_or(u64::MAX),
            crate::plan::priority_rank(&text(x, "priority")),
            text(x, "id"),
        )
    };
    let named = next
        .ok()
        .and_then(|v| v.pointer("/issue/id"))
        .and_then(Value::as_str);
    movable()
        .find(|(_, x)| Some(text(x, "id").as_str()) == named)
        .or_else(|| movable().filter(|(m, _)| *m == "start").min_by_key(rank))
        .or_else(|| movable().min_by_key(rank))
}

/// The words of the primary action: the move and the issue it is made on.
fn action_label(word: &str, id: &str) -> String {
    match word {
        "start" => format!("Start {id}"),
        "verify" => format!("Verify {id}"),
        _ => format!("Mark {id} done"),
    }
}

/// The intent's one primary action (I2917): the move [`primary_move`] picks, drawn as the
/// page's one primary button, which opens that issue's move panel on this screen. Every move
/// in the table of its work stays secondary. An intent with nothing to move says so.
fn primary_action(issues: &Answer, next: &Answer, mine: &[Value], intent: &str) -> El {
    let body = unanswered(issues).unwrap_or_else(|| match primary_move(next, mine) {
        None => nothing(
            "No issue of this intent is ready, under way or awaiting its verification: there is nothing to move.",
        ),
        Some((word, x)) => {
            let id = text(x, "id");
            el("div")
                .class("mj-ui-stack")
                .child(
                    el("p")
                        .class("mj-ui-badges")
                        .child(
                            el("a")
                                .class("mj-btn mj-btn--primary")
                                .attr("href", move_href(intent, &id))
                                .attr(
                                    "title",
                                    format!("the move plan.transition makes: {word} {id}"),
                                )
                                .text(action_label(word, &id)),
                        )
                        .child(state(x, "status"))
                        .child(el("span").class("mj-ui-cell-title").text(text(x, "title"))),
                )
                .child(el("p").class("mj-ui-dim").text(
                    "Picked the way the plan picks its next issue: its own next issue when it serves this intent, else the lowest wave, then priority, then id, with ready work before work under way.",
                ))
        }
    });
    el("div").attr("id", "next-move").child(panel(
        "What should happen next?",
        "The one move this intent asks for now; every other move is in the table of its work.",
        body,
        &[issues, next],
    ))
}

/// The move panel of the picked issue: the form the Cockpit's issue page renders, offering
/// the moves `plan.transition` makes as executions (start, verify, done), wired by the one
/// module both surfaces load. The screen decides no move: the capability refuses one the
/// plan does not allow and names why, and the form shows that answer as it came. An issue
/// that does not serve the intent is refused by name rather than moved from here.
fn move_panel(ctx: &Context, issues: &Answer, mine: &[Value], issue: &str) -> El {
    let body =
        unanswered(issues).unwrap_or_else(|| match mine.iter().find(|x| text(x, "id") == issue) {
            None => nothing(&format!(
                "{issue} does not serve this intent: pick one of its issues below."
            )),
            Some(x) => el("div")
                .class("mj-ui-stack")
                .child(
                    el("p")
                        .class("mj-ui-badges")
                        .child(chip(issue_href(issue), issue.to_string()))
                        .child(state(x, "status"))
                        .child(el("span").class("mj-ui-cell-title").text(text(x, "title"))),
                )
                .child(crate::cockpit::plan::moves(
                    ctx,
                    issue,
                    text(x, "status").eq_ignore_ascii_case("READY"),
                )),
        });
    el("div").attr("id", "move").child(panel(
        "What does this move do?",
        "The issue's next move, made as an execution on this server; a move the plan does not allow is refused with the reason.",
        body,
        &[issues],
    ))
}

/// The waves the intent's issues occupy, as `plan.waves` layers the whole plan, and the
/// scope conflicts among them that serialise issues of one wave.
fn wave_panel(waves: &Answer, mine: &[Value], known: &super::prose::Known) -> El {
    let ids: BTreeSet<String> = mine.iter().map(|x| text(x, "id")).collect();
    let body = unanswered(waves).unwrap_or_else(|| {
        let rows = list(waves, "/waves")
            .iter()
            .filter_map(|w| {
                let members: Vec<Value> = w
                    .get("issues")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter(|x| ids.contains(&text(x, "id")))
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                if members.is_empty() {
                    return None;
                }
                Some(
                    el("tr")
                        .child(
                            el("td")
                                .class("mj-ui-mono")
                                .text(w.get("wave").map(Value::to_string).unwrap_or_default()),
                        )
                        .child(el("td").child(el("p").class("mj-ui-badges").children(
                            members.iter().map(|x| {
                                el("span")
                                    .child(chip(issue_href(&text(x, "id")), text(x, "id")))
                                    .child(state(x, "status"))
                            }),
                        ))),
                )
            })
            .collect::<Vec<_>>();
        let conflicts: Vec<El> = list(waves, "/serialised_by_scope")
            .iter()
            .filter(|f| ids.contains(&text(f, "subject")))
            .map(|f| {
                let subject = text(f, "subject");
                el("li")
                    .child(chip(issue_href(&subject), subject))
                    .child(el("span").text(" "))
                    .nodes(super::prose::inline(&text(f, "message"), known))
            })
            .collect();
        let mut body = el("div").class("mj-ui-stack").child(table(
            &["Wave", "Issues of this intent in it"],
            rows,
            "No issue of this intent is in a wave.",
        ));
        if !conflicts.is_empty() {
            // one line says how many pairs share a scope; the pairs are one click away, so they
            // never outweigh the waves they qualify (I2900)
            let pairs = conflicts.len();
            body = body.child(
                el("details")
                    .child(el("summary").text(format!(
                        "Shared scope: {pairs} pair(s) of these issues may not run at the same time"
                    )))
                    .child(el("ul").class("mj-ui-prose-list").children(conflicts)),
            );
        }
        body
    });
    panel(
        "What can run together?",
        "An issue enters a wave once every dependency has left it; sharing a wave is necessary to run together, not sufficient.",
        body,
        &[waves],
    )
}

/// Whether an issue is still to be done: anything the plan has neither finished nor given up.
fn is_open(issue: &Value) -> bool {
    !matches!(
        text(issue, "status").to_ascii_uppercase().as_str(),
        "DONE" | "CANCELLED"
    )
}

/// The whole plan's waves, as `plan.waves` layers it and whichever intent is open: a line per
/// wave with how many issues it holds and how many are still open, those open issues one click
/// away, and the scope conflicts that serialise issues of one wave across the plan. The first
/// wave with open work is unfolded; a done or cancelled issue counts in its wave's size and is
/// not listed. What #1184's separate `/ui/waves` drew, folded into the workspace.
fn plan_waves(waves: &Answer, known: &super::prose::Known) -> El {
    let body = unanswered(waves).unwrap_or_else(|| {
        let all = list(waves, "/waves");
        if all.is_empty() {
            return nothing("The plan has no wave.");
        }
        let mut unfolded = false;
        let mut body = el("div").class("mj-ui-stack");
        for wave in &all {
            let (summary, open, issues) = open_wave(wave);
            let unfold = open > 0 && !unfolded;
            unfolded |= unfold;
            body = body.child(
                el("details")
                    .attr_if("open", unfold.then_some(""))
                    .child(el("summary").text(summary))
                    .child(issues),
            );
        }
        body.child(plan_conflicts(waves, known))
    });
    panel(
        "What is still open in each wave of the plan?",
        "The whole plan in the order its dependencies allow: an issue of a wave can start once the waves before it are done.",
        body,
        &[waves],
    )
}

/// One wave of the plan: the line naming it with its size and what is still open in it, how
/// many that is, and the open issues, each leading to its inspector, bounded at [`ROWS`]. A
/// wave without a number is `unknown`, never a made-up one.
fn open_wave(wave: &Value) -> (String, usize, El) {
    let number = wave
        .get("wave")
        .and_then(Value::as_u64)
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unknown".into());
    let issues: Vec<Value> = wave
        .get("issues")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let open: Vec<&Value> = issues.iter().filter(|i| is_open(i)).collect();
    let rows = open
        .iter()
        .take(ROWS)
        .map(|i| {
            let milestone = text(i, "milestone");
            el("tr")
                .child(
                    el("td").child(
                        el("a")
                            .class("mj-ui-row-link")
                            .attr("href", issue_href(&text(i, "id")))
                            .child(
                                el("span")
                                    .class("mj-ui-mono mj-ui-dim")
                                    .text(value(i, "id")),
                            )
                            .child(el("span").class("mj-ui-cell-title").text(value(i, "title"))),
                    ),
                )
                .child(el("td").child(if milestone.is_empty() {
                    super::view::none()
                } else {
                    super::view::code_link(super::milestone::href(&milestone), milestone)
                }))
                .child(el("td").child(state(i, "status")))
                .child(el("td").child(state(i, "priority")))
        })
        .collect::<Vec<_>>();
    let body = el("div")
        .child(table(
            &["Issue", "Milestone", "Status", "Priority"],
            rows,
            "Every issue of this wave is done or cancelled.",
        ))
        .children(more(ROWS.min(open.len()), open.len(), None));
    (
        format!(
            "Wave {number}: {} issue(s), {} still open",
            issues.len(),
            open.len()
        ),
        open.len(),
        body,
    )
}

/// The scope conflicts across the whole plan: issues of one wave the plan will not let run
/// together because they write the same paths, bounded at [`ROWS`]; none is said in words.
fn plan_conflicts(waves: &Answer, known: &super::prose::Known) -> El {
    let found = list(waves, "/serialised_by_scope");
    if found.is_empty() {
        return nothing("No two issues of a wave share a scope.");
    }
    let rows = found
        .iter()
        .take(ROWS)
        .map(|f| {
            let subject = text(f, "subject");
            el("tr")
                .child(el("td").child(chip(issue_href(&subject), subject)))
                .child(el("td").child(state(f, "level")))
                .child(el("td").nodes(super::prose::inline(&text(f, "message"), known)))
        })
        .collect::<Vec<_>>();
    el("details")
        .child(el("summary").text(format!(
            "Shared scope across the plan: {} issue(s) may not run with another of their wave",
            found.len()
        )))
        .child(table(&["Issue", "Level", "Why"], rows, ""))
        .children(more(ROWS.min(found.len()), found.len(), None))
}

/// The records the workspace's prose may name: every issue and every milestone the plan holds, so
/// an id in a sentence is a link exactly when there is a record behind it (I3000).
fn known(issues: &Answer, roadmap: &Answer) -> super::prose::Known {
    super::prose::Known {
        issues: list(issues, "/issues")
            .iter()
            .map(|x| text(x, "id"))
            .collect(),
        milestones: list(roadmap, "/milestones")
            .iter()
            .map(|m| text(m, "id"))
            .collect(),
    }
}

/// The dependency graph of the intent's issues: each a node in the tone of its status, each
/// dependency the plan records an edge, and a dependency outside the intent drawn small. The
/// table beneath it is the same edges, for a reader without the picture.
fn dependency_graph(issues: &Answer, mine: &[Value]) -> El {
    let body = unanswered(issues).unwrap_or_else(|| {
        let by_id: BTreeMap<String, &Value> = super::view::items(issues, "/issues")
            .iter()
            .map(|x| (text(x, "id"), x))
            .collect();
        let own: BTreeSet<String> = mine.iter().map(|x| text(x, "id")).collect();
        let edges: Vec<(String, String)> = mine
            .iter()
            .flat_map(|x| {
                let to = text(x, "id");
                strings(x, "depends_on")
                    .into_iter()
                    .map(move |from| (from, to.clone()))
            })
            .collect();
        let mut ids: BTreeSet<String> = own.clone();
        ids.extend(edges.iter().map(|(from, _)| from.clone()));
        let nodes: Vec<GraphNode> = ids
            .iter()
            .map(|id| {
                let status = by_id.get(id).map(|x| text(x, "status")).unwrap_or_default();
                GraphNode {
                    id: id.clone(),
                    label: id.clone(),
                    tone: status_tone(&status),
                    href: issue_href(id),
                    kind: if own.contains(id) { "node" } else { "leaf" },
                }
            })
            .collect();
        let rows = edges
            .iter()
            .map(|(from, to)| {
                el("tr")
                    .child(el("td").child(chip(issue_href(to), to.clone())))
                    .child(el("td").child(chip(issue_href(from), from.clone())))
                    .child(el("td").child(match by_id.get(from) {
                        Some(x) => state(x, "status"),
                        None => state(&Value::Null, "status"),
                    }))
            })
            .collect::<Vec<_>>();
        let fallback = table(
            &["Issue", "Depends on", "Its status"],
            rows,
            "No issue of this intent depends on another.",
        );
        graph(
            "The issues of this intent and the dependencies between them",
            Layout::Directed,
            &nodes,
            &edges,
            fallback,
        )
    });
    panel(
        "What waits for what?",
        "As the plan records it; a dependency outside this intent is drawn small.",
        body,
        &[issues],
    )
}

/// Where the standing of intent `id` is answered.
pub fn standing_href(id: &str) -> String {
    format!("{STANDING_PANEL}?intent={id}")
}

/// The standing panel as the screen draws it before it is asked: a placeholder the script
/// fills from [`STANDING_PANEL`], and a link to that route for a reader without it.
fn standing_placeholder(id: &str) -> El {
    el("section")
        .class("mj-ui-panel")
        .attr("aria-label", STANDING_TITLE)
        .attr("data-panel-src", standing_href(id))
        .attr("aria-busy", "false")
        .child(
            el("header").class("mj-ui-panel-head").child(
                el("div")
                    .class("mj-ui-panel-heading")
                    .child(el("h2").class("mj-ui-panel-title").text(STANDING_TITLE))
                    .child(el("p").class("mj-ui-panel-sub").text(
                        "Asked on its own: the answer joins the ledger, the session records and the peer board, which would hold this screen back.",
                    )),
            ),
        )
        .child(
            el("p").class("mj-ui-panel-body").child(
                el("a")
                    .class("mj-ui-link")
                    .attr("href", standing_href(id))
                    .text("Open why this intent stands where it stands"),
            ),
        )
}

/// The intent a panel query names, when it is one an id could be: letters, digits, `-` and
/// `_`.
fn named_intent(query: &[(String, String)]) -> Option<String> {
    query
        .iter()
        .find(|(k, _)| k == "intent")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| {
            !v.is_empty()
                && v.len() <= 128
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

/// Why the intent the query names stands where it stands, from `intent_realization.explain`:
/// the sentences its stage and verdict rest on, each unmet criterion with its evidence state,
/// the command that reproduces it and the issues serving it, the work realising the intent
/// and its drift.
pub fn standing_panel(ctx: &Context, query: &[(String, String)]) -> View {
    let Some(id) = named_intent(query) else {
        return View::new(
            STANDING_TITLE,
            panel(
                STANDING_TITLE,
                "One intent's stage, its verdict and what it still misses.",
                nothing("Name an intent: open it from the intent workspace."),
                &[],
            ),
        );
    };
    let explain = super::source::ask_input(
        ctx,
        "intent_realization.explain",
        serde_json::json!({ "id": id }),
    );
    // the records its sentences may name, read from the plan's own answers (I3000)
    let plan = ask_all(ctx, &[ISSUES, ROADMAP]);
    let body = standing_body(&id, &explain, &known(&plan[&ISSUES], &plan[&ROADMAP]));
    View::new(
        STANDING_TITLE,
        panel(
            STANDING_TITLE,
            &format!("{id}: its stage, its verdict, what is still missing and who is on it."),
            body,
            &[&explain],
        ),
    )
}

/// The standing of intent `id` as `intent_realization.explain` answered it: the sentences, the
/// unmet criteria, the work realising it and its drift; or why there is no answer.
fn standing_body(id: &str, explain: &Answer, known: &super::prose::Known) -> El {
    unanswered(explain).unwrap_or_else(|| {
        let because = list(explain, "/because");
        let unmet = list(explain, "/realization/unmet");
        let work = list(explain, "/work");
        let findings = list(explain, "/realization/findings");
        let sentences = el("ul").class("mj-ui-prose-list").children(
            because
                .iter()
                .filter_map(Value::as_str)
                .map(|line| el("li").nodes(super::prose::inline(line, known))),
        );
        let criteria = table(
            &["Criterion", "Evidence", "State", "Reproduce", "Served by"],
            unmet
                .iter()
                .map(|c| {
                    el("tr")
                        .child(el("td").child(el("code").text(text(c, "id"))))
                        .child(el("td").text(text(c, "evidence")))
                        .child(el("td").child(state(c, "state")))
                        .child(
                            el("td")
                                .child(el("code").class("mj-ui-mono").text(text(c, "reproduce"))),
                        )
                        .child(
                            el("td").child(
                                el("p").class("mj-ui-badges").children(
                                    strings(c, "issues")
                                        .into_iter()
                                        .map(|i| chip(issue_href(&i), i)),
                                ),
                            ),
                        )
                })
                .collect(),
            "Every criterion is met.",
        );
        let realising = table(
            &["Work", "Outcome", "Issue", "Provenance"],
            work.iter()
                .map(|w| {
                    let unit = w.get("work").unwrap_or(&Value::Null);
                    let links = w
                        .get("links")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let link = links.iter().find(|l| text(l, "intent") == id);
                    el("tr")
                        .child(
                            el("td")
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(format!(
                                    "{} {}",
                                    text(unit, "kind"),
                                    text(unit, "id")
                                )))
                                .child(
                                    el("span")
                                        .class("mj-ui-cell-title")
                                        .text(text(unit, "title")),
                                ),
                        )
                        .child(el("td").child(state(unit, "outcome")))
                        .child(el("td").text(link.map(|l| text(l, "issue")).unwrap_or_default()))
                        .child(
                            el("td").text(link.map(|l| text(l, "provenance")).unwrap_or_default()),
                        )
                })
                .collect(),
            "No work this checkout can see realises this intent.",
        );
        let drift =
            el("ul")
                .class("mj-ui-prose-list")
                .children(findings.iter().map(
                    |f| match f.get("message").and_then(Value::as_str) {
                        Some(m) => el("li").nodes(super::prose::inline(m, known)),
                        None => el("li").text(f.to_string()),
                    },
                ));
        let mut stack = el("div")
            .class("mj-ui-stack")
            .child(sentences)
            .child(el("h3").class("mj-ui-panel-title").text("What is missing?"))
            .child(criteria)
            .child(
                el("h3")
                    .class("mj-ui-panel-title")
                    .text("What work realises it?"),
            )
            .child(realising);
        if !findings.is_empty() {
            stack = stack
                .child(
                    el("h3")
                        .class("mj-ui-panel-title")
                        .text("Where has the record drifted?"),
                )
                .child(drift);
        }
        stack
    })
}

/// Every move this server made on the intent's issues, newest first, each leading to its
/// execution on the Cockpit.
fn moves_panel(moves: &Answer, mine: &[Value]) -> El {
    let ids: BTreeSet<String> = mine.iter().map(|x| text(x, "id")).collect();
    let body = unanswered(moves).unwrap_or_else(|| {
        let rows = list(moves, "/executions")
            .iter()
            .filter(|x| {
                x.pointer("/input/issue")
                    .and_then(Value::as_str)
                    .is_some_and(|i| ids.contains(i))
            })
            .map(|x| {
                let id = text(x, "id");
                let issue = x
                    .pointer("/input/issue")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let transition = x
                    .pointer("/input/transition")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                el("tr")
                    .attr("id", format!("move-{id}"))
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-row-link")
                                .attr("href", format!("/cockpit/executions/{id}"))
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone())),
                        ),
                    )
                    .child(el("td").child(chip(issue_href(issue), issue.to_string())))
                    .child(el("td").text(transition))
                    .child(el("td").child(state(x, "state")))
                    .child(el("td").class("mj-ui-mono").text(text(x, "created_at")))
            })
            .collect::<Vec<_>>();
        table(
            &["Execution", "Issue", "Move", "State", "Started"],
            rows,
            "No move was made on this intent's issues on this server.",
        )
    });
    panel(
        "What was done here?",
        "Every move this server made on the intent's issues, newest first.",
        body,
        &[moves],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    fn q(k: &str, v: &str) -> Vec<(String, String)> {
        vec![(k.to_string(), v.to_string())]
    }

    /// `planned` with one intent naming its first milestone, and the intent source declared.
    fn planned_with_an_intent() -> SyntheticRepository {
        let repo = SyntheticRepository::planned();
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).expect("the sources");
        text.push_str(
            "  - id: intent\n    kind: intent\n    discovery: vcs\n    \
             pathspec: ':(glob).ai/repo/project/intents/*.yaml'\n    required: false\n",
        );
        std::fs::write(&sources, text).expect("the sources");
        let intent = repo.root().join(".ai/repo/project/intents/i-first.yaml");
        std::fs::create_dir_all(intent.parent().expect("a parent")).expect("a directory");
        std::fs::write(
            &intent,
            "id: i-first\ntitle: The first intent\nstatement: \"The first milestone is real.\"\n\
             invariants:\n  - It stays real\nmilestones:\n  - m-first\nsatisfaction:\n  - id: it-holds\n    \
             criterion: It holds\n    evidence: test\n    ref: tests/first.rs\n",
        )
        .expect("an intent");
        repo
    }

    /// The rule that opens an intent when the link names none: the query first, then the
    /// earliest preferred stage in the list's order, then the first.
    #[test]
    fn the_intent_opened_is_the_named_one_then_the_preferred_stage() {
        let intents = vec![
            serde_json::json!({ "id": "a", "stage": "satisfied" }),
            serde_json::json!({ "id": "b", "stage": "planned" }),
            serde_json::json!({ "id": "c", "stage": "executing" }),
            serde_json::json!({ "id": "d", "stage": "executing" }),
        ];
        assert_eq!(chosen(&q("intent", "b"), &intents).as_deref(), Some("b"));
        assert_eq!(
            chosen(&[], &intents).as_deref(),
            Some("c"),
            "the first executing"
        );
        assert_eq!(chosen(&[], &intents[..2]).as_deref(), Some("b"));
        assert_eq!(
            chosen(&[], &intents[..1]).as_deref(),
            Some("a"),
            "else the first"
        );
        assert_eq!(chosen(&[], &[]), None);
        // a named id is kept as named, to be refused by name, unless it is no id at all
        assert_eq!(chosen(&q("intent", "zz"), &intents).as_deref(), Some("zz"));
        assert_eq!(chosen(&q("intent", "a\"b"), &intents).as_deref(), Some("c"));
    }

    /// An intent is served by the issues of its milestones and by any issue naming it.
    #[test]
    fn an_intent_is_served_by_its_milestones_issues_and_by_what_names_it() {
        let intent = serde_json::json!({ "id": "i", "milestones": [{ "id": "m1" }] });
        let issues = vec![
            serde_json::json!({ "id": "I1", "milestone": "m1" }),
            serde_json::json!({ "id": "I2", "milestone": "m2", "serves": ["i#c"] }),
            serde_json::json!({ "id": "I3", "milestone": "m2", "serves": ["other"] }),
        ];
        let ids: Vec<String> = serving(&intent, &issues)
            .iter()
            .map(|x| text(x, "id"))
            .collect();
        assert_eq!(ids, ["I1", "I2"]);
    }

    /// The issues are listed in the order plan.waves lays them out — a dependency before
    /// what depends on it, whatever the ids say — and one no wave holds comes after them.
    #[test]
    fn the_issues_follow_the_waves_not_their_ids() {
        let waves = Answer {
            source: WAVES,
            route: None,
            exposure: crate::ui::source::Exposure::default(),
            value: Ok(serde_json::json!({ "waves": [
                { "wave": 0, "issues": [{ "id": "I9" }, { "id": "I0" }] },
                { "wave": 1, "issues": [{ "id": "I1" }] },
            ]})
            .into()),
            took: std::time::Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        };
        let mine = vec![
            serde_json::json!({ "id": "I1" }),
            serde_json::json!({ "id": "I5" }),
            serde_json::json!({ "id": "I9" }),
        ];
        let ids: Vec<String> = in_wave_order(&waves, &mine)
            .into_iter()
            .map(|x| text(x, "id"))
            .collect();
        assert_eq!(ids, ["I9", "I1", "I5"]);
    }

    /// The workspace of a real plan: the intent's milestone, every issue of it with its
    /// status and wave, the graph's edge, every source named, and the verdict read as the
    /// word intents.list answers rather than `unknown`.
    #[test]
    fn the_workspace_draws_the_intents_milestones_issues_waves_and_graph() {
        let repo = planned_with_an_intent();
        let ctx = repo.context().expect("a context");
        let view = screen(&ctx, &[]);
        let html = view.main.render();
        assert_eq!(view.status, 200);
        assert!(
            html.contains("aria-current=\"page\""),
            "i-first is opened: {html}"
        );
        assert!(html.contains("The first intent"), "{html}");
        assert!(
            html.contains("/ui/roadmap?milestone=m-first#milestone"),
            "{html}"
        );
        for capability in ["intents.list", "plan.roadmap", "plan.issues", "plan.waves"] {
            assert!(html.contains(capability), "{capability} is named: {html}");
        }
        assert!(html.contains("Issues of this intent in it"), "{html}");
        assert!(!html.contains("No issue serves this intent yet."), "{html}");
        assert!(!html.contains("NaN") && !html.contains("undefined"));
    }

    /// The text of every element of `class` in `html`, in order.
    fn texts_of(html: &str, class: &str) -> Vec<String> {
        let open = format!("class=\"{class}\"");
        html.match_indices(&open)
            .filter_map(|(at, _)| {
                let rest = &html[at..];
                let start = rest.find('>')? + 1;
                let end = rest[start..].find('<')? + start;
                Some(rest[start..end].to_string())
            })
            .collect()
    }

    /// The chosen intent's title is the page's heading with the screen's name above it, and
    /// every panel is titled as a question a person asks, in plain words: no heading names a
    /// capability, which only the panel's source line does (I3001, I2915).
    #[test]
    fn the_intent_is_the_heading_and_every_panel_asks_a_plain_question() {
        let repo = planned_with_an_intent();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[]).main.render();
        assert_eq!(
            texts_of(&html, "mj-ui-title"),
            ["The first intent"],
            "{html}"
        );
        assert_eq!(
            texts_of(&html, "mj-ui-eyebrow"),
            ["Intent workspace"],
            "{html}"
        );
        let titles = texts_of(&html, "mj-ui-panel-title");
        assert!(titles.len() >= 5, "the panels are titled: {titles:?}");
        for title in &titles {
            assert!(title.ends_with('?'), "{title} is a question");
            assert!(
                !title.split_whitespace().any(|w| {
                    let w = w.trim_end_matches('?');
                    w.contains('.') || w.contains('_')
                }),
                "{title} names no capability"
            );
        }
        // with no intent recorded there is no record to title the page: the screen's name is
        let empty = SyntheticRepository::planned();
        let html = screen(&empty.context().expect("a context"), &[])
            .main
            .render();
        assert_eq!(
            texts_of(&html, "mj-ui-title"),
            ["Intent workspace"],
            "{html}"
        );
        assert!(texts_of(&html, "mj-ui-eyebrow").is_empty(), "{html}");
    }

    /// The panels stand in the order a person asks: what the intent is, what it still
    /// misses, what to do next, the work that serves it, and last what was done (I2916).
    #[test]
    fn the_panels_stand_in_the_order_a_person_asks() {
        let repo = planned_with_an_intent();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &q("issue", "I0001")).main.render();
        let titles = texts_of(&html, "mj-ui-panel-title");
        let at = |title: &str| {
            titles
                .iter()
                .position(|t| t == title)
                .unwrap_or_else(|| panic!("{title} is a panel: {titles:?}"))
        };
        let order = [
            "What is this intent for?",
            "What should happen next?",
            STANDING_TITLE,
            "What does this move do?",
            "Which milestones carry it?",
            "What work serves it?",
            "What can run together?",
            "What waits for what?",
            "What was done here?",
        ];
        for pair in order.windows(2) {
            assert!(
                at(pair[0]) < at(pair[1]),
                "{} stands before {}: {titles:?}",
                pair[0],
                pair[1]
            );
        }
    }

    /// The primary move is `plan.next` restricted to the intent's issues: the plan's own next
    /// issue when it serves the intent, else the lowest wave, then priority, then id among the
    /// ready ones, then among those under way; an intent with nothing to move has none (I2917).
    #[test]
    fn the_primary_move_is_the_plans_next_restricted_to_the_intent() {
        let issue = |id: &str, status: &str, wave: u64, priority: &str| serde_json::json!({ "id": id, "status": status, "wave": wave, "priority": priority });
        let next = |id: &str| answer(NEXT, Ok(serde_json::json!({ "issue": { "id": id } })));
        let picked = |next: &Answer, mine: &[Value]| {
            primary_move(next, mine).map(|(m, x)| (m, text(x, "id")))
        };
        let mine = vec![
            issue("I5", "active", 1, "p0"),
            issue("I4", "ready", 3, "p1"),
            issue("I3", "ready", 2, "p2"),
            issue("I2", "ready", 2, "p1"),
            issue("I1", "done", 1, "p0"),
        ];
        assert_eq!(
            picked(&next("I4"), &mine),
            Some(("start", "I4".into())),
            "the plan's own next issue, when it serves the intent"
        );
        assert_eq!(
            picked(&next("I9999"), &mine),
            Some(("start", "I2".into())),
            "else the lowest wave, then priority, among the ready ones"
        );
        let under_way = vec![
            issue("I7", "verify", 2, "p1"),
            issue("I6", "active", 2, "p1"),
            issue("I1", "done", 1, "p0"),
        ];
        assert_eq!(
            picked(&next("I9999"), &under_way),
            Some(("verify", "I6".into())),
            "with nothing ready, the work under way, ranked the same way"
        );
        assert_eq!(
            picked(&next("I1"), &[issue("I1", "done", 1, "p0")]),
            None,
            "an issue whose status admits no move is never the primary one"
        );
        assert_eq!(action_label("done", "I7"), "Mark I7 done");
    }

    /// The workspace offers exactly one primary action, the picked move as a button that opens
    /// its issue's move panel here; every move in the table stays secondary (I2917).
    #[test]
    fn the_intent_offers_one_primary_action() {
        let repo = planned_with_an_intent();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &q("intent", "i-first")).main.render();
        assert_eq!(
            html.matches("mj-btn--primary").count(),
            1,
            "one primary action: {html}"
        );
        let issues = crate::ui::source::ask(&ctx, ISSUES);
        let intents = list(&crate::ui::source::ask(&ctx, INTENTS), "/intents");
        let mine = serving(&intents[0], &list(&issues, "/issues"));
        let (word, x) = primary_move(&crate::ui::source::ask(&ctx, NEXT), &mine)
            .expect("the first milestone has work to move");
        let id = text(x, "id");
        assert!(
            html.contains(&format!(
                "class=\"mj-btn mj-btn--primary\" href=\"{}\"",
                move_href("i-first", &id).replace('&', "&amp;")
            )),
            "the button opens the move's panel here: {html}"
        );
        assert!(
            html.contains(&format!(">{}</a>", action_label(word, &id))),
            "the button names the move: {html}"
        );
    }

    /// An intent with nothing ready, under way or awaiting verification says so, and offers no
    /// primary button (I2917).
    #[test]
    fn an_intent_with_nothing_to_move_says_so() {
        let issues = answer(ISSUES, Ok(serde_json::json!({ "issues": [] })));
        let next = answer(NEXT, Ok(serde_json::json!({})));
        let html = primary_action(&issues, &next, &[], "i").render();
        assert!(html.contains("there is nothing to move"), "{html}");
        assert!(!html.contains("mj-btn--primary"), "{html}");
    }

    /// An issue added to the plan's record appears in the workspace with no other edit.
    #[test]
    fn an_issue_added_to_the_plan_appears_with_no_registration() {
        let repo = planned_with_an_intent();
        let before = screen(&repo.context().expect("a context"), &q("intent", "i-first"))
            .main
            .render();
        assert!(!before.contains("I9999"), "{before}");
        let issues = repo.root().join(".ai/repo/project/issues");
        let template = std::fs::read_dir(&issues)
            .expect("the issues")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| std::fs::read_to_string(p).is_ok_and(|t| t.contains("milestone: m-first")))
            .expect("an issue of m-first");
        let text = std::fs::read_to_string(&template).expect("the issue");
        let id = text
            .lines()
            .find_map(|l| l.strip_prefix("id: "))
            .expect("an id")
            .trim()
            .trim_matches('"')
            .to_string();
        let added: String = text
            .lines()
            .map(|l| match l {
                l if l.starts_with("id: ") => "id: I9999".to_string(),
                l if l.starts_with("slug: ") => "slug: the-added-one".to_string(),
                l => l.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(issues.join("I9999.yaml"), format!("{added}\n")).expect("an issue");
        assert_ne!(id, "I9999");
        let after = screen(&repo.context().expect("a context"), &q("intent", "i-first"))
            .main
            .render();
        assert!(after.contains("I9999"), "the added issue is drawn: {after}");
    }

    /// A picked issue of the intent gets the move form `plan.transition` answers through
    /// executions.start, and an issue that does not serve the intent is refused by name; the
    /// issue rows lead to the panel on this screen, not to another surface.
    #[test]
    fn a_picked_issue_is_offered_its_moves_here() {
        let repo = planned_with_an_intent();
        let ctx = repo.context().expect("a context");
        let first = list(&crate::ui::source::ask(&ctx, ISSUES), "/issues")
            .into_iter()
            .find(|x| text(x, "milestone") == "m-first")
            .map(|x| text(&x, "id"))
            .expect("an issue of m-first");
        let html = screen(
            &ctx,
            &[
                ("intent".into(), "i-first".into()),
                ("issue".into(), first.clone()),
            ],
        )
        .main
        .render();
        assert!(html.contains("id=\"move\""), "{html}");
        assert!(
            html.contains(&format!("data-mj-transition=\"{first}\"")),
            "{html}"
        );
        assert!(
            html.contains("data-mj-capability=\"plan.transition\""),
            "{html}"
        );
        assert!(
            html.contains(&move_href("i-first", &first).replace('&', "&amp;")),
            "the row leads to the panel here: {html}"
        );
        let outside = screen(
            &ctx,
            &[
                ("intent".into(), "i-first".into()),
                ("issue".into(), "I9998".into()),
            ],
        )
        .main
        .render();
        assert!(
            outside.contains("I9998 does not serve this intent"),
            "{outside}"
        );
        assert!(!outside.contains("data-mj-transition"), "{outside}");
        assert_eq!(
            picked_issue(&q("issue", "I1\"x")),
            None,
            "only an id is picked"
        );
    }

    /// An intent the link names that is not recorded is refused by name.
    #[test]
    fn an_unknown_intent_is_refused_by_name() {
        let repo = planned_with_an_intent();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &q("intent", "intent-nope")).main.render();
        assert!(html.contains("intent-nope is not an intent"), "{html}");
    }

    /// A repository with no intent says how to record one.
    #[test]
    fn no_intent_says_how_to_propose_one() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[]).main.render();
        assert!(html.contains("No intent is recorded"), "{html}");
        assert!(html.contains("intent propose"), "{html}");
    }

    fn answer(source: Source, value: Result<Value, String>) -> Answer {
        Answer {
            source,
            route: None,
            exposure: crate::ui::source::Exposure::default(),
            value: value.map(Into::into),
            took: std::time::Duration::ZERO,
            standing: crate::ui::source::Standing::NOW,
        }
    }

    /// The work realising an intent is a row each: the unit's kind, id and title, its outcome,
    /// and the issue and provenance of the link that names this intent; a unit linked only to
    /// another intent leaves those two cells empty. Drift is a list of each finding's message,
    /// or the finding itself where it has none, and is not drawn when there is no drift.
    #[test]
    fn the_standing_names_the_work_realising_the_intent_and_its_drift() {
        const EXPLAIN: Source = Source::of("intent_realization.explain");
        let explain = answer(
            EXPLAIN,
            Ok(serde_json::json!({
                "because": ["executing: an issue is ACTIVE"],
                "work": [
                    {
                        "work": { "kind": "task", "id": "t-1", "title": "Walk the loop", "outcome": "completed" },
                        "links": [
                            { "intent": "other", "issue": "I9", "provenance": "inferred" },
                            { "intent": "i", "issue": "I7", "provenance": "declared" },
                        ],
                    },
                    {
                        "work": { "kind": "episode", "id": "e-2", "title": "Elsewhere", "outcome": "partial" },
                        "links": [{ "intent": "other", "issue": "I8", "provenance": "inferred" }],
                    },
                ],
                "realization": {
                    "unmet": [],
                    "findings": [
                        { "message": "the scope drifted past `src/a.rs` (I7), not I99" },
                        { "kind": "orphan", "subject": "t-3" },
                    ],
                },
            })),
        );
        let known = super::super::prose::Known {
            issues: ["I7".to_string()].into(),
            milestones: Default::default(),
        };
        let html = standing_body("i", &explain, &known).render();
        assert!(html.contains("What work realises it?"), "{html}");
        assert!(
            html.contains(">task t-1</span>"),
            "the unit is named: {html}"
        );
        assert!(html.contains(">Walk the loop</span>"), "its title: {html}");
        assert!(html.contains("completed"), "its outcome: {html}");
        assert!(
            html.contains("<td>I7</td><td>declared</td>"),
            "the link naming i: {html}"
        );
        assert!(
            !html.contains(">I9<") && !html.contains(">inferred<"),
            "not another intent's link: {html}"
        );
        assert!(html.contains(">episode e-2</span>"), "{html}");
        assert!(
            html.contains("<td></td><td></td>"),
            "a unit not linked to i has no issue: {html}"
        );
        assert!(
            html.contains(">Where has the record drifted?</h3>"),
            "{html}"
        );
        assert!(
            html.contains("<li>the scope drifted past <code class=\"mj-md-code\">src/a.rs</code> (<a class=\"mj-md-ref\" href=\"/ui/prs?issue=I7#issue\">I7</a>), not I99</li>"),
            "a drift message is prose: its path is code, the issue the plan holds a link, one it does not hold text (I3000): {html}"
        );
        assert!(
            html.contains("&quot;kind&quot;:&quot;orphan&quot;"),
            "a finding without a message is shown as it is: {html}"
        );

        let calm = answer(
            EXPLAIN,
            Ok(
                serde_json::json!({ "because": [], "work": [], "realization": { "unmet": [], "findings": [] } }),
            ),
        );
        let html = standing_body("i", &calm, &known).render();
        assert!(
            !html.contains(">Drift</h3>"),
            "no drift, no heading: {html}"
        );
        assert!(
            html.contains("No work this checkout can see realises this intent."),
            "{html}"
        );
        assert!(html.contains("Every criterion is met."), "{html}");
    }

    /// The headline numbers are the chosen intent's own, each read from its record and citing
    /// it; nothing global (the plan's milestones, the roadmap's now) is counted at the top, and
    /// with no intent chosen there is only how many intents there are (I2907).
    #[test]
    fn the_headline_numbers_are_the_chosen_intents() {
        let record = answer(
            Source::of(RECORD),
            Ok(serde_json::json!({
                "id": "i", "stage": "executing", "met": 1,
                "verdict": { "state": "unsatisfied", "reasons": [] },
                "satisfaction": [{ "id": "a" }, { "id": "b" }, { "id": "c" }],
                "milestones": [{ "id": "m1", "status": "DONE" }, { "id": "m2", "status": "ACTIVE" }],
            })),
        );
        let intents = answer(
            INTENTS,
            Ok(serde_json::json!({ "intents": [intent_i(), intent_i()] })),
        );
        let next = answer(NEXT, Ok(serde_json::json!({ "issue": { "id": "I0702" } })));
        let html: String = headline(Some(&record), &intents, &next)
            .iter()
            .map(El::render)
            .collect();
        for want in [
            "Stage",
            "executing",
            "Verdict",
            "unsatisfied",
            "Criteria met",
            "Its milestones",
            "intents.record",
        ] {
            assert!(
                html.contains(want),
                "the headline leaves out {want}: {html}"
            );
        }
        assert!(html.contains(">1<"), "one criterion met: {html}");
        assert!(html.contains(">3<"), "of three criteria: {html}");
        assert!(html.contains(">2<"), "two milestones: {html}");
        assert!(
            !html.contains("Roadmap now") && !html.contains(">Intents<"),
            "a global count at the top: {html}"
        );
        assert!(
            html.contains(&issue_href("I0702")),
            "the plan's next issue leads to it: {html}"
        );

        let html: String = headline(None, &intents, &next)
            .iter()
            .map(El::render)
            .collect();
        assert!(
            html.contains("Intents") && !html.contains("Criteria met"),
            "{html}"
        );
    }

    fn intent_i() -> Value {
        serde_json::json!({
            "id": "i", "title": "The i intent", "stage": "executing",
            "milestones": [{ "id": "m1" }, { "id": "m-gone" }],
        })
    }

    /// A list of intents that cannot answer refuses no named intent it cannot see, and an
    /// issue list that cannot answer is said while the intent is still drawn, serving nothing.
    #[test]
    fn a_source_that_cannot_answer_is_said_and_nothing_is_invented() {
        let empty = |s| answer(s, Ok(serde_json::json!({})));
        let failed = answer(INTENTS, Err("no ledger".into()));
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let view = render(
            &ctx,
            None,
            &failed,
            &empty(ROADMAP),
            &empty(ISSUES),
            &empty(WAVES),
            &empty(NEXT),
            &empty(MOVES),
            &q("intent", "i"),
        );
        let html = view.main.render();
        assert!(
            html.contains("intents.list could not answer: no ledger"),
            "{html}"
        );
        assert!(!html.contains("is not an intent"), "{html}");

        let intents = answer(INTENTS, Ok(serde_json::json!({ "intents": [intent_i()] })));
        let issues = answer(ISSUES, Err("no plan".into()));
        let html = render(
            &ctx,
            None,
            &intents,
            &empty(ROADMAP),
            &issues,
            &empty(WAVES),
            &empty(NEXT),
            &empty(MOVES),
            &q("intent", "i"),
        )
        .main
        .render();
        assert!(html.contains("The i intent"), "{html}");
        assert!(
            html.contains("plan.issues could not answer: no plan"),
            "{html}"
        );
        assert!(
            html.contains("No issue of this intent is in a wave."),
            "nothing serves the intent: {html}"
        );
    }

    /// A milestone the intent names waits for what the roadmap says, as links, and one the
    /// plan does not hold is said to be missing rather than dropped.
    #[test]
    fn a_milestone_says_what_it_waits_for_and_one_the_plan_lacks_is_said() {
        let roadmap = answer(
            ROADMAP,
            Ok(serde_json::json!({ "milestones": [
                { "id": "m0", "status": "DONE" },
                { "id": "m1", "status": "ACTIVE", "blocked_by": ["m0"] },
            ]})),
        );
        let html = milestones(&roadmap, &intent_i()).render();
        assert!(
            html.contains(&super::super::milestone::href("m0")),
            "{html}"
        );
        assert!(html.contains(">m0</code>"), "the wait is a chip: {html}");
        assert!(
            !html.contains("milestone-m0"),
            "m0 is not the intent's: {html}"
        );
        assert!(html.contains(">m-gone</td>"), "{html}");
        assert!(html.contains("not in the plan"), "{html}");
        let none = milestones(&roadmap, &serde_json::json!({ "id": "x" })).render();
        assert!(none.contains("The intent names no milestone."), "{none}");
    }

    /// Each status the plan can move offers its move, and any other offers none.
    #[test]
    fn each_movable_status_offers_its_move() {
        let mine: Vec<Value> = [
            ("I1", "ready"),
            ("I2", "active"),
            ("I3", "verify"),
            ("I4", "done"),
        ]
        .iter()
        .map(|(id, status)| serde_json::json!({ "id": id, "status": status, "milestone": "m1" }))
        .collect();
        let issues = answer(ISSUES, Ok(serde_json::json!({ "issues": mine })));
        let waves = answer(WAVES, Ok(serde_json::json!({ "waves": [] })));
        let next = answer(NEXT, Ok(serde_json::json!({ "issue": { "id": "I1" } })));
        let html = issue_table(&issues, &waves, &next, &mine, "i").render();
        for (word, id) in [("start", "I1"), ("verify", "I2"), ("done", "I3")] {
            assert!(
                html.contains(&format!("{word} {id}\">{word}</a>")),
                "{id} offers {word}: {html}"
            );
        }
        assert!(html.contains(">none</span>"), "I4 offers no move: {html}");
        assert!(html.contains("the plan&#39;s next"), "{html}");
    }

    /// A wave that holds none of the intent's issues is not drawn; one that does is, and a
    /// scope conflict among them is said.
    #[test]
    fn a_wave_without_the_intents_issues_is_not_drawn() {
        let waves = answer(
            WAVES,
            Ok(serde_json::json!({
                "waves": [
                    { "wave": 7, "issues": [{ "id": "OTHER", "status": "READY" }] },
                    { "wave": 8, "issues": [{ "id": "I1", "status": "READY" }] },
                ],
                "serialised_by_scope": [{ "subject": "I1", "message": "shares lib/a.rs" }],
            })),
        );
        let mine = vec![serde_json::json!({ "id": "I1" })];
        let html = wave_panel(&waves, &mine, &Default::default()).render();
        assert!(html.contains(">8</td>"), "{html}");
        assert!(!html.contains(">7</td>"), "{html}");
        assert!(!html.contains("OTHER"), "{html}");
        assert!(html.contains("shares lib/a.rs"), "{html}");
    }

    /// A wave of the plan lists what is still open and says how many of its issues that is; a
    /// done or cancelled issue is counted in the wave's size and not drawn as work (#1184).
    #[test]
    fn a_wave_of_the_plan_lists_what_is_open_and_not_what_is_done() {
        let wave = serde_json::json!({ "wave": 2, "issues": [
            { "id": "I0001", "title": "Open work", "milestone": "M1", "status": "READY", "priority": "p1" },
            { "id": "I0002", "title": "Finished work", "milestone": "M1", "status": "DONE", "priority": "p1" },
            { "id": "I0003", "title": "Dropped work", "milestone": "M1", "status": "CANCELLED", "priority": "p2" }
        ]});
        let (summary, open, body) = open_wave(&wave);
        assert_eq!(summary, "Wave 2: 3 issue(s), 1 still open");
        assert_eq!(open, 1);
        let html = body.render();
        assert!(
            html.contains("I0001") && html.contains("Open work"),
            "{html}"
        );
        assert!(!html.contains("I0002") && !html.contains("I0003"), "{html}");
        assert!(
            html.contains("href=\"/ui/prs?issue=I0001#issue\""),
            "{html}"
        );
    }

    /// A wave with nothing open says so in words, and a wave without a number is `unknown`,
    /// never a made-up one (#1184).
    #[test]
    fn a_finished_wave_says_so_and_an_unnumbered_one_is_unknown() {
        let done = serde_json::json!({ "wave": 0, "issues": [
            { "id": "I0001", "title": "t", "status": "DONE", "priority": "p0" }
        ]});
        let (summary, open, body) = open_wave(&done);
        assert_eq!(summary, "Wave 0: 1 issue(s), 0 still open");
        assert_eq!(open, 0);
        let html = body.render();
        assert!(
            html.contains("Every issue of this wave is done or cancelled."),
            "{html}"
        );
        let (summary, _, _) = open_wave(&serde_json::json!({ "issues": [] }));
        assert!(summary.starts_with("Wave unknown"), "{summary}");
    }

    /// The plan's scope conflicts are drawn from the answer and bounded; none is said in words
    /// (#1184).
    #[test]
    fn the_plans_scope_conflicts_are_bounded_and_none_is_said() {
        let many: Vec<Value> = (0..ROWS + 5)
            .map(|i| serde_json::json!({ "level": "WARN", "subject": format!("I{i:04}"), "message": "shares share with I9999" }))
            .collect();
        let waves = answer(
            WAVES,
            Ok(serde_json::json!({ "serialised_by_scope": many })),
        );
        let html = plan_conflicts(&waves, &Default::default()).render();
        assert!(
            html.contains("I0000") && html.contains("5 more not shown"),
            "{html}"
        );
        let waves = answer(WAVES, Ok(serde_json::json!({ "serialised_by_scope": [] })));
        let html = plan_conflicts(&waves, &Default::default()).render();
        assert!(
            html.contains("No two issues of a wave share a scope."),
            "{html}"
        );
    }

    /// The workspace draws the whole plan's waves from a real plan, whichever intent is open
    /// and when none is recorded, and only the first wave with open work is unfolded; the
    /// separate `/ui/waves` screen it replaces is not a screen of the UI (#1184).
    #[test]
    fn the_workspace_draws_the_plans_waves_with_or_without_an_intent() {
        const TITLE: &str = "What is still open in each wave of the plan?";
        let repo = planned_with_an_intent();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[]).main.render();
        assert!(html.contains(TITLE), "{html}");
        assert!(html.contains("Wave 0: "), "{html}");
        assert!(html.matches("<details open").count() <= 1, "{html}");
        let titles = texts_of(&html, "mj-ui-panel-title");
        assert_eq!(
            titles.last().map(String::as_str),
            Some(TITLE),
            "the plan's waves close the workspace: {titles:?}"
        );

        let empty = SyntheticRepository::planned();
        let html = screen(&empty.context().expect("a context"), &[])
            .main
            .render();
        assert!(html.contains(TITLE), "{html}");
        assert!(html.contains("plan.waves"), "{html}");

        assert!(
            crate::ui::SCREENS.iter().all(|s| s.route != "/ui/waves"),
            "the plan's waves are a panel of the workspace, not a screen of their own"
        );
    }

    /// A dependency outside the intent is drawn small, and one the plan does not hold reads
    /// as unknown rather than as any status.
    #[test]
    fn a_dependency_outside_the_intent_is_a_leaf_and_an_unknown_one_says_so() {
        let issues = answer(
            ISSUES,
            Ok(serde_json::json!({ "issues": [
                { "id": "I0", "status": "DONE" },
                { "id": "I1", "status": "READY", "depends_on": ["I0", "IX"] },
            ]})),
        );
        let mine =
            vec![serde_json::json!({ "id": "I1", "status": "READY", "depends_on": ["I0", "IX"] })];
        let html = dependency_graph(&issues, &mine).render();
        assert!(html.contains("leaf"), "I0 is outside the intent: {html}");
        assert!(html.contains(">IX</code>"), "{html}");
        assert!(html.contains(">done<"), "I0's status is drawn: {html}");
        assert!(html.contains(">unknown<"), "IX's status is unknown: {html}");
    }
}
