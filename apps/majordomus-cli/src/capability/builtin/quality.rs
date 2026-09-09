//! The `quality` module: what this executable's own public surface is held to, answered
//! through every transport the surface is measured against.
//!
//! This is the subsystem dogfooding itself. `project.rust-public-api-quality` and
//! `project.operation-transport-parity` are rules about the crate; the measurement of
//! them is a capability of the crate, declared once here, and therefore reaches the
//! command line, HTTP, the OpenAPI document, MCP and the Cockpit as projections rather
//! than as five renderers. A quality subsystem that needed hand-written adapters would be
//! failing the architecture it exists to enforce.
//!
//! The measurement reads the crate's sources from the repository the index was built from.
//! A repository that carries no Rust crate — the layer installs into plenty of them — is
//! answered with `measured: false` and the reason, never with a clean report over nothing.
//!
//! ```
//! use majordomus_cli::capability::builtin::quality::module;
//! let m = module();
//! let c = m.capabilities.iter().find(|c| c.capability.id.as_str() == "quality.report").unwrap();
//! assert_eq!(c.capability.exposure.http.as_ref().unwrap().path, "/api/v1/quality");
//! assert_eq!(
//!     c.capability.exposure.cli.as_ref().unwrap().path,
//!     vec!["quality".to_string(), "report".to_string()]
//! );
//! ```

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::quality::{QualityReport, ViolationCode};
use crate::{capability, module};

use super::get;

/// The URI under which the report is read as an MCP resource.
pub const QUALITY_URI: &str = "majordomus://quality";

/// Where the crate this executable is built from lives, relative to the repository root.
/// The one fact the measurement needs that the index does not carry.
pub const CRATE_DIR: &str = crate::capability::model::CRATE_DIR;

/// The input of `quality.report`: which findings to answer with.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct QualityInput {
    /// Only findings carrying this code (`RUST_PUBLIC_MISSING_EXAMPLE`). All of them when
    /// absent. A code nothing recognises is refused rather than answered with an empty
    /// list, so a typo cannot read as a clean report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Only findings whose file path starts with this, repository-relative
    /// (`apps/majordomus-cli/src/web`). All of them when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Answer with the counts and leave the findings out. For a caller that wants the
    /// state of the crate and not the list of what to do about it.
    #[serde(default)]
    pub summary_only: bool,
    /// Include the findings the ratchet already accepts, which are left out by default.
    ///
    /// The default is what a gate wants: only what is new. This is what a person paying the
    /// debt down wants, and it is what `--write-baseline` must ask for — a baseline written
    /// from a report that had the baseline applied to it would empty the file, accepting
    /// nothing and failing on everything the next time it ran.
    #[serde(default)]
    pub include_baselined: bool,
}

impl BenchmarkCases for QualityInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // One case, and deliberately one: the cost of this capability is parsing the crate,
        // which every input pays identically, so a second case would time the same work
        // again and lengthen every benchmark run for no evidence.
        vec![NamedCase::new("default", QualityInput::default())]
    }
}

/// The answer: the measurement, or the reason there was not one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QualityAnswer {
    /// Whether a crate was found and measured. `false` is a complete answer, not a failure:
    /// the layer installs into repositories that carry no Rust crate, and a doctrine that
    /// cannot apply is not a violation.
    pub measured: bool,
    /// Why nothing was measured, when nothing was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The measurement. Present and empty-of-findings when `measured` is false, so that a
    /// consumer reads one shape either way and `measured` is the only question it must ask.
    pub report: QualityReport,
    /// Whether the report as filtered leaves nothing blocking. Answered here so that a
    /// caller over any transport reads the verdict rather than deriving it, and so that the
    /// command line's exit code and this field can never disagree.
    pub passes: bool,
    /// The findings the ratchet accepts because they stood when the rule landed. A finding
    /// outside this count is what fails the gate.
    pub baselined: usize,
}

/// The crate directory of a repository, when it has one.
///
/// ```
/// use majordomus_cli::capability::builtin::quality::crate_dir;
/// let dir = tempfile::tempdir().unwrap();
/// assert_eq!(crate_dir(dir.path()), None, "a repository with no crate has none");
/// std::fs::create_dir_all(dir.path().join("apps/majordomus-cli/src")).unwrap();
/// std::fs::write(dir.path().join("apps/majordomus-cli/Cargo.toml"), "").unwrap();
/// std::fs::write(dir.path().join("apps/majordomus-cli/src/lib.rs"), "//! x\n").unwrap();
/// assert!(crate_dir(dir.path()).is_some());
/// ```
pub fn crate_dir(root: &Path) -> Option<PathBuf> {
    let dir = root.join(CRATE_DIR);
    (dir.join("Cargo.toml").is_file() && dir.join("src/lib.rs").is_file()).then_some(dir)
}

/// The findings the ratchet accepts, read from the repository's baseline file.
///
/// One finding per line, `CODE<tab>path<tab>symbol`, in the order the report sorts them.
/// A repository with no baseline file accepts none, which is the finished state the rule
/// describes.
///
/// ```
/// use majordomus_cli::capability::builtin::quality::baseline_keys;
/// let dir = tempfile::tempdir().unwrap();
/// assert!(baseline_keys(dir.path()).is_empty(), "no file, no debt accepted");
/// ```
pub fn baseline_keys(root: &Path) -> std::collections::BTreeSet<String> {
    let path = root.join(BASELINE);
    let Ok(text) = std::fs::read_to_string(path) else {
        return Default::default();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Where the ratchet's accepted findings are recorded, repository-relative.
pub const BASELINE: &str = ".ai/repo/rust-quality-baseline.txt";

/// The key of one finding in the baseline: its code, its file and its symbol.
///
/// The line is deliberately not part of it. A finding that moves down a file when something
/// above it is edited is the same finding, and a baseline keyed by line would turn every
/// unrelated edit into new debt.
///
/// ```
/// use majordomus_cli::capability::builtin::quality::baseline_key;
/// use majordomus_cli::quality::{Violation, ViolationCode};
/// let v = Violation::new(
///     ViolationCode::RustPublicMissingExample, "a::b", "src/a.rs", Some(9), "none");
/// assert_eq!(baseline_key(&v), "RUST_PUBLIC_MISSING_EXAMPLE\tsrc/a.rs\ta::b");
/// ```
pub fn baseline_key(violation: &crate::quality::Violation) -> String {
    format!(
        "{}\t{}\t{}",
        violation.code.as_str(),
        violation.path,
        violation.symbol
    )
}

fn report(ctx: &Context, input: QualityInput) -> Result<QualityAnswer, CapabilityError> {
    if let Some(code) = &input.code {
        if !ViolationCode::all().iter().any(|c| c.as_str() == code) {
            return Err(CapabilityError::InvalidInput(format!(
                "'{code}' is not a violation code; the codes are: {}",
                ViolationCode::all()
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    let root = Path::new(&ctx.index.repository.root);
    let Some(dir) = crate_dir(root) else {
        return Ok(QualityAnswer {
            measured: false,
            reason: Some(format!(
                "no Rust crate at {CRATE_DIR}; there is nothing here for this rule to apply to"
            )),
            report: QualityReport {
                schema: crate::quality::SCHEMA.to_string(),
                target: CRATE_DIR.to_string(),
                ..QualityReport::default()
            },
            passes: true,
            baselined: 0,
        });
    };

    let openapi = crate::http::openapi::document(&ctx.registry, crate::VERSION, None)
        .map_err(|e| CapabilityError::Internal(e.to_string()))?;
    let mut report = crate::quality::report::inspect(
        &dir,
        CRATE_DIR,
        &ctx.registry,
        &crate::cli::tree(),
        &openapi,
    )
    .map_err(|e| CapabilityError::Internal(e.to_string()))?;

    let accepted = baseline_keys(root);
    let baselined = report
        .violations
        .iter()
        .filter(|v| accepted.contains(&baseline_key(v)))
        .count();
    // the verdict is always about what the ratchet does *not* accept, whether or not the
    // caller asked to see the rest: a report that showed the debt and then failed on it
    // would make `--include-baselined` a way of failing a build that passes
    let passes = report
        .violations
        .iter()
        .all(|v| accepted.contains(&baseline_key(v)));
    if !input.include_baselined {
        report
            .violations
            .retain(|v| !accepted.contains(&baseline_key(v)));
    }

    if let Some(code) = &input.code {
        report.violations.retain(|v| v.code.as_str() == code);
    }
    if let Some(prefix) = &input.path {
        report.violations.retain(|v| v.path.starts_with(prefix));
    }
    if input.summary_only {
        report.violations.clear();
    }
    Ok(QualityAnswer {
        measured: true,
        reason: None,
        report,
        passes,
        baselined,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "quality",
        title: "Public API quality",
        description: "What this executable's own public surface is held to, measured from its syntax tree: documentation that says more than the signature, an executable example on everything that carries behaviour, a module boundary something exercises, and every command of the command line accounted for against the capability registry. The rules are project.rust-public-api-quality and project.operation-transport-parity; this is the measurement of them.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "quality.report",
                title: "Public API quality report",
                description: "The crate's exported surface measured against the repository's rules: how many items are documented and exampled, how many modules are documented, exampled and behaviourally tested, how the canonical operations stand against the command line, HTTP, OpenAPI and MCP, and one finding per violation carrying a stable code, the rule that requires it, its file and line, why it matters and what to do about it.",
                input: QualityInput,
                output: QualityAnswer,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_quality".into()),
                        resource: Some(McpResource { uri: QUALITY_URI.into(), name: "quality".into() }),
                    }),
                    http: get("/api/v1/quality"),
                    cli: Some(CliExposure { path: vec!["quality".into(), "report".into()] }),
                },
                tags: ["quality", "introspection", "rust"],
                // the crate is parsed on every call and the sources do not change under a
                // running process; a short-lived entry keeps a Cockpit page that polls from
                // reparsing a hundred files each time
                cache: CachePolicy::Process { max_entries: 8, ttl_seconds: Some(10) },
                handler: report,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place the id and the three projection names exist; a
    /// refactor that dropped one would still compile and every test of the measurement
    /// itself would still pass.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "quality");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["quality.report"]);
        let e = &m.capabilities[0].capability.exposure;
        assert_eq!(
            e.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_quality")
        );
        assert_eq!(
            e.mcp
                .as_ref()
                .and_then(|m| m.resource.as_ref())
                .map(|r| r.uri.as_str()),
            Some(QUALITY_URI)
        );
        assert_eq!(
            e.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/quality")
        );
        assert_eq!(
            e.cli.as_ref().map(|c| c.path.clone()),
            Some(vec!["quality".to_string(), "report".to_string()])
        );
    }

    #[test]
    fn a_baseline_key_survives_the_finding_moving_down_its_file() {
        use crate::quality::{Violation, ViolationCode};
        let at = |line| {
            Violation::new(
                ViolationCode::RustPublicMissingExample,
                "majordomus_cli::a::b",
                "apps/majordomus-cli/src/a.rs",
                Some(line),
                "none",
            )
        };
        assert_eq!(baseline_key(&at(9)), baseline_key(&at(400)));
    }

    #[test]
    fn a_repository_with_no_crate_is_answered_and_never_failed() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(crate_dir(dir.path()), None);
        // and the answer's shape is the same one a measured repository gets
        let answer = QualityAnswer {
            measured: false,
            reason: Some("no crate".into()),
            report: QualityReport::default(),
            passes: true,
            baselined: 0,
        };
        let json = serde_json::to_value(&answer).unwrap();
        assert_eq!(json["measured"], false);
        assert_eq!(json["passes"], true);
        assert!(json.get("report").is_some(), "one shape either way");
    }
}
