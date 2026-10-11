//! Issues: every issue of the plan, narrowed by what the reader picks — a status, a
//! milestone, a priority, text — each a link to its inspector, its milestone a link to the
//! milestone's, and what blocks it a link to that issue.
//!
//! Read from `plan.issues`, `plan.status` and `plan.roadmap`, all of which answer in
//! milliseconds, so the screen is one paint. Every control is a link or a GET form: the
//! status tabs, the milestone and priority choices, the search. A value a link carries that is
//! not one of the plan's own words is dropped, never echoed.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{hero, kpi, list, meter, panel, state, table, text, unanswered, wide};

/// The route of the screen.
pub const ROUTE: &str = "/ui/issues";

const ISSUES: Source = Source::of("plan.issues");
const PLAN: Source = Source::of("plan.status");
const ROADMAP: Source = Source::of("plan.roadmap");

/// The query parameters this screen reads.
const KEYS: &[&str] = &["status", "milestone", "priority", "q"];

/// What the reader picked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pick {
    /// A status, as the plan writes it (`READY`).
    pub status: Option<String>,
    /// A milestone id.
    pub milestone: Option<String>,
    /// A priority (`p0`).
    pub priority: Option<String>,
    /// Text an id, title or objective must contain.
    pub q: Option<String>,
}

/// A word of the plan's vocabulary: letters, digits, `-` and `_`, as its statuses,
/// priorities and milestone ids are.
fn is_word(w: &str) -> bool {
    !w.is_empty()
        && w.len() <= 64
        && w.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl Pick {
    /// The pick a query names; a value that is not a word of the plan is dropped.
    pub fn from_query(query: &[(String, String)]) -> Self {
        let get = |k: &str| {
            query
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        Pick {
            status: get("status").filter(|w| is_word(w)),
            milestone: get("milestone").filter(|w| is_word(w)),
            priority: get("priority").filter(|w| is_word(w)),
            q: get("q"),
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        match key {
            "status" => self.status.clone(),
            "milestone" => self.milestone.clone(),
            "priority" => self.priority.clone(),
            "q" => self.q.clone(),
            _ => None,
        }
    }

    /// This pick with `key` set to `value`, or cleared, as a link to this screen.
    fn href(&self, key: &str, value: Option<&str>) -> String {
        let pairs: Vec<String> = KEYS
            .iter()
            .filter_map(|k| {
                let v = if *k == key {
                    value.map(str::to_string)
                } else {
                    self.get(k)
                };
                v.map(|v| format!("{k}={}", encode(&v)))
            })
            .collect();
        if pairs.is_empty() {
            ROUTE.to_string()
        } else {
            format!("{ROUTE}?{}", pairs.join("&"))
        }
    }

    /// Does this issue pass the pick?
    fn admits(&self, i: &Value) -> bool {
        let is = |k: &str, want: &Option<String>| {
            want.as_deref()
                .is_none_or(|w| text(i, k).eq_ignore_ascii_case(w))
        };
        let found = self.q.as_deref().is_none_or(|q| {
            let q = q.to_lowercase();
            ["id", "title", "objective", "slug"]
                .iter()
                .any(|k| text(i, k).to_lowercase().contains(&q))
        });
        is("status", &self.status)
            && is("milestone", &self.milestone)
            && is("priority", &self.priority)
            && found
    }
}

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

fn chip(href: String, label: String) -> El {
    el("a")
        .class("mj-ui-chip mj-ui-chip--link")
        .attr("href", href)
        .child(el("code").class("mj-ui-chip-value").text(label))
}

fn issue_href(id: &str) -> String {
    format!("{}?issue={id}#issue", super::prs::ROUTE)
}

/// The values of `key` the issues hold, each with how many hold it, in the plan's own words.
fn tally(issues: &[Value], key: &str) -> Vec<(String, u64)> {
    let mut t: BTreeMap<String, u64> = BTreeMap::new();
    for i in issues {
        let w = text(i, key);
        if !w.is_empty() {
            *t.entry(w).or_default() += 1;
        }
    }
    t.into_iter().collect()
}

/// The Issues screen.
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let pick = Pick::from_query(query);
    let answers = ask_all(ctx, &[ISSUES, PLAN, ROADMAP]);
    let (issues, plan, roadmap) = (&answers[&ISSUES], &answers[&PLAN], &answers[&ROADMAP]);
    let top = hero(
        "list",
        "cyan",
        "Issues",
        "Every piece of work the plan holds, and where each stands.",
        "Narrow the plan by status, milestone, priority or text; every issue opens its inspector, every milestone its own.",
        vec![
            kpi("Issues", &plan.read("/counts/total")),
            kpi("Ready", &plan.read("/counts/by_status/READY")),
            kpi("Blocked", &plan.read("/counts/by_status/BLOCKED")),
            kpi("Done", &plan.read("/counts/by_status/DONE")),
        ],
    );
    View::new(
        "Issues",
        el("div").class("mj-ui-stack").child(top).child(
            el("div")
                .class("mj-ui-grid")
                .child(wide(browser(issues, roadmap, &pick))),
        ),
    )
}

/// The browser: the status tabs, the filters, the issues the pick admits.
pub fn browser(issues: &Answer, roadmap: &Answer, pick: &Pick) -> El {
    let body = unanswered(issues).unwrap_or_else(|| {
        let all = list(issues, "/issues");
        let shown: Vec<&Value> = all.iter().filter(|i| pick.admits(i)).collect();
        let tab = |label: String, href: String, current: bool| {
            el("a")
                .class("mj-ui-tab")
                .attr("href", href)
                .attr_if("aria-current", current.then_some("page"))
                .text(label)
        };
        let tabs = el("nav")
            .class("mj-ui-tabs")
            .attr("aria-label", "Which issues")
            .child(tab(
                format!("All {}", all.len()),
                pick.href("status", None),
                pick.status.is_none(),
            ))
            .children(tally(&all, "status").into_iter().map(|(s, n)| {
                let current = pick
                    .status
                    .as_deref()
                    .is_some_and(|p| p.eq_ignore_ascii_case(&s));
                tab(
                    format!("{} {n}", s.to_lowercase()),
                    pick.href("status", Some(&s)),
                    current,
                )
            }));
        let milestones: Vec<(String, String)> = list(roadmap, "/milestones")
            .iter()
            .map(|m| (text(m, "id"), text(m, "title")))
            .collect();
        let select =
            |name: &str, label: &str, options: Vec<(String, String)>, picked: &Option<String>| {
                el("label")
                    .class("mj-ui-line")
                    .child(el("span").class("mj-ui-dim").text(format!("{label} ")))
                    .child(
                        el("select")
                            .attr("name", name)
                            .child(el("option").attr("value", "").text("any"))
                            .children(options.into_iter().map(|(value, shown)| {
                                el("option")
                                    .attr_if(
                                        "selected",
                                        (picked.as_deref() == Some(value.as_str()))
                                            .then_some("selected"),
                                    )
                                    .attr("value", value)
                                    .text(shown)
                            })),
                    )
            };
        let form = el("form")
            .class("mj-ui-search")
            .attr("role", "search")
            .attr("method", "get")
            .attr("action", ROUTE)
            .child(
                el("label")
                    .class("mj-ui-sr-only")
                    .attr("for", "mj-ui-issues-q")
                    .text("Search issues"),
            )
            .child(
                el("input")
                    .attr("id", "mj-ui-issues-q")
                    .attr("type", "search")
                    .attr("name", "q")
                    .attr("placeholder", "Id, title or objective")
                    .attr("value", pick.q.clone().unwrap_or_default()),
            )
            .children(pick.status.clone().map(|s| {
                el("input")
                    .attr("type", "hidden")
                    .attr("name", "status")
                    .attr("value", s)
            }))
            .child(select(
                "milestone",
                "Milestone",
                milestones
                    .iter()
                    .map(|(id, t)| (id.clone(), format!("{id} {t}")))
                    .collect(),
                &pick.milestone,
            ))
            .child(select(
                "priority",
                "Priority",
                tally(&all, "priority")
                    .into_iter()
                    .map(|(p, n)| (p.clone(), format!("{p} ({n})")))
                    .collect(),
                &pick.priority,
            ))
            .child(
                el("button")
                    .attr("type", "submit")
                    .class("mj-ui-more")
                    .text("Apply"),
            )
            .when(pick != &Pick::default(), |f| {
                f.child(
                    el("a")
                        .class("mj-ui-more")
                        .attr("href", ROUTE)
                        .text("Clear"),
                )
            });
        let rows = shown
            .iter()
            .map(|i| {
                let id = text(i, "id");
                let milestone = text(i, "milestone");
                let blocked = strings(i, "blocked_by");
                el("tr")
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-row-link")
                                .attr("href", issue_href(&id))
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()))
                                .child(el("span").class("mj-ui-cell-title").text(text(i, "title"))),
                        ),
                    )
                    .child(el("td").child(state(i, "status")))
                    .child(el("td").child(if milestone.is_empty() {
                        el("span").class("mj-ui-dim").text("none")
                    } else {
                        chip(super::milestone::href(&milestone), milestone)
                    }))
                    .child(el("td").class("mj-ui-mono").text(text(i, "priority")))
                    .child(el("td").child(if blocked.is_empty() {
                        el("span").class("mj-ui-dim").text("nothing")
                    } else {
                        el("span")
                            .class("mj-ui-badges")
                            .children(blocked.iter().map(|b| chip(issue_href(b), b.clone())))
                    }))
                    .child(el("td").class("mj-ui-cell-meter").child(meter(
                        i.get("evidence_have").and_then(Value::as_u64),
                        i.get("evidence_need").and_then(Value::as_u64),
                    )))
            })
            .collect::<Vec<_>>();
        el("div")
            .class("mj-ui-stack")
            .child(tabs)
            .child(form)
            .child(table(
                &[
                    "Issue",
                    "Status",
                    "Milestone",
                    "Priority",
                    "Blocked by",
                    "Evidence",
                ],
                rows,
                "No issue of the plan matches.",
            ))
            .child(el("p").class("mj-ui-line").text(format!(
                "{} of {} issues shown",
                shown.len(),
                all.len()
            )))
    });
    panel(
        "Issues",
        "Every issue of the plan, as the plan records it.",
        body,
        &[issues, roadmap],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::source::Exposure;
    use std::time::Duration;

    fn answer(capability: &'static str, value: Value) -> Answer {
        Answer {
            source: Source::of(capability),
            route: None,
            exposure: Exposure::default(),
            value: Ok(value),
            took: Duration::from_millis(1),
        }
    }

    fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn plan() -> (Answer, Answer) {
        let i = |id: &str, status: &str, m: &str, p: &str, blocked: &[&str]| {
            serde_json::json!({ "id": id, "status": status, "milestone": m, "priority": p, "title": format!("issue {id}"),
                "objective": format!("do {id}"), "blocked_by": blocked, "evidence_have": 0, "evidence_need": 1 })
        };
        (
            answer(
                "plan.issues",
                serde_json::json!({ "issues": [
                    i("I0001", "DONE", "M1", "p0", &[]),
                    i("I0002", "READY", "M1", "p1", &[]),
                    i("I0003", "BLOCKED", "M2", "p1", &["I0002"]),
                ]}),
            ),
            answer(
                "plan.roadmap",
                serde_json::json!({ "milestones": [
                { "id": "M1", "title": "first" }, { "id": "M2", "title": "second" } ]}),
            ),
        )
    }

    #[test]
    fn the_tabs_are_the_plans_statuses_and_each_narrows_the_issues() {
        let (issues, roadmap) = plan();
        let all = browser(&issues, &roadmap, &Pick::default()).render();
        for want in [
            "All 3",
            "blocked 1",
            "done 1",
            "ready 1",
            "3 of 3 issues shown",
        ] {
            assert!(all.contains(want), "no {want}: {all}");
        }
        let blocked = browser(
            &issues,
            &roadmap,
            &Pick::from_query(&q(&[("status", "BLOCKED")])),
        )
        .render();
        assert!(blocked.contains("1 of 3 issues shown") && blocked.contains("I0003"));
        assert!(
            blocked.contains("/ui/prs?issue=I0002#issue"),
            "what blocks it is a link"
        );
        assert!(
            blocked.contains("/ui/roadmap?milestone=M2#milestone"),
            "its milestone is a link"
        );
    }

    #[test]
    fn a_milestone_a_priority_and_a_search_narrow_together() {
        let (issues, roadmap) = plan();
        let html = browser(
            &issues,
            &roadmap,
            &Pick::from_query(&q(&[("milestone", "M1"), ("priority", "p1")])),
        )
        .render();
        assert!(
            html.contains("1 of 3 issues shown") && html.contains("I0002"),
            "{html}"
        );
        assert!(
            html.contains("selected=\"selected\" value=\"M1\""),
            "the pick stays picked"
        );
        let found = browser(
            &issues,
            &roadmap,
            &Pick::from_query(&q(&[("q", "do I0003")])),
        )
        .render();
        assert!(found.contains("1 of 3 issues shown"));
        let none = browser(
            &issues,
            &roadmap,
            &Pick::from_query(&q(&[("q", "nothing like it")])),
        )
        .render();
        assert!(none.contains("No issue of the plan matches."));
    }

    #[test]
    fn a_value_that_is_no_word_of_the_plan_is_dropped() {
        let p = Pick::from_query(&q(&[
            ("status", "READY\" onclick=\"x"),
            ("milestone", "M1"),
            ("q", "a b"),
        ]));
        assert_eq!(p.status, None);
        assert_eq!(
            p.href("priority", Some("p0")),
            "/ui/issues?milestone=M1&priority=p0&q=a%20b"
        );
    }
}
