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
use super::source::{ask_all, Answer, Availability, Reading, Source};
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
            ledger_kpi(
                "Closed, underived",
                status,
                kpi(
                    "Closed, underived",
                    &status.read("/closed_without_derivation"),
                ),
            ),
            ledger_kpi(
                "Newest episode",
                status,
                kpi_state("Newest episode", &status.read("/freshness")),
            ),
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
        .children(queue_panels(queue));

    View::new(
        "Knowledge",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// The queue's panels: what it is about and what awaits review, or, when the queue answered
/// that nothing does, one sentence in their place. Two empty tables say the same thing twice
/// and say neither why nor how a record would arrive.
fn queue_panels(queue: &Answer) -> Vec<El> {
    if queue_empty(queue) {
        return vec![wide(panel(
            "Awaiting review",
            "The candidates no reviewer has promoted or rejected yet.",
            empty_queue(),
            &[queue],
        ))];
    }
    vec![
        wide(panel(
            "By subject",
            "The records derived from observations, grouped by what they are about.",
            unanswered(queue).unwrap_or_else(|| subjects(queue)),
            &[queue],
        )),
        wide(panel(
            "Awaiting review",
            "The newest candidates, each leading to its record.",
            unanswered(queue).unwrap_or_else(|| candidates(queue)),
            &[queue],
        )),
    ]
}

/// Whether the queue answered and holds nothing: a total of zero and no candidate. A queue
/// that did not answer is not empty; it is unanswered, and its panels say so.
fn queue_empty(queue: &Answer) -> bool {
    queue
        .ok()
        .is_some_and(|answer| answer.get("total").and_then(Value::as_u64) == Some(0))
        && list(queue, "/candidates").is_empty()
}

/// The sentence an empty queue is told in: that nothing awaits review, and how a record
/// would come to.
fn empty_queue() -> El {
    el("p")
        .class("mj-ui-prose")
        .text("Nothing awaits review: no candidate record is tracked here. A record arrives when a session closes and the deriver writes down what it left behind, or when ")
        .child(el("code").class("mj-ui-code").text("majordomus knowledge observe"))
        .text(" records what was noticed.")
}

/// The judgement of the deriver: when it last wrote, when an episode last closed, why the
/// newest episode is judged as it is, and every finding the status reports.
fn deriver(status: &Answer) -> El {
    let Some(answer) = status.ok() else {
        return nothing("The status did not answer.");
    };
    if !ledger_recorded(answer) {
        return no_ledger();
    }
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

/// What a checkout without a session ledger is told instead of a judgement: the published
/// snapshot and a fresh clone carry none, and a value read from a ledger that is not there
/// would read as "never" or "0" about the repository when it is only about this checkout.
const NO_LEDGER: &str = "not recorded here";

/// Whether the status read any line of the session ledger: a close, a derivation, or a
/// close no derivation names. The status's own `present` also counts candidates, which are
/// tracked records and are read in any checkout; the ledger is local, so it is asked apart.
fn ledger_recorded(answer: &Value) -> bool {
    answer.get("last_closed").is_some_and(|m| !m.is_null())
        || answer.get("last_derived").is_some_and(|m| !m.is_null())
        || answer
            .get("closed_without_derivation")
            .and_then(Value::as_u64)
            .is_some_and(|n| n > 0)
}

/// A tile whose value is read from the session ledger: as drawn when the status read a
/// ledger, and as [`NO_LEDGER`] when it read none, so a snapshot never states a count or a
/// freshness it could not observe.
fn ledger_kpi(label: &str, status: &Answer, drawn: El) -> El {
    match status.ok() {
        Some(answer) if !ledger_recorded(answer) => kpi(
            label,
            &Reading {
                source: STATUS,
                pointer: "/last_closed",
                state: Availability::Available(Value::String(NO_LEDGER.to_string())),
            },
        ),
        _ => drawn,
    }
}

/// The deriver's panel where no session ledger is here: what that means, and where the
/// judgement is made instead. Prose, not facts: there is nothing to tabulate.
fn no_ledger() -> El {
    el("p")
        .class("mj-ui-prose")
        .text("No session ledger is recorded in this checkout, so whether the deriver keeps up is not judged here. The published snapshot and a fresh clone carry none; in a working checkout, ")
        .child(el("code").class("mj-ui-code").text("majordomus knowledge status"))
        .text(" judges it, and this panel shows when the deriver last wrote.")
}

/// The review, as the commands it is made of: each step is the command as code, which breaks
/// anywhere at phone width, and what it does as a sentence at reading size. The command is
/// what a reviewer copies and the sentence what they read; neither is a headline value.
fn review() -> El {
    el("ol")
        .class("mj-ui-prose-list")
        .children(REVIEW.iter().map(|(command, does)| {
            el("li")
                .child(el("code").class("mj-ui-code").text(*command))
                .text(format!(": {does}."))
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

    /// The review is an ordered list of steps: each command is code and what it does is a
    /// sentence beside it, never a fact value drawn as a headline.
    #[test]
    fn the_review_is_steps_of_command_and_sentence() {
        let html = review().render();
        assert!(
            html.starts_with("<ol class=\"mj-ui-prose-list\">"),
            "{html}"
        );
        assert_eq!(html.matches("<li>").count(), REVIEW.len(), "{html}");
        assert!(
            html.contains(
                "<li><code class=\"mj-ui-code\">majordomus knowledge check</code>: every \
                 record is valid, evidenced and resolves what it names.</li>"
            ),
            "{html}"
        );
        assert!(!html.contains("mj-ui-fact"), "{html}");
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

    /// An empty queue is one panel and one sentence, saying how a record arrives; a queue that
    /// holds records, or did not answer, keeps both panels.
    #[test]
    fn an_empty_queue_is_one_sentence_not_two_empty_tables() {
        let render = |panels: Vec<El>| panels.iter().map(El::render).collect::<String>();
        let empty = answer(CANDIDATES, json!({ "total": 0, "candidates": [] }));
        let panels = queue_panels(&empty);
        assert_eq!(panels.len(), 1);
        let html = render(panels);
        assert!(html.contains("Nothing awaits review"), "{html}");
        assert!(
            html.contains("<code class=\"mj-ui-code\">majordomus knowledge observe</code>"),
            "{html}"
        );
        assert!(html.contains("knowledge_base.candidates"), "{html}");
        for gone in ["By subject", "<table", "No candidate awaits review."] {
            assert!(!html.contains(gone), "{gone}: {html}");
        }

        let held = answer(
            CANDIDATES,
            json!({ "total": 1, "candidates": [candidate(1)] }),
        );
        assert_eq!(queue_panels(&held).len(), 2);
        let failed = Answer {
            value: Err("no queue".to_string()),
            ..answer(CANDIDATES, json!({}))
        };
        assert!(!queue_empty(&failed));
        assert_eq!(queue_panels(&failed).len(), 2);
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
        let unjudged = answer(
            STATUS,
            json!({
                "judged": false,
                "stopped_writer": false,
                "last_closed": { "ts": "2026-10-13T10:00:00Z", "episode": "s-1" },
            }),
        );
        assert!(deriver(&unjudged)
            .render()
            .contains("not judged; the findings say why"));
    }

    /// A checkout with no session ledger, as the published snapshot is, states no count and
    /// no "never" about the repository: the ledger tiles and the deriver say it is not
    /// recorded here, and where it is judged. A close no derivation names is a ledger line.
    #[test]
    fn a_checkout_without_a_ledger_states_nothing_it_could_not_observe() {
        let fresh = answer(
            STATUS,
            json!({
                "present": false, "judged": false, "stopped_writer": false,
                "closed_without_derivation": 0, "freshness": "unknown",
                "freshness_reason": "no episode has closed here",
                "findings": ["no episode has closed here; nothing to judge"],
            }),
        );
        let html = deriver(&fresh).render();
        assert!(
            html.contains("No session ledger is recorded in this checkout"),
            "{html}"
        );
        assert!(
            html.contains("majordomus knowledge status</code>"),
            "{html}"
        );
        assert!(!html.contains("never"), "{html}");
        let tile = ledger_kpi("Closed, underived", &fresh, el("span").text("drawn")).render();
        assert!(tile.contains(NO_LEDGER), "{tile}");
        assert!(!tile.contains("drawn"), "{tile}");
        assert!(
            tile.contains("knowledge_base.status"),
            "the tile still cites its source: {tile}"
        );

        let closed = answer(STATUS, json!({ "closed_without_derivation": 2 }));
        assert!(ledger_recorded(closed.ok().expect("an answer")));
        let kept = ledger_kpi("Closed, underived", &closed, el("span").text("drawn")).render();
        assert!(kept.contains("drawn"), "{kept}");
        let failed = Answer {
            value: Err("no status".to_string()),
            ..answer(STATUS, json!({}))
        };
        let shown = ledger_kpi("Closed, underived", &failed, el("span").text("drawn")).render();
        assert!(
            shown.contains("drawn"),
            "an unanswered status keeps its own tile: {shown}"
        );
    }

    /// Each line of the session ledger counts on its own, and a null or a zero is no line:
    /// a close, a derivation and an underived close are each enough to read the ledger.
    #[test]
    fn each_ledger_line_is_read_on_its_own_and_a_null_is_none() {
        let read = |value: Value| ledger_recorded(&value);
        assert!(read(json!({ "last_closed": { "episode": "s-1" } })));
        assert!(read(json!({ "last_derived": { "episode": "s-1" } })));
        assert!(read(json!({ "closed_without_derivation": 1 })));
        assert!(!read(json!({
            "last_closed": null, "last_derived": null, "closed_without_derivation": 0,
        })));
        assert!(!read(json!({})));
    }
}
