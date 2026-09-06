//! The refusals of the command line's documentation contract, each one provoked.
//!
//! `cli::validate` is exercised elsewhere against the real declaration, which is complete —
//! so every branch that *reports* something is unreached by that test, and a refusal nothing
//! runs is a refusal nobody knows still works. A validator is only worth its exit code if
//! each of its findings has been seen to fire.
//!
//! The trees below are deliberately broken copies built in memory. Nothing here touches the
//! real declaration, so a command added tomorrow changes none of it.

use majordomus_cli::cli::{self, ArgDoc, CommandDoc, ExampleView, PossibleValueDoc, SetupView};

/// A minimal command that satisfies the contract, to be broken one field at a time.
fn ok_command(name: &str) -> CommandDoc {
    CommandDoc {
        path: vec!["majordomus".to_string(), name.to_string()],
        route: format!("/docs/cli/{name}/"),
        executable: true,
        usage: format!("majordomus {name}"),
        about: "does a thing".to_string(),
        long_about: Some("does a thing, at length".to_string()),
        args: Vec::new(),
        examples: vec![ok_example(name)],
        subcommands: Vec::new(),
    }
}

fn ok_example(command: &str) -> ExampleView {
    ExampleView {
        id: format!("{command}-example"),
        title: "An example".to_string(),
        description: "What it shows".to_string(),
        argv: vec![command.to_string()],
        command: format!("majordomus {command}"),
        setup: Vec::new(),
        expectation: "prints something a reader can check".to_string(),
    }
}

fn arg(name: &str, help: &str) -> ArgDoc {
    ArgDoc {
        name: name.to_string(),
        long: Some(name.to_string()),
        short: None,
        positional: false,
        help: help.to_string(),
        value_name: Some("VALUE".to_string()),
        takes_value: true,
        required: false,
        global: false,
        possible_values: Vec::new(),
        defaults: Vec::new(),
    }
}

/// The root of a tree, with whatever children are given.
fn root(children: Vec<CommandDoc>) -> CommandDoc {
    CommandDoc {
        path: vec!["majordomus".to_string()],
        route: "/docs/cli/".to_string(),
        executable: false,
        usage: "majordomus <COMMAND>".to_string(),
        about: "the tool".to_string(),
        long_about: Some("the tool, at length".to_string()),
        args: Vec::new(),
        examples: Vec::new(),
        subcommands: children,
    }
}

/// The codes reported for a tree, sorted, so an assertion names what it expects.
fn codes(tree: &CommandDoc) -> Vec<&'static str> {
    let mut c: Vec<&'static str> = cli::validate(tree).into_iter().map(|v| v.code).collect();
    c.sort_unstable();
    c.dedup();
    c
}

/// Whether a code is among the findings. The real EXAMPLES set is validated alongside every
/// tree, so a tree's own findings are asserted by containment rather than by equality.
fn reports(tree: &CommandDoc, code: &str) -> bool {
    cli::validate(tree).iter().any(|v| v.code == code)
}

// ---------------------------------------------------------------- the commands themselves

#[test]
fn a_command_without_a_description_is_reported() {
    let mut c = ok_command("silent");
    c.about = "   ".to_string();
    let tree = root(vec![c]);
    assert!(
        reports(&tree, "CLI_DOC_MISSING_ABOUT"),
        "a command with no one-line description must be reported: {:?}",
        codes(&tree)
    );
}

#[test]
fn the_root_without_a_long_description_is_reported() {
    let mut tree = root(vec![ok_command("thing")]);
    tree.long_about = None;
    assert!(
        reports(&tree, "CLI_DOC_MISSING_LONG_ABOUT"),
        "the root carries the long description of the executable: {:?}",
        codes(&tree)
    );

    // and a whitespace-only one is the same absence, not a description
    let mut blank = root(vec![ok_command("thing")]);
    blank.long_about = Some("  \n ".to_string());
    assert!(reports(&blank, "CLI_DOC_MISSING_LONG_ABOUT"));
}

// ---------------------------------------------------------------- the arguments

#[test]
fn an_argument_without_help_is_reported() {
    let mut c = ok_command("thing");
    c.args = vec![arg("mute", "  ")];
    let tree = root(vec![c]);
    assert!(
        reports(&tree, "CLI_DOC_EMPTY_ARG_HELP"),
        "every argument says what it is for: {:?}",
        codes(&tree)
    );
}

#[test]
fn an_enumerated_value_without_help_is_reported() {
    let mut c = ok_command("thing");
    let mut a = arg("mode", "how to do it");
    a.possible_values = vec![
        PossibleValueDoc {
            name: "explained".to_string(),
            help: Some("what this one means".to_string()),
        },
        PossibleValueDoc {
            name: "bare".to_string(),
            help: None,
        },
    ];
    c.args = vec![a];
    let tree = root(vec![c]);
    let found = cli::validate(&tree);
    assert!(
        found.iter().any(|v| v.code == "CLI_DOC_EMPTY_VALUE_HELP"),
        "an accepted value must say what it means: {:?}",
        codes(&tree)
    );
    // the finding names the value that is bare, not the one that is documented
    let detail = found
        .iter()
        .find(|v| v.code == "CLI_DOC_EMPTY_VALUE_HELP")
        .map(|v| v.detail.clone())
        .unwrap_or_default();
    assert!(
        detail.contains("bare") && !detail.contains("explained"),
        "the finding names the undocumented value: {detail}"
    );
}

// ---------------------------------------------------------------- the examples

#[test]
fn a_runnable_command_without_an_example_is_reported() {
    let mut c = ok_command("bare");
    c.examples = Vec::new();
    let tree = root(vec![c]);
    assert!(
        reports(&tree, "CLI_DOC_MISSING_EXAMPLE"),
        "every command a person can run carries an example: {:?}",
        codes(&tree)
    );
}

#[test]
fn an_example_on_a_command_that_only_groups_others_is_reported() {
    let mut c = ok_command("group");
    c.executable = false;
    let tree = root(vec![c]);
    assert!(
        reports(&tree, "CLI_DOC_EXAMPLE_ON_GROUP"),
        "an example belongs to a command that can be run: {:?}",
        codes(&tree)
    );
}

#[test]
fn an_example_without_a_title_or_a_description_is_reported() {
    let mut c = ok_command("thing");
    c.examples[0].title = " ".to_string();
    assert!(reports(&root(vec![c.clone()]), "CLI_DOC_EMPTY_EXAMPLE"));

    let mut d = ok_command("thing");
    d.examples[0].description = String::new();
    assert!(
        reports(&root(vec![d]), "CLI_DOC_EMPTY_EXAMPLE"),
        "an example says what it shows and what comes back"
    );
}

#[test]
fn an_example_the_parser_refuses_is_reported() {
    let mut c = ok_command("thing");
    c.examples[0].argv = vec!["thing".to_string(), "--no-such-flag".to_string()];
    let tree = root(vec![c]);
    assert!(
        reports(&tree, "CLI_DOC_EXAMPLE_DOES_NOT_PARSE"),
        "an example parses through the command line it documents: {:?}",
        codes(&tree)
    );
}

#[test]
fn a_setup_step_the_parser_refuses_is_reported() {
    let mut c = ok_command("thing");
    c.examples[0].setup = vec![SetupView {
        argv: vec!["thing".to_string(), "--not-a-flag".to_string()],
        command: "majordomus thing --not-a-flag".to_string(),
    }];
    let tree = root(vec![c]);
    assert!(
        reports(&tree, "CLI_DOC_EXAMPLE_DOES_NOT_PARSE"),
        "a setup step is held to the parser exactly as the example is: {:?}",
        codes(&tree)
    );
}

#[test]
fn an_example_that_runs_another_command_is_reported() {
    let mut c = ok_command("thing");
    c.examples[0].argv = vec!["scope".to_string()];
    let tree = root(vec![c]);
    assert!(
        reports(&tree, "CLI_DOC_EXAMPLE_WRONG_COMMAND"),
        "an example of a command runs that command: {:?}",
        codes(&tree)
    );
}

// ---------------------------------------------------------------- the parser itself

#[test]
fn the_parser_accepts_what_the_declaration_declares_and_refuses_the_rest() {
    assert!(
        cli::parse(&["scope".to_string()]).is_ok(),
        "a real command must parse"
    );
    assert!(
        cli::parse(&["not-a-command".to_string()]).is_err(),
        "a command the declaration does not have must not parse"
    );
    // the argv is taken as given: the parser is handed the vector a reader would type,
    // with the executable's own name supplied by the function rather than by the caller
    assert!(cli::parse(&[]).is_err(), "no command is not a run");
}

// ---------------------------------------------------------------- the finding renders

#[test]
fn a_finding_says_the_code_the_command_the_file_and_the_rule() {
    let mut c = ok_command("silent");
    c.about = String::new();
    let found = cli::validate(&root(vec![c]));
    let v = found
        .iter()
        .find(|v| v.code == "CLI_DOC_MISSING_ABOUT")
        .expect("the missing description is reported");
    let rendered = v.to_string();
    for part in [
        "CLI_DOC_MISSING_ABOUT",
        "command:",
        "source:",
        "detail:",
        "rule:",
    ] {
        assert!(
            rendered.contains(part),
            "a finding a person can act on names {part}: {rendered}"
        );
    }
    assert!(
        rendered.contains("majordomus silent"),
        "the finding names the command it is about: {rendered}"
    );
}

// ---------------------------------------------------------------- the real declaration

#[test]
fn the_declaration_this_crate_ships_has_no_violations() {
    let found = cli::validate(&cli::tree());
    assert!(
        found.is_empty(),
        "the shipped command line must satisfy its own contract: {}",
        found
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
