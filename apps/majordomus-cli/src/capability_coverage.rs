//! The coverage matrix: for every executable capability, whether it is exposed, documented,
//! tested, enforced and benchmarked, each cell naming the artifact that supports it (I1011).
//!
//! Nothing here is a second opinion. A cell is read off the engine that already decides it:
//! the declaration's own exposure, the evidence subject index's routes (the same walk
//! `evidence show` makes) for `tested` and `enforced`, and the benchmark projection's coverage
//! for `benchmarked`. `documented` is the one column this module reads itself, and it reads
//! only what a person wrote: a hand-written document under `docs/` that names the capability,
//! or an example of its command. The generated reference documents every capability by
//! construction, so it is never counted.
//!
//! A cell is never green without an artifact. A relation that claims evidence and does not
//! resolve is a gap whatever the policy requires, and the gap says which end failed: the
//! subject the index does not hold, or the test file the tree does not carry. Which columns a
//! capability must resolve is the repository's `coverage:` policy, never a condition here.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::bench::{BenchmarkProjection, Coverage, CoverageState};
use crate::capability::handler::Context;
use crate::capability::model::{Capability, Stability};
use crate::evidence::subject::{self, Route, SubjectIndex, Via};
use crate::policy::CoveragePolicy;

/// The schema id of the matrix.
pub const SCHEMA: &str = "majordomus/capability-coverage/v1";

/// One column of the matrix.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CoverageColumn {
    /// A transport answers it: an MCP tool, an HTTP route, a command, a skill.
    Exposed,
    /// A hand-written document names it, or its command has an example.
    Documented,
    /// A test reaches it through the evidence subject index.
    Tested,
    /// A rule's enforcement reaches it through the evidence subject index.
    Enforced,
    /// The benchmark projection times it on every transport it is exposed on.
    Benchmarked,
}

impl CoverageColumn {
    /// Every column, in the order the matrix shows them.
    pub const ALL: [CoverageColumn; 5] = [
        CoverageColumn::Exposed,
        CoverageColumn::Documented,
        CoverageColumn::Tested,
        CoverageColumn::Enforced,
        CoverageColumn::Benchmarked,
    ];

    /// The word the policy and the wire spell it with.
    pub fn as_str(self) -> &'static str {
        match self {
            CoverageColumn::Exposed => "exposed",
            CoverageColumn::Documented => "documented",
            CoverageColumn::Tested => "tested",
            CoverageColumn::Enforced => "enforced",
            CoverageColumn::Benchmarked => "benchmarked",
        }
    }
}

/// What a cell says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverageCellState {
    /// Every artifact it names is there.
    Resolved,
    /// Declared not to apply, with the reason in `gap`: a benchmark waiver.
    Waived,
    /// Nothing supports it and the policy does not require it.
    Absent,
    /// Required and unsupported, or a relation that does not resolve; `gap` says which.
    Gap,
    /// The column cannot apply: a capability the benchmark projection does not time.
    NotApplicable,
}

/// One cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityCoverageCell {
    /// The column.
    pub column: CoverageColumn,
    /// Whether the repository's policy requires it.
    pub required: bool,
    /// What it says.
    pub state: CoverageCellState,
    /// What supports it, each something a reader can follow: a path, a tool, a route, a
    /// test id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,
    /// For a gap, what is missing and which end failed; for a waiver, its reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
}

/// One capability's row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityCoverageRow {
    /// The capability id.
    pub capability: String,
    /// Its module.
    pub module: String,
    /// `query` or `command`.
    pub kind: String,
    /// Its declared stability.
    pub stability: String,
    /// One cell per column, in [`CoverageColumn::ALL`]'s order.
    pub cells: Vec<CapabilityCoverageCell>,
}

/// How one column stands over every row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CoverageColumnTally {
    /// Cells resolved.
    pub resolved: usize,
    /// Cells waived.
    pub waived: usize,
    /// Cells absent and not required.
    pub absent: usize,
    /// Cells that are gaps.
    pub gaps: usize,
    /// Cells that cannot apply.
    pub not_applicable: usize,
}

/// The matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityCoverage {
    /// [`SCHEMA`].
    pub schema: String,
    /// The columns, in order.
    pub columns: Vec<CoverageColumn>,
    /// One row per executable capability, in the registry's order.
    pub rows: Vec<CapabilityCoverageRow>,
    /// Per column, by its word.
    pub tallies: BTreeMap<String, CoverageColumnTally>,
    /// Every gap cell, over every row.
    pub gaps: usize,
    /// What is wrong with the policy itself: a column it names that the matrix has not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

/// The matrix of every executable capability the context's registry holds, under `policy`.
pub fn matrix(ctx: &Context, policy: &CoveragePolicy) -> CapabilityCoverage {
    let root = Path::new(&ctx.index.repository.root);
    let subjects = subject::index(&ctx.index, &ctx.product, &ctx.registry);
    let projection = BenchmarkProjection::from_context(ctx);
    let bench = Coverage::compute(ctx, &projection);
    let documents = Documents::read(root);
    let findings: Vec<String> = policy
        .require
        .iter()
        .flat_map(|r| r.columns.iter())
        .filter(|c| !CoverageColumn::ALL.iter().any(|k| k.as_str() == c.as_str()))
        .map(|c| format!("coverage.require names the column '{c}', which the matrix has not"))
        .collect();

    let mut rows = Vec::new();
    for c in ctx.registry.iter().filter(|c| c.kind.is_executable()) {
        let stability = word(&c.stability);
        let required = |column: CoverageColumn| {
            policy.require.iter().any(|r| {
                r.kind.as_deref().is_none_or(|k| k == c.kind.as_str())
                    && r.stability.as_deref().is_none_or(|s| s == stability)
                    && r.columns.iter().any(|col| col == column.as_str())
            })
        };
        let cells = CoverageColumn::ALL
            .iter()
            .map(|column| {
                let reading = match column {
                    CoverageColumn::Exposed => exposed(c),
                    CoverageColumn::Documented => documents.naming(c),
                    CoverageColumn::Tested => routes(&subjects, c, root, false),
                    CoverageColumn::Enforced => routes(&subjects, c, root, true),
                    CoverageColumn::Benchmarked => benchmarked(&bench, c),
                };
                reading.cell(*column, required(*column))
            })
            .collect();
        rows.push(CapabilityCoverageRow {
            capability: c.id.as_str().to_string(),
            module: c.module.as_str().to_string(),
            kind: c.kind.as_str().to_string(),
            stability,
            cells,
        });
    }

    let mut tallies: BTreeMap<String, CoverageColumnTally> = CoverageColumn::ALL
        .iter()
        .map(|c| (c.as_str().to_string(), CoverageColumnTally::default()))
        .collect();
    let mut gaps = 0;
    for cell in rows.iter().flat_map(|r| r.cells.iter()) {
        let tally = tallies.entry(cell.column.as_str().to_string()).or_default();
        match cell.state {
            CoverageCellState::Resolved => tally.resolved += 1,
            CoverageCellState::Waived => tally.waived += 1,
            CoverageCellState::Absent => tally.absent += 1,
            CoverageCellState::Gap => {
                tally.gaps += 1;
                gaps += 1;
            }
            CoverageCellState::NotApplicable => tally.not_applicable += 1,
        }
    }
    CapabilityCoverage {
        schema: SCHEMA.to_string(),
        columns: CoverageColumn::ALL.to_vec(),
        rows,
        tallies,
        gaps,
        findings,
    }
}

/// What a column found for one capability, before the policy says whether it is required.
enum Reading {
    /// These artifacts support it.
    Found(Vec<String>),
    /// Nothing supports it; the text says what was looked for.
    Missing(String),
    /// A relation claims evidence that does not resolve; the text says which end failed.
    Dangling(Vec<String>, String),
    /// Declared not to apply, for this reason.
    Waived(String),
    /// The column cannot apply.
    NotApplicable(String),
}

impl Reading {
    fn cell(self, column: CoverageColumn, required: bool) -> CapabilityCoverageCell {
        let (state, artifacts, gap) = match self {
            Reading::Found(a) => (CoverageCellState::Resolved, a, None),
            Reading::Missing(why) if required => (CoverageCellState::Gap, Vec::new(), Some(why)),
            Reading::Missing(_) => (CoverageCellState::Absent, Vec::new(), None),
            Reading::Dangling(a, why) => (CoverageCellState::Gap, a, Some(why)),
            Reading::Waived(why) => (CoverageCellState::Waived, Vec::new(), Some(why)),
            Reading::NotApplicable(why) => {
                (CoverageCellState::NotApplicable, Vec::new(), Some(why))
            }
        };
        CapabilityCoverageCell {
            column,
            required,
            state,
            artifacts,
            gap,
        }
    }
}

/// A value's serialised word: a stability is spelled as the declarations spell it.
fn word<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn exposed(c: &Capability) -> Reading {
    let mut found = Vec::new();
    if let Some(tool) = c.exposure.mcp.as_ref().and_then(|m| m.tool.as_ref()) {
        found.push(format!("mcp:{tool}"));
    }
    if let Some(http) = &c.exposure.http {
        found.push(format!("http:{} {}", http.method.as_str(), http.path));
    }
    if let Some(cli) = &c.exposure.cli {
        found.push(format!("cli:majordomus {}", cli.path.join(" ")));
    }
    if let Some(skill) = &c.skill {
        found.push(format!("skill:{}", skill.alias));
    }
    if found.is_empty() {
        Reading::Missing("the declaration exposes it on no transport".into())
    } else {
        Reading::Found(found)
    }
}

/// The routes the evidence index walks from `capability:<id>`: the rule-enforcement routes
/// for `enforced`, every other route for `tested`.
fn routes(subjects: &SubjectIndex, c: &Capability, root: &Path, enforced: bool) -> Reading {
    let key = format!("capability:{}", c.id.as_str());
    let reached: Vec<&Route> = match subjects.reach(&key) {
        Ok(r) => r,
        Err(_) => {
            return Reading::Dangling(
                Vec::new(),
                format!("subject end: the evidence index holds no {key}"),
            )
        }
    };
    let mine: Vec<&Route> = reached
        .into_iter()
        .filter(|r| (r.via == Via::Rule) == enforced)
        .collect();
    let artifacts: Vec<String> = mine
        .iter()
        .filter_map(|r| r.path.clone().or_else(|| r.test.clone()))
        .collect();
    let missing: Vec<&str> = mine
        .iter()
        .filter_map(|r| r.path.as_deref())
        .filter(|p| !root.join(p).exists())
        .collect();
    if let Some(first) = missing.first() {
        return Reading::Dangling(
            artifacts.clone(),
            format!(
                "test end: {first} is named{} and the tree does not carry it",
                if missing.len() > 1 {
                    format!(" (and {} more)", missing.len() - 1)
                } else {
                    String::new()
                }
            ),
        );
    }
    if artifacts.is_empty() {
        Reading::Missing(if enforced {
            "no rule's enforcement reaches it".into()
        } else {
            "no test reaches it: no claim, case header or example names it".into()
        })
    } else {
        Reading::Found(artifacts)
    }
}

fn benchmarked(bench: &Coverage, c: &Capability) -> Reading {
    let lines: Vec<_> = bench
        .lines
        .iter()
        .filter(|l| l.subject == c.id.as_str())
        .collect();
    if lines.is_empty() {
        return Reading::NotApplicable(
            "the benchmark projection times executable capabilities that are not planned".into(),
        );
    }
    let missing: Vec<&str> = lines
        .iter()
        .filter(|l| matches!(l.state, CoverageState::Missing))
        .map(|l| l.transport.name())
        .collect();
    if !missing.is_empty() {
        return Reading::Missing(format!("no benchmark case on {}", missing.join(", ")));
    }
    let covered: Vec<String> = lines
        .iter()
        .filter(|l| matches!(l.state, CoverageState::Covered))
        .map(|l| format!("bench:{} ({} case(s))", l.transport.name(), l.cases))
        .collect();
    if covered.is_empty() {
        let reason = lines
            .iter()
            .find_map(|l| l.reason.clone())
            .unwrap_or_else(|| "waived".into());
        return Reading::Waived(reason);
    }
    Reading::Found(covered)
}

/// Every hand-written document under `docs/`, read once: the generated reference names every
/// capability by construction and is left out.
struct Documents {
    texts: BTreeMap<String, String>,
}

impl Documents {
    fn read(root: &Path) -> Documents {
        let mut texts = BTreeMap::new();
        let mut stack = vec![root.join("docs")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(rel) = path.strip_prefix(root) else {
                    continue;
                };
                let rel = rel.to_string_lossy().replace('\\', "/");
                if rel.starts_with("docs/generated") {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else if rel.ends_with(".md") {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        texts.insert(rel, text);
                    }
                }
            }
        }
        Documents { texts }
    }

    /// The documents that name `c` by its id or its MCP tool, and the examples of its command.
    fn naming(&self, c: &Capability) -> Reading {
        let id = c.id.as_str();
        let tool = c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref());
        let mut found: Vec<String> = self
            .texts
            .iter()
            .filter(|(_, text)| text.contains(id) || tool.is_some_and(|t| text.contains(t)))
            .map(|(path, _)| path.clone())
            .collect();
        if let Some(cli) = &c.exposure.cli {
            let command = cli.path.join(" ");
            found.extend(
                crate::cli::EXAMPLES
                    .iter()
                    .filter(|e| e.command == command)
                    .flat_map(|e| e.examples.iter())
                    .map(|e| format!("example:{}", e.id)),
            );
        }
        if found.is_empty() {
            Reading::Missing(
                "no hand-written document names it and its command has no example; only the \
                 generated reference does"
                    .into(),
            )
        } else {
            Reading::Found(found)
        }
    }
}

/// Whether the repository at `root` carries this executable's own crate (I3019): the only
/// repository whose tests can reach this executable's capabilities. In any other — a
/// fixture, an adopter — no case names `capability:<id>` of this executable, and judging its
/// declarations there would refuse every one of them for a property the repository cannot
/// have.
pub fn carries_this_crate(root: &Path) -> bool {
    let dir = root.join(crate::capability::model::CRATE_DIR);
    let named = format!("name = \"{}\"", env!("CARGO_PKG_NAME"));
    let declared = std::fs::read_to_string(dir.join("Cargo.toml"))
        .is_ok_and(|text| text.lines().any(|line| line.trim() == named));
    declared && dir.join("src/lib.rs").is_file()
}

/// Every executable capability declared behaviorally verified that no test reaches (I3019),
/// by the routes the matrix's `tested` column reads, so the matrix and this refusal cannot
/// disagree: a declaration that says verified is a measurement, or it is refused.
pub fn verified_without_a_test(ctx: &Context) -> Vec<String> {
    let root = Path::new(&ctx.index.repository.root);
    let subjects = subject::index(&ctx.index, &ctx.product, &ctx.registry);
    ctx.registry
        .iter()
        .filter(|c| c.kind.is_executable() && c.stability == Stability::BehaviorallyVerified)
        .filter(|c| !matches!(routes(&subjects, c, root, false), Reading::Found(_)))
        .map(|c| c.id.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::CoverageRequirement;

    fn rule(kind: Option<&str>, stability: Option<&str>, columns: &[&str]) -> CoverageRequirement {
        CoverageRequirement {
            kind: kind.map(str::to_string),
            stability: stability.map(str::to_string),
            columns: columns.iter().map(|c| c.to_string()).collect(),
        }
    }

    fn cell(row: &CapabilityCoverageRow, column: CoverageColumn) -> &CapabilityCoverageCell {
        row.cells.iter().find(|c| c.column == column).unwrap()
    }

    /// A gap whose relation dangles names the end that failed.
    fn dangles(c: &CapabilityCoverageCell) -> bool {
        c.gap
            .as_deref()
            .is_some_and(|g| g.starts_with("test end:") || g.starts_with("subject end:"))
    }

    #[test]
    fn a_cell_is_never_green_without_an_artifact() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let m = matrix(&ctx, &CoveragePolicy::default());
        assert!(!m.rows.is_empty());
        assert!(
            m.rows.iter().all(|r| r.kind != "resource"),
            "executable capabilities only"
        );
        for c in m.rows.iter().flat_map(|r| r.cells.iter()) {
            assert_eq!(
                c.state == CoverageCellState::Resolved,
                !c.artifacts.is_empty() && c.gap.is_none(),
                "{c:?}"
            );
            assert!(
                !c.required,
                "a policy that requires nothing requires no cell"
            );
            if c.state == CoverageCellState::Gap {
                assert!(
                    dangles(c),
                    "unrequired, only a dangling relation is a gap: {c:?}"
                );
            }
        }
        let health = m
            .rows
            .iter()
            .find(|r| r.capability == "health.report")
            .unwrap();
        let exposed = cell(health, CoverageColumn::Exposed);
        assert!(exposed
            .artifacts
            .contains(&"mcp:majordomus_health".to_string()));
        assert!(exposed
            .artifacts
            .contains(&"http:GET /api/v1/health".to_string()));
    }

    #[test]
    fn the_policy_decides_what_is_required_and_a_query_owes_no_test_unless_asked() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let m = matrix(
            &ctx,
            &CoveragePolicy {
                require: vec![
                    rule(Some("command"), None, &["tested"]),
                    rule(None, Some("behaviorally_verified"), &["exposed"]),
                    rule(None, None, &["nonsense"]),
                ],
            },
        );
        assert_eq!(
            m.findings.len(),
            1,
            "an unknown column is the policy's finding"
        );
        for row in &m.rows {
            let tested = cell(row, CoverageColumn::Tested);
            assert_eq!(tested.required, row.kind == "command", "{}", row.capability);
            if row.kind == "query" && tested.state == CoverageCellState::Gap {
                assert!(
                    dangles(tested),
                    "a query is not asked for a test: {tested:?}"
                );
            }
            assert_eq!(
                cell(row, CoverageColumn::Exposed).required,
                row.stability == "behaviorally_verified"
            );
            if tested.required && tested.artifacts.is_empty() {
                assert_eq!(
                    tested.state,
                    CoverageCellState::Gap,
                    "required and unsupported"
                );
            }
        }
        let counted: usize = m.tallies.values().map(|t| t.gaps).sum();
        assert_eq!(counted, m.gaps, "the tallies count the cells");
    }

    #[test]
    fn a_relation_that_does_not_resolve_is_a_gap_even_when_nothing_requires_it() {
        let reading = Reading::Dangling(
            vec!["test/cases/01_gone.sh".into()],
            "test end: test/cases/01_gone.sh is named and the tree does not carry it".into(),
        );
        let c = reading.cell(CoverageColumn::Tested, false);
        assert_eq!(c.state, CoverageCellState::Gap);
        assert!(dangles(&c));
        assert_eq!(
            c.artifacts,
            ["test/cases/01_gone.sh"],
            "what was claimed is still named"
        );
        let absent =
            Reading::Missing("no test reaches it".into()).cell(CoverageColumn::Tested, false);
        assert_eq!(absent.state, CoverageCellState::Absent);
        assert_eq!(absent.gap, None);
    }

    /// The refusal and the matrix read the same routes (I3019): the capabilities refused
    /// are exactly the verified rows whose `tested` cell is not resolved.
    #[test]
    fn the_refusal_reads_the_matrix_routes() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let m = matrix(&ctx, &CoveragePolicy::default());
        let from_matrix: Vec<String> = m
            .rows
            .iter()
            .filter(|r| r.stability == "behaviorally_verified")
            .filter(|r| cell(r, CoverageColumn::Tested).state != CoverageCellState::Resolved)
            .map(|r| r.capability.clone())
            .collect();
        assert_eq!(verified_without_a_test(&ctx), from_matrix);
    }

    /// Only the tree that carries this crate is judged: this one is, an empty directory and
    /// a synthetic repository are not.
    #[test]
    fn only_the_repository_that_carries_the_crate_is_judged() {
        let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(carries_this_crate(&here));
        let empty = tempfile::tempdir().unwrap();
        assert!(!carries_this_crate(empty.path()));
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        assert!(!carries_this_crate(repo.root()));
    }
}
