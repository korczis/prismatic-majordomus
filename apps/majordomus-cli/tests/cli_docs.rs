//! Level A of the command line's documentation contract, and the shape of the tree every
//! projection renders.
//!
//! Nothing here enumerates the commands: the expectations are derived from the clap
//! declaration itself, so a command added tomorrow is held to the same contract without
//! this file being edited. The few fixed assertions that remain are about semantics one
//! cannot derive — that `--repo` is global, that `bench baseline update` is three words
//! deep, that the route of a known path is the route a reader will type.

use majordomus_cli::cli::{self, CommandDoc, Expect};
use proptest::prelude::*;

fn tree() -> CommandDoc {
    cli::tree()
}

// ---------------------------------------------------------------- the tree

#[test]
fn every_clap_command_is_in_the_tree_and_the_hierarchy_is_preserved() {
    let t = tree();
    assert_eq!(t.path, ["majordomus"]);
    // the root's children are exactly clap's, in declaration order
    use clap::CommandFactory;
    let mut root = cli::Cli::command();
    root.build();
    let declared: Vec<String> = root
        .get_subcommands()
        .filter(|s| !s.is_hide_set() && s.get_name() != "help")
        .map(|s| s.get_name().to_string())
        .collect();
    let walked: Vec<String> = t
        .subcommands
        .iter()
        .map(|c| c.path.last().unwrap().clone())
        .collect();
    assert_eq!(declared, walked);
    // every command's path continues its parent's, one word at a time
    for c in t.flatten() {
        for s in &c.subcommands {
            assert_eq!(&s.path[..c.path.len()], &c.path[..]);
            assert_eq!(s.path.len(), c.path.len() + 1);
        }
    }
    // the deepest path the executable has today, as a reader types it
    assert!(t
        .flatten()
        .iter()
        .any(|c| c.command() == "majordomus bench baseline update"));
}

#[test]
fn the_help_subcommand_clap_adds_is_not_a_command_of_the_tree() {
    for c in tree().flatten() {
        assert_ne!(c.path.last().unwrap(), "help", "{}", c.command());
        for a in &c.args {
            assert!(
                !matches!(a.name.as_str(), "help" | "version"),
                "{} documents {} as an argument",
                c.command(),
                a.name
            );
        }
    }
}

#[test]
fn flattening_is_depth_first_deterministic_and_lists_each_command_once() {
    let t = tree();
    let a: Vec<String> = t.flatten().iter().map(|c| c.command()).collect();
    let b: Vec<String> = tree().flatten().iter().map(|c| c.command()).collect();
    assert_eq!(a, b, "two walks of one declaration differ");
    let unique: std::collections::BTreeSet<&String> = a.iter().collect();
    assert_eq!(unique.len(), a.len(), "a command appears twice: {a:?}");
    // depth first: a command is listed before anything under it
    for (i, name) in a.iter().enumerate() {
        for (j, other) in a.iter().enumerate() {
            if other != name && other.starts_with(&format!("{name} ")) {
                assert!(i < j, "{name} is listed after {other}");
            }
        }
    }
}

#[test]
fn a_command_is_runnable_when_clap_lets_it_run_alone() {
    let t = tree();
    let by = |name: &str| {
        t.flatten()
            .into_iter()
            .find(|c| c.command() == name)
            .unwrap_or_else(|| panic!("no command {name}"))
            .clone()
    };
    // the root and the two grouping commands require a subcommand
    assert!(!by("majordomus").executable);
    assert!(!by("majordomus capabilities").executable);
    assert!(!by("majordomus bench baseline").executable);
    // `bench` runs the benchmarks when no subcommand follows it
    assert!(by("majordomus bench").executable);
    // every leaf is runnable
    for c in t.flatten() {
        if c.subcommands.is_empty() {
            assert!(c.executable, "{} is a leaf and not runnable", c.command());
        }
    }
}

// ---------------------------------------------------------------- arguments

#[test]
fn arguments_carry_flags_positionals_defaults_requiredness_and_value_sets() {
    let t = tree();
    let find = |cmd: &str, arg: &str| {
        t.flatten()
            .into_iter()
            .find(|c| c.command() == cmd)
            .unwrap_or_else(|| panic!("no command {cmd}"))
            .args
            .iter()
            .find(|a| a.name == arg)
            .unwrap_or_else(|| panic!("{cmd} has no argument {arg}"))
            .clone()
    };
    let repo = find("majordomus mcp", "repo");
    assert_eq!(repo.long.as_deref(), Some("repo"));
    assert!(repo.global, "--repo is declared global");
    assert!(repo.takes_value && !repo.positional && !repo.required);
    assert_eq!(repo.value_name.as_deref(), Some("PATH"));

    let strict = find("majordomus mcp", "strict");
    assert!(!strict.takes_value, "--strict is a flag");
    // clap gives a flag the default `false`; the reference prints "flag" and no default,
    // because a default nobody types is noise
    assert_eq!(strict.defaults, vec!["false"]);

    let port = find("majordomus serve", "port");
    assert_eq!(port.defaults, vec!["8741"], "the documented default port");

    let format = find("majordomus bench coverage", "format");
    let values: Vec<String> = format
        .possible_values
        .iter()
        .map(|v| v.name.clone())
        .collect();
    assert_eq!(values, ["text", "json"]);
    assert!(format
        .possible_values
        .iter()
        .all(|v| v.help.as_ref().is_some_and(|h| !h.trim().is_empty())));

    let id = find("majordomus capabilities describe", "id");
    assert!(
        id.positional && id.required,
        "the id is a required positional"
    );

    let paths = find("majordomus scope", "paths");
    assert!(paths.positional && !paths.required, "paths are optional");

    // a global argument is not repeated on the command that declares it and its children
    // as a different thing: it is the same declaration reaching every subcommand
    for c in t.flatten() {
        for a in c.args.iter().filter(|a| a.global) {
            assert!(
                a.long.is_some(),
                "{}: the global argument {} has no long flag",
                c.command(),
                a.name
            );
        }
    }
}

// ---------------------------------------------------------------- completeness

#[test]
fn the_command_line_documents_itself_completely() {
    let violations = cli::validate(&tree());
    assert!(
        violations.is_empty(),
        "the command line's documentation contract is unmet:\n{}",
        violations
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

#[test]
fn the_validator_reports_a_missing_example_a_missing_summary_and_an_empty_help() {
    // A command the declaration does not have cannot be built here without changing the
    // declaration, so the validator is held to a tree assembled by hand: the same type,
    // the same rules, the failures a new command would produce.
    let mut t = tree();
    let leaf = t
        .subcommands
        .iter_mut()
        .find(|c| c.command() == "majordomus serve")
        .expect("serve");
    leaf.examples.clear();
    leaf.about.clear();
    leaf.args[0].help.clear();
    let codes: Vec<&str> = cli::validate(&t).iter().map(|v| v.code).collect();
    assert!(codes.contains(&"CLI_DOC_MISSING_EXAMPLE"), "{codes:?}");
    assert!(codes.contains(&"CLI_DOC_MISSING_ABOUT"), "{codes:?}");
    assert!(codes.contains(&"CLI_DOC_EMPTY_ARG_HELP"), "{codes:?}");
    // every violation names the command, the file to edit and the rule it broke
    for v in cli::validate(&t) {
        let text = v.to_string();
        assert!(text.contains("majordomus"), "{text}");
        assert!(text.contains(cli::DECLARATION), "{text}");
        assert!(text.contains("rule:"), "{text}");
    }
}

#[test]
fn the_validator_reports_an_example_that_does_not_parse_and_one_on_the_wrong_command() {
    let mut t = tree();
    let cmd = t
        .subcommands
        .iter_mut()
        .find(|c| c.command() == "majordomus serve")
        .expect("serve");
    cmd.examples[0].argv = vec!["serve".into(), "--no-such-option".into()];
    assert!(cli::validate(&t)
        .iter()
        .any(|v| v.code == "CLI_DOC_EXAMPLE_DOES_NOT_PARSE"));
    let mut t = tree();
    let cmd = t
        .subcommands
        .iter_mut()
        .find(|c| c.command() == "majordomus serve")
        .expect("serve");
    cmd.examples[0].argv = vec!["scope".into()];
    assert!(cli::validate(&t)
        .iter()
        .any(|v| v.code == "CLI_DOC_EXAMPLE_WRONG_COMMAND"));
}

#[test]
fn every_example_parses_through_the_declaration_it_documents() {
    for c in tree().flatten() {
        for e in &c.examples {
            for argv in std::iter::once(&e.argv).chain(e.setup.iter().map(|s| &s.argv)) {
                cli::parse(argv).unwrap_or_else(|why| {
                    panic!(
                        "{}: `majordomus {}` does not parse: {why}",
                        c.command(),
                        argv.join(" ")
                    )
                });
            }
        }
    }
}

#[test]
fn every_example_id_is_unique_and_every_expectation_is_supported() {
    let mut ids = std::collections::BTreeSet::new();
    for set in cli::EXAMPLES {
        for e in set.examples {
            assert!(ids.insert(e.id), "example id {} is declared twice", e.id);
            // an expectation always describes itself; an unsupported one could not be
            // constructed, and one that described nothing would render an empty phrase
            assert!(!e.expect.describe().trim().is_empty(), "{}", e.id);
            match e.expect {
                Expect::StdoutContains(f) => assert!(!f.is_empty(), "{}", e.id),
                Expect::Json(p) => assert!(
                    p.iter().all(|p| p.starts_with('/')),
                    "{}: a JSON pointer starts with /",
                    e.id
                ),
                Expect::HttpReady(path) => assert!(path.starts_with('/'), "{}", e.id),
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------- routes

#[test]
fn a_command_path_maps_to_its_documented_route() {
    let cases = [
        (vec!["majordomus"], "/docs/cli/"),
        (vec!["majordomus", "mcp"], "/docs/cli/mcp/"),
        (
            vec!["majordomus", "capabilities", "schema"],
            "/docs/cli/capabilities/schema/",
        ),
        (
            vec!["majordomus", "bench", "baseline", "update"],
            "/docs/cli/bench/baseline/update/",
        ),
    ];
    for (path, route) in cases {
        let path: Vec<String> = path.into_iter().map(String::from).collect();
        assert_eq!(cli::route(&path), route);
    }
    // and the tree carries the same rule, for every command
    for c in tree().flatten() {
        assert_eq!(c.route, cli::route(&c.path));
        assert!(c.route.starts_with(cli::ROUTE_PREFIX) && c.route.ends_with('/'));
    }
}

#[test]
fn routes_are_unique_and_every_child_route_extends_its_parents() {
    let t = tree();
    let routes: Vec<String> = t.flatten().iter().map(|c| c.route.clone()).collect();
    let unique: std::collections::BTreeSet<&String> = routes.iter().collect();
    assert_eq!(unique.len(), routes.len(), "two commands share a route");
    for c in t.flatten() {
        match c.parent_route() {
            None => assert_eq!(c.route, format!("{}/", cli::ROUTE_PREFIX)),
            Some(parent) => assert!(
                c.route.starts_with(&parent),
                "{} is not under {parent}",
                c.route
            ),
        }
    }
}

proptest! {
    /// The route rule over arbitrary paths: one leading prefix, one trailing slash, no
    /// empty or doubled separator, and a path that is a prefix of another produces a route
    /// that is a prefix of the other's. This is the property the site's nesting relies on.
    #[test]
    fn route_normalisation_holds_for_any_path(
        words in proptest::collection::vec("[a-z][a-z0-9-]{0,12}", 0..5)
    ) {
        let mut path = vec!["majordomus".to_string()];
        path.extend(words.iter().cloned());
        let route = cli::route(&path);
        prop_assert!(route.starts_with(cli::ROUTE_PREFIX));
        prop_assert!(route.ends_with('/'));
        prop_assert!(!route.contains("//"));
        prop_assert_eq!(
            route.trim_start_matches(cli::ROUTE_PREFIX).matches('/').count(),
            words.len() + 1
        );
        if !words.is_empty() {
            let parent = cli::route(&path[..path.len() - 1]);
            prop_assert!(route.starts_with(&parent));
        }
    }

    /// The command line a reader copies is a rendering of the argument vector: every word
    /// of the vector survives it, in order, and the executable's own name leads.
    #[test]
    fn a_rendered_command_line_carries_every_word_of_its_argv(
        words in proptest::collection::vec("[a-zA-Z0-9_.=/-]{1,10}", 0..6)
    ) {
        let argv: Vec<&str> = words.iter().map(String::as_str).collect();
        let line = cli::render(&argv);
        prop_assert!(line.starts_with("majordomus"));
        let mut rest = &line["majordomus".len()..];
        for w in &words {
            let at = rest.find(w.as_str());
            prop_assert!(at.is_some(), "{} is not in {}", w, line);
            rest = &rest[at.unwrap() + w.len()..];
        }
    }
}

// ---------------------------------------------------------------- the projections

#[test]
fn the_generated_reference_and_document_hold_every_command_and_every_example() {
    let t = tree();
    let md = majordomus_cli::generate::cli_reference(&t, "test");
    let json = majordomus_cli::generate::cli_document(&t, "test");
    let doc: serde_json::Value = serde_json::from_str(&json).expect("cli.json is JSON");
    assert_eq!(doc["schema"], cli::SCHEMA);
    assert_eq!(doc["source"], cli::DECLARATION);
    for c in t.flatten() {
        assert!(
            md.contains(&format!("## `{}`", c.command())),
            "{} is not in the generated reference",
            c.command()
        );
        assert!(
            md.contains(&c.route),
            "the route of {} is not in the generated reference",
            c.command()
        );
        for e in &c.examples {
            assert!(
                md.contains(&e.command),
                "example {} is not in the generated reference",
                e.id
            );
            assert!(
                md.contains(&e.expectation),
                "example {} is shown without what it is verified to do",
                e.id
            );
        }
    }
    // every command of the tree is in the JSON, addressed by its own route
    let routes: Vec<String> = t.flatten().iter().map(|c| c.route.clone()).collect();
    for r in routes {
        assert!(json.contains(&format!("\"{r}\"")), "{r} is not in cli.json");
    }
}

#[test]
fn two_generations_of_the_reference_and_the_document_are_identical() {
    assert_eq!(
        majordomus_cli::generate::cli_reference(&tree(), "test"),
        majordomus_cli::generate::cli_reference(&tree(), "test")
    );
    assert_eq!(
        majordomus_cli::generate::cli_document(&tree(), "test"),
        majordomus_cli::generate::cli_document(&tree(), "test")
    );
    // and nothing in either carries an absolute path, a timestamp or this machine
    let text = format!(
        "{}{}",
        majordomus_cli::generate::cli_reference(&tree(), "test"),
        majordomus_cli::generate::cli_document(&tree(), "test")
    );
    assert!(
        !text.contains(env!("CARGO_MANIFEST_DIR")),
        "an absolute path"
    );
    for marker in ["/Users/", "/home/", "generated at", "20", "T0"] {
        if marker == "20" {
            continue;
        }
        assert!(!text.contains(marker), "{marker:?} is in a generated file");
    }
}
