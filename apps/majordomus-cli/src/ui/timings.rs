//! How long CI takes, for the CI/CD screen: `integration.timings`, measured from the recorded
//! forge observation. Every check with its runs, failures and median, 90th percentile and
//! longest duration, each median drawn as a bar against the slowest; and the open heads that
//! waited longest for CI, each a link to its pull request on the PR Drain.
//!
//! Nothing is estimated: a run without a start or an end is counted beside the figures and
//! never drawn as zero, and a checkout that recorded no observation says so.

use serde_json::Value;

use crate::cockpit::html::{el, El};

use super::source::{Answer, Source};
use super::view::{kpi, list, nothing, panel, strip, table, text, unanswered};

/// The source.
pub const TIMINGS: Source = Source::of("integration.timings");

/// How many of the longest-waiting heads are listed.
const HEADS: usize = 10;

/// Seconds as a reader says them: `42 s`, `7 min 05 s`, `1 h 02 min`.
///
/// ```text
/// assert_eq!(duration(42), "42 s");
/// assert_eq!(duration(425), "7 min 05 s");
/// assert_eq!(duration(3720), "1 h 02 min");
/// ```
pub fn duration(s: u64) -> String {
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min {:02} s", s / 60, s % 60),
        _ => format!("{} h {:02} min", s / 3600, (s % 3600) / 60),
    }
}

fn num(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(Value::as_u64)
}

/// A duration the answer gave, or `unknown` when it gave none: never a zero it did not say.
fn seconds(s: Option<u64>) -> String {
    s.map_or_else(|| "unknown".into(), duration)
}

/// A count the answer gave, or `unknown`.
fn count(n: Option<u64>) -> String {
    n.map_or_else(|| "unknown".into(), |n| n.to_string())
}

/// A bar of `value` against `max`, drawn on the server in the given tone, with its value as
/// text beside it so the bar is never the only way to read it.
fn bar(value: Option<u64>, max: u64, tone: &str) -> El {
    // ui-integrity: a width, not a count shown: an unknown value draws an empty bar, and the
    // text beside every bar says unknown; anything measured is at least visible
    let width = value
        .and_then(|v| (v * 100).checked_div(max))
        .map_or(0, |w| w.max(1));
    el("span")
        .class(format!("mj-tone--{tone}"))
        .attr("aria-hidden", "true")
        .raw(format!(
            r#"<svg viewBox="0 0 100 6" width="120" height="8" preserveAspectRatio="none" focusable="false"><rect x="0" y="0" width="100" height="6" rx="3" fill="currentColor" opacity="0.15"/><rect x="0" y="0" width="{width}" height="6" rx="3" fill="currentColor"/></svg>"#
        ))
}

/// The panel.
pub fn panel_of(timings: &Answer) -> El {
    let body = unanswered(timings).unwrap_or_else(|| {
        let v = timings.ok();
        if v.and_then(|v| v.get("observed")) != Some(&Value::Bool(true)) {
            return nothing(&format!(
                "No forge observation is recorded here, so no run is measured: {}",
                v.and_then(|v| v.get("reason"))
                    .and_then(Value::as_str)
                    .unwrap_or("no reason was given")
            ));
        }
        let checks = list(timings, "/timings/checks");
        if checks.is_empty() {
            return nothing(
                "No check run on an open pull request has both a start and an end in the observation; run majordomus prs refresh to record them.",
            );
        }
        // the slowest median the answer gave; a check without one draws no bar
        let slowest = checks
            .iter()
            .filter_map(|c| num(c, "median_seconds"))
            .max()
            .unwrap_or(1);
        let check_rows = checks
            .iter()
            .map(|c| {
                let median = num(c, "median_seconds");
                let failed = num(c, "failed");
                let failing = failed.is_some_and(|f| f > 0);
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(c, "name")))
                    .child(
                        el("td")
                            .child(bar(median, slowest, if failing { "rose" } else { "blue" }))
                            .child(el("span").class("mj-ui-mono").text(format!(" {}", seconds(median)))),
                    )
                    .child(el("td").class("mj-ui-mono").text(seconds(num(c, "p90_seconds"))))
                    .child(el("td").class("mj-ui-mono").text(seconds(num(c, "slowest_seconds"))))
                    .child(el("td").class("mj-ui-mono").text(format!(
                        "{} · {} failed",
                        count(num(c, "runs")),
                        count(failed)
                    )))
            })
            .collect::<Vec<_>>();
        let heads = list(timings, "/timings/heads");
        let longest = heads
            .iter()
            .filter_map(|h| num(h, "wall_seconds"))
            .max()
            .unwrap_or(1);
        let head_rows = heads
            .iter()
            .take(HEADS)
            .map(|h| {
                el("tr")
                    .child(el("td").child(match num(h, "number") {
                        Some(n) => el("a")
                            .attr("href", format!("{}?pr={n}#pr", super::prs::ROUTE))
                            .text(format!("#{n}")),
                        None => el("span").class("mj-ui-dim").text("unnumbered"),
                    }))
                    .child(
                        el("td")
                            .child(bar(
                                num(h, "wall_seconds"),
                                longest,
                                if h.get("failed") == Some(&Value::Bool(true)) { "rose" } else { "violet" },
                            ))
                            .child(el("span").class("mj-ui-mono").text(format!(" {}", seconds(num(h, "wall_seconds"))))),
                    )
                    .child(el("td").class("mj-ui-mono mj-ui-dim").text(format!(
                        "{} · {}",
                        text(h, "slowest_check"),
                        seconds(num(h, "slowest_seconds"))
                    )))
            })
            .collect::<Vec<_>>();
        el("div")
            .class("mj-ui-stack")
            .child(strip(vec![
                kpi("Runs measured", &timings.read("/timings/measured_runs")),
                kpi("Not measured", &timings.read("/timings/unmeasured_runs")),
                kpi("Observed", &timings.read("/timings/observed_at")),
            ]))
            .child(table(
                &["Check", "Median", "90th percentile", "Longest", "Runs"],
                check_rows,
                "No check was measured.",
            ))
            .child(el("h3").class("mj-ui-panel-title").text("Longest waits"))
            .child(table(
                &["Pull request", "Wall time", "Slowest check"],
                head_rows,
                "No open head has a measured check.",
            ))
    });
    panel(
        "Build timings",
        "How long each check takes on the open pull requests, measured from when each run started to when it completed.",
        body,
        &[timings],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::source::Exposure;
    use std::time::Duration;

    fn answer(value: Value) -> Answer {
        Answer {
            source: TIMINGS,
            route: Some("GET /api/v1/pull-requests/timings".into()),
            exposure: Exposure::default(),
            value: Ok(value),
            took: Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        }
    }

    /// A check or a head the answer gives without its numbers is drawn as unknown and
    /// unnumbered, never as a zero it did not say.
    #[test]
    fn what_the_answer_leaves_out_is_unknown_never_zero() {
        let html = panel_of(&answer(serde_json::json!({ "observed": true, "timings": {
            "observed_at": "2026-10-11T12:00:00Z",
            "checks": [{ "name": "site" }],
            "heads": [{ "head_sha": "a", "slowest_check": "site", "failed": false }]
        }})))
        .render();
        assert!(html.contains("unknown"), "{html}");
        assert!(html.contains("unnumbered"), "{html}");
        assert!(!html.contains(">0 s<"), "{html}");
    }

    #[test]
    fn a_duration_reads_as_a_person_says_it() {
        assert_eq!(duration(42), "42 s");
        assert_eq!(duration(425), "7 min 05 s");
        assert_eq!(duration(3720), "1 h 02 min");
    }

    #[test]
    fn the_checks_and_the_longest_waits_are_drawn_with_their_numbers() {
        let html = panel_of(&answer(serde_json::json!({ "observed": true, "timings": {
            "observed_at": "2026-10-11T12:00:00Z", "measured_runs": 3, "unmeasured_runs": 1,
            "checks": [
                { "name": "site", "runs": 2, "passed": 2, "failed": 0, "median_seconds": 2400, "p90_seconds": 2460, "slowest_seconds": 2460 },
                { "name": "rust", "runs": 1, "passed": 0, "failed": 1, "median_seconds": 600, "p90_seconds": 600, "slowest_seconds": 600 }
            ],
            "heads": [{ "number": 952, "head_sha": "a", "started_at": "x", "completed_at": "y",
                        "wall_seconds": 2460, "slowest_check": "site", "slowest_seconds": 2460, "failed": false }]
        }})))
        .render();
        for want in [
            "site",
            "40 min 00 s",
            "41 min 00 s",
            "1 failed",
            "/ui/prs?pr=952#pr",
            "<svg",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(
            html.contains("mj-tone--rose"),
            "a check that failed is drawn as one"
        );
    }

    #[test]
    fn no_observation_and_no_measured_run_say_so_and_draw_nothing() {
        let none = panel_of(&answer(
            serde_json::json!({ "observed": false, "reason": "no forge observation is recorded" }),
        ))
        .render();
        assert!(none.contains("so no run is measured: no forge observation is recorded"));
        assert!(!none.contains("<svg"));
        let empty = panel_of(&answer(serde_json::json!({ "observed": true, "timings": {
            "observed_at": "t", "measured_runs": 0, "unmeasured_runs": 4, "checks": [], "heads": [] }}))).render();
        assert!(empty.contains("has both a start and an end"));
        assert!(!empty.contains("<svg"));
    }

    /// A head whose checks failed is drawn in the failing tone even when no check's median
    /// is.
    #[test]
    fn a_failed_head_is_drawn_as_one() {
        let html = panel_of(&answer(serde_json::json!({ "observed": true, "timings": {
            "observed_at": "2026-10-11T12:00:00Z", "measured_runs": 1, "unmeasured_runs": 0,
            "checks": [{ "name": "rust", "runs": 1, "passed": 1, "failed": 0, "median_seconds": 600,
                         "p90_seconds": 600, "slowest_seconds": 600 }],
            "heads": [{ "number": 960, "head_sha": "a", "wall_seconds": 600,
                        "slowest_check": "rust", "slowest_seconds": 600, "failed": true }]
        }})))
        .render();
        assert!(html.contains("/ui/prs?pr=960#pr"), "{html}");
        assert!(html.contains("mj-tone--rose"), "{html}");
    }
}
