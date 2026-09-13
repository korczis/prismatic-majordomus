//! Benchmark coverage: the denominator is generated from the registry and the
//! projection, never written down. A required capability is covered on a transport when
//! the projection has a target for it there (its input type produced a case in this
//! repository); missing when it has none; waived when its policy says so, which is
//! reported and never counted as covered. System targets are always required and always
//! covered by construction, and listed so that the total is honest.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::{BenchmarkPolicy, Context};

use super::projection::{BenchmarkProjection, Transport};
use super::system::SystemTarget;

/// The schema of the coverage document.
pub const COVERAGE_SCHEMA: &str = "majordomus/benchmark-coverage/v1";

/// Where one requirement stands: covered, missing, or waived.
///
/// Three states and not two, because a waiver is a decision and an absence is a defect,
/// and collapsing them would let either hide behind the other. A waived requirement is
/// counted in the denominator, printed in the report with the reason its descriptor gave,
/// and never counted as covered — so the ratio a reader sees goes *down* when a waiver is
/// granted, which is the honest direction.
///
/// ```
/// use majordomus_cli::bench::CoverageState;
/// // Only one of the three is evidence that something is timed.
/// let states = [CoverageState::Covered, CoverageState::Missing, CoverageState::Waived];
/// assert_eq!(states.iter().filter(|s| **s == CoverageState::Covered).count(), 1);
/// assert_ne!(CoverageState::Waived, CoverageState::Covered, "a waiver is not coverage");
/// assert_eq!(serde_json::to_value(CoverageState::Waived).unwrap(), "waived");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverageState {
    /// A target exists.
    Covered,
    /// Required, and no case could be produced for this repository.
    Missing,
    /// Waived by the descriptor, for its typed reason.
    Waived,
}

/// One requirement: a capability on a transport, or a system target.
///
/// The unit is the *pair*, not the capability: one entry exposed over three transports is
/// three requirements, and a regression in the HTTP adapter cannot be hidden by a fast
/// direct call. `cases` says how many targets feed the line, which is how `Covered` and
/// `Missing` are told apart — a required line with no case is missing, and the count is
/// printed so that a line covered by a single thin case is visible as one.
///
/// ```
/// use majordomus_cli::bench::{CoverageLine, CoverageState, Transport};
///
/// let line: CoverageLine = serde_json::from_value(serde_json::json!({
///     "subject": "context.resolve",
///     "module": "context",
///     "transport": "http",
///     "state": "missing",
///     "cases": 0
/// }))
/// .unwrap();
/// assert_eq!(line.transport, Transport::Http);
/// assert_eq!(line.state, CoverageState::Missing);
/// assert_eq!(line.cases, 0, "nothing feeds it, which is what missing means");
/// assert!(line.reason.is_none(), "a reason belongs to a waiver");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CoverageLine {
    /// The capability id, or the system target's key.
    pub subject: String,
    /// The module, or `system`.
    pub module: String,
    /// The transport.
    pub transport: Transport,
    /// Where it stands.
    pub state: CoverageState,
    /// How many cases feed it.
    pub cases: usize,
    /// The waiver's reason, when waived.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Per-transport tallies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Tally {
    /// Requirements.
    pub required: usize,
    /// Covered.
    pub covered: usize,
    /// Missing.
    pub missing: usize,
    /// Waived.
    pub waived: usize,
}

/// The module name this projection gives the transports' own targets, and the bucket it
/// tallies them under. It is not a capability module and no capability module may be
/// called it: a capability whose module were `system` would have its per-transport lines
/// tallied here instead, and would vanish from the direct, MCP and HTTP denominators
/// without any check saying so. The unit test `reserved_namespace_is_not_a_module` holds
/// it shut.
pub const SYSTEM_MODULE: &str = "system";

/// The coverage document: every requirement, and the tallies over them.
///
/// The denominator is the point. `lines` is derived from the registry and the projection
/// on every run, so a capability added today is a requirement today and cannot be covered
/// by not being counted. `tallies` is a sum over those lines and holds no fact of its own:
/// each line lands in exactly one transport bucket — or in `system`, for the transports'
/// own targets — and in `total`, so `total.required` is the number of lines and the buckets
/// partition it.
///
/// ```
/// use majordomus_cli::bench::Coverage;
///
/// let coverage: Coverage = serde_json::from_value(serde_json::json!({
///     "schema": "majordomus/benchmark-coverage/v1",
///     "lines": [
///         { "subject": "context.resolve", "module": "context", "transport": "direct",
///           "state": "covered", "cases": 2 },
///         { "subject": "context.resolve", "module": "context", "transport": "http",
///           "state": "covered", "cases": 2 },
///         { "subject": "system.http.openapi", "module": "system", "transport": "http",
///           "state": "covered", "cases": 1 }
///     ],
///     "tallies": {
///         "direct": { "required": 1, "covered": 1, "missing": 0, "waived": 0 },
///         "http":   { "required": 1, "covered": 1, "missing": 0, "waived": 0 },
///         "system": { "required": 1, "covered": 1, "missing": 0, "waived": 0 },
///         "total":  { "required": 3, "covered": 3, "missing": 0, "waived": 0 }
///     }
/// }))
/// .unwrap();
///
/// // The buckets partition the lines: nothing is counted twice and nothing escapes.
/// let buckets: usize = coverage
///     .tallies
///     .iter()
///     .filter(|(name, _)| name.as_str() != "total")
///     .map(|(_, t)| t.required)
///     .sum();
/// assert_eq!(buckets, coverage.lines.len());
/// assert_eq!(coverage.tallies["total"].required, coverage.lines.len());
/// assert!(coverage.is_complete());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Coverage {
    /// `majordomus/benchmark-coverage/v1`.
    pub schema: String,
    /// Every requirement.
    pub lines: Vec<CoverageLine>,
    /// Tallies by transport name, plus `system` and `total`.
    pub tallies: BTreeMap<String, Tally>,
}

impl Coverage {
    /// Compute coverage for a context: the requirements from the registry, the evidence
    /// from the projection.
    ///
    /// The two arguments are the numerator and the denominator, and they are read from
    /// different places on purpose. The registry says what is *required* — every executable,
    /// stable entry, once per transport its exposure declares. The projection says what is
    /// *timed*. Nothing is written down twice, so the only way to raise coverage is to make
    /// the projection produce a target, and the only way to lower the denominator is to stop
    /// exposing a capability.
    ///
    /// ```no_run
    /// use majordomus_cli::app::App;
    /// use majordomus_cli::bench::{BenchmarkProjection, Coverage};
    /// use majordomus_cli::cli::{DiscoveryMode, RepoArgs};
    ///
    /// let app = App::load(&RepoArgs {
    ///     repo: Some("<repository>".into()),
    ///     discovery: DiscoveryMode::Vcs,
    ///     strict: false,
    ///     share: None,
    /// })
    /// .unwrap();
    /// let projection = BenchmarkProjection::from_context(&app.context);
    /// let coverage = Coverage::compute(&app.context, &projection);
    ///
    /// // A line's `cases` is the projection's count for that pair, not a second opinion.
    /// for line in coverage.lines.iter().filter(|l| l.module != "system") {
    ///     let timed = projection
    ///         .of_capability(&line.subject)
    ///         .filter(|t| t.transport() == line.transport)
    ///         .count();
    ///     assert_eq!(line.cases, timed);
    /// }
    /// ```
    pub fn compute(ctx: &Context, projection: &BenchmarkProjection) -> Self {
        let mut lines = Vec::new();
        for c in ctx.registry.iter() {
            if !c.kind.is_executable() || !c.stability.executable() {
                continue;
            }
            let exposures = [
                (Transport::Direct, true),
                (
                    Transport::Mcp,
                    c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()),
                ),
                (Transport::Http, c.exposure.http.is_some()),
            ];
            for (transport, exposed) in exposures {
                if !exposed {
                    continue;
                }
                let cases = projection
                    .of_capability(c.id.as_str())
                    .filter(|t| t.transport() == transport)
                    .count();
                let (state, reason) = match c.benchmark {
                    BenchmarkPolicy::Waived { reason } => (
                        CoverageState::Waived,
                        Some(
                            serde_json::to_value(reason)
                                .ok()
                                .and_then(|v| v.as_str().map(str::to_string))
                                .unwrap_or_default(),
                        ),
                    ),
                    BenchmarkPolicy::Required if cases > 0 => (CoverageState::Covered, None),
                    BenchmarkPolicy::Required => (CoverageState::Missing, None),
                };
                lines.push(CoverageLine {
                    subject: c.id.to_string(),
                    module: c.module.as_str().to_string(),
                    transport,
                    state,
                    cases,
                    reason,
                });
            }
        }
        for s in SystemTarget::ALL {
            lines.push(CoverageLine {
                subject: s.key().to_string(),
                module: SYSTEM_MODULE.into(),
                transport: s.transport(),
                state: if projection.targets.iter().any(|t| t.key == s.key()) {
                    CoverageState::Covered
                } else {
                    CoverageState::Missing
                },
                cases: 1,
                reason: None,
            });
        }
        let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
        for line in &lines {
            // `system` is this projection's own bucket for the transports' targets, which
            // is why no capability module may be called that: `reserved_namespace_is_not_a_module`
            let bucket = if line.module == SYSTEM_MODULE {
                "system".to_string()
            } else {
                line.transport.name().to_string()
            };
            for key in [bucket, "total".to_string()] {
                let t = tallies.entry(key).or_default();
                t.required += 1;
                match line.state {
                    CoverageState::Covered => t.covered += 1,
                    CoverageState::Missing => t.missing += 1,
                    CoverageState::Waived => t.waived += 1,
                }
            }
        }
        Coverage {
            schema: COVERAGE_SCHEMA.into(),
            lines,
            tallies,
        }
    }

    /// Nothing missing, nothing waived: the state the repository wants.
    ///
    /// The stricter of the pair. It and [`has_no_missing`](Coverage::has_no_missing) differ
    /// on exactly one case — a standing waiver — and that difference is deliberate: the
    /// gate refuses a defect and reports a decision, so a waiver keeps the run green while
    /// it stops the repository from claiming complete coverage.
    ///
    /// ```
    /// use majordomus_cli::bench::Coverage;
    ///
    /// let waived: Coverage = serde_json::from_value(serde_json::json!({
    ///     "schema": "majordomus/benchmark-coverage/v1",
    ///     "lines": [{ "subject": "mesh.status", "module": "mesh", "transport": "mcp",
    ///                 "state": "waived", "cases": 0, "reason": "measures another process" }],
    ///     "tallies": {
    ///         "mcp":   { "required": 1, "covered": 0, "missing": 0, "waived": 1 },
    ///         "total": { "required": 1, "covered": 0, "missing": 0, "waived": 1 }
    ///     }
    /// }))
    /// .unwrap();
    ///
    /// assert!(!waived.is_complete(), "a waiver stands between this and complete");
    /// assert!(waived.has_no_missing(), "and still nothing is missing, so the gate passes");
    /// ```
    pub fn is_complete(&self) -> bool {
        let total = self.tallies.get("total").cloned().unwrap_or_default();
        total.missing == 0 && total.waived == 0
    }

    /// Nothing missing; waivers reported but not failing. What `bench coverage --check`
    /// exits on.
    ///
    /// A document with no `total` tally has nothing missing, because it has nothing at all:
    /// the check is over the lines that exist, and an empty run is not a failure to
    /// benchmark. What it is not is a pass anyone should read as coverage — the tallies are
    /// printed beside the verdict for that reason.
    ///
    /// ```
    /// use majordomus_cli::bench::Coverage;
    ///
    /// let missing: Coverage = serde_json::from_value(serde_json::json!({
    ///     "schema": "majordomus/benchmark-coverage/v1",
    ///     "lines": [{ "subject": "context.resolve", "module": "context", "transport": "http",
    ///                 "state": "missing", "cases": 0 }],
    ///     "tallies": {
    ///         "http":  { "required": 1, "covered": 0, "missing": 1, "waived": 0 },
    ///         "total": { "required": 1, "covered": 0, "missing": 1, "waived": 0 }
    ///     }
    /// }))
    /// .unwrap();
    /// assert!(!missing.has_no_missing(), "a required pair no case feeds fails the check");
    ///
    /// let empty: Coverage = serde_json::from_value(serde_json::json!({
    ///     "schema": "majordomus/benchmark-coverage/v1", "lines": [], "tallies": {}
    /// }))
    /// .unwrap();
    /// assert!(empty.has_no_missing(), "nothing required, nothing missing");
    /// assert!(empty.is_complete());
    /// ```
    pub fn has_no_missing(&self) -> bool {
        self.tallies.get("total").is_none_or(|t| t.missing == 0)
    }

    /// The human report: a ratio per bucket, the totals, then every line that is not
    /// covered.
    ///
    /// Covered lines are summed and not named — there is nothing to do about them — while
    /// each missing or waived line is printed with its transport and, for a waiver, the
    /// reason its descriptor gave. So the report is short when the repository is in good
    /// order and grows by exactly what is wrong.
    ///
    /// ```
    /// use majordomus_cli::bench::Coverage;
    ///
    /// let coverage: Coverage = serde_json::from_value(serde_json::json!({
    ///     "schema": "majordomus/benchmark-coverage/v1",
    ///     "lines": [
    ///         { "subject": "context.resolve", "module": "context", "transport": "direct",
    ///           "state": "covered", "cases": 2 },
    ///         { "subject": "mesh.status", "module": "mesh", "transport": "mcp",
    ///           "state": "waived", "cases": 0, "reason": "measures another process" }
    ///     ],
    ///     "tallies": {
    ///         "direct": { "required": 1, "covered": 1, "missing": 0, "waived": 0 },
    ///         "mcp":    { "required": 1, "covered": 0, "missing": 0, "waived": 1 },
    ///         "total":  { "required": 2, "covered": 1, "missing": 0, "waived": 1 }
    ///     }
    /// }))
    /// .unwrap();
    ///
    /// let report = coverage.render();
    /// assert!(report.contains("WAIVED   mesh.status on mcp (measures another process)"));
    /// assert!(!report.contains("context.resolve"), "a covered line is counted, not listed");
    /// assert!(report.contains("total          1 /   2"));
    /// ```
    pub fn render(&self) -> String {
        let mut s = String::from("Benchmark coverage\n\n");
        for (name, t) in &self.tallies {
            if name == "total" {
                continue;
            }
            s.push_str(&format!(
                "{:<12} {:>3} / {:>3}{}\n",
                name,
                t.covered,
                t.required,
                if t.missing + t.waived > 0 {
                    format!("   missing {} waived {}", t.missing, t.waived)
                } else {
                    String::new()
                }
            ));
        }
        let total = self.tallies.get("total").cloned().unwrap_or_default();
        s.push_str(&format!(
            "\ntotal        {:>3} / {:>3}\nmissing      {:>3}\nwaived       {:>3}\n",
            total.covered, total.required, total.missing, total.waived
        ));
        for line in self
            .lines
            .iter()
            .filter(|l| l.state != CoverageState::Covered)
        {
            s.push_str(&format!(
                "{:<8} {} on {}{}\n",
                format!("{:?}", line.state).to_uppercase(),
                line.subject,
                line.transport.name(),
                line.reason
                    .as_ref()
                    .map(|r| format!(" ({r})"))
                    .unwrap_or_default()
            ));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    /// The one name this projection reserves is not taken by a capability module. Adding a
    /// module called `system` would compile, validate, and quietly move three coverage
    /// lines per capability into the transports' bucket; this is where that stops.
    #[test]
    fn reserved_namespace_is_not_a_module() {
        for m in crate::capability::builtin::modules() {
            assert_ne!(
                m.id.as_str(),
                super::SYSTEM_MODULE,
                "the benchmark projection reserves `{}` for the transports' own targets; \
                 a capability module of that name has its lines tallied as transport \
                 targets and disappears from the per-transport denominators",
                super::SYSTEM_MODULE
            );
        }
    }
}
