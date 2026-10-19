//! Trains: the open pull requests stacked on each other, in the order they land, read from
//! `integration.trains`.
//!
//! A reader who opens the queue sees each pull request alone; a train is what they cannot see
//! there — that #12 waits for #11 because it is built on #11's branch, and that both land
//! together when the landing seat lands the top of the segment. The screen draws each train
//! as its links in landing order, every link leading to its pull request, its checks and the
//! drain's detail of it, and it says in words what a train is and what each word on it means.
//!
//! Nothing here is typed twice. The trains are the capability's answer; the meaning of each
//! disposition and lane is the doc comment of its variant in the integration model, read from
//! the variant's JSON schema (`meanings`), so the legend changes when the model does. The
//! screen reads and moves nothing.

use schemars::JsonSchema;
use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::integration::model::{IntegrationLane, PullRequestDisposition};

use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{
    hero, kpi, kpi_count, link, list, nothing, panel, state, table, text, unanswered, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/trains";

const TRAINS: Source = Source::of("integration.trains");

/// Where the integration model is explained in prose, on the published site.
const MANUAL: &str = "https://majordomus.dev/docs/integration/";

/// Every variant of an enum with the doc comment of that variant, as the enum's JSON schema
/// carries them: `(word, meaning)`, in declaration order, with the comment's line breaks
/// joined into one line. A variant without a comment is listed with an empty meaning rather
/// than dropped, so a word the screen draws is always a word the legend names.
pub fn meanings<T: JsonSchema>() -> Vec<(String, String)> {
    let schema = schemars::schema_for!(T).to_value();
    let mut out = Vec::new();
    for key in ["oneOf", "anyOf"] {
        for variant in schema
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let word = variant.get("const").and_then(Value::as_str).or_else(|| {
                variant
                    .get("enum")
                    .and_then(Value::as_array)
                    .and_then(|e| e.first())
                    .and_then(Value::as_str)
            });
            if let Some(word) = word {
                let meaning = variant
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push((word.to_string(), meaning));
            }
        }
    }
    if out.is_empty() {
        // a unit enum without comments is a plain `enum` list
        for word in schema
            .get("enum")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(word) = word.as_str() {
                out.push((word.to_string(), String::new()));
            }
        }
    }
    out
}

/// The meaning of one word of a vocabulary, or nothing when the model does not name it.
fn meaning_of(vocabulary: &[(String, String)], word: &str) -> String {
    vocabulary
        .iter()
        .find(|(w, _)| w == word)
        .map(|(_, m)| m.clone())
        .unwrap_or_default()
}

/// The Trains screen.
pub fn screen(ctx: &Context, _query: &[(String, String)]) -> View {
    let answers = ask_all(ctx, &[TRAINS]);
    View::new("Trains", body(&answers[&TRAINS]))
}

/// The screen of one answer: kept apart from the asking, so every shape of the answer can be
/// drawn and tested without a forge observation behind it.
fn body(trains: &Answer) -> El {
    let dispositions = meanings::<PullRequestDisposition>();
    let lanes = meanings::<IntegrationLane>();

    let top = hero(
        "layers",
        "blue",
        "Trains",
        "Pull requests stacked on each other, in the order they land.",
        "A train is a run of open pull requests in which each one is built on the branch of the one below it rather than on master. They land from the bottom up, together: the landing seat lands the top of a prepared segment and every pull request beneath it lands with it.",
        vec![
            kpi("Trains", &trains.read("/tallies/trains")),
            kpi("Pull requests in trains", &trains.read("/tallies/links")),
            kpi("Ready to land", &trains.read("/tallies/by_disposition/ready")),
            kpi_count("Prepared heads no train holds", &trains.read("/unattached_prep")),
        ],
    );

    let explained = el("ul")
        .class("mj-ui-prose-list")
        .child(el("li").text(
            "Read a train from the top of its table down: that is the order in which its pull requests land.",
        ))
        .child(el("li").text(
            "The first pull request whose dependencies have all landed is next in order, and nothing above it can land before it, because each is built on the branch below. Whether it can merge now is its disposition: next in order is not the same as ready.",
        ))
        .child(el("li").text(
            "Each pull request has one disposition, why it is where it is, and one lane, what it is waiting for; hover a word, or read the legend at the foot of the screen, for what the integration model means by it.",
        ))
        .child(
            el("li")
                .text("The landing seat lands trains, and who holds it is on ")
                .child(link(super::mesh::ROUTE, "the Mesh screen"))
                .text("; every open pull request, lane by lane, is on ")
                .child(link(super::prs::ROUTE, "the PR Drain"))
                .text("; how integration works is in ")
                .child(link(MANUAL, "the integration manual"))
                .text("."),
        );

    let mut grid = el("div").class("mj-ui-grid").child(wide(panel(
        "What a train is",
        "How to read this screen, and where each decision about a train is made.",
        explained,
        &[trains],
    )));

    let observed = trains
        .value
        .as_ref()
        .ok()
        .and_then(|v| v.get("observed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let trains_list = list(trains, "/trains");

    let drawn = unanswered(trains).unwrap_or_else(|| {
        if !observed {
            let reason = trains
                .value
                .as_ref()
                .ok()
                .map(|v| text(v, "reason"))
                .filter(|r| !r.is_empty())
                .unwrap_or_else(|| "nothing is observed".into());
            el("div")
                .child(nothing(&format!("No trains are drawn: {reason}.")))
                .child(el("p").class("mj-ui-dim").text(
                    "Trains are read from a recorded observation of the forge; `majordomus prs refresh` records one.",
                ))
        } else if trains_list.is_empty() {
            nothing("No open pull request is stacked on another, and none declares a dependency on another.")
        } else {
            overview(&trains_list)
        }
    });
    grid = grid.child(wide(panel(
        "Trains",
        "Every train at a glance: how long it is, what it is built on, and which pull request lands next. A train's name leads to its table.",
        drawn,
        &[trains],
    )));

    for train in &trains_list {
        grid = grid.child(wide(train_panel(train, trains, &dispositions, &lanes)));
    }

    grid = grid.child(wide(panel(
        "What each word means",
        "The dispositions and lanes of the integration model, with the meaning the model itself gives each.",
        legend(&dispositions, &lanes),
        &[trains],
    )));

    el("div").class("mj-ui-stack").child(top).child(grid)
}

/// One train: its links in landing order, the next in order marked with whether it can merge.
fn train_panel(
    train: &Value,
    answer: &Answer,
    dispositions: &[(String, String)],
    lanes: &[(String, String)],
) -> El {
    let id = text(train, "id");
    let next = train.get("next").and_then(Value::as_u64);
    let links = train
        .get("links")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let base = links
        .first()
        .map(|l| text(l, "base_ref"))
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| "master".into());
    let next_word = next
        .and_then(|n| {
            links
                .iter()
                .find(|l| l.get("number").and_then(Value::as_u64) == Some(n))
        })
        .map(|l| text(l, "disposition").replace('_', " "))
        .filter(|w| !w.is_empty());
    let sub = match (next, next_word) {
        (Some(n), Some(word)) if word == "ready" => format!(
            "{} pull request(s) built on {base}; #{n} is next in order and ready to land.",
            links.len()
        ),
        (Some(n), Some(word)) => format!(
            "{} pull request(s) built on {base}; #{n} is next in order, and cannot merge yet: {word}.",
            links.len()
        ),
        (Some(n), None) => format!(
            "{} pull request(s) built on {base}; #{n} is next in order.",
            links.len()
        ),
        (None, _) => format!(
            "{} pull request(s) built on {base}; none of them has every dependency landed yet.",
            links.len()
        ),
    };

    let rows = links
        .iter()
        .map(|l| {
            let number = l.get("number").and_then(Value::as_u64);
            let label = number.map_or_else(|| "unknown".to_string(), |n| format!("#{n}"));
            let url = text(l, "url");
            let pr = if url.is_empty() {
                el("span").class("mj-ui-mono").text(label.clone())
            } else {
                link(url, label.clone())
            };
            let mut first = el("td").child(pr);
            if next.is_some() && next == number {
                first = first.child(el("span").class("mj-ui-dim").text(" next in order"));
            }
            let disposition = text(l, "disposition");
            let lane = text(l, "lane");
            let why = {
                let action = text(l, "next_action");
                if !action.is_empty() {
                    action
                } else {
                    l.get("reasons")
                        .and_then(Value::as_array)
                        .and_then(|r| r.first())
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                }
            };
            let checks_url = text(l, "checks_url");
            let checks = if checks_url.is_empty() {
                el("td").child(state(l, "required_checks"))
            } else {
                el("td").child(
                    el("a")
                        .attr("href", checks_url)
                        .attr("aria-label", format!("The checks of {label}"))
                        .child(state(l, "required_checks")),
                )
            };
            el("tr")
                .child(first)
                .child(
                    el("td").child(
                        el("a")
                            .class("mj-ui-row-link")
                            .attr(
                                "href",
                                number.map_or_else(
                                    || super::prs::ROUTE.to_string(),
                                    |n| format!("{}?pr={n}", super::prs::ROUTE),
                                ),
                            )
                            .child(el("span").class("mj-ui-cell-title").text(text(l, "title")))
                            .child(
                                el("span")
                                    .class("mj-ui-dim")
                                    .text(format!(" by {}", text(l, "author"))),
                            ),
                    ),
                )
                .child(el("td").class("mj-ui-mono mj-ui-dim").text(format!(
                    "{} → {}",
                    text(l, "head_ref"),
                    text(l, "base_ref")
                )))
                .child(
                    el("td")
                        .attr("title", meaning_of(dispositions, &disposition))
                        .child(state(l, "disposition")),
                )
                .child(
                    el("td")
                        .attr("title", meaning_of(lanes, &lane))
                        .child(state(l, "lane")),
                )
                .child(checks)
                .child(el("td").class("mj-ui-dim").text(why))
        })
        .collect::<Vec<_>>();

    let title = match root(train) {
        Some(n) => format!("Train #{n} → {base}"),
        None => format!("Train {id}"),
    };
    let drawn = panel(
        &title,
        &sub,
        table(
            &[
                "Pull request",
                "Title",
                "Branch",
                "Disposition",
                "Lane",
                "Checks",
                "What happens next",
            ],
            rows,
            "This train has no open pull request.",
        ),
        &[answer],
    );
    el("div").attr("id", anchor(&id)).child(drawn)
}

/// The pull request a train starts from: the first of its roots, else its first link.
fn root(train: &Value) -> Option<u64> {
    train
        .get("roots")
        .and_then(Value::as_array)
        .and_then(|r| r.first())
        .and_then(Value::as_u64)
        .or_else(|| {
            train
                .get("links")
                .and_then(Value::as_array)
                .and_then(|l| l.first())
                .and_then(|l| l.get("number"))
                .and_then(Value::as_u64)
        })
}

/// The id of a train's panel on the screen, so the overview can lead to it.
fn anchor(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("train-{}", safe.trim_start_matches("train-"))
}

/// Every train in one table: its name leading to its panel, how many pull requests it holds,
/// what it is built on, which lands next, and how many of its links stand in each disposition.
fn overview(trains: &[Value]) -> El {
    let rows = trains
        .iter()
        .map(|t| {
            let id = text(t, "id");
            let links = t
                .get("links")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let base = links
                .first()
                .map(|l| text(l, "base_ref"))
                .filter(|b| !b.is_empty())
                .unwrap_or_else(|| "master".into());
            let name = match root(t) {
                Some(n) => format!("#{n}"),
                None => id.clone(),
            };
            let next = match t.get("next").and_then(Value::as_u64) {
                Some(n) => {
                    let word = links
                        .iter()
                        .find(|l| l.get("number").and_then(Value::as_u64) == Some(n))
                        .map(|l| text(l, "disposition").replace('_', " "))
                        .unwrap_or_default();
                    if word.is_empty() {
                        format!("#{n}")
                    } else {
                        format!("#{n}, {word}")
                    }
                }
                None => "none yet".into(),
            };
            // the model's own order (by_disposition is a map keyed by the word); no second order here
            let by: Vec<(String, u64)> = t
                .get("by_disposition")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| v.as_u64().map(|n| (k.replace('_', " "), n)))
                        .collect()
                })
                .unwrap_or_default();
            let standing = by
                .iter()
                .map(|(word, n)| format!("{n} {word}"))
                .collect::<Vec<_>>()
                .join(", ");
            el("tr")
                .child(el("td").child(link(format!("#{}", anchor(&id)), name)))
                .child(
                    el("td").text(
                        t.get("links")
                            .and_then(Value::as_array)
                            .map_or_else(|| "unknown".to_string(), |l| l.len().to_string()),
                    ),
                )
                .child(el("td").class("mj-ui-mono mj-ui-dim").text(base))
                .child(el("td").text(next))
                .child(el("td").class("mj-ui-dim").text(standing))
        })
        .collect::<Vec<_>>();
    table(
        &[
            "Train",
            "Pull requests",
            "Built on",
            "Next to land",
            "Where its pull requests stand",
        ],
        rows,
        "No train.",
    )
}

/// The legend: every disposition and lane with the model's own meaning.
fn legend(dispositions: &[(String, String)], lanes: &[(String, String)]) -> El {
    let rows = |words: &[(String, String)], kind: &str| {
        words
            .iter()
            .map(|(word, meaning)| {
                el("tr")
                    .child(el("td").class("mj-ui-dim").text(kind.to_string()))
                    .child(el("td").class("mj-ui-mono").text(word.replace('_', " ")))
                    .child(el("td").text(meaning.clone()))
            })
            .collect::<Vec<_>>()
    };
    let mut all = rows(dispositions, "disposition");
    all.extend(rows(lanes, "lane"));
    table(
        &["Kind", "Word", "What it means"],
        all,
        "The model names no word.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    use crate::ui::source::Exposure;

    fn answer(value: Value) -> Answer {
        Answer {
            source: TRAINS,
            route: Some("GET /api/v1/pull-requests/trains".into()),
            exposure: Exposure::default(),
            value: Ok(value.into()),
            took: Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        }
    }

    /// The legend is the model's: every disposition the integration model declares is a row,
    /// with the variant's own doc comment as its meaning, and a word added to the model
    /// appears here without this file changing.
    #[test]
    fn the_legend_is_the_models_own_words() {
        let words = meanings::<PullRequestDisposition>();
        assert!(
            words
                .iter()
                .any(|(w, m)| w == "ready" && m.contains("required check passed")),
            "{words:?}"
        );
        assert!(
            words
                .iter()
                .any(|(w, m)| w == "needs_refresh" && m.contains("does not contain master")),
            "{words:?}"
        );
        assert!(
            words.iter().all(|(_, m)| !m.contains('\n')),
            "a meaning is one line"
        );
        let lanes = meanings::<IntegrationLane>();
        assert!(lanes.iter().any(|(w, _)| w == "ready"), "{lanes:?}");
    }

    /// A train is its links in landing order, each leading to its pull request, its checks and
    /// the drain's detail of it, the next in order marked, and each word explained on hover.
    #[test]
    fn a_train_is_drawn_in_landing_order_with_every_link_followable() {
        let view = json!({
            "observed": true,
            "tallies": { "trains": 1, "links": 2, "pending_prep": 0, "by_disposition": { "ready": 1, "needs_refresh": 1 } },
            "unattached_prep": [],
            "trains": [{
                "id": "train-11",
                "roots": [11],
                "next": 11,
                "by_disposition": { "ready": 1, "needs_refresh": 1 },
                "links": [
                    { "number": 11, "title": "The engine", "author": "korczis", "head_ref": "feature/engine",
                      "base_ref": "master", "url": "https://github.com/o/r/pull/11", "checks_url": "https://github.com/o/r/pull/11/checks",
                      "disposition": "ready", "lane": "ready", "required_checks": "passed", "reasons": [], "next_action": "the landing seat merges it" },
                    { "number": 12, "title": "Its first use", "author": "korczis", "head_ref": "feature/use",
                      "base_ref": "feature/engine", "url": "https://github.com/o/r/pull/12", "checks_url": "",
                      "disposition": "needs_refresh", "lane": "waiting", "required_checks": "pending", "reasons": ["does not contain master"], "next_action": "" }
                ]
            }]
        });
        let html = body(&answer(view)).render();
        let at = |needle: &str| {
            html.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}\n{html}"))
        };
        assert!(
            at("#11") < at("#12"),
            "landing order: the link built on master first"
        );
        assert!(
            html.contains("https://github.com/o/r/pull/11/checks"),
            "the checks are followable"
        );
        assert!(
            html.contains("/ui/prs?pr=12"),
            "the drain's detail of each link"
        );
        assert!(
            html.contains("next in order"),
            "the next in order is marked"
        );
        assert!(
            html.contains("#11 is next in order and ready to land"),
            "and said, with whether it can merge, in the panel's line"
        );
        assert!(
            html.contains("#11, ready"),
            "the overview names the next link's disposition"
        );
        assert!(
            html.contains("feature/use → feature/engine"),
            "the branch a link is built on"
        );
        assert!(
            html.contains("does not contain master"),
            "the reason when there is no next action"
        );
        assert!(
            html.contains(super::super::mesh::ROUTE) && html.contains(MANUAL),
            "who lands, and the manual"
        );
        assert!(html.contains("What each word means"), "the legend");
        assert!(
            html.contains("Train #11 → master"),
            "a train is named by the pull request it starts from"
        );
        assert!(
            html.contains(r##"href="#train-11""##) && html.contains(r##"id="train-11""##),
            "the overview leads to the train's own table"
        );
        assert!(
            html.contains("1 needs refresh") && html.contains("1 ready"),
            "where its pull requests stand"
        );
    }

    /// A link the observation names without a number, and a train without its links, are
    /// unknown: never "#0", never a link to `?pr=0`, never a count of none.
    #[test]
    fn an_absent_number_or_count_is_unknown_not_zero() {
        let view = json!({
            "observed": true,
            "tallies": { "trains": 2, "links": 1, "pending_prep": 0, "by_disposition": {} },
            "unattached_prep": [],
            "trains": [
                { "id": "train-x", "links": [ { "title": "Nameless", "author": "a", "head_ref": "h",
                  "base_ref": "master", "url": "", "checks_url": "", "disposition": "blocked",
                  "lane": "repair", "required_checks": "unknown", "reasons": [], "next_action": "" } ] },
                { "id": "train-y" }
            ]
        });
        let html = body(&answer(view)).render();
        assert!(!html.contains("#0"), "{html}");
        assert!(!html.contains("pr=0"), "{html}");
        assert!(
            html.contains(">unknown<"),
            "an absent number or count says unknown: {html}"
        );
    }

    /// No observation is said with its reason and the command that records one, never drawn
    /// as an empty list of trains.
    #[test]
    fn no_observation_is_said_not_drawn_as_no_trains() {
        let html = body(&answer(
            json!({ "observed": false, "reason": "no forge observation is recorded",
            "tallies": { "trains": 0, "links": 0, "pending_prep": 0, "by_disposition": {} },
            "trains": [], "unattached_prep": [] }),
        ))
        .render();
        assert!(
            html.contains("No trains are drawn: no forge observation is recorded."),
            "{html}"
        );
        assert!(html.contains("majordomus prs refresh"), "{html}");
        assert!(!html.contains("No open pull request is stacked"), "{html}");
    }
}
