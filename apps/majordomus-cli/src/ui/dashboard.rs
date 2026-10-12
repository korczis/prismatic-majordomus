//! The Dashboard: how the delivery stands, on one screen.
//!
//! Its first paint asks only sources that answer in well under a second on this
//! repository, all at once: `repository.info`, `health.report`, `release.version`,
//! `distribution.releases`, `plan.status`, `plan.roadmap` and `mesh.peers`. Measured one at a
//! time on 2026-10-11, the slowest was `release.version` at 0.5-0.8 s. The pull-request queue
//! (`integration.queue`) took 50 s for 1.2 MB, so it is a panel of its own at
//! [`PULL_REQUESTS_PANEL`]: the script pulls it in once the screen has painted, and a reader
//! without JavaScript follows its link.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;

use super::shell::View;
use super::source::{ask, ask_all, Answer, Availability, Source};
use super::view::{
    counts, day, donut, hero, kpi, kpi_count, kpi_state, kpi_to, list, meter, more, nothing,
    panel_to, shown, state, strip, table, text, timeline, unanswered, value, wide,
};

/// The route of the pull-request queue panel.
pub const PULL_REQUESTS_PANEL: &str = "/ui/panels/pull-requests";

const REPOSITORY: Source = Source::of("repository.info");
const HEALTH: Source = Source::of("health.report");
const VERSION: Source = Source::of("release.version");
const RELEASES: Source = Source::of("distribution.releases");
const PLAN: Source = Source::of("plan.status");
const ROADMAP: Source = Source::of("plan.roadmap");
const MESH: Source = Source::of("mesh.peers");
const QUEUE: Source = Source::of("integration.queue");

/// The sources the first paint asks, together.
pub const FIRST_PAINT: &[Source] = &[REPOSITORY, HEALTH, VERSION, RELEASES, PLAN, ROADMAP, MESH];

/// How many rows a panel shows before it says how many there are in all.
pub const ROWS: usize = 8;

/// The Dashboard screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, FIRST_PAINT);
    let a = |s: Source| &answers[&s];

    let git = |p: &'static str| a(REPOSITORY).read(p).text();
    let lead = match (git("/repository/git/branch"), git("/repository/git/head")) {
        (Some(branch), Some(head)) => format!(
            "{branch} at {} · working tree {}",
            &head[..12.min(head.len())],
            git("/repository/git/working_tree")
                .as_deref()
                .unwrap_or("unknown")
        ),
        _ => "The repository's version control could not be read.".into(),
    };

    let top = hero(
        "home",
        "blue",
        "Dashboard",
        "How the delivery stands, read from the repository itself.",
        &lead,
        vec![
            kpi_to(kpi_state("Health", &a(HEALTH).read("/status")), "#health"),
            kpi_to(
                kpi("Released", &a(RELEASES).read("/latest/version")),
                super::delivery::ROUTE,
            ),
            kpi_to(
                kpi("Next release", &a(VERSION).read("/next")),
                super::delivery::ROUTE,
            ),
            kpi_to(
                kpi("Issues ready", &a(PLAN).read("/counts/by_status/READY")),
                super::work::ROUTE,
            ),
            kpi_to(
                kpi("Issues blocked", &a(PLAN).read("/counts/by_status/BLOCKED")),
                super::roadmap::ROUTE,
            ),
            kpi_to(
                kpi_count("Mesh machines", &a(MESH).read("/machines")),
                super::mesh::ROUTE,
            ),
        ],
    );

    let grid = el("div")
        .class("mj-ui-grid")
        .child(plan_panel(a(ROADMAP), a(PLAN)))
        .child(issues_panel(a(PLAN)))
        .child(wide(queue_placeholder()))
        .child(releases_panel(a(RELEASES), a(VERSION), Some(ROWS)))
        .child(mesh_panel(a(MESH)))
        .child(health_panel(a(HEALTH)));

    View::new(
        "Dashboard",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// The pull-request queue, as a panel: what the fragment request and the page without
/// JavaScript both show.
pub fn pull_requests_panel(ctx: &Context, query: &[(String, String)]) -> View {
    let selected = query
        .iter()
        .find(|(k, _)| k == "pr")
        .and_then(|(_, v)| v.parse::<u64>().ok());
    View::new(
        "Pull-request queue",
        queue_panel(&ask(ctx, QUEUE), selected),
    )
}

/// The milestones still open, in the plan's own order, each with the issues it has done
/// out of those it holds.
pub fn plan_panel(roadmap: &Answer, plan: &Answer) -> El {
    let body = unanswered(roadmap).unwrap_or_else(|| {
        let open: Vec<Value> = list(roadmap, "/milestones")
            .into_iter()
            .filter(|m| m.get("status").and_then(Value::as_str) != Some("DONE"))
            .collect();
        let rows = open
            .iter()
            .take(ROWS)
            .map(milestone_row)
            .collect::<Vec<_>>();
        el("div")
            .child(el("p").class("mj-ui-line").text(format!(
                "Now {} · next {}",
                shown(&roadmap.read("/now")),
                shown(&roadmap.read("/next"))
            )))
            .child(table(
                &["Milestone", "State", "Issues done"],
                rows,
                "No milestone is open.",
            ))
            .children(more(
                ROWS.min(open.len()),
                open.len(),
                Some(super::roadmap::ROUTE),
            ))
    });
    panel_to(
        "Roadmap",
        "The milestones still open, in the plan's own order.",
        Some(super::roadmap::ROUTE),
        body,
        &[roadmap, plan],
    )
}

/// One milestone as a table row: its id and title, its state and its issues done.
pub fn milestone_row(m: &Value) -> El {
    let total = m.pointer("/counts/total").and_then(Value::as_u64);
    let done = m.pointer("/counts/by_status/DONE").and_then(Value::as_u64);
    let id = text(m, "id");
    el("tr")
        .attr("id", format!("milestone-{id}"))
        .child(
            el("td").child(
                el("a")
                    .class("mj-ui-row-link")
                    .attr("href", super::milestone::href(&id))
                    .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()))
                    .child(el("span").class("mj-ui-cell-title").text(text(m, "title"))),
            ),
        )
        .child(el("td").child(state(m, "status")))
        .child(el("td").class("mj-ui-cell-meter").child(meter(done, total)))
}

/// The issues of the plan by their status, as `plan.status` counts them.
pub fn issues_panel(plan: &Answer) -> El {
    let body =
        unanswered(plan).unwrap_or_else(|| donut("issues", &counts(plan, "/counts/by_status")));
    panel_to(
        "Issues by status",
        "Every issue of the plan, counted by the plan.",
        Some(super::work::ROUTE),
        body,
        &[plan],
    )
}

/// The label of the tile counting merges: the window is the queue's own (`throughput/
/// window_days`), so the label says what the queue counted rather than a window of its own.
pub fn merged_label(queue: &Answer) -> String {
    match queue.read("/throughput/window_days").count() {
        Some(days) => format!("Merged, {days} days"),
        None => "Merged, over a window the queue does not state".into(),
    }
}

/// The first two reasons of a pull request, and how many more there are: a row has room for
/// two, and its inspector holds them all.
pub fn first_reasons(reasons: &[String]) -> String {
    match reasons.len() {
        0 => "none".into(),
        1 | 2 => reasons.join(", "),
        n => format!("{} and {} more", reasons[..2].join(", "), n - 2),
    }
}

/// What was published, newest first, as a timeline: the first `limit` of them, saying how
/// many more there are, or every one of them when there is no limit.
pub fn releases_panel(releases: &Answer, version: &Answer, limit: Option<usize>) -> El {
    let body = unanswered(releases).unwrap_or_else(|| {
        let all = list(releases, "/releases");
        let limit = limit.unwrap_or(all.len());
        let entries = all
            .iter()
            .take(limit)
            .map(|r| {
                let commit = text(r, "commit");
                let record = text(r, "metadata_url");
                (
                    text(r, "published_at"),
                    format!("v{}", value(r, "version")),
                    el("span")
                        .class("mj-ui-dim")
                        .text(format!(
                            "{} · {}",
                            value(r, "channel"),
                            commit.get(..10).unwrap_or(if commit.is_empty() {
                                "no commit"
                            } else {
                                &commit
                            })
                        ))
                        // the release's own record, at the address the distribution gives
                        .when(!record.is_empty(), |s| {
                            s.text(" · ")
                                .child(super::view::link(record.clone(), "its record"))
                        }),
                )
            })
            .collect::<Vec<_>>();
        el("div")
            .child(el("p").class("mj-ui-line").text(format!(
                "Declared {} · the commits since the last release imply a {} release",
                shown(&version.read("/declared")),
                shown(&version.read("/bump"))
            )))
            .child(if entries.is_empty() {
                nothing("Nothing has been released yet.")
            } else {
                timeline(entries)
            })
            .children(more(
                limit.min(all.len()),
                all.len(),
                Some(super::delivery::ROUTE),
            ))
    });
    panel_to(
        "Releases",
        "What was published, newest first.",
        Some(super::delivery::ROUTE),
        body,
        &[releases, version],
    )
}

/// The machines this runtime knows, with the liveness of each runtime.
pub fn mesh_panel(mesh: &Answer) -> El {
    let body = unanswered(mesh).unwrap_or_else(|| {
        let machines = list(mesh, "/machines");
        let rows = machines
            .iter()
            .take(ROWS)
            .map(machine_row)
            .collect::<Vec<_>>();
        el("div")
            .child(table(
                &["Machine", "Where", "Runtimes"],
                rows,
                "No machine is known to the mesh runtime.",
            ))
            .children(more(
                ROWS.min(machines.len()),
                machines.len(),
                Some(super::mesh::ROUTE),
            ))
    });
    panel_to(
        "Mesh",
        "The machines this runtime knows, and how alive each runtime is.",
        Some(super::mesh::ROUTE),
        body,
        &[mesh],
    )
}

/// One machine as a table row: its name, whether it is this one, and the liveness of each
/// of its runtimes as the mesh runtime observed it.
pub fn machine_row(m: &Value) -> El {
    let local = m.get("local").and_then(Value::as_bool);
    let words: Vec<String> = m
        .get("runtimes")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .filter_map(|r| r.get("liveness").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    el("tr")
        .child(el("td").class("mj-ui-mono").text(text(m, "name")))
        .child(el("td").text(match local {
            Some(true) => "this machine",
            Some(false) => "remote",
            None => "unknown",
        }))
        .child(
            el("td")
                .class("mj-ui-badges")
                .children(words.iter().map(|w| word_badge(w))),
        )
}

/// Every check this process runs on itself and its checkout.
pub fn health_panel(health: &Answer) -> El {
    let body = unanswered(health).unwrap_or_else(|| {
        el("ul")
            .class("mj-ui-checks")
            .children(list(health, "/checks").iter().map(|c| {
                el("li")
                    .class("mj-ui-check")
                    .child(
                        el("span")
                            .class("mj-ui-check-text")
                            .child(el("span").class("mj-ui-check-title").text(text(c, "title")))
                            .child(
                                el("span")
                                    .class("mj-ui-check-detail")
                                    .text(text(c, "detail")),
                            ),
                    )
                    .child(state(c, "status"))
            }))
    });
    panel_to(
        "Health",
        "Every check this process runs on itself and its checkout.",
        None,
        body,
        &[health],
    )
    .attr("id", "health")
}

/// Where the queue panel will be. Without JavaScript it is a link to the panel's own page;
/// the script replaces it with the panel once the screen has painted.
fn queue_placeholder() -> El {
    el("section")
        .class("mj-ui-panel")
        .attr("aria-label", "Pull-request queue")
        .attr("data-panel-src", PULL_REQUESTS_PANEL)
        .attr("aria-busy", "false")
        .child(
            el("header")
                .class("mj-ui-panel-head")
                .child(el("h2").class("mj-ui-panel-title").text("Pull-request queue"))
                .child(el("p").class("mj-ui-panel-sub").text(
                    "Asked on its own: integration.queue reads every open pull request and its checks, which would hold this screen back.",
                )),
        )
        .child(
            el("p").class("mj-ui-panel-body").child(
                el("a")
                    .class("mj-ui-link")
                    .attr("href", PULL_REQUESTS_PANEL)
                    .text("Open the pull-request queue"),
            ),
        )
}

/// The queue: its tallies, its throughput and the pull requests in rank order.
pub fn queue_panel(queue: &Answer, selected: Option<u64>) -> El {
    let body = unanswered(queue).unwrap_or_else(|| {
        if queue.read("/observed").state != Availability::Available(Value::Bool(true)) {
            return el("p").class("mj-ui-empty").text(format!(
                "The forge was not observed in this checkout: {}",
                shown(&queue.read("/reason"))
            ));
        }
        let tally = |p: &'static str| queue.read(p);
        let tallies = strip(vec![
            kpi("Open", &tally("/queue/tallies/open")),
            kpi(
                "Needs refresh",
                &tally("/queue/tallies/by_disposition/needs_refresh"),
            ),
            kpi(
                "Needs repair",
                &tally("/queue/tallies/by_disposition/needs_repair"),
            ),
            kpi(
                "Conflicting",
                &tally("/queue/tallies/by_disposition/conflicting"),
            ),
            kpi(&merged_label(queue), &tally("/throughput/merges")),
            kpi("Merges per day", &tally("/throughput/merges_per_day")),
        ]);
        let assessments = list(queue, "/queue/assessments");
        let rows = assessments
            .iter()
            .take(ROWS * 2)
            .map(|a| {
                let number = a
                    .get("number")
                    .and_then(Value::as_u64)
                    .map_or_else(|| "unknown".to_string(), |n| format!("#{n}"));
                let reasons: Vec<String> = a
                    .get("reasons")
                    .and_then(Value::as_array)
                    .map(|rs| {
                        rs.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let n = a.get("number").and_then(Value::as_u64);
                el("tr")
                    .attr_if("id", n.map(|n| format!("pr-{n}")))
                    .child(el("td").class("mj-ui-mono").text(number))
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-row-link")
                                .attr(
                                    "href",
                                    n.map_or_else(
                                        || super::prs::ROUTE.to_string(),
                                        |n| format!("{}?pr={n}#pr", super::prs::ROUTE),
                                    ),
                                )
                                .child(el("span").class("mj-ui-cell-title").text(text(a, "title"))),
                        ),
                    )
                    .child(el("td").child(state(a, "disposition")))
                    .child(el("td").text(value(a, "lane")))
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(first_reasons(&reasons)),
                    )
            })
            .collect::<Vec<_>>();
        let observed = queue
            .read("/queue/observed_at")
            .text()
            .map(|t| t.replace('T', " "))
            .unwrap_or_else(|| "at an unknown time".into());
        let detail = selected.map(|n| {
            list(queue, "/queue/assessments")
                .into_iter()
                .find(|a| a.get("number").and_then(Value::as_u64) == Some(n))
                .map_or_else(
                    || {
                        nothing(&format!(
                            "#{n} is not among the open pull requests the queue observed."
                        ))
                    },
                    |a| pull_request_detail(&a),
                )
        });
        el("div")
            .class("mj-ui-stack")
            .children(detail)
            .child(tallies)
            .child(table(
                &["PR", "Title", "Disposition", "Lane", "Why"],
                rows,
                "No pull request is open.",
            ))
            .children(more(
                (ROWS * 2).min(assessments.len()),
                assessments.len(),
                Some(super::prs::ROUTE),
            ))
            .child(
                el("p")
                    .class("mj-ui-line")
                    .text(format!("Observed {observed} ({})", day(&observed))),
            )
    });
    panel_to(
        "Pull-request queue",
        "What the integration lane would merge, repair or wait for, and why.",
        Some(super::prs::ROUTE),
        body,
        &[queue],
    )
}

/// One pull request as the queue assessed it: what it is, every gate with its verdict, the
/// evidence each verdict was read from, why it stands where it stands, what would move it,
/// and the factors of its rank. Read from the queue's own answer, so it costs nothing more.
fn pull_request_detail(a: &Value) -> El {
    let s = |k: &str| text(a, k);
    // an assessment without a number is shown as unknown, never as #0
    let number = a
        .get("number")
        .and_then(Value::as_u64)
        .map_or_else(|| "unknown".to_string(), |n| n.to_string());
    let strings = |k: &str| -> Vec<String> {
        a.get(k)
            .and_then(Value::as_array)
            .map(|v| {
                v.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let gates = el("ul").class("mj-ui-checks").children(
        a.get("gates")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|g| {
                let passed = g.get("passed").and_then(Value::as_bool);
                el("li")
                    .class("mj-ui-check")
                    .child(
                        el("span")
                            .class("mj-ui-check-title mj-ui-mono")
                            .text(text(g, "gate")),
                    )
                    .child(word_badge(match passed {
                        Some(true) => "pass",
                        Some(false) => "fail",
                        None => "unknown",
                    }))
            }),
    );
    let evidence = a
        .get("evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|e| {
            el("tr")
                .child(el("td").class("mj-ui-mono").text(text(e, "kind")))
                .child(el("td").child(state(e, "status")))
                .child(el("td").class("mj-ui-dim").text(text(e, "detail")))
        })
        .collect::<Vec<_>>();
    let factors = a
        .get("rank_factors")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    el("div")
                        .class("mj-ui-fact")
                        .child(el("dt").text(k.replace('_', " ")))
                        .child(el("dd").class("mj-ui-mono").text(match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    el("section")
        .class("mj-ui-detail")
        .attr("id", "pr")
        .attr("aria-label", format!("Pull request #{number}"))
        .child(
            el("div")
                .class("mj-ui-detail-head")
                .child(el("p").class("mj-ui-mono mj-ui-dim").text(format!(
                    "#{number} · {} · {} → {}",
                    s("author"),
                    s("head_ref"),
                    s("base_ref")
                )))
                .child(el("h3").class("mj-ui-detail-title").text(s("title")))
                .child(
                    el("p")
                        .class("mj-ui-badges")
                        .child(state(a, "disposition"))
                        .child(state(a, "lane"))
                        .child(state(a, "risk")),
                ),
        )
        .child(el("p").class("mj-ui-line").text(format!(
            "Next: {}",
            if s("next_action").is_empty() {
                "nothing to do".into()
            } else {
                s("next_action")
            }
        )))
        .child(
            el("p")
                .class("mj-ui-mono mj-ui-dim")
                .text(strings("reasons").join(" · ")),
        )
        .child(el("h4").class("mj-ui-panel-title").text("Gates"))
        .child(gates)
        .child(el("h4").class("mj-ui-panel-title").text("Evidence"))
        .child(table(
            &["Kind", "Verdict", "What it says"],
            evidence,
            "No evidence was recorded.",
        ))
        .child(el("h4").class("mj-ui-panel-title").text("Paths it authors"))
        .child(el("p").class("mj-ui-mono mj-ui-dim").text({
            let paths = strings("authored_paths");
            if paths.is_empty() {
                "none".into()
            } else {
                paths.join(", ")
            }
        }))
        .child(
            el("h4")
                .class("mj-ui-panel-title")
                .text("Why it ranks where it does"),
        )
        .child(el("dl").class("mj-ui-facts").children(factors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};
    use crate::ui::source::Exposure;

    /// Against a real registry every source of the first paint is a capability that
    /// `capabilities.describe` knows, and none of them is the queue, which is the one this
    /// screen must not wait for. Asked, not read: the test holds the rule the screen does.
    #[test]
    fn the_first_paint_asks_only_declared_sources_and_never_the_queue() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let declared = |id: &str| {
            ctx.execute("capabilities.describe", serde_json::json!({ "id": id }))
                .is_ok()
        };
        for s in FIRST_PAINT {
            assert!(
                declared(s.capability),
                "{} is not a capability",
                s.capability
            );
        }
        assert!(!FIRST_PAINT.contains(&QUEUE));
        assert!(declared(QUEUE.capability));
        assert!(
            !declared("no.such.capability"),
            "the describe call must be able to say no"
        );
    }

    /// The screen renders against a synthetic repository, every source is named, and the
    /// queue is a placeholder pointing at its own route.
    #[test]
    fn every_source_is_named_and_the_queue_waits_for_its_own_route() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        for s in FIRST_PAINT.iter().filter(|s| **s != REPOSITORY) {
            assert!(html.contains(s.capability), "{} is not named", s.capability);
        }
        assert!(html.contains(&format!("data-panel-src=\"{PULL_REQUESTS_PANEL}\"")));
    }

    /// A selected pull request opens its detail from the queue's own answer, with its gates
    /// and their verdicts; a number the queue did not observe says so.
    #[test]
    fn a_selected_pull_request_opens_its_detail() {
        let queue = Answer {
            source: QUEUE,
            route: None,
            exposure: Exposure::default(),
            value: Ok(serde_json::json!({
                "observed": true,
                "queue": { "tallies": { "open": 1 }, "assessments": [{
                    "number": 7, "title": "a change", "disposition": "needs_refresh",
                    "gates": [{ "gate": "base", "passed": true }, { "gate": "ci", "passed": false }],
                    "reasons": ["behind_master:2"], "next_action": "bring master in"
                }]}
            })),
            took: std::time::Duration::ZERO,
        };
        let html = queue_panel(&queue, Some(7)).render();
        assert!(
            html.contains("id=\"pr\"") && html.contains("bring master in"),
            "{html}"
        );
        assert!(html.contains(">base<") && html.contains(">ci<"));
        assert!(
            html.contains("/ui/prs?pr=7#pr"),
            "rows lead to their detail"
        );
        assert!(queue_panel(&queue, Some(8))
            .render()
            .contains("#8 is not among"));
    }

    /// An assessment that carries no number is shown as unknown, never as #0.
    #[test]
    fn a_pull_request_without_a_number_is_unknown_never_zero() {
        let html = pull_request_detail(&serde_json::json!({ "title": "unnumbered", "gates": [] }))
            .render();
        assert!(html.contains("unknown") && !html.contains("#0"), "{html}");
    }

    /// A failed source says it could not answer, and a queue never observed says so.
    #[test]
    fn a_missing_value_reads_as_missing() {
        let failed = Answer {
            source: HEALTH,
            route: None,
            exposure: Exposure::default(),
            value: Err("no".into()),
            took: std::time::Duration::ZERO,
        };
        assert!(health_panel(&failed).render().contains("could not answer"));
        let unobserved = Answer {
            source: QUEUE,
            value: Ok(serde_json::json!({ "observed": false, "reason": "never refreshed" })),
            ..failed.clone()
        };
        assert!(queue_panel(&unobserved, None)
            .render()
            .contains("never refreshed"));
    }

    /// An answer of `source` holding `value`, as the screen would read it.
    fn answer(source: Source, value: Value) -> Answer {
        Answer {
            source,
            route: None,
            exposure: Exposure::default(),
            value: Ok(value),
            took: std::time::Duration::ZERO,
        }
    }

    /// What was published is a timeline, newest first, each entry with its channel and a
    /// commit cut to ten characters; nothing published says so.
    #[test]
    fn the_releases_are_a_timeline_with_channel_and_commit() {
        let version = answer(
            VERSION,
            serde_json::json!({ "declared": "0.21.0", "bump": "minor" }),
        );
        let releases = answer(
            RELEASES,
            serde_json::json!({ "releases": [
                { "version": "0.20.0", "channel": "stable", "commit": "c8f0e4c5011234567890",
                  "published_at": "2026-10-10T16:38:00Z",
                  "metadata_url": "https://example.test/releases/v0.20.0.json" },
                { "version": "0.19.1", "channel": "stable", "commit": "abc",
                  "published_at": "2026-10-09T10:00:00Z" }
            ]}),
        );
        let html = releases_panel(&releases, &version, Some(ROWS)).render();
        assert!(
            html.contains("v0.20.0") && html.contains("v0.19.1"),
            "{html}"
        );
        assert!(
            html.contains("stable · c8f0e4c501 · <a"),
            "a commit is cut to ten: {html}"
        );
        assert!(
            html.contains("href=\"https://example.test/releases/v0.20.0.json\">its record</a>"),
            "a release leads to the record the distribution gives: {html}"
        );
        assert!(
            html.contains("stable · abc<"),
            "a short commit is kept whole: {html}"
        );
        assert!(html.contains("Declared 0.21.0"), "{html}");
        let none = answer(RELEASES, serde_json::json!({ "releases": [] }));
        assert!(releases_panel(&none, &version, Some(ROWS))
            .render()
            .contains("Nothing has been released yet."));
    }

    /// A machine says whether it is this one, and each of its runtimes is its own liveness
    /// word; a machine that does not say where it is reads as unknown, never as this one.
    #[test]
    fn every_machine_says_where_it_is_and_how_alive_its_runtimes_are() {
        let mesh = answer(
            MESH,
            serde_json::json!({ "machines": [
                { "name": "lundra", "local": false,
                  "runtimes": [{ "liveness": "live" }, { "liveness": "expired" }, {}] },
                { "name": "this-mac", "local": true, "runtimes": [{ "liveness": "own" }] },
                { "name": "elsewhere" }
            ]}),
        );
        let html = mesh_panel(&mesh).render();
        assert!(
            html.contains(">lundra<") && html.contains(">remote<"),
            "{html}"
        );
        assert!(
            html.contains(">this-mac<") && html.contains(">this machine<"),
            "{html}"
        );
        assert!(
            html.contains(">elsewhere<") && html.contains(">unknown<"),
            "{html}"
        );
        for word in ["live", "expired", "own"] {
            assert!(html.contains(&format!(">{word}<")), "{word}: {html}");
        }
    }

    /// The queue as the lane observed it: a row per pull request, a row with no number leads
    /// to the work screen rather than to a detail, and the time it was observed is said.
    /// The selected one opens its detail with every gate, its evidence, what it authors and
    /// the factors of its rank; a gate with no verdict reads as unknown.
    #[test]
    fn a_full_queue_opens_every_part_of_a_pull_requests_detail() {
        let queue = answer(
            QUEUE,
            serde_json::json!({
                "observed": true,
                "queue": {
                    "observed_at": "2026-10-11T09:00:00Z",
                    "tallies": { "open": 3, "by_disposition": { "needs_refresh": 1 } },
                    "assessments": [
                        {
                            "number": 948, "title": "the UI has every screen",
                            "author": "korczis", "head_ref": "feature/ui", "base_ref": "master",
                            "disposition": "ready", "lane": "train", "risk": "low",
                            "next_action": "",
                            "reasons": ["stacked_on:940", "ci:pass", "third"],
                            "gates": [
                                { "gate": "ci", "passed": true },
                                { "gate": "freshness", "passed": false },
                                { "gate": "review" }
                            ],
                            "evidence": [
                                { "kind": "check", "status": "pass", "detail": "ci is green" },
                                { "kind": "review", "status": "pending", "detail": "requested" }
                            ],
                            "authored_paths": ["apps/majordomus-cli/src/ui", "share/ui"],
                            "rank_factors": { "lane_order": 2, "stacked_on": "940" }
                        },
                        { "title": "a pull request with no number", "disposition": "blocked" }
                    ]
                },
                "throughput": { "merges": 4, "merges_per_day": 0.57 }
            }),
        );
        let html = queue_panel(&queue, Some(948)).render();
        assert!(html.contains("id=\"pr-948\""), "{html}");
        assert!(
            html.contains(&format!("href=\"{}\"", super::super::prs::ROUTE)),
            "a row with no number leads to the drain: {html}"
        );
        assert!(html.contains("Observed 2026-10-11 09:00:00Z"), "{html}");
        assert!(
            html.contains("stacked_on:940, ci:pass and 1 more<"),
            "two reasons in a row, and how many it leaves out: {html}"
        );
        assert!(html.contains("Next: nothing to do"), "{html}");
        assert!(
            html.contains(">freshness<") && html.contains(">review<"),
            "{html}"
        );
        assert!(html.contains(">unknown<"), "a gate with no verdict: {html}");
        assert!(
            html.contains("ci is green") && html.contains("requested"),
            "{html}"
        );
        assert!(
            html.contains("apps/majordomus-cli/src/ui, share/ui"),
            "the paths it authors: {html}"
        );
        assert!(html.contains("<dt>lane order</dt>"), "{html}");
        assert!(html.contains(">2<") && html.contains(">940<"), "{html}");
        assert!(
            html.contains("#948 · korczis · feature/ui → master"),
            "{html}"
        );
    }

    /// The panel's own route reads `?pr` as the selection; a value that is no number selects
    /// nothing rather than failing.
    #[test]
    fn the_panel_route_reads_its_selection_from_the_query() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        for pr in ["12", "twelve"] {
            let view = pull_requests_panel(&ctx, &[("pr".into(), pr.into())]);
            assert_eq!(view.status, 200);
            assert!(view.main.render().contains("Pull-request queue"));
        }
    }

    /// The merge tile is labelled with the queue's own window, and says so when the queue
    /// states none; a row names two reasons and how many more there are.
    #[test]
    fn the_merge_window_and_the_reasons_are_the_queues_own() {
        let stated = answer(
            QUEUE,
            serde_json::json!({ "throughput": { "window_days": 7 } }),
        );
        assert_eq!(merged_label(&stated), "Merged, 7 days");
        let silent = answer(QUEUE, serde_json::json!({ "throughput": {} }));
        assert_eq!(
            merged_label(&silent),
            "Merged, over a window the queue does not state"
        );
        let r = |n: usize| (0..n).map(|i| format!("r{i}")).collect::<Vec<_>>();
        assert_eq!(first_reasons(&r(0)), "none");
        assert_eq!(first_reasons(&r(1)), "r0");
        assert_eq!(first_reasons(&r(2)), "r0, r1");
        assert_eq!(first_reasons(&r(5)), "r0, r1 and 3 more");
    }

    /// A panel that shows the first rows of a longer list says how many it leaves out and
    /// leads to the screen that shows them all.
    #[test]
    fn a_panel_of_a_longer_list_says_how_many_it_leaves_out() {
        let milestones: Vec<Value> = (0..ROWS + 3)
            .map(|i| serde_json::json!({ "id": format!("M{i:03}"), "title": "t", "status": "ACTIVE" }))
            .collect();
        let roadmap = answer(ROADMAP, serde_json::json!({ "milestones": milestones }));
        let html = plan_panel(&roadmap, &answer(PLAN, serde_json::json!({}))).render();
        assert!(
            html.contains(&format!("The first {ROWS} of {}; 3 more", ROWS + 3)),
            "{html}"
        );
        let machines: Vec<Value> = (0..ROWS + 2)
            .map(|i| serde_json::json!({ "name": format!("m{i}"), "runtimes": [] }))
            .collect();
        let mesh = mesh_panel(&answer(MESH, serde_json::json!({ "machines": machines }))).render();
        assert!(mesh.contains("2 more"), "{mesh}");
        let releases: Vec<Value> = (0..ROWS + 1)
            .map(|i| serde_json::json!({ "version": format!("0.{i}.0"), "published_at": "2026-10-01T00:00:00Z" }))
            .collect();
        let all = answer(RELEASES, serde_json::json!({ "releases": releases }));
        let version = answer(VERSION, serde_json::json!({}));
        assert!(releases_panel(&all, &version, Some(ROWS))
            .render()
            .contains("1 more"));
        let whole = releases_panel(&all, &version, None).render();
        assert!(!whole.contains("more not shown"), "{whole}");
        assert!(
            whole.contains("none · no commit"),
            "a release with neither says so: {whole}"
        );
    }
}
