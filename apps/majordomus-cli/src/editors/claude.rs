//! Claude Code: what it reads from a project, and the projection of the capabilities that
//! declare a skill onto it.
//!
//! Verified against Claude Code 2.1 — <https://code.claude.com/docs/en/skills> (a project
//! skill is `.claude/skills/<name>/SKILL.md`; its front matter carries `name`, `description`,
//! `argument-hint` and `disable-model-invocation`; `$ARGUMENTS` is replaced by the text typed
//! after the name; custom commands and skills are one mechanism), `/docs/en/mcp` (a project's
//! servers are `.mcp.json`) and `/docs/en/memory` (`CLAUDE.md`).
//!
//! A skill written here is an adapter and nothing else: it names the capability, says where
//! the typed text goes, and tells the agent to call it once. What the capability does, what
//! it accepts and what it answers are its declaration's to say, and this file repeats none of
//! it beyond the title and description the declaration itself carries (ADR 0144, ADR 0140).

use super::{CommandRef, EditorAdapter, EditorCapability, EditorDiagnostic, EditorSupport};
use crate::generate::{banner_lines, Artifact, ArtifactFormat};
use crate::model::Severity;

/// The adapter's id.
pub const ID: &str = "claude-code";

/// The release the claims below were verified against.
pub const VERIFIED_AGAINST: &str = "Claude Code 2.1";

/// Where Claude Code discovers a project's skills.
pub const SKILLS_DIR: &str = ".claude/skills";

/// The most a skill's description may hold before Claude Code truncates it in its listing.
const DESCRIPTION_BUDGET: usize = 1024;

/// The path of the skill a command is projected to.
pub fn skill_path(alias: &str) -> String {
    format!("{SKILLS_DIR}/{alias}/SKILL.md")
}

/// The skill of one capability: front matter Claude Code requires, the banner, and the
/// instruction to call the capability once with the typed text in its declared field.
///
/// A capability that changes something is invoked by a person only
/// (`disable-model-invocation: true`): the model does not decide on its own to write the
/// repository because a conversation resembled the description.
pub fn command_artifact(command: &CommandRef, version: &str) -> Artifact {
    let path = skill_path(&command.alias);
    let summary = format!(
        "{}. Runs the Majordomus capability {} with the text you type as its `{}`.",
        command.title.trim_end_matches('.'),
        command.capability,
        command.argument
    );
    let description: String = summary.chars().take(DESCRIPTION_BUDGET).collect();
    let quoted = serde_json::to_string(&description).unwrap_or_else(|_| "\"\"".into());
    let hint =
        serde_json::to_string(&format!("<{}>", command.argument)).unwrap_or_else(|_| "\"\"".into());
    let [a, b, c] = banner_lines(&command.source, version);
    let mut front = format!(
        "---\nname: {}\ndescription: {quoted}\nargument-hint: {hint}\n",
        command.alias
    );
    if command.changes_something {
        front.push_str("disable-model-invocation: true\n");
    }
    front.push_str("---\n");

    let mut ways = String::new();
    if let Some(tool) = &command.tool {
        ways.push_str(&format!(
            "- With the Majordomus MCP server attached, call the tool `{tool}` with that text as `{}`.\n",
            command.argument
        ));
    }
    if let Some(cli) = &command.cli {
        ways.push_str(&format!(
            "- Otherwise run `majordomus {cli}` from the repository root, giving that text as its `{}` (`majordomus {cli} --help` shows how).\n",
            command.argument
        ));
    }
    let once = if command.changes_something {
        "Call it once. It changes the repository: if the call is refused or its outcome is not \
         known, report that as it is and do not try again by another way.\n"
    } else {
        "Call it once and report what it answers.\n"
    };
    let content = format!(
        "{front}<!-- {a}\n     {b}\n     {c} -->\n\n\
         # {title}\n\n\
         Invoke the Majordomus capability `{id}`. Its `{arg}` is the text below, whole and \
         verbatim — do not rephrase, shorten, translate or complete it:\n\n\
         $ARGUMENTS\n\n\
         If no text was given, ask for it and stop; do not invent one.\n\n\
         {ways}\n\
         {once}\n\
         Then report what the capability answered, as it answered.\n\n\
         What `{id}` does, what it accepts and what it answers are defined once, in its \
         declaration: `majordomus capabilities describe {id}`, or the MCP tool's own description. \
         This file only makes it reachable by name and says nothing the declaration does not.\n",
        title = command.title,
        id = command.capability,
        arg = command.argument,
    );
    Artifact::verbatim(
        path,
        format!("{}command", super::DOCUMENT_PREFIX),
        ArtifactFormat::Markdown,
        None,
        command.source.clone(),
        content,
    )
}

/// A command whose name a workflow skill of the layer already has, where both products that
/// read `.agents/skills/` and Claude Code would show two things under one name.
pub(crate) fn name_findings(
    commands: &[CommandRef],
    skills: &[super::SkillRef],
) -> Vec<EditorDiagnostic> {
    commands
        .iter()
        .filter(|c| skills.iter().any(|s| s.id == c.alias))
        .map(|c| EditorDiagnostic {
            severity: Severity::Warning,
            code: "command-shares-a-skill-name".into(),
            adapter: Some(ID.into()),
            path: Some(skill_path(&c.alias)),
            message: format!(
                "the capability {} is invoked as /{}, and a workflow skill of the same name exists under .ai/repo/skills/",
                c.capability, c.alias
            ),
            remedy: "rename the skill, or give the capability another alias in its declaration".into(),
        })
        .collect()
}

/// The workflow skills Claude Code is given a pointer to: every skill of the layer with a
/// name both products accept, except one whose name a command already has.
pub(crate) fn workflow_skills<'a>(
    commands: &[CommandRef],
    skills: &'a [super::SkillRef],
) -> Vec<&'a super::SkillRef> {
    skills
        .iter()
        .filter(|s| super::is_skill_name(&s.id))
        .filter(|s| !commands.iter().any(|c| c.alias == s.id))
        .collect()
}

/// What Claude Code does with each capability here.
pub fn adapter(commands: &[CommandRef], skills: &[super::SkillRef]) -> EditorAdapter {
    EditorAdapter {
        id: ID.into(),
        title: "Claude Code".into(),
        verified_against: VERIFIED_AGAINST.into(),
        capabilities: vec![
            EditorCapability {
                id: "commands".into(),
                support: EditorSupport::Supported,
                artifacts: commands.iter().map(|c| skill_path(&c.alias)).collect(),
                evidence: "code.claude.com/docs/en/skills".into(),
                note: "One skill per capability that declares an alias: /<alias> <text> calls the capability once with the text in its declared field. A capability that changes something is invoked by a person only. Every other capability is reached by its MCP tool, which needs no file.".into(),
            },
            EditorCapability {
                id: "skills".into(),
                support: EditorSupport::Supported,
                artifacts: workflow_skills(commands, skills)
                    .iter()
                    .map(|s| skill_path(&s.id))
                    .collect(),
                evidence: "code.claude.com/docs/en/skills".into(),
                note: "One pointer per workflow skill of the layer, so that /<id> reaches the definition under .ai/repo/skills/; Claude Code reads .claude/skills/ and not .agents/skills/. A skill whose name a command has is not written twice.".into(),
            },
            EditorCapability {
                id: "instructions".into(),
                support: EditorSupport::Supported,
                artifacts: vec!["CLAUDE.md".into()],
                evidence: "code.claude.com/docs/en/memory".into(),
                note: "Claude Code reads the provider bootstrap CLAUDE.md, which `majordomus generate providers` already derives from the policy; nothing is added here.".into(),
            },
            EditorCapability {
                id: "mcp".into(),
                support: EditorSupport::Supported,
                artifacts: vec![".mcp.json".into()],
                evidence: "code.claude.com/docs/en/mcp".into(),
                note: "The project's .mcp.json starts bin/majordomus-mcp; a person approves a project server once, in their own client.".into(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(changes: bool) -> CommandRef {
        CommandRef {
            alias: "intent".into(),
            argument: "statement".into(),
            capability: "intents.propose".into(),
            title: "Propose an intent: what must become true".into(),
            tool: Some("majordomus_intent_propose".into()),
            cli: Some("intent propose".into()),
            changes_something: changes,
            source: "apps/majordomus-cli/src/capability/builtin/intents.rs".into(),
            sha256: "0".repeat(64),
        }
    }

    /// The file Claude Code reads: front matter first, the banner after it, the typed text
    /// placed once, and both ways to reach the capability named.
    #[test]
    fn a_command_is_a_skill_that_names_its_capability_and_places_the_text_once() {
        let a = command_artifact(&command(true), "1.2.3");
        assert_eq!(a.path, ".claude/skills/intent/SKILL.md");
        assert_eq!(a.document, "editors/command");
        let c = &a.content;
        assert!(c.starts_with(
            "---\nname: intent\ndescription: \"Propose an intent: what must become true. Runs the Majordomus capability intents.propose with the text you type as its `statement`.\"\nargument-hint: \"<statement>\"\ndisable-model-invocation: true\n---\n<!-- "
        ), "{c}");
        assert!(crate::editors::banner_is_present(c));
        assert_eq!(c.matches("$ARGUMENTS").count(), 1);
        for said in [
            "capability `intents.propose`",
            "call the tool `majordomus_intent_propose` with that text as `statement`",
            "run `majordomus intent propose` from the repository root",
            "do not try again by another way",
            "Generator: majordomus-cli 1.2.3",
        ] {
            assert!(c.contains(said), "{said:?} is missing from:\n{c}");
        }
    }

    /// A read is the model's to call, has no warning about a second attempt, and a
    /// capability with no command line is not told to use one.
    #[test]
    fn a_read_may_be_invoked_by_the_model_and_names_only_the_ways_it_has() {
        let mut read = command(false);
        read.cli = None;
        let c = command_artifact(&read, "1").content;
        assert!(!c.contains("disable-model-invocation"));
        assert!(!c.contains("another way"));
        assert!(!c.contains("Otherwise run"));
        assert!(c.contains("Call it once and report what it answers."));
    }

    #[test]
    fn a_long_title_is_cut_to_what_the_listing_shows() {
        let mut long = command(false);
        long.title = "x".repeat(2 * DESCRIPTION_BUDGET);
        let c = command_artifact(&long, "1").content;
        let line = c.lines().find(|l| l.starts_with("description: ")).unwrap();
        let said: String = serde_json::from_str(&line["description: ".len()..]).unwrap();
        assert_eq!(said.chars().count(), DESCRIPTION_BUDGET);
    }

    fn workflow(id: &str) -> super::super::SkillRef {
        super::super::SkillRef {
            id: id.into(),
            title: format!("The {id} skill"),
            description: "d".into(),
            path: format!(".ai/repo/skills/{id}/SKILL.md"),
            sha256: String::new(),
        }
    }

    #[test]
    fn a_workflow_skill_is_reachable_by_its_name_unless_a_command_has_it() {
        let skills = [
            workflow("evolve"),
            workflow("intent"),
            workflow("Not A Name"),
        ];
        let given = workflow_skills(&[command(true)], &skills);
        assert_eq!(
            given.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["evolve"],
            "the command keeps `intent`, and a name the product refuses is not written"
        );
        let a = adapter(&[command(true)], &skills);
        let listed = a.capabilities.iter().find(|c| c.id == "skills").unwrap();
        assert_eq!(listed.artifacts, [".claude/skills/evolve/SKILL.md"]);
        let commands = a.capabilities.iter().find(|c| c.id == "commands").unwrap();
        assert_eq!(commands.artifacts, [".claude/skills/intent/SKILL.md"]);
    }

    #[test]
    fn a_command_named_as_a_workflow_skill_is_a_finding() {
        let skill = super::super::SkillRef {
            id: "intent".into(),
            title: "t".into(),
            description: "d".into(),
            path: ".ai/repo/skills/intent/SKILL.md".into(),
            sha256: String::new(),
        };
        let found = name_findings(&[command(true)], &[skill]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "command-shares-a-skill-name");
        assert!(name_findings(&[command(true)], &[]).is_empty());
        let a = adapter(&[command(true)], &[]);
        assert_eq!(a.id, "claude-code");
        assert_eq!(
            a.capabilities[0].artifacts,
            [".claude/skills/intent/SKILL.md"]
        );
    }
}
