//! One milestone in full, for the Roadmap's inspector (`/ui/roadmap?milestone=M004`): what it
//! holds and where each issue stands, what can start now, the longest chain of open work
//! through it, the intents its issues serve, and the milestones it waits for and unblocks.
//!
//! Read from `plan.roadmap` and `plan.issues`, both of which answer in milliseconds, so the
//! inspector is part of the first paint. Every issue is a link to its own inspector on the PR
//! Drain, and every milestone a link to its own here: a reader walks the plan by clicking it.
//!
//! What the milestone is *for* — its problem, the outcome it promises, where things stand and
//! where they should, what is in and out of its scope, and what would prove it — is the
//! record's own prose, asked of `objects.get` and drawn by [`super::prose`]: markup as markup,
//! and every issue or milestone the prose names a link to it.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::cockpit::html::{el, El};

use super::source::Answer;
use super::view::{list, meter, nothing, panel, state, table, text, unanswered};

/// A plan milestone id: letters, digits, `-` and `_`, as the plan's own ids are (`M004`,
/// `development-runtime`), and nothing a link could carry anything else in.
///
/// ```text
/// assert!(is_milestone_id("M004") && is_milestone_id("development-runtime"));
/// assert!(!is_milestone_id("M0\"4") && !is_milestone_id(""));
/// ```
pub fn is_milestone_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The milestone a query names, when it names one.
pub fn picked(query: &[(String, String)]) -> Option<String> {
    query
        .iter()
        .find(|(k, _)| k == "milestone")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| is_milestone_id(v))
}

/// Where a milestone's inspector is.
pub fn href(id: &str) -> String {
    format!("{}?milestone={id}#milestone", super::roadmap::ROUTE)
}

fn issue_href(id: &str) -> String {
    format!("{}?issue={id}#issue", super::prs::ROUTE)
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

fn closed(status: &str) -> bool {
    matches!(status.to_ascii_uppercase().as_str(), "DONE" | "CANCELLED")
}

/// The longest chain of open issues through a milestone, first to last: each issue depends on
/// the one before it, as the plan records `depends_on`, and none of them is done. The order a
/// reader would have to work through, at the least, before the milestone can close. Issues of
/// other milestones are not part of it; a cycle, which the plan's validator refuses, ends a
/// chain rather than looping.
pub fn critical_path(issues: &[Value]) -> Vec<String> {
    let open: BTreeMap<String, Vec<String>> = issues
        .iter()
        .filter(|i| !closed(&text(i, "status")))
        .map(|i| (text(i, "id"), strings(i, "depends_on")))
        .collect();
    fn longest(
        id: &str,
        open: &BTreeMap<String, Vec<String>>,
        memo: &mut BTreeMap<String, Vec<String>>,
        on_stack: &mut BTreeSet<String>,
    ) -> Vec<String> {
        if let Some(found) = memo.get(id) {
            return found.clone();
        }
        if !on_stack.insert(id.to_string()) {
            return Vec::new();
        }
        let mut best: Vec<String> = Vec::new();
        for dep in open.get(id).into_iter().flatten() {
            if open.contains_key(dep) {
                let chain = longest(dep, open, memo, on_stack);
                if chain.len() > best.len() {
                    best = chain;
                }
            }
        }
        on_stack.remove(id);
        best.push(id.to_string());
        memo.insert(id.to_string(), best.clone());
        best
    }
    let mut memo = BTreeMap::new();
    let mut best: Vec<String> = Vec::new();
    for id in open.keys() {
        let chain = longest(id, &open, &mut memo, &mut BTreeSet::new());
        if chain.len() > best.len() {
            best = chain;
        }
    }
    best
}

fn chip(href: String, label: String) -> El {
    el("a")
        .class("mj-ui-chip mj-ui-chip--link")
        .attr("href", href)
        .child(el("code").class("mj-ui-chip-value").text(label))
}

fn chips(ids: &[String], to: fn(&str) -> String) -> El {
    if ids.is_empty() {
        return el("span").class("mj-ui-dim").text("none");
    }
    el("span")
        .class("mj-ui-badges")
        .children(ids.iter().map(|id| chip(to(id), id.clone())))
}

/// The inspector of milestone `id`.
/// The prose sections of a milestone's record, in the order a reader asks: why, what for,
/// from where to where, within what bounds, and proven how. A section the record does not
/// carry is not drawn.
const NARRATIVE: &[(&str, &str)] = &[
    ("problem", "The problem"),
    ("outcome", "The outcome"),
    ("current_state", "Where it stands"),
    ("desired_state", "Where it should stand"),
    ("scope", "In scope"),
    ("non_scope", "Out of scope"),
    ("acceptance_criteria", "Proven when"),
];

/// The record's narrative as headed prose: a string as a paragraph, a list as a list.
fn narrative(record: &Value, known: &super::prose::Known) -> Vec<El> {
    let mut out = Vec::new();
    for (key, heading) in NARRATIVE {
        let drawn = match record.get(*key) {
            Some(Value::String(prose)) if !prose.trim().is_empty() => {
                super::prose::paragraph(prose, known)
            }
            Some(Value::Array(_)) if !strings(record, key).is_empty() => {
                super::prose::bullets(&strings(record, key), known)
            }
            _ => continue,
        };
        out.push(el("h4").class("mj-ui-panel-title").text(*heading));
        out.push(drawn);
    }
    out
}

/// The milestone inspector with the record's own prose: `record` is `objects.get` for the
/// milestone, when the caller asked it.
pub fn inspector_with(roadmap: &Answer, issues: &Answer, record: &Answer, id: &str) -> El {
    let known = super::prose::Known {
        issues: list(issues, "/issues")
            .iter()
            .map(|i| text(i, "id"))
            .collect(),
        milestones: list(roadmap, "/milestones")
            .iter()
            .map(|m| text(m, "id"))
            .collect(),
    };
    let told = record
        .ok()
        .and_then(|r| r.get("metadata"))
        .map(|metadata| narrative(metadata, &known))
        .unwrap_or_default();
    let base = inspector(roadmap, issues, id);
    match told.is_empty() {
        true => base,
        false => panel(
            "What it is for",
            "The milestone's own record: why it exists, what it promises, and what would prove it.",
            told.into_iter()
                .fold(el("div").class("mj-ui-stack"), El::child),
            &[record],
        )
        .attr("id", "milestone-record")
        .child(base),
    }
}

pub fn inspector(roadmap: &Answer, issues: &Answer, id: &str) -> El {
    let body = unanswered(roadmap)
        .or_else(|| unanswered(issues))
        .unwrap_or_else(|| {
            let Some(m) = list(roadmap, "/milestones")
                .into_iter()
                .find(|m| text(m, "id") == id)
            else {
                return nothing(&format!("{id} is not a milestone of the plan."));
            };
            let held: BTreeSet<String> = strings(&m, "issues").into_iter().collect();
            let mine: Vec<Value> = list(issues, "/issues")
                .into_iter()
                .filter(|i| held.contains(&text(i, "id")) || text(i, "milestone") == id)
                .collect();
            let ready: Vec<String> = mine
                .iter()
                .filter(|i| text(i, "status").eq_ignore_ascii_case("READY"))
                .map(|i| text(i, "id"))
                .collect();
            let path = critical_path(&mine);
            let serves: Vec<String> = mine
                .iter()
                .flat_map(|i| {
                    strings(i, "serves")
                        .into_iter()
                        .map(|s| super::intent::intent_of(&s).to_string())
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let rows = mine
                .iter()
                .map(|i| {
                    let iid = text(i, "id");
                    el("tr")
                        .child(
                            el("td").child(
                                el("a")
                                    .class("mj-ui-row-link")
                                    .attr("href", issue_href(&iid))
                                    .child(
                                        el("span").class("mj-ui-mono mj-ui-dim").text(iid.clone()),
                                    )
                                    .child(
                                        el("span").class("mj-ui-cell-title").text(text(i, "title")),
                                    ),
                            ),
                        )
                        .child(el("td").child(state(i, "status")))
                        .child(
                            el("td")
                                .class("mj-ui-mono")
                                .text(super::view::value(i, "priority")),
                        )
                        .child(
                            el("td").class("mj-ui-mono").text(
                                i.get("wave")
                                    .filter(|w| !w.is_null())
                                    .map_or_else(|| "none".into(), Value::to_string),
                            ),
                        )
                        .child(el("td").child(chips(&strings(i, "blocked_by"), issue_href)))
                })
                .collect::<Vec<_>>();
            let fact = |label: &str, value: El| {
                el("div")
                    .class("mj-ui-fact")
                    .child(el("dt").text(label))
                    .child(el("dd").child(value))
            };
            let count = |s: &str| {
                m.pointer(&format!("/counts/by_status/{s}"))
                    .and_then(Value::as_u64)
            };
            el("section")
                .class("mj-ui-detail")
                .attr("id", "milestone")
                .attr("aria-label", format!("Milestone {id}"))
                .child(
                    el("div")
                        .class("mj-ui-detail-head")
                        .child(el("p").class("mj-ui-mono mj-ui-dim").text(format!(
                            "{id} · order {} · {}",
                            m.get("order").map(Value::to_string).unwrap_or_else(|| "?".into()),
                            text(&m, "priority")
                        )))
                        .child(el("h3").class("mj-ui-detail-title").text(text(&m, "title")))
                        .child(
                            el("p")
                                .class("mj-ui-badges")
                                .child(state(&m, "status"))
                                .child(
                                    el("a")
                                        .class("mj-ui-more")
                                        .attr(
                                            "href",
                                            format!("{}#milestone-{id}", super::roadmap::ROUTE),
                                        )
                                        .text("Close"),
                                ),
                        ),
                )
                .child(
                    el("dl")
                        .class("mj-ui-facts")
                        .child(fact(
                            "Done",
                            meter(
                                count("DONE"),
                                m.pointer("/counts/total").and_then(Value::as_u64),
                            ),
                        ))
                        .child(fact("Ready now", chips(&ready, issue_href)))
                        .child(fact(
                            "Blocked",
                            el("span").class("mj-ui-mono").text(
                                count("BLOCKED").map_or_else(|| "none".into(), |n| n.to_string()),
                            ),
                        ))
                        .child(fact(
                            "Waits for",
                            chips(&strings(&m, "depends_on"), href_owned),
                        ))
                        .child(fact(
                            "Blocked by",
                            chips(&strings(&m, "blocked_by"), href_owned),
                        ))
                        .child(fact(
                            "Unblocks",
                            chips(&strings(&m, "dependents"), href_owned),
                        ))
                        .child(fact("Serves", chips(&serves, super::intent::href))),
                )
                .child(el("h4").class("mj-ui-panel-title").text("Critical path"))
                .child(if path.is_empty() {
                    nothing("No issue of it is open.")
                } else {
                    el("ol")
                        .class("mj-ui-badges")
                        .attr("aria-label", "The longest chain of open issues")
                        .children(
                            path.iter()
                                .map(|i| el("li").child(chip(issue_href(i), i.clone()))),
                        )
                })
                .child(el("h4").class("mj-ui-panel-title").text("Issues"))
                .child(table(
                    &["Issue", "Status", "Priority", "Wave", "Blocked by"],
                    rows,
                    "The milestone holds no issue.",
                ))
        });
    panel(
        "Milestone",
        "One milestone of the plan: what it holds, what can start, and what it waits for.",
        body,
        &[roadmap, issues],
    )
}

fn href_owned(id: &str) -> String {
    href(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::source::{Exposure, Source};
    use std::time::Duration;

    fn answer(capability: &'static str, value: Value) -> Answer {
        Answer {
            source: Source::of(capability),
            route: None,
            exposure: Exposure::default(),
            value: Ok(value),
            took: Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        }
    }

    fn issue(id: &str, status: &str, deps: &[&str]) -> Value {
        serde_json::json!({ "id": id, "milestone": "M004", "status": status, "title": format!("issue {id}"),
            "priority": "p1", "wave": 1, "depends_on": deps, "blocked_by": [], "serves": ["INT-1"] })
    }

    fn plan() -> (Answer, Answer) {
        let roadmap = answer(
            "plan.roadmap",
            serde_json::json!({ "milestones": [
                { "id": "M003", "status": "DONE", "order": 3, "priority": "p0", "title": "before",
                  "depends_on": [], "blocked_by": [], "dependents": ["M004"], "issues": [] },
                { "id": "M004", "status": "ACTIVE", "order": 4, "priority": "p0", "title": "the one",
                  "depends_on": ["M003"], "blocked_by": [], "dependents": [],
                  "counts": { "total": 4, "by_status": { "DONE": 1, "READY": 1, "BLOCKED": 2 } },
                  "issues": ["I0001", "I0002", "I0003", "I0004"] }
            ]}),
        );
        let issues = answer(
            "plan.issues",
            serde_json::json!({ "issues": [
                issue("I0001", "DONE", &[]),
                issue("I0002", "READY", &["I0001"]),
                issue("I0003", "BLOCKED", &["I0002"]),
                issue("I0004", "BLOCKED", &["I0003", "I0002"]),
            ]}),
        );
        (roadmap, issues)
    }

    /// The record's own prose is drawn above the plan's arithmetic: every section it carries,
    /// markup as markup, and an issue or milestone it names as a link to it. A section it does
    /// not carry is not drawn, and a record nobody could read leaves the inspector as it was.
    #[test]
    fn the_records_prose_is_drawn_with_its_markup_and_its_links() {
        let (roadmap, issues) = plan();
        let record = answer(
            "objects.get",
            serde_json::json!({ "metadata": {
                "id": "M004",
                "problem": "The shell tool is slow: `doctor` takes ten seconds, see I0002.",
                "outcome": "Every command is **benchmarked**, after M003.",
                "current_state": "   ",
                "scope": ["lib", "bin/majordomus"],
                "non_scope": [],
                "acceptance_criteria": ["A regression is refused, as I0003 proves"]
            }}),
        );
        let html = inspector_with(&roadmap, &issues, &record, "M004").render();
        for expected in [
            "The problem",
            "<code class=\"mj-ui-code\">doctor</code>",
            "?issue=I0002#issue\">I0002</a>",
            "The outcome",
            "<strong>benchmarked</strong>",
            "?milestone=M003#milestone\">M003</a>",
            "In scope",
            "<li>bin/majordomus</li>",
            "Proven when",
            "?issue=I0003#issue\">I0003</a>",
            "id=\"milestone-record\"",
            // and the plan's own inspector is still there, below it
            "Critical path",
        ] {
            assert!(html.contains(expected), "{expected} is missing from {html}");
        }
        for absent in ["Where it stands", "Out of scope", "Where it should stand"] {
            assert!(
                !html.contains(absent),
                "{absent} was drawn from nothing: {html}"
            );
        }

        let unread = Answer {
            value: Err("no such object".into()),
            ..answer("objects.get", Value::Null)
        };
        let plain = inspector_with(&roadmap, &issues, &unread, "M004").render();
        assert_eq!(plain, inspector(&roadmap, &issues, "M004").render());
        let bare = answer(
            "objects.get",
            serde_json::json!({ "metadata": { "id": "M004" } }),
        );
        assert_eq!(
            inspector_with(&roadmap, &issues, &bare, "M004").render(),
            inspector(&roadmap, &issues, "M004").render()
        );
    }

    /// A milestone that gives no order, counts nothing blocked and holds no open issue says
    /// each of those rather than inventing a number or drawing an empty path.
    #[test]
    fn a_milestone_with_nothing_open_says_so() {
        let roadmap = answer(
            "plan.roadmap",
            serde_json::json!({ "milestones": [
                { "id": "M009", "status": "DONE", "priority": "p2", "title": "done already",
                  "depends_on": [], "blocked_by": [], "dependents": [],
                  "counts": { "total": 1, "by_status": { "DONE": 1 } }, "issues": ["I0009"] }
            ]}),
        );
        let issues = answer(
            "plan.issues",
            serde_json::json!({ "issues": [issue("I0009", "DONE", &[])] }),
        );
        let html = inspector(&roadmap, &issues, "M009").render();
        for want in ["M009 · order ? · p2", "No issue of it is open."] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(html.contains(">none<"), "no blocked count is none: {html}");
    }

    #[test]
    fn the_critical_path_is_the_longest_chain_of_open_issues() {
        let (_, issues) = plan();
        let all = list(&issues, "/issues");
        assert_eq!(critical_path(&all), ["I0002", "I0003", "I0004"]);
        let cyclic = vec![issue("I1", "READY", &["I2"]), issue("I2", "READY", &["I1"])];
        assert!(critical_path(&cyclic).len() <= 2, "a cycle ends a chain");
    }

    #[test]
    fn the_inspector_links_every_issue_and_milestone_it_names() {
        let (roadmap, issues) = plan();
        let html = inspector(&roadmap, &issues, "M004").render();
        assert!(html.contains("id=\"milestone\""));
        for want in [
            "/ui/prs?issue=I0002#issue",
            "/ui/prs?issue=I0004#issue",
            "/ui/roadmap?milestone=M003#milestone",
            "INT-1",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(
            html.contains("1 of 4"),
            "the meter says how many are done: {html}"
        );
    }

    #[test]
    fn an_unknown_milestone_says_so_and_a_hostile_one_is_never_picked() {
        let (roadmap, issues) = plan();
        assert!(inspector(&roadmap, &issues, "M999")
            .render()
            .contains("M999 is not a milestone of the plan."));
        let q = |v: &str| vec![("milestone".to_string(), v.to_string())];
        assert_eq!(picked(&q("M004")), Some("M004".into()));
        assert_eq!(
            picked(&q("development-runtime")),
            Some("development-runtime".into()),
            "a milestone named by a slug is a milestone"
        );
        assert_eq!(picked(&q("M0\" onclick=\"x")), None);
        assert_eq!(picked(&q("")), None);
    }

    /// An issue that names the milestone is one of its own even when the milestone's list
    /// leaves it out; an issue of another milestone is not.
    #[test]
    fn an_issue_naming_the_milestone_is_its_own_though_the_list_omits_it() {
        let (roadmap, _) = plan();
        let mut stray = issue("I0099", "READY", &[]);
        stray["milestone"] = serde_json::json!("M003");
        let issues = answer(
            "plan.issues",
            serde_json::json!({ "issues": [
                issue("I0001", "DONE", &[]),
                issue("I0005", "READY", &[]),
                stray,
            ]}),
        );
        let html = inspector(&roadmap, &issues, "M004").render();
        assert!(html.contains("/ui/prs?issue=I0005#issue"), "{html}");
        assert!(!html.contains("I0099"), "{html}");
    }

    /// An issue that gives no wave or priority says so in its row rather than leaving it blank.
    #[test]
    fn an_issue_without_a_wave_says_none() {
        let roadmap = answer(
            "plan.roadmap",
            serde_json::json!({ "milestones": [
                { "id": "M010", "status": "ACTIVE", "title": "waveless", "issues": ["I0100"] }
            ]}),
        );
        let issues = answer(
            "plan.issues",
            serde_json::json!({ "issues": [
                { "id": "I0100", "milestone": "M010", "status": "READY", "title": "no wave", "wave": null }
            ]}),
        );
        let html = inspector(&roadmap, &issues, "M010").render();
        assert!(
            html.contains("<td class=\"mj-ui-mono\">none</td><td class=\"mj-ui-mono\">none</td>"),
            "{html}"
        );
    }
}
