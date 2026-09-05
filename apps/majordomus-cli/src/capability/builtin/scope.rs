//! The scope capabilities of the `repository` module: the repository scope as data, and
//! any path judged against it. They are composed in `repository.rs`; the declarative
//! `scope` kind keeps the `scope` module id for the declaration itself.
//!
//! The scope is compiled once at start-up and carried by the index; nothing here reads the
//! declaration again. `repository.scope` answers the declaration, where it came from and
//! every tracked file tallied against it; `repository.scope_classify` judges one path by
//! name, size and content, and names the rule that decided.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::scope::{Classification, Declaration, Origin, Tally};

use super::Empty;

/// The URI under which `repository.scope` is read as an MCP resource.
pub const SCOPE_URI: &str = "majordomus://scope";

// ---------------------------------------------------------------- repository.scope

/// The scope: the declaration as read, its origin, and every tracked file against it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScopeReport {
    /// `repository` when the manifest names a scope section, `distribution` for the default.
    pub origin: Origin,
    /// The file the declaration was read from.
    pub path: String,
    /// The declaration as read.
    pub declaration: Declaration,
    /// Every tracked file judged by name and size, counted; the out ones listed.
    pub tracked: Tally,
}

pub(super) fn scope_info(ctx: &Context, _: Empty) -> Result<ScopeReport, CapabilityError> {
    let scoped = &ctx.index.scoped;
    Ok(ScopeReport {
        origin: scoped.scope.origin(),
        path: scoped.scope.path().to_string(),
        declaration: scoped.scope.declaration().clone(),
        tracked: scoped.tally.clone(),
    })
}

// ---------------------------------------------------------------- repository.scope_classify

/// Which path to judge.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClassifyInput {
    /// Repository-relative, forward slashes; `./` is stripped. An absolute path or a
    /// `..` segment is an invalid input.
    pub path: String,
}

impl BenchmarkCases for ClassifyInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases = vec![
            NamedCase::new(
                "layer-file",
                ClassifyInput {
                    path: ".ai/manifest.yaml".into(),
                },
            ),
            NamedCase::new(
                "local-half",
                ClassifyInput {
                    path: ".ai/local/state/current.yaml".into(),
                },
            ),
            NamedCase::new(
                "secret",
                ClassifyInput {
                    path: "config/.env".into(),
                },
            ),
            NamedCase::new(
                "undeclared",
                ClassifyInput {
                    path: "node_modules/left-pad/index.js".into(),
                },
            ),
        ];
        if let Some(o) = ctx.index.objects.first() {
            cases.push(NamedCase::new(
                "first-object",
                ClassifyInput {
                    path: o.provenance.path.clone(),
                },
            ));
        }
        cases
    }
}

/// A repository-relative path as the scope judges it: forward slashes, no `./`, no
/// trailing slash. Absolute paths and parent segments are refused.
///
/// ```
/// use majordomus_cli::capability::builtin::normalise_path;
/// assert_eq!(normalise_path("./docs/CLI.md").unwrap(), "docs/CLI.md");
/// assert_eq!(normalise_path("docs\\CLI.md").unwrap(), "docs/CLI.md");
/// assert_eq!(normalise_path("docs/").unwrap(), "docs");
/// assert!(normalise_path("/etc/passwd").is_err());
/// assert!(normalise_path("../x").is_err());
/// assert!(normalise_path("").is_err());
/// ```
pub fn normalise_path(raw: &str) -> Result<String, CapabilityError> {
    let mut p = raw.trim().replace('\\', "/");
    while let Some(rest) = p.strip_prefix("./") {
        p = rest.to_string();
    }
    while p.ends_with('/') && p.len() > 1 {
        p.pop();
    }
    if p.is_empty() || p == "." {
        return Err(CapabilityError::InvalidInput(
            "path is empty; name a repository-relative path".into(),
        ));
    }
    if p.starts_with('/') || p.chars().nth(1) == Some(':') {
        return Err(CapabilityError::InvalidInput(format!(
            "'{raw}' is absolute; name a repository-relative path"
        )));
    }
    if p.split('/').any(|s| s == "..") {
        return Err(CapabilityError::InvalidInput(format!(
            "'{raw}' carries a '..' segment; name a path inside the repository"
        )));
    }
    let collapsed: Vec<&str> = p
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    Ok(collapsed.join("/"))
}

pub(super) fn scope_classify(
    ctx: &Context,
    input: ClassifyInput,
) -> Result<Classification, CapabilityError> {
    let path = normalise_path(&input.path)?;
    let root = std::path::Path::new(&ctx.index.repository.root);
    Ok(ctx.index.scoped.scope.classify(root, &path))
}
