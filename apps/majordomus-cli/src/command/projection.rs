//! Projection: how one canonical identity becomes a name on each surface.
//!
//! Naming is an algorithm, never a table. `native.bench.coverage` becomes the Just recipe
//! `bench-coverage`, the Cockpit action `command/native.bench.coverage` and the page
//! `/docs/commands/native.bench.coverage/`, and it does so by rule, so that a command
//! added tomorrow is named the same way as one added today and no surface has a list to
//! keep in step.
//!
//! Two programs may want the same bare name — both executables have a `bench`. The rule
//! is fixed: the program with the lowest [`Program::precedence`] keeps the bare name and
//! every other takes its own prefix. Precedence is a constant of the model, so a name
//! never depends on the order the contributors ran in.

use std::collections::BTreeMap;

use super::model::{CommandId, CommandNode, Diagnostic, Program, Projections};

/// The route prefix of the generated command reference. The native command line's own
/// pages live under `/docs/cli/`; this section is the whole graph, every program in it.
pub const DOCS_PREFIX: &str = "/docs/commands";

/// The name a Just recipe would have if nothing else wanted it: the path words joined
/// with a hyphen.
///
/// ```
/// use majordomus_cli::command::projection::base_name;
/// assert_eq!(base_name(&["bench".into(), "coverage".into()]), "bench-coverage");
/// assert_eq!(base_name(&["doctor".into()]), "doctor");
/// ```
pub fn base_name(path: &[String]) -> String {
    path.join("-")
}

/// Is this a name Just can carry as a recipe? Just recipe names are identifiers, and a
/// name that is not one would be a file the parser rejects rather than a recipe nobody
/// notices.
///
/// ```
/// use majordomus_cli::command::projection::is_recipe_name;
/// assert!(is_recipe_name("bench-coverage"));
/// assert!(!is_recipe_name("-leading"));
/// assert!(!is_recipe_name("with space"));
/// assert!(!is_recipe_name("semi;colon"));
/// ```
pub fn is_recipe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Names Just itself uses, which a generated recipe may not take.
const RESERVED: &[&str] = &["alias", "export", "import", "mod", "set", "if", "else"];

/// The Cockpit's action name for a command: the identity, namespaced, so that the
/// Cockpit's routes are stable across every rename of every surface.
///
/// ```
/// use majordomus_cli::command::projection::cockpit_action;
/// use majordomus_cli::command::CommandId;
/// let id = CommandId::parse("native.bench.coverage").unwrap();
/// assert_eq!(cockpit_action(&id), "command/native.bench.coverage");
/// ```
pub fn cockpit_action(id: &CommandId) -> String {
    format!("command/{id}")
}

/// The route of a command's page in the generated reference, derived from the identity so
/// that no filename is ever chosen by hand.
///
/// ```
/// use majordomus_cli::command::projection::docs_route;
/// use majordomus_cli::command::CommandId;
/// let id = CommandId::parse("shell.doctor").unwrap();
/// assert_eq!(docs_route(&id), "/docs/commands/shell.doctor/");
/// ```
pub fn docs_route(id: &CommandId) -> String {
    format!("{DOCS_PREFIX}/{id}/")
}

/// Assign every node the names its surfaces know it by, and report every collision the
/// assignment had to resolve.
///
/// The nodes are visited in a fixed order — program precedence, then identity — so the
/// result is a function of the set of commands and nothing else.
pub fn assign(nodes: &mut [CommandNode]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|a, b| {
        nodes[*a]
            .program
            .precedence()
            .cmp(&nodes[*b].program.precedence())
            .then_with(|| nodes[*a].id.cmp(&nodes[*b].id))
    });

    let mut taken: BTreeMap<String, CommandId> = BTreeMap::new();
    for i in order.iter().copied() {
        let id = nodes[i].id.clone();
        let program = nodes[i].program;
        let base = base_name(&nodes[i].path);
        let name = claim(&base, program, &id, &mut taken, &mut diagnostics, "recipe");
        nodes[i].projections = Projections {
            cli: matches!(program, Program::Native).then(|| nodes[i].path.clone()),
            just: name,
            cockpit: Some(cockpit_action(&id)),
            docs: Some(docs_route(&id)),
            just_aliases: Vec::new(),
        };
    }

    // Aliases are claimed only after every command has its own name: a compatibility
    // alias may never take the name of a command that exists.
    for i in order.iter().copied() {
        let id = nodes[i].id.clone();
        let program = nodes[i].program;
        let alias_names: Vec<String> = nodes[i].aliases.iter().map(|a| a.name.clone()).collect();
        let mut kept = Vec::new();
        for alias in alias_names {
            match claim(&alias, program, &id, &mut taken, &mut diagnostics, "alias") {
                Some(name) if name == alias => kept.push(name),
                Some(_) => diagnostics.push(Diagnostic {
                    code: "alias-taken".into(),
                    fatal: false,
                    message: format!(
                        "the alias '{alias}' of {id} is a name another command already has; the alias is not projected"
                    ),
                    commands: vec![id.clone(), taken[&alias].clone()],
                }),
                None => {}
            }
        }
        nodes[i].projections.just_aliases = kept;
    }
    diagnostics
}

/// Claim a name for a command, taking the program's prefix when the bare name is gone and
/// reporting what happened. `None` when no legal name could be formed at all.
fn claim(
    wanted: &str,
    program: Program,
    id: &CommandId,
    taken: &mut BTreeMap<String, CommandId>,
    diagnostics: &mut Vec<Diagnostic>,
    what: &str,
) -> Option<String> {
    let mut candidate = wanted.to_string();
    if !is_recipe_name(&candidate) || RESERVED.contains(&candidate.as_str()) {
        candidate = format!("{}-{}", program.projection_prefix(), sanitise(wanted));
        diagnostics.push(Diagnostic {
            code: "projection-renamed".into(),
            fatal: false,
            message: format!(
                "the {what} name '{wanted}' of {id} is not one Just can carry; it is projected as '{candidate}'"
            ),
            commands: vec![id.clone()],
        });
    }
    if let Some(holder) = taken.get(&candidate) {
        if holder == id {
            return Some(candidate);
        }
        let prefixed = format!("{}-{}", program.projection_prefix(), candidate);
        if let Some(other) = taken.get(&prefixed) {
            diagnostics.push(Diagnostic {
                code: "projection-collision".into(),
                fatal: true,
                message: format!(
                    "'{candidate}' is taken by {holder} and '{prefixed}' by {other}; {id} has no {what} name left"
                ),
                commands: vec![id.clone(), holder.clone(), other.clone()],
            });
            return None;
        }
        if what == "recipe" {
            diagnostics.push(Diagnostic {
                code: "projection-prefixed".into(),
                fatal: false,
                message: format!(
                    "'{candidate}' is {holder}'s by program precedence; {id} is projected as '{prefixed}'"
                ),
                commands: vec![id.clone(), holder.clone()],
            });
        }
        taken.insert(prefixed.clone(), id.clone());
        return Some(prefixed);
    }
    taken.insert(candidate.clone(), id.clone());
    Some(candidate)
}

/// A name Just can carry, from one it cannot: everything outside the identifier set
/// becomes a hyphen, and runs of hyphens collapse.
fn sanitise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
            out.push(c);
        } else if c.is_ascii_uppercase() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::model::{
        Availability, EffectClass, Execution, Interactivity, Provenance, Visibility,
    };

    fn node(program: Program, path: &[&str], aliases: &[&str]) -> CommandNode {
        let path: Vec<String> = path.iter().map(|s| (*s).to_string()).collect();
        CommandNode {
            id: CommandId::new(program, &path),
            program,
            summary: "x".into(),
            description: None,
            usage: "x".into(),
            runnable: true,
            arguments: Vec::new(),
            execution: Execution::Native { argv: path.clone() },
            effect: EffectClass::ReadOnly,
            interactivity: Interactivity::NonInteractive,
            visibility: Visibility::Public,
            availability: Availability::always(),
            provenance: Provenance::Clap { path: "x".into() },
            aliases: aliases
                .iter()
                .map(|a| super::super::model::Alias {
                    name: (*a).to_string(),
                    reason: "test".into(),
                })
                .collect(),
            deprecation: None,
            tags: Vec::new(),
            examples: Vec::new(),
            path,
            projections: Projections::default(),
        }
    }

    fn just_names(nodes: &[CommandNode]) -> Vec<(String, String)> {
        nodes
            .iter()
            .map(|n| {
                (
                    n.id.to_string(),
                    n.projections.just.clone().unwrap_or_default(),
                )
            })
            .collect()
    }

    #[test]
    fn the_path_becomes_the_recipe_name() {
        let mut nodes = vec![node(Program::Native, &["bench", "coverage"], &[])];
        assert!(assign(&mut nodes).is_empty());
        assert_eq!(nodes[0].projections.just.as_deref(), Some("bench-coverage"));
        assert_eq!(
            nodes[0].projections.cockpit.as_deref(),
            Some("command/native.bench.coverage")
        );
        assert_eq!(
            nodes[0].projections.docs.as_deref(),
            Some("/docs/commands/native.bench.coverage/")
        );
        assert_eq!(
            nodes[0].projections.cli,
            Some(vec!["bench".to_string(), "coverage".to_string()])
        );
    }

    #[test]
    fn precedence_decides_a_collision_and_says_so() {
        let mut nodes = vec![
            node(Program::Shell, &["bench"], &[]),
            node(Program::Native, &["bench"], &[]),
        ];
        let diagnostics = assign(&mut nodes);
        let names = just_names(&nodes);
        assert!(names.contains(&("native.bench".into(), "bench".into())), "{names:?}");
        assert!(names.contains(&("shell.bench".into(), "mj-bench".into())), "{names:?}");
        let d = diagnostics
            .iter()
            .find(|d| d.code == "projection-prefixed")
            .expect("the collision is reported");
        assert!(!d.fatal);
        assert!(d.message.contains("native.bench") && d.message.contains("shell.bench"));
    }

    #[test]
    fn the_result_does_not_depend_on_the_order_the_contributors_ran_in() {
        let a = {
            let mut n = vec![
                node(Program::Shell, &["bench"], &[]),
                node(Program::Native, &["bench"], &[]),
            ];
            assign(&mut n);
            just_names(&n)
        };
        let b = {
            let mut n = vec![
                node(Program::Native, &["bench"], &[]),
                node(Program::Shell, &["bench"], &[]),
            ];
            assign(&mut n);
            let mut names = just_names(&n);
            names.reverse();
            names
        };
        assert_eq!(a, b);
    }

    #[test]
    fn an_alias_never_takes_a_name_a_command_already_has() {
        let mut nodes = vec![
            node(Program::Native, &["bench"], &[]),
            node(Program::Workflow, &["bench-criterion"], &["bench"]),
        ];
        let diagnostics = assign(&mut nodes);
        let workflow = nodes
            .iter()
            .find(|n| n.program == Program::Workflow)
            .unwrap();
        assert!(
            workflow.projections.just_aliases.is_empty(),
            "the alias is refused, not silently redirected"
        );
        assert!(diagnostics.iter().any(|d| d.code == "alias-taken"));
    }

    #[test]
    fn a_free_alias_is_projected() {
        let mut nodes = vec![node(Program::Native, &["bench"], &["bench-run"])];
        assign(&mut nodes);
        assert_eq!(nodes[0].projections.just_aliases, vec!["bench-run"]);
    }

    #[test]
    fn a_name_just_cannot_carry_is_renamed_and_reported() {
        let mut nodes = vec![node(Program::Workflow, &["Bad Name;rm"], &[])];
        let diagnostics = assign(&mut nodes);
        let name = nodes[0].projections.just.clone().unwrap();
        assert!(is_recipe_name(&name), "{name}");
        assert!(diagnostics.iter().any(|d| d.code == "projection-renamed"));
    }

    #[test]
    fn a_reserved_word_is_not_taken() {
        let mut nodes = vec![node(Program::Workflow, &["import"], &[])];
        assign(&mut nodes);
        assert_eq!(nodes[0].projections.just.as_deref(), Some("wf-import"));
    }
}
