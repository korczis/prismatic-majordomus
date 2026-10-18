//! Intent workspace: one intent and everything that serves it — its criteria, its
//! milestones, their issues with what blocks each, the waves those issues occupy and the
//! dependency graph between them — with every ready issue one link from the move that
//! starts it. Read from `intents.list`, `plan.roadmap`, `plan.issues`, `plan.waves` and
//! `plan.next`.
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

use super::dashboard::milestone_row;
use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{
    graph, hero, kpi, kpi_count, list, nothing, panel, panel_to, state, status_tone, table, text,
    unanswered, wide, GraphNode, Layout,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/idd";

const INTENTS: Source = Source::of("intents.list");
const ROADMAP: Source = Source::of("plan.roadmap");
const ISSUES: Source = Source::of("plan.issues");
const WAVES: Source = Source::of("plan.waves");
const NEXT: Source = Source::of("plan.next");

/// The stages, in the order the screen prefers them when the link names no intent: the
/// first intent `intents.list` reports at the earliest of these is opened. A presentation
/// rule, printed on the screen, never a status.
pub const PREFERRED_STAGES: &[&str] = &["executing", "verifying", "planned", "declared"];

/// Where the workspace of intent `id` is.
pub fn href(id: &str) -> String {
    format!("{ROUTE}?intent={id}")
}

fn issue_href(id: &str) -> String {
    format!("{}?issue={id}#issue", super::prs::ROUTE)
}

/// The Cockpit's page of an issue, which carries the moves `plan.transition` makes.
fn move_href(id: &str) -> String {
    format!("{}/plan/issues/{id}", crate::cockpit::PREFIX)
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
    let answers = ask_all(ctx, &[INTENTS, ROADMAP, ISSUES, WAVES, NEXT]);
    render(
        &answers[&INTENTS],
        &answers[&ROADMAP],
        &answers[&ISSUES],
        &answers[&WAVES],
        &answers[&NEXT],
        query,
    )
}

/// The workspace drawn from the five answers, whichever of them could not answer.
fn render(
    intents: &Answer,
    roadmap: &Answer,
    issues: &Answer,
    waves: &Answer,
    next: &Answer,
    query: &[(String, String)],
) -> View {
    let all = list(intents, "/intents");
    let id = chosen(query, &all);
    let intent = id
        .as_deref()
        .and_then(|id| all.iter().find(|i| text(i, "id") == id));

    let top = hero(
        "target",
        "violet",
        "Intent workspace",
        "From what the work is for to the move that starts it.",
        "One intent and everything that serves it: its milestones, their issues and what blocks them, the waves the dependencies allow and the graph of what waits for what.",
        vec![
            kpi_count("Intents", &intents.read("/intents")),
            kpi_count("Milestones", &roadmap.read("/milestones")),
            kpi("Roadmap now", &roadmap.read("/now")),
            kpi("Plan's next issue", &next.read("/issue/id")),
        ],
    );

    let mut grid = el("div")
        .class("mj-ui-grid")
        .child(wide(choice(intents, id.as_deref())));
    match (id.as_deref(), intent) {
        (None, _) => {}
        (Some(id), None) => {
            if unanswered(intents).is_none() {
                grid = grid.child(wide(panel(
                    "Intent",
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
            grid = grid
                .child(wide(super::intent::inspector_closing_to(
                    intents, issues, id, ROUTE,
                )))
                .child(wide(milestones(roadmap, intent)))
                .child(wide(issue_table(issues, waves, next, &mine)))
                .child(wide(wave_panel(waves, &mine)))
                .child(wide(dependency_graph(issues, &mine)));
        }
    }

    View::new(
        "Intent workspace",
        el("div").class("mj-ui-stack").child(top).child(grid),
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
                "A link that names no intent opens the first one intents.list reports {}, in that order.",
                PREFERRED_STAGES.join(", then ")
            )))
    });
    panel(
        "Intents",
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
        "Milestones",
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
fn issue_table(issues: &Answer, waves: &Answer, next: &Answer, mine: &[Value]) -> El {
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
                let mv = match status.as_str() {
                    "READY" => Some("start"),
                    "ACTIVE" => Some("verify"),
                    "VERIFY" => Some("done"),
                    _ => None,
                };
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
                                .attr("href", move_href(&id))
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
        "Issues",
        "Every issue of its milestones or naming it, by wave; a move opens the issue's page, where plan.transition makes it and refuses one the plan does not allow.",
        Some(super::issues::ROUTE),
        body,
        &[issues, waves, next],
    )
}

/// The waves the intent's issues occupy, as `plan.waves` layers the whole plan, and the
/// scope conflicts among them that serialise issues of one wave.
fn wave_panel(waves: &Answer, mine: &[Value]) -> El {
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
                el("li")
                    .child(el("code").class("mj-ui-mono").text(text(f, "subject")))
                    .child(el("span").text(format!(" {}", text(f, "message"))))
            })
            .collect();
        let mut body = el("div").class("mj-ui-stack").child(table(
            &["Wave", "Issues of this intent in it"],
            rows,
            "No issue of this intent is in a wave.",
        ));
        if !conflicts.is_empty() {
            body = body
                .child(
                    el("h4")
                        .class("mj-ui-panel-title")
                        .text("Shared scope: these may not run at the same time"),
                )
                .child(el("ul").class("mj-ui-prose-list").children(conflicts));
        }
        body
    });
    panel(
        "Waves",
        "An issue enters a wave once every dependency has left it; sharing a wave is necessary to run together, not sufficient.",
        body,
        &[waves],
    )
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
        "Dependencies",
        "What waits for what, as the plan records it; a dependency outside this intent is drawn small.",
        body,
        &[issues],
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
        let view = render(
            &failed,
            &empty(ROADMAP),
            &empty(ISSUES),
            &empty(WAVES),
            &empty(NEXT),
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
            &intents,
            &empty(ROADMAP),
            &issues,
            &empty(WAVES),
            &empty(NEXT),
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
        let html = issue_table(&issues, &waves, &next, &mine).render();
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
        let html = wave_panel(&waves, &mine).render();
        assert!(html.contains(">8</td>"), "{html}");
        assert!(!html.contains(">7</td>"), "{html}");
        assert!(!html.contains("OTHER"), "{html}");
        assert!(html.contains("shares lib/a.rs"), "{html}");
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
