//! Knowledge: what the deriver left for review and whether it is still writing. Read from
//! `knowledge_base.status` and `knowledge_base.candidates`.
//!
//! The screen shows the queue and names the commands a reviewer runs. It promotes and rejects
//! nothing: a screen reads and never executes (ADR 0142), and promotion is a person's act.
//! The table is bounded, because the queue is not: what is left out is counted and said.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{
    hero, kpi, kpi_state, list, more, nothing, object, panel, state, table, text, unanswered, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/knowledge";

const STATUS: Source = Source::of("knowledge_base.status");
const CANDIDATES: Source = Source::of("knowledge_base.candidates");

/// How many candidates, and how many subjects, the screen draws. The queue has no bound of
/// its own beyond the policy's cap, and a screen answers within its budget whatever the queue
/// holds; the newest are drawn and the rest are counted.
pub const SHOWN: usize = 50;

/// The commands a reviewer runs, in the order a review takes them. Text, never a control:
/// the screen is a reading.
const REVIEW: &[(&str, &str)] = &[
    (
        "majordomus knowledge candidates",
        "the queue, with the branch each record came from",
    ),
    (
        "majordomus knowledge observe --kind <kind> --subject <subject> \"<statement>\"",
        "what was noticed is written down, so the next derivation has it",
    ),
    (
        "majordomus knowledge record <id>",
        "one record with every reference it names resolved",
    ),
    (
        "majordomus knowledge promote <id> < evidence.md",
        "the record becomes curated knowledge, with the evidence a person wrote",
    ),
    (
        "majordomus knowledge reject <id> --reason \"<why>\"",
        "the record is kept as rejected, with the reason",
    ),
    (
        "majordomus knowledge check",
        "every record is valid, evidenced and resolves what it names",
    ),
];

/// The Knowledge screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[STATUS, CANDIDATES]);
    let status = &answers[&STATUS];
    let queue = &answers[&CANDIDATES];

    let top = hero(
        "sparkles",
        "orange",
        "Knowledge",
        "What the work left behind, awaiting a reviewer.",
        "Every record the deriver wrote from a closed episode and nobody has promoted or rejected yet, what it is about, and whether the deriver is still writing.",
        vec![
            kpi("Awaiting review", &queue.read("/total")),
            kpi("On this branch", &queue.read("/on_this_branch")),
            kpi("Unattributed", &queue.read("/unattributed")),
            kpi("Closed, underived", &status.read("/closed_without_derivation")),
            kpi_state("Newest episode", &status.read("/freshness")),
        ],
    );

    let grid = el("div")
        .class("mj-ui-grid")
        .child(panel(
            "The deriver",
            "Whether a derivation still follows every closed episode, as the status judges it.",
            unanswered(status).unwrap_or_else(|| deriver(status)),
            &[status],
        ))
        .child(panel(
            "Review",
            "What a reviewer runs. This screen reads the queue; it promotes and rejects nothing.",
            review(),
            &[],
        ))
        .child(wide(panel(
            "By subject",
            "The records derived from observations, grouped by what they are about.",
            unanswered(queue).unwrap_or_else(|| subjects(queue)),
            &[queue],
        )))
        .child(wide(panel(
            "Awaiting review",
            "The newest candidates, each leading to its record.",
            unanswered(queue).unwrap_or_else(|| candidates(queue)),
            &[queue],
        )));

    View::new(
        "Knowledge",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// The judgement of the deriver: when it last wrote, when an episode last closed, why the
/// newest episode is judged as it is, and every finding the status reports.
fn deriver(status: &Answer) -> El {
    let Some(answer) = status.ok() else {
        return nothing("The status did not answer.");
    };
    let mark = |key: &str| -> String {
        answer
            .get(key)
            .map(|m| {
                let at = text(m, "ts");
                let episode = text(m, "episode");
                if episode.is_empty() {
                    at
                } else {
                    format!("{at}, episode {episode}")
                }
            })
            .filter(|line| !line.is_empty())
            .unwrap_or_else(|| "never, in this checkout".to_string())
    };
    let judged = answer.get("judged").and_then(Value::as_bool);
    let stopped = answer.get("stopped_writer").and_then(Value::as_bool);
    let verdict = match (judged, stopped) {
        (Some(true), Some(true)) => "episodes close and no derivation followed the newest one",
        (Some(true), _) => "a derivation follows the newest closed episode",
        (Some(false), _) => "not judged; the findings say why",
        (None, _) => "the status carries no judgement",
    };
    let fact = |name: &str, value: String| {
        el("div")
            .class("mj-ui-fact")
            .child(el("dt").text(name))
            .child(el("dd").text(value))
    };
    let facts = el("dl")
        .class("mj-ui-facts")
        .child(fact("Last derived", mark("last_derived")))
        .child(fact("Last closed", mark("last_closed")))
        .child(fact("Judgement", verdict.to_string()))
        .child(fact("Why", text(answer, "freshness_reason")));
    let findings = list(status, "/findings");
    let body = el("div").class("mj-ui-stack").child(facts);
    if findings.is_empty() {
        return body;
    }
    body.child(
        el("ul").class("mj-ui-prose-list").children(
            findings
                .iter()
                .filter_map(Value::as_str)
                .map(|finding| el("li").text(finding)),
        ),
    )
}

/// The review, as the commands it is made of.
fn review() -> El {
    el("dl")
        .class("mj-ui-facts")
        .children(REVIEW.iter().map(|(command, does)| {
            el("div")
                .class("mj-ui-fact")
                .child(el("dt").child(el("code").class("mj-ui-mono").text(*command)))
                .child(el("dd").text(*does))
        }))
}

/// The subjects, the first [`SHOWN`] of them, each with how many records await review, how
/// many were rejected and kept, and how many episodes observed it.
fn subjects(queue: &Answer) -> El {
    let groups = list(queue, "/groups");
    let size = |group: &Value, key: &str| -> String {
        group
            .get(key)
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
            .to_string()
    };
    let rows = groups
        .iter()
        .take(SHOWN)
        .map(|group| {
            el("tr")
                .child(el("td").text(text(group, "about")))
                .child(el("td").class("mj-ui-dim").text(text(group, "route")))
                .child(el("td").text(size(group, "candidates")))
                .child(el("td").text(size(group, "rejected")))
                .child(el("td").text(size(group, "episodes")))
        })
        .collect::<Vec<_>>();
    let shown = rows.len();
    let body = el("div").class("mj-ui-stack").child(table(
        &["About", "Route", "Awaiting", "Rejected", "Episodes"],
        rows,
        "No record derived from an observation awaits review.",
    ));
    match more(shown, groups.len(), None) {
        Some(line) => body.child(line),
        None => body,
    }
}

/// The candidates, the newest [`SHOWN`] of them first. The source orders by id, which begins
/// with the episode, so the last of its list are the newest; the order is the source's and is
/// only read from its other end.
fn candidates(queue: &Answer) -> El {
    let all = list(queue, "/candidates");
    let rows = all
        .iter()
        .rev()
        .take(SHOWN)
        .map(|record| {
            let id = text(record, "id");
            let uri = text(record, "uri");
            let from = record
                .get("branch")
                .and_then(Value::as_str)
                .filter(|branch| !branch.is_empty())
                .map_or_else(|| "not known in this checkout".to_string(), str::to_string);
            el("tr")
                .child(
                    el("td").child(object(&uri, id)).child(
                        el("span")
                            .class("mj-ui-cell-title")
                            .text(text(record, "title")),
                    ),
                )
                .child(el("td").class("mj-ui-dim").text(text(record, "class")))
                .child(el("td").class("mj-ui-dim").text(from))
                .child(
                    el("td")
                        .child(state(record, "freshness"))
                        .child(el("span").class("mj-ui-dim").text(waited(record))),
                )
        })
        .collect::<Vec<_>>();
    let shown = rows.len();
    let body = el("div").class("mj-ui-stack").child(table(
        &["Record", "Class", "From", "Waiting"],
        rows,
        "No candidate awaits review.",
    ));
    match more(shown, all.len(), None) {
        Some(line) => body.child(line),
        None => body,
    }
}

/// How long a record has waited, in the largest whole unit: the source measured it in
/// minutes, and says in `freshness_reason` when it could not.
fn waited(record: &Value) -> String {
    match record.get("age_minutes").and_then(Value::as_i64) {
        Some(minutes) if minutes >= 2 * 24 * 60 => format!(" {} d", minutes / (24 * 60)),
        Some(minutes) if minutes >= 2 * 60 => format!(" {} h", minutes / 60),
        Some(minutes) => format!(" {minutes} min"),
        None => format!(" {}", text(record, "freshness_reason")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;
    use crate::ui::source::Standing;
    use serde_json::json;

    fn answer(source: Source, value: Value) -> Answer {
        Answer {
            source,
            route: None,
            exposure: Default::default(),
            value: Ok(value.into()),
            took: std::time::Duration::ZERO,
            standing: Standing::NOW,
        }
    }

    fn candidate(n: usize) -> Value {
        json!({
            "id": format!("s-20261013{n:06}-aaaa-{n:012x}"),
            "uri": format!("majordomus://knowledge/s-20261013{n:06}-aaaa-{n:012x}"),
            "class": "lesson",
            "title": format!("lesson {n}"),
            "branch": if n % 2 == 0 { json!("master") } else { Value::Null },
            "freshness": "aging",
            "age_minutes": 90 * n,
            "freshness_reason": "measured from the derivation",
        })
    }

    /// The screen names both sources, draws every panel, and its review is text: no form,
    /// no button and no link that would run a command.
    #[test]
    fn the_screen_reads_two_sources_and_runs_nothing() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let view = screen(&ctx);
        assert_eq!(view.title, "Knowledge");
        let html = view.main.render();
        for want in [
            "The deriver",
            "Review",
            "By subject",
            "Awaiting review",
            "knowledge_base.status",
            "knowledge_base.candidates",
            "majordomus knowledge promote",
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

    /// A queue longer than the bound is drawn newest first, to the bound, and says how many
    /// it left out; each row leads to its record and says where the record came from.
    #[test]
    fn a_long_queue_is_bounded_newest_first_and_says_what_it_left_out() {
        let total = SHOWN + 7;
        let queue = answer(
            CANDIDATES,
            json!({ "total": total, "candidates": (0..total).map(candidate).collect::<Vec<_>>() }),
        );
        let html = candidates(&queue).render();
        assert_eq!(html.matches("<tr>").count(), SHOWN + 1, "{html}");
        let newest = format!("lesson {}", total - 1);
        let oldest = "lesson 0<";
        assert!(html.contains(&newest), "the newest is drawn: {html}");
        assert!(!html.contains(oldest), "the oldest is left out: {html}");
        assert!(
            html.contains(&format!(
                "The first {SHOWN} of {total}; 7 more not shown here."
            )),
            "{html}"
        );
        assert!(html.contains("not known in this checkout"), "{html}");
        assert!(
            html.contains(&crate::ui::view::object_href(&format!(
                "majordomus://knowledge/s-20261013{:06}-aaaa-{:012x}",
                total - 1,
                total - 1
            ))),
            "{html}"
        );
    }

    /// An empty queue is an answer, said in words; a wait is told in its largest whole unit.
    #[test]
    fn an_empty_queue_says_so_and_a_wait_is_told_in_whole_units() {
        let empty = answer(CANDIDATES, json!({ "total": 0 }));
        assert!(candidates(&empty)
            .render()
            .contains("No candidate awaits review."));
        assert!(subjects(&empty)
            .render()
            .contains("No record derived from an observation awaits review."));
        assert_eq!(waited(&json!({ "age_minutes": 45 })), " 45 min");
        assert_eq!(waited(&json!({ "age_minutes": 300 })), " 5 h");
        assert_eq!(waited(&json!({ "age_minutes": 3 * 24 * 60 + 5 })), " 3 d");
        assert_eq!(
            waited(&json!({ "freshness_reason": "the record carries no date" })),
            " the record carries no date"
        );
    }

    /// The deriver's panel says the judgement in words and every finding the status carries;
    /// a checkout where it never ran says never, not an empty cell.
    #[test]
    fn the_deriver_is_judged_in_words_and_its_findings_are_listed() {
        let stopped = answer(
            STATUS,
            json!({
                "judged": true,
                "stopped_writer": true,
                "last_closed": { "ts": "2026-10-13T10:00:00Z", "episode": "s-1" },
                "freshness_reason": "the newest closed episode is 400 min old",
                "findings": ["no derivation names s-1"],
            }),
        );
        let html = deriver(&stopped).render();
        assert!(
            html.contains("episodes close and no derivation followed the newest one"),
            "{html}"
        );
        assert!(html.contains("never, in this checkout"), "{html}");
        assert!(html.contains("2026-10-13T10:00:00Z, episode s-1"), "{html}");
        assert!(html.contains("no derivation names s-1"), "{html}");
        let unjudged = answer(STATUS, json!({ "judged": false, "stopped_writer": false }));
        assert!(deriver(&unjudged)
            .render()
            .contains("not judged; the findings say why"));
    }
}
