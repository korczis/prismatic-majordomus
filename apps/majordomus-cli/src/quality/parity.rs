//! Transport parity: the canonical registry against every projection of it.
//!
//! # What parity means here
//!
//! A capability is declared once and projected to MCP, to HTTP, to the OpenAPI document
//! and to the command line. The crate's own suites already prove that a *declared*
//! projection exists and that no projection carries an orphan — that is
//! `tests/projections.rs`, and this module does not repeat it. What no test could see is
//! the other direction on the command line: a command that is nobody's projection.
//!
//! The command line is the only projection with commands of its own, so it is the only one
//! where an operation can go missing from the API by nobody noticing. This module closes
//! that: every runnable command is either bound to a capability by its
//! [`CliExposure`](crate::capability::CliExposure), or classified in
//! [`crate::cli::local::LOCAL`] with a reason that is itself checked.
//!
//! # The three ways it fails
//!
//! ```text
//! runnable command, no capability, no classification   -> OPERATION_CLI_UNCLASSIFIED
//! classification naming no command                     -> OPERATION_CLASSIFICATION_STALE
//! classification of a command that is a capability     -> OPERATION_CLASSIFICATION_CONFLICT
//! ```
//!
//! and a fourth for the claims a classification makes: a rendering must name a capability
//! that exists and answers over HTTP, and an alias must name a command that exists.
//!
//! ```
//! use majordomus_cli::quality::parity;
//! use majordomus_cli::cli;
//!
//! // the command tree is pure, so parity's command inventory can be read without a repository
//! let commands = parity::runnable_commands(&cli::tree());
//! assert!(commands.iter().any(|c| c == "serve"));
//! assert!(!commands.iter().any(|c| c == "capabilities"), "a pure group runs nothing");
//! ```

use crate::capability::{CapabilityRegistry, HttpMethod};
use crate::cli::local::{self, LocalReason, LOCAL};
use crate::cli::CommandDoc;

use super::model::{OperationParity, Violation, ViolationCode};

/// Every command a person can run, as they type it after `majordomus`.
///
/// A command that only groups others is not one: it runs nothing, so there is no operation
/// for it to be or to be missing. That is read from clap's own declaration — a command that
/// grows a required subcommand stops being runnable on the same edit — and never from a list.
///
/// ```
/// use majordomus_cli::{cli, quality::parity};
/// let commands = parity::runnable_commands(&cli::tree());
/// assert!(commands.iter().any(|c| c == "capabilities list"));
/// assert!(commands.iter().all(|c| !c.starts_with("majordomus")), "the executable's own name is not part of a command");
/// ```
pub fn runnable_commands(tree: &CommandDoc) -> Vec<String> {
    tree.flatten()
        .into_iter()
        .filter(|c| c.path.len() > 1 && c.executable)
        .map(|c| c.path[1..].join(" "))
        .collect()
}

/// Measure the registry against its projections, and against the command line.
///
/// `openapi` is the generated document; parity asserts that every capability declaring an
/// HTTP route is described by it, which is what makes Swagger UI and every generated
/// client a reading of the registry rather than of a hand-kept file.
///
/// ```
/// use majordomus_cli::{cli, quality::parity};
/// use majordomus_cli::capability::CapabilityRegistry;
/// use serde_json::json;
///
/// // an empty registry, an empty document: the command line is then the whole finding set
/// let registry = CapabilityRegistry::builder().build().unwrap();
/// let (counts, findings) = parity::inspect(&registry, &cli::tree(), &json!({"paths": {}}));
/// assert_eq!(counts.canonical, 0);
/// assert!(counts.cli_commands > 0, "the command line has commands regardless");
/// assert!(!findings.is_empty(), "every command is then unaccounted for");
/// ```
pub fn inspect(
    registry: &CapabilityRegistry,
    tree: &CommandDoc,
    openapi: &serde_json::Value,
) -> (OperationParity, Vec<Violation>) {
    let mut findings = Vec::new();
    let mut counts = OperationParity::default();

    let declaration = crate::cli::DECLARATION;

    // ---------------------------------------------------------------- the registry
    for c in registry.iter().filter(|c| c.kind.is_executable()) {
        counts.canonical += 1;
        if c.exposure.cli.is_some() {
            counts.cli += 1;
        }
        if c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()) {
            counts.mcp += 1;
        }
        let Some(http) = &c.exposure.http else {
            continue;
        };
        counts.http += 1;
        let described = openapi
            .get("paths")
            .and_then(|p| p.get(&http.path))
            .and_then(|m| m.get(http.method.as_str().to_lowercase()))
            .is_some();
        if described {
            counts.openapi += 1;
        } else {
            findings.push(Violation::new(
                ViolationCode::OperationMissingOpenapi,
                c.id.to_string(),
                c.provenance.source_path(),
                None,
                format!(
                    "declares {} {} and the OpenAPI document describes no such operation",
                    http.method.as_str(),
                    http.path
                ),
            ));
        }
    }

    // ---------------------------------------------------------------- the command line
    let commands = runnable_commands(tree);
    counts.cli_commands = commands.len();
    for command in &commands {
        let words: Vec<String> = command.split(' ').map(str::to_string).collect();
        let capability = registry.by_cli(&words);
        let classified = local::find(command);
        match (capability, classified) {
            (Some(_), None) => counts.cli_from_capability += 1,
            (None, Some(_)) => counts.cli_local += 1,
            (Some(cap), Some(_)) => {
                counts.cli_from_capability += 1;
                findings.push(Violation::new(
                    ViolationCode::OperationClassificationConflict,
                    format!("majordomus {command}"),
                    declaration,
                    None,
                    format!(
                        "is the projection of {} and is also listed in cli::LOCAL",
                        cap.id
                    ),
                ));
            }
            (None, None) => findings.push(Violation::new(
                ViolationCode::OperationCliUnclassified,
                format!("majordomus {command}"),
                declaration,
                None,
                "is runnable, is the projection of no capability, and cli::LOCAL does not say why"
                    .to_string(),
            )),
        }
    }

    // ---------------------------------------------------------------- the classification
    for entry in LOCAL {
        if !commands.iter().any(|c| c == entry.command) {
            findings.push(Violation::new(
                ViolationCode::OperationClassificationStale,
                format!("majordomus {}", entry.command),
                declaration,
                None,
                "is listed in cli::LOCAL and the command line has no runnable command by that name"
                    .to_string(),
            ));
            continue;
        }
        match entry.reason {
            LocalReason::RendersCapability(id) => {
                let Some(cap) = registry.get(id) else {
                    findings.push(Violation::new(
                        ViolationCode::OperationClassificationStale,
                        format!("majordomus {}", entry.command),
                        declaration,
                        None,
                        format!("is said to render '{id}', which the registry does not hold"),
                    ));
                    continue;
                };
                if cap.exposure.http.is_none() {
                    findings.push(Violation::new(
                        ViolationCode::OperationClassificationStale,
                        format!("majordomus {}", entry.command),
                        declaration,
                        None,
                        format!(
                            "is said to render {id}, which is exposed over no HTTP route, so the operation is not in the API after all"
                        ),
                    ));
                }
            }
            LocalReason::Alias(target) => {
                if !commands.iter().any(|c| c == target) {
                    findings.push(Violation::new(
                        ViolationCode::OperationClassificationStale,
                        format!("majordomus {}", entry.command),
                        declaration,
                        None,
                        format!("is said to be another name for '{target}', which is not a runnable command"),
                    ));
                }
            }
            LocalReason::ProcessLifecycle
            | LocalReason::WritesRepository
            | LocalReason::SessionLocal => {}
        }
    }

    // ---------------------------------------------------------------- orphans in OpenAPI
    if let Some(paths) = openapi.get("paths").and_then(|p| p.as_object()) {
        for (path, methods) in paths {
            let Some(methods) = methods.as_object() else {
                continue;
            };
            for method in methods.keys() {
                let Some(m) = HttpMethod::parse(&method.to_uppercase()) else {
                    continue;
                };
                if registry.by_http(m, path).is_none() {
                    findings.push(Violation::new(
                        ViolationCode::OperationProjectionOrphan,
                        format!("{} {path}", method.to_uppercase()),
                        "docs/generated/openapi.json",
                        None,
                        "is described by the OpenAPI document and declared by no capability"
                            .to_string(),
                    ));
                }
            }
        }
    }

    (counts, findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli;

    #[test]
    fn a_pure_group_is_not_a_command_and_a_runnable_parent_is() {
        let commands = runnable_commands(&cli::tree());
        // `capabilities` requires a subcommand: it runs nothing
        assert!(!commands.iter().any(|c| c == "capabilities"));
        // `why` runs the listing when none follows it, so it is runnable and classified
        assert!(commands.iter().any(|c| c == "why"));
        assert!(local::find("why").is_some());
    }

    #[test]
    fn no_classification_names_a_command_or_a_capability_that_is_not_there() {
        // the whole point of the entry list: it cannot go stale without this failing. Run
        // against the real registry, because a rendering entry names a capability in it.
        let registry = CapabilityRegistry::builder()
            .with_builtin(crate::capability::builtin::all())
            .build()
            .unwrap();
        let (_, findings) = inspect(&registry, &cli::tree(), &serde_json::json!({"paths": {}}));
        assert!(
            findings
                .iter()
                .all(|f| f.code != ViolationCode::OperationClassificationStale),
            "no entry of LOCAL names a command the tree does not have: {:?}",
            findings
                .iter()
                .filter(|f| f.code == ViolationCode::OperationClassificationStale)
                .map(|f| f.symbol.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_openapi_document_missing_a_declared_route_is_reported() {
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_builtin(crate::capability::builtin::all())
            .build()
            .unwrap();
        let (counts, findings) =
            inspect(&registry, &cli::tree(), &serde_json::json!({"paths": {}}));
        assert!(counts.http > 0);
        assert_eq!(
            findings
                .iter()
                .filter(|f| f.code == ViolationCode::OperationMissingOpenapi)
                .count(),
            counts.http,
            "an empty document describes none of them"
        );
    }
}
