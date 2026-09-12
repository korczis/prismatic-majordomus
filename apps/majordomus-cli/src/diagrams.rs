//! The diagrams this repository publishes, each derived from the declaration that already
//! owns the relationship it draws.
//!
//! [`crate::diagram`] is the *model*: nodes, edges, groups, and Mermaid as one renderer of
//! it. This module is what the model is built out of — the policy's projections, the
//! provider declarations, the rule package, the profiles, the seeded workflows, the
//! capability registry, the tree itself — and the artifact the site reads.
//!
//! # Why the executable draws them and the site generator does not
//!
//! Until this module the six diagrams on the site were Mermaid *source strings* composed by
//! `printf` and `jq` inside `scripts/generate-site-data`:
//!
//! ```text
//!   policy.yaml ── jq ──> "    U --> T0[\"AGENTS.md…\"]\n" ──> diagrams.json
//!                              │
//!                    the only statement of the structure
//! ```
//!
//! The counts in two of them were joins over real data, so they moved when the repository
//! moved — but the *topology* was a string, and a string is not readable by a second
//! renderer, a validator, or anything that wants to ask what the picture claims. That is
//! what `project.a-diagram-is-drawn-not-typed` forbids.
//!
//! The site generator cannot call this executable: the job that builds the published site
//! installs Node and Zola and no Rust toolchain (`.github/actions/setup-site`), and every
//! fact the generator takes from the executable it takes from a *committed* artifact —
//! `docs/generated/registry.json`, `docs/generated/web.json`, `site/data/registry/why.json`
//! — refusing with exit 10 when one is missing. So the diagrams travel the same way: this
//! module writes `site/data/registry/diagrams.json`, `majordomus generate --check` says
//! when it has gone stale, and the generator projects it into the shape the templates read.
//!
//! # What is derived and what could not be
//!
//! Five of the six draw a relationship some declaration already states, and are read out of
//! it. [`pipeline`] is the exception and says so in its own documentation: nothing in the
//! repository declares the stages of the site build, so its shape is written here — and
//! every node of it names a path the derivation refuses to draw without.

use std::collections::BTreeSet;
use std::path::Path;

use crate::app::App;
use crate::capability::Provenance as CapabilityProvenance;
use crate::diagram::{Diagram, Edge, Group, Kind, Node, Orientation, Provenance, Shape};
use crate::error::{Error, Result};
use crate::generate::{Artifact, ArtifactFormat, SITE_DATA_DIR};
use crate::policy::{LoadedPolicy, Projection};
use crate::rules::{Class, RuleDefinition};
use crate::share::ProviderDeclaration;

/// The schema of `site/data/registry/diagrams.json`.
pub const SCHEMA: &str = "majordomus-site-diagrams/v1";

/// What the artifact says it was derived from.
pub const SOURCE: &str = "the declarations that own each topology: the outcome vocabulary, \
                          the policy's projections, the provider declarations, the rule \
                          package, the profiles, the seeded workflows and the capability \
                          registry";

/// The shell source whose `case` arm is the outcome vocabulary.
pub const FINISH: &str = "lib/finish.sh";

/// The namespace of the rule package whose rules are the doctrine layer.
const STANDARD: &str = "majordomus";

/// The tag a rule of that package carries when it is a principle rather than a rule.
const PRINCIPLE: &str = "principle";

/// The seeded workflows, relative to the distribution directory.
const WORKFLOWS: &str = "skeleton/ai/repo/workflows";

/// One layer an invariant descends through, as the governance diagram draws it.
///
/// ```
/// use majordomus_cli::diagrams::Layer;
/// let layer = Layer { level: "Rule".into(), count: 44, unit: "rules".into(), lifetime: "months".into() };
/// assert_eq!(layer.detail(), "44 rules · months");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    /// The layer's name: `Doctrine`, `Policy`, `Rule`, `Enforcement`, `Diagnostic`.
    pub level: String,
    /// What this repository holds at that layer today.
    pub count: usize,
    /// What is being counted.
    pub unit: String,
    /// How long an answer at this layer lives.
    pub lifetime: String,
}

impl Layer {
    /// The qualifier under the level's name.
    pub fn detail(&self) -> String {
        format!("{} {} · {}", self.count, self.unit, self.lifetime)
    }
}

/// One interface the same capability can be reached on, as the axes diagram draws it.
///
/// ```
/// use majordomus_cli::diagrams::Interface;
/// let cli = Interface { id: "cli".into(), label: "Command line".into(), count: 48 };
/// assert_eq!(cli.detail(108), "48 of 108");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// The exposure key: `cli`, `http`, `mcp`.
    pub id: String,
    /// The name a reader knows it by.
    pub label: String,
    /// How many capabilities declare it.
    pub count: usize,
}

impl Interface {
    /// The qualifier under the interface's name.
    pub fn detail(&self, capabilities: usize) -> String {
        format!("{} of {capabilities}", self.count)
    }
}

/// How this site is derived: the canonical inputs, the generator, the derived data, and the
/// three programs between that and the published page.
///
/// **This is the one diagram no declaration owns.** There is no file that states the stages
/// of the site build; `scripts/generate-site-data` is a program, and a program's stages are
/// not data. So the shape below is written here — which is the defect this module exists to
/// remove, admitted rather than hidden.
///
/// What keeps it from being a free-hand drawing is that every node names a path, and the
/// derivation refuses when one of them is not in the tree. A picture that survives
/// `docs/CLAIMS.yaml` being renamed would be a claim about a repository that no longer
/// exists; this one stops the build instead.
///
/// ```
/// use majordomus_cli::diagrams::pipeline;
/// assert!(pipeline(std::path::Path::new("/nonexistent")).is_none(), "no tree, no drawing");
/// ```
pub fn pipeline(root: &Path) -> Option<Diagram> {
    // (node id, label, the path that must exist, the shape)
    let stages: [(&str, &str, &str, Shape); 8] = [
        ("A", "share/skeleton", "share/skeleton", Shape::Box),
        ("B", "docs + README", "README.md", Shape::Box),
        ("C", "docs/CLAIMS.yaml", "docs/CLAIMS.yaml", Shape::Box),
        (
            "G",
            "scripts/generate-site-data",
            "scripts/generate-site-data",
            Shape::Box,
        ),
        (
            "D",
            "site/data/generated",
            "site/data/generated",
            Shape::Cylinder,
        ),
        ("Z", "Zola", "site/config.toml", Shape::Box),
        (
            "F",
            "Tailwind + Flowbite + Alpine",
            "package.json",
            Shape::Box,
        ),
        (
            "P",
            "GitHub Pages",
            ".github/workflows/pages.yml",
            Shape::Box,
        ),
    ];
    let mut drawn = Diagram::new(
        "pipeline",
        "How this site is derived",
        Kind::Flow,
        Provenance::new(
            "scripts/generate-site-data",
            "the inputs it reads and the programs between it and the published page",
        ),
    )
    .oriented(Orientation::LeftRight);
    for (id, label, path, shape) in stages {
        if !root.join(path).exists() {
            return None;
        }
        drawn = drawn.with_node(Node::new(id, label).shaped(shape));
    }
    Some(
        drawn
            .with_group(Group::new("canonical", "canonical", ["A", "B", "C"]))
            .with_edge(Edge::new("A", "G"))
            .with_edge(Edge::new("B", "G"))
            .with_edge(Edge::new("C", "G"))
            .with_edge(Edge::new("G", "D"))
            .with_edge(Edge::new("D", "Z"))
            .with_edge(Edge::new("Z", "F"))
            .with_edge(Edge::new("F", "P")),
    )
}

/// The policy projected into the instruction file of every worker: one target per
/// `projections[]` entry, and the stamp `majordomus doctor` reads back out of each.
///
/// Nothing about the set is written here. A repository that projects two bootstraps draws
/// two, and one that adds a provider tomorrow draws it tomorrow.
///
/// The stamp arrow reaches *every* target, which the published diagram did not: it drew one
/// arrow, from the first. The stamp is a line in each generated file and `doctor` compares
/// all of them, so an arrow from one was a picture of a check that does not exist.
///
/// ```
/// use majordomus_cli::diagrams::projection;
/// use majordomus_cli::policy::{Projection, ProjectionMode};
///
/// let projections = vec![Projection {
///     provider: "claude-code".into(), target: "CLAUDE.md".into(),
///     always_loaded: true, mode: ProjectionMode::File,
/// }];
/// let drawn = projection(".ai/repo/policy.yaml", &projections);
/// assert!(drawn.validate().is_empty());
/// let mermaid = drawn.to_mermaid();
/// assert!(mermaid.contains(r#"T0["CLAUDE.md<br/><small>claude-code</small>"]"#));
/// assert!(mermaid.contains("T0 -.->|stamp| D"));
/// ```
pub fn projection(policy_path: &str, projections: &[Projection]) -> Diagram {
    let mut drawn = Diagram::new(
        "projection",
        "Policy to instruction files",
        Kind::Flow,
        Provenance::new(policy_path, "the projections it declares"),
    )
    .with_node(Node::new("P", policy_path))
    .with_node(Node::new("U", "majordomus update"))
    .with_edge(Edge::new("P", "U"));
    for (i, p) in projections.iter().enumerate() {
        drawn = drawn.with_node(Node::new(format!("T{i}"), &p.target).detailed(&p.provider));
    }
    drawn = drawn.with_node(Node::new("D", "majordomus doctor"));
    for (i, _) in projections.iter().enumerate() {
        drawn = drawn.with_edge(Edge::new("U", format!("T{i}")));
    }
    for (i, _) in projections.iter().enumerate() {
        drawn = drawn.with_edge(Edge::new(format!("T{i}"), "D").labelled("stamp").dotted());
    }
    drawn
}

/// The core model: a person supervises Majordomus, Majordomus supervises the workers, and
/// what comes back is verified.
///
/// The workers are the providers the distribution ships an adapter for — `share/providers`
/// joined with `share/providers.yaml` — which is the set `majordomus product providers`,
/// the provider table and the site's provider cards all answer from (ADR 0024). The
/// published diagram drew three boxes, one of which said `Gemini · Cursor · local`; a
/// provider added to the distribution did not appear in it, and one of the names on it was
/// never a provider of this tool at all.
///
/// ```
/// use majordomus_cli::diagrams::supervision;
/// use majordomus_cli::share::ProviderDeclaration;
///
/// let codex = ProviderDeclaration {
///     id: "codex".into(), title: "Codex".into(), template: true, ..Default::default()
/// };
/// let drawn = supervision(&[codex]);
/// assert!(drawn.validate().is_empty());
/// assert!(drawn.to_mermaid().contains(r#"W0["Codex"]"#));
/// ```
pub fn supervision(providers: &[ProviderDeclaration]) -> Diagram {
    let mut drawn = Diagram::new(
        "supervision",
        "The core model",
        Kind::Flow,
        Provenance::new(
            "share/providers.yaml",
            "the providers the distribution ships an adapter for",
        ),
    )
    .with_node(Node::new("H", "Human / Organisation"))
    .with_node(Node::new("M", "Majordomus").detailed("policy · state · verification"));
    let shipped: Vec<&ProviderDeclaration> = providers.iter().filter(|p| p.template).collect();
    for (i, p) in shipped.iter().enumerate() {
        drawn = drawn.with_node(Node::new(format!("W{i}"), &p.title));
    }
    drawn = drawn
        .with_node(Node::new("V", "Verified, accepted outcomes"))
        .with_edge(Edge::new("H", "M"));
    for (i, _) in shipped.iter().enumerate() {
        drawn = drawn.with_edge(Edge::new("M", format!("W{i}")));
    }
    for (i, _) in shipped.iter().enumerate() {
        drawn = drawn.with_edge(Edge::new(format!("W{i}"), "V"));
    }
    drawn
}

/// The layers an invariant descends through, counted over the rule package this repository
/// vendors and the policy beside it.
///
/// `projections` is the policy's count, which is the one number here the rules do not hold.
/// Everything else is read off the rules: a principle is a rule of the standard package
/// tagged `principle`, a doctrine is one that names a validator, and the reporting commands
/// are the `enforced_by` of those doctrines — which is a list in every rule file that
/// declares it, and was invisible to this crate until it was read as one.
///
/// ```
/// use majordomus_cli::diagrams::governance_layers;
/// assert_eq!(governance_layers(&[], 3).len(), 5, "five layers, whatever is in the tree");
/// assert_eq!(governance_layers(&[], 3)[1].count, 3, "the policy's own number");
/// ```
pub fn governance_layers(rules: &[RuleDefinition], projections: usize) -> Vec<Layer> {
    let package: Vec<&RuleDefinition> = rules.iter().filter(|r| r.namespace == STANDARD).collect();
    let principles = package
        .iter()
        .filter(|r| r.tags.iter().any(|t| t == PRINCIPLE))
        .count();
    let doctrines: Vec<&&RuleDefinition> = package
        .iter()
        .filter(|r| r.enforcement.validator.is_some())
        .collect();
    let validators: BTreeSet<&str> = doctrines
        .iter()
        .filter_map(|r| r.enforcement.validator.as_deref())
        .collect();
    let commands: BTreeSet<&str> = doctrines
        .iter()
        .flat_map(|r| r.enforcement.enforced_by.iter().map(String::as_str))
        .collect();
    let layer = |level: &str, count: usize, unit: &str, lifetime: &str| Layer {
        level: level.into(),
        count,
        unit: unit.into(),
        lifetime: lifetime.into(),
    };
    vec![
        layer("Doctrine", principles, "principles", "years"),
        layer("Policy", projections, "projections", "months to years"),
        layer("Rule", doctrines.len(), "rules", "months"),
        layer(
            "Enforcement",
            validators.len(),
            "validators",
            "the implementation that answers it",
        ),
        layer("Diagnostic", commands.len(), "reporting commands", "one run"),
    ]
}

/// The exit codes a blocking doctrine of the package refuses with, in order.
pub fn blocking_exit_codes(rules: &[RuleDefinition]) -> Vec<i64> {
    let codes: BTreeSet<i64> = rules
        .iter()
        .filter(|r| {
            r.namespace == STANDARD
                && r.class == Class::Blocking
                && r.enforcement.validator.is_some()
        })
        .filter_map(|r| r.enforcement.exit_code)
        .collect();
    codes.into_iter().collect()
}

/// One invariant, all the way down: the five layers, and the arrow back from the diagnostic
/// to the rule it names.
///
/// ```
/// use majordomus_cli::diagrams::{governance, Layer};
/// let layers = vec![
///     Layer { level: "Doctrine".into(), count: 10, unit: "principles".into(), lifetime: "years".into() },
///     Layer { level: "Rule".into(), count: 44, unit: "rules".into(), lifetime: "months".into() },
/// ];
/// let drawn = governance(&layers, &[10]);
/// assert!(drawn.validate().is_empty());
/// let mermaid = drawn.to_mermaid();
/// assert!(mermaid.contains(r#"L0["Doctrine<br/><small>10 principles · years</small>"]"#));
/// assert!(mermaid.contains("L1 -.->|exit 10 · the commit does not land| L0"));
/// ```
pub fn governance(layers: &[Layer], exit_codes: &[i64]) -> Diagram {
    let mut drawn = Diagram::new(
        "governance",
        "One invariant, all the way down",
        Kind::Flow,
        Provenance::new(
            "share/standard/majordomus/",
            "the rule package and the policy beside it",
        ),
    );
    for (i, layer) in layers.iter().enumerate() {
        drawn = drawn.with_node(Node::new(format!("L{i}"), &layer.level).detailed(layer.detail()));
    }
    for i in 1..layers.len() {
        drawn = drawn.with_edge(Edge::new(format!("L{}", i - 1), format!("L{i}")));
    }
    // the last layer is the one that reports, and what it reports is a rule — the third
    // layer where there are five, and the last-but-one wherever the package is smaller
    if layers.len() >= 2 {
        let codes: Vec<String> = exit_codes.iter().map(i64::to_string).collect();
        let rule_layer = if layers.len() >= 5 { 2 } else { layers.len() - 2 };
        drawn = drawn.with_edge(
            Edge::new(format!("L{}", layers.len() - 1), format!("L{rule_layer}"))
                .labelled(format!(
                    "exit {} · the commit does not land",
                    codes.join(", ")
                ))
                .dotted(),
        );
    }
    drawn
}

/// Four axes, deliberately not one: what a piece of work is described on, where the
/// capability it reaches is exposed, and the model that is on none of them.
///
/// ```
/// use majordomus_cli::diagrams::{axes, Interface};
/// let cli = Interface { id: "cli".into(), label: "Command line".into(), count: 48 };
/// let drawn = axes(4, 4, 108, &[cli]);
/// assert!(drawn.validate().is_empty());
/// let mermaid = drawn.to_mermaid();
/// assert!(mermaid.contains(r#"A0["Profile<br/><small>4 profiles</small>"]"#));
/// assert!(mermaid.contains(r#"S0["Command line<br/><small>48 of 108</small>"]"#));
/// assert!(mermaid.contains("A1 -.->|executed by| M"));
/// ```
pub fn axes(
    profiles: usize,
    workflows: usize,
    capabilities: usize,
    interfaces: &[Interface],
) -> Diagram {
    let mut drawn = Diagram::new(
        "axes",
        "Four axes, deliberately not one",
        Kind::Flow,
        Provenance::new(
            "the profiles, the seeded workflows and the capability registry",
            "their counts and the exposure each capability declares",
        ),
    )
    .with_node(Node::new("I", "Intent").detailed("one task, started once"))
    .with_node(Node::new("A0", "Profile").detailed(format!("{profiles} profiles")))
    .with_node(Node::new("A1", "Workflow").detailed(format!("{workflows} workflows")))
    .with_node(Node::new("C", "Capability").detailed(format!("{capabilities} · declared once")));
    for (i, interface) in interfaces.iter().enumerate() {
        drawn = drawn.with_node(
            Node::new(format!("S{i}"), &interface.label).detailed(interface.detail(capabilities)),
        );
    }
    drawn = drawn
        .with_node(
            Node::new("M", "Model")
                .detailed("chosen per run · named in no file of the layer")
                .shaped(Shape::Stadium),
        )
        .with_edge(Edge::new("I", "A0"))
        .with_edge(Edge::new("A0", "A1"))
        .with_edge(Edge::new("A1", "C"));
    for (i, _) in interfaces.iter().enumerate() {
        drawn = drawn.with_edge(Edge::new("C", format!("S{i}")));
    }
    drawn.with_edge(Edge::new("A1", "M").labelled("executed by").dotted())
}

/// Every diagram this repository publishes, in the order the artifact carries them.
///
/// A derivation that cannot read its source refuses here rather than drawing a picture out
/// of what it managed to find: an empty lifecycle, a pipeline with a stage missing and a
/// governance layer counted as zero are all worse than a build that stops.
pub fn published(app: &App, policy: &LoadedPolicy) -> Result<Vec<Diagram>> {
    let root = app.repository.root();
    let refuse = |reason: String| Error::Protocol { reason };

    let finish = std::fs::read_to_string(root.join(FINISH)).map_err(|e| Error::io(FINISH, e))?;
    let lifecycle = crate::diagram::lifecycle(&finish).ok_or_else(|| {
        refuse(format!(
            "{FINISH} carries no outcome vocabulary this can read; the lifecycle is not drawn"
        ))
    })?;
    let pipeline = pipeline(root).ok_or_else(|| {
        refuse("a stage of the site build names a path that is not in this tree".into())
    })?;

    let declarations = app.share.providers()?;
    let rules = crate::rules::definitions(&app.context.index);
    let layers = governance_layers(&rules, policy.policy.projections.len());
    if let Some(empty) = layers.iter().find(|l| l.count == 0) {
        return Err(refuse(format!(
            "the {} layer counts nothing; the governance diagram would draw a layer that is not there",
            empty.level
        )));
    }

    let workflows = seeded_workflows(app)?;
    if workflows == 0 {
        return Err(refuse(format!(
            "{} holds no workflow; the axes diagram counts them",
            WORKFLOWS
        )));
    }
    let profiles = crate::policy::profile_files(&app.repository)?.len();
    if profiles == 0 {
        return Err(refuse(
            "this repository declares no profile; the axes diagram counts them".into(),
        ));
    }

    let builtin: Vec<_> = app
        .context
        .registry
        .iter()
        .filter(|c| matches!(c.provenance, CapabilityProvenance::Builtin { .. }))
        .collect();
    // the three interfaces, in the order the registry's exposure keys are written, with the
    // name each is known by where a reader meets it
    let interfaces: Vec<Interface> = [
        ("cli", "Command line"),
        ("http", "HTTP + OpenAPI"),
        ("mcp", "MCP tools and resources"),
    ]
    .into_iter()
    .map(|(id, label)| Interface {
        id: id.into(),
        label: label.into(),
        count: builtin
            .iter()
            .filter(|c| match id {
                "cli" => c.exposure.cli.is_some(),
                "http" => c.exposure.http.is_some(),
                _ => c.exposure.mcp.is_some(),
            })
            .count(),
    })
    .collect();

    Ok(vec![
        lifecycle,
        pipeline,
        projection(&policy.path, &policy.policy.projections),
        supervision(&declarations.providers),
        governance(&layers, &blocking_exit_codes(&rules)),
        axes(profiles, workflows, builtin.len(), &interfaces),
    ])
}

/// How many workflows a fresh install is seeded with: every document under the skeleton's
/// workflow directory but its own README. The same set the site's method page counts.
fn seeded_workflows(app: &App) -> Result<usize> {
    let dir = app.share.dir().join(WORKFLOWS);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(Error::io(&dir, e)),
    };
    Ok(entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .filter(|p| p.file_name().is_some_and(|n| n != "README.md"))
        .count())
}

/// The artifact the site reads: every diagram as the model, with the Mermaid one renderer
/// of it produced beside it so that the generator does not need a renderer of its own.
///
/// A model this executable refuses to render is not published: `validate` runs on each and
/// the first finding stops `generate`, which is the point of having a model at all.
pub fn artifacts(app: &App, policy: &LoadedPolicy) -> Result<Vec<Artifact>> {
    let drawings = published(app, policy)?;
    let mut records = Vec::new();
    for drawn in &drawings {
        let findings = drawn.validate();
        if let Some(first) = findings.first() {
            return Err(Error::Protocol {
                reason: format!("the {} diagram does not render: {first}", drawn.id),
            });
        }
        let mut record = serde_json::to_value(drawn).unwrap_or_default();
        if let Some(o) = record.as_object_mut() {
            o.insert(
                "mermaid".into(),
                serde_json::Value::String(drawn.to_mermaid()),
            );
        }
        records.push(record);
    }
    let document = serde_json::json!({
        "schema": SCHEMA,
        "generated": crate::generate::json_banner(SOURCE),
        "generator": { "id": "majordomus-cli", "version": crate::VERSION },
        "diagrams": records,
    });
    Ok(vec![Artifact::verbatim(
        format!("{SITE_DATA_DIR}/diagrams.json"),
        "site-diagrams",
        ArtifactFormat::Json,
        Some(SCHEMA.to_string()),
        SOURCE,
        crate::site::render_json(&document),
    )])
}

#[cfg(test)]
mod tests;
