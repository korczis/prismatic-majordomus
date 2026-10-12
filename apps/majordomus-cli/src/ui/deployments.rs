//! Deployments: where the product runs and whether what is live is what was built. Read
//! from `deploy.list` (what the layer declares) and `deploy.observed` (what the forge
//! recorded: every deployment to every environment, and every release); the delivery report
//! (`delivery.report`), which probes the public site and judges every feature's delivery, is
//! slow enough to be a panel of its own.
//!
//! Every commit, ref, environment, run and release here is a link, built from the
//! repository's address as the forge itself gave it — no host is written in this file.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask, ask_all, Answer, Source};
use super::view::{
    anchor, hero, kpi, kpi_count, link, list, none, nothing, object_if, panel, panel_to, state,
    strip, table, text, unanswered, value, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/deployments";

/// Where one declared deployment is: its row on this screen.
pub fn href(id: &str) -> String {
    format!("{ROUTE}#deployment-{}", anchor(id))
}

/// One declared deployment as a row: its id leading to its row here, its declaration's
/// technical view, the application it runs and its state. The CI/CD screen draws the same row.
pub fn summary_row(ctx: &Context, d: &Value) -> El {
    let id = text(d, "id");
    el("tr")
        .child(
            el("td")
                .child(
                    el("a")
                        .class("mj-ui-row-link")
                        .attr("href", href(&id))
                        .child(
                            el("span")
                                .class("mj-ui-mono mj-ui-dim")
                                .text(value(d, "id")),
                        )
                        .child(el("span").class("mj-ui-cell-title").text(value(d, "title"))),
                )
                .child(object_if(
                    ctx,
                    &format!("majordomus://deployment/{id}"),
                    &text(d, "file"),
                )),
        )
        .child(el("td").class("mj-ui-mono").text(value(d, "application")))
        .child(el("td").child(state(d, "status")))
}

/// The route of the delivery panel.
pub const DELIVERY_PANEL: &str = "/ui/panels/delivery";

const DEPLOYMENTS: Source = Source::of("deploy.list");
const OBSERVED: Source = Source::of("deploy.observed");

/// A link that leaves the UI: the forge, a run, a served environment.
fn outside(href: &str, label: &str) -> El {
    el("a")
        .class("mj-ui-link")
        .attr("href", href)
        .attr("rel", "noopener noreferrer")
        .text(label)
}

/// A field of an entry as a link out, or `none` when the entry has no such field.
fn outside_field(entry: &Value, key: &str, label: &str) -> El {
    match text(entry, key) {
        href if href.is_empty() => none(),
        href => outside(&href, label),
    }
}

/// A commit as its short name, leading to the commit on the forge.
fn commit(repository_url: &str, sha: &str) -> El {
    let short = &sha[..sha.len().min(10)];
    el("span")
        .class("mj-ui-mono")
        .child(outside(&format!("{repository_url}/commit/{sha}"), short))
}

/// The three tables of what the forge recorded: where each environment stands, every
/// deployment observed, and every release. A checkout that has never asked the forge shows
/// the reason in their place — the command that asks is in it.
fn observed_panels(observed: &Answer) -> Vec<El> {
    if let Some(unread) = unanswered(observed) {
        return vec![unread];
    }
    let root = observed.ok().cloned().unwrap_or(Value::Null);
    if root.get("observed").and_then(Value::as_bool) != Some(true) {
        return vec![nothing(&value(&root, "reason"))];
    }
    let forge = text(&root, "repository_url");
    let environments = list(observed, "/environments")
        .iter()
        .map(|e| {
            el("tr")
                .child(el("td").child(outside(&text(e, "activity_url"), &text(e, "name"))))
                .child(el("td").child(state(e, "state")))
                .child(el("td").child(commit(&forge, &text(e, "sha"))))
                .child(el("td").class("mj-ui-mono").text(value(e, "deployed_at")))
                .child(el("td").child(outside_field(e, "url", &text(e, "url"))))
                .child(
                    el("td").class("mj-ui-mono").text(
                        e.get("deployments")
                            .and_then(Value::as_u64)
                            .unwrap_or(0)
                            .to_string(),
                    ),
                )
        })
        .collect::<Vec<_>>();
    let deployments = list(observed, "/deployments")
        .iter()
        .map(|d| {
            let git_ref = text(d, "git_ref");
            el("tr")
                .child(el("td").class("mj-ui-mono").text(value(d, "created_at")))
                .child(el("td").class("mj-ui-mono").text(value(d, "environment")))
                .child(el("td").child(state(d, "state")))
                .child(el("td").child(commit(&forge, &text(d, "sha"))))
                .child(
                    el("td")
                        .class("mj-ui-mono")
                        .child(match git_ref.is_empty() {
                            true => none(),
                            false => outside(&format!("{forge}/tree/{git_ref}"), &git_ref),
                        }),
                )
                .child(el("td").class("mj-ui-mono").text(value(d, "creator")))
                .child(el("td").child(outside_field(d, "log_url", "the run")))
        })
        .collect::<Vec<_>>();
    let releases = list(observed, "/releases")
        .iter()
        .map(|r| {
            let kind = match (
                r.get("draft").and_then(Value::as_bool) == Some(true),
                r.get("prerelease").and_then(Value::as_bool) == Some(true),
            ) {
                (true, _) => "draft",
                (false, true) => "pre-release",
                (false, false) => "release",
            };
            el("tr")
                .child(
                    el("td")
                        .class("mj-ui-mono")
                        .child(outside(&text(r, "url"), &text(r, "tag"))),
                )
                .child(el("td").text(value(r, "name")))
                .child(el("td").class("mj-ui-mono").text(value(r, "published_at")))
                .child(el("td").text(kind))
        })
        .collect::<Vec<_>>();
    vec![
        el("h3")
            .class("mj-ui-panel-title")
            .text("Where each environment stands"),
        table(
            &[
                "Environment",
                "State",
                "Commit",
                "Deployed",
                "Served at",
                "Deployments",
            ],
            environments,
            "The forge has recorded no deployment.",
        ),
        el("h3")
            .class("mj-ui-panel-title")
            .text("Every deployment observed"),
        table(
            &[
                "Deployed",
                "Environment",
                "State",
                "Commit",
                "Ref",
                "By",
                "Run",
            ],
            deployments,
            "The forge has recorded no deployment.",
        ),
        el("h3").class("mj-ui-panel-title").text("Releases"),
        table(
            &["Tag", "Title", "Published", "Kind"],
            releases,
            "The forge has published no release.",
        ),
    ]
}
const DELIVERY: Source = Source::of("delivery.report");

/// The Deployments screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[DEPLOYMENTS, OBSERVED]);
    let deployments = &answers[&DEPLOYMENTS];
    let observed = &answers[&OBSERVED];

    let top = hero(
        "server",
        "green",
        "Deployments",
        "What is live, and whether it is what was built.",
        "Every declared deployment, every deployment and release the forge recorded, and — asked on its own — whether the public site serves the trunk and how far each feature has been delivered.",
        vec![
            kpi("Deployments", &deployments.read("/count")),
            kpi_count("Declared", &deployments.read("/deployments")),
            kpi_count("Environments", &observed.read("/environments")),
            kpi_count("Observed", &observed.read("/deployments")),
            kpi_count("Releases", &observed.read("/releases")),
        ],
    );

    let declared = unanswered(deployments).unwrap_or_else(|| {
        let rows = list(deployments, "/deployments")
            .iter()
            .map(|d| {
                summary_row(ctx, d).attr("id", format!("deployment-{}", anchor(&text(d, "id"))))
            })
            .collect::<Vec<_>>();
        table(
            &["Deployment", "Application", "State"],
            rows,
            "No deployment is declared.",
        )
    });

    let placeholder = el("section")
        .class("mj-ui-panel")
        .attr("aria-label", "Delivery")
        .attr("data-panel-src", DELIVERY_PANEL)
        .attr("aria-busy", "false")
        .child(
            el("header").class("mj-ui-panel-head").child(
                el("div")
                    .class("mj-ui-panel-heading")
                    .child(el("h2").class("mj-ui-panel-title").text("Delivery"))
                    .child(el("p").class("mj-ui-panel-sub").text(
                        "Asked on its own: delivery.report probes the public site and judges every feature, which would hold this screen back.",
                    )),
            ),
        )
        .child(
            el("p").class("mj-ui-panel-body").child(
                el("a").class("mj-ui-link").attr("href", DELIVERY_PANEL).text("Open the delivery report"),
            ),
        );

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(panel(
            "Declared deployments",
            "Where the product is declared to run.",
            declared,
            &[deployments],
        )))
        .child(wide(panel(
            "What was deployed",
            "Every deployment the forge recorded, to every environment, and every release — as it was last observed.",
            observed_panels(observed)
                .into_iter()
                .fold(el("div").class("mj-ui-stack"), El::child),
            &[observed],
        )))
        .child(wide(placeholder));

    View::new(
        "Deployments",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// The delivery report as a panel: the publication's verification and every feature's stage.
pub fn delivery_panel(ctx: &Context) -> View {
    View::new("Delivery", delivery(ctx, &ask(ctx, DELIVERY)))
}

fn delivery(ctx: &Context, report: &Answer) -> El {
    let body = unanswered(report).unwrap_or_else(|| {
        let tallies = strip(vec![
            kpi("Features", &report.read("/tallies/features")),
            kpi("On the trunk", &report.read("/tallies/on_master")),
            kpi("Deployed", &report.read("/tallies/deployed")),
            kpi(
                "Publicly verified",
                &report.read("/tallies/publicly_verified"),
            ),
        ]);
        let url = report.read("/publication/url").text();
        let publication = el("p")
            .class("mj-ui-line")
            .child(state(
                report
                    .ok()
                    .and_then(|v| v.pointer("/publication/verification"))
                    .unwrap_or(&Value::Null),
                "state",
            ))
            .text(" ")
            .child(match &url {
                // the address the report probed, as it gave it
                Some(u) => link(u.clone(), u.clone()),
                None => el("span").text("no public address"),
            })
            .text(format!(
                " — {}",
                report
                    .read("/publication/verification/detail")
                    .text()
                    .unwrap_or_else(|| "none".into())
            ));
        let rows = list(report, "/features")
            .iter()
            .map(|f| {
                let blocking: Vec<String> = f
                    .pointer("/delivery/blocking")
                    .and_then(Value::as_array)
                    .map(|b| {
                        b.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let id = text(f, "id");
                el("tr")
                    .child(
                        el("td")
                            .child(object_if(ctx, &format!("majordomus://feature/{id}"), &id))
                            .child(el("span").class("mj-ui-cell-title").text(value(f, "title"))),
                    )
                    .child(
                        el("td").child(state(f.get("delivery").unwrap_or(&Value::Null), "stage")),
                    )
                    .child(
                        el("td").child(state(f.get("delivery").unwrap_or(&Value::Null), "state")),
                    )
                    .child(el("td").child(if blocking.is_empty() {
                        none()
                    } else {
                        el("span")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(blocking.join(", "))
                    }))
            })
            .collect::<Vec<_>>();
        el("div")
            .class("mj-ui-stack")
            .child(tallies)
            .child(publication)
            .child(table(
                &["Feature", "Stage", "Delivery", "What blocks it"],
                rows,
                "No feature is declared.",
            ))
    });
    panel_to(
        "Delivery",
        "Whether the public site serves the trunk, and how far each feature has been delivered.",
        None,
        body,
        &[report],
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;
    use serde_json::json;
    use std::time::Duration;

    /// The sources a furnished repository adds to the synthetic one's: the objects the
    /// screens draw rows from.
    const FURNISHED_SOURCES: &str = "
  - id: deployment
    kind: deployment
    discovery: vcs
    pathspec: ':(glob).ai/repo/deployments/*.yaml'
    required: false
  - id: feature
    kind: feature
    discovery: vcs
    pathspec: ':(glob).ai/repo/features/*.md'
    required: false
  - id: claims
    kind: claim
    discovery: vcs
    pathspec: ':(glob)docs/CLAIMS.yaml'
    required: false
";

    /// A synthetic repository furnished with what the screens draw rows from: this
    /// repository's own deployment, one feature, two claims (one of them a guarantee no
    /// test supports, so the report has a finding) and a CI model with a gate of each kind
    /// of selection.
    pub(crate) fn furnished() -> SyntheticRepository {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let write = |rel: &str, body: &str| {
            let path = repo.root().join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            std::fs::write(path, body).expect("a file");
        };
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).expect("the sources");
        text.push_str(FURNISHED_SOURCES);
        std::fs::write(&sources, text).expect("the sources");
        let deployment = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../.ai/repo/deployments/majordomus.yaml"
        ))
        .expect("this repository's deployment");
        write(".ai/repo/deployments/majordomus.yaml", &deployment);
        write(
            ".ai/repo/features/synthetic.md",
            "---\nschema: feature/v1\nid: synthetic\nkind: feature\ntitle: A synthetic feature\n\
             headline: A feature that exists for a test.\n\
             summary: What a synthetic feature does.\nstatus: stable\n---\n\n## What it does\n\nIt is a fixture.\n",
        );
        write(
            "docs/CLAIMS.yaml",
            "version: 1\nclaims:\n  - id: synthetic-guarantee\n    claim: A guarantee no test supports\n\
             \x20   source: docs/DOC_0.md\n    status: guaranteed\n\
             \x20 - id: synthetic-advice\n    claim: An advisory claim\n    source: docs/DOC_0.md\n\
             \x20   test: test/cases/1_synthetic.sh\n    status: advisory\n",
        );
        write(
            ".ai/repo/ci/gates.yaml",
            "version: 1\ngates:\n  - id: every-plan\n    job: structure\n    always: true\n\
             \x20   runs: scripts/ci/every-plan\n    summary: runs in every plan\n\
             \x20 - id: when-asked\n    job: bench\n    on-demand: true\n    runs: scripts/ci/when-asked\n\
             \x20   summary: runs when asked\n  - id: by-path\n    job: rust\n    runs: scripts/ci/by-path\n\
             \x20   summary: runs when its paths change\nclasses:\n  - id: rust\n    paths: [lib/**]\n\
             \x20   gates: [by-path]\n",
        );
        repo
    }

    /// An answer as a capability would give it, built here so every branch of a panel can
    /// be drawn from rows the fixture repository does not hold.
    pub(crate) fn answer(source: Source, value: Result<Value, String>) -> Answer {
        Answer {
            source,
            route: None,
            exposure: Default::default(),
            value,
            took: Duration::ZERO,
            standing: crate::ui::source::Standing::NOW,
        }
    }

    /// What the forge recorded is drawn as rows whose every commit, ref, environment, run
    /// and release is a link built from the address the forge gave — and a checkout that has
    /// never asked says what to run instead of drawing empty tables.
    #[test]
    fn what_the_forge_recorded_is_rows_of_links_and_never_asked_is_said() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let unasked = screen(&ctx).main.render();
        assert!(unasked.contains("majordomus prs refresh"), "{unasked}");
        assert!(
            !unasked.contains("Where each environment stands"),
            "{unasked}"
        );

        let observation = crate::deploy::observed::parse(
            "o/r",
            "2026-10-12T14:00:00Z",
            &json!({"data": {"repository": {"url": "https://forge.example/o/r",
            "deployments": {"nodes": [
                {"databaseId": 7, "environment": "github-pages", "createdAt": "2026-10-12T10:00:00Z",
                 "commitOid": "0123456789abcdef", "ref": {"name": "gh-pages"}, "creator": {"login": "bot"},
                 "latestStatus": {"state": "SUCCESS", "environmentUrl": "https://site.example/",
                                  "logUrl": "https://forge.example/o/r/actions/runs/1"}},
                {"databaseId": 8, "environment": "production", "createdAt": "2026-10-12T11:00:00Z",
                 "commitOid": "fedcba9876543210"}
            ]},
            "releases": {"nodes": [
                {"tagName": "v1.0.0", "name": "One", "publishedAt": "2026-10-12T09:00:00Z",
                 "url": "https://forge.example/o/r/releases/tag/v1.0.0", "isPrerelease": false, "isDraft": false},
                {"tagName": "v1.1.0-rc1", "url": "https://forge.example/o/r/releases/tag/v1.1.0-rc1",
                 "isPrerelease": true, "isDraft": false},
                {"tagName": "v2.0.0", "url": "https://forge.example/o/r/releases/tag/v2.0.0",
                 "isPrerelease": false, "isDraft": true}
            ]}}}}),
        );
        crate::deploy::observed::store(repo.root(), &observation).expect("stored");
        // a context of its own: the first one remembers its reading of the source, as a
        // running server does until the reading is renewed
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        for expected in [
            "Where each environment stands",
            "href=\"https://forge.example/o/r/deployments/github-pages\"",
            "href=\"https://forge.example/o/r/commit/0123456789abcdef\"",
            ">0123456789<",
            "href=\"https://forge.example/o/r/tree/gh-pages\"",
            "href=\"https://site.example/\"",
            "href=\"https://forge.example/o/r/actions/runs/1\"",
            "href=\"https://forge.example/o/r/releases/tag/v1.0.0\"",
            ">pre-release<",
            ">draft<",
            ">release<",
        ] {
            assert!(html.contains(expected), "{expected} is missing from {html}");
        }
        // a deployment with no status, no ref and no run says so, cell by cell
        assert!(html.contains("unknown"), "{html}");
    }

    /// A source that did not answer is drawn as unanswered, not as "never asked".
    #[test]
    fn an_unanswered_observation_is_drawn_as_unanswered() {
        let drawn = observed_panels(&answer(OBSERVED, Err("no such capability".into())));
        assert_eq!(drawn.len(), 1);
        assert!(drawn[0].render().contains("no such capability"));
    }

    /// Every declared deployment is a row: its identity, its title, the application it
    /// ships and its own status word.
    #[test]
    fn every_declared_deployment_is_a_row_with_its_own_status() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let view = screen(&ctx);
        let html = view.main.render();
        assert_eq!(view.status, 200);
        assert!(
            html.contains("The Majordomus layer, served from its container image"),
            "{html}"
        );
        assert!(html.contains(">majordomus<"), "{html}");
        assert!(html.contains("declared"), "{html}");
        // the row is where a link to the deployment leads, and its declaration is the
        // technical view of the object the index holds
        assert!(html.contains("id=\"deployment-majordomus\""), "{html}");
        assert!(
            html.contains(&format!("href=\"{}\"", href("majordomus"))),
            "{html}"
        );
        assert!(
            html.contains("/cockpit/object?uri=majordomus%3A%2F%2Fdeployment%2Fmajordomus"),
            "{html}"
        );
        assert!(!html.contains("No deployment is declared."), "{html}");
        assert!(
            html.contains(&format!("data-panel-src=\"{DELIVERY_PANEL}\"")),
            "the slow report is a panel of its own: {html}"
        );
    }

    /// The panel route asks the report itself; a repository with no public address says
    /// so rather than inventing one.
    #[test]
    fn the_delivery_panel_asks_the_report_and_says_when_there_is_no_public_address() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let view = delivery_panel(&ctx);
        let html = view.main.render();
        assert_eq!(view.title, "Delivery");
        assert!(html.contains("no public address"), "{html}");
        assert!(html.contains("unmeasured"), "{html}");
        assert!(
            html.contains("delivery.report"),
            "the panel names its source: {html}"
        );
    }

    /// A report with a publication and features draws the tallies, the verification with
    /// its detail, and every feature with its stage, its state and what blocks it; a
    /// feature that carries no delivery is `unknown`, never delivered.
    #[test]
    fn a_report_draws_every_feature_with_its_stage_state_and_blockers() {
        let report = answer(
            DELIVERY,
            Ok(json!({
                "publication": {
                    "url": "https://example.test/",
                    "verification": { "state": "verified", "detail": "the site serves the trunk" }
                },
                "tallies": { "features": 2, "on_master": 2, "deployed": 1, "publicly_verified": 1 },
                "features": [
                    { "id": "ci", "title": "CI from one model",
                      "delivery": { "stage": "deployed", "state": "partial", "blocking": ["publication", 7, "site"] } },
                    { "id": "bare", "title": "A feature with no delivery" }
                ]
            })),
        );
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = delivery(&ctx, &report).render();
        assert!(
            html.contains(
                "href=\"https://example.test/\">https://example.test/</a> — the site serves the trunk"
            ),
            "the address the report probed is a link: {html}"
        );
        assert!(html.contains("verified"), "{html}");
        assert!(
            html.contains("CI from one model") && html.contains("deployed"),
            "{html}"
        );
        assert!(
            html.contains("publication, site"),
            "only the words block: {html}"
        );
        assert!(html.contains("A feature with no delivery"), "{html}");
        assert!(html.contains("unknown"), "{html}");
        // a feature with nothing blocking it says so; the fixture's index holds neither
        // feature, so each id is text rather than a link to a page that would answer 404
        assert!(
            html.contains("<span class=\"mj-ui-dim\">none</span>"),
            "{html}"
        );
        assert!(
            !html.contains("/cockpit/object?uri=majordomus%3A%2F%2Ffeature%2Fci"),
            "{html}"
        );
        assert!(!html.contains("No feature is declared."), "{html}");
    }

    /// A report that could not be made is said to be unanswered, with its reason.
    #[test]
    fn a_report_that_could_not_be_made_says_why() {
        let report = answer(DELIVERY, Err("the probe timed out".into()));
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = delivery(&ctx, &report).render();
        assert!(
            html.contains("delivery.report could not answer: the probe timed out"),
            "{html}"
        );
    }
}
