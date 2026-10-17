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

use super::{
    CommandRef, DomainRef, EditorAdapter, EditorCapability, EditorDiagnostic, EditorSupport,
};
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

/// What a domain's description says in the listing at most: Claude Code shows the start of
/// it beside every other skill, and fifty-odd domains must not crowd the listing out.
const DOMAIN_DESCRIPTION_BUDGET: usize = 300;

/// The description of a domain skill: the module and its capabilities' own titles, cut at a
/// word so that what the listing shows is what the registry says.
fn domain_description(domain: &DomainRef) -> String {
    let titles: Vec<&str> = domain
        .entries
        .iter()
        .map(|e| e.title.trim_end_matches('.'))
        .collect();
    let whole = format!(
        "Majordomus {}: {}.",
        domain.module.replace('_', " "),
        titles.join("; ")
    );
    if whole.chars().count() <= DOMAIN_DESCRIPTION_BUDGET {
        return whole;
    }
    let cut: String = whole.chars().take(DOMAIN_DESCRIPTION_BUDGET - 1).collect();
    let at = cut.rfind(' ').unwrap_or(cut.len());
    format!("{}…", cut[..at].trim_end_matches([';', ',', ' ']))
}

/// Text as one Markdown table cell: a pipe is escaped and a line break becomes a space, so
/// a title can neither end its cell nor its row.
fn table_cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// The skill of one domain (ADR 0165): the module's capabilities as a table the agent chooses
/// from, the request typed after the name, and the rule that a capability that changes
/// something runs only when the person asked for that change. The model may load it on its
/// own, because loading it changes nothing; calling a capability is what the rule governs.
pub fn domain_artifact(domain: &DomainRef, version: &str) -> Artifact {
    let path = skill_path(&domain.alias);
    let quoted =
        serde_json::to_string(&domain_description(domain)).unwrap_or_else(|_| "\"\"".into());
    let source = format!("module:{}", domain.module);
    let [a, b, c] = banner_lines(&source, version);
    let mut rows = String::new();
    for e in &domain.entries {
        rows.push_str(&format!(
            "| `{}` | {} | `{}` | {} | {} |\n",
            e.capability,
            table_cell(&e.title),
            e.tool,
            e.cli
                .as_ref()
                .map(|c| format!("`majordomus {c}`"))
                .unwrap_or_else(|| "—".into()),
            if e.changes_something {
                "changes"
            } else {
                "reads"
            },
        ));
    }
    let content = format!(
        "---\nname: {alias}\ndescription: {quoted}\nargument-hint: \"[request]\"\n---\n\
         <!-- {a}\n     {b}\n     {c} -->\n\n\
         # Majordomus `{module}`\n\n\
         The capabilities the Majordomus registry declares in its module `{module}`. Choose the \
         one that answers the request below — or, without one, the person's last request about \
         `{module}` — call it through its MCP tool, and report what it answers, as it answered:\n\n\
         $ARGUMENTS\n\n\
         | Capability | What it is | MCP tool | Command line | Effect |\n\
         |---|---|---|---|---|\n\
         {rows}\n\
         A capability that *changes* something runs only when the person asked for that change: \
         call it once, and if it is refused or its outcome is not known, report that as it is \
         and do not try again by another way. One that *reads* may be called whenever the \
         request needs it.\n\n\
         Without the Majordomus MCP server attached, run the command line form from the \
         repository root; `--help` shows its arguments. What each capability accepts and \
         answers is defined once, in its declaration: `majordomus capabilities describe <id>`, \
         or the MCP tool's own description. This file is derived from the registry and says \
         nothing the declarations do not.\n",
        alias = domain.alias,
        module = domain.module,
    );
    Artifact::verbatim(
        path,
        format!("{}domain", super::DOCUMENT_PREFIX),
        ArtifactFormat::Markdown,
        None,
        source,
        content,
    )
}

/// The domains Claude Code is given: every derived domain whose name no command alias and no
/// workflow skill of the layer already has. A name both want stays theirs, and the domain is
/// reported by [`name_findings`].
pub(crate) fn reachable_domains(
    domains: Vec<DomainRef>,
    commands: &[CommandRef],
    skills: &[super::SkillRef],
) -> Vec<DomainRef> {
    domains
        .into_iter()
        .filter(|d| !commands.iter().any(|c| c.alias == d.alias))
        .filter(|d| !skills.iter().any(|s| s.id == d.alias))
        .collect()
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
pub fn adapter(
    commands: &[CommandRef],
    domains: &[DomainRef],
    skills: &[super::SkillRef],
) -> EditorAdapter {
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
                id: "domains".into(),
                support: EditorSupport::Supported,
                artifacts: domains.iter().map(|d| skill_path(&d.alias)).collect(),
                evidence: "code.claude.com/docs/en/skills".into(),
                note: "One skill per module of the registry with a public MCP tool (ADR 0165): /mj-<module> lists the module's capabilities, and the model may load it when a request is about that module. A capability that changes something runs only on the person's request.".into(),
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
        let a = adapter(&[command(true)], &[], &skills);
        let listed = a.capabilities.iter().find(|c| c.id == "skills").unwrap();
        assert_eq!(listed.artifacts, [".claude/skills/evolve/SKILL.md"]);
        let commands = a.capabilities.iter().find(|c| c.id == "commands").unwrap();
        assert_eq!(commands.artifacts, [".claude/skills/intent/SKILL.md"]);
    }

    fn domain(module: &str, titles: &[(&str, bool)]) -> DomainRef {
        DomainRef {
            module: module.into(),
            alias: format!("mj-{}", module.replace('_', "-")),
            entries: titles
                .iter()
                .enumerate()
                .map(|(i, (title, changes))| crate::editors::DomainEntry {
                    capability: format!("{module}.c{i}"),
                    title: (*title).into(),
                    tool: format!("majordomus_{module}_c{i}"),
                    cli: (i == 0).then(|| format!("{module} c{i}")),
                    changes_something: *changes,
                })
                .collect(),
            sha256: "0".repeat(64),
        }
    }

    /// A domain is one skill the model may load: every capability of the module is a row
    /// naming its tool, its command line where it has one, and whether it changes anything.
    #[test]
    fn a_domain_is_one_skill_listing_every_capability_of_its_module() {
        let d = domain(
            "knowledge_base",
            &[("Read the base", false), ("Record a | lesson", true)],
        );
        let a = domain_artifact(&d, "1.2.3");
        assert_eq!(a.path, ".claude/skills/mj-knowledge-base/SKILL.md");
        assert_eq!(a.document, "editors/domain");
        let c = &a.content;
        assert!(c.starts_with(
            "---\nname: mj-knowledge-base\ndescription: \"Majordomus knowledge base: Read the base; Record a | lesson.\"\nargument-hint: \"[request]\"\n---\n<!-- "
        ), "{c}");
        assert!(
            !c.contains("disable-model-invocation"),
            "loading a domain changes nothing"
        );
        assert!(crate::editors::banner_is_present(c));
        assert_eq!(c.matches("$ARGUMENTS").count(), 1);
        for said in [
            "| `knowledge_base.c0` | Read the base | `majordomus_knowledge_base_c0` | `majordomus knowledge_base c0` | reads |",
            "| `knowledge_base.c1` | Record a \\| lesson | `majordomus_knowledge_base_c1` | — | changes |",
            "runs only when the person asked for that change",
            "Generator: majordomus-cli 1.2.3",
        ] {
            assert!(c.contains(said), "{said:?} is missing from:\n{c}");
        }
    }

    #[test]
    fn a_title_can_end_neither_its_cell_nor_its_row() {
        assert_eq!(table_cell("a | b\nc"), "a \\| b c");
        assert_eq!(table_cell("plain"), "plain");
    }

    #[test]
    fn a_long_domain_description_is_cut_at_a_word() {
        let many: Vec<(&str, bool)> = (0..60).map(|_| ("Something rather long", false)).collect();
        let said = domain_description(&domain("mesh", &many));
        assert!(said.chars().count() <= DOMAIN_DESCRIPTION_BUDGET, "{said}");
        assert!(said.ends_with("long…"), "{said}");
    }

    #[test]
    fn a_domain_yields_a_name_a_command_or_a_workflow_skill_has() {
        let mut taken = command(true);
        taken.alias = "mj-plan".into();
        let given = reachable_domains(
            vec![
                domain("plan", &[("p", false)]),
                domain("mesh", &[("m", false)]),
                domain("brief", &[("b", false)]),
            ],
            &[taken],
            &[workflow("mj-brief")],
        );
        assert_eq!(
            given.iter().map(|d| d.alias.as_str()).collect::<Vec<_>>(),
            ["mj-mesh"]
        );
        let a = adapter(&[], &given, &[]);
        let listed = a.capabilities.iter().find(|c| c.id == "domains").unwrap();
        assert_eq!(listed.artifacts, [".claude/skills/mj-mesh/SKILL.md"]);
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
        let a = adapter(&[command(true)], &[], &[]);
        assert_eq!(a.id, "claude-code");
        assert_eq!(
            a.capabilities[0].artifacts,
            [".claude/skills/intent/SKILL.md"]
        );
    }
}
