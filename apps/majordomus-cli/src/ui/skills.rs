//! Skills: what an agent can be asked by name, and where each name comes from (ADR 0165).
//! Read from `skills.status` (the layer's workflow skills) and `editors.status` (every file
//! an editor or an agent reads, the derived domain and command skills among them); nothing
//! here names a skill, so a skill the projection derives is a row here as soon as it exists.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{hero, kpi_count, list, panel, state, table, text, unanswered, wide};

/// The route of the screen.
pub const ROUTE: &str = "/ui/skills";

const SKILLS: Source = Source::of("skills.status");
const EDITORS: Source = Source::of("editors.status");

/// The Skills screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[SKILLS, EDITORS]);
    let (skills, editors) = (&answers[&SKILLS], &answers[&EDITORS]);

    let top = hero(
        "sparkles",
        "violet",
        "Skills",
        "What an agent can be asked by name, and where each name comes from.",
        "Workflow skills are written once under the layer; command and domain skills are derived from the capability registry. Every editor and agent reads the same projection.",
        vec![
            kpi_count("Workflow skills", &skills.read("/skills")),
            kpi_count("Projected files", &editors.read("/artifacts")),
            kpi_count("Editors and agents", &editors.read("/adapters")),
            kpi_count("Findings", &editors.read("/diagnostics")),
        ],
    );

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(panel(
            "Workflow skills",
            "The layer's own procedures, one directory each under the skills section.",
            workflows(skills),
            &[skills],
        )))
        .child(wide(panel(
            "By name, in every editor and agent",
            "Each name as it is typed, the adapter that reads it, and what it projects: a workflow skill, a capability, or a domain of the registry.",
            projected(editors),
            &[editors],
        )));

    View::new(
        "Skills",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

fn workflows(answer: &Answer) -> El {
    unanswered(answer).unwrap_or_else(|| {
        let rows = list(answer, "/skills")
            .iter()
            .map(|s| {
                el("tr")
                    .attr("id", format!("skill-{}", text(s, "id")))
                    .child(
                        el("td")
                            .child(el("span").class("mj-ui-mono mj-ui-dim").text(text(s, "id")))
                            .child(el("span").class("mj-ui-cell-title").text(text(s, "title"))),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-dim")
                            .nodes(super::prose::marks(&text(s, "description"))),
                    )
                    .child(el("td").child(state(s, "status")))
            })
            .collect::<Vec<_>>();
        table(
            &["Skill", "When to use it", "Status"],
            rows,
            "The layer declares no workflow skill.",
        )
    })
}

/// The name a file is typed by: its skill directory for a `SKILL.md`, the path otherwise.
fn typed(path: &str) -> String {
    match path.strip_suffix("/SKILL.md") {
        Some(dir) => format!("/{}", dir.rsplit('/').next().unwrap_or(dir)),
        None => path.to_string(),
    }
}

fn projected(answer: &Answer) -> El {
    unanswered(answer).unwrap_or_else(|| {
        let mut rows = Vec::new();
        for adapter in list(answer, "/adapters") {
            let title = text(&adapter, "title");
            let capabilities = adapter
                .get("capabilities")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for capability in capabilities {
                let what = text(&capability, "id");
                for path in capability
                    .get("artifacts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    rows.push(
                        el("tr")
                            .child(el("td").class("mj-ui-mono").text(typed(path)))
                            .child(el("td").text(title.clone()))
                            .child(el("td").class("mj-ui-mono mj-ui-dim").text(what.clone()))
                            .child(el("td").child(state(&capability, "support"))),
                    );
                }
            }
        }
        table(
            &["Name", "Read by", "Projects", "Support"],
            rows,
            "No editor or agent reads anything from this repository.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skill_is_named_as_it_is_typed() {
        assert_eq!(typed(".claude/skills/mj-mesh/SKILL.md"), "/mj-mesh");
        assert_eq!(typed(".agents/skills/evolve/SKILL.md"), "/evolve");
        assert_eq!(typed(".mcp.json"), ".mcp.json");
    }

    fn answer(value: Result<Value, String>) -> Answer {
        Answer {
            source: SKILLS,
            route: None,
            exposure: Default::default(),
            value: value.map(Into::into),
            took: std::time::Duration::ZERO,
            standing: super::super::source::Standing::NOW,
        }
    }

    /// Each workflow skill is a row anchored by its id, carrying its title, its description
    /// and its status; a skill with no status is `unknown`, never an empty badge.
    #[test]
    fn a_workflow_skill_is_a_row_with_its_title_description_and_status() {
        let html = workflows(&answer(Ok(serde_json::json!({
            "skills": [
                {
                    "id": "evolve",
                    "title": "Evolve",
                    "description": "Turn observations into knowledge.",
                    "status": "Active"
                },
                {"id": "implement", "title": "Implement", "description": "Carry a change."}
            ]
        }))))
        .render();
        for said in [
            "id=\"skill-evolve\"",
            "id=\"skill-implement\"",
            "Evolve",
            "Turn observations into knowledge.",
            "Carry a change.",
            "active",
            "unknown",
        ] {
            assert!(html.contains(said), "{said:?} is missing from:\n{html}");
        }
        assert!(!html.contains("declares no workflow skill"), "{html}");
    }

    /// No skill is said in words, and a source that could not answer says why.
    #[test]
    fn no_workflow_skill_is_said_and_an_unanswered_source_says_why() {
        let none = workflows(&answer(Ok(serde_json::json!({"skills": []})))).render();
        assert!(
            none.contains("The layer declares no workflow skill."),
            "{none}"
        );
        assert!(!none.contains("<tr"), "{none}");

        let failed = workflows(&answer(Err("the index is rebuilding".into()))).render();
        assert!(
            failed.contains("skills.status could not answer: the index is rebuilding"),
            "{failed}"
        );
    }
}
