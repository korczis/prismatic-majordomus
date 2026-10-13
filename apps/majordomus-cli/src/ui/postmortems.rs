//! Post-mortems: what turned the trunk's CI red, and why. Read from `postmortems.list`.
//!
//! One row per record: how far it has come, the class of its cause, the runs it answers,
//! where the cause lived and what else of the layer it concerns. A reference a record
//! carries is a link to the screen that inspects that kind, or to the object's technical
//! view. The screen reads the index's records and nothing else: which of the trunk's failed
//! runs still owe a record is a question to the forge, and a page never asks it.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::postmortem::{path_of, DIRECTORY};

use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{
    code_link, counts, donut, hero, kpi, kpi_count, list, none, object_if, object_or, panel, state,
    table, text, unanswered, value, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/post-mortems";

const RECORDS: Source = Source::of("postmortems.list");

/// One reference of a record, `<kind>:<identity>`, as a link to where that kind is
/// inspected: a rule, an issue, a milestone, an intent and a claim have a screen; any other
/// object leads to its technical view when the index holds it; a path is a path.
fn reference(ctx: &Context, reference: &str) -> El {
    if let Some(path) = path_of(reference) {
        return el("span").class("mj-ui-mono").text(path);
    }
    let Some((kind, identity)) = reference.split_once(':') else {
        return el("span").class("mj-ui-mono").text(reference);
    };
    match kind {
        "rule" => code_link(super::rule::href(identity), identity),
        "issue" => code_link(super::issues::issue_href(identity), identity),
        "milestone" => code_link(super::milestone::href(identity), identity),
        "intent" => code_link(super::intent::href(identity), identity),
        "claim" => code_link(super::claim::href(identity), identity),
        _ => object_if(ctx, &crate::model::uri_for(kind, identity), reference),
    }
}

/// The references under `key` of one record, each as its link, or `none`.
fn references(ctx: &Context, record: &Value, key: &str) -> El {
    let all: Vec<&str> = record
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if all.is_empty() {
        return none();
    }
    el("div").children(all.into_iter().map(|r| el("div").child(reference(ctx, r))))
}

/// The runs one record answers, each as the workflow and the forge's identifier.
fn runs(record: &Value) -> El {
    let all = record
        .get("runs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if all.is_empty() {
        return none();
    }
    el("div").children(all.iter().map(|run| {
        el("div").class("mj-ui-mono").text(format!(
            "{} {}",
            value(run, "workflow"),
            value(run, "id")
        ))
    }))
}

/// The Post-mortems screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[RECORDS]);
    let records = &answers[&RECORDS];

    let top = hero(
        "alert",
        "rose",
        "Post-mortems",
        "What turned the trunk's CI red, and why.",
        "One record per cause: the runs it failed, the class of the cause, where it lived, what fixed it and what now stops it from returning. A new failure is searched for the signals these records carry.",
        vec![
            kpi("Records", &records.read("/count")),
            kpi_count("Statuses", &records.read("/by_status")),
            kpi_count("Cause classes", &records.read("/by_cause")),
            kpi_count("Unreadable", &records.read("/unreadable")),
        ],
    );

    let rows_body = unanswered(records).unwrap_or_else(|| {
        let rows = list(records, "/records")
            .iter()
            .map(|r| {
                el("tr")
                    .child(el("td").child(state(r, "status")))
                    .child(
                        el("td")
                            .class("mj-ui-mono")
                            .text(text(&r["cause"], "class")),
                    )
                    .child(
                        el("td")
                            .child(
                                el("div")
                                    .class("mj-ui-cell-title")
                                    .child(object_or(&text(r, "uri"), &text(r, "title"))),
                            )
                            .child(el("div").class("mj-ui-dim").text(format!(
                                "{} · {}",
                                value(r, "date"),
                                value(r, "id")
                            )))
                            .child(
                                el("div")
                                    .class("mj-ui-dim")
                                    .text(text(&r["cause"], "summary")),
                            ),
                    )
                    .child(el("td").child(runs(r)))
                    .child(el("td").child(references(ctx, r, "components")))
                    .child(el("td").child(references(ctx, r, "related")))
            })
            .collect::<Vec<_>>();
        let unreadable = list(records, "/unreadable");
        el("div")
            .child(table(
                &["Status", "Cause", "Record", "Runs", "Components", "Related"],
                rows,
                &format!("No post-mortem is recorded under {DIRECTORY}."),
            ))
            .children(unreadable.iter().map(|u| {
                el("p").class("mj-ui-dim").text(format!(
                    "{} is indexed as a post-mortem and does not read as one: {}",
                    text(u, "path"),
                    text(u, "reason")
                ))
            }))
    });

    let by_cause =
        unanswered(records).unwrap_or_else(|| donut("causes", &counts(records, "/by_cause")));
    let by_status =
        unanswered(records).unwrap_or_else(|| donut("records", &counts(records, "/by_status")));

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(panel(
            "Records",
            "Every post-mortem, newest first, with what it answers and what it names.",
            rows_body,
            &[records],
        )))
        .child(panel(
            "By cause",
            "How many records carry each class of cause.",
            by_cause,
            &[records],
        ))
        .child(panel(
            "By status",
            "How many records are drafts, analysed, and resolved.",
            by_status,
            &[records],
        ));

    View::new(
        "Post-mortems",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::builtin::postmortems::tests::recorded;
    use crate::synthetic::SyntheticRepository;

    /// Every record is a row: its status, its cause, its title leading to the object's
    /// technical view, the runs it answers, and its references as links.
    #[test]
    fn every_record_is_a_row_with_its_runs_and_its_references() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let view = screen(&ctx);
        assert_eq!(view.status, 200);
        let html = view.main.render();
        assert!(html.contains("Two writers lost the lease"), "{html}");
        assert!(
            html.contains(
                "href=\"/cockpit/object?uri=majordomus%3A%2F%2Fpost-mortem%2Fpm-20261013-lease\""
            ),
            "the title leads to the record: {html}"
        );
        assert!(html.contains("Two writers, one file."), "{html}");
        assert!(html.contains(">validate 100<"), "{html}");
        assert!(html.contains(">validate 200<"), "{html}");
        // a path is a path, a rule leads to its inspector
        assert!(html.contains(">docs/DOC_0.md<"), "{html}");
        for rule in ["project.rule-0@1", "project.rule-1@1"] {
            assert!(
                html.contains(&format!("href=\"/ui/governance?rule={rule}#rule\"")),
                "{rule} is not a link: {html}"
            );
        }
        assert!(
            html.contains("resolved") && html.contains("draft"),
            "{html}"
        );
        assert!(html.contains("undetermined"), "{html}");
        assert!(!html.contains("could not answer"), "{html}");
        // the draft names no component and nothing related, and says none
        assert!(html.contains("mj-ui-dim\">none<"), "{html}");
    }

    #[test]
    fn a_repository_with_no_record_says_where_one_would_live() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(
            html.contains("No post-mortem is recorded under .ai/repo/post-mortems/."),
            "{html}"
        );
        assert!(html.contains("mj-ui-hero") && html.contains("mj-ui-source"));
    }

    #[test]
    fn a_reference_leads_to_the_screen_that_inspects_its_kind() {
        let repo = recorded();
        let ctx = repo.context().expect("a context");
        let link = |r: &str| reference(&ctx, r).render();
        assert!(link("issue:I2647").contains("href=\"/ui/prs?issue=I2647#issue\""));
        assert!(link("milestone:M001").contains("?milestone=M001#milestone"));
        assert!(link("intent:an-intent").contains("?intent=an-intent#intent"));
        assert!(link("claim:a-claim").contains("?claim=a-claim#claim"));
        assert!(link("test:test/cases/1_x.sh").contains(">test/cases/1_x.sh<"));
        // any other object of the index leads to its technical view; one it does not hold
        // is its reference as text, never a link to a page that answers 404
        let held = link("document:docs/DOC_0.md");
        assert!(held.contains("/cockpit/object?uri="), "{held}");
        let absent = link("adr:9999");
        assert!(
            !absent.contains("href") && absent.contains("adr:9999"),
            "{absent}"
        );
        assert!(!link("no prefix").contains("href"));
        let record = serde_json::json!({ "runs": [], "related": "not a list" });
        assert!(runs(&record).render().contains("none"));
        assert!(references(&ctx, &record, "related")
            .render()
            .contains("none"));
    }
}
