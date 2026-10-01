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
//! `quality.rustdoc` holds the tree rustdoc renders from the same crate to the same
//! inventory: every exported item has its page at the route rustdoc gives it, the tree is
//! HEAD's, and nothing in it is broken or leaks the machine it was built on. Its subject is
//! the `rustdoc` web surface's artifact as the topology resolves it — never a path a caller
//! names — and a tree that is not there is the `no_tree` verdict, exit 12, never a pass.
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
use crate::quality::rustdoc::{RustdocFindingKind, RustdocReport, Subject, Tree};
use crate::quality::{QualityReport, ViolationCode};
use crate::web::discover::{discover, Runtime, GENERATED_ROOT};
use crate::{capability, module};

use super::get;

/// The URI under which the report is read as an MCP resource.
pub const QUALITY_URI: &str = "majordomus://quality";

/// The URI under which the judgement of the rustdoc tree is read as an MCP resource.
pub const RUSTDOC_URI: &str = "majordomus://quality/rustdoc";

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

/// The input of `quality.rustdoc`: which findings to answer with.
///
/// There is deliberately no field naming the tree. The subject is the `rustdoc` surface's
/// artifact as the repository's web topology resolves it, so a caller over HTTP or MCP can
/// ask about the one tree the repository publishes and about no other directory of the
/// machine; the command line's `--tree` is a local affordance of a person at their own
/// terminal, and never reaches this input.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct RustdocInput {
    /// Only findings of this kind (`missing-page`, `link`, `stale`). All of them when
    /// absent. The verdict and the counts are never narrowed: a filter that changed the
    /// verdict would be a way of passing a tree with a missing page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<RustdocFindingKind>,
    /// Answer with the verdict, the counts and the module routes, and leave the findings
    /// out.
    #[serde(default)]
    pub summary_only: bool,
}

impl BenchmarkCases for RustdocInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // the cost is the crate's parse and one walk of the tree, which every input pays;
        // the second case exists so that each optional parameter has an example
        vec![
            NamedCase::new("default", RustdocInput::default()),
            NamedCase::new(
                "missing-pages-summary",
                RustdocInput {
                    kind: Some(RustdocFindingKind::MissingPage),
                    summary_only: true,
                },
            ),
        ]
    }
}

/// The tree `quality.rustdoc` judges: the artifact of the `rustdoc` surface, as the web
/// topology of the repository at `root` resolves it.
///
/// A topology that declares no such surface, or one that cannot be read, is a tree that is
/// not there — with the reason, which names the producer — and never a tree that is
/// clean.
pub(crate) fn rustdoc_tree(root: &Path) -> Tree {
    use crate::quality::rustdoc::{PRODUCER, SURFACE};
    let shown = format!("{GENERATED_ROOT}/{SURFACE}");
    let topology = match discover(root, Runtime::full()) {
        Ok(t) => t,
        Err(e) => {
            let reason =
                format!("the web topology cannot be read, so '{SURFACE}' cannot be found: {e}");
            return Tree::Absent { shown, reason };
        }
    };
    let Some(surface) = topology.get(SURFACE) else {
        return Tree::Absent {
            shown,
            reason: format!(
                "the web topology declares no '{SURFACE}' surface: discovery declares it from the crate, which this repository does not have, so {PRODUCER} has nothing to build"
            ),
        };
    };
    let Some(artifact) = &surface.artifact else {
        return Tree::Absent {
            shown,
            reason: format!("the '{SURFACE}' surface declares no artifact to judge"),
        };
    };
    Tree::At {
        dir: root.join(artifact),
        shown: artifact.to_string_lossy().replace('\\', "/"),
        mount: Some(surface.mount.as_str().to_string()),
    }
}

/// Judge a tree of the repository at `root` against its crate: the one call the capability
/// and the command line's `--tree` share, so the two cannot judge differently.
pub(crate) fn judge_rustdoc(root: &Path, tree: Tree) -> Result<RustdocReport, crate::Error> {
    let dir = crate_dir(root);
    let head = crate::worktree::git::head_of(root).ok().flatten();
    crate::quality::rustdoc::judge(&Subject {
        root,
        crate_dir: dir.as_deref(),
        tree,
        head: head.as_deref(),
    })
}

fn rustdoc(ctx: &Context, input: RustdocInput) -> Result<RustdocReport, CapabilityError> {
    let root = Path::new(&ctx.index.repository.root);
    let report = judge_rustdoc(root, rustdoc_tree(root))
        .map_err(|e| CapabilityError::Internal(e.to_string()))?;
    Ok(report.filtered(input.kind, input.summary_only))
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
            capability! {
                id: "quality.rustdoc",
                title: "Rustdoc tree integrity",
                description: "The crate's rustdoc tree — the rustdoc surface's artifact, as the web topology resolves it — judged against the crate's own inventory of exported items: every item that owns a page has it at the route rustdoc gives it, no item page is left without an item, the tree declares it was built from HEAD, the library's index is present and names the crate, its assets are present, every relative link resolves, and no file names the machine it was built on or carries a credential. Answers the verdict (clean, findings, or no_tree when there is nothing to judge), the counts it joined, every exported module with its page, and one typed finding per defect with the file and what to do.",
                input: RustdocInput,
                output: RustdocReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_quality_rustdoc".into()),
                        resource: Some(McpResource { uri: RUSTDOC_URI.into(), name: "quality-rustdoc".into() }),
                    }),
                    http: get("/api/v1/quality/rustdoc"),
                    cli: Some(CliExposure { path: vec!["quality".into(), "rustdoc".into()] }),
                },
                tags: ["quality", "introspection", "rust", "documentation"],
                // one walk of a tree of thousands of files per call; a Cockpit page that
                // polls reads the same answer for a few seconds instead of walking again
                cache: CachePolicy::Process { max_entries: 8, ttl_seconds: Some(10) },
                handler: rustdoc,
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
        assert_eq!(ids, ["quality.report", "quality.rustdoc"]);
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

    /// A repository with the crate, so that whichever source declares the `rustdoc`
    /// surface — the crate itself, or the producer's declaration — declares it here.
    fn repository_with_the_crate() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let krate = dir.path().join(CRATE_DIR);
        std::fs::create_dir_all(krate.join("src")).unwrap();
        std::fs::write(
            krate.join("Cargo.toml"),
            "[package]\nname = \"majordomus-cli\"\n[lib]\nname = \"majordomus_cli\"\n",
        )
        .unwrap();
        std::fs::write(krate.join("src/lib.rs"), "//! Root.\n").unwrap();
        dir
    }

    #[test]
    fn the_rustdoc_tree_is_the_topologys_surface_and_never_a_callers_path() {
        use crate::quality::rustdoc::{PRODUCER, SURFACE};
        // no crate, so nothing declares the surface: absent, naming the producer
        let bare = tempfile::tempdir().unwrap();
        match rustdoc_tree(bare.path()) {
            Tree::Absent { shown, reason } => {
                assert_eq!(shown, format!("{GENERATED_ROOT}/{SURFACE}"));
                assert!(reason.contains(PRODUCER), "{reason}");
            }
            other => panic!("{other:?}"),
        }
        // the producer ran: the artifact and the mount are the topology's
        let repo = repository_with_the_crate();
        let out = repo.path().join(GENERATED_ROOT).join(SURFACE);
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            out.join("surface.json"),
            r#"{"schema":"web-surface/v1","id":"rustdoc","mount":"/rustdoc","built_from":"abc"}"#,
        )
        .unwrap();
        match rustdoc_tree(repo.path()) {
            Tree::At { dir, shown, mount } => {
                assert_eq!(dir, out);
                assert_eq!(shown, "target/web/rustdoc");
                assert_eq!(mount.as_deref(), Some("/rustdoc"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_repository_without_the_tree_or_the_crate_is_no_tree_and_exits_twelve() {
        use crate::quality::rustdoc::RustdocVerdict;
        let bare = tempfile::tempdir().unwrap();
        let report = judge_rustdoc(bare.path(), rustdoc_tree(bare.path())).unwrap();
        assert_eq!(report.verdict, RustdocVerdict::NoTree);
        assert_eq!(report.exit_code(), 12);
        assert!(report.reason.unwrap().contains("no Rust crate"));

        // the crate, and nothing built: still nothing to judge, and the module routes are
        // answered because they are the crate's
        let repo = repository_with_the_crate();
        let report = judge_rustdoc(repo.path(), rustdoc_tree(repo.path())).unwrap();
        assert_eq!(report.exit_code(), 12, "{report:?}");
        assert_eq!(report.krate, "majordomus_cli");
        assert_eq!(report.modules[0].route, "majordomus_cli/index.html");
    }
}
