//! Zed: what it reads from a project, and the projection of the environment onto it.
//!
//! Verified against Zed 1.23.2 — the installed application and the source at that tag:
//! `crates/settings_content/src/project.rs` (the keys `.zed/settings.json` may carry),
//! `crates/task/src/task_template.rs` (the shape of `.zed/tasks.json`),
//! `crates/prompt_store/src/prompts.rs` (the instruction files, first match wins), and the
//! documentation at <https://zed.dev/docs/tasks>, <https://zed.dev/docs/ai/instructions>,
//! <https://zed.dev/docs/ai/skills> and <https://zed.dev/docs/ai/mcp>.

use std::path::Path;

use serde_json::{json, Value};

use super::{
    jsonc_banner, EditorAdapter, EditorCapability, EditorDiagnostic, EditorEnvironment,
    EditorSupport, DOCUMENT_PREFIX, JUSTFILE_SOURCE, SKILLS_DIR,
};
use crate::generate::{Artifact, ArtifactFormat};
use crate::model::Severity;

/// The adapter's id.
pub const ID: &str = "zed";

/// The project task list.
pub const TASKS: &str = ".zed/tasks.json";

/// The release the claims below were verified against.
pub const VERIFIED_AGAINST: &str = "Zed 1.23.2";

/// The instruction files Zed reads, in the order it looks for them; the first present wins
/// and the rest are ignored (`prompts.rs`, `RULES_FILE_NAMES`).
pub const INSTRUCTION_FILES: &[&str] = &[
    ".rules",
    ".cursorrules",
    ".windsurfrules",
    ".clinerules",
    ".github/copilot-instructions.md",
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
];

/// `.zed/tasks.json`: one task per recipe, run as `just <recipe>` from the project root —
/// the canonical command, never a copy of its body. `None` when there is nothing to run.
pub fn tasks_artifact(env: &EditorEnvironment, version: &str) -> Option<Artifact> {
    if env.tasks.is_empty() {
        return None;
    }
    let tasks: Vec<Value> = env
        .tasks
        .iter()
        .map(|t| {
            let mut task = json!({
                "label": format!("just {}", t.recipe),
                "command": "just",
                "args": [t.recipe],
            });
            if let Some(g) = &t.group {
                task["tags"] = json!([g]);
            }
            task
        })
        .collect();
    // `{:#}` is serde_json's pretty form of a value, and a value cannot fail to print
    let body = format!("{:#}", Value::Array(tasks));
    Some(Artifact::verbatim(
        TASKS,
        format!("{DOCUMENT_PREFIX}zed-tasks"),
        ArtifactFormat::Json,
        None,
        JUSTFILE_SOURCE,
        format!("{}{body}\n", jsonc_banner(JUSTFILE_SOURCE, version)),
    ))
}

/// What Zed does with each capability here.
pub fn adapter(env: &EditorEnvironment) -> EditorAdapter {
    let skills = env
        .skills
        .iter()
        .filter(|s| super::is_skill_name(&s.id))
        .map(|s| format!("{SKILLS_DIR}/{}/SKILL.md", s.id))
        .collect();
    let tasks = if env.tasks.is_empty() {
        Vec::new()
    } else {
        vec![TASKS.to_string()]
    };
    EditorAdapter {
        id: ID.into(),
        title: "Zed".into(),
        verified_against: VERIFIED_AGAINST.into(),
        capabilities: vec![
            EditorCapability {
                id: "instructions".into(),
                support: EditorSupport::Supported,
                artifacts: vec!["AGENTS.md".into()],
                evidence: "zed.dev/docs/ai/instructions; prompts.rs RULES_FILE_NAMES".into(),
                note: "Zed reads the provider bootstrap AGENTS.md, which `majordomus generate providers` already derives from the policy; nothing is added for Zed.".into(),
            },
            EditorCapability {
                id: "mcp".into(),
                support: EditorSupport::RequiresUserAction,
                artifacts: Vec::new(),
                evidence: "zed.dev/docs/ai/mcp; project.rs ContextServerSettingsContent".into(),
                note: "A project `context_servers` entry needs a command Zed can start, and the repository's launcher is the relative path bin/majordomus-mcp, whose resolution Zed does not document. Add it in your own settings: {\"context_servers\": {\"majordomus\": {\"command\": \"<absolute path to this checkout>/bin/majordomus-mcp\", \"args\": []}}}.".into(),
            },
            EditorCapability {
                id: "settings".into(),
                support: EditorSupport::Unsupported,
                artifacts: Vec::new(),
                evidence: "project.rs ProjectSettingsContent".into(),
                note: "No repository policy differs from Zed's defaults, so no .zed/settings.json is written; theme, fonts, keys and agent settings are personal and never projected.".into(),
            },
            EditorCapability {
                id: "skills".into(),
                support: EditorSupport::Supported,
                artifacts: skills,
                evidence: "zed.dev/docs/ai/skills".into(),
                note: "Read from .agents/skills/<name>/SKILL.md in trusted worktrees; each file points at the canonical definition.".into(),
            },
            EditorCapability {
                id: "tasks".into(),
                support: if env.workflows_resolved == Some(false) {
                    EditorSupport::Unknown
                } else {
                    EditorSupport::Supported
                },
                artifacts: tasks,
                evidence: "zed.dev/docs/tasks; task_template.rs TaskTemplate".into(),
                note: "Every public recipe of the tracked justfile without [confirm], run as `just <recipe>` from the project root.".into(),
            },
        ],
    }
}

/// Which instruction file Zed reads in this checkout. A file earlier in its list than
/// AGENTS.md hides the bootstrap from Zed entirely.
pub fn instruction_findings(root: &Path) -> Vec<EditorDiagnostic> {
    let first = INSTRUCTION_FILES
        .iter()
        .find(|f| root.join(f).is_file())
        .copied();
    match first {
        Some("AGENTS.md") | None => Vec::new(),
        Some(other) => vec![EditorDiagnostic {
            severity: Severity::Warning,
            code: "zed-reads-another-instruction-file".into(),
            adapter: Some(ID.into()),
            path: Some(other.into()),
            message: format!(
                "Zed reads the first of its instruction files that exists, and here that is {other}, not AGENTS.md"
            ),
            remedy: format!("fold {other} into the policy's projections or remove it, so that Zed reads AGENTS.md"),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editors::TaskRef;

    #[test]
    fn a_task_runs_the_recipe_and_carries_no_shell() {
        let env = EditorEnvironment {
            tasks: vec![TaskRef {
                recipe: "test".into(),
                group: Some("test".into()),
                group_rank: 0,
            }],
            ..Default::default()
        };
        let a = tasks_artifact(&env, "0.0.0").unwrap();
        let body = crate::editors::jsonc_body(&a.content).unwrap();
        let v: Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            v,
            json!([{"label": "just test", "command": "just", "args": ["test"], "tags": ["test"]}])
        );
    }

    #[test]
    fn no_recipe_no_file() {
        assert!(tasks_artifact(&EditorEnvironment::default(), "0.0.0").is_none());
    }

    #[test]
    fn a_rules_file_earlier_in_zeds_list_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "x").unwrap();
        assert!(instruction_findings(dir.path()).is_empty());
        std::fs::write(dir.path().join(".cursorrules"), "x").unwrap();
        let found = instruction_findings(dir.path());
        assert_eq!(found[0].path.as_deref(), Some(".cursorrules"));
    }

    #[test]
    fn the_adapter_says_unknown_when_the_justfile_could_not_be_read() {
        let unread = EditorEnvironment {
            workflows_resolved: Some(false),
            ..Default::default()
        };
        let tasks = |env: &EditorEnvironment| {
            adapter(env)
                .capabilities
                .into_iter()
                .find(|c| c.id == "tasks")
                .unwrap()
        };
        assert_eq!(tasks(&unread).support, EditorSupport::Unknown);
        assert!(tasks(&unread).artifacts.is_empty());

        let read = EditorEnvironment {
            workflows_resolved: Some(true),
            tasks: vec![TaskRef {
                recipe: "loose".into(),
                group: None,
                group_rank: usize::MAX,
            }],
            skills: vec![crate::editors::SkillRef {
                id: "ok".into(),
                title: "Ok".into(),
                description: "d".into(),
                path: ".ai/repo/skills/ok/SKILL.md".into(),
                sha256: String::new(),
            }],
            ..Default::default()
        };
        assert_eq!(tasks(&read).support, EditorSupport::Supported);
        assert_eq!(tasks(&read).artifacts, [TASKS]);
        // a recipe in no group carries no tag
        let a = tasks_artifact(&read, "0.0.0").unwrap();
        assert!(!a.content.contains("\"tags\""), "{}", a.content);
    }
}
