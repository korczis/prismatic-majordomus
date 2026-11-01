//! The monitor page: everything that happens around this repository, as one live list
//! (ADR 0133). The first page is rendered here from `monitor.stream`, the same answer every
//! other surface renders; `monitor.js` then follows the live channel from its cursor
//! (`/events?follow=blackboard&frames=signals`, ADR 0176) and adds what the server pushes on
//! top, so nothing is fetched twice and nothing is shown that the capability did not say. A
//! page whose channel cannot be opened asks for the page after its cursor every few seconds
//! instead, and says that it polls. Without JavaScript the page is complete, and a refresh
//! shows the newest signals.

use serde_json::json;

use crate::capability::Context;
use crate::http::router::percent_encode;
use crate::monitor::{MonitorPage, MonitorSeverity, MonitorSignal, MonitorSourceHealth};

use super::html::{el, El};
use super::nav::Area;
use super::pages::{ask, failed, param, Page};
use super::view::{card, card_with, code, table, word_badge};

/// How many signals the first page shows.
const FIRST_PAGE: u64 = 100;

/// The sources the first page reads, by `monitor.stream`'s default, named for the live
/// channel so that it continues the same cursor.
const SOURCES: &str = "mesh,executor,github";

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
            let state = health_word(s.state);
            let mut item = el("li")
                .child(el("strong").text(format!("{:?}", s.source).to_lowercase()))
                .child(el("span").text(" "))
                .child(word_badge(state))
                .child(el("span").text(format!(
                    " {} signal(s), {} folded away",
                    s.signals, s.suppressed
                )));
            if let Some(reason) = &s.reason {
                // `mj-identity`: a source's reason may name a key with no break in it
                item = item.child(
                    el("span")
                        .class("mj-muted mj-identity")
                        .text(format!(" — {reason}")),
                );
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
        .text("While this page is open the server pushes what happens and it is added on top.");
    let mut source_url = "/api/v1/monitor?limit=200".to_string();
    let mut live_url = format!("/events?follow=blackboard&frames=signals&sources={SOURCES}");
    // the page's cursor always, an empty one too: it is the start of the stream, so a page
    // rendered before anything happened resumes from there and misses nothing since
    live_url.push_str(&format!(
        "&after={}&epoch={}",
        percent_encode(&page.cursor),
        percent_encode(&crate::http::follow::epoch(ctx))
    ));
    if let Some(reader) = &reader {
        source_url.push_str(&format!("&for={}", percent_encode(reader)));
        live_url.push_str(&format!("&for={}", percent_encode(reader)));
    }
    let frame = el("div")
        .attr("data-mj-monitor", source_url)
        .attr("data-mj-monitor-cursor", &page.cursor)
        .attr("data-mj-monitor-live", live_url)
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
                // the scrolling block every listing uses: a bare <pre> is wider than a phone
                code(
                    "bash",
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

/// The badge word of a source's state.
fn health_word(state: MonitorSourceHealth) -> &'static str {
    match state {
        MonitorSourceHealth::Ok => "ok",
        MonitorSourceHealth::Absent => "absent",
        MonitorSourceHealth::Error => "error",
        MonitorSourceHealth::Skipped => "skipped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::MonitorSource;

    use crate::monitor::testing::{context, cooperation, session};

    fn body(page: &Page) -> String {
        page.main.render()
    }

    /// A reader named in the address sees what names them first, as rows with the action
    /// marked, and the live list asks for the same reader; a reader named nowhere is told so.
    #[test]
    fn a_named_reader_sees_their_signals_first_and_the_live_list_asks_for_them() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = context(&repo);
        let c = cooperation();
        session(&c, "s1", "3d", "the runtime train");
        c.request_review("s1", "1b, read #885".into(), vec![], None, None, None)
            .unwrap();
        crate::mesh::testing::attach(&ctx, c);

        let for_1b = page(&ctx, &[("for".into(), "1b".into())]);
        let html = body(&for_1b);
        assert!(html.contains("For you (1)"), "{html}");
        assert!(html.contains("data-mj-severity=\"action\""), "{html}");
        assert!(html.contains("1b, read #885"), "{html}");
        assert!(
            html.contains("/api/v1/monitor?limit=200&amp;for=1b"),
            "{html}"
        );
        // the live channel continues the page's own cursor, for the same reader
        let live = html
            .split("data-mj-monitor-live=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page names its live channel");
        assert!(
            live.starts_with("/events?follow=blackboard&amp;frames=signals"),
            "{live}"
        );
        assert!(live.contains("&amp;after="), "{live}");
        assert!(live.contains("&amp;epoch="), "{live}");
        assert!(live.ends_with("&amp;for=1b"), "{live}");
        assert!(html.contains("<strong>mesh</strong>"), "{html}");
        assert!(
            !html.contains("Nothing happened in the last six hours"),
            "{html}"
        );

        let for_nobody = page(&ctx, &[("for".into(), "zz".into())]);
        assert!(body(&for_nobody).contains("Nothing in this window names you."));
        let blank = page(&ctx, &[("for".into(), " ".into())]);
        assert!(
            body(&blank).contains("Say who you are"),
            "a blank name is no name"
        );
    }

    #[test]
    fn every_source_state_has_its_word() {
        let words: Vec<&str> = [
            MonitorSourceHealth::Ok,
            MonitorSourceHealth::Absent,
            MonitorSourceHealth::Error,
            MonitorSourceHealth::Skipped,
        ]
        .into_iter()
        .map(health_word)
        .collect();
        assert_eq!(words, vec!["ok", "absent", "error", "skipped"]);
    }

    /// A server whose registry has no monitor capability shows the failure, not a page.
    #[test]
    fn a_stream_that_cannot_be_asked_is_a_failed_page() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let registry = crate::capability::CapabilityRegistry::builder()
            .build()
            .unwrap();
        let ctx = Context::new(
            std::sync::Arc::new(repo.index().unwrap()),
            std::sync::Arc::new(registry),
        );
        let html = body(&page(&ctx, &[]));
        assert!(html.contains("monitor.stream"), "{html}");
    }

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
