//! The native contributor: the Rust executable's own commands, walked out of the clap
//! declaration.
//!
//! clap is the one declaration of the native command line — every command, argument,
//! default and accepted value — and [`crate::cli::tree`] already walks it into
//! [`crate::cli::CommandDoc`] for `--help`, the reference and the site. This contributor
//! reads that same tree and adds only what clap cannot carry: the effect, the
//! interactivity, the requirements and the aliases, from [`crate::cli::SEMANTICS`].
//!
//! Nothing here restates a path, a description, an argument or a default. A command added
//! to the clap declaration appears in the graph — and therefore in every projection — on
//! the same edit that adds it.

use crate::cli::{ArgDoc, CommandDoc};

use super::model::{
    ArgumentSpec, Availability, CommandId, CommandNode, EnumValue, Example, Execution,
    Projections, Program, Provenance, Sensitivity, ValueSource, Visibility,
};
use super::semantics;

/// Every native command, root first and then depth first, as canonical nodes. The
/// availability of each is left unresolved here — [`super::graph`] resolves it once
/// against the facts of the checkout, so that the contributor stays pure and testable.
pub fn contribute(tree: &CommandDoc) -> Vec<CommandNode> {
    tree.flatten().iter().filter_map(|c| node(c)).collect()
}

/// The canonical node of one clap command, or `None` for the root: the executable itself
/// is not a command, and `majordomus` with nothing after it prints help.
fn node(doc: &CommandDoc) -> Option<CommandNode> {
    let path: Vec<String> = doc.path.iter().skip(1).cloned().collect();
    if path.is_empty() {
        return None;
    }
    let key = path.join(" ");
    let declared = semantics::lookup(&key);
    let id = CommandId::new(Program::Native, &path);
    Some(CommandNode {
        summary: doc.about.clone(),
        description: doc.long_about.clone(),
        usage: doc.usage.clone(),
        runnable: doc.executable,
        arguments: doc.args.iter().map(argument).collect(),
        execution: Execution::Native { argv: path.clone() },
        effect: declared
            .map(|s| s.effect)
            .unwrap_or(super::model::EffectClass::ReadOnly),
        interactivity: declared.map(|s| s.interactivity).unwrap_or_default(),
        visibility: declared.map(|s| s.visibility).unwrap_or(Visibility::Public),
        availability: Availability {
            available: true,
            requires: declared
                .map(|s| s.requirements())
                .unwrap_or_else(|| vec![super::model::Requirement::Always]),
            reason: None,
        },
        provenance: Provenance::Clap {
            path: crate::cli::DECLARATION.to_string(),
        },
        aliases: declared.map(|s| s.alias_values()).unwrap_or_default(),
        deprecation: declared.and_then(|s| {
            s.deprecation_note().map(|mut d| {
                d.replaced_by = s
                    .replacement_path()
                    .map(|p| CommandId::new(Program::Native, &words(p)));
                d
            })
        }),
        tags: declared
            .map(|s| s.tags.iter().map(|t| (*t).to_string()).collect())
            .unwrap_or_default(),
        examples: doc
            .examples
            .iter()
            .map(|e| Example {
                id: e.id.clone(),
                title: e.title.clone(),
                argv: e.argv.clone(),
                command: e.command.clone(),
            })
            .collect(),
        program: Program::Native,
        path,
        id,
        projections: Projections::default(),
    })
}

fn words(path: &str) -> Vec<String> {
    path.split_whitespace().map(str::to_string).collect()
}

/// One clap argument as a canonical argument. The value source comes from the
/// placeholder the declaration already writes, and an argument whose type enumerates its
/// values carries them: neither is declared a second time for completion's sake.
fn argument(a: &ArgDoc) -> ArgumentSpec {
    let values = if !a.possible_values.is_empty() {
        ValueSource::Enumerated {
            values: a
                .possible_values
                .iter()
                .map(|v| EnumValue {
                    value: v.name.clone(),
                    description: v.help.clone(),
                })
                .collect(),
        }
    } else if a.takes_value {
        a.value_name
            .as_deref()
            .map(ValueSource::of_placeholder)
            .unwrap_or(ValueSource::None)
    } else {
        ValueSource::None
    };
    ArgumentSpec {
        name: a.name.clone(),
        long: a.long.clone(),
        short: a.short,
        positional: a.positional,
        required: a.required,
        variadic: a.variadic,
        global: a.global,
        takes_value: a.takes_value,
        help: a.help.clone(),
        placeholder: a.value_name.clone(),
        defaults: a.defaults.clone(),
        values,
        // The native command line takes no credential: every argument of it names a
        // path, an identity or a shape. A future argument that did would be marked in
        // the declaration, and `cli::validate` is where that would be enforced.
        sensitivity: Sensitivity::Public,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::model::EffectClass;

    #[test]
    fn every_native_command_of_the_declaration_becomes_a_node_and_the_root_does_not() {
        let tree = crate::cli::tree();
        let nodes = contribute(&tree);
        assert_eq!(nodes.len(), tree.flatten().len() - 1, "the root is not a command");
        assert!(nodes.iter().all(|n| n.program == Program::Native));
        assert!(nodes.iter().all(|n| !n.path.is_empty()));
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"native.capabilities.list"), "{ids:?}");
        assert!(ids.contains(&"native.bench.baseline.update"), "{ids:?}");
    }

    #[test]
    fn arguments_and_their_accepted_values_come_from_the_declaration() {
        let tree = crate::cli::tree();
        let nodes = contribute(&tree);
        let list = nodes
            .iter()
            .find(|n| n.id.as_str() == "native.capabilities.list")
            .expect("capabilities list");
        let format = list
            .arguments
            .iter()
            .find(|a| a.long.as_deref() == Some("format"))
            .expect("--format");
        match &format.values {
            ValueSource::Enumerated { values } => {
                assert!(values.iter().any(|v| v.value == "json"), "{values:?}");
            }
            other => panic!("an enumerated argument carries its values, not {other:?}"),
        }
    }

    #[test]
    fn the_effect_comes_from_the_declaration_beside_the_command() {
        let tree = crate::cli::tree();
        let nodes = contribute(&tree);
        let generate = nodes
            .iter()
            .find(|n| n.id.as_str() == "native.generate")
            .expect("generate");
        assert_eq!(
            generate.effect,
            EffectClass::GeneratedOutput,
            "generate rewrites committed projections and says so"
        );
        let list = nodes
            .iter()
            .find(|n| n.id.as_str() == "native.capabilities.list")
            .expect("capabilities list");
        assert!(list.effect.is_read_only());
    }

    #[test]
    fn a_documented_example_reaches_the_node_without_being_written_again() {
        let tree = crate::cli::tree();
        let nodes = contribute(&tree);
        let show = nodes
            .iter()
            .find(|n| n.id.as_str() == "native.distribution.show")
            .expect("distribution show");
        assert!(!show.examples.is_empty());
        assert!(show.examples[0].command.starts_with("majordomus distribution"));
    }
}
