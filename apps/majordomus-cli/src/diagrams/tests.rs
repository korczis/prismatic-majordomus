//! What the conversion is allowed to change, as tests.
//!
//! Every diagram here replaced a Mermaid *source string* that `scripts/generate-site-data`
//! composed by hand. A replacement is only credible against what it replaced, so
//! `before.json` is frozen: it is what the site published at `98a1f0800`, and nothing
//! regenerates it.
//!
//! Comparing the two as text would prove nothing — the renderer writes `a -.->|x| b` where
//! the jq wrote `a -. "x" .-> b`, and quotes every label the printf left bare. Both are the
//! same *drawing*. So the comparison is over the graph: [`Drawing::parse`] reads Mermaid
//! back into nodes, edges and groups, and the assertion is that the model renders to the
//! same graph the site already publishes — or, where it deliberately does not, exactly
//! which facts were added and why.

use std::collections::BTreeSet;

use super::*;
use crate::policy::{Projection, ProjectionMode};

/// What the site published before this module existed.
const BEFORE: &str = include_str!("before.json");

/// The Mermaid the site published for one diagram, at `98a1f0800`.
fn before(id: &str) -> String {
    let doc: serde_json::Value = serde_json::from_str(BEFORE).expect("the frozen fixture parses");
    doc["diagrams"][id]["mermaid"]
        .as_str()
        .unwrap_or_else(|| panic!("{id} is not in the frozen fixture"))
        .to_string()
}

/// A Mermaid document read back as the graph it draws: the comparison that survives two
/// renderers writing the same picture in different syntax.
#[derive(Debug, Default, PartialEq, Eq)]
struct Drawing {
    /// `flowchart LR`, `stateDiagram-v2`, ...
    header: String,
    /// `<id>|<label>|<shape>`, one per declared node.
    nodes: BTreeSet<String>,
    /// `<from>-><to>|<label>|<style>`, one per arrow.
    edges: BTreeSet<String>,
    /// `<group id>:<member>,<member>`, one per subgraph.
    groups: BTreeSet<String>,
}

impl Drawing {
    /// Read a Mermaid document. Deliberately narrow: it understands the dialect this
    /// repository's diagrams are written in and nothing else, so a construct it cannot read
    /// is a panic rather than a silently empty graph.
    fn parse(mermaid: &str) -> Drawing {
        let mut out = Drawing::default();
        let mut group: Option<(String, Vec<String>)> = None;
        for raw in mermaid.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with("%%") {
                continue;
            }
            if out.header.is_empty() {
                out.header = line.to_string();
                continue;
            }
            if let Some(rest) = line.strip_prefix("subgraph ") {
                let id = rest.split_whitespace().next().unwrap_or_default();
                group = Some((id.to_string(), Vec::new()));
                continue;
            }
            if line == "end" {
                let (id, members) = group.take().expect("`end` without a subgraph");
                out.groups.insert(format!("{id}:{}", members.join(",")));
                continue;
            }
            if out.header == "stateDiagram-v2" {
                out.edges.insert(state_edge(line));
                continue;
            }
            match flow_edges(line) {
                Some(edges) => out.edges.extend(edges),
                None => {
                    let (id, node) = flow_node(line);
                    if let Some((_, members)) = group.as_mut() {
                        members.push(id);
                    }
                    out.nodes.insert(node);
                }
            }
        }
        out
    }
}

/// One node declaration: `A[label]`, `A["label"]`, `D[(label)]`, `M(["label"])`.
fn flow_node(line: &str) -> (String, String) {
    let open = line
        .find(['[', '('])
        .unwrap_or_else(|| panic!("not a node declaration: {line:?}"));
    let (id, rest) = line.split_at(open);
    let (shape, inner) = match (&rest[..2.min(rest.len())], rest) {
        ("[(", r) => ("cylinder", &r[2..r.len() - 2]),
        ("([", r) => ("stadium", &r[2..r.len() - 2]),
        ("((", r) => ("terminal", &r[2..r.len() - 2]),
        (_, r) if r.starts_with('[') => ("box", &r[1..r.len() - 1]),
        (_, r) => ("rounded", &r[1..r.len() - 1]),
    };
    let label = inner.trim_matches('"');
    (id.to_string(), format!("{id}|{label}|{shape}"))
}

/// Every arrow on one line, or `None` when the line carries none. Chains (`a --> b --> c`)
/// are the edges they stand for, which is how a renderer that never writes a chain compares
/// equal to a printf that did.
fn flow_edges(line: &str) -> Option<Vec<String>> {
    if !line.contains("-->") && !line.contains(".->") {
        return None;
    }
    // `a -. "text" .-> b` and `a -.->|text| b` are one arrow written two ways
    let line = line.replace("-. \"", "-.->|").replace("\" .->", "|");
    let mut edges = Vec::new();
    let mut rest = line.as_str();
    let mut tail: Option<String> = None;
    loop {
        let dotted_at = rest.find("-.->");
        let solid_at = rest.find("-->");
        let (at, len, dotted) = match (dotted_at, solid_at) {
            (Some(d), Some(s)) if d <= s => (d, 4, true),
            (Some(d), None) => (d, 4, true),
            (_, Some(s)) => (s, 3, false),
            (None, None) => break,
        };
        let from = tail.unwrap_or_else(|| rest[..at].trim().to_string());
        let after = &rest[at + len..];
        let (label, after) = match after.trim_start().strip_prefix('|') {
            Some(l) => {
                let close = l.find('|').expect("an unclosed edge label");
                (l[..close].to_string(), &l[close + 1..])
            }
            None => (String::new(), after),
        };
        let to = after
            .trim_start()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
        edges.push(format!(
            "{from}->{to}|{label}|{}",
            if dotted { "dotted" } else { "solid" }
        ));
        tail = Some(to);
        rest = after;
    }
    Some(edges)
}

/// One transition of a state diagram: `active --> failed: finish --outcome failed`.
fn state_edge(line: &str) -> String {
    let (from, rest) = line.split_once(" --> ").expect("a state transition");
    let (to, label) = match rest.split_once(": ") {
        Some((to, label)) => (to, label),
        None => (rest, ""),
    };
    format!("{}->{}|{label}|solid", from.trim(), to.trim())
}

/// The fixtures the derivations are exercised with: this repository's own shape, stated
/// once so that a test reads as an assertion and not as a second derivation.
fn projections() -> Vec<Projection> {
    [
        ("agents", "AGENTS.md"),
        ("claude-code", "CLAUDE.md"),
        ("gemini", "GEMINI.md"),
    ]
    .into_iter()
    .map(|(provider, target)| Projection {
        provider: provider.into(),
        target: target.into(),
        always_loaded: true,
        mode: ProjectionMode::File,
    })
    .collect()
}

fn layers() -> Vec<Layer> {
    [
        ("Doctrine", 10usize, "principles", "years"),
        ("Policy", 3, "projections", "months to years"),
        ("Rule", 44, "rules", "months"),
        (
            "Enforcement",
            44,
            "validators",
            "the implementation that answers it",
        ),
        ("Diagnostic", 4, "reporting commands", "one run"),
    ]
    .into_iter()
    .map(|(level, count, unit, lifetime)| Layer {
        level: level.into(),
        count,
        unit: unit.into(),
        lifetime: lifetime.into(),
    })
    .collect()
}

fn interfaces() -> Vec<Interface> {
    [
        ("cli", "Command line", 48usize),
        ("http", "HTTP + OpenAPI", 107),
        ("mcp", "MCP tools and resources", 105),
    ]
    .into_iter()
    .map(|(id, label, count)| Interface {
        id: id.into(),
        label: label.into(),
        count,
    })
    .collect()
}

#[test]
fn the_parser_can_tell_two_different_drawings_apart() {
    // the negative control of the comparison itself. Every assertion below is an equality
    // between two `Drawing`s, and an equality is worth nothing from a reader that collapses
    // everything it is given to the same value.
    let one = Drawing::parse("flowchart TD\n    a[\"A\"]\n    b[\"B\"]\n    a --> b");
    assert_eq!(one, Drawing::parse("flowchart TD\n    a[A]\n    b[B]\n    a --> b"));
    assert_ne!(one, Drawing::parse("flowchart TD\n    a[\"A\"]\n    b[\"B\"]\n    b --> a"));
    assert_ne!(one, Drawing::parse("flowchart TD\n    a[\"A\"]\n    b[\"different\"]\n    a --> b"));
    assert_ne!(one, Drawing::parse("flowchart TD\n    a[\"A\"]\n    b[\"B\"]\n    a -.-> b"));
    assert_ne!(one, Drawing::parse("flowchart LR\n    a[\"A\"]\n    b[\"B\"]\n    a --> b"));

    // and a chain is the edges it stands for, which is the whole reason the comparison is
    // over the graph rather than over the text
    assert_eq!(
        Drawing::parse("flowchart LR\n    a[A]\n    b[B]\n    c[C]\n    a --> b --> c"),
        Drawing::parse("flowchart LR\n    a[A]\n    b[B]\n    c[C]\n    a --> b\n    b --> c"),
    );
}

#[test]
fn the_pipeline_draws_what_the_site_already_published() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the repository root is two levels above the crate");
    let drawn = pipeline(root).expect("every stage of the site build is in this tree");
    assert!(drawn.validate().is_empty(), "{:?}", drawn.validate());
    assert_eq!(
        Drawing::parse(&drawn.to_mermaid()),
        Drawing::parse(&before("pipeline")),
        "the one diagram no declaration owns must at least still be the same drawing",
    );
}

#[test]
fn the_projection_adds_the_stamps_the_published_diagram_left_out() {
    let drawn = projection(".ai/repo/policy.yaml", &projections());
    assert!(drawn.validate().is_empty(), "{:?}", drawn.validate());
    let now = Drawing::parse(&drawn.to_mermaid());
    let then = Drawing::parse(&before("projection"));

    assert_eq!(now.nodes, then.nodes, "the same boxes, from the same policy");
    assert_eq!(now.header, then.header);
    // the difference, stated rather than tolerated: `doctor` reads the stamp back out of
    // every projected file, and the published diagram drew one arrow, from the first
    let added: Vec<&String> = now.edges.difference(&then.edges).collect();
    assert_eq!(
        added,
        ["T1->D|stamp|dotted", "T2->D|stamp|dotted"]
            .iter()
            .collect::<Vec<_>>(),
        "only the missing stamp arrows are new",
    );
    assert!(
        then.edges.difference(&now.edges).next().is_none(),
        "and nothing the site drew was dropped",
    );
}

#[test]
fn supervision_draws_the_providers_the_distribution_declares() {
    let providers: Vec<ProviderDeclaration> = [
        ("agents", "Any tool that reads AGENTS.md"),
        ("claude-code", "Claude Code"),
        ("codex", "Codex"),
    ]
    .into_iter()
    .map(|(id, title)| ProviderDeclaration {
        id: id.into(),
        title: title.into(),
        template: true,
        ..Default::default()
    })
    .collect();
    let drawn = supervision(&providers);
    assert!(drawn.validate().is_empty(), "{:?}", drawn.validate());

    let now = Drawing::parse(&drawn.to_mermaid());
    let then = Drawing::parse(&before("supervision"));
    assert_eq!(now.header, then.header);
    // the frame is the same: the person, the tool, the outcome, and an arrow each way
    for kept in ["H|Human / Organisation|box", "V|Verified, accepted outcomes|box"] {
        assert!(now.nodes.contains(kept), "{kept} was dropped");
        assert!(then.nodes.contains(kept));
    }
    assert!(now.edges.contains("H->M||solid"));
    // what changed: the worker boxes are the declared providers, not three names typed once
    assert!(!now.nodes.iter().any(|n| n.contains("Cursor")), "the typed-once names are gone");
    assert!(
        now.nodes.contains("W0|Any tool that reads AGENTS.md|box"),
        "{:?}",
        now.nodes
    );
    assert_eq!(
        now.nodes.iter().filter(|n| n.starts_with('W')).count(),
        providers.len(),
        "one box per provider the distribution ships an adapter for",
    );
    // a provider that is only a name in the declaration is not a worker to supervise
    let named_only = ProviderDeclaration {
        id: "ghost".into(),
        title: "Ghost".into(),
        template: false,
        ..Default::default()
    };
    let with_ghost = supervision(&[named_only]);
    assert!(!with_ghost.to_mermaid().contains("Ghost"));
}

#[test]
fn governance_draws_the_layers_the_site_already_published() {
    let drawn = governance(&layers(), &[10]);
    assert!(drawn.validate().is_empty(), "{:?}", drawn.validate());
    assert_eq!(
        Drawing::parse(&drawn.to_mermaid()),
        Drawing::parse(&before("governance")),
        "same layers, same counts, same arrow back to the rule",
    );
}

#[test]
fn the_axes_draw_what_the_site_already_published() {
    let drawn = axes(4, 4, 108, &interfaces());
    assert!(drawn.validate().is_empty(), "{:?}", drawn.validate());
    assert_eq!(
        Drawing::parse(&drawn.to_mermaid()),
        Drawing::parse(&before("axes")),
        "same axes, same interfaces, same counts",
    );
}

#[test]
fn the_lifecycle_the_model_draws_is_the_one_the_site_published() {
    // the model's own test proves this byte for byte against `lib/finish.sh`; this one
    // proves the same diagram travels through the graph comparison, so that a change to the
    // outcome vocabulary is visible here too
    let shell = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("the repository root")
            .join(FINISH),
    )
    .expect("lib/finish.sh is in this repository");
    let drawn = crate::diagram::lifecycle(&shell).expect("the vocabulary is readable");
    assert_eq!(
        Drawing::parse(&drawn.to_mermaid()),
        Drawing::parse(&before("lifecycle")),
    );
}

#[test]
fn a_layer_that_counts_nothing_is_still_drawn_when_it_is_asked_for() {
    // `published` refuses an empty layer; `governance` itself does not, because a caller
    // that has a reason to draw a smaller package must get the picture it asked for
    let small = vec![Layer {
        level: "Rule".into(),
        count: 0,
        unit: "rules".into(),
        lifetime: "months".into(),
    }];
    let drawn = governance(&small, &[]);
    assert!(drawn.validate().is_empty());
    assert!(drawn.edges.is_empty(), "one layer descends into nothing");
}

#[test]
fn governance_counts_the_package_and_never_the_repository_rules_beside_it() {
    let rule = |namespace: &str, tags: &[&str], validator: Option<&str>, class: Class| {
        RuleDefinition {
            id: format!("{namespace}.r{}", tags.len()),
            identity: String::new(),
            uri: String::new(),
            version: 1,
            title: String::new(),
            description: None,
            statement: None,
            status: "active".into(),
            class,
            namespace: namespace.into(),
            depends_on: vec![],
            tags: tags.iter().map(|t| t.to_string()).collect(),
            path: String::new(),
            enforcement: crate::rules::Enforcement {
                mode: crate::rules::Mode::Declarative,
                validator: validator.map(str::to_string),
                category: None,
                exit_code: validator.map(|_| 10),
                enforced_by: vec!["doctor".into(), "watch".into()],
                tests: vec![],
                reviewed_because: None,
            },
        }
    };
    let rules = vec![
        rule("majordomus", &["principle"], None, Class::Advisory),
        rule("majordomus", &[], Some("adr"), Class::Blocking),
        rule("majordomus", &["a", "b"], Some("adr"), Class::Blocking),
        rule("project", &["principle"], Some("other"), Class::Blocking),
    ];
    let layers = governance_layers(&rules, 3);
    assert_eq!(layers[0].count, 1, "one principle, and not the project's");
    assert_eq!(layers[1].count, 3, "the policy's own number");
    assert_eq!(layers[2].count, 2, "a doctrine is a rule that names a validator");
    assert_eq!(layers[3].count, 1, "two doctrines, one validator between them");
    assert_eq!(layers[4].count, 2, "doctor and watch, read as the list they are written as");
    assert_eq!(blocking_exit_codes(&rules), vec![10]);
}
