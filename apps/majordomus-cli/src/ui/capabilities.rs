//! Capabilities: what the product can be asked, and how. Read from `repository.info` (the
//! registry's own tallies) and `capabilities.list` filtered by kind; one capability in full
//! from `capabilities.describe` when the reader picks it (`?id=`).
//!
//! The queries and commands are one browser, narrowed by what the reader picks: a kind, a
//! module, a stability, a surface it is projected on, text. Each facet counts what it would
//! show among what the other picks already admit, so a choice never leads to an empty list
//! without saying so first. Every control is a link or a GET form, and a value a link carries
//! that is not a word of the registry is dropped, never echoed.
//!
//! The resources of the layer — one per object — are counted here and listed nowhere: they
//! are the objects, and the Documentation screen is where a reader meets them.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask_all, ask_input, Answer, Source};
use super::view::{
    counts, donut, hero, kpi, list, nothing, panel, state, table, text, unanswered, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/capabilities";

const REPOSITORY: Source = Source::of("repository.info");
const COMMANDS: Source = Source::with("capabilities.list", r#"{"kind":"command"}"#);
const QUERIES: Source = Source::with("capabilities.list", r#"{"kind":"query"}"#);

/// The surfaces a capability can be projected on, as `exposure` names them, and as a reader
/// reads them.
const SURFACES: &[(&str, &str)] = &[("mcp", "MCP"), ("http", "HTTP"), ("cli", "CLI")];

/// The query parameters this screen reads, in the order its links write them.
const KEYS: &[&str] = &["kind", "module", "stability", "surface", "q", "id"];

/// What the reader picked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pick {
    /// `query` or `command`.
    pub kind: Option<String>,
    /// A module id (`artifacts`).
    pub module: Option<String>,
    /// A stability (`behaviorally_verified`).
    pub stability: Option<String>,
    /// A surface it must be projected on: `mcp`, `http` or `cli`.
    pub surface: Option<String>,
    /// Text an id, title, description, module or tag must contain.
    pub q: Option<String>,
    /// The capability shown in full.
    pub id: Option<String>,
}

/// A word of the registry: letters, digits, `.`, `-` and `_`, as its ids, modules, kinds
/// and stabilities are.
fn is_word(w: &str) -> bool {
    !w.is_empty()
        && w.len() <= 96
        && w.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

impl Pick {
    /// The pick a query names; a value that is not a word of the registry is dropped, and a
    /// search is cut to 80 characters.
    pub fn from_query(query: &[(String, String)]) -> Self {
        let get = |k: &str| {
            query
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let word = |k: &str| get(k).filter(|w| is_word(w));
        Pick {
            kind: word("kind"),
            module: word("module"),
            stability: word("stability"),
            surface: word("surface").filter(|s| SURFACES.iter().any(|(k, _)| k == s)),
            q: get("q").map(|q| q.chars().take(80).collect()),
            id: word("id"),
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        match key {
            "kind" => self.kind.as_deref(),
            "module" => self.module.as_deref(),
            "stability" => self.stability.as_deref(),
            "surface" => self.surface.as_deref(),
            "q" => self.q.as_deref(),
            "id" => self.id.as_deref(),
            _ => None,
        }
    }

    /// This pick with `key` set to `value`, or cleared, as a link to this screen. A change
    /// of the filters closes the capability shown in full; picking one keeps the filters.
    pub fn href(&self, key: &str, value: Option<&str>) -> String {
        let pairs: Vec<String> = KEYS
            .iter()
            .filter_map(|k| {
                let v = if *k == key {
                    value
                } else if *k == "id" {
                    None
                } else {
                    self.get(k)
                };
                v.map(|v| format!("{k}={}", encode(v)))
            })
            .collect();
        if pairs.is_empty() {
            ROUTE.to_string()
        } else {
            format!("{ROUTE}?{}", pairs.join("&"))
        }
    }

    /// `nofollow` for the link [`Pick::href`] draws when the page it leads to is narrowed.
    /// Such a page is one more view of rows the unnarrowed screen lists, and every
    /// combination of facets, each with every row it admits, is an address of its own:
    /// thousands over the executable's registry. A crawler reads each declaration once, at
    /// its own `?id=`, and no view (`tests/ui_links.rs`, ADR 0135).
    fn rel(&self, key: &str, value: Option<&str>) -> Option<&'static str> {
        let narrows = KEYS.iter().filter(|k| **k != "id").any(|k| {
            if *k == key {
                value.is_some()
            } else {
                self.get(k).is_some()
            }
        });
        narrows.then_some("nofollow")
    }

    /// Is any filter picked (the capability shown in full is no filter)?
    fn narrows(&self) -> bool {
        Pick {
            id: None,
            ..self.clone()
        } != Pick::default()
    }

    /// Does `c` pass every pick but `except`'s? A facet counts with its own pick left out,
    /// so each of its values says what choosing it would show.
    fn admits_except(&self, c: &Value, except: &str) -> bool {
        let is = |k: &str, want: &Option<String>| {
            k == except
                || want
                    .as_deref()
                    .is_none_or(|w| text(c, k).eq_ignore_ascii_case(w))
        };
        let exposed = except == "surface"
            || self
                .surface
                .as_deref()
                .is_none_or(|s| surfaces(c).contains(&s));
        let found = except == "q"
            || self.q.as_deref().is_none_or(|q| {
                let q = q.to_lowercase();
                ["id", "title", "description", "module"]
                    .iter()
                    .map(|k| text(c, k))
                    .chain(strings(c, "tags"))
                    .any(|t| t.to_lowercase().contains(&q))
            });
        is("kind", &self.kind)
            && is("module", &self.module)
            && is("stability", &self.stability)
            && exposed
            && found
    }

    /// Does `c` pass the pick?
    pub fn admits(&self, c: &Value) -> bool {
        self.admits_except(c, "")
    }
}

fn encode(v: &str) -> String {
    v.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn strings(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The surfaces `c` is projected on, by `exposure`'s own names.
fn surfaces(c: &Value) -> Vec<&'static str> {
    SURFACES
        .iter()
        .filter(|(k, _)| {
            c.pointer(&format!("/exposure/{k}"))
                .is_some_and(|v| !v.is_null())
        })
        .map(|(k, _)| *k)
        .collect()
}

/// The values of facet `key` among the capabilities every other pick admits, each with how
/// many it would show.
fn facet(all: &[Value], pick: &Pick, key: &str) -> Vec<(String, u64)> {
    let mut t: BTreeMap<String, u64> = BTreeMap::new();
    for c in all.iter().filter(|c| pick.admits_except(c, key)) {
        let values = if key == "surface" {
            surfaces(c).into_iter().map(str::to_string).collect()
        } else {
            vec![text(c, key)]
        };
        for v in values.into_iter().filter(|v| !v.is_empty()) {
            *t.entry(v).or_default() += 1;
        }
    }
    t.into_iter().collect()
}

/// The Capabilities screen; `query` may narrow the browser and name one capability to show
/// in full.
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let pick = Pick::from_query(query);
    let answers = ask_all(ctx, &[REPOSITORY, COMMANDS, QUERIES]);
    let (repo, commands, queries) = (
        &answers[&REPOSITORY],
        &answers[&COMMANDS],
        &answers[&QUERIES],
    );

    let top = hero(
        "puzzle",
        "violet",
        "Capabilities",
        "Everything the product can be asked, once, from every surface.",
        "Each capability is declared once and projected onto the API, MCP and the command line; the UI asks the same declarations.",
        vec![
            kpi("Capabilities", &repo.read("/capabilities/total")),
            kpi("Built in", &repo.read("/capabilities/builtin")),
            kpi("HTTP routes", &repo.read("/capabilities/http_routes")),
            kpi("MCP tools", &repo.read("/capabilities/mcp_tools")),
            kpi("CLI commands", &repo.read("/capabilities/cli_commands")),
            kpi("Modules", &repo.read("/capabilities/modules")),
        ],
    );

    let mut grid = el("div").class("mj-ui-grid");
    if let Some(id) = &pick.id {
        grid = grid.child(wide(detail(ctx, id)));
    }
    grid = grid
        .child(wide(browser(commands, queries, &pick)))
        .child(panel(
            "By kind",
            "Queries read, commands change, resources are the objects of the layer.",
            unanswered(repo)
                .unwrap_or_else(|| donut("capabilities", &counts(repo, "/capabilities/by_kind"))),
            &[repo],
        ))
        .child(panel(
            "By stability",
            "How far each capability's behaviour is proven.",
            unanswered(repo).unwrap_or_else(|| {
                donut("capabilities", &counts(repo, "/capabilities/by_stability"))
            }),
            &[repo],
        ));

    View::new(
        "Capabilities",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// The browser: the kind tabs, the facets, the search, and the queries and commands the pick
/// admits, each opening its declaration in full.
pub fn browser(commands: &Answer, queries: &Answer, pick: &Pick) -> El {
    let body = unanswered(commands)
        .or_else(|| unanswered(queries))
        .unwrap_or_else(|| {
            let mut all = list(queries, "/capabilities");
            all.extend(list(commands, "/capabilities"));
            let shown: Vec<&Value> = all.iter().filter(|c| pick.admits(c)).collect();
            let tab = |label: String, kind: Option<&str>, current: bool| {
                el("a")
                    .class("mj-ui-tab")
                    .attr("href", pick.href("kind", kind))
                    .attr_if("rel", pick.rel("kind", kind))
                    .attr_if("aria-current", current.then_some("page"))
                    .text(label)
            };
            let kinds = facet(&all, pick, "kind");
            let tabs = el("nav")
                .class("mj-ui-tabs")
                .attr("aria-label", "Which capabilities")
                .child(tab(
                    format!("All {}", kinds.iter().map(|(_, n)| n).sum::<u64>()),
                    None,
                    pick.kind.is_none(),
                ))
                .children(kinds.into_iter().map(|(k, n)| {
                    let current = pick.kind.as_deref() == Some(k.as_str());
                    tab(format!("{k} {n}"), Some(&k), current)
                }));
            let select = |name: &str, label: &str, options: Vec<(String, u64)>| {
                let picked = pick.get(name);
                el("label")
                    .class("mj-ui-line")
                    .child(el("span").class("mj-ui-dim").text(format!("{label} ")))
                    .child(
                        el("select")
                            .attr("name", name)
                            .child(el("option").attr("value", "").text("any"))
                            .children(options.into_iter().map(|(value, n)| {
                                let shown = SURFACES
                                    .iter()
                                    .find(|(k, _)| *k == value)
                                    .map_or(value.clone(), |(_, s)| (*s).to_string());
                                el("option")
                                    .attr_if(
                                        "selected",
                                        (picked == Some(value.as_str())).then_some("selected"),
                                    )
                                    .attr("value", value)
                                    .text(format!("{shown} ({n})"))
                            })),
                    )
            };
            let form = el("form")
                .class("mj-ui-search")
                .attr("role", "search")
                .attr("method", "get")
                .attr("action", ROUTE)
                .child(
                    el("label")
                        .class("mj-sr-only")
                        .attr("for", "mj-ui-capabilities-q")
                        .text("Search capabilities"),
                )
                .child(
                    el("input")
                        .attr("id", "mj-ui-capabilities-q")
                        .attr("type", "search")
                        .attr("name", "q")
                        .attr("placeholder", "Id, title, description or tag")
                        .attr("value", pick.q.clone().unwrap_or_default()),
                )
                .children(pick.kind.clone().map(|k| {
                    el("input")
                        .attr("type", "hidden")
                        .attr("name", "kind")
                        .attr("value", k)
                }))
                .child(select("module", "Module", facet(&all, pick, "module")))
                .child(select(
                    "stability",
                    "Stability",
                    facet(&all, pick, "stability"),
                ))
                .child(select("surface", "Surface", facet(&all, pick, "surface")))
                .child(
                    el("button")
                        .attr("type", "submit")
                        .class("mj-ui-more")
                        .text("Apply"),
                )
                .when(pick.narrows(), |f| {
                    f.child(
                        el("a")
                            .class("mj-ui-more")
                            .attr("href", ROUTE)
                            .text("Clear"),
                    )
                });
            let rows = shown
                .iter()
                .map(|c| {
                    let id = text(c, "id");
                    let module = text(c, "module");
                    let on = surfaces(c);
                    el("tr")
                        .attr("id", format!("capability-{id}"))
                        .child(
                            el("td").child(
                                el("a")
                                    .class("mj-ui-row-link")
                                    .attr("href", format!("{}#detail", pick.href("id", Some(&id))))
                                    .attr_if("rel", pick.rel("id", Some(&id)))
                                    .child(
                                        el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()),
                                    )
                                    .child(
                                        el("span").class("mj-ui-cell-title").text(text(c, "title")),
                                    ),
                            ),
                        )
                        .child(el("td").child(state(c, "kind")))
                        .child(
                            el("td").child(
                                el("a")
                                    .class("mj-ui-mono")
                                    .attr("href", pick.href("module", Some(&module)))
                                    .attr_if("rel", pick.rel("module", Some(&module)))
                                    .text(module.clone()),
                            ),
                        )
                        .child(el("td").child(if on.is_empty() {
                            el("span").class("mj-ui-dim").text("none")
                        } else {
                            el("span").class("mj-ui-badges").children(
                                SURFACES.iter().filter(|(k, _)| on.contains(k)).map(
                                    |(k, label)| {
                                        el("a")
                                            .class("mj-ui-chip mj-ui-chip--link")
                                            .attr("href", pick.href("surface", Some(k)))
                                            .attr_if("rel", pick.rel("surface", Some(k)))
                                            .child(
                                                el("code").class("mj-ui-chip-value").text(*label),
                                            )
                                    },
                                ),
                            )
                        }))
                        .child(el("td").child(state(c, "stability")))
                })
                .collect::<Vec<_>>();
            el("div")
                .class("mj-ui-stack")
                .child(tabs)
                .child(form)
                .child(table(
                    &["Capability", "Kind", "Module", "Surfaces", "Stability"],
                    rows,
                    "No query or command matches.",
                ))
                .child(el("p").class("mj-ui-line").text(format!(
                    "{} of {} capabilities shown",
                    shown.len(),
                    all.len()
                )))
        });
    panel(
        "Queries and commands",
        "What the product can be asked and told, narrowed by kind, module, stability, surface or text.",
        body,
        &[queries, commands],
    )
}

/// One capability in full, as `capabilities.describe` answers it: what it is, every
/// surface it is exposed on, and the fields its input takes.
fn detail(ctx: &Context, id: &str) -> El {
    let described = ask_input(
        ctx,
        "capabilities.describe",
        serde_json::json!({ "id": id }),
    );
    let body = unanswered(&described).unwrap_or_else(|| {
        let at = |p: &str| {
            described
                .ok()
                .and_then(|v| v.pointer(p))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let fields = described
            .ok()
            .and_then(|v| v.pointer("/input/schema/properties"))
            .and_then(Value::as_object)
            .map(|props| {
                props
                    .iter()
                    .map(|(name, schema)| {
                        el("tr")
                            .child(el("td").class("mj-ui-mono").text(name))
                            .child(
                                el("td")
                                    .class("mj-ui-dim")
                                    .text(text(schema, "description")),
                            )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        el("div")
            .class("mj-ui-stack")
            .child(
                el("div")
                    .class("mj-ui-detail-head")
                    .child(el("h3").class("mj-ui-detail-title").text(at("/title")))
                    .child(el("p").class("mj-ui-dim").text(at("/description"))),
            )
            .child(
                el("dl").class("mj-ui-facts").children(
                    [
                        ("kind", "/kind"),
                        ("module", "/module"),
                        ("stability", "/stability"),
                    ]
                    .into_iter()
                    .map(|(label, p)| {
                        el("div")
                            .class("mj-ui-fact")
                            .child(el("dt").text(label))
                            .child(el("dd").text(at(p)))
                    }),
                ),
            )
            .child(if fields.is_empty() {
                nothing("It takes no input.")
            } else {
                table(
                    &["Input field", "What it means"],
                    fields,
                    "It takes no input.",
                )
            })
    });
    panel(
        id,
        "One capability, as its declaration describes it.",
        body,
        &[&described],
    )
    .attr("id", "detail")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    /// A capability that takes no input says so instead of drawing an empty table, and one
    /// that takes fields lists each with what it means.
    #[test]
    fn a_capability_without_input_says_it_takes_none() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = detail(&ctx, "deploy.list").render();
        assert!(html.contains("It takes no input."), "{html}");
        assert!(html.contains("id=\"detail\""), "{html}");
        let html = detail(&ctx, "objects.list").render();
        assert!(html.contains("Input field"), "{html}");
        assert!(!html.contains("It takes no input."), "{html}");
    }

    fn answer(input: &'static str, capabilities: Value) -> Answer {
        Answer {
            source: Source::with("capabilities.list", input),
            route: None,
            exposure: super::super::source::Exposure::default(),
            value: Ok(serde_json::json!({ "capabilities": capabilities }).into()),
            took: std::time::Duration::from_millis(1),
            standing: super::super::source::Standing::NOW,
        }
    }

    fn registry() -> (Answer, Answer) {
        let c = |id: &str, kind: &str, module: &str, stability: &str, exposure: Value| {
            serde_json::json!({ "id": id, "kind": kind, "module": module, "title": format!("{id} title"),
                "description": "", "stability": stability, "exposure": exposure, "tags": ["tagged"] })
        };
        let everywhere = serde_json::json!({ "mcp": { "tool": "t" }, "http": { "path": "/p" }, "cli": { "path": ["x"] } });
        let mcp_only = serde_json::json!({ "mcp": { "tool": "t" } });
        (
            answer(
                r#"{"kind":"command"}"#,
                serde_json::json!([c(
                    "plan.transition",
                    "command",
                    "plan",
                    "experimental",
                    mcp_only.clone()
                )]),
            ),
            answer(
                r#"{"kind":"query"}"#,
                serde_json::json!([
                    c(
                        "plan.issues",
                        "query",
                        "plan",
                        "behaviorally_verified",
                        everywhere.clone()
                    ),
                    c("plan.status", "query", "plan", "implemented", mcp_only),
                    c(
                        "artifacts.list",
                        "query",
                        "artifacts",
                        "behaviorally_verified",
                        everywhere
                    ),
                ]),
            ),
        )
    }

    fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    /// Facets narrow together, and each counts what choosing it would show among what the
    /// other picks admit; a row opens its declaration without losing the filters.
    #[test]
    fn the_facets_narrow_together_and_count_what_they_would_show() {
        let (commands, queries) = registry();
        let pick = Pick::from_query(&q(&[("module", "plan"), ("surface", "http")]));
        let html = browser(&commands, &queries, &pick).render();
        assert!(html.contains("1 of 4 capabilities shown"), "{html}");
        assert!(
            html.contains("capability-plan.issues") && !html.contains("capability-plan.status")
        );
        // kind: within module plan on HTTP, one query and no command
        assert!(
            html.contains(">query 1<") && !html.contains(">command "),
            "{html}"
        );
        // surface: with module plan, three are on MCP and one on HTTP
        assert!(
            html.contains("MCP (3)") && html.contains("HTTP (1)"),
            "{html}"
        );
        // module: on HTTP, one in plan and one in artifacts
        assert!(
            html.contains("plan (1)") && html.contains("artifacts (1)"),
            "{html}"
        );
        assert!(
            html.contains(
                "/ui/capabilities?module=plan&amp;surface=http&amp;id=plan.issues#detail"
            ),
            "a row keeps the filters: {html}"
        );
        assert!(html.contains("Clear"));
    }

    /// A link to a narrowed view asks a crawler not to follow it, and a declaration's own
    /// unnarrowed link does not: the crawl reads each declaration once, not once per view.
    #[test]
    fn only_a_link_to_an_unnarrowed_page_is_followed() {
        let (commands, queries) = registry();
        let html = browser(&commands, &queries, &Pick::default()).render();
        assert!(
            html.contains("href=\"/ui/capabilities?id=plan.issues#detail\">"),
            "an unnarrowed row is followed: {html}"
        );
        assert!(
            html.contains("href=\"/ui/capabilities?module=plan\" rel=\"nofollow\""),
            "a module narrows: {html}"
        );
        assert!(
            html.contains("href=\"/ui/capabilities?kind=query\" rel=\"nofollow\""),
            "a kind tab narrows: {html}"
        );
        assert!(html.contains("href=\"/ui/capabilities\" aria-current=\"page\""));
        let pick = Pick::from_query(&q(&[("module", "plan")]));
        let html = browser(&commands, &queries, &pick).render();
        assert!(
            html.contains(
                "href=\"/ui/capabilities?module=plan&amp;id=plan.issues#detail\" rel=\"nofollow\""
            ),
            "a row of a narrowed view keeps its filters and is not followed: {html}"
        );
        assert!(
            html.contains("href=\"/ui/capabilities\">Clear<"),
            "Clear leads to the unnarrowed list and is followed: {html}"
        );
    }

    /// The search reads ids, titles, descriptions, modules and tags; a value a link carries
    /// that is no word of the registry is dropped, and an unknown surface with it.
    #[test]
    fn a_search_finds_by_tag_and_a_hostile_value_is_dropped() {
        let (commands, queries) = registry();
        let html = browser(
            &commands,
            &queries,
            &Pick::from_query(&q(&[("q", "ARTIFACTS")])),
        )
        .render();
        assert!(html.contains("1 of 4 capabilities shown"), "{html}");
        let html = browser(
            &commands,
            &queries,
            &Pick::from_query(&q(&[("q", "tagged")])),
        )
        .render();
        assert!(html.contains("4 of 4 capabilities shown"), "{html}");
        let pick = Pick::from_query(&q(&[
            ("module", "x\" onclick=\"y"),
            ("surface", "ftp"),
            ("id", "<script>"),
        ]));
        assert_eq!(pick, Pick::default());
        let html = browser(&commands, &queries, &pick).render();
        assert!(
            !html.contains("onclick") && !html.contains("Clear"),
            "{html}"
        );
    }
}
