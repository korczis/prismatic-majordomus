//! PR Drain: every open pull request as the integration lane assessed it, narrowed by what
//! the reader picks, with one of them inspected in full, and the issue it carries.
//!
//! The queue (`integration.queue`) asks the forge and takes tens of seconds, so the screen
//! paints first and the drain arrives as a panel ([`PANEL`]), as the Dashboard's queue does.
//! Every control is a link or a GET form: the disposition and lane tabs, the search, a row,
//! a dependency, an overlap, a linked issue. Each leads to this screen with the choice in its
//! query, so the screen is complete without JavaScript and every view of it has an address.
//!
//! An issue the reader picks (`?issue=I2417`) is read from `plan.issues`, which answers in
//! milliseconds, so its inspector is part of the first paint.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;

use super::icons::icon;
use super::shell::View;
use super::source::{ask_all, Answer, Availability, Reading, Source};
use super::view::{
    code_link, day, graph, hero, kpi, kpi_to, list, meter, more, none, nothing, panel, panel_to,
    state, status_tone, strip, table, text, unanswered, value, wide, GraphNode, Layout,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/prs";

/// The route of the drain itself, the slow panel the screen fills once it has painted.
pub const PANEL: &str = "/ui/panels/drain";

const QUEUE: Source = Source::of("integration.queue");
const EVENTS: Source = Source::of("integration.events");

/// How many of the executor's events the inspector and the activity list show; each says how
/// many more the trail holds.
const TRAIL: usize = 15;

/// How many pull requests a "next up" line names before it says how many more there are.
const NEXT_UP: usize = 12;

const ISSUES: Source = Source::of("plan.issues");

/// The query parameters this screen reads, and nothing else is carried from one view of it
/// to the next.
const KEYS: &[&str] = &["disposition", "lane", "q", "pr", "issue"];

/// What the reader picked: which disposition or lane, what to search for, which pull
/// request and which issue to inspect. Each is optional; an issue id that is not one is
/// dropped rather than echoed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// A disposition, in the queue's own word (`needs_refresh`).
    pub disposition: Option<String>,
    /// A lane, in the queue's own word (`repair`).
    pub lane: Option<String>,
    /// Text a title, branch, author or number must contain.
    pub q: Option<String>,
    /// The pull request to inspect.
    pub pr: Option<u64>,
    /// The plan issue to inspect (`I2417`).
    pub issue: Option<String>,
}

impl Filter {
    /// The filter a query names.
    ///
    /// ```text
    /// let f = Filter::from_query(&[("pr".into(), "820".into()), ("issue".into(), "x".into())]);
    /// assert_eq!((f.pr, f.issue), (Some(820), None), "an issue id that is not one is dropped");
    /// ```
    pub fn from_query(query: &[(String, String)]) -> Self {
        let get = |k: &str| {
            query
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        Filter {
            disposition: get("disposition").filter(|w| is_word(w)),
            lane: get("lane").filter(|w| is_word(w)),
            q: get("q"),
            pr: get("pr").and_then(|v| v.trim_start_matches('#').parse().ok()),
            issue: get("issue").filter(|i| is_issue_id(i)),
        }
    }

    /// This filter with one parameter set to `value`, or cleared when `value` is `None`, as
    /// the query of a link to this screen or to `base`.
    fn href(&self, base: &str, key: &str, value: Option<&str>) -> String {
        let mut pairs: Vec<(&str, String)> = Vec::new();
        for k in KEYS {
            let v = if *k == key {
                value.map(str::to_string)
            } else {
                self.get(k)
            };
            if let Some(v) = v {
                pairs.push((k, v));
            }
        }
        if pairs.is_empty() {
            return base.to_string();
        }
        let query = pairs
            .iter()
            .map(|(k, v)| format!("{k}={}", encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{base}?{query}")
    }

    fn get(&self, key: &str) -> Option<String> {
        match key {
            "disposition" => self.disposition.clone(),
            "lane" => self.lane.clone(),
            "q" => self.q.clone(),
            "pr" => self.pr.map(|n| n.to_string()),
            "issue" => self.issue.clone(),
            _ => None,
        }
    }

    /// Does this assessment pass the filter? The pull request inspected is not a filter: it
    /// is shown whatever the tabs say.
    fn admits(&self, a: &Value) -> bool {
        let word = |k: &str, want: &Option<String>| {
            want.as_deref()
                .is_none_or(|w| a.get(k).and_then(Value::as_str) == Some(w))
        };
        let found = self.q.as_deref().is_none_or(|q| {
            let q = q.to_lowercase();
            let number = number(a);
            ["title", "head_ref", "author"]
                .iter()
                .any(|k| text(a, k).to_lowercase().contains(&q))
                || number.is_some_and(|n| format!("#{n}") == q || n.to_string() == q)
        });
        word("disposition", &self.disposition) && word("lane", &self.lane) && found
    }
}

/// A word of the queue's vocabulary: lower case, digits and underscores, nothing a link
/// could carry anything else in.
fn is_word(w: &str) -> bool {
    !w.is_empty()
        && w.len() <= 40
        && w.bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
}

/// A plan issue id: `I` and four digits.
fn is_issue_id(i: &str) -> bool {
    i.len() == 5 && i.starts_with('I') && i[1..].bytes().all(|b| b.is_ascii_digit())
}

/// A query value, percent-encoded except for the characters a value may carry as they are.
fn encode(v: &str) -> String {
    v.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// The plan issues a pull request names, in its title or its branch, in the order they
/// first appear: a convention of this repository's titles (`(I2417)`), read and not assumed,
/// so a pull request that names none links to none.
pub fn issues_named(a: &Value) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for field in [text(a, "title"), text(a, "head_ref").to_uppercase()] {
        let bytes = field.as_bytes();
        for (at, _) in field.match_indices('I') {
            let id = field.get(at..at + 5).unwrap_or_default();
            let bounded = at
                .checked_sub(1)
                .and_then(|p| bytes.get(p))
                .is_none_or(|b| !b.is_ascii_alphanumeric())
                && bytes.get(at + 5).is_none_or(|b| !b.is_ascii_alphanumeric());
            if is_issue_id(id) && bounded && !found.iter().any(|f| f == id) {
                found.push(id.to_string());
            }
        }
    }
    found
}

/// The PR Drain screen.
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let filter = Filter::from_query(query);
    let answers = ask_all(ctx, &[ISSUES]);
    let issues = &answers[&ISSUES];

    let top = hero(
        "merge",
        "blue",
        "PR Drain",
        "Pull requests converging on master, one at a time.",
        "Every open pull request as the integration lane assessed it: what it waits for, what would move it, what it depends on and overlaps, and the issue it carries.",
        // the screen counts pull requests, and those only the drain below reads: the plan's
        // issue counts belong to the Roadmap and Issues screens, not to this one's hero
        Vec::new(),
    );

    let src = filter.href(PANEL, "", None);
    let drain = el("section")
        .class("mj-ui-panel")
        .attr("aria-label", "Pull-request drain")
        .attr("data-panel-src", src.clone())
        .attr("aria-busy", "false")
        .child(
            el("header").class("mj-ui-panel-head").child(
                el("div")
                    .class("mj-ui-panel-heading")
                    .child(el("h2").class("mj-ui-panel-title").text("Pull-request drain"))
                    .child(el("p").class("mj-ui-panel-sub").text(
                        "The integration lane asks the forge for every open pull request; this takes a while and arrives after the screen.",
                    )),
            ),
        )
        .child(
            el("div").class("mj-ui-panel-body").child(
                el("p").class("mj-ui-empty").child(
                    el("a")
                        .attr("href", src)
                        .text("Open the drain on its own page"),
                ),
            ),
        );

    let mut grid = el("div").class("mj-ui-grid");
    if let Some(id) = &filter.issue {
        grid = grid.child(wide(issue_panel(issues, id, &filter)));
    }
    grid = grid.child(wide(drain));
    View::new("PR Drain", el("div").child(top).child(grid))
}

/// The drain as a panel: what the fragment request and the page without JavaScript show.
pub fn panel_view(ctx: &Context, query: &[(String, String)]) -> View {
    let filter = Filter::from_query(query);
    let answers = ask_all(ctx, &[QUEUE, EVENTS]);
    View::new(
        "Pull-request drain",
        drain_with(&answers[&QUEUE], Some(&answers[&EVENTS]), &filter),
    )
}

/// The drain: the lane's tallies, what it does next, the tabs and the search, the pull
/// requests the filter admits in the queue's rank order, the one inspected, the blockers, and
/// the graph of what depends on what.
#[cfg(test)]
pub fn drain(queue: &Answer, filter: &Filter) -> El {
    drain_with(queue, None, filter)
}

/// [`drain`] with the executor's trail (`integration.events`): the inspected pull request's
/// own events, newest first, and the most recent events of the lane.
pub fn drain_with(queue: &Answer, events: Option<&Answer>, filter: &Filter) -> El {
    // newest first: the trail is recorded oldest first
    let trail: Vec<Value> = events
        .filter(|e| unanswered(e).is_none())
        .map(|e| {
            let mut all = list(e, "/events");
            all.reverse();
            all
        })
        .unwrap_or_default();
    let body = unanswered(queue).unwrap_or_else(|| {
        if queue.ok().and_then(|v| v.get("observed")) != Some(&Value::Bool(true)) {
            return nothing(&format!(
                "The forge was not observed in this checkout: {}",
                queue
                    .ok()
                    .and_then(|v| v.get("reason"))
                    .and_then(Value::as_str)
                    .unwrap_or("no reason was given")
            ));
        }
        let all = list(queue, "/queue/assessments");
        let repository = queue
            .ok()
            .and_then(|v| v.pointer("/queue/repository"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let shown: Vec<&Value> = all.iter().filter(|a| filter.admits(a)).collect();
        let inspected = filter.pr.map(|n| {
            all.iter().find(|a| number(a) == Some(n)).map_or_else(
                || {
                    nothing(&format!(
                        "#{n} is not among the open pull requests the queue observed."
                    ))
                },
                |a| {
                    let all_own: Vec<&Value> = trail
                        .iter()
                        .filter(|e| e.get("pr").and_then(Value::as_u64) == Some(n))
                        .collect();
                    let own: Vec<Value> =
                        all_own.iter().take(TRAIL).map(|e| (*e).clone()).collect();
                    inspector(a, &all, &repository, filter)
                        .child(
                            el("h4")
                                .class("mj-ui-panel-title")
                                .text("What the executor did"),
                        )
                        .child(if events.is_none() {
                            nothing("The executor's trail was not asked.")
                        } else if own.is_empty() {
                            nothing("The executor has recorded nothing about it.")
                        } else {
                            trail_of(&own, filter)
                        })
                        .children(more(own.len(), all_own.len(), None))
                },
            )
        });
        el("div")
            .class("mj-ui-stack")
            .child(tallies(queue))
            .when(events.is_some(), |s| s.child(throughput(&trail, queue)))
            .child(next_up(queue, filter))
            .child(tabs(queue, filter))
            .child(search(filter))
            .children(inspected)
            .child(rows(&shown, filter))
            .child(el("p").class("mj-ui-line").text(format!(
                "{} of {} open pull requests shown · observed {}",
                shown.len(),
                all.len(),
                queue
                    .ok()
                    .and_then(|v| v.pointer("/queue/observed_at"))
                    .and_then(Value::as_str)
                    .map(|t| t.replace('T', " "))
                    .unwrap_or_else(|| "at an unknown time".into())
            )))
            .child(blockers(&all, filter))
            .child(dependency_graph(&all, &repository, filter))
            .when(events.is_some(), |s| {
                let about_one: Vec<&Value> = trail
                    .iter()
                    .filter(|e| e.get("pr").and_then(Value::as_u64).is_some())
                    .collect();
                let recent: Vec<Value> =
                    about_one.iter().take(TRAIL).map(|e| (*e).clone()).collect();
                s.child(
                    el("div")
                        .attr("id", "activity")
                        .child(el("h3").class("mj-ui-panel-title").text("Recent activity"))
                        .child(if recent.is_empty() {
                            nothing("The executor has recorded nothing about a pull request.")
                        } else {
                            trail_of(&recent, filter)
                        })
                        .children(more(recent.len(), about_one.len(), None)),
                )
            })
    });
    let mut answers = vec![queue];
    answers.extend(events);
    panel_to(
        "Pull-request drain",
        "What the integration lane would merge, refresh, repair or wait for, and why.",
        Some(ROUTE),
        body,
        &answers,
    )
}

/// Merges per day: how many pull requests the executor merged on each of the `days` days
/// that end on the day of the trail's newest event, from its `merge_succeeded` events. The
/// number of days is the queue's own throughput window (`throughput/window_days`), so the
/// chart and the tile count over one window and the screen chooses none of its own. The
/// window ends where the record ends, not at the reader's clock, so the chart says what was
/// recorded and an idle week is a row of zeros the trail itself vouches for. Each day's count
/// is written beside its bar, and a table carries the same numbers.
pub fn merges_per_day(trail_newest_first: &[Value], days: u64) -> Vec<(String, u64)> {
    let day_of = |e: &Value| crate::peers::parse_rfc3339(&text(e, "at")).map(|s| s / 86_400);
    if days == 0 {
        return Vec::new();
    }
    let Some(last) = trail_newest_first.iter().filter_map(day_of).max() else {
        return Vec::new();
    };
    let first = last.saturating_sub(days - 1);
    let mut counts = vec![0u64; (last - first + 1) as usize];
    for e in trail_newest_first {
        if text(e, "action") != "merge_succeeded" {
            continue;
        }
        if let Some(d) = day_of(e).filter(|d| (first..=last).contains(d)) {
            counts[(d - first) as usize] += 1;
        }
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            let at =
                std::time::UNIX_EPOCH + std::time::Duration::from_secs((first + i as u64) * 86_400);
            (super::view::day(&crate::peers::rfc3339(at)), n)
        })
        .collect()
}

fn throughput(trail_newest_first: &[Value], queue: &Answer) -> El {
    let Some(window) = queue.read("/throughput/window_days").count() else {
        return el("div")
            .attr("id", "throughput")
            .child(el("h3").class("mj-ui-panel-title").text("Merge throughput"))
            .child(nothing(
                "The queue states no throughput window, so no day is counted here.",
            ));
    };
    let days = merges_per_day(trail_newest_first, window);
    let body = if days.iter().all(|(_, n)| *n == 0) {
        nothing("The executor recorded no merge in the days its trail covers.")
    } else {
        let max = days.iter().map(|(_, n)| *n).max().unwrap_or(1).max(1);
        let w = 100 / days.len().max(1) as u64;
        let bars: String = days
            .iter()
            .enumerate()
            .map(|(i, (day, n))| {
                let h = n * 40 / max;
                format!(
                    r#"<rect x="{x}" y="{y}" width="{bw}" height="{h}" rx="1" fill="currentColor"><title>{day}: {n}</title></rect><text x="{tx}" y="{ty}" font-size="4" text-anchor="middle" fill="currentColor">{n}</text>"#,
                    x = i as u64 * w + 1,
                    y = 45 - h,
                    bw = w.saturating_sub(2).max(1),
                    tx = i as u64 * w + w / 2,
                    ty = 44 - h,
                )
            })
            .collect();
        let rows = days
            .iter()
            .map(|(day, n)| {
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(day.clone()))
                    .child(el("td").class("mj-ui-mono").text(n.to_string()))
            })
            .collect::<Vec<_>>();
        el("div")
            .child(
                el("span")
                    .class("mj-tone--green")
                    .attr("role", "img")
                    .attr(
                        "aria-label",
                        format!(
                            "Merges per day from {} to {}: {}",
                            days.first().map(|d| d.0.clone()).unwrap_or_default(),
                            days.last().map(|d| d.0.clone()).unwrap_or_default(),
                            days.iter().map(|(_, n)| n.to_string()).collect::<Vec<_>>().join(", ")
                        ),
                    )
                    .raw(format!(
                        r#"<svg viewBox="0 0 100 46" width="100%" height="120" preserveAspectRatio="none" focusable="false">{bars}</svg>"#
                    )),
            )
            .child(table(&["Day", "Merges"], rows, "No day."))
    };
    el("div")
        .attr("id", "throughput")
        .child(el("h3").class("mj-ui-panel-title").text("Merge throughput"))
        .child(el("p").class("mj-ui-line mj-ui-dim").text(format!(
            "Over the queue's window of {window} days, ending on the day of the trail's newest event."
        )))
        .child(body)
}

/// Events of the executor's trail as a timeline, newest first: what it did, to which pull
/// request (a link to its inspector), why, and who did it.
fn trail_of(events: &[Value], filter: &Filter) -> El {
    super::view::timeline(
        events
            .iter()
            .map(|e| {
                let pr = e.get("pr").and_then(Value::as_u64);
                let action = text(e, "action").replace('_', " ");
                let title = match pr {
                    Some(n) => format!("#{n} {action}"),
                    None => action,
                };
                let reasons = strings(e, "reasons").join(", ");
                let detail = el("span")
                    .class("mj-ui-dim")
                    .when(pr.is_some(), |d| {
                        d.child(
                            el("a")
                                .class("mj-ui-more")
                                .attr(
                                    "href",
                                    format!(
                                        "{}#pr",
                                        filter.href(
                                            ROUTE,
                                            "pr",
                                            pr.map(|n| n.to_string()).as_deref()
                                        )
                                    ),
                                )
                                .text("open"),
                        )
                    })
                    .child(el("span").text(format!(
                        " {}{}{}",
                        text(e, "detail"),
                        if reasons.is_empty() { String::new() } else { format!(" · {reasons}") },
                        Some(text(e, "actor"))
                            .filter(|a| !a.is_empty())
                            .map(|a| format!(" · {a}"))
                            .unwrap_or_default()
                    )));
                (text(e, "at"), title, detail)
            })
            .collect(),
    )
}

fn number(a: &Value) -> Option<u64> {
    a.get("number").and_then(|n| {
        n.as_u64()
            .or_else(|| n.as_str().and_then(|s| s.parse().ok()))
    })
}

/// The lane's tallies, each tile a link to the pull requests it counts.
fn tallies(queue: &Answer) -> El {
    let to_lane = |lane: &str| Filter::default().href(ROUTE, "lane", Some(lane));
    // every lane the queue counted, in the queue's own words: a lane it adds is a card
    // here without a change to this screen, and one it drops is not a card counting nothing.
    // Each count is read from the one map the queue answers, which its source line names.
    let by_lane = queue.read("/queue/tallies/by_lane");
    let lanes: Vec<(String, Value)> = match &by_lane.state {
        Availability::Available(Value::Object(m)) => {
            m.iter().map(|(k, n)| (k.clone(), n.clone())).collect()
        }
        _ => Vec::new(),
    };
    let mut cards = vec![kpi_to(
        kpi("Open", &queue.read("/queue/tallies/open")),
        ROUTE,
    )];
    cards.extend(lanes.into_iter().map(|(lane, n)| {
        let count = Reading {
            state: Availability::Available(n),
            ..by_lane.clone()
        };
        kpi_to(kpi(&lane_label(&lane), &count), &to_lane(&lane))
    }));
    cards.push(kpi(
        &super::dashboard::merged_label(queue),
        &queue.read("/throughput/merges"),
    ));
    cards.push(kpi(
        "Merges per day",
        &queue.read("/throughput/merges_per_day"),
    ));
    strip(cards)
}

/// A lane's word as a card's label: `repair` reads "Repair lane", `waiting` "Waiting lane".
fn lane_label(lane: &str) -> String {
    let words = lane.replace('_', " ");
    let mut chars = words.chars();
    let first = chars.next().map(|c| c.to_uppercase().collect::<String>());
    format!("{}{} lane", first.unwrap_or_default(), chars.as_str())
}

/// What the lane does next: the pull request it would merge, those it would refresh, those
/// starving, and the last merge it recorded. Each pull request is a link to its inspector.
fn next_up(queue: &Answer, filter: &Filter) -> El {
    let v = queue.ok();
    let links = |ns: &[u64]| -> El {
        if ns.is_empty() {
            return el("span").class("mj-ui-dim").text("none");
        }
        el("span")
            .children(ns.iter().take(NEXT_UP).map(|n| {
                el("a")
                    .class("mj-ui-chip mj-ui-chip--link")
                    .attr("href", filter.href(ROUTE, "pr", Some(&n.to_string())))
                    .child(el("code").class("mj-ui-chip-value").text(format!("#{n}")))
            }))
            .when(ns.len() > NEXT_UP, |s| {
                s.child(el("span").class("mj-ui-dim").text(format!(
                    " and {} more in the rows below",
                    ns.len() - NEXT_UP
                )))
            })
    };
    // a list the queue did not answer is unknown; an empty one it answered is none
    let listed = |p: &str| -> El {
        match v
            .and_then(|v| v.pointer(p))
            .and_then(Value::as_array)
            .map(|xs| xs.iter().filter_map(Value::as_u64).collect::<Vec<_>>())
        {
            Some(ns) => links(&ns),
            None => el("span").class("mj-ui-dim").text("unknown"),
        }
    };
    let next_merge: Vec<u64> = v
        .and_then(|v| v.pointer("/queue/next_merge"))
        .and_then(|m| {
            m.as_u64()
                .or_else(|| m.get("number").and_then(Value::as_u64))
        })
        .into_iter()
        .collect();
    let last = v.and_then(|v| v.get("last_merge")).filter(|m| !m.is_null());
    let fact = |label: &str, value: El| {
        el("div")
            .class("mj-ui-fact")
            .child(el("dt").text(label))
            .child(el("dd").child(value))
    };
    el("dl")
        .class("mj-ui-facts")
        .attr("aria-label", "What the lane does next")
        .child(fact("Next merge", links(&next_merge)))
        .child(fact("Next refresh", listed("/queue/next_refresh")))
        .child(fact("Starving", listed("/queue/starving")))
        .child(fact(
            "Last merge",
            match last {
                Some(m) => el("span")
                    .child(links(
                        &m.get("pr")
                            .and_then(Value::as_u64)
                            .into_iter()
                            .collect::<Vec<_>>(),
                    ))
                    .child(el("span").class("mj-ui-dim").text(format!(
                        " {} · {}",
                        text(m, "at").replace('T', " "),
                        text(m, "detail")
                    ))),
                None => el("span").class("mj-ui-dim").text("none recorded"),
            },
        ))
}

/// The dispositions the queue counted, as tabs: the queue's own words and counts, so a
/// disposition nobody holds has no tab, and a new one appears without a change here.
fn tabs(queue: &Answer, filter: &Filter) -> El {
    let counts: Option<Vec<(String, u64)>> = queue
        .ok()
        .and_then(|v| v.pointer("/queue/tallies/by_disposition"))
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n)))
                .collect()
        });
    // a queue that did not count its dispositions has no tabs, rather than an `All 0`
    let Some(counts) = counts else {
        return el("p")
            .class("mj-ui-empty")
            .text("The queue did not count its dispositions.");
    };
    let total: u64 = counts.iter().map(|(_, n)| n).sum();
    let tab = |label: String, href: String, current: bool| {
        el("a")
            .class("mj-ui-tab")
            .attr("href", href)
            .attr_if("aria-current", current.then_some("page"))
            .text(label)
    };
    el("nav")
        .class("mj-ui-tabs")
        .attr("aria-label", "Which pull requests")
        .child(tab(
            format!("All {total}"),
            filter.href(ROUTE, "disposition", None),
            filter.disposition.is_none(),
        ))
        .children(counts.iter().map(|(word, n)| {
            tab(
                format!("{} {n}", word.replace('_', " ")),
                filter.href(ROUTE, "disposition", Some(word)),
                filter.disposition.as_deref() == Some(word.as_str()),
            )
        }))
}

/// The search: a GET form to this screen, carrying the tabs the reader is on.
fn search(filter: &Filter) -> El {
    let hidden = |k: &str, v: Option<String>| {
        v.map(|v| {
            el("input")
                .attr("type", "hidden")
                .attr("name", k)
                .attr("value", v)
        })
    };
    el("form")
        .class("mj-ui-search")
        .attr("role", "search")
        .attr("method", "get")
        .attr("action", ROUTE)
        .child(icon("search"))
        .child(
            el("label")
                .class("mj-sr-only")
                .attr("for", "mj-ui-drain-q")
                .text("Search pull requests"),
        )
        .child(
            el("input")
                .attr("id", "mj-ui-drain-q")
                .attr("type", "search")
                .attr("name", "q")
                .attr("placeholder", "Title, branch, author or #number")
                .attr("value", filter.q.clone().unwrap_or_default()),
        )
        .children(hidden("disposition", filter.disposition.clone()))
        .children(hidden("lane", filter.lane.clone()))
        .when(filter.q.is_some(), |f| {
            f.child(
                el("a")
                    .class("mj-ui-more")
                    .attr("href", filter.href(ROUTE, "q", None))
                    .text("Clear"),
            )
        })
}

/// The pull requests the filter admits, in the queue's rank order, each row a link to its
/// inspector.
fn rows(shown: &[&Value], filter: &Filter) -> El {
    let rows = shown
        .iter()
        .map(|a| {
            let n = number(a);
            let reasons = strings(a, "reasons");
            el("tr")
                .attr_if("id", n.map(|n| format!("pr-{n}")))
                .attr_if(
                    "aria-current",
                    (n.is_some() && n == filter.pr).then_some("true"),
                )
                .child(
                    el("td")
                        .class("mj-ui-mono")
                        .text(n.map_or("?".into(), |n| format!("#{n}"))),
                )
                .child(
                    el("td").child(
                        el("a")
                            .class("mj-ui-row-link")
                            .attr(
                                "href",
                                format!(
                                    "{}#pr",
                                    filter.href(ROUTE, "pr", n.map(|n| n.to_string()).as_deref())
                                ),
                            )
                            .child(el("span").class("mj-ui-cell-title").text(text(a, "title"))),
                    ),
                )
                .child(
                    el("td")
                        .class("mj-ui-mono mj-ui-dim")
                        .text(text(a, "head_ref")),
                )
                .child(el("td").text(text(a, "author")))
                .child(el("td").child(state(a, "disposition")))
                .child(el("td").child(state(a, "lane")))
                .child(el("td").child(state(a, "risk")))
                .child(
                    el("td")
                        .class("mj-ui-mono mj-ui-dim")
                        .text(super::dashboard::first_reasons(&reasons)),
                )
        })
        .collect::<Vec<_>>();
    table(
        &[
            "PR",
            "Title",
            "Branch",
            "Author",
            "Disposition",
            "Lane",
            "Risk",
            "Why",
        ],
        rows,
        "No open pull request matches.",
    )
}

fn strings(a: &Value, k: &str) -> Vec<String> {
    a.get(k)
        .and_then(Value::as_array)
        .map(|v| {
            v.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// How many paths a row of chips shows before it says how many more there are.
const PATHS: usize = 6;

/// Paths as chips: each its own element, so a row of them wraps between paths and a long one
/// is clipped inside its chip rather than widening the screen on a phone. A long list shows
/// its first [`PATHS`] and says how many more there are, with every path in its title.
fn path_chips(paths: &[String]) -> El {
    if paths.is_empty() {
        return none();
    }
    el("span")
        .class("mj-ui-badges")
        .children(paths.iter().take(PATHS).map(|p| {
            el("span")
                .class("mj-ui-chip")
                .attr("title", p.clone())
                .child(el("code").class("mj-ui-chip-value").text(p.clone()))
        }))
        .when(paths.len() > PATHS, |s| {
            s.child(
                el("span")
                    .class("mj-ui-chip mj-ui-dim")
                    .attr("title", paths[PATHS..].join("\n"))
                    .text(format!("and {} more", paths.len() - PATHS)),
            )
        })
}

/// One pull request in full: the queue's assessment of it, where it is on the forge, what
/// it depends on and overlaps (each a link to that pull request), and the issues it names.
fn inspector(a: &Value, all: &[Value], repository: &str, filter: &Filter) -> El {
    let n = number(a).unwrap_or_default();
    let open = |m: u64| all.iter().any(|x| number(x) == Some(m));
    let pr_link = |m: u64, extra: String| {
        let href = if open(m) {
            format!("{}#pr", filter.href(ROUTE, "pr", Some(&m.to_string())))
        } else if !repository.is_empty() {
            format!("https://github.com/{repository}/pull/{m}")
        } else {
            ROUTE.to_string()
        };
        el("li")
            .child(el("a").attr("href", href).text(format!("#{m}")))
            .child(el("span").class("mj-ui-dim").text(extra))
    };
    let dependencies = a
        .get("dependencies")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let overlaps = a
        .get("overlaps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let dep_list = el("ul")
        .class("mj-ui-prose-list")
        .children(dependencies.iter().filter_map(|d| {
            let m = d.get("number").and_then(Value::as_u64)?;
            let satisfied = d.get("satisfied").and_then(Value::as_bool).unwrap_or(false);
            Some(pr_link(
                m,
                format!(
                    " {} · {} · {}",
                    text(d, "state"),
                    text(d, "certainty"),
                    if satisfied {
                        "satisfied"
                    } else {
                        "not satisfied"
                    }
                ),
            ))
        }));
    let overlap_list = el("ul")
        .class("mj-ui-prose-list")
        .children(overlaps.iter().filter_map(|o| {
            let m = o.get("number").and_then(Value::as_u64)?;
            let paths = strings(o, "paths");
            Some(
                pr_link(
                    m,
                    format!(
                        " {} · {} shared path{}",
                        text(o, "kind"),
                        paths.len(),
                        if paths.len() == 1 { "" } else { "s" },
                    ),
                )
                .child(path_chips(&paths)),
            )
        }));
    let named = issues_named(a);
    let gates = el("ul").class("mj-ui-checks").children(
        a.get("gates")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|g| {
                el("li")
                    .class("mj-ui-check")
                    .child(
                        el("span")
                            .class("mj-ui-check-title mj-ui-mono")
                            .text(text(g, "gate")),
                    )
                    .child(word_badge(match g.get("passed").and_then(Value::as_bool) {
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
    let since = a
        .pointer("/wait/actionable_since")
        .and_then(Value::as_str)
        .map(day);
    let section = |title: &str, body: El| {
        el("div")
            .child(el("h4").class("mj-ui-panel-title").text(title))
            .child(body)
    };
    el("section")
        .class("mj-ui-detail")
        .attr("id", "pr")
        .attr("aria-label", format!("Pull request #{n}"))
        .child(
            el("div")
                .class("mj-ui-detail-head")
                .child(el("p").class("mj-ui-mono mj-ui-dim").text(format!(
                    "#{n} · {} · {} → {}",
                    text(a, "author"),
                    text(a, "head_ref"),
                    text(a, "base_ref")
                )))
                .child(el("h3").class("mj-ui-detail-title").text(text(a, "title")))
                .child(
                    el("p")
                        .class("mj-ui-badges")
                        .child(state(a, "disposition"))
                        .child(state(a, "lane"))
                        .child(state(a, "risk"))
                        .child(state(a, "required_checks"))
                        .child(state(a, "review")),
                )
                .child(
                    el("p")
                        .class("mj-ui-line")
                        .when(!repository.is_empty(), |p| {
                            p.child(
                                el("a")
                                    .class("mj-ui-more")
                                    .attr(
                                        "href",
                                        format!("https://github.com/{repository}/pull/{n}"),
                                    )
                                    .attr("rel", "noopener noreferrer")
                                    .child(icon("github"))
                                    .child(el("span").text("Open on GitHub")),
                            )
                        })
                        .child(
                            el("a")
                                .class("mj-ui-more")
                                .attr("href", format!("{}#pr", filter.href(ROUTE, "pr", None)))
                                .text("Close"),
                        ),
                ),
        )
        .child(el("p").class("mj-ui-line").text(format!(
            "Next: {}",
            Some(text(a, "next_action"))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "nothing to do".into())
        )))
        .child(el("p").class("mj-ui-mono mj-ui-dim").text(format!(
            "{}{}",
            strings(a, "reasons").join(" · "),
            since.map(|d| format!(" · actionable since {d}")).unwrap_or_default()
        )))
        .when(!named.is_empty(), |s| {
            s.child(section(
                "Issues it names",
                el("p")
                    .class("mj-ui-badges")
                    .children(named.iter().map(|id| {
                        el("a")
                            .class("mj-ui-chip mj-ui-chip--link")
                            .attr("href", filter.href(ROUTE, "issue", Some(id)))
                            .child(el("code").class("mj-ui-chip-value").text(id.clone()))
                    })),
            ))
        })
        .child(section("Gates", gates))
        .child(section(
            "Depends on",
            if dependencies.is_empty() {
                nothing("It depends on no other pull request.")
            } else {
                // a pull request that depends on another is a link of a train: the Trains
                // screen draws the whole stack in landing order (I3601)
                el("div").child(dep_list).child(
                    el("p").class("mj-ui-line").child(
                        el("a")
                            .class("mj-ui-more")
                            .attr("href", super::trains::ROUTE)
                            .text("See its train, in landing order"),
                    ),
                )
            },
        ))
        .child(section(
            "Overlaps",
            if overlaps.is_empty() {
                nothing("It shares no path with another open pull request.")
            } else {
                overlap_list
            },
        ))
        .child(section(
            "Risk",
            el("p").class("mj-ui-dim").text({
                let f = strings(a, "risk_factors");
                if f.is_empty() {
                    "No factor was named.".into()
                } else {
                    f.join(" · ")
                }
            }),
        ))
        .child(section(
            "Evidence",
            table(
                &["Kind", "Verdict", "What it says"],
                evidence,
                "No evidence was recorded.",
            ),
        ))
        .child(section(
            "Paths it authors",
            path_chips(&strings(a, "authored_paths")),
        ))
}

/// What stops pull requests: those the lane cannot merge until someone repairs them, with
/// the first reason it gave, each a link to its inspector.
fn blockers(all: &[Value], filter: &Filter) -> El {
    let rows = all
        .iter()
        .filter(|a| {
            matches!(
                a.get("disposition").and_then(Value::as_str),
                Some("conflicting" | "needs_repair")
            )
        })
        .map(|a| {
            let n = number(a);
            el("tr")
                .child(
                    el("td").child(
                        el("a")
                            .attr(
                                "href",
                                format!(
                                    "{}#pr",
                                    filter.href(ROUTE, "pr", n.map(|n| n.to_string()).as_deref())
                                ),
                            )
                            .text(n.map_or("?".into(), |n| format!("#{n}"))),
                    ),
                )
                .child(el("td").child(state(a, "disposition")))
                .child(
                    el("td")
                        .class("mj-ui-mono mj-ui-dim")
                        .text(strings(a, "reasons").join(", ")),
                )
                .child(el("td").class("mj-ui-dim").text(text(a, "next_action")))
        })
        .collect::<Vec<_>>();
    el("div")
        .attr("id", "blockers")
        .child(el("h3").class("mj-ui-panel-title").text("Blockers"))
        .child(table(
            &["PR", "Disposition", "Why", "What would move it"],
            rows,
            "No pull request is conflicting or waiting for a repair.",
        ))
}

/// What depends on what: an edge from a pull request to each open pull request it depends
/// on, as the queue confirmed or inferred it. A dependency already merged is not drawn: it
/// no longer holds anything.
fn dependency_graph(all: &[Value], repository: &str, filter: &Filter) -> El {
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut fallback = Vec::new();
    for a in all {
        let Some(n) = number(a) else { continue };
        for d in a
            .get("dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(m) = d.get("number").and_then(Value::as_u64) else {
                continue;
            };
            if d.get("state").and_then(Value::as_str) == Some("merged") {
                continue;
            }
            edges.push((n.to_string(), m.to_string()));
            fallback.push(
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(format!("#{n}")))
                    .child(el("td").class("mj-ui-mono").text(format!("#{m}")))
                    .child(el("td").class("mj-ui-dim").text(format!(
                        "{} · {}",
                        text(d, "certainty"),
                        text(d, "state")
                    ))),
            );
        }
    }
    let mut nodes: Vec<GraphNode> = all
        .iter()
        .filter_map(|a| {
            let n = number(a)?;
            Some(GraphNode {
                id: n.to_string(),
                label: format!("#{n}"),
                tone: status_tone(&text(a, "disposition")),
                href: format!("{}#pr", filter.href(ROUTE, "pr", Some(&n.to_string()))),
                kind: "main",
            })
        })
        .collect();
    // a dependency the queue does not hold (it is not open against the base, or the forge
    // did not list it) is still drawn, as a small node leading to the forge
    let held: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut outside: Vec<String> = edges
        .iter()
        .map(|(_, m)| m.clone())
        .filter(|m| !held.contains(m))
        .collect();
    crate::order::canonical_strings(&mut outside);
    outside.dedup();
    nodes.extend(outside.into_iter().map(|m| GraphNode {
        href: if repository.is_empty() {
            ROUTE.to_string()
        } else {
            format!("https://github.com/{repository}/pull/{m}")
        },
        label: format!("#{m}"),
        tone: "violet",
        kind: "leaf",
        id: m,
    }));
    el("div")
        .attr("id", "dependencies")
        .child(
            el("h3")
                .class("mj-ui-panel-title")
                .text("What depends on what"),
        )
        .child(if edges.is_empty() {
            nothing("No open pull request depends on another open one.")
        } else {
            graph(
                "Dependencies between open pull requests",
                Layout::Directed,
                &nodes,
                &edges,
                table(
                    &["Pull request", "Depends on", "How it is known"],
                    fallback,
                    "None.",
                ),
            )
        })
}

/// One plan issue in full, from `plan.issues`: what it is for, where it stands, what it
/// waits for and what waits for it (each a link to that issue), the intents it serves, and
/// a link to the pull requests that name it.
fn issue_panel(issues: &Answer, id: &str, filter: &Filter) -> El {
    let body = unanswered(issues).unwrap_or_else(|| {
        let Some(i) = list(issues, "/issues")
            .into_iter()
            .find(|i| i.get("id").and_then(Value::as_str) == Some(id))
        else {
            return nothing(&format!("{id} is not an issue of the plan."));
        };
        let ids = |k: &str| -> El {
            let v = strings(&i, k);
            if v.is_empty() {
                return el("span").class("mj-ui-dim").text("none");
            }
            el("span").children(v.into_iter().map(|other| {
                el("a")
                    .class("mj-ui-chip mj-ui-chip--link")
                    .attr("href", filter.href(ROUTE, "issue", Some(&other)))
                    .child(el("code").class("mj-ui-chip-value").text(other))
            }))
        };
        let fact = |label: &str, value: El| {
            el("div")
                .class("mj-ui-fact")
                .child(el("dt").text(label))
                .child(el("dd").child(value))
        };
        let when = |k: &str| {
            el("span").class("mj-ui-mono").text(
                Some(text(&i, k))
                    .filter(|s| !s.is_empty())
                    .map_or("not yet".into(), |t| t.replace('T', " ")),
            )
        };
        el("section")
            .class("mj-ui-detail")
            .attr("id", "issue")
            .attr("aria-label", format!("Issue {id}"))
            .child(
                el("div")
                    .class("mj-ui-detail-head")
                    .child({
                        let milestone = text(&i, "milestone");
                        el("p")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(format!("{id} · "))
                            .child(if milestone.is_empty() {
                                none()
                            } else {
                                code_link(super::milestone::href(&milestone), milestone)
                            })
                            .text(format!(
                                " · {} · wave {}",
                                value(&i, "priority"),
                                i.get("wave")
                                    .map(Value::to_string)
                                    .unwrap_or_else(|| "unknown".into())
                            ))
                    })
                    .child(el("h3").class("mj-ui-detail-title").text(text(&i, "title")))
                    .child(
                        el("p")
                            .class("mj-ui-badges")
                            .child(state(&i, "status"))
                            .child(
                                el("a")
                                    .class("mj-ui-more")
                                    .attr("href", {
                                        let f = Filter {
                                            q: Some(id.to_string()),
                                            issue: Some(id.to_string()),
                                            ..Filter::default()
                                        };
                                        f.href(ROUTE, "", None)
                                    })
                                    .text("Pull requests that name it"),
                            )
                            .child(
                                el("a")
                                    .class("mj-ui-more")
                                    .attr("href", filter.href(ROUTE, "issue", None))
                                    .text("Close"),
                            ),
                    ),
            )
            .child(el("p").class("mj-ui-lead").text(text(&i, "objective")))
            .child(
                el("dl")
                    .class("mj-ui-facts")
                    .child(fact("Depends on", ids("depends_on")))
                    .child(fact("Blocked by", ids("blocked_by")))
                    .child(fact("Unblocks", ids("dependents")))
                    .child(fact("Serves", {
                        // an intent opens its own inspector on the roadmap
                        let intents = strings(&i, "serves");
                        if intents.is_empty() {
                            none()
                        } else {
                            el("span")
                                .class("mj-ui-badges")
                                .children(intents.into_iter().map(|served| {
                                    // `intent#criterion`: the intent opens, the criterion is
                                    // named in the label
                                    let intent = served.split('#').next().unwrap_or(&served);
                                    el("a")
                                        .class("mj-ui-chip mj-ui-chip--link")
                                        .attr("href", super::intent::href(intent))
                                        .child(
                                            el("code")
                                                .class("mj-ui-chip-value")
                                                .text(served.clone()),
                                        )
                                }))
                        }
                    }))
                    .child(fact("Scope", path_chips(&strings(&i, "scope"))))
                    .child(fact(
                        "Evidence",
                        meter(
                            i.get("evidence_have").and_then(Value::as_u64),
                            i.get("evidence_need").and_then(Value::as_u64),
                        ),
                    ))
                    .child(fact("Started", when("started_at")))
                    .child(fact("Verified", when("verified_at")))
                    .child(fact("Completed", when("completed_at"))),
            )
    });
    panel(
        "Issue",
        "One issue of the plan, as the plan records it.",
        body,
        &[issues],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::source::Exposure;
    use std::time::Duration;

    fn queue(value: Value) -> Answer {
        Answer {
            source: QUEUE,
            route: Some("GET /api/v1/pull-requests".into()),
            exposure: Exposure::default(),
            value: Ok(value.into()),
            took: Duration::from_millis(5),
            standing: crate::ui::source::Standing::NOW,
        }
    }

    fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn sample() -> Value {
        serde_json::json!({
            "observed": true,
            "last_merge": { "at": "2026-10-11T01:10:03Z", "pr": 908, "detail": "merged and verified" },
            "throughput": { "window_days": 7, "merges": 16, "merges_per_day": 2.29 },
            "queue": {
                "repository": "korczis/prismatic-majordomus",
                "observed_at": "2026-10-11T08:18:59Z",
                "next_merge": null,
                "next_refresh": [820],
                "starving": [],
                "tallies": {
                    "open": 3,
                    "by_disposition": { "needs_refresh": 1, "conflicting": 1, "waiting_for_dependency": 1 },
                    "by_lane": { "waiting": 2, "repair": 1 }
                },
                "assessments": [
                    { "number": 820, "title": "chore(plan): close the issues master holds (I1403)",
                      "author": "korczis", "head_ref": "chore/plan", "base_ref": "master",
                      "disposition": "needs_refresh", "lane": "waiting", "risk": "low",
                      "reasons": ["behind_master:2"], "next_action": "bring master in",
                      "dependencies": [], "overlaps": [{ "number": 916, "kind": "authored", "paths": ["a.yaml"] }],
                      "gates": [{ "gate": "base", "passed": true }] },
                    { "number": 931, "title": "chore(campaigns): the packs are tracked",
                      "author": "korczis", "head_ref": "chore/packs", "base_ref": "master",
                      "disposition": "conflicting", "lane": "repair", "risk": "low",
                      "reasons": ["conflicts_on:2"], "next_action": "resolve by hand",
                      "dependencies": [], "overlaps": [] },
                    { "number": 914, "title": "fix: stacked on another",
                      "author": "b4", "head_ref": "fix/stacked", "base_ref": "master",
                      "disposition": "waiting_for_dependency", "lane": "waiting", "risk": "medium",
                      "reasons": ["depends_on:909"],
                      "dependencies": [{ "number": 909, "certainty": "inferred", "satisfied": false, "state": "open" },
                                       { "number": 823, "certainty": "confirmed", "satisfied": true, "state": "merged" }],
                      "overlaps": [] }
                ]
            }
        })
    }

    fn events(value: Value) -> Answer {
        Answer {
            source: EVENTS,
            route: Some("GET /api/v1/pull-requests/events".into()),
            exposure: Exposure::default(),
            value: Ok(value.into()),
            took: Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        }
    }

    #[test]
    fn the_inspector_shows_what_the_executor_did_and_the_drain_its_recent_activity() {
        let trail = events(serde_json::json!({ "events": [
            { "at": "2026-10-11T01:00:00Z", "action": "observed", "pr": null, "detail": "3 open", "reasons": [], "actor": "x" },
            { "at": "2026-10-11T02:00:00Z", "action": "refresh_selected", "pr": 820, "detail": "behind", "reasons": ["behind_master:2"], "actor": "3d" },
            { "at": "2026-10-11T03:00:00Z", "action": "merge_attempted", "pr": 931, "detail": "", "reasons": [], "actor": "3d" },
            { "at": "2026-10-11T04:00:00Z", "action": "repaired", "pr": 820, "detail": "derived", "reasons": [], "actor": "3d" }
        ]}));
        let html = drain_with(
            &queue(sample()),
            Some(&trail),
            &Filter::from_query(&q(&[("pr", "820")])),
        )
        .render();
        let inspector = html
            .split("What the executor did")
            .nth(1)
            .unwrap_or_default();
        let inspector = inspector.split("</ol>").next().unwrap_or_default();
        assert!(
            inspector.contains("#820 repaired") && inspector.contains("#820 refresh selected"),
            "{inspector}"
        );
        assert!(
            !inspector.contains("#931"),
            "another pull request's events are not its own"
        );
        assert!(
            inspector.find("#820 repaired") < inspector.find("#820 refresh selected"),
            "newest first"
        );
        let activity = html.split("id=\"activity\"").nth(1).unwrap_or_default();
        assert!(
            activity.contains("#931 merge attempted") && activity.contains("/ui/prs?pr=931#pr")
        );
        assert!(
            !activity.contains(">observed<"),
            "an event about no pull request is not activity"
        );
        let without = drain(&queue(sample()), &Filter::from_query(&q(&[("pr", "820")]))).render();
        assert!(without.contains("trail was not asked."));
        assert!(!without.contains("id=\"activity\""));
    }

    #[test]
    fn merges_are_counted_per_day_over_the_days_the_trail_ends_on() {
        let ev =
            |at: &str, action: &str| serde_json::json!({ "at": at, "action": action, "pr": 1 });
        // newest first, as the drain passes it
        let trail = vec![
            ev("2026-10-11T09:00:00Z", "merge_succeeded"),
            ev("2026-10-11T08:00:00Z", "merge_attempted"),
            ev("2026-10-10T20:00:00Z", "merge_succeeded"),
            ev("2026-10-10T10:00:00Z", "merge_succeeded"),
            ev("2026-09-01T10:00:00Z", "merge_succeeded"),
        ];
        let days = merges_per_day(&trail, 14);
        assert_eq!(
            days.len(),
            14,
            "fourteen days end on the newest event's day"
        );
        assert_eq!(
            days.last(),
            Some(&("2026-10-11".to_string(), 1)),
            "an attempt is no merge"
        );
        assert_eq!(days[12], ("2026-10-10".to_string(), 2));
        assert_eq!(days[0].0, "2026-09-28");
        assert_eq!(
            days.iter().map(|(_, n)| n).sum::<u64>(),
            3,
            "a merge outside the window is not drawn"
        );
        assert!(merges_per_day(&[], 14).is_empty());
        assert!(
            merges_per_day(&trail, 0).is_empty(),
            "a window of no days counts none"
        );
        // the chart counts over the queue's own window, and says which
        let windowed = queue(serde_json::json!({ "throughput": { "window_days": 14 } }));
        let html = throughput(&trail, &windowed).render();
        assert!(
            html.contains("<svg") && html.contains("Merges per day from 2026-09-28 to 2026-10-11"),
            "{html}"
        );
        assert!(html.contains("window of 14 days"), "{html}");
        let seven = queue(serde_json::json!({ "throughput": { "window_days": 7 } }));
        assert!(throughput(&trail, &seven)
            .render()
            .contains("Merges per day from 2026-10-05 to 2026-10-11"));
        let silent = throughput(&trail, &queue(serde_json::json!({}))).render();
        assert!(
            silent.contains("states no throughput window") && !silent.contains("<svg"),
            "a window the queue does not state is never invented: {silent}"
        );
        let idle = throughput(&[ev("2026-10-11T09:00:00Z", "observed")], &windowed).render();
        assert!(idle.contains("recorded no merge") && !idle.contains("<svg"));
    }

    #[test]
    fn the_tabs_are_the_queues_own_dispositions_and_each_narrows_the_rows() {
        let a = queue(sample());
        let all = drain(&a, &Filter::default()).render();
        for word in [
            "needs refresh 1",
            "conflicting 1",
            "waiting for dependency 1",
            "All 3",
        ] {
            assert!(all.contains(word), "no tab {word}");
        }
        assert!(all.contains("#820") && all.contains("#931") && all.contains("#914"));
        let f = Filter::from_query(&q(&[("disposition", "conflicting")]));
        let narrowed = drain(&a, &f).render();
        assert!(
            narrowed.contains("1 of 3 open pull requests shown"),
            "{narrowed}"
        );
        assert!(narrowed.contains("aria-current=\"page\">conflicting 1"));
    }

    #[test]
    fn the_search_finds_a_title_a_branch_an_author_or_a_number() {
        let a = queue(sample());
        for (needle, want) in [
            ("packs", "#931"),
            ("fix/stacked", "#914"),
            ("b4", "#914"),
            ("#820", "#820"),
        ] {
            let html = drain(&a, &Filter::from_query(&q(&[("q", needle)]))).render();
            assert!(
                html.contains("1 of 3 open pull requests shown"),
                "{needle}: {html}"
            );
            assert!(html.contains(want), "{needle}");
        }
        let none = drain(&a, &Filter::from_query(&q(&[("q", "nothing like it")]))).render();
        assert!(none.contains("No open pull request matches."));
    }

    #[test]
    fn the_inspector_links_dependencies_overlaps_the_forge_and_the_issues_named() {
        let a = queue(sample());
        let html = drain(&a, &Filter::from_query(&q(&[("pr", "820")]))).render();
        assert!(html.contains("id=\"pr\""), "no inspector");
        assert!(html.contains("https://github.com/korczis/prismatic-majordomus/pull/820"));
        // #916 is not open in this queue, so its link leads to the forge
        assert!(html.contains("https://github.com/korczis/prismatic-majordomus/pull/916"));
        assert!(html.contains("/ui/prs?pr=820&amp;issue=I1403"), "{html}");
        let stacked = drain(&a, &Filter::from_query(&q(&[("pr", "914")]))).render();
        assert!(stacked.contains("inferred · not satisfied"));
        assert!(
            stacked.contains("href=\"/ui/trains\""),
            "a pull request with a dependency leads to its train: {stacked}"
        );
        let gone = drain(&a, &Filter::from_query(&q(&[("pr", "1")]))).render();
        assert!(gone.contains("#1 is not among the open pull requests"));
    }

    #[test]
    fn the_blockers_are_the_pull_requests_waiting_for_a_repair() {
        let html = drain(&queue(sample()), &Filter::default()).render();
        let blockers = html.split("id=\"blockers\"").nth(1).unwrap_or_default();
        let blockers = blockers
            .split("id=\"dependencies\"")
            .next()
            .unwrap_or_default();
        assert!(
            blockers.contains("#931") && !blockers.contains("#820"),
            "{blockers}"
        );
    }

    #[test]
    fn the_graph_draws_open_dependencies_and_never_a_merged_one() {
        let html = drain(&queue(sample()), &Filter::default()).render();
        assert!(html.contains("data-graph"), "no graph");
        let graph = html.split("data-graph=\"").nth(1).unwrap_or_default();
        let graph = graph.split('"').next().unwrap_or_default();
        assert!(
            graph.contains("909"),
            "the open dependency is drawn: {graph}"
        );
        assert!(!graph.contains("823"), "a merged dependency holds nothing");
    }

    #[test]
    fn an_unobserved_forge_says_so_and_draws_no_numbers() {
        let html = drain(
            &queue(serde_json::json!({ "observed": false, "reason": "gh is not authenticated" })),
            &Filter::default(),
        )
        .render();
        assert!(html.contains("not observed in this checkout: gh is not authenticated"));
        assert!(!html.contains("mj-ui-tabs"));
    }

    #[test]
    fn a_query_value_is_carried_encoded_and_a_bad_word_is_dropped() {
        let f = Filter::from_query(&q(&[
            ("q", "a b&c"),
            ("disposition", "x\" onclick=\""),
            ("issue", "I24170"),
            ("pr", "#931"),
        ]));
        assert_eq!(f.disposition, None);
        assert_eq!(f.issue, None);
        assert_eq!(f.pr, Some(931));
        assert_eq!(f.href(ROUTE, "", None), "/ui/prs?q=a%20b%26c&pr=931");
        assert_eq!(f.href(ROUTE, "pr", None), "/ui/prs?q=a%20b%26c");
    }

    #[test]
    fn the_issues_a_pull_request_names_are_read_from_its_title_and_branch() {
        let a = serde_json::json!({ "title": "feat(site): the story (I2417) and I2418x I241 XI2419", "head_ref": "fix/i2420-thing" });
        assert_eq!(
            issues_named(&a),
            vec!["I2417".to_string(), "I2420".to_string()]
        );
    }

    fn plan(value: Value) -> Answer {
        Answer {
            source: ISSUES,
            route: Some("GET /api/v1/plan/issues".into()),
            exposure: Exposure::default(),
            value: Ok(value.into()),
            took: Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        }
    }

    /// An issue opens with where it sits, what it waits for and unblocks (each a link to that
    /// issue), what it serves, its scope, its evidence and its dates, and says "none" and
    /// "not yet" for what it lacks rather than drawing nothing.
    #[test]
    fn an_issue_opens_with_what_it_waits_for_unblocks_and_serves() {
        let issues = plan(serde_json::json!({ "issues": [{
            "id": "I0002", "title": "The second thing", "milestone": "M1", "priority": "p1", "wave": 3,
            "status": "READY", "objective": "Make the second thing true.",
            "depends_on": ["I0001"], "blocked_by": [], "dependents": ["I0003"],
            "serves": ["intent-a#c1"], "scope": ["apps/a.rs"],
            "evidence_have": 1, "evidence_need": 2, "started_at": "2026-10-01T10:00:00Z"
        }]}));
        let filter = Filter::from_query(&q(&[("lane", "repair"), ("issue", "I0002")]));
        let html = issue_panel(&issues, "I0002", &filter).render();
        for want in [
            "id=\"issue\"",
            "I0002 · <a",
            "href=\"/ui/roadmap?milestone=M1#milestone\">M1</a> · p1 · wave 3",
            "href=\"/ui/roadmap?intent=intent-a#intent\"",
            "Make the second thing true.",
            "issue=I0001",
            "issue=I0003",
            "intent-a#c1",
            "apps/a.rs",
            "2026-10-01 10:00:00Z",
            "not yet",
            "Pull requests that name it",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(
            html.contains("lane=repair"),
            "a link keeps the drain's filter: {html}"
        );
        let missing = issue_panel(&issues, "I9999", &filter).render();
        assert!(missing.contains("I9999 is not an issue of the plan."));
    }

    /// A queue that left things out is drawn as what it said: no tabs without counts, an
    /// unknown time, no last merge, a number given as text, a gate without a verdict, a
    /// dependency without a number, and a trail that names no pull request.
    #[test]
    fn a_queue_that_left_things_out_is_drawn_as_what_it_said() {
        let queue = queue(serde_json::json!({
            "observed": true,
            "queue": {
                "repository": "",
                "assessments": [
                    { "number": "77", "title": "the seventy-seventh", "author": "a", "head_ref": "f/77",
                      "disposition": "needs_refresh", "lane": "waiting", "reasons": ["behind_master:1"],
                      "wait": { "actionable_since": "2026-10-10T08:00:00Z" },
                      "gates": [{ "gate": "ci", "passed": false }, { "gate": "review" }],
                      "dependencies": [{ "number": 5, "state": "merged", "satisfied": true }, { "state": "open" }],
                      "overlaps": [] },
                    { "number": 78, "title": "the seventy-eighth", "author": "a", "head_ref": "f/78",
                      "disposition": "waiting_for_dependency", "lane": "waiting", "reasons": ["depends_on:77"],
                      "dependencies": [{ "number": 77, "state": "open", "satisfied": false }],
                      "overlaps": [] }
                ]
            }
        }));
        let trail = events(serde_json::json!({ "events": [
            { "at": "2026-10-11T01:00:00Z", "action": "observed", "pr": null, "detail": "2 open", "reasons": [], "actor": "x" }
        ]}));
        let html = drain_with(
            &queue,
            Some(&trail),
            &Filter::from_query(&q(&[("pr", "77"), ("lane", "waiting")])),
        )
        .render();
        for want in [
            "The queue did not count its dispositions.",
            "at an unknown time",
            "none recorded",
            "id=\"pr\"",
            "actionable since",
            "No factor was named.",
            "The executor has recorded nothing about it.",
            "The executor has recorded nothing about a pull request.",
            ">fail<",
            ">unknown<",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        // #78 depends on #77, which is open: the link stays in the drain
        let html78 = drain_with(&queue, None, &Filter::from_query(&q(&[("pr", "78")]))).render();
        assert!(
            html78.contains("pr=77#pr"),
            "an open dependency leads to its inspector: {html78}"
        );
    }

    /// The drain counts pull requests by every lane the queue counted, each card a link to
    /// that lane, and a lane the queue did not count is no card; the screen's hero counts no
    /// plan issues, since this screen is about pull requests (I2850).
    #[test]
    fn the_counts_are_the_queues_own_lanes_and_never_the_plans_issues() {
        let mut v = sample();
        v["queue"]["tallies"]["by_lane"] =
            serde_json::json!({ "held": 4, "repair": 1, "ready": 2 });
        let html = tallies(&queue(v)).render();
        for (label, lane) in [
            ("Held lane", "held"),
            ("Repair lane", "repair"),
            ("Ready lane", "ready"),
        ] {
            assert!(html.contains(label), "no card {label}: {html}");
            assert!(
                html.contains(&format!("{ROUTE}?lane={lane}")),
                "{lane} is not a link: {html}"
            );
        }
        assert!(
            !html.contains("Waiting lane"),
            "a lane the queue did not count is a card"
        );
        assert_eq!(lane_label("waiting_for_review"), "Waiting for review lane");

        // a queue that counted no lane draws no lane card, and still counts what is open
        let mut bare = sample();
        bare["queue"]["tallies"]
            .as_object_mut()
            .expect("the tallies")
            .remove("by_lane");
        let html = tallies(&queue(bare)).render();
        assert!(html.contains("Open") && !html.contains(" lane<"), "{html}");

        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let hero = screen(&ctx, &[]).main.render();
        assert!(
            !hero.contains("Issues ready") && !hero.contains("plan.status"),
            "{hero}"
        );
    }

    #[test]
    fn the_screen_paints_the_issue_and_defers_the_drain() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let view = screen(
            &ctx,
            &q(&[("issue", "I0001"), ("disposition", "conflicting")]),
        );
        let html = view.main.render();
        assert!(
            html.contains(&format!(
                "data-panel-src=\"{PANEL}?disposition=conflicting&amp;issue=I0001\""
            )),
            "{html}"
        );
        assert!(html.contains("id=\"issue\"") || html.contains("is not an issue of the plan"));
    }

    /// The inspector draws each overlap with its own count of shared paths and skips one
    /// that names no pull request, lists the evidence and names the risk factors; an
    /// assessment without a number is neither a node nor an edge, and with no repository a
    /// dependency the queue does not hold leads back to the drain.
    #[test]
    fn the_inspector_draws_evidence_risk_and_every_overlap_it_can_name() {
        let queue = queue(serde_json::json!({
            "observed": true,
            "queue": {
                "repository": "",
                "assessments": [
                    { "number": 40, "title": "the fortieth", "author": "a", "head_ref": "f/40",
                      "disposition": "waiting_for_dependency", "lane": "waiting",
                      "risk_factors": ["wide_change", "no_tests"],
                      "evidence": [{ "kind": "ci_run", "status": "PASSED", "detail": "run 12 green" }],
                      "dependencies": [{ "number": 39, "state": "open", "certainty": "confirmed", "satisfied": false }],
                      "overlaps": [
                          { "number": 41, "kind": "authored", "paths": ["a.rs", "b.rs"] },
                          { "number": 42, "kind": "derived", "paths": ["c.rs"] },
                          { "kind": "nameless", "paths": ["d.rs"] }
                      ] },
                    { "title": "no number at all", "author": "a", "head_ref": "f/none",
                      "disposition": "needs_refresh", "lane": "waiting",
                      "dependencies": [{ "number": 40, "state": "open" }] }
                ]
            }
        }));
        let html = drain(&queue, &Filter::from_query(&q(&[("pr", "40")]))).render();
        for want in [
            "authored · 2 shared paths",
            "derived · 1 shared path<",
            "ci_run",
            "run 12 green",
            "wide_change · no_tests",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(
            !html.contains("nameless"),
            "an overlap without a number: {html}"
        );
        assert!(
            !html.contains("https://github.com"),
            "no repository, no forge: {html}"
        );
        let graph = html.split("data-graph=\"").nth(1).unwrap_or_default();
        let graph = graph.split('"').next().unwrap_or_default();
        assert!(
            graph.contains("39"),
            "the dependency outside is drawn: {graph}"
        );
    }

    /// A drain whose pull requests depend on no open one says so instead of drawing a graph.
    #[test]
    fn no_dependency_between_open_pull_requests_draws_no_graph() {
        let html = dependency_graph(
            &[serde_json::json!({ "number": 1, "dependencies": [] })],
            "",
            &Filter::default(),
        )
        .render();
        assert!(html.contains("No open pull request depends on another open one."));
        assert!(!html.contains("data-graph"));
    }

    /// An event about no pull request is titled by its action alone and opens nothing, and a
    /// key the filter does not carry has no value.
    #[test]
    fn an_event_about_no_pull_request_is_its_action_alone() {
        let html = trail_of(
            &[
                serde_json::json!({ "at": "2026-10-11T01:00:00Z", "action": "lane_observed",
                "pr": null, "detail": "3 open", "reasons": [], "actor": "" }),
            ],
            &Filter::default(),
        )
        .render();
        assert!(html.contains("lane observed"), "{html}");
        assert!(!html.contains("#pr"), "{html}");
        assert_eq!(Filter::from_query(&q(&[("pr", "1")])).get("wave"), None);
    }

    /// An issue that gives no wave and serves nothing says so.
    #[test]
    fn an_issue_without_a_wave_or_an_intent_says_so() {
        let issues = plan(serde_json::json!({ "issues": [{
            "id": "I0007", "title": "The seventh", "milestone": "M1", "priority": "p2",
            "status": "READY", "serves": []
        }]}));
        let html = issue_panel(&issues, "I0007", &Filter::default()).render();
        assert!(html.contains(">M1</a> · p2 · wave unknown"), "{html}");
        assert!(
            html.contains("<dt>Serves</dt><dd><span class=\"mj-ui-dim\">none</span>"),
            "{html}"
        );
    }

    /// A line of pull requests names its first dozen and how many more there are, a row of
    /// paths its first six, and an issue with no milestone says none.
    #[test]
    fn a_long_line_of_pull_requests_or_paths_counts_the_rest() {
        let ns: Vec<u64> = (1..=NEXT_UP as u64 + 3).collect();
        let q = queue(
            serde_json::json!({ "queue": { "next_refresh": ns, "starving": [], "tallies": {} } }),
        );
        let html = next_up(&q, &Filter::default()).render();
        assert!(html.contains("and 3 more in the rows below"), "{html}");
        let paths: Vec<String> = (0..PATHS + 2).map(|i| format!("p/{i}.rs")).collect();
        let chips = path_chips(&paths).render();
        assert!(
            chips.contains("and 2 more") && chips.contains("p/7.rs"),
            "{chips}"
        );
        assert!(
            !chips.contains(">p/7.rs<"),
            "the rest is in the title: {chips}"
        );
        assert!(path_chips(&[]).render().contains(">none<"));
        let issues = plan(
            serde_json::json!({ "issues": [{ "id": "I0008", "title": "Adrift", "status": "READY" }]}),
        );
        let html = issue_panel(&issues, "I0008", &Filter::default()).render();
        assert!(
            html.contains("I0008 · <span class=\"mj-ui-dim\">none</span> · none · wave unknown"),
            "{html}"
        );
    }
}
