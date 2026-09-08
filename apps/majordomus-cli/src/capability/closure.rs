//! Projection closure: a declared exposure is answered by the surface that would carry it.
//!
//! `project.interfaces-are-projections@1` says a capability is defined once and every
//! external interface is derived from that definition. Three of the four interfaces prove
//! it by construction — the MCP tool list, the HTTP route table and the OpenAPI document
//! are each built by walking the registry, so an entry cannot exist without a descriptor
//! and a descriptor cannot fail to produce one. The command line is the exception. It is
//! declared a second time, in clap, in `cli.rs`; [`CliExposure`](super::CliExposure) is a
//! *claim* about that declaration, and until this module nothing compared the two.
//!
//! The gap was not theoretical. `tests/projections.rs` checked the CLI half with
//! `registry.by_cli(&cli.path).unwrap().id == c.id`, which asks the registry whether it
//! agrees with itself and is satisfied by any path at all, including one clap has never
//! heard of. A capability could declare `cli: ["worktree", "topology"]` with no such
//! subcommand and nothing — not the build, not the tests, not `generate --check` — would
//! say so; the failure surfaced at run time, to whoever typed the command, as "no
//! capability is exposed as `majordomus …`".
//!
//! So this module compares the two declarations, in both directions:
//!
//! * **declared, not present** — a capability claims a CLI path the clap tree does not
//!   have, or has only as a group that cannot be run. That is a broken promise and a
//!   failure ([`Finding`]).
//! * **present, not declared** — a runnable clap command that no capability claims. That
//!   is not automatically wrong: `serve`, `mcp` and `generate` are infrastructure and have
//!   no capability behind them by design. It is, however, the measure of how much of the
//!   command line is still hand-written rather than derived, so it is reported as an
//!   inventory ([`unbacked`]) that a gate ratchets: what is undeclared today may not grow.
//!
//! Everything here is a pure function of the registry and the clap tree. Nothing reads the
//! repository, the environment or the network, so the whole check costs a walk of two
//! in-memory structures and runs inside a unit test.

use std::collections::{BTreeMap, BTreeSet};

use super::model::Capability;
use super::registry::CapabilityRegistry;
use crate::cli::{CommandDoc, DECLARATION};

/// Which interface a finding is about. Only the command line can be closed by comparison
/// today, because it is the only interface with a declaration of its own to compare
/// against; the variant exists so that a surface which grows one later says which it is
/// rather than being told apart by the text of its message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Projection {
    /// The native command line, declared in clap in `cli.rs`.
    Cli,
}

impl Projection {
    /// The name a report prints.
    ///
    /// ```
    /// use majordomus_cli::capability::closure::Projection;
    /// assert_eq!(Projection::Cli.name(), "cli");
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            Projection::Cli => "cli",
        }
    }

    /// The file the projection is declared in; where a reader goes to fix it.
    pub fn declaration(self) -> &'static str {
        match self {
            Projection::Cli => DECLARATION,
        }
    }
}

/// One broken promise: a capability declared an exposure and the surface does not carry it.
///
/// The shape follows `project.finding-carries-reproduce@1` — a finding names the subject,
/// what is wrong, where the claim was made, where the surface is declared, and the one
/// command that shows it again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The stable code a reader greps for.
    pub code: &'static str,
    /// The capability whose claim is unmet.
    pub capability: String,
    /// The interface the claim was about.
    pub projection: Projection,
    /// The exposure as declared, `majordomus why show`.
    pub claim: String,
    /// What is wrong, in one line.
    pub detail: String,
    /// Where the capability is declared; repository-relative, never a machine path.
    pub source: String,
    /// The rule the claim belongs to.
    pub rule: &'static str,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:\n  capability: {}\n  claims: {}\n  source: {}\n  projection: {}\n  detail: {}\n  rule: {}\n  reproduce: majordomus capabilities projections --unmet",
            self.code,
            self.capability,
            self.claim,
            self.source,
            self.projection.declaration(),
            self.detail,
            self.rule,
        )
    }
}

/// The rule every finding here belongs to; named once so the text cannot drift from it.
const RULE: &str = "a capability is defined once and every external interface is derived from that definition";

/// A runnable command of the command line that no capability claims.
///
/// Not a violation on its own: the process commands (`serve`, `mcp`), the generators
/// (`generate`) and the lifecycle verbs that call a service directly are all legitimately
/// hand-written today. It is the debt the rule is measured against, and the gate that reads
/// it refuses growth rather than existence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unbacked {
    /// The command as a person types it, `majordomus web list`.
    pub command: String,
    /// The words after `majordomus`, which is what a [`CliExposure`](super::CliExposure)
    /// would have to carry for this command to be derived.
    pub path: Vec<String>,
}

/// Where one capability is projected, and where it is not.
///
/// This is the coverage matrix of the rule: a row per capability, a column per interface.
/// It is derived, never written down — a capability that reaches nothing shows as a row of
/// `false`, which is exactly the "exists but is invisible" case worth seeing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Row {
    /// The canonical id.
    pub id: String,
    /// The module that composes it.
    pub module: String,
    /// Query, command or resource.
    pub kind: String,
    /// Where it stands.
    pub stability: String,
    /// The command line, when it declares one and clap has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    /// The HTTP route, when it declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    /// The MCP tool name, when it declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_tool: Option<String>,
    /// The MCP resource URI, when it declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_resource: Option<String>,
    /// Whether every exposure this row declares is answered by its surface.
    pub closed: bool,
    /// Where the capability is declared; repository-relative.
    pub source: String,
}

/// The whole matrix, with the findings and the debt beside it: one value that answers
/// "where does each capability appear, and is any claim unmet".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Matrix {
    /// One row per capability, in id order.
    pub rows: Vec<Row>,
    /// Every unmet claim, in capability order. Empty is the rule satisfied.
    #[serde(skip)]
    pub findings: Vec<Finding>,
    /// Runnable commands of the command line that no capability claims, in command order.
    pub unbacked: Vec<String>,
}

/// Every unmet claim, in capability order, then in projection order.
///
/// Pure: the registry and the clap tree, nothing read from disk. An empty result is the
/// closure the rule asks for.
///
/// ```
/// # use majordomus_cli::capability::{closure, CapabilityRegistry};
/// // the command line this crate declares answers every claim made about it
/// let registry = CapabilityRegistry::builder()
///     .with_modules(majordomus_cli::capability::builtin::modules())
///     .build()
///     .expect("the builtin registry builds");
/// assert!(closure::findings(&registry, &majordomus_cli::cli::tree()).is_empty());
/// ```
pub fn findings(registry: &CapabilityRegistry, tree: &CommandDoc) -> Vec<Finding> {
    let commands = index(tree);
    let mut out = Vec::new();
    for c in registry.iter() {
        let Some(cli) = &c.exposure.cli else { continue };
        let claim = format!("majordomus {}", cli.path.join(" "));
        match commands.get(cli.path.as_slice()) {
            None => out.push(Finding {
                code: "CLOSURE_CLI_ABSENT",
                capability: c.id.to_string(),
                projection: Projection::Cli,
                claim,
                detail: format!(
                    "the clap declaration has no command `{}`",
                    cli.path.join(" ")
                ),
                source: c.provenance.source_path(),
                rule: RULE,
            }),
            Some(cmd) if !cmd.executable => out.push(Finding {
                code: "CLOSURE_CLI_NOT_RUNNABLE",
                capability: c.id.to_string(),
                projection: Projection::Cli,
                claim,
                detail:
                    "the command exists but only groups other commands and cannot be run on its own"
                        .to_string(),
                source: c.provenance.source_path(),
                rule: RULE,
            }),
            Some(_) => {}
        }
    }
    out.sort_by(|a, b| {
        (a.capability.as_str(), a.projection, a.code).cmp(&(
            b.capability.as_str(),
            b.projection,
            b.code,
        ))
    });
    out
}

/// Every runnable command of the command line that no capability claims, in command order.
///
/// ```
/// # use majordomus_cli::capability::{closure, CapabilityRegistry};
/// let registry = CapabilityRegistry::builder()
///     .with_modules(majordomus_cli::capability::builtin::modules())
///     .build()
///     .unwrap();
/// let debt = closure::unbacked(&registry, &majordomus_cli::cli::tree());
/// // `serve` starts a process and is not a capability; it is in the inventory by design
/// assert!(debt.iter().any(|u| u.command == "majordomus serve"));
/// ```
pub fn unbacked(registry: &CapabilityRegistry, tree: &CommandDoc) -> Vec<Unbacked> {
    let declared: BTreeSet<&[String]> = registry
        .iter()
        .filter_map(|c| c.exposure.cli.as_ref().map(|e| e.path.as_slice()))
        .collect();
    let mut out: Vec<Unbacked> = tree
        .flatten()
        .into_iter()
        // the root groups everything and is nobody's exposure
        .filter(|c| c.executable && c.path.len() > 1)
        .filter(|c| !declared.contains(&c.path[1..]))
        .map(|c| Unbacked {
            command: c.command(),
            path: c.path[1..].to_vec(),
        })
        .collect();
    out.sort();
    out
}

/// The matrix, the findings and the debt in one pass.
pub fn matrix(registry: &CapabilityRegistry, tree: &CommandDoc) -> Matrix {
    let commands = index(tree);
    let mut rows: Vec<Row> = registry.iter().map(|c| row(c, &commands)).collect();
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Matrix {
        rows,
        findings: findings(registry, tree),
        unbacked: unbacked(registry, tree)
            .into_iter()
            .map(|u| u.command)
            .collect(),
    }
}

fn row(c: &Capability, commands: &BTreeMap<&[String], &CommandDoc>) -> Row {
    let cli = c.exposure.cli.as_ref();
    // the claim is shown only when the surface answers it; a row that reports a command
    // clap does not have would be the same lie the findings are there to catch
    let cli_present = cli
        .map(|e| {
            commands
                .get(e.path.as_slice())
                .is_some_and(|cmd| cmd.executable)
        })
        .unwrap_or(false);
    Row {
        id: c.id.to_string(),
        module: c.module.to_string(),
        kind: format!("{:?}", c.kind).to_lowercase(),
        stability: format!("{:?}", c.stability).to_lowercase(),
        cli: cli
            .filter(|_| cli_present)
            .map(|e| format!("majordomus {}", e.path.join(" "))),
        http: c
            .exposure
            .http
            .as_ref()
            .map(|h| format!("{} {}", h.method.as_str(), h.path)),
        mcp_tool: c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
        mcp_resource: c
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref().map(|r| r.uri.clone())),
        closed: cli.is_none() || cli_present,
        source: c.provenance.source_path(),
    }
}

/// The clap tree by path-after-`majordomus`, so a claim is looked up rather than searched.
fn index(tree: &CommandDoc) -> BTreeMap<&[String], &CommandDoc> {
    tree.flatten()
        .into_iter()
        .filter(|c| c.path.len() > 1)
        .map(|c| (&c.path[1..], c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::model::{
        Availability, BenchmarkPolicy, CachePolicy, CapabilityId, CapabilityKind, CliExposure,
        Exposure, ModuleId, Provenance, Stability, Visibility,
    };
    use crate::capability::schema::CanonicalSchema;

    /// A descriptor claiming one CLI path, with nothing else that matters here.
    fn claiming(id: &str, path: &[&str]) -> Capability {
        let exposure = Exposure {
            cli: Some(CliExposure {
                path: path.iter().map(|w| w.to_string()).collect(),
            }),
            ..Default::default()
        };
        Capability {
            id: CapabilityId::unchecked(id),
            module: ModuleId::unchecked(id.split('.').next().unwrap()),
            kind: CapabilityKind::Query,
            title: "T".into(),
            description: "D".into(),
            input: CanonicalSchema::of::<()>(),
            output: CanonicalSchema::of::<()>(),
            provenance: Provenance::Builtin {
                module: "majordomus_cli::capability::builtin::demo".into(),
            },
            availability: Availability::classify(CapabilityKind::Query, &exposure),
            visibility: Visibility::classify(&exposure),
            exposure,
            stability: Stability::Experimental,
            tags: vec![],
            benchmark: BenchmarkPolicy::Required,
            cache: CachePolicy::Disabled,
        }
    }

    /// The check is a pure function of two structures, so a finding can be produced without
    /// a registry: this drives the same comparison `findings` runs, over one descriptor.
    fn check_one(c: &Capability, tree: &CommandDoc) -> Vec<Finding> {
        let commands = index(tree);
        let mut out = Vec::new();
        let cli = c.exposure.cli.as_ref().unwrap();
        let claim = format!("majordomus {}", cli.path.join(" "));
        match commands.get(cli.path.as_slice()) {
            None => out.push(Finding {
                code: "CLOSURE_CLI_ABSENT",
                capability: c.id.to_string(),
                projection: Projection::Cli,
                claim,
                detail: String::new(),
                source: c.provenance.source_path(),
                rule: RULE,
            }),
            Some(cmd) if !cmd.executable => out.push(Finding {
                code: "CLOSURE_CLI_NOT_RUNNABLE",
                capability: c.id.to_string(),
                projection: Projection::Cli,
                claim,
                detail: String::new(),
                source: c.provenance.source_path(),
                rule: RULE,
            }),
            Some(_) => {}
        }
        out
    }

    #[test]
    fn a_claim_the_command_line_does_not_answer_is_a_finding() {
        let tree = crate::cli::tree();
        let bogus = claiming("demo.ghost", &["ghost", "walks"]);
        let found = check_one(&bogus, &tree);
        assert_eq!(found.len(), 1, "an absent command is one finding");
        assert_eq!(found[0].code, "CLOSURE_CLI_ABSENT");
        assert_eq!(found[0].capability, "demo.ghost");
        assert_eq!(found[0].claim, "majordomus ghost walks");
        // the finding names the file to edit, repository-relative
        assert_eq!(
            found[0].source,
            "apps/majordomus-cli/src/capability/builtin/demo.rs"
        );
        assert!(!found[0].source.starts_with('/'), "never a machine path");
    }

    #[test]
    fn a_claim_on_a_group_that_cannot_be_run_is_a_finding() {
        let tree = crate::cli::tree();
        // the group is found in the tree rather than named here: a command that requires a
        // subcommand today may gain a default tomorrow, and this test should follow it
        let group = tree
            .flatten()
            .into_iter()
            .find(|c| !c.executable && c.path.len() > 1)
            .expect("the command line has at least one command that only groups others");
        let words: Vec<&str> = group.path[1..].iter().map(String::as_str).collect();
        let found = check_one(&claiming("demo.group", &words), &tree);
        assert_eq!(found.len(), 1, "{} should be unrunnable", group.command());
        assert_eq!(found[0].code, "CLOSURE_CLI_NOT_RUNNABLE");
    }

    #[test]
    fn a_claim_the_command_line_answers_is_no_finding() {
        let tree = crate::cli::tree();
        let runnable: Vec<Vec<&str>> = tree
            .flatten()
            .into_iter()
            .filter(|c| c.executable && c.path.len() > 1)
            .map(|c| c.path[1..].iter().map(String::as_str).collect())
            .collect();
        assert!(!runnable.is_empty(), "the command line has runnable commands");
        for words in &runnable {
            assert!(
                check_one(&claiming("demo.ok", words), &tree).is_empty(),
                "majordomus {} is runnable and must not be a finding",
                words.join(" ")
            );
        }
    }

    #[test]
    fn the_declaration_this_crate_ships_is_closed() {
        let registry = CapabilityRegistry::builder()
            .with_modules(crate::capability::builtin::modules())
            .build()
            .expect("the builtin registry builds");
        let tree = crate::cli::tree();
        let unmet = findings(&registry, &tree);
        assert!(
            unmet.is_empty(),
            "every declared CLI exposure must be a runnable command:\n{}",
            unmet
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn the_matrix_reports_a_row_per_capability_and_hides_nothing_it_cannot_answer() {
        let registry = CapabilityRegistry::builder()
            .with_modules(crate::capability::builtin::modules())
            .build()
            .unwrap();
        let tree = crate::cli::tree();
        let m = matrix(&registry, &tree);
        assert_eq!(m.rows.len(), registry.len());
        assert!(m.findings.is_empty());
        assert!(m.rows.iter().all(|r| r.closed), "the shipped tree is closed");
        // ordering is by id and therefore stable across runs
        let ids: Vec<&str> = m.rows.iter().map(|r| r.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        // every row that claims a command names one a person can actually type
        let commands = index(&tree);
        for r in m.rows.iter().filter(|r| r.cli.is_some()) {
            let words: Vec<String> = r.cli.as_ref().unwrap()["majordomus ".len()..]
                .split(' ')
                .map(str::to_string)
                .collect();
            assert!(
                commands.get(words.as_slice()).is_some_and(|c| c.executable),
                "{} reports a command that cannot be run",
                r.id
            );
        }
    }

    #[test]
    fn the_debt_is_the_commands_no_capability_claims() {
        let registry = CapabilityRegistry::builder()
            .with_modules(crate::capability::builtin::modules())
            .build()
            .unwrap();
        let tree = crate::cli::tree();
        let debt = unbacked(&registry, &tree);
        // the process commands have no capability behind them and are expected here
        for expected in ["majordomus serve", "majordomus mcp", "majordomus generate"] {
            assert!(
                debt.iter().any(|u| u.command == expected),
                "{expected} should be in the inventory"
            );
        }
        // and a command that *is* a capability is not
        assert!(
            !debt.iter().any(|u| u.command == "majordomus why list"),
            "a derived command is not debt"
        );
        // the root is never debt: it groups and is nobody's exposure
        assert!(!debt.iter().any(|u| u.command == "majordomus"));
        let mut sorted = debt.clone();
        sorted.sort();
        assert_eq!(debt, sorted, "the inventory is ordered, so a baseline is stable");
    }
}
