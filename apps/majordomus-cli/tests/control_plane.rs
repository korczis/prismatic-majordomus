//! The invariant the control plane exists for: a command is declared once, and every
//! surface that spells it picks it up without being edited.
//!
//! The test is deliberately hostile to the architecture. It composes a graph over a
//! command line this crate does not ship — an `acme foo` that exists nowhere in the source
//! — and then asks each projection whether it knows about it. Nothing in `control::just`,
//! `control::completion`, `control::projection` or any surface has a name of this command,
//! a description of it, or a branch for it; if any of them had to be told, one of these
//! assertions fails.

use majordomus_cli::capability::CapabilityRegistry;
use majordomus_cli::cli::{ArgDoc, CommandDoc, PossibleValueDoc};
use majordomus_cli::control::completion::{self, Offline, Request};
use majordomus_cli::control::effect::{EffectClass, Semantics};
use majordomus_cli::control::graph::{self, Declaration};
use majordomus_cli::control::{just, Surface};

/// A command line with one namespace and two commands under it: one that reads, one that
/// destroys. Neither exists anywhere in this crate.
fn synthetic() -> CommandDoc {
    let leaf = |name: &str, about: &str, args: Vec<ArgDoc>| CommandDoc {
        path: vec!["majordomus".into(), "acme".into(), name.into()],
        route: format!("/docs/cli/acme/{name}/"),
        executable: true,
        usage: format!("majordomus acme {name}"),
        about: about.into(),
        long_about: None,
        args,
        examples: Vec::new(),
        subcommands: Vec::new(),
    };
    let shape = ArgDoc {
        name: "shape".into(),
        long: Some("shape".into()),
        short: None,
        positional: false,
        help: "How the answer is shaped".into(),
        value_name: Some("SHAPE".into()),
        takes_value: true,
        required: false,
        global: false,
        possible_values: vec![
            PossibleValueDoc {
                name: "round".into(),
                help: Some("a circle".into()),
            },
            PossibleValueDoc {
                name: "square".into(),
                help: None,
            },
        ],
        defaults: vec!["round".into()],
    };
    CommandDoc {
        path: vec!["majordomus".into()],
        route: "/docs/cli/".into(),
        executable: false,
        usage: "majordomus <COMMAND>".into(),
        about: "the executable".into(),
        long_about: None,
        args: Vec::new(),
        examples: Vec::new(),
        subcommands: vec![CommandDoc {
            path: vec!["majordomus".into(), "acme".into()],
            route: "/docs/cli/acme/".into(),
            executable: false,
            usage: "majordomus acme <COMMAND>".into(),
            about: "the acme namespace".into(),
            long_about: None,
            args: Vec::new(),
            examples: Vec::new(),
            subcommands: vec![
                leaf("foo", "Answer with a shape", vec![shape]),
                leaf("burn", "Delete the shapes", Vec::new()),
            ],
        }],
    }
}

fn declarations() -> Vec<Declaration<'static>> {
    vec![
        Declaration {
            command: "acme foo",
            semantics: Semantics::read_only(),
            aliases: &["acme-legacy-name"],
        },
        Declaration {
            command: "acme burn",
            semantics: Semantics::of(EffectClass::Destructive),
            aliases: &[],
        },
    ]
}

fn graph() -> graph::CommandGraph {
    let registry = CapabilityRegistry::builder()
        .build()
        .expect("an empty registry is a registry");
    graph::compose(&synthetic(), &registry, &[], &declarations())
}

#[test]
fn one_declaration_reaches_every_surface_without_a_surface_being_edited() {
    let g = graph();
    assert!(g.is_valid(), "{:?}", g.diagnostics);

    // the graph
    let foo = g.find("acme.foo").expect("the command is in the graph");
    assert_eq!(foo.summary, "Answer with a shape");

    // the command line, and the documentation route
    assert_eq!(foo.projections.cli, vec!["acme", "foo"]);
    assert_eq!(foo.projections.docs, "/docs/cli/acme/foo/");

    // the just bridge: the recipe, its description, its group, and its alias
    let bridge = just::render(&g);
    assert!(bridge.contains("\nacme-foo *args:\n"), "{bridge}");
    assert!(bridge.contains("# Answer with a shape\n"), "{bridge}");
    assert!(bridge.contains("[group('acme')]"), "{bridge}");
    assert!(
        bridge.contains("alias acme-legacy-name := acme-foo"),
        "{bridge}"
    );

    // machine surfaces, for the read
    assert_eq!(foo.projections.mcp.as_deref(), Some("majordomus_acme_foo"));
    assert_eq!(foo.projections.cockpit.as_deref(), Some("acme.foo"));

    // completion, at the command position and at the value position
    let at_command = completion::answer(
        &g,
        &Request::new(Surface::Cli, vec!["acme".into(), String::new()], 1),
        &Offline,
    );
    let offered: Vec<&str> = at_command
        .candidates
        .iter()
        .map(|c| c.value.as_str())
        .collect();
    assert!(offered.contains(&"foo"), "{offered:?}");

    let at_value = completion::answer(
        &g,
        &Request::new(
            Surface::Cli,
            vec!["acme".into(), "foo".into(), "--shape".into(), String::new()],
            3,
        ),
        &Offline,
    );
    let values: Vec<&str> = at_value
        .candidates
        .iter()
        .map(|c| c.value.as_str())
        .collect();
    assert_eq!(
        values,
        vec!["round", "square"],
        "the declared values, and only those"
    );

    // and the same request against the just surface resolves to the same answer
    let by_just = completion::answer(
        &g,
        &Request::new(
            Surface::Just,
            vec!["acme-foo".into(), "--shape".into(), String::new()],
            2,
        ),
        &Offline,
    );
    assert_eq!(by_just.candidates, at_value.candidates);
}

#[test]
fn the_classification_alone_decides_what_a_machine_is_offered() {
    let g = graph();
    let burn = g.find("acme.burn").expect("the command is in the graph");
    assert!(
        burn.projections.mcp.is_none(),
        "a destructive command is not a tool"
    );
    assert!(burn.projections.cockpit.is_none());
    // it is still on the command line and in the bridge, where a person is asking
    assert_eq!(burn.projections.just.as_deref(), Some("acme-burn"));
    let bridge = just::render(&g);
    let block = bridge
        .split("\n\n")
        .find(|b| b.contains("\nacme-burn *args:"))
        .expect("the recipe");
    assert!(block.contains("[confirm("), "{block}");
}

#[test]
fn a_command_that_does_not_say_what_it_changes_fails_the_graph() {
    let registry = CapabilityRegistry::builder().build().expect("a registry");
    // the same command line, with the declaration for `acme foo` withheld
    let partial = vec![Declaration {
        command: "acme burn",
        semantics: Semantics::of(EffectClass::Destructive),
        aliases: &[],
    }];
    let g = graph::compose(&synthetic(), &registry, &[], &partial);
    assert!(!g.is_valid());
    let d = g
        .diagnostics
        .iter()
        .find(|d| d.code == "COMMAND_WITHOUT_SEMANTICS")
        .expect("the diagnostic");
    assert!(d.subject.contains("acme foo"), "{d:?}");
}

#[test]
fn two_commands_that_would_answer_to_one_spelling_are_refused_with_both_named() {
    let registry = CapabilityRegistry::builder().build().expect("a registry");
    // `acme foo` and `acme-foo` both project to the recipe `acme-foo`
    let mut tree = synthetic();
    let colliding = CommandDoc {
        path: vec!["majordomus".into(), "acme-foo".into()],
        route: "/docs/cli/acme-foo/".into(),
        executable: true,
        usage: "majordomus acme-foo".into(),
        about: "a command whose name collides with a nested one".into(),
        long_about: None,
        args: Vec::new(),
        examples: Vec::new(),
        subcommands: Vec::new(),
    };
    tree.subcommands.push(colliding);
    let mut declared = declarations();
    declared.push(Declaration {
        command: "acme-foo",
        semantics: Semantics::read_only(),
        aliases: &[],
    });
    let g = graph::compose(&tree, &registry, &[], &declared);
    let d = g
        .diagnostics
        .iter()
        .find(|d| d.code == "PROJECTION_COLLISION")
        .expect("the collision is reported");
    assert!(d.detail.contains("acme.foo"), "{d:?}");
    assert!(d.detail.contains("acme-foo"), "{d:?}");
    assert!(
        !g.is_valid(),
        "a collision is fatal, never resolved by order"
    );
}
