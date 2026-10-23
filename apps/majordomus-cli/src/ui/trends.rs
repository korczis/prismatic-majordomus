//! Trends: how coverage and CI time move from run to run, for the CI/CD family of screens.
//! Read from `delivery.trends`, which reads `site/data/trends.json` of this checkout as it
//! lies when the screen is asked: the series the pages publication recorded, oldest run
//! first. Coverage is drawn as percentage lines (lines, functions, regions), wall and queued
//! time as minutes per run, both as inline SVG, with the slowest job medians in a table and
//! every run's figures as text beneath.
//!
//! The screen writes nothing and derives nothing: a checkout the publication never wrote
//! into has no series, and says so in words. A run that measured no coverage is a gap in the
//! line, never a point at zero; a figure the file does not give reads `unknown`.

use serde_json::Value;

use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::timings::duration;
use super::view::{
    hero, kpi, kpi_count, kpi_not_observed, list, nothing, panel, table, text, unanswered, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/trends";

const TRENDS: Source = Source::of("delivery.trends");

/// What the screen says when the checkout holds no series.
pub const NONE_RECORDED: &str =
    "no trends recorded in this checkout: the series is written by the pages publication";

/// The chart's drawing area, in the units of its view box.
const WIDTH: f64 = 600.0;
const HEIGHT: f64 = 200.0;
const LEFT: f64 = 44.0;
const RIGHT: f64 = 12.0;
const TOP: f64 = 12.0;
const BOTTOM: f64 = 28.0;

/// One line of a chart: its name, its tone and one value per run, `None` where the run has
/// none.
struct Line {
    name: &'static str,
    tone: &'static str,
    values: Vec<Option<f64>>,
}

fn number(v: &Value, pointer: &str) -> Option<f64> {
    v.pointer(pointer).and_then(Value::as_f64)
}

/// A percentage as a reader says it: one place, or `unknown`.
fn percent(p: Option<f64>) -> String {
    p.map_or_else(|| "unknown".into(), |p| format!("{p:.1}%"))
}

/// Seconds as minutes with one place, or `unknown`: never a zero the file did not give.
fn minutes(s: Option<f64>) -> String {
    s.map_or_else(|| "unknown".into(), |s| format!("{:.1} min", s / 60.0))
}

/// The day of an RFC 3339 timestamp, as written.
fn day(run: &Value) -> String {
    run.get("started_at")
        .and_then(Value::as_str)
        .map_or_else(|| "unknown".into(), super::view::day)
}

/// A line chart of `lines` over the runs, as an SVG with the numbers of each line in a
/// legend beneath it. `ceiling` fixes the top of the scale; `None` scales to the largest
/// value drawn. A `None` value breaks the line: the run has no point there.
fn chart(label: &str, unit: &str, lines: &[Line], runs: &[Value], ceiling: Option<f64>) -> El {
    let drawn = lines.iter().flat_map(|l| l.values.iter().flatten()).count();
    if drawn == 0 {
        return nothing(&format!("No run recorded {label}."));
    }
    let top = ceiling.unwrap_or_else(|| {
        lines
            .iter()
            .flat_map(|l| l.values.iter().flatten().copied())
            .fold(0.0_f64, f64::max)
            .max(1.0)
    });
    let n = runs.len();
    let x = |i: usize| {
        if n < 2 {
            LEFT + (WIDTH - LEFT - RIGHT) / 2.0
        } else {
            LEFT + (WIDTH - LEFT - RIGHT) * i as f64 / (n - 1) as f64
        }
    };
    let y = |v: f64| TOP + (HEIGHT - TOP - BOTTOM) * (1.0 - v.min(top) / top);
    let mut svg = el("svg")
        .attr("viewBox", format!("0 0 {WIDTH} {HEIGHT}"))
        .attr("width", "100%")
        .attr("role", "img")
        .attr(
            "aria-label",
            format!("{label}, one point per run, oldest first"),
        )
        .attr("focusable", "false");
    // the axes: a baseline at zero and a rule at the top of the scale, each named
    for (value, caption) in [(0.0, format!("0{unit}")), (top, format!("{top:.0}{unit}"))] {
        svg = svg
            .child(
                el("line")
                    .attr("x1", format!("{LEFT}"))
                    .attr("x2", format!("{}", WIDTH - RIGHT))
                    .attr("y1", format!("{:.1}", y(value)))
                    .attr("y2", format!("{:.1}", y(value)))
                    .attr("stroke", "currentColor")
                    .attr("opacity", "0.2"),
            )
            .child(
                el("text")
                    .attr("x", format!("{:.1}", LEFT - 6.0))
                    .attr("y", format!("{:.1}", y(value) + 4.0))
                    .attr("text-anchor", "end")
                    .attr("font-size", "11")
                    .attr("fill", "currentColor")
                    .text(caption),
            );
    }
    // one run is captioned once, centred under its point; more runs at both ends of the axis
    let captions = match (runs.first(), runs.last()) {
        (Some(only), _) if n == 1 => vec![(0, only, "middle")],
        (Some(first), Some(last)) => vec![(0, first, "start"), (n - 1, last, "end")],
        _ => Vec::new(),
    };
    for (i, run, anchor) in captions {
        svg = svg.child(
            el("text")
                .attr("x", format!("{:.1}", x(i)))
                .attr("y", format!("{:.1}", HEIGHT - 8.0))
                .attr("text-anchor", anchor)
                .attr("font-size", "11")
                .attr("fill", "currentColor")
                .text(day(run)),
        );
    }
    for line in lines {
        // a path per unbroken stretch; a stretch of one run is its point alone
        let mut path = String::new();
        let mut previous = false;
        for (i, v) in line.values.iter().enumerate() {
            match v {
                Some(v) => {
                    path.push_str(&format!(
                        "{}{:.1} {:.1} ",
                        if previous { "L" } else { "M" },
                        x(i),
                        y(*v)
                    ));
                    previous = true;
                }
                None => previous = false,
            }
        }
        let mut group = el("g").class(format!("mj-tone--{}", line.tone));
        if !path.is_empty() {
            group = group.child(
                el("path")
                    .attr("d", path.trim_end())
                    .attr("fill", "none")
                    .attr("stroke", "var(--mj-tone-fg)")
                    .attr("stroke-width", "2"),
            );
        }
        for (i, v) in line.values.iter().enumerate() {
            if let Some(v) = v {
                group = group.child(
                    el("circle")
                        .attr("cx", format!("{:.1}", x(i)))
                        .attr("cy", format!("{:.1}", y(*v)))
                        .attr("r", "3")
                        .attr("fill", "var(--mj-tone-fg)"),
                );
            }
        }
        svg = svg.child(group);
    }
    let legend = el("ul")
        .class("mj-ui-legend")
        .children(lines.iter().map(|l| {
            let latest = l.values.iter().rev().flatten().next();
            el("li")
                .class(format!("mj-ui-legend-item mj-tone--{}", l.tone))
                .child(
                    el("span")
                        .class("mj-ui-legend-dot")
                        .attr("aria-hidden", "true"),
                )
                .child(el("span").class("mj-ui-legend-name").text(l.name))
                .child(el("span").class("mj-ui-legend-value").text(match latest {
                    Some(v) => format!("{v:.1}{unit} latest"),
                    None => "never recorded".into(),
                }))
        }));
    el("div").class("mj-ui-stack").child(svg).child(legend)
}

/// The percentage of one coverage kind of a run, or `None` when the run measured none or
/// the file gave no total.
fn coverage(run: &Value, kind: &str) -> Option<f64> {
    number(run, &format!("/coverage/{kind}/percent"))
}

fn run_row(run: &Value) -> El {
    let cell = |s: String| el("td").class("mj-ui-mono").text(s);
    let sha = text(run, "head_sha");
    el("tr")
        .child(cell(text(run, "run_id")))
        .child(cell(if sha.is_empty() {
            "unknown".into()
        } else {
            sha.chars().take(7).collect()
        }))
        .child(cell(day(run)))
        .child(cell(match text(run, "conclusion").as_str() {
            "" => "unknown".into(),
            c => c.to_string(),
        }))
        .child(cell(minutes(number(run, "/wall_seconds"))))
        .child(cell(minutes(number(run, "/queued_seconds_total"))))
        .child(cell(match run.get("coverage").filter(|c| c.is_object()) {
            None => "no coverage".into(),
            Some(_) => format!(
                "{} · {} · {}",
                percent(coverage(run, "lines")),
                percent(coverage(run, "functions")),
                percent(coverage(run, "regions"))
            ),
        }))
}

fn medians(trends: &Answer) -> El {
    let rows = list(trends, "/job_medians")
        .iter()
        .map(|m| {
            let seconds = |k: &str| {
                m.get(k)
                    .and_then(Value::as_f64)
                    .map_or_else(|| "unknown".into(), |s| duration(s.round() as u64))
            };
            el("tr")
                .child(el("td").class("mj-ui-mono").text(text(m, "name")))
                .child(
                    el("td")
                        .class("mj-ui-mono")
                        .text(seconds("median_run_seconds")),
                )
                .child(
                    el("td")
                        .class("mj-ui-mono")
                        .text(seconds("median_queued_seconds")),
                )
                .child(
                    el("td").class("mj-ui-mono").text(
                        m.get("runs")
                            .and_then(Value::as_u64)
                            .map_or_else(|| "unknown".into(), |n| n.to_string()),
                    ),
                )
        })
        .collect::<Vec<_>>();
    table(
        &["Job", "Median run", "Median queued", "Runs"],
        rows,
        "The series gives no job median.",
    )
}

/// The Trends screen.
pub fn screen(ctx: &crate::capability::Context) -> View {
    let answers = ask_all(ctx, &[TRENDS]);
    let trends = &answers[&TRENDS];
    let recorded = trends.read("/recorded").text().as_deref() == Some("true");
    let reason = trends.read("/reason").text();

    let kpis = if recorded {
        vec![
            kpi_count("Runs", &trends.read("/runs")),
            kpi_count("Jobs measured", &trends.read("/job_medians")),
            kpi("Series recorded", &trends.read("/recorded")),
        ]
    } else {
        let why = reason.as_deref().unwrap_or("no reason was given");
        vec![
            kpi_not_observed("Runs", why, &trends.read("/runs")),
            kpi_not_observed("Jobs measured", why, &trends.read("/job_medians")),
        ]
    };
    let top = hero(
        "chart",
        "violet",
        "Trends",
        "How coverage and CI time move, run after run.",
        "Coverage and time per run of the pages publication's series, read from this checkout when you ask.",
        kpis,
    );

    let body = |draw: &dyn Fn(&[Value]) -> El| {
        unanswered(trends).unwrap_or_else(|| {
            if !recorded {
                return el("div")
                    .class("mj-ui-stack")
                    .child(nothing(NONE_RECORDED))
                    .child(
                        el("p")
                            .class("mj-ui-dim")
                            .text(reason.clone().unwrap_or_default()),
                    );
            }
            draw(
                trends
                    .ok()
                    .and_then(|v| v.get("runs"))
                    .and_then(Value::as_array)
                    .map_or(&[][..], Vec::as_slice),
            )
        })
    };
    let kind = |name: &'static str, tone: &'static str, key: &'static str| {
        move |runs: &[Value]| Line {
            name,
            tone,
            values: runs.iter().map(|r| coverage(r, key)).collect(),
        }
    };
    let coverage_chart = body(&|runs| {
        chart(
            "coverage",
            "%",
            &[
                kind("Lines", "blue", "lines")(runs),
                kind("Functions", "green", "functions")(runs),
                kind("Regions", "violet", "regions")(runs),
            ],
            runs,
            Some(100.0),
        )
    });
    let time_chart = body(&|runs| {
        let per = |key: &str| {
            runs.iter()
                .map(|r| number(r, key).map(|s| s / 60.0))
                .collect::<Vec<_>>()
        };
        chart(
            "wall or queued time",
            " min",
            &[
                Line {
                    name: "Wall time",
                    tone: "orange",
                    values: per("/wall_seconds"),
                },
                Line {
                    name: "Queued time",
                    tone: "cyan",
                    values: per("/queued_seconds_total"),
                },
            ],
            runs,
            None,
        )
    });
    let slowest = body(&|_| medians(trends));
    let runs_table = body(&|runs| {
        table(
            &[
                "Run",
                "Commit",
                "Started",
                "Result",
                "Wall",
                "Queued",
                "Coverage: lines · functions · regions",
            ],
            runs.iter()
                .rev()
                .take(super::dashboard::ROWS * 2)
                .map(run_row)
                .collect(),
            "The series holds no run.",
        )
    });

    let grid = el("div")
        .class("mj-ui-grid")
        .child(panel(
            "Coverage",
            "Percent covered per run: lines, functions and regions. A run that measured none leaves a gap.",
            coverage_chart,
            &[trends],
        ))
        .child(panel(
            "Wall and queued time",
            "Minutes per run, from its start to its end, and the time its jobs waited for a runner.",
            time_chart,
            &[trends],
        ))
        .child(panel(
            "Slowest jobs",
            "The median run and queue time of each job over the series, the slowest first.",
            slowest,
            &[trends],
        ))
        .child(wide(panel(
            "Runs",
            "The newest runs with the figures the lines are drawn from.",
            runs_table,
            &[trends],
        )));

    View::new(
        "Trends",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};

    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test/fixtures/trends/trends.json"
    ));

    fn repository(trends: Option<&str>) -> SyntheticRepository {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        if let Some(body) = trends {
            let path = repo.root().join(crate::delivery::trends::FILE);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            std::fs::write(path, body).expect("a series");
        }
        repo
    }

    /// The route the dispatcher answers, as a request: its status and its page.
    fn get(repo: &SyntheticRepository) -> (u16, String) {
        let ui = crate::ui::Ui::new(repo.context().expect("a context"), "0.0.0-test", None);
        let r = ui.handle(&crate::http::router::Request::parse_target(
            "GET",
            ROUTE,
            Vec::new(),
        ));
        let page = r.body.text().into_owned();
        // the frame draws icons of its own: the screen is what lies inside its main part
        let main = page
            .split("<main")
            .nth(1)
            .and_then(|m| m.split("</main>").next())
            .expect("the page has a main part");
        (r.status, main.to_string())
    }

    /// The series is drawn: coverage as percentages computed from covered over total, the
    /// run that measured none as a gap with no zero point, the time in minutes, and the
    /// slowest job first.
    #[test]
    fn the_series_is_drawn_with_its_percentages_and_its_gap() {
        let (status, html) = get(&repository(Some(FIXTURE)));
        assert_eq!(status, 200);
        assert!(html.contains("<svg"), "{html}");
        for want in [
            "70.0%",
            "75.0%",
            "80.0%",
            "94.0%",
            "87.5%",
            "no coverage",
            "50.0 min",
            "9004",
            "4444444",
            "30 min 01 s",
            "mj-tone--blue",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(!html.contains(NONE_RECORDED), "{html}");
        // the slowest median leads the table
        let rust = html.find(">rust<").expect("rust");
        let lint = html.find(">lint<").expect("lint");
        assert!(rust < lint, "the slowest job is not first");
        // the lines of the coverage chart: lines has four runs but one gap, so its path is
        // broken into a stretch and a stretch (two moves) and never drawn through zero
        let chart = html
            .split("aria-label=\"coverage, ")
            .nth(1)
            .and_then(|c| c.split("</svg>").next())
            .expect("the coverage chart");
        let paths: Vec<&str> = chart
            .split("<path d=\"")
            .skip(1)
            .map(|p| p.split('"').next().unwrap_or_default())
            .collect();
        assert_eq!(paths.len(), 3, "{paths:?}");
        assert_eq!(paths[0].matches('M').count(), 2, "{}", paths[0]);
        assert_eq!(paths[0].matches('L').count(), 1, "{}", paths[0]);
        assert!(
            !html.contains("NaN") && !html.contains("undefined"),
            "{html}"
        );
    }

    /// A checkout the publication never wrote into says so in words: a page, status 200, no
    /// chart and no zero.
    #[test]
    fn an_absent_series_is_said_plainly() {
        let (status, html) = get(&repository(None));
        assert_eq!(status, 200);
        assert!(html.contains(NONE_RECORDED), "{html}");
        assert!(
            html.contains("site/data/trends.json is not in this checkout"),
            "{html}"
        );
        assert!(!html.contains("var(--mj-tone-fg)"), "{html}");
        assert!(html.contains("not observed"), "{html}");
        assert!(html.contains("<h1"), "{html}");
    }

    /// A file that is another schema, or no JSON at all, is not drawn as the series.
    #[test]
    fn a_wrong_schema_is_said_and_not_drawn() {
        for body in [
            r#"{"schema":"trends/v2","runs":[{"run_id":1,"wall_seconds":60}]}"#,
            "not json",
        ] {
            let (status, html) = get(&repository(Some(body)));
            assert_eq!(status, 200, "{body}");
            assert!(html.contains(NONE_RECORDED), "{body}: {html}");
            assert!(!html.contains("var(--mj-tone-fg)"), "{body}: {html}");
        }
        let (_, html) = get(&repository(Some(r#"{"schema":"trends/v2","runs":[]}"#)));
        assert!(html.contains("trends/v2"), "{html}");
    }

    /// A series whose runs measured no coverage draws the time and says no run recorded
    /// coverage, rather than a coverage line at zero.
    #[test]
    fn a_series_without_coverage_draws_no_coverage_line() {
        let (status, html) = get(&repository(Some(
            r#"{"schema":"trends/v1","runs":[{"run_id":1,"started_at":"2026-10-22T00:00:00Z","wall_seconds":600,"coverage":null}],"job_medians":[]}"#,
        )));
        assert_eq!(status, 200);
        assert!(html.contains("No run recorded coverage."), "{html}");
        assert!(html.contains("10.0 min"), "{html}");
        assert!(html.contains("The series gives no job median."), "{html}");
    }

    /// A figure the file does not give reads `unknown` wherever it would have been a number:
    /// a run without a start or a conclusion, a kind without a percentage, a median without
    /// seconds or a run count.
    #[test]
    fn a_figure_the_file_does_not_give_reads_unknown() {
        let (status, html) = get(&repository(Some(
            r#"{"schema":"trends/v1","runs":[
                {"run_id":1,"coverage":{"lines":{"covered":1,"total":0},"functions":{"covered":1,"total":2}}}],
               "job_medians":[{"name":"bare"}]}"#,
        )));
        assert_eq!(status, 200);
        // the run row: no sha, no day, no conclusion, no times; lines have no percentage
        assert!(
            html.contains(">unknown · 50.0% · unknown<"),
            "no coverage cell: {html}"
        );
        assert!(html.matches(">unknown<").count() >= 8, "{html}");
        // the median row: name only, three unknown cells
        let row = html
            .split(">bare<")
            .nth(1)
            .and_then(|r| r.split("</tr>").next())
            .expect("the median row");
        assert_eq!(row.matches(">unknown<").count(), 3, "{row}");
        assert!(!html.contains("NaN"), "{html}");
    }

    /// A chart handed values and no run draws its lines and no day captions.
    #[test]
    fn a_chart_of_values_without_runs_draws_no_day_caption() {
        let lines = [Line {
            name: "Lines",
            tone: "blue",
            values: vec![Some(50.0)],
        }];
        let html = chart("coverage", "%", &lines, &[], Some(100.0)).render();
        assert!(html.contains("<circle"), "{html}");
        assert!(html.contains("50.0% latest"), "{html}");
        assert!(!html.contains("text-anchor=\"start\""), "{html}");
        assert!(!html.contains("text-anchor=\"middle\""), "{html}");
        // a line without any value is named, and never drawn
        let empty = [Line {
            name: "Lines",
            tone: "blue",
            values: vec![],
        }];
        assert!(chart("coverage", "%", &empty, &[], None)
            .render()
            .contains("No run recorded coverage."));
    }

    #[test]
    fn one_run_is_captioned_once_and_more_runs_at_both_ends() {
        let one = [Line {
            name: "Lines",
            tone: "blue",
            values: vec![Some(50.0)],
        }];
        let run = |day: &str| serde_json::json!({ "started_at": format!("{day}T10:00:00Z") });
        let only = run("2026-10-20");
        let html = chart(
            "coverage",
            "%",
            &one,
            std::slice::from_ref(&only),
            Some(100.0),
        )
        .render();
        // the caption the chart draws is day(run); one run must carry it exactly once
        assert_eq!(
            html.matches(&format!(">{}<", day(&only))).count(),
            1,
            "{html}"
        );
        assert_eq!(html.matches("text-anchor=\"middle\"").count(), 1, "{html}");
        assert!(!html.contains("text-anchor=\"start\""), "{html}");
        let two = [Line {
            name: "Lines",
            tone: "blue",
            values: vec![Some(40.0), Some(50.0)],
        }];
        let runs = [run("2026-10-20"), run("2026-10-22")];
        let html = chart("coverage", "%", &two, &runs, Some(100.0)).render();
        assert_eq!(html.matches("text-anchor=\"start\"").count(), 1, "{html}");
        assert!(!html.contains("text-anchor=\"middle\""), "{html}");
    }
}
