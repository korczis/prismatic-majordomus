//! The monitor page: everything that happens around this repository, as one live list
//! (ADR 0133). The first page is rendered here from `monitor.stream`, the same answer every
//! other surface renders; `monitor.js` then asks for the page after its cursor while the
//! page is visible and adds what is new on top, so nothing is fetched twice and nothing is
//! shown that the capability did not say. Without JavaScript the page is complete, and a
//! refresh shows the newest signals.

use serde_json::json;

use crate::capability::Context;
use crate::http::router::percent_encode;
use crate::monitor::{MonitorPage, MonitorSeverity, MonitorSignal, MonitorSourceHealth};

use super::html::{el, El};
use super::nav::Area;
use super::pages::{ask, failed, param, Page};
use super::view::{card, card_with, table, word_badge};

/// How many signals the first page shows.
const FIRST_PAGE: u64 = 100;

/// `/cockpit/monitor[?for=<names>]`.
pub fn page(ctx: &Context, query: &[(String, String)]) -> Page {
    let reader = param(query, "for").filter(|r| !r.trim().is_empty());
    let mut input = json!({ "limit": FIRST_PAGE, "since": 6 * 3600 });
    if let Some(reader) = &reader {
        input["for"] = json!(reader);
    }
    let page: MonitorPage = match ask(ctx, "monitor.stream", input) {
        Ok(p) => p,
        Err(e) => return failed(Area::Mesh, "Monitor", e),
    };

    let mine: Vec<&MonitorSignal> = page.signals.iter().rev().filter(|s| s.for_you).collect();
    let for_you = if reader.is_none() {
        el("p").class("mj-prose").text(
            "Say who you are and what names you is listed here first: add ?for=<your names> to this page's address, e.g. ?for=1b,cd.",
        )
    } else if mine.is_empty() {
        el("p")
            .class("mj-empty")
            .text("Nothing in this window names you.")
    } else {
        rows(&mine)
    };

    let sources = el("ul")
        .class("mj-list")
        .children(page.sources.iter().map(|s| {
            let state = match s.state {
                MonitorSourceHealth::Ok => "ok",
                MonitorSourceHealth::Absent => "absent",
                MonitorSourceHealth::Error => "error",
                MonitorSourceHealth::Skipped => "skipped",
            };
            let mut item = el("li")
                .child(el("strong").text(format!("{:?}", s.source).to_lowercase()))
                .child(el("span").text(" "))
                .child(word_badge(state))
                .child(el("span").text(format!(
                    " {} signal(s), {} folded away",
                    s.signals, s.suppressed
                )));
            if let Some(reason) = &s.reason {
                item = item.child(el("span").class("mj-muted").text(format!(" — {reason}")));
            }
            item
        }));

    let newest_first: Vec<&MonitorSignal> = page.signals.iter().rev().collect();
    let stream = if newest_first.is_empty() {
        el("p")
            .class("mj-empty")
            .attr("data-mj-monitor-empty", "")
            .text("Nothing happened in the last six hours.")
    } else {
        rows(&newest_first)
    };
    let live = el("p")
        .class("mj-muted")
        .attr("data-mj-monitor-note", "")
        .text("While this page is visible it asks for what happened since and adds it on top.");
    let mut source_url = "/api/v1/monitor?limit=200".to_string();
    if let Some(reader) = &reader {
        source_url.push_str(&format!("&for={}", percent_encode(reader)));
    }
    let frame = el("div")
        .attr("data-mj-monitor", source_url)
        .attr("data-mj-monitor-cursor", &page.cursor)
        .child(live)
        .child(stream);

    Page::new(
        Area::Mesh,
        "Monitor",
        el("div")
            .class("mj-grid")
            .child(card(format!("For you ({})", mine.len()), for_you))
            .child(card_with(
                "Everything, newest first",
                el("span").class("mj-muted").text(format!(
                    "{} folded away as noise",
                    page.suppressed
                )),
                frame,
            ))
            .child(card("Sources", sources))
            .child(card(
                "From a terminal",
                el("pre").class("mj-code").text(
                    "majordomus monitor --for 1b            # this page, for one reader\nmajordomus monitor --follow             # keep printing what is new\nmajordomus monitor --follow --match '#885' --until merged --timeout 1h",
                ),
            )),
    )
    .subtitle("Everything that happens around this repository, in one stream: sessions, claims, handovers and reviews on every linked machine, the executor's merges and failures, and GitHub's pull requests and runs.")
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Mesh", Some("/cockpit/mesh")),
        ("Monitor", None),
    ])
    .script("monitor.js")
}

/// Signals as table rows: time (UTC), source, machine, severity, summary.
fn rows(signals: &[&MonitorSignal]) -> El {
    table(
        &["Time (UTC)", "Source", "Machine", "", "What happened"],
        signals.iter().map(|s| row(s)).collect(),
    )
}

fn row(s: &MonitorSignal) -> El {
    let day = s.at % 86_400;
    let time = format!("{:02}:{:02}:{:02}", day / 3600, day % 3600 / 60, day % 60);
    let mark = match s.severity {
        MonitorSeverity::Action => "action",
        MonitorSeverity::Attention => "attention",
        MonitorSeverity::Info => "",
    };
    let summary = match &s.link {
        Some(link) => el("a").attr("href", link).text(&s.summary),
        None => el("span").text(&s.summary),
    };
    el("tr")
        .attr("data-mj-severity", mark)
        .attr("data-mj-for-you", if s.for_you { "yes" } else { "no" })
        .child(el("td").class("mj-mono").text(time))
        .child(el("td").text(format!("{:?}", s.source).to_lowercase()))
        .child(el("td").text(s.machine.clone().unwrap_or_default()))
        .child(el("td").child(if mark.is_empty() {
            el("span")
        } else {
            word_badge(mark)
        }))
        .child(el("td").child(summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::MonitorSource;

    #[test]
    fn a_row_carries_its_time_severity_and_link() {
        let mut s = MonitorSignal::new(
            3_723,
            MonitorSource::Github,
            "pr_merged",
            MonitorSeverity::Attention,
            "#885 merged",
        );
        s.link = Some("https://example.invalid/pull/885".into());
        s.for_you = true;
        let html = row(&s).render();
        assert!(html.contains("01:02:03"), "{html}");
        assert!(html.contains("data-mj-severity=\"attention\""), "{html}");
        assert!(html.contains("data-mj-for-you=\"yes\""), "{html}");
        assert!(
            html.contains("href=\"https://example.invalid/pull/885\""),
            "{html}"
        );
        let plain = MonitorSignal::new(0, MonitorSource::Mesh, "k", MonitorSeverity::Info, "x");
        assert!(row(&plain).render().contains("<span>x</span>"));
    }
}
