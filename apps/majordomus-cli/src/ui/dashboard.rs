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
    counts, day, donut, hero, kpi, kpi_count, kpi_state, kpi_to, list, meter, nothing, panel_to,
    shown, state, strip, table, text, timeline, unanswered, wide,
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
            git("/repository/git/working_tree").unwrap_or_else(|| "unknown".into())
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
        .child(releases_panel(a(RELEASES), a(VERSION)))
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
        let rows = list(roadmap, "/milestones")
            .iter()
            .filter(|m| m.get("status").and_then(Value::as_str) != Some("DONE"))
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
                    .attr("href", format!("{}#milestone-{id}", super::roadmap::ROUTE))
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

/// What was published, newest first, as a timeline.
pub fn releases_panel(releases: &Answer, version: &Answer) -> El {
    let body = unanswered(releases).unwrap_or_else(|| {
        let entries = list(releases, "/releases")
            .iter()
            .take(ROWS)
            .map(|r| {
                let commit = text(r, "commit");
                (
                    text(r, "published_at"),
                    format!("v{}", text(r, "version")),
                    el("span").class("mj-ui-dim").text(format!(
                        "{} · {}",
                        text(r, "channel"),
                        &commit[..10.min(commit.len())]
                    )),
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
        let rows = list(mesh, "/machines")
            .iter()
            .take(ROWS)
            .map(machine_row)
            .collect::<Vec<_>>();
        table(
            &["Machine", "Where", "Runtimes"],
            rows,
            "No machine is known to the mesh runtime.",
        )
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
            kpi("Merged, 7 days", &tally("/throughput/merges")),
            kpi("Merges per day", &tally("/throughput/merges_per_day")),
        ]);
        let rows = list(queue, "/queue/assessments")
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
                            .take(2)
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
                                        || super::work::ROUTE.to_string(),
                                        |n| format!("{}?pr={n}#pr", super::work::ROUTE),
                                    ),
                                )
                                .child(el("span").class("mj-ui-cell-title").text(text(a, "title"))),
                        ),
                    )
                    .child(el("td").child(state(a, "disposition")))
                    .child(el("td").text(text(a, "lane")))
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(reasons.join(", ")),
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
            .when(detail.is_some(), |s| {
                s.child(detail.clone().unwrap_or_else(|| el("div")))
            })
            .child(tallies)
            .child(table(
                &["PR", "Title", "Disposition", "Lane", "Why"],
                rows,
                "No pull request is open.",
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
        Some(super::work::ROUTE),
        body,
        &[queue],
    )
}

/// One pull request as the queue assessed it: what it is, every gate with its verdict, the
/// evidence each verdict was read from, why it stands where it stands, what would move it,
/// and the factors of its rank. Read from the queue's own answer, so it costs nothing more.
fn pull_request_detail(a: &Value) -> El {
    let s = |k: &str| text(a, k);
    let number = a.get("number").and_then(Value::as_u64).unwrap_or_default();
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
            at: std::time::SystemTime::now(),
        };
        let html = queue_panel(&queue, Some(7)).render();
        assert!(
            html.contains("id=\"pr\"") && html.contains("bring master in"),
            "{html}"
        );
        assert!(html.contains(">base<") && html.contains(">ci<"));
        assert!(
            html.contains("/ui/work?pr=7#pr"),
            "rows lead to their detail"
        );
        assert!(queue_panel(&queue, Some(8))
            .render()
            .contains("#8 is not among"));
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
            at: std::time::SystemTime::now(),
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
}
