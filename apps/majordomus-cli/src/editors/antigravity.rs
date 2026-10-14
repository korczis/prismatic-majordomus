//! Google Antigravity: what it reads from a project, and the projection of the environment
//! onto it.
//!
//! Verified against Antigravity 2.22.0 (`com.google.antigravity`, the 2.0 desktop app) —
//! the guide text shipped inside the application's language server, the changelog at
//! <https://antigravity.google/docs/changelog/>, and the pages <https://antigravity.google/docs/rules>,
//! `/docs/skills`, `/docs/mcp`, `/docs/ide/workflows` and `/docs/migration/workflows-to-skills`.
//! Where the shipped guide and the web pages disagree the projection writes nothing and the
//! capability says [`EditorSupport::Unknown`].

use std::path::Path;

use super::{
    EditorAdapter, EditorCapability, EditorDiagnostic, EditorEnvironment, EditorSupport, SKILLS_DIR,
};
use crate::model::Severity;

/// The adapter's id.
pub const ID: &str = "antigravity";

/// The release the claims below were verified against.
pub const VERIFIED_AGAINST: &str = "Antigravity 2.22.0";

/// The values a modular rule's `trigger` may take; anything else, or none, and Antigravity
/// discards the rule without a word.
pub const TRIGGERS: &[&str] = &["always_on", "model_decision", "glob", "manual"];

/// The most a rule file may hold, after its includes are expanded.
pub const RULE_BYTES: usize = 24_000;

/// What Antigravity does with each capability here.
pub fn adapter(env: &EditorEnvironment) -> EditorAdapter {
    let skills = env
        .skills
        .iter()
        .filter(|s| super::is_skill_name(&s.id))
        .map(|s| format!("{SKILLS_DIR}/{}/SKILL.md", s.id))
        .collect();
    EditorAdapter {
        id: ID.into(),
        title: "Google Antigravity".into(),
        verified_against: VERIFIED_AGAINST.into(),
        capabilities: vec![
            EditorCapability {
                id: "mcp".into(),
                support: EditorSupport::Unknown,
                artifacts: Vec::new(),
                evidence: "antigravity.google/docs/mcp documents .agents/mcp_config.json; the MCP guide shipped in 2.22.0 names only the global and plugin locations".into(),
                note: "No workspace MCP file is written while the installed product does not confirm it reads one. Add the server to ~/.gemini/config/mcp_config.json yourself: {\"mcpServers\": {\"majordomus\": {\"command\": \"<absolute path to this checkout>/bin/majordomus-mcp\", \"args\": []}}}.".into(),
            },
            EditorCapability {
                id: "permissions".into(),
                support: EditorSupport::RequiresUserAction,
                artifacts: Vec::new(),
                evidence: "antigravity.google/docs/agent-settings".into(),
                note: "Approval and terminal-execution modes are personal and are never projected; a repository does not loosen them.".into(),
            },
            EditorCapability {
                id: "rules".into(),
                support: EditorSupport::Supported,
                artifacts: vec!["AGENTS.md".into()],
                evidence: "antigravity.google/docs/rules: AGENTS.md is always on for its directory".into(),
                note: "The provider bootstrap AGENTS.md routes into .ai/; no rule corpus is copied into .agents/rules/.".into(),
            },
            EditorCapability {
                id: "skills".into(),
                support: EditorSupport::Supported,
                artifacts: skills,
                evidence: "antigravity.google/docs/skills; the built-in guide's skill section (name and description required)".into(),
                note: "Read from .agents/skills/<name>/SKILL.md; each file points at the canonical definition.".into(),
            },
            EditorCapability {
                id: "workflows".into(),
                support: EditorSupport::Deprecated,
                artifacts: Vec::new(),
                evidence: "antigravity.google/docs/migration/workflows-to-skills: workflows stop working on 2026-10-19".into(),
                note: "Never written; the skills carry what a workflow would.".into(),
            },
        ],
    }
}

/// What this checkout holds that Antigravity would read differently from what the
/// projection intends. Read only; nothing here is ever rewritten.
pub fn legacy_findings(root: &Path) -> Vec<EditorDiagnostic> {
    let mut out = Vec::new();
    let finding =
        |severity, code: &str, path: &str, message: String, remedy: &str| EditorDiagnostic {
            severity,
            code: code.into(),
            adapter: Some(ID.into()),
            path: Some(path.into()),
            message,
            remedy: remedy.into(),
        };
    if root.join(".agents/workflows").is_dir() {
        out.push(finding(
            Severity::Warning,
            "antigravity-legacy-workflows",
            ".agents/workflows",
            "Antigravity stops running .agents/workflows on 2026-10-19".into(),
            "turn each workflow into a skill under .ai/repo/skills/, which is projected to .agents/skills/",
        ));
    }
    if root.join(".agents/skills.json").is_file() {
        out.push(finding(
            Severity::Info,
            "antigravity-skills-manifest",
            ".agents/skills.json",
            "a skills manifest makes Antigravity load the directories it names as well as .agents/skills/; a manifest naming .ai/repo/skills loads canonical files that carry no `name`, which the shipped guide requires".into(),
            "remove .agents/skills.json; the projected .agents/skills/ already carries every active skill",
        ));
    }
    let Ok(entries) = std::fs::read_dir(root.join(".agents/rules")) else {
        return out;
    };
    let mut rules: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md") && p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    crate::order::canonical_strings(&mut rules);
    for path in rules.iter().map(std::path::Path::new) {
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        // a rule that cannot be read is one Antigravity cannot load either: it reads as empty,
        // and is reported below as carrying no trigger
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if text.len() > RULE_BYTES {
            out.push(finding(
                Severity::Warning,
                "antigravity-rule-too-large",
                &rel,
                format!(
                    "{} bytes; Antigravity reads at most {RULE_BYTES}",
                    text.len()
                ),
                "split the rule, or move its substance under .ai/ and link to it",
            ));
        }
        match trigger_of(&text) {
            Some(t) if TRIGGERS.contains(&t.as_str()) => {}
            found => out.push(finding(
                Severity::Warning,
                "antigravity-rule-discarded",
                &rel,
                match found {
                    Some(t) => format!("trigger {t:?} is not one of {}; Antigravity discards the rule", TRIGGERS.join(", ")),
                    None => "no `trigger` in front matter; Antigravity discards the rule".into(),
                },
                "open the file with front matter `trigger: always_on` (or model_decision with a description, glob with globs, manual)",
            )),
        }
    }
    out
}

/// The `trigger` of a rule's front matter, when it has one.
fn trigger_of(text: &str) -> Option<String> {
    let front = text.strip_prefix("---\n")?.split_once("\n---")?.0;
    front.lines().find_map(|l| {
        l.strip_prefix("trigger:")
            .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_without_a_valid_trigger_is_reported_and_one_with_it_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let rules = dir.path().join(".agents/rules");
        std::fs::create_dir_all(&rules).unwrap();
        std::fs::write(rules.join("plain.md"), "# no front matter\n").unwrap();
        std::fs::write(rules.join("camel.md"), "---\ntrigger: alwaysOn\n---\nx\n").unwrap();
        std::fs::write(rules.join("good.md"), "---\ntrigger: always_on\n---\nx\n").unwrap();
        // front matter that never closes is no front matter
        std::fs::write(rules.join("open.md"), "---\ntrigger: always_on\n").unwrap();
        let found = legacy_findings(dir.path());
        let paths: Vec<&str> = found.iter().filter_map(|d| d.path.as_deref()).collect();
        assert_eq!(
            paths,
            [
                ".agents/rules/camel.md",
                ".agents/rules/open.md",
                ".agents/rules/plain.md"
            ]
        );
        assert!(found.iter().all(|d| d.code == "antigravity-rule-discarded"));
    }

    #[test]
    fn legacy_workflows_and_a_skills_manifest_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".agents/workflows")).unwrap();
        std::fs::write(dir.path().join(".agents/skills.json"), "{}").unwrap();
        let codes: Vec<String> = legacy_findings(dir.path())
            .into_iter()
            .map(|d| d.code)
            .collect();
        assert_eq!(
            codes,
            [
                "antigravity-legacy-workflows",
                "antigravity-skills-manifest"
            ]
        );
    }

    #[test]
    fn a_rule_over_the_budget_is_reported_and_a_quoted_trigger_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let rules = dir.path().join(".agents/rules");
        std::fs::create_dir_all(&rules).unwrap();
        let body = "x".repeat(RULE_BYTES + 1);
        std::fs::write(
            rules.join("big.md"),
            format!("---\ntrigger: \"always_on\"\n---\n{body}\n"),
        )
        .unwrap();
        let found = legacy_findings(dir.path());
        let codes: Vec<&str> = found.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(
            codes,
            ["antigravity-rule-too-large"],
            "the quoted trigger is valid"
        );
    }

    #[test]
    fn the_adapter_names_every_projected_skill_and_no_refused_one() {
        let skill = |id: &str| crate::editors::SkillRef {
            id: id.into(),
            title: id.into(),
            description: "d".into(),
            path: format!(".ai/repo/skills/{id}/SKILL.md"),
            sha256: String::new(),
        };
        let env = EditorEnvironment {
            skills: vec![skill("ok"), skill("Not_Ok")],
            ..Default::default()
        };
        let view = adapter(&env);
        let skills = view.capabilities.iter().find(|c| c.id == "skills").unwrap();
        assert_eq!(skills.artifacts, [".agents/skills/ok/SKILL.md"]);
        assert_eq!(skills.support, EditorSupport::Supported);
        let mcp = view.capabilities.iter().find(|c| c.id == "mcp").unwrap();
        assert_eq!(
            mcp.support,
            EditorSupport::Unknown,
            "what is not verified is not written"
        );
    }
}
