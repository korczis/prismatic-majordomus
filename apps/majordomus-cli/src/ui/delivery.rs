//! CI/CD & Releases: what was released, whether the landed work since is released, where it
//! is deployed, and what the CI model runs. Read from `distribution.releases`,
//! `release.version`, `release.currency`, `deploy.list` and `gates.model`.
//!
//! How long CI takes is `integration.timings`, measured from the recorded forge observation
//! (`super::timings`): the CI model says which gates run and when, the timings how long the
//! runs on the open pull requests took.

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;
use serde_json::Value;

use super::dashboard::releases_panel;
use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{
    anchor, code_link, hero, kpi, kpi_state, list, none, panel, table, text, unanswered, value,
    wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/delivery";

/// Where one gate of the CI model is: its row on this screen. A rule's proof and a pull
/// request's checks name gates by id, and lead here.
pub fn gate_href(id: &str) -> String {
    format!("{ROUTE}#gate-{}", anchor(id))
}

const RELEASES: Source = Source::of("distribution.releases");
const VERSION: Source = Source::of("release.version");
const DEPLOYMENTS: Source = Source::of("deploy.list");
const GATES: Source = Source::of("gates.model");
// asked without the version: the hero's Next is release.version's, and one screen names one
// next version (the plan critique of 2026-10-19)
const CURRENCY: Source = Source::with("release.currency", r#"{"next":false}"#);

/// The CI/CD & Releases screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(
        ctx,
        &[
            RELEASES,
            VERSION,
            DEPLOYMENTS,
            GATES,
            CURRENCY,
            super::timings::TIMINGS,
        ],
    );
    let (releases, version, deployments, gates, currency) = (
        &answers[&RELEASES],
        &answers[&VERSION],
        &answers[&DEPLOYMENTS],
        &answers[&GATES],
        &answers[&CURRENCY],
    );

    let top = hero(
        "layers",
        "orange",
        "CI/CD & Releases",
        "From a green check to a published release.",
        "What was released, whether the work landed since is released, what the tree declares and what its commits imply next, where it is deployed, and every gate the CI model can run.",
        vec![
            kpi("Released", &releases.read("/latest/version")),
            kpi_state("Since then", &currency.read("/stance")),
            kpi("Declared", &version.read("/declared")),
            kpi("Next", &version.read("/next")),
            kpi("Gates", &gates.read("/count")),
        ],
    );

    let deployed = unanswered(deployments).unwrap_or_else(|| {
        let rows = list(deployments, "/deployments")
            .iter()
            .map(|d| super::deployments::summary_row(ctx, d))
            .collect::<Vec<_>>();
        table(
            &["Deployment", "Application", "State"],
            rows,
            "No deployment is declared.",
        )
    });

    let model = unanswered(gates).unwrap_or_else(|| {
        let rows = list(gates, "/gates")
            .iter()
            .map(|g| {
                let when = if g.get("always").and_then(Value::as_bool) == Some(true) {
                    "every plan"
                } else if g.get("on-demand").and_then(Value::as_bool) == Some(true) {
                    "on demand"
                } else {
                    "when its paths change"
                };
                let id = text(g, "id");
                el("tr")
                    .attr("id", format!("gate-{}", anchor(&id)))
                    .child(el("td").child(code_link(gate_href(&id), value(g, "id"))))
                    .child(el("td").text(value(g, "job")))
                    .child(el("td").class("mj-ui-dim").text(when))
                    .child(el("td").class("mj-ui-dim").text(value(g, "summary")))
                    .child(el("td").child(gates_named(g, "implies")))
            })
            .collect::<Vec<_>>();
        table(
            &["Gate", "Job", "Runs", "What it holds", "Implies"],
            rows,
            "The CI model declares no gate.",
        )
    });

    let grid = el("div")
        .class("mj-ui-grid")
        .child(releases_panel(releases, version, None))
        .child(panel(
            "Deployments",
            "Where the product is declared to run.",
            deployed,
            &[deployments],
        ))
        // full width, after the pair above, so that adding it moves no panel of the grid
        .child(wide(currency_panel(currency)))
        .child(wide(panel(
            "CI model",
            "Every gate the model declares, the job it runs in and when it is selected.",
            model,
            &[gates],
        )))
        .child(wide(super::timings::panel_of(
            &answers[&super::timings::TIMINGS],
        )));

    View::new(
        "CI/CD & Releases",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// Whether the trunk's landed work is released (`release.currency`, I3071): the stance and
/// its sentence, how much landed work waits, since when and how that was dated, and the
/// release it is measured from. The version required next is the hero's, read from
/// `release.version`. The capability reads the remote-tracking trunk as of its last fetch and
/// the clock, so the panel says that a published snapshot shows it as read when the snapshot
/// was built.
fn currency_panel(currency: &Answer) -> El {
    let body = unanswered(currency)
        .unwrap_or_else(|| currency_body(currency.ok().unwrap_or(&Value::Null)));
    panel(
        "Is the trunk released?",
        "The landed work past the last release, read from the remote-tracking trunk as of its last fetch. A published snapshot shows it as read when the snapshot was built.",
        body,
        &[currency],
    )
}

/// The rows of one answer of `release.currency`, each in the answer's own words: a figure
/// the answer did not give is said as missing, never as a zero.
fn currency_body(answer: &Value) -> El {
    let number = |key: &str| answer.get(key).and_then(Value::as_u64);
    let waited = match (
        number("waited_seconds"),
        answer.get("oldest_landing").and_then(Value::as_str),
    ) {
        (Some(seconds), at) => {
            let dated = match answer.get("landed_at").and_then(Value::as_str) {
                // a commit time bounds the wait from above, and the row says so
                Some("commit") => format!(
                    "at most {}, from its commit time: this clone holds no record of when it landed",
                    super::timings::duration(seconds)
                ),
                _ => format!(
                    "{}, since this clone saw it land",
                    super::timings::duration(seconds)
                ),
            };
            match at {
                Some(at) => format!("{dated} ({at})"),
                None => dated,
            }
        }
        (None, _) => "nothing waits".into(),
    };
    let landed = match (number("landed"), number("landings")) {
        (Some(commits), Some(landings)) => {
            format!("{commits} commit(s) in {landings} landing(s)")
        }
        _ => "not measured".into(),
    };
    let measured_from = answer.get("release").map_or_else(
        || "no release".to_string(),
        |release| {
            let reference = value(release, "reference");
            if release.get("recorded").and_then(Value::as_bool) == Some(false) {
                format!("{reference} (tagged; its record has not landed yet)")
            } else {
                reference
            }
        },
    );
    let row = |label: &str, cell: El| {
        el("tr")
            .child(el("th").attr("scope", "row").text(label))
            .child(el("td").child(cell))
    };
    table(
        &["Reading", "Answer"],
        vec![
            row(
                "Stance",
                el("span")
                    .child(word_badge(&value(answer, "stance")))
                    .text(" ")
                    .text(text(answer, "summary")),
            ),
            row("Landed past it", el("span").text(landed)),
            row("Waited", el("span").text(waited)),
            row("Measured from", el("span").text(measured_from)),
        ],
        "release.currency gave no answer.",
    )
}

/// The gates an entry names under `key`, each a link to its own row; `none` when it names none.
fn gates_named(g: &Value, key: &str) -> crate::cockpit::html::El {
    let ids: Vec<String> = g
        .get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if ids.is_empty() {
        return none();
    }
    el("span")
        .class("mj-ui-badges")
        .children(ids.iter().map(|id| code_link(gate_href(id), id.clone())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::deployments::tests::furnished;

    /// Every declared deployment is a row, and every gate of the CI model says when it
    /// runs: in every plan, on demand, or when its paths change.
    #[test]
    fn every_deployment_and_every_gate_is_a_row_that_says_when_it_runs() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(
            html.contains("The Majordomus layer, served from its container image"),
            "{html}"
        );
        assert!(!html.contains("No deployment is declared."), "{html}");
        for (gate, when) in [
            ("every-plan", "every plan"),
            ("when-asked", "on demand"),
            ("by-path", "when its paths change"),
        ] {
            assert!(html.contains(gate), "{gate} is missing: {html}");
            assert!(html.contains(when), "{gate} does not say {when}: {html}");
        }
        assert!(
            html.contains("runs in every plan"),
            "the gate's own summary: {html}"
        );
        assert!(!html.contains("The CI model declares no gate."), "{html}");
    }

    /// The release panel says the stance in its own sentence, how much landed work waits and
    /// since when, the release it is measured from and the version required next (I3071).
    #[test]
    fn the_release_panel_says_what_waits_past_the_last_release() {
        // the answers below are shaped as measure() gives them, asked with next false
        let owed = currency_body(&serde_json::json!({
            "stance": "owed",
            "summary": "1 landed commit(s) in 1 landing(s) wait past v0.30.2 (tagged, its record not landed yet) on origin/master, the oldest for 1 h 2 min since this clone saw it land",
            "trunk": "origin/master",
            "release": {"version": "0.30.2", "reference": "v0.30.2", "at": "v0.30.2", "recorded": false},
            "landed": 1,
            "landings": 1,
            "oldest_landing": "2026-10-19T05:00:00Z",
            "landed_at": "reflog",
            "waited_seconds": 3720,
            "overdue_after_seconds": 21600
        }))
        .render();
        for want in [
            ">owed<",
            "wait past v0.30.2",
            "1 commit(s) in 1 landing(s)",
            "1 h 02 min, since this clone saw it land (2026-10-19T05:00:00Z)",
            "v0.30.2 (tagged; its record has not landed yet)",
        ] {
            assert!(owed.contains(want), "{want}: {owed}");
        }

        let bounded = currency_body(&serde_json::json!({
            "stance": "owed",
            "summary": "1 landed commit(s) in 1 landing(s) wait past v0.30.2 on origin/master, the oldest for at most 7 h 0 min",
            "trunk": "origin/master",
            "release": {"version": "0.30.2", "reference": "v0.30.2", "at": "abc", "recorded": true},
            "landed": 1,
            "landings": 1,
            "oldest_landing": "2026-10-18T23:00:00Z",
            "landed_at": "commit",
            "waited_seconds": 25200,
            "overdue_after_seconds": 21600
        }))
        .render();
        assert!(
            bounded.contains("at most 7 h 00 min, from its commit time"),
            "{bounded}"
        );

        let current = currency_body(&serde_json::json!({
            "stance": "current",
            "summary": "origin/master holds no landed work past v0.30.2",
            "trunk": "origin/master",
            "release": {"version": "0.30.2", "reference": "v0.30.2", "at": "abc", "recorded": true},
            "landed": 0,
            "landings": 0,
            "overdue_after_seconds": 21600
        }))
        .render();
        assert!(current.contains("nothing waits"), "{current}");
        assert!(
            current.contains("holds no landed work past v0.30.2"),
            "{current}"
        );
        assert!(!current.contains("its record has not landed"), "{current}");

        // a figure the answer did not give is said as missing, never as a zero
        let bare = currency_body(&serde_json::json!({
            "stance": "unknown",
            "summary": "the trunk cannot be told: no remote-tracking branch",
            "landed": 0,
            "landings": 0,
            "overdue_after_seconds": 21600
        }))
        .render();
        assert!(bare.contains("no release"), "{bare}");
        assert!(bare.contains("nothing waits"), "{bare}");
        assert!(bare.contains("no remote-tracking branch"), "{bare}");

        // a wait the answer gave without its landing's date is the wait alone, and counts it
        // did not give are not measured
        let partial = currency_body(&serde_json::json!({
            "stance": "owed",
            "summary": "landed work waits past v0.30.2 on origin/master",
            "trunk": "origin/master",
            "release": {"version": "0.30.2", "reference": "v0.30.2", "at": "abc", "recorded": true},
            "landed_at": "reflog",
            "waited_seconds": 3600,
            "overdue_after_seconds": 21600
        }))
        .render();
        assert!(
            partial.contains("1 h 00 min, since this clone saw it land"),
            "{partial}"
        );
        assert!(!partial.contains("saw it land ("), "{partial}");
        assert!(partial.contains("not measured"), "{partial}");
    }

    /// The screen reads release.currency: a KPI beside what was released, and the panel that
    /// names the capability it read.
    #[test]
    fn the_screen_says_whether_the_trunk_is_released() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(html.contains("Since then"), "{html}");
        assert!(html.contains("Is the trunk released?"), "{html}");
        assert!(
            html.contains("release.currency"),
            "the panel's source line names the capability: {html}"
        );
    }

    /// A gate is a row a link can lead to, and the gates it implies lead to theirs.
    #[test]
    fn a_gate_is_a_row_links_lead_to() {
        assert_eq!(gate_href("shell-suite"), "/ui/delivery#gate-shell-suite");
        let implied = gates_named(
            &serde_json::json!({ "implies": ["rust-integration", "rust-quality"] }),
            "implies",
        )
        .render();
        assert!(
            implied.contains("href=\"/ui/delivery#gate-rust-integration\""),
            "{implied}"
        );
        assert!(
            implied.contains("href=\"/ui/delivery#gate-rust-quality\""),
            "{implied}"
        );
        assert!(gates_named(&serde_json::json!({}), "implies")
            .render()
            .contains(">none<"));

        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(html.contains("id=\"gate-every-plan\""), "{html}");
        assert!(
            html.contains("href=\"/ui/delivery#gate-by-path\""),
            "{html}"
        );
        assert!(
            html.contains(&format!(
                "href=\"{}\"",
                super::super::deployments::href("majordomus")
            )),
            "a deployment leads to its row on Deployments: {html}"
        );
    }
}
